//! The vibration test (coordinator and clockinfra, 2026-10-20): a design that holds a sculk sensor
//! hears game events, and the judge's own input (a redstone block written with `setBlock`) makes
//! none. A sculk design first passes the plain run (no game events from the judge: it must work
//! on its redstone input alone, never by hearing the device), then:
//!
//! * DEVICE (a hard rule): the same schedule with the game events a lever makes at every plain
//!   input cell (`LeverBlock.pull`: `block_activate` when it turns on, `block_deactivate` when it
//!   turns off; a button makes the same two) must judge exactly as the plain run (same metrics).
//!   Skipped when the design says `input-device: far` (its input comes by a line from beyond its
//!   sensors' reach; the metric says so). Lever and button inputs are real devices: they make
//!   their own events in every run.
//! * AMBIENT (a property, `vibration-proof yes / no` and its own archive axis): the device run plus
//!   seeded pseudo-random game events every 5..15 gt at cells within the sensors' listener radius of
//!   the design's swept box (8 blocks, 16 with a calibrated sensor), never inside that box: a
//!   footstep, an item hitting the ground, a block placed, a block broken, an entity placed. `yes`
//!   when the design judges exactly as without them. A design that says `vibration: proof` is
//!   refused when it is not.
//!
//! Every event is posted at its cell's center, on redsim and in the game alike (redsim measures
//! block centers), and the generator is seeded by the build itself (the circuit as rotated and
//! placed), so redsim and the game see the same events for the same build.

use crate::check::Outcome;
use crate::stimulus::{EventAt, Schedule, TargetOp};
use crate::trace::Trace;
use crate::{Circuit, InputHow, Kind, Pos};
use anyhow::Result;

pub use clock::vibration::{AMBIENT_EVENTS, AMBIENT_REACH};

/// The schedule with its input devices' events (`block_activate` / `block_deactivate` at each edge
/// of every plain input cell), unless the design says `input-device: far`.
pub fn with_device(c: &Circuit, s: &Schedule) -> Schedule {
    let mut out = s.clone();
    if c.input_far() {
        return out;
    }
    for (i, inp) in c.inputs.iter().enumerate() {
        // a plain input: the judge's written block, or (R7) the judge's repeater line, which
        // makes no game event itself; the lever a real build would switch it with does
        if !matches!(inp.how, InputHow::Block { .. } | InputHow::Driven { .. }) {
            continue;
        }
        for &(t, on) in s.edges.get(i).map(|e| e.as_slice()).unwrap_or(&[]) {
            out.events.push((t, EventAt::Input(i), TargetOp::GameEvent(if on { "block_activate" } else { "block_deactivate" })));
        }
    }
    out.events.sort_by_key(|e| e.0);
    out
}

/// The box every cell of a run ever held a block in (the design's swept box), from its trace.
pub fn swept(t: &Trace) -> Option<(Pos, Pos)> {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for f in &t.cells {
        for (p, _) in f {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
    }
    (lo[0] <= hi[0]).then_some((lo, hi))
}

/// The schedule with ambient events added ([`clock::vibration::ambient_events`]: one every 5..15
/// gt at a cell of `cells`, seeded by the build).
pub fn with_ambient(c: &Circuit, s: &Schedule, cells: &[Pos]) -> Schedule {
    let mut out = s.clone();
    let from = c.clock.free_from() + crate::stimulus::SETTLE;
    for (t, p, ev) in clock::vibration::ambient_events(clock::vibration::seed(&c.clock.design), from, s.ticks, cells) {
        out.events.push((t, EventAt::Cell(p), TargetOp::GameEvent(ev)));
    }
    out.events.sort_by_key(|e| e.0);
    out
}

/// The device and ambient runs of a sculk design that passed its plain run (`quiet`: that run's
/// trace; `run`: the trace of a schedule on redsim or in the game): `base` with the
/// `vibration-proof` metric, or why it fails (the device rule; a false `vibration: proof` claim).
pub fn check(c: &Circuit, kind: &Kind, base: &Outcome, s: &Schedule, quiet: &Trace, mut run: impl FnMut(&Circuit, &Schedule) -> Result<std::result::Result<Trace, String>>) -> Result<Outcome> {
    let mut judge = |sv: &Schedule| -> Result<std::result::Result<Outcome, String>> {
        Ok(match run(c, sv)? {
            Ok(t) => Ok(crate::check::check(c, kind, sv, &t)),
            Err(why) => Err(why),
        })
    };
    let same = |o: &Outcome| o.pass && o.metrics == base.metrics;
    let sd = with_device(c, s);
    if !c.input_far() && c.inputs.iter().any(|i| matches!(i.how, InputHow::Block { .. } | InputHow::Driven { .. })) {
        let what = "its input's lever (block_activate / block_deactivate at the input; `input-device: far` if the input comes from beyond the sensors' reach)";
        match judge(&sd)? {
            Err(why) => return Ok(Outcome::fail(format!("hears its input device: {what}: {why}"), base.table.clone())),
            Ok(o) if !same(&o) => {
                let why = if o.pass { format!("{} where the quiet run gives {}", o.metrics_text(), base.metrics_text()) } else { o.why.clone() };
                return Ok(Outcome::fail(format!("hears its input device: {what}: {why}"), o.table));
            }
            Ok(_) => {}
        }
    }
    let cells = swept(quiet).map(|b| clock::vibration::ambient_cells(&c.clock.design, b)).unwrap_or_default();
    let proof = match judge(&with_ambient(c, &sd, &cells))? {
        Err(why) => Err(why),
        Ok(o) if same(&o) => Ok(()),
        Ok(o) => Err(if o.pass { format!("{} where the quiet run gives {}", o.metrics_text(), base.metrics_text()) } else { o.why }),
    };
    let mut out = base.clone();
    match proof {
        Ok(()) => out.metrics.push(("vibration-proof".to_string(), if c.input_far() { "yes (input from afar)".to_string() } else { "yes".to_string() })),
        Err(why) => {
            if c.claims_vibration_proof() {
                return Ok(Outcome::fail(format!("not vibration-proof (it says `vibration: proof`): ambient game events (footsteps, items, blocks within the sensors' reach): {why}"), base.table.clone()));
            }
            out.table.push(format!("  ambient game events: {why}"));
            out.metrics.push(("vibration-proof".to_string(), if c.input_far() { "no (input from afar)".to_string() } else { "no".to_string() }));
        }
    }
    Ok(out)
}
