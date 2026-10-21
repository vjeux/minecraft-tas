//! Contraption study 15: 7-segment display decoders (`circuit: seven-segment 4`, the judge in
//! crates/circuit/src/compute.rs, the board in designs/circuits/display/WR.md).
//! `mctas circuit display <command> ...` dispatches here; designs live in
//! designs/circuits/tracks/display/.

pub mod check;
pub mod settle;
pub mod stack;
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
    Towers(towers::Opts),
    /// The compact stack (crates/circuit-display/src/towers/stack_build.rs) of a stack-search
    /// solution: rings every 2 levels in a 9x9 column, rear columns as torch chains plus injections.
    Stack(towers::stack_build::StackOpts),
    /// The compact stack's feasibility search (crates/circuit-display/src/stack.rs): segment
    /// orders x corner assignments whose 4 rear columns are torch chains plus 2 literal injections.
    StackSearch {
        /// Bit 0: no outer injection (7x7); bit 1: no Y1 side dust.
        #[arg(long, default_value_t = 0)]
        model: u8,
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
        Cmd::Towers(o) => towers::run(&o),
        Cmd::Stack(o) => towers::stack_build::run(&o),
        Cmd::StackSearch { model } => stack::search(model),
        Cmd::Check { design, hold } => check::run(&design, hold),
        Cmd::Settle { design, out, frames } => settle::run(&design, out.as_deref().unwrap_or(&design), frames),
    }
}
