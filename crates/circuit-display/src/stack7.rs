//! The 7x7 stack: [`crate::stack`] without the outer injection (whose tower stood at (0,-4)).
//! Per side (north shown): the diagonal injection (-1,-2) reading X2 at (-2,-2) (a repeater, or a
//! subtract comparator whose sides are the ring corner and the dust (-1,-3) beside the tower TL at
//! (-2,-3)), and under the rear a NOR: a wall torch at (0,-2,L-1) hanging on the block
//! (0,-3,L-1), that block fed by repeaters at (-1,-3,L-1) (from TL) and (1,-3,L-1) (from TR at
//! (2,-3)). The standing chain torch (NOT the rear below) takes the wall torch's cell instead.
//! TL is read at L (the dust) and at L-1 (the repeater): a tower with a block at the ring levels
//! shows the same value at both, else the complement (`tl_same`).

use crate::stack::{Lit, LITS, NAMES, corners, permutations, rear_fns, seg_roles};
use anyhow::Result;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Towers7 {
    pub x2: Lit,
    /// TL's value at the ring levels (the dust reads it there).
    pub tl: Lit,
    pub tl_same: bool,
    /// TR's value one level under the rings (only the NOR repeater reads it).
    pub tr: Lit,
}

pub fn all_towers7() -> Vec<Towers7> {
    let mut v = Vec::new();
    for x2 in LITS {
        for tl in LITS {
            for tl_same in [false, true] {
                for tr in LITS {
                    v.push(Towers7 { x2, tl, tl_same, tr });
                }
            }
        }
    }
    v
}

/// One rear of one ring: `diag` 0 none, 1 repeater, 2 comparator; `ydust` its TL dust side;
/// `wall` the NOR wall torch with its repeaters `left` (TL) and `right` (TR).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step7 {
    pub torch: bool,
    pub wall: bool,
    pub left: bool,
    pub right: bool,
    pub diag: u8,
    pub ydust: bool,
    pub rblock: bool,
    pub absent: bool,
    pub value: u16,
}

impl Step7 {
    pub fn cost(&self) -> usize {
        self.torch as usize + 2 * self.wall as usize + self.left as usize + self.right as usize + (self.diag > 0) as usize + self.ydust as usize + self.rblock as usize
    }
}

pub fn steps7(t: &Towers7, k: usize, prev: u16, corner: u16) -> Vec<Step7> {
    let mut out = vec![Step7 { rblock: true, value: 0xffff, ..Default::default() }];
    let x2 = t.x2.at(k);
    let tl_ring = t.tl.at(k);
    let tl_under = if t.tl_same { tl_ring } else { !tl_ring };
    let tr_under = t.tr.at(k);
    for torch in [false, true] {
        for wall in [false, true] {
            if torch && wall {
                continue; // one cell
            }
            for (left, right) in [(false, false), (true, false), (false, true), (true, true)] {
                if !wall && (left || right) {
                    continue;
                }
                for diag in 0..3u8 {
                    for ydust in [false, true] {
                        if ydust && diag != 2 {
                            continue;
                        }
                        let mut r = if torch { !prev } else { 0 };
                        if wall {
                            let fed = if left { tl_under } else { 0 } | if right { tr_under } else { 0 };
                            r |= !fed;
                        }
                        r |= match diag {
                            1 => x2,
                            2 => x2 & !(corner | if ydust { tl_ring } else { 0 }),
                            _ => 0,
                        };
                        out.push(Step7 { torch, wall, left, right, diag, ydust, rblock: false, absent: false, value: r });
                    }
                }
            }
        }
    }
    out
}

/// The DP of one column; the state is (rear value, whether the TL dust was used at the ring
/// below: the left NOR repeater stands on that cell).
pub fn column7(fs: &[u16], t: &Towers7, sels: &[u16], corners: &[u16]) -> Option<Vec<Step7>> {
    type St = (u16, bool);
    let mut layers: Vec<BTreeMap<St, (St, Step7)>> = Vec::new();
    let mut cur: Vec<St> = vec![(0, false)];
    for (k, &f) in fs.iter().enumerate() {
        let sel = sels[k];
        let mut next: BTreeMap<St, (St, Step7)> = BTreeMap::new();
        for &(prev, ydust_below) in &cur {
            for mut s in steps7(t, k, prev, corners[k]) {
                if s.left && (ydust_below || s.ydust) {
                    continue; // the repeater needs a block under it, and the dust one under it
                }
                if (s.value ^ f) & sel != 0 {
                    if f & sel != 0 {
                        continue;
                    }
                    s.absent = true;
                }
                let key = (s.value, s.ydust);
                let e = next.entry(key).or_insert(((prev, ydust_below), s));
                if s.cost() < e.1.cost() {
                    *e = ((prev, ydust_below), s);
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        cur = next.keys().copied().collect();
        layers.push(next);
    }
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

pub struct Solution7 {
    pub perm: [u32; 4],
    pub assign: u32,
    pub order: Vec<usize>,
    pub sides: Vec<(Towers7, Vec<Step7>)>,
}

pub fn side_cost7(st: &[Step7]) -> usize {
    let firsts = [st.iter().position(|s| s.diag > 0), st.iter().position(|s| s.ydust || s.left), st.iter().position(|s| s.right)];
    let towers: usize = firsts.iter().flatten().map(|&k| 14 + 3 * (st.len() - k)).sum();
    st.iter().map(|s| s.cost()).sum::<usize>() + towers
}

pub fn solve7(perm: [u32; 4], assign: u32, order: &[usize], towers: &[Towers7]) -> Option<Solution7> {
    let c0 = corners(assign);
    let fs: Vec<[u16; 4]> = order.iter().enumerate().map(|(k, &s)| rear_fns(seg_roles(NAMES[s], perm), c0, k)).collect();
    let mut sides = Vec::new();
    for j in 0..4 {
        let col: Vec<u16> = fs.iter().map(|f| f[j]).collect();
        let cs: Vec<[u16; 4]> = (0..order.len()).map(|k| if k % 2 == 1 { c0.map(|x| !x) } else { c0 }).collect();
        let sels: Vec<u16> = cs.iter().map(|c| !c[j] & !c[(j + 1) % 4]).collect();
        let corners_j: Vec<u16> = cs.iter().map(|c| c[j]).collect();
        let mut best: Option<(usize, Towers7, Vec<Step7>)> = None;
        for t in towers {
            if let Some(st) = column7(&col, t, &sels, &corners_j) {
                let cost = side_cost7(&st);
                if best.as_ref().is_none_or(|b| cost < b.0) {
                    best = Some((cost, *t, st));
                }
            }
        }
        let (_, t, st) = best?;
        sides.push((t, st));
    }
    Some(Solution7 { perm, assign, order: order.to_vec(), sides })
}

pub fn cost7(s: &Solution7) -> usize {
    s.sides.iter().map(|(_, st)| side_cost7(st)).sum()
}

pub fn search_best7() -> Option<Solution7> {
    let towers = all_towers7();
    let perms: Vec<[u32; 4]> = permutations(4).into_iter().map(|p| [p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32]).collect();
    let orders = permutations(7);
    let results: Vec<(usize, Option<Solution7>)> = std::thread::scope(|sc| {
        let handles: Vec<_> = perms
            .chunks(3)
            .map(|chunk| {
                let (towers, orders) = (&towers, &orders);
                sc.spawn(move || {
                    let mut best: Option<Solution7> = None;
                    let mut n = 0;
                    for &perm in chunk {
                        for assign in 0..8 {
                            for order in orders {
                                if let Some(s) = solve7(perm, assign, order, towers) {
                                    n += 1;
                                    if best.as_ref().is_none_or(|b| cost7(&s) < cost7(b)) {
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
    results.into_iter().filter_map(|r| r.1).min_by_key(cost7)
}

pub fn describe7(s: &Solution7) -> String {
    let names: String = s.order.iter().map(|&k| NAMES[k]).collect();
    let mut out = format!("roles {:?} corners {} order {names} cost {}\n", s.perm, s.assign, cost7(s));
    for (j, (t, st)) in s.sides.iter().enumerate() {
        let steps: Vec<String> = st
            .iter()
            .map(|s| {
                if s.absent {
                    return "-".into();
                }
                let d = ["", "d", "D"][s.diag as usize];
                let w = if s.wall { format!("W{}{}", if s.left { "l" } else { "" }, if s.right { "r" } else { "" }) } else { String::new() };
                format!("{}{w}{d}{}{}", if s.torch { "T" } else { "" }, if s.ydust { "y" } else { "" }, if s.rblock { "R" } else { "" })
            })
            .collect();
        out.push_str(&format!("  side {j}: {t:?}: {}\n", steps.join(" ")));
    }
    out
}

pub fn search7() -> Result<()> {
    match search_best7() {
        Some(s) => print!("{}", describe7(&s)),
        None => println!("no solution"),
    }
    Ok(())
}
