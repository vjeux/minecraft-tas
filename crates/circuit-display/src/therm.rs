//! The thermometer decoder (`mctas circuit display therm ...`): the value as one strength s (the
//! analog chain), then per segment a STRIP: a dust ruler from s (cell j carries s - j), and per run
//! [lo, hi] of the segment's ON strengths one subtract comparator C whose rear is a repeater tap
//! T_lo (on iff s >= lo: the ruler cell lo-1) and whose side is a tap T_hi+1 (the ruler cell hi).
//! Every tap reads the same dust network, so every tap of a strip switches in the same tick and C
//! never sees its two sides apart: no glitch from the window itself. Taps are delay-2 repeaters, or
//! a delay-1 tap plus a delay-1 repeater into C's side (4 gt either way); a run touching s = 0 takes
//! a redstone block as C's rear, a run touching 15 has no side. Runs alternate between the two sides
//! of the ruler; each side's comparators power the blocks of an OR row with dust on top; each row is
//! taken by one repeater in its middle and both come back, as dust of equal length, into one block X
//! that the strip's output repeater reads.
//!
//! Strip frame (u along the ruler, h up, v across): ruler at h 0, v 0, cells u = 0..=14; taps at
//! v = +-1, C at +-2, OR blocks at +-3 (dust on top at h 1), merge repeater at (u_m, 1, +-4), return
//! dust at v = +-5 and u = -3, X at (-3, 1, 0), the output repeater at (-4, 1, 0) pointing -u. The
//! ruler's first cell stands on a block B (0, -1, 0) powered by the copy comparator Q (-1, -1, 0)
//! that reads the strip's hub (-2, -1, 0).

use crate::abuild::{B, D, P, add};
use crate::analog::{Code, Run, all_codes, masks, runs};
use anyhow::{Result, anyhow, ensure};

/// A strip placement: global = o + (su * u, h, v), or along z when `z_axis` (u along z, v along x).
#[derive(Clone, Copy)]
pub struct T {
    pub o: P,
    pub su: i32,
    pub z_axis: bool,
}

impl T {
    pub fn p(&self, u: i32, h: i32, v: i32) -> P {
        if self.z_axis { add(self.o, [v, h, self.su * u]) } else { add(self.o, [self.su * u, h, v]) }
    }
    /// The world direction of a strip direction (written in the x-axis frame: E = +u, W = -u,
    /// S = +v, N = -v).
    pub fn d(&self, d: D) -> D {
        let (along, pos) = match d {
            D::E => (true, true),
            D::W => (true, false),
            D::S => (false, true),
            D::N => (false, false),
        };
        let pos = if along { pos == (self.su > 0) } else { pos };
        match (self.z_axis, along, pos) {
            (false, true, true) => D::E,
            (false, true, false) => D::W,
            (false, false, true) => D::S,
            (false, false, false) => D::N,
            (true, true, true) => D::S,
            (true, true, false) => D::N,
            (true, false, true) => D::E,
            (true, false, false) => D::W,
        }
    }
}

/// Thermometer cost of a code: (runs, low edges).
pub fn cost(c: &Code) -> (usize, usize) {
    let mut n = 0;
    let mut low = 0;
    for m in masks(c) {
        for r in runs(m) {
            n += 1;
            if r.lo == 0 {
                low += 1;
            }
        }
    }
    (n, low)
}

pub fn orders(neg: Option<u8>, top: usize) {
    // per segment: the fewest runs over all codes, and how many codes reach it
    let codes = all_codes();
    for (k, ch) in "abcdefg".chars().enumerate() {
        let best = codes.iter().map(|c| runs(masks(c)[k]).len()).min().unwrap();
        let n = codes.iter().filter(|c| runs(masks(c)[k]).len() == best).count();
        let ex = codes.iter().find(|c| runs(masks(c)[k]).len() == best).unwrap();
        println!("segment {ch}: best {best} runs ({n} codes, e.g. w {:?} neg {:04b}: {})", ex.w, ex.neg, crate::analog::fmt_runs(masks(ex)[k]));
    }
    // the best pairs of codes (each segment takes the better code)
    let mut best: Vec<(usize, usize, usize)> = Vec::new();
    for i in 0..codes.len() {
        for j in i + 1..codes.len() {
            let (mi, mj) = (masks(&codes[i]), masks(&codes[j]));
            let tot: usize = (0..7).map(|k| runs(mi[k]).len().min(runs(mj[k]).len())).sum();
            best.push((tot, i, j));
        }
    }
    best.sort();
    for (tot, i, j) in best.iter().take(5) {
        println!("pair: {tot} runs: w {:?} neg {:04b} + w {:?} neg {:04b}", codes[*i].w, codes[*i].neg, codes[*j].w, codes[*j].neg);
    }
    // the codes the chains build (neg 0001: agen::chain4, neg 1111: astack::build_chain), by the
    // most runs of a segment (a ring holds 3 when one arm is the output)
    let mut v: Vec<(usize, usize, Code)> = codes.iter().filter(|c| c.neg == 1 || c.neg == 15).map(|c| {
        let r: Vec<usize> = masks(c).iter().map(|m| runs(*m).len()).collect();
        (*r.iter().max().unwrap(), r.iter().sum(), *c)
    }).collect();
    v.sort_by_key(|x| (x.0, x.1));
    // shared rulers: all runs of a segment on ONE side of the ruler. A run's side feeder ends at
    // u = hi; the next run's comparator sits at u = lo' - 1, so a gap of exactly one off-value
    // puts that feeder on the comparator's other side. A multi-cell run ends that feeder in a
    // BLOCK (a comparator's side ignores blocks), a single-cell run cannot: its one cell must
    // stay dust. Codes with no single-cell run followed by a gap-1 run:
    let mut ok: Vec<(usize, Code)> = Vec::new();
    for c in codes.iter().filter(|c| c.neg == 1 || c.neg == 15) {
        let mut good = true;
        let mut tot = 0;
        for mk in masks(c) {
            let rs = runs(mk);
            tot += rs.len();
            for w in rs.windows(2) {
                if w[0].lo == w[0].hi && w[1].lo == w[0].hi + 2 {
                    good = false;
                }
            }
        }
        if good {
            ok.push((tot, *c));
        }
    }
    ok.sort_by_key(|x| x.0);
    for (tot, c) in ok.iter().take(6) {
        println!("shared-ruler code w {:?} neg {:04b}: {tot} runs", c.w, c.neg);
    }
    // ring stacks: a and d may take 4 runs (a lamp-side P stone), every other segment 3
    for c in &codes {
        let r: Vec<usize> = masks(c).iter().map(|m| runs(*m).len()).collect();
        let ok = (0..7).all(|k| r[k] <= if k == 0 || k == 3 { 4 } else { 3 });
        if ok {
            println!("ring code w {:?} neg {:04b}: per segment {r:?}", c.w, c.neg);
        }
    }
    for (mx, tot, c) in v.iter().take(6) {
        let r: Vec<usize> = masks(c).iter().map(|m| runs(*m).len()).collect();
        println!("chain code w {:?} neg {:04b}: max {mx}, total {tot}, per segment {r:?}", c.w, c.neg);
    }
    // strip length per buildable code: the farthest ruler cell or comparator over all segments
    let mut lens: Vec<(i32, usize, Code)> = codes
        .iter()
        .filter(|c| c.neg == 15)
        .map(|c| {
            let mut far = 0i32;
            let mut n = 0;
            for mk in masks(c) {
                for r in runs(mk) {
                    n += 1;
                    let cell = if r.hi == 15 { r.lo as i32 - 1 } else { r.hi as i32 };
                    far = far.max(cell).max(r.lo as i32 - 1);
                }
            }
            (far, n, *c)
        })
        .collect();
    lens.sort_by_key(|x| (x.0, x.1));
    for (far, n, c) in lens.iter().take(6) {
        println!("strip-length code w {:?} neg {:04b}: farthest cell {far}, {n} runs", c.w, c.neg);
    }
    // shared rulers per buildable code: the segments that cannot put all their runs on one side
    // (a single-cell run followed by a gap-1 run); rulers = those + the others paired
    let mut sh: Vec<(usize, String, Code)> = codes
        .iter()
        .filter(|c| c.neg == 15)
        .map(|c| {
            let bad: String = "abcdefg"
                .chars()
                .zip(masks(c))
                .filter(|(_, mk)| runs(*mk).windows(2).any(|w| w[0].lo == w[0].hi && w[1].lo == w[0].hi + 2))
                .map(|(ch, _)| ch)
                .collect();
            (bad.len() + (7 - bad.len()).div_ceil(2), bad, *c)
        })
        .collect();
    sh.sort_by_key(|x| x.0);
    for (n, bad, c) in sh.iter().take(8) {
        println!("shared-ruler code w {:?} neg {:04b}: {n} rulers (alone: {bad})", c.w, c.neg);
    }
    let mut v: Vec<(Code, (usize, usize))> = all_codes().into_iter().filter(|c| neg.is_none_or(|n| c.neg == n)).map(|c| (c, cost(&c))).collect();
    v.sort_by_key(|(_, k)| *k);
    for (c, (n, low)) in v.iter().take(top) {
        let m = masks(c);
        let segs: Vec<String> = "abcdefg".chars().zip(m).map(|(ch, m)| format!("{ch}[{}]", crate::analog::fmt_runs(m))).collect();
        println!("w {:?} neg {:04b}: {n} runs, {low} low edges  {}", c.w, c.neg, segs.join(" "));
    }
}

/// Where the strip's OR rows are merged (u of the merge repeater on each side).
fn merge_u(cs: &[i32]) -> i32 {
    let (lo, hi) = (*cs.iter().min().unwrap(), *cs.iter().max().unwrap());
    ((lo + hi) / 2).min(7)
}

thread_local! {
    /// The narrow strip: no return rows (the OR rows reach the head themselves), v in -3..=3.
    pub static NARROW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// Narrow strips with each OR row's repeater at the head (the tower).
    pub static HEAD_REP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// One segment's strip. Returns the global cell the output repeater points into.
pub fn build_strip(b: &mut B, t: &T, rs: &[Run]) -> P {
    let sided: Vec<(Run, usize)> = rs.iter().enumerate().map(|(k, r)| (*r, k % 2)).collect();
    build_strip_sided(b, t, &sided, false)[0]
}

/// A strip whose runs sit on the given sides (0: +v, 1: -v). `split`: a SHARED ruler, each side
/// one segment with its own head (entry block (-3, 1, +-1), output repeater (-4, 1, +-1)); a
/// multi-cell run's feeder then ends in a block (a gap-1 run's comparator on the same side would
/// read a dust there). Returns the cells the output repeaters point into ([+v, -v] when split).
pub fn build_strip_sided(b: &mut B, t: &T, rs: &[(Run, usize)], split: bool) -> Vec<P> {
    let st = |b: &mut B, u: i32, h: i32, v: i32| {
        let q = t.p(u, h, v);
        if !b.blocks.contains_key(&q) {
            b.stone(q);
        }
    };
    let max_idx = rs.iter().map(|(r, _)| if r.hi == 15 { r.lo as i32 - 1 } else { r.hi as i32 }).max().unwrap_or(0).max(0);
    // ruler
    for u in 0..=max_idx {
        b.dust(t.p(u, 0, 0));
    }
    // B under the first cell, Q, hub
    b.stone(t.p(0, -1, 0));
    b.comp(t.p(-1, -1, 0), t.d(D::E), false);
    b.stone(t.p(-2, -1, 0));
    let mut cs: [Vec<i32>; 2] = [Vec::new(), Vec::new()];
    for &(r, side) in rs {
        let sg = if side == 0 { 1 } else { -1 };
        let out = if sg > 0 { D::S } else { D::N };
        let (lo, hi) = (r.lo as i32, r.hi as i32);
        let i = lo - 1;
        // C
        b.comp(t.p(i, 0, 2 * sg), t.d(out), true);
        cs[side].push(i);
        // rear
        if lo == 0 {
            b.set(t.p(i, 0, sg), "redstone_block");
        } else {
            b.rep(t.p(i, 0, sg), t.d(out), 2);
        }
        // side
        if hi < 15 {
            if hi == lo {
                b.rep(t.p(hi, 0, sg), t.d(out), 2);
                b.dust(t.p(hi, 0, 2 * sg));
                st(b, hi, 1, 2 * sg);
            } else {
                b.rep(t.p(hi, 0, sg), t.d(out), 1);
                for u in lo + 1..=hi {
                    if split && u == hi {
                        b.stone(t.p(u, 0, 2 * sg));
                    } else {
                        b.dust(t.p(u, 0, 2 * sg));
                        st(b, u, 1, 2 * sg);
                    }
                }
                b.rep(t.p(lo, 0, 2 * sg), t.d(D::W), 1);
            }
        }
    }
    // OR rows: a block under each comparator's output, dust on top along u, one repeater in the
    // row (the farthest comparator is within 15 of it, the row's end within 15 of the repeater),
    // then an L at the head joining both sides on the lane-entry block
    for side in 0..2 {
        if cs[side].is_empty() {
            continue;
        }
        let sg = if side == 0 { 1 } else { -1 };
        let out = if sg > 0 { D::S } else { D::N };
        let (lo, hi) = (*cs[side].iter().min().unwrap(), *cs[side].iter().max().unwrap());
        if NARROW.with(|n| n.get()) {
            // the repeater sits at a u with no comparator, as far from the head as 15 allows;
            // HEAD_REP: right at the head (u = -2) whenever the farthest comparator reaches it, so
            // every window passes exactly one repeater (no 2 gt handover dip between windows on
            // either side of a mid-row repeater)
            let um = if HEAD_REP.with(|h| h.get()) && hi - (lo - 1) <= 15 {
                (lo - 1).max(-2)
            } else {
                (4..=hi).rev().find(|u| !cs[side].contains(u)).unwrap_or(hi + 1).min(10)
            };
            for u in lo.min(-3)..=hi.max(um) {
                st(b, u, 0, 3 * sg);
                if u == um {
                    b.rep(t.p(u, 1, 3 * sg), t.d(D::W), 1);
                } else {
                    b.dust(t.p(u, 1, 3 * sg));
                }
            }
            let ks: &[i32] = if split { &[2] } else { &[1, 2] };
            for &k in ks {
                b.dust(t.p(-3, 1, k * sg));
                st(b, -3, 0, k * sg);
            }
            continue;
        }
        let um = merge_u(&cs[side]);
        for u in lo.min(um)..=hi.max(um) {
            st(b, u, 0, 3 * sg);
            b.dust(t.p(u, 1, 3 * sg));
        }
        st(b, um, 0, 4 * sg);
        b.rep(t.p(um, 1, 4 * sg), t.d(out), 1);
        for u in -3..=um {
            b.dust(t.p(u, 1, 5 * sg));
        }
        for k in 1..5 {
            b.dust(t.p(-3, 1, k * sg));
        }
    }
    if split {
        let mut out = Vec::new();
        for sg in [1, -1] {
            b.stone(t.p(-3, 1, sg));
            b.rep(t.p(-4, 1, sg), t.d(D::W), 1);
            out.push(t.p(-5, 1, sg));
        }
        return out;
    }
    b.stone(t.p(-3, 1, 0));
    b.rep(t.p(-4, 1, 0), t.d(D::W), 1);
    vec![t.p(-5, 1, 0)]
}

/// A lossless line: `cells[0]` is a powered block; the following cells alternate comparator
/// (pointing along the path) and block. Horizontal steps only. Returns the last block.
pub fn lossless(b: &mut B, cells: &[P]) -> P {
    for k in 1..cells.len() {
        let d = [cells[k][0] - cells[k - 1][0], cells[k][2] - cells[k - 1][2]];
        let dir = match d {
            [1, 0] => D::E,
            [-1, 0] => D::W,
            [0, 1] => D::S,
            [0, -1] => D::N,
            _ => panic!("lossless: not a horizontal step {:?} -> {:?}", cells[k - 1], cells[k]),
        };
        if k % 2 == 1 {
            b.comp(cells[k], dir, false);
        } else {
            b.stone(cells[k]);
        }
    }
    *cells.last().unwrap()
}

pub fn parse_code(w: &str, neg: u8) -> Result<Code> {
    let w: Vec<u8> = w.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    ensure!(w.len() == 4, "--w takes 4 weights");
    Ok(Code { w: [w[0], w[1], w[2], w[3]], neg })
}

/// One strip after the chain, for one segment (a redsim test of the strip).
pub fn test(seg: char, code: &Code, out: &std::path::Path) -> Result<()> {
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let si = "abcdefg".find(seg).ok_or_else(|| anyhow::anyhow!("segment"))?;
    let rs = runs(masks(code)[si]);
    let mut b = B::default();
    // chain hub at (5,1,0) relative to o: put it at (-4,-1,0)
    crate::astack::build_chain(&mut b, [-9, -2, 0], code);
    b.blocks.remove(&[-4, 0, 0]);
    b.needs.retain(|p| *p != [-4, 0, 0]);
    b.comp([-3, -1, 0], D::E, false);
    let t = T { o: [0, 0, 0], su: 1, z_axis: false };
    let o = build_strip(&mut b, &t, &rs);
    // the test's face: the plane z = -7, facing north; the lane turns north at (-5, 1, 0)
    b.stone(o);
    for z in -6..=-1 {
        b.rep([o[0], o[1], z], D::N, 1);
    }
    let o = [o[0], o[1], -7];
    let face = circuit::compute::FACE;
    let (_, c0, r0) = face.iter().find(|(n, _, _)| n.starts_with(seg)).copied().unwrap();
    for (n, c, r) in face {
        b.port(&format!("output.{n}"), [o[0] + c0 as i32 - c as i32, o[1] + r0 as i32 - r as i32, o[2]]);
    }
    let d = b.design(&format!("therm-test-{seg}"), Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("runs {rs:?}: {} blocks", d.blocks.len());
    Ok(())
}

/// The analog chain (astack::build_chain at `o`) with every input brought to the back row
/// (z = o.z + 2, driven from the south) and s taken up from the hub: returns the block (o + (9, 2, 0))
/// that the chain's copy comparator powers at s.
pub fn chain_back(b: &mut B, o: P, code: &Code) -> P {
    let p = |x: i32, y: i32, z: i32| add(o, [x, y, z]);
    crate::astack::build_chain(b, o, code);
    b.ports.clear();
    // a0: repeater into the dust at (0, 1, 1)
    b.rep(p(0, 1, 2), D::N, 1);
    b.port("input.a0", p(0, 1, 2));
    // a1: a block powering the dust at (0, 1, -1)
    b.stone(p(-1, 1, -1));
    for z in 0..=2 {
        b.rep(p(-1, 1, z), D::N, 1);
    }
    b.port("input.a1", p(-1, 1, 2));
    // a2: (5, 1, -1) <- (6, 1, -1) <- block (7, 1, -1) <- repeaters from the back
    b.rep(p(6, 1, -1), D::W, 1);
    b.stone(p(7, 1, -1));
    for z in 0..=2 {
        b.rep(p(7, 1, z), D::N, 1);
    }
    b.port("input.a2", p(7, 1, 2));
    // a3: (5, 1, 1) <- block (6, 1, 1) <- repeater from the back
    b.stone(p(6, 1, 1));
    b.rep(p(6, 1, 2), D::N, 1);
    b.port("input.a3", p(6, 1, 2));
    // s: the dust on the hub (5, 2, 0) -> comparator -> block -> comparator -> block
    b.comp(p(6, 2, 0), D::E, false);
    b.stone(p(7, 2, 0));
    b.comp(p(8, 2, 0), D::E, false);
    b.stone(p(9, 2, 0));
    p(9, 2, 0)
}

/// Walk a lossless line from the powered block `p`: moves `N`/`S`/`E`/`W` (comparator, block),
/// `uN` (dust on the block, comparator one level up, block), `dN` (dust under the block, comparator
/// one level down, block). Returns the last block.
pub fn walk(b: &mut B, mut p: P, moves: &str) -> P {
    for m in moves.split_whitespace() {
        let (dy, dc) = match m.len() {
            1 => (0, m.chars().next().unwrap()),
            _ => (if m.starts_with('u') { 1 } else { -1 }, m.chars().nth(1).unwrap()),
        };
        let d = match dc {
            'N' => D::N,
            'S' => D::S,
            'E' => D::E,
            _ => D::W,
        };
        let v = d.v();
        let mut put = |b: &mut B, q: P, what: u8| {
            if b.blocks.contains_key(&q) {
                panic!("walk: {q:?} is already {} (move {m})", b.blocks[&q]);
            }
            match what {
                0 => b.dust(q),
                1 => b.stone(q),
                _ => {}
            }
        };
        if dy != 0 {
            put(b, add(p, [0, dy, 0]), 0);
            if dy > 0 {
                put(b, add(p, v), 1);
            }
        }
        let c = add(add(p, v), [0, dy, 0]);
        if b.blocks.contains_key(&c) {
            panic!("walk: comparator cell {c:?} is already {}", b.blocks[&c]);
        }
        b.comp(c, d, false);
        p = add(c, v);
        if !b.blocks.get(&p).is_some_and(|s| s == "minecraft:stone") {
            put(b, p, 1);
        }
    }
    p
}

/// The whole thermometer decoder: chain at the back, two lossless spines, seven strips in four
/// depth bands beside the lattice of lanes behind the face (lamps at z = 0, the design at z >= 1).
pub fn full(code: &Code, out: &std::path::Path) -> Result<()> {
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    let mut b = B::default();
    let split = chain_back(&mut b, [-12, -2, 78], code);
    // -x spine: d (z 66, y -2), g (46, 0), c (26, -1), b (6, 1); every height step at least 2
    // cells from a band (a step's dust would join a strip's return row)
    let band = "N N N N N N";
    let half = "N N N";
    walk(&mut b, split, &format!("N dN dN {band} N uN uN N {band} N dN N N {band} N uN uN N {half}"));
    // +x spine: a (46, 2), e (26, -1), f (6, 1)
    let e = walk(&mut b, split, "E E E E");
    walk(&mut b, e, &format!("uN uN N N N N N N N N N N N {band} N dN dN dN {band} N uN uN N {half}"));
    // segment, lamp column x, lamp y, band centre z, side (+1: strip at +x)
    let segs: [(char, i32, i32, i32, i32); 7] = [('f', 2, 3, 6, 1), ('b', 0, 3, 6, -1), ('e', 2, 1, 26, 1), ('c', 0, 1, 26, -1), ('a', 1, 4, 46, 1), ('g', 1, 2, 46, -1), ('d', 1, 0, 66, -1)];
    for (seg, x, y, zc, side) in segs {
        let si = "abcdefg".find(seg).unwrap();
        let t = T { o: [if side > 0 { 7 } else { -5 }, y - 1, zc], su: side, z_axis: false };
        let rs = runs(m[si]);
        let mut cell = build_strip(&mut b, &t, &rs);
        if x == 1 {
            b.rep(cell, if side > 0 { D::W } else { D::E }, 1);
            cell = add(cell, [-side, 0, 0]);
        }
        ensure!(cell == [x, y, zc], "lane entry {cell:?} for {seg}");
        b.stone(cell);
        for z in 1..zc {
            b.rep([x, y, z], D::N, 1);
        }
        b.port(&format!("output.{seg}"), [x, y, 0]);
    }
    let d = b.design("seven-segment-therm", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}

/// therm-v3's packing: the strips of one side all at one height (flat spines, bands 12 apart),
/// each strip shifted 3 out with a stair (rows -2..+1) before its lane entry.
pub fn full_flat(code: &Code, narrow: bool, out: &std::path::Path) -> Result<()> {
    NARROW.with(|n| n.set(narrow));
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    let mut b = B::default();
    // narrow strips are 7 wide (v -3..3): bands 8 apart instead of 12
    let (pitch, z0) = if narrow { (8, 5) } else { (12, 6) };
    let split = chain_back(&mut b, [-15, -2, z0 + 3 * pitch + 8], code);
    let band = (0..pitch / 2).map(|_| "N").collect::<Vec<_>>().join(" ");
    let half = (0..(pitch / 2 + 1) / 2 + 1).map(|_| "N").collect::<Vec<_>>().join(" ");
    walk(&mut b, split, &format!("N {band} {band} {band} {half}"));
    let e = walk(&mut b, split, "E E E E E E E");
    walk(&mut b, e, &format!("uN {band} {band} {band} {half}"));
    // segment, lamp column x, lamp y, band centre z, side (+1: strip at +x)
    let zs = [z0, z0 + pitch, z0 + 2 * pitch, z0 + 3 * pitch];
    let segs: [(char, i32, i32, i32, i32); 7] = [('f', 2, 3, zs[0], 1), ('b', 0, 3, zs[0], -1), ('e', 2, 1, zs[1], 1), ('c', 0, 1, zs[1], -1), ('a', 1, 4, zs[2], 1), ('g', 1, 2, zs[2], -1), ('d', 1, 0, zs[3], -1)];
    for (seg, x, y, zc, side) in segs {
        let si = "abcdefg".find(seg).unwrap();
        let oy = if side > 0 { 3 } else { 2 };
        let t = T { o: [if side > 0 { 10 } else { -8 }, oy - 1, zc], su: side, z_axis: false };
        let rs = runs(m[si]);
        build_strip(&mut b, &t, &rs);
        // the stair: u -5..-7, from h 1 to h 1 + dy; the entry at u -8
        let dy = y - oy;
        let h_end = 1 + dy;
        match dy {
            0 => {
                for u in -7..=-5 {
                    b.rep(t.p(u, 1, 0), t.d(D::W), 1);
                }
            }
            1 => {
                b.stone(t.p(-5, 1, 0));
                b.dust(t.p(-5, 2, 0));
                b.dust(t.p(-6, 2, 0));
                b.rep(t.p(-7, 2, 0), t.d(D::W), 1);
            }
            -1 => {
                b.dust(t.p(-5, 1, 0));
                b.dust(t.p(-6, 0, 0));
                b.rep(t.p(-7, 0, 0), t.d(D::W), 1);
            }
            -2 => {
                b.dust(t.p(-5, 1, 0));
                b.dust(t.p(-6, 0, 0));
                b.dust(t.p(-7, -1, 0));
            }
            _ => anyhow::bail!("stair {dy} for {seg}"),
        }
        let mut cell = t.p(-8, h_end, 0);
        if x == 1 {
            b.rep(cell, t.d(D::W), 1);
            cell = t.p(-9, h_end, 0);
        }
        ensure!(cell == [x, y, zc], "lane entry {cell:?} for {seg}");
        b.stone(cell);
        for z in 1..zc {
            b.rep([x, y, z], D::N, 1);
        }
        b.port(&format!("output.{seg}"), [x, y, 0]);
    }
    let d = b.design(if narrow { "seven-segment-therm-v4" } else { "seven-segment-therm-v3" }, Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}

/// therm-v5, the tower: the strips run along +z in two columns (x around 4 and 12) and are
/// stacked every 3 levels; s is carried up one lossless column between them (`dE dW` zigzag, 2
/// cells per level) with a branch to every strip's hub; each strip's output descends to its
/// lamp's row with dust steps (west and down) and runs west, then north into the lamp.
pub fn full_tower(code: &Code, o: &TowerOpts, out: &std::path::Path) -> Result<()> {
    // search: which segment takes which (column, level) slot, and the order the lanes are routed in
    let mut rng: u64 = 0x9e3779b97f4a7c15 ^ o.seed;
    let mut next = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        rng
    };
    let shuffle = |v: &mut [usize; 7], next: &mut dyn FnMut() -> u64| {
        for i in (1..7).rev() {
            let j = (next() % (i as u64 + 1)) as usize;
            v.swap(i, j);
        }
    };
    let mut best: Option<(i32, [usize; 7], [usize; 7])> = None;
    let mut fails = 0;
    for t in 0..o.tries {
        let mut slots = [0usize, 1, 2, 3, 4, 5, 6];
        if let Some(s) = &o.slots {
            slots = *s;
        } else {
            shuffle(&mut slots, &mut next);
        }
        let mut order = [0usize, 1, 2, 3, 4, 5, 6];
        shuffle(&mut order, &mut next);
        if let Some(fixed) = &o.order {
            order = *fixed;
        }
        let (b, lanes, lo, hi) = tower_base(code, &slots, o)?;
        let mut w = crate::route::world_from(&b, &lanes, lo, hi);
        match crate::route::route_all(&mut w, &lanes, &order, o.expand) {
            Ok(n) => {
                if best.as_ref().is_none_or(|x| n < x.0) {
                    println!("try {t}: slots {slots:?} order {order:?}: {n} lane blocks");
                    best = Some((n, slots, order));
                }
            }
            Err(_) => fails += 1,
        }
    }
    let (n, slots, order) = best.ok_or_else(|| anyhow!("no tower routes ({fails} tries failed)"))?;
    println!("best: slots {slots:?} order {order:?}: {n} lane blocks ({fails} of {} tries failed)", o.tries);
    let (mut b, lanes, lo, hi) = tower_base(code, &slots, o)?;
    let mut w = crate::route::world_from(&b, &lanes, lo, hi);
    crate::route::route_all(&mut w, &lanes, &order, o.expand)?;
    crate::route::write_lanes(&w, &mut b);
    for (k, (seg, _, _)) in LAMPS.iter().enumerate() {
        b.port(&format!("output.{seg}"), lanes[k].lamp);
    }
    let bad = crate::agen::bad_supports(&b).len();
    let d = b.design(&o.name, Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    let (mn, mx) = bounds(&d.blocks);
    println!("{}: {} blocks, {bad} unsupported, box {:?} .. {:?}", out.display(), d.blocks.len(), mn, mx);
    Ok(())
}

pub fn bounds(m: &std::collections::BTreeMap<P, String>) -> (P, P) {
    let mut mn = [i32::MAX; 3];
    let mut mx = [i32::MIN; 3];
    for p in m.keys() {
        for i in 0..3 {
            mn[i] = mn[i].min(p[i]);
            mx[i] = mx[i].max(p[i]);
        }
    }
    (mn, mx)
}

/// The tower's options (`display therm --tower`).
#[derive(Clone, Debug)]
pub struct TowerOpts {
    /// z of the strips' ruler start (the lanes get z 1 ..= oz - 5)
    pub oz: i32,
    pub tries: usize,
    pub expand: usize,
    pub seed: u64,
    pub slots: Option<[usize; 7]>,
    /// fixed routing order (lane indices in LAMPS order)
    pub order: Option<[usize; 7]>,
    /// extra routing columns beyond the base's x extent
    pub margin: i32,
    /// the s spiral's footprint depth beyond the hub row (z oz-4 ..= oz-4+spiral_depth)
    pub spiral_depth: i32,
    /// column B's strip x (column A at 3; the s line runs in the gap 7 ..= cxb - 4)
    pub cxb: i32,
    /// the design's name
    pub name: String,
}

/// The lamps in routing-lane order: segment, column x, row y (the face at z = 0, looking north).
pub const LAMPS: [(char, i32, i32); 7] = [('a', 1, 4), ('f', 2, 3), ('b', 0, 3), ('g', 1, 2), ('e', 2, 1), ('c', 0, 1), ('d', 1, 0)];

/// The tower without its lanes: the chain on top, the s column, the seven strips. Returns the
/// design, the lanes to route (in [`LAMPS`] order: each from its strip's output repeater to its
/// lamp) and the routing box.
#[allow(clippy::type_complexity)]
fn tower_base(code: &Code, slots: &[usize; 7], o: &TowerOpts) -> Result<(B, Vec<crate::route::Lane>, P, P)> {
    NARROW.with(|n| n.set(true));
    HEAD_REP.with(|n| n.set(true));
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    let mut b = B::default();
    let oz = o.oz;
    // the slots: (strip column x, strip level)
    let cb = o.cxb;
    let (xa, xb) = (3 + 4, cb - 4);
    let slot_at: [(i32, i32); 7] = [(3, 0), (3, 3), (3, 6), (3, 9), (cb, 0), (cb, 3), (cb, 6)];
    let segs: Vec<(char, i32, i32, i32, i32)> = (0..7)
        .map(|k| {
            let (seg, lx, row) = LAMPS[k];
            let (cx, lvl) = slot_at[slots[k]];
            (seg, lx, row, cx, lvl)
        })
        .collect();
    let mut lanes = Vec::new();
    for &(_, _, _, cx, lvl) in &segs {
        let si = "abcdefg".find(segs.iter().find(|s| s.3 == cx && s.4 == lvl).unwrap().0).unwrap();
        let rs = runs(m[si]);
        build_strip(&mut b, &T { o: [cx, lvl, oz], su: 1, z_axis: true }, &rs);
    }
    b.supports();
    // the s column: a lossless line in the gap x xa..xb (z from the hub row oz - 4 back), found
    // by `spiral_search` clear of the strips; the branches leave the hub rows' ends. The line's
    // blocks stand at odd x (it starts at x 9): an end at even x is a DUST beside the line's block
    // one cell toward the gap (strongly powered at s, the branch's comparator reads the dust)
    let mut reserve: Vec<(P, u8)> = Vec::new();
    let mut ends: Vec<(i32, P)> = Vec::new();
    let mut parity_dust: Vec<P> = Vec::new();
    for &(_, _, _, cx, lvl) in &segs {
        let (x, e, inward) = if cx < 8 { (xa - 1, xa, 1) } else { (xb + 1, xb, -1) };
        reserve.push(([x, lvl - 1, oz - 4], 0));
        reserve.push(([x, lvl - 2, oz - 4], 1));
        if e.rem_euclid(2) == 1 {
            ends.push((lvl - 1, [e, lvl - 1, oz - 4]));
        } else {
            reserve.push(([e, lvl - 1, oz - 4], 2));
            reserve.push(([e, lvl - 2, oz - 4], 1));
            ends.push((lvl - 1, [e + inward, lvl - 1, oz - 4]));
            parity_dust.push([e, lvl - 1, oz - 4]);
        }
    }
    let lx = if xa % 2 == 0 { xa + 1 } else { xa };
    let hx = if xb % 2 == 0 { xb - 1 } else { xb };
    let (top, moves, cuts) = spiral_search(lx, hx, oz - 4, oz - 4 + o.spiral_depth, 10, &ends, &b.blocks, &reserve).ok_or_else(|| anyhow!("no s spiral"))?;
    if std::env::var("MCTAS_SPIRAL_DEBUG").is_ok() {
        println!("s spiral from {top:?}: {}; cut stones {cuts:?}", moves.join(" "));
    }
    for c in cuts {
        b.stone(c);
    }
    b.stone(top);
    let mut col = top;
    for m in &moves {
        col = walk(&mut b, col, m);
    }
    for q in parity_dust {
        if !b.blocks.contains_key(&q) {
            b.dust(q);
        }
    }
    for (seg, lx, row, cx, lvl) in segs.into_iter() {
        // the hub: the strip's Q comparator reads the block at frame (-2, -1, 0)
        let hub = [cx, lvl - 1, oz - 2];
        // the hub row's end on this side of the gap (x 7 for column A, 9 for column B): a block,
        // or the dust under the previous block (a comparator reads either)
        let side_x = if cx < 8 { xa } else { xb };
        let src = [side_x, lvl - 1, oz - 4];
        ensure!(b.blocks.contains_key(&src), "no s on the hub row at {src:?}");
        let dir = if cx < src[0] { "W" } else { "E" };
        let steps = ((cx - src[0]).abs() / 2) as usize;
        ensure!(steps * 2 == (cx - src[0]).unsigned_abs() as usize, "hub {cx} not an even number of steps from {src:?}");
        let mut moves: Vec<&str> = std::iter::repeat_n(dir, steps).collect();
        moves.push("S");
        let end = walk(&mut b, src, &moves.join(" "));
        ensure!(end == hub, "s branch ends at {end:?}, want {hub:?}");
        // the lane: from the strip's output repeater (frame u = -4, pointing -u) to the lamp
        let _ = row;
        lanes.push(crate::route::Lane { src: [cx, lvl + 1, oz - 4], src_dir: D::N, lamp: [lx, row, 0] });
    }
    // the chain at the back, over column B (x 7..15, y 8..10), its inputs on the back face; s
    // leaves the hub east (comparator, block) and turns north along x = 15 at y = 10, then west
    // into the column's top
    let (_, smx) = bounds(&b.blocks);
    let mut zc = smx[2] - 2;
    if (zc - top[2]) % 2 != 0 {
        zc += 1;
    }
    chain_back(&mut b, [8, 8, zc], code);
    for q in [[16, 10, zc], [17, 10, zc]] {
        b.blocks.remove(&q);
        b.needs.retain(|p| *p != q);
    }
    let n = ((zc - top[2]) / 2) as usize;
    let mut moves: Vec<&str> = std::iter::repeat_n("N", n).collect();
    moves.extend(["W", "W", "W"]);
    let end = walk(&mut b, [15, 10, zc], &moves.join(" "));
    ensure!(end == top, "chain feed ends at {end:?}");
    // the routing box: the base's own x and y extent, z from 1 to the strip heads
    let (mn, mx) = bounds(&b.blocks);
    let lo = [mn[0].min(0) - o.margin, mn[1].min(-1), 1];
    let hi = [mx[0] + o.margin, mx[1], oz - 5];
    Ok((b, lanes, lo, hi))
}

/// A lossless line (walk moves `dN`/`dS`/`dE`/`dW`: dust under the block, comparator one level
/// down, block; flat `N`/`S`/`E`/`W`: comparator, block) in the gap x `x_lo..=x_hi`, z
/// `z_lo..=z_hi`, from a block at (9, `y_top`, z) down to the lowest hub level, clear of the strips
/// (`base`). No comparator or dust may stand on anything but a stone (a stone can be shared as a
/// support), no cell is used twice, and before the line leaves a hub level every `ends` cell of
/// that level (where a branch to a strip starts) is a block or dust of the line.
#[allow(clippy::too_many_arguments)]
pub fn spiral_search(x_lo: i32, x_hi: i32, z_lo: i32, z_hi: i32, y_top: i32, ends: &[(i32, P)], base: &std::collections::BTreeMap<P, String>, reserve: &[(P, u8)]) -> Option<(P, Vec<&'static str>, Vec<P>)> {
    use std::collections::HashMap;
    #[derive(Clone, Copy, PartialEq)]
    enum C {
        Stone,
        Used,
        Cut,
        Dust,
        /// a comparator, along x (true) or z
        Comp(bool),
    }
    let y_end = ends.iter().map(|e| e.0).min()?;
    // base stones a base diode points into (strongly powered: a dust next to them reads them)
    let mut strong: std::collections::HashSet<P> = std::collections::HashSet::new();
    for (p, st) in base {
        if !(st.contains("repeater") || st.contains("comparator")) {
            continue;
        }
        let out = if st.contains("facing=north") {
            [0, 0, 1]
        } else if st.contains("facing=south") {
            [0, 0, -1]
        } else if st.contains("facing=west") {
            [1, 0, 0]
        } else {
            [-1, 0, 0]
        };
        let q = add(*p, out);
        if base.get(&q).is_some_and(|b| b == "minecraft:stone") {
            strong.insert(q);
        }
    }
    struct S<'a> {
        y_end: i32,
        x_lo: i32,
        x_hi: i32,
        z_lo: i32,
        z_hi: i32,
        ends: &'a [(i32, P)],
        base: &'a std::collections::BTreeMap<P, String>,
        m: HashMap<P, C>,
        out: Vec<&'static str>,
        line: Vec<P>,
        flats: usize,
        strong: std::collections::HashSet<P>,
    }
    fn hub_done(s: &S, y: i32) -> bool {
        s.ends.iter().filter(|e| e.0 == y).all(|e| s.line.contains(&e.1))
    }
    fn nb(q: P) -> [P; 6] {
        [add(q, [1, 0, 0]), add(q, [-1, 0, 0]), add(q, [0, 1, 0]), add(q, [0, -1, 0]), add(q, [0, 0, 1]), add(q, [0, 0, -1])]
    }
    /// A cell of the line must not touch the strips: only base stone next to it (a dust: only
    /// stone no base diode powers), and a dust no base dust diagonally below past an open corner
    /// unless a cut stone fills the corner (returned; None: the cell is not clear).
    fn clear(s: &S, q: P, dust: bool) -> Option<Vec<P>> {
        if s.base.contains_key(&q) {
            return None;
        }
        for n in nb(q) {
            if let Some(b) = s.base.get(&n)
                && (b != "minecraft:stone" || (dust && s.strong.contains(&n)))
            {
                return None;
            }
        }
        let mut cuts = Vec::new();
        if dust {
            for h in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                let corner = add(q, h);
                let below = add(corner, [0, -1, 0]);
                if !s.base.contains_key(&corner) && !s.m.contains_key(&corner) && s.base.get(&below).is_some_and(|b| b.contains("redstone_wire")) {
                    // the cut stone touches no base diode
                    if nb(corner).iter().any(|n| s.base.get(n).is_some_and(|b| b.contains("repeater") || b.contains("comparator"))) {
                        return None;
                    }
                    cuts.push(corner);
                }
            }
        }
        Some(cuts)
    }
    fn rec(s: &mut S, p: P) -> bool {
        if std::env::var("MCTAS_SPIRAL_DEBUG2").is_ok() {
            println!("{} at {p:?}", s.out.join(" "));
        }
        if p[1] == s.y_end && hub_done(s, p[1]) {
            return true;
        }
        let moves: [(&'static str, [i32; 2], i32); 8] = [
            ("dN", [0, -1], -1),
            ("dS", [0, 1], -1),
            ("dE", [1, 0], -1),
            ("dW", [-1, 0], -1),
            ("N", [0, -1], 0),
            ("S", [0, 1], 0),
            ("E", [1, 0], 0),
            ("W", [-1, 0], 0),
        ];
        for (name, d, dy) in moves {
            if dy < 0 && !hub_done(s, p[1]) {
                continue;
            }
            if dy < 0 && p[1] == s.y_end {
                continue;
            }
            if dy == 0 && s.flats >= 10 {
                continue;
            }
            let y = p[1] + dy;
            let comp = [p[0] + d[0], y, p[2] + d[1]];
            let blk = [p[0] + 2 * d[0], y, p[2] + 2 * d[1]];
            if blk[0] < s.x_lo || blk[0] > s.x_hi || blk[2] < s.z_lo || blk[2] > s.z_hi {
                continue;
            }
            // (cell, what, kind: 0 comparator, 1 block, 2 dust, 3 comparator support, 4 dust support)
            let along_x = d[1] == 0;
            let mut want: Vec<(P, C, u8)> = vec![(comp, C::Comp(along_x), 0), (blk, C::Used, 1), ([comp[0], y - 1, comp[2]], C::Stone, 3)];
            if dy < 0 {
                let dust = [p[0], y, p[2]];
                want.push((dust, C::Dust, 2));
                want.push(([dust[0], y - 1, dust[2]], C::Stone, 4));
            }
            let mut ok = true;
            let mut added: Vec<P> = Vec::new();
            for (q, c, k) in &want {
                if *k == 3 && s.base.get(q).is_some_and(|b| b == "minecraft:stone") {
                    continue;
                }
                let clean = match k {
                    0 | 1 | 4 => clear(s, *q, false),
                    2 => clear(s, *q, true),
                    _ => (!s.base.contains_key(q)).then(Vec::new),
                };
                let Some(cuts) = clean else {
                    ok = false;
                    break;
                };
                for cut in cuts {
                    if let std::collections::hash_map::Entry::Vacant(e) = s.m.entry(cut) {
                        e.insert(C::Cut);
                        added.push(cut);
                    }
                }
                match s.m.get(q) {
                    None => {
                        s.m.insert(*q, *c);
                        added.push(*q);
                    }
                    Some(C::Stone | C::Cut) if *c == C::Stone => {}
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }
            // no line dust beside a line comparator's side (a compare-mode comparator outputs 0
            // while a side leads its rear: a glitch at every strip)
            if ok {
                let side = |q: P, x_axis: bool| -> [P; 2] { if x_axis { [add(q, [0, 0, 1]), add(q, [0, 0, -1])] } else { [add(q, [1, 0, 0]), add(q, [-1, 0, 0])] } };
                for (q, c, _) in &want {
                    match c {
                        C::Comp(ax) => {
                            if side(*q, *ax).iter().any(|n| s.m.get(n) == Some(&C::Dust)) {
                                ok = false;
                            }
                        }
                        C::Dust => {
                            for h in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                                let n = add(*q, h);
                                if let Some(C::Comp(ax)) = s.m.get(&n)
                                    && side(n, *ax).contains(q)
                                {
                                    ok = false;
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            if ok {
                s.line.push(blk);
                if dy < 0 {
                    s.line.push([p[0], y, p[2]]);
                }
                s.out.push(name);
                if dy == 0 {
                    s.flats += 1;
                }
                if rec(s, blk) {
                    return true;
                }
                if dy == 0 {
                    s.flats -= 1;
                }
                s.out.pop();
                s.line.pop();
                if dy < 0 {
                    s.line.pop();
                }
            }
            for q in added {
                s.m.remove(&q);
            }
        }
        false
    }
    for z0 in z_lo..=z_hi {
        let p = [9, y_top, z0];
        let mut s = S { y_end, x_lo, x_hi, z_lo, z_hi, ends, base, m: HashMap::new(), out: Vec::new(), line: vec![p], flats: 0, strong: strong.clone() };
        s.m.insert(p, C::Used);
        for (q, k) in reserve {
            s.m.insert(*q, match k {
                1 => C::Stone,
                2 => C::Dust,
                _ => C::Used,
            });
        }
        if rec(&mut s, p) {
            let cuts: Vec<P> = s.m.iter().filter(|(_, c)| **c == C::Cut).map(|(q, _)| *q).collect();
            return Some((p, s.out, cuts));
        }
    }
    None
}
