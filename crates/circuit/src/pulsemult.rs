//! Study 11 (designs/circuits/pulsemult/VARIATIONS.md): pulse multipliers, frequency dividers,
//! monostables and timers. The stimuli and checkers of `burst N`, `divider N`, `monostable
//! [retriggerable | non-retriggerable]`, `off-delay`, `on-delay` and `pulse-train`. The input is a
//! judge-placed block or a lever (the judge sets every length); the output is read like every
//! circuit's (the judge's lamp in an air output cell).
//!
//! * `burst N`: input pulses of [`BURST_LENS`] gt, each followed by the gap: every input from the
//!   min-pulse up (the 3 longest at least) gives exactly N output pulses, the output off at the end
//!   of every gap. Then pairs of 4 gt inputs [`PAIR_DELAYS`] apart: a second input inside the burst
//!   is `ignored` (N pulses in all), `adds` (2N) or `restarts` (between); then trains of 6 inputs
//!   every [`TRAIN_PERIODS`] gt: `repeat` is the fastest period from which every slower train gives
//!   exactly 6N. Metrics: N, pulse (output lengths), period (rising-edge spacing), latency, span,
//!   min-pulse, retrigger, repeat.
//! * `divider N` (N >= 3): square clocks of [`CLOCK_PERIODS`] gt, N warm-up periods then 4N: the
//!   output repeats with period exactly N x P, one rise per N input periods. Metrics at the slowest
//!   clock: duty, square / pulse, latency; `min-period`: the fastest clock from which every slower
//!   one divides.
//! * `monostable`: isolated pulses as for `rising-edge` (one output pulse of L per input), then
//!   pairs of 4 gt inputs d apart: inside the pulse (d < L) a `non-retriggerable` one gives one pulse
//!   of exactly L, a `retriggerable` one a pulse ending L after the second edge; `re-arm`: the
//!   smallest d from which every pair gives two full pulses.
//! * `off-delay`: isolated pulses: the output follows the input at a fixed latency and stays on L
//!   more (L >= 1); 3-pulse trains whose gaps are shorter than L give one pulse ending L after the
//!   last fall.
//! * `on-delay`: isolated pulses: no output for inputs of T gt or less, one pulse of the input
//!   length - T (T >= 1) at a fixed latency from the input's fall for longer ones.
//! * `pulse-train`: the input held for [`WINDOWS`] gt: pulses every P gt while it is on (the first
//!   at the same latency every time), the last starting within P of the release, the output off at
//!   the end of every gap.

use crate::check::{Outcome, main_output};
use crate::stimulus::Schedule;
use crate::trace::{Trace, runs};
use crate::{Circuit, InputHow, Kind};

pub const BURST_LENS: [u32; 8] = [1, 2, 3, 4, 6, 10, 20, 40];
/// A monostable's isolated inputs: up to 160 gt, so a dual-edge one-shot whose falling-edge pulse
/// hides inside its own L for short inputs still shows it (clk-rt: study 1's pulse-dual-lock-L34).
pub const ISO_LENS: [u32; 14] = [1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40, 80, 160];
pub const FILTER_LENS: [u32; 16] = [1, 2, 3, 4, 5, 6, 8, 10, 12, 15, 20, 25, 30, 40, 60, 80];
pub const OFF_LENS: [u32; 14] = [1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40, 60, 80];
pub const PAIR_DELAYS: [u32; 15] = [5, 6, 8, 10, 12, 16, 20, 24, 32, 40, 48, 64, 80, 120, 160];
pub const TRAIN_PERIODS: [u32; 10] = [40, 30, 20, 16, 12, 10, 8, 6, 5, 4];
pub const TRAIN_GAPS: [u32; 6] = [1, 2, 4, 8, 16, 32];
pub const CLOCK_PERIODS: [u32; 12] = [40, 30, 20, 16, 12, 10, 8, 6, 5, 4, 3, 2];
pub const WINDOWS: [u32; 3] = [20, 60, 200];

fn gt(n: i64) -> String {
    format!("{n} gt")
}

fn range(v: &[i64]) -> String {
    let (lo, hi) = (v.iter().min().copied().unwrap_or(0), v.iter().max().copied().unwrap_or(0));
    if lo == hi { gt(lo) } else { format!("{lo}-{hi} gt") }
}

/// The kind's stimulus (`check::stimulus`), from `t` (the first free frame) with the sizing gap.
pub fn stimulus(c: &Circuit, kind: &Kind, start: u32, gap: u32) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let mut t = start;
    let pulse = |s: &mut Schedule, t: &mut u32, len: u32| {
        s.pulse(0, *t, len);
        s.phase(*t, *t + len + gap, format!("pulse {len}"));
        *t += len + gap;
    };
    let pairs = |s: &mut Schedule, t: &mut u32| {
        for d in PAIR_DELAYS {
            s.pulse(0, *t, 4);
            s.pulse(0, *t + d, 4);
            s.phase(*t, *t + d + 4 + gap, format!("pair {d}"));
            *t += d + 4 + gap;
        }
    };
    match kind {
        Kind::Burst(_) => {
            for l in BURST_LENS {
                pulse(&mut s, &mut t, l);
            }
            pairs(&mut s, &mut t);
            for r in TRAIN_PERIODS {
                let w = (r / 2).clamp(1, 4);
                for k in 0..6 {
                    s.pulse(0, t + k * r, w);
                }
                s.phase(t, t + 6 * r + gap, format!("train {r}"));
                t += 6 * r + gap;
            }
        }
        Kind::Monostable(_) => {
            for l in ISO_LENS {
                pulse(&mut s, &mut t, l);
            }
            pairs(&mut s, &mut t);
        }
        Kind::OffDelay => {
            for l in OFF_LENS {
                pulse(&mut s, &mut t, l);
            }
            for g in TRAIN_GAPS {
                for k in 0..3 {
                    s.pulse(0, t + k * (4 + g), 4);
                }
                s.phase(t, t + 3 * (4 + g) + gap, format!("train {g}"));
                t += 3 * (4 + g) + gap;
            }
        }
        Kind::OnDelay => {
            for l in FILTER_LENS {
                pulse(&mut s, &mut t, l);
            }
        }
        Kind::Divider(n) => {
            for p in CLOCK_PERIODS {
                let periods = 5 * n;
                for k in 0..periods {
                    s.pulse(0, t + k * p, p.div_ceil(2));
                }
                s.phase(t, t + periods * p + gap, format!("clock {p}"));
                t += periods * p + gap;
            }
        }
        Kind::PulseTrain => {
            for w in WINDOWS {
                pulse(&mut s, &mut t, w);
            }
        }
        _ => {}
    }
    s.ticks = t;
    s
}

/// The phases of one label prefix: (from, to, the number after the prefix).
fn phases<'a>(s: &'a Schedule, prefix: &'a str) -> impl Iterator<Item = (usize, usize, u32)> + 'a {
    s.phases.iter().filter_map(move |(a, b, l)| l.strip_prefix(prefix).and_then(|v| v.trim().parse::<u32>().ok()).map(|v| (*a as usize, *b as usize, v)))
}

/// Output runs (start, length) starting in `from..to`.
fn runs_in(r: &[(usize, usize)], from: usize, to: usize) -> Vec<(usize, usize)> {
    r.iter().filter(|(s, _)| *s >= from && *s < to).copied().collect()
}

/// The input's runs starting in `from..to`.
fn input_runs(t: &Trace, from: usize, to: usize) -> Vec<(usize, usize)> {
    runs_in(&runs(&t.inputs[0]), from, to)
}

/// The shortest input length from which every longer one is handled (`ok` in input-length order),
/// when the `need` longest at least are.
fn min_pulse(lens: &[u32], ok: &[bool], need: usize) -> Option<u32> {
    let n = ok.len();
    let k0 = (0..=n).rev().take_while(|&k| k == n || ok[k]).last().unwrap_or(n);
    (n - k0 >= need.min(n)).then(|| lens[k0.min(n - 1)])
}

/// The checks every study-11 kind shares: a block or lever input; the output off at rest (all but
/// dividers, which hold a state).
fn common(c: &Circuit, kind: &Kind, t: &Trace) -> Option<String> {
    if c.inputs.iter().any(|i| i.how == InputHow::Button) {
        return Some("a button input cannot give the judge's input lengths: use a block or lever input".into());
    }
    if !matches!(kind, Kind::Divider(_)) {
        let out = t.out_seq(main_output(c));
        let (a, b) = ((c.clock.free_from() + crate::stimulus::SETTLE / 2) as usize, (c.clock.free_from() + crate::stimulus::SETTLE) as usize);
        if out[a..b.min(out.len())].iter().any(|x| *x) {
            return Some("the output is on at rest (before the first input)".into());
        }
    }
    None
}

pub fn check(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace) -> Outcome {
    if let Some(why) = common(c, kind, t) {
        return Outcome { pass: false, why, metrics: Vec::new(), table: Vec::new() };
    }
    match kind {
        Kind::Burst(n) => burst(c, *n as usize, s, t),
        Kind::Monostable(claim) => monostable(c, claim.as_deref(), s, t),
        Kind::OffDelay => off_delay(c, s, t),
        Kind::OnDelay => on_delay(c, s, t),
        Kind::Divider(n) => divider(c, *n as usize, s, t),
        Kind::PulseTrain => pulse_train(c, s, t),
        _ => Outcome { pass: false, why: "not a study-11 kind".into(), metrics: Vec::new(), table: Vec::new() },
    }
}

/// A tiled run (`tile: dx dy dz [copies]`, [`crate::tiled`]): two passes, copy k's inputs getting
/// the kind's stimulus `shift` x k gt later: 7 (the copies' edges interleave) and 0 (all in phase:
/// clockinfra, a fixed shift lines edges up again whenever `shift` x k is a multiple of a stimulus
/// period, so crosstalk between interleaved edges could hide on those copies).
pub const TILE_SHIFTS: [u32; 2] = [7, 0];

/// `s` with every edge and phase `dt` gt later.
fn shifted(s: &Schedule, dt: u32) -> Schedule {
    let mut o = s.clone();
    for e in &mut o.edges {
        for (t, _) in e.iter_mut() {
            *t += dt;
        }
    }
    for (a, b, _) in &mut o.phases {
        *a += dt;
        *b += dt;
    }
    o.ticks += dt;
    o
}

/// A port name of the tiled circuit back to its copy: `<name>.t<k>` is copy k of `<name>`, a bare
/// `t<k>` copy k of the unnamed port (`default`: `in` for inputs, `out` for outputs), any other
/// name copy 0.
fn copy_of(name: &str, default: &str) -> (String, u32) {
    let k = crate::shulker::group_of(name);
    if k == 0 {
        return (name.to_string(), 0);
    }
    match name.rsplit_once('.') {
        Some((base, _)) => (base.to_string(), k),
        None => (default.to_string(), k),
    }
}

/// The schedule of `tc`, the tiled `c`: its input `<name>.t<k>` gets `c`'s input `<name>` edges
/// `shift` x k gt later (copy 0's phases kept; each copy is judged on its own, shifted).
pub fn tiled_schedule(c: &Circuit, tc: &Circuit, s: &Schedule, shift: u32) -> Schedule {
    let mut out = Schedule::new(tc.inputs.len());
    let mut last = 0;
    for (i, inp) in tc.inputs.iter().enumerate() {
        let (base, k) = copy_of(&inp.name, "in");
        if let Some(b) = c.inputs.iter().position(|x| x.name == base) {
            out.edges[i] = shifted(s, shift * k).edges[b].clone();
            last = last.max(k);
        }
    }
    out.phases = s.phases.clone();
    out.events = s.events.clone();
    out.ticks = s.ticks + shift * last;
    out
}

/// Judge a tiled run `t` of `tc` (schedule [`tiled_schedule`] with `shift`): each copy k, its own
/// inputs and outputs (`<name>.t<k>`), against `s` shifted `shift` x k gt; every copy must pass
/// and measure what the design measures alone (`alone`), else the copies disturb each other.
/// Passes with `alone`'s metrics plus `tileable` (`label`, the design's own step).
pub fn check_tiled(c: &Circuit, kind: &Kind, tc: &Circuit, s: &Schedule, t: &Trace, alone: &Outcome, shift: u32, label: &str) -> Outcome {
    let Some(tile) = c.tile else { return alone.clone() };
    let phase = if shift == 0 { "in phase".to_string() } else { format!("{shift} gt apart") };
    for k in 0..tile.copies {
        let inputs: Option<Vec<Vec<bool>>> = c.inputs.iter().map(|i| tc.inputs.iter().position(|x| copy_of(&x.name, "in") == (i.name.clone(), k)).and_then(|j| t.inputs.get(j).cloned())).collect();
        let levels: Option<Vec<Vec<u8>>> = c.outputs.iter().map(|o| tc.outputs.iter().position(|x| copy_of(&x.name, "out") == (o.name.clone(), k)).and_then(|j| t.levels.get(j).cloned())).collect();
        let (Some(inputs), Some(levels)) = (inputs, levels) else { return fail(format!("tile: copy {k}'s ports are not in the tiled run"), Vec::new()) };
        // copy k's cells seen from copy 0's place (a translation keeps them sorted): the checks
        // that read a cell by the design's position (the judge's lamp, `check::update_less`) see
        // copy k's own
        let off = [tile.step[0] * k as i32, tile.step[1] * k as i32, tile.step[2] * k as i32];
        let cells: Vec<Vec<(crate::Pos, u32)>> = t.cells.iter().map(|f| f.iter().map(|(p, s)| ([p[0] - off[0], p[1] - off[1], p[2] - off[2]], *s)).collect()).collect();
        let tk = Trace { inputs, levels, names: t.names.clone(), cells, ..Default::default() };
        let out = crate::check::check(c, kind, &shifted(s, shift * k), &tk);
        if !out.pass {
            return fail(format!("tile {} {} {} (copies driven {phase}): copy {k} of {}: {}", tile.step[0], tile.step[1], tile.step[2], tile.copies, out.why), out.table);
        }
        if out.metrics != alone.metrics {
            let text = |m: &[(String, String)]| m.iter().map(|(a, b)| format!("{a} {b}")).collect::<Vec<_>>().join(", ");
            return fail(format!("tile {} {} {} (copies driven {phase}): copy {k} of {} measures {} (alone: {})", tile.step[0], tile.step[1], tile.step[2], tile.copies, text(&out.metrics), text(&alone.metrics)), out.table);
        }
    }
    let mut o = alone.clone();
    // `label`: the design's own step (crate::bud::tile_label of the unturned design), the same in
    // every build, as BUD switches name it
    o.metrics.push(("tileable".into(), label.to_string()));
    o
}

fn fail(why: impl Into<String>, table: Vec<String>) -> Outcome {
    Outcome { pass: false, why: why.into(), metrics: Vec::new(), table }
}

/// `burst N`.
fn burst(c: &Circuit, n: usize, s: &Schedule, t: &Trace) -> Outcome {
    let out = t.out_seq(main_output(c));
    let orun = runs(&out);
    let mut table = Vec::new();
    // single pulses: per input, the output pulses relative to the input's rising edge
    let mut rows: Vec<(u32, Vec<(i64, usize)>, bool)> = Vec::new();
    for (from, to, l) in phases(s, "pulse ") {
        let to = to.min(out.len());
        let Some(&(on, _)) = input_runs(t, from, to).first() else { return fail(format!("pulse {l}: the input never turned on"), table) };
        let os: Vec<(i64, usize)> = runs_in(&orun, from, to).into_iter().map(|(st, len)| (st as i64 - on as i64, len)).collect();
        let back = !out[to - 1];
        table.push(format!("in {l:>2} gt: {} pulse(s){}", os.len(), if back { "" } else { " (still on at the end)" }));
        rows.push((l, os, back));
    }
    if let Some((l, _, _)) = rows.iter().find(|r| !r.2) {
        return fail(format!("latches: after a {l} gt input the output is still on at the end of its gap"), table);
    }
    let lens: Vec<u32> = rows.iter().map(|r| r.0).collect();
    // the burst must not depend on the input's length: every handled input gives the longest
    // one's N pulses at the same delays and lengths (a dual-edge detector, whose second pulse
    // follows the falling edge, is not a burst)
    let refr0 = rows.last().map(|r| r.1.clone()).unwrap_or_default();
    let ok: Vec<bool> = rows.iter().map(|r| r.1.len() == n && r.1 == refr0).collect();
    let Some(mp) = min_pulse(&lens, &ok, 3) else {
        let last = rows.last().map(|r| r.1.len()).unwrap_or(0);
        return fail(format!("the longest input gives {last} output pulse(s): want {n}, the same for the 3 longest inputs at least (a burst does not depend on the input's length)"), table);
    };
    let handled: Vec<&(u32, Vec<(i64, usize)>, bool)> = rows.iter().filter(|r| r.0 >= mp).collect();
    let refr = &handled.last().unwrap().1;
    let pulse_lens: Vec<i64> = handled.iter().flat_map(|r| r.1.iter().map(|p| p.1 as i64)).collect();
    let periods: Vec<i64> = handled.iter().flat_map(|r| r.1.windows(2).map(|w| w[1].0 - w[0].0)).collect();
    let latencies: Vec<i64> = handled.iter().map(|r| r.1[0].0).collect();
    let span = refr.last().map(|p| p.0 + p.1 as i64).unwrap_or(0);
    // the pairs and the trains are made of 4 gt inputs (clockinfra): with a min-pulse over 4 gt
    // they would measure the short inputs, not re-triggering, so both read n/a
    let short_ok = mp <= 4;
    // re-trigger: a second input inside the burst
    let mut classes: Vec<&str> = Vec::new();
    for (from, to, d) in phases(s, "pair ").filter(|_| short_ok) {
        let to = to.min(out.len());
        let cnt = runs_in(&orun, from, to).len();
        if out[to - 1] {
            return fail(format!("a second input {d} gt after the first leaves the output on"), table);
        }
        if (d as i64) < span {
            let class = if cnt == n {
                "ignores"
            } else if cnt == 2 * n {
                "adds"
            } else if cnt > n && cnt < 2 * n {
                "restarts"
            } else {
                "breaks"
            };
            table.push(format!("second input {d} gt after the first (inside the {span} gt burst): {cnt} pulses: {class}"));
            classes.push(class);
        }
    }
    classes.dedup();
    let na = format!("n/a (min-pulse {mp} gt > 4)");
    let retrigger = match classes.as_slice() {
        _ if !short_ok => na.clone(),
        [] => "n/a".to_string(),
        [one] => one.to_string(),
        _ => "mixed".to_string(),
    };
    // repeat rate: trains of 6 inputs
    let mut train_ok: Vec<(u32, bool)> = Vec::new();
    for (from, to, r) in phases(s, "train ").filter(|_| short_ok) {
        let to = to.min(out.len());
        let cnt = runs_in(&orun, from, to).len();
        let good = cnt == 6 * n && !out[to - 1];
        table.push(format!("6 inputs every {r} gt: {cnt} pulses{}", if good { "" } else { "  <- not 6N" }));
        train_ok.push((r, good));
    }
    let repeat = if short_ok { train_ok.iter().take_while(|(_, g)| *g).last().map(|(r, _)| gt(*r as i64)).unwrap_or_else(|| "> 40 gt".into()) } else { na };
    let _ = (&pulse_lens, &periods);
    let metrics = vec![
        ("N".to_string(), n.to_string()),
        ("pulse".to_string(), range(&pulse_lens)),
        ("period".to_string(), if n > 1 { range(&periods) } else { "-".into() }),
        ("latency".to_string(), range(&latencies)),
        ("span".to_string(), gt(span)),
        ("min-pulse".to_string(), gt(mp as i64)),
        ("retrigger".to_string(), retrigger),
        ("repeat".to_string(), repeat),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `monostable [retriggerable | non-retriggerable]`.
fn monostable(c: &Circuit, claim: Option<&str>, s: &Schedule, t: &Trace) -> Outcome {
    let out = t.out_seq(main_output(c));
    let orun = runs(&out);
    let mut table = Vec::new();
    let mut rows: Vec<(u32, Option<(i64, usize)>, bool)> = Vec::new();
    for (from, to, l) in phases(s, "pulse ") {
        let to = to.min(out.len());
        let Some(&(on, _)) = input_runs(t, from, to).first() else { return fail(format!("pulse {l}: the input never turned on"), table) };
        let os = runs_in(&orun, from, to);
        let one = (os.len() == 1 && os[0].0 >= on).then(|| (os[0].0 as i64 - on as i64, os[0].1));
        let back = !out[to - 1];
        table.push(format!("in {l:>2} gt: {}", os.iter().map(|(st, len)| format!("+{} for {len}", *st as i64 - on as i64)).collect::<Vec<_>>().join(", ")));
        rows.push((l, one, back));
    }
    if let Some((l, _, _)) = rows.iter().find(|r| !r.2) {
        return fail(format!("latches: after a {l} gt input the output is still on at the end of its gap"), table);
    }
    let Some(r) = rows.last().and_then(|r| r.1) else { return fail("the longest input does not give exactly one output pulse", table) };
    let lens: Vec<u32> = rows.iter().map(|r| r.0).collect();
    let ok: Vec<bool> = rows.iter().map(|x| x.1 == Some(r)).collect();
    let Some(mp) = min_pulse(&lens, &ok, 3) else { return fail("fewer than the 3 longest inputs give the same pulse", table) };
    if mp > 4 {
        return fail(format!("min-pulse {mp} gt: the second-edge test's 4 gt inputs are not handled"), table);
    }
    let (delay, l) = (r.0, r.1 as i64);
    // pairs: the second rising edge d gt after the first
    let mut class: Option<&str> = None;
    let mut rearm: Option<u32> = None;
    let mut inside = 0;
    for (from, to, d) in phases(s, "pair ") {
        let to = to.min(out.len());
        if out[to - 1] {
            return fail(format!("a second input {d} gt after the first leaves the output on"), table);
        }
        let Some(&(on, _)) = input_runs(t, from, to).first() else { continue };
        let os: Vec<(i64, i64)> = runs_in(&orun, from, to).into_iter().map(|(st, len)| (st as i64 - on as i64, len as i64)).collect();
        let d = d as i64;
        let two_full = os == vec![(delay, l), (d + delay, l)];
        if two_full {
            if rearm.is_none() {
                rearm = Some(d as u32);
            }
        } else {
            rearm = None;
        }
        if d < l {
            inside += 1;
            let this = if os == vec![(delay, l)] {
                "non-retriggerable"
            } else if os == vec![(delay, d + l)] {
                "retriggerable"
            } else {
                "other"
            };
            table.push(format!("second edge {d} gt after the first: {} -> {this}", os.iter().map(|(a, b)| format!("+{a} for {b}")).collect::<Vec<_>>().join(", ")));
            match class {
                None => class = Some(this),
                Some(c0) if c0 != this => return fail(format!("re-trigger behaviour changes with the delay ({c0}, then {this} at {d} gt)"), table),
                _ => {}
            }
        }
    }
    let Some(class) = class else { return fail(format!("L = {l} gt is too short to test a second edge inside the pulse (want L >= 6)"), table) };
    if class == "other" {
        return fail("a second edge inside the pulse gives neither one pulse of L (non-retriggerable) nor a pulse ending L after it (retriggerable)", table);
    }
    if let Some(cl) = claim {
        if cl != class {
            return fail(format!("claims {cl} but behaves as {class}"), table);
        }
    }
    let _ = inside;
    let metrics = vec![
        ("L".to_string(), gt(l)),
        ("delay".to_string(), gt(delay)),
        ("retrigger".to_string(), class.to_string()),
        ("re-arm".to_string(), rearm.map(|d| gt(d as i64)).unwrap_or_else(|| "> 160 gt".into())),
        ("min-pulse".to_string(), gt(mp as i64)),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `off-delay`: the output follows the input and stays on L more.
fn off_delay(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let out = t.out_seq(main_output(c));
    let orun = runs(&out);
    let mut table = Vec::new();
    let mut rows: Vec<(u32, Option<(i64, i64)>)> = Vec::new();
    for (from, to, w) in phases(s, "pulse ") {
        let to = to.min(out.len());
        if out[to - 1] {
            return fail(format!("latches: after a {w} gt input the output is still on at the end of its gap"), table);
        }
        let Some(&(on, _)) = input_runs(t, from, to).first() else { return fail(format!("pulse {w}: the input never turned on"), table) };
        let os = runs_in(&orun, from, to);
        let one = (os.len() == 1).then(|| (os[0].0 as i64 - on as i64, os[0].1 as i64 - w as i64));
        table.push(format!("in {w:>2} gt: {}", os.iter().map(|(st, len)| format!("+{} for {len}", *st as i64 - on as i64)).collect::<Vec<_>>().join(", ")));
        rows.push((w, one));
    }
    let Some(r) = rows.last().and_then(|r| r.1) else { return fail("the longest input does not give exactly one output pulse", table) };
    let (lat, l) = r;
    if l < 1 {
        return fail(format!("the output is {} the input's length: not an off-delay", if l == 0 { "exactly" } else { "shorter than" }), table);
    }
    let lens: Vec<u32> = rows.iter().map(|r| r.0).collect();
    let ok: Vec<bool> = rows.iter().map(|x| x.1 == Some(r)).collect();
    let Some(mp) = min_pulse(&lens, &ok, 3) else { return fail("fewer than the 3 longest inputs give input + L", table) };
    if mp > 4 {
        return fail(format!("min-pulse {mp} gt: the restart test's 4 gt inputs are not handled"), table);
    }
    // trains: gaps shorter than L are bridged
    for (from, to, g) in phases(s, "train ") {
        let to = to.min(out.len());
        let ins = input_runs(t, from, to);
        let os = runs_in(&orun, from, to);
        let (Some(first), Some(last)) = (ins.first(), ins.last()) else { continue };
        let fall = (last.0 + last.1) as i64 - first.0 as i64;
        let want: Vec<(i64, i64)> = if (g as i64) < l {
            vec![(lat, fall + l)]
        } else if (g as i64) > l {
            ins.iter().map(|(st, len)| (*st as i64 - first.0 as i64 + lat, *len as i64 + l)).collect()
        } else {
            continue;
        };
        let got: Vec<(i64, i64)> = os.iter().map(|(st, len)| (*st as i64 - first.0 as i64, *len as i64)).collect();
        table.push(format!("3 inputs {g} gt apart: {}", got.iter().map(|(a, b)| format!("+{a} for {b}")).collect::<Vec<_>>().join(", ")));
        if got != want {
            return fail(format!("3 inputs {g} gt apart: want {want:?}, got {got:?} (gaps shorter than L = {l} gt are bridged)"), table);
        }
    }
    let metrics = vec![("L".to_string(), gt(l)), ("latency".to_string(), gt(lat)), ("min-pulse".to_string(), gt(mp as i64))];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `on-delay`: the input must be held T gt for any output (shorter inputs are rejected); every
/// longer input gives one pulse of its length minus a fixed shortening, ending at a fixed latency
/// after the input falls.
fn on_delay(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let out = t.out_seq(main_output(c));
    let orun = runs(&out);
    let mut table = Vec::new();
    let mut rows: Vec<(u32, Vec<(i64, i64)>)> = Vec::new();
    for (from, to, w) in phases(s, "pulse ") {
        let to = to.min(out.len());
        if out[to - 1] {
            return fail(format!("latches: after a {w} gt input the output is still on at the end of its gap"), table);
        }
        let Some(&(on, _)) = input_runs(t, from, to).first() else { return fail(format!("pulse {w}: the input never turned on"), table) };
        // (delay from the input's fall to the output's fall, length)
        let os: Vec<(i64, i64)> = runs_in(&orun, from, to).into_iter().map(|(st, len)| ((st + len) as i64 - (on + w as usize) as i64, len as i64)).collect();
        table.push(format!("in {w:>2} gt: {}", if os.is_empty() { "no output".into() } else { os.iter().map(|(f, l)| format!("{l} gt, ends +{f} after the input")).collect::<Vec<_>>().join(", ") }));
        rows.push((w, os));
    }
    let Some((wmax, last)) = rows.last() else { return fail("no input", table) };
    if last.len() != 1 {
        return fail("the longest input does not give exactly one output pulse", table);
    }
    let (fall, len) = last[0];
    let short = *wmax as i64 - len;
    if short < 1 {
        return fail("the output is not shorter than the input: not an on-delay", table);
    }
    // the threshold: the shortest input with an output; every shorter one gives none, every
    // longer one the input length - the shortening
    let Some(th) = rows.iter().find(|r| !r.1.is_empty()).map(|r| r.0 as i64) else { return fail("no output", table) };
    if th < 2 {
        return fail("a 1 gt input already gives an output: nothing is rejected", table);
    }
    for (w, os) in &rows {
        let w = *w as i64;
        let good = if w < th { os.is_empty() } else { os.as_slice() == [(fall, w - short)] };
        if !good {
            return fail(format!("a {w} gt input gives {os:?}: want {} (held at least {th} gt, shortened by {short} gt)", if w < th { "no output".to_string() } else { format!("one pulse of {} gt ending +{fall}", w - short) }), table);
        }
    }
    let metrics = vec![("T".to_string(), gt(th)), ("shortening".to_string(), gt(short)), ("latency".to_string(), gt(fall))];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `divider N`: square clocks; the output repeats with period N x P.
fn divider(c: &Circuit, n: usize, s: &Schedule, t: &Trace) -> Outcome {
    let out = t.out_seq(main_output(c));
    let mut table = Vec::new();
    let mut res: Vec<(u32, bool, Option<(usize, i64)>)> = Vec::new();
    for (from, _to, p) in phases(s, "clock ") {
        let pu = p as usize;
        let np = n * pu;
        let (a, b) = (from + np, (from + 5 * np).min(out.len()));
        if b <= a + np {
            res.push((p, false, None));
            continue;
        }
        let o = &out[a..b];
        let periodic = (0..o.len() - np).all(|i| o[i] == o[i + np]);
        let rises = (1..o.len()).filter(|&i| o[i] && !o[i - 1]).count();
        let good = periodic && rises == 4 || periodic && rises == 3 && o[0];
        // the duty and the latency (first output rise after an input rise) over one output period
        let on = o[..np].iter().filter(|x| **x).count();
        let rise = (1..o.len()).find(|&i| o[i] && !o[i - 1]).map(|i| {
            let abs = a + i;
            let in_rise = (from..=abs).rev().find(|&f| f > 0 && t.inputs[0][f] && !t.inputs[0][f - 1]).unwrap_or(from);
            (abs - in_rise) as i64
        });
        table.push(format!("clock {p} gt: {}", if good { format!("divides by {n} (on {on} of {np} gt)") } else if periodic { format!("periodic, {rises} rises in {} input periods", 4 * n) } else { "not periodic with N x P".into() }));
        res.push((p, good, good.then(|| (on, rise.unwrap_or(0)))));
    }
    if !res.first().is_some_and(|r| r.1) {
        return fail(format!("does not divide the slowest clock ({} gt) by {n}", CLOCK_PERIODS[0]), table);
    }
    let fastest = res.iter().take_while(|r| r.1).last().unwrap();
    let (on, lat) = res[0].2.unwrap();
    let np = n * CLOCK_PERIODS[0] as usize;
    let square = (2 * on as i64 - np as i64).abs() <= 2;
    let metrics = vec![
        ("N".to_string(), n.to_string()),
        ("duty".to_string(), format!("{}%", (100 * on + np / 2) / np)),
        ("output".to_string(), if square { "square".into() } else { format!("pulse ({on} gt)") }),
        ("latency".to_string(), gt(lat)),
        ("min-period".to_string(), gt(fastest.0 as i64)),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// `pulse-train`: pulses every P gt while the input is held.
fn pulse_train(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let out = t.out_seq(main_output(c));
    let orun = runs(&out);
    let mut table = Vec::new();
    let mut period: Option<i64> = None;
    let mut latency: Option<i64> = None;
    let mut stops: Vec<i64> = Vec::new();
    let mut lens: Vec<i64> = Vec::new();
    for (from, to, w) in phases(s, "pulse ") {
        let to = to.min(out.len());
        if out[to - 1] {
            return fail(format!("still on at the end of the gap after a {w} gt hold"), table);
        }
        let Some(&(on, _)) = input_runs(t, from, to).first() else { return fail(format!("hold {w}: the input never turned on"), table) };
        let os: Vec<(i64, i64)> = runs_in(&orun, from, to).into_iter().map(|(st, len)| (st as i64 - on as i64, len as i64)).collect();
        table.push(format!("held {w} gt: {} pulses{}", os.len(), os.first().map(|f| format!(", first +{}", f.0)).unwrap_or_default()));
        let Some(first) = os.first() else { return fail(format!("no output while held {w} gt"), table) };
        match latency {
            None => latency = Some(first.0),
            Some(l) if l != first.0 => return fail(format!("the first pulse comes +{} after a {w} gt hold, +{l} before: not the same phase", first.0), table),
            _ => {}
        }
        let gaps: Vec<i64> = os.windows(2).map(|x| x[1].0 - x[0].0).collect();
        if let Some(&g0) = gaps.first() {
            if gaps.iter().any(|&g| g != g0) {
                return fail(format!("held {w} gt: uneven pulses ({gaps:?})"), table);
            }
            match period {
                None => period = Some(g0),
                Some(p) if p != g0 => return fail(format!("the period changes ({p}, then {g0})"), table),
                _ => {}
            }
        }
        if let Some(p) = period {
            if os.last().unwrap().0 > w as i64 + p {
                return fail(format!("held {w} gt: a pulse starts more than one period after the release"), table);
            }
        }
        let last = os.last().unwrap();
        stops.push(last.0 + last.1 - w as i64);
        lens.extend(os.iter().map(|x| x.1));
    }
    let Some(p) = period else { return fail("never 2 pulses in one hold: no period", table) };
    if p < 2 {
        return fail("one output run per hold", table);
    }
    let metrics = vec![
        ("P".to_string(), gt(p)),
        ("pulse".to_string(), range(&lens)),
        ("latency".to_string(), gt(latency.unwrap_or(0))),
        ("stop".to_string(), gt(stops.iter().copied().max().unwrap_or(0))),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}
