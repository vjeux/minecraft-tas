//! `mctas circuit display compact7` (clk-obs, 2026-10-22): the search seed for the 7-segment display
//! of one signal strength (designs/briefs/display-7seg.md, layouts 4 + 2): a dust ladder from the
//! chest's comparator holds s - d at its cell d (a zero-delay comparison: "s > d"), and every
//! segment is an OR of its runs of values:
//! - a run [0, hi] is a torch hung on the support of ladder cell hi (lit iff that cell is dark);
//! - a run [lo, 15] is a repeater reading ladder cell lo - 1;
//! - an inner run [lo, hi] is a subtract comparator whose rear reads ladder cell lo - 1 (s - lo + 1)
//!   and whose side a repeater reading ladder cell hi powers (15 iff s > hi).
//! Each term's source repeater is routed (crate::route) to its own cell of the segment's comb, a
//! walled dust line feeding the face module's socket (crate::obvious's face: segment stones, dots,
//! corner dusts).

use circuit::compute::SEGMENTS;

/// The runs of values a lamp lights on (maximal intervals of its lit set).
pub fn runs(lit: impl Fn(usize) -> bool) -> Vec<(u8, u8)> {
    let mut v = Vec::new();
    let mut start: Option<usize> = None;
    for s in 0..=16 {
        let on = s < 16 && lit(s);
        match (start, on) {
            (None, true) => start = Some(s),
            (Some(a), false) => {
                v.push((a as u8, (s - 1) as u8));
                start = None;
            }
            _ => {}
        }
    }
    v
}

/// Every segment's runs: (segment, lo, hi).
pub fn terms() -> Vec<(char, u8, u8)> {
    let mut t = Vec::new();
    for seg in "abcdefg".chars() {
        for (lo, hi) in runs(|s| SEGMENTS[s].contains(seg)) {
            t.push((seg, lo, hi));
        }
    }
    t
}

/// The decimal display's terms (values 0..9; 10..15 never happen): each segment's runs over 0..9,
/// a run reaching 9 extended to 15 (a top window: one repeater on the ladder).
pub fn decimal_terms() -> Vec<(char, u8, u8)> {
    let mut t = Vec::new();
    for seg in "abcdefg".chars() {
        for (lo, hi) in runs(|s| s < 10 && SEGMENTS[s].contains(seg)) {
            t.push((seg, lo, if hi == 9 { 15 } else { hi }));
        }
    }
    t
}

#[cfg(test)]
mod tests {
    #[test]
    fn segment_runs() {
        let t = super::terms();
        assert_eq!(t.len(), 28);
        assert!(t.contains(&('a', 5, 10)) && t.contains(&('g', 13, 15)) && t.contains(&('c', 0, 1)));
    }
}

/// Every lamp (segments and corners) with its on-runs and off-runs (debugging aid for the brief).
pub fn lamp_runs() -> Vec<(String, Vec<(u8, u8)>, Vec<(u8, u8)>)> {
    let mut v = Vec::new();
    let names = ["a", "b", "c", "d", "e", "f", "g", "af", "ab", "efg", "bcg", "de", "cd"];
    for n in names {
        let lit = |s: usize| n.chars().any(|ch| SEGMENTS[s].contains(ch));
        v.push((n.to_string(), runs(lit), runs(|s| !lit(s))));
    }
    v
}

#[cfg(test)]
mod print {
    /// Prime implicants (value, mask of don't-care bits) of a 4-bit function's set.
    fn primes(set: &[u8]) -> Vec<(u8, u8)> {
        let mut cur: Vec<(u8, u8)> = set.iter().map(|&m| (m, 0)).collect();
        let mut primes = Vec::new();
        while !cur.is_empty() {
            let mut next = Vec::new();
            let mut used = vec![false; cur.len()];
            for i in 0..cur.len() {
                for j in i + 1..cur.len() {
                    let (a, b) = (cur[i], cur[j]);
                    if a.1 == b.1 && (a.0 ^ b.0).count_ones() == 1 {
                        let n = (a.0 & b.0, a.1 | (a.0 ^ b.0));
                        if !next.contains(&n) {
                            next.push(n);
                        }
                        used[i] = true;
                        used[j] = true;
                    }
                }
            }
            for (i, c) in cur.iter().enumerate() {
                if !used[i] && !primes.contains(c) {
                    primes.push(*c);
                }
            }
            cur = next;
        }
        primes
    }

    /// The fewest primes covering `set` (exact, by subset size), and their literal count.
    fn cover(set: &[u8]) -> (usize, usize) {
        let p = primes(set);
        for k in 1..=p.len() {
            let mut best: Option<usize> = None;
            let mut idx: Vec<usize> = (0..k).collect();
            loop {
                let covers = set.iter().all(|&m| idx.iter().any(|&i| m & !p[i].1 == p[i].0));
                if covers {
                    let lits: usize = idx.iter().map(|&i| 4 - p[i].1.count_ones() as usize).sum();
                    best = Some(best.map_or(lits, |b: usize| b.min(lits)));
                }
                // next combination
                let mut i = k;
                loop {
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                    if idx[i] < p.len() - k + i {
                        idx[i] += 1;
                        for j in i + 1..k {
                            idx[j] = idx[j - 1] + 1;
                        }
                        break;
                    }
                    if i == 0 {
                        idx.clear();
                    }
                }
                if idx.len() != k || (k > 0 && idx[0] > p.len() - k) {
                    break;
                }
            }
            if let Some(l) = best {
                return (k, l);
            }
        }
        (0, 0)
    }

    #[test]
    fn sop_sizes() {
        let names = ["a", "b", "c", "d", "e", "f", "g", "af", "ab", "efg", "bcg", "de", "cd"];
        let (mut t_on, mut t_best) = (0, 0);
        for n in names {
            let lit = |s: usize| n.chars().any(|ch| circuit::compute::SEGMENTS[s].contains(ch));
            let on: Vec<u8> = (0..16u8).filter(|&s| lit(s as usize)).collect();
            let off: Vec<u8> = (0..16u8).filter(|&s| !lit(s as usize)).collect();
            let (a, b) = (cover(&on), cover(&off));
            t_on += a.0;
            t_best += a.0.min(b.0);
            println!("{n:4} ON {} terms {} literals | OFF {} terms {} literals", a.0, a.1, b.0, b.1);
        }
        println!("terms: all ON {t_on}, best polarity per lamp {t_best}");
    }

    #[test]
    fn print_lamp_runs() {
        let mut inner_on = 0;
        let mut inner_off = 0;
        for (n, on, off) in super::lamp_runs() {
            let inner = |r: &[(u8, u8)]| r.iter().filter(|(lo, hi)| *lo > 0 && *hi < 15).count();
            inner_on += inner(&on);
            inner_off += inner(&off);
            println!("{n:4} on {on:?} (inner {}) off {off:?} (inner {})", inner(&on), inner(&off));
        }
        println!("inner windows: on-form {inner_on}, off-form {inner_off}");
    }
}
