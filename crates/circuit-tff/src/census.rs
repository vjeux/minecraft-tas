//! The census engine: every filling of k free cells (a box, around an optional skeleton) with
//! palette states, judged on redsim under a fixed stimulus, every cell near the structure tried as
//! the output. Irreducible circuits are kept, one per type (rotations + mirror images, the input
//! cell marked).

use crate::rs::Event;
use redsim::behavior::can_survive;
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the census looks for: the stimulus and the per-cell judge.
pub trait Spec: Sync {
    /// Input events (input index into the census inputs).
    fn events(&self) -> Vec<Event>;
    fn frames(&self) -> u32;
    /// The first frame whose output changes are judged (everything before is the settle).
    fn watch_from(&self) -> u32;
    /// A frame by which a candidate must have reacted (some cell changed), else it is dropped.
    fn react_by(&self) -> u32;
    /// Judge one cell from its on/off changes (frames >= watch_from, each a frame whose on/off
    /// differs from the frame before) and its on/off at `watch_from - 1`: a score (lower = better,
    /// e.g. the latency) or None.
    fn judge(&self, rest_on: bool, changes: &[u32], samples: &[u8]) -> Option<u32>;
    /// Frames whose level (0-15) each cell records for the judge (`samples`), ascending.
    fn samples(&self) -> Vec<u32> {
        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub struct Census {
    pub inputs: Vec<BlockPos>,
    pub fixed: Vec<(BlockPos, Block)>,
    pub fixed_be: Vec<(BlockPos, String)>,
    pub free: Vec<BlockPos>,
    pub palette: Vec<Block>,
    /// Container contents per palette entry (`nbt:` data), or empty for none.
    pub palette_be: Vec<Option<String>>,
    pub k_min: usize,
    pub k_max: usize,
    /// Candidate output cells (None: every cell of the structure's box grown by 1).
    pub outputs: Option<Vec<BlockPos>>,
    pub jobs: usize,
    pub margin: i32,
}

/// One candidate's run: per output cell its rest state, on/off change frames and samples.
struct Traced {
    cells: Vec<BlockPos>,
    rest: Vec<bool>,
    changes: Vec<Vec<u32>>,
    samples: Vec<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct Found {
    pub blocks: Vec<(BlockPos, Block)>,
    pub be: Vec<(BlockPos, String)>,
    pub output: BlockPos,
    pub score: u32,
    pub key: String,
}

fn face_neighbors(p: BlockPos) -> [BlockPos; 6] {
    [
        BlockPos::new(p.x + 1, p.y, p.z),
        BlockPos::new(p.x - 1, p.y, p.z),
        BlockPos::new(p.x, p.y + 1, p.z),
        BlockPos::new(p.x, p.y - 1, p.z),
        BlockPos::new(p.x, p.y, p.z + 1),
        BlockPos::new(p.x, p.y, p.z - 1),
    ]
}

/// Are the cells (and the inputs) one face-connected group?
fn connected(cells: &[BlockPos], inputs: &[BlockPos]) -> bool {
    let all: HashSet<BlockPos> = cells.iter().chain(inputs).copied().collect();
    let Some(&start) = inputs.first().or(cells.first()) else { return true };
    let mut seen: HashSet<BlockPos> = HashSet::new();
    let mut stack = vec![start];
    seen.insert(start);
    while let Some(p) = stack.pop() {
        for q in face_neighbors(p) {
            if all.contains(&q) && seen.insert(q) {
                stack.push(q);
            }
        }
    }
    seen.len() == all.len()
}

fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut idx = Vec::new();
    fn rec(n: usize, k: usize, start: usize, idx: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if idx.len() == k {
            out.push(idx.clone());
            return;
        }
        for i in start..n {
            idx.push(i);
            rec(n, k, i + 1, idx, out);
            idx.pop();
        }
    }
    rec(n, k, 0, &mut idx, &mut out);
    out
}

/// The box of the cells grown by `g`.
fn bounds(cells: impl Iterator<Item = BlockPos>, g: i32) -> (BlockPos, BlockPos) {
    let mut lo = BlockPos::new(i32::MAX, i32::MAX, i32::MAX);
    let mut hi = BlockPos::new(i32::MIN, i32::MIN, i32::MIN);
    for p in cells {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    (BlockPos::new(lo.x - g, lo.y - g, lo.z - g), BlockPos::new(hi.x + g, hi.y + g, hi.z + g))
}

impl Census {
    /// The world box for any candidate (every free cell, fixed block and input, plus margin).
    fn world_box(&self, blocks: &[(BlockPos, Block)]) -> (BlockPos, [i32; 3]) {
        let (lo, hi) = bounds(self.free.iter().chain(self.inputs.iter()).copied().chain(self.fixed.iter().map(|(p, _)| *p)).chain(blocks.iter().map(|(p, _)| *p)), self.margin);
        (lo, [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1])
    }

    /// Run one candidate: per output cell, its score, or None (not placeable, a clock, no
    /// reaction).
    pub fn simulate(&self, blocks: &[(BlockPos, Block)], be: &[(BlockPos, String)], spec: &dyn Spec) -> Option<Vec<(BlockPos, u32)>> {
        let tr = self.trace_cells(blocks, be, spec).ok()?;
        let mut out = Vec::new();
        for (i, p) in tr.cells.iter().enumerate() {
            if let Some(s) = spec.judge(tr.rest[i], &tr.changes[i], &tr.samples[i]) {
                out.push((*p, s));
            }
        }
        if out.is_empty() { None } else { Some(out) }
    }

    /// [`Census::simulate`] for one design, every output cell described (`circuit memory check`).
    pub fn simulate_debug(&self, blocks: &[(BlockPos, Block)], be: &[(BlockPos, String)], spec: &dyn Spec) -> Result<Vec<String>, String> {
        let tr = self.trace_cells(blocks, be, spec).map_err(|e| e.to_string())?;
        Ok(tr
            .cells
            .iter()
            .enumerate()
            .map(|(i, p)| format!("{},{},{}: rest {}, changes {:?}, samples {:?}: {:?}", p.x, p.y, p.z, if tr.rest[i] { "on" } else { "off" }, tr.changes[i], tr.samples[i], spec.judge(tr.rest[i], &tr.changes[i], &tr.samples[i])))
            .collect())
    }

    /// The run behind [`Census::simulate`]: per output cell its rest state, change frames and
    /// samples, or why the candidate is dropped.
    fn trace_cells(&self, blocks: &[(BlockPos, Block)], be: &[(BlockPos, String)], spec: &dyn Spec) -> Result<Traced, &'static str> {
        let (min, size) = self.world_box(blocks);
        let mut w = World::new(min, size);
        w.place_raw(blocks);
        let lines = self.fixed_be.iter().chain(be.iter()).filter(|(p, _)| blocks.iter().any(|(q, _)| q == p));
        if redsim::redstone::container::apply_block_entities(&mut w, lines.map(|(p, s)| (*p, s.as_str()))).is_err() {
            return Err("nbt not modeled");
        }
        w.place_updates(blocks);
        if !w.drops.is_empty() || !w.errors.is_empty() {
            return Err("a drop or a model error at placement");
        }
        for (p, _) in blocks {
            let b = w.get(*p);
            if !can_survive(&w, &b, *p) {
                return Err("an unsupported block");
            }
        }
        let cells: Vec<BlockPos> = match &self.outputs {
            Some(v) => v.clone(),
            None => {
                let (lo, hi) = bounds(blocks.iter().map(|(p, _)| *p), 1);
                let mut v = Vec::new();
                for y in lo.y..=hi.y {
                    for z in lo.z..=hi.z {
                        for x in lo.x..=hi.x {
                            let p = BlockPos::new(x, y, z);
                            if !self.inputs.contains(&p) {
                                v.push(p);
                            }
                        }
                    }
                }
                v
            }
        };
        let mut ev = spec.events();
        ev.sort_by_key(|e| e.t);
        let frames = spec.frames();
        let from = spec.watch_from();
        let react = spec.react_by();
        let mut k = 0;
        let mut last: Vec<bool> = vec![false; cells.len()];
        let mut rest: Vec<bool> = vec![false; cells.len()];
        let mut changes: Vec<Vec<u32>> = vec![Vec::new(); cells.len()];
        let sample_at = spec.samples();
        let mut si = 0;
        let mut samples: Vec<Vec<u8>> = vec![Vec::with_capacity(sample_at.len()); cells.len()];
        // a tick of a quiet world (no scheduled tick, block event or moving block, and no block
        // that acts every tick) changes nothing: its frame keeps the previous levels
        let self_ticking = blocks.iter().any(|(_, b)| matches!(b, Block::Hopper { .. } | Block::Sculk { .. } | Block::Falling { .. } | Block::Crafter { .. }));
        let mut dirty = true;
        let mut lv: Vec<u8> = vec![0; cells.len()];
        for t in 0..frames {
            while k < ev.len() && ev[k].t == t {
                let e = ev[k];
                let pos = self.inputs[e.input];
                // the judge's input device: the cell is air whenever the input is off, and the
                // redstone block stays put while on (nothing pushes it or moves into the cell)
                let ok = if e.on { w.get(pos) == Block::Air } else { w.get(pos) == Block::RedstoneBlock };
                if !ok {
                    return Err("the input cell was not air (or the input block moved)");
                }
                w.set_block(pos, if e.on { Block::RedstoneBlock } else { Block::Air }, flags::ALL);
                dirty = true;
                k += 1;
            }
            if !w.drops.is_empty() || !w.errors.is_empty() {
                return Err("a drop or a model error");
            }
            let need_levels = t + 1 >= from || sample_at.get(si) == Some(&t);
            if need_levels && dirty {
                for (i, p) in cells.iter().enumerate() {
                    lv[i] = crate::rs::level(&w, *p);
                }
            }
            if t + 1 >= from {
                if t + 1 == from && self.inputs.iter().any(|p| w.get(*p) != Block::Air) {
                    // something moved into an input cell during the settle
                    return Err("something moved into an input cell");
                }
                for i in 0..cells.len() {
                    let on = lv[i] > 0;
                    if t + 1 == from {
                        rest[i] = on;
                    } else if on != last[i] {
                        changes[i].push(t);
                    }
                    last[i] = on;
                }
                if t + 1 == from && (w.ticks.count() > 0 || w.piston.live_count() > 0) {
                    // still running at rest: a clock or a slow settle
                    return Err("still running at rest");
                }
            }
            if t == react && changes.iter().all(|c| c.is_empty()) {
                return Err("no reaction");
            }
            while si < sample_at.len() && sample_at[si] == t {
                for i in 0..cells.len() {
                    samples[i].push(lv[i]);
                }
                si += 1;
            }
            dirty = self_ticking || w.ticks.count() > 0 || w.piston.live_count() > 0 || !w.events.is_empty() || !need_levels;
            w.tick();
        }
        Ok(Traced { cells, rest, changes, samples })
    }

    /// The best output of a candidate: an air cell first (it is not counted), then the lowest
    /// score.
    fn best(&self, blocks: &[(BlockPos, Block)], outs: &[(BlockPos, u32)]) -> (BlockPos, u32) {
        let occupied: HashSet<BlockPos> = blocks.iter().map(|(p, _)| *p).collect();
        *outs.iter().min_by_key(|(p, s)| (occupied.contains(p), *s, *p)).unwrap()
    }

    /// The type key: the blocks with the input cells marked, smallest over rotations and mirrors.
    pub fn key(&self, blocks: &[(BlockPos, Block)], be: &[(BlockPos, String)]) -> String {
        let mut m: BTreeMap<[i32; 3], String> = BTreeMap::new();
        for (p, b) in blocks {
            m.insert([p.x, p.y, p.z], b.to_string());
        }
        // container contents tell types apart: rotate them with their block (the canon turns the
        // block state; the contents ride along as a property-free suffix block name)
        let _ = be;
        for p in &self.inputs {
            m.insert([p.x, p.y, p.z], "minecraft:lime_stained_glass".into());
        }
        clock::canon::type_key(m.iter())
    }

    /// Run the census: every irreducible circuit found, one per type.
    pub fn run(&self, spec: &dyn Spec, progress: bool) -> Vec<Found> {
        let found: Mutex<BTreeMap<String, Found>> = Mutex::new(BTreeMap::new());
        let free_set: HashSet<BlockPos> = self.free.iter().copied().collect();
        let _ = free_set;
        for k in self.k_min..=self.k_max {
            let subs: Vec<Vec<usize>> = combinations(self.free.len(), k)
                .into_iter()
                .filter(|s| {
                    let cells: Vec<BlockPos> = s.iter().map(|&i| self.free[i]).chain(self.fixed.iter().map(|(p, _)| *p)).collect();
                    connected(&cells, &self.inputs)
                })
                .collect();
            let next = AtomicUsize::new(0);
            let judged = AtomicUsize::new(0);
            let start = std::time::Instant::now();
            let before = found.lock().unwrap().len();
            // work items: (cell set, the first two cells' states) so the threads stay busy
            let p = self.palette.len();
            let pre = k.min(2);
            let per: usize = p.pow(pre as u32);
            let items = subs.len() * per;
            std::thread::scope(|sc| {
                for _ in 0..self.jobs.max(1) {
                    sc.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, Ordering::Relaxed);
                            if i >= items {
                                break;
                            }
                            let (si, mut code) = (i / per, i % per);
                            let cells: Vec<BlockPos> = subs[si].iter().map(|&j| self.free[j]).collect();
                            let mut assign: Vec<usize> = Vec::with_capacity(k);
                            for _ in 0..pre {
                                assign.push(code % p);
                                code /= p;
                            }
                            self.dfs(&cells, &mut assign, spec, &found, &judged);
                        }
                    });
                }
            });
            if progress {
                eprintln!(
                    "census k={k}: {} cell sets, {} candidates judged, {} new types in {:.1} s",
                    subs.len(),
                    judged.load(Ordering::Relaxed),
                    found.lock().unwrap().len() - before,
                    start.elapsed().as_secs_f64()
                );
            }
        }
        found.into_inner().unwrap().into_values().collect()
    }

    fn dfs(&self, cells: &[BlockPos], assign: &mut Vec<usize>, spec: &dyn Spec, found: &Mutex<BTreeMap<String, Found>>, judged: &AtomicUsize) {
        if assign.len() == cells.len() {
            let mut blocks: Vec<(BlockPos, Block)> = self.fixed.clone();
            blocks.extend(cells.iter().copied().zip(assign.iter().map(|&i| self.palette[i])));
            let be: Vec<(BlockPos, String)> = cells.iter().zip(assign.iter()).filter_map(|(p, &i)| self.palette_be.get(i).cloned().flatten().map(|s| (*p, s))).collect();
            // every input must reach something that reacts to it
            if !self.inputs.iter().all(|ip| blocks.iter().any(|(q, b)| face_neighbors(*ip).contains(q) && responsive(b, *q, *ip))) {
                return;
            }
            judged.fetch_add(1, Ordering::Relaxed);
            let Some(outs) = self.simulate(&blocks, &be, spec) else { return };
            let (output, score) = self.best(&blocks, &outs);
            // irreducible: no single free block can go with the same class of result (score /
            // 1000: a counter's style, a latch's polarity)
            for i in 0..cells.len() {
                let fewer: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| *p != cells[i]).copied().collect();
                if let Some(o2) = self.simulate(&fewer, &be, spec)
                    && o2.iter().any(|(_, s)| s / 1000 == score / 1000)
                {
                    return;
                }
            }
            let mut key = self.key(&blocks, &be);
            if !be.is_empty() {
                // contents per relative cell (cheap but rotation-unaware: two rotated fillings may
                // both be kept; the archive's type identity folds them)
                let lo = blocks.iter().map(|(p, _)| (p.x, p.y, p.z)).min().unwrap();
                let mut v: Vec<String> = be.iter().map(|(p, s)| format!("{},{},{}={s}", p.x - lo.0, p.y - lo.1, p.z - lo.2)).collect();
                v.sort();
                key.push_str(&format!("|{}", v.join(";")));
            }
            let mut f = found.lock().unwrap();
            let better = match f.get(&key) {
                None => true,
                Some(old) => (score, output) < (old.score, old.output),
            };
            if better {
                f.insert(key.clone(), Found { blocks, be, output, score, key });
            }
            return;
        }
        for i in 0..self.palette.len() {
            assign.push(i);
            self.dfs(cells, assign, spec, found, judged);
            assign.pop();
        }
    }
}

/// Does a block at `q`, a face neighbor of the input cell `p`, react to a redstone block written
/// in `p` (with updates)? Dust, a diode whose back (or a comparator whose side) faces it, a
/// piston not facing it, an observer watching it, droppers, dispensers, crafters, bulbs, lamps,
/// note blocks, trapdoors, doors, gates, rails, hoppers. Plain blocks do not: a redstone block
/// does not power a solid block.
pub fn responsive(b: &Block, q: BlockPos, p: BlockPos) -> bool {
    let to_p = |d: redsim::Dir| q.rel(d) == p;
    match *b {
        Block::Wire { .. } => true,
        Block::Repeater { facing, .. } => to_p(facing),
        Block::Comparator { facing, .. } => !to_p(facing.opposite()),
        Block::Piston { facing, .. } => !to_p(facing),
        Block::Observer { facing, .. } => to_p(facing),
        Block::Dispenser { .. } | Block::Crafter { .. } | Block::Bulb { .. } | Block::Lamp { .. } | Block::Note { .. } | Block::Trapdoor { .. } | Block::Door { .. } | Block::FenceGate { .. } | Block::Rail { .. } | Block::Hopper { .. } | Block::Sculk { .. } => true,
        _ => false,
    }
}

/// The TFF census spec: pulses of `len` gt every `period` gt, `count` of them; a cell passes when
/// it is steady at rest and changes exactly once per pulse (at or after the pulse, within
/// `MAX_LATENCY`); score = the worst latency.
pub struct TffSpec {
    pub len: u32,
    pub period: u32,
    pub count: u32,
    /// Two pulses this long after the train (0: none).
    pub long: u32,
}

impl TffSpec {
    /// Each pulse: (start, length).
    fn pulses(&self) -> Vec<(u32, u32)> {
        let mut v: Vec<(u32, u32)> = (0..self.count).map(|i| (crate::tff::SETTLE + i * self.period, self.len)).collect();
        if self.long > 0 {
            let gap = self.period.max(self.long + 30);
            let mut t = crate::tff::SETTLE + (self.count - 1) * self.period + self.period.max(self.len + 30);
            for _ in 0..2 {
                v.push((t, self.long));
                t += gap;
            }
        }
        v
    }
}

impl Spec for TffSpec {
    fn events(&self) -> Vec<Event> {
        let mut ev = Vec::new();
        for (t, l) in self.pulses() {
            ev.push(Event { t, input: 0, on: true });
            ev.push(Event { t: t + l, input: 0, on: false });
        }
        ev
    }
    fn frames(&self) -> u32 {
        let (t, l) = *self.pulses().last().unwrap();
        t + l + 40
    }
    fn watch_from(&self) -> u32 {
        crate::tff::SETTLE - crate::tff::REST
    }
    fn react_by(&self) -> u32 {
        // a candidate whose first reaction comes more than 16 gt after the first pulse is dropped
        // early (census speed)
        crate::tff::SETTLE + 16.min(self.period - 1)
    }
    fn judge(&self, _rest_on: bool, changes: &[u32], _samples: &[u8]) -> Option<u32> {
        let p = self.pulses();
        if changes.len() != p.len() {
            return None;
        }
        let mut worst = 0;
        for (i, &t) in changes.iter().enumerate() {
            let r = p[i].0;
            if t < r || t - r > crate::tff::MAX_LATENCY + p[i].1 || p.get(i + 1).is_some_and(|n| t >= n.0) {
                return None;
            }
            if i < self.count as usize {
                worst = worst.max(t - r);
            }
        }
        Some(worst)
    }
}
