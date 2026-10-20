//! `mctas door selffeed <plan.ideal>`: replace a mechanism's ideal inputs by OBSERVERS that watch
//! the mechanism itself (track pext / spine). An observer in a free cell of the door's plane fires
//! 2 gt after the cell it watches changes, so a piston whose plan pulses come right after some
//! block next to its pocket moves can be fed by the mechanism's own motion, with no wiring from
//! the lever at all: the cascade trick of the piston extenders (each unit fired by an observer
//! that sees the previous unit move), applied to doors. Every plan input is a 2 gt pulse or a
//! level; an observer only ever gives 2 gt pulses, and a sticky piston on 2 gt pulses spits when
//! its front is occupied and reaches when it is empty, which is what plans made of 2 gt pulses
//! (`door fullmech --pulses 1`) already use.
//!
//! For each input cell of the plan (a redstone block set and cleared off the plane behind its
//! piston, or in a cell outside the box), every observer placement that powers the same piston
//! is tried with that input's events removed: the observer outputs into the piston, into the cell
//! above it (quasi-connectivity), or into a solid block touching either (a strongly powered block
//! powers its neighbours). Each try is judged by `door ideal-rs`'s checks (redsim, the judge's
//! positions, full open/close cycles). Greedy rounds keep the substitution that still works with
//! the shortest open + close, confirm it at the full positions and go on with the rest; what is
//! left is a smaller plan for the wiring tools (pockets, satwire).

use crate::cosearch as cs;
use crate::ideal::Schedule;
use crate::ideal_rs;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::Pos;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// (open?, tick, cell, Some(state) = set / None = clear / Some("@use") = right-click).
type Event = (bool, u32, Pos, Option<String>);
type Edit = ideal_rs::Edit;

pub struct Options {
    pub extra_offsets: u32,
    pub cycles: u32,
    pub threads: usize,
    /// Grow the design's own box in the plane by this many cells for observer cells (0 keeps the
    /// volume).
    pub margin: i32,
    /// Write the reduced plan (`<out>.ideal`) and its design (`<out>.txt`).
    pub out: Option<PathBuf>,
    /// Only list the working substitutions of the first round.
    pub list: bool,
    /// Inputs never replaced (`x,y,z`).
    pub keep: Vec<Pos>,
    /// Print why each rejected placement fails.
    pub why: bool,
    /// Print the alignment table (pulses vs the watched cells' changes) with this slack, no search.
    pub align: Option<i64>,
    /// Shifts tried for the rest of the plan on each edge (0 first).
    pub shifts: Vec<i64>,
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

const DIRS: [(Pos, &str); 6] = [([1, 0, 0], "east"), ([-1, 0, 0], "west"), ([0, 1, 0], "up"), ([0, -1, 0], "down"), ([0, 0, 1], "south"), ([0, 0, -1], "north")];

fn is_piston(s: &str) -> bool {
    (s.contains("minecraft:piston") || s.contains("minecraft:sticky_piston")) && !s.contains("piston_head")
}

/// A block a strongly powering observer turns into a power source for its neighbours.
fn is_conductor(s: &str) -> bool {
    let name = s.split('[').next().unwrap_or(s);
    matches!(
        name,
        "minecraft:stone" | "minecraft:smooth_stone" | "minecraft:cobblestone" | "minecraft:stone_bricks" | "minecraft:andesite" | "minecraft:diorite" | "minecraft:granite" | "minecraft:deepslate" | "minecraft:obsidian" | "minecraft:white_wool" | "minecraft:white_concrete"
    )
}

/// One observer substitution: the input cell it replaces, the observer's cell and facing, and the
/// worst (open, close) ticks at the screening positions.
#[derive(Clone, Debug)]
struct Sub {
    input: Pos,
    cell: Pos,
    facing: &'static str,
    ticks: (u32, u32),
    /// The shift of the rest of the plan on each edge (open, close).
    shift: [i64; 2],
}

struct Ctx {
    design: Design,
    phase: crate::ideal::PlanPhase,
    cycles: u32,
    /// In-plane free cells (air at rest, inside the box).
    free: BTreeSet<Pos>,
    /// The plane's in-plane directions.
    plane: Vec<(Pos, &'static str)>,
    /// Print why each rejected placement fails.
    why: bool,
}

impl Ctx {
    fn schedule(&self, evs: &[Event]) -> Schedule {
        let mut s = Schedule::from_events(self.design.clone(), evs);
        s.phase = self.phase;
        s
    }

    fn judge(&self, evs: &[Event], edits: &[Edit], offsets: &[Pos]) -> std::result::Result<(u32, u32), String> {
        ideal_rs::run_edited(&self.schedule(evs), &BTreeMap::new(), edits, offsets, self.cycles, false)
    }

    /// The pistons an input cell powers: those next to it and those whose cell above touches it.
    fn fed(&self, g: Pos) -> Vec<Pos> {
        let mut out = Vec::new();
        for (p, st) in &self.design.blocks {
            if !is_piston(st) {
                continue;
            }
            let above = add(*p, [0, 1, 0]);
            let touch = |q: Pos| DIRS.iter().any(|(d, _)| add(q, *d) == g);
            if touch(*p) || (touch(above) && above != g) {
                out.push(*p);
            }
        }
        out
    }

    /// Observer placements that power piston `p`: (cell, facing).
    fn placements(&self, p: Pos, taken: &BTreeSet<Pos>) -> Vec<(Pos, &'static str)> {
        let above = add(p, [0, 1, 0]);
        let mut outs: Vec<Pos> = vec![p, above];
        for base in [p, above] {
            for (d, _) in DIRS {
                let q = add(base, d);
                if self.design.blocks.get(&q).is_some_and(|s| is_conductor(s)) {
                    outs.push(q);
                }
            }
        }
        let mut v = Vec::new();
        for o in outs {
            for (d, f) in &self.plane {
                // facing d: watches c + d, outputs into c - d = o
                let c = add(o, *d);
                if self.free.contains(&c) && !taken.contains(&c) && !v.contains(&(c, *f)) {
                    v.push((c, *f));
                }
            }
        }
        v
    }
}

fn load(path: &Path) -> Result<(Schedule, String)> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let s = Schedule::parse(&text)?;
    Ok((s, text))
}

fn events_of(s: &Schedule) -> Result<Vec<Event>> {
    let mut evs = Vec::new();
    for open in [true, false] {
        for (t, p, st) in s.resolved(open)? {
            evs.push((open, t, p, st));
        }
    }
    Ok(evs)
}

/// The in-plane free cells of the design's box (grown by `margin`).
fn free_cells(d: &Design, along_z: bool, door: Pos, margin: i32) -> BTreeSet<Pos> {
    let (a, b, k) = if along_z { (2usize, 1usize, 0usize) } else { (0usize, 1usize, 2usize) };
    let plane: Vec<&Pos> = d.blocks.keys().filter(|p| p[k] == door[k]).collect();
    let lo_a = plane.iter().map(|p| p[a]).min().unwrap_or(0) - margin;
    let hi_a = plane.iter().map(|p| p[a]).max().unwrap_or(0) + margin;
    let lo_b = plane.iter().map(|p| p[b]).min().unwrap_or(0) - margin;
    let hi_b = plane.iter().map(|p| p[b]).max().unwrap_or(0) + margin;
    let mut out = BTreeSet::new();
    for i in lo_a..=hi_a {
        for j in lo_b..=hi_b {
            let mut p = door;
            p[a] = i;
            p[b] = j;
            if !d.blocks.contains_key(&p) && d.ports.get("lever") != Some(&p) {
                out.insert(p);
            }
        }
    }
    out
}

/// The plan with input `g` removed and, on each edge, every event from `g`'s first pulse on
/// shifted by `d[edge]` ticks (an observer fires when the mechanism lets it, not at the plan's
/// tick: the rest of the plan moves with it).
fn retimed(evs: &[Event], g: Pos, d: [i64; 2]) -> Option<Vec<Event>> {
    let first = |open: bool| evs.iter().filter(|e| e.0 == open && e.2 == g).map(|e| e.1).min();
    let f = [first(true), first(false)];
    let mut out = Vec::new();
    for e in evs.iter().filter(|e| e.2 != g) {
        let k = if e.0 { 0 } else { 1 };
        let mut e2 = e.clone();
        if let Some(t0) = f[k]
            && e.1 >= t0
        {
            let t = e.1 as i64 + d[k];
            if t < 0 {
                return None;
            }
            e2.1 = t as u32;
        }
        out.push(e2);
    }
    Some(out)
}

/// Every working substitution of one input, given the current plan and edits.
fn try_input(ctx: &Ctx, evs: &[Event], edits: &[Edit], g: Pos, screen: &[Pos], threads: usize, shifts: &[i64]) -> Vec<(Sub, Vec<Event>)> {
    let taken: BTreeSet<Pos> = edits.iter().map(|e| e.0).collect();
    let mut cands: Vec<(Pos, &'static str)> = Vec::new();
    for p in ctx.fed(g) {
        for c in ctx.placements(p, &taken) {
            if !cands.contains(&c) {
                cands.push(c);
            }
        }
    }
    let mut jobs: Vec<((Pos, &'static str), [i64; 2])> = Vec::new();
    for c in &cands {
        for &a in shifts {
            for &b in shifts {
                jobs.push((*c, [a, b]));
            }
        }
    }
    let res = cs::par_map(&jobs, threads, |((c, f), d)| {
        let rest = retimed(evs, g, *d)?;
        let mut e2 = edits.to_vec();
        e2.push((*c, Some(format!("minecraft:observer[facing={f}]"))));
        Some((ctx.judge(&rest, &e2, screen), rest))
    });
    let mut out: Vec<(Sub, Vec<Event>)> = Vec::new();
    for (((c, f), d), r) in jobs.iter().zip(res) {
        let Some((r, rest)) = r else { continue };
        match r {
            Ok(ticks) => {
                // keep the smallest shift per placement
                if out.iter().any(|(s, _)| s.cell == *c && s.facing == *f) {
                    continue;
                }
                out.push((Sub { input: g, cell: *c, facing: f, ticks, shift: *d }, rest));
            }
            Err(e) if ctx.why && *d == [0, 0] => println!("    observer at {c:?} facing {f}: {}", e.chars().take(160).collect::<String>()),
            Err(_) => {}
        }
    }
    out
}

/// Every in-box cell's change ticks on each edge (first cycle, the first position): `[open,
/// close]`, cell -> ticks after the flip at which its block state changed (inputs applied at a
/// tick count for that tick).
fn trace(ctx: &Ctx, evs: &[Event], edits: &[Edit]) -> Result<[BTreeMap<Pos, Vec<u32>>; 2]> {
    let s = ctx.schedule(evs);
    let (d, open_ev, close_ev, _) = s.realize(&BTreeMap::new())?;
    let door = *d.ports.get("door").context("design has no door port")?;
    let spec = crate::DoorSpec { door, lever: [0, 0, 0], along_z: d.meta.get("door-axis").is_some_and(|v| v == "z") };
    let mut cells: BTreeMap<Pos, String> = d.blocks.clone();
    for (p, st) in edits {
        match st {
            Some(st) => cells.insert(*p, st.clone()),
            None => cells.remove(p),
        };
    }
    let mut blocks = Vec::new();
    for (p, st) in cells {
        blocks.push((p, cs::convert_state(&st).map_err(|e| anyhow::anyhow!("{st}: {e}"))?));
    }
    let (mut w, (lo, hi), idle) = cs::place_and_settle(&blocks, &spec, 400);
    if !idle {
        bail!("never settles");
    }
    let mut out: [BTreeMap<Pos, Vec<u32>>; 2] = [BTreeMap::new(), BTreeMap::new()];
    for (k, evs) in [(0usize, &open_ev), (1usize, &close_ev)] {
        let mut prev = cs::Snap::take(&w, lo, hi);
        let mut record = |w: &redsim::World, t: u32, prev: &mut cs::Snap, out: &mut BTreeMap<Pos, Vec<u32>>| {
            let now = cs::Snap::take(w, lo, hi);
            for (q, b) in now.iter() {
                if prev.get(q) != Some(b) {
                    out.entry(q).or_default().push(t);
                }
            }
            *prev = now;
        };
        let mut t = 0u32;
        let mut i = 0;
        loop {
            while i < evs.len() && evs[i].0 == t {
                let (_, q, st) = &evs[i];
                if st.as_deref() == Some("@use") {
                    w.use_block(cs::bp(*q));
                } else {
                    let b = match st {
                        Some(s) => cs::convert_state(s).map_err(|e| anyhow::anyhow!("{s}: {e}"))?,
                        None => redsim::Block::Air,
                    };
                    w.set_block(cs::bp(*q), b, redsim::world::flags::ALL);
                }
                i += 1;
            }
            record(&w, t, &mut prev, &mut out[k]);
            if i >= evs.len() {
                break;
            }
            w.tick();
            t += 1;
            record(&w, t, &mut prev, &mut out[k]);
        }
        let mut tt = t;
        let _ = cs::step_until_idle(&mut w, 300, |w| {
            tt += 1;
            record(w, tt, &mut prev, &mut out[k]);
        });
    }
    Ok(out)
}

/// `--align`: for each input, its pulse ticks and, for every observer placement that could
/// replace it, the change ticks of the cell that observer would watch (in the plan as it runs):
/// an observer fires 2 gt after a change, so a placement whose cell changes 2 gt before every
/// pulse (within `slack`) is a self-feed once the plan is retimed to it.
fn align(ctx: &Ctx, evs: &[Event], g: Pos, tr: &[BTreeMap<Pos, Vec<u32>>; 2], slack: i64) {
    let ons = |open: bool| -> Vec<u32> { evs.iter().filter(|e| e.0 == open && e.2 == g && e.3.as_deref().is_some_and(|s| s != "@use")).map(|e| e.1).collect() };
    let (po, pc) = (ons(true), ons(false));
    println!("  input {g:?} -> pistons {:?}: open pulses {po:?}, close pulses {pc:?}", ctx.fed(g));
    let taken = BTreeSet::new();
    let mut seen = BTreeSet::new();
    for p in ctx.fed(g) {
        for (c, f) in ctx.placements(p, &taken) {
            if !seen.insert((c, f)) {
                continue;
            }
            let d = DIRS.iter().find(|(_, n)| *n == f).map(|(d, _)| *d).unwrap();
            let wcell = add(c, d);
            let ch = |k: usize| tr[k].get(&wcell).cloned().unwrap_or_default();
            let (co, cc) = (ch(0), ch(1));
            let fits = |pulses: &[u32], changes: &[u32]| pulses.iter().all(|&t| changes.iter().any(|&u| ((u as i64 + 2) - t as i64).abs() <= slack));
            let ok = fits(&po, &co) && fits(&pc, &cc);
            println!("    {} observer at {c:?} facing {f} (watches {wcell:?}): changes open {co:?} close {cc:?}", if ok { "FITS" } else { "    " });
        }
    }
}

fn write_out(ctx: &Ctx, evs: &[Event], edits: &[Edit], out: &Path, header: &[String]) -> Result<()> {
    let mut d = ctx.design.clone();
    for (p, st) in edits {
        match st {
            Some(s) => d.blocks.insert(*p, s.clone()),
            None => d.blocks.remove(p),
        };
    }
    d.name = format!("{}-sf", d.name);
    let design_path = out.with_extension("txt");
    d.save(&design_path)?;
    let mut lines: Vec<String> = header.iter().map(|h| format!("# {h}")).collect();
    lines.push(format!("design {}", design_path.display()));
    lines.push(format!("phase {}", if ctx.phase == crate::ideal::PlanPhase::Between { "between" } else { "tile" }));
    let mut evs = evs.to_vec();
    evs.sort_by_key(|e| (!e.0, e.1));
    for (open, t, q, st) in &evs {
        let ph = if *open { "open" } else { "close" };
        lines.push(match st.as_deref() {
            Some("@use") => format!("{ph} {t} use {} {} {}", q[0], q[1], q[2]),
            Some(b) => format!("{ph} {t} set {} {} {} {b}", q[0], q[1], q[2]),
            None => format!("{ph} {t} clear {} {} {}", q[0], q[1], q[2]),
        });
    }
    let plan_path = out.with_extension("ideal");
    std::fs::write(&plan_path, lines.join("\n") + "\n")?;
    println!("wrote {} + {}", plan_path.display(), design_path.display());
    Ok(())
}

pub fn cli(path: &Path, o: &Options) -> Result<()> {
    let (s, _text) = load(path)?;
    let design = s.design().clone();
    let door = *design.ports.get("door").context("design has no door port")?;
    let along_z = design.meta.get("door-axis").is_some_and(|v| v == "z");
    let plane: Vec<(Pos, &'static str)> = DIRS.iter().copied().filter(|(d, _)| if along_z { d[0] == 0 } else { d[2] == 0 }).collect();
    let ctx = Ctx { free: free_cells(&design, along_z, door, o.margin), design, phase: s.phase, cycles: o.cycles, plane, why: o.why };
    let mut evs = events_of(&s)?;
    let screen = crate::ideal::offsets(0);
    let full = crate::ideal::offsets(o.extra_offsets);
    let base = ctx.judge(&evs, &[], &screen).map_err(|e| anyhow::anyhow!("the plan itself fails: {e}"))?;
    let inputs = |evs: &[Event]| -> Vec<Pos> {
        let mut v: Vec<Pos> = evs.iter().filter(|e| e.3.as_deref() != Some("@use")).map(|e| e.2).collect();
        v.sort();
        v.dedup();
        v.retain(|p| !o.keep.contains(p));
        v
    };
    println!("{}: {} inputs, {} free plane cells; plan open {} / close {}", path.display(), inputs(&evs).len(), ctx.free.len(), base.0, base.1);
    if let Some(slack) = o.align {
        let tr = trace(&ctx, &evs, &[])?;
        for g in inputs(&evs) {
            align(&ctx, &evs, g, &tr, slack);
        }
        return Ok(());
    }
    let mut edits: Vec<Edit> = Vec::new();
    let mut log: Vec<String> = Vec::new();
    loop {
        let mut found: Vec<(Sub, Vec<Event>)> = Vec::new();
        for g in inputs(&evs) {
            let subs = try_input(&ctx, &evs, &edits, g, &screen, o.threads, &o.shifts);
            let pulses = evs.iter().filter(|e| e.2 == g && e.3.is_some()).count();
            println!("  input {:?} ({} pistons, {pulses} on-events): {} observer substitutions work", g, ctx.fed(g).len(), subs.len());
            if o.list {
                for (s, _) in &subs {
                    println!("    observer at {:?} facing {} (open {} / close {}, rest of the plan shifted {:?})", s.cell, s.facing, s.ticks.0, s.ticks.1, s.shift);
                }
            }
            found.extend(subs);
        }
        if o.list || found.is_empty() {
            break;
        }
        found.sort_by_key(|(s, _)| (s.shift[0].abs() + s.shift[1].abs(), s.ticks.0 + s.ticks.1, s.input, s.cell));
        let mut applied = false;
        for (s, rest) in &found {
            let mut e2 = edits.clone();
            e2.push((s.cell, Some(format!("minecraft:observer[facing={}]", s.facing))));
            match ctx.judge(rest, &e2, &full) {
                Ok(t) => {
                    let line = format!("input {:?} -> observer at {:?} facing {} (rest shifted {:?}; open {} / close {} at {} positions)", s.input, s.cell, s.facing, s.shift, t.0, t.1, full.len());
                    println!("APPLIED {line}");
                    log.push(line);
                    evs = rest.clone();
                    edits = e2;
                    applied = true;
                    break;
                }
                Err(e) => println!("  rejected at the full positions: {:?} at {:?}: {e}", s.input, s.cell),
            }
        }
        if !applied {
            break;
        }
    }
    println!("{} of {} inputs replaced by observers; left: {:?}", edits.len(), edits.len() + inputs(&evs).len(), inputs(&evs));
    if let Some(out) = &o.out {
        if edits.is_empty() {
            bail!("nothing replaced: no output written");
        }
        let mut header = vec![format!("door selffeed {}: {} inputs replaced by observers watching the mechanism", path.display(), edits.len())];
        header.extend(log);
        write_out(&ctx, &evs, &edits, out, &header)?;
    }
    Ok(())
}
