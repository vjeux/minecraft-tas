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

/// `--extent`: the door-plane bounding box of every non-air cell seen while a plan replays (at
/// rest and after every tick), the plan's own input cells and the lever it uses excluded (the
/// judge's volume ignores the lever; the inputs stand in for wiring). Catches a piston head or
/// a moved block leaving the intended box, which the open/close checks alone do not.
#[derive(Default)]
pub struct ExtentAcc {
    pub skip: std::collections::BTreeSet<Pos>,
    pub plane_z: i32,
    pub lohi: Option<(Pos, Pos)>,
}

impl ExtentAcc {
    fn add_snap(&mut self, s: &Snap, off: Pos) {
        for (q, b) in s.iter() {
            if b == Block::Air {
                continue;
            }
            let r = [q[0] - off[0], q[1] - off[1], q[2] - off[2]];
            if r[2] != self.plane_z || self.skip.contains(&r) {
                continue;
            }
            self.lohi = Some(match self.lohi {
                None => (r, r),
                Some((lo, hi)) => ([lo[0].min(r[0]), lo[1].min(r[1]), lo[2].min(r[2])], [hi[0].max(r[0]), hi[1].max(r[1]), hi[2].max(r[2])]),
            });
        }
    }
}

/// What a run prints of its first cycle at its first position: every block change per tick
/// (`--trace`) and/or the door plane drawn after every tick of a window (`--planes lo..hi`, open
/// and close; ticks count from each flip like the plan's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Trace {
    pub changes: bool,
    pub planes: Option<(u32, u32)>,
}

impl From<bool> for Trace {
    fn from(changes: bool) -> Self {
        Trace { changes, planes: None }
    }
}

/// The door plane (z = the door's) of the watch box as sheet glyphs, rows top down, labeled in
/// design coordinates (`off` = the run's world offset).
fn plane_rows(w: &World, wlo: Pos, whi: Pos, z: i32, off: Pos) -> Vec<String> {
    let mut rows = Vec::new();
    for y in (wlo[1]..=whi[1]).rev() {
        let mut line = format!("{:>3} ", y - off[1]);
        for x in wlo[0]..=whi[0] {
            line.push(crate::sheet::glyph_of(&w.get(BlockPos::new(x, y, z))));
        }
        rows.push(line);
    }
    rows.push(format!("    x from {}", wlo[0] - off[0]));
    rows
}

/// One run at one position. Ok((worst open ticks, worst close ticks)).
fn run_at(blocks: &[(Pos, Block)], spec: &DoorSpec, off: Pos, open_ev: &[Ev], close_ev: &[Ev], cycles: u32, trace: Trace, mut ext: Option<&mut ExtentAcc>) -> std::result::Result<(u32, u32), String> {
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
    if let Some(e) = ext.as_deref_mut() {
        e.add_snap(&rest, off);
    }
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
            let mut prev = if trace.changes && cycle == 0 { Some(Snap::take(&w, wlo, whi)) } else { None };
            if prev.is_some() || (trace.planes.is_some() && cycle == 0) {
                println!("== {phase} (cycle {cycle})");
            }
            let planes = if cycle == 0 { trace.planes } else { None };
            let draw = |w: &World, t: u32| {
                if let Some((lo, hi)) = planes
                    && (lo..=hi).contains(&t)
                {
                    println!("-- {phase} t{t}");
                    for r in plane_rows(w, wlo, whi, spec.door[2], off) {
                        println!("{r}");
                    }
                }
            };
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
                if let Some(e) = ext.as_deref_mut() {
                    e.add_snap(&Snap::take(&w, wlo, whi), off);
                }
                show(&w, format!("t{t}"), &mut prev);
                draw(&w, t);
            }
            let mut tt = t;
            let (n, idle) = cs::step_until_idle(&mut w, 300, |w| {
                ok.push(flag(w));
                tt += 1;
                if let Some(e) = ext.as_deref_mut() {
                    e.add_snap(&Snap::take(w, wlo, whi), off);
                }
                show(w, format!("t{tt}"), &mut prev);
                draw(w, tt);
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
pub fn run(s: &Schedule, vars: &BTreeMap<String, i64>, offsets: &[Pos], cycles: u32, trace: impl Into<Trace>) -> std::result::Result<(u32, u32), String> {
    run_edited(s, vars, &[], offsets, cycles, trace)
}

/// `run` on the schedule's design with some cells replaced first (in memory, no JVM).
pub fn run_edited(s: &Schedule, vars: &BTreeMap<String, i64>, edits: &[Edit], offsets: &[Pos], cycles: u32, trace: impl Into<Trace>) -> std::result::Result<(u32, u32), String> {
    let trace = trace.into();
    let (d, open_ev, close_ev, _notes) = s.realize(vars).map_err(|e| format!("{e:#}"))?;
    let d = &d;
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
        let (o, c) = run_at(&blocks, &spec, *off, &open_ev, &close_ev, cycles, if oi == 0 { trace } else { Trace::default() }, None).map_err(|e| format!("at offset {off:?}: {e}"))?;
        worst = (worst.0.max(o), worst.1.max(c));
    }
    Ok(worst)
}

/// `door ideal-rs --extent`: one cycle at the plan's own position; the door-plane bounding box
/// of every non-air cell at rest and after every tick (`ExtentAcc`: inputs and lever excluded).
pub fn extent(s: &Schedule, vars: &BTreeMap<String, i64>) -> std::result::Result<(Pos, Pos), String> {
    let (d, open_ev, close_ev, _notes) = s.realize(vars).map_err(|e| format!("{e:#}"))?;
    let door = *d.ports.get("door").ok_or("design has no door port")?;
    let spec = DoorSpec { door, lever: [0, 0, 0], along_z: d.meta.get("door-axis").is_some_and(|v| v == "z") };
    if spec.along_z {
        return Err("--extent: only for doors in the x-y plane".into());
    }
    let mut blocks = Vec::new();
    for (p, st) in &d.blocks {
        blocks.push((*p, cs::convert_state(st).map_err(|e| format!("{st} at {p:?}: {e}"))?));
    }
    let mut acc = ExtentAcc { plane_z: door[2], ..Default::default() };
    for (_, p, _) in open_ev.iter().chain(close_ev.iter()) {
        acc.skip.insert(*p);
    }
    // the lever (the judge's volume leaves it out), whether the plan uses it or not
    acc.skip.extend(d.ports.get("lever").copied());
    acc.skip.extend(d.blocks.iter().filter(|(_, st)| st.trim_start_matches("minecraft:").starts_with("lever")).map(|(p, _)| *p));
    run_at(&blocks, &spec, [0, 0, 0], &open_ev, &close_ev, 1, Trace::default(), Some(&mut acc))?;
    acc.lohi.ok_or_else(|| "nothing in the door plane".to_string())
}

/// `--vary` axis: `x,y[,z]=alt|alt|...` (door-plane coordinates of the design; z defaults to 0).
/// An alternative is a block state (`minecraft:` optional), `air`, or `keep` (the design's own
/// block). Returns the cell and its alternatives.
pub(crate) fn parse_vary(v: &str) -> Result<(Pos, Vec<Option<Option<String>>>)> {
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

/// `--holds-rank`: screen many plans for sensor wake-ups. For every ideal input cell, try the
/// sensor modes of `--holds` (hold10, hold30, taps3, taps2) with the SAME start on both edges,
/// starts 0..=max_tick, the other inputs at their plan events. Prints one line per plan, plans
/// with the fewest exact-only inputs (no sensor mode works) first.
pub fn holds_rank(paths: &[std::path::PathBuf], extra_offsets: u32, cycles: u32, threads: usize, max_tick: i64) -> Result<()> {
    let offsets = crate::ideal::offsets(extra_offsets);
    let modes: [(&str, Vec<(i64, i64)>); 4] = [
        ("hold10", vec![(0, 10)]),
        ("hold30", vec![(0, 30)]),
        ("taps3", vec![(0, 2), (10, 12), (20, 22)]),
        ("taps2", vec![(0, 2), (14, 16)]),
    ];
    let mut lines: Vec<(usize, usize, String)> = Vec::new();
    for path in paths {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                println!("{}: {e}", path.display());
                continue;
            }
        };
        let s = match Schedule::parse_with(&text, &BTreeMap::new()) {
            Ok(s) => s,
            Err(e) => {
                println!("{}: {e:#}", path.display());
                continue;
            }
        };
        let vars = s.vars().clone();
        if run(&s, &vars, &offsets, cycles, false).is_err() {
            println!("{}: the plan itself fails", path.display());
            continue;
        }
        let ev_open = s.events_of(true, &vars)?;
        let ev_close = s.events_of(false, &vars)?;
        let cells = s.input_cells();
        // jobs: (input index, mode index, start)
        let mut jobs: Vec<(usize, usize, i64)> = Vec::new();
        for i in 0..cells.len() {
            for m in 0..modes.len() {
                for st in 0..=max_tick {
                    jobs.push((i, m, st));
                }
            }
        }
        let results = cs::par_map(&jobs, threads, |(i, m, st)| {
            let p = cells[*i];
            let state = ev_open
                .iter()
                .chain(ev_close.iter())
                .find(|e| e.1 == p && e.2.is_some())
                .and_then(|e| e.2.clone())
                .unwrap_or_else(|| "minecraft:redstone_block".to_string());
            let mut evs: Vec<(bool, u32, Pos, Option<String>)> = ev_open
                .iter()
                .map(|e| (true, e.0, e.1, e.2.clone()))
                .chain(ev_close.iter().map(|e| (false, e.0, e.1, e.2.clone())))
                .filter(|e| e.2 != p)
                .collect();
            for open in [true, false] {
                for (a, b) in &modes[*m].1 {
                    evs.push((open, (st + a) as u32, p, Some(state.clone())));
                    evs.push((open, (st + b) as u32, p, None));
                }
            }
            let sched = Schedule::from_events(s.design().clone(), &evs);
            run(&sched, &BTreeMap::new(), &offsets, cycles, false).is_ok()
        });
        let mut friendly: Vec<String> = Vec::new();
        let mut exact: Vec<String> = Vec::new();
        for (i, p) in cells.iter().enumerate() {
            let mut parts: Vec<String> = Vec::new();
            for (m, (name, _)) in modes.iter().enumerate() {
                let starts: Vec<i64> = jobs.iter().zip(results.iter()).filter(|((ji, jm, _), r)| *ji == i && *jm == m && **r).map(|((_, _, st), _)| *st).collect();
                if !starts.is_empty() {
                    parts.push(format!("{name}:{}", ranges(&starts)));
                }
            }
            let cell = format!("{},{},{}", p[0], p[1], p[2]);
            if parts.is_empty() {
                exact.push(cell);
            } else {
                friendly.push(format!("{cell}({})", parts.join("|")));
            }
        }
        let line = format!(
            "{}: {} inputs, {} sensor-friendly, {} exact-only [{}]; friendly: {}",
            path.display(),
            cells.len(),
            friendly.len(),
            exact.len(),
            exact.join(" "),
            friendly.join(" ")
        );
        eprintln!("{line}");
        lines.push((exact.len(), cells.len(), line));
    }
    lines.sort_by_key(|l| (l.0, l.1));
    for (_, _, l) in &lines {
        println!("{l}");
    }
    Ok(())
}

/// `--offplane-dir`: write each plan with its in-plane input cells below row `door.y + below`
/// (the sealed bottom; air in the design) moved behind the pistons they feed: one input at
/// (x, y, z+1) per in-plane neighbor holding a piston or standing on one (direct + QC, as the
/// in-plane block powered them). Prints, per plan, the `satwire --keep-ideal` list of those
/// cells: wire the rest, let sensors drive the kept ones.
pub fn offplane_dir(paths: &[std::path::PathBuf], dir: &std::path::Path, below: i32) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for path in paths {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let design_line = text.lines().find(|l| l.trim_start().starts_with("design ")).context("no design line")?.trim().to_string();
        let s = Schedule::parse_with(&text, &BTreeMap::new())?;
        let vars = s.vars().clone();
        let d = s.design();
        let door = *d.ports.get("door").context("design has no door port")?;
        let is_piston = |p: Pos| d.blocks.get(&p).is_some_and(|b| b.contains("piston[") && !b.contains("moving_piston") && !b.contains("piston_head"));
        let mut kept: std::collections::BTreeSet<(i32, i32)> = std::collections::BTreeSet::new();
        let mut lines = vec![format!("# door ideal-rs --offplane-dir {}: in-plane inputs below row {} moved behind the pistons they feed", path.display(), door[1] + below), design_line];
        lines.extend(text.lines().find(|l| l.trim_start().starts_with("phase ")).map(|l| l.trim().to_string()));
        for (open, ph) in [(true, "open"), (false, "close")] {
            for (t, q, st) in s.events_of(open, &vars)? {
                let mut targets = vec![q];
                if q[2] == door[2] && q[1] < door[1] + below && !d.blocks.contains_key(&q) && st.as_deref() != Some("@use") {
                    let mut moved = Vec::new();
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let n = [q[0] + dx, q[1] + dy, q[2]];
                        if is_piston(n) || is_piston([n[0], n[1] - 1, n[2]]) {
                            moved.push([n[0], n[1], n[2] + 1]);
                            kept.insert((n[0], n[1]));
                        }
                    }
                    if !moved.is_empty() {
                        targets = moved;
                    }
                }
                for p in targets {
                    lines.push(match st.as_deref() {
                        Some("@use") => format!("{ph} {t} use {} {} {}", p[0], p[1], p[2]),
                        Some(b) => format!("{ph} {t} set {} {} {} {b}", p[0], p[1], p[2]),
                        None => format!("{ph} {t} clear {} {} {}", p[0], p[1], p[2]),
                    });
                }
            }
        }
        let stem = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let out = dir.join(format!("{stem}-off.ideal"));
        std::fs::write(&out, lines.join("\n") + "\n")?;
        let keep: Vec<String> = kept.iter().map(|(x, y)| format!("{x},{y}")).collect();
        println!("{} --keep-ideal={}", out.display(), keep.join(";"));
    }
    Ok(())
}

/// CLI: run once, or sweep variables (`name=lo..hi`; several = grid, `--independent` = one axis
/// at a time) and/or block variants (`--vary`: every combination of alternatives per cell,
/// applied in memory before each run: screening hand-built wiring variants in milliseconds).
#[allow(clippy::too_many_arguments)]
pub fn cli(path: &std::path::Path, set: &[String], sweep: &[String], independent: bool, extra_offsets: u32, cycles: u32, trace: Trace, threads: usize, vary: &[String], vary_max: usize, ok_only: bool, drop: usize, holds: Option<i64>) -> Result<()> {
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
                        // --vary-max: at most that many changed cells (pruned while building)
                        if vary_max > 0 && e2.len() >= vary_max {
                            continue;
                        }
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
        if let Ok((_, _, _, notes)) = s.realize(&vars) {
            for n in notes {
                eprintln!("note: {n}");
            }
        }
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
    let results = cs::par_map(&jobs, threads, |(_, c, e)| run_edited(&s, c, e, &offsets, cycles, if jobs.len() == 1 { trace } else { Trace::default() }));
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

/// One input group of a plan: a pulse (a `set` and the next `clear` of the same cell) or a lone
/// level edge, in one phase (0 open, 1 close): indices into that phase's event list.
#[derive(Clone, Debug)]
struct Group {
    phase: usize,
    evi: Vec<usize>,
}

fn plan_groups(ev: &[Vec<Ev>; 2]) -> Vec<Group> {
    let mut out = Vec::new();
    for (phase, evs) in ev.iter().enumerate() {
        let mut used = vec![false; evs.len()];
        for i in 0..evs.len() {
            if used[i] || evs[i].2.as_deref() == Some("@use") {
                continue;
            }
            used[i] = true;
            let mut g = vec![i];
            if evs[i].2.is_some()
                && let Some(j) = (i + 1..evs.len()).find(|&j| !used[j] && evs[j].1 == evs[i].1)
                && evs[j].2.is_none()
            {
                used[j] = true;
                g.push(j);
            }
            out.push(Group { phase, evi: g });
        }
    }
    out
}

fn shifted(ev: &[Vec<Ev>; 2], g: &Group, d: i64) -> Option<[Vec<Ev>; 2]> {
    let mut e2 = ev.clone();
    for &i in &g.evi {
        let t = e2[g.phase][i].0 as i64 + d;
        if t < 0 {
            return None;
        }
        e2[g.phase][i].0 = t as u32;
    }
    Some(e2)
}

fn schedule_of(design: &crate::Design, ev: &[Vec<Ev>; 2]) -> Schedule {
    let mut all: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
    for (phase, evs) in ev.iter().enumerate() {
        for (t, p, s) in evs {
            all.push((phase == 0, *t, *p, s.clone()));
        }
    }
    Schedule::from_events(design.clone(), &all)
}

/// `door ideal-rs --compact OUT`: pull every input group (a pulse, or a lone level edge) as EARLY
/// as the mechanism allows, one group at a time in plan order with the others where they stand
/// (two passes), then write the plan with literal ticks and print every group's window (its own
/// shifts that keep the door working, the others fixed). mech-synth plans fire one piston every
/// ~6 gt (inputs up to t66: delay lines no wiring pocket holds); compacted plans put what can
/// happen together at the same tick, before a wiring search.
pub fn compact(path: &std::path::Path, out: &std::path::Path, extra_offsets: u32, cycles: u32, threads: usize) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let s = Schedule::parse(&text)?;
    let vars = s.vars().clone();
    let offsets = crate::ideal::offsets(extra_offsets);
    let design_line = text.lines().find(|l| l.trim_start().starts_with("design ")).context("no design line")?.trim().to_string();
    let mut ev: [Vec<Ev>; 2] = [s.events_of(true, &vars)?, s.events_of(false, &vars)?];
    let works = |e: &[Vec<Ev>; 2]| run(&schedule_of(s.design(), e), &BTreeMap::new(), &offsets, cycles, false);
    let (o0, c0) = works(&ev).map_err(|e| anyhow::anyhow!("the plan itself is BROKEN: {e}"))?;
    let groups = plan_groups(&ev);
    let span = |e: &[Vec<Ev>; 2]| e.iter().map(|v| v.iter().map(|x| x.0).max().unwrap_or(0)).max().unwrap_or(0);
    println!("{}: {} groups, last input t{}, open {o0} / close {c0}", path.display(), groups.len(), span(&ev));
    for pass in 0..2 {
        let mut order: Vec<usize> = (0..groups.len()).collect();
        order.sort_by_key(|&gi| ev[groups[gi].phase][groups[gi].evi[0]].0);
        let mut moved = 0;
        for gi in order {
            let g = &groups[gi];
            let start = ev[g.phase][g.evi[0]].0 as i64;
            if start == 0 {
                continue;
            }
            let ds: Vec<i64> = (-start..0).collect();
            let res = cs::par_map(&ds, threads, |&d| shifted(&ev, g, d).is_some_and(|e2| works(&e2).is_ok()));
            if let Some((d, _)) = ds.iter().zip(res.iter()).find(|(_, ok)| **ok) {
                // (no re-sort: the groups index the events; the runner sorts by tick itself)
                ev = shifted(&ev, g, *d).unwrap();
                moved += 1;
            }
        }
        println!("pass {}: {moved} groups moved earlier, last input t{}", pass + 1, span(&ev));
        if moved == 0 {
            break;
        }
    }
    // their windows (the same groups: indices into the unsorted event lists)
    let (o1, c1) = works(&ev).map_err(|e| anyhow::anyhow!("compacted plan BROKEN (bug): {e}"))?;
    // tidy: identical events of one tick once; a clear and a set of the same cell at one tick
    // (two pulses merged by the compaction) become one continuous pulse; kept only if the plan
    // still works that way
    let mut tidy = ev.clone();
    for evs in tidy.iter_mut() {
        evs.sort_by_key(|x| x.0);
        let mut out: Vec<Ev> = Vec::new();
        for e in evs.iter() {
            if out.iter().any(|o| o.0 == e.0 && o.1 == e.1 && o.2 == e.2) {
                continue;
            }
            out.push(e.clone());
        }
        let mut i = 0;
        while i < out.len() {
            if out[i].2.is_none()
                && let Some(j) = (i + 1..out.len()).find(|&j| out[j].0 == out[i].0 && out[j].1 == out[i].1 && out[j].2.is_some())
            {
                out.remove(j);
                out.remove(i);
                continue;
            }
            i += 1;
        }
        *evs = out;
    }
    let tidy_ok = works(&tidy);
    let (ev, groups, o1, c1) = match tidy_ok {
        Ok((o, c)) => {
            let g = plan_groups(&tidy);
            (tidy, g, o, c)
        }
        Err(_) => (ev, groups, o1, c1),
    };
    let mut lines = vec![format!("# door ideal-rs --compact {}: every input group pulled as early as it goes (others fixed, 2 passes);", path.display()), format!("# open {o1} / close {c1} (was {o0} / {c0}); windows: the group's own shifts that keep it working"), design_line];
    // keep the plan's input phase: without it the written plan is read in the default phase and
    // replays at different ticks than the ones just measured
    lines.extend(text.lines().find(|l| l.trim_start().starts_with("phase ")).map(|l| l.trim().to_string()));
    for g in &groups {
        let ds: Vec<i64> = (-24..=24).filter(|d| *d != 0).collect();
        let res = cs::par_map(&ds, threads, |&d| shifted(&ev, g, d).is_some_and(|e2| works(&e2).is_ok()));
        let ok = |d: i64| d == 0 || ds.iter().zip(res.iter()).any(|(x, r)| *x == d && *r);
        let (mut lo, mut hi) = (0i64, 0i64);
        while lo > -24 && ok(lo - 1) {
            lo -= 1;
        }
        while hi < 24 && ok(hi + 1) {
            hi += 1;
        }
        let (t, q, st) = &ev[g.phase][g.evi[0]];
        let what = match (st, g.evi.len()) {
            (Some(_), 2) => format!("pulse {} gt", ev[g.phase][g.evi[1]].0 - t),
            (Some(_), _) => "on".to_string(),
            (None, _) => "off".to_string(),
        };
        lines.push(format!("# {} t{t} {:?} {what}: window [{lo:+}, {hi:+}]", if g.phase == 0 { "open " } else { "close" }, q));
    }
    for (phase, evs) in ev.iter().enumerate() {
        lines.extend(event_lines(phase, evs));
    }
    std::fs::write(out, lines.join("\n") + "\n")?;
    println!("wrote {} (last input t{}, open {o1} / close {c1})", out.display(), span(&ev));
    Ok(())
}

/// One phase's events as plan lines (`open|close <t> set|clear|use x y z [block]`), by tick.
fn event_lines(phase: usize, evs: &[Ev]) -> Vec<String> {
    let mut evs = evs.to_vec();
    evs.sort_by_key(|x| x.0);
    let ph = if phase == 0 { "open" } else { "close" };
    evs.iter()
        .map(|(t, q, st)| match st.as_deref() {
            Some("@use") => format!("{ph} {t} use {} {} {}", q[0], q[1], q[2]),
            Some(b) => format!("{ph} {t} set {} {} {} {b}", q[0], q[1], q[2]),
            None => format!("{ph} {t} clear {} {} {}", q[0], q[1], q[2]),
        })
        .collect()
}

/// `door prune-plan`: drop every input PULSE (a `set` and its `clear`) the plan does not need,
/// greedily, latest pulse first, each drop kept only while the plan with all drops so far still
/// works (screened at the 4 default positions, then confirmed at `extra_offsets`). Lone level edges,
/// lever uses and the `keep` cells stay. A pulse the plan works without ALONE (`door shuttle`
/// lists those) often cannot go together with the others; this finds a set that can. Fewer pulses
/// = lighter wiring areas for pockets / satwire.
pub fn prune(path: &std::path::Path, out: &std::path::Path, keep: &[Pos], only: &[Pos], exhaustive: bool, threads: usize, extra_offsets: u32, cycles: u32) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let s = Schedule::parse(&text)?;
    let vars = s.vars().clone();
    let (screen, full) = (crate::ideal::offsets(0), crate::ideal::offsets(extra_offsets));
    let design_line = text.lines().find(|l| l.trim_start().starts_with("design ")).context("no design line")?.trim().to_string();
    let ev: [Vec<Ev>; 2] = [s.events_of(true, &vars)?, s.events_of(false, &vars)?];
    let works = |e: &[Vec<Ev>; 2], offs: &[[i32; 3]]| run(&schedule_of(s.design(), e), &BTreeMap::new(), offs, cycles, false);
    let (o0, c0) = works(&ev, &full).map_err(|e| anyhow::anyhow!("the plan itself is BROKEN: {e}"))?;
    let groups = plan_groups(&ev);
    // `only` (when given) restricts the drops to one area's input cells: the greedy order then
    // spends no drop elsewhere that would block one there
    let cand = |p: &Pos| !keep.contains(p) && (only.is_empty() || only.contains(p));
    let mut order: Vec<usize> = (0..groups.len()).filter(|&gi| groups[gi].evi.len() == 2 && cand(&ev[groups[gi].phase][groups[gi].evi[0]].1)).collect();
    order.sort_by_key(|&gi| std::cmp::Reverse(ev[groups[gi].phase][groups[gi].evi[0]].0));
    let without = |rm: &[usize]| -> [Vec<Ev>; 2] {
        let gone = |phase: usize, i: usize| rm.iter().any(|&gi| groups[gi].phase == phase && groups[gi].evi.contains(&i));
        [0, 1].map(|phase| ev[phase].iter().enumerate().filter(|(i, _)| !gone(phase, *i)).map(|(_, e)| e.clone()).collect())
    };
    println!("{}: {} pulses can be tried (of {} input groups), open {o0} / close {c0}", path.display(), order.len(), groups.len());
    let mut removed: Vec<usize> = Vec::new();
    if exhaustive {
        // every subset of the candidates, largest first (each size screened in parallel at the 4
        // default positions, the first hit confirmed at the full set)
        anyhow::ensure!(order.len() <= 16, "--exhaustive takes at most 16 candidate pulses ({}): narrow with --only", order.len());
        let n = order.len();
        'size: for k in (1..=n).rev() {
            let subsets: Vec<Vec<usize>> = (0u32..1 << n).filter(|m| m.count_ones() as usize == k).map(|m| (0..n).filter(|i| m >> i & 1 == 1).map(|i| order[i]).collect()).collect();
            let ok = cs::par_map(&subsets, threads, |sub| works(&without(sub), &screen).is_ok());
            for (sub, _) in subsets.iter().zip(ok.iter()).filter(|(_, ok)| **ok) {
                if works(&without(sub), &full).is_ok() {
                    removed = sub.clone();
                    break 'size;
                }
            }
        }
        for &gi in &removed {
            let g = &groups[gi];
            let (t, q, _) = &ev[g.phase][g.evi[0]];
            println!("  dropped {} t{t} {q:?} ({} gt)", if g.phase == 0 { "open " } else { "close" }, ev[g.phase][g.evi[1]].0 - t);
        }
    }
    for gi in if exhaustive { Vec::new() } else { order } {
        let mut trial = removed.clone();
        trial.push(gi);
        let e2 = without(&trial);
        if works(&e2, &screen).is_ok() && works(&e2, &full).is_ok() {
            let g = &groups[gi];
            let (t, q, _) = &ev[g.phase][g.evi[0]];
            println!("  dropped {} t{t} {q:?} ({} gt)", if g.phase == 0 { "open " } else { "close" }, ev[g.phase][g.evi[1]].0 - t);
            removed = trial;
        }
    }
    let ev = without(&removed);
    let (o1, c1) = works(&ev, &full).map_err(|e| anyhow::anyhow!("pruned plan BROKEN (bug): {e}"))?;
    let how = if exhaustive { "largest working subset" } else { "greedy, latest first" };
    let mut lines = vec![format!("# door prune-plan {}: {} pulses dropped ({how}); open {o1} / close {c1} (was {o0} / {c0})", path.display(), removed.len()), design_line];
    lines.extend(text.lines().find(|l| l.trim_start().starts_with("phase ")).map(|l| l.trim().to_string()));
    for (phase, evs) in ev.iter().enumerate() {
        lines.extend(event_lines(phase, evs));
    }
    std::fs::write(out, lines.join("\n") + "\n")?;
    println!("{} pulses dropped; wrote {} (open {o1} / close {c1}, {} positions x {cycles} cycles)", removed.len(), out.display(), full.len());
    Ok(())
}
