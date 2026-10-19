//! Track clk-item's clock commands: `mctas clock item <args...>` calls [`run`] with every argument
//! after `item`. OWNED BY TRACK clk-item (item clocks: hoppers, droppers, comparators reading
//! containers, item-count-tuned periods).
//!
//! * [`sim`]: a design (blocks + `nbt:` container contents) run in redsim, the exact model, until
//!   its WHOLE state repeats (block states, comparator outputs, container slots, hopper cooldowns,
//!   pending ticks, torch memory, moving blocks): the first repeat gives the exact period and the
//!   transient; then one period is replayed to get the active set (blocks whose state or
//!   container contents change: the family) and the output waveform.
//! * [`census`]: every design of k blocks in a box from a palette, with container contents from a
//!   list, judged by [`sim`]; irreducible clocks deduplicated by type (translation + the 4
//!   rotations) and by mechanism, written per period, smallest first.
//! * [`jvm`]: the same design placed in the real game (the bridge), its block states and
//!   block-entity data recorded every tick and compared with redsim; the period measured there.
//!
//! The official judge is `mctas clock verify` (crates/clock, docs/clocks.md); this crate is the
//! item family's search and measuring tool.

pub mod ccensus;
pub mod census;
pub mod cli;
pub mod families;
pub mod fill;
pub mod jvm;
pub mod sim;

use clap::Parser;

#[derive(Parser)]
#[command(name = "mctas clock item", about = "Item clocks (track clk-item): run in redsim, measure in the game, census")]
struct Args {
    /// Minecraft version (the game runs of `verify`).
    #[arg(long, global = true, default_value = mctas::DEFAULT_VERSION)]
    version: String,
    #[command(subcommand)]
    cmd: cli::ItemClockCmd,
}

/// `mctas clock item <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let a = match Args::try_parse_from(std::iter::once("mctas clock item".to_string()).chain(args)) {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => {
            print!("{e}");
            return Ok(());
        }
        Err(e) => anyhow::bail!("{e}"),
    };
    cli::run(&a.version, a.cmd)
}
