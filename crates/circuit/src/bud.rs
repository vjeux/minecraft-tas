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
    // a tiled run ([`tile_check`]): one target and one output per copy
    let copies = c.clock.design.meta.get("tiled").and_then(|n| n.parse::<usize>().ok()).unwrap_or(1);
    if c.outputs.len() != copies || c.targets.len() % copies != 0 {
        bail!("bud: needs exactly one output (port output:), has {}", c.outputs.len());
    }
    if !c.inputs.is_empty() && !latched(c) {
        bail!("bud: takes no input (the judge's updates at its targets are the stimulus), or one reset input (port input.reset:) for a latched design");
    }
    if c.inputs.len() > copies {
        bail!("bud: one reset input per copy, has {}", c.inputs.len());
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
/// re-arm pairs: the census's screen). A LATCHED design (a `port input.reset:`) gets a reset pulse
/// [`HOLD`]-ish after every event it may answer (half the gap), and a relatch test (a second event
/// while it holds).
pub fn stimulus_with(c: &Circuit, z: &Sizing, full: bool) -> Schedule {
    let gap = z.gap.clamp(30, 200);
    let latched = latched(c);
    let hold = gap / 2;
    let mut s = Schedule::new(c.inputs.len());
    // the reset after an event at `at` (latched designs only)
    let rst = |s: &mut Schedule, at: u32| {
        // every copy's reset at once (a tiled run: the copies that did not latch stay off)
        for i in 0..s.edges.len() {
            if latched {
                s.pulse(i, at + hold, RESET_LEN);
            }
        }
    };
    let mut t = c.clock.free_from() + SETTLE;
    if latched {
        // a latched design may latch as it is built (the placement updates every block): one reset
        // right after, before the rest window
        for i in 0..s.edges.len() {
            s.pulse(i, c.clock.free_from() + 4, RESET_LEN);
        }
    }
    s.phase(t, t + 2 * gap, "idle");
    t += 2 * gap;
    for (i, tg) in c.targets.iter().enumerate() {
        let (a, b, n) = (tg.a.clone(), tg.b.clone(), tg.name.clone());
        let mut evr = |s: &mut Schedule, t: &mut u32, op: TargetOp, label: String| {
            ev(s, *t, gap, i, op, label);
            rst(s, *t);
            *t += gap;
        };
        for k in 1..=REPEATS {
            evr(&mut s, &mut t, TargetOp::Place(a.clone()), format!("place {k} {n}"));
            evr(&mut s, &mut t, TargetOp::Clear, format!("remove {k} {n}"));
        }
        evr(&mut s, &mut t, TargetOp::Place(a.clone()), format!("setup {n}"));
        for k in 1..=REPEATS {
            evr(&mut s, &mut t, TargetOp::Place(b.clone()), format!("state {k} {n}"));
            evr(&mut s, &mut t, TargetOp::Place(a.clone()), format!("state {k}b {n}"));
        }
        for k in 1..=REPEATS {
            evr(&mut s, &mut t, TargetOp::Shape(b.clone()), format!("shape {k} {n}"));
            evr(&mut s, &mut t, TargetOp::Shape(a.clone()), format!("shape {k}b {n}"));
        }
        evr(&mut s, &mut t, TargetOp::Clear, format!("setup {n}"));
        for k in 1..=REPEATS {
            evr(&mut s, &mut t, TargetOp::Update, format!("update {k} {n}"));
        }
        if latched {
            // a latch holds through a second event (a toggle would let go): a placement, then its
            // removal while it holds, then the reset; counted as a placement
            s.events.push((t, EventAt::Target(i), TargetOp::Place(a.clone())));
            s.events.push((t + hold / 2, EventAt::Target(i), TargetOp::Clear));
            s.phase(t, t + gap, format!("relatch {n}"));
            rst(&mut s, t);
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
            rst(&mut s, t);
            t += gap;
        }
    }
    // re-arm pairs, last (a pair too close together may jam a design: nothing after a jam is
    // measured): two events g apart then the gap, from the longest g down, every family at each g
    // (a latched design: g after the end of the reset that follows the first event)
    let step = |g: u32| if latched { hold + RESET_LEN + g } else { g };
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
                    rst(&mut s, t);
                    t += gap;
                }
                let d = step(g);
                let mut at = vec![t, t + d];
                s.events.push((t, EventAt::Target(i), e1));
                s.events.push((t + d, EventAt::Target(i), e2));
                if fam == "place" {
                    // place, remove, place: a design that sees only one of them (place-only,
                    // remove-only) is measured too (the next placement after a removal)
                    s.events.push((t + 2 * d, EventAt::Target(i), TargetOp::Place(a.clone())));
                    at.push(t + 2 * d);
                }
                for x in &at {
                    rst(&mut s, *x);
                }
                let last = *at.last().unwrap();
                s.phase(t, last + gap, format!("rearm {fam} {g} {n}"));
                t = last + gap;
                if setup || fam == "place" {
                    ev(&mut s, t, gap, i, TargetOp::Clear, format!("rsetup {fam} {n}"));
                    rst(&mut s, t);
                    t += gap;
                }
            }
        }
    }
    s.ticks = t;
    s
}

/// The reset pulse a latched design gets (gt).
pub const RESET_LEN: u32 = 4;

/// Whether a BUD design is latched: its output holds until the judge pulses its reset input (a
/// tiled run: one reset per copy, `reset.t<k>`).
pub fn latched(c: &Circuit) -> bool {
    !c.inputs.is_empty() && c.inputs.iter().all(|i| i.name == "r" || i.name.starts_with("reset.t"))
}

/// A design's `tile:` claim (study 10; the header and the copies are the shulker study's,
/// [`crate::shulker::tiled`]): the copies side by side, every update class performed at every
/// copy's target (the classes part of the stimulus, `gap` frames per event). Each copy's output
/// must answer its own target exactly as the single design does (`base`: its detected classes,
/// latency and pulse) and stay silent for the other copies' (no cross-talk). `run` records the
/// copies' trace (redsim or the game). Returns `base` with a `tileable` metric (`label`: the
/// step as the design writes it, the same in every build), or why not.
pub fn tile_check(c: &Circuit, base: &Outcome, gap: u32, label: &str, run: impl FnOnce(&Circuit, &Schedule) -> Result<std::result::Result<Trace, String>>) -> Result<Outcome> {
    let tc = crate::shulker::tiled(c)?;
    let s = stimulus_with(&tc, &Sizing { gap }, false);
    let t = match run(&tc, &s)? {
        Ok(t) => t,
        Err(why) => return Ok(Outcome::fail(format!("not tileable: the copies' run: {why}"), Vec::new())),
    };
    let copy_of_target: Vec<u32> = tc.targets.iter().map(|tg| crate::shulker::group_of(&tg.name)).collect();
    let metric = |o: &Outcome, k: &str| o.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
    let from = (tc.clock.free_from() + SETTLE / 2) as usize;
    for (oi, o) in tc.outputs.iter().enumerate() {
        let k = crate::shulker::group_of(&o.name);
        // this copy's events keep their labels, the other copies' must leave its output alone
        let mut sk = s.clone();
        for (_, _, label) in sk.phases.iter_mut() {
            let last = label.split_whitespace().last().unwrap_or("").to_string();
            if label.starts_with("idle") {
                continue;
            }
            if let Some(ti) = tc.targets.iter().position(|tg| tg.name == last)
                && copy_of_target[ti] != k
            {
                *label = format!("xtalk {label}");
            }
        }
        let ok = judge_with(&t.out_seq(oi), &sk, from, 1, &[], None, latched(&tc));
        if !ok.pass {
            return Ok(Outcome::fail(format!("not tileable ({label}): copy {k}: {}", ok.why), ok.table));
        }
        for m in ["detects", "latency", "pulse", "reset"] {
            if metric(&ok, m) != metric(base, m) {
                return Ok(Outcome::fail(format!("not tileable ({label}): copy {k} {m} {} where the single design has {}", metric(&ok, m), metric(base, m)), ok.table));
            }
        }
    }
    let mut out = base.clone();
    out.metrics.push(("tileable".to_string(), label.to_string()));
    Ok(out)
}

/// The frames each event is given in a BUD schedule (its first class phase).
pub fn gap_of(s: &Schedule) -> u32 {
    s.phases.iter().find(|p| class_of(&p.2).is_some()).map(|p| p.1 - p.0).unwrap_or(30)
}

/// A `tile:` step as the metric reads it: `every N (dx dy dz)`, N its length.
pub fn tile_label(step: Pos) -> String {
    format!("every {} ({} {} {})", step[0].abs() + step[1].abs() + step[2].abs(), step[0], step[1], step[2])
}

/// The class an event label belongs to (`place 2 t` -> place; `state 1b t` -> state).
fn class_of(label: &str) -> Option<&'static str> {
    let w = label.split_whitespace().next()?;
    CLASSES.iter().copied().find(|c| *c == w)
}

/// Judge a BUD run.
pub fn check(c: &Circuit, s: &Schedule, t: &Trace) -> Outcome {
    // the footprint probes must leave the design as they found it (every cell as just before the
    // probe once its block is gone): a design that carries a probe block away or is broken by it
    // cannot be measured after that (the later probes, the re-arm pairs)
    let mut broke: Option<usize> = None;
    let mut before: Option<usize> = None;
    for (a, b, label) in &s.phases {
        let w: Vec<&str> = label.split_whitespace().collect();
        if w.first() != Some(&"probe") {
            continue;
        }
        match w.get(2).copied() {
            Some("place") => before = Some((*a as usize).saturating_sub(1)),
            Some("update") => {
                let end = (*b as usize).saturating_sub(1);
                if let (Some(f0), Some(f1)) = (before.and_then(|f| t.cells.get(f)), t.cells.get(end))
                    && f0 != f1
                {
                    broke = w.get(1).and_then(|k| k.parse().ok());
                    break;
                }
            }
            _ => {}
        }
    }
    judge_with(&t.out_seq(0), s, (c.clock.free_from() + SETTLE / 2) as usize, c.targets.len(), &c.bud_claims, broke, latched(c))
}

/// Judge an output sequence (on per frame) against a BUD schedule: `from` is the first frame the
/// output must be off at rest, `targets` the number of target cells, `claims` the claimed classes.
/// The census screens every candidate output cell of a design with it.
pub fn judge(out: &[bool], s: &Schedule, from: usize, targets: usize, claims: &[String]) -> Outcome {
    judge_with(out, s, from, targets, claims, None, false)
}

/// [`judge`]; `broke`: the footprint probe after which the design is no longer as it was (the
/// later probes and the re-arm pairs are not measured); `latched`: the output must hold until the
/// reset that follows each event (half a gap later) and go off after it.
fn judge_with(out: &[bool], s: &Schedule, from: usize, targets: usize, claims: &[String], broke: Option<usize>, latched: bool) -> Outcome {
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
    // latched designs: the relatch windows (a placement, its removal while it holds, the reset)
    let mut relatch: Vec<(u32, Vec<(usize, usize)>)> = Vec::new();
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
            Some("xtalk") => {
                // a tiled run: another copy's event must leave this copy's output alone
                if let Some((st, _)) = ps.first() {
                    return Outcome::fail(format!("cross-talk: this copy's output turns on at frame {st} during `{}` (another copy's target)", w[1..].join(" ")), table);
                }
            }
            Some("relatch") => {
                table.push(format!("  {label}: {}", if ps.is_empty() { "no output".to_string() } else { ps.iter().map(|(st, l)| format!("+{} for {l}", *st as i64 - *a as i64)).collect::<Vec<_>>().join(", ") }));
                relatch.push((*a, ps));
            }
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
            "place" => detected.contains(&"place") || detected.contains(&"remove"),
            x => detected.contains(&x),
        }
    };
    // pulses a re-arm sequence gives when every event in it is seen: place, remove, place for the
    // place family, two events for the others
    let want = |f: &str| -> usize {
        match f {
            "place" => 2 * usize::from(detected.contains(&"place")) + usize::from(detected.contains(&"remove")),
            _ => 2,
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
            if *n == want(fam) {
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
    if latched {
        // the output must hold until the reset (half a gap after the event) and go off after it:
        // `reset` is how long after the reset pulse starts
        let hold = s.phases.iter().find(|p| class_of(&p.2).is_some()).map(|p| (p.1 - p.0) / 2).unwrap_or(0) as i64;
        let mut off: Vec<(String, String)> = Vec::new();
        for ((cl, l), (_, n)) in lat.iter().zip(&len) {
            let end = *l + *n as i64;
            if end <= hold {
                return Outcome::fail(format!("not latched: after a {cl} the output goes off {} gt before the reset", hold - end), table);
            }
            off.push((cl.to_string(), format!("{} gt", end - hold)));
        }
        metrics.retain(|(k, _)| k != "pulse");
        metrics.push(("reset".to_string(), format!("manual, off {} after it", joined(off))));
        // a latch holds through a removal while it is on (a toggle lets go)
        if detected.contains(&"place") {
            for (a, ps) in &relatch {
                let held = ps.len() == 1 && (ps[0].0 + ps[0].1) as i64 > *a as i64 + hold;
                if !held {
                    return Outcome::fail(format!("not a latch: a placement then its removal while the output holds (frame {a}) gives {} pulse(s) before the reset: it toggles or lets go", ps.len()), table);
                }
            }
        }
    }
    if let Some(k) = broke {
        // the re-arm pairs ran on a changed design
        table.push(format!("  footprint probe {k} changed the design (a block placed beside it and removed leaves it different): the later probes and the re-arm pairs are not measured"));
        metrics.push(("fragile".to_string(), "(a block placed beside it changes it)".to_string()));
    } else {
        if !re.is_empty() {
            metrics.push(("re-arm".to_string(), re_s));
        }
        if let Some((fam, g)) = jam {
            // two updates this close leave it stuck on (it needs a block update to recover)
            metrics.push(("jams".to_string(), format!("{fam} pair {g} gt apart")));
        }
    }
    // footprint: the targets plus every cell around the design where a place, a removal or a pure
    // update gives a pulse; counting stops at a probe that leaves the output on
    let mut hit: std::collections::BTreeSet<usize> = std::collections::BTreeSet::new();
    let mut stuck = false;
    for (k, n, off) in &probes {
        // a probe after the one that changed the design measures another design
        if broke.is_some_and(|b| *k > b) {
            stuck = true;
            break;
        }
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
        let why = if broke.is_some() { " (a probe changed it)" } else if stuck { " (a probe jammed it)" } else { "" };
        table.push(format!("  footprint: {targets} target(s) + probe cells {:?} of {}{why}", hit, probes.len() / 3));
        metrics.push(("footprint".to_string(), format!("{}{cells} cell{}", if stuck { ">= " } else { "" }, if cells == 1 { "" } else { "s" })));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}
