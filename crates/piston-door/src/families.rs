//! `mctas door families`: the catalog of 1-wide (flat) 3x3 door MECHANISMS per box.
//!
//! A family = the pistons that move the 9 door blocks and the paths the blocks take (wiring and
//! timing aside). Everything lives in the door plane: pistons face east/west/up/down, the door
//! is the 3x3 at x 0..2, y 0..2 (door coordinates), the box W x H holds every block and every
//! piston head at every state of the cycle.
//!
//! Model: a fast grid port of mech-synth's macro piston model (`mechsynth.rs`, itself a port of
//! `PistonStructureResolver` + the sticky retract rules): one settled piston action at a time,
//! `L` = toggle the level (extend / retract with the sticky pull), `P` = a 2 gt pulse from rest
//! (push and spit; or extend into a gap and pull the block beyond). No slime or honey (the
//! default and, for now, the only mode). A level-on that cannot extend is refused (strict), and
//! a move that would leave the box is refused.
//!
//! Generation ("exit plans"): every door block gets a path of unit moves out of the 3x3 (the
//! center exactly 2 moves, the 8 others 1 or 2), and every move gets a provider: a PULL by a
//! sticky piston two cells ahead (retracted with a gap, or extended at rest with its head on
//! the gap), or a PUSH by a piston one or two cells behind; a piston already placed may serve
//! again from its rest cell or lifted one cell along its own axis by a LIFT piston right behind
//! it (the elevator). The union of the providers is a candidate mechanism (<= P pistons).
//! Each distinct candidate is then solved exactly in the macro model (the whole reachable state
//! graph): some schedule must open (3x3 empty, SEAMLESS: no piston, head or obsidian in the 12
//! ring cells around the opening, every swept ring cell that is air when open has nothing but
//! air or a door block right behind it) and close back to the exact rest state. Survivors are
//! reduced to MINIMAL mechanisms (no piston can be dropped), mirror images (x -> 2 - x) are
//! merged, and each gets its shortest open/close schedules and, when one exists, ONE pulse train
//! that both lever edges can share (product BFS of the run from rest and the run from open).

use crate::mechsynth::{self, Act, Ev, Kind, World};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const MAXC: usize = 144;
const AIR: u8 = 0;
const DOOR: u8 = 1;
const STONE: u8 = 2;
const OBS: u8 = 3;
const BODY: u8 = 16;
const HEAD: u8 = 48;
/// East, west, up, down (mechsynth's D indices 0..3).
const F: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const FNAME: [&str; 4] = ["E", "W", "U", "D"];
const MAX_PUSH: usize = 12;

fn opp(f: u8) -> u8 {
    f ^ 1
}
fn fdir(d: (i8, i8)) -> u8 {
    F.iter().position(|&x| x == d).unwrap() as u8
}
pub fn in_t(x: i8, y: i8) -> bool {
    (0..=2).contains(&x) && (0..=2).contains(&y)
}
/// The 12 hallway-ring cells of the door plane (face-adjacent to the 3x3).
pub fn in_r(x: i8, y: i8) -> bool {
    ((x == -1 || x == 3) && (0..=2).contains(&y)) || ((y == -1 || y == 3) && (0..=2).contains(&x))
}

/// A piece at rest (door coordinates).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum K {
    Stone,
    Obsidian,
    /// f: 0 E, 1 W, 2 U, 3 D.
    Piston { sticky: bool, f: u8, ext: bool },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Pc {
    pub x: i8,
    pub y: i8,
    pub k: K,
}

impl Pc {
    pub fn code(&self) -> String {
        match self.k {
            K::Stone => format!("stone({},{})", self.x, self.y),
            K::Obsidian => format!("obsidian({},{})", self.x, self.y),
            K::Piston { sticky, f, ext } => format!("{}{}{}({},{})", if sticky { "S" } else { "N" }, FNAME[f as usize], if ext { "+" } else { "" }, self.x, self.y),
        }
    }
    fn cells(&self) -> Vec<(i8, i8)> {
        match self.k {
            K::Piston { ext: true, f, .. } => vec![(self.x, self.y), (self.x + F[f as usize].0, self.y + F[f as usize].1)],
            _ => vec![(self.x, self.y)],
        }
    }
    fn mirror(&self) -> Pc {
        let k = match self.k {
            K::Piston { sticky, f, ext } => K::Piston { sticky, f: if f < 2 { opp(f) } else { f }, ext },
            k => k,
        };
        Pc { x: 2 - self.x, y: self.y, k }
    }
}

/// The box in door coordinates: x in x0..x0+w, y in y0..y0+h.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Bx {
    pub x0: i8,
    pub y0: i8,
    pub w: i8,
    pub h: i8,
}

impl Bx {
    pub fn has(&self, x: i8, y: i8) -> bool {
        x >= self.x0 && x < self.x0 + self.w && y >= self.y0 && y < self.y0 + self.h
    }
    fn idx(&self, x: i8, y: i8) -> usize {
        ((y - self.y0) as usize) * (self.w as usize) + (x - self.x0) as usize
    }
    fn xy(&self, i: usize) -> (i8, i8) {
        (self.x0 + (i % self.w as usize) as i8, self.y0 + (i / self.w as usize) as i8)
    }
    fn n(&self) -> usize {
        (self.w as usize) * (self.h as usize)
    }
}

/// One state: the grid (codes) + the level bits; `pp` = piston positions (derived).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct St {
    pub g: [u8; MAXC],
    pub pow: u16,
    pub pp: [u8; 16],
}

/// The static part of a mechanism: box + pistons (sticky, facing) by id.
#[derive(Clone, Debug)]
pub struct Mech {
    pub bx: Bx,
    pub pieces: Vec<Pc>,
    pub info: Vec<(bool, u8)>,
    /// piece index of piston id
    pub pid: Vec<usize>,
    /// Extended (powered) at rest, by piston id.
    pub ext_rest: Vec<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct E {
    pub id: u8,
    pub act: Act,
}

pub enum Out {
    Ok(St),
    Noop,
    Illegal,
}

impl Mech {
    pub fn new(bx: Bx, pieces: &[Pc]) -> Option<(Mech, St)> {
        let mut pieces = pieces.to_vec();
        pieces.sort();
        let mut g = [AIR; MAXC];
        let mut pp = [0u8; 16];
        let mut pow = 0u16;
        let mut info = Vec::new();
        let mut pid = Vec::new();
        for x in 0..3 {
            for y in 0..3 {
                if !bx.has(x, y) {
                    return None;
                }
                g[bx.idx(x, y)] = DOOR;
            }
        }
        for (pi, p) in pieces.iter().enumerate() {
            for (x, y) in p.cells() {
                if !bx.has(x, y) || g[bx.idx(x, y)] != AIR {
                    return None;
                }
            }
            match p.k {
                K::Stone => g[bx.idx(p.x, p.y)] = STONE,
                K::Obsidian => g[bx.idx(p.x, p.y)] = OBS,
                K::Piston { sticky, f, ext } => {
                    let id = info.len();
                    if id >= 16 {
                        return None;
                    }
                    info.push((sticky, f));
                    pid.push(pi);
                    let i = bx.idx(p.x, p.y);
                    g[i] = BODY + id as u8;
                    pp[id] = i as u8;
                    if ext {
                        let (dx, dy) = F[f as usize];
                        g[bx.idx(p.x + dx, p.y + dy)] = HEAD + id as u8;
                        pow |= 1 << id;
                    }
                }
            }
        }
        let ext_rest: Vec<bool> = (0..info.len()).map(|id| pow & (1 << id) != 0).collect();
        Some((Mech { bx, pieces, info, pid, ext_rest }, St { g, pow, pp }))
    }

    fn step(&self, i: usize, f: u8) -> Option<usize> {
        let (x, y) = self.bx.xy(i);
        let (dx, dy) = F[f as usize];
        let (nx, ny) = (x + dx, y + dy);
        if self.bx.has(nx, ny) { Some(self.bx.idx(nx, ny)) } else { None }
    }

    fn ext(&self, s: &St, id: usize) -> bool {
        let (_, f) = self.info[id];
        self.step(s.pp[id] as usize, f).is_some_and(|h| s.g[h] == HEAD + id as u8)
    }

    fn pushable(&self, s: &St, c: u8) -> bool {
        match c {
            DOOR | STONE => true,
            OBS => false,
            c if (BODY..HEAD).contains(&c) => !self.ext(s, (c - BODY) as usize),
            _ => false,
        }
    }

    /// The push list from `start` along f: Ok(list) / Err(true) = out of box / Err(false) = blocked.
    fn push_list(&self, s: &St, start: Option<usize>, f: u8) -> Result<Vec<usize>, bool> {
        let mut list = Vec::new();
        let mut c = start.ok_or(true)?;
        while s.g[c] != AIR {
            if !self.pushable(s, s.g[c]) {
                return Err(false);
            }
            list.push(c);
            if list.len() > MAX_PUSH {
                return Err(false);
            }
            c = self.step(c, f).ok_or(true)?;
        }
        Ok(list)
    }

    fn move_list(&self, s: &mut St, list: &[usize], f: u8) {
        for &c in list.iter().rev() {
            let n = self.step(c, f).unwrap();
            let v = s.g[c];
            s.g[n] = v;
            s.g[c] = AIR;
            if (BODY..HEAD).contains(&v) {
                s.pp[(v - BODY) as usize] = n as u8;
            }
        }
    }

    /// Sticky pull: the block two cells ahead of the piston at `p` (head already gone).
    fn pull(&self, s: &mut St, p: usize, f: u8) {
        let Some(one) = self.step(p, f) else { return };
        let Some(two) = self.step(one, f) else { return };
        let c = s.g[two];
        if s.g[one] != AIR || c == AIR || c == OBS || !self.pushable(s, c) {
            return;
        }
        s.g[one] = c;
        s.g[two] = AIR;
        if (BODY..HEAD).contains(&c) {
            s.pp[(c - BODY) as usize] = one as u8;
        }
    }

    pub fn apply(&self, s: &St, e: E) -> Out {
        let id = e.id as usize;
        let (sticky, f) = self.info[id];
        let p = s.pp[id] as usize;
        let bit = 1u16 << id;
        let powered = s.pow & bit != 0;
        let ext = self.ext(s, id);
        let mut n = s.clone();
        match e.act {
            Act::L => {
                if powered {
                    n.pow &= !bit;
                    if ext {
                        let h = self.step(p, f).unwrap();
                        n.g[h] = AIR;
                        if sticky {
                            self.pull(&mut n, p, f);
                        }
                    }
                    Out::Ok(n)
                } else {
                    match self.push_list(s, self.step(p, f), f) {
                        Err(true) => Out::Illegal,
                        Err(false) => Out::Illegal, // strict: a blocked level-on is refused
                        Ok(list) => {
                            self.move_list(&mut n, &list, f);
                            n.g[self.step(p, f).unwrap()] = HEAD + id as u8;
                            n.pow |= bit;
                            Out::Ok(n)
                        }
                    }
                }
            }
            Act::P => {
                if powered || ext {
                    return Out::Noop;
                }
                match self.push_list(s, self.step(p, f), f) {
                    Err(true) => Out::Illegal,
                    Err(false) => Out::Noop,
                    Ok(list) if !list.is_empty() => {
                        self.move_list(&mut n, &list, f);
                        Out::Ok(n)
                    }
                    Ok(_) => {
                        if !sticky {
                            return Out::Noop;
                        }
                        self.pull(&mut n, p, f);
                        if n == *s { Out::Noop } else { Out::Ok(n) }
                    }
                }
            }
        }
    }

    /// The pulse model: a 2 gt pulse on every piston, levels only on pistons extended at rest
    /// (their release and re-power).
    pub fn events_pulse(&self) -> Vec<E> {
        let mut v = Vec::new();
        for id in 0..self.info.len() {
            v.push(E { id: id as u8, act: Act::P });
            if self.ext_rest[id] {
                v.push(E { id: id as u8, act: Act::L });
            }
        }
        v
    }

    pub fn events(&self, levels: bool) -> Vec<E> {
        let mut v = Vec::new();
        for id in 0..self.info.len() {
            v.push(E { id: id as u8, act: Act::P });
            if levels {
                v.push(E { id: id as u8, act: Act::L });
            }
        }
        v
    }

    fn code_at(&self, s: &St, x: i8, y: i8) -> u8 {
        if self.bx.has(x, y) { s.g[self.bx.idx(x, y)] } else { AIR }
    }

    /// Static open test (FULL seamless for a flat door): the 3x3 empty, the ring holds only
    /// air, door blocks or plain (stone) helpers.
    pub fn is_open(&self, s: &St) -> bool {
        for x in 0..3 {
            for y in 0..3 {
                if self.code_at(s, x, y) != AIR {
                    return false;
                }
            }
        }
        for (x, y) in ring() {
            if !matches!(self.code_at(s, x, y), AIR | DOOR | STONE) {
                return false;
            }
        }
        true
    }

    /// The hole rule over a cycle: ring cells some state of the cycle fills but the open state
    /// leaves empty must have air or a door block right behind them (outward) when open.
    pub fn holes_ok(&self, cycle: &[&St], open: &St) -> bool {
        for (x, y) in ring() {
            if self.code_at(open, x, y) != AIR {
                continue;
            }
            if !cycle.iter().any(|s| self.code_at(s, x, y) != AIR) {
                continue;
            }
            let (ox, oy) = outward(x, y);
            if !matches!(self.code_at(open, ox, oy), AIR | DOOR) {
                return false;
            }
        }
        true
    }

    /// No piston, head or obsidian in the ring at rest (the coordinator's strict reading).
    pub fn rest_ring_clean(&self, rest: &St) -> bool {
        ring().into_iter().all(|(x, y)| matches!(self.code_at(rest, x, y), AIR | DOOR | STONE))
    }

    /// ASCII picture of a state (top row first): # door, s stone, O obsidian, piston letters
    /// (> < ^ v; uppercase sticky), heads = '=' or '|'.
    pub fn render(&self, s: &St) -> String {
        let mut out = String::new();
        for y in (self.bx.y0..self.bx.y0 + self.bx.h).rev() {
            out.push_str(&format!("{:>3} ", y));
            for x in self.bx.x0..self.bx.x0 + self.bx.w {
                let c = s.g[self.bx.idx(x, y)];
                let ch = match c {
                    AIR => if in_t(x, y) { ',' } else { '.' },
                    DOOR => '#',
                    STONE => 's',
                    OBS => 'O',
                    c if (BODY..HEAD).contains(&c) => {
                        let (sticky, f) = self.info[(c - BODY) as usize];
                        let ch = ['>', '<', '^', 'v'][f as usize];
                        if sticky { ch } else { ['}', '{', 'A', 'V'][f as usize] }
                    }
                    c => {
                        let (_, f) = self.info[(c - HEAD) as usize];
                        if f < 2 { '=' } else { '|' }
                    }
                };
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }

    pub fn fmt_ev(&self, e: E) -> String {
        let p = self.pieces[self.pid[e.id as usize]];
        format!("{}{}", if e.act == Act::P { "P" } else { "L" }, p.code())
    }
    pub fn fmt_seq(&self, s: &[E]) -> String {
        s.iter().map(|e| self.fmt_ev(*e)).collect::<Vec<_>>().join(" ")
    }
    /// Bounding box (door coords) of every state in the list.
    pub fn extent(&self, states: &[&St]) -> (i8, i8, i8, i8) {
        let (mut x0, mut y0, mut x1, mut y1) = (0i8, 0i8, 2i8, 2i8);
        for s in states {
            for i in 0..self.bx.n() {
                if s.g[i] != AIR {
                    let (x, y) = self.bx.xy(i);
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0, y0, x1, y1)
    }
}

pub fn ring() -> Vec<(i8, i8)> {
    let mut v = Vec::new();
    for t in 0..3 {
        v.push((-1, t));
        v.push((3, t));
        v.push((t, -1));
        v.push((t, 3));
    }
    v
}

fn outward(x: i8, y: i8) -> (i8, i8) {
    if x == -1 {
        (-2, y)
    } else if x == 3 {
        (4, y)
    } else if y == -1 {
        (x, -2)
    } else {
        (x, 4)
    }
}

// ---------------------------------------------------------------- solving

#[derive(Clone, Debug)]
pub struct Sol {
    pub open: Vec<E>,
    pub close: Vec<E>,
    pub sym: Option<Vec<E>>,
    pub sym_levels: bool,
    pub states: usize,
    pub extent: (i8, i8, i8, i8),
    /// Found only with level events (the pulse model has no open+close).
    pub levels: bool,
}

/// Weighted A* from `start` to states meeting `goal` (up to `want` goal states, in order found).
/// Err(()) when the cap stops it before `want` goals; Ok(v) with fewer goals = space exhausted.
fn astar(m: &Mech, start: &St, goal: &dyn Fn(&St) -> bool, h: &dyn Fn(&St) -> u32, cap: usize, want: usize) -> (Vec<(St, Vec<E>)>, bool) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let evs = m.events(true);
    let mut states: Vec<St> = vec![start.clone()];
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    let mut gs: Vec<u32> = vec![0];
    let mut index: HashMap<St, u32> = HashMap::new();
    index.insert(start.clone(), 0);
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((2 * h(start), 0u32, 0u32)));
    let mut closed = vec![false];
    let mut out = Vec::new();
    let mut capped = false;
    while let Some(Reverse((_, g, s))) = heap.pop() {
        if closed[s as usize] || g > gs[s as usize] {
            continue;
        }
        closed[s as usize] = true;
        let st = states[s as usize].clone();
        if goal(&st) {
            let mut path = Vec::new();
            let mut c = s;
            while let (p, Some(e)) = parent[c as usize] {
                path.push(e);
                c = p;
            }
            path.reverse();
            out.push((st, path));
            if out.len() >= want {
                return (out, false);
            }
            continue;
        }
        for &e in &evs {
            let Out::Ok(n) = m.apply(&st, e) else { continue };
            let ng = g + 1;
            match index.get(&n) {
                Some(&id) => {
                    if ng < gs[id as usize] && !closed[id as usize] {
                        gs[id as usize] = ng;
                        parent[id as usize] = (s, Some(e));
                        heap.push(Reverse((ng + 2 * h(&n), ng, id)));
                    }
                }
                None => {
                    if states.len() >= cap {
                        capped = true;
                        continue;
                    }
                    let id = states.len() as u32;
                    let f = ng + 2 * h(&n);
                    index.insert(n.clone(), id);
                    states.push(n);
                    parent.push((s, Some(e)));
                    gs.push(ng);
                    closed.push(false);
                    heap.push(Reverse((f, ng, id)));
                }
            }
        }
    }
    (out, capped)
}

/// Open + close by weighted A* (h = door blocks still in the 3x3, the center counting 2; then
/// cells differing from rest). Ok(None) = the reachable space has no open state (exhausted);
/// Err(()) = not found within the cap.
/// Open + close: first EXACTLY in the pulse model (the whole state graph: pulses, levels only on
/// pistons extended at rest); if that space has no open+close, weighted A* with level events too
/// (h = door blocks still in the 3x3, the center counting 2; then cells differing from rest).
/// Ok(None) = proven impossible in the pulse model and not found with levels within the cap;
/// Err(()) = the pulse graph itself hit the cap.
pub fn solve(m: &Mech, rest: &St, cap: usize, levels: bool) -> Result<Option<Sol>, ()> {
    match solve_full(m, rest, cap, &m.events_pulse()) {
        Ok(Some(s)) => return Ok(Some(s)),
        Err(()) => return Err(()),
        Ok(None) if !levels => return Ok(None),
        Ok(None) => {}
    }
    match solve_levels(m, rest, cap) {
        Ok(s) => Ok(s),
        Err(()) => Ok(None),
    }
}

fn solve_levels(m: &Mech, rest: &St, cap: usize) -> Result<Option<Sol>, ()> {
    let bx = m.bx;
    let ci = bx.idx(1, 1);
    let tcells: Vec<usize> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).map(|(x, y)| bx.idx(x, y)).collect();
    let h_open = |s: &St| -> u32 { tcells.iter().map(|&i| if s.g[i] == AIR { 0 } else if i == ci { 2 } else { 1 }).sum() };
    let (opens, capped) = astar(m, rest, &|s| m.is_open(s), &h_open, cap, 6);
    if opens.is_empty() {
        return if capped { Err(()) } else { Ok(None) };
    }
    let n = bx.n();
    let h_close = |s: &St| -> u32 {
        let mut d = 0;
        for i in 0..n {
            if (s.g[i] == AIR) != (rest.g[i] == AIR) {
                d += 1;
            }
        }
        d / 2 + (s.pow ^ rest.pow).count_ones()
    };
    let mut best: Option<Sol> = None;
    let mut any_cap = capped;
    for (o, open) in &opens {
        let (back, c2) = astar(m, o, &|s| s == rest, &h_close, cap, 1);
        any_cap |= c2;
        let Some((_, close)) = back.into_iter().next() else { continue };
        // the cycle's states for the hole rule and the extent
        let mut cyc = vec![rest.clone()];
        let mut c = rest.clone();
        for e in open.iter().chain(close.iter()) {
            if let Out::Ok(nx) = m.apply(&c, *e) {
                c = nx;
            }
            cyc.push(c.clone());
        }
        let refs: Vec<&St> = cyc.iter().collect();
        if !m.holes_ok(&refs, o) {
            continue;
        }
        if best.as_ref().is_none_or(|b| open.len() + close.len() < b.open.len() + b.close.len()) {
            let extent = m.extent(&refs);
            best = Some(Sol { open: open.clone(), close, sym: None, sym_levels: false, states: 0, extent, levels: true });
        }
    }
    match best {
        Some(b) => Ok(Some(b)),
        None if any_cap => Err(()),
        None => Ok(None),
    }
}

/// Whole reachable graph (capped). None = no open+close; Err(()) = cap hit before an answer.
pub fn solve_full(m: &Mech, rest: &St, cap: usize, evs: &[E]) -> Result<Option<Sol>, ()> {
    let evs = evs.to_vec();
    let mut states: Vec<St> = vec![rest.clone()];
    let mut index: HashMap<St, u32> = HashMap::new();
    index.insert(rest.clone(), 0);
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    let mut edges: Vec<Vec<(u32, E)>> = vec![Vec::new()];
    let mut q = VecDeque::from([0u32]);
    let mut complete = true;
    while let Some(si) = q.pop_front() {
        let st = states[si as usize].clone();
        for &e in &evs {
            let Out::Ok(n) = m.apply(&st, e) else { continue };
            let ni = match index.get(&n) {
                Some(&i) => i,
                None => {
                    if states.len() >= cap {
                        complete = false;
                        continue;
                    }
                    let i = states.len() as u32;
                    index.insert(n.clone(), i);
                    states.push(n);
                    parent.push((si, Some(e)));
                    edges.push(Vec::new());
                    q.push_back(i);
                    i
                }
            };
            edges[si as usize].push((ni, e));
        }
    }
    let opens: Vec<u32> = (0..states.len() as u32).filter(|&i| m.is_open(&states[i as usize])).collect();
    if opens.is_empty() {
        return if complete { Ok(None) } else { Err(()) };
    }
    // distances to rest
    let n = states.len();
    let mut rev: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (s, es) in edges.iter().enumerate() {
        for (t, _) in es {
            rev[*t as usize].push(s as u32);
        }
    }
    let mut d2r = vec![u32::MAX; n];
    d2r[0] = 0;
    let mut q = VecDeque::from([0u32]);
    while let Some(t) = q.pop_front() {
        for &s in &rev[t as usize] {
            if d2r[s as usize] == u32::MAX {
                d2r[s as usize] = d2r[t as usize] + 1;
                q.push_back(s);
            }
        }
    }
    let depth = |mut i: u32| -> usize {
        let mut d = 0;
        while let (p, Some(_)) = parent[i as usize] {
            i = p;
            d += 1;
        }
        d
    };
    let mut ranked: Vec<(usize, u32)> = opens.iter().filter(|&&o| d2r[o as usize] != u32::MAX).map(|&o| (depth(o) + d2r[o as usize] as usize, o)).collect();
    ranked.sort();
    for &(_, o) in ranked.iter().take(64) {
        let mut open = Vec::new();
        let mut c = o;
        let mut path_states = vec![o];
        while let (p, Some(e)) = parent[c as usize] {
            open.push(e);
            c = p;
            path_states.push(c);
        }
        open.reverse();
        let mut close = Vec::new();
        let mut c = o;
        while d2r[c as usize] > 0 {
            let (t, e) = *edges[c as usize].iter().find(|(t, _)| d2r[*t as usize] == d2r[c as usize] - 1).unwrap();
            close.push(e);
            c = t;
            path_states.push(c);
        }
        let cyc: Vec<&St> = path_states.iter().map(|&i| &states[i as usize]).collect();
        if !m.holes_ok(&cyc, &states[o as usize]) {
            continue;
        }
        let extent = m.extent(&cyc);
        return Ok(Some(Sol { open, close, sym: None, sym_levels: false, states: n, extent, levels: false }));
    }
    if complete { Ok(None) } else { Err(()) }
}

/// One train S for both lever edges: S(rest) = an open state, S(that open state) = rest (door
/// blocks interchangeable). Product BFS over (run from rest, run from open); a no-op in one run
/// leaves it as is; a move out of the box (or a blocked level-on) rejects the event.
pub fn sym_train(m: &Mech, rest: &St, open: &St, levels: bool, cap: usize) -> Option<Vec<E>> {
    let evs = m.events(levels);
    let start = (rest.clone(), open.clone());
    let mut index: HashMap<(St, St), u32> = HashMap::new();
    let mut nodes: Vec<(St, St)> = vec![start.clone()];
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    index.insert(start, 0);
    let mut q = VecDeque::from([0u32]);
    while let Some(i) = q.pop_front() {
        let (a, b) = nodes[i as usize].clone();
        for &e in &evs {
            let na = match m.apply(&a, e) {
                Out::Ok(n) => Some(n),
                Out::Noop => None,
                Out::Illegal => continue,
            };
            let nb = match m.apply(&b, e) {
                Out::Ok(n) => Some(n),
                Out::Noop => None,
                Out::Illegal => continue,
            };
            if na.is_none() && nb.is_none() {
                continue;
            }
            let key = (na.unwrap_or_else(|| a.clone()), nb.unwrap_or_else(|| b.clone()));
            if index.contains_key(&key) {
                continue;
            }
            let done = key.0 == *open && key.1 == *rest;
            let j = nodes.len() as u32;
            index.insert(key.clone(), j);
            nodes.push(key);
            parent.push((i, Some(e)));
            if done {
                let mut seq = Vec::new();
                let mut c = j;
                while let (p, Some(e)) = parent[c as usize] {
                    seq.push(e);
                    c = p;
                }
                seq.reverse();
                return Some(seq);
            }
            if nodes.len() >= cap {
                return None;
            }
            q.push_back(j);
        }
    }
    None
}

/// Shortest symmetric train over the open states reachable by `open`-like paths: tries the
/// solution's open state first, then every open state of the graph (up to `tries`).
pub fn best_sym(m: &Mech, rest: &St, sol: &Sol, cap: usize, _tries: usize) -> Option<(Vec<E>, bool)> {
    let mut o = rest.clone();
    for e in &sol.open {
        if let Out::Ok(n) = m.apply(&o, *e) {
            o = n;
        }
    }
    // edge pistons: one pulse on each edge in the found schedules, touching no cell any other
    // piston's action changes (independent toggles); the rest is core
    let cnt = |s: &[E], id: u8, a: Act| s.iter().filter(|e| e.id == id && e.act == a).count();
    let mut touched: Vec<HashSet<usize>> = vec![HashSet::new(); m.info.len()];
    {
        let mut c = rest.clone();
        for e in sol.open.iter().chain(sol.close.iter()) {
            if let Out::Ok(n) = m.apply(&c, *e) {
                for i in 0..m.bx.n() {
                    if n.g[i] != c.g[i] {
                        touched[e.id as usize].insert(i);
                    }
                }
                c = n;
            }
        }
    }
    let mut edges = Vec::new();
    let mut core = Vec::new();
    for id in 0..m.info.len() as u8 {
        let po = cnt(&sol.open, id, Act::P);
        let pc = cnt(&sol.close, id, Act::P);
        let used = po + pc + cnt(&sol.open, id, Act::L) + cnt(&sol.close, id, Act::L);
        if used == 0 {
            continue;
        }
        let alone = (0..m.info.len()).all(|j| j == id as usize || touched[j].is_disjoint(&touched[id as usize]));
        if po == 1 && pc == 1 && used == 2 && !m.ext_rest[id as usize] && alone {
            edges.push(E { id, act: Act::P });
        } else {
            core.push(E { id, act: Act::P });
            if m.ext_rest[id as usize] || sol.levels {
                core.push(E { id, act: Act::L });
            }
        }
    }
    let run = |s: &St, seq: &[E]| -> Option<St> {
        let mut c = s.clone();
        for e in seq {
            match m.apply(&c, *e) {
                Out::Ok(n) => c = n,
                Out::Noop => {}
                Out::Illegal => return None,
            }
        }
        Some(c)
    };
    let k = edges.len();
    let combos: Vec<u32> = if k <= 9 { (0..(1u32 << k)).collect() } else { vec![0, (1u32 << k) - 1] };
    let mut best: Option<Vec<E>> = None;
    for mask in combos {
        let start: Vec<E> = (0..k).filter(|i| mask & (1 << i) != 0).map(|i| edges[i]).collect();
        let end: Vec<E> = (0..k).filter(|i| mask & (1 << i) == 0).map(|i| edges[i]).collect();
        let (Some(x0), Some(y0)) = (run(rest, &start), run(&o, &start)) else { continue };
        // product BFS over the core events
        let fin = |x: &St, y: &St| -> bool { run(x, &end).is_some_and(|a| a == o) && run(y, &end).is_some_and(|b| b == *rest) };
        let found = if fin(&x0, &y0) {
            Some(Vec::new())
        } else {
            let mut index: HashMap<(St, St), u32> = HashMap::new();
            let mut nodes: Vec<(St, St)> = vec![(x0.clone(), y0.clone())];
            let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
            index.insert((x0, y0), 0);
            let mut q = VecDeque::from([0u32]);
            let mut got = None;
            let limit = best.as_ref().map_or(usize::MAX, |b| b.len().saturating_sub(k));
            'bfs: while let Some(i) = q.pop_front() {
                let depth = {
                    let mut d = 0;
                    let mut c = i;
                    while let (p, Some(_)) = parent[c as usize] {
                        c = p;
                        d += 1;
                    }
                    d
                };
                if depth + 1 >= limit {
                    break;
                }
                let (a, b) = nodes[i as usize].clone();
                for &e in &core {
                    let na = match m.apply(&a, e) {
                        Out::Ok(n) => Some(n),
                        Out::Noop => None,
                        Out::Illegal => continue,
                    };
                    let nb = match m.apply(&b, e) {
                        Out::Ok(n) => Some(n),
                        Out::Noop => None,
                        Out::Illegal => continue,
                    };
                    if na.is_none() && nb.is_none() {
                        continue;
                    }
                    let key = (na.unwrap_or_else(|| a.clone()), nb.unwrap_or_else(|| b.clone()));
                    if index.contains_key(&key) {
                        continue;
                    }
                    let done = fin(&key.0, &key.1);
                    let j = nodes.len() as u32;
                    index.insert(key.clone(), j);
                    nodes.push(key);
                    parent.push((i, Some(e)));
                    if done {
                        let mut seq = Vec::new();
                        let mut c = j;
                        while let (p, Some(e)) = parent[c as usize] {
                            seq.push(e);
                            c = p;
                        }
                        seq.reverse();
                        got = Some(seq);
                        break 'bfs;
                    }
                    if nodes.len() >= cap {
                        break 'bfs;
                    }
                    q.push_back(j);
                }
            }
            got
        };
        let Some(c) = found else { continue };
        let mut full = start.clone();
        full.extend(c);
        full.extend(end.iter().copied());
        // the hole rule on the train's two runs
        let mut cyc: Vec<St> = vec![rest.clone()];
        for st in [rest, &o] {
            let mut cur = st.clone();
            for e in &full {
                if let Out::Ok(n) = m.apply(&cur, *e) {
                    cur = n;
                }
                cyc.push(cur.clone());
            }
        }
        let refs: Vec<&St> = cyc.iter().collect();
        if !m.holes_ok(&refs, &o) {
            continue;
        }
        if best.as_ref().is_none_or(|b| full.len() < b.len()) {
            best = Some(full);
        }
    }
    let lv = core.iter().any(|e| e.act == Act::L);
    best.map(|b| (b, lv))
}

// ---------------------------------------------------------------- generation (exit plans)

struct Gen {
    bx: Bx,
    pmax: usize,
    lifts: bool,
    pushes: bool,
    out: HashSet<Vec<Pc>>,
    memo: HashSet<(usize, Vec<Pc>)>,
    order: Vec<(i8, i8)>,
}

fn n_pistons(v: &[Pc]) -> usize {
    v.iter().filter(|p| matches!(p.k, K::Piston { .. })).count()
}

impl Gen {
    fn occupied(&self, v: &[Pc], x: i8, y: i8) -> bool {
        v.iter().any(|p| p.cells().contains(&(x, y)))
    }
    /// Add a piece; None when it conflicts. Same cell + same piece = reuse.
    fn add(&self, v: &[Pc], p: Pc) -> Option<Vec<Pc>> {
        if v.contains(&p) {
            return Some(v.to_vec());
        }
        for (x, y) in p.cells() {
            if !self.bx.has(x, y) || in_t(x, y) || self.occupied(v, x, y) {
                return None;
            }
        }
        let mut w = v.to_vec();
        w.push(p);
        w.sort();
        if n_pistons(&w) > self.pmax {
            return None;
        }
        Some(w)
    }

    /// Every way to provide the move c -> c + d.
    fn providers(&self, v: &[Pc], c: (i8, i8), d: (i8, i8)) -> Vec<Vec<Pc>> {
        let mut out = Vec::new();
        let fd = fdir(d);
        // PULL: sticky at c + 2d facing -d
        let a = (c.0 + 2 * d.0, c.1 + 2 * d.1);
        let fpull = opp(fd);
        for ext in [false, true] {
            if let Some(w) = self.add(v, Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: fpull, ext } }) {
                out.push(w);
            }
        }
        // PUSH: piston at c - k d facing d (k = 1, 2), retracted
        if self.pushes {
            for k in 1..=2i8 {
                let a = (c.0 - k * d.0, c.1 - k * d.1);
                if in_t(a.0, a.1) {
                    continue;
                }
                for sticky in [false, true] {
                    if let Some(w) = self.add(v, Pc { x: a.0, y: a.1, k: K::Piston { sticky, f: fd, ext: false } }) {
                        out.push(w);
                    }
                }
            }
        }
        // REUSE LIFTED: an existing piston of the right kind one cell off along its axis
        if self.lifts {
            for p in v {
                let K::Piston { sticky, f, ext } = p.k else { continue };
                let (fx, fy) = F[f as usize];
                let want: Vec<(i8, i8, bool)> = {
                    // (action position, needs sticky) for pull and push providers
                    let mut w = vec![(c.0 + 2 * d.0, c.1 + 2 * d.1, true)];
                    if f == fd {
                        w.push((c.0 - d.0, c.1 - d.1, false));
                        w.push((c.0 - 2 * d.0, c.1 - 2 * d.1, false));
                    }
                    w
                };
                for (ax, ay, need_sticky) in want {
                    if need_sticky && (!sticky || f != fpull) {
                        continue;
                    }
                    if !need_sticky && f != fd {
                        continue;
                    }
                    // lifted toward its facing: rest + f = action; lift at rest - f facing f
                    if p.x + fx == ax && p.y + fy == ay && !ext {
                        if let Some(w) = self.add(v, Pc { x: p.x - fx, y: p.y - fy, k: K::Piston { sticky: true, f, ext: false } }) {
                            out.push(w);
                        }
                    }
                    // pulled back: rest - f = action; lift extended at rest - 2f facing f
                    if p.x - fx == ax && p.y - fy == ay && !ext {
                        if let Some(w) = self.add(v, Pc { x: p.x - 2 * fx, y: p.y - 2 * fy, k: K::Piston { sticky: true, f, ext: true } }) {
                            out.push(w);
                        }
                    }
                }
            }
        }
        out
    }

    fn paths(&self, b: (i8, i8)) -> Vec<Vec<(i8, i8)>> {
        let center = b == (1, 1);
        let mut out = Vec::new();
        for d1 in F {
            let c1 = (b.0 + d1.0, b.1 + d1.1);
            if !self.bx.has(c1.0, c1.1) {
                continue;
            }
            if !center && !in_t(c1.0, c1.1) {
                out.push(vec![d1]);
            }
            for d2 in F {
                if d2 == (-d1.0, -d1.1) {
                    continue;
                }
                let c2 = (c1.0 + d2.0, c1.1 + d2.1);
                if !self.bx.has(c2.0, c2.1) || in_t(c2.0, c2.1) {
                    continue;
                }
                out.push(vec![d1, d2]);
            }
        }
        out
    }

    /// Provide every move of `path` from `b` (all provider combinations).
    fn provide_path(&self, v: &[Pc], b: (i8, i8), path: &[(i8, i8)]) -> Vec<Vec<Pc>> {
        let mut frontier = vec![v.to_vec()];
        let mut c = b;
        for d in path {
            let mut next = Vec::new();
            let mut seen = HashSet::new();
            for w in &frontier {
                for w2 in self.providers(w, c, *d) {
                    if seen.insert(w2.clone()) {
                        next.push(w2);
                    }
                }
            }
            frontier = next;
            c = (c.0 + d.0, c.1 + d.1);
        }
        frontier
    }

    /// The core: the center leaves straight along d in 2 moves, and the edge block on that line
    /// leaves in 2 moves (any 2-move path not ending on the center's park cell).
    fn core(&self, d: (i8, i8)) -> Vec<Vec<Pc>> {
        let e = (1 + d.0, 1 + d.1);
        let cpark = (1 + 2 * d.0, 1 + 2 * d.1);
        if !self.bx.has(cpark.0, cpark.1) {
            return Vec::new();
        }
        let mut out: HashSet<Vec<Pc>> = HashSet::new();
        for path in self.paths(e) {
            if path.len() != 2 {
                continue;
            }
            let end = path.iter().fold(e, |c, d| (c.0 + d.0, c.1 + d.1));
            if end == cpark {
                continue;
            }
            for w in self.provide_path(&[], e, &path) {
                for w2 in self.provide_path(&w, (1, 1), &[d, d]) {
                    out.insert(w2);
                }
            }
        }
        let mut v: Vec<Vec<Pc>> = out.into_iter().collect();
        v.sort();
        v
    }

    /// The other perimeter blocks: a 1-move exit by a pull-type puller (retracted), or, while
    /// the budget allows (`two_left`), a 2-move path with any providers.
    fn others(&mut self, i: usize, v: Vec<Pc>, two_left: usize) {
        if n_pistons(&v) > self.pmax {
            return;
        }
        if i == self.order.len() {
            self.out.insert(v);
            return;
        }
        if !self.memo.insert((i, v.clone())) {
            return;
        }
        let b = self.order[i];
        for d in F {
            let c = (b.0 + d.0, b.1 + d.1);
            if in_t(c.0, c.1) {
                continue;
            }
            let a = (b.0 + 2 * d.0, b.1 + 2 * d.1);
            if let Some(w) = self.add(&v, Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: opp(fdir(d)), ext: false } }) {
                self.others(i + 1, w, two_left);
            }
        }
        if two_left > 0 {
            for path in self.paths(b) {
                if path.len() != 2 {
                    continue;
                }
                for w in self.provide_path(&v, b, &path) {
                    self.others(i + 1, w, two_left - 1);
                }
            }
        }
    }
}

/// Candidate mechanisms of one box offset: for each center exit direction, every core option
/// times every exit choice of the 7 other perimeter blocks (`two` of them may take 2 moves).
pub fn generate(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize) -> Vec<Vec<Pc>> {
    let mut all: HashSet<Vec<Pc>> = HashSet::new();
    for d in F {
        let e = (1 + d.0, 1 + d.1);
        let order: Vec<(i8, i8)> = [(1, 0), (1, 2), (0, 1), (2, 1), (0, 0), (2, 0), (0, 2), (2, 2)].into_iter().filter(|&b| b != e).collect();
        let mut g = Gen { bx, pmax, lifts, pushes, out: HashSet::new(), memo: HashSet::new(), order };
        for c in g.core(d) {
            if n_pistons(&c) > pmax {
                continue;
            }
            g.others(0, c, two);
        }
        all.extend(g.out);
    }
    let mut v: Vec<Vec<Pc>> = all.into_iter().collect();
    v.sort();
    v
}

/// Canonical form under the left-right mirror (x -> 2 - x).
pub fn canon(v: &[Pc]) -> Vec<Pc> {
    let mut a = v.to_vec();
    a.sort();
    let mut b: Vec<Pc> = v.iter().map(|p| p.mirror()).collect();
    b.sort();
    a.min(b)
}

// ---------------------------------------------------------------- catalog

#[derive(Clone, Debug)]
pub struct Entry {
    pub pieces: Vec<Pc>,
    pub w: i8,
    pub h: i8,
    pub bx: Bx,
    pub sol: Sol,
    pub rest_clean: bool,
    pub pistons: usize,
    pub sticky: usize,
    pub free_cells: usize,
    pub ring_quasi: usize,
    pub verified: Option<String>,
    /// The pieces as solved in `bx` (the canonical form or its mirror image).
    pub orient: Vec<Pc>,
}

fn wireability(m: &Mech, rest: &St, sol: &Sol) -> (usize, usize) {
    // cells of the box never used by the mechanism over the cycle
    let mut used = vec![false; m.bx.n()];
    let mut c = rest.clone();
    let mut mark = |s: &St| {
        for i in 0..m.bx.n() {
            if s.g[i] != AIR {
                used[i] = true;
            }
        }
    };
    mark(&c);
    for e in sol.open.iter().chain(sol.close.iter()) {
        if let Out::Ok(n) = m.apply(&c, *e) {
            c = n;
        }
        mark(&c);
    }
    let free = used.iter().filter(|u| !**u).count();
    // pistons whose quasi cell (above) is a ring cell or the 3x3 (hard to power by QC)
    let rq = m
        .pieces
        .iter()
        .filter(|p| matches!(p.k, K::Piston { .. }) && (in_r(p.x, p.y + 1) || in_t(p.x, p.y + 1)))
        .count();
    (free, rq)
}

/// Convert to mech-synth's world (for ideal plans and simulator replays).
pub fn to_world(m: &Mech) -> (World, Vec<u8>) {
    let mut b: Vec<([i8; 3], Kind)> = Vec::new();
    for x in 0..3 {
        for y in 0..3 {
            b.push(([x, y, 0], Kind::Door));
        }
    }
    for p in &m.pieces {
        let k = match p.k {
            K::Stone => Kind::Stone,
            K::Obsidian => Kind::Obsidian,
            K::Piston { sticky, f, ext } => Kind::Piston { sticky, f, ext },
        };
        b.push(([p.x, p.y, 0], k));
    }
    b.sort();
    let pow = World::rest_pow(&b);
    let w = World { b, pow };
    // piston id -> index in w.b
    let map: Vec<u8> = m
        .pid
        .iter()
        .map(|&pi| {
            let p = m.pieces[pi];
            w.b.iter().position(|(q, k)| *q == [p.x, p.y, 0] && matches!(k, Kind::Piston { .. })).unwrap() as u8
        })
        .collect();
    (w, map)
}

pub fn ms_seq(map: &[u8], s: &[E]) -> Vec<Ev> {
    s.iter().map(|e| Ev { id: map[e.id as usize], act: e.act }).collect()
}

pub struct Opts {
    pub boxes: Vec<(i8, i8)>,
    pub pmax: usize,
    pub cap: usize,
    pub sym_cap: usize,
    pub threads: usize,
    pub lifts: bool,
    pub pushes: bool,
    pub out_dir: std::path::PathBuf,
    pub verify: bool,
    pub workers: usize,
    pub extra: u32,
    pub gap: u32,
    pub limit: usize,
    /// Non-core perimeter blocks that may take a 2-move path.
    pub two: usize,
    /// Also search with level events (A*) when the pulse model fails.
    pub levels: bool,
}

/// `--check <design>`: the mechanism of a design (wiring dropped) solved in its own bounding
/// box grown by `margin`: open/close schedules, symmetric train, extent.
pub fn check(path: &std::path::Path, margin: i8, cap: usize, sym_cap: usize) -> Result<()> {
    let d = crate::cli::load_design(path)?;
    let w = mechsynth::world_of_design(&d)?;
    let mut pieces = Vec::new();
    for (p, k) in &w.b {
        if p[2] != 0 {
            anyhow::bail!("not flat: block at {p:?}");
        }
        let k = match k {
            Kind::Door => continue,
            // plain blocks of a wired design are its wiring (floors, droppers, lamps): dropped
            Kind::Stone | Kind::Obsidian => continue,
            Kind::Piston { sticky, f, ext } => K::Piston { sticky: *sticky, f: *f, ext: *ext },
            k => anyhow::bail!("unsupported block {k:?}"),
        };
        pieces.push(Pc { x: p[0], y: p[1], k });
    }
    let (mut x0, mut y0, mut x1, mut y1) = (0i8, 0i8, 2i8, 2i8);
    for p in &pieces {
        for (x, y) in p.cells() {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let bx = Bx { x0: x0 - margin, y0: y0 - margin, w: x1 - x0 + 1 + 2 * margin, h: y1 - y0 + 1 + 2 * margin };
    let (m, rest) = Mech::new(bx, &pieces).context("pieces overlap")?;
    println!("{} pieces: {}", m.pieces.len(), m.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "));
    println!("box {}x{} (x {}..{}, y {}..{}), rest ring clean: {}", bx.w, bx.h, bx.x0, bx.x0 + bx.w - 1, bx.y0, bx.y0 + bx.h - 1, m.rest_ring_clean(&rest));
    match solve(&m, &rest, cap, true) {
        Ok(Some(sol)) => {
            println!("open {} + close {} ({} states), extent x {}..{} y {}..{} = {}x{}", sol.open.len(), sol.close.len(), sol.states, sol.extent.0, sol.extent.2, sol.extent.1, sol.extent.3, sol.extent.2 - sol.extent.0 + 1, sol.extent.3 - sol.extent.1 + 1);
            println!("  open:  {}\n  close: {}", m.fmt_seq(&sol.open), m.fmt_seq(&sol.close));
            if std::env::var("FAMILIES_TRACE").is_ok() {
                let mut c = rest.clone();
                println!("{}", m.render(&c));
                for e in sol.open.iter().chain(sol.close.iter()) {
                    if let Out::Ok(n) = m.apply(&c, *e) {
                        c = n;
                    }
                    println!("after {}:\n{}", m.fmt_ev(*e), m.render(&c));
                }
            }
            match best_sym(&m, &rest, &sol, sym_cap, 6) {
                Some((s, lv)) => println!("  sym {}{}: {}", s.len(), if lv { " (levels)" } else { " (pulses only)" }, m.fmt_seq(&s)),
                None => println!("  no symmetric train"),
            }
        }
        Ok(None) => println!("does not open (FULL seamless) and close in the macro model"),
        Err(()) => println!("state cap hit"),
    }
    Ok(())
}

pub fn parse_boxes(s: &str) -> Result<Vec<(i8, i8)>> {
    s.split(',')
        .map(|t| {
            let (a, b) = t.trim().split_once('x').context("box WxH")?;
            Ok((a.parse()?, b.parse()?))
        })
        .collect()
}

/// Every door offset of a W x H box (door coordinates of the box).
fn offsets(w: i8, h: i8) -> Vec<Bx> {
    let mut v = Vec::new();
    for dx in 0..=(w - 3) {
        for dy in 0..=(h - 3) {
            v.push(Bx { x0: -dx, y0: -dy, w, h });
        }
    }
    v
}

fn minimal(m_pieces: &[Pc], bx: Bx, cap: usize, levels: bool) -> Option<Vec<Pc>> {
    // drop pieces greedily while the rest still opens and closes
    let mut cur = m_pieces.to_vec();
    loop {
        let mut dropped = false;
        for i in 0..cur.len() {
            let mut t = cur.clone();
            t.remove(i);
            let Some((m, rest)) = Mech::new(bx, &t) else { continue };
            if let Ok(Some(_)) = solve(&m, &rest, cap, levels) {
                cur = t;
                dropped = true;
                break;
            }
        }
        if !dropped {
            return Some(cur);
        }
    }
}

pub fn run(version: &str, o: &Opts) -> Result<()> {
    let log = |s: &str| eprintln!("[families] {s}");
    std::fs::create_dir_all(&o.out_dir)?;
    let mut entries: BTreeMap<Vec<Pc>, Entry> = BTreeMap::new();
    let mut per_box: Vec<(i8, i8, usize, usize, usize, bool)> = Vec::new(); // (w, h, candidates, fits, new, complete)
    let mut boxes = o.boxes.clone();
    boxes.sort_by_key(|&(w, h)| ((w as i32) * (h as i32), w.max(h), w));
    for &(w, h) in &boxes {
        let t0 = std::time::Instant::now();
        let mut cands: Vec<(Bx, Vec<Pc>)> = Vec::new();
        let mut seen_c: HashSet<Vec<Pc>> = HashSet::new();
        for bx in offsets(w, h) {
            // mirror: only door offsets with dx <= mirrored dx (others are mirror images)
            let dx = -bx.x0;
            if dx > (w - 3) - dx {
                continue;
            }
            let g = generate(bx, o.pmax, o.lifts, o.pushes, o.two);
            for v in g {
                let c = canon(&v);
                if seen_c.insert(c) {
                    cands.push((bx, v));
                }
            }
        }
        if o.limit > 0 && cands.len() > o.limit {
            cands.truncate(o.limit);
        }
        log(&format!("{w}x{h}: {} candidate mechanisms ({:.1}s)", cands.len(), t0.elapsed().as_secs_f64()));
        let found: Mutex<Vec<(Bx, Vec<Pc>)>> = Mutex::new(Vec::new());
        let capped = AtomicUsize::new(0);
        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cands.len() {
                        break;
                    }
                    let (bx, v) = &cands[i];
                    // the full box (any offset) for solving
                    if let Some((m, rest)) = Mech::new(*bx, v) {
                        match solve(&m, &rest, o.cap, o.levels) {
                            Ok(Some(_)) => found.lock().unwrap().push((*bx, v.clone())),
                            Ok(None) => {}
                            Err(()) => {
                                capped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d % 20000 == 0 {
                        eprintln!("[families]   {w}x{h}: {d}/{} solved, {} work, {:.0}s", cands.len(), found.lock().unwrap().len(), t0.elapsed().as_secs_f64());
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        log(&format!("{w}x{h}: {} open+close ({} hit the state cap), minimizing", found.len(), capped.load(Ordering::Relaxed)));
        // minimal + canonical + every offset of this box that fits
        let mins: Mutex<HashSet<Vec<Pc>>> = Mutex::new(HashSet::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= found.len() {
                        break;
                    }
                    let (bx, v) = &found[i];
                    if let Some(mv) = minimal(v, *bx, o.cap, o.levels) {
                        mins.lock().unwrap().insert(canon(&mv));
                    }
                });
            }
        });
        let mins: Vec<Vec<Pc>> = {
            let mut v: Vec<Vec<Pc>> = mins.into_inner().unwrap().into_iter().collect();
            v.sort();
            v
        };
        // describe each minimal mechanism (in this box: first offset that works)
        let described: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= mins.len() {
                        break;
                    }
                    let v = &mins[i];
                    for vv in [v.clone(), v.iter().map(|p| p.mirror()).collect::<Vec<_>>()] {
                        let mut got = None;
                        for bx in offsets(w, h) {
                            let Some((m, rest)) = Mech::new(bx, &vv) else { continue };
                            if let Ok(Some(mut sol)) = solve(&m, &rest, o.cap, o.levels) {
                                if let Some((s, lv)) = best_sym(&m, &rest, &sol, o.sym_cap, 6) {
                                    sol.sym = Some(s);
                                    sol.sym_levels = lv;
                                }
                                let (free, rq) = wireability(&m, &rest, &sol);
                                let np = n_pistons(&vv);
                                let sticky = vv.iter().filter(|p| matches!(p.k, K::Piston { sticky: true, .. })).count();
                                got = Some(Entry { pieces: v.clone(), w, h, bx, sol, rest_clean: m.rest_ring_clean(&rest), pistons: np, sticky, free_cells: free, ring_quasi: rq, verified: None, orient: vv.clone() });
                                break;
                            }
                        }
                        if let Some(e) = got {
                            described.lock().unwrap().push(e);
                            break;
                        }
                    }
                });
            }
        });
        let described = described.into_inner().unwrap();
        let fits = described.len();
        let mut new = 0;
        for e in described {
            if !entries.contains_key(&e.pieces) {
                new += 1;
                entries.insert(e.pieces.clone(), e);
            }
        }
        per_box.push((w, h, cands.len(), fits, new, capped.load(Ordering::Relaxed) == 0));
        log(&format!("{w}x{h}: {fits} minimal mechanisms fit ({new} new) in {:.0}s", t0.elapsed().as_secs_f64()));
        write_catalog(o, &entries, &per_box)?;
    }
    if o.verify {
        verify_entries(version, o, &mut entries)?;
        write_catalog(o, &entries, &per_box)?;
    }
    Ok(())
}

fn verify_entries(version: &str, o: &Opts, entries: &mut BTreeMap<Vec<Pc>, Entry>) -> Result<()> {
    let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
    let pool = mctas::sim::Pool::spawn(o.workers, &cfg)?;
    let keys: Vec<Vec<Pc>> = entries.keys().cloned().collect();
    let ideal_dir = o.out_dir.join("ideal");
    std::fs::create_dir_all(&ideal_dir)?;
    let items: Vec<(usize, bool)> = keys.iter().enumerate().flat_map(|(i, k)| {
        let mut v = vec![(i, false)];
        if entries[k].sol.sym.is_some() {
            v.push((i, true));
        }
        v
    }).collect();
    let ents: Vec<Entry> = keys.iter().map(|k| entries[k].clone()).collect();
    let res = pool.map(items.clone(), |wk, _, (i, sym)| {
        let e = &ents[i];
        let (m, _) = Mech::new(e.bx, &e.orient).context("rebuild")?;
        let (w, map) = to_world(&m);
        let sol = mechsynth::Solution {
            open: ms_seq(&map, &e.sol.open),
            close: ms_seq(&map, &e.sol.close),
            sym: e.sol.sym.as_ref().map(|s| ms_seq(&map, s)),
            states: 0,
        };
        let name = format!("{}x{}-{:04}", e.w, e.h, i);
        let plan = mechsynth::ideal_plan(&w, &sol, sym, o.gap, &name)?;
        // keep the files
        let tag = if sym { "sym" } else { "oc" };
        let mut dd = plan.design.clone();
        dd.name = format!("{name}-{tag}");
        let dpath = ideal_dir.join(format!("{name}-{tag}.txt"));
        dd.save(&dpath)?;
        let text = plan.text.replace(&format!("design {name}.txt"), &format!("design {}", dpath.display()));
        std::fs::write(ideal_dir.join(format!("{name}-{tag}.ideal")), text)?;
        Ok(mechsynth::verify_plan(wk, &plan, o.extra).map_err(|e| format!("{e:#}")))
    });
    let mut verdicts: HashMap<usize, Vec<String>> = HashMap::new();
    for ((i, sym), r) in items.iter().zip(res) {
        let tag = if *sym { "sym" } else { "oc" };
        let v = match r {
            Ok(Ok(())) => format!("{tag} WORKS"),
            Ok(Err(e)) => format!("{tag} BROKEN ({})", e.chars().take(80).collect::<String>().replace('\t', " ").replace('|', "/")),
            Err(e) => format!("{tag} ERROR ({})", format!("{e:#}").chars().take(80).collect::<String>().replace('|', "/")),
        };
        verdicts.entry(*i).or_default().push(v);
    }
    for (i, k) in keys.iter().enumerate() {
        if let Some(v) = verdicts.get(&i) {
            entries.get_mut(k).unwrap().verified = Some(v.join("; "));
        }
    }
    Ok(())
}

fn write_catalog(o: &Opts, entries: &BTreeMap<Vec<Pc>, Entry>, per_box: &[(i8, i8, usize, usize, usize, bool)]) -> Result<()> {
    let mut list: Vec<&Entry> = entries.values().collect();
    list.sort_by_key(|e| ((e.w as i32) * (e.h as i32), e.w.max(e.h), e.w, e.pistons, e.sol.sym.as_ref().map_or(99, |s| s.len()), e.sol.open.len() + e.sol.close.len()));
    let mut md = String::new();
    md.push_str("# Flat (1-wide) seamless 3x3 door mechanisms: the catalog per box\n\n");
    md.push_str("Generated by `mctas door families` (crates/piston-door/src/families.rs). Do not edit by hand.\n\n");
    md.push_str(&format!(
        "Bounds: every block in the door plane; pistons sticky or normal facing E/W/U/D (retracted or extended at rest), at most **{} pistons**, no helpers, no slime/honey; every door block leaves the 3x3 in 1-2 unit moves (the center in exactly 2), each move a pull by a sticky piston 2 cells ahead or a push by a piston 1-2 cells behind, a piston reused from its rest cell or lifted 1 cell along its axis by one lift piston (the elevator){}{}. Each candidate is solved EXACTLY in the macro piston model (whole state graph, cap {} states): FULL seamless open state (3x3 empty, only door blocks / plain blocks in the 12 ring cells, nothing but air or a door block behind a swept ring cell that is empty when open), closing back to the exact rest state, everything inside the box at every state. Only MINIMAL mechanisms (no piston can be dropped), left-right mirror images merged (up/down is not a symmetry: quasi-connectivity and gravity).\n\n",
        o.pmax,
        if o.lifts { "" } else { " (lifts off)" },
        if o.pushes { "" } else { " (pushes off)" },
        o.cap
    ));
    md.push_str("Columns: box (W x H, door offset = columns left of / rows below the door), pistons (sticky), rest-ring = no piston/head in the ring at rest (the strict reading), sym = one pulse train for both lever edges (P = 2 gt pulse, L = level toggle; `+L` = needs levels), free = box cells the mechanism never touches (room for wiring), rq = pistons whose quasi-connectivity cell (above) is in the ring or the 3x3, sim = `door ideal` replay with ideal inputs (oc = separate open/close schedules, sym = the shared train).\n\n");
    md.push_str("| box | candidates | minimal mechanisms that fit | first fitting here | search complete |\n|---|---|---|---|---|\n");
    for (w, h, c, f, n, comp) in per_box {
        md.push_str(&format!("| {w}x{h} | {c} | {f} | {n} | {} |\n", if *comp { "yes" } else { "no (state cap hit)" }));
    }
    md.push('\n');
    let mut tsv = String::from("id\tbox\tdoor_offset\tpistons\tsticky\trest_ring_clean\tsym_len\tsym_levels\topen_len\tclose_len\tfree_cells\tring_quasi\textent\tpieces\topen\tclose\tsym\tsim\n");
    let mut cur_box = (0, 0);
    for (i, e) in list.iter().enumerate() {
        if (e.w, e.h) != cur_box {
            cur_box = (e.w, e.h);
            md.push_str(&format!("\n## {}x{} (volume {})\n\n| id | offset | pistons | rest-ring | sym | open/close | free | rq | pieces | sim |\n|---|---|---|---|---|---|---|---|---|---|\n", e.w, e.h, e.w as i32 * e.h as i32));
        }
        let (m, _) = Mech::new(e.bx, &e.orient).unwrap();
        let sym = e.sol.sym.as_ref().map(|s| format!("{}{}: {}", s.len(), if e.sol.sym_levels { "+L" } else { "" }, m.fmt_seq(s))).unwrap_or("-".into());
        let pieces = m.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" ");
        let id = format!("{}x{}-{:03}", e.w, e.h, i);
        md.push_str(&format!(
            "| {id} | {},{} | {} ({}) | {} | {} | {}/{} | {} | {} | `{}` | {} |\n",
            -e.bx.x0,
            -e.bx.y0,
            e.pistons,
            e.sticky,
            if e.rest_clean { "clean" } else { "piston" },
            sym,
            e.sol.open.len(),
            e.sol.close.len(),
            e.free_cells,
            e.ring_quasi,
            pieces,
            e.verified.clone().unwrap_or("-".into())
        ));
        tsv.push_str(&format!(
            "{id}\t{}x{}\t{},{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\n",
            e.w,
            e.h,
            -e.bx.x0,
            -e.bx.y0,
            e.pistons,
            e.sticky,
            e.rest_clean,
            e.sol.sym.as_ref().map_or(0, |s| s.len()),
            e.sol.sym_levels,
            e.sol.open.len(),
            e.sol.close.len(),
            e.free_cells,
            e.ring_quasi,
            e.sol.extent,
            pieces,
            m.fmt_seq(&e.sol.open),
            m.fmt_seq(&e.sol.close),
            e.sol.sym.as_ref().map(|s| m.fmt_seq(s)).unwrap_or_default(),
            e.verified.clone().unwrap_or_default()
        ));
    }
    std::fs::write(o.out_dir.join("CATALOG.md"), md)?;
    std::fs::write(o.out_dir.join("CATALOG.tsv"), tsv)?;
    Ok(())
}
