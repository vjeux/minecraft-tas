//! `door shuttle` (track clk-rt): feed a piston from a SHUTTLED observer. A sticky piston S carries
//! an observer O one cell out and back; a moved observer fires 2 gt after it lands, into the cell
//! behind it, so one S pulse gives two O pulses: on arrival at the far cell and on the return to the
//! near cell. Pistons with no free neighbour (a wiggle piston boxed in the frame, a piston sitting on
//! a drop cell) can be fed this way: mm01z (c70b's 7x7 mm01, its P) and s69z (fly's 6x9 s69, its Qv),
//! designs/piston-door-3x3/tracks/clk-rt/.
//!
//! For the target inputs (plan input cells whose pulses the shuttle must replace), every placement
//! of S (a free plane cell and an in-plane facing) with O in front (an in-plane facing) and a free
//! landing cell beyond it is tried. Each target pulse at tick t comes from O's arrival (S set at
//! t - lead, held h ticks) or from its return (S cleared at t - lead, after a hold of h). Stage 1
//! screens every pulse on its own (the other target pulses kept ideal); stage 2 judges combinations
//! of the survivors with every target input removed, at the four screening positions, then at the
//! full positions. `--absorb` then greedily drops other plan pulses that O's free pulses (the other
//! end of each S pulse) or extra S pulses can give instead (Q in mm01z). S's ideal input sits behind
//! it (off the plane): whatever powers S in a real build also reaches the piston below S through
//! quasi-connectivity, and so does that input.

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

#[derive(Clone)]
pub struct Options {
    /// Plan input cells whose pulses the shuttle replaces.
    pub targets: Vec<Pos>,
    /// Only the target pulses of this edge (true = open, false = close; None = both).
    pub edge: Option<bool>,
    /// Also target every input with a firing `door feeds` finds no option for.
    pub unfed: bool,
    /// Keep the shuttles that replace only some of the target pulses (the most first; the rest stay
    /// ideal): load relief for an overloaded area.
    pub partial: bool,
    /// The plan is ONE train for both edges: a tick is dropped only with both edges' copies.
    pub train: bool,
    /// Ticks S stays powered (>= 3: a sticky piston on a shorter pulse drops the observer).
    pub holds: Vec<u32>,
    /// Ticks from S's switch to the plan tick it stands for (4 = 2 gt travel + 2 gt observer delay).
    pub leads: Vec<u32>,
    /// The same for a carried redstone block (it powers on landing: about 2).
    pub rb_leads: Vec<u32>,
    /// S, O and the landing cell within this in-plane distance of a fed piston (0 = anywhere).
    pub radius: i32,
    /// Grow the design's box in the plane by this many cells (0 keeps the volume).
    pub margin: i32,
    pub extra_offsets: u32,
    pub cycles: u32,
    pub threads: usize,
    /// Combinations judged per placement in stage 2.
    pub max_combos: usize,
    /// Greedily replace other plan pulses with O's free pulses or extra S pulses.
    pub absorb: bool,
    /// Write the working variants (`<stem>-sh<k>.txt` + `.ideal`) here.
    pub out_dir: Option<PathBuf>,
    /// Variants printed and written.
    pub top: usize,
    /// What S carries: an observer (fires 2 gt after landing, into the cell behind it, in every
    /// in-plane facing) or a redstone block (powers its neighbours while it sits).
    pub payloads: Vec<String>,
}

const DIRS: [(Pos, &str); 6] = [([1, 0, 0], "east"), ([-1, 0, 0], "west"), ([0, 1, 0], "up"), ([0, -1, 0], "down"), ([0, 0, 1], "south"), ([0, 0, -1], "north")];
const RB: &str = "minecraft:redstone_block";

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// No facing (a carried redstone block).
fn d_none() -> (Pos, &'static str) {
    ([0, 0, 0], "-")
}

/// One plan pulse: an input cell set at `set` and cleared at `clear` on one edge.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pulse {
    open: bool,
    set: u32,
    clear: u32,
    cell: Pos,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Arrive,
    Return,
}

/// How S gives one pulse: by O's arrival or its return, S's hold, the lead.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Opt {
    mode: Mode,
    hold: u32,
    lead: u32,
}

impl Opt {
    /// S's (set, clear) ticks for a plan pulse at tick t; None when S would switch before the flip.
    fn window(&self, t: u32) -> Option<(u32, u32)> {
        let (t, l, h) = (t as i64, self.lead as i64, self.hold as i64);
        let (s, c) = match self.mode {
            Mode::Arrive => (t - l, t - l + h),
            Mode::Return => (t - l - h, t - l),
        };
        (s >= 0).then_some((s as u32, c as u32))
    }
    fn label(&self) -> String {
        format!("{} h{}{}", if self.mode == Mode::Arrive { "arrive" } else { "return" }, self.hold, if self.lead == 4 { String::new() } else { format!(" lead{}", self.lead) })
    }
}

/// A shuttle placement: S's cell and facing, the carried block's cell and state (an observer and its
/// facing, or a redstone block), the landing cell, S's input cell.
#[derive(Clone, Debug)]
struct Place {
    s: Pos,
    sdir: &'static str,
    o: Pos,
    odir: (Pos, &'static str),
    rb: bool,
    land: Pos,
    input: Pos,
}

impl Place {
    fn edits(&self) -> Vec<Edit> {
        let carried = if self.rb { RB.to_string() } else { format!("minecraft:observer[facing={},powered=false]", self.odir.1) };
        vec![(self.s, Some(format!("minecraft:sticky_piston[extended=false,facing={}]", self.sdir))), (self.o, Some(carried))]
    }
    fn label(&self) -> String {
        let carried = if self.rb { format!("redstone block {:?}", self.o) } else { format!("O {:?} facing {}", self.o, self.odir.1) };
        format!("S {:?} facing {}, {carried} (lands {:?})", self.s, self.sdir, self.land)
    }
}

/// S's pulses: (open?, set, clear).
type Windows = Vec<(bool, u32, u32)>;

struct Ctx {
    design: Design,
    phase: crate::ideal::PlanPhase,
    cycles: u32,
}

impl Ctx {
    fn judge(&self, evs: &[Event], edits: &[Edit], offsets: &[Pos]) -> std::result::Result<(u32, u32), String> {
        let mut s = Schedule::from_events(self.design.clone(), evs);
        s.phase = self.phase;
        ideal_rs::run_edited(&s, &BTreeMap::new(), edits, offsets, self.cycles, false)
    }
}

/// The plan's pulses on `cells` (a set paired with the next clear of the same cell and edge).
fn pulses_of(evs: &[Event], cells: &[Pos]) -> Vec<Pulse> {
    let mut out = Vec::new();
    for &g in cells {
        for open in [true, false] {
            let mut evs_g: Vec<&Event> = evs.iter().filter(|e| e.0 == open && e.2 == g && e.3.as_deref() != Some("@use")).collect();
            evs_g.sort_by_key(|e| e.1);
            let mut on: Option<u32> = None;
            for e in evs_g {
                match (&e.3, on) {
                    (Some(_), None) => on = Some(e.1),
                    (None, Some(t)) => {
                        out.push(Pulse { open, set: t, clear: e.1, cell: g });
                        on = None;
                    }
                    _ => {}
                }
            }
        }
    }
    out.sort_by_key(|p| (!p.open, p.set));
    out
}

/// The plan without these pulses' set and clear events.
fn without(evs: &[Event], drop: &[Pulse]) -> Vec<Event> {
    evs.iter().filter(|e| !drop.iter().any(|p| p.open == e.0 && p.cell == e.2 && (e.1 == p.set || e.1 == p.clear))).cloned().collect()
}

/// The plan with S's pulses added at S's input cell.
fn with_windows(evs: &[Event], input: Pos, w: &Windows) -> Vec<Event> {
    let mut out = evs.to_vec();
    for &(open, s, c) in w {
        out.push((open, s, input, Some(RB.to_string())));
        out.push((open, c, input, None));
    }
    out
}

/// No two S pulses of one edge overlap (the next one starts >= 2 gt after the last clear).
fn disjoint(w: &Windows) -> bool {
    for open in [true, false] {
        let mut v: Vec<(u32, u32)> = w.iter().filter(|x| x.0 == open).map(|x| (x.1, x.2)).collect();
        v.sort();
        if v.windows(2).any(|p| p[1].0 < p[0].1 + 2) {
            return false;
        }
    }
    true
}

/// The pistons each pulse powers when it switches on (next to its cell, or whose cell above touches
/// it), from a replay of the plan at the plan's own position.
fn fed_pistons(ctx: &Ctx, evs: &[Event], pulses: &[Pulse]) -> Result<Vec<Vec<Pos>>> {
    let mut s = Schedule::from_events(ctx.design.clone(), evs);
    s.phase = ctx.phase;
    let (d, open_ev, close_ev, _) = s.realize(&BTreeMap::new())?;
    let door = *d.ports.get("door").context("design has no door port")?;
    let spec = crate::DoorSpec { door, lever: [0, 0, 0], along_z: d.meta.get("door-axis").is_some_and(|v| v == "z") };
    let mut blocks = Vec::new();
    for (p, st) in &d.blocks {
        blocks.push((*p, cs::convert_state(st).map_err(|e| anyhow::anyhow!("{st}: {e}"))?));
    }
    let (mut w, _, idle) = cs::place_and_settle(&blocks, &spec, 400);
    if !idle {
        bail!("the plan's design never settles");
    }
    let is_piston = |w: &redsim::World, q: Pos| matches!(w.get(cs::bp(q)), redsim::Block::Piston { .. });
    let mut out = vec![Vec::new(); pulses.len()];
    for (open, evs) in [(true, &open_ev), (false, &close_ev)] {
        let (mut t, mut i) = (0u32, 0usize);
        loop {
            for (k, p) in pulses.iter().enumerate() {
                if p.open == open && p.set == t {
                    for (dv, _) in DIRS {
                        let q = add(p.cell, dv);
                        if is_piston(&w, q) && !out[k].contains(&q) {
                            out[k].push(q);
                        }
                        let below = sub(q, [0, 1, 0]);
                        if q != p.cell && is_piston(&w, below) && !out[k].contains(&below) {
                            out[k].push(below);
                        }
                    }
                }
            }
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
            if i >= evs.len() {
                break;
            }
            w.tick();
            t += 1;
        }
        let _ = cs::step_until_idle(&mut w, 300, |_| {});
    }
    Ok(out)
}

/// Cells an output cell must be in to power piston p: p, the cell above it, or a block touching either.
fn feed_set(p: Pos) -> BTreeSet<Pos> {
    let above = add(p, [0, 1, 0]);
    let mut s = BTreeSet::from([p, above]);
    for base in [p, above] {
        for (d, _) in DIRS {
            s.insert(add(base, d));
        }
    }
    s
}

/// In-plane free cells of the design's box grown by `margin` (air at rest, not the lever).
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

/// Every shuttle placement whose observer can reach a fed piston of each pulse from one end.
fn placements(ctx: &Ctx, along_z: bool, door: Pos, o: &Options, evs: &[Event], fed: &[Vec<Pos>]) -> Vec<Place> {
    let used: BTreeSet<Pos> = evs.iter().map(|e| e.2).filter(|c| !o.targets.contains(c)).collect();
    let free: BTreeSet<Pos> = free_cells(&ctx.design, along_z, door, o.margin).into_iter().filter(|c| !used.contains(c)).collect();
    let plane: Vec<(Pos, &'static str)> = DIRS.iter().copied().filter(|(d, _)| if along_z { d[0] == 0 } else { d[2] == 0 }).collect();
    let normal: Pos = if along_z { [1, 0, 0] } else { [0, 0, 1] };
    let all_fed: Vec<Pos> = fed.iter().flatten().copied().collect();
    let near = |c: Pos| o.radius <= 0 || all_fed.is_empty() || all_fed.iter().any(|p| (0..3).map(|i| (p[i] - c[i]).abs()).max().unwrap_or(0) <= o.radius);
    let sets: Vec<BTreeSet<Pos>> = fed.iter().map(|ps| ps.iter().flat_map(|p| feed_set(*p)).collect()).collect();
    let mut out = Vec::new();
    for &s in &free {
        for &(d, dn) in &plane {
            let (oc, land, input) = (add(s, d), add(s, add(d, d)), add(s, normal));
            if !free.contains(&oc) || !free.contains(&land) || used.contains(&input) || ctx.design.blocks.contains_key(&input) {
                continue;
            }
            if !(near(s) && near(oc) && near(land)) {
                continue;
            }
            if o.payloads.iter().any(|p| p == "redstone_block") {
                // a redstone block powers what touches it (and QC-powers the piston below an
                // empty neighbour): the landing cell's neighbours and their cells below
                let reach: BTreeSet<Pos> = DIRS.iter().flat_map(|(dv, _)| [add(land, *dv), sub(add(land, *dv), [0, 1, 0])]).collect();
                let hit = |ps: &Vec<Pos>| !ps.is_empty() && ps.iter().any(|p| reach.contains(p));
                let ok = if o.partial { fed.iter().any(hit) } else { fed.iter().all(|ps| ps.is_empty() || hit(ps)) };
                if ok {
                    out.push(Place { s, sdir: dn, o: oc, odir: d_none(), rb: true, land, input });
                }
            }
            if !o.payloads.iter().any(|p| p == "observer") {
                continue;
            }
            for &(f, fname) in &plane {
                let (rest_out, land_out) = (sub(oc, f), sub(land, f));
                let hit = |st: &BTreeSet<Pos>| !st.is_empty() && (st.contains(&rest_out) || st.contains(&land_out));
                let ok = if o.partial { sets.iter().any(hit) } else { sets.iter().all(|st| st.is_empty() || hit(st)) };
                if ok {
                    out.push(Place { s, sdir: dn, o: oc, odir: (f, fname), rb: false, land, input });
                }
            }
        }
    }
    out
}

/// Every option for one pulse (a carried redstone block takes its own leads).
fn options(o: &Options, rb: bool) -> Vec<Opt> {
    let mut v = Vec::new();
    for &lead in if rb { &o.rb_leads } else { &o.leads } {
        for mode in [Mode::Arrive, Mode::Return] {
            for &hold in &o.holds {
                v.push(Opt { mode, hold, lead });
            }
        }
    }
    v
}

/// Stage 1: for each placement and target pulse, the options that work with only that pulse replaced.
fn stage1(ctx: &Ctx, evs: &[Event], pulses: &[Pulse], places: &[Place], o: &Options, screen: &[Pos]) -> Vec<Vec<Vec<Opt>>> {
    let mut jobs: Vec<(usize, usize, Opt)> = Vec::new();
    for (pi, pl) in places.iter().enumerate() {
        let opts = options(o, pl.rb);
        for (ki, p) in pulses.iter().enumerate() {
            for op in &opts {
                if op.window(p.set).is_some() {
                    jobs.push((pi, ki, *op));
                }
            }
        }
    }
    let res = cs::par_map(&jobs, o.threads, |(pi, ki, op)| {
        let p = pulses[*ki];
        let (s, c) = op.window(p.set).unwrap();
        let e = with_windows(&without(evs, &[p]), places[*pi].input, &vec![(p.open, s, c)]);
        ctx.judge(&e, &places[*pi].edits(), screen).is_ok()
    });
    let mut out = vec![vec![Vec::new(); pulses.len()]; places.len()];
    for ((pi, ki, op), ok) in jobs.iter().zip(res) {
        if ok {
            out[*pi][*ki].push(*op);
        }
    }
    out
}

/// The combinations of one placement's per-pulse options (at most `cap`, disjoint S pulses).
fn combos(pulses: &[Pulse], per: &[Vec<Opt>], cap: usize) -> Vec<(Vec<Opt>, Windows)> {
    let mut out: Vec<(Vec<Opt>, Windows)> = Vec::new();
    let mut idx = vec![0usize; per.len()];
    if per.iter().any(|v| v.is_empty()) {
        return out;
    }
    loop {
        let pick: Vec<Opt> = idx.iter().enumerate().map(|(k, &i)| per[k][i]).collect();
        let w: Windows = pick.iter().zip(pulses).map(|(op, p)| {
            let (s, c) = op.window(p.set).unwrap();
            (p.open, s, c)
        }).collect();
        if disjoint(&w) {
            out.push((pick, w));
            if out.len() >= cap {
                return out;
            }
        }
        let mut k = 0;
        loop {
            if k == idx.len() {
                return out;
            }
            idx[k] += 1;
            if idx[k] < per[k].len() {
                break;
            }
            idx[k] = 0;
            k += 1;
        }
    }
}

/// A working shuttle: placement, per-pulse options, S's pulses, the plan it runs, its ticks.
#[derive(Clone)]
struct Found {
    place: Place,
    /// The target pulses this shuttle replaces (all of them unless `--partial`), and how.
    replaced: Vec<Pulse>,
    picks: Vec<Opt>,
    windows: Windows,
    evs: Vec<Event>,
    ticks: (u32, u32),
    absorbed: Vec<String>,
}

/// Stage 2: per placement, the first combination that works with every target pulse replaced.
fn stage2(ctx: &Ctx, evs: &[Event], pulses: &[Pulse], places: &[Place], per: &[Vec<Vec<Opt>>], screen: &[Pos], o: &Options) -> Vec<Found> {
    let base = without(evs, pulses);
    let jobs: Vec<usize> = (0..places.len()).filter(|&pi| per[pi].iter().all(|v| !v.is_empty())).collect();
    let res = cs::par_map(&jobs, o.threads, |&pi| {
        for (picks, w) in combos(pulses, &per[pi], o.max_combos) {
            let e = with_windows(&base, places[pi].input, &w);
            if let Ok(t) = ctx.judge(&e, &places[pi].edits(), screen) {
                return Some(Found { place: places[pi].clone(), replaced: pulses.to_vec(), picks, windows: w, evs: e, ticks: t, absorbed: Vec::new() });
            }
        }
        None
    });
    res.into_iter().flatten().collect()
}

/// `--partial`: per placement, a large subset of the target pulses it can replace (the rest kept
/// ideal), grown greedily: each pulse with options is added if the shuttle still works with it.
fn stage2_partial(ctx: &Ctx, evs: &[Event], pulses: &[Pulse], places: &[Place], per: &[Vec<Vec<Opt>>], screen: &[Pos], o: &Options) -> Vec<Found> {
    let jobs: Vec<usize> = (0..places.len()).filter(|&pi| per[pi].iter().any(|v| !v.is_empty())).collect();
    let res = cs::par_map(&jobs, o.threads, |&pi| {
        let mut best: Option<Found> = None;
        let mut chosen: Vec<usize> = Vec::new();
        for i in (0..pulses.len()).filter(|&i| !per[pi][i].is_empty()) {
            let mut sub = chosen.clone();
            sub.push(i);
            let sp: Vec<Pulse> = sub.iter().map(|&j| pulses[j]).collect();
            let sper: Vec<Vec<Opt>> = sub.iter().map(|&j| per[pi][j].clone()).collect();
            let base = without(evs, &sp);
            for (picks, w) in combos(&sp, &sper, o.max_combos) {
                let e = with_windows(&base, places[pi].input, &w);
                if let Ok(t) = ctx.judge(&e, &places[pi].edits(), screen) {
                    best = Some(Found { place: places[pi].clone(), replaced: sp, picks, windows: w, evs: e, ticks: t, absorbed: Vec::new() });
                    chosen = sub;
                    break;
                }
            }
        }
        best
    });
    res.into_iter().flatten().collect()
}

/// The plan's pulses (target cells excepted) it still works without, each dropped alone.
fn spare_pulses(ctx: &Ctx, evs: &[Event], targets: &[Pos], screen: &[Pos], threads: usize) -> Vec<Pulse> {
    let mut cells: Vec<Pos> = evs.iter().filter(|e| e.3.as_deref() != Some("@use")).map(|e| e.2).collect();
    cells.sort();
    cells.dedup();
    cells.retain(|c| !targets.contains(c));
    let all = pulses_of(evs, &cells);
    let res = cs::par_map(&all, threads, |q| ctx.judge(&without(evs, &[*q]), &[], screen).is_ok());
    all.into_iter().zip(res).filter(|(_, ok)| *ok).map(|(q, _)| q).collect()
}

/// The plan tick O's other end stands for (its free pulse) for each S pulse.
fn free_ticks(picks: &[Opt], windows: &Windows) -> Vec<(bool, u32)> {
    picks.iter().zip(windows).map(|(op, &(open, s, c))| (open, if op.mode == Mode::Arrive { c + op.lead } else { s + op.lead })).collect()
}

/// `--absorb`: drop other plan pulses that a free O pulse gives already, or that an extra S pulse
/// can give (greedy, earliest first), keeping the plan working at the screening positions. Pulses
/// the plan works without anyway (`spare`) are skipped.
fn absorb(ctx: &Ctx, f: &mut Found, o: &Options, opts: &[Opt], screen: &[Pos], spare: &[Pulse]) {
    let mut cells: Vec<Pos> = f.evs.iter().filter(|e| e.3.as_deref() != Some("@use")).map(|e| e.2).collect();
    cells.sort();
    cells.dedup();
    cells.retain(|c| *c != f.place.input);
    let cands: Vec<Pulse> = pulses_of(&f.evs, &cells).into_iter().filter(|q| !spare.contains(q)).collect();
    let edits = f.place.edits();
    for q in cands {
        let free = free_ticks(&f.picks, &f.windows);
        if free.iter().any(|&(open, t)| open == q.open && (t as i64 - q.set as i64).abs() <= 1) {
            let e = without(&f.evs, &[q]);
            if let Ok(t) = ctx.judge(&e, &edits, screen) {
                f.absorbed.push(format!("{:?} {} t{} (a free O pulse)", q.cell, if q.open { "open" } else { "close" }, q.set));
                f.evs = e;
                f.ticks = t;
                continue;
            }
        }
        let tries: Vec<(Opt, (bool, u32, u32))> = opts
            .iter()
            .filter_map(|op| op.window(q.set).map(|(s, c)| (*op, (q.open, s, c))))
            .filter(|(_, w)| {
                let mut all = f.windows.clone();
                all.push(*w);
                disjoint(&all)
            })
            .collect();
        let base = without(&f.evs, &[q]);
        let res = cs::par_map(&tries, o.threads, |(_, w)| {
            let e = with_windows(&base, f.place.input, &vec![*w]);
            ctx.judge(&e, &edits, screen).ok().map(|t| (e, t))
        });
        if let Some(((op, w), Some((e, t)))) = tries.iter().zip(res).find(|(_, r)| r.is_some()) {
            f.absorbed.push(format!("{:?} {} t{} (an extra S pulse, {})", q.cell, if q.open { "open" } else { "close" }, q.set, op.label()));
            f.picks.push(*op);
            f.windows.push(*w);
            f.evs = e;
            f.ticks = t;
        }
    }
}

fn write_out(ctx: &Ctx, f: &Found, out: &Path, header: &[String]) -> Result<()> {
    let mut d = ctx.design.clone();
    for (p, st) in f.place.edits() {
        if let Some(s) = st {
            d.blocks.insert(p, s);
        }
    }
    d.name = format!("{}-{}", d.name, out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
    let design_path = out.with_extension("txt");
    d.save(&design_path)?;
    let mut lines: Vec<String> = header.iter().map(|h| format!("# {h}")).collect();
    lines.push(format!("design {}", design_path.display()));
    lines.push(format!("phase {}", if ctx.phase == crate::ideal::PlanPhase::Between { "between" } else { "tile" }));
    let mut evs = f.evs.clone();
    evs.sort_by_key(|e| (!e.0, e.1, e.2));
    for (open, t, q, st) in &evs {
        let ph = if *open { "open" } else { "close" };
        lines.push(match st.as_deref() {
            Some("@use") => format!("{ph} {t} use {} {} {}", q[0], q[1], q[2]),
            Some(b) => format!("{ph} {t} set {} {} {} {b}", q[0], q[1], q[2]),
            None => format!("{ph} {t} clear {} {} {}", q[0], q[1], q[2]),
        });
    }
    std::fs::write(out.with_extension("ideal"), lines.join("\n") + "\n")?;
    Ok(())
}

fn describe(f: &Found) -> Vec<String> {
    let mut v = vec![f.place.label()];
    let parts: Vec<String> = f.replaced.iter().zip(&f.picks).map(|(p, op)| format!("{:?} {} t{} <- {}", p.cell, if p.open { "open" } else { "close" }, p.set, op.label())).collect();
    v.push(format!("replaced {} pulses: {}", f.replaced.len(), parts.join(", ")));
    let w: Vec<String> = f.windows.iter().map(|(open, s, c)| format!("{} {s}-{c}", if *open { "open" } else { "close" })).collect();
    v.push(format!("S input {:?} pulses: {}", f.place.input, w.join(", ")));
    for a in &f.absorbed {
        v.push(format!("absorbed {a}"));
    }
    v
}

/// The input cells of the firings `door feeds` finds no option for (its `: NONE` lines).
fn unfed_inputs(text: &str, label: &str) -> Result<Vec<Pos>> {
    let r = crate::feeds::analyze(text, label, None)?;
    let mut out: Vec<Pos> = Vec::new();
    for l in r.text.iter().filter(|l| l.contains(": NONE")) {
        let Some(rest) = l.split(" input (").nth(1) else { continue };
        let Some(cell) = rest.split(')').next() else { continue };
        let v: Vec<i32> = cell.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        if v.len() == 3 && !out.contains(&[v[0], v[1], v[2]]) {
            out.push([v[0], v[1], v[2]]);
        }
    }
    Ok(out)
}

pub fn cli(paths: &[PathBuf], o: &Options, per_cell: bool) -> Result<()> {
    let quiet = paths.len() > 1;
    for path in paths {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut targets = o.targets.clone();
        if o.unfed {
            match unfed_inputs(&text, &path.display().to_string()) {
                Ok(v) => targets.extend(v.into_iter().filter(|c| !o.targets.contains(c))),
                Err(e) => {
                    println!("{}: door feeds fails: {e:#}", path.display());
                    continue;
                }
            }
            if !quiet {
                println!("unfed inputs (door feeds: no option): {targets:?}");
            }
        }
        let groups: Vec<Vec<Pos>> = if per_cell { targets.iter().map(|c| vec![*c]).collect() } else { vec![targets.clone()] };
        for g in groups.into_iter().filter(|g| !g.is_empty()) {
            let tag = if per_cell { format!("-{}_{}_{}", g[0][0], g[0][1], g[0][2]) } else { String::new() };
            let og = Options { targets: g.clone(), ..o.clone() };
            match run_one(path, &text, &og, &tag, quiet) {
                Ok(n) if quiet || per_cell => println!("{} target {g:?}: {n} shuttles WORK", path.display()),
                Ok(_) => {}
                Err(e) => println!("{} target {g:?}: {e:#}", path.display()),
            }
        }
    }
    Ok(())
}

/// One search: the shuttles that replace every pulse of `o.targets` in the plan at `path`; returns
/// how many WORK at the full positions (printed unless `quiet`, written to `--out-dir`).
fn run_one(path: &Path, text: &str, o: &Options, tag: &str, quiet: bool) -> Result<usize> {
    let s = Schedule::parse(text)?;
    let say = |m: String| {
        if !quiet {
            println!("{m}");
        }
    };
    let design = s.design().clone();
    let door = *design.ports.get("door").context("design has no door port")?;
    let along_z = design.meta.get("door-axis").is_some_and(|v| v == "z");
    let ctx = Ctx { design, phase: s.phase, cycles: o.cycles };
    let mut evs: Vec<Event> = Vec::new();
    for open in [true, false] {
        for (t, p, st) in s.resolved(open)? {
            evs.push((open, t, p, st));
        }
    }
    let (screen, full) = (crate::ideal::offsets(0), crate::ideal::offsets(o.extra_offsets));
    let base = ctx.judge(&evs, &[], &screen).map_err(|e| anyhow::anyhow!("the plan itself fails: {e}"))?;
    let pulses: Vec<Pulse> = pulses_of(&evs, &o.targets).into_iter().filter(|p| o.edge.is_none_or(|e| e == p.open)).collect();
    anyhow::ensure!(!pulses.is_empty(), "no set/clear pulses on the target cells {:?} (a level held across a rest is not a pulse)", o.targets);
    // target pulses the plan does not need at all: dropped with no shuttle
    let need = cs::par_map(&pulses, o.threads, |p| ctx.judge(&without(&evs, &[*p]), &[], &screen).is_err());
    let spare_t: Vec<Pulse> = pulses.iter().zip(&need).filter(|(_, n)| !**n).map(|(p, _)| *p).collect();
    if !spare_t.is_empty() {
        let s: Vec<String> = spare_t.iter().map(|q| format!("{:?} {} t{}", q.cell, if q.open { "open" } else { "close" }, q.set)).collect();
        println!("{}{}target pulses the plan works without (each alone, no shuttle): {}", if quiet { path.display().to_string() } else { String::new() }, if quiet { " " } else { "" }, s.join(", "));
        // the largest set of them it works without together (greedy, in order); with `--train`
        // a tick only counts when both edges' copies go together (one train feeds both edges)
        let mut joint: Vec<Pulse> = Vec::new();
        let groups: Vec<Vec<Pulse>> = if o.train {
            let mut g: Vec<Vec<Pulse>> = Vec::new();
            for q in spare_t.iter().filter(|q| q.open) {
                if let Some(c) = spare_t.iter().find(|c| !c.open && c.cell == q.cell && c.set == q.set) {
                    g.push(vec![*q, *c]);
                }
            }
            g
        } else {
            spare_t.iter().map(|q| vec![*q]).collect()
        };
        for grp in &groups {
            let mut t = joint.clone();
            t.extend(grp.iter().copied());
            if ctx.judge(&without(&evs, &t), &[], &screen).is_ok() {
                joint = t;
            }
        }
        let s: Vec<String> = joint.iter().map(|q| format!("{:?} {} t{}", q.cell, if q.open { "open" } else { "close" }, q.set)).collect();
        let ok_full = ctx.judge(&without(&evs, &joint), &[], &full).is_ok();
        println!("  dropped together ({}): {} [{}]", joint.len(), s.join(", "), if ok_full { format!("WORKS at {} positions", full.len()) } else { "fails at the full positions".into() });
    }
    let pulses: Vec<Pulse> = pulses.into_iter().zip(need).filter(|(_, n)| *n).map(|(p, _)| p).collect();
    anyhow::ensure!(!pulses.is_empty(), "every target pulse is spare: no shuttle needed");
    let fed = fed_pistons(&ctx, &evs, &pulses)?;
    say(format!("{}: plan open {} / close {}; {} target pulses:", path.display(), base.0, base.1, pulses.len()));
    for (p, f) in pulses.iter().zip(&fed) {
        say(format!("  {} t{}-{} at {:?} -> pistons {:?}", if p.open { "open" } else { "close" }, p.set, p.clear, p.cell, f));
    }
    let places = placements(&ctx, along_z, door, o, &evs, &fed);
    let per = stage1(&ctx, &evs, &pulses, &places, o, &screen);
    let covered = per.iter().filter(|v| v.iter().all(|x| !x.is_empty())).count();
    let rbs = places.iter().filter(|p| p.rb).count();
    say(format!("{} placements reach every target pulse ({rbs} with a redstone block); {covered} give each pulse alone ({} options per pulse)", places.len(), options(o, false).len()));
    let by_pulse: Vec<String> = pulses.iter().enumerate().map(|(k, p)| format!("{} t{} {}", if p.open { "open" } else { "close" }, p.set, per.iter().filter(|v| !v[k].is_empty()).count())).collect();
    say(format!("  placements giving each pulse alone: {}", by_pulse.join(", ")));
    let mut found = if o.partial { stage2_partial(&ctx, &evs, &pulses, &places, &per, &screen, o) } else { stage2(&ctx, &evs, &pulses, &places, &per, &screen, o) };
    // a partial set the plan works without anyway (jointly spare) needs no shuttle
    found.retain(|f| f.replaced.len() < 2 || ctx.judge(&without(&evs, &f.replaced), &[], &screen).is_err());
    found.sort_by_key(|f| (std::cmp::Reverse(f.replaced.len()), f.windows.len(), f.ticks.0 + f.ticks.1));
    let spare = if o.absorb && !found.is_empty() { spare_pulses(&ctx, &evs, &o.targets, &screen, o.threads) } else { Vec::new() };
    if !spare.is_empty() {
        let s: Vec<String> = spare.iter().map(|q| format!("{:?} {} t{}", q.cell, if q.open { "open" } else { "close" }, q.set)).collect();
        say(format!("pulses the plan works without anyway (not absorbed): {}", s.join(", ")));
    }
    let mut done: Vec<Found> = Vec::new();
    for mut f in found {
        if done.len() >= o.top {
            break;
        }
        if o.absorb {
            let opts = options(o, f.place.rb);
            absorb(&ctx, &mut f, o, &opts, &screen, &spare);
        }
        match ctx.judge(&f.evs, &f.place.edits(), &full) {
            Ok(t) => {
                f.ticks = t;
                done.push(f);
            }
            Err(e) => say(format!("  fails at the full positions: {}: {}", f.place.label(), e.chars().take(160).collect::<String>())),
        }
    }
    say(format!("{} shuttles WORK at {} positions x {} cycles:", done.len(), full.len(), o.cycles));
    for (k, f) in done.iter().enumerate() {
        let lines = describe(f);
        println!("{}{}#{k}: open {} / close {}: {}", if quiet { path.display().to_string() } else { String::new() }, if quiet { " " } else { "" }, f.ticks.0, f.ticks.1, lines.join("; "));
        if let Some(dir) = &o.out_dir {
            std::fs::create_dir_all(dir)?;
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let out = dir.join(format!("{stem}{tag}-sh{k}"));
            let mut header = vec![format!("door shuttle {}: target inputs {:?} fed by a shuttled observer; WORKS at {} positions x {} cycles, open {} / close {}", path.display(), o.targets, full.len(), o.cycles, f.ticks.0, f.ticks.1)];
            header.extend(lines);
            write_out(&ctx, f, &out, &header)?;
            println!("    wrote {}.ideal + .txt", out.display());
        }
    }
    Ok(done.len())
}
