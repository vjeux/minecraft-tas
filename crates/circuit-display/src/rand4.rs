//! `mctas circuit display rand4 --out <file>`: the randomizer-4 decoder found by the 3D layout
//! search (two token pairs: coin A on the ground, coin B two blocks up and across; each output a
//! subtract comparator at height 1 between a threshold dust of each coin). Kept here until the
//! random study has a generator crate of its own.

use crate::abuild::{B, D, P};
use anyhow::Result;
use std::path::Path;

const TOKEN: &str = "{Items:[{Slot:0b,id:\"minecraft:iron_shovel\",count:1}]}";
const JUNK: &str = "{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}";

pub fn build(out: &Path) -> Result<()> {
    let mut b = B::default();
    // coin A: A0 (token) and A1 face each other at y = 0; coin B: B0 (token) and B1 at y = 2
    for (p, facing, items) in [([0, 0, 0], "east", TOKEN), ([1, 0, 0], "west", JUNK), ([-1, 2, -1], "north", TOKEN), ([-1, 2, -2], "south", JUNK)] {
        b.set(p, &format!("dropper[facing={facing},triggered=false]"));
        b.nbt.insert(p, items.to_string());
    }
    // readers (through a block where the search put one) and their two threshold dusts
    b.stone([0, 0, -1]);
    b.comp([0, 0, -2], D::N, false);
    b.dust([0, 0, -3]);
    b.dust([1, 1, -3]);
    b.stone([1, 0, 1]);
    b.comp([1, 0, 2], D::S, false);
    b.dust([1, 0, 3]);
    b.dust([0, 1, 3]);
    b.stone([0, 2, -1]);
    b.comp([1, 2, -1], D::E, false);
    b.dust([2, 2, -1]);
    b.dust([2, 1, -2]);
    b.comp([-1, 2, 0], D::S, false);
    b.dust([-1, 2, 1]);
    b.dust([-1, 1, 2]);
    // outputs: rear one coin's second dust, side the other's
    b.comp([2, 1, -3], D::E, true); // A0 - B0 = o_01
    b.comp([1, 1, -2], D::W, true); // B0 - A0 = o_10
    b.comp([0, 1, 2], D::E, true); // B0 - A1 = o_00
    b.comp([-1, 1, 3], D::W, true); // A1 - B0 = o_11
    // the press: dust on all four droppers, joined into one network by the router
    let corners = [[0, 1, -3], [2, 2, -2], [-1, 2, 2], [1, 1, 3]];
    let through = [[0, 0, -1], [1, 0, 1], [0, 2, -1]];
    let starts = [[0, 1, 0], [1, 1, 0]];
    let goals = [[-1, 3, -1], [-1, 3, -2]];
    b.supports();
    let (path, port) = route(&b, &starts, &goals, &corners, &through).ok_or_else(|| anyhow::anyhow!("no route for the press"))?;
    for p in starts.iter().chain(goals.iter()).chain(path.iter()) {
        b.dust(*p);
    }
    b.port("input", port);
    b.port("output.a", [2, 1, -3]);
    b.port("output.b", [1, 1, -2]);
    b.port("output.c", [0, 1, 2]);
    b.port("output.d", [-1, 1, 3]);
    let d = b.design("rand4-plus", Some("randomizer 4"));
    std::fs::write(out, d.to_text())?;
    println!("wrote {} ({} blocks)", out.display(), d.blocks.len());
    Ok(())
}

fn plus(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// Comparator cells that a dust must not touch: every comparator's rear and both sides.
fn guarded(b: &B) -> Vec<P> {
    let mut v = Vec::new();
    for (p, s) in &b.blocks {
        if !s.contains("comparator") && !s.contains("repeater") {
            continue;
        }
        let f = s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
        let back = match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "west" => [-1, 0, 0],
            _ => [1, 0, 0],
        };
        v.push(plus(*p, back));
        v.push(plus(*p, [back[2], 0, back[0]]));
        v.push(plus(*p, [-back[2], 0, -back[0]]));
    }
    v
}

/// The press network: the shortest dust path joining a start (dust on a coin A dropper) to a goal
/// (dust on a coin B dropper) that touches no other dust (not even diagonally), no comparator's
/// rear or side, never stands on a through block, and keeps the corner cells of the threshold
/// dusts' climbs free; then an input cell next to it on a horizontal face of the box.
fn route(b: &B, starts: &[P], goals: &[P], corners: &[P], through: &[P]) -> Option<(Vec<P>, P)> {
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
    let signal: Vec<P> = b.blocks.iter().filter(|(_, s)| s.contains("redstone_wire")).map(|(p, _)| *p).collect();
    let guard = guarded(b);
    let near_signal = |p: P| signal.iter().any(|s| (s[0] - p[0]).abs() + (s[2] - p[2]).abs() <= 1 && (s[1] - p[1]).abs() <= 1);
    let fixed: BTreeSet<P> = starts.iter().chain(goals.iter()).copied().collect();
    let ok = |p: P| -> bool {
        if b.blocks.contains_key(&p) || near_signal(p) || guard.contains(&p) || corners.contains(&p) {
            return false;
        }
        let below = plus(p, [0, -1, 0]);
        match b.blocks.get(&below) {
            Some(s) => s == "minecraft:stone" && !through.contains(&below),
            None => !corners.contains(&below) && !guard.contains(&below) && !fixed.contains(&below),
        }
    };
    let solid_after = |p: P, path_sup: &BTreeSet<P>| b.blocks.get(&p).is_some_and(|s| !s.contains("redstone_wire") && !s.contains("comparator") && !s.contains("repeater")) || path_sup.contains(&p);
    let mut prev: BTreeMap<P, P> = BTreeMap::new();
    let mut q: VecDeque<P> = starts.iter().copied().collect();
    let mut seen: BTreeSet<P> = q.iter().copied().collect();
    let mut end = None;
    while let Some(c) = q.pop_front() {
        if goals.iter().any(|g| (g[0] - c[0]).abs() + (g[2] - c[2]).abs() == 1 && (g[1] - c[1]).abs() <= 1) && !starts.contains(&c) {
            end = Some(c);
            break;
        }
        for h in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            for dy in -1..=1 {
                let n = [c[0] + h[0], c[1] + dy, c[2] + h[2]];
                if seen.contains(&n) || !ok(n) {
                    continue;
                }
                // a climb needs its corner (over the lower dust) free
                let corner = if dy > 0 { [c[0], n[1], c[2]] } else if dy < 0 { [n[0], c[1], n[2]] } else { n };
                if dy != 0 && (b.blocks.contains_key(&corner) || corner == plus(n, [0, -1, 0])) {
                    continue;
                }
                seen.insert(n);
                prev.insert(n, c);
                q.push_back(n);
            }
        }
    }
    let mut c = end?;
    let mut path = vec![c];
    while let Some(p) = prev.get(&c) {
        if starts.contains(p) {
            break;
        }
        c = *p;
        path.push(c);
    }
    // the last step onto a goal must be legal too (its climb corner free)
    let sup: BTreeSet<P> = path.iter().map(|p| plus(*p, [0, -1, 0])).filter(|p| !b.blocks.contains_key(p)).collect();
    let mut all: Vec<P> = path.clone();
    all.extend(starts.iter().chain(goals.iter()).copied());
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in b.blocks.keys().chain(all.iter()).chain(sup.iter()) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let used: BTreeSet<P> = b.blocks.keys().chain(all.iter()).chain(sup.iter()).copied().collect();
    let mut best: Option<P> = None;
    for d in &all {
        for o in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
            let p = plus(*d, o);
            if used.contains(&p) || corners.contains(&p) || guard.contains(&p) || near_signal(p) || solid_after(p, &sup) {
                continue;
            }
            let edge = p[0] == lo[0] || p[0] == hi[0] || p[2] == lo[2] || p[2] == hi[2];
            if edge && (p[1] >= lo[1] && p[1] <= hi[1]) && best.is_none() {
                best = Some(p);
            }
        }
    }
    Some((path, best?))
}

/// Build a randomizer from a layout line of the one-off layout search: `lo (x, y, z)` then
/// ` y<N>:` layers of `|`-separated rows (z rising), one character per x: droppers `A`.. (pairs
/// ab, cd, ef, or AB, CD: the first of a pair holds the token), readers `<>^v` and outputs `NEWV` by output
/// direction, dusts `0`-`9`, stone `S`, through blocks `X`. The press network is routed.
pub fn from_pic(line: &str, name: &str, out: &Path) -> Result<()> {
    let lo: Vec<i32> = line.split("lo (").nth(1).and_then(|r| r.split(')').next()).ok_or_else(|| anyhow::anyhow!("no lo"))?.split(',').map(|s| s.trim().parse().unwrap_or(0)).collect();
    let mut b = B::default();
    let mut drops: Vec<(usize, P)> = Vec::new();
    let mut outs: Vec<P> = Vec::new();
    let mut through: Vec<P> = Vec::new();
    let mut press: Vec<P> = Vec::new();
    for seg in line.split(" y").skip(1) {
        let Some((ys, rows)) = seg.split_once(':') else { continue };
        let Ok(y) = ys.parse::<i32>() else { continue };
        for (dz, row) in rows.split('|').enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                let p = [lo[0] + dx as i32, y, lo[2] + dz as i32];
                match ch {
                    'a'..='f' => drops.push(((ch as u8 - b'a') as usize, p)),
                    'A'..='D' => drops.push(((ch as u8 - b'A') as usize, p)),
                    '<' => b.comp(p, D::W, false),
                    '>' => b.comp(p, D::E, false),
                    '^' => b.comp(p, D::N, false),
                    'v' => b.comp(p, D::S, false),
                    'N' | 'E' | 'W' | 'V' => {
                        b.comp(p, match ch { 'N' => D::N, 'E' => D::E, 'W' => D::W, _ => D::S }, true);
                        outs.push(p);
                    }
                    '9' => {
                        b.dust(p);
                        press.push(p);
                    }
                    '0'..='8' => b.dust(p),
                    'S' => b.stone(p),
                    'X' => {
                        b.stone(p);
                        through.push(p);
                    }
                    _ => {}
                }
            }
        }
    }
    drops.sort();
    for (i, p) in &drops {
        let partner = drops.iter().find(|(j, _)| *j == (i ^ 1)).map(|(_, q)| *q).ok_or_else(|| anyhow::anyhow!("dropper {i} has no partner"))?;
        let d = [partner[0] - p[0], partner[1] - p[1], partner[2] - p[2]];
        let facing = match d { [1, 0, 0] => "east", [-1, 0, 0] => "west", [0, 0, 1] => "south", [0, 0, -1] => "north", [0, 1, 0] => "up", _ => "down" };
        b.set(*p, &format!("dropper[facing={facing},triggered=false]"));
        b.nbt.insert(*p, if i % 2 == 0 { TOKEN } else { JUNK }.to_string());
    }
    b.supports();
    // the threshold dusts' climbs keep their corner cell free
    let dusts: Vec<P> = b.blocks.iter().filter(|(_, s)| s.contains("redstone_wire")).map(|(p, _)| *p).collect();
    let mut corners = Vec::new();
    for a in &dusts {
        for c in &dusts {
            if c[1] == a[1] + 1 && (c[0] - a[0]).abs() + (c[2] - a[2]).abs() == 1 {
                corners.push([a[0], c[1], a[2]]);
            }
        }
    }
    // pair 0's tops start the press, every other pair's tops are goals, joined pair by pair
    let tops = |k: usize| -> Vec<P> { drops.iter().filter(|(i, _)| i / 2 == k).map(|(_, p)| plus(*p, [0, 1, 0])).collect() };
    let pairs = drops.len() / 2;
    let mut net: Vec<P> = tops(0);
    let mut port = None;
    // a layout line that carries its press network (`9`) only needs an input cell
    if !press.is_empty() {
        port = port_for(&b, &press, &corners);
    }
    for k in (if press.is_empty() { 1 } else { pairs })..pairs {
        let goals = tops(k);
        let (path, pt) = route(&b, &net, &goals, &corners, &through).ok_or_else(|| anyhow::anyhow!("no route to pair {k}"))?;
        for p in net.iter().chain(goals.iter()).chain(path.iter()) {
            if !b.blocks.contains_key(p) {
                b.dust(*p);
            }
        }
        b.supports();
        net.extend(goals);
        net.extend(path);
        port = Some(pt);
    }
    let port = port.ok_or_else(|| anyhow::anyhow!("one pair: nothing to route"))?;
    b.port("input", port);
    for (i, o) in outs.iter().enumerate() {
        b.port(&format!("output.{}", (b'a' + i as u8) as char), *o);
    }
    let d = b.design(name, Some(&format!("randomizer {}", outs.len())));
    std::fs::write(out, d.to_text())?;
    println!("wrote {} ({} blocks)", out.display(), d.blocks.len());
    Ok(())
}

/// An input cell for a press network already in the design: next to it, on a horizontal face of
/// the box, touching no other dust and no comparator's rear or side.
fn port_for(b: &B, net: &[P], corners: &[P]) -> Option<P> {
    let signal: Vec<P> = b.blocks.iter().filter(|(p, s)| s.contains("redstone_wire") && !net.contains(p)).map(|(p, _)| *p).collect();
    let guard = guarded(b);
    let near_signal = |p: P| signal.iter().any(|s| (s[0] - p[0]).abs() + (s[2] - p[2]).abs() <= 1 && (s[1] - p[1]).abs() <= 1);
    let (mut lo, mut hi) = ([i32::MAX; 3], [i32::MIN; 3]);
    for p in b.blocks.keys() {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    for d in net {
        for o in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0]] {
            let p = plus(*d, o);
            if b.blocks.contains_key(&p) || corners.contains(&p) || guard.contains(&p) || near_signal(p) {
                continue;
            }
            let edge = p[0] == lo[0] || p[0] == hi[0] || p[2] == lo[2] || p[2] == hi[2];
            if edge && p[1] >= lo[1] && p[1] <= hi[1] {
                return Some(p);
            }
        }
    }
    None
}
