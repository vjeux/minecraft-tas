//! The analog stack (`mctas circuit display analog-stack`): the chain s = 15 - sum(w_i lit_i)
//! (crates/circuit-display/src/analog.rs codes), then one cross ring per segment: the lamp in the
//! middle lit through 4 blocks, each block driven by a window comparator W at distance 2 (rear a
//! hopper constant k). W's sides: an s contact (dust) and an offset comparator O (rear a hopper
//! constant C, side an s contact) giving C - s + d1, so W is on iff s in [C + d1 - k + 1, k + d2 - 1].
//! One s network per ring: radius-3 arcs bridged under the rear hoppers one level down.

use crate::abuild::{B, D, P, add};
use crate::analog::{Code, Run, masks, runs};
use std::collections::{BTreeMap, VecDeque};

/// Rotate a ring-relative (x, z) for unit u (0 E, 1 N, 2 W, 3 S).
fn rot(u: usize, x: i32, z: i32) -> (i32, i32) {
    match u {
        0 => (x, z),
        1 => (z, -x),
        2 => (-x, -z),
        _ => (-z, x),
    }
}

fn rotd(u: usize, d: D) -> D {
    let v = d.v();
    let (x, z) = rot(u, v[0], v[2]);
    match (x, z) {
        (0, -1) => D::N,
        (0, 1) => D::S,
        (-1, 0) => D::W,
        _ => D::E,
    }
}

/// What one slot of a ring does.
#[derive(Clone, Copy, Debug)]
pub enum Slot {
    Empty,
    /// inner window: W + O (k, C)
    Inner { k: u8, c: u8 },
    /// top run: W + O, no s contact on W
    Top { k: u8, c: u8 },
    /// bottom run: W with its s contact, no O
    Bottom { k: u8 },
}

/// The dust cells of a ring (ring-relative x, dy, z), given which slots have an O and a W contact.
fn ring_dust(slots: &[Slot; 4]) -> Vec<(i32, i32, i32)> {
    let mut v = ring_dust_all(slots);
    let removed = REMOVED.with(|r| r.borrow().clone());
    v.retain(|c| !removed.contains(c));
    v
}

fn ring_dust_all(slots: &[Slot; 4]) -> Vec<(i32, i32, i32)> {
    let mut v = Vec::new();
    // arcs: radius 3 ring minus the rear hoppers on the axes
    for i in -3..=3 {
        for (x, z) in [(3, i), (-3, i), (i, 3), (i, -3)] {
            if x == 0 || z == 0 {
                continue;
            }
            if !v.contains(&(x, 0, z)) {
                v.push((x, 0, z));
            }
        }
    }
    // (the arcs are joined under the rear hoppers by comparator bridges: `BRIDGES`)
    let _ = ();
    for (u, s) in slots.iter().enumerate() {
        if matches!(s, Slot::Inner { .. } | Slot::Bottom { .. }) {
            let (x, z) = rot(u, 2, -1);
            v.push((x, 0, z));
        }
    }
    v
}

/// The comparator bridges under the rear hoppers: the arc cells they join (ring-relative, level 0).
pub const BRIDGES: [((i32, i32, i32), (i32, i32, i32)); 4] = [((3, 0, -1), (3, 0, 1)), ((-3, 0, -1), (-3, 0, 1)), ((-1, 0, 3), (1, 0, 3)), ((-1, 0, -3), (1, 0, -3))];

/// Distances over a ring's dust from a feed cell (ring-relative): dust steps cost 1, a bridge 0
/// (it copies the support block's level to the other side).
pub type Feed = [((i32, i32, i32), u8)];

fn distances(dust: &[(i32, i32, i32)], feed: &Feed) -> BTreeMap<(i32, i32, i32), u8> {
    let mut d = BTreeMap::new();
    let mut q = VecDeque::new();
    let mut f = feed.to_vec();
    f.sort_by_key(|x| x.1);
    for (c, v) in f {
        if !dust.contains(&c) {
            continue;
        }
        if d.get(&c).is_none_or(|&o| o > v) {
            d.insert(c, v);
            q.push_back(c);
        }
    }
    while let Some(p) = q.pop_front() {
        let dp = d[&p];
        let cm = COPY_MASK.with(|c| c.get());
        for (u, &(a, b)) in COPIES.iter().enumerate() {
            // the copy needs its W contact cell to exist (a dust cell of the ring)
            if cm >> u & 1 == 1 && a == p && dust.contains(&b) && d.get(&b).is_none_or(|&v| v > dp) {
                d.insert(b, dp);
                q.push_front(b);
            }
        }
        for (bi, &(a, b)) in BRIDGES.iter().enumerate() {
            if NO_BRIDGE.with(|c| c.get()) == bi || (COPY_MASK.with(|c| c.get()) >> (4 + bi)) & 1 == 1 {
                continue;
            }
            for (x, y) in [(a, b), (b, a)] {
                if x == p && d.get(&y).is_none_or(|&v| v > dp) {
                    d.insert(y, dp);
                    q.push_front(y);
                }
            }
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (p.0 + dx, p.1, p.2 + dz);
            if dust.contains(&n) && d.get(&n).is_none_or(|&v| v > dp + 1) {
                d.insert(n, dp + 1);
                q.push_back(n);
            }
        }
    }
    d
}

/// Assign a segment's runs to slots and constants, given distances; None when nothing fits.
pub fn solve_ring(rs: &[Run], feed: &Feed) -> Option<[Slot; 4]> {
    solve_ring_c(rs, feed, 0).map(|x| x.0)
}

/// The copy comparators under the corner hoppers (bit u of a mask): arc cell -> W contact.
pub const COPIES: [((i32, i32, i32), (i32, i32, i32)); 4] = [((2, 0, -3), (2, 0, -1)), ((-3, 0, -2), (-1, 0, -2)), ((-2, 0, 3), (-2, 0, 1)), ((3, 0, 2), (1, 0, 2))];

/// solve_ring with any subset of the `allowed` copies; returns the slots and the copies used.
pub fn solve_ring_c(rs: &[Run], feed: &Feed, allowed: u8) -> Option<([Slot; 4], u8)> {
    solve_ring_f(rs, feed, allowed, 0)
}

/// solve_ring_c with the bridges in `forced_off` (bit i: bridge i) left out.
pub fn solve_ring_f(rs: &[Run], feed: &Feed, allowed: u8, forced_off: u8) -> Option<([Slot; 4], u8)> {
    let mut best: Option<(u32, [Slot; 4], u8)> = None;
    // low 4 bits: copies used; high 4 bits: bridges left out
    for m in 0..=255u8 {
        if m & 15 & !allowed != 0 || (m >> 4) & forced_off != forced_off {
            continue;
        }
        COPY_MASK.with(|c| c.set(m));
        let r = solve_ring_inner(rs, feed);
        COPY_MASK.with(|c| c.set(0));
        if let Some((sc, s)) = r {
            if !scoring() {
                return Some((s, m));
            }
            if best.as_ref().is_none_or(|b: &(u32, [Slot; 4], u8)| sc < b.0) {
                best = Some((sc, s, m));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

/// The glitch scorer's transitions (s before, s after, lit before, lit after) for the segment
/// being solved; empty = no scoring (the first fitting layout wins).
thread_local! {
    pub static SCORE_PAIRS: std::cell::RefCell<Vec<(u8, u8, bool, bool)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Feed cell times (gt after s changes at the chain), for the scorer.
    pub static FEED_T: std::cell::RefCell<BTreeMap<(i32, i32, i32), u32>> = const { std::cell::RefCell::new(BTreeMap::new()) };
}

fn scoring() -> bool {
    SCORE_PAIRS.with(|p| !p.borrow().is_empty())
}

/// Distances with arrival times: the dominant (shortest) path's time, bridges and copies 2 gt.
fn distances_t(dust: &[(i32, i32, i32)], feed: &Feed) -> BTreeMap<(i32, i32, i32), (u8, u32)> {
    let ft = FEED_T.with(|f| f.borrow().clone());
    let mut d: BTreeMap<(i32, i32, i32), (u8, u32)> = BTreeMap::new();
    let mut heap = std::collections::BinaryHeap::new();
    for &(c, v) in feed {
        if !dust.contains(&c) {
            continue;
        }
        let t = ft.get(&c).copied().unwrap_or(0);
        heap.push(std::cmp::Reverse((v, t, c)));
    }
    let cm = COPY_MASK.with(|c| c.get());
    while let Some(std::cmp::Reverse((v, t, p))) = heap.pop() {
        if d.contains_key(&p) {
            continue;
        }
        d.insert(p, (v, t));
        for (u, &(a, b)) in COPIES.iter().enumerate() {
            if cm >> u & 1 == 1 && a == p && dust.contains(&b) {
                heap.push(std::cmp::Reverse((v, t + 2, b)));
            }
        }
        for (bi, &(a, b)) in BRIDGES.iter().enumerate() {
            if (cm >> (4 + bi)) & 1 == 1 {
                continue;
            }
            for (x, y) in [(a, b), (b, a)] {
                if x == p {
                    heap.push(std::cmp::Reverse((v, t + 2, y)));
                }
            }
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (p.0 + dx, p.1, p.2 + dz);
            if dust.contains(&n) {
                heap.push(std::cmp::Reverse((v + 1, t, n)));
            }
        }
    }
    d
}

/// How many of the scorer's transitions make this ring's lamp glitch.
fn glitch_score(slots: &[Slot; 4], dt: &BTreeMap<(i32, i32, i32), (u8, u32)>) -> u32 {
    let pairs = SCORE_PAIRS.with(|p| p.borrow().clone());
    let mut n = 0;
    for (sa, sb, la, lb) in pairs {
        let on = |t: i32| -> bool {
            (0..4).any(|u| {
                let tau = t - 2;
                let wc = dt.get(&{
                    let (x, z) = rot(u, 2, -1);
                    (x, 0, z)
                });
                let oc = dt.get(&{
                    let (x, z) = rot(u, 3, 1);
                    (x, 0, z)
                });
                let sv = |time: u32| -> i32 { if tau >= time as i32 { sb as i32 } else { sa as i32 } };
                let wside = |d: u8, time: u32| (sv(time) - d as i32).max(0);
                let oside = |c: u8, d: u8, time: u32| {
                    let x = (sv(time + 2) - d as i32).max(0);
                    if c as i32 >= x { c as i32 - x } else { 0 }
                };
                match slots[u] {
                    Slot::Empty => false,
                    Slot::Inner { k, c } => match (wc, oc) {
                        (Some(&(d2, t2)), Some(&(d1, t1))) => (k as i32) > wside(d2, t2).max(oside(c, d1, t1)),
                        _ => false,
                    },
                    Slot::Top { k, c } => oc.is_some_and(|&(d1, t1)| (k as i32) > oside(c, d1, t1)),
                    Slot::Bottom { k } => wc.is_some_and(|&(d2, t2)| (k as i32) > wside(d2, t2)),
                }
            })
        };
        let _ = la;
        let mut last = on(-100);
        let mut changes = 0;
        for t in 0..60 {
            let v = on(t);
            if v != last {
                changes += 1;
                last = v;
            }
        }
        if changes > usize::from(on(-100) != on(100)) || on(100) != lb {
            n += 1;
        }
    }
    n
}

thread_local! {
    static COPY_MASK: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static NO_BRIDGE: std::cell::Cell<usize> = const { std::cell::Cell::new(99) };
    pub static FORBID: std::cell::RefCell<Vec<(i32, i32, i32)>> = const { std::cell::RefCell::new(Vec::new()) };
    /// Arc cells left out of the ring (something above stands there on a stone).
    pub static REMOVED: std::cell::RefCell<Vec<(i32, i32, i32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn solve_ring_inner(rs: &[Run], feed: &Feed) -> Option<(u32, [Slot; 4])> {
    let n = rs.len();
    if n > 4 {
        return None;
    }
    let mut best: Option<(u32, [Slot; 4])> = None;
    let perms = perms4(n);
    for p in perms {
        let mut slots = [Slot::Empty; 4];
        // first pass: kinds, to know which W contacts exist
        for (r, &u) in rs.iter().zip(&p) {
            slots[u] = if r.lo == 0 { Slot::Bottom { k: 0 } } else if r.hi == 15 { Slot::Top { k: 0, c: 0 } } else { Slot::Inner { k: 0, c: 0 } };
        }
        let dust = ring_dust(&slots);
        // cells something above must stand on: no dust or comparator there
        let forbid = FORBID.with(|f| f.borrow().clone());
        if !forbid.is_empty() {
            let mut soft: Vec<(i32, i32, i32)> = dust.clone();
            for (u, s) in slots.iter().enumerate() {
                if matches!(s, Slot::Empty) {
                    continue;
                }
                let (x, z) = rot(u, 2, 0);
                soft.push((x, 0, z));
                if matches!(s, Slot::Inner { .. } | Slot::Top { .. }) {
                    let (x, z) = rot(u, 2, 1);
                    soft.push((x, 0, z));
                }
            }
            if forbid.iter().any(|c| soft.contains(c)) {
                continue;
            }
        }
        let dist = distances(&dust, feed);
        let mut ok = true;
        let mut score = 0u32;
        for (r, &u) in rs.iter().zip(&p) {
            let (ox, oz) = rot(u, 3, 1);
            let (wx, wz) = rot(u, 2, -1);
            let d1 = dist.get(&(ox, 0, oz)).copied();
            let d2 = dist.get(&(wx, 0, wz)).copied();
            let (lo, hi) = (r.lo as i32, r.hi as i32);
            slots[u] = match slots[u] {
                Slot::Inner { .. } => {
                    let (Some(d1), Some(d2)) = (d1, d2) else {
                        ok = false;
                        break;
                    };
                    let (d1, d2) = (d1 as i32, d2 as i32);
                    let k = hi + 1 - d2;
                    let c = lo + hi - d1 - d2;
                    if !(1..=15).contains(&k) || c < k || c > 15 {
                        ok = false;
                        break;
                    }
                    score += (d1 + d2) as u32;
                    Slot::Inner { k: k as u8, c: c as u8 }
                }
                Slot::Top { .. } => {
                    let Some(d1) = d1 else {
                        ok = false;
                        break;
                    };
                    let c = lo - d1 as i32;
                    if c < 1 {
                        ok = false;
                        break;
                    }
                    Slot::Top { k: 1, c: c as u8 }
                }
                Slot::Bottom { .. } => {
                    let Some(d2) = d2 else {
                        ok = false;
                        break;
                    };
                    let k = hi + 1 - d2 as i32;
                    if !(1..=15).contains(&k) {
                        ok = false;
                        break;
                    }
                    Slot::Bottom { k: k as u8 }
                }
                Slot::Empty => Slot::Empty,
            };
        }
        if ok {
            if scoring() {
                score += 1000 * glitch_score(&slots, &distances_t(&dust, feed));
            }
            if best.as_ref().is_none_or(|b| score < b.0) {
                best = Some((score, slots));
            }
        }
    }
    best
}

fn perms4(n: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    fn rec(n: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == n {
            out.push(cur.clone());
            return;
        }
        for p in 0..4 {
            if !cur.contains(&p) {
                cur.push(p);
                rec(n, cur, out);
                cur.pop();
            }
        }
    }
    rec(n, &mut Vec::new(), &mut out);
    out
}

/// Build one ring at lamp `c` (world), with its dust.
pub fn build_ring(b: &mut B, c: P, slots: &[Slot; 4], feed: &Feed, copies: u8) {
    let at = |x: i32, dy: i32, z: i32| add(c, [x, dy, z]);
    for (x, z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        b.stone(at(x, 0, z));
    }
    for (u, s) in slots.iter().enumerate() {
        let p = |x: i32, z: i32| {
            let (rx, rz) = rot(u, x, z);
            at(rx, 0, rz)
        };
        let (k, o) = match *s {
            Slot::Empty => continue,
            Slot::Inner { k, c } | Slot::Top { k, c } => (k, Some(c)),
            Slot::Bottom { k } => (k, None),
        };
        b.comp(p(2, 0), rotd(u, D::W), true);
        b.hopper(p(3, 0), k);
        if let Some(cc) = o {
            b.comp(p(2, 1), rotd(u, D::N), true);
            b.hopper(p(2, 2), cc);
        }
    }
    for (x, dy, z) in ring_dust(slots) {
        b.dust(at(x, dy, z));
    }
    COPY_MASK.with(|c| c.set(copies));
    let dist = distances(&ring_dust(slots), feed);
    COPY_MASK.with(|c| c.set(0));
    for (u, &(a, cc)) in COPIES.iter().enumerate() {
        if copies >> u & 1 == 0 {
            continue;
        }
        let (kx, kz) = rot(u, 2, -2);
        let dir = match ((cc.0 - a.0).signum(), (cc.2 - a.2).signum()) {
            (1, _) => D::E,
            (-1, _) => D::W,
            (_, 1) => D::S,
            _ => D::N,
        };
        b.comp(at(kx, -1, kz), dir, true);
        b.stone(at(a.0, -1, a.2));
        b.stone(at(cc.0, -1, cc.2));
    }
    for (bi, &(p, q)) in BRIDGES.iter().enumerate() {
        if (copies >> (4 + bi)) & 1 == 1 {
            continue;
        }
        let (dp, dq) = (dist.get(&p).copied().unwrap_or(99), dist.get(&q).copied().unwrap_or(99));
        let p_first = if dp != dq {
            dp < dq
        } else {
            // a tie means the bridge itself carried it: the source is the side nearer without it
            COPY_MASK.with(|c| c.set(copies));
            NO_BRIDGE.with(|c| c.set(bi));
            let d2 = distances(&ring_dust(slots), feed);
            NO_BRIDGE.with(|c| c.set(99));
            COPY_MASK.with(|c| c.set(0));
            d2.get(&p).copied().unwrap_or(99) <= d2.get(&q).copied().unwrap_or(99)
        };
        let (from, to) = if p_first { (p, q) } else { (q, p) };
        let mid = at((from.0 + to.0) / 2, -1, (from.2 + to.2) / 2);
        let dir = match ((to.0 - from.0).signum(), (to.2 - from.2).signum()) {
            (1, _) => D::E,
            (-1, _) => D::W,
            (_, 1) => D::S,
            _ => D::N,
        };
        b.comp(mid, dir, true);
        b.stone(at(from.0, -1, from.2));
        b.stone(at(to.0, -1, to.2));
    }
}

/// The chain (analog-chain-test layout) at `o`; returns the cell holding s (dust on the block).
pub fn build_chain(b: &mut B, o: P, code: &Code) -> P {
    // stage i at x = 1 + i reads gen i; gens: 0 south, 1 north, 2 north, 3 south (inputs west/east)
    let p = |x: i32, y: i32, z: i32| add(o, [x, y, z]);
    b.hopper(p(0, 1, 0), 15);
    for i in 0..4 {
        b.comp(p(1 + i, 1, 0), D::E, true);
    }
    b.stone(p(5, 1, 0));
    // gen i serves stage i: weight w of the bit wired to it; all bits complemented here (neg = 1111
    // relative to the gen: out = w (1 - bit)) -> a code with neg bit 0 needs the plain form, unsupported
    let gens = [(1, 1, D::N), (2, -1, D::S), (3, -1, D::S), (4, 1, D::N)];
    let order = [0usize, 1, 2, 3]; // input bit wired to stage i
    for (i, &(x, z, out)) in gens.iter().enumerate() {
        let bit = order[i];
        b.comp(p(x, 1, z), out, true);
        b.composter(p(x, 1, z + if z > 0 { 1 } else { -1 }), code.w[bit]);
    }
    // inputs: stage 0 side from (0,1,1) dust; stage 1 via repeater (1,1,-1) from (0,1,-1);
    // stage 2 via repeaters (4,1,-1) <- (5,1,-1); stage 3 via repeater (5,1,1)
    b.dust(p(0, 1, 1));
    b.port(&format!("input.a{}", order[0]), p(0, 1, 1));
    b.rep(p(1, 1, -1), D::E, 1);
    b.dust(p(0, 1, -1));
    b.port(&format!("input.a{}", order[1]), p(0, 1, -1));
    b.rep(p(4, 1, -1), D::W, 1);
    b.rep(p(5, 1, -1), D::W, 1);
    b.port(&format!("input.a{}", order[2]), p(5, 1, -1));
    b.rep(p(5, 1, 1), D::W, 1);
    b.port(&format!("input.a{}", order[3]), p(5, 1, 1));
    for x in 0..6 {
        for z in -1..=1 {
            let q = p(x, 0, z);
            if !b.blocks.contains_key(&q) {
                b.stone(q);
            }
        }
    }
    let s = p(5, 2, 0);
    b.dust(s);
    s
}

pub fn seg_runs(code: &Code, seg: usize) -> Vec<Run> {
    runs(masks(code)[seg])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analog::{all_codes, cost};

    #[test]
    #[ignore]
    fn feed_search() {
        let all = [Slot::Inner { k: 0, c: 0 }; 4];
        let cells = ring_dust(&all);
        let mut codes: Vec<(Code, (usize, usize))> = all_codes().into_iter().map(|c| (c, cost(&c))).collect();
        codes.retain(|(c, _)| masks(c).iter().all(|m| runs(*m).len() <= 4));
        codes.sort_by_key(|x| x.1);
        for (code, k) in codes {
            let mut ok = 0;
            let mut feeds = Vec::new();
            for m in masks(&code) {
                let rs = runs(m);
                let f = cells.iter().find(|&&c| solve_ring(&rs, &[(c, 0)]).is_some());
                let f = f.map(|c| format!("{c:?}")).or_else(|| {
                    for (i, &a) in cells.iter().enumerate() {
                        for &b in &cells[i + 1..] {
                            if solve_ring(&rs, &[(a, 0), (b, 0)]).is_some() {
                                return Some(format!("{a:?}+{b:?}"));
                            }
                        }
                    }
                    None
                });
                if let Some(f) = f {
                    ok += 1;
                    feeds.push(f);
                } else {
                    feeds.push("-".into());
                }
            }
            if ok >= 6 {
                println!("FEED {ok}/7 w {:?} neg {:04b} {k:?}: {}", code.w, code.neg, feeds.join(" "));
            }
        }
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    #[ignore]
    fn feed_slack() {
        let all = [Slot::Inner { k: 0, c: 0 }; 4];
        let cells = ring_dust(&all);
        for (w, neg) in [([4u8, 1, 8, 2], 15u8), ([8, 1, 4, 2], 1), ([2, 8, 1, 4], 15)] {
            let code = Code { w, neg };
            let mut line = format!("SLACK {w:?} {neg:04b}:");
            for m in masks(&code) {
                let rs = runs(m);
                let mut best = -1i32;
                for f in 0..6u8 {
                    if cells.iter().any(|&c| solve_ring(&rs, &[(c, f)]).is_some()) {
                        best = f as i32;
                    }
                }
                line.push_str(&format!(" {best}"));
            }
            println!("{line}");
        }
    }
}

#[cfg(test)]
mod tests3 {
    use super::*;
    use crate::analog::{all_codes, cost};

    #[test]
    #[ignore]
    fn riser_feeds() {
        let mut codes: Vec<(Code, (usize, usize))> = all_codes().into_iter().map(|c| (c, cost(&c))).collect();
        codes.retain(|(c, _)| masks(c).iter().all(|m| runs(*m).len() <= 4));
        codes.sort_by_key(|x| x.1);
        for (code, k) in codes {
            let mut n = 0;
            let mut s = String::new();
            for m in masks(&code) {
                let rs = runs(m);
                let a = solve_ring_c(&rs, &[((3, 0, -2), 0)], 15).is_some();
                let b = solve_ring_c(&rs, &[((3, 0, 2), 0)], 15).is_some();
                if a || b {
                    n += 1;
                }
                s.push_str(match (a, b) { (true, true) => " B", (true, false) => " P", (false, true) => " Q", _ => " -" });
            }
            if n >= 6 {
                println!("RISER {n}/7 w {:?} neg {:04b} {k:?}:{s}", code.w, code.neg);
            }
        }
    }
}

#[cfg(test)]
mod tests4 {
    use super::*;

    #[test]
    #[ignore]
    fn g_pairs() {
        let all = [Slot::Inner { k: 0, c: 0 }; 4];
        let cells = ring_dust(&all);
        let code = Code { w: [8, 1, 4, 2], neg: 1 };
        let rs = runs(masks(&code)[6]);
        for (i, &a) in cells.iter().enumerate() {
            for &b in &cells[i + 1..] {
                if solve_ring(&rs, &[(a, 0), (b, 0)]).is_some() {
                    println!("GPAIR {a:?} {b:?}");
                }
            }
        }
    }
}

/// The distances a ring's dust should carry (ring-relative), as the builder wires it.
pub fn ring_distances(slots: &[Slot; 4], feed: &Feed, copies: u8) -> BTreeMap<(i32, i32, i32), u8> {
    COPY_MASK.with(|c| c.set(copies));
    let d = distances(&ring_dust(slots), feed);
    COPY_MASK.with(|c| c.set(0));
    d
}

/// The bridges' orientations (from, to) as the builder wires them.
pub fn bridge_orient(slots: &[Slot; 4], feed: &Feed, copies: u8) -> Vec<((i32, i32, i32), (i32, i32, i32))> {
    let dist = ring_distances(slots, feed, copies);
    let mut out = Vec::new();
    for (bi, &(p, q)) in BRIDGES.iter().enumerate() {
        if (copies >> (4 + bi)) & 1 == 1 {
            continue;
        }
        let (dp, dq) = (dist.get(&p).copied().unwrap_or(99), dist.get(&q).copied().unwrap_or(99));
        let p_first = if dp != dq {
            dp < dq
        } else {
            COPY_MASK.with(|c| c.set(copies));
            NO_BRIDGE.with(|c| c.set(bi));
            let d2 = distances(&ring_dust(slots), feed);
            NO_BRIDGE.with(|c| c.set(99));
            COPY_MASK.with(|c| c.set(0));
            d2.get(&p).copied().unwrap_or(99) <= d2.get(&q).copied().unwrap_or(99)
        };
        out.push(if p_first { (p, q) } else { (q, p) });
    }
    out
}

/// What a ring's support-level comparators (bridges, copies) put into the ring below: the cells
/// right under their output blocks (ring-relative, level 0) and the distance they carry.
pub fn coupling(slots: &[Slot; 4], feed: &Feed, copies: u8) -> Vec<((i32, i32, i32), u8)> {
    let dist = ring_distances(slots, feed, copies);
    let mut out = Vec::new();
    for (from, to) in bridge_orient(slots, feed, copies) {
        if let Some(&v) = dist.get(&from) {
            out.push((to, v));
        }
    }
    for (u, &(a, c)) in COPIES.iter().enumerate() {
        if copies >> u & 1 == 1
            && let Some(&v) = dist.get(&a)
        {
            out.push((c, v));
        }
    }
    out
}

/// Whether a ring-relative cell is dust in a ring with these slots.
pub fn is_dust(slots: &[Slot; 4], c: (i32, i32, i32)) -> bool {
    ring_dust(slots).contains(&c)
}

pub fn rot_pub(u: usize, x: i32, z: i32) -> (i32, i32) {
    rot(u, x, z)
}

/// coupling() with arrival times (the source cell's time + 2), using FEED_T.
pub fn coupling_t(slots: &[Slot; 4], feed: &Feed, copies: u8) -> Vec<((i32, i32, i32), u32)> {
    COPY_MASK.with(|c| c.set(copies));
    let dt = distances_t(&ring_dust(slots), feed);
    COPY_MASK.with(|c| c.set(0));
    let mut out = Vec::new();
    for (from, to) in bridge_orient(slots, feed, copies) {
        if let Some(&(_, t)) = dt.get(&from) {
            out.push((to, t + 2));
        }
    }
    for (u, &(a, c)) in COPIES.iter().enumerate() {
        if copies >> u & 1 == 1
            && let Some(&(_, t)) = dt.get(&a)
        {
            out.push((c, t + 2));
        }
    }
    out
}

/// The glitch model's count for a built ring (FEED_T and SCORE_PAIRS set by the caller).
pub fn predict(slots: &[Slot; 4], feed: &Feed, copies: u8) -> u32 {
    COPY_MASK.with(|c| c.set(copies));
    let dt = distances_t(&ring_dust(slots), feed);
    COPY_MASK.with(|c| c.set(0));
    glitch_score(slots, &dt)
}
