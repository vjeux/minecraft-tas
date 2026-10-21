//! `pext anneal`: simulated annealing for the smallest BOX (volume first, then blocks), judged on
//! redsim like `pext lns`. The volume is part of the energy (1000 per cell of the design's box with
//! the target), so with `--slack` the box may grow for a while to rearrange the design and then
//! shrink: the move that took the 1-wide shulker loader from 1x7x10 to 1x7x8.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::Result;
use mctas::design::Design;

use crate::rs;
use crate::search;
use crate::spec::Pext;

type Pos = [i32; 3];

/// A small xorshift generator (reproducible runs from `--seed`).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
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

/// `pext anneal` settings (see `Cmd::Anneal`).
pub struct AnnealOptions {
    pub steps: usize,
    pub batch: usize,
    pub threads: usize,
    pub palette: Vec<String>,
    /// Keep a flat design flat: new blocks only in this z plane.
    pub plane_z: Option<i32>,
    /// How far outside the current box an added or moved block may go.
    pub slack: i32,
    /// Starting temperature (energy units: 1000 per cell of volume, 10 per block).
    pub temp: f64,
    pub seed: u64,
    /// Up to this many edits per candidate.
    pub moves: usize,
    /// The lever may move (to a free cell next to it).
    pub free_lever: bool,
    /// When no candidate improves, take a random passing one (not the batch's best).
    pub random_pick: bool,
    pub out_dir: Option<PathBuf>,
}

/// A judged design: (volume, blocks, extend + retract gt).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score {
    pub volume: i64,
    pub blocks: usize,
    pub gt: u32,
}

pub fn volume(p: &Pext) -> i64 {
    let (lo, hi) = p.bounds();
    (0..3).map(|i| (hi[i] - lo[i] + 1) as i64).product()
}

/// The game judge's volume: the box of every non-air cell over a cycle, the input left out
/// (redsim's run box); the static box with the target when the run gave none.
pub fn run_volume(p: &Pext, r: &rs::Report) -> i64 {
    match r.run_box {
        Some((lo, hi)) => (0..3).map(|i| (hi[i] - lo[i] + 1) as i64).product(),
        None => volume(p),
    }
}

/// The redsim judge (the screen first, then the full judge): the score when the design works.
pub fn judge(d: &Design) -> Option<(Score, Pext)> {
    let p = Pext::from_design(d.clone()).ok()?;
    let prep = rs::Prepared::new(&p, 2).ok()?;
    // the screen (2 cycles, no per-tick snapshots) with room for long designs: its default 120 gt
    // phases refuse an N = 10 cascade (135 gt to retract)
    let screen = rs::Params { max_phase: 400, ..rs::Params::screen() };
    if !rs::judge(&prep, &screen, false).0.works() {
        return None;
    }
    let (r, _) = rs::judge(&prep, &rs::Params::default(), false);
    if !r.works() {
        return None;
    }
    let c = r.first();
    Some((Score { volume: run_volume(&p, &r), blocks: p.blocks(), gt: c.extend + c.retract }, p))
}

fn energy(s: &Score) -> i64 {
    1000 * s.volume + 10 * s.blocks as i64 + s.gt as i64 / 10
}

/// One random edit (None: no edit possible this way): delete, re-state, add, move, swap.
fn mutate(d: &Design, p: &Pext, r: &mut Rng, o: &AnnealOptions, pal: &[String], lo: Pos, hi: Pos, path: &[Pos]) -> Option<Design> {
    let mut d = d.clone();
    let fixed = |q: &Pos| *q == p.payload || (*q == p.input && !o.free_lever);
    let cells: Vec<Pos> = d.blocks.keys().copied().filter(|q| !fixed(q)).collect();
    let free = |d: &Design, q: &Pos| {
        !d.blocks.contains_key(q) && !path.contains(q) && *q != p.target() && (0..3).all(|i| q[i] >= lo[i] && q[i] <= hi[i]) && o.plane_z.is_none_or(|z| q[2] == z)
    };
    let neighbours = |q: Pos| -> Vec<Pos> { [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().map(|dv| [q[0] + dv[0], q[1] + dv[1], q[2] + dv[2]]).collect() };
    match r.below(5) {
        // delete
        0 => {
            let q = *cells.get(r.below(cells.len()))?;
            if q == p.input {
                return None;
            }
            d.blocks.remove(&q);
        }
        // re-state
        1 => {
            let q = *cells.get(r.below(cells.len()))?;
            if q == p.input {
                return None;
            }
            d.blocks.insert(q, pal[r.below(pal.len())].clone());
        }
        // add next to a block
        2 => {
            let all: Vec<Pos> = d.blocks.keys().copied().collect();
            let q = neighbours(all[r.below(all.len())]);
            let q = q[r.below(q.len())];
            if !free(&d, &q) {
                return None;
            }
            d.blocks.insert(q, pal[r.below(pal.len())].clone());
        }
        // move one cell
        3 => {
            let q = *cells.get(r.below(cells.len()))?;
            let to = neighbours(q);
            let to = to[r.below(to.len())];
            if !free(&d, &to) {
                return None;
            }
            let s = d.blocks.remove(&q)?;
            d.blocks.insert(to, s);
            if q == p.input {
                d.ports.insert("input".into(), to);
            }
        }
        // swap two blocks' states
        _ => {
            let a = *cells.get(r.below(cells.len()))?;
            let b = *cells.get(r.below(cells.len()))?;
            if a == b || a == p.input || b == p.input {
                return None;
            }
            let (sa, sb) = (d.blocks[&a].clone(), d.blocks[&b].clone());
            if sa == sb {
                return None;
            }
            d.blocks.insert(a, sb);
            d.blocks.insert(b, sa);
        }
    }
    Some(d)
}

/// `pext anneal`: the best (volume, blocks, gt) found; every new best saved in `--out-dir`.
pub fn anneal(path: &Path, o: &AnnealOptions) -> Result<(Score, Design)> {
    let p0 = Pext::load(path)?;
    let Some((s0, _)) = judge(&p0.design) else { anyhow::bail!("{}: does not work on redsim", p0.name()) };
    let mut pal = o.palette.clone();
    if o.plane_z.is_some() {
        pal.retain(|s| !s.contains("facing=north") && !s.contains("facing=south"));
    }
    let path_cells = search::path_cells(&p0);
    let mut cur = p0.design.clone();
    let mut cur_s = s0;
    let mut best = (s0, cur.clone());
    println!("{}: {} blocks, volume {}, {} gt (extend + retract)", p0.name(), s0.blocks, s0.volume, s0.gt);
    let mut rng = Rng::new(o.seed);
    let t0 = std::time::Instant::now();
    for step in 0..o.steps {
        let temp = o.temp * (1.0 - step as f64 / o.steps as f64);
        let p = Pext::from_design(cur.clone())?;
        let (blo, bhi) = p.bounds();
        let lo = [blo[0] - o.slack, blo[1] - o.slack, blo[2] - o.slack];
        let hi = [bhi[0] + o.slack, bhi[1] + o.slack, bhi[2] + o.slack];
        let seeds: Vec<u64> = (0..o.batch).map(|_| rng.next()).collect();
        let next = AtomicUsize::new(0);
        let results: Mutex<Vec<(Score, Design)>> = Mutex::new(Vec::new());
        std::thread::scope(|sc| {
            for _ in 0..o.threads.max(1) {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= seeds.len() {
                            break;
                        }
                        let mut r = Rng::new(seeds[i]);
                        let n = 1 + r.below(o.moves);
                        let mut d = cur.clone();
                        let mut ok = true;
                        for _ in 0..n {
                            match mutate(&d, &p, &mut r, o, &pal, lo, hi, &path_cells) {
                                Some(x) => d = x,
                                None => {
                                    ok = false;
                                    break;
                                }
                            }
                        }
                        if !ok || (d.blocks == cur.blocks && d.ports == cur.ports) {
                            continue;
                        }
                        if let Some((s, _)) = judge(&d) {
                            results.lock().unwrap().push((s, d));
                        }
                    }
                });
            }
        });
        let mut results = results.into_inner().unwrap();
        results.sort_by_key(|x| energy(&x.0));
        let Some(first) = results.first().map(|x| energy(&x.0)) else { continue };
        // the lowest energy (among equals one at random); with --random-pick any passing
        // candidate (classic annealing: a box that must grow before it can shrink gets its turn)
        let pick = if o.random_pick && first >= energy(&cur_s) { rng.below(results.len()) } else { rng.below(results.iter().take_while(|x| energy(&x.0) == first).count()) };
        let (s, d) = results.swap_remove(pick);
        let de = energy(&s) - energy(&cur_s);
        let take = de <= 0 || (temp > 0.0 && (rng.next() % 1_000_000) as f64 / 1e6 < (-(de as f64) / temp).exp());
        if !take {
            continue;
        }
        cur = d;
        cur_s = s;
        if s < best.0 {
            best = (s, cur.clone());
            println!("step {step}: {} blocks, volume {}, {} gt ({:.0} s)", s.blocks, s.volume, s.gt, t0.elapsed().as_secs_f64());
            if let Some(dir) = &o.out_dir {
                std::fs::create_dir_all(dir)?;
                let mut dd = cur.clone();
                dd.name = format!("{}-v{}-b{}-s{step}", p0.name(), s.volume, s.blocks);
                dd.save(&dir.join(format!("{}.txt", dd.name)))?;
            }
        }
        if step % 100 == 99 {
            eprintln!("  step {}: volume {}, {} blocks, T {temp:.0} ({:.0} s)", step + 1, cur_s.volume, cur_s.blocks, t0.elapsed().as_secs_f64());
        }
    }
    println!("end: {} blocks, volume {}, {} gt", best.0.blocks, best.0.volume, best.0.gt);
    Ok(best)
}
