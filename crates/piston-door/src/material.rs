//! `mctas door material` (track clk-rt, vjeux 2026-10-21 15:26Z: "please make sure we only use
//! obsidian if we need to, otherwise use stone"): for each design, every obsidian block tried as
//! stone on the redsim door screen (`redsim_screen::screen`, the judge's checks at the record
//! judge's offsets): all at once first, else one at a time, then the passing ones together (greedy
//! when the set does not pass as a whole). A replacement is kept only when the door still passes
//! with the same open and close ticks. What stays obsidian is needed: pistons cannot move it (a
//! door has no explosions). The changed designs then go to the real judge (`door verify`).

use crate::redsim_screen::{Screen, screen};
use mctas::design::Design;
use mctas::sim::Pos;

/// One design's result.
pub struct Material {
    pub obsidian: Vec<Pos>,
    pub replaced: Vec<Pos>,
    /// The baseline screen's (open, close) ticks, or why it does not pass on redsim.
    pub base: Result<(u32, u32), String>,
    /// The design with the replacements (None when nothing changed).
    pub design: Option<Design>,
}

pub const STONE: &str = "minecraft:stone";

fn is_obsidian(s: &str) -> bool {
    s == "minecraft:obsidian" || s == "obsidian"
}

/// The obsidian cells of a design.
pub fn obsidian_cells(d: &Design) -> Vec<Pos> {
    d.blocks.iter().filter(|(_, s)| is_obsidian(s)).map(|(p, _)| *p).collect()
}

/// The design with these cells turned to stone.
pub fn with_stone(d: &Design, cells: &[Pos]) -> Design {
    let mut d2 = d.clone();
    for p in cells {
        d2.blocks.insert(*p, STONE.to_string());
    }
    d2
}

/// Every obsidian block of `d` tried as stone (see the module doc).
pub fn obsidian_pass(d: &Design, offsets: &[Pos], cycles: usize, max_ticks: u32, empty_walkway: bool) -> Material {
    let cells = obsidian_cells(d);
    let pass = |x: &Design| match screen(x, offsets, cycles, max_ticks, empty_walkway) {
        Screen::Pass { open, close } => Ok((open, close)),
        Screen::Fail(m) => Err(m),
        Screen::Unsupported(m) => Err(format!("unsupported on redsim: {m}")),
    };
    let base = pass(d);
    let mut m = Material { obsidian: cells.clone(), replaced: Vec::new(), base: base.clone(), design: None };
    let Ok(ticks) = base else { return m };
    if cells.is_empty() {
        return m;
    }
    let same = |x: &Design| pass(x).is_ok_and(|t| t == ticks);
    if same(&with_stone(d, &cells)) {
        m.replaced = cells;
    } else {
        let singles: Vec<Pos> = cells.iter().copied().filter(|p| same(&with_stone(d, &[*p]))).collect();
        if !singles.is_empty() && same(&with_stone(d, &singles)) {
            m.replaced = singles;
        } else {
            for p in singles {
                let mut t = m.replaced.clone();
                t.push(p);
                if same(&with_stone(d, &t)) {
                    m.replaced = t;
                }
            }
        }
    }
    if !m.replaced.is_empty() {
        m.design = Some(with_stone(d, &m.replaced));
    }
    m
}

/// The game's rotation check (`crate::rotation_check`, the leaderboard judge's: the design built
/// turned 90/180/270 degrees at the record positions) of the original and of its stone design:
/// the first breaking turn of each, `None` = rotation-safe. The redsim screen builds no turned
/// copies, and a stone block where obsidian stood can break a turned build (clk-rt 2026-10-21:
/// bridge-p2-sculk-r-49b and 9 more broke at 180 degrees).
pub fn rotation_pair(w: &mut mctas::sim::Worker, orig: &Design, stone: &Design, opts: &crate::VerifyOptions) -> anyhow::Result<(Option<u8>, Option<u8>)> {
    let o = crate::rotation_check(w, orig, opts)?.map(|x| x.0);
    let s = crate::rotation_check(w, stone, opts)?.map(|x| x.0);
    Ok((o, s))
}

/// A stone design keeps the original's rotation result: rotation-safe, or the original is not
/// rotation-safe either.
pub fn rotation_ok(orig: Option<u8>, stone: Option<u8>) -> bool {
    stone.is_none() || orig.is_some()
}
