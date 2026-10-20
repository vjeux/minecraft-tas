//! `mctas circuit timer rings`: comparator rings, enumerated and judged on redsim.
//!
//! The core: two comparators in a ring. C1 reads the stone I beside it and powers A (stone or
//! dust); C2 reads A and powers T (stone or dust); a line of dust carries T back into I. Each dust
//! after the first loses 1, so the ring loses `dust - 1` per round of 4 gt (2 comparators x 2 gt)
//! and once its charge stops the output stays on until it decays to 0. What charges it (`--front`):
//! - `input` (an off-delay): the input cell beside a line dust holds the ring up while it is on;
//! - `retrig` (a retriggerable monostable): each rising edge gives 4 gt of 15 into I: a copper bulb
//!   the input toggles, a comparator K reading it, an observer O1 under K firing down into a stone
//!   whose cross dust powers I, and an observer O2 watching O1 firing the next 2 gt into a dust on
//!   top of I (both at 15, so C1 charges to the same level whatever its tick phase).
//! Every simple dust line from T to I in the box (no dust beside another one out of turn, none on a
//! comparator's side or beside the front), every input cell beside a line dust (`input`) and every
//! output cell beside the ring are tried; the judge's own redsim check (`circuit::verify::sim`)
//! keeps the passes. The smallest pass of each L is written to `<out>/<kind>-ring-L<L>.txt`;
//! `<out>/rings.tsv` lists every pass.

use anyhow::{Result, bail};
use circuit::Circuit;
use circuit::verify::Options;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

pub type P2 = (i32, i32);
type P3 = (i32, i32, i32);

const C1: P2 = (1, 0); // facing east: reads I (east), powers A (west)
const C2: P2 = (0, 1); // facing north: reads A (north), powers T (south)
const I: P2 = (2, 0);
const A: P2 = (0, 0);
const T: P2 = (0, 2);

/// What charges the ring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Front {
    /// An input cell beside a line dust (an off-delay).
    Input,
    /// The copper-bulb two-observer edge front east of I (a retriggerable monostable).
    Retrig,
}

impl Front {
    pub fn parse(s: &str) -> Result<Front> {
        Ok(match s {
            "input" => Front::Input,
            "retrig" => Front::Retrig,
            _ => bail!("--front {s:?}: input or retrig"),
        })
    }

    fn kind(self) -> &'static str {
        match self {
            Front::Input => "off-delay",
            Front::Retrig => "monostable retriggerable",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Front::Input => "off-delay",
            Front::Retrig => "mono-retrig",
        }
    }

    /// The front's own blocks (x, y, z) -> palette char; the ring's plane is y = 1.
    fn blocks(self) -> Vec<(P3, char)> {
        match self {
            Front::Input => Vec::new(),
            Front::Retrig => vec![
                ((3, 0, 0), 'S'),
                ((3, 1, 0), 'd'), // cross dust: Q's strong power into I at 15
                ((4, 1, 0), 'S'), // Q
                ((4, 2, 0), 'U'), // O1, facing up (watches K), fires down into Q
                ((3, 2, 0), 'O'), // O2, facing east (watches O1), fires into the dust on I
                ((2, 2, 0), 'd'), // dust on top of I
                ((4, 3, 0), 'E'), // K, reads the bulb
                ((5, 3, 0), 'B'),
            ],
        }
    }

    /// Ring-plane cells a return line may not use: the front's own cells and the cells beside its
    /// cross dust and its stone Q (a line dust there would join or tap them).
    fn blocked(self) -> HashSet<P2> {
        match self {
            Front::Input => HashSet::new(),
            Front::Retrig => [(3, 0), (4, 0), (3, 1), (3, -1), (4, 1), (4, -1), (5, 0)].into_iter().collect(),
        }
    }
}

fn adj(a: P2, b: P2) -> bool {
    (a.0 - b.0).abs() + (a.1 - b.1).abs() == 1
}

fn neighbors(p: P2) -> [P2; 4] {
    [(p.0 + 1, p.1), (p.0 - 1, p.1), (p.0, p.1 + 1), (p.0, p.1 - 1)]
}

/// One ring: its return line (from beside T to beside I), A and T as dust or stone, what charges
/// it, the input cell (`Front::Input`; the front's own otherwise), the output cell.
#[derive(Clone, Debug)]
pub struct Ring {
    pub line: Vec<P2>,
    pub a_dust: bool,
    pub t_dust: bool,
    pub front: Front,
    pub input: P2,
    pub output: P2,
}

impl Ring {
    /// Blocks placed (supports included).
    pub fn blocks(&self) -> usize {
        // comparators + their supports, I, A (stone, or dust + support), T, the line + supports
        let a = if self.a_dust { 2 } else { 1 };
        let t = if self.t_dust { 2 } else { 1 };
        4 + 1 + a + t + 2 * self.line.len() + self.front.blocks().len()
    }

    fn input3(&self) -> P3 {
        match self.front {
            Front::Input => (self.input.0, 1, self.input.1),
            Front::Retrig => (6, 3, 0),
        }
    }

    pub fn design(&self, name: &str) -> String {
        let mut cells: BTreeMap<P3, char> = BTreeMap::new();
        let mut put2 = |p: P2, c: char| {
            cells.insert((p.0, 1, p.1), c);
            if matches!(c, 'E' | 'n' | 'd') {
                cells.insert((p.0, 0, p.1), 'S');
            }
        };
        put2(C1, 'E');
        put2(C2, 'n');
        put2(I, 'S');
        put2(A, if self.a_dust { 'd' } else { 'S' });
        put2(T, if self.t_dust { 'd' } else { 'S' });
        for p in &self.line {
            put2(*p, 'd');
        }
        for (p, c) in self.front.blocks() {
            cells.insert(p, c);
        }
        let inp = self.input3();
        let outp = (self.output.0, 1, self.output.1);
        let all: Vec<P3> = cells.keys().copied().chain([inp, outp]).collect();
        let (x0, x1) = (all.iter().map(|p| p.0).min().unwrap(), all.iter().map(|p| p.0).max().unwrap());
        let y1 = all.iter().map(|p| p.1).max().unwrap();
        let (z0, z1) = (all.iter().map(|p| p.2).min().unwrap(), all.iter().map(|p| p.2).max().unwrap());
        let mut layers = String::new();
        for y in 0..=y1 {
            layers.push_str(&format!("layer {y}:\n"));
            for z in z0..=z1 {
                layers.push_str("  ");
                for x in x0..=x1 {
                    layers.push(cells.get(&(x, y, z)).copied().unwrap_or('.'));
                }
                layers.push('\n');
            }
        }
        let dust = self.line.len() + self.t_dust as usize;
        let how = match self.front {
            Front::Input => format!("{dust} dust carry C2's output back to C1, losing {} per round of 4 gt; the input holds the ring up", dust - 1),
            Front::Retrig => format!(
                "{dust} dust carry C2's output back to C1, losing {} per round of 4 gt; each rising edge charges it with 4 gt of 15 (copper bulb, comparator K, observer O1 down into a stone and a cross dust, observer O2 watching O1 into the dust on top of I)",
                dust - 1
            ),
        };
        let group = match self.front {
            Front::Input => "group: off-delay-ring\ngroup title: comparator-ring off-delay\ngroup note: two comparators in a ring with a dust return line; L set by the line's length and the cell the output reads\n",
            Front::Retrig => "group: mono-retrig-ring\ngroup title: comparator-ring retriggerable monostable\ngroup note: a copper-bulb two-observer edge front charging a two-comparator ring; L set by the return line's length and the cell the output reads\n",
        };
        format!(
            "name: {name}\ncircuit: {}\n{group}# a comparator ring (crates/circuit-timer/src/rings.rs): {how}\norigin: {x0} 0 {z0}\nport input: {} {} {}\nport output: {} {} {}\npalette:\n  S minecraft:stone\n  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]\n  E minecraft:comparator[facing=east,mode=compare,powered=false]\n  n minecraft:comparator[facing=north,mode=compare,powered=false]\n  O minecraft:observer[facing=east,powered=false]\n  U minecraft:observer[facing=up,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\n{layers}",
            self.front.kind(),
            inp.0,
            inp.1,
            inp.2,
            outp.0,
            outp.1,
            outp.2
        )
    }
}

/// Every return line of `min..=max` dust inside the box: a simple path whose first cell is beside
/// T, whose last cell is beside I and points into it (in line with its predecessor, or alone in
/// line with T and I), no two cells adjacent out of turn, none on a ring cell, a comparator side or
/// a `blocked` cell. With `top` (a dust on top of I), the cells beside I may only end the line.
pub fn lines(lo: P2, hi: P2, min: usize, max: usize, blocked: &HashSet<P2>, top: bool) -> Vec<Vec<P2>> {
    let ring: HashSet<P2> = [C1, C2, I, A, T].into_iter().collect();
    // comparator sides: C1 (east-west) has (1,-1), (1,1); C2 (north-south) has (-1,1), (1,1)
    let sides: HashSet<P2> = [(1, -1), (1, 1), (-1, 1)].into_iter().collect();
    let inside = |p: P2| p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1;
    let mut out = Vec::new();
    let mut path: Vec<P2> = Vec::new();
    struct Env<'a> {
        ok: &'a dyn Fn(P2) -> bool,
        min: usize,
        max: usize,
        top: bool,
    }
    fn rec(path: &mut Vec<P2>, out: &mut Vec<Vec<P2>>, e: &Env) {
        let last = *path.last().unwrap();
        // a finished line: the last cell beside I, pointing into it
        if path.len() >= e.min && adj(last, I) {
            let points = if path.len() == 1 {
                // alone: a straight line through T, the dust and I
                (T.0 - last.0, T.1 - last.1) == (last.0 - I.0, last.1 - I.1)
            } else {
                let prev = path[path.len() - 2];
                (last.0 - prev.0, last.1 - prev.1) == (I.0 - last.0, I.1 - last.1)
            };
            if points {
                out.push(path.clone());
            }
        }
        if path.len() >= e.max || (e.top && adj(last, I)) {
            return;
        }
        for n in neighbors(last) {
            if !(e.ok)(n) || path.contains(&n) {
                continue;
            }
            // no adjacency to earlier cells but the last one (a clean line)
            if path[..path.len() - 1].iter().any(|q| adj(*q, n)) {
                continue;
            }
            path.push(n);
            rec(path, out, e);
            path.pop();
        }
    }
    let ok = |p: P2| inside(p) && !ring.contains(&p) && !sides.contains(&p) && !blocked.contains(&p);
    let env = Env { ok: &ok, min, max, top };
    for s in neighbors(T) {
        if !ok(s) {
            continue;
        }
        path.push(s);
        rec(&mut path, &mut out, &env);
        path.pop();
    }
    out
}

/// The judge's line of one ring on redsim: `Some((L, latency or delay, line))` when it passes.
pub fn judge(r: &Ring, o: &Options) -> Option<(i64, i64, String)> {
    let text = r.design("ring");
    let d = mctas::design::Design::parse(&text).ok()?;
    let c = Circuit::from_design(d).ok()?;
    let rep = circuit::verify::sim(&c, o).ok()?;
    let out = rep.outcome.as_ref()?;
    if !out.pass {
        return None;
    }
    let num = |k: &str| out.metrics.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.split_whitespace().next()?.parse::<i64>().ok());
    Some((num("L")?, num("latency").or_else(|| num("delay"))?, rep.summary(&c)))
}

/// Every candidate ring of one front.
fn candidates(front: Front, max_line: usize) -> Vec<Ring> {
    let blocked = front.blocked();
    let front_cells: HashSet<P2> = front.blocks().iter().filter(|(p, _)| p.1 == 1).map(|(p, _)| (p.0, p.2)).collect();
    let mut rings: Vec<Ring> = Vec::new();
    for line in lines((-2, -1), (4, 5), 1, max_line, &blocked, front == Front::Retrig) {
        let used: HashSet<P2> = line.iter().copied().chain([C1, C2, I, A, T]).collect();
        for a_dust in [false, true] {
            for t_dust in [false, true] {
                let mut inputs: Vec<P2> = Vec::new();
                if front == Front::Input {
                    // input cells: beside exactly one line dust, beside no comparator, not used
                    for p in &line {
                        for n in neighbors(*p) {
                            if used.contains(&n) || inputs.contains(&n) || adj(n, C1) || adj(n, C2) {
                                continue;
                            }
                            if line.iter().filter(|q| adj(**q, n)).count() == 1 {
                                inputs.push(n);
                            }
                        }
                    }
                } else {
                    inputs.push((6, 0));
                }
                // output cells: any free cell beside the ring (A, T, I, a line dust, a comparator):
                // the judge decides what powers its lamp
                let mut outputs: Vec<P2> = Vec::new();
                for src in used.iter() {
                    for n in neighbors(*src) {
                        if !used.contains(&n) && !outputs.contains(&n) && !front_cells.contains(&n) && !blocked.contains(&n) {
                            outputs.push(n);
                        }
                    }
                }
                for &i in &inputs {
                    for &op in &outputs {
                        if front == Front::Input && (op == i || adj(op, i)) {
                            continue;
                        }
                        rings.push(Ring { line: line.clone(), a_dust, t_dust, front, input: i, output: op });
                    }
                }
            }
        }
    }
    rings
}

pub fn run(out: &Path, max_line: usize, workers: usize, version: &str, front: Front) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let o = Options { version: version.to_string(), ..Default::default() };
    let rings = candidates(front, max_line);
    println!("{} ring candidates", rings.len());
    let found: Mutex<Vec<(i64, i64, usize, String, Ring)>> = Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(r) = rings.get(k) else { break };
                    if let Some((l, lat, line)) = judge(r, &o) {
                        found.lock().unwrap().push((l, lat, r.blocks(), line, r.clone()));
                    }
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap();
    found.sort_by_key(|f| (f.0, f.2, f.1));
    let mut tsv = String::from("L\tlatency\tblocks\tline\n");
    let mut best: BTreeMap<i64, (usize, i64, Ring)> = BTreeMap::new();
    for (l, lat, b, line, r) in &found {
        tsv.push_str(&format!("{l}\t{lat}\t{b}\t{line}\n"));
        let e = best.entry(*l).or_insert((*b, *lat, r.clone()));
        if (*b, *lat) < (e.0, e.1) {
            *e = (*b, *lat, r.clone());
        }
    }
    std::fs::write(out.join("rings.tsv"), tsv)?;
    for (l, (b, lat, r)) in &best {
        let name = format!("{}-ring-L{l}", front.prefix());
        std::fs::write(out.join(format!("{name}.txt")), r.design(&name))?;
        println!("L {l} gt: {b} blocks, latency/delay {lat} gt");
    }
    println!("{} passes, {} L values", found.len(), best.len());
    Ok(())
}
