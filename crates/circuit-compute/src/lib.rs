//! Study 12, track compute (designs/circuits/compute/VARIATIONS.md): `mctas circuit compute
//! <args...>`. The official judge is `mctas circuit sim|verify` (crates/circuit, `compute.rs`);
//! `census` fills a box around fixed input cells with every placement of k palette blocks
//! (track delay's search, `circuit_delay::census`) and screens each filling on redsim with the
//! judge's own stimulus and ideal outputs, every candidate output cell read at once; each output
//! gets a distinct cell that matches it, and the smallest and the fastest designs are written.

pub mod census;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mctas circuit compute", about = "Study 12, track compute: muxes, decoders, encoders, counters")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Every filling of a box with kmin..=k palette blocks around fixed input cells, screened with
    /// the study 12 judge's stimulus on redsim (every candidate output cell at once); the smallest
    /// and the fastest designs per kind, written as design files
    Census(census::CensusArgs),
}

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit compute".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Census(a) => census::run(a),
    }
}
