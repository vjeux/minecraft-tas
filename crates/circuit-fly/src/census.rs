//! `mctas circuit fly census`: the exhaustive search for small flying machines on redsim. Every
//! face-connected filling of an AxBxC box with kmin..=k blocks of the palette (default: slime,
//! honey, stone and every facing of observers, pistons and sticky pistons), one per class up to
//! translation (it touches the box's low faces) and the 4 rotations (the smallest encoding among
//! the rotations that fit the box), with at least one piston and one observer and every observer
//! pointing its output into a piston or a conductor (else it powers nothing). Each candidate is
//! kicked once per observer (a 1 gt stone in its watched cell, or that cell's own block placed
//! last), run in a world with `margin` free cells around the box, and tracked every tick (fast
//! Block-level copy of [`crate::track`]). A machine is FOUND when it moves steadily (frame n + P =
//! frame n shifted by d != 0 for 2+ periods) and its own design shape recurs at 2+ different
//! places (it stays intact). Found machines go to `DIR/found.tsv` and one design per type (up to
//! rotation and mirror image) to `DIR/designs/`; `circuit fly sim` / `verify` confirm them.

use crate::track::steady_of;
use anyhow::{Result, bail};
use clock::Pos;
use redsim::world::flags;
use redsim::{Block, BlockPos, Dir, World};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug)]
pub struct CensusOptions {
    pub dims: [i32; 3],
    pub k: usize,
    pub kmin: usize,
    pub palette: Vec<String>,
    pub max_pistons: usize,
    pub max_observers: usize,
    pub horizon: u32,
    pub margin: i32,
    pub workers: usize,
    pub out: PathBuf,
    /// Keep observers whose output feeds nothing (spacers).
    pub loose: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Piston,
    Observer(Dir),
    Other,
}

struct Palette {
    blocks: Vec<Block>,
    names: Vec<String>,
    roles: Vec<Role>,
    /// Index of the state turned a quarter turn clockwise.
    turn: Vec<usize>,
    /// The state conducts (an observer's output into it powers the pistons next to it) or is a
    /// piston itself.
    feeds: Vec<bool>,
}

fn turn_block(b: Block) -> Option<Block> {
    Some(match b {
        Block::Observer { facing, powered } => Block::Observer { facing: facing.clockwise(), powered },
        Block::Piston { sticky, facing, extended } => Block::Piston { sticky, facing: facing.clockwise(), extended },
        Block::Glazed { facing } => Block::Glazed { facing: facing.clockwise() },
        Block::Simple(_) | Block::RedstoneBlock => b,
        _ => return None,
    })
}

impl Palette {
    fn new(names: &[String]) -> Result<Palette> {
        let mut blocks = Vec::new();
        for n in names {
            let b = Block::parse(n).map_err(|e| anyhow::anyhow!("{e}"))?;
            if turn_block(b).is_none() {
                bail!("census palette: {n} is not supported (observers, pistons, plain blocks, glazed terracotta, redstone block)");
            }
            if !blocks.contains(&b) {
                blocks.push(b);
            }
        }
        let names: Vec<String> = blocks.iter().map(|b| b.to_string()).collect();
        let roles = blocks
            .iter()
            .map(|b| match b {
                Block::Piston { .. } => Role::Piston,
                Block::Observer { facing, .. } => Role::Observer(*facing),
                _ => Role::Other,
            })
            .collect();
        let mut turn = Vec::new();
        for b in &blocks {
            let t = turn_block(*b).unwrap();
            match blocks.iter().position(|x| *x == t) {
                Some(i) => turn.push(i),
                None => bail!("census palette: {b} turned ({t}) is not in the palette (give every facing)"),
            }
        }
        let feeds = blocks.iter().map(|b| matches!(b, Block::Piston { .. }) || matches!(b, Block::Simple(s) if s.conductor()) || matches!(b, Block::Glazed { .. })).collect();
        Ok(Palette { blocks, names, roles, turn, feeds })
    }
}

/// A found machine.
#[derive(Clone, Debug)]
pub struct Found {
    /// The cells as written in the design (the first rotation that flies), touching the origin.
    pub cells: Vec<(Pos, usize)>,
    pub kick: Kick,
    pub period: u32,
    pub d: Pos,
    pub first_change: u32,
    /// Which quarter turns of `cells` fly (bit r = turned r times clockwise; bit 0 always set).
    pub mask: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kick {
    /// The block in this cell placed last.
    Last(Pos),
    /// A stone in this (air) cell for 1 gt.
    Temp(Pos),
}

/// 128-bit two-lane hasher (every steady state found here is re-checked with exact frames by
/// `circuit fly sim`).
struct H2 {
    a: u64,
    b: u64,
}
impl Hasher for H2 {
    fn finish(&self) -> u64 {
        self.a
    }
    fn write(&mut self, bytes: &[u8]) {
        for &x in bytes {
            self.write_u64(x as u64);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.a = (self.a ^ x).wrapping_mul(0x100000001b3).rotate_left(29);
        self.b = (self.b ^ x.rotate_left(17)).wrapping_mul(0x9e3779b97f4a7c15).rotate_left(31);
    }
    fn write_u8(&mut self, x: u8) {
        self.write_u64(x as u64 | 0x100)
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64 | 0x2_0000_0000)
    }
    fn write_i32(&mut self, x: i32) {
        self.write_u32(x as u32)
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64 ^ 0x7777_0000_0000_0000)
    }
    fn write_isize(&mut self, x: isize) {
        self.write_u64(x as u64 ^ 0x3333_0000_0000_0000)
    }
}

/// The structure of a block (dynamic properties dropped); None for a piston head.
fn rest(b: Block) -> Option<Block> {
    match b {
        Block::PistonHead { .. } => None,
        Block::Piston { sticky, facing, .. } => Some(Block::Piston { sticky, facing, extended: false }),
        Block::Observer { facing, .. } => Some(Block::Observer { facing, powered: false }),
        other => Some(other),
    }
}

/// Run one candidate with one kick (cells touching the origin's three low faces); Some when it
/// flies.
fn run(cells: &[(Pos, Block)], kick: Kick, o: &CensusOptions) -> Option<(u32, Pos, u32)> {
    let m = o.margin;
    let dims = [0, 1, 2].map(|i| cells.iter().map(|(p, _)| p[i]).max().unwrap_or(0) + 1);
    let min = BlockPos::new(-m, -m, -m);
    let size = [dims[0] + 2 * m, dims[1] + 2 * m, dims[2] + 2 * m];
    let mut w = World::new(min, size);
    let withheld = match kick {
        Kick::Last(p) => Some(p),
        Kick::Temp(_) => None,
    };
    let kept: Vec<(BlockPos, Block)> = cells.iter().filter(|(p, _)| Some(*p) != withheld).map(|(p, b)| (BlockPos::new(p[0], p[1], p[2]), *b)).collect();
    w.place_raw(&kept);
    w.place_updates(&kept);
    // the design's structure, in scan order (y, z, x), relative to its corner
    let mut reference: Vec<(Pos, Block)> = cells.iter().filter_map(|(p, b)| rest(*b).map(|r| (*p, r))).collect();
    reference.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
    let rc = corner(reference.iter().map(|(p, _)| *p));
    for (p, _) in reference.iter_mut() {
        *p = [p[0] - rc[0], p[1] - rc[1], p[2] - rc[2]];
    }
    match kick {
        Kick::Last(p) => {
            let b = cells.iter().find(|(q, _)| *q == p).unwrap().1;
            w.set_block(BlockPos::new(p[0], p[1], p[2]), b, flags::ALL);
        }
        Kick::Temp(p) => {
            w.set_block(BlockPos::new(p[0], p[1], p[2]), Block::Simple(redsim::block::Simple::Stone), flags::ALL);
        }
    }
    let wlo = [-m, -m, -m];
    let whi = [dims[0] + m - 1, dims[1] + m - 1, dims[2] + m - 1];
    let mut lo = [0, 0, 0];
    let mut hi = [dims[0] - 1, dims[1] - 1, dims[2] - 1];
    let mut canon: Vec<u64> = Vec::with_capacity(o.horizon as usize + 1);
    let mut corners: Vec<Pos> = Vec::with_capacity(o.horizon as usize + 1);
    let mut shifts: Vec<Option<Pos>> = Vec::with_capacity(o.horizon as usize + 1);
    let mut frame: Vec<(Pos, Block)> = Vec::new();
    let mut shape: Vec<(Pos, Block)> = Vec::new();
    let mut edge = false;
    for t in 0..=o.horizon {
        if t == 1
            && let Kick::Temp(p) = kick
        {
            w.set_block(BlockPos::new(p[0], p[1], p[2]), Block::Air, flags::ALL);
        }
        // scan the window around the last box
        frame.clear();
        let (a, b) = ([0, 1, 2].map(|i| (lo[i] - 2).max(wlo[i])), [0, 1, 2].map(|i| (hi[i] + 2).min(whi[i])));
        for y in a[1]..=b[1] {
            for z in a[2]..=b[2] {
                for x in a[0]..=b[0] {
                    let s = w.get(BlockPos::new(x, y, z));
                    if s.is_air() {
                        continue;
                    }
                    if t == 0
                        && let Kick::Temp(p) = kick
                        && p == [x, y, z]
                    {
                        continue;
                    }
                    frame.push(([x, y, z], s));
                }
            }
        }
        if frame.is_empty() {
            return None;
        }
        lo = corner(frame.iter().map(|(p, _)| *p));
        hi = [0, 1, 2].map(|i| frame.iter().map(|(p, _)| p[i]).max().unwrap());
        if (0..3).any(|i| lo[i] <= wlo[i] || hi[i] >= whi[i]) {
            edge = true;
        }
        let mut h = H2 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
        let mut moving = false;
        for (p, s) in &frame {
            [p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]].hash(&mut h);
            s.hash(&mut h);
            moving |= matches!(s, Block::MovingPiston { .. });
        }
        canon.push(h.a ^ h.b.rotate_left(13));
        corners.push(lo);
        let shift = if moving {
            None
        } else {
            shape.clear();
            shape.extend(frame.iter().filter_map(|(p, s)| rest(*s).map(|r| (*p, r))));
            if shape.len() != reference.len() {
                None
            } else {
                let c = corner(shape.iter().map(|(p, _)| *p));
                if shape.iter().zip(&reference).all(|((p, s), (q, r))| s == r && [p[0] - c[0], p[1] - c[1], p[2] - c[2]] == *q) { Some([c[0] - rc[0], c[1] - rc[1], c[2] - rc[2]]) } else { None }
            }
        };
        shifts.push(shift);
        if !w.drops.is_empty() {
            return None;
        }
        if !w.errors.is_empty() || edge {
            break;
        }
        if t >= 1 && w.idle() {
            return None;
        }
        w.tick();
    }
    let st = steady_of(&canon, &corners, 1)?;
    if st.cells() == 0 || st.periods < 2 {
        return None;
    }
    let mut places: Vec<Pos> = shifts.iter().flatten().copied().collect();
    places.sort();
    places.dedup();
    if places.iter().filter(|s| **s != [0, 0, 0]).count() < 2 {
        return None;
    }
    let first = (1..canon.len()).find(|t| canon[*t] != canon[0] || corners[*t] != corners[0]).unwrap_or(0) as u32;
    Some((st.period, st.d, first))
}

fn corner(ps: impl Iterator<Item = Pos>) -> Pos {
    let mut lo = [i32::MAX; 3];
    for p in ps {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
        }
    }
    lo
}

fn cell_index(p: Pos, dims: [i32; 3]) -> usize {
    ((p[1] * dims[2] + p[2]) * dims[0] + p[0]) as usize
}

/// Is this assignment the smallest encoding among its rotations that fit the box?
fn canonical(cells: &[(Pos, usize)], pal: &Palette, dims: [i32; 3], buf: &mut Vec<(usize, usize)>, enc0: &mut Vec<(usize, usize)>) -> bool {
    enc0.clear();
    enc0.extend(cells.iter().map(|(p, s)| (cell_index(*p, dims), *s)));
    enc0.sort();
    for t in 1..4u8 {
        let rot: Vec<(Pos, usize)> = cells
            .iter()
            .map(|(p, s)| {
                let mut st = *s;
                for _ in 0..t {
                    st = pal.turn[st];
                }
                (clock::rotate_pos(*p, t), st)
            })
            .collect();
        let c = corner(rot.iter().map(|(p, _)| *p));
        let fits = rot.iter().all(|(p, _)| (0..3).all(|i| p[i] - c[i] < dims[i]));
        if !fits {
            continue;
        }
        buf.clear();
        buf.extend(rot.iter().map(|(p, s)| (cell_index([p[0] - c[0], p[1] - c[1], p[2] - c[2]], dims), *s)));
        buf.sort();
        if *buf < *enc0 {
            return false;
        }
    }
    true
}

/// Face-connected subsets of the box with kmin..=k cells touching the three low faces.
fn subsets(dims: [i32; 3], kmin: usize, k: usize) -> Vec<Vec<Pos>> {
    let all: Vec<Pos> = (0..dims[1]).flat_map(|y| (0..dims[2]).flat_map(move |z| (0..dims[0]).map(move |x| [x, y, z]))).collect();
    let n = all.len();
    if n > 24 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for mask in 1u32..(1u32 << n) {
        let c = mask.count_ones() as usize;
        if c < kmin || c > k {
            continue;
        }
        let cells: Vec<Pos> = (0..n).filter(|i| mask >> i & 1 == 1).map(|i| all[i]).collect();
        if (0..3).any(|i| cells.iter().all(|p| p[i] > 0)) {
            continue;
        }
        // connected
        let mut seen = vec![false; cells.len()];
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(i) = stack.pop() {
            for (j, q) in cells.iter().enumerate() {
                if !seen[j] && (0..3).map(|a| (cells[i][a] - q[a]).abs()).sum::<i32>() == 1 {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if seen.iter().all(|s| *s) {
            out.push(cells);
        }
    }
    out
}

/// The census.
pub fn census(o: &CensusOptions) -> Result<Vec<Found>> {
    let pal = Palette::new(&o.palette)?;
    let subs = subsets(o.dims, o.kmin, o.k);
    // chunks: (subset, state of its first cell)
    let chunks: Vec<(usize, usize)> = (0..subs.len()).flat_map(|s| (0..pal.blocks.len()).map(move |f| (s, f))).collect();
    eprintln!("census {}x{}x{} k {}..={}: {} cell sets, {} palette states, {} chunks, {} workers", o.dims[0], o.dims[1], o.dims[2], o.kmin, o.k, subs.len(), pal.blocks.len(), chunks.len(), o.workers);
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let sims = AtomicUsize::new(0);
    let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
    let start = std::time::Instant::now();
    std::thread::scope(|s| {
        for _ in 0..o.workers.max(1) {
            s.spawn(|| {
                let mut buf = Vec::new();
                let mut enc0 = Vec::new();
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= chunks.len() {
                        break;
                    }
                    let (si, first) = chunks[i];
                    let cells = &subs[si];
                    let mut assign: Vec<usize> = vec![0; cells.len()];
                    assign[0] = first;
                    let mut local = 0usize;
                    let mut hits: Vec<Found> = Vec::new();
                    dfs(1, cells, &mut assign, &pal, o, &mut buf, &mut enc0, &mut local, &mut hits);
                    sims.fetch_add(local, Ordering::Relaxed);
                    if !hits.is_empty() {
                        found.lock().unwrap().extend(hits);
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d % 200 == 0 || d == chunks.len() {
                        eprintln!("  {d}/{} chunks, {} sims, {} found, {:.0} s", chunks.len(), sims.load(Ordering::Relaxed), found.lock().unwrap().len(), start.elapsed().as_secs_f64());
                    }
                }
            });
        }
    });
    let mut v = found.into_inner().unwrap();
    v.sort_by_key(|f| (f.cells.len(), f.period * 1000 / f.d.iter().map(|c| c.abs()).sum::<i32>().max(1) as u32, f.cells.clone()));
    Ok(v)
}

#[allow(clippy::too_many_arguments)]
fn dfs(i: usize, cells: &[Pos], assign: &mut Vec<usize>, pal: &Palette, o: &CensusOptions, buf: &mut Vec<(usize, usize)>, enc0: &mut Vec<(usize, usize)>, sims: &mut usize, hits: &mut Vec<Found>) {
    let count = |assign: &[usize], n: usize, r: fn(Role) -> bool| assign[..n].iter().filter(|s| r(pal.roles[**s])).count();
    let is_p = |r: Role| r == Role::Piston;
    let is_o = |r: Role| matches!(r, Role::Observer(_));
    if count(assign, i, is_p) > o.max_pistons || count(assign, i, is_o) > o.max_observers {
        return;
    }
    if i < cells.len() {
        for s in 0..pal.blocks.len() {
            assign[i] = s;
            dfs(i + 1, cells, assign, pal, o, buf, enc0, sims, hits);
        }
        return;
    }
    if count(assign, i, is_p) == 0 || count(assign, i, is_o) == 0 {
        return;
    }
    let at = |p: Pos| cells.iter().position(|q| *q == p);
    // every observer feeds a piston or a conductor
    if !o.loose {
        for (c, s) in cells.iter().zip(assign.iter()) {
            if let Role::Observer(f) = pal.roles[*s] {
                let st = f.step();
                let back = [c[0] - st[0], c[1] - st[1], c[2] - st[2]];
                match at(back) {
                    Some(j) if pal.feeds[assign[j]] => {}
                    _ => return,
                }
            }
        }
    }
    let cs: Vec<(Pos, usize)> = cells.iter().copied().zip(assign.iter().copied()).collect();
    if !canonical(&cs, pal, o.dims, buf, enc0) {
        return;
    }
    let mut kicks: Vec<Kick> = Vec::new();
    for (c, s) in &cs {
        if let Role::Observer(f) = pal.roles[*s] {
            let st = f.step();
            let q = [c[0] + st[0], c[1] + st[1], c[2] + st[2]];
            let k = if at(q).is_some() { Kick::Last(q) } else { Kick::Temp(q) };
            if !kicks.contains(&k) {
                kicks.push(k);
            }
        }
    }
    // every quarter turn: Java's update order (W, E, D, U, N, S) makes some machines fly in one
    // orientation only
    let turned: Vec<(Vec<(Pos, usize)>, Pos)> = (0..4u8)
        .map(|t| {
            let rot: Vec<(Pos, usize)> = cs
                .iter()
                .map(|(p, s)| {
                    let mut st = *s;
                    for _ in 0..t {
                        st = pal.turn[st];
                    }
                    (clock::rotate_pos(*p, t), st)
                })
                .collect();
            let c = corner(rot.iter().map(|(p, _)| *p));
            (rot.into_iter().map(|(p, s)| ([p[0] - c[0], p[1] - c[1], p[2] - c[2]], s)).collect(), c)
        })
        .collect();
    for k in kicks {
        let mut hit: Option<(u8, u32, Pos, u32)> = None;
        let mut mask = 0u8;
        for (t, (rot, c)) in turned.iter().enumerate() {
            let blocks: Vec<(Pos, Block)> = rot.iter().map(|(p, s)| (*p, pal.blocks[*s])).collect();
            let kp = |p: Pos| {
                let q = clock::rotate_pos(p, t as u8);
                [q[0] - c[0], q[1] - c[1], q[2] - c[2]]
            };
            let kk = match k {
                Kick::Last(p) => Kick::Last(kp(p)),
                Kick::Temp(p) => Kick::Temp(kp(p)),
            };
            *sims += 1;
            if let Some((period, d, first)) = run(&blocks, kk, o) {
                mask |= 1 << t;
                if hit.is_none() {
                    hit = Some((t as u8, period, d, first));
                }
            }
        }
        if let Some((t, period, d, first)) = hit {
            let (rot, c) = &turned[t as usize];
            let kp = |p: Pos| {
                let q = clock::rotate_pos(p, t);
                [q[0] - c[0], q[1] - c[1], q[2] - c[2]]
            };
            let kick = match k {
                Kick::Last(p) => Kick::Last(kp(p)),
                Kick::Temp(p) => Kick::Temp(kp(p)),
            };
            // the mask relative to the written rotation t
            let rel = (0..4u8).filter(|r| mask >> ((t + r) % 4) & 1 == 1).fold(0u8, |a, r| a | 1 << r);
            hits.push(Found { cells: rot.clone(), kick, period, d, first_change: first, mask: rel });
            break;
        }
    }
}

/// A found machine as a design (the circuit format), named after its blocks and speed.
pub fn design_text(f: &Found, pal_names: &[String], name: &str) -> String {
    let mut lo = corner(f.cells.iter().map(|(p, _)| *p));
    let mut hi = [0, 1, 2].map(|i| f.cells.iter().map(|(p, _)| p[i]).max().unwrap());
    let kick_pos = match f.kick {
        Kick::Last(p) | Kick::Temp(p) => p,
    };
    for i in 0..3 {
        lo[i] = lo[i].min(kick_pos[i]);
        hi[i] = hi[i].max(kick_pos[i]);
    }
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".chars().collect();
    let mut used: BTreeMap<usize, char> = BTreeMap::new();
    for (_, s) in &f.cells {
        let n = used.len();
        used.entry(*s).or_insert(chars[n]);
    }
    let n = f.d.iter().map(|c| c.abs()).sum::<i32>().max(1);
    let dir = f.d.map(|c| c / n);
    let mut t = String::new();
    t.push_str(&format!("# census: {} blocks, {} gt per {} cell(s), first move t{}, flies turned {} (redsim)\n", f.cells.len(), f.period, n, f.first_change, mask_text(f.mask)));
    t.push_str(&format!("name: {name}\ncircuit: flying-machine\ntravel: {} {} {}\ndistance: 24\n", dir[0], dir[1], dir[2]));
    t.push_str(&format!("origin: {} {} {}\n", lo[0], lo[1], lo[2]));
    match f.kick {
        Kick::Last(p) => t.push_str(&format!("port start: {} {} {} last\n", p[0], p[1], p[2])),
        Kick::Temp(p) => t.push_str(&format!("port start: {} {} {} minecraft:stone 1\n", p[0], p[1], p[2])),
    }
    t.push_str("palette:\n");
    for (s, c) in &used {
        t.push_str(&format!("  {c} {}\n", pal_names[*s]));
    }
    let map: HashMap<Pos, usize> = f.cells.iter().copied().collect();
    for y in lo[1]..=hi[1] {
        t.push_str(&format!("layer {}:\n", y - lo[1]));
        for z in lo[2]..=hi[2] {
            let row: String = (lo[0]..=hi[0]).map(|x| map.get(&[x, y, z]).map(|s| used[s]).unwrap_or('.')).collect();
            t.push_str(&format!("  {row}\n"));
        }
    }
    t
}

/// `4/4` (robust) or the quarter turns that fly, `0,2`.
pub fn mask_text(mask: u8) -> String {
    if mask == 15 { "4/4".into() } else { (0..4).filter(|r| mask >> r & 1 == 1).map(|r| r.to_string()).collect::<Vec<_>>().join(",") }
}

/// The census command over several boxes: run, write found.tsv and one design per type.
pub fn run_census(o: &CensusOptions, boxes: &[[i32; 3]]) -> Result<()> {
    let pal = Palette::new(&o.palette)?;
    let mut found: Vec<([i32; 3], Found)> = Vec::new();
    for b in boxes {
        let ob = CensusOptions { dims: *b, ..o.clone() };
        found.extend(census(&ob)?.into_iter().map(|f| (*b, f)));
    }
    found.sort_by_key(|(_, f)| (f.cells.len(), f.period * 1000 / f.d.iter().map(|c| c.abs()).sum::<i32>().max(1) as u32, std::cmp::Reverse(f.mask.count_ones())));
    std::fs::create_dir_all(o.out.join("designs"))?;
    let mut tsv = String::from("blocks\tspeed\tperiod\tcells\td\tfirst_move\trotations\tkick\tbox\ttype\tdesign\n");
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut new_types = 0;
    for (b, f) in &found {
        let strings: Vec<(Pos, String)> = f.cells.iter().map(|(p, s)| (*p, pal.names[*s].clone())).collect();
        let key = clock::canon::type_key(strings.iter().map(|(p, s)| (p, s)));
        let n = f.d.iter().map(|c| c.abs()).sum::<i32>().max(1);
        let speed = if f.period % n as u32 == 0 { format!("{}", f.period / n as u32) } else { format!("{}/{}", f.period, n) };
        let file = match seen.get(&key) {
            Some(f) => f.clone(),
            None => {
                new_types += 1;
                let h = {
                    let mut hh = H2 { a: 1, b: 2 };
                    key.hash(&mut hh);
                    hh.a
                };
                let name = format!("fly-c{}-s{}-{:06x}", f.cells.len(), speed.replace('/', "_"), h & 0xffffff);
                let path = o.out.join("designs").join(format!("{name}.txt"));
                std::fs::write(&path, design_text(f, &pal.names, &name))?;
                seen.insert(key.clone(), name.clone());
                name
            }
        };
        let kick = match f.kick {
            Kick::Last(p) => format!("last {},{},{}", p[0], p[1], p[2]),
            Kick::Temp(p) => format!("stone {},{},{}", p[0], p[1], p[2]),
        };
        tsv.push_str(&format!(
            "{}\t{speed}\t{}\t{n}\t{},{},{}\t{}\t{}\t{kick}\t{}x{}x{}\t{key}\t{file}\n",
            f.cells.len(),
            f.period,
            f.d[0],
            f.d[1],
            f.d[2],
            f.first_change,
            mask_text(f.mask),
            b[0],
            b[1],
            b[2]
        ));
    }
    std::fs::write(o.out.join("found.tsv"), tsv)?;
    println!("census: {} machines found, {} types, written to {}", found.len(), new_types, o.out.display());
    Ok(())
}
