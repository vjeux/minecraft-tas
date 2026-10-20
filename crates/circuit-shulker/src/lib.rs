//! Contraption study 8, track shulker: shulker loaders and unloaders (docs/circuits.md, the
//! variations in designs/circuits/shulker/VARIATIONS.md). `mctas circuit shulker <command> ...`
//! dispatches here. The judge itself (kinds `shulker-loader` / `shulker-unloader`, the badges) is
//! in crates/circuit/src/shulker.rs; this crate holds the searches, judged in the game, the
//! real-game traces and their redsim replay (`compare`: redsim models shulker boxes, chests, the
//! dispenser's box placement and the piston-broken drop as the bridge seeds it).

pub mod compare;
pub mod shrink;
pub mod trace;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit shulker", about = "Shulker loaders and unloaders (contraption study 8)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Greedy block removal judged in the game: each round tries removing every design block (the
    /// start lever stays), first on the plain scenario at one rotation, then the survivors on the
    /// plain scenario and every kept badge at 4 rotations; the first full survivor is removed.
    /// Ends with a full `circuit verify` of the result.
    Shrink {
        design: PathBuf,
        /// Where the smaller design goes (default: `<design>-shrunk.txt` beside it).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Badges every removal must keep (comma-separated: colours, restart, top-up, flush,
        /// box-starved, drop-free, tileable, 16-stack, unstackable); `always-on` is kept anyway.
        #[arg(long, default_value = "")]
        badges: String,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Real-game traces for the in-browser viewer (the clock traces' JSON Lines format), one file
    /// per design in designs/circuits/shulker/traces (or traces/exact with --exact): the judge's
    /// first build running 3 boxes of a short scenario (unstackables by default), from the filled
    /// chests to the last box.
    Trace {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// The scenario item's max stack (1: iron shovels, a short replay; 16; 64).
        #[arg(long, default_value_t = 1)]
        stack: u32,
        /// The game's own doubles and floats, each entity's motion and network id.
        #[arg(long)]
        exact: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Replay exact real-game traces (traces/exact/*.trace) in redsim, every cell and entity
    /// compared every tick (the judge's run: chests, settle, supply, lever, then each gt a tick,
    /// the frame, the judge's visit; drops from the trace's seed): redsim's shulker gate. One line
    /// per trace (OK, the first divergence, UNMODELED).
    Compare {
        traces: Vec<PathBuf>,
        /// Differences shown at the first diverging tick.
        #[arg(long, default_value_t = 6)]
        max: usize,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit shulker".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Shrink { design, out, badges, workers, version, margin } => {
            let keep: Vec<String> = badges.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let out = out.unwrap_or_else(|| {
                let stem = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                design.with_file_name(format!("{stem}-shrunk.txt"))
            });
            shrink::shrink(&design, &out, &keep, workers, &version, margin)
        }
        Cmd::Trace { designs, out_dir, stack, exact, workers, version, margin } => trace::trace(&designs, out_dir, stack, exact, workers, &version, margin),
        Cmd::Compare { traces, max } => compare::compare(&traces, max),
    }
}
