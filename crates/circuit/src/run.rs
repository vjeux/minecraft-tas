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
/// reaches the box's far end along `travel` ends its run there ([`Trace::ended`]). A block that
/// cannot survive where it stands (a floating lever, torch, dust or repeater) fails it.
pub fn sim_trace(c: &Circuit, s: &Schedule, margin: i32) -> std::result::Result<Trace, String> {
    let actions = c.actions(s);
    let probes: Vec<Pos> = c.outputs.iter().map(|o| o.pos).collect();
    let (from, to) = watch_box(c, margin);
    // the judge's consumer in every air output cell (crate::CONSUMER)
    let cc = c.with_consumers().map_err(|e| format!("{e:#}"))?;
    if let Some((p, st)) = clock::sim::unsupported(&cc.clock, from, to)?.first() {
        return Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(st)));
    }
    let tnt = matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered));
    match clock::sim::drive_with(&cc.clock, &actions, s.ticks, &probes, from, to, tnt && !c.explodes()) {
        Ok(d) => {
            let ended = match d.stopped {
                None => None,
                Some((t, why)) if c.kind == Some(Kind::FlyingMachine) && (why.contains("outside the box") || why.contains("escapes")) => Some(format!("reached the corridor end at frame {t}")),
                Some((_, why)) => return Err(why),
            };
            let mut t = Trace::new(c, d.cells, d.signals);
            t.ended = ended;
            // a primed TNT spawns at its cell's bottom center (redsim has no motion for it)
            t.primed = d.primed.iter().map(|(f, p)| crate::trace::Primed { frame: *f, pos: [p[0] as f64 + 0.5, p[1] as f64, p[2] as f64 + 0.5], motion: None, end: None }).collect();
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
    // the judge's consumer in every air output cell (crate::CONSUMER)
    let cc = c.with_consumers()?;
    clock::verify::place(w, sb, &cc.clock)?;
    // every block must be able to stand where it is (a player could build it): the placement
    // removes a floating dust (its shape update) and keeps a floating lever, torch or repeater,
    // which would pop at its first shape update
    let placed = w.get_region(sb, from, to)?;
    let popped = cc.clock.design.blocks.iter().filter(|(p, _)| Some(**p) != cc.clock.withheld()).find(|(p, _)| placed.get(**p).is_none_or(mctas::blockstate::is_air)).map(|(p, s)| (*p, s.clone()));
    let floating = w.unsupported(sb, from, to)?.first().map(|p| (*p, cc.clock.design.blocks.get(p).cloned().unwrap_or_default()));
    if let Some((p, st)) = popped.or(floating) {
        return Ok(Err(format!("{} at {p:?} has no support (a player could not build it: it pops at its first shape update)", clock::short(&st))));
    }
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
    // TNT dupers: one tick at a time, every new primed TNT recorded and removed before it can
    // explode
    let tnt = matches!(c.kind, Some(Kind::TntDuper | Kind::TntDuperTriggered));
    let seg = if tnt { 1 } else { (400_000 / volume).clamp(10, 2000) };
    let mut primed: Vec<crate::trace::Primed> = Vec::new();
    let mut seen_tnt: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut tnt_index: HashMap<String, usize> = HashMap::new();
    let explode = tnt && c.explodes();
    let mut burning: HashMap<String, i32> = HashMap::new();
    let mut exploded = 0u32;
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
        if tnt {
            // every primed TNT is followed to fuse 1 (its position there: the precision) and
            // removed then, before it can explode (with `explode: on`: left to explode; a TNT seen
            // before and gone now has exploded)
            let ents = w.entities(sb)?;
            let alive: std::collections::HashSet<&str> = ents.iter().filter(|e| e.kind.trim_start_matches("minecraft:") == "tnt").map(|e| e.uuid.as_str()).collect();
            let mut vanished_early: Option<(String, i32)> = None;
            burning.retain(|u, last_fuse| {
                let gone = !alive.contains(u.as_str());
                if gone {
                    // an explosion only if its last sighting had fuse <= 1 (clockinfra): a TNT gone
                    // earlier left the loaded world or was removed otherwise
                    if *last_fuse <= 1 {
                        exploded += 1;
                    } else if vanished_early.is_none() {
                        vanished_early = Some((u.clone(), *last_fuse));
                    }
                }
                !gone
            });
            if let Some((u, f)) = vanished_early {
                return Ok(Err(format!("frame {next}: a primed TNT ({u}) vanished with fuse {f} left (out of the loaded world?), not an explosion")));
            }
            for e in ents {
                if e.kind.trim_start_matches("minecraft:") != "tnt" {
                    continue;
                }
                if seen_tnt.insert(e.uuid.clone()) {
                    tnt_index.insert(e.uuid.clone(), primed.len());
                    primed.push(crate::trace::Primed { frame: next, pos: e.pos, motion: Some(e.motion), end: None });
                }
                let fuse = e.state.split("fuse: ").nth(1).and_then(|s| s.split(|c: char| !c.is_ascii_digit() && c != '-').next()).and_then(|s| s.parse::<i32>().ok());
                let Some(fuse) = fuse else {
                    return Err(anyhow!("a primed TNT's fuse is not in its saved data ({}): the bridge's entity data changed", e.state));
                };
                if explode {
                    burning.insert(e.uuid.clone(), fuse);
                }
                // the burning TNT is a real entity in the game only (redsim records primes): an
                // entity-reacting block it touches would see it in the game and not on redsim
                let bb = e.bbox;
                if let Some(frame) = cells.last() {
                    let hit = frame.iter().find(|(p, s)| {
                        (s.contains("pressure_plate") || s.contains("tripwire") || s.contains("detector_rail"))
                            && (0..3).all(|i| (p[i] as f64) < bb[i + 3] && (p[i] as f64 + 1.0) > bb[i])
                    });
                    if let Some((p, s)) = hit {
                        return Ok(Err(format!("frame {next}: a primed TNT touches {} at {p:?} (entity-reacting blocks see burning TNT in the game, not on redsim)", clock::short(s))));
                    }
                }
                if fuse <= 1 {
                    if let Some(&k) = tnt_index.get(&e.uuid) {
                        primed[k].end = Some((e.pos, e.motion));
                    }
                    if !explode {
                        w.exec(sb, &format!("kill {}", e.uuid))?;
                    }
                }
            }
        }
        t = next;
    }
    if tnt {
        // the TNT still burning when the run ends: removed (the judge's, not the design's)
        for e in w.entities(sb)? {
            if e.kind.trim_start_matches("minecraft:") == "tnt" {
                w.exec(sb, &format!("kill {}", e.uuid))?;
            }
        }
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
    t.primed = primed;
    t.exploded = exploded;
    Ok(Ok(t))
}
