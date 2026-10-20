//! `mctas circuit tnt improve`: local search from working dupers on redsim. Each worker starts
//! from one of the given designs and proposes mutations (delete a block, move one to a free cell
//! next to the design, replace one by a palette state, delete one and add another, add one); a
//! candidate that still dupes is accepted when it is no bigger and no slower (sometimes one block
//! bigger, to cross plateaus), and every distinct duper is recorded. The Pareto front (fewest
//! blocks for each rate, fastest for each block count) is written to DIR/front/ with
//! DIR/front.tsv; `circuit tnt run` / `circuit verify` confirm them in the game.

use crate::search::{dupes, expand};
use anyhow::{Result, bail};
use clock::{Clock, Pos};
use redsim::Block;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct ImproveOptions {
    pub designs: Vec<PathBuf>,
    pub palette: Vec<String>,
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    pub ticks: u32,
    pub out: PathBuf,
    /// Free cells around the design's box that blocks may move into.
    pub reach: i32,
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

const DIRS6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// A recorded duper: its blocks (positions normalized), period and primes per period.
#[derive(Clone)]
struct Found {
    blocks: BTreeMap<Pos, String>,
    p: u32,
    per: usize,
    template: usize,
}

impl Found {
    fn rate(&self) -> f64 {
        self.per as f64 * 1200.0 / self.p as f64
    }
}

fn normalized(b: &BTreeMap<Pos, String>) -> Vec<(Pos, String)> {
    let lo = b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
    b.iter().map(|(p, s)| ([p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]], s.clone())).collect()
}

/// A free cell next to the design (within `reach` of its box).
fn free_next_to(r: &mut Rng, b: &BTreeMap<Pos, String>, reach: i32, lo: Pos, hi: Pos) -> Option<Pos> {
    let cells: Vec<&Pos> = b.keys().collect();
    for _ in 0..30 {
        let c = cells[r.below(cells.len())];
        let d = DIRS6[r.below(6)];
        let p = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
        if !b.contains_key(&p) && (0..3).all(|i| p[i] >= lo[i] - reach && p[i] <= hi[i] + reach) {
            return Some(p);
        }
    }
    None
}

pub fn improve(o: &ImproveOptions) -> Result<usize> {
    let starts: Vec<Clock> = o.designs.iter().map(|d| Clock::load(d)).collect::<Result<_>>()?;
    if starts.is_empty() {
        bail!("no designs");
    }
    let mut pal: Vec<String> = Vec::new();
    for n in &o.palette {
        pal.extend(expand(n)?);
    }
    for s in &pal {
        Block::parse(s).map_err(|e| anyhow::anyhow!("{s}: {e}"))?;
    }
    let found: Mutex<HashMap<Vec<(Pos, String)>, Found>> = Mutex::new(HashMap::new());
    let iter = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..o.workers.max(1) {
            let (starts, pal, found, iter) = (&starts, &pal, &found, &iter);
            sc.spawn(move || {
                let mut r = Rng(o.rng.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 * 104729 + 7) | 1);
                let ti = wi % starts.len();
                let base = &starts[ti];
                // a triggered duper is scored by the judge's own redsim check (its value: TNT
                // per pulse); a self-running one by the fast screen (its value: TNT per minute)
                let triggered = base.design.meta.get("circuit").is_some_and(|k| k.trim() == "tnt-duper-triggered");
                let eval = |b: &BTreeMap<Pos, String>| -> Option<(u32, usize)> {
                    let mut d = base.design.clone();
                    d.blocks = b.clone();
                    if triggered {
                        let c = circuit::Circuit::from_design(d).ok()?;
                        let rep = circuit::verify::sim(&c, &circuit::verify::Options::default()).ok()?;
                        let out = rep.outcome?;
                        if !out.pass {
                            return None;
                        }
                        let per: usize = out.metrics.iter().find(|(n, _)| n == "per pulse")?.1.parse().ok()?;
                        // a pulse is 1 "period" of 1200 gt: rate() = per pulse
                        return Some((1200, per));
                    }
                    let c = Clock::from_design(d).ok()?;
                    dupes(&c, &[], o.ticks)
                };
                let Some((p0, per0)) = eval(&base.design.blocks) else { return };
                let (lo, hi) = base.design.bounds().unwrap();
                let mut cur = Found { blocks: base.design.blocks.clone(), p: p0, per: per0, template: ti };
                let mut stale = 0usize;
                loop {
                    let i = iter.fetch_add(1, Ordering::Relaxed);
                    if i >= o.iterations {
                        break;
                    }
                    if i > 0 && i % 100_000 == 0 {
                        let g = found.lock().unwrap();
                        let best = g.values().map(|f| f.blocks.len()).min().unwrap_or(0);
                        let fastest = g.values().map(|f| f.rate()).fold(0.0, f64::max);
                        eprintln!("  {i}/{} iterations, {} dupers, fewest {best} blocks, fastest {fastest:.1} TNT/min, {:.0} s", o.iterations, g.len(), t0.elapsed().as_secs_f64());
                    }
                    let mut b = cur.blocks.clone();
                    let keys: Vec<Pos> = b.keys().copied().filter(|k| base.withheld() != Some(*k)).collect();
                    let tnts = b.values().filter(|s| s.contains("tnt")).count();
                    let pick = keys[r.below(keys.len())];
                    let removable = !(b[&pick].contains("tnt") && tnts <= 1);
                    match r.below(5) {
                        0 if removable => {
                            b.remove(&pick);
                        }
                        1 => {
                            let s = b.remove(&pick).unwrap();
                            match free_next_to(&mut r, &b, o.reach, lo, hi) {
                                Some(q) => {
                                    b.insert(q, s);
                                }
                                None => continue,
                            }
                        }
                        2 if removable && !pal.is_empty() => {
                            b.insert(pick, pal[r.below(pal.len())].clone());
                        }
                        3 if removable && !pal.is_empty() => {
                            b.remove(&pick);
                            match free_next_to(&mut r, &b, o.reach, lo, hi) {
                                Some(q) => {
                                    b.insert(q, pal[r.below(pal.len())].clone());
                                }
                                None => continue,
                            }
                        }
                        4 if !pal.is_empty() => match free_next_to(&mut r, &b, o.reach, lo, hi) {
                            Some(q) => {
                                b.insert(q, pal[r.below(pal.len())].clone());
                            }
                            None => continue,
                        },
                        _ => continue,
                    }
                    let Some((p, per)) = eval(&b) else {
                        stale += 1;
                        if stale > 20_000 {
                            // restart from a random recorded duper of this template
                            let g = found.lock().unwrap();
                            let pool: Vec<&Found> = g.values().filter(|f| f.template == ti).collect();
                            if !pool.is_empty() {
                                cur = pool[r.below(pool.len())].clone();
                            }
                            stale = 0;
                        }
                        continue;
                    };
                    let cand = Found { blocks: b, p, per, template: ti };
                    {
                        let mut g = found.lock().unwrap();
                        g.entry(normalized(&cand.blocks)).or_insert_with(|| cand.clone());
                    }
                    let bigger = cand.blocks.len() > cur.blocks.len();
                    let slower = cand.rate() + 1e-9 < cur.rate();
                    let better = cand.blocks.len() < cur.blocks.len() || cand.rate() > cur.rate() + 1e-9;
                    if (!bigger && !slower) || (bigger && !slower && r.below(20) == 0) || (!bigger && slower && r.below(50) == 0) {
                        if better {
                            stale = 0;
                        }
                        cur = cand;
                    } else {
                        stale += 1;
                    }
                }
            });
        }
    });
    // the Pareto front: no other duper is both no bigger and no slower (and better in one)
    let g = found.into_inner().unwrap();
    let all: Vec<&Found> = g.values().collect();
    let mut front: Vec<&Found> = all
        .iter()
        .copied()
        .filter(|f| !all.iter().any(|h| h.blocks.len() <= f.blocks.len() && h.rate() >= f.rate() && (h.blocks.len() < f.blocks.len() || h.rate() > f.rate() + 1e-9)))
        .collect();
    front.sort_by(|a, b| a.blocks.len().cmp(&b.blocks.len()).then(b.rate().partial_cmp(&a.rate()).unwrap()));
    std::fs::create_dir_all(o.out.join("front"))?;
    let mut tsv = String::from("name\tblocks\tperiod\tper_period\trate\n");
    let mut seen_cls: HashMap<(usize, u32, usize), usize> = HashMap::new();
    for f in &front {
        let k = seen_cls.entry((f.blocks.len(), f.p, f.per)).or_insert(0);
        *k += 1;
        let name = format!("tnt-p{}-x{}-{}b-{}", f.p, f.per, f.blocks.len(), k);
        let mut d = starts[f.template].design.clone();
        d.name = name.clone();
        d.blocks = f.blocks.clone();
        std::fs::write(o.out.join("front").join(format!("{name}.txt")), d.to_text())?;
        tsv += &format!("{name}\t{}\t{}\t{}\t{:.1}\n", f.blocks.len(), f.p, f.per, f.rate());
    }
    std::fs::write(o.out.join("front.tsv"), tsv)?;
    eprintln!("  {} dupers seen, {} on the front", g.len(), front.len());
    Ok(front.len())
}
