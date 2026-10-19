//! Type identity of flying machines (the clock study's rule, `clock::ident`, docs/clocks.md): two
//! machines are one type when they are the same structure up to translation, the 4 rotations and
//! the mirror image, after the block swaps that leave the run identical:
//!
//! * a sticky piston that never pulls counts as a piston,
//! * a honey block whose behavior a slime block would copy counts as slime,
//! * a slime or honey block whose stickiness is never used counts as stone,
//! * an observer that never matters counts as stone.
//!
//! Each swap is checked on redsim: the swapped machine's frames, tick for tick over several
//! periods, must equal the original's with the swapped kind mapped onto the new one in both runs
//! (positions and every other state identical). The swaps are applied one cell at a time until
//! none applies: the NORMAL FORM; its `clock::canon::type_key` is the machine's type key.

use crate::machine::Machine;
use crate::sim::{self, SimOptions};
use redsim::block::Simple;
use redsim::{Block, BlockPos};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Swap {
    StickyToPiston,
    HoneyToSlime,
    SlimeToStone,
    HoneyToStone,
    ObserverToStone,
}

impl Swap {
    pub const ALL: [Swap; 5] = [Swap::StickyToPiston, Swap::HoneyToSlime, Swap::SlimeToStone, Swap::HoneyToStone, Swap::ObserverToStone];

    pub fn word(self) -> &'static str {
        match self {
            Swap::StickyToPiston => "sticky_piston->piston",
            Swap::HoneyToSlime => "honey->slime",
            Swap::SlimeToStone => "slime->stone",
            Swap::HoneyToStone => "honey->stone",
            Swap::ObserverToStone => "observer->stone",
        }
    }

    /// The replacement for a design cell's state, when the swap applies to it.
    fn replace(self, state: &str) -> Option<String> {
        let id = state.split('[').next().unwrap_or(state);
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        match (self, id) {
            (Swap::StickyToPiston, "sticky_piston") => Some(state.replacen("sticky_piston", "piston", 1)),
            (Swap::HoneyToSlime, "honey_block") => Some("minecraft:slime_block".into()),
            (Swap::SlimeToStone, "slime_block") | (Swap::HoneyToStone, "honey_block") | (Swap::ObserverToStone, "observer") => Some("minecraft:stone".into()),
            _ => None,
        }
    }

    /// A run's block mapped onto its class under the swap (applied to both runs).
    fn class(self, b: Block) -> Block {
        match (self, b) {
            (Swap::StickyToPiston, Block::Piston { facing, extended, .. }) => Block::Piston { sticky: false, facing, extended },
            (Swap::StickyToPiston, Block::PistonHead { facing, short, .. }) => Block::PistonHead { sticky: false, facing, short },
            (Swap::StickyToPiston, Block::MovingPiston { facing, .. }) => Block::MovingPiston { sticky: false, facing },
            (Swap::HoneyToSlime, Block::Simple(Simple::HoneyBlock)) => Block::Simple(Simple::SlimeBlock),
            (Swap::SlimeToStone, Block::Simple(Simple::SlimeBlock)) | (Swap::HoneyToStone, Block::Simple(Simple::HoneyBlock)) | (Swap::ObserverToStone, Block::Observer { .. }) => Block::Simple(Simple::Stone),
            _ => b,
        }
    }
}

/// Ticks to compare: the first move plus 4 periods, at least 120.
fn horizon(m: &Machine) -> u32 {
    let r = sim::simulate(m, &SimOptions { distance: 12, ..Default::default() });
    let p = r.steady.as_ref().map(|s| s.period).unwrap_or(40);
    (r.first_change.unwrap_or(0) + 4 * p + 10).max(120)
}

fn same_run(a: &[Vec<(BlockPos, Block)>], b: &[Vec<(BlockPos, Block)>], swap: Swap) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(fa, fb)| fa.len() == fb.len() && fa.iter().zip(fb).all(|((pa, ba), (pb, bb))| pa == pb && swap.class(*ba) == swap.class(*bb)))
}

/// The normal form and the swaps applied (cell: swap).
pub fn normalized(m: &Machine) -> (Machine, Vec<String>) {
    let o = SimOptions { distance: 12, ..Default::default() };
    let ticks = horizon(m);
    let mut cur = m.clone();
    let mut applied = Vec::new();
    let Ok(mut base) = sim::frames(&cur, &o, ticks) else {
        return (cur, applied);
    };
    loop {
        let mut changed = false;
        let cells: Vec<(clock::Pos, String)> = cur.design().blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
        'cells: for (p, s) in cells {
            for swap in Swap::ALL {
                let Some(new) = swap.replace(&s) else { continue };
                let mut cand = cur.clone();
                cand.clock.design.blocks.insert(p, new);
                let Ok(cand) = Machine::parse(&cand.to_text()) else { continue };
                let Ok(fr) = sim::frames(&cand, &o, ticks) else { continue };
                if same_run(&base, &fr, swap) {
                    applied.push(format!("{},{},{} {}", p[0], p[1], p[2], swap.word()));
                    cur = cand;
                    base = fr;
                    changed = true;
                    break 'cells;
                }
            }
        }
        if !changed {
            break;
        }
    }
    (cur, applied)
}

/// The type key: the normal form's blocks up to translation, rotation and mirror image.
pub fn type_key(m: &Machine) -> String {
    let (n, _) = normalized(m);
    clock::canon::type_key(n.design().blocks.iter())
}
