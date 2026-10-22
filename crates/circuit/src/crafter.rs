//! The auto-crafter study (designs/circuits/crafter/WR.md, docs/circuits.md "Auto-crafters"): kind
//! `crafter <result>` (`circuit: crafter iron_block`). A crafter with a set recipe is fed its
//! ingredients from chests, crafts whenever a complete set is in its grid, and sends every result
//! to an output chest.
//!
//! Judged in the game by its own runner, on the sorter's benches ([`crate::sorter`]: the same runs
//! in the game and on redsim), on a PURE supply (the coordinator 2026-10-21: "only the recipe's
//! ingredients arrive, as players usually feed a crafter from a sorted line"). The judge puts a
//! chest on every `items` port and on `output` (air cells, not counted):
//! * one bare `items`: every ingredient goes into that chest (a multi-ingredient recipe's mixed);
//! * or one `items.<ingredient>` per ingredient (`items.gunpowder`, `items.sand`): each gets its
//!   own ingredient (two farms, two lines).
//!
//! The runs ([`Run`]):
//! * `base` (N positions x 4 rotations, the metrics): full and partial stacks, kept topped up so
//!   the design's intake runs at hopper speed. Once everything is quiet (nothing delivered,
//!   nothing taken from the supply for [`QUIET`] gt) the judge checks the crafts so far: every
//!   complete set crafted, the partial set left waiting (no craft of a partly filled grid: one iron
//!   ingot makes nine nuggets, one plank a button). Then it supplies a tail that completes the
//!   waiting set and leaves a new partial one, and checks again once quiet. Required: the output
//!   gets exactly floor(supplied / per craft) crafts of the result and nothing else (the scarcest
//!   ingredient for a multi-ingredient recipe), every ingredient is accounted for (crafted,
//!   waiting in the design, or never taken from its chest), the design's containers hold nothing
//!   else than at the start besides the waiting ingredients, no item entity, then rest.
//! * `intrusion` (4 rotations): the same, with stray items dropped on every open hopper top at a
//!   quarter, half and three quarters of the stream (the real-use audit's,
//!   [`crate::sorter::STRAYS`]): none may get in.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, SEG, Stack};
use crate::sorter::{self, Bench, Feed, GameBench, Ports, RunLog, SimBench, bp};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// A recipe the study judges.
#[derive(Debug)]
pub struct Recipe {
    /// The result (full id) and how many one craft gives.
    pub result: &'static str,
    pub count: u32,
    /// One craft's ingredients: (full id, items per craft).
    pub ingredients: &'static [(&'static str, u32)],
    /// The board's recipe class: `2x2` (a recipe that fits a 2x2 grid), `3x3` (nine of one item),
    /// `multi` (several ingredients: where each goes in the grid matters).
    pub class: &'static str,
}

/// The recipes of the study (the coordinator 2026-10-21: "recipes 2x2 (e.g. crafting
/// table/planks->sticks) and 3x3 (e.g. a 9-ingredient block like iron block)", and "a
/// MULTI-INGREDIENT class too (e.g. TNT from gunpowder + sand, the real-use one)").
pub const RECIPES: [Recipe; 4] = [
    Recipe { result: "minecraft:iron_block", count: 1, ingredients: &[("minecraft:iron_ingot", 9)], class: "3x3" },
    Recipe { result: "minecraft:crafting_table", count: 1, ingredients: &[("minecraft:oak_planks", 4)], class: "2x2" },
    Recipe { result: "minecraft:stick", count: 4, ingredients: &[("minecraft:oak_planks", 2)], class: "2x2" },
    Recipe { result: "minecraft:tnt", count: 1, ingredients: &[("minecraft:gunpowder", 5), ("minecraft:sand", 4)], class: "multi" },
];

/// The quiet window before the judge checks the crafts and supplies the tail (and ends): nothing
/// delivered and nothing taken from the supply for this long.
pub const QUIET: u32 = 400;

/// `minecraft:` + the id.
pub fn full_id(id: &str) -> String {
    sorter::full_id(id)
}

/// The recipe of a result (full or short id).
pub fn recipe(result: &str) -> Option<&'static Recipe> {
    let id = full_id(result);
    RECIPES.iter().find(|r| r.result == id)
}

/// The study's results, for messages.
pub fn results() -> String {
    RECIPES.iter().map(|r| r.result.trim_start_matches("minecraft:")).collect::<Vec<_>>().join(", ")
}

/// An auto-crafter's box (its blocks; the judge's chests left out): the archive's size.
pub fn dims(c: &Circuit) -> [i32; 3] {
    shulker::design_dims(c)
}

/// The judge's runs ([`verify_in_pool`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Base,
    Intrusion,
}

impl Run {
    pub fn parse(s: &str) -> Option<Run> {
        Some(match s {
            "base" => Run::Base,
            "intrusion" | "strays" => Run::Intrusion,
            _ => return None,
        })
    }

    pub fn title(self) -> &'static str {
        match self {
            Run::Base => "full and partial stacks from the supply chests",
            Run::Intrusion => "stray items dropped on every open hopper top",
        }
    }
}

/// One run's inputs: per supply port (`items`, `items.<ingredient>`), the stacks it gets before
/// the quiet check, then the tail's.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub run: Run,
    pub recipe: &'static Recipe,
    pub feeds: Vec<(String, Vec<Stack>)>,
    pub tails: Vec<(String, Vec<Stack>)>,
    /// (gt, stack) dropped on every open hopper top at the first visit from that gt on.
    pub strays: Vec<(u32, Stack)>,
    pub cap: u32,
}

/// Split `n` items into stacks of at most 64, a full stack first, then the rest as given sizes
/// (`sizes`, cycled), so the supply mixes full and partial stacks.
fn stacks_of(id: &str, n: u32, sizes: &[u32]) -> Vec<Stack> {
    let mut v = Vec::new();
    let mut left = n;
    let mut i = 0;
    while left > 0 {
        let s = sizes[i % sizes.len()].min(64).min(left);
        v.push(Stack::new(id, s));
        left -= s;
        i += 1;
    }
    v
}

impl Scenario {
    /// The base run (or the intrusion run) for a design of `recipe` whose supply ports are
    /// `ports` (`items`, or `items.<ingredient>` each).
    pub fn new(run: Run, recipe: &'static Recipe, ports: &[String]) -> Result<Scenario> {
        // phase 1: the same number of complete sets of every ingredient (19, or 183 items of a
        // single one), each with a partial set left over; the tail completes it, adds one more set
        // (and, for a multi-ingredient recipe, two sets' worth of the last ingredient in excess:
        // it waits)
        let single = recipe.ingredients.len() == 1;
        let mut first: Vec<(&str, u32)> = Vec::new();
        let mut tail: Vec<(&str, u32)> = Vec::new();
        for (i, (id, k)) in recipe.ingredients.iter().enumerate() {
            let n1 = if single { 183 } else { 19 * k + 1 };
            let r1 = n1 % k;
            let extra = if !single && i + 1 == recipe.ingredients.len() { 2 * k } else { 0 };
            first.push((id, n1));
            tail.push((id, (k - r1) + k + 1 + extra));
        }
        let sizes = [64, 21, 1, 33];
        let by_port = |phase: &[(&str, u32)]| -> Result<Vec<(String, Vec<Stack>)>> {
            if ports.len() == 1 && ports[0] == "items" {
                // one chest: the ingredients' stacks interleaved
                let lists: Vec<Vec<Stack>> = phase.iter().map(|(id, n)| stacks_of(id, *n, &sizes)).collect();
                let mut v = Vec::new();
                let most = lists.iter().map(|l| l.len()).max().unwrap_or(0);
                for j in 0..most {
                    for l in &lists {
                        if let Some(s) = l.get(j) {
                            v.push(s.clone());
                        }
                    }
                }
                return Ok(vec![("items".to_string(), v)]);
            }
            let mut out = Vec::new();
            for p in ports {
                let sub = p.strip_prefix("items.").ok_or_else(|| anyhow!("port {p}: the supply is one `items` chest or one `items.<ingredient>` per ingredient"))?;
                let id = full_id(sub);
                let (_, n) = phase.iter().find(|(i, _)| *i == id).ok_or_else(|| anyhow!("port {p}: {} is not an ingredient of {} ({})", sub, recipe.result, recipe.ingredients.iter().map(|x| x.0.trim_start_matches("minecraft:")).collect::<Vec<_>>().join(", ")))?;
                out.push((p.clone(), stacks_of(&id, *n, &sizes)));
            }
            for (id, _) in phase {
                if !out.iter().any(|(p, _)| full_id(p.trim_start_matches("items.")) == *id) {
                    bail!("no supply port for {} (`port items.{}: x y z`, or one `port items:` for every ingredient)", id, id.trim_start_matches("minecraft:"));
                }
            }
            Ok(out)
        };
        let feeds = by_port(&first)?;
        let tails = by_port(&tail)?;
        let n: u64 = feeds.iter().chain(tails.iter()).flat_map(|(_, v)| v.iter()).map(|s| s.count as u64).sum();
        // one hopper's pace (8 gt an item) x 1.5, two quiet windows, + 3000
        let cap = (n * 12) as u32 + 2 * QUIET + 3000;
        let mut sc = Scenario { run, recipe, feeds, tails, strays: Vec::new(), cap };
        if run == Run::Intrusion {
            let span = n as u32 * 8;
            sc.strays = sorter::STRAYS.iter().enumerate().map(|(k, (id, c))| (span * (k as u32 + 1) / 4, Stack::new(id, *c))).collect();
        }
        Ok(sc)
    }

    /// Items of an ingredient supplied before the tail, and in all.
    pub fn supplied(&self, id: &str, with_tail: bool) -> u64 {
        let phases: Vec<&Vec<(String, Vec<Stack>)>> = if with_tail { vec![&self.feeds, &self.tails] } else { vec![&self.feeds] };
        phases.into_iter().flat_map(|p| p.iter()).flat_map(|(_, v)| v.iter()).filter(|s| s.id == id).map(|s| s.count as u64).sum()
    }

    /// The crafts the supply allows: the scarcest ingredient's complete sets.
    pub fn crafts(&self, with_tail: bool) -> u64 {
        self.recipe.ingredients.iter().map(|(id, k)| self.supplied(id, with_tail) / *k as u64).min().unwrap_or(0)
    }

    pub fn total(&self) -> u64 {
        self.feeds.iter().chain(self.tails.iter()).flat_map(|(_, v)| v.iter()).map(|s| s.count as u64).sum()
    }
}

/// The supply ports of a design (`items`, `items.<ingredient>`), in name order.
pub fn supply_ports(c: &Circuit) -> Vec<String> {
    c.stores.iter().filter(|s| s.role == "items").map(|s| s.name.clone()).collect()
}

/// The judge's chests: every supply port and the output; Err when one lands on a design block.
fn ports(c: &Circuit) -> std::result::Result<Ports, String> {
    let sources: Vec<Pos> = c.stores.iter().filter(|s| s.role == "items").map(|s| s.pos).collect();
    let sinks: Vec<(String, Pos)> = c.outputs.iter().map(|o| (if o.name == "out" { "output".to_string() } else { format!("output.{}", o.name) }, o.pos)).collect();
    let judge: Vec<Pos> = sources.iter().copied().chain(sinks.iter().map(|s| s.1)).collect();
    let blocks: Vec<(Pos, String)> = judge.iter().map(|p| (*p, CHEST.to_string())).collect();
    for (q, _) in &blocks {
        if let Some(s) = c.clock.design.blocks.get(q) {
            return Err(format!("the judge's chest at {q:?} lands on the design's {}", clock::short(s)));
        }
    }
    let strays: Vec<Pos> = crate::intrusion::intakes(c).into_iter().filter(|p| !judge.contains(p)).collect();
    Ok(Ports { sources, hopper: None, drop: None, sinks, judge, blocks, strays })
}

/// A run's supply chests: (position, port name) in the scenario's port order.
fn sources_of(c: &Circuit, sc: &Scenario) -> std::result::Result<Vec<Pos>, String> {
    sc.feeds.iter().map(|(name, _)| c.stores.iter().find(|s| s.name == *name).map(|s| s.pos).ok_or_else(|| format!("no port {name}"))).collect()
}

/// What the crafter run adds to a [`RunLog`]: the crafts at the quiet check before the tail, and
/// the supply's leftovers per kind at the end.
#[derive(Clone, Debug, Default)]
pub struct CraftLog {
    pub log: RunLog,
    pub before_tail: u64,
    pub left: BTreeMap<String, u64>,
}

/// Results delivered so far (every output).
fn results_of(log: &RunLog, result: &str) -> u64 {
    log.reached.values().map(|m| m.get(result).copied().unwrap_or(0)).sum()
}

/// A design aid: print the cells of the watched box that changed since the last dump (block state
/// and container contents), every [`SEG`] gt of a run (`crafter-run --trace`).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) trait Dump {
    /// Every non-air cell of the watched box with its state (and its container's stacks).
    fn cells(&mut self) -> Result<BTreeMap<Pos, String>>;
}

impl Dump for SimBench<'_> {
    fn cells(&mut self) -> Result<BTreeMap<Pos, String>> {
        use redsim::redstone::container as rc;
        let mut m = BTreeMap::new();
        for x in self.from[0]..=self.to[0] {
            for y in self.from[1]..=self.to[1] {
                for z in self.from[2]..=self.to[2] {
                    let s = self.w.get(bp([x, y, z])).to_string();
                    if mctas::blockstate::is_air(&s) {
                        continue;
                    }
                    let items = self.w.containers.map.get(&bp([x, y, z])).map(|c| c.slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| format!("{i}:{}x{}", rc::item_name(s.item), s.count))).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                    m.insert([x, y, z], format!("{} {items}", clock::short(&s)));
                }
            }
        }
        Ok(m)
    }
}

impl Dump for GameBench<'_> {
    fn cells(&mut self) -> Result<BTreeMap<Pos, String>> {
        let f = self.w.get_region(self.sb, self.from, self.to)?;
        let bes: BTreeMap<Pos, String> = self
            .w
            .block_entities(self.sb, self.from, self.to)?
            .into_iter()
            .map(|b| (b.pos, crate::shulker::stacks_of_be(&b.nbt).iter().map(|s| format!("{}x{}", clock::short(&s.id), s.count)).collect::<Vec<_>>().join(" ")))
            .collect();
        Ok(f.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(p, s)| (p, format!("{} {}", clock::short(s), bes.get(&p).cloned().unwrap_or_default()))).collect())
    }
}

/// [`TRACE`]: the cells that changed since `prev`.
fn dump<B: Dump>(b: &mut B, t: u32, prev: &mut BTreeMap<Pos, String>) -> Result<()> {
    let now = b.cells()?;
    for (p, s) in &now {
        if prev.get(p) != Some(s) {
            println!("t={t} {p:?} {s}");
        }
    }
    for p in prev.keys() {
        if !now.contains_key(p) {
            println!("t={t} {p:?} air");
        }
    }
    *prev = now;
    Ok(())
}

/// The run's loop, on any bench: supply every chest, count and empty the outputs every [`SEG`]
/// gt; once quiet, note the crafts and supply the tail; once quiet again, stop (or at the cap).
fn drive<B: Bench + Dump>(b: &mut B, sc: &Scenario, p: &Ports, srcs: &[Pos], cl: &mut CraftLog) -> Result<()> {
    let trace = TRACE.load(std::sync::atomic::Ordering::Relaxed);
    let mut prev = BTreeMap::new();
    let one = |q: Pos| Ports { sources: vec![q], ..p.clone() };
    let mut cursors = vec![0usize; srcs.len()];
    let mut tail_cursors = vec![0usize; srcs.len()];
    let feed = |b: &mut B, phase: &[(String, Vec<Stack>)], cursors: &mut [usize], log: &mut RunLog| -> Result<()> {
        for (k, q) in srcs.iter().enumerate() {
            sorter::supply(b, Feed::Chest, &one(*q), &phase[k].1, &mut cursors[k], log)?;
        }
        Ok(())
    };
    let log = &mut cl.log;
    feed(b, &sc.feeds, &mut cursors, log)?;
    let (mut phase, mut quiet, mut t, mut strays) = (1u32, 0u32, 0u32, 0usize);
    let mut last_left = u64::MAX;
    loop {
        b.step(SEG)?;
        t += SEG;
        b.note(t, &mut log.notes)?;
        if trace {
            dump(b, t, &mut prev)?;
        }
        while strays < sc.strays.len() && t >= sc.strays[strays].0 {
            sorter::drop_strays(b, p, &sc.strays[strays].1, log)?;
            log.notes.push(format!("t={t}: the judge drops {} x{} on each of {} open hopper top(s) {:?}", clock::short(&sc.strays[strays].1.id), sc.strays[strays].1.count, p.strays.len(), p.strays));
            strays += 1;
        }
        let before: u64 = log.reached.values().flat_map(|m| m.values()).sum();
        for (name, q) in &p.sinks {
            let have = b.read(*q)?;
            if have.is_empty() {
                continue;
            }
            let m = log.reached.entry(name.clone()).or_default();
            for s in &have {
                *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(*q, &[])?;
        }
        let now: u64 = log.reached.values().flat_map(|m| m.values()).sum();
        if now != before {
            log.flow.push((t, results_of(log, sc.recipe.result)));
        }
        if phase == 1 {
            feed(b, &sc.feeds, &mut cursors, log)?;
        } else {
            feed(b, &sc.tails, &mut tail_cursors, log)?;
        }
        let left = sorter::supply_left(b, p)?;
        quiet = if now == before && left == last_left { quiet + SEG } else { 0 };
        last_left = left;
        let supplied = |cur: &[usize], ph: &[(String, Vec<Stack>)]| cur.iter().zip(ph.iter()).all(|(c, (_, v))| *c >= v.len());
        if phase == 1 && quiet >= QUIET && supplied(&cursors, &sc.feeds) && strays >= sc.strays.len().min(1) {
            cl.before_tail = results_of(log, sc.recipe.result);
            log.tail_at = t;
            log.notes.push(format!("t={t}: quiet: {} result(s) so far; the judge supplies the tail", cl.before_tail));
            phase = 2;
            quiet = 0;
            feed(b, &sc.tails, &mut tail_cursors, log)?;
        } else if phase == 2 && quiet >= QUIET && supplied(&tail_cursors, &sc.tails) && strays >= sc.strays.len() {
            break;
        }
        if t >= sc.cap {
            log.timed_out = true;
            log.notes.push(format!("stopped at the cap in phase {phase}"));
            break;
        }
    }
    log.lying = sorter::strays_lying(b, p)?;
    log.end = t;
    for (k, q) in srcs.iter().enumerate() {
        let _ = k;
        for s in b.read(*q)? {
            *cl.left.entry(s.id.clone()).or_insert(0) += s.count as u64;
        }
    }
    Ok(())
}

/// Run a scenario on a circuit already turned and moved, in a fresh game sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, sc: &Scenario, margin: i32) -> Result<std::result::Result<CraftLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, sc, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos) -> Result<std::result::Result<CraftLog, String>> {
    let p = match ports(c) {
        Ok(p) => p,
        Err(why) => return Ok(Err(why)),
    };
    let srcs = match sources_of(c, sc) {
        Ok(s) => s,
        Err(why) => return Ok(Err(why)),
    };
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    w.set_blocks(sb, &p.blocks, PlaceMode::Raw)?;
    let mut b = GameBench { w, sb, from, to, base: None, frames: None };
    b.step(c.clock.free_from() + SETTLE)?;
    b.base = Some(b.w.get_region(sb, from, to)?);
    let mut cl = CraftLog { log: RunLog { dims: shulker::design_dims(c), initial: sorter::held(b.w, sb, from, to, &p.judge)?, ..Default::default() }, ..Default::default() };
    drive(&mut b, sc, &p, &srcs, &mut cl)?;
    sorter::finish(w, sb, c, from, to, &p, &mut cl.log)?;
    Ok(Ok(cl))
}

/// The same run on redsim (milliseconds): `mctas circuit crafter-run --sim` and the searches'
/// screen. Err: the design cannot stand or redsim does not model it.
pub fn sim_run(c: &Circuit, sc: &Scenario, margin: i32) -> std::result::Result<CraftLog, String> {
    let (from, to) = crate::run::watch_box(c, margin);
    if let Some((p, st)) = clock::sim::unsupported(&c.clock, from, to)?.first() {
        return Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(st)));
    }
    let p = ports(c)?;
    let srcs = sources_of(c, sc)?;
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    for (q, s) in &p.blocks {
        w.set_block(bp(*q), redsim::Block::parse(s)?, redsim::world::flags::ALL);
    }
    let mut b = SimBench { w: &mut w, from, to, frames: None, coal: None, watch: None };
    b.step(c.clock.free_from() + SETTLE).map_err(|e| format!("{e:#}"))?;
    let mut cl = CraftLog { log: RunLog { dims: shulker::design_dims(c), initial: sorter::sim_held(b.w, &p.judge), ..Default::default() }, ..Default::default() };
    drive(&mut b, sc, &p, &srcs, &mut cl).map_err(|e| format!("{e:#}"))?;
    // rest: the whole state repeats over a window of REST gt, nothing scheduled
    let mut key = w.state_key();
    let mut still = false;
    for _ in 0..5 {
        for _ in 0..REST {
            w.tick();
        }
        let k = w.state_key();
        if k == key && w.idle() {
            still = true;
            break;
        }
        key = k;
    }
    if let Some(e) = w.errors.first() {
        return Err(format!("redsim: {e}"));
    }
    let log = &mut cl.log;
    if !still {
        log.restless = format!("the state still changes {} gt after the last item", 5 * REST);
    }
    log.awake = c.clock.design.blocks.keys().filter(|p| matches!(w.get(bp(**p)), redsim::Block::Hopper { enabled: true, .. })).count() as u32;
    log.final_held = sorter::sim_held(&w, &p.judge);
    log.loose = w.entities.list.iter().filter(|e| !e.removed && !sorter::on_top(&p.strays, [e.pos.x, e.pos.y, e.pos.z])).map(|e| format!("{:?}", e.kind)).collect();
    if let Some((p, b)) = w.drops.first() {
        log.loose.push(format!("{b} at {p} broke with a drop"));
    }
    Ok(cl)
}

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(sc: &Scenario, cl: &CraftLog) -> Outcome {
    let log = &cl.log;
    let r = sc.recipe;
    let mut table: Vec<String> = Vec::new();
    for (port, m) in &log.reached {
        let list: Vec<String> = m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect();
        table.push(format!("{port}: {}", list.join(", ")));
    }
    let l = |m: &BTreeMap<String, u64>| if m.is_empty() { "none".to_string() } else { m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ") };
    table.push(format!("supplied {} items; left in the supply chests: {}; end t={}", log.supplied, l(&cl.left), log.end));
    if !log.dropped.is_empty() {
        table.push(format!("strays dropped: {}; still lying on a hopper top: {}", l(&log.dropped), l(&log.lying)));
    }
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    for (port, m) in &log.reached {
        if let Some((wrong, n)) = m.iter().find(|(id, _)| id.as_str() != r.result) {
            return fail(format!("{} x{n} reached {port} (only {} belongs there: a wrong craft, or an ingredient let through)", clock::short(wrong), clock::short(r.result)), table);
        }
    }
    if log.timed_out {
        return fail(format!("{} results after {} gt (the cap)", results_of(log, r.result), log.end), table);
    }
    let want1 = sc.crafts(false) * r.count as u64;
    if log.tail_at > 0 && cl.before_tail != want1 {
        return fail(format!("{} {} before the tail, {want1} expected ({} complete sets supplied{})", cl.before_tail, clock::short(r.result), sc.crafts(false), if cl.before_tail < want1 { ": a complete set waits uncrafted" } else { "" }), table);
    }
    let got = results_of(log, r.result);
    let want = sc.crafts(true) * r.count as u64;
    if got != want {
        return fail(format!("{got} {} in all, {want} expected ({} complete sets supplied{})", clock::short(r.result), sc.crafts(true), if got < want { ": a complete set never crafted (the partial set jammed?)" } else { "" }), table);
    }
    // the strays: none may get in
    for (id, n) in &log.dropped {
        let lying = log.lying.get(id).copied().unwrap_or(0);
        if lying != *n {
            return fail(format!("{} of {n} stray {} got in (dropped on an open hopper top)", n - lying, clock::short(id)), table);
        }
    }
    // every ingredient accounted for: crafted, waiting in the design, or never taken in
    let crafts = sc.crafts(true);
    for (id, k) in r.ingredients {
        let supplied = sc.supplied(id, true);
        let used = crafts * *k as u64;
        let left = cl.left.get(*id).copied().unwrap_or(0);
        let waiting = log.final_held.get(*id).copied().unwrap_or(0) as i64 - log.initial.get(*id).copied().unwrap_or(0) as i64;
        if used as i64 + left as i64 + waiting != supplied as i64 {
            return fail(format!("{} {}: {supplied} supplied, {used} crafted, {left} left in its chest, {waiting} more in the design than at the start (lost or duplicated)", clock::short(id), if waiting < 0 { "(the design lost some of its own)" } else { "" }), table);
        }
    }
    // nothing else changed in the design's containers (no wrong craft kept, no recipe lost)
    let ingredient = |k: &str| r.ingredients.iter().any(|(id, _)| *id == k);
    let keys: std::collections::BTreeSet<&String> = log.initial.keys().chain(log.final_held.keys()).collect();
    let changed: Vec<String> = keys.into_iter().filter(|k| !ingredient(k) && log.initial.get(*k) != log.final_held.get(*k)).map(|k| format!("{} {} -> {}", clock::short(k), log.initial.get(k).copied().unwrap_or(0), log.final_held.get(k).copied().unwrap_or(0))).collect();
    if !changed.is_empty() {
        return fail(format!("the design's containers changed: {}", changed.join(", ")), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    if !log.restless.is_empty() {
        return fail(log.restless.clone(), table);
    }
    // crafts per minute: the flow between a tenth and nine tenths of the results (phase 1: the
    // full-speed part)
    let total = want1.max(1);
    let at = |q: u64| log.flow.iter().find(|(_, m)| *m >= q).copied();
    let rate = match (at(total.div_ceil(10)), at(total * 9 / 10)) {
        (Some((ta, qa)), Some((tb, qb))) if tb > ta => (qb - qa) as f64 / r.count as f64 * 1200.0 / (tb - ta) as f64,
        _ => 0.0,
    };
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let metrics = vec![
        ("crafts/min".to_string(), format!("{rate:.1}")),
        ("first craft".to_string(), format!("by {first} gt")),
        ("crafts".to_string(), format!("{} ({} waiting sets checked)", crafts, 2)),
        ("hoppers awake".to_string(), log.awake.to_string()),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The scenario of one run for a design.
pub fn scenario_for(c: &Circuit, run: Run) -> Result<Scenario> {
    let Some(Kind::Crafter(result)) = &c.kind else { bail!("{}: not an auto-crafter", c.name()) };
    let r = recipe(result).ok_or_else(|| anyhow!("crafter {result}: not a recipe of the study ({})", results()))?;
    Scenario::new(run, r, &supply_ports(c))
}

/// The judge for the auto-crafter kind (`mctas circuit verify`): the base run at N positions x 4
/// rotations, then the intrusion run at 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let sc = scenario_for(c, Run::Base)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|cl| BuildOk { outcome: judge(&sc, &cl), dims: cl.log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = sc.cap;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind.clone()), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    let si = scenario_for(c, Run::Intrusion)?;
    let jobs: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let results = pool.map(jobs.clone(), |w, _, t| {
        let rc = c.rotated(w, t)?;
        game_run(w, &rc, &si, o.margin)
    });
    let mut line = format!("{}: passes ({} rotations)", Run::Intrusion.title(), jobs.len());
    for (t, r) in jobs.into_iter().zip(results) {
        let why = match r {
            Ok(Ok(cl)) => {
                let out = judge(&si, &cl);
                if out.pass { None } else { Some(out.why) }
            }
            Ok(Err(why)) => Some(why),
            Err(e) => Some(format!("{e:#}")),
        };
        if let Some(why) = why {
            rep.pass = false;
            rep.verdict = format!("{}: rotation {t}: {why}", Run::Intrusion.title());
            return Ok(rep);
        }
    }
    if crate::intrusion::intakes(c).is_empty() {
        line = "sealed: no open hopper top (no stray can get in)".to_string();
    }
    let r = recipe(match &kind {
        Kind::Crafter(x) => x,
        _ => "",
    });
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("recipe class".to_string(), r.map(|r| r.class.to_string()).unwrap_or_default()));
        k.outcome.metrics.push(("supply".to_string(), supply_ports(c).join(" + ")));
        k.outcome.metrics.push(("bill".to_string(), shulker::bill(c)));
        k.outcome.table.push(line);
    }
    Ok(rep)
}

/// A design aid (`mctas circuit crafter-run`): one run at rotation 0 in the game, its table.
pub fn run_one(pool: &Pool, c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let sc = scenario_for(c, run)?;
    let r = pool.map(vec![0u8], |w, _, _| game_run(w, c, &sc, margin));
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        Ok(cl) => Ok(judge(&sc, &cl)),
        Err(why) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// [`run_one`] on redsim (`mctas circuit crafter-run --sim`).
pub fn sim_one(c: &Circuit, run: Run, margin: i32) -> Result<Outcome> {
    let sc = scenario_for(c, run)?;
    Ok(match sim_run(c, &sc, margin) {
        Ok(cl) => judge(&sc, &cl),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}

/// The whole judge on redsim at rotation 0 (the searches' screen): the base run, then the
/// intrusion run.
pub fn sim_verdict(c: &Circuit) -> Outcome {
    sim_score(c).1
}

/// The searches' score of an auto-crafter on redsim (`circuit compact`): the base run, then the
/// intrusion run; the score counts the runs passed (x 1000) plus, for the first failing run, the
/// results it delivered (a gradient for broken designs, capped at 999). The outcome is the base
/// run's (failing with the first run that fails).
pub fn sim_score(c: &Circuit) -> (i64, Outcome) {
    let one = |r: Run| -> (Outcome, i64) {
        let sc = match scenario_for(c, r) {
            Ok(s) => s,
            Err(e) => return (Outcome::fail(format!("{e:#}"), Vec::new()), 0),
        };
        match sim_run(c, &sc, 3) {
            Ok(cl) => (judge(&sc, &cl), results_of(&cl.log, sc.recipe.result).min(999) as i64),
            Err(why) => (Outcome::fail(why, Vec::new()), 0),
        }
    };
    let (base, k) = one(Run::Base);
    if !base.pass {
        return (k, base);
    }
    let (strays, k) = one(Run::Intrusion);
    if !strays.pass {
        return (1000 + k, Outcome::fail(format!("{}: {}", Run::Intrusion.title(), strays.why), strays.table));
    }
    (2000, base)
}
