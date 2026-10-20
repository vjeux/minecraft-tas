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
use anyhow::{Context, Result, anyhow, bail};
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
/// The stacks a container entity holds (its `Items:[...]` list: a hopper or chest minecart).
fn stacks_of_entity_items(state: &str) -> Vec<Stack> {
    let Some(i) = state.find("Items:[") else { return Vec::new() };
    let s = &state[i + 6..];
    let mut depth = 0i32;
    for (k, ch) in s.char_indices() {
        match ch {
            '[' | '{' => depth += 1,
            ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return stacks_of_be(&format!("{{Items:{}}}", &s[..=k]));
                }
            }
            _ => {}
        }
    }
    Vec::new()
}

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
    /// Badge `colours`: the box supply mixes colours and one named box.
    pub colours: bool,
    /// Badge `restart`: the source stops for this many gt once 1.5 boxes' worth was supplied.
    pub pause: Option<u32>,
    /// Badge `box-starved`: the design gets 2 boxes (a loader empty ones, an unloader full ones);
    /// STARVE_WAIT gt after the 2nd arrives the judge adds the rest.
    pub starve: bool,
    /// Badge `drop-free`: entities polled every gt (each item entity's life and ground contact).
    pub poll_entities: bool,
    /// Badge `tileable`: the copies of a tiled design ([`tiled`]), each with its own chests and
    /// its own `boxes` and items (ports `<name>.t<k>` belong to copy k).
    pub groups: u32,
    /// Badge `top-up` (loaders): the supplied boxes already hold some of the item ([`Scenario::supply`]).
    pub prefill: bool,
    /// Badge `flush` (loaders): 2.5 boxes' worth; the half box must arrive within [`FLUSH_WAIT`] gt
    /// after the source runs dry.
    pub partial: bool,
}

/// Badge `flush`: the most gt from the source running dry to the partial box's arrival.
/// Badge `flush`: the most gt from the source running dry to the partial box's arrival (5 min:
/// longer than `restart`'s 3000 gt pause, so a design that waits between the two gets both).
pub const FLUSH_WAIT: u32 = 6000;

/// The ticks a starved loader waits for its 3rd box (badge `box-starved`).
pub const STARVE_WAIT: u32 = 3000;
/// An item entity picked up within this many gt and never on the ground: badge `drop-free`.
pub const DROP_LIFE: u32 = 8;

impl Scenario {
    /// The BASE scenario of a kind.
    pub fn base(kind: &Kind) -> Option<Scenario> {
        let (loader, stack) = match kind {
            Kind::ShulkerLoader(s) => (true, *s),
            Kind::ShulkerUnloader(s) => (false, *s),
            _ => return None,
        };
        Some(Scenario::with_stack(loader, stack))
    }

    /// The plain scenario for a max stack (64, 16, 1).
    pub fn with_stack(loader: bool, stack: u32) -> Scenario {
        let (item, stack) = scenario_item(stack);
        let per_box = 27 * stack;
        // one hopper's pace (8 gt per item) plus a generous box swap, x 1.25, + 2000
        let cap = BOXES * (per_box * 8 + 400) * 5 / 4 + 2000;
        Scenario { loader, item, stack, boxes: BOXES, spare: 1, cap, colours: false, pause: None, starve: false, poll_entities: false, groups: 1, prefill: false, partial: false }
    }

    pub fn per_box(&self) -> u64 {
        27 * self.stack as u64
    }

    pub fn total(&self) -> u64 {
        self.boxes as u64 * self.per_box()
    }

    /// What a loader's source supplies per copy: the boxes' worth, less what the supplied boxes
    /// already hold (`top-up`), or 2.5 boxes' worth (`flush`).
    pub fn source_total(&self) -> u64 {
        if self.partial {
            return (self.boxes as u64 - 1) * self.per_box() + self.per_box() / 2;
        }
        self.total() - self.prefilled()
    }

    /// The items the supplied boxes hold at the start (`top-up`).
    pub fn prefilled(&self) -> u64 {
        if !self.prefill {
            return 0;
        }
        self.supply().iter().map(|b| b.count_of(self.item)).sum()
    }

    /// Over every copy of a tiled run.
    pub fn total_all(&self) -> u64 {
        self.total() * self.groups as u64
    }

    pub fn boxes_all(&self) -> u32 {
        self.boxes * self.groups
    }

    /// A full box of the scenario item.
    fn full_box(&self) -> Stack {
        Stack { id: BOX.to_string(), count: 1, contents: (0..27).map(|_| Stack::new(self.item, self.stack)).collect(), name: None }
    }

    /// The box supply: plain boxes, or (badge `colours`) red, blue, a named plain box, then plain;
    /// a loader's are empty (badge `top-up`: the first three already hold 10 stacks and a half,
    /// 1 item, 26 stacks), an unloader's full.
    fn supply(&self) -> Vec<Stack> {
        let n = if self.loader { self.boxes + self.spare } else { self.boxes };
        (0..n)
            .map(|k| {
                let mut b = if self.loader { Stack::new(BOX, 1) } else { self.full_box() };
                if self.colours {
                    match k {
                        0 => b.id = "minecraft:red_shulker_box".into(),
                        1 => b.id = "minecraft:blue_shulker_box".into(),
                        2 => b.name = Some("\"Box A\"".into()),
                        _ => {}
                    }
                }
                if self.prefill && self.loader {
                    let full = |n: usize| (0..n).map(|_| Stack::new(self.item, self.stack)).collect::<Vec<_>>();
                    b.contents = match k {
                        0 => {
                            let mut v = full(10);
                            if self.stack > 1 {
                                v.push(Stack::new(self.item, self.stack / 2));
                            }
                            v
                        }
                        1 => vec![Stack::new(self.item, 1)],
                        2 => full(26),
                        _ => Vec::new(),
                    };
                }
                b
            })
            .collect()
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
    /// Design hoppers unlocked at rest (lag: each looks for items every gt).
    pub awake: u32,
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
    /// The boxes the judge supplied (all of them, held ones included).
    pub supply: Vec<Stack>,
    /// Badge `box-starved`: the held boxes were added.
    pub refilled: bool,
    /// Badge `drop-free`: per item entity (uuid): first and last gt seen, whether it touched the
    /// ground.
    pub item_lives: std::collections::BTreeMap<String, (u32, u32, bool)>,
    /// (gt, items moved so far) at every change: what left a loader's source chests, what reached
    /// an unloader's items chests ([`flow_rate`]).
    pub flow: Vec<(u32, u64)>,
    /// Per copy of a tiled run (one entry otherwise): items supplied (loader), drained (unloader).
    pub supplied_g: Vec<u64>,
    pub drained_g: Vec<u64>,
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
    let source_groups: Vec<usize> = c.stores.iter().filter(|s| s.role == "items").map(|s| group_of(&s.name) as usize).collect();
    let supplies: Vec<Pos> = c.stores.iter().filter(|s| s.role == "boxes").map(|s| s.pos).collect();
    let supply_groups: Vec<u32> = c.stores.iter().filter(|s| s.role == "boxes").map(|s| group_of(&s.name)).collect();
    let outs: Vec<(String, Pos)> = c.outputs.iter().map(|o| (o.name.clone(), o.pos)).collect();
    let chests: Vec<(Pos, String)> = c.stores.iter().map(|s| s.pos).chain(outs.iter().map(|o| o.1)).map(|p| (p, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    let start = c.clock.free_from() + SETTLE;
    w.step(sb, start)?;
    let base = w.get_region(sb, from, to)?;
    let groups = sc.groups.max(1) as usize;
    let mut log = RunLog { dims: design_dims(c), supplied_g: vec![0; groups], drained_g: vec![0; groups], ..Default::default() };
    // the start: the box supply, the items (a loader's source, topped up later), the lever
    let per_chest = 27usize;
    let mut supply: Vec<Stack> = sc.supply();
    // badge `box-starved`: the design starts with 2 boxes, the rest come later
    let held: Vec<Stack> = if sc.starve { supply.split_off(2.min(supply.len())) } else { Vec::new() };
    for g in 0..groups as u32 {
        let mine: Vec<Pos> = supplies.iter().zip(&supply_groups).filter(|(_, sg)| **sg == g).map(|(p, _)| *p).collect();
        for (k, chunk) in supply.chunks(per_chest).enumerate() {
            let p = *mine.get(k).ok_or_else(|| anyhow!("{} boxes need {} `boxes` chests (copy {g})", supply.len(), supply.len().div_ceil(per_chest)))?;
            w.merge_block_entity(sb, p, &items_snbt(chunk))?;
        }
        log.supply.extend(supply.iter().cloned());
    }
    log.supply.extend(held.iter().cloned());
    // badge `restart`: the source stops for a while once 1.5 boxes' worth was supplied
    let pause_at = sc.total() / 2;
    let mut paused_until: Option<u32> = None;
    let topup = |w: &mut Worker, log: &mut RunLog, t: u32, paused_until: &mut Option<u32>| -> Result<()> {
        if !sc.loader {
            return Ok(());
        }
        if let Some(len) = sc.pause {
            if paused_until.is_none() && log.supplied >= pause_at {
                *paused_until = Some(t + len);
            }
            if paused_until.is_some_and(|u| t < u) {
                return Ok(());
            }
        }
        // each copy's sources get that copy's items
        for (p, g) in sources.iter().zip(&source_groups) {
            if log.supplied_g[*g] >= sc.source_total() {
                continue;
            }
            let mut have = read_chest(w, sb, *p)?;
            let before = have.len();
            // under the pause: never more than the pause point before it ends
            let cap = if sc.pause.is_some() && paused_until.is_none() { pause_at } else { sc.source_total() };
            while have.len() < per_chest && log.supplied_g[*g] < cap {
                let n = (cap - log.supplied_g[*g]).min(sc.stack as u64) as u32;
                have.push(Stack::new(sc.item, n));
                log.supplied_g[*g] += n as u64;
                log.supplied += n as u64;
            }
            if have.len() != before {
                w.merge_block_entity(sb, *p, &items_snbt(&have))?;
            }
        }
        Ok(())
    };
    topup(w, &mut log, 0, &mut paused_until)?;
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
    // the record box: every output chest (the per-gt frames time each delivery) and the items
    // chests (the flow: what left a loader's sources, what reached an unloader's sinks)
    let watch: Vec<Pos> = outs.iter().map(|o| o.1).chain(sources.iter().copied()).collect();
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
    let seg = if window.is_some() || sc.poll_entities { 1 } else { SEG };
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
            // the flow: items moved so far (a loader: supplied minus what its sources still hold;
            // an unloader: drained before this segment plus what its sinks hold now)
            let mut held = 0u64;
            for p in &sources {
                if let Some(b) = bes.iter().find(|b| b.pos == *p) {
                    held += stacks_of_be(&b.nbt).iter().map(|s| s.count as u64).sum::<u64>();
                }
            }
            let moved = if sc.loader { log.supplied.saturating_sub(held) } else { drained + held };
            let all_seen = sources.iter().all(|p| bes.iter().any(|b| b.pos == *p));
            if all_seen && log.flow.last().is_none_or(|x| x.1 != moved) {
                log.flow.push((t + k as u32 + 1, moved));
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
        for (p, g) in sinks.iter().zip(&source_groups) {
            let have = read_chest(w, sb, *p)?;
            if !have.is_empty() {
                for s in &have {
                    if s.id == sc.item && s.contents.is_empty() {
                        drained += s.count as u64;
                        log.drained_g[*g] += s.count as u64;
                    } else {
                        log.foreign.push(s.id.clone());
                    }
                }
                log.drained.push((t, drained));
                w.merge_block_entity(sb, *p, "{Items:[]}")?;
            }
        }
        topup(w, &mut log, t, &mut paused_until)?;
        let boxes_out = log.deliveries.iter().filter(|d| d.stack.is_box()).count() as u32;
        // badge `box-starved`: STARVE_WAIT gt after the 2nd box arrives, the held boxes come
        if !held.is_empty() && boxes_out >= 2 {
            let second = log.deliveries.iter().filter(|d| d.stack.is_box()).nth(1).map(|d| d.t).unwrap_or(t);
            if t >= second + STARVE_WAIT && !log.refilled {
                let p = supplies[0];
                let mut have = read_chest(w, sb, p)?;
                have.extend(held.iter().cloned());
                w.merge_block_entity(sb, p, &items_snbt(&have))?;
                log.refilled = true;
            }
        }
        // badge `drop-free`: the item entities alive now (with SEG = 1 every gt is seen)
        if sc.poll_entities {
            for e in w.entities(sb)?.into_iter().filter(|e| !e.design && e.kind.ends_with("item")) {
                let r = log.item_lives.entry(e.uuid.clone()).or_insert((t, t, false));
                r.1 = t;
                r.2 |= e.on_ground;
            }
        }
        let done = if sc.loader { boxes_out >= sc.boxes_all() && log.supplied >= sc.source_total() * sc.groups.max(1) as u64 } else { boxes_out >= sc.boxes_all() && drained >= sc.total_all() };
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
    // lag: the design's hoppers still unlocked at rest
    log.awake = b.0.iter().filter(|(p, s)| c.clock.design.blocks.contains_key(p) && s.starts_with("minecraft:hopper[") && s.contains("enabled=true")).count() as u32;
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
            // a design's container entity (a hopper minecart): what it holds is in the machine
            for s in stacks_of_entity_items(&e.state) {
                log.left_items += s.count_of(sc.item);
                if s.is_box() {
                    log.left_boxes += s.count as u64;
                }
            }
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
            // `flush`: the last box carries the half box's worth, whatever its slots
            let flushed = sc.partial && k as u32 + 1 == sc.boxes && n == sc.source_total() - (sc.boxes as u64 - 1) * sc.per_box();
            if !full && !flushed {
                return fail(format!("box {} arrived at t={} with {n} of {} items (not full)", k + 1, d.t, sc.per_box()), table);
            }
        } else if n > 0 {
            return fail(format!("box {} arrived at t={} with {n} items left in it", k + 1, d.t), table);
        }
    }
    if log.timed_out {
        return fail(format!("{} of {} boxes after {} gt (the cap)", boxes.len(), sc.boxes_all(), log.end), table);
    }
    if boxes.len() as u32 != sc.boxes_all() {
        return fail(format!("{} boxes delivered, {} expected", boxes.len(), sc.boxes_all()), table);
    }
    // a tiled run: each copy delivers its own boxes and drains its own items
    if sc.groups > 1 {
        for g in 0..sc.groups {
            let n = boxes.iter().filter(|d| group_of(&d.port) == g).count() as u32;
            if n != sc.boxes {
                return fail(format!("copy {g} delivered {n} boxes, {} expected (the copies interfere)", sc.boxes), table);
            }
            if !sc.loader && log.drained_g.get(g as usize).copied().unwrap_or(0) != sc.total() {
                return fail(format!("copy {g} drained {} of {} items (the copies interfere)", log.drained_g.get(g as usize).copied().unwrap_or(0), sc.total()), table);
            }
        }
    }
    if !log.foreign.is_empty() {
        return fail(format!("{} reached the items chest", log.foreign[0]), table);
    }
    let accounted = if sc.loader { boxes.iter().map(|d| d.stack.count_of(sc.item)).sum::<u64>() } else { log.drained.last().map(|x| x.1).unwrap_or(0) };
    if log.left_items > 0 {
        return fail(format!("{} items left in the machine at the end", log.left_items), table);
    }
    let want_items = if sc.partial { sc.source_total() * sc.groups.max(1) as u64 } else { sc.total_all() };
    if accounted != want_items {
        return fail(format!("{} of {} items accounted for (lost or duplicated)", accounted, want_items), table);
    }
    let boxes_in = (if sc.loader { sc.boxes + sc.spare } else { sc.boxes } * sc.groups.max(1)) as u64;
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
    let rate = flow_rate(sc, log).unwrap_or(sc.per_box() as f64 * 20.0 / cycle);
    let metrics = vec![
        ("boxes/min".to_string(), format!("{:.3}", rate * 60.0 / sc.per_box() as f64)),
        ("items/s".to_string(), format!("{rate:.2}")),
        ("first box".to_string(), format!("{t1} gt")),
        ("cycle".to_string(), format!("{} gt", cycle.round() as i64)),
        // lag: hoppers left unlocked at rest look for items every gt
        ("hoppers awake".to_string(), log.awake.to_string()),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The copy a port belongs to in a tiled run: `<name>.t<k>` (or a bare `t<k>`, an output port
/// `output.t<k>`) is copy k, every other port copy 0.
pub fn group_of(name: &str) -> u32 {
    let last = name.rsplit('.').next().unwrap_or(name);
    last.strip_prefix('t').and_then(|k| k.parse::<u32>().ok()).filter(|_| last.len() > 1).unwrap_or(0)
}

/// A tiled design ([`crate::Tile`]): `copies` copies along the step, copy k's ports renamed
/// `<port>.t<k>` (copy 0 keeps its names), as one circuit the runner judges copy by copy.
pub fn tiled(c: &Circuit) -> Result<Circuit> {
    let t = c.tile.ok_or_else(|| anyhow!("{}: no `tile:` header", c.name()))?;
    let base = c.clock.design.clone();
    let mut d = base.clone();
    d.meta.remove("tile");
    d.meta.insert("tiled".into(), t.copies.to_string());
    for k in 1..t.copies as i32 {
        let off = [t.step[0] * k, t.step[1] * k, t.step[2] * k];
        let sh = |p: &Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        for (p, s) in &base.blocks {
            let q = sh(p);
            if let Some(o) = d.blocks.get(&q) {
                bail!("tile: copy {k} puts {} on {} at {q:?} (the copies overlap)", clock::short(s), clock::short(o));
            }
            d.blocks.insert(q, s.clone());
        }
        for (p, s) in &base.block_entities {
            d.block_entities.insert(sh(p), s.clone());
        }
        for e in &base.entities {
            let mut e2 = e.clone();
            for i in 0..3 {
                e2.pos[i] += off[i] as f64;
            }
            d.entities.push(e2);
        }
        for (name, p) in &base.ports {
            let q = sh(p);
            if d.blocks.contains_key(&q) || d.ports.values().any(|x| *x == q) {
                bail!("tile: copy {k}'s port {name} at {q:?} lands on the other copies");
            }
            let renamed = format!("{name}.t{k}");
            if let Some(args) = base.meta.get(&format!("port {name}")) {
                d.meta.insert(format!("port {renamed}"), args.clone());
            }
            d.ports.insert(renamed, q);
        }
    }
    // copy 0's ports must not land on the other copies' blocks either
    for (name, p) in &base.ports {
        if !base.blocks.contains_key(p) && d.blocks.contains_key(p) {
            bail!("tile: port {name} at {p:?} is covered by another copy");
        }
    }
    Circuit::from_design(d)
}

/// The steady flow in items/s: the items moved (a loader: taken from its source chests; an
/// unloader: drained into its items chests) between 10% and 90% of the scenario's total, over
/// the gt between those two moments (per-gt block-entity frames). `None` without enough samples.
pub fn flow_rate(sc: &Scenario, log: &RunLog) -> Option<f64> {
    let total = sc.total_all();
    let at = |q: u64| log.flow.iter().find(|(_, m)| *m >= q).copied();
    let (ta, qa) = at(total.div_ceil(10))?;
    let (tb, qb) = at(total * 9 / 10)?;
    (tb > ta).then(|| (qb - qa) as f64 * 20.0 / (tb - ta) as f64)
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
    let rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind.clone()), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    let mut rep = tolerate_jitter(rep, &sc);
    if !rep.pass {
        return Ok(rep);
    }
    // the variations (designs/circuits/shulker/VARIATIONS.md): badges, each judged at 4 rotations
    let badges = run_badges(pool, c, &sc, o)?;
    // `tileable(N)`: copies every N blocks
    let pitch = c.tile.map(|t| t.step.iter().map(|x| x.unsigned_abs()).sum::<u32>()).unwrap_or(0);
    let won: Vec<String> = badges.iter().filter(|b| b.1.is_ok()).map(|b| if b.0 == "tileable" { format!("tileable({pitch})") } else { b.0.clone() }).collect();
    let bill = bill(c);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass)) {
        if let Ok(k) = &mut b.result {
            k.outcome.metrics.push(("badges".to_string(), if won.is_empty() { "none".to_string() } else { won.join("+") }));
            k.outcome.metrics.push(("bill".to_string(), bill));
            for (name, r) in &badges {
                k.outcome.table.push(match r {
                    Ok(()) => format!("badge {name}: yes"),
                    Err(why) => format!("badge {name}: no ({why})"),
                });
            }
        }
    }
    Ok(rep)
}

/// The survival cost of the design's blocks: iron, quartz, redstone dust, slime balls (only the
/// ingredients that matter; wood and stone are free).
pub fn bill(c: &Circuit) -> String {
    let (mut fe, mut q, mut r, mut s) = (0u32, 0u32, 0u32, 0u32);
    let ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    for (p, st) in &c.clock.design.blocks {
        if ins.contains(p) {
            continue;
        }
        let id = st.split(['[', '{']).next().unwrap_or(st).trim_start_matches("minecraft:");
        let (a, b, d, e) = match id {
            "hopper" => (5, 0, 0, 0),
            "piston" => (1, 0, 1, 0),
            "sticky_piston" => (1, 0, 1, 1),
            "dispenser" | "dropper" | "redstone_torch" | "redstone_wall_torch" | "redstone_wire" | "note_block" | "target" => (0, 0, 1, 0),
            "observer" => (0, 1, 2, 0),
            "comparator" => (0, 1, 3, 0),
            "repeater" => (0, 0, 3, 0),
            "redstone_block" => (0, 0, 9, 0),
            "redstone_lamp" => (0, 0, 4, 0),
            "slime_block" => (0, 0, 0, 9),
            "iron_trapdoor" => (4, 0, 0, 0),
            "iron_block" => (9, 0, 0, 0),
            "hopper_minecart" | "minecart" => (5, 0, 0, 0),
            _ => (0, 0, 0, 0),
        };
        fe += a;
        q += b;
        r += d;
        s += e;
    }
    for e in &c.clock.design.entities {
        if e.kind.contains("minecart") {
            fe += 5 + if e.kind.contains("hopper") { 5 } else { 0 };
        }
    }
    let mut out = format!("{fe} iron, {q} quartz, {r} redstone");
    if s > 0 {
        out.push_str(&format!(", {s} slime"));
    }
    out
}

/// Judge the badge scenarios of a design that passed its BASE (each at 1 position x 4
/// rotations): (name, Ok or why not).
pub fn run_badges(pool: &Pool, c: &Circuit, base: &Scenario, o: &Options) -> Result<Vec<(String, std::result::Result<(), String>)>> {
    let mut out: Vec<(String, std::result::Result<(), String>)> = Vec::new();
    out.push(("always-on".to_string(), if c.inputs.is_empty() { Ok(()) } else { Err("it needs its start lever".into()) }));
    let loader = base.loader;
    let quick = |f: &dyn Fn(&mut Scenario)| {
        let mut s = Scenario::with_stack(loader, 1);
        f(&mut s);
        s
    };
    let mut runs: Vec<(String, Scenario)> = Vec::new();
    if base.stack == 64 {
        runs.push(("16-stack".into(), Scenario::with_stack(loader, 16)));
    }
    if base.stack != 1 {
        runs.push(("unstackable".into(), Scenario::with_stack(loader, 1)));
    }
    runs.push(("colours".into(), quick(&|s| s.colours = true)));
    if loader {
        runs.push(("restart".into(), quick(&|s| {
            s.pause = Some(3000);
            s.cap += 3000;
        })));
        // no spare: the dispenser places a random slot, so an empty spare could take a partial box's
        // turn and the items would run out before 3 boxes are full
        runs.push(("top-up".into(), quick(&|s| {
            s.prefill = true;
            s.spare = 0;
        })));
        runs.push(("flush".into(), quick(&|s| {
            s.partial = true;
            s.cap += FLUSH_WAIT;
        })));
    }
    // a loader starts with 2 empty boxes, an unloader with 2 full ones; the rest come later
    runs.push(("box-starved".into(), quick(&|s| {
        s.starve = true;
        s.cap += STARVE_WAIT + 2000;
    })));
    runs.push(("drop-free".into(), quick(&|s| s.poll_entities = true)));
    // badge `tileable` (a `tile:` header): the copies side by side, each judged on its own
    let tile = c.tile;
    if let Some(t) = tile {
        runs.push(("tileable".into(), quick(&|s| s.groups = t.copies)));
    }
    for (name, sc) in runs {
        let jobs: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
        let results = pool.map(jobs.clone(), |w, _, t| {
            let rc = c.rotated(w, t)?;
            let rc = if sc.groups > 1 { tiled(&rc)? } else { rc };
            game_run(w, &rc, &sc, o.margin)
        });
        let mut verdict: std::result::Result<(), String> = Ok(());
        for (t, r) in jobs.into_iter().zip(results) {
            let r = match r {
                Ok(Ok(log)) => {
                    let out = judge(&sc, &log);
                    if !out.pass {
                        Err(out.why)
                    } else {
                        badge_check(&name, &sc, &log)
                    }
                }
                Ok(Err(why)) => Err(why),
                Err(e) => Err(format!("{e:#}")),
            };
            if let Err(why) = r {
                verdict = Err(format!("rotation {t}: {why}"));
                break;
            }
        }
        out.push((name, verdict));
    }
    Ok(out)
}

/// The badge-specific part of a run's verdict (the run already passed the plain judge).
fn badge_check(name: &str, sc: &Scenario, log: &RunLog) -> std::result::Result<(), String> {
    match name {
        "colours" => {
            // every delivered box is one the judge supplied, colour and name kept
            let mut left: Vec<(String, Option<String>)> = log.supply.iter().map(|s| (s.id.clone(), s.name.clone())).collect();
            for d in log.deliveries.iter().filter(|d| d.stack.is_box()) {
                let key = (d.stack.id.clone(), d.stack.name.clone());
                match left.iter().position(|k| *k == key) {
                    Some(i) => {
                        left.remove(i);
                    }
                    None => return Err(format!("{} arrived but no such box was supplied (colour or name lost)", describe(&d.stack, sc))),
                }
            }
            Ok(())
        }
        "box-starved" if !log.refilled => Err("the run ended before the held boxes were added".into()),
        "flush" => {
            // from the gt the source ran dry for good to the partial box's arrival
            let dry = log.flow.iter().find(|(_, m)| *m >= sc.source_total()).map(|x| x.0).ok_or("the source never ran dry")?;
            let last = log.deliveries.iter().filter(|d| d.stack.is_box()).map(|d| d.t).max().unwrap_or(0);
            if last > dry + FLUSH_WAIT {
                return Err(format!("the half box arrived {} gt after the source ran dry (most {FLUSH_WAIT})", last - dry));
            }
            Ok(())
        }
        "drop-free" => {
            if let Some((_, (a, b, g))) = log.item_lives.iter().find(|(_, (a, b, g))| b - a > DROP_LIFE || *g) {
                return Err(format!("an item entity lived {} gt{}", b - a + 1, if *g { " and touched the ground" } else { "" }));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The drop pops at a random offset with a random drift, which can move the gt its hopper picks it
/// up by one or two: builds whose times differ from the base build's by at most [`JITTER`] gt, and
/// whose flows differ by at most [`FLOW_JITTER`] (and all pass), are the same circuit, `robust`.
pub const JITTER: i64 = 4;
pub const FLOW_JITTER: f64 = 0.03;

fn tolerate_jitter(mut rep: VerifyReport, sc: &Scenario) -> VerifyReport {
    if !rep.pass {
        return rep;
    }
    let times = |o: &Outcome| -> Option<(i64, i64, f64)> {
        let get = |k: &str| o.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.trim_end_matches(" gt").to_string());
        Some((get("first box")?.parse().ok()?, get("cycle")?.parse().ok()?, get("items/s")?.parse().ok()?))
    };
    let Some(base) = rep.base().and_then(|b| times(&b.outcome)) else { return rep };
    let all: Vec<(i64, i64, f64)> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok().filter(|k| k.outcome.pass).and_then(|k| times(&k.outcome))).collect();
    let close = all.len() == rep.builds.len() && all.iter().all(|(f, c, r)| (f - base.0).abs() <= JITTER && (c - base.1).abs() <= JITTER && (r - base.2).abs() <= FLOW_JITTER * base.2);
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
    let mut rates: Vec<f64> = all.iter().map(|x| x.2).collect();
    rates.sort_by(|a, b| a.total_cmp(b));
    let rate = rates[rates.len() / 2];
    if let Some(b) = rep.builds.iter_mut().find(|b| b.result.as_ref().is_ok_and(|k| k.outcome.pass) && b.turns == 0 && b.offset == [0, 0, 0]) {
        if let Ok(k) = &mut b.result {
            let timing = ["boxes/min", "items/s", "first box", "cycle"];
            let extra: Vec<(String, String)> = k.outcome.metrics.iter().filter(|(n, _)| !timing.contains(&n.as_str())).cloned().collect();
            k.outcome.metrics = vec![
                ("boxes/min".to_string(), format!("{:.3}", rate * 60.0 / sc.per_box() as f64)),
                ("items/s".to_string(), format!("{rate:.2}")),
                ("first box".to_string(), format!("{first} gt")),
                ("cycle".to_string(), format!("{cycle} gt")),
            ];
            k.outcome.metrics.extend(extra);
        }
    }
    rep
}
