//! `mctas door ideal-rs`: `door ideal` on redsim (no JVM). A mechanism driven by a `door ideal`
//! schedule (redstone blocks placed / cleared, lever uses at given ticks after each flip) is
//! placed like the judge places designs, then run for `--cycles` open/close cycles at the judge's
//! positions with `ideal::run_at`'s checks (closed at rest, open = door cells and walkway clear,
//! closed = door cells restored and the whole watch box back at rest, no drops). `--sweep` runs a
//! grid of variable values in parallel (the timing windows of a mechanism in milliseconds, e.g.
//! for planning sculk sensor timings); `--trace` prints every block change of the first cycle.
//! The JVM `door ideal` stays the reference (redsim is fuzzed against it, `door pistfuzz`).

use crate::DoorSpec;
use crate::cosearch::{self as cs, Pos, Snap};
use crate::ideal::Schedule;
use anyhow::{Context, Result};
use redsim::{Block, BlockPos, World};
use std::collections::BTreeMap;

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

type Ev = (u32, Pos, Option<String>);

/// One run at one position. Ok((worst open ticks, worst close ticks)).
fn run_at(blocks: &[(Pos, Block)], spec: &DoorSpec, off: Pos, open_ev: &[Ev], close_ev: &[Ev], cycles: u32, trace: bool) -> std::result::Result<(u32, u32), String> {
    let blocks: Vec<(Pos, Block)> = blocks.iter().map(|(p, b)| (add(*p, off), *b)).collect();
    let spec = spec.translated(off);
    let (mut w, (wlo, whi), idle) = cs::place_and_settle(&blocks, &spec, 400);
    if !w.errors.is_empty() {
        return Err(format!("model: {}", w.errors.join("; ")));
    }
    if !idle {
        return Err("never settles after placement".into());
    }
    let rest = Snap::take(&w, wlo, whi);
    let problems = cs::closed_problems(&rest, &spec);
    if !problems.is_empty() {
        return Err(format!("not closed at rest: {}", problems.join("; ")));
    }
    let door_cells = spec.door_cells();
    let closed_door: Vec<Block> = door_cells.iter().map(|p| rest.get(*p).unwrap_or(Block::Air)).collect();
    let mut walk: Vec<Pos> = door_cells.clone();
    walk.extend(spec.corridor_cells());
    let walk_bp: Vec<BlockPos> = walk.iter().map(|p| cs::bp(*p)).collect();
    let door_bp: Vec<BlockPos> = door_cells.iter().map(|p| cs::bp(*p)).collect();
    let (mut worst_open, mut worst_close) = (0u32, 0u32);
    for cycle in 0..cycles {
        for (open, evs) in [(true, open_ev), (false, close_ev)] {
            let phase = if open { "open" } else { "close" };
            let mut ok: Vec<bool> = Vec::new();
            let flag = |w: &World| if open { cs::open_ok_world(w, &walk_bp) } else { door_bp.iter().zip(closed_door.iter()).all(|(p, s)| w.get(*p) == *s) };
            let mut prev = if trace && cycle == 0 { Some(Snap::take(&w, wlo, whi)) } else { None };
            if prev.is_some() {
                println!("== {phase} (cycle {cycle})");
            }
            let mut show = |w: &World, label: String, prev: &mut Option<Snap>| {
                if let Some(p) = prev.as_mut() {
                    let now = Snap::take(w, wlo, whi);
                    let ch: Vec<String> = now
                        .iter()
                        .filter(|(q, b)| p.get(*q) != Some(*b))
                        .map(|(q, b)| {
                            let r = [q[0] - off[0], q[1] - off[1], q[2] - off[2]];
                            format!("{:?} {} -> {}", r, p.get(q).map(|x| x.to_string()).unwrap_or_default().trim_start_matches("minecraft:"), b.to_string().trim_start_matches("minecraft:"))
                        })
                        .collect();
                    if !ch.is_empty() {
                        println!("{label}: {}", ch.join("; "));
                    }
                    *p = now;
                }
            };
            let mut t = 0u32;
            let mut i = 0;
            while i < evs.len() {
                while i < evs.len() && evs[i].0 == t {
                    let (_, q, st) = &evs[i];
                    let q = add(*q, off);
                    if st.as_deref() == Some("@use") {
                        w.use_block(cs::bp(q));
                    } else {
                        let b = match st {
                            Some(s) => cs::convert_state(s)?,
                            None => Block::Air,
                        };
                        w.set_block(cs::bp(q), b, redsim::world::flags::ALL);
                    }
                    i += 1;
                }
                show(&w, format!("t{t} inputs"), &mut prev);
                if i >= evs.len() {
                    break;
                }
                w.tick();
                t += 1;
                ok.push(flag(&w));
                show(&w, format!("t{t}"), &mut prev);
            }
            let mut tt = t;
            let (n, idle) = cs::step_until_idle(&mut w, 300, |w| {
                ok.push(flag(w));
                tt += 1;
                show(w, format!("t{tt}"), &mut prev);
            });
            if !w.errors.is_empty() {
                return Err(format!("model: {}", w.errors.join("; ")));
            }
            if !idle {
                return Err(format!("cycle {cycle}: still moving {n} ticks after the last {phase} event"));
            }
            let ticks = cs::settled_from_flags(&ok).unwrap_or(u32::MAX);
            if !w.drops.is_empty() {
                return Err(format!("cycle {cycle}: entities appeared while {phase}ing (something broke): {:?}", w.drops.iter().take(3).collect::<Vec<_>>()));
            }
            if open {
                worst_open = worst_open.max(ticks);
                let p = cs::open_problems(&w, &walk);
                if !p.is_empty() {
                    return Err(format!("cycle {cycle}: not open: {}", p.join("; ")));
                }
            } else {
                worst_close = worst_close.max(ticks);
                let now = Snap::take(&w, wlo, whi);
                if now != rest {
                    let diffs: Vec<String> = rest
                        .iter()
                        .filter(|(q, s)| now.get(*q) != Some(*s))
                        .take(6)
                        .map(|(q, s)| format!("{:?} {} -> {}", [q[0] - off[0], q[1] - off[1], q[2] - off[2]], s.to_string().trim_start_matches("minecraft:"), now.get(q).map(|b| b.to_string()).unwrap_or_default().trim_start_matches("minecraft:")))
                        .collect();
                    return Err(format!("cycle {cycle}: not back at rest after closing: {}", diffs.join("; ")));
                }
            }
        }
    }
    Ok((worst_open, worst_close))
}

/// One block edit applied to the design before a run (`--vary`): the cell and its new state
/// (`None` = air).
pub type Edit = (Pos, Option<String>);

/// Run a schedule with these variables at every offset.
pub fn run(s: &Schedule, vars: &BTreeMap<String, i64>, offsets: &[Pos], cycles: u32, trace: bool) -> std::result::Result<(u32, u32), String> {
    run_edited(s, vars, &[], offsets, cycles, trace)
}

/// `run` on the schedule's design with some cells replaced first (in memory, no JVM).
pub fn run_edited(s: &Schedule, vars: &BTreeMap<String, i64>, edits: &[Edit], offsets: &[Pos], cycles: u32, trace: bool) -> std::result::Result<(u32, u32), String> {
    let open_ev = s.events_of(true, vars).map_err(|e| format!("{e:#}"))?;
    let close_ev = s.events_of(false, vars).map_err(|e| format!("{e:#}"))?;
    let d = s.design();
    let door = *d.ports.get("door").ok_or("design has no door port")?;
    let spec = DoorSpec { door, lever: [0, 0, 0], along_z: d.meta.get("door-axis").is_some_and(|v| v == "z") };
    let mut cells: BTreeMap<Pos, String> = d.blocks.clone();
    for (p, st) in edits {
        match st {
            Some(st) => cells.insert(*p, st.clone()),
            None => cells.remove(p),
        };
    }
    let mut blocks = Vec::new();
    for (p, st) in cells {
        blocks.push((p, cs::convert_state(&st).map_err(|e| format!("{st} at {p:?}: {e}"))?));
    }
    let mut worst = (0u32, 0u32);
    for (oi, off) in offsets.iter().enumerate() {
        let (o, c) = run_at(&blocks, &spec, *off, &open_ev, &close_ev, cycles, trace && oi == 0).map_err(|e| format!("at offset {off:?}: {e}"))?;
        worst = (worst.0.max(o), worst.1.max(c));
    }
    Ok(worst)
}

/// `--vary` axis: `x,y[,z]=alt|alt|...` (door-plane coordinates of the design; z defaults to 0).
/// An alternative is a block state (`minecraft:` optional), `air`, or `keep` (the design's own
/// block). Returns the cell and its alternatives.
fn parse_vary(v: &str) -> Result<(Pos, Vec<Option<Option<String>>>)> {
    let (c, alts) = v.split_once('=').context("--vary x,y[,z]=state|state|...")?;
    let n: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().context("--vary cell")?;
    let p = match n.as_slice() {
        [x, y] => [*x, *y, 0],
        [x, y, z] => [*x, *y, *z],
        _ => anyhow::bail!("--vary cell must be x,y or x,y,z: {c}"),
    };
    let mut out = Vec::new();
    for a in alts.split('|') {
        let a = a.trim();
        out.push(match a {
            "keep" => None,
            "air" | "minecraft:air" => Some(None),
            _ if a.contains(':') => Some(Some(a.to_string())),
            _ => Some(Some(format!("minecraft:{a}"))),
        });
    }
    Ok((p, out))
}

/// Compact "a..b,c" form of a sorted list of ticks.
fn ranges(v: &[i64]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < v.len() {
        let mut j = i;
        while j + 1 < v.len() && v[j + 1] == v[j] + 1 {
            j += 1;
        }
        out.push(if j > i { format!("{}..{}", v[i], v[j]) } else { v[i].to_string() });
        i = j + 1;
    }
    if out.is_empty() { "-".to_string() } else { out.join(",") }
}

/// `--holds`: for every ideal input cell of the plan, replace its events by what a sculk sensor
/// can give it and report which starts work: `hold10` / `hold30` (the cell powered for a
/// calibrated / plain sensor's window), `taps3` (3 two-tick pulses 10 apart: an observer on a
/// calibrated sensor), `taps2` (2 pulses 14 apart: sensor -> lamp -> observer), `pulse` (one
/// two-tick pulse, for reference). The start on open and on close are swept independently over
/// `none` and 0..=max_tick; the other inputs keep their plan events. "both" lists the starts that
/// work with the SAME tick on both edges (one sensor hearing the same event at both flips).
fn holds_cli(s: &Schedule, vars: &BTreeMap<String, i64>, offsets: &[Pos], cycles: u32, threads: usize, max_tick: i64) -> Result<()> {
    let ev_open = s.events_of(true, vars)?;
    let ev_close = s.events_of(false, vars)?;
    let cells = s.input_cells();
    let modes: [(&str, Vec<(i64, i64)>); 5] = [
        ("pulse", vec![(0, 2)]),
        ("hold10", vec![(0, 10)]),
        ("hold30", vec![(0, 30)]),
        ("taps3", vec![(0, 2), (10, 12), (20, 22)]),
        ("taps2", vec![(0, 2), (14, 16)]),
    ];
    let starts: Vec<i64> = std::iter::once(-1).chain(0..=max_tick).collect();
    for p in &cells {
        let state = ev_open
            .iter()
            .chain(ev_close.iter())
            .find(|e| e.1 == *p && e.2.is_some())
            .and_then(|e| e.2.clone())
            .unwrap_or_else(|| "minecraft:redstone_block".to_string());
        let base: Vec<(bool, u32, Pos, Option<String>)> = ev_open
            .iter()
            .map(|e| (true, e.0, e.1, e.2.clone()))
            .chain(ev_close.iter().map(|e| (false, e.0, e.1, e.2.clone())))
            .filter(|e| e.2 != *p)
            .collect();
        for (name, shape) in &modes {
            let mut jobs: Vec<(i64, i64)> = Vec::new();
            for &ho in &starts {
                for &hc in &starts {
                    if ho >= 0 || hc >= 0 {
                        jobs.push((ho, hc));
                    }
                }
            }
            let results = cs::par_map(&jobs, threads, |(ho, hc)| {
                let mut evs = base.clone();
                for (open, h) in [(true, *ho), (false, *hc)] {
                    if h < 0 {
                        continue;
                    }
                    for (a, b) in shape {
                        evs.push((open, (h + a) as u32, *p, Some(state.clone())));
                        evs.push((open, (h + b) as u32, *p, None));
                    }
                }
                let sched = Schedule::from_events(s.design().clone(), &evs);
                run(&sched, &BTreeMap::new(), offsets, cycles, false).is_ok()
            });
            let ok: Vec<(i64, i64)> = jobs.iter().zip(results).filter(|(_, r)| *r).map(|(j, _)| *j).collect();
            let mut opens: Vec<i64> = ok.iter().map(|j| j.0).collect();
            opens.sort();
            opens.dedup();
            let mut closes: Vec<i64> = ok.iter().map(|j| j.1).collect();
            closes.sort();
            closes.dedup();
            let both: Vec<i64> = ok.iter().filter(|j| j.0 == j.1).map(|j| j.0).collect();
            let fmt = |v: &[i64]| {
                let none = v.first() == Some(&-1);
                let rest: Vec<i64> = v.iter().copied().filter(|t| *t >= 0).collect();
                format!("{}{}", if none { "none," } else { "" }, ranges(&rest))
            };
            println!(
                "input {},{},{} {name}: {} of {} (open start, close start) pairs work; open {}; close {}; both {}",
                p[0],
                p[1],
                p[2],
                ok.len(),
                jobs.len(),
                fmt(&opens),
                fmt(&closes),
                ranges(&both)
            );
        }
    }
    Ok(())
}

/// CLI: run once, or sweep variables (`name=lo..hi`; several = grid, `--independent` = one axis
/// at a time) and/or block variants (`--vary`: every combination of alternatives per cell,
/// applied in memory before each run: screening hand-built wiring variants in milliseconds).
#[allow(clippy::too_many_arguments)]
pub fn cli(path: &std::path::Path, set: &[String], sweep: &[String], independent: bool, extra_offsets: u32, cycles: u32, trace: bool, threads: usize, vary: &[String], ok_only: bool, drop: usize, holds: Option<i64>) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut overrides = BTreeMap::new();
    for kv in set {
        let (k, v) = kv.split_once('=').context("--set name=value")?;
        overrides.insert(k.to_string(), v.parse::<i64>()?);
    }
    let s = Schedule::parse_with(&text, &overrides)?;
    let vars = s.vars().clone();
    let offsets = crate::ideal::offsets(extra_offsets);
    if let Some(max_tick) = holds {
        return holds_cli(&s, &vars, &offsets, cycles, threads, max_tick);
    }
    let mut variants: Vec<(Vec<String>, Vec<Edit>)> = vec![(Vec::new(), Vec::new())];
    for v in vary {
        let (p, alts) = parse_vary(v)?;
        let mut next = Vec::new();
        for (label, edits) in &variants {
            for a in &alts {
                let mut l2 = label.clone();
                let mut e2 = edits.clone();
                match a {
                    None => l2.push(format!("{},{}=keep", p[0], p[1])),
                    Some(st) => {
                        l2.push(format!("{},{}={}", p[0], p[1], st.as_deref().unwrap_or("air").trim_start_matches("minecraft:")));
                        e2.push((p, st.clone()));
                    }
                }
                next.push((l2, e2));
            }
        }
        variants = next;
    }
    if drop > 0 {
        // Every subset of up to `drop` removable blocks (not the 9 door cells, not the lever
        // port, not piston heads: their base is removed with them) taken out, on top of each
        // --vary combination: a redsim block minimizer.
        let d = s.design();
        let door = *d.ports.get("door").context("design has no door port")?;
        let spec = DoorSpec { door, lever: [0, 0, 0], along_z: d.meta.get("door-axis").is_some_and(|v| v == "z") };
        let fixed: std::collections::BTreeSet<Pos> = spec.door_cells().into_iter().chain(d.ports.get("lever").copied()).collect();
        let cells: Vec<Pos> = d.blocks.iter().filter(|(p, st)| !fixed.contains(*p) && !st.contains("piston_head")).map(|(p, _)| *p).collect();
        let mut subsets: Vec<Vec<Pos>> = vec![Vec::new()];
        let mut frontier: Vec<(usize, Vec<Pos>)> = vec![(0, Vec::new())];
        for _ in 0..drop {
            let mut next = Vec::new();
            for (start, sub) in &frontier {
                for (i, p) in cells.iter().enumerate().skip(*start) {
                    let mut s2 = sub.clone();
                    s2.push(*p);
                    subsets.push(s2.clone());
                    next.push((i + 1, s2));
                }
            }
            frontier = next;
        }
        let mut next = Vec::new();
        for (label, edits) in &variants {
            for sub in &subsets {
                let mut l2 = label.clone();
                let mut e2 = edits.clone();
                for p in sub {
                    l2.push(format!("-{},{}", p[0], p[1]));
                    e2.push((*p, None));
                }
                next.push((l2, e2));
            }
        }
        variants = next;
    }
    if sweep.is_empty() && vary.is_empty() && drop == 0 {
        match run(&s, &vars, &offsets, cycles, trace) {
            Ok((o, c)) => println!("WORKS in redsim ({} positions x {cycles} cycles): open after {o} ticks / closed after {c} ticks", offsets.len()),
            Err(e) => {
                println!("BROKEN: {e}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    let mut axes: Vec<(String, Vec<i64>)> = Vec::new();
    for sw in sweep {
        let (k, r) = sw.split_once('=').context("--sweep name=lo..hi")?;
        let (lo, hi) = r.split_once("..").context("--sweep name=lo..hi")?;
        let (lo, hi): (i64, i64) = (lo.parse()?, hi.parse()?);
        axes.push((k.to_string(), (lo..=hi).collect()));
    }
    let mut combos: Vec<(Vec<String>, BTreeMap<String, i64>)> = vec![(Vec::new(), vars.clone())];
    if independent && !axes.is_empty() {
        combos.clear();
        for (k, vals) in &axes {
            for v in vals {
                let mut c = vars.clone();
                c.insert(k.clone(), *v);
                combos.push((vec![format!("{k}={v}")], c));
            }
        }
    } else {
        for (k, vals) in &axes {
            let mut next = Vec::new();
            for (label, c) in &combos {
                for v in vals {
                    let mut c2 = c.clone();
                    c2.insert(k.clone(), *v);
                    let mut l2 = label.clone();
                    l2.push(format!("{k}={v}"));
                    next.push((l2, c2));
                }
            }
            combos = next;
        }
    }
    let mut jobs: Vec<(String, BTreeMap<String, i64>, Vec<Edit>)> = Vec::new();
    for (vl, edits) in &variants {
        for (label, c) in &combos {
            let mut l = vl.clone();
            l.extend(label.iter().cloned());
            jobs.push((l.join(" "), c.clone(), edits.clone()));
        }
    }
    let results = cs::par_map(&jobs, threads, |(_, c, e)| run_edited(&s, c, e, &offsets, cycles, trace && jobs.len() == 1));
    let mut n_ok = 0usize;
    for ((label, _, _), r) in jobs.iter().zip(results) {
        match r {
            Ok((o, c)) => {
                n_ok += 1;
                println!("{label}: ok open {o} close {c}");
            }
            Err(e) => {
                if !ok_only {
                    println!("{label}: FAIL {}", e.chars().take(170).collect::<String>());
                }
            }
        }
    }
    if ok_only {
        eprintln!("{n_ok} of {} combinations work", jobs.len());
    }
    Ok(())
}
