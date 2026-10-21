//! The analog stack generator (`mctas circuit display analog-stack`): the chain at the bottom,
//! a lossless riser (comparator -> block -> dust on top) spiralling on the radius-4 square around
//! the ring column (2 cells per level), and one cross ring per segment every 2 levels (astack.rs),
//! each fed at distance 0 by the riser cells beside it.

use crate::abuild::{B, D, P, add};
use crate::analog::{Code, masks, runs};
use crate::astack::{self, Slot};
use anyhow::{Result, anyhow, bail};

/// A placement transform of the chain frame: rotation (0..4 quarter turns, x,z -> z,-x each) and
/// mirror (x -> -x first), then translation.
#[derive(Clone, Copy, Debug)]
pub struct Tf {
    pub mirror: bool,
    pub rot: usize,
    pub t: P,
}

impl Tf {
    pub fn xz(&self, x: i32, z: i32) -> (i32, i32) {
        let (mut x, mut z) = if self.mirror { (-x, z) } else { (x, z) };
        for _ in 0..self.rot {
            (x, z) = (z, -x);
        }
        (x, z)
    }
    pub fn p(&self, p: P) -> P {
        let (x, z) = self.xz(p[0], p[2]);
        add([x, p[1], z], self.t)
    }
    pub fn d(&self, d: D) -> D {
        let v = d.v();
        let (x, z) = self.xz(v[0], v[2]);
        match (x, z) {
            (0, -1) => D::N,
            (0, 1) => D::S,
            (-1, 0) => D::W,
            _ => D::E,
        }
    }
}

/// Rewrite the facing of a block state through a transform.
fn tf_state(s: &str, tf: &Tf) -> String {
    let Some(i) = s.find("facing=") else { return s.to_string() };
    let rest = &s[i + 7..];
    let end = rest.find([',', ']']).unwrap_or(rest.len());
    let f = &rest[..end];
    let d = match f {
        "north" => D::N,
        "south" => D::S,
        "west" => D::W,
        "east" => D::E,
        _ => return s.to_string(),
    };
    // facing names the input side; transforming the direction vector keeps that meaning
    format!("{}facing={}{}", &s[..i], tf.d(d).name(), &rest[end..])
}

/// Copy `src` into `dst` through a transform; returns the cells that clash.
pub fn merge(dst: &mut B, src: &B, tf: &Tf) -> Vec<P> {
    let mut clash = Vec::new();
    for (p, s) in &src.blocks {
        let q = tf.p(*p);
        let st = tf_state(s, tf);
        if let Some(old) = dst.blocks.get(&q)
            && *old != st
        {
            clash.push(q);
        }
        dst.blocks.insert(q, st);
    }
    for (p, n) in &src.nbt {
        dst.nbt.insert(tf.p(*p), n.clone());
    }
    for (n, p) in &src.ports {
        dst.ports.push((n.clone(), tf.p(*p)));
    }
    for p in &src.needs {
        dst.needs.push(tf.p(*p));
    }
    clash
}

/// The chain for w [8,1,4,2] neg 0001 (a0 complemented, a1 a2 a3 plain), in its own frame
/// (comparators at y = 1, stones at y = 0). s comes out as the dust on top of the block (5,1,0).
/// Returns the builder and each input's (cell, outward direction) for the extension lines.
pub fn chain4(code: &Code) -> (B, Vec<(String, P, D)>) {
    let mut b = B::default();
    let p = |x: i32, z: i32| [x, 1, z];
    b.hopper(p(0, 0), 15);
    for i in 0..4 {
        b.comp(p(1 + i, 0), D::E, true);
    }
    b.stone(p(5, 0));
    // stage 1: a0 complemented, weight w[0]: rear composter, side the bit
    b.comp(p(1, 1), D::N, true);
    b.composter(p(1, 2), code.w[0]);
    b.dust(p(0, 1));
    // stage 2: a1 plain: rear the bit (input repeater), side K2 = hopper (15 - w)
    b.comp(p(2, -1), D::S, true);
    b.rep(p(2, -2), D::S, 1);
    b.comp(p(1, -1), D::E, false);
    b.hopper(p(0, -1), 15 - code.w[1]);
    // stage 3: a2 plain
    b.comp(p(3, -1), D::S, true);
    b.rep(p(3, -2), D::S, 1);
    b.comp(p(4, -1), D::W, false);
    b.hopper(p(5, -1), 15 - code.w[2]);
    // stage 4: a3 plain
    b.comp(p(4, 1), D::N, true);
    b.rep(p(4, 2), D::N, 1);
    b.comp(p(3, 1), D::E, false);
    b.hopper(p(2, 1), 15 - code.w[3]);
    let inputs = vec![("input.a0".to_string(), p(0, 1), D::W), ("input.a1".to_string(), p(2, -2), D::N), ("input.a2".to_string(), p(3, -2), D::N), ("input.a3".to_string(), p(4, 2), D::S)];
    (b, inputs)
}

/// The radius-4 square around (cx, cz), starting at `start` and going `dir` (+1 / -1).
fn perimeter(cx: i32, cz: i32, start: (i32, i32), dir: i32) -> Option<Vec<(i32, i32)>> {
    let mut cells = Vec::new();
    // clockwise from the NW corner: north side west->east, east side north->south, ...
    for x in cx - 4..cx + 4 {
        cells.push((x, cz - 4));
    }
    for z in cz - 4..cz + 4 {
        cells.push((cx + 4, z));
    }
    for x in (cx - 3..=cx + 4).rev() {
        cells.push((x, cz + 4));
    }
    for z in (cz - 3..=cz + 4).rev() {
        cells.push((cx - 4, z));
    }
    let i = cells.iter().position(|&c| c == start)?;
    let n = cells.len() as i32;
    Some((0..n).map(|k| cells[((i as i32 + dir * k).rem_euclid(n)) as usize]).collect())
}

pub struct Plan {
    pub b: B,
    pub clashes: Vec<P>,
    pub report: Vec<String>,
    /// per ring: its lamp and the dust distances it should carry (world cells)
    pub expect: Vec<(P, Vec<(P, u8)>)>,
}

/// Build the whole stack; `rings` are the ring levels (lowest first).
pub fn build(code: &Code, tf: &Tf, center: (i32, i32), dir: i32, levels: &[i32]) -> Result<Plan> {
    build_mode(code, tf, center, dir, levels, None)
}

/// `down`: no riser; the chain sits on top (its frame's y = 1 at `top + 2`, through `tf`), its
/// output block drives a dust under it, a comparator in direction `down` and a block that powers
/// the top ring's arc cell under it; s then flows down through the rings' bridges.
pub fn build_mode(code: &Code, tf: &Tf, center: (i32, i32), dir: i32, levels: &[i32], down: Option<Vec<D>>) -> Result<Plan> {
    let (cx, cz) = center;
    TOP_FORBID.with(|f| f.borrow_mut().clear());
    let mut b = B::default();
    let mut clashes = Vec::new();
    let mut report = Vec::new();
    let mut expect = Vec::new();
    let (cb, inputs) = chain4(code);
    clashes.extend(merge(&mut b, &cb, tf));
    let top = *levels.last().unwrap();
    let xo = tf.p([5, 1, 0]);
    let mut top_feed: Vec<((i32, i32, i32), u8)> = Vec::new();
    let mut riser: Vec<(i32, P, &str)> = Vec::new();
    let radius = if down.is_some() { 3 } else { 4 };
    if let Some(path) = &down
        && path.is_empty()
    {
        // the chain's output block over a dust on a rear hopper (axis cell of radius 3): the dust
        // reaches the two arc cells beside the hopper diagonally (distance 1)
        let dd = [xo[0], xo[1] - 1, xo[2]];
        let r = (dd[0] - cx, dd[2] - cz);
        if !((r.0.abs() == 3 && r.1 == 0) || (r.1.abs() == 3 && r.0 == 0)) || dd[1] != top + 1 {
            bail!("chain output not over a rear hopper of the top ring");
        }
        if b.blocks.contains_key(&dd) {
            clashes.push(dd);
        }
        b.dust(dd);
        let side = if r.0 == 0 { (1, 0) } else { (0, 1) };
        for sgn in [1, -1] {
            let a = (r.0 + sgn * side.0, 0, r.1 + sgn * side.1);
            if !b.blocks.contains_key(&[cx + a.0, top + 1, cz + a.2]) {
                top_feed.push((a, 1));
            }
        }
    } else if let Some(path) = &down {
        let dd = [xo[0], xo[1] - 1, xo[2]];
        if b.blocks.contains_key(&dd) {
            clashes.push(dd);
        }
        b.dust(dd);
        let mut forbid = vec![(dd[0] - cx, 0, dd[2] - cz)];
        if dd[0] == cx && dd[2] == cz {
            bail!("descent dust over the lamp");
        }
        let mut cur = dd;
        let mut last = dd;
        for &u in path {
            let v = u.v();
            let q = add(cur, v);
            let z = add(q, v);
            if b.blocks.contains_key(&q) {
                clashes.push(q);
            }
            b.comp(q, u, false);
            if b.blocks.get(&z).is_some_and(|s| s != "minecraft:stone") {
                clashes.push(z);
            }
            b.stone(z);
            forbid.push((q[0] - cx, 0, q[2] - cz));
            if z[0] == cx && z[2] == cz || q[0] == cx && q[2] == cz {
                bail!("descent block over the lamp");
            }
            // a powered block over ring dust feeds it at distance 0
            top_feed.push(((z[0] - cx, 0, z[2] - cz), 0));
            cur = z;
            last = z;
        }
        let f = (last[0] - cx, 0, last[2] - cz);
        if !(f.0.abs() <= 3 && f.2.abs() <= 3 && (f.0.abs() == 3 || f.2.abs() == 3) && f.0 != 0 && f.2 != 0) || last[1] != top + 1 {
            bail!("descent block {last:?} is not over an arc cell of the top ring");
        }
        TOP_FORBID.with(|fb| *fb.borrow_mut() = forbid);
        // caps: the descent dust must not reach the top ring's dust diagonally
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let n = [dd[0] + dx, dd[1], dd[2] + dz];
            if !b.blocks.contains_key(&n) {
                b.stone(n);
            }
        }
    } else {
    // the chain's output block and the riser start
    let path = perimeter(cx, cz, (xo[0], xo[2]), dir).ok_or_else(|| anyhow!("chain output {xo:?} not on the radius-4 square"))?;
    // riser: level h has D at path[2(h-2)], R at +1, X at +2
    for h in 2..=top {
        let i = 2 * (h - 2) as usize;
        let at = |k: usize| {
            let c = path[k % path.len()];
            [c.0, h, c.1]
        };
        riser.push((h, at(i), "D"));
        riser.push((h, at(i + 1), "R"));
        riser.push((h, at(i + 2), "X"));
        let set = |b: &mut B, p: P, s: &str, cl: &mut Vec<P>| {
            if b.blocks.contains_key(&p) {
                cl.push(p);
            }
            match s {
                "D" => b.dust(p),
                "X" => b.stone(p),
                _ => {}
            }
        };
        set(&mut b, at(i), "D", &mut clashes);
        set(&mut b, at(i + 2), "X", &mut clashes);
        let (r, x) = (at(i + 1), at(i + 2));
        let dir = match ((x[0] - r[0]).signum(), (x[2] - r[2]).signum()) {
            (1, _) => D::E,
            (-1, _) => D::W,
            (_, 1) => D::S,
            _ => D::N,
        };
        if b.blocks.contains_key(&r) {
            clashes.push(r);
        }
        b.comp(r, dir, false);
    }
    }
    // caps over the riser dust: no dust link to the ring cells one level up or down
    for &(h, p, kind) in &riser {
        if kind == "D" && h < top {
            let q = [p[0], h + 1, p[2]];
            if !b.blocks.contains_key(&q) {
                b.stone(q);
            }
        }
    }
    // rings, top down: feeds = arc cells beside a riser X (distance 0) or D (1) at that level,
    // plus what the ring above's bridges and copies power from above
    let segs: Vec<Vec<crate::analog::Run>> = masks(code).iter().map(|m| runs(*m)).collect();
    let riser_feed = |y: i32| {
        let mut feed = Vec::new();
        if y == top {
            feed.extend(top_feed.iter().copied());
        }
        for &(h, p, kind) in &riser {
            if h != y || kind == "R" {
                continue;
            }
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let rel = (p[0] + dx - cx, 0, p[2] + dz - cz);
                if rel.0.abs() <= 3 && rel.2.abs() <= 3 && (rel.0.abs() == 3 || rel.2.abs() == 3) && rel.0 != 0 && rel.2 != 0 {
                    feed.push((rel, if kind == "X" { 0u8 } else { 1u8 }));
                }
            }
        }
        feed
    };
    type Sol = (usize, [Slot; 4], u8, Vec<((i32, i32, i32), u8)>);
    // per level: the copies allowed and the bridges forced off (their support two levels down
    // is a chain or riser block a comparator cannot stand on)
    let sturdy_or_free = |p: P| b.blocks.get(&p).is_none_or(|s| s == "minecraft:stone" || s.contains("hopper"));
    let limits: Vec<(u8, u8)> = levels
        .iter()
        .map(|&y| {
            if y - 2 < 0 {
                // the bottom ring: nothing to stand its support-level comparators on
                return (0u8, 15u8);
            }
            let mut forced = 0u8;
            for (bi, &(p, q)) in astack::BRIDGES.iter().enumerate() {
                let m = [cx + (p.0 + q.0) / 2, y - 2, cz + (p.2 + q.2) / 2];
                if !sturdy_or_free(m) {
                    forced |= 1 << bi;
                }
            }
            let mut allowed = 0u8;
            for u in 0..4 {
                let (kx, kz) = astack::rot_pub(u, 2, -2);
                if sturdy_or_free([cx + kx, y - 2, cz + kz]) {
                    allowed |= 1 << u;
                }
            }
            (allowed, forced)
        })
        .collect();
    #[allow(clippy::too_many_arguments)]
    fn rec(t: usize, levels: &[i32], segs: &[Vec<crate::analog::Run>], used: &mut [bool], above: Vec<((i32, i32, i32), u8)>, riser_feed: &dyn Fn(i32) -> Vec<((i32, i32, i32), u8)>, limits: &[(u8, u8)], out: &mut Vec<Sol>) -> bool {
        let mut feed = riser_feed(levels[t]);
        feed.extend(above);
        DEEPEST.with(|d| d.set(d.get().min(t)));
        for s in 0..segs.len() {
            if used[s] {
                continue;
            }
            let under: Vec<(i32, i32, i32)> = if t == levels.len() - 1 { TOP_FORBID.with(|f| f.borrow().clone()) } else { Vec::new() };
            let is_arc = |c: &(i32, i32, i32)| (c.0.abs() == 3 || c.2.abs() == 3) && c.0.abs() <= 3 && c.2.abs() <= 3 && c.0 != 0 && c.2 != 0;
            let forbid: Vec<(i32, i32, i32)> = under.iter().filter(|c| !is_arc(c)).copied().collect();
            let removed: Vec<(i32, i32, i32)> = under.iter().filter(|c| is_arc(c)).copied().collect();
            let mut forced = limits[t].1;
            let mut allowed = limits[t].0;
            for (u, (a, _)) in astack::COPIES.iter().enumerate() {
                if removed.contains(a) {
                    allowed &= !(1 << u);
                }
            }
            for (bi, (p, q)) in astack::BRIDGES.iter().enumerate() {
                if removed.contains(p) || removed.contains(q) {
                    forced |= 1 << bi;
                }
            }
            let key = (s, feed.clone(), (allowed, forced), under.clone());
            let hit = MEMO.with(|m| m.borrow().get(&key).cloned());
            astack::REMOVED.with(|f| *f.borrow_mut() = removed.clone());
            let r = match hit {
                Some(r) => r,
                None => {
                    astack::FORBID.with(|f| *f.borrow_mut() = forbid);
                    let r = astack::solve_ring_f(&segs[s], &feed, allowed, forced);
                    astack::FORBID.with(|f| f.borrow_mut().clear());
                    MEMO.with(|m| m.borrow_mut().insert(key, r));
                    r
                }
            };
            let Some((slots, copies)) = r else {
                astack::REMOVED.with(|f| f.borrow_mut().clear());
                continue;
            };
            let below = astack::coupling(&slots, &feed, copies);
            astack::REMOVED.with(|f| f.borrow_mut().clear());
            used[s] = true;
            out.push((s, slots, copies, feed.clone()));
            if t == 0 || rec(t - 1, levels, segs, used, below, riser_feed, limits, out) {
                return true;
            }
            out.pop();
            used[s] = false;
        }
        false
    }
    let mut sols = Vec::new();
    DEEPEST.with(|d| d.set(99));
    let mut used = vec![false; segs.len()];
    if !rec(levels.len() - 1, levels, &segs, &mut used, Vec::new(), &riser_feed, &limits, &mut sols) {
        bail!("no assignment of segments to levels (deepest level index {})", DEEPEST.with(|d| d.get()));
    }
    for (k, (s, slots, copies, feed)) in sols.iter().enumerate() {
        let t = levels.len() - 1 - k;
        let c = [cx, levels[t], cz];
        if k == 0 {
            let is_arc = |c: &(i32, i32, i32)| (c.0.abs() == 3 || c.2.abs() == 3) && c.0.abs() <= 3 && c.2.abs() <= 3 && c.0 != 0 && c.2 != 0;
            let removed: Vec<(i32, i32, i32)> = TOP_FORBID.with(|f| f.borrow().iter().filter(|c| is_arc(c)).copied().collect());
            astack::REMOVED.with(|f| *f.borrow_mut() = removed);
        }
        let mut rb = B::default();
        astack::build_ring(&mut rb, [0, 0, 0], slots, feed, *copies);
        let rc = merge(&mut b, &rb, &Tf { mirror: false, rot: 0, t: c });
        clashes.extend(rc);
        b.port(&format!("output.{}", "abcdefg".as_bytes()[*s] as char), c);
        report.push(format!("level {}: segment {} feeds {:?} slots {slots:?} copies {copies:04b}", levels[t], "abcdefg".as_bytes()[*s] as char, feed));
        let dm = astack::ring_distances(slots, feed, *copies);
        expect.push((c, dm.iter().map(|(r, v)| (add(c, [r.0, r.1, r.2]), *v)).collect()));
        astack::REMOVED.with(|f| f.borrow_mut().clear());
    }
    // input extension lines out to the footprint's edge
    for (name, p, d) in inputs {
        let mut q = tf.p(p);
        let dd = tf.d(d);
        let v = dd.v();
        let state = b.blocks.get(&q).cloned().unwrap_or_default();
        let mut last = q;
        loop {
            let nq = add(q, v);
            if (nq[0] - cx).abs() > radius || (nq[2] - cz).abs() > radius {
                break;
            }
            q = nq;
            if b.blocks.contains_key(&q) {
                clashes.push(q);
            }
            b.rep(q, dd.opp(), 1);
            last = q;
        }
        let _ = state;
        b.port(&name, last);
    }
    Ok(Plan { b, clashes, report, expect })
}

/// Cells that need a sturdy block under them but stand on something else.
pub fn bad_supports(b: &B) -> Vec<P> {
    let mut bad = Vec::new();
    for p in &b.needs {
        let q = [p[0], p[1] - 1, p[2]];
        if let Some(s) = b.blocks.get(&q)
            && !(s == "minecraft:stone" || s.contains("hopper"))
        {
            bad.push(*p);
        }
    }
    bad
}

pub fn search_and_write(out: &std::path::Path, y0: i32, place: Option<&str>) -> Result<()> {
    let code = Code { w: [8, 1, 4, 2], neg: 1 };
    let levels: Vec<i32> = (0..7).map(|k| y0 + 2 * k).collect();
    let fixed: Option<Vec<i32>> = place.map(|s| s.split(',').map(|x| x.trim().parse().unwrap_or(0)).collect());
    let mut best: Option<(usize, Plan, String)> = None;
    for mirror in [false, true] {
        for rot in 0..4 {
            if let Some(f) = &fixed
                && (f[0] != mirror as i32 || f[1] != rot as i32)
            {
                continue;
            }
            let probe = Tf { mirror, rot, t: [0, 0, 0] };
            // chain cells' extent in this orientation
            let (cb, _) = chain4(&code);
            let pts: Vec<(i32, i32)> = cb.blocks.keys().map(|p| probe.xz(p[0], p[2])).collect();
            let (minx, maxx) = (pts.iter().map(|p| p.0).min().unwrap(), pts.iter().map(|p| p.0).max().unwrap());
            let (minz, maxz) = (pts.iter().map(|p| p.1).min().unwrap(), pts.iter().map(|p| p.1).max().unwrap());
            let xo = probe.xz(5, 0);
            for tx in -12..12 {
                for tz in -12..12 {
                    // ring center at the origin; the chain inside the 9x9 and its output on the square
                    let (ox, oz) = (xo.0 + tx, xo.1 + tz);
                    if !((ox.abs() == 4 && oz.abs() <= 4) || (oz.abs() == 4 && ox.abs() <= 4)) {
                        continue;
                    }
                    if minx + tx < -4 || maxx + tx > 4 || minz + tz < -4 || maxz + tz > 4 {
                        continue;
                    }
                    for dir in [1, -1] {
                        if let Some(f) = &fixed
                            && (f[2] != tx || f[3] != tz || f[4] != dir)
                        {
                            continue;
                        }
                        let tf = Tf { mirror, rot, t: [tx, 0, tz] };
                        let Ok(mut plan) = build(&code, &tf, (0, 0), dir, &levels) else { continue };
                        plan.b.supports();
                        let bad = bad_supports(&plan.b);
                        let score = plan.clashes.len() + bad.len();
                        let label = format!("mirror {mirror} rot {rot} t ({tx},{tz}) dir {dir}: {} clashes {:?}, {} bad supports {:?}", plan.clashes.len(), plan.clashes, bad.len(), bad);
                        if best.as_ref().is_none_or(|b| score < b.0) {
                            best = Some((score, plan, label));
                        }
                    }
                }
            }
        }
    }
    let (score, mut plan, label) = best.ok_or_else(|| anyhow!("no placement fits all segments"))?;
    println!("best: {label}");
    for r in &plan.report {
        println!("  {r}");
    }
    let d = plan.b.design("seven-segment-analog", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: score {score}, {} blocks", out.display(), d.blocks.len());
    // the dust distances on redsim with s = 15 (value 1), against the plan
    let c = circuit::Circuit::load(out)?;
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let s = circuit::stimulus::parse("a0 10+", &names, 100)?;
    let t = circuit::run::sim_trace(&c, &s, 3).map_err(|e| anyhow!("{e}"))?;
    let f = 150.min(t.cells.len() - 1);
    let cells: std::collections::HashMap<[i32; 3], &str> = t.cells[f].iter().map(|(p, i)| (*p, t.names[*i as usize].as_str())).collect();
    for (lamp, exp) in &plan.expect {
        let mut bad = Vec::new();
        for (p, dv) in exp {
            let st = cells.get(p).copied().unwrap_or("air");
            let pw: i32 = st.split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|x| x.parse().ok()).unwrap_or(-1);
            if pw != 15 - *dv as i32 {
                bad.push(format!("{p:?} want d{dv} got power {pw}"));
            }
        }
        println!("ring {lamp:?}: {} dust cells off: {}", bad.len(), bad.join("; "));
    }
    Ok(())
}

type MemoKey = (usize, Vec<((i32, i32, i32), u8)>, (u8, u8), Vec<(i32, i32, i32)>);
thread_local! {
    static TOP_FORBID: std::cell::RefCell<Vec<(i32, i32, i32)>> = const { std::cell::RefCell::new(Vec::new()) };
}
thread_local! {
    static MEMO: std::cell::RefCell<std::collections::HashMap<MemoKey, Option<([Slot; 4], u8)>>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The riser-free stack: chain on top, s flowing down through the rings' bridges.
pub fn search_down(out: &std::path::Path, y0: i32) -> Result<()> {
    let slack: i32 = std::env::var("MCTAS_DISPLAY_SLACK").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let code = Code { w: [8, 1, 4, 2], neg: 1 };
    let levels: Vec<i32> = (0..7).map(|k| y0 + 2 * k).collect();
    let top = *levels.last().unwrap();
    let mut best: Option<(usize, Plan, String)> = None;
    let mut arcs = Vec::new();
    for i in -3..=3 {
        for (x, z) in [(3, i), (-3, i), (i, 3), (i, -3)] {
            if x != 0 && z != 0 && !arcs.contains(&(x, z)) {
                arcs.push((x, z));
            }
        }
    }
    let (cb, _) = chain4(&code);
    let _ = &arcs;
    let mut errs: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let dirs = [D::N, D::S, D::W, D::E];
    let mut paths: Vec<Vec<D>> = vec![Vec::new()];
    for a in dirs {
        paths.push(vec![a]);
        for b2 in dirs {
            paths.push(vec![a, b2]);
            for c in dirs {
                paths.push(vec![a, b2, c]);
                for d4 in dirs {
                    paths.push(vec![a, b2, c, d4]);
                }
            }
        }
    }
    for mirror in [false, true] {
        for rot in 0..4 {
            let probe = Tf { mirror, rot, t: [0, 0, 0] };
            let xo = probe.xz(5, 0);
            for xx in -3i32..=3 {
                for xz in -3i32..=3 {
                    // the descent dust stands on a hopper (axis, corner of radius 2) or a corner stone
                    let ok_dd = (xx.abs() == 3 && xz == 0) || (xz.abs() == 3 && xx == 0) || (xx.abs() == 2 && xz.abs() == 2) || (xx.abs() == 1 && xz.abs() == 1);
                    if !ok_dd && slack < 0 {
                        continue;
                    }
                    let t = [xx - xo.0, top + 1, xz - xo.1];
                    let tf = Tf { mirror, rot, t };
                    if cb.blocks.keys().any(|p| {
                        let q = tf.p(*p);
                        q[0].abs() > 3 || q[2].abs() > 3
                    }) {
                        continue;
                    }
                    for path in &paths {
                        // the last block must land over an arc cell, the path inside the 7x7
                        let (mut px, mut pz) = (xx, xz);
                        let mut inside = true;
                        for d in path {
                            let v = d.v();
                            px += 2 * v[0];
                            pz += 2 * v[2];
                            let out_z = |z: i32| z > 3 || z < -3 - slack;
                            if px.abs() > 3 || out_z(pz) || (px - v[0]).abs() > 3 || out_z(pz - v[2]) {
                                inside = false;
                            }
                        }
                        let axis = (px.abs() == 3 && pz == 0) || (pz.abs() == 3 && px == 0);
                        let over_arc = px.abs() <= 3 && pz.abs() <= 3 && (px.abs() == 3 || pz.abs() == 3) && px != 0 && pz != 0;
                        if !inside || (path.is_empty() && !axis) || (!path.is_empty() && !over_arc) {
                            continue;
                        }
                        let mut plan = match build_mode(&code, &tf, (0, 0), 1, &levels, Some(path.clone())) {
                            Ok(p) => p,
                            Err(e) => {
                                let k = format!("{}{}", e.to_string().lines().next().unwrap_or("").chars().take(70).collect::<String>(), if path.is_empty() { " [axis]" } else { "" });
                                *errs.entry(k).or_insert(0) += 1;
                                continue;
                            }
                        };
                        plan.b.supports();
                        let bad = bad_supports(&plan.b);
                        let score = plan.clashes.len() + bad.len() + path.len();
                        let label = format!("mirror {mirror} rot {rot} X ({xx},{xz}) path {path:?}: {} clashes {:?}, {} bad {:?}", plan.clashes.len(), plan.clashes, bad.len(), bad);
                        if best.as_ref().is_none_or(|b| score < b.0) {
                            best = Some((score, plan, label));
                        }
                    }
                }
            }
        }
    }
    println!("build errors: {errs:?}");
    let (score, plan, label) = best.ok_or_else(|| anyhow!("no down placement fits"))?;
    write_plan(out, score, plan, &label)
}

fn write_plan(out: &std::path::Path, score: usize, mut plan: Plan, label: &str) -> Result<()> {
    println!("best: {label}");
    for r in &plan.report {
        println!("  {r}");
    }
    let d = plan.b.design("seven-segment-analog", Some("seven-segment 4"));
    std::fs::write(out, d.to_text())?;
    println!("{}: score {score}, {} blocks", out.display(), d.blocks.len());
    let c = circuit::Circuit::load(out)?;
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let s = circuit::stimulus::parse("a0 10+", &names, 100)?;
    let t = circuit::run::sim_trace(&c, &s, 3).map_err(|e| anyhow!("{e}"))?;
    let f = 150.min(t.cells.len() - 1);
    let cells: std::collections::HashMap<[i32; 3], &str> = t.cells[f].iter().map(|(p, i)| (*p, t.names[*i as usize].as_str())).collect();
    for (lamp, exp) in &plan.expect {
        let mut bad = Vec::new();
        for (p, dv) in exp {
            let st = cells.get(p).copied().unwrap_or("air");
            let pw: i32 = st.split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|x| x.parse().ok()).unwrap_or(-1);
            if pw != 15 - *dv as i32 {
                bad.push(format!("{p:?} want d{dv} got power {pw}"));
            }
        }
        println!("ring {lamp:?}: {} dust cells off: {}", bad.len(), bad.join("; "));
    }
    Ok(())
}
thread_local! {
    static DEEPEST: std::cell::Cell<usize> = const { std::cell::Cell::new(99) };
}
