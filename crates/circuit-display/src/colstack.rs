//! The column stack (`mctas circuit display column-stack`): 7 cross rings every 2 levels, the
//! rings alternately mirrored, s carried down 4 lossless COLUMNS (one per axis). In a slot (E,
//! ring type A): W (2,0) window comparator out west into the lamp's stone, rear hopper k (3,0);
//! O (2,1) out north, rear hopper C (2,2), side the column cell (3,1) (dust, s - f, time T); W's
//! other side the W contact (2,-1)+(3,-1) dust, fed by the slot's bridge under k (level -1, from
//! the block under (3,1) to the block under (3,-1): s - f at T + 2, the same tick O's output
//! changes). The bridge's output block also feeds the column cell of the mirrored ring below.
//! W is on iff s in [lo, hi] with k = hi - f, C = lo + hi - 2f - 1 (inner), O alone (top runs),
//! W alone (bottom runs). The chain sits on the top ring (one level); 4 taps feed the columns.

use crate::abuild::{B, D, P, add};
use crate::agen::{Tf, chain4, merge};
use crate::analog::{Code, Run, masks, runs};
use anyhow::{Result, anyhow, bail};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A slot's job.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    Unused,
    Inner { k: u8, c: u8 },
    Top { c: u8 },
    Bottom { k: u8 },
}

/// Whether run r fits a slot whose O contact is at distance d1 and W contact at d2.
pub fn fits(r: Run, d1: i32, d2: i32) -> bool {
    let (lo, hi) = (r.lo as i32, r.hi as i32);
    if lo == 0 {
        hi + 1 - d2 >= 1
    } else if hi == 15 {
        d1 < lo
    } else {
        d1 < lo && hi + 1 - d2 >= 1 && lo + hi - d1 - d2 <= 15
    }
}

pub fn job(r: Run, d1: i32, d2: i32) -> Job {
    let (lo, hi) = (r.lo as i32, r.hi as i32);
    if lo == 0 {
        Job::Bottom { k: (hi + 1 - d2) as u8 }
    } else if hi == 15 {
        Job::Top { c: (lo - d1) as u8 }
    } else {
        Job::Inner { k: (hi + 1 - d2) as u8, c: (lo + hi - d1 - d2) as u8 }
    }
}

/// The (d1, d2) of each axis: a column ring (W contact through its own bridge) or the bottom
/// ring (W contact through a ring-level comparator from the next slot's column cell).
pub fn dists(f: [i32; 4], bottom: bool) -> [(i32, i32); 4] {
    let mut d = [(0, 0); 4];
    for a in 0..4 {
        d[a] = (f[a], if bottom { f[(a + 1) % 4] + 3 } else { f[a] + 1 });
    }
    d
}

/// A segment's axis assignment for the given distances.
pub fn assign_one(rs: &[Run], d: [(i32, i32); 4]) -> Option<[Job; 4]> {
    let n = rs.len();
    for mm in 0..256u32 {
        let a: Vec<usize> = (0..n).map(|i| (mm >> (2 * i) & 3) as usize).collect();
        let mut seen = [false; 4];
        if !a.iter().all(|&x| !std::mem::replace(&mut seen[x], true)) {
            continue;
        }
        if rs.iter().zip(&a).all(|(r, &x)| fits(*r, d[x].0, d[x].1)) {
            let mut jobs = [Job::Unused; 4];
            for (r, &x) in rs.iter().zip(&a) {
                jobs[x] = job(*r, d[x].0, d[x].1);
            }
            return Some(jobs);
        }
    }
    None
}

/// Per ring (bottom first), the segment and its jobs: the bottom ring takes a segment that fits
/// its distances, the others fit the column distances.
pub fn assign(code: &Code, f: [i32; 4]) -> Option<Vec<(usize, [Job; 4])>> {
    let segs: Vec<Vec<Run>> = masks(code).iter().map(|m| runs(*m)).collect();
    let col: Vec<Option<[Job; 4]>> = segs.iter().map(|rs| assign_one(rs, dists(f, false))).collect();
    let bot: Vec<Option<[Job; 4]>> = segs.iter().map(|rs| assign_one(rs, dists(f, true))).collect();
    for b in 0..segs.len() {
        let Some(jb) = bot[b] else { continue };
        if (0..segs.len()).all(|s| s == b || col[s].is_some()) {
            let mut out = vec![(b, jb)];
            for s in 0..segs.len() {
                if s != b {
                    out.push((s, col[s].unwrap()));
                }
            }
            return Some(out);
        }
    }
    None
}

/// Ring-relative transform of slot u (0 E, 1 N, 2 W, 3 S) in a ring of type `mirror`.
fn tr(u: usize, mirror: bool, x: i32, z: i32) -> (i32, i32) {
    let (x, z) = match u {
        0 => (x, z),
        1 => (z, -x),
        2 => (-x, -z),
        _ => (-z, x),
    };
    if mirror { (x, -z) } else { (x, z) }
}

fn trd(u: usize, mirror: bool, d: D) -> D {
    let v = d.v();
    let (x, z) = tr(u, mirror, v[0], v[2]);
    match (x, z) {
        (0, -1) => D::N,
        (0, 1) => D::S,
        (-1, 0) => D::W,
        _ => D::E,
    }
}

/// The axis (column) a slot of a ring type serves: slot u of a mirrored ring lies on axis
/// `axis_of(u, true)`.
pub fn axis_of(u: usize, mirror: bool) -> usize {
    let (x, z) = tr(u, mirror, 1, 0);
    match (x, z) {
        (1, 0) => 0,
        (0, -1) => 1,
        (-1, 0) => 2,
        _ => 3,
    }
}

/// The column cell (ring-relative) of an axis in a ring type.
pub fn column_cell(axis: usize, mirror: bool) -> (i32, i32) {
    let u = (0..4).find(|&u| axis_of(u, mirror) == axis).unwrap();
    tr(u, mirror, 3, 1)
}

/// Build one ring at center c; `jobs` indexed by axis; `bridges`: put the 4 bridges at y - 1.
pub fn build_ring(b: &mut B, c: P, mirror: bool, jobs: &[Job; 4], bridges: bool) {
    let at = |u: usize, x: i32, dy: i32, z: i32| {
        let (rx, rz) = tr(u, mirror, x, z);
        add(c, [rx, dy, rz])
    };
    for u in 0..4 {
        let j = jobs[axis_of(u, mirror)];
        // the column cell always carries s (it powers the block under it for the bridge)
        b.dust(at(u, 3, 0, 1));
        match j {
            Job::Unused => b.stone(at(u, 3, 0, 0)),
            Job::Inner { k, c: cc } => {
                b.comp(at(u, 2, 0, 0), trd(u, mirror, D::W), true);
                b.hopper(at(u, 3, 0, 0), k);
                b.comp(at(u, 2, 0, 1), trd(u, mirror, D::N), true);
                b.hopper(at(u, 2, 0, 2), cc);
                b.dust(at(u, 2, 0, -1));
                b.dust(at(u, 3, 0, -1));
                b.stone(at(u, 1, 0, 0));
            }
            Job::Top { c: cc } => {
                b.comp(at(u, 2, 0, 0), trd(u, mirror, D::W), true);
                b.hopper(at(u, 3, 0, 0), 1);
                b.comp(at(u, 2, 0, 1), trd(u, mirror, D::N), true);
                b.hopper(at(u, 2, 0, 2), cc);
                b.stone(at(u, 1, 0, 0));
            }
            Job::Bottom { k } => {
                b.comp(at(u, 2, 0, 0), trd(u, mirror, D::W), true);
                b.hopper(at(u, 3, 0, 0), k);
                b.dust(at(u, 2, 0, -1));
                b.dust(at(u, 3, 0, -1));
                b.stone(at(u, 1, 0, 0));
            }
        }
        if bridges {
            b.comp(at(u, 3, -1, 0), trd(u, mirror, D::N), true);
            b.stone(at(u, 3, -1, 1));
            b.stone(at(u, 3, -1, -1));
        } else if matches!(j, Job::Inner { .. } | Job::Bottom { .. }) {
            // the bottom ring: the W contact from the next slot's column cell (1,-3) through
            // (2,-3), (3,-3) and a ring-level comparator at (3,-2)
            b.dust(at(u, 2, 0, -3));
            b.dust(at(u, 3, 0, -3));
            b.comp(at(u, 3, 0, -2), trd(u, mirror, D::S), false);
        }
    }
}

/// Cells (ring-relative, ring level) of a ring that are dust, and that are sturdy.
fn ring_cells(mirror: bool, jobs: &[Job; 4]) -> (BTreeSet<(i32, i32)>, BTreeSet<(i32, i32)>, BTreeSet<(i32, i32)>) {
    let mut b = B::default();
    build_ring(&mut b, [0, 0, 0], mirror, jobs, true);
    let mut dust = BTreeSet::new();
    let mut sturdy = BTreeSet::new();
    let mut taken = BTreeSet::new();
    for (p, s) in &b.blocks {
        if p[1] != 0 {
            continue;
        }
        taken.insert((p[0], p[2]));
        if s.contains("redstone_wire") {
            dust.insert((p[0], p[2]));
        } else if s == "minecraft:stone" || s.contains("hopper") {
            sturdy.insert((p[0], p[2]));
        }
    }
    taken.insert((0, 0));
    (dust, sturdy, taken)
}

pub struct Top {
    pub b: B,
    pub f: [i32; 4],
    pub label: String,
}

/// Lay the chain (components at y = 2, supports at y = 1), the taps and the s network (y = 1)
/// above the top ring (at y = 0); None when it does not fit. Returns the column distances.
pub fn top_layout(code: &Code, mirror: bool, jobs: &[Job; 4], tf: &Tf) -> Option<Top> {
    let (rdust, rsturdy, rtaken) = ring_cells(mirror, jobs);
    let inside = |x: i32, z: i32| x.abs() <= 3 && z.abs() <= 3;
    let mut b = B::default();
    let (cb, inputs) = chain4(code);
    let lift = LIFT.with(|l| l.get());
    let tf = &Tf { mirror: tf.mirror, rot: tf.rot, t: [tf.t[0], lift - 1, tf.t[2]] };
    let clash = merge(&mut b, &cb, tf);
    if !clash.is_empty() {
        { FAIL.with(|f| f.borrow_mut()[0] += 1); return None; }
    }
    for p in b.blocks.keys() {
        if !inside(p[0], p[2]) || p[1] != lift {
            { FAIL.with(|f| f.borrow_mut()[1] += 1); return None; }
        }
    }
    for p in b.needs.clone() {
        b.stone([p[0], lift - 1, p[2]]);
    }
    b.needs.clear();
    // the a0 input: a repeater into gen 1's side instead of dust (dust would link to the network)
    let a0 = tf.p([0, 1, 1]);
    b.rep(a0, tf.d(D::E), 1);
    b.needs.clear();
    let can_support = |x: i32, z: i32| rsturdy.contains(&(x, z)) || !rtaken.contains(&(x, z));
    let dust2: Vec<(i32, i32)> = b.blocks.iter().filter(|(p, s)| p[1] == lift && s.contains("redstone_wire")).map(|(p, _)| (p[0], p[2])).collect();
    // the columns start at the top ring's column cells: network dust beside one (at y = 1, the
    // cell above it left air) steps down into it
    let mut ccs = Vec::new();
    for u in 0..4 {
        let (bx, bz) = tr(u, mirror, 3, 1);
        if b.blocks.contains_key(&[bx, 1, bz]) {
            { FAIL.with(|f| f.borrow_mut()[2] += 1); return None; }
        }
        ccs.push((axis_of(u, mirror), (bx, bz)));
    }
    let taps: Vec<((i32, i32), (i32, i32))> = Vec::new();
    let tap_blocks: Vec<(i32, i32)> = Vec::new();
    // the lamp's stones are powered by the windows: no dust on them
    let lamp_stones = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    let rears: Vec<(usize, (i32, i32))> = ccs.clone();
    let _ = &rears;
    // the s network: dust at y = 1 (on the ring's sturdy cells) and y = 2 (on stones), from the
    // output block (the dust under it, or beside it at y = 2) to cells beside the column cells
    let xo = tf.p([5, 1, 0]);
    let _ = (&taps, &tap_blocks);
    let Some((net, f)) = route(&b, xo, &ccs, &can_support, &lamp_stones, &dust2) else {
        if FAIL.with(|f| f.borrow()[3]) == 0 {
            for y in [1, 2] {
                eprintln!("level {y} (output {xo:?}, column cells {ccs:?}):");
                for z in -3..=3 {
                    let row: String = (-3..=3)
                        .map(|x| match b.blocks.get(&[x, y, z]).map(|s| s.as_str()) {
                            None => if y == 1 && !can_support(x, z) { 'x' } else { '.' },
                            Some(s) if s.contains("comparator") => 'C',
                            Some(s) if s.contains("repeater") => 'R',
                            Some(s) if s.contains("wire") => 'd',
                            Some(s) if s.contains("hopper") => 'H',
                            Some(s) if s.contains("composter") => 'K',
                            Some(_) => 's',
                        })
                        .collect();
                    eprintln!("  {row}");
                }
            }
        }
        { FAIL.with(|f| f.borrow_mut()[3] += 1); return None; }
    };
    for c in &net {
        if c.1 >= 2 && !b.blocks.contains_key(&[c.0, c.1 - 1, c.2]) {
            b.stone([c.0, c.1 - 1, c.2]);
        }
        b.dust([c.0, c.1, c.2]);
    }
    b.needs.clear();
    // caps over ring dust beside the network or the chain's dust (not over the column cells)
    let dusts: Vec<(i32, i32)> = b.blocks.iter().filter(|(p, s)| p[1] == 1 && s.contains("redstone_wire")).map(|(p, _)| (p[0], p[2])).collect();
    for (x, z) in dusts {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (x + dx, z + dz);
            if rdust.contains(&n) && !ccs.iter().any(|c| c.1 == n) && !b.blocks.contains_key(&[n.0, 1, n.1]) {
                b.stone([n.0, 1, n.1]);
            }
        }
    }
    // input extension lines out to the 7x7 edge (repeaters at y = 2 on stones at y = 1)
    for (name, p, d) in inputs {
        let mut q = tf.p(p);
        let dd = tf.d(d);
        let v = dd.v();
        let mut last = q;
        loop {
            let nq = add(q, v);
            if !inside(nq[0], nq[2]) {
                break;
            }
            let sup = [nq[0], lift - 1, nq[2]];
            if b.blocks.contains_key(&nq) || b.blocks.get(&sup).is_some_and(|s| s != "minecraft:stone") || ccs.iter().any(|c| c.1 == (nq[0], nq[2])) {
                { FAIL.with(|f| f.borrow_mut()[4] += 1); return None; }
            }
            q = nq;
            b.rep(q, dd.opp(), 1);
            b.stone(sup);
            last = q;
        }
        b.needs.clear();
        b.port(&name, last);
    }
    Some(Top { b, f, label: format!("{tf:?}") })
}

/// Search: top layouts (chain placement) x their column distances x a segment assignment.
pub fn search(out: &std::path::Path, code: &Code, y0: i32, lift: i32) -> Result<()> {
    LIFT.with(|l| l.set(lift));
    let levels: Vec<i32> = (0..7).map(|k| y0 + 2 * k).collect();
    let top = *levels.last().unwrap();
    // the top ring's mirror: rings alternate, the bottom one is type A
    let mirror_of = |t: usize| t % 2 == 1;
    let top_mirror = mirror_of(levels.len() - 1);
    let mut tried = 0;
    for mirror in [false, true] {
        for rot in 0..4 {
            for tx in -3..=3 {
                for tz in -3..=3 {
                    let tf = Tf { mirror, rot, t: [tx, 0, tz] };
                    // the segments' jobs need f, f needs the top ring's jobs: try the top segment's
                    // jobs as Unused-agnostic first (cells taken by any job), then refine
                    let full = [Job::Inner { k: 1, c: 1 }; 4];
                    let Some(t0) = top_layout2(code, top_mirror, &full, &tf) else { continue };
                    tried += 1;
                    let Some(jobs) = assign(code, t0.f) else { continue };
                    // jobs[t] is ring t's (segment, jobs), the bottom ring first
                    let mut b = B::default();
                    for (t, &y) in levels.iter().enumerate() {
                        let (seg, j) = jobs[t];
                        build_ring(&mut b, [0, y, 0], mirror_of(t), &j, t > 0);
                        b.port(&format!("output.{}", "abcdefg".as_bytes()[seg] as char), [0, y, 0]);
                    }
                    let tl = top_layout2(code, top_mirror, &jobs[levels.len() - 1].1, &tf).ok_or_else(|| anyhow!("top layout lost"))?;
                    if tl.f != t0.f {
                        continue;
                    }
                    let clash = merge(&mut b, &tl.b, &Tf { mirror: false, rot: 0, t: [0, top, 0] });
                    if !clash.is_empty() {
                        continue;
                    }
                    let d = b.design("seven-segment-columns", Some("seven-segment 4"));
                    std::fs::write(out, d.to_text())?;
                    println!("{}: {} blocks, f {:?}, chain {}, {tried} layouts tried", out.display(), d.blocks.len(), t0.f, tl.label);
                    for (t, (s, j)) in jobs.iter().enumerate() {
                        println!("  ring {t}: {} {j:?}", "abcdefg".as_bytes()[*s] as char);
                    }
                    return Ok(());
                }
            }
        }
    }
    bail!("no layout ({tried} top layouts fit; fail counts {:?}, unreachable axis counts {:?})", FAIL.with(|f| *f.borrow()), FAILAX.with(|f| *f.borrow()))
}

thread_local! {
    pub static FAIL: std::cell::RefCell<[usize; 7]> = const { std::cell::RefCell::new([0; 7]) };
}

type N3 = (i32, i32, i32);

/// Whether two network dust cells are linked (same level and adjacent, or one level apart,
/// horizontally adjacent, with air above the lower one).
fn linked(a: N3, b: N3, solid: &dyn Fn(N3) -> bool) -> bool {
    let h = (a.0 - b.0).abs() + (a.2 - b.2).abs();
    if h != 1 {
        return false;
    }
    match a.1 - b.1 {
        0 => true,
        1 => !solid((b.0, b.1 + 1, b.2)),
        -1 => !solid((a.0, a.1 + 1, a.2)),
        _ => false,
    }
}

/// Route the s network (see top_layout). Returns the dust cells and the column distances.
fn route(b: &B, xo: P, ccs: &[(usize, (i32, i32))], can_support: &dyn Fn(i32, i32) -> bool, lamp_stones: &[(i32, i32)], dust2: &[(i32, i32)]) -> Option<(BTreeSet<N3>, [i32; 4])> {
    let inside = |x: i32, z: i32| x.abs() <= 3 && z.abs() <= 3;
    let is_cc = |x: i32, z: i32| ccs.iter().any(|c| c.1 == (x, z));
    let at = |x: i32, y: i32, z: i32| b.blocks.get(&[x, y, z]).map(|s| s.as_str());
    let ok = |n: N3| -> bool {
        let (x, y, z) = n;
        if !inside(x, z) || at(x, y, z).is_some() || is_cc(x, z) {
            return false;
        }
        if dust2.iter().any(|d| (d.0 - x).abs() + (d.1 - z).abs() == 1) && y == LIFT.with(|l| l.get()) - 1 {
            return false;
        }
        if y == 1 {
            if !can_support(x, z) || lamp_stones.contains(&(x, z)) {
                return false;
            }
        } else if at(x, y - 1, z).is_some_and(|s| s != "minecraft:stone") || (y >= 3 && at(x, y - 2, z).is_some_and(|s| s.contains("wire"))) {
            return false;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if let Some(s) = at(x + dx, y, z + dz) {
                // a repeater only cares about its back and front; a comparator about every side
                let repeater_side = s.contains("repeater") && {
                    let f = s.split("facing=").nth(1).unwrap_or("");
                    let ns = f.starts_with("north") || f.starts_with("south");
                    (dx != 0 && ns) || (dz != 0 && !ns)
                };
                if (s.contains("comparator") || s.contains("repeater") || s.contains("redstone_wire")) && !repeater_side {
                    return false;
                }
            }
        }
        true
    };
    let solid = |n: N3| at(n.0, n.1, n.2).is_some();
    let mut starts = Vec::new();
    let lift = LIFT.with(|l| l.get());
    if ok((xo[0], lift - 1, xo[2])) {
        starts.push((xo[0], lift - 1, xo[2]));
    }
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let n = (xo[0] + dx, lift, xo[2] + dz);
        if ok(n) {
            starts.push(n);
        }
    }
    let nbrs = |c: N3| -> Vec<N3> {
        let mut v = Vec::new();
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for y in 1..=LIFT.with(|l| l.get()) {
                v.push((c.0 + dx, y, c.2 + dz));
            }
        }
        v
    };
    let mut dist: BTreeMap<N3, i32> = BTreeMap::new();
    let mut prev: BTreeMap<N3, N3> = BTreeMap::new();
    let mut q = VecDeque::new();
    for s in &starts {
        dist.insert(*s, 0);
        q.push_back(*s);
    }
    while let Some(c) = q.pop_front() {
        for n in nbrs(c) {
            if !dist.contains_key(&n) && ok(n) && linked(c, n, &solid) {
                dist.insert(n, dist[&c] + 1);
                prev.insert(n, c);
                q.push_back(n);
            }
        }
    }
    let mut net = BTreeSet::new();
    for (_, cc) in ccs {
        let best = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().map(|(dx, dz)| (cc.0 + dx, 1, cc.1 + dz)).filter(|c| dist.contains_key(c)).min_by_key(|c| dist[c]);
        let Some(mut c) = best else {
            FAILAX.with(|f| f.borrow_mut()[ccs.iter().position(|x| x.1 == *cc).unwrap()] += 1);
            if FAIL.with(|f| f.borrow()[3]) == 0 {
                eprintln!("axis cell {cc:?} unreachable; reached {:?}", dist.keys().collect::<Vec<_>>());
            }
            return None;
        };
        loop {
            net.insert(c);
            match prev.get(&c) {
                Some(p) => c = *p,
                None => break,
            }
        }
    }
    // an upper dust's support must not be network dust
    if net.iter().any(|c| c.1 >= 2 && net.contains(&(c.0, c.1 - 1, c.2))) {
        return None;
    }
    // true distances over the built network (a level-2 dust's stone counts as solid)
    let solid2 = |n: N3| solid(n) || net.contains(&(n.0, n.1 + 1, n.2)) || net.contains(&n);
    let mut d2: BTreeMap<N3, i32> = BTreeMap::new();
    let mut q = VecDeque::new();
    for s in &starts {
        if net.contains(s) {
            d2.insert(*s, 0);
            q.push_back(*s);
        }
    }
    while let Some(c) = q.pop_front() {
        for n in nbrs(c) {
            if net.contains(&n) && !d2.contains_key(&n) && linked(c, n, &solid2) {
                d2.insert(n, d2[&c] + 1);
                q.push_back(n);
            }
        }
    }
    let mut f = [0i32; 4];
    for (axis, cc) in ccs {
        f[*axis] = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter_map(|(dx, dz)| d2.get(&(cc.0 + dx, 1, cc.1 + dz))).min()? + 1;
    }
    Some((net, f))
}
thread_local! {
    /// The chain's level above the top ring (2: its supports share the network level; 3: its own).
    pub static LIFT: std::cell::Cell<i32> = const { std::cell::Cell::new(2) };
}
thread_local! {
    pub static FAILAX: std::cell::RefCell<[usize; 4]> = const { std::cell::RefCell::new([0; 4]) };
}

/// The top, second form: the chain at y = 2 (supports at y = 1); the s network as dust at y = 2
/// on stones; one tap per column: a comparator at y = 1 (on the top ring) facing the block above
/// the column cell, its rear a stone under network dust. Every column starts 2 gt after the
/// network, at distance f = the network distance of its tap's dust.
pub fn top_layout2(code: &Code, mirror: bool, jobs: &[Job; 4], tf: &Tf) -> Option<Top> {
    let (_rdust, rsturdy, rtaken) = ring_cells(mirror, jobs);
    let inside = |x: i32, z: i32| x.abs() <= 3 && z.abs() <= 3;
    let can_support = |x: i32, z: i32| rsturdy.contains(&(x, z)) || !rtaken.contains(&(x, z));
    let mut b = B::default();
    let (cb, inputs) = chain4(code);
    let tf = &Tf { mirror: tf.mirror, rot: tf.rot, t: [tf.t[0], 1, tf.t[2]] };
    if !merge(&mut b, &cb, tf).is_empty() {
        return fail(0);
    }
    if b.blocks.keys().any(|p| !inside(p[0], p[2])) {
        return fail(1);
    }
    for p in b.needs.clone() {
        b.stone([p[0], 1, p[2]]);
    }
    b.needs.clear();
    let a0 = tf.p([0, 1, 1]);
    b.rep(a0, tf.d(D::E), 1);
    b.needs.clear();
    // input extension lines (repeaters at y = 2 on stones)
    for (name, p, d) in inputs {
        let mut q = tf.p(p);
        let dd = tf.d(d);
        let v = dd.v();
        let mut last = q;
        loop {
            let nq = add(q, v);
            if !inside(nq[0], nq[2]) {
                break;
            }
            let sup = [nq[0], 1, nq[2]];
            if b.blocks.contains_key(&nq) || b.blocks.get(&sup).is_some_and(|s| s != "minecraft:stone") {
                return fail(2);
            }
            q = nq;
            b.rep(q, dd.opp(), 1);
            b.stone(sup);
            last = q;
        }
        b.needs.clear();
        b.port(&name, last);
    }
    // tap options per column: (tap cell, its output direction in base coords, rear cell)
    let mut opts: Vec<(usize, (i32, i32), Vec<((i32, i32), D, (i32, i32))>)> = Vec::new();
    for u in 0..4 {
        let cc = tr(u, mirror, 3, 1);
        if b.blocks.get(&[cc.0, 1, cc.1]).is_some_and(|s| s != "minecraft:stone") || b.blocks.get(&[cc.0, 2, cc.1]).is_some_and(|s| s.contains("wire")) {
            return fail(3);
        }
        let mut o = Vec::new();
        for (t, d, r) in [((3, 2), D::N, (3, 3)), ((3, 0), D::S, (3, -1))] {
            let t2 = tr(u, mirror, t.0, t.1);
            let r2 = tr(u, mirror, r.0, r.1);
            let tap_ok = can_support(t2.0, t2.1) && !b.blocks.contains_key(&[t2.0, 1, t2.1]);
            let rear_ok = b.blocks.get(&[r2.0, 1, r2.1]).is_none_or(|s| s == "minecraft:stone");
            if tap_ok && rear_ok {
                o.push((t2, trd(u, mirror, d), r2));
            }
        }
        opts.push((axis_of(u, mirror), cc, o));
    }
    // network cells at y = 2
    let xo = tf.p([5, 1, 0]);
    let ccs: Vec<(i32, i32)> = opts.iter().map(|o| o.1).collect();
    let taps_all: Vec<(i32, i32)> = opts.iter().flat_map(|o| o.2.iter().map(|t| t.0)).collect();
    let ok = |x: i32, z: i32| -> bool {
        if !inside(x, z) || b.blocks.contains_key(&[x, 2, z]) || ccs.contains(&(x, z)) || taps_all.contains(&(x, z)) {
            return false;
        }
        if b.blocks.get(&[x, 1, z]).is_some_and(|s| s != "minecraft:stone") {
            return false;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if let Some(s) = b.blocks.get(&[x + dx, 2, z + dz]) {
                let repeater_side = s.contains("repeater") && {
                    let f = s.split("facing=").nth(1).unwrap_or("");
                    let ns = f.starts_with("north") || f.starts_with("south");
                    (dx != 0 && ns) || (dz != 0 && !ns)
                };
                if (s.contains("comparator") || s.contains("repeater") || s.contains("redstone_wire")) && !repeater_side {
                    return false;
                }
            }
        }
        true
    };
    let mut dist: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    let mut prev: BTreeMap<(i32, i32), (i32, i32)> = BTreeMap::new();
    let mut q = VecDeque::new();
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let c = (xo[0] + dx, xo[2] + dz);
        if ok(c.0, c.1) {
            dist.insert(c, 0);
            q.push_back(c);
        }
    }
    while let Some(c) = q.pop_front() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dz);
            if !dist.contains_key(&n) && ok(n.0, n.1) {
                dist.insert(n, dist[&c] + 1);
                prev.insert(n, c);
                q.push_back(n);
            }
        }
    }
    let mut net = BTreeSet::new();
    let mut chosen = Vec::new();
    for (axis, cc, o) in &opts {
        let Some(best) = o.iter().filter(|t| dist.contains_key(&t.2)).min_by_key(|t| dist[&t.2]) else {
            if FAIL.with(|f| f.borrow()[4]) < 2 {
                eprintln!("axis {axis} cc {cc:?} options {o:?} out {xo:?} reached {:?}", dist.keys().collect::<Vec<_>>());
                for y in [1, 2] {
                    for z in -3..=3 {
                        let row: String = (-3..=3)
                            .map(|x| match b.blocks.get(&[x, y, z]).map(|s| s.as_str()) {
                                None => '.',
                                Some(s) if s.contains("comparator") => 'C',
                                Some(s) if s.contains("repeater") => 'R',
                                Some(s) if s.contains("wire") => 'd',
                                Some(s) if s.contains("hopper") => 'H',
                                Some(s) if s.contains("composter") => 'K',
                                Some(_) => 's',
                            })
                            .collect();
                        eprintln!("  y{y} {row}");
                    }
                }
            }
            return fail(4);
        };
        let mut c = best.2;
        loop {
            net.insert(c);
            match prev.get(&c) {
                Some(p) => c = *p,
                None => break,
            }
        }
        chosen.push((*axis, *cc, *best));
    }
    // taps must not stand where network dust is planned (their cells are excluded) and rears
    // must be distinct from other taps
    for (_, cc, (t, d, r)) in &chosen {
        b.stone([cc.0, 1, cc.1]);
        b.comp([t.0, 1, t.1], *d, false);
        if !b.blocks.contains_key(&[r.0, 1, r.1]) {
            b.stone([r.0, 1, r.1]);
        }
    }
    b.needs.clear();
    for c in &net {
        if !b.blocks.contains_key(&[c.0, 1, c.1]) {
            b.stone([c.0, 1, c.1]);
        }
        if b.blocks.get(&[c.0, 1, c.1]).is_some_and(|s| s != "minecraft:stone") {
            return None;
        }
        b.dust([c.0, 2, c.1]);
    }
    b.needs.clear();
    // true distances
    let mut d2: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    let mut q = VecDeque::new();
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
        let c = (xo[0] + dx, xo[2] + dz);
        if net.contains(&c) {
            d2.insert(c, 0);
            q.push_back(c);
        }
    }
    while let Some(c) = q.pop_front() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = (c.0 + dx, c.1 + dz);
            if net.contains(&n) && !d2.contains_key(&n) {
                d2.insert(n, d2[&c] + 1);
                q.push_back(n);
            }
        }
    }
    let mut f = [0i32; 4];
    for (axis, _, (_, _, r)) in &chosen {
        f[*axis] = *d2.get(r)?;
    }
    Some(Top { b, f, label: format!("{tf:?} taps {:?}", chosen.iter().map(|c| c.2.0).collect::<Vec<_>>()) })
}

fn fail<T>(i: usize) -> Option<T> {
    FAIL.with(|f| f.borrow_mut()[i] += 1);
    None
}
