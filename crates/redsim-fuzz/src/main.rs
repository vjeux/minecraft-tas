//! `redsim-fuzz`: the differential runner as a small binary (fast to rebuild while fixing the
//! model; `mctas door redsim-fuzz` / `door redsim-case` run the same code).

use anyhow::Result;
use clap::{Parser, Subcommand};
use mctas::sim::{Pool, Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = mctas::DEFAULT_VERSION)]
    version: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Random circuits in the game and in redsim, every cell compared every tick.
    Fuzz {
        #[arg(long, default_value_t = 1000)]
        n: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value = "6,3,6")]
        size: String,
        #[arg(long, default_value_t = 0.45)]
        density: f64,
        #[arg(long, default_value = "core")]
        palette: String,
        #[arg(long, default_value = "designs/redsim/fails")]
        out: PathBuf,
        #[arg(long)]
        no_shrink: bool,
        #[arg(long, default_value_t = 8)]
        max_report: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Run this many batches of n (seeds seed, seed+1, ...), printing the running total.
        #[arg(long, default_value_t = 1)]
        batches: usize,
    },
    /// Replay a case file in both; print differing frames.
    Case {
        case: PathBuf,
        #[arg(long, default_value_t = 3)]
        max: usize,
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(long, default_value = "")]
        note: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = WorkerConfig { version: cli.version.clone(), heap_mb: 768, ..Default::default() };
    match cli.cmd {
        Cmd::Fuzz { n, seed, size, density, palette, out, no_shrink, max_report, workers, batches } => {
            let sz: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            anyhow::ensure!(sz.len() == 3, "--size x,y,z");
            let pool = Pool::spawn(workers, &cfg)?;
            let (mut total, mut bad) = (0, 0);
            for b in 0..batches {
                let o = redsim_fuzz::FuzzOptions {
                    n,
                    seed: seed + b as u64,
                    size: [sz[0], sz[1], sz[2]],
                    density,
                    palette: redsim_fuzz::palette(&palette)?,
                    out: out.clone(),
                    shrink: !no_shrink,
                    max_report,
                };
                let (t, m) = redsim_fuzz::fuzz(&pool, &o, &|s| eprintln!("{s}"))?;
                total += t;
                bad += m;
                if batches > 1 {
                    eprintln!("running total: {bad} mismatches in {total} cases ({:.4}%)", 100.0 * bad as f64 / total.max(1) as f64);
                }
            }
        }
        Cmd::Case { case, max, fixture, note } => {
            let mut w = Worker::spawn(&cfg)?;
            let same = redsim_fuzz::replay(&mut w, &case, max)?;
            if let Some(path) = fixture {
                anyhow::ensure!(same || !note.is_empty(), "a fixture of a mismatching case needs a --note");
                let c = redsim_fuzz::Case::parse(&std::fs::read_to_string(&case)?)?;
                let text = redsim_fuzz::fixture(&mut w, &c, &note)?;
                std::fs::write(&path, text)?;
                eprintln!("fixture written to {}", path.display());
            }
        }
    }
    Ok(())
}
