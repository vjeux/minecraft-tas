//! `mctas circuit display rand8 --pic <file> --out <dir>`: randomizer 8 in two stages (WORK IN
//! PROGRESS: no layout it builds has passed yet; see designs/circuits/random/WR.md).
//!
//! - Stage 1 is a randomizer-4 layout line of the layout search: two token pairs, outputs lit at
//!   level 1, two comparators after the press.
//! - Stage 2 splits each stage-1 output in two:
//!   - a comparator Q relays the output (over a plain block when the front cell has no footing);
//!   - a hub (a dust, or a plain block with a dust under it) feeds two subtract comparators, the
//!     copies;
//!   - each copy has a repeater at its side carrying a coin bit: the other bit of its coin.
//! - One coin per side the outputs face. The coin only has to be independent of stage 1: exactly
//!   one stage-1 output is lit, so exactly one copy is lit, and every outcome has probability
//!   1/4 x 1/2.
//! - Timing: both inputs of every copy change two comparators/repeaters after the readers.
//! - Each coin's bits are its readers' threshold lines ending in repeaters, routed as dust (at most
//!   14 long) to the side repeaters. The press reaches the coins' droppers too.

use crate::abuild::{B, D, P};
use crate::rand4::{parse_pic, Pic};
use anyhow::{Result, anyhow};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;

const TOKEN: &str = "{Items:[{Slot:0b,id:\"minecraft:iron_shovel\",count:1}]}";
const RAY: i32 = 8;
const JUNK: &str = "{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}";

pub(crate) fn plus(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn times(a: P, k: i32) -> P {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dir_of(v: P) -> D {
    match v {
        [0, 0, -1] => D::N,
        [0, 0, 1] => D::S,
        [-1, 0, 0] => D::W,
        _ => D::E,
    }
}
fn perp(v: P) -> [P; 2] {
    [[v[2], 0, v[0]], [-v[2], 0, -v[0]]]
}
/// A component's output direction from its block state (`facing` is the input side).
fn out_dir(s: &str) -> P {
    match s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("") {
        "north" => [0, 0, 1],
        "south" => [0, 0, -1],
        "west" => [1, 0, 0],
        _ => [-1, 0, 0],
    }
}

/// Cells a dust must not take: every comparator's rear and sides, every repeater's rear.
pub(crate) fn guards(b: &B) -> BTreeSet<P> {
    let mut g = BTreeSet::new();
    for (p, s) in &b.blocks {
        let comp = s.contains("comparator");
        if !comp && !s.contains("repeater") {
            continue;
        }
        let f = out_dir(s);
        g.insert(plus(*p, times(f, -1)));
        if comp {
            for q in perp(f) {
                g.insert(plus(*p, q));
            }
        }
    }
    g
}

/// Corner cells of every dust climb (over the lower dust of two connected dusts a level apart).
pub(crate) fn corners(b: &B) -> BTreeSet<P> {
    let dusts: Vec<P> = b.blocks.iter().filter(|(_, s)| s.contains("redstone_wire")).map(|(p, _)| *p).collect();
    let mut c = BTreeSet::new();
    for a in &dusts {
        for u in &dusts {
            if u[1] == a[1] + 1 && (u[0] - a[0]).abs() + (u[2] - a[2]).abs() == 1 {
                c.insert([a[0], u[1], a[2]]);
            }
        }
    }
    c
}

/// Shortest dust path from the net `net` to `target` (which becomes a dust): every new dust on
/// stone (an existing plain one, not a through block, or a new one), touching no dust outside
/// the net (diagonals included), on no guarded cell (except `target`), keeping climb corners free.
pub(crate) fn route_net(b: &B, net: &BTreeSet<P>, target: P, through: &[P], keep: &BTreeSet<P>) -> Option<Vec<P>> {
    let guard = guards(b);
    let corner = corners(b);
    let foreign: Vec<P> = b.blocks.iter().filter(|(p, s)| s.contains("redstone_wire") && !net.contains(*p)).map(|(p, _)| *p).collect();
    let near = |p: P| foreign.iter().any(|s| (s[0] - p[0]).abs() + (s[2] - p[2]).abs() <= 1 && (s[1] - p[1]).abs() <= 1);
    // a bounded search: the design's box and 3 cells around it
    let lo = [0, 1, 2].map(|i| b.blocks.keys().map(|p| p[i]).min().unwrap_or(0) - 3);
    let hi = [0, 1, 2].map(|i| b.blocks.keys().map(|p| p[i]).max().unwrap_or(0) + 3);
    let ok = |p: P| -> bool {
        // never under a dust (it would stand on this one), never outside the box
        if b.blocks.get(&plus(p, [0, 1, 0])).is_some_and(|s| s.contains("redstone_wire")) || (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]) {
            return false;
        }
        if p[1] < 0 || b.blocks.contains_key(&p) || (p != target && (keep.contains(&p) || keep.contains(&plus(p, [0, -1, 0])))) || near(p) || corner.contains(&p) || (guard.contains(&p) && p != target) {
            return false;
        }
        let below = plus(p, [0, -1, 0]);
        match b.blocks.get(&below) {
            Some(s) => (s == "minecraft:stone" && !through.contains(&below)) || (p == target && s.contains("dropper")),
            None => !corner.contains(&below) && !guard.contains(&below),
        }
    };
    if std::env::var("RAND8_DEBUG").is_ok() && !ok(target) { eprintln!("target {target:?} refused: keep {} near {} corner {} occupied {:?} below {:?}", keep.contains(&target), near(target), corner.contains(&target), b.blocks.get(&target), b.blocks.get(&plus(target, [0, -1, 0]))); }
    if std::env::var("RAND8_WHY").is_ok() {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0], [1, 1, 0], [-1, 1, 0], [0, 1, 1], [0, 1, -1], [1, -1, 0], [-1, -1, 0], [0, -1, 1], [0, -1, -1]] {
            let n = plus(target, d);
            eprintln!("  {n:?}: ok {} keep {} near {} corner {} guard {} occupied {:?} below {:?}", ok(n), keep.contains(&n), near(n), corner.contains(&n), guard.contains(&n), b.blocks.get(&n), b.blocks.get(&plus(n, [0, -1, 0])));
        }
    }
    if !ok(target) {
        return None;
    }
    let mut prev: BTreeMap<P, P> = BTreeMap::new();
    let mut q: VecDeque<P> = net.iter().copied().collect();
    let mut seen: BTreeSet<P> = net.clone();
    while let Some(c) = q.pop_front() {
        for h in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for dy in -1..=1 {
                let n = [c[0] + h[0], c[1] + dy, c[2] + h[2]];
                // (nor over or under a cell this search reached: one path never stacks on itself)
                if seen.contains(&n) || !ok(n) || stacks(&prev, c, n) {
                    continue;
                }
                if dy != 0 {
                    let cr = if dy > 0 { [c[0], n[1], c[2]] } else { [n[0], c[1], n[2]] };
                    if b.blocks.contains_key(&cr) {
                        continue;
                    }
                }
                seen.insert(n);
                prev.insert(n, c);
                if n == target {
                    let mut path = vec![n];
                    let mut k = n;
                    while let Some(p) = prev.get(&k) {
                        if net.contains(p) {
                            break;
                        }
                        k = *p;
                        path.push(k);
                    }
                    return Some(path);
                }
                q.push_back(n);
            }
        }
    }
    if std::env::var("RAND8_DEBUG").is_ok() { eprintln!("no path to {target:?}: seen {} cells from a net of {}", seen.len(), net.len()); }
    None
}

/// Lay a path's dusts (their supports come with them).
pub(crate) fn lay(b: &mut B, path: &[P]) {
    for p in path {
        b.dust(*p);
    }
    b.supports();
}

/// Two cells a dust connection could join (side by side, or a level up or down).
fn reach(a: P, b: P) -> bool {
    (a[0] - b[0]).abs() + (a[2] - b[2]).abs() == 1 && (a[1] - b[1]).abs() <= 1 || (a[0] == b[0] && a[2] == b[2] && (a[1] - b[1]).abs() == 1)
}


/// Stage 2 for one stage-1 output `o` (output direction `f`): a comparator Q relaying it one cell
/// on (over a plain block it powers when the front cell has no footing), a hub dust in front of
/// Q, and two subtract comparators reading the hub (one straight on, one sideways, both with a
/// free line out), each with a repeater at a side whose input is a sink for a coin bit.
/// Returns the grown design, [(copy, sink)] for its two copies, and their lines out.
fn copies_of(b: &B, keep: &BTreeSet<P>, taken: &BTreeMap<P, usize>, o: P, f: P) -> Vec<(B, [(P, P); 2], Vec<P>)> {
    let mut variants = Vec::new();
    let free = |t: &B, p: P| !t.blocks.contains_key(&p) && !keep.contains(&p);
    let room = |t: &B, p: P| free(t, p) && !keep.contains(&plus(p, [0, -1, 0])) && t.blocks.get(&plus(p, [0, -1, 0])).is_none_or(|s| s == "minecraft:stone");
    let quiet = |t: &B, p: P| !t.blocks.iter().any(|(q, s)| s.contains("redstone_wire") && reach(*q, p));
    let ray = |t: &B, c: P, v: P| (1..=RAY).all(|k| free(t, plus(c, times(v, k))));
    // stacked: Q relays the output into a plain block K; one copy reads K (straight on), the
    // other reads the dust under K (powered by it) one level down, also straight on
    for lead in [false, true] {
        let mut t = b.clone();
        let mut q = plus(o, f);
        if lead {
            let near_dust = [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d| t.blocks.get(&plus(q, *d)).is_some_and(|s| s.contains("redstone_wire")));
            if !free(&t, q) || near_dust {
                break;
            }
            t.stone(q);
            q = plus(q, f);
        }
        let k = plus(q, f);
        let d0 = plus(k, [0, -1, 0]);
        let (c1, c2) = (plus(k, f), plus(d0, f));
        let near_k = [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d| plus(k, *d) != q && plus(k, *d) != c1 && t.blocks.get(&plus(k, *d)).is_some_and(|s| s.contains("redstone_wire")));
        if d0[1] < 0 || !room(&t, q) || !free(&t, k) || near_k || !room(&t, d0) || !quiet(&t, d0) || !room(&t, c1) || !room(&t, c2) || !ray(&t, c1, f) || !ray(&t, c2, f) {
            continue;
        }
        t.comp(q, dir_of(f), false);
        t.stone(k);
        t.dust(d0);
        let fits = |t: &B, c: P, s: P, used: &[P], bit: usize| -> Option<(P, P, P)> {
            let (r, kk) = (plus(c, s), plus(c, times(s, 2)));
            let other = plus(c, times(s, -1));
            let ok = room(t, r) && room(t, kk) && quiet(t, kk) && !reach(kk, d0) && free(t, other) && ![r, kk].iter().any(|x| used.contains(x) || *x == c1 || *x == c2 || *x == k || *x == d0) && !used.iter().any(|u| reach(*u, kk));
            let shared = taken.get(&kk).is_none_or(|j| *j == bit) && !taken.iter().any(|(qq, j)| *j != bit && reach(*qq, kk));
            (ok && shared).then_some((c, r, kk))
        };
        for s1 in perp(f) {
            let Some(a) = fits(&t, c1, s1, &[], 0) else { continue };
            for s2 in perp(f) {
                let Some(bb) = fits(&t, c2, s2, &[a.0, a.1, a.2], 1) else { continue };
                let mut t2 = t.clone();
                t2.comp(c1, dir_of(f), true);
                t2.comp(c2, dir_of(f), true);
                for (c, r, _) in [a, bb] {
                    t2.rep(r, dir_of([c[0] - r[0], 0, c[2] - r[2]]), 1);
                }
                t2.supports();
                let mut rays: Vec<P> = (1..=RAY).map(|i| plus(c1, times(f, i))).chain((1..=RAY).map(|i| plus(c2, times(f, i)))).collect();
                for d in [[0, 0, 0], [0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                    rays.push(plus(k, d));
                }
                variants.push((t2, [(a.0, a.2), (bb.0, bb.2)], rays));
            }
        }
    }
    for lead in [false, true] {
        let mut t = b.clone();
        let mut q = plus(o, f);
        if lead {
            // the plain block the output powers: no dust on it or next to it (it would power it)
            let near_dust = [[0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]].iter().any(|d| t.blocks.get(&plus(q, *d)).is_some_and(|s| s.contains("redstone_wire")));
            if !free(&t, q) || near_dust {
                break;
            }
            t.stone(q);
            q = plus(q, f);
        }
        let hub = plus(q, f);
        if !room(&t, q) || !room(&t, hub) || !quiet(&t, hub) {
            continue;
        }
        t.comp(q, dir_of(f), false);
        t.dust(hub);
        let c1 = plus(hub, f);
        for v2 in perp(f) {
            let c2 = plus(hub, v2);
            if !room(&t, c1) || !room(&t, c2) || !ray(&t, c1, f) || !ray(&t, c2, v2) || !free(&t, plus(hub, times(v2, -1))) {
                continue;
            }
            // each copy: a side repeater pointing in, its input behind it; the other side free
            let fits = |c: P, s: P, used: &[P], bit: usize| -> Option<(P, P, P)> {
                let (r, k) = (plus(c, s), plus(c, times(s, 2)));
                let other = plus(c, times(s, -1));
                let ok = room(&t, r) && room(&t, k) && quiet(&t, k) && !reach(k, hub) && (free(&t, other) || other == hub) && ![r, k].iter().any(|x| used.contains(x) || *x == c1 || *x == c2 || *x == hub) && !used.iter().any(|u| reach(*u, k));
                let shared = taken.get(&k).is_none_or(|j| *j == bit) && !taken.iter().any(|(q, j)| *j != bit && reach(*q, k));
                (ok && shared).then_some((c, r, k))
            };
            for s1 in perp(f) {
                let Some(a) = fits(c1, s1, &[], 0) else { continue };
                for s2 in perp(v2) {
                    let Some(bb) = fits(c2, s2, &[a.0, a.1, a.2], 1) else { continue };
                    let sides = [a, bb];
                    let mut t2 = t.clone();
                    t2.comp(c1, dir_of(f), true);
                    t2.comp(c2, dir_of(v2), true);
                    for (c, r, _) in &sides {
                        t2.rep(*r, dir_of([c[0] - r[0], 0, c[2] - r[2]]), 1);
                    }
                    t2.supports();
                    let mut rays: Vec<P> = (1..=RAY).map(|k| plus(c1, times(f, k))).chain((1..=RAY).map(|k| plus(c2, times(v2, k)))).collect();
                    if lead {
                        // the plain block the output powers: no dust on it, over it or next to it
                        let k0 = plus(o, f);
                        for d in [[0, 0, 0], [0, 1, 0], [1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
                            rays.push(plus(k0, d));
                        }
                    }
                    variants.push((t2, [(sides[0].0, sides[0].2), (sides[1].0, sides[1].2)], rays));
                }
            }
        }
    }
    variants
}

/// A coin (a token pair, each dropper read by a comparator whose two threshold dusts end in a
/// repeater: [R0 w w C0 D0 D1 C1 w w R1]) near `near`, its bit 0 (token in D0) routed to
/// `sinks[1]`, bit 1 to `sinks[0]`, each sink within strength. Returns the grown design and the
/// coin's two top cells (for the press).
fn coin_for(b: &B, keep: &BTreeSet<P>, near: P, sinks: &[Vec<P>; 2], through: &[P]) -> Option<(B, [P; 2])> {
    let mut cands: Vec<(i32, P, P)> = Vec::new();
    for axis in [[1, 0, 0], [0, 0, 1]] {
        for dx in -9..=9 {
            for dz in -9..=9 {
                for dy in -1..=3 {
                    let base = [near[0] + dx, near[1] + dy, near[2] + dz];
                    let cells: Vec<P> = (0..10).map(|i| plus(base, times(axis, i))).collect();
                    let sup: Vec<P> = cells.iter().enumerate().filter(|(i, _)| *i != 4 && *i != 5).map(|(_, p)| plus(*p, [0, -1, 0])).collect();
                    if base[1] < 0 || cells.iter().chain(sup.iter()).any(|p| b.blocks.contains_key(p) || keep.contains(p)) {
                        continue;
                    }
                    let mid = plus(base, times(axis, 4));
                    cands.push(((mid[0] - near[0]).abs() + (mid[1] - near[1]).abs() * 2 + (mid[2] - near[2]).abs(), base, axis));
                }
            }
        }
    }
    cands.sort();
    for (_, base, axis) in cands.into_iter().take(80) {
        let mut t = b.clone();
        let at = |i: i32| plus(base, times(axis, i));
        let (fwd, back) = (dir_of(axis), dir_of(times(axis, -1)));
        t.rep(at(0), back, 1);
        t.dust(at(1));
        t.dust(at(2));
        t.comp(at(3), back, false);
        t.set(at(4), &format!("dropper[facing={},triggered=false]", fwd.name()));
        t.nbt.insert(at(4), TOKEN.to_string());
        t.set(at(5), &format!("dropper[facing={},triggered=false]", back.name()));
        t.nbt.insert(at(5), JUNK.to_string());
        t.comp(at(6), fwd, false);
        t.dust(at(7));
        t.dust(at(8));
        t.rep(at(9), fwd, 1);
        t.supports();
        // bit 1 (token in D1) leaves at(9) forwards: it feeds sinks[0]; bit 0 leaves at(0) backwards
        let src = [plus(at(9), axis), plus(at(0), times(axis, -1))];
        let mut ok = true;
        for k in 0..2 {
            if t.blocks.contains_key(&src[k]) || keep.contains(&src[k]) {
                ok = false;
                break;
            }
            lay(&mut t, &[src[k]]);
            let mut net: BTreeSet<P> = [src[k]].into_iter().collect();
            let mut dist: BTreeMap<P, i32> = [(src[k], 0)].into_iter().collect();
            let mut keep_k = keep.clone();
            for s in &sinks[1 - k] {
                for d in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0], [0, 0, 0]] {
                    keep_k.insert(plus(*s, d));
                }
            }
            for s in &sinks[k] {
                if net.contains(s) {
                    continue;
                }
                let Some(path) = route_net(&t, &net, *s, through, &keep_k) else {
                    ok = false;
                    break;
                };
                let step = |p: P, q: P| (p[0] - q[0]).abs() + (p[2] - q[2]).abs() == 1 && (p[1] - q[1]).abs() <= 1;
                let mut d = path.last().and_then(|l| net.iter().filter(|n| step(**n, *l)).filter_map(|n| dist.get(n)).min().copied()).unwrap_or(0);
                for p in path.iter().rev() {
                    d += 1;
                    dist.insert(*p, d);
                }
                if d > 14 {
                    ok = false;
                    break;
                }
                lay(&mut t, &path);
                net.extend(path);
            }
            if !ok {
                break;
            }
        }
        if ok {
            return Some((t, [plus(at(4), [0, 1, 0]), plus(at(5), [0, 1, 0])]));
        }
    }
    None
}

pub fn build(line: &str, name: &str, out: &Path) -> Result<()> {
    let Pic { mut b, outs, through, press, .. } = parse_pic(line)?;
    if press.is_empty() {
        return Err(anyhow!("the layout line carries no press network"));
    }
    let mut keep: BTreeSet<P> = BTreeSet::new();
    let mut finals: Vec<P> = Vec::new();
    // outputs grouped by the side they face: one coin per side
    let mut groups: BTreeMap<P, ([Vec<P>; 2], Vec<P>)> = BTreeMap::new();
    let (nb, nkeep, placed) = place_outputs(&b, &keep, &BTreeMap::new(), &outs, 0).ok_or_else(|| anyhow!("no room for every output's copies"))?;
    b = nb;
    keep = nkeep;
    for (_, f, copies) in &placed {
        let g = groups.entry(*f).or_default();
        for (k, (c, s)) in copies.iter().enumerate() {
            finals.push(*c);
            g.0[k].push(*s);
            g.1.push(*c);
        }
    }
    let mut pnet: BTreeSet<P> = press.iter().copied().collect();
    let mut coins_tops: Vec<[P; 2]> = Vec::new();
    for (f, (sinks, copies)) in &groups {
        let n = copies.len() as i32;
        let near = [copies.iter().map(|c| c[0]).sum::<i32>() / n + 3 * f[0], copies[0][1], copies.iter().map(|c| c[2]).sum::<i32>() / n + 3 * f[2]];
        let (t, tops) = coin_for(&b, &keep, near, sinks, &through).ok_or_else(|| anyhow!("no coin for the outputs facing {f:?}"))?;
        b = t;
        coins_tops.push(tops);
    }
    for tops in &coins_tops {
        // every coin top stays clear of the route (nothing on or over it) but the one it ends on
        let mut keep_p = keep.clone();
        for t2 in coins_tops.iter().flatten() {
            keep_p.insert(*t2);
            keep_p.insert(plus(*t2, [0, 1, 0]));
        }
        let path = route_net(&b, &pnet, tops[0], &through, &keep_p).or_else(|| route_net(&b, &pnet, tops[1], &through, &keep_p)).ok_or_else(|| anyhow!("no press route to the coin at {:?}", tops[0]))?;
        lay(&mut b, &path);
        pnet.extend(path);
        lay(&mut b, tops);
        pnet.extend(tops.iter().copied());
    }
    let crn: Vec<P> = corners(&b).into_iter().collect();
    let net: Vec<P> = pnet.into_iter().collect();
    let port = crate::rand4::port_for(&b, &net, &crn).ok_or_else(|| anyhow!("no input cell"))?;
    b.port("input", port);
    for (i, p) in finals.iter().enumerate() {
        b.port(&format!("output.{}", (b'a' + i as u8) as char), *p);
    }
    let d = b.design(name, Some(&format!("randomizer {}", finals.len())));
    std::fs::write(out, d.to_text())?;
    println!("wrote {} ({} blocks)", out.display(), d.blocks.len());
    Ok(())
}

/// Every output's copies, backtracking over each output's variants.
#[allow(clippy::type_complexity)]
fn place_outputs(b: &B, keep: &BTreeSet<P>, taken: &BTreeMap<P, usize>, outs: &[P], i: usize) -> Option<(B, BTreeSet<P>, Vec<(P, P, [(P, P); 2])>)> {
    if i == outs.len() {
        return Some((b.clone(), keep.clone(), vec![]));
    }
    let o = outs[i];
    let f = out_dir(&b.blocks[&o]);
    let vs = copies_of(b, keep, taken, o, f);
    for (t, copies, rays) in vs {
        let mut k2 = keep.clone();
        k2.extend(rays);
        let mut tk = taken.clone();
        tk.insert(copies[0].1, 0);
        tk.insert(copies[1].1, 1);
        if let Some((bb, kk, mut rest)) = place_outputs(&t, &k2, &tk, outs, i + 1) {
            rest.insert(0, (o, f, copies));
            return Some((bb, kk, rest));
        }
    }
    None
}

/// Whether `n` would stand on, or carry, a cell of the search path that reaches `c` (one path
/// never stacks on itself).
fn stacks(prev: &BTreeMap<P, P>, c: P, n: P) -> bool {
    let (up, down) = (plus(n, [0, 1, 0]), plus(n, [0, -1, 0]));
    // the corner of the step c -> n (the cell over its lower dust), if it climbs
    let corner = |a: P, b: P| -> Option<P> {
        match b[1] - a[1] {
            1 => Some([a[0], b[1], a[2]]),
            -1 => Some([b[0], a[1], b[2]]),
            _ => None,
        }
    };
    let step = corner(c, n);
    let mut cur = Some(c);
    while let Some(p) = cur {
        // a path cell over or under n; n's support on a corner of the path; the corner of this
        // step on a path cell's support (a support there would cut the climb)
        if p == up || p == down || step == Some(plus(p, [0, -1, 0])) {
            return true;
        }
        if let Some(q) = prev.get(&p) {
            if corner(*q, p) == Some(down) {
                return true;
            }
        }
        cur = prev.get(&p).copied();
    }
    false
}
