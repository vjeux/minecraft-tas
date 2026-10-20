//! `mctas circuit timer cells <design> --stimulus ...`: a design's run on redsim, every cell that
//! changes printed per frame (a design aid: why does a ring not charge, when does a piston land).

use anyhow::{Result, anyhow};
use circuit::Circuit;
use std::collections::HashMap;
use std::path::Path;

pub fn run(design: &Path, stimulus: &str, from: u32, to: u32, margin: i32) -> Result<()> {
    let c = Circuit::load(design)?;
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let s = circuit::stimulus::parse(stimulus, &names, 100)?;
    let t = circuit::run::sim_trace(&c, &s, margin).map_err(|e| anyhow!("{e}"))?;
    let mut prev: HashMap<[i32; 3], u32> = HashMap::new();
    for (f, cells) in t.cells.iter().enumerate() {
        let now: HashMap<[i32; 3], u32> = cells.iter().copied().collect();
        let mut changes: Vec<String> = Vec::new();
        for (p, i) in &now {
            if prev.get(p) != Some(i) {
                changes.push(format!("{},{},{} {}", p[0], p[1], p[2], t.names[*i as usize].trim_start_matches("minecraft:")));
            }
        }
        for p in prev.keys() {
            if !now.contains_key(p) {
                changes.push(format!("{},{},{} air", p[0], p[1], p[2]));
            }
        }
        changes.sort();
        let f = f as u32;
        if f >= from && f <= to && !changes.is_empty() {
            let ins: String = t.inputs.iter().map(|v| if v.get(f as usize) == Some(&true) { '#' } else { '.' }).collect();
            let outs: String = t.levels.iter().map(|v| v.get(f as usize).map(|l| format!("{l:x}")).unwrap_or_default()).collect();
            println!("t={f} in {ins} out {outs}: {}", changes.join("; "));
        }
        prev = now;
    }
    Ok(())
}
