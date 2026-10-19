//! The filler: a skeleton design (with its container contents) plus free cells; every filling of
//! k of the free cells from a palette is judged in redsim ([`crate::sim`]); irreducible item
//! clocks are kept (deduplicated by type with contents). For wiring a mechanism idea: the core
//! (hoppers, items, the comparator) is fixed, the feedback is searched.

use crate::census::{self, Found, Opts};
use crate::sim::Spec;
use redsim::{Block, BlockPos};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct FillOpts {
    pub base: Spec,
    /// Candidate cells, in (y, z, x) order (floors come before what stands on them).
    pub free: Vec<BlockPos>,
    pub k_min: usize,
    pub k_max: usize,
    pub palette: Vec<Block>,
    pub max_ticks: u32,
    pub jobs: usize,
}

fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut idx = Vec::new();
    fn rec(n: usize, k: usize, start: usize, idx: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if idx.len() == k {
            out.push(idx.clone());
            return;
        }
        for i in start..n {
            idx.push(i);
            rec(n, k, i + 1, idx, out);
            idx.pop();
        }
    }
    rec(n, k, 0, &mut idx, &mut out);
    out
}

/// Run the filler; every irreducible clock found (a type with its contents once).
pub fn run(o: &FillOpts, progress: bool) -> Vec<Found> {
    let base_at: HashMap<BlockPos, Block> = o.base.blocks.iter().copied().collect();
    let copts = Opts { size: [0; 3], k_min: 0, k_max: 0, palette: Vec::new(), contents: Vec::new(), max_ticks: o.max_ticks, jobs: o.jobs, any_family: false };
    let mut all = Vec::new();
    let seen: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    for k in o.k_min..=o.k_max {
        let subs = subsets(o.free.len(), k);
        let next = AtomicUsize::new(0);
        let tried = AtomicUsize::new(0);
        let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
        let start = std::time::Instant::now();
        std::thread::scope(|sc| {
            for _ in 0..o.jobs.max(1) {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= subs.len() {
                            break;
                        }
                        let cells: Vec<BlockPos> = subs[i].iter().map(|j| o.free[*j]).collect();
                        let mut assign = Vec::with_capacity(k);
                        dfs(o, &base_at, &copts, &cells, &mut assign, &tried, &found, &seen);
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        if progress {
            eprintln!("fill k={k}: {} cell sets, {} candidates judged, {} clocks in {:.1} s", subs.len(), tried.load(Ordering::Relaxed), found.len(), start.elapsed().as_secs_f64());
        }
        all.extend(found);
    }
    all
}

#[allow(clippy::too_many_arguments)]
fn dfs(
    o: &FillOpts,
    base_at: &HashMap<BlockPos, Block>,
    copts: &Opts,
    cells: &[BlockPos],
    assign: &mut Vec<Block>,
    tried: &AtomicUsize,
    found: &Mutex<Vec<Found>>,
    seen: &Mutex<HashSet<String>>,
) {
    let i = assign.len();
    if i == cells.len() {
        let mut blocks = o.base.blocks.clone();
        blocks.extend(cells.iter().copied().zip(assign.iter().copied()));
        if !census::wall_torches_ok(&blocks) {
            return;
        }
        let spec = Spec { blocks, nbt: o.base.nbt.clone() };
        tried.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = census::judge_candidate(&spec, copts) {
            let key = census::type_key(&f.spec, true);
            if seen.lock().unwrap().contains(&key) {
                return;
            }
            if census::irreducible(&f, copts) {
                seen.lock().unwrap().insert(key);
                found.lock().unwrap().push(f);
            }
        }
        return;
    }
    let p = cells[i];
    for b in &o.palette {
        if let Some(need) = census::needs_floor(b) {
            let below = p.below();
            let floor = base_at.get(&below).copied().or_else(|| cells[..i].iter().position(|q| *q == below).map(|j| assign[j]));
            match floor {
                Some(f) if census::floor_ok(&f, need, b) => {}
                _ => continue,
            }
        }
        assign.push(*b);
        dfs(o, base_at, copts, cells, assign, tried, found, seen);
        assign.pop();
    }
}
