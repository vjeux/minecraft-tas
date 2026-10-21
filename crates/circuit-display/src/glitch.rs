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
    // the transition comes after the start state settles; the window after it covers the slowest
    // designs (the tower's lanes: latency ~120 gt)
    let (t0, t1, t2) = (10u32, 300u32, 520u32);
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
        let s = circuit::stimulus::parse(&parts.join("; "), &names, t2 - t1)?;
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
                    wave.push_str(&format!(" {}{}", f - t1, if v { "+" } else { "-" }));
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

/// `--judge`: the judge's own stimulus (circuit::check::stimulus, the redsim sizing's gap), each
/// phase whose output changes more often than its ideal (the judge's `glitches` count,
/// compute::check) with the phase's transition and the outputs' waveforms.
pub fn run_judge(design: &Path) -> Result<Vec<String>> {
    let c = Circuit::load(design)?;
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no circuit kind"))?;
    let gap = circuit::verify::sizing(&c, 3).ok_or_else(|| anyhow!("redsim sizing failed"))?;
    let s = circuit::check::stimulus(&c, &kind, &circuit::check::Sizing { gap });
    let t = circuit::run::sim_trace(&c, &s, 3).map_err(|e| anyhow!("{e}"))?;
    let outs: Vec<char> = c.outputs.iter().map(|o| o.name.chars().last().unwrap_or('?')).collect();
    let on = |v: u32, ch: char| SEGMENTS[v as usize].contains(ch);
    let lvl = |k: usize, f: usize| t.levels[k].get(f).copied().unwrap_or(0) > 0;
    let mut prev = 0u32;
    let mut found: Vec<String> = Vec::new();
    for (a, b, label) in &s.phases {
        let (a, b) = (*a as usize, (*b as usize).min(t.levels[0].len()));
        let x: u32 = label.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        if a >= b {
            break;
        }
        let mut bad = Vec::new();
        for (k, ch) in outs.iter().enumerate() {
            let mut last = if a > 0 { lvl(k, a - 1) } else { false };
            let mut wave = String::new();
            let mut changes = 0;
            for f in a..b {
                if lvl(k, f) != last {
                    last = !last;
                    changes += 1;
                    wave.push_str(&format!(" {}{}", f - a, if last { "+" } else { "-" }));
                }
            }
            if changes > usize::from(on(prev, *ch) != on(x, *ch)) {
                bad.push(format!("{ch}{wave}"));
            }
        }
        if !bad.is_empty() {
            found.push(format!("phase {label:?} at {a} ({prev:04b} -> {x:04b}): {}", bad.join(" | ")));
        }
        prev = x;
    }
    found.push(format!("{} glitching phases of {} (gap {gap} gt)", found.len(), s.phases.len()));
    Ok(found)
}
