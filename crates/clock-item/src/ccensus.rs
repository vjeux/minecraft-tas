//! The constructive census: item clocks built the way they must be built. Items keep moving only
//! around a directed CYCLE of containers (a hopper pushes into the container it faces and pulls
//! from the one above; a dropper pushes into the one it faces when triggered), and the signal
//! needs a comparator reading a cycle container. So: every flow cycle of up to `max_cycle`
//! containers, every way a comparator reads one of them (beside it, or through one conductor),
//! a floor under it when needed, then up to `extra` more blocks from a palette next to the
//! structure, then the contents of the cycle containers. Judged in redsim; irreducible clocks
//! kept (one per type and contents).

use crate::census::{self, Found, Opts};
use crate::sim::Spec;
use redsim::{Block, BlockPos, Dir};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct COpts {
    /// Cycle cells lie in [0, size) (x, y, z); the start of each cycle is its smallest cell.
    pub size: [i32; 3],
    pub max_cycle: usize,
    pub min_cycle: usize,
    pub droppers: bool,
    /// Comparators per design (1 or 2).
    pub comparators: usize,
    pub extra: usize,
    pub extra_palette: Vec<Block>,
    pub contents: Vec<String>,
    /// At most this many cycle containers hold items.
    pub max_filled: usize,
    pub max_ticks: u32,
    pub jobs: usize,
}

/// A container of a cycle: hopper or dropper, its facing (None = free: not used by the cycle).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cell {
    pos: BlockPos,
    /// None = either type (only suction edges touch it so far).
    hopper: Option<bool>,
    facing: Option<Dir>,
}

const DIRS: [Dir; 6] = [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East];
const HDIRS: [Dir; 4] = [Dir::North, Dir::East, Dir::South, Dir::West];

fn inside(p: BlockPos, size: [i32; 3]) -> bool {
    p.x >= 0 && p.y >= 0 && p.z >= 0 && p.x < size[0] && p.y < size[1] && p.z < size[2]
}

/// Every directed simple cycle (as cells with type/facing constraints), smallest cell first.
fn cycles(o: &COpts) -> Vec<Vec<Cell>> {
    let mut out = Vec::new();
    for y in 0..o.size[1] {
        for z in 0..o.size[2] {
            for x in 0..o.size[0] {
                let s = BlockPos::new(x, y, z);
                let mut path = vec![Cell { pos: s, hopper: None, facing: None }];
                walk(o, s, &mut path, &mut out);
            }
        }
    }
    out
}

fn walk(o: &COpts, s: BlockPos, path: &mut Vec<Cell>, out: &mut Vec<Vec<Cell>>) {
    let cur = *path.last().unwrap();
    for d in DIRS {
        let next = cur.pos.rel(d);
        if !inside(next, o.size) || (next < s) {
            continue;
        }
        let closing = next == s;
        if !closing && (path.len() >= o.max_cycle || path.iter().any(|c| c.pos == next)) {
            continue;
        }
        if closing && path.len() < o.min_cycle.max(2) {
            continue;
        }
        // edge cur -> next: a push (cur faces d: a hopper never faces up; droppers only when
        // allowed) or a suction (d = down, next is a hopper)
        let mut options: Vec<(Cell, Option<bool>)> = Vec::new(); // (new cur, constraint on next's type)
        if cur.facing.is_none() {
            if d != Dir::Up && cur.hopper != Some(false) {
                options.push((Cell { hopper: Some(true), facing: Some(d), ..cur }, None));
            }
            if o.droppers && cur.hopper != Some(true) {
                options.push((Cell { hopper: Some(false), facing: Some(d), ..cur }, None));
            }
        }
        if d == Dir::Down {
            options.push((cur, Some(true)));
        }
        for (newcur, next_type) in options {
            let n = path.len();
            let saved = path[n - 1];
            path[n - 1] = newcur;
            if closing {
                let first = path[0];
                let ok = match next_type {
                    Some(t) => first.hopper.is_none_or(|h| h == t),
                    None => true,
                };
                if ok {
                    let mut cyc = path.clone();
                    if let Some(t) = next_type {
                        cyc[0].hopper = Some(t);
                    }
                    out.push(cyc);
                }
            } else {
                path.push(Cell { pos: next, hopper: next_type, facing: None });
                walk(o, s, path, out);
                path.pop();
            }
            path[n - 1] = saved;
        }
    }
}

/// Every concrete assignment of a cycle's free types and facings.
fn concretize(cyc: &[Cell], droppers: bool) -> Vec<Vec<(BlockPos, Block)>> {
    let mut out = vec![Vec::new()];
    for c in cyc {
        let mut opts: Vec<Block> = Vec::new();
        let types: Vec<bool> = match c.hopper {
            Some(h) => vec![h],
            None if droppers => vec![true, false],
            None => vec![true],
        };
        for h in types {
            let facings: Vec<Dir> = match c.facing {
                Some(f) => vec![f],
                None if h => vec![Dir::Down, Dir::North, Dir::South, Dir::West, Dir::East],
                None => DIRS.to_vec(),
            };
            for f in facings {
                opts.push(if h { Block::Hopper { facing: f, enabled: true } } else { Block::Dispenser { dropper: true, facing: f, triggered: false } });
            }
        }
        let mut next = Vec::new();
        for partial in &out {
            for b in &opts {
                let mut v = partial.clone();
                v.push((c.pos, *b));
                next.push(v);
            }
        }
        out = next;
    }
    out
}

/// Comparator placements reading a cycle container: (blocks to add).
fn readers(blocks: &[(BlockPos, Block)], cyc: &HashSet<BlockPos>) -> Vec<Vec<(BlockPos, Block)>> {
    let taken: HashSet<BlockPos> = blocks.iter().map(|(p, _)| *p).collect();
    let mut out = Vec::new();
    for x in cyc {
        for h in HDIRS {
            for subtract in [false, true] {
                // beside it
                let c = x.rel(h);
                if !taken.contains(&c) {
                    out.push(vec![(c, Block::Comparator { facing: h.opposite(), subtract, powered: false })]);
                }
                // through one conductor
                let s = x.rel(h);
                let c2 = s.rel(h);
                if !taken.contains(&s) && !taken.contains(&c2) {
                    out.push(vec![(s, Block::Simple(redsim::block::Simple::Stone)), (c2, Block::Comparator { facing: h.opposite(), subtract, powered: false })]);
                }
            }
        }
    }
    out
}

/// Floors for the diodes, dust and torches that lack one: a stone under each (None if a cell
/// below is taken by something that cannot hold it).
fn add_floors(blocks: &mut Vec<(BlockPos, Block)>) -> bool {
    let mut add = Vec::new();
    let at: HashMap<BlockPos, Block> = blocks.iter().copied().collect();
    for (p, b) in blocks.iter() {
        if let Some(need) = census::needs_floor(b) {
            match at.get(&p.below()) {
                Some(f) if census::floor_ok(f, need, b) => {}
                Some(_) => return false,
                None => add.push((p.below(), Block::Simple(redsim::block::Simple::Stone))),
            }
        }
    }
    add.sort_by_key(|(p, _)| *p);
    add.dedup();
    blocks.extend(add);
    true
}

/// Run it: the clocks found, irreducible, one per type and contents.
pub fn run(o: &COpts, progress: bool) -> Vec<Found> {
    let cycles = cycles(o);
    // structures before extras: every concrete cycle with its comparator(s) and floors
    let mut cores: Vec<(Vec<(BlockPos, Block)>, Vec<BlockPos>)> = Vec::new();
    let mut core_keys = HashSet::new();
    for cyc in &cycles {
        let cyc_set: HashSet<BlockPos> = cyc.iter().map(|c| c.pos).collect();
        for conc in concretize(cyc, o.droppers) {
            for r in readers(&conc, &cyc_set) {
                let mut blocks = conc.clone();
                blocks.extend(r);
                let mut sets = vec![blocks];
                if o.comparators >= 2 {
                    let mut more = Vec::new();
                    for b in &sets {
                        for r2 in readers(b, &cyc_set) {
                            let mut v = b.clone();
                            v.extend(r2);
                            more.push(v);
                        }
                    }
                    sets.extend(more);
                }
                for mut b in sets {
                    if !add_floors(&mut b) {
                        continue;
                    }
                    let key = census::type_key(&Spec { blocks: b.clone(), nbt: Vec::new() }, false);
                    if core_keys.insert(key) {
                        cores.push((b, cyc.iter().map(|c| c.pos).collect()));
                    }
                }
            }
        }
    }
    if progress {
        eprintln!("ccensus: {} cycles, {} cores (cycle + comparator + floors, up to rotation)", cycles.len(), cores.len());
    }
    let copts = Opts { size: [0; 3], k_min: 0, k_max: 0, palette: Vec::new(), contents: Vec::new(), max_ticks: o.max_ticks, jobs: o.jobs, any_family: false };
    let next = AtomicUsize::new(0);
    let tried = AtomicUsize::new(0);
    let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
    let seen: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let start = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..o.jobs.max(1) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cores.len() {
                        break;
                    }
                    let (core, cyc) = &cores[i];
                    extras(o, &copts, core, cyc, 0, &tried, &found, &seen);
                }
            });
        }
        if progress {
            sc.spawn(|| {
                let mut last = std::time::Instant::now();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    if next.load(Ordering::Relaxed) >= cores.len() {
                        break;
                    }
                    if last.elapsed().as_secs() >= 30 {
                        last = std::time::Instant::now();
                        eprintln!("  {}/{} cores, {} judged, {} clocks, {:.0} s", next.load(Ordering::Relaxed), cores.len(), tried.load(Ordering::Relaxed), found.lock().unwrap().len(), start.elapsed().as_secs_f64());
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    if progress {
        eprintln!("ccensus: {} candidates judged, {} clock variants in {:.1} s", tried.load(Ordering::Relaxed), found.len(), start.elapsed().as_secs_f64());
    }
    found
}

/// Add up to `o.extra - depth` blocks next to the structure (cells after the last added one, to
/// enumerate each set once), judging every structure on the way with every contents choice.
#[allow(clippy::too_many_arguments)]
fn extras(o: &COpts, copts: &Opts, blocks: &[(BlockPos, Block)], cyc: &[BlockPos], depth: usize, tried: &AtomicUsize, found: &Mutex<Vec<Found>>, seen: &Mutex<HashSet<String>>) {
    judge_contents(o, copts, blocks, cyc, tried, found, seen);
    if depth >= o.extra {
        return;
    }
    let taken: HashSet<BlockPos> = blocks.iter().map(|(p, _)| *p).collect();
    let mut cand: Vec<BlockPos> = Vec::new();
    for (p, _) in blocks {
        for d in DIRS {
            let q = p.rel(d);
            if !taken.contains(&q) && !cand.contains(&q) {
                cand.push(q);
            }
        }
    }
    cand.sort();
    // only cells after the extras already added (the core's own cells are not ordered)
    let last_extra = if depth > 0 { Some(blocks[blocks.len() - 1].0) } else { None };
    let at: HashMap<BlockPos, Block> = blocks.iter().copied().collect();
    for q in cand {
        if last_extra.is_some_and(|l| q <= l) {
            continue;
        }
        for b in &o.extra_palette {
            if let Some(need) = census::needs_floor(b) {
                match at.get(&q.below()) {
                    Some(f) if census::floor_ok(f, need, b) => {}
                    _ => continue,
                }
            }
            let mut v = blocks.to_vec();
            v.push((q, *b));
            if !census::wall_torches_ok(&v) {
                continue;
            }
            extras(o, copts, &v, cyc, depth + 1, tried, found, seen);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn judge_contents(o: &COpts, copts: &Opts, blocks: &[(BlockPos, Block)], cyc: &[BlockPos], tried: &AtomicUsize, found: &Mutex<Vec<Found>>, seen: &Mutex<HashSet<String>>) {
    // every choice of 1..=max_filled cycle containers, each holding one of the contents options
    let mut fills: Vec<Vec<(BlockPos, String)>> = Vec::new();
    fn rec(o: &COpts, cyc: &[BlockPos], start: usize, cur: &mut Vec<(BlockPos, String)>, out: &mut Vec<Vec<(BlockPos, String)>>) {
        if !cur.is_empty() {
            out.push(cur.clone());
        }
        if cur.len() >= o.max_filled {
            return;
        }
        for i in start..cyc.len() {
            for c in &o.contents {
                cur.push((cyc[i], c.clone()));
                rec(o, cyc, i + 1, cur, out);
                cur.pop();
            }
        }
    }
    rec(o, cyc, 0, &mut Vec::new(), &mut fills);
    for nbt in fills {
        let spec = Spec { blocks: blocks.to_vec(), nbt };
        tried.fetch_add(1, Ordering::Relaxed);
        if let Some(f) = census::judge_candidate(&spec, copts) {
            let key = census::type_key(&f.spec, true);
            if seen.lock().unwrap().contains(&key) {
                continue;
            }
            if census::irreducible(&f, copts) {
                seen.lock().unwrap().insert(key);
                found.lock().unwrap().push(f);
            }
        }
    }
}
