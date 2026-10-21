//! Study 13, item sorters (designs/circuits/tracks/sorter/WR.md, docs/circuits.md "Item
//! sorters"): kind `sorter <item>`. A slice takes items from a line, sends every item of its kind
//! into its chest and lets every other item go on along the line.
//!
//! Judged in the game by its own runner, like the shulker kinds ([`crate::shulker`]): the judge
//! puts a chest on each `items`, `pass` and `output` port (air cells, not counted), keeps the
//! source (`items`) topped up from a stream of stacks, counts and empties `pass` and the outputs at
//! each visit, then waits for rest and takes an inventory of the watched box.
//!
//! The base run: the design's kind mixed with other items (64-stack, 16-stack, non-stackable, full
//! and partial stacks, runs of the kind back to back). Every item of the kind must reach `output`,
//! every other item `pass`, nothing else; the design's containers must end as they started (the
//! filter recipe intact), no item entity, then rest. Badges: `tileable` (copies at the `tile:` step
//! chained along the line, each sorting its own kind: the judge rewrites the recipe's item) and
//! `overflow` (in that tiled run the middle copy's chest starts full but one slot and gets
//! [`OVERFLOW_STACKS`] stacks of its kind: its overflow may go on to `pass`, nothing may reach a
//! wrong chest, no copy may lose its recipe, and once the judge empties that chest the copy must
//! sort again).

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, SEG, Stack, items_snbt, stacks_of_be};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// Kinds the stream carries besides the sorted ones, by max stack; a run leaves out any kind the
/// design itself holds (its recipe and fillers). All in redsim's item registry, so the redsim
/// screen ([`sim_run`]) runs the same scenarios as the game.
pub const JUNK64: [&str; 5] = ["minecraft:cobblestone", "minecraft:stone", "minecraft:iron_block", "minecraft:slime_block", "minecraft:heavy_weighted_pressure_plate"];
pub const JUNK16: [&str; 3] = ["minecraft:snowball", "minecraft:ender_pearl", "minecraft:bucket"];
pub const JUNK1: [&str; 3] = ["minecraft:iron_shovel", "minecraft:wooden_shovel", "minecraft:diamond_sword"];
/// The kinds the tiled copies sort (copy 1, 2, ...), 64-stack like most recipes.
pub const COPY_KINDS64: [&str; 4] = ["minecraft:redstone", "minecraft:slime_ball", "minecraft:iron_nugget", "minecraft:iron_bars"];
/// The same for a 16-stack recipe.
pub const COPY_KINDS16: [&str; 3] = ["minecraft:ender_pearl", "minecraft:bucket", "minecraft:snowball"];
/// Copies in the tiled runs.
pub const COPIES: usize = 3;
/// Badge `overflow`: the stacks of its kind the full copy gets.
pub const OVERFLOW_STACKS: u32 = 8;
/// The judge's filler of a full chest (badge `overflow`): 26 of it (1-stack: each slot full), the
/// last slot free.
pub const FULL_FILLER: &str = "minecraft:music_disc_11";
/// Badge `overflow`: once the stream is supplied, the gt without any delivery before the judge
/// empties the full chest.
pub const QUIET: u32 = 200;

/// `minecraft:` + the id.
pub fn full_id(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

/// An item's max stack (redsim's registry; 64 for the kinds it does not know).
pub fn max_stack(id: &str) -> u32 {
    redsim::redstone::container::max_stack_of(&full_id(id)) as u32
}

/// One run's inputs.
#[derive(Clone, Debug)]
pub struct Scenario {
    /// The kind each copy sorts, copy 0 first (one entry: a single slice).
    pub sorts: Vec<String>,
    /// What the source supplies, in order.
    pub stream: Vec<Stack>,
    /// Badge `overflow`: this copy's chest starts full but one slot; once the stream is supplied and
    /// nothing arrives for [`QUIET`] gt the judge empties it and supplies `tail`.
    pub overflow: Option<usize>,
    pub tail: Vec<Stack>,
    /// Ticks after the start before giving up.
    pub cap: u32,
}

fn pick<'a>(from: &[&'a str], avoid: &[String], n: usize) -> Vec<&'a str> {
    from.iter().filter(|k| !avoid.iter().any(|a| a == *k)).take(n).copied().collect()
}

impl Scenario {
    fn with_cap(sorts: Vec<String>, stream: Vec<Stack>, overflow: Option<usize>, tail: Vec<Stack>) -> Scenario {
        let n: u64 = stream.iter().chain(tail.iter()).map(|s| s.count as u64).sum();
        // one hopper's pace (8 gt an item) x 1.5, + an overflow's drain, + 3000
        let cap = (n * 12) as u32 + if overflow.is_some() { 8000 } else { 0 } + 3000;
        Scenario { sorts, stream, overflow, tail, cap }
    }

    /// The base run for a design sorting `kind`, avoiding the kinds the design holds.
    pub fn base(kind: &str, avoid: &[String]) -> Result<Scenario> {
        let k = full_id(kind);
        let mut av = avoid.to_vec();
        av.push(k.clone());
        let j = pick(&JUNK64, &av, 3);
        let s = pick(&JUNK16, &av, 2);
        let u = pick(&JUNK1, &av, 1);
        if j.len() < 3 || s.len() < 2 || u.is_empty() {
            bail!("sorter: the design holds too many of the judge's stream kinds ({av:?})");
        }
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let stream = vec![
            st(&k, 64), st(j[0], 64), st(&k, 20), st(s[0], 16), st(j[1], 37), st(&k, 64), st(&k, 64), st(u[0], 1), st(s[1], 5), st(&k, 33), st(j[2], 64), st(u[0], 1), st(&k, 1), st(j[0], 10), st(&k, 64),
        ];
        Ok(Scenario::with_cap(vec![k], stream, None, Vec::new()))
    }

    /// The tiled runs: copy k sorts `sorts[k]`; `overflow`: the middle copy's chest starts full.
    pub fn tiled(base_kind: &str, avoid: &[String], overflow: bool) -> Result<Scenario> {
        let k0 = full_id(base_kind);
        let mut av = avoid.to_vec();
        av.push(k0.clone());
        let pool: &[&str] = if max_stack(&k0) == 16 { &COPY_KINDS16 } else { &COPY_KINDS64 };
        let more = pick(pool, &av, COPIES - 1);
        if more.len() < COPIES - 1 {
            bail!("sorter: no copy kinds left ({av:?})");
        }
        let mut sorts = vec![k0.clone()];
        sorts.extend(more.iter().map(|s| s.to_string()));
        av.extend(sorts.iter().cloned());
        let j = pick(&JUNK64, &av, 3);
        let s = pick(&JUNK16, &av, 2);
        let u = pick(&JUNK1, &av, 1);
        if j.len() < 3 || u.is_empty() {
            bail!("sorter: the design holds too many of the judge's stream kinds ({av:?})");
        }
        // a 16-stack recipe's copies use up the 16-stack kinds: 64-stack junk stands in
        let s: Vec<&str> = (0..2).map(|i| s.get(i).copied().unwrap_or(j[i])).collect();
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let (a, b, c) = (sorts[0].as_str(), sorts[1].as_str(), sorts[2].as_str());
        if overflow {
            let mut stream = Vec::new();
            for i in 0..OVERFLOW_STACKS {
                stream.push(st(b, 64));
                if i == 3 {
                    stream.extend([st(j[0], 64), st(a, 20), st(u[0], 1)]);
                }
            }
            stream.extend([st(c, 30), st(s[0], 16), st(j[1], 37), st(a, 5)]);
            let tail = vec![st(b, 20), st(j[2], 10), st(c, 5), st(a, 3)];
            return Ok(Scenario::with_cap(sorts, stream, Some(1), tail));
        }
        let stream = vec![
            st(a, 64), st(j[0], 64), st(b, 64), st(c, 20), st(s[0], 16), st(b, 30), st(a, 10), st(u[0], 1), st(c, 64), st(j[1], 37), st(a, 64), st(b, 1), st(s[1], 5), st(c, 33), st(j[2], 64), st(a, 1), st(b, 64),
        ];
        Ok(Scenario::with_cap(sorts, stream, None, Vec::new()))
    }

    /// Items supplied of a kind (stream and tail).
    pub fn supplied(&self, id: &str) -> u64 {
        self.stream.iter().chain(self.tail.iter()).filter(|s| s.id == id).map(|s| s.count as u64).sum()
    }

    pub fn total(&self) -> u64 {
        self.stream.iter().chain(self.tail.iter()).map(|s| s.count as u64).sum()
    }

    /// The kinds supplied, sorted kinds first.
    pub fn kinds(&self) -> Vec<String> {
        let mut v = self.sorts.clone();
        for s in self.stream.iter().chain(self.tail.iter()) {
            if !v.contains(&s.id) {
                v.push(s.id.clone());
            }
        }
        v
    }
}

/// One run in the game.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    /// Per sink port (`pass`, `output`, `output.t1`...): items of each kind that reached it.
    pub reached: BTreeMap<String, BTreeMap<String, u64>>,
    /// (gt, items delivered so far, every sink) at every change: the flow.
    pub flow: Vec<(u32, u64)>,
    pub supplied: u64,
    pub end: u32,
    pub timed_out: bool,
    /// Badge `overflow`: the gt the judge emptied the full chest (0: never).
    pub emptied_at: u32,
    pub restless: String,
    /// Design hoppers unlocked at rest (lag).
    pub awake: u32,
    /// The design's containers, per kind: before the judge fills anything, and at the end.
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    /// Entities alive at the end that the design did not declare.
    pub loose: Vec<String>,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

impl RunLog {
    fn delivered(&self) -> u64 {
        self.reached.values().flat_map(|m| m.values()).sum()
    }
}

/// The kinds and counts the design's own containers hold (judge chests left out).
fn held(w: &mut Worker, sb: u32, from: Pos, to: Pos, judge: &[Pos]) -> Result<BTreeMap<String, u64>> {
    let mut m = BTreeMap::new();
    for be in w.block_entities(sb, from, to)? {
        if judge.contains(&be.pos) {
            continue;
        }
        for s in stacks_of_be(&be.nbt) {
            *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
        }
    }
    Ok(m)
}

/// The kinds the design's containers hold as written in its file (the run's stream avoids them).
pub fn design_kinds(c: &Circuit) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for nbt in c.clock.design.block_entities.values() {
        for s in stacks_of_be(nbt) {
            if !v.contains(&s.id) {
                v.push(s.id.clone());
            }
        }
    }
    v
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

/// The sink chests: `pass` and every output, with the copy each output belongs to.
fn sinks(c: &Circuit) -> Vec<(String, Pos)> {
    let mut v: Vec<(String, Pos)> = c.stores.iter().filter(|s| s.role == "pass").map(|s| ("pass".to_string(), s.pos)).collect();
    for o in &c.outputs {
        v.push((if o.name == "out" { "output".to_string() } else { format!("output.{}", o.name) }, o.pos));
    }
    v
}

/// The copy an output port belongs to (`output` 0, `output.t<k>` k).
pub fn copy_of(port: &str) -> Option<usize> {
    if port == "pass" {
        return None;
    }
    Some(shulker::group_of(port) as usize)
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let (sources, sinks, judge) = ports(c);
    let chests: Vec<(Pos, String)> = judge.iter().map(|p| (*p, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    let base = w.get_region(sb, from, to)?;
    let mut log = RunLog { dims: shulker::design_dims(c), initial: held(w, sb, from, to, &judge)?, ..Default::default() };
    let mut b = GameBench { w, sb, from, to, base };
    drive(&mut b, sc, &sources, &sinks, &mut log)?;
    finish(w, sb, c, from, to, &judge, &mut log)?;
    Ok(Ok(log))
}

/// The judge's chests: the sources (`items`), the sinks (`pass` and every output, named) and all
/// of them.
fn ports(c: &Circuit) -> (Vec<Pos>, Vec<(String, Pos)>, Vec<Pos>) {
    let sources: Vec<Pos> = c.stores.iter().filter(|s| s.role == "items").map(|s| s.pos).collect();
    let sinks = sinks(c);
    let judge: Vec<Pos> = sources.iter().copied().chain(sinks.iter().map(|s| s.1)).collect();
    (sources, sinks, judge)
}

/// What a run needs from the world it runs in: the game's sandbox ([`GameBench`]) or a redsim
/// world ([`SimBench`]).
trait Bench {
    fn step(&mut self, n: u32) -> Result<()>;
    /// A judge chest's stacks.
    fn read(&mut self, p: Pos) -> Result<Vec<Stack>>;
    /// Replace a judge chest's contents (slot order).
    fn write(&mut self, p: Pos, items: &[Stack]) -> Result<()>;
    /// After the steps up to `t` (the game: the changed blocks for the table, early on).
    fn note(&mut self, _t: u32, _log: &mut RunLog) -> Result<()> {
        Ok(())
    }
}

struct GameBench<'a> {
    w: &'a mut Worker,
    sb: u32,
    from: Pos,
    to: Pos,
    base: mctas::sim::Region,
}

impl Bench for GameBench<'_> {
    fn step(&mut self, n: u32) -> Result<()> {
        self.w.step(self.sb, n).map(|_| ())
    }
    fn read(&mut self, p: Pos) -> Result<Vec<Stack>> {
        shulker::read_chest(self.w, self.sb, p)
    }
    fn write(&mut self, p: Pos, items: &[Stack]) -> Result<()> {
        self.w.merge_block_entity(self.sb, p, &items_snbt(items)).map(|_| ())
    }
    fn note(&mut self, t: u32, log: &mut RunLog) -> Result<()> {
        if t == SEG || t == 10 * SEG {
            log.notes.extend(shulker::snapshot(self.w, self.sb, self.from, self.to, &self.base, &format!("t={t}"))?);
        }
        Ok(())
    }
}

struct SimBench<'a> {
    w: &'a mut redsim::World,
}

fn bp(p: Pos) -> redsim::BlockPos {
    redsim::BlockPos::new(p[0], p[1], p[2])
}

impl Bench for SimBench<'_> {
    fn step(&mut self, n: u32) -> Result<()> {
        for _ in 0..n {
            self.w.tick();
        }
        if let Some(e) = self.w.errors.first() {
            bail!("redsim: {e}");
        }
        Ok(())
    }
    fn read(&mut self, p: Pos) -> Result<Vec<Stack>> {
        use redsim::redstone::container as rc;
        let c = self.w.containers.map.get(&bp(p)).ok_or_else(|| anyhow!("no container at {p:?}"))?;
        Ok(c.slots.iter().flatten().map(|s| Stack { id: format!("minecraft:{}", rc::item_name(s.item)), count: s.count as u32, contents: Vec::new(), name: (s.payload != 0).then(|| format!("payload {}", s.payload)) }).collect())
    }
    fn write(&mut self, p: Pos, items: &[Stack]) -> Result<()> {
        use redsim::redstone::container as rc;
        let mut v = Vec::new();
        for (i, s) in items.iter().enumerate() {
            let item = rc::item_id(&s.id).ok_or_else(|| anyhow!("redsim has no item {}", s.id))?;
            if s.name.is_some() || !s.contents.is_empty() {
                bail!("the redsim screen feeds plain stacks only ({} has components)", s.id);
            }
            v.push((i, rc::Stack { item, count: s.count as u8, payload: 0 }));
        }
        rc::set_contents(self.w, bp(p), &v);
        Ok(())
    }
}

/// The run's loop, on any bench: top the source up, count and empty the sinks every [`SEG`] gt;
/// badge `overflow`: the full chest first, emptied once the stream is supplied and the line quiet,
/// then the tail. Stops when every supplied item is accounted for, or at the cap.
fn drive<B: Bench>(b: &mut B, sc: &Scenario, sources: &[Pos], sinks: &[(String, Pos)], log: &mut RunLog) -> Result<()> {
    let full: Option<Pos> = sc.overflow.and_then(|k| sinks.iter().find(|(n, _)| copy_of(n) == Some(k)).map(|s| s.1));
    if let Some(p) = full {
        let filler: Vec<Stack> = (0..26).map(|_| Stack::new(FULL_FILLER, 1)).collect();
        b.write(p, &filler)?;
    }
    let mut cursor = 0usize;
    let mut tail_cursor = 0usize;
    let mut quiet = 0u32;
    let mut t = 0u32;
    topup(b, sources, &sc.stream, &mut cursor, log)?;
    loop {
        b.step(SEG)?;
        t += SEG;
        b.note(t, log)?;
        let before = log.delivered();
        for (name, p) in sinks {
            if Some(*p) == full && log.emptied_at == 0 {
                continue;
            }
            let have = b.read(*p)?;
            if have.is_empty() {
                continue;
            }
            let m = log.reached.entry(name.clone()).or_default();
            for s in &have {
                *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(*p, &[])?;
        }
        let now = log.delivered();
        if now != before {
            log.flow.push((t, now));
            quiet = 0;
        } else {
            quiet += SEG;
        }
        topup(b, sources, &sc.stream, &mut cursor, log)?;
        // badge `overflow`: the stream supplied, the source empty and the line quiet: the judge
        // empties the full chest (its filler left out) and supplies the tail
        let source_left = if full.is_some() && log.emptied_at == 0 && cursor >= sc.stream.len() {
            let mut n = 0usize;
            for p in sources {
                n += b.read(*p)?.len();
            }
            n
        } else {
            1
        };
        if let Some(p) = full
            && log.emptied_at == 0
            && source_left == 0
            && quiet >= QUIET
        {
            let have = b.read(p)?;
            let name = sinks.iter().find(|s| s.1 == p).map(|s| s.0.clone()).unwrap_or_default();
            let held: u32 = have.iter().filter(|s| s.id != FULL_FILLER).map(|s| s.count).sum();
            let filler: u32 = have.iter().filter(|s| s.id == FULL_FILLER).map(|s| s.count).sum();
            log.notes.push(format!("t={t}: the judge empties {name}, full since the start ({filler} x {} + {held} items)", clock::short(FULL_FILLER)));
            let m = log.reached.entry(name).or_default();
            for s in have.iter().filter(|s| s.id != FULL_FILLER) {
                *m.entry(s.id.clone()).or_insert(0) += s.count as u64;
            }
            b.write(p, &[])?;
            log.emptied_at = t;
            quiet = 0;
        }
        if log.emptied_at > 0 {
            topup(b, sources, &sc.tail, &mut tail_cursor, log)?;
        }
        let all_supplied = cursor >= sc.stream.len() && tail_cursor >= sc.tail.len() && (full.is_none() || log.emptied_at > 0);
        let done = all_supplied && log.delivered() >= sc.total();
        if done || t >= sc.cap {
            log.timed_out = !done;
            break;
        }
    }
    log.end = t;
    Ok(())
}

/// Top the source chests up from a stream (27 slots each).
fn topup<B: Bench>(b: &mut B, sources: &[Pos], stream: &[Stack], cursor: &mut usize, log: &mut RunLog) -> Result<()> {
    for p in sources {
        if *cursor >= stream.len() {
            return Ok(());
        }
        let mut have = b.read(*p)?;
        let before = have.len();
        while have.len() < 27 && *cursor < stream.len() {
            log.supplied += stream[*cursor].count as u64;
            have.push(stream[*cursor].clone());
            *cursor += 1;
        }
        if have.len() != before {
            b.write(*p, &have)?;
        }
    }
    Ok(())
}

/// The kinds and counts a redsim world's design containers hold (judge chests left out).
fn sim_held(w: &redsim::World, judge: &[Pos]) -> BTreeMap<String, u64> {
    use redsim::redstone::container as rc;
    let mut m = BTreeMap::new();
    for (p, c) in &w.containers.map {
        if judge.contains(&[p.x, p.y, p.z]) {
            continue;
        }
        for s in c.slots.iter().flatten() {
            *m.entry(format!("minecraft:{}", rc::item_name(s.item))).or_insert(0) += s.count as u64;
        }
    }
    m
}

/// The same run on redsim (milliseconds): `mctas circuit sim` and the searches' screen. Err: the
/// design cannot stand or redsim does not model it.
pub fn sim_run(c: &Circuit, sc: &Scenario, margin: i32) -> std::result::Result<RunLog, String> {
    let (from, to) = crate::run::watch_box(c, margin);
    if let Some((p, st)) = clock::sim::unsupported(&c.clock, from, to)?.first() {
        return Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(st)));
    }
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    let (sources, sinks, judge) = ports(c);
    let chest = redsim::Block::parse(CHEST)?;
    for p in &judge {
        w.set_block(bp(*p), chest, redsim::world::flags::ALL);
    }
    for _ in 0..c.clock.free_from() + SETTLE {
        w.tick();
    }
    let mut log = RunLog { dims: shulker::design_dims(c), initial: sim_held(&w, &judge), ..Default::default() };
    drive(&mut SimBench { w: &mut w }, sc, &sources, &sinks, &mut log).map_err(|e| format!("{e:#}"))?;
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
    if !still {
        log.restless = format!("the state still changes {} gt after the last item", 5 * REST);
    }
    log.awake = c.clock.design.blocks.keys().filter(|p| matches!(w.get(bp(**p)), redsim::Block::Hopper { enabled: true, .. })).count() as u32;
    log.final_held = sim_held(&w, &judge);
    log.loose = w.entities.list.iter().map(|e| format!("{:?}", e.kind)).collect();
    if let Some((p, b)) = w.drops.first() {
        log.loose.push(format!("{b} at {p} broke with a drop"));
    }
    Ok(log)
}

/// After the run: rest (up to 5 windows of [`REST`] gt in which nothing may change), the design's
/// containers, the entities left.
fn finish(w: &mut Worker, sb: u32, c: &Circuit, from: Pos, to: Pos, judge: &[Pos], log: &mut RunLog) -> Result<()> {
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
    if a.0 != b.0 {
        let p = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s))).unwrap_or_default();
        log.restless = format!("blocks still change {} gt after the last item ({p})", 5 * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.kind), x.pos)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after the last item ({p})", 5 * REST);
    } else if status.block_ticks + status.block_events + status.moving_pistons > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after the last item", status.block_ticks, status.block_events, status.moving_pistons, 5 * REST);
    }
    log.awake = b.0.iter().filter(|(p, s)| c.clock.design.blocks.contains_key(p) && s.starts_with("minecraft:hopper[") && s.contains("enabled=true")).count() as u32;
    log.final_held = held(w, sb, from, to, judge)?;
    for e in w.entities(sb)? {
        if e.design {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    Ok(())
}

/// Judge a run: the outcome, its metrics and a table (per sink, what reached it).
pub fn judge(sc: &Scenario, log: &RunLog) -> Outcome {
    let mut table: Vec<String> = Vec::new();
    for (port, m) in &log.reached {
        let list: Vec<String> = m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect();
        table.push(format!("{port}: {}", list.join(", ")));
    }
    table.push(format!("supplied {} items, delivered {}, end t={}", log.supplied, log.delivered(), log.end));
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    // every output: its copy's kind alone; `pass`: everything else (an overflowing copy's kind too)
    for (port, m) in &log.reached {
        match copy_of(port) {
            Some(k) => {
                let want = sc.sorts.get(k).cloned().unwrap_or_default();
                if let Some((wrong, n)) = m.iter().find(|(id, _)| **id != want) {
                    return fail(format!("{} x{n} reached copy {k}'s chest (it sorts {})", clock::short(wrong), clock::short(&want)), table);
                }
            }
            None => {
                for (k, kind) in sc.sorts.iter().enumerate() {
                    if let Some(n) = m.get(kind)
                        && sc.overflow != Some(k)
                    {
                        return fail(format!("{} x{n} went past copy {k} to the end of the line (copy {k} sorts it)", clock::short(kind)), table);
                    }
                }
            }
        }
    }
    if log.timed_out {
        return fail(format!("{} of {} items delivered after {} gt (the cap)", log.delivered(), sc.total(), log.end), table);
    }
    for kind in sc.kinds() {
        let got: u64 = log.reached.values().map(|m| m.get(&kind).copied().unwrap_or(0)).sum();
        let want = sc.supplied(&kind);
        if got != want {
            return fail(format!("{got} of {want} {} accounted for (lost or duplicated)", clock::short(&kind)), table);
        }
    }
    if log.final_held != log.initial {
        let diff: Vec<String> = log.initial.keys().chain(log.final_held.keys()).collect::<std::collections::BTreeSet<_>>().into_iter().filter(|k| log.initial.get(*k) != log.final_held.get(*k)).map(|k| format!("{} {} -> {}", clock::short(k), log.initial.get(k).copied().unwrap_or(0), log.final_held.get(k).copied().unwrap_or(0))).collect();
        return fail(format!("the design's containers changed: {} (a recipe must stay as it was)", diff.join(", ")), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...)", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    if !log.restless.is_empty() {
        return fail(log.restless.clone(), table);
    }
    let total = sc.total();
    let at = |q: u64| log.flow.iter().find(|(_, m)| *m >= q).copied();
    let rate = match (at(total.div_ceil(10)), at(total * 9 / 10)) {
        (Some((ta, qa)), Some((tb, qb))) if tb > ta => (qb - qa) as f64 * 20.0 / (tb - ta) as f64,
        _ => 0.0,
    };
    let first = log.flow.first().map(|x| x.0).unwrap_or(0);
    let metrics = vec![("items/s".to_string(), format!("{rate:.2}")), ("first item".to_string(), format!("by {first} gt")), ("hoppers awake".to_string(), log.awake.to_string())];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The tiled design of a sorter: `copies` copies at the `tile:` step chained along the line;
/// copy k's recipe holds `sorts[k]` instead of the design's kind. Ports: copy 0's `items`, the
/// last copy's `pass`, every copy's output (`output`, `output.t<k>`).
pub fn tiled(c: &Circuit, sorts: &[String]) -> Result<Circuit> {
    let t = c.tile.ok_or_else(|| anyhow!("{}: no `tile:` header", c.name()))?;
    let base = c.clock.design.clone();
    let n = sorts.len() as i32;
    let k0 = format!("\"{}\"", sorts[0]);
    let mut d = base.clone();
    d.meta.remove("tile");
    d.meta.insert("tiled".into(), n.to_string());
    d.ports.retain(|name, _| name == "items" || name == "output");
    for k in 1..n {
        let off = [t.step[0] * k, t.step[1] * k, t.step[2] * k];
        let sh = |p: &Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        for (p, s) in &base.blocks {
            let q = sh(p);
            if let Some(o) = d.blocks.get(&q) {
                bail!("tile: copy {k} puts {} on {} at {q:?} (the copies overlap)", clock::short(s), clock::short(o));
            }
            d.blocks.insert(q, s.clone());
        }
        let kk = format!("\"{}\"", sorts[k as usize]);
        for (p, s) in &base.block_entities {
            d.block_entities.insert(sh(p), s.replace(&k0, &kk));
        }
        for e in &base.entities {
            let mut e2 = e.clone();
            for i in 0..3 {
                e2.pos[i] += off[i] as f64;
            }
            d.entities.push(e2);
        }
        if let Some(p) = base.ports.get("output") {
            d.ports.insert(format!("output.t{k}"), sh(p));
        }
        if k == n - 1
            && let Some(p) = base.ports.get("pass")
        {
            d.ports.insert("pass".into(), sh(p));
        }
    }
    for (name, p) in &d.ports {
        if d.blocks.contains_key(p) {
            bail!("tile: port {name} at {p:?} lands on a copy's block");
        }
    }
    Circuit::from_design(d).context("tiled sorter")
}

/// The judge for the sorter kind (`mctas circuit verify`): the base run at N positions x 4
/// rotations, then (a `tile:` header) the badges `tileable` and `overflow`, each at 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let Kind::Sorter(item) = &kind else { bail!("{}: not a sorter", kind.name()) };
    let avoid = design_kinds(c);
    let sc = Scenario::base(item, &avoid)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, &sc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&sc, &log), dims: log.dims, family: family.clone() }))
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
    let mut won: Vec<String> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    if c.tile.is_some() {
        for (badge, overflow) in [("tileable", false), ("overflow", true)] {
            let r = badge_run(pool, c, item, &avoid, overflow, o);
            match r {
                Ok(()) => {
                    won.push(badge.to_string());
                    lines.push(format!("badge {badge}: yes"));
                }
                Err(why) => lines.push(format!("badge {badge}: no ({why})")),
            }
        }
    }
    let bill = shulker::bill(c);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        k.outcome.metrics.push(("badges".to_string(), if won.is_empty() { "none".to_string() } else { won.join("+") }));
        k.outcome.metrics.push(("bill".to_string(), bill));
        k.outcome.table.extend(lines);
    }
    Ok(rep)
}

/// One badge's tiled run at 4 rotations (the first position): Err(why) at the first failure.
fn badge_run(pool: &Pool, c: &Circuit, item: &str, avoid: &[String], overflow: bool, o: &Options) -> std::result::Result<(), String> {
    let sc = Scenario::tiled(item, avoid, overflow).map_err(|e| format!("{e:#}"))?;
    let tc = tiled(c, &sc.sorts).map_err(|e| format!("{e:#}"))?;
    let jobs: Vec<u8> = vec![0, 1, 2, 3];
    let results = pool.map(jobs.clone(), |w, _, t| {
        let rc = tc.rotated(w, t)?;
        game_run(w, &rc, &sc, o.margin)
    });
    for (t, r) in jobs.into_iter().zip(results) {
        match r {
            Ok(Ok(log)) => {
                let out = judge(&sc, &log);
                if !out.pass {
                    return Err(format!("rotation {t}: {}", out.why));
                }
            }
            Ok(Err(why)) => return Err(format!("rotation {t}: {why}")),
            Err(e) => return Err(format!("rotation {t}: {e:#}")),
        }
    }
    Ok(())
}

/// A design aid (`mctas circuit sorter-run`): one run of the base or a badge scenario at rotation
/// 0, its table printed.
pub fn run_one(pool: &Pool, c: &Circuit, badge: Option<&str>, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, badge)?;
    let r = pool.map(vec![0u8], |w, _, _| game_run(w, &rc, &sc, margin));
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        Ok(log) => Ok(judge(&sc, &log)),
        Err(why) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// [`run_one`] on redsim (`mctas circuit sorter-run --sim`).
pub fn sim_one(c: &Circuit, badge: Option<&str>, margin: i32) -> Result<Outcome> {
    let (sc, rc) = scenario_for(c, badge)?;
    Ok(match sim_run(&rc, &sc, margin) {
        Ok(log) => judge(&sc, &log),
        Err(why) => Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() },
    })
}

/// The scenario and the circuit (tiled for a badge) of one run.
fn scenario_for(c: &Circuit, badge: Option<&str>) -> Result<(Scenario, Circuit)> {
    let Some(Kind::Sorter(item)) = &c.kind else { bail!("{}: not a sorter", c.name()) };
    let avoid = design_kinds(c);
    Ok(match badge {
        None => (Scenario::base(item, &avoid)?, c.clone()),
        Some(b) => {
            let sc = Scenario::tiled(item, &avoid, b == "overflow")?;
            let tc = tiled(c, &sc.sorts)?;
            (sc, tc)
        }
    })
}

impl Scenario {
    /// The census's quick screen: a short stream (the kind, 64-stack, 16-stack and non-stackable
    /// junk, 14 items).
    pub fn quick(kind: &str, avoid: &[String]) -> Result<Scenario> {
        let full = Scenario::base(kind, avoid)?;
        let k = full.sorts[0].clone();
        let junk: Vec<String> = full.kinds().into_iter().filter(|x| *x != k).collect();
        let by = |n: u32| junk.iter().find(|j| max_stack(j) == n).cloned().unwrap_or_default();
        let (j, s, u) = (by(64), by(16), by(1));
        let st = |id: &str, n: u32| Stack::new(id, n.min(max_stack(id)));
        let stream = vec![st(&k, 3), st(&j, 3), st(&k, 1), st(&s, 2), st(&k, 2), st(&u, 1), st(&j, 2)];
        Ok(Scenario::with_cap(vec![k], stream, None, Vec::new()))
    }
}

/// `mctas circuit sorter-census` (a local search on redsim): designs `remove` or fewer of the base
/// design's blocks (hoppers and comparators stay) lighter, with up to `remove - 1` palette blocks
/// added in the air cells of `cells` (ports left out), so every candidate is smaller than the
/// base. A candidate passes the quick screen, then the base run, then the tiled and overflow runs
/// (all on redsim). Returns the passing designs (block count, the design), smallest first.
pub fn census(base: &Circuit, remove: usize, cells: (Pos, Pos), palette: &[String], threads: usize) -> Result<Vec<(usize, mctas::design::Design)>> {
    let Some(Kind::Sorter(item)) = &base.kind else { bail!("{}: not a sorter", base.name()) };
    let avoid = design_kinds(base);
    let quick = Scenario::quick(item, &avoid)?;
    let full = Scenario::base(item, &avoid)?;
    let tiled_sc = Scenario::tiled(item, &avoid, false)?;
    let over_sc = Scenario::tiled(item, &avoid, true)?;
    let keep = vec!["minecraft:hopper".to_string(), "minecraft:comparator".to_string()];
    let ok = |sc: &Scenario, cc: &Circuit| sim_run(cc, sc, 3).is_ok_and(|log| judge(sc, &log).pass);
    let pass = |cand: &Circuit| ok(&quick, cand) && ok(&full, cand) && cand.tile.is_some() && tiled(cand, &tiled_sc.sorts).is_ok_and(|t| ok(&tiled_sc, &t)) && tiled(cand, &over_sc.sorts).is_ok_and(|t| ok(&over_sc, &t));
    Ok(crate::local::census(base, remove, cells, palette, &keep, threads, &pass))
}
