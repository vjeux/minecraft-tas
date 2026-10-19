//! Run a design in redsim (the exact model) until its whole state repeats: the period, the
//! transient, the active set (the family) and the output waveforms.

use redsim::redstone::container;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// A design as redsim sees it: blocks (placement order) and block-entity data (`nbt:` lines).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Spec {
    pub blocks: Vec<(BlockPos, Block)>,
    pub nbt: Vec<(BlockPos, String)>,
}

impl Spec {
    /// From a design file (blocks in position order, as the judge pastes them).
    pub fn from_design(d: &mctas::design::Design) -> Result<Spec, String> {
        let mut blocks = Vec::new();
        for (p, s) in &d.blocks {
            blocks.push((BlockPos::from_arr(*p), Block::parse(s)?));
        }
        let nbt = d.block_entities.iter().map(|(p, s)| (BlockPos::from_arr(*p), s.clone())).collect();
        Ok(Spec { blocks, nbt })
    }

    /// As a design (name, an optional `output` port).
    pub fn to_design(&self, name: &str, output: Option<BlockPos>) -> mctas::design::Design {
        let mut d = mctas::design::Design { name: name.to_string(), ..Default::default() };
        for (p, b) in &self.blocks {
            d.blocks.insert(p.arr(), b.to_string());
        }
        for (p, s) in &self.nbt {
            d.block_entities.insert(p.arr(), s.clone());
        }
        if let Some(o) = output {
            d.ports.insert("output".into(), o.arr());
        }
        d
    }

    /// Inclusive bounding box of the blocks.
    pub fn bounds(&self) -> (BlockPos, BlockPos) {
        let mut lo = self.blocks[0].0;
        let mut hi = lo;
        for (p, _) in &self.blocks {
            lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
            hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
        }
        (lo, hi)
    }
}

/// Cells of air kept around a design in its world: anything written there is a change outside
/// the design's box (reported), anything beyond is a model error.
pub const MARGIN: i32 = 2;

/// An empty world around the design.
pub fn new_world(spec: &Spec) -> World {
    let (lo, hi) = spec.bounds();
    let min = BlockPos::new(lo.x - MARGIN, lo.y - MARGIN, lo.z - MARGIN);
    World::new(min, [hi.x - lo.x + 1 + 2 * MARGIN, hi.y - lo.y + 1 + 2 * MARGIN, hi.z - lo.z + 1 + 2 * MARGIN])
}

/// What makes a run untrustworthy or the design illegal as a clock: model errors (unmodeled
/// cases, writes outside the world), blocks destroyed with drops (an item entity).
pub fn problem(w: &World) -> Option<String> {
    if let Some(e) = w.errors.first() {
        return Some(format!("model: {e}"));
    }
    if let Some((p, b)) = w.drops.first() {
        return Some(format!("drop: {b} at {p}"));
    }
    None
}

/// The clock judge's placement: raw paste, the `nbt:` lines (`data merge block`), wire shapes,
/// a self update of every block.
pub fn place(w: &mut World, spec: &Spec) -> Result<(), String> {
    w.place_raw(&spec.blocks);
    container::apply_block_entities(w, spec.nbt.iter().map(|(p, s)| (*p, s.as_str())))?;
    w.place_updates(&spec.blocks);
    match problem(w) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// A hash of the WHOLE state of the world, relative to the current time: every block, comparator
/// outputs, container slots, hopper cooldowns (and how long ago each hopper ticked), pending
/// scheduled ticks (delay, priority, order), torch toggle memory (age <= 60), moving blocks,
/// falling blocks. Two equal keys = the same future (the model is deterministic).
pub fn state_key(w: &World) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for y in w.min.y..w.min.y + w.size[1] {
        for z in w.min.z..w.min.z + w.size[2] {
            for x in w.min.x..w.min.x + w.size[0] {
                w.get(BlockPos::new(x, y, z)).hash(&mut h);
            }
        }
    }
    let mut cmp: Vec<(&BlockPos, &u8)> = w.comparator_out.iter().collect();
    cmp.sort();
    cmp.hash(&mut h);
    let mut cont: Vec<(&BlockPos, &container::ContainerBe)> = w.containers.map.iter().collect();
    cont.sort_by_key(|(p, _)| **p);
    for (p, c) in cont {
        p.hash(&mut h);
        c.slots.hash(&mut h);
        c.cooldown.hash(&mut h);
        if c.hopper {
            (w.time - c.ticked).hash(&mut h);
        }
    }
    for t in w.ticks.all() {
        (t.pos, t.kind, t.trigger - w.time, t.priority).hash(&mut h);
    }
    for (p, when) in &w.torch_toggles {
        if w.time - when <= 60 {
            (p, w.time - when).hash(&mut h);
        }
    }
    w.events.len().hash(&mut h);
    for (p, m) in w.piston.all() {
        (p, m.moved, m.direction, m.extending, m.source, m.progress, m.progress_o, w.time - m.last_ticked).hash(&mut h);
    }
    for e in &w.gravity.entities {
        format!("{e:?}").hash(&mut h);
    }
    h.finish()
}

/// The outcome of running a design.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The whole state repeats every `period` ticks (1 = static) once `transient` ticks passed.
    Cycle { period: u32, transient: u32 },
    /// No repeat within the horizon (a longer period, or not periodic).
    NoRepeat { ticks: u32 },
    /// Untrustworthy or illegal (see [`problem`]).
    Invalid(String),
}

/// Tick until the whole state repeats (at most `max_ticks`). On a cycle the world is left at the
/// repeat, i.e. at the phase of the cycle's start.
pub fn find_cycle(w: &mut World, max_ticks: u32) -> Verdict {
    let mut seen: HashMap<u64, u32> = HashMap::new();
    seen.insert(state_key(w), 0);
    for t in 1..=max_ticks {
        w.tick();
        if let Some(e) = problem(w) {
            return Verdict::Invalid(e);
        }
        let k = state_key(w);
        if let Some(&t0) = seen.get(&k) {
            return Verdict::Cycle { period: t - t0, transient: t0 };
        }
        seen.insert(k, t);
    }
    Verdict::NoRepeat { ticks: max_ticks }
}

/// One active cell over a period: what changes there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wave {
    pub pos: BlockPos,
    /// Its block id without `minecraft:`.
    pub name: String,
    /// The block state changes (not only its block-entity data).
    pub block_changes: bool,
    /// Its container contents change (items move).
    pub items_change: bool,
    /// Per tick of one period: the probe value ([`probe`]; a container: its item count).
    pub values: Vec<u8>,
}

/// A clock's behavior over one period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Character {
    pub period: u32,
    pub transient: u32,
    /// Every cell whose block state or block-entity data (container contents, comparator output)
    /// changes during a period.
    pub waves: Vec<Wave>,
    /// The family: the active set's block types, sorted, joined by `+` (a container counts when
    /// its contents change).
    pub family: String,
    /// Some block STATE changes (a redstone signal exists, not only moving items).
    pub signal: bool,
    /// The output cell (the `output` port, else the first active comparator, else the first
    /// cell whose block state changes) and its on/off waveform: on ticks, rising edges.
    pub output: Option<BlockPos>,
    pub on_ticks: u32,
    pub rising: u32,
}

/// A compact per-tick value of a cell: comparator output signal, dust power, 1/0 for powered or
/// lit components, enabled hoppers, triggered droppers, extended pistons; a container adds its
/// total item count in the upper bits when it has one ([`Character::waves`] keeps both apart by
/// cell kind).
pub fn probe(w: &World, p: BlockPos) -> u8 {
    let b = w.get(p);
    match b {
        Block::Comparator { .. } => *w.comparator_out.get(&p).unwrap_or(&0),
        Block::Wire { power, .. } => power,
        Block::Repeater { powered, .. } | Block::Observer { powered, .. } => powered as u8,
        Block::Torch { lit } | Block::WallTorch { lit, .. } | Block::Lamp { lit } => lit as u8,
        Block::Bulb { lit, .. } => lit as u8,
        Block::Note { powered, .. } => powered as u8,
        Block::Piston { extended, .. } => extended as u8,
        Block::Hopper { enabled, .. } => enabled as u8,
        Block::Dispenser { triggered, .. } => triggered as u8,
        Block::Crafter { triggered, .. } => triggered as u8,
        _ => 0,
    }
}

/// Total items in the container at `p` (0 without one).
pub fn item_count(w: &World, p: BlockPos) -> u32 {
    w.containers.map.get(&p).map(|c| c.slots.iter().flatten().map(|s| s.count as u32).sum()).unwrap_or(0)
}

/// Block id without the namespace and the properties.
pub fn short_name(b: &Block) -> String {
    let s = b.to_string();
    let s = s.strip_prefix("minecraft:").unwrap_or(&s);
    s.split('[').next().unwrap_or(s).to_string()
}

/// Replay one period from the cycle's start (the world as [`find_cycle`] left it) and describe
/// it. Errors if a cell outside the design changes or the model reports a problem.
pub fn characterize(w: &mut World, spec: &Spec, period: u32, transient: u32, output: Option<BlockPos>) -> Result<Character, String> {
    let cells: Vec<BlockPos> = spec.blocks.iter().map(|(p, _)| *p).collect();
    let design: std::collections::HashSet<BlockPos> = cells.iter().copied().collect();
    let n = cells.len();
    let mut blocks: Vec<Vec<Block>> = vec![Vec::new(); n];
    let mut values: Vec<Vec<u8>> = vec![Vec::new(); n];
    let mut slots: Vec<Vec<Option<Vec<Option<container::Stack>>>>> = vec![Vec::new(); n];
    for _ in 0..period {
        w.tick();
        if let Some(e) = problem(w) {
            return Err(e);
        }
        for y in w.min.y..w.min.y + w.size[1] {
            for z in w.min.z..w.min.z + w.size[2] {
                for x in w.min.x..w.min.x + w.size[0] {
                    let p = BlockPos::new(x, y, z);
                    if !design.contains(&p) && !w.get(p).is_air() {
                        return Err(format!("outside the design: {} at {p}", w.get(p)));
                    }
                }
            }
        }
        for (i, p) in cells.iter().enumerate() {
            blocks[i].push(w.get(*p));
            let mut v = probe(w, *p);
            if w.containers.map.contains_key(p) {
                v = item_count(w, *p).min(255) as u8;
            }
            values[i].push(v);
            slots[i].push(w.containers.map.get(p).map(|c| c.slots.clone()));
        }
    }
    let mut waves = Vec::new();
    for (i, p) in cells.iter().enumerate() {
        let block_changes = blocks[i].iter().any(|b| *b != blocks[i][0]);
        let items_change = slots[i].iter().any(|s| *s != slots[i][0]);
        let be_changes = items_change || values[i].iter().any(|v| *v != values[i][0]);
        if block_changes || be_changes {
            waves.push(Wave { pos: *p, name: short_name(&blocks[i][0]), block_changes, items_change, values: values[i].clone() });
        }
    }
    let mut names: Vec<String> = waves.iter().map(|w| w.name.clone()).collect();
    names.sort();
    names.dedup();
    let signal = waves.iter().any(|w| w.block_changes);
    let out = output
        .or_else(|| waves.iter().find(|w| w.name == "comparator" && w.block_changes).map(|w| w.pos))
        .or_else(|| waves.iter().find(|w| w.block_changes).map(|w| w.pos));
    let (mut on_ticks, mut rising) = (0, 0);
    if let Some(o) = out
        && let Some(i) = cells.iter().position(|p| *p == o)
    {
        let on: Vec<bool> = values[i].iter().map(|v| *v > 0).collect();
        on_ticks = on.iter().filter(|b| **b).count() as u32;
        for t in 0..on.len() {
            let prev = on[(t + on.len() - 1) % on.len()];
            if on[t] && !prev {
                rising += 1;
            }
        }
    }
    Ok(Character { period, transient, waves, family: names.join("+"), signal, output: out, on_ticks, rising })
}

/// Everything about one design: place, find the cycle, characterize it.
pub fn judge(spec: &Spec, max_ticks: u32, output: Option<BlockPos>) -> Result<Result<Character, Verdict>, String> {
    let mut w = new_world(spec);
    place(&mut w, spec)?;
    match find_cycle(&mut w, max_ticks) {
        Verdict::Cycle { period, transient } => Ok(Ok(characterize(&mut w, spec, period, transient, output)?)),
        v => Ok(Err(v)),
    }
}
