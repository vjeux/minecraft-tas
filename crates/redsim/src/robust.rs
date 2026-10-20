//! Robustness over a continuous random input (clk-misc, 2026-10-20; vjeux: "make sure they pass
//! every possible random number generated, not just test 64"). A primed TNT's kick angle is
//! `d * (float)(2 pi)` with `d = nextDouble()` in [0, 1): a design's outcome is piecewise constant
//! in each `d` (the TNT either clears an edge or not). [`critical_points`] finds the pieces of one
//! `d` (the others held by the caller): a sweep of `samples` evenly spaced values, then a
//! bisection of every change between neighbours down to `tol`; every piece and both sides of every
//! change are then known outcomes (a change narrower than the sweep's step, with the same outcome
//! on both sides, is the sweep's blind spot: refine `samples` for it).
//!
//! The other random inputs of a TNT design (`World::tnt_kicks`, `World::chain_fuses`,
//! `World::explosion_rolls`) are injected the same way; `Rolls::Max` covers every ray roll at once.

/// One piece of [0, 1): every `d` in `lo..hi` gives `outcome` (as far as the sweep can tell).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub lo: f64,
    pub hi: f64,
    pub outcome: u64,
}

/// The largest `nextDouble()` (`(2^53 - 1) / 2^53`).
pub const MAX_DOUBLE: f64 = 1.0 - 1.0 / 9_007_199_254_740_992.0;

/// The pieces of `outcome(d)` over d in [0, 1) (`samples` sweep points, every change bisected to
/// `tol`), in order, and every point evaluated near a change (`(d, outcome)`: the last left value
/// and the first right value of each change, sorted).
pub fn critical_points(outcome: impl Fn(f64) -> u64, samples: usize, tol: f64) -> (Vec<Piece>, Vec<(f64, u64)>) {
    let n = samples.max(1);
    let mut xs: Vec<f64> = (0..n).map(|i| i as f64 / n as f64).collect();
    xs.push(MAX_DOUBLE);
    let ys: Vec<u64> = xs.iter().map(|&d| outcome(d)).collect();
    // changes, refined: (left d, left outcome, right d, right outcome)
    let mut changes: Vec<(f64, u64, f64, u64)> = Vec::new();
    let mut stack: Vec<(f64, u64, f64, u64)> = Vec::new();
    for i in 0..xs.len() - 1 {
        if ys[i] != ys[i + 1] {
            stack.push((xs[i], ys[i], xs[i + 1], ys[i + 1]));
        }
    }
    while let Some((mut lo, olo, mut hi, ohi)) = stack.pop() {
        loop {
            if hi - lo <= tol {
                changes.push((lo, olo, hi, ohi));
                break;
            }
            let mid = lo + (hi - lo) / 2.0;
            if mid <= lo || mid >= hi {
                changes.push((lo, olo, hi, ohi));
                break;
            }
            let om = outcome(mid);
            if om == olo {
                lo = mid;
            } else if om == ohi {
                hi = mid;
            } else {
                // a third outcome in between: two changes, each bisected on its own
                stack.push((mid, om, hi, ohi));
                stack.push((lo, olo, mid, om));
                break;
            }
        }
    }
    changes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let mut pieces = Vec::new();
    let mut start = 0.0;
    let mut cur = ys[0];
    for &(lo, olo, hi, ohi) in &changes {
        if olo != cur {
            // (a change found inside another's bisection keeps the order consistent)
            cur = olo;
        }
        pieces.push(Piece { lo: start, hi, outcome: cur });
        let _ = lo;
        start = hi;
        cur = ohi;
    }
    pieces.push(Piece { lo: start, hi: 1.0, outcome: cur });
    let mut points: Vec<(f64, u64)> = changes.iter().flat_map(|&(lo, olo, hi, ohi)| [(lo, olo), (hi, ohi)]).collect();
    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    points.dedup();
    (pieces, points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_both_edges_of_a_window() {
        let f = |d: f64| if (0.3..0.30001).contains(&d) { 1 } else { 0 };
        let (pieces, points) = critical_points(f, 4096, 1e-12);
        // the window is narrower than the step (1/4096 = 0.000244): a sample lands in it only by
        // luck; widen it so the sweep sees it
        let g = |d: f64| if (0.3..0.301).contains(&d) { 1 } else { 0 };
        let (gp, gpts) = critical_points(g, 4096, 1e-12);
        assert_eq!(gp.len(), 3);
        assert!((gp[1].lo - 0.3).abs() < 1e-11 && (gp[1].hi - 0.301).abs() < 1e-11);
        assert_eq!(gpts.len(), 4);
        let _ = (pieces, points);
    }
}
