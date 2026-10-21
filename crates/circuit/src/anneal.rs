//! Box-bounded annealing over a design's blocks (track clk-rt, 2026-10-21): the move set and the
//! chain loop shared by `circuit tnt compact` (TNT dupers: its own screen and judge) and `circuit
//! compact` (any kind: its redsim judge, the same kind and axis). Random moves: delete, step one
//! cell, move next to the design, replace by a palette state, add a palette block, turn, swap two,
//! translate a cluster of touching blocks, relocate a cluster turned about y, push a face layer of
//! the box inward. Ports follow their blocks; port blocks (and whatever `keep` names) are never
//! deleted or replaced. A big move that broke the design gets a short greedy repair burst around
//! the cells it touched; one move in 25 may walk sideways within its tier; a chain restarts from
//! the best after `stale` moves without progress. Every new passing design is written to
//! DIR/found/ at once, each improvement to DIR/best.txt; [`front`] and [`write_front`] give the
//! (volume, blocks) front.

use anyhow::Result;
use clock::Pos;
use mctas::design::Design;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A xorshift generator (one per chain).
pub struct Rng(pub u64);

impl Rng {
    /// The generator of chain `worker` for run seed `seed`.
    pub fn new(seed: u64, worker: usize) -> Rng {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(worker as u64 * 104729 + 7) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

pub const DIRS6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

pub fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A search state: the blocks and the ports (which follow their blocks).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct St {
    pub b: BTreeMap<Pos, String>,
    pub ports: BTreeMap<String, Pos>,
}

/// A translation-free state key.
pub type Key = (Vec<(Pos, String)>, Vec<(String, Pos)>);

impl St {
    pub fn of(d: &Design) -> St {
        St { b: d.blocks.clone(), ports: d.ports.clone() }
    }
    /// The seed's design with this state's blocks and ports.
    pub fn design(&self, seed: &Design) -> Design {
        let mut d = seed.clone();
        d.blocks = self.b.clone();
        d.ports = self.ports.clone();
        d
    }
    /// Translation-free key (blocks and ports relative to the box's low corner).
    pub fn key(&self) -> Key {
        let lo = self.b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
        let rel = |p: &Pos| [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]];
        (self.b.iter().map(|(p, s)| (rel(p), s.clone())).collect(), self.ports.iter().map(|(n, p)| (n.clone(), rel(p))).collect())
    }
    /// Blocks outside the region.
    pub fn outside(&self, region: (Pos, Pos)) -> i64 {
        self.b.keys().filter(|p| !(0..3).all(|i| p[i] >= region.0[i] && p[i] <= region.1[i])).count() as i64
    }
}

/// A candidate's score, compared field by field (derive order).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score {
    pub tier: u8,
    /// Within a tier: the evaluator's gradient, before the squeeze.
    pub partial: i64,
    /// Minus the blocks outside the region (a squeeze: a seed bigger than the region is moved
    /// in, one block at a time, never at the cost of its tier).
    pub outside: i64,
    /// Minus the volume, minus the block count (bigger is better).
    pub neg_volume: i64,
    pub neg_blocks: i64,
}

/// The bounding-box volume of the blocks and its size.
pub fn volume(b: &BTreeMap<Pos, String>) -> (i64, [i32; 3]) {
    if b.is_empty() {
        return (0, [0; 3]);
    }
    let lo = b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
    let hi = b.keys().fold([i32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
    let s = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    (s[0] as i64 * s[1] as i64 * s[2] as i64, s)
}

/// The state with its `facing=` turned to another allowed value (None: not directional).
pub fn turned(s: &str, r: &mut Rng) -> Option<String> {
    let i = s.find("facing=")? + "facing=".len();
    let j = s[i..].find([',', ']']).map(|k| i + k)?;
    let cur = &s[i..j];
    let id = s.split('[').next().unwrap_or(s);
    let horizontal = !(id.ends_with("observer") || id.ends_with("piston") || id.ends_with("dispenser") || id.ends_with("dropper"));
    let opts: &[&str] = if horizontal { &["north", "east", "south", "west"] } else { &["north", "east", "south", "west", "up", "down"] };
    let alts: Vec<&&str> = opts.iter().filter(|o| **o != cur).collect();
    if alts.is_empty() {
        return None;
    }
    let n = alts[r.below(alts.len())];
    Some(format!("{}{}{}", &s[..i], n, &s[j..]))
}

/// Where blocks may go: the region, minus frozen cells (never moved, never filled).
pub struct Space<'a> {
    pub region: (Pos, Pos),
    pub frozen: &'a BTreeSet<Pos>,
}

impl Space<'_> {
    pub fn inside(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.region.0[i] && p[i] <= self.region.1[i])
    }
    pub fn free(&self, b: &BTreeMap<Pos, String>, p: Pos) -> bool {
        self.inside(p) && !self.frozen.contains(&p) && !b.contains_key(&p)
    }
    pub fn movable(&self, p: Pos) -> bool {
        !self.frozen.contains(&p)
    }
}

/// A move's result: the new state, the cells it changed (old and new), whether it was a big
/// (structural) move worth a repair burst.
pub struct Moved {
    pub st: St,
    pub touched: Vec<Pos>,
    pub big: bool,
}

/// `keep(blocks, cell)`: the block there is never deleted or replaced (port blocks always are).
pub type Keep<'a> = &'a (dyn Fn(&BTreeMap<Pos, String>, Pos) -> bool + Sync);

/// A connected cluster of movable blocks from `pick` (random BFS order, up to `size`).
fn cluster(r: &mut Rng, b: &BTreeMap<Pos, String>, sp: &Space, pick: Pos, size: usize) -> Vec<Pos> {
    let mut cl: Vec<Pos> = vec![pick];
    let mut seen: BTreeSet<Pos> = BTreeSet::from([pick]);
    let mut i = 0;
    while i < cl.len() && cl.len() < size {
        let c = cl[i];
        i += 1;
        let mut ns: Vec<Pos> = DIRS6.iter().map(|d| add(c, *d)).filter(|q| b.contains_key(q) && sp.movable(*q) && !seen.contains(q)).collect();
        for k in (1..ns.len()).rev() {
            ns.swap(k, r.below(k + 1));
        }
        for q in ns {
            if cl.len() >= size {
                break;
            }
            seen.insert(q);
            cl.push(q);
        }
    }
    cl
}

/// One random move; None when the move does not apply. `near`: only cells within 2 of these
/// (a repair burst's local moves: delete, step, replace, add, turn).
pub fn mutate(r: &mut Rng, cur: &St, sp: &Space, pal: &[String], near: Option<&[Pos]>, keep: Keep) -> Option<Moved> {
    let mut b = cur.b.clone();
    let port_cells: BTreeSet<Pos> = cur.ports.values().copied().filter(|p| b.contains_key(p)).collect();
    let close = |p: Pos| near.is_none_or(|n| n.iter().any(|q| (0..3).all(|i| (p[i] - q[i]).abs() <= 2)));
    let keys: Vec<Pos> = b.keys().copied().filter(|k| sp.movable(*k) && close(*k)).collect();
    let next_to = |r: &mut Rng, b: &BTreeMap<Pos, String>| -> Option<Pos> {
        let all: Vec<Pos> = b.keys().copied().filter(|p| close(*p)).collect();
        if all.is_empty() {
            return None;
        }
        for _ in 0..40 {
            let c = all[r.below(all.len())];
            let q = add(c, DIRS6[r.below(6)]);
            if sp.free(b, q) && close(q) {
                return Some(q);
            }
        }
        None
    };
    let mut touched: Vec<Pos> = Vec::new();
    // block moves (from, to): the ports follow them
    let mut moves: Vec<(Pos, Pos)> = Vec::new();
    let mut big = false;
    let roll = if near.is_some() { [0, 10, 35, 43, 51][r.below(5)] + r.below(8) } else { r.below(100) };
    if keys.is_empty() && !(43..=50).contains(&roll) {
        return None;
    }
    let pick = if keys.is_empty() { [0; 3] } else { keys[r.below(keys.len())] };
    let fixed_kind = port_cells.contains(&pick) || (b.contains_key(&pick) && keep(&b, pick));
    match roll {
        // delete
        0..=9 => {
            if fixed_kind {
                return None;
            }
            b.remove(&pick);
            touched.push(pick);
        }
        // step one cell
        10..=24 => {
            let q = add(pick, DIRS6[r.below(6)]);
            if !sp.free(&b, q) {
                return None;
            }
            let s = b.remove(&pick)?;
            b.insert(q, s);
            touched.extend([pick, q]);
            moves.push((pick, q));
        }
        // move next to the design
        25..=34 => {
            let s = b.remove(&pick)?;
            let q = next_to(r, &b)?;
            b.insert(q, s);
            touched.extend([pick, q]);
            moves.push((pick, q));
        }
        // replace by a palette state
        35..=42 => {
            if fixed_kind || pal.is_empty() {
                return None;
            }
            b.insert(pick, pal[r.below(pal.len())].clone());
            touched.push(pick);
        }
        // add a palette block
        43..=50 => {
            if pal.is_empty() {
                return None;
            }
            let q = next_to(r, &b)?;
            b.insert(q, pal[r.below(pal.len())].clone());
            touched.push(q);
        }
        // turn
        51..=58 => {
            let t = turned(&b[&pick], r)?;
            b.insert(pick, t);
            touched.push(pick);
        }
        // swap two
        59..=64 => {
            let other = keys[r.below(keys.len())];
            if other == pick || b[&other] == b[&pick] {
                return None;
            }
            let (x, y) = (b[&pick].clone(), b[&other].clone());
            b.insert(pick, y);
            b.insert(other, x);
            touched.extend([pick, other]);
            moves.extend([(pick, other), (other, pick)]);
        }
        // translate a cluster of touching blocks by one cell
        65..=82 => {
            let size = 1 + r.below(16);
            let cl = cluster(r, &b, sp, pick, size);
            let d = DIRS6[r.below(6)];
            let set: BTreeSet<Pos> = cl.iter().copied().collect();
            if !cl.iter().all(|c| {
                let q = add(*c, d);
                set.contains(&q) || sp.free(&b, q)
            }) {
                return None;
            }
            let moved: Vec<(Pos, Pos, String)> = cl.iter().map(|c| (*c, add(*c, d), b[c].clone())).collect();
            for c in &cl {
                b.remove(c);
            }
            for (c, q, s) in moved {
                touched.extend([c, q]);
                moves.push((c, q));
                b.insert(q, s);
            }
            big = cl.len() > 1;
        }
        // relocate a cluster anywhere in the region, turned 0-3 quarter turns about y
        83..=91 => {
            let size = 2 + r.below(19);
            let cl = cluster(r, &b, sp, pick, size);
            let turns = r.below(4) as u8;
            let set: BTreeSet<Pos> = cl.iter().copied().collect();
            let (lo, hi) = sp.region;
            let anchor = [lo[0] + r.below((hi[0] - lo[0] + 1) as usize) as i32, lo[1] + r.below((hi[1] - lo[1] + 1) as usize) as i32, lo[2] + r.below((hi[2] - lo[2] + 1) as usize) as i32];
            let moved: Vec<(Pos, Pos, String)> = cl
                .iter()
                .map(|c| {
                    let rel = clock::rotate_pos([c[0] - pick[0], c[1] - pick[1], c[2] - pick[2]], turns);
                    (*c, add(anchor, rel), clock::canon::rotate_state(&b[c], turns))
                })
                .collect();
            if !moved.iter().all(|(_, q, _)| set.contains(q) || sp.free(&b, *q)) {
                return None;
            }
            for c in &cl {
                b.remove(c);
            }
            for (c, q, s) in moved {
                touched.extend([c, q]);
                moves.push((c, q));
                b.insert(q, s);
            }
            big = true;
        }
        // push a face layer of the bounding box one cell inward
        _ => {
            let lo = b.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
            let hi = b.keys().fold([i32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
            let axis = r.below(3);
            let low_face = r.below(2) == 0;
            let (at, step) = if low_face { (lo[axis], 1) } else { (hi[axis], -1) };
            let layer: Vec<Pos> = b.keys().copied().filter(|p| p[axis] == at).collect();
            if layer.is_empty() || layer.iter().any(|p| !sp.movable(*p)) {
                return None;
            }
            let mut d = [0; 3];
            d[axis] = step;
            if !layer.iter().all(|p| sp.free(&b, add(*p, d))) {
                return None;
            }
            for p in layer {
                let s = b.remove(&p)?;
                b.insert(add(p, d), s);
                touched.extend([p, add(p, d)]);
                moves.push((p, add(p, d)));
            }
            big = true;
        }
    }
    let mut ports = cur.ports.clone();
    for p in ports.values_mut() {
        if let Some((_, to)) = moves.iter().find(|(from, _)| from == p) {
            *p = *to;
        }
    }
    let st = St { b, ports };
    if &st == cur { None } else { Some(Moved { st, touched, big }) }
}

/// A passing design the chains met.
#[derive(Clone)]
pub struct Found {
    pub st: St,
    pub verdict: String,
    pub score: Score,
}

/// The chain loop's settings.
pub struct Chains<'a> {
    pub seed: &'a Design,
    pub space: Space<'a>,
    pub palette: &'a [String],
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    /// Moves without progress before a chain restarts from the best.
    pub stale: usize,
    /// Local repair moves after a big move that broke the design.
    pub repair: usize,
    /// The tier of a passing design (recorded in DIR/found/ and the front).
    pub pass_tier: u8,
    pub keep: Keep<'a>,
    pub out: &'a Path,
    /// Progress lines on stderr.
    pub verbose: bool,
}

/// One chain's evaluator: (candidate, the chain's current score, a sideways walk is allowed) ->
/// (score, verdict). Each chain gets its own from the factory (its own caches).
pub type Eval<'a> = Box<dyn FnMut(&St, Score, bool) -> (Score, String) + 'a>;

/// Run the chains from `st0` (scored `s0`, verdict `v0`): the passing designs met, by key, and
/// the best state.
pub fn run<'e, F>(c: &Chains, st0: &St, s0: Score, v0: &str, make_eval: F) -> (HashMap<Key, Found>, (Score, St))
where
    F: Fn() -> Eval<'e> + Sync,
{
    let found: Mutex<HashMap<Key, Found>> = Mutex::new(HashMap::new());
    let best: Mutex<(Score, St)> = Mutex::new((s0, st0.clone()));
    if s0.tier >= c.pass_tier {
        found.lock().unwrap().insert(st0.key(), Found { st: st0.clone(), verdict: v0.to_string(), score: s0 });
    }
    let iter = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..c.workers.max(1) {
            let (found, best, iter, make_eval) = (&found, &best, &iter, &make_eval);
            sc.spawn(move || {
                let mut r = Rng::new(c.rng, wi);
                let mut eval = make_eval();
                let mut cur = st0.clone();
                let mut cur_s = s0;
                let mut stale = 0usize;
                loop {
                    let i = iter.fetch_add(1, Ordering::Relaxed);
                    if i >= c.iterations {
                        break;
                    }
                    if c.verbose && i > 0 && i % 2000 == 0 {
                        let g = best.lock().unwrap();
                        let n4 = found.lock().unwrap().len();
                        eprintln!("  {i}/{} moves, {:.0} s: best tier {} partial {} volume {} {} blocks; {n4} passing designs", c.iterations, t0.elapsed().as_secs_f64(), g.0.tier, g.0.partial, -g.0.neg_volume, g.1.b.len());
                    }
                    let Some(mv) = mutate(&mut r, &cur, &c.space, c.palette, None, c.keep) else { continue };
                    let walk = r.below(25) == 0;
                    let (mut sc_, mut verdict) = eval(&mv.st, cur_s, walk);
                    let mut cand = mv.st;
                    // a big move that broke the design: a short local repair burst around the
                    // cells it touched, greedy on the score
                    if mv.big && sc_ < cur_s && c.repair > 0 {
                        for _ in 0..c.repair {
                            let Some(m2) = mutate(&mut r, &cand, &c.space, c.palette, Some(&mv.touched), c.keep) else { continue };
                            let (s2, v2) = eval(&m2.st, cur_s, false);
                            if s2 > sc_ {
                                sc_ = s2;
                                verdict = v2;
                                cand = m2.st;
                                if sc_ >= cur_s {
                                    break;
                                }
                            }
                        }
                    }
                    if sc_.tier >= c.pass_tier {
                        let mut g = found.lock().unwrap();
                        let fresh = !g.contains_key(&cand.key());
                        g.entry(cand.key()).or_insert_with(|| Found { st: cand.clone(), verdict: verdict.clone(), score: sc_ });
                        if fresh {
                            // every passing design is written at once (a long run can be read while it goes)
                            let mut d = cand.design(c.seed);
                            d.name = format!("{}-v{}-{}b-{}", c.seed.name, -sc_.neg_volume, cand.b.len(), g.len());
                            let _ = std::fs::create_dir_all(c.out.join("found"));
                            let _ = std::fs::write(c.out.join("found").join(format!("{}.txt", d.name)), format!("# {verdict}\n{}", d.to_text()));
                        }
                    }
                    {
                        let mut g = best.lock().unwrap();
                        if sc_ > g.0 {
                            if c.verbose {
                                eprintln!("  NEW BEST tier {} partial {} volume {} {} blocks: {}", sc_.tier, sc_.partial, -sc_.neg_volume, cand.b.len(), verdict.chars().take(160).collect::<String>());
                            }
                            *g = (sc_, cand.clone());
                            let mut d = cand.design(c.seed);
                            d.name = format!("{}-best", c.seed.name);
                            let _ = std::fs::create_dir_all(c.out);
                            let _ = std::fs::write(c.out.join("best.txt"), format!("# tier {} {verdict}\n{}", sc_.tier, d.to_text()));
                        }
                    }
                    let same_tier_walk = sc_.tier == cur_s.tier && walk;
                    if sc_ >= cur_s || same_tier_walk {
                        if sc_ > cur_s {
                            stale = 0;
                        } else {
                            stale += 1;
                        }
                        cur = cand;
                        cur_s = sc_;
                    } else {
                        stale += 1;
                    }
                    if stale > c.stale {
                        let g = best.lock().unwrap();
                        cur = g.1.clone();
                        cur_s = g.0;
                        stale = 0;
                    }
                }
            });
        }
    });
    (found.into_inner().unwrap(), best.into_inner().unwrap())
}

/// The (volume, blocks) front of the passing designs, smallest first, at most `per_key` designs
/// per (volume, blocks). The volume is the score's (the evaluator's judged box).
pub fn front(found: &HashMap<Key, Found>, per_key: usize) -> Vec<&Found> {
    let all: Vec<&Found> = found.values().collect();
    let key = |f: &Found| (-f.score.neg_volume, f.st.b.len());
    let mut front: Vec<&Found> = all.iter().copied().filter(|f| !all.iter().any(|h| key(h).0 <= key(f).0 && key(h).1 <= key(f).1 && key(h) != key(f))).collect();
    front.sort_by_key(|f| (key(f), f.st.key()));
    let mut seen: HashMap<(i64, usize), usize> = HashMap::new();
    front.retain(|f| {
        let n = seen.entry(key(f)).or_insert(0);
        *n += 1;
        *n <= per_key
    });
    front
}

/// The front's file name for entry `i`.
pub fn front_name(seed: &Design, f: &Found, i: usize) -> String {
    format!("{}-v{}-{}b-{i}", seed.name, -f.score.neg_volume, f.st.b.len())
}

/// Write the front to DIR/front/ and DIR/front.tsv.
pub fn write_front(seed: &Design, front: &[&Found], out: &Path) -> Result<()> {
    std::fs::create_dir_all(out.join("front"))?;
    let mut tsv = String::from("name\tvolume\tsize\tblocks\tverdict\n");
    for (i, f) in front.iter().enumerate() {
        let (_, s) = volume(&f.st.b);
        let name = front_name(seed, f, i);
        let mut d = f.st.design(seed);
        d.name = name.clone();
        std::fs::write(out.join("front").join(format!("{name}.txt")), d.to_text())?;
        tsv += &format!("{name}\t{}\t{}x{}x{} (static)\t{}\t{}\n", -f.score.neg_volume, s[0], s[1], s[2], f.st.b.len(), f.verdict);
    }
    std::fs::write(out.join("front.tsv"), tsv)?;
    Ok(())
}
