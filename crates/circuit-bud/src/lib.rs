//! Contraption study 10, track bud (docs/circuits.md, designs/circuits/bud/VARIATIONS.md):
//! `mctas circuit bud <args...>`. BUD switches. The official judge is `mctas circuit sim|verify`
//! (crates/circuit, kind `bud`); `census` enumerates every small design around a target cell and
//! screens it on redsim with the same judge at every candidate output cell.

pub mod census;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mctas circuit bud", about = "Track bud: BUD switches")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Every design of K blocks around a target cell (connected, one block beside the target),
    /// deduplicated by structure, screened on redsim with the BUD judge at every air cell next to
    /// it; per detected class set: the count, the smallest and the fastest
    Census(census::CensusArgs),
    /// A design aid: run a BUD design's stimulus (classes only) on redsim and print, per frame in
    /// a window, the phase, the output level and the blocks that changed
    Trace(census::TraceArgs),
}

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit bud".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Census(a) => census::run(a),
        Cmd::Trace(a) => census::trace(a),
    }
}
