//! A clock's mechanism (the coordinator 2026-10-21 11:33Z, vjeux on the Clocks tab at 19 ticks:
//! "The first 10 clocks look all the same to me?": 43 census clocks there are one observer and
//! sticky-piston loop with a filler block swapped): its design with the blocks a builder swaps for
//! one another normalized, so the clocks of one period that differ only by such a swap fold into
//! one card, every period alike (the Clocks tab and the time search), the others listed on its
//! page ("also works with: note block, redstone lamp...").
//!
//! The key: every block of the design as its class (a redstone component keeps its id and its
//! orientation: an observer facing up, dust, a repeater facing west; every other block is one
//! class, a block: a note block, a lamp, a copper bulb, a redstone block, a piston, stone, and
//! every toggled component, a trapdoor, a door, a fence gate, a powered or activator rail, a
//! crafter, a bell, whatever its wood or metal (a door one block: its upper half has no cell);
//! an observer that powers nothing, its output toward an empty cell, is a block too), its entities
//! by id, in the smallest of its 48 orientations (the cube's 24 turns, mirrored or not), from its
//! box's corner. Dynamic state (powered, lit, extended) and settings (a repeater's delay, a
//! comparator's mode) are no part of it. Every clock folds, the families' too (vjeux 13:09Z).

use site_model::Build;
use std::collections::{BTreeMap, BTreeSet};

/// Blocks that keep their identity in a mechanism (redstone components, blocks that move or carry
/// a structure, containers a comparator reads, rails that are no relay, fluids); every other block
/// is a block, the toggled components included (`toggled`).
fn kept(id: &str) -> bool {
    const EXACT: &[&str] = &[
        "observer", "redstone_wire", "repeater", "comparator", "redstone_torch", "redstone_wall_torch", "lever", "hopper", "dropper", "dispenser", "daylight_detector",
        "sculk_sensor", "calibrated_sculk_sensor", "sculk_shrieker", "tripwire_hook", "tripwire", "tnt", "slime_block", "honey_block", "lectern", "chest", "trapped_chest", "barrel",
        "composter", "cauldron", "water_cauldron", "lava_cauldron", "jukebox", "lightning_rod", "water", "lava", "bubble_column", "scaffolding", "target", "piston_head",
        "moving_piston", "furnace", "blast_furnace", "smoker", "brewing_stand", "beehive", "bee_nest", "shulker_box", "chiseled_bookshelf", "decorated_pot", "redstone_ore",
        "rail", "detector_rail",
    ];
    !toggled(id) && (EXACT.contains(&id) || id.ends_with("_button") || id.ends_with("_pressure_plate") || id.ends_with("_shulker_box"))
}

/// A toggled component (vjeux 13:09Z: the observer rings with a fence gate, a trapdoor or a
/// powered rail "are not deduplicated"; "fix the entire class"): a block whose only part in a
/// circuit is to change its state when powered (it opens, lights, tilts, triggers), which an
/// observer watching it sees the same way whatever the block: trapdoors, doors and fence gates
/// of every wood, iron and copper, powered and activator rails, crafters, bells (with the note
/// blocks, lamps, copper bulbs, heads and pistons every clock already swaps). In a mechanism it is
/// a block, a door one block (its upper half is part of it).
fn toggled(id: &str) -> bool {
    id.ends_with("_trapdoor") || id.ends_with("_door") || id.ends_with("_fence_gate") || id == "powered_rail" || id == "activator_rail" || id == "crafter" || id == "bell"
}

/// A door's upper half: no cell of its own in a mechanism (the door is one block, its lower half).
fn upper_half(state: &str) -> bool {
    let s = state.trim_start_matches("minecraft:");
    s.split('[').next().is_some_and(|id| id.ends_with("_door")) && s.contains("half=upper")
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
    Some(key_of_cells(&cells, &entities, &stocked(&d)))
}

/// The cells whose containers the design stocks with items (its `nbt:` lines' `Items`): a
/// dispenser or a dropper holding nothing only clicks when powered (the sculk census clocks'
/// vibration source), a toggled component; stocked, it moves items. Containers never move, so
/// the cells are the same at the still frame.
fn stocked(d: &mctas::design::Design) -> BTreeSet<[i32; 3]> {
    d.block_entities.iter().filter(|(_, nbt)| nbt.contains("Items:[{")).map(|(p, _)| *p).collect()
}

/// The mechanism keys of a clock: its design as written (a block swapped for another) and at its
/// thumbnail's still frame (the picture its card shows: two designs of one loop started at another
/// moment of it show the same one).
pub fn keys_of(still: &crate::simulate::Still, text: &str) -> Vec<String> {
    let mut v: Vec<String> = key(text).into_iter().collect();
    if !still.cells.is_empty() {
        let d = mctas::design::Design::parse(text).ok();
        let entities: Vec<String> = d.as_ref().map(|d| d.entities.iter().map(|e| e.kind.clone()).collect()).unwrap_or_default();
        let stocked = d.as_ref().map(stocked).unwrap_or_default();
        v.push(format!("still:{}", key_of_cells(&still.cells, &entities, &stocked)));
    }
    v
}

/// The mechanism key of some blocks (cell, state) and entity ids; `stocked`: the cells whose
/// containers hold items.
pub fn key_of_cells(blocks: &[([i32; 3], String)], entity_ids: &[String], stocked: &BTreeSet<[i32; 3]>) -> String {
    let blocks: Vec<&([i32; 3], String)> = blocks.iter().filter(|(_, s)| !upper_half(s)).collect();
    let filled: BTreeSet<[i32; 3]> = blocks.iter().map(|(p, _)| *p).collect();
    let mut cells: Vec<([i32; 3], (String, Vec<(String, String)>))> = blocks.iter().map(|(p, s)| (*p, class(s))).collect();
    // a dispenser or dropper holding nothing is a toggled component: a block
    for (p, (id, orient)) in cells.iter_mut() {
        if (id == "dispenser" || id == "dropper") && !stocked.contains(p) {
            *id = "block".into();
            orient.clear();
        }
    }
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

/// A clock that carries its family (a family's base design or an extended clock): first among the
/// clocks of one mechanism and size, so the card that stays says every size it comes in.
fn keeps_card(b: &Build) -> bool {
    b.clock.as_ref().is_some_and(|c| c.family.is_some() || c.card.is_some() || c.n.is_some()) || b.flags.iter().any(|f| f == "extended" || f == "extendable")
}

/// The Clocks kind's own card (its `kind_range`: the periods its page covers): never folded.
fn kind_card(b: &Build) -> bool {
    b.clock.as_ref().is_some_and(|c| !c.kind_range.is_empty())
}

/// The folds of a set of clock builds: per period (as the Clocks tab groups them), the clocks that
/// share a mechanism key (`keys[i]`: build i's keys, its design as written and at its still frame;
/// empty: not a clock) are one mechanism, transitively; of each, the smallest (blocks, then box
/// volume; a family's clock first among equals, then by name) keeps the card and every other one
/// folds under it, the families' clocks too (vjeux 13:09Z: eleven observer rings that only swap
/// their relay block were eleven cards a period). Returns (folded, host) pairs.
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
        g.sort_by_key(|&i| (!kind_card(&builds[i]), builds[i].blocks, builds[i].volume.unwrap_or(u32::MAX), !keeps_card(&builds[i]), builds[i].name.clone()));
        let host = g[0];
        out.extend(g[1..].iter().copied().filter(|&i| !kind_card(&builds[i])).map(|i| (i, host)));
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

/// The doors' cards (vjeux 13:10Z, "What about doors?"): two board leaders that are one door or
/// one mechanism with a swapped block (`keys[i]`: build i's mechanism keys, its design as written
/// and at its still frame; empty: no door card) and the same numbers (category, open and close
/// ticks: the boards they lead are then the same records) are one card: the smallest (blocks,
/// then box volume, then name) keeps it and leads every board of them; the other keeps its page,
/// listed on the card's page (its `host`). Returns one note per fold.
pub fn fold_doors(builds: &mut [Build], keys: &[Vec<String>]) -> Vec<String> {
    let mut groups: BTreeMap<(String, Option<u32>, Option<u32>, &String), Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(d) = &b.door else { continue };
        if d.leads.is_empty() || d.host.is_some() {
            continue;
        }
        for k in keys.get(i).into_iter().flatten() {
            groups.entry((b.group.clone(), d.open, d.close, k)).or_default().push(i);
        }
    }
    // union the groups sharing a door (one door may share each of its keys with another)
    let mut parent: Vec<usize> = (0..builds.len()).collect();
    fn find(parent: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while parent[r] != r {
            r = parent[r];
        }
        parent[i] = r;
        r
    }
    for g in groups.values() {
        for w in g.windows(2) {
            let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            parent[a] = b;
        }
    }
    let mut sets: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for g in groups.values() {
        for &i in g {
            let r = find(&mut parent, i);
            let set = sets.entry(r).or_default();
            if !set.contains(&i) {
                set.push(i);
            }
        }
    }
    let mut notes = Vec::new();
    for mut set in sets.into_values().filter(|s| s.len() > 1) {
        set.sort_by_key(|&i| (builds[i].blocks, builds[i].volume.unwrap_or(u32::MAX), builds[i].name.clone()));
        let host = set[0];
        let host_id = builds[host].id.clone();
        for &i in &set[1..] {
            let leads = builds[i].door.as_mut().map(|d| std::mem::take(&mut d.leads)).unwrap_or_default();
            if let Some(d) = builds[i].door.as_mut() {
                d.host = Some(host_id.clone());
            }
            let id = builds[i].id.clone();
            notes.push(format!("door {} folds under {} (one mechanism, the same numbers): its boards {} move to it", builds[i].name, builds[host].name, leads.join(", ")));
            if let Some(d) = builds[host].door.as_mut() {
                for l in leads {
                    if !d.leads.contains(&l) {
                        d.leads.push(l);
                    }
                }
                d.variants.push(id);
            }
        }
    }
    notes
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

    /// clk-misc's observer ring (`mctas clock misc ring --relay R`): the relay block one observer
    /// powers and the next watches.
    fn ring(relay: &str) -> String {
        format!("name: r\norigin: 0 0 0\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  B minecraft:observer[facing=up,powered=false]\n  C minecraft:observer[facing=west,powered=false]\n  O {relay}\n  o minecraft:observer[facing=down,powered=false]\nlayer 0:\n  AB\nlayer 1:\n  oB\nlayer 2:\n  OC\n")
    }

    #[test]
    fn every_toggled_relay_is_one_mechanism() {
        // (vjeux 13:09Z: the fence gate, trapdoor and powered rail rings "are not deduplicated";
        // "fix the entire class")
        let gate = key(&ring("minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]"));
        for relay in [
            "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]",
            "minecraft:iron_trapdoor[facing=east,half=top,open=false,powered=false,waterlogged=false]",
            "minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]",
            "minecraft:activator_rail[powered=false,shape=north_south,waterlogged=false]",
            "minecraft:crafter[crafting=false,orientation=north_up,triggered=false]",
            "minecraft:note_block[instrument=harp,note=0,powered=false]",
            "minecraft:copper_bulb[lit=false,powered=false]",
            "minecraft:skeleton_skull[powered=false,rotation=0]",
            "minecraft:bell[attachment=floor,facing=north,powered=false]",
        ] {
            assert_eq!(key(&ring(relay)), gate, "{relay}");
        }
        // a door is one block: its upper half above the relay is part of it
        let door = ring("minecraft:iron_door[facing=north,half=lower,hinge=left,open=false,powered=false]").replace("layer 2:\n  OC\n", "layer 2:\n  OC\nlayer 3:\n  U\n").replace("  o minecraft", "  U minecraft:iron_door[facing=north,half=upper,hinge=left,open=false,powered=false]\n  o minecraft");
        assert_eq!(key(&door), gate, "a door relay");
        // a redstone component in the relay's cell is another mechanism
        assert_ne!(key(&ring("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]")), gate);
        assert_ne!(key(&ring("minecraft:hopper[enabled=true,facing=down]")), gate);
        // a dispenser holding nothing only clicks: a relay like the others; stocked, it is not
        let empty = ring("minecraft:dispenser[facing=north,triggered=false]");
        assert_eq!(key(&empty), gate, "an empty dispenser");
        let stocked = format!("{empty}nbt: 0 2 0 {{Items:[{{Slot:0b,id:\"minecraft:arrow\",count:1}}]}}\n");
        assert_ne!(key(&stocked), gate, "a stocked dispenser");
    }

    #[test]
    fn a_family_clock_folds_too_under_the_smallest() {
        let b = |name: &str, blocks: u32, family: bool| Build {
            name: name.into(),
            id: format!("clocks/{name}"),
            blocks,
            volume: Some(8),
            clock: Some(site_model::ClockInfo { period: 14, family: family.then(|| name.to_string()), n: family.then_some(1), ..Default::default() }),
            ..Default::default()
        };
        let builds = vec![b("z-gate-ring-n1", 8, true), b("a-door-ring-n1", 9, true), b("census-6b", 8, false), b("m-trap-ring-n1", 8, true)];
        let keys = vec![vec!["k".to_string()]; 4];
        let mut pairs = folds(&builds, &keys);
        pairs.sort();
        // the smallest (8 blocks) keeps the card, a family's clock first among equals, then by
        // name: m-trap-ring; every other one folds, the families' clocks too
        assert_eq!(pairs, vec![(0, 3), (1, 3), (2, 3)]);
    }
}
