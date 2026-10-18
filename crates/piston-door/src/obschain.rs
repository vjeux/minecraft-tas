//! `mctas door obs-chain <spec>`: wire a pulse-driven mechanism with a tree of observers.
//!
//! Every piston group of the mechanism is fired by one *driver* observer (vetted by hand: its
//! output reaches exactly the intended pistons, directly or by quasi-connectivity) that must fire
//! at a given time. An observer fires 2 gt after the block it watches changes, and a chain of
//! observers (each watching the previous one) carries one 2 gt pulse 2 gt per observer. So the
//! whole control is a tree rooted at an observer watching the lever (one pulse on BOTH lever
//! edges): each driver at time T watches a tree observer at time T-1. Observers need no support,
//! so the search is a pure geometry problem: exact-length paths through free cells.
//!
//! Spec (`#` comments):
//! ```text
//! schedule designs/.../shift2.ideal   # ideal-input schedule: its design is the mechanism, the
//!                                     # replay gives the cells the machine uses (forbidden)
//! lever x y z <state>                 # the lever (port) ...
//! support x y z <state>               # ... the block it hangs on
//! root x y z <facing>                 # the observer watching the lever (time 0)
//! box x0 y0 z0 x1 y1 z1               # where observers may go
//! group <name> <time>                 # a driver firing at <time> (2 gt units after the root)
//! opt x y z <facing>                  #   its options: observer cell and the way it faces
//! forbid x y z                        # extra cells no observer may take
//! ```
//! Cells forbidden for chain observers: the mechanism's cells (any tick of the replay), the cell
//! in front of every piston position and the one beyond it, the door columns (x 0..2, y 0..2,
//! any z: nothing in front of or behind the door), y < 0. A chain observer's output cell (behind
//! it) must not be a mechanism cell, a cell right above a piston position (quasi-connectivity),
//! or the lever or its support (an output down into the ground check's floor is harmless).

use crate::ideal::Schedule;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{Pos, Worker};
use std::collections::{BTreeMap, HashMap, HashSet};

const DIRS: [(Pos, &str); 6] =
    [([1, 0, 0], "east"), ([-1, 0, 0], "west"), ([0, 1, 0], "up"), ([0, -1, 0], "down"), ([0, 0, 1], "south"), ([0, 0, -1], "north")];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dist(a: Pos, b: Pos) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}
fn dir_of(name: &str) -> Result<Pos> {
    DIRS.iter().find(|(_, n)| *n == name).map(|(d, _)| *d).with_context(|| format!("unknown facing {name:?}"))
}
fn name_of(d: Pos) -> &'static str {
    DIRS.iter().find(|(v, _)| *v == d).map(|(_, n)| *n).unwrap()
}
fn observer(facing: Pos) -> String {
    format!("minecraft:observer[facing={},powered=false]", name_of(facing))
}

#[derive(Clone, Debug)]
pub struct Group {
    pub name: String,
    pub time: i32,
    pub opts: Vec<(Pos, Pos)>,
    /// Per option: a conductor block the driver powers (`optb`), or none.
    pub conductors: Vec<Option<(Pos, String)>>,
}

pub struct Spec {
    pub schedule: std::path::PathBuf,
    pub lever: (Pos, String),
    pub support: (Pos, String),
    pub root: (Pos, Pos),
    pub lo: Pos,
    pub hi: Pos,
    pub groups: Vec<Group>,
    pub forbid: Vec<Pos>,
}

fn p3(v: &[&str]) -> Result<Pos> {
    Ok([v[0].parse()?, v[1].parse()?, v[2].parse()?])
}

pub fn parse_spec(text: &str) -> Result<Spec> {
    let mut schedule = None;
    let mut lever = None;
    let mut support = None;
    let mut root = None;
    let (mut lo, mut hi) = ([-8, 0, -5], [10, 7, 5]);
    let mut groups: Vec<Group> = Vec::new();
    let mut forbid = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let v: Vec<&str> = line.split_whitespace().collect();
        let ctx = || format!("spec line {}: {line}", i + 1);
        match v[0] {
            "schedule" => schedule = Some(std::path::PathBuf::from(v[1])),
            "lever" => lever = Some((p3(&v[1..4]).with_context(ctx)?, v[4].to_string())),
            "support" => support = Some((p3(&v[1..4]).with_context(ctx)?, v[4].to_string())),
            "root" => root = Some((p3(&v[1..4]).with_context(ctx)?, dir_of(v[4])?)),
            "box" => {
                lo = p3(&v[1..4]).with_context(ctx)?;
                hi = p3(&v[4..7]).with_context(ctx)?;
            }
            "group" => groups.push(Group { name: v[1].to_string(), time: v[2].parse().with_context(ctx)?, opts: Vec::new(), conductors: Vec::new() }),
            "opt" => {
                let g = groups.last_mut().with_context(ctx)?;
                g.opts.push((p3(&v[1..4]).with_context(ctx)?, dir_of(v[4])?));
                g.conductors.push(None);
            }
            "optb" => {
                let g = groups.last_mut().with_context(ctx)?;
                g.opts.push((p3(&v[1..4]).with_context(ctx)?, dir_of(v[4])?));
                g.conductors.push(Some((p3(&v[5..8]).with_context(ctx)?, v[8].to_string())));
            }
            "forbid" => forbid.push(p3(&v[1..4]).with_context(ctx)?),
            x => bail!("spec line {}: unknown keyword {x:?}", i + 1),
        }
    }
    Ok(Spec {
        schedule: schedule.context("spec: no schedule")?,
        lever: lever.context("spec: no lever")?,
        support: support.context("spec: no support")?,
        root: root.context("spec: no root")?,
        lo,
        hi,
        groups,
        forbid,
    })
}

/// The search state: observers placed so far (cell -> (time, facing)).
#[derive(Clone)]
struct Tree {
    obs: BTreeMap<Pos, (i32, Pos)>,
    /// Conductor blocks placed by `optb` drivers.
    extra: BTreeMap<Pos, String>,
}

struct Ctx {
    blocked: HashSet<Pos>,
    no_out: HashSet<Pos>,
    lo: Pos,
    hi: Pos,
    groups: Vec<Group>,
    max_paths: usize,
    best: Option<(usize, Tree, Vec<usize>)>,
    nodes: usize,
    deepest: usize,
    deadline: std::time::Instant,
    seed: u64,
}

impl Ctx {
    fn in_box(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
    }
    fn free(&self, t: &Tree, p: Pos) -> bool {
        self.in_box(p) && !self.blocked.contains(&p) && !t.obs.contains_key(&p) && !t.extra.contains_key(&p)
    }
    /// Exact-length paths of new observers from a tree node (time i) to `target` (time `tt`).
    fn paths(&self, t: &Tree, target: Pos, tt: i32, reserved: &HashSet<Pos>) -> Vec<Vec<(Pos, Pos)>> {
        let mut out = Vec::new();
        // True distances to the target through free cells (the DFS prune: fails fast when the
        // target is out of reach, instead of enumerating every wandering path).
        let mut dt: HashMap<Pos, i32> = HashMap::from([(target, 0)]);
        let mut q = std::collections::VecDeque::from([target]);
        while let Some(c) = q.pop_front() {
            let dc = dt[&c];
            if dc >= tt + 1 {
                continue;
            }
            for (d, _) in DIRS {
                let n = add(c, d);
                if dt.contains_key(&n) || !self.free(t, n) {
                    continue;
                }
                dt.insert(n, dc + 1);
                q.push_back(n);
            }
        }
        let mut srcs: Vec<(Pos, i32)> = t.obs.iter().filter(|(_, (ti, _))| *ti < tt).map(|(p, (ti, _))| (*p, *ti)).collect();
        // Latest sources first: fewest new observers.
        srcs.sort_by_key(|(p, ti)| (-ti, dist(*p, target)));
        let mut budget = 200_000usize;
        for (src, ti) in srcs {
            let len = tt - ti;
            // The source's best free neighbour must reach the target in len-1 steps.
            let near = DIRS.iter().filter_map(|(d, _)| dt.get(&add(src, *d))).min().copied();
            if near.is_none_or(|n| n > len - 1) || (dist(src, target) - len) % 2 != 0 {
                continue;
            }
            let mut path = Vec::new();
            self.dfs_path(t, src, target, len, &mut path, &mut out, reserved, &dt, &mut budget);
            if out.len() >= self.max_paths || budget == 0 {
                break;
            }
        }
        out.sort_by_key(|p| p.len());
        if std::env::var("OBS_DEBUG").is_ok() {
            eprintln!("[debug] paths to {:?} at t{}: {} found", target, tt, out.len());
        }
        out
    }
    #[allow(clippy::too_many_arguments)]
    fn dfs_path(
        &self,
        t: &Tree,
        cur: Pos,
        target: Pos,
        left: i32,
        path: &mut Vec<(Pos, Pos)>,
        out: &mut Vec<Vec<(Pos, Pos)>>,
        reserved: &HashSet<Pos>,
        dt: &HashMap<Pos, i32>,
        budget: &mut usize,
    ) {
        if out.len() >= self.max_paths || *budget == 0 {
            return;
        }
        *budget -= 1;
        if left == 0 {
            if cur == target {
                out.push(path.clone());
            }
            return;
        }
        let h = mix(self.seed ^ ((cur[0] as u64) << 40) ^ ((cur[1] as u64 & 0xff) << 32) ^ ((cur[2] as u64 & 0xffff) << 16) ^ left as u64);
        for di in shuffled(6, if self.seed == 0 { 0 } else { h }) {
            let d = DIRS[di].0;
            let n = add(cur, d);
            if dt.get(&n).is_none_or(|x| *x > left - 1) {
                continue;
            }
            if !self.free(t, n) || path.iter().any(|(q, _)| *q == n) || (reserved.contains(&n) && n != target) {
                continue;
            }
            // n faces cur (watches it); its output goes out the back.
            let facing = sub(cur, n);
            let outc = sub(n, facing);
            // An output into the floor (y = -1, the ground check's stone) is harmless: the only
            // cell above that floor block is this observer.
            if self.no_out.contains(&outc) || t.extra.contains_key(&outc) {
                continue;
            }
            path.push((n, facing));
            self.dfs_path(t, n, target, left - 1, path, out, reserved, dt, budget);
            path.pop();
        }
    }
    fn search(&mut self, t: &Tree, gi: usize, choice: &mut Vec<usize>) {
        self.nodes += 1;
        self.deepest = self.deepest.max(gi);
        if std::time::Instant::now() > self.deadline {
            return;
        }
        let count = t.obs.len() + t.extra.len();
        if let Some((b, _, _)) = &self.best {
            if count + (self.groups.len() - gi) >= *b {
                return;
            }
        }
        if gi == self.groups.len() {
            self.best = Some((count, t.clone(), choice.clone()));
            eprintln!("[obs-chain] {} observers", count);
            return;
        }
        // Forward check: every later group keeps enough viable options (groups sharing one option
        // list need that many distinct viable cells).
        {
            let mut need: HashMap<Vec<(Pos, Pos)>, (usize, usize)> = HashMap::new();
            for g in &self.groups[gi..] {
                let viable = g
                    .opts
                    .iter()
                    .filter(|(c, f)| {
                        let w = add(*c, *f);
                        !t.obs.contains_key(c) && !t.extra.contains_key(c) && !self.blocked.contains(c) && self.in_box(*c)
                            && match t.obs.get(&w) {
                                Some((ti, _)) => *ti == g.time - 1,
                                None => self.free(t, w),
                            }
                    })
                    .count();
                let e = need.entry(g.opts.clone()).or_insert((0, viable));
                e.0 += 1;
            }
            if need.values().any(|(n, v)| v < n) {
                if std::env::var("OBS_DEBUG2").is_ok() {
                    for g in &self.groups[gi..] {
                        if let Some((n, v)) = need.get(&g.opts) {
                            if v < n {
                                eprintln!("[fc] at group {gi}: {} needs {n} viable options, has {v}", g.name);
                                for (c, fa) in &g.opts {
                                    let w = add(*c, *fa);
                                    eprintln!("[fc]    opt {:?}: cell obs {:?} extra {} blocked {} inbox {}; watched {:?} obs {:?} free {}", c, t.obs.get(c), t.extra.contains_key(c), self.blocked.contains(c), self.in_box(*c), w, t.obs.get(&w), self.free(t, w));
                                }
                            }
                        }
                    }
                }
                return;
            }
        }
        if false {
            self.best = Some((count, t.clone(), choice.clone()));
            eprintln!("[obs-chain] {} observers", count);
            return;
        }
        let g = self.groups[gi].clone();
        // Cells of later drivers' options stay free as long as possible (soft: tried later).
        let order = shuffled(g.opts.len(), if self.seed == 0 { 0 } else { mix(self.seed ^ (gi as u64) ^ ((count as u64) << 20)) });
        for oi in order {
            let (cell, facing) = (&g.opts[oi].0, &g.opts[oi].1);
            let watched = add(*cell, *facing);
            if let Some((ti, _)) = t.obs.get(cell) {
                // Already an observer there: usable only if it is exactly this driver.
                let _ = ti;
                continue;
            }
            if !self.in_box(*cell) || self.blocked.contains(cell) {
                if std::env::var("OBS_DEBUG").is_ok() {
                    eprintln!("[debug] {} opt {:?}: cell blocked", g.name, cell);
                }
                continue;
            }
            if std::env::var("OBS_DEBUG").is_ok() {
                let st = match t.obs.get(&watched) {
                    Some((ti, _)) => format!("watched holds an observer at t{ti}"),
                    None if self.free(t, watched) => "watched free".to_string(),
                    None => format!("watched {:?} blocked", watched),
                };
                eprintln!("[debug] {} opt {:?} (t{}): {st}", g.name, cell, g.time);
            }
            let mut base = t.clone();
            base.obs.insert(*cell, (g.time, *facing));
            if let Some((cp, cs)) = &g.conductors[oi] {
                // The conductor: a free cell no placed observer outputs into.
                if !self.free(t, *cp) || *cp == *cell || *cp == watched || t.obs.iter().any(|(p, (_, f))| sub(*p, *f) == *cp) {
                    continue;
                }
                base.extra.insert(*cp, cs.clone());
            }
            choice.push(oi);
            match t.obs.get(&watched) {
                Some((ti, _)) if *ti == g.time - 1 => {
                    self.search(&base, gi + 1, choice);
                }
                Some(_) => {}
                None => {
                    if self.free(&base, watched) {
                        // Soft reservation: the option cells and watched cells of later groups that
                        // have no spare option (as many viable options as groups sharing them).
                        let mut classes: HashMap<Vec<(Pos, Pos)>, (usize, usize)> = HashMap::new();
                        for h in &self.groups[gi + 1..] {
                            let viable = h
                                .opts
                                .iter()
                                .filter(|(c, f)| {
                                    let w = add(*c, *f);
                                    !base.obs.contains_key(c) && !base.extra.contains_key(c) && !self.blocked.contains(c) && self.in_box(*c)
                                        && match base.obs.get(&w) {
                                            Some((ti, _)) => *ti == h.time - 1,
                                            None => self.free(&base, w),
                                        }
                                })
                                .count();
                            let e = classes.entry(h.opts.clone()).or_insert((0, viable));
                            e.0 += 1;
                        }
                        let reserved: HashSet<Pos> = self.groups[gi + 1..]
                            .iter()
                            .filter(|h| classes.get(&h.opts).is_some_and(|(n, v)| v <= n))
                            .flat_map(|h| h.opts.iter().flat_map(|(c, f)| [*c, add(*c, *f)]))
                            .filter(|p| *p != watched)
                            .collect();
                        // Paths end at `watched` (time T-1); they may not pass through the driver.
                        let mut paths = self.paths(&base, watched, g.time - 1, &reserved);
                        if paths.is_empty() {
                            paths = self.paths(&base, watched, g.time - 1, &HashSet::new());
                        }
                        for p in paths {
                            let mut nt = base.clone();
                            let mut ok = true;
                            let mut tt = g.time - 1 - p.len() as i32;
                            for (q, f) in &p {
                                tt += 1;
                                if nt.obs.insert(*q, (tt, *f)).is_some() {
                                    ok = false;
                                }
                            }
                            if ok {
                                self.search(&nt, gi + 1, choice);
                            }
                        }
                    }
                }
            }
            choice.pop();
        }
    }
}

pub fn run(w: &mut Worker, spec_path: &std::path::Path, out: &std::path::Path, max_paths: usize, secs: u64, shift: i32, restarts: usize) -> Result<()> {
    let spec = parse_spec(&std::fs::read_to_string(spec_path)?)?;
    let sched = Schedule::parse_with(&std::fs::read_to_string(&spec.schedule)?, &BTreeMap::new())?;
    let vars = sched.vars().clone();
    let mech = crate::mechwire::measure_ideal(w, &sched, &vars)?;
    // The mechanism: the schedule's rest design without its lever and without the redstone
    // blocks it holds at rest (input cells that hold a mechanism block at rest keep it: an
    // update trick may set and clear a block in a piston's home cell).
    let mut d0: Design = sched.design().clone();
    d0.ports.remove("lever");
    d0.blocks.retain(|_, s| !s.contains("redstone_block") && !s.contains("lever"));
    let mut blocked: HashSet<Pos> = mech.meas.occupied.iter().copied().collect();
    for p in d0.blocks.keys() {
        blocked.insert(*p);
    }
    let mut no_out: HashSet<Pos> = blocked.clone();
    for (p, f) in &mech.meas.pistons {
        if *f == crate::route2::READ_LATENT || *f == crate::route2::READ_DIRECT || *f == crate::route2::READ_QC {
            continue;
        }
        no_out.insert(*p);
        no_out.insert(add(*p, [0, 1, 0]));
    }
    // Every piston position at any tick: the cell in front and the one beyond.
    for p in &mech.meas.occupied {
        if let Some(s) = sched.design().blocks.get(p) {
            let _ = s;
        }
    }
    for (p, f) in &mech.meas.pistons {
        if [*f].iter().any(|f| f.iter().map(|c| c.abs()).sum::<i32>() == 1) {
            blocked.insert(add(*p, *f));
            blocked.insert(add(add(*p, *f), *f));
        }
    }
    for x in 0..=2 {
        for y in 0..=2 {
            for z in spec.lo[2] - 3..=spec.hi[2] + 3 {
                blocked.insert([x, y, z]);
            }
        }
    }
    for p in &spec.forbid {
        blocked.insert(*p);
    }
    blocked.insert(spec.lever.0);
    blocked.insert(spec.support.0);
    no_out.insert(spec.lever.0);
    no_out.insert(spec.support.0);
    let mut groups = spec.groups.clone();
    for g in &mut groups {
        g.time += shift;
    }
    groups.sort_by_key(|g| g.time);
    let mut t = Tree { obs: BTreeMap::new(), extra: BTreeMap::new() };
    let (rc, rf) = spec.root;
    anyhow::ensure!(add(rc, rf) == spec.lever.0, "the root must face the lever");
    t.obs.insert(rc, (0, rf));
    let mut ctx = Ctx {
        blocked,
        no_out,
        lo: spec.lo,
        hi: spec.hi,
        groups: groups.clone(),
        max_paths,
        best: None,
        nodes: 0,
        deepest: 0,
        deadline: std::time::Instant::now() + std::time::Duration::from_secs(secs),
        seed: 0,
    };
    let mut choice = Vec::new();
    if std::env::var("OBS_DEBUG").is_ok() {
        for y in (ctx.lo[1]..=ctx.hi[1]).rev() {
            eprintln!("[map] y={y} (rows z {}..{}, columns x {}..{}; B blocked, n no output into, . free)", ctx.lo[2], ctx.hi[2], ctx.lo[0], ctx.hi[0]);
            for z in ctx.lo[2]..=ctx.hi[2] {
                let row: String = (ctx.lo[0]..=ctx.hi[0])
                    .map(|x| {
                        let p = [x, y, z];
                        if ctx.blocked.contains(&p) {
                            'B'
                        } else if ctx.no_out.contains(&p) {
                            'n'
                        } else {
                            '.'
                        }
                    })
                    .collect();
                eprintln!("[map]   {row}");
            }
        }
    }
    let restarts = restarts.max(1);
    for r in 0..restarts {
        ctx.seed = if r == 0 { 0 } else { mix(r as u64 * 0x9e37_79b9_7f4a_7c15) | 1 };
        ctx.deadline = std::time::Instant::now() + std::time::Duration::from_secs((secs / restarts as u64).max(1));
        choice.clear();
        ctx.search(&t, 0, &mut choice);
    }
    eprintln!("[obs-chain] {} search nodes, deepest group reached: {} ({})", ctx.nodes, ctx.deepest, groups.get(ctx.deepest).map(|g| g.name.as_str()).unwrap_or("all"));
    let Some((n, tree, ch)) = ctx.best else {
        bail!("no wiring found");
    };
    let mut d = d0.clone();
    d.blocks.insert(spec.lever.0, spec.lever.1.clone());
    d.blocks.insert(spec.support.0, spec.support.1.clone());
    d.ports.insert("lever".into(), spec.lever.0);
    for (p, (_, f)) in &tree.obs {
        d.blocks.insert(*p, observer(*f));
    }
    for (p, s) in &tree.extra {
        d.blocks.insert(*p, s.clone());
    }
    d.name = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let d = crate::canonicalize(w, &d)?;
    d.save(out)?;
    println!("{n} observers ({} blocks), written to {}", crate::block_count(&d), out.display());
    for (g, c) in groups.iter().zip(ch.iter()) {
        let (p, f) = g.opts[*c];
        println!("  {} at t{}: observer {:?} facing {}", g.name, g.time, p, name_of(f));
    }
    let _ = HashMap::<i32, i32>::new();
    Ok(())
}

/// A 64-bit mix (splitmix64's finalizer).
fn mix(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// 0..n in an order drawn from `h` (0: the natural order).
fn shuffled(n: usize, h: u64) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    if h == 0 {
        return v;
    }
    let mut s = h;
    for i in (1..n).rev() {
        s = mix(s.wrapping_add(0x9e37_79b9_7f4a_7c15));
        let j = (s % (i as u64 + 1)) as usize;
        v.swap(i, j);
    }
    v
}
