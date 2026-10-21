//! Compact ring stacks for the display face (clk-item, `mctas circuit display rings2`): rings.rs's
//! three stacks (each segment one analog cross ring one level above its lamp, its centre dust read
//! under the front axis, a repeater lane to the lamp), solved FIRST and wired after: a lower ring
//! takes what the ring above couples into it (the ring above's bridge and copy targets sit right
//! over its arc cells and W contacts) and gets its own feed line only when that is not enough.
//! Lines: no line block under a line dust (it would read the block and close a lossless loop).

use crate::abuild::{B, D, P, add};
use crate::analog::{Code, masks, runs};
use crate::astack::{Slot, build_ring, coupling, solve_ring_f};
use crate::therm::walk;
use anyhow::{Result, anyhow, ensure};

type Feed = Vec<((i32, i32, i32), u8)>;

/// One stack: centre x, rings top down (segment, lamp row, ring level), how its lanes reach the
/// lamps (0: straight, -1/+1: along the face plane toward -x / +x).
struct Stack {
    cx: i32,
    rings: Vec<(char, i32, i32)>,
    turn: i32,
    /// ring-relative cells the stack's line feed and second feed comparators feed
    direct: (i32, i32, i32),
    second: (i32, i32, i32),
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

thread_local! {
    /// The current stack's line feed cells (ring-relative): its direct feed and its second feed.
    static CELLS: std::cell::Cell<((i32, i32, i32), (i32, i32, i32))> = const { std::cell::Cell::new(((-1, 0, 3), (-2, 0, 3))) };
}
#[allow(non_snake_case)]
fn DIRECT() -> (i32, i32, i32) {
    CELLS.with(|c| c.get().0)
}
#[allow(non_snake_case)]
fn SECOND() -> (i32, i32, i32) {
    CELLS.with(|c| c.get().1)
}

fn solve(rs: &[crate::analog::Run], above: &Feed, top: bool, lean: bool, allowed: u8, bridges_off: u8) -> Option<Solved> {
    let mut tries: Vec<(bool, bool)> = Vec::new();
    if !top && lean {
        tries.push((false, false));
    }
    tries.extend([(true, false), (true, true)]);
    for (direct, second) in tries {
        let mut feed: Feed = Vec::new();
        if direct {
            feed.push((DIRECT(), 0));
        }
        if second {
            feed.push((SECOND(), 0));
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
    let mut keep: Vec<(i32, i32, i32)> = best.feed.iter().map(|(c, _)| *c).filter(|c| *c == DIRECT() || *c == SECOND()).collect();
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
    if std::env::var("RINGS2_DUMP").is_ok() {
        let mut kinds: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for s in b.blocks.values() {
            *kinds.entry(s.split('[').next().unwrap_or("").trim_start_matches("minecraft:").to_string()).or_default() += 1;
        }
        println!("    ring blocks by kind: {kinds:?}");
    }
    b.blocks.len()
}

/// clk-obs's options (`display rings2 --timing --direct --ring-bridges-off`).
#[derive(Default)]
pub struct Extra {
    /// Segments whose rings must be race-free (astack::PULSE_RUNS).
    pub timing: String,
    /// Rings that must have their own line feed.
    pub direct: String,
    /// Extra bridges left out per ring: `a=4,g=1`.
    pub ring_bridges_off: String,
    /// Ring a's line onto the arc cell east of its rear hopper (`--middle-east`).
    pub middle_east: bool,
    /// Print every ring's race report (`--report`).
    pub report: bool,
    /// The b/c stack's line onto the arc cell east of b's rear hopper (`--side-east`).
    pub side_east: bool,
    /// No per-ring lines (the sweep).
    pub quiet: bool,
    /// The chain under the lines, four cells forward (`--chain-front`).
    pub chain_front: bool,
    /// b's line from the chain's first s block, the hub climbed from the west (`--hub-west`).
    pub hub_west: bool,
}

impl Extra {
    fn bridges_off(&self, seg: char) -> u8 {
        for part in self.ring_bridges_off.split(',').filter(|s| !s.trim().is_empty()) {
            if let Some((s, m)) = part.split_once('=')
                && s.trim().starts_with(seg)
            {
                return m.trim().parse().unwrap_or(0);
            }
        }
        0
    }
}

type Times = Vec<((i32, i32, i32), u32)>;

#[allow(clippy::too_many_arguments)]
pub fn build(code: &Code, allowed: u8, bridges_off: u8, lean: bool, prune_arcs: bool, tries: u64, tight: bool, deglitch: u8, off: &str, x: &Extra, out: &std::path::Path) -> Result<usize> {
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    // --off SEGS: those rings compute the segment's complement (its OFF runs); their lanes invert
    // it back through a torch between two filtering repeaters (middle stack only)
    let seg_mask = |seg: char| {
        let mk = m["abcdefg".find(seg).unwrap()];
        if off.contains(seg) { !mk } else { mk }
    };
    let cz = 5;
    // --tight: the side stacks one cell closer (no gap column between the stacks' arcs: they sit
    // at odd and even levels, and the diagonal dust steps between them are cut by the arc
    // supports, or by a block where none is); their line feeds move to the arc cells nearest
    // the middle (even x, as the lines' blocks need), so the lines and lanes are shorter
    let mut stacks = if tight {
        [
            Stack { cx: 1, rings: vec![('a', 4, 5), ('g', 2, 3), ('d', 0, 1)], turn: 0, direct: (-1, 0, 3), second: (-2, 0, 3) },
            Stack { cx: 8, rings: vec![('f', 3, 4), ('e', 1, 2)], turn: -1, direct: (-2, 0, 3), second: (-1, 0, 3) },
            Stack { cx: -7, rings: vec![('b', 3, 4), ('c', 1, 2)], turn: 1, direct: (-1, 0, 3), second: (-2, 0, 3) },
        ]
    } else {
        [
            Stack { cx: 1, rings: vec![('a', 4, 5), ('g', 2, 3), ('d', 0, 1)], turn: 0, direct: (-1, 0, 3), second: (-2, 0, 3) },
            Stack { cx: 9, rings: vec![('f', 3, 4), ('e', 1, 2)], turn: -1, direct: (-1, 0, 3), second: (-2, 0, 3) },
            Stack { cx: -7, rings: vec![('b', 3, 4), ('c', 1, 2)], turn: 1, direct: (-1, 0, 3), second: (-2, 0, 3) },
        ]
    };
    // --middle-east: ring a's line comes straight up from the hub onto the arc cell east of the rear
    // hopper (the hub's column), so a's back bridge points west and couples into the ring below
    // west of its hopper first (a race-free timing for g's [1, 3] window in slot 3)
    if x.middle_east {
        stacks[0].direct = (1, 0, 3);
        stacks[0].second = (2, 0, 3);
    }
    // --side-east: the same for the b/c stack (its line one move shorter; c's descent mirrored)
    if x.side_east {
        stacks[2].direct = (1, 0, 3);
        stacks[2].second = (2, 0, 3);
    }
    // the line moves (from the hub; g and d descend from a's rear, e from f's, c from b's): the
    // feed times the race check models (2 gt per comparator move, plus the feed's own comparator)
    let (f_moves, b_moves, e_moves, c_moves) = match (tight, x.side_east) {
        (true, false) => ("E E N", "W W W W W N", "W dS dE N", "E dS dW N"),
        (true, true) => ("E E N", "W W W W N", "W dS dE N", "W dS dE N"),
        (false, false) => ("E E E N", "W W W W W N", "S dS dE N N W", "E dS dW N"),
        (false, true) => ("E E E N", "W W W W N", "S dS dE N N W", "W dS dE N"),
    };
    let (a_moves, g_moves, d_moves) = (if x.middle_east { "uN" } else { "N uW" }, "W dS dE N", "E dS dW N");
    let nm = |m: &str| m.split_whitespace().count() as u32;
    let line_t = |seg: char| -> u32 {
        let moves = match seg {
            'a' => nm(a_moves),
            'g' => nm(a_moves) + nm(g_moves),
            'd' => nm(a_moves) + nm(g_moves) + nm(d_moves),
            'f' => nm(f_moves),
            'e' => nm(f_moves) + nm(e_moves),
            'b' => nm(b_moves),
            _ => nm(b_moves) + nm(c_moves),
        };
        2 * moves + 2
    };
    // the inner runs a transition of the judge's stimulus crosses with the segment off at both ends
    // (compute::sequence: every single-bit change, every complement, and from each Gray code value
    // to the complement of the next; an ON ring's lane does not filter pulses, an OFF ring's lane
    // filters both ways)
    let gray: Vec<u32> = (0..16u32).map(|i| i ^ (i >> 1)).collect();
    let mut judged: Vec<(u32, u32)> = Vec::new();
    for v in 0..16u32 {
        for w in [v ^ 1, v ^ 2, v ^ 4, v ^ 8, v ^ 15] {
            judged.push((v, w));
        }
    }
    for k in 0..15 {
        judged.push((gray[k], !gray[k + 1] & 15));
    }
    let pulse_runs = |seg: char| -> Vec<(u8, u8, bool, bool)> {
        if off.contains(seg) {
            return Vec::new();
        }
        let rs = runs(seg_mask(seg));
        let mut v_out: Vec<(u8, u8, bool, bool)> = Vec::new();
        for &(v, w) in &judged {
            let lit = |u: u32| circuit::compute::SEGMENTS[u as usize].contains(seg);
            if lit(v) || lit(w) {
                continue;
            }
            let (sa, sb) = (code.s(v), code.s(w));
            let (lo, hi) = (sa.min(sb), sa.max(sb));
            for r in &rs {
                if r.lo > lo && r.hi < hi {
                    let i = match v_out.iter().position(|p| (p.0, p.1) == (r.lo, r.hi)) {
                        Some(i) => i,
                        None => {
                            v_out.push((r.lo, r.hi, false, false));
                            v_out.len() - 1
                        }
                    };
                    if sb < sa {
                        v_out[i].2 = true;
                    } else {
                        v_out[i].3 = true;
                    }
                }
            }
        }
        v_out
    };
    // the full race check for a --timing ring: every judged transition (as s before and after),
    // the ring's ON set (an OFF ring's: the complement), what its lane filters (the first
    // repeater drops dips under 2 x deglitch gt; an OFF lane's delay-4 repeater after the torch
    // drops the ring output's pulses under 8 gt)
    let race_of = |seg: char| -> Option<crate::astack::Race> {
        if !x.timing.contains(seg) {
            return None;
        }
        let trans: Vec<(u8, u8)> = judged.iter().map(|&(v, w)| (code.s(v), code.s(w))).collect();
        Some(crate::astack::Race { trans, on: seg_mask(seg), dip_ok: 2 * deglitch as u32, pulse_ok: if off.contains(seg) { 8 } else { 0 } })
    };
    // the race check's context for one ring: its feed cells' times (a line feed: the line's time;
    // a coupling cell: the ring above's, the entry with the least distance winning) and its runs
    let set_ctx = |seg: char, feed: &Feed, above: &Feed, above_t: &Times| {
        let mut ft: std::collections::BTreeMap<(i32, i32, i32), (u8, u32)> = std::collections::BTreeMap::new();
        for &(c, v) in feed {
            let mut ts: Vec<u32> = above.iter().zip(above_t).filter(|((a, av), _)| *a == c && *av == v).map(|(_, (_, t))| *t).collect();
            if (c == DIRECT() || c == SECOND()) && v == 0 {
                ts.push(line_t(seg));
            }
            let t = ts.into_iter().min().unwrap_or(line_t(seg));
            let e = ft.entry(c).or_insert((v, t));
            if (v, t) < *e {
                *e = (v, t);
            }
        }
        crate::astack::FEED_T.with(|f| *f.borrow_mut() = ft.into_iter().map(|(c, (_, t))| (c, t)).collect());
        crate::astack::RACE.with(|r| *r.borrow_mut() = race_of(seg));
    };
    let clear_ctx = || {
        crate::astack::FEED_T.with(|f| f.borrow_mut().clear());
        crate::astack::PULSE_RUNS.with(|p| p.borrow_mut().clear());
        crate::astack::RACE.with(|r| *r.borrow_mut() = None);
    };
    // solve every ring first, top down per stack
    let mut solved: Vec<Vec<Solved>> = Vec::new();
    for st in &stacks {
        CELLS.with(|c| c.set((st.direct, st.second)));
        // --tight: the bridge on the axis facing the middle stack stays off (its target block, one
        // level down, would sit beside the middle rings' arc dust at their level and feed it)
        let stack_off = bridges_off | if tight && st.cx == 8 { 2 } else { 0 };
        let mut above: Feed = Vec::new();
        let mut above_t: Times = Vec::new();
        let mut v = Vec::new();
        for (i, &(seg, _row, level)) in st.rings.iter().enumerate() {
            let bridges_off = stack_off | x.bridges_off(seg);
            let rs = runs(seg_mask(seg));
            // every feed option (coupling only for a lower ring with --lean; a line of its own;
            // a line and a second feed), each pruned with `tries` orders: the cheapest ring plus
            // feed cost whose coupling still lets the ring below solve
            let only = std::env::var("RINGS2_PRUNE_ONLY").unwrap_or_default();
            let do_prune = prune_arcs && (only.is_empty() || only.contains(seg));
            let mut opts: Vec<(bool, bool)> = Vec::new();
            if i > 0 && lean && !x.direct.contains(seg) {
                opts.push((false, false));
            }
            opts.extend([(true, false), (true, true)]);
            let mut vs: Vec<(usize, Solved)> = Vec::new();
            for (direct, second) in opts {
                let mut feed: Feed = Vec::new();
                if direct {
                    feed.push((DIRECT(), 0));
                }
                if second {
                    feed.push((SECOND(), 0));
                }
                feed.extend(above.iter().copied());
                set_ctx(seg, &feed, &above, &above_t);
                let Some((slots, mask)) = (if feed.is_empty() { None } else { solve_ring_f(&rs, &feed, allowed, bridges_off) }) else {
                    clear_ctx();
                    continue;
                };
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
                clear_ctx();
            }
            ensure!(!vs.is_empty(), "no ring for {seg} {rs:?} (above {above:?})");
            vs.sort_by_key(|x| x.0);
            let below = st.rings.get(i + 1).map(|&(bs, _, _)| (bs, runs(seg_mask(bs))));
            // what a candidate couples into the ring below, with times (this ring's context set)
            let couple = |v: &Solved| -> (Feed, Times) {
                set_ctx(seg, &v.feed, &above, &above_t);
                crate::astack::REMOVED.with(|r| *r.borrow_mut() = v.removed.clone());
                let cpl = coupling(&v.slots, &v.feed, v.mask);
                let cpl_t = crate::astack::coupling_t(&v.slots, &v.feed, v.mask);
                crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
                clear_ctx();
                (cpl, cpl_t)
            };
            let pick = vs
                .iter()
                .position(|(_, v)| {
                    below.as_ref().is_none_or(|(bs, brs)| {
                        let (cpl, cpl_t) = couple(v);
                        // the ring below's own line feed is tried too: its time with the coupling's
                        let mut all = cpl.clone();
                        all.extend([(DIRECT(), 0), (SECOND(), 0)]);
                        set_ctx(*bs, &all, &cpl, &cpl_t);
                        let ok = solve(brs, &cpl, false, lean && !x.direct.contains(*bs), allowed, stack_off | x.bridges_off(*bs)).is_some();
                        clear_ctx();
                        ok
                    })
                })
                .ok_or_else(|| anyhow!("no {seg} ring lets the ring below solve"))?;
            let s = vs.swap_remove(pick).1;
            if !x.quiet {
                println!("  ring {seg} at level {level} ({} blocks): slots {:?} mask {:08b} direct {} second {} removed {:?} feed {:?}", ring_cost(&s), s.slots, s.mask, s.direct, s.second, s.removed, s.feed);
            }
            if (x.timing.contains(seg) || x.report) && !x.quiet {
                set_ctx(seg, &s.feed, &above, &above_t);
                crate::astack::PULSE_RUNS.with(|p| *p.borrow_mut() = pulse_runs(seg));
                // the report judges every ring (OFF rings with their lane's filters)
                let trans: Vec<(u8, u8)> = judged.iter().map(|&(v, w)| (code.s(v), code.s(w))).collect();
                crate::astack::RACE.with(|r| *r.borrow_mut() = Some(crate::astack::Race { trans, on: seg_mask(seg), dip_ok: 2 * deglitch as u32, pulse_ok: if off.contains(seg) { 8 } else { 0 } }));
                crate::astack::REMOVED.with(|r| *r.borrow_mut() = s.removed.clone());
                let times: Times = crate::astack::FEED_T.with(|f| f.borrow().iter().map(|(c, t)| (*c, *t)).collect());
                println!("    {seg} feed times {times:?}; watched runs {:?}", pulse_runs(seg));
                for l in crate::astack::pulse_report(&rs, &s.slots, &s.feed, s.mask) {
                    println!("    {seg} {l}");
                }
                if let Some((n, why)) = crate::astack::race_report(&rs, &s.slots, &s.feed, s.mask) {
                    println!("    {seg} race model: {n} glitching transition(s) {why:?}");
                }
                crate::astack::REMOVED.with(|r| r.borrow_mut().clear());
                clear_ctx();
            }
            let (cpl, cpl_t) = couple(&s);
            above = cpl;
            above_t = cpl_t;
            v.push(s);
        }
        solved.push(v);
    }
    let need = |si: usize, ri: usize| solved[si][ri].direct;
    let mut b = B::default();
    // the chain right behind the lines: its split two moves from the hub H = (2, 4, 12), from
    // which a, f and b's lines start (the chain's composters sit at z = 14 and 18, clear of them)
    // --chain-front (clk-obs): the chain one level down and four cells forward, under the lines (its
    // inputs driven from z = 14), s climbing to the hub beside it: the box 4 cells shallower.
    // --hub-west (with --chain-front --side-east): the chain's first s block (-2, 2, 12) feeds both
    // the hub (two climbs east, `uE uE`) and b's line (`uN uW W`, sharing the climb's first dust),
    // so b's line no longer runs west from the hub
    ensure!(!x.hub_west || (x.chain_front && x.side_east), "--hub-west needs --chain-front --side-east");
    let (chain_o, to_hub) = if x.hub_west {
        ([-9, 0, 12], "uE uE")
    } else if x.chain_front {
        ([-9, 0, 12], "S uE uN")
    } else {
        ([-7, 2, 16], "N N")
    };
    let split = crate::therm::chain_back_with(&mut b, chain_o, code, x.chain_front, x.hub_west);
    let hub = walk(&mut b, split, to_hub);
    ensure!(hub == [2, 4, 12], "hub at {hub:?}");
    let rear_of = |st: &Stack, level: i32| [st.cx + st.direct.0, level, cz + st.direct.2 + 2];
    let f_rear = walk(&mut b, hub, f_moves);
    let b_rear = if x.hub_west { walk(&mut b, split, "uN uW W") } else { walk(&mut b, hub, b_moves) };
    let a_rear = walk(&mut b, hub, a_moves);
    let mut rears: Vec<(P, P)> = vec![(a_rear, rear_of(&stacks[0], 5)), (f_rear, rear_of(&stacks[1], 4)), (b_rear, rear_of(&stacks[2], 4))];
    // descents (two levels down each): only for the lower rings that need their own feed; d's
    // starts from g's rear, so g's descent is built when either needs it
    if need(0, 1) || need(0, 2) {
        let g_rear = walk(&mut b, a_rear, g_moves);
        if need(0, 1) {
            rears.push((g_rear, rear_of(&stacks[0], 3)));
        }
        if need(0, 2) {
            let d_rear = walk(&mut b, g_rear, d_moves);
            rears.push((d_rear, rear_of(&stacks[0], 1)));
        }
    }
    if need(1, 1) {
        let e_rear = walk(&mut b, f_rear, e_moves);
        rears.push((e_rear, rear_of(&stacks[1], 2)));
    }
    if need(2, 1) {
        let c_rear = walk(&mut b, b_rear, c_moves);
        rears.push((c_rear, rear_of(&stacks[2], 2)));
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
                // a dust beside the direct feed's rear block, and a comparator into the second cell
                b.dust([st.cx + st.second.0, level, cz + st.second.2 + 2]);
                b.comp([st.cx + st.second.0, level, cz + st.second.2 + 1], D::N, false);
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
            // --deglitch: the lane's first repeater at delay `deglitch` (a repeater stays on when its
            // input drops for less than its delay: Java's DiodeBlock re-checks the input at the
            // scheduled tick), so the rings' short hand-over dips never reach the lamp
            b.rep([lane_x, y, cz - 1], D::N, if deglitch > 0 { deglitch } else { 1 });
            b.stone([lane_x, y, cz - 2]);
            if off.contains(seg) {
                // the ring gives NOT(segment): the first repeater (delay `deglitch`) drops its
                // short dips (the segment's short pulses); a wall torch on the stone inverts it; a
                // delay-4 repeater drops the segment's short dips and lights the lamp (a side
                // stack's lane then turns along the face as before: a stone, repeaters)
                ensure!(cz == 5, "--off lanes are 4 cells deep");
                b.set([lane_x, y, cz - 3], "redstone_wall_torch[facing=north,lit=true]");
                b.rep([lane_x, y, 1], D::N, 4);
                let lx = lamp_x(seg);
                if st.turn != 0 {
                    b.stone([lane_x, y, 0]);
                    let dir = if st.turn < 0 { D::W } else { D::E };
                    let mut x = lane_x + st.turn;
                    while x != lx {
                        b.rep([x, y, 0], dir, 1);
                        x += st.turn;
                    }
                }
                b.port(&format!("output.{seg}"), [lx, row, 0]);
                continue;
            }
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
    // stacks one column apart (--tight): a dust step between two stacks' rings (an upper dust and
    // a lower dust in neighbouring columns) connects their networks unless the cell above the
    // lower dust is solid; put a stone there (or fail when something else holds that cell)
    if tight {
        let stack_of = |x: i32| stacks.iter().position(|st| (st.cx - 3..=st.cx + 3).contains(&x));
        let is_dust = |b: &B, p: &P| b.blocks.get(p).is_some_and(|s| s.starts_with("minecraft:redstone_wire"));
        let dusts: Vec<P> = b.blocks.keys().copied().filter(|p| is_dust(&b, p) && (cz - 3..=cz + 3).contains(&p[2])).collect();
        let mut cuts: Vec<P> = Vec::new();
        for l in &dusts {
            for d in [[1, 0, 0], [-1, 0, 0]] {
                let u = [l[0] - d[0], l[1] + 1, l[2]];
                if is_dust(&b, &u) && stack_of(u[0]) != stack_of(l[0]) {
                    cuts.push([l[0], l[1] + 1, l[2]]);
                }
            }
        }
        for c in cuts {
            match b.blocks.get(&c) {
                None => b.stone(c),
                Some(s) if s == "minecraft:stone" || s.starts_with("minecraft:hopper") => {}
                Some(s) => anyhow::bail!("cut cell {c:?} holds {s}: two stacks' dust connect there"),
            }
        }
    }
    let bad = crate::agen::bad_supports(&b);
    let d = b.design("seven-segment-rings2", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    if !x.quiet {
        println!("{}: {} blocks; bad supports {:?}", out.display(), d.blocks.len(), bad);
    }
    // a sweep counts only buildable designs
    ensure!(!x.quiet || bad.is_empty(), "bad supports {bad:?}");
    Ok(d.blocks.len())
}

/// `mctas circuit display rings2-sweep` (clk-obs): for each code (the chain's bit weights) every
/// subset of `candidates` as the `--off` segments (the rings computing the complement, their lanes
/// filtering both ways), each built with the same options in parallel; one line per build (blocks,
/// code, subset), the cheapest first; the designs in `out_dir` (`off-<w>-<segs>.txt`). `judge` > 0:
/// the K cheapest finished (equalised, settled, constants in chests) and run through the judge's
/// own stimulus on redsim (their glitching phases).
#[allow(clippy::too_many_arguments)]
pub fn sweep(codes: &[Code], allowed: u8, bridges_off: u8, lean: bool, prune_arcs: bool, tries: u64, tight: bool, deglitch: u8, candidates: &str, x: &Extra, out_dir: &std::path::Path, threads: usize, judge: usize) -> Result<()> {
    std::fs::create_dir_all(out_dir)?;
    let segs: Vec<char> = candidates.chars().filter(|c| "abcdefg".contains(*c)).collect();
    let subsets: Vec<String> = (0..1u32 << segs.len()).map(|m| segs.iter().enumerate().filter(|(i, _)| m >> i & 1 == 1).map(|(_, c)| *c).collect()).collect();
    let jobs: Vec<(usize, String)> = (0..codes.len()).flat_map(|ci| subsets.iter().map(move |s| (ci, s.clone()))).collect();
    let wname = |ci: usize| codes[ci].w.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",");
    let file = |ci: usize, off: &str, what: &str| out_dir.join(format!("{what}-{}-{}.txt", wname(ci).replace(',', ""), if off.is_empty() { "none" } else { off }));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((ci, off)) = jobs.get(i) else { break };
                    let xo = Extra { timing: x.timing.clone(), direct: x.direct.clone(), ring_bridges_off: x.ring_bridges_off.clone(), middle_east: x.middle_east, report: false, side_east: x.side_east, quiet: true, chain_front: x.chain_front, hub_west: x.hub_west };
                    let r = build(&codes[*ci], allowed, bridges_off, lean, prune_arcs, tries, tight, deglitch, off, &xo, &file(*ci, off, "off"));
                    results.lock().unwrap().push((*ci, off.clone(), r.map_err(|e| e.to_string())));
                }
            });
        }
    });
    let mut v = results.into_inner().unwrap();
    v.sort_by_key(|(ci, off, r)| (r.as_ref().map(|n| *n).unwrap_or(usize::MAX), off.len(), *ci));
    let shown = |off: &str| if off.is_empty() { "-".to_string() } else { off.to_string() };
    let built = v.iter().filter(|x| x.2.is_ok()).count();
    for (ci, off, r) in &v {
        match r {
            Ok(n) => println!("{n}\tw={}\toff={}\t{}", wname(*ci), shown(off), file(*ci, off, "off").display()),
            Err(e) if codes.len() == 1 => println!("-\tw={}\toff={}\t{}", wname(*ci), shown(off), e.lines().next().unwrap_or("")),
            Err(_) => {}
        }
    }
    println!("{built} of {} builds", v.len());
    let top: Vec<(usize, String, usize)> = v.iter().filter_map(|(ci, off, r)| r.as_ref().ok().map(|n| (*ci, off.clone(), *n))).take(judge).collect();
    if top.is_empty() {
        return Ok(());
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let judged = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1).min(top.len()) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((ci, off, n)) = top.get(i) else { break };
                    let fin = file(*ci, off, "finished");
                    // the equaliser watches a cell on s's way whose state changes: the comparator
                    // into the hub (from the south, or from the west with --hub-west)
                    let at = if x.hub_west { [1, 4, 12] } else { [2, 4, 13] };
                    let r = crate::finish(&file(*ci, off, "off"), &fin, "seven-segment-rings2", at).and_then(|_| crate::glitch::run_judge(&fin));
                    judged.lock().unwrap().push((*ci, off.clone(), *n, fin, r.map_err(|e| e.to_string())));
                }
            });
        }
    });
    let mut j = judged.into_inner().unwrap();
    j.sort_by_key(|(_, _, n, _, r)| (r.as_ref().map(|l| l.len()).unwrap_or(usize::MAX), *n));
    for (ci, off, n, fin, r) in j {
        match r {
            Ok(lines) => println!("JUDGED {n}\tw={}\toff={}\t{} glitching phase(s)\t{}\t{}", wname(ci), shown(&off), lines.len() - 1, fin.display(), lines[..lines.len() - 1].join(" ; ")),
            Err(e) => println!("JUDGED {n}\tw={}\toff={}\tERROR {}", wname(ci), shown(&off), e.lines().next().unwrap_or("")),
        }
    }
    Ok(())
}
