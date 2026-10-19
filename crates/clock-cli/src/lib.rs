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
        /// Test positions (the door judge's offsets; vjeux 2026-10-19: 4 positions x 4 rotations).
        #[arg(long, default_value_t = 4)]
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
        /// Keep ONE archived design per type (the earliest archived), remove the others (design,
        /// pictures, rows) and record them in aliases.tsv; no judging.
        #[arg(long)]
        dedupe: bool,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Exhaustive census on redsim: every filling of an AxBxC box with kmin..=k blocks of the
    /// palette (every facing), one per class up to translation and the 4 rotations, judged
    /// self-starting and then with each block placed last; writes <out>/clocks.tsv (every clock:
    /// period, waveform, family, blocks, box, start, type key, canonical encoding), a design per
    /// smallest clock of each (period, family) in <out>/designs/, <out>/summary.md. Resumable
    /// (<out>/parts/, one file per finished chunk). The ground truth of the small clocks.
    Census {
        /// Box as AxBxC (x, y, z).
        #[arg(long = "box")]
        dims: String,
        /// Comma list of shorthands (core, stone, glass, rblock, dust, torch, repeater, comparator,
        /// observer, piston, sticky, lamp, bulb, note, target, slime, sculk, csculk, hopper,
        /// dropper, trapdoor, leaves, scaffold...) or block states.
        #[arg(long, default_value = "core")]
        palette: String,
        #[arg(long)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        /// Longest period looked for (the horizon: 4 periods + --warmup gt); a candidate whose
        /// whole state does not repeat within 4x it is dropped early (e.g. `--max-period 3
        /// --warmup 60`: the P <= 3 question).
        #[arg(long, default_value_t = 500)]
        max_period: u32,
        /// Warmup allowance in the horizon (gt).
        #[arg(long, default_value_t = 1000)]
        warmup: u32,
        #[arg(long, default_value_t = 2)]
        margin: i32,
        /// Self-starting only (no block placed last).
        #[arg(long)]
        no_kick: bool,
        /// Threads (default: every core but 4).
        #[arg(long)]
        workers: Option<usize>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Pictures in the REAL client (one session for all): each clock in its own sandbox on a grass
    /// floor at noon, stepped to the most active moment of a period after its warmup (or --tick),
    /// a still camera framing its whole swept box; saved as <out-dir>/<design stem>.jpg (--width
    /// px). --strip N: also N moments of one period tiled (<stem>.strip.jpg). The catalog shows
    /// them. Uses an X display: run it on a box where you may (docs/clocks.md).
    Shot {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = clock::shot::IMG_DIR)]
        out_dir: PathBuf,
        #[arg(long)]
        tick: Option<u32>,
        #[arg(long)]
        strip: Option<u32>,
        #[arg(long, default_value_t = 800)]
        width: u32,
        /// Wait after the camera enters a clock's sandbox (ms; the default shooter: two sandboxes
        /// used in turn, ~2 s a clock).
        #[arg(long, default_value_t = 1500)]
        hold_ms: u64,
        /// One-world shooter: wait after a camera move (ms, plus 15 ms per block moved).
        #[arg(long, default_value_t = 700)]
        settle_ms: u64,
        /// EXPERIMENTAL, not recommended: ONE world per batch of clocks (staggered starts, only the
        /// camera moves); neighbors show in the frames. The default shooter is as fast now (~2 s a
        /// clock) with every clock alone on its floor.
        #[arg(long)]
        grid: bool,
        /// Take pictures that exist again.
        #[arg(long)]
        redo: bool,
    },
    /// Compare the pictures of a directory with the same-named ones of another (SSIM, lowest
    /// first): checks one shooter's output against a trusted set.
    ShotCompare {
        dir: PathBuf,
        #[arg(long, default_value = clock::shot::IMG_DIR)]
        against: PathBuf,
        /// Print only the pictures below this SSIM.
        #[arg(long, default_value_t = 1.0)]
        below: f64,
    },
    /// (videos: the real client) For each period, every given clock's labeled video (each filmed
    /// alone, >= 2 periods; clips already filmed are reused) after a title card, concatenated into
    /// <out-dir>/montage-p<P>.mp4 (the per-clock videos stay in <out-dir>/clips/).
    Montage {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: PathBuf,
        /// Only these periods (comma list).
        #[arg(long, value_delimiter = ',')]
        periods: Vec<u32>,
        #[arg(long, default_value_t = 6.0)]
        min_secs: f64,
    },
    /// (COORDINATOR ONLY: the X displays are shared) The clock running in the real client, a
    /// still camera framing its swept box, >= 2 periods (and >= --min-secs of footage) after its
    /// warmup, with a corner label (period; family, blocks, box; duty, start; name) and an optional
    /// title card; H.264. The clean clip is kept as <out>.clip.mp4 (--relabel recuts from it), its
    /// first frame as <out>.png. --dry-run prints the plan and writes the label image only.
    Video {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 2)]
        periods: u32,
        #[arg(long, default_value_t = 8.0)]
        min_secs: f64,
        /// Ticks per second while filming (default 10; faster for long periods, <= 60 s).
        #[arg(long)]
        tps: Option<f64>,
        #[arg(long)]
        card: bool,
        #[arg(long)]
        relabel: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// Merge census runs (their parts/ and params.txt) into one report: per (period, family) the
    /// fewest blocks among MINIMAL clocks, its box and census, the minimal type count; the periods
    /// never found below the largest. Writes Markdown to --out (default: stdout).
    CensusReport {
        dirs: Vec<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Judge instances of an extendable family (a design with `extend` headers) in the game:
    /// build each n with its generator, verify it, compare period and blocks with the formulas.
    ExtendCheck {
        design: PathBuf,
        /// `a..b`, `a,b,c` or one n (default: low, low+1, low+2, middle, high of `extend n:`).
        #[arg(long)]
        n: Option<String>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Record the judged instances in the archive's extend.tsv (the catalog then leaves them
        /// out of the per-period gallery).
        #[arg(long)]
        record: bool,
        #[arg(long, default_value = clock::catalog::ARCHIVE_DIR)]
        dir: PathBuf,
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
        ClockCmd::Archive { designs, caption, dedupe, dir, positions, horizon, margin, workers } => {
            if dedupe {
                let img = dir.parent().map(|p| p.join("img")).unwrap_or_else(|| PathBuf::from(clock::shot::IMG_DIR));
                for l in clock::catalog::dedupe(&dir, &img)? {
                    println!("{l}");
                }
                return Ok(());
            }
            if designs.is_empty() {
                bail!("clock archive <design>...");
            }
            let o = VerifyOptions { positions, rotations: true, horizon, margin, workers, version: version.to_string(), ..Default::default() };
            let pool = clock::verify::spawn_pool(&o)?;
            let mut failed = 0;
            for path in &designs {
                match clock::catalog::archive(&pool, path, &o, &dir, caption.as_deref()) {
                    Ok(a) if a.new => println!("ARCHIVED {}: {}", a.file.display(), a.summary),
                    Ok(a) if a.summary.starts_with("same type") => println!("SAME TYPE {}: {}", path.display(), a.summary),
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
        ClockCmd::Shot { designs, out_dir, tick, strip, width, hold_ms, settle_ms, grid, redo } => {
            let mut items = Vec::new();
            let mut hints = std::collections::HashMap::new();
            for p in &designs {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                if let Some(h) = std::fs::read_to_string(p).ok().as_deref().and_then(clock::shot::judge_hint) {
                    hints.insert(stem.clone(), h);
                }
                items.push((stem, Clock::load(p)?));
            }
            let o = clock::shot::ShotOptions { out_dir, tick, strip, width, hold_ms, settle_ms, redo, hints, ..Default::default() };
            let lines = if grid && strip.is_none() { clock::shot::shots_grid(version, &items, &o)? } else { clock::shot::shots(version, &items, &o)? };
            for l in lines {
                println!("{l}");
            }
        }
        ClockCmd::ShotCompare { dir, against, below } => {
            let r = clock::shot::compare(&dir, &against)?;
            let n = r.len();
            for (name, v) in r.iter().filter(|(_, v)| *v < below) {
                println!("{v:.3} {name}");
            }
            let mean = if n > 0 { r.iter().map(|(_, v)| v).sum::<f64>() / n as f64 } else { 0.0 };
            println!("{n} pictures compared, mean SSIM {mean:.3}");
        }
        ClockCmd::Montage { designs, out_dir, periods, min_secs } => {
            let mut items = Vec::new();
            for p in &designs {
                let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                items.push((stem, Clock::load(p)?));
            }
            let o = clock::video::ClockVideoOptions { min_secs, ..Default::default() };
            let ps = (!periods.is_empty()).then_some(periods.as_slice());
            for l in clock::video::montage(version, &items, &out_dir, ps, &o)? {
                println!("{l}");
            }
        }
        ClockCmd::Video { design, out, periods, min_secs, tps, card, relabel, dry_run } => {
            let c = Clock::load(&design)?;
            let o = clock::video::ClockVideoOptions { periods, min_secs, tps, card, relabel, dry_run, ..Default::default() };
            let p = clock::video::video(version, &c, &out, &o)?;
            println!(
                "{} {}: P {} gt, skip {} ticks (warmup + kick), film {} ticks ({} periods) at {} tps = {:.1} s{}",
                if dry_run { "PLAN" } else { "VIDEO" },
                out.display(),
                p.period,
                p.skip,
                p.ticks,
                p.ticks / p.period.max(1),
                p.tps,
                p.ticks as f64 / p.tps,
                if dry_run { format!("; label: {}", out.with_extension("label.png").display()) } else { String::new() }
            );
        }
        ClockCmd::CensusReport { dirs, out } => {
            let md = clock::census::report(&dirs)?;
            match out {
                Some(p) => {
                    std::fs::write(&p, &md)?;
                    println!("{}", p.display());
                }
                None => print!("{md}"),
            }
        }
        ClockCmd::ExtendCheck { design, n, positions, workers, record, dir } => {
            let c = Clock::load(&design)?;
            let f = clock::extend::Family::of(&c)?.ok_or_else(|| anyhow::anyhow!("{}: no `extend family:` header", design.display()))?;
            let ns = match &n {
                Some(s) => clock::extend::parse_ns(s)?,
                None => f.sample(),
            };
            let o = VerifyOptions { positions, rotations: true, workers, version: version.to_string(), ..Default::default() };
            let pool = clock::verify::spawn_pool(&o)?;
            println!("extend family {}: P = {} gt ({}), blocks = {}, n = {}..{}; judging n = {}", f.id, f.period.text(), f.period.seconds(), f.blocks.text(), f.n_lo, f.n_hi, ns.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", "));
            let rows = clock::extend::check(&pool, &design, &f, &ns, &o)?;
            let bad = rows.iter().filter(|(_, ok, _)| !ok).count();
            for (_, _, line) in &rows {
                println!("{line}");
            }
            if record {
                let good: Vec<clock::extend::Instance> = rows.iter().filter(|(_, ok, _)| *ok).map(|(i, _, _)| i.clone()).collect();
                clock::extend::record(&dir, &good)?;
                println!("recorded {} instance(s) in {}", good.len(), dir.join(clock::extend::EXTEND_TSV).display());
            }
            if bad > 0 {
                bail!("{bad} of {} instances do not follow the formulas", rows.len());
            }
        }
        ClockCmd::Catalog { dir, out, coverage } => {
            let s = clock::catalog::catalog(&dir, &out, &coverage)?;
            println!("{}: {} clocks, {} types, {} families, {} periods; {}", out.display(), s.clocks, s.types, s.families, s.periods, coverage.display());
        }
        ClockCmd::Census { dims, palette, k, kmin, max_period, warmup, margin, no_kick, workers, out } => {
            let d: Vec<i32> = dims.split('x').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            if d.len() != 3 || d.iter().any(|x| *x < 1) {
                bail!("--box AxBxC, got {dims:?}");
            }
            let workers = workers.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(4).max(1));
            let o = clock::census::CensusOptions {
                dims: [d[0], d[1], d[2]],
                palette: clock::census::expand_palette(&palette)?,
                k,
                kmin,
                horizon: 4 * max_period + warmup,
                max_period,
                margin,
                kick_last: !no_kick,
                workers,
                out: out.clone(),
            };
            println!("census {}x{}x{} k {}..={} palette {} states, {workers} threads -> {}", d[0], d[1], d[2], kmin, k, o.palette.len(), out.display());
            let t0 = std::time::Instant::now();
            let last = std::sync::Mutex::new(std::time::Instant::now());
            let found = clock::census::census(&o, &|done, total, n| {
                let mut l = last.lock().unwrap();
                if l.elapsed().as_secs() >= 30 || done == total {
                    *l = std::time::Instant::now();
                    eprintln!("  {done}/{total} chunks, {n} clocks, {:.0} s", t0.elapsed().as_secs_f64());
                }
            })?;
            let (n, types, pairs) = clock::census::write_outputs(&o, &found)?;
            println!("{n} clocks, {types} types, {pairs} (period, family) pairs in {:.1} s: {}/clocks.tsv, summary.md, designs/", t0.elapsed().as_secs_f64(), out.display());
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
    if !r.analysis.moved.is_empty() {
        println!("  moved only (not in the family): {}", r.analysis.moved.join(", "));
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
