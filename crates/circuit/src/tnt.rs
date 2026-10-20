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
        _ if !c.inputs.is_empty() => {
            // a switchable duper (the lever on/off variation): off, on, off, on, each window SWITCH
            // gt long; it must dupe in one input state and stop in the other
            t = c.clock.free_from() + 20;
            for k in 0..4 {
                if k % 2 == 1 {
                    s.pulse(0, t, SWITCH);
                }
                s.phase(t, t + SWITCH, if k % 2 == 1 { format!("on {}", k / 2 + 1) } else { format!("off {}", k / 2 + 1) });
                t += SWITCH;
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

/// Each window of the switchable duper's stimulus (gt).
pub const SWITCH: u32 = 400;
/// A switchable duper must stop within this many gt after the switch; its run is judged from
/// then on (8+ periods in SWITCH - STOP gt: P <= 45 gt).
pub const STOP: u32 = 40;

/// `tnt-duper` with an input (lever on/off): dupes periodically while the input is in one state,
/// spawns nothing in the other from `STOP` gt after the switch. A run window is judged from `STOP`
/// gt after its switch (a start-up may differ): the smallest P such that the spawn count of every
/// tick equals the count P ticks later, with 8+ periods and n >= 1 TNT per period (several TNT per
/// period, same tick or offset, are fine), every TNT block standing again over the last period;
/// both run windows must give the same (P, n). Metrics: rate, period, per period, runs (input on /
/// off), stop and start (the switch to the last spawn of a stopped window / the first of a run).
pub fn check_switched(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let _ = c;
    switched(s, t)
}

fn switched(s: &Schedule, t: &Trace) -> Outcome {
    let n0 = tnt_in(t, 0);
    let mut table = Vec::new();
    let windows: Vec<(u32, u32, bool)> = s.phases.iter().map(|(a, b, l)| (*a, *b, l.starts_with("on"))).collect();
    if windows.len() != 4 || n0 == 0 {
        return Outcome { pass: false, why: "no TNT block, or not the switchable stimulus".into(), metrics: Vec::new(), table };
    }
    let frames = t.cells.len() as u32;
    let spawns_in = |a: u32, b: u32| t.primed.iter().filter(|q| q.frame >= a && q.frame < b).map(|q| q.frame).collect::<Vec<u32>>();
    let mut run_state: Option<bool> = None;
    let mut runs: Vec<(u32, usize)> = Vec::new();
    let (mut stop, mut start) = (0u32, 0u32);
    for &(a, b, on) in &windows {
        let b = b.min(frames);
        let lo = a + STOP;
        let sp = spawns_in(a, b);
        let late = spawns_in(lo, b);
        table.push(format!("{} window {}..{}: {} primed ({} from {lo})", if on { "on" } else { "off" }, a, b, sp.len(), late.len()));
        if late.is_empty() {
            if let Some(last) = sp.last() {
                stop = stop.max(last - a);
            }
            continue;
        }
        match run_state {
            None => run_state = Some(on),
            Some(r) if r != on => return Outcome { pass: false, why: format!("dupes with the input both on and off (window {}..{})", a, b), metrics: Vec::new(), table },
            _ => {}
        }
        if let Some(first) = sp.first() {
            start = start.max(first - a);
        }
        let span = (b - lo) as usize;
        let mut cnt = vec![0usize; span];
        for f in &late {
            cnt[(f - lo) as usize] += 1;
        }
        let found = (1..=span / 8).find(|&p| cnt[..p].iter().sum::<usize>() > 0 && (0..span - p).all(|i| cnt[i] == cnt[i + p]));
        let Some(p) = found else {
            return Outcome { pass: false, why: format!("window {a}..{b}: {} primed from {lo}, not periodic with 8+ periods (P <= {} gt)", late.len(), span / 8), metrics: Vec::new(), table };
        };
        let n: usize = cnt[..p].iter().sum();
        let most = (b as usize - p..b as usize).map(|f| tnt_in(t, f)).max().unwrap_or(0);
        if most < n0 {
            return Outcome { pass: false, why: format!("TNT consumed: {most} of {n0} TNT blocks stand in the last period of window {a}..{b}"), metrics: Vec::new(), table };
        }
        runs.push((p as u32, n));
    }
    let Some(on) = run_state else {
        return Outcome { pass: false, why: "never dupes".into(), metrics: Vec::new(), table };
    };
    if runs.len() != 2 || runs[0] != runs[1] {
        return Outcome { pass: false, why: format!("the two run windows differ (period, TNT per period): {runs:?}"), metrics: Vec::new(), table };
    }
    let (p, n) = runs[0];
    let metrics = vec![
        ("rate".to_string(), format!("{} TNT/min", fmt_rate(n as f64 * 1200.0 / p as f64))),
        ("period".to_string(), format!("{p} gt")),
        ("per period".to_string(), n.to_string()),
        ("runs".to_string(), if on { "input on".to_string() } else { "input off".to_string() }),
        ("stop".to_string(), format!("{stop} gt")),
        ("start".to_string(), format!("{start} gt")),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
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

/// The cells of the primed TNT seen in frames `lo..hi`.
fn cells(t: &Trace, lo: usize, hi: usize) -> Vec<[i32; 3]> {
    let mut v: Vec<[i32; 3]> = t.primed.iter().filter(|q| (q.frame as usize) >= lo && (q.frame as usize) < hi).map(|q| [q.pos[0].floor() as i32, q.pos[1].floor() as i32, q.pos[2].floor() as i32]).collect();
    v.sort();
    v
}

/// Every cell of `b` within one cell of a cell of `a` (the game sees a TNT one tick after it
/// spawned, sometimes already pushed a cell: a drifting duper walks further than that).
fn same_place(a: &[[i32; 3]], b: &[[i32; 3]]) -> bool {
    a.len() == b.len() && b.iter().all(|q| a.iter().any(|p| (0..3).all(|i| (p[i] - q[i]).abs() <= 1)))
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
    // the primes repeat in place too (clockinfra: a drifting duper is caught even when the block
    // frames happen to repeat)
    let first_cells = cells(t, start, start + p);
    for k in 1..cycles {
        let ck = cells(t, start + k * p, start + (k + 1) * p);
        if !same_place(&first_cells, &ck) {
            return Outcome { pass: false, why: format!("primed TNT drift: period {} spawns at {ck:?}, period 1 at {first_cells:?}", k + 1), metrics: Vec::new(), table };
        }
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
    Outcome { pass: true, why: precise_flag(t, start), metrics, table }
}

/// The PRECISE variation (the real game only: each TNT followed to fuse 1): every TNT of the
/// periodic region that reached fuse 1 is at the same point then, bit for bit (8 or more).
fn precise_flag(t: &Trace, start: usize) -> String {
    let ends: Vec<[f64; 3]> = t.primed.iter().filter(|q| q.frame as usize >= start).filter_map(|q| q.end.map(|e| e.0)).collect();
    if ends.len() >= 8 && ends.iter().all(|e| e.iter().zip(&ends[0]).all(|(a, b)| a.to_bits() == b.to_bits())) {
        format!("precise: {} TNT at one point at fuse 1", ends.len())
    } else {
        String::new()
    }
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
    let window = |k: usize| (starts[k], starts.get(k + 1).copied().unwrap_or(t.cells.len()));
    let (a0, b0) = window(0);
    let first_cells = cells(t, a0, b0);
    for k in 1..starts.len() {
        let (a, b) = window(k);
        let ck = cells(t, a, b);
        if !same_place(&first_cells, &ck) {
            return Outcome { pass: false, why: format!("primed TNT drift: pulse {} spawns at {ck:?}, pulse 1 at {first_cells:?}", k + 1), metrics: Vec::new(), table };
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A trace with one TNT block standing in every frame and primed TNT at `spawns`.
    fn trace(frames: usize, spawns: &[u32]) -> Trace {
        let mut t = Trace { names: vec!["minecraft:tnt[unstable=false]".into()], cells: vec![vec![([0, 0, 0], 0)]; frames], ..Default::default() };
        t.primed = spawns.iter().map(|&f| crate::trace::Primed { frame: f, ..Default::default() }).collect();
        t
    }

    fn sched() -> Schedule {
        let mut s = Schedule::new(1);
        let mut t = 20;
        for k in 0..4 {
            s.phase(t, t + SWITCH, if k % 2 == 1 { "on" } else { "off" });
            t += SWITCH;
        }
        s.ticks = t;
        s
    }

    /// Spawns in the off windows: `per` TNT every `p` gt (offset `gap` within a period), after a
    /// start-up of `first` gt with one odd gap.
    fn runs(p: u32, per: &[u32], first: u32) -> Vec<u32> {
        let mut v = Vec::new();
        for a in [20, 20 + 2 * SWITCH] {
            v.push(a + first);
            let mut t = a + first + p + 3;
            while t < a + SWITCH {
                for off in per {
                    if t + off < a + SWITCH {
                        v.push(t + off);
                    }
                }
                t += p;
            }
        }
        v.sort();
        v
    }

    #[test]
    fn switched_accepts_bursts_offsets_and_startups() {
        let n = (20 + 4 * SWITCH + 1) as usize;
        // one per period
        let o = switched(&sched(), &trace(n, &runs(6, &[0], 2)));
        assert!(o.pass, "{}", o.why);
        assert_eq!(o.metrics_text(), "rate 200 TNT/min, period 6 gt, per period 1, runs input off, stop 0 gt, start 2 gt");
        // two in the same tick, and two offset within the period, after an odd first gap
        for per in [&[0u32, 0][..], &[0, 2][..]] {
            let o = switched(&sched(), &trace(n, &runs(10, per, 5)));
            assert!(o.pass, "{per:?}: {}", o.why);
            assert!(o.metrics_text().starts_with("rate 240 TNT/min, period 10 gt, per period 2"), "{}", o.metrics_text());
        }
        // a period too long for 8 periods in the judged span
        let o = switched(&sched(), &trace(n, &runs(50, &[0], 5)));
        assert!(!o.pass);
        // dupes in both input states
        let mut both = runs(6, &[0], 2);
        both.extend((20 + SWITCH + 50..20 + 2 * SWITCH).step_by(6));
        both.sort();
        assert!(!switched(&sched(), &trace(n, &both)).pass);
    }
}
