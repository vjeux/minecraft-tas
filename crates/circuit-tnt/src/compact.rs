//! `mctas circuit tnt compact` (track clk-rt with clk-misc, 2026-10-21; vjeux 02:47Z "compact all
//! the entire build ... treat this as the door experiment where we want to find the minimum
//! volume"): the MINIMUM-VOLUME PRECISE DROP as a box-bounded search on redsim.
//!
//! The moves and the chain loop are `circuit::anneal`'s (shared with `circuit compact` for every
//! other kind): annealing chains from the seed inside `--region`, never touching `--frozen` cells,
//! ports following their blocks, repair bursts after big moves. The last TNT is never deleted or
//! replaced. A candidate is scored in tiers (higher is better, then by its own partial score, then
//! smaller volume, then fewer blocks):
//!   0. does not dupe one TNT per period (`search::dupes`);
//!   1. dupes; partial = the largest group of identical fuse-1 positions of its `nth` TNT over the
//!      screening kicks (`precise::ends`);
//!   2. one fuse-1 point over every screening kick; partial = minus the distance of its burst depth
//!      (blocks below the machine's lowest block) to `--depth`;
//!   3. the burst depth is in range; the redsim judge (`circuit::verify::sim`) still fails;
//!   4. the judge passes: `precise: N TNT at one point at fuse 1, in mid-air D blocks below the
//!      machine` with D in `--depth`, `design intact` (survives its own explosions) and `useful:`.
//! The volume of a tier-4 design is the judge's box (every non-air cell over the run). Every tier-4
//! design is recorded; the (volume, blocks) Pareto front is written to DIR/front/ and
//! DIR/front.tsv, and `--game N` judges the N smallest in the real game (DIR/game.tsv).

use crate::precise;
use crate::search::{dupes, expand};
use anyhow::Result;
use circuit::anneal::{self, Chains, Key, Space};
pub use circuit::anneal::{Score, St, volume};
use clock::{Clock, Pos};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;

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
    /// Game-judge (`circuit verify`) this many of the smallest front designs at the end.
    pub game: usize,
}

/// Where a passing design's burst must be (see `CompactOptions::target`).
#[derive(Clone, Copy, Debug)]
pub enum Target {
    Below(f64, f64),
    AnyBelow,
    Above(f64),
    /// (clk-misc, coordinator 2026-10-21: the minimum-volume FAST duper) not precise: at least
    /// this many TNT a minute, safe on the ground (`safe drop 0 blocks`), surviving its own
    /// explosions and useful. Screen: the rate, then strict runs on both grounds at drop 0
    /// (`circuit::robust::passes_on_ground`, 4 kicks).
    Fast(f64),
}

/// The burst depth the judge's verdict states (`in mid-air D blocks below the machine`).
fn verdict_depth(why: &str) -> Option<f64> {
    let i = why.find(" blocks below the machine")?;
    let head = &why[..i];
    head.rsplit(' ').next()?.parse().ok()
}

fn base_score(st: &St, o: &CompactOptions, tier: u8, partial: i64) -> Score {
    let outside = o.region.map_or(0, |r| st.outside(r));
    // tiers 0-1 (not precise): no reward for a smaller box (it would just delete the machine)
    let (nv, nb) = if tier >= 2 { (-volume(&st.b).0, -(st.b.len() as i64)) } else { (0, 0) };
    Score { tier, outside: -outside, partial, neg_volume: nv, neg_blocks: nb }
}

/// The screen of one state (tiers 0-3, no judge): (score, why). Tier 3 means "ready for the
/// judge" (`judge` decides tier 4).
pub fn screen(seed: &Clock, st: &St, o: &CompactOptions, kicks: &[f64]) -> (Score, String) {
    let mk = |tier: u8, partial: i64| base_score(st, o, tier, partial);
    let Ok(c) = Clock::from_design(st.design(&seed.design)) else { return (mk(0, 0), "not a design".into()) };
    // the FAST duper (Target::Fast): the rate, then safe on the ground on the screen
    if let Target::Fast(min_rate) = o.target {
        let rate = match dupes(&c, &[], o.ticks.min(260)) {
            Some((p, per)) if p > 0 => per as f64 * 1200.0 / p as f64,
            _ => return (mk(0, 0), "does not dupe".into()),
        };
        if rate < min_rate - 1e-9 {
            return (mk(0, (rate * 10.0).round() as i64), format!("{rate:.1} TNT/min, under {min_rate}"));
        }
        let Ok(cc) = circuit::Circuit::from_design(st.design(&seed.design)) else { return (mk(1, 0), "the judge cannot load it".into()) };
        if !circuit::robust::passes_on_ground(&cc, 4) {
            return (mk(2, 0), format!("{rate:.1} TNT/min, not safe on the ground on the screen"));
        }
        return (mk(3, 0), format!("screen passes: {rate:.1} TNT/min, safe on the ground"));
    }
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
        // handled before the precise screen
        Target::Fast(_) => {}
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
    let Ok(cc) = circuit::Circuit::from_design(st.design(&seed.design)) else { return (mk(3, 0), "the judge cannot load it".into()) };
    let rep = match circuit::verify::sim(&cc, &circuit::verify::Options::default()) {
        Ok(r) => r,
        Err(e) => return (mk(3, 0), format!("judge error: {e:#}")),
    };
    let Some(out) = rep.outcome else { return (mk(3, 0), "no verdict".into()) };
    if !out.pass {
        return (mk(3, 0), format!("judge: {}", out.why.chars().take(160).collect::<String>()));
    }
    let w = &out.why;
    // redsim's judge checks the turned builds of a TNT duper: a rotation-dependent one is no record
    if w.contains("rotation-dependent") {
        return (mk(3, 0), format!("judge: {}", w.chars().take(160).collect::<String>()));
    }
    let metrics: String = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
    let jd = verdict_depth(w);
    let rate = out.metrics.iter().find(|(n, _)| n == "rate").map(|(_, v)| v.clone()).unwrap_or_default();
    // the judge's volume: the box of every non-air cell over the run (moving parts, flowing water)
    let judged = || -> (Score, String) {
        let mut s4 = mk(4, 0);
        let mut size = String::new();
        if let Ok(t) = &rep.trace {
            let d = circuit::verify::dims(&cc, t);
            s4.neg_volume = -(d[0] as i64 * d[1] as i64 * d[2] as i64);
            size = format!("judged box {}x{}x{} = {}; ", d[0], d[1], d[2], -s4.neg_volume);
        }
        (s4, size)
    };
    if let Target::Fast(min_rate) = o.target {
        let r: f64 = rate.split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let ok = metrics.contains("safe drop 0 blocks") && w.contains("design intact") && w.contains("useful:") && r >= min_rate - 1e-9;
        if !ok {
            return (mk(3, 1), format!("judge passes without every flag: {}; {}", metrics.chars().take(120).collect::<String>(), w.chars().take(200).collect::<String>()));
        }
        let (s4, size) = judged();
        return (s4, format!("{size}{rate}; {metrics}; {w}"));
    }
    let target_ok = match (o.target, jd) {
        (Target::Below(dlo, dhi), Some(x)) => x >= dlo - 1e-9 && x <= dhi + 1e-9,
        (Target::AnyBelow, Some(_)) => true,
        (Target::Above(_), None) => !w.contains("below the machine") && (metrics.contains("safe drop 0 blocks") || w.contains("safe drop 0 blocks")),
        _ => false,
    };
    if !(w.contains("precise:") && w.contains("in mid-air") && target_ok && w.contains("design intact") && w.contains("useful:")) {
        return (mk(3, 1), format!("judge passes without every flag: {}; {}", metrics.chars().take(120).collect::<String>(), w.chars().take(200).collect::<String>()));
    }
    let (s4, size) = judged();
    (s4, format!("{size}{rate}; {metrics}; {w}"))
}

/// Screen then judge (the seed's own evaluation).
pub fn evaluate(seed: &Clock, st: &St, o: &CompactOptions, kicks: &[f64]) -> (Score, String) {
    let s = screen(seed, st, o, kicks);
    if s.0.tier < 3 { s } else { judge(seed, st, o) }
}

/// The palette shorthands of `compact` on top of `search::expand`'s: water, fence, trapdoor (iron,
/// top half, closed, 4 facings), repeater (4 facings x 4 delays), walltorch, torch, lamp.
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
    let st0 = St::of(&seed.design);
    let (s0, v0) = evaluate(&seed, &st0, o, &kicks);
    let (vol0, size0) = volume(&st0.b);
    eprintln!("seed: tier {} partial {} volume {vol0} ({}x{}x{}), {} blocks: {v0}", s0.tier, s0.partial, size0[0], size0[1], size0[2], st0.b.len());
    if st0.outside(region) > 0 {
        eprintln!("the seed has blocks outside --region: a squeeze (blocks outside are moved in first; only all-inside designs are recorded)");
    }
    // the last TNT is never deleted or replaced
    let keep = |b: &BTreeMap<Pos, String>, p: Pos| b.get(&p).is_some_and(|s| s.contains("minecraft:tnt")) && b.values().filter(|s| s.contains("minecraft:tnt")).count() <= 1;
    let chains = Chains {
        seed: &seed.design,
        space: Space { region, frozen: &o.frozen },
        palette: &pal,
        iterations: o.iterations,
        workers: o.workers,
        rng: o.rng,
        stale: o.stale,
        repair: o.repair,
        pass_tier: 4,
        keep: &keep,
        out: &o.out,
        verbose: true,
    };
    // the judge's verdicts (shared: chains meet the same designs) and a gate: the judge runs its
    // kick sweep on every core itself, so one judge at a time
    let judged: Mutex<HashMap<Key, (Score, String)>> = Mutex::new(HashMap::new());
    let gate: Mutex<()> = Mutex::new(());
    let (seed_r, kicks_r, judged_r, gate_r) = (&seed, &kicks, &judged, &gate);
    let make_eval = || -> anneal::Eval<'_> {
        // screen (cached per chain), then the judge when the candidate could be accepted as tier 4
        let mut cache: HashMap<St, (Score, String)> = HashMap::new();
        Box::new(move |cand: &St, cur_s: Score, walk: bool| -> (Score, String) {
            let s = match cache.get(cand) {
                Some(v) => v.clone(),
                None => {
                    let v = screen(seed_r, cand, o, kicks_r);
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
            if let Some(v) = judged_r.lock().unwrap().get(&key) {
                return v.clone();
            }
            let v = {
                let _g = gate_r.lock().unwrap();
                judge(seed_r, cand, o)
            };
            judged_r.lock().unwrap().insert(key, v.clone());
            v
        })
    };
    let (g, b) = anneal::run(&chains, &st0, s0, &v0, make_eval);
    let front = anneal::front(&g, 3);
    anneal::write_front(&seed.design, &front, &o.out)?;
    // the real game on the smallest front designs (redsim can call a design precise that the game
    // does not): `circuit verify`, 16 builds each
    if o.game > 0 {
        let vo = circuit::verify::Options { workers: 8, ..Default::default() };
        let pool = circuit::verify::spawn_pool(&vo)?;
        let mut gt = String::from("name\tgame\tverdict\n");
        let mut seen: Vec<(i64, usize)> = Vec::new();
        for (i, f) in front.iter().enumerate() {
            let fk = (-f.score.neg_volume, f.st.b.len());
            if seen.len() >= o.game || seen.contains(&fk) {
                continue;
            }
            seen.push(fk);
            let mut d = f.st.design(&seed.design);
            d.name = anneal::front_name(&seed.design, f, i);
            let c = circuit::Circuit::from_design(d.clone())?;
            let rep = circuit::verify::verify_in_pool(&pool, &c, &vo)?;
            let line = rep.summary(&c);
            let target_ok = match o.target {
                Target::Below(dlo, dhi) => verdict_depth(&line).is_some_and(|x| x >= dlo - 1e-9 && x <= dhi + 1e-9),
                Target::AnyBelow => verdict_depth(&line).is_some(),
                Target::Above(_) => !line.contains("below the machine") && line.contains("safe drop 0 blocks"),
                Target::Fast(_) => line.contains("safe drop 0 blocks"),
            };
            let precise_ok = matches!(o.target, Target::Fast(_)) || (line.contains("precise:") && line.contains("in mid-air"));
            let ok = rep.pass && precise_ok && line.contains("design intact") && line.contains("useful:") && target_ok;
            eprintln!("  game: {} {}", d.name, if ok { "GAME-OK" } else { "game-fail" });
            gt += &format!("{}\t{}\t{line}\n", d.name, if ok { "GAME-OK" } else { "game-fail" });
        }
        std::fs::write(o.out.join("game.tsv"), gt)?;
    }
    let mut d = b.1.design(&seed.design);
    d.name = format!("{}-best", seed.design.name);
    std::fs::write(o.out.join("best.txt"), d.to_text())?;
    eprintln!("  {} passing designs, {} on the (volume, blocks) front; best tier {} written to {}", g.len(), front.len(), b.0.tier, o.out.join("best.txt").display());
    Ok(front.len())
}
