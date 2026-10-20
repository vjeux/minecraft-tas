//! `mctas circuit tnt align`: local search on redsim for a PRECISE duper. Starting from a design
//! that dupes, it adds, removes or replaces blocks of an aligner palette in a box, never touching
//! the design's own cells (the duper), and keeps a change when the largest group of identical
//! fuse-1 positions over the kicks (`precise::groups`) does not shrink (fewer blocks break ties).
//! A design with every kick in one group is written to DIR (then: `precise-sim` with all kicks,
//! `circuit tnt precision` in the game).

use crate::precise;
use crate::search::expand;
use anyhow::Result;
use clock::{Clock, Pos};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct AlignOptions {
    pub design: PathBuf,
    /// Only these cells are never touched (default: every cell of `design`).
    pub fixed: Option<PathBuf>,
    pub lo: Pos,
    pub hi: Pos,
    pub palette: Vec<String>,
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    pub ticks: u32,
    pub nth: usize,
    pub kicks: usize,
    pub out: PathBuf,
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// (largest group, blocks) of a block set, or None when it no longer runs.
fn score(base: &Clock, blocks: &BTreeMap<Pos, String>, kicks: &[f64], ticks: u32, nth: usize) -> Option<usize> {
    let mut d = base.design.clone();
    d.blocks = blocks.clone();
    let c = Clock::from_design(d).ok()?;
    let (g, bad) = precise::groups(&c, kicks, ticks, nth);
    if !bad.is_empty() {
        return None;
    }
    g.first().map(|x| x.1)
}

pub fn align(o: &AlignOptions) -> Result<Option<PathBuf>> {
    let base = Clock::load(&o.design)?;
    let start: BTreeMap<Pos, String> = base.design.blocks.clone();
    let fixed: BTreeMap<Pos, String> = match &o.fixed {
        Some(f) => Clock::load(f)?.design.blocks,
        None => start.clone(),
    };
    let mut pal: Vec<String> = Vec::new();
    for n in &o.palette {
        pal.extend(expand(n)?);
    }
    let all = precise::default_kicks();
    let kicks: Vec<f64> = if o.kicks >= all.len() { all.clone() } else { (0..o.kicks).map(|k| all[k * all.len() / o.kicks]).collect() };
    let s0 = score(&base, &start, &kicks, o.ticks, o.nth).unwrap_or(0);
    eprintln!("  start: {s0} of {} kicks in the largest group", kicks.len());
    let cells: Vec<Pos> = {
        let mut v = Vec::new();
        for y in o.lo[1]..=o.hi[1] {
            for z in o.lo[2]..=o.hi[2] {
                for x in o.lo[0]..=o.hi[0] {
                    if !fixed.contains_key(&[x, y, z]) {
                        v.push([x, y, z]);
                    }
                }
            }
        }
        v
    };
    let best: Mutex<(usize, BTreeMap<Pos, String>)> = Mutex::new((s0, start.clone()));
    let iter = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..o.workers.max(1) {
            let (best, iter, cells, pal, kicks, base, fixed) = (&best, &iter, &cells, &pal, &kicks, &base, &fixed);
            sc.spawn(move || {
                let mut r = Rng(o.rng.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 * 7919 + 3) | 1);
                let (mut cur_s, mut cur) = { let b = best.lock().unwrap(); (b.0, b.1.clone()) };
                loop {
                    let i = iter.fetch_add(1, Ordering::Relaxed);
                    if i >= o.iterations || cur_s == kicks.len() && cur.len() <= fixed.len() {
                        break;
                    }
                    if i % 200 == 0 && i > 0 {
                        let b = best.lock().unwrap();
                        eprintln!("  {i}/{} iterations, best {} of {} ({} blocks), {:.0} s", o.iterations, b.0, kicks.len(), b.1.len(), t0.elapsed().as_secs_f64());
                    }
                    let mut b = cur.clone();
                    let extra: Vec<Pos> = b.keys().filter(|p| !fixed.contains_key(*p)).copied().collect();
                    match r.below(4) {
                        0 if !extra.is_empty() => {
                            b.remove(&extra[r.below(extra.len())]);
                        }
                        1 if !extra.is_empty() => {
                            b.insert(extra[r.below(extra.len())], pal[r.below(pal.len())].clone());
                        }
                        _ => {
                            let p = cells[r.below(cells.len())];
                            b.insert(p, pal[r.below(pal.len())].clone());
                        }
                    }
                    let Some(s) = score(base, &b, kicks, o.ticks, o.nth) else { continue };
                    if s > cur_s || (s == cur_s && (b.len() <= cur.len() || r.below(4) == 0)) {
                        cur_s = s;
                        cur = b;
                        let mut g = best.lock().unwrap();
                        if cur_s > g.0 || (cur_s == g.0 && cur.len() < g.1.len()) {
                            *g = (cur_s, cur.clone());
                        }
                    } else if r.below(300) == 0 {
                        let g = best.lock().unwrap();
                        cur_s = g.0;
                        cur = g.1.clone();
                    }
                }
            });
        }
    });
    let (s, blocks) = best.into_inner().unwrap();
    eprintln!("  best: {s} of {} kicks, {} blocks", kicks.len(), blocks.len());
    std::fs::create_dir_all(&o.out)?;
    let mut d = base.design.clone();
    d.name = format!("{}-align-{s}of{}-{}b", base.design.name, kicks.len(), blocks.len());
    d.blocks = blocks;
    let path = o.out.join(format!("{}.txt", d.name));
    std::fs::write(&path, d.to_text())?;
    Ok(Some(path))
}
