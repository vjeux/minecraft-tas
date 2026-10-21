//! The ring stack: segment s is the OR of 4 subtract comparators facing into its output cell
//! (the judge's lamp). Comparator k reads, at its rear, a function F_k(a1, a0) and, at its two
//! sides, the ring's corner dusts (an a3 literal and an a2 literal): out = F_k AND NOT side AND NOT
//! side = F_k on the row the corners select. The corners (NW, NE, SE, SW = a3, a2', a3', a2') touch
//! four torch towers; every rear is a block powered by a dust line that touches the tower of its
//! function. A torch tower (block, torch, block, torch...) carries a signal up, its polarity
//! flipping every 2 levels: with rings every 4 levels each tower shows the same value at every ring.
//! Towers start from the inputs (the input cell under the tower's first torch) or from a base block
//! that a dust line touching other towers powers (an OR of their values: an OR of literals, an AND
//! one level of torch later, an XOR from the two AND towers).

use anyhow::{Result, anyhow, bail};
use circuit::compute::SEGMENTS;
use mctas::design::Design;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

type P = [i32; 3];
const H: [[i32; 2]; 4] = [[0, -1], [0, 1], [-1, 0], [1, 0]];

#[derive(Clone, Copy, PartialEq, Debug)]
enum K {
    Stone,
    /// A comparator's rear block (pointed into only by its own feed line).
    Target,
    Dust,
    /// A repeater in a line (facing = its rear).
    Rep(&'static str),
    Corner,
    TBlock(usize),
    TTorch(usize),
    Comp(&'static str),
    RBlock,
    Lamp,
    /// Air that must stay air.
    Keep,
    /// A planned tower column, not built yet: no dust next to it.
    Reserve,
}

#[derive(Clone, Debug)]
struct Tower {
    x: i32,
    z: i32,
    b: i32,
    top: i32,
    s: u16,
    label: String,
    uses: Vec<i32>,
}

impl Tower {
    fn value(&self, l: i32) -> u16 {
        let o = (l - self.b).rem_euclid(4);
        if o == 0 || o == 3 { self.s } else { !self.s }
    }
}

fn add(p: P, h: [i32; 2]) -> P {
    [p[0] + h[0], p[1], p[2] + h[1]]
}

/// A repeater at `c` taking its input from `from`: `facing` names its rear.
fn facing(from: P, c: P) -> &'static str {
    match (from[0] - c[0], from[2] - c[2]) {
        (0, -1) => "north",
        (0, 1) => "south",
        (-1, 0) => "west",
        _ => "east",
    }
}

/// Where a line (index 0 at the target, the last cell touching its tower at 15) needs repeaters:
/// at straight cells, before its power runs out; None when a run cannot be bridged.
fn repeaters(path: &[P]) -> Option<Vec<usize>> {
    let last = path.len() - 1;
    let straight = |i: usize| {
        let (a, b) = (path[i - 1], path[i + 1]);
        a[0] == b[0] || a[2] == b[2]
    };
    let mut reps = Vec::new();
    let mut incoming = 15;
    for i in (0..last).rev() {
        if i >= 1 && incoming - 1 <= 3 && straight(i) {
            reps.push(i);
            incoming = 15;
            continue;
        }
        let p = incoming - 1;
        if p < 1 {
            return None;
        }
        incoming = p;
    }
    Some(reps)
}
fn up(p: P, d: i32) -> P {
    [p[0], p[1] + d, p[2]]
}
fn lit(k: u32) -> u16 {
    (0..16u32).filter(|v| v >> k & 1 == 1).fold(0, |m, v| m | 1 << v)
}
fn seg_mask(c: char) -> u16 {
    (0..16).filter(|&v| SEGMENTS[v].contains(c)).fold(0, |m, v| m | 1 << v)
}

struct World {
    cells: HashMap<P, K>,
    towers: Vec<Tower>,
    radius: i32,
    max_line: usize,
    budget: usize,
    b_max: i32,
    spacing: i32,
    planned: Vec<(i32, i32)>,
    /// Only bases of this parity (a copy keeps or flips its source's phase pattern).
    parity: Option<i32>,
    /// Longest line with repeaters (cells).
    reach: usize,
}

impl World {
    fn get(&self, p: P) -> Option<K> {
        self.cells.get(&p).copied()
    }

    /// Reserve a tower column from b to top; the cell at b (its base) is a block.
    fn put_tower(&mut self, x: i32, z: i32, b: i32, top: i32, s: u16, label: &str) -> Option<usize> {
        for l in b - 1..=top + 1 {
            let p = [x, l, z];
            if l >= b && l <= top && self.cells.contains_key(&p) {
                return None;
            }
            if matches!(self.get(p), Some(K::Dust | K::Corner)) {
                return None;
            }
            if l >= b && l <= top && H.iter().any(|&h| matches!(self.get(add(p, h)), Some(K::Dust | K::Rep(_) | K::Corner | K::Comp(_) | K::Lamp))) {
                return None;
            }
        }
        let id = self.towers.len();
        self.towers.push(Tower { x, z, b, top, s, label: label.to_string(), uses: Vec::new() });
        for l in b..=top {
            self.cells.insert([x, l, z], if (l - b) % 2 == 0 { K::TBlock(id) } else { K::TTorch(id) });
        }
        Some(id)
    }

    fn remove_tower(&mut self, id: usize) {
        let t = self.towers.pop().unwrap();
        assert_eq!(id, self.towers.len());
        for l in t.b..=t.top {
            self.cells.remove(&[t.x, l, t.z]);
        }
    }

    fn trim(&mut self, id: usize, top: i32) {
        let t = self.towers[id].clone();
        for l in top + 1..=t.top {
            self.cells.remove(&[t.x, l, t.z]);
        }
        self.cells.entry([t.x, top + 1, t.z]).or_insert(K::Keep);
        self.towers[id].top = top;
    }

    fn reserve(&mut self, c: (i32, i32)) {
        for l in -1..=200 {
            self.cells.entry([c.0, l, c.1]).or_insert(K::Reserve);
        }
        self.planned.push(c);
    }

    fn unreserve(&mut self, c: (i32, i32)) {
        for l in -1..=200 {
            if self.get([c.0, l, c.1]) == Some(K::Reserve) {
                self.cells.remove(&[c.0, l, c.1]);
            }
        }
    }

    fn ascii(&self, l: i32) -> String {
        let r = self.radius + 1;
        let mut s = format!("level {l}:\n");
        for z in -r..=r {
            for x in -r..=r {
                let ch = match self.get([x, l, z]) {
                    None => '.',
                    Some(K::Stone) => 's',
                    Some(K::Target) => 'R',
                    Some(K::Dust) => '+',
                    Some(K::Rep(_)) => '>',
                    Some(K::Corner) => 'c',
                    Some(K::TBlock(_)) => '#',
                    Some(K::TTorch(_)) => 'i',
                    Some(K::Comp(_)) => 'C',
                    Some(K::RBlock) => 'X',
                    Some(K::Lamp) => 'L',
                    Some(K::Keep) => ',',
                    Some(K::Reserve) => 'o',
                };
                s.push(ch);
            }
            s.push('\n');
        }
        s
    }

    fn tower_at(&self, p: P) -> Option<usize> {
        match self.get(p) {
            Some(K::TBlock(i) | K::TTorch(i)) => Some(i),
            _ => None,
        }
    }

    /// Can dust go at c (after q in the path being built), touching only the towers in `touch`?
    fn dust_ok(&self, c: P, q: Option<P>, target: P, first: bool, path: &HashSet<P>, touch: &[usize]) -> bool {
        if self.cells.contains_key(&c) || path.contains(&c) {
            return false;
        }
        if c[0].abs() > self.radius + 1 || c[2].abs() > self.radius + 1 {
            return false;
        }
        let below = up(c, -1);
        match self.get(below) {
            None | Some(K::Stone) => {}
            _ => return false,
        }
        if self.get(below).is_none() && H.iter().any(|&h| matches!(self.get(add(below, h)), Some(K::Lamp | K::Comp(_)))) {
            return false;
        }
        match self.get(up(c, 1)) {
            None | Some(K::Stone | K::Keep | K::Target) => {}
            _ => return false,
        }
        for &h in &H {
            let n = add(c, h);
            if Some(n) == q || (first && n == target) {
                continue;
            }
            if path.contains(&n) {
                return false;
            }
            match self.get(n) {
                None | Some(K::Stone | K::Target | K::Keep) => {}
                Some(K::TBlock(i) | K::TTorch(i)) if touch.contains(&i) => {}
                _ => return false,
            }
            for d in [-1, 1] {
                if matches!(self.get(up(n, d)), Some(K::Dust | K::Corner)) {
                    return false;
                }
            }
        }
        true
    }

    /// Where each dust cell of a finished path points (the 1.16+ shape rule).
    fn points(&self, path: &[P], i: usize) -> Vec<P> {
        let c = path[i];
        let mut conns: Vec<[i32; 2]> = Vec::new();
        for &h in &H {
            let n = add(c, h);
            let in_path = (i > 0 && path[i - 1] == n) || (i + 1 < path.len() && path[i + 1] == n);
            if in_path || matches!(self.get(n), Some(K::TTorch(_) | K::Comp(_) | K::RBlock | K::Dust | K::Corner)) {
                conns.push(h);
            }
        }
        if conns.len() == 1 {
            let h = conns[0];
            conns.push([-h[0], -h[1]]);
        }
        if conns.is_empty() {
            conns = H.to_vec();
        }
        conns.into_iter().map(|h| add(c, h)).collect()
    }

    fn path_ok(&self, path: &[P], target: P, reps: &[usize]) -> bool {
        for i in 0..path.len() {
            if reps.contains(&i) {
                continue;
            }
            for n in self.points(path, i) {
                if n == target {
                    continue;
                }
                if matches!(self.get(n), Some(K::Stone | K::Target | K::TBlock(_) | K::RBlock)) {
                    return false;
                }
            }
        }
        self.points(path, 0).contains(&target)
    }

    /// A dust line at the target's level, ending in the target block (pointing straight in) and
    /// touching every tower in `touch`.
    fn route(&mut self, target: P, touch: &[usize]) -> Option<Vec<P>> {
        let l = target[1];
        if touch.iter().any(|&i| self.towers[i].b >= l || self.towers[i].top < l) {
            return None;
        }
        let cols: Vec<P> = touch.iter().map(|&i| [self.towers[i].x, l, self.towers[i].z]).collect();
        let mut best: Option<(Vec<P>, Vec<usize>)> = None;
        if touch.len() == 1 {
            best = self.bfs(target, touch, cols[0]);
        }
        let skip_dfs = best.is_some();
        for maxlen in [5, 9, 12, self.max_line].into_iter().filter(|&m| m <= self.max_line && !skip_dfs) {
            for &h in &H {
                let e = add(target, h);
                let p = add(e, h);
                let mut path_set = HashSet::new();
                if !self.dust_ok(e, None, target, true, &path_set, touch) {
                    continue;
                }
                path_set.insert(e);
                if !self.dust_ok(p, Some(e), target, false, &path_set, touch) {
                    continue;
                }
                path_set.insert(p);
                let mut path = vec![e, p];
                let mut budget = self.budget;
                if self.dfs(&mut path, &mut path_set, target, touch, &cols, maxlen, &mut budget) {
                    best = Some((path, Vec::new()));
                    break;
                }
            }
            if best.is_some() {
                break;
            }
        }
        let (path, reps) = best?;
        let mut added = Vec::new();
        for (i, &c) in path.iter().enumerate() {
            let kind = if reps.contains(&i) { K::Rep(facing(path[i + 1], c)) } else { K::Dust };
            self.cells.insert(c, kind);
            added.push(c);
            if !self.cells.contains_key(&up(c, -1)) {
                self.cells.insert(up(c, -1), K::Stone);
                added.push(up(c, -1));
            }
        }
        for &i in touch {
            self.towers[i].uses.push(l);
        }
        Some(added)
    }

    fn unroute(&mut self, added: &[P], touch: &[usize]) {
        for c in added {
            self.cells.remove(c);
        }
        for &i in touch {
            self.towers[i].uses.pop();
        }
    }

    /// Shortest lines first (one tower to touch): a BFS from each straight start, every path it
    /// finds re-checked with the full rules (self-adjacency, dust shapes).
    fn bfs(&self, target: P, touch: &[usize], col: P) -> Option<(Vec<P>, Vec<usize>)> {
        let mut found: Vec<Vec<P>> = Vec::new();
        for &h in &H {
            let e = add(target, h);
            let p = add(e, h);
            let start: HashSet<P> = HashSet::new();
            if !self.dust_ok(e, None, target, true, &start, touch) {
                continue;
            }
            let one: HashSet<P> = [e].into_iter().collect();
            if !self.dust_ok(p, Some(e), target, false, &one, touch) {
                continue;
            }
            let mut parent: HashMap<P, P> = HashMap::new();
            let mut depth: HashMap<P, usize> = HashMap::new();
            parent.insert(p, e);
            depth.insert(p, 2);
            let mut queue = std::collections::VecDeque::from([p]);
            while let Some(c) = queue.pop_front() {
                let dc = depth[&c];
                if H.iter().any(|&hh| add(c, hh) == col) {
                    let mut path = vec![c];
                    while let Some(&q) = parent.get(path.last().unwrap()) {
                        path.push(q);
                    }
                    path.reverse();
                    found.push(path);
                    if found.len() > 60 {
                        break;
                    }
                    continue;
                }
                if dc >= self.reach {
                    continue;
                }
                for &hh in &H {
                    let n = add(c, hh);
                    if parent.contains_key(&n) || n == e {
                        continue;
                    }
                    let prev: HashSet<P> = [parent[&c]].into_iter().collect();
                    if !self.dust_ok(n, Some(c), target, false, &prev, touch) {
                        continue;
                    }
                    parent.insert(n, c);
                    depth.insert(n, dc + 1);
                    queue.push_back(n);
                }
            }
        }
        found.sort_by_key(|p| p.len());
        'paths: for path in found {
            let mut set = HashSet::new();
            for (i, &c) in path.iter().enumerate() {
                let q = if i == 0 { None } else { Some(path[i - 1]) };
                if !self.dust_ok(c, q, target, i == 0, &set, touch) {
                    continue 'paths;
                }
                set.insert(c);
            }
            let Some(reps) = repeaters(&path) else { continue };
            if self.path_ok(&path, target, &reps) {
                return Some((path, reps));
            }
        }
        None
    }

    #[allow(clippy::too_many_arguments)]
    fn dfs(&self, path: &mut Vec<P>, set: &mut HashSet<P>, target: P, touch: &[usize], cols: &[P], maxlen: usize, budget: &mut usize) -> bool {
        let all = cols.iter().all(|t| path.iter().any(|c| H.iter().any(|&h| add(*c, h) == *t)));
        if all && path.len() >= 2 && self.path_ok(path, target, &[]) {
            return true;
        }
        if path.len() >= maxlen || *budget == 0 {
            return false;
        }
        *budget -= 1;
        let last = *path.last().unwrap();
        let mut next: Vec<(i32, P)> = Vec::new();
        for &h in &H {
            let c = add(last, h);
            if !self.dust_ok(c, Some(last), target, false, set, touch) {
                continue;
            }
            let d = cols.iter().filter(|t| !path.iter().any(|q| H.iter().any(|&hh| add(*q, hh) == **t))).map(|t| (t[0] - c[0]).abs() + (t[2] - c[2]).abs()).min().unwrap_or(0);
            next.push((d, c));
        }
        next.sort();
        for (_, c) in next {
            path.push(c);
            set.insert(c);
            if self.dfs(path, set, target, touch, cols, maxlen, budget) {
                return true;
            }
            path.pop();
            set.remove(&c);
        }
        false
    }

    /// Candidate tower columns, nearest the center first: outside the rings' 5x5 and the 4 cells
    /// the feed lines leave by, not next to another tower.
    fn columns(&self, b: i32, top: i32, near: &[(i32, i32)]) -> Vec<(i32, i32)> {
        let r = self.radius;
        let mut out: Vec<(i32, (i32, i32))> = Vec::new();
        for x in -r..=r {
            for z in -r..=r {
                if x.abs() <= 2 && z.abs() <= 2 {
                    continue;
                }
                if (x == 0 && z.abs() == 3) || (z == 0 && x.abs() == 3) {
                    continue;
                }
                let next_to = self.towers.iter().any(|t| (t.x - x).abs() + (t.z - z).abs() <= self.spacing - 1 && t.top >= b && t.b <= top)
                    || self.planned.iter().any(|&(a, c)| (a - x).abs() + (c - z).abs() <= self.spacing - 1);
                if next_to {
                    continue;
                }
                let pinwheel = [(-1, -2), (2, -1), (1, 2), (-2, 1)];
                if pinwheel.iter().any(|&(a, c)| (a - x).abs() + (c - z).abs() <= 1) {
                    continue;
                }
                // a column a built line already passes through or beside cannot take a tower
                let blocked = (-2..=200).any(|l| {
                    let p = [x, l, z];
                    matches!(self.get(p), Some(k) if k != K::Reserve && k != K::Keep)
                        || matches!(self.get(p), Some(K::Keep) if l >= 0)
                        || H.iter().any(|&h| matches!(self.get(add(p, h)), Some(K::Dust | K::Rep(_) | K::Corner)))
                });
                if blocked {
                    continue;
                }
                let d = near.iter().map(|&(a, c)| (a - x).abs() + (c - z).abs()).sum::<i32>() + x.abs().max(z.abs());
                out.push((d, (x, z)));
            }
        }
        out.sort();
        out.into_iter().map(|(_, c)| c).collect()
    }

    /// A tower showing `f` at every ring level (levels = 0 mod 4), fed by a line at its base level.
    fn make(&mut self, f: u16, label: &str, top: i32, fixed: Option<(i32, i32)>, prefer: &[(i32, i32)]) -> Result<usize> {
        for b in 1i32..self.b_max {
            if self.parity.is_some_and(|p| b.rem_euclid(2) != p) {
                continue;
            }
            let s = if (-b).rem_euclid(4) == 0 || (-b).rem_euclid(4) == 3 { f } else { !f };
            // a tower's base block is only dust-powered (weak): it powers no dust beside it, so a
            // source is touched above its base
            let avail: Vec<usize> = (0..self.towers.len()).filter(|&i| self.towers[i].b < b && self.towers[i].top >= b).collect();
            let mut subsets: Vec<Vec<usize>> = avail.iter().filter(|&&i| self.towers[i].value(b) == s).map(|&i| vec![i]).collect();
            for (ai, &i) in avail.iter().enumerate() {
                for &j in &avail[ai + 1..] {
                    if self.towers[i].value(b) | self.towers[j].value(b) == s && self.towers[i].value(b) != s && self.towers[j].value(b) != s {
                        subsets.push(vec![i, j]);
                    }
                }
            }
            if let Some(&(px, pz)) = prefer.first() {
                let dist = |sub: &Vec<usize>| sub.iter().map(|&i| (self.towers[i].x - px).abs() + (self.towers[i].z - pz).abs()).max().unwrap_or(0);
                subsets.sort_by_key(|s| dist(s));
            }
            for sub in subsets {
                let mut near: Vec<(i32, i32)> = sub.iter().map(|&i| (self.towers[i].x, self.towers[i].z)).collect();
                if !prefer.is_empty() {
                    near = prefer.to_vec();
                }
                let cols = match fixed {
                    Some(c) => {
                        self.unreserve(c);
                        vec![c]
                    }
                    None => self.columns(b, top, &near),
                };
                for (x, z) in cols.into_iter().take(if prefer.is_empty() { 40 } else { 6 }) {
                    let Some(id) = self.put_tower(x, z, b, top, s, label) else {
                        if std::env::var("DISPLAY_DEBUG").is_ok() {
                            eprintln!("  {label}: column ({x},{z}) base {b} blocked");
                        }
                        continue;
                    };
                    if std::env::var("DISPLAY_DEBUG").is_ok() {
                        eprintln!("  {label}: try ({x},{z}) base {b} from {sub:?}");
                    }
                    let mut ok = self.route([x, b, z], &sub).is_some();
                    if !ok && sub.len() == 2 {
                        // one line per source, into two sides of the base
                        if let Some(first) = self.route([x, b, z], &sub[..1]) {
                            ok = self.route([x, b, z], &sub[1..]).is_some();
                            if !ok {
                                self.unroute(&first, &sub[..1]);
                            }
                        }
                    }
                    if ok {
                        let names: Vec<String> = sub.iter().map(|&i| self.towers[i].label.clone()).collect();
                        eprintln!("tower {label} at ({x},{z}) base {b} from {}", names.join("+"));
                        return Ok(id);
                    }
                    self.remove_tower(id);
                }
            }
        }
        bail!("no tower for {label} ({f:016b})")
    }
}

const NAMES: [char; 7] = ['a', 'b', 'c', 'd', 'e', 'f', 'g'];

pub fn run(out: &Path, pitch: i32, radius: i32, max_line: usize, spacing: i32, name: &str) -> Result<()> {
    if pitch % 4 != 0 {
        bail!("--pitch must be a multiple of 4");
    }
    let mut w = World { cells: HashMap::new(), towers: Vec::new(), radius, max_line, budget: 3000, b_max: 40, spacing, planned: Vec::new(), parity: None, reach: 48 };
    let top_max = 200;
    let mut design = Design { name: name.to_string(), ..Default::default() };
    design.meta.insert("circuit".into(), "seven-segment 4".into());
    // inputs: a tower on each input cell (level -2, left air: the judge's stone; a tower based at
    // 2 mod 4 can be copied into the other ring polarity), level -1 kept clear around it for the
    // judge's repeater and switch; a3 and a2 stand at the SE and NE corners (an input tower shows
    // its input inverted at the rings: a3', a2')
    let input_cols = [(1, -12), (-7, -12), (2, -1), (1, 2)];
    let mut inputs = Vec::new();
    for (k, &(x, z)) in input_cols.iter().enumerate() {
        let id = w.put_tower(x, z, -2, top_max, lit(k as u32), &format!("a{k}")).ok_or_else(|| anyhow!("input column"))?;
        design.ports.insert(format!("input.a{k}"), [x, -2, z]);
        inputs.push(id);
    }
    for &(x, z) in &input_cols {
        for dx in -2i32..=2 {
            for dz in -2i32..=2 {
                if dx.abs() + dz.abs() <= 2 && (dx, dz) != (0, 0) {
                    w.cells.entry([x + dx, -1, z + dz]).or_insert(K::Keep);
                }
            }
        }
    }
    // the other ring polarity of a0 and a1 (both polarities at one level), then the corners
    let a = [lit(0), lit(1), lit(2), lit(3)];
    let corner_f = [(a[3], (-1, -2), "NW a3"), (!a[2], (2, -1), "NE a2'"), (!a[3], (1, 2), "SE a3'"), (a[2], (-2, 1), "SW a2")];
    // ring k: comparators N E S W, their side corners, their rear function
    let nw = a[3];
    let ne = !a[2];
    let se = !a[3];
    let sw = a[2];
    let comps: [(&str, [i32; 2], u16, u16); 4] = [("north", [0, -1], nw, ne), ("east", [1, 0], ne, se), ("south", [0, 1], se, sw), ("west", [-1, 0], sw, nw)];
    let mut need: Vec<Vec<u16>> = Vec::new();
    for &c in &NAMES {
        let seg = seg_mask(c);
        let mut fs = Vec::new();
        let mut acc = 0u16;
        for &(_, _, s1, s2) in &comps {
            let sel = !s1 & !s2;
            let row = (0..16).find(|&v| sel >> v & 1 == 1).unwrap() & 12;
            let f = (0..16).filter(|&v| seg >> (row | (v & 3)) & 1 == 1).fold(0u16, |m, v| m | 1 << v);
            acc |= f & sel;
            fs.push(f);
        }
        if acc != seg {
            bail!("segment {c}: the rows do not cover it");
        }
        need.push(fs);
    }
    // per side of the ring (west, north, south, east): a literal pair, an a1 tower of even base
    // and an a0 tower of odd base (their phase patterns differ: at the 4 levels mod 4 they show
    // all 4 polarity pairs), then one tower per (function, side) next to it; AND / OR kinds first,
    // XOR kinds after (they read two AND towers)
    let xorish = |f: u16| {
        let c = lit(1);
        let d = lit(0);
        f == c ^ d || f == !(c ^ d)
    };
    let mut pairs: Vec<(u16, usize)> = Vec::new();
    for fs in &need {
        for (j, &f) in fs.iter().enumerate() {
            if f != 0 && f != 0xffff && !pairs.contains(&(f, j)) {
                pairs.push((f, j));
            }
        }
    }
    let side_order = [3usize, 0, 2, 1];
    pairs.sort_by_key(|&(f, j)| (side_order.iter().position(|&s| s == j), xorish(f), f));
    for &(f, c, label) in &corner_f {
        if let Some(i) = w.tower_at([c.0, 0, c.1]) {
            if w.towers[i].value(0) != f {
                bail!("corner {label}: the input there shows the wrong polarity");
            }
            w.towers[i].label = label.to_string();
            continue;
        }
        w.make(f, label, top_max, Some(c), &[])?;
    }
    let mut ftower: BTreeMap<(u16, usize), usize> = BTreeMap::new();
    // every side's function towers get their columns first, nearest their rear, reserved so that
    // no line passes beside them before they are built
    let mut todo: Vec<(u16, usize)> = Vec::new();
    for &j in &side_order {
        for &(f, _) in pairs.iter().filter(|p| p.1 == j) {
            if xorish(f) {
                for h in [0x2222u16, 0x4444] {
                    if !todo.contains(&(h, j)) {
                        todo.push((h, j));
                    }
                }
            }
            if !todo.contains(&(f, j)) {
                todo.push((f, j));
            }
        }
    }
    let mut cols: BTreeMap<(u16, usize), (i32, i32)> = BTreeMap::new();
    let _ = &mut cols;
    // fixed sectors (a pinwheel): every side has 6 function slots and 4 literal slots; all are
    // reserved before any line is routed, each tower takes the first slot of its side it routes in
    let rot = |j: usize, (x, z): (i32, i32)| match j {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    };
    let fn_slots_n = [(-3, -5), (2, -5), (0, -10), (-5, -10), (5, -10), (-10, -10)];
    let lit_slots_n = [(-7, -12), (-3, -12), (1, -12), (5, -12)];
    let relay_slots_ne = [(9, -9), (13, -12), (9, -14)];
    let rot = |j: usize, s: (i32, i32)| rot(j % 4, s);
    for j in 0..4 {
        for &s in fn_slots_n.iter() {
            let c = rot(j, s);
            if w.tower_at([c.0, 0, c.1]).is_none() {
                w.reserve(c);
            }
        }
    }
    let mut build = |w: &mut World, f: u16, label: &str, slots: Vec<(i32, i32)>| -> Result<usize> {
        for c in slots {
            if w.get([c.0, 0, c.1]) != Some(K::Reserve) {
                continue;
            }
            match w.make(f, label, top_max, Some(c), &[]) {
                Ok(id) => return Ok(id),
                Err(_) => w.reserve(c),
            }
        }
        bail!("no slot for {label}")
    };
    for &j in &[0usize, 1, 2, 3] {
        let side = &comps[j].0[..1];
        // the inputs show a1' and a0' at the rings; their flips (a1, a0) next to them (lines with
        // repeaters reach every side from there)
        if j == 0 {
            for (f, label) in [(a[1], "a1"), (a[0], "a0")] {
                w.make(f, &format!("{label}+"), top_max, None, &[(-1, -13)])?;
            }
        }
        w.parity = None;
        for &(f, jj) in todo.iter().filter(|p| p.1 == j) {
            let mut slots: Vec<(i32, i32)> = fn_slots_n.iter().map(|&s| rot(j, s)).collect();
            if xorish(f) || !need.iter().any(|fs| fs[jj] == f) {
                // a helper only an XOR reads takes the far slots
                slots.reverse();
            }
            let id = build(&mut w, f, &format!("F{f:04x}{side}"), slots)?;
            ftower.insert((f, jj), id);
        }
    }
    for c in w.planned.clone() {
        w.unreserve(c);
    }
    let bottom = w.cells.keys().filter(|p| matches!(w.cells[*p], K::Dust)).map(|p| p[1]).max().unwrap_or(0);
    let l0 = (bottom + 3 + 3) / 4 * 4;
    let ring_levels: Vec<i32> = (0..7).map(|k| l0 + pitch * k as i32).collect();
    // trim every tower to its last use
    for (k, fs) in need.iter().enumerate() {
        for (j, f) in fs.iter().enumerate() {
            if let Some(&id) = ftower.get(&(*f, j)) {
                w.towers[id].uses.push(ring_levels[k]);
            }
        }
    }
    for i in 0..w.towers.len() {
        let mut top = w.towers[i].uses.iter().copied().max().unwrap_or(w.towers[i].b);
        if w.towers[i].label.starts_with(['N', 'S']) {
            top = ring_levels[6];
        }
        w.trim(i, top);
    }
    place_rings(&mut w, &ring_levels, &comps, &need, &mut design)?;
    for (k, fs) in need.iter().enumerate() {
        let l = ring_levels[k];
        for (j, &(_, d, _, _)) in comps.iter().enumerate() {
            let f = fs[j];
            if f == 0 || f == 0xffff {
                continue;
            }
            let target = [2 * d[0], l, 2 * d[1]];
            let id = ftower[&(f, j)];
            if w.route(target, &[id]).is_none() {
                bail!("ring {} ({}): no feed line from {} to the {} rear", k, NAMES[k], w.towers[id].label, comps[j].0);
            }
        }
    }
    emit(&w, &mut design);
    let mut text = design.to_text();
    for t in &w.towers {
        text.push_str(&format!("# tower {} {} {} {} {} {:04x}\n", t.label.replace(' ', "_"), t.x, t.z, t.b, t.top, t.s));
    }
    std::fs::write(out, text)?;
    let (lo, hi) = design.bounds().unwrap();
    println!("{}: {} blocks, box {}x{}x{}", out.display(), design.block_count(), hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1);
    Ok(())
}

fn place_rings(w: &mut World, levels: &[i32], comps: &[(&'static str, [i32; 2], u16, u16); 4], need: &[Vec<u16>], design: &mut Design) -> Result<()> {
    for (k, &l) in levels.iter().enumerate() {
        let mut put = |p: P, kind: K| -> Result<()> {
            if let Some(old) = w.cells.get(&p)
                && *old != kind
            {
                bail!("ring {k}: {p:?} is {old:?}, want {kind:?}");
            }
            w.cells.insert(p, kind);
            Ok(())
        };
        put([0, l, 0], K::Lamp)?;
        put([0, l - 1, 0], K::Keep)?;
        put([0, l + 1, 0], K::Keep)?;
        for (j, &(facing, d, _, _)) in comps.iter().enumerate() {
            put([d[0], l, d[1]], K::Comp(facing))?;
            put([d[0], l - 1, d[1]], K::Stone)?;
            let f = need[k][j];
            let rear = [2 * d[0], l, 2 * d[1]];
            put(rear, if f == 0xffff { K::RBlock } else if f == 0 { K::Keep } else { K::Target })?;
        }
        for (x, z) in [(-1, -1), (1, -1), (1, 1), (-1, 1)] {
            put([x, l, z], K::Corner)?;
            put([x, l - 1, z], K::Stone)?;
        }
        design.ports.insert(format!("output.{}", NAMES[k]), [0, l, 0]);
    }
    Ok(())
}

fn emit(w: &World, design: &mut Design) {
    for (p, k) in &w.cells {
        let s = match k {
            K::Stone | K::Target => "minecraft:stone".to_string(),
            K::TBlock(i) if p[1] == w.towers[*i].b && p[1] < 0 => continue,
            K::TBlock(_) => "minecraft:stone".to_string(),
            K::TTorch(_) => "minecraft:redstone_torch".to_string(),
            K::Dust | K::Corner => "minecraft:redstone_wire".to_string(),
            K::Comp(f) => format!("minecraft:comparator[facing={f},mode=subtract]"),
            K::RBlock => "minecraft:redstone_block".to_string(),
            K::Rep(f) => format!("minecraft:repeater[facing={f},delay=1]"),
            K::Lamp | K::Keep | K::Reserve => continue,
        };
        design.blocks.insert(*p, s);
    }
}
