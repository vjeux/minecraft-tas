//! Study 19's HOPPER-MINECART UNLOADER and LOADER (clk-obs, coordinator 2026-10-21: "storage
//! transport"): `circuit: cart-unloader` and `circuit: cart-loader`.
//!
//! The design marks its line like a station ([`crate::station`]): `port line.in:` (required) and
//! `port line.out:` (optional), the first cells of the judge's own track. With both, the station
//! stands on a through line and the cart leaves out through `line.out`; with `line.in` alone it is
//! a terminus and the cart leaves back out through `line.in`. The judge's chest is `port output:`
//! (an unloader: where every item must end) or `port items:` (a loader: the supply): an air cell,
//! not counted. No input: it runs by itself.
//!
//! UNLOADER: a hopper minecart arrives holding a load. It must come to rest in the station, stay
//! there while it holds any item, and leave on its own within [`GO`] gt of its last item leaving
//! it, rolling on `TRACK - 2` blocks along the judge's track. An empty cart may leave without
//! stopping. Every item must end in the judge's chest (checked after each cart, once the design's
//! containers hold what they held at the start): none lost, none left in the cart or the design.
//!
//! LOADER: an empty hopper minecart arrives. It must come to rest, stay while it can still take
//! items (not full and the supply not dry), and leave on its own within [`GO`] gt of becoming full
//! (every slot at its item's max stack) or of the supply running dry (the judge's chest and the
//! design's containers hold no more of it). What a full cart leaves in the supply goes to the next
//! cart. The judge's supplies use at most 5 kinds per cart, so a cart that can take no more of the
//! supply is full or the supply is dry. Nothing lost: every supplied item ends in a cart.
//!
//! Both: 12 carts a run (a loader 13: the last after its supply waited 3000 gt), arriving at 0.02 to
//! 2.0 b/t: mixed loads, single items, full stacks,
//! 16-stackables, non-stackables, empty carts (an unloader) or an empty supply (a loader). The
//! loads use items the real-use audit never drops; its stray items ([`crate::intrusion::DROPS`])
//! fall on every open hopper top before 3 of the carts, and may end in the chest or a cart. A
//! station's hopper under the cart is open by necessity (the cart rolls over it): strays that land
//! there are cargo, so the next cart may leave later by their own transfer time (8 gt a stray item),
//! and that cart's delay is left out of the metric. Metrics:
//! blocks, rate (items per second while carts stood in the station), departure delay (the longest
//! from empty, full or dry to the cart's first move).

use crate::check::Outcome;
use crate::shulker::{self, CHEST, Stack};
use crate::station::{self, End};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::EntitySpec;
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// A hopper minecart's slots.
pub const SLOTS: usize = 5;
/// A hopper moves one item per this many gt.
pub const PER_ITEM: u32 = 8;
/// The cart must start moving within this many gt of becoming empty (unloader) or full or dry
/// (loader).
pub const GO: u32 = 200;
/// Extra time allowed for a cart's transfer beyond one hopper's pace.
pub const SLACK: u32 = 600;

/// The two kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Unload,
    Load,
}

fn mode(c: &Circuit) -> Option<Mode> {
    match c.kind {
        Some(Kind::CartUnloader) => Some(Mode::Unload),
        Some(Kind::CartLoader) => Some(Mode::Load),
        _ => None,
    }
}

/// An item's max stack (the judge's items).
pub fn max_stack(id: &str) -> u32 {
    match id.trim_start_matches("minecraft:") {
        "snowball" | "ender_pearl" | "egg" => 16,
        "iron_sword" | "bow" | "iron_shovel" => 1,
        _ => 64,
    }
}

fn full_id(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

/// The stray items' ids ([`crate::intrusion::DROPS`]).
fn is_stray(id: &str) -> bool {
    matches!(id.trim_start_matches("minecraft:"), "cobblestone" | "iron_ingot" | "iron_shovel")
}

/// One cart of the run: its load (unloader: the cart's slots, `None` = an empty slot) or the supply
/// added to the judge's chest before it comes (loader), and its momentum toward the station.
#[derive(Clone, Debug)]
pub struct Cycle {
    pub stacks: Vec<Option<(&'static str, u32)>>,
    pub v: f64,
    pub note: &'static str,
    /// Ticks the supply waits in the judge's chest before the cart is sent (a loader: a cart that
    /// comes long after the supply).
    pub wait: u32,
}

fn cy(stacks: &[(&'static str, u32)], v: f64, note: &'static str) -> Cycle {
    Cycle { stacks: stacks.iter().map(|s| Some(*s)).collect(), v, note, wait: 0 }
}

/// The unloader's carts.
pub fn unload_cycles() -> Vec<Cycle> {
    vec![
        cy(&[("dirt", 64), ("sand", 64), ("snowball", 16), ("iron_sword", 1), ("oak_log", 37)], 2.0, "mixed"),
        cy(&[("dirt", 1)], 0.02, "one item"),
        cy(&[("sand", 64); 5], 1.0, "full stacks"),
        cy(&[("iron_sword", 1); 5], 0.1, "non-stackables"),
        cy(&[], 0.4, "empty"),
        cy(&[("ender_pearl", 16); 5], 0.25, "16-stackables"),
        cy(&[("oak_log", 30), ("glass", 5)], 1.5, "partial, mixed"),
        cy(&[("glass", 64); 3], 0.6, "three stacks"),
        cy(&[("bow", 1)], 0.05, "one non-stackable"),
        cy(&[("dirt", 64), ("bow", 1), ("dirt", 64), ("bow", 1), ("dirt", 64)], 0.8, "stacks and non-stackables"),
        cy(&[], 2.0, "empty"),
        Cycle { stacks: vec![None, None, None, None, Some(("sand", 2))], v: 0.1, note: "the last slot only", wait: 0 },
    ]
}

/// The loader's supplies (added to the judge's chest before each cart).
pub fn load_cycles() -> Vec<Cycle> {
    vec![
        cy(&[("dirt", 64)], 2.0, "one stack: dry"),
        cy(&[("sand", 64); 5], 0.02, "a cart's worth: full"),
        cy(&[("dirt", 64), ("dirt", 64), ("dirt", 64), ("dirt", 64), ("dirt", 64), ("dirt", 64), ("dirt", 16)], 1.0, "400 items: full, 80 left"),
        cy(&[], 0.1, "the 80 left: dry"),
        cy(&[("oak_log", 1)], 1.5, "one item: dry"),
        cy(&[("iron_sword", 1); 5], 0.4, "5 non-stackables: full"),
        cy(&[("bow", 1); 6], 0.25, "6 non-stackables: full, 1 left"),
        cy(&[], 0.8, "the 1 left: dry"),
        cy(&[("dirt", 64), ("snowball", 16), ("iron_sword", 1), ("sand", 30), ("glass", 64)], 0.6, "mixed: dry"),
        cy(&[], 0.05, "no supply: dry on arrival"),
        cy(&[("ender_pearl", 16); 5], 1.0, "16-stackables: full"),
        cy(&[("glass", 10)], 0.1, "ten items: dry"),
        Cycle { wait: 3000, ..cy(&[("sand", 64), ("sand", 64), ("sand", 64)], 0.4, "three stacks waiting 3000 gt: dry") },
    ]
}

/// The cart's NBT: its items and momentum.
fn cart_nbt(stacks: &[Option<(&str, u32)>], motion: [f64; 2]) -> String {
    let items: Vec<String> = stacks.iter().enumerate().filter_map(|(k, s)| s.map(|(id, n)| format!("{{Slot:{k}b,id:\"{}\",count:{n}}}", full_id(id)))).collect();
    format!("{{Motion:[{:.6}d,0.0d,{:.6}d],Items:[{}]}}", motion[0], motion[1], items.join(","))
}

fn tally<'a>(stacks: impl IntoIterator<Item = &'a Stack>) -> BTreeMap<String, u32> {
    let mut m = BTreeMap::new();
    for s in stacks {
        *m.entry(s.id.clone()).or_insert(0) += s.count;
    }
    m
}

fn total(m: &BTreeMap<String, u32>) -> u32 {
    m.values().sum()
}

fn without_strays(m: &BTreeMap<String, u32>) -> BTreeMap<String, u32> {
    m.iter().filter(|(k, _)| !is_stray(k)).map(|(k, v)| (k.clone(), *v)).collect()
}

fn show(m: &BTreeMap<String, u32>) -> String {
    if m.is_empty() {
        return "nothing".into();
    }
    m.iter().map(|(k, v)| format!("{v} {}", k.trim_start_matches("minecraft:"))).collect::<Vec<_>>().join(", ")
}

/// Every slot at its item's max stack.
pub fn is_full(stacks: &[Stack]) -> bool {
    stacks.len() == SLOTS && stacks.iter().all(|s| s.count == max_stack(&s.id))
}

/// The line's ends: the entry and, on a through line, the exit.
pub fn ends(c: &Circuit) -> Result<(End, Option<End>)> {
    let entry = station::end_of(c, "line.in")?;
    let exit = if c.clock.design.ports.contains_key("line.out") { Some(station::end_of(c, "line.out")?) } else { None };
    Ok((entry, exit))
}

/// The judge chest's cell.
fn chest_cell(c: &Circuit) -> Option<Pos> {
    match mode(c)? {
        Mode::Unload => c.outputs.first().map(|o| o.pos),
        Mode::Load => c.stores.iter().find(|s| s.role == "items").map(|s| s.pos),
    }
}

/// The static checks: the line ends, the judge's chest, no input, the judge's tracks clear.
pub fn check_ports(c: &Circuit) -> Result<()> {
    let m = mode(c).ok_or_else(|| anyhow!("not a cart loader or unloader"))?;
    let (entry, exit) = ends(c)?;
    if !c.inputs.is_empty() {
        bail!("{}: runs by itself (no input port)", c.kind.as_ref().map(|k| k.name()).unwrap_or_default());
    }
    match m {
        Mode::Unload if c.outputs.len() != 1 => bail!("cart-unloader: needs one `port output: x y z` (the judge's chest every item must end in: an air cell)"),
        Mode::Load if !c.stores.iter().any(|s| s.role == "items") => bail!("cart-loader: needs `port items: x y z` (the judge's supply chest: an air cell)"),
        _ => {}
    }
    let p = chest_cell(c).ok_or_else(|| anyhow!("no judge chest"))?;
    if c.clock.design.blocks.contains_key(&p) {
        bail!("the judge's chest goes on an air cell, the design has a block at {p:?}");
    }
    for (name, e) in std::iter::once(("line.in", &entry)).chain(exit.as_ref().map(|e| ("line.out", e))) {
        for k in 0..station::TRACK {
            let q = [e.cell[0] + k * e.out[0], e.cell[1], e.cell[2] + k * e.out[2]];
            if c.clock.design.blocks.contains_key(&q) || q == p {
                bail!("port {name}: the judge's track runs {} cells straight out from it, {q:?} is taken", station::TRACK);
            }
        }
    }
    Ok(())
}

/// What one run saw.
#[derive(Debug, Default)]
pub struct RunLog {
    pub lines: Vec<String>,
    pub fail: Option<String>,
    /// Items moved and the ticks carts stood in the station moving them.
    pub moved: u32,
    pub standing: u32,
    /// The longest from empty / full / dry to the cart's first move (gt).
    pub delay: u32,
    pub through: bool,
    pub dims: [i32; 3],
    pub ticks: u32,
    /// This cart's extra time to leave: strays were just dropped into the design's open hoppers,
    /// and a hopper busy with them (8 gt an item) may hold the cart (its delay is left out of the
    /// metric).
    pub extra_go: u32,
}

/// The cart's position and items (`None`: gone).
fn read(w: &mut Worker, sb: u32, uuid: &str) -> Result<Option<([f64; 3], Vec<Stack>)>> {
    Ok(w.entities(sb)?.into_iter().find(|e| e.uuid == uuid).map(|e| (e.pos, shulker::stacks_of_entity_items(&e.state))))
}

/// The judge chest's stacks and the design's containers' tally.
fn holdings(w: &mut Worker, sb: u32, from: Pos, to: Pos, chest: Pos) -> Result<(Vec<Stack>, BTreeMap<String, u32>)> {
    let mut judge = Vec::new();
    let mut design = BTreeMap::new();
    for be in w.block_entities(sb, from, to)? {
        let st = shulker::stacks_of_be(&be.nbt);
        if be.pos == chest {
            judge = st;
        } else {
            for s in st {
                *design.entry(s.id.clone()).or_insert(0) += s.count;
            }
        }
    }
    Ok((judge, design))
}

/// Loose item entities' tally.
fn loose(w: &mut Worker, sb: u32) -> Result<BTreeMap<String, u32>> {
    let mut m = BTreeMap::new();
    for e in w.entities(sb)? {
        if e.kind.trim_start_matches("minecraft:") == "item"
            && let Some(s) = shulker::stack_of_entity(&e.state)
        {
            *m.entry(s.id).or_insert(0) += s.count;
        }
    }
    Ok(m)
}

/// Run a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (entry, exit) = match ends(c) {
        Ok(e) => e,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let (mut from, mut to) = crate::run::watch_box(c, margin);
    for e in std::iter::once(&entry).chain(exit.as_ref()) {
        let far = [e.cell[0] + station::TRACK * e.out[0], e.cell[1], e.cell[2] + station::TRACK * e.out[2]];
        for i in 0..3 {
            from[i] = from[i].min(far[i] - 1);
            to[i] = to[i].max(far[i] + 1);
        }
    }
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, entry, exit, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

struct Site {
    entry: End,
    exit: Option<End>,
    chest: Pos,
    from: Pos,
    to: Pos,
    lo: Pos,
    hi: Pos,
    /// The design's containers' tally before the run (strays left out).
    initial: BTreeMap<String, u32>,
    mode: Mode,
}

impl Site {
    /// Where a leaving cart goes and the wrong way.
    fn leaving(&self) -> (&End, Option<&End>) {
        match &self.exit {
            Some(x) => (x, Some(&self.entry)),
            None => (&self.entry, None),
        }
    }
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, entry: End, exit: Option<End>, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let m = mode(c).ok_or_else(|| anyhow!("not a cart loader or unloader"))?;
    let chest = chest_cell(c).ok_or_else(|| anyhow!("no judge chest"))?;
    let mut cells = station::track(c, &entry);
    if let Some(x) = &exit {
        cells.extend(station::track(c, x));
    }
    cells.push((chest, CHEST.to_string()));
    w.set_blocks(sb, &cells, PlaceMode::Raw)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    let (lo, hi) = c.clock.design.bounds().ok_or_else(|| anyhow!("an empty design"))?;
    let (_, design0) = holdings(w, sb, from, to, chest)?;
    let site = Site { entry, exit, chest, from, to, lo, hi, initial: without_strays(&design0), mode: m };
    let mut log = RunLog { through: site.exit.is_some(), dims: shulker::design_dims(c), ..Default::default() };
    let list = if m == Mode::Unload { unload_cycles() } else { load_cycles() };
    // open hopper tops, but not one the judge's own chest covers
    let intakes: Vec<Pos> = crate::intrusion::intakes(c).into_iter().filter(|p| *p != chest).collect();
    let drop_at: Vec<usize> = if intakes.is_empty() || !crate::check::real_use() { Vec::new() } else { (1..=3).map(|k| list.len() * k / 4).collect() };
    let mut t = 0u32;
    // the loader's books: supplied and delivered (strays left out)
    let mut supplied: BTreeMap<String, u32> = BTreeMap::new();
    let mut delivered: BTreeMap<String, u32> = BTreeMap::new();
    for (k, cyc) in list.iter().enumerate() {
        log.extra_go = 0;
        if let Some(j) = drop_at.iter().position(|&d| d == k) {
            let item = crate::intrusion::DROPS[j];
            let n: u32 = item.split("count:").nth(1).and_then(|r| r.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|v| v.parse().ok()).unwrap_or(1);
            log.extra_go = PER_ITEM * n * intakes.len() as u32;
            let specs: Vec<EntitySpec> = intakes.iter().map(|p| EntitySpec { kind: "minecraft:item".into(), pos: [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5], nbt: Some(format!("{{Item:{item}}}")) }).collect();
            w.summon(sb, &specs)?;
            log.lines.push(format!("before cart {}: {item} dropped on each open hopper top {intakes:?}", k + 1));
        }
        let label = format!("cart {} ({}, {:.2} b/t)", k + 1, cyc.note, cyc.v);
        let r = match m {
            Mode::Unload => unload(w, sb, &site, cyc, &mut t, &mut log)?,
            Mode::Load => {
                for (id, n) in cyc.stacks.iter().flatten() {
                    *supplied.entry(full_id(id)).or_insert(0) += n;
                }
                load(w, sb, &site, cyc, &mut t, &mut log, &mut delivered)?
            }
        };
        match r {
            Ok(line) => log.lines.push(format!("{label}: {line}")),
            Err(why) => {
                log.fail = Some(format!("{label}: {why}"));
                break;
            }
        }
    }
    if log.fail.is_none() {
        // the end: everything accounted for
        w.step(sb, 200)?;
        t += 200;
        let (judge, design) = holdings(w, sb, from, to, chest)?;
        let left = without_strays(&loose(w, sb)?);
        let judge = without_strays(&tally(&judge));
        let design = without_strays(&design);
        if !left.is_empty() {
            log.fail = Some(format!("the end: items left loose: {}", show(&left)));
        } else if design != site.initial {
            log.fail = Some(format!("the end: the design's containers hold {}, they held {} at the start", show(&design), show(&site.initial)));
        } else if m == Mode::Load && !judge.is_empty() {
            log.fail = Some(format!("the end: the supply still holds {}", show(&judge)));
        } else if m == Mode::Load && delivered != supplied {
            log.fail = Some(format!("the end: carts took {}, the supply was {}", show(&delivered), show(&supplied)));
        }
    }
    log.ticks = t;
    Ok(Ok(log))
}

fn summon_cart(w: &mut Worker, sb: u32, e: &End, cyc: &Cycle, load: bool) -> Result<String> {
    let at = [e.cell[0] as f64 + 0.5 - 0.48 * e.out[0] as f64, e.cell[1] as f64 + station::RAIL_Y, e.cell[2] as f64 + 0.5 - 0.48 * e.out[2] as f64];
    let motion = [-(e.out[0] as f64) * cyc.v, -(e.out[2] as f64) * cyc.v];
    let stacks: Vec<Option<(&str, u32)>> = if load { Vec::new() } else { cyc.stacks.clone() };
    w.summon(sb, &[EntitySpec { kind: "minecraft:hopper_minecart".into(), pos: at, nbt: Some(cart_nbt(&stacks, motion)) }])?.into_iter().next().ok_or_else(|| anyhow!("summon: no uuid"))
}

fn fmt(p: [f64; 3]) -> String {
    format!("({:.2}, {:.2}, {:.2})", p[0], p[1], p[2])
}

fn step(w: &mut Worker, sb: u32, n: u32, t: &mut u32) -> Result<()> {
    w.step(sb, n)?;
    *t += n;
    Ok(())
}

/// Where the cart rests: `Ok(Some(pos))` at rest, `Ok(None)` left its way (allowed by `may_leave`).
#[allow(clippy::too_many_arguments)]
fn arrive(w: &mut Worker, sb: u32, s: &Site, uuid: &str, t: &mut u32, may_leave: &dyn Fn(&[Stack]) -> bool, transferred: &mut bool) -> Result<std::result::Result<Option<[f64; 3]>, String>> {
    let Some((mut prev, first)) = read(w, sb, uuid)? else { return Ok(Err("the summoned cart is missing".into())) };
    let n0: u32 = first.iter().map(|x| x.count).sum();
    let (out, wrong) = s.leaving();
    let mut still = 0u32;
    for _ in 0..station::ARRIVE {
        step(w, sb, 1, t)?;
        let Some((p, items)) = read(w, sb, uuid)? else { return Ok(Err("the cart was destroyed".into())) };
        let n: u32 = items.iter().map(|x| x.count).sum();
        if n != n0 {
            *transferred = true;
        }
        if station::gone(p, out) || wrong.is_some_and(|x| station::gone(p, x)) {
            if wrong.is_some_and(|x| station::gone(p, x)) {
                return Ok(Err(format!("went the wrong way without stopping, holding {n} items")));
            }
            if may_leave(&items) {
                return Ok(Ok(None));
            }
            return Ok(Err(format!("went through without stopping, holding {n} items ({:.2} b/t)", station::hdist(prev, p))));
        }
        if s.exit.is_some() && station::gone(p, &s.entry) {
            return Ok(Err("rolled back out through line.in".into()));
        }
        if station::dist(prev, p) < station::STILL {
            still += 1;
        } else {
            still = 0;
        }
        prev = p;
        if still >= station::REST {
            let inside = p[0] >= s.lo[0] as f64 && p[0] <= (s.hi[0] + 1) as f64 && p[2] >= s.lo[2] as f64 && p[2] <= (s.hi[2] + 1) as f64;
            if !inside {
                return Ok(Err(format!("came to rest at {}, outside the station", fmt(p))));
            }
            return Ok(Ok(Some(p)));
        }
    }
    Ok(Err(format!("still moving {} gt after it was sent in", station::ARRIVE)))
}

/// After `t0` (empty, full or dry): the cart starts within [`GO`] gt and leaves its way. Returns
/// the delay and the cart's items as it left.
fn depart(w: &mut Worker, sb: u32, s: &Site, uuid: &str, rest: [f64; 3], t: &mut u32, extra: u32) -> Result<std::result::Result<(u32, Vec<Stack>), String>> {
    let t0 = *t;
    let mut moved = None;
    for _ in 0..GO + extra {
        step(w, sb, 1, t)?;
        let Some((p, _)) = read(w, sb, uuid)? else { return Ok(Err("the cart was destroyed".into())) };
        if station::dist(rest, p) > 1.0e-3 {
            moved = Some(*t - t0);
            break;
        }
    }
    let Some(delay) = moved else { return Ok(Err(format!("rests at {} and has not moved {} gt later", fmt(rest), GO + extra))) };
    let (out, wrong) = s.leaving();
    let mut last = Vec::new();
    for _ in 0..station::LEAVE {
        step(w, sb, 1, t)?;
        let Some((p, items)) = read(w, sb, uuid)? else { return Ok(Err("the cart was destroyed".into())) };
        last = items;
        if wrong.is_some_and(|x| station::gone(p, x)) {
            return Ok(Err("left the wrong way (back out through line.in)".into()));
        }
        if station::gone(p, out) {
            return Ok(Ok((delay, last)));
        }
    }
    let _ = last;
    Ok(Err(format!("started {delay} gt after, but did not roll {} blocks along the judge's track within {} gt", station::TRACK - 2, station::LEAVE)))
}

fn kill(w: &mut Worker, sb: u32, uuid: &str) -> Result<()> {
    // a container minecart killed drops its contents (`AbstractMinecartContainer.remove`, KILLED
    // destroys): emptied first, so the judge's own removal leaves nothing on the track
    w.exec(sb, &format!("data merge entity {uuid} {{Items:[]}}"))?;
    w.exec(sb, &format!("kill {uuid}"))
}

fn unload(w: &mut Worker, sb: u32, s: &Site, cyc: &Cycle, t: &mut u32, log: &mut RunLog) -> Result<std::result::Result<String, String>> {
    let want: BTreeMap<String, u32> = {
        let mut m = BTreeMap::new();
        for (id, n) in cyc.stacks.iter().flatten() {
            *m.entry(full_id(id)).or_insert(0) += n;
        }
        m
    };
    let n_load = total(&want);
    let uuid = summon_cart(w, sb, &s.entry, cyc, false)?;
    let t_in = *t;
    let mut transferred = false;
    let rest = match arrive(w, sb, s, &uuid, t, &|items| items.is_empty(), &mut transferred)? {
        Err(why) => {
            kill(w, sb, &uuid)?;
            return Ok(Err(why));
        }
        Ok(r) => r,
    };
    let mut line;
    match rest {
        None => line = "empty: went on without stopping".to_string(),
        Some(r) => {
            // unloading: it stays while it holds any item
            let limit = PER_ITEM * n_load + SLACK;
            let mut n = u32::MAX;
            while n > 0 {
                let chunk = if n <= 3 { 1 } else { 4 };
                step(w, sb, chunk, t)?;
                let Some((p, items)) = read(w, sb, &uuid)? else { return Ok(Err("the cart was destroyed".into())) };
                n = items.iter().map(|x| x.count).sum();
                if station::dist(r, p) > 1.0e-3 && n > 0 {
                    kill(w, sb, &uuid)?;
                    return Ok(Err(format!("rested at {} but moved while it still held {n} items ({})", fmt(r), show(&tally(&items)))));
                }
                if *t - t_in > limit {
                    kill(w, sb, &uuid)?;
                    return Ok(Err(format!("rests at {} and still holds {n} items ({}) {} gt after it came in", fmt(r), show(&tally(&items)), *t - t_in)));
                }
            }
            let t_empty = *t;
            log.moved += n_load;
            log.standing += t_empty - t_in;
            match depart(w, sb, s, &uuid, r, t, log.extra_go)? {
                Err(why) => {
                    kill(w, sb, &uuid)?;
                    return Ok(Err(format!("empty {} gt after it came in, then: {why}", t_empty - t_in)));
                }
                Ok((delay, _)) => {
                    if log.extra_go == 0 {
                        log.delay = log.delay.max(delay);
                    }
                    line = format!("rests at {}, empty {} gt after it came in, leaves {delay} gt later", fmt(r), t_empty - t_in);
                }
            }
        }
    }
    kill(w, sb, &uuid)?;
    // the design settles: its containers back to what they held at the start
    let t_settle = *t;
    let mut design = BTreeMap::new();
    let mut judge = Vec::new();
    while *t - t_settle <= PER_ITEM * n_load + SLACK {
        (judge, design) = holdings(w, sb, s.from, s.to, s.chest)?;
        if without_strays(&design) == s.initial {
            break;
        }
        step(w, sb, PER_ITEM, t)?;
    }
    if without_strays(&design) != s.initial {
        return Ok(Err(format!("{line}; the design's containers still hold {} (they held {} at the start)", show(&without_strays(&design)), show(&s.initial))));
    }
    let got = tally(&judge);
    let strays: BTreeMap<String, u32> = got.iter().filter(|(k, _)| is_stray(k)).map(|(k, v)| (k.clone(), *v)).collect();
    if without_strays(&got) != want {
        return Ok(Err(format!("{line}; the chest got {}, the cart held {}", show(&without_strays(&got)), show(&want))));
    }
    if !strays.is_empty() {
        line.push_str(&format!("; strays in the chest: {}", show(&strays)));
    }
    // the chest emptied for the next cart
    let c = s.chest;
    w.exec(sb, &format!("data merge block {} {} {} {{Items:[]}}", c[0], c[1], c[2]))?;
    Ok(Ok(format!("{line}; all {n_load} items in the chest")))
}

/// The judge chest's contents rewritten with `add` after what it holds.
fn add_supply(w: &mut Worker, sb: u32, s: &Site, add: &[Option<(&str, u32)>]) -> Result<()> {
    if add.is_empty() {
        return Ok(());
    }
    let (judge, _) = holdings(w, sb, s.from, s.to, s.chest)?;
    let mut items: Vec<String> = Vec::new();
    for (k, st) in judge.iter().enumerate() {
        items.push(format!("{{Slot:{k}b,id:\"{}\",count:{}}}", st.id, st.count));
    }
    let base = items.len();
    for (j, (id, n)) in add.iter().flatten().enumerate() {
        items.push(format!("{{Slot:{}b,id:\"{}\",count:{n}}}", base + j, full_id(id)));
    }
    let c = s.chest;
    w.exec(sb, &format!("data merge block {} {} {} {{Items:[{}]}}", c[0], c[1], c[2], items.join(",")))
}

/// The supply still to come: the judge's chest and what the design holds beyond its start.
fn upstream(w: &mut Worker, sb: u32, s: &Site) -> Result<i64> {
    let (judge, design) = holdings(w, sb, s.from, s.to, s.chest)?;
    Ok(total(&tally(&judge)) as i64 + total(&design) as i64 - total(&s.initial) as i64)
}

fn load(w: &mut Worker, sb: u32, s: &Site, cyc: &Cycle, t: &mut u32, log: &mut RunLog, delivered: &mut BTreeMap<String, u32>) -> Result<std::result::Result<String, String>> {
    add_supply(w, sb, s, &cyc.stacks)?;
    if cyc.wait > 0 {
        step(w, sb, cyc.wait, t)?;
    }
    let uuid = summon_cart(w, sb, &s.entry, cyc, true)?;
    let t_in = *t;
    let mut transferred = false;
    // it may go on without stopping when, as it leaves, it is full or the supply is dry (it took
    // all there was on its way in): checked below
    let rest = match arrive(w, sb, s, &uuid, t, &|_| true, &mut transferred)? {
        Err(why) => {
            kill(w, sb, &uuid)?;
            return Ok(Err(why));
        }
        Ok(r) => r,
    };
    let (line, took) = match rest {
        None => {
            let items = read(w, sb, &uuid)?.map(|(_, it)| it).unwrap_or_default();
            let up = upstream(w, sb, s)?;
            let n: u32 = items.iter().map(|x| x.count).sum();
            if !is_full(&items) && up > 0 {
                kill(w, sb, &uuid)?;
                return Ok(Err(format!("went on without stopping, holding {n} items while {up} more were waiting")));
            }
            (format!("went on without stopping, {} with {n} items", if is_full(&items) { "full" } else { "the supply dry" }), items)
        }
        Some(r) => {
            let budget = PER_ITEM * (SLOTS as u32 * 64) + SLACK;
            let mut items;
            loop {
                let up = upstream(w, sb, s)?;
                let Some((p, it)) = read(w, sb, &uuid)? else { return Ok(Err("the cart was destroyed".into())) };
                items = it;
                let n: u32 = items.iter().map(|x| x.count).sum();
                let full = is_full(&items);
                if full || up <= 0 {
                    break;
                }
                if station::dist(r, p) > 1.0e-3 {
                    kill(w, sb, &uuid)?;
                    return Ok(Err(format!("rested at {} but left holding {n} items while {up} more were coming (not full)", fmt(r))));
                }
                if *t - t_in > budget {
                    kill(w, sb, &uuid)?;
                    return Ok(Err(format!("rests at {} holding {n} items, {up} more waiting, {} gt after it came in", fmt(r), *t - t_in)));
                }
                let chunk = if up <= 3 || SLOTS as u32 * 64 - n.min(SLOTS as u32 * 64) <= 3 { 1 } else { 4 };
                step(w, sb, chunk, t)?;
            }
            let t_done = *t;
            let n: u32 = items.iter().map(|x| x.count).sum();
            let why = if is_full(&items) { "full" } else { "dry" };
            log.moved += n;
            log.standing += t_done - t_in;
            match depart(w, sb, s, &uuid, r, t, log.extra_go)? {
                Err(e) => {
                    kill(w, sb, &uuid)?;
                    return Ok(Err(format!("{why} with {n} items {} gt after it came in, then: {e}", t_done - t_in)));
                }
                Ok((delay, left_with)) => {
                    if log.extra_go == 0 {
                        log.delay = log.delay.max(delay);
                    }
                    let ln = format!("rests at {}, {why} with {n} items {} gt after it came in, leaves {delay} gt later", fmt(r), t_done - t_in);
                    (ln, left_with)
                }
            }
        }
    };
    kill(w, sb, &uuid)?;
    let got = tally(&took);
    let strays: BTreeMap<String, u32> = got.iter().filter(|(k, _)| is_stray(k)).map(|(k, v)| (k.clone(), *v)).collect();
    for (k, v) in without_strays(&got) {
        *delivered.entry(k).or_insert(0) += v;
    }
    let mut ln = format!("{line}; took {}", show(&without_strays(&got)));
    if !strays.is_empty() {
        ln.push_str(&format!(" and strays {}", show(&strays)));
    }
    Ok(Ok(ln))
}

/// The run's verdict.
pub fn judge(c: &Circuit, log: &RunLog) -> Outcome {
    let table = log.lines.clone();
    if let Some(f) = &log.fail {
        return Outcome { pass: false, why: f.clone(), metrics: Vec::new(), table };
    }
    let rate = if log.standing > 0 { log.moved as f64 * 20.0 / log.standing as f64 } else { 0.0 };
    let what = if matches!(c.kind, Some(Kind::CartLoader)) { "load rate" } else { "unload rate" };
    let metrics = vec![
        ("line".to_string(), if log.through { "through" } else { "terminus" }.to_string()),
        (what.to_string(), format!("{rate:.2} items/s")),
        ("departure delay".to_string(), format!("{} gt", log.delay)),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The judge (`mctas circuit verify`): the run at N positions x 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    if mode(c).is_none() {
        bail!("{}: not a cart loader or unloader", kind.name());
    }
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&rc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = 0;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    // the worst over the builds: the slowest rate, the longest delay
    let num = |k: &BuildOk, name: &str| k.outcome.metrics.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok());
    let oks: Vec<&BuildOk> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok()).filter(|k| k.outcome.pass).collect();
    let rate_key = if matches!(c.kind, Some(Kind::CartLoader)) { "load rate" } else { "unload rate" };
    let rate = oks.iter().filter_map(|k| num(k, rate_key)).fold(f64::MAX, f64::min);
    let delay = oks.iter().filter_map(|k| num(k, "departure delay")).fold(0.0f64, f64::max);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        for (n, v) in k.outcome.metrics.iter_mut() {
            if n == rate_key {
                *v = format!("{rate:.2} items/s");
            } else if n == "departure delay" {
                *v = format!("{} gt", delay as u32);
            }
        }
    }
    Ok(rep)
}

/// A design aid (`mctas circuit cart-trim`): the blocks a design does not need. Every block is
/// removed alone and the design re-run (in parallel); then the removable ones are taken out one at a
/// time, each re-run with the earlier ones gone. Returns the trimmed design and the removed cells.
/// `circuit verify` has the last word (every position and rotation).
pub fn trim(pool: &Pool, c: &Circuit, margin: i32) -> Result<(mctas::design::Design, Vec<Pos>)> {
    let base = c.clock.design.clone();
    let cells: Vec<Pos> = base.blocks.keys().copied().collect();
    let passes = |d: &mctas::design::Design, w: &mut Worker| -> Result<bool> {
        let Ok(rc) = Circuit::from_design(d.clone()) else { return Ok(false) };
        Ok(match game_run(w, &rc, margin)? {
            Ok(log) => judge(&rc, &log).pass,
            Err(_) => false,
        })
    };
    let jobs: Vec<usize> = (0..cells.len()).collect();
    let results = pool.map(jobs.clone(), |w, _, i| {
        let mut d = base.clone();
        d.blocks.remove(&cells[i]);
        passes(&d, w)
    });
    let spare: Vec<Pos> = jobs.iter().zip(results).filter_map(|(i, r)| r.ok().filter(|ok| *ok).map(|_| cells[*i])).collect();
    println!("{} of {} blocks spare alone: {spare:?}", spare.len(), cells.len());
    let mut d = base.clone();
    let mut gone = Vec::new();
    for p in spare {
        let mut e = d.clone();
        e.blocks.remove(&p);
        let ok = pool.map(vec![0u8], |w, _, _| passes(&e, w)).into_iter().next().ok_or_else(|| anyhow!("no run"))??;
        if ok {
            d = e;
            gone.push(p);
            println!("  removed {p:?}");
        }
    }
    Ok((d, gone))
}

/// A design aid (`mctas circuit cart-run`): one run turned and moved like one of `circuit verify`'s
/// builds.
pub fn run_one(pool: &Pool, c: &Circuit, turns: u8, offset: Pos, margin: i32) -> Result<Outcome> {
    let r = pool.map(vec![0u8], |w, _, _| {
        let rc = c.rotated(w, turns)?.translated(offset)?;
        Ok((game_run(w, &rc, margin)?, rc))
    });
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        (Ok(log), rc) => Ok(judge(&rc, &log)),
        (Err(why), _) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads() {
        // the loads never use a stray item, and the loader's supplies stay within 5 kinds a cart
        for c in unload_cycles().iter().chain(load_cycles().iter()) {
            for (id, n) in c.stacks.iter().flatten() {
                assert!(!is_stray(id), "{id} is a stray item");
                assert!(*n <= max_stack(id), "{n} {id} over a stack");
            }
            assert!(c.stacks.len() <= 7);
        }
        let full = |v: &[(&str, u32)]| is_full(&v.iter().map(|(id, n)| Stack { id: full_id(id), count: *n, contents: Vec::new(), name: None }).collect::<Vec<_>>());
        assert!(full(&[("sand", 64); 5]));
        assert!(full(&[("ender_pearl", 16); 5]));
        assert!(full(&[("bow", 1); 5]));
        assert!(!full(&[("sand", 64); 4]));
        assert!(!full(&[("dirt", 64), ("snowball", 16), ("iron_sword", 1), ("sand", 30), ("glass", 64)]));
        assert!(cart_nbt(&[None, Some(("sand", 2))], [0.1, 0.0]).contains("{Slot:1b,id:\"minecraft:sand\",count:2}"));
        // the bridge's entity state: pretty SNBT, whitespace collapsed
        let st = "{ Enabled: 1b, Items: [ { Slot: 0b, count: 64, id: \"minecraft:dirt\" }, { Slot: 3b, count: 2, id: \"minecraft:sand\" } ], mctas_rail: \"0,2,0\" }";
        let got = shulker::stacks_of_entity_items(st);
        assert_eq!(got.iter().map(|s| (s.id.as_str(), s.count)).collect::<Vec<_>>(), vec![("minecraft:dirt", 64), ("minecraft:sand", 2)]);
        assert!(shulker::stacks_of_entity_items("{ Enabled: 1b, mctas_rail: \"none\" }").is_empty());
    }
}
