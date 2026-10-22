//! Study 19's MINECART STATION (clk-obs, coordinator 2026-10-21: "rail transit's basic block"): a
//! station on a rail line. `circuit: station 1` stands on a one-way line, `circuit: station 2` on a
//! two-way line.
//!
//! The design marks where the line meets it: `port line.in: x y z` and `port line.out: x y z` are
//! the first cells of the judge's own track at each end (air in the design, beside one of the
//! design's rails, level or one up or down); the judge lays [`TRACK`] cells of straight plain rail
//! outward from each, on stone where the design has nothing below. `port input.go: x y z button`
//! is the design's departure button.
//!
//! The rule: a cart arriving through `line.in` comes to rest inside the station (a ridden one where
//! the rider can click the button from the cart: [`clickable`], a look ray from a player's eyes,
//! [`EYE`] above the cart, reaching the button within [`REACH`] past every block and the cart) and
//! stays there until the button is pressed; a press sends it on in its direction, out through
//! `line.out`, rolling on by itself at least `TRACK - 2` blocks along the judge's track. A press
//! with no cart in the station changes nothing: the next cart still stops. `station 2`: a cart
//! coming the other way (in through `line.out`, fast enough to roll through that much plain track)
//! passes through without stopping and leaves through `line.in`.
//!
//! The carts are the game's own minecarts. A rider is an armor stand riding the cart (`/ride`), the
//! way a player rides it, so the cart runs the ridden physics (`OldMinecartBehavior`: its move
//! scaled by 0.75 and slowed by 0.997 a tick instead of 0.96). One run sends [`cars`] one after the
//! other: ridden and empty, arriving with 0.02 to 2.0 blocks per tick of momentum (2.0: a cart off a
//! booster line, the most a cart keeps), each held at rest for a seeded random time before the
//! press, some cycles with a press while the station is empty first. Item parts get the real-use
//! audit's stray items on their open hopper tops ([`crate::intrusion`]). Metrics: blocks, stop
//! distance (the farthest a cart rests past the station's entry), departure delay (the longest from
//! a press to the cart's first move).

use crate::check::Outcome;
use crate::stimulus::{SETTLE, Schedule};
use crate::verify::{Build, BuildOk, Options, VerifyReport};
use crate::{Circuit, InputHow, Kind, Pos};
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::EntitySpec;
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};

/// Cells of judge track laid beyond each line port.
pub const TRACK: i32 = 8;
/// A player's block interaction range, from the eyes.
pub const REACH: f64 = 4.5;
/// A player's eyes above the position of the cart they ride: the minecart's passenger attachment
/// (0.1875) minus the player's vehicle attachment (0.6) plus the eye height (1.62).
pub const EYE: f64 = 1.2075;
/// A cart that moves less than this in a tick stands still.
pub const STILL: f64 = 1.0e-4;
/// A cart standing still this many ticks in a row is at rest.
pub const REST: u32 = 10;
/// An arriving cart must be at rest within this many ticks.
pub const ARRIVE: u32 = 300;
/// A pressed departure must move the cart within this many ticks.
pub const GO: u32 = 80;
/// A leaving (or passing) cart must reach the end of the judge's track within this many ticks.
pub const LEAVE: u32 = 300;
/// The gap after each cart (a wooden button stays down 30 gt).
pub const GAP: u32 = 40;
/// A cart's height above its flat rail's block (`OldMinecartBehavior.getPos`).
pub(crate) const RAIL_Y: f64 = 0.0625;

/// A rail block of any kind.
pub fn is_rail(s: &str) -> bool {
    let id = s.split('[').next().unwrap_or(s);
    matches!(id, "minecraft:rail" | "minecraft:powered_rail" | "minecraft:detector_rail" | "minecraft:activator_rail")
}

/// One end of the line: the first judge rail and the direction away from the station.
#[derive(Clone, Copy, Debug)]
pub struct End {
    pub cell: Pos,
    pub out: [i32; 3],
}

/// The line through the station.
#[derive(Clone, Copy, Debug)]
pub struct Line {
    pub entry: End,
    pub exit: End,
}

pub(crate) fn end_of(c: &Circuit, name: &str) -> Result<End> {
    let d = &c.clock.design;
    let Some(p) = d.ports.get(name).copied() else {
        bail!("station: needs `port {name}: x y z` (the first cell of the judge's track at that end: air beside the design's end rail)");
    };
    if d.blocks.contains_key(&p) {
        bail!("port {name}: the judge's rail goes on an air cell, the design has a block at {p:?}");
    }
    let dirs = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
    let inward: Vec<[i32; 3]> = dirs.iter().copied().filter(|v| (-1..=1).any(|dy| d.blocks.get(&[p[0] + v[0], p[1] + dy, p[2] + v[2]]).is_some_and(|s| is_rail(s)))).collect();
    if inward.len() != 1 {
        bail!("port {name} at {p:?}: want exactly one of the design's rails beside it (level, one up or one down), found {}", inward.len());
    }
    Ok(End { cell: p, out: [-inward[0][0], 0, -inward[0][2]] })
}

/// The line's two ends.
pub fn line(c: &Circuit) -> Result<Line> {
    Ok(Line { entry: end_of(c, "line.in")?, exit: end_of(c, "line.out")? })
}

/// The judge's track at one end: supports (where the design has no block) and straight rails.
pub(crate) fn track(c: &Circuit, e: &End) -> Vec<(Pos, String)> {
    let shape = if e.out[0] != 0 { "east_west" } else { "north_south" };
    let mut v = Vec::new();
    for k in 0..TRACK {
        let q = [e.cell[0] + k * e.out[0], e.cell[1], e.cell[2] + k * e.out[2]];
        let below = [q[0], q[1] - 1, q[2]];
        if !c.clock.design.blocks.contains_key(&below) {
            v.push((below, "minecraft:stone".to_string()));
        }
        v.push((q, format!("minecraft:rail[shape={shape},waterlogged=false]")));
    }
    v
}

/// The static checks: both line ports, a departure button, the judge's tracks clear of the design.
pub fn check_ports(c: &Circuit) -> Result<()> {
    let l = line(c)?;
    match c.inputs.iter().find(|i| i.name == "go") {
        Some(i) if i.how == InputHow::Button => {}
        _ => bail!("station: the departure input is the design's button (`port input.go: x y z button`)"),
    }
    for (name, e) in [("line.in", &l.entry), ("line.out", &l.exit)] {
        for k in 0..TRACK {
            let q = [e.cell[0] + k * e.out[0], e.cell[1], e.cell[2] + k * e.out[2]];
            if c.clock.design.blocks.contains_key(&q) {
                bail!("port {name}: the judge's track runs {TRACK} cells straight out from it, the design has a block at {q:?}");
            }
        }
    }
    Ok(())
}

/// One cart of the run.
#[derive(Clone, Debug)]
pub struct Car {
    pub rider: bool,
    /// Its momentum toward the station (blocks per tick).
    pub v: f64,
    /// It comes the other way (in through `line.out`: a two-way line's traffic, which passes).
    pub back: bool,
    /// Ticks at rest before the press.
    pub wait: u32,
    /// A press this many ticks before the cart is sent in (the station empty).
    pub early: Option<u32>,
    /// A press this many ticks after the cart is sent in, while it arrives: it may leave at once
    /// or stop and wait for the next press, but it must leave in its direction.
    pub during: Option<u32>,
}

/// The run's carts, in order (seeded): every speed ridden and empty; on a two-way line, carts the
/// other way between them (fast enough to roll through the station's length of plain track: an
/// empty cart at 0.02 b/t stops within half a block anywhere).
pub fn cars(two: bool) -> Vec<Car> {
    let mut x: u64 = 0x2545_f491_4f6c_dd1d;
    let mut rnd = |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    let ridden = [2.0, 0.02, 1.0, 0.1, 0.4, 0.25, 1.5, 0.6, 0.05, 0.8];
    let empty = [1.5, 0.02, 0.8, 0.1, 0.4, 0.25, 0.05];
    let back_ridden = [2.0, 0.1, 0.4, 1.0, 0.25];
    let back_empty = [1.5, 0.4, 0.8];
    let mut v = Vec::new();
    for i in 0..ridden.len().max(empty.len()) {
        for (rider, list, back) in [(true, &ridden[..], &back_ridden[..]), (false, &empty[..], &back_empty[..])] {
            let Some(&s) = list.get(i) else { continue };
            let wait = 5 + rnd(150) as u32;
            let early = if rnd(3) == 0 { Some(60 + rnd(150) as u32) } else { None };
            let during = if early.is_none() && v.len() % 3 == 1 { Some(1 + rnd(8) as u32) } else { None };
            v.push(Car { rider, v: s, back: false, wait, early, during });
            if two && let Some(&b) = back.get(i) {
                v.push(Car { rider, v: b, back: true, wait: 0, early: None, during: None });
            }
        }
    }
    v
}

/// What one run saw.
#[derive(Debug, Default)]
pub struct RunLog {
    pub lines: Vec<String>,
    pub fail: Option<String>,
    /// The farthest a cart came to rest past the station's entry (blocks).
    pub stop: f64,
    /// The longest from a press to the cart's first move (gt).
    pub delay: u32,
    /// The slowest leaving cart at the end of the judge's track (blocks per tick).
    pub exit_min: f64,
    /// The slowest cart coming the other way, at the end of the judge's track.
    pub pass_min: Option<f64>,
    pub dims: [i32; 3],
    pub ticks: u32,
}

/// Run a circuit already turned and moved, in a fresh sandbox.
pub fn game_run(w: &mut Worker, c: &Circuit, margin: i32) -> Result<std::result::Result<RunLog, String>> {
    let l = match line(c) {
        Ok(l) => l,
        Err(e) => return Ok(Err(format!("{e:#}"))),
    };
    let (mut from, mut to) = crate::run::watch_box(c, margin);
    for e in [&l.entry, &l.exit] {
        let far = [e.cell[0] + TRACK * e.out[0], e.cell[1], e.cell[2] + TRACK * e.out[2]];
        for i in 0..3 {
            from[i] = from[i].min(far[i] - 1);
            to[i] = to[i].max(far[i] + 1);
        }
    }
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: crate::shulker::build_seed(from, to, 0) })?;
    let r = run_in(w, sb, c, &l, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

pub(crate) fn hdist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

pub(crate) fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// How far `p` is past an end's first judge rail, outward (negative: inside the station).
pub(crate) fn along(p: [f64; 3], e: &End) -> f64 {
    (p[0] - (e.cell[0] as f64 + 0.5)) * e.out[0] as f64 + (p[2] - (e.cell[2] as f64 + 0.5)) * e.out[2] as f64
}

/// Out at the far end of an end's judge track.
pub(crate) fn gone(p: [f64; 3], e: &End) -> bool {
    along(p, e) >= (TRACK - 2) as f64
}

fn cart_at(w: &mut Worker, sb: u32, uuid: &str) -> Result<Option<[f64; 3]>> {
    Ok(w.entities(sb)?.into_iter().find(|e| e.uuid == uuid).map(|e| e.pos))
}

/// The rider still sits in the cart (`None`), or where it went.
fn rider_problem(w: &mut Worker, sb: u32, cart: &str, rider: &str) -> Result<Option<String>> {
    let ents = w.entities(sb)?;
    let (Some(c), r) = (ents.iter().find(|e| e.uuid == cart), ents.iter().find(|e| e.uuid == rider)) else { return Ok(Some("the cart is gone".into())) };
    match r {
        None => Ok(Some("the rider is gone".into())),
        Some(r) if hdist(r.pos, c.pos) > 0.3 || (r.pos[1] - c.pos[1]).abs() > 1.0 => Ok(Some(format!("the rider was thrown out: it stands at ({:.2}, {:.2}, {:.2}), the cart at ({:.2}, {:.2}, {:.2})", r.pos[0], r.pos[1], r.pos[2], c.pos[0], c.pos[1], c.pos[2]))),
        Some(_) => Ok(None),
    }
}

fn remove(w: &mut Worker, sb: u32, cart: &str, rider: Option<&str>) -> Result<()> {
    w.exec(sb, &format!("kill {cart}"))?;
    if let Some(r) = rider {
        w.exec(sb, &format!("kill {r}"))?;
    }
    Ok(())
}

fn press(w: &mut Worker, sb: u32, go: Pos) -> Result<()> {
    clock::verify::apply(w, sb, &clock::Action::Use { pos: go })
}

fn run_in(w: &mut Worker, sb: u32, c: &Circuit, l: &Line, from: Pos, to: Pos) -> Result<std::result::Result<RunLog, String>> {
    clock::verify::place(w, sb, &c.clock)?;
    let placed = w.get_region(sb, from, to)?;
    let popped = c.clock.design.blocks.iter().find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, c.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
    let mut cells = track(c, &l.entry);
    cells.extend(track(c, &l.exit));
    w.set_blocks(sb, &cells, PlaceMode::Raw)?;
    TRACE_CELLS.with(|t| *t.borrow_mut() = c.clock.design.blocks.iter().filter(|(_, s)| is_rail(s)).map(|(p, _)| *p).collect());
    w.step(sb, c.clock.free_from() + SETTLE)?;
    let go = c.inputs.iter().find(|i| i.name == "go").map(|i| i.pos).ok_or_else(|| anyhow!("no input.go"))?;
    let (lo, hi) = c.clock.design.bounds().ok_or_else(|| anyhow!("an empty design"))?;
    // what a rider's look ray meets: the design as built and the judge's track
    let mut view: std::collections::BTreeMap<Pos, String> = c.clock.design.blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
    for (p, s) in &cells {
        view.insert(*p, s.clone());
    }
    let site = Site { l, go, lo, hi, view: &view };
    let two = matches!(c.kind, Some(Kind::Station(2)));
    let mut log = RunLog { exit_min: f64::MAX, dims: crate::shulker::design_dims(c), ..Default::default() };
    let list = cars(two);
    let intakes = crate::intrusion::intakes(c);
    let drop_at: Vec<usize> = if intakes.is_empty() || !crate::check::real_use() { Vec::new() } else { (1..=3).map(|k| list.len() * k / 4).collect() };
    let mut t = 0u32;
    for (k, car) in list.iter().enumerate() {
        if let Some(j) = drop_at.iter().position(|&d| d == k) {
            let item = crate::intrusion::DROPS[j];
            let specs: Vec<EntitySpec> = intakes.iter().map(|p| EntitySpec { kind: "minecraft:item".into(), pos: [p[0] as f64 + 0.5, p[1] as f64 + 0.25, p[2] as f64 + 0.5], nbt: Some(format!("{{Item:{item}}}")) }).collect();
            w.summon(sb, &specs)?;
            log.lines.push(format!("before car {}: {item} dropped on each open hopper top {intakes:?}", k + 1));
        }
        let label = format!("car {} ({}, {:.2} b/t{})", k + 1, if car.rider { "ridden" } else { "empty" }, car.v, if car.back { ", the other way" } else { "" });
        if let Err(why) = cycle(w, sb, &site, car, &label, &mut t, &mut log)? {
            log.fail = Some(format!("{label}: {why}"));
            break;
        }
    }
    log.ticks = t;
    Ok(Ok(log))
}

/// The station as the run sees it.
struct Site<'a> {
    l: &'a Line,
    go: Pos,
    lo: Pos,
    hi: Pos,
    view: &'a std::collections::BTreeMap<Pos, String>,
}

fn cycle(w: &mut Worker, sb: u32, s: &Site, car: &Car, label: &str, t: &mut u32, log: &mut RunLog) -> Result<std::result::Result<(), String>> {
    let (l, go) = (s.l, s.go);
    if let Some(e) = car.early {
        press(w, sb, go)?;
        w.step(sb, e)?;
        *t += e;
        log.lines.push(format!("{label}: the button pressed {e} gt before it comes (no cart in the station)"));
    }
    let end = if car.back { l.exit } else { l.entry };
    // at the station's edge (a creeping empty cart stops within half a block)
    let at = [end.cell[0] as f64 + 0.5 - 0.48 * end.out[0] as f64, end.cell[1] as f64 + RAIL_Y, end.cell[2] as f64 + 0.5 - 0.48 * end.out[2] as f64];
    let nbt = format!("{{Motion:[{:.6}d,0.0d,{:.6}d]}}", -(end.out[0] as f64) * car.v, -(end.out[2] as f64) * car.v);
    let cart = w.summon(sb, &[EntitySpec { kind: "minecraft:minecart".into(), pos: at, nbt: Some(nbt) }])?.into_iter().next().ok_or_else(|| anyhow!("summon: no uuid"))?;
    let rider = if car.rider {
        let r = w.summon(sb, &[EntitySpec { kind: "minecraft:armor_stand".into(), pos: at, nbt: None }])?.into_iter().next().ok_or_else(|| anyhow!("summon: no uuid"))?;
        w.exec(sb, &format!("ride {r} mount {cart}"))?;
        Some(r)
    } else {
        None
    };
    let r = follow(w, sb, s, car, &cart, rider.as_deref(), t, log)?;
    remove(w, sb, &cart, rider.as_deref())?;
    w.step(sb, GAP)?;
    *t += GAP;
    if let Ok(line) = &r {
        let pre = match car.during {
            Some(d) if !line.starts_with("pressed") => format!("pressed {d} gt after it came in, then "),
            _ => String::new(),
        };
        log.lines.push(format!("{label}: {pre}{line}"));
    }
    Ok(r.map(|_| ()))
}

/// One tick, then the cart's position (`None`: it is gone). `STATION_TRACE=1`: every tick's cart
/// position and the design's rails' states on stderr.
fn tick(w: &mut Worker, sb: u32, cart: &str, t: &mut u32) -> Result<Option<[f64; 3]>> {
    w.step(sb, 1)?;
    *t += 1;
    let p = cart_at(w, sb, cart)?;
    if std::env::var("STATION_TRACE").is_ok() {
        let n = w.entities(sb)?.len();
        TRACE_CELLS.with(|cells| -> Result<()> {
            let cells = cells.borrow();
            let mut states = Vec::new();
            for q in cells.iter() {
                let r = w.get_region(sb, *q, *q)?;
                let s = r.get(*q).map(|s| s.to_string()).unwrap_or_default();
                let short = s.trim_start_matches("minecraft:").replace("waterlogged=false", "").replace(",]", "]");
                states.push(format!("{q:?} {short}"));
            }
            eprintln!("t={t} cart={p:.3?} entities={n} | {}", states.join(" | "));
            Ok(())
        })?;
    }
    Ok(p)
}

thread_local! {
    /// The rail cells `STATION_TRACE` prints.
    static TRACE_CELLS: std::cell::RefCell<Vec<Pos>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[allow(clippy::too_many_arguments)]
fn follow(w: &mut Worker, sb: u32, s: &Site, car: &Car, cart: &str, rider: Option<&str>, t: &mut u32, log: &mut RunLog) -> Result<std::result::Result<String, String>> {
    let (l, go, lo, hi) = (s.l, s.go, s.lo, s.hi);
    let start = cart_at(w, sb, cart)?.ok_or_else(|| anyhow!("the summoned cart is missing"))?;
    let fmt = |p: [f64; 3]| format!("({:.2}, {:.2}, {:.2})", p[0], p[1], p[2]);
    if car.back {
        // the other way: through without stopping
        let (mut prev, mut still, mut speed) = (start, 0u32, None);
        for _ in 0..LEAVE {
            let Some(p) = tick(w, sb, cart, t)? else { return Ok(Err("the cart was destroyed".into())) };
            if dist(prev, p) < STILL {
                still += 1;
            } else {
                still = 0;
            }
            if still >= REST {
                return Ok(Err(format!("a cart coming the other way stopped at {} (it must pass through)", fmt(p))));
            }
            if gone(p, &l.exit) {
                return Ok(Err("a cart coming the other way turned back".into()));
            }
            if speed.is_none() && along(p, &l.entry) > -0.5 {
                speed = Some(hdist(prev, p));
            }
            if gone(p, &l.entry) {
                let sp = speed.unwrap_or_else(|| hdist(prev, p));
                log.pass_min = Some(log.pass_min.map_or(sp, |m| m.min(sp)));
                if let Some(r) = rider
                    && let Some(why) = rider_problem(w, sb, cart, r)?
                {
                    return Ok(Err(why));
                }
                return Ok(Ok(format!("passes through, out at {sp:.2} b/t")));
            }
            prev = p;
        }
        return Ok(Err(format!("a cart coming the other way did not get through within {LEAVE} gt")));
    }
    // arrival: at rest inside the station (or, pressed while it arrives, sent straight on)
    let (mut prev, mut still, mut rest) = (start, 0u32, None);
    for k in 0..ARRIVE {
        if car.during == Some(k) {
            press(w, sb, go)?;
        }
        let Some(p) = tick(w, sb, cart, t)? else { return Ok(Err("the cart was destroyed".into())) };
        if gone(p, &l.exit) {
            if let Some(d) = car.during.filter(|d| *d <= k) {
                if let Some(rd) = rider
                    && let Some(why) = rider_problem(w, sb, cart, rd)?
                {
                    return Ok(Err(why));
                }
                return Ok(Ok(format!("pressed {d} gt after it came in: sent straight on")));
            }
            return Ok(Err(format!("passed through without stopping ({:.2} b/t at the end of the track)", hdist(prev, p))));
        }
        if gone(p, &l.entry) {
            return Ok(Err("rolled back out through line.in".into()));
        }
        if dist(prev, p) < STILL {
            still += 1;
        } else {
            still = 0;
        }
        prev = p;
        if still >= REST {
            rest = Some(p);
            break;
        }
    }
    let Some(r) = rest else { return Ok(Err(format!("still moving {ARRIVE} gt after it was sent in"))) };
    let inside = r[0] >= lo[0] as f64 && r[0] <= (hi[0] + 1) as f64 && r[2] >= lo[2] as f64 && r[2] <= (hi[2] + 1) as f64;
    if !inside {
        return Ok(Err(format!("came to rest at {}, outside the station", fmt(r))));
    }
    if let Some(rd) = rider {
        if let Some(why) = rider_problem(w, sb, cart, rd)? {
            return Ok(Err(why));
        }
        // the rider presses the button from the cart: a look ray from a player's eyes there
        let eye = [r[0], r[1] + EYE, r[2]];
        if !clickable(s.view, eye, go) {
            return Ok(Err(format!("came to rest at {}, where the rider cannot click the button: no look ray from a player's eyes in the cart reaches it within {REACH} blocks past the blocks in the way", fmt(r))));
        }
    }
    let face = [l.entry.cell[0] as f64 + 0.5 - 0.5 * l.entry.out[0] as f64, 0.0, l.entry.cell[2] as f64 + 0.5 - 0.5 * l.entry.out[2] as f64];
    let stop = (r[0] - face[0]).abs() + (r[2] - face[2]).abs();
    log.stop = log.stop.max(stop);
    // stored: it stays until the press
    for k in 0..car.wait {
        let Some(p) = tick(w, sb, cart, t)? else { return Ok(Err("the cart was destroyed".into())) };
        if dist(r, p) > 1.0e-3 {
            return Ok(Err(format!("rolled away {} gt after it came to rest at {}, with no press", k + 1, fmt(r))));
        }
    }
    // the press: it leaves in its direction
    press(w, sb, go)?;
    let tp = *t;
    let mut moved = None;
    for _ in 0..GO {
        let Some(p) = tick(w, sb, cart, t)? else { return Ok(Err("the cart was destroyed".into())) };
        if dist(r, p) > 1.0e-3 {
            moved = Some(*t - tp);
            break;
        }
    }
    let Some(delay) = moved else { return Ok(Err(format!("rests at {} and did not move within {GO} gt of the press", fmt(r)))) };
    log.delay = log.delay.max(delay);
    let mut prev = cart_at(w, sb, cart)?.unwrap_or(r);
    let mut speed = None;
    for _ in 0..LEAVE {
        let Some(p) = tick(w, sb, cart, t)? else { return Ok(Err("the cart was destroyed".into())) };
        if gone(p, &l.entry) {
            return Ok(Err("left the wrong way (out through line.in)".into()));
        }
        if speed.is_none() && along(p, &l.exit) > -0.5 {
            speed = Some(hdist(prev, p));
        }
        if gone(p, &l.exit) {
            let sp = speed.unwrap_or_else(|| hdist(prev, p));
            log.exit_min = log.exit_min.min(sp);
            if let Some(rd) = rider
                && let Some(why) = rider_problem(w, sb, cart, rd)?
            {
                return Ok(Err(why));
            }
            return Ok(Ok(format!("rests {stop:.2} blocks in at {}, held {} gt, moves {delay} gt after the press, out at {sp:.2} b/t", fmt(r), car.wait)));
        }
        prev = p;
    }
    Ok(Err(format!("pressed at rest at {}, but did not roll {} blocks past the station within {LEAVE} gt", fmt(r), TRACK - 2)))
}

// ------------------------------------------------------------------ can the rider click the button

/// An axis-aligned box (world coordinates).
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

fn prop<'a>(state: &'a str, key: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.trim_end_matches(']');
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim()))
}

/// A block's outline box (`getShape`: what a player's look ray hits) in sixteenths of its cell:
/// rails 2/16 tall (8/16 on a slope), an unpressed button its 6x4x2 box (`ButtonBlock`), air none,
/// anything else a full cube (the conservative reading: it blocks the view).
pub fn outline(state: &str) -> Option<[f64; 6]> {
    let id = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    if id == "air" || id == "cave_air" || id == "void_air" {
        return None;
    }
    if is_rail(state) {
        let slope = prop(state, "shape").is_some_and(|s| s.starts_with("ascending"));
        return Some([0.0, 0.0, 0.0, 16.0, if slope { 8.0 } else { 2.0 }, 16.0]);
    }
    if id.ends_with("_button") {
        let thick = if prop(state, "powered") == Some("true") { 1.0 } else { 2.0 };
        let facing = prop(state, "facing").unwrap_or("north");
        return Some(match prop(state, "face").unwrap_or("wall") {
            "floor" | "ceiling" => {
                let (y0, y1) = if prop(state, "face") == Some("floor") { (0.0, thick) } else { (16.0 - thick, 16.0) };
                if matches!(facing, "east" | "west") { [6.0, y0, 5.0, 10.0, y1, 11.0] } else { [5.0, y0, 6.0, 11.0, y1, 10.0] }
            }
            _ => match facing {
                "north" => [5.0, 6.0, 16.0 - thick, 11.0, 10.0, 16.0],
                "south" => [5.0, 6.0, 0.0, 11.0, 10.0, thick],
                "east" => [0.0, 6.0, 5.0, thick, 10.0, 11.0],
                _ => [16.0 - thick, 6.0, 5.0, 16.0, 10.0, 11.0],
            },
        });
    }
    Some([0.0, 0.0, 0.0, 16.0, 16.0, 16.0])
}

fn cell_box(p: Pos, s: [f64; 6]) -> Aabb {
    Aabb { lo: [p[0] as f64 + s[0] / 16.0, p[1] as f64 + s[1] / 16.0, p[2] as f64 + s[2] / 16.0], hi: [p[0] as f64 + s[3] / 16.0, p[1] as f64 + s[4] / 16.0, p[2] as f64 + s[5] / 16.0] }
}

/// Where the segment `a + t (b - a)`, t in 0..=1, enters the box (`None`: it misses).
fn enter(a: [f64; 3], b: [f64; 3], bx: &Aabb) -> Option<f64> {
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for i in 0..3 {
        let d = b[i] - a[i];
        if d.abs() < 1e-12 {
            if a[i] < bx.lo[i] || a[i] > bx.hi[i] {
                return None;
            }
            continue;
        }
        let (mut u, mut v) = ((bx.lo[i] - a[i]) / d, (bx.hi[i] - a[i]) / d);
        if u > v {
            std::mem::swap(&mut u, &mut v);
        }
        t0 = t0.max(u);
        t1 = t1.min(v);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// Whether a player whose eyes are at `eye` can click the button at `button` among `blocks`: some
/// look ray from the eyes reaches the button's outline first, within [`REACH`] (no block's outline
/// in the way; the player's own cart does not take the click: `ProjectileUtil.getEntityHitResult`
/// passes over an entity of the player's root vehicle). The look rays aim at a 5 x 5 grid on each
/// face of the button's box.
pub fn clickable(blocks: &std::collections::BTreeMap<Pos, String>, eye: [f64; 3], button: Pos) -> bool {
    let Some(bs) = blocks.get(&button).and_then(|s| outline(s)) else { return false };
    let bb = cell_box(button, bs);
    let lo = [eye[0].min(bb.lo[0]).floor() as i32 - 1, eye[1].min(bb.lo[1]).floor() as i32 - 1, eye[2].min(bb.lo[2]).floor() as i32 - 1];
    let hi = [eye[0].max(bb.hi[0]).ceil() as i32 + 1, eye[1].max(bb.hi[1]).ceil() as i32 + 1, eye[2].max(bb.hi[2]).ceil() as i32 + 1];
    let others: Vec<Aabb> = blocks.iter().filter(|(p, _)| **p != button && (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i])).filter_map(|(p, s)| outline(s).map(|o| cell_box(*p, o))).collect();
    let f = [0.1, 0.3, 0.5, 0.7, 0.9];
    for axis in 0..3 {
        for side in [bb.lo[axis], bb.hi[axis]] {
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            for fu in f {
                for fv in f {
                    let mut tgt = [0.0; 3];
                    tgt[axis] = side;
                    tgt[u] = bb.lo[u] + fu * (bb.hi[u] - bb.lo[u]);
                    tgt[v] = bb.lo[v] + fv * (bb.hi[v] - bb.lo[v]);
                    let Some(tb) = enter(eye, tgt, &bb) else { continue };
                    let hit = [eye[0] + tb * (tgt[0] - eye[0]), eye[1] + tb * (tgt[1] - eye[1]), eye[2] + tb * (tgt[2] - eye[2])];
                    if dist(eye, hit) > REACH {
                        continue;
                    }
                    let blocked = others.iter().any(|o| enter(eye, tgt, o).is_some_and(|t| t < tb));
                    if !blocked {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// The run's verdict.
pub fn judge(c: &Circuit, log: &RunLog) -> Outcome {
    let table = log.lines.clone();
    if let Some(f) = &log.fail {
        return Outcome { pass: false, why: f.clone(), metrics: Vec::new(), table };
    }
    let two = matches!(c.kind, Some(Kind::Station(2)));
    let mut metrics = vec![
        ("line".to_string(), if two { "two-way" } else { "one-way" }.to_string()),
        ("stop distance".to_string(), format!("{:.2} blocks", log.stop)),
        ("departure delay".to_string(), format!("{} gt", log.delay)),
        ("leaves at".to_string(), format!("{:.2} b/t", log.exit_min)),
    ];
    if let Some(p) = log.pass_min {
        metrics.push(("other way passes at".to_string(), format!("{p:.2} b/t")));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The judge (`mctas circuit verify`): the run at N positions x 4 rotations.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no kind"))?;
    if !matches!(kind, Kind::Station(_)) {
        bail!("{}: not a station", kind.name());
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
    let mut schedule = Schedule::new(c.inputs.len());
    schedule.ticks = 0;
    let mut rep = crate::verify::verdict(VerifyReport { name: c.name().to_string(), kind: Some(kind), blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false, note: String::new() });
    if !rep.pass {
        return Ok(rep);
    }
    // the worst over the builds
    let num = |k: &BuildOk, name: &str| k.outcome.metrics.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.split_whitespace().next()).and_then(|v| v.parse::<f64>().ok());
    let oks: Vec<&BuildOk> = rep.builds.iter().filter_map(|b| b.result.as_ref().ok()).filter(|k| k.outcome.pass).collect();
    let stop = oks.iter().filter_map(|k| num(k, "stop distance")).fold(0.0f64, f64::max);
    let delay = oks.iter().filter_map(|k| num(k, "departure delay")).fold(0.0f64, f64::max);
    let leave = oks.iter().filter_map(|k| num(k, "leaves at")).fold(f64::MAX, f64::min);
    if let Some(b) = rep.builds.iter_mut().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && b.result.as_ref().is_ok_and(|k| k.outcome.pass))
        && let Ok(k) = &mut b.result
    {
        for (n, v) in k.outcome.metrics.iter_mut() {
            match n.as_str() {
                "stop distance" => *v = format!("{stop:.2} blocks"),
                "departure delay" => *v = format!("{} gt", delay as u32),
                "leaves at" => *v = format!("{leave:.2} b/t"),
                _ => {}
            }
        }
    }
    Ok(rep)
}

/// A design aid (`mctas circuit station-run`): one run turned and moved like one of `circuit
/// verify`'s builds.
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

    /// The two-slope station's blocks and the judge's rails beside it.
    fn slopes() -> std::collections::BTreeMap<Pos, String> {
        let mut m = std::collections::BTreeMap::new();
        for p in [[-1, 2, 0], [0, 1, 0], [1, 0, 0], [-2, 2, 0], [2, 0, 0]] {
            m.insert(p, "minecraft:stone".to_string());
        }
        for p in [[0, 2, 0], [1, 1, 0]] {
            m.insert(p, "minecraft:powered_rail[powered=false,shape=ascending_west]".to_string());
        }
        for p in [[-1, 3, 0], [-2, 3, 0], [2, 1, 0]] {
            m.insert(p, "minecraft:rail[shape=east_west]".to_string());
        }
        m
    }

    fn eye(p: [f64; 3]) -> [f64; 3] {
        [p[0], p[1] + EYE, p[2]]
    }

    #[test]
    fn button_sight() {
        let rests = [[1.60, 1.46, 0.5], [0.01, 3.05, 0.5], [0.81, 2.26, 0.5]];
        // a button on the north face of the slope's support: hidden behind that block
        let mut m = slopes();
        m.insert([0, 1, -1], "minecraft:stone_button[face=wall,facing=north,powered=false]".to_string());
        for r in rests {
            assert!(!clickable(&m, eye(r), [0, 1, -1]), "hidden button seen from {r:?}");
        }
        // a floor button beside the track, below every rest's eyes: in sight
        let mut m = slopes();
        m.insert([1, 1, -1], "minecraft:stone".to_string());
        m.insert([1, 2, -1], "minecraft:stone_button[face=floor,facing=north,powered=false]".to_string());
        for r in rests {
            assert!(clickable(&m, eye(r), [1, 2, -1]), "floor button not seen from {r:?}");
        }
        // the same button on a block whose top is above the low rest's eyes: hidden from there
        let mut m = slopes();
        m.insert([0, 2, -1], "minecraft:stone".to_string());
        m.insert([0, 3, -1], "minecraft:stone_button[face=floor,facing=north,powered=false]".to_string());
        assert!(!clickable(&m, eye(rests[0]), [0, 3, -1]));
        assert!(clickable(&m, eye(rests[1]), [0, 3, -1]));
        // out of reach
        let mut m = slopes();
        m.insert([1, 2, -6], "minecraft:stone_button[face=floor,facing=north,powered=false]".to_string());
        assert!(!clickable(&m, eye(rests[0]), [1, 2, -6]));
    }
}
