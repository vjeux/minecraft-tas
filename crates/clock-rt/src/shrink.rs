//! Local search for smaller clocks (`mctas clock rt shrink`): from a seed design, try every
//! single-cell change (remove a block, or put any palette state in a cell of the seed's box grown
//! by one) and every "remove two, add one" move near each other, judge each with `clock sim`, and
//! keep the best design per period (fewest blocks, then smallest swept volume). Rounds repeat from
//! the best design of the target period; `plateau` rounds accept an equal-size change (a walk
//! across designs of the same block count) when nothing is smaller.

use crate::sweep::{Found, judge};
use mctas::design::Design;
use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;

pub type Pos = [i32; 3];

/// The redstone-tick palette: every state a cell may take.
pub fn palette_rt() -> Vec<String> {
    let mut v = vec!["minecraft:stone".to_string(), "minecraft:redstone_block".to_string(), "minecraft:redstone_wire".to_string()];
    for f in ["north", "south", "east", "west"] {
        for d in 1..=4 {
            for p in [false, true] {
                v.push(format!("minecraft:repeater[delay={d},facing={f},locked=false,powered={p}]"));
            }
        }
        for m in ["compare", "subtract"] {
            v.push(format!("minecraft:comparator[facing={f},mode={m},powered=false]"));
        }
        for lit in [true, false] {
            v.push(format!("minecraft:redstone_wall_torch[facing={f},lit={lit}]"));
        }
    }
    for lit in [true, false] {
        v.push(format!("minecraft:redstone_torch[lit={lit}]"));
    }
    v
}

fn bounds(d: &Design) -> (Pos, Pos) {
    let (lo, hi) = d.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
    (lo, hi)
}

fn with(d: &Design, changes: &[(Pos, Option<&str>)]) -> Design {
    let mut e = d.clone();
    for (p, s) in changes {
        match s {
            Some(s) => {
                e.blocks.insert(*p, s.to_string());
            }
            None => {
                e.blocks.remove(p);
            }
        }
    }
    // a port on a cell that left the design moves nowhere: drop the output port then
    if let Some(o) = e.ports.get("output").copied()
        && !e.blocks.contains_key(&o)
    {
        e.ports.remove("output");
    }
    e
}

/// Every neighbor of `d` (single-cell changes in the box grown by `grow`, and remove-two-add-one
/// moves within distance 2).
pub fn neighbors(d: &Design, palette: &[String], grow: i32, pairs: bool) -> Vec<Design> {
    let (lo, hi) = bounds(d);
    let mut out = Vec::new();
    let cells: Vec<Pos> = (lo[1] - grow..=hi[1] + grow)
        .flat_map(|y| (lo[2] - grow..=hi[2] + grow).flat_map(move |z| (lo[0] - grow..=hi[0] + grow).map(move |x| [x, y, z])))
        .collect();
    let filled: Vec<Pos> = d.blocks.keys().copied().collect();
    for c in &cells {
        let cur = d.blocks.get(c);
        if cur.is_some() {
            out.push(with(d, &[(*c, None)]));
        }
        for s in palette {
            if cur.map(|x| x == s).unwrap_or(false) {
                continue;
            }
            out.push(with(d, &[(*c, Some(s))]));
        }
    }
    if pairs {
        for (i, a) in filled.iter().enumerate() {
            for b in &filled[i + 1..] {
                let dist = (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs();
                if dist > 2 {
                    continue;
                }
                // remove a and b, put one palette state in a cell next to either (or in a / b)
                let mut spots: HashSet<Pos> = HashSet::new();
                for p in [a, b] {
                    for dlt in [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                        spots.insert([p[0] + dlt[0], p[1] + dlt[1], p[2] + dlt[2]]);
                    }
                }
                for sp in spots {
                    if sp != *a && sp != *b && d.blocks.contains_key(&sp) {
                        continue;
                    }
                    for s in palette {
                        out.push(with(d, &[(*a, None), (*b, None), (sp, Some(s))]));
                    }
                }
            }
        }
    }
    out
}

pub struct Shrink {
    /// Best per period (fewest blocks, then volume).
    pub best: BTreeMap<u32, Found>,
}

fn better(f: &Found, old: Option<&Found>) -> bool {
    match old {
        None => true,
        Some(o) => (f.blocks, f.volume) < (o.blocks, o.volume),
    }
}

/// Judge designs in parallel; returns the clocks.
fn judge_all(ds: &[Design], horizon: u32, threads: usize) -> Vec<Found> {
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
                    if i >= ds.len() {
                        break;
                    }
                    let text = ds[i].to_text();
                    if let Some(f) = judge(&ds[i].name, &text, horizon) {
                        out.lock().unwrap().push(f);
                    }
                }
            });
        }
    });
    out.into_inner().unwrap()
}

/// Run the local search. `target`: the period to shrink (None = the seed's).
#[allow(clippy::too_many_arguments)]
pub fn shrink(seed: &Design, target: Option<u32>, palette: &[String], rounds: usize, plateau: usize, pairs: bool, horizon: u32, threads: usize, log: bool) -> Shrink {
    let mut best: BTreeMap<u32, Found> = BTreeMap::new();
    let Some(f0) = judge(&seed.name, &seed.to_text(), horizon) else {
        eprintln!("the seed is not a clock");
        return Shrink { best };
    };
    let target = target.unwrap_or(f0.period);
    best.insert(f0.period, f0.clone());
    let mut cur = seed.clone();
    let mut seen: HashSet<String> = HashSet::new();
    seen.insert(seed.content_key());
    let mut flat = 0usize;
    for round in 0..rounds {
        let ns: Vec<Design> = neighbors(&cur, palette, 1, pairs).into_iter().filter(|d| !d.blocks.is_empty() && seen.insert(d.content_key())).collect();
        let found = judge_all(&ns, horizon, threads);
        let cur_found = judge(&cur.name, &cur.to_text(), horizon);
        let (cb, cv) = cur_found.as_ref().map(|f| (f.blocks, f.volume)).unwrap_or((usize::MAX, i32::MAX));
        let mut step: Option<Found> = None;
        let mut equal: Vec<Found> = Vec::new();
        for f in &found {
            if better(f, best.get(&f.period)) {
                best.insert(f.period, f.clone());
            }
            if f.period == target {
                if (f.blocks, f.volume) < (cb, cv) && step.as_ref().is_none_or(|s| (f.blocks, f.volume) < (s.blocks, s.volume)) {
                    step = Some(f.clone());
                } else if f.blocks == cb {
                    equal.push(f.clone());
                }
            }
        }
        if log {
            eprintln!(
                "round {round}: {} neighbors, {} clocks, target P={target} at {} blocks{}",
                ns.len(),
                found.len(),
                best.get(&target).map(|f| f.blocks).unwrap_or(0),
                if step.is_some() { " (improved)" } else { "" }
            );
        }
        let next = match step {
            Some(s) => {
                flat = 0;
                s
            }
            None => {
                if flat >= plateau || equal.is_empty() {
                    break;
                }
                flat += 1;
                // a deterministic walk: the equal-size neighbor with the smallest volume, ties by
                // label order of its text
                equal.sort_by(|a, b| (a.volume, &a.design).cmp(&(b.volume, &b.design)));
                equal[(round * 7919) % equal.len()].clone()
            }
        };
        cur = Design::parse(&next.design).unwrap_or(cur);
    }
    Shrink { best }
}
