//! The sequential memory kinds (docs/circuits.md#memory): `tff-reset`, `dff`, `dff-enable` and
//! `shift N`: their stimulus and their checkers. The register kinds are judged against an ideal
//! model of themselves run on the schedule (q takes d at each clock edge of the claimed type),
//! once per edge type; the type that passes is reported.

use crate::check::{Outcome, settle};
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, Kind};

fn gt(n: i64) -> String {
    format!("{n} gt")
}

/// The input pulse lengths of the tff-reset checker's B phase (the shortest `t` pulse accepted).
pub const TFF_RESET_SHORT: [u32; 3] = [3, 2, 1];

/// The register checkers' main sequence: per clock cycle (a, b, e): d := a (and e) with the clock
/// low, the clock rises, d := b, the clock falls; each step `gap` frames. A rising-edge register
/// loads a, a falling-edge one b; a level latch fails (it follows d while transparent).
pub const DFF_CYCLES: [(bool, bool, bool); 8] =
    [(true, false, true), (false, true, true), (true, true, true), (false, false, true), (true, false, true), (false, false, true), (false, true, true), (true, true, true)];

/// [`DFF_CYCLES`] for `dff-enable`: cycles with e off must not load (each would change q).
pub const DFF_ENABLE_CYCLES: [(bool, bool, bool); 10] = [
    (true, false, true),
    (false, true, true),
    (true, true, false),
    (false, false, true),
    (true, false, false),
    (true, true, true),
    (false, false, false),
    (false, true, true),
    (true, true, false),
    (true, false, true),
];

/// The d-to-edge offsets the `dff` checker tries for its setup metric (0: d changes in the
/// edge's own frame) and its hold metric.
pub const DFF_SETUP: [u32; 5] = [8, 4, 2, 1, 0];
pub const DFF_HOLD: [u32; 4] = [8, 4, 2, 1];

/// The bits a `shift N` checker shifts in (N + 12 of them, from this pseudo-random pattern).
const SHIFT_BITS: [bool; 31] = [
    true, false, false, false, true, false, false, true, true, false, true, false, true, true, true, true, false, false, false, true, false, false, true, true, false, true, false, true, true,
    true, false,
];

/// The stimulus of a sequential memory kind, from frame `t`; returns the frame after it.
pub fn stimulus(kind: &Kind, s: &mut Schedule, mut t: u32, gap: u32, button: bool) -> u32 {
    match kind {
        Kind::TffReset => {
            // inputs in name order: r, t
            let p = if button { 1 } else { 4 };
            let long = if button { 1 } else { 20 };
            let main = [(1, p), (1, p), (1, p), (0, p), (0, p), (1, p), (0, p), (1, p), (1, p), (0, p), (1, long), (0, long), (1, long), (0, long)];
            // the real presses (the real-use audit): a wooden button and a lever held down, for t and r
            let presses: Vec<(usize, u32)> = if button || !crate::check::real_use() { Vec::new() } else { crate::check::PRESSES[1..].iter().flat_map(|&l| [(1, l), (0, l)]).collect() };
            for (i, len) in main.into_iter().chain(presses) {
                s.pulse(i, t, len);
                s.phase(t, t + len + gap, format!("A {} {len}", if i == 1 { "t" } else { "r" }));
                t += len + gap;
            }
            if !button {
                for len in TFF_RESET_SHORT {
                    for _ in 0..2 {
                        s.pulse(1, t, len);
                        s.phase(t, t + len + gap, format!("B t {len}"));
                        t += len + gap;
                    }
                }
                s.pulse(0, t, 4);
                s.phase(t, t + 4 + gap, "B r 4");
                t += 4 + gap;
            }
        }
        Kind::Dff | Kind::DffEnable | Kind::Shift(_) => {
            // inputs in name order: clk, d (, e)
            let cycles: Vec<(bool, bool, bool)> = match kind {
                Kind::Dff => DFF_CYCLES.to_vec(),
                Kind::DffEnable => DFF_ENABLE_CYCLES.to_vec(),
                Kind::Shift(n) => (0..*n as usize + 12).map(|k| (SHIFT_BITS[k % 31], SHIFT_BITS[(k + 11) % 31], true)).collect(),
                _ => unreachable!(),
            };
            for (k, (a, b, e)) in cycles.iter().enumerate() {
                s.set(1, t, *a);
                if *kind == Kind::DffEnable {
                    s.set(2, t, *e);
                }
                s.phase(t, t + gap, format!("M {} d={} clk=0{}", k + 1, *a as u8, if *kind == Kind::DffEnable { format!(" e={}", *e as u8) } else { String::new() }));
                t += gap;
                s.set(0, t, true);
                s.phase(t, t + gap, format!("M {} clk rises", k + 1));
                t += gap;
                s.set(1, t, *b);
                s.phase(t, t + gap, format!("M {} d={} clk=1", k + 1, *b as u8));
                t += gap;
                s.set(0, t, false);
                s.phase(t, t + gap, format!("M {} clk falls", k + 1));
                t += gap;
            }
            if *kind == Kind::Dff {
                // setup and hold sweeps: a prime clock pulse with d = !v (both edge types end with
                // q = !v), then the test edges around T
                let mut v = true;
                for test in ["setup-r", "setup-f", "hold-r", "hold-f"] {
                    let xs: &[u32] = if test.starts_with("setup") { &DFF_SETUP } else { &DFF_HOLD };
                    for &x in xs {
                        let from = t;
                        s.set(1, t, !v);
                        s.set(0, t + gap / 2, true);
                        s.set(0, t + gap, false);
                        let tt = t + 3 * gap;
                        let end = match test {
                            "setup-r" => {
                                s.set(1, tt - x, v);
                                s.set(0, tt, true);
                                s.set(0, tt + gap / 2, false);
                                tt + gap / 2 + gap
                            }
                            "setup-f" => {
                                s.set(0, tt - gap / 2, true);
                                s.set(1, tt - x, v);
                                s.set(0, tt, false);
                                tt + gap
                            }
                            "hold-r" => {
                                s.set(1, tt - gap / 2, v);
                                s.set(0, tt, true);
                                s.set(1, tt + x, !v);
                                s.set(0, tt + gap / 2, false);
                                tt + gap / 2 + gap
                            }
                            _ => {
                                s.set(1, tt - gap, v);
                                s.set(0, tt - gap / 2, true);
                                s.set(0, tt, false);
                                s.set(1, tt + x, !v);
                                tt + gap
                            }
                        };
                        s.phase(from, end, format!("S {test} {x}"));
                        t = end;
                        v = !v;
                    }
                }
            }
        }
        _ => {}
    }
    t
}

/// Judge a run of a sequential memory kind.
pub fn check(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace) -> Outcome {
    match kind {
        Kind::TffReset => tff_reset(c, s, t),
        _ => {
            let rising = register(c, kind, s, t, true);
            if rising.pass {
                return rising;
            }
            let falling = register(c, kind, s, t, false);
            if falling.pass {
                return falling;
            }
            let mut o = Outcome { pass: false, why: format!("rising edge: {}; falling edge: {}", rising.why, falling.why), metrics: Vec::new(), table: rising.table };
            o.table.push("-- falling edge --".into());
            o.table.extend(falling.table);
            o
        }
    }
}

fn tff_reset(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let k = crate::check::main_output(c);
    let out = t.out_seq(k);
    let mut table = Vec::new();
    let (mut latency, mut reset) = (0usize, 0usize);
    let mut lens: Vec<(u32, bool)> = Vec::new();
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let prev = out[from - 1];
        let (fin, last, changes) = settle(&out, from, to);
        // as the tff checker: one change per toggle, the phase spans the recovery gap
        let steady = true;
        let w: Vec<&str> = label.split(' ').collect();
        let (phase, input, len) = (w[0], w[1], w.get(2).and_then(|x| x.parse::<u32>().ok()).unwrap_or(0));
        if input == "t" {
            let toggled = steady && changes == 1 && fin != prev;
            table.push(format!("{label}: q {} -> {}{}", prev as u8, fin as u8, if toggled { format!(" after {} gt", last.unwrap_or(from) - from) } else { format!(", {changes} change(s): not a toggle") }));
            if phase == "A" {
                if !toggled {
                    return Outcome { pass: false, why: format!("{label}: a t pulse must toggle q exactly once"), metrics: Vec::new(), table };
                }
                // the latency of the short pulses (a long one may hold a slow toggle back)
                if len <= 4 {
                    latency = latency.max(last.unwrap_or(from) - from);
                }
            }
            lens.push((len, toggled));
        } else {
            let good = steady && !fin && changes == prev as usize;
            table.push(format!("{label}: q {} -> {}{}", prev as u8, fin as u8, if good { String::new() } else { format!(", {changes} change(s)  <- wrong") }));
            if phase == "A" {
                if !good {
                    return Outcome { pass: false, why: format!("{label}: an r pulse must clear q and keep it clear (q {} -> {}, {changes} change(s))", prev as u8, fin as u8), metrics: Vec::new(), table };
                }
                if prev {
                    reset = reset.max(last.unwrap_or(from) - from);
                }
            }
        }
    }
    let mut ls: Vec<u32> = lens.iter().map(|(l, _)| *l).collect();
    ls.sort();
    ls.dedup();
    let good = |l: u32| lens.iter().filter(|(x, _)| *x == l).all(|(_, ok)| *ok);
    let min_pulse = ls.iter().rev().take_while(|l| good(**l)).last().copied().unwrap_or(4);
    Outcome { pass: true, why: String::new(), metrics: vec![("latency".into(), gt(latency as i64)), ("reset".into(), gt(reset as i64)), ("min-pulse".into(), gt(min_pulse as i64))], table }
}

/// The ideal register of `kind` loading at the `rising` (or falling) clock edges: its q after
/// frame `f` for every frame (None while a shift register's cells still hold their unknown rest).
fn ideal(kind: &Kind, s: &Schedule, rising: bool, q0: bool, frames: usize) -> Vec<Option<bool>> {
    let n = match kind {
        Kind::Shift(n) => *n as usize,
        _ => 1,
    };
    // cells[0] is the input side, cells[n-1] the output; the rest state is q0 at the output
    let mut cells: Vec<Option<bool>> = vec![None; n];
    cells[n - 1] = Some(q0);
    if n == 1 {
        cells[0] = Some(q0);
    }
    let mut out = vec![None; frames];
    let mut edges = s.edges[0].iter().filter(|(_, on)| *on == rising).map(|(f, _)| *f as usize).peekable();
    for (f, slot) in out.iter_mut().enumerate() {
        while edges.peek().is_some_and(|e| *e == f) {
            edges.next();
            let load = *kind != Kind::DffEnable || s.on_at(2, f as u32);
            if load {
                cells.rotate_right(1);
                cells[0] = Some(s.on_at(1, f as u32));
            }
        }
        *slot = cells[n - 1];
    }
    out
}

fn register(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace, rising: bool) -> Outcome {
    let k = crate::check::main_output(c);
    let out = t.out_seq(k);
    let mut table = Vec::new();
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(1);
    let q = ideal(kind, s, rising, out[first - 1], out.len());
    let own = if rising { "-r " } else { "-f " };
    let mut latency = 0usize;
    // sweep results: (test, offset, pass)
    let mut sweeps: Vec<(String, u32, bool)> = Vec::new();
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let (fin, last, changes) = settle(&out, from, to);
        // steady over the last half step (a main step is one gap long)
        let g = (s.phases[0].1 - s.phases[0].0) as usize;
        let steady = last.is_none_or(|l| l < to - g / 2);
        let want = q[to - 1];
        let known = q[from - 1].is_some() && q[to - 1].is_some();
        let changed = known && q[to - 1] != q[from - 1];
        let ok = steady && want.is_none_or(|w| w == fin);
        if label.starts_with('S') {
            // a sweep: the type's own setup / hold tests only bound its metrics
            let w: Vec<&str> = label.split(' ').collect();
            let x = w.get(2).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
            table.push(format!("{label}: q = {}, want {}{}", fin as u8, want.map(|w| (w as u8).to_string()).unwrap_or("-".into()), if ok { "" } else { "  <- no" }));
            if label.contains(own) {
                sweeps.push((w[1].to_string(), x, ok));
            } else if !ok {
                return Outcome { pass: false, why: format!("{label}: q = {}, want {} (d steady around the {} edges)", fin as u8, want.map(|w| (w as u8).to_string()).unwrap_or("-".into()), if rising { "rising" } else { "falling" }), metrics: Vec::new(), table };
            }
            continue;
        }
        // before a shift register's cells are known, q may take its first value once
        let good = ok && if known { changes == changed as usize } else { changes <= 1 };
        table.push(format!("{label}: q = {}{}{}", fin as u8, if last.is_some() { format!(" after {} gt", last.unwrap() - from) } else { String::new() }, if good { "" } else { "  <- wrong" }));
        if !good {
            let why = if !steady {
                format!("{label}: q is not steady in the second half of the step")
            } else if want.is_some_and(|w| w != fin) {
                format!("{label}: q = {}, want {}", fin as u8, want.unwrap() as u8)
            } else {
                format!("{label}: q changes {changes} time(s), want {}", changed as u8)
            };
            return Outcome { pass: false, why, metrics: Vec::new(), table };
        }
        if changed {
            latency = latency.max(last.unwrap_or(from) - from);
        }
    }
    let edge = if rising { "rising" } else { "falling" };
    let mut metrics = Vec::new();
    if let Kind::Shift(n) = kind {
        metrics.push(("N".to_string(), n.to_string()));
    }
    metrics.push(("latency".into(), gt(latency as i64)));
    metrics.push(("edge".into(), edge.into()));
    if *kind == Kind::Dff {
        for test in ["setup", "hold"] {
            let mut xs: Vec<(u32, bool)> = sweeps.iter().filter(|(n, _, _)| n.starts_with(test)).map(|(_, x, ok)| (*x, *ok)).collect();
            xs.sort();
            // the smallest offset from which every larger tested offset passes
            let best = xs.iter().rev().take_while(|(_, ok)| *ok).last().map(|(x, _)| *x);
            metrics.push((test.to_string(), match best {
                Some(x) => gt(x as i64),
                None => format!("over {} gt", DFF_SETUP[0]),
            }));
        }
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}
