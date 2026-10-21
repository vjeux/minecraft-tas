//! Contraption study 15: 7-segment display decoders (`circuit: seven-segment 4`, the judge in
//! crates/circuit/src/compute.rs, the board in designs/circuits/display/WR.md).
//! `mctas circuit display <command> ...` dispatches here; designs live in
//! designs/circuits/tracks/display/.

pub mod check;
pub mod settle;
pub mod towers;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit display", about = "7-segment decoder generators (contraption study 15)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The ring stack (crates/circuit-display/src/towers.rs): one ring of 4 subtract comparators
    /// per segment, stacked every `--pitch` levels; the ring corners and the comparators' rear
    /// functions come from torch towers (both literal polarities, the row functions of a1 a0),
    /// every tower and feed line routed by a small search. Writes the design to `--out`.
    Towers {
        #[arg(long)]
        out: PathBuf,
        /// Levels between two rings (a multiple of 4: a tower shows the same polarity there).
        #[arg(long, default_value_t = 4)]
        pitch: i32,
        /// Half-width of the square the towers may stand in.
        #[arg(long, default_value_t = 6)]
        radius: i32,
        /// Longest feed line (dust cells).
        #[arg(long, default_value_t = 12)]
        max_line: usize,
        /// Least Manhattan distance between two towers (5 leaves a corridor between any two; 4
        /// walls off diagonal pairs).
        #[arg(long, default_value_t = 5)]
        spacing: i32,
        #[arg(long, default_value = "seven-segment-towers")]
        name: String,
    },
    /// Hold each value on redsim and compare every tower torch (the design's `# tower` lines)
    /// with the value it should carry; prints the outputs and the lowest wrong torch per tower.
    Check {
        design: PathBuf,
        /// The frame read (the value is set at 10).
        #[arg(long, default_value_t = 200)]
        hold: u32,
    },
    /// Rewrite a design's block states with its state at rest on redsim (inputs off, `--frames`
    /// after placement), so that it is settled the moment it is placed.
    Settle {
        design: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 300)]
        frames: u32,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit display".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Towers { out, pitch, radius, max_line, spacing, name } => towers::run(&out, pitch, radius, max_line, spacing, &name),
        Cmd::Check { design, hold } => check::run(&design, hold),
        Cmd::Settle { design, out, frames } => settle::run(&design, out.as_deref().unwrap_or(&design), frames),
    }
}
