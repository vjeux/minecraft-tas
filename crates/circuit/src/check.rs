//! The per-kind checkers: each generates the stimulus its kind is judged with ([`stimulus`]) and
//! judges a recorded run ([`check`]). The numbers they report are the study's axis and metrics
//! (docs/circuits.md#metrics).

use crate::stimulus::{SETTLE, Schedule, euler_walk};
use crate::trace::{Trace, runs};
use crate::{Circuit, GATES2, InputHow, Kind, Pos};

/// The verdict of one run.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub pass: bool,
    /// Why it fails; on a pass, a flag worth reporting (a one-edge delay), else empty.
    pub why: String,
    /// The axis value and the metrics, in report order (`("L", "2 gt")`).
    pub metrics: Vec<(String, String)>,
    /// One line per test (an input pulse, a transition, a train).
    pub table: Vec<String>,
}

impl Outcome {
    pub(crate) fn fail(why: impl Into<String>, table: Vec<String>) -> Outcome {
        Outcome { pass: false, why: why.into(), metrics: Vec::new(), table }
    }

    /// `L 2 gt, delay 1 gt, min-pulse 1 gt`.
    pub fn metrics_text(&self) -> String {
        self.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")
    }
}

/// How long the circuit takes to come back to rest after an input edge (from a redsim probe run;
/// a default when redsim does not model it): the checkers space their tests by it.
#[derive(Clone, Copy, Debug)]
pub struct Sizing {
    pub gap: u32,
}

impl Default for Sizing {
    fn default() -> Self {
        Sizing { gap: 100 }
    }
}

/// The input pulse lengths the pulse checkers try.
pub const PULSE_LENS: [u32; 12] = [1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40];

/// [`PULSE_LENS`], then longer pulses (60, 80, 120, 160, 240, 320... gt) up to twice the recovery
/// gap: the longest input must exceed a long limiter's or extender's L (the gap covers its output).
pub fn pulse_lens(gap: u32) -> Vec<u32> {
    const LONG: [u32; 13] = [60, 80, 120, 160, 240, 320, 480, 640, 960, 1280, 1920, 2560, 3840];
    PULSE_LENS.iter().copied().chain(LONG.iter().copied().filter(|l| *l <= 2 * gap)).collect()
}
/// The pulse lengths the delay checker tries, then the gaps between two 20 gt pulses.
pub const DELAY_LENS: [u32; 11] = [1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30];
pub const DELAY_GAPS: [u32; 8] = [1, 2, 3, 4, 5, 6, 8, 10];
/// The T flip-flop checker: 8 pulses of 4 gt, then pulses of 10, 3, 2, 1 gt (the shortest it
/// accepts), then trains of 6 pulses at these periods (the fastest it follows).
pub const TFF_PERIODS: [u32; 11] = [30, 20, 16, 12, 10, 8, 6, 5, 4, 3, 2];
/// How a player gives an input (the real-use audit, 2026-10-21): a stone button holds 20 gt, a
/// wooden one 30 gt, a lever held down as long as the player wants (200 gt here). The kinds whose
/// input is one event per press (a T flip-flop, a counter, a latch's set and reset, a delay line)
/// must handle each.
pub const PRESSES: [u32; 3] = [20, 30, 200];
/// A counter's real presses, N of them after its 4 gt pulses: stone and wooden buttons, and a
/// lever held down once every 8 (the count must go on across them).
pub const COUNTER_PRESSES: [u32; 8] = [20, 30, 200, 20, 30, 20, 30, 20];
/// A lever flipped back too soon (the real-use audit, shared by the kinds a lever drives: a
/// piston extender, a flying machine's switch): on for these many gt then off (a `flip`), or off
/// for these many gt then on again (a `reflip`). From 4 gt: a 2 gt flip makes any sticky piston
/// spit its block (vanilla), and no hand flips a lever back within 100 ms.
pub const LEVER_FLIPS: [u32; 8] = [4, 5, 6, 8, 10, 12, 16, 20];

static REAL_USE_OFF: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The real-use audit's stimulus parts (2026-10-21: real presses, a lever held down, the lever
/// flipped back too soon) on or off: off only to list what they change (`circuit real-use`).
pub fn set_real_use(on: bool) {
    REAL_USE_OFF.store(!on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether the real-use audit's stimulus parts run ([`set_real_use`]).
pub fn real_use() -> bool {
    !REAL_USE_OFF.load(std::sync::atomic::Ordering::Relaxed)
}

fn gt(n: i64) -> String {
    format!("{n} gt")
}

/// The first frame the circuit may be driven: after its kick and the settle time.
fn start(c: &Circuit) -> u32 {
    c.clock.free_from() + SETTLE
}

/// The stimulus `kind` is judged with.
pub fn stimulus(c: &Circuit, kind: &Kind, z: &Sizing) -> Schedule {
    let mut s = stimulus_kind(c, kind, z);
    // R7: a driven input (the judge's repeater) makes no pulse or gap under 2 gt
    if c.inputs.iter().any(|i| matches!(i.how, InputHow::Driven { .. })) {
        s.drop_short(2);
    }
    s
}

fn stimulus_kind(c: &Circuit, kind: &Kind, z: &Sizing) -> Schedule {
    if *kind == Kind::Bud {
        return crate::bud::stimulus(c, z);
    }
    if let Kind::Compute(spec) = kind {
        return crate::compute::stimulus(c, *spec, z);
    }
    if let Kind::Lock(n) = kind {
        return crate::lock::stimulus(c, *n, z);
    }
    let mut s = Schedule::new(c.inputs.len());
    let mut t = start(c);
    let gap = z.gap.max(20);
    let button = c.inputs.first().is_some_and(|i| i.how == InputHow::Button);
    match kind {
        Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => {
            if button {
                for k in 0..3 {
                    s.pulse(0, t, 1);
                    s.phase(t, t + gap + 40, format!("press {}", k + 1));
                    t += gap + 40;
                }
            } else {
                for l in pulse_lens(gap) {
                    s.pulse(0, t, l);
                    s.phase(t, t + l + gap, format!("pulse {l}"));
                    t += l + gap;
                }
            }
        }
        Kind::Delay => {
            let lens: &[u32] = if button { &[1, 1, 1] } else { &DELAY_LENS };
            for &l in lens {
                s.pulse(0, t, l);
                s.phase(t, t + l + gap + if button { 40 } else { 0 }, format!("pulse {l}"));
                t += l + gap + if button { 40 } else { 0 };
            }
            if !button {
                for &g in &DELAY_GAPS {
                    s.pulse(0, t, 20);
                    s.pulse(0, t + 20 + g, 20);
                    s.phase(t, t + 40 + g + gap, format!("gap {g}"));
                    t += 40 + g + gap;
                }
                // a lever held down (the real-use audit): it must come out shifted too
                if real_use() {
                    let hold = PRESSES[PRESSES.len() - 1];
                    s.pulse(0, t, hold);
                    s.phase(t, t + hold + gap, format!("hold {hold}"));
                    t += hold + gap;
                }
            }
        }
        Kind::Gate(_) => {
            let n = c.inputs.len();
            let walk = euler_walk(1 << n);
            s.phase(c.clock.free_from(), t, format!("{} (rest)", state_name(0, n)));
            for &st in &walk[1..] {
                for i in 0..n {
                    s.set(i, t, st >> (n - 1 - i) & 1 == 1);
                }
                s.phase(t, t + gap, state_name(st, n));
                t += gap;
            }
        }
        Kind::Tff => {
            let p0 = if button { 1 } else { 4 };
            for k in 0..8 {
                s.pulse(0, t, p0);
                s.phase(t, t + p0 + gap, format!("A {} pulse {p0}", k + 1));
                t += p0 + gap;
            }
            if !button {
                // the real presses first (a lever held down, a wooden and a stone button: each
                // must toggle it exactly once), then the short ones (the shortest it takes)
                let presses: &[u32] = if real_use() { &PRESSES } else { &[] };
                for &p in presses.iter().rev().chain(&[10u32, 3, 2, 1]) {
                    for _ in 0..2 {
                        s.pulse(0, t, p);
                        s.phase(t, t + p + gap, format!("B pulse {p}"));
                        t += p + gap;
                    }
                }
                for &per in &TFF_PERIODS {
                    let p = (per / 2).max(1);
                    for j in 0..6 {
                        s.pulse(0, t + j * per, p);
                    }
                    s.phase(t, t + 6 * per + gap, format!("C period {per}"));
                    t += 6 * per + gap;
                }
            }
        }
        Kind::RsLatch => {
            // inputs in name order: r, s
            let p = if button { 1 } else { 4 };
            for (i, label, len) in [(1, "s", p), (1, "s", p), (0, "r", p), (0, "r", p), (1, "s", p), (0, "r", p), (1, "s", 20), (0, "r", 20)] {
                s.pulse(i, t, len);
                s.phase(t, t + len + gap, format!("{label} pulse {len}"));
                t += len + gap;
            }
            // the real presses (the real-use audit): a wooden button and a lever held down
            if !button && real_use() {
                for &len in &PRESSES[1..] {
                    for (i, label) in [(1, "s"), (0, "r")] {
                        s.pulse(i, t, len);
                        s.phase(t, t + len + gap, format!("{label} pulse {len}"));
                        t += len + gap;
                    }
                }
            }
        }
        Kind::DLatch => {
            // inputs in name order: d, e; (d, e) held `gap` frames each
            for (d, e) in [(1, 0), (1, 1), (0, 1), (1, 1), (1, 0), (0, 0), (0, 1), (0, 0), (1, 0), (1, 1), (0, 1), (0, 0)] {
                s.set(0, t, d == 1);
                s.set(1, t, e == 1);
                s.phase(t, t + gap, format!("d={d} e={e}"));
                t += gap;
            }
        }
        Kind::Counter(n) => {
            let p = if button { 1 } else { 4 };
            for k in 0..(3 * n + 1) {
                s.pulse(0, t, p);
                s.phase(t, t + p + gap, format!("pulse {}", k + 1));
                t += p + gap;
            }
            // the real presses (the real-use audit, 2026-10-21: a counter counts button presses
            // and lever flips): N more inputs, the count going on across them
            if !button && real_use() {
                for k in 0..*n as usize {
                    let l = COUNTER_PRESSES[k % COUNTER_PRESSES.len()];
                    s.pulse(0, t, l);
                    s.phase(t, t + l + gap, format!("press {} {l}", k + 1));
                    t += l + gap;
                }
            }
        }
        Kind::TffReset | Kind::Dff | Kind::DffEnable | Kind::Shift(_) => {
            t = crate::seq::stimulus(kind, &mut s, t, gap, button);
        }
        Kind::PistonExtender(_) => {
            for k in 0..3 {
                if button {
                    s.pulse(0, t, 1);
                    s.phase(t, t + 2 * gap, format!("press {}", k + 1));
                    t += 2 * gap;
                } else {
                    s.pulse(0, t, gap);
                    s.phase(t, t + 2 * gap, format!("cycle {}", k + 1));
                    t += 2 * gap;
                }
            }
            // the lever flipped back too soon (the real-use audit, 2026-10-21): off before the
            // extension ends, then on again before the retraction ends; each must leave it at
            // rest (`flip`) or extended (`reflip`), and a last full cycle must still work. The
            // longest first: the first that breaks it gives the shortest safe flip
            if !button && real_use() {
                for &k in LEVER_FLIPS.iter().rev().filter(|k| **k < gap) {
                    s.pulse(0, t, k);
                    s.phase(t, t + 2 * gap, format!("flip {k}"));
                    t += 2 * gap;
                }
                for &k in LEVER_FLIPS.iter().rev().filter(|k| **k < gap) {
                    s.pulse(0, t, gap);
                    s.pulse(0, t + gap + k, gap);
                    s.phase(t, t + 3 * gap + k, format!("reflip {k}"));
                    t += 3 * gap + k;
                }
                s.pulse(0, t, gap);
                s.phase(t, t + 2 * gap, "after flips");
                t += 2 * gap;
            }
        }
        Kind::FlyingMachine if start_input(c).is_some() => {
            // a one-way machine with a START input (coordinator 2026-10-21: "start: flip the
            // lever"): parked at rest, then the device keeps working at other moments of the
            // flight ([`flying_start`]). This is the first of the START_FLIPS runs
            // ([`start_variants`] has the others, [`start_check`] judges them).
            let (i, button) = start_input(c).unwrap();
            let t0 = c.clock.free_from() + 60;
            s.phase(0, t0, "rest");
            start_flips(&mut s, i, button, t0, START_FLIPS[0]);
            t = t0 + 400 + 60 * c.distance.unwrap_or(16);
            s.phase(t0, t, "launch");
        }
        Kind::FlyingMachine => {
            t = c.clock.free_from() + 200 + 60 * c.distance.unwrap_or(16);
            s.phase(0, t, "flight");
            // an on/off switch (a lever input, vjeux 2026-10-21 on the two-way machine: "Can you
            // add an on/off switch?"): off at rest, so the first flight must end docked; then
            // on/off cycles, each off edge at another moment of the round trip ([`flying_switch`])
            if let Some(i) = c.inputs.iter().position(|inp| inp.how == crate::InputHow::Lever) {
                let d = c.distance.unwrap_or(16);
                for k in 0..4u32 {
                    let run = 40 * d + 120 + 23 * k;
                    s.set(i, t, true);
                    s.phase(t, t + run, format!("run {}", k + 1));
                    t += run;
                    s.set(i, t, false);
                    s.phase(t, t + 40 * d + 200, format!("dock {}", k + 1));
                    t += 40 * d + 200;
                }
            }
        }
        Kind::Bud => unreachable!("the BUD stimulus is crate::bud::stimulus"),
        // judged by crate::shulker's own runner (judge chests), not by a stimulus
        Kind::ShulkerLoader(_) | Kind::ShulkerUnloader(_) | Kind::Sorter(_) => {}
        Kind::TntDuper | Kind::TntDuperTriggered => return crate::tnt::stimulus(c, kind, t, gap),
        Kind::Burst(_) | Kind::Divider(_) | Kind::Monostable(_) | Kind::OffDelay | Kind::OnDelay | Kind::PulseTrain => return crate::pulsemult::stimulus(c, kind, t, gap),
        Kind::Compute(..) => unreachable!("the compute stimulus is crate::compute::stimulus"),
        Kind::Lock(_) => unreachable!("the lock stimulus is crate::lock::stimulus"),
    }
    s.ticks = t;
    s
}

fn state_name(st: usize, n: usize) -> String {
    (0..n).map(|i| if st >> (n - 1 - i) & 1 == 1 { '1' } else { '0' }).collect()
}

/// Judge a run of `kind`'s stimulus.
pub fn check(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace) -> Outcome {
    let mut out = check_kind(c, kind, s, t);
    real_inputs(c, &mut out.metrics);
    if out.pass
        && real_use()
        && let Some(why) = buried_output(c, s, t)
    {
        let table = std::mem::take(&mut out.table);
        out = Outcome::fail(why, table);
    }
    out
}

/// A player takes every output (the real-use audit, 2026-10-21; the display kind's buried lamps
/// were the first case): from the output cell a wire must find its way out of the build's box
/// through cells nothing occupies at any frame of the run (no design block, no block a piston
/// moves through, no judge device; the BUD judge's footprint probes are the judge's own test
/// blocks, not part of the build). The first output that cannot, as the reason.
pub fn buried_output(c: &Circuit, s: &Schedule, t: &Trace) -> Option<String> {
    let (lo, hi) = c.clock.bounds();
    let outside = |p: Pos| (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]);
    let probes: std::collections::HashSet<Pos> =
        if s.events.iter().any(|e| matches!(e.1, crate::stimulus::EventAt::Probe(_))) { c.probe_cells().into_iter().collect() } else { Default::default() };
    let mut occupied: std::collections::HashSet<Pos> = std::collections::HashSet::new();
    for f in &t.cells {
        occupied.extend(f.iter().map(|(p, _)| *p).filter(|p| !probes.contains(p)));
    }
    occupied.extend(c.clock.design.blocks.keys().copied());
    'outputs: for o in &c.outputs {
        if outside(o.pos) {
            continue;
        }
        let mut seen: std::collections::HashSet<Pos> = std::collections::HashSet::from([o.pos]);
        let mut queue = std::collections::VecDeque::from([o.pos]);
        while let Some(p) = queue.pop_front() {
            for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                if occupied.contains(&n) || !seen.insert(n) {
                    continue;
                }
                if outside(n) {
                    continue 'outputs;
                }
                queue.push_back(n);
            }
        }
        return Some(format!("output {} at {:?} is buried: no free cell leads from it out of the build, so a player cannot wire it to anything", o.name, o.pos));
    }
    None
}

/// R7 (real input devices): a driven input's repeater cannot pulse or pause for less than 2 gt (it
/// stretches a 1 gt pulse to 2 gt and swallows a 1 gt gap), so a measured minimum below that came
/// from a stretched input: it is reported as the shortest a real input makes. A driven input fed
/// through the build's own air cells (no side of it leads straight out) says so.
fn real_inputs(c: &Circuit, metrics: &mut Vec<(String, String)>) {
    if !c.inputs.iter().any(|i| matches!(i.how, InputHow::Driven { .. })) {
        return;
    }
    for (k, v) in metrics.iter_mut() {
        let floor = match k.as_str() {
            "min-pulse" | "min-gap" => 2,
            "min-period" => 4,
            _ => continue,
        };
        if v.strip_suffix(" gt").and_then(|x| x.trim().parse::<u32>().ok()).is_some_and(|n| n <= floor) {
            *v = format!("{floor} gt (the shortest a real input makes)");
        }
    }
    if c.inputs.iter().any(|i| matches!(i.how, InputHow::Driven { through: true, .. })) && !metrics.is_empty() {
        metrics.push(("input".into(), "fed through the build".into()));
    }
}

fn check_kind(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace) -> Outcome {
    if let Some((name, p, id)) = c.passive_output() {
        return Outcome::fail(format!("output {name} at {p:?} holds {id}, which drives nothing by itself: leave the output cell air (the judge puts a lamp there) or put a consumer there"), Vec::new());
    }
    if let Some(why) = update_less(c, t) {
        return Outcome::fail(why, Vec::new());
    }
    match kind {
        Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => pulse(c, kind, s, t),
        Kind::Delay => delay(c, s, t),
        Kind::Gate(claim) => gate(c, claim.as_deref(), s, t),
        Kind::Tff => tff(c, s, t),
        Kind::RsLatch | Kind::DLatch => latch(c, kind, s, t),
        Kind::Counter(n) => counter(c, *n, s, t),
        Kind::TffReset | Kind::Dff | Kind::DffEnable | Kind::Shift(_) => crate::seq::check(c, kind, s, t),
        Kind::PistonExtender(n) => pext(c, *n, s, t),
        Kind::FlyingMachine if start_input(c).is_some() => flying_start(c, s, t),
        Kind::FlyingMachine if c.inputs.iter().any(|i| i.how == crate::InputHow::Lever) => flying_switch(c, s, t),
        Kind::FlyingMachine => flying(c, t),
        Kind::Bud => crate::bud::check(c, s, t),
        Kind::ShulkerLoader(_) | Kind::ShulkerUnloader(_) => Outcome::fail("a shulker loader or unloader is judged by its own runner (crate::shulker), not by a trace", Vec::new()),
        Kind::Sorter(_) => Outcome::fail("an item sorter is judged by its own runner (crate::sorter), not by a trace", Vec::new()),
        Kind::TntDuper | Kind::TntDuperTriggered => crate::tnt::useful(c, t, match kind {
            Kind::TntDuper if c.inputs.iter().any(|i| i.how == crate::InputHow::Lever) => crate::tnt::check_lever(c, s, t),
            Kind::TntDuper if !c.inputs.is_empty() => crate::tnt::check_switched(c, s, t),
            Kind::TntDuper => crate::tnt::check_duper(c, s, t),
            _ => crate::tnt::check_triggered(c, s, t),
        }),
        Kind::Burst(_) | Kind::Divider(_) | Kind::Monostable(_) | Kind::OffDelay | Kind::OnDelay | Kind::PulseTrain => crate::pulsemult::check(c, kind, s, t),
        Kind::Compute(spec) => crate::compute::check(c, *spec, s, t),
        Kind::Lock(n) => crate::lock::check(c, *n, s, t),
    }
}

/// A circuit must drive something: the judge's lamp in each air output cell
/// ([`crate::CONSUMER`]) must follow the output, from the first frame the circuit may be driven:
/// lit whenever the output is on, and unlit within 5 gt after the output turns off (a lamp turns off
/// 4 gt after the block update that tells it; a later flash while the output stays off, from a
/// state inside one tick, is a glitch, not a stale lamp). An output that changes without a block
/// update (a dust redirected without a power change) leaves the lamp stale: why it fails.
pub fn update_less(c: &Circuit, t: &Trace) -> Option<String> {
    let from = (start(c) as usize).min(t.frames());
    for p in c.watched_lamps() {
        let Some(k) = c.outputs.iter().position(|o| o.pos == p) else {
            continue;
        };
        let name = &c.outputs[k].name;
        // the frame the output turned off, and whether the lamp has gone off since
        let mut fell: Option<(usize, bool)> = None;
        for f in from..t.frames() {
            let on = t.on(k, f);
            let lit = crate::trace::lamp_lit(t.state(f, p));
            if on {
                if !lit {
                    return Some(format!("update-less output: {name} turns on at frame {f} without a block update (a lamp in its cell stays unlit), it drives nothing"));
                }
                fell = None;
                continue;
            }
            let (at, gone) = fell.unwrap_or((f, false));
            let gone = gone || !lit;
            if !gone && f >= at + 5 {
                return Some(format!("update-less output: {name} is off from frame {at} without a block update (a lamp in its cell stays lit), it drives nothing"));
            }
            fell = Some((at, gone));
        }
    }
    None
}

/// A flying machine (track fly's tracker, [`crate::fly`]): a steady motion along `travel` (frame
/// t + P = frame t shifted by d cells, at least 2 periods), intact over `distance` cells (or to the
/// corridor's end); or a steady shuttle in place that takes 2+ positions (two-way).
fn flying(c: &Circuit, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    let travel = c.travel.unwrap_or([1, 0, 0]);
    let kick = match &c.clock.kick {
        Some(clock::Kick::Place { pos, ticks, .. }) => Some((*pos, c.clock.kick_at, c.clock.kick_at + ticks)),
        _ => None,
    };
    let mut r = crate::fly::Recorder::new(&c.clock.design.blocks, kick, &c.stations);
    for f in 0..t.frames() {
        r.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    let n = t.frames();
    let from = c.clock.free_from() as usize;
    let first_change = (1..n).find(|&f| r.canon[f] != r.canon[0] || r.corner[f] != r.corner[0]);
    let along = |s: &Pos| s[0] * travel[0] + s[1] * travel[1] + s[2] * travel[2];
    let intact: Vec<(usize, Pos)> = r.shifts.iter().enumerate().filter_map(|(f, s)| s.map(|s| (f, s))).collect();
    let far = intact.iter().map(|(_, s)| along(s)).max().unwrap_or(0);
    let distinct: std::collections::BTreeSet<Pos> = intact.iter().map(|(_, s)| *s).collect();
    let latency = first_change.map(|f| f.saturating_sub(c.clock.kick_at as usize));
    if let Some(e) = &t.ended {
        table.push(e.clone());
    }
    table.push(format!("first move at frame {}, intact positions seen: {}, farthest {far} cells along {travel:?}", first_change.map(|f| f.to_string()).unwrap_or("-".into()), distinct.len()));
    match crate::fly::steady(&r, from) {
        Some(st) if st.cells() > 0 && st.periods >= 2 => {
            let k = st.cells();
            let dir = st.d.map(|x| x / k);
            table.push(format!("steady from frame {}: period {} gt, {k} cell(s) per period along {dir:?} ({} periods seen)", st.start, st.period, st.periods));
            if st.d.iter().filter(|x| **x != 0).count() != 1 || dir != travel {
                return Outcome::fail(format!("flies along {:?}, not along travel {travel:?}", st.d), table);
            }
            let need = c.distance.unwrap_or(0) as i32;
            if far < need && t.ended.is_none() {
                return Outcome::fail(format!("intact only {far} cells along travel (distance {need})"), table);
            }
            let speed = if st.period % k as u32 == 0 { format!("{} gt/block", st.period / k as u32) } else { format!("{}/{} gt/block", st.period, k) };
            Outcome {
                pass: true,
                why: String::new(),
                metrics: vec![("speed".into(), speed), ("period".into(), format!("{} gt, {k} cell(s)", st.period)), ("latency".into(), latency.map(|l| gt(l as i64)).unwrap_or("-".into()))],
                table,
            }
        }
        Some(st) if st.periods >= 2 && distinct.len() >= 2 => {
            table.push(format!("steady in place from frame {}: period {} gt, {} positions", st.start, st.period, distinct.len()));
            Outcome { pass: true, why: String::new(), metrics: vec![("two-way".into(), format!("period {} gt over {} positions", st.period, distinct.len())), ("latency".into(), latency.map(|l| gt(l as i64)).unwrap_or("-".into()))], table }
        }
        Some(st) if st.periods >= 2 => Outcome::fail(format!("oscillates in place (period {} gt): a clock, not a machine", st.period), table),
        _ => Outcome::fail(if first_change.is_none() { "never moves".to_string() } else { "no steady motion (it breaks, stops, or never repeats its shape)".to_string() }, table),
    }
}

/// The audit's lever flips (clk-misc 2026-10-21, real use: no hand flips a lever back within
/// 100 ms, and a 2 gt flip makes any sticky piston spit, so 4 gt is the floor). A one-way flyer's
/// start lever is judged once per value k: it turns on, then off k gt later, on again k gt after
/// that, off once more, and twice more long after the machine left ([`start_flips`]).
pub const START_FLIPS: [u32; 8] = [4, 5, 6, 8, 10, 12, 16, 20];

/// One start run's device edges from `t0` with flip gap `k` (a lever: on at t0, off at t0 + k, on
/// at t0 + 2k, off at t0 + 3k, on 40 gt and off 60 gt after that; a button: pressed at t0, again
/// k gt after it pops back up (a stone button stays pressed 10 gt, a wooden one 15) and once
/// more 60 gt later), with the "launch" phase.
fn start_flips(s: &mut Schedule, i: usize, button: bool, t0: u32, k: u32) {
    let mut last = t0;
    if button {
        for at in [t0, t0 + 15 + k, t0 + 15 + k + 60] {
            s.pulse(i, at, 1);
            last = at;
        }
    } else {
        for (n, at) in [t0, t0 + k, t0 + 2 * k, t0 + 3 * k, t0 + 3 * k + 40, t0 + 3 * k + 100].into_iter().enumerate() {
            s.set(i, at, n % 2 == 0);
            last = at;
        }
    }
    s.ticks = s.ticks.max(last);
}

/// The other START_FLIPS runs of a one-way flyer with a start device (the first is the kind's
/// stimulus): (k, its schedule). Empty for every other design.
pub fn start_variants(c: &Circuit, base: &Schedule) -> Vec<(u32, Schedule)> {
    let Some((i, button)) = start_input(c) else { return Vec::new() };
    let Some(&(t0, end, _)) = base.phases.iter().find(|p| p.2 == "launch") else { return Vec::new() };
    START_FLIPS[1..]
        .iter()
        .map(|&k| {
            let mut s = base.clone();
            s.edges[i].clear();
            start_flips(&mut s, i, button, t0, k);
            s.ticks = base.ticks.max(end);
            (k, s)
        })
        .collect()
}

/// A passing one-way start flyer must pass its other START_FLIPS runs too: `run` gives each
/// schedule's trace (redsim or this build in the game). The first failure names its flip gap.
pub fn start_check<F>(c: &Circuit, kind: &Kind, out: Outcome, base: &Schedule, mut run: F) -> anyhow::Result<Outcome>
where
    F: FnMut(&Schedule) -> anyhow::Result<std::result::Result<Trace, String>>,
{
    if !out.pass || *kind != Kind::FlyingMachine {
        return Ok(out);
    }
    let mut out = out;
    for (k, s) in start_variants(c, base) {
        let t = match run(&s)? {
            Ok(t) => t,
            Err(why) => return Ok(Outcome::fail(format!("flips {k} gt apart: {why}"), out.table)),
        };
        let o = check(c, kind, &s, &t);
        if !o.pass {
            return Ok(Outcome::fail(format!("flips {k} gt apart: {}", o.why), o.table));
        }
        out.table.push(format!("flips {k} gt apart: flies intact ({})", o.metrics.iter().map(|(a, b)| format!("{a} {b}")).collect::<Vec<_>>().join(", ")));
    }
    Ok(out)
}

/// A one-way flying machine's START input: a button, or a lever on a machine with no `port start`
/// kick (with a kick, a lever is a docking machine's on/off switch: [`flying_switch`]). (index,
/// is a button).
fn start_input(c: &Circuit) -> Option<(usize, bool)> {
    c.inputs.iter().enumerate().find_map(|(i, inp)| match inp.how {
        crate::InputHow::Button => Some((i, true)),
        crate::InputHow::Lever if c.clock.kick.is_none() => Some((i, false)),
        _ => None,
    })
}

/// A one-way flying machine with a START input ([`stimulus`], coordinator 2026-10-21: "start:
/// flip the lever"). Parked at rest: nothing changes, stations included, until the first flip or
/// press. Then it must leave and fly its distance intact along `travel` in a steady motion while
/// the device keeps working at other moments of the launch: an extra flip never breaks it, never
/// starts it twice, never leaves a block behind (anything left behind breaks the machine's shape,
/// so it never stands intact again). The launch station (the device and its support, `port
/// station`) ends as built, the lever's own state aside. Metrics: speed, latency (the first flip
/// to the first move), start.
fn flying_start(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    let travel = c.travel.unwrap_or([1, 0, 0]);
    let mut r = crate::fly::Recorder::machine(&c.clock.design.blocks, None, &c.stations);
    for f in 0..t.frames() {
        r.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    let n = t.frames();
    let Some(t0) = s.phases.iter().find(|p| p.2 == "launch").map(|p| p.0 as usize) else {
        return Outcome::fail("no launch phase in the stimulus", table);
    };
    let from = (c.clock.free_from() as usize).min(t0);
    if t0 >= n {
        return Outcome::fail("the run ends before the first flip", table);
    }
    if let Some(f) = (from..t0).find(|&f| t.cells[f] != t.cells[from]) {
        return Outcome::fail(format!("not parked: something changes at frame {f}, before the first flip (frame {t0})"), table);
    }
    let Some(first) = (t0..n).find(|&f| r.canon[f] != r.canon[t0] || r.corner[f] != r.corner[t0]) else {
        return Outcome::fail("never moves after the first flip", table);
    };
    let along = |p: &Pos| p[0] * travel[0] + p[1] * travel[1] + p[2] * travel[2];
    let far = (t0..n).filter_map(|f| r.shifts[f].map(|p| along(&p))).max().unwrap_or(0);
    let start = if c.inputs.iter().any(|i| i.how == crate::InputHow::Button) { "press the button" } else { "flip the lever" };
    if let Some(e) = &t.ended {
        table.push(e.clone());
    }
    table.push(format!("parked until the first flip (frame {t0}), first move at frame {first}, farthest intact {far} cells along {travel:?}"));
    // the launch station as built (structural: a lever's `powered` aside)
    if let Some(last) = t.cells.last() {
        for p in &c.stations {
            let built = c.clock.design.blocks.get(p).map(|b| clock::canon::structural(b));
            let now = last.iter().find(|(q, _)| q == p).map(|(_, id)| clock::canon::structural(&t.names[*id as usize]));
            if built != now {
                return Outcome::fail(format!("the launch station changed at {p:?}: {} at the end, {} as built", now.as_deref().unwrap_or("air"), built.as_deref().unwrap_or("air")), table);
            }
        }
    }
    let Some(st) = crate::fly::steady(&r, t0) else {
        return Outcome::fail("no steady flight after the launch (it breaks, stops, or never repeats its shape)", table);
    };
    let k = st.cells();
    if k == 0 || st.periods < 2 {
        return Outcome::fail(format!("no flight after the launch (period {} gt, {k} cells)", st.period), table);
    }
    let dir = st.d.map(|x| x / k);
    table.push(format!("steady from frame {}: period {} gt, {k} cell(s) per period along {dir:?} ({} periods seen)", st.start, st.period, st.periods));
    if st.d.iter().filter(|x| **x != 0).count() != 1 || dir != travel {
        return Outcome::fail(format!("flies along {:?}, not along travel {travel:?}", st.d), table);
    }
    let need = c.distance.unwrap_or(0) as i32;
    if far < need && t.ended.is_none() {
        return Outcome::fail(format!("intact only {far} cells along travel (distance {need}): extra flips broke it or it stopped"), table);
    }
    let speed = if st.period % k as u32 == 0 { format!("{} gt/block", st.period / k as u32) } else { format!("{}/{} gt/block", st.period, k) };
    Outcome {
        pass: true,
        why: String::new(),
        metrics: vec![("speed".into(), speed), ("period".into(), format!("{} gt, {k} cell(s)", st.period)), ("latency".into(), gt((first - t0) as i64)), ("start".into(), start.to_string())],
        table,
    }
}

/// A two-way flying machine with an on/off lever ([`stimulus`]: off at rest, then four on/off
/// cycles). Off: it must end DOCKED — nothing changes for the last 100 gt of the phase, the
/// machine intact, at the same place every time (the station that obeys the switch). On: it must
/// leave the dock, fly at least 3 cells away and come back to within a cell of the dock (a real
/// round trip, not a twitch). Metrics: the round trip, how long after the off edge it stands still
/// at the latest, how long after the on edge it moves at the latest.
fn flying_switch(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    let travel = c.travel.unwrap_or([1, 0, 0]);
    let kick = match &c.clock.kick {
        Some(clock::Kick::Place { pos, ticks, .. }) => Some((*pos, c.clock.kick_at, c.clock.kick_at + ticks)),
        _ => None,
    };
    let mut r = crate::fly::Recorder::new(&c.clock.design.blocks, kick, &c.stations);
    for f in 0..t.frames() {
        r.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    if let Some(e) = &t.ended {
        table.push(e.clone());
        return Outcome::fail(format!("the run ended early: {e}"), table);
    }
    let along = |p: &Pos| p[0] * travel[0] + p[1] * travel[1] + p[2] * travel[2];
    let same = |a: usize, b: usize| r.canon[a] == r.canon[b] && r.corner[a] == r.corner[b];
    const HOLD: usize = 100;
    let (mut dock, mut stop_max, mut start_max, mut trip) = (None::<Pos>, 0usize, 0usize, None::<usize>);
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(r.canon.len()));
        if to <= from + HOLD {
            return Outcome::fail(format!("{label}: too short to judge"), table);
        }
        let last = to - 1;
        if label == "flight" || label.starts_with("dock") {
            if !(to - HOLD..to).all(|f| same(f, last)) {
                return Outcome::fail(format!("{label}: still moving {HOLD} gt before the phase ends: the switch did not stop it"), table);
            }
            let Some(pos) = r.shifts[last] else {
                return Outcome::fail(format!("{label}: stopped, but the machine is not intact"), table);
            };
            match dock {
                Some(p) if p != pos => return Outcome::fail(format!("{label}: docks at {pos:?}, earlier at {p:?}: it must always stop at the switch station"), table),
                _ => dock = Some(pos),
            }
            let still = (from..to).rev().take_while(|&f| same(f, last)).last().unwrap_or(from);
            if label != "flight" {
                stop_max = stop_max.max(still - from);
            }
            table.push(format!("{label}: docked at {pos:?} from frame {still} ({} gt after the phase starts)", still - from));
        } else if label.starts_with("run") {
            let Some(first) = (from..to).find(|&f| !same(f, from - 1)) else {
                return Outcome::fail(format!("{label}: never moves after the switch turns on"), table);
            };
            start_max = start_max.max(first - from);
            let home = dock.map(|p| along(&p)).unwrap_or(0);
            let ps: Vec<(usize, i32)> = (from..to).filter_map(|f| r.shifts[f].map(|p| (f, (along(&p) - home).abs()))).collect();
            let far = ps.iter().map(|(_, x)| *x).max().unwrap_or(0);
            let back = ps.iter().position(|(_, x)| *x == far).is_some_and(|i| ps[i..].iter().any(|(_, x)| *x <= 1));
            if far < 3 || !back {
                return Outcome::fail(format!("{label}: does not shuttle (it flies {far} cells from the dock{})", if back { "" } else { " and never comes back" }), table);
            }
            // the round trip: the smallest period over which a whole window of the run repeats
            if trip.is_none() {
                let a = first + 40;
                trip = (8..(to - a) / 2).find(|&p| (a..a + p).all(|f| same(f, f + p)));
                // a machine that makes one trip and parks while the switch is still on is no
                // docking machine (track fly 2026-10-21: an annealed one did exactly that)
                if trip.is_none() {
                    return Outcome::fail(format!("{label}: no round trip repeats while the switch is on: it must shuttle until the switch turns off"), table);
                }
            }
            if let Some(p) = trip
                && to > p + 10
                && !(to - p - 10..to).any(|f| !same(f, f - 1))
            {
                return Outcome::fail(format!("{label}: stops shuttling before the switch turns off (still for the last {} gt)", p + 10), table);
            }
            table.push(format!("{label}: moves {} gt after the switch turns on, flies {far} cells and comes back", first - from));
        }
    }
    let mut metrics = Vec::new();
    if let Some(p) = trip {
        metrics.push(("two-way".into(), format!("round trip {p} gt")));
    }
    metrics.push(("switch".into(), format!("docks within {stop_max} gt, restarts within {start_max} gt")));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// A piston extender: per cycle, the payload stands at p0 + N * travel (placed, nothing moving)
/// from some frame after the on edge to the off edge (a lever; a button: at some point of the
/// press), then the whole rest state comes back exactly after the off edge and stays.
fn pext(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    let Some(d) = c.travel else {
        return Outcome::fail("no travel: dx dy dz", table);
    };
    let p0 = c.payloads[0].pos;
    let target = [p0[0] + d[0] * n as i32, p0[1] + d[1] * n as i32, p0[2] + d[2] * n as i32];
    let rest_at = (start(c) as usize).saturating_sub(1);
    let Some(payload) = t.state(rest_at, p0).map(|s| s.to_string()) else {
        return Outcome::fail(format!("no payload block at {p0:?} at rest"), table);
    };
    let rest = &t.cells[rest_at];
    let moving = |f: usize| t.cells[f].iter().any(|(_, id)| t.names[*id as usize].starts_with("minecraft:moving_piston"));
    let pid = t.names.iter().position(|n| *n == payload).map(|i| i as u32);
    let once = |f: usize| t.cells[f].iter().filter(|(_, id)| Some(*id) == pid).count() == 1;
    if !once(rest_at) {
        return Outcome::fail(format!("the payload's state ({}) must be unique in the design", clock::short(&payload)), table);
    }
    let extended = |f: usize| !moving(f) && t.state(f, target) == Some(payload.as_str()) && once(f);
    let button = c.inputs[0].how == InputHow::Button;
    let mut cycles: Vec<(usize, usize)> = Vec::new();
    // the lever flipped back too soon (the real-use audit): the first failure
    let mut flip_fail: Option<String> = None;
    let restored = |from: usize, to: usize| (from..to).find(|&f| !moving(f) && t.cells[f] == *rest && (f..to).all(|g| t.cells[g] == *rest));
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(t.frames()));
        if label.starts_with("flip ") || label.starts_with("reflip ") {
            if flip_fail.is_some() {
                continue;
            }
            let ins = runs(&t.inputs[0][from..to]);
            // what differs from the rest state at the phase's end (stranded blocks)
            let left = || -> String {
                let last = to - 1;
                let diff: Vec<String> = t.cells[last].iter().filter(|c| !rest.contains(c)).take(3).map(|(p, id)| format!("{p:?} {}", clock::short(&t.names[*id as usize]))).collect();
                if diff.is_empty() { "cells missing".into() } else { diff.join(", ") }
            };
            let why = match (label.starts_with("reflip"), ins.as_slice()) {
                (false, [(a, l)]) => restored(from + a + l, to).is_none().then(|| format!("{label}: on {l} gt then off: the rest state does not come back (left: {})", left())),
                (true, [(a1, l1), (a2, l2)]) => {
                    let (off1, on2, off2) = (from + a1 + l1, from + a2, from + a2 + l2);
                    if !extended(off1 - 1) {
                        Some(format!("{label}: not extended before the first off edge"))
                    } else if !extended(off2 - 1) {
                        Some(format!("{label}: off {} gt then on again: not extended {} gt later", on2 - off1, off2 - on2))
                    } else if restored(off2, to).is_none() {
                        Some(format!("{label}: the rest state does not come back after the last off edge"))
                    } else {
                        None
                    }
                }
                _ => Some(format!("{label}: the input pulses are not the stimulus' ({ins:?})")),
            };
            if let Some(w) = why {
                table.push(w.clone());
                flip_fail = Some(w);
            }
            continue;
        }
        if label == "after flips" {
            // a full cycle after the flips: a broken machine shows here (folded into `flips`)
            let ok = input_pulse(t, 0, from, to).is_some_and(|(on, len)| {
                let off = on + len;
                (on..off).find(|&f| extended(f)).is_some_and(|e| (e..off).all(|f| extended(f))) && restored(off, to).is_some()
            });
            table.push(format!("{label}: a full cycle {}", if ok { "works" } else { "fails" }));
            if !ok && flip_fail.is_none() {
                flip_fail = Some("a full cycle after the flips fails".into());
            }
            continue;
        }
        let Some((on, len)) = input_pulse(t, 0, from, to) else {
            return Outcome::fail(format!("{label}: the input never turned on"), table);
        };
        let off = on + len;
        // extended: from its first frame to the off edge (a lever), or at some frame (a button)
        let end = if button { to } else { off };
        let Some(e) = (on..end).find(|&f| extended(f)) else {
            table.push(format!("{label}: never extended"));
            return Outcome::fail(format!("{label}: the payload never stands still at {target:?} (N = {n} along {d:?}) after the input turns on"), table);
        };
        if !button && let Some(f) = (e..off).find(|&f| !extended(f)) {
            table.push(format!("{label}: extended at +{} but not held (frame {f})", e - on));
            return Outcome::fail(format!("{label}: the payload leaves {target:?} at frame {f} while the input is still on"), table);
        }
        // restored: the whole rest state back, and staying until the phase ends
        let Some(r) = (off.max(e)..to).find(|&f| !moving(f) && t.cells[f] == *rest && (f..to).all(|g| t.cells[g] == *rest)) else {
            let last = to - 1;
            let diff: Vec<String> = t.cells[last].iter().filter(|c| !rest.contains(c)).take(4).map(|(p, id)| format!("{p:?} {}", clock::short(&t.names[*id as usize]))).collect();
            table.push(format!("{label}: extended at +{}, not restored", e - on));
            return Outcome::fail(format!("{label}: the rest state does not come back after the input turns off (left: {})", if diff.is_empty() { "cells missing".to_string() } else { diff.join(", ") }), table);
        };
        // retract: from the off edge (a lever) or from the press (a button), to the first frame of
        // the final rest
        let from_edge = if button { on } else { off };
        table.push(format!("{label}: extended at +{} gt, restored {} gt after the {}", e - on, r - from_edge, if button { "press" } else { "off edge" }));
        cycles.push((e - on, r - from_edge));
    }
    let (ext, ret) = cycles[0];
    let why = if cycles.iter().all(|c| *c == cycles[0]) { String::new() } else { format!("the cycles differ: {cycles:?} (extend, retract)") };
    let retract = if button { format!("{} (from the press)", gt(ret as i64)) } else { gt(ret as i64) };
    let mut metrics = vec![("N".into(), n.to_string()), ("extend".into(), gt(ext as i64)), ("retract".into(), retract)];
    if s.phases.iter().any(|p| p.2.starts_with("flip ")) {
        // short on the card: the first flip that breaks it (the table has the cells it strands)
        metrics.push(("flips".into(), match &flip_fail {
            None => "safe (the lever flipped back at any time)".into(),
            Some(w) => {
                let k = w.split(':').next().unwrap_or(w);
                if let Some(k) = k.strip_prefix("flip ") {
                    format!("breaks: off {k} gt after on strands blocks")
                } else if let Some(k) = k.strip_prefix("reflip ") {
                    format!("breaks: on again {k} gt after off strands blocks")
                } else {
                    format!("breaks: {w}")
                }
            }
        }));
    }
    Outcome { pass: why.is_empty(), why, metrics, table }
}

/// The main output: `q` when there is one, else the first.
pub(crate) fn main_output(c: &Circuit) -> usize {
    c.outputs.iter().position(|o| o.name == "q").unwrap_or(0)
}

/// The output must be off and steady over the second half of the settle time.
fn rest_off(c: &Circuit, out: &[bool]) -> Option<String> {
    let (a, b) = ((c.clock.free_from() + SETTLE / 2) as usize, start(c) as usize);
    if out[a..b.min(out.len())].iter().any(|x| *x) {
        return Some("the output is on at rest (before the first input)".into());
    }
    None
}

/// Output runs starting in frames `from..to`.
fn runs_in(out_runs: &[(usize, usize)], from: usize, to: usize) -> Vec<(usize, usize)> {
    out_runs.iter().filter(|(s, _)| *s >= from && *s < to).copied().collect()
}

/// The input pulse (start, length) that starts in a phase.
fn input_pulse(t: &Trace, i: usize, from: usize, to: usize) -> Option<(usize, usize)> {
    runs(&t.inputs[i]).into_iter().find(|(s, _)| *s >= from && *s < to)
}

fn pulse(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace) -> Outcome {
    let k = main_output(c);
    let out = t.out_seq(k);
    let mut table = Vec::new();
    if let Some(why) = rest_off(c, &out) {
        return Outcome::fail(why, table);
    }
    let orun = runs(&out);
    // per input pulse: (input length, output pulses as (delay from the rising edge, delay from the
    // falling edge, length)), and whether the output is back off at the end of the phase
    let mut rows: Vec<(usize, Vec<(i64, i64, usize)>, bool)> = Vec::new();
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let Some((on, len)) = input_pulse(t, 0, from, to) else {
            return Outcome::fail(format!("{label}: the input never turned on"), table);
        };
        let os: Vec<(i64, i64, usize)> = runs_in(&orun, from, to).into_iter().map(|(st, l)| (st as i64 - on as i64, st as i64 - (on + len) as i64, l)).collect();
        let back = !out[to - 1];
        table.push(format!(
            "in {len:>2} gt: {}{}",
            if os.is_empty() { "no output".to_string() } else { os.iter().map(|(d, _, l)| format!("+{d} for {l}")).collect::<Vec<_>>().join(", ") },
            if back { "" } else { " (still on at the end)" }
        ));
        rows.push((len, os, back));
    }
    // off at rest and back off at the end of EVERY gap (docs/circuits.md), handled input or not: a
    // short input that leaves the output on would hold it on until the next input
    if let Some((len, _, _)) = rows.iter().find(|r| !r.2) {
        let len = *len;
        return Outcome::fail(format!("latches: after a {len} gt input the output is still on at the end of its gap (the output must be back off after every input, handled or not)"), table);
    }
    let n = rows.len();
    let last = &rows[n - 1];
    // each row's verdict against the reference (the longest input, or the kind's rule)
    let lens: Vec<usize> = rows.iter().map(|r| r.0).collect();
    match kind {
        Kind::RisingEdge | Kind::FallingEdge => {
            let pick = |os: &Vec<(i64, i64, usize)>| -> Option<(i64, usize)> {
                if os.len() != 1 {
                    return None;
                }
                let (dr, df, l) = os[0];
                if *kind == Kind::RisingEdge { (dr >= 0).then_some((dr, l)) } else { (df >= 0).then_some((df, l)) }
            };
            let Some(r) = pick(&last.1) else {
                return Outcome::fail(format!("the longest input pulse ({} gt) gives {} output pulse(s): want 1 on its {} edge", last.0, last.1.len(), if *kind == Kind::RisingEdge { "rising" } else { "falling" }), table);
            };
            let ok: Vec<bool> = rows.iter().map(|(_, os, back)| *back && pick(os) == Some(r)).collect();
            finish(c, lens, ok, table, |m| vec![("L".into(), gt(r.1 as i64)), ("delay".into(), gt(r.0)), ("min-pulse".into(), gt(m as i64))])
        }
        Kind::DualEdge => {
            let pick = |os: &Vec<(i64, i64, usize)>| -> Option<(i64, usize, i64, usize)> { (os.len() == 2 && os[0].0 >= 0 && os[1].1 >= 0).then(|| (os[0].0, os[0].2, os[1].1, os[1].2)) };
            let Some(r) = pick(&last.1) else {
                return Outcome::fail(format!("the longest input pulse ({} gt) gives {} output pulse(s): want one per edge", last.0, last.1.len()), table);
            };
            let ok: Vec<bool> = rows.iter().map(|(_, os, back)| *back && pick(os) == Some(r)).collect();
            finish(c, lens, ok, table, |m| {
                let l = if r.1 == r.3 { gt(r.1 as i64) } else { format!("{} gt rising / {} gt falling", r.1, r.3) };
                let d = if r.0 == r.2 { gt(r.0) } else { format!("{} gt rising / {} gt falling", r.0, r.2) };
                vec![("L".into(), l), ("delay".into(), d), ("min-pulse".into(), gt(m as i64))]
            })
        }
        Kind::PulseLimiter => {
            if last.1.len() != 1 || last.1[0].0 < 0 {
                return Outcome::fail(format!("the longest input pulse ({} gt) gives {} output pulse(s): want 1 from its rising edge", last.0, last.1.len()), table);
            }
            let (d, l) = (last.1[0].0, last.1[0].2);
            if l >= last.0 {
                return Outcome::fail(format!("never limits: the longest input pulse ({} gt) comes out {l} gt long", last.0), table);
            }
            let ok: Vec<bool> = rows.iter().map(|(len, os, back)| *back && os.len() == 1 && os[0].0 == d && if *len >= l { os[0].2 == l } else { os[0].2 <= l }).collect();
            // limiting is witnessed by two input lengths above L coming out exactly L (one would
            // not tell a limiter from a delay line that shortens every pulse by a fixed amount)
            let above = rows.iter().zip(&ok).filter(|((len, _, _), o)| **o && *len > l).count();
            if above < 2 {
                return Outcome::fail(
                    format!("limiting not witnessed: {above} input pulse(s) longer than L ({l} gt) come out L long, want 2 (a delay line that shortens pulses looks the same; a longer gap or a `stimulus:` hint adds longer inputs)"),
                    table,
                );
            }
            finish(c, lens, ok, table, |m| vec![("L".into(), gt(l as i64)), ("delay".into(), gt(d)), ("min-pulse".into(), gt(m as i64))])
        }
        _ => {
            // a pulse extender: L is the output of the shortest inputs; a candidate L (each one-pulse
            // output, shortest input first) holds when two input lengths shorter than L come out
            // exactly L INSIDE the handled suffix (an input below the min-pulse shows no extension,
            // and one length alone would not tell an extender from a stretcher that adds a fixed
            // amount to every pulse when the stimulus has no other length between them)
            if last.1.len() != 1 || last.1[0].0 < 0 {
                return Outcome::fail(format!("the longest input pulse ({} gt) gives {} output pulse(s): want 1 from its rising edge", last.0, last.1.len()), table);
            }
            let d = last.1[0].0;
            let candidates: Vec<usize> = rows.iter().filter(|(_, os, back)| *back && os.len() == 1 && os[0].0 == d).map(|(_, os, _)| os[0].2).collect();
            if candidates.is_empty() {
                return Outcome::fail("no input pulse comes out as one pulse", table);
            }
            let verdict = |l: usize| -> (Vec<bool>, bool) {
                let ok: Vec<bool> = rows.iter().map(|(len, os, back)| *back && os.len() == 1 && os[0].0 == d && if *len < l { os[0].2 == l } else { os[0].2 >= l }).collect();
                let n = ok.len();
                let k0 = (0..=n).rev().take_while(|&k| k == n || ok[k]).last().unwrap_or(n);
                // two witnesses, or every tested length below L when fewer exist (L 2: only 1 gt)
                let below = rows.iter().filter(|r| r.0 < l).count();
                let extends = below > 0 && (k0..n).filter(|&k| rows[k].0 < l).count() >= below.min(2);
                (ok, extends)
            };
            let Some((l, ok)) = candidates.iter().find_map(|&l| {
                let (ok, extends) = verdict(l);
                extends.then_some((l, ok))
            }) else {
                let l = candidates[0];
                let (ok, _) = verdict(l);
                let mut table = table;
                for (k, line) in table.iter_mut().enumerate() {
                    if k < ok.len() && !ok[k] {
                        line.push_str("  <- not handled");
                    }
                }
                return Outcome::fail(format!("never extends: no two input pulses shorter than its output come out exactly that long among the handled inputs (from the shortest input: L {l} gt)"), table);
            };
            finish(c, lens, ok, table, |m| vec![("L".into(), gt(l as i64)), ("delay".into(), gt(d)), ("min-pulse".into(), gt(m as i64))])
        }
    }
}

/// A suffix verdict: the inputs from the shortest handled length up must all be handled (at least
/// the 3 longest; a button's presses: all).
fn finish(c: &Circuit, lens: Vec<usize>, ok: Vec<bool>, mut table: Vec<String>, metrics: impl Fn(usize) -> Vec<(String, String)>) -> Outcome {
    let n = ok.len();
    let k0 = (0..=n).rev().take_while(|&k| k == n || ok[k]).last().unwrap_or(n);
    for (k, line) in table.iter_mut().enumerate() {
        if k < ok.len() && !ok[k] {
            line.push_str("  <- not handled");
        }
    }
    let need = if c.inputs.first().is_some_and(|i| i.how == InputHow::Button) { n } else { 3.min(n) };
    if n - k0 < need {
        return Outcome::fail(format!("only the {} longest input pulse(s) are handled (want at least {need})", n - k0), table);
    }
    Outcome { pass: true, why: String::new(), metrics: metrics(lens[k0]), table }
}

fn delay(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let k = main_output(c);
    let out = t.out_seq(k);
    let mut table = Vec::new();
    if let Some(why) = rest_off(c, &out) {
        return Outcome::fail(why, table);
    }
    let orun = runs(&out);
    let ins = runs(&t.inputs[0]);
    // the reference: the longest isolated pulse
    let pulses: Vec<&(u32, u32, String)> = s.phases.iter().filter(|p| p.2.starts_with("pulse")).collect();
    let Some(refp) = pulses.last() else {
        return Outcome::fail("no pulse phase", table);
    };
    let (rf, rt) = (refp.0 as usize, (refp.1 as usize).min(out.len()));
    let Some((ron, rlen)) = input_pulse(t, 0, rf, rt) else {
        return Outcome::fail("the input never turned on", table);
    };
    let Some((oon, olen)) = runs_in(&orun, ron, rt).first().copied() else {
        return Outcome::fail(format!("the longest pulse ({rlen} gt) gives no output"), table);
    };
    let (nr, nf) = (oon as i64 - ron as i64, (oon + olen) as i64 - (ron + rlen) as i64);
    // a phase is handled when its output pulses are exactly its input pulses shifted (nr, nf)
    let handled = |from: usize, to: usize| -> (bool, String) {
        let inp: Vec<(usize, usize)> = ins.iter().filter(|(st, _)| *st >= from && *st < to).copied().collect();
        let os = runs_in(&orun, from, to + nr.max(0) as usize);
        let want: Vec<(i64, i64)> = inp.iter().map(|(st, l)| (*st as i64 + nr, (*st + *l) as i64 + nf)).collect();
        let got: Vec<(i64, i64)> = os.iter().map(|(st, l)| (*st as i64, (*st + *l) as i64)).collect();
        let first = inp.first().map(|(st, _)| *st as i64).unwrap_or(from as i64);
        let desc = if got.is_empty() { "no output".to_string() } else { got.iter().map(|(a, b)| format!("+{} for {}", a - first, b - a)).collect::<Vec<_>>().join(", ") };
        (want == got, desc)
    };
    let mut lens = Vec::new();
    let mut ok = Vec::new();
    let mut gaps: Vec<(u32, bool)> = Vec::new();
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let (h, desc) = handled(from, to);
        if let Some(l) = label.strip_prefix("pulse ") {
            lens.push(l.parse::<usize>().unwrap_or(0));
            ok.push(h);
            table.push(format!("in {l:>2} gt: {desc}{}", if h { "" } else { "  <- not handled" }));
        } else if let Some(g) = label.strip_prefix("gap ") {
            let g: u32 = g.parse().unwrap_or(0);
            gaps.push((g, h));
            table.push(format!("2 x 20 gt, gap {g:>2} gt: {desc}{}", if h { "" } else { "  <- not handled" }));
        } else if let Some(l) = label.strip_prefix("hold ") {
            table.push(format!("held {l} gt: {desc}{}", if h { "" } else { "  <- not handled" }));
            if !h {
                return Outcome::fail(format!("a lever held {l} gt does not come out shifted ({desc}): a delay line must pass a held input"), table);
            }
        }
    }
    let n = ok.len();
    let k0 = (0..=n).rev().take_while(|&k| k == n || ok[k]).last().unwrap_or(n);
    if n - k0 < 3.min(n) {
        return Outcome::fail(format!("only the {} longest pulse(s) come out shifted (want at least 3)", n - k0), table);
    }
    let mut metrics = vec![("N".to_string(), if nr == nf { gt(nr) } else { format!("{nr} gt rising / {nf} gt falling") }), ("min-pulse".into(), gt(lens[k0] as i64))];
    if !gaps.is_empty() {
        let g0 = (0..=gaps.len()).rev().take_while(|&i| i == gaps.len() || gaps[i].1).last().unwrap_or(gaps.len());
        metrics.push(("min-gap".into(), if g0 < gaps.len() { gt(gaps[g0].0 as i64) } else { "over 10 gt".into() }));
    }
    let why = if nr == nf { String::new() } else { format!("one-edge delay: the rising edge is delayed {nr} gt, the falling edge {nf} gt (pulses change length)") };
    Outcome { pass: true, why, metrics, table }
}

/// For each phase of a steady-state test: the value at its end, the frame of its last change
/// (`None`: no change), and the number of changes.
pub(crate) fn settle(out: &[bool], from: usize, to: usize) -> (bool, Option<usize>, usize) {
    let mut last = None;
    let mut changes = 0;
    for i in from.max(1)..to {
        if out[i] != out[i - 1] {
            last = Some(i);
            changes += 1;
        }
    }
    (out[to - 1], last, changes)
}

fn gate(c: &Circuit, claim: Option<&str>, s: &Schedule, t: &Trace) -> Outcome {
    if c.inputs.iter().any(|i| i.how == InputHow::Button) {
        return Outcome::fail("a gate is judged with steady inputs: use a lever or a block input, not a button", Vec::new());
    }
    let k = main_output(c);
    let out = t.out_seq(k);
    let n = c.inputs.len();
    let mut table = Vec::new();
    let mut truth: Vec<Option<bool>> = vec![None; 1 << n];
    let (mut worst, mut glitches) = (0usize, 0usize);
    let mut prev = None;
    for (w, (from, to, label)) in s.phases.iter().enumerate() {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let st = usize::from_str_radix(label.split_whitespace().next().unwrap_or("0"), 2).unwrap_or(0);
        let (fin, last, changes) = settle(&out, from, to);
        if let Some(l) = last
            && l >= from + (to - from) / 2
        {
            table.push(format!("{label}: still changing at frame {l}"));
            return Outcome::fail(format!("{label}: the output is not steady in the second half of the hold ({} gt)", to - from), table);
        }
        match truth[st] {
            Some(v) if v != fin => {
                table.push(format!("{label}: output {}", fin as u8));
                return Outcome::fail(format!("not a function of its inputs: {label} gives {} and {}", v as u8, fin as u8), table);
            }
            _ => truth[st] = Some(fin),
        }
        // latency and glitches: every transition (the rest phase only fixes the state at rest)
        if w > 0 {
            let lat = last.map(|l| l - from).unwrap_or(0);
            let needed = prev.is_some_and(|p| p != fin) as usize;
            let g = changes.saturating_sub(needed);
            worst = worst.max(if needed == 1 { lat } else { 0 });
            glitches += g;
            table.push(format!("-> {label}: output {}{}{}", fin as u8, if needed == 1 { format!(" after {lat} gt") } else { String::new() }, if g > 0 { format!(", {g} glitch change(s)") } else { String::new() }));
        } else {
            table.push(format!("{label}: output {}", fin as u8));
        }
        prev = Some(fin);
    }
    let tt: Vec<bool> = truth.iter().map(|v| v.unwrap_or(false)).collect();
    let found = if n == 1 {
        match (tt[0], tt[1]) {
            (true, false) => "not",
            (false, true) => "buffer",
            (false, false) => "false",
            (true, true) => "true",
        }
        .to_string()
    } else {
        GATES2.iter().find(|(_, t4)| t4[..] == tt[..]).map(|(name, _)| name.to_string()).unwrap_or_default()
    };
    let degenerate = ["false", "true", "a", "b", "not-a", "not-b"];
    if degenerate.contains(&found.as_str()) || (n == 1 && (found == "false" || found == "true")) {
        return Outcome::fail(format!("not a gate: the output is {found} (truth table {})", tt.iter().map(|b| if *b { '1' } else { '0' }).collect::<String>()), table);
    }
    if let Some(cl) = claim
        && cl != found
    {
        return Outcome::fail(format!("claims {cl} but behaves as {found}"), table);
    }
    let mut metrics = vec![("function".to_string(), found), ("latency".into(), gt(worst as i64))];
    if glitches > 0 {
        metrics.push(("glitches".into(), glitches.to_string()));
    }
    if worst == 0 {
        metrics.push(("class".into(), "instant".into()));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}

fn tff(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    let k = main_output(c);
    let out = t.out_seq(k);
    let mut table = Vec::new();
    // phase A and B: each pulse toggles the output exactly once; (pulse length, change offset)
    let mut offsets: Vec<(usize, Option<usize>)> = Vec::new();
    for (from, to, label) in s.phases.iter().filter(|p| p.2.starts_with('A') || p.2.starts_with('B')) {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let Some((on, len)) = input_pulse(t, 0, from, to) else {
            return Outcome::fail(format!("{label}: the input never turned on"), table);
        };
        let (_, last, changes) = settle(&out, from, to);
        let toggled = changes == 1 && out[to - 1] != out[from - 1];
        let off = if toggled { last.map(|l| l - on) } else { None };
        table.push(format!("{label}: {}", if toggled { format!("toggled +{}", off.unwrap_or(0)) } else { format!("{changes} change(s): not a toggle") }));
        let _ = len;
        offsets.push((len, off));
    }
    let a: Vec<&(usize, Option<usize>)> = offsets.iter().take(8).collect();
    if a.iter().any(|(_, o)| o.is_none()) {
        return Outcome::fail("the first 8 pulses must each toggle the output exactly once", table);
    }
    // a long pulse (a lever held) must toggle it too
    if let Some((l, _)) = offsets.iter().filter(|(l, _)| *l > a[0].0).find(|(_, o)| o.is_none()) {
        return Outcome::fail(format!("a {l} gt pulse does not toggle it exactly once (long pulses must: a lever held down)"), table);
    }
    // the edge: the offset stays when the pulse length changes (rising) or moves with it (falling)
    let a_len = a[0].0;
    let a_off = a.iter().map(|(_, o)| o.unwrap()).max().unwrap();
    let long = offsets.iter().find(|(l, o)| *l > a_len && o.is_some());
    let falling = match long {
        Some((l, o)) => o.unwrap() as i64 - a_off as i64 == *l as i64 - a_len as i64 && o.unwrap() != a_off,
        None => a_off >= a_len,
    };
    let latency = if falling { a_off - a_len } else { a_off };
    // the shortest pulse: every tested length from it up toggles
    let mut lens: Vec<usize> = offsets.iter().map(|(l, _)| *l).collect();
    lens.sort();
    lens.dedup();
    let good = |l: usize| offsets.iter().filter(|(x, _)| *x == l).all(|(_, o)| o.is_some());
    let min_pulse = lens.iter().rev().take_while(|l| good(**l)).last().copied().unwrap_or(a_len);
    // phase C: trains of 6 pulses: exactly 6 changes, steady at the end
    let mut periods: Vec<(u32, bool)> = Vec::new();
    for (from, to, label) in s.phases.iter().filter(|p| p.2.starts_with('C')) {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let pulses = runs(&t.inputs[0][from..to]).len();
        let (_, last, changes) = settle(&out, from, to);
        let steady = last.is_none_or(|l| l < to - (to - from).min(20) / 2);
        let pass = changes == pulses && steady;
        let per: u32 = label.rsplit(' ').next().and_then(|x| x.parse().ok()).unwrap_or(0);
        table.push(format!("{label}: {pulses} pulses, {changes} change(s){}", if pass { "" } else { "  <- not followed" }));
        periods.push((per, pass));
    }
    let mut metrics = vec![("latency".to_string(), gt(latency as i64)), ("edge".into(), if falling { "falling" } else { "rising" }.into()), ("min-pulse".into(), gt(min_pulse as i64))];
    if !periods.is_empty() {
        // the periods run from slow to fast: the fastest of the leading run that is followed
        metrics.push(("min-period".into(), match periods.iter().take_while(|(_, ok)| *ok).last() {
            Some((p, _)) => gt(*p as i64),
            None => format!("over {} gt", periods[0].0),
        }));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}

fn latch(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace) -> Outcome {
    if *kind == Kind::DLatch {
        // both polarities: transparent while e is on (enable high), or while it is off (a
        // repeater lock: enable low)
        let high = latch_run(c, kind, s, t, true);
        if high.pass {
            let mut o = high;
            o.metrics.push(("enable".into(), "high".into()));
            return o;
        }
        let mut low = latch_run(c, kind, s, t, false);
        if low.pass {
            low.metrics.push(("enable".into(), "low".into()));
            return low;
        }
        return Outcome::fail(format!("enable high: {}; enable low: {}", high.why, low.why), high.table);
    }
    latch_run(c, kind, s, t, true)
}

fn latch_run(c: &Circuit, kind: &Kind, s: &Schedule, t: &Trace, enable_high: bool) -> Outcome {
    let k = main_output(c);
    let out = t.out_seq(k);
    let qbar = c.outputs.iter().position(|o| o.name == "qbar").map(|i| t.out_seq(i));
    let mut table = Vec::new();
    let mut q: Option<bool> = None;
    let mut worst = 0usize;
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let want = match kind {
            Kind::RsLatch => Some(label.starts_with('s')),
            _ => {
                // `d=D e=E`: transparent while e is on (enable high) or off (enable low), else holds
                let d = label.contains("d=1");
                if label.contains("e=1") == enable_high { Some(d) } else { q }
            }
        };
        let (fin, last, _) = settle(&out, from, to);
        if let Some(l) = last
            && l >= from + (to - from) / 2
        {
            return Outcome::fail(format!("{label}: the output is not steady in the second half of the step"), table);
        }
        let lat = last.map(|l| l - from).unwrap_or(0);
        let bad = want.is_some_and(|w| w != fin) || qbar.as_ref().is_some_and(|qb| qb[to - 1] == fin);
        table.push(format!("{label}: q = {}{}{}", fin as u8, if last.is_some() { format!(" after {lat} gt") } else { String::new() }, if bad { "  <- wrong" } else { "" }));
        if bad {
            return Outcome::fail(format!("{label}: q = {}, want {}", fin as u8, want.map(|w| (w as u8).to_string()).unwrap_or("-".into())), table);
        }
        if q.is_some_and(|p| p != fin) || (q.is_none() && last.is_some()) {
            worst = worst.max(lat);
        }
        q = Some(fin);
    }
    Outcome { pass: true, why: String::new(), metrics: vec![("latency".into(), gt(worst as i64))], table }
}

fn counter(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    // the 4 gt pulses alone first (the count's shape), then with the real presses after them
    // (the real-use audit: a counter counts button presses and lever flips; the count must go on)
    let short = Schedule { phases: s.phases.iter().filter(|p| p.2.starts_with("pulse")).cloned().collect(), ..s.clone() };
    let base = counter_run(c, n, &short, t);
    if !base.pass || short.phases.len() == s.phases.len() {
        return base;
    }
    let all = counter_run(c, n, s, t);
    if all.pass && all.metrics == base.metrics {
        return all;
    }
    let mut table = all.table;
    table.push("-- the 4 gt pulses alone --".into());
    table.extend(base.table);
    Outcome::fail(format!("counts 4 gt pulses but not real presses (stone and wooden buttons, a lever held down: {} gt): {}", COUNTER_PRESSES[..3].iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", "), if all.pass { format!("they count otherwise ({:?})", all.metrics) } else { all.why }), table)
}

fn counter_run(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let k = main_output(c);
    let out = t.out_seq(k);
    let levels = &t.levels[k];
    let mut table = Vec::new();
    // per pulse: the output's value at the end of its phase, and whether it pulsed within it
    let mut toggles: Vec<usize> = Vec::new();
    let mut pulses: Vec<usize> = Vec::new();
    let mut ends: Vec<u8> = Vec::new();
    // the output (on or off) at the end of each pulse's phase
    let mut fins: Vec<bool> = Vec::new();
    let mut prev = out[(start(c) as usize).saturating_sub(1)];
    for (j, (from, to, label)) in s.phases.iter().enumerate() {
        let (from, to) = (*from as usize, (*to as usize).min(out.len()));
        let (fin, _, changes) = settle(&out, from, to);
        fins.push(fin);
        let pulsed = fin == prev && changes >= 2;
        if fin != prev {
            toggles.push(j + 1);
        }
        if pulsed {
            pulses.push(j + 1);
        }
        table.push(format!("{label}: output {} (level {}){}{}", fin as u8, levels[to - 1], if fin != prev { ", changed" } else { "" }, if pulsed { ", pulsed" } else { "" }));
        prev = fin;
        ends.push(levels[to - 1]);
    }
    let every = |v: &[usize]| v.len() >= 2 && v.windows(2).all(|w| w[1] - w[0] == n as usize);
    // a level counter: the level after each pulse repeats with period exactly N
    let period = (1..=ends.len()).find(|&p| (p..ends.len()).all(|j| ends[j] == ends[j - p])).unwrap_or(ends.len());
    let style = if !toggles.is_empty() && pulses.is_empty() && every(&toggles) {
        "toggles"
    } else if toggles.is_empty() && every(&pulses) {
        "pulses"
    } else if period == n as usize && ends.len() >= 2 * period {
        "level"
    } else {
        return Outcome::fail(format!("the output does not change once every {n} pulses (changed after pulses {toggles:?}, pulsed after {pulses:?}, levels {ends:?})"), table);
    };
    // what it counts (site QA 2026-10-21: a counter's pick gives ONE output pulse per N input
    // pulses): a pulse within every Nth input's phase, or a level with one rising edge per N inputs
    // (on after k of them), is one pulse per N; toggling every N pulses gives one pulse per 2N
    // inputs (a square wave: a divider by 2N)
    let tail: Vec<bool> = fins.len().checked_sub(n as usize).map(|from| fins[from..].to_vec()).unwrap_or_default();
    let ons = tail.iter().filter(|x| **x).count();
    let rises = (0..tail.len()).filter(|&i| tail[i] && !tail[(i + tail.len() - 1) % tail.len()]).count();
    let counts = match style {
        "pulses" => "one pulse per N".to_string(),
        "toggles" => format!("one pulse per {} inputs (toggles every {n}: a square wave, a divider by {})", 2 * n, 2 * n),
        _ if rises == 1 && ons == 1 => "one pulse per N".to_string(),
        _ if rises == 1 => format!("one pulse per N, on {ons} of {n}"),
        _ if rises == 0 => "levels only".to_string(),
        _ => format!("{rises} pulses per {n} inputs"),
    };
    Outcome { pass: true, why: String::new(), metrics: vec![("N".into(), n.to_string()), ("output".into(), format!("{style} every {n} pulses")), ("counts".into(), counts)], table }
}
