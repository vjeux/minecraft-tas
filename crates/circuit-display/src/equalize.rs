//! `mctas circuit display equalize <design> --below Y`: make every input reach the stack at the same
//! tick. For each input, the first frame after its rise where any block below y = Y changes; the
//! faster inputs get their input repeater (the input cell, then the repeaters behind it along the
//! line) slowed by the difference, so a complement transition moves s in one step.

use anyhow::{Result, anyhow, bail};
use circuit::Circuit;
use mctas::design::Design;
use std::collections::HashMap;
use std::path::Path;

fn arrival(c: &Circuit, names: &[String], input: &str, below: i32) -> Result<u32> {
    let s = circuit::stimulus::parse(&format!("{input} 20+"), names, 100)?;
    let t = circuit::run::sim_trace(c, &s, 3).map_err(|e| anyhow!("{e}"))?;
    let base: HashMap<[i32; 3], u32> = t.cells[19].iter().filter(|(p, _)| p[1] < below).map(|(p, i)| (*p, *i)).collect();
    for f in 20..t.cells.len() {
        let now: HashMap<[i32; 3], u32> = t.cells[f].iter().filter(|(p, _)| p[1] < below).map(|(p, i)| (*p, *i)).collect();
        if now != base {
            return Ok(f as u32 - 20);
        }
    }
    bail!("{input} never reaches below y = {below}")
}

pub fn run(design: &Path, out: &Path, below: i32) -> Result<()> {
    let text = std::fs::read_to_string(design)?;
    let mut d = Design::parse(&text)?;
    for _round in 0..4 {
        std::fs::write(out, d.to_text())?;
        let c = Circuit::load(out)?;
        let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
        let times: Vec<u32> = names.iter().map(|n| arrival(&c, &names, n, below)).collect::<Result<_>>()?;
        println!("arrivals {:?}", names.iter().zip(&times).collect::<Vec<_>>());
        let max = *times.iter().max().unwrap();
        if times.iter().all(|&t| t == max) {
            return Ok(());
        }
        for (i, inp) in c.inputs.iter().enumerate() {
            let mut need = (max - times[i]) / 2;
            // walk the input line: the input cell, then the repeater it faces into, ...
            let mut p = inp.pos;
            while need > 0 {
                let Some(st) = d.blocks.get(&p).cloned() else { break };
                if !st.contains("repeater") {
                    break;
                }
                let delay: u32 = st.split("delay=").nth(1).and_then(|r| r[..1].parse().ok()).unwrap_or(1);
                let add = need.min(4 - delay);
                let nst = st.replacen(&format!("delay={delay}"), &format!("delay={}", delay + add), 1);
                d.blocks.insert(p, nst);
                need -= add;
                // the next repeater: the cell this one faces (its output side)
                let facing = st.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
                let v = match facing {
                    "north" => [0, 0, 1],
                    "south" => [0, 0, -1],
                    "west" => [1, 0, 0],
                    _ => [-1, 0, 0],
                };
                p = [p[0] + v[0], p[1], p[2] + v[2]];
            }
            if need > 0 {
                bail!("input {} needs {need} more ticks of delay", inp.name);
            }
        }
    }
    std::fs::write(out, d.to_text())?;
    Ok(())
}
