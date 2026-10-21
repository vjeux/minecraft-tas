//! `mctas circuit display glitches <design>`: every single-bit transition and every complement
//! transition of a seven-segment decoder on redsim, each from its settled start state; prints the
//! transitions where an output changes more often than its ideal, with that output's waveform, and
//! the counts per segment and per changed bit set.

use anyhow::{Result, anyhow};
use circuit::Circuit;
use circuit::compute::SEGMENTS;
use std::collections::BTreeMap;
use std::path::Path;

pub fn run(design: &Path, verbose: bool) -> Result<()> {
    let c = Circuit::load(design)?;
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let outs: Vec<char> = c.outputs.iter().map(|o| o.name.chars().last().unwrap_or('?')).collect();
    let mut trans: Vec<(u32, u32)> = Vec::new();
    for p in 0..16u32 {
        for k in 0..4 {
            trans.push((p, p ^ (1 << k)));
        }
        trans.push((p, p ^ 15));
    }
    let (t0, t1, t2) = (10u32, 150u32, 260u32);
    let mut total = 0;
    let mut per_seg: BTreeMap<char, usize> = BTreeMap::new();
    let mut per_bits: BTreeMap<u32, usize> = BTreeMap::new();
    for (p, x) in trans {
        let mut parts = Vec::new();
        for k in 0..4 {
            let a = p >> k & 1 == 1;
            let b = x >> k & 1 == 1;
            let mut e = String::new();
            if a {
                e.push_str(&format!(" {t0}+"));
            }
            if a != b {
                e.push_str(&format!(" {t1}{}", if b { "+" } else { "-" }));
            }
            if !e.is_empty() {
                parts.push(format!("a{k}{e}"));
            }
        }
        if parts.is_empty() {
            continue;
        }
        let s = circuit::stimulus::parse(&parts.join("; "), &names, 100)?;
        let t = circuit::run::sim_trace(&c, &s, 3).map_err(|e| anyhow!("{e}"))?;
        let t2 = t2.min(t.levels[0].len() as u32);
        let mut bad = Vec::new();
        for (k, ch) in outs.iter().enumerate() {
            let on = |v: u32| SEGMENTS[v as usize].contains(*ch);
            let lvl = |f: u32| t.levels[k].get(f as usize).copied().unwrap_or(0) > 0;
            let mut changes = 0;
            let mut last = lvl(t1 - 1);
            let mut wave = String::new();
            for f in t1..t2 {
                let v = lvl(f);
                if v != last {
                    changes += 1;
                    last = v;
                }
                if f < t1 + 60 {
                    wave.push(if v { '#' } else { '.' });
                }
            }
            if lvl(t1 - 1) != on(p) || last != on(x) {
                bad.push(format!("{ch} WRONG(start {} want {}, end {} want {}) {wave}", lvl(t1 - 1), on(p), last, on(x)));
            } else if changes > usize::from(on(p) != on(x)) {
                bad.push(format!("{ch} {wave}"));
                *per_seg.entry(*ch).or_insert(0) += 1;
            }
        }
        if !bad.is_empty() {
            total += 1;
            *per_bits.entry(p ^ x).or_insert(0) += 1;
            if verbose {
                println!("{p:04b} -> {x:04b}: {}", bad.join(" | "));
            }
        }
    }
    println!("{total} glitching transitions of 80; per segment {per_seg:?}; per changed bits {per_bits:?}");
    Ok(())
}
