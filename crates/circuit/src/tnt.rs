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
use crate::{Circuit, Kind, Pos};

/// Ticks a self-running duper runs after its kick: 8 periods of up to 110 gt fit in the second
/// half (a slow duper whose period exceeds a fuse keeps one TNT alive at a time: the PRECISE one).
pub const RUN: u32 = 1800;
/// The smallest gap between a triggered duper's pulses with explosions on: a TNT's fuse (80 gt)
/// and some (the previous pulse's TNT has exploded before the next pulse).
pub const TRIGGER_GAP: u32 = 100;
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
            // explosions on: wait for the last pulse's TNT to go off (a fuse is 80 gt)
            if c.explodes() {
                s.phase(t, t + EXPLODE_TAIL, "explosions");
                t += EXPLODE_TAIL;
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

/// The survival flag: how many TNT exploded, how many left out of range (`run::OUT_OF_RANGE`).
fn survive_flag(t: &Trace) -> String {
    let oor = if t.out_of_range > 0 { format!(", {} out of range", t.out_of_range) } else { String::new() };
    format!("survives its own explosions: {} exploded{oor}, design intact", t.exploded)
}

/// The usefulness rule on top of a TNT duper's verdict (vjeux 2026-10-20: "a tnt duper that
/// explodes in water is useless"): with explosions on, every TNT must go off where it destroys
/// blocks: never in a fluid's cell, and with something to destroy around it on open ground
/// (`redsim::explosion::open_ground`: ground under the blast, the fluids where they are, the
/// weakest rolls). A passing duper fails on its first useless blast; a passing one gets the
/// blasts' summary (where they go off, the fewest blocks one would destroy on open ground), and
/// the table lists where every blast went off.
pub fn useful(c: &Circuit, t: &Trace, mut out: Outcome) -> Outcome {
    if !c.explodes() || t.blasts.is_empty() {
        return out;
    }
    let shown = 12;
    for b in t.blasts.iter().take(shown) {
        out.table.push(format!(
            "blast at frame {} ({:.2}, {:.2}, {:.2}): {}",
            b.frame,
            b.pos[0],
            b.pos[1],
            b.pos[2],
            if b.in_fluid { "IN A FLUID, destroys nothing".to_string() } else { format!("would destroy {} blocks on open ground", b.open_ground) }
        ));
    }
    if t.blasts.len() > shown {
        out.table.push(format!("... {} blasts in all", t.blasts.len()));
    }
    if !out.pass {
        return out;
    }
    if let Some(why) = t.blasts.iter().find_map(|b| b.useless()) {
        let n = t.blasts.iter().filter(|b| b.useless().is_some()).count();
        return Outcome { pass: false, why: format!("useless TNT: {why} ({n} of {} blasts)", t.blasts.len()), metrics: Vec::new(), table: out.table };
    }
    // (the flag carries no count: the fewest blocks a blast would destroy follows the random
    // kicks, and the builds of a robust design must print the same line; the table has the counts)
    let least = t.blasts.iter().map(|b| b.open_ground).min().unwrap_or(0);
    out.table.push(format!("useful: every blast clear of fluids, the weakest destroys {least} blocks on open ground"));
    let flag = "useful: every blast clear of fluids, each destroys blocks on open ground".to_string();
    out.why = if out.why.is_empty() { flag } else { format!("{}; {flag}", out.why) };
    out
}

/// No TNT went off nor left out of range during the run: the explosions were not exercised.
fn no_explosion(t: &Trace) -> bool {
    t.exploded + t.out_of_range == 0
}
/// With explosions on, the triggered stimulus waits this long after its last window (a fuse is
/// 80 gt).
pub const EXPLODE_TAIL: u32 = 100;
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
    let out = switched(s, t);
    // explosions on: the machine's block frames over one period at the end of every running
    // window equal those of the first running window's first period (before any explosion), and
    // the stopped windows end the same; so nothing its explosions did changed it
    if out.pass && c.explodes() {
        if no_explosion(t) {
            return Outcome { pass: false, why: "no primed TNT exploded during the run (explosions are judged in the real game; redsim does not model them)".into(), metrics: Vec::new(), table: out.table };
        }
        let m = |k: &str| out.metrics.iter().find(|(a, _)| a == k).map(|(_, b)| b.clone()).unwrap_or_default();
        let runs_on = m("runs").contains("input on");
        let p: usize = m("period").split_whitespace().next().and_then(|v| v.parse().ok()).unwrap_or(1);
        let n = t.cells.len();
        let frames = |a: usize, b: usize| -> std::collections::BTreeSet<&Vec<(Pos, u32)>> { (a.min(n)..b.min(n)).map(|f| &t.cells[f]).collect() };
        let mut first: Option<std::collections::BTreeSet<&Vec<(Pos, u32)>>> = None;
        let mut rest_end: Option<usize> = None;
        for (a, b, l) in &s.phases {
            let (a, b) = (*a as usize, (*b as usize).min(n));
            if l.starts_with("on") == runs_on {
                let early = frames(a + STOP as usize, a + STOP as usize + p);
                let late = frames(b - p, b);
                let base = first.get_or_insert(early);
                if late != *base {
                    return Outcome { pass: false, why: format!("blown up: at the end of {l} (frames {}..{b}) the running machine differs from before its first explosion", b - p), metrics: Vec::new(), table: out.table };
                }
            } else {
                match rest_end {
                    None => rest_end = Some(b - 1),
                    Some(e) if t.cells[e] != t.cells[b - 1] => {
                        return Outcome { pass: false, why: format!("blown up: the stopped machine at the end of {l} (frame {}) differs from the first stop (frame {e})", b - 1), metrics: Vec::new(), table: out.table };
                    }
                    _ => {}
                }
            }
        }
        let mut out = out;
        out.why = survive_flag(t);
        return out;
    }
    out
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

/// A run's PRECISE point (every TNT followed to fuse 1): every TNT of the periodic region that
/// reached fuse 1 stood at one point then, bit for bit (8 or more): `Some(Ok(point))`;
/// `Some(Err((i, p, main)))` when the `i`-th primed TNT (in `t.primed` order) ended at `p`, not at
/// the others' `main`; `None` when the run is not periodic or too few TNT reached fuse 1.
pub fn precise_point(c: &Circuit, t: &Trace) -> Option<std::result::Result<[f64; 3], (usize, [f64; 3], [f64; 3])>> {
    let (start, _) = period(t, c.clock.free_from() as usize)?;
    let ended: Vec<(usize, [f64; 3])> = t.primed.iter().enumerate().filter(|(_, q)| q.frame as usize >= start).filter_map(|(i, q)| q.end.map(|e| (i, e.0))).collect();
    if ended.len() < 8 {
        return None;
    }
    let same = |a: &[f64; 3], b: &[f64; 3]| a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
    // the main point: the one most TNT share
    let main = ended.iter().max_by_key(|(_, p)| ended.iter().filter(|(_, q)| same(p, q)).count()).map(|(_, p)| *p)?;
    match ended.iter().find(|(_, p)| !same(p, &main)) {
        Some((i, p)) => Some(Err((*i, *p, main))),
        None => Some(Ok(main)),
    }
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
    if c.explodes() && no_explosion(t) {
        return Outcome { pass: false, why: "explode: on, but no primed TNT exploded during the run (the real game judges it; redsim does not model explosions)".into(), metrics: Vec::new(), table };
    }
    let Some((start, p)) = period(t, from) else {
        return Outcome { pass: false, why: format!("not periodic within {} gt", s.ticks), metrics: Vec::new(), table };
    };
    // explosions on (the default): the block frames repeat with the period from before the first
    // explosion through the end, so nothing the explosions did changed the machine (a block they
    // destroyed would be missing from the frames seen before them)
    if c.explodes()
        && let Some(&x) = t.explosions.iter().min()
        && start >= x as usize
    {
        return Outcome {
            pass: false,
            why: format!("blown up: its first explosion (frame {x}) changes it (periodic only from frame {start}, period {p} gt)"),
            metrics: Vec::new(),
            table,
        };
    }
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
    Outcome { pass: true, why: flags(c, t, start), metrics, table }
}

/// The variation flags of a passing self-running duper: PRECISE and SURVIVABLE.
fn flags(c: &Circuit, t: &Trace, start: usize) -> String {
    let mut v: Vec<String> = Vec::new();
    if c.explodes() {
        // explosions on (the default): the survival, then the other flags as measured with the
        // explosions (their knockback moves the TNT still burning)
        v.push(survive_flag(t));
    }
    let p = precise_flag(t, start, c.clock.design.bounds().map(|(lo, _)| lo[1]), c.drop);
    if !p.is_empty() {
        v.push(p);
    }
    // SLIME-FREE: no slime block in the design (a honey frame, or none)
    if !c.clock.design.blocks.values().any(|s| s.starts_with("minecraft:slime_block")) {
        let honey = c.clock.design.blocks.values().any(|s| s.starts_with("minecraft:honey_block"));
        v.push(if honey { "slime-free: a honey frame".to_string() } else { "slime-free".to_string() });
    }
    // LAUNCHED: every TNT of the periodic region reaches 0.4+ blocks per gt sideways; its launch
    // motion (the largest), averaged, with the spread (the random kick shows on the cross axis)
    let ps: Vec<&crate::trace::Primed> = t.primed.iter().filter(|q| q.frame as usize >= start).collect();
    if ps.len() >= 8 && ps.iter().all(|q| q.launch.is_some()) {
        let ls: Vec<[f64; 3]> = ps.iter().map(|q| q.launch.unwrap()).collect();
        let mean = |i: usize| ls.iter().map(|l| l[i]).sum::<f64>() / ls.len() as f64;
        let spread = |i: usize| ls.iter().map(|l| l[i]).fold(f64::MIN, f64::max) - ls.iter().map(|l| l[i]).fold(f64::MAX, f64::min);
        v.push(format!("launched at ({:.4}, {:.4}, {:.4}) blocks/gt, spread ({:.4}, {:.4}, {:.4})", mean(0), mean(1), mean(2), spread(0), spread(1), spread(2)));
    }
    // STATIONARY at fuse 1: every TNT of the periodic region that reached fuse 1 had been at rest
    // (the same position, bit for bit) for 10+ gt
    let ended: Vec<&&crate::trace::Primed> = ps.iter().filter(|q| q.end.is_some()).collect();
    if ended.len() >= 8 {
        let rest = ended.iter().map(|q| q.end_frame.saturating_sub(q.moved)).min().unwrap_or(0);
        if rest >= 10 {
            v.push(format!("stationary: every TNT at rest for {rest}+ gt before fuse 1"));
        }
        // DROPPED straight down: every TNT 3+ blocks below its first sighting at fuse 1, falling
        // vertically then (horizontal motion under 0.01 blocks per gt)
        let fall = ended.iter().map(|q| q.pos[1] - q.end.unwrap().0[1]).fold(f64::MAX, f64::min);
        let vertical = ended.iter().all(|q| {
            let m = q.end.unwrap().1;
            m[0].abs() < 0.01 && m[2].abs() < 0.01 && m[1] < 0.0
        });
        if vertical && fall >= 3.0 {
            v.push(format!("dropped: every TNT falls straight down, {fall:.1}+ blocks by fuse 1"));
        }
    }
    // DROPPED out of range (explosions on): every TNT of the periodic region removed out of range
    // (the last ones still fall at the end of the run) left 3+ blocks below its first sighting,
    // falling straight down
    let gone: Vec<&&crate::trace::Primed> = ps.iter().filter(|q| q.gone.is_some()).collect();
    let burning_at_end = ps.iter().filter(|q| q.gone.is_none() && q.end.is_none()).count();
    if gone.len() >= 8 && gone.len() + burning_at_end == ps.len() {
        let straight = gone.iter().all(|q| {
            let (p, m) = q.gone.unwrap();
            m[0].abs() < 0.01 && m[2].abs() < 0.01 && m[1] < 0.0 && q.pos[1] - p[1] >= 3.0
        });
        if straight {
            v.push("dropped: every TNT falls straight down out of range".to_string());
        }
    }
    // DROPPED on the ground (the judge's ground, a drop under the machine): every TNT goes off
    // below the machine, within 8 blocks of its box sideways (the pile at the bottom scatters a
    // few blocks; a launched TNT lands 12+ away) and 3+ blocks under it
    if ended.len() >= 8
        && let Some((lo, hi)) = c.clock.design.bounds()
    {
        let below = ended.iter().all(|q| {
            let p = q.end.unwrap().0;
            let side = |i: usize| (lo[i] as f64 - p[i]).max(p[i] - (hi[i] as f64 + 1.0)).max(0.0);
            side(0).max(side(2)) <= 8.0 && p[1] <= lo[1] as f64 - 3.0
        });
        if below && !v.iter().any(|f| f.starts_with("dropped:")) {
            v.push("dropped: every TNT goes off below the machine".to_string());
        }
    }
    v.join("; ")
}

/// The PRECISE variation (the real game only: each TNT followed to fuse 1): every TNT of the
/// periodic region that reached fuse 1 is at the same point then, bit for bit (8 or more); `in
/// mid-air` (the PRECISE AIR BURST, vjeux 16:23Z "explode at the exact same spot in the air") when
/// that point has air under it and no water around it (it explodes flying, not resting); `N
/// blocks below the machine` (the PRECISE DROP, vjeux 2026-10-21 01:00Z, tree farms) when that
/// mid-air point is under the design's lowest layer `lowest` (N: from the machine's bottom down to
/// the TNT's feet; `drop`: the air blocks between the machine and the judge's ground, so a burst
/// below the machine also reads `H blocks above the ground`).
fn precise_flag(t: &Trace, start: usize, lowest: Option<i32>, drop: i32) -> String {
    let ended: Vec<&crate::trace::Primed> = t.primed.iter().filter(|q| q.frame as usize >= start).filter(|q| q.end.is_some()).collect();
    let ends: Vec<[f64; 3]> = ended.iter().map(|q| q.end.unwrap().0).collect();
    if ends.len() >= 8 && ends.iter().all(|e| e.iter().zip(&ends[0]).all(|(a, b)| a.to_bits() == b.to_bits())) {
        // the cells around its feet at its last frame: air under it, no fluid where it is
        let cell = |f: usize, p: [i32; 3]| -> Option<&str> {
            let frame = t.cells.get(f)?;
            frame.binary_search_by_key(&p, |(q, _)| *q).ok().map(|i| t.names[frame[i].1 as usize].as_str())
        };
        let airborne = ended.iter().all(|q| {
            let p = q.end.unwrap().0;
            let (x, z) = (p[0].floor() as i32, p[2].floor() as i32);
            let feet = (p[1] - 0.01).floor() as i32;
            let f = (q.end_frame as usize).min(t.cells.len().saturating_sub(1));
            let below = cell(f, [x, feet, z]);
            let here = cell(f, [x, feet + 1, z]);
            below.is_none() && here.is_none_or(|s| !s.contains("water") && !s.contains("bubble"))
        });
        if airborne {
            match lowest {
                Some(lo) if ends[0][1] < lo as f64 => {
                    let below = lo as f64 - ends[0][1];
                    let above = if drop > 0 { format!(", {:.1} blocks above the ground", drop as f64 - below) } else { String::new() };
                    format!("precise: {} TNT at one point at fuse 1, in mid-air {:.1} blocks below the machine{above}", ends.len(), below)
                }
                _ => format!("precise: {} TNT at one point at fuse 1, in mid-air", ends.len()),
            }
        } else {
            format!("precise: {} TNT at one point at fuse 1", ends.len())
        }
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
        table.push(format!("pulse {} at frame {lo}: {} primed{}, TNT blocks at the end {end}", k + 1, qs.len(), if qs.is_empty() { String::new() } else { format!(" (at +{} gt)", qs.iter().map(|f| (f - lo).to_string()).collect::<Vec<_>>().join(", +")) }));
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
    // explosions on: every TNT has gone off by the end (the stimulus waits for them) and the
    // machine is back to its rest state
    if c.explodes() {
        if no_explosion(t) {
            return Outcome { pass: false, why: "no primed TNT exploded during the run (explosions are judged in the real game; redsim does not model them)".into(), metrics: Vec::new(), table };
        }
        let last = t.cells.len() - 1;
        if t.cells[last] != t.cells[starts[0] - 1] {
            return Outcome { pass: false, why: format!("blown up: after its explosions the machine differs from its rest state (frame {last} vs {})", starts[0] - 1), metrics: Vec::new(), table };
        }
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
    let why = if c.explodes() { survive_flag(t) } else { String::new() };
    Outcome { pass: true, why, metrics, table }
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
