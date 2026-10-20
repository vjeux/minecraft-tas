//! `mctas circuit tnt search`: seeded random search for TNT dupers on redsim. The seed design
//! (an engine: a clock, a piston with its kick...) stays fixed; each sample adds k blocks drawn
//! from the palette at random cells of the search box (a dust or a floor fan only on a full block,
//! a wall fan only against one, at least the required kinds), runs it on redsim and keeps it when
//! it DUPES: periodic, at least one TNT primed per period, and every TNT block of the design still
//! standing in the periodic state (none consumed). Found designs are written once per distinct
//! block set to `DIR/designs/` with a line in `DIR/found.tsv`; `circuit tnt run` confirms them in
//! the game.

use anyhow::{Context, Result, bail};
use clock::{Clock, Pos};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub seed: PathBuf,
    /// Inclusive search box, in the seed's coordinates.
    pub lo: Pos,
    pub hi: Pos,
    pub palette: Vec<String>,
    pub k: usize,
    pub kmin: usize,
    /// Kinds every sample must contain (palette shorthands).
    pub require: Vec<String>,
    pub samples: usize,
    /// Keep only dupers with at least this many TNT per period (a seed that already dupes: 2+
    /// asks for bursts), and at most `max_found` of them.
    pub min_per: usize,
    pub max_found: usize,
    /// Cells that must stay empty.
    pub forbid: Vec<Pos>,
    pub ticks: u32,
    pub rng: u64,
    pub workers: usize,
    pub out: PathBuf,
    /// Keep only dupers that survive their own explosions on redsim at `survive` evenly spaced
    /// kicks (0: no explosion screen); the full all-kicks sweep and the game come after.
    pub survive: usize,
}

/// A palette shorthand's block states.
pub fn expand(name: &str) -> Result<Vec<String>> {
    let dirs = ["north", "east", "south", "west"];
    let all = ["down", "up", "north", "south", "west", "east"];
    Ok(match name {
        "slime" => vec!["minecraft:slime_block".into()],
        "honey" => vec!["minecraft:honey_block".into()],
        "stone" => vec!["minecraft:stone".into()],
        "obsidian" => vec!["minecraft:obsidian".into()],
        "tnt" => vec!["minecraft:tnt[unstable=false]".into()],
        "fan" => vec!["minecraft:dead_tube_coral_fan[waterlogged=false]".into()],
        "wallfan" => dirs.iter().map(|d| format!("minecraft:dead_tube_coral_wall_fan[facing={d},waterlogged=false]")).collect(),
        "dust" => vec!["minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into()],
        "rblock" => vec!["minecraft:redstone_block".into()],
        "observer" => all.iter().map(|d| format!("minecraft:observer[facing={d},powered=false]")).collect(),
        "piston" => all.iter().map(|d| format!("minecraft:piston[extended=false,facing={d}]")).collect(),
        "sticky" => all.iter().map(|d| format!("minecraft:sticky_piston[extended=false,facing={d}]")).collect(),
        s if s.contains(':') || s.contains('[') => vec![if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") }],
        _ => bail!("unknown palette entry {name:?}"),
    })
}

/// The shorthand a state belongs to (for `require`).
fn kind_of(state: &str) -> &'static str {
    let id = state.trim_start_matches("minecraft:");
    if id.starts_with("tnt") {
        "tnt"
    } else if id.contains("coral_wall_fan") {
        "wallfan"
    } else if id.contains("coral_fan") {
        "fan"
    } else if id.starts_with("redstone_wire") {
        "dust"
    } else if id.starts_with("redstone_block") {
        "rblock"
    } else if id.starts_with("observer") {
        "observer"
    } else if id.starts_with("sticky_piston") {
        "sticky"
    } else if id.starts_with("piston") {
        "piston"
    } else if id.starts_with("slime") {
        "slime"
    } else {
        "other"
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

const DIRS6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Is the block a full sturdy cube (supports dust and fans)?
fn full(b: &Block) -> bool {
    redsim::behavior::sturdy_of(b, redsim::Dir::Up, redsim::behavior::Support::Full) && redsim::behavior::full_cube(b)
}

/// Every support rule holds (dust and floor fans on a full block, wall fans against one).
fn supported(cells: &BTreeMap<Pos, Block>) -> bool {
    let full_at = |p: Pos| cells.get(&p).is_some_and(full);
    cells.iter().all(|(p, b)| match b {
        Block::Wire { .. } | Block::CoralFan { .. } => full_at([p[0], p[1] - 1, p[2]]),
        Block::CoralWallFan { facing, .. } => {
            let q = facing.opposite();
            full_at([p[0] + q.step()[0], p[1] + q.step()[1], p[2] + q.step()[2]])
        }
        _ => true,
    })
}

/// What the samples did (for the search's progress lines).
#[derive(Default)]
pub struct Stats {
    pub ran: AtomicUsize,
    pub unmodeled: AtomicUsize,
    pub no_period: AtomicUsize,
    pub primed: AtomicUsize,
    pub consumed: AtomicUsize,
}

impl Stats {
    pub fn line(&self) -> String {
        let g = |a: &AtomicUsize| a.load(Ordering::Relaxed);
        format!("ran {}, unmodeled/drops {}, no period {}, primed {}, consumed {}", g(&self.ran), g(&self.unmodeled), g(&self.no_period), g(&self.primed), g(&self.consumed))
    }
}

/// The verdict of one candidate: Some((period, primes per period)) when it dupes.
pub fn dupes(c: &Clock, extra: &[(Pos, Block)], ticks: u32) -> Option<(u32, usize)> {
    dupes_counted(c, extra, ticks, &Stats::default())
}

fn dupes_counted(c: &Clock, extra: &[(Pos, Block)], ticks: u32, st: &Stats) -> Option<(u32, usize)> {
    st.ran.fetch_add(1, Ordering::Relaxed);
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for (p, s) in &c.design.blocks {
        blocks.push((bp(*p), Block::parse(s).ok()?));
    }
    blocks.extend(extra.iter().map(|(p, b)| (bp(*p), *b)));
    // the judge's placement order (`Design::block_list`: sorted by position): the start-up
    // depends on it
    blocks.sort_by_key(|(p, _)| [p.x, p.y, p.z]);
    let (mut lo, mut hi) = (blocks[0].0, blocks[0].0);
    for (p, _) in &blocks {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let m = 4;
    let mut w = World::new(BlockPos::new(lo.x - m, lo.y - m, lo.z - m), [hi.x - lo.x + 1 + 2 * m, hi.y - lo.y + 1 + 2 * m, hi.z - lo.z + 1 + 2 * m]);
    w.remove_primed = true;
    let last = c.withheld().map(bp);
    let kept: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
    w.place_raw(&kept);
    w.place_updates(&kept);
    let tnt0 = kept.iter().filter(|(_, b)| matches!(b, Block::Tnt { .. })).count();
    let mut actions = c.actions();
    let last_action = actions.last().map(|(t, _)| *t).unwrap_or(0);
    let mut seen: HashMap<u64, u32> = HashMap::new();
    let mut prime_ticks: Vec<u32> = Vec::new();
    for t in 0..ticks {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            if let clock::Action::Place { pos, state, .. } = a {
                w.set_block(bp(pos), Block::parse(&state).ok()?, flags::ALL);
            }
        }
        let n0 = w.primed.len();
        w.tick();
        for _ in n0..w.primed.len() {
            prime_ticks.push(t + 1);
        }
        if !w.errors.is_empty() || !w.drops.is_empty() {
            st.unmodeled.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        if t + 1 > last_action {
            let mut h = std::collections::hash_map::DefaultHasher::new();
            w.state_key_into(&mut h);
            let key = std::hash::Hasher::finish(&h);
            if let Some(&first) = seen.get(&key) {
                let p = t + 1 - first;
                let per = prime_ticks.iter().filter(|&&x| x > first && x <= t + 1).count();
                let tnt = w.blocks().iter().filter(|(_, b)| matches!(b, Block::Tnt { .. })).count();
                if !prime_ticks.is_empty() {
                    st.primed.fetch_add(1, Ordering::Relaxed);
                }
                if tnt < tnt0 {
                    st.consumed.fetch_add(1, Ordering::Relaxed);
                }
                return if per > 0 && tnt >= tnt0 && tnt0 > 0 { Some((p, per)) } else { None };
            }
            seen.insert(key, t + 1);
        }
    }
    st.no_period.fetch_add(1, Ordering::Relaxed);
    None
}

/// Run the search; returns the number of distinct dupers found.
pub fn search(o: &SearchOptions) -> Result<usize> {
    let c = Clock::load(&o.seed).with_context(|| format!("seed {}", o.seed.display()))?;
    let mut pal: Vec<(String, Block)> = Vec::new();
    for n in &o.palette {
        for s in expand(n)? {
            let b = Block::parse(&s).map_err(|e| anyhow::anyhow!("{s}: {e}"))?;
            pal.push((s, b));
        }
    }
    let seed_cells: BTreeMap<Pos, Block> = c.design.blocks.iter().map(|(p, s)| Ok((*p, Block::parse(s).map_err(|e| anyhow::anyhow!(e))?))).collect::<Result<_>>()?;
    let mut free: Vec<Pos> = Vec::new();
    for y in o.lo[1]..=o.hi[1] {
        for z in o.lo[2]..=o.hi[2] {
            for x in o.lo[0]..=o.hi[0] {
                if !seed_cells.contains_key(&[x, y, z]) && !o.forbid.contains(&[x, y, z]) && c.withheld() != Some([x, y, z]) && !matches!(c.kick, Some(clock::Kick::Place { pos, .. }) if pos == [x, y, z]) {
                    free.push([x, y, z]);
                }
            }
        }
    }
    if free.len() < o.k {
        bail!("{} free cells in the box, k = {}", free.len(), o.k);
    }
    std::fs::create_dir_all(o.out.join("designs"))?;
    let free_set: HashSet<Pos> = free.iter().copied().collect();
    // supports for dust and fans: the palette's full blocks
    let fulls: Vec<usize> = (0..pal.len()).filter(|&s| full(&pal[s].1) && !matches!(pal[s].1, Block::Tnt { .. }) && kind_of(&pal[s].0) != "rblock").collect();
    let tried = AtomicUsize::new(0);
    let stats = Stats::default();
    let found: Mutex<(HashSet<Vec<(Pos, String)>>, Vec<String>)> = Mutex::new((HashSet::new(), Vec::new()));
    let start = std::time::Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..o.workers.max(1) {
            let (c, pal, free, seed_cells, tried, found, free_set, fulls, stats) = (&c, &pal, &free, &seed_cells, &tried, &found, &free_set, &fulls, &stats);
            sc.spawn(move || {
                let mut r = Rng(o.rng.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 * 7919 + 1) | 1);
                loop {
                    let i = tried.fetch_add(1, Ordering::Relaxed);
                    if i >= o.samples {
                        break;
                    }
                    if i > 0 && i % 200_000 == 0 {
                        let n = found.lock().unwrap().1.len();
                        eprintln!("  {i}/{} samples, {n} found, {:.0} s; {}", o.samples, start.elapsed().as_secs_f64(), stats.line());
                    }
                    let k = o.kmin + r.below(o.k - o.kmin + 1);
                    let mut cells = seed_cells.clone();
                    let mut extra: Vec<(Pos, usize)> = Vec::new();
                    let free_set: &HashSet<Pos> = free_set;
                    // a free cell, mostly next to a block already there (compact designs)
                    let pick = |r: &mut Rng, cells: &BTreeMap<Pos, Block>| -> Option<Pos> {
                        for _ in 0..40 {
                            let p = free[r.below(free.len())];
                            if cells.contains_key(&p) {
                                continue;
                            }
                            let near = DIRS6.iter().any(|d| cells.contains_key(&[p[0] + d[0], p[1] + d[1], p[2] + d[2]]));
                            if near || r.below(5) == 0 {
                                return Some(p);
                            }
                        }
                        None
                    };
                    let put = |cells: &mut BTreeMap<Pos, Block>, extra: &mut Vec<(Pos, usize)>, p: Pos, s: usize| {
                        cells.insert(p, pal[s].1);
                        extra.push((p, s));
                    };
                    // the required kinds first, each dust and fan with its support
                    let mut ok = true;
                    for q in &o.require {
                        let choices: Vec<usize> = (0..pal.len()).filter(|&s| kind_of(&pal[s].0) == q.as_str() || (q == "fan" && kind_of(&pal[s].0) == "wallfan")).collect();
                        if choices.is_empty() || extra.len() >= k {
                            ok = false;
                            break;
                        }
                        let s = choices[r.below(choices.len())];
                        let Some(p) = pick(&mut r, &cells) else {
                            ok = false;
                            break;
                        };
                        let support: Option<Pos> = match pal[s].1 {
                            Block::Wire { .. } | Block::CoralFan { .. } => Some([p[0], p[1] - 1, p[2]]),
                            Block::CoralWallFan { facing, .. } => {
                                let st = facing.opposite().step();
                                Some([p[0] + st[0], p[1] + st[1], p[2] + st[2]])
                            }
                            _ => None,
                        };
                        put(&mut cells, &mut extra, p, s);
                        if let Some(sp) = support {
                            if !cells.get(&sp).is_some_and(full) {
                                if cells.contains_key(&sp) || !free_set.contains(&sp) || fulls.is_empty() {
                                    ok = false;
                                    break;
                                }
                                put(&mut cells, &mut extra, sp, fulls[r.below(fulls.len())]);
                            }
                        }
                    }
                    if !ok {
                        continue;
                    }
                    while extra.len() < k {
                        let Some(p) = pick(&mut r, &cells) else { break };
                        put(&mut cells, &mut extra, p, r.below(pal.len()));
                    }
                    if !supported(&cells) {
                        continue;
                    }
                    let ex: Vec<(Pos, Block)> = extra.iter().map(|(p, s)| (*p, pal[*s].1)).collect();
                    if let Some((p, per)) = dupes_counted(c, &ex, o.ticks, stats).filter(|(_, per)| *per >= o.min_per) {
                        let mut key: Vec<(Pos, String)> = extra.iter().map(|(p, s)| (*p, pal[*s].0.clone())).collect();
                        key.sort();
                        if o.survive > 0 && !survives(c, &key, o.survive) {
                            continue;
                        }
                        let mut g = found.lock().unwrap();
                        if g.1.len() < o.max_found && g.0.insert(key.clone()) {
                            let n = g.1.len();
                            let name = format!("tnt-search-p{p}-x{per}-{}b-{n}", seed_cells.len() + key.len());
                            let text = design_text(c, &key, &name);
                            let _ = std::fs::write(o.out.join("designs").join(format!("{name}.txt")), text);
                            g.1.push(format!("{name}\t{p}\t{per}\t{}\t{}", seed_cells.len() + key.len(), key.iter().map(|(p, s)| format!("{},{},{}={}", p[0], p[1], p[2], clock::short(s))).collect::<Vec<_>>().join(";")));
                        }
                    }
                }
            });
        }
    });
    let g = found.into_inner().unwrap();
    eprintln!("  {}", stats.line());
    let mut tsv = String::from("name\tperiod\tprimes_per_period\tblocks\textra\n");
    for l in &g.1 {
        tsv += l;
        tsv.push('\n');
    }
    std::fs::write(o.out.join("found.tsv"), tsv)?;
    Ok(g.1.len())
}

/// Greedy shrink on redsim: drop one block at a time (never the TNT) while the design still dupes
/// with at least the same TNT per period; returns the smaller design's blocks.
pub fn shrink(c: &Clock, ticks: u32) -> Option<(BTreeMap<Pos, String>, u32, usize)> {
    let base: BTreeMap<Pos, String> = c.design.blocks.clone();
    let as_clock = |blocks: &BTreeMap<Pos, String>| -> Option<Clock> {
        let mut d = c.design.clone();
        d.blocks = blocks.clone();
        Clock::from_design(d).ok()
    };
    let (mut p, mut per) = dupes(c, &[], ticks)?;
    let mut cur = base;
    loop {
        let mut better = false;
        let keys: Vec<Pos> = cur.keys().copied().collect();
        for k in keys {
            if cur[&k].contains("tnt") || c.withheld() == Some(k) {
                continue;
            }
            let mut t = cur.clone();
            t.remove(&k);
            let Some(cc) = as_clock(&t) else { continue };
            if let Some((p2, per2)) = dupes(&cc, &[], ticks) {
                if per2 * p as usize >= per * p2 as usize {
                    cur = t;
                    p = p2;
                    per = per2;
                    better = true;
                }
            }
        }
        if !better {
            break;
        }
    }
    Some((cur, p, per))
}

/// The seed design plus the extra blocks, as a design file.
fn design_text(c: &Clock, extra: &[(Pos, String)], name: &str) -> String {
    let mut d = c.design.clone();
    d.name = name.to_string();
    for (p, s) in extra {
        d.blocks.insert(*p, s.clone());
    }
    d.to_text()
}

/// The duper (seed + extra) judged with its explosions on redsim at `kicks` evenly spaced kicks
/// (circuit::robust::judge_kick): every one must pass.
pub fn survives(c: &Clock, extra: &[(Pos, String)], kicks: usize) -> bool {
    let mut d = c.design.clone();
    for (p, s) in extra {
        d.blocks.insert(*p, s.clone());
    }
    if !d.meta.contains_key("circuit") {
        d.meta.insert("circuit".into(), "tnt-duper".into());
    }
    d.meta.remove("explode");
    let Ok(cc) = circuit::Circuit::from_design(d) else { return false };
    let o = circuit::verify::Options { margin: 3, ..Default::default() };
    let Ok(rep) = circuit::verify::sim(&cc, &o) else { return false };
    if !rep.outcome.is_some_and(|x| x.pass) {
        return false;
    }
    let s = rep.schedule;
    (1..kicks).all(|k| circuit::robust::judge_kick(&cc, &s, k as f64 / kicks as f64, 3).is_ok_and(|x| x.pass))
}
