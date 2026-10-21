//! `mctas circuit display rand-search --k 2 --bound 210 --out <file>`: the backtracking layout
//! search for randomizers with k token pairs (N = 2^k outputs); its layout lines build with
//! `circuit display rand4 --pic <file>`.
//!
//! Signals: pair i has droppers 2i (the token) and 2i+1. A tap = a reader comparator on a free
//! face of a dropper (directly, or through one block), its front dust w1, and a second dust w2
//! next to w1 (same level, or one up/down): level 1 with the token, 0 without. The output for
//! bits b is a subtract comparator whose rear is a w2 of one pair's signal b_i and whose sides
//! are w2s of the OTHER signals of the remaining pairs (k = 2: one side; k = 3: both sides).
//! Every input is one comparator from its dropper, so all outputs change in the same tick.
//! Outputs are placed one by one, every partial layout checked, bounded by the box volume; a
//! complete layout also needs its press network (dust on every dropper, routed) and a free
//! straight line out of the box from every output's front.
use std::collections::HashMap;
type P = (i32, i32, i32);
fn ad(a: P, b: P) -> P { (a.0 + b.0, a.1 + b.1, a.2 + b.2) }
fn sb(a: P, b: P) -> P { (a.0 - b.0, a.1 - b.1, a.2 - b.2) }
const H: [P; 4] = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)];
#[derive(Clone, Copy, PartialEq, Debug)]
enum C { Drop(u8), Cmp(P, i8), Dust(u8), Sup, Blk }
// Cmp(f, -1) = reader, Cmp(f, k) = output k
#[derive(Clone)]
struct G { m: HashMap<P, C>, outs: Vec<(P, P, Vec<P>)>, taps: Vec<(P, P)> }
fn solid(g: &G, p: P) -> bool { matches!(g.m.get(&p), Some(C::Drop(_)) | Some(C::Sup) | Some(C::Blk)) }
fn conn(g: &G, a: P, b: P) -> bool {
    let d = sb(b, a);
    if d.0.abs() + d.2.abs() != 1 { return false; }
    if d.1 == 0 { return true; }
    if d.1.abs() != 1 { return false; }
    let (lo, hi) = if d.1 > 0 { (a, b) } else { (b, a) };
    !solid(g, (lo.0, hi.1, lo.2)) && solid(g, (hi.0, lo.1, hi.2))
}
// full validity of a grid
fn valid(g: &G) -> bool {
    for &(a, b) in &g.taps { if !conn(g, a, b) { return false; } }
    // supports
    for (p, c) in &g.m {
        if matches!(c, C::Cmp(..) | C::Dust(_)) {
            match g.m.get(&ad(*p, (0, -1, 0))) {
                Some(C::Sup) | None => {}
                Some(C::Drop(_)) | Some(C::Blk) => { if matches!(c, C::Dust(_)) { return false; } }
                _ => return false,
            }
        }
    }
    let dusts: Vec<(P, u8)> = g.m.iter().filter_map(|(p, c)| if let C::Dust(s) = c { Some((*p, *s & 15)) } else { None }).collect();
    for i in 0..dusts.len() { for j in i + 1..dusts.len() {
        if dusts[i].1 != dusts[j].1 && conn(g, dusts[i].0, dusts[j].0) { return false; }
    }}
    for (p, c) in &g.m {
        if let C::Cmp(f, k) = c {
            let intended: Vec<P> = if *k >= 0 { g.outs[*k as usize].2.clone() } else { vec![] };
            for sdir in [(f.2, 0, f.0), (-f.2, 0, -f.0)] {
                let q = ad(*p, sdir);
                if intended.contains(&q) { continue; }
                match g.m.get(&q) { Some(C::Dust(_)) => return false, Some(C::Cmp(f2, _)) if ad(q, *f2) == *p => return false, _ => {} }
            }
            // rear of an output must be its intended dust; of a reader its dropper / block
            let r = sb(*p, *f);
            if *k >= 0 { if !matches!(g.m.get(&r), Some(C::Dust(_))) { return false; } }
            // front: output -> free (checked at the end), reader -> its w1 dust
            if *k < 0 { if !matches!(g.m.get(&ad(*p, *f)), Some(C::Dust(_))) { return false; } }
            if *k >= 0 { if g.m.contains_key(&ad(*p, *f)) { return false; } }
        }
    }
    // dust shapes: never pointing into a dropper or a through block
    for &(p, _) in &dusts {
        let cn: Vec<P> = H.iter().copied().filter(|&h| { let q = ad(p, h); matches!(g.m.get(&q), Some(C::Cmp(..))) || dusts.iter().any(|&(d2, _)| (d2.0, d2.2) == (q.0, q.2) && (d2.1 - p.1).abs() <= 1 && d2 != p && conn(g, p, d2)) }).collect();
        for &h in &H {
            if let Some(C::Drop(_)) | Some(C::Blk) = g.m.get(&ad(p, h)) {
                let points = match cn.len() { 0 => true, 1 => cn[0] == h || cn[0] == (-h.0, 0, -h.2), _ => cn.contains(&h) };
                if points { return false; }
            }
        }
    }
    true
}
fn bbox(g: &G) -> (P, P) {
    let (mut lo, mut hi) = ((99, 99, 99), (-99, -99, -99));
    for (p, c) in &g.m {
        let q = if matches!(c, C::Cmp(..) | C::Dust(_)) { vec![*p, ad(*p, (0, -1, 0))] } else { vec![*p] };
        for p in q { lo = (lo.0.min(p.0), lo.1.min(p.1), lo.2.min(p.2)); hi = (hi.0.max(p.0), hi.1.max(p.1), hi.2.max(p.2)); }
    }
    (lo, hi)
}
fn vol(g: &G) -> i32 { let (lo, hi) = bbox(g); (hi.0 - lo.0 + 1) * (hi.1 - lo.1 + 1) * (hi.2 - lo.2 + 1) }
// a tap putting a dust of signal s at `cell`: depth 0 = the reader's front dust (level 2 with the
// token, 1 without), depth 1 = the second dust next to it (1 or 0). Dust codes carry the depth
// in bit 4 (`s | depth << 4`); the signal is `code & 15`.
fn taps_to(g: &G, drops: &[P], s: u8, cell: P, depth: u8) -> Vec<Vec<(P, C)>> {
    let mut v = Vec::new();
    let d = drops[s as usize];
    for &u in &H { for th in [false, true] {
        let rd = if th { ad(ad(d, u), u) } else { ad(d, u) };
        let w1 = ad(rd, u);
        let blk = if th { Some(ad(d, u)) } else { None };
        let mut cells = vec![(rd, C::Cmp(u, -1)), (w1, C::Dust(s))];
        if depth == 0 {
            if w1 != cell { continue; }
        } else {
            let dd = sb(cell, w1);
            if !(dd.0.abs() + dd.2.abs() == 1 && dd.1.abs() <= 1) || cell == rd { continue; }
            cells.push((cell, C::Dust(s | 16)));
        }
        if let Some(b) = blk { cells.push((b, C::Blk)); }
        if cells.iter().all(|(p, c)| g.m.get(p).map_or(true, |e| e == c)) { v.push(cells); }
    }}
    v
}
fn place(g: &G, cells: &[(P, C)]) -> Option<G> {
    let mut g2 = g.clone();
    for (p, c) in cells {
        match g2.m.get(p) { None => { g2.m.insert(*p, *c); } Some(e) if e == c => {} _ => return None }
    }
    for (p, c) in cells {
        if matches!(c, C::Cmp(..) | C::Dust(_)) {
            let b = ad(*p, (0, -1, 0));
            match g2.m.get(&b) { None => { g2.m.insert(b, C::Sup); } Some(C::Sup) => {} Some(C::Drop(_)) | Some(C::Blk) => { if matches!(c, C::Dust(_)) { return None; } } _ => return None }
        }
    }
    if let (C::Cmp(_, -1), Some((w2, C::Dust(_)))) = (cells[0].1, cells.get(2)) { g2.taps.push((cells[1].0, *w2)); }
    Some(g2)
}
struct S { k: usize, drops: Vec<P>, best: i32, found: Vec<(i32, usize, String)>, nodes: u64, leaves: u64, pressed: u64, all: bool }
fn rec(st: &mut S, g: &G, idx: usize) {
    st.nodes += 1;
    if st.found.len() >= 300 { return; }
    if st.nodes > 40_000_000 { return; }
    if vol(g) > st.best { return; }
    let n = 1usize << st.k;
    if idx == n {
        // fronts: every output's straight line out of the box free
        let (lo, hi) = bbox(g);
        for (o, f, _) in &g.outs {
            let mut q = ad(*o, *f);
            while q.0 >= lo.0 && q.0 <= hi.0 && q.2 >= lo.2 && q.2 <= hi.2 { if g.m.contains_key(&q) || g.m.contains_key(&ad(q, (0, 1, 0))) && false { return; } q = ad(q, *f); }
        }
        st.leaves += 1;
        let Some(g) = press(g, st.k) else { return };
        st.pressed += 1;
        let g = &g;
        let (lo, hi) = bbox(g);
        { let (lo, hi) = bbox(g); for (o, f, _) in &g.outs { let mut q = ad(*o, *f); while q.0 >= lo.0 && q.0 <= hi.0 && q.2 >= lo.2 && q.2 <= hi.2 { if g.m.contains_key(&q) { return; } q = ad(q, *f); } } }
        let v = vol(g);
        if v <= st.best {
            if !st.all { st.best = v; }
            let cells = g.m.len();
            let mut pic = String::new();
            for y in lo.1..=hi.1 { pic.push_str(&format!(" y{y}:")); for z in lo.2..=hi.2 { for x in lo.0..=hi.0 {
                let ch = match g.m.get(&(x, y, z)) { None => if g.m.get(&(x, y + 1, z)).map_or(false, |c| matches!(c, C::Cmp(..) | C::Dust(_))) { 'S' } else { '.' }, Some(C::Drop(i)) => (b'a' + *i) as char, Some(C::Sup) => 'S', Some(C::Blk) => 'X', Some(C::Dust(s)) => (b'0' + (*s & 15)) as char,
                    Some(C::Cmp(f, k)) => match (f, *k >= 0) { ((1, 0, 0), false) => '>', ((-1, 0, 0), false) => '<', ((0, 0, 1), false) => 'v', ((0, 0, -1), false) => '^', ((1, 0, 0), true) => 'E', ((-1, 0, 0), true) => 'W', ((0, 0, 1), true) => 'V', _ => 'N' } };
                pic.push(ch);
            } pic.push('|'); } }
            let line = format!("vol {v} cells {cells} lo {lo:?} drops {:?}{pic}", st.drops);
            eprintln!("{line}");
            st.found.push((v, cells, line));
        }
        return;
    }
    // output idx: bits b_i = (idx >> i) & 1; signal of pair i with bit b: 2i + b; "other" = 2i + 1 - b
    let bits: Vec<u8> = (0..st.k).map(|i| ((idx >> i) & 1) as u8).collect();
    let (lo, hi) = bbox(g);
    for rp in 0..st.k {
        let rs = 2 * rp as u8 + bits[rp];
        let sides: Vec<u8> = (0..st.k).filter(|&i| i != rp).map(|i| 2 * i as u8 + 1 - bits[i]).collect();
        // candidate rear dust cells: existing w2 of rs or any cell near
        for y in (lo.1 - 1).max(-1)..=hi.1 + 1 { for z in lo.2 - 3..=hi.2 + 3 { for x in lo.0 - 3..=hi.0 + 3 {
            let rc = (x, y, z);
            for &f in &H {
                let o = ad(rc, f);
                if g.m.contains_key(&o) { continue; }
                let perp = [(f.2, 0, f.0), (-f.2, 0, -f.0)];
                // side assignment
                let assigns: Vec<Vec<(u8, P)>> = if sides.len() == 1 { perp.iter().map(|&s| vec![(sides[0], ad(o, s))]).collect() } else { vec![vec![(sides[0], ad(o, perp[0])), (sides[1], ad(o, perp[1]))], vec![(sides[0], ad(o, perp[1])), (sides[1], ad(o, perp[0]))]] };
                for asg in assigns {
                    for d in 0..2u8 {
                    let mut need: Vec<(u8, P)> = vec![(rs, rc)];
                    need.extend(asg.iter().copied());
                    // each needed dust: existing dust of that signal, or a new tap
                    let mut partial: Vec<G> = vec![];
                    let mut g0 = g.clone();
                    g0.outs.push((o, f, asg.iter().map(|a| a.1).collect()));
                    let Some(g0) = place(&g0, &[(o, C::Cmp(f, idx as i8))]) else { continue };
                    partial.push(g0);
                    for &(s, cell) in &need {
                        let mut next = vec![];
                        for pg in &partial {
                            match pg.m.get(&cell) {
                                Some(C::Dust(t)) if *t == s | (d << 4) => next.push(pg.clone()),
                                Some(_) => {}
                                None => { for t in taps_to(pg, &st.drops, s, cell, d) { if let Some(g2) = place(pg, &t) { next.push(g2); } } }
                            }
                        }
                        partial = next;
                        if partial.is_empty() { break; }
                    }
                    for pg in partial {
                        if vol(&pg) <= st.best && valid(&pg) { rec(st, &pg, idx + 1); }
                    }
                    }
                }
            }
        }}}
    }
}
/// Run the search; returns every layout line found (each a new best or tie), smallest first.
/// `all`: every complete layout within the bound (up to 300 per dropper arrangement), not only
/// each new best (some layouts the model accepts fail on redsim: build them all and keep the
/// passing ones).
pub fn run(k: usize, best0: i32, all: bool, wide: bool) -> Vec<String> {
    // dropper arrangements: pair 0 at y 0 along x; pair 1 at y 2 (along x or z, offsets); pair 2
    // (k = 3) at y 0, 2 or 4; `wide`: every base in a window around pair 0 (pairs 1 and 2 at y 2,
    // pair 2 also at y 0)
    let window = |ys: &[i32]| -> Vec<P> { let mut v = Vec::new(); for &y in ys { for x in -4..=5 { for z in -4..=4 { v.push((x, y, z)); } } } v };
    let b1s: Vec<P> = if wide { window(&[2]) } else { vec![(0, 2, 0), (-1, 2, -1), (0, 2, -1), (1, 2, -1), (-1, 2, 0), (0, 2, 1), (-1, 2, 1)] };
    let b2s: Vec<P> = if wide { window(&[0, 2]) } else { vec![(0, 0, 2), (0, 0, -2), (2, 0, 1), (-1, 0, 1), (0, 4, 0), (0, 2, 2), (2, 2, 0)] };
    let mut configs: Vec<Vec<P>> = Vec::new();
    for &b1 in &b1s { for d1 in H {
        let mut v = vec![(0, 0, 0), (1, 0, 0), b1, ad(b1, d1)];
        if k == 3 {
            for &b2 in &b2s { for d2 in H {
                let mut w = v.clone(); w.push(b2); w.push(ad(b2, d2));
                if (0..6).any(|i| (i + 1..6).any(|j| w[i] == w[j])) { continue; }
                configs.push(w);
            }}
            continue;
        }
        if (0..4).any(|i| (i + 1..4).any(|j| v[i] == v[j])) { continue; }
        v.truncate(4);
        configs.push(v);
    }}
    let mut lines = Vec::new();
    let mut best = best0;
    for (ci, drops) in configs.iter().enumerate() {
        let mut g = G { m: HashMap::new(), outs: vec![], taps: vec![] };
        for (i, d) in drops.iter().enumerate() { g.m.insert(*d, C::Drop(i as u8)); }
        let mut st = S { k, drops: drops.clone(), best, found: vec![], nodes: 0, leaves: 0, pressed: 0, all };
        rec(&mut st, &g, 0);
        if ci % 1000 == 0 || !st.found.is_empty() { eprintln!("config {ci}/{} nodes {} leaves {} pressed {} best {}", configs.len(), st.nodes, st.leaves, st.pressed, st.best); }
        best = best.min(st.best);
        lines.extend(st.found);
    }
    lines.sort();
    lines.into_iter().map(|a| a.2).collect()
}
// The press network (as circuit-display's rand4::route): dust on every dropper of every pair,
// pair 0's tops joined to each other pair's tops by a shortest path touching no threshold dust,
// no comparator rear or side, no through block, keeping climb corners free; then an input cell on
// a horizontal face of the box. None if impossible.
fn press(g: &G, k: usize) -> Option<G> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    let mut g = g.clone();
    let signal: Vec<P> = g.m.iter().filter_map(|(p, c)| if let C::Dust(_) = c { Some(*p) } else { None }).collect();
    let mut guard: Vec<P> = Vec::new();
    for (p, c) in &g.m { if let C::Cmp(f, _) = c { guard.push(sb(*p, *f)); guard.push(ad(*p, (f.2, 0, f.0))); guard.push(ad(*p, (-f.2, 0, -f.0))); } }
    let mut corners: Vec<P> = Vec::new();
    for &(a, b) in &g.taps { if b.1 == a.1 + 1 { corners.push((a.0, b.1, a.2)); } if b.1 == a.1 - 1 { corners.push((b.0, a.1, b.2)); } }
    let near = |p: P| signal.iter().any(|s| (s.0 - p.0).abs() + (s.2 - p.2).abs() <= 1 && (s.1 - p.1).abs() <= 1);
    let mut drops: Vec<(u8, P)> = g.m.iter().filter_map(|(p, c)| if let C::Drop(i) = c { Some((*i, *p)) } else { None }).collect();
    drops.sort();
    let tops = |pair: usize| -> Vec<P> { drops.iter().filter(|(i, _)| *i as usize / 2 == pair).map(|(_, p)| ad(*p, (0, 1, 0))).collect() };
    let mut net: Vec<P> = Vec::new();
    for pair in 0..k { for t in tops(pair) {
        if g.m.contains_key(&t) || near(t) || guard.contains(&t) || corners.contains(&t) { return None; }
        g.m.insert(t, C::Dust(9));
        if pair == 0 { net.push(t); }
    }}
    for pair in 1..k {
        let goals = tops(pair);
        let fixed: BTreeSet<P> = (0..k).flat_map(|q| tops(q)).collect();
        let ok = |g: &G, p: P| -> bool {
            if g.m.contains_key(&p) || near(p) || guard.contains(&p) || corners.contains(&p) { return false; }
            let below = ad(p, (0, -1, 0));
            match g.m.get(&below) { Some(C::Sup) => true, None => !corners.contains(&below) && !guard.contains(&below) && !fixed.contains(&below), _ => false }
        };
        let mut prev: BTreeMap<P, P> = BTreeMap::new();
        let mut q: VecDeque<P> = net.iter().copied().collect();
        let mut seen: BTreeSet<P> = q.iter().copied().collect();
        let mut end = None;
        while let Some(c) = q.pop_front() {
            if goals.iter().any(|t| (t.0 - c.0).abs() + (t.2 - c.2).abs() == 1 && (t.1 - c.1).abs() <= 1) {
                // a climb onto the goal keeps its corner free
                let t = goals.iter().find(|t| (t.0 - c.0).abs() + (t.2 - c.2).abs() == 1 && (t.1 - c.1).abs() <= 1).unwrap();
                let corner = if t.1 > c.1 { (c.0, t.1, c.2) } else if t.1 < c.1 { (t.0, c.1, t.2) } else { *t };
                if t.1 == c.1 || !g.m.contains_key(&corner) { end = Some(c); break; }
            }
            for h in H { for dy in -1..=1 {
                let n = (c.0 + h.0, c.1 + dy, c.2 + h.2);
                if seen.contains(&n) || !ok(&g, n) { continue; }
                let corner = if dy > 0 { (c.0, n.1, c.2) } else { (n.0, c.1, n.2) };
                if dy != 0 && (g.m.contains_key(&corner) || corner == ad(n, (0, -1, 0))) { continue; }
                seen.insert(n); prev.insert(n, c); q.push_back(n);
            }}
        }
        let end = end?;
        let goal = *goals.iter().find(|t| (t.0 - end.0).abs() + (t.2 - end.2).abs() == 1 && (t.1 - end.1).abs() <= 1).unwrap();
        let mut chain = vec![goal];
        let mut c = end;
        loop {
            chain.push(c);
            if net.contains(&c) { break; }
            g.m.insert(c, C::Dust(9));
            let b = ad(c, (0, -1, 0));
            if !g.m.contains_key(&b) { g.m.insert(b, C::Sup); }
            net.push(c);
            match prev.get(&c) { Some(p) => c = *p, None => break }
        }
        // every climb along the chain keeps its corner (over the lower dust) free
        for w in chain.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a.1 != b.1 { let (lo, hi) = if a.1 < b.1 { (a, b) } else { (b, a) }; if g.m.contains_key(&(lo.0, hi.1, lo.2)) && !chain.contains(&(lo.0, hi.1, lo.2)) { return None; } if !solid(&g, (hi.0, lo.1, hi.2)) { return None; } }
        }
        net.extend(goals);
    }
    // the input cell: next to the network, on a horizontal face of the box
    let (lo, hi) = bbox(&g);
    for d in net.clone() {
        for o in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0)] {
            let p = ad(d, o);
            if g.m.contains_key(&p) || guard.contains(&p) || near(p) || corners.contains(&p) { continue; }
            if (p.0 == lo.0 || p.0 == hi.0 || p.2 == lo.2 || p.2 == hi.2) && p.1 >= lo.1 && p.1 <= hi.1 { return Some(g); }
        }
    }
    None
}
