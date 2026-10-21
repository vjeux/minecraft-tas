//! Where a stray item gets into a design's item timing (the real-use audit's ITEM INTRUSION,
//! 2026-10-21; the circuit judge's run is `circuit::intrusion`, the clock judge may use the same
//! intakes). A hopper takes every item entity that comes to rest in the cell above it (its pickup
//! box covers that whole cell), so a hopper whose top is open is an intake for any stray item: a
//! mob drop, a crater's dirt thrown by a TNT, a player's dropped stack.

use mctas::design::Design;
use std::collections::{BTreeSet, HashSet, VecDeque};

type Pos = [i32; 3];

/// A block that closes the cell above a hopper: a full collision cube (an item rests on top of
/// it, outside the hopper's pickup box) or a container (the hopper pulls from it instead: that
/// container's own top is the intake, when open).
pub fn closes(state: &str) -> bool {
    let id = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    if ["hopper", "chest", "trapped_chest", "barrel", "dropper", "dispenser", "crafter", "composter"].contains(&id) || id.ends_with("shulker_box") {
        return true;
    }
    match redsim::Block::parse(state) {
        Ok(b) => redsim::behavior::full_cube(&b) || matches!(b, redsim::Block::Piston { extended: false, .. }),
        // a block redsim does not know: taken as closing (no intake claimed on a guess)
        Err(_) => true,
    }
}

/// The intakes of a design: the cell above each hopper whose top is open (nothing in that cell
/// closes it) and that a dropped item can reach: the cell lies outside the design's box, or a
/// path of cells holding no design block and no `blocked` cell (an input device's block) leads
/// from it out of the box. In position order.
pub fn intakes(d: &Design, blocked: &[Pos]) -> Vec<Pos> {
    let Some((lo, hi)) = d.bounds() else { return Vec::new() };
    let outside = |p: Pos| (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]);
    let blocked: HashSet<Pos> = blocked.iter().copied().collect();
    let mut out = BTreeSet::new();
    for (p, st) in &d.blocks {
        if st.split('[').next().unwrap_or(st).trim_start_matches("minecraft:") != "hopper" {
            continue;
        }
        let up = [p[0], p[1] + 1, p[2]];
        if d.blocks.get(&up).is_some_and(|s| closes(s)) {
            continue;
        }
        // reachable: a breadth-first walk through free cells (the cell above itself may hold a
        // thin block: dust, a rail, a repeater)
        let mut seen: HashSet<Pos> = HashSet::from([up]);
        let mut queue = VecDeque::from([up]);
        let mut reach = outside(up);
        while let Some(q) = queue.pop_front() {
            if reach {
                break;
            }
            for dd in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let n = [q[0] + dd[0], q[1] + dd[1], q[2] + dd[2]];
                if d.blocks.contains_key(&n) || blocked.contains(&n) || !seen.insert(n) {
                    continue;
                }
                if outside(n) {
                    reach = true;
                    break;
                }
                queue.push_back(n);
            }
        }
        if reach {
            out.insert(up);
        }
    }
    out.into_iter().collect()
}
