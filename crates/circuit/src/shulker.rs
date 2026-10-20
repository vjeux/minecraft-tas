//! Study 8, shulker loaders and unloaders (docs/circuits.md "Shulker loaders and unloaders",
//! designs/circuits/shulker/VARIATIONS.md): kinds `shulker-loader [64|16|1]` and
//! `shulker-unloader [64|16|1]`, the number being the scenario item's max stack (cobblestone,
//! snowballs, iron shovels).
//!
//! Not judged by the waveform trace: the judge puts a single chest on each of the design's
//! `items`, `boxes` and `output` ports (air cells, not counted), fills them at the start, keeps an
//! item source topped up and empties the outputs between ticks, records every box that reaches an
//! output (the gt, its colour, its name, its contents) from per-gt block-entity frames, then waits
//! for rest and takes a full inventory of the watched box (every container, every placed box,
//! every entity).
//!
//! A loader gets 3 boxes' worth of items and 4 empty boxes: 3 FULL boxes must arrive, every item
//! inside them, nothing left in the machine, no item entity at the end, then rest. An unloader gets
//! 3 full boxes: every item must reach `items`, 3 EMPTY boxes must reach `output`. Metrics: boxes
//! per minute and items per second over the steady part (first box to last box), the first box's
//! latency and the cycle in gt.

use crate::check::Outcome;
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, InputHow, Kind, Pos};
use anyhow::{Context, Result, anyhow};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use redsim::redstone::container::{Snbt, parse_snbt};

/// The judge's chest.
pub const CHEST: &str = "minecraft:chest[facing=north,type=single,waterlogged=false]";
/// Boxes per run (the brief: "keeps running for >= 3 boxes").
pub const BOXES: u32 = 3;
/// Ticks between two judge visits of its chests (deliveries are timed to the gt from per-gt
/// block-entity frames of the output chests).
pub const SEG: u32 = 20;
/// After the last box the design must stand still this long.
pub const REST: u32 = 400;
/// The plain shulker box.
pub const BOX: &str = "minecraft:shulker_box";

/// One item stack as the judge sees it: a shulker box item carries its contents and name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub id: String,
    pub count: u32,
    /// A shulker box's contents (`minecraft:container`), slot order.
    pub contents: Vec<Stack>,
    /// `minecraft:custom_name`, as SNBT text.
    pub name: Option<String>,
}

impl Stack {
    pub fn new(id: &str, count: u32) -> Stack {
        Stack { id: id.to_string(), count, contents: Vec::new(), name: None }
    }

    pub fn is_box(&self) -> bool {
        is_box(&self.id)
    }

    /// How many of `item` this stack holds, inside a box included.
    pub fn count_of(&self, item: &str) -> u64 {
        let own = if self.id == item { self.count as u64 } else { 0 };
        own + self.contents.iter().map(|s| s.count_of(item)).sum::<u64>()
    }

    /// How many items a box holds (any kind).
    pub fn count_of_any(&self) -> u64 {
        self.contents.iter().map(|s| s.count as u64).sum()
    }

    /// The SNBT of this stack as a container slot entry (`{Slot:0b,id:...,count:...}`).
    fn snbt(&self, slot: usize, chest: bool) -> String {
        let mut parts = Vec::new();
        if chest {
            parts.push(format!("Slot:{slot}b"));
        }
        parts.push(format!("id:\"{}\"", self.id));
        parts.push(format!("count:{}", self.count));
        let mut comps = Vec::new();
        if !self.contents.is_empty() {
            let inner: Vec<String> = self.contents.iter().enumerate().map(|(i, s)| format!("{{slot:{i},item:{}}}", s.snbt(0, false))).collect();
            comps.push(format!("\"minecraft:container\":[{}]", inner.join(",")));
        }
        if let Some(n) = &self.name {
            comps.push(format!("\"minecraft:custom_name\":{n}"));
        }
        if !comps.is_empty() {
            parts.push(format!("components:{{{}}}", comps.join(",")));
        }
        format!("{{{}}}", parts.join(","))
    }
}

/// A shulker box item or block id (any colour).
pub fn is_box(id: &str) -> bool {
    id.ends_with("shulker_box")
}

/// The scenario item for a max stack: (id, max stack).
pub fn scenario_item(stack: u32) -> (&'static str, u32) {
    match stack {
        16 => ("minecraft:snowball", 16),
        1 => ("minecraft:iron_shovel", 1),
        _ => ("minecraft:cobblestone", 64),
    }
}

/// The `Items` of a container chest written as SNBT for `merge_block_entity`.
fn items_snbt(stacks: &[Stack]) -> String {
    let v: Vec<String> = stacks.iter().enumerate().map(|(i, s)| s.snbt(i, true)).collect();
    format!("{{Items:[{}]}}", v.join(","))
}

fn field<'a>(c: &'a Snbt, key: &str) -> Option<&'a Snbt> {
    match c {
        Snbt::Compound(v) => v.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn int_of(s: &Snbt) -> Option<i64> {
    match s {
        Snbt::Int(i) => Some(*i),
        Snbt::Word(w) => w.trim_end_matches(['b', 's', 'L', 'l', 'B', 'S']).parse().ok(),
        _ => None,
    }
}

/// SNBT text of a value (for names).
fn render(s: &Snbt) -> String {
    match s {
        Snbt::Compound(v) => format!("{{{}}}", v.iter().map(|(k, v)| format!("{k}:{}", render(v))).collect::<Vec<_>>().join(",")),
        Snbt::List(v) => format!("[{}]", v.iter().map(render).collect::<Vec<_>>().join(",")),
        Snbt::Str(t) => format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"")),
        Snbt::Int(i) => i.to_string(),
        Snbt::Word(w) => w.clone(),
    }
}

/// An item stack from its SNBT compound (`{count:..,id:..,components:{..}}`).
fn stack_of(c: &Snbt) -> Option<Stack> {
    let id = match field(c, "id")? {
        Snbt::Str(s) | Snbt::Word(s) => s.clone(),
        _ => return None,
    };
    let count = field(c, "count").and_then(int_of).unwrap_or(1).max(0) as u32;
    let mut st = Stack { id, count, contents: Vec::new(), name: None };
    if let Some(comps) = field(c, "components") {
        if let Some(Snbt::List(entries)) = field(comps, "minecraft:container") {
            let mut slots: Vec<(i64, Stack)> = entries.iter().filter_map(|e| Some((field(e, "slot").and_then(int_of).unwrap_or(0), stack_of(field(e, "item")?)?))).collect();
            slots.sort_by_key(|(s, _)| *s);
            st.contents = slots.into_iter().map(|(_, s)| s).collect();
        }
        if let Some(n) = field(comps, "minecraft:custom_name") {
            st.name = Some(render(n));
        }
    }
    Some(st)
}

/// The stacks of a container block entity's SNBT (`Items`), slot order.
pub fn stacks_of_be(nbt: &str) -> Vec<Stack> {
    stacks_of_be_checked(nbt).unwrap_or_default()
}

/// [`stacks_of_be`], or why the SNBT does not parse.
pub fn stacks_of_be_checked(nbt: &str) -> std::result::Result<Vec<Stack>, String> {
    let v = parse_snbt(nbt)?;
    let Some(Snbt::List(items)) = field(&v, "Items") else { return Ok(Vec::new()) };
    let mut slots: Vec<(i64, Stack)> = Vec::new();
    for e in items {
        let st = stack_of(e).ok_or_else(|| format!("an item without an id: {e:?}"))?;
        slots.push((field(e, "Slot").and_then(int_of).unwrap_or(0), st));
    }
    slots.sort_by_key(|(s, _)| *s);
    Ok(slots.into_iter().map(|(_, s)| s).collect())
}

/// The stack an item entity carries (its `state` from the bridge: the saved data with `Item:{..}`).
fn stack_of_entity(state: &str) -> Option<Stack> {
    let i = state.find("Item:")?;
    let v = parse_snbt(&state[i + 5..]).ok().or_else(|| {
        // the state may go on after the compound: cut at its matching brace
        let s = &state[i + 5..];
        let mut depth = 0i32;
        for (k, ch) in s.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return parse_snbt(&s[..=k]).ok();
                    }
                }
                _ => {}
            }
        }
        None
    })?;
    stack_of(&v)
}

/// A judge scenario.
#[derive(Clone, Debug)]
pub struct Scenario {
    pub loader: bool,
    pub item: &'static str,
    pub stack: u32,
    /// Boxes' worth of items (loader) or full boxes supplied (unloader).
    pub boxes: u32,
    /// Extra empty boxes in a loader's supply.
    pub spare: u32,
    /// Ticks after the start before giving up.
    pub cap: u32,
}

impl Scenario {
    /// The BASE scenario of a kind.
    pub fn base(kind: &Kind) -> Option<Scenario> {
        let (loader, stack) = match kind {
            Kind::ShulkerLoader(s) => (true, *s),
            Kind::ShulkerUnloader(s) => (false, *s),
            _ => return None,
        };
        let (item, stack) = scenario_item(stack);
        let per_box = 27 * stack;
        // one hopper's pace (8 gt per item) plus a generous box swap, x 1.25, + 2000
        let cap = BOXES * (per_box * 8 + 400) * 5 / 4 + 2000;
        Some(Scenario { loader, item, stack, boxes: BOXES, spare: 1, cap })
    }

    pub fn per_box(&self) -> u64 {
        27 * self.stack as u64
    }

    pub fn total(&self) -> u64 {
        self.boxes as u64 * self.per_box()
    }

    /// A full box of the scenario item.
    fn full_box(&self) -> Stack {
        Stack { id: BOX.to_string(), count: 1, contents: (0..27).map(|_| Stack::new(self.item, self.stack)).collect(), name: None }
    }
}

/// A box (or anything else) that reached an output chest.
#[derive(Clone, Debug)]
pub struct Delivery {
    /// Ticks after the start.
    pub t: u32,
    pub port: String,
    pub stack: Stack,
}

/// One run of a scenario in the game.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    pub deliveries: Vec<Delivery>,
    /// Unloader: items drained from the `items` chests, (tick after start, cumulative count).
    pub drained: Vec<(u32, u64)>,
    /// Non-scenario items drained from the sinks (ids).
    pub foreign: Vec<String>,
    /// Items put into the sources.
    pub supplied: u64,
    /// Ticks after the start when the run stopped (done or the cap).
    pub end: u32,
    pub timed_out: bool,
    /// Why the design was not at rest after the last box (empty: at rest).
    pub restless: String,
    /// The scenario item left anywhere in the watched box at the end (containers, placed boxes,
    /// entities), and the boxes left there.
    pub left_items: u64,
    pub left_boxes: u64,
    /// Entities alive at the end that the design did not declare.
    pub loose: Vec<String>,
    /// The design's block box (judge chests and input cells left out).
    pub dims: [i32; 3],
    /// Snapshots for the table: what differs from the placed design (cells and container
    /// contents), early in the run and at the end.
    pub notes: Vec<String>,
}

/// One snapshot line set: every cell whose state differs from `base`, every non-empty container.
fn snapshot(w: &mut Worker, sb: u32, from: Pos, to: Pos, base: &mctas::sim::Region, label: &str) -> Result<Vec<String>> {
    let r = w.get_region(sb, from, to)?;
    let mut out = vec![format!("-- {label}")];
    for (p, s) in r.iter() {
        if base.get(p) != Some(s) {
            out.push(format!("   {:?} {} (was {})", p, clock::short(s), base.get(p).map(clock::short).unwrap_or_default()));
        }
    }
    for be in w.block_entities(sb, from, to)? {
        let st = stacks_of_be(&be.nbt);
        if !st.is_empty() {
            let list: Vec<String> = st.iter().map(|s| if s.is_box() { format!("{}[{}]", clock::short(&s.id), s.count_of_any()) } else { format!("{}x{}", clock::short(&s.id), s.count) }).collect();
            out.push(format!("   {:?} {} holds {}", be.pos, clock::short(&be.kind), list.join(" ")));
        }
    }
    for e in w.entities(sb)? {
        out.push(format!("   entity {} at {:.2?}", clock::short(&e.kind), e.pos));
    }
    Ok(out)
}

/// Run a scenario on a circuit already turned and moved, in a fresh sandbox: `Ok(Err(why))` when
/// a rule of the run itself breaks (a block without support, something escaping the watched box).
pub fn game_run(w: &mut Worker, c: &Circuit, sc: &Scenario, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = run_in(w, sb, c, sc, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

fn design_dims(c: &Circuit) -> [i32; 3] {
    let ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for p in c.clock.design.blocks.keys().filter(|p| !ins.contains(p)) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    if lo[0] == i32::MAX { [0, 0, 0] } else { [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1] }
}

fn read_chest(w: &mut Worker, sb: u32, p: Pos) -> Result<Vec<Stack>> {
    match w.block_entities(sb, p, p)?.into_iter().find(|b| b.pos == p) {
        Some(b) => stacks_of_be_checked(&b.nbt).map_err(|e| anyhow!("judge: the chest at {p:?} does not parse ({e}): {}", b.nbt)),
        None => Ok(Vec::new()),
    }
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().filter(|(p, _)| Some(**p) != c.clock.withheld()).find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    // the judge's chests (empty for now)
    let sources: Vec<Pos> = c.stores.iter().filter(|s| s.role == "items").map(|s| s.pos).collect();
    let supplies: Vec<Pos> = c.stores.iter().filter(|s| s.role == "boxes").map(|s| s.pos).collect();
    let outs: Vec<(String, Pos)> = c.outputs.iter().map(|o| (o.name.clone(), o.pos)).collect();
    let chests: Vec<(Pos, String)> = c.stores.iter().map(|s| s.pos).chain(outs.iter().map(|o| o.1)).map(|p| (p, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    let start = c.clock.free_from() + SETTLE;
    w.step(sb, start)?;
    let base = w.get_region(sb, from, to)?;
    let mut log = RunLog { dims: design_dims(c), ..Default::default() };
    // the start: the box supply, the items (a loader's source, topped up later), the lever
    let per_chest = 27usize;
    let supply: Vec<Stack> = if sc.loader { (0..sc.boxes + sc.spare).map(|_| Stack::new(BOX, 1)).collect() } else { (0..sc.boxes).map(|_| sc.full_box()).collect() };
    for (k, chunk) in supply.chunks(per_chest).enumerate() {
        let p = *supplies.get(k).ok_or_else(|| anyhow!("{} boxes need {} `boxes` chests", supply.len(), supply.len().div_ceil(per_chest)))?;
        w.merge_block_entity(sb, p, &items_snbt(chunk))?;
    }
    let topup = |w: &mut Worker, log: &mut RunLog| -> Result<()> {
        if !sc.loader {
            return Ok(());
        }
        for p in &sources {
            if log.supplied >= sc.total() {
                break;
            }
            let mut have = read_chest(w, sb, *p)?;
            let before = have.len();
            while have.len() < per_chest && log.supplied < sc.total() {
                let n = (sc.total() - log.supplied).min(sc.stack as u64) as u32;
                have.push(Stack::new(sc.item, n));
                log.supplied += n as u64;
            }
            if have.len() != before {
                w.merge_block_entity(sb, *p, &items_snbt(&have))?;
            }
        }
        Ok(())
    };
    topup(w, &mut log)?;
    for i in &c.inputs {
        if i.how == InputHow::Lever {
            w.use_block(sb, i.pos)?;
        }
    }
    // a design aid: MCTAS_SHULKER_DEBUG=N snapshots each of the first N gt (the table shows them)
    let debug: u32 = std::env::var("MCTAS_SHULKER_DEBUG").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut t = 0u32;
    while t < debug {
        let before = log.deliveries.len();
        w.step(sb, 1)?;
        t += 1;
        log.notes.extend(snapshot(w, sb, from, to, &base, &format!("t={t}"))?);
        debug_assert_eq!(before, log.deliveries.len());
    }
    // the record box: every output chest (the per-gt frames time each delivery)
    let watch: Vec<Pos> = outs.iter().map(|o| o.1).collect();
    let mut rlo = watch[0];
    let mut rhi = watch[0];
    for p in &watch {
        for i in 0..3 {
            rlo[i] = rlo[i].min(p[i]);
            rhi[i] = rhi[i].max(p[i]);
        }
    }
    let sinks: Vec<Pos> = if sc.loader { Vec::new() } else { sources.clone() };
    let mut drained = 0u64;
    // a design aid: MCTAS_SHULKER_WATCH=a-b steps 1 gt at a time and snapshots ticks a..=b
    let window: Option<(u32, u32)> = std::env::var("MCTAS_SHULKER_WATCH").ok().and_then(|v| {
        let (a, b) = v.split_once('-')?;
        Some((a.parse().ok()?, b.parse().ok()?))
    });
    let seg = if window.is_some() { 1 } else { SEG };
    loop {
        let st = w.step_probe_be(sb, seg, false, &[], Some((rlo, rhi)), true)?;
        // per output chest: the stacks seen so far this segment (the judge empties it after)
        let mut seen: Vec<usize> = vec![0; outs.len()];
        let mut last = String::new();
        for (k, dump) in st.block_entities.iter().enumerate() {
            if *dump == last {
                continue;
            }
            last = dump.clone();
            let bes = mctas::sim::parse_block_entities(dump)?;
            for (oi, (name, p)) in outs.iter().enumerate() {
                let stacks = match bes.iter().find(|b| b.pos == *p) {
                    Some(b) => stacks_of_be_checked(&b.nbt).map_err(|e| anyhow!("judge: the output chest at {p:?} does not parse ({e}): {}", b.nbt))?,
                    None => Vec::new(),
                };
                // one entry per item that arrived: boxes stack to 1; other items are counted once
                let n: usize = stacks.len();
                if n > seen[oi] {
                    for s in &stacks[seen[oi]..] {
                        log.deliveries.push(Delivery { t: t + k as u32 + 1, port: name.clone(), stack: s.clone() });
                    }
                    seen[oi] = n;
                }
            }
        }
        t += seg;
        if t == SEG || t == 10 * SEG || window.is_some_and(|(a, b)| a <= t && t <= b) {
            log.notes.extend(snapshot(w, sb, from, to, &base, &format!("t={t}"))?);
        }
        // the judge's visit: empty the outputs (anything the frames did not show arrived on the
        // segment's last gt) and the sinks, top up the sources
        for (oi, (name, p)) in outs.iter().enumerate() {
            let have = read_chest(w, sb, *p)?;
            if have.len() > seen[oi] {
                for s in &have[seen[oi]..] {
                    log.deliveries.push(Delivery { t, port: name.clone(), stack: s.clone() });
                }
            }
            if !have.is_empty() {
                w.merge_block_entity(sb, *p, "{Items:[]}")?;
            }
        }
        for p in &sinks {
            let have = read_chest(w, sb, *p)?;
            if !have.is_empty() {
                for s in &have {
                    if s.id == sc.item && s.contents.is_empty() {
                        drained += s.count as u64;
                    } else {
                        log.foreign.push(s.id.clone());
                    }
                }
                log.drained.push((t, drained));
                w.merge_block_entity(sb, *p, "{Items:[]}")?;
            }
        }
        topup(w, &mut log)?;
        let boxes_out = log.deliveries.iter().filter(|d| d.stack.is_box()).count() as u32;
        let done = if sc.loader { boxes_out >= sc.boxes && log.supplied >= sc.total() } else { boxes_out >= sc.boxes && drained >= sc.total() };
        if done || t >= sc.cap {
            log.timed_out = !done;
            break;
        }
    }
    log.end = t;
    log.notes.extend(snapshot(w, sb, from, to, &base, &format!("end t={t}"))?);
    // rest: the last moves may still run (the next box placed...): up to 5 windows of REST gt, the
    // design must reach a window in which nothing changes
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
        log.restless = format!("blocks still change {} gt after the last box ({p})", 5 * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.kind), x.pos)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after the last box ({p})", 5 * REST);
    } else if status.block_ticks + status.block_events + status.moving_pistons > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after the last box", status.block_ticks, status.block_events, status.moving_pistons, 5 * REST);
    }
    // nothing may reach the outer layer of the watched box
    if let Some((p, s)) = b.0.iter().find(|(p, s)| !mctas::blockstate::is_air(s) && (0..3).any(|i| p[i] == from[i] || p[i] == to[i])) {
        return Ok(Err(format!("{} at {p:?} reached the edge of the watched box", clock::short(s))));
    }
    // the inventory: every container and placed box, every entity
    for be in &b.1 {
        for s in stacks_of_be(&be.nbt) {
            log.left_items += s.count_of(sc.item);
            if s.is_box() {
                log.left_boxes += s.count as u64;
            }
        }
        if is_box(&be.kind) {
            log.left_boxes += 1;
        }
    }
    for e in w.entities(sb)? {
        if e.design {
            continue;
        }
        if let Some(s) = stack_of_entity(&e.state) {
            log.left_items += s.count_of(sc.item);
            if s.is_box() {
                log.left_boxes += s.count as u64;
            }
            log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos));
        } else {
            log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos));
        }
    }
    Ok(Ok(log))
}

/// Judge a run: the outcome with its metrics and a table (one line per delivery).
pub fn judge(sc: &Scenario, log: &RunLog) -> Outcome {
    let mut table: Vec<String> = log.deliveries.iter().map(|d| format!("t={:>6} {:<10} {}", d.t, d.port, describe(&d.stack, sc))).collect();
    if !sc.loader && !log.drained.is_empty() {
        table.push(format!("items drained: {} of {}", log.drained.last().map(|x| x.1).unwrap_or(0), sc.total()));
    }
    table.extend(log.notes.iter().cloned());
    let fail = |why: String, table: Vec<String>| Outcome { pass: false, why, metrics: Vec::new(), table };
    let boxes: Vec<&Delivery> = log.deliveries.iter().filter(|d| d.stack.is_box()).collect();
    if let Some(d) = log.deliveries.iter().find(|d| !d.stack.is_box()) {
        return fail(format!("{} reached output {} at t={} (only boxes may)", describe(&d.stack, sc), d.port, d.t), table);
    }
    // a wrong box is wrong whatever happens after it
    for (k, d) in boxes.iter().enumerate() {
        let n = d.stack.count_of(sc.item);
        let others: Vec<&Stack> = d.stack.contents.iter().filter(|s| s.id != sc.item).collect();
        if !others.is_empty() {
            return fail(format!("box {} holds {} (not the scenario item)", k + 1, others[0].id), table);
        }
        if sc.loader {
            let full = d.stack.contents.len() == 27 && d.stack.contents.iter().all(|s| s.count == sc.stack);
            if !full {
                return fail(format!("box {} arrived at t={} with {n} of {} items (not full)", k + 1, d.t, sc.per_box()), table);
            }
        } else if n > 0 {
            return fail(format!("box {} arrived at t={} with {n} items left in it", k + 1, d.t), table);
        }
    }
    if log.timed_out {
        return fail(format!("{} of {} boxes after {} gt (the cap)", boxes.len(), sc.boxes, log.end), table);
    }
    if boxes.len() as u32 != sc.boxes {
        return fail(format!("{} boxes delivered, {} expected", boxes.len(), sc.boxes), table);
    }
    if !log.foreign.is_empty() {
        return fail(format!("{} reached the items chest", log.foreign[0]), table);
    }
    let accounted = if sc.loader { boxes.iter().map(|d| d.stack.count_of(sc.item)).sum::<u64>() } else { log.drained.last().map(|x| x.1).unwrap_or(0) };
    if log.left_items > 0 {
        return fail(format!("{} items left in the machine at the end", log.left_items), table);
    }
    if accounted != sc.total() {
        return fail(format!("{} of {} items accounted for (lost or duplicated)", accounted, sc.total()), table);
    }
    let boxes_in = if sc.loader { sc.boxes + sc.spare } else { sc.boxes } as u64;
    if boxes.len() as u64 + log.left_boxes != boxes_in {
        return fail(format!("{} boxes supplied, {} delivered + {} left in the machine (lost or duplicated)", boxes_in, boxes.len(), log.left_boxes), table);
    }
    if let Some(e) = log.loose.first() {
        return fail(format!("{} entit{} alive at the end ({e}...): a drop left behind", log.loose.len(), if log.loose.len() == 1 { "y" } else { "ies" }), table);
    }
    if !log.restless.is_empty() {
        return fail(log.restless.clone(), table);
    }
    let t1 = boxes[0].t;
    let tn = boxes[boxes.len() - 1].t;
    let span = (tn - t1).max(1) as f64;
    let n = (boxes.len() - 1).max(1) as f64;
    let cycle = span / n;
    let metrics = vec![
        ("boxes/min".to_string(), format!("{:.3}", 1200.0 / cycle)),
        ("items/s".to_string(), format!("{:.2}", sc.per_box() as f64 * 20.0 / cycle)),
        ("first box".to_string(), format!("{t1} gt")),
        ("cycle".to_string(), format!("{} gt", cycle.round() as i64)),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `shulker_box (27 x 64 cobblestone, full)`.
pub fn describe(s: &Stack, sc: &Scenario) -> String {
    if !s.is_box() {
        return format!("{} x{}", clock::short(&s.id), s.count);
    }
    let n = s.count_of(sc.item);
    let name = s.name.as_ref().map(|n| format!(" named {n}")).unwrap_or_default();
    let full = s.contents.len() == 27 && s.contents.iter().all(|x| x.id == sc.item && x.count == sc.stack);
    format!("{}{name} ({} stacks, {n} {}{})", clock::short(&s.id), s.contents.len(), clock::short(sc.item), if full { ", full" } else if n == 0 { ", empty" } else { "" })
}

/// The judge for the shulker kinds (`mctas circuit verify`): the BASE scenario at N positions x 4
/// rotations, the builds compared as for every kind.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let sc = Scenario::base(&kind).ok_or_else(|| anyhow!("{}: not a shulker kind", kind.name()))?;
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
        let result = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        builds.push(Build { turns: t, offset: off, result });
    }
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = sc.cap;
    let rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    Ok(tolerate_jitter(rep))
}

/// The drop pops at a random offset with a random drift, which can move the gt its hopper picks it
/// up by one or two: builds whose times differ from the base build's by at most [`JITTER`] gt (and
/// all pass) are the same circuit, `robust`.
pub const JITTER: i64 = 4;

fn tolerate_jitter(mut rep: VerifyReport) -> VerifyReport {
    if !rep.pass {
        return rep;
    }
    let times = |o: &Outcome| -> Option<(i64, i64)> {
        let gt = |k: &str| o.metrics.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.trim_end_matches(" gt").parse::<i64>().ok());
        Some((gt("first box")?, gt("cycle")?))
    };
    let Some(base) = rep.base().and_then(|b| times(&b.outcome)) else { return rep };
    let all: Vec<(i64, i64)> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok().filter(|k| k.outcome.pass).and_then(|k| times(&k.outcome))).collect();
    let close = all.len() == rep.builds.len() && all.iter().all(|(f, c)| (f - base.0).abs() <= JITTER && (c - base.1).abs() <= JITTER);
    if !close {
        return rep;
    }
    rep.verdict = "robust".into();
    rep.note.clear();
    // report the builds' medians (the base build's own times carry the random gt)
    let med = |mut v: Vec<i64>| -> i64 {
        v.sort();
        v[v.len() / 2]
    };
    let first = med(all.iter().map(|x| x.0).collect());
    let cycle = med(all.iter().map(|x| x.1).collect());
    let per_box: Option<f64> = rep.base().and_then(|b| {
        let ips: f64 = b.outcome.metrics.iter().find(|(n, _)| n == "items/s")?.1.parse().ok()?;
        let cyc = times(&b.outcome)?.1 as f64;
        Some((ips * cyc / 20.0).round())
    });
    if let (Some(per_box), Some(b)) = (per_box, rep.builds.iter_mut().find(|b| b.result.as_ref().is_ok_and(|k| k.outcome.pass) && b.turns == 0 && b.offset == [0, 0, 0])) {
        if let Ok(k) = &mut b.result {
            k.outcome.metrics = vec![
                ("boxes/min".to_string(), format!("{:.3}", 1200.0 / cycle as f64)),
                ("items/s".to_string(), format!("{:.2}", per_box * 20.0 / cycle as f64)),
                ("first box".to_string(), format!("{first} gt")),
                ("cycle".to_string(), format!("{cycle} gt")),
            ];
        }
    }
    rep
}
