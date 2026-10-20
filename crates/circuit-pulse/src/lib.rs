//! Contraption study 1, track pulse (docs/circuits.md): `mctas circuit pulse <args...>`.
//! Edge detectors (rising, falling, dual), pulse limiters and pulse extenders: searches and
//! families. The official judge is `mctas circuit sim|verify` (crates/circuit); `screen` is a fast
//! redsim screen that reads EVERY cell around a design at once (to find where its output is),
//! `census` enumerates small structures with an input cell and keeps the best of each (kind, L).

pub mod family;
pub mod screen;
pub mod unarchive;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mctas circuit pulse", about = "Track pulse: edge detectors, pulse limiters and extenders")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Screen designs on redsim: the kind (rising/falling/dual-edge, limiter, extender, delay) at
    /// the declared output, or at every cell around the design without one
    Screen(screen::PulseArgs),
    /// Every structure of k blocks + an input cell, every output cell: the best designs of each
    /// (kind, L), written as circuit designs (`circuit:` header, input and output ports)
    Census(screen::PulseCensusArgs),
    /// Extendable families: the instance for any n, or the family card (`extend` headers)
    Family(family::FamilyArgs),
    /// Remove archived designs (file + verdict and caption rows): instances a family card covers
    Unarchive(unarchive::UnarchiveArgs),
}

/// `mctas circuit pulse <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit pulse".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Screen(a) => screen::run(a),
        Cmd::Census(a) => screen::census(a),
        Cmd::Family(a) => family::run(a),
        Cmd::Unarchive(a) => unarchive::run(a),
    }
}
