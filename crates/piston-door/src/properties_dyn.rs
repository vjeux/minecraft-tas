//! Dynamic door properties (track props-dyn): how a WORKING door behaves in surroundings and with
//! inputs the judge does not try. Every check runs once: the design as written (offset 0), one
//! orientation, 1 open/close cycle (2 for snow, 20 for the endurance check). Companion of
//! `properties.rs` (the cheap, static properties): the two TSVs join on the design hash (the
//! leaderboard's verdict-cache key, 16 hex digits).
//!
//! 1. **hallway**: every empty cell face-adjacent to the 3x3 tunnel (the opening's columns along
//!    the door axis, both sides, door plane included, over the build's depth extent + 3 each way)
//!    is smooth stone; the door must still pass the judge's cycle checks (seamless/dentless).
//! 2. **contained**: a one-block shell around the swept bounding box (every cell the build holds at
//!    rest or at any tick of a cycle, plus the opening and its walkway), the tunnel's 3x3 columns
//!    left open, of air (baseline), smooth stone, slime, honey, obsidian and one seeded random mix
//!    of those: it must work with every shell, and every shell block must be back after the close.
//! 3. **snow**: a snow layer, placed as weather does (setBlock with updates: observers see it), on
//!    every sky-exposed top face that can hold one, in each stable state, over 2 cycles: the door
//!    still opens and closes, no entity (item) appears, nothing but snow changes. Also lists the
//!    flammable blocks whose top face is sky-exposed (lightning fire risk).
//! 4. **repeater**: the lever stays off; a repeater (delay 1) points into the lever's support
//!    block, fed by a redstone block placed and removed behind it (outside the build and its swept
//!    cells), so the input changes in the scheduled-tick phase instead of the player phase (the
//!    idea of MC-172213). Open/close ticks are counted like the judge's, from the tick before the
//!    repeater's output changes (the equivalent of a lever flip's tick 0).
//! 5. **steps**: in each stable state, a footstep (GameEvent.STEP, no source entity) at every
//!    walkable cell of the tunnel's floor row within 3 of the door; nothing may change for 40
//!    ticks and until idle (sculk doors are the ones at risk).
//! 6. **20 cycles**: the judge's own checks (`crate::verify`) for 20 cycles at the design's
//!    position (the walkway-clutter rule off, so older designs are judged as they were).

use crate::DoorSpec;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, Worker};
use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_TICKS: u32 = 400;
const DIRS6: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn short(s: &str) -> String {
    s.trim_start_matches("minecraft:").to_string()
}

fn is_air(s: &str) -> bool {
    mctas::blockstate::is_air(s)
}

/// Extra blocks around a design, raw-placed with it before it settles (already there when the
/// door is built).
#[derive(Clone, Debug, Default)]
pub struct Env {
    pub blocks: Vec<(Pos, String)>,
}

/// How the door is switched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Input {
    /// The judge's player right-click on the lever.
    Lever,
    /// A redstone block placed (on) / removed (off) at `source`, behind the repeater at
    /// `repeater` that points into the lever's support block.
    Repeater { source: Pos, repeater: Pos },
}

#[derive(Clone, Debug)]
pub struct RunOpts {
    pub cycles: u32,
    pub input: Input,
    /// Snow on every sky-exposed top face in each stable state.
    pub snow: bool,
    /// Footsteps on the tunnel floor in each stable state.
    pub steps: bool,
}

#[derive(Clone, Debug, Default)]
pub struct RunResult {
    pub open_ticks: Vec<u32>,
    pub close_ticks: Vec<u32>,
    /// Cells that held snow at some point.
    pub snow_cells: usize,
}

/// The judge's watch box grown to hold `extra` cells (+1 margin).
fn grow_box((mut lo, mut hi): (Pos, Pos), extra: impl IntoIterator<Item = Pos>) -> (Pos, Pos) {
    for p in extra {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i] - 1);
            hi[i] = hi[i].max(p[i] + 1);
        }
    }
    (lo, hi)
}

fn bbox(cells: &[Pos]) -> (Pos, Pos) {
    let mut lo = cells[0];
    let mut hi = cells[0];
    for p in cells {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

/// Cells of `rest` that differ in `now` (same box), `ignore` excepted.
fn region_diffs(now: &Region, rest: &Region, ignore: &HashSet<Pos>, max: usize) -> Vec<String> {
    rest.iter()
        .filter(|(p, _)| !ignore.contains(p))
        .filter(|(p, s)| now.get(*p) != Some(*s))
        .take(max)
        .map(|(p, s)| format!("{p:?} {} -> {}", short(s), short(now.get(p).unwrap_or("?"))))
        .collect()
}

/// Place the design (and `env`) the way the judge does and let it settle.
fn place(w: &mut Worker, sb: u32, d: &Design, env: &Env) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    let mut all: Vec<(Pos, String)> = blocks.clone();
    all.extend(env.blocks.iter().cloned());
    w.set_blocks(sb, &all, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let mut selfs: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    selfs.extend(env.blocks.iter().filter(|(_, s)| s.contains("repeater")).map(|(p, _)| *p));
    w.update(sb, &selfs, "self")?;
    if !d.entities.is_empty() {
        w.summon(sb, &d.entities)?;
    }
    let settle = w.step_until_idle(sb, MAX_TICKS)?;
    if settle.idle != Some(true) {
        bail!("never settles after placement ({} ticks)", settle.ticked);
    }
    Ok(())
}

fn entity_check(w: &mut Worker, sb: u32, d: &Design, what: &str) -> Result<()> {
    let ents = w.entities(sb)?;
    if let Some(e) = ents.iter().find(|e| !e.design) {
        bail!("an entity appeared {what}: {} at {:?}", e.kind, e.pos);
    }
    if ents.len() != d.entities.len() {
        bail!("{} of the design's {} entities left {what}", ents.len(), d.entities.len());
    }
    Ok(())
}

fn flip(w: &mut Worker, sb: u32, spec: &DoorSpec, input: Input, on: bool) -> Result<()> {
    match input {
        Input::Lever => {
            w.use_block(sb, spec.lever)?;
        }
        Input::Repeater { source, .. } => {
            let s = if on { "minecraft:redstone_block" } else { "minecraft:air" };
            w.set_blocks(sb, &[(source, s)], PlaceMode::Update)?;
        }
    }
    Ok(())
}

/// 0-based frame index of the first frame whose repeater output shows the new input (0 for the
/// lever: its flip is tick 0).
fn input_frame(frames: &[Region], input: Input, on: bool) -> u32 {
    match input {
        Input::Lever => 0,
        Input::Repeater { repeater, .. } => {
            let want = if on { "powered=true" } else { "powered=false" };
            frames.iter().position(|f| f.get(repeater).is_some_and(|s| s.contains(want))).map(|i| i as u32).unwrap_or(0)
        }
    }
}

/// Snow cells for `snap`: above the highest non-air block of every column of the box.
fn sky_cells(snap: &Region) -> Vec<(Pos, Pos)> {
    let (lo, size) = (snap.from, snap.size);
    let mut out = Vec::new();
    for x in lo[0]..lo[0] + size[0] {
        for z in lo[2]..lo[2] + size[2] {
            for y in (lo[1]..lo[1] + size[1]).rev() {
                if !is_air(snap.get([x, y, z]).unwrap_or("minecraft:air")) {
                    if y + 1 < lo[1] + size[1] {
                        out.push(([x, y, z], [x, y + 1, z]));
                    }
                    break;
                }
            }
        }
    }
    out
}

/// A snow layer on every sky-exposed top face of `snap` that can hold one (the game's own
/// `canSurvive`), placed with updates like weather (`setBlockAndUpdate`). Returns the cells.
fn snow_fall(w: &mut Worker, sb: u32, snap: &Region) -> Result<Vec<Pos>> {
    let cand: Vec<Pos> = sky_cells(snap).into_iter().map(|(_, above)| above).collect();
    if cand.is_empty() {
        return Ok(cand);
    }
    let lo = snap.from;
    let hi = [lo[0] + snap.size[0] - 1, lo[1] + snap.size[1] - 1, lo[2] + snap.size[2] - 1];
    let snow: Vec<(Pos, &str)> = cand.iter().map(|p| (*p, "minecraft:snow[layers=1]")).collect();
    w.set_blocks(sb, &snow, PlaceMode::Raw)?;
    let bad: HashSet<Pos> = w.unsupported(sb, lo, hi)?.into_iter().collect();
    let air: Vec<(Pos, &str)> = cand.iter().map(|p| (*p, "minecraft:air")).collect();
    w.set_blocks(sb, &air, PlaceMode::Raw)?;
    let ok: Vec<Pos> = cand.into_iter().filter(|p| !bad.contains(p)).collect();
    let place: Vec<(Pos, &str)> = ok.iter().map(|p| (*p, "minecraft:snow[layers=1]")).collect();
    if !place.is_empty() {
        w.set_blocks(sb, &place, PlaceMode::Update)?;
    }
    Ok(ok)
}

/// Feet cells of the tunnel within 3 of the door (its floor row) a player can stand in.
fn step_cells(spec: &DoorSpec, snap: &Region) -> Vec<Pos> {
    let mut cells = spec.door_cells();
    cells.extend(spec.corridor_cells());
    cells.into_iter().filter(|p| p[1] == spec.door[1]).filter(|p| !snap.flags_at(*p).is_some_and(|f| f.collides)).collect()
}

/// Weather and footsteps in a stable state (`snap`): nothing but snow may change.
#[allow(clippy::too_many_arguments)]
fn disturb(w: &mut Worker, sb: u32, spec: &DoorSpec, opts: &RunOpts, wbox: (Pos, Pos), snap: &Region, volatile: &mut HashSet<Pos>, state: &str) -> Result<()> {
    let (wlo, whi) = wbox;
    if opts.snow {
        let placed = snow_fall(w, sb, snap)?;
        volatile.extend(placed.iter().copied());
        let run = w.step_record(sb, MAX_TICKS, true, wlo, whi)?;
        if run.idle != Some(true) {
            bail!("snow on the {state} door: never settles");
        }
        for (i, f) in run.frames.iter().enumerate() {
            let diffs = region_diffs(f, snap, volatile, 6);
            if !diffs.is_empty() {
                bail!("snow on the {state} door changed it at tick {}: {}", i + 1, diffs.join("; "));
            }
        }
        if let Some(e) = w.entities(sb)?.iter().find(|e| !e.design) {
            bail!("snow on the {state} door: an entity appeared: {} at {:?}", e.kind, e.pos);
        }
    }
    if opts.steps {
        let cells = step_cells(spec, snap);
        w.game_event(sb, "step", &cells)?;
        let a = w.step_record(sb, 40, false, wlo, whi)?;
        let b = w.step_record(sb, MAX_TICKS, true, wlo, whi)?;
        for (i, f) in a.frames.iter().chain(b.frames.iter()).enumerate() {
            let diffs = region_diffs(f, snap, volatile, 6);
            if !diffs.is_empty() {
                bail!("footsteps by the {state} door changed it at tick {}: {}", i + 1, diffs.join("; "));
            }
        }
    }
    Ok(())
}

/// The judge's cycle checks (rest state, open, close back to the rest state, no entities) with
/// surroundings `env`, input `opts.input` and the stable-state disturbances of `opts`.
pub fn env_run(w: &mut Worker, sb: u32, d: &Design, spec: &DoorSpec, env: &Env, opts: &RunOpts) -> Result<RunResult> {
    place(w, sb, d, env)?;
    let (wlo, whi) = grow_box(crate::watch_box(d, spec), env.blocks.iter().map(|(p, _)| *p));
    let rest = w.get_region(sb, wlo, whi)?;
    let (floating, _) = w.static_problems(sb, wlo, whi)?;
    let mine: HashSet<Pos> =
        d.blocks.keys().copied().chain(env.blocks.iter().filter(|(_, s)| s.contains("repeater")).map(|(p, _)| *p)).collect();
    let fl: Vec<String> = floating.iter().filter(|p| mine.contains(*p)).map(|p| format!("{p:?} {}", short(rest.get(*p).unwrap_or("?")))).collect();
    if !fl.is_empty() {
        bail!("unsupported blocks: {}", fl.join("; "));
    }
    let problems = crate::closed_problems(&rest, spec);
    if !problems.is_empty() {
        bail!("not closed at rest: {}", problems.join("; "));
    }
    let lever = rest.get(spec.lever).unwrap_or("?");
    if !lever.starts_with("minecraft:lever") || !lever.contains("powered=false") {
        bail!("port lever {:?} is {lever}", spec.lever);
    }
    entity_check(w, sb, d, "at rest")?;
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| rest.get(*p).unwrap_or("?").to_string()).collect();
    let mut volatile: HashSet<Pos> = HashSet::new();
    let mut res = RunResult::default();
    let mut closed_snap = rest.clone();
    for cycle in 0..opts.cycles {
        disturb(w, sb, spec, opts, (wlo, whi), &closed_snap, &mut volatile, "closed").with_context(|| format!("cycle {cycle}"))?;
        flip(w, sb, spec, opts.input, true)?;
        let open = w.step_record(sb, MAX_TICKS, true, wlo, whi)?;
        if open.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after opening", open.ticked);
        }
        let last = open.frames.last().context("no frames")?.clone();
        let p = crate::open_problems(&last, spec);
        if !p.is_empty() {
            bail!("cycle {cycle}: not open: {}", p.join("; "));
        }
        entity_check(w, sb, d, "while opening").with_context(|| format!("cycle {cycle}"))?;
        let t0 = input_frame(&open.frames, opts.input, true);
        let ot = crate::settled_from(&open.frames, |f| crate::open_problems(f, spec).is_empty()).unwrap_or(open.ticked);
        res.open_ticks.push(ot.saturating_sub(t0));
        disturb(w, sb, spec, opts, (wlo, whi), &last, &mut volatile, "open").with_context(|| format!("cycle {cycle}"))?;
        flip(w, sb, spec, opts.input, false)?;
        let close = w.step_record(sb, MAX_TICKS, true, wlo, whi)?;
        if close.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after closing", close.ticked);
        }
        let last = close.frames.last().context("no frames")?.clone();
        let door_now: Vec<String> = spec.door_cells().iter().map(|p| last.get(*p).unwrap_or("?").to_string()).collect();
        if door_now != closed_door {
            let bad: Vec<String> = spec
                .door_cells()
                .iter()
                .zip(door_now.iter().zip(closed_door.iter()))
                .filter(|(_, (a, b))| a != b)
                .map(|(p, (a, _))| format!("{p:?} is {}", short(a)))
                .collect();
            bail!("cycle {cycle}: door not restored: {}", bad.join("; "));
        }
        let diffs = region_diffs(&last, &rest, &volatile, 6);
        if !diffs.is_empty() {
            bail!("cycle {cycle}: not back to its rest state: {}", diffs.join("; "));
        }
        entity_check(w, sb, d, "while closing").with_context(|| format!("cycle {cycle}"))?;
        let t0 = input_frame(&close.frames, opts.input, false);
        let ct = crate::settled_from(&close.frames, |f| spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str())))
            .unwrap_or(close.ticked);
        res.close_ticks.push(ct.saturating_sub(t0));
        closed_snap = last;
    }
    res.snow_cells = volatile.len();
    Ok(res)
}

/// Every cell the build holds at rest or at any tick of one lever cycle (empty surroundings), and
/// the settled rest state.
pub fn swept_cells(w: &mut Worker, sb: u32, d: &Design, spec: &DoorSpec) -> Result<(HashSet<Pos>, Region)> {
    place(w, sb, d, &Env::default())?;
    let (wlo, whi) = crate::watch_box(d, spec);
    let mut cells: HashSet<Pos> = d.blocks.keys().copied().collect();
    let rest = w.get_region(sb, wlo, whi)?;
    let mut add_frame = |r: &Region| {
        for (p, s) in r.iter() {
            if !is_air(s) {
                cells.insert(p);
            }
        }
    };
    add_frame(&rest);
    for _ in 0..2 {
        w.use_block(sb, spec.lever)?;
        let run = w.step_record(sb, MAX_TICKS, true, wlo, whi)?;
        for f in &run.frames {
            add_frame(f);
        }
    }
    Ok((cells, rest))
}

/// Smooth stone in every empty cell face-adjacent to the 3x3 tunnel (both sides of the opening's
/// columns, above and below, door plane included) over the build's depth extent + 3 each way.
pub fn hallway_env(d: &Design, spec: &DoorSpec) -> Env {
    let (wa, da) = if spec.along_z { (2, 0) } else { (0, 2) };
    let (lo, hi) = d.bounds().unwrap_or((spec.door, spec.door));
    let at = |wv: i32, y: i32, t: i32| {
        let mut p = [0; 3];
        p[wa] = wv;
        p[1] = y;
        p[da] = t;
        p
    };
    let mut out = Vec::new();
    for t in (lo[da].min(spec.door[da]) - 3)..=(hi[da].max(spec.door[da]) + 3) {
        let mut ring = Vec::new();
        for y in spec.door[1]..spec.door[1] + 3 {
            ring.push(at(spec.door[wa] - 1, y, t));
            ring.push(at(spec.door[wa] + 3, y, t));
        }
        for wv in spec.door[wa]..spec.door[wa] + 3 {
            ring.push(at(wv, spec.door[1] - 1, t));
            ring.push(at(wv, spec.door[1] + 3, t));
        }
        for p in ring {
            if !d.blocks.contains_key(&p) {
                out.push((p, "minecraft:smooth_stone".to_string()));
            }
        }
    }
    Env { blocks: out }
}

/// The shell: the cells one step outside the swept bounding box (swept cells + opening + walkway)
/// grown by `gap` (0 = flush against it), the tunnel's 3x3 columns in front of and behind the
/// door left open.
pub fn shell_cells(spec: &DoorSpec, swept: &HashSet<Pos>, gap: i32) -> Vec<Pos> {
    let mut cells: Vec<Pos> = swept.iter().copied().collect();
    cells.extend(spec.door_cells());
    cells.extend(spec.corridor_cells());
    let (mut lo, mut hi) = bbox(&cells);
    for i in 0..3 {
        lo[i] -= gap;
        hi[i] += gap;
    }
    let tunnel: HashSet<Pos> = spec.in_front_and_behind(96).into_iter().collect();
    let mut out = Vec::new();
    for x in lo[0] - 1..=hi[0] + 1 {
        for y in lo[1] - 1..=hi[1] + 1 {
            for z in lo[2] - 1..=hi[2] + 1 {
                let inside = (lo[0]..=hi[0]).contains(&x) && (lo[1]..=hi[1]).contains(&y) && (lo[2]..=hi[2]).contains(&z);
                let p = [x, y, z];
                if !inside && !tunnel.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}

pub const SHELLS: [&str; 6] = ["air", "smooth_stone", "slime_block", "honey_block", "obsidian", "mix"];

fn shell_env(cells: &[Pos], material: &str) -> Env {
    let mix = ["air", "smooth_stone", "slime_block", "honey_block", "obsidian"];
    let mut seed: u64 = 0x2545f4914f6cdd1d;
    let mut blocks = Vec::new();
    for p in cells {
        let m = if material == "mix" {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            mix[(seed % mix.len() as u64) as usize]
        } else {
            material
        };
        if m != "air" {
            blocks.push((*p, format!("minecraft:{m}")));
        }
    }
    Env { blocks }
}

fn prop(state: &str, key: &str) -> Option<String> {
    let props = state.split_once('[')?.1.trim_end_matches(']');
    props.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string()))
}

fn dir_vec(n: &str) -> Option<Pos> {
    match n {
        "north" => Some([0, 0, -1]),
        "south" => Some([0, 0, 1]),
        "east" => Some([1, 0, 0]),
        "west" => Some([-1, 0, 0]),
        "up" => Some([0, 1, 0]),
        "down" => Some([0, -1, 0]),
        _ => None,
    }
}

fn dir_name(v: Pos) -> &'static str {
    match v {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        _ => "down",
    }
}

/// The block a lever hangs on (`face=floor`: below, `ceiling`: above, `wall`: behind its facing).
pub fn lever_support(state: &str, lever: Pos) -> Option<Pos> {
    match prop(state, "face")?.as_str() {
        "floor" => Some(add(lever, [0, -1, 0])),
        "ceiling" => Some(add(lever, [0, 1, 0])),
        _ => {
            let f = dir_vec(&prop(state, "facing")?)?;
            Some([lever[0] - f[0], lever[1] - f[1], lever[2] - f[2]])
        }
    }
}

/// Where the repeater input goes: a free horizontal side of the lever's support block for the
/// repeater, the cell behind it for the redstone block (whose other neighbors are neither design
/// blocks nor swept cells), smooth stone under the repeater when nothing holds it. Outside the
/// build, its swept cells and the tunnel.
pub fn repeater_site(d: &Design, spec: &DoorSpec, swept: &HashSet<Pos>) -> Option<(Env, Input, Pos)> {
    repeater_sites(d, spec, swept).into_iter().next()
}

/// Every repeater site, strict ones first (nothing of the build next to the redstone block), then
/// the ones letting it touch blocks that ignore power (no redstone component, no piston, no
/// sculk...).
pub fn repeater_sites(d: &Design, spec: &DoorSpec, swept: &HashSet<Pos>) -> Vec<(Env, Input, Pos)> {
    let mut out = repeater_site_tier(d, spec, swept, false);
    for s in repeater_site_tier(d, spec, swept, true) {
        if !out.iter().any(|o: &(Env, Input, Pos)| o.1 == s.1) {
            out.push(s);
        }
    }
    out
}

/// Blocks a redstone block next to them could switch or update.
fn power_sensitive(state: &str) -> bool {
    let n = mctas::blockstate::block_name(state);
    [
        "piston", "redstone", "repeater", "comparator", "observer", "lamp", "dispenser", "dropper", "note_block", "door", "rail", "hopper",
        "tnt", "bulb", "crafter", "lever", "button", "sculk", "target", "bell", "fence_gate", "moving",
    ]
    .iter()
    .any(|k| n.contains(k))
}

fn repeater_site_tier(d: &Design, spec: &DoorSpec, swept: &HashSet<Pos>, loose: bool) -> Vec<(Env, Input, Pos)> {
    let mut out = Vec::new();
    let Some(s) = d.blocks.get(&spec.lever).and_then(|st| lever_support(st, spec.lever)) else { return out };
    let tunnel: HashSet<Pos> = spec.door_cells().into_iter().chain(spec.in_front_and_behind(96)).collect();
    let solid = |p: &Pos| d.blocks.contains_key(p) || swept.contains(p) || *p == spec.lever;
    let busy = |p: &Pos| solid(p) || tunnel.contains(p);
    let touches = |p: &Pos| {
        if loose {
            d.blocks.get(p).is_some_and(|s| power_sensitive(s)) || (swept.contains(p) && !d.blocks.contains_key(p)) || *p == spec.lever
        } else {
            solid(p)
        }
    };
    for dir in [[0, 0, -1], [0, 0, 1], [1, 0, 0], [-1, 0, 0]] {
        let p = add(s, dir);
        let q = add(p, dir);
        let below = add(p, [0, -1, 0]);
        if busy(&p) || busy(&q) {
            continue;
        }
        if DIRS6.iter().map(|o| add(q, *o)).filter(|n| *n != p).any(|n| touches(&n)) {
            continue;
        }
        let mut env = Vec::new();
        if !d.blocks.contains_key(&below) {
            if busy(&below) {
                continue;
            }
            env.push((below, "minecraft:smooth_stone".to_string()));
        }
        env.push((p, format!("minecraft:repeater[delay=1,facing={},locked=false,powered=false]", dir_name(dir))));
        out.push((Env { blocks: env }, Input::Repeater { source: q, repeater: p }, s));
    }
    out
}

/// Burnable blocks (FireBlock's flammable list, by name): wood, leaves, wool, carpets...
pub fn flammable(state: &str) -> bool {
    let n = short(mctas::blockstate::block_name(state));
    if n.contains("crimson") || n.contains("warped") {
        return false;
    }
    let wood = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "bamboo"];
    let wooden_part = ["_slab", "_stairs", "_fence", "_fence_gate"];
    [
        "planks", "_log", "_wood", "leaves", "wool", "carpet", "bookshelf", "tnt", "hay_block", "target", "dried_kelp_block", "scaffolding",
        "beehive", "bee_nest", "composter", "lectern", "vine", "azalea", "moss_block", "moss_carpet", "coal_block",
    ]
    .iter()
    .any(|k| n.contains(k))
        || (wood.iter().any(|w| n.starts_with(w)) && wooden_part.iter().any(|p| n.ends_with(p)))
}

/// Flammable blocks of `rest` whose top face is sky-exposed.
pub fn exposed_flammables(rest: &Region) -> Vec<String> {
    sky_cells(rest)
        .into_iter()
        .filter_map(|(top, _)| rest.get(top).filter(|s| flammable(s)).map(|s| format!("{} {top:?}", short(mctas::blockstate::block_name(s)))))
        .collect()
}

type Check = std::result::Result<(), String>;

/// One design's dynamic properties.
#[derive(Clone, Debug)]
pub struct Row {
    pub hash: String,
    pub source: String,
    pub blocks: usize,
    pub category: &'static str,
    pub verdict: String,
    pub lever_ticks: Option<(u32, u32)>,
    pub hallway: Check,
    pub hallway_cells: usize,
    /// Per shell material: Ok or why.
    pub contained: Vec<(&'static str, Check)>,
    /// The same shells one block further out (an air gap between the swept box and the shell).
    pub contained_gap: Vec<(&'static str, Check)>,
    pub snow: Check,
    pub snow_cells: usize,
    pub flammable: Vec<String>,
    pub repeater: Check,
    pub repeater_ticks: Option<(u32, u32)>,
    pub steps: Check,
    pub cycles20: Check,
}

fn err_text(e: anyhow::Error) -> String {
    format!("{e:#}").replace(['\t', '\n'], " ")
}

/// All six checks on one design (one sandbox, reused).
pub fn check_design(w: &mut Worker, source: &str, d: &Design, verdict: &str) -> Result<Row> {
    let spec = DoorSpec::from_design(d)?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Row> {
        let (swept, rest) = swept_cells(w, sb, d, &spec)?;
        let lever = RunOpts { cycles: 1, input: Input::Lever, snow: false, steps: false };
        // Hallway.
        let hall = hallway_env(d, &spec);
        let hallway = env_run(w, sb, d, &spec, &hall, &lever).map(|_| ()).map_err(err_text);
        // Contained: every shell, flush against the swept box, then with a one-block air gap.
        let shell = shell_cells(&spec, &swept, 0);
        let mut contained = Vec::new();
        let mut lever_ticks = None;
        for m in SHELLS {
            let r = env_run(w, sb, d, &spec, &shell_env(&shell, m), &lever);
            if m == "air" {
                if let Ok(rr) = &r {
                    lever_ticks = Some((rr.open_ticks[0], rr.close_ticks[0]));
                }
            }
            contained.push((m, r.map(|_| ()).map_err(err_text)));
        }
        let shell2 = shell_cells(&spec, &swept, 1);
        let mut contained_gap = Vec::new();
        for m in SHELLS.iter().copied().filter(|m| *m != "air") {
            let r = env_run(w, sb, d, &spec, &shell_env(&shell2, m), &lever);
            contained_gap.push((m, r.map(|_| ()).map_err(err_text)));
        }
        // Snow, 2 cycles.
        let snow_run = env_run(w, sb, d, &spec, &Env::default(), &RunOpts { cycles: 2, input: Input::Lever, snow: true, steps: false });
        let snow_cells = snow_run.as_ref().map(|r| r.snow_cells).unwrap_or(0);
        let snow = snow_run.map(|_| ()).map_err(err_text);
        let flammable = exposed_flammables(&rest);
        // Repeater input: the first site whose rig stands (a repeater on a non-sturdy design block
        // pops; that is the rig's fault, not the door's: try the next site).
        let mut repeater: Check = Err("no free side of the lever's support block for a repeater".to_string());
        let mut repeater_ticks = None;
        for (env, input, _) in repeater_sites(d, &spec, &swept) {
            match env_run(w, sb, d, &spec, &env, &RunOpts { cycles: 1, input, snow: false, steps: false }) {
                Ok(rr) => {
                    repeater = Ok(());
                    repeater_ticks = Some((rr.open_ticks[0], rr.close_ticks[0]));
                    break;
                }
                Err(e) => {
                    let e = err_text(e);
                    let rig = e.starts_with("an entity appeared at rest") || e.starts_with("unsupported blocks") || e.starts_with("not closed at rest");
                    if !rig {
                        repeater = Err(e);
                        break;
                    }
                }
            }
        }
        // Footsteps.
        let steps = env_run(w, sb, d, &spec, &Env::default(), &RunOpts { cycles: 1, input: Input::Lever, snow: false, steps: true })
            .map(|_| ())
            .map_err(err_text);
        Ok(Row {
            hash: format!("{:016x}", crate::leaderboard::design_hash(d)),
            source: source.to_string(),
            blocks: crate::block_count(d),
            category: crate::category(d),
            verdict: verdict.to_string(),
            lever_ticks,
            hallway,
            hallway_cells: hall.blocks.len(),
            contained,
            contained_gap,
            snow,
            snow_cells,
            flammable,
            repeater,
            repeater_ticks,
            steps,
            cycles20: Ok(()),
        })
    })();
    let _ = w.drop_sandbox(sb);
    let mut row = r?;
    // 20 cycles: the judge itself at the design's own position.
    let opts = crate::VerifyOptions { cycles: 20, offsets: vec![[0, 0, 0]], empty_walkway: false, ..Default::default() };
    row.cycles20 = crate::verify(w, d, &opts).map(|_| ()).map_err(err_text);
    Ok(row)
}

fn mark(c: &Check) -> &'static str {
    if c.is_ok() { "yes" } else { "no" }
}

fn contained_text(row: &Row) -> String {
    let bad: Vec<&str> = row.contained.iter().filter(|(_, c)| c.is_err()).map(|(m, _)| *m).collect();
    let gap_bad = row.contained_gap.iter().filter(|(_, c)| c.is_err()).count();
    let gap = if gap_bad == 0 { "gap ok".to_string() } else { format!("gap fails {gap_bad}") };
    if bad.is_empty() { format!("yes ({gap})") } else { format!("no: {} ({gap})", bad.join(", ").replace("_block", "").replace("smooth_stone", "stone")) }
}

fn ticks(t: Option<(u32, u32)>) -> String {
    t.map(|(o, c)| format!("{o}/{c}")).unwrap_or_else(|| "-".to_string())
}

fn why(c: &Check) -> String {
    c.as_ref().err().cloned().unwrap_or_default()
}

pub const TSV_HEADER: &str = "hash\tsource\tblocks\tcategory\tverdict\tlever_open_close\thallway\tcontained\tcontained_failing\tcontained_gap\tcontained_gap_failing\tsnow\tsnow_cells\tflammable_exposed\trepeater\trepeater_open_close\tsteps\tcycles20\thallway_why\tcontained_why\tsnow_why\trepeater_why\tsteps_why\tcycles20_why";

pub fn tsv_line(r: &Row) -> String {
    let failing: Vec<&str> = r.contained.iter().filter(|(_, c)| c.is_err()).map(|(m, _)| *m).collect();
    let gap_failing: Vec<&str> = r.contained_gap.iter().filter(|(_, c)| c.is_err()).map(|(m, _)| *m).collect();
    let cwhy: Vec<String> = r
        .contained
        .iter()
        .filter_map(|(m, c)| c.as_ref().err().map(|e| format!("{m}: {e}")))
        .chain(r.contained_gap.iter().filter_map(|(m, c)| c.as_ref().err().map(|e| format!("gap {m}: {e}"))))
        .collect();
    [
        r.hash.clone(),
        r.source.clone(),
        r.blocks.to_string(),
        r.category.to_string(),
        r.verdict.clone(),
        ticks(r.lever_ticks),
        mark(&r.hallway).to_string(),
        if failing.is_empty() { "yes" } else { "no" }.to_string(),
        failing.join(","),
        if gap_failing.is_empty() { "yes" } else { "no" }.to_string(),
        gap_failing.join(","),
        mark(&r.snow).to_string(),
        r.snow_cells.to_string(),
        r.flammable.join(","),
        mark(&r.repeater).to_string(),
        ticks(r.repeater_ticks),
        mark(&r.steps).to_string(),
        mark(&r.cycles20).to_string(),
        why(&r.hallway),
        cwhy.join(" | "),
        why(&r.snow),
        why(&r.repeater),
        why(&r.steps),
        why(&r.cycles20),
    ]
    .iter()
    .map(|s| s.replace(['\t', '\n'], " "))
    .collect::<Vec<_>>()
    .join("\t")
}

/// The markdown table: design | blocks | hallway | contained | snow | repeater (open/close) | steps | 20 cycles.
pub fn table(rows: &[Row]) -> String {
    let mut s = String::from(
        "| design | blocks | category | lever open/close | hallway | contained | snow | repeater (open/close) | steps | 20 cycles |\n|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for r in rows {
        let name = r.source.rsplit('/').next().unwrap_or(&r.source).trim_end_matches(".txt");
        let snow = match (&r.snow, r.flammable.is_empty()) {
            (Ok(()), true) => "yes".to_string(),
            (Ok(()), false) => format!("yes (flammable on top: {})", r.flammable.len()),
            (Err(_), true) => "no".to_string(),
            (Err(_), false) => format!("no (flammable on top: {})", r.flammable.len()),
        };
        let rep = match &r.repeater {
            Ok(()) => format!("yes ({})", ticks(r.repeater_ticks)),
            Err(e) if e.starts_with("no free side") => "n/a (no site)".to_string(),
            Err(_) => "no".to_string(),
        };
        s.push_str(&format!(
            "| {name} | {} | {} | {} | {} | {} | {snow} | {rep} | {} | {} |\n",
            r.blocks,
            r.category,
            ticks(r.lever_ticks),
            mark(&r.hallway),
            contained_text(r),
            mark(&r.steps),
            mark(&r.cycles20)
        ));
    }
    s
}

/// The leaderboard's verdict cache (`verdicts-v5.tsv`): design hash (hex) -> works.
pub fn load_verdicts(path: &Path) -> HashMap<String, bool> {
    let mut m = HashMap::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 2 {
                m.insert(f[0].to_string(), f[1] == "ok");
            }
        }
    }
    m
}

/// Every design `.txt` under designs/piston-door-3x3 on HEAD and on every ref under `refs_prefix`
/// (what [`crate::leaderboard::collect`] gathers: door + lever ports, at most `max_blocks`,
/// deduplicated by content), read with one `git cat-file --batch` over the unique blobs instead of
/// a `git show` per file and ref (~50k processes, ~15 min on a busy box).
pub fn collect_refs(repo: &Path, refs_prefix: &str, max_blocks: usize) -> Result<Vec<(String, Design)>> {
    use std::io::{BufRead, Read, Write};
    use std::process::{Command, Stdio};
    let git = |args: &[&str]| -> Result<String> {
        let out = Command::new("git").arg("-C").arg(repo).args(args).output()?;
        if !out.status.success() {
            bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let mut refs = vec!["HEAD".to_string()];
    for line in git(&["for-each-ref", "--format=%(refname:short)", refs_prefix])?.lines() {
        refs.push(line.trim().to_string());
    }
    // (label, blob) in ref order; blobs read once.
    let mut files: Vec<(String, String)> = Vec::new();
    let mut blobs: Vec<String> = Vec::new();
    let mut seen_blob = HashSet::new();
    for r in &refs {
        let Ok(list) = git(&["ls-tree", "-r", r, "designs/piston-door-3x3"]) else { continue };
        for line in list.lines() {
            // "<mode> blob <hash>\t<path>"
            let Some((meta, path)) = line.split_once('\t') else { continue };
            if !path.ends_with(".txt") {
                continue;
            }
            let Some(hash) = meta.split_whitespace().nth(2) else { continue };
            files.push((format!("{r}:{path}"), hash.to_string()));
            if seen_blob.insert(hash.to_string()) {
                blobs.push(hash.to_string());
            }
        }
    }
    let mut child = Command::new("git").arg("-C").arg(repo).args(["cat-file", "--batch"]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    let mut stdin = child.stdin.take().context("git stdin")?;
    let input: String = blobs.iter().map(|b| format!("{b}\n")).collect();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(input.as_bytes());
    });
    let mut out = std::io::BufReader::new(child.stdout.take().context("git stdout")?);
    let mut text: HashMap<String, String> = HashMap::new();
    for b in &blobs {
        let mut header = String::new();
        out.read_line(&mut header)?;
        let f: Vec<&str> = header.split_whitespace().collect();
        if f.len() < 3 || f[1] == "missing" {
            continue;
        }
        let size: usize = f[2].parse()?;
        let mut buf = vec![0u8; size + 1];
        out.read_exact(&mut buf)?;
        buf.pop();
        text.insert(b.clone(), String::from_utf8_lossy(&buf).into_owned());
    }
    let _ = writer.join();
    let _ = child.wait();
    let mut seen = HashSet::new();
    let mut res = Vec::new();
    for (label, blob) in files {
        let Some(t) = text.get(&blob) else { continue };
        let Ok(d) = Design::parse(t) else { continue };
        if !d.ports.contains_key("door") || !d.ports.contains_key("lever") || crate::block_count(&d) > max_blocks {
            continue;
        }
        if seen.insert(d.content_key()) {
            res.push((label, d));
        }
    }
    Ok(res)
}
