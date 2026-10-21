//! Compact ring stacks for the display face (clk-item, `mctas circuit display rings2`): rings.rs's
//! three stacks (each segment one analog cross ring one level above its lamp, its centre dust read
//! under the front axis, a repeater lane to the lamp), solved FIRST and wired after: a lower ring
//! takes what the ring above couples into it (the ring above's bridge and copy targets sit right
//! over its arc cells and W contacts) and gets its own feed line only when that is not enough.
//! Lines: no line block under a line dust (it would read the block and close a lossless loop).

use crate::abuild::{B, D, P, add};
use crate::analog::{Code, masks, runs};
use crate::astack::{Slot, build_ring, coupling, solve_ring_f};
use crate::therm::{chain_back, walk};
use anyhow::{Result, anyhow, ensure};

type Feed = Vec<((i32, i32, i32), u8)>;

/// One stack: centre x, rings top down (segment, lamp row, ring level), how its lanes reach the
/// lamps (0: straight, -1/+1: along the face plane toward -x / +x).
struct Stack {
    cx: i32,
    rings: Vec<(char, i32, i32)>,
    turn: i32,
}

/// A solved ring: its slots, copy/bridge mask, feed, and which line feeds it needs.
#[derive(Clone)]
struct Solved {
    slots: [Slot; 4],
    mask: u8,
    feed: Feed,
    direct: bool,
    second: bool,
    /// arc cells left out (astack::REMOVED while this ring is solved, coupled and built)
    removed: Vec<(i32, i32, i32)>,
}

const DIRECT: (i32, i32, i32) = (-1, 0, 3);
const SECOND: (i32, i32, i32) = (-2, 0, 3);

fn solve(rs: &[crate::analog::Run], above: &Feed, top: bool, lean: bool, allowed: u8, bridges_off: u8) -> Option<Solved> {
    let mut tries: Vec<(bool, bool)> = Vec::new();
    if !top && lean {
        tries.push((false, false));
    }
    tries.extend([(true, false), (true, true)]);
    for (direct, second) in tries {
        let mut feed: Feed = Vec::new();
        if direct {
            feed.push((DIRECT, 0));
        }
        if second {
            feed.push((SECOND, 0));
        }
        feed.extend(above.iter().copied());
        if feed.is_empty() {
            continue;
        }
        if let Some((slots, mask)) = solve_ring_f(rs, &feed, allowed, bridges_off) {
            return Some(Solved { slots, mask, feed, direct, second, removed: Vec::new() });
        }
    }
    None
}

/// The radius-3 arc's corner groups (3 cells each): a ring whose sides are still joined through
/// the other corners and the bridges does not need them all.
const CORNERS: [[(i32, i32, i32); 3]; 4] = [
    [(3, 0, -2), (3, 0, -3), (2, 0, -3)],
    [(3, 0, 2), (3, 0, 3), (2, 0, 3)],
    [(-3, 0, 2), (-3, 0, 3), (-2, 0, 3)],
    [(-3, 0, -2), (-3, 0, -3), (-2, 0, -3)],
];

/// solve_ring_f with these arc cells left out and these bridges off, accepted only when every
/// contact a slot uses is still reached by the dust (the solver takes a missing contact as no
/// constraint, which the game does not).
fn try_solve(rs: &[crate::analog::Run], feed: &Feed, removed: &[(i32, i32, i32)], allowed: u8, off: u8) -> Option<([Slot; 4], u8)> {
    crate::astack::REMOVED.with(|r| *r.borrow_mut() = removed.to_vec());
    let r = solve_ring_f(rs, feed, allowed, off);
    let ok = r.is_some_and(|(slots, mask)| {
        let dist = crate::astack::ring_distances(&slots, feed, mask);
        slots.iter().enumerate().all(|(u, s)| {
            let at = |x: i32, z: i32| {
                let (rx, rz) = crate::astack::rot_pub(u, x, z);
                (rx, 0, rz)
            };
            let w_ok = || dist.contains_key(&at(2, -1));
            let o_ok = || dist.contains_key(&at(3, 1));
            match s {
                Slot::Empty => true,
                Slot::Inner { .. } => w_ok() && o_ok(),
                Slot::Top { .. } => o_ok(),
                Slot::Bottom { .. } => w_ok(),
            }
        })
    });
    crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
    if ok { r } else { None }
}

/// Leave out bridges, then arc corner groups and single arc cells, while the ring still solves
/// with the same feed: fewer dust, supports and comparators. Kept: the feed cells, copy endpoints
/// and the endpoints of the bridges still on (astack's distances carry a bridge into its other end
/// even when that cell is gone, and the game then has a dead half-arc). `seed` > 0 shuffles the
/// order the bridges and cells are tried in (the greedy result depends on it).
fn prune(rs: &[crate::analog::Run], s: Solved, allowed: u8, bridges_off: u8, seed: u64) -> Solved {
    let mut rng = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    let mut next = move |n: usize| -> usize {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((rng >> 33) as usize) % n.max(1)
    };
    let mut best = s;
    let mut off = bridges_off | (best.mask >> 4);
    let mut order: Vec<usize> = (0..4).collect();
    if seed > 0 {
        for i in (1..order.len()).rev() {
            order.swap(i, next(i + 1));
        }
    }
    for i in order {
        if off >> i & 1 == 1 {
            continue;
        }
        if let Some((slots, mask)) = try_solve(rs, &best.feed, &best.removed, allowed, off | 1 << i) {
            off |= 1 << i;
            best = Solved { slots, mask, ..best };
        }
    }
    // kept: the line feeds' cells (a removed coupling cell just stops feeding; copies check their
    // ends in astack's distances) and both ends of every bridge still on
    let mut keep: Vec<(i32, i32, i32)> = best.feed.iter().map(|(c, _)| *c).filter(|c| *c == DIRECT || *c == SECOND).collect();
    for (i, (a, c)) in crate::astack::BRIDGES.iter().enumerate() {
        if off >> i & 1 == 0 {
            keep.extend([*a, *c]);
        }
    }
    let mut groups: Vec<Vec<(i32, i32, i32)>> = CORNERS.iter().map(|g| g.to_vec()).collect();
    for i in -3..=3 {
        for (x, z) in [(3, i), (-3, i), (i, 3), (i, -3)] {
            if x != 0 && z != 0 {
                groups.push(vec![(x, 0, z)]);
            }
        }
    }
    if seed > 0 {
        for i in (1..groups.len()).rev() {
            groups.swap(i, next(i + 1));
        }
    }
    for g in groups {
        if g.iter().any(|c| keep.contains(c) || best.removed.contains(c)) {
            continue;
        }
        let mut removed = best.removed.clone();
        removed.extend(g.iter().copied());
        if let Some((slots, mask)) = try_solve(rs, &best.feed, &removed, allowed, off) {
            best = Solved { slots, mask, removed, ..best };
        }
    }
    best
}

/// A ring's own block count (with its supports) as build_ring places it.
fn ring_cost(s: &Solved) -> usize {
    let mut b = B::default();
    crate::astack::REMOVED.with(|r| *r.borrow_mut() = s.removed.clone());
    build_ring(&mut b, [0, 0, 0], &s.slots, &s.feed, s.mask);
    crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
    b.dust([0, 0, 0]);
    b.supports();
    b.blocks.len()
}

pub fn build(code: &Code, allowed: u8, bridges_off: u8, lean: bool, prune_arcs: bool, tries: u64, out: &std::path::Path) -> Result<()> {
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    let cz = 5;
    let stacks = [
        Stack { cx: 1, rings: vec![('a', 4, 5), ('g', 2, 3), ('d', 0, 1)], turn: 0 },
        Stack { cx: 9, rings: vec![('f', 3, 4), ('e', 1, 2)], turn: -1 },
        Stack { cx: -7, rings: vec![('b', 3, 4), ('c', 1, 2)], turn: 1 },
    ];
    // solve every ring first, top down per stack
    let mut solved: Vec<Vec<Solved>> = Vec::new();
    for st in &stacks {
        let mut above: Feed = Vec::new();
        let mut v = Vec::new();
        for (i, &(seg, _row, level)) in st.rings.iter().enumerate() {
            let si = "abcdefg".find(seg).unwrap();
            let rs = runs(m[si]);
            // every feed option (coupling only for a lower ring with --lean; a line of its own;
            // a line and a second feed), each pruned with `tries` orders: the cheapest ring plus
            // feed cost whose coupling still lets the ring below solve
            let only = std::env::var("RINGS2_PRUNE_ONLY").unwrap_or_default();
            let do_prune = prune_arcs && (only.is_empty() || only.contains(seg));
            let mut opts: Vec<(bool, bool)> = Vec::new();
            if i > 0 && lean {
                opts.push((false, false));
            }
            opts.extend([(true, false), (true, true)]);
            let mut vs: Vec<(usize, Solved)> = Vec::new();
            for (direct, second) in opts {
                let mut feed: Feed = Vec::new();
                if direct {
                    feed.push((DIRECT, 0));
                }
                if second {
                    feed.push((SECOND, 0));
                }
                feed.extend(above.iter().copied());
                let Some((slots, mask)) = (if feed.is_empty() { None } else { solve_ring_f(&rs, &feed, allowed, bridges_off) }) else { continue };
                let base = Solved { slots, mask, feed, direct, second, removed: Vec::new() };
                // a lower ring's own line: a descent (about 14 blocks per level step); a second
                // feed: a dust and a comparator with their supports
                let extra = if direct && i > 0 { 14 * i } else { 0 } + if second { 4 } else { 0 };
                if do_prune {
                    for seed in 0..tries.max(1) {
                        let v = prune(&rs, base.clone(), allowed, bridges_off, seed);
                        vs.push((ring_cost(&v) + extra, v));
                    }
                } else {
                    vs.push((ring_cost(&base) + extra, base));
                }
            }
            ensure!(!vs.is_empty(), "no ring for {seg} {rs:?} (above {above:?})");
            vs.sort_by_key(|x| x.0);
            let below = st.rings.get(i + 1).map(|&(bs, _, _)| runs(m["abcdefg".find(bs).unwrap()]));
            let pick = vs
                .iter()
                .position(|(_, v)| {
                    below.as_ref().is_none_or(|brs| {
                        crate::astack::REMOVED.with(|r| *r.borrow_mut() = v.removed.clone());
                        let cpl = coupling(&v.slots, &v.feed, v.mask);
                        crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
                        solve(brs, &cpl, false, lean, allowed, bridges_off).is_some()
                    })
                })
                .ok_or_else(|| anyhow!("no {seg} ring lets the ring below solve"))?;
            let s = vs.swap_remove(pick).1;
            println!("  ring {seg} at level {level} ({} blocks): slots {:?} mask {:08b} direct {} second {} removed {:?} feed {:?}", ring_cost(&s), s.slots, s.mask, s.direct, s.second, s.removed, s.feed);
            crate::astack::REMOVED.with(|r| *r.borrow_mut() = s.removed.clone());
            above = coupling(&s.slots, &s.feed, s.mask);
            crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
            v.push(s);
        }
        solved.push(v);
    }
    let need = |si: usize, ri: usize| solved[si][ri].direct;
    let mut b = B::default();
    // the chain right behind the lines: its split two moves from the hub H = (2, 4, 12), from
    // which a, f and b's lines start (the chain's composters sit at z = 14 and 18, clear of them)
    let split = chain_back(&mut b, [-7, 2, 16], code);
    let hub = walk(&mut b, split, "N N");
    ensure!(hub == [2, 4, 12], "hub at {hub:?}");
    let f_rear = walk(&mut b, hub, "E E E N");
    let b_rear = walk(&mut b, hub, "W W W W W N");
    let a_rear = walk(&mut b, hub, "N uW");
    let mut rears: Vec<(P, P)> = vec![(a_rear, [0, 5, 10]), (f_rear, [8, 4, 10]), (b_rear, [-8, 4, 10])];
    // descents (two levels down each): only for the lower rings that need their own feed; d's
    // starts from g's rear, so g's descent is built when either needs it
    if need(0, 1) || need(0, 2) {
        let g_rear = walk(&mut b, a_rear, "W dS dE N");
        if need(0, 1) {
            rears.push((g_rear, [0, 3, 10]));
        }
        if need(0, 2) {
            let d_rear = walk(&mut b, g_rear, "E dS dW N");
            rears.push((d_rear, [0, 1, 10]));
        }
    }
    if need(1, 1) {
        let e_rear = walk(&mut b, f_rear, "S dS dE N N W");
        rears.push((e_rear, [8, 2, 10]));
    }
    if need(2, 1) {
        let c_rear = walk(&mut b, b_rear, "E dS dW N");
        rears.push((c_rear, [-8, 2, 10]));
    }
    for (rear, want) in rears {
        ensure!(rear == want, "feed line ends at {rear:?}, want {want:?}");
        b.comp(add(rear, [0, 0, -1]), D::N, false);
    }
    let lamp_x = |seg: char| match seg {
        'f' | 'e' => 2,
        'b' | 'c' => 0,
        _ => 1,
    };
    for (si, st) in stacks.iter().enumerate() {
        for (ri, &(seg, row, level)) in st.rings.iter().enumerate() {
            let s = &solved[si][ri];
            if s.second {
                b.dust([st.cx - 2, level, cz + 5]);
                b.comp([st.cx - 2, level, cz + 4], D::N, false);
            }
            let c = [st.cx, level, cz];
            crate::astack::REMOVED.with(|r| *r.borrow_mut() = s.removed.clone());
            if std::env::var("RINGS2_DEBUG").is_ok_and(|v| v.contains(seg)) {
                let dist = crate::astack::ring_distances(&s.slots, &s.feed, s.mask);
                for (p, d) in &dist {
                    println!("    {seg} dist {:?} = {d}", [c[0] + p.0, c[1] + p.1, c[2] + p.2]);
                }
            }
            build_ring(&mut b, c, &s.slots, &s.feed, s.mask);
            crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
            // an empty slot's stone (the block its window would light) is not needed
            for (u, sl) in s.slots.iter().enumerate() {
                if matches!(sl, Slot::Empty) {
                    let (x, z) = crate::astack::rot_pub(u, 1, 0);
                    b.blocks.remove(&add(c, [x, 0, z]));
                }
            }
            b.dust(c);
            let y = level - 1;
            ensure!(y == row, "ring {seg} at {level}, lamp row {row}");
            let lane_x = st.cx;
            b.rep([lane_x, y, cz - 1], D::N, 1);
            b.stone([lane_x, y, cz - 2]);
            b.rep([lane_x, y, cz - 3], D::N, 1);
            for z in 1..cz - 3 {
                b.rep([lane_x, y, z], D::N, 1);
            }
            let lx = lamp_x(seg);
            if st.turn != 0 {
                b.rep([lane_x, y, 1], D::N, 1);
                b.stone([lane_x, y, 0]);
                let dir = if st.turn < 0 { D::W } else { D::E };
                let mut x = lane_x + st.turn;
                while x != lx {
                    b.rep([x, y, 0], dir, 1);
                    x += st.turn;
                }
            } else {
                ensure!(lane_x == lx, "straight lane off its lamp");
            }
            b.port(&format!("output.{seg}"), [lx, row, 0]);
        }
    }
    let bad = crate::agen::bad_supports(&b);
    let d = b.design("seven-segment-rings2", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: {} blocks; bad supports {:?}", out.display(), d.blocks.len(), bad);
    Ok(())
}
