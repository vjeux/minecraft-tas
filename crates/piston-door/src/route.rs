//! Dust auto-router: shortest (fewest new blocks) redstone-dust path between two cells of a
//! design, respecting what dust and its floors must not touch.
//!
//! Rules: every dust cell needs a full block below it (an existing plain floor block or a new
//! one); dust connects horizontally and by one-block steps (the cell above the lower dust's
//! neighbor must be clear); new dust and new floors may not be face-adjacent to any component
//! (pistons, torches, repeaters, observers, levers, slime, door blocks, other dust) except the
//! cells listed in `allow`; and nothing may be placed in `reserved` cells (piston travel, the
//! door corridor).

use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::cmp::Reverse;

use mctas::blockstate::BlockState;
use mctas::sim::Pos;

pub const FLOOR: &str = "minecraft:smooth_stone";

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const HORIZ: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];

pub struct Router<'a> {
    pub blocks: &'a BTreeMap<Pos, String>,
    pub reserved: HashSet<Pos>,
    pub allow: HashSet<Pos>,
    pub lo: Pos,
    pub hi: Pos,
}

impl Router<'_> {
    fn name(&self, p: Pos) -> Option<String> {
        self.blocks.get(&p).map(|s| BlockState::parse(s).map(|b| b.short_name().to_string()).unwrap_or_default())
    }
    fn is_plain(&self, p: Pos) -> bool {
        matches!(self.name(p).as_deref(), Some("smooth_stone"))
    }
    fn free(&self, p: Pos, placed: &HashSet<Pos>) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i]) && !self.blocks.contains_key(&p) && !self.reserved.contains(&p) && !placed.contains(&p)
    }
    /// A new block at `p` must not touch components (anything but plain floors) unless allowed.
    fn clean(&self, p: Pos, path_dust: &HashSet<Pos>) -> bool {
        FACES.iter().all(|f| {
            let n = add(p, *f);
            if self.allow.contains(&n) || path_dust.contains(&n) {
                return true;
            }
            match self.name(n) {
                None => true,
                Some(s) => s == "smooth_stone",
            }
        })
    }

    /// Route from any of `start` (existing dust cells of the net) to `goal` (a free cell that
    /// becomes dust). Returns the new blocks (dust + floors) or None.
    pub fn route(&self, start: &[Pos], goal: Pos) -> Option<Vec<(Pos, String)>> {
        let none = HashSet::new();
        // Cost to stand dust at cell c: 1 (dust, 0 if already dust) + 1 if its floor is new.
        let floor_cost = |c: Pos| -> Option<u32> {
            let f = add(c, [0, -1, 0]);
            if self.is_plain(f) && (self.clean(f, &none) || self.allow.contains(&f)) {
                Some(0)
            } else if self.free(f, &none) && self.clean(f, &none) {
                Some(1)
            } else {
                None
            }
        };
        let mut dist: HashMap<Pos, u32> = HashMap::new();
        let mut prev: HashMap<Pos, Pos> = HashMap::new();
        let mut heap = BinaryHeap::new();
        for s in start {
            dist.insert(*s, 0);
            heap.push(Reverse((0u32, *s)));
        }
        while let Some(Reverse((d, c))) = heap.pop() {
            if dist.get(&c).is_some_and(|&x| x < d) {
                continue;
            }
            if c == goal {
                break;
            }
            for h in HORIZ {
                for dy in [0, 1, -1] {
                    let n = add(add(c, h), [0, dy, 0]);
                    // Step clearance.
                    if dy == 1 && self.blocks.contains_key(&add(c, [0, 1, 0])) {
                        continue;
                    }
                    if dy == -1 && self.blocks.contains_key(&add(c, h)) {
                        continue;
                    }
                    if !(n == goal || self.free(n, &none)) || !self.clean(n, &[c].into_iter().collect()) {
                        continue;
                    }
                    let Some(fc) = floor_cost(n) else { continue };
                    let nd = d + 1 + fc;
                    if dist.get(&n).is_none_or(|&x| nd < x) {
                        dist.insert(n, nd);
                        prev.insert(n, c);
                        heap.push(Reverse((nd, n)));
                    }
                }
            }
        }
        if !dist.contains_key(&goal) {
            return None;
        }
        let mut out = Vec::new();
        let mut c = goal;
        while !start.contains(&c) {
            out.push((c, "minecraft:redstone_wire".to_string()));
            let f = add(c, [0, -1, 0]);
            if !self.is_plain(f) {
                out.push((f, FLOOR.to_string()));
            }
            c = prev[&c];
        }
        Some(out)
    }
}
