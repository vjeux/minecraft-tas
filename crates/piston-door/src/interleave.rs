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
/// `edge`: 0 = fires on both lever edges (the train's normal move), 1 = only while opening, 2 =
/// only while closing (`--edge-cost`: a pulse wiring must gate on the lever's state).
#[derive(Clone, Debug, PartialEq, Default)]
struct Step {
    evs: Vec<(u32, Pos, Option<String>)>,
    edge: u8,
}

impl Step {
    fn new(evs: Vec<(u32, Pos, Option<String>)>) -> Step {
        Step { evs, edge: 0 }
    }
    fn label(&self) -> String {
        let mut cells: Vec<String> = self.evs.iter().filter(|e| e.2.is_some()).map(|e| format!("{},{},{}", e.1[0], e.1[1], e.1[2])).collect();
        cells.dedup();
        let tag = match self.edge {
            1 => "open-only ",
            2 => "close-only ",
            _ => "",
        };
        format!("[{tag}{}]", cells.join(" "))
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
            out.push(Step::new(g.evs.iter().filter(|e| e.1 == c).cloned().collect()));
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
                        out.push((t, Step::new(Vec::new())));
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
                } else {
                    // a level switched off (a block present at rest): a step of its own tick
                    let i = match out.iter().position(|(t0, _)| *t0 == t) {
                        Some(i) => i,
                        None => {
                            out.push((t, Step::new(Vec::new())));
                            out.len() - 1
                        }
                    };
                    out[i].1.evs.push((0, p, None));
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
fn train_plan(d: &mctas::design::Design, phase: crate::ideal::PlanPhase, train: &[Step], times: &[u32], fixed: &[(bool, u32, Pos, Option<String>)]) -> (Schedule, Vec<(bool, u32, Pos, Option<String>)>) {
    let mut all: Vec<(bool, u32, Pos, Option<String>)> = fixed.to_vec();
    for (st, t) in train.iter().zip(times) {
        for (dt, c, s) in &st.evs {
            for open in [true, false] {
                if (st.edge == 1 && !open) || (st.edge == 2 && open) {
                    continue;
                }
                let e = (open, *t + dt, *c, s.clone());
                if !all.contains(&e) {
                    all.push(e);
                }
            }
        }
    }
    let mut plan = Schedule::from_events(d.clone(), &all);
    // the search applies its pulses between ticks (raw redstone blocks), so the timed train does too
    plan.phase = crate::ideal::PlanPhase::Between;
    let _ = phase;
    (plan, all)
}

/// Pull every step of a working train as early as it goes, on BOTH edges together (unlike
/// `ideal-rs --compact`, which moves each edge's groups on their own), two passes.
fn compact_train(d: &mctas::design::Design, phase: crate::ideal::PlanPhase, train: &[Step], times: &mut [u32], fixed: &[(bool, u32, Pos, Option<String>)], cycles: u32) {
    let screen = crate::ideal::offsets(0);
    let works = |t: &[u32]| ideal_rs::run_edited(&train_plan(d, phase, train, t, fixed).0, &BTreeMap::new(), &[], &screen, cycles, false).is_ok();
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

/// Per-move pocket budgets for `--search`: the pocket a move fires from (if any) and how many
/// moves each pocket may fire in the whole train.
struct Limits {
    pocket_of: Vec<Option<usize>>,
    budgets: Vec<usize>,
    /// Threads simulating a batch of expansions.
    threads: usize,
}

/// `--search`: weighted A* over pairs of idle worlds (the closed door opening, the open door
/// closing) with ANY input cell pulsed (2 gt) on both at once, from (rest, open) to (open, rest):
/// a train not tied to the plan's own steps. Returns the cells pulsed, in order (or the pairs
/// searched and the best distance reached: cells off the two targets).
#[allow(clippy::too_many_arguments)]
fn search_pairs(sim: &Sim, starts: (&World, &World), w0: &World, wopen: &World, moves: &[Step], costs: &[usize], limits: &Limits, cap: usize, weight: usize) -> (Option<Vec<Step>>, usize, usize) {
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
    let key = |a: &Sparse, b: &Sparse, used: &[u8]| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        a.hash(&mut h);
        0xffff_ffffu32.hash(&mut h);
        b.hash(&mut h);
        used.hash(&mut h);
        h.finish()
    };
    let mut seen: HashSet<u64> = HashSet::new();
    // (open-edge world, close-edge world, parent, move, steps, moves fired per pocket)
    let (sa0, sb0) = (sim.sparse(starts.0), sim.sparse(starts.1));
    let used0 = vec![0u8; limits.budgets.len()];
    let mut nodes: Vec<(Sparse, Sparse, Option<usize>, usize, usize, Vec<u8>)> = vec![(sa0.clone(), sb0.clone(), None, 0, 0, used0.clone())];
    let h0 = diff(&sa0, &target_open) + diff(&sb0, &target_rest);
    let mut heap = BinaryHeap::new();
    heap.push((std::cmp::Reverse(h0 * weight), 0usize));
    seen.insert(key(&sa0, &sb0, &used0));
    let mut best = h0;
    let mut best_id = 0usize;
    // expand up to `batch` best nodes at once, their children simulated in parallel
    let batch = if limits.threads > 1 { limits.threads * 2 } else { 1 };
    loop {
        let mut ids: Vec<usize> = Vec::new();
        while ids.len() < batch {
            let Some((_, id)) = heap.pop() else { break };
            let (a, b) = (&nodes[id].0, &nodes[id].1);
            if *a == target_open && *b == target_rest {
                let mut out = Vec::new();
                let mut c = id;
                while let Some(p) = nodes[c].2 {
                    out.push(moves[nodes[c].3].clone());
                    c = p;
                }
                out.reverse();
                return (Some(out), nodes.len(), 0);
            }
            ids.push(id);
        }
        if ids.is_empty() || nodes.len() >= cap {
            break;
        }
        let mut jobs: Vec<(usize, usize, Vec<u8>)> = Vec::new();
        for &id in &ids {
            for mi in 0..moves.len() {
                let mut used2 = nodes[id].5.clone();
                if let Some(Some(p)) = limits.pocket_of.get(mi) {
                    if used2[*p] as usize >= limits.budgets[*p] {
                        continue;
                    }
                    used2[*p] += 1;
                }
                jobs.push((id, mi, used2));
            }
        }
        let res = cs::par_map(&jobs, limits.threads.max(1), |(id, mi, _)| {
            let m = &moves[*mi];
            // an edge-specific move leaves the other edge's world as it is
            let side = |s: &Sparse, on: bool| -> Option<Sparse> {
                if !on {
                    return Some(s.clone());
                }
                sim.apply(&sim.rebuild(s), m).ok().map(|w| sim.sparse(&w))
            };
            match (side(&nodes[*id].0, m.edge != 2), side(&nodes[*id].1, m.edge != 1)) {
                (Some(a2), Some(b2)) => Some((a2, b2)),
                _ => None,
            }
        });
        for ((id, mi, used2), r) in jobs.into_iter().zip(res) {
            let Some((sa, sb)) = r else { continue };
            let k = key(&sa, &sb, &used2);
            if !seen.insert(k) {
                continue;
            }
            let h2 = diff(&sa, &target_open) + diff(&sb, &target_rest);
            if h2 < best {
                best_id = nodes.len();
            }
            best = best.min(h2);
            let g2 = nodes[id].4 + costs.get(mi).copied().unwrap_or(1);
            nodes.push((sa, sb, Some(id), mi, g2, used2));
            heap.push((std::cmp::Reverse(g2 + h2 * weight), nodes.len() - 1));
        }
    }
    if std::env::var("INTERLEAVE_DEBUG").is_ok() && best_id < nodes.len() {
        let show = |a: &Sparse, b: &Sparse, what: &str| {
            let ma: HashMap<u32, Block> = a.iter().copied().collect();
            let mb: HashMap<u32, Block> = b.iter().copied().collect();
            let size = [sim.hi[0] - sim.lo[0] + 1, sim.hi[1] - sim.lo[1] + 1, sim.hi[2] - sim.lo[2] + 1];
            let mut keys: Vec<u32> = ma.keys().chain(mb.keys()).copied().collect();
            keys.sort();
            keys.dedup();
            for i in keys {
                let (x, y) = (ma.get(&i), mb.get(&i));
                if x != y {
                    let ii = i as i32;
                    let p = [sim.lo[0] + ii % size[0], sim.lo[1] + ii / (size[0] * size[2]), sim.lo[2] + (ii / size[0]) % size[2]];
                    eprintln!("  {what} {p:?}: {:?} (want {:?})", x, y);
                }
            }
        };
        show(&nodes[best_id].0, &target_open, "opening world");
        show(&nodes[best_id].1, &target_rest, "closing world");
    }
    (None, nodes.len(), best)
}

#[derive(Clone)]
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
    /// `--search` also offers the plan's own tick groups as moves.
    pub groups: bool,
    /// `--search` also offers every pair of inputs pulsed together as moves.
    pub pairs: bool,
    /// Input cells always pulsed together (one source for several pistons), each set one move.
    pub merge: Vec<Vec<Pos>>,
    /// Search cost of a move pulsing these cells (default 1): trains that use them less.
    pub cost: Vec<(Pos, usize)>,
    /// Pockets: input cells one small pocket feeds and how many ticks it can make in the train.
    pub pockets: Vec<(Vec<Pos>, usize)>,
    /// Blocks added to the design (e.g. observers in a pocket), raw in both rest states.
    pub fix: Vec<(Pos, String)>,
    /// Inputs the train may not use (their pistons must be fed by the `fix` blocks).
    pub drop: Vec<Pos>,
    /// `--search` also offers level moves (an input switched on, later off): holds.
    pub levels: bool,
    /// Threads simulating each batch of the search's expansions (1 in batch mode).
    pub search_threads: usize,
    /// Cells emptied in both rest states (a plan input's leftover block, e.g. a trigger).
    pub clear: Vec<Pos>,
    /// The cells the train pulses, instead of the plan's inputs (feed cells in the box).
    pub cells: Vec<Pos>,
    /// Offer one-edge pulses (wiring gates them on the lever's state) at this search cost; 0 = off.
    pub edge_cost: usize,
}

/// Several plans at once (in parallel), one summary line each; `out_dir` gets
/// `<stem>-train.ideal` for each plan with a train.
/// `--sat-loop N`: search a train, wire it with `door satwire <sat-args>`, and while satwire
/// answers UNSAT, make the inputs next to its core pistons dearer (their search cost doubles) and
/// search again; stops at the first train satwire wires and judges (`JUDGE OK`).
pub fn sat_loop(path: &Path, o: &Options, iters: usize, sat_args: &str, dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let exe = std::env::current_exe()?;
    let mut o = o.clone();
    o.quiet = true;
    let text = std::fs::read_to_string(path)?;
    let s = Schedule::parse(&text)?;
    let mut cells: Vec<Pos> = Vec::new();
    for open in [true, false] {
        for (_, p, st) in s.resolved(open)? {
            if st.is_some() && !cells.contains(&p) {
                cells.push(p);
            }
        }
    }
    for i in 0..iters {
        let plan = dir.join(format!("loop{i}.ideal"));
        let summary = run(path, &o, Some(&plan))?;
        let costs: Vec<String> = o.cost.iter().map(|(c, k)| format!("{},{},{}={k}", c[0], c[1], c[2])).collect();
        println!("iteration {i} (costs {}): {summary}", costs.join(" "));
        if !summary.starts_with("ONE TRAIN") {
            break;
        }
        let out_dir = dir.join(format!("sat{i}"));
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("door").arg("satwire").arg(&plan).args(sat_args.split_whitespace()).arg("--out").arg(&out_dir);
        let res = cmd.output()?;
        let log = format!("{}{}", String::from_utf8_lossy(&res.stdout), String::from_utf8_lossy(&res.stderr));
        std::fs::write(dir.join(format!("sat{i}.log")), &log)?;
        if log.contains("JUDGE OK with --extra-offsets") {
            println!("  SATWIRE WIRED AND JUDGED IT: {} (plan {})", out_dir.display(), plan.display());
            return Ok(());
        }
        let Some(core) = log.lines().find(|l| l.contains("core (pistons")) else {
            println!("  satwire gave no core: {}", log.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or(""));
            break;
        };
        println!("  satwire: {}", core.trim());
        // the core's pistons as (x, y)
        let mut pistons: Vec<(i32, i32)> = Vec::new();
        for part in core.split('(').skip(2) {
            let inner = part.split(')').next().unwrap_or("");
            let v: Vec<i32> = inner.split(',').filter_map(|t| t.trim().parse::<i32>().ok()).collect();
            if v.len() == 2 {
                pistons.push((v[0], v[1]));
            }
        }
        let mut bumped = false;
        for c in &cells {
            if pistons.iter().any(|(x, y)| (c[0] - x).abs() + (c[1] - y).abs() <= 1) {
                match o.cost.iter_mut().find(|(p, _)| p == c) {
                    Some((_, k)) => *k *= 2,
                    None => o.cost.push((*c, 2)),
                }
                bumped = true;
            }
        }
        if !bumped {
            println!("  no input next to the core pistons {pistons:?}: stopping");
            break;
        }
    }
    Ok(())
}

/// `--observe cells`: every filling of these (pocket) cells with air or an observer facing any way
/// in the door's plane is added to the design (`fix`), the inputs in `o.drop` are taken away, and
/// a train is searched for the rest: the pocket's pistons then fire from the mechanism's own
/// moves (each observer 2 gt after the cell it watches changes), on both edges alike. One line
/// per filling with a train; `out_dir` gets its plan + design.
pub fn observe(path: &Path, o: &Options, pocket: &[Pos], threads: usize, out_dir: Option<&Path>) -> Result<()> {
    let facings = ["east", "west", "up", "down"];
    let mut fills: Vec<Vec<(Pos, String)>> = vec![Vec::new()];
    for c in pocket {
        let mut next = Vec::new();
        for f in &fills {
            next.push(f.clone());
            for fc in facings {
                let mut g = f.clone();
                g.push((*c, format!("minecraft:observer[facing={fc}]")));
                next.push(g);
            }
        }
        fills = next;
    }
    fills.retain(|f| !f.is_empty());
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(dir)?;
    }
    println!("{} fillings of {:?}, inputs {:?} dropped", fills.len(), pocket, o.drop);
    let res = cs::par_map(&fills, threads, |f| {
        let mut o2 = o.clone();
        o2.quiet = true;
        o2.fix.extend(f.iter().cloned());
        let tag: Vec<String> = f.iter().map(|(p, s)| format!("{},{}{}", p[0], p[1], s.split("facing=").nth(1).map(|x| &x[..1]).unwrap_or(""))).collect();
        let out = out_dir.map(|d| d.join(format!("obs-{}.ideal", tag.join("_"))));
        (tag.join(" "), run(path, &o2, out.as_deref()))
    });
    let mut found = 0;
    for (tag, r) in res {
        match r {
            Ok(s) if s.starts_with("ONE TRAIN") => {
                found += 1;
                println!("{tag}: {s}");
            }
            Ok(_) | Err(_) => {}
        }
    }
    println!("{found} fillings give a train");
    Ok(())
}

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

/// Write a plan file: a header comment, the source plan's `design` line (or `design`), its phase,
/// the events.
fn write_plan(out: &Path, header: &str, text: &str, phase: crate::ideal::PlanPhase, all: &mut [(bool, u32, Pos, Option<String>)], design: Option<&Path>) -> Result<()> {
    let mut lines = vec![format!("# {header}")];
    match design {
        Some(p) => lines.push(format!("design {}", p.display())),
        None => {
            if let Some(l) = text.lines().find(|l| l.trim_start().starts_with("design ")) {
                lines.push(l.to_string());
            }
        }
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
#[allow(clippy::too_many_arguments)]
fn timed_train(d: &mctas::design::Design, phase: crate::ideal::PlanPhase, text: &str, path: &Path, train: &[Step], fixed: &[(bool, u32, Pos, Option<String>)], n: usize, o: &Options, out: Option<&Path>) -> Result<String> {
    let shown: Vec<String> = train.iter().map(|st| st.label()).collect();
    say!(o, "TRAIN of {} steps for both edges ({n} pairs searched): {}", train.len(), shown.join(" "));
    let offsets = crate::ideal::offsets(o.extra_offsets);
    for spacing in [4u32, 6, 8, 10, 12, 16, 20, 30] {
        // with lever-held inputs the train starts one spacing after the lever's own change
        let lead = if fixed.is_empty() { 0 } else { 1 };
        let mut times: Vec<u32> = (0..train.len() as u32).map(|k| (k + lead) * spacing).collect();
        let (plan, _) = train_plan(d, phase, train, &times, fixed);
        if let Err(e) = ideal_rs::run_edited(&plan, &BTreeMap::new(), &[], &offsets, o.cycles, false) {
            say!(o, "timed every {spacing} gt: FAILS: {}", e.chars().take(200).collect::<String>());
            continue;
        }
        say!(o, "WORKS timed every {spacing} gt ({} positions x {} cycles)", offsets.len(), o.cycles);
        compact_train(d, phase, train, &mut times, fixed, o.cycles);
        let (plan, mut all) = train_plan(d, phase, train, &times, fixed);
        let shown: Vec<String> = train.iter().zip(&times).map(|(st, t)| format!("t{t}:{}", st.label())).collect();
        say!(o, "compacted (both edges together): {}", shown.join(" "));
        let distinct = all.iter().filter(|e| e.0 && e.3.is_some()).count();
        let summary = match ideal_rs::run_edited(&plan, &BTreeMap::new(), &[], &offsets, o.cycles, false) {
            Ok((a, b)) => {
                let last = times.iter().max().unwrap_or(&0);
                say!(o, "WORKS compacted ({} positions x {} cycles): open {a} / close {b}, last step t{last}", offsets.len(), o.cycles);
                format!("ONE TRAIN: {distinct} pulses, last t{last}, open {a} / close {b} ({} positions)", offsets.len())
            }
            Err(e) => {
                say!(o, "compacted train fails at the full positions ({e}); keeping every {spacing} gt");
                times = (0..train.len() as u32).map(|k| (k + lead) * spacing).collect();
                all = train_plan(d, phase, train, &times, fixed).1;
                format!("ONE TRAIN: {} steps every {spacing} gt (compaction failed)", train.len())
            }
        };
        let mut per_cell: BTreeMap<Pos, Vec<u32>> = BTreeMap::new();
        for e in all.iter().filter(|e| e.0 && e.3.is_some()) {
            per_cell.entry(e.2).or_default().push(e.1);
        }
        for (c, ts) in per_cell.iter_mut() {
            ts.sort();
            say!(o, "  input {c:?}: on at {ts:?} on both edges");
        }
        if let Some(out) = out {
            let header = format!("door interleave --search {}: ONE train of {} pulses, the same events on both lever edges (found every {spacing} gt, then compacted on both edges together)", path.display(), train.len());
            // with `--fix` blocks the plan needs its own design (the source's plus those blocks)
            let design = if o.fix.is_empty() {
                None
            } else {
                let mut dd = d.clone();
                dd.name = format!("{}-fix", dd.name);
                let p = out.with_extension("txt");
                dd.save(&p)?;
                Some(p)
            };
            write_plan(out, &header, text, crate::ideal::PlanPhase::Between, &mut all, design.as_deref())?;
        }
        return Ok(summary);
    }
    Ok(format!("train of {} pulses found step by step but no timing works", train.len()))
}

fn run(path: &Path, o: &Options, out: Option<&Path>) -> Result<String> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let s = Schedule::parse(&text)?;
    let (mut d, open_all, close_all, _) = s.realize(s.vars())?;
    // the design's real lever (`use` events): flipped at t0 of both edges, like lever-held inputs
    let is_use = |e: &(u32, Pos, Option<String>)| e.2.as_deref() == Some("@use");
    let mut uses: Vec<Pos> = open_all.iter().chain(close_all.iter()).filter(|e| is_use(e)).map(|e| e.1).collect();
    uses.sort();
    uses.dedup();
    if open_all.iter().chain(close_all.iter()).any(|e| is_use(e) && e.0 != 0) {
        bail!("lever uses after t0 are not handled");
    }
    let open_ev: Vec<(u32, Pos, Option<String>)> = open_all.into_iter().filter(|e| !is_use(e)).collect();
    let close_ev: Vec<(u32, Pos, Option<String>)> = close_all.into_iter().filter(|e| !is_use(e)).collect();
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
    // `--fix`: blocks added to the design (observers in a pocket): placed raw into both rest
    // states, so the plan's own replay (which knows nothing of them) still gives the targets
    for (p, st) in &o.fix {
        d.blocks.insert(*p, st.clone());
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
    // the open rest state: the lever used, then the open plan run with its own timing (a plan's
    // holds may overlap other inputs, which a step-by-step replay would serialize)
    let mut wopen = w0.clone();
    {
        let mut evs = open_ev.clone();
        evs.sort_by_key(|e| e.0);
        for c in &uses {
            wopen.use_block(cs::bp(*c));
        }
        let mut t = 0u32;
        let mut i = 0;
        while i < evs.len() {
            while i < evs.len() && evs[i].0 == t {
                let (_, q, st) = &evs[i];
                let b = match st {
                    Some(x) => cs::convert_state(x).map_err(|e| anyhow::anyhow!("{x}: {e}"))?,
                    None => Block::Air,
                };
                wopen.set_block(cs::bp(*q), b, redsim::world::flags::ALL);
                i += 1;
            }
            if i < evs.len() {
                wopen.tick();
                t += 1;
            }
        }
        let (_, idle) = cs::step_until_idle(&mut wopen, 300, |_| {});
        if !idle {
            bail!("the open plan never settles");
        }
    }
    let (w0, wopen) = if o.fix.is_empty() && o.clear.is_empty() {
        (w0, wopen)
    } else {
        // `--fix` blocks placed and `--clear` cells emptied, raw, in both rest states
        let with_fix = |w: &World| -> Result<World> {
            let mut w = w.clone();
            for (p, st) in &o.fix {
                w.set_block(cs::bp(*p), cs::convert_state(st).map_err(|e| anyhow::anyhow!("{st}: {e}"))?, redsim::world::flags::RAW);
            }
            for p in &o.clear {
                w.set_block(cs::bp(*p), Block::Air, redsim::world::flags::RAW);
            }
            Ok(w)
        };
        (with_fix(&w0)?, with_fix(&wopen)?)
    };
    if let Some(cap) = o.search {
        let mut cells: Vec<Pos> = open_ev.iter().chain(close_ev.iter()).filter(|e| e.2.is_some()).map(|e| e.1).collect();
        cells.sort();
        cells.dedup();
        cells.retain(|c| !o.drop.contains(c) && !o.clear.contains(c));
        // lever-held inputs: cells whose block differs between the open and the closed rest
        // (levels the plan leaves on): the lever itself sets them at t0 of each edge, the train
        // never touches them
        let (rs, os_) = (sim.snap(&w0), sim.snap(&wopen));
        let is_rb = |b: Option<Block>| b.is_some_and(|b| b.to_string().contains("redstone_block"));
        let lever: Vec<(Pos, Block, Block)> = cells
            .iter()
            .filter(|c| rs.get(**c) != os_.get(**c) && (is_rb(rs.get(**c)) || is_rb(os_.get(**c))))
            .map(|c| (*c, rs.get(*c).unwrap_or(Block::Air), os_.get(*c).unwrap_or(Block::Air)))
            .collect();
        cells.retain(|c| !lever.iter().any(|l| l.0 == *c) && !uses.contains(c));
        // `--cells`: the train pulses exactly these cells (feed cells in the box) instead of the
        // plan's ideal inputs (lever-held inputs and the real lever stay as they are)
        if !o.cells.is_empty() {
            cells = o.cells.clone();
        }
        let flip = |w: &World, to_open: bool| -> Result<World> {
            let mut w = w.clone();
            for (c, closed, open) in &lever {
                w.set_block(cs::bp(*c), if to_open { *open } else { *closed }, redsim::world::flags::ALL);
            }
            for c in &uses {
                w.use_block(cs::bp(*c));
            }
            let (_, idle) = cs::step_until_idle(&mut w, 300, |_| {});
            if !idle {
                bail!("never idle after the lever-held inputs flip");
            }
            Ok(w)
        };
        let (a0, b0) = if lever.is_empty() && uses.is_empty() { (w0.clone(), wopen.clone()) } else { (flip(&w0, true)?, flip(&wopen, false)?) };
        let mut fixed: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
        for (c, closed, open) in &lever {
            let st = |b: &Block| if *b == Block::Air { None } else { Some(b.to_string()) };
            fixed.push((true, 0, *c, st(open)));
            fixed.push((false, 0, *c, st(closed)));
        }
        for c in &uses {
            fixed.push((true, 0, *c, Some("@use".to_string())));
            fixed.push((false, 0, *c, Some("@use".to_string())));
        }
        if !lever.is_empty() {
            say!(o, "lever-held inputs (set at t0 of each edge, not in the train): {:?}", lever.iter().map(|l| l.0).collect::<Vec<_>>());
        }
        // moves: a 2 gt pulse on any input cell (or on each `--merge` set of cells at once: one
        // source powering them together), plus (`--groups`) every tick group of the plan itself
        // (several inputs with their own relative timing)
        let rbs = || Some("minecraft:redstone_block".to_string());
        let mut moves: Vec<Step> = Vec::new();
        let mut pocket_of: Vec<Option<usize>> = Vec::new();
        for set in &o.merge {
            let mut evs: Vec<(u32, Pos, Option<String>)> = set.iter().map(|c| (0, *c, rbs())).collect();
            evs.extend(set.iter().map(|c| (2, *c, None)));
            moves.push(Step::new(evs));
            pocket_of.push(None);
        }
        // `--pocket`: the inputs a small pocket feeds fire at most `budget` times in the whole
        // train; any subset of them may fire together (one tick of that pocket)
        for (pi, (set, _)) in o.pockets.iter().enumerate() {
            let set: Vec<Pos> = set.iter().copied().filter(|c| cells.contains(c)).collect();
            for mask in 1u32..(1 << set.len()) {
                let sub: Vec<Pos> = (0..set.len()).filter(|i| mask >> i & 1 == 1).map(|i| set[i]).collect();
                let mut evs: Vec<(u32, Pos, Option<String>)> = sub.iter().map(|c| (0, *c, rbs())).collect();
                evs.extend(sub.iter().map(|c| (2, *c, None)));
                moves.push(Step::new(evs));
                pocket_of.push(Some(pi));
            }
        }
        for &c in &cells {
            if !o.merge.iter().any(|s| s.contains(&c)) && !o.pockets.iter().any(|(s, _)| s.contains(&c)) {
                moves.push(Step::new(vec![(0, c, rbs()), (2, c, None)]));
                pocket_of.push(None);
            }
        }
        let in_pocket = |c: &Pos| o.pockets.iter().any(|(s, _)| s.contains(c));
        if o.groups {
            for st in steps(&open_ev, false).into_iter().chain(steps(&close_ev, false)) {
                if !moves.contains(&st) && !st.evs.iter().any(|e| lever.iter().any(|l| l.0 == e.1) || in_pocket(&e.1)) {
                    moves.push(st);
                    pocket_of.push(None);
                }
            }
        }
        if o.pairs {
            // two inputs pulsed together (2 gt, the same tick)
            for i in 0..cells.len() {
                for j in i + 1..cells.len() {
                    let (a, b) = (cells[i], cells[j]);
                    if in_pocket(&a) || in_pocket(&b) {
                        continue;
                    }
                    let rb = || Some("minecraft:redstone_block".to_string());
                    moves.push(Step::new(vec![(0, a, rb()), (0, b, rb()), (2, a, None), (2, b, None)]));
                    pocket_of.push(None);
                }
            }
        }
        if o.levels {
            // an input switched on (and left on until a later move switches it off): holds
            for &c in &cells {
                if in_pocket(&c) {
                    continue;
                }
                moves.push(Step::new(vec![(0, c, rbs())]));
                pocket_of.push(None);
                moves.push(Step::new(vec![(0, c, None)]));
                pocket_of.push(None);
            }
        }
        if o.edge_cost > 0 {
            // a pulse on one edge only (wiring gates it on the lever's state), at a higher cost
            for &c in &cells {
                for edge in [1u8, 2] {
                    moves.push(Step { evs: vec![(0, c, rbs()), (2, c, None)], edge });
                    pocket_of.push(None);
                }
            }
        }
        // a move's cost: 1, or the largest `--cost` of the cells it pulses
        let costs: Vec<usize> = moves
            .iter()
            .map(|m| {
                let base = m.evs.iter().filter_map(|e| o.cost.iter().find(|(c, _)| *c == e.1).map(|(_, k)| *k)).max().unwrap_or(1);
                if m.edge != 0 { base.max(o.edge_cost) } else { base }
            })
            .collect();
        let limits = Limits { pocket_of, budgets: o.pockets.iter().map(|(_, b)| *b).collect(), threads: o.search_threads };
        let (found, n, best) = search_pairs(&sim, (&a0, &b0), &w0, &wopen, &moves, &costs, &limits, cap, o.weight);
        return match found {
            None => Ok(format!("NO train within {n} world pairs (best: {best} cells off the targets)")),
            Some(train) => timed_train(&d, s.phase, &text, path, &train, &fixed, n, o, out),
        };
    }
    if !uses.is_empty() {
        bail!("the plain merge does not handle the real lever: use --search");
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
        write_plan(out, &header, text, phase, &mut all, None)?;
    }
    Ok(summary)
}
