//! The analog decoder's code search (`mctas circuit display analog-orders`): the 4-bit value as
//! one signal strength s = 15 - sum(w_i b_i) (weights 8 4 2 1 in some order, bits possibly
//! complemented: 384 codes), each segment the OR of its runs of ON values along s. A run touching
//! s = 15 is one tap of s, a run touching s = 0 one tap of t = 15 - s, an inner run a window (a
//! subtract comparator and a repeater). Prints the codes with the fewest windows, then runs.

use circuit::compute::SEGMENTS;

/// A code: `w[i]` is the weight of input bit i, `neg` the bits read complemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Code {
    pub w: [u8; 4],
    pub neg: u8,
}

impl Code {
    /// The strength of value v.
    pub fn s(&self, v: u32) -> u8 {
        let sub: u32 = (0..4).map(|i| ((v >> i & 1) ^ (self.neg as u32 >> i & 1)) * self.w[i] as u32).sum();
        15 - sub as u8
    }
}

/// A run of ON strengths [lo, hi] of one segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub lo: u8,
    pub hi: u8,
}

/// Each segment's ON set as a 16-bit mask over s, for a code.
pub fn masks(c: &Code) -> [u16; 7] {
    let mut m = [0u16; 7];
    for v in 0..16u32 {
        let s = c.s(v);
        for (k, ch) in "abcdefg".chars().enumerate() {
            if SEGMENTS[v as usize].contains(ch) {
                m[k] |= 1 << s;
            }
        }
    }
    m
}

pub fn runs(mask: u16) -> Vec<Run> {
    let mut out = Vec::new();
    let mut s = 0u8;
    while s < 16 {
        if mask >> s & 1 == 1 {
            let lo = s;
            while s < 16 && mask >> s & 1 == 1 {
                s += 1;
            }
            out.push(Run { lo, hi: s - 1 });
        } else {
            s += 1;
        }
    }
    out
}

/// (inner windows, total runs) of a code.
pub fn cost(c: &Code) -> (usize, usize) {
    let mut inner = 0;
    let mut total = 0;
    for m in masks(c) {
        for r in runs(m) {
            total += 1;
            if r.lo > 0 && r.hi < 15 {
                inner += 1;
            }
        }
    }
    (inner, total)
}

pub fn all_codes() -> Vec<Code> {
    let mut out = Vec::new();
    let ws = [8u8, 4, 2, 1];
    for p in 0..24usize {
        // the p-th permutation of 4
        let mut pool: Vec<u8> = ws.to_vec();
        let mut k = p;
        let mut w = [0u8; 4];
        for (i, slot) in w.iter_mut().enumerate() {
            let f = [6, 2, 1, 1][i];
            *slot = pool.remove(k / f);
            k %= f;
        }
        for neg in 0..16u8 {
            out.push(Code { w, neg });
        }
    }
    out
}

pub fn fmt_runs(m: u16) -> String {
    runs(m).iter().map(|r| if r.lo == r.hi { format!("{}", r.lo) } else { format!("{}-{}", r.lo, r.hi) }).collect::<Vec<_>>().join(",")
}

pub fn orders(top: usize) {
    let mut v: Vec<(Code, (usize, usize))> = all_codes().into_iter().map(|c| (c, cost(&c))).collect();
    v.sort_by_key(|(_, k)| *k);
    for (c, (inner, total)) in v.iter().take(top) {
        let m = masks(c);
        let segs: Vec<String> = "abcdefg".chars().zip(m).map(|(ch, m)| format!("{ch}[{}]", fmt_runs(m))).collect();
        println!("w a0..a3 = {:?} neg {:04b}: {inner} windows, {total} runs  {}", c.w, c.neg, segs.join(" "));
    }
}

/// A gated term: the values with s in [lo, hi] and none of the kill literals true (a literal is
/// (bit, polarity): polarity 1 kills when the bit is on, 0 when it is off). One comparator whose
/// rear is a tap and whose two sides are the kills (a repeater from a tap for a two-sided window,
/// the bit lines for literals).
#[derive(Clone, Debug)]
pub struct Term {
    pub lo: u8,
    pub hi: u8,
    pub kills: Vec<(u8, u8)>,
    pub mask: u16,
    /// Components: 1 tap (one-sided, no kills), 1 comparator (one-sided with kills), 2
    /// (two-sided: comparator + repeater).
    pub cost: u8,
}

pub fn terms(c: &Code, on: u16) -> Vec<Term> {
    let mut out = Vec::new();
    let lits: Vec<(u8, u8)> = (0..4).flat_map(|b| [(b, 1), (b, 0)]).collect();
    let mut kill_sets: Vec<Vec<(u8, u8)>> = vec![vec![]];
    for (i, a) in lits.iter().enumerate() {
        kill_sets.push(vec![*a]);
        for b in &lits[i + 1..] {
            if a.0 != b.0 {
                kill_sets.push(vec![*a, *b]);
            }
        }
    }
    for lo in 0..16u8 {
        for hi in lo..16u8 {
            let two = lo > 0 && hi < 15;
            for k in &kill_sets {
                // a two-sided window takes one side; one literal is left
                if two && k.len() > 1 {
                    continue;
                }
                let mut mask = 0u16;
                for v in 0..16u32 {
                    let s = c.s(v);
                    if s < lo || s > hi {
                        continue;
                    }
                    if k.iter().any(|&(b, p)| (v >> b & 1) as u8 == p) {
                        continue;
                    }
                    mask |= 1 << s;
                }
                if mask == 0 || mask & !on != 0 {
                    continue;
                }
                let cost = if two { 2 } else { 1 };
                out.push(Term { lo, hi, kills: k.clone(), mask, cost });
            }
        }
    }
    out
}

/// The cheapest cover of `on` by terms: (terms, cost) minimizing terms, then cost (depth <= 6).
pub fn cover(c: &Code, on: u16) -> Option<(Vec<Term>, u32)> {
    let ts = terms(c, on);
    // keep only maximal masks per cost
    for n in 1..=6usize {
        let mut best: Option<(Vec<usize>, u32)> = None;
        let mut pick = Vec::new();
        fn rec(ts: &[Term], on: u16, n: usize, start: usize, acc: u16, pick: &mut Vec<usize>, best: &mut Option<(Vec<usize>, u32)>) {
            if pick.len() == n {
                if acc == on {
                    let cost: u32 = pick.iter().map(|&i| ts[i].cost as u32).sum();
                    if best.as_ref().is_none_or(|b| cost < b.1) {
                        *best = Some((pick.clone(), cost));
                    }
                }
                return;
            }
            // the lowest uncovered value must be covered by the next term or a later one: pick
            // terms covering it only
            let low = (on & !acc).trailing_zeros();
            for i in start..ts.len() {
                if ts[i].mask >> low & 1 == 0 {
                    continue;
                }
                pick.push(i);
                rec(ts, on, n, 0, acc | ts[i].mask, pick, best);
                pick.pop();
            }
        }
        rec(&ts, on, n, 0, 0, &mut pick, &mut best);
        if let Some((p, cost)) = best {
            return Some((p.into_iter().map(|i| ts[i].clone()).collect(), cost));
        }
    }
    None
}

pub fn gated(top: usize) {
    let mut v = Vec::new();
    for c in all_codes() {
        let mut n = 0;
        let mut cost = 0;
        let mut desc = Vec::new();
        for (ch, m) in "abcdefg".chars().zip(masks(&c)) {
            let (t, k) = cover(&c, m).expect("cover");
            n += t.len();
            cost += k;
            let ds: Vec<String> = t
                .iter()
                .map(|t| {
                    let ks: String = t.kills.iter().map(|&(b, p)| format!("{}a{b}", if p == 1 { "" } else { "!" })).collect::<Vec<_>>().join("");
                    format!("{}-{}{}", t.lo, t.hi, if ks.is_empty() { String::new() } else { format!("/{ks}") })
                })
                .collect();
            desc.push(format!("{ch}[{}]", ds.join(",")));
        }
        v.push((n, cost, c, desc));
    }
    v.sort_by_key(|x| (x.0, x.1));
    for (n, cost, c, desc) in v.iter().take(top) {
        println!("w {:?} neg {:04b}: {n} terms, cost {cost}  {}", c.w, c.neg, desc.join(" "));
    }
}

/// Ring corners: 0 NW (s), 1 NE (t), 2 SE (s), 3 SW (t). A comparator's (s corner, t corner) per
/// position E, N, W, S.
pub const RING: [(usize, usize); 4] = [(2, 1), (0, 1), (0, 3), (2, 3)];
pub const ABSENT: u8 = 99;

/// A ring comparator's rear constant for run r with corner distances (ds, dt), or None.
pub fn ring_k(r: Run, ds: u8, dt: u8) -> Option<u8> {
    let inner = r.lo > 0 && r.hi < 15;
    if inner {
        if ds == ABSENT || dt == ABSENT || ds as i32 - dt as i32 != r.lo as i32 + r.hi as i32 - 15 {
            return None;
        }
        let k = r.hi as i32 + 1 - ds as i32;
        return (1..=15).contains(&k).then_some(k as u8);
    }
    if r.lo == 0 {
        // on for s <= hi: k = hi + 1 - ds; t never kills: 15 - dt < k
        if ds == ABSENT {
            return None;
        }
        let k = r.hi as i32 + 1 - ds as i32;
        if !(1..=15).contains(&k) {
            return None;
        }
        if dt != ABSENT && 15 - dt as i32 >= k {
            return None;
        }
        return Some(k as u8);
    }
    // top run: k = 16 - lo - dt; s never kills: 15 - ds < k
    if dt == ABSENT {
        return None;
    }
    let k = 16 - r.lo as i32 - dt as i32;
    if !(1..=15).contains(&k) {
        return None;
    }
    if ds != ABSENT && 15 - ds as i32 >= k {
        return None;
    }
    Some(k as u8)
}

/// The best ring of one segment's runs: (corner distances, position of each run, rear k of each
/// run), minimizing the largest corner distance, then their sum. Distances 0..=dmax or ABSENT.
pub fn best_ring(runs: &[Run], dmax: u8) -> Option<([u8; 4], Vec<usize>, Vec<u8>)> {
    if runs.len() > 4 {
        return None;
    }
    let vals: Vec<u8> = (0..=dmax).chain([ABSENT]).collect();
    let mut best: Option<((u32, u32), [u8; 4], Vec<usize>, Vec<u8>)> = None;
    let perms: Vec<Vec<usize>> = {
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
        rec(runs.len(), &mut Vec::new(), &mut out);
        out
    };
    for &a in &vals {
        for &b in &vals {
            for &c in &vals {
                for &d in &vals {
                    let ds = [a, b, c, d];
                    let score = {
                        let present: Vec<u32> = ds.iter().filter(|&&x| x != ABSENT).map(|&x| x as u32).collect();
                        (present.iter().copied().max().unwrap_or(0), present.iter().sum::<u32>())
                    };
                    if best.as_ref().is_some_and(|b| b.0 <= score) {
                        continue;
                    }
                    'perm: for p in &perms {
                        let mut ks = Vec::new();
                        for (r, &pos) in runs.iter().zip(p) {
                            let (sc, tc) = RING[pos];
                            match ring_k(*r, ds[sc], ds[tc]) {
                                Some(k) => ks.push(k),
                                None => continue 'perm,
                            }
                        }
                        best = Some((score, ds, p.clone(), ks));
                        break;
                    }
                }
            }
        }
    }
    best.map(|(_, d, p, k)| (d, p, k))
}

pub fn rings(top: usize, dmax: u8) {
    let mut v: Vec<(Code, (usize, usize))> = all_codes().into_iter().map(|c| (c, cost(&c))).collect();
    v.retain(|(c, _)| masks(c).iter().all(|m| runs(*m).len() <= 4));
    v.sort_by_key(|(_, k)| *k);
    for (c, (inner, total)) in v.iter().take(top) {
        let mut line = format!("w {:?} neg {:04b}: {inner} windows {total} runs:", c.w, c.neg);
        let mut worst = 0;
        let mut ok = true;
        for (ch, m) in "abcdefg".chars().zip(masks(c)) {
            let rs = runs(m);
            match best_ring(&rs, dmax) {
                Some((d, p, k)) => {
                    let ds: Vec<String> = d.iter().map(|&x| if x == ABSENT { "-".into() } else { x.to_string() }).collect();
                    let w: Vec<String> = rs.iter().zip(&p).zip(&k).map(|((r, &pos), k)| format!("{}{}-{}k{k}", ["E", "N", "W", "S"][pos], r.lo, r.hi)).collect();
                    worst = worst.max(d.iter().filter(|&&x| x != ABSENT).copied().max().unwrap_or(0));
                    line.push_str(&format!(" {ch}[{} | {}]", ds.join(","), w.join(" ")));
                }
                None => {
                    ok = false;
                    line.push_str(&format!(" {ch}[none]"));
                }
            }
        }
        println!("{} max d {worst}: {line}", if ok { "OK" } else { "--" });
    }
}

/// The direct ring's corner configurations: each corner (NE, SE, SW, NW) is dust (s, read by both
/// window comparators beside it), an offset comparator facing one of them, or empty. Returns the
/// achievable (inner, top, bottom) counts.
pub fn ring_patterns() -> Vec<[u8; 3]> {
    // slots E N W S; corner c touches two slots
    let touch: [[usize; 2]; 4] = [[0, 1], [0, 3], [2, 3], [2, 1]]; // NE: E,N; SE: E,S; SW: W,S; NW: W,N
    let mut out = std::collections::BTreeSet::new();
    for cfg in 0..256u32 {
        let mut o = [false; 4];
        let mut d = [false; 4];
        for (c, t) in touch.iter().enumerate() {
            match cfg >> (2 * c) & 3 {
                0 => {}
                1 => {
                    d[t[0]] = true;
                    d[t[1]] = true;
                }
                2 => o[t[0]] = true,
                _ => o[t[1]] = true,
            }
        }
        let mut n = [0u8; 3];
        for s in 0..4 {
            match (o[s], d[s]) {
                (true, true) => n[0] += 1,
                (true, false) => n[1] += 1,
                (false, true) => n[2] += 1,
                _ => {}
            }
        }
        out.insert(n);
    }
    out.into_iter().collect()
}

/// Segments that fit one direct ring per code (inner/top/bottom counts within a pattern; a slot may
/// go unused, an inner slot may not serve an end run).
pub fn direct_fit(top: usize) {
    let pats = ring_patterns();
    println!("patterns {pats:?}");
    let fits = |n: [u8; 3]| pats.iter().any(|p| p[0] >= n[0] && p[1] >= n[1] && p[2] >= n[2]);
    let mut v = Vec::new();
    for c in all_codes() {
        let mut ok = 0;
        let mut desc = Vec::new();
        for m in masks(&c) {
            let mut n = [0u8; 3];
            for r in runs(m) {
                if r.lo > 0 && r.hi < 15 {
                    n[0] += 1
                } else if r.hi == 15 {
                    n[1] += 1
                } else {
                    n[2] += 1
                }
            }
            if fits(n) {
                ok += 1;
            }
            desc.push(format!("{}{}{}", n[0], n[1], n[2]));
        }
        v.push((ok, cost(&c), c, desc));
    }
    v.sort_by_key(|x| (std::cmp::Reverse(x.0), x.1));
    for (ok, (inner, total), c, desc) in v.iter().take(top) {
        println!("{ok}/7 fit: w {:?} neg {:04b} {inner} windows {total} runs  itb {}", c.w, c.neg, desc.join(" "));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn print_best() {
        for (w, neg) in [([4u8, 1, 8, 2], 0u8), ([4, 1, 8, 2], 15), ([8, 1, 4, 2], 1), ([8, 1, 4, 2], 14), ([1, 4, 2, 8], 5)] {
            let c = Code { w, neg };
            let m = masks(&c);
            let segs: Vec<String> = "abcdefg".chars().zip(m).map(|(ch, m)| format!("{ch}[{}]", fmt_runs(m))).collect();
            println!("RUNS {w:?} {neg:04b} {:?}: {}", cost(&c), segs.join(" "));
        }
    }

    #[test]
    fn plain_value_runs() {
        // s = 15 - v reversed: runs are the same as along v: a5 b3 c3 d5 e5 f4 g3
        let c = Code { w: [1, 2, 4, 8], neg: 0b1111 };
        for v in 0..16 {
            assert_eq!(c.s(v), v as u8);
        }
        let n: Vec<usize> = masks(&c).iter().map(|m| runs(*m).len()).collect();
        assert_eq!(n, vec![5, 3, 3, 5, 5, 4, 3]);
        assert_eq!(all_codes().len(), 384);
        let s: std::collections::HashSet<_> = (0..16).map(|v| c.s(v)).collect();
        assert_eq!(s.len(), 16);
    }
}

#[cfg(test)]
mod columns {
    use super::*;

    /// Whether run r fits a slot whose column carries s at distance f (O contact f, W contact f + 1).
    fn fits(r: Run, f: i32) -> bool {
        let (lo, hi) = (r.lo as i32, r.hi as i32);
        if lo == 0 {
            hi >= f + 1
        } else if hi == 15 {
            f < lo
        } else {
            f < lo && hi >= f + 1 && lo + hi - 2 * f - 1 <= 15
        }
    }

    #[test]
    #[ignore]
    fn column_feasibility() {
        for c in all_codes() {
            let segs: Vec<Vec<Run>> = masks(&c).iter().map(|m| runs(*m)).collect();
            if segs.iter().any(|r| r.len() > 4) {
                continue;
            }
            let mut best: Option<[i32; 4]> = None;
            for fe in 0..8 {
                for fnn in 0..8 {
                    for fw in 0..8 {
                        for fs in 0..8 {
                            let f = [fe, fnn, fw, fs];
                            let ok = segs.iter().all(|rs| {
                                // some injective map runs -> axes
                                let n = rs.len();
                                (0..256u32).any(|m| {
                                    let a: Vec<usize> = (0..n).map(|i| (m >> (2 * i) & 3) as usize).collect();
                                    let mut seen = [false; 4];
                                    a.iter().all(|&x| !std::mem::replace(&mut seen[x], true)) && rs.iter().zip(&a).all(|(r, &x)| fits(*r, f[x]))
                                })
                            });
                            if ok && best.is_none() {
                                best = Some(f);
                            }
                        }
                    }
                }
            }
            if let Some(f) = best {
                println!("COLS w {:?} neg {:04b} {:?}: f {f:?}", c.w, c.neg, cost(&c));
            }
        }
    }
}
