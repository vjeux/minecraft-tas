//! `mctas circuit display check <design>`: hold each of the 16 values on redsim and compare every
//! torch of every tower (the `# tower` lines the towers generator writes) with the value the
//! generator meant it to carry; prints the lowest wrong torch of each tower, and the outputs.

use anyhow::{Result, anyhow};
use circuit::Circuit;
use circuit::compute::SEGMENTS;
use std::collections::HashMap;
use std::path::Path;

struct T {
    label: String,
    x: i32,
    z: i32,
    b: i32,
    top: i32,
    s: u16,
}

fn value(t: &T, l: i32) -> u16 {
    let o = (l - t.b).rem_euclid(4);
    if o == 0 || o == 3 { t.s } else { !t.s }
}

pub fn run(design: &Path, hold: u32) -> Result<()> {
    let text = std::fs::read_to_string(design)?;
    let mut towers = Vec::new();
    for l in text.lines() {
        let Some(rest) = l.strip_prefix("# tower ") else { continue };
        let w: Vec<&str> = rest.split_whitespace().collect();
        let n = |i: usize| -> Result<i32> { w.get(i).ok_or_else(|| anyhow!("bad tower line {l:?}"))?.parse::<i32>().map_err(|e| anyhow!("{e}")) };
        towers.push(T { label: w[0].to_string(), x: n(1)?, z: n(2)?, b: n(3)?, top: n(4)?, s: u16::from_str_radix(w.get(5).ok_or_else(|| anyhow!("bad tower line"))?, 16)? });
    }
    let c = Circuit::load(design)?;
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    for v in 0..16u32 {
        let mut parts: Vec<String> = (0..4).filter(|k| v >> k & 1 == 1).map(|k| format!("a{k} 10+")).collect();
        if parts.is_empty() {
            parts.push(format!("a0 {}+ {}-", hold + 50, hold + 60));
        }
        let s = circuit::stimulus::parse(&parts.join("; "), &names, 100)?;
        let t = circuit::run::sim_trace(&c, &s, 3).map_err(|e| anyhow!("{e}"))?;
        let f = (hold as usize).min(t.cells.len() - 1);
        let cells: HashMap<[i32; 3], &str> = t.cells[f].iter().map(|(p, i)| (*p, t.names[*i as usize].as_str())).collect();
        let outs: String = t.levels.iter().map(|l| if l.get(f).copied().unwrap_or(0) > 0 { '#' } else { '.' }).collect();
        let want: String = "abcdefg".chars().map(|ch| if SEGMENTS[v as usize].contains(ch) { '#' } else { '.' }).collect();
        let mut bad = Vec::new();
        for tw in &towers {
            for l in tw.b..=tw.top {
                if (l - tw.b) % 2 == 0 {
                    continue;
                }
                let want = value(tw, l) >> v & 1 == 1;
                let st = cells.get(&[tw.x, l, tw.z]).copied().unwrap_or("air");
                let lit = st.contains("lit=true");
                if !st.contains("torch") || lit != want {
                    bad.push(format!("{} ({},{}) torch {l}: {st}, want lit={want}", tw.label, tw.x, tw.z));
                    break;
                }
            }
        }
        println!("value {v:x}: out {outs} want {want}{}", if outs == want { "" } else { "  WRONG" });
        for b in bad {
            println!("  {b}");
        }
    }
    Ok(())
}
