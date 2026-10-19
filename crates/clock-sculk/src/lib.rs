//! Track clk-sculk's clock commands: `mctas clock sculk <args...>` calls [`run`] with every
//! argument after `sculk`. OWNED BY TRACK clk-sculk (vibration clocks: sculk and calibrated sculk
//! sensors, amethyst resonance, every vibration source). Notebook:
//! `designs/clocks/tracks/clk-sculk/NOTES.md`.
//!
//! Every generator builds candidate designs from a parametric template, runs each through the
//! exact redsim detector (`clock::sim`), keeps the clocks, groups them by (period, family), ranks
//! them like the study (fewest blocks, then the smallest swept box), drops repeats of a structure
//! (up to translation and the 4 horizontal rotations) and writes the best few of every group.
//! The real-game judge (`mctas clock verify`) decides; these are candidates.
//!
//! The floor every generator respects (read in the source, `crates/redsim/src/sculk.rs`): a sensor
//! activated at tick A is INACTIVE again in the tile-tick phase of A+20 (calibrated) / A+40 (plain)
//! and a vibration posted at T lands at T + max(1, floor(distance)), so P >= 21 / P >= 41.

mod build;
mod gens;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

pub use build::{Build, Emitter, Sensor};

#[derive(Parser)]
#[command(name = "mctas clock sculk", about = "vibration clock generators (track clk-sculk)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(clap::Args, Clone)]
pub struct Out {
    /// Write the kept clocks here (`<prefix>-p<P>-<n>.txt`); nothing written without it.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// File name prefix.
    #[arg(long, default_value = "gen")]
    pub prefix: String,
    /// Distinct types kept per (period, family).
    #[arg(long, default_value_t = 1)]
    pub keep: usize,
    /// Only periods in this range.
    #[arg(long, default_value_t = 1)]
    pub min_p: u32,
    #[arg(long, default_value_t = 100_000)]
    pub max_p: u32,
    /// Print every kept clock, not just the per-period table.
    #[arg(long)]
    pub verbose: bool,
    /// Simulation horizon (gt).
    #[arg(long, default_value_t = 4_000)]
    pub horizon: u32,
}

#[derive(Subcommand)]
enum Cmd {
    /// Sensor + a chain of k observers (the first watches the sensor) + an emitter at the last
    /// observer's output (or beside a block it powers): every embedding up to rotation. P = 20 + 2k
    /// (+4 for a dispenser) + floor(distance) for a calibrated sensor, +20 for a plain one.
    Obs(gens::ObsArgs),
    /// Sensor + a line of repeaters (every delay) + a terminal (observer + emitter, a piston, a
    /// wall torch + note block), read from the sensor or from the block under it.
    Chain(gens::ChainArgs),
    /// Rings of calibrated sensors, each with an amethyst block (pure vibration: only the sensors
    /// change state): random embeddings for given hop lengths, kicked by a note block that only
    /// the first sensor hears.
    Ring(gens::RingArgs),
    /// A big ring of n calibrated sensors on a circle (long periods: P = sum of hops <= 16 n).
    RingBig(gens::RingBigArgs),
    /// Simulate designs and print one line each, sorted by period (`--best`: the smallest per
    /// period only).
    Table {
        files: Vec<PathBuf>,
        #[arg(long)]
        best: bool,
    },
    /// Keep the smallest design per (period, family) among the given files (by blocks, then
    /// swept volume, then name) and copy them into --out-dir: the batch to judge with
    /// `mctas clock verify`.
    Select {
        files: Vec<PathBuf>,
        #[arg(long)]
        out_dir: PathBuf,
        /// Only periods in this range.
        #[arg(long, default_value_t = 1)]
        min_p: u32,
        #[arg(long, default_value_t = 100_000)]
        max_p: u32,
        /// Skip (period, family) pairs already present (with at most as many blocks) in these
        /// files (e.g. the track's verified designs).
        #[arg(long, num_args = 1..)]
        known: Vec<PathBuf>,
        /// One design per PERIOD (the smallest of any family) instead of per (period, family).
        #[arg(long)]
        per_period: bool,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas clock sculk".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Obs(a) => gens::obs(&a),
        Cmd::Chain(a) => gens::chain(&a),
        Cmd::Ring(a) => gens::ring(&a),
        Cmd::RingBig(a) => gens::ring_big(&a),
        Cmd::Table { files, best } => table(&files, best),
        Cmd::Select { files, out_dir, min_p, max_p, known, per_period } => select(&files, &out_dir, min_p, max_p, &known, per_period),
    }
}

/// (period, family, blocks, volume) of a design by `clock::sim`, `None` unless it is a clock.
fn sim_key(path: &std::path::Path) -> Option<(u32, String, usize, i32)> {
    let c = clock::Clock::load(path).ok()?;
    let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
    (r.verdict == clock::sim::Verdict::Clock).then(|| (r.period, r.analysis.family.join("+"), r.blocks, r.volume()))
}

fn select(files: &[PathBuf], out_dir: &std::path::Path, min_p: u32, max_p: u32, known: &[PathBuf], per_period: bool) -> Result<()> {
    let group = |fam: &str| if per_period { String::new() } else { fam.to_string() };
    let mut have: BTreeMap<(u32, String), usize> = BTreeMap::new();
    for k in known {
        if let Some((p, f, b, _)) = sim_key(k) {
            let e = have.entry((p, group(&f))).or_insert(b);
            *e = (*e).min(b);
        }
    }
    let mut best: BTreeMap<(u32, String), (usize, i32, PathBuf, String)> = BTreeMap::new();
    for f in files {
        let Some((p, fam, b, v)) = sim_key(f) else { continue };
        let g = group(&fam);
        if p < min_p || p > max_p || have.get(&(p, g.clone())).is_some_and(|hb| *hb <= b) {
            continue;
        }
        let e = best.entry((p, g)).or_insert((b, v, f.clone(), fam.clone()));
        if (b, v, f) < (e.0, e.1, &e.2) {
            *e = (b, v, f.clone(), fam);
        }
    }
    std::fs::create_dir_all(out_dir)?;
    for ((p, _), (b, _, f, fam)) in &best {
        let name = f.file_name().context("file name")?;
        std::fs::copy(f, out_dir.join(name))?;
        println!("P={p:<4} {b:>3} blocks  {fam}  <{}>", f.display());
    }
    println!("selected {} designs into {}", best.len(), out_dir.display());
    Ok(())
}

/// One clock found by a generator.
#[derive(Clone, Debug)]
pub struct Found {
    pub period: u32,
    pub blocks: usize,
    pub volume: i32,
    pub family: String,
    pub summary: String,
    /// Structure key up to translation and rotation (no kick).
    pub key: String,
    pub clock: clock::Clock,
    /// How it was built (template + parameters).
    pub how: String,
}

/// Simulate a candidate; `Some` if it is a clock in the period range.
pub fn try_clock(c: &clock::Clock, how: &str, out: &Out) -> Option<Found> {
    let r = clock::sim::simulate(c, &clock::sim::SimOptions { horizon: out.horizon, margin: 4 });
    if r.verdict != clock::sim::Verdict::Clock || r.period < out.min_p || r.period > out.max_p {
        return None;
    }
    Some(Found {
        period: r.period,
        blocks: r.blocks,
        volume: r.volume(),
        family: r.analysis.family.join("+"),
        summary: r.summary(),
        key: build::structure_key(&c.design.blocks),
        clock: c.clone(),
        how: how.to_string(),
    })
}

/// Simulate every candidate on all cores.
pub fn simulate_all(cands: Vec<(clock::Clock, String)>, out: &Out) -> Vec<Found> {
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(64);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..n {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((c, how)) = cands.get(i) else { break };
                    if let Some(f) = try_clock(c, how, out) {
                        found.lock().unwrap().push(f);
                    }
                }
            });
        }
    });
    found.into_inner().unwrap()
}

/// Group by (period, family), rank, dedupe structures, keep `out.keep` per group; print the
/// per-period table and write the files.
pub fn report(found: Vec<Found>, out: &Out) -> Result<()> {
    let mut groups: BTreeMap<(u32, String), Vec<Found>> = BTreeMap::new();
    for f in found {
        groups.entry((f.period, f.family.clone())).or_default().push(f);
    }
    let mut kept: Vec<Found> = Vec::new();
    for (_, mut v) in groups {
        v.sort_by(|a, b| (a.blocks, a.volume, &a.key).cmp(&(b.blocks, b.volume, &b.key)));
        let mut seen: HashSet<String> = HashSet::new();
        for f in v {
            if seen.len() >= out.keep {
                break;
            }
            if seen.insert(f.key.clone()) {
                kept.push(f);
            }
        }
    }
    // Per-period table: the smallest clock and every family reaching it.
    let mut by_p: BTreeMap<u32, Vec<&Found>> = BTreeMap::new();
    for f in &kept {
        by_p.entry(f.period).or_default().push(f);
    }
    println!("{:>4} {:>6} {:>5}  smallest (how) | families", "P", "blocks", "types");
    for (p, v) in &by_p {
        let best = v.iter().min_by_key(|f| (f.blocks, f.volume)).unwrap();
        let fams: Vec<String> = {
            let mut s: Vec<String> = v.iter().map(|f| format!("{}:{}", f.family, f.blocks)).collect();
            s.sort();
            s.dedup();
            s
        };
        println!("{p:>4} {:>6} {:>5}  {} | {}", best.blocks, v.len(), best.how, fams.join(" "));
    }
    if out.verbose {
        for f in &kept {
            println!("{}   [{}]", f.summary, f.how);
        }
    }
    if let Some(dir) = &out.out_dir {
        std::fs::create_dir_all(dir)?;
        let mut n_per_p: BTreeMap<u32, usize> = BTreeMap::new();
        for f in &kept {
            let n = n_per_p.entry(f.period).or_default();
            *n += 1;
            let name = format!("{}-p{}-{}", out.prefix, f.period, n);
            let mut d = f.clock.design.clone();
            d.name = format!("clk-sculk-{name}");
            let text = format!("# {}\n# built by `mctas clock sculk`: {}\n{}", f.summary, f.how, d.to_text());
            let path = dir.join(format!("{name}.txt"));
            std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        }
        println!("wrote {} designs to {}", kept.len(), dir.display());
    }
    Ok(())
}

fn table(files: &[PathBuf], best: bool) -> Result<()> {
    let mut rows: Vec<(u32, usize, i32, String)> = Vec::new();
    for f in files {
        let c = clock::Clock::load(f)?;
        let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
        let p = if r.verdict == clock::sim::Verdict::Clock { r.period } else { 0 };
        rows.push((p, r.blocks, r.volume(), format!("{}  <{}>", r.summary(), f.display())));
    }
    rows.sort();
    let mut last_p = None;
    for (p, _, _, line) in rows {
        if best && last_p == Some(p) {
            continue;
        }
        last_p = Some(p);
        println!("{line}");
    }
    Ok(())
}
