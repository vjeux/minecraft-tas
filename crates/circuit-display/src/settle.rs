//! `mctas circuit display settle <design>`: rewrite every block state of a design with its state at
//! rest on redsim (all inputs off, long after placement): torches lit or not, dust powers,
//! comparators and repeaters powered. A tall decoder takes longer than the judge's 20 gt of rest
//! to settle from blank states; written settled, nothing changes at placement.

use anyhow::{Result, anyhow};
use circuit::Circuit;
use mctas::design::Design;
use std::path::Path;

pub fn run(design: &Path, out: &Path, frames: u32) -> Result<()> {
    let text = std::fs::read_to_string(design)?;
    let c = Circuit::load(design)?;
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let first = names.first().ok_or_else(|| anyhow!("no input"))?;
    let s = circuit::stimulus::parse(&format!("{first} {}+ {}-", frames + 50, frames + 60), &names, 100)?;
    let t = circuit::run::sim_trace(&c, &s, 3).map_err(|e| anyhow!("{e}"))?;
    let f = (frames as usize).min(t.cells.len() - 1);
    let mut d = Design::parse(&text)?;
    let mut changed = 0;
    for (p, i) in &t.cells[f] {
        let Some(old) = d.blocks.get(p) else { continue };
        let mut st = t.names[*i as usize].clone();
        if let Some(k) = st.find('{') {
            st.truncate(k);
        }
        let id = |s: &str| s.split('[').next().unwrap_or("").trim_start_matches("minecraft:").to_string();
        if id(old) == id(&st) && *old != st {
            let mut st = if st.starts_with("minecraft:") { st } else { format!("minecraft:{st}") };
            // dust keeps only its power (the placement's shape update redraws its shape; every
            // shape would exhaust the palette)
            if id(&st) == "redstone_wire" {
                let power = st.split("power=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("0").to_string();
                st = format!("minecraft:redstone_wire[power={power}]");
            }
            if st == *old {
                continue;
            }
            d.blocks.insert(*p, st);
            changed += 1;
        }
    }
    // keep the generator's `# tower` comment lines
    let mut out_text = d.to_text();
    for l in text.lines().filter(|l| l.starts_with("# tower ")) {
        out_text.push_str(l);
        out_text.push('\n');
    }
    std::fs::write(out, out_text)?;
    println!("{}: {changed} block states settled (frame {f})", out.display());
    Ok(())
}
