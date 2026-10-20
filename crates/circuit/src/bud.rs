//! The BUD-switch kind (study 10, docs/circuits.md#bud-switches): a design with one or more
//! `port target[.<name>]: x y z [state-a [state-b]]` cells (air in the design) and an output. The
//! judge performs five classes of block update at every target, a few times each at spaced frames,
//! with idle spans in between, and checks that each event of a class gives exactly one output pulse
//! (the class is DETECTED) or none (not detected), always with the same latency and length, that
//! nothing fires while idle and that the output is off again before the next event:
//!
//! * `place`: air -> state-a with updates (flags 3, like a player);
//! * `remove`: state-a -> air with updates;
//! * `state`: state-a <-> state-b with updates (flags 3);
//! * `shape`: the same change with shape updates only (flags 2: no block update to the neighbors,
//!   how crops grow);
//! * `update`: a pure block update around the cell (`updateNeighborsAt`), nothing changes.
//!
//! Then pairs of events at shorter and shorter gaps give the RE-ARM time (the shortest gap at
//! which both events are detected). Read VARIATIONS.md (designs/circuits/bud/) for why these five.

use crate::check::{Outcome, Sizing};
use crate::stimulus::{EventAt, SETTLE, Schedule, TargetOp};
use crate::trace::{Trace, runs};
use crate::{Circuit, Pos};
use anyhow::{Result, bail};

/// The update classes, in report order.
pub const CLASSES: [&str; 5] = ["place", "remove", "state", "shape", "update"];
/// The gaps the re-arm pairs try (gt).
pub const REARM_GAPS: [u32; 8] = [2, 4, 6, 8, 10, 15, 20, 30];
/// Events per class (each toggle of `state` / `shape` counts).
const REPEATS: u32 = 3;
/// The default target states: a log (no redstone behavior; its axis is a pure state change).
pub const DEFAULT_A: &str = "minecraft:oak_log[axis=y]";
pub const DEFAULT_B: &str = "minecraft:oak_log[axis=x]";

/// A target cell: where the judge performs its updates.
#[derive(Clone, Debug)]
pub struct Target {
    pub name: String,
    pub pos: Pos,
    /// The block placed there (`place`) and toggled from (`state`, `shape`).
    pub a: String,
    /// The other state of that block (`state`, `shape`).
    pub b: String,
}

impl Target {
    pub fn parse(d: &mctas::design::Design, name: &str, pos: Pos, args: &str) -> Result<Target> {
        if let Some(s) = d.blocks.get(&pos) {
            bail!("target at {pos:?}: the design must leave the target cell air (the judge places and removes blocks there), it holds {s}");
        }
        let w: Vec<&str> = args.split_whitespace().collect();
        let st = |s: &str| -> Result<String> { Ok(mctas::blockstate::BlockState::parse(s)?.to_string()) };
        let (a, b) = match w.as_slice() {
            [] => (DEFAULT_A.to_string(), DEFAULT_B.to_string()),
            [a, b] => (st(a)?, st(b)?),
            _ => bail!("want `port target: x y z [<state-a> <state-b>]`, got {args:?}"),
        };
        if a == b {
            bail!("target at {pos:?}: state-a and state-b are the same ({a})");
        }
        Ok(Target { name: name.to_string(), pos, a, b })
    }
}

/// The ports a BUD switch needs: one or more targets, one output, no inputs.
pub fn check_ports(c: &Circuit) -> Result<()> {
    if c.targets.is_empty() {
        bail!("bud: needs a target cell (port target: x y z [state-a state-b])");
    }
    if c.outputs.len() != 1 {
        bail!("bud: needs exactly one output (port output:), has {}", c.outputs.len());
    }
    if !c.inputs.is_empty() {
        bail!("bud: takes no input (the judge's updates at its targets are the stimulus)");
    }
    for w in &c.bud_claims {
        if !CLASSES.contains(&w.as_str()) {
            bail!("bud: claims {w:?}: not an update class ({})", CLASSES.join(", "));
        }
    }
    Ok(())
}

fn ev(s: &mut Schedule, t: u32, span: u32, i: usize, op: TargetOp, label: String) {
    s.events.push((t, EventAt::Target(i), op));
    s.phase(t, t + span, label);
}

/// The BUD stimulus: idle, then per target: place / remove x3, state x3 (both ways), shape x3, update
/// x3; idle; then the re-arm pairs. `gap`: the frames each event is given (the circuit's recovery).
pub fn stimulus(c: &Circuit, z: &Sizing) -> Schedule {
    stimulus_with(c, z, true)
}

/// [`stimulus`]; `full` false stops after the classes and the idle span (no footprint probes, no
/// re-arm pairs: the census's screen).
pub fn stimulus_with(c: &Circuit, z: &Sizing, full: bool) -> Schedule {
    let gap = z.gap.clamp(30, 200);
    let mut s = Schedule::new(0);
    let mut t = c.clock.free_from() + SETTLE;
    s.phase(t, t + 2 * gap, "idle");
    t += 2 * gap;
    for (i, tg) in c.targets.iter().enumerate() {
        let (a, b, n) = (tg.a.clone(), tg.b.clone(), tg.name.clone());
        for k in 1..=REPEATS {
            ev(&mut s, t, gap, i, TargetOp::Place(a.clone()), format!("place {k} {n}"));
            t += gap;
            ev(&mut s, t, gap, i, TargetOp::Clear, format!("remove {k} {n}"));
            t += gap;
        }
        ev(&mut s, t, gap, i, TargetOp::Place(a.clone()), format!("setup {n}"));
        t += gap;
        for k in 1..=REPEATS {
            ev(&mut s, t, gap, i, TargetOp::Place(b.clone()), format!("state {k} {n}"));
            t += gap;
            ev(&mut s, t, gap, i, TargetOp::Place(a.clone()), format!("state {k}b {n}"));
            t += gap;
        }
        for k in 1..=REPEATS {
            ev(&mut s, t, gap, i, TargetOp::Shape(b.clone()), format!("shape {k} {n}"));
            t += gap;
            ev(&mut s, t, gap, i, TargetOp::Shape(a.clone()), format!("shape {k}b {n}"));
            t += gap;
        }
        ev(&mut s, t, gap, i, TargetOp::Clear, format!("setup {n}"));
        t += gap;
        for k in 1..=REPEATS {
            ev(&mut s, t, gap, i, TargetOp::Update, format!("update {k} {n}"));
            t += gap;
        }
    }
    s.phase(t, t + 2 * gap, "idle");
    t += 2 * gap;
    if !full {
        s.ticks = t;
        return s;
    }
    // the footprint: place, remove and a pure update at every cell around the design (the cells
    // that trigger it, the targets aside); a probe that leaves the output on ends it
    let a0 = c.targets.first().map(|t| t.a.clone()).unwrap_or_else(|| DEFAULT_A.to_string());
    for k in 0..c.probe_cells().len() {
        for (op, what) in [(TargetOp::Place(a0.clone()), "place"), (TargetOp::Clear, "remove"), (TargetOp::Update, "update")] {
            s.events.push((t, EventAt::Probe(k), op));
            s.phase(t, t + gap, format!("probe {k} {what}"));
            t += gap;
        }
    }
    // re-arm pairs, last (a pair too close together may jam a design: nothing after a jam is
    // measured): two events g apart then the gap, from the longest g down, every family at each g
    for g in REARM_GAPS.iter().rev().copied() {
        for (i, tg) in c.targets.iter().enumerate() {
            let (a, b, n) = (tg.a.clone(), tg.b.clone(), tg.name.clone());
            for fam in ["update", "state", "shape", "place"] {
                let (e1, e2, setup) = match fam {
                    "place" => (TargetOp::Place(a.clone()), TargetOp::Clear, false),
                    "state" => (TargetOp::Place(b.clone()), TargetOp::Place(a.clone()), true),
                    "shape" => (TargetOp::Shape(b.clone()), TargetOp::Shape(a.clone()), true),
                    _ => (TargetOp::Update, TargetOp::Update, false),
                };
                if setup {
                    ev(&mut s, t, gap, i, TargetOp::Place(a.clone()), format!("rsetup {fam} {n}"));
                    t += gap;
                }
                s.events.push((t, EventAt::Target(i), e1));
                s.events.push((t + g, EventAt::Target(i), e2));
                s.phase(t, t + g + gap, format!("rearm {fam} {g} {n}"));
                t += g + gap;
                if setup {
                    ev(&mut s, t, gap, i, TargetOp::Clear, format!("rsetup {fam} {n}"));
                    t += gap;
                }
            }
        }
    }
    s.ticks = t;
    s
}

/// The class an event label belongs to (`place 2 t` -> place; `state 1b t` -> state).
fn class_of(label: &str) -> Option<&'static str> {
    let w = label.split_whitespace().next()?;
    CLASSES.iter().copied().find(|c| *c == w)
}

/// Judge a BUD run.
pub fn check(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    judge(&t.out_seq(0), s, (c.clock.free_from() + SETTLE / 2) as usize, c.targets.len(), &c.bud_claims)
}

/// Judge an output sequence (on per frame) against a BUD schedule: `from` is the first frame the
/// output must be off at rest, `targets` the number of target cells, `claims` the claimed classes.
/// The census screens every candidate output cell of a design with it.
pub fn judge(out: &[bool], s: &Schedule, from: usize, targets: usize, claims: &[String]) -> Outcome {
    let mut table = Vec::new();
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(out.len());
    if out[from.min(out.len())..first.min(out.len())].iter().any(|x| *x) {
        return Outcome::fail("the output is on at rest (before the first update)", table);
    }
    let all = runs(out);
    let starting = |a: u32, b: u32| -> Vec<(usize, usize)> { all.iter().copied().filter(|(st, _)| *st >= a as usize && *st < b as usize).collect() };
    // per class: (event frame, pulses starting in its window)
    let mut events: Vec<(&'static str, u32, Vec<(usize, usize)>)> = Vec::new();
    let mut rearm: Vec<(String, u32, usize, bool)> = Vec::new();
    // footprint probes: (cell index, pulses, off at the end)
    let mut probes: Vec<(usize, usize, bool)> = Vec::new();
    for (a, b, label) in &s.phases {
        let ps = starting(*a, *b);
        let end = (*b as usize).saturating_sub(1).min(out.len().saturating_sub(1));
        let w: Vec<&str> = label.split_whitespace().collect();
        // the re-arm pairs come last and are measured separately (a pair too close may jam)
        if w.first() == Some(&"rearm") {
            rearm.push((w.get(1).copied().unwrap_or("").to_string(), w.get(2).and_then(|g| g.parse().ok()).unwrap_or(0), ps.len(), !out[end]));
            continue;
        }
        if w.first() == Some(&"rsetup") {
            continue;
        }
        if w.first() == Some(&"probe") {
            probes.push((w.get(1).and_then(|k| k.parse().ok()).unwrap_or(0), ps.len(), !out[end]));
            continue;
        }
        // the output must be off again before the next event
        if out[end] {
            return Outcome::fail(format!("latches: after `{label}` (frame {a}) the output is still on at the end of its gap"), table);
        }
        match w.first().copied() {
            Some("idle") => {
                if let Some((st, _)) = ps.first() {
                    return Outcome::fail(format!("false trigger: the output turns on at frame {st} with no update (idle)"), table);
                }
            }
            Some("setup") => {}
            _ => {
                if let Some(cl) = class_of(label) {
                    let desc = if ps.is_empty() { "no output".to_string() } else { ps.iter().map(|(st, l)| format!("+{} for {l}", *st as i64 - *a as i64)).collect::<Vec<_>>().join(", ") };
                    table.push(format!("  {label}: {desc}"));
                    events.push((cl, *a, ps));
                }
            }
        }
    }
    // per class: always one pulse (detected) or never (not), with one latency and one length
    let mut detected: Vec<&str> = Vec::new();
    let mut lat: Vec<(&str, i64)> = Vec::new();
    let mut len: Vec<(&str, usize)> = Vec::new();
    for cl in CLASSES {
        let es: Vec<&(&str, u32, Vec<(usize, usize)>)> = events.iter().filter(|e| e.0 == cl).collect();
        if es.is_empty() {
            continue;
        }
        let n1 = es.iter().filter(|e| e.2.len() == 1).count();
        let n0 = es.iter().filter(|e| e.2.is_empty()).count();
        if n0 == es.len() {
            continue;
        }
        if n1 != es.len() {
            return Outcome::fail(format!("unreliable: {cl}: {n1} of {} updates give one pulse, {n0} none, the others several", es.len()), table);
        }
        let ls: Vec<i64> = es.iter().map(|e| e.2[0].0 as i64 - e.1 as i64).collect();
        let ns: Vec<usize> = es.iter().map(|e| e.2[0].1).collect();
        if ls.iter().any(|l| *l != ls[0]) || ns.iter().any(|x| *x != ns[0]) {
            return Outcome::fail(format!("unsteady: {cl}: latencies {ls:?}, pulse lengths {ns:?}"), table);
        }
        detected.push(cl);
        lat.push((cl, ls[0]));
        len.push((cl, ns[0]));
    }
    if detected.is_empty() {
        return Outcome::fail("detects nothing: no update class gives an output pulse", table);
    }
    for w in claims {
        if !detected.contains(&w.as_str()) {
            return Outcome::fail(format!("claims {w} but does not detect it (detects {})", detected.join(" ")), table);
        }
    }
    // re-arm: per family, from the longest gap down, the last gap at which both events give their
    // own pulse and the output is off afterwards; the first failing pair ends that family (a pair
    // that leaves the output on JAMS the design: nothing after it is measured)
    let fam_ok = |f: &str| -> bool {
        match f {
            "place" => detected.contains(&"place") && detected.contains(&"remove"),
            x => detected.contains(&x),
        }
    };
    let mut re: Vec<(String, String)> = Vec::new();
    // in stimulus order: once any pair leaves the output stuck on, nothing after it is measured
    let mut jam: Option<(String, u32)> = None;
    let mut state: Vec<(String, Option<u32>, bool)> = Vec::new(); // fam, best, stopped
    for (fam, g, n, off) in &rearm {
        if jam.is_some() {
            break;
        }
        if !*off {
            jam = Some((fam.clone(), *g));
            if let Some(s) = state.iter_mut().find(|s| s.0 == *fam) {
                s.2 = true;
            }
            break;
        }
        if !fam_ok(fam) {
            continue;
        }
        let k = match state.iter().position(|s| s.0 == *fam) {
            Some(k) => k,
            None => {
                state.push((fam.clone(), None, false));
                state.len() - 1
            }
        };
        if !state[k].2 {
            if *n == 2 {
                state[k].1 = Some(*g);
            } else {
                state[k].2 = true;
            }
        }
    }
    for fam in ["update", "state", "shape", "place"] {
        if !fam_ok(fam) {
            continue;
        }
        let pairs: Vec<&(String, u32, usize, bool)> = rearm.iter().filter(|(f, _, _, _)| f == fam).collect();
        table.push(format!("  rearm {fam}: {}", pairs.iter().map(|(_, g, n, off)| format!("{g} gt: {n} pulse(s){}", if *off { "" } else { " (stuck on)" })).collect::<Vec<_>>().join(", ")));
        let v = match state.iter().find(|s| s.0 == fam) {
            Some((_, Some(g), _)) => format!("{g} gt"),
            Some((_, None, true)) => format!("> {} gt", REARM_GAPS[REARM_GAPS.len() - 1]),
            _ => "not measured (an earlier pair jammed it)".into(),
        };
        re.push((fam.to_string(), v));
    }
    // per class when they differ, `2 gt place / 6 gt remove` (`, ` separates the metrics)
    let joined = |v: Vec<(String, String)>| -> String {
        if v.iter().all(|(_, x)| *x == v[0].1) {
            v[0].1.clone()
        } else {
            v.iter().map(|(k, x)| format!("{x} {k}")).collect::<Vec<_>>().join(" / ")
        }
    };
    let lat_s = joined(lat.iter().map(|(k, l)| (k.to_string(), format!("{l} gt"))).collect());
    let len_s = joined(len.iter().map(|(k, l)| (k.to_string(), format!("{l} gt"))).collect());
    let re_s = if re.is_empty() { String::new() } else { joined(re.clone()) };
    let mut metrics = vec![("detects".to_string(), detected.join(" ")), ("latency".to_string(), lat_s), ("pulse".to_string(), len_s)];
    if !re.is_empty() {
        metrics.push(("re-arm".to_string(), re_s));
    }
    if let Some((fam, g)) = jam {
        // two updates this close leave it stuck on (it needs a block update to recover)
        metrics.push(("jams".to_string(), format!("{fam} pair {g} gt apart")));
    }
    // footprint: the targets plus every cell around the design where a place, a removal or a pure
    // update gives a pulse; counting stops at a probe that leaves the output on
    let mut hit: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    let mut stuck = false;
    for (k, n, off) in &probes {
        if *n > 0 {
            hit.insert(*k);
        }
        if !*off {
            stuck = true;
            break;
        }
    }
    if !probes.is_empty() {
        let cells = targets + hit.len();
        table.push(format!("  footprint: {targets} target(s) + probe cells {:?} of {}{}", hit, probes.len() / 3, if stuck { " (a probe jammed it)" } else { "" }));
        metrics.push(("footprint".to_string(), format!("{}{cells} cell{}", if stuck { ">= " } else { "" }, if cells == 1 { "" } else { "s" })));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}
