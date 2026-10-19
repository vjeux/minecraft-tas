//! `mctas clock ...`: the clock study's commands (docs/clocks.md). Argument parsing and dispatch
//! only (compiled unoptimized, like the door CLI crates); the work is in `crates/clock`.

use anyhow::{Result, bail};
use clap::Subcommand;
use clock::sim::{SimOptions, SimReport, Verdict, simulate};
use clock::verify::{VerifyOptions, VerifyReport};
use clock::{Clock, wave};
use serde_json::json;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ClockCmd {
    /// Exact period on redsim (milliseconds): place like the judge, kick, run until the whole
    /// state repeats; prints the period, warmup, waveform per changing cell and at the output,
    /// the family, blocks placed and the swept box. Blocks redsim does not model: UNMODELED.
    Sim {
        designs: Vec<PathBuf>,
        /// Ticks to run at most while looking for a repeat.
        #[arg(long, default_value_t = 50_000)]
        horizon: u32,
        /// Free cells simulated around the design (a write beyond = INVALID, escapes).
        #[arg(long, default_value_t = 4)]
        margin: i32,
        /// One JSON object per design.
        #[arg(long)]
        json: bool,
        /// Only the summary line per design.
        #[arg(long, short)]
        quiet: bool,
        /// Print every cell change of the first N ticks instead.
        #[arg(long)]
        trace: Option<u32>,
    },
    /// The official judge in the real game: N positions x 4 rotations, kick, run to the horizon,
    /// exact periodicity for >= 3 periods after warmup, the same period/waveform/family everywhere
    /// (else position- or rotation-dependent, flagged), no entities, never reaching the edge of
    /// the watched box. Prints one CLOCK line, then the redsim agreement check.
    Verify {
        designs: Vec<PathBuf>,
        /// Test positions (the door judge's offsets).
        #[arg(long, default_value_t = 8)]
        positions: usize,
        /// Only as built (default: all 4 rotations).
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        /// Watched cells around the box (anything reaching the outer layer fails).
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// JVM workers (capped by MCTAS_MAX_WORKERS).
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Every build's result.
        #[arg(long, short)]
        verbose: bool,
        /// Skip the redsim agreement check.
        #[arg(long)]
        no_sim: bool,
        #[arg(long)]
        json: bool,
    },
    /// The judge, then keep it: a copy in designs/clocks/archive/ (header: date, source, the
    /// judge's line), its verdict row (verdicts-v1.tsv) and a caption (captions.tsv; default: the
    /// design's first comment block). A clock already archived is skipped. Then run `clock catalog`.
    Archive {
        designs: Vec<PathBuf>,
        /// One line: how it works (default: the design's first comment block).
        #[arg(long)]
        caption: Option<String>,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value_t = 8)]
        positions: usize,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Write designs/clocks/CATALOG.md (per period: the distinct types, families and the smallest
    /// of each; the period x family coverage matrix; the impossibility table from
    /// designs/clocks/impossible.tsv) and designs/clocks/coverage.md (the matrix alone, mirrored
    /// on the board) from the archive's verdicts. Runs no game.
    Catalog {
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value = clock::catalog::CATALOG)]
        out: PathBuf,
        #[arg(long, default_value = clock::catalog::COVERAGE)]
        coverage: PathBuf,
    },
    /// Track clk-obs's commands (crates/clock-obs: observer / piston / bulb / note block clocks).
    Obs {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-rt's commands (crates/clock-rt: torch / repeater / comparator clocks).
    Rt {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-sculk's commands (crates/clock-sculk: sculk / vibration clocks).
    Sculk {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track clk-item's commands (crates/clock-item: hopper / dropper / item clocks).
    Item {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

pub fn run(version: &str, cmd: ClockCmd) -> Result<()> {
    match cmd {
        ClockCmd::Sim { designs, horizon, margin, json, quiet, trace } => {
            if designs.is_empty() {
                bail!("clock sim <design>...");
            }
            let o = SimOptions { horizon, margin };
            for path in &designs {
                let c = Clock::load(path)?;
                if let Some(n) = trace {
                    for l in clock::sim::trace(&c, &o, n) {
                        println!("{l}");
                    }
                    continue;
                }
                let t0 = std::time::Instant::now();
                let r = simulate(&c, &o);
                if json {
                    println!("{}", sim_json(&c, &r));
                } else if quiet {
                    println!("{}", r.summary());
                } else {
                    print_sim(&c, &r, t0.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
        ClockCmd::Verify { designs, positions, no_rotations, horizon, margin, workers, verbose, no_sim, json } => {
            if designs.is_empty() {
                bail!("clock verify <design>...");
            }
            let o = VerifyOptions { positions, rotations: !no_rotations, horizon, margin, workers, version: version.to_string(), ..Default::default() };
            let mut failed = false;
            let pool = clock::verify::spawn_pool(&o)?;
            for path in &designs {
                let c = Clock::load(path)?;
                let t0 = std::time::Instant::now();
                let r = clock::verify::verify_in_pool(&pool, &c, &o)?;
                let secs = t0.elapsed().as_secs_f64();
                let sim = (!no_sim).then(|| simulate(&c, &SimOptions::default()));
                if json {
                    println!("{}", verify_json(&r, sim.as_ref()));
                } else {
                    print_verify(&r, verbose, secs, sim.as_ref());
                }
                failed |= r.verdict != Verdict::Clock;
            }
            if failed {
                std::process::exit(1);
            }
        }
        ClockCmd::Archive { designs, caption, dir, positions, horizon, margin, workers } => {
            if designs.is_empty() {
                bail!("clock archive <design>...");
            }
            let o = VerifyOptions { positions, rotations: true, horizon, margin, workers, version: version.to_string(), ..Default::default() };
            let pool = clock::verify::spawn_pool(&o)?;
            let mut failed = 0;
            for path in &designs {
                match clock::catalog::archive(&pool, path, &o, &dir, caption.as_deref()) {
                    Ok(a) if a.new => println!("ARCHIVED {}: {}", a.file.display(), a.summary),
                    Ok(a) => println!("ALREADY {}: {}", a.file.display(), a.summary),
                    Err(e) => {
                        failed += 1;
                        println!("NOT ARCHIVED {}: {e:#}", path.display());
                    }
                }
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        ClockCmd::Catalog { dir, out, coverage } => {
            let s = clock::catalog::catalog(&dir, &out, &coverage)?;
            println!("{}: {} clocks, {} types, {} families, {} periods; {}", out.display(), s.clocks, s.types, s.families, s.periods, coverage.display());
        }
        ClockCmd::Obs { args } => clock_obs::run(args)?,
        ClockCmd::Rt { args } => clock_rt::run(args)?,
        ClockCmd::Sculk { args } => clock_sculk::run(args)?,
        ClockCmd::Item { args } => clock_item::run(args)?,
    }
    Ok(())
}

fn print_sim(c: &Clock, r: &SimReport, ms: f64) {
    println!("{}", r.summary());
    if r.verdict != Verdict::Clock {
        return;
    }
    let (lo, hi) = r.swept;
    println!("  start: {}; swept box {:?}..{:?} (volume {}); {:.1} ms", r.start, lo, hi, r.volume(), ms);
    let hidden = if r.state_period != r.period { format!(" (hidden state: the visible state repeats every {} gt)", r.period) } else { String::new() };
    println!("  the whole state (pending ticks, block entities, torch memory) repeats exactly every {} gt from tick {}{hidden}", r.state_period, r.state_warmup);
    if let Some(o) = &r.output {
        println!(
            "  output {:?}{}: {}",
            o.pos,
            if o.auto { " (auto: no `port output`, the cell with the most edges)" } else { "" },
            o.wave.describe(r.period)
        );
    }
    println!("  {} changing cells:", r.analysis.cells.len());
    for w in &r.analysis.cells {
        let states = if w.states.len() > 6 { format!("{} | ... ({} states)", w.states[..6].join(" | "), w.states.len()) } else { w.states.join(" | ") };
        println!("    {:?} {}: {}", w.pos, states, w.describe(r.period));
    }
    let _ = c;
}

fn wave_json(w: &wave::CellWave) -> serde_json::Value {
    json!({"pos": w.pos, "states": w.states, "kinds": w.kinds, "on": w.on, "edges": w.edges, "pulses": w.widths})
}

fn sim_json(c: &Clock, r: &SimReport) -> serde_json::Value {
    json!({
        "name": c.name(),
        "verdict": r.verdict.word(),
        "why": r.why,
        "period": r.period,
        "state_period": r.state_period,
        "warmup": r.warmup,
        "state_warmup": r.state_warmup,
        "family": wave::signature(&r.analysis.family),
        "blocks": r.blocks,
        "dims": r.dims(),
        "volume": r.volume(),
        "start": r.start,
        "output": r.output.as_ref().map(|o| json!({"pos": o.pos, "auto": o.auto, "on": o.wave.on, "edges": o.wave.edges, "pulses": o.wave.widths})),
        "cells": r.analysis.cells.iter().map(wave_json).collect::<Vec<_>>(),
        "summary": r.summary(),
    })
}

fn agreement(r: &VerifyReport, sim: &SimReport) -> String {
    let Some(b) = r.base() else {
        return format!("redsim: {}", sim.summary());
    };
    if sim.verdict == Verdict::Unmodeled {
        return format!("redsim: UNMODELED ({}): the real game is the only judge", sim.why);
    }
    let (son, sedges) = sim.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
    let (von, vedges) = b.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
    let same = sim.verdict == Verdict::Clock
        && sim.period == b.period
        && sim.warmup == b.warmup
        && son == von
        && sedges == vedges
        && sim.analysis.family == b.analysis.family
        && sim.dims() == b.dims;
    if same {
        "redsim AGREES (period, warmup, output waveform, family, box)".into()
    } else {
        format!("redsim DISAGREES: {}", sim.summary())
    }
}

fn print_verify(r: &VerifyReport, verbose: bool, secs: f64, sim: Option<&SimReport>) {
    println!("{}", r.summary());
    let n_ok = r.runs.iter().filter(|x| x.result.is_ok()).count();
    println!("  {}; {} builds periodic of {} ({:.1} s)", r.start, n_ok, r.runs.len(), secs);
    for run in &r.runs {
        let line = match &run.result {
            Ok(k) => format!(
                "P={} W={} family {} output {}",
                k.period,
                k.warmup,
                wave::signature(&k.analysis.family),
                k.output.as_ref().map(|o| o.wave.describe(k.period)).unwrap_or_default()
            ),
            Err(e) => format!("FAIL {e}"),
        };
        let show = verbose || run.result.is_err() || r.robustness != "robust";
        if show {
            println!("    rotation {} at {:?}: {line}", run.turns, run.offset);
        }
    }
    if let Some(s) = sim {
        println!("  {}", agreement(r, s));
    }
}

fn verify_json(r: &VerifyReport, sim: Option<&SimReport>) -> serde_json::Value {
    json!({
        "name": r.name,
        "verdict": r.verdict.word(),
        "robustness": r.robustness,
        "summary": r.summary(),
        "runs": r.runs.iter().map(|run| match &run.result {
            Ok(k) => json!({"turns": run.turns, "offset": run.offset, "period": k.period, "warmup": k.warmup, "family": wave::signature(&k.analysis.family), "dims": k.dims,
                "output": k.output.as_ref().map(|o| json!({"pos": o.pos, "on": o.wave.on, "edges": o.wave.edges, "pulses": o.wave.widths}))}),
            Err(e) => json!({"turns": run.turns, "offset": run.offset, "fail": e}),
        }).collect::<Vec<_>>(),
        "redsim": sim.map(|s| agreement(r, s)),
    })
}
