//! Kick search (`mctas clock rt kick`): try a one-time start kick — a temporary block placed in an
//! air cell for T gt, starting at tick N — and report the clocks it makes. Temporary blocks: a
//! redstone block next to a dust of the design; a wall torch (or a standing torch) directly under
//! a plain block of the design (it strongly powers that block: a torch hanging on it turns off).
//! Every redstone-tick delay is even, so a kick at an ODD tick starts a second parity class: the
//! only way to an odd period (docs/clocks.md, NOTES.md).

use crate::sweep::{Found, judge};
use mctas::design::Design;
use std::sync::Mutex;

pub type Pos = [i32; 3];

const N6: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn add(p: Pos, d: Pos) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

fn is_plain(s: &str) -> bool {
    s == "minecraft:stone"
}

/// Candidate kicks: (cell, temporary block state).
pub fn kick_cells(d: &Design) -> Vec<(Pos, String)> {
    let mut v: Vec<(Pos, String)> = Vec::new();
    let free = |q: &Pos| !d.blocks.contains_key(q);
    for (p, s) in &d.blocks {
        if s.contains("redstone_wire") {
            for n in N6 {
                let q = add(*p, n);
                if free(&q) {
                    v.push((q, "minecraft:redstone_block".into()));
                }
            }
        }
        if is_plain(s) {
            // a torch under this block powers it strongly
            let q = add(*p, [0, -1, 0]);
            if !free(&q) {
                continue;
            }
            for (f, dl) in [("north", [0, 0, 1]), ("south", [0, 0, -1]), ("east", [-1, 0, 0]), ("west", [1, 0, 0])] {
                // a wall torch facing f hangs on the block at q - dir(f)
                let wall = add(q, dl);
                if d.blocks.get(&wall).is_some_and(|w| is_plain(w)) {
                    v.push((q, format!("minecraft:redstone_wall_torch[facing={f},lit=true]")));
                }
            }
            if d.blocks.get(&add(q, [0, -1, 0])).is_some_and(|w| is_plain(w)) {
                v.push((q, "minecraft:redstone_torch[lit=true]".into()));
            }
        }
    }
    v.sort();
    v.dedup();
    v
}

pub fn with_kick(d: &Design, cell: Pos, state: &str, len: u32, at: u32) -> Design {
    let mut e = d.clone();
    e.ports.insert("start".into(), cell);
    e.meta.insert("port start".into(), if at > 0 { format!("{state} {len} at {at}") } else { format!("{state} {len}") });
    e
}

/// Judge every kick variant; returns (cell, state, len, at, found).
pub fn search(d: &Design, cells: &[(Pos, String)], lens: &[u32], ats: &[u32], horizon: u32, threads: usize) -> Vec<(Pos, String, u32, u32, Found)> {
    let mut jobs: Vec<(Pos, String, u32, u32, String)> = Vec::new();
    for (c, st) in cells {
        for l in lens {
            for a in ats {
                jobs.push((*c, st.clone(), *l, *a, with_kick(d, *c, st, *l, *a).to_text()));
            }
        }
    }
    let out = Mutex::new(Vec::new());
    let next = Mutex::new(0usize);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        let i = *n;
                        *n += 1;
                        i
                    };
                    if i >= jobs.len() {
                        break;
                    }
                    let (c, st, l, a, text) = &jobs[i];
                    if let Some(f) = judge(&d.name, text, horizon) {
                        out.lock().unwrap().push((*c, st.clone(), *l, *a, f));
                    }
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by(|x, y| (x.4.period, x.3, x.2, x.0, &x.1).cmp(&(y.4.period, y.3, y.2, y.0, &y.1)));
    v
}
