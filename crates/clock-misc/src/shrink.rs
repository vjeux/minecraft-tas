//! `mctas clock misc shrink DESIGNS...`: greedy block removal for clocks, judged in the real game
//! (coordinator 2026-10-20, after vjeux found a lever the detector rail made redundant: "so no
//! other redundant block survives").
//!
//! Each round tries removing every block of the design (never the start kick's block), screens the
//! removal on redsim when redsim models the clock (still a clock with the same period: a cheap
//! filter), judges the survivors in the game (`clock verify`: a robust CLOCK with the same period
//! and the same number of output edges per period) and keeps the first that passes; then repeats.
//! A removal that changes the period or the pulse count is another clock, not a smaller one.

use anyhow::Result;
use clap::Args as ClapArgs;
use clock::Clock;
use clock::sim::{SimOptions, Verdict, simulate};
use clock::verify::{VerifyOptions, VerifyReport, spawn_pool, verify_in_pool};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct ShrinkArgs {
    pub designs: Vec<PathBuf>,
    /// Write each shrunk design here (`<name>-<blocks>b.txt`).
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long, default_value_t = 4)]
    pub positions: usize,
    #[arg(long, default_value_t = 8)]
    pub workers: usize,
}

/// What a smaller version must keep: a robust clock's period and output edges per period.
fn identity(rep: &VerifyReport) -> Option<(u32, u32)> {
    if rep.verdict != Verdict::Clock || rep.robustness != "robust" {
        return None;
    }
    rep.base().map(|b| (b.period, b.output.as_ref().map(|o| o.wave.edges).unwrap_or(0)))
}

pub fn run(a: ShrinkArgs) -> Result<()> {
    let o = VerifyOptions { positions: a.positions, workers: a.workers, ..Default::default() };
    let pool = spawn_pool(&o)?;
    if let Some(dir) = &a.out {
        std::fs::create_dir_all(dir)?;
    }
    for path in &a.designs {
        let c0 = Clock::load(path)?;
        let rep0 = verify_in_pool(&pool, &c0, &o)?;
        let Some(want) = identity(&rep0) else {
            println!("SKIP {}: not a robust clock in the game: {}", c0.name(), rep0.summary());
            continue;
        };
        let sim0 = simulate(&c0, &SimOptions::default());
        let screen = sim0.verdict == Verdict::Clock;
        let kick = match &c0.kick {
            Some(clock::Kick::Last { pos }) | Some(clock::Kick::Use { pos, .. }) => Some(*pos),
            _ => None,
        };
        println!("{}: {} blocks, period {} gt, {} edges{}", c0.name(), c0.blocks(), want.0, want.1, if screen { "" } else { " (redsim does not model it: every removal judged in the game)" });
        let mut cur = c0.clone();
        let mut removed: Vec<String> = Vec::new();
        loop {
            let cands: Vec<clock::Pos> = cur.design.blocks.keys().filter(|p| Some(**p) != kick).copied().collect();
            let mut hit = None;
            for p in cands {
                let mut d = cur.design.clone();
                d.blocks.remove(&p);
                d.block_entities.remove(&p);
                let Ok(c) = Clock::from_design(d) else { continue };
                if screen {
                    let s = simulate(&c, &SimOptions::default());
                    if s.verdict != Verdict::Clock || s.period != sim0.period {
                        continue;
                    }
                }
                let Ok(rep) = verify_in_pool(&pool, &c, &o) else { continue };
                if identity(&rep) == Some(want) {
                    hit = Some((p, c));
                    break;
                }
            }
            let Some((p, c)) = hit else { break };
            let what = clock::short(cur.design.blocks.get(&p).map(|s| s.as_str()).unwrap_or("?")).to_string();
            println!("  removed {what} at {p:?}: {} blocks", c.blocks());
            removed.push(format!("{what} at {p:?}"));
            cur = c;
        }
        if removed.is_empty() {
            println!("NOTHING: no block of {} can go ({} blocks)", c0.name(), c0.blocks());
            continue;
        }
        println!("SHRUNK: {} {} -> {} blocks, without {}", c0.name(), c0.blocks(), cur.blocks(), removed.join(", "));
        if let Some(dir) = &a.out {
            let text = format!("# {} shrunk by `mctas clock misc shrink` (same period and edges in the game): without {}.\n{}", c0.name(), removed.join(", "), cur.to_text());
            std::fs::write(dir.join(format!("{}-{}b.txt", c0.name(), cur.blocks())), text)?;
        }
    }
    Ok(())
}
