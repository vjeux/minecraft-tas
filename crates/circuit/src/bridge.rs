//! The RETRACTABLE PISTON BRIDGE (track pext, coordinator 2026-10-21; `circuit: piston-bridge N`):
//! a lever extends a walkable bridge across a gap of N cells and takes it fully out of the gap
//! again.
//!
//! The geometry: `port gap: x y z` is the first walkway cell (next to the near bank), `travel:
//! dx 0 dz` the span (from the near bank across the gap), `width: W` (default 1) how many cells
//! wide the walkway is, toward the span's right-hand side (travel turned a quarter turn clockwise
//! seen from above). The WALKWAY is the N x W cells from the gap port along the span; the BANKS
//! are the cell before it and the cell after it in every column (design blocks: the ground a
//! player stands on at each end); the PASSAGE is the two cells above every walkway and bank cell
//! (where a walker's body goes); the PIT is every cell under the walkway (a real gap: nothing of
//! the design may stand under it, at any depth).
//!
//! The rules (real use: a player walks across when the bridge is out, and the gap is a gap when it
//! is in):
//! - at rest (as built, and after every off edge): the walkway, the passage and the pit empty, every
//!   bank a block a player stands on (a full collision cube: `redsim::behavior::full_cube`);
//! - extended: from some frame after the on edge until the off edge, nothing moving, every walkway
//!   cell a block a player stands on (a walker crossing from bank to bank on one level, no jump),
//!   the banks still such blocks, the passage and the pit empty;
//! - after the off edge the whole rest state comes back exactly and stays; three cycles alike; then
//!   the real-use audit's lever flips (`check::LEVER_FLIPS`, as for a piston extender): the metric
//!   `flips` (safe, or the first flip that strands blocks).

use crate::check::Outcome;
use crate::stimulus::Schedule;
use crate::trace::{Trace, runs};
use crate::{Circuit, InputHow, Kind, Pos};
use anyhow::{Result, bail};
use std::collections::BTreeSet;

/// The reserved cells of a bridge.
#[derive(Clone, Debug)]
pub struct Geometry {
    pub n: u32,
    pub width: u32,
    pub walkway: Vec<Pos>,
    pub banks: Vec<Pos>,
    pub passage: Vec<Pos>,
    /// The pit's columns (x, z): every cell below `floor` there.
    pub pit: BTreeSet<(i32, i32)>,
    /// The walkway's level.
    pub floor: i32,
}

impl Geometry {
    /// Whether `p` lies in the pit.
    pub fn in_pit(&self, p: Pos) -> bool {
        p[1] < self.floor && self.pit.contains(&(p[0], p[2]))
    }
}

fn add(a: Pos, b: Pos, k: i32) -> Pos {
    [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]]
}

/// The bridge's reserved cells (see the module doc).
pub fn geometry(c: &Circuit) -> Result<Geometry> {
    let Some(Kind::PistonBridge(n)) = c.kind else { bail!("not a piston bridge") };
    let Some(gap) = c.clock.design.ports.get("gap").copied() else {
        bail!("piston-bridge: needs `port gap: x y z` (the first walkway cell, next to the near bank)");
    };
    let Some(span) = c.travel else {
        bail!("piston-bridge: needs `travel: dx 0 dz` (the span, from the near bank across the gap)");
    };
    if span[1] != 0 {
        bail!("piston-bridge: travel {span:?}: the span is horizontal");
    }
    let width = match c.clock.design.meta.get("width") {
        Some(w) => w.trim().parse::<u32>().map_err(|_| anyhow::anyhow!("width: {w:?}: a number of cells"))?,
        None => 1,
    };
    if width == 0 {
        bail!("width: 0: a walkway is at least 1 cell wide");
    }
    let across = clock::rotate_pos(span, 1);
    let mut g = Geometry { n, width, walkway: Vec::new(), banks: Vec::new(), passage: Vec::new(), pit: BTreeSet::new(), floor: gap[1] };
    for j in 0..width as i32 {
        let col = add(gap, across, j);
        for i in 0..n as i32 {
            let p = add(col, span, i);
            g.walkway.push(p);
            g.pit.insert((p[0], p[2]));
        }
        g.banks.push(add(col, span, -1));
        g.banks.push(add(col, span, n as i32));
    }
    for p in g.walkway.iter().chain(g.banks.iter()) {
        g.passage.push([p[0], p[1] + 1, p[2]]);
        g.passage.push([p[0], p[1] + 2, p[2]]);
    }
    Ok(g)
}

/// The block-state part of a trace cell's name (without contents or strengths).
fn state_of(name: &str) -> &str {
    name.split([' ', '{']).next().unwrap_or(name)
}

/// A block a player stands on: a full collision cube (slime counts; honey, soul sand, slabs and
/// moving or extended pistons do not).
pub fn walkable(state: &str) -> bool {
    redsim::Block::parse(state_of(state)).map(|b| redsim::behavior::full_cube(&b)).unwrap_or(false)
}

/// The static checks: the geometry, a lever, banks that are full cubes, nothing of the design in
/// the walkway, the passage or the pit as built.
pub fn check_ports(c: &Circuit) -> Result<()> {
    let g = geometry(c)?;
    if c.inputs.iter().any(|i| i.how != InputHow::Lever) {
        bail!("piston-bridge: its input is the design's lever (`port input: x y z lever`)");
    }
    let blocks = &c.clock.design.blocks;
    for p in &g.banks {
        match blocks.get(p) {
            Some(s) if walkable(s) => {}
            Some(s) => bail!("piston-bridge: the bank at {p:?} holds {}, not a block a player stands on (a full cube)", clock::short(s)),
            None => bail!("piston-bridge: no bank at {p:?}: the design's ground at each end of the walkway (a full cube)"),
        }
    }
    let inputs: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    for p in g.walkway.iter().chain(g.passage.iter()) {
        if blocks.contains_key(p) || inputs.contains(p) {
            bail!("piston-bridge: {p:?} is in the walkway or the passage above it: empty as built (the bridge is in at rest)");
        }
    }
    if let Some(p) = blocks.keys().chain(inputs.iter()).find(|p| g.in_pit(**p)) {
        bail!("piston-bridge: {p:?} is under the walkway: the gap is a gap, nothing of the design stands in its pit");
    }
    Ok(())
}

/// The judge (see the module doc).
pub fn check(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    let g = match geometry(c) {
        Ok(g) => g,
        Err(e) => return Outcome::fail(format!("{e:#}"), table),
    };
    if let Some(e) = &t.ended {
        return Outcome::fail(format!("the run ended early: {e}"), vec![e.clone()]);
    }
    let rest_at = (c.clock.free_from() + crate::stimulus::SETTLE) as usize - 1;
    if rest_at >= t.frames() {
        return Outcome::fail("the run is shorter than its settle time", table);
    }
    let rest = &t.cells[rest_at];
    let moving = |f: usize| t.cells[f].iter().any(|(_, id)| t.names[*id as usize].starts_with("minecraft:moving_piston"));
    let solid = |f: usize, p: Pos| t.state(f, p).is_some_and(walkable);
    let passage_clear = |f: usize| g.passage.iter().find(|p| t.state(f, **p).is_some()).copied();
    let pit_clear = |f: usize| t.cells[f].iter().find(|(p, _)| g.in_pit(*p)).map(|(p, _)| *p);
    let banks_ok = |f: usize| g.banks.iter().find(|p| !solid(f, **p)).copied();
    // why a frame is not the extended bridge (`None`: it is)
    let not_extended = |f: usize| -> Option<String> {
        if moving(f) {
            return Some("something is still moving".into());
        }
        if let Some(p) = g.walkway.iter().find(|p| !solid(f, **p)) {
            return Some(format!("the walkway cell {p:?} holds {}", t.state(f, *p).map(clock::short).unwrap_or("air")));
        }
        if let Some(p) = banks_ok(f) {
            return Some(format!("the bank {p:?} holds {}", t.state(f, p).map(clock::short).unwrap_or("air")));
        }
        if let Some(p) = passage_clear(f) {
            return Some(format!("the passage cell {p:?} holds {}", t.state(f, p).map(clock::short).unwrap_or("?")));
        }
        if let Some(p) = pit_clear(f) {
            return Some(format!("the pit cell {p:?} holds {}", t.state(f, p).map(clock::short).unwrap_or("?")));
        }
        None
    };
    let extended = |f: usize| not_extended(f).is_none();
    // the rest state: the gap clear (walkway, passage and pit empty), the banks standing
    if let Some(p) = g.walkway.iter().find(|p| t.state(rest_at, **p).is_some()) {
        return Outcome::fail(format!("at rest the walkway cell {p:?} holds {}: the bridge is in at rest, the gap clear", t.state(rest_at, *p).map(clock::short).unwrap_or("?")), table);
    }
    if let Some(p) = passage_clear(rest_at) {
        return Outcome::fail(format!("at rest the passage cell {p:?} is not empty"), table);
    }
    if let Some(p) = pit_clear(rest_at) {
        return Outcome::fail(format!("at rest the pit cell {p:?} is not empty"), table);
    }
    if let Some(p) = banks_ok(rest_at) {
        return Outcome::fail(format!("at rest the bank {p:?} is not a block a player stands on"), table);
    }
    let restored = |from: usize, to: usize| (from..to).find(|&f| !moving(f) && t.cells[f] == *rest && (f..to).all(|h| t.cells[h] == *rest));
    let mut cycles: Vec<(usize, usize)> = Vec::new();
    let mut first_out: Option<usize> = None;
    let mut flip_fail: Option<String> = None;
    for (from, to, label) in &s.phases {
        let (from, to) = (*from as usize, (*to as usize).min(t.frames()));
        let left = || -> String {
            let last = to - 1;
            let diff: Vec<String> = t.cells[last].iter().filter(|c| !rest.contains(c)).take(3).map(|(p, id)| format!("{p:?} {}", clock::short(&t.names[*id as usize]))).collect();
            if diff.is_empty() { "cells missing".into() } else { diff.join(", ") }
        };
        if label.starts_with("flip ") || label.starts_with("reflip ") {
            if flip_fail.is_some() {
                continue;
            }
            let ins: Vec<(usize, usize)> = runs(&t.inputs[0][from..to]);
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
        let Some((on, len)) = runs(&t.inputs[0]).into_iter().find(|(st, _)| *st >= from && *st < to) else {
            return Outcome::fail(format!("{label}: the lever never turned on"), table);
        };
        let off = on + len;
        if label == "after flips" {
            let ok = (on..off).find(|&f| extended(f)).is_some_and(|e| (e..off).all(|f| extended(f))) && restored(off, to).is_some();
            table.push(format!("{label}: a full cycle {}", if ok { "works" } else { "fails" }));
            if !ok && flip_fail.is_none() {
                flip_fail = Some("a full cycle after the flips fails".into());
            }
            continue;
        }
        let Some(e) = (on..off).find(|&f| extended(f)) else {
            let why = not_extended(off - 1).unwrap_or_default();
            table.push(format!("{label}: never extended ({why})"));
            return Outcome::fail(format!("{label}: the bridge is never out while the lever is on: at the off edge {why}"), table);
        };
        if let Some(f) = (e..off).find(|&f| !extended(f)) {
            let why = not_extended(f).unwrap_or_default();
            table.push(format!("{label}: extended at +{} but not held (frame {f}: {why})", e - on));
            return Outcome::fail(format!("{label}: out at frame {e}, but at frame {f} {why} while the lever is still on"), table);
        }
        let Some(r) = restored(off.max(e), to) else {
            table.push(format!("{label}: extended at +{}, not restored", e - on));
            return Outcome::fail(format!("{label}: the rest state does not come back after the lever turns off (left: {})", left()), table);
        };
        table.push(format!("{label}: out at +{} gt, back in {} gt after the off edge", e - on, r - off));
        first_out.get_or_insert(e);
        cycles.push((e - on, r - off));
    }
    let Some(&(ext, ret)) = cycles.first() else {
        return Outcome::fail("no cycle judged", table);
    };
    let why = if cycles.iter().all(|c| *c == cycles[0]) { String::new() } else { format!("the cycles differ: {cycles:?} (out, back)") };
    let mut metrics = vec![("gap".into(), format!("{n} cells")), ("width".into(), format!("{} cell{}", g.width, if g.width == 1 { "" } else { "s" })), ("extend".into(), format!("{ext} gt")), ("retract".into(), format!("{ret} gt"))];
    // what the walkway is made of (slime slows a walker)
    if let Some(f) = first_out {
        let mut mats: Vec<String> = g.walkway.iter().filter_map(|p| t.state(f, *p)).map(|s| clock::short(state_of(s)).split('[').next().unwrap_or("").replace('_', " ")).collect();
        mats.sort();
        mats.dedup();
        metrics.push(("walkway".into(), mats.join(", ")));
    }
    if s.phases.iter().any(|p| p.2.starts_with("flip ")) {
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
