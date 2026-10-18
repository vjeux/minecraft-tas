//! `mctas door obs-path`: shortest (or exact-length) observer chains between two cells of a
//! design, for wiring pulse trains (each observer adds 2 gt and needs no floor).
//!
//! A chain o1..ok: o1 watches `--from` (a cell whose block changes once per lever edge: the lever,
//! an observer, a moving door block...), o(i+1) watches o(i), and ok's output (its back) points
//! into `--to` (a piston, its quasi cell, a block next to it, a stack-feed dust...). If `--from`
//! changes at t, ok fires at t + 2k.
//!
//! Safety rules (static, the simulator has the last word): chain cells are free, outside the
//! walkway and the `--forbid` cells, and watched by no observer of the design; every output of an
//! intermediate observer goes into the next observer or into an empty cell that is not a piston's
//! quasi cell (the cell above a piston) and holds no component; dust next to a chain observer
//! whose back faces it is avoided (it would change shape).

use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::Pos;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;

const DIRS: [(&str, Pos); 6] = [
    ("north", [0, 0, -1]),
    ("south", [0, 0, 1]),
    ("east", [1, 0, 0]),
    ("west", [-1, 0, 0]),
    ("up", [0, 1, 0]),
    ("down", [0, -1, 0]),
];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn prop<'a>(state: &'a str, key: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.trim_end_matches(']');
    inner.split(',').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k.trim() == key).then_some(v.trim())
    })
}

fn dir_of(name: &str) -> Option<Pos> {
    DIRS.iter().find(|(n, _)| *n == name).map(|(_, d)| *d)
}

pub fn parse_pos(s: &str) -> Result<Pos> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
    if v.len() != 3 {
        bail!("expected x,y,z, got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

pub struct Opts {
    pub from: Pos,
    pub to: Pos,
    pub len: Option<usize>,
    pub max_len: usize,
    pub forbid: Vec<Pos>,
    pub lo: Option<Pos>,
    pub hi: Option<Pos>,
    pub limit: usize,
}

/// Every chain found, as (cell, facing name) lists.
pub fn search(d: &Design, o: &Opts) -> Result<Vec<Vec<(Pos, &'static str)>>> {
    let door = *d.ports.get("door").context("design has no door port")?;
    let in_walkway = |p: Pos| {
        p[0] >= door[0] && p[0] <= door[0] + 2 && p[1] >= door[1] && p[1] <= door[1] + 2 && (p[2] - door[2]).abs() <= 3
    };
    let forbid: BTreeSet<Pos> = o.forbid.iter().copied().collect();
    let mut watched: BTreeSet<Pos> = BTreeSet::new();
    let mut pistons: BTreeSet<Pos> = BTreeSet::new();
    for (p, s) in &d.blocks {
        let name = s.split('[').next().unwrap_or(s);
        if name == "minecraft:observer" {
            if let Some(f) = prop(s, "facing").and_then(dir_of) {
                watched.insert(add(*p, f));
            }
        }
        if name == "minecraft:piston" || name == "minecraft:sticky_piston" {
            pistons.insert(*p);
        }
    }
    let is_dust = |p: Pos| d.blocks.get(&p).is_some_and(|s| s.starts_with("minecraft:redstone_wire"));
    let in_box = |p: Pos| match (o.lo, o.hi) {
        (Some(lo), Some(hi)) => (0..3).all(|i| p[i] >= lo[i].min(hi[i]) && p[i] <= lo[i].max(hi[i])),
        _ => true,
    };
    let free = |p: Pos| !d.blocks.contains_key(&p) && !in_walkway(p) && !forbid.contains(&p) && !watched.contains(&p) && in_box(p);
    // An observer at c facing f (watches c+f, outputs into c-f).
    let out_ok = |c: Pos, f: Pos, next: Option<Pos>| -> bool {
        let e = sub(c, f);
        if e == o.to {
            return true;
        }
        if Some(e) == next {
            return true;
        }
        if let Some(s) = d.blocks.get(&e) {
            return s.starts_with("minecraft:observer");
        }
        if is_dust(e) {
            return false;
        }
        !pistons.contains(&add(e, [0, -1, 0]))
    };
    // Dust beside c that c's back faces would connect to it: avoid unless it is the target.
    let dust_ok = |c: Pos, f: Pos| -> bool {
        let e = sub(c, f);
        e == o.to || !is_dust(e)
    };
    let max_len = o.len.unwrap_or(o.max_len);
    let mut results = Vec::new();
    // BFS over paths (cells may not repeat inside one path); layer by layer.
    let mut frontier: VecDeque<Vec<(Pos, Pos)>> = VecDeque::new();
    for (_, f) in DIRS {
        let c = sub(o.from, f);
        if free(c) && dust_ok(c, f) {
            frontier.push_back(vec![(c, f)]);
        }
    }
    let mut best: Option<usize> = None;
    let mut seen: HashMap<(Pos, Pos, usize), ()> = HashMap::new();
    while let Some(path) = frontier.pop_front() {
        let k = path.len();
        if let Some(b) = best {
            if o.len.is_none() && k > b {
                break;
            }
        }
        let (c, f) = *path.last().unwrap();
        // Done?
        if sub(c, f) == o.to && o.len.is_none_or(|l| l == k) {
            // every intermediate output must be safe given its successor
            let ok = path.windows(2).all(|w| out_ok(w[0].0, w[0].1, Some(w[1].0)));
            if ok {
                best.get_or_insert(k);
                let names: Vec<(Pos, &'static str)> =
                    path.iter().map(|(p, f)| (*p, DIRS.iter().find(|(_, d)| d == f).unwrap().0)).collect();
                results.push(names);
                if results.len() >= o.limit {
                    break;
                }
            }
            continue;
        }
        if k >= max_len {
            continue;
        }
        for (_, f2) in DIRS {
            let c2 = sub(c, f2);
            if !free(c2) || !dust_ok(c2, f2) || path.iter().any(|(p, _)| *p == c2) || c2 == o.from || c2 == o.to {
                continue;
            }
            // c's output must be safe with c2 as successor
            if !out_ok(c, f, Some(c2)) {
                continue;
            }
            // the new observer must not sit in the output of an earlier one (other than c)
            if path[..k - 1].iter().any(|(p, pf)| sub(*p, *pf) == c2) {
                continue;
            }
            if seen.insert((c2, f2, k + 1), ()).is_some() && o.len.is_none() {
                continue;
            }
            let mut p2 = path.clone();
            p2.push((c2, f2));
            frontier.push_back(p2);
        }
    }
    Ok(results)
}

pub fn cli(design: &Path, o: &Opts, out: Option<&Path>) -> Result<()> {
    let d = Design::load(design)?;
    let res = search(&d, o)?;
    if res.is_empty() {
        println!("no chain");
        return Ok(());
    }
    for (i, chain) in res.iter().enumerate() {
        let s: Vec<String> = chain.iter().map(|(p, f)| format!("({},{},{}){}", p[0], p[1], p[2], &f[..1])).collect();
        println!("#{i} len {}: {}", chain.len(), s.join(" -> "));
    }
    if let Some(out) = out {
        for (i, chain) in res.iter().enumerate() {
            let mut d2 = d.clone();
            for (p, f) in chain {
                d2.blocks.insert(*p, format!("minecraft:observer[facing={f},powered=false]"));
            }
            let path = if res.len() == 1 { out.to_path_buf() } else { out.with_file_name(format!("{}-{i}.txt", out.file_stem().unwrap().to_string_lossy())) };
            d2.save(&path)?;
        }
    }
    Ok(())
}
