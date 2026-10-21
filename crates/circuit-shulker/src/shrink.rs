//! `mctas circuit shulker shrink`: greedy block removal, judged in the game.

use anyhow::{Result, anyhow, bail};
use circuit::shulker::{Scenario, badge_scenarios, flow_rate, game_run, judge, scenario_check};
use circuit::verify::{Options, verify_in_pool};
use circuit::{Circuit, Pos};
use mctas::sim::{Pool, Worker, WorkerConfig};
use std::collections::BTreeSet;
use std::path::Path;

/// Whether `c` runs the plain scenario `sc` at rotation 0, passes, and keeps items/s >= `min`.
fn rate_ok(w: &mut Worker, c: &Circuit, sc: &Scenario, min: f64, margin: i32) -> bool {
    match game_run(w, c, sc, margin) {
        Ok(Ok(log)) => judge(sc, &log).pass && flow_rate(sc, &log).is_some_and(|r| r >= min),
        _ => false,
    }
}

/// Shrink `design` and write the result to `out` (only when at least one block went). With
/// `min_rate`, every removal also keeps the plain 64-stack scenario's items/s at or above it.
pub fn shrink(design: &Path, out: &Path, keep: &[String], min_rate: Option<f64>, workers: usize, version: &str, margin: i32) -> Result<()> {
    let c0 = Circuit::load(design)?;
    let kind = c0.kind.clone().ok_or_else(|| anyhow!("{}: no `circuit:` line", design.display()))?;
    let base = Scenario::base(&kind).ok_or_else(|| anyhow!("{}: {} is not a shulker kind", design.display(), kind.name()))?;
    // the search's scenarios: the plain one with unstackables (fast), then each kept badge
    let mut scen: Vec<(String, Scenario)> = vec![("base".into(), Scenario::with_stack(base.loader, 1))];
    let all = badge_scenarios(&c0, &base);
    for b in keep {
        if b == "always-on" {
            continue;
        }
        match all.iter().find(|(n, _)| n == b) {
            Some(s) => scen.push(s.clone()),
            None => bail!("badge {b}: not judged for this design (known: always-on, {})", all.iter().map(|s| s.0.as_str()).collect::<Vec<_>>().join(", ")),
        }
    }
    let pool = Pool::spawn(workers.max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let protected: BTreeSet<Pos> = c0.inputs.iter().map(|i| i.pos).collect();
    let mut cur = c0.clock.design.clone();
    let mut removed: Vec<String> = Vec::new();
    let without = |d: &mctas::design::Design, p: Pos| {
        let mut d = d.clone();
        d.blocks.remove(&p);
        d.block_entities.remove(&p);
        d
    };
    for round in 1.. {
        let cands: Vec<Pos> = cur.blocks.keys().filter(|p| !protected.contains(*p)).copied().collect();
        // stage 1: the plain scenario at rotation 0
        let first = pool.map(cands.clone(), |w, _, p| {
            Ok(match Circuit::from_design(without(&cur, p)) {
                Ok(c) => scenario_check(w, &c, "base", &scen[0].1, 0, margin).is_ok() && min_rate.is_none_or(|m| rate_ok(w, &c, &base, m, margin)),
                Err(_) => false,
            })
        });
        let alive: Vec<Pos> = cands.iter().zip(&first).filter(|(_, r)| matches!(r, Ok(true))).map(|(p, _)| *p).collect();
        // stage 2: the plain scenario at 4 rotations x 4 positions (the verdict's 16 builds, each
        // seeding the drop differently), every kept badge at 4 rotations
        let offsets = clock::offsets(4);
        let mut jobs: Vec<(usize, usize, u8, circuit::Pos)> = Vec::new();
        for i in 0..alive.len() {
            for t in 0..4u8 {
                for off in &offsets {
                    jobs.push((i, 0, t, *off));
                }
                for s in 1..scen.len() {
                    jobs.push((i, s, t, [0, 0, 0]));
                }
            }
        }
        let full = pool.map(jobs.clone(), |w, _, (i, s, t, off)| {
            let c = Circuit::from_design(without(&cur, alive[i]))?.translated(off)?;
            Ok(scenario_check(w, &c, &scen[s].0, &scen[s].1, t, margin).is_ok())
        });
        let mut ok = vec![true; alive.len()];
        for ((i, _, _, _), r) in jobs.iter().zip(&full) {
            if !matches!(r, Ok(true)) {
                ok[*i] = false;
            }
        }
        let Some(i) = (0..alive.len()).find(|i| ok[*i]) else {
            println!("round {round}: {} candidates, {} pass one rotation, none passes everything: done", cands.len(), alive.len());
            break;
        };
        let p = alive[i];
        let what = clock::short(cur.blocks.get(&p).map(|s| s.as_str()).unwrap_or("?")).to_string();
        removed.push(format!("{what} at {p:?}"));
        cur = without(&cur, p);
        println!("round {round}: removed {what} at {p:?} ({} candidates, {} pass one rotation): {} blocks", cands.len(), alive.len(), cur.blocks.len());
    }
    if removed.is_empty() {
        println!("NOTHING: no block of {} can go", c0.name());
        return Ok(());
    }
    cur.name = format!("{}-shrunk", c0.name());
    let text = format!(
        "# {} shrunk by `mctas circuit shulker shrink` (kept: plain{}{}): without {}.\n{}",
        c0.name(),
        keep.iter().map(|b| format!(", {b}")).collect::<String>(),
        min_rate.map(|m| format!(", items/s >= {m}")).unwrap_or_default(),
        removed.join(", "),
        cur.to_text()
    );
    std::fs::write(out, &text)?;
    // the judge's full verdict on the result
    let c = Circuit::load(out)?;
    let o = Options { positions: 4, rotations: true, workers, version: version.to_string(), margin, ..Default::default() };
    let rep = verify_in_pool(&pool, &c, &o)?;
    println!("{}", rep.summary(&c));
    println!("SHRUNK: {} -> {} blocks, {}", c0.blocks(), c.blocks(), out.display());
    Ok(())
}
