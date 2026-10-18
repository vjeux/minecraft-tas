//! `mctas door pistfuzz`: differential fuzzing of the redsim piston model against the real game
//! (the bridge sandbox): random piston setups with scripted power changes, every block state,
//! every moving block entity and the pending queue sizes compared after every tick; mismatches
//! shrunk to minimal scenarios (track pistmodel).

use anyhow::{Context, Result, bail};
use mctas::sim::{PlaceMode, SandboxOptions, Worker};
use serde_json::json;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub type P3 = [i32; 3];

/// One scripted run: blocks placed raw, a self update of every block (list order), then per tick
/// the actions due before it, then the tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    /// Compared region (inclusive).
    pub lo: P3,
    pub hi: P3,
    pub blocks: Vec<(P3, String)>,
    /// (before tick index t (0-based), action).
    pub actions: Vec<(u32, Action)>,
    pub ticks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// `setBlock(pos, state, 3)` (a player-like placement or removal: `minecraft:air`).
    Set(P3, String),
    /// A lever or button click.
    Use(P3),
}

/// What both simulators report after a tick.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Frame {
    /// Non-air blocks of the region, sorted by position.
    pub blocks: BTreeMap<P3, String>,
    /// Moving blocks: pos -> "state ext src dir progress(halves)".
    pub moving: BTreeMap<P3, String>,
    pub block_events: u32,
    pub block_ticks: u32,
    pub moving_count: u32,
    /// Any item dropped so far (the bridge: entities that are not falling blocks).
    pub drops: bool,
}

/// A simulator under test.
pub trait Sim {
    fn place_raw(&mut self, blocks: &[(P3, String)]) -> Result<()>;
    fn update_self(&mut self, positions: &[P3]) -> Result<()>;
    fn set_update(&mut self, pos: P3, state: &str) -> Result<()>;
    fn use_block(&mut self, pos: P3) -> Result<()>;
    fn tick(&mut self, lo: P3, hi: P3) -> Result<Frame>;
}

/// Run a scenario: one frame per tick (frame 0 = after the placement and the self updates,
/// before any tick).
pub fn run(sim: &mut dyn Sim, sc: &Scenario) -> Result<Vec<Frame>> {
    sim.place_raw(&sc.blocks)?;
    let positions: Vec<P3> = sc.blocks.iter().map(|(p, _)| *p).collect();
    sim.update_self(&positions)?;
    let mut frames = Vec::with_capacity(sc.ticks as usize);
    for t in 0..sc.ticks {
        for (at, a) in &sc.actions {
            if *at == t {
                match a {
                    Action::Set(p, s) => sim.set_update(*p, s)?,
                    Action::Use(p) => sim.use_block(*p)?,
                }
            }
        }
        frames.push(sim.tick(sc.lo, sc.hi)?);
    }
    Ok(frames)
}

/// The first difference between two runs.
pub fn compare(a: &[Frame], b: &[Frame]) -> Option<String> {
    for (t, (fa, fb)) in a.iter().zip(b.iter()).enumerate() {
        if fa == fb {
            continue;
        }
        let mut s = format!("tick {}:", t + 1);
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
        if (fa.block_events, fa.block_ticks, fa.moving_count, fa.drops) != (fb.block_events, fb.block_ticks, fb.moving_count, fb.drops) {
            let _ = write!(
                s,
                " queues game=(events {}, ticks {}, moving {}, drops {}) model=(events {}, ticks {}, moving {}, drops {})",
                fa.block_events, fa.block_ticks, fa.moving_count, fa.drops, fb.block_events, fb.block_ticks, fb.moving_count, fb.drops
            );
        }
        return Some(s);
    }
    if a.len() != b.len() {
        return Some(format!("frame counts differ: {} vs {}", a.len(), b.len()));
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
    fn place_raw(&mut self, blocks: &[(P3, String)]) -> Result<()> {
        if !blocks.is_empty() {
            self.w.set_blocks(self.sb, blocks, PlaceMode::Raw)?;
        }
        Ok(())
    }
    fn update_self(&mut self, positions: &[P3]) -> Result<()> {
        if !positions.is_empty() {
            self.w.update(self.sb, positions, "self")?;
        }
        Ok(())
    }
    fn set_update(&mut self, pos: P3, state: &str) -> Result<()> {
        self.w.set_blocks(self.sb, &[(pos, state)], PlaceMode::Update)?;
        Ok(())
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
        let (sx, _sy, sz) = (size[0] as i32, size[1] as i32, size[2] as i32);
        for (i, c) in bytes.chunks_exact(2).enumerate() {
            let idx = u16::from_le_bytes([c[0], c[1]]) as usize;
            let s = &palette[idx];
            if s == "minecraft:air" {
                continue;
            }
            let i = i as i32;
            let x = i % sx;
            let z = (i / sx) % sz;
            let y = i / (sx * sz);
            f.blocks.insert([lo[0] + x, lo[1] + y, lo[2] + z], s.clone());
        }
        for m in g["moving"].as_array().cloned().unwrap_or_default() {
            let p: Vec<i32> = m["pos"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap() as i32).collect();
            let prog = (m["progress"].as_f64().unwrap_or(0.0) * 2.0).round() as i32;
            f.moving.insert(
                [p[0], p[1], p[2]],
                format!(
                    "{} {} {} {} {}",
                    short(m["state"].as_str().unwrap_or("")),
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
        f.drops = ents["entities"].as_array().is_some_and(|v| v.iter().any(|e| e["type"].as_str() != Some("minecraft:falling_block")));
        Ok(f)
    }
}

/// Run on a fresh sandbox of the worker (created once per worker, cleared between runs).
pub fn run_bridge(w: &mut Worker, sb: &mut Option<u32>, sc: &Scenario) -> Result<Vec<Frame>> {
    let id = match *sb {
        Some(id) => {
            w.clear(id)?;
            id
        }
        None => {
            let id = w.create(&SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
            *sb = Some(id);
            id
        }
    };
    let mut sim = BridgeSim { w, sb: id };
    run(&mut sim, sc)
}

// ---- generator ----

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
}

const HORIZ: [&str; 4] = ["north", "south", "west", "east"];

fn facings(flat: bool) -> &'static [&'static str] {
    if flat { &["east", "west", "up", "down"] } else { &["north", "south", "west", "east", "up", "down"] }
}

fn step(f: &str) -> P3 {
    match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        "up" => [0, 1, 0],
        _ => [0, -1, 0],
    }
}

/// A random piston setup.
pub fn gen_piston(seed: u64, o: GenOpts) -> Scenario {
    let mut r = Rng::new(seed);
    let (sx, sy, sz) = if o.flat { (7, 7, 1) } else { (6, 6, 3) };
    // anywhere in the sandbox: the vacated-cell order (a Java HashMap) depends on positions
    let ox = -12 + r.below(10) as i32;
    let oz = -12 + r.below(10) as i32;
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
    for &p in &all {
        if cells.contains_key(&p) {
            continue;
        }
        let roll = r.below(1000);
        let s: String = if roll < 380 {
            continue;
        } else if roll < 520 {
            "minecraft:stone".into()
        } else if roll < 760 {
            let sticky = r.chance(0.5);
            let f = *r.pick(fac);
            let name = if sticky { "sticky_piston" } else { "piston" };
            let front = [p[0] + step(f)[0], p[1] + step(f)[1], p[2] + step(f)[2]];
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
            if o.observers {
                format!("minecraft:observer[facing={},powered=false]", r.pick(fac))
            } else {
                "minecraft:stone".into()
            }
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
    let blocks: Vec<(P3, String)> = cells.into_iter().collect();
    // power actions: redstone blocks set / cleared (mostly next to a piston or its QC cell),
    // short pulses of 0..5 gt, blocks placed or removed
    let mut hot: Vec<P3> = Vec::new();
    for (p, s) in &blocks {
        if s.contains("piston[") {
            let f = s.split("facing=").nth(1).and_then(|r| r.split(&[',', ']'][..]).next()).unwrap_or("up");
            let fr = step(f);
            for d in ["north", "south", "west", "east", "up", "down"] {
                let v = step(d);
                if v == fr {
                    continue;
                }
                hot.push([p[0] + v[0], p[1] + v[1], p[2] + v[2]]);
                // quasi-connectivity: the neighbors of the cell above
                if d != "down" {
                    hot.push([p[0] + v[0], p[1] + 1 + v[1], p[2] + v[2]]);
                }
            }
        }
    }
    hot.retain(|p| inside(*p));
    let mut actions: Vec<(u32, Action)> = Vec::new();
    let n = 5 + r.below(10);
    for _ in 0..n {
        let t = r.below(30) as u32;
        let p = if !hot.is_empty() && r.chance(0.75) { *r.pick(&hot) } else { *r.pick(&all) };
        match r.below(12) {
            0..=6 => {
                actions.push((t, Action::Set(p, "minecraft:redstone_block".into())));
                if r.chance(0.7) {
                    let k = r.below(6) as u32;
                    actions.push((t + k, Action::Set(p, "minecraft:air".into())));
                }
            }
            7..=8 => actions.push((t, Action::Set(p, "minecraft:air".into()))),
            9 => {
                let sticky = r.chance(0.5);
                let f = *r.pick(fac);
                actions.push((t, Action::Set(p, format!("minecraft:{}[extended=false,facing={f}]", if sticky { "sticky_piston" } else { "piston" }))));
            }
            _ => actions.push((t, Action::Set(p, "minecraft:stone".into()))),
        }
    }
    actions.sort_by_key(|(t, _)| *t);
    let m = 4;
    Scenario {
        name: format!("piston-{}-{seed}", if o.flat { "flat" } else { "box" }),
        lo: [ox - m, oy - m, oz - if o.flat { 1 } else { m }],
        hi: [ox + sx - 1 + m, oy + sy - 1 + m, oz + sz - 1 + if o.flat { 1 } else { m }],
        blocks,
        actions,
        ticks: 48,
    }
}

// ---- text format (regressions) ----

impl Scenario {
    pub fn to_text(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# redsim differential scenario (mctas door pistfuzz)");
        let _ = writeln!(s, "name {}", self.name);
        let _ = writeln!(s, "region {} {} {} {} {} {}", self.lo[0], self.lo[1], self.lo[2], self.hi[0], self.hi[1], self.hi[2]);
        let _ = writeln!(s, "ticks {}", self.ticks);
        for (p, b) in &self.blocks {
            let _ = writeln!(s, "block {} {} {} {}", p[0], p[1], p[2], b);
        }
        for (t, a) in &self.actions {
            match a {
                Action::Set(p, b) => {
                    let _ = writeln!(s, "at {t} set {} {} {} {}", p[0], p[1], p[2], b);
                }
                Action::Use(p) => {
                    let _ = writeln!(s, "at {t} use {} {} {}", p[0], p[1], p[2]);
                }
            }
        }
        s
    }

    pub fn from_text(text: &str) -> Result<Scenario> {
        let mut sc = Scenario { name: String::new(), lo: [0; 3], hi: [0; 3], blocks: Vec::new(), actions: Vec::new(), ticks: 40 };
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
                "ticks" => sc.ticks = num(1)? as u32,
                "block" => sc.blocks.push(([num(1)?, num(2)?, num(3)?], w.get(4).context("state")?.to_string())),
                "at" => {
                    let t = num(1)? as u32;
                    match w.get(2).copied() {
                        Some("set") => sc.actions.push((t, Action::Set([num(3)?, num(4)?, num(5)?], w.get(6).context("state")?.to_string()))),
                        Some("use") => sc.actions.push((t, Action::Use([num(3)?, num(4)?, num(5)?]))),
                        _ => bail!("bad action {line:?}"),
                    }
                }
                _ => bail!("bad line {line:?}"),
            }
        }
        Ok(sc)
    }
}

/// Shrink a failing scenario: drop blocks and actions one at a time (then fewer ticks) while
/// `fails` still holds.
pub fn shrink(sc: &Scenario, fails: &mut dyn FnMut(&Scenario) -> bool) -> Scenario {
    let mut cur = sc.clone();
    loop {
        let mut progress = false;
        let mut i = 0;
        while i < cur.actions.len() {
            let mut c = cur.clone();
            c.actions.remove(i);
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
        // simplify block states: pistons -> stone, extended -> retracted
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
    // fewest ticks that still fail
    while cur.ticks > 1 {
        let mut c = cur.clone();
        c.ticks -= 1;
        if fails(&c) { cur = c } else { break }
    }
    cur
}

// ---- the model side ----

/// The redsim side of a scenario.
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

impl Sim for ModelSim {
    fn place_raw(&mut self, blocks: &[(P3, String)]) -> Result<()> {
        for (p, s) in blocks {
            let b = parse_block(s)?;
            self.w.set_block(bp(*p), b, redsim::world::flags::RAW);
        }
        Ok(())
    }
    fn update_self(&mut self, positions: &[P3]) -> Result<()> {
        for p in positions {
            let k = self.w.get(bp(*p)).kind();
            self.w.neighbor_changed(bp(*p), k);
        }
        Ok(())
    }
    fn set_update(&mut self, pos: P3, state: &str) -> Result<()> {
        let b = parse_block(state)?;
        self.w.set_block(bp(pos), b, redsim::world::flags::ALL);
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

// ---- command line ----

pub struct CliOpts {
    pub count: u64,
    pub seed: u64,
    pub mode: String,
    pub slime: bool,
    pub gravity: bool,
    pub observers: bool,
    pub against: String,
    pub workers: usize,
    pub shrink: bool,
    pub replay: Vec<std::path::PathBuf>,
    pub out_dir: std::path::PathBuf,
    pub max_fail: usize,
    pub trace: bool,
}

/// Print each tick's changes of a run.
pub fn print_trace(sc: &Scenario, frames: &[Frame]) {
    println!("== {} ({} blocks, {} actions)", sc.name, sc.blocks.len(), sc.actions.len());
    let mut prev: BTreeMap<P3, String> = sc.blocks.iter().cloned().collect();
    for (t, f) in frames.iter().enumerate() {
        let mut line = String::new();
        for (at, a) in &sc.actions {
            if *at == t as u32 {
                let _ = write!(line, " [{a:?}]");
            }
        }
        let keys: std::collections::BTreeSet<&P3> = prev.keys().chain(f.blocks.keys()).collect();
        for k in keys {
            let (x, y) = (prev.get(k), f.blocks.get(k));
            if x != y {
                let _ = write!(line, " {:?}:{}->{}", k, x.map(|v| short(v)).unwrap_or("air"), y.map(|v| short(v)).unwrap_or("air"));
            }
        }
        if !f.moving.is_empty() {
            let _ = write!(line, " | moving {}", f.moving.len());
        }
        println!("t{:>2} ev{} st{} mv{}{}{line}", t + 1, f.block_events, f.block_ticks, f.moving_count, if f.drops { " DROPS" } else { "" });
        prev = f.blocks.clone();
    }
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
    if !o.replay.is_empty() {
        for p in &o.replay {
            let text = std::fs::read_to_string(p).with_context(|| format!("reading {}", p.display()))?;
            let mut sc = Scenario::from_text(&text)?;
            if sc.name.is_empty() {
                sc.name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
            }
            scenarios.push(sc);
        }
    } else {
        for s in o.seed..o.seed + o.count {
            let flat = match o.mode.as_str() {
                "flat" => true,
                "box" => false,
                _ => s % 2 == 0,
            };
            scenarios.push(gen_piston(s, GenOpts { flat, slime: o.slime, gravity: o.gravity, observers: o.observers }));
        }
    }
    let n = scenarios.len();
    let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
    let pool = mctas::sim::Pool::spawn(o.workers.min(n).max(1), &cfg)?;
    let t0 = std::time::Instant::now();
    let sandboxes: Vec<std::sync::Mutex<(Option<u32>, Option<u32>)>> = (0..pool.len()).map(|_| std::sync::Mutex::new((None, None))).collect();
    let failures = std::sync::atomic::AtomicUsize::new(0);
    let results = pool.map(scenarios, |w, wi, sc| -> Result<(Scenario, Option<String>)> {
        if failures.load(std::sync::atomic::Ordering::Relaxed) >= o.max_fail {
            return Ok((sc, None));
        }
        let mut sbs = sandboxes[wi].lock().unwrap();
        let a = run_bridge(w, &mut sbs.0, &sc)?;
        if o.trace {
            print_trace(&sc, &a);
        }
        let b = other_run(o, w, &mut sbs.1, &sc)?;
        let d = compare(&a, &b);
        if d.is_none() {
            return Ok((sc, None));
        }
        failures.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if !o.shrink {
            return Ok((sc, d));
        }
        let (mut s0, mut s1) = (sbs.0, sbs.1);
        let mut fails = |c: &Scenario| -> bool {
            let a = run_bridge(w, &mut s0, c);
            let b = other_run(o, w, &mut s1, c);
            match (a, b) {
                (Ok(a), Ok(b)) => compare(&a, &b).is_some(),
                _ => false,
            }
        };
        let small = shrink(&sc, &mut fails);
        let a = run_bridge(w, &mut s0, &small)?;
        let b = other_run(o, w, &mut s1, &small)?;
        sbs.0 = s0;
        sbs.1 = s1;
        let d2 = compare(&a, &b).or(d);
        Ok((small, d2))
    });
    let mut ok = 0usize;
    let mut bad = 0usize;
    let mut errors = 0usize;
    for r in results {
        match r {
            Ok((_, None)) => ok += 1,
            Ok((sc, Some(d))) => {
                bad += 1;
                std::fs::create_dir_all(&o.out_dir)?;
                let path = o.out_dir.join(format!("{}.txt", sc.name));
                std::fs::write(&path, format!("# {d}\n{}", sc.to_text()))?;
                println!("MISMATCH {} ({} blocks, {} actions): {d}  [{}]", sc.name, sc.blocks.len(), sc.actions.len(), path.display());
            }
            Err(e) => {
                errors += 1;
                println!("ERROR {e:#}");
            }
        }
    }
    println!(
        "pistfuzz: {ok} identical, {bad} mismatching, {errors} errors of {n} ({:.1}s, against {})",
        t0.elapsed().as_secs_f64(),
        o.against
    );
    if bad > 0 || errors > 0 {
        std::process::exit(1);
    }
    Ok(())
}
