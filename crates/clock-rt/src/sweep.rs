//! Sweeps: enumerate a parametric family, judge every member with `clock sim` (redsim, exact) and
//! keep, per period, the fewest blocks and every distinct output waveform.

use crate::families::{Cell, Head, Tail, ULoop};
use clock::Clock;
use clock::sim::{SimOptions, Verdict, simulate};
use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Clone, Debug)]
pub struct Found {
    pub label: String,
    pub period: u32,
    pub blocks: usize,
    pub volume: i32,
    /// Output on-ticks, rising edges and pulse widths.
    pub on: u32,
    pub edges: u32,
    pub widths: Vec<u32>,
    pub family: String,
    pub design: String,
}

impl Found {
    /// The waveform part of the key: on/P, edges, widths.
    pub fn wave(&self) -> String {
        format!("on {}/{} edges {} widths {:?}", self.on, self.period, self.edges, self.widths)
    }
}

/// Judge one design text with `clock sim`; `None` unless it is a clock.
pub fn judge(name: &str, text: &str, horizon: u32) -> Option<Found> {
    let d = mctas::design::Design::parse(text).ok()?;
    let c = Clock::from_design(d).ok()?;
    let r = simulate(&c, &SimOptions { horizon, margin: 4 });
    if r.verdict != Verdict::Clock {
        return None;
    }
    let o = r.output.as_ref()?;
    Some(Found {
        label: name.to_string(),
        period: r.period,
        blocks: r.blocks,
        volume: r.volume(),
        on: o.wave.on,
        edges: o.wave.edges,
        widths: o.wave.widths.clone(),
        family: clock::wave::signature(&r.analysis.family),
        design: text.to_string(),
    })
}

/// Every row of length k over `alphabet`.
fn rows(alphabet: &[Cell], k: usize) -> Vec<Vec<Cell>> {
    let mut out = vec![Vec::new()];
    for _ in 0..k {
        let mut next = Vec::new();
        for r in &out {
            for c in alphabet {
                let mut r2 = r.clone();
                r2.push(*c);
                next.push(r2);
            }
        }
        out = next;
    }
    out
}

/// The U-loops of row length 1..=kmax over the alphabet, with these heads and tails.
pub fn uloops(kmax: usize, alphabet: &[Cell], heads: &[Head], tails: &[Tail]) -> Vec<ULoop> {
    let mut v = Vec::new();
    for k in 1..=kmax {
        let rs = rows(alphabet, k);
        for h in heads {
            for tl in tails {
                for t in &rs {
                    for b in &rs {
                        v.push(ULoop { head: *h, top: t.clone(), bottom: b.clone(), tail: *tl });
                    }
                }
            }
        }
    }
    v
}

/// Judge every candidate in parallel.
pub fn run_all(cands: Vec<(String, String)>, horizon: u32, threads: usize) -> Vec<Found> {
    let out = Mutex::new(Vec::new());
    let next = Mutex::new(0usize);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        let i = *n;
                        *n += 1;
                        i
                    };
                    if i >= cands.len() {
                        break;
                    }
                    let (name, text) = &cands[i];
                    if let Some(f) = judge(name, text, horizon) {
                        out.lock().unwrap().push(f);
                    }
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by(|a, b| (a.period, a.blocks, a.volume, &a.label).cmp(&(b.period, b.blocks, b.volume, &b.label)));
    v
}

/// Per period: the best (fewest blocks, then volume) and the distinct waveforms (each with its
/// smallest design).
pub struct Table {
    pub best: BTreeMap<u32, Found>,
    pub waves: BTreeMap<(u32, String), Found>,
}

pub fn table(found: &[Found]) -> Table {
    let mut best: BTreeMap<u32, Found> = BTreeMap::new();
    let mut waves: BTreeMap<(u32, String), Found> = BTreeMap::new();
    for f in found {
        let better = |old: &Found| (f.blocks, f.volume) < (old.blocks, old.volume);
        match best.get(&f.period) {
            Some(old) if !better(old) => {}
            _ => {
                best.insert(f.period, f.clone());
            }
        }
        let key = (f.period, format!("{} | {}", f.wave(), f.family));
        match waves.get(&key) {
            Some(old) if !better(old) => {}
            _ => {
                waves.insert(key, f.clone());
            }
        }
    }
    Table { best, waves }
}
