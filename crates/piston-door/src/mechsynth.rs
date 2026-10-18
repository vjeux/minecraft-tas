//! `mctas door mech-synth`: mechanism synthesis for the 3x3 door.
//!
//! A *macro* model of piston physics (a port of `PistonStructureResolver` and the sticky
//! retract rules of `PistonBaseBlock`, read in the 26.3 source) moves blocks one settled piston
//! action at a time. A mechanism is the door (9 stone at x 0..2, y 0..2, z 0) plus pistons,
//! slime/honey, obsidian, glazed terracotta and plain blocks. Every piston is driven by an ideal
//! signal; the actions are `L` (toggle its level: extend / retract with the sticky pull) and `P`
//! (a 2 gt pulse from rest: push and spit, or extend into a gap and pull the block beyond it).
//!
//! For each mechanism the whole state graph reachable from rest is built (capped), and the
//! cheapest open path (door + walkway clear) plus close path (back to the exact rest state) is
//! read off it. `--symmetric` also looks for ONE event sequence that opens from rest and closes
//! from the open state (the same delays on both lever edges: the cheapest control).
//!
//! Enumeration: a base design (e.g. the door with its side columns and their drivers) plus up
//! to `--add k` blocks from a candidate list (pistons of 4 planar facings, retracted or extended,
//! whose push/pull reach touches the mechanism or the ring of cells around the door; slime next
//! to the mechanism; optional extra kinds). Survivors are ranked by (blocks, control events) and
//! written as ideal-input designs + schedules that `door ideal` replays in the real simulator
//! (`--verify` does it directly).

use anyhow::{Context, Result, bail};
use mctas::design::Design;
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type P = [i8; 3];
pub const D: [P; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const DNAME: [&str; 6] = ["east", "west", "up", "down", "south", "north"];
const MAX_PUSH: usize = 12;

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: P, k: i8) -> P {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn opp(f: u8) -> u8 {
    f ^ 1
}
fn axis(f: u8) -> u8 {
    f / 2
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Kind {
    Door,
    Slime,
    Honey,
    Obsidian,
    /// Glazed terracotta: pushed along the line only, never pulled or dragged.
    Gt,
    /// A plain movable block that is not a door block.
    Stone,
    Piston { sticky: bool, f: u8, ext: bool },
}

impl Kind {
    fn sticky_block(self) -> bool {
        matches!(self, Kind::Slime | Kind::Honey)
    }
    pub fn code(self) -> String {
        match self {
            Kind::Door => "door".into(),
            Kind::Slime => "slime".into(),
            Kind::Honey => "honey".into(),
            Kind::Obsidian => "obsidian".into(),
            Kind::Gt => "gt".into(),
            Kind::Stone => "stone".into(),
            Kind::Piston { sticky, f, ext } => format!("{}{}{}", if sticky { "S" } else { "N" }, DNAME[f as usize], if ext { "+" } else { "" }),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Occ {
    Air,
    Block(usize),
    Head(usize),
}

/// One state: blocks with fixed identities, plus the level signal of each piston (bit = index).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct World {
    pub b: Vec<(P, Kind)>,
    pub pow: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Act {
    /// Toggle the level signal.
    L,
    /// A 2 gt pulse (only from unpowered + retracted).
    P,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Ev {
    pub id: u8,
    pub act: Act,
}

pub fn in_window(p: P) -> bool {
    (0..=2).contains(&p[0]) && (0..=2).contains(&p[1]) && p[2] == 0
}
pub fn in_corridor(p: P) -> bool {
    (0..=2).contains(&p[0]) && (0..=2).contains(&p[1]) && (-3..=3).contains(&p[2])
}

impl World {
    fn occ(&self, p: P) -> Occ {
        for (i, (q, k)) in self.b.iter().enumerate() {
            if *q == p {
                return Occ::Block(i);
            }
            if let Kind::Piston { ext: true, f, .. } = k {
                if add(*q, D[*f as usize]) == p {
                    return Occ::Head(i);
                }
            }
        }
        Occ::Air
    }
    fn kind_at(&self, p: P) -> Option<Kind> {
        match self.occ(p) {
            Occ::Block(i) => Some(self.b[i].1),
            _ => None,
        }
    }

    /// `PistonBaseBlock.isPushable` (no world border, no destroyables in our palette).
    fn pushable(&self, o: Occ, push: u8, conn: u8) -> bool {
        match o {
            Occ::Air => true,
            Occ::Head(_) => false,
            Occ::Block(i) => match self.b[i].1 {
                Kind::Obsidian => false,
                Kind::Gt => push == conn,
                Kind::Piston { ext, .. } => !ext,
                _ => true,
            },
        }
    }

    fn can_stick(a: Option<Kind>, b: Option<Kind>) -> bool {
        match (a, b) {
            (Some(Kind::Honey), Some(Kind::Slime)) | (Some(Kind::Slime), Some(Kind::Honey)) => false,
            (x, y) => x.is_some_and(|k| k.sticky_block()) || y.is_some_and(|k| k.sticky_block()),
        }
    }

    /// `PistonStructureResolver.resolve`: the positions to move, or None.
    fn resolve(&self, piston: P, f: u8, extending: bool) -> Option<Vec<P>> {
        let push = if extending { f } else { opp(f) };
        let start = if extending { add(piston, D[f as usize]) } else { add(piston, mul(D[f as usize], 2)) };
        let mut r = Resolver { w: self, piston, push, to_push: Vec::new() };
        let o = self.occ(start);
        if !self.pushable(o, push, f) {
            return None;
        }
        if !r.add_line(start, push) {
            return None;
        }
        let mut i = 0;
        while i < r.to_push.len() {
            let p = r.to_push[i];
            if self.kind_at(p).is_some_and(|k| k.sticky_block()) && !r.branch(p) {
                return None;
            }
            i += 1;
        }
        Some(r.to_push)
    }

    fn move_all(&mut self, list: &[P], dir: u8) {
        let ids: Vec<usize> = list.iter().filter_map(|p| match self.occ(*p) {
            Occ::Block(i) => Some(i),
            _ => None,
        }).collect();
        for i in ids {
            self.b[i].0 = add(self.b[i].0, D[dir as usize]);
        }
    }

    fn set_ext(&mut self, i: usize, e: bool) {
        if let Kind::Piston { sticky, f, .. } = self.b[i].1 {
            self.b[i].1 = Kind::Piston { sticky, f, ext: e };
        }
    }

    /// Sticky retract of piston i (its head already gone): pull the block 2 in front.
    fn pull(&mut self, i: usize) {
        let (pos, k) = self.b[i];
        let Kind::Piston { sticky: true, f, .. } = k else { return };
        let two = add(pos, mul(D[f as usize], 2));
        let o = self.occ(two);
        let Occ::Block(j) = o else { return };
        if !self.pushable(o, opp(f), f) {
            return;
        }
        if matches!(self.b[j].1, Kind::Gt | Kind::Obsidian) {
            return;
        }
        if let Some(list) = self.resolve(pos, f, false) {
            self.move_all(&list, opp(f));
        }
    }

    /// Apply an event; None when nothing changes (a no-op).
    pub fn apply(&self, e: Ev) -> Option<World> {
        if NO_RING_FIRE.load(Ordering::Relaxed) && in_s(self.b[e.id as usize].0) {
            return None;
        }
        let w = self.apply_mode(e, false)?;
        if SEAMLESS.load(Ordering::Relaxed) && s_off_plane(&w) {
            return None;
        }
        Some(w)
    }

    /// `strict`: a level-on that cannot extend is refused (no powered-but-blocked states).
    pub fn apply_mode(&self, e: Ev, strict: bool) -> Option<World> {
        let i = e.id as usize;
        let (pos, k) = self.b[i];
        let Kind::Piston { sticky, f, ext } = k else { return None };
        let bit = 1u64 << i;
        let powered = self.pow & bit != 0;
        let mut w = self.clone();
        match e.act {
            Act::L => {
                w.pow ^= bit;
                if powered {
                    if ext {
                        w.set_ext(i, false);
                        if sticky {
                            w.pull(i);
                        }
                    }
                } else if !ext {
                    if let Some(list) = self.resolve(pos, f, true) {
                        w.move_all(&list, f);
                        w.set_ext(i, true);
                    } else if strict {
                        return None;
                    }
                }
                Some(w)
            }
            Act::P => {
                if powered || ext {
                    return None;
                }
                let list = self.resolve(pos, f, true)?;
                if !list.is_empty() {
                    w.move_all(&list, f);
                    // spit: blocks stay, piston retracted
                    return Some(w);
                }
                if !sticky {
                    return None;
                }
                w.pull(i);
                if w == *self { None } else { Some(w) }
            }
        }
    }

    pub fn is_open(&self) -> bool {
        let seamless = SEAMLESS.load(Ordering::Relaxed);
        for (p, k) in &self.b {
            if in_corridor(*p) {
                return false;
            }
            if seamless && in_s(*p) && (*k != Kind::Door || p[2] != 0) {
                return false;
            }
            if let Kind::Piston { ext: true, f, .. } = k {
                if in_corridor(add(*p, D[*f as usize])) {
                    return false;
                }
                if seamless && in_s(add(*p, D[*f as usize])) {
                    return false;
                }
            }
        }
        if SEAMLESS_OPEN.load(Ordering::Relaxed) {
            return self.ring_clean();
        }
        true
    }

    /// `--seamless-open`: the open state shows no circuitry in the door plane's hallway ring (the
    /// 12 cells face-adjacent to the opening at z = 0): each holds a door block, a plain stone
    /// (quartz-like) or air, no piston head; an air ring cell has no circuitry right behind it
    /// (one further out: it may be a hole); no slime or honey in the ring (it would drag the
    /// hallway walls at z = +-1). FULL seamless for a flat door, conservatively.
    pub fn ring_clean(&self) -> bool {
        for (r, out) in ring12() {
            match self.occ(r) {
                Occ::Head(_) => return false,
                Occ::Block(i) => {
                    if !matches!(self.b[i].1, Kind::Door | Kind::Stone) {
                        return false;
                    }
                }
                Occ::Air => match self.occ(out) {
                    Occ::Head(_) => return false,
                    Occ::Block(i) => {
                        if !matches!(self.b[i].1, Kind::Door | Kind::Stone) {
                            return false;
                        }
                    }
                    Occ::Air => {}
                },
            }
        }
        true
    }

    pub fn pistons(&self) -> Vec<usize> {
        self.b.iter().enumerate().filter(|(_, (_, k))| matches!(k, Kind::Piston { .. })).map(|(i, _)| i).collect()
    }

    /// Mechanism blocks (heads are not blocks).
    pub fn blocks(&self) -> usize {
        self.b.len()
    }

    pub fn rest_pow(b: &[(P, Kind)]) -> u64 {
        let mut pow = 0;
        for (i, (_, k)) in b.iter().enumerate() {
            if let Kind::Piston { ext: true, .. } = k {
                pow |= 1 << i;
            }
        }
        pow
    }
}

struct Resolver<'a> {
    w: &'a World,
    piston: P,
    push: u8,
    to_push: Vec<P>,
}

impl Resolver<'_> {
    fn add_line(&mut self, start: P, dir: u8) -> bool {
        let w = self.w;
        let o = w.occ(start);
        if o == Occ::Air {
            return true;
        }
        if !w.pushable(o, self.push, dir) {
            return true;
        }
        if start == self.piston || self.to_push.contains(&start) {
            return true;
        }
        let mut count: i8 = 1;
        if count as usize + self.to_push.len() > MAX_PUSH {
            return false;
        }
        let back = opp(self.push);
        let mut prev = w.kind_at(start);
        while prev.is_some_and(|k| k.sticky_block()) {
            let p = add(start, mul(D[back as usize], count));
            let no = w.occ(p);
            let nk = w.kind_at(p);
            if no == Occ::Air || !World::can_stick(prev, nk) || !w.pushable(no, self.push, back) || p == self.piston {
                break;
            }
            count += 1;
            if count as usize + self.to_push.len() > MAX_PUSH {
                return false;
            }
            prev = nk;
        }
        let mut added = 0usize;
        for i in (0..count).rev() {
            self.to_push.push(add(start, mul(D[back as usize], i)));
            added += 1;
        }
        let mut i: i8 = 1;
        loop {
            let p = add(start, mul(D[self.push as usize], i));
            if let Some(cp) = self.to_push.iter().position(|q| *q == p) {
                self.reorder(added, cp);
                for j in 0..=(cp + added).min(self.to_push.len() - 1) {
                    let q = self.to_push[j];
                    if w.kind_at(q).is_some_and(|k| k.sticky_block()) && !self.branch(q) {
                        return false;
                    }
                }
                return true;
            }
            let o = w.occ(p);
            if o == Occ::Air {
                return true;
            }
            if !w.pushable(o, self.push, self.push) || p == self.piston {
                return false;
            }
            if self.to_push.len() >= MAX_PUSH {
                return false;
            }
            self.to_push.push(p);
            added += 1;
            i += 1;
        }
    }

    fn reorder(&mut self, added: usize, cp: usize) {
        let n = self.to_push.len();
        let head: Vec<P> = self.to_push[..cp].to_vec();
        let last: Vec<P> = self.to_push[n - added..].to_vec();
        let mid: Vec<P> = self.to_push[cp..n - added].to_vec();
        self.to_push = head;
        self.to_push.extend(last);
        self.to_push.extend(mid);
    }

    fn branch(&mut self, from: P) -> bool {
        let fk = self.w.kind_at(from);
        for d in 0..6u8 {
            if axis(d) == axis(self.push) {
                continue;
            }
            let np = add(from, D[d as usize]);
            let nk = self.w.kind_at(np);
            let head = matches!(self.w.occ(np), Occ::Head(_));
            if (nk.is_some() || head) && World::can_stick(nk.or(Some(Kind::Stone)), fk) && !self.add_line(np, d) {
                return false;
            }
        }
        true
    }
}

/// The reachable state graph of a mechanism.
pub struct Graph {
    pub states: Vec<World>,
    pub edges: Vec<Vec<(u32, Ev)>>,
    pub depth: Vec<u32>,
    pub parent: Vec<(u32, Option<Ev>)>,
    pub complete: bool,
}

/// When set, pistons are pulse-driven only (no level events): the cheapest control, one observer
/// chain from the lever (`--pulses-only`).
pub static PULSES_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--seamless-open`: open states must also pass `World::ring_clean`.
pub static SEAMLESS_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// Where the split symmetric search puts the "edge" pulses: 0 both, 1 start, 2 end (`--edges`).
pub static EDGES: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
/// `--split-only`: skip the graph symmetric search (use the edge/core split search).
pub static SPLIT_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--seamless`: FULL seamless + hallway in the macro model. S = the cells face-adjacent to the
/// tunnel (the 3x3 columns through the opening, any depth). No block ever enters S off the door
/// plane (the hallway's walls stand there), candidates are not placed there, and the open state
/// holds nothing but door blocks in S (door plane included).
pub static SEAMLESS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--no-ring-fire`: a piston standing in S cannot be fired (a bottom-row piston in the ring has
/// no power cell outside S: its sides are S, below is the floor).
pub static NO_RING_FIRE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// In the hallway surface S (face-adjacent to the tunnel, any depth).
pub fn in_s(p: P) -> bool {
    ((p[0] == -1 || p[0] == 3) && (0..=2).contains(&p[1])) || ((p[1] == -1 || p[1] == 3) && (0..=2).contains(&p[0]))
}

/// Some block (or piston head) in S off the door plane.
fn s_off_plane(w: &World) -> bool {
    w.b.iter().any(|(p, k)| {
        (in_s(*p) && p[2] != 0) || matches!(k, Kind::Piston { ext: true, f, .. } if { let h = add(*p, D[*f as usize]); in_s(h) && h[2] != 0 })
    })
}

pub fn events(w: &World) -> Vec<Ev> {
    let po = PULSES_ONLY.load(Ordering::Relaxed);
    let mut out = Vec::new();
    for i in w.pistons() {
        let ext = matches!(w.b[i].1, Kind::Piston { ext: true, .. });
        if !po || ext {
            out.push(Ev { id: i as u8, act: Act::L });
        }
        out.push(Ev { id: i as u8, act: Act::P });
    }
    out
}

pub fn build_graph(rest: &World, cap: usize) -> Graph {
    let evs = events(rest);
    let mut index: HashMap<World, u32> = HashMap::new();
    let mut g = Graph { states: vec![rest.clone()], edges: vec![Vec::new()], depth: vec![0], parent: vec![(0, None)], complete: true };
    index.insert(rest.clone(), 0);
    let mut q = VecDeque::from([0u32]);
    while let Some(s) = q.pop_front() {
        let st = g.states[s as usize].clone();
        for e in &evs {
            let Some(n) = st.apply(*e) else { continue };
            let id = match index.get(&n) {
                Some(&id) => id,
                None => {
                    if g.states.len() >= cap {
                        g.complete = false;
                        continue;
                    }
                    let id = g.states.len() as u32;
                    index.insert(n.clone(), id);
                    g.states.push(n);
                    g.edges.push(Vec::new());
                    g.depth.push(g.depth[s as usize] + 1);
                    g.parent.push((s, Some(*e)));
                    q.push_back(id);
                    id
                }
            };
            g.edges[s as usize].push((id, *e));
        }
    }
    g
}

/// Distance from every state to `target` (u32::MAX = unreachable), over the graph edges.
pub fn dist_to(g: &Graph, target: u32) -> Vec<u32> {
    let n = g.states.len();
    let mut rev: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (s, es) in g.edges.iter().enumerate() {
        for (t, _) in es {
            rev[*t as usize].push(s as u32);
        }
    }
    let mut d = vec![u32::MAX; n];
    d[target as usize] = 0;
    let mut q = VecDeque::from([target]);
    while let Some(t) = q.pop_front() {
        for &s in &rev[t as usize] {
            if d[s as usize] == u32::MAX {
                d[s as usize] = d[t as usize] + 1;
                q.push_back(s);
            }
        }
    }
    d
}

pub fn path_from_rest(g: &Graph, s: u32) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut c = s;
    while let (p, Some(e)) = g.parent[c as usize] {
        out.push(e);
        c = p;
    }
    out.reverse();
    out
}

pub fn path_to(g: &Graph, d: &[u32], from: u32) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut c = from;
    while d[c as usize] > 0 {
        let (t, e) = *g.edges[c as usize].iter().filter(|(t, _)| d[*t as usize] == d[c as usize] - 1).next().unwrap();
        out.push(e);
        c = t;
    }
    out
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub open: Vec<Ev>,
    pub close: Vec<Ev>,
    /// One sequence for both edges, if found.
    pub sym: Option<Vec<Ev>>,
    pub states: usize,
}

/// Weighted A* from `start` to states meeting `goal`; up to `want` goal states (with paths).
pub fn astar(start: &World, goal: &dyn Fn(&World) -> bool, h: &dyn Fn(&World) -> u32, cap: usize, want: usize, strict: bool) -> Vec<(World, Vec<Ev>)> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let evs = events(start);
    let mut states: Vec<World> = vec![start.clone()];
    let mut parent: Vec<(u32, Option<Ev>)> = vec![(0, None)];
    let mut g: Vec<u32> = vec![0];
    let mut index: HashMap<World, u32> = HashMap::new();
    index.insert(start.clone(), 0);
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((2 * h(start), 0u32, 0u32)));
    let mut out = Vec::new();
    let mut closed: HashSet<u32> = HashSet::new();
    while let Some(Reverse((_, gs, s))) = heap.pop() {
        if !closed.insert(s) {
            continue;
        }
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
                break;
            }
            continue;
        }
        for e in &evs {
            let Some(n) = st.apply_mode(*e, strict) else { continue };
            let ng = gs + 1;
            match index.get(&n) {
                Some(&id) => {
                    if ng < g[id as usize] {
                        g[id as usize] = ng;
                        parent[id as usize] = (s, Some(*e));
                        heap.push(Reverse((ng + 2 * h(&n), ng, id)));
                    }
                }
                None => {
                    if states.len() >= cap {
                        continue;
                    }
                    let id = states.len() as u32;
                    let f = ng + 2 * h(&n);
                    index.insert(n.clone(), id);
                    states.push(n);
                    parent.push((s, Some(*e)));
                    g.push(ng);
                    heap.push(Reverse((f, ng, id)));
                }
            }
        }
    }
    out
}

pub fn h_open(w: &World) -> u32 {
    let mut n = 0;
    for (p, k) in &w.b {
        if in_corridor(*p) {
            n += if *p == [1, 1, 0] { 2 } else { 1 };
        }
        if let Kind::Piston { ext: true, f, .. } = k {
            if in_corridor(add(*p, D[*f as usize])) {
                n += 1;
            }
        }
    }
    n
}

/// A*-based solve for mechanisms whose full state graph is too big.
pub fn solve_astar(rest: &World, cap: usize) -> Option<Solution> {
    let opens = astar(rest, &|w| w.is_open(), &h_open, cap, 6, true);
    let mut best: Option<Solution> = None;
    for (o, open) in opens {
        let h_close = |w: &World| -> u32 {
            let mut n = 0;
            for (a, b) in w.b.iter().zip(rest.b.iter()) {
                if a != b {
                    n += 1;
                }
            }
            n + (w.pow ^ rest.pow).count_ones()
        };
        let back = astar(&o, &|w| w == rest, &h_close, cap, 1, true);
        if let Some((_, close)) = back.into_iter().next() {
            let better = best.as_ref().is_none_or(|b| open.len() + close.len() < b.open.len() + b.close.len());
            if better {
                best = Some(Solution { open: open.clone(), close, sym: None, states: 0 });
            }
        }
    }
    best
}

/// Symmetric schedules for big mechanisms: "edge" pistons (exactly one pulse per edge in the
/// asymmetric solution) fire as one block at the start or at the end of the sequence, the other
/// pistons' events are searched by iterative deepening (both runs simulated on the fly) against
/// candidate open states.
pub fn symmetric_split(rest: &World, sol: &Solution, max_core: usize, cap: usize) -> Option<Vec<Ev>> {
    let count = |s: &[Ev], id: u8, a: Act| s.iter().filter(|e| e.id == id && e.act == a).count();
    let mut edge = Vec::new();
    let mut core = Vec::new();
    for i in rest.pistons() {
        let id = i as u8;
        let used = count(&sol.open, id, Act::P) + count(&sol.open, id, Act::L) + count(&sol.close, id, Act::P) + count(&sol.close, id, Act::L);
        if count(&sol.open, id, Act::P) == 1 && count(&sol.close, id, Act::P) == 1 && used == 2 && rest.apply(Ev { id, act: Act::P }).is_some() {
            edge.push(Ev { id, act: Act::P });
        } else if used > 0 {
            if !PULSES_ONLY.load(Ordering::Relaxed) {
                core.push(Ev { id, act: Act::L });
            }
            core.push(Ev { id, act: Act::P });
        }
    }
    let mut opens: Vec<World> = astar(rest, &|w| w.is_open(), &h_open, cap, 12, false).into_iter().map(|(w, _)| w).collect();
    let mut o0 = rest.clone();
    for e in &sol.open {
        o0 = o0.apply(*e).unwrap_or(o0);
    }
    opens.insert(0, o0);
    let run = |w: &World, s: &[Ev]| -> World {
        let mut c = w.clone();
        for e in s {
            c = c.apply(*e).unwrap_or(c);
        }
        c
    };
    for o in &opens {
        let places: Vec<bool> = match EDGES.load(Ordering::Relaxed) {
            1 => vec![true],
            2 => vec![false],
            _ => vec![true, false],
        };
        for at_start in places {
            let (x0, y0) = if at_start { (run(rest, &edge), run(o, &edge)) } else { (rest.clone(), o.clone()) };
            for len in 1..=max_core {
                let mut seq = Vec::new();
                let fin = |x: &World, y: &World| -> bool {
                    if at_start { x == o && y == rest } else { run(x, &edge) == *o && run(y, &edge) == *rest }
                };
                if split_dfs(&core, &x0, &y0, len, &mut seq, &fin) {
                    let mut full = Vec::new();
                    if at_start {
                        full.extend(edge.iter().copied());
                        full.extend(seq);
                    } else {
                        full.extend(seq);
                        full.extend(edge.iter().copied());
                    }
                    return Some(full);
                }
            }
        }
    }
    None
}

fn split_dfs(evs: &[Ev], x: &World, y: &World, left: usize, seq: &mut Vec<Ev>, fin: &dyn Fn(&World, &World) -> bool) -> bool {
    if left == 0 {
        return fin(x, y);
    }
    for e in evs {
        let nx = x.apply(*e);
        let ny = y.apply(*e);
        if nx.is_none() && ny.is_none() {
            continue;
        }
        let nx = nx.unwrap_or_else(|| x.clone());
        let ny = ny.unwrap_or_else(|| y.clone());
        seq.push(*e);
        if split_dfs(evs, &nx, &ny, left - 1, seq, fin) {
            return true;
        }
        seq.pop();
    }
    false
}

/// Quick screen: BFS (strict, capped) from rest to the first open states, then from each of
/// them back to rest. True when some open state can close.
pub fn screen(rest: &World, cap: usize) -> bool {
    let evs = events(rest);
    let bfs = |start: &World, goal: &dyn Fn(&World) -> bool, stop_all: bool| -> Vec<World> {
        let mut seen: HashSet<World> = HashSet::new();
        seen.insert(start.clone());
        let mut layer = vec![start.clone()];
        let mut hits = Vec::new();
        while !layer.is_empty() && seen.len() < cap {
            let mut next = Vec::new();
            for s in &layer {
                for e in &evs {
                    let Some(n) = s.apply_mode(*e, true) else { continue };
                    if seen.contains(&n) {
                        continue;
                    }
                    if goal(&n) {
                        hits.push(n.clone());
                        if !stop_all {
                            return hits;
                        }
                    }
                    seen.insert(n.clone());
                    next.push(n);
                }
            }
            if !hits.is_empty() {
                return hits;
            }
            layer = next;
        }
        hits
    };
    let opens = bfs(rest, &|w: &World| w.is_open(), true);
    for o in opens.iter().take(4) {
        if !bfs(o, &|w: &World| w == rest, false).is_empty() {
            return true;
        }
    }
    false
}

/// Cheapest open + close paths (and optionally a symmetric sequence).
pub fn solve(rest: &World, cap: usize, sym_len: usize, sym_tries: usize) -> Option<Solution> {
    let g = build_graph(rest, cap);
    let opens: Vec<u32> = (0..g.states.len() as u32).filter(|&s| g.states[s as usize].is_open()).collect();
    if opens.is_empty() {
        return None;
    }
    let d2r = dist_to(&g, 0);
    let mut ranked: Vec<(u32, u32)> = opens.iter().filter(|&&o| d2r[o as usize] != u32::MAX).map(|&o| (g.depth[o as usize] + d2r[o as usize], o)).collect();
    if ranked.is_empty() {
        return None;
    }
    ranked.sort();
    let best = ranked[0].1;
    let mut sol = Solution { open: path_from_rest(&g, best), close: path_to(&g, &d2r, best), sym: None, states: g.states.len() };
    if sym_len > 0 {
        sol.sym = symmetric(&g, &d2r, &ranked, sym_len, sym_tries);
    }
    Some(sol)
}

/// One event sequence S with S(rest) = O (open) and S(O) = rest, for the first `tries` open states.
fn symmetric(g: &Graph, d2r: &[u32], ranked: &[(u32, u32)], max_len: usize, tries: usize) -> Option<Vec<Ev>> {
    let index: HashMap<&World, u32> = g.states.iter().enumerate().map(|(i, s)| (s, i as u32)).collect();
    let evs = events(&g.states[0]);
    let mut best: Option<Vec<Ev>> = None;
    for &(_, o) in ranked.iter().take(tries) {
        let d2o = dist_to(g, o);
        for len in 1..=max_len {
            if best.as_ref().is_some_and(|b| b.len() <= len) {
                break;
            }
            let mut seq = Vec::new();
            if sym_dfs(g, &index, &evs, d2r, &d2o, 0, o, o, len, &mut seq) {
                best = Some(seq);
                break;
            }
        }
    }
    best
}

#[allow(clippy::too_many_arguments)]
fn sym_dfs(g: &Graph, index: &HashMap<&World, u32>, evs: &[Ev], d2r: &[u32], d2o: &[u32], x: u32, y: u32, o: u32, left: usize, seq: &mut Vec<Ev>) -> bool {
    if left == 0 {
        return x == o && y == 0;
    }
    for e in evs {
        let step = |s: u32| -> Option<u32> {
            match g.states[s as usize].apply(*e) {
                None => Some(s),
                Some(n) => index.get(&n).copied(),
            }
        };
        let (Some(nx), Some(ny)) = (step(x), step(y)) else { continue };
        if nx == x && ny == y {
            continue;
        }
        let r = (left - 1) as u32;
        if d2o[nx as usize] > r || d2r[ny as usize] > r {
            continue;
        }
        seq.push(*e);
        if sym_dfs(g, index, evs, d2r, d2o, nx, ny, o, left - 1, seq) {
            return true;
        }
        seq.pop();
    }
    false
}

// ---------------------------------------------------------------- designs <-> worlds

pub fn kind_of_state(s: &str) -> Option<Kind> {
    let name = mctas::blockstate::block_name(s);
    let n = name.trim_start_matches("minecraft:");
    let prop = |k: &str| -> Option<String> {
        let props = s.split_once('[')?.1.trim_end_matches(']');
        props.split(',').find_map(|kv| kv.split_once('=').filter(|(a, _)| a.trim() == k).map(|(_, v)| v.trim().to_string()))
    };
    Some(match n {
        "slime_block" => Kind::Slime,
        "honey_block" => Kind::Honey,
        "obsidian" | "crying_obsidian" => Kind::Obsidian,
        n if n.ends_with("glazed_terracotta") => Kind::Gt,
        "piston" | "sticky_piston" => {
            let f = DNAME.iter().position(|d| Some(d.to_string()) == prop("facing"))? as u8;
            Kind::Piston { sticky: n == "sticky_piston", f, ext: prop("extended").as_deref() == Some("true") }
        }
        "piston_head" => return None,
        _ => Kind::Stone,
    })
}

/// The mechanism part of a design (door blocks = the 9 window cells), ignoring wiring
/// (dust, repeaters, observers, torches, levers, redstone blocks are dropped).
pub fn world_of_design(d: &Design) -> Result<World> {
    let door = *d.ports.get("door").context("design has no door port")?;
    let mut b = Vec::new();
    for (p, s) in &d.blocks {
        let q = [(p[0] - door[0]) as i8, (p[1] - door[1]) as i8, (p[2] - door[2]) as i8];
        let name = mctas::blockstate::block_name(s);
        let n = name.trim_start_matches("minecraft:");
        if matches!(n, "redstone_wire" | "repeater" | "comparator" | "observer" | "redstone_torch" | "redstone_wall_torch" | "lever" | "redstone_block" | "piston_head") {
            continue;
        }
        let Some(k) = kind_of_state(s) else { continue };
        let k = if in_window(q) { Kind::Door } else { k };
        b.push((q, k));
    }
    b.sort();
    let pow = World::rest_pow(&b);
    Ok(World { b, pow })
}

pub fn state_of_kind(k: Kind) -> String {
    match k {
        Kind::Door => "minecraft:stone".into(),
        Kind::Slime => "minecraft:slime_block".into(),
        Kind::Honey => "minecraft:honey_block".into(),
        Kind::Obsidian => "minecraft:obsidian".into(),
        Kind::Gt => "minecraft:white_glazed_terracotta[facing=north]".into(),
        Kind::Stone => "minecraft:smooth_stone".into(),
        Kind::Piston { sticky, f, ext } => format!(
            "minecraft:{}[extended={},facing={}]",
            if sticky { "sticky_piston" } else { "piston" },
            ext,
            DNAME[f as usize]
        ),
    }
}

pub fn design_of_world(w: &World, name: &str) -> Design {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    d.ports.insert("door".into(), [0, 0, 0]);
    for (p, k) in &w.b {
        let q = [p[0] as i32, p[1] as i32, p[2] as i32];
        d.blocks.insert(q, state_of_kind(*k));
        if let Kind::Piston { sticky, f, ext: true } = k {
            let h = add(*p, D[*f as usize]);
            d.blocks.insert(
                [h[0] as i32, h[1] as i32, h[2] as i32],
                format!("minecraft:piston_head[facing={},short=false,type={}]", DNAME[*f as usize], if *sticky { "sticky" } else { "normal" }),
            );
        }
    }
    d
}

pub fn fmt_ev(w: &World, e: &Ev) -> String {
    let (p, k) = w.b[e.id as usize];
    format!("{}{}@({},{},{})", match e.act {
        Act::L => "L",
        Act::P => "P",
    }, k.code(), p[0], p[1], p[2])
}

pub fn fmt_seq(w: &World, s: &[Ev]) -> String {
    s.iter().map(|e| fmt_ev(w, e)).collect::<Vec<_>>().join(" ")
}

/// A state with interchangeable non-piston blocks (the door's 9 identical blocks may trade
/// places): pistons by identity, every other block as a sorted set, plus the level bits.
fn canon(w: &World) -> (Vec<(P, Kind)>, Vec<(P, Kind)>, u64) {
    let mut pistons = Vec::new();
    let mut rest = Vec::new();
    for (p, k) in &w.b {
        if matches!(k, Kind::Piston { .. }) {
            pistons.push((*p, *k));
        } else {
            rest.push((*p, *k));
        }
    }
    rest.sort_by_key(|(p, k)| (*p, format!("{k:?}")));
    (pistons, rest, w.pow)
}

/// Parse "P:x,y,z L:x,y,z ..." (pistons named by their rest position) into events of `rest`.
pub fn parse_seq(rest: &World, text: &str) -> Result<Vec<Ev>> {
    text.split_whitespace()
        .map(|tok| {
            let (act, pos) = tok.split_once(':').with_context(|| format!("event {tok:?}: expected P:x,y,z or L:x,y,z"))?;
            let act = match act {
                "P" => Act::P,
                "L" => Act::L,
                a => bail!("event {tok:?}: unknown action {a:?} (P = 2 gt pulse, L = level toggle)"),
            };
            let p = parse_p(pos)?;
            let id = rest
                .b
                .iter()
                .position(|(q, k)| *q == p && matches!(k, Kind::Piston { .. }))
                .with_context(|| format!("event {tok:?}: no piston at {pos} at rest"))?;
            Ok(Ev { id: id as u8, act })
        })
        .collect()
}

/// Symmetric control by product BFS: one event sequence S with S(rest) = `open` and S(open) =
/// rest, found by searching pairs of states (the run from rest, the run from open) under the
/// same events, shortest first. An event that changes nothing in one run (a swallowed pulse, a
/// blocked push, a pull of air) leaves that run's state as it is. `levels` also allows level
/// toggles. Interchangeable door blocks count as equal. None when no S exists within `cap` pairs.
pub fn sym_bfs(rest: &World, open: &World, levels: bool, cap: usize) -> Option<Vec<Ev>> {
    let evs: Vec<Ev> = rest
        .pistons()
        .into_iter()
        .flat_map(|i| {
            let mut v = vec![Ev { id: i as u8, act: Act::P }];
            if levels {
                v.push(Ev { id: i as u8, act: Act::L });
            }
            v
        })
        .collect();
    let goal = (canon(open), canon(rest));
    type Key = ((Vec<(P, Kind)>, Vec<(P, Kind)>, u64), (Vec<(P, Kind)>, Vec<(P, Kind)>, u64));
    let mut parent: HashMap<Key, (Key, Ev)> = HashMap::new();
    let start_key: Key = (canon(rest), canon(open));
    let mut seen: HashSet<Key> = HashSet::from([start_key.clone()]);
    let mut layer: Vec<(World, World, Key)> = vec![(rest.clone(), open.clone(), start_key.clone())];
    while !layer.is_empty() && seen.len() < cap {
        let mut next = Vec::new();
        for (a, b, k) in &layer {
            for e in &evs {
                let na = a.apply(*e);
                let nb = b.apply(*e);
                if na.is_none() && nb.is_none() {
                    continue;
                }
                let na = na.unwrap_or_else(|| a.clone());
                let nb = nb.unwrap_or_else(|| b.clone());
                let nk: Key = (canon(&na), canon(&nb));
                if !seen.insert(nk.clone()) {
                    continue;
                }
                parent.insert(nk.clone(), (k.clone(), *e));
                if nk.0 == goal.0 && nk.1 == goal.1 {
                    let mut seq = Vec::new();
                    let mut cur = nk;
                    while let Some((pk, ev)) = parent.get(&cur) {
                        seq.push(*ev);
                        cur = pk.clone();
                    }
                    seq.reverse();
                    return Some(seq);
                }
                next.push((na, nb, nk));
            }
        }
        layer = next;
    }
    None
}

// ---------------------------------------------------------------- enumeration

#[derive(Clone, Debug)]
pub struct Require {
    pub kinds: Vec<String>,
    pub lo: P,
    pub hi: P,
}

impl Require {
    /// "piston,slime:x0,y0,z0:x1,y1,z1": some block of one of these kinds inside the box.
    pub fn parse(s: &str) -> Result<Require> {
        let parts: Vec<&str> = s.split(':').collect();
        anyhow::ensure!(parts.len() == 3, "--require kinds:x0,y0,z0:x1,y1,z1");
        Ok(Require { kinds: parts[0].split(',').map(|k| k.to_string()).collect(), lo: parse_p(parts[1])?, hi: parse_p(parts[2])? })
    }
    pub fn met(&self, w: &World) -> bool {
        w.b.iter().any(|(p, k)| {
            (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
                && self.kinds.iter().any(|n| match (n.as_str(), k) {
                    ("piston", Kind::Piston { .. }) => true,
                    ("slime", Kind::Slime) | ("honey", Kind::Honey) | ("obsidian", Kind::Obsidian) | ("gt", Kind::Gt) | ("stone", Kind::Stone) => true,
                    ("any", _) => true,
                    _ => false,
                })
        })
    }
}

#[derive(Clone, Debug)]
pub struct EnumOptions {
    pub require: Vec<Require>,
    pub add: usize,
    pub lo: P,
    pub hi: P,
    /// Candidate kinds: slime, honey, obsidian, gt, stone, sticky, normal.
    pub kinds: Vec<String>,
    pub ext: bool,
    pub cap: usize,
    pub threads: usize,
    pub max_blocks: usize,
    pub sym_len: usize,
    pub sym_tries: usize,
    /// `--topology x0,y0:x1,y1`: report whether each mechanism can be wired inside that box of the
    /// door plane with FULL seamless (topology.rs: wiring components, links, parity).
    pub topology: Option<([i8; 2], [i8; 2])>,
}

/// The door plane's hallway ring (S at z = 0): each cell with the cell one further out.
pub fn ring12() -> Vec<(P, P)> {
    let mut out = Vec::new();
    for t in 0..=2i8 {
        out.push(([-1, t, 0], [-2, t, 0]));
        out.push(([3, t, 0], [4, t, 0]));
        out.push(([t, -1, 0], [t, -2, 0]));
        out.push(([t, 3, 0], [t, 4, 0]));
    }
    out
}

fn ring16() -> Vec<P> {
    let mut out = Vec::new();
    for x in -1..=3i8 {
        for y in -1..=3i8 {
            if !in_window([x, y, 0]) {
                out.push([x, y, 0]);
            }
        }
    }
    out
}

pub fn candidates(base: &World, o: &EnumOptions) -> Vec<(P, Kind)> {
    let occupied = |p: P| base.occ(p) != Occ::Air;
    let ring: HashSet<P> = ring16().into_iter().collect();
    let base_cells: HashSet<P> = base.b.iter().map(|(p, _)| *p).collect();
    let active = |p: P| ring.contains(&p) || base_cells.contains(&p);
    let mut out = Vec::new();
    for x in o.lo[0]..=o.hi[0] {
        for y in o.lo[1]..=o.hi[1] {
            for z in o.lo[2]..=o.hi[2] {
                let p = [x, y, z];
                if occupied(p) || in_corridor(p) || (SEAMLESS.load(Ordering::Relaxed) && in_s(p) && p[2] != 0) {
                    continue;
                }
                let near = D.iter().any(|d| active(add(p, *d)));
                for k in &o.kinds {
                    let block = match k.as_str() {
                        "slime" => Some(Kind::Slime),
                        "honey" => Some(Kind::Honey),
                        "obsidian" => Some(Kind::Obsidian),
                        "gt" => Some(Kind::Gt),
                        "stone" => Some(Kind::Stone),
                        _ => None,
                    };
                    if let Some(b) = block {
                        if near {
                            out.push((p, b));
                        }
                        continue;
                    }
                    let sticky = match k.as_str() {
                        "sticky" => true,
                        "normal" => false,
                        _ => continue,
                    };
                    let planar = o.lo[2] == o.hi[2];
                    for f in 0..6u8 {
                        if planar && f >= 4 {
                            continue;
                        }
                        for ext in [false, true] {
                            if ext && !o.ext {
                                continue;
                            }
                            let head = add(p, D[f as usize]);
                            if ext && (occupied(head) || in_corridor(head)) {
                                continue;
                            }
                            // reach: what this piston can push or pull
                            let reach: Vec<P> = if ext {
                                vec![add(head, D[f as usize])]
                            } else {
                                vec![head, add(head, D[f as usize])]
                            };
                            if !reach.iter().any(|r| active(*r) || in_window(*r)) {
                                continue;
                            }
                            out.push((p, Kind::Piston { sticky, f, ext }));
                        }
                    }
                }
            }
        }
    }
    out
}

fn cells_of(p: P, k: Kind) -> Vec<P> {
    match k {
        Kind::Piston { ext: true, f, .. } => vec![p, add(p, D[f as usize])],
        _ => vec![p],
    }
}

/// Cheap necessary conditions: every door block can be touched (glued to a sticky block or on
/// the line of a piston facing it), every sticky block touches something movable.
fn plausible(w: &World) -> bool {
    let cells: HashMap<P, Kind> = w.b.iter().map(|(p, k)| (*p, *k)).collect();
    let pist: Vec<(P, u8)> = w.b.iter().filter_map(|(p, k)| match k {
        Kind::Piston { f, .. } => Some((*p, *f)),
        _ => None,
    }).collect();
    for (p, k) in &w.b {
        if *k != Kind::Door {
            continue;
        }
        let glued = D.iter().any(|d| cells.get(&add(*p, *d)).is_some_and(|k| k.sticky_block()));
        if glued {
            continue;
        }
        // a piston on the line through p, facing p
        let lined = pist.iter().any(|(q, f)| {
            let d = D[*f as usize];
            let rel = sub(*p, *q);
            (1..=8).any(|t| mul(d, t) == rel)
        });
        if !lined {
            return false;
        }
    }
    true
}

#[derive(Clone, Debug)]
pub struct Found {
    pub world: World,
    pub added: Vec<(P, Kind)>,
    pub sol: Solution,
}

impl Found {
    pub fn control(&self) -> usize {
        match &self.sol.sym {
            Some(s) => s.len(),
            None => 100 + self.sol.open.len() + self.sol.close.len(),
        }
    }
}

pub fn enumerate(base: &World, o: &EnumOptions, log: &(dyn Fn(&str) + Sync)) -> Vec<Found> {
    let cands = candidates(base, o);
    log(&format!("{} candidates ({} base blocks)", cands.len(), base.b.len()));
    // combos of up to o.add candidates, pairwise compatible
    let n = cands.len();
    let cells: Vec<Vec<P>> = cands.iter().map(|(p, k)| cells_of(*p, *k)).collect();
    let compatible = |a: usize, b: usize| !cells[a].iter().any(|c| cells[b].contains(c));
    let mut combos: Vec<Vec<u16>> = vec![Vec::new()];
    let mut frontier: Vec<Vec<u16>> = vec![Vec::new()];
    for _ in 0..o.add {
        let mut next = Vec::new();
        for c in &frontier {
            let start = c.last().map(|&l| l as usize + 1).unwrap_or(0);
            for i in start..n {
                if c.iter().all(|&j| compatible(i, j as usize)) {
                    let mut c2 = c.clone();
                    c2.push(i as u16);
                    next.push(c2);
                }
            }
        }
        combos.extend(next.iter().cloned());
        frontier = next;
    }
    log(&format!("{} combinations", combos.len()));
    let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|s| {
        for _ in 0..o.threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= combos.len() {
                    break;
                }
                let c = &combos[i];
                let mut b = base.b.clone();
                for &j in c {
                    b.push(cands[j as usize]);
                }
                if b.len() > o.max_blocks {
                    continue;
                }
                b.sort();
                let w = World { pow: World::rest_pow(&b), b };
                // heads of base pistons must not sit on new blocks
                let ok = w.b.iter().all(|(p, k)| match k {
                    Kind::Piston { ext: true, f, .. } => w.kind_at(add(*p, D[*f as usize])).is_none(),
                    _ => true,
                });
                if !ok || !plausible(&w) || !o.require.iter().all(|r| r.met(&w)) {
                    continue;
                }
                if !screen(&w, o.cap) {
                    continue;
                }
                if let Some(sol) = solve(&w, o.cap * 4, 0, 0) {
                    let added = c.iter().map(|&j| cands[j as usize]).collect();
                    found.lock().unwrap().push(Found { world: w, added, sol });
                }
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 200000 == 0 {
                    log(&format!("  {d} solved-checked, {} found, {:.0}s", found.lock().unwrap().len(), t0.elapsed().as_secs_f64()));
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap();
    // symmetric schedules for the smallest ones
    found.sort_by_key(|f| (f.world.blocks(), f.sol.open.len() + f.sol.close.len()));
    log(&format!("{} mechanisms open and close; symmetric search on the smallest", found.len()));
    let lim = found.len().min(2000);
    let idx = AtomicUsize::new(0);
    let syms: Mutex<Vec<(usize, Option<Vec<Ev>>)>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.threads.max(1) {
            s.spawn(|| loop {
                let i = idx.fetch_add(1, Ordering::Relaxed);
                if i >= lim {
                    break;
                }
                let sol = solve(&found[i].world, o.cap, o.sym_len, o.sym_tries);
                syms.lock().unwrap().push((i, sol.and_then(|s| s.sym)));
            });
        }
    });
    for (i, s) in syms.into_inner().unwrap() {
        found[i].sol.sym = s;
    }
    found.sort_by_key(|f| (f.world.blocks(), f.control()));
    found
}

// ---------------------------------------------------------------- ideal-input schedules

/// Input cell for piston `id` usable over states `span` (all must keep it air, away from
/// slime/honey and from other pistons' power checks).
fn input_cell(states: &[&World], id: usize) -> Option<P> {
    input_cell_avoid(states, id, &[])
}

fn input_cell_avoid(states: &[&World], id: usize, avoid: &[P]) -> Option<P> {
    let w0 = states[0];
    let (pos, k) = w0.b[id];
    let Kind::Piston { f, .. } = k else { return None };
    let mut best: Option<(i32, P)> = None;
    for d in 0..6u8 {
        if d == f {
            continue;
        }
        let c = add(pos, D[d as usize]);
        if in_corridor(c) || avoid.contains(&c) {
            continue;
        }
        let mut ok = true;
        let mut qc = 0;
        for w in states {
            if w.b[id].0 != pos || w.occ(c) != Occ::Air {
                ok = false;
                break;
            }
            for dd in D {
                let n = add(c, dd);
                match w.occ(n) {
                    Occ::Block(j) => {
                        let kj = w.b[j].1;
                        if kj.sticky_block() {
                            ok = false;
                        }
                        if j != id && matches!(kj, Kind::Piston { .. }) {
                            ok = false;
                        }
                    }
                    Occ::Head(j) if j != id => {
                        // heads forward updates only; harmless
                    }
                    _ => {}
                }
                // quasi-connectivity: c next to the cell above another piston (it is powered
                // but not updated by the placement: allowed, but avoided when possible)
                for (j, (q, kj)) in w.b.iter().enumerate() {
                    if j != id && matches!(kj, Kind::Piston { .. }) && add(*q, [0, 1, 0]) == n {
                        qc += 1;
                    }
                }
            }
            if !ok {
                break;
            }
        }
        if ok {
            // prefer cells away from the door plane's corridor and below/beside
            let score = 10 * qc + if d == 3 { 0 } else { 1 } + if c[2] != 0 { 2 } else { 0 };
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, c));
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Group a sequence into steps: consecutive pulses on different pistons that commute (either
/// order gives the same state, from every start in `starts`) share one tick. Returns the step
/// index of every event.
fn steps_of(starts: &[&World], seq: &[Ev]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut cur: Vec<Ev> = Vec::new();
    let mut start: Vec<World> = starts.iter().map(|w| (*w).clone()).collect();
    let mut state: Vec<World> = start.clone();
    let mut step = 0usize;
    let run = |w: &World, s: &[Ev]| -> World {
        let mut c = w.clone();
        for e in s {
            c = c.apply(*e).unwrap_or(c);
        }
        c
    };
    for (i, e) in seq.iter().enumerate() {
        let joins = i > 0
            && e.act == Act::P
            && cur.iter().all(|x| x.act == Act::P && x.id != e.id)
            && start.iter().all(|st| {
                let mut a = cur.clone();
                a.push(*e);
                let mut b = vec![*e];
                b.extend(cur.iter().copied());
                run(st, &a) == run(st, &b)
            });
        if joins {
            cur.push(*e);
        } else {
            if i > 0 {
                step += 1;
            }
            cur = vec![*e];
            start = state.clone();
        }
        out.push(step);
        for s in state.iter_mut() {
            *s = s.apply(*e).unwrap_or(s.clone());
        }
    }
    out
}

pub struct IdealPlan {
    pub design: Design,
    /// (open?, tick, pos, Some(state) = set / None = clear)
    pub events: Vec<(bool, u32, [i32; 3], Option<String>)>,
    pub text: String,
}

/// Convert a macro solution into a `door ideal` schedule (redstone blocks set/cleared next to
/// each piston at `gap`-tick steps). `sym` uses the symmetric sequence on both edges.
pub fn ideal_plan(rest: &World, sol: &Solution, sym: bool, gap: u32, name: &str) -> Result<IdealPlan> {
    let (open_seq, close_seq) = match (&sol.sym, sym) {
        (Some(s), true) => (s.clone(), s.clone()),
        _ => (sol.open.clone(), sol.close.clone()),
    };
    // trajectory over open then close
    let mut traj = vec![rest.clone()];
    for e in open_seq.iter().chain(close_seq.iter()) {
        let cur = traj.last().unwrap().clone();
        traj.push(cur.apply(*e).unwrap_or(cur));
    }
    let all: Vec<Ev> = open_seq.iter().chain(close_seq.iter()).copied().collect();
    let nopen = open_seq.len();
    let mut design = design_of_world(rest, name);
    let mut events = Vec::new();
    let r = |p: P| [p[0] as i32, p[1] as i32, p[2] as i32];
    let rb = "minecraft:redstone_block".to_string();
    // level spans: for each piston, intervals [a, b) of steps where its level is on
    let n = all.len();
    // steps: commuting pulses share a tick
    let open_steps = if sym { steps_of(&[rest, &traj[nopen]], &open_seq) } else { steps_of(&[rest], &open_seq) };
    let close_steps = if sym { open_steps.clone() } else { steps_of(&[&traj[nopen]], &close_seq) };
    let step_of = |ev: usize| -> (bool, u32) {
        if ev < nopen { (true, open_steps[ev] as u32 * gap) } else { (false, close_steps[ev - nopen] as u32 * gap) }
    };
    // event index range [first, last] of the step holding event ev
    let step_span = |ev: usize| -> (usize, usize) {
        let key = step_of(ev);
        let mut a = ev;
        while a > 0 && step_of(a - 1) == key {
            a -= 1;
        }
        let mut b = ev;
        while b + 1 < n && step_of(b + 1) == key {
            b += 1;
        }
        (a, b)
    };
    // the tick of the event that moved traj[step-1] to traj[step]
    let at = |step: usize| -> (bool, u32) { step_of(step - 1) };
    let mut level_cells: Vec<P> = Vec::new();
    for id in rest.pistons() {
        let bit = 1u64 << id;
        // intervals [a, b) of states with the level on (b = n + 1: still on at the end)
        let mut iv = Vec::new();
        let mut s = 0;
        while s <= n {
            if traj[s].pow & bit == 0 {
                s += 1;
                continue;
            }
            let a = s;
            while s <= n && traj[s].pow & bit != 0 {
                s += 1;
            }
            iv.push((a, s));
        }
        let span_of = |a: usize, b: usize| -> Vec<&World> { traj[a.saturating_sub(1)..=b.min(n)].iter().collect() };
        let rest_on = traj[0].pow & bit != 0;
        let mut rest_cell = None;
        if rest_on {
            if traj[n].pow & bit == 0 {
                bail!("piston {id} powered at rest but not after closing");
            }
            let (_, b0) = iv[0];
            let (a1, _) = *iv.last().unwrap();
            let mut span = span_of(0, b0);
            span.extend(span_of(a1, n));
            let cell = input_cell(&span, id).with_context(|| format!("no rest input cell for piston {id}"))?;
            design.blocks.insert(r(cell), rb.clone());
            rest_cell = Some(cell);
        }
        for &(a, b) in &iv {
            let cell = match rest_cell {
                Some(c) if a == 0 || b == n + 1 => c,
                _ => input_cell(&span_of(a, b), id).with_context(|| format!("no input cell for piston {id} over steps {a}..{b}"))?,
            };
            if a > 0 {
                let (op, t) = at(a);
                events.push((op, t, r(cell), Some(rb.clone())));
            }
            if b <= n {
                let (op, t) = at(b);
                events.push((op, t, r(cell), None));
            }
            level_cells.push(cell);
        }
    }
    // pulses (a pulse on a powered piston is swallowed: not emitted)
    for (s, e) in all.iter().enumerate() {
        if e.act != Act::P || traj[s].pow & (1u64 << e.id) != 0 {
            continue;
        }
        let (a, b) = step_span(s);
        let span: Vec<&World> = traj[a..=b + 1].iter().collect();
        let cell = input_cell_avoid(&span, e.id as usize, &level_cells).with_context(|| format!("no input cell for pulse {}", fmt_ev(&traj[s], e)))?;
        let (op, t) = step_of(s);
        events.push((op, t, r(cell), Some(rb.clone())));
        events.push((op, t + 2, r(cell), None));
    }
    events.sort_by_key(|e| (!e.0, e.1));
    let mut text = format!("# mech-synth ideal schedule ({} on both edges)\ndesign {name}.txt\n", if sym { "symmetric" } else { "open/close" });
    text.push_str(&format!("# open:  {}\n# close: {}\n", fmt_seq(rest, &open_seq), fmt_seq(rest, &close_seq)));
    for (op, t, p, st) in &events {
        match st {
            Some(s) => text.push_str(&format!("{} {} set {} {} {} {}\n", if *op { "open" } else { "close" }, t, p[0], p[1], p[2], s)),
            None => text.push_str(&format!("{} {} clear {} {} {}\n", if *op { "open" } else { "close" }, t, p[0], p[1], p[2])),
        }
    }
    Ok(IdealPlan { design, events, text })
}

// ---------------------------------------------------------------- CLI

pub fn parse_p(s: &str) -> Result<P> {
    let v: Vec<i8> = s.split(',').map(|t| t.trim().parse::<i8>()).collect::<Result<_, _>>()?;
    anyhow::ensure!(v.len() == 3, "expected x,y,z, got {s:?}");
    Ok([v[0], v[1], v[2]])
}

/// Replay a plan in the simulator with ideal inputs; Ok(()) = works at every position.
pub fn verify_plan(w: &mut mctas::sim::Worker, plan: &IdealPlan, extra: u32) -> Result<()> {
    let s = crate::ideal::Schedule::from_events(plan.design.clone(), &plan.events);
    let opts = crate::ideal::IdealOptions { cycles: 2, offsets: crate::ideal::offsets(extra), trace: None };
    crate::ideal::run(w, &s, &Default::default(), &opts).map(|_| ())
}

/// Wiring traits of a symmetric sequence: step of the bottom-middle piston's first pulse (the one
/// under M), and the pistons other than the elevator column (x 1, z 0) that are pulsed at two
/// different positions (they ride a structure: two places to power).
pub fn traits(w: &World, seq: &[Ev]) -> (Option<usize>, Vec<String>) {
    let mut cur = w.clone();
    let mut m_step = None;
    let mut seen: HashMap<u8, HashSet<P>> = HashMap::new();
    for (i, e) in seq.iter().enumerate() {
        let (p, _) = cur.b[e.id as usize];
        let (p0, _) = w.b[e.id as usize];
        if p0[0] == 1 && p0[2] == 0 && p0[1] < 0 && m_step.is_none() {
            m_step = Some(i);
        }
        seen.entry(e.id).or_default().insert(p);
        cur = cur.apply(*e).unwrap_or(cur);
    }
    let riders = seen
        .iter()
        .filter(|(id, ps)| {
            let (p0, _) = w.b[**id as usize];
            ps.len() > 1 && !(p0[0] == 1 && p0[2] == 0)
        })
        .map(|(id, _)| {
            let (p0, k) = w.b[*id as usize];
            format!("{}@({},{},{})", k.code(), p0[0], p0[1], p0[2])
        })
        .collect();
    (m_step, riders)
}

fn describe(f: &Found) -> String {
    let sym = match &f.sol.sym {
        Some(s) => {
            let (m, riders) = traits(&f.world, s);
            format!(
                "SYM {} events: {} [M pulse at step {}, riders: {}]",
                s.len(),
                fmt_seq(&f.world, s),
                m.map(|i| i.to_string()).unwrap_or("-".into()),
                if riders.is_empty() { "none".to_string() } else { riders.join(" ") }
            )
        }
        None => "no symmetric sequence".to_string(),
    };
    format!(
        "{} blocks | open {} + close {} events | {}\n    added: {}\n    open:  {}\n    close: {}",
        f.world.blocks(),
        f.sol.open.len(),
        f.sol.close.len(),
        sym,
        f.added.iter().map(|(p, k)| format!("{}@({},{},{})", k.code(), p[0], p[1], p[2])).collect::<Vec<_>>().join(" "),
        fmt_seq(&f.world, &f.sol.open),
        fmt_seq(&f.world, &f.sol.close)
    )
}

#[allow(clippy::too_many_arguments)]
/// `mech-synth <base> --sym-bfs "<open sequence>"`: the open state reached by the given events,
/// then [`sym_bfs`] for one sequence on both lever edges; prints it, writes `sym.ideal` (+ the
/// design) into `out_dir` and replays it in the simulator when `verify` > 0.
pub fn sym_cli(version: &str, base: &std::path::Path, open_seq: &str, levels: bool, cap: usize, out_dir: Option<&std::path::Path>, verify: usize, extra: u32, gap: u32) -> Result<()> {
    let d = crate::cli::load_design(base)?;
    let rest = world_of_design(&d)?;
    if open_seq.trim() == "auto" {
        // Every open state reachable from rest (by the same events), each tried as O.
        let evs: Vec<Ev> = rest.pistons().into_iter().flat_map(|i| {
            let mut v = vec![Ev { id: i as u8, act: Act::P }];
            if levels {
                v.push(Ev { id: i as u8, act: Act::L });
            }
            v
        }).collect();
        let mut seen: HashSet<_> = HashSet::from([canon(&rest)]);
        let mut layer = vec![(rest.clone(), Vec::<Ev>::new())];
        let mut opens: Vec<(World, Vec<Ev>)> = Vec::new();
        while !layer.is_empty() && seen.len() < cap {
            let mut next = Vec::new();
            for (w, path) in &layer {
                for e in &evs {
                    let Some(n) = w.apply(*e) else { continue };
                    if !seen.insert(canon(&n)) {
                        continue;
                    }
                    let mut p = path.clone();
                    p.push(*e);
                    if n.is_open() {
                        opens.push((n.clone(), p.clone()));
                    }
                    next.push((n, p));
                }
            }
            layer = next;
        }
        println!("{} open states reachable ({} states)", opens.len(), seen.len());
        let mut best: Option<(Vec<Ev>, Vec<Ev>)> = None;
        for (o, path) in &opens {
            let limit = best.as_ref().map_or(usize::MAX, |b| b.0.len());
            if let Some(s) = sym_bfs(&rest, o, levels, cap / 4) {
                if s.len() < limit {
                    println!("  {} events via open state reached by: {}", s.len(), fmt_seq(&rest, path));
                    best = Some((s, path.clone()));
                }
            }
        }
        let Some((_, path)) = best else {
            println!("no symmetric sequence for any open state");
            return Ok(());
        };
        let text: Vec<String> = path.iter().map(|e| {
            let p = rest.b[e.id as usize].0;
            format!("{}:{},{},{}", if e.act == Act::P { "P" } else { "L" }, p[0], p[1], p[2])
        }).collect();
        return sym_cli(version, base, &text.join(" "), levels, cap, out_dir, verify, extra, gap);
    }
    let seq = parse_seq(&rest, open_seq)?;
    let mut open = rest.clone();
    for e in &seq {
        open = open.apply(*e).with_context(|| format!("open sequence: {} changes nothing", fmt_ev(&rest, e)))?;
    }
    anyhow::ensure!(open.is_open(), "the open sequence does not open the door (corridor not clear)");
    let t0 = std::time::Instant::now();
    let Some(sym) = sym_bfs(&rest, &open, levels, cap) else {
        println!("no symmetric sequence within {cap} state pairs ({:.1} s)", t0.elapsed().as_secs_f64());
        return Ok(());
    };
    println!("symmetric sequence, {} events ({:.1} s):\n    {}", sym.len(), t0.elapsed().as_secs_f64(), fmt_seq(&rest, &sym));
    // Where each pulsed piston stands in the two runs (its power cell must reach it there), and
    // which cells change at each step in each run (what an observer could watch).
    let occ_map = |w: &World| -> HashMap<P, String> {
        let mut m = HashMap::new();
        for (i, (p, k)) in w.b.iter().enumerate() {
            m.insert(*p, format!("{i}:{k:?}"));
            if let Kind::Piston { ext: true, f, .. } = k {
                m.insert(add(*p, D[*f as usize]), format!("{i}:head"));
            }
        }
        m
    };
    let changed = |a: &World, b: &World| -> Vec<P> {
        let (ma, mb) = (occ_map(a), occ_map(b));
        let mut cells: Vec<P> = ma.keys().chain(mb.keys()).copied().filter(|p| ma.get(p) != mb.get(p)).collect();
        cells.sort();
        cells.dedup();
        cells
    };
    let (mut a, mut b) = (rest.clone(), open.clone());
    for (i, e) in sym.iter().enumerate() {
        let (pa, pb) = (a.b[e.id as usize].0, b.b[e.id as usize].0);
        let na = a.apply(*e);
        let nb = b.apply(*e);
        println!(
            "  {:>2} {:<18} from rest: at {:?}{}   from open: at {:?}{}",
            i + 1,
            fmt_ev(&rest, e),
            pa,
            if na.is_some() { "" } else { " (no-op)" },
            pb,
            if nb.is_some() { "" } else { " (no-op)" }
        );
        let na = na.unwrap_or_else(|| a.clone());
        let nb = nb.unwrap_or_else(|| b.clone());
        let fmt = |v: Vec<P>| v.iter().map(|p| format!("({},{},{})", p[0], p[1], p[2])).collect::<Vec<_>>().join(" ");
        println!("       changed from rest: {}   from open: {}", fmt(changed(&a, &na)), fmt(changed(&b, &nb)));
        a = na;
        b = nb;
    }
    let sol = Solution { open: sym.clone(), close: sym.clone(), sym: Some(sym), states: 0 };
    let plan = ideal_plan(&rest, &sol, true, gap, "sym")?;
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(dir)?;
        let mut dd = plan.design.clone();
        dd.name = "sym".into();
        dd.save(&dir.join("sym.txt"))?;
        let text = plan.text.replace("design sym.txt", &format!("design {}", dir.join("sym.txt").display()));
        std::fs::write(dir.join("sym.ideal"), text)?;
        println!("wrote {}", dir.join("sym.ideal").display());
    }
    if verify > 0 {
        let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
        let mut wk = mctas::sim::Worker::spawn(&cfg)?;
        match verify_plan(&mut wk, &plan, extra) {
            Ok(()) => println!("  sim symmetric: WORKS"),
            Err(e) => println!("  sim symmetric: BROKEN {e:#}"),
        }
    }
    Ok(())
}

/// `--oc "<open events> | <close events>"`: replay a hand-written open path and close path in the
/// macro model (each event must change something; open must clear the corridor, close must give
/// back the exact rest state), print what each step moves, write the ideal-input design and
/// schedule (`--out-dir`) and replay it in the simulator (`--verify`).
pub fn oc_cli(version: &str, base: &std::path::Path, oc: &str, out_dir: Option<&std::path::Path>, verify: usize, extra: u32, gap: u32) -> Result<()> {
    let d = crate::cli::load_design(base)?;
    let rest = world_of_design(&d)?;
    let (o, c) = oc.split_once('|').context("--oc wants \"<open events> | <close events>\"")?;
    let (open_seq, close_seq) = (parse_seq(&rest, o)?, parse_seq(&rest, c)?);
    let show = |w: &World| -> String {
        let mut cells: Vec<String> = Vec::new();
        for (i, (p, k)) in w.b.iter().enumerate() {
            if !matches!(k, Kind::Piston { .. }) && !matches!(k, Kind::Door) {
                continue;
            }
            if let Kind::Door = k {
                cells.push(format!("d{i}({},{})", p[0], p[1]));
            }
        }
        cells.join(" ")
    };
    let mut w = rest.clone();
    for (label, seq) in [("open", &open_seq), ("close", &close_seq)] {
        for (i, e) in seq.iter().enumerate() {
            let p = w.b[e.id as usize].0;
            let n = w.apply(*e).with_context(|| format!("{label} step {}: {} at {:?} changes nothing (or is not allowed)", i + 1, fmt_ev(&rest, e), p))?;
            println!("  {label} {:>2} {:<18} at {:?}   doors: {}", i + 1, fmt_ev(&rest, e), p, show(&n));
            w = n;
        }
        if label == "open" {
            anyhow::ensure!(w.is_open(), "the open path does not open the door (corridor not clear{})", if SEAMLESS_OPEN.load(Ordering::Relaxed) { " or ring not clean" } else { "" });
            println!("  OPEN reached");
        } else {
            anyhow::ensure!(canon(&w) == canon(&rest), "the close path does not return to the rest state");
            println!("  REST reached");
        }
    }
    let sol = Solution { open: open_seq.clone(), close: close_seq.clone(), sym: None, states: 0 };
    let plan = ideal_plan(&rest, &sol, false, gap, "oc")?;
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(dir)?;
        let mut dd = plan.design.clone();
        dd.name = "oc".into();
        dd.save(&dir.join("oc.txt"))?;
        let text = plan.text.replace("design oc.txt", &format!("design {}", dir.join("oc.txt").display()));
        std::fs::write(dir.join("oc.ideal"), text)?;
        println!("wrote {}", dir.join("oc.ideal").display());
    }
    if verify > 0 {
        let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
        let mut wk = mctas::sim::Worker::spawn(&cfg)?;
        match verify_plan(&mut wk, &plan, extra) {
            Ok(()) => println!("  sim open/close: WORKS"),
            Err(e) => println!("  sim open/close: BROKEN {e:#}"),
        }
    }
    Ok(())
}

pub fn cli(version: &str, base: &std::path::Path, o: &EnumOptions, out_dir: Option<&std::path::Path>, top: usize, verify: usize, extra: u32, gap: u32, workers: usize, just_base: bool) -> Result<()> {
    let d = crate::cli::load_design(base)?;
    let mut w = world_of_design(&d)?;
    if o.topology.is_some() {
        // A full door design: its plain blocks are wiring (stack feeds, floors), not mechanism.
        w.b.retain(|(_, k)| !matches!(k, Kind::Stone));
        w.pow = World::rest_pow(&w.b);
    }
    let log = |s: &str| eprintln!("[mech-synth] {s}");
    if just_base {
        let t0 = std::time::Instant::now();
        println!("screen(cap {}): {} ({:.1} ms)", o.cap, screen(&w, o.cap), t0.elapsed().as_secs_f64() * 1e3);
        let t0 = std::time::Instant::now();
        match solve_astar(&w, o.cap) {
            Some(s) => println!("A*: open {} + close {} ({:.1} ms)\n    open:  {}\n    close: {}", s.open.len(), s.close.len(), t0.elapsed().as_secs_f64() * 1e3, fmt_seq(&w, &s.open), fmt_seq(&w, &s.close)),
            None => println!("A*: none ({:.1} ms)", t0.elapsed().as_secs_f64() * 1e3),
        }
        let t0 = std::time::Instant::now();
        let split_only = SPLIT_ONLY.load(Ordering::Relaxed);
        let full = solve(&w, o.cap * 8, if split_only { 0 } else { o.sym_len }, o.sym_tries);
        let sol = full.or_else(|| solve_astar(&w, o.cap)).map(|mut s| {
            if s.sym.is_none() && o.sym_len > 0 {
                s.sym = symmetric_split(&w, &s, o.sym_len, o.cap);
            }
            s
        });
        match sol {
            Some(sol) => {
                let f = Found { world: w.clone(), added: Vec::new(), sol };
                println!("{}  ({} states, {:.1} ms)", describe(&f), f.sol.states, t0.elapsed().as_secs_f64() * 1e3);
                if let Some((lo, hi)) = o.topology {
                    let t = crate::topology::analyze(&f.world, &f.sol, lo, hi);
                    print!("topology in x {}..{}, y {}..{}:\n{}", lo[0], hi[0], lo[1], hi[1], t.report);
                }
                if let Some(dir) = out_dir {
                    write_found(dir, "base", &f, gap)?;
                }
                if verify > 0 {
                    let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
                    let mut wk = mctas::sim::Worker::spawn(&cfg)?;
                    for sym in [true, false] {
                        if sym && f.sol.sym.is_none() {
                            continue;
                        }
                        let plan = ideal_plan(&f.world, &f.sol, sym, gap, "base")?;
                        match verify_plan(&mut wk, &plan, extra) {
                            Ok(()) => println!("  sim {}: WORKS", if sym { "symmetric" } else { "open/close" }),
                            Err(e) => println!("  sim {}: BROKEN {e:#}", if sym { "symmetric" } else { "open/close" }),
                        }
                    }
                }
            }
            None => println!("base does not open+close in the macro model ({:.1} ms)", t0.elapsed().as_secs_f64() * 1e3),
        }
        return Ok(());
    }
    let found = enumerate(&w, o, &log);
    // dedupe identical worlds
    let mut seen = HashSet::new();
    let found: Vec<Found> = found.into_iter().filter(|f| seen.insert(f.world.b.clone())).collect();
    println!("{} distinct mechanisms open and close", found.len());
    let mut hist: std::collections::BTreeMap<(usize, bool), usize> = Default::default();
    for f in &found {
        *hist.entry((f.world.blocks(), f.sol.sym.is_some())).or_default() += 1;
    }
    for ((b, s), n) in &hist {
        println!("  {b} blocks{}: {n}", if *s { " (symmetric)" } else { "" });
    }
    let found: Vec<Found> = match o.topology {
        None => found,
        Some((lo, hi)) => {
            let mut keep = Vec::new();
            let (mut strict, mut permissive) = (0, 0);
            for f in found {
                let t = crate::topology::analyze(&f.world, &f.sol, lo, hi);
                strict += t.strict as usize;
                permissive += t.permissive as usize;
                if t.permissive {
                    keep.push(f);
                }
            }
            println!("topology in x {}..{}, y {}..{}: {strict} wirable (strict parity), {permissive} (permissive); listing the permissive ones", lo[0], hi[0], lo[1], hi[1]);
            keep
        }
    };
    for (i, f) in found.iter().take(top).enumerate() {
        println!("#{i} {}", describe(f));
    }
    if let Some(dir) = out_dir {
        std::fs::create_dir_all(dir)?;
        for (i, f) in found.iter().take(top).enumerate() {
            write_found(dir, &format!("m{i:02}-{}b", f.world.blocks()), f, gap)?;
        }
    }
    if verify > 0 {
        let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
        let pool = mctas::sim::Pool::spawn(workers, &cfg)?;
        let items: Vec<(usize, bool)> = found.iter().take(verify).enumerate().flat_map(|(i, f)| {
            let mut v = vec![(i, false)];
            if f.sol.sym.is_some() {
                v.push((i, true));
            }
            v
        }).collect();
        let res = pool.map(items.clone(), |wk, _, (i, sym)| {
            let f = &found[i];
            let plan = ideal_plan(&f.world, &f.sol, sym, gap, &format!("m{i:02}"))?;
            Ok(verify_plan(wk, &plan, extra).map_err(|e| format!("{e:#}")))
        });
        for ((i, sym), r) in items.iter().zip(res) {
            let tag = if *sym { "symmetric" } else { "open/close" };
            match r {
                Ok(Ok(())) => println!("sim #{i} {tag}: WORKS"),
                Ok(Err(e)) => println!("sim #{i} {tag}: BROKEN {}", e.chars().take(200).collect::<String>()),
                Err(e) => println!("sim #{i} {tag}: ERROR {}", format!("{e:#}").chars().take(200).collect::<String>()),
            }
        }
    }
    Ok(())
}

fn write_found(dir: &std::path::Path, name: &str, f: &Found, gap: u32) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut info = describe(f);
    info.push('\n');
    for sym in [false, true] {
        if sym && f.sol.sym.is_none() {
            continue;
        }
        match ideal_plan(&f.world, &f.sol, sym, gap, name) {
            Ok(plan) => {
                let tag = if sym { "sym" } else { "oc" };
                plan.design.save(&dir.join(format!("{name}.txt")))?;
                let text = plan.text.replace(&format!("design {name}.txt"), &format!("design {}", dir.join(format!("{name}-{tag}.txt")).display()));
                let mut dd = plan.design.clone();
                dd.name = format!("{name}-{tag}");
                dd.save(&dir.join(format!("{name}-{tag}.txt")))?;
                std::fs::write(dir.join(format!("{name}-{tag}.ideal")), text)?;
            }
            Err(e) => info.push_str(&format!("no ideal plan ({}): {e:#}\n", if sym { "sym" } else { "open/close" })),
        }
    }
    std::fs::write(dir.join(format!("{name}.info")), info)?;
    Ok(())
}
