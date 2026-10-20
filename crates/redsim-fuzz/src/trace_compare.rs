//! `redsim-fuzz trace-compare <exact traces>...` (track clk-misc, the fluids + entities port's
//! gate): replay a design in redsim and compare it, tick by tick, with an exact real-game trace
//! (`mctas clock misc trace-export --exact`, designs/clocks/traces/exact/<stem>.trace).
//!
//! The design is built like `clock sim` / `clock verify` (raw placement, block-entity data, a
//! `self` update of every block; the design's `entity:` lines summoned once the blocks stand,
//! once redsim has entities), the kick's inputs are applied between ticks, and frame t (the state
//! after t ticks, the inputs due at t applied) is compared with the trace's line t: EVERY cell of
//! the trace's watched box (the trace lists every non-air cell at t = 0 and the changed cells
//! after that), then every entity (same count and order, type, position, motion and rotations
//! bit for bit). Prints `OK` (every tick identical), the first divergence, or what redsim does
//! not model yet.

use anyhow::{Context, Result, bail};
use clock::{Action, Clock, Placement};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;
use std::path::Path;

/// One entity of a trace line (exact values).
#[derive(Clone, Debug, PartialEq)]
pub struct TraceEntity {
    pub id: u64,
    pub kind: String,
    pub pos: [f64; 3],
    pub motion: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    /// The game's network id (`getId()`), when the trace has it (`net_id`).
    pub net_id: Option<i32>,
}

/// A parsed exact trace.
pub struct Trace {
    pub name: String,
    pub design: String,
    pub period: u32,
    pub warmup: u32,
    pub from: [i32; 3],
    pub to: [i32; 3],
    /// Per tick: the cells that changed (all non-air cells at t = 0), and every entity.
    pub frames: Vec<(Vec<([i32; 3], String)>, Vec<TraceEntity>)>,
}

fn f3(v: &serde_json::Value) -> Option<[f64; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?, a.get(2)?.as_f64()?])
}

fn i3(v: &serde_json::Value) -> Option<[i32; 3]> {
    let a = v.as_array()?;
    Some([a.first()?.as_i64()? as i32, a.get(1)?.as_i64()? as i32, a.get(2)?.as_i64()? as i32])
}

pub fn load_trace(path: &Path) -> Result<Trace> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut lines = text.lines();
    let h: serde_json::Value = serde_json::from_str(lines.next().context("empty trace")?)?;
    let bx = h["box"].as_array().context("header: box")?;
    let mut t = Trace {
        name: h["name"].as_str().unwrap_or("").to_string(),
        design: h["design"].as_str().context("header: design")?.to_string(),
        period: h["period"].as_u64().unwrap_or(0) as u32,
        warmup: h["warmup"].as_u64().unwrap_or(0) as u32,
        from: i3(&bx[0]).context("header: box")?,
        to: i3(&bx[1]).context("header: box")?,
        frames: Vec::new(),
    };
    for (i, line) in lines.enumerate() {
        let v: serde_json::Value = serde_json::from_str(line).with_context(|| format!("line {}", i + 2))?;
        let mut blocks = Vec::new();
        for b in v["blocks"].as_array().context("blocks")? {
            let a = b.as_array().context("block")?;
            let p = [a[0].as_i64().context("x")? as i32, a[1].as_i64().context("y")? as i32, a[2].as_i64().context("z")? as i32];
            blocks.push((p, a[3].as_str().context("state")?.to_string()));
        }
        let mut ents = Vec::new();
        for e in v["entities"].as_array().context("entities")? {
            let motion = f3(&e["motion"]).context("entity motion: an exact trace (trace-export --exact) is needed")?;
            ents.push(TraceEntity {
                id: e["id"].as_u64().unwrap_or(0),
                kind: e["type"].as_str().unwrap_or("").to_string(),
                pos: f3(&e["pos"]).context("entity pos")?,
                motion,
                yaw: e["yaw"].as_f64().unwrap_or(0.0) as f32,
                pitch: e["pitch"].as_f64().unwrap_or(0.0) as f32,
                net_id: e["net_id"].as_i64().map(|n| n as i32),
            });
        }
        t.frames.push((blocks, ents));
    }
    Ok(t)
}

fn bp(p: [i32; 3]) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// The game's state string through redsim's parser (so formatting differences never count), or
/// `?state` for a block redsim does not model.
fn canon(cache: &mut HashMap<String, String>, s: &str) -> String {
    if let Some(c) = cache.get(s) {
        return c.clone();
    }
    let c = match Block::parse(s) {
        Ok(b) => b.to_string(),
        Err(_) => format!("?{s}"),
    };
    cache.insert(s.to_string(), c.clone());
    c
}

/// The outcome for one trace.
pub enum Outcome {
    /// Every tick identical.
    Same { ticks: usize },
    /// The first divergence: the tick and up to a few differences.
    Diverges { tick: usize, diffs: Vec<String>, more: usize },
    /// redsim cannot build or run it yet.
    Unmodeled(String),
}

/// The design's entities as redsim entities (`Sandbox.summon`: created at their position, at
/// rest, in the design's order, once the blocks stand; `redsim::entity::spawn`, the clock sim's
/// path too); an error names what is missing.
fn summon(w: &mut World, c: &Clock) -> Result<(), String> {
    for e in &c.design.entities {
        // spawn names any data it does not model (Motion, Rotation; items' and containers' own)
        redsim::entity::spawn(w, &e.kind, e.pos, e.nbt.as_deref())?;
    }
    Ok(())
}

/// The live entities of redsim's world, in tick order.
fn entities_of(w: &World) -> Vec<TraceEntity> {
    use redsim::entity::EntityKind;
    // every entity in id (spawn) order: falling blocks ([`redsim::gravity`]) share the id counter
    let mut out: Vec<(u32, TraceEntity)> = w
        .entities
        .list
        .iter()
        .filter(|e| !e.removed)
        .map(|e| {
            (
                e.id,
                TraceEntity {
                    id: e.id as u64,
                    kind: match &e.kind {
                        EntityKind::Boat(b) => if b.chest { "minecraft:oak_chest_boat" } else { "minecraft:oak_boat" }.to_string(),
                        EntityKind::Minecart(m) => if m.hopper { "minecraft:hopper_minecart" } else { "minecraft:minecart" }.to_string(),
                        EntityKind::Item(_) => "minecraft:item".to_string(),
                        EntityKind::ArmorStand(_) => "minecraft:armor_stand".to_string(),
                    },
                    pos: [e.pos.x, e.pos.y, e.pos.z],
                    motion: [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z],
                    yaw: e.y_rot,
                    pitch: e.x_rot,
                    net_id: Some(e.network_id),
                },
            )
        })
        .collect();
    out.extend(w.gravity.entities.iter().filter(|f| !f.removed).map(|f| {
        (
            f.id,
            TraceEntity {
                id: f.id as u64,
                kind: "minecraft:falling_block".to_string(),
                pos: [f.x, f.y, f.z],
                motion: [0.0, f.vy, 0.0],
                yaw: 0.0,
                pitch: 0.0,
                net_id: None,
            },
        )
    }));
    out.sort_by_key(|(id, _)| *id);
    out.into_iter().map(|(_, e)| e).collect()
}

fn entity_diffs(want: &[TraceEntity], have: &[TraceEntity], out: &mut Vec<String>) {
    if want.len() != have.len() {
        out.push(format!("{} entities in the game, {} in redsim", want.len(), have.len()));
    }
    for (i, (g, r)) in want.iter().zip(have).enumerate() {
        let short = |k: &str| k.trim_start_matches("minecraft:").to_string();
        if g.kind != r.kind {
            out.push(format!("entity {i}: {} in the game, {} in redsim", short(&g.kind), short(&r.kind)));
            continue;
        }
        let bits = |a: [f64; 3], b: [f64; 3]| (0..3).all(|k| a[k].to_bits() == b[k].to_bits() || (a[k] == 0.0 && b[k] == 0.0));
        if !bits(g.pos, r.pos) {
            out.push(format!("{} {i} pos: game {:?}, redsim {:?}", short(&g.kind), g.pos, r.pos));
        }
        if !bits(g.motion, r.motion) {
            out.push(format!("{} {i} motion: game {:?}, redsim {:?}", short(&g.kind), g.motion, r.motion));
        }
        if g.yaw.to_bits() != r.yaw.to_bits() || g.pitch.to_bits() != r.pitch.to_bits() {
            out.push(format!("{} {i} rotation: game ({}, {}), redsim ({}, {})", short(&g.kind), g.yaw, g.pitch, r.yaw, r.pitch));
        }
    }
}

/// Replay `trace` in redsim and compare.
pub fn compare(trace: &Trace, max_diffs: usize) -> Result<Outcome> {
    let c = Clock::load(Path::new(&trace.design)).with_context(|| format!("load {}", trace.design))?;
    let mut blocks = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for (p, s) in &c.design.blocks {
        match Block::parse(s) {
            Ok(b) => blocks.push((bp(*p), b)),
            Err(e) => {
                if !bad.contains(&e) {
                    bad.push(e)
                }
            }
        }
    }
    if !bad.is_empty() {
        return Ok(Outcome::Unmodeled(format!("blocks: {}", bad.join("; "))));
    }
    let (from, to) = (trace.from, trace.to);
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut w = World::new(bp(from), size);
    let last = c.withheld().map(bp);
    let be: Vec<(BlockPos, &str)> = c.design.block_entities.iter().map(|(p, s)| (bp(*p), s.as_str())).filter(|(p, _)| Some(*p) != last).collect();
    match c.placement {
        Placement::Judge => {
            let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
            w.place_raw(&silent);
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| anyhow::anyhow!("nbt: {e}"))?;
            w.place_updates(&silent);
        }
        Placement::Player => {
            for (p, b) in blocks.iter().filter(|(p, _)| Some(*p) != last) {
                w.set_block(*p, *b, flags::ALL);
            }
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| anyhow::anyhow!("nbt: {e}"))?;
        }
    }
    if let Err(e) = summon(&mut w, &c) {
        return Ok(Outcome::Unmodeled(e));
    }
    // the game's network ids (some logic reads them: ItemEntity's 1-in-4 move at rest), from the
    // first frame, in the summon order (falling blocks carry no network id here)
    if let Some((_, ents)) = trace.frames.first() {
        let mut k = 0;
        for te in ents.iter().filter(|te| te.kind != "minecraft:falling_block") {
            if let (Some(n), Some(e)) = (te.net_id, w.entities.list.get_mut(k)) {
                e.network_id = n;
            }
            k += 1;
        }
    }
    let mut actions = c.actions();
    let mut cache: HashMap<String, String> = HashMap::new();
    let mut expect: HashMap<[i32; 3], String> = HashMap::new();
    for (t, (changes, ents)) in trace.frames.iter().enumerate() {
        // the exporter's order: the inputs due at t, then (t > 0) one tick, then the frame
        while actions.first().is_some_and(|(at, _)| *at as usize == t) {
            let (_, a) = actions.remove(0);
            match a {
                Action::Place { pos, state, nbt } => {
                    let b = Block::parse(&state).map_err(|e| anyhow::anyhow!(e))?;
                    w.set_block(bp(pos), b, flags::ALL);
                    if let Some(n) = nbt {
                        redsim::redstone::container::apply_block_entities(&mut w, [(bp(pos), n.as_str())]).map_err(|e| anyhow::anyhow!("nbt: {e}"))?;
                    }
                }
                Action::Use { pos } => {
                    if !w.use_block(bp(pos)) {
                        return Ok(Outcome::Unmodeled(format!("a right-click on {} is not modeled", w.get(bp(pos)))));
                    }
                }
                Action::Clear { pos } => {
                    w.set_block(bp(pos), Block::Air, flags::ALL);
                }
                Action::ShapeSet { pos, state } => {
                    let b = Block::parse(&state).map_err(|e| anyhow::anyhow!(e))?;
                    w.set_block(bp(pos), b, flags::CLIENTS);
                }
                Action::Update { pos } => {
                    let src = w.get(bp(pos)).kind();
                    w.update_neighbors_at(bp(pos), src);
                }
            }
        }
        if t > 0 {
            w.tick();
        }
        for (p, s) in changes {
            expect.insert(*p, canon(&mut cache, s));
        }
        let mut diffs = Vec::new();
        let mut n_diff = 0usize;
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                for x in from[0]..=to[0] {
                    let p = [x, y, z];
                    let game = expect.get(&p).cloned().unwrap_or_else(|| Block::Air.to_string());
                    let ours = w.get(bp(p)).to_string();
                    if game != ours {
                        n_diff += 1;
                        if diffs.len() < max_diffs {
                            diffs.push(format!("{x},{y},{z}: game {game}, redsim {ours}"));
                        }
                    }
                }
            }
        }
        let mut ed = Vec::new();
        entity_diffs(ents, &entities_of(&w), &mut ed);
        n_diff += ed.len();
        for d in ed {
            if diffs.len() < max_diffs {
                diffs.push(d);
            }
        }
        if n_diff > 0 {
            let shown = diffs.len();
            if let Some(e) = w.errors.first() {
                diffs.push(format!("(redsim's model note this tick: {e})"));
            }
            return Ok(Outcome::Diverges { tick: t, diffs, more: n_diff - shown });
        }
        if let Some(e) = w.errors.first() {
            return Ok(Outcome::Unmodeled(format!("tick {t}: {e}")));
        }
    }
    Ok(Outcome::Same { ticks: trace.frames.len() })
}

/// Compare every trace; one line each; Err if any is not identical.
pub fn run(paths: &[std::path::PathBuf], max_diffs: usize) -> Result<()> {
    let (mut same, mut total) = (0, 0);
    for p in paths {
        total += 1;
        let t = load_trace(p)?;
        match compare(&t, max_diffs)? {
            Outcome::Same { ticks } => {
                same += 1;
                println!("OK {}: {ticks} ticks identical", t.name);
            }
            Outcome::Diverges { tick, diffs, more } => {
                println!("DIVERGES {} at tick {tick} (warmup {}, period {}):", t.name, t.warmup, t.period);
                for d in diffs {
                    println!("  {d}");
                }
                if more > 0 {
                    println!("  ... and {more} more");
                }
            }
            Outcome::Unmodeled(why) => println!("UNMODELED {}: {why}", t.name),
        }
    }
    println!("{same} of {total} traces tick-identical");
    if same < total {
        bail!("{} traces differ or are not modeled", total - same);
    }
    Ok(())
}
