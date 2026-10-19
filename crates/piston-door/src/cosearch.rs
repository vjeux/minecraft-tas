//! `mctas door cosearch`: a pure-Rust door judge on redsim (crates/redsim, no JVM) and a joint
//! mechanism + wiring search on top of it (track cosearch).
//!
//! The judge here mirrors `crate::verify_in_spec` step by step (placement like
//! `video.rs::place`: raw writes, a shape update of every wire, a `self` update of every block;
//! settle; survival checks; closed at rest; the empty-walkway rule; the lever; 3 lever cycles at
//! every position with the open / restored / rest-state / no-entity checks), plus the two search
//! filters every new door needs: FULL seamless (`properties::seamless` on one recorded cycle,
//! door blocks tracked through every piston move) and the hallway check
//! (`properties_dyn::hallway_env` + `env_run`). The real judge stays final: every find is
//! confirmed with `door verify --extra-offsets 28`, `door properties`, `door properties-dyn`.
//!
//! `door cosearch agree` measures the agreement with the real judge (designs and random
//! mutants), `door cosearch eval` judges designs, `door cosearch search` is the joint search.

use crate::DoorSpec;
use crate::properties::Geo;
use anyhow::Result;
use mctas::design::Design;
use redsim::behavior as bh;
use redsim::block::Simple;
use redsim::world::flags;
use redsim::{Block, BlockPos, Dir, World};
use std::collections::{BTreeSet, HashMap, HashSet};

pub type Pos = [i32; 3];

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

const DIRS: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

// ---------------------------------------------------------------------- block facts

/// A design state as a redsim block. Quartz-like full blocks redsim does not list (quartz,
/// concrete, terracotta...) behave as stone (opaque, full, conductive, push-pull); only a note
/// block's instrument above one would differ.
pub fn convert_state(s: &str) -> std::result::Result<Block, String> {
    match Block::parse(s) {
        Ok(b) => Ok(b),
        Err(e) => {
            if crate::properties::quartz_like(s) {
                Ok(Block::Simple(Simple::Stone))
            } else {
                Err(e)
            }
        }
    }
}

/// `!getCollisionShape().isEmpty()` (the bridge's `collides` flag).
pub fn collides(b: &Block) -> bool {
    match *b {
        Block::Air | Block::Wire { .. } | Block::Torch { .. } | Block::WallTorch { .. } | Block::Lever { .. } | Block::Button { .. } | Block::Rail { .. } => false,
        Block::FenceGate { open, .. } => !open,
        // No block entity at BlockPos.ZERO: the bridge's flag is false (the judge counts a moving
        // block as blocking on its own).
        Block::MovingPiston { .. } => false,
        _ => true,
    }
}

/// `isCollisionShapeFullBlock` (the bridge's `full` flag).
pub fn full_cube(b: &Block) -> bool {
    match *b {
        Block::Simple(s) => match s {
            Simple::HoneyBlock | Simple::SoulSand => false,
            _ => true,
        },
        Block::Glazed { .. }
        | Block::RedstoneBlock
        | Block::Observer { .. }
        | Block::Lamp { .. }
        | Block::Bulb { .. }
        | Block::Note { .. }
        | Block::Target { .. }
        | Block::Dispenser { .. }
        | Block::Crafter { .. }
        | Block::Leaves { .. }
        | Block::Log { .. }
        | Block::RespawnAnchor { .. } => true,
        Block::Slab { kind } => kind == 2,
        Block::Piston { extended, .. } => !extended,
        _ => false,
    }
}

/// `crate::is_mechanism_block`: may not be a door block.
pub fn is_mechanism(b: &Block) -> bool {
    matches!(
        b,
        Block::Simple(Simple::SlimeBlock)
            | Block::Simple(Simple::HoneyBlock)
            | Block::Piston { .. }
            | Block::PistonHead { .. }
            | Block::MovingPiston { .. }
            | Block::Observer { .. }
            | Block::RedstoneBlock
            | Block::Lamp { .. }
            | Block::Target { .. }
            | Block::Repeater { .. }
            | Block::Comparator { .. }
            | Block::Wire { .. }
            | Block::Torch { .. }
            | Block::WallTorch { .. }
            | Block::Lever { .. }
            | Block::Dispenser { .. }
            | Block::Hopper { .. }
            | Block::Note { .. }
    )
}

pub fn is_moving(b: &Block) -> bool {
    matches!(b, Block::MovingPiston { .. })
}

/// `properties::quartz_like` on redsim blocks (quartz-likes redsim does not list are stone).
pub fn quartz_like(b: &Block) -> bool {
    matches!(
        b,
        Block::Simple(Simple::Stone)
            | Block::Simple(Simple::SmoothStone)
            | Block::Simple(Simple::Cobblestone)
            | Block::Simple(Simple::WhiteWool)
            | Block::Simple(Simple::OakPlanks)
            | Block::Simple(Simple::GoldBlock)
            | Block::Simple(Simple::IronBlock)
            | Block::Simple(Simple::EmeraldBlock)
            | Block::Log { .. }
    )
}

/// `isRandomlyTicking` among redsim's blocks (leaves: distance 7 and not persistent).
pub fn random_ticking(b: &Block) -> bool {
    matches!(b, Block::Leaves { persistent: false, distance: 7 })
}

// ---------------------------------------------------------------------- designs

/// A door as redsim blocks, in the judge's placement order (`Design::block_list`).
#[derive(Clone, Debug)]
pub struct RsDoor {
    pub spec: DoorSpec,
    pub blocks: Vec<(Pos, Block)>,
}

impl RsDoor {
    pub fn from_design(d: &Design) -> std::result::Result<RsDoor, String> {
        if !d.entities.is_empty() {
            return Err("entities are out of scope".into());
        }
        let spec = DoorSpec::from_design(d).map_err(|e| format!("{e:#}"))?;
        let illegal = crate::legality_problems(d);
        if !illegal.is_empty() {
            return Err(format!("illegal design: {}", illegal.join("; ")));
        }
        let mut blocks = Vec::new();
        for (p, s) in d.block_list() {
            let b = convert_state(&s).map_err(|e| format!("unsupported block {s} at {p:?}: {e}"))?;
            blocks.push((p, b));
        }
        Ok(RsDoor { spec, blocks })
    }

    pub fn to_design(&self, name: &str) -> Design {
        let mut d = Design { name: name.to_string(), ..Default::default() };
        d.ports.insert("door".into(), self.spec.door);
        d.ports.insert("lever".into(), self.spec.lever);
        if self.spec.along_z {
            d.meta.insert("door-axis".into(), "z".into());
        }
        for (p, b) in &self.blocks {
            d.blocks.insert(*p, b.to_string());
        }
        d
    }

    /// Placed blocks (piston heads excluded), as `crate::block_count`.
    pub fn block_count(&self) -> usize {
        self.blocks.iter().filter(|(_, b)| !matches!(b, Block::PistonHead { .. })).count()
    }
}

/// The judge's watch box (`crate::watch_box`): the blocks, the opening and its walkway, +2.
pub fn watch_box(blocks: &[(Pos, Block)], spec: &DoorSpec) -> (Pos, Pos) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    let cells = spec.door_cells();
    let corr = spec.corridor_cells();
    for p in blocks.iter().map(|(p, _)| p).chain(cells.iter()).chain(corr.iter()) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    ([lo[0] - 2, lo[1] - 2, lo[2] - 2], [hi[0] + 2, hi[1] + 2, hi[2] + 2])
}

/// Cells of the simulated world beyond the watch box (blocks pushed out of it stay modeled).
const MARGIN: i32 = 4;

fn world_for(lo: Pos, hi: Pos) -> World {
    let min = BlockPos::new(lo[0] - MARGIN, lo[1] - MARGIN, lo[2] - MARGIN);
    let size = [hi[0] - lo[0] + 1 + 2 * MARGIN, hi[1] - lo[1] + 1 + 2 * MARGIN, hi[2] - lo[2] + 1 + 2 * MARGIN];
    World::new(min, size)
}

/// A snapshot of the box `lo..=hi` (x fastest, then z, then y, like `Region`).
#[derive(Clone, PartialEq, Eq)]
pub struct Snap {
    pub lo: Pos,
    pub size: [i32; 3],
    pub cells: Vec<Block>,
}

impl Snap {
    pub fn take(w: &World, lo: Pos, hi: Pos) -> Snap {
        let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let mut cells = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    cells.push(w.get(BlockPos::new(x, y, z)));
                }
            }
        }
        Snap { lo, size, cells }
    }
    pub fn index(&self, p: Pos) -> Option<usize> {
        let x = p[0] - self.lo[0];
        let y = p[1] - self.lo[1];
        let z = p[2] - self.lo[2];
        if x < 0 || y < 0 || z < 0 || x >= self.size[0] || y >= self.size[1] || z >= self.size[2] {
            return None;
        }
        Some(((y * self.size[2] + z) * self.size[0] + x) as usize)
    }
    pub fn get(&self, p: Pos) -> Option<Block> {
        self.index(p).map(|i| self.cells[i])
    }
    pub fn iter(&self) -> impl Iterator<Item = (Pos, Block)> + '_ {
        let (lo, size) = (self.lo, self.size);
        self.cells.iter().enumerate().map(move |(i, b)| {
            let i = i as i32;
            let x = i % size[0];
            let z = (i / size[0]) % size[2];
            let y = i / (size[0] * size[2]);
            ([lo[0] + x, lo[1] + y, lo[2] + z], *b)
        })
    }
}

/// The bridge's `step` with `until_idle`: tick at least once, stop once idle, at most `max`.
/// `on_tick` sees the world after every tick. Returns (ticks run, idle reached).
pub fn step_until_idle(w: &mut World, max: u32, mut on_tick: impl FnMut(&World)) -> (u32, bool) {
    let mut n = 0;
    while n < max {
        w.tick();
        n += 1;
        on_tick(w);
        if w.idle() {
            return (n, true);
        }
    }
    (n, false)
}

fn st(b: &Block) -> String {
    b.to_string()
}

// ---------------------------------------------------------------------- the judge

/// Knobs of [`judge`] (the defaults are `door verify --extra-offsets 28`).
#[derive(Clone, Debug)]
pub struct JudgeOpts {
    pub cycles: u32,
    pub max_ticks: u32,
    pub offsets: Vec<Pos>,
    pub empty_walkway: bool,
}

impl JudgeOpts {
    pub fn with_extra(extra: u32) -> JudgeOpts {
        let mut offsets = crate::VerifyOptions::default().offsets;
        offsets.extend(crate::extra_offsets(extra));
        JudgeOpts { cycles: 3, max_ticks: 400, offsets, empty_walkway: true }
    }
}

/// Why a door fails. `Model` = redsim met something it does not model (a write outside its box,
/// an unmodeled case): the verdict is unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fail {
    Judge { offset: usize, cycle: Option<u32>, kind: &'static str, msg: String },
    Model(String),
}

impl std::fmt::Display for Fail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fail::Judge { offset, msg, .. } => write!(f, "at offset #{offset}: {msg}"),
            Fail::Model(m) => write!(f, "MODEL: {m}"),
        }
    }
}

/// (open ticks, close ticks) per cycle at the first position.
pub type Cycles = Vec<(u32, u32)>;

fn jfail(offset: usize, cycle: Option<u32>, kind: &'static str, msg: String) -> Fail {
    Fail::Judge { offset, cycle, kind, msg }
}

/// Door cells hold 9 identical full plain blocks; the walkway has no collision.
fn closed_problems(snap: &Snap, spec: &DoorSpec) -> Vec<String> {
    let mut out = Vec::new();
    let cells = spec.door_cells();
    let first = snap.get(cells[0]).unwrap_or(Block::Air);
    for p in &cells {
        let s = snap.get(*p).unwrap_or(Block::Air);
        if s != first {
            out.push(format!("door cell {p:?} is {}, but the door must be 9 identical blocks ({})", st(&s), st(&first)));
        }
        if is_mechanism(&s) {
            out.push(format!("door cell {p:?} is {}, not a plain door block", st(&s)));
        }
    }
    for p in &cells {
        let s = snap.get(*p).unwrap_or(Block::Air);
        if !full_cube(&s) || is_moving(&s) {
            out.push(format!("door cell {p:?} is {}", st(&s)));
        }
    }
    for p in spec.corridor_cells() {
        if let Some(s) = snap.get(p)
            && collides(&s)
        {
            out.push(format!("corridor cell {p:?} is blocked by {}", st(&s)));
        }
    }
    out
}

fn open_ok_world(w: &World, cells: &[BlockPos]) -> bool {
    cells.iter().all(|p| {
        let s = w.get(*p);
        !collides(&s) && !is_moving(&s)
    })
}

fn open_problems(w: &World, cells: &[Pos]) -> Vec<String> {
    cells
        .iter()
        .filter_map(|p| {
            let s = w.get(bp(*p));
            (collides(&s) || is_moving(&s)).then(|| format!("{p:?} is {}", st(&s)))
        })
        .collect()
}

fn settled_from_flags(ok: &[bool]) -> Option<u32> {
    let mut first = None;
    for (i, &b) in ok.iter().enumerate() {
        if b {
            if first.is_none() {
                first = Some(i as u32 + 1);
            }
        } else {
            first = None;
        }
    }
    first
}

fn model_errors(w: &World) -> Option<Fail> {
    (!w.errors.is_empty()).then(|| Fail::Model(w.errors.iter().take(3).cloned().collect::<Vec<_>>().join("; ")))
}

/// Place like the judge and settle (the world and the watch box).
fn place_and_settle(blocks: &[(Pos, Block)], spec: &DoorSpec, max: u32) -> (World, (Pos, Pos), bool) {
    let (wlo, whi) = watch_box(blocks, spec);
    let mut w = world_for(wlo, whi);
    let list: Vec<(BlockPos, Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
    w.place_like_judge(&list);
    let (_, idle) = step_until_idle(&mut w, max, |_| {});
    (w, (wlo, whi), idle)
}

/// The judge at one position (the door already translated): `crate::verify_at`.
fn judge_at(blocks: &[(Pos, Block)], spec: &DoorSpec, opts: &JudgeOpts, oi: usize) -> std::result::Result<Cycles, Fail> {
    let (mut w, (wlo, whi), idle) = place_and_settle(blocks, spec, opts.max_ticks);
    if let Some(e) = model_errors(&w) {
        return Err(e);
    }
    if !idle {
        return Err(jfail(oi, None, "settle", format!("the build never settles after placement ({} ticks)", opts.max_ticks)));
    }
    let rest = Snap::take(&w, wlo, whi);
    let mut floating = Vec::new();
    let mut ticking = Vec::new();
    for (p, s) in rest.iter() {
        if s.is_air() {
            continue;
        }
        if !bh::can_survive(&w, &s, bp(p)) {
            floating.push(format!("{p:?} {}", st(&s)));
        }
        if random_ticking(&s) {
            ticking.push(format!("{p:?} {}", st(&s)));
        }
    }
    if !floating.is_empty() {
        return Err(jfail(oi, None, "unsupported", format!("unsupported blocks (they could not stand there in survival): {}", floating.join("; "))));
    }
    if !ticking.is_empty() {
        return Err(jfail(oi, None, "ticking", format!("blocks that change by themselves over time: {}", ticking.join("; "))));
    }
    let problems = closed_problems(&rest, spec);
    if !problems.is_empty() {
        return Err(jfail(oi, None, "closed", format!("not closed at rest: {}", problems.join("; "))));
    }
    if opts.empty_walkway {
        let reach = (whi[0] - wlo[0]).max(whi[2] - wlo[2]) + 1;
        let clutter: Vec<String> = spec
            .in_front_and_behind(reach)
            .iter()
            .filter_map(|p| rest.get(*p).filter(|s| !s.is_air()).map(|s| format!("{p:?} {}", st(&s))))
            .collect();
        if !clutter.is_empty() {
            return Err(jfail(oi, None, "walkway", format!("blocks in front of or behind the door: {}", clutter.join("; "))));
        }
    }
    match rest.get(spec.lever) {
        // Either rest polarity counts (vjeux, 2026-10-19), like `crate::verify`.
        Some(Block::Lever { .. }) => {}
        other => return Err(jfail(oi, None, "lever", format!("port lever {:?} is {}, expected a lever", spec.lever, other.map(|b| st(&b)).unwrap_or("?".into())))),
    }
    let door_cells = spec.door_cells();
    let closed_door: Vec<Block> = door_cells.iter().map(|p| rest.get(*p).unwrap()).collect();
    let mut walk: Vec<Pos> = door_cells.clone();
    walk.extend(spec.corridor_cells());
    let walk_bp: Vec<BlockPos> = walk.iter().map(|p| bp(*p)).collect();
    let door_bp: Vec<BlockPos> = door_cells.iter().map(|p| bp(*p)).collect();
    let lever = bp(spec.lever);
    let mut cycles = Vec::new();
    for cycle in 0..opts.cycles {
        w.use_block(lever);
        let mut ok = Vec::new();
        let (n, idle) = step_until_idle(&mut w, opts.max_ticks, |w| ok.push(open_ok_world(w, &walk_bp)));
        if let Some(e) = model_errors(&w) {
            return Err(e);
        }
        if !idle {
            return Err(jfail(oi, Some(cycle), "moving-open", format!("cycle {cycle}: still moving {n} ticks after opening")));
        }
        let p = open_problems(&w, &walk);
        if !p.is_empty() {
            return Err(jfail(oi, Some(cycle), "not-open", format!("cycle {cycle}: not open after the lever was switched on: {}", p.join("; "))));
        }
        let open_ticks = settled_from_flags(&ok).unwrap_or(0);
        if !w.drops.is_empty() {
            return Err(jfail(oi, Some(cycle), "entities-open", format!("cycle {cycle}: entities appeared while opening (something broke): {:?}", w.drops)));
        }
        w.use_block(lever);
        let mut ok = Vec::new();
        let (n, idle) = step_until_idle(&mut w, opts.max_ticks, |w| ok.push(door_bp.iter().zip(closed_door.iter()).all(|(p, s)| w.get(*p) == *s)));
        if let Some(e) = model_errors(&w) {
            return Err(e);
        }
        if !idle {
            return Err(jfail(oi, Some(cycle), "moving-close", format!("cycle {cycle}: still moving {n} ticks after closing")));
        }
        let now: Vec<Block> = door_bp.iter().map(|p| w.get(*p)).collect();
        if now != closed_door {
            let bad: Vec<String> = door_cells
                .iter()
                .zip(now.iter().zip(closed_door.iter()))
                .filter(|(_, (a, b))| a != b)
                .map(|(p, (a, b))| format!("{p:?} is {}, was {}", st(a), st(b)))
                .collect();
            return Err(jfail(oi, Some(cycle), "not-restored", format!("cycle {cycle}: door not restored: {}", bad.join("; "))));
        }
        let snap = Snap::take(&w, wlo, whi);
        if snap != rest {
            let diffs: Vec<String> = rest
                .iter()
                .filter(|(p, s)| snap.get(*p) != Some(*s))
                .take(8)
                .map(|(p, s)| format!("{p:?} {} -> {}", st(&s), snap.get(p).map(|b| st(&b)).unwrap_or("?".into())))
                .collect();
            return Err(jfail(oi, Some(cycle), "rest-state", format!("cycle {cycle}: the build did not return to its rest state: {}", diffs.join("; "))));
        }
        if !w.drops.is_empty() {
            return Err(jfail(oi, Some(cycle), "entities-close", format!("cycle {cycle}: entities appeared while closing (something broke)")));
        }
        let close_ticks = settled_from_flags(&ok).unwrap_or(0);
        cycles.push((open_ticks, close_ticks));
    }
    Ok(cycles)
}

/// The judge (`crate::verify` with `opts`): Ok(cycles at the first position) or the first failure.
pub fn judge(door: &RsDoor, opts: &JudgeOpts) -> std::result::Result<Cycles, Fail> {
    let mut first = None;
    for (i, off) in opts.offsets.iter().enumerate() {
        let blocks: Vec<(Pos, Block)> = door.blocks.iter().map(|(p, b)| (add(*p, *off), *b)).collect();
        let spec = door.spec.translated(*off);
        let c = judge_at(&blocks, &spec, opts, i)?;
        if i == 0 {
            first = Some(c);
        }
    }
    Ok(first.unwrap_or_default())
}

/// The judge's failure kind from one of its messages (for agreement statistics): (offset index,
/// cycle, kind).
pub fn classify_judge_error(msg: &str, offsets: &[Pos]) -> (usize, Option<u32>, &'static str) {
    let mut oi = 0;
    if let Some(r) = msg.strip_prefix("at offset [") {
        let inner = r.split(']').next().unwrap_or("");
        let v: Vec<i32> = inner.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if v.len() == 3 {
            oi = offsets.iter().position(|o| o[0] == v[0] && o[1] == v[1] && o[2] == v[2]).unwrap_or(usize::MAX);
        }
    }
    let cycle = msg.find("cycle ").and_then(|i| msg[i + 6..].split(':').next()).and_then(|t| t.trim().parse::<u32>().ok());
    let kind = if msg.contains("illegal design") {
        "illegal"
    } else if msg.contains("never settles after placement") {
        "settle"
    } else if msg.contains("unsupported blocks") {
        "unsupported"
    } else if msg.contains("change by themselves") {
        "ticking"
    } else if msg.contains("not closed at rest") {
        "closed"
    } else if msg.contains("in front of or behind the door") {
        "walkway"
    } else if msg.contains("port lever") {
        "lever"
    } else if msg.contains("after opening") {
        "moving-open"
    } else if msg.contains("not open after") {
        "not-open"
    } else if msg.contains("appeared while opening") {
        "entities-open"
    } else if msg.contains("after closing") {
        "moving-close"
    } else if msg.contains("door not restored") {
        "not-restored"
    } else if msg.contains("did not return to its rest state") {
        "rest-state"
    } else if msg.contains("appeared while closing") {
        "entities-close"
    } else {
        "other"
    };
    (oi, if kind == "settle" || kind == "unsupported" || kind == "ticking" || kind == "closed" || kind == "walkway" || kind == "lever" { None } else { cycle }, kind)
}

// ---------------------------------------------------------------------- one recorded cycle

/// One frame of a recording: the watch box and its moving blocks.
#[derive(Clone)]
pub struct Frame {
    pub snap: Snap,
    pub moving: Vec<(Pos, redsim::piston::MovingBe)>,
}

impl Frame {
    fn take(w: &World, lo: Pos, hi: Pos) -> Frame {
        let snap = Snap::take(w, lo, hi);
        let moving = w.piston.all().map(|(p, m)| (p.arr(), *m)).filter(|(p, _)| snap.index(*p).is_some()).collect();
        Frame { snap, moving }
    }
}

/// `properties::Recording` on redsim: one open/close cycle at the design's own position.
pub struct Rec {
    pub geo: Geo,
    pub lever: Pos,
    pub lo: Pos,
    pub hi: Pos,
    pub rest: Snap,
    pub open: Vec<Frame>,
    pub close: Vec<Frame>,
    pub door_state: Block,
    pub track_open: Track,
    pub track_close: Track,
}

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub pos: Vec<Vec<Option<Pos>>>,
    pub lost: usize,
}

fn motion(m: &redsim::piston::MovingBe) -> Pos {
    m.push_direction().step()
}

fn holds(f: &Frame, moving: &HashMap<Pos, redsim::piston::MovingBe>, p: Pos, state: Block) -> bool {
    match f.snap.get(p) {
        Some(s) if s == state => true,
        Some(s) if is_moving(&s) => moving.get(&p).is_some_and(|m| m.moved == state),
        _ => false,
    }
}

/// `properties::track` on redsim frames.
fn track(start: &[Pos], before: &Snap, frames: &[Frame], door_state: Block) -> Track {
    let n = start.len();
    let mut t = Track::default();
    let mut paths: Vec<Vec<Pos>> = start.iter().map(|p| vec![*p]).collect();
    let mut cur: Vec<Option<Pos>> = start.iter().map(|p| Some(*p)).collect();
    let mut prev_keys: HashSet<(Pos, Dir, bool)> = HashSet::new();
    let mut prev_snap = before;
    let static_same: HashSet<Pos> = before.iter().filter(|(q, s)| *s == door_state && !start.contains(q)).map(|(q, _)| q).collect();
    for f in frames {
        let mut moves: HashMap<Pos, Pos> = HashMap::new();
        let mut keys = HashSet::new();
        for (p, m) in f.moving.iter().filter(|(_, m)| !m.source) {
            let key = (*p, m.direction, m.extending);
            if !prev_keys.contains(&key) {
                let mv = motion(m);
                moves.insert([p[0] - mv[0], p[1] - mv[1], p[2] - mv[2]], *p);
            }
            keys.insert(key);
        }
        prev_keys = keys;
        let moving_at: HashMap<Pos, redsim::piston::MovingBe> = f.moving.iter().map(|(p, m)| (*p, *m)).collect();
        for (i, c) in cur.iter_mut().enumerate() {
            if let Some(p) = *c
                && let Some(&q) = moves.get(&p)
            {
                *c = Some(q);
                paths[i].push(q);
            }
        }
        for i in 0..n {
            let Some(p) = cur[i] else { continue };
            if holds(f, &moving_at, p, door_state) {
                continue;
            }
            let claimed: HashSet<Pos> = cur.iter().flatten().copied().collect();
            let mut found = None;
            'search: for d in DIRS {
                for k in 1..=2 {
                    let q = [p[0] + d[0] * k, p[1] + d[1] * k, p[2] + d[2] * k];
                    if !claimed.contains(&q) && holds(f, &moving_at, q, door_state) && prev_snap.get(q) != Some(door_state) {
                        found = Some(q);
                        break 'search;
                    }
                }
            }
            match found {
                Some(q) => {
                    cur[i] = Some(q);
                    paths[i].push(q);
                }
                None => {
                    cur[i] = None;
                    t.lost += 1;
                }
            }
        }
        if cur.iter().any(|c| c.is_none()) {
            let claimed: HashSet<Pos> = cur.iter().flatten().copied().collect();
            let mut cands: Vec<Pos> =
                f.snap.iter().filter(|(q, _)| holds(f, &moving_at, *q, door_state) && !claimed.contains(q) && !static_same.contains(q)).map(|(q, _)| q).collect();
            for i in 0..n {
                if cur[i].is_some() || cands.is_empty() {
                    continue;
                }
                let last = *paths[i].last().unwrap();
                let dist = |q: &Pos| (0..3).map(|k| (q[k] - last[k]).abs()).sum::<i32>();
                let k = (0..cands.len()).min_by_key(|k| dist(&cands[*k])).unwrap();
                let q = cands.remove(k);
                cur[i] = Some(q);
                paths[i].push(q);
                t.lost -= 1;
            }
        }
        t.pos.push(cur.clone());
        prev_snap = &f.snap;
    }
    t
}

fn record_until_idle(w: &mut World, lo: Pos, hi: Pos, max: u32) -> std::result::Result<Vec<Frame>, String> {
    let mut out = Vec::new();
    for _ in 0..max {
        w.tick();
        out.push(Frame::take(w, lo, hi));
        if w.idle() {
            return Ok(out);
        }
    }
    Err(format!("still moving {max} ticks after the flip"))
}

/// `properties::record` on redsim (the door at its own position).
/// The settled watch-box snapshot after each of `flips` lever flips (index 0 = settled rest), with
/// the door translated by `offset` (a judge position). Err = never settles / a model error.
pub fn flip_states(door: &RsDoor, offset: Pos, flips: usize) -> std::result::Result<Vec<Snap>, String> {
    let blocks: Vec<(Pos, Block)> = door.blocks.iter().map(|(p, b)| (add(*p, offset), *b)).collect();
    let spec = door.spec.translated(offset);
    let (mut w, (lo, hi), idle) = place_and_settle(&blocks, &spec, 400);
    if !idle {
        return Err("the build never settles after placement".into());
    }
    let mut out = vec![Snap::take(&w, lo, hi)];
    for i in 0..flips {
        w.use_block(bp(spec.lever));
        record_until_idle(&mut w, lo, hi, 400).map_err(|e| format!("flip {}: {e}", i + 1))?;
        if !w.errors.is_empty() {
            return Err(format!("MODEL: {}", w.errors.join("; ")));
        }
        if !w.drops.is_empty() {
            return Err(format!("flip {}: entities appeared (something broke): {:?}", i + 1, w.drops));
        }
        out.push(Snap::take(&w, lo, hi));
    }
    Ok(out)
}

/// `properties::record` on redsim (the door at its own position).
pub fn record(door: &RsDoor) -> std::result::Result<Rec, String> {
    let spec = door.spec;
    let geo = Geo::new(spec);
    let (mut w, (lo, hi), idle) = place_and_settle(&door.blocks, &spec, 400);
    if !idle {
        return Err("the build never settles after placement".into());
    }
    let rest = Snap::take(&w, lo, hi);
    let cells = spec.door_cells();
    let door_state = rest.get(cells[0]).ok_or("door cell outside the box")?;
    w.use_block(bp(spec.lever));
    let open = record_until_idle(&mut w, lo, hi, 400)?;
    w.use_block(bp(spec.lever));
    let close = record_until_idle(&mut w, lo, hi, 400)?;
    if !w.errors.is_empty() {
        return Err(format!("MODEL: {}", w.errors.join("; ")));
    }
    let track_open = track(&cells, &rest, &open, door_state);
    let open_last = track_open.pos.last().cloned().unwrap_or_default();
    let start_close: Vec<Pos> = open_last.iter().zip(cells.iter()).map(|(p, c)| p.unwrap_or(*c)).collect();
    let track_close = track(&start_close, &open.last().unwrap().snap, &close, door_state);
    Ok(Rec { geo, lever: spec.lever, lo, hi, rest, open, close, door_state, track_open, track_close })
}

impl Rec {
    pub fn open_state(&self) -> &Snap {
        &self.open.last().unwrap().snap
    }
    fn open_doors(&self) -> HashSet<Pos> {
        self.track_open.pos.last().map(|v| v.iter().flatten().copied().collect()).unwrap_or_default()
    }
}

/// `properties::circuitry`.
fn circuitry(geo: &Geo, f: &Snap, doors: &HashSet<Pos>, lever: Pos) -> Vec<(Pos, Block)> {
    f.iter().filter(|(p, s)| !s.is_air() && !doors.contains(p) && *p != lever && !(geo.in_s(*p) && !is_moving(s) && quartz_like(s))).collect()
}

/// FULL seamless (`properties::seamless` reaching FULL or better) of a recorded cycle: Ok or the
/// first blocker.
pub fn seamless_full(rec: &Rec) -> std::result::Result<(), String> {
    let geo = rec.geo;
    let lever = rec.lever;
    let rest_doors: HashSet<Pos> = geo.spec.door_cells().into_iter().collect();
    let open_doors = rec.open_doors();
    let circ_r = circuitry(&geo, &rec.rest, &rest_doors, lever);
    let circ_o = circuitry(&geo, rec.open_state(), &open_doors, lever);
    let vis = |p: Pos| geo.in_t(p) || geo.in_s(p);
    let mut swept_s: BTreeSet<Pos> = BTreeSet::new();
    for f in rec.open.iter().chain(rec.close.iter()) {
        for (q, s) in f.snap.iter() {
            if geo.in_s(q) && !s.is_air() {
                swept_s.insert(q);
            }
        }
    }
    let holes_in = |f: &Snap, closed: bool| -> Vec<Pos> { swept_s.iter().copied().filter(|q| f.get(*q).is_none_or(|b| b.is_air()) && !(closed && geo.depth(*q) == 0)).collect() };
    let holes_r = holes_in(&rec.rest, true);
    let holes_o = holes_in(rec.open_state(), false);
    let outward = |h: Pos| -> Pos {
        for d in DIRS {
            if geo.in_t(add(h, d)) {
                return [h[0] - d[0], h[1] - d[1], h[2] - d[2]];
            }
        }
        h
    };
    let by_r: HashMap<Pos, Block> = circ_r.iter().copied().collect();
    let by_o: HashMap<Pos, Block> = circ_o.iter().copied().collect();
    let through_r: Vec<(Pos, Block)> = holes_r.iter().filter_map(|h| by_r.get(&outward(*h)).map(|b| (outward(*h), *b))).collect();
    let through_o: Vec<(Pos, Block)> = holes_o.iter().filter_map(|h| by_o.get(&outward(*h)).map(|b| (outward(*h), *b))).collect();
    for s in [-1, 1] {
        if let Some((p, b)) = circ_r.iter().filter(|c| vis(c.0) && geo.depth(c.0) * s > 0).chain(through_r.iter().filter(|c| geo.depth(c.0) * s > 0)).next() {
            return Err(format!("closed: {} at {p:?}", st(b)));
        }
    }
    if let Some((p, b)) = circ_o.iter().filter(|c| vis(c.0)).chain(through_o.iter()).next() {
        return Err(format!("open: {} at {p:?}", st(b)));
    }
    Ok(())
}

/// The community volume (`properties::cheap`'s `volume`): (volume, [width, height, depth]).
pub fn volume(rec: &Rec) -> Option<(i32, [i32; 3])> {
    let geo = rec.geo;
    let lever = rec.lever;
    let rest_doors: HashSet<Pos> = geo.spec.door_cells().into_iter().collect();
    let open_doors = rec.open_doors();
    let mut cells: Vec<Pos> = circuitry(&geo, &rec.rest, &rest_doors, lever).into_iter().map(|c| c.0).filter(|q| !geo.in_t(*q)).collect();
    for (frames, tr) in [(&rec.open, &rec.track_open), (&rec.close, &rec.track_close)] {
        for (i, f) in frames.iter().enumerate() {
            let doors: HashSet<Pos> = tr.pos[i].iter().flatten().copied().collect();
            cells.extend(circuitry(&geo, &f.snap, &doors, lever).into_iter().map(|c| c.0).filter(|q| !geo.in_t(*q)));
        }
    }
    cells.extend(circuitry(&geo, rec.open_state(), &open_doors, lever).into_iter().map(|c| c.0).filter(|q| !geo.in_frame(*q)));
    if cells.is_empty() {
        return None;
    }
    let mut lo = cells[0];
    let mut hi = cells[0];
    for q in &cells {
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    let (wa, a) = if geo.spec.along_z { (2, 0) } else { (0, 2) };
    let dims = [hi[wa] - lo[wa] + 1, hi[1] - lo[1] + 1, hi[a] - lo[a] + 1];
    Some((dims[0] * dims[1] * dims[2], dims))
}

// ---------------------------------------------------------------------- the hallway check

/// Every cell the build holds at rest or at any tick of one lever cycle (`properties_dyn::swept_cells`).
pub fn swept_cells(door: &RsDoor) -> std::result::Result<HashSet<Pos>, String> {
    let spec = door.spec;
    let (mut w, (lo, hi), idle) = place_and_settle(&door.blocks, &spec, 400);
    if !idle {
        return Err("never settles after placement".into());
    }
    let mut cells: HashSet<Pos> = door.blocks.iter().map(|(p, _)| *p).collect();
    let mut add_snap = |s: &Snap| {
        for (p, b) in s.iter() {
            if !b.is_air() {
                cells.insert(p);
            }
        }
    };
    add_snap(&Snap::take(&w, lo, hi));
    for _ in 0..2 {
        w.use_block(bp(spec.lever));
        let mut snaps = Vec::new();
        step_until_idle(&mut w, 400, |w| snaps.push(Snap::take(w, lo, hi)));
        for s in &snaps {
            add_snap(s);
        }
    }
    Ok(cells)
}

/// `properties_dyn::hallway_env`: smooth stone in every ring cell over the build's depth extent
/// + 3 that the build never holds.
pub fn hallway_env(door: &RsDoor, swept: &HashSet<Pos>) -> Vec<(Pos, Block)> {
    let spec = door.spec;
    let (wa, da) = if spec.along_z { (2, 0) } else { (0, 2) };
    let mut lo = spec.door;
    let mut hi = spec.door;
    if let Some((p0, _)) = door.blocks.first() {
        lo = *p0;
        hi = *p0;
        for (p, _) in &door.blocks {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    let have: HashSet<Pos> = door.blocks.iter().map(|(p, _)| *p).collect();
    let at = |wv: i32, y: i32, t: i32| {
        let mut p = [0; 3];
        p[wa] = wv;
        p[1] = y;
        p[da] = t;
        p
    };
    let mut out = Vec::new();
    for t in (lo[da].min(spec.door[da]) - 3)..=(hi[da].max(spec.door[da]) + 3) {
        let mut ring = Vec::new();
        for y in spec.door[1]..spec.door[1] + 3 {
            ring.push(at(spec.door[wa] - 1, y, t));
            ring.push(at(spec.door[wa] + 3, y, t));
        }
        for wv in spec.door[wa]..spec.door[wa] + 3 {
            ring.push(at(wv, spec.door[1] - 1, t));
            ring.push(at(wv, spec.door[1] + 3, t));
        }
        for p in ring {
            if !have.contains(&p) && !swept.contains(&p) {
                out.push((p, Block::Simple(Simple::SmoothStone)));
            }
        }
    }
    out
}

/// `properties_dyn::env_run` with the hallway, one lever cycle: Ok or why it breaks.
pub fn hallway_check(door: &RsDoor) -> std::result::Result<(), String> {
    let spec = door.spec;
    let swept = swept_cells(door)?;
    let env = hallway_env(door, &swept);
    let (mut wlo, mut whi) = watch_box(&door.blocks, &spec);
    for (p, _) in &env {
        for i in 0..3 {
            wlo[i] = wlo[i].min(p[i] - 1);
            whi[i] = whi[i].max(p[i] + 1);
        }
    }
    let mut w = world_for(wlo, whi);
    // properties_dyn::place: raw writes (design, then the hallway), design wires' shapes, the
    // shape of design blocks that follow their neighbours next to the hallway (note blocks), a
    // self update of every design block, settle.
    for (p, b) in door.blocks.iter().chain(env.iter()) {
        w.set_block(bp(*p), *b, flags::RAW);
    }
    for (p, b) in &door.blocks {
        if matches!(b, Block::Wire { .. }) {
            let q = bp(*p);
            let cur = w.get(q);
            let mut s2 = cur;
            for d in Dir::SHAPE_ORDER {
                let np = q.rel(d);
                let ns = w.get(np);
                s2 = bh::update_shape(&mut w, s2, q, d, np, ns);
            }
            if s2 != cur {
                w.set_block(q, s2, flags::RAW);
            }
        }
    }
    if !env.is_empty() {
        let envs: HashSet<Pos> = env.iter().map(|(p, _)| *p).collect();
        for (p, b) in &door.blocks {
            if matches!(b, Block::Note { .. }) && DIRS.iter().any(|o| envs.contains(&add(*p, *o))) {
                let q = bp(*p);
                let cur = w.get(q);
                let up = bh::update_from_neighbour_shapes(&mut w, cur, q);
                if up != cur {
                    w.set_block(q, up, flags::RAW);
                }
            }
        }
    }
    for (p, _) in &door.blocks {
        let q = bp(*p);
        let k = w.get(q).kind();
        w.neighbor_changed(q, k);
    }
    let (n, idle) = step_until_idle(&mut w, 400, |_| {});
    if !idle {
        return Err(format!("never settles after placement ({n} ticks)"));
    }
    let rest = Snap::take(&w, wlo, whi);
    let mine: HashSet<Pos> = door.blocks.iter().map(|(p, _)| *p).collect();
    let fl: Vec<String> = rest.iter().filter(|(p, s)| mine.contains(p) && !s.is_air() && !bh::can_survive(&w, s, bp(*p))).map(|(p, s)| format!("{p:?} {}", st(&s))).collect();
    if !fl.is_empty() {
        return Err(format!("unsupported blocks: {}", fl.join("; ")));
    }
    let problems = closed_problems(&rest, &spec);
    if !problems.is_empty() {
        return Err(format!("not closed at rest: {}", problems.join("; ")));
    }
    if !matches!(rest.get(spec.lever), Some(Block::Lever { powered: false, .. })) {
        return Err("lever".into());
    }
    if !w.drops.is_empty() {
        return Err("an entity appeared at rest".into());
    }
    let cells = spec.door_cells();
    let closed_door: Vec<Block> = cells.iter().map(|p| rest.get(*p).unwrap()).collect();
    let mut walk: Vec<Pos> = cells.clone();
    walk.extend(spec.corridor_cells());
    w.use_block(bp(spec.lever));
    let (n, idle) = step_until_idle(&mut w, 400, |_| {});
    if !idle {
        return Err(format!("still moving {n} ticks after opening"));
    }
    let p = open_problems(&w, &walk);
    if !p.is_empty() {
        return Err(format!("not open: {}", p.join("; ")));
    }
    if !w.drops.is_empty() {
        return Err("an entity appeared while opening".into());
    }
    w.use_block(bp(spec.lever));
    let (n, idle) = step_until_idle(&mut w, 400, |_| {});
    if !idle {
        return Err(format!("still moving {n} ticks after closing"));
    }
    let now: Vec<Block> = cells.iter().map(|p| w.get(bp(*p))).collect();
    if now != closed_door {
        return Err("door not restored".into());
    }
    let snap = Snap::take(&w, wlo, whi);
    if snap != rest {
        let d: Vec<String> = rest.iter().filter(|(p, s)| snap.get(*p) != Some(*s)).take(4).map(|(p, s)| format!("{p:?} {} -> {}", st(&s), snap.get(p).map(|b| st(&b)).unwrap_or("?".into()))).collect();
        return Err(format!("not back to its rest state: {}", d.join("; ")));
    }
    if !w.drops.is_empty() {
        return Err("an entity appeared while closing".into());
    }
    if !w.errors.is_empty() {
        return Err(format!("MODEL: {}", w.errors.join("; ")));
    }
    Ok(())
}

// ---------------------------------------------------------------------- full evaluation

/// Everything `door cosearch eval` reports for one door.
#[derive(Clone, Debug)]
pub struct Eval {
    pub judge: std::result::Result<Cycles, Fail>,
    /// FULL seamless (None: not measured, the judge failed).
    pub full: Option<std::result::Result<(), String>>,
    pub hallway: Option<std::result::Result<(), String>>,
    pub volume: Option<(i32, [i32; 3])>,
    pub blocks: usize,
}

impl Eval {
    pub fn record_ok(&self) -> bool {
        self.judge.is_ok() && matches!(self.full, Some(Ok(()))) && matches!(self.hallway, Some(Ok(())))
    }
}

pub fn evaluate(door: &RsDoor, opts: &JudgeOpts, props_even_if_broken: bool) -> Eval {
    let judge = judge(door, opts);
    let mut e = Eval { judge, full: None, hallway: None, volume: None, blocks: door.block_count() };
    if e.judge.is_ok() || props_even_if_broken {
        match record(door) {
            Ok(rec) => {
                e.full = Some(seamless_full(&rec));
                e.volume = volume(&rec);
            }
            Err(why) => e.full = Some(Err(why)),
        }
        e.hallway = Some(hallway_check(door));
    }
    e
}

pub fn eval_line(name: &str, e: &Eval, ms: f64) -> String {
    let j = match &e.judge {
        Ok(c) => {
            let o = c.iter().map(|x| x.0).max().unwrap_or(0);
            let cl = c.iter().map(|x| x.1).max().unwrap_or(0);
            format!("WORKS open {o} close {cl}")
        }
        Err(f) => format!("FAILS {f}"),
    };
    let full = match &e.full {
        None => "-".to_string(),
        Some(Ok(())) => "FULL".to_string(),
        Some(Err(w)) => format!("not FULL ({w})"),
    };
    let hall = match &e.hallway {
        None => "-".to_string(),
        Some(Ok(())) => "hallway ok".to_string(),
        Some(Err(w)) => format!("hallway breaks it ({w})"),
    };
    let vol = e.volume.map(|(v, d)| format!("{v}b ({}x{}x{})", d[0], d[1], d[2])).unwrap_or("-".into());
    format!("{name}: {j} | {full} | {hall} | {vol}, {} blocks | {ms:.1} ms", e.blocks)
}

/// Load a design file (or `@builtin`) as a redsim door.
pub fn load_door(path: &std::path::Path) -> Result<(Design, RsDoor)> {
    let d = crate::cli::load_design(path)?;
    let r = RsDoor::from_design(&d).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
    Ok((d, r))
}

/// Run `f` over `items` on `threads` threads (results in input order).
pub fn par_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = items.len();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<Option<R>>> = std::sync::Mutex::new((0..n).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1).min(n.max(1)) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let r = f(&items[i]);
                    out.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|r| r.unwrap()).collect()
}

pub fn context_note() -> &'static str {
    "redsim judge (crates/redsim): a filter; the real judge (`door verify --extra-offsets 28`) is final"
}

// ---------------------------------------------------------------------- CLI

#[derive(clap::Subcommand, Clone)]
pub enum CosearchCmd {
    /// Judge designs in redsim (no JVM): the judge's checks at 4 + `--extra-offsets` positions
    /// x 3 cycles, then FULL seamless, the hallway check and the volume. One line per design.
    Eval {
        designs: Vec<std::path::PathBuf>,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 8)]
        threads: usize,
        /// Also measure FULL / hallway / volume when the judge fails.
        #[arg(long)]
        props_always: bool,
        /// Evaluate each design this many times (speed measurement: evaluations per second).
        #[arg(long, default_value_t = 1)]
        repeat: usize,
    },
    /// Agreement with the real judge: every design given (and `--mutants` random near-miss
    /// edits of each) judged by redsim and by the JVM (`verify --extra-offsets N`, the seamless
    /// tier, the hallway check); prints the agreement rate and writes every disagreement as a
    /// design file (+ both verdicts) to `--out`.
    Agree {
        designs: Vec<std::path::PathBuf>,
        /// Random mutants per design (1-2 edits: remove, replace, re-face, delay, move, add).
        #[arg(long, default_value_t = 0)]
        mutants: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Only designs whose blocks all lie in the door plane.
        #[arg(long)]
        flat_only: bool,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        /// Worker JVMs (capped by MCTAS_MAX_WORKERS).
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Threads for redsim.
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/cosearch/disagree")]
        out: std::path::PathBuf,
        /// Skip the FULL seamless and hallway comparison (judge only).
        #[arg(long)]
        judge_only: bool,
    },
    /// The joint mechanism + wiring search (cosearch_search.rs).
    Search(crate::cosearch_search::SearchArgs),
    /// Pocket-exhaustive wiring (cosearch_search.rs `pockets_run`): every filling of up to `--k`
    /// blocks of each free pocket, simulated with the rest of the door driven by the plan, kept
    /// when it drives plan inputs inside their timing windows; then one filling per pocket
    /// combined and verified.
    Pockets(crate::cosearch_search::PocketArgs),
    /// Print every block change tick by tick in redsim (placement, then `--flips` lever flips):
    /// compare with `door trace` (the real game) to locate a model disagreement.
    Trace {
        design: std::path::PathBuf,
        /// Judge position index (0 = the design as written; 4.. = the extra offsets).
        #[arg(long, default_value_t = 0)]
        offset: usize,
        #[arg(long, default_value_t = 2)]
        flips: u32,
        /// Only these cells (x,y,z;x,y,z...), default every cell.
        #[arg(long)]
        cells: Option<String>,
        /// Apply a `door ideal` plan's inputs like `search --assist` (odd flips: the open edge's
        /// events, even flips: the close edge's).
        #[arg(long)]
        assist: Option<std::path::PathBuf>,
        /// Only the plan inputs at these cells (door coordinates `x,y;x,y`).
        #[arg(long)]
        assist_cells: Option<String>,
        /// Print the door plane as a picture after every tick that changed it, over this box
        /// (door coordinates `x0,y0:x1,y1`), instead of the change list.
        #[arg(long)]
        grid: Option<String>,
    },
}

pub fn run(version: &str, cmd: CosearchCmd) -> Result<()> {
    match cmd {
        CosearchCmd::Eval { designs, extra_offsets, threads, props_always, repeat } => {
            let opts = JudgeOpts::with_extra(extra_offsets);
            let mut doors = Vec::new();
            for p in &designs {
                match load_door(p) {
                    Ok((_, r)) => doors.push((p.display().to_string(), r)),
                    Err(e) => println!("{}: SKIPPED ({e:#})", p.display()),
                }
            }
            let t0 = std::time::Instant::now();
            let jobs: Vec<(usize, usize)> = (0..doors.len()).flat_map(|i| (0..repeat).map(move |k| (i, k))).collect();
            let res = par_map(&jobs, threads, |&(i, _)| {
                let t = std::time::Instant::now();
                let e = evaluate(&doors[i].1, &opts, props_always);
                (e, t.elapsed().as_secs_f64() * 1000.0)
            });
            let secs = t0.elapsed().as_secs_f64();
            for ((i, k), (e, ms)) in jobs.iter().zip(res.iter()) {
                if *k == 0 {
                    println!("{}", eval_line(&doors[*i].0, e, *ms));
                }
            }
            println!(
                "{} evaluations ({} positions x {} cycles + FULL + hallway when the judge passes) in {secs:.2} s on {threads} threads: {:.1} evaluations/s",
                jobs.len(),
                opts.offsets.len(),
                opts.cycles,
                jobs.len() as f64 / secs
            );
            println!("({})", context_note());
        }
        CosearchCmd::Agree { designs, mutants, seed, flat_only, extra_offsets, workers, threads, out, judge_only } => {
            agree(version, &designs, mutants, seed, flat_only, extra_offsets, workers, threads, &out, judge_only)?;
        }
        CosearchCmd::Search(a) => crate::cosearch_search::run(version, &a)?,
        CosearchCmd::Pockets(a) => crate::cosearch_search::pockets_run(version, &a)?,
        CosearchCmd::Trace { design, offset, flips, cells, assist, assist_cells, grid } => {
            let (_, door) = load_door(&design)?;
            let opts = JudgeOpts::with_extra(28);
            let off = opts.offsets[offset.min(opts.offsets.len() - 1)];
            let blocks: Vec<(Pos, Block)> = door.blocks.iter().map(|(p, b)| (add(*p, off), *b)).collect();
            let spec = door.spec.translated(off);
            let d0 = door.spec.door;
            // the assist plan's events, only the chosen input cells
            let mut asst: [Vec<(u32, BlockPos, Block)>; 2] = [Vec::new(), Vec::new()];
            if let Some(path) = &assist {
                let a = crate::cosearch_search::load_assist(path, d0[2])?;
                let keep: Option<Vec<(i32, i32)>> = assist_cells.as_ref().map(|s| {
                    s.split(';')
                        .filter_map(|t| {
                            let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                            (v.len() == 2).then(|| (v[0] + d0[0], v[1] + d0[1]))
                        })
                        .collect()
                });
                for ph in 0..2 {
                    for (t, k, pos, b) in &a.events[ph] {
                        if keep.as_ref().is_none_or(|c| c.contains(&a.cells[*k])) {
                            asst[ph].push((*t, BlockPos::new(pos.x + off[0], pos.y + off[1], pos.z + off[2]), *b));
                        }
                    }
                }
            }
            let gbox: Option<([i32; 2], [i32; 2])> = match &grid {
                Some(g) => {
                    let (a, b) = anyhow::Context::context(g.split_once(':'), "--grid x0,y0:x1,y1")?;
                    let p = |t: &str| -> Result<[i32; 2]> {
                        let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 2, "--grid x0,y0:x1,y1");
                        Ok([v[0], v[1]])
                    };
                    Some((p(a)?, p(b)?))
                }
                None => None,
            };
            let only: Option<HashSet<Pos>> = cells.map(|c| {
                c.split(';')
                    .filter_map(|t| {
                        let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        (v.len() == 3).then(|| add([v[0], v[1], v[2]], off))
                    })
                    .collect()
            });
            let (wlo, whi) = watch_box(&blocks, &spec);
            let mut w = world_for(wlo, whi);
            let list: Vec<(BlockPos, Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
            w.place_like_judge(&list);
            let mut prev = Snap::take(&w, wlo, whi);
            let picture = |w: &World| -> String {
                let Some((a, b)) = gbox else { return String::new() };
                let mut s = String::new();
                for y in (a[1]..=b[1]).rev() {
                    s.push_str(&format!("{y:>4} |"));
                    for x in a[0]..=b[0] {
                        let q = BlockPos::new(x + d0[0] + off[0], y + d0[1] + off[1], d0[2] + off[2]);
                        s.push_str(&crate::cosearch_search::cell3(&w.get(q)));
                        s.push('|');
                    }
                    s.push('\n');
                }
                s
            };
            let show = |label: &str, prev: &Snap, now: &Snap, w: &World| {
                if gbox.is_some() {
                    let changed = now.iter().any(|(p, b)| prev.get(p) != Some(b) && p[2] == d0[2] + off[2]);
                    if changed || !w.drops.is_empty() {
                        println!("{label}{}\n{}", if w.drops.is_empty() { String::new() } else { format!(" DROPS {:?}", w.drops) }, picture(w));
                    }
                    return;
                }
                let ch: Vec<String> = now
                    .iter()
                    .filter(|(p, b)| prev.get(*p) != Some(*b) && only.as_ref().is_none_or(|o| o.contains(p)))
                    .map(|(p, b)| format!("{:?} {} -> {}", [p[0] - off[0], p[1] - off[1], p[2] - off[2]], prev.get(p).map(|x| st(&x)).unwrap_or_default().trim_start_matches("minecraft:"), st(&b).trim_start_matches("minecraft:")))
                    .collect();
                if !ch.is_empty() || !w.drops.is_empty() {
                    println!("{label}: {}{}", ch.join("; "), if w.drops.is_empty() { String::new() } else { format!(" DROPS {:?}", w.drops) });
                }
            };
            let now = Snap::take(&w, wlo, whi);
            println!("placed (before settling)");
            prev = {
                show("placement", &prev, &now, &w);
                now
            };
            let mut t = 0;
            for flip in 0..=flips {
                if flip > 0 {
                    w.use_block(bp(spec.lever));
                    let now = Snap::take(&w, wlo, whi);
                    show(&format!("flip {flip} (lever)"), &prev, &now, &w);
                    prev = now;
                    t = 0;
                }
                let evs: &[(u32, BlockPos, Block)] = if flip == 0 { &[] } else { &asst[((flip - 1) % 2) as usize] };
                let mut i = 0;
                loop {
                    while i < evs.len() && evs[i].0 == t {
                        w.set_block_and_update(evs[i].1, evs[i].2);
                        println!("  assist t{t}: {:?} := {}", [evs[i].1.x - off[0], evs[i].1.y - off[1], evs[i].1.z - off[2]], st(&evs[i].2).trim_start_matches("minecraft:"));
                        i += 1;
                    }
                    w.tick();
                    t += 1;
                    let now = Snap::take(&w, wlo, whi);
                    show(&format!("{} t{t}", if flip == 0 { "settle" } else { "after flip" }), &prev, &now, &w);
                    prev = now;
                    if (i >= evs.len() && w.idle()) || t >= 400 {
                        break;
                    }
                }
            }
            if !w.errors.is_empty() {
                println!("MODEL ERRORS: {:?}", w.errors);
            }
        }
    }
    Ok(())
}

/// A comparable verdict: the judge's outcome class plus FULL / hallway / volume.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// Ok(cycles at the first position) or (offset index, cycle, kind).
    pub judge: std::result::Result<Cycles, (usize, Option<u32>, &'static str)>,
    pub full: Option<bool>,
    pub hallway: Option<bool>,
    pub volume: Option<i32>,
}

impl Verdict {
    fn short(&self) -> String {
        let j = match &self.judge {
            Ok(c) => format!("WORKS {c:?}"),
            Err((o, c, k)) => format!("FAILS #{o} cycle {c:?} {k}"),
        };
        format!("{j} full={:?} hallway={:?} volume={:?}", self.full, self.hallway, self.volume)
    }
}

fn rs_verdict(d: &Design, opts: &JudgeOpts, judge_only: bool) -> std::result::Result<(Verdict, String), String> {
    if !crate::legality_problems(d).is_empty() {
        return Ok((Verdict { judge: Err((0, None, "illegal")), full: None, hallway: None, volume: None }, "illegal".into()));
    }
    let r = RsDoor::from_design(d)?;
    let e = if judge_only { Eval { judge: judge(&r, opts), full: None, hallway: None, volume: None, blocks: r.block_count() } } else { evaluate(&r, opts, false) };
    let detail = eval_line(&d.name, &e, 0.0);
    let judge = match &e.judge {
        Ok(c) => Ok(c.clone()),
        Err(Fail::Judge { offset, cycle, kind, .. }) => Err((*offset, *cycle, *kind)),
        Err(Fail::Model(m)) => return Err(format!("model: {m}")),
    };
    Ok((Verdict { judge, full: e.full.map(|x| x.is_ok()), hallway: e.hallway.map(|x| x.is_ok()), volume: e.volume.map(|v| v.0) }, detail))
}

fn jvm_verdict(w: &mut mctas::sim::Worker, d: &Design, opts: &crate::VerifyOptions, offsets: &[Pos], judge_only: bool) -> Result<(Verdict, String)> {
    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<(Verdict, String)> {
        let rep = crate::verify_in(w, sb, d, opts);
        let judge = match &rep {
            Ok(r) => Ok(r.cycles.iter().map(|c| (c.open_ticks, c.close_ticks)).collect::<Vec<_>>()),
            Err(e) => {
                let msg = format!("{e:#}");
                Err(classify_judge_error(&msg, offsets))
            }
        };
        let mut detail = match &rep {
            Ok(_) => "WORKS".to_string(),
            Err(e) => format!("FAILS {e:#}"),
        };
        let mut v = Verdict { judge, full: None, hallway: None, volume: None };
        if v.judge.is_ok() && !judge_only {
            let rec = crate::properties::record(w, sb, d)?;
            let s = crate::properties::seamless(&rec);
            v.full = Some(s.tier >= crate::properties::Tier::Full);
            let p = crate::properties::cheap(&rec, d);
            v.volume = p.get("volume").and_then(|x| x.parse().ok());
            let spec = DoorSpec::from_design(d)?;
            let (swept, _) = crate::properties_dyn::swept_cells(w, sb, d, &spec)?;
            let hall = crate::properties_dyn::hallway_env(d, &spec, &swept);
            let ro = crate::properties_dyn::RunOpts { cycles: 1, input: crate::properties_dyn::Input::Lever, snow: false, steps: false };
            let h = crate::properties_dyn::env_run(w, sb, d, &spec, &hall, &ro);
            v.hallway = Some(h.is_ok());
            detail = format!("{detail} | seamless {} ({}) | hallway {}", s.label, s.why.unwrap_or_default(), match &h {
                Ok(_) => "ok".to_string(),
                Err(e) => format!("breaks it: {e:#}"),
            });
        }
        Ok((v, detail))
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// splitmix64.
pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

/// States a mutant may put in a cell (flat-door components, in-plane facings).
pub const MUTANT_STATES: &[&str] = &[
    "minecraft:stone",
    "minecraft:smooth_stone",
    "minecraft:glass",
    "minecraft:redstone_wire",
    "minecraft:redstone_block",
    "minecraft:redstone_torch",
    "minecraft:redstone_wall_torch[facing=east]",
    "minecraft:redstone_wall_torch[facing=west]",
    "minecraft:repeater[facing=east,delay=1]",
    "minecraft:repeater[facing=west,delay=1]",
    "minecraft:repeater[facing=east,delay=2]",
    "minecraft:repeater[facing=west,delay=4]",
    "minecraft:comparator[facing=east]",
    "minecraft:comparator[facing=west,mode=subtract]",
    "minecraft:observer[facing=up]",
    "minecraft:observer[facing=down]",
    "minecraft:observer[facing=east]",
    "minecraft:observer[facing=west]",
    "minecraft:sticky_piston[facing=up]",
    "minecraft:sticky_piston[facing=down]",
    "minecraft:sticky_piston[facing=east]",
    "minecraft:sticky_piston[facing=west]",
    "minecraft:piston[facing=up]",
    "minecraft:piston[facing=east]",
    "minecraft:redstone_lamp",
    "minecraft:note_block",
    "minecraft:target",
    "minecraft:waxed_copper_bulb",
];

/// A random near-miss edit of a design (1 or 2 edits); the lever port keeps its lever.
pub fn mutate(d: &Design, rng: &mut Rng) -> Design {
    let mut m = d.clone();
    let lever = d.ports.get("lever").copied();
    let edits = 1 + rng.below(2);
    for _ in 0..edits {
        let keys: Vec<Pos> = m.blocks.keys().copied().filter(|p| Some(*p) != lever && !m.blocks[p].starts_with("minecraft:piston_head")).collect();
        if keys.is_empty() {
            break;
        }
        let p = keys[rng.below(keys.len())];
        let s = m.blocks[&p].clone();
        let take_head = |m: &mut Design, p: Pos| {
            // a piston takes its head along
            for dv in DIRS {
                let q = add(p, dv);
                if m.blocks.get(&q).is_some_and(|h| h.starts_with("minecraft:piston_head")) {
                    m.blocks.remove(&q);
                }
            }
        };
        match rng.below(6) {
            0 => {
                if s.contains("piston") {
                    take_head(&mut m, p);
                }
                m.blocks.remove(&p);
            }
            1 => {
                if s.contains("piston") {
                    take_head(&mut m, p);
                }
                m.blocks.insert(p, MUTANT_STATES[rng.below(MUTANT_STATES.len())].to_string());
            }
            2 => {
                // re-face / re-delay in place
                let n = if s.contains("delay=") {
                    let dl = 1 + rng.below(4);
                    let mut out = String::new();
                    for (i, part) in s.split("delay=").enumerate() {
                        if i == 0 {
                            out.push_str(part);
                        } else {
                            out.push_str(&format!("delay={dl}{}", &part[1..]));
                        }
                    }
                    out
                } else if s.contains("facing=") && !s.contains("extended=true") {
                    let f = ["up", "down", "east", "west"][rng.below(4)];
                    let mut out = String::new();
                    for (i, part) in s.split("facing=").enumerate() {
                        if i == 0 {
                            out.push_str(part);
                        } else {
                            let rest = part.trim_start_matches(|c: char| c.is_ascii_alphabetic());
                            let f = if s.contains("repeater") || s.contains("comparator") || s.contains("wall_torch") { ["east", "west"][rng.below(2)] } else { f };
                            out.push_str(&format!("facing={f}{rest}"));
                        }
                    }
                    out
                } else {
                    MUTANT_STATES[rng.below(MUTANT_STATES.len())].to_string()
                };
                m.blocks.insert(p, n);
            }
            3 | 4 => {
                // add next to an existing block (in the door plane)
                let dv = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0]][rng.below(4)];
                let q = add(p, dv);
                if !m.blocks.contains_key(&q) && Some(q) != lever {
                    m.blocks.insert(q, MUTANT_STATES[rng.below(MUTANT_STATES.len())].to_string());
                }
            }
            _ => {
                // move a non-piston block one cell
                if !s.contains("piston") {
                    let dv = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0]][rng.below(4)];
                    let q = add(p, dv);
                    if !m.blocks.contains_key(&q) && Some(q) != lever {
                        m.blocks.remove(&p);
                        m.blocks.insert(q, s);
                    }
                }
            }
        }
    }
    m
}

#[allow(clippy::too_many_arguments)]
fn agree(
    version: &str,
    designs: &[std::path::PathBuf],
    mutants: usize,
    seed: u64,
    flat_only: bool,
    extra: u32,
    workers: usize,
    threads: usize,
    out: &std::path::Path,
    judge_only: bool,
) -> Result<()> {
    let mut cases: Vec<(String, Design)> = Vec::new();
    let mut skipped = 0;
    let mut rng = Rng(seed ^ 0x5DEECE66D);
    for p in designs {
        let Ok(d) = crate::cli::load_design(p) else {
            skipped += 1;
            continue;
        };
        if flat_only && !crate::single_layer(&d) {
            continue;
        }
        if !d.entities.is_empty() || RsDoor::from_design(&d).is_err() && crate::legality_problems(&d).is_empty() {
            skipped += 1;
            continue;
        }
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        for k in 0..mutants {
            let mut m = mutate(&d, &mut rng);
            m.name = format!("{stem}-m{k}");
            if m.blocks.values().all(|s| convert_state(s).is_ok()) {
                cases.push((m.name.clone(), m));
            }
        }
        cases.insert(cases.len() - cases.iter().rev().take_while(|(n, _)| n.starts_with(&format!("{stem}-m"))).count(), (stem.clone(), d));
    }
    eprintln!("{} cases ({} designs skipped: unreadable, entities or blocks redsim does not model)", cases.len(), skipped);
    let opts = JudgeOpts::with_extra(extra);
    let t0 = std::time::Instant::now();
    let rs: Vec<std::result::Result<(Verdict, String), String>> = par_map(&cases, threads, |(_, d)| rs_verdict(d, &opts, judge_only));
    let rs_secs = t0.elapsed().as_secs_f64();
    eprintln!("redsim: {} cases in {rs_secs:.1} s ({:.1}/s on {threads} threads)", cases.len(), cases.len() as f64 / rs_secs);
    let pool = mctas::sim::Pool::spawn(workers, &mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let mut vopts = crate::VerifyOptions::default();
    vopts.offsets.extend(crate::extra_offsets(extra));
    let t1 = std::time::Instant::now();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let total = cases.len();
    let jv: Vec<Result<(Verdict, String)>> = pool.map(cases.iter().map(|(_, d)| d.clone()).collect(), |w, _, d| {
        let r = jvm_verdict(w, &d, &vopts, &opts.offsets, judge_only);
        let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        if k % 50 == 0 {
            eprintln!("  jvm {k}/{total}");
        }
        r
    });
    let jv_secs = t1.elapsed().as_secs_f64();
    std::fs::create_dir_all(out)?;
    let (mut same, mut diff, mut model, mut jerr) = (0, 0, 0, 0);
    let (mut pass_both, mut judge_same) = (0, 0);
    let mut report = String::new();
    for (((name, d), r), j) in cases.iter().zip(rs.iter()).zip(jv.iter()) {
        let (j, jd) = match j {
            Ok(x) => x,
            Err(e) => {
                jerr += 1;
                report.push_str(&format!("{name}: JVM error {e:#}\n"));
                continue;
            }
        };
        let (r, rd) = match r {
            Ok(x) => x,
            Err(e) => {
                model += 1;
                report.push_str(&format!("{name}: redsim cannot judge it ({e}); JVM: {}\n", j.short()));
                continue;
            }
        };
        if r.judge == j.judge {
            judge_same += 1;
        }
        if r.judge.is_ok() && j.judge.is_ok() {
            pass_both += 1;
        }
        if r == j {
            same += 1;
        } else {
            diff += 1;
            let path = out.join(format!("{name}.txt"));
            let mut dd = d.clone();
            dd.meta.insert("redsim".into(), r.short());
            dd.meta.insert("jvm".into(), j.short());
            dd.save(&path)?;
            report.push_str(&format!("{name}: DISAGREE\n  redsim: {}\n          {rd}\n  jvm:    {}\n          {jd}\n  file: {}\n", r.short(), j.short(), path.display()));
        }
    }
    let n = same + diff;
    let summary = format!(
        "agreement: {same}/{n} cases identical ({:.2}%), judge verdict class identical {judge_same}/{n}; {pass_both} work in both; {diff} disagreements, {model} redsim model limits, {jerr} JVM errors\nredsim {rs_secs:.1} s ({:.1} cases/s, {threads} threads), JVM {jv_secs:.1} s ({:.2} cases/s, {} workers)\n",
        100.0 * same as f64 / n.max(1) as f64,
        cases.len() as f64 / rs_secs,
        cases.len() as f64 / jv_secs,
        pool.len()
    );
    report.push_str(&summary);
    std::fs::write(out.join("REPORT.txt"), &report)?;
    print!("{report}");
    Ok(())
}
