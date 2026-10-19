//! Recording a run: on redsim (the screen: exact for the blocks it models) and in the real game (the
//! judge: the JVM bridge the door and clock judges use), both placing the design exactly like
//! `clock sim` / `clock verify` and applying the inputs between ticks.

use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, Kind, Pos};
use anyhow::{Result, anyhow};
use mctas::sim::{Region, SandboxOptions, Worker};
use std::collections::HashMap;

/// The watched box: the circuit's box plus `margin`, swept along `travel` by `distance` + margin
/// (a flying machine) or by its reach + 1 (a piston extender).
pub fn watch_box(c: &Circuit, margin: i32) -> (Pos, Pos) {
    let (lo, hi) = c.bounds();
    let mut from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let mut to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let sweep = match (&c.kind, c.travel) {
        (Some(Kind::FlyingMachine), Some(_)) => c.distance.unwrap_or(16) as i32 + margin,
        (Some(Kind::PistonExtender(n)), Some(_)) => *n as i32 + 1,
        _ => 0,
    };
    if let Some(t) = c.travel {
        for i in 0..3 {
            if t[i] > 0 {
                to[i] += sweep;
            } else if t[i] < 0 {
                from[i] -= sweep;
            }
        }
    }
    (from, to)
}

/// Run a circuit on redsim with a schedule (its own kick inputs included). A flying machine that
/// reaches the box's far end along `travel` ends its run there ([`Trace::ended`]).
pub fn sim_trace(c: &Circuit, s: &Schedule, margin: i32) -> std::result::Result<Trace, String> {
    let actions = c.actions(s);
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let (from, to) = watch_box(c, margin);
    match clock::sim::drive(&c.clock, &actions, s.ticks, &probes, from, to) {
        Ok(d) => {
            let ended = match d.stopped {
                None => None,
                Some((t, why)) if c.kind == Some(Kind::FlyingMachine) && (why.contains("outside the box") || why.contains("escapes")) => Some(format!("reached the corridor end at frame {t}")),
                Some((_, why)) => return Err(why),
            };
            let mut t = Trace::new(c, d.cells, d.signals);
            t.ended = ended;
            Ok(t)
        }
        Err((_, why)) => Err(why),
    }
}

/// Run a circuit (already turned and moved) in the real game, in a fresh sandbox: `Ok(Err(why))`
/// when the run itself breaks a rule (an entity, a block reaching the edge of the watched box).
pub fn game_trace(w: &mut Worker, c: &Circuit, s: &Schedule, margin: i32) -> Result<std::result::Result<Trace, String>> {
    let (from, to) = watch_box(c, margin);
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = game_in(w, sb, c, s, from, to, margin);
    let _ = w.drop_sandbox(sb);
    r
}

fn frame_cells(f: &Region, extras: &HashMap<Pos, String>) -> Vec<(Pos, String)> {
    let mut v: Vec<(Pos, String)> = f.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(p, s)| (p, clock::wave::with_extra(s, extras.get(&p).map(|e| e.as_str())))).collect();
    v.sort_by_key(|(p, _)| *p);
    v
}

fn game_in(w: &mut Worker, sb: u32, c: &Circuit, s: &Schedule, from: Pos, to: Pos, margin: i32) -> Result<std::result::Result<Trace, String>> {
    clock::verify::place(w, sb, &c.clock)?;
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let mut actions = c.clock.actions();
    actions.extend(c.actions(s));
    actions.sort_by_key(|(t, _)| *t);
    let extras_of = |infos: Vec<mctas::sim::BlockEntityInfo>| -> HashMap<Pos, String> {
        infos.into_iter().filter_map(|b| clock::wave::be_extra_from_nbt(&b.kind, &b.nbt).map(|e| (b.pos, e))).collect()
    };
    let best_of = |flat: &[u8]| -> Vec<u8> { flat.iter().step_by(2).copied().collect() };
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let volume = (size[0] * size[1] * size[2]).max(1) as u32;
    let seg = (400_000 / volume).clamp(10, 2000);
    let mut cells: Vec<Vec<(Pos, String)>> = Vec::with_capacity(s.ticks as usize + 1);
    let mut signals: Vec<Vec<u8>> = Vec::with_capacity(s.ticks as usize + 1);
    let mut t = 0u32;
    loop {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            clock::verify::apply(w, sb, &a)?;
        }
        // frame t: the state after t ticks with its inputs applied (replaces the frame the last
        // step recorded at t)
        let f = w.get_region(sb, from, to)?;
        let ex = extras_of(w.block_entities(sb, from, to)?);
        let sg: Vec<u8> = if probes.is_empty() { Vec::new() } else { w.signals(sb, &probes)?.into_iter().map(|(b, _)| b).collect() };
        if cells.len() as u32 == t + 1 {
            *cells.last_mut().unwrap() = frame_cells(&f, &ex);
            *signals.last_mut().unwrap() = sg;
        } else {
            cells.push(frame_cells(&f, &ex));
            signals.push(sg);
        }
        let next = actions.first().map(|(at, _)| *at).unwrap_or(s.ticks).min(s.ticks).min(t + seg);
        if next <= t {
            break;
        }
        let st = w.step_probe_be(sb, next - t, false, &probes, Some((from, to)), true)?;
        if st.block_entities.len() != st.frames.len() {
            return Err(anyhow!("block-entity frames ({}) do not match the block frames ({}): rebuild the bridge (mctas setup)", st.block_entities.len(), st.frames.len()));
        }
        for (f, dump) in st.frames.iter().zip(&st.block_entities) {
            let ex = extras_of(mctas::sim::parse_block_entities(dump)?);
            cells.push(frame_cells(f, &ex));
        }
        signals.extend(st.signals.iter().map(|v| best_of(v)));
        t = next;
    }
    // nothing may reach the outer layer of the watched box (it escaped the margin), except a flying
    // machine reaching its far end along `travel` (it flew the corridor: the run ends there); and no
    // entity may appear (a drop, a popped block) except the design's own
    let far = |p: &Pos| -> bool { c.kind == Some(Kind::FlyingMachine) && c.travel.is_some_and(|tr| (0..3).any(|i| (tr[i] > 0 && p[i] == to[i]) || (tr[i] < 0 && p[i] == from[i]))) };
    let mut ended = None;
    for (k, frame) in cells.iter().enumerate() {
        if let Some((p, st)) = frame.iter().find(|(p, _)| (0..3).any(|i| p[i] == from[i] || p[i] == to[i])) {
            if far(p) {
                ended = Some(k);
                break;
            }
            return Ok(Err(format!("frame {k}: {} at {p:?} reached the edge of the watched box (margin {margin})", clock::short(st))));
        }
    }
    if let Some(k) = ended {
        cells.truncate(k);
        signals.truncate(k);
    }
    let ents = w.entities(sb)?;
    let foreign = ents.len().saturating_sub(c.clock.design.entities.len());
    if foreign > 0 && ended.is_none() {
        let e = &ents[0];
        return Ok(Err(format!("{} entit{} at the end ({} at {:.1?}...): a drop or a popped block", ents.len(), if ents.len() == 1 { "y" } else { "ies" }, clock::short(&e.kind), e.pos)));
    }
    let mut t = Trace::new(c, cells, signals);
    t.ended = ended.map(|k| format!("reached the corridor end at frame {k}"));
    Ok(Ok(t))
}
