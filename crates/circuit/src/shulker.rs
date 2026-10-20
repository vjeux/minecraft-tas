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
/// The `{Items:[..]}` SNBT the judge merges into a chest: the stacks in slots 0.. in order.
pub fn items_snbt(stacks: &[Stack]) -> String {
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
    /// Badge `mixed` (loaders): the source cycles through [`MIXED`] stacks [`MIXED_ROUNDS`] times;
    /// every item must end in a delivered box, every box but the last with 27 occupied slots, the
    /// last within [`FLUSH_WAIT`] gt of the source running dry.
    pub mixed: bool,
    /// Badge `filter` (loaders with a `port reject`): one box's worth of the item in full stacks,
    /// with dirt and snowballs between ([`FILTER_JUNK`]); the box must leave full of the item
    /// alone, and every other item must reach the reject chest.
    pub filter: bool,
    /// Mixed into the level's seed (the drop stress runs: the same build, other drop draws).
    pub seed_salt: i64,
    /// Drop stress, burst runs (loaders): every supplied box already holds all but one stack, so
    /// boxes break right after they are placed and drops follow each other within a hopper's
    /// cooldown: a drop that waits on a busy collection hopper drifts the furthest.
    pub burst: bool,
    /// Badge `combo`: the design's mode lever (`port input.mode`) is on: a loader judged as an
    /// unloader (or the other way), on the same chests.
    pub mode_on: bool,
}

/// Stress runs after a design passes its builds: the plain scenario (unstackables, or the design's
/// own item when it sorts) at rotations in turn with other drop draws; one lost drop fails the
/// design. A drop drifts up to about half a block before a hopper takes it, so a missing wall loses
/// one now and then: 16 builds see 48 drops, these add 192 (a 1-in-100 loss shows with p ~ 0.9).
pub const DROP_STRESS: u32 = 64;

/// Badge `filter`: the junk between the item's stacks, alternating (dirt 8, sticks 2).
pub const FILTER_JUNK: [(&str, u32); 2] = [("minecraft:dirt", 8), ("minecraft:stick", 2)];

/// Badge `mixed`: the source's stacks, in order, one round (cobblestone, snowballs and dirt, full
/// and partial stacks, so boxes fill with partial stacks a comparator never reads as full).
pub const MIXED: [(&str, u32); 6] =
    [("minecraft:cobblestone", 64), ("minecraft:snowball", 16), ("minecraft:dirt", 37), ("minecraft:cobblestone", 20), ("minecraft:snowball", 5), ("minecraft:dirt", 64)];
/// A rare item, once at the very start: its slot stays partial for good, so a design that waits
/// for a box to read full jams.
pub const MIXED_RARE: (&str, u32) = ("minecraft:iron_ingot", 3);
/// About 2.5 boxes of slots.
pub const MIXED_ROUNDS: usize = 16;

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
        Scenario { loader, item, stack, boxes: BOXES, spare: 1, cap, colours: false, pause: None, starve: false, poll_entities: false, groups: 1, prefill: false, partial: false, mixed: false, filter: false, seed_salt: 0, burst: false, mode_on: false }
    }

    /// The scenario's items in a stack (a box's contents included): the scenario item, or with
    /// `mixed` / `filter` every item the source supplies.
    pub fn counts(&self, s: &Stack) -> u64 {
        if !self.mixed && !self.filter {
            return s.count_of(self.item);
        }
        let own = if self.is_item(&s.id) { s.count as u64 } else { 0 };
        own + s.contents.iter().map(|x| self.counts(x)).sum::<u64>()
    }

    /// Whether an item belongs to the scenario (what the source supplies).
    pub fn is_item(&self, id: &str) -> bool {
        if self.mixed {
            MIXED.iter().any(|(m, _)| *m == id) || id == MIXED_RARE.0
        } else if self.filter {
            id == self.item || FILTER_JUNK.iter().any(|(j, _)| *j == id)
        } else {
            id == self.item
        }
    }

    /// Everything a loader's source supplies per copy, stack by stack, in order.
    pub fn source_stacks(&self) -> Vec<Stack> {
        if self.mixed {
            let mut v = vec![Stack::new(MIXED_RARE.0, MIXED_RARE.1)];
            v.extend((0..MIXED_ROUNDS).flat_map(|_| MIXED.iter().map(|(id, n)| Stack::new(id, *n))));
            return v;
        }
        if self.filter {
            // the boxes' worth of the item in full stacks, junk after each
            let n = (self.total() / self.stack as u64) as usize;
            return (0..n).flat_map(|k| [Stack::new(self.item, self.stack), Stack::new(FILTER_JUNK[k % 2].0, FILTER_JUNK[k % 2].1)]).collect();
        }
        let total = self.source_total();
        let full = total / self.stack as u64;
        let mut v: Vec<Stack> = (0..full).map(|_| Stack::new(self.item, self.stack)).collect();
        if total % self.stack as u64 > 0 {
            v.push(Stack::new(self.item, (total % self.stack as u64) as u32));
        }
        v
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
        if self.mixed {
            return MIXED_RARE.1 as u64 + MIXED_ROUNDS as u64 * MIXED.iter().map(|(_, n)| *n as u64).sum::<u64>();
        }
        if self.filter {
            return self.source_stacks().iter().map(|s| s.count as u64).sum();
        }
        if self.partial {
            return (self.boxes as u64 - 1) * self.per_box() + self.per_box() / 2;
        }
        self.total() - self.prefilled()
    }

    /// The items the supplied boxes hold at the start (`top-up`).
    pub fn prefilled(&self) -> u64 {
        if !self.prefill && !self.burst {
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
    pub fn supply(&self) -> Vec<Stack> {
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
                if self.burst && self.loader && (k as u32) < self.boxes {
                    b.contents = (0..26).map(|_| Stack::new(self.item, self.stack)).collect();
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
    /// What the design's own containers held before the judge filled anything (a sorter's filter
    /// hopper, filler boxes): left out of `left_items` / `left_boxes` at the end.
    pub initial_items: u64,
    pub initial_boxes: u64,
    /// Badge `filter`: items that reached the reject chests (the junk), and anything else that
    /// did (the item itself, a box).
    pub rejected: u64,
    pub misrejected: Vec<String>,
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
    game_run_recorded(w, c, sc, margin, None)
}

/// A per-gt observer of a run (trace export): called at t = 0, once the judge's chests are filled
/// and the lever is on, then after every gt until the run is done; the run steps 1 gt at a time
/// while one is attached. Gets the watched box.
pub trait Recorder {
    fn tick(&mut self, w: &mut Worker, sb: u32, t: u32, from: Pos, to: Pos) -> Result<()>;
}

/// [`game_run`] with an optional [`Recorder`].
pub fn game_run_recorded(w: &mut Worker, c: &Circuit, sc: &Scenario, margin: i32, rec: Option<&mut dyn Recorder>) -> Result<std::result::Result<RunLog, String>> {
    let (from, to) = crate::run::watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let seed = build_seed(from, to, sc.seed_salt);
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    let r = run_in(w, sb, c, sc, from, to, rec);
    let _ = w.drop_sandbox(sb);
    r
}

/// The sandbox seed of a build: from where it stands (its watch box), so the builds of a verdict
/// sample different drops instead of replaying one, and the stress run's number. The bridge seeds
/// each piston-broken drop's own random from it (docs/mechanics.md, "Drops and the bridge's
/// seeds") and reseeds the sandbox's level random to it; redsim's replay of a trace takes it from
/// the trace's header, the viewer from the design (one formula: `redsim::redstone::shulker`).
pub fn build_seed(from: Pos, to: Pos, salt: i64) -> i64 {
    redsim::redstone::shulker::build_seed(from, to, salt)
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

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, sc: &Scenario, from: Pos, to: Pos, mut rec: Option<&mut dyn Recorder>) -> Result<std::result::Result<RunLog, String>> {
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
    // what the design's own containers hold before the judge fills anything (a sorter's filter
    // hopper, filler boxes): theirs, not the judge's
    for be in w.block_entities(sb, from, to)? {
        for s in stacks_of_be(&be.nbt) {
            log.initial_items += sc.counts(&s);
            if s.is_box() {
                log.initial_boxes += s.count as u64;
            }
        }
    }
    let rejects: Vec<Pos> = c.stores.iter().filter(|s| s.role == "reject").map(|s| s.pos).collect();
    let junk_total: u64 = if sc.filter { sc.source_stacks().iter().filter(|s| s.id != sc.item).map(|s| s.count as u64).sum() } else { 0 };
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
    // each copy's source gets the scenario's stacks in order (a cursor per copy)
    let stacks = sc.source_stacks();
    let mut cursor: Vec<usize> = vec![0; groups];
    let topup = |w: &mut Worker, log: &mut RunLog, t: u32, paused_until: &mut Option<u32>, cursor: &mut Vec<usize>| -> Result<()> {
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
        for (p, g) in sources.iter().zip(&source_groups) {
            if cursor[*g] >= stacks.len() {
                continue;
            }
            let mut have = read_chest(w, sb, *p)?;
            let before = have.len();
            while have.len() < per_chest && cursor[*g] < stacks.len() {
                // under the pause: nothing past the pause point before it ends
                if sc.pause.is_some() && paused_until.is_none() && log.supplied_g[*g] >= pause_at {
                    break;
                }
                let s = stacks[cursor[*g]].clone();
                log.supplied_g[*g] += s.count as u64;
                log.supplied += s.count as u64;
                have.push(s);
                cursor[*g] += 1;
            }
            if have.len() != before {
                w.merge_block_entity(sb, *p, &items_snbt(&have))?;
            }
        }
        Ok(())
    };
    topup(w, &mut log, 0, &mut paused_until, &mut cursor)?;
    // the start lever(s); a combo's mode lever (`port input.mode`) only in the other mode's run
    for i in &c.inputs {
        let mode = i.name == "mode" || i.name.starts_with("mode.");
        if i.how == InputHow::Lever && (!mode || sc.mode_on) {
            w.use_block(sb, i.pos)?;
        }
    }
    if let Some(r) = rec.as_mut() {
        r.tick(w, sb, 0, from, to)?;
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
    let seg = if window.is_some() || sc.poll_entities || rec.is_some() { 1 } else { SEG };
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
        if let Some(r) = rec.as_mut() {
            r.tick(w, sb, t, from, to)?;
        }
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
                    if sc.is_item(&s.id) && s.contents.is_empty() {
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
        // badge `filter`: the reject chests, emptied and counted (junk only may arrive there)
        for p in &rejects {
            let have = read_chest(w, sb, *p)?;
            if !have.is_empty() {
                for s in &have {
                    if s.id != sc.item && sc.is_item(&s.id) && s.contents.is_empty() {
                        log.rejected += s.count as u64;
                    } else {
                        log.misrejected.push(describe(s, sc));
                    }
                }
                w.merge_block_entity(sb, *p, "{Items:[]}")?;
            }
        }
        topup(w, &mut log, t, &mut paused_until, &mut cursor)?;
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
        let delivered_items: u64 = log.deliveries.iter().filter(|d| d.stack.is_box()).map(|d| sc.counts(&d.stack)).sum();
        let done = if sc.filter {
            log.supplied >= sc.source_total() && boxes_out >= sc.boxes && log.rejected >= junk_total
        } else if sc.mixed {
            log.supplied >= sc.source_total() && delivered_items >= sc.source_total()
        } else if sc.loader {
            boxes_out >= sc.boxes_all() && log.supplied >= sc.source_total() * sc.groups.max(1) as u64
        } else {
            boxes_out >= sc.boxes_all() && drained >= sc.total_all()
        };
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
            log.left_items += sc.counts(&s);
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
                log.left_items += sc.counts(&s);
                if s.is_box() {
                    log.left_boxes += s.count as u64;
                }
            }
            continue;
        }
        if let Some(s) = stack_of_entity(&e.state) {
            log.left_items += sc.counts(&s);
            if s.is_box() {
                log.left_boxes += s.count as u64;
            }
            log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos));
        } else {
            log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos));
        }
    }
    // the design's own contents are not left over: only what exceeds them
    log.left_items = log.left_items.saturating_sub(log.initial_items);
    log.left_boxes = log.left_boxes.saturating_sub(log.initial_boxes);
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
        let n = sc.counts(&d.stack);
        let others: Vec<&Stack> = d.stack.contents.iter().filter(|s| !sc.is_item(&s.id)).collect();
        if !others.is_empty() {
            return fail(format!("box {} holds {} (not the scenario item)", k + 1, others[0].id), table);
        }
        if sc.mixed {
            // `mixed`: a box leaves when it can take nothing more: 27 occupied slots (the last box
            // excepted: the flush); judged once the run is over
            continue;
        }
        if sc.filter {
            // `filter`: boxes hold the item alone, full
            if let Some(j) = d.stack.contents.iter().find(|s| s.id != sc.item) {
                return fail(format!("box {} holds {} (the filter lets only {} into boxes)", k + 1, j.id, clock::short(sc.item)), table);
            }
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
    if sc.mixed {
        if let Some((k, d)) = boxes.iter().enumerate().find(|(k, d)| k + 1 < boxes.len() && d.stack.contents.len() != 27) {
            return fail(format!("box {} left at t={} with {} occupied slots (it could take more: 27 wanted but in the last box)", k + 1, d.t, d.stack.contents.len()), table);
        }
    } else if boxes.len() as u32 != sc.boxes_all() {
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
    let mut accounted = if sc.loader { boxes.iter().map(|d| sc.counts(&d.stack)).sum::<u64>() } else { log.drained.last().map(|x| x.1).unwrap_or(0) };
    if sc.filter {
        if let Some(m) = log.misrejected.first() {
            return fail(format!("{m} reached the reject chest (only the other items may)"), table);
        }
        accounted += log.rejected;
    }
    if log.left_items > 0 {
        return fail(format!("{} items left in the machine at the end", log.left_items), table);
    }
    let want_items = if sc.partial || sc.mixed || sc.filter { sc.source_total() * sc.groups.max(1) as u64 } else { sc.total_all() };
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
    // a loader: the items in its delivered boxes over the run up to the last box (hoppers and
    // droppers buffer what leaves the source, so the source's outflow overstates a loader); an
    // unloader: the steady flow into its items chests
    let rate = if sc.loader {
        boxes.iter().map(|d| sc.counts(&d.stack)).sum::<u64>() as f64 * 20.0 / tn.max(1) as f64
    } else {
        flow_rate(sc, log).unwrap_or(sc.per_box() as f64 * 20.0 / cycle)
    };
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
            // a port on the design's own block (a BUD's reset input) sits on its copy's block
            let own = base.blocks.contains_key(p);
            if (d.blocks.contains_key(&q) && !own) || d.ports.values().any(|x| *x == q) {
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
    let n = sc.counts(s);
    let name = s.name.as_ref().map(|n| format!(" named {n}")).unwrap_or_default();
    if sc.mixed {
        return format!("{}{name} ({} stacks, {n} items)", clock::short(&s.id), s.contents.len());
    }
    let full = s.contents.len() == 27 && s.contents.iter().all(|x| x.id == sc.item && x.count == sc.stack);
    format!("{}{name} ({} stacks, {n} {}{})", clock::short(&s.id), s.contents.len(), clock::short(sc.item), if full { ", full" } else if n == 0 { ", empty" } else { "" })
}

/// The judge for the shulker kinds (`mctas circuit verify`): the BASE scenario at N positions x 4
/// rotations, the builds compared as for every kind.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    let mut sc = Scenario::base(&kind).ok_or_else(|| anyhow!("{}: not a shulker kind", kind.name()))?;
    // a design aid: MCTAS_SHULKER_BADGE=<badge> judges that badge's scenario as the base (its
    // table, its snapshots), without the other badges
    let focus = std::env::var("MCTAS_SHULKER_BADGE").ok();
    if let Some(b) = &focus {
        sc = if b == "burst" {
            // the drop stress's burst runs
            let mut s = Scenario::with_stack(sc.loader, 1);
            s.burst = sc.loader;
            s.spare = 0;
            s
        } else {
            badge_scenarios(c, &sc).into_iter().find(|(n, _)| n == b).map(|x| x.1).ok_or_else(|| anyhow!("MCTAS_SHULKER_BADGE={b}: no such badge scenario (or `burst`)"))?
        };
        // MCTAS_SHULKER_SALT=k: the drop draws of the stress run k
        sc.seed_salt = std::env::var("MCTAS_SHULKER_SALT").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    }
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
    if !rep.pass || focus.is_some() {
        return Ok(rep);
    }
    // the drop stress: other drop draws; one lost drop fails the design
    if let Some(why) = drop_stress(pool, c, &sc, o)? {
        rep.pass = false;
        rep.verdict = format!("fragile: {why}");
        rep.note = why.clone();
        if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0]) {
            if let Ok(k) = &mut b.result {
                k.outcome.pass = false;
                k.outcome.why = why;
            }
        }
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
    let starts = c.inputs.iter().filter(|i| !(i.name == "mode" || i.name.starts_with("mode."))).count();
    out.push(("always-on".to_string(), if starts == 0 { Ok(()) } else { Err("it needs its start lever".into()) }));
    // every badge run at every rotation, in one pool batch
    let runs = badge_scenarios(c, base);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(usize, u8)> = (0..runs.len()).flat_map(|i| turns.iter().map(move |t| (i, *t))).collect();
    let results = pool.map(jobs.clone(), |w, _, (i, t)| Ok(scenario_check(w, c, &runs[i].0, &runs[i].1, t, o.margin)));
    for (i, (name, _)) in runs.iter().enumerate() {
        let mut verdict: std::result::Result<(), String> = Ok(());
        for ((ji, t), r) in jobs.iter().zip(&results) {
            if *ji != i {
                continue;
            }
            let r = match r {
                Ok(r) => r.clone(),
                Err(e) => Err(format!("{e:#}")),
            };
            if let Err(why) = r {
                verdict = Err(format!("rotation {t}: {why}"));
                break;
            }
        }
        out.push((name.clone(), verdict));
    }
    Ok(out)
}

/// The drop stress ([`DROP_STRESS`] runs; an eighth of them for a design that sorts, which runs
/// its own item): `None` when every run passes, else why.
pub fn drop_stress(pool: &Pool, c: &Circuit, base: &Scenario, o: &Options) -> Result<Option<String>> {
    let sorts = c.stores.iter().any(|s| s.role == "reject");
    // MCTAS_SHULKER_STRESS=N: N runs instead (a design aid: measure how often a design loses one)
    let full = std::env::var("MCTAS_SHULKER_STRESS").ok().and_then(|v| v.parse().ok()).unwrap_or(DROP_STRESS);
    let n = if sorts { (full / 8).max(1) } else { full };
    let jobs: Vec<u32> = (1..=n).collect();
    let results = pool.map(jobs.clone(), |w, _, k| {
        let mut sc = Scenario::with_stack(base.loader, if sorts { base.stack } else { 1 });
        sc.seed_salt = k as i64;
        // every other run a burst (loaders): boxes break right after they are placed
        if base.loader && k % 2 == 1 {
            sc.burst = true;
            sc.spare = 0;
        }
        let t = if o.rotations { (k % 4) as u8 } else { 0 };
        Ok(scenario_check(w, c, "base", &sc, t, o.margin))
    });
    let mut bad = Vec::new();
    for (k, r) in jobs.iter().zip(results) {
        match r {
            Ok(Ok(())) => {}
            Ok(Err(why)) => bad.push(format!("run {k}: {why}")),
            Err(e) => bad.push(format!("run {k}: {e:#}")),
        }
    }
    // one failure is not enough: a drop pushed into a block takes a random way out, so a sound
    // design can lose one in a few hundred bursts; a missing wall loses one every few runs
    Ok((bad.len() >= 2).then(|| format!("{} of {n} drop stress runs fail ({})", bad.len(), bad[0])))
}

/// The badge scenarios a design is judged on (`always-on` is static): (badge, scenario), all with
/// unstackables but `16-stack`.
pub fn badge_scenarios(c: &Circuit, base: &Scenario) -> Vec<(String, Scenario)> {
    let loader = base.loader;
    // a design with a reject chest sorts for its kind's own item (a filter set up for it): its
    // badges run with that item, the others with unstackables (short runs)
    let quick_stack = if c.stores.iter().any(|s| s.role == "reject") { base.stack } else { 1 };
    let quick = |f: &dyn Fn(&mut Scenario)| {
        let mut s = Scenario::with_stack(loader, quick_stack);
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
        // 6 empty boxes: as many as the stacks need; the run lasts the items at hopper pace
        runs.push(("mixed".into(), quick(&|s| {
            s.mixed = true;
            s.spare = 3;
            s.cap = (s.source_total() * 8) as u32 * 5 / 4 + FLUSH_WAIT + 6 * 400 + 2000;
        })));
        // a design with a reject chest: one box's worth of its own item with junk between
        if c.stores.iter().any(|s| s.role == "reject") {
            let mut s = Scenario::with_stack(loader, base.stack);
            s.filter = true;
            s.boxes = 1;
            s.cap = (s.source_total() * 8) as u32 * 5 / 4 + 2 * 400 + 2000;
            runs.push(("filter".into(), s));
        }
    }
    // a loader starts with 2 empty boxes, an unloader with 2 full ones; the rest come later
    runs.push(("box-starved".into(), quick(&|s| {
        s.starve = true;
        s.cap += STARVE_WAIT + 2000;
    })));
    runs.push(("drop-free".into(), quick(&|s| s.poll_entities = true)));
    // badge `tileable` (a `tile:` header): the copies side by side, each judged on its own
    if let Some(t) = c.tile {
        runs.push(("tileable".into(), quick(&|s| s.groups = t.copies)));
    }
    // badge `combo` (a `port input.mode` lever): the other job on the same chests, mode lever on
    if c.inputs.iter().any(|i| i.name == "mode") {
        let mut s = Scenario::with_stack(!loader, quick_stack);
        s.mode_on = true;
        runs.push(("combo".into(), s));
    }
    runs
}

/// One scenario at one rotation, judged (the plain judge, then the badge's own check): `Ok(())`
/// or why not. `name` is the badge (`base` for none).
pub fn scenario_check(w: &mut Worker, c: &Circuit, name: &str, sc: &Scenario, turns: u8, margin: i32) -> std::result::Result<(), String> {
    let mut run = || -> Result<std::result::Result<RunLog, String>> {
        let rc = c.rotated(w, turns)?;
        let rc = if sc.groups > 1 { tiled(&rc)? } else { rc };
        game_run(w, &rc, sc, margin)
    };
    match run() {
        Ok(Ok(log)) => {
            let out = judge(sc, &log);
            if !out.pass { Err(out.why) } else { badge_check(name, sc, &log) }
        }
        Ok(Err(why)) => Err(why),
        Err(e) => Err(format!("{e:#}")),
    }
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
        "flush" | "mixed" => {
            // from the gt the source ran dry for good to the last (partial) box's arrival
            let dry = log.flow.iter().find(|(_, m)| *m >= sc.source_total()).map(|x| x.0).ok_or("the source never ran dry")?;
            let last = log.deliveries.iter().filter(|d| d.stack.is_box()).map(|d| d.t).max().unwrap_or(0);
            if last > dry + FLUSH_WAIT {
                return Err(format!("the last box arrived {} gt after the source ran dry (most {FLUSH_WAIT})", last - dry));
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
