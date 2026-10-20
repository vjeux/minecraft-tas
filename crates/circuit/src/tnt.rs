//! TNT dupers (study 9; designs/circuits/tnt/VARIATIONS.md): the stimuli and checkers of
//! `tnt-duper` and `tnt-duper-triggered`. The runners record every primed TNT
//! ([`crate::trace::Primed`]) and the game's runner removes each one before it can explode.
//!
//! * `tnt-duper` (self-running): run [`RUN`] gt after the kick; the frames must repeat with a
//!   period P through the run's second half for at least [`CYCLES`] periods, every period must
//!   spawn the same n >= 1 primed TNT, and the design's TNT blocks must all stand again in every
//!   period (a moved TNT is in flight for 2 gt, so the most seen in the period counts). Metrics:
//!   rate (TNT per minute), period, TNT per period, latency (the kick to the first spawn).
//! * `tnt-duper-triggered`: [`PULSES`] input pulses of [`PULSE`] gt spaced by the sizing gap;
//!   nothing may spawn before the first one, each pulse's window must spawn the same k >= 1 primed
//!   TNT and end with every TNT block standing. Metrics: TNT per pulse, latency (the pulse's start
//!   to its first spawn), the gap.
//!
//! Precision (where the primed TNT is and how it moves) draws from the game's random kick, so it
//! is reported in the table, not in the metrics the builds are compared on.

use crate::check::Outcome;
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, Kind};

/// Ticks a self-running duper runs after its kick.
pub const RUN: u32 = 600;
/// Periods a self-running duper must repeat for.
pub const CYCLES: usize = 8;
/// Pulses and pulse length of the triggered stimulus.
pub const PULSES: usize = 6;
pub const PULSE: u32 = 4;

/// The kind's stimulus (`check::stimulus`).
pub fn stimulus(c: &Circuit, kind: &Kind, start: u32, gap: u32) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let mut t = start;
    match kind {
        Kind::TntDuperTriggered => {
            for k in 0..PULSES {
                s.pulse(0, t, PULSE);
                s.phase(t, t + PULSE + gap, format!("pulse {}", k + 1));
                t += PULSE + gap;
            }
        }
        _ => {
            t = c.clock.free_from() + RUN;
            s.phase(0, t, "run");
        }
    }
    s.ticks = t;
    s
}

/// TNT blocks standing in a frame.
fn tnt_in(t: &Trace, f: usize) -> usize {
    t.cells[f].iter().filter(|(_, id)| t.names[*id as usize].starts_with("minecraft:tnt")).count()
}

/// The smallest period of the frames from `from` on (every frame of the second half equal to the
/// one P later), and the first frame it holds from.
fn period(t: &Trace, from: usize) -> Option<(usize, usize)> {
    let n = t.cells.len();
    let half = (n / 2).max(from);
    for p in 1..=n.saturating_sub(half) / 2 {
        if (half..n - p).all(|i| t.cells[i] == t.cells[i + p]) {
            let mut start = half;
            while start > from && t.cells[start - 1] == t.cells[start - 1 + p] {
                start -= 1;
            }
            return Some((start, p));
        }
    }
    None
}

/// Spread of the primed TNT seen in frames `lo..hi` (max - min; horizontal = the larger of x and
/// z, so it reads the same at every rotation), for the table.
fn spread(t: &Trace, lo: usize, hi: usize) -> String {
    let ps: Vec<&crate::trace::Primed> = t.primed.iter().filter(|p| (p.frame as usize) >= lo && (p.frame as usize) < hi).collect();
    if ps.len() < 2 {
        return String::new();
    }
    let range = |f: &dyn Fn(&crate::trace::Primed) -> f64| -> f64 {
        let v: Vec<f64> = ps.iter().map(|p| f(p)).collect();
        v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
    };
    let mut s = format!("spread over {} spawns: position {:.4} across, {:.4} up", ps.len(), range(&|p| p.pos[0]).max(range(&|p| p.pos[2])), range(&|p| p.pos[1]));
    if ps.iter().all(|p| p.motion.is_some()) {
        let m = |i: usize| move |p: &crate::trace::Primed| p.motion.unwrap()[i];
        s += &format!("; motion {:.4} across, {:.4} up", range(&m(0)).max(range(&m(2))), range(&m(1)));
    }
    s
}

/// `tnt-duper`.
pub fn check_duper(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let from = c.clock.free_from() as usize;
    let n0 = tnt_in(t, 0);
    let mut table = Vec::new();
    if n0 == 0 {
        return Outcome { pass: false, why: "no TNT block in the design".into(), metrics: Vec::new(), table };
    }
    if t.primed.is_empty() {
        return Outcome { pass: false, why: "no primed TNT".into(), metrics: Vec::new(), table };
    }
    let Some((start, p)) = period(t, from) else {
        return Outcome { pass: false, why: format!("not periodic within {} gt", s.ticks), metrics: Vec::new(), table };
    };
    let cycles = (t.cells.len() - start) / p;
    if cycles < CYCLES {
        return Outcome { pass: false, why: format!("period {p} gt repeats only {cycles} times (at least {CYCLES})"), metrics: Vec::new(), table };
    }
    let mut per: Vec<usize> = Vec::new();
    for k in 0..cycles {
        let (lo, hi) = (start + k * p, start + (k + 1) * p);
        let n = t.primed.iter().filter(|q| (q.frame as usize) >= lo && (q.frame as usize) < hi).count();
        let most = (lo..hi).map(|f| tnt_in(t, f)).max().unwrap_or(0);
        table.push(format!("period {}: frames {lo}..{hi}: {n} primed, TNT blocks up to {most}", k + 1));
        per.push(n);
        if most < n0 {
            return Outcome { pass: false, why: format!("TNT consumed: {most} of {n0} TNT blocks stand in period {}", k + 1), metrics: Vec::new(), table };
        }
    }
    if per.iter().any(|&n| n != per[0]) || per[0] == 0 {
        return Outcome { pass: false, why: format!("primed TNT per period {per:?}: not the same every period"), metrics: Vec::new(), table };
    }
    let sp = spread(t, start, start + cycles * p);
    if !sp.is_empty() {
        table.push(sp);
    }
    let first = t.primed[0].frame as usize;
    let rate = per[0] as f64 * 1200.0 / p as f64;
    let metrics = vec![
        ("rate".to_string(), format!("{} TNT/min", fmt_rate(rate))),
        ("period".to_string(), format!("{p} gt")),
        ("per period".to_string(), per[0].to_string()),
        ("latency".to_string(), format!("{} gt", first.saturating_sub(from))),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `tnt-duper-triggered`.
pub fn check_triggered(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let _ = c;
    let n0 = tnt_in(t, 0);
    let mut table = Vec::new();
    if n0 == 0 {
        return Outcome { pass: false, why: "no TNT block in the design".into(), metrics: Vec::new(), table };
    }
    let starts: Vec<usize> = (1..t.cells.len()).filter(|&f| t.inputs[0][f] && !t.inputs[0][f - 1]).collect();
    if starts.is_empty() {
        return Outcome { pass: false, why: "the input never turned on".into(), metrics: Vec::new(), table };
    }
    if let Some(q) = t.primed.iter().find(|q| (q.frame as usize) < starts[0]) {
        return Outcome { pass: false, why: format!("primed TNT at frame {} before the first pulse (self-running: tnt-duper)", q.frame), metrics: Vec::new(), table };
    }
    let mut per: Vec<usize> = Vec::new();
    let mut lat: Vec<usize> = Vec::new();
    for (k, &lo) in starts.iter().enumerate() {
        let hi = starts.get(k + 1).copied().unwrap_or(t.cells.len());
        let qs: Vec<usize> = t.primed.iter().filter(|q| (q.frame as usize) >= lo && (q.frame as usize) < hi).map(|q| q.frame as usize).collect();
        let end = tnt_in(t, hi - 1);
        table.push(format!("pulse {} at frame {lo}: {} primed{}, TNT blocks at the end {end}", k + 1, qs.len(), qs.first().map(|f| format!(" (first at +{} gt)", f - lo)).unwrap_or_default()));
        if end < n0 {
            return Outcome { pass: false, why: format!("TNT consumed: {end} of {n0} TNT blocks stand after pulse {}", k + 1), metrics: Vec::new(), table };
        }
        per.push(qs.len());
        if let Some(f) = qs.first() {
            lat.push(f - lo);
        }
    }
    if per.iter().any(|&n| n != per[0]) || per[0] == 0 {
        return Outcome { pass: false, why: format!("primed TNT per pulse {per:?}: not the same every pulse"), metrics: Vec::new(), table };
    }
    let sp = spread(t, starts[0], t.cells.len());
    if !sp.is_empty() {
        table.push(sp);
    }
    let gap = s.phases.first().map(|ph| ph.1 - ph.0 - PULSE).unwrap_or(0);
    let metrics = vec![
        ("per pulse".to_string(), per[0].to_string()),
        ("latency".to_string(), format!("{} gt", lat.iter().max().copied().unwrap_or(0))),
        ("gap".to_string(), format!("{gap} gt")),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

fn fmt_rate(r: f64) -> String {
    if (r - r.round()).abs() < 1e-9 { format!("{}", r.round() as i64) } else { format!("{r:.1}") }
}
