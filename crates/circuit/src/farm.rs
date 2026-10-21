//! Study 18, crop farms (designs/circuits/farm/WR.md, docs/circuits.md "Crop farms"): kind
//! `harvester <crop>` (`sugar_cane` for now). A harvester breaks every grown block of its crop and
//! sends the items into a chest, by itself, forever.
//!
//! Judged in the game only (redsim has no random ticks): the judge plants the crop on every `port
//! crop:` (an air cell on the design's soil; the crop is not counted), puts a chest on every `port
//! output:` (an air cell, not counted), raises `random_tick_speed` to [`RTS`] (100 times vanilla's
//! 3, so the [`RUN`] ticks of the run hold about a hundred growths per cane) and records every
//! crop column after every tick: each block that grows above the bottom one, each harvest (blocks
//! gone), how long a grown block waited and how tall the canes got. Then growth stops, the run
//! rests until nothing moves, and the judge counts the items.
//!
//! Pass: no crop is lost (the bottom block stays), no cane stands at full height longer than
//! [`STUCK`] ticks (a missed harvest), every harvested block's item is in the judge's chests at
//! the end (no item on the ground, none lost to despawning, none left in the design's
//! containers), nothing else is left loose, and the build comes to rest. Metrics: blocks per
//! cane, items per hour per cane at vanilla speed (the measured harvest, scaled by 3 / RTS), the
//! harvest height (2: harvested as soon as it grows; 3: when full) and the longest wait from a
//! block growing to its harvest. A `tile:` design is also judged as copies side by side.

use crate::check::Outcome;
use crate::shulker::{self, CHEST, REST, stacks_of_be};
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;

/// `random_tick_speed` during the judged run (vanilla: 3).
pub const RTS: u32 = 300;
/// Vanilla's `random_tick_speed`, for the per-hour metric.
pub const VANILLA_RTS: u32 = 3;
/// The judged run: 20 minutes of game time.
pub const RUN: u32 = 24000;
/// Ticks per recorded batch.
const BATCH: u32 = 500;
/// `mctas circuit farm-run --trace`: 20-tick batches, and every crop item seen outside the crop
/// columns and the cells above the design's hoppers is noted (where items get lost).
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// A cane may stand at full height (3) at most this long before it is harvested.
pub const STUCK: u32 = 40;
/// Game ticks per hour.
const HOUR: f64 = 72000.0;
/// ITEM INTRUSION ([`crate::intrusion`], like every kind): at a quarter, half and three quarters
/// of the run the judge drops each of [`crate::intrusion::DROPS`] (one cobblestone, 64 iron ingots,
/// an iron shovel) on every open hopper intake of the design and above every crop column (items
/// falling onto the farm): they may end anywhere, but the crop must still all reach the chests.
pub const INTRUDERS: [(&str, u32); 3] = [("minecraft:cobblestone", 1), ("minecraft:iron_ingot", 64), ("minecraft:iron_shovel", 1)];

/// The crops a harvester may name, with their full block ids and the state the judge plants.
pub const CROPS: [(&str, &str); 6] = [
    ("sugar_cane", "minecraft:sugar_cane[age=0]"),
    ("bamboo", "minecraft:bamboo[age=0,leaves=none,stage=0]"),
    ("kelp", "minecraft:kelp[age=0]"),
    ("pumpkin", "minecraft:pumpkin_stem[age=0]"),
    ("melon", "minecraft:melon_stem[age=0]"),
    ("cactus", "minecraft:cactus[age=0]"),
];

/// A stem crop (pumpkin, melon): the judge plants the stem; its fruit grows on one of the four
/// cells beside it (the game picks a side at random; it grows there only on air with dirt or
/// farmland below). The column's height is 1 for the stem, 2 while a fruit is attached to it.
pub fn is_stem_crop(crop: &str) -> bool {
    crop == "minecraft:pumpkin" || crop == "minecraft:melon"
}

/// The item a harvest gives: the crop's own item, melon slices for a melon (3 to 7 a fruit, in one
/// stack).
pub fn crop_item(crop: &str) -> &str {
    if crop == "minecraft:melon" { "minecraft:melon_slice" } else { crop }
}

/// A crop name (`sugar_cane` or `minecraft:sugar_cane`) as its full id.
pub fn crop_id(name: &str) -> Result<String> {
    let short = name.strip_prefix("minecraft:").unwrap_or(name);
    match CROPS.iter().find(|(c, _)| *c == short) {
        Some((c, _)) => Ok(format!("minecraft:{c}")),
        None => bail!("circuit: harvester {name:?}: the crops judged so far are {}", CROPS.iter().map(|c| c.0).collect::<Vec<_>>().join(", ")),
    }
}

fn planted_state(crop: &str) -> &'static str {
    let short = crop.strip_prefix("minecraft:").unwrap_or(crop);
    CROPS.iter().find(|(c, _)| *c == short).map(|c| c.1).unwrap_or("minecraft:sugar_cane[age=0]")
}

/// The tallest a crop grows by itself (sugar cane: 3; bamboo: 16, though a stalk may stop earlier:
/// its top block then reads `stage=1`).
fn full_height(crop: &str) -> u32 {
    if crop.ends_with("bamboo") {
        16
    } else if crop.ends_with("kelp") {
        26
    } else if is_stem_crop(crop) {
        2
    } else {
        3
    }
}

/// Whether growths are counted by age resets (sugar cane: only the top block ages, and a growth
/// restarts it at 0, so a block destroyed in the tick it grew still counts) or by height (bamboo:
/// no age counter; its harvesters fire only on the growth itself).
fn grows_by_age(crop: &str) -> bool {
    // kelp: a harvested stalk's new top takes a random age, so ages cannot count growths; stems:
    // a fruit is counted when it attaches
    !crop.ends_with("bamboo") && !crop.ends_with("kelp") && !is_stem_crop(crop)
}

/// Whether the column has stopped growing by itself: at full height, (bamboo) its top block reads
/// `stage=1`, or (kelp) its top is at age 25 or has no water (still or flowing) above it.
fn stopped(frame: &mctas::sim::Region, p: Pos, h: u32, crop: &str) -> bool {
    let top = frame.get([p[0], p[1] + h as i32 - 1, p[2]]);
    h >= full_height(crop)
        || (h >= 2 && crop.ends_with("bamboo") && top.is_some_and(|s| s.contains("stage=1")))
        || (h >= 1 && crop.ends_with("kelp") && (top.is_some_and(|s| s.contains("age=25")) || frame.get([p[0], p[1] + h as i32, p[2]]).is_some_and(|s| !s.starts_with("minecraft:water"))))
}

/// A water source block (what kelp grows into).
pub fn is_water_source(s: &str) -> bool {
    s == "minecraft:water" || s == "minecraft:water[level=0]"
}

/// Whether a block state is part of the crop's column (kelp: its top `kelp` and the `kelp_plant`
/// below it).
fn is_crop_block(s: &str, crop: &str) -> bool {
    let is = |id: &str| s == id || s.strip_prefix(id).is_some_and(|r| r.starts_with('['));
    is(crop) || (crop.ends_with("kelp") && is("minecraft:kelp_plant"))
}

/// One run's record.
#[derive(Clone, Debug, Default)]
pub struct RunLog {
    pub canes: usize,
    pub ticks: u32,
    pub rts: u32,
    /// Blocks that grew above the bottom ones.
    pub grown: u64,
    /// Blocks harvested (gone from above the bottom ones).
    pub harvested: u64,
    /// Items of the crop in the judge's chests at the end.
    pub delivered: u64,
    /// Items of the crop that the design's own containers hold at the end, minus at the start.
    pub held: i64,
    /// The design's containers, every kind: at the start and at the end.
    pub initial: BTreeMap<String, u64>,
    pub final_held: BTreeMap<String, u64>,
    /// Entities alive at the end that the design did not declare (the intruders aside).
    pub loose: Vec<String>,
    /// The intruders: loose at the end, and in the judge's chests; the cells they were dropped on.
    pub intruders_loose: u64,
    pub intruders_delivered: u64,
    pub drop_cells: usize,
    /// A crop whose bottom block broke (tick, position).
    pub lost: Option<String>,
    /// A cane left at full height longer than [`STUCK`].
    pub stuck: Option<String>,
    /// The longest a grown block waited for its harvest (ticks), and the tallest a cane got.
    pub wait_max: u32,
    pub height_max: u32,
    /// The longest a cane stood at full height.
    pub full_max: u32,
    pub restless: String,
    /// Crop items seen loose at two batch ends in a row (where items get stranded).
    pub stranded: Vec<String>,
    /// (melon) the drop stacks seen, and their items when first seen: what the harvests gave.
    pub drops_seen: u64,
    pub drop_items: u64,
    /// (`--trace`) the tick-by-tick path of the first crop items seen off their way.
    pub trails: Vec<String>,
    /// Entities seen leaving the build's box (the first per batch).
    pub escaped: Vec<String>,
    pub dims: [i32; 3],
    pub notes: Vec<String>,
}

/// Run a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, rts: u32, ticks: u32, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let (mut from, mut to) = crate::run::watch_box(c, margin);
    // room for the crop to grow
    to[1] = to[1].max(c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos[1] + 3).max().unwrap_or(to[1]));
    from[1] = from[1].min(to[1] - 2);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, rts, ticks, from, to);
    let _ = w.drop_sandbox(sb);
    r
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

/// The crop column's height: crop blocks from `p` up.
fn height(frame: &mctas::sim::Region, p: Pos, crop: &str) -> u32 {
    if is_stem_crop(crop) {
        // 1 for the stem, 2 while a fruit is attached (an attached stem faces its fruit)
        let name = crop.trim_start_matches("minecraft:");
        return match frame.get(p) {
            Some(s) if s.starts_with(&format!("minecraft:{name}_stem")) => 1,
            Some(s) if s.starts_with(&format!("minecraft:attached_{name}_stem")) => 2,
            _ => 0,
        };
    }
    let mut h = 0;
    while frame.get([p[0], p[1] + h as i32, p[2]]).is_some_and(|s| is_crop_block(s, crop)) {
        h += 1;
    }
    h
}

/// Per crop column, what the judge follows tick by tick.
struct Column {
    pos: Pos,
    h: u32,
    /// Each standing block's age, bottom first (a growth resets the grower's age to 0).
    ages: Vec<u8>,
    /// The height before the run.
    h0: u32,
    /// The tick each block above the bottom one grew (bottom first).
    grew: Vec<u32>,
    full_since: Option<u32>,
}

/// The ages of the column's standing crop blocks, bottom first.
fn ages_of(frame: &mctas::sim::Region, p: Pos, h: u32) -> Vec<u8> {
    (0..h).map(|i| frame.get([p[0], p[1] + i as i32, p[2]]).and_then(|s| s.split_once("age=")).and_then(|(_, r)| r.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|a| a.parse::<u8>().ok()).unwrap_or(0)).collect()
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, rts: u32, ticks: u32, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    let Some(Kind::Harvester(crop)) = &c.kind else { bail!("{}: not a harvester", c.name()) };
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let crops: Vec<Pos> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    let judge: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let chests: Vec<(Pos, String)> = judge.iter().map(|p| (*p, CHEST.to_string())).collect();
    w.set_blocks(sb, &chests, PlaceMode::Raw)?;
    w.exec(sb, "gamerule random_tick_speed 0")?;
    let planted: Vec<(Pos, String)> = crops.iter().map(|p| (*p, planted_state(crop).to_string())).collect();
    w.set_blocks(sb, &planted, PlaceMode::Update)?;
    w.step(sb, c.clock.free_from() + SETTLE)?;
    // the record box: every crop column, full height
    let full = full_height(crop);
    let rf = [crops.iter().map(|p| p[0]).min().unwrap(), crops.iter().map(|p| p[1]).min().unwrap(), crops.iter().map(|p| p[2]).min().unwrap()];
    let rt = [crops.iter().map(|p| p[0]).max().unwrap(), crops.iter().map(|p| p[1]).max().unwrap() + full as i32, crops.iter().map(|p| p[2]).max().unwrap()];
    let start = w.get_region(sb, rf, rt)?;
    if let Some(p) = crops.iter().find(|p| height(&start, **p, crop) == 0) {
        let why = if crop.ends_with("kelp") {
            "kelp needs a water source at the crop cell, water above it and a sturdy block or kelp below it"
        } else if crop.ends_with("bamboo") {
            "bamboo needs dirt, mud, sand or gravel below it"
        } else if is_stem_crop(crop) {
            "a pumpkin or melon stem needs farmland below it"
        } else if crop.ends_with("cactus") {
            "cactus needs sand below it and nothing solid beside it"
        } else {
            "sugar cane needs sand, dirt or grass below it with water beside that block"
        };
        return Ok(Err(format!("the crop planted at {p:?} does not survive ({why})")));
    }
    let mut cols: Vec<Column> = crops.iter().map(|p| {
        let h = height(&start, *p, crop);
        Column { pos: *p, h, ages: ages_of(&start, *p, h), h0: h, grew: Vec::new(), full_since: None }
    }).collect();
    let mut log = RunLog { canes: crops.len(), ticks, rts, dims: shulker::design_dims(c), initial: held(w, sb, from, to, &judge)?, ..Default::default() };
    for col in &cols {
        if col.h > 1 {
            log.notes.push(format!("the crop at {:?} stands {} high before the run", col.pos, col.h));
        }
    }
    w.exec(sb, &format!("gamerule random_tick_speed {rts}"))?;
    let mut t = 0u32;
    let mut seen_items: Vec<String> = Vec::new();
    // an entity leaving the build's box (an item falling off it: lost) is noted
    let watch = mctas::sim::EntityWatch { watch: (from, to), walk: ([from[0], from[1] - 64, from[2]], [from[0], from[1] - 64, from[2]]) };
    // the run, then the rest (growth stopped) while the last harvests come in
    let rest_ticks = 2 * REST;
    let trace = TRACE.load(std::sync::atomic::Ordering::Relaxed);
    // a melon breaks into 3 to 7 slices (one stack), and a stem's fruit may be broken within the tick
    // it grew (another stem's observer firing the shared piston): every crop item stack is seen once,
    // tick by tick, to know what the harvests gave (the loss check is exact when every drop was seen)
    let count_drops = crop_item(crop) != crop || is_stem_crop(crop);
    let mut ever: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // (trace) one tick per batch, a wider record box: every crop item's path, tick by tick
    let batch = if trace || count_drops { 1 } else { BATCH };
    let (qf, qt) = if trace { ([rf[0] - 2, rf[1] - 2, rf[2] - 2], [rt[0] + 2, rt[1] + 1, rt[2] + 2]) } else { (rf, rt) };
    let mut trails: BTreeMap<String, Vec<TrailPoint>> = BTreeMap::new();
    let mut reported: Vec<String> = Vec::new();
    let mut column_since: BTreeMap<String, u32> = BTreeMap::new();
    // item intrusion: one of the drops at a quarter, half and three quarters of the run, on every
    // open hopper intake (crate::intrusion) and above every crop column (items falling onto the farm)
    let intakes = crate::intrusion::intakes(c);
    let drop_at: Vec<u32> = (1..=3).map(|k| (ticks * k / 4 / batch) * batch).collect();
    let drop_cells: Vec<[f64; 3]> = intakes.iter().map(|p| [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5]).chain(crops.iter().map(|p| [p[0] as f64 + 0.5, (p[1] + full as i32 + 1) as f64, p[2] as f64 + 0.5])).collect();
    log.drop_cells = drop_cells.len();
    // (trace) the cells an item may be in on its way: the crop columns (a stem's: with the four
    // fruit cells beside it) and above the hoppers
    let hoppers: Vec<Pos> = c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:hopper")).map(|(p, _)| *p).collect();
    let reach = if is_stem_crop(crop) { 1.2 } else { 0.2 };
    let on_way = |p: [f64; 3]| {
        crops.iter().any(|q| p[0] >= q[0] as f64 - reach && p[0] <= q[0] as f64 + 1.0 + reach && p[2] >= q[2] as f64 - reach && p[2] <= q[2] as f64 + 1.0 + reach && p[1] >= q[1] as f64 - 0.2)
            || hoppers.iter().any(|h| p[0] >= h[0] as f64 - 0.2 && p[0] <= h[0] as f64 + 1.2 && p[2] >= h[2] as f64 - 0.2 && p[2] <= h[2] as f64 + 1.2 && p[1] >= h[1] as f64 + 0.5 && p[1] <= h[1] as f64 + 3.2)
    };
    while t < ticks + rest_ticks {
        if t == ticks {
            w.exec(sb, "gamerule random_tick_speed 0")?;
        }
        if let Some(k) = drop_at.iter().position(|d| *d == t && t > 0) {
            // the bridge's summon (clk-misc 2026-10-21, w86 agreed): each item's own random is
            // seeded (`moveTowardsClosestSpace` draws from it when a piston pushes the item into a
            // block), so a run repeats for its seed and the redsim twin follows it ([`sim_run`]);
            // vanilla's summon command left it unseeded
            w.summon(sb, &intruder_specs(k, &drop_cells))?;
        }
        let n = batch.min(if t < ticks { ticks - t } else { ticks + rest_ticks - t });
        let r = w.step_record_watch(sb, n, false, qf, qt, &watch)?;
        if let Some(e) = &r.entity_escape
            && log.escaped.len() < 4
        {
            log.escaped.push(format!("batch from tick {t}: {e}"));
        }
        for frame in &r.frames {
            t += 1;
            follow(&mut cols, frame, crop, t, &mut log);
        }
        if log.lost.is_some() {
            column_notes(w, sb, &cols, t, &mut log)?;
            break;
        }
        if t == ticks && log.grown == 0 {
            column_notes(w, sb, &cols, t, &mut log)?;
        }
        // stranded items: a crop item still loose one batch later (a note: where items get lost)
        let mut now_items: Vec<String> = Vec::new();
        for e in w.entities(sb)? {
            if e.design || shulker::stack_of_entity(&e.state).is_none_or(|s| s.id != crop_item(crop)) {
                continue;
            }
            // (melon) every drop stack's count when first seen: what the harvest gave
            if count_drops && !ever.contains(&e.uuid) {
                ever.insert(e.uuid.clone());
                log.drops_seen += 1;
                log.drop_items += shulker::stack_of_entity(&e.state).map(|s| s.count as u64).unwrap_or(0);
            }
            if !trace && seen_items.contains(&e.uuid) && log.stranded.len() < 6 {
                let under = [e.pos[0].floor() as i32, (e.pos[1] - 0.01).floor() as i32, e.pos[2].floor() as i32];
                let what = w.get_region(sb, under, under).ok().and_then(|r| r.get(under).map(|s| clock::short(s).to_string())).unwrap_or_default();
                log.stranded.push(format!("tick {t}: an item at {:.2?} on {what} at {under:?} (motion {:.2?})", e.pos, e.motion));
            }
            if trace {
                let frame = r.frames.last();
                let cell = |p: [f64; 3]| -> String {
                    let q = [p[0].floor() as i32, p[1].floor() as i32, p[2].floor() as i32];
                    frame.and_then(|f| f.get(q)).map(|s| clock::short(s).to_string()).unwrap_or_else(|| "?".into())
                };
                let pt = TrailPoint { t0: t, t1: t, pos: e.pos, motion: e.motion, inside: cell(e.pos), on: cell([e.pos[0], e.pos[1] - 0.01, e.pos[2]]), column: column_letters(frame, &crops, e.pos, full) };
                let tr = trails.entry(e.uuid.clone()).or_default();
                match tr.last_mut() {
                    Some(last) if last.pos == pt.pos && last.inside == pt.inside && last.on == pt.on && last.column == pt.column => last.t1 = t,
                    _ => {
                        tr.push(pt);
                        if tr.len() > TRAIL {
                            tr.remove(0);
                        }
                    }
                }
                // an item resting on a crop (above its soil) for 5 ticks: only the next growth clears it
                let in_column = crops.iter().any(|q| e.pos[0] >= q[0] as f64 && e.pos[0] < q[0] as f64 + 1.0 && e.pos[2] >= q[2] as f64 && e.pos[2] < q[2] as f64 + 1.0 && e.pos[1] >= q[1] as f64 + 0.99);
                let since = *column_since.entry(e.uuid.clone()).or_insert(t);
                if !in_column {
                    column_since.insert(e.uuid.clone(), t);
                }
                // an item resting on a crop for 5 ticks, or lingering above it for 40 (afloat)
                let on_crop = in_column && (tr.last().is_some_and(|p| p.t1 >= p.t0 + 4 && p.motion.iter().all(|m| m.abs() < 1e-9)) || t >= since + 40);
                if (!on_way(e.pos) || on_crop) && !reported.contains(&e.uuid) && reported.len() < 4 {
                    reported.push(e.uuid.clone());
                    let what = if on_crop { "resting on a crop" } else { "off its way" };
                    log.trails.push(format!("a crop item {what} at tick {t}; its last {} moves (column: the nearest crop, from under its soil up):", tr.len()));
                    log.trails.extend(tr.iter().map(TrailPoint::line));
                }
            }
            now_items.push(e.uuid.clone());
        }
        if trace {
            // a crop item gone while inside a crop column (killed there: a cactus) is reported
            for (uuid, tr) in trails.iter() {
                if now_items.contains(uuid) || reported.contains(uuid) || reported.len() >= 4 {
                    continue;
                }
                let Some(last) = tr.last() else { continue };
                let inside = crops.iter().any(|q| last.pos[0] >= q[0] as f64 - 0.125 && last.pos[0] < q[0] as f64 + 1.125 && last.pos[2] >= q[2] as f64 - 0.125 && last.pos[2] < q[2] as f64 + 1.125 && last.pos[1] < q[1] as f64 + full as f64);
                if inside {
                    reported.push(uuid.clone());
                    log.trails.push(format!("a crop item gone inside a crop column at tick {t}; its last {} moves:", tr.len()));
                    log.trails.extend(tr.iter().map(TrailPoint::line));
                }
            }
            trails.retain(|k, _| now_items.contains(k));
            column_since.retain(|k, _| now_items.contains(k));
        }
        seen_items = now_items;
    }
    w.exec(sb, "gamerule random_tick_speed 0")?;
    finish(w, sb, c, crop, from, to, &judge, &mut log)?;
    Ok(Ok(log))
}

/// (`--trace`) points kept per crop item path (a resting item's ticks fold into one point).
const TRAIL: usize = 160;

/// (`--trace`) one point of a crop item's path: from tick t0 to t1 at `pos` with `motion`, the
/// block its center is in and the block under it.
struct TrailPoint {
    t0: u32,
    t1: u32,
    pos: [f64; 3],
    motion: [f64; 3],
    inside: String,
    on: String,
    /// The nearest crop's column from its soil up, one letter a cell.
    column: String,
}

/// (`--trace`) the column of the crop nearest to `p`, from its soil up to `full` + 1 blocks: `b`
/// the crop, `m` a moving piston, `h` a piston head, `.` air, `#` anything else.
fn column_letters(frame: Option<&mctas::sim::Region>, crops: &[Pos], p: [f64; 3], full: u32) -> String {
    let d = |q: &Pos| (q[0] as f64 + 0.5 - p[0]).powi(2) + (q[2] as f64 + 0.5 - p[2]).powi(2);
    let Some(q) = crops.iter().min_by(|a, b| d(a).total_cmp(&d(b))) else { return String::new() };
    let top = (full + 1).min(5) as i32;
    (-1..=top)
        .map(|k| match frame.and_then(|f| f.get([q[0], q[1] + k, q[2]])) {
            None => '?',
            Some(s) if s.contains("moving_piston") => 'm',
            Some(s) if s.contains("piston_head") => 'h',
            Some(s) if mctas::blockstate::is_air(s) => '.',
            Some(s) if s.contains("bamboo") || s.contains("sugar_cane") || s.contains("kelp") || s.contains("_stem") || s.contains("cactus") => 'b',
            Some(s) if s == "minecraft:pumpkin" || s == "minecraft:melon" => 'f',
            Some(_) => '#',
        })
        .collect()
}

impl TrailPoint {
    fn line(&self) -> String {
        let ticks = if self.t1 > self.t0 { format!("{}-{}", self.t0, self.t1) } else { self.t0.to_string() };
        let [x, y, z] = self.pos;
        let [u, v, s] = self.motion;
        format!("  tick {ticks}: at [{x:.3}, {y:.3}, {z:.3}] motion [{u:.3}, {v:.3}, {s:.3}] in {} on {}, column {}", self.inside, self.on, self.column)
    }
}

/// Why a crop broke or never grew: each column and its neighbours as they are now (a note).
fn column_notes(w: &mut Worker, sb: u32, cols: &[Column], t: u32, log: &mut RunLog) -> Result<()> {
    for col in cols.iter().take(2) {
        let p = col.pos;
        let now = w.get_region(sb, [p[0] - 1, p[1] - 1, p[2] - 1], [p[0] + 1, p[1] + 2, p[2] + 1])?;
        let mut cells: Vec<String> = Vec::new();
        for (q, s) in now.iter() {
            if !mctas::blockstate::is_air(s) {
                cells.push(format!("{:?} {}", [q[0] - p[0], q[1] - p[1], q[2] - p[2]], clock::short(s)));
            }
        }
        log.notes.push(format!("after tick {t}: around the crop at {p:?} (relative): {}", cells.join(", ")));
    }
    Ok(())
}

/// One recorded tick: growths, harvests, waits, full-height time.
fn follow(cols: &mut [Column], frame: &mctas::sim::Region, crop: &str, t: u32, log: &mut RunLog) {
    for col in cols.iter_mut() {
        let h = height(frame, col.pos, crop);
        let ages = ages_of(frame, col.pos, h);
        if h != col.h && log.notes.len() < 6 {
            let s: Vec<String> = (0..4).map(|i| frame.get([col.pos[0], col.pos[1] + i - 1, col.pos[2]]).map(|s| clock::short(s).to_string()).unwrap_or_else(|| "-".into())).collect();
            log.notes.push(format!("tick {t}: the crop at {:?} {} -> {} ({})", col.pos, col.h, h, s.join(" / ")));
        }
        if h == 0 && log.lost.is_none() {
            log.lost = Some(format!("tick {t}: the crop at {:?} broke (its bottom block)", col.pos));
        }
        // a growth: a standing block's age dropped (the grower restarts at 0), seen even when the
        // new block is destroyed within the same tick (sugar cane); bamboo: the height grew
        if grows_by_age(crop) {
            for j in 0..(h.min(col.h) as usize) {
                if ages[j] < col.ages[j] {
                    log.grown += 1;
                }
            }
        } else if h > col.h && col.h > 0 {
            log.grown += (h - col.h) as u64;
        }
        if h > col.h {
            for _ in col.h..h {
                col.grew.push(t);
            }
            log.height_max = log.height_max.max(h);
        } else if h < col.h {
            let gone = col.h - h.max(1);
            for _ in 0..gone {
                if let Some(g) = col.grew.pop() {
                    log.wait_max = log.wait_max.max(t - g);
                }
            }
            // the blocks below the harvested ones keep their growth ticks
            col.grew.truncate(h.saturating_sub(1) as usize);
        }
        if stopped(frame, col.pos, h, crop) {
            let since = *col.full_since.get_or_insert(t);
            log.full_max = log.full_max.max(t - since);
            if t - since > STUCK && log.stuck.is_none() {
                log.stuck = Some(format!("the crop at {:?} has stopped growing {h} high since tick {since}", col.pos));
            }
        } else {
            col.full_since = None;
        }
        col.h = h;
        col.ages = ages;
    }
    // harvested: every growth minus what still stands above the start
    let standing: i64 = cols.iter().map(|c| c.h as i64 - c.h0 as i64).sum();
    log.harvested = (log.grown as i64 - standing).max(0) as u64;
}

/// Rest until nothing moves (up to 5 x [`REST`]), then count what is where.
#[allow(clippy::too_many_arguments)]
fn finish(w: &mut Worker, sb: u32, c: &Circuit, crop: &str, from: Pos, to: Pos, judge: &[Pos], log: &mut RunLog) -> Result<()> {
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
        log.restless = format!("blocks still change {} gt after growth stopped ({p})", 5 * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.kind), x.pos)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after growth stopped ({p})", 5 * REST);
    } else if status.block_ticks + status.block_events + status.moving_pistons > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after growth stopped", status.block_ticks, status.block_events, status.moving_pistons, 5 * REST);
    }
    for be in &b.1 {
        if judge.contains(&be.pos) {
            log.delivered += stacks_of_be(&be.nbt).iter().filter(|s| s.id == crop_item(crop)).map(|s| s.count as u64).sum::<u64>();
        }
    }
    log.final_held = held(w, sb, from, to, judge)?;
    let had = log.initial.get(crop_item(crop)).copied().unwrap_or(0) as i64;
    log.held = log.final_held.get(crop_item(crop)).copied().unwrap_or(0) as i64 - had;
    for e in w.entities(sb)? {
        // the intruders (design entities since they are summoned by the bridge) may end anywhere
        if let Some(s) = shulker::stack_of_entity(&e.state)
            && INTRUDERS.iter().any(|(id, _)| *id == s.id)
        {
            log.intruders_loose += s.count as u64;
            continue;
        }
        if e.design {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            Some(s) => log.loose.push(format!("{} x{} at {:.1?}", clock::short(&s.id), s.count, e.pos)),
            None => log.loose.push(format!("{} at {:.1?}", clock::short(&e.kind), e.pos)),
        }
    }
    for be in &b.1 {
        if judge.contains(&be.pos) {
            log.intruders_delivered += stacks_of_be(&be.nbt).iter().filter(|s| INTRUDERS.iter().any(|(id, _)| *id == s.id)).map(|s| s.count as u64).sum::<u64>();
        }
    }
    let _ = c;
    Ok(())
}

/// Items per hour per cane at vanilla speed.
pub fn per_hour(log: &RunLog) -> f64 {
    if log.canes == 0 || log.ticks == 0 {
        return 0.0;
    }
    log.delivered as f64 / log.canes as f64 / (log.ticks as f64 / HOUR) * VANILLA_RTS as f64 / log.rts.max(1) as f64
}

/// The crop a harvester design names (`minecraft:sugar_cane`...), empty for another kind.
fn crop_of(c: &Circuit) -> String {
    match &c.kind {
        Some(Kind::Harvester(crop)) => crop.clone(),
        _ => String::new(),
    }
}

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(c: &Circuit, log: &RunLog) -> Outcome {
    let blocks = c.blocks();
    let crop = crop_of(c);
    let what = if is_stem_crop(&crop) { "fruits" } else { "blocks" };
    let gave = if log.drops_seen > 0 { format!(" ({} drops seen, {} items)", log.drops_seen, log.drop_items) } else { String::new() };
    let mut table = vec![
        format!("  {} canes, {} ticks at random_tick_speed {}: {} {what} grew, {} harvested{gave}, {} items in the judge's chests", log.canes, log.ticks, log.rts, log.grown, log.harvested, log.delivered),
        format!("  tallest cane {}, longest wait from growing to harvest {} gt, longest at full height {} gt", log.height_max, log.wait_max, log.full_max),
        format!("  intruders ({} dropped on {} cells): {} in the judge's chests, {} loose", log.drop_cells as u64 * INTRUDERS.iter().map(|x| x.1 as u64).sum::<u64>(), log.drop_cells, log.intruders_delivered, log.intruders_loose),
    ];
    for n in &log.notes {
        table.push(format!("  note: {n}"));
    }
    for n in &log.stranded {
        table.push(format!("  stranded: {n}"));
    }
    for n in &log.trails {
        table.push(format!("  trail: {n}"));
    }
    for n in &log.escaped {
        table.push(format!("  escaped: {n}"));
    }
    let mut why: Vec<String> = Vec::new();
    if let Some(l) = &log.lost {
        why.push(l.clone());
    }
    if let Some(s) = &log.stuck {
        why.push(format!("a missed harvest: {s}"));
    }
    if log.harvested == 0 {
        why.push("nothing was harvested".into());
    }
    // what the harvests gave: a block's item each, or (stem crops) the drop stacks seen tick by tick:
    // one per fruit, so more drops than fruits seen grow means fruits broken within the tick they
    // grew, and fewer means drops taken in the tick they appeared (not countable)
    let gave = if is_stem_crop(&crop) {
        if log.drops_seen < log.harvested {
            why.push(format!("{} fruits were harvested but {} drops were seen: a drop taken in the tick it appeared (a container reaching the fruit's cell) cannot be counted, so the loss check would not be exact", log.harvested, log.drops_seen));
        } else if log.drops_seen > log.harvested {
            table.push(format!("  note: {} fruits were broken within the tick they grew (their drops are counted)", log.drops_seen - log.harvested));
        }
        log.drop_items
    } else {
        log.harvested
    };
    let missing = gave as i64 - log.delivered as i64;
    if missing != 0 {
        why.push(format!("{} harvested items are not in the judge's chests ({} left in the design's containers, {} loose)", missing, log.held, log.loose.len()));
    }
    if log.held != 0 && missing == 0 {
        why.push(format!("the design's containers hold {} more items of the crop than at the start", log.held));
    }
    let other: Vec<&String> = log.loose.iter().collect();
    if !other.is_empty() {
        why.push(format!("left loose: {}", other.iter().take(4).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
    }
    if !log.restless.is_empty() {
        why.push(log.restless.clone());
    }
    let per_cane = blocks as f64 / log.canes.max(1) as f64;
    let per_cane_text = if (blocks * 10) % log.canes.max(1) == 0 { format!("{per_cane:.1}") } else { format!("{per_cane:.2}") };
    let mut metrics = vec![
        ("per cane".to_string(), format!("{per_cane_text} blocks")),
        ("items per hour per cane".to_string(), format!("{:.2}", per_hour(log))),
        ("harvest height".to_string(), log.height_max.to_string()),
        ("wait".to_string(), format!("{} gt", log.wait_max)),
    ];
    // a stem's fruit grows beside it: no harvest height
    if is_stem_crop(&crop_of(c)) {
        metrics.retain(|m| m.0 != "harvest height");
    }
    if why.is_empty() {
        Outcome { pass: true, why: String::new(), metrics, table }
    } else {
        Outcome { pass: false, why: why.join("; "), metrics, table }
    }
}

/// The judge for the harvester kind (`mctas circuit verify`): the run at N positions x 4
/// rotations, then (a `tile:` header) the copies side by side at 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    if !matches!(kind, Kind::Harvester(_)) {
        bail!("{}: not a harvester", kind.name());
    }
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let family = c.kinds().into_iter().collect::<Vec<_>>().join("+");
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = game_run(w, &rc, RTS, RUN, o.margin)?;
        Ok(r.map(|log| BuildOk { outcome: judge(&rc, &log), dims: log.dims, family: family.clone() }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        builds.push(Build { turns: t, offset: off, result: r.with_context(|| format!("rotation {t} at {off:?}"))? });
    }
    let mut schedule = Schedule::new(0);
    schedule.ticks = RUN;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind.clone()), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    // the builds' mean rate and worst wait
    let metric = |k: &BuildOk, name: &str| k.outcome.metrics.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok());
    let oks: Vec<&BuildOk> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok()).filter(|k| k.outcome.pass).collect();
    let rates: Vec<f64> = oks.iter().filter_map(|k| metric(k, "items per hour per cane")).collect();
    let mean = if rates.is_empty() { 0.0 } else { rates.iter().sum::<f64>() / rates.len() as f64 };
    let worst = oks.iter().filter_map(|k| metric(k, "wait")).fold(0.0f64, f64::max) as u32;
    let tallest = oks.iter().filter_map(|k| metric(k, "harvest height")).fold(0.0f64, f64::max) as u32;
    let mut line = String::new();
    if c.tile.is_some() {
        line = match tiled_run(pool, c, o) {
            Ok(per) => format!("tileable: yes ({per})"),
            Err(why) => {
                rep.pass = false;
                rep.verdict = format!("FAIL: tiled: {why}");
                format!("tileable: no ({why})")
            }
        };
    }
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        // the rate and the wait follow the random growth: the builds' mean rate, the worst wait
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "items per hour per cane") {
            m.1 = format!("{mean:.2}");
        }
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "wait") {
            m.1 = format!("{worst} gt");
        }
        if let Some(m) = k.outcome.metrics.iter_mut().find(|(n, _)| n == "harvest height") {
            m.1 = tallest.to_string();
        }
        k.outcome.metrics.push(("bill".to_string(), shulker::bill(c)));
        if !line.is_empty() {
            k.outcome.table.push(format!("  {line}"));
        }
    }
    Ok(rep)
}

/// The copies side by side ([`crate::tiled`]) at 4 rotations: Ok(the copies' per-hour line) or
/// Err(why) at the first failure.
fn tiled_run(pool: &Pool, c: &Circuit, o: &Options) -> std::result::Result<String, String> {
    let tc = crate::tiled(c).map_err(|e| format!("{e:#}"))?;
    let jobs: Vec<u8> = vec![0, 1, 2, 3];
    let results = pool.map(jobs.clone(), |w, _, t| {
        let rc = tc.rotated(w, t)?;
        Ok((game_run(w, &rc, RTS, RUN, o.margin)?, rc))
    });
    let mut line = String::new();
    for (t, r) in jobs.into_iter().zip(results) {
        match r {
            Ok((Ok(log), rc)) => {
                let out = judge(&rc, &log);
                if !out.pass {
                    let extra: Vec<String> = log.stranded.iter().chain(log.escaped.iter()).take(4).cloned().collect();
                    return Err(format!("rotation {t}: {}{}", out.why, if extra.is_empty() { String::new() } else { format!(" (stranded: {})", extra.join("; ")) }));
                }
                if t == 0 {
                    line = format!("{} copies, {:.2} items per hour per cane", log.canes, per_hour(&log));
                }
            }
            Ok((Err(why), _)) => return Err(format!("rotation {t}: {why}")),
            Err(e) => return Err(format!("rotation {t}: {e:#}")),
        }
    }
    Ok(line)
}

/// A design aid (`mctas circuit farm-run`): one run (`--tiled`: the copies) turned and moved like
/// one of `circuit verify`'s builds, its table printed.
#[allow(clippy::too_many_arguments)]
pub fn run_one(pool: &Pool, c: &Circuit, tiled: bool, turns: u8, offset: Pos, rts: u32, ticks: u32, margin: i32) -> Result<Outcome> {
    let base = if tiled { crate::tiled(c)? } else { c.clone() };
    let r = pool.map(vec![0u8], |w, _, _| {
        let rc = base.rotated(w, turns)?.translated(offset)?;
        Ok((game_run(w, &rc, rts, ticks, margin)?, rc))
    });
    match r.into_iter().next().ok_or_else(|| anyhow!("no run"))?? {
        (Ok(log), rc) => Ok(judge(&rc, &log)),
        (Err(why), _) => Ok(Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() }),
    }
}

/// The intruders of drop round `k` (`INTRUDERS[k]` on every drop cell), as the bridge summons them.
fn intruder_specs(k: usize, cells: &[[f64; 3]]) -> Vec<mctas::design::EntitySpec> {
    let (id, n) = INTRUDERS[k];
    cells.iter().map(|q| mctas::design::EntitySpec { kind: "minecraft:item".into(), pos: *q, nbt: Some(format!("{{Item:{{id:\"{id}\",count:{n}}}}}")) }).collect()
}

// ------------------------------------------------------------------------ the redsim twin

/// The farm judge's run on redsim (clk-misc, 2026-10-21; coordinator: "a redsim twin for the farm
/// judge so w86's searches can run in redsim"): [`game_run`] step for step on redsim's world
/// (the same box, seed and chunks; the judge's placement, the chests placed raw, the crop planted
/// with updates, the settle, `random_tick_speed` set as the judge sets it, the intruders summoned
/// as the bridge summons them, the batches, the rest), the game's random ticks mirrored draw for
/// draw (`redsim::crops`), so a build's twin run IS the game run of that build and seed
/// (`redsim-fuzz trace-compare` on `trace-export --farm` traces). The same [`RunLog`], judged
/// by [`judge`]. Err: what redsim cannot build or run.
pub fn sim_run(c: &Circuit, rts: u32, ticks: u32, margin: i32) -> std::result::Result<RunLog, String> {
    use redsim::world::flags;
    use redsim::{Block, BlockPos, World};
    let Some(Kind::Harvester(crop)) = &c.kind else { return Err(format!("{}: not a harvester", c.name())) };
    let crop = crop.clone();
    let bp = |p: Pos| BlockPos::new(p[0], p[1], p[2]);
    let (mut from, mut to) = crate::run::watch_box(c, margin);
    to[1] = to[1].max(c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos[1] + 3).max().unwrap_or(to[1]));
    from[1] = from[1].min(to[1] - 2);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let seed = shulker::build_seed(from, to, 0);
    let mut w = World::new(bp(from), [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1]);
    w.mirror_level_random(seed, chunks);
    w.drop_seed = Some(seed);
    w.random_ticks = Some(redsim::crops::RandomTicks::new(3, seed));
    let parse = |s: &str| Block::parse(s).map_err(|e| format!("not modeled by redsim: {e}"));
    // the judge's placement (clock::verify::place: raw writes, the block entities' data, the
    // wires' shapes and a self update of every block), then the design's entities
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for (p, s) in c.clock.design.block_list() {
        blocks.push((bp(p), parse(&s)?));
    }
    w.place_raw(&blocks);
    let be: Vec<(BlockPos, &str)> = c.clock.design.block_entities.iter().map(|(p, s)| (bp(*p), s.as_str())).collect();
    redsim::redstone::container::apply_block_entities(&mut w, be).map_err(|e| format!("nbt: {e}"))?;
    w.place_updates(&blocks);
    for e in &c.clock.design.entities {
        redsim::entity::spawn(&mut w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| format!("not modeled by redsim: {why}"))?;
    }
    let popped = c.clock.design.blocks.keys().find(|p| w.get(bp(**p)).is_air());
    let floating = blocks.iter().find(|(p, s)| !w.get(*p).is_air() && !redsim::behavior::can_survive(&w, s, *p)).map(|(p, _)| [p.x, p.y, p.z]);
    if let Some(p) = popped.copied().or(floating) {
        let st = c.clock.design.blocks.get(&p).cloned().unwrap_or_default();
        return Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st)));
    }
    let crops: Vec<Pos> = c.stores.iter().filter(|s| s.role == "crop").map(|s| s.pos).collect();
    let judge_cells: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let chest = parse(CHEST)?;
    let chests: Vec<(BlockPos, Block)> = judge_cells.iter().map(|p| (bp(*p), chest)).collect();
    w.place_raw(&chests);
    let set_rts = |w: &mut World, n: u32| {
        if let Some(rt) = w.random_ticks.as_mut() {
            rt.speed = n;
        }
    };
    set_rts(&mut w, 0);
    let planted = parse(planted_state(&crop))?;
    for p in &crops {
        w.set_block(bp(*p), planted, flags::ALL);
    }
    for _ in 0..c.clock.free_from() + SETTLE {
        w.tick();
    }
    let full = full_height(&crop);
    let rf = [crops.iter().map(|p| p[0]).min().unwrap_or(0), crops.iter().map(|p| p[1]).min().unwrap_or(0), crops.iter().map(|p| p[2]).min().unwrap_or(0)];
    let rt = [crops.iter().map(|p| p[0]).max().unwrap_or(0), crops.iter().map(|p| p[1]).max().unwrap_or(0) + full as i32, crops.iter().map(|p| p[2]).max().unwrap_or(0)];
    let start = sim_region(&w, rf, rt);
    if let Some(p) = crops.iter().find(|p| height(&start, **p, &crop) == 0) {
        return Err(format!("the crop planted at {p:?} does not survive (sugar cane needs sand, dirt or grass below it with water beside that block)"));
    }
    let mut cols: Vec<Column> = crops
        .iter()
        .map(|p| {
            let h = height(&start, *p, &crop);
            Column { pos: *p, h, ages: ages_of(&start, *p, h), h0: h, grew: Vec::new(), full_since: None }
        })
        .collect();
    let mut log = RunLog { canes: crops.len(), ticks, rts, dims: shulker::design_dims(c), initial: sim_held(&w, from, to, &judge_cells), ..Default::default() };
    for col in &cols {
        if col.h > 1 {
            log.notes.push(format!("the crop at {:?} stands {} high before the run", col.pos, col.h));
        }
    }
    set_rts(&mut w, rts);
    let rest_ticks = 2 * REST;
    let intakes = crate::intrusion::intakes(c);
    let drop_at: Vec<u32> = (1..=3).map(|k| (ticks * k / 4 / BATCH) * BATCH).collect();
    let drop_cells: Vec<[f64; 3]> = intakes.iter().map(|p| [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5]).chain(crops.iter().map(|p| [p[0] as f64 + 0.5, (p[1] + full as i32 + 1) as f64, p[2] as f64 + 0.5])).collect();
    log.drop_cells = drop_cells.len();
    let outside = |p: redsim::entity::Vec3| p.x < from[0] as f64 || p.y < from[1] as f64 || p.z < from[2] as f64 || p.x > (to[0] + 1) as f64 || p.y > (to[1] + 1) as f64 || p.z > (to[2] + 1) as f64;
    let mut t = 0u32;
    let mut seen_items: Vec<u32> = Vec::new();
    while t < ticks + rest_ticks {
        if t == ticks {
            set_rts(&mut w, 0);
        }
        if let Some(k) = drop_at.iter().position(|d| *d == t && t > 0) {
            for s in intruder_specs(k, &drop_cells) {
                redsim::entity::spawn(&mut w, &s.kind, s.pos, s.nbt.as_deref()).map_err(|why| format!("an intruder: {why}"))?;
            }
        }
        let n = BATCH.min(if t < ticks { ticks - t } else { ticks + rest_ticks - t });
        let mut escape: Option<String> = None;
        for _ in 0..n {
            w.tick();
            if let Some(e) = w.errors.first() {
                return Err(format!("tick {}: {e}", t + 1));
            }
            t += 1;
            if escape.is_none()
                && let Some(e) = w.entities.list.iter().find(|e| !e.removed && outside(e.pos))
            {
                escape = Some(format!("{} at ({:.1}, {:.1}, {:.1})", sim_entity_name(e), e.pos.x, e.pos.y, e.pos.z));
            }
            follow(&mut cols, &sim_region(&w, rf, rt), &crop, t, &mut log);
        }
        if let Some(e) = escape
            && log.escaped.len() < 4
        {
            log.escaped.push(format!("batch from tick {}: {e}", t - n));
        }
        if log.lost.is_some() {
            sim_column_notes(&w, &cols, t, &mut log);
            break;
        }
        if t == ticks && log.grown == 0 {
            sim_column_notes(&w, &cols, t, &mut log);
        }
        // stranded items: a crop item still loose one batch later (a note)
        let mut now_items: Vec<u32> = Vec::new();
        for e in w.entities.list.iter().filter(|e| !e.removed && e.summon_index.is_none()) {
            let redsim::entity::EntityKind::Item(s) = &e.kind else { continue };
            if norm_id(&s.item) != crop_item(&crop) {
                continue;
            }
            if seen_items.contains(&e.id) && log.stranded.len() < 6 {
                let under = BlockPos::new(e.pos.x.floor() as i32, (e.pos.y - 0.01).floor() as i32, e.pos.z.floor() as i32);
                let what = clock::short(&w.get(under).to_string()).to_string();
                log.stranded.push(format!("tick {t}: an item at {:.2?} on {what} at {:?} (motion {:.2?})", [e.pos.x, e.pos.y, e.pos.z], [under.x, under.y, under.z], [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z]));
            }
            now_items.push(e.id);
        }
        seen_items = now_items;
    }
    set_rts(&mut w, 0);
    sim_finish(&mut w, &crop, from, to, &judge_cells, &mut log)?;
    Ok(log)
}

/// A stack's item id with the `minecraft:` prefix.
fn norm_id(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") }
}

/// The type of a redsim entity, as the game names it.
fn sim_entity_name(e: &redsim::entity::Entity) -> &'static str {
    use redsim::entity::EntityKind;
    match &e.kind {
        EntityKind::Boat(b) => {
            if b.chest {
                "minecraft:oak_chest_boat"
            } else {
                "minecraft:oak_boat"
            }
        }
        EntityKind::Minecart(m) => {
            if m.hopper {
                "minecraft:hopper_minecart"
            } else {
                "minecraft:minecart"
            }
        }
        EntityKind::Item(_) => "minecraft:item",
        EntityKind::ArmorStand(_) => "minecraft:armor_stand",
        EntityKind::PrimedTnt(_) => "minecraft:tnt",
    }
}

/// Redsim's cells `from..=to` as the game's `Region` (the states as the game writes them).
fn sim_region(w: &redsim::World, from: Pos, to: Pos) -> mctas::sim::Region {
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut palette: Vec<String> = Vec::new();
    let mut data: Vec<u16> = Vec::with_capacity((size[0] * size[1] * size[2]) as usize);
    for y in from[1]..=to[1] {
        for z in from[2]..=to[2] {
            for x in from[0]..=to[0] {
                let s = w.get(redsim::BlockPos::new(x, y, z)).to_string();
                let i = match palette.iter().position(|p| *p == s) {
                    Some(i) => i,
                    None => {
                        palette.push(s);
                        palette.len() - 1
                    }
                };
                data.push(i as u16);
            }
        }
    }
    mctas::sim::Region { from, size, palette, flags: Vec::new(), data }
}

/// The kinds and counts the design's own block containers hold (judge chests left out).
fn sim_held(w: &redsim::World, from: Pos, to: Pos, judge: &[Pos]) -> BTreeMap<String, u64> {
    let mut m = BTreeMap::new();
    for (p, c) in &w.containers.map {
        let q = [p.x, p.y, p.z];
        if judge.contains(&q) || (0..3).any(|i| q[i] < from[i] || q[i] > to[i]) {
            continue;
        }
        for s in c.slots.iter().flatten() {
            *m.entry(norm_id(redsim::redstone::container::item_name(s.item))).or_insert(0) += s.count as u64;
        }
    }
    m
}

/// The crop items and the intruders in the judge's chests.
fn sim_chest_counts(w: &redsim::World, judge: &[Pos], crop: &str) -> (u64, u64) {
    let (mut crops, mut intruders) = (0u64, 0u64);
    for p in judge {
        let Some(c) = w.containers.map.get(&redsim::BlockPos::new(p[0], p[1], p[2])) else { continue };
        for s in c.slots.iter().flatten() {
            let id = norm_id(redsim::redstone::container::item_name(s.item));
            if id == crop_item(crop) {
                crops += s.count as u64;
            } else if INTRUDERS.iter().any(|(i, _)| *i == id) {
                intruders += s.count as u64;
            }
        }
    }
    (crops, intruders)
}

/// [`column_notes`] on redsim.
fn sim_column_notes(w: &redsim::World, cols: &[Column], t: u32, log: &mut RunLog) {
    for col in cols.iter().take(2) {
        let p = col.pos;
        let now = sim_region(w, [p[0] - 1, p[1] - 1, p[2] - 1], [p[0] + 1, p[1] + 2, p[2] + 1]);
        let cells: Vec<String> = now.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(q, s)| format!("{:?} {}", [q[0] - p[0], q[1] - p[1], q[2] - p[2]], clock::short(s))).collect();
        log.notes.push(format!("after tick {t}: around the crop at {p:?} (relative): {}", cells.join(", ")));
    }
}

/// [`finish`] on redsim: the rest (up to 5 x REST ticks until nothing changes and nothing is
/// pending), then the counts.
fn sim_finish(w: &mut redsim::World, crop: &str, from: Pos, to: Pos, judge: &[Pos], log: &mut RunLog) -> std::result::Result<(), String> {
    // the block entities in the region's order (y, z, x), each with its block's name and contents
    let containers = |w: &redsim::World| -> Vec<(Pos, String, String)> {
        let mut v: Vec<(Pos, String, String)> = w
            .containers
            .map
            .iter()
            .filter(|(p, _)| (0..3).all(|i| [p.x, p.y, p.z][i] >= from[i] && [p.x, p.y, p.z][i] <= to[i]))
            .map(|(p, c)| {
                let name = w.get(*p).to_string();
                let kind = name.split('[').next().unwrap_or_default().to_string();
                ([p.x, p.y, p.z], kind, format!("{:?}", c.slots))
            })
            .collect();
        v.sort_by_key(|(p, _, _)| (p[1], p[2], p[0]));
        v
    };
    // the bridge's status: scheduled block ticks, block events, moving pistons (no fluid ticks)
    let pending = |w: &redsim::World| (w.ticks.count(), w.events.len(), w.piston.live_count());
    let mut a = (sim_region(w, from, to), containers(w));
    let mut b = a.clone();
    let mut status = pending(w);
    for _ in 0..5 {
        for _ in 0..REST {
            w.tick();
        }
        if let Some(e) = w.errors.first() {
            return Err(format!("the rest: {e}"));
        }
        b = (sim_region(w, from, to), containers(w));
        status = pending(w);
        if a == b && status.0 + status.1 + status.2 == 0 {
            break;
        }
        a = b.clone();
    }
    let changed = b.0.iter().find(|(p, s)| a.0.get(*p) != Some(*s)).map(|(p, s)| format!("{} at {p:?}", clock::short(s)));
    if let Some(p) = changed {
        log.restless = format!("blocks still change {} gt after growth stopped ({p})", 5 * REST);
    } else if a.1 != b.1 {
        let p = b.1.iter().find(|x| !a.1.contains(x)).map(|x| format!("{} at {:?}", clock::short(&x.1), x.0)).unwrap_or_default();
        log.restless = format!("container contents still change {} gt after growth stopped ({p})", 5 * REST);
    } else if status.0 + status.1 + status.2 > 0 {
        log.restless = format!("{} scheduled ticks, {} block events, {} moving pistons pending {} gt after growth stopped", status.0, status.1, status.2, 5 * REST);
    }
    let (delivered, intruders) = sim_chest_counts(w, judge, crop);
    log.delivered = delivered;
    log.intruders_delivered = intruders;
    log.final_held = sim_held(w, from, to, judge);
    let had = log.initial.get(crop_item(crop)).copied().unwrap_or(0) as i64;
    log.held = log.final_held.get(crop_item(crop)).copied().unwrap_or(0) as i64 - had;
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        if let redsim::entity::EntityKind::Item(s) = &e.kind {
            let id = norm_id(&s.item);
            if INTRUDERS.iter().any(|(i, _)| *i == id) {
                log.intruders_loose += s.count as u64;
                continue;
            }
            if e.summon_index.is_none() {
                log.loose.push(format!("{} x{} at {:.1?}", clock::short(&id), s.count, [e.pos.x, e.pos.y, e.pos.z]));
            }
            continue;
        }
        if e.summon_index.is_none() {
            log.loose.push(format!("{} at {:.1?}", clock::short(sim_entity_name(e)), [e.pos.x, e.pos.y, e.pos.z]));
        }
    }
    Ok(())
}

/// The harvester judge on redsim (`circuit sim`, the searches' screen): [`sim_run`] of the base
/// build (rotation 0, no offset) and, with a `tile:` header, of the copies side by side.
pub fn sim_judge(c: &Circuit, margin: i32) -> std::result::Result<Outcome, String> {
    let log = sim_run(c, RTS, RUN, margin)?;
    let mut out = judge(c, &log);
    if out.pass && c.tile.is_some() {
        let tc = crate::tiled(c).map_err(|e| format!("{e:#}"))?;
        match sim_run(&tc, RTS, RUN, margin) {
            Ok(tlog) => {
                let t = judge(&tc, &tlog);
                if t.pass {
                    out.table.push(format!("  tileable: yes ({} copies, {:.2} items per hour per cane)", tlog.canes, per_hour(&tlog)));
                } else {
                    out.pass = false;
                    out.why = format!("tiled: {}", t.why);
                }
            }
            Err(why) => {
                out.pass = false;
                out.why = format!("tiled: {why}");
            }
        }
    }
    Ok(out)
}
