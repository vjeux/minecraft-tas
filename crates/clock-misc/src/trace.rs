//! `mctas clock misc trace <design> [--ticks N] [--every K] [--margin M]`: build a design in the
//! real game exactly like `clock verify` (blocks, block entities, then the design's `entity:`
//! lines, then the kick), then step it one tick at a time and print, per tick, every entity's
//! position and motion and the cells of the watched box that changed. For designs with
//! entities (minecarts, boats), whose motion `clock sim` cannot show.

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use clock::Clock;
use mctas::sim::{SandboxOptions, Worker, WorkerConfig};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct TraceArgs {
    pub design: PathBuf,
    /// Ticks to run.
    #[arg(long, default_value_t = 200)]
    pub ticks: u32,
    /// Print entities every K ticks (block changes are printed every tick).
    #[arg(long, default_value_t = 1)]
    pub every: u32,
    /// Margin of the watched box around the design.
    #[arg(long, default_value_t = 3)]
    pub margin: i32,
    /// Print block changes only for these block-state prefixes (comma-separated, e.g.
    /// `oak_pressure_plate,detector_rail`); default all.
    #[arg(long, default_value = "")]
    pub only: String,
}

fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

pub fn run(a: TraceArgs) -> Result<()> {
    let c = Clock::load(&a.design).with_context(|| format!("load {}", a.design.display()))?;
    let (lo, hi) = c.bounds();
    let m = a.margin;
    let from = [lo[0] - m, lo[1] - m, lo[2] - m];
    let to = [hi[0] + m, hi[1] + m, hi[2] + m];
    let mut w = Worker::spawn(&WorkerConfig::default())?;
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    clock::verify::place(&mut w, sb, &c)?;
    let only: Vec<&str> = a.only.split(',').filter(|s| !s.is_empty()).collect();
    let mut prev: HashMap<[i32; 3], String> = w.get_region(sb, from, to)?.iter().map(|(p, s)| (p, s.to_string())).collect();
    let mut actions = c.actions();
    for t in 0..=a.ticks {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, act) = actions.remove(0);
            clock::verify::apply(&mut w, sb, &act)?;
        }
        if t > 0 {
            w.step(sb, 1)?;
        }
        let region = w.get_region(sb, from, to)?;
        let mut changes = Vec::new();
        for (p, s) in region.iter() {
            if prev.get(&p).map(|x| x.as_str()) != Some(s) {
                if only.is_empty() || only.iter().any(|o| short(s).starts_with(o) || prev.get(&p).is_some_and(|x| short(x).starts_with(o))) {
                    changes.push(format!("{},{},{} {}", p[0], p[1], p[2], short(s)));
                }
                prev.insert(p, s.to_string());
            }
        }
        let ents = if t % a.every == 0 { w.entities(sb)? } else { Vec::new() };
        let e: Vec<String> = ents
            .iter()
            .map(|e| {
                format!(
                    "{} ({:.4}, {:.4}, {:.4}) v ({:.4}, {:.4}, {:.4}){}",
                    short(&e.kind),
                    e.pos[0],
                    e.pos[1],
                    e.pos[2],
                    e.motion[0],
                    e.motion[1],
                    e.motion[2],
                    if e.design { "" } else { " [not the design's]" }
                )
            })
            .collect();
        if !changes.is_empty() || !e.is_empty() {
            println!("t={t}: {}{}{}", e.join("; "), if !e.is_empty() && !changes.is_empty() { " | " } else { "" }, changes.join("; "));
        }
    }
    let _ = w.drop_sandbox(sb);
    Ok(())
}
