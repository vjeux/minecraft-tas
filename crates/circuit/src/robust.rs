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

/// The judge's verdict for one kick, on the schedule `s`.
pub fn judge_kick(c: &Circuit, s: &crate::stimulus::Schedule, kick: f64, margin: i32) -> Result<Outcome, String> {
    let kind = c.kind.clone().ok_or("no kind")?;
    let t = crate::run::tnt_kick_trace(c, s, margin, kick)?;
    Ok(check::check(c, &kind, s, &t))
}

/// The all-kicks report: every piece of [0, 1) with its outcome; `Err` with the first failing
/// piece (or two passing pieces that measure differently).
pub fn tnt_robust(c: &Circuit, samples: usize, workers: usize) -> Result<String, String> {
    if !matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered)) || !c.explodes() {
        return Err("the all-kicks rule is for TNT dupers with explosions on".into());
    }
    let margin = 3;
    let o = crate::verify::Options { margin, ..Default::default() };
    let (s, _) = crate::verify::schedule(c, &o).map_err(|e| format!("{e:#}"))?;
    let s = &s;
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
                    let r = code(&judge_kick(c, s, xs[i], margin));
                    memo.lock().unwrap().insert(xs[i].to_bits(), r);
                }
            });
        }
    });
    let eval = |d: f64| -> u64 {
        if let Some(r) = memo.lock().unwrap().get(&d.to_bits()) {
            return r.0;
        }
        let r = code(&judge_kick(c, s, d, margin));
        let h = r.0;
        memo.lock().unwrap().insert(d.to_bits(), r);
        h
    };
    let (pieces, bounds) = redsim::robust::critical_points(eval, samples, 1e-12);
    let memo = memo.into_inner().unwrap();
    let text_of = |h: u64| memo.values().find(|(k, _)| *k == h).map(|(_, t)| t.clone()).unwrap_or_default();
    let mut passing: Option<String> = None;
    for p in &pieces {
        let t = text_of(p.outcome);
        if !t.starts_with("pass") {
            return Err(format!("fails for every kick in [{:.12}, {:.12}): {t}", p.lo, p.hi));
        }
        match &passing {
            None => passing = Some(t),
            Some(q) if *q != t => return Err(format!("kicks in [{:.12}, {:.12}) measure `{t}`, others `{q}`", p.lo, p.hi)),
            _ => {}
        }
    }
    for (d, h) in &bounds {
        let t = text_of(*h);
        if !t.starts_with("pass") {
            return Err(format!("fails at the boundary kick {d:.15}: {t}"));
        }
    }
    Ok(format!("robust over every kick: {} piece(s), {} boundary kick(s) judged, all `{}`", pieces.len(), bounds.len(), passing.unwrap_or_default()))
}
