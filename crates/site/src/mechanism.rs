//! A clock's mechanism (the coordinator 2026-10-21 11:33Z, vjeux on the Clocks tab at 19 ticks:
//! "The first 10 clocks look all the same to me?": 43 census clocks there are one observer and
//! sticky-piston loop with a filler block swapped): its design with the blocks a builder swaps for
//! one another normalized, so the clocks of one period that differ only by such a swap fold into
//! one card, every period alike (the Clocks tab and the time search), the others listed on its
//! page ("also works with: note block, redstone lamp...").
//!
//! The key: every block of the design as its class (a redstone component keeps its id and its
//! orientation: an observer facing up, dust, a repeater facing west; every other block is one
//! class, a block: a note block, a lamp, a copper bulb, a redstone block, a piston, stone...; an
//! observer that powers nothing, its output toward an empty cell, is a block too), its entities by
//! id, in the smallest of its 48 orientations (the cube's 24 turns, mirrored or not), from its
//! box's corner. Dynamic state (powered, lit, extended) and settings (a repeater's delay, a
//! comparator's mode) are no part of it.

use site_model::Build;
use std::collections::{BTreeMap, BTreeSet};

/// Blocks that keep their identity in a mechanism (redstone components, blocks that move or carry
/// a structure, containers a comparator reads, rails, fluids); every other block is a block.
fn kept(id: &str) -> bool {
    const EXACT: &[&str] = &[
        "observer", "redstone_wire", "repeater", "comparator", "redstone_torch", "redstone_wall_torch", "lever", "hopper", "dropper", "dispenser", "crafter", "daylight_detector",
        "sculk_sensor", "calibrated_sculk_sensor", "sculk_shrieker", "tripwire_hook", "tripwire", "tnt", "slime_block", "honey_block", "lectern", "chest", "trapped_chest", "barrel",
        "composter", "cauldron", "water_cauldron", "lava_cauldron", "jukebox", "bell", "lightning_rod", "water", "lava", "bubble_column", "scaffolding", "target", "piston_head",
        "moving_piston", "furnace", "blast_furnace", "smoker", "brewing_stand", "beehive", "bee_nest", "shulker_box", "chiseled_bookshelf", "decorated_pot", "redstone_ore",
    ];
    EXACT.contains(&id) || id.ends_with("_button") || id.ends_with("_pressure_plate") || id.ends_with("rail") || id.ends_with("_shulker_box") || id.ends_with("_trapdoor") || id.ends_with("_door") || id.ends_with("fence_gate")
}

/// The states that orient a block (kept in its class; the rest is dynamic or a setting).
const ORIENT: &[&str] = &["facing", "face", "axis", "half", "type", "hinge", "attachment"];

/// A block's class: (id, orientation states), or `block` for a swappable one. A piston is a
/// swappable block too: a plain piston in the cell an observer powers works like the note block
/// another census clock has there (clk-obs-p19-lamp-note-obs-pis-rb-stk-7b is the note-block one
/// with a plain piston below its observer, mirrored); the clocks it folds are of one period, with
/// the same cells and the same redstone components (observers, dust, repeaters...) in them.
fn class(state: &str) -> (String, Vec<(String, String)>) {
    let s = state.trim_start_matches("minecraft:");
    let (id, props) = match s.split_once('[') {
        Some((id, rest)) => (id, rest.trim_end_matches(']')),
        None => (s, ""),
    };
    let id = id.trim();
    if !kept(id) {
        return ("block".into(), Vec::new());
    }
    let orient: Vec<(String, String)> = props
        .split(',')
        .filter_map(|kv| kv.split_once('='))
        // (a piston head's `type` is sticky or normal: a piston is a piston)
        .filter(|(k, _)| ORIENT.contains(&k.trim()) && !(k.trim() == "type" && (id == "piston_head" || id == "moving_piston")))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    (id.to_string(), orient)
}

/// The 48 orientations of a design (the 24 turns of a cube, mirrored or not): signed permutations
/// of the axes, as (axis of x', sign), (axis of y', sign), (axis of z', sign). A clock that works
/// turned on its side or mirrored is the same mechanism (an observer loop works in any
/// orientation; a clock that needs its floor has no working turned copy to fold with).
fn orientations() -> Vec<[(usize, i32); 3]> {
    let mut v = Vec::new();
    for p in [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
        for s in 0..8 {
            let sign = |k: usize| if s >> k & 1 == 1 { -1 } else { 1 };
            v.push([(p[0], sign(0)), (p[1], sign(1)), (p[2], sign(2))]);
        }
    }
    v
}

fn apply(o: &[(usize, i32); 3], p: [i32; 3]) -> [i32; 3] {
    [o[0].1 * p[o[0].0], o[1].1 * p[o[1].0], o[2].1 * p[o[2].0]]
}

/// A direction's unit vector (`north` = -z) and back.
fn dir_vec(d: &str) -> Option<[i32; 3]> {
    Some(match d {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}

fn vec_dir(v: [i32; 3]) -> &'static str {
    match v {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        _ => "down",
    }
}

fn orient_state(o: &[(usize, i32); 3], k: &str, v: &str) -> String {
    match k {
        "facing" => dir_vec(v).map(|d| vec_dir(apply(o, d)).to_string()).unwrap_or_else(|| v.to_string()),
        "axis" => {
            let d = match v {
                "x" => [1, 0, 0],
                "y" => [0, 1, 0],
                _ => [0, 0, 1],
            };
            let r = apply(o, d);
            (if r[0] != 0 { "x" } else if r[1] != 0 { "y" } else { "z" }).to_string()
        }
        _ => v.to_string(),
    }
}

/// The mechanism key of a design's text, as written (None: it does not parse).
pub fn key(text: &str) -> Option<String> {
    let d = mctas::design::Design::parse(text).ok()?;
    let cells: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
    let entities: Vec<String> = d.entities.iter().map(|e| e.kind.clone()).collect();
    Some(key_of_cells(&cells, &entities))
}

/// The mechanism keys of a clock: its design as written (a block swapped for another) and at its
/// thumbnail's still frame (the picture its card shows: two designs of one loop started at another
/// moment of it show the same one).
pub fn keys_of(still: &crate::simulate::Still, text: &str) -> Vec<String> {
    let mut v: Vec<String> = key(text).into_iter().collect();
    if !still.cells.is_empty() {
        let entities: Vec<String> = mctas::design::Design::parse(text).map(|d| d.entities.iter().map(|e| e.kind.clone()).collect()).unwrap_or_default();
        v.push(format!("still:{}", key_of_cells(&still.cells, &entities)));
    }
    v
}

/// The mechanism key of some blocks (cell, state) and entity ids.
pub fn key_of_cells(blocks: &[([i32; 3], String)], entity_ids: &[String]) -> String {
    let filled: BTreeSet<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();
    let mut cells: Vec<([i32; 3], (String, Vec<(String, String)>))> = blocks.iter().map(|(p, s)| (*p, class(s))).collect();
    // an observer that powers nothing (its output face toward an empty cell) is a block there,
    // like the note block another census clock has in that cell
    for (p, (id, orient)) in cells.iter_mut() {
        if id == "observer"
            && let Some(f) = orient.iter().find(|(k, _)| k == "facing").and_then(|(_, v)| dir_vec(v))
            && !filled.contains(&[p[0] - f[0], p[1] - f[1], p[2] - f[2]])
        {
            *id = "block".into();
            orient.clear();
        }
    }
    let mut entities: Vec<String> = entity_ids.iter().map(|e| e.trim_start_matches("minecraft:").to_string()).collect();
    entities.sort();
    let mut best: Option<String> = None;
    for o in orientations() {
        let placed: Vec<([i32; 3], String)> = cells
            .iter()
            .map(|(p, (id, orient))| {
                let s: Vec<String> = orient.iter().map(|(k, v)| format!("{k}={}", orient_state(&o, k, v))).collect();
                (apply(&o, *p), if s.is_empty() { id.clone() } else { format!("{id}[{}]", s.join(",")) })
            })
            .collect();
        let lo = [0, 1, 2].map(|k| placed.iter().map(|(p, _)| p[k]).min().unwrap_or(0));
        let mut v: Vec<String> = placed.iter().map(|(p, c)| format!("{},{},{}={c}", p[0] - lo[0], p[1] - lo[1], p[2] - lo[2])).collect();
        v.sort();
        let s = format!("{}|{}", v.join(";"), entities.join(","));
        if best.as_ref().is_none_or(|b| s < *b) {
            best = Some(s);
        }
    }
    best.unwrap_or_default()
}

/// A clock that never folds under another: a family's base design or an extended clock (a builder
/// picks it for its sizes).
fn keeps_card(b: &Build) -> bool {
    b.clock.as_ref().is_some_and(|c| c.family.is_some() || c.card.is_some() || c.n.is_some()) || b.flags.iter().any(|f| f == "extended" || f == "extendable")
}

/// The folds of a set of clock builds: per period (as the Clocks tab groups them), the clocks that
/// share a mechanism key (`keys[i]`: build i's keys, its design as written and at its still frame;
/// empty: not a clock) are one mechanism, transitively; of each, the smallest (blocks, then box
/// volume, then name; a family's clock first) keeps the card and every other one that is no
/// family's folds under it. Returns (folded, host) pairs.
pub fn folds(builds: &[Build], keys: &[Vec<String>]) -> Vec<(usize, usize)> {
    // union-find over the builds sharing a (period, key)
    let mut parent: Vec<usize> = (0..builds.len()).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while parent[r] != r {
            r = parent[r];
        }
        let mut j = i;
        while parent[j] != r {
            let n = parent[j];
            parent[j] = r;
            j = n;
        }
        r
    }
    let mut first: BTreeMap<(u32, &String), usize> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(c) = &b.clock else { continue };
        for k in keys.get(i).into_iter().flatten() {
            match first.get(&(c.every(), k)) {
                Some(&j) => {
                    let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                    parent[a] = b;
                }
                None => {
                    first.insert((c.every(), k), i);
                }
            }
        }
    }
    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..builds.len() {
        if builds[i].clock.is_some() && keys.get(i).is_some_and(|k| !k.is_empty()) {
            let r = find(&mut parent, i);
            groups.entry(r).or_default().push(i);
        }
    }
    let mut out = Vec::new();
    for mut g in groups.into_values().filter(|g| g.len() > 1) {
        g.sort_by_key(|&i| (!keeps_card(&builds[i]), builds[i].blocks, builds[i].volume.unwrap_or(u32::MAX), builds[i].name.clone()));
        let host = g[0];
        out.extend(g[1..].iter().copied().filter(|&i| !keeps_card(&builds[i])).map(|i| (i, host)));
    }
    out
}

/// Fold the clocks of one mechanism and period under their card (`folds`): the folded ones keep
/// their pages (their `host` set: off the Clocks tab and the time search, like any variant), the
/// card lists them (`variants`) and the blocks they swap in (`swaps`). Returns how many folded.
pub fn fold(builds: &mut [Build], keys: &[Vec<String>]) -> usize {
    // (the catalog's older variant lists, the dominance fold every clock left when each became a
    // card of its own: replaced by the mechanisms)
    for b in builds.iter_mut() {
        if let Some(c) = b.clock.as_mut() {
            c.variants.clear();
            c.swaps.clear();
        }
    }
    let pairs = folds(builds, keys);
    let mut by_host: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &(v, h) in &pairs {
        by_host.entry(h).or_default().push(v);
    }
    for (h, vs) in by_host {
        let host_id = builds[h].id.clone();
        let host_kinds: BTreeSet<String> = builds[h].kinds.iter().cloned().collect();
        let mut swaps: BTreeSet<String> = BTreeSet::new();
        let mut ids = Vec::new();
        for &v in &vs {
            swaps.extend(builds[v].kinds.iter().filter(|k| !host_kinds.contains(*k)).cloned());
            ids.push(builds[v].id.clone());
            if let Some(c) = builds[v].clock.as_mut() {
                c.host = Some(host_id.clone());
            }
        }
        if let Some(c) = builds[h].clock.as_mut() {
            c.variants = ids;
            c.swaps = swaps.into_iter().collect();
        }
    }
    pairs.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAMP: &str = "name: a\nperiod: 19\norigin: 0 0 0\npalette:\n  A minecraft:redstone_block\n  B minecraft:sticky_piston[extended=false,facing=north]\n  D minecraft:redstone_lamp[lit=false]\n  E minecraft:observer[facing=south,powered=false]\nlayer 0:\n  AD\n  BE\n";

    #[test]
    fn a_filler_swap_is_one_mechanism_a_new_part_is_not() {
        let note = LAMP.replace("minecraft:redstone_lamp[lit=false]", "minecraft:note_block[instrument=harp,note=0,powered=false]");
        let plain = LAMP.replace("minecraft:sticky_piston", "minecraft:piston");
        assert_eq!(key(LAMP), key(&note), "a lamp or a note block: one mechanism");
        assert_eq!(key(LAMP), key(&plain), "a sticky or a plain piston: one mechanism");
        // another facing, or dust in place of the block, is another mechanism
        let turned = LAMP.replace("observer[facing=south", "observer[facing=north");
        let dust = LAMP.replace("minecraft:redstone_lamp[lit=false]", "minecraft:redstone_wire");
        assert_ne!(key(LAMP), key(&turned));
        assert_ne!(key(LAMP), key(&dust));
    }

    #[test]
    fn a_turned_or_mirrored_copy_is_the_same_mechanism() {
        // the same loop turned a quarter: rows become columns, every facing turns
        let turned = "name: b\nperiod: 19\norigin: 0 0 0\npalette:\n  A minecraft:redstone_block\n  B minecraft:sticky_piston[extended=false,facing=east]\n  D minecraft:redstone_lamp[lit=false]\n  E minecraft:observer[facing=west,powered=false]\nlayer 0:\n  BA\n  ED\n";
        assert_eq!(key(LAMP), key(turned));
    }
}
