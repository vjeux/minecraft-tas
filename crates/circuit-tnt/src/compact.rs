//! `mctas circuit tnt compact` (track clk-rt with clk-misc, 2026-10-21; vjeux 02:47Z "compact all
//! the entire build ... treat this as the door experiment where we want to find the minimum
//! volume"): the MINIMUM-VOLUME PRECISE DROP as a box-bounded search on redsim.
//!
//! Each worker runs an annealing chain from the seed design. Moves stay inside `--region` and never
//! touch `--frozen` cells: delete a block, move one (to a neighbor cell or next to the design),
//! replace one by a palette state, add one, turn a directional block, swap two, translate a cluster
//! of touching blocks by one cell, relocate a cluster anywhere (turned 0-3 quarter turns), or push
//! a whole face layer of the bounding box one cell inward. Ports (the clock's `start` block placed
//! last, ...) follow their blocks. A big move (cluster / relocate / face) that breaks the design
//! gets a short repair burst of local moves around the cells it touched.
//! A candidate is scored in tiers (higher is better, then by its own partial score, then smaller
//! volume, then fewer blocks):
//!   0. does not dupe one TNT per period (`search::dupes`);
//!   1. dupes; partial = the largest group of identical fuse-1 positions of its `nth` TNT over the
//!      screening kicks (`precise::ends`);
//!   2. one fuse-1 point over every screening kick; partial = minus the distance of its burst depth
//!      (blocks below the machine's lowest block) to `--depth`;
//!   3. the burst depth is in range; the redsim judge (`circuit::verify::sim`) still fails;
//!   4. the judge passes: `precise: N TNT at one point at fuse 1, in mid-air D blocks below the
//!      machine` with D in `--depth`, `design intact` (survives its own explosions) and `useful:`.
//! The volume is the bounding box of every non-air block (the judge's `AxBxC`, water included). A
//! chain accepts any candidate that scores no worse, a worse one of the same tier now and then
//! (plateaus), and restarts from the best design found when it goes stale. Every tier-4 design is
//! recorded; the (volume, blocks) Pareto front is written to DIR/front/ and DIR/front.tsv.

use crate::precise;
use crate::search::{dupes, expand};
use anyhow::Result;
use clock::{Clock, Pos};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct CompactOptions {
    pub seed: PathBuf,
    /// The cells blocks may occupy (default: the seed's bounding box).
    pub region: Option<(Pos, Pos)>,
    /// Cells never changed (their blocks, or their emptiness, stay).
    pub frozen: BTreeSet<Pos>,
    pub palette: Vec<String>,
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    /// Screening run length and the steady-state TNT it follows.
    pub ticks: u32,
    pub nth: usize,
    /// Screening kicks (evenly sampled from the default kick list).
    pub kicks: usize,
    /// Where the burst must be: `Below(lo, hi)` blocks below the machine's lowest block (a precise
    /// drop), anywhere below (`AnyBelow`), or an air burst (`Above(clear)`: not below the machine,
    /// at least `clear` blocks from its box on the screen; the judge: no "below the machine", safe
    /// drop 0 blocks).
    pub target: Target,
    pub out: PathBuf,
    /// Stop a chain's plateau walk after this many rejected moves and restart from the best.
    pub stale: usize,
    /// Local moves tried around a big move (cluster translate / relocate / face push) that broke
    /// the design, before it is rejected.
    pub repair: usize,
}

/// Where a passing design's burst must be (see `CompactOptions::target`).
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Below(f64, f64),
    AnyBelow,
    Above(f64),
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const DIRS6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A search state: the blocks and the ports (which follow their blocks).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct St {
    pub b: BTreeMap<Pos, String>,
    pub ports: BTreeMap<String, Pos>,
}

impl St {
    /// The seed's design with this state's blocks and ports.
    fn design(&self, seed: &Clock) -> mctas::design::Design {
        let mut d = seed.design.clone();
        d.blocks = self.b.clone();
        d.ports = self.ports.clone();
        d
    }
    /// Translation-free key (blocks and ports relative to the box's low corner).
    fn key(&self) -> (Vec<(Pos, String)>, Vec<(String, Pos)>) {
        let lo = self.b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
        let rel = |p: &Pos| [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
        (self.b.iter().map(|(p, s)| (rel(p), s.clone())).collect(), self.ports.iter().map(|(n, p)| (n.clone(), rel(p))).collect())
    }
}

type Key = (Vec<(Pos, String)>, Vec<(String, Pos)>);

/// A candidate's score: compared field by field (derive order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score {
    pub tier: u8,
    /// Minus the blocks outside `--region` (a squeeze: a seed bigger than the region is moved in,
    /// one block at a time, never at the cost of its tier).
    pub outside: i64,
    pub partial: i64,
    /// Minus the volume, minus the block count (bigger is better).
    pub neg_volume: i64,
    pub neg_blocks: i64,
}

/// The bounding-box volume and its size.
pub fn volume(b: &BTreeMap<Pos, String>) -> (i64, [i32; 3]) {
    if b.is_empty() {
        return (0, [0; 3]);
    }
    let lo = b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
    let hi = b.keys().fold([i32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
    let s = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    (s[0] as i64 * s[1] as i64 * s[2] as i64, s)
}

/// The burst depth the judge's verdict states (`in mid-air D blocks below the machine`).
fn verdict_depth(why: &str) -> Option<f64> {
    let i = why.find(" blocks below the machine")?;
    let head = &why[..i];
    head.rsplit(' ').next()?.parse().ok()
}

fn base_score(st: &St, o: &CompactOptions, tier: u8, partial: i64) -> Score {
    let outside = match o.region {
        Some((lo, hi)) => st.b.keys().filter(|p| !(0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i])).count() as i64,
        None => 0,
    };
    // tiers 0-1 (not precise): no reward for a smaller box (it would just delete the machine)
    let (nv, nb) = if tier >= 2 { (-volume(&st.b).0, -(st.b.len() as i64)) } else { (0, 0) };
    Score { tier, outside: -outside, partial, neg_volume: nv, neg_blocks: nb }
}

/// The screen of one state (tiers 0-3, no judge): (score, why). Tier 3 means "ready for the
/// judge" (`judge` decides tier 4).
pub fn screen(seed: &Clock, st: &St, o: &CompactOptions, kicks: &[f64]) -> (Score, String) {
    let mk = |tier: u8, partial: i64| base_score(st, o, tier, partial);
    let Ok(c) = Clock::from_design(st.design(seed)) else { return (mk(0, 0), "not a design".into()) };
    match dupes(&c, &[], o.ticks.min(260)) {
        Some((_, per)) if per == 1 => {}
        Some((p, per)) => return (mk(0, 0), format!("dupes {per} per {p} gt")),
        None => return (mk(0, 0), "does not dupe".into()),
    }
    // the screening kicks: the nth TNT's fuse-1 position each
    let mut groups: Vec<([f64; 3], usize)> = Vec::new();
    for &u in kicks {
        if let Ok(v) = precise::ends(&c, u, o.ticks)
            && v.len() > o.nth
        {
            let p = v[o.nth].1;
            match groups.iter_mut().find(|(q, _)| q.iter().zip(&p).all(|(a, b)| a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0))) {
                Some(g) => g.1 += 1,
                None => groups.push((p, 1)),
            }
        }
    }
    groups.sort_by(|a, b| b.1.cmp(&a.1));
    let best = groups.first().map(|g| g.1).unwrap_or(0);
    if best < kicks.len() {
        return (mk(1, best as i64), format!("{best} of {} kicks at one fuse-1 point", kicks.len()));
    }
    let lo_y = st.b.keys().map(|p| p[1]).min().unwrap_or(0) as f64;
    let depth = lo_y - groups[0].0[1];
    match o.target {
        Target::Below(dlo, dhi) => {
            let miss = if depth < dlo { dlo - depth } else if depth > dhi { depth - dhi } else { 0.0 };
            if miss > 0.0 {
                return (mk(2, -(miss * 100.0).round() as i64), format!("precise on the screen, burst {depth:.1} blocks below"));
            }
        }
        Target::AnyBelow => {
            if depth <= 0.0 {
                return (mk(2, (depth * 100.0).round() as i64), format!("precise on the screen, burst {depth:.1} blocks below (not below the machine)"));
            }
        }
        Target::Above(clear) => {
            // an air burst: not below the machine, `clear` blocks or more from its box
            let p = groups[0].0;
            let lo = st.b.keys().fold([i32::MAX; 3], |a, q| [a[0].min(q[0]), a[1].min(q[1]), a[2].min(q[2])]);
            let hi = st.b.keys().fold([i32::MIN; 3], |a, q| [a[0].max(q[0]), a[1].max(q[1]), a[2].max(q[2])]);
            let gap = |i: usize| (lo[i] as f64 - p[i]).max(p[i] - (hi[i] as f64 + 1.0)).max(0.0);
            let dist = (gap(0).powi(2) + gap(1).powi(2) + gap(2).powi(2)).sqrt();
            if depth > 0.0 {
                return (mk(2, -10_000 - (depth * 100.0).round() as i64), format!("precise on the screen, burst {depth:.1} blocks below the machine (an air burst must not be)"));
            }
            if dist < clear {
                return (mk(2, -((clear - dist) * 100.0).round() as i64), format!("precise on the screen, burst {dist:.1} blocks from the machine (wants {clear})"));
            }
            return (mk(3, 0), format!("screen passes, air burst {dist:.1} blocks from the machine"));
        }
    }
    (mk(3, 0), format!("screen passes, burst {depth:.1} blocks below"))
}

/// The redsim judge on a state the screen passed: tier 4 when it passes with every flag.
pub fn judge(seed: &Clock, st: &St, o: &CompactOptions) -> (Score, String) {
    let mk = |tier: u8, partial: i64| base_score(st, o, tier, partial);
    let Ok(cc) = circuit::Circuit::from_design(st.design(seed)) else { return (mk(3, 0), "the judge cannot load it".into()) };
    let rep = match circuit::verify::sim(&cc, &circuit::verify::Options::default()) {
        Ok(r) => r,
        Err(e) => return (mk(3, 0), format!("judge error: {e:#}")),
    };
    let Some(out) = rep.outcome else { return (mk(3, 0), "no verdict".into()) };
    if !out.pass {
        return (mk(3, 0), format!("judge: {}", out.why.chars().take(160).collect::<String>()));
    }
    let w = &out.why;
    let metrics: String = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
    let jd = verdict_depth(w);
    let target_ok = match (o.target, jd) {
        (Target::Below(dlo, dhi), Some(x)) => x >= dlo - 1e-9 && x <= dhi + 1e-9,
        (Target::AnyBelow, Some(_)) => true,
        (Target::Above(_), None) => !w.contains("below the machine") && (metrics.contains("safe drop 0 blocks") || w.contains("safe drop 0 blocks")),
        _ => false,
    };
    if !(w.contains("precise:") && w.contains("in mid-air") && target_ok && w.contains("design intact") && w.contains("useful:")) {
        return (mk(3, 1), format!("judge passes without every flag: {}; {}", metrics.chars().take(120).collect::<String>(), w.chars().take(200).collect::<String>()));
    }
    let rate = out.metrics.iter().find(|(n, _)| n == "rate").map(|(_, v)| v.clone()).unwrap_or_default();
    (mk(4, 0), format!("{rate}; {metrics}; {w}"))
}

/// Screen then judge (the seed's own evaluation).
pub fn evaluate(seed: &Clock, st: &St, o: &CompactOptions, kicks: &[f64]) -> (Score, String) {
    let s = screen(seed, st, o, kicks);
    if s.0.tier < 3 { s } else { judge(seed, st, o) }
}

/// A recorded tier-4 design.
#[derive(Clone)]
struct Found {
    st: St,
    verdict: String,
}

/// The state with its `facing=` turned to another allowed value (None: not directional).
fn turned(s: &str, r: &mut Rng) -> Option<String> {
    let i = s.find("facing=")? + "facing=".len();
    let j = s[i..].find([',', ']']).map(|k| i + k)?;
    let cur = &s[i..j];
    let id = s.split('[').next().unwrap_or(s);
    let horizontal = !(id.ends_with("observer") || id.ends_with("piston") || id.ends_with("dispenser") || id.ends_with("dropper"));
    let opts: &[&str] = if horizontal { &["north", "east", "south", "west"] } else { &["north", "east", "south", "west", "up", "down"] };
    let alts: Vec<&&str> = opts.iter().filter(|o| **o != cur).collect();
    if alts.is_empty() {
        return None;
    }
    let n = alts[r.below(alts.len())];
    Some(format!("{}{}{}", &s[..i], n, &s[j..]))
}

struct Space<'a> {
    region: (Pos, Pos),
    frozen: &'a BTreeSet<Pos>,
}

impl Space<'_> {
    fn inside(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.region.0[i] && p[i] <= self.region.1[i])
    }
    fn free(&self, b: &BTreeMap<Pos, String>, p: Pos) -> bool {
        self.inside(p) && !self.frozen.contains(&p) && !b.contains_key(&p)
    }
    fn movable(&self, p: Pos) -> bool {
        !self.frozen.contains(&p)
    }
}

/// A move's result: the new state, the cells it changed (old and new), whether it was a big
/// (structural) move worth a repair burst.
struct Moved {
    st: St,
    touched: Vec<Pos>,
    big: bool,
}

/// A connected cluster of movable blocks from `pick` (random BFS order, up to `size`).
fn cluster(r: &mut Rng, b: &BTreeMap<Pos, String>, sp: &Space, pick: Pos, size: usize) -> Vec<Pos> {
    let mut cl: Vec<Pos> = vec![pick];
    let mut seen: BTreeSet<Pos> = BTreeSet::from([pick]);
    let mut i = 0;
    while i < cl.len() && cl.len() < size {
        let c = cl[i];
        i += 1;
        let mut ns: Vec<Pos> = DIRS6.iter().map(|d| add(c, *d)).filter(|q| b.contains_key(q) && sp.movable(*q) && !seen.contains(q)).collect();
        for k in (1..ns.len()).rev() {
            ns.swap(k, r.below(k + 1));
        }
        for q in ns {
            if cl.len() >= size {
                break;
            }
            seen.insert(q);
            cl.push(q);
        }
    }
    cl
}

/// One random move; None when the move does not apply. `near`: only cells within 2 of these
/// (a repair burst's local moves: delete, step, replace, add, turn).
fn mutate(r: &mut Rng, cur: &St, sp: &Space, pal: &[String], near: Option<&[Pos]>) -> Option<Moved> {
    let mut b = cur.b.clone();
    let port_cells: BTreeSet<Pos> = cur.ports.values().copied().filter(|p| b.contains_key(p)).collect();
    let close = |p: Pos| near.is_none_or(|n| n.iter().any(|q| (0..3).all(|i| (p[i] - q[i]).abs() <= 2)));
    let keys: Vec<Pos> = b.keys().copied().filter(|k| sp.movable(*k) && close(*k)).collect();
    let tnts = b.values().filter(|s| s.contains("minecraft:tnt")).count();
    let next_to = |r: &mut Rng, b: &BTreeMap<Pos, String>| -> Option<Pos> {
        let all: Vec<Pos> = b.keys().copied().filter(|p| close(*p)).collect();
        if all.is_empty() {
            return None;
        }
        for _ in 0..40 {
            let c = all[r.below(all.len())];
            let q = add(c, DIRS6[r.below(6)]);
            if sp.free(b, q) && close(q) {
                return Some(q);
            }
        }
        None
    };
    let mut touched: Vec<Pos> = Vec::new();
    // block moves (from, to): the ports follow them
    let mut moves: Vec<(Pos, Pos)> = Vec::new();
    let mut big = false;
    let roll = if near.is_some() { [0, 10, 35, 43, 51][r.below(5)] + r.below(8) } else { r.below(100) };
    if keys.is_empty() && !(43..=50).contains(&roll) {
        return None;
    }
    let pick = if keys.is_empty() { [0; 3] } else { keys[r.below(keys.len())] };
    // the last TNT and port blocks are never deleted or replaced
    let fixed_kind = (b.get(&pick).is_some_and(|s| s.contains("minecraft:tnt")) && tnts <= 1) || port_cells.contains(&pick);
    match roll {
        // delete
        0..=9 => {
            if fixed_kind {
                return None;
            }
            b.remove(&pick);
            touched.push(pick);
        }
        // step one cell
        10..=24 => {
            let q = add(pick, DIRS6[r.below(6)]);
            if !sp.free(&b, q) {
                return None;
            }
            let s = b.remove(&pick)?;
            b.insert(q, s);
            touched.extend([pick, q]);
            moves.push((pick, q));
        }
        // move next to the design
        25..=34 => {
            let s = b.remove(&pick)?;
            let q = next_to(r, &b)?;
            b.insert(q, s);
            touched.extend([pick, q]);
            moves.push((pick, q));
        }
        // replace by a palette state
        35..=42 => {
            if fixed_kind || pal.is_empty() {
                return None;
            }
            b.insert(pick, pal[r.below(pal.len())].clone());
            touched.push(pick);
        }
        // add a palette block
        43..=50 => {
            if pal.is_empty() {
                return None;
            }
            let q = next_to(r, &b)?;
            b.insert(q, pal[r.below(pal.len())].clone());
            touched.push(q);
        }
        // turn
        51..=58 => {
            let t = turned(&b[&pick], r)?;
            b.insert(pick, t);
            touched.push(pick);
        }
        // swap two
        59..=64 => {
            let other = keys[r.below(keys.len())];
            if other == pick || b[&other] == b[&pick] {
                return None;
            }
            let (x, y) = (b[&pick].clone(), b[&other].clone());
            b.insert(pick, y);
            b.insert(other, x);
            touched.extend([pick, other]);
            moves.extend([(pick, other), (other, pick)]);
        }
        // translate a cluster of touching blocks by one cell
        65..=82 => {
            let size = 1 + r.below(16);
            let cl = cluster(r, &b, sp, pick, size);
            let d = DIRS6[r.below(6)];
            let set: BTreeSet<Pos> = cl.iter().copied().collect();
            if !cl.iter().all(|c| {
                let q = add(*c, d);
                set.contains(&q) || sp.free(&b, q)
            }) {
                return None;
            }
            let moved: Vec<(Pos, Pos, String)> = cl.iter().map(|c| (*c, add(*c, d), b[c].clone())).collect();
            for c in &cl {
                b.remove(c);
            }
            for (c, q, s) in moved {
                touched.extend([c, q]);
                moves.push((c, q));
                b.insert(q, s);
            }
            big = cl.len() > 1;
        }
        // relocate a cluster anywhere in the region, turned 0-3 quarter turns about y
        83..=91 => {
            let size = 2 + r.below(19);
            let cl = cluster(r, &b, sp, pick, size);
            let turns = r.below(4) as u8;
            let set: BTreeSet<Pos> = cl.iter().copied().collect();
            let (lo, hi) = sp.region;
            let anchor = [lo[0] + r.below((hi[0] - lo[0] + 1) as usize) as i32, lo[1] + r.below((hi[1] - lo[1] + 1) as usize) as i32, lo[2] + r.below((hi[2] - lo[2] + 1) as usize) as i32];
            let moved: Vec<(Pos, Pos, String)> = cl
                .iter()
                .map(|c| {
                    let rel = clock::rotate_pos([c[0] - pick[0], c[1] - pick[1], c[2] - pick[2]], turns);
                    (*c, add(anchor, rel), clock::canon::rotate_state(&b[c], turns))
                })
                .collect();
            if !moved.iter().all(|(_, q, _)| set.contains(q) || sp.free(&b, *q)) {
                return None;
            }
            for c in &cl {
                b.remove(c);
            }
            for (c, q, s) in moved {
                touched.extend([c, q]);
                moves.push((c, q));
                b.insert(q, s);
            }
            big = true;
        }
        // push a face layer of the bounding box one cell inward
        _ => {
            let lo = b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
            let hi = b.keys().fold([i32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
            let axis = r.below(3);
            let low_face = r.below(2) == 0;
            let (at, step) = if low_face { (lo[axis], 1) } else { (hi[axis], -1) };
            let layer: Vec<Pos> = b.keys().copied().filter(|p| p[axis] == at).collect();
            if layer.is_empty() || layer.iter().any(|p| !sp.movable(*p)) {
                return None;
            }
            let mut d = [0; 3];
            d[axis] = step;
            if !layer.iter().all(|p| sp.free(&b, add(*p, d))) {
                return None;
            }
            for p in layer {
                let s = b.remove(&p)?;
                b.insert(add(p, d), s);
                touched.extend([p, add(p, d)]);
                moves.push((p, add(p, d)));
            }
            big = true;
        }
    }
    let mut ports = cur.ports.clone();
    for p in ports.values_mut() {
        if let Some((_, to)) = moves.iter().find(|(from, _)| from == p) {
            *p = *to;
        }
    }
    let st = St { b, ports };
    if &st == cur { None } else { Some(Moved { st, touched, big }) }
}

/// The palette shorthands of `compact` on top of `search::expand`'s: water, fence, trapdoor (iron,
/// top half, closed, 4 facings), repeater (4 facings x delays 1-4), walltorch, torch, lamp.
fn expand_compact(name: &str) -> Result<Vec<String>> {
    let dirs = ["north", "east", "south", "west"];
    Ok(match name {
        "water" => vec!["minecraft:water[level=0]".into()],
        "fence" => vec!["minecraft:oak_fence[east=false,north=false,south=false,waterlogged=false,west=false]".into()],
        "trapdoor" => dirs.iter().map(|d| format!("minecraft:iron_trapdoor[facing={d},half=top,open=false,powered=false,waterlogged=false]")).collect(),
        "repeater" => dirs.iter().flat_map(|d| (1..=4).map(move |k| format!("minecraft:repeater[delay={k},facing={d},locked=false,powered=false]"))).collect(),
        "walltorch" => dirs.iter().map(|d| format!("minecraft:redstone_wall_torch[facing={d},lit=true]")).collect(),
        "torch" => vec!["minecraft:redstone_torch[lit=true]".into()],
        "lamp" => vec!["minecraft:redstone_lamp[lit=false]".into()],
        _ => expand(name)?,
    })
}

pub fn compact(o: &CompactOptions) -> Result<usize> {
    let seed = Clock::load(&o.seed)?;
    let mut pal: Vec<String> = Vec::new();
    for n in &o.palette {
        pal.extend(expand_compact(n)?);
    }
    for s in &pal {
        redsim::Block::parse(s).map_err(|e| anyhow::anyhow!("{s}: {e}"))?;
    }
    let (slo, shi) = seed.design.bounds().ok_or_else(|| anyhow::anyhow!("empty seed"))?;
    let region = o.region.unwrap_or((slo, shi));
    let all = precise::default_kicks();
    let k = o.kicks.clamp(1, all.len());
    let kicks: Vec<f64> = (0..k).map(|i| all[i * all.len() / k]).collect();
    let st0 = St { b: seed.design.blocks.clone(), ports: seed.design.ports.clone() };
    let (s0, v0) = evaluate(&seed, &st0, o, &kicks);
    let (vol0, size0) = volume(&st0.b);
    eprintln!("seed: tier {} partial {} volume {vol0} ({}x{}x{}), {} blocks: {v0}", s0.tier, s0.partial, size0[0], size0[1], size0[2], st0.b.len());
    if st0.b.keys().any(|p| !(0..3).all(|i| p[i] >= region.0[i] && p[i] <= region.1[i])) {
        eprintln!("the seed has blocks outside --region: a squeeze (blocks outside are moved in first; only all-inside designs are recorded)");
    }
    let sp = Space { region, frozen: &o.frozen };
    let found: Mutex<HashMap<Key, Found>> = Mutex::new(HashMap::new());
    let best: Mutex<(Score, St)> = Mutex::new((s0, st0.clone()));
    // the judge's verdicts (shared: chains meet the same designs) and a gate: the judge runs its
    // kick sweep on every core itself, so one judge at a time
    let judged: Mutex<HashMap<Key, (Score, String)>> = Mutex::new(HashMap::new());
    let gate: Mutex<()> = Mutex::new(());
    let iter = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..o.workers.max(1) {
            let (seed, pal, found, best, iter, kicks, sp, judged, gate, st0) = (&seed, &pal, &found, &best, &iter, &kicks, &sp, &judged, &gate, &st0);
            sc.spawn(move || {
                let mut r = Rng(o.rng.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 * 104729 + 7) | 1);
                let mut cache: HashMap<St, (Score, String)> = HashMap::new();
                // screen (cached), then the judge when the candidate could be accepted as tier 4
                let mut eval = |cand: &St, cur_s: Score, walk: bool| -> (Score, String) {
                    let s = match cache.get(cand) {
                        Some(v) => v.clone(),
                        None => {
                            let v = screen(seed, cand, o, kicks);
                            if cache.len() > 50_000 {
                                cache.clear();
                            }
                            cache.insert(cand.clone(), v.clone());
                            v
                        }
                    };
                    if s.0.tier < 3 {
                        return s;
                    }
                    let potential = Score { tier: 4, partial: 0, ..s.0 };
                    if potential < cur_s && !walk {
                        return s;
                    }
                    let key = cand.key();
                    if let Some(v) = judged.lock().unwrap().get(&key) {
                        return v.clone();
                    }
                    let v = {
                        let _g = gate.lock().unwrap();
                        judge(seed, cand, o)
                    };
                    judged.lock().unwrap().insert(key, v.clone());
                    v
                };
                let mut cur = st0.clone();
                let mut cur_s = s0;
                let mut stale = 0usize;
                loop {
                    let i = iter.fetch_add(1, Ordering::Relaxed);
                    if i >= o.iterations {
                        break;
                    }
                    if i > 0 && i % 2000 == 0 {
                        let g = best.lock().unwrap();
                        let (v, s) = volume(&g.1.b);
                        let n4 = found.lock().unwrap().len();
                        eprintln!("  {i}/{} moves, {:.0} s: best tier {} partial {} volume {v} ({}x{}x{}) {} blocks; {n4} passing designs", o.iterations, t0.elapsed().as_secs_f64(), g.0.tier, g.0.partial, s[0], s[1], s[2], g.1.b.len());
                    }
                    let Some(mv) = mutate(&mut r, &cur, sp, pal, None) else { continue };
                    let walk = r.below(25) == 0;
                    let (mut sc_, mut verdict) = eval(&mv.st, cur_s, walk);
                    let mut cand = mv.st;
                    // a big move that broke the design: a short local repair burst around the
                    // cells it touched, greedy on the score
                    if mv.big && sc_ < cur_s && o.repair > 0 {
                        for _ in 0..o.repair {
                            let Some(m2) = mutate(&mut r, &cand, sp, pal, Some(&mv.touched)) else { continue };
                            let (s2, v2) = eval(&m2.st, cur_s, false);
                            if s2 > sc_ {
                                sc_ = s2;
                                verdict = v2;
                                cand = m2.st;
                                if sc_ >= cur_s {
                                    break;
                                }
                            }
                        }
                    }
                    if sc_.tier == 4 {
                        let mut g = found.lock().unwrap();
                        let fresh = !g.contains_key(&cand.key());
                        g.entry(cand.key()).or_insert_with(|| Found { st: cand.clone(), verdict: verdict.clone() });
                        if fresh {
                            // every passing design is written at once (a long run can be read while it goes)
                            let (v, _) = volume(&cand.b);
                            let mut d = cand.design(seed);
                            d.name = format!("{}-v{v}-{}b-{}", seed.design.name, cand.b.len(), g.len());
                            let _ = std::fs::create_dir_all(o.out.join("found"));
                            let _ = std::fs::write(o.out.join("found").join(format!("{}.txt", d.name)), format!("# {verdict}\n{}", d.to_text()));
                        }
                    }
                    {
                        let mut g = best.lock().unwrap();
                        if sc_ > g.0 {
                            let (v, s) = volume(&cand.b);
                            eprintln!("  NEW BEST tier {} partial {} volume {v} ({}x{}x{}) {} blocks: {}", sc_.tier, sc_.partial, s[0], s[1], s[2], cand.b.len(), verdict.chars().take(160).collect::<String>());
                            *g = (sc_, cand.clone());
                            let mut d = cand.design(seed);
                            d.name = format!("{}-best", seed.design.name);
                            let _ = std::fs::create_dir_all(&o.out);
                            let _ = std::fs::write(o.out.join("best.txt"), format!("# tier {} {verdict}\n{}", sc_.tier, d.to_text()));
                        }
                    }
                    let same_tier_walk = sc_.tier == cur_s.tier && walk;
                    if sc_ >= cur_s || same_tier_walk {
                        if sc_ > cur_s {
                            stale = 0;
                        } else {
                            stale += 1;
                        }
                        cur = cand;
                        cur_s = sc_;
                    } else {
                        stale += 1;
                    }
                    if stale > o.stale {
                        let g = best.lock().unwrap();
                        cur = g.1.clone();
                        cur_s = g.0;
                        stale = 0;
                    }
                }
            });
        }
    });
    let g = found.into_inner().unwrap();
    let all: Vec<&Found> = g.values().collect();
    let key = |f: &Found| (volume(&f.st.b).0, f.st.b.len());
    let mut front: Vec<&Found> = all.iter().copied().filter(|f| !all.iter().any(|h| key(h).0 <= key(f).0 && key(h).1 <= key(f).1 && key(h) != key(f))).collect();
    front.sort_by_key(|f| key(f));
    // at most 3 designs per (volume, blocks)
    let mut per_key: HashMap<(i64, usize), usize> = HashMap::new();
    front.retain(|f| {
        let n = per_key.entry(key(f)).or_insert(0);
        *n += 1;
        *n <= 3
    });
    std::fs::create_dir_all(o.out.join("front"))?;
    let mut tsv = String::from("name\tvolume\tsize\tblocks\tverdict\n");
    for (i, f) in front.iter().enumerate() {
        let (v, s) = volume(&f.st.b);
        let name = format!("{}-v{v}-{}b-{i}", seed.design.name, f.st.b.len());
        let mut d = f.st.design(&seed);
        d.name = name.clone();
        std::fs::write(o.out.join("front").join(format!("{name}.txt")), d.to_text())?;
        tsv += &format!("{name}\t{v}\t{}x{}x{}\t{}\t{}\n", s[0], s[1], s[2], f.st.b.len(), f.verdict);
    }
    std::fs::write(o.out.join("front.tsv"), tsv)?;
    let b = best.into_inner().unwrap();
    let mut d = b.1.design(&seed);
    d.name = format!("{}-best", seed.design.name);
    std::fs::write(o.out.join("best.txt"), d.to_text())?;
    eprintln!("  {} passing designs, {} on the (volume, blocks) front; best tier {} written to {}", g.len(), front.len(), b.0.tier, o.out.join("best.txt").display());
    Ok(front.len())
}
