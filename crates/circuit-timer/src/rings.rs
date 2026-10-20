//! `mctas circuit timer rings`: comparator-ring off-delays, enumerated and judged on redsim.
//!
//! The core: two comparators in a ring. C1 reads the stone I beside it and powers A (stone or
//! dust); C2 reads A and powers T (stone or dust); a line of dust carries T back into I. Each dust
//! after the first loses 1, so the ring loses `dust - 1` per round of 4 gt (2 comparators x 2 gt)
//! and an input that held it up leaves the output on until it decays to 0: an off-delay whose L is
//! set by the line's length, where the input charges it and which ring cell the output lamp reads.
//! Every simple dust line from T to I in the box (no dust beside another one out of turn, none on a
//! comparator's side), every input cell beside a line dust and every output cell beside A or T are
//! tried; the judge's own redsim check (`circuit::verify::sim`) keeps the passes. The smallest pass
//! of each L is written to `<out>/off-delay-ring-L<L>.txt`; `<out>/rings.tsv` lists every pass.

use anyhow::Result;
use circuit::Circuit;
use circuit::verify::Options;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

pub type P2 = (i32, i32);

const C1: P2 = (1, 0); // facing east: reads I (east), powers A (west)
const C2: P2 = (0, 1); // facing north: reads A (north), powers T (south)
const I: P2 = (2, 0);
const A: P2 = (0, 0);
const T: P2 = (0, 2);

fn adj(a: P2, b: P2) -> bool {
    (a.0 - b.0).abs() + (a.1 - b.1).abs() == 1
}

fn neighbors(p: P2) -> [P2; 4] {
    [(p.0 + 1, p.1), (p.0 - 1, p.1), (p.0, p.1 + 1), (p.0, p.1 - 1)]
}

/// One ring: its return line (from beside T to beside I), A and T as dust or stone, the input
/// cell, the output cell.
#[derive(Clone, Debug)]
pub struct Ring {
    pub line: Vec<P2>,
    pub a_dust: bool,
    pub t_dust: bool,
    pub input: P2,
    pub output: P2,
}

impl Ring {
    /// Blocks placed (supports included).
    pub fn blocks(&self) -> usize {
        // comparators + their supports, I, A (stone, or dust + support), T, the line + supports
        let a = if self.a_dust { 2 } else { 1 };
        let t = if self.t_dust { 2 } else { 1 };
        4 + 1 + a + t + 2 * self.line.len()
    }

    pub fn design(&self, name: &str) -> String {
        let mut cells: BTreeMap<P2, char> = BTreeMap::new();
        cells.insert(C1, 'E');
        cells.insert(C2, 'n');
        cells.insert(I, 'S');
        cells.insert(A, if self.a_dust { 'd' } else { 'S' });
        cells.insert(T, if self.t_dust { 'd' } else { 'S' });
        for p in &self.line {
            cells.insert(*p, 'd');
        }
        let all: Vec<P2> = cells.keys().copied().chain([self.input, self.output]).collect();
        let (x0, x1) = (all.iter().map(|p| p.0).min().unwrap(), all.iter().map(|p| p.0).max().unwrap());
        let (z0, z1) = (all.iter().map(|p| p.1).min().unwrap(), all.iter().map(|p| p.1).max().unwrap());
        let mut l0 = String::new();
        let mut l1 = String::new();
        for z in z0..=z1 {
            l0.push_str("  ");
            l1.push_str("  ");
            for x in x0..=x1 {
                let c = cells.get(&(x, z)).copied().unwrap_or('.');
                l1.push(c);
                l0.push(if matches!(c, 'E' | 'n' | 'd') { 'S' } else { '.' });
            }
            l0.push('\n');
            l1.push('\n');
        }
        format!(
            "name: {name}\ncircuit: off-delay\n# a comparator ring (crates/circuit-timer/src/rings.rs): {} dust carry C2's output back to C1, losing {} per round of 4 gt\norigin: {} 0 {}\nport input: {} 1 {}\nport output: {} 1 {}\npalette:\n  S minecraft:stone\n  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]\n  E minecraft:comparator[facing=east,mode=compare,powered=false]\n  n minecraft:comparator[facing=north,mode=compare,powered=false]\nlayer 0:\n{l0}layer 1:\n{l1}",
            self.line.len() + self.t_dust as usize,
            self.line.len() + self.t_dust as usize - 1,
            x0,
            z0,
            self.input.0,
            self.input.1,
            self.output.0,
            self.output.1
        )
    }
}

/// Every return line of `min..=max` dust inside the box: a simple path whose first cell is beside
/// T, whose last cell is beside I and points into it (in line with its predecessor, or alone in
/// line with T and I), no two cells adjacent out of turn, none on a ring cell or a comparator side.
pub fn lines(lo: P2, hi: P2, min: usize, max: usize) -> Vec<Vec<P2>> {
    let ring: HashSet<P2> = [C1, C2, I, A, T].into_iter().collect();
    // comparator sides: C1 (east-west) has (1,-1), (1,1); C2 (north-south) has (-1,1), (1,1)
    let sides: HashSet<P2> = [(1, -1), (1, 1), (-1, 1)].into_iter().collect();
    let inside = |p: P2| p.0 >= lo.0 && p.0 <= hi.0 && p.1 >= lo.1 && p.1 <= hi.1;
    let mut out = Vec::new();
    let mut path: Vec<P2> = Vec::new();
    fn rec(path: &mut Vec<P2>, out: &mut Vec<Vec<P2>>, ok: &dyn Fn(P2) -> bool, min: usize, max: usize) {
        let last = *path.last().unwrap();
        // a finished line: the last cell beside I, pointing into it
        if path.len() >= min && adj(last, I) {
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
        if path.len() >= max {
            return;
        }
        for n in neighbors(last) {
            if !ok(n) || path.contains(&n) {
                continue;
            }
            // no adjacency to earlier cells but the last one (a clean line)
            if path[..path.len() - 1].iter().any(|q| adj(*q, n)) {
                continue;
            }
            path.push(n);
            rec(path, out, ok, min, max);
            path.pop();
        }
    }
    let ok = |p: P2| inside(p) && !ring.contains(&p) && !sides.contains(&p);
    for s in neighbors(T) {
        if !ok(s) {
            continue;
        }
        path.push(s);
        rec(&mut path, &mut out, &ok, min, max);
        path.pop();
    }
    out
}

/// The judge's line of one ring on redsim: `Some((L, latency, line))` when it passes.
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
    Some((num("L")?, num("latency")?, rep.summary(&c)))
}

pub fn run(out: &Path, max_line: usize, workers: usize, version: &str) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let o = Options { version: version.to_string(), ..Default::default() };
    let mut rings: Vec<Ring> = Vec::new();
    for line in lines((-2, -1), (4, 5), 1, max_line) {
        let used: HashSet<P2> = line.iter().copied().chain([C1, C2, I, A, T]).collect();
        for a_dust in [false, true] {
            for t_dust in [false, true] {
                // input cells: beside exactly one line dust, beside no comparator, not used
                let mut inputs: Vec<P2> = Vec::new();
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
                // output cells: any free cell beside the ring (A, T, I, a line dust, a comparator):
                // the judge decides what powers its lamp
                let mut outputs: Vec<P2> = Vec::new();
                for src in used.iter() {
                    for n in neighbors(*src) {
                        if !used.contains(&n) && !outputs.contains(&n) {
                            outputs.push(n);
                        }
                    }
                }
                for &i in &inputs {
                    for &op in &outputs {
                        if op == i || adj(op, i) {
                            continue;
                        }
                        rings.push(Ring { line: line.clone(), a_dust, t_dust, input: i, output: op });
                    }
                }
            }
        }
    }
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
        let name = format!("off-delay-ring-L{l}");
        std::fs::write(out.join(format!("{name}.txt")), r.design(&name))?;
        println!("L {l} gt: {b} blocks, latency {lat} gt");
    }
    println!("{} passes, {} L values", found.len(), best.len());
    Ok(())
}
