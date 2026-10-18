//! `mctas door fullmech`: mechanism search on the EXACT model (track pistmodel).
//!
//! The macro model of `door families` / `door mech-synth` moves blocks one settled piston action
//! at a time. Here every transition is the real tick-level game rule set (`redsim`: pistons,
//! moving blocks, spitting, 0-ticks, the block-event order, quasi-connectivity and BUD, moved
//! redstone blocks and observers powering what they land next to, pistons carried by pistons),
//! checked against the game by `door pistfuzz`.
//!
//! A mechanism = pieces in the door plane (door coordinates: the 3x3 door is x 0..2, y 0..2;
//! everything stays in the plane z = 0) inside a box. Every piston has an ideal INPUT (the
//! wiring to come), tracked by identity: a piston carried by another keeps its input. A state is
//! a settled world (nothing pending) plus the input levels. Actions: `L` toggles an input, `P k`
//! is a k-tick pulse (k = 1, 2 spit what was pushed: the block-event order decides; k >= 3
//! pushes and pulls back). After an action the world is ticked until idle; a transition that
//! drops an item, writes outside the box or never settles is refused.
//!
//! Open = the 3x3 empty, only air or plain blocks in the 12 ring cells, every ring cell swept
//! during the cycle but empty when open backed by air or a door block (FULL seamless, as in
//! `families`). Close = back to the exact rest state (blocks and inputs).

use crate::families::{self, Bx, K, Pc};
use anyhow::{Context, Result};
use redsim::block::Simple;
use redsim::{Block, BlockPos, Dir, World};
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Piece kinds: families' plus the helpers only the exact model understands.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum FK {
    Stone,
    Obsidian,
    /// f: 0 east, 1 west, 2 up, 3 down (families' codes).
    Piston { sticky: bool, f: u8, ext: bool },
    RedstoneBlock,
    Observer { f: u8 },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct FPc {
    pub x: i8,
    pub y: i8,
    pub k: FK,
}

const F: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const FNAME: [&str; 4] = ["E", "W", "U", "D"];

fn fdir(f: u8) -> Dir {
    [Dir::East, Dir::West, Dir::Up, Dir::Down][f as usize]
}

impl FPc {
    pub fn from_pc(p: &Pc) -> FPc {
        let k = match p.k {
            K::Stone => FK::Stone,
            K::Obsidian => FK::Obsidian,
            K::Piston { sticky, f, ext } => FK::Piston { sticky, f, ext },
        };
        FPc { x: p.x, y: p.y, k }
    }
    pub fn code(&self) -> String {
        match self.k {
            FK::Stone => format!("stone({},{})", self.x, self.y),
            FK::Obsidian => format!("obsidian({},{})", self.x, self.y),
            FK::RedstoneBlock => format!("rb({},{})", self.x, self.y),
            FK::Observer { f } => format!("obs{}({},{})", FNAME[f as usize], self.x, self.y),
            FK::Piston { sticky, f, ext } => format!("{}{}{}({},{})", if sticky { "S" } else { "N" }, FNAME[f as usize], if ext { "+" } else { "" }, self.x, self.y),
        }
    }
    fn cells(&self) -> Vec<(i8, i8)> {
        match self.k {
            FK::Piston { ext: true, f, .. } => vec![(self.x, self.y), (self.x + F[f as usize].0, self.y + F[f as usize].1)],
            _ => vec![(self.x, self.y)],
        }
    }
}

/// World coordinates of a door-plane cell.
const OY: i32 = 100;
fn wp(x: i8, y: i8) -> BlockPos {
    BlockPos::new(x as i32, OY + y as i32, 0)
}

fn in_t(x: i8, y: i8) -> bool {
    (0..=2).contains(&x) && (0..=2).contains(&y)
}

/// The 12 ring cells and the cell right behind each (outward).
fn ring() -> Vec<((i8, i8), (i8, i8))> {
    let mut v = Vec::new();
    for i in 0..3 {
        v.push(((-1, i), (-2, i)));
        v.push(((3, i), (4, i)));
        v.push(((i, -1), (i, -2)));
        v.push(((i, 3), (i, 4)));
    }
    v
}

/// A settled state: the box's blocks (row-major), input bits, tracked piston cells.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Key {
    pub g: Vec<Block>,
    pub inp: u32,
    pub pos: Vec<BlockPos>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum A {
    L(u8),
    P(u8, u8),
    /// Two overlapping pulses: input a for `ka` ticks from tick 0, input b for `kb` ticks from
    /// tick `d` (d = 0: switched right after a, before the same tick): same-tick and
    /// next-tick interactions (one block pushed by two pistons in one tick, a spit while
    /// another piston moves, killer-piston orders).
    S { a: u8, b: u8, ka: u8, kb: u8, d: u8 },
}

pub struct Ctx {
    pub bx: Bx,
    pub pieces: Vec<FPc>,
    min: BlockPos,
    size: [i32; 3],
    /// Index of each door cell (x, y) in the 3x3 (None outside the box).
    door: Vec<Option<usize>>,
    ring: Vec<(Option<usize>, Option<usize>)>,
    pub n_pistons: usize,
    pub actions: Vec<A>,
    pub max_ticks: u32,
}

const STONE: Block = Block::Simple(Simple::Stone);

impl Ctx {
    fn idx(&self, x: i8, y: i8) -> Option<usize> {
        let bx = self.bx;
        if x < bx.x0 || y < bx.y0 || x >= bx.x0 + bx.w || y >= bx.y0 + bx.h {
            return None;
        }
        Some(((y - bx.y0) as usize) * bx.w as usize + (x - bx.x0) as usize)
    }

    fn world_of(&self, key: &Key) -> World {
        let mut w = World::new(self.min, self.size);
        let bw = self.bx.w as usize;
        for (i, b) in key.g.iter().enumerate() {
            if !b.is_air() {
                let x = self.bx.x0 + (i % bw) as i8;
                let y = self.bx.y0 + (i / bw) as i8;
                w.put(wp(x, y), *b);
            }
        }
        w.piston.tracked = key.pos.iter().enumerate().map(|(i, p)| (*p, key.inp >> i & 1 == 1)).collect();
        w
    }

    fn key_of(&self, w: &World) -> Key {
        let bw = self.bx.w as usize;
        let n = bw * self.bx.h as usize;
        let mut g = Vec::with_capacity(n);
        for i in 0..n {
            let x = self.bx.x0 + (i % bw) as i8;
            let y = self.bx.y0 + (i / bw) as i8;
            g.push(w.get(wp(x, y)));
        }
        let mut inp = 0u32;
        let mut pos = Vec::with_capacity(w.piston.tracked.len());
        for (i, (p, on)) in w.piston.tracked.iter().enumerate() {
            if *on {
                inp |= 1 << i;
            }
            pos.push(*p);
        }
        Key { g, inp, pos }
    }

    fn ok(&self, w: &World) -> bool {
        w.drops.is_empty() && w.errors.is_empty()
    }

    /// Ring cells that are not air now (bit per ring index).
    fn ring_mask(&self, w: &World) -> u16 {
        let mut m = 0u16;
        for (i, (c, _)) in ring().iter().enumerate() {
            if !w.get(wp(c.0, c.1)).is_air() {
                m |= 1 << i;
            }
        }
        m
    }

    /// Run the world until idle; None when it misbehaves.
    fn settle(&self, w: &mut World, swept: &mut u16) -> Option<()> {
        for _ in 0..self.max_ticks {
            if w.idle() {
                return Some(());
            }
            w.tick();
            if !self.ok(w) {
                return None;
            }
            *swept |= self.ring_mask(w);
        }
        if w.idle() { Some(()) } else { None }
    }

    /// One action from a settled state: the next settled state and the ring cells swept.
    pub fn step(&self, key: &Key, a: A) -> Option<(Key, u16)> {
        let mut w = self.world_of(key);
        let mut swept = 0u16;
        match a {
            A::L(i) => {
                let on = key.inp >> i & 1 == 0;
                redsim::piston::set_input(&mut w, i as usize, on);
            }
            A::P(i, k) => {
                if key.inp >> i & 1 == 1 {
                    return None;
                }
                redsim::piston::set_input(&mut w, i as usize, true);
                for _ in 0..k {
                    w.tick();
                    if !self.ok(&w) {
                        return None;
                    }
                    swept |= self.ring_mask(&w);
                }
                redsim::piston::set_input(&mut w, i as usize, false);
            }
            A::S { a, b, ka, kb, d } => {
                if key.inp >> a & 1 == 1 || key.inp >> b & 1 == 1 {
                    return None;
                }
                let end = (ka as u32).max(d as u32 + kb as u32);
                for t in 0..=end {
                    // switches at this tick boundary: offs first, then ons (a before b)
                    if t == ka as u32 {
                        redsim::piston::set_input(&mut w, a as usize, false);
                    }
                    if t == d as u32 + kb as u32 {
                        redsim::piston::set_input(&mut w, b as usize, false);
                    }
                    if t == 0 {
                        redsim::piston::set_input(&mut w, a as usize, true);
                    }
                    if t == d as u32 {
                        redsim::piston::set_input(&mut w, b as usize, true);
                    }
                    if t == end {
                        break;
                    }
                    w.tick();
                    if !self.ok(&w) {
                        return None;
                    }
                    swept |= self.ring_mask(&w);
                }
            }
        }
        if !self.ok(&w) {
            return None;
        }
        self.settle(&mut w, &mut swept)?;
        let nk = self.key_of(&w);
        if nk == *key {
            return None;
        }
        Some((nk, swept))
    }

    pub fn is_open(&self, key: &Key) -> bool {
        for x in 0..3 {
            for y in 0..3 {
                if let Some(i) = self.door[(x * 3 + y) as usize] {
                    if !key.g[i].is_air() {
                        return false;
                    }
                }
            }
        }
        for (c, _) in &self.ring {
            if let Some(i) = c {
                if !matches!(key.g[*i], Block::Air | STONE) {
                    return false;
                }
            }
        }
        true
    }

    /// The hole rule: ring cells swept during the cycle but empty when open need air or a door
    /// block (stone) behind them.
    pub fn holes_ok(&self, open: &Key, swept: u16) -> bool {
        for (i, (c, b)) in self.ring.iter().enumerate() {
            let Some(ci) = c else { continue };
            if !open.g[*ci].is_air() || swept >> i & 1 == 0 {
                continue;
            }
            if let Some(bi) = b {
                if !matches!(open.g[*bi], Block::Air | STONE) {
                    return false;
                }
            }
        }
        true
    }

    /// Door blocks still in the 3x3 (the center counting 2).
    fn h_open(&self, key: &Key) -> u32 {
        let mut h = 0;
        for x in 0..3 {
            for y in 0..3 {
                if let Some(i) = self.door[(x * 3 + y) as usize] {
                    if !key.g[i].is_air() {
                        h += if x == 1 && y == 1 { 2 } else { 1 };
                    }
                }
            }
        }
        h
    }

    pub fn fmt_action(&self, a: A) -> String {
        let pid = |i: u8| {
            let p = self.pieces.iter().filter(|p| matches!(p.k, FK::Piston { .. })).nth(i as usize).unwrap();
            p.code()
        };
        match a {
            A::L(i) => format!("L{}", pid(i)),
            A::P(i, k) => format!("P{k}{}", pid(i)),
            A::S { a, b, ka, kb, d } => format!("[P{ka}{} & P{kb}{} +{d}]", pid(a), pid(b)),
        }
    }

    pub fn render(&self, key: &Key) -> String {
        let mut out = String::new();
        let bw = self.bx.w as usize;
        for row in (0..self.bx.h as usize).rev() {
            out.push_str(&format!("{:>3} ", self.bx.y0 + row as i8));
            for col in 0..bw {
                let x = self.bx.x0 + col as i8;
                let y = self.bx.y0 + row as i8;
                let b = key.g[row * bw + col];
                let ch = match b {
                    Block::Air => {
                        if in_t(x, y) {
                            ','
                        } else {
                            '.'
                        }
                    }
                    STONE => '#',
                    Block::Simple(Simple::Obsidian) => 'O',
                    Block::RedstoneBlock => 'R',
                    Block::Observer { .. } => 'o',
                    Block::Piston { sticky, facing, .. } => {
                        let c = match facing {
                            Dir::East => '>',
                            Dir::West => '<',
                            Dir::Up => '^',
                            _ => 'v',
                        };
                        if sticky { c } else { c.to_ascii_lowercase() }
                    }
                    Block::PistonHead { facing, .. } => {
                        if facing.is_horizontal() {
                            '='
                        } else {
                            '|'
                        }
                    }
                    _ => '?',
                };
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }
}

pub struct Opts {
    pub pulses: Vec<u8>,
    pub levels: bool,
    pub cap: usize,
    pub max_ticks: u32,
    /// Also overlapping pulse pairs of nearby pistons (same-tick / next-tick timing).
    pub pairs: bool,
}

/// Build a mechanism's rest state (placed like the judge, inputs on for pistons extended at rest,
/// settled); None when the pieces overlap or leave the box or the rest state does not hold.
pub fn build(bx: Bx, pieces: &[FPc], o: &Opts) -> Option<(Ctx, Key)> {
    build_driven(bx, pieces, o, &[])
}

/// [`build`] where the pistons listed in `undriven` get no input (only physical power: carried
/// or static redstone blocks and observers can fire them).
pub fn build_driven(bx: Bx, pieces: &[FPc], o: &Opts, undriven: &[FPc]) -> Option<(Ctx, Key)> {
    let mut pieces = pieces.to_vec();
    pieces.sort();
    let min = wp(bx.x0, bx.y0);
    let size = [bx.w as i32, bx.h as i32, 1];
    let mut planned: HashMap<(i8, i8), Block> = HashMap::new();
    for x in 0..3 {
        for y in 0..3 {
            planned.insert((x, y), STONE);
        }
    }
    let mut tracked = Vec::new();
    for p in &pieces {
        for c in p.cells() {
            if c.0 < bx.x0 || c.1 < bx.y0 || c.0 >= bx.x0 + bx.w || c.1 >= bx.y0 + bx.h {
                return None;
            }
            if planned.contains_key(&c) {
                return None;
            }
        }
        match p.k {
            FK::Stone => {
                planned.insert((p.x, p.y), STONE);
            }
            FK::Obsidian => {
                planned.insert((p.x, p.y), Block::Simple(Simple::Obsidian));
            }
            FK::RedstoneBlock => {
                planned.insert((p.x, p.y), Block::RedstoneBlock);
            }
            FK::Observer { f } => {
                planned.insert((p.x, p.y), Block::Observer { facing: fdir(f), powered: false });
            }
            FK::Piston { sticky, f, ext } => {
                planned.insert((p.x, p.y), Block::Piston { sticky, facing: fdir(f), extended: ext });
                if ext {
                    let (dx, dy) = F[f as usize];
                    planned.insert((p.x + dx, p.y + dy), Block::PistonHead { sticky, facing: fdir(f), short: false });
                }
                tracked.push((wp(p.x, p.y), ext && !undriven.contains(p)));
            }
        }
    }
    // door blocks outside the box (a row/column box) are not part of it
    planned.retain(|c, _| c.0 >= bx.x0 && c.1 >= bx.y0 && c.0 < bx.x0 + bx.w && c.1 < bx.y0 + bx.h);
    if tracked.len() > 31 {
        return None;
    }
    let mut actions = Vec::new();
    let piston_pieces: Vec<&FPc> = pieces.iter().filter(|p| matches!(p.k, FK::Piston { .. })).collect();
    for (i, (_, ext)) in tracked.iter().enumerate() {
        if undriven.contains(piston_pieces[i]) {
            continue;
        }
        let i = i as u8;
        // level toggles: every piston with --levels, else only pistons extended at rest (their
        // release and re-power), like families' pulse model
        if o.levels || *ext {
            actions.push(A::L(i));
        }
        for &k in &o.pulses {
            actions.push(A::P(i, k));
        }
    }
    if o.pairs {
        let driven: Vec<usize> = (0..tracked.len()).filter(|i| !undriven.contains(piston_pieces[*i])).collect();
        for &a in &driven {
            for &b in &driven {
                let (pa, pb) = (tracked[a].0, tracked[b].0);
                if a == b || (pa.x - pb.x).abs().max((pa.y - pb.y).abs()) > 3 {
                    continue;
                }
                for (ka, kb, d) in [(1u8, 1u8, 0u8), (1, 1, 1), (1, 1, 2), (3, 3, 0)] {
                    actions.push(A::S { a: a as u8, b: b as u8, ka, kb, d });
                }
            }
        }
    }
    let mut door = vec![None; 9];
    let mut ctx = Ctx { bx, pieces: pieces.clone(), min, size, door: Vec::new(), ring: Vec::new(), n_pistons: tracked.len(), actions, max_ticks: o.max_ticks };
    for x in 0..3i8 {
        for y in 0..3i8 {
            door[(x * 3 + y) as usize] = ctx.idx(x, y);
        }
    }
    ctx.door = door;
    ctx.ring = ring().iter().map(|(c, b)| (ctx.idx(c.0, c.1), ctx.idx(b.0, b.1))).collect();
    let mut w = World::new(min, size);
    w.piston.tracked = tracked;
    let mut blocks: Vec<(BlockPos, Block)> = planned.iter().map(|(c, b)| (wp(c.0, c.1), *b)).collect();
    blocks.sort_by_key(|(p, _)| (p.y, p.x));
    w.place_like_judge(&blocks);
    let mut swept = 0;
    ctx.settle(&mut w, &mut swept)?;
    let key = ctx.key_of(&w);
    // the rest state must be the placement itself
    for (c, b) in &planned {
        if key.g[ctx.idx(c.0, c.1)?] != *b {
            return None;
        }
    }
    if key.g.iter().filter(|b| !b.is_air()).count() != planned.len() {
        return None;
    }
    Some((ctx, key))
}

#[derive(Clone, Debug)]
pub struct Found {
    pub open: Vec<A>,
    pub close: Vec<A>,
    pub states: usize,
    /// Pistons whose input the schedules use.
    pub used: Vec<bool>,
}

#[derive(PartialEq, Eq)]
struct Node {
    f: u32,
    g: u32,
    id: usize,
}
impl Ord for Node {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.f.cmp(&self.f).then(o.g.cmp(&self.g)).then(self.id.cmp(&o.id))
    }
}
impl PartialOrd for Node {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

/// Weighted A* (h x2) from `start` to a goal state; returns the action path with the swept ring
/// cells, and the number of states seen. Err = cap hit.
fn astar(ctx: &Ctx, start: &Key, goal: &dyn Fn(&Key) -> bool, h: &dyn Fn(&Key) -> u32, cap: usize, skip: &HashSet<Key>) -> (Result<Option<(Vec<A>, u16, Key)>, ()>, usize) {
    let mut keys: Vec<Key> = vec![start.clone()];
    let mut parent: Vec<Option<(usize, A, u16)>> = vec![None];
    let mut gcost: Vec<u32> = vec![0];
    let mut index: HashMap<Key, usize> = HashMap::new();
    index.insert(start.clone(), 0);
    let mut heap = BinaryHeap::new();
    heap.push(Node { f: 2 * h(start), g: 0, id: 0 });
    while let Some(Node { g, id, .. }) = heap.pop() {
        if g > gcost[id] {
            continue;
        }
        if id != 0 && goal(&keys[id]) && !skip.contains(&keys[id]) {
            let mut path = Vec::new();
            let mut swept = 0u16;
            let mut cur = id;
            while let Some((p, a, s)) = parent[cur] {
                path.push(a);
                swept |= s;
                cur = p;
            }
            path.reverse();
            return (Ok(Some((path, swept, keys[id].clone()))), keys.len());
        }
        if keys.len() >= cap {
            return (Err(()), keys.len());
        }
        let cur = keys[id].clone();
        for &a in &ctx.actions {
            let Some((nk, swept)) = ctx.step(&cur, a) else { continue };
            let ng = g + 1;
            match index.get(&nk) {
                Some(&j) => {
                    if ng < gcost[j] {
                        gcost[j] = ng;
                        parent[j] = Some((id, a, swept));
                        heap.push(Node { f: ng + 2 * h(&nk), g: ng, id: j });
                    }
                }
                None => {
                    let j = keys.len();
                    index.insert(nk.clone(), j);
                    heap.push(Node { f: ng + 2 * h(&nk), g: ng, id: j });
                    keys.push(nk);
                    parent.push(Some((id, a, swept)));
                    gcost.push(ng);
                }
            }
        }
    }
    (Ok(None), keys.len())
}

/// Open (FULL seamless) and close back to rest. Ok(None) = the state space has no open state
/// (exhausted); Err = the cap stopped it.
pub fn solve(ctx: &Ctx, rest: &Key, cap: usize) -> Result<Option<Found>, ()> {
    let mut tried: HashSet<Key> = HashSet::new();
    let mut states = 0;
    for _attempt in 0..6 {
        let (r, n) = astar(ctx, rest, &|k| ctx.is_open(k), &|k| ctx.h_open(k), cap, &tried);
        states += n;
        let (open_path, open_swept, open_key) = match r {
            Err(()) => return Err(()),
            Ok(None) => return Ok(None),
            Ok(Some(x)) => x,
        };
        tried.insert(open_key.clone());
        let diff = |k: &Key| -> u32 { k.g.iter().zip(rest.g.iter()).filter(|(a, b)| a != b).count() as u32 + (k.inp ^ rest.inp).count_ones() };
        let (r2, n2) = astar(ctx, &open_key, &|k| k == rest, &diff, cap, &HashSet::new());
        states += n2;
        if let Ok(Some((close_path, close_swept, _))) = r2 {
            if ctx.holes_ok(&open_key, open_swept | close_swept) {
                let mut used = vec![false; ctx.n_pistons];
                for a in open_path.iter().chain(close_path.iter()) {
                    match *a {
                        A::L(i) | A::P(i, _) => used[i as usize] = true,
                        A::S { a, b, .. } => {
                            used[a as usize] = true;
                            used[b as usize] = true;
                        }
                    }
                }
                return Ok(Some(Found { open: open_path, close: close_path, states, used }));
            }
        }
    }
    Ok(None)
}

/// The exact verdict for a families candidate (an API for `door families`): Ok(Some) = opens
/// (FULL seamless) and closes back to rest with ideal inputs on these pistons under the real
/// game rules; Ok(None) = the reachable space has no such schedule (or the rest state does not
/// hold); Err = the state cap stopped it. Helpers (redstone blocks, observers) go in `extra`.
pub fn exact_verdict(bx: Bx, pcs: &[Pc], extra: &[FPc], pulses: &[u8], cap: usize) -> Result<Option<Found>, ()> {
    let mut v: Vec<FPc> = pcs.iter().map(FPc::from_pc).collect();
    v.extend_from_slice(extra);
    let o = Opts { pulses: pulses.to_vec(), levels: false, cap, max_ticks: 80, pairs: false };
    match build(bx, &v, &o) {
        None => Ok(None),
        Some((ctx, rest)) => solve(&ctx, &rest, cap),
    }
}

// ---- input reduction: helpers that replace a piston's wiring ----

/// For each piston of a working mechanism, try to drop its input: one helper (a redstone block
/// or an observer of each in-plane facing) in each free cell of the box, the piston undriven,
/// solved exactly. Returns (undriven piston, helper, solution).
pub fn reduce(bx: Bx, pieces: &[FPc], o: &Opts, cap: usize, threads: usize) -> Vec<(FPc, FPc, Found)> {
    let mut occupied: HashSet<(i8, i8)> = HashSet::new();
    for x in 0..3 {
        for y in 0..3 {
            occupied.insert((x, y));
        }
    }
    for p in pieces {
        for c in p.cells() {
            occupied.insert(c);
        }
    }
    let mut free = Vec::new();
    for y in bx.y0..bx.y0 + bx.h {
        for x in bx.x0..bx.x0 + bx.w {
            if !occupied.contains(&(x, y)) {
                free.push((x, y));
            }
        }
    }
    let pistons: Vec<FPc> = pieces.iter().filter(|p| matches!(p.k, FK::Piston { .. })).copied().collect();
    let kinds = [FK::RedstoneBlock, FK::Observer { f: 0 }, FK::Observer { f: 1 }, FK::Observer { f: 2 }, FK::Observer { f: 3 }];
    let mut jobs = Vec::new();
    for p in &pistons {
        for &c in &free {
            for k in kinds {
                jobs.push((*p, FPc { x: c.0, y: c.1, k }));
            }
        }
    }
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<(FPc, FPc, Found)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let (p, h) = jobs[i];
                let mut v = pieces.to_vec();
                v.push(h);
                if let Some((ctx, rest)) = build_driven(bx, &v, o, &[p]) {
                    if let Ok(Some(f)) = solve(&ctx, &rest, cap) {
                        out.lock().unwrap().push((p, h, f));
                    }
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|(p, h, f)| (f.open.len() + f.close.len(), *p, *h));
    v
}

// ---- the command ----

pub struct CliOpts {
    pub check: Vec<std::path::PathBuf>,
    pub boxes: Vec<(i8, i8)>,
    pub pmax: usize,
    pub two: usize,
    pub pulses: Vec<u8>,
    pub levels: bool,
    pub cap: usize,
    pub threads: usize,
    pub limit: usize,
    pub out: Option<std::path::PathBuf>,
    pub macro_cap: usize,
    pub only_new: bool,
    pub reduce: bool,
    pub reduce_cap: usize,
    pub pairs: bool,
}

fn pieces_of_design(path: &std::path::Path) -> Result<Vec<FPc>> {
    let d = crate::cli::load_design(path)?;
    let w = crate::mechsynth::world_of_design(&d)?;
    let mut pieces = Vec::new();
    for (p, k) in &w.b {
        if p[2] != 0 {
            anyhow::bail!("not flat: block at {p:?}");
        }
        use crate::mechsynth::Kind;
        let k = match k {
            Kind::Door | Kind::Stone | Kind::Obsidian => continue,
            Kind::Piston { sticky, f, ext } => FK::Piston { sticky: *sticky, f: *f, ext: *ext },
            k => anyhow::bail!("unsupported block {k:?}"),
        };
        pieces.push(FPc { x: p[0], y: p[1], k });
    }
    Ok(pieces)
}

fn describe(ctx: &Ctx, f: &Found) -> String {
    let used = f.used.iter().filter(|u| **u).count();
    format!(
        "open {} + close {} ({} states, {} of {} inputs used)\n  open:  {}\n  close: {}",
        f.open.len(),
        f.close.len(),
        f.states,
        used,
        ctx.n_pistons,
        f.open.iter().map(|a| ctx.fmt_action(*a)).collect::<Vec<_>>().join(" "),
        f.close.iter().map(|a| ctx.fmt_action(*a)).collect::<Vec<_>>().join(" ")
    )
}

pub fn cli(o: &CliOpts) -> Result<()> {
    let so = Opts { pulses: o.pulses.clone(), levels: o.levels, cap: o.cap, max_ticks: 80, pairs: o.pairs };
    for path in &o.check {
        let pieces = pieces_of_design(path)?;
        let (mut x0, mut y0, mut x1, mut y1) = (0i8, 0i8, 2i8, 2i8);
        for p in &pieces {
            for (x, y) in p.cells() {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
        let bx = Bx { x0, y0, w: x1 - x0 + 1, h: y1 - y0 + 1 };
        println!("{}: {} pieces in {}x{}: {}", path.display(), pieces.len(), bx.w, bx.h, pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "));
        let Some((ctx, rest)) = build(bx, &pieces, &so) else {
            println!("  rest state does not hold");
            continue;
        };
        print!("{}", ctx.render(&rest));
        let t0 = std::time::Instant::now();
        match solve(&ctx, &rest, o.cap) {
            Ok(Some(f)) => println!("  {} [{:.1}s]", describe(&ctx, &f), t0.elapsed().as_secs_f64()),
            Ok(None) => println!("  no FULL seamless open + close (exhausted) [{:.1}s]", t0.elapsed().as_secs_f64()),
            Err(()) => println!("  cap hit [{:.1}s]", t0.elapsed().as_secs_f64()),
        }
        if o.reduce {
            let t1 = std::time::Instant::now();
            let r = reduce(bx, &pieces, &so, o.reduce_cap, o.threads);
            let mut by_piston: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
            for (p, _, _) in &r {
                *by_piston.entry(p.code()).or_default() += 1;
            }
            println!("  input reduction: {} (piston, helper) pairs work [{:.1}s]; per undriven piston: {:?}", r.len(), t1.elapsed().as_secs_f64(), by_piston);
            for (p, h, f) in r.iter().take(12) {
                let mut v = pieces.clone();
                v.push(*h);
                if let Some((c2, rest2)) = build_driven(bx, &v, &so, &[*p]) {
                    println!("  - {} undriven, helper {}: {}", p.code(), h.code(), describe(&c2, f).replace('\n', "\n    "));
                    let _ = rest2;
                }
            }
        }
    }
    let mut report = String::new();
    for &(w, h) in &o.boxes {
        let t0 = std::time::Instant::now();
        let mut cands: Vec<(Bx, Vec<Pc>)> = Vec::new();
        let mut seen: HashSet<Vec<Pc>> = HashSet::new();
        for dx in 0..=(w - 3) {
            for dy in 0..=(h - 3) {
                if dx > (w - 3) - dx {
                    continue;
                }
                let bx = Bx { x0: -dx, y0: -dy, w, h };
                for v in families::generate(bx, o.pmax, true, true, o.two) {
                    if seen.insert(families::canon(&v)) {
                        cands.push((bx, v));
                    }
                }
            }
        }
        if o.limit > 0 && cands.len() > o.limit {
            cands.truncate(o.limit);
        }
        eprintln!("[fullmech] {w}x{h}: {} candidates ({:.1}s)", cands.len(), t0.elapsed().as_secs_f64());
        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        let found: Mutex<Vec<(Bx, Vec<Pc>, Found, bool)>> = Mutex::new(Vec::new());
        let (capped, macro_ok, nobuild) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0));
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cands.len() {
                        break;
                    }
                    let (bx, v) = &cands[i];
                    // the macro model's verdict (a cheap filter for --only-new)
                    let macro_solves = families::Mech::new(*bx, v).is_some_and(|(m, rest)| matches!(families::solve(&m, &rest, o.macro_cap, false), Ok(Some(_))));
                    if macro_solves {
                        macro_ok.fetch_add(1, Ordering::Relaxed);
                    }
                    if !(o.only_new && macro_solves) {
                        let fp: Vec<FPc> = v.iter().map(FPc::from_pc).collect();
                        match build(*bx, &fp, &so) {
                            None => {
                                nobuild.fetch_add(1, Ordering::Relaxed);
                            }
                            Some((ctx, rest)) => match solve(&ctx, &rest, o.cap) {
                                Ok(Some(f)) => found.lock().unwrap().push((*bx, v.clone(), f, macro_solves)),
                                Ok(None) => {}
                                Err(()) => {
                                    capped.fetch_add(1, Ordering::Relaxed);
                                }
                            },
                        }
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d % 200 == 0 {
                        eprintln!("[fullmech] {w}x{h}: {d}/{} done, {} found, {} capped ({:.0}s)", cands.len(), found.lock().unwrap().len(), capped.load(Ordering::Relaxed), t0.elapsed().as_secs_f64());
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        let new = found.iter().filter(|f| !f.3).count();
        let line = format!(
            "{w}x{h}: {} candidates, exact model opens+closes {} ({} that the macro model does not), macro model {} , capped {}, rest state not holding {} [{:.0}s]",
            cands.len(),
            found.len(),
            new,
            macro_ok.load(Ordering::Relaxed),
            capped.load(Ordering::Relaxed),
            nobuild.load(Ordering::Relaxed),
            t0.elapsed().as_secs_f64()
        );
        println!("{line}");
        report.push_str(&line);
        report.push('\n');
        for (bx, v, f, m) in found.iter().filter(|f| !o.only_new || !f.3).take(40) {
            let fp: Vec<FPc> = v.iter().map(FPc::from_pc).collect();
            if let Some((ctx, rest)) = build(*bx, &fp, &so) {
                let s = format!(
                    "  [{}] {}\n{}  {}\n",
                    if *m { "macro too" } else { "EXACT ONLY" },
                    fp.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "),
                    ctx.render(&rest),
                    describe(&ctx, f)
                );
                print!("{s}");
                report.push_str(&s);
            }
        }
    }
    if let Some(out) = &o.out {
        std::fs::write(out, report).with_context(|| format!("writing {}", out.display()))?;
    }
    Ok(())
}
