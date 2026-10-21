//! `mctas circuit shulker lns` and `mctas circuit shulker anneal`: smaller shulker loaders and
//! unloaders (vjeux's standing rule: minimum volume, the design co-designed as a whole). Every
//! candidate is judged on redsim (rs.rs: the judge's own scenarios and verdict), first on a short
//! screen (the unstackable scenario, capped), then on every scenario the original passes there,
//! at the original's rate or better (within a tolerance). The game (`mctas circuit verify`) judges
//! the finds that are kept.
//!
//! - `lns`: remove every r-subset of the design's blocks (plus every block outside `--box`, the
//!   box compaction) and grow back at most `--add` blocks of the palette near the removed cells,
//!   inside the box.
//! - `anneal`: a random local search (a batch of random moves per step: delete, re-state, add,
//!   move a block, move a port chest), accepting the best candidate that passes and does not make
//!   (volume, blocks, outer-layer blocks) worse.

use crate::rs;
use anyhow::{Result, anyhow, bail};
use circuit::Circuit;
use circuit::shulker::Scenario;
use mctas::design::Design;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

type Pos = [i32; 3];

/// What a candidate must do: the scenarios the original passes on redsim, its rate.
#[derive(Clone)]
pub struct Target {
    /// The screen: one short scenario with a tight cap.
    pub screen: (String, Scenario),
    pub screen_cap: u32,
    pub required: Vec<(String, Scenario)>,
    /// The plain scenario's items/s floor.
    pub min_rate: f64,
    /// Trickle sweeps (`anneal --sweep`): a required one-box scenario run at every phase of a box
    /// every `every` gt (`RunOpts::trickle`), each run capped at `cap`; hard constraints checked
    /// after the required runs.
    pub sweeps: Vec<Sweep>,
}

/// One trickle sweep of a [`Target`].
#[derive(Clone)]
pub struct Sweep {
    pub name: String,
    pub scenario: Scenario,
    pub every: u32,
    pub cap: u32,
}

/// The first phase at which `c` fails the sweep, or `None` when every phase passes.
pub fn sweep_fails(c: &Circuit, s: &Sweep) -> Option<u32> {
    (0..s.every).find(|&phase| rs::check(c, &s.name, &s.scenario, rs::RunOpts { cap: Some(s.cap), trickle: Some((s.every, phase)), ..Default::default() }).is_err())
}

/// Add trickle sweeps to the target of the original `c`: for each `every`, the required one-box
/// scenarios (`one-box`, and `one-box-burst` for a loader), every run capped at twice the
/// original's longest phase plus 400 gt. A sweep the original fails is not required (warned).
pub fn add_sweeps(c: &Circuit, t: &mut Target, everies: &[u32]) {
    for &every in everies {
        for name in ["one-box", "one-box-burst"] {
            let Some((_, sc)) = t.required.iter().find(|(n, _)| n == name).cloned() else { continue };
            let mut longest = 0u32;
            let mut fail = None;
            for phase in 0..every.max(1) {
                match rs::run(c, &sc, rs::RunOpts { trickle: Some((every.max(1), phase)), ..Default::default() }) {
                    Ok(log) if circuit::shulker::judge(&sc, &log).pass && rs::badge_ok(name, &sc, &log).is_ok() => longest = longest.max(log.end),
                    _ => {
                        fail = Some(phase);
                        break;
                    }
                }
            }
            match fail {
                None => t.sweeps.push(Sweep { name: name.to_string(), scenario: sc, every: every.max(1), cap: longest * 2 + 400 }),
                Some(p) => eprintln!("  {}: {name} fails at phase {p} of a box every {every} gt, that sweep is not required", c.name()),
            }
        }
    }
}

/// A judged candidate's numbers.
#[derive(Clone, Debug)]
pub struct Score {
    pub volume: i64,
    pub blocks: usize,
    pub dims: [i32; 3],
    pub rate: f64,
}

fn rate_of(m: &[(String, String)]) -> f64 {
    m.iter().find(|(k, _)| k == "items/s").and_then(|(_, v)| v.parse().ok()).unwrap_or(0.0)
}

/// The original's target: every scenario it passes on redsim (`colours` and the game-only checks
/// aside), its plain rate less `tol`.
pub fn target_of(c: &Circuit, tol: f64) -> Result<Target> {
    let mut required = Vec::new();
    let mut base_rate = 0.0;
    let mut screen = None;
    for (name, sc) in rs::scenarios(c) {
        match rs::run(c, &sc, rs::RunOpts::default()) {
            Ok(log) => {
                let out = circuit::shulker::judge(&sc, &log);
                if out.pass && rs::badge_ok(&name, &sc, &log).is_ok() {
                    if name == "base" {
                        base_rate = rate_of(&out.metrics);
                    }
                    if (name == "unstackable" || (name == "base" && sc.stack == 1)) && screen.is_none() {
                        screen = Some(((name.clone(), sc.clone()), log.end * 2 + 400));
                    }
                    required.push((name, sc));
                } else {
                    eprintln!("  {}: {name} fails on redsim ({}), not required", c.name(), if out.pass { "badge" } else { out.why.as_str() });
                }
            }
            Err(why) => eprintln!("  {}: {name} does not run on redsim ({why}), not required", c.name()),
        }
    }
    if !required.iter().any(|(n, _)| n == "base") {
        bail!("{}: the original fails its plain scenario on redsim", c.name());
    }
    let ((sn, ssc), cap) = screen.unwrap_or_else(|| (required[0].clone(), required[0].1.cap));
    Ok(Target { screen: (sn, ssc), screen_cap: cap, required, min_rate: base_rate - tol, sweeps: Vec::new() })
}

/// A candidate judged against the target: its score, or `None`.
pub fn judge(c: &Circuit, t: &Target) -> Option<Score> {
    let (sn, ssc) = &t.screen;
    rs::check(c, sn, ssc, rs::RunOpts { cap: Some(t.screen_cap), ..Default::default() }).ok()?;
    let mut rate = 0.0;
    for (name, sc) in &t.required {
        if name == sn {
            continue;
        }
        let m = rs::check(c, name, sc, rs::RunOpts::default()).ok()?;
        if name == "base" {
            rate = rate_of(&m);
            if rate + 1e-9 < t.min_rate {
                return None;
            }
        }
    }
    if t.sweeps.iter().any(|s| sweep_fails(c, s).is_some()) {
        return None;
    }
    Some(score_of(c, rate))
}

pub fn score_of(c: &Circuit, rate: f64) -> Score {
    let dims = rs::dims(c);
    let ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    let blocks = c.clock.design.blocks.keys().filter(|p| !ins.contains(p)).count();
    Score { volume: dims.iter().map(|&d| d as i64).product(), blocks, dims, rate }
}

/// The 1-wide palette (a design tiled along x: nothing faces the neighbouring copies, no dust,
/// slime or honey), or the shorthands of `mctas clock census` plus `onewide`.
pub fn palette(spec: &str) -> Result<Vec<String>> {
    let mut v = Vec::new();
    for item in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if item == "onewide" {
            let f4 = ["north", "south", "up", "down"];
            let h2 = ["north", "south"];
            v.push("minecraft:stone".to_string());
            v.push("minecraft:glass".to_string());
            for f in f4 {
                v.push(format!("minecraft:observer[facing={f}]"));
                v.push(format!("minecraft:dispenser[facing={f}]"));
                v.push(format!("minecraft:dropper[facing={f}]"));
                v.push(format!("minecraft:piston[facing={f}]"));
                v.push(format!("minecraft:sticky_piston[facing={f}]"));
            }
            for f in h2 {
                v.push(format!("minecraft:comparator[facing={f},mode=compare]"));
                for d in 1..=4 {
                    v.push(format!("minecraft:repeater[delay={d},facing={f}]"));
                }
                v.push(format!("minecraft:hopper[facing={f}]"));
                v.push(format!("minecraft:redstone_wall_torch[facing={f}]"));
            }
            v.push("minecraft:hopper[facing=down]".to_string());
            v.push("minecraft:redstone_lamp".to_string());
            v.push("minecraft:waxed_copper_bulb".to_string());
            v.push("minecraft:redstone_torch".to_string());
            v.push("minecraft:redstone_block".to_string());
            v.push("minecraft:target".to_string());
        } else {
            v.extend(clock::census::expand_palette(item)?);
        }
    }
    // canonical text through redsim, without duplicates
    let mut out: Vec<String> = Vec::new();
    for s in v {
        let t = redsim::block::Block::parse(&s).map(|b| b.to_string()).map_err(|e| anyhow!("{s}: {e}"))?;
        if !out.contains(&t) {
            out.push(t);
        }
    }
    Ok(out)
}

/// The cells a design may use: inside `lo..=hi`, not a port (the judge's chests and inputs).
fn free_cells(d: &Design, lo: Pos, hi: Pos) -> Vec<Pos> {
    let ports: Vec<Pos> = d.ports.values().copied().collect();
    let mut v = Vec::new();
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let p = [x, y, z];
                if !d.blocks.contains_key(&p) && !ports.contains(&p) {
                    v.push(p);
                }
            }
        }
    }
    v
}

fn manhattan(a: Pos, b: Pos) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

/// The design's block box (inputs left out).
pub fn box_of(d: &Design, inputs: &[Pos]) -> (Pos, Pos) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for p in d.blocks.keys().filter(|p| !inputs.contains(p)) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    (lo, hi)
}

/// `pext lns`-style settings.
pub struct LnsOptions {
    pub r: usize,
    pub add: usize,
    pub local: i32,
    pub palette: Vec<String>,
    pub threads: usize,
    /// The box the result must fit (default: the design's own box).
    pub bbox: Option<(Pos, Pos)>,
    pub tol: f64,
    pub out_dir: Option<std::path::PathBuf>,
    /// Blocks never removed.
    pub keep: Vec<Pos>,
}

fn combos(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = Vec::with_capacity(k);
    fn rec(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            if n - i < k - cur.len() {
                break;
            }
            cur.push(i);
            rec(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    rec(0, n, k, &mut cur, &mut out);
    out
}

/// One candidate design as a circuit (`None` when it does not load).
fn circuit_of(d: &Design) -> Option<Circuit> {
    Circuit::from_design(d.clone()).ok()
}

/// `shulker lns`: every r-subset removed (with every block outside the box), at most `add`
/// blocks grown back near the removed cells inside the box; the finds smaller than the original
/// (volume, then blocks), best first.
pub fn lns(path: &Path, o: &LnsOptions) -> Result<Vec<(Score, Design)>> {
    let c0 = Circuit::load(path)?;
    let t = target_of(&c0, o.tol)?;
    let s0 = score_of(&c0, t.min_rate + o.tol);
    let d0 = c0.clock.design.clone();
    let inputs: Vec<Pos> = c0.inputs.iter().map(|i| i.pos).collect();
    let (blo, bhi) = o.bbox.unwrap_or_else(|| box_of(&d0, &inputs));
    let forced: Vec<Pos> = d0.blocks.keys().copied().filter(|p| !inputs.contains(p) && (0..3).any(|i| p[i] < blo[i] || p[i] > bhi[i])).collect();
    let removable: Vec<Pos> = d0.blocks.keys().copied().filter(|p| !inputs.contains(p) && !forced.contains(p) && !o.keep.contains(p)).collect();
    let subsets = combos(removable.len(), o.r);
    println!(
        "{}: {} blocks, {}x{}x{} = {} (target box {:?}..{:?}: {} forced out), {} removable, {} subsets of {}, growing back <= {}, required: {} (rate >= {:.2})",
        c0.name(),
        s0.blocks,
        s0.dims[0],
        s0.dims[1],
        s0.dims[2],
        s0.volume,
        blo,
        bhi,
        forced.len(),
        removable.len(),
        subsets.len(),
        o.r,
        o.add,
        t.required.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" "),
        t.min_rate
    );
    let t0 = std::time::Instant::now();
    // work units: (subset, cells grown, cell combo), so a run with few subsets still fills the
    // threads
    let mut bases: Vec<(Design, Vec<Pos>)> = Vec::with_capacity(subsets.len());
    let mut units: Vec<(usize, Vec<usize>)> = Vec::new();
    for sub in &subsets {
        let mut base = d0.clone();
        let mut removed: Vec<Pos> = forced.clone();
        for &i in sub {
            removed.push(removable[i]);
        }
        for p in &removed {
            base.blocks.remove(p);
        }
        let cells: Vec<Pos> = free_cells(&base, blo, bhi).into_iter().filter(|c| removed.iter().any(|r| manhattan(*r, *c) <= o.local)).collect();
        for k in 0..=o.add {
            for cs in combos(cells.len(), k) {
                units.push((bases.len(), cs));
            }
        }
        bases.push((base, cells));
    }
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let tried = AtomicUsize::new(0);
    let found: Mutex<Vec<(Score, Design)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.threads.max(1) {
            s.spawn(|| loop {
                let ui = next.fetch_add(1, Ordering::Relaxed);
                if ui >= units.len() {
                    break;
                }
                let (bi, cs) = &units[ui];
                let (base, cells) = &bases[*bi];
                let m = o.palette.len();
                let total = m.pow(cs.len() as u32);
                for tup in 0..total {
                    let mut d = base.clone();
                    let mut tt = tup;
                    for &ci in cs {
                        d.blocks.insert(cells[ci], o.palette[tt % m].clone());
                        tt /= m;
                    }
                    tried.fetch_add(1, Ordering::Relaxed);
                    let Some(c) = circuit_of(&d) else { continue };
                    if let Some(sc) = judge(&c, &t)
                        && (sc.volume, sc.blocks) < (s0.volume, s0.blocks)
                    {
                        found.lock().unwrap().push((sc, d));
                    }
                }
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 500 == 0 || n == units.len() {
                    let f = found.lock().unwrap();
                    let best = f.iter().map(|(s, _)| (s.volume, s.blocks)).min();
                    eprintln!("  {n}/{} units, {} candidates ({:.0} s), best {best:?}", units.len(), tried.load(Ordering::Relaxed), t0.elapsed().as_secs_f64());
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap();
    found.sort_by(|a, b| (a.0.volume, a.0.blocks).cmp(&(b.0.volume, b.0.blocks)).then(b.0.rate.total_cmp(&a.0.rate)));
    save_finds(&c0, &found, o.out_dir.as_deref())?;
    Ok(found)
}

fn save_finds(c0: &Circuit, found: &[(Score, Design)], dir: Option<&Path>) -> Result<()> {
    for (i, (s, d)) in found.iter().enumerate() {
        if i < 30 {
            println!("  {}: {} blocks, {}x{}x{} = {}, {:.2} items/s: {}", i + 1, s.blocks, s.dims[0], s.dims[1], s.dims[2], s.volume, s.rate, diff(&c0.clock.design, d));
        }
        if let Some(dir) = dir {
            std::fs::create_dir_all(dir)?;
            let mut d = d.clone();
            d.name = format!("{}-v{}-b{}-{}", c0.name(), s.volume, s.blocks, i + 1);
            d.save(&dir.join(format!("{}.txt", d.name)))?;
        }
    }
    Ok(())
}

/// What changed from `a` to `b`: `-x,y,z state` for each removed block, `+x,y,z state` for each
/// added or re-stated one, `port name x,y,z` for each moved port.
pub fn diff(a: &Design, b: &Design) -> String {
    let mut v = Vec::new();
    for (p, s) in &a.blocks {
        if !b.blocks.contains_key(p) {
            v.push(format!("-{},{},{} {}", p[0], p[1], p[2], clock::short(s)));
        }
    }
    for (p, s) in &b.blocks {
        if a.blocks.get(p) != Some(s) {
            v.push(format!("+{},{},{} {}", p[0], p[1], p[2], clock::short(s)));
        }
    }
    for (n, p) in &b.ports {
        if a.ports.get(n) != Some(p) {
            v.push(format!("port {n} {},{},{}", p[0], p[1], p[2]));
        }
    }
    v.join("; ")
}

/// A small deterministic random (xorshift).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// `shulker anneal` settings.
pub struct AnnealOptions {
    pub steps: usize,
    /// Candidates per step (judged in parallel).
    pub batch: usize,
    pub threads: usize,
    pub palette: Vec<String>,
    pub tol: f64,
    pub seed: u64,
    /// Moves per candidate (1..=this).
    pub moves: usize,
    /// Port chests may move (one cell at a time, to a free cell next to the design).
    pub move_ports: bool,
    /// The box the design may grow into around its own (cells).
    pub slack: i32,
    /// The starting temperature (energy units: 10 per block; 0 = greedy).
    pub temp: f64,
    /// A target face (axis 0/1/2, plane coordinate): the energy is 1000 per block left on it.
    pub face: Option<(usize, i32)>,
    /// Trickle sweeps required of every candidate ([`add_sweeps`]): a box every N gt, every phase.
    pub sweep: Vec<u32>,
    pub out_dir: Option<std::path::PathBuf>,
}

/// The objective (smaller is better): (volume, energy). The energy rewards emptying a face of
/// the box (a face emptied is a layer saved): 100 x the sum over the faces of sqrt(blocks on the
/// face), so a block taken off a nearly empty face counts most, plus 10 per block.
fn objective(d: &Design, inputs: &[Pos], s: &Score, face: Option<(usize, i32)>) -> (i64, i64) {
    // a target face (axis, plane): 1000 per block left on it, 10 per block
    if let Some((axis, plane)) = face {
        let n = d.blocks.keys().filter(|p| !inputs.contains(p) && p[axis] == plane).count();
        return (s.volume, 1000 * n as i64 + 10 * s.blocks as i64);
    }
    // a soft volume (face plane coordinate i32::MIN): 1000 per cell of the box in the energy, so
    // the annealing may grow the box for a while to rearrange it
    let soft = face.is_none() && SOFT_VOLUME.load(Ordering::Relaxed);
    let (lo, hi) = box_of(d, inputs);
    let mut e = 0.0f64;
    for axis in 0..3 {
        if hi[axis] == lo[axis] {
            continue;
        }
        for side in [lo[axis], hi[axis]] {
            let n = d.blocks.keys().filter(|p| !inputs.contains(p) && p[axis] == side).count();
            e += (n as f64).sqrt();
        }
    }
    let e = (100.0 * e).round() as i64 + 10 * s.blocks as i64;
    if soft { (0, 1000 * s.volume + e) } else { (s.volume, e) }
}

/// `anneal --soft-volume`: the volume is part of the energy instead of a hard bound.
static SOFT_VOLUME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set the soft-volume mode of [`objective`].
pub fn set_soft_volume(on: bool) {
    SOFT_VOLUME.store(on, Ordering::Relaxed);
}

/// One random edit of a design (`None` when the move does not apply).
fn mutate(d: &Design, rng: &mut Rng, o: &AnnealOptions, inputs: &[Pos], lo: Pos, hi: Pos) -> Option<Design> {
    let mut d = d.clone();
    let blocks: Vec<Pos> = d.blocks.keys().copied().filter(|p| !inputs.contains(p)).collect();
    let ports: Vec<(String, Pos)> = d.ports.iter().filter(|(n, _)| !n.starts_with("input")).map(|(n, p)| (n.clone(), *p)).collect();
    let kind = rng.below(if o.move_ports { 8 } else { 7 });
    let near = |d: &Design, p: Pos| -> bool { d.blocks.keys().any(|q| manhattan(*q, p) == 1) };
    let inside = |q: Pos, d: &Design| -> bool { (0..3).all(|i| q[i] >= lo[i] && q[i] <= hi[i]) && !d.blocks.contains_key(&q) && !d.ports.values().any(|x| *x == q) };
    let dirs = [[0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1], [1, 0, 0], [-1, 0, 0]];
    let ndirs = if hi[0] == lo[0] { 4 } else { 6 };
    match kind {
        // delete a block
        0 => {
            let p = blocks[rng.below(blocks.len())];
            d.blocks.remove(&p);
        }
        // re-state a block
        1 => {
            let p = blocks[rng.below(blocks.len())];
            let s = o.palette[rng.below(o.palette.len())].clone();
            if d.blocks.get(&p) == Some(&s) {
                return None;
            }
            d.blocks.insert(p, s);
        }
        // add a block next to the design, inside the box
        2 => {
            let cells: Vec<Pos> = free_cells(&d, lo, hi).into_iter().filter(|c| near(&d, *c)).collect();
            if cells.is_empty() {
                return None;
            }
            let p = cells[rng.below(cells.len())];
            d.blocks.insert(p, o.palette[rng.below(o.palette.len())].clone());
        }
        // move a block one or two cells (same state)
        3 | 4 => {
            let p = blocks[rng.below(blocks.len())];
            let dv = dirs[rng.below(ndirs)];
            let k = 1 + rng.below(2) as i32;
            let q = [p[0] + k * dv[0], p[1] + k * dv[1], p[2] + k * dv[2]];
            if !inside(q, &d) {
                return None;
            }
            let s = d.blocks.remove(&p)?;
            d.blocks.insert(q, s);
        }
        // swap two blocks within 2 cells
        5 => {
            let p = blocks[rng.below(blocks.len())];
            let near2: Vec<Pos> = blocks.iter().copied().filter(|q| *q != p && manhattan(*q, p) <= 2).collect();
            if near2.is_empty() {
                return None;
            }
            let q = near2[rng.below(near2.len())];
            let (a, b) = (d.blocks.get(&p)?.clone(), d.blocks.get(&q)?.clone());
            if a == b {
                return None;
            }
            d.blocks.insert(p, b);
            d.blocks.insert(q, a);
        }
        // shift a row or column segment of blocks one cell toward its free end
        6 => {
            let p = blocks[rng.below(blocks.len())];
            let dv = dirs[rng.below(ndirs)];
            // the run of blocks from p along dv, then a free cell to move into
            let mut run = vec![p];
            let mut q = [p[0] + dv[0], p[1] + dv[1], p[2] + dv[2]];
            while d.blocks.contains_key(&q) && run.len() < 6 {
                run.push(q);
                q = [q[0] + dv[0], q[1] + dv[1], q[2] + dv[2]];
            }
            if !inside(q, &d) || run.len() < 2 {
                return None;
            }
            let states: Vec<String> = run.iter().map(|x| d.blocks.remove(x).unwrap()).collect();
            for (x, s) in run.iter().zip(states) {
                d.blocks.insert([x[0] + dv[0], x[1] + dv[1], x[2] + dv[2]], s);
            }
        }
        // move a port chest one cell
        _ => {
            if ports.is_empty() {
                return None;
            }
            let (name, p) = ports[rng.below(ports.len())].clone();
            let dv = dirs[rng.below(4)];
            let q = [p[0] + dv[0], p[1] + dv[1], p[2] + dv[2]];
            if d.blocks.contains_key(&q) || d.ports.values().any(|x| *x == q) {
                return None;
            }
            d.ports.insert(name, q);
        }
    }
    Some(d)
}

/// `shulker anneal`: simulated annealing on (volume, energy) over designs that pass: a smaller
/// volume is always taken; at the same volume the lowest energy of the batch (ties at random) is
/// taken when not worse, or with probability exp(-dE / T) (T falls linearly from `--temp` to 0).
/// Every new best (volume, blocks) is saved.
pub fn anneal(path: &Path, o: &AnnealOptions) -> Result<(Score, Design)> {
    let c0 = Circuit::load(path)?;
    let mut t = target_of(&c0, o.tol)?;
    add_sweeps(&c0, &mut t, &o.sweep);
    let t = t;
    let inputs: Vec<Pos> = c0.inputs.iter().map(|i| i.pos).collect();
    let mut cur = c0.clock.design.clone();
    let mut cur_s = score_of(&c0, t.min_rate + o.tol);
    let mut cur_obj = objective(&cur, &inputs, &cur_s, o.face);
    let mut best = (cur_s.clone(), cur.clone());
    println!(
        "{}: {} blocks, {}x{}x{} = {}, objective {:?}, required: {}{} (rate >= {:.2})",
        c0.name(),
        cur_s.blocks,
        cur_s.dims[0],
        cur_s.dims[1],
        cur_s.dims[2],
        cur_s.volume,
        cur_obj,
        t.required.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" "),
        t.sweeps.iter().map(|s| format!(" {}@every-{}", s.name, s.every)).collect::<String>(),
        t.min_rate
    );
    let mut rng = Rng::new(o.seed);
    let t0 = std::time::Instant::now();
    let mut accepted = 0usize;
    for step in 0..o.steps {
        let temp = o.temp * (1.0 - step as f64 / o.steps as f64);
        let (blo, bhi) = box_of(&cur, &inputs);
        let lo = [blo[0] - if blo[0] == bhi[0] { 0 } else { o.slack }, blo[1] - o.slack, blo[2] - o.slack];
        let hi = [bhi[0] + if blo[0] == bhi[0] { 0 } else { o.slack }, bhi[1] + o.slack, bhi[2] + o.slack];
        // a batch of random candidates (seeds drawn here, the edits made in the workers)
        let seeds: Vec<u64> = (0..o.batch).map(|_| rng.next()).collect();
        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<((i64, i64), Score, Design)>> = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= seeds.len() {
                        break;
                    }
                    let mut r = Rng::new(seeds[i]);
                    let n = 1 + r.below(o.moves);
                    let mut d = cur.clone();
                    let mut ok = true;
                    for _ in 0..n {
                        match mutate(&d, &mut r, o, &inputs, lo, hi) {
                            Some(x) => d = x,
                            None => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if !ok || d.blocks == cur.blocks && d.ports == cur.ports {
                        continue;
                    }
                    let Some(c) = circuit_of(&d) else { continue };
                    if let Some(sc) = judge(&c, &t) {
                        let obj = objective(&d, &inputs, &sc, o.face);
                        results.lock().unwrap().push((obj, sc, d));
                    }
                });
            }
        });
        let mut results = results.into_inner().unwrap();
        results.sort_by(|a, b| a.0.cmp(&b.0));
        let Some(first) = results.first().map(|x| x.0) else { continue };
        // the lowest objective; among equals one at random
        let ties = results.iter().take_while(|x| x.0 == first).count();
        let (obj, sc, d) = results.swap_remove(rng.below(ties));
        let take = obj.0 < cur_obj.0
            || (obj.0 == cur_obj.0 && (obj.1 <= cur_obj.1 || (temp > 0.0 && (rng.next() % 1_000_000) as f64 / 1e6 < (-((obj.1 - cur_obj.1) as f64) / temp).exp())));
        if !take {
            continue;
        }
        cur = d;
        cur_s = sc;
        cur_obj = obj;
        accepted += 1;
        if (cur_s.volume, cur_s.blocks) < (best.0.volume, best.0.blocks) {
            best = (cur_s.clone(), cur.clone());
            println!(
                "step {step}: {} blocks, {}x{}x{} = {}, objective {:?}, {:.2} items/s ({:.0} s)",
                cur_s.blocks,
                cur_s.dims[0],
                cur_s.dims[1],
                cur_s.dims[2],
                cur_s.volume,
                cur_obj,
                cur_s.rate,
                t0.elapsed().as_secs_f64()
            );
            if let Some(dir) = &o.out_dir {
                std::fs::create_dir_all(dir)?;
                let mut dd = cur.clone();
                dd.name = format!("{}-v{}-b{}-s{step}", c0.name(), cur_s.volume, cur_s.blocks);
                dd.save(&dir.join(format!("{}.txt", dd.name)))?;
            }
        }
        if step % 50 == 49 {
            eprintln!("  step {}: {accepted} accepted, objective {cur_obj:?}, T {temp:.1} ({:.0} s)", step + 1, t0.elapsed().as_secs_f64());
        }
    }
    let (s, mut d) = best;
    d.name = format!("{}-v{}-b{}", c0.name(), s.volume, s.blocks);
    Ok((s, d))
}

/// How much of the screen scenario a (possibly broken) design gets done: full boxes out (a
/// loader) or empty boxes out (an unloader), the items moved, wrong deliveries counted against
/// it; a passing screen adds 100000. Blocks without support are taken out first (they would pop)
/// at 1000 each.
pub fn grade(c: &Circuit, t: &Target) -> i64 {
    let (_, sc) = &t.screen;
    let (from, to) = circuit::run::watch_box(c, 3);
    let loose = clock::sim::unsupported(&c.clock, from, to).unwrap_or_default();
    let popped;
    let c = if loose.is_empty() {
        c
    } else {
        let mut d = c.clock.design.clone();
        for (p, _) in &loose {
            d.blocks.remove(p);
        }
        match Circuit::from_design(d) {
            Ok(x) => {
                popped = x;
                &popped
            }
            Err(_) => return -1_000_000,
        }
    };
    let mut boxes: std::collections::HashMap<(i32, i32, i32), (u64, bool)> = std::collections::HashMap::new();
    let (mut placed, mut done_boxes, mut broke_ok, mut broke_bad) = (0i64, 0i64, 0i64, 0i64);
    let loader = sc.loader;
    let stack = sc.stack;
    let mut hook = |_t: u32, w: &redsim::world::World| {
        let mut now: std::collections::HashMap<(i32, i32, i32), (u64, bool)> = std::collections::HashMap::new();
        for (p, be) in &w.containers.map {
            if redsim::redstone::shulker::box_item_of(&w.get(*p)).is_none() {
                continue;
            }
            let items: u64 = be.slots.iter().flatten().map(|s| s.count as u64).sum();
            let full = be.slots.len() == 27 && be.slots.iter().all(|s| s.is_some_and(|s| s.count as u32 == stack));
            now.insert((p.x, p.y, p.z), (items, full));
        }
        for (p, (items, full)) in &now {
            match boxes.get(p) {
                None => placed += 1,
                Some((pi, pf)) => {
                    if (loader && *full && !*pf) || (!loader && *items == 0 && *pi > 0) {
                        done_boxes += 1;
                    }
                }
            }
        }
        for (p, (items, full)) in &boxes {
            if !now.contains_key(p) {
                if (loader && *full) || (!loader && *items == 0) {
                    broke_ok += 1;
                } else {
                    broke_bad += 1;
                }
            }
        }
        boxes = now;
    };
    let Ok(log) = rs::run_with(c, sc, rs::RunOpts { cap: Some(t.screen_cap), ..Default::default() }, Some(&mut hook)) else { return -1_000_000 };
    let mut g = -1000 * loose.len() as i64 + 1000 * placed.min(4) + 1000 * done_boxes.min(3) + 2000 * broke_ok.min(3) - 3000 * broke_bad;
    for d in &log.deliveries {
        if !d.stack.is_box() {
            g -= 2000;
            continue;
        }
        let n = sc.counts(&d.stack) as i64;
        let good = if sc.loader { d.stack.contents.len() == 27 && d.stack.contents.iter().all(|s| s.count == sc.stack) } else { n == 0 };
        g += if good { 3000 } else { -500 };
    }
    g += 10 * log.flow.last().map(|x| x.1 as i64).unwrap_or(0);
    if loose.is_empty() && !log.timed_out && circuit::shulker::judge(sc, &log).pass {
        g += 100_000;
    }
    g
}

/// `shulker repair` settings.
pub struct RepairOptions {
    /// The box the design must fit (the blocks outside it are removed first).
    pub bbox: (Pos, Pos),
    pub steps: usize,
    pub batch: usize,
    pub threads: usize,
    pub palette: Vec<String>,
    pub tol: f64,
    pub seed: u64,
    pub moves: usize,
    pub temp: f64,
    /// Edits only within this distance of the cut blocks.
    pub local: i32,
    /// The design whose scenarios and rate the result must match (default: the input's own).
    pub target: Option<std::path::PathBuf>,
    pub out_dir: Option<std::path::PathBuf>,
}

/// `shulker repair`: the design cut down to `--box` (its blocks outside removed), then a search
/// inside the box (the anneal's edits) on the graded screen ([`grade`], less 5 per block) until a
/// candidate passes the full target; every full pass is saved (the box is smaller than the
/// original's, so each is a smaller volume).
pub fn repair(path: &Path, o: &RepairOptions) -> Result<Vec<(Score, Design)>> {
    let c0 = Circuit::load(path)?;
    // the target: from a reference design that passes (a hand seed that does not pass yet is
    // repaired against the design it should replace)
    let mut t = match &o.target {
        Some(r) => target_of(&Circuit::load(r)?, o.tol)?,
        None => target_of(&c0, o.tol)?,
    };
    // graded on the hardest required run: the endless supply when required (a seed that jams
    // there scores its placed and delivered boxes in that very run)
    if let Some((n, sc)) = t.required.iter().find(|(n, _)| n == "endless").cloned() {
        t.screen_cap = sc.cap;
        t.screen = (n, sc);
    }
    let inputs: Vec<Pos> = c0.inputs.iter().map(|i| i.pos).collect();
    let (lo, hi) = o.bbox;
    let mut cur = c0.clock.design.clone();
    let cut: Vec<Pos> = cur.blocks.keys().copied().filter(|p| !inputs.contains(p) && (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i])).collect();
    for p in &cut {
        cur.blocks.remove(p);
    }
    let energy = |g: i64, _d: &Design| -> i64 { -g };
    let mut cur_g = circuit_of(&cur).map(|c| grade(&c, &t)).unwrap_or(-1_000_000);
    let mut cur_e = energy(cur_g, &cur);
    // edits only near the cut (the rest of the design stays as it works)
    let focus: Vec<Pos> = if cut.is_empty() { cur.blocks.keys().copied().collect() } else { cut.clone() };
    let near_focus = |p: &Pos| focus.iter().any(|q| manhattan(*q, *p) <= o.local);
    let base_blocks = cur.blocks.clone();
    let local_only = |d: &Design| -> bool {
        d.blocks.iter().all(|(p, s)| base_blocks.get(p) == Some(s) || near_focus(p)) && base_blocks.iter().all(|(p, s)| d.blocks.get(p) == Some(s) || near_focus(p))
    };
    println!("{}: cut to {lo:?}..{hi:?} ({} blocks removed), grade {cur_g}", c0.name(), cut.len());
    let ao = AnnealOptions { steps: o.steps, batch: o.batch, threads: o.threads, palette: o.palette.clone(), tol: o.tol, seed: o.seed, moves: o.moves, move_ports: false, slack: 0, temp: o.temp, face: None, sweep: Vec::new(), out_dir: None };
    let mut rng = Rng::new(o.seed);
    let mut finds: Vec<(Score, Design)> = Vec::new();
    let t0 = std::time::Instant::now();
    for step in 0..o.steps {
        let temp = o.temp * (1.0 - step as f64 / o.steps as f64);
        let seeds: Vec<u64> = (0..o.batch).map(|_| rng.next()).collect();
        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<(i64, i64, Design)>> = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= seeds.len() {
                        break;
                    }
                    let mut r = Rng::new(seeds[i]);
                    let n = 1 + r.below(o.moves);
                    let mut d = cur.clone();
                    for _ in 0..n {
                        if let Some(x) = mutate(&d, &mut r, &ao, &inputs, lo, hi) {
                            d = x;
                        }
                    }
                    if d.blocks == cur.blocks || !local_only(&d) {
                        continue;
                    }
                    let Some(c) = circuit_of(&d) else { continue };
                    let g = grade(&c, &t);
                    results.lock().unwrap().push((energy(g, &d), g, d));
                });
            }
        });
        let mut results = results.into_inner().unwrap();
        results.sort_by_key(|x| x.0);
        // full passes: judged against the whole target, saved
        for (_, g, d) in results.iter().filter(|x| x.1 >= 100_000) {
            if let Some(c) = circuit_of(d)
                && let Some(sc) = judge(&c, &t)
                && !finds.iter().any(|(_, f)| f.blocks == d.blocks)
            {
                println!("step {step}: PASS {} blocks, {}x{}x{} = {}, {:.2} items/s, grade {g} ({:.0} s)", sc.blocks, sc.dims[0], sc.dims[1], sc.dims[2], sc.volume, sc.rate, t0.elapsed().as_secs_f64());
                if let Some(dir) = &o.out_dir {
                    std::fs::create_dir_all(dir)?;
                    let mut dd = d.clone();
                    dd.name = format!("{}-v{}-b{}-r{step}", c0.name(), sc.volume, sc.blocks);
                    dd.save(&dir.join(format!("{}.txt", dd.name)))?;
                }
                finds.push((sc, d.clone()));
            }
        }
        let Some(e_best) = results.first().map(|x| x.0) else { continue };
        let ties = results.iter().take_while(|x| x.0 == e_best).count();
        let (e, g, d) = results.swap_remove(rng.below(ties));
        let take = e <= cur_e || (temp > 0.0 && (rng.next() % 1_000_000) as f64 / 1e6 < (-((e - cur_e) as f64) / temp).exp());
        if take {
            cur = d;
            cur_e = e;
            cur_g = g;
        }
        if step % 50 == 49 {
            eprintln!("  step {}: grade {cur_g}, {} blocks, T {temp:.0}, {} passes ({:.0} s)", step + 1, cur.blocks.len(), finds.len(), t0.elapsed().as_secs_f64());
        }
    }
    finds.sort_by_key(|(s, _)| (s.volume, s.blocks));
    Ok(finds)
}
