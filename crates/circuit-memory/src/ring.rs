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
}

impl Ring {
    pub fn blocks(&self) -> usize {
        self.droppers.len() + self.hoppers.len() + self.dust.len() + self.stone.len() + 1 + 2 * self.bulbs
    }

    /// The counter's N.
    pub fn n(&self) -> usize {
        self.droppers.len() << self.bulbs
    }

    /// The design text (with a caption line).
    pub fn design(&self, name: &str) -> String {
        let n = self.droppers.len();
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
        for i in 0..n {
            let (a, h, b) = (self.droppers[i], self.hoppers[i], self.droppers[(i + 1) % n]);
            d.blocks.insert(a, format!("minecraft:dropper[facing={},triggered=false]", dir_name(sub(h, a))));
            d.blocks.insert(h, format!("minecraft:hopper[enabled=true,facing={}]", dir_name(sub(b, h))));
        }
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
        let direct = self.droppers.iter().filter(|p| dist(**p, X) == 1).count();
        let caption = format!(
            "# Counter {total}, dropper ring{}, {} blocks (track memory, `mctas circuit memory ring --n {n}{}`): the input cell fires {n} droppers ({direct} next to it{}), and {n} hoppers between them close the cycle dropper -> hopper -> dropper; each pulse fires every dropper, only the one holding the stone moves it, and its hopper delivers it to the next dropper after all have fired, so the stone advances one dropper per pulse whatever the firing order. The comparator reads one dropper: on during one pulse window in {n}{} (level style).\n",
            if self.bulbs > 0 { format!(" + {} copper bulb{}", self.bulbs, if self.bulbs > 1 { "s" } else { "" }) } else { String::new() },
            self.blocks(),
            if self.bulbs > 0 { format!(" --bulbs {}", self.bulbs) } else { String::new() },
            if self.dust.is_empty() { String::new() } else { format!(", {} through {} dust", n - direct, self.dust.len()) },
            if self.bulbs > 0 { format!(", and toggles a chain of {} bulb T flip-flop{} (bulb + comparator), each halving the rate", self.bulbs, if self.bulbs > 1 { "s" } else { "" }) } else { String::new() },
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
                // the output cell (or the bulb there) must not touch the input or a dust
                if DIRS.iter().any(|dd| emitters.contains(&add(o, *dd))) {
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
            let r = Ring { droppers: ds.to_vec(), hoppers: hs.to_vec(), dust: lay.dust.clone(), stone: st, comparator: add(d, u), read: d, bulbs };
            if best.as_ref().is_none_or(|b| r.blocks() < b.blocks()) {
                best = Some(r);
            }
        }
    }
    best
}

/// `circuit memory ring`: write the smallest rings for each N.
pub fn run(ns: &[usize], max_dust: usize, keep: usize, bulbs: usize, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    for &n in ns {
        if n < 2 {
            bail!("--n: at least 2");
        }
        let rings = search(n, max_dust, keep, bulbs);
        let total = n << bulbs;
        if rings.is_empty() {
            println!("counter {total}: no ring of {n} with at most {max_dust} dust and {bulbs} bulbs");
            continue;
        }
        for (i, r) in rings.iter().enumerate() {
            let base = format!("counter{total}-ring{n}{}", if bulbs > 0 { format!("-bulb{bulbs}") } else { String::new() });
            let name = if rings.len() == 1 { base } else { format!("{base}-{i}") };
            std::fs::write(out.join(format!("{name}.txt")), r.design(&name))?;
            println!("counter {total}: {} blocks ({} dust) -> {name}", r.blocks(), r.dust.len());
        }
    }
    Ok(())
}
