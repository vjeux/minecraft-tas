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
pub const CROPS: [(&str, &str); 2] = [("sugar_cane", "minecraft:sugar_cane[age=0]"), ("bamboo", "minecraft:bamboo[age=0,leaves=none,stage=0]")];

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
    if crop.ends_with("bamboo") { 16 } else { 3 }
}

/// Whether growths are counted by age resets (sugar cane: only the top block ages, and a growth
/// restarts it at 0, so a block destroyed in the tick it grew still counts) or by height (bamboo:
/// no age counter; its harvesters fire only on the growth itself).
fn grows_by_age(crop: &str) -> bool {
    !crop.ends_with("bamboo")
}

/// Whether the column has stopped growing by itself: at full height, or (bamboo) its top block
/// reads `stage=1`.
fn stopped(frame: &mctas::sim::Region, p: Pos, h: u32, crop: &str) -> bool {
    h >= full_height(crop) || (h >= 2 && crop.ends_with("bamboo") && frame.get([p[0], p[1] + h as i32 - 1, p[2]]).is_some_and(|s| s.contains("stage=1")))
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
    let mut h = 0;
    while frame.get([p[0], p[1] + h as i32, p[2]]).is_some_and(|s| s == crop || s.starts_with(&format!("{crop}["))) {
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
        return Ok(Err(format!("the crop planted at {p:?} does not survive (sugar cane needs sand, dirt or grass below it with water beside that block)")));
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
    let batch = if trace { 20 } else { BATCH };
    // item intrusion: one of the drops at a quarter, half and three quarters of the run, on every
    // open hopper intake (crate::intrusion) and above every crop column (items falling onto the farm)
    let intakes = crate::intrusion::intakes(c);
    let drop_at: Vec<u32> = (1..=3).map(|k| (ticks * k / 4 / batch) * batch).collect();
    let drop_cells: Vec<[f64; 3]> = intakes.iter().map(|p| [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5]).chain(crops.iter().map(|p| [p[0] as f64 + 0.5, (p[1] + full as i32 + 1) as f64, p[2] as f64 + 0.5])).collect();
    log.drop_cells = drop_cells.len();
    // (trace) the cells an item may be in on its way: the crop columns and above the hoppers
    let hoppers: Vec<Pos> = c.clock.design.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:hopper")).map(|(p, _)| *p).collect();
    let on_way = |p: [f64; 3]| {
        crops.iter().any(|q| p[0] >= q[0] as f64 - 0.2 && p[0] <= q[0] as f64 + 1.2 && p[2] >= q[2] as f64 - 0.2 && p[2] <= q[2] as f64 + 1.2 && p[1] >= q[1] as f64)
            || hoppers.iter().any(|h| p[0] >= h[0] as f64 - 0.2 && p[0] <= h[0] as f64 + 1.2 && p[2] >= h[2] as f64 - 0.2 && p[2] <= h[2] as f64 + 1.2 && p[1] >= h[1] as f64 + 0.5 && p[1] <= h[1] as f64 + 2.2)
    };
    while t < ticks + rest_ticks {
        if t == ticks {
            w.exec(sb, "gamerule random_tick_speed 0")?;
        }
        if let Some(k) = drop_at.iter().position(|d| *d == t && t > 0) {
            let (id, n) = INTRUDERS[k];
            for q in &drop_cells {
                w.exec(sb, &format!("summon minecraft:item {} {} {} {{Item:{{id:\"{id}\",count:{n}}}}}", q[0], q[1], q[2]))?;
            }
        }
        let n = batch.min(if t < ticks { ticks - t } else { ticks + rest_ticks - t });
        let r = w.step_record_watch(sb, n, false, rf, rt, &watch)?;
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
            if e.design || shulker::stack_of_entity(&e.state).is_none_or(|s| s.id != *crop) {
                continue;
            }
            if !trace && seen_items.contains(&e.uuid) && log.stranded.len() < 6 {
                let under = [e.pos[0].floor() as i32, (e.pos[1] - 0.01).floor() as i32, e.pos[2].floor() as i32];
                let what = w.get_region(sb, under, under).ok().and_then(|r| r.get(under).map(|s| clock::short(s).to_string())).unwrap_or_default();
                log.stranded.push(format!("tick {t}: an item at {:.2?} on {what} at {under:?} (motion {:.2?})", e.pos, e.motion));
            }
            if trace && !on_way(e.pos) && log.stranded.len() < 40 {
                log.stranded.push(format!("tick {t}: a stray item at {:.2?} (motion {:.2?})", e.pos, e.motion));
            }
            now_items.push(e.uuid.clone());
        }
        seen_items = now_items;
    }
    w.exec(sb, "gamerule random_tick_speed 0")?;
    finish(w, sb, c, crop, from, to, &judge, &mut log)?;
    Ok(Ok(log))
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
            log.delivered += stacks_of_be(&be.nbt).iter().filter(|s| s.id == crop).map(|s| s.count as u64).sum::<u64>();
        }
    }
    log.final_held = held(w, sb, from, to, judge)?;
    let had = log.initial.get(crop).copied().unwrap_or(0) as i64;
    log.held = log.final_held.get(crop).copied().unwrap_or(0) as i64 - had;
    for e in w.entities(sb)? {
        if e.design {
            continue;
        }
        match shulker::stack_of_entity(&e.state) {
            // the intruders may end anywhere
            Some(s) if INTRUDERS.iter().any(|(id, _)| *id == s.id) => log.intruders_loose += s.count as u64,
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

/// Judge a run: the outcome, its metrics and a table.
pub fn judge(c: &Circuit, log: &RunLog) -> Outcome {
    let blocks = c.blocks();
    let mut table = vec![
        format!("  {} canes, {} ticks at random_tick_speed {}: {} blocks grew, {} harvested, {} items in the judge's chests", log.canes, log.ticks, log.rts, log.grown, log.harvested, log.delivered),
        format!("  tallest cane {}, longest wait from growing to harvest {} gt, longest at full height {} gt", log.height_max, log.wait_max, log.full_max),
        format!("  intruders ({} dropped on {} cells): {} in the judge's chests, {} loose", log.drop_cells as u64 * INTRUDERS.iter().map(|x| x.1 as u64).sum::<u64>(), log.drop_cells, log.intruders_delivered, log.intruders_loose),
    ];
    for n in &log.notes {
        table.push(format!("  note: {n}"));
    }
    for n in &log.stranded {
        table.push(format!("  stranded: {n}"));
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
    let missing = log.harvested as i64 - log.delivered as i64;
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
    let metrics = vec![
        ("per cane".to_string(), format!("{per_cane_text} blocks")),
        ("items per hour per cane".to_string(), format!("{:.2}", per_hour(log))),
        ("harvest height".to_string(), log.height_max.to_string()),
        ("wait".to_string(), format!("{} gt", log.wait_max)),
    ];
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
