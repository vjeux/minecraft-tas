//! `mctas door pistfuzz`: differential fuzzing of the redsim model (track pistmodel: pistons and
//! moving blocks, with redmodel's redstone components around them) against the real game (the
//! bridge sandbox): random setups with scripted inputs, the gadget library's drive scripts and
//! archived doors; every block state, every moving block entity, the pending queue sizes, the
//! drops and the idle flag compared after every tick; mismatches shrunk to minimal scenarios.

use anyhow::{Context, Result, bail};
use mctas::sim::{PlaceMode, SandboxOptions, Worker};
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub type P3 = [i32; 3];

/// One scripted run: blocks placed (raw, then a shape update of every wire if `judge`, then a
/// `self` update of every block, list order), then the steps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    /// Compared region (inclusive).
    pub lo: P3,
    pub hi: P3,
    pub blocks: Vec<(P3, String)>,
    pub judge: bool,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// `setBlock(pos, state, 3)` (a player-like placement; `minecraft:air` removes).
    Set(P3, String),
    /// The game's `/setblock` command (non-strict): shape-updated state with flags 258, then
    /// `updateNeighboursOnBlockSet`.
    SetCmd(P3, String),
    /// A lever or button click.
    Use(P3),
    /// n ticks.
    Tick(u32),
    /// Ticks until idle (at most n).
    Settle(u32),
}

/// What both simulators report after a tick.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Frame {
    /// Non-air blocks of the region.
    pub blocks: BTreeMap<P3, String>,
    /// Moving blocks: pos -> "state ext|ret src|blk direction progress(halves)".
    pub moving: BTreeMap<P3, String>,
    pub block_events: u32,
    pub block_ticks: u32,
    pub moving_count: u32,
    /// Any item dropped so far (the bridge: an entity that is not a falling block).
    pub drops: bool,
    pub idle: bool,
}

/// A simulator under test.
pub trait Sim {
    fn place(&mut self, blocks: &[(P3, String)], judge: bool) -> Result<()>;
    fn set_update(&mut self, pos: P3, state: &str) -> Result<()>;
    fn setblock_cmd(&mut self, pos: P3, state: &str) -> Result<()>;
    fn use_block(&mut self, pos: P3) -> Result<()>;
    fn tick(&mut self, lo: P3, hi: P3) -> Result<Frame>;
}

/// Run a scenario: one frame per tick.
pub fn run(sim: &mut dyn Sim, sc: &Scenario) -> Result<Vec<Frame>> {
    sim.place(&sc.blocks, sc.judge)?;
    let mut frames = Vec::new();
    for s in &sc.steps {
        match s {
            Step::Set(p, st) => sim.set_update(*p, st)?,
            Step::SetCmd(p, st) => sim.setblock_cmd(*p, st)?,
            Step::Use(p) => sim.use_block(*p)?,
            Step::Tick(n) => {
                for _ in 0..*n {
                    frames.push(sim.tick(sc.lo, sc.hi)?);
                }
            }
            Step::Settle(n) => {
                for _ in 0..*n {
                    let f = sim.tick(sc.lo, sc.hi)?;
                    let idle = f.idle;
                    frames.push(f);
                    if idle {
                        break;
                    }
                }
            }
        }
    }
    Ok(frames)
}

/// The first difference between two runs.
pub fn compare(a: &[Frame], b: &[Frame]) -> Option<String> {
    compare_upto(a, b, false).0
}

/// [`compare`], optionally stopping after the first frame where the game dropped an item: item
/// entities are not modeled, and with sculk sensors around they are heard (an item landing is a
/// `hit_ground` vibration), so nothing after a drop is comparable. Returns the difference and
/// the frames compared.
pub fn compare_upto(a: &[Frame], b: &[Frame], stop_at_drop: bool) -> (Option<String>, usize) {
    let n = if stop_at_drop { a.iter().position(|f| f.drops).map(|i| i + 1).unwrap_or(a.len()) } else { a.len() };
    let (a, b) = if n < a.len() { (&a[..n], &b[..n.min(b.len())]) } else { (a, b) };
    (compare_frames(a, b), a.len())
}

fn compare_frames(a: &[Frame], b: &[Frame]) -> Option<String> {
    for (t, (fa, fb)) in a.iter().zip(b.iter()).enumerate() {
        if fa == fb {
            continue;
        }
        let mut s = format!("frame {}:", t + 1);
        let keys: std::collections::BTreeSet<&P3> = fa.blocks.keys().chain(fb.blocks.keys()).collect();
        let mut n = 0;
        for k in keys {
            let (x, y) = (fa.blocks.get(k), fb.blocks.get(k));
            if x != y && n < 8 {
                let _ = write!(s, " {:?} game={} model={};", k, x.map(|v| short(v)).unwrap_or("air"), y.map(|v| short(v)).unwrap_or("air"));
                n += 1;
            }
        }
        let keys: std::collections::BTreeSet<&P3> = fa.moving.keys().chain(fb.moving.keys()).collect();
        for k in keys {
            let (x, y) = (fa.moving.get(k), fb.moving.get(k));
            if x != y && n < 12 {
                let _ = write!(s, " moving {:?} game={} model={};", k, x.map(|v| v.as_str()).unwrap_or("-"), y.map(|v| v.as_str()).unwrap_or("-"));
                n += 1;
            }
        }
        let qa = (fa.block_events, fa.block_ticks, fa.moving_count, fa.drops, fa.idle);
        let qb = (fb.block_events, fb.block_ticks, fb.moving_count, fb.drops, fb.idle);
        if qa != qb {
            let _ = write!(s, " (events, ticks, moving, drops, idle) game={qa:?} model={qb:?}");
        }
        return Some(s);
    }
    if a.len() != b.len() {
        return Some(format!("frame counts differ: game {} vs model {}", a.len(), b.len()));
    }
    None
}

fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

// ---- the bridge side ----

pub struct BridgeSim<'a> {
    pub w: &'a mut Worker,
    pub sb: u32,
}

impl Sim for BridgeSim<'_> {
    fn place(&mut self, blocks: &[(P3, String)], judge: bool) -> Result<()> {
        if blocks.is_empty() {
            return Ok(());
        }
        self.w.set_blocks(self.sb, blocks, PlaceMode::Raw)?;
        if judge {
            let wires: Vec<P3> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
            if !wires.is_empty() {
                self.w.update(self.sb, &wires, "shape")?;
            }
        }
        let positions: Vec<P3> = blocks.iter().map(|(p, _)| *p).collect();
        self.w.update(self.sb, &positions, "self")?;
        Ok(())
    }
    fn set_update(&mut self, pos: P3, state: &str) -> Result<()> {
        self.w.set_blocks(self.sb, &[(pos, state)], PlaceMode::Update)?;
        Ok(())
    }
    fn setblock_cmd(&mut self, pos: P3, state: &str) -> Result<()> {
        self.w.exec(self.sb, &format!("setblock {} {} {} {state}", pos[0], pos[1], pos[2]))
    }
    fn use_block(&mut self, pos: P3) -> Result<()> {
        self.w.use_block(self.sb, pos)?;
        Ok(())
    }
    fn tick(&mut self, lo: P3, hi: P3) -> Result<Frame> {
        // pipelined: step, get (moving), status, entities
        let sb = self.sb;
        self.w.send(json!({"op": "step", "sandbox": sb, "ticks": 1}))?;
        self.w.send(json!({"op": "get", "sandbox": sb, "from": lo, "to": hi, "moving": true}))?;
        self.w.send(json!({"op": "status", "sandbox": sb}))?;
        self.w.send(json!({"op": "entities", "sandbox": sb}))?;
        self.w.flush()?;
        let _ = self.w.recv().context("step")?;
        let g = self.w.recv().context("get")?;
        let st = self.w.recv().context("status")?;
        let ents = self.w.recv().context("entities")?;
        let mut f = Frame::default();
        let palette: Vec<String> = g["palette"].as_array().cloned().unwrap_or_default().iter().map(|v| v.as_str().unwrap_or("").to_string()).collect();
        let size: Vec<i64> = g["size"].as_array().cloned().unwrap_or_default().iter().map(|v| v.as_i64().unwrap_or(0)).collect();
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::STANDARD.decode(g["data"].as_str().unwrap_or(""))?;
        let (sx, sz) = (size[0] as i32, size[2] as i32);
        for (i, c) in bytes.chunks_exact(2).enumerate() {
            let idx = u16::from_le_bytes([c[0], c[1]]) as usize;
            let s = &palette[idx];
            if s == "minecraft:air" {
                continue;
            }
            let i = i as i32;
            f.blocks.insert([lo[0] + i % sx, lo[1] + i / (sx * sz), lo[2] + (i / sx) % sz], canon(s));
        }
        for m in g["moving"].as_array().cloned().unwrap_or_default() {
            let p: Vec<i32> = m["pos"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
            let prog = (m["progress"].as_f64().unwrap_or(0.0) * 2.0).round() as i32;
            f.moving.insert(
                [p[0], p[1], p[2]],
                format!(
                    "{} {} {} {} {}",
                    short(&canon(m["state"].as_str().unwrap_or(""))),
                    if m["extending"].as_bool().unwrap_or(false) { "ext" } else { "ret" },
                    if m["source"].as_bool().unwrap_or(false) { "src" } else { "blk" },
                    m["direction"].as_str().unwrap_or(""),
                    prog
                ),
            );
        }
        f.block_events = st["block_events"].as_u64().unwrap_or(0) as u32;
        f.block_ticks = st["block_ticks"].as_u64().unwrap_or(0) as u32;
        f.moving_count = st["moving_pistons"].as_u64().unwrap_or(0) as u32;
        f.idle = st["idle"].as_bool().unwrap_or(false);
        f.drops = ents["entities"].as_array().is_some_and(|v| v.iter().any(|e| e["type"].as_str() != Some("minecraft:falling_block")));
        Ok(f)
    }
}

/// The bridge's state strings through redsim's parser when it knows the block (formatting
/// differences never count), as-is otherwise.
fn canon(s: &str) -> String {
    match redsim::Block::parse(s) {
        Ok(b) => b.to_string(),
        Err(_) => s.to_string(),
    }
}

/// Run on the worker's sandbox (created once, cleared between runs).
pub fn run_bridge(w: &mut Worker, sb: &mut Option<u32>, sc: &Scenario) -> Result<Vec<Frame>> {
    let id = match *sb {
        Some(id) => {
            w.clear(id)?;
            id
        }
        None => {
            let id = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
            *sb = Some(id);
            id
        }
    };
    let mut sim = BridgeSim { w, sb: id };
    run(&mut sim, sc)
}

// ---- the model side ----

pub struct ModelSim {
    pub w: redsim::World,
}

impl ModelSim {
    pub fn new(lo: P3, hi: P3) -> ModelSim {
        let m = 15;
        let min = redsim::BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m);
        let size = [hi[0] - lo[0] + 1 + 2 * m, hi[1] - lo[1] + 1 + 2 * m, hi[2] - lo[2] + 1 + 2 * m];
        ModelSim { w: redsim::World::new(min, size) }
    }
}

fn bp(p: P3) -> redsim::BlockPos {
    redsim::BlockPos::new(p[0], p[1], p[2])
}

fn parse_block(s: &str) -> Result<redsim::Block> {
    redsim::Block::parse(s).map_err(|e| anyhow::anyhow!("{e}"))
}

/// Property names written in a state string.
fn explicit_props(s: &str) -> Vec<(String, String)> {
    let Some(i) = s.find('[') else { return Vec::new() };
    s[i + 1..]
        .trim_end_matches(']')
        .split(',')
        .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect()
}

impl Sim for ModelSim {
    fn place(&mut self, blocks: &[(P3, String)], judge: bool) -> Result<()> {
        let mut list = Vec::with_capacity(blocks.len());
        for (p, s) in blocks {
            list.push((bp(*p), parse_block(s)?));
        }
        if judge {
            self.w.place_like_judge(&list);
        } else {
            for (p, b) in &list {
                self.w.set_block(*p, *b, redsim::world::flags::RAW);
            }
            for (p, _) in &list {
                let k = self.w.get(*p).kind();
                self.w.neighbor_changed(*p, k);
            }
        }
        Ok(())
    }
    fn set_update(&mut self, pos: P3, state: &str) -> Result<()> {
        let b = parse_block(state)?;
        self.w.set_block(bp(pos), b, redsim::world::flags::ALL);
        Ok(())
    }
    fn setblock_cmd(&mut self, pos: P3, state: &str) -> Result<()> {
        // BlockInput.place: the state updated from its neighbors' shapes (air -> as given), the
        // properties written in the command kept, setBlock(.., 2 | 256); then
        // updateNeighboursOnBlockSet.
        let p = bp(pos);
        let given = parse_block(state)?;
        let old = self.w.get(p);
        let mut st = redsim::behavior::update_from_neighbour_shapes(&mut self.w, given, p);
        if st.is_air() {
            st = given;
        }
        let ex = explicit_props(state);
        if !ex.is_empty() {
            let text = st.to_string();
            let name = text.split('[').next().unwrap_or(&text).to_string();
            let mut props = explicit_props(&text);
            for (k, v) in &ex {
                if let Some(e) = props.iter_mut().find(|(kk, _)| kk == k) {
                    e.1 = v.clone();
                }
            }
            let merged = if props.is_empty() { name } else { format!("{name}[{}]", props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(",")) };
            st = parse_block(&merged)?;
        }
        self.w.set_block(p, st, 2 | 256);
        // ServerLevel.updateNeighboursOnBlockSet
        let now = self.w.get(p);
        if old.kind() != now.kind() {
            redsim::behavior::affect_neighbors_after_removal(&mut self.w, old, p, false);
        }
        self.w.update_neighbors_at(p, now.kind());
        if redsim::behavior::has_analog_output(&now) {
            self.w.update_neighbour_for_output_signal(p, now.kind());
        }
        Ok(())
    }
    fn use_block(&mut self, pos: P3) -> Result<()> {
        self.w.use_block(bp(pos));
        Ok(())
    }
    fn tick(&mut self, lo: P3, hi: P3) -> Result<Frame> {
        self.w.tick();
        let mut f = Frame::default();
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    let b = self.w.get(redsim::BlockPos::new(x, y, z));
                    if !b.is_air() {
                        f.blocks.insert([x, y, z], b.to_string());
                    }
                }
            }
        }
        for (p, be) in self.w.piston.all() {
            if p.x < lo[0] || p.y < lo[1] || p.z < lo[2] || p.x > hi[0] || p.y > hi[1] || p.z > hi[2] {
                continue;
            }
            f.moving.insert(
                [p.x, p.y, p.z],
                format!(
                    "{} {} {} {} {}",
                    short(&be.moved.to_string()),
                    if be.extending { "ext" } else { "ret" },
                    if be.source { "src" } else { "blk" },
                    be.direction.name(),
                    be.progress_o
                ),
            );
        }
        f.block_events = self.w.events.len() as u32;
        f.block_ticks = self.w.ticks.count() as u32;
        f.moving_count = self.w.piston.live_count() as u32;
        f.drops = !self.w.drops.is_empty();
        f.idle = self.w.idle();
        Ok(f)
    }
}

/// Run a scenario in the redsim model (errors the model reports itself count as a failure).
pub fn run_model(sc: &Scenario) -> Result<Vec<Frame>> {
    let mut sim = ModelSim::new(sc.lo, sc.hi);
    let frames = run(&mut sim, sc)?;
    if std::env::var("PISTFUZZ_DEBUG").is_ok() {
        eprintln!("model drops: {:?}", sim.w.drops);
        eprintln!("model errors: {:?}", sim.w.errors);
    }
    if let Some(e) = sim.w.errors.first() {
        bail!("model error: {e}");
    }
    Ok(frames)
}

// ---- generators ----

/// A tiny deterministic RNG (splitmix64).
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next() % n }
    }
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64 <= p
    }
    pub fn pick<'a, T>(&mut self, v: &'a [T]) -> &'a T {
        &v[self.below(v.len() as u64) as usize]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GenOpts {
    /// Flat 7x7x1 in the door plane (pistons east/west/up/down) or a 6x6x3 box (any facing).
    pub flat: bool,
    pub slime: bool,
    pub gravity: bool,
    pub observers: bool,
    /// Redstone components next to the pistons (dust, torches, repeaters, comparators, levers,
    /// lamps, note blocks, targets), judge-style placement.
    pub mixed: bool,
    /// Shaped blocks among the solids: slabs, trapdoors, scaffolding, fence gates (moved by
    /// pistons, landed on by falling blocks).
    pub shapes: bool,
}

const HORIZ: [&str; 4] = ["north", "south", "west", "east"];

fn facings(flat: bool) -> &'static [&'static str] {
    if flat { &["east", "west", "up", "down"] } else { &["north", "south", "west", "east", "up", "down"] }
}

fn step_of(f: &str) -> P3 {
    match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        "up" => [0, 1, 0],
        _ => [0, -1, 0],
    }
}

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A random piston setup.
pub fn gen_piston(seed: u64, o: GenOpts) -> Scenario {
    let mut r = Rng::new(seed);
    let (sx, sy, sz) = if o.flat { (7, 7, 1) } else { (6, 6, 3) };
    // anywhere in the sandbox: the vacated-cell order (a Java HashMap) depends on positions
    let ox = -26 + r.below(40) as i32;
    let oz = -26 + r.below(40) as i32;
    let oy = 60 + r.below(100) as i32;
    let mut cells: BTreeMap<P3, String> = BTreeMap::new();
    let inside = |p: P3| p[0] >= ox && p[0] < ox + sx && p[1] >= oy && p[1] < oy + sy && p[2] >= oz && p[2] < oz + sz;
    let mut all: Vec<P3> = Vec::new();
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                all.push([ox + x, oy + y, oz + z]);
            }
        }
    }
    let fac = facings(o.flat);
    let mut levers: Vec<P3> = Vec::new();
    for &p in &all {
        if cells.contains_key(&p) {
            continue;
        }
        let roll = r.below(1000);
        let s: String = if roll < 380 {
            continue;
        } else if roll < 520 {
            if o.shapes && r.chance(0.45) {
                // collision / support shapes that pistons move and falling blocks land on
                match r.below(6) {
                    0 => format!("minecraft:smooth_stone_slab[type={},waterlogged=false]", r.pick(&["bottom", "top", "double"])),
                    1 | 2 => format!(
                        "minecraft:{}_trapdoor[facing={},half={},open={},powered=false,waterlogged=false]",
                        r.pick(&["oak", "iron"]),
                        r.pick(&HORIZ),
                        r.pick(&["bottom", "top"]),
                        r.chance(0.5)
                    ),
                    3 | 4 => format!("minecraft:scaffolding[bottom=false,distance={},waterlogged=false]", r.pick(&["0", "0", "1", "7"])),
                    _ => format!("minecraft:oak_fence_gate[facing={},in_wall=false,open={},powered=false]", r.pick(&HORIZ), r.chance(0.5)),
                }
            } else {
                "minecraft:stone".into()
            }
        } else if roll < 760 {
            let sticky = r.chance(0.5);
            let f = *r.pick(fac);
            let name = if sticky { "sticky_piston" } else { "piston" };
            let front = add(p, step_of(f));
            if r.chance(0.2) && inside(front) && !cells.contains_key(&front) {
                cells.insert(front, format!("minecraft:piston_head[facing={f},short=false,type={}]", if sticky { "sticky" } else { "normal" }));
                format!("minecraft:{name}[extended=true,facing={f}]")
            } else {
                format!("minecraft:{name}[extended=false,facing={f}]")
            }
        } else if roll < 800 {
            "minecraft:obsidian".into()
        } else if roll < 860 {
            "minecraft:redstone_block".into()
        } else if roll < 890 {
            if o.observers { format!("minecraft:observer[facing={},powered=false]", r.pick(fac)) } else { "minecraft:stone".into() }
        } else if roll < 915 {
            format!("minecraft:white_glazed_terracotta[facing={}]", r.pick(&HORIZ))
        } else if roll < 965 {
            if o.slime {
                if r.chance(0.65) { "minecraft:slime_block".into() } else { "minecraft:honey_block".into() }
            } else {
                "minecraft:stone".into()
            }
        } else if roll < 985 {
            if o.gravity { r.pick(&["minecraft:sand", "minecraft:gravel", "minecraft:red_sand"]).to_string() } else { "minecraft:stone".into() }
        } else {
            "minecraft:crying_obsidian".into()
        };
        cells.insert(p, s);
    }
    if o.mixed {
        // components on/next to solid blocks
        let solid: Vec<P3> = cells.iter().filter(|(_, s)| s.as_str() == "minecraft:stone").map(|(p, _)| *p).collect();
        let n = 3 + r.below(8);
        for _ in 0..n {
            if solid.is_empty() {
                break;
            }
            let sp = *r.pick(&solid);
            let up = add(sp, [0, 1, 0]);
            let side_f = *r.pick(if o.flat { &["east", "west"][..] } else { &HORIZ[..] });
            let side = add(sp, step_of(side_f));
            match r.below(9) {
                0 | 1 => {
                    if inside(up) && !cells.contains_key(&up) {
                        cells.insert(up, "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into());
                    }
                }
                2 => {
                    if inside(up) && !cells.contains_key(&up) {
                        cells.insert(up, "minecraft:redstone_torch[lit=true]".into());
                    }
                }
                3 => {
                    if inside(side) && !cells.contains_key(&side) {
                        cells.insert(side, format!("minecraft:redstone_wall_torch[facing={side_f},lit=true]"));
                    }
                }
                4 => {
                    if inside(up) && !cells.contains_key(&up) {
                        let f = *r.pick(if o.flat { &["east", "west"][..] } else { &HORIZ[..] });
                        cells.insert(up, format!("minecraft:repeater[delay={},facing={f},locked=false,powered=false]", 1 + r.below(4)));
                    }
                }
                5 => {
                    if inside(up) && !cells.contains_key(&up) {
                        let f = *r.pick(if o.flat { &["east", "west"][..] } else { &HORIZ[..] });
                        cells.insert(up, format!("minecraft:comparator[facing={f},mode={},powered=false]", if r.chance(0.5) { "compare" } else { "subtract" }));
                    }
                }
                6 => {
                    if inside(side) && !cells.contains_key(&side) {
                        cells.insert(side, format!("minecraft:lever[face=wall,facing={side_f},powered=false]"));
                        levers.push(side);
                    }
                }
                7 => {
                    cells.insert(sp, if r.chance(0.5) { "minecraft:redstone_lamp[lit=false]".into() } else { "minecraft:note_block[instrument=harp,note=0,powered=false]".into() });
                }
                _ => {
                    cells.insert(sp, "minecraft:target[power=0]".into());
                }
            }
        }
    }
    let blocks: Vec<(P3, String)> = cells.into_iter().collect();
    // inputs: redstone blocks set / cleared (mostly next to a piston or its QC cell), short
    // pulses of 0..5 gt, blocks placed or removed, lever clicks
    let mut hot: Vec<P3> = Vec::new();
    for (p, s) in &blocks {
        if s.contains("piston[") {
            let f = s.split("facing=").nth(1).and_then(|r| r.split(&[',', ']'][..]).next()).unwrap_or("up");
            let fr = step_of(f);
            for d in ["north", "south", "west", "east", "up", "down"] {
                let v = step_of(d);
                if v == fr {
                    continue;
                }
                hot.push(add(*p, v));
                if d != "down" {
                    hot.push(add(add(*p, v), [0, 1, 0]));
                }
            }
        }
    }
    hot.retain(|p| inside(*p));
    let mut timed: Vec<(u32, Step)> = Vec::new();
    let n = 5 + r.below(10);
    for _ in 0..n {
        let t = r.below(30) as u32;
        if !levers.is_empty() && r.chance(0.25) {
            timed.push((t, Step::Use(*r.pick(&levers))));
            continue;
        }
        let p = if !hot.is_empty() && r.chance(0.75) { *r.pick(&hot) } else { *r.pick(&all) };
        match r.below(12) {
            0..=6 => {
                timed.push((t, Step::Set(p, "minecraft:redstone_block".into())));
                if r.chance(0.7) {
                    let k = r.below(6) as u32;
                    timed.push((t + k, Step::Set(p, "minecraft:air".into())));
                }
            }
            7..=8 => timed.push((t, Step::Set(p, "minecraft:air".into()))),
            9 => {
                let sticky = r.chance(0.5);
                let f = *r.pick(fac);
                timed.push((t, Step::Set(p, format!("minecraft:{}[extended=false,facing={f}]", if sticky { "sticky_piston" } else { "piston" }))));
            }
            _ => timed.push((t, Step::Set(p, "minecraft:stone".into()))),
        }
    }
    timed.sort_by_key(|(t, _)| *t);
    let total = 48;
    let mut steps = Vec::new();
    let mut now = 0;
    for (t, s) in timed {
        if t > now {
            steps.push(Step::Tick(t - now));
            now = t;
        }
        steps.push(s);
    }
    steps.push(Step::Tick(total - now));
    let m = 4;
    Scenario {
        name: format!("{}-{}-{seed}", if o.mixed { "mixed" } else { "piston" }, if o.flat { "flat" } else { "box" }),
        lo: [ox - m, oy - m, oz - if o.flat { 1 } else { m }],
        hi: [ox + sx - 1 + m, oy + sy - 1 + m, oz + sz - 1 + if o.flat { 1 } else { m }],
        blocks,
        judge: o.mixed,
        steps,
    }
}

/// A random sculk setup (track sculk, `--sculk`): plain and calibrated sculk sensors among
/// pistons, wool (occludes vibrations), amethyst (resonates), note blocks, trapdoors, empty
/// dispensers (they click: `block_activate`), redstone blocks, observers; then wiring: dust on
/// solids, repeaters, comparators reading sensors (the frequency), observers watching sensors,
/// levers, and frequency filters (a dust line from a redstone block or a lever into a calibrated
/// sensor's back). Inputs: redstone blocks set and cleared near pistons, lever clicks, blocks
/// placed or removed, over 130 ticks (a sensor cycle is 40). Every sensor stands in chunks -1..0
/// of the sandbox (x, z in -16..15): `areAdjacentChunksTicking` holds there (forced chunks
/// -2..1); `border` lets the box reach the sandbox's edge chunks instead.
pub fn gen_sculk(seed: u64, flat: bool, border: bool) -> Scenario {
    let mut r = Rng::new(seed ^ 0x5C01_7C01);
    let (sx, sy, sz) = if flat { (13, 10, 1) } else { (10, 7, 6) };
    let (ox, oz) = if border {
        (-32 + r.below((64 - sx) as u64) as i32, -32 + r.below((64 - sz) as u64) as i32)
    } else {
        (-16 + r.below((33 - sx) as u64) as i32, -16 + r.below((33 - sz) as u64) as i32)
    };
    let oy = 60 + r.below(100) as i32;
    let inside = |p: P3| p[0] >= ox && p[0] < ox + sx && p[1] >= oy && p[1] < oy + sy && p[2] >= oz && p[2] < oz + sz;
    let mut all: Vec<P3> = Vec::new();
    for y in 0..sy {
        for z in 0..sz {
            for x in 0..sx {
                all.push([ox + x, oy + y, oz + z]);
            }
        }
    }
    let fac = facings(flat);
    let hfac: &[&str] = if flat { &["east", "west"] } else { &HORIZ };
    // calm boxes (fewer pistons and redstone blocks: fewer drops, longer comparable runs) or busy
    let calm = r.chance(0.6);
    let (air, stone, pist) = if calm { (520u64, 600u64, 640u64) } else { (340, 440, 600) };
    let mut cells: BTreeMap<P3, String> = BTreeMap::new();
    let mut levers: Vec<P3> = Vec::new();
    let mut sensors: Vec<P3> = Vec::new();
    for &p in &all {
        if cells.contains_key(&p) {
            continue;
        }
        let roll = r.below(1000);
        let s: String = if roll < air {
            continue;
        } else if roll < stone {
            "minecraft:stone".into()
        } else if roll < pist {
            let sticky = r.chance(0.5);
            let f = *r.pick(fac);
            let name = if sticky { "sticky_piston" } else { "piston" };
            let front = add(p, step_of(f));
            if r.chance(0.2) && inside(front) && !cells.contains_key(&front) {
                cells.insert(front, format!("minecraft:piston_head[facing={f},short=false,type={}]", if sticky { "sticky" } else { "normal" }));
                format!("minecraft:{name}[extended=true,facing={f}]")
            } else {
                format!("minecraft:{name}[extended=false,facing={f}]")
            }
        } else {
            // the rest, by weight (out of 400)
            let q = r.below(400);
            if q < 70 {
                sensors.push(p);
                "minecraft:sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]".into()
            } else if q < 120 {
                sensors.push(p);
                format!("minecraft:calibrated_sculk_sensor[facing={},power=0,sculk_sensor_phase=inactive,waterlogged=false]", r.pick(hfac))
            } else if q < 160 {
                format!("minecraft:{}", r.pick(&["white_wool", "orange_wool", "black_wool"]))
            } else if q < 190 {
                "minecraft:amethyst_block".into()
            } else if q < 230 {
                if calm && r.chance(0.7) { "minecraft:obsidian".into() } else { "minecraft:redstone_block".into() }
            } else if q < 260 {
                format!("minecraft:observer[facing={},powered=false]", r.pick(fac))
            } else if q < 280 {
                "minecraft:note_block[instrument=harp,note=0,powered=false]".into()
            } else if q < 300 {
                format!(
                    "minecraft:{}_trapdoor[facing={},half={},open=false,powered=false,waterlogged=false]",
                    r.pick(&["oak", "iron"]),
                    r.pick(&HORIZ),
                    r.pick(&["bottom", "top"])
                )
            } else if q < 320 {
                format!("minecraft:dispenser[facing={},triggered=false]", r.pick(fac))
            } else if q < 340 {
                "minecraft:redstone_lamp[lit=false]".into()
            } else if q < 355 {
                "minecraft:obsidian".into()
            } else {
                "minecraft:stone".into()
            }
        };
        cells.insert(p, s);
    }
    // frequency filters: a dust line on a stone floor from a redstone block or a lever into a
    // calibrated sensor's back (power 15 - (n - 1) at the back)
    let calibrated: Vec<P3> = cells.iter().filter(|(_, s)| s.starts_with("minecraft:calibrated")).map(|(p, _)| *p).collect();
    for c in calibrated {
        if !r.chance(0.5) {
            continue;
        }
        let s = cells[&c].clone();
        let f = s.split("facing=").nth(1).and_then(|t| t.split(&[',', ']'][..]).next()).unwrap_or("north").to_string();
        let back = step_of(match f.as_str() {
            "north" => "south",
            "south" => "north",
            "west" => "east",
            _ => "west",
        });
        let n = 1 + r.below(8) as i32;
        let mut cur = c;
        let mut line: Vec<P3> = Vec::new();
        let mut ok = true;
        for _ in 0..n {
            cur = add(cur, back);
            line.push(cur);
        }
        let end = add(cur, back);
        for q in line.iter().chain(std::iter::once(&end)) {
            let below = add(*q, [0, -1, 0]);
            if !inside(*q) || !inside(below) || cells.get(q).is_some_and(|s| s.contains("sculk")) || cells.get(&below).is_some_and(|s| s.contains("sculk")) {
                ok = false;
            }
        }
        if !ok {
            continue;
        }
        for q in &line {
            cells.insert(*q, "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into());
            cells.insert(add(*q, [0, -1, 0]), "minecraft:stone".into());
        }
        cells.insert(add(end, [0, -1, 0]), "minecraft:stone".into());
        if r.chance(0.5) {
            cells.insert(end, "minecraft:redstone_block".into());
        } else {
            let lf = match back {
                [1, 0, 0] => "east",
                [-1, 0, 0] => "west",
                [0, 0, 1] => "south",
                _ => "north",
            };
            // a floor lever (stands on the stone below), facing along the line
            cells.insert(end, format!("minecraft:lever[face=floor,facing={lf},powered=false]"));
            levers.push(end);
        }
    }
    // wiring next to the sensors and on solids
    let sensors: Vec<P3> = sensors.into_iter().filter(|p| cells.get(p).is_some_and(|s| s.contains("sculk"))).collect();
    // amethyst next to some sensors (resonance: a `resonate_<frequency>` event when it fires) and
    // wool shells around others (occlusion)
    for sp in &sensors {
        if r.chance(0.25) {
            let d = *r.pick(&["north", "south", "west", "east", "up", "down"]);
            let q = add(*sp, step_of(d));
            if inside(q) && !cells.contains_key(&q) {
                cells.insert(q, "minecraft:amethyst_block".into());
            }
        }
        if r.chance(0.15) {
            for d in ["north", "south", "west", "east", "up", "down"] {
                let q = add(*sp, step_of(d));
                if inside(q) && !cells.contains_key(&q) && r.chance(0.8) {
                    cells.insert(q, "minecraft:white_wool".into());
                }
            }
        }
    }
    for sp in &sensors {
        if !r.chance(0.6) {
            continue;
        }
        let d = *r.pick(hfac);
        let q = add(*sp, step_of(d));
        let below = add(q, [0, -1, 0]);
        if !inside(q) || cells.contains_key(&q) {
            continue;
        }
        let toward = match d {
            "north" => "south",
            "south" => "north",
            "west" => "east",
            _ => "west",
        };
        match r.below(4) {
            0 | 1 => {
                // a comparator reading the sensor (input from the sensor's side), on a floor
                if inside(below) && !cells.get(&below).is_some_and(|s| s.contains("sculk")) {
                    cells.insert(below, "minecraft:stone".into());
                    cells.insert(q, format!("minecraft:comparator[facing={toward},mode={},powered=false]", if r.chance(0.7) { "compare" } else { "subtract" }));
                    let out = add(q, step_of(d));
                    let ob = add(out, [0, -1, 0]);
                    if inside(out) && !cells.contains_key(&out) && inside(ob) && r.chance(0.6) {
                        cells.insert(ob, "minecraft:stone".into());
                        cells.insert(out, "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into());
                    }
                }
            }
            2 => {
                // an observer watching the sensor
                cells.insert(q, format!("minecraft:observer[facing={toward},powered=false]"));
            }
            _ => {
                // dust next to the sensor (it points into it)
                if inside(below) && !cells.get(&below).is_some_and(|s| s.contains("sculk")) {
                    cells.insert(below, "minecraft:stone".into());
                    cells.insert(q, "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into());
                }
            }
        }
    }
    let solid: Vec<P3> = cells.iter().filter(|(_, s)| s.as_str() == "minecraft:stone").map(|(p, _)| *p).collect();
    let n = 2 + r.below(6);
    for _ in 0..n {
        if solid.is_empty() {
            break;
        }
        let sp = *r.pick(&solid);
        let up = add(sp, [0, 1, 0]);
        let side_f = *r.pick(hfac);
        let side = add(sp, step_of(side_f));
        match r.below(5) {
            0 | 1 => {
                if inside(up) && !cells.contains_key(&up) {
                    cells.insert(up, "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into());
                }
            }
            2 => {
                if inside(up) && !cells.contains_key(&up) {
                    cells.insert(up, format!("minecraft:repeater[delay={},facing={},locked=false,powered=false]", 1 + r.below(4), r.pick(hfac)));
                }
            }
            3 => {
                if inside(side) && !cells.contains_key(&side) {
                    cells.insert(side, format!("minecraft:lever[face=wall,facing={side_f},powered=false]"));
                    levers.push(side);
                }
            }
            _ => {
                if inside(up) && !cells.contains_key(&up) {
                    cells.insert(up, "minecraft:redstone_torch[lit=true]".into());
                }
            }
        }
    }
    let blocks: Vec<(P3, String)> = cells.into_iter().collect();
    let mut hot: Vec<P3> = Vec::new();
    for (p, s) in &blocks {
        if s.contains("piston[") {
            let f = s.split("facing=").nth(1).and_then(|t| t.split(&[',', ']'][..]).next()).unwrap_or("up");
            let fr = step_of(f);
            for d in ["north", "south", "west", "east", "up", "down"] {
                let v = step_of(d);
                if v == fr {
                    continue;
                }
                hot.push(add(*p, v));
                if d != "down" {
                    hot.push(add(add(*p, v), [0, 1, 0]));
                }
            }
        }
    }
    hot.retain(|p| inside(*p));
    let mut timed: Vec<(u32, Step)> = Vec::new();
    let n = 6 + r.below(12);
    for _ in 0..n {
        let t = r.below(100) as u32;
        if !levers.is_empty() && r.chance(0.3) {
            timed.push((t, Step::Use(*r.pick(&levers))));
            continue;
        }
        let p = if !hot.is_empty() && r.chance(0.75) { *r.pick(&hot) } else { *r.pick(&all) };
        // an extended piston's base removed: its head breaks without a drop (`block_destroy`)
        if r.chance(0.1) {
            let ext: Vec<P3> = blocks.iter().filter(|(_, s)| s.contains("piston[extended=true")).map(|(q, _)| *q).collect();
            if !ext.is_empty() {
                timed.push((t, Step::Set(*r.pick(&ext), "minecraft:air".into())));
                continue;
            }
        }
        // only air, stone and redstone blocks get replaced (a removed piston head breaks its
        // piston: a drop, and drops end the comparison)
        if blocks.iter().any(|(q, s)| *q == p && !matches!(s.as_str(), "minecraft:stone" | "minecraft:redstone_block")) {
            continue;
        }
        match r.below(12) {
            0..=6 => {
                timed.push((t, Step::Set(p, "minecraft:redstone_block".into())));
                if r.chance(0.7) {
                    let k = r.below(6) as u32;
                    timed.push((t + k, Step::Set(p, "minecraft:air".into())));
                }
            }
            7..=8 => timed.push((t, Step::Set(p, "minecraft:air".into()))),
            9 => timed.push((t, Step::Set(p, "minecraft:white_wool".into()))),
            _ => timed.push((t, Step::Set(p, "minecraft:stone".into()))),
        }
    }
    timed.sort_by_key(|(t, _)| *t);
    let total = 130;
    let mut steps = Vec::new();
    let mut now = 0;
    for (t, s) in timed {
        if t > now {
            steps.push(Step::Tick(t - now));
            now = t;
        }
        steps.push(s);
    }
    steps.push(Step::Tick(total - now));
    let m = 2;
    Scenario {
        name: format!("sculk-{}{}-{seed}", if flat { "flat" } else { "box" }, if border { "-border" } else { "" }),
        lo: [ox - m, oy - m, oz - if flat { 1 } else { m }],
        hi: [ox + sx - 1 + m, oy + sy - 1 + m, oz + sz - 1 + if flat { 1 } else { m }],
        blocks,
        judge: true,
        steps,
    }
}

/// A door design: the judge's placement and settle, then `cycles` lever on / off flips, each
/// followed by a settle.
pub fn from_design(path: &std::path::Path, cycles: u32) -> Result<Scenario> {
    let d = crate::cli::load_design(path)?;
    if !d.entities.is_empty() {
        bail!("designs with entities are not modeled");
    }
    let lever = *d.ports.get("lever").context("no lever port")?;
    let (lo, hi) = d.bounds().context("empty design")?;
    let mut steps = vec![Step::Settle(400)];
    for _ in 0..cycles {
        steps.push(Step::Use(lever));
        steps.push(Step::Settle(300));
        steps.push(Step::Use(lever));
        steps.push(Step::Settle(300));
    }
    Ok(Scenario {
        name: path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
        lo: [lo[0] - 3, lo[1] - 3, lo[2] - 3],
        hi: [hi[0] + 3, hi[1] + 3, hi[2] + 3],
        blocks: d.block_list(),
        judge: true,
        steps,
    })
}

/// A gadget drive script: its design, then its inputs and its ticks (probes and expectations
/// are the drive tool's business: here every tick is compared).
pub fn from_drive(path: &std::path::Path) -> Result<Scenario> {
    let text = std::fs::read_to_string(path)?;
    let mut sc: Option<Scenario> = None;
    let mut probes: BTreeMap<String, ()> = BTreeMap::new();
    for raw in text.lines() {
        let line = raw.trim();
        let line = if line.starts_with('#') { "" } else { line.split(" # ").next().unwrap().trim() };
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> { Ok(parts.get(i).context("missing number")?.parse::<i32>()?) };
        let pos = |i: usize| -> Result<P3> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
        if parts[0] == "design" {
            let d = crate::cli::load_design(std::path::Path::new(parts[1]))?;
            if !d.entities.is_empty() {
                bail!("designs with entities are not modeled");
            }
            let d = if parts.len() >= 5 { d.translated([int(2)?, int(3)?, int(4)?]) } else { d };
            let (lo, hi) = d.bounds().context("empty design")?;
            sc = Some(Scenario {
                name: path.file_stem().unwrap_or_default().to_string_lossy().to_string(),
                lo: [lo[0] - 3, lo[1] - 3, lo[2] - 3],
                hi: [hi[0] + 3, hi[1] + 3, hi[2] + 3],
                blocks: d.block_list(),
                judge: true,
                steps: vec![Step::Settle(400)],
            });
            continue;
        }
        let Some(s) = sc.as_mut() else { bail!("command before the design: {line}") };
        match parts[0] {
            "set" => s.steps.push(Step::Set(pos(1)?, parts.get(4).context("state")?.to_string())),
            "clear" => s.steps.push(Step::Set(pos(1)?, "minecraft:air".into())),
            "setblock" => s.steps.push(Step::SetCmd(pos(1)?, parts.get(4).context("state")?.to_string())),
            "use" => s.steps.push(Step::Use(pos(1)?)),
            "step" | "quiet" | "trace" => s.steps.push(Step::Tick(int(1)? as u32)),
            "settle" => s.steps.push(Step::Settle(if parts.len() > 1 { int(1)? as u32 } else { 400 })),
            "expect_trace" => {
                // steps one tick per character of the trace
                let n = parts.get(2).map(|t| t.chars().count()).unwrap_or(0);
                s.steps.push(Step::Tick(n as u32));
            }
            "probe" | "probe_signal" => {
                probes.insert(parts.get(1).unwrap_or(&"").to_string(), ());
            }
            "summon" | "entities" | "etrace" | "expect_entity" | "expect_entities" => bail!("entity gadget: not modeled"),
            _ => {}
        }
    }
    sc.context("no design line")
}

// ---- text format (regressions) ----

impl Scenario {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# redsim differential scenario (mctas door pistfuzz --replay)");
        let _ = writeln!(s, "name {}", self.name);
        let _ = writeln!(s, "region {} {} {} {} {} {}", self.lo[0], self.lo[1], self.lo[2], self.hi[0], self.hi[1], self.hi[2]);
        if self.judge {
            let _ = writeln!(s, "judge");
        }
        for (p, b) in &self.blocks {
            let _ = writeln!(s, "block {} {} {} {}", p[0], p[1], p[2], b);
        }
        for st in &self.steps {
            let _ = match st {
                Step::Set(p, b) => writeln!(s, "set {} {} {} {}", p[0], p[1], p[2], b),
                Step::SetCmd(p, b) => writeln!(s, "setblock {} {} {} {}", p[0], p[1], p[2], b),
                Step::Use(p) => writeln!(s, "use {} {} {}", p[0], p[1], p[2]),
                Step::Tick(n) => writeln!(s, "tick {n}"),
                Step::Settle(n) => writeln!(s, "settle {n}"),
            };
        }
        s
    }

    pub fn from_text(text: &str) -> Result<Scenario> {
        let mut sc = Scenario { name: String::new(), lo: [0; 3], hi: [0; 3], blocks: Vec::new(), judge: false, steps: Vec::new() };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let w: Vec<&str> = line.split_whitespace().collect();
            let num = |i: usize| -> Result<i32> { w.get(i).context("missing field")?.parse::<i32>().with_context(|| format!("bad number in {line:?}")) };
            match w[0] {
                "name" => sc.name = w.get(1).unwrap_or(&"").to_string(),
                "region" => {
                    sc.lo = [num(1)?, num(2)?, num(3)?];
                    sc.hi = [num(4)?, num(5)?, num(6)?];
                }
                "judge" => sc.judge = true,
                "block" => sc.blocks.push(([num(1)?, num(2)?, num(3)?], w.get(4).context("state")?.to_string())),
                "set" => sc.steps.push(Step::Set([num(1)?, num(2)?, num(3)?], w.get(4).context("state")?.to_string())),
                "setblock" => sc.steps.push(Step::SetCmd([num(1)?, num(2)?, num(3)?], w.get(4).context("state")?.to_string())),
                "use" => sc.steps.push(Step::Use([num(1)?, num(2)?, num(3)?])),
                "tick" => sc.steps.push(Step::Tick(num(1)? as u32)),
                "settle" => sc.steps.push(Step::Settle(num(1)? as u32)),
                _ => bail!("bad line {line:?}"),
            }
        }
        Ok(sc)
    }
}

/// Shrink a failing scenario: drop steps and blocks one at a time, simplify states, then trim
/// the last ticks, while `fails` still holds.
pub fn shrink(sc: &Scenario, fails: &mut dyn FnMut(&Scenario) -> bool) -> Scenario {
    let mut cur = sc.clone();
    loop {
        let mut progress = false;
        let mut i = 0;
        while i < cur.steps.len() {
            if matches!(cur.steps[i], Step::Tick(_) | Step::Settle(_)) {
                i += 1;
                continue;
            }
            let mut c = cur.clone();
            c.steps.remove(i);
            if fails(&c) {
                cur = c;
                progress = true;
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < cur.blocks.len() {
            let mut c = cur.clone();
            c.blocks.remove(i);
            if fails(&c) {
                cur = c;
                progress = true;
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < cur.blocks.len() {
            let b = cur.blocks[i].1.clone();
            if b != "minecraft:stone" && !b.starts_with("minecraft:piston") && !b.starts_with("minecraft:sticky_piston") {
                let mut c = cur.clone();
                c.blocks[i].1 = "minecraft:stone".into();
                if fails(&c) {
                    cur = c;
                    progress = true;
                }
            }
            i += 1;
        }
        if !progress {
            break;
        }
    }
    // fewer final ticks
    while let Some(Step::Tick(n)) = cur.steps.last().cloned() {
        if n <= 1 {
            break;
        }
        let mut c = cur.clone();
        *c.steps.last_mut().unwrap() = Step::Tick(n / 2);
        if fails(&c) { cur = c } else { break }
    }
    cur
}

// ---- command line ----

pub struct CliOpts {
    pub count: u64,
    pub seed: u64,
    pub mode: String,
    pub slime: bool,
    pub gravity: bool,
    pub observers: bool,
    pub mixed: bool,
    pub shapes: bool,
    pub against: String,
    pub workers: usize,
    pub shrink: bool,
    pub replay: Vec<std::path::PathBuf>,
    pub gadgets: bool,
    pub designs: Vec<std::path::PathBuf>,
    pub cycles: u32,
    pub out_dir: std::path::PathBuf,
    pub max_fail: usize,
    pub trace: bool,
    /// Generate sculk scenarios (`gen_sculk`) instead of piston ones.
    pub sculk: bool,
    /// Sculk scenarios anywhere in the sandbox, its edge chunks included.
    pub border: bool,
}

/// Print each tick's changes of a run.
pub fn print_trace(sc: &Scenario, frames: &[Frame]) {
    println!("== {} ({} blocks, {} steps)", sc.name, sc.blocks.len(), sc.steps.len());
    let mut prev: BTreeMap<P3, String> = BTreeMap::new();
    for (t, f) in frames.iter().enumerate() {
        let mut line = String::new();
        let keys: std::collections::BTreeSet<&P3> = prev.keys().chain(f.blocks.keys()).collect();
        for k in keys {
            let (x, y) = (prev.get(k), f.blocks.get(k));
            if x != y && t > 0 {
                let _ = write!(line, " {:?}:{}->{}", k, x.map(|v| short(v)).unwrap_or("air"), y.map(|v| short(v)).unwrap_or("air"));
            }
        }
        if !f.moving.is_empty() {
            let _ = write!(line, " | moving {}", f.moving.len());
        }
        println!("f{:>3} ev{} st{} mv{}{}{}{line}", t + 1, f.block_events, f.block_ticks, f.moving_count, if f.drops { " DROPS" } else { "" }, if f.idle { " idle" } else { "" });
        prev = f.blocks.clone();
    }
}

/// Does the scenario contain a sculk sensor (placed or set)? Then a drop ends the comparison.
pub fn has_sensor(sc: &Scenario) -> bool {
    sc.blocks.iter().any(|(_, s)| s.contains("sculk_sensor"))
        || sc.steps.iter().any(|s| matches!(s, Step::Set(_, b) | Step::SetCmd(_, b) if b.contains("sculk_sensor")))
}

/// Sensor activations seen in a run (a cell turning `sculk_sensor_phase=active`).
pub fn count_activations(frames: &[Frame]) -> usize {
    let mut n = 0;
    let mut prev: Option<&Frame> = None;
    for f in frames {
        for (p, s) in &f.blocks {
            if s.contains("sculk_sensor_phase=active") && !prev.and_then(|q| q.blocks.get(p)).is_some_and(|t| t.contains("sculk_sensor_phase=active")) {
                n += 1;
            }
        }
        prev = Some(f);
    }
    n
}

fn other_run(o: &CliOpts, w: &mut Worker, sb: &mut Option<u32>, sc: &Scenario) -> Result<Vec<Frame>> {
    match o.against.as_str() {
        "bridge" => run_bridge(w, sb, sc),
        "model" => run_model(sc),
        a => bail!("--against {a}: expected model or bridge"),
    }
}

pub fn cli(version: &str, o: &CliOpts) -> Result<()> {
    let mut scenarios: Vec<Scenario> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    if !o.replay.is_empty() {
        for p in &o.replay {
            let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
            let mut sc = Scenario::from_text(&text)?;
            if sc.name.is_empty() {
                sc.name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            }
            scenarios.push(sc);
        }
    }
    if o.gadgets {
        let mut files: Vec<_> = std::fs::read_dir("designs/gadgets")?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "drive")).collect();
        files.sort();
        for f in files {
            match from_drive(&f) {
                Ok(sc) => scenarios.push(sc),
                Err(e) => skipped.push(format!("{}: {e:#}", f.display())),
            }
        }
    }
    for p in &o.designs {
        match from_design(p, o.cycles) {
            Ok(sc) => scenarios.push(sc),
            Err(e) => skipped.push(format!("{}: {e:#}", p.display())),
        }
    }
    if o.replay.is_empty() && !o.gadgets && o.designs.is_empty() {
        for s in o.seed..o.seed + o.count {
            let flat = match o.mode.as_str() {
                "flat" => true,
                "box" => false,
                _ => s % 2 == 0,
            };
            if o.sculk {
                scenarios.push(gen_sculk(s, flat, o.border));
                continue;
            }
            scenarios.push(gen_piston(s, GenOpts { flat, slime: o.slime, gravity: o.gravity, observers: o.observers, mixed: o.mixed, shapes: o.shapes }));
        }
    }
    for s in &skipped {
        println!("skipped {s}");
    }
    // scenarios with blocks redsim does not model are skipped (reported), not failed
    let mut runnable = Vec::new();
    for sc in scenarios {
        let unknown: Vec<&String> = sc
            .blocks
            .iter()
            .map(|(_, s)| s)
            .chain(sc.steps.iter().filter_map(|s| match s {
                Step::Set(_, b) | Step::SetCmd(_, b) => Some(b),
                _ => None,
            }))
            .filter(|s| redsim::Block::parse(s).is_err())
            .collect();
        if let Some(u) = unknown.first() {
            println!("skipped {}: {} not modeled by redsim", sc.name, short(u));
            continue;
        }
        runnable.push(sc);
    }
    let scenarios = runnable;
    let n = scenarios.len();
    if n == 0 {
        println!("pistfuzz: nothing to run");
        return Ok(());
    }
    let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
    let pool = mctas::sim::Pool::spawn(o.workers.min(n).max(1), &cfg)?;
    let t0 = std::time::Instant::now();
    let sandboxes: Vec<std::sync::Mutex<(Option<u32>, Option<u32>)>> = (0..pool.len()).map(|_| std::sync::Mutex::new((None, None))).collect();
    let failures = std::sync::atomic::AtomicUsize::new(0);
    let activations = std::sync::atomic::AtomicUsize::new(0);
    let results = pool.map(scenarios, |w, wi, sc| -> Result<(Scenario, Option<String>, usize)> {
        if failures.load(std::sync::atomic::Ordering::Relaxed) >= o.max_fail {
            return Ok((sc, None, 0));
        }
        let mut sbs = sandboxes[wi].lock().unwrap();
        let a = run_bridge(w, &mut sbs.0, &sc)?;
        if o.trace {
            print_trace(&sc, &a);
        }
        let b = match other_run(o, w, &mut sbs.1, &sc) {
            Ok(b) => b,
            Err(e) if format!("{e:#}").contains("is not modeled") => return Ok((sc, Some(format!("SKIP {e:#}")), a.len())),
            Err(e) => return Ok((sc, Some(format!("model failed: {e:#}")), a.len())),
        };
        if o.trace {
            println!("-- {}:", o.against);
            print_trace(&sc, &b);
        }
        let stop = has_sensor(&sc);
        let (d, compared) = compare_upto(&a, &b, stop);
        if d.is_none() {
            activations.fetch_add(count_activations(&a[..compared.min(a.len())]), std::sync::atomic::Ordering::Relaxed);
            return Ok((sc, None, compared));
        }
        failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if !o.shrink {
            return Ok((sc, d, compared));
        }
        let (mut s0, mut s1) = (sbs.0, sbs.1);
        let mut fails = |c: &Scenario| -> bool {
            let a = run_bridge(w, &mut s0, c);
            let b = other_run(o, w, &mut s1, c);
            match (a, b) {
                (Ok(a), Ok(b)) => compare_upto(&a, &b, has_sensor(c)).0.is_some(),
                (Ok(_), Err(_)) => true,
                _ => false,
            }
        };
        let small = shrink(&sc, &mut fails);
        let a = run_bridge(w, &mut s0, &small)?;
        let d2 = match other_run(o, w, &mut s1, &small) {
            Ok(b) => compare_upto(&a, &b, has_sensor(&small)).0,
            Err(e) => Some(format!("model failed: {e:#}")),
        };
        sbs.0 = s0;
        sbs.1 = s1;
        Ok((small, d2.or(d), compared))
    });
    let (mut ok, mut bad, mut errors, mut frames, mut unmodeled) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for r in results {
        match r {
            Ok((_, None, f)) => {
                ok += 1;
                frames += f;
            }
            Ok((sc, Some(d), _)) if d.starts_with("SKIP") => {
                if unmodeled < 5 {
                    println!("skipped {}: {}", sc.name, d.trim_start_matches("SKIP "));
                }
                unmodeled += 1
            }
            Ok((sc, Some(d), f)) => {
                bad += 1;
                frames += f;
                std::fs::create_dir_all(&o.out_dir)?;
                let path = o.out_dir.join(format!("{}.txt", sc.name));
                std::fs::write(&path, format!("# {d}\n{}", sc.to_text()))?;
                println!("MISMATCH {} ({} blocks, {} steps): {d}  [{}]", sc.name, sc.blocks.len(), sc.steps.len(), path.display());
            }
            Err(e) => {
                errors += 1;
                println!("ERROR {e:#}");
            }
        }
    }
    let acts = activations.load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "pistfuzz: {ok} identical, {bad} mismatching, {errors} errors, {unmodeled} skipped (an input redsim does not model) of {n} ({frames} ticks compared{}, {:.1}s, against {})",
        if acts > 0 { format!(", {acts} sculk sensor activations in identical runs") } else { String::new() },
        t0.elapsed().as_secs_f64(),
        o.against
    );
    if bad > 0 || errors > 0 {
        std::process::exit(1);
    }
    Ok(())
}
