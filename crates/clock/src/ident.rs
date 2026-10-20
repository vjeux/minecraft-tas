//! Type identity (vjeux 2026-10-19: "census-p6-observer-piston-3b / census-p6-observer-sticky_piston-3b:
//! this is the same twice"): two clocks are ONE type when swapping a block for a functionally
//! equivalent one gives the identical run. The type key ([`type_key`]) is computed on the design
//! with every swap that keeps the run identical on redsim (the same block states, comparator
//! strengths and container contents at every tick, modulo the swap):
//!
//! * sticky pistons -> pistons (the clock never pulls a block back);
//! * inert full solid blocks (stone, smooth stone, cobblestone, wool, planks, metal blocks, clay,
//!   packed ice, obsidian...) -> stone (a support or conductor whose kind changes nothing: a note
//!   block's instrument or obsidian's immovability shows in the run, so such swaps are refused).
//!
//! Designs redsim does not model (sculk, jukeboxes, entities...) keep their own blocks. The
//! archive keeps a cache of the keys (`archive/type-keys.tsv`: file, text hash, key).

use crate::sim::{SimOptions, Verdict, run_states, simulate};
use crate::{Clock, canon};
use std::collections::BTreeMap;
use std::path::Path;

/// Inert full solids: opaque conductive full cubes with no state (redsim's simple blocks).
const SOLIDS: [&str; 14] = [
    "minecraft:stone",
    "minecraft:smooth_stone",
    "minecraft:cobblestone",
    "minecraft:obsidian",
    "minecraft:white_wool",
    "minecraft:oak_planks",
    "minecraft:gold_block",
    "minecraft:iron_block",
    "minecraft:clay",
    "minecraft:emerald_block",
    "minecraft:packed_ice",
    "minecraft:diamond_block",
    "minecraft:stone_bricks",
    "minecraft:deepslate",
];

fn id(s: &str) -> &str {
    s.split('[').next().unwrap_or(s)
}

/// A state with the sticky-piston swap applied (sticky_piston -> piston, a sticky head or moving
/// block -> normal).
pub fn unstick(s: &str) -> String {
    if id(s) == "minecraft:sticky_piston" {
        return s.replacen("minecraft:sticky_piston", "minecraft:piston", 1);
    }
    if id(s) == "minecraft:piston_head" || id(s) == "minecraft:moving_piston" {
        return s.replace("type=sticky", "type=normal");
    }
    s.to_string()
}

/// A state with the solid swap applied (any inert full solid -> stone).
pub fn unsolid(s: &str) -> String {
    if SOLIDS.contains(&id(s)) { "minecraft:stone".to_string() } else { s.to_string() }
}

/// The design with `f` applied to every block (`None` when nothing changes).
fn swapped(c: &Clock, f: &dyn Fn(&str) -> String) -> Option<Clock> {
    let mut d = c.design.clone();
    let mut changed = false;
    for s in d.blocks.values_mut() {
        let n = f(s);
        if n != *s {
            *s = n;
            changed = true;
        }
    }
    if !changed {
        return None;
    }
    Clock::from_design(d).ok()
}

/// Does `b` run exactly like `a` once both runs are mapped through `f`?
fn same_run(a: &Clock, b: &Clock, ticks: u32, f: &dyn Fn(&str) -> String) -> bool {
    let o = SimOptions::default();
    let (Some(ra), Some(rb)) = (run_states(a, &o, ticks), run_states(b, &o, ticks)) else {
        return false;
    };
    ra.len() == rb.len() && ra.iter().zip(&rb).all(|(x, y)| x.len() == y.len() && x.iter().zip(y).all(|((p, s), (q, t))| p == q && f(s) == f(t)))
}

/// The design the type key is computed on: every swap that keeps the run identical applied (the
/// clock itself when redsim does not model it).
pub fn normalized(c: &Clock) -> Clock {
    let r = simulate(c, &SimOptions::default());
    if r.verdict != Verdict::Clock {
        return c.clone();
    }
    // the warmup and two whole-state periods (capped: a long divider's first ticks decide)
    let ticks = (r.state_warmup.max(r.warmup) + 2 * r.state_period).min(6000);
    let mut cur = c.clone();
    let swaps: [&dyn Fn(&str) -> String; 2] = [&unstick, &unsolid];
    for f in swaps {
        if let Some(s) = swapped(&cur, f) {
            if same_run(&cur, &s, ticks, f) {
                cur = s;
            }
        }
    }
    cur
}

/// The clock's type key: the canonical key ([`canon::design_type_key`]: rotations, mirror image,
/// nbt) of its [`normalized`] design.
pub fn type_key(c: &Clock) -> String {
    canon::design_type_key(&normalized(c).design)
}

pub const TYPE_KEYS: &str = "type-keys.tsv";

fn hash(text: &str) -> String {
    let h = text.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    format!("{h:016x}")
}

/// The type key of every given archived file (cached in `archive/type-keys.tsv` by the file's
/// text hash; missing keys computed in parallel). A file that does not load keeps its fallback.
pub fn archive_keys(dir: &Path, files: &[(String, String)]) -> BTreeMap<String, String> {
    let cache_path = dir.join(TYPE_KEYS);
    let mut cache: BTreeMap<String, (String, String)> = BTreeMap::new();
    for l in std::fs::read_to_string(&cache_path).unwrap_or_default().lines().skip(1) {
        let f: Vec<&str> = l.splitn(3, '\t').collect();
        if f.len() == 3 {
            cache.insert(f[0].to_string(), (f[1].to_string(), f[2].to_string()));
        }
    }
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    let mut todo: Vec<(String, String, String)> = Vec::new();
    for (file, fallback) in files {
        let text = std::fs::read_to_string(dir.join(file)).unwrap_or_default();
        let h = hash(&text);
        match cache.get(file) {
            Some((ch, key)) if *ch == h => {
                out.insert(file.clone(), key.clone());
            }
            _ => todo.push((file.clone(), h, fallback.clone())),
        }
    }
    if !todo.is_empty() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).clamp(1, 24);
        let done: Vec<(String, String, String)> = std::thread::scope(|s| {
            let parts: Vec<&[(String, String, String)]> = todo.chunks(todo.len().div_ceil(threads)).collect();
            let hs: Vec<_> = parts
                .into_iter()
                .map(|part| {
                    s.spawn(move || {
                        part.iter()
                            .map(|(file, h, fallback)| {
                                let key = Clock::load(&dir.join(file)).map(|c| type_key(&c)).unwrap_or_else(|_| fallback.clone());
                                (file.clone(), h.clone(), key)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
        });
        for (file, h, key) in done {
            cache.insert(file.clone(), (h, key.clone()));
            out.insert(file, key);
        }
        cache.retain(|f, _| files.iter().any(|(x, _)| x == f));
        let mut s = String::from("file\thash\ttype\n");
        for (f, (h, k)) in &cache {
            s.push_str(&format!("{f}\t{h}\t{k}\n"));
        }
        let _ = std::fs::write(&cache_path, s);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(text: &str) -> Clock {
        Clock::from_design(mctas::design::Design::parse(text).unwrap()).unwrap()
    }

    /// A stone support swapped for wool gives the same run: one type; the observer pair alone has
    /// nothing to swap.
    #[test]
    fn solid_swap() {
        let obs_pair = clock("name: a\nport start: 1 0 0 last\npalette:\n  E minecraft:observer[facing=east]\n  W minecraft:observer[facing=west]\nlayer 0:\n  EW\n");
        assert_eq!(normalized(&obs_pair).design.blocks, obs_pair.design.blocks);
        let on_stone = clock("name: b\nport start: 1 1 0 last\npalette:\n  S minecraft:stone\n  E minecraft:observer[facing=east]\n  W minecraft:observer[facing=west]\nlayer 0:\n  SS\nlayer 1:\n  EW\n");
        let on_wool = clock("name: c\nport start: 1 1 0 last\npalette:\n  S minecraft:white_wool\n  E minecraft:observer[facing=east]\n  W minecraft:observer[facing=west]\nlayer 0:\n  SS\nlayer 1:\n  EW\n");
        assert_eq!(type_key(&on_stone), type_key(&on_wool));
    }
}
