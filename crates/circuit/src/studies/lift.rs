//! Item elevators (clk-misc, 2026-10-21; coordinator: "a new real-use kind, the ITEM ELEVATOR
//! (move items up N blocks: dropper chains, bubble columns, etc.): judge throughput at hopper
//! speed (items/s), no item lost or stuck, mixed stacks/non-stackables, intrusion-safe, robust;
//! heights 5/10/20; minimum blocks per height"): kind `item-elevator N`.
//!
//! The design declares `port items:` (the input chest, an air cell: the design takes the items
//! out of it, usually with a hopper under it) and `port output:` (the output chest, an air cell,
//! exactly N blocks above the input chest: `output.y - items.y = N`). The judge places both
//! chests (not counted), lets the design settle, fills the input chest with a mixed load (64-,
//! 16-stackables and non-stackables, [`LOAD`]), empties the output chest every gt, and waits
//! until every item arrived, then for rest. Every run must:
//! * deliver the whole load into the output chest, kind by kind (nothing lost, nothing left in
//!   the input chest, nothing kept in the design's containers, no item loose anywhere);
//! * keep up with a hopper: the flow into the output chest between 10% and 90% of the load at
//!   least 98% of a hopper's pace (2.5 items/s: one item per 8 gt);
//! * rest, or run a clock of its own (metric `at rest` no).
//!
//! The runs ([`Run`]): `base` at 4 positions x 4 rotations (the metrics; the builds agree when
//! every one passes, rests alike and flows within 2% of the base build), and `intrusion` (4
//! rotations; the real-use audit's ITEM INTRUSION): 1 cobblestone, 64 iron ingots and 1 iron
//! shovel dropped on every open hopper top at a quarter, half and three quarters of the run; the
//! load must still arrive whole, and every stray either reaches the output chest or still lies
//! where it was dropped.
//!
//! Metrics: items/s, first item (the gt from the fill to the first item in the output chest:
//! the trip up), at rest, blocks; the archive's axis is the height N. Judged in the game
//! (`circuit verify`) and by its redsim twin ([`sim_verify`]: `circuit sim`, the searches).

use crate::check::Outcome;
use crate::registry::{Agree, KindDef, RunAt, Site, StudyDef, Twin};
use crate::shulker::{self, CHEST, REST, Stack};
use crate::sorter::{Bench, GameBench, STRAYS, SimBench, bp, held, on_top, sim_held, top_box};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// The study's registration (crate::registry): `circuit: item-elevator N`.
pub static STUDY: StudyDef = StudyDef {
    id: "elevator-items",
    title: "Item elevators",
    site_title: "Item elevators",
    site_intro: "An item elevator lifts items straight up: a hopper takes them from a chest at the bottom, a column of droppers fires them up block by block, and the top one pushes them into a chest at the top. Our test, in the real game, fills the input chest with 193 mixed items, empties the output chest every tick, and every item must arrive at the top, none lost or stuck; it measures how many items a second it lifts and how soon the first one arrives. For each height we keep the design with the fewest blocks placed.",
    site_uses: "Send items up: from a farm's collection chest at the bottom of a base up to the storage room, or from a mine up to the surface, with no water streams or ladders of hoppers.",
    site_blurb: "Lift items N blocks up from a chest into a chest, as fast as a hopper feeds them.",
    kinds: &[KindDef {
        word: "item-elevator",
        parse: parse_kind,
        inputs: &[],
        check: check_kind,
        verify: verify_in_pool,
        twin: Twin::Exact(sim_verify),
        agree: Agree::Rate { key: "items/s", tol: 0.02 },
        ticks: |_| Scenario::new(Run::Base).cap,
        axis: |args, _| format!("{} blocks up", args.first().map(String::as_str).unwrap_or("?")),
        runs: &["base", "intrusion"],
        run: run_kind,
        site: Site {
            title: |_| "Item elevator: lifts the items of a chest up into another (dropper elevator)".into(),
            summary: |_| "lifts every item of its input chest straight up into its output chest, a set number of blocks higher, as fast as a hopper feeds them".into(),
            metric: |k| match k {
                "first item" => Some("game ticks from the start until the first item reaches the output chest"),
                _ => None,
            },
        },
    }],
};

/// `item-elevator N`: the height, at least 1.
fn parse_kind(w: &[&str]) -> Result<Vec<String>> {
    match w.first().and_then(|s| s.parse::<u32>().ok()) {
        Some(n) if n >= 1 => Ok(vec![n.to_string()]),
        _ => bail!("item-elevator: give the height, the output chest's y minus the input chest's (`circuit: item-elevator 5`)"),
    }
}

/// The ports: the input chest, one output chest N blocks above it, both on air cells.
fn check_kind(c: &Circuit, args: &[String]) -> Result<()> {
    if !c.stores.iter().any(|s| s.role == "items") || c.outputs.len() != 1 {
        bail!("item-elevator: needs `port items: x y z` (the input chest: an air cell) and one `port output: x y z` (the output chest, N blocks above it)");
    }
    for o in &c.outputs {
        if c.clock.design.blocks.contains_key(&o.pos) {
            bail!("port output.{}: a judge chest goes on an air cell, the design has a block at {:?}", o.name, o.pos);
        }
    }
    check_ports(c, args.first().and_then(|n| n.parse().ok()).unwrap_or(0))
}

/// `mctas circuit run` (and `lift-run`): one run, the game's or the twin's.
fn run_kind(pool: Option<&Pool>, c: &Circuit, at: &RunAt) -> Result<Outcome> {
    let r = match at.run {
        "base" => Run::Base,
        "intrusion" => Run::Intrusion,
        other => bail!("item-elevator: no run {other:?} (base, intrusion)"),
    };
    if pool.is_none() && (at.turns != 0 || at.offset != [0, 0, 0] || at.watch.is_some()) {
        return run_one_sim_at(c, r, at.margin, at.turns, at.offset, at.watch);
    }
    if at.turns != 0 || at.offset != [0, 0, 0] {
        bail!("item-elevator: --turns/--offset replay a build on the twin (--sim)");
    }
    run_one(pool, c, r, at.margin)
}

/// A hopper's pace in items/s (one item per 8 gt).
pub const HOPPER_RATE: f64 = 2.5;
/// The share of a hopper's pace a run must keep up.
pub const KEEP_UP: f64 = 0.98;
/// The load, in the input chest's slot order: 64-stackables, 16-stackables (snowballs, ender
/// pearls) and non-stackables (iron shovels, a diamond sword), full and partial stacks.
pub const LOAD: [(&str, u32); 11] = [
    ("minecraft:cobblestone", 64),
    ("minecraft:snowball", 16),
    ("minecraft:iron_shovel", 1),
    ("minecraft:sand", 37),
    ("minecraft:ender_pearl", 5),
    ("minecraft:iron_ingot", 20),
    ("minecraft:diamond_sword", 1),
    ("minecraft:kelp", 23),
    ("minecraft:snowball", 9),
    ("minecraft:paper", 16),
    ("minecraft:iron_shovel", 1),
];

/// One run of the judge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Base,
    Intrusion,
}

impl Run {
    pub fn title(&self) -> &'static str {
        match self {
            Run::Base => "base",
            Run::Intrusion => "intrusion",
        }
    }
}

/// One run's inputs.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub run: Run,
    pub load: Vec<Stack>,
    /// (gt, stack) dropped on every open hopper top at that gt.
    pub strays: Vec<(u32, Stack)>,
    pub cap: u32,
}

impl Scenario {
    pub fn new(run: Run) -> Scenario {
        let load: Vec<Stack> = LOAD.iter().map(|(id, n)| Stack::new(id, *n)).collect();
        let total: u32 = load.iter().map(|s| s.count).sum();
        // the load at a hopper's pace, half again, and the trip
        let span = total * 8;
        let strays = if run == Run::Intrusion { STRAYS.iter().enumerate().map(|(k, (id, c))| (span * (k as u32 + 1) / 4, Stack::new(id, *c))).collect() } else { Vec::new() };
        Scenario { run, load, strays, cap: span * 3 / 2 + 2000 }
    }

    pub fn total(&self) -> u64 {
        self.load.iter().map(|s| s.count as u64).sum()
    }

    pub fn supplied(&self, id: &str) -> u64 {
        self.load.iter().filter(|s| s.id == id).map(|s| s.count as u64).sum()
    }
}

/// One run's record.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// Items that reached the output chest, by kind.
    pub reached: BTreeMap<String, u64>,
    /// (gt, delivered so far) at every gt the count rose.
    pub flow: Vec<(u32, u64)>,
    pub dropped: BTreeMap<String, u64>,
    /// Strays still lying on an open hopper top at the end.
    pub lying: BTreeMap<String, u64>,
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    pub input_left: BTreeMap<String, u64>,
    pub loose: Vec<String>,
    pub rests: bool,
    pub timed_out: bool,
    pub end: u32,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn delivered(&self) -> u64 {
        self.reached.values().sum()
    }

    /// The gt of the first item in the output chest.
    pub fn first(&self) -> Option<u32> {
        self.flow.first().map(|f| f.0)
    }

    /// The flow into the output chest between 10% and 90% of `total`, in items/s.
    pub fn rate(&self, total: u64) -> f64 {
        let at = |n: u64| self.flow.iter().find(|(_, d)| *d >= n).copied();
        let (lo, hi) = (total.div_ceil(10).max(1), (total * 9).div_ceil(10));
        match (at(lo), at(hi)) {
            (Some((t0, d0)), Some((t1, d1))) if t1 > t0 => (d1 - d0) as f64 * 20.0 / (t1 - t0) as f64,
            _ => 0.0,
        }
    }
}

/// Where the judge's things go.
struct Ports {
    items: Pos,
    output: Pos,
    judge: Vec<Pos>,
    /// The open hopper tops (strays), the judge's chests closing the ones they stand on.
    strays: Vec<Pos>,
}

fn ports(c: &Circuit) -> std::result::Result<Ports, String> {
    let items = c.stores.iter().find(|s| s.role == "items").map(|s| s.pos).ok_or_else(|| "item-elevator: no `port items:`".to_string())?;
    let output = c.outputs.first().map(|o| o.pos).ok_or_else(|| "item-elevator: no `port output:`".to_string())?;
    let judge = vec![items, output];
    let strays = crate::intrusion::intakes(c).into_iter().filter(|p| !judge.contains(p)).collect();
    Ok(Ports { items, output, judge, strays })
}

/// The design's height claim checked: the output chest N blocks above the input chest.
pub fn check_ports(c: &Circuit, n: u32) -> Result<()> {
    let p = ports(c).map_err(|e| anyhow!(e))?;
    let rise = p.output[1] - p.items[1];
    if rise != n as i32 {
        bail!("item-elevator {n}: the output chest stands {rise} blocks above the input chest, not {n}");
    }
    Ok(())
}

/// Run a scenario on a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, sc: &Scenario, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, sc, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    let p = match ports(c) {
        Ok(p) => p,
        Err(why) => return Ok(Err(why)),
    };
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    // (a start block placed last is not there yet)
    let last = c.clock.withheld();
    let popped = c.clock.design.blocks.iter().filter(|(q, _)| Some(**q) != last).find(|(q, _)| placed.get(**q).is_none_or(mctas::blockstate::is_air)).map(|(q, s)| (*q, s.clone()));
    let floating = w.unsupported(sb, from, to)?.into_iter().find(|q| Some(*q) != last).map(|q| (q, c.clock.design.blocks.get(&q).cloned().unwrap_or_default()));
    if let Some((q, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let chests: Vec<(Pos, String)> = p.judge.iter().map(|q| (*q, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    // the design's start kick (`port start:`) at its ticks, then the settle
    let actions = c.clock.actions();
    for t in 0..c.clock.free_from() {
        for (_, a) in actions.iter().filter(|(at, _)| *at == t) {
            clock::verify::apply(w, sb, a)?;
        }
        w.step(sb, 1)?;
    }
    for (_, a) in actions.iter().filter(|(at, _)| *at == c.clock.free_from()) {
        clock::verify::apply(w, sb, a)?;
    }
    let mut b = GameBench { w, sb, from, to, base: None, frames: None };
    b.step(SETTLE)?;
    let mut log = RunLog { dims: shulker::design_dims(c), initial: held(b.w, sb, from, to, &p.judge)?, ..Default::default() };
    drive(&mut b, sc, &p, &mut log)?;
    finish(w, sb, from, to, &p, &mut log)?;
    Ok(Ok(log))
}

/// Top the input chest up from the load (27 slots).
fn topup<B: Bench>(b: &mut B, at: Pos, list: &[Stack], cursor: &mut usize) -> Result<()> {
    if *cursor >= list.len() {
        return Ok(());
    }
    let mut have = b.read(at)?;
    let before = have.len();
    while have.len() < 27 && *cursor < list.len() {
        have.push(list[*cursor].clone());
        *cursor += 1;
    }
    if have.len() != before {
        b.write(at, &have)?;
    }
    Ok(())
}

/// The run's loop: the input chest filled, then every gt the output chest counted and emptied,
/// strays when due. Stops when every item (and every stray) is accounted for, or at the cap.
fn drive<B: Bench>(b: &mut B, sc: &Scenario, p: &Ports, log: &mut RunLog) -> Result<()> {
    let mut cursor = 0usize;
    topup(b, p.items, &sc.load, &mut cursor)?;
    let (mut t, mut strays, mut quiet) = (0u32, 0usize, 0u32);
    loop {
        b.step(1)?;
        t += 1;
        while strays < sc.strays.len() && t >= sc.strays[strays].0 {
            let s = &sc.strays[strays].1;
            for q in &p.strays {
                b.drop_item([q[0] as f64 + 0.5, q[1] as f64 + 0.5, q[2] as f64 + 0.5], s)?;
                *log.dropped.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            if !p.strays.is_empty() {
                log.notes.push(format!("t={t}: the judge drops {} x{} on each of {} open hopper top(s) {:?}", clock::short(&s.id), s.count, p.strays.len(), p.strays));
            }
            strays += 1;
        }
        let have = b.read(p.output)?;
        if !have.is_empty() {
            for s in &have {
                *log.reached.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(p.output, &[])?;
            log.flow.push((t, log.delivered()));
            quiet = 0;
        } else {
            quiet += 1;
        }
        topup(b, p.items, &sc.load, &mut cursor)?;
        let dropped: u64 = log.dropped.values().sum();
        let all = cursor >= sc.load.len() && strays >= sc.strays.len();
        let now = log.delivered();
        // strays still lying on a hopper top count as accounted for once nothing moved for 400 gt
        let lying: u64 = if dropped > 0 && now < sc.total() + dropped && t % 20 == 0 {
            let mut n = 0;
            for q in &p.strays {
                let (lo, hi) = top_box(*q);
                n += b.items_in(lo, hi)?.iter().map(|x| x.1).sum::<u64>();
            }
            n
        } else {
            0
        };
        let done = all && (now >= sc.total() + dropped || (now + lying >= sc.total() + dropped && quiet >= 400));
        if done || t >= sc.cap {
            log.timed_out = !done;
            break;
        }
    }
    for q in &p.strays {
        let (lo, hi) = top_box(*q);
        for (id, n) in b.items_in(lo, hi)? {
            *log.lying.entry(id).or_insert(0) += n;
        }
    }
    for s in b.read(p.items)? {
        *log.input_left.entry(s.id.clone()).or_insert(0) += s.count as u64;
    }
    log.end = t;
    Ok(())
}

/// After the run: rest, the design's containers, the entities left (strays lying on an open
/// hopper top apart).
fn finish(w: &mut Worker, sb: u32, from: Pos, to: Pos, p: &Ports, log: &mut RunLog) -> Result<()> {
    let mut a = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
    let mut b = a.clone();
    let mut status = w.status(sb)?;
    for _ in 0..5 {
        w.step(sb, REST)?;
        b = (w.get_region(sb, from, to)?, w.block_entities(sb, from, to)?);
        status = w.status(sb)?;
        if a == b && status.block_ticks + status.block_events + status.moving_pistons == 0 {
            break;
        }
        a = b.clone();
    }
    log.rests = a == b && status.block_ticks + status.block_events + status.moving_pistons == 0;
    log.final_held = held(w, sb, from, to, &p.judge)?;
    // where items stay (the table)
    for be in &b.1 {
        if p.judge.contains(&be.pos) {
            continue;
        }
        let st = shulker::stacks_of_be(&be.nbt);
        if !st.is_empty() {
            log.notes.push(format!("at the end, the container at {:?} holds {}", be.pos, st.iter().map(|s| format!("{} x{}", clock::short(&s.id), s.count)).collect::<Vec<_>>().join(", ")));
        }
    }
    for e in w.entities(sb)? {
        if (e.design && e.kind != "minecraft:item") || on_top(&p.strays, e.pos) {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    Ok(())
}

/// The same run on redsim, the game's sandbox mirrored (its seed's level random: dropper slots,
/// drops). Outer Err: what redsim cannot model; inner: the judge's refusal.
pub fn sim_run(c: &Circuit, sc: &Scenario, margin: i32) -> std::result::Result<std::result::Result<RunLog, String>, String> {
    sim_run_watch(c, sc, margin, None)
}

/// [`sim_run`], printing every gt the item entities whose id contains `watch` (`lift-run --watch`).
pub fn sim_run_watch(c: &Circuit, sc: &Scenario, margin: i32, watch: Option<&str>) -> std::result::Result<std::result::Result<RunLog, String>, String> {
    use redsim::{Block, BlockPos, World};
    let (from, to) = crate::run::watch_box(c, margin);
    let p = match ports(c) {
        Ok(p) => p,
        Err(why) => return Ok(Err(why)),
    };
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let seed = shulker::build_seed(from, to, 0);
    let mut w = World::new(BlockPos::new(from[0], from[1], from[2]), [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1]);
    w.mirror_level_random(seed, chunks);
    w.drop_seed = Some(seed);
    let parse = |s: &str| Block::parse(s).map_err(|e| format!("not modeled by redsim: {e}"));
    let last = c.clock.withheld();
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for (q, s) in c.clock.design.block_list() {
        if Some(q) != last {
            blocks.push((bp(q), parse(&s)?));
        }
    }
    blocks.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    let be: Vec<(BlockPos, &str)> = c.clock.design.block_entities.iter().filter(|(q, _)| Some(**q) != last).map(|(q, s)| (bp(*q), s.as_str())).collect();
    match c.clock.placement {
        clock::Placement::Judge => {
            w.place_raw(&blocks);
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
            w.place_updates(&blocks);
        }
        // every block with updates in order, as a player builds it (observer clocks start)
        clock::Placement::Player => {
            for (q, s) in &blocks {
                w.set_block(*q, *s, redsim::world::flags::ALL);
            }
            redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
        }
    }
    for e in &c.clock.design.entities {
        redsim::entity::spawn(&mut w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| format!("not modeled by redsim: {why}"))?;
    }
    let popped = c.clock.design.blocks.keys().filter(|q| Some(**q) != last).find(|q| w.get(bp(**q)).is_air()).copied();
    let floating = blocks.iter().find(|(q, s)| !w.get(*q).is_air() && !redsim::behavior::can_survive(&w, s, *q)).map(|(q, _)| [q.x, q.y, q.z]);
    if let Some(q) = popped.or(floating) {
        let st = c.clock.design.blocks.get(&q).cloned().unwrap_or_default();
        return Ok(Err(format!("{} at {q:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let chest = parse(CHEST)?;
    let chests: Vec<(BlockPos, Block)> = p.judge.iter().map(|q| (bp(*q), chest)).collect();
    w.place_raw(&chests);
    // the design's start kick at its ticks (redsim: placed or used like the game's), then the
    // settle
    let actions = c.clock.actions();
    let apply = |w: &mut World, a: &clock::Action| -> std::result::Result<(), String> {
        match a {
            clock::Action::Place { pos, state, nbt } => {
                w.set_block(bp(*pos), parse(state)?, redsim::world::flags::ALL);
                if let Some(n) = nbt {
                    redsim::redstone::container::apply_block_entities(w, [(bp(*pos), n.as_str())]).map_err(|e| format!("nbt: {e}"))?;
                }
            }
            clock::Action::Clear { pos } => {
                w.set_block(bp(*pos), Block::Air, redsim::world::flags::ALL);
            }
            clock::Action::Use { pos } => {
                if !w.use_block(bp(*pos)) {
                    return Err(format!("a right-click on {} is not modeled", w.get(bp(*pos))));
                }
            }
            other => return Err(format!("the start kick {other:?} is not modeled for item elevators")),
        }
        Ok(())
    };
    for t in 0..c.clock.free_from() {
        for (_, a) in actions.iter().filter(|(at, _)| *at == t) {
            apply(&mut w, a)?;
        }
        w.tick();
    }
    for (_, a) in actions.iter().filter(|(at, _)| *at == c.clock.free_from()) {
        apply(&mut w, a)?;
    }
    let mut b = SimBench { w: &mut w, from, to, frames: None, coal: None, watch: watch.map(str::to_string) };
    b.step(SETTLE).map_err(|e| format!("{e:#}"))?;
    let mut log = RunLog { dims: shulker::design_dims(c), initial: sim_held(b.w, &p.judge), ..Default::default() };
    drive(&mut b, sc, &p, &mut log).map_err(|e| format!("{e:#}"))?;
    let snapshot = |w: &World| -> (Vec<String>, String) {
        let mut cells = Vec::new();
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                for x in from[0]..=to[0] {
                    cells.push(w.get(BlockPos::new(x, y, z)).to_string());
                }
            }
        }
        let mut cs: Vec<(&BlockPos, String)> = w.containers.map.iter().map(|(q, k)| (q, format!("{:?}", k.slots))).collect();
        cs.sort();
        (cells, format!("{cs:?}"))
    };
    let pending = |w: &World| w.ticks.count() + w.events.len() + w.piston.live_count();
    let mut a = snapshot(&w);
    let mut still = false;
    for _ in 0..5 {
        for _ in 0..REST {
            w.tick();
        }
        let s = snapshot(&w);
        if s == a && pending(&w) == 0 {
            still = true;
            break;
        }
        a = s;
    }
    if let Some(e) = w.errors.first() {
        return Err(format!("redsim: {e}"));
    }
    log.rests = still;
    log.final_held = sim_held(&w, &p.judge);
    // where items stay (the table)
    let mut stuck: Vec<(BlockPos, String)> = w
        .containers
        .map
        .iter()
        .filter(|(q, k)| !p.judge.contains(&[q.x, q.y, q.z]) && k.slots.iter().any(|s| s.is_some()))
        .map(|(q, k)| (*q, k.slots.iter().flatten().map(|s| format!("{} x{}", redsim::redstone::container::item_name(s.item), s.count)).collect::<Vec<_>>().join(", ")))
        .collect();
    stuck.sort_by_key(|(q, _)| [q.x, q.y, q.z]);
    for (q, s) in stuck {
        log.notes.push(format!("at the end, the container at [{}, {}, {}] holds {s}", q.x, q.y, q.z));
    }
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        let at = [e.pos.x, e.pos.y, e.pos.z];
        if on_top(&p.strays, at) {
            continue;
        }
        match &e.kind {
            redsim::entity::EntityKind::Item(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.item), s.count, at)),
            _ if e.summon_index.is_some() => {}
            k => log.loose.push(format!("{k:?} at {at:.1?}")),
        }
    }
    Ok(Ok(log))
}

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(c: &Circuit, sc: &Scenario, log: &RunLog) -> Outcome {
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let total = sc.total();
    let rate = log.rate(total);
    let mut table = vec![
        format!("  output: {}", l(&log.reached)),
        format!("  loaded {total} items ({} kinds), delivered {}, first item at {} gt, the last at {} gt; {:.2} items/s between 10% and 90%", LOAD.len(), log.delivered(), log.first().map(|t| t.to_string()).unwrap_or("-".into()), log.flow.last().map(|f| f.0).unwrap_or(0), rate),
    ];
    if !log.dropped.is_empty() {
        table.push(format!("  strays dropped: {}; still lying on a hopper top: {}", l(&log.dropped), if log.lying.is_empty() { "none".to_string() } else { l(&log.lying) }));
    }
    table.extend(log.notes.iter().map(|n| format!("  {n}")));
    let mut why: Vec<String> = Vec::new();
    // the load, kind by kind (strays may come through too)
    let mut kinds: Vec<String> = sc.load.iter().map(|s| s.id.clone()).chain(log.reached.keys().cloned()).collect();
    kinds.sort();
    kinds.dedup();
    let (mut short, mut got_of_load) = (Vec::new(), 0u64);
    for k in &kinds {
        let (got, want, dropped) = (log.reached.get(k).copied().unwrap_or(0), sc.supplied(k), log.dropped.get(k).copied().unwrap_or(0));
        got_of_load += got.min(want);
        if got < want {
            short.push(format!("{} x{}", clock::short(k), want - got));
        } else if got > want + dropped {
            why.push(format!("{} {} reached the output chest, {} were loaded", got, clock::short(k), want + dropped));
        }
    }
    if !short.is_empty() {
        why.insert(0, format!("{got_of_load} of {total} items reached the output chest (missing {})", short.join(", ")));
    }
    // every stray: delivered or still lying where it was dropped
    for (k, d) in &log.dropped {
        let through = log.reached.get(k).copied().unwrap_or(0).saturating_sub(sc.supplied(k));
        let lying = log.lying.get(k).copied().unwrap_or(0);
        if through + lying < *d {
            why.push(format!("{} of {d} stray {} neither reached the output chest nor lie where they were dropped", d - through - lying, clock::short(k)));
        }
    }
    if !log.input_left.is_empty() {
        why.push(format!("the input chest still holds {}", l(&log.input_left)));
    }
    if log.final_held != log.initial {
        let mut extra: Vec<String> = Vec::new();
        for (k, n) in &log.final_held {
            let had = log.initial.get(k).copied().unwrap_or(0);
            if *n > had {
                extra.push(format!("{} x{}", clock::short(k), n - had));
            }
        }
        why.push(format!("the design's containers keep {}", if extra.is_empty() { "other items than at the start".to_string() } else { extra.join(", ") }));
    }
    if !log.loose.is_empty() {
        why.push(format!("left loose: {}", log.loose.iter().take(4).cloned().collect::<Vec<_>>().join(", ")));
    }
    if log.timed_out && why.is_empty() {
        why.push(format!("not done after {} gt", log.end));
    }
    if why.is_empty() && rate < HOPPER_RATE * KEEP_UP {
        why.push(format!("slower than a hopper: {rate:.2} items/s (a hopper moves {HOPPER_RATE})"));
    }
    let metrics = vec![
        ("items/s".to_string(), format!("{rate:.2}")),
        ("first item".to_string(), format!("{} gt", log.first().unwrap_or(0))),
        ("at rest".to_string(), if log.rests { "yes".into() } else { "no".into() }),
    ];
    let _ = c;
    if why.is_empty() { Outcome { pass: true, why: String::new(), metrics, table } } else { Outcome { pass: false, why: why.join("; "), metrics, table } }
}

/// The judge (`mctas circuit verify`): the base run at N positions x 4 rotations, then the
/// intrusion run at 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Study(_) = kind else { bail!("{}: not an item elevator", kind.name()) };
    let sc = Scenario::new(Run::Base);
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&rc, &sc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.map_err(|e| anyhow!("rotation {t} at {off:?}: {e:#}"))? });
    }
    let intrusion = |rep: &VerifyReport| -> Result<std::result::Result<(), String>> {
        if !rep.pass {
            return Ok(Ok(()));
        }
        let sc = Scenario::new(Run::Intrusion);
        let results = pool.map(vec![0u8, 1, 2, 3], |w, _, t| {
            let rc = c.rotated(w, t)?;
            Ok((game_run(w, &rc, &sc, o.margin)?, rc))
        });
        for (t, r) in results.into_iter().enumerate() {
            match r? {
                (Ok(log), rc) => {
                    let out = judge(&rc, &sc, &log);
                    if !out.pass {
                        return Ok(Err(format!("rotation {t}: {}", out.why)));
                    }
                }
                (Err(why), _) => return Ok(Err(format!("rotation {t}: {why}"))),
            }
        }
        Ok(Ok(()))
    };
    let rep = report(c, kind, sc.cap, builds);
    let intr = intrusion(&rep)?;
    Ok(finish_report(c, rep, intr))
}

fn report(c: &Circuit, kind: Kind, cap: u32, builds: Vec<Build>) -> VerifyReport {
    let mut schedule = Schedule::new(0);
    schedule.ticks = cap;
    crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() })
}

/// The intrusion run's verdict and the bill on the base build's card.
fn finish_report(c: &Circuit, mut rep: VerifyReport, intrusion: std::result::Result<(), String>) -> VerifyReport {
    if !rep.pass {
        return rep;
    }
    // an elevator is built facing any way: a rotation none of whose builds passes fails it (the
    // intrusion run, at every rotation, would fail it too, under a misleading name)
    for t in 0..4u8 {
        let mut of_t = rep.builds.iter().filter(|b| b.turns == t).peekable();
        if of_t.peek().is_none() {
            continue;
        }
        let failing: Vec<&Build> = of_t.filter(|b| !b.result.as_ref().is_ok_and(|k| k.outcome.pass)).collect();
        if failing.len() == rep.builds.iter().filter(|b| b.turns == t).count() {
            let why = match &failing[0].result {
                Ok(k) => k.outcome.why.clone(),
                Err(e) => e.to_string(),
            };
            rep.pass = false;
            rep.verdict = format!("rotation {t} fails in every position: {why}");
            return rep;
        }
    }
    if let Err(why) = intrusion {
        rep.pass = false;
        rep.verdict = format!("intrusion: {why}");
        return rep;
    }
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("bill".to_string(), shulker::bill(c)));
        k.outcome.table.push("  intrusion: passes (4 rotations)".to_string());
    }
    rep
}

/// The whole judge on redsim (`circuit sim`, the searches): the same builds (turned without the
/// game) and runs. Err: what redsim cannot model.
pub fn sim_verify(c: &Circuit, o: &Options) -> std::result::Result<VerifyReport, String> {
    let kind = c.kind.clone().ok_or_else(|| "no kind".to_string())?;
    let sc = Scenario::new(Run::Base);
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let mut builds = Vec::new();
    for t in &turns {
        let tc = c.turned_pure(*t).map_err(|e| format!("rotation {t}: {e:#}"))?;
        for off in &offsets {
            let rc = tc.translated(*off).map_err(|e| format!("{e:#}"))?;
            let r = sim_run(&rc, &sc, o.margin).map_err(|why| format!("rotation {t} at {off:?}: {why}"))?;
            builds.push(Build { turns: *t, offset: *off, result: r.map(|log| BuildOk { outcome: judge(&rc, &sc, &log), dims: log.dims, family: family.clone() }) });
        }
    }
    let rep = report(c, kind, sc.cap, builds);
    let mut intr = Ok(());
    if rep.pass {
        let sc = Scenario::new(Run::Intrusion);
        for t in 0..4u8 {
            let rc = c.turned_pure(t).map_err(|e| format!("rotation {t}: {e:#}"))?;
            match sim_run(&rc, &sc, o.margin).map_err(|why| format!("intrusion, rotation {t}: {why}"))? {
                Ok(log) => {
                    let out = judge(&rc, &sc, &log);
                    if !out.pass {
                        intr = Err(format!("rotation {t}: {}", out.why));
                        break;
                    }
                }
                Err(why) => {
                    intr = Err(format!("rotation {t}: {why}"));
                    break;
                }
            }
        }
    }
    Ok(finish_report(c, rep, intr))
}

/// A design aid (`mctas circuit lift-run --sim --turns T --offset X,Y,Z --watch ITEM`): one of the
/// judge's twin builds (rotated, then moved), printing the watched items every gt.
pub fn run_one_sim_at(c: &Circuit, run: Run, margin: i32, turns: u8, offset: Pos, watch: Option<&str>) -> Result<Outcome> {
    let sc = Scenario::new(run);
    let rc = c.turned_pure(turns)?.translated(offset)?;
    Ok(match sim_run_watch(&rc, &sc, margin, watch) {
        Ok(Ok(log)) => judge(&rc, &sc, &log),
        Ok(Err(why)) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
        Err(why) => Outcome { pass: false, why: format!("redsim does not model it: {why}"), metrics: Vec::new(), table: Vec::new() },
    })
}

/// A design aid (`mctas circuit lift-run`): one run at rotation 0, in the game or on redsim.
pub fn run_one(pool: Option<&Pool>, c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let sc = Scenario::new(run);
    let r = match pool {
        Some(pool) => pool.map(vec![0u8], |w, _, _| game_run(w, c, &sc, margin)).into_iter().next().ok_or_else(|| anyhow!("no run"))??,
        None => match sim_run(c, &sc, margin) {
            Ok(r) => r,
            Err(why) => return Ok(Outcome { pass: false, why: format!("redsim does not model it: {why}"), metrics: Vec::new(), table: Vec::new() }),
        },
    };
    Ok(match r {
        Ok(log) => judge(c, &sc, &log),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}

/// The bubble-column family (`mctas circuit gen bubble-column --n N`): an elevator N = 4 + n blocks
/// up in 40 + 5n blocks. The hopper under the input chest feeds a dropper that shoots each item into
/// a water column over soul sand; the bubble column lifts it to the top source, under a ceiling,
/// whose flow toward the flowing water beside it carries it back over the collecting hopper on the
/// dropper's side (the items drift away from the dropper as they rise: on the far side, the hollow
/// under the hopper's rim caught one at 3 up), which pushes it into the output chest. Two observers
/// facing each other in the column's wall are the clock (6 gt): the lower one's back powers the stone
/// above the dropper hard (a hard-powered dropper would lock the hopper beside it); the upper one is
/// placed last (at 3 up its back would lock the collecting hopper: the family starts at 4).
pub fn bubble_column(n: u32) -> String {
    let h = 4 + n;
    let mut layers: Vec<[&str; 3]> = vec![[".....", "..S..", "....."], ["..G..", "HDWG.", "..G.."]];
    for y in 1..=h {
        let west = match y {
            1 => ".TWG.",
            2 => ".UWG.",
            3 => ".NWG.",
            _ => ".GWG.",
        };
        layers.push(["..G..", west, "..G.."]);
    }
    layers.push(["..G..", ".KWG.", "..G.."]);
    layers.push([".GG..", "G.WG.", ".GG.."]);
    layers.push([".....", "..G..", "....."]);
    let mut s = format!(
        "name: bubble-column-n{n}\n\
         # A bubble-column elevator {h} blocks up ({} blocks): the hopper under the input chest feeds a dropper\n\
         # that shoots each item into a water column over soul sand; the bubble column lifts it to the top\n\
         # source, under a ceiling, whose flow toward the flowing water beside it carries it back over the\n\
         # collecting hopper, on the dropper's side (the items drift away from the dropper as they rise, and the\n\
         # hollow under a hopper's rim beside the column would catch one), which pushes it into the output chest.\n\
         # Two observers facing each other in the column's wall are the clock (6 gt): the lower one's back powers\n\
         # the stone above the dropper hard (a hard-powered dropper would lock the hopper beside it); the upper\n\
         # one is placed last (a player starts the clock by placing it). No redstone part grows with the height:\n\
         # the bill stays 10 iron, 2 quartz, 5 redstone (from 4 blocks up: lower, the upper observer's back would\n\
         # lock the collecting hopper).\n\
         circuit: item-elevator {h}\n\
         extend family: bubble-column\n\
         extend axis: 4 + n\n\
         extend blocks: 40 + 5n\n\
         extend rule: one more water source in the column and its four walls\n\
         extend n: 0..16\n\
         extend base: {n}\n\
         extend generator: mctas circuit gen bubble-column --n {{n}} --out {{out}}\n\
         port items: 0 1 0\n\
         port output: 0 {} 0\n\
         port start: 1 3 0 last\n\
         origin: 0 0 -1\n\
         palette:\n  H minecraft:hopper[enabled=true,facing=east]\n  K minecraft:hopper[enabled=true,facing=west]\n  \
         D minecraft:dropper[facing=east,triggered=false]\n  \
         U minecraft:observer[facing=up,powered=false]\n  N minecraft:observer[facing=down,powered=false]\n  \
         S minecraft:soul_sand\n  W minecraft:water[level=0]\n  G minecraft:glass\n  T minecraft:stone\n",
        40 + 5 * n,
        h + 1
    );
    for (i, l) in layers.iter().enumerate() {
        s.push_str(&format!("layer {}:\n  {}\n  {}\n  {}\n", i as i32 - 1, l[0], l[1], l[2]));
    }
    s
}

/// The resting bubble-column family (`mctas circuit gen bubble-rest --n N`): an elevator N = 3 + n
/// blocks up in 40 + 5n blocks that rests when no item comes. The bubble column of
/// [`bubble_column`]; the dropper's clock runs only while the dropper holds an item: a comparator
/// reads the dropper and its signal reaches the sticky piston below the cell in front of it
/// (through the cell above the piston), which pushes an observer up beside the clock's other
/// observer; the pair then pulses every 6 gt (the lower one's back powers the stone above the
/// dropper hard). With the dropper empty the piston pulls the observer back and the clock stops.
pub fn bubble_rest(n: u32) -> String {
    let h = 3 + n;
    let mut layers: Vec<[&str; 4]> = vec![
        [".....", "..S..", ".T...", ".P..."],
        ["..G..", "HDWG.", ".CG..", ".N..."],
        ["..G..", ".TWG.", ".OG..", "....."],
    ];
    for _ in 2..=h {
        layers.push(["..G..", ".GWG.", "..G..", "....."]);
    }
    layers.push(["..G..", ".KWG.", "..G..", "....."]);
    layers.push([".GG..", "G.WG.", ".GG..", "....."]);
    layers.push([".....", "..G..", ".....", "....."]);
    let mut s = format!(
        "name: bubble-rest-n{n}\n\
         # A bubble-column elevator {h} blocks up ({} blocks) that rests when no item comes: the hopper under the\n\
         # input chest feeds a dropper that shoots each item into a water column over soul sand; the bubble\n\
         # column lifts it to the top source, under a ceiling, whose flow carries it back over the collecting\n\
         # hopper on the dropper's side (the items drift away from the dropper as they rise), which pushes it\n\
         # into the output chest. The dropper's clock runs only while the dropper holds an item: a comparator\n\
         # reads the dropper, and its signal reaches the sticky piston below the cell in front of it (a piston\n\
         # is also powered through the cell above it), which pushes an observer up beside the clock's other\n\
         # observer; the pair then pulses every 6 gt, and the lower one's back powers the stone above the\n\
         # dropper hard (the hopper beside the dropper stays unpowered). With the dropper empty the piston pulls\n\
         # the observer back down and the clock stops.\n\
         circuit: item-elevator {h}\n\
         extend family: bubble-rest\n\
         extend axis: 3 + n\n\
         extend blocks: 40 + 5n\n\
         extend rule: one more water source in the column and its four walls\n\
         extend n: 0..17\n\
         extend base: {n}\n\
         extend generator: mctas circuit gen bubble-rest --n {{n}} --out {{out}}\n\
         port items: 0 1 0\n\
         port output: 0 {} 0\n\
         origin: 0 0 -1\n\
         palette:\n  H minecraft:hopper[enabled=true,facing=east]\n  K minecraft:hopper[enabled=true,facing=west]\n  \
         D minecraft:dropper[facing=east,triggered=false]\n  \
         C minecraft:comparator[facing=north,mode=compare,powered=false]\n  O minecraft:observer[facing=south,powered=false]\n  \
         N minecraft:observer[facing=north,powered=false]\n  P minecraft:sticky_piston[extended=false,facing=up]\n  \
         S minecraft:soul_sand\n  W minecraft:water[level=0]\n  G minecraft:glass\n  T minecraft:stone\n",
        40 + 5 * n,
        h + 1
    );
    for (i, l) in layers.iter().enumerate() {
        s.push_str(&format!("layer {}:\n  {}\n  {}\n  {}\n  {}\n", i as i32 - 1, l[0], l[1], l[2], l[3]));
    }
    s
}

/// The dropper-bend family (`mctas circuit gen dropper-bend --n N`): an elevator N = 5 + 2n blocks
/// up in 14 + 5n blocks (lift5-chain-14b is n = 0), the odd heights. The clock observer at the
/// second dropper leads (its partner is placed last) and a four-observer bend from it fires the
/// fifth dropper's group 4 gt later; then a straight chain climbs a block every 2 gt and an observer
/// every two blocks fires the next three droppers 4 gt after the ones below.
pub fn dropper_bend(n: u32) -> String {
    let h = 5 + 2 * n;
    let mut layers = vec!["HD..".to_string(), ".DEW".into(), ".DV.".into(), ".DVW".into(), ".DEV".into()];
    for y in 5..h {
        layers.push(if y % 2 == 1 { ".D.V".into() } else { ".DEV".into() });
    }
    layers.push(".D..".into());
    let mut s = format!(
        "name: lift-bend-n{n}\n\
         # A dropper elevator {h} blocks up ({} blocks): the hopper under the input chest feeds a column of\n\
         # droppers facing up, the top one pushing into the output chest. One observer clock (6 gt) fires the\n\
         # column: its observer at the second dropper fires droppers 0-2 and leads (its partner is placed last:\n\
         # a player starts the clock by placing it); a four-observer bend from it fires droppers 3-5 4 gt later;\n\
         # then a chain of observers facing down climbs a block every 2 gt and an observer every two blocks fires\n\
         # the next three droppers 4 gt after the ones below (a hard-powered dropper powers the ones above and\n\
         # below it). A dropper also reads the power of the dropper two above (quasi-connectivity), so each\n\
         # group must be off whenever the group below turns off.\n\
         circuit: item-elevator {h}\n\
         extend family: dropper-bend\n\
         extend axis: 5 + 2n\n\
         extend blocks: 14 + 5n\n\
         extend rule: two more droppers on the column, two more chain observers and one observer firing the new top group\n\
         extend n: 0..8\n\
         extend base: {n}\n\
         extend generator: mctas circuit gen dropper-bend --n {{n}} --out {{out}}\n\
         port items: 0 1 0\n\
         port output: 1 {} 0\n\
         port start: 3 1 0 last\n\
         origin: 0 0 0\n\
         palette:\n  H minecraft:hopper[enabled=true,facing=east]\n  D minecraft:dropper[facing=up,triggered=false]\n  \
         E minecraft:observer[facing=east,powered=false]\n  W minecraft:observer[facing=west,powered=false]\n  \
         V minecraft:observer[facing=down,powered=false]\n",
        14 + 5 * n,
        h + 1
    );
    for (y, l) in layers.iter().enumerate() {
        s.push_str(&format!("layer {y}:\n  {l}\n"));
    }
    s
}

/// The dropper-pair family (`mctas circuit gen dropper-pair --n N`): an elevator N = 4 + 2n blocks
/// up in 11 + 5n blocks. The clock observer at the second dropper is placed last (`port start`), so
/// its partner leads by 2 gt; a straight chain of observers facing down climbs from the partner a
/// block every 2 gt, and an observer every two blocks fires the next group of three droppers 4 gt
/// after the one below (the clock observer fires droppers 0-2).
pub fn dropper_pair(n: u32) -> String {
    let h = 4 + 2 * n;
    let mut layers = vec!["HD..".to_string(), ".DEW".into()];
    for y in 2..h {
        layers.push(if y % 2 == 1 { ".DEV".into() } else { ".D.V".into() });
    }
    layers.push(".D..".into());
    let mut s = format!(
        "name: lift-pair-n{n}\n\
         # A dropper elevator {h} blocks up ({} blocks): the hopper under the input chest feeds a column of\n\
         # droppers facing up, the top one pushing into the output chest. One observer clock (6 gt) fires the\n\
         # column: its observer at the second dropper is placed last (a player starts the clock by placing\n\
         # it), so its partner leads by 2 gt; a chain of observers facing down climbs from the partner a block\n\
         # every 2 gt, and an observer every two blocks fires the next three droppers 4 gt after the ones below\n\
         # (a hard-powered dropper powers the ones above and below it). A dropper also reads the power of the\n\
         # dropper two above (quasi-connectivity), so each group must be off whenever the group below turns off.\n\
         circuit: item-elevator {h}\n\
         extend family: dropper-pair\n\
         extend axis: 4 + 2n\n\
         extend blocks: 11 + 5n\n\
         extend rule: two more droppers on the column, two more chain observers and one observer firing the new top group\n\
         extend n: 0..8\n\
         extend base: {n}\n\
         extend generator: mctas circuit gen dropper-pair --n {{n}} --out {{out}}\n\
         port items: 0 1 0\n\
         port output: 1 {} 0\n\
         port start: 2 1 0 last\n\
         origin: 0 0 0\n\
         palette:\n  H minecraft:hopper[enabled=true,facing=east]\n  D minecraft:dropper[facing=up,triggered=false]\n  \
         E minecraft:observer[facing=east,powered=false]\n  W minecraft:observer[facing=west,powered=false]\n  \
         V minecraft:observer[facing=down,powered=false]\n",
        11 + 5 * n,
        h + 1
    );
    for (y, l) in layers.iter().enumerate() {
        s.push_str(&format!("layer {y}:\n  {l}\n"));
    }
    s
}

/// The dropper-chain family (`mctas circuit gen dropper-lift --n N`): an elevator N = 4 + 2n
/// blocks up in 13 + 5n blocks (lift10-chain-28b is n = 3, lift20-chain-53b n = 8). A column of
/// droppers facing up fed by the hopper under the input chest; the clock observer at the second
/// dropper (its partner placed last) fires droppers 0-2, a four-observer bend fires 3-5 4 gt later,
/// then every two blocks of a straight chain fire the next group 4 gt after the one below.
pub fn dropper_chain(n: u32) -> String {
    let h = 4 + 2 * n;
    let mut layers = vec!["HD..".to_string(), ".DEW".into(), ".DV.".into(), ".DVW".into(), ".DEV".into()];
    for y in 5..=h {
        layers.push(if y % 2 == 1 { ".D.V".into() } else { ".DEV".into() });
    }
    let mut s = format!(
        "name: lift-chain-n{n}\n\
         # A dropper elevator {h} blocks up ({} blocks): the hopper under the input chest feeds a column of\n\
         # droppers facing up, the top one pushing into the output chest. One observer clock (6 gt, its partner\n\
         # placed last: a player starts it) fires the column through a chain of observers facing down; the\n\
         # observer whose back touches a dropper powers it hard, and a hard-powered dropper powers the ones\n\
         # above and below it. The clock's own observer fires droppers 0-2, a four-observer bend fires 3-5 4 gt\n\
         # later, then every two blocks of a straight chain fire the next three 4 gt after the ones below:\n\
         # a dropper also reads the power of the dropper two above (quasi-connectivity), so each group must be\n\
         # off whenever the group below turns off.\n\
         circuit: item-elevator {h}\n\
         extend family: dropper-chain\n\
         extend axis: 4 + 2n\n\
         extend blocks: 13 + 5n\n\
         extend rule: two more droppers on the column, two more chain observers and one observer firing the new top group\n\
         extend n: 0..8\n\
         extend base: {n}\n\
         extend generator: mctas circuit gen dropper-lift --n {{n}} --out {{out}}\n\
         port items: 0 1 0\n\
         port output: 1 {} 0\n\
         port start: 3 1 0 last\n\
         origin: 0 0 0\n\
         palette:\n  H minecraft:hopper[enabled=true,facing=east]\n  D minecraft:dropper[facing=up,triggered=false]\n  \
         E minecraft:observer[facing=east,powered=false]\n  W minecraft:observer[facing=west,powered=false]\n  \
         V minecraft:observer[facing=down,powered=false]\n",
        13 + 5 * n,
        h + 1
    );
    for (y, l) in layers.iter().enumerate() {
        s.push_str(&format!("layer {y}:\n  {l}\n"));
    }
    s
}
