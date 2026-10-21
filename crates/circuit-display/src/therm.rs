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
use anyhow::{Result, ensure};

/// A strip placement: global = o + (su * u, h, v).
#[derive(Clone, Copy)]
pub struct T {
    pub o: P,
    pub su: i32,
}

impl T {
    pub fn p(&self, u: i32, h: i32, v: i32) -> P {
        add(self.o, [self.su * u, h, v])
    }
    pub fn d(&self, d: D) -> D {
        match (d, self.su) {
            (D::E, -1) => D::W,
            (D::W, -1) => D::E,
            (d, _) => d,
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

/// One segment's strip. Returns the global cell the output repeater points into.
pub fn build_strip(b: &mut B, t: &T, rs: &[Run]) -> P {
    let st = |b: &mut B, u: i32, h: i32, v: i32| {
        let q = t.p(u, h, v);
        if !b.blocks.contains_key(&q) {
            b.stone(q);
        }
    };
    let max_idx = rs.iter().map(|r| if r.hi == 15 { r.lo as i32 - 1 } else { r.hi as i32 }).max().unwrap_or(0).max(0);
    // ruler
    for u in 0..=max_idx {
        b.dust(t.p(u, 0, 0));
    }
    // B under the first cell, Q, hub
    b.stone(t.p(0, -1, 0));
    b.comp(t.p(-1, -1, 0), t.d(D::E), false);
    b.stone(t.p(-2, -1, 0));
    let mut cs: [Vec<i32>; 2] = [Vec::new(), Vec::new()];
    for (k, r) in rs.iter().enumerate() {
        let side = k % 2;
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
                    b.dust(t.p(u, 0, 2 * sg));
                    st(b, u, 1, 2 * sg);
                }
                b.rep(t.p(lo, 0, 2 * sg), t.d(D::W), 1);
            }
        }
    }
    // OR rows, merges, returns
    for side in 0..2 {
        if cs[side].is_empty() {
            continue;
        }
        let sg = if side == 0 { 1 } else { -1 };
        let out = if sg > 0 { D::S } else { D::N };
        let um = merge_u(&cs[side]);
        let (lo, hi) = (*cs[side].iter().min().unwrap(), *cs[side].iter().max().unwrap());
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
    b.stone(t.p(-3, 1, 0));
    b.rep(t.p(-4, 1, 0), t.d(D::W), 1);
    t.p(-5, 1, 0)
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
    let t = T { o: [0, 0, 0], su: 1 };
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
        let t = T { o: [if side > 0 { 7 } else { -5 }, y - 1, zc], su: side };
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
pub fn full_flat(code: &Code, out: &std::path::Path) -> Result<()> {
    ensure!(code.neg == 15, "the chain supports neg 1111 only");
    let m = masks(code);
    let mut b = B::default();
    // the split block at (-6, 0, 50): the -x spine's height
    let split = chain_back(&mut b, [-15, -2, 50], code);
    let band = "N N N N N N";
    let half = "N N N";
    // -x spine (x -6, y 0): d (z 42), g (30), c (18), b (6)
    walk(&mut b, split, &format!("N {band} {band} {band} {half}"));
    // +x spine (x 8, y 1): a (30), e (18), f (6)
    let e = walk(&mut b, split, "E E E E E E E");
    walk(&mut b, e, &format!("uN {band} {band} {band} {half}"));
    // segment, lamp column x, lamp y, band centre z, side (+1: strip at +x)
    let segs: [(char, i32, i32, i32, i32); 7] = [('f', 2, 3, 6, 1), ('b', 0, 3, 6, -1), ('e', 2, 1, 18, 1), ('c', 0, 1, 18, -1), ('a', 1, 4, 30, 1), ('g', 1, 2, 30, -1), ('d', 1, 0, 42, -1)];
    for (seg, x, y, zc, side) in segs {
        let si = "abcdefg".find(seg).unwrap();
        let oy = if side > 0 { 3 } else { 2 };
        let t = T { o: [if side > 0 { 10 } else { -8 }, oy - 1, zc], su: side };
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
    let d = b.design("seven-segment-therm-v3", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: {} blocks", out.display(), d.blocks.len());
    Ok(())
}
