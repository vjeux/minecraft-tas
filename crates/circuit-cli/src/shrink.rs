//! `mctas circuit shrink`: greedy block removal judged in the real game, for ANY circuit kind
//! (the shulker track has its own, which judges badges instead of the kind's metrics).
//!
//! Each round tries removing every block of the design, keeps the first removal whose judge
//! verdict still passes with the SAME metrics as the design started with, and repeats. A design
//! whose metrics change is not a smaller version of it: a 3-block gate that lost its glitch
//! freedom, or a delay line that now delays less, is another design.

use anyhow::{Result, anyhow};
use circuit::verify::{Options, verify_in_pool};
use circuit::{Circuit, Pos};
use std::collections::BTreeSet;
use std::path::Path;

/// The metrics line a verdict reports (`function xor, latency 4 gt`), the identity a shrink keeps.
fn metrics(rep: &circuit::verify::VerifyReport) -> Option<String> {
    rep.base().map(|b| b.outcome.metrics_text())
}

/// Shrink `design` into `out`: greedy removal, every candidate judged in the game.
pub fn shrink(design: &Path, out: &Path, keep_ports: bool, o: &Options) -> Result<()> {
    let c0 = Circuit::load(design)?;
    let pool = circuit::verify::spawn_pool(o)?;
    let rep0 = verify_in_pool(&pool, &c0, o)?;
    let want = metrics(&rep0).ok_or_else(|| anyhow!("{}: the design does not pass the judge, nothing to shrink", design.display()))?;
    if !rep0.pass {
        return Err(anyhow!("{}: {}", design.display(), rep0.verdict));
    }
    println!("{}: {} blocks, {want}", c0.name(), c0.blocks());
    // ports stay: an input cell's receiving block and an output's lamp cell are the interface
    let ports: BTreeSet<Pos> = c0.inputs.iter().map(|i| i.pos).chain(c0.outputs.iter().map(|x| x.pos)).collect();
    let mut cur = c0.clock.design.clone();
    let mut removed: Vec<String> = Vec::new();
    for round in 1.. {
        let cands: Vec<Pos> = cur.blocks.keys().filter(|p| !keep_ports || !ports.contains(*p)).copied().collect();
        let mut hit = None;
        for p in cands {
            let mut d = cur.clone();
            d.blocks.remove(&p);
            d.block_entities.remove(&p);
            let Ok(c) = Circuit::from_design(d) else { continue };
            let Ok(rep) = verify_in_pool(&pool, &c, o) else { continue };
            if rep.pass && metrics(&rep).as_deref() == Some(want.as_str()) {
                hit = Some((p, c));
                break;
            }
        }
        let Some((p, c)) = hit else {
            println!("round {round}: nothing more can go: {} blocks", cur.blocks.len());
            break;
        };
        let what = clock::short(cur.blocks.get(&p).map(|s| s.as_str()).unwrap_or("?")).to_string();
        println!("round {round}: removed {what} at {p:?}: {} blocks", c.blocks());
        removed.push(format!("{what} at {p:?}"));
        cur = c.clock.design;
    }
    if removed.is_empty() {
        println!("NOTHING: no block of {} can go", c0.name());
        return Ok(());
    }
    let c = Circuit::from_design(cur)?;
    let text = format!("# {} shrunk by `mctas circuit shrink` (same verdict: {want}): without {}.\n{}", c0.name(), removed.join(", "), c.clock.to_text());
    std::fs::write(out, &text)?;
    println!("SHRUNK: {} -> {} blocks, {}", c0.blocks(), c.blocks(), out.display());
    Ok(())
}
