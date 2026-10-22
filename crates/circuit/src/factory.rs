//! Study 23, fuel factories (designs/circuits/tracks/factory/NOTES.md, docs/circuits.md
//! "kelp-factory N"): kind `kelp-factory N`, a self-fueling dried kelp block factory of N kelp
//! plants: kelp harvested, smelted into dried kelp in furnaces fuelled by the factory's own
//! dried kelp blocks, crafted into blocks (a crafter), part fed back as fuel and the surplus into
//! an output chest (coordinator 2026-10-21: a composite real-use build from the farm, smelter and
//! crafter studies).
//!
//! Judged end to end over a long run at vanilla speed (random tick speed 3: the fuel economy
//! depends on how fast kelp arrives, since a lit furnace burns on idle): the judge plants kelp at
//! every `port crop:` (the design's water source there), puts ONE dried kelp block in the chest on
//! `port fuel:` (the start), empties the chest on `port output:` every [`SEG`] gt and counts its
//! blocks. The run must: take the starting block, deliver only dried kelp blocks, deliver more
//! than it borrowed (net positive), keep delivering to the end (it never starves itself of fuel:
//! a block in the last third of the run), and leave no item on the floor. Metrics: net blocks a
//! (vanilla) hour, blocks/hour per block placed (vjeux's efficiency style), first block, kelp
//! burnt per fuel block.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, Stack};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

pub const BLOCK: &str = "minecraft:dried_kelp_block";
/// The planted kelp (the farm judge's).
pub const KELP_PLANTED: &str = "minecraft:kelp[age=0]";
/// Vanilla's random tick speed (the run's).
pub const RTS: u32 = 3;
/// The judged run: 6 Minecraft hours of 72000 gt (real hours at 20 gt/s).
pub const RUN: u32 = 6 * 72000;
/// The output chest is counted and emptied every SEG gt.
pub const SEG: u32 = 1000;
pub const HOUR: f64 = 72000.0;

/// `factory-run --trace`: every container of the design and the furnace, comparator and dropper
/// states every [`SEG`] gt of a redsim run (stderr).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One run.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// What reached the output chest, per kind.
    pub reached: BTreeMap<String, u64>,
    /// (gt, blocks so far) at every change.
    pub flow: Vec<(u32, u64)>,
    pub ticks: u32,
    /// Left in the fuel chest at the end.
    pub fuel_left: u64,
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    pub loose: Vec<String>,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn blocks(&self) -> u64 {
        self.reached.get(BLOCK).copied().unwrap_or(0)
    }
}

struct Ports {
    crops: Vec<Pos>,
    fuel: Pos,
    output: Pos,
}

/// The design's open hopper tops a stray can reach (the intrusion run's targets); a hopper under
/// one of the judge's chests (the start chest on `port fuel:`) is closed by it.
pub fn intakes(c: &Circuit) -> Vec<Pos> {
    let Ok(p) = ports(c) else { return Vec::new() };
    let mut blocked = crate::intrusion::device_cells(c);
    blocked.extend([p.fuel, p.output]);
    blocked.extend(p.crops.iter().copied());
    clock::intrusion::intakes(&c.clock.design, &blocked).into_iter().filter(|q| *q != p.fuel && *q != p.output).collect()
}

fn ports(c: &Circuit) -> std::result::Result<Ports, String> {
    let crops: Vec<Pos> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    if crops.is_empty() {
        return Err("kelp-factory: no `port crop:` (a kelp plant's cell)".into());
    }
    let fuel = c.stores.iter().find(|s| s.role == "fuel").map(|s| s.pos).ok_or("kelp-factory: no `port fuel:` (the chest the starting block goes in)")?;
    let output = c.outputs.first().map(|o| o.pos).ok_or("kelp-factory: no `port output:`")?;
    Ok(Ports { crops, fuel, output })
}

/// Game: one run of `ticks` gt on a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, ticks: u32, margin: i32, strays: bool) -> Result<std::result::Result<RunLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, ticks, from, to, strays);
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, ticks: u32, from: Pos, to: Pos, strays: bool) -> Result<std::result::Result<RunLog, String>> {
    let p = match ports(c) {
        Ok(p) => p,
        Err(why) => return Ok(Err(why)),
    };
    clock::verify::place(w, sb, &c.clock)?;
    // a `kick: last` block (an observer clock's second observer) placed last, like a player
    for (_, a) in c.clock.actions() {
        if let clock::Action::Place { pos, state, nbt } = a {
            w.set_blocks(sb, &[(pos, state)], PlaceMode::Update)?;
            if let Some(n) = nbt {
                w.merge_block_entity(sb, pos, &n)?;
            }
        }
    }
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(q, _)| placed.get(**q).is_none_or(mctas::blockstate::is_air)).map(|(q, s)| (*q, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|q| (*q, c.clock.design.blocks.get(q).cloned().unwrap_or_default()));
    if let Some((q, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let chests = vec![(p.fuel, CHEST.to_string()), (p.output, CHEST.to_string())];
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    w.exec(sb, "gamerule random_tick_speed 0")?;
    let planted: Vec<(Pos, String)> = p.crops.iter().map(|q| (*q, KELP_PLANTED.to_string())).collect();
    w.set_blocks(sb, &planted, PlaceMode::Update)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    let now = w.get_region(sb, from, to)?;
    if let Some(q) = p.crops.iter().find(|q| !now.get(**q).is_some_and(|s| s.starts_with("minecraft:kelp"))) {
        return Ok(Err(format!("the kelp planted at {q:?} does not survive (kelp needs a water source at the crop cell, water above it and a sturdy block or kelp below it)")));
    }
    let mut log = RunLog { ticks, dims: shulker::design_dims(c), initial: crate::sorter::held(w, sb, from, to, &[p.fuel, p.output])?, ..Default::default() };
    w.merge_block_entity(sb, p.fuel, &shulker::items_snbt(&[Stack::new(BLOCK, 1)]))?;
    w.exec(sb, &format!("gamerule random_tick_speed {RTS}"))?;
    let tops = if strays { intakes(c) } else { Vec::new() };
    let mut t = 0u32;
    while t < ticks {
        let n = SEG.min(ticks - t);
        w.step(sb, n)?;
        t += n;
        for (k, (id, count)) in crate::sorter::STRAYS.iter().enumerate() {
            let at = ticks / 4 * (k as u32 + 1);
            if at > t - n && at <= t && !tops.is_empty() {
                let specs: Vec<mctas::design::EntitySpec> = tops
                    .iter()
                    .map(|q| mctas::design::EntitySpec { kind: "minecraft:item".into(), pos: [q[0] as f64 + 0.5, q[1] as f64 + 0.5, q[2] as f64 + 0.5], nbt: Some(format!("{{Item:{{id:\"{id}\",count:{count}}},PickupDelay:0s,Motion:[0.0d,0.0d,0.0d]}}")) })
                    .collect();
                w.summon(sb, &specs)?;
                log.notes.push(format!("t={t}: the judge drops {} x{count} on each of {} open hopper top(s) {:?}", clock::short(id), tops.len(), tops));
            }
        }
        let have = shulker::read_chest(w, sb, p.output)?;
        if !have.is_empty() {
            for s in &have {
                *log.reached.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            w.merge_block_entity(sb, p.output, &shulker::items_snbt(&[]))?;
            log.flow.push((t, log.blocks()));
        }
    }
    log.fuel_left = shulker::read_chest(w, sb, p.fuel)?.iter().map(|s| s.count as u64).sum();
    log.final_held = crate::sorter::held(w, sb, from, to, &[p.fuel, p.output])?;
    for e in w.entities(sb)? {
        if (e.design && e.kind != "minecraft:item") || crate::sorter::on_top(&tops, e.pos) {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    w.exec(sb, "gamerule random_tick_speed 0")?;
    Ok(Ok(log))
}

/// redsim: the same run, random ticks seeded as the farm judge's twin.
pub fn sim_run(c: &Circuit, ticks: u32, margin: i32, strays: bool) -> std::result::Result<RunLog, String> {
    use redsim::world::flags;
    use redsim::{Block, BlockPos, World};
    let p = ports(c)?;
    let bp = |q: Pos| BlockPos::new(q[0], q[1], q[2]);
    let (from, to) = crate::run::watch_box(c, margin);
    let seed = shulker::build_seed(from, to, 0);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let mut w = World::new(bp(from), [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1]);
    w.mirror_level_random(seed, chunks);
    w.drop_seed = Some(seed);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(0, seed));
    let parse = |s: &str| Block::parse(s).map_err(|e| format!("not modeled by redsim: {e}"));
    let last = c.clock.withheld();
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for (q, s) in c.clock.design.block_list() {
        if Some(q) != last {
            blocks.push((bp(q), parse(&s)?));
        }
    }
    w.place_raw(&blocks);
    let be: Vec<(BlockPos, &str)> = c.clock.design.block_entities.iter().filter(|(q, _)| Some(**q) != last).map(|(q, s)| (bp(*q), s.as_str())).collect();
    redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
    w.place_updates(&blocks);
    // a `kick: last` block (an observer clock's second observer) placed last, like a player
    if let Some(q) = last {
        let s = parse(&c.clock.design.blocks[&q])?;
        w.set_block(bp(q), s, flags::ALL);
        blocks.push((bp(q), s));
    }
    let popped = c.clock.design.blocks.keys().find(|q| w.get(bp(**q)).is_air());
    let floating = blocks.iter().find(|(q, s)| !w.get(*q).is_air() && !redsim::behavior::can_survive(&w, s, *q)).map(|(q, _)| [q.x, q.y, q.z]);
    if let Some(q) = popped.copied().or(floating) {
        let st = c.clock.design.blocks.get(&q).cloned().unwrap_or_default();
        return Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st)));
    }
    let chest = parse(CHEST)?;
    w.place_raw(&[(bp(p.fuel), chest), (bp(p.output), chest)]);
    let kelp = parse(KELP_PLANTED)?;
    for q in &p.crops {
        w.set_block(bp(*q), kelp, flags::ALL);
    }
    for _ in 0..c.clock.free_from() + SETTLE {
        w.tick();
    }
    if let Some(q) = p.crops.iter().find(|q| !matches!(w.get(bp(**q)), Block::Kelp { .. } | Block::KelpPlant)) {
        return Err(format!("the kelp planted at {q:?} does not survive (kelp needs a water source at the crop cell, water above it and a sturdy block or kelp below it)"));
    }
    let judge = [p.fuel, p.output];
    let mut log = RunLog { ticks, dims: shulker::design_dims(c), initial: crate::sorter::sim_held(&w, &judge), ..Default::default() };
    let item = |id: &str| redsim::redstone::container::item_id(id.trim_start_matches("minecraft:")).ok_or_else(|| format!("redsim has no item {id}"));
    redsim::redstone::shulker::set_chest(&mut w, bp(p.fuel), &[redsim::redstone::container::Stack::new(item(BLOCK)?, 1)]);
    if let Some(rt) = w.random_ticks.as_mut() {
        rt.speed = RTS;
    }
    let tops = if strays { intakes(c) } else { Vec::new() };
    let mut t = 0u32;
    while t < ticks {
        let n = SEG.min(ticks - t);
        for _ in 0..n {
            w.tick();
        }
        t += n;
        for (k, (id, count)) in crate::sorter::STRAYS.iter().enumerate() {
            let at = ticks / 4 * (k as u32 + 1);
            if at > t - n && at <= t && !tops.is_empty() {
                let mut sb = crate::sorter::SimBench { w: &mut w, from, to, frames: None, coal: None, watch: None };
                for q in &tops {
                    use crate::sorter::Bench;
                    sb.drop_item([q[0] as f64 + 0.5, q[1] as f64 + 0.5, q[2] as f64 + 0.5], &Stack::new(id, *count)).map_err(|e| format!("{e:#}"))?;
                }
                log.notes.push(format!("t={t}: the judge drops {} x{count} on each of {} open hopper top(s) {:?}", clock::short(id), tops.len(), tops));
            }
        }
        if let Some(e) = w.errors.first() {
            return Err(format!("redsim (t={t}): {e}"));
        }
        if TRACE.load(std::sync::atomic::Ordering::Relaxed) {
            let mut line = format!("t={t}");
            for (q, st) in &c.clock.design.blocks {
                let name = clock::short(st);
                let name = name.split('[').next().unwrap_or("");
                if let Some(x) = w.containers.map.get(&bp(*q)) {
                    let items: Vec<String> = x.slots.iter().flatten().map(|i| format!("{}x{}", redsim::redstone::container::item_name(i.item), i.count)).collect();
                    if !items.is_empty() {
                        line.push_str(&format!(" {name}{q:?}[{}]", items.join(" ")));
                    }
                }
            }
            for q in c.clock.design.blocks.keys() {
                let s = w.get(bp(*q));
                let txt = format!("{s}");
                if txt.contains("furnace") || txt.contains("comparator") || txt.contains("dropper") {
                    line.push_str(&format!(" {q:?}={}", txt.trim_start_matches("minecraft:")));
                }
            }
            eprintln!("{line}");
        }
        let have = redsim::redstone::shulker::chest_stacks(&w, bp(p.output));
        if !have.is_empty() {
            for s in &have {
                *log.reached.entry(format!("minecraft:{}", redsim::redstone::container::item_name(s.item))).or_insert(0) += s.count as u64;
            }
            redsim::redstone::shulker::set_chest(&mut w, bp(p.output), &[]);
            log.flow.push((t, log.blocks()));
        }
    }
    log.fuel_left = redsim::redstone::shulker::chest_stacks(&w, bp(p.fuel)).iter().map(|s| s.count as u64).sum();
    log.final_held = crate::sorter::sim_held(&w, &judge);
    log.loose = w.entities.list.iter().filter(|e| !e.removed && !crate::sorter::on_top(&tops, [e.pos.x, e.pos.y, e.pos.z])).map(|e| format!("{:?}", e.kind)).collect();
    Ok(log)
}

/// Judge a run.
pub fn judge(c: &Circuit, log: &RunLog) -> Outcome {
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let hours = log.ticks as f64 / HOUR;
    let mut table = vec![format!("output chest over {hours:.1} h: {}", l(&log.reached))];
    table.push(format!("blocks by the hour: {}", (1..=hours.ceil() as u32).map(|h| log.flow.iter().filter(|(t, _)| *t <= h * HOUR as u32).map(|x| x.1).max().unwrap_or(0).to_string()).collect::<Vec<_>>().join(", ")));
    table.push(format!("in the machine at the end: {}", l(&log.final_held)));
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    if let Some((k, n)) = log.initial.iter().find(|(k, _)| k.contains("kelp")) {
        return fail(format!("the design holds {n} {} of its own (it starts from the judge's one block)", clock::short(k)), table);
    }
    if log.fuel_left > 0 {
        return fail("the starting block never left the fuel chest".to_string(), table);
    }
    if let Some((k, n)) = log.reached.iter().find(|(k, _)| k.as_str() != BLOCK) {
        return fail(format!("{} x{n} reached the output chest (dried kelp blocks only)", clock::short(k)), table);
    }
    let blocks = log.blocks();
    if blocks <= 1 {
        return fail(format!("{blocks} block(s) out for the 1 borrowed: no net output"), table);
    }
    let last = log.flow.last().map(|x| x.0).unwrap_or(0);
    if last < log.ticks * 2 / 3 {
        return fail(format!("the last block came out at {last} gt of {}: it stopped (starved of fuel or jammed)", log.ticks), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    let net = blocks as f64 - 1.0;
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let per_h = net / hours;
    let metrics = vec![
        ("blocks/hour per block placed".to_string(), format!("{:.4}", per_h / c.blocks().max(1) as f64)),
        ("net blocks/hour".to_string(), format!("{per_h:.2}")),
        ("blocks".to_string(), format!("{blocks} out for 1 borrowed in {hours:.0} h")),
        ("first block".to_string(), format!("by {first} gt")),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The judge (`mctas circuit verify`): the run at N positions x 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, RUN, o.margin, false)?;
        Ok(r.map(|log| BuildOk { outcome: judge(c, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = RUN;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    // intrusion: the sorter's strays on every open hopper top a quarter, half and three quarters
    // into the run, at 4 rotations (none to drop on: noted)
    let tops = intakes(c);
    let line = if tops.is_empty() {
        "intrusion: no open hopper top (a stray cannot get in)".to_string()
    } else {
        let results = pool.map(vec![0u8, 1, 2, 3], |w, _, t| {
            let rc = c.rotated(w, t)?;
            game_run(w, &rc, RUN, o.margin, true)
        });
        for (t, r) in results.into_iter().enumerate() {
            let why = match r {
                Ok(Ok(log)) => {
                    let out = judge(c, &log);
                    if out.pass { None } else { Some(out.why) }
                }
                Ok(Err(why)) => Some(why),
                Err(e) => Some(format!("{e:#}")),
            };
            if let Some(why) = why {
                rep.pass = false;
                rep.verdict = format!("stray items dropped on every open hopper top: rotation {t}: {why}");
                return Ok(rep);
            }
        }
        format!("intrusion: strays on {} open hopper top(s): passes (4 rotations)", tops.len())
    };
    let bill = shulker::bill(c);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("bill".to_string(), bill));
        k.outcome.table.push(line);
    }
    Ok(rep)
}

/// A design aid (`mctas circuit factory-run`): one game run at rotation 0.
pub fn run_one(pool: &Pool, c: &Circuit, ticks: u32, margin: i32, strays: bool) -> Result<Outcome> {
    let r = pool.map(vec![0u8], |w, _, _| game_run(w, c, ticks, margin, strays));
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        Ok(log) => Ok(judge(c, &log)),
        Err(why) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// [`run_one`] on redsim.
pub fn sim_one(c: &Circuit, ticks: u32, margin: i32, strays: bool) -> Result<Outcome> {
    if !matches!(c.kind, Some(Kind::KelpFactory(_))) {
        bail!("{}: not a kelp factory", c.name());
    }
    Ok(match sim_run(c, ticks, margin, strays) {
        Ok(log) => judge(c, &log),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}
