//! `mctas circuit pext stack --n N`: a SLIME-FREE plain-stack extender built from an interleaved
//! static schedule (`trains::interleave`): sticky pistons A (x=1), P2..PN (x=2..N) and the payload
//! (x=N+1) in a line (y=1, z=0, travel +x); every pulse of the schedule is a fixed observer next to
//! its cell (sides, top, bottom; A also from its back), fired 2 gt after a spine observer passes
//! the cell it watches; one spine from the lever visits the watched cells in schedule order, routed
//! by breadth-first search around everything else, at least `hops` cells between two pulses (the
//! steps of one edge need ~6 gt to land).

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use anyhow::{Result, anyhow, bail};
use mctas::design::Design;

use crate::trains;

pub type Pos = [i32; 3];

fn obs(f: &str) -> String {
    format!("minecraft:observer[facing={f}]")
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

const DIRS: [(Pos, &str); 6] = [([1, 0, 0], "east"), ([-1, 0, 0], "west"), ([0, 1, 0], "up"), ([0, -1, 0], "down"), ([0, 0, 1], "south"), ([0, 0, -1], "north")];

fn facing_towards(from: Pos, to: Pos) -> Result<&'static str> {
    let d = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    DIRS.iter().find(|(v, _)| *v == d).map(|(_, f)| *f).ok_or_else(|| anyhow!("{from:?} -> {to:?} not adjacent"))
}

pub struct StackOptions {
    pub n: usize,
    pub max_train: usize,
    /// Minimum spine cells between two consecutive pulses.
    pub hops: usize,
    /// Which interleavable train to use (0 = the one with the fewest pulses).
    pub pick: usize,
}

pub fn generate(o: &StackOptions) -> Result<(Design, Vec<String>)> {
    let n = o.n;
    let ni = n as i32;
    let all = trains::trains(n, o.max_train);
    let mut found: Vec<(Vec<usize>, Vec<(i32, bool, bool)>)> = all.iter().filter_map(|(t, _)| trains::interleave(n, t).map(|p| (t.clone(), p))).collect();
    found.sort_by_key(|(t, p)| (p.len(), t.len(), t.clone()));
    let (train, plan) = found.get(o.pick).cloned().ok_or_else(|| anyhow!("no interleavable train for n = {n} up to {} pulses", o.max_train))?;
    let mut log = vec![format!(
        "train {}: {} pulses",
        train.iter().map(|&k| (b'A' + k as u8) as char).collect::<String>(),
        plan.len()
    )];
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    // machine: abstract position p -> x = p + 1
    b.insert([1, 1, 0], "minecraft:sticky_piston[facing=east]".into());
    for k in 2..=ni {
        b.insert([k, 1, 0], "minecraft:sticky_piston[facing=east]".into());
    }
    let payload = [ni + 1, 1, 0];
    b.insert(payload, "minecraft:gold_block".into());
    let target_x = ni + 1 + ni;
    // the line and the payload's path are off limits
    let mut blocked: BTreeSet<Pos> = (0..=target_x + 1).map(|x| [x, 1, 0]).collect();
    // pulser slots per cell, in order of use
    let slots = |x: i32| -> Vec<(Pos, &'static str, Pos)> {
        let mut v = vec![([x, 1, 1], "south", [x, 1, 2]), ([x, 1, -1], "north", [x, 1, -2]), ([x, 2, 0], "up", [x, 3, 0]), ([x, 0, 0], "down", [x, -1, 0])];
        if x == 1 {
            v.insert(0, ([0, 1, 0], "west", [-1, 1, 0]));
        }
        v
    };
    let mut used: HashMap<i32, usize> = HashMap::new();
    let mut targets: Vec<Pos> = Vec::new();
    for (c, on, off) in &plan {
        let x = c + 1;
        let k = used.entry(x).or_default();
        let sl = slots(x);
        let Some((p, f, w)) = sl.get(*k).copied() else {
            bail!("cell x{x} needs more than {} pulsers", sl.len());
        };
        *k += 1;
        b.insert(p, obs(f));
        blocked.insert(p);
        targets.push(w);
        log.push(format!("x{x} {} from {p:?} watching {w:?}", match (on, off) { (true, true) => "both", (true, false) => "on", _ => "off" }));
    }
    for w in &targets {
        blocked.insert(*w);
    }
    // no spine observer may emit into a cell right above the line: that powers the piston below
    // by quasi-connectivity (the top pulsers there are intended)
    for x in 0..=target_x + 1 {
        blocked.insert([x, 2, 0]);
    }
    // the lever and the first spine node: start near the first target
    let lo = [-4, -4, -5];
    let hi = [target_x + 3, 6, 5];
    let inside = |p: Pos| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]);
    let first = targets[0];
    if std::env::var("PEXT_DEBUG").is_ok() {
        eprintln!("targets: {targets:?}");
        eprintln!("blocked near first: {:?}", blocked.iter().filter(|p| (p[0] - first[0]).abs() + (p[1] - first[1]).abs() + (p[2] - first[2]).abs() <= 1).collect::<Vec<_>>());
    }
    // lever two cells away from the first target, on a stone below it
    let mut start = None;
    for (d, _) in DIRS {
        let l = add(first, [d[0] * 2, d[1] * 2, d[2] * 2]);
        let mid = add(first, d);
        let under = add(l, [0, -1, 0]);
        if inside(l) && !blocked.contains(&l) && !blocked.contains(&mid) && !blocked.contains(&under) && !b.contains_key(&l) && !b.contains_key(&under) && !b.contains_key(&mid) && d[1] == 0 {
            start = Some((l, mid, under));
            break;
        }
    }
    let (lever, mid, under) = start.ok_or_else(|| anyhow!("no room for the lever"))?;
    b.insert(under, "minecraft:stone".into());
    b.insert(lever, "minecraft:lever[face=floor,facing=north]".into());
    blocked.insert(lever);
    blocked.insert(under);
    // spine: lever -> mid -> first target -> ... each node watches the previous one. An observer
    // emits straight through (its back is opposite its face, whatever way the chain turns there):
    // never into the line, a cell right above it (quasi-connectivity) or a non-observer block
    let mut forbid: BTreeSet<Pos> = (0..=target_x + 1).flat_map(|x| [[x, 1, 0], [x, 2, 0]]).collect();
    for (p, s) in &b {
        if !s.contains("observer") {
            forbid.insert(*p);
        }
    }
    let mut chain: Vec<Pos> = vec![lever, mid, first];
    if forbid.contains(&add(first, [first[0] - mid[0], first[1] - mid[1], first[2] - mid[2]])) {
        bail!("the first spine node would emit into a forbidden cell");
    }
    blocked.insert(mid);
    for (k, w) in targets.iter().enumerate().skip(1) {
        let from = *chain.last().unwrap();
        // a top pulser's watched node must not face up (its back would power the cell above the
        // piston): never arrive from above
        let mut extra = blocked.clone();
        if w[1] == 3 && w[2] == 0 {
            extra.insert([w[0], 4, 0]);
        }
        // keep the neighbours of the targets still to come free (each needs a way in and out)
        for (j, t) in targets.iter().enumerate() {
            if j > k {
                for (d, _) in DIRS {
                    extra.insert(add(*t, d));
                }
            }
        }
        let path = route(from, *w, &extra, &b, inside, o.hops, &forbid)?;
        for p in &path {
            blocked.insert(*p);
        }
        chain.extend(path);
    }
    for i in 1..chain.len() {
        let f = facing_towards(chain[i], chain[i - 1])?;
        if b.insert(chain[i], obs(f)).is_some() {
            bail!("spine collision at {:?}", chain[i]);
        }
    }
    log.push(format!("spine: {} observers", chain.len() - 1));
    let mut d = Design { name: format!("pext-n{n}-stack-gen"), ..Default::default() };
    d.meta.insert("circuit".into(), format!("piston-extender {n}"));
    d.meta.insert("travel".into(), "1 0 0".into());
    d.meta.insert("port input".into(), "lever".into());
    d.ports.insert("input".into(), lever);
    d.ports.insert("payload".into(), payload);
    d.blocks = b;
    Ok((d, log))
}

/// Shortest free path from `from` (exclusive) to `to` (inclusive) of at least `min` cells; a path
/// too short gets a perpendicular bump (a -> a+d -> b+d -> b) until it is long enough.
fn route(from: Pos, to: Pos, blocked: &BTreeSet<Pos>, b: &BTreeMap<Pos, String>, inside: impl Fn(Pos) -> bool, min: usize, forbid: &BTreeSet<Pos>) -> Result<Vec<Pos>> {
    let free = |p: Pos| p == to || (inside(p) && !blocked.contains(&p) && !b.contains_key(&p));
    // a node entered along d emits into node + d
    let ok_back = |n: Pos, d: Pos| !forbid.contains(&add(n, d));
    // breadth-first over (cell, the direction it was entered along)
    let mut prev: HashMap<(Pos, usize), (Pos, usize)> = HashMap::new();
    let mut q = VecDeque::new();
    let start = (from, usize::MAX);
    q.push_back(start);
    prev.insert(start, start);
    let mut goal = None;
    while let Some((c, cd)) = q.pop_front() {
        if c == to {
            goal = Some((c, cd));
            break;
        }
        for (di, (d, _)) in DIRS.iter().enumerate() {
            let nx = add(c, *d);
            if free(nx) && ok_back(nx, *d) && !prev.contains_key(&(nx, di)) {
                prev.insert((nx, di), (c, cd));
                q.push_back((nx, di));
            }
        }
    }
    let Some(goal) = goal else {
        let around = |c: Pos| -> String {
            DIRS.iter()
                .map(|(d, f)| {
                    let p = add(c, *d);
                    format!("{f}:{}", if free(p) { "free" } else if b.contains_key(&p) { "block" } else if !inside(p) { "out" } else { "reserved" })
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        bail!("no route from {from:?} ({}) to {to:?} ({}); {} cells reached", around(from), around(to), prev.len());
    };
    let mut path = vec![to];
    let mut s = goal;
    while prev[&s].0 != from || prev[&s].1 != usize::MAX {
        s = prev[&s];
        path.push(s.0);
    }
    path.reverse();
    if path.iter().collect::<BTreeSet<_>>().len() != path.len() {
        bail!("route {from:?} -> {to:?} crosses itself");
    }
    // every node's back (straight through) must stay out of the forbidden cells
    let backs_ok = |full: &[Pos]| -> bool {
        (1..full.len()).all(|i| {
            let d = [full[i][0] - full[i - 1][0], full[i][1] - full[i - 1][1], full[i][2] - full[i - 1][2]];
            ok_back(full[i], d)
        })
    };
    // lengthen with bumps
    let mut guard = 0;
    while path.len() < min {
        guard += 1;
        if guard > 50 {
            bail!("cannot lengthen the route {from:?} -> {to:?}");
        }
        let mut full = vec![from];
        full.extend(path.iter().copied());
        let taken: BTreeSet<Pos> = full.iter().copied().collect();
        let mut done = false;
        'outer: for i in 0..full.len() - 1 {
            let (a, c2) = (full[i], full[i + 1]);
            for (d, _) in DIRS {
                let (p1, p2) = (add(a, d), add(c2, d));
                if p1 == c2 || p2 == a {
                    continue;
                }
                if free(p1) && free(p2) && p1 != to && p2 != to && !taken.contains(&p1) && !taken.contains(&p2) {
                    let mut np: Vec<Pos> = full[1..=i].to_vec();
                    np.push(p1);
                    np.push(p2);
                    np.extend(full[i + 1..].iter().copied());
                    let mut check = vec![from];
                    check.extend(np.iter().copied());
                    if !backs_ok(&check) {
                        continue;
                    }
                    path = np;
                    done = true;
                    break 'outer;
                }
            }
        }
        if !done {
            bail!("no room to lengthen the route {from:?} -> {to:?}");
        }
    }
    Ok(path)
}
