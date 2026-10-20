//! The TNT dupers' all-randoms pass rule (vjeux 2026-10-20: "make sure they pass every possible
//! random number generated, not just test 64"). A duper is run on redsim with its explosions (the
//! exact port, every ray at its largest roll: surviving the strongest roll is surviving every
//! roll), every primed TNT kicked by the same `d` (the kick angle's `nextDouble()`); the outcome
//! is piecewise constant in `d`, so `redsim::robust::critical_points` sweeps [0, 1) and bisects
//! every change down to 1e-12, and every piece and both sides of every change are judged. A pass
//! needs every one of them to pass with the same metrics (and the same PRECISE claim).
//!
//! Limits: the kicks of different TNT are tied together (one `d` for all); per-spawn kicks
//! (`World::tnt_kicks`) are the next step. The real-game judge spot-checks the result.

use crate::check::{self, Outcome};
use crate::{Circuit, Kind};
use std::collections::HashMap;
use std::sync::Mutex;

/// Sweep points over [0, 1) before the bisection.
pub const SAMPLES: usize = 4096;

/// The outcome of one kick, as the sweep compares them: pass or fail with its reason class, the
/// metrics, and the PRECISE claim.
fn code(out: &Result<Outcome, String>) -> (u64, String) {
    let text = match out {
        Ok(o) if o.pass => {
            let precise = if o.why.contains("precise:") { " precise" } else { "" };
            format!("pass {}{precise}", o.metrics_text())
        }
        Ok(o) => format!("FAIL {}", o.why.split(':').next().unwrap_or("")),
        Err(e) => format!("FAIL {}", e.split(':').next().unwrap_or("")),
    };
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (h, text)
}

/// The judge's verdict for one kick, on the schedule `s`, standing on `ground`, with no
/// out-of-range removal (every TNT flies on until it explodes).
pub fn judge_kick(c: &Circuit, s: &crate::stimulus::Schedule, kick: f64, margin: i32, ground: crate::Ground) -> Result<Outcome, String> {
    let kind = c.kind.clone().ok_or("no kind")?;
    let t = crate::run::tnt_kick_trace(c, s, margin, kick, ground, false)?;
    Ok(check::check(c, &kind, s, &t))
}

/// The all-kicks report, on every ground ([`crate::Ground::ALL`]): every piece of [0, 1) with its
/// outcome; `Err` with the first failing piece (or two passing pieces that measure differently).
pub fn tnt_robust(c: &Circuit, samples: usize, workers: usize) -> Result<String, String> {
    if !matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered)) || !c.explodes() {
        return Err("the all-kicks rule is for TNT dupers with explosions on".into());
    }
    let margin = 3;
    let o = crate::verify::Options { margin, ..Default::default() };
    let (s, _) = crate::verify::schedule(c, &o).map_err(|e| format!("{e:#}"))?;
    tnt_robust_at(c, &s, margin, samples, workers)
}

/// The deepest drop the judge tries (vjeux 13:31Z: "safe drop distance for tnt").
pub const MAX_DROP: i32 = 40;
/// Kicks per ground in the safe-drop screen (the full all-kicks sweep follows at the depth it finds).
pub const DROP_SCREEN_KICKS: usize = 16;

/// The safe drop's words on a card: `safe on the ground` or `needs an N-block drop`.
pub fn drop_words(h: i32) -> String {
    if h == 0 { "safe on the ground".into() } else { format!("needs a {h}-block drop") }
}

/// A search screen (single-threaded): the duper passes at `c.drop` on both grounds at `kicks`
/// evenly spaced kicks.
pub fn passes_on_ground(c: &Circuit, kicks: usize) -> bool {
    let margin = 3;
    let o = crate::verify::Options { margin, ..Default::default() };
    let Ok((s, _)) = crate::verify::schedule(c, &o) else { return false };
    crate::Ground::ALL.iter().all(|&g| (0..kicks.max(1)).all(|k| judge_kick(c, &s, k as f64 / kicks.max(1) as f64, margin, g).is_ok_and(|x| x.pass)))
}

/// The metric the judge adds: (`safe drop`, `0 blocks (on the ground)` / `1 block` / `N blocks`).
pub fn drop_metric(h: i32) -> (String, String) {
    ("safe drop".into(), match h {
        0 => "0 blocks (on the ground)".into(),
        1 => "1 block".into(),
        _ => format!("{h} blocks"),
    })
}

/// Every (ground, kick) of the screen passes at `c.drop`.
fn screen(c: &Circuit, s: &crate::stimulus::Schedule, margin: i32, kicks: usize, workers: usize) -> bool {
    let jobs: Vec<(crate::Ground, f64)> = crate::Ground::ALL.iter().flat_map(|&g| (0..kicks).map(move |k| (g, k as f64 / kicks as f64))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let bad = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|sc| {
        for _ in 0..workers.max(1) {
            let (jobs, next, bad) = (&jobs, &next, &bad);
            sc.spawn(move || {
                while !bad.load(std::sync::atomic::Ordering::Relaxed) {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= jobs.len() {
                        break;
                    }
                    let (g, kick) = jobs[i];
                    if !judge_kick(c, s, kick, margin, g).is_ok_and(|o| o.pass) {
                        bad.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            });
        }
    });
    !bad.load(std::sync::atomic::Ordering::Relaxed)
}

/// The safe drop height from the screen alone (`circuit sim`): the smallest drop at which every
/// screened kick passes on both grounds.
pub fn safe_drop_screen(c: &Circuit, workers: usize) -> Result<i32, String> {
    let margin = 3;
    let o = crate::verify::Options { margin, ..Default::default() };
    let (s, _) = crate::verify::schedule(c, &o).map_err(|e| format!("{e:#}"))?;
    for d in 0..=MAX_DROP {
        let mut cd = c.clone();
        cd.drop = d;
        if screen(&cd, &s, margin, DROP_SCREEN_KICKS, workers) {
            return Ok(d);
        }
    }
    Err(format!("blows itself up at every drop up to {MAX_DROP} blocks (it needs a pit deeper than {MAX_DROP})"))
}

/// The SAFE DROP HEIGHT (vjeux 13:31Z): the smallest number of air blocks between the duper's
/// lowest block and the ground under it at which it stays intact on both grounds (grass with
/// craters, obsidian without) for EVERY kick (the screen, then the full all-kicks sweep), with
/// the sweep's report. 0 = safe on the ground.
pub fn safe_drop(c: &Circuit, workers: usize) -> Result<(i32, String), String> {
    let margin = 3;
    let o = crate::verify::Options { margin, ..Default::default() };
    let (s, _) = crate::verify::schedule(c, &o).map_err(|e| format!("{e:#}"))?;
    let mut last = String::new();
    for d in 0..=MAX_DROP {
        let mut cd = c.clone();
        cd.drop = d;
        if !screen(&cd, &s, margin, DROP_SCREEN_KICKS, workers) {
            continue;
        }
        match tnt_robust_at(&cd, &s, margin, SAMPLES, workers) {
            Ok(r) => return Ok((d, r)),
            Err(e) => last = format!(" (at {d} blocks: {e})"),
        }
    }
    Err(format!("blows itself up at every drop up to {MAX_DROP} blocks (it needs a pit deeper than {MAX_DROP}){last}"))
}

/// [`tnt_robust`] at `c.drop` on a given schedule.
fn tnt_robust_at(c: &Circuit, s: &crate::stimulus::Schedule, margin: i32, samples: usize, workers: usize) -> Result<String, String> {
    let mut parts = Vec::new();
    let mut metrics: Option<String> = None;
    for g in crate::Ground::ALL {
        let (pieces, bounds, passing) = sweep(c, s, margin, g, samples, workers).map_err(|e| format!("on {}: {e}", g.name()))?;
        // the metrics (not the flags) agree across grounds
        let m = passing.split(" precise").next().unwrap_or(&passing).to_string();
        if let Some(q) = &metrics
            && *q != m
        {
            return Err(format!("measures `{m}` on {} but `{q}` on grass ground", g.name()));
        }
        metrics = Some(m);
        parts.push(format!("{}: {pieces} piece(s), {bounds} boundary kick(s)", g.name()));
    }
    Ok(format!("robust over every kick on both grounds ({}), all `{}`", parts.join("; "), metrics.unwrap_or_default()))
}

/// One ground's sweep: (pieces, boundary kicks, the passing outcome).
fn sweep(c: &Circuit, s: &crate::stimulus::Schedule, margin: i32, g: crate::Ground, samples: usize, workers: usize) -> Result<(usize, usize, String), String> {
    let memo: Mutex<HashMap<u64, (u64, String)>> = Mutex::new(HashMap::new());
    // the sweep in parallel, then the bisection (sequential) through the memo
    let xs: Vec<f64> = (0..samples).map(|i| i as f64 / samples as f64).chain(std::iter::once(redsim::robust::MAX_DOUBLE)).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        for _ in 0..workers.max(1) {
            let (xs, memo, next) = (&xs, &memo, &next);
            sc.spawn(move || {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= xs.len() {
                        break;
                    }
                    let r = code(&judge_kick(c, s, xs[i], margin, g));
                    memo.lock().unwrap().insert(xs[i].to_bits(), r);
                }
            });
        }
    });
    let eval = |d: f64| -> u64 {
        if let Some(r) = memo.lock().unwrap().get(&d.to_bits()) {
            return r.0;
        }
        let r = code(&judge_kick(c, s, d, margin, g));
        let h = r.0;
        memo.lock().unwrap().insert(d.to_bits(), r);
        h
    };
    let (pieces, bounds) = redsim::robust::critical_points(eval, samples, 1e-12);
    let memo = memo.into_inner().unwrap();
    let text_of = |h: u64| memo.values().find(|(k, _)| *k == h).map(|(_, t)| t.clone()).unwrap_or_default();
    // the metrics must agree across kicks; PRECISE is claimed only when every kick is precise (a
    // kick of exactly 0 makes every TNT identical: precise by accident)
    let mut passing: Option<String> = None;
    let mut precise_all = true;
    for p in &pieces {
        let t = text_of(p.outcome);
        if !t.starts_with("pass") {
            return Err(format!("fails for every kick in [{:.12}, {:.12}): {t}", p.lo, p.hi));
        }
        precise_all &= t.ends_with(" precise");
        let m = t.trim_end_matches(" precise").to_string();
        match &passing {
            None => passing = Some(m),
            Some(q) if *q != m => return Err(format!("kicks in [{:.12}, {:.12}) measure `{m}`, others `{q}`", p.lo, p.hi)),
            _ => {}
        }
    }
    for (d, h) in &bounds {
        let t = text_of(*h);
        if !t.starts_with("pass") {
            return Err(format!("fails at the boundary kick {d:.15}: {t}"));
        }
        precise_all &= t.ends_with(" precise");
    }
    let passing = passing.unwrap_or_default();
    Ok((pieces.len(), bounds.len(), if precise_all { format!("{passing} precise") } else { passing }))
}

/// The hard evidence for a TNT duper's survival (coordinator, after vjeux 13:13Z): every explosion
/// on redsim with NO out-of-range removal (each TNT flies on until it explodes), at `kicks` evenly
/// spaced kick angles, every ray at its largest roll. With `floor`, the machine stands on an obsidian
/// floor one block under it, `FLOOR_REACH` blocks wide around it (the TNT lands on it).
#[derive(Clone, Debug, Default)]
pub struct BlastReport {
    pub runs: usize,
    pub blasts: usize,
    /// The explosion centers' distances to the machine's box (0 = inside it), smallest and largest.
    pub min_dist: f64,
    pub max_dist: f64,
    /// Blocks of the machine destroyed (any run), and floor blocks destroyed.
    pub machine_destroyed: usize,
    pub floor_destroyed: usize,
    /// The first run a blast broke the machine in: the kick and why.
    pub broken: Option<(f64, String)>,
}

/// How far the evidence floor reaches around the machine.
pub const FLOOR_REACH: i32 = 24;

/// See [`BlastReport`].
pub fn tnt_blasts(c: &Circuit, kicks: usize, ground: Option<crate::Ground>) -> Result<BlastReport, String> {
    tnt_blasts_for(c, kicks, ground, None)
}

/// [`tnt_blasts`] over `ticks` (default: the judge's run).
pub fn tnt_blasts_for(c: &Circuit, kicks: usize, ground: Option<crate::Ground>, ticks: Option<u32>) -> Result<BlastReport, String> {
    let (lo, hi) = c.clock.design.bounds().ok_or("an empty design")?;
    let o = crate::verify::Options { margin: 3, ..Default::default() };
    let (mut s, _) = crate::verify::schedule(c, &o).map_err(|e| format!("{e:#}"))?;
    if let Some(t) = ticks {
        s.ticks = t;
    }
    let actions = c.actions(&s);
    let mut rep = BlastReport { min_dist: f64::INFINITY, ..Default::default() };
    let dist = |p: [f64; 3]| -> f64 { (0..3).map(|i| (lo[i] as f64 - p[i]).max(p[i] - (hi[i] as f64 + 1.0)).max(0.0)).map(|d| d * d).sum::<f64>().sqrt() };
    let cc = c.with_consumers().map_err(|e| format!("{e:#}"))?;
    for k in 0..kicks.max(1) {
        let kick = k as f64 / kicks.max(1) as f64;
        // the ground grows until no burning TNT leaves it, as in the judge's strict run
        let mut r = FLOOR_REACH;
        let d = loop {
            let floor = ground.map(|g| c.ground_blocks(g, r, c.drop)).unwrap_or_default();
            let from = [lo[0] - r - 2, lo[1] - 6 - c.drop, lo[2] - r - 2];
            let to = [hi[0] + r + 2, hi[1] + 16, hi[2] + r + 2];
            let to_ = clock::sim::TntOptions { kick, kicks: Vec::new(), max_rolls: true, out_of_range: f64::INFINITY, design_box: (lo, hi), machine_box: Some((lo, hi)), floor, watch: Some((lo, hi)), escape: ground.map(|_| r as f64 - 1.0) };
            let d = clock::sim::drive_tnt(&cc.clock, &actions, s.ticks, &[], from, to, &to_).map_err(|(_, why)| why)?;
            if d.escaped.is_some() && r < crate::run::MAX_GROUND_REACH {
                r *= 2;
                continue;
            }
            break d;
        };
        rep.runs += 1;
        for &(frame, center, mine, other) in &d.blasts {
            if std::env::var_os("MCTAS_BLASTS_LIST").is_some() {
                eprintln!("  kick {kick:.3} frame {frame}: center ({:.2}, {:.2}, {:.2}), {:.1} blocks away, destroyed {mine} machine + {other} ground", center[0], center[1], center[2], dist(center));
            }
            rep.blasts += 1;
            let dd = dist(center);
            rep.min_dist = rep.min_dist.min(dd);
            rep.max_dist = rep.max_dist.max(dd);
            rep.machine_destroyed += mine;
            rep.floor_destroyed += other;
        }
        if rep.broken.is_none()
            && let Some((_, why)) = &d.driven.stopped
        {
            rep.broken = Some((kick, why.clone()));
        }
    }
    Ok(rep)
}
