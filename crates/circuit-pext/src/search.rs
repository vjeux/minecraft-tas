//! Searches on redsim: `grow` (every way to add k blocks from a palette in a region of a base
//! design) and `minimize` (removals, then substitutions, while the extender keeps working).
//! Candidates are screened fast (2 cycles, no per-tick snapshots), then judged in full; `grow`
//! keeps only MINIMAL finds (no added block can be removed with the extender still working).

use crate::rs::{self, Params, Prepared};
use crate::spec::{Pext, Pos, add, scale};
use anyhow::{Result, bail};
use redsim::{Block, BlockPos};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Palette shorthands: the clock census's (`stone`, `glass`, `rblock`, `dust`, `torch`,
/// `repeater`, `repeater1`, `comparator`, `observer`, `piston`, `sticky`, `slime`, `honey`,
/// `lamp`, `note`, `target`, `obsidian`, `wool`, ... or block states) plus `cross` (a cross dust,
/// which stays a cross and points into all four neighbors).
pub fn palette(spec: &str) -> Result<Vec<String>> {
    let mut extra = Vec::new();
    let rest: Vec<&str> = spec
        .split(',')
        .filter(|s| {
            if s.trim() == "cross" {
                extra.push("minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".to_string());
                false
            } else {
                true
            }
        })
        .collect();
    let mut v = if rest.iter().all(|s| s.trim().is_empty()) { Vec::new() } else { clock::census::expand_palette(&rest.join(","))? };
    v.extend(extra);
    for s in &v {
        if let Err(e) = Block::parse(s) {
            bail!("palette state {s}: {e}");
        }
    }
    Ok(v)
}

#[derive(Clone, Debug)]
pub struct GrowOptions {
    pub lo: Pos,
    pub hi: Pos,
    pub palette: Vec<String>,
    pub k: usize,
    pub kmin: usize,
    pub threads: usize,
    pub out_dir: Option<PathBuf>,
    /// Only cells with this z (flat searches), when set.
    pub plane_z: Option<i32>,
    /// Keep every find, not only the minimal ones.
    pub all: bool,
}

#[derive(Clone, Debug)]
pub struct Find {
    pub added: Vec<(Pos, String)>,
    pub report: rs::Report,
    pub blocks: usize,
}

/// The cells of the payload's path (rest excluded, target included).
pub fn path_cells(p: &Pext) -> Vec<Pos> {
    (1..=p.reach).map(|k| add(p.payload, scale(p.travel, k))).collect()
}

/// A redsim world box covering the base, the target and a region, plus a margin.
pub fn prepared_for(p: &Pext, lo: Pos, hi: Pos, margin: i32) -> Result<Prepared> {
    let mut prep = Prepared::new(p, 0).map_err(|e| anyhow::anyhow!("not modeled by redsim: {e}"))?;
    let (blo, bhi) = p.bounds();
    let l = [blo[0].min(lo[0]) - margin, blo[1].min(lo[1]) - margin, blo[2].min(lo[2]) - margin];
    let h = [bhi[0].max(hi[0]) + margin, bhi[1].max(hi[1]) + margin, bhi[2].max(hi[2]) + margin];
    prep.min = BlockPos::new(l[0], l[1], l[2]);
    prep.size = [h[0] - l[0] + 1, h[1] - l[1] + 1, h[2] - l[2] + 1];
    Ok(prep)
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

fn with_added(base: &Prepared, added: &[(Pos, Block)]) -> Prepared {
    let mut p = base.clone();
    p.blocks.extend(added.iter().map(|(q, b)| (bp(*q), *b)));
    p
}

fn screen_ok(prep: &Prepared) -> bool {
    rs::judge(prep, &Params::screen(), false).0.works()
}

fn combos(n: usize, k: usize) -> Vec<Vec<usize>> {
    fn rec(n: usize, k: usize, start: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            rec(n, k, i + 1, cur, out);
            cur.pop();
        }
    }
    let mut out = Vec::new();
    rec(n, k, 0, &mut Vec::new(), &mut out);
    out
}

/// Every way to add `kmin..=k` blocks of the palette in the region's free cells.
pub fn grow(p: &Pext, o: &GrowOptions, progress: &(dyn Fn(usize, usize, usize) + Sync)) -> Result<Vec<Find>> {
    let base = prepared_for(p, o.lo, o.hi, 2)?;
    let forbidden: Vec<Pos> = path_cells(p);
    let mut cells: Vec<Pos> = Vec::new();
    for y in o.lo[1]..=o.hi[1] {
        for z in o.lo[2]..=o.hi[2] {
            for x in o.lo[0]..=o.hi[0] {
                let c = [x, y, z];
                if p.design.blocks.contains_key(&c) || forbidden.contains(&c) || o.plane_z.is_some_and(|pz| pz != z) {
                    continue;
                }
                cells.push(c);
            }
        }
    }
    let states: Vec<Block> = o.palette.iter().map(|s| Block::parse(s).map_err(|e| anyhow::anyhow!("{e}"))).collect::<Result<_>>()?;
    let m = states.len();
    let mut finds: Vec<Find> = Vec::new();
    for k in o.kmin..=o.k {
        let cs = combos(cells.len(), k);
        // work units: (cell combo, state of its first cell)
        let units = cs.len() * m.max(1);
        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        let found: Mutex<Vec<Vec<(Pos, Block)>>> = Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| {
                    loop {
                        let u = next.fetch_add(1, Ordering::Relaxed);
                        if u >= units {
                            break;
                        }
                        let combo = &cs[u / m.max(1)];
                        let first = u % m.max(1);
                        // the other cells' states: every tuple
                        let rest = combo.len().saturating_sub(1);
                        let total = m.pow(rest as u32);
                        for t in 0..total {
                            let mut added: Vec<(Pos, Block)> = Vec::with_capacity(combo.len());
                            let mut tt = t;
                            for (j, ci) in combo.iter().enumerate() {
                                let si = if j == 0 {
                                    first
                                } else {
                                    let v = tt % m;
                                    tt /= m;
                                    v
                                };
                                added.push((cells[*ci], states[si]));
                            }
                            if k == 0 && first > 0 {
                                break;
                            }
                            let cand = with_added(&base, &added);
                            if screen_ok(&cand) {
                                found.lock().unwrap().push(added);
                            }
                        }
                        let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                        if d % 2000 == 0 || d == units {
                            progress(k, d, units);
                        }
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        // full judge + minimality (no added block removable)
        for added in found {
            let cand = with_added(&base, &added);
            let (r, _) = rs::judge(&cand, &Params::default(), false);
            if !r.works() {
                continue;
            }
            if !o.all && added.len() > 1 {
                let removable = (0..added.len()).any(|i| {
                    let mut fewer = added.clone();
                    fewer.remove(i);
                    screen_ok(&with_added(&base, &fewer))
                });
                if removable {
                    continue;
                }
            }
            let blocks = p.blocks() + added.len();
            finds.push(Find { added: added.iter().map(|(q, b)| (*q, b.to_string())).collect(), report: r, blocks });
        }
    }
    finds.sort_by_key(|f| (f.blocks, f.report.first().extend + f.report.first().retract));
    Ok(finds)
}

/// The base design with the find's blocks added, renamed.
pub fn find_design(p: &Pext, f: &Find, name: &str) -> mctas::design::Design {
    let mut d = p.design.clone();
    for (q, s) in &f.added {
        d.blocks.insert(*q, s.clone());
    }
    d.name = name.to_string();
    d
}

#[derive(Clone, Debug)]
pub struct MinOptions {
    pub palette: Vec<String>,
    pub threads: usize,
}

/// Greedy shrink: remove any block (not the input or payload) while it still works; then try
/// replacing each block by a palette state and removing another. Returns the designs found in
/// order (each smaller or equal and different), the last being the smallest.
pub fn minimize(p: &Pext, o: &MinOptions) -> Result<Vec<mctas::design::Design>> {
    let mut cur = p.clone();
    let mut out = Vec::new();
    loop {
        let prep = Prepared::new(&cur, 2).map_err(|e| anyhow::anyhow!("not modeled: {e}"))?;
        if !rs::judge(&prep, &Params::default(), false).0.works() {
            bail!("{} does not work on redsim", cur.name());
        }
        let positions: Vec<Pos> = cur.design.blocks.keys().copied().filter(|q| *q != cur.input && *q != cur.payload).collect();
        // 1. a single removal
        let removal = positions.iter().find_map(|q| {
            let mut d = cur.design.clone();
            d.blocks.remove(q);
            let np = Pext::from_design(d).ok()?;
            let pr = Prepared::new(&np, 2).ok()?;
            rs::judge(&pr, &Params::default(), false).0.works().then_some(np)
        });
        if let Some(np) = removal {
            cur = np;
            out.push(cur.design.clone());
            continue;
        }
        // 2. a substitution plus a removal
        let states = &o.palette;
        let mut tries: Vec<(Pos, String, Pos)> = Vec::new();
        for a in &positions {
            for s in states {
                for b in &positions {
                    if b != a {
                        tries.push((*a, s.clone(), *b));
                    }
                }
            }
        }
        let next = AtomicUsize::new(0);
        let hit: Mutex<Option<Pext>> = Mutex::new(None);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| {
                    loop {
                        if hit.lock().unwrap().is_some() {
                            break;
                        }
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= tries.len() {
                            break;
                        }
                        let (a, st, b) = &tries[i];
                        let mut d = cur.design.clone();
                        d.blocks.insert(*a, st.clone());
                        d.blocks.remove(b);
                        let Ok(np) = Pext::from_design(d) else { continue };
                        let Ok(pr) = Prepared::new(&np, 2) else { continue };
                        if screen_ok(&pr) && rs::judge(&pr, &Params::default(), false).0.works() {
                            let mut h = hit.lock().unwrap();
                            if h.is_none() {
                                *h = Some(np);
                            }
                        }
                    }
                });
            }
        });
        match hit.into_inner().unwrap() {
            Some(np) => {
                cur = np;
                out.push(cur.design.clone());
            }
            None => break,
        }
    }
    Ok(out)
}
