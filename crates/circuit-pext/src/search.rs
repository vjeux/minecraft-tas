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
            let s = s.trim();
            let faces = |id: &str| -> Vec<String> {
                let mut v = vec![format!("minecraft:{id}[face=floor,facing=north]"), format!("minecraft:{id}[face=ceiling,facing=north]")];
                v.extend(["north", "east", "south", "west"].iter().map(|f| format!("minecraft:{id}[face=wall,facing={f}]")));
                v
            };
            match s {
                "cross" => extra.push("minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".to_string()),
                "lever" => extra.extend(faces("lever")),
                "button" => extra.extend(faces("stone_button")),
                _ => return true,
            }
            false
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
    /// Only cells face-adjacent (Manhattan distance <= near) to a base block other than the
    /// input, when set.
    pub near: Option<i32>,
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

fn input_kind_of(b: &Block) -> Option<crate::spec::InputKind> {
    match b {
        Block::Lever { .. } => Some(crate::spec::InputKind::Lever),
        Block::Button { .. } => Some(crate::spec::InputKind::Button),
        _ => None,
    }
}

/// The base with blocks added; a base without an input takes the one added lever or button as
/// its input (None: not exactly one).
fn with_added(base: &Prepared, added: &[(Pos, Block)]) -> Option<Prepared> {
    let mut p = base.clone();
    if base.input.x == i32::MIN {
        let mut inputs = added.iter().filter_map(|(q, b)| input_kind_of(b).map(|k| (*q, k)));
        let (q, k) = inputs.next()?;
        if inputs.next().is_some() {
            return None;
        }
        p.input = bp(q);
        p.kind = k;
    }
    p.blocks.extend(added.iter().map(|(q, b)| (bp(*q), *b)));
    Some(p)
}

fn screen_ok(prep: &Option<Prepared>) -> bool {
    prep.as_ref().is_some_and(|p| rs::judge(p, &Params::screen(), false).0.works())
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
                if let Some(nd) = o.near {
                    let close = p.design.blocks.keys().any(|q| *q != p.input && (q[0] - c[0]).abs() + (q[1] - c[1]).abs() + (q[2] - c[2]).abs() <= nd);
                    if !close {
                        continue;
                    }
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
            let Some(cand) = with_added(&base, &added) else { continue };
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
            let blocks = p.blocks() + added.len() - usize::from(!p.has_input());
            finds.push(Find { added: added.iter().map(|(q, b)| (*q, b.to_string())).collect(), report: r, blocks });
        }
    }
    finds.sort_by_key(|f| (f.blocks, f.report.first().extend + f.report.first().retract));
    Ok(finds)
}

/// The base design with the find's blocks added, renamed (a base without an input gets the
/// added lever or button as its `port input`).
pub fn find_design(p: &Pext, f: &Find, name: &str) -> mctas::design::Design {
    let mut d = p.design.clone();
    for (q, s) in &f.added {
        d.blocks.insert(*q, s.clone());
        if !p.has_input() && (s.contains("minecraft:lever") || s.contains("_button")) {
            d.ports.insert("input".into(), *q);
            d.meta.insert("port input".into(), if s.contains("lever") { "lever".into() } else { "button".into() });
        }
    }
    d.name = name.to_string();
    d
}

#[derive(Clone, Debug)]
pub struct MinOptions {
    pub palette: Vec<String>,
    pub threads: usize,
    /// Only place new blocks in this z plane (keeps a flat design flat).
    pub plane_z: Option<i32>,
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
        // 2. a substitution plus a removal; 3. a block placed in an empty cell next to the design
        // plus two removals (a move that frees a block)
        let states = &o.palette;
        let (lo, hi) = cur.design.bounds().unwrap_or(([0; 3], [0; 3]));
        let path = path_cells(&cur);
        let mut empties: Vec<Pos> = Vec::new();
        for y in lo[1] - 1..=hi[1] + 1 {
            for z in lo[2] - 1..=hi[2] + 1 {
                for x in lo[0] - 1..=hi[0] + 1 {
                    let c = [x, y, z];
                    if !cur.design.blocks.contains_key(&c) && !path.contains(&c) && o.plane_z.is_none_or(|pz| pz == z) {
                        empties.push(c);
                    }
                }
            }
        }
        let n = positions.len();
        let mut tries: Vec<(Pos, usize, usize, Option<usize>)> = Vec::new();
        for (ai, a) in positions.iter().enumerate() {
            for si in 0..states.len() {
                for bi in 0..n {
                    if bi != ai {
                        tries.push((*a, si, bi, None));
                    }
                }
            }
        }
        let subs = tries.len();
        for c in &empties {
            for si in 0..states.len() {
                for b1 in 0..n {
                    for b2 in b1 + 1..n {
                        tries.push((*c, si, b1, Some(b2)));
                    }
                }
            }
        }
        let next = AtomicUsize::new(0);
        let hit: Mutex<Option<(usize, Pext)>> = Mutex::new(None);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= tries.len() {
                            break;
                        }
                        // the substitutions first: stop at the first hit of the earlier phase
                        if let Some((hi, _)) = &*hit.lock().unwrap() {
                            if *hi < subs || i >= subs {
                                break;
                            }
                        }
                        let (a, si, b1, b2) = &tries[i];
                        let mut d = cur.design.clone();
                        d.blocks.insert(*a, states[*si].clone());
                        d.blocks.remove(&positions[*b1]);
                        if let Some(b2) = b2 {
                            d.blocks.remove(&positions[*b2]);
                        }
                        let Ok(np) = Pext::from_design(d) else { continue };
                        let Ok(pr) = Prepared::new(&np, 2) else { continue };
                        if screen_ok(&Some(pr.clone())) && rs::judge(&pr, &Params::default(), false).0.works() {
                            let mut h = hit.lock().unwrap();
                            if h.as_ref().is_none_or(|(hi, _)| i < *hi) {
                                *h = Some((i, np));
                            }
                        }
                    }
                });
            }
        });
        match hit.into_inner().unwrap() {
            Some((_, np)) => {
                cur = np;
                out.push(cur.design.clone());
            }
            None => break,
        }
    }
    Ok(out)
}
