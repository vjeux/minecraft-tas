//! `mctas circuit fly sim`: the redsim runner (milliseconds). Builds the corridor world, places
//! the stop blocks and the machine exactly like the clock judge (raw writes, a shape update of
//! every wire, a `self` update of every block; a `last` kick's block withheld), applies the kick,
//! and records every tick's frame into the [`Recorder`] until the machine comes to rest, reaches
//! the corridor's end, breaks something or the horizon.

use crate::machine::Machine;
use crate::report::{self, End, FlyReport};
use crate::track::Recorder;
use clock::{Action, Kick, Placement, Pos};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};

#[derive(Clone, Debug)]
pub struct SimOptions {
    /// Ticks to run at most (0 = 200 + 60 gt per corridor cell).
    pub horizon: u32,
    /// Corridor length when the design has no `distance:`.
    pub distance: i32,
    /// Free cells around the corridor.
    pub margin: i32,
    /// Print every cell change of the first N ticks.
    pub trace: u32,
    /// Keep running after the machine comes to rest for this many ticks (a machine waiting for
    /// nothing is at rest: a stop).
    pub idle_grace: u32,
}

impl Default for SimOptions {
    fn default() -> Self {
        SimOptions { horizon: 0, distance: 32, margin: 3, trace: 0, idle_grace: 0 }
    }
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// The cells of the machine as redsim blocks (or why redsim cannot run it).
fn parse(m: &Machine) -> Result<(Vec<(BlockPos, Block)>, Vec<(BlockPos, Block)>), String> {
    let mut bad: Vec<String> = Vec::new();
    let mut conv = |s: &str| match Block::parse(s) {
        Ok(b) => Some(b),
        Err(e) => {
            if !bad.contains(&e) {
                bad.push(e);
            }
            None
        }
    };
    let blocks: Vec<(BlockPos, Block)> = m.design().blocks.iter().filter_map(|(p, s)| conv(s).map(|b| (bp(*p), b))).collect();
    let stops: Vec<(BlockPos, Block)> = m.stops.iter().filter_map(|(p, s)| conv(s).map(|b| (bp(*p), b))).collect();
    if let Some(Kick::Place { state, .. }) = &m.clock.kick {
        conv(state);
    }
    if bad.is_empty() { Ok((blocks, stops)) } else { Err(bad.join("; ")) }
}

/// The temporary kick block's cell and window (for the recorder).
pub fn kick_window(m: &Machine) -> Option<(Pos, u32, u32)> {
    match &m.clock.kick {
        Some(Kick::Place { pos, ticks, .. }) => Some((*pos, m.clock.kick_at, m.clock.kick_at + ticks)),
        _ => None,
    }
}

/// Run the machine in redsim.
pub fn simulate(m: &Machine, o: &SimOptions) -> FlyReport {
    let mut log = Vec::new();
    simulate_logged(m, o, &mut log)
}

/// The corridor world with the stop blocks and the machine placed (a `last` kick's block
/// withheld), and the corridor's box; or why redsim cannot run it.
pub fn build(m: &Machine, o: &SimOptions) -> Result<(World, (Pos, Pos)), String> {
    let (blocks, stops) = parse(m).map_err(|e| format!("not modeled by redsim: {e}"))?;
    let (lo, hi) = m.corridor(o.distance, o.margin);
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut w = World::new(bp(lo), size);
    let last = m.clock.withheld().map(bp);
    // stops first, placed and updated on their own (the environment stands before the machine
    // is built; the game runner does the same), then the machine like the clock judge
    if !stops.is_empty() {
        w.place_raw(&stops);
        w.place_updates(&stops);
    }
    let kept: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
    match m.clock.placement {
        Placement::Judge => {
            w.place_raw(&kept);
            w.place_updates(&kept);
        }
        Placement::Player => {
            for (p, b) in &kept {
                w.set_block(*p, *b, flags::ALL);
            }
        }
    }
    Ok((w, (lo, hi)))
}

/// Apply the kick's inputs due at tick `t` (between ticks).
pub fn apply_due(w: &mut World, actions: &mut Vec<(u32, Action)>, t: u32) -> Result<(), String> {
    while actions.first().is_some_and(|(at, _)| *at == t) {
        let (_, a) = actions.remove(0);
        match a {
            Action::Place { pos, state, .. } => {
                let b = Block::parse(&state).map_err(|e| format!("kick block not modeled: {e}"))?;
                w.set_block(bp(pos), b, flags::ALL);
            }
            Action::Use { pos } => {
                if !w.use_block(bp(pos)) {
                    return Err(format!("a right-click on {} is not modeled", w.get(bp(pos))));
                }
            }
            Action::Clear { pos } => {
                w.set_block(bp(pos), Block::Air, flags::ALL);
            }
            Action::ShapeSet { pos, state } => {
                let b = Block::parse(&state).map_err(|e| format!("kick block not modeled: {e}"))?;
                w.set_block(bp(pos), b, flags::CLIENTS);
            }
            Action::Update { pos } => {
                let src = w.get(bp(pos)).kind();
                w.update_neighbors_at(bp(pos), src);
            }
            Action::GameEvent { pos, event } => {
                w.game_event(event, bp(pos));
            }
        }
    }
    Ok(())
}

/// The machine's every frame (non-air cells, stops included) for ticks 0..=ticks, or why it
/// cannot run (the type identity compares runs with these).
pub fn frames(m: &Machine, o: &SimOptions, ticks: u32) -> Result<Vec<Vec<(BlockPos, Block)>>, String> {
    let (mut w, _) = build(m, o)?;
    let mut actions = m.clock.actions();
    let mut out = Vec::new();
    for t in 0..=ticks {
        apply_due(&mut w, &mut actions, t)?;
        out.push(w.blocks());
        if !w.drops.is_empty() || !w.errors.is_empty() {
            break;
        }
        w.tick();
    }
    Ok(out)
}

/// [`simulate`], with the trace lines (`o.trace`) appended to `log`.
pub fn simulate_logged(m: &Machine, o: &SimOptions, log: &mut Vec<String>) -> FlyReport {
    let mut rec = Recorder::new(&m.design().blocks, &m.stops, kick_window(m));
    let (mut w, (lo, hi)) = match build(m, o) {
        Ok(v) => v,
        Err(e) => return report::classify(m, &rec, End::Error(0, e)),
    };
    let k = m.distance.unwrap_or(o.distance).max(1) as u32;
    let horizon = if o.horizon > 0 { o.horizon } else { 200 + 60 * k };
    let mut actions = m.clock.actions();
    let edge = |p: BlockPos| p.x <= lo[0] || p.y <= lo[1] || p.z <= lo[2] || p.x >= hi[0] || p.y >= hi[1] || p.z >= hi[2];
    let mut prev: Vec<(BlockPos, Block)> = Vec::new();
    let mut idle_since: Option<u32> = None;
    let mut t = 0u32;
    let end = loop {
        if let Err(e) = apply_due(&mut w, &mut actions, t) {
            break End::Error(t, e);
        }
        let now = w.blocks();
        let strings: Vec<(Pos, String)> = now.iter().map(|(p, b)| (p.arr(), b.to_string())).collect();
        rec.ingest(t, strings.iter().map(|(p, s)| (*p, s.as_str())), false);
        if t < o.trace {
            log.push(trace_line(t, &w, &prev, &now));
        }
        if let Some((p, b)) = w.drops.first() {
            break End::Drop(t, format!("{b} at {p} was destroyed with a drop (an item entity)"));
        }
        if let Some(e) = w.errors.first() {
            if e.contains("outside the box") {
                break End::Edge(t);
            }
            break End::Error(t, e.clone());
        }
        if now.iter().any(|(p, _)| edge(*p)) {
            break End::Edge(t);
        }
        if actions.is_empty() && w.idle() {
            let since = *idle_since.get_or_insert(t);
            if t - since >= o.idle_grace {
                break End::Idle(since);
            }
        } else {
            idle_since = None;
        }
        if t >= horizon {
            break End::Horizon(t);
        }
        prev = now;
        w.tick();
        t += 1;
    };
    report::classify(m, &rec, end)
}

fn trace_line(t: u32, w: &World, prev: &[(BlockPos, Block)], now: &[(BlockPos, Block)]) -> String {
    use std::collections::BTreeMap;
    let a: BTreeMap<BlockPos, Block> = prev.iter().copied().collect();
    let b: BTreeMap<BlockPos, Block> = now.iter().copied().collect();
    let mut keys: Vec<BlockPos> = a.keys().chain(b.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    let mut changes: Vec<String> = Vec::new();
    for p in keys {
        let x = a.get(&p).copied().unwrap_or(Block::Air);
        let y = b.get(&p).copied().unwrap_or(Block::Air);
        if x != y {
            changes.push(format!("{},{},{} {}", p.x, p.y, p.z, clock::short(&y.to_string())));
        }
    }
    format!("t={t} ({} pending ticks, {} events, {} moving): {}", w.ticks.count(), w.events.len(), w.piston.live_count(), if changes.is_empty() { "-".into() } else { changes.join("; ") })
}
