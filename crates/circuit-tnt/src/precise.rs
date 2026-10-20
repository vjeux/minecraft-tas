//! Precision on redsim (the PRECISE variation, designs/circuits/tnt/VARIATIONS.md): the game's
//! random kick of a primed TNT is replaced by fixed values (`World::tnt_kick`), the design runs
//! once per kick with every TNT an entity (pushed, flung, bouncing, carried by water: the redsim
//! entity port), and each TNT's state at fuse 1 is recorded and the TNT removed. A design is
//! PRECISE on redsim when every kick gives the same fuse-1 position, bit for bit; `circuit tnt
//! precision` then checks it in the game.

use crate::sim::build;
use clock::Clock;
use redsim::entity::EntityKind;
use std::collections::HashMap;

/// The default kicks: 24 even values and the edges of the kick-dependent families (a TNT pushed
/// along by its moving copy or not: the kick's component along a push just above / below 0.01).
pub fn default_kicks() -> Vec<f64> {
    let mut v: Vec<f64> = (0..24).map(|k| (k as f64 + 0.37) / 24.0).collect();
    for e in [1.0 / 6.0, 1.0 / 3.0, 2.0 / 3.0, 5.0 / 6.0, 0.25, 0.5, 0.75, 0.0] {
        v.push((e + 0.004f64).rem_euclid(1.0));
        v.push((e - 0.004f64).rem_euclid(1.0));
    }
    v
}

/// One TNT's fuse-1 state: (spawn tick, position, motion).
pub type End = (u32, [f64; 3], [f64; 3]);

/// Run with kick `u` for `ticks` gt: every TNT's state at fuse 1 (the TNT is removed there), or
/// why the run cannot be trusted. `follow`: print the first TNT's state every tick.
pub fn ends(c: &Clock, u: f64, ticks: u32) -> Result<Vec<End>, String> {
    ends_traced(c, u, ticks, false)
}

pub fn ends_traced(c: &Clock, u: f64, ticks: u32, follow: bool) -> Result<Vec<End>, String> {
    let mut w = build(c, 4)?;
    w.tnt_kick = Some(u);
    let mut actions = c.actions();
    let mut born: HashMap<u32, u32> = HashMap::new();
    let mut out = Vec::new();
    for t in 0..ticks {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            if let clock::Action::Place { pos, state, .. } = a {
                let b = redsim::Block::parse(&state).map_err(|e| format!("kick block: {e}"))?;
                w.set_block(redsim::BlockPos::new(pos[0], pos[1], pos[2]), b, redsim::world::flags::ALL);
            }
        }
        w.tick();
        if let Some(e) = w.errors.first() {
            return Err(e.clone());
        }
        for e in w.entities.list.iter_mut() {
            if let EntityKind::PrimedTnt(s) = e.kind {
                born.entry(e.id).or_insert(t + 1);
                if follow && born.len() == 1 {
                    println!("  gt {}: fuse {} pos ({:.4}, {:.4}, {:.4}) motion ({:.4}, {:.4}, {:.4}){}", t + 1, s.fuse, e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z, if e.on_ground { " ground" } else { "" });
                }
                if s.fuse <= 1 && !e.removed {
                    out.push((born[&e.id], [e.pos.x, e.pos.y, e.pos.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z]));
                    e.removed = true;
                }
            }
        }
        w.entities.list.retain(|e| !e.removed);
    }
    Ok(out)
}

/// Over `kicks`: the fuse-1 position of the `nth` TNT of each run (a steady-state one), grouped:
/// (position, how many kicks), most common first; and the kicks whose run failed.
pub fn groups(c: &Clock, kicks: &[f64], ticks: u32, nth: usize) -> (Vec<([f64; 3], usize)>, Vec<String>) {
    let mut g: Vec<([f64; 3], usize)> = Vec::new();
    let mut bad = Vec::new();
    for &u in kicks {
        match ends(c, u, ticks) {
            Ok(v) if v.len() > nth => {
                let p = v[nth].1;
                match g.iter_mut().find(|(q, _)| q.iter().zip(&p).all(|(a, b)| a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0))) {
                    Some(e) => e.1 += 1,
                    None => g.push((p, 1)),
                }
            }
            Ok(v) => bad.push(format!("kick {u:.4}: only {} TNT reached fuse 1", v.len())),
            Err(e) => bad.push(format!("kick {u:.4}: {e}")),
        }
    }
    g.sort_by(|a, b| b.1.cmp(&a.1));
    (g, bad)
}
