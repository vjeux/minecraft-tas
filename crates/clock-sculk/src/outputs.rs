//! `mctas clock sculk outputs`: the ruling "a clock's period is its output's period" (the
//! coordinator, 16:10Z). For each design: the period P of the whole state (`clock::sim`), then the
//! smallest period of every changing cell over one period; the output (declared, or the judge's
//! automatic one: the cell with the most rising edges) must have the full period P. A cell with
//! a shorter period is a fast part of the clock (e.g. an observer pair that a sensor divides);
//! with `--fix` the output is moved to a sculk sensor (else any cell) whose own period is P. A
//! design where no cell has period P is composite (its parts cycle independently): reported.

use anyhow::{Context, Result};
use clock::Pos;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct OutputsArgs {
    files: Vec<PathBuf>,
    /// Rewrite `port output:` where the output's own period is shorter than the clock's.
    #[arg(long)]
    fix: bool,
}

#[derive(clap::Args)]
pub struct ReduceArgs {
    files: Vec<PathBuf>,
    /// Write the reduced designs here (same file names; unreducible ones are copied).
    #[arg(long)]
    out_dir: PathBuf,
}

/// Greedy reduction: remove any block (not the kick's) whose removal keeps a clock of the same
/// period with a cell of that full period (not composite); repeat until none can go.
pub fn reduce_clock(c: &clock::Clock) -> Option<clock::Clock> {
    let o = clock::sim::SimOptions { horizon: 20_000, margin: 4 };
    let r = clock::sim::simulate(c, &o);
    if r.verdict != clock::sim::Verdict::Clock {
        return None;
    }
    let p = r.period;
    let kick = c.kick.as_ref().map(|k| k.pos());
    let mut cur = c.clone();
    'outer: loop {
        let cells: Vec<Pos> = cur.design.blocks.keys().copied().collect();
        for q in cells {
            if Some(q) == kick {
                continue;
            }
            let mut d = cur.design.clone();
            d.blocks.remove(&q);
            if d.ports.get("output") == Some(&q) {
                d.ports.remove("output");
            }
            let Ok(c2) = clock::Clock::from_design(d) else { continue };
            let r2 = clock::sim::simulate(&c2, &o);
            if r2.verdict == clock::sim::Verdict::Clock && r2.period == p && has_full_period_cell(&c2) == Some(true) {
                cur = c2;
                continue 'outer;
            }
        }
        break;
    }
    Some(cur)
}

pub fn reduce(a: &ReduceArgs) -> Result<()> {
    std::fs::create_dir_all(&a.out_dir)?;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(64);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(f) = a.files.get(i) else { break };
                let Ok(c) = clock::Clock::load(f) else { continue };
                let Some(red) = reduce_clock(&c) else { continue };
                let (b0, b1) = (c.design.blocks.len(), red.design.blocks.len());
                let text = std::fs::read_to_string(f).unwrap_or_default();
                let comments: Vec<&str> = text.lines().filter(|l| l.starts_with('#')).collect();
                let mut out = comments.join("\n");
                if !out.is_empty() {
                    out.push('\n');
                }
                if b1 < b0 {
                    out.push_str(&format!("# reduced from {b0} to {b1} blocks by `mctas clock sculk reduce` (same period, not composite)\n"));
                }
                out.push_str(&red.design.to_text());
                let Some(name) = f.file_name() else { continue };
                if std::fs::write(a.out_dir.join(name), out).is_ok() && b1 < b0 {
                    println!("{}: {b0} -> {b1} blocks", f.display());
                }
            });
        }
    });
    Ok(())
}

/// Per-tick states of every cell from `clock::sim::trace` (t = 0..=ticks).
fn states(c: &clock::Clock, ticks: u32) -> Vec<HashMap<Pos, String>> {
    let lines = clock::sim::trace(c, &clock::sim::SimOptions::default(), ticks);
    let mut cur: HashMap<Pos, String> = HashMap::new();
    let mut out = Vec::new();
    for l in lines {
        let Some(rest) = l.split_once("): ").map(|(_, r)| r) else { continue };
        if rest != "-" {
            for ch in rest.split("; ") {
                let Some((p, s)) = ch.split_once(' ') else { continue };
                let v: Vec<i32> = p.split(',').filter_map(|t| t.parse().ok()).collect();
                if v.len() == 3 {
                    cur.insert([v[0], v[1], v[2]], s.to_string());
                }
            }
        }
        out.push(cur.clone());
    }
    out
}

/// The smallest d dividing `p` with seq[t] == seq[(t + d) % p] for all t.
fn min_period(seq: &[&str]) -> usize {
    let p = seq.len();
    (1..=p).find(|d| p % d == 0 && (0..p).all(|t| seq[t] == seq[(t + d) % p])).unwrap_or(p)
}

/// The clock's period and whether some cell has that full period (false = composite: its parts
/// cycle with shorter periods only); `None` unless it is a clock on redsim.
pub fn has_full_period_cell(c: &clock::Clock) -> Option<bool> {
    let r = clock::sim::simulate(c, &clock::sim::SimOptions::default());
    if r.verdict != clock::sim::Verdict::Clock {
        return None;
    }
    let (p, w) = (r.period as usize, r.warmup as usize);
    let st = states(c, (w + 2 * p) as u32);
    if st.len() < w + p {
        return None;
    }
    let mut cells: Vec<Pos> = st[w..w + p].iter().flat_map(|m| m.keys().copied()).collect();
    cells.sort();
    cells.dedup();
    Some(cells.iter().any(|q| {
        let seq: Vec<&str> = (0..p).map(|t| st[w + t].get(q).map(|s| s.as_str()).unwrap_or("air")).collect();
        min_period(&seq) == p
    }))
}

pub fn outputs(a: &OutputsArgs) -> Result<()> {
    for f in &a.files {
        let c = clock::Clock::load(f)?;
        let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
        if r.verdict != clock::sim::Verdict::Clock {
            println!("{}: not a clock on redsim ({})", f.display(), r.verdict.word());
            continue;
        }
        let (p, w) = (r.period as usize, r.warmup as usize);
        let st = states(&c, (w + 2 * p) as u32);
        if st.len() < w + p {
            println!("{}: trace too short", f.display());
            continue;
        }
        let mut cells: Vec<Pos> = st[w..w + p].iter().flat_map(|m| m.keys().copied()).collect();
        cells.sort();
        cells.dedup();
        let period_of = |pos: Pos| -> usize {
            let seq: Vec<&str> = (0..p).map(|t| st[w + t].get(&pos).map(|s| s.as_str()).unwrap_or("air")).collect();
            min_period(&seq)
        };
        let cur = r.output.as_ref().map(|o| o.pos);
        let cur_p = cur.map(period_of).unwrap_or(0);
        if cur_p == p {
            println!("{}: OK P={p}, output {:?} has the full period", f.display(), cur.unwrap());
            continue;
        }
        let full: Vec<Pos> = cells.iter().copied().filter(|q| period_of(*q) == p).collect();
        // A sculk sensor first, then a cell that always holds a component (never air or a moving
        // block: the judge reads power at those), then anything.
        let always_block = |q: &Pos| (0..p).all(|t| st[w + t].get(q).is_some_and(|s| s != "air" && !s.contains("moving_piston") && !s.contains("piston_head")));
        let pick = full
            .iter()
            .copied()
            .find(|q| st[w].get(q).is_some_and(|s| s.contains("sculk_sensor")))
            .or(full.iter().copied().find(|q| always_block(q)))
            .or(full.first().copied());
        let Some(q) = pick else {
            println!("{}: COMPOSITE P={p}: no cell has the full period (output {:?} has {cur_p})", f.display(), cur);
            continue;
        };
        println!("{}: P={p}, output {:?} has period {cur_p}; {:?} ({}) has the full period{}", f.display(), cur, q, st[w].get(&q).map(|s| s.as_str()).unwrap_or("air"), if a.fix { " -> fixed" } else { "" });
        if a.fix {
            let text = std::fs::read_to_string(f)?;
            let line = format!("port output: {} {} {}", q[0], q[1], q[2]);
            let mut out: Vec<String> = Vec::new();
            let mut done = false;
            for l in text.lines() {
                if l.starts_with("port output:") {
                    out.push(line.clone());
                    done = true;
                } else {
                    out.push(l.to_string());
                    if l.starts_with("name:") && !text.contains("\nport output:") && !done {
                        out.push(line.clone());
                        done = true;
                    }
                }
            }
            std::fs::write(f, out.join("\n") + "\n").with_context(|| format!("writing {}", f.display()))?;
        }
    }
    Ok(())
}
