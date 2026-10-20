//! Contraption study 11, track timer: monostables (retriggerable and non-retriggerable),
//! off-delays and on-delays / pulse filters (docs/circuits.md "Study 11 timers", the variations in
//! designs/circuits/pulsemult/VARIATIONS.md). `mctas circuit timer <command> ...` dispatches here.
//! The judge's checkers are crates/circuit/src/pulsemult.rs's `monostable`, `off_delay` and
//! `on_delay`; the designs live in designs/circuits/pulsemult/tracks/timer/.

pub mod cells;
pub mod rehead;
pub mod rings;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit timer", about = "monostables, off-delays, on-delays (contraption study 11, the timer half)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Judge designs of any study as another kind on redsim (their `circuit:` line replaced):
    /// candidates from earlier studies (a rising-edge detector as a monostable, a pulse extender
    /// as an off-delay...). One line per design: the judge's line, or `FAIL`; with `--out`, every
    /// pass is written there under its new kind (name `<prefix><name>`).
    As {
        /// The kind to judge them as (`monostable`, `monostable retriggerable`, `off-delay`,
        /// `on-delay`, ...).
        kind: String,
        designs: Vec<PathBuf>,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Prefix of the written designs' names.
        #[arg(long, default_value = "")]
        prefix: String,
        /// Only print the passes.
        #[arg(long, short)]
        quiet: bool,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long, default_value = "26.3")]
        version: String,
    },
    /// A design's run on redsim under an explicit stimulus (`10+ 14-`, as `circuit sim
    /// --stimulus`): every cell that changes, frame by frame, from `--from` to `--to`.
    Cells {
        design: PathBuf,
        #[arg(long)]
        stimulus: String,
        #[arg(long, default_value_t = 0)]
        from: u32,
        #[arg(long, default_value_t = u32::MAX)]
        to: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Comparator-ring off-delays (crates/circuit-timer/src/rings.rs): two comparators in a ring,
    /// every dust return line in the box, every input and output cell, judged on redsim; the
    /// smallest pass of each L is written to `<out>/off-delay-ring-L<L>.txt`.
    Rings {
        #[arg(long)]
        out: PathBuf,
        /// The longest return line (dust).
        #[arg(long, default_value_t = 7)]
        max_line: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value = "26.3")]
        version: String,
        /// What charges the ring: `input` (an off-delay: the input beside a line dust) or `retrig`
        /// (a retriggerable monostable: the copper-bulb two-observer edge front).
        #[arg(long, default_value = "input")]
        front: String,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit timer".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::As { kind, designs, out, prefix, quiet, margin, version } => rehead::run(&kind, &designs, out.as_deref(), &prefix, quiet, margin, &version),
        Cmd::Rings { out, max_line, workers, version, front } => rings::run(&out, max_line, workers, &version, rings::Front::parse(&front)?),
        Cmd::Cells { design, stimulus, from, to, margin } => cells::run(&design, &stimulus, from, to, margin),
    }
}
