//! The compact stack (clk-obs plan, 2026-10-21): the 7 rings every 2 levels in one column, the 4
//! ring corners fed by 4 torch towers in a pinwheel next to them, every rear column a torch chain
//! (rear_k = NOT rear_(k-1) through the torch between them) OR two injections: the outer one at
//! (0,-3) from a literal tower X1 at (0,-4), the diagonal one at (-1,-2) from a tower X2 at
//! (-2,-2) (N side; the others rotated), each a repeater (X) or a subtract comparator (X AND NOT
//! Y, Y the dust at (-1,-3) beside a tower at (-1,-4), shared by both comparators; the diagonal
//! comparator's other side is the ring corner, a factor the ring comparator applies anyway).
//! `mctas circuit display stack-search` finds the input roles, segment orders and towers this
//! realizes.

use anyhow::Result;
use circuit::compute::SEGMENTS;
use std::collections::HashMap;

pub fn lit(k: u32) -> u16 {
    (0..16u32).filter(|v| v >> k & 1 == 1).fold(0, |m, v| m | 1 << v)
}

pub const NAMES: [char; 7] = ['a', 'b', 'c', 'd', 'e', 'f', 'g'];

/// Segment `c` in role bits: role bit i is input bit perm[i] (corners use roles 3 and 2, the
/// literal towers roles 1 and 0).
pub fn seg_roles(c: char, perm: [u32; 4]) -> u16 {
    let mut m = 0u16;
    for v in 0..16u32 {
        let u: u32 = (0..4).map(|i| (v >> i & 1) << perm[i]).sum();
        if SEGMENTS[u as usize].contains(c) {
            m |= 1 << v;
        }
    }
    m
}

/// Corner values at ring 0 (NW, NE, SE, SW): bit 0 of `assign` swaps roles 3/2 between the NW/SE
/// and NE/SW pairs, bits 1-2 the NW and NE polarities.
pub fn corners(assign: u32) -> [u16; 4] {
    let (va, vb) = if assign & 1 == 0 { (lit(3), lit(2)) } else { (lit(2), lit(3)) };
    let nw = if assign & 2 == 0 { va } else { !va };
    let ne = if assign & 4 == 0 { vb } else { !vb };
    [nw, ne, !nw, !ne]
}

/// The rear functions of ring k (comparators N, E, S, W): the corners flip every ring.
pub fn rear_fns(seg: u16, c0: [u16; 4], k: usize) -> [u16; 4] {
    let c = if k % 2 == 1 { c0.map(|x| !x) } else { c0 };
    let sides = [(c[0], c[1]), (c[1], c[2]), (c[2], c[3]), (c[3], c[0])];
    let mut out = [0u16; 4];
    for (j, &(s1, s2)) in sides.iter().enumerate() {
        let sel = !s1 & !s2;
        let row = (0..16).find(|&v| sel >> v & 1 == 1).unwrap() & 12;
        out[j] = (0..16).filter(|&v| seg >> (row | (v & 3)) & 1 == 1).fold(0u16, |m, v| m | 1 << v);
    }
    out
}

/// A literal tower: role 0 or 1, its polarity at ring 0 (it flips every ring).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lit {
    pub var: u32,
    pub neg: bool,
}

impl Lit {
    pub fn at(&self, k: usize) -> u16 {
        let x = lit(self.var);
        if self.neg ^ (k % 2 == 1) { !x } else { x }
    }
}

pub const LITS: [Lit; 4] = [Lit { var: 0, neg: false }, Lit { var: 0, neg: true }, Lit { var: 1, neg: false }, Lit { var: 1, neg: true }];

/// One side's literal towers: X1 behind the outer injection, X2 behind the diagonal one, Y1 beside
/// the outer comparator's own side dust (1,-3), Y2 beside the dust both comparators share (-1,-3).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Towers {
    pub x1: Lit,
    pub x2: Lit,
    pub y1: Lit,
    pub y2: Lit,
}

pub fn all_towers() -> Vec<Towers> {
    let mut v = Vec::new();
    for x1 in LITS {
        for x2 in LITS {
            for y1 in LITS {
                for y2 in LITS {
                    v.push(Towers { x1, x2, y1, y2 });
                }
            }
        }
    }
    v
}

/// How one rear of one ring is made. `outer`/`diag`: 0 none, 1 repeater, 2 comparator.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub torch: bool,
    pub outer: u8,
    pub diag: u8,
    pub y1dust: bool,
    pub y2dust: bool,
    pub rblock: bool,
    pub absent: bool,
    pub value: u16,
}

impl Step {
    pub fn cost(&self) -> usize {
        self.torch as usize + (self.outer > 0) as usize + (self.diag > 0) as usize + self.y1dust as usize + self.y2dust as usize + self.rblock as usize
    }
}

/// Every step at ring k after a rear valued `prev` (ring 0's rear has a plain block under it, so
/// a torch there is a constant 1).
/// Which injections exist (a search knob): bit 0 forbids the outer one (a 7x7 footprint), bit 1
/// the outer comparator's own side dust Y1.
pub static MODEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Every step at ring k after a rear valued `prev`; `corner` is the ring corner beside the
/// diagonal comparator's other side (it subtracts it: X2 AND NOT (Y2 OR corner)).
pub fn steps(t: &Towers, k: usize, prev: u16, corner: u16) -> Vec<Step> {
    let model = MODEL.load(std::sync::atomic::Ordering::Relaxed);
    let max_outer = if model & 1 == 1 { 1 } else { 3 };
    let y1_ok = model & 3 == 0;
    let mut out = vec![Step { rblock: true, value: 0xffff, ..Default::default() }];
    let (x1, x2, y1, y2) = (t.x1.at(k), t.x2.at(k), t.y1.at(k), t.y2.at(k));
    for torch in [false, true] {
        for y1dust in [false, true] {
            for y2dust in [false, true] {
                for outer in 0..max_outer as u8 {
                    if y1dust && !y1_ok {
                        continue;
                    }
                    for diag in 0..3u8 {
                        // a repeater only with a comparator reading it
                        if (y1dust && outer != 2) || (y2dust && diag != 2) {
                            continue;
                        }
                        // repeaters carry Y1 into the outer comparator's side and Y2 into the
                        // diagonal one's
                        let ys_outer = if y1dust { y1 } else { 0 };
                        let ys_diag = if y2dust { y2 } else { 0 } | corner;
                        if outer == 2 && !y1dust {
                            continue; // an outer comparator with no side is a repeater
                        }
                        let mut r = if torch { !prev } else { 0 };
                        r |= match outer {
                            1 => x1,
                            2 => x1 & !ys_outer,
                            _ => 0,
                        };
                        r |= match diag {
                            1 => x2,
                            2 => x2 & !ys_diag,
                            _ => 0,
                        };
                        out.push(Step { torch, outer, diag, y1dust, y2dust, rblock: false, absent: false, value: r });
                    }
                }
            }
        }
    }
    out
}

/// The DP of one column: per ring, reachable rear values with a back pointer. The ring's
/// comparator only passes its row `sels[k]`: the rear must equal F there (anything elsewhere),
/// and with F empty on its row the comparator is left out.
pub fn column(fs: &[u16], t: &Towers, sels: &[u16], corners: &[u16]) -> Option<Vec<Step>> {
    let mut layers: Vec<HashMap<u16, (u16, Step)>> = Vec::new();
    let mut cur: Vec<u16> = vec![0];
    for (k, &f) in fs.iter().enumerate() {
        let sel = sels[k];
        let mut next: HashMap<u16, (u16, Step)> = HashMap::new();
        for &prev in &cur {
            for mut s in steps(t, k, prev, corners[k]) {
                if (s.value ^ f) & sel != 0 {
                    if f & sel != 0 {
                        continue;
                    }
                    s.absent = true;
                }
                let cost = |s: &Step| s.cost();
                let e = next.entry(s.value).or_insert((prev, s));
                if cost(&s) < cost(&e.1) {
                    *e = (prev, s);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        cur = next.keys().copied().collect();
        layers.push(next);
    }
    // back from any final value
    let mut v = *layers.last()?.keys().next()?;
    let mut out = Vec::new();
    for layer in layers.iter().rev() {
        let (prev, s) = layer[&v];
        out.push(s);
        v = prev;
    }
    out.reverse();
    Some(out)
}

fn permutations(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![vec![]];
    }
    let mut out = Vec::new();
    for p in permutations(n - 1) {
        for i in 0..=p.len() {
            let mut q = p.clone();
            q.insert(i, n - 1);
            out.push(q);
        }
    }
    out
}

/// A solution: input roles, corner assignment, segment order, per side (towers, steps).
pub struct Solution {
    pub perm: [u32; 4],
    pub assign: u32,
    pub order: Vec<usize>,
    pub sides: Vec<(Towers, Vec<Step>)>,
}

pub fn solve(perm: [u32; 4], assign: u32, order: &[usize], towers: &[Towers]) -> Option<Solution> {
    let c0 = corners(assign);
    let fs: Vec<[u16; 4]> = order.iter().enumerate().map(|(k, &s)| rear_fns(seg_roles(NAMES[s], perm), c0, k)).collect();
    let mut sides = Vec::new();
    for j in 0..4 {
        let col: Vec<u16> = fs.iter().map(|f| f[j]).collect();
        let cs: Vec<[u16; 4]> = (0..order.len()).map(|k| if k % 2 == 1 { c0.map(|x| !x) } else { c0 }).collect();
        let sels: Vec<u16> = cs.iter().map(|c| !c[j] & !c[(j + 1) % 4]).collect();
        let corners_j: Vec<u16> = cs.iter().map(|c| c[j]).collect();
        let mut best: Option<(usize, Towers, Vec<Step>)> = None;
        for t in towers {
            if let Some(st) = column(&col, t, &sels, &corners_j) {
                let cost: usize = side_cost(&st);
                if best.as_ref().is_none_or(|b| cost < b.0) {
                    best = Some((cost, *t, st));
                }
            }
        }
        let (_, t, st) = best?;
        sides.push((t, st));
    }
    Some(Solution { perm, assign, order: order.to_vec(), sides })
}

pub fn side_cost(st: &[Step]) -> usize {
    let used = st.iter().any(|s| s.outer > 0) as usize + st.iter().any(|s| s.diag > 0) as usize + st.iter().any(|s| s.y1dust) as usize + st.iter().any(|s| s.y2dust) as usize;
    st.iter().map(|s| s.cost()).sum::<usize>() + 20 * used
}

pub fn cost(s: &Solution) -> usize {
    s.sides.iter().map(|(_, st)| side_cost(st)).sum()
}

pub fn search_best() -> Option<Solution> {
    let towers = all_towers();
    let perms: Vec<[u32; 4]> = permutations(4).into_iter().map(|p| [p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32]).collect();
    let orders = permutations(7);
    // one thread per input-role permutation group (8 threads: the box's heavy-process budget)
    let results: Vec<(usize, Option<Solution>)> = std::thread::scope(|sc| {
        let handles: Vec<_> = perms
            .chunks(3)
            .map(|chunk| {
                let (towers, orders) = (&towers, &orders);
                sc.spawn(move || {
                    let mut best: Option<Solution> = None;
                    let mut n = 0;
                    for &perm in chunk {
                        for assign in 0..8 {
                            for order in orders {
                                if let Some(s) = solve(perm, assign, order, towers) {
                                    n += 1;
                                    if best.as_ref().is_none_or(|b| cost(&s) < cost(b)) {
                                        best = Some(s);
                                    }
                                }
                            }
                        }
                    }
                    (n, best)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let n: usize = results.iter().map(|r| r.0).sum();
    eprintln!("{n} feasible (roles, corners, order)");
    results.into_iter().filter_map(|r| r.1).min_by_key(cost)
}

pub fn describe(s: &Solution) -> String {
    let names: String = s.order.iter().map(|&k| NAMES[k]).collect();
    let mut out = format!("roles {:?} corners {} order {names} cost {}\n", s.perm, s.assign, cost(s));
    for (j, (t, st)) in s.sides.iter().enumerate() {
        let steps: Vec<String> = st
            .iter()
            .map(|s| {
                if s.absent {
                    return "-".into();
                }
                let o = ["", "o", "O"][s.outer as usize];
                let d = ["", "d", "D"][s.diag as usize];
                format!("{}{o}{d}{}{}{}", if s.torch { "T" } else { "" }, if s.y1dust { "y" } else { "" }, if s.y2dust { "Y" } else { "" }, if s.rblock { "R" } else { "" })
            })
            .collect();
        out.push_str(&format!("  side {j}: {t:?}: {}\n", steps.join(" ")));
    }
    out
}

pub fn search(model: u8) -> Result<()> {
    MODEL.store(model, std::sync::atomic::Ordering::Relaxed);
    match search_best() {
        Some(s) => print!("{}", describe(&s)),
        None => println!("no solution"),
    }
    Ok(())
}
