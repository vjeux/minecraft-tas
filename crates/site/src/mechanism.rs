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
    let d = untapped(text)?;
    let cells: Vec<([i32; 3], String)> = d.blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
    let entities: Vec<String> = d.entities.iter().map(|e| e.kind.clone()).collect();
    Some(key_of_cells(&cells, &entities, &stocked(&d)))
}

/// The cells the usable-output rule added to a clock to give it a tap (webrender's
/// `clock::usable`, each written down in a comment: `# port output moved ... (usable-output rule):
/// the output of a comparator ... added at 0 0 1 (...)`): the comparator or observer tap, the block
/// under a comparator, and the block put under a repeater tap's cell (`... (on a block put under
/// it) ...`). The tap is how a builder takes the signal, not the clock's mechanism: two clocks of
/// one mechanism, one tapped by a comparator and one by a repeater, are one card.
fn tap_cells(text: &str, d: &mctas::design::Design) -> BTreeSet<[i32; 3]> {
    let mut out = BTreeSet::new();
    let below = |p: [i32; 3]| [p[0], p[1] - 1, p[2]];
    let plain_block = |p: [i32; 3]| d.blocks.get(&p).is_some_and(|s| class(s).0 == "block");
    for line in text.lines().filter(|l| l.trim_start().starts_with('#') && l.contains("(usable-output rule)")) {
        let rule = line.split_once("(usable-output rule):").map(|(_, r)| r).unwrap_or("");
        // (only the rule's own words, not its "(was ...)" reason)
        let rule = rule.split(" (was ").next().unwrap_or(rule);
        if let Some(at) = rule.split_once("added at ").map(|(_, r)| r) {
            let n: Vec<i32> = at.split_whitespace().take(3).filter_map(|w| w.trim_end_matches([',', ')']).parse().ok()).collect();
            if let [x, y, z] = n[..] {
                let p = [x, y, z];
                out.insert(p);
                if rule.contains("comparator") && plain_block(below(p)) {
                    out.insert(below(p));
                }
            }
        } else if rule.contains("on a block put under it")
            && let Some(&port) = d.ports.get("output")
            && plain_block(below(port))
        {
            out.insert(below(port));
        }
    }
    out
}

/// A design without the cells its tap added ([`tap_cells`]).
fn untapped(text: &str) -> Option<mctas::design::Design> {
    let mut d = mctas::design::Design::parse(text).ok()?;
    let taps = tap_cells(text, &d);
    d.blocks.retain(|p, _| !taps.contains(p));
    d.block_entities.retain(|p, _| !taps.contains(p));
    Some(d)
}

/// The cells whose containers the design stocks with items (its `nbt:` lines' `Items`): a
/// dispenser or a dropper holding nothing only clicks when powered (the sculk census clocks'
/// vibration source), a toggled component; stocked, it moves items. Containers never move, so
/// the cells are the same at the still frame.
fn stocked(d: &mctas::design::Design) -> BTreeSet<[i32; 3]> {
    d.block_entities.iter().filter(|(_, nbt)| nbt.contains("Items:[{")).map(|(p, _)| *p).collect()
}

/// The mechanism keys of a clock: its design as written (a block swapped for another), at its
/// thumbnail's still frame (the picture its card shows: two designs of one loop started at another
/// moment of it show the same one), and, for an observer ring, its loop whatever its layout
/// ([`ring_key`]).
pub fn keys_of(still: &crate::simulate::Still, text: &str) -> Vec<String> {
    let mut v: Vec<String> = key(text).into_iter().collect();
    let d = untapped(text);
    if !still.cells.is_empty() {
        let entities: Vec<String> = d.as_ref().map(|d| d.entities.iter().map(|e| e.kind.clone()).collect()).unwrap_or_default();
        let stocked = d.as_ref().map(stocked).unwrap_or_default();
        // (the still frame without the tap's cells either)
        let taps = mctas::design::Design::parse(text).map(|full| tap_cells(text, &full)).unwrap_or_default();
        let cells: Vec<([i32; 3], String)> = still.cells.iter().filter(|(p, _)| !taps.contains(p)).cloned().collect();
        v.push(format!("still:{}", key_of_cells(&cells, &entities, &stocked)));
    }
    v.extend(d.as_ref().and_then(ring_key));
    v.extend(d.as_ref().and_then(hopper_loop_key));
    v
}

/// The hopper loop key of a design (the same class as [`ring_key`], vjeux 13:09Z "fix the entire
/// class": the two-hopper clocks of 14 ticks, hop2-p14 with its comparator behind a hopper and
/// clk-item-hopper-ring with it beside one, read the same on the Clocks tab): hoppers passing
/// their items round one loop (each hopper's output, its `facing`, into the next hopper, through
/// every hopper of the design), comparators reading hoppers of the loop (the cell at a comparator's
/// back), and inert blocks under them (stone: no redstone component, no relay, no power source);
/// keyed by the loop's length, which hoppers the comparators read (smallest rotation) and the items
/// each hopper starts with (they set the period: a clock of the same period, as the fold groups
/// them, with other contents is another design). Anything else gets none.
pub fn hopper_loop_key(d: &mctas::design::Design) -> Option<String> {
    if !d.entities.is_empty() {
        return None;
    }
    let split = |s: &str| -> (String, Vec<(String, String)>) {
        let s = s.trim_start_matches("minecraft:");
        let (id, props) = s.split_once('[').map(|(a, b)| (a, b.trim_end_matches(']'))).unwrap_or((s, ""));
        (id.trim().to_string(), props.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect())
    };
    let mut hoppers: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
    let mut comparators: Vec<([i32; 3], [i32; 3])> = Vec::new();
    for (p, s) in &d.blocks {
        let (id, props) = split(s);
        let prop = |k: &str| props.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        match id.as_str() {
            "hopper" => {
                hoppers.insert(*p, dir_vec(&prop("facing")?)?);
            }
            // (a comparator's `facing` points at its back, where it reads: the vanilla placement
            // state is the player's facing reversed)
            "comparator" => comparators.push((*p, dir_vec(&prop("facing")?)?)),
            _ if kept(&id) || toggled(&id) || relay(&id, false) || id == "redstone_block" || id.contains("piston") => return None,
            _ => {}
        }
    }
    if hoppers.len() < 2 || comparators.is_empty() {
        return None;
    }
    // one loop through every hopper
    let start = *hoppers.keys().next()?;
    let mut order: Vec<[i32; 3]> = Vec::new();
    let mut u = start;
    loop {
        order.push(u);
        let f = hoppers[&u];
        let next = [u[0] + f[0], u[1] + f[1], u[2] + f[2]];
        if !hoppers.contains_key(&next) {
            return None;
        }
        u = next;
        if u == start {
            break;
        }
        if order.len() > hoppers.len() {
            return None;
        }
    }
    if order.len() != hoppers.len() {
        return None;
    }
    // each comparator reads a hopper of the loop
    let mut read = vec![0usize; order.len()];
    for (p, f) in &comparators {
        let back = [p[0] + f[0], p[1] + f[1], p[2] + f[2]];
        let k = order.iter().position(|h| *h == back)?;
        read[k] += 1;
    }
    // the items each hopper starts with (its `nbt:` line's ids and counts, sorted)
    let items: Vec<String> = order
        .iter()
        .map(|h| {
            let nbt = d.block_entities.get(h).cloned().unwrap_or_default();
            let mut v: Vec<String> = nbt.split("id:\"").skip(1).map(|part| {
                let id = part.split('"').next().unwrap_or("").trim_start_matches("minecraft:").to_string();
                let count = part.split("count:").nth(1).and_then(|c| c.split(|ch: char| !ch.is_ascii_digit()).next()).unwrap_or("1").to_string();
                format!("{id}x{count}")
            }).collect();
            v.sort();
            v.join("+")
        })
        .collect();
    let n = order.len();
    let best = (0..n).map(|k| (0..n).map(|j| format!("{}:{}", read[(k + j) % n], items[(k + j) % n])).collect::<Vec<_>>().join(";")).min()?;
    Some(format!("hoppers:{best}"))
}

/// A relay: a block whose state changes when the observer behind it pulses, so that the observer
/// watching it sees the pulse ([`toggled`] components, note blocks, lamps, copper bulbs, heads; a
/// dispenser or a dropper only when it holds nothing).
fn relay(id: &str, stocked: bool) -> bool {
    toggled(id)
        || matches!(id, "note_block" | "redstone_lamp")
        || id.ends_with("copper_bulb")
        || ((id.ends_with("_skull") || id.ends_with("_head")) && id != "piston_head")
        || (matches!(id, "dispenser" | "dropper") && !stocked)
}

/// The ring key of a design (the coordinator 13:55Z for vjeux: the observer rings with a relay
/// fold as one class, whatever their layout: clk-misc stands its ring up with the relay in a
/// corner, clk-obs lays it flat with the relay mid-row): its signal's loop, from the smallest
/// rotation of its sequence of observers (`O`) and relays (`R`). Conservative: a design gets one
/// only when every block is an observer or a relay, no two relays touch (a powered relay would
/// power its neighbour), it has no entity, and the signal runs one way round one loop through every
/// block: each observer watches exactly one block of the design (the cell its face looks at) and
/// is reached by that block's changes, and each relay is powered by exactly one observer (the one
/// whose back faces it) and watched by exactly one; an observer whose back faces another observer
/// or the air affects nothing there (an observer ignores power). Then its geometry plays no part:
/// an observer reacts to the block it watches and nothing else, a relay to the observer behind it,
/// so the loop is the whole mechanism. Anything else (a branch, a block off the loop, a piston, a
/// dust line, two loops) gets none and folds only by its layout ([`key_of_cells`]).
pub fn ring_key(d: &mctas::design::Design) -> Option<String> {
    if !d.entities.is_empty() || d.blocks.is_empty() {
        return None;
    }
    let stocked = stocked(d);
    let split = |s: &str| -> (String, Vec<(String, String)>) {
        let s = s.trim_start_matches("minecraft:");
        let (id, props) = s.split_once('[').map(|(a, b)| (a, b.trim_end_matches(']'))).unwrap_or((s, ""));
        (id.trim().to_string(), props.split(',').filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect())
    };
    // the nodes: observers (with the way they face) and relays; a door's upper half is its door
    let mut facing: BTreeMap<[i32; 3], Option<[i32; 3]>> = BTreeMap::new();
    let mut alias: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
    let mut relays: Vec<[i32; 3]> = Vec::new();
    for (p, s) in &d.blocks {
        let (id, props) = split(s);
        let prop = |k: &str| props.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        if id == "observer" {
            facing.insert(*p, Some(dir_vec(prop("facing")?)?));
        } else if relay(&id, stocked.contains(p)) {
            relays.push(*p);
            if id.ends_with("_door") && prop("half") == Some("upper") {
                alias.insert(*p, [p[0], p[1] - 1, p[2]]);
            } else {
                facing.insert(*p, None);
            }
        } else {
            return None;
        }
    }
    let at = |p: [i32; 3]| -> Option<[i32; 3]> {
        let q = alias.get(&p).copied().unwrap_or(p);
        facing.contains_key(&q).then_some(q)
    };
    // an upper half with no door under it
    if alias.values().any(|q| facing.get(q) != Some(&None)) {
        return None;
    }
    // no two relays touching
    for a in &relays {
        for b in &relays {
            let touch = (0..3).map(|k| (a[k] - b[k]).abs()).sum::<i32>() == 1;
            if touch && at(*a) != at(*b) {
                return None;
            }
        }
    }
    // the signal's way: next[u] = the node u's change reaches (one only), and how many reach each
    let mut next: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
    let mut reached: BTreeMap<[i32; 3], usize> = BTreeMap::new();
    for (p, f) in &facing {
        let Some(f) = f else { continue };
        // it watches the cell its face looks at: that block's changes reach it
        let front = at([p[0] + f[0], p[1] + f[1], p[2] + f[2]])?;
        if next.insert(front, *p).is_some() {
            return None;
        }
        *reached.entry(*p).or_default() += 1;
        // it powers the relay its back faces
        if let Some(b) = at([p[0] - f[0], p[1] - f[1], p[2] - f[2]])
            && facing.get(&b) == Some(&None)
        {
            if next.insert(*p, b).is_some() {
                return None;
            }
            *reached.entry(b).or_default() += 1;
        }
    }
    if next.len() != facing.len() || reached.len() != facing.len() || reached.values().any(|&n| n != 1) {
        return None;
    }
    // one loop through every node
    let start = *facing.keys().next()?;
    let mut seq = String::new();
    let mut u = start;
    loop {
        seq.push(if facing[&u].is_some() { 'O' } else { 'R' });
        u = next[&u];
        if u == start {
            break;
        }
        if seq.len() > facing.len() {
            return None;
        }
    }
    if seq.len() != facing.len() {
        return None;
    }
    let best = (0..seq.len()).map(|k| format!("{}{}", &seq[k..], &seq[..k])).min()?;
    Some(format!("ring:{best}"))
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
        // (a clock a builder can wire into a circuit keeps the card when its mechanism has one:
        // the coordinator 2026-10-21, the judge's `usable`; a hard one is listed under it)
        let hard = |i: usize| builds[i].clock.as_ref().is_some_and(|c| c.usable.starts_with("no"));
        g.sort_by_key(|&i| (!kind_card(&builds[i]), hard(i), builds[i].blocks, builds[i].volume.unwrap_or(u32::MAX), !keeps_card(&builds[i]), builds[i].name.clone()));
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
        // a clock that is hard to wire never keeps the card over one a builder can wire, however
        // small (the judge's `usable`)
        let mut hard = builds.clone();
        if let Some(c) = hard[3].clock.as_mut() {
            c.usable = "no: the output is under the build".into();
        }
        let mut pairs = folds(&hard, &keys);
        pairs.sort();
        assert_eq!(pairs, vec![(1, 0), (2, 0), (3, 0)], "z-gate-ring-n1, easy, keeps the card");
    }

    /// clk-misc's ring one step longer (n = 1): standing up, the relay in a corner.
    const UPRIGHT: &str = "name: u\norigin: 0 0 0\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  B minecraft:observer[facing=up,powered=false]\n  C minecraft:observer[facing=west,powered=false]\n  O minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]\n  o minecraft:observer[facing=down,powered=false]\nlayer 0:\n  AB\nlayer 1:\n  oB\nlayer 2:\n  oB\nlayer 3:\n  OC\n";
    /// clk-obs's ring at n = 1: flat, the relay mid-row (another layout of the same loop).
    const FLAT: &str = "name: f\norigin: 0 0 0\npalette:\n  A minecraft:observer[facing=south,powered=false]\n  B minecraft:observer[facing=west,powered=false]\n  C minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]\n  D minecraft:observer[facing=north,powered=false]\n  E minecraft:observer[facing=east,powered=false]\nlayer 0:\n  ABCB\n  EEED\n";

    fn ring_of(text: &str) -> Option<String> {
        ring_key(&mctas::design::Design::parse(text).unwrap())
    }

    #[test]
    fn a_ring_is_its_loop_whatever_its_layout() {
        // (the coordinator 13:55Z: rings at 14 ticks and up showed two cards, the upright and the
        // flat layouts)
        assert_ne!(key(UPRIGHT), key(FLAT), "two layouts");
        assert_eq!(ring_of(UPRIGHT).as_deref(), Some("ring:OOOOOOOR"));
        assert_eq!(ring_of(UPRIGHT), ring_of(FLAT));
        // the n = 0 ring (five observers and a relay) is another loop
        assert_eq!(ring_of(&ring("minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]")).as_deref(), Some("ring:OOOOOR"));
        // a door relay: its upper half is the door
        let door = ring("minecraft:iron_door[facing=north,half=lower,hinge=left,open=false,powered=false]").replace("layer 2:\n  OC\n", "layer 2:\n  OC\nlayer 3:\n  U\n").replace("  o minecraft", "  U minecraft:iron_door[facing=north,half=upper,hinge=left,open=false,powered=false]\n  o minecraft");
        assert_eq!(ring_of(&door).as_deref(), Some("ring:OOOOOR"));
    }

    #[test]
    fn anything_but_one_clean_loop_has_no_ring_key() {
        // another component (dust, stone, a stocked dispenser), a block off the loop, two relays
        // touching, an observer watching the air: no ring key
        assert_eq!(ring_of(&ring("minecraft:stone")), None, "stone is no relay");
        assert_eq!(ring_of(&ring("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]")), None);
        let stocked = format!("{}nbt: 0 2 0 {{Items:[{{Slot:0b,id:\"minecraft:arrow\",count:1}}]}}\n", ring("minecraft:dispenser[facing=north,triggered=false]"));
        assert_eq!(ring_of(&stocked), None, "a stocked dispenser");
        let extra = UPRIGHT.replace("layer 3:\n  OC\n", "layer 3:\n  OC\nlayer 4:\n  O\n");
        assert_eq!(ring_of(&extra), None, "two relays touching");
        let off = UPRIGHT.replace("layer 0:\n  AB\n", "layer 0:\n  AB.O\n");
        assert_eq!(ring_of(&off), None, "a relay off the loop");
        let blind = FLAT.replace("  ABCB\n", "  ABCB.A\n");
        assert_eq!(ring_of(&blind), None, "an observer watching the air");
        assert_eq!(ring_of(LAMP), None, "a piston loop");
    }

    fn loop_of(text: &str) -> Option<String> {
        hopper_loop_key(&mctas::design::Design::parse(text).unwrap())
    }

    #[test]
    fn a_tap_is_no_part_of_the_mechanism() {
        // clk-obs-p30-obs-stk-6b-f5eb19 before and after the usable-output rule gave it an observer
        // tap (its comment names the cell), and a comparator tap with the stone under it
        let before = "name: c\norigin: 0 0 -1\nport output: 0 -1 -1\npalette:\n  A minecraft:observer[facing=up,powered=false]\n  B minecraft:note_block[instrument=harp,note=0,powered=false]\n  C minecraft:sticky_piston[extended=false,facing=north]\n  D minecraft:observer[facing=down,powered=false]\n  E minecraft:observer[facing=north,powered=false]\nlayer 0:\n  .\n  A\n  B\n  C\nlayer 1:\n  D\n  D\n  E\n  .\n";
        let after = "# port output moved 2026-10-21 (usable-output rule): the output of an observer watching the build to its east, added at -1 1 -1 (was 0 -1 -1: the output [0, -1, -1] is under the build)\nname: c\nport output: -2 1 -1\norigin: -1 0 -1\npalette:\n  A minecraft:observer[facing=north,powered=false]\n  B minecraft:observer[facing=up,powered=false]\n  N minecraft:note_block[instrument=harp,note=0,powered=false]\n  O minecraft:observer[facing=down,powered=false]\n  S minecraft:sticky_piston[extended=false,facing=north]\n  o minecraft:observer[facing=east,powered=false]\nlayer 0:\n  .\n  .B\n  .N\n  .S\nlayer 1:\n  oO\n  .O\n  .A\n  .\n";
        assert_ne!(key(after.trim_start_matches(|c| c != '\n')), key(before), "the tap is a block");
        assert_eq!(key(after), key(before), "without the tap: one mechanism");
        // a comparator tap with its stone, and a block put under a repeater's cell
        let plain = "name: d\norigin: 0 0 0\nport output: 1 0 0\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  N minecraft:note_block[instrument=harp,note=0,powered=false]\nlayer 0:\n  NA\n";
        let comparator = "# port output moved 2026-10-21 (usable-output rule): the output of a comparator facing south (on the ground) that reads the build, added at 0 0 1 (was 1 0 0: ...)\nname: d\norigin: 0 0 0\nport output: 0 0 2\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  N minecraft:note_block[instrument=harp,note=0,powered=false]\n  S minecraft:stone\n  c minecraft:comparator[facing=north,mode=compare,powered=false]\nlayer -1:\n  ..\n  S\nlayer 0:\n  NA\n  c\n";
        assert_ne!(key(&comparator.replace("(usable-output rule)", "(clockinfra)")), key(plain), "the comparator and its stone are blocks");
        assert_eq!(key(comparator), key(plain), "without the comparator tap and its stone: one mechanism");
        let put = "# port output moved 2026-10-21 (usable-output rule): an air cell where a repeater facing north (on a block put under it) takes every pulse (was 0 0 0: ...)\nname: d\norigin: 0 0 0\nport output: 2 0 0\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  N minecraft:note_block[instrument=harp,note=0,powered=false]\n  S minecraft:stone\nlayer -1:\n  ..S\nlayer 0:\n  NA\n";
        assert_eq!(key(put), key(plain), "without the block put under the repeater's cell");
    }

    #[test]
    fn a_hopper_loop_is_its_loop_whatever_its_layout() {
        // hop2-p14 (the comparator behind a hopper) and clk-item-hopper-ring (beside it)
        let line = "name: a\norigin: -1 0 0\nnbt: 0 1 0 {Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}\npalette:\n  C minecraft:comparator[facing=east,mode=compare,powered=false]\n  H minecraft:hopper[enabled=true,facing=east]\n  S minecraft:stone\n  h minecraft:hopper[enabled=true,facing=west]\nlayer 0:\n  S\nlayer 1:\n  CHh\n";
        let side = "name: b\norigin: 0 0 -1\nnbt: 0 1 0 {Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}\npalette:\n  C minecraft:comparator[facing=south,mode=compare,powered=false]\n  H minecraft:hopper[enabled=true,facing=east]\n  S minecraft:stone\n  h minecraft:hopper[enabled=true,facing=west]\nlayer 0:\n  S\n  .\nlayer 1:\n  C\n  Hh\n";
        assert_ne!(key(line), key(side), "two layouts");
        assert!(loop_of(line).is_some());
        assert_eq!(loop_of(line), loop_of(side));
        // other contents, a comparator reading something else, a dust line: another key or none
        assert_ne!(loop_of(line), loop_of(&line.replace("count:1}", "count:5}")), "other items");
        assert_eq!(loop_of(&line.replace("facing=east,mode", "facing=west,mode")), None, "a comparator reading no hopper");
        assert_eq!(loop_of(&line.replace("  S minecraft:stone", "  S minecraft:redstone_wire")), None, "dust");
        assert_eq!(loop_of(&line.replace("h minecraft:hopper[enabled=true,facing=west]", "h minecraft:hopper[enabled=true,facing=down]")), None, "no loop");
    }
}
