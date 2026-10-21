//! The analog ladder (crates/circuit-display/src/ladder.rs): straight s lines (fed from the west,
//! distance d = x + alpha) and t lines (t = 15 - s, fed from the east, e = (L-1-x) + beta)
//! alternate in z with gap rows. In a gap row, window comparators W face in pairs into a block B
//! (pattern K W B W K W B W K along x, K a rear hopper of constant k); the lamp sits on B; a second
//! ladder two levels up feeds the lamp from above. W at x is on iff s in [16-k-e, k+d-1].

use crate::analog::{Code, Run, all_codes, cost, masks, runs};

/// The rear constant for run r on a W with line distances (d, e), or None.
pub fn w_k(r: Run, d: i32, e: i32) -> Option<u8> {
    let (lo, hi) = (r.lo as i32, r.hi as i32);
    let ok = |k: i32| (1..=15).contains(&k);
    if lo > 0 && hi < 15 {
        if d > 14 || e > 14 || d - e != lo + hi - 15 {
            return None;
        }
        let k = hi + 1 - d;
        return ok(k).then_some(k as u8);
    }
    if hi == 15 {
        if e > 14 {
            return None;
        }
        let k = 16 - lo - e;
        return (ok(k) && 15 - d < k).then_some(k as u8);
    }
    if d > 14 {
        return None;
    }
    let k = hi + 1 - d;
    (ok(k) && 15 - e < k).then_some(k as u8)
}

/// A geometry: line length, site positions, per gap row and level the (alpha, beta) of its lines.
pub struct Geo {
    pub len: i32,
    pub sites_x: Vec<i32>,
    /// per (row, level): (alpha of its s line, beta of its t line)
    pub ab: Vec<[(i32, i32); 2]>,
}

/// The 4 W's of a site: (d, e) for lower-left, lower-right, upper-left, upper-right.
pub fn site_ws(g: &Geo, row: usize, x0: i32) -> [(i32, i32); 4] {
    let mut out = [(0, 0); 4];
    for lvl in 0..2 {
        let (a, b) = g.ab[row][lvl];
        for (j, x) in [x0 - 1, x0 + 1].into_iter().enumerate() {
            out[lvl * 2 + j] = (x + a, (g.len - 1 - x) + b);
        }
    }
    out
}

/// Assign runs to the 4 W's; returns per W (run index, k).
pub fn fit_site(rs: &[Run], ws: &[(i32, i32); 4]) -> Option<[Option<(usize, u8)>; 4]> {
    fn rec(rs: &[Run], ws: &[(i32, i32); 4], i: usize, used: &mut [Option<(usize, u8)>; 4]) -> bool {
        if i == rs.len() {
            return true;
        }
        for w in 0..4 {
            if used[w].is_some() {
                continue;
            }
            if let Some(k) = w_k(rs[i], ws[w].0, ws[w].1) {
                used[w] = Some((i, k));
                if rec(rs, ws, i + 1, used) {
                    return true;
                }
                used[w] = None;
            }
        }
        false
    }
    let mut used = [None; 4];
    rec(rs, ws, 0, &mut used).then_some(used)
}

/// Flexible sites: one lamp per row at x0; per level a left W anywhere in 1..x0-1 and a right W
/// anywhere in x0+1..len-2 (repeaters carry its output to B). Returns per slot (run, x, k).
pub fn fit_site2(rs: &[Run], g: &Geo, row: usize, x0: i32) -> Option<[Option<(usize, i32, u8)>; 4]> {
    let opts: Vec<Vec<(i32, i32, i32)>> = (0..4)
        .map(|slot| {
            let (a, b) = g.ab[row][slot / 2];
            let xs: Vec<i32> = if slot % 2 == 0 { (1..x0).collect() } else { (x0 + 1..g.len - 1).collect() };
            xs.into_iter().map(|x| (x, x + a, (g.len - 1 - x) + b)).collect()
        })
        .collect();
    fn rec(rs: &[Run], opts: &[Vec<(i32, i32, i32)>], i: usize, used: &mut [Option<(usize, i32, u8)>; 4]) -> bool {
        if i == rs.len() {
            return true;
        }
        for w in 0..4 {
            if used[w].is_some() {
                continue;
            }
            for &(x, d, e) in &opts[w] {
                if let Some(k) = w_k(rs[i], d, e) {
                    used[w] = Some((i, x, k));
                    if rec(rs, opts, i + 1, used) {
                        return true;
                    }
                    used[w] = None;
                }
            }
        }
        false
    }
    let mut used = [None; 4];
    rec(rs, &opts, 0, &mut used).then_some(used)
}

/// Match the 7 segments to sites (row, x0); returns per segment its site index.
pub fn match_segments(code: &Code, g: &Geo) -> Option<Vec<usize>> {
    let sites: Vec<(usize, i32)> = (0..g.ab.len()).flat_map(|r| g.sites_x.iter().map(move |&x| (r, x))).collect();
    let segs: Vec<Vec<Run>> = masks(code).iter().map(|m| runs(*m)).collect();
    let fits: Vec<Vec<bool>> = segs
        .iter()
        .map(|rs| sites.iter().map(|&(r, x)| if g.sites_x.len() == 1 { fit_site2(rs, g, r, x).is_some() } else { fit_site(rs, &site_ws(g, r, x)).is_some() }).collect())
        .collect();
    // bipartite matching (Kuhn)
    let mut owner: Vec<Option<usize>> = vec![None; sites.len()];
    fn aug(s: usize, fits: &[Vec<bool>], seen: &mut [bool], owner: &mut [Option<usize>]) -> bool {
        for t in 0..owner.len() {
            if fits[s][t] && !seen[t] {
                seen[t] = true;
                if owner[t].is_none_or(|o| aug(o, fits, seen, owner)) {
                    owner[t] = Some(s);
                    return true;
                }
            }
        }
        false
    }
    for s in 0..segs.len() {
        let mut seen = vec![false; sites.len()];
        if !aug(s, &fits, &mut seen, &mut owner) {
            return None;
        }
    }
    let mut out = vec![0; segs.len()];
    for (t, o) in owner.iter().enumerate() {
        if let Some(s) = o {
            out[*s] = t;
        }
    }
    Some(out)
}

/// Random search over codes and line offsets for a ladder that hosts all 7 segments.
pub fn search(rows: usize, len: i32, sites_x: &[i32], tries: u64, amax: i32) {
    let mut codes: Vec<(Code, (usize, usize))> = all_codes().into_iter().map(|c| (c, cost(&c))).collect();
    codes.retain(|(c, _)| masks(c).iter().all(|m| runs(*m).len() <= 4));
    codes.sort_by_key(|x| x.1);
    let mut seed = 0x9e3779b97f4a7c15u64;
    let mut rnd = |n: i32| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as i32
    };
    let mut found = 0;
    for (code, (inner, total)) in &codes {
        for _ in 0..tries {
            // lines: s at even line index, t at odd; row r lies between lines r and r+1
            let nl = rows + 1;
            let off: Vec<[i32; 2]> = (0..nl).map(|_| [rnd(amax + 1), rnd(amax + 1)]).collect();
            let ab: Vec<[(i32, i32); 2]> = (0..rows)
                .map(|r| {
                    let (sl, tl) = if r % 2 == 0 { (r, r + 1) } else { (r + 1, r) };
                    [(off[sl][0], off[tl][0]), (off[sl][1], off[tl][1])]
                })
                .collect();
            let g = Geo { len, sites_x: sites_x.to_vec(), ab };
            if let Some(m) = match_segments(code, &g) {
                println!("FOUND w {:?} neg {:04b} ({inner} windows, {total} runs) offsets {off:?} sites {m:?}", code.w, code.neg);
                found += 1;
                break;
            }
        }
        if found >= 5 {
            break;
        }
    }
    if found == 0 {
        println!("none");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore]
    fn ladder_diag() {
        for (w, neg) in [([4u8, 1, 8, 2], 15u8), ([8, 1, 4, 2], 1)] {
            let code = Code { w, neg };
            for (si, m) in masks(&code).iter().enumerate() {
                let rs = runs(*m);
                let mut ok = 0;
                for a0 in 0..6 {
                    for b0 in 0..6 {
                        for a1 in 0..6 {
                            for b1 in 0..6 {
                                let g = Geo { len: 11, sites_x: vec![5], ab: vec![[(a0, b0), (a1, b1)]] };
                                if fit_site2(&rs, &g, 0, 5).is_some() {
                                    ok += 1;
                                }
                            }
                        }
                    }
                }
                println!("DIAG {w:?} {neg} seg {si} runs {rs:?}: {ok} offset sets fit");
            }
        }
    }

    #[test]
    #[ignore]
    fn ladder_search() {
        for rows in [7usize] {
            for len in [9, 11, 13] {
                println!("rows {rows} len {len}");
                search(rows, len, &[len / 2], 2000, 5);
            }
        }
    }
}
