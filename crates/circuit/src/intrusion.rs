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
/// input device, and so do the judge's driver cells (the repeater, its support and its switch), so
/// no item passes there.
pub fn intakes(c: &Circuit) -> Vec<Pos> {
    clock::intrusion::intakes(&c.clock.design, &device_cells(c))
}

/// The cells a player's input devices fill: every input cell and a driven input's repeater,
/// support and switch cells.
pub fn device_cells(c: &Circuit) -> Vec<Pos> {
    let mut v = Vec::new();
    for i in &c.inputs {
        v.push(i.pos);
        if let crate::InputHow::Driven { dir, .. } = i.how {
            let (r, s, w) = crate::driver_cells(i.pos, dir);
            v.extend([r, s, w]);
        }
    }
    v
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

/// What [`seal`] puts on an intake: glass (a full collision cube that conducts no redstone, so a
/// dust or a repeater next to it behaves as before; an item rests on top of it, out of the
/// hopper's reach).
pub const SEAL_BLOCK: &str = "minecraft:glass";

/// The design with its intakes sealed (clk-rt 2026-10-21: "seal them"): an intake cell that is
/// air gets [`SEAL_BLOCK`]; one holding a thin block (dust, a repeater, a rail) gets it on each
/// free cell around it but below (the hopper), port cells left free. Repeated until no intake is
/// left (a cell sealed around may open a path elsewhere), at most 4 rounds. The added cells.
pub fn seal(c: &Circuit) -> (mctas::design::Design, Vec<Pos>) {
    let mut d = c.clock.design.clone();
    let inputs = device_cells(c);
    let mut ports: std::collections::HashSet<Pos> = d.ports.values().copied().collect();
    ports.extend(inputs.iter().copied());
    let mut added = Vec::new();
    for _ in 0..4 {
        let at = clock::intrusion::intakes(&d, &inputs);
        if at.is_empty() {
            break;
        }
        for u in at {
            let cells: Vec<Pos> = if d.blocks.contains_key(&u) {
                [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, 0, 1], [0, 0, -1]].iter().map(|dd| [u[0] + dd[0], u[1] + dd[1], u[2] + dd[2]]).filter(|n| !d.blocks.contains_key(n) && !ports.contains(n)).collect()
            } else if ports.contains(&u) {
                Vec::new()
            } else {
                vec![u]
            };
            for n in cells {
                d.blocks.insert(n, SEAL_BLOCK.to_string());
                added.push(n);
            }
        }
    }
    (d, added)
}
