//! A lane router for the tower decoder (crates/circuit-display/src/route.rs): every lane is a
//! chain of ELEMENTS (repeaters, dust, powered turn blocks) from a source repeater to a lamp, laid
//! by A* in a box of free cells, one lane after the other.
//!
//! The interaction model is conservative: an element may be fed only by its predecessor in its own
//! lane and may feed only its successor. Power is modelled per cell:
//! - a repeater emits into its front cell; it reads its back cell (anything) and its two side cells
//!   (repeaters and the base only: a lock);
//! - dust emits into the block under it and into all four horizontal neighbours (whatever its
//!   shape), reads all six neighbours, and connects to dust one level up or down diagonally unless
//!   the corner cell is a block;
//! - a block is powered by whatever emits into it, and then everything that reads a block next to
//!   it (a repeater's back, any dust, a lamp) reads every one of its emitters;
//! - a base cell (the strips, the chain, the s column) emits into and reads all six neighbours.
//! Dust steps reserve their corner cell as air. Every edge (emitter, reader) must be (lane i,
//! lane i+1), dust next to dust of its own lane one index apart, or a lane's element into its own
//! lamp.

use crate::abuild::{B, D, P, add};
use anyhow::{Result, bail};
use std::collections::{BinaryHeap, HashMap};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Rep(D),
    Dust,
    /// A stone: a lane's turn block (an element), a support, or the base's own stone.
    Block,
    Lamp,
    Base,
    Air,
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub kind: Kind,
    pub lane: i32,
    pub idx: i32,
    /// a support stone (not an element of its lane)
    pub support: bool,
    /// the lane's source repeater (a base component: its edges with the base are not checked)
    pub source: bool,
}

const UP: P = [0, 1, 0];
const DN: P = [0, -1, 0];
const HZ: [D; 4] = [D::N, D::S, D::W, D::E];

fn nb6(p: P) -> [P; 6] {
    [add(p, [1, 0, 0]), add(p, [-1, 0, 0]), add(p, UP), add(p, DN), add(p, [0, 0, 1]), add(p, [0, 0, -1])]
}

fn perp(a: D, b: D) -> bool {
    matches!((a, b), (D::N | D::S, D::W | D::E) | (D::W | D::E, D::N | D::S))
}

/// The world as the router sees it: committed cells plus the path under construction.
pub struct World {
    pub cells: HashMap<P, Cell>,
    pub lo: P,
    pub hi: P,
}

struct View<'a> {
    w: &'a World,
    over: &'a [(P, Cell)],
}

impl View<'_> {
    fn get(&self, p: P) -> Option<Cell> {
        for (q, c) in self.over.iter().rev() {
            if *q == p {
                return Some(*c);
            }
        }
        self.w.cells.get(&p).copied()
    }
    /// Cells `p` emits into (blocks: only when powered, handled separately).
    fn emits(&self, p: P, c: &Cell) -> Vec<P> {
        match c.kind {
            Kind::Rep(d) => vec![add(p, d.v())],
            Kind::Dust => vec![add(p, DN), add(p, [1, 0, 0]), add(p, [-1, 0, 0]), add(p, [0, 0, 1]), add(p, [0, 0, -1])],
            Kind::Base => nb6(p).to_vec(),
            _ => vec![],
        }
    }
    /// Does the component `cb` at `pb` react to power from `pa` (a diode-like emitter when `diode`)?
    fn reads(&self, pb: P, cb: &Cell, pa: P, diode: bool) -> bool {
        match cb.kind {
            Kind::Rep(d) => {
                let back = add(pb, [-d.v()[0], 0, -d.v()[2]]);
                if pa == back {
                    return true;
                }
                if pa[1] != pb[1] {
                    return false;
                }
                let dv = [pa[0] - pb[0], 0, pa[2] - pb[2]];
                let side = HZ.iter().any(|h| h.v() == dv && perp(*h, d));
                side && diode
            }
            Kind::Dust | Kind::Lamp | Kind::Base => true,
            _ => false,
        }
    }
    fn allowed(a: &Cell, b: &Cell) -> bool {
        if a.kind == Kind::Base && b.kind == Kind::Base {
            return true;
        }
        if (a.kind == Kind::Base && b.source) || (b.kind == Kind::Base && a.source) {
            return true;
        }
        if a.kind == Kind::Base || b.kind == Kind::Base || a.lane != b.lane {
            return false;
        }
        if b.kind == Kind::Lamp {
            return true;
        }
        if b.idx == a.idx + 1 {
            return true;
        }
        if a.kind == Kind::Dust && b.kind == Kind::Dust && (a.idx - b.idx).abs() <= 1 {
            return true;
        }
        // dust powering back into its own feeding turn block (no loop: dust ignores that power)
        a.kind == Kind::Dust && b.kind == Kind::Block && !b.support && b.idx == a.idx - 1
    }
    /// Check every interaction around the cells `ps`.
    fn ok_around(&self, ps: &[P]) -> bool {
        let mut s: Vec<P> = Vec::new();
        for p in ps {
            s.push(*p);
            s.extend(nb6(*p));
        }
        s.sort();
        s.dedup();
        for a in &s {
            let Some(ca) = self.get(*a) else { continue };
            match ca.kind {
                Kind::Rep(_) | Kind::Dust | Kind::Base => {
                    let diode = matches!(ca.kind, Kind::Rep(_) | Kind::Base);
                    for b in self.emits(*a, &ca) {
                        let Some(cb) = self.get(b) else { continue };
                        if cb.kind == Kind::Block || cb.kind == Kind::Air {
                            continue;
                        }
                        if self.reads(b, &cb, *a, diode) && !Self::allowed(&ca, &cb) {
                            return false;
                        }
                    }
                    // dust connections, diagonally up and down
                    if ca.kind == Kind::Dust {
                        for h in HZ {
                            for dy in [-1, 1] {
                                let b = add(add(*a, h.v()), [0, dy, 0]);
                                let Some(cb) = self.get(b) else { continue };
                                if cb.kind != Kind::Dust {
                                    continue;
                                }
                                let corner = if dy < 0 { add(*a, h.v()) } else { add(*a, UP) };
                                let cut = self.get(corner).is_some_and(|c| c.kind == Kind::Block || c.kind == Kind::Base);
                                if !cut && !Self::allowed(&ca, &cb) {
                                    return false;
                                }
                            }
                        }
                    }
                }
                Kind::Block => {
                    // emitters and readers of this block
                    let mut em: Vec<Cell> = Vec::new();
                    let mut rd: Vec<(P, Cell)> = Vec::new();
                    for q in nb6(*a) {
                        let Some(cq) = self.get(q) else { continue };
                        if matches!(cq.kind, Kind::Rep(_) | Kind::Dust | Kind::Base) && self.emits(q, &cq).contains(a) {
                            em.push(cq);
                        }
                        if matches!(cq.kind, Kind::Rep(_) | Kind::Dust | Kind::Lamp | Kind::Base) && self.reads(q, &cq, *a, false) {
                            rd.push((q, cq));
                        }
                    }
                    if em.is_empty() {
                        continue;
                    }
                    let element = !ca.support && ca.kind == Kind::Block && ca.lane >= 0;
                    for e in &em {
                        if element && !Self::allowed(e, &ca) {
                            return false;
                        }
                        for (_, r) in &rd {
                            if same(e, r) {
                                continue;
                            }
                            let ok = if element { Self::allowed(&ca, r) } else { Self::allowed(e, r) };
                            if !ok {
                                return false;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        true
    }
}

fn same(a: &Cell, b: &Cell) -> bool {
    a.lane == b.lane && a.idx == b.idx && a.kind == b.kind && a.support == b.support && a.lane >= 0
}

/// A lane to route: from the source repeater at `src` (already in the world) to the lamp at `lamp`
/// (the lamp is fed from behind: the last element sits at lamp + (0, 0, 1)).
#[derive(Clone, Copy, Debug)]
pub struct Lane {
    pub src: P,
    pub src_dir: D,
    pub lamp: P,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
enum St {
    Rep(u8),
    Blk,
    Dust(u8),
}

#[derive(Clone)]
struct Node {
    p: P,
    st: St,
    cost: i32,
    parent: usize,
    /// cells this step adds (the element first)
    add: Vec<(P, Cell)>,
}

fn dcode(d: D) -> u8 {
    match d {
        D::N => 0,
        D::S => 1,
        D::W => 2,
        D::E => 3,
    }
}
fn dfrom(c: u8) -> D {
    [D::N, D::S, D::W, D::E][c as usize]
}

impl World {
    fn inside(&self, p: P) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
    }

    /// Route one lane; on success the lane's cells are committed and its block count returned.
    pub fn route(&mut self, lane: i32, l: &Lane, max_expand: usize) -> Option<i32> {
        let goal = add(l.lamp, [0, 0, 1]);
        let h = |p: P| -> i32 {
            let dh = (p[0] - goal[0]).abs() + (p[2] - goal[2]).abs();
            dh.max((p[1] - goal[1]).abs())
        };
        let mut nodes: Vec<Node> = vec![Node { p: l.src, st: St::Rep(dcode(l.src_dir)), cost: 0, parent: usize::MAX, add: vec![] }];
        let mut heap: BinaryHeap<(i32, i32, usize)> = BinaryHeap::new();
        heap.push((-h(l.src), 0, 0));
        let mut seen: HashMap<(P, St), i32> = HashMap::new();
        let mut expanded = 0;
        while let Some((_, _, ni)) = heap.pop() {
            expanded += 1;
            if expanded > max_expand {
                return None;
            }
            let n = nodes[ni].clone();
            if n.p == goal && matches!(n.st, St::Rep(0) | St::Blk) && ni != 0 {
                // commit
                let mut k = ni;
                let mut total = 0;
                while k != usize::MAX {
                    for (p, c) in &nodes[k].add {
                        if c.kind != Kind::Air {
                            total += 1;
                        }
                        self.cells.insert(*p, *c);
                    }
                    k = nodes[k].parent;
                }
                return Some(total);
            }
            // the path so far, as an overlay
            let mut over: Vec<(P, Cell)> = Vec::new();
            let mut k = ni;
            let mut depth = 0;
            while k != usize::MAX {
                over.extend(nodes[k].add.iter().copied());
                k = nodes[k].parent;
                depth += 1;
            }
            let idx = depth as i32; // the next element's index (the source is 0)
            for (st, add_cells, step) in self.moves(lane, idx, &n, &over, goal) {
                let p = add_cells[0].0;
                let cost = n.cost + step;
                let key = (p, st);
                if seen.get(&key).is_some_and(|c| *c <= cost) {
                    continue;
                }
                seen.insert(key, cost);
                nodes.push(Node { p, st, cost, parent: ni, add: add_cells });
                heap.push((-(cost + h(p)), -cost, nodes.len() - 1));
            }
        }
        None
    }

    /// The legal successors of node `n` (element index `idx` of `lane`).
    fn moves(&self, lane: i32, idx: i32, n: &Node, over: &[(P, Cell)], goal: P) -> Vec<(St, Vec<(P, Cell)>, i32)> {
        let el = |kind: Kind| Cell { kind, lane, idx, support: false, source: false };
        let sup = Cell { kind: Kind::Block, lane, idx, support: true, source: false };
        let air = Cell { kind: Kind::Air, lane, idx, support: false, source: false };
        let mut cands: Vec<(St, Vec<(P, Cell)>)> = Vec::new();
        let p = n.p;
        let rep_at = |q: P, d: D| -> (St, Vec<(P, Cell)>) { (St::Rep(dcode(d)), vec![(q, el(Kind::Rep(d))), (add(q, DN), sup)]) };
        let dust_at = |q: P, s: u8| -> (St, Vec<(P, Cell)>) { (St::Dust(s), vec![(q, el(Kind::Dust)), (add(q, DN), sup)]) };
        match n.st {
            St::Rep(dc) => {
                let d = dfrom(dc);
                let q = add(p, d.v());
                cands.push(rep_at(q, d));
                cands.push((St::Blk, vec![(q, el(Kind::Block))]));
                cands.push(dust_at(q, 15));
            }
            St::Blk => {
                for d in HZ {
                    let q = add(p, d.v());
                    cands.push(rep_at(q, d));
                    cands.push(dust_at(q, 15));
                }
                cands.push(dust_at(add(p, DN), 15));
            }
            St::Dust(s) => {
                for d in HZ {
                    let q = add(p, d.v());
                    if s >= 1 {
                        cands.push(rep_at(q, d));
                    }
                    if s >= 2 {
                        cands.push(dust_at(q, s - 1));
                        // down: the corner (q) stays air
                        let (st, mut v) = dust_at(add(q, DN), s - 1);
                        v.push((q, air));
                        cands.push((st, v));
                        // up: the cell above p stays air, q is the upper dust's support
                        let (st, mut v) = dust_at(add(q, UP), s - 1);
                        v.push((add(p, UP), air));
                        cands.push((st, v));
                    }
                }
            }
        }
        let view = View { w: self, over };
        let mut out = Vec::new();
        'c: for (st, cells) in cands {
            // the goal is fed from behind only: an element at the goal must be a north repeater or
            // a block; nothing else may stand there
            let e = cells[0].0;
            if e == goal && !matches!(st, St::Rep(0) | St::Blk) {
                continue;
            }
            let mut step = 0;
            let mut adds: Vec<(P, Cell)> = Vec::new();
            for (k, (q, c)) in cells.iter().enumerate() {
                if !self.inside(*q) {
                    continue 'c;
                }
                match view.get(*q) {
                    None => {
                        adds.push((*q, *c));
                        if c.kind != Kind::Air {
                            step += 1;
                        }
                    }
                    Some(ex) => {
                        if k == 0 {
                            continue 'c;
                        }
                        let ok = match c.kind {
                            // a support may be any stone that exists (the base's stone too)
                            Kind::Block => ex.kind == Kind::Block || (ex.kind == Kind::Base && self.base_stone(*q)),
                            Kind::Air => ex.kind == Kind::Air,
                            _ => false,
                        };
                        if !ok {
                            continue 'c;
                        }
                    }
                }
            }
            // goal cell reserved for its lamp's lane: nothing else may end up powering into a lamp,
            // which ok_around checks; here: nothing but the goal element at a lamp's back cell
            let mut over2: Vec<(P, Cell)> = over.to_vec();
            over2.extend(adds.iter().copied());
            let v2 = View { w: self, over: &over2 };
            let ps: Vec<P> = cells.iter().map(|x| x.0).collect();
            if !v2.ok_around(&ps) {
                continue;
            }
            if adds.is_empty() || adds[0].0 != e {
                continue;
            }
            out.push((st, adds, step));
        }
        out
    }

    fn base_stone(&self, _q: P) -> bool {
        false
    }
}

/// Build the world from a design under construction: every block is base except the sources.
pub fn world_from(b: &B, lanes: &[Lane], lo: P, hi: P) -> World {
    let mut cells = HashMap::new();
    for p in b.blocks.keys() {
        cells.insert(*p, Cell { kind: Kind::Base, lane: -1, idx: 0, support: false, source: false });
    }
    for (k, l) in lanes.iter().enumerate() {
        cells.insert(l.src, Cell { kind: Kind::Rep(l.src_dir), lane: k as i32, idx: 0, support: false, source: true });
        cells.insert(l.lamp, Cell { kind: Kind::Lamp, lane: k as i32, idx: 1000, support: false, source: false });
    }
    World { cells, lo, hi }
}

/// Route every lane in `order`; returns the total block count.
pub fn route_all(w: &mut World, lanes: &[Lane], order: &[usize], max_expand: usize) -> Result<i32> {
    let mut tot = 0;
    for &k in order {
        match w.route(k as i32, &lanes[k], max_expand) {
            Some(n) => tot += n,
            None => bail!("lane {k} does not route"),
        }
    }
    Ok(tot)
}

/// Write the routed lanes into the design.
pub fn write_lanes(w: &World, b: &mut B) {
    if std::env::var("MCTAS_ROUTE_DEBUG").is_ok() {
        let mut v: Vec<(i32, i32, bool, P, Kind)> = w.cells.iter().filter(|(_, c)| c.lane >= 0).map(|(p, c)| (c.lane, c.idx, c.support, *p, c.kind)).collect();
        v.sort_by_key(|x| (x.0, x.1, x.2));
        for (l, i, s, p, k) in v {
            println!("lane {l} #{i}{} {p:?} {k:?}", if s { " support" } else { "" });
        }
    }
    for (p, c) in &w.cells {
        if c.lane < 0 || c.source || c.kind == Kind::Lamp {
            continue;
        }
        match c.kind {
            Kind::Rep(d) => b.rep(*p, d, 1),
            Kind::Dust => b.dust(*p),
            Kind::Block => b.stone(*p),
            _ => {}
        }
    }
}
