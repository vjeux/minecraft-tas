//! Track tff's circuit commands: `mctas circuit tff <args...>` calls [`run`] with every argument
//! after `tff`. OWNED BY TRACK tff (T flip-flops; docs/circuits.md).
//!
//! * [`model`]: a circuit design (inputs, outputs, kind).
//! * [`rs`]: the redsim runner (the judge's placement, inputs between ticks, output levels).
//! * [`tff`]: the TFF metrics (latency, min-pulse, min-period, edge) on redsim.

pub mod census;
pub mod cli;
pub mod game;
pub mod model;
pub mod rs;
pub mod tff;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mctas circuit tff", about = "T flip-flops (track tff): redsim metrics, families, census")]
struct Args {
    #[command(subcommand)]
    cmd: cli::Cmd,
}

/// `mctas circuit tff <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let a = match Args::try_parse_from(std::iter::once("mctas circuit tff".to_string()).chain(args)) {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => {
            print!("{e}");
            return Ok(());
        }
        Err(e) => anyhow::bail!("{e}"),
    };
    cli::run(a.cmd)
}
