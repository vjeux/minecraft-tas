//! ITEM INTRUSION (the real-use audit, coordinator 2026-10-21 after vjeux 15:24Z on the precise
//! drop's lever variant: "Having exposed hoppers like this is really bad as it's very likely an
//! item gets dropped, messed up the timing and the whole machine explodes with tnt"). A hopper
//! takes every item entity that comes to rest in the cell above it (its pickup box covers the
//! whole cell), so a hopper whose top is open is an intake for any stray item: a mob drop, a
//! crater's dirt thrown by a TNT, a player's dropped stack. A design whose timing runs on items
//! must not care: the judge drops items onto every open intake at several moments of the run
//! (one cobblestone, a stack of 64 iron ingots, an unstackable shovel), and the run must pass with
//! the same metrics as without them. A sealed hopper (a full block or a container over it, or no
//! way for an item to get there from outside the design) is no intake; item-free timing has none.
//! The intakes themselves: [`clock::intrusion::intakes`] (the clock judge can use them too).

use crate::stimulus::{EventAt, Schedule, TargetOp};
use crate::{Circuit, Pos};

/// The stacks dropped at each intake, one per moment: a single item, a full stack, an
/// unstackable item (all in redsim's item registry).
pub const DROPS: [&str; 3] = ["{id:\"minecraft:cobblestone\",count:1}", "{id:\"minecraft:iron_ingot\",count:64}", "{id:\"minecraft:iron_shovel\",count:1}"];

/// The intakes of a circuit ([`clock::intrusion::intakes`]): its input cells hold the player's
/// input device, so no item passes there.
pub fn intakes(c: &Circuit) -> Vec<Pos> {
    let inputs: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    clock::intrusion::intakes(&c.clock.design, &inputs)
}

/// The run with items dropped: `s` plus, at a quarter, half and three quarters of the way from
/// its first phase to its end, each of [`DROPS`] at every intake (3 gt apart from one intake to
/// the next), each at the first frame from then on when the base run has that cell empty for 4
/// frames (a moving block sweeps some intakes: a stray item comes to rest in an empty cell).
/// `None` when the design has no intake (or the real-use audit is off).
pub fn schedule(c: &Circuit, s: &Schedule, base: &crate::trace::Trace) -> Option<Schedule> {
    let at = intakes(c);
    if at.is_empty() || !crate::check::real_use() {
        return None;
    }
    let a = s.phases.iter().map(|p| p.0).min().unwrap_or(0);
    let b = s.ticks;
    if b <= a + 40 {
        return None;
    }
    let empty = |p: &Pos, f: usize| base.cells.get(f).is_some_and(|cells| cells.binary_search_by(|(q, _)| q.cmp(p)).is_err());
    let mut s2 = s.clone();
    for (k, item) in DROPS.iter().enumerate() {
        let f = a + (b - a) * (k as u32 + 1) / 4;
        for (i, p) in at.iter().enumerate() {
            let want = (f + 3 * i as u32) as usize;
            let Some(g) = (want..base.cells.len().saturating_sub(4)).find(|&g| (g..g + 4).all(|h| empty(p, h))) else { continue };
            s2.events.push((g as u32, EventAt::Cell(*p), TargetOp::Drop(item.to_string())));
        }
    }
    s2.events.sort_by_key(|e| e.0);
    Some(s2)
}

/// The words of the drops for a verdict: where and when.
pub fn describe(c: &Circuit, s: &Schedule) -> String {
    let at = intakes(c);
    let a = s.phases.iter().map(|p| p.0).min().unwrap_or(0);
    let frames: Vec<String> = (1..=3).map(|k| (a + (s.ticks - a) * k / 4).to_string()).collect();
    format!("{} dropped on the open hopper top(s) at {} in frames {}", "1 cobblestone, 64 iron ingots, 1 iron shovel", at.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(" "), frames.join(", "))
}

/// The verdict of the intrusion run against the base run's outcome: `None` when it passes with
/// the same metrics (and keeps a precise flag), else why not.
pub fn judge(base: &crate::check::Outcome, with: &crate::check::Outcome) -> Option<String> {
    if !with.pass {
        return Some(with.why.clone());
    }
    if with.metrics != base.metrics {
        let m = |o: &crate::check::Outcome| o.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ");
        return Some(format!("its timing changes: {} (without the items: {})", m(with), m(base)));
    }
    if base.why.contains("precise:") && !with.why.contains("precise:") {
        return Some("it is no longer precise".into());
    }
    None
}
