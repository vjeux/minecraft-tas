//! `mctas door interleave <plan.ideal>`: ONE pulse train for both lever edges (track pext). A
//! mechanism's plan opens with one sequence of input pulses and closes with another; wiring that
//! tells the two edges apart needs the lever's state everywhere. If one sequence of steps can do
//! both, every input fires identically on both flips, and a single observer chain started by the
//! lever (an observer watching it fires on either flip) delivers it: the piston extenders'
//! interleaved schedules, applied to doors. A step of the open plan may fire during the closing
//! (and vice versa) only where it is HARMLESS there: the same input cells pulsed on the other
//! edge's current state change nothing (the cell is empty, holds a block, or a piston that
//! cannot move). Breadth-first over (open steps done, close steps done): each state's two worlds
//! are fixed by the counts, since harmless steps change nothing; a step common to both plans
//! fires once for both. The merged train is then judged as a plan with the same events on both
//! edges (`door ideal-rs` checks, the judge's positions).

use crate::cosearch as cs;
use crate::ideal::Schedule;
use crate::ideal_rs;
use anyhow::{Context, Result, bail};
use mctas::sim::Pos;
use redsim::{Block, World};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;

/// `println!` unless the run is quiet (batch mode prints one summary line per plan).
macro_rules! say {
    ($o:expr, $($t:tt)*) => {
        if !$o.quiet {
            println!($($t)*);
        }
    };
}

/// One step: the input events of one tick group of a plan, ticks relative to the group's start.
#[derive(Clone, Debug, PartialEq)]
struct Step {
    evs: Vec<(u32, Pos, Option<String>)>,
}

impl Step {
    fn label(&self) -> String {
        let mut cells: Vec<String> = self.evs.iter().filter(|e| e.2.is_some()).map(|e| format!("{},{},{}", e.1[0], e.1[1], e.1[2])).collect();
        cells.dedup();
        format!("[{}]", cells.join(" "))
    }
    fn len(&self) -> u32 {
        self.evs.iter().map(|e| e.0).max().unwrap_or(0)
    }
}

/// The plan's tick groups: events whose set falls on one tick start a step; a clear belongs to the
/// step that set its cell. With `split`, every input cell of a group is a step of its own.
fn steps(evs: &[(u32, Pos, Option<String>)], split: bool) -> Vec<Step> {
    let groups = steps_grouped(evs);
    if !split {
        return groups;
    }
    let mut out = Vec::new();
    for g in groups {
        let mut cells: Vec<Pos> = g.evs.iter().map(|e| e.1).collect();
        cells.dedup();
        let mut seen = Vec::new();
        for c in cells {
            if seen.contains(&c) {
                continue;
            }
            seen.push(c);
            out.push(Step { evs: g.evs.iter().filter(|e| e.1 == c).cloned().collect() });
        }
    }
    out
}

fn steps_grouped(evs: &[(u32, Pos, Option<String>)]) -> Vec<Step> {
    let mut evs = evs.to_vec();
    evs.sort_by_key(|e| e.0);
    let mut out: Vec<(u32, Step)> = Vec::new();
    let mut owner: HashMap<Pos, usize> = HashMap::new();
    for (t, p, st) in evs {
        match &st {
            Some(_) => {
                let i = match out.iter().position(|(t0, _)| *t0 == t) {
                    Some(i) => i,
                    None => {
                        out.push((t, Step { evs: Vec::new() }));
                        out.len() - 1
                    }
                };
                let t0 = out[i].0;
                out[i].1.evs.push((t - t0, p, st));
                owner.insert(p, i);
            }
            None => {
                if let Some(&i) = owner.get(&p) {
                    let t0 = out[i].0;
                    out[i].1.evs.push((t - t0, p, None));
                }
            }
        }
    }
    out.into_iter().map(|(_, s)| s).collect()
}

struct Sim {
    lo: Pos,
    hi: Pos,
}

impl Sim {
    /// Apply one step to a world and run until idle.
    fn apply(&self, w: &World, s: &Step) -> std::result::Result<World, String> {
        let mut w = w.clone();
        let mut evs = s.evs.clone();
        evs.sort_by_key(|e| e.0);
        let mut t = 0u32;
        let mut i = 0;
        while i < evs.len() {
            while i < evs.len() && evs[i].0 == t {
                let (_, q, st) = &evs[i];
                let b = match st {
                    Some(x) => cs::convert_state(x)?,
                    None => Block::Air,
                };
                w.set_block(cs::bp(*q), b, redsim::world::flags::ALL);
                i += 1;
            }
            if i < evs.len() {
                w.tick();
                t += 1;
            }
        }
        let (_, idle) = cs::step_until_idle(&mut w, 300, |_| {});
        if !idle {
            return Err("never idle".into());
        }
        if !w.errors.is_empty() || !w.drops.is_empty() {
            return Err("broke".into());
        }
        Ok(w)
    }
    fn snap(&self, w: &World) -> cs::Snap {
        cs::Snap::take(w, self.lo, self.hi)
    }
}

/// An idle world as its non-air cells (index into the watch box, state): idle worlds have no
/// pending ticks or moving blocks, so placing these raw rebuilds the world exactly.
type Sparse = Vec<(u32, Block)>;

impl Sim {
    fn sparse(&self, w: &World) -> Sparse {
        self.snap(w).cells.iter().enumerate().filter(|(_, b)| **b != Block::Air).map(|(i, b)| (i as u32, *b)).collect()
    }
    fn rebuild(&self, s: &Sparse) -> World {
        let mut w = cs::world_for(self.lo, self.hi);
        let size = [self.hi[0] - self.lo[0] + 1, self.hi[1] - self.lo[1] + 1, self.hi[2] - self.lo[2] + 1];
        for (i, b) in s {
            let i = *i as i32;
            let x = i % size[0];
            let z = (i / size[0]) % size[2];
            let y = i / (size[0] * size[2]);
            w.set_block(cs::bp([self.lo[0] + x, self.lo[1] + y, self.lo[2] + z]), *b, redsim::world::flags::RAW);
        }
        w
    }
}

/// The train as a plan: pulse `k` (2 gt) at tick `times[k]` on both edges.
fn train_plan(d: &mctas::design::Design, phase: crate::ideal::PlanPhase, train: &[Pos], times: &[u32]) -> (Schedule, Vec<(bool, u32, Pos, Option<String>)>) {
    let mut all: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
    let mut seen: Vec<(Pos, u32)> = Vec::new();
    for (c, t) in train.iter().zip(times) {
        if seen.contains(&(*c, *t)) {
            continue;
        }
        seen.push((*c, *t));
        for open in [true, false] {
            all.push((open, *t, *c, Some("minecraft:redstone_block".into())));
            all.push((open, *t + 2, *c, None));
        }
    }
    let mut plan = Schedule::from_events(d.clone(), &all);
    plan.phase = phase;
    (plan, all)
}

/// Pull every pulse of a working train as early as it goes, on BOTH edges together (unlike
/// `ideal-rs --compact`, which moves each edge's groups on their own), two passes.
fn compact_train(d: &mctas::design::Design, phase: crate::ideal::PlanPhase, train: &[Pos], times: &mut [u32], cycles: u32) {
    let screen = crate::ideal::offsets(0);
    let works = |t: &[u32]| ideal_rs::run_edited(&train_plan(d, phase, train, t).0, &BTreeMap::new(), &[], &screen, cycles, false).is_ok();
    for _ in 0..2 {
        for k in 0..times.len() {
            let orig = times[k];
            let mut best = orig;
            let mut t = orig;
            while t > 0 {
                t -= 1;
                times[k] = t;
                if works(times) {
                    best = t;
                }
            }
            times[k] = best;
        }
    }
}

/// `--search`: weighted A* over pairs of idle worlds (the closed door opening, the open door
/// closing) with ANY input cell pulsed (2 gt) on both at once, from (rest, open) to (open, rest):
/// a train not tied to the plan's own steps. Returns the cells pulsed, in order (or the pairs
/// searched and the best distance reached: cells off the two targets).
fn search_pairs(sim: &Sim, w0: &World, wopen: &World, cells: &[Pos], cap: usize, weight: usize) -> (Option<Vec<Pos>>, usize, usize) {
    use std::collections::{BinaryHeap, HashSet};
    use std::hash::{Hash, Hasher};
    let target_open = sim.sparse(wopen);
    let target_rest = sim.sparse(w0);
    let diff = |a: &Sparse, b: &Sparse| {
        let ma: HashMap<u32, Block> = a.iter().copied().collect();
        let mb: HashMap<u32, Block> = b.iter().copied().collect();
        let mut n = ma.iter().filter(|(i, x)| mb.get(i) != Some(x)).count();
        n += mb.keys().filter(|i| !ma.contains_key(i)).count();
        n
    };
    let key = |a: &Sparse, b: &Sparse| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        a.hash(&mut h);
        0xffff_ffffu32.hash(&mut h);
        b.hash(&mut h);
        h.finish()
    };
    let mut seen: HashSet<u64> = HashSet::new();
    // (open-edge world, close-edge world, parent, cell pulsed, steps)
    let mut nodes: Vec<(Sparse, Sparse, Option<usize>, Pos, usize)> = vec![(target_rest.clone(), target_open.clone(), None, [0, 0, 0], 0)];
    let h0 = diff(&target_rest, &target_open) * 2;
    let mut heap = BinaryHeap::new();
    heap.push((std::cmp::Reverse(h0 * weight), 0usize));
    seen.insert(key(&target_rest, &target_open));
    let mut best = h0;
    while let Some((_, id)) = heap.pop() {
        let (a, b, _, _, g) = nodes[id].clone();
        if a == target_open && b == target_rest {
            let mut out = Vec::new();
            let mut c = id;
            while let Some(p) = nodes[c].2 {
                out.push(nodes[c].3);
                c = p;
            }
            out.reverse();
            return (Some(out), nodes.len(), 0);
        }
        if nodes.len() >= cap {
            break;
        }
        let (wa, wb) = (sim.rebuild(&a), sim.rebuild(&b));
        for &c in cells {
            let st = Step { evs: vec![(0, c, Some("minecraft:redstone_block".into())), (2, c, None)] };
            let (Ok(a2), Ok(b2)) = (sim.apply(&wa, &st), sim.apply(&wb, &st)) else { continue };
            let (sa, sb) = (sim.sparse(&a2), sim.sparse(&b2));
            let k = key(&sa, &sb);
            if !seen.insert(k) {
                continue;
            }
            let h2 = diff(&sa, &target_open) + diff(&sb, &target_rest);
            best = best.min(h2);
            nodes.push((sa, sb, Some(id), c, g + 1));
            heap.push((std::cmp::Reverse(g + 1 + h2 * weight), nodes.len() - 1));
        }
    }
    (None, nodes.len(), best)
}

pub struct Options {
    pub extra_offsets: u32,
    pub cycles: u32,
    /// Ticks between two steps of the merged train.
    pub gap: u32,
    pub out: Option<std::path::PathBuf>,
    /// One step per input cell instead of per tick group.
    pub split: bool,
    /// Best-first search over any input cell pulses (not the plan's steps), up to this many
    /// world pairs.
    pub search: Option<usize>,
    /// The heuristic's weight in the search (A* with f = steps + weight * cells off).
    pub weight: usize,
    /// Print nothing but the summary (batch mode).
    pub quiet: bool,
}

/// Several plans at once (in parallel), one summary line each; `out_dir` gets
/// `<stem>-train.ideal` for each plan with a train.
pub fn batch(paths: &[std::path::PathBuf], o: &Options, out_dir: Option<&Path>, threads: usize) -> Result<()> {
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(dir)?;
    }
    let res = cs::par_map(paths, threads, |p| {
        let out = out_dir.map(|d| d.join(format!("{}-train.ideal", p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default())));
        run(p, o, out.as_deref())
    });
    for (p, r) in paths.iter().zip(res) {
        match r {
            Ok(s) => println!("{}: {s}", p.display()),
            Err(e) => println!("{}: error: {e:#}", p.display()),
        }
    }
    Ok(())
}

pub fn cli(path: &Path, o: &Options) -> Result<()> {
    let s = run(path, o, o.out.as_deref())?;
    println!("{s}");
    Ok(())
}

/// Write a plan file: a header comment, the source plan's `design` line, its phase, the events.
fn write_plan(out: &Path, header: &str, text: &str, phase: crate::ideal::PlanPhase, all: &mut [(bool, u32, Pos, Option<String>)]) -> Result<()> {
    let mut lines = vec![format!("# {header}")];
    if let Some(l) = text.lines().find(|l| l.trim_start().starts_with("design ")) {
        lines.push(l.to_string());
    }
    lines.push(format!("phase {}", if phase == crate::ideal::PlanPhase::Between { "between" } else { "tile" }));
    all.sort_by_key(|e| (!e.0, e.1));
    for (open, t, p, st) in all.iter() {
        let ph = if *open { "open" } else { "close" };
        lines.push(match st {
            Some(b) => format!("{ph} {t} set {} {} {} {b}", p[0], p[1], p[2]),
            None => format!("{ph} {t} clear {} {} {}", p[0], p[1], p[2]),
        });
    }
    std::fs::write(out, lines.join("\n") + "\n")?;
    println!("wrote {}", out.display());
    Ok(())
}

/// A train found by `--search`: judge it timed (one 2 gt pulse every `spacing` ticks, the same on
/// both edges), compact it on both edges together, print each input's ticks, write it (to `out`).
/// Returns a one-line summary.
fn timed_train(d: &mctas::design::Design, phase: crate::ideal::PlanPhase, text: &str, path: &Path, train: &[Pos], n: usize, o: &Options, out: Option<&Path>) -> Result<String> {
    let shown: Vec<String> = train.iter().map(|c| format!("{},{},{}", c[0], c[1], c[2])).collect();
    say!(o, "TRAIN of {} single pulses for both edges ({n} pairs searched): {}", train.len(), shown.join(" "));
    let offsets = crate::ideal::offsets(o.extra_offsets);
    for spacing in [4u32, 6, 8, 10, 12] {
        let mut times: Vec<u32> = (0..train.len() as u32).map(|k| k * spacing).collect();
        let (plan, _) = train_plan(d, phase, train, &times);
        if let Err(e) = ideal_rs::run_edited(&plan, &BTreeMap::new(), &[], &offsets, o.cycles, false) {
            say!(o, "timed every {spacing} gt: FAILS: {}", e.chars().take(200).collect::<String>());
            continue;
        }
        say!(o, "WORKS timed every {spacing} gt ({} positions x {} cycles)", offsets.len(), o.cycles);
        compact_train(d, phase, train, &mut times, o.cycles);
        let (plan, mut all) = train_plan(d, phase, train, &times);
        let shown: Vec<String> = train.iter().zip(&times).map(|(c, t)| format!("t{t}:{},{},{}", c[0], c[1], c[2])).collect();
        say!(o, "compacted (both edges together): {}", shown.join(" "));
        let distinct = all.iter().filter(|e| e.0 && e.3.is_some()).count();
        let summary = match ideal_rs::run_edited(&plan, &BTreeMap::new(), &[], &offsets, o.cycles, false) {
            Ok((a, b)) => {
                let last = times.iter().max().unwrap_or(&0);
                say!(o, "WORKS compacted ({} positions x {} cycles): open {a} / close {b}, last pulse t{last}", offsets.len(), o.cycles);
                format!("ONE TRAIN: {distinct} pulses, last t{last}, open {a} / close {b} ({} positions)", offsets.len())
            }
            Err(e) => {
                say!(o, "compacted train fails at the full positions ({e}); keeping every {spacing} gt");
                times = (0..train.len() as u32).map(|k| k * spacing).collect();
                all = train_plan(d, phase, train, &times).1;
                format!("ONE TRAIN: {} pulses every {spacing} gt (compaction failed)", train.len())
            }
        };
        let mut per_cell: BTreeMap<Pos, Vec<u32>> = BTreeMap::new();
        for (c, t) in train.iter().zip(&times) {
            per_cell.entry(*c).or_default().push(*t);
        }
        for (c, ts) in &per_cell {
            say!(o, "  input {c:?}: pulses at {ts:?} on both edges");
        }
        if let Some(out) = out {
            let header = format!("door interleave --search {}: ONE train of {} pulses, the same events on both lever edges (found every {spacing} gt, then compacted on both edges together)", path.display(), train.len());
            write_plan(out, &header, text, phase, &mut all)?;
        }
        return Ok(summary);
    }
    Ok(format!("train of {} pulses found step by step but no timing works", train.len()))
}

fn run(path: &Path, o: &Options, out: Option<&Path>) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let s = Schedule::parse(&text)?;
    let (d, open_ev, close_ev, _) = s.realize(&BTreeMap::new())?;
    if open_ev.iter().chain(close_ev.iter()).any(|e| e.2.as_deref() == Some("@use")) {
        bail!("plans with lever uses are not handled");
    }
    let door = *d.ports.get("door").context("design has no door port")?;
    let spec = crate::DoorSpec { door, lever: [0, 0, 0], along_z: d.meta.get("door-axis").is_some_and(|v| v == "z") };
    let mut blocks = Vec::new();
    for (p, st) in &d.blocks {
        blocks.push((*p, cs::convert_state(st).map_err(|e| anyhow::anyhow!("{st}: {e}"))?));
    }
    let (w0, (lo, hi), idle) = cs::place_and_settle(&blocks, &spec, 400);
    if !idle {
        bail!("never settles");
    }
    let sim = Sim { lo, hi };
    let os = steps(&open_ev, o.split);
    let cl = steps(&close_ev, o.split);
    say!(o, "{}: open {} steps, close {} steps", path.display(), os.len(), cl.len());
    for (k, st) in os.iter().enumerate() {
        say!(o, "  O{} {} ({} gt)", k + 1, st.label(), st.len());
    }
    for (k, st) in cl.iter().enumerate() {
        say!(o, "  C{} {} ({} gt)", k + 1, st.label(), st.len());
    }
    // the open rest state: the open plan applied step by step
    let mut wopen = w0.clone();
    for st in &os {
        wopen = sim.apply(&wopen, st).map_err(|e| anyhow::anyhow!("open plan step by step: {e}"))?;
    }
    if let Some(cap) = o.search {
        let mut cells: Vec<Pos> = open_ev.iter().chain(close_ev.iter()).filter(|e| e.2.is_some()).map(|e| e.1).collect();
        cells.sort();
        cells.dedup();
        let (found, n, best) = search_pairs(&sim, &w0, &wopen, &cells, cap, o.weight);
        return match found {
            None => Ok(format!("NO train within {n} world pairs (best: {best} cells off the targets)")),
            Some(train) => timed_train(&d, s.phase, &text, path, &train, n, o, out),
        };
    }
    merge(&sim, &d, s.phase, &text, path, &os, &cl, &w0, &wopen, o, out)
}

/// The plan's own steps merged into one train (plain mode): breadth-first over (open steps done,
/// close steps done).
#[allow(clippy::too_many_arguments)]
fn merge(sim: &Sim, d: &mctas::design::Design, phase: crate::ideal::PlanPhase, text: &str, path: &Path, os: &[Step], cl: &[Step], w0: &World, wopen: &World, o: &Options, out: Option<&Path>) -> Result<String> {
    let start = (0usize, 0usize);
    let mut worlds: HashMap<(usize, usize), (World, World)> = HashMap::new();
    worlds.insert(start, (w0.clone(), wopen.clone()));
    let mut prev: HashMap<(usize, usize), ((usize, usize), char)> = HashMap::new();
    let mut q = VecDeque::new();
    q.push_back(start);
    let goal = (os.len(), cl.len());
    while let Some((i, j)) = q.pop_front() {
        if (i, j) == goal {
            break;
        }
        let (wa, wb) = worlds[&(i, j)].clone();
        let mut push = |n: (usize, usize), pair: (World, World), how: char, q: &mut VecDeque<(usize, usize)>| {
            if let std::collections::hash_map::Entry::Vacant(e) = worlds.entry(n) {
                e.insert(pair);
                prev.insert(n, ((i, j), how));
                q.push_back(n);
            }
        };
        // a common step
        if i < os.len() && j < cl.len() && os[i] == cl[j] {
            if let (Ok(a), Ok(b)) = (sim.apply(&wa, &os[i]), sim.apply(&wb, &cl[j])) {
                push((i + 1, j + 1), (a, b), 'B', &mut q);
            }
        }
        // the next open step, harmless on the closing world
        if i < os.len() {
            if let (Ok(a), Ok(b)) = (sim.apply(&wa, &os[i]), sim.apply(&wb, &os[i])) {
                if sim.snap(&b) == sim.snap(&wb) {
                    push((i + 1, j), (a, wb.clone()), 'O', &mut q);
                }
            }
        }
        // the next close step, harmless on the opening world
        if j < cl.len() {
            if let (Ok(a), Ok(b)) = (sim.apply(&wa, &cl[j]), sim.apply(&wb, &cl[j])) {
                if sim.snap(&a) == sim.snap(&wa) {
                    push((i, j + 1), (wa.clone(), b), 'C', &mut q);
                }
            }
        }
    }
    if !worlds.contains_key(&goal) {
        let best = worlds.keys().max_by_key(|(i, j)| i + j).copied().unwrap_or(start);
        return Ok(format!("NO single train from the plan's own steps: the furthest state is {} open + {} close steps ({} states)", best.0, best.1, worlds.len()));
    }
    let mut path_steps: Vec<char> = Vec::new();
    let mut c = goal;
    while let Some((p, how)) = prev.get(&c) {
        path_steps.push(*how);
        c = *p;
    }
    path_steps.reverse();
    // the merged train
    let (mut i, mut j) = (0usize, 0usize);
    let mut train: Vec<Step> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    for how in &path_steps {
        match how {
            'B' => {
                labels.push(format!("O{}=C{}", i + 1, j + 1));
                train.push(os[i].clone());
                i += 1;
                j += 1;
            }
            'O' => {
                labels.push(format!("O{}", i + 1));
                train.push(os[i].clone());
                i += 1;
            }
            _ => {
                labels.push(format!("C{}", j + 1));
                train.push(cl[j].clone());
                j += 1;
            }
        }
    }
    say!(o, "ONE TRAIN for both edges, {} steps: {}", train.len(), labels.join(" "));
    // judge it as a plan with the same events on both edges
    let mut all: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
    let mut t0 = 0u32;
    for st in &train {
        for (dt, p, stt) in &st.evs {
            for open in [true, false] {
                all.push((open, t0 + dt, *p, stt.clone()));
            }
        }
        t0 += st.len().max(2) + o.gap;
    }
    let mut plan = Schedule::from_events(d.clone(), &all);
    plan.phase = phase;
    let offsets = crate::ideal::offsets(o.extra_offsets);
    let summary = match ideal_rs::run_edited(&plan, &BTreeMap::new(), &[], &offsets, o.cycles, false) {
        Ok((a, b)) => format!("ONE TRAIN of the plan's own {} steps ({}): WORKS on both edges ({} positions x {} cycles): open {a} / close {b}", train.len(), labels.join(" "), offsets.len(), o.cycles),
        Err(e) => format!("ONE TRAIN of the plan's own {} steps ({}), but it FAILS timed with a {} gt gap: {e}", train.len(), labels.join(" "), o.gap),
    };
    if let Some(out) = out {
        let header = format!("door interleave {}: one train for both edges: {}", path.display(), labels.join(" "));
        write_plan(out, &header, text, phase, &mut all)?;
    }
    Ok(summary)
}
