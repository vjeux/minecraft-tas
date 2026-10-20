//! Dropper-ring counters (`mctas circuit memory ring --n N`): the input cell fires droppers (its
//! face neighbours, and the cells a dust next to it powers); a cycle dropper -> hopper -> dropper
//! -> ... carries one stone one dropper per pulse (every transit crosses a hopper, so the order in
//! which the droppers fire never matters), and a comparator on one dropper reads a counter N in
//! the judge's level style.
//!
//! Rules the layouts obey: hoppers never face up and never sit under a container (a hopper pulls
//! from the container above it); a dust next to the input points away from it (it connects to
//! the redstone block) and powers the block beyond and its floor, a dust under the input is a dot
//! and powers its four sides and its floor; nothing that emits a signal touches the comparator's
//! sides.

use anyhow::{Result, bail};
use std::collections::BTreeSet;
use std::path::Path;

type P = [i32; 3];

const X: P = [0, 0, 0];
const UP: P = [0, 1, 0];
const DIRS: [P; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const HORIZ: [P; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];

fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dist(a: P, b: P) -> i32 {
    (a[0] - b[0]).abs() + (a[1] - b[1]).abs() + (a[2] - b[2]).abs()
}

fn dir_name(d: P) -> &'static str {
    match d {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}

/// Which cells the input fires, given dust cells next to it.
struct Layout {
    dust: Vec<P>,
    /// Stones a dust points into: powered, they fire their own neighbours (relays).
    relays: Vec<P>,
    fired: BTreeSet<P>,
    /// Cells that must be solid (dust floors).
    floors: Vec<P>,
}

/// Every choice per neighbour of the input below or beside it: nothing (a dropper may go there),
/// a dust (its floor and the cell beyond fire), or a dust pointing into a relay stone (beside
/// only: the stone's neighbours fire).
fn layouts(max_dust: usize) -> Vec<Layout> {
    let spots: Vec<P> = DIRS.iter().copied().filter(|d| *d != UP).collect();
    let opts = |p: P| if p == [0, -1, 0] { 2 } else { 3 };
    let mut out = Vec::new();
    let mut idx = vec![0usize; spots.len()];
    loop {
        let dust: Vec<P> = (0..spots.len()).filter(|&i| idx[i] > 0).map(|i| spots[i]).collect();
        if dust.len() <= max_dust {
            let mut fired: BTreeSet<P> = DIRS.iter().copied().filter(|d| !dust.contains(d)).collect();
            let mut floors = Vec::new();
            let mut relays = Vec::new();
            for i in 0..spots.len() {
                let h = spots[i];
                if idx[i] == 0 {
                    continue;
                }
                let below = add(h, [0, -1, 0]);
                floors.push(below);
                fired.insert(below);
                if h == [0, -1, 0] {
                    // under the input: a cross, powering its four sides
                    for d in HORIZ {
                        fired.insert(add(h, d));
                    }
                } else if idx[i] == 2 {
                    // next to the input, pointing into a relay stone
                    let s = add(h, sub(h, X));
                    relays.push(s);
                    for d in DIRS {
                        let q = add(s, d);
                        if q != h {
                            fired.insert(q);
                        }
                    }
                } else {
                    // next to the input: a line pointing away from it
                    fired.insert(add(h, sub(h, X)));
                }
            }
            fired.remove(&X);
            for d in dust.iter().chain(relays.iter()) {
                fired.remove(d);
            }
            out.push(Layout { dust, relays, fired, floors });
        }
        let mut i = 0;
        loop {
            if i == spots.len() {
                return out;
            }
            idx[i] += 1;
            if idx[i] < opts(spots[i]) {
                break;
            }
            idx[i] = 0;
            i += 1;
        }
    }
}

/// One counter: the ring (dropper, hopper) pairs in flow order, the dust, stone floors, and the
/// comparator (cell, the dropper it reads, its floor if added).
#[derive(Clone)]
pub struct Ring {
    pub droppers: Vec<P>,
    pub hoppers: Vec<P>,
    pub dust: Vec<P>,
    pub stone: Vec<P>,
    pub comparator: P,
    pub read: P,
    /// Copper bulb stages after the comparator (each halves the rate: counter N * 2^bulbs).
    pub bulbs: usize,
    /// Judged as a divider: the last bulb's comparator changes once every N * 2^(bulbs-1) pulses
    /// (toggles style) instead of a level with period N * 2^bulbs.
    pub divider: bool,
}

impl Ring {
    pub fn blocks(&self) -> usize {
        self.droppers.len() + self.hoppers.len() + self.dust.len() + self.stone.len() + 1 + 2 * self.bulbs
    }

    /// The counter's N.
    pub fn n(&self) -> usize {
        if self.divider && self.bulbs > 0 { self.droppers.len() << (self.bulbs - 1) } else { self.droppers.len() << self.bulbs }
    }

    /// The design text (with a caption line).
    /// The design (blocks, ports, the stone's nbt), without a caption.
    pub fn build(&self, name: &str) -> mctas::design::Design {
        let total = self.n();
        let mut d = mctas::design::Design { name: name.to_string(), ..Default::default() };
        d.meta.insert("circuit".into(), format!("counter {total}"));
        d.ports.insert("input".into(), X);
        let face = sub(self.comparator, self.read);
        let at = |k: i32| add(self.read, [face[0] * k, 0, face[2] * k]);
        d.ports.insert("output".into(), at(2 * self.bulbs as i32 + 2));
        for j in 0..self.bulbs as i32 {
            d.blocks.insert(at(2 * j + 2), "minecraft:waxed_copper_bulb[lit=false,powered=false]".into());
            d.blocks.insert(at(2 * j + 3), format!("minecraft:comparator[facing={},mode=compare,powered=false]", dir_name([-face[0], 0, -face[2]])));
        }
        put_ring(&mut d, &self.droppers, &self.hoppers);
        for p in &self.dust {
            // under the input: a cross (powers its four sides); next to it: a line away from it
            let shape = if *p == [0, -1, 0] {
                "east=side,north=side,power=0,south=side,west=side"
            } else if p[0] != 0 {
                "east=side,north=none,power=0,south=none,west=side"
            } else {
                "east=none,north=side,power=0,south=side,west=none"
            };
            d.blocks.insert(*p, format!("minecraft:redstone_wire[{shape}]"));
        }
        for p in &self.stone {
            d.blocks.insert(*p, "minecraft:stone".into());
        }
        d.blocks.insert(self.comparator, format!("minecraft:comparator[facing={},mode=compare,powered=false]", dir_name(sub(self.read, self.comparator))));
        d.block_entities.insert(self.read, "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}".into());
        d
    }

    /// The design text (with a caption line).
    pub fn design(&self, name: &str) -> String {
        let n = self.droppers.len();
        let total = self.n();
        let d = self.build(name);
        let direct = self.droppers.iter().filter(|p| dist(**p, X) == 1).count();
        let caption = format!(
            "# Counter {total}{}, dropper ring{}, {} blocks (track memory, `mctas circuit memory ring --n {n}{}`): the input cell fires {n} droppers ({direct} next to it{}), and {n} hoppers between them close the cycle dropper -> hopper -> dropper; each pulse fires every dropper, only the one holding the stone moves it, and its hopper delivers it to the next dropper after all have fired, so the stone advances one dropper per pulse whatever the firing order. The comparator reads one dropper: on during one pulse window in {n}{} ({}).\n",
            if self.divider { " as a divider" } else { "" },
            if self.bulbs > 0 { format!(" + {} copper bulb{}", self.bulbs, if self.bulbs > 1 { "s" } else { "" }) } else { String::new() },
            self.blocks(),
            if self.bulbs > 0 { format!(" --bulbs {}{}", self.bulbs, if self.divider { " --divider" } else { "" }) } else { String::new() },
            if self.dust.is_empty() { String::new() } else { format!(", {} through {} dust", n - direct, self.dust.len()) },
            if self.bulbs > 0 { format!(", and toggles a chain of {} bulb T flip-flop{} (bulb + comparator), each halving the rate", self.bulbs, if self.bulbs > 1 { "s" } else { "" }) } else { String::new() },
            if self.divider { format!("the last comparator changes state once every {total} pulses: toggles style") } else { "level style".to_string() },
        );
        format!("{caption}{}", d.to_text())
    }
}

/// The smallest rings for counter `n`: every cycle over the fired cells with valid hoppers, its
/// best comparator; sorted by blocks, at most `keep`.
pub fn search(n: usize, max_dust: usize, keep: usize, bulbs: usize) -> Vec<Ring> {
    let mut found: Vec<Ring> = Vec::new();
    let mut seen: BTreeSet<Vec<P>> = BTreeSet::new();
    for lay in layouts(max_dust) {
        let f: Vec<P> = lay.fired.iter().copied().collect();
        if f.len() < n {
            continue;
        }
        let blocked: BTreeSet<P> = lay.dust.iter().chain(lay.relays.iter()).copied().chain(std::iter::once(X)).collect();
        // dropper graph: pairs two apart with a free common neighbour (the hopper cells)
        let links = |a: P, b: P| -> Vec<P> {
            if dist(a, b) != 2 {
                return Vec::new();
            }
            DIRS.iter().map(|d| add(a, *d)).filter(|c| dist(*c, b) == 1 && !blocked.contains(c)).collect()
        };
        let mut path: Vec<P> = Vec::new();
        let mut hops: Vec<P> = Vec::new();
        for s in 0..f.len() {
            path.clear();
            hops.clear();
            path.push(f[s]);
            cycles(&f, s, n, &links, &mut path, &mut hops, &mut |ds: &[P], hs: &[P]| {
                for rev in [false, true] {
                    let (mut ds2, mut hs2) = (ds.to_vec(), hs.to_vec());
                    if rev {
                        // reverse flow: d0 <- h(n-1) <- d(n-1) ...
                        ds2 = std::iter::once(ds[0]).chain(ds[1..].iter().rev().copied()).collect();
                        hs2 = hs.iter().rev().copied().collect();
                    }
                    if let Some(r) = finish(&lay, &ds2, &hs2, bulbs) {
                        let mut key: Vec<P> = r.droppers.iter().chain(r.hoppers.iter()).chain(r.dust.iter()).copied().collect();
                        key.sort();
                        if seen.insert(key) {
                            found.push(r);
                        }
                    }
                }
            });
        }
    }
    found.sort_by_key(|r| r.blocks());
    found.truncate(keep);
    found
}

/// Simple cycles of `n` droppers starting at `f[s]` through droppers of larger index, with
/// distinct hopper cells.
fn cycles(f: &[P], s: usize, n: usize, links: &dyn Fn(P, P) -> Vec<P>, path: &mut Vec<P>, hops: &mut Vec<P>, emit: &mut dyn FnMut(&[P], &[P])) {
    let last = *path.last().unwrap();
    if path.len() == n {
        for h in links(last, f[s]) {
            if !hops.contains(&h) && !path.contains(&h) {
                hops.push(h);
                emit(path, hops);
                hops.pop();
            }
        }
        return;
    }
    for j in s + 1..f.len() {
        let b = f[j];
        if path.contains(&b) || hops.contains(&b) {
            continue;
        }
        for h in links(last, b) {
            if hops.contains(&h) || path.contains(&h) || f[s] == h {
                continue;
            }
            path.push(b);
            hops.push(h);
            cycles(f, s, n, links, path, hops, emit);
            hops.pop();
            path.pop();
        }
    }
}

/// Write a ring's droppers and hoppers (flow order) into a design.
fn put_ring(d: &mut mctas::design::Design, ds: &[P], hs: &[P]) {
    let n = ds.len();
    for i in 0..n {
        let (a, h, b) = (ds[i], hs[i], ds[(i + 1) % n]);
        d.blocks.insert(a, format!("minecraft:dropper[facing={},triggered=false]", dir_name(sub(h, a))));
        d.blocks.insert(h, format!("minecraft:hopper[enabled=true,facing={}]", dir_name(sub(b, h))));
    }
}

/// One more ring of a chain: its droppers are the neighbours of `center`, a stone the previous
/// stage's comparator strongly powers.
#[derive(Clone)]
pub struct Stage {
    pub center: P,
    pub droppers: Vec<P>,
    pub hoppers: Vec<P>,
    pub comparator: P,
    pub read: P,
    /// Floors added for this stage's comparator.
    pub stone: Vec<P>,
}

/// A multi-stage counter: ring 1 around the input, then rings around the stones the previous
/// comparators power; the last comparator is the output (counter n1 * n2 * ...).
#[derive(Clone)]
pub struct Chain {
    pub r1: Ring,
    pub stages: Vec<Stage>,
    /// Copper bulb T flip-flops after the last comparator (each doubles N), and their floors.
    pub bulbs: usize,
    pub bulb_stone: Vec<P>,
}

impl Chain {
    pub fn blocks(&self) -> usize {
        self.r1.blocks() + self.stages.iter().map(|s| 1 + 2 * s.droppers.len() + 1 + s.stone.len()).sum::<usize>() + 2 * self.bulbs + self.bulb_stone.len()
    }

    pub fn n(&self) -> usize {
        (self.r1.droppers.len() * self.stages.iter().map(|s| s.droppers.len()).product::<usize>()) << self.bulbs
    }

    /// Bulb T flip-flops in a straight line out of the last comparator (bulb, comparator facing
    /// back at it, its floor), the same checks as a ring's bulbs.
    fn with_bulbs(&self, lay: &Layout, k: usize) -> Option<Chain> {
        if k == 0 {
            return Some(self.clone());
        }
        let (cp, rd) = self.last();
        let u = sub(cp, rd);
        let side = [u[2], 0, u[0]];
        let emit = self.emitters(lay);
        let drops = self.droppers();
        let mut taken = self.used(lay);
        let mut stone = Vec::new();
        let at = |j: i32| add(cp, [u[0] * j, 0, u[2] * j]);
        for j in 0..k as i32 {
            // bulb at cp + (2j+1)u, comparator at cp + (2j+2)u, output beyond the last
            let (b, c) = (at(2 * j + 1), at(2 * j + 2));
            let fl = add(c, [0, -1, 0]);
            if taken.contains(&b) || taken.contains(&c) || taken.contains(&at(2 * j + 3)) || emit.contains(&fl) || self.comparators().contains(&fl) {
                return None;
            }
            // nothing else may power the bulb or the comparator's sides
            if [add(b, side), sub(b, side), add(b, UP), add(b, [0, -1, 0])].iter().any(|q| taken.contains(q) || emit.contains(q)) {
                return None;
            }
            if [add(c, side), sub(c, side)].iter().any(|q| emit.contains(q) || taken.contains(q)) {
                return None;
            }
            taken.insert(b);
            taken.insert(c);
            if !taken.contains(&fl) {
                stone.push(fl);
                taken.insert(fl);
            }
        }
        let o = at(2 * k as i32 + 1);
        if DIRS.iter().any(|d| emit.contains(&add(o, *d))) || drops.contains(&add(o, [0, -1, 0])) {
            return None;
        }
        let mut next = self.clone();
        next.bulbs = k;
        next.bulb_stone = stone;
        Some(next)
    }

    /// The last comparator and the dropper it reads.
    fn last(&self) -> (P, P) {
        match self.stages.last() {
            Some(s) => (s.comparator, s.read),
            None => (self.r1.comparator, self.r1.read),
        }
    }

    fn containers(&self) -> BTreeSet<P> {
        self.r1.droppers.iter().chain(self.r1.hoppers.iter()).chain(self.stages.iter().flat_map(|s| s.droppers.iter().chain(s.hoppers.iter()))).copied().collect()
    }

    fn droppers(&self) -> BTreeSet<P> {
        self.r1.droppers.iter().chain(self.stages.iter().flat_map(|s| s.droppers.iter())).copied().collect()
    }

    fn hoppers(&self) -> Vec<P> {
        self.r1.hoppers.iter().chain(self.stages.iter().flat_map(|s| s.hoppers.iter())).copied().collect()
    }

    /// Comparators (no block may stand on one).
    fn comparators(&self) -> BTreeSet<P> {
        std::iter::once(self.r1.comparator).chain(self.stages.iter().map(|s| s.comparator)).collect()
    }

    fn used(&self, lay: &Layout) -> BTreeSet<P> {
        let mut u = self.containers();
        u.extend(lay.dust.iter().copied());
        u.extend(self.r1.stone.iter().copied());
        u.insert(X);
        u.extend(self.comparators());
        for s in &self.stages {
            u.insert(s.center);
            u.extend(s.stone.iter().copied());
        }
        u
    }

    /// What may power a cell next to it: the input, dust, relays, the stage stones.
    fn emitters(&self, lay: &Layout) -> BTreeSet<P> {
        lay.dust.iter().chain(lay.relays.iter()).copied().chain(std::iter::once(X)).chain(self.stages.iter().map(|s| s.center)).collect()
    }

    pub fn design(&self, name: &str) -> String {
        let mut d = self.r1.build(name);
        d.meta.insert("circuit".into(), format!("counter {}", self.n()));
        let (cp, rd) = self.last();
        let u = sub(cp, rd);
        let at = |j: i32| add(cp, [u[0] * j, 0, u[2] * j]);
        d.ports.insert("output".into(), at(2 * self.bulbs as i32 + 1));
        for s in &self.stages {
            d.blocks.insert(s.center, "minecraft:stone".into());
            put_ring(&mut d, &s.droppers, &s.hoppers);
            for p in &s.stone {
                d.blocks.insert(*p, "minecraft:stone".into());
            }
            d.blocks.insert(s.comparator, format!("minecraft:comparator[facing={},mode=compare,powered=false]", dir_name(sub(s.read, s.comparator))));
            d.block_entities.insert(s.read, "{Items:[{Slot:0b,id:\"minecraft:stone\",count:1}]}".into());
        }
        for j in 0..self.bulbs as i32 {
            d.blocks.insert(at(2 * j + 1), "minecraft:waxed_copper_bulb[lit=false,powered=false]".into());
            d.blocks.insert(at(2 * j + 2), format!("minecraft:comparator[facing={},mode=compare,powered=false]", dir_name([-u[0], 0, -u[2]])));
        }
        for p in &self.bulb_stone {
            d.blocks.insert(*p, "minecraft:stone".into());
        }
        let sizes: Vec<String> = std::iter::once(self.r1.droppers.len()).chain(self.stages.iter().map(|s| s.droppers.len())).map(|k| k.to_string()).collect();
        let caption = format!(
            "# Counter {}, {} chained dropper rings ({}){}, {} blocks (track memory, `mctas circuit memory ring --n {} --then {}{}`): ring 1 ({} droppers fired by the input{}, hoppers between them) advances its stone one dropper per pulse; each ring's comparator lights once per lap and strongly powers a stone whose neighbours are the next ring's droppers, so each ring advances once per lap of the previous one{}; the last comparator is on during one pulse window in {} (level style). Every transit crosses a hopper, so the firing order never matters.\n",
            self.n(),
            sizes.len(),
            sizes.join(" x "),
            if self.bulbs > 0 { format!(" + {} copper bulb{}", self.bulbs, if self.bulbs > 1 { "s" } else { "" }) } else { String::new() },
            self.blocks(),
            sizes[0],
            sizes[1..].join(","),
            if self.bulbs > 0 { format!(" --bulbs {}", self.bulbs) } else { String::new() },
            sizes[0],
            if self.r1.dust.is_empty() { String::new() } else { format!(", some through {} dust", self.r1.dust.len()) },
            if self.bulbs > 0 { format!(", and the last ring toggles {} bulb T flip-flop{}, each halving the rate", self.bulbs, if self.bulbs > 1 { "s" } else { "" }) } else { String::new() },
            self.n(),
        );
        format!("{caption}{}", d.to_text())
    }
}

/// Every valid comparator placement for a ring core (no bulbs).
fn placements(lay: &Layout, ds: &[P], hs: &[P]) -> Vec<Ring> {
    let Some(best) = finish(lay, ds, hs, 0) else { return Vec::new() };
    let base_stone: Vec<P> = {
        let containers: BTreeSet<P> = ds.iter().chain(hs.iter()).copied().collect();
        let mut s: Vec<P> = lay.floors.iter().copied().filter(|p| !containers.contains(p)).chain(lay.relays.iter().copied()).collect();
        s.sort();
        s.dedup();
        s
    };
    let used: BTreeSet<P> = ds.iter().chain(hs.iter()).chain(lay.dust.iter()).chain(base_stone.iter()).copied().chain(std::iter::once(X)).collect();
    let emitters: BTreeSet<P> = lay.dust.iter().chain(lay.relays.iter()).copied().chain(std::iter::once(X)).collect();
    let mut out = Vec::new();
    for &d in ds {
        for u in HORIZ {
            let (c, o, fl) = (add(d, u), add(add(d, u), u), add(add(d, u), [0, -1, 0]));
            let side = [u[2], 0, u[0]];
            if used.contains(&c) || used.contains(&o) || emitters.contains(&fl) || [add(c, side), sub(c, side)].iter().any(|s| emitters.contains(s)) {
                continue;
            }
            let mut st = base_stone.clone();
            if !used.contains(&fl) {
                st.push(fl);
            }
            out.push(Ring { comparator: c, read: d, stone: st, ..best.clone() });
        }
    }
    out
}

/// The next ring of a chain, around the stone the chain's last comparator powers: the smallest
/// fit.
fn next_stage(lay: &Layout, ch: &Chain, n: usize) -> Option<Chain> {
    let (cp, rd) = ch.last();
    let s = add(cp, sub(cp, rd));
    let used = ch.used(lay);
    if used.contains(&s) {
        return None;
    }
    let cont = ch.containers();
    let drops = ch.droppers();
    // nothing earlier touches S but the comparator feeding it (it would fire or lock), no dust
    if DIRS.iter().map(|d| add(s, *d)).any(|q| q != cp && (cont.contains(&q) || lay.dust.contains(&q))) {
        return None;
    }
    // nor sits under a cell next to S (S's signal there would quasi-power it)
    if DIRS.iter().map(|d| add(add(s, *d), [0, -1, 0])).any(|q| drops.contains(&q)) {
        return None;
    }
    let prev = ch.emitters(lay);
    // a cell an earlier event may power: next to the input, a dust, a relay or an earlier stone
    let near = |p: P| DIRS.iter().any(|d| prev.contains(&add(p, *d)));
    let free = |p: P| !used.contains(&p) && p != s && !near(p);
    // droppers also stay out from under such cells (quasi-power)
    let f: Vec<P> = DIRS.iter().map(|d| add(s, *d)).filter(|p| *p != cp && free(*p) && !near(add(*p, UP)) && !lay.dust.contains(&add(*p, UP))).collect();
    if f.len() < n {
        return None;
    }
    // this ring's hoppers stay off S (it stays powered for a whole window and would lock them)
    let links = |a: P, b: P| -> Vec<P> {
        if dist(a, b) != 2 {
            return Vec::new();
        }
        DIRS.iter().map(|d| add(a, *d)).filter(|c| dist(*c, b) == 1 && free(*c) && dist(*c, s) != 1).collect()
    };
    let comparators = ch.comparators();
    let old_hoppers = ch.hoppers();
    let mut best: Option<Chain> = None;
    let mut path: Vec<P> = Vec::new();
    let mut hops: Vec<P> = Vec::new();
    for st in 0..f.len() {
        path.clear();
        hops.clear();
        path.push(f[st]);
        cycles(&f, st, n, &links, &mut path, &mut hops, &mut |ds: &[P], hs: &[P]| {
            for rev in [false, true] {
                let (mut dn, mut hn) = (ds.to_vec(), hs.to_vec());
                if rev {
                    dn = std::iter::once(ds[0]).chain(ds[1..].iter().rev().copied()).collect();
                    hn = hs.iter().rev().copied().collect();
                }
                let k = dn.len();
                if (0..k).any(|i| sub(dn[(i + 1) % k], hn[i]) == UP) {
                    continue;
                }
                let all: BTreeSet<P> = cont.iter().chain(dn.iter()).chain(hn.iter()).copied().collect();
                if old_hoppers.iter().chain(hn.iter()).any(|h| all.contains(&add(*h, UP))) {
                    continue;
                }
                let all_drops: BTreeSet<P> = drops.iter().chain(dn.iter()).copied().collect();
                let used2: BTreeSet<P> = used.iter().chain(dn.iter()).chain(hn.iter()).copied().chain(std::iter::once(s)).collect();
                let emit: BTreeSet<P> = prev.iter().copied().chain(std::iter::once(s)).collect();
                for &d2 in &dn {
                    for u2 in HORIZ {
                        let (c2, o2, fl) = (add(d2, u2), add(add(d2, u2), u2), add(add(d2, u2), [0, -1, 0]));
                        let side = [u2[2], 0, u2[0]];
                        // the floor: empty (stone goes there) or a block with a full top
                        if comparators.contains(&fl) || used2.contains(&c2) || used2.contains(&o2) || emit.contains(&fl) || [add(c2, side), sub(c2, side)].iter().any(|q| emit.contains(q)) {
                            continue;
                        }
                        // the output cell must not touch an emitter nor sit on a dropper
                        if DIRS.iter().any(|dd| emit.contains(&add(o2, *dd))) || all_drops.contains(&add(o2, [0, -1, 0])) {
                            continue;
                        }
                        let stone = if used2.contains(&fl) { Vec::new() } else { vec![fl] };
                        let mut next = ch.clone();
                        next.stages.push(Stage { center: s, droppers: dn.clone(), hoppers: hn.clone(), comparator: c2, read: d2, stone });
                        if best.as_ref().is_none_or(|b| next.blocks() < b.blocks()) {
                            best = Some(next);
                        }
                    }
                }
            }
        });
    }
    best
}

/// The smallest chained counters n1 x n2 x ... (`ns` = the ring sizes, input side first).
pub fn chain_search(ns: &[usize], max_dust: usize, keep: usize, bulbs: usize) -> Vec<Chain> {
    let n1 = ns[0];
    let mut found: Vec<Chain> = Vec::new();
    for lay in layouts(max_dust) {
        let f: Vec<P> = lay.fired.iter().copied().collect();
        if f.len() < n1 {
            continue;
        }
        let blocked: BTreeSet<P> = lay.dust.iter().chain(lay.relays.iter()).copied().chain(std::iter::once(X)).collect();
        let links = |a: P, b: P| -> Vec<P> {
            if dist(a, b) != 2 {
                return Vec::new();
            }
            DIRS.iter().map(|d| add(a, *d)).filter(|c| dist(*c, b) == 1 && !blocked.contains(c)).collect()
        };
        let mut path: Vec<P> = Vec::new();
        let mut hops: Vec<P> = Vec::new();
        for s in 0..f.len() {
            path.clear();
            hops.clear();
            path.push(f[s]);
            cycles(&f, s, n1, &links, &mut path, &mut hops, &mut |ds: &[P], hs: &[P]| {
                for rev in [false, true] {
                    let (mut ds1, mut hs1) = (ds.to_vec(), hs.to_vec());
                    if rev {
                        ds1 = std::iter::once(ds[0]).chain(ds[1..].iter().rev().copied()).collect();
                        hs1 = hs.iter().rev().copied().collect();
                    }
                    'placement: for r1 in placements(&lay, &ds1, &hs1) {
                        let mut ch = Chain { r1, stages: Vec::new(), bulbs: 0, bulb_stone: Vec::new() };
                        for &nk in &ns[1..] {
                            match next_stage(&lay, &ch, nk) {
                                Some(next) => ch = next,
                                None => continue 'placement,
                            }
                        }
                        if let Some(b) = ch.with_bulbs(&lay, bulbs) {
                            found.push(b);
                        }
                    }
                }
            });
        }
    }
    found.sort_by_key(|c| c.blocks());
    found.truncate(keep);
    found
}

/// Check a cycle's hoppers, add the floors and the best comparator.
fn finish(lay: &Layout, ds: &[P], hs: &[P], bulbs: usize) -> Option<Ring> {
    let n = ds.len();
    let containers: BTreeSet<P> = ds.iter().chain(hs.iter()).copied().collect();
    if hs.iter().any(|h| ds.contains(h) || lay.dust.contains(h) || lay.relays.contains(h) || *h == X) {
        return None;
    }
    for i in 0..n {
        let (h, b) = (hs[i], ds[(i + 1) % n]);
        if sub(b, h) == UP {
            return None;
        }
        if containers.contains(&add(h, UP)) {
            return None;
        }
    }
    let mut stone: Vec<P> = lay.floors.iter().copied().filter(|p| !containers.contains(p)).chain(lay.relays.iter().copied()).collect();
    stone.sort();
    stone.dedup();
    let used: BTreeSet<P> = containers.iter().chain(lay.dust.iter()).chain(stone.iter()).copied().chain(std::iter::once(X)).collect();
    // what may power a cell next to it: the input, dust, relay stones
    let emitters: BTreeSet<P> = lay.dust.iter().chain(lay.relays.iter()).copied().chain(std::iter::once(X)).collect();
    let mut best: Option<Ring> = None;
    for &d in ds {
        'dir: for u in HORIZ {
            let side = [u[2], 0, u[0]];
            let mut st = stone.clone();
            let mut taken = used.clone();
            // comparator j (0 = the ring's) at d + (2j+1)u, bulb j+1 in front of it; the last
            // comparator's front is the output
            for j in 0..=bulbs {
                let c = add(d, [u[0] * (2 * j as i32 + 1), 0, u[2] * (2 * j as i32 + 1)]);
                let o = add(c, u);
                let fl = add(c, [0, -1, 0]);
                if taken.contains(&c) || taken.contains(&o) || emitters.contains(&fl) {
                    continue 'dir;
                }
                if [add(c, side), sub(c, side)].iter().any(|s| emitters.contains(s) || (j > 0 && taken.contains(s))) {
                    continue 'dir;
                }
                // the output cell (or the bulb there) must not touch the input or a dust, nor sit
                // on a dropper (the comparator would quasi-power it)
                if DIRS.iter().any(|dd| emitters.contains(&add(o, *dd))) || (j == bulbs && ds.contains(&add(o, [0, -1, 0]))) {
                    continue 'dir;
                }
                // a bulb's sides must be free of ring blocks too (nothing else may power it)
                if j < bulbs && [add(o, side), sub(o, side), add(o, UP), add(o, [0, -1, 0])].iter().any(|s| taken.contains(s)) {
                    continue 'dir;
                }
                taken.insert(c);
                if !taken.contains(&fl) {
                    st.push(fl);
                    taken.insert(fl);
                }
                if j < bulbs {
                    taken.insert(o);
                }
            }
            // the stone starts in the dropper the comparator reads
            let r = Ring { droppers: ds.to_vec(), hoppers: hs.to_vec(), dust: lay.dust.clone(), stone: st, comparator: add(d, u), read: d, bulbs, divider: false };
            if best.as_ref().is_none_or(|b| r.blocks() < b.blocks()) {
                best = Some(r);
            }
        }
    }
    best
}

/// `circuit memory ring`: write the smallest rings for each N.
pub fn run(ns: &[usize], max_dust: usize, keep: usize, bulbs: usize, then: &[usize], divider: bool, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    if !then.is_empty() {
        for &n in ns {
            let sizes: Vec<usize> = std::iter::once(n).chain(then.iter().copied()).collect();
            let chains = chain_search(&sizes, max_dust, keep, bulbs);
            let total: usize = sizes.iter().product::<usize>() << bulbs;
            let mut label: Vec<String> = sizes.iter().map(|k| format!("ring{k}")).collect();
            if bulbs > 0 {
                label.push(format!("bulb{bulbs}"));
            }
            if chains.is_empty() {
                println!("counter {total}: no {} chain with at most {max_dust} dust", label.join(" -> "));
                continue;
            }
            for (i, c) in chains.iter().enumerate() {
                let base = format!("counter{total}-{}", label.join("-"));
                let name = if chains.len() == 1 { base } else { format!("{base}-{i}") };
                std::fs::write(out.join(format!("{name}.txt")), c.design(&name))?;
                println!("counter {total}: {} blocks -> {name}", c.blocks());
            }
        }
        return Ok(());
    }
    for &n in ns {
        if n < 2 {
            bail!("--n: at least 2");
        }
        let mut rings = search(n, max_dust, keep, bulbs);
        if divider {
            if bulbs == 0 {
                bail!("--divider needs --bulbs 1 or more");
            }
            for r in &mut rings {
                r.divider = true;
            }
        }
        let total = if divider { n << (bulbs - 1) } else { n << bulbs };
        if rings.is_empty() {
            println!("counter {total}: no ring of {n} with at most {max_dust} dust and {bulbs} bulbs");
            continue;
        }
        for (i, r) in rings.iter().enumerate() {
            let base = format!("counter{total}-ring{n}{}{}", if bulbs > 0 { format!("-bulb{bulbs}") } else { String::new() }, if divider { "-div" } else { "" });
            let name = if rings.len() == 1 { base } else { format!("{base}-{i}") };
            std::fs::write(out.join(format!("{name}.txt")), r.design(&name))?;
            println!("counter {total}: {} blocks ({} dust) -> {name}", r.blocks(), r.dust.len());
        }
    }
    Ok(())
}
