//! Track clk-obs of the clock study (designs/clocks/tracks/clk-obs): generators and searches for
//! block-update clocks: observers, pistons and sticky pistons (moved blocks, moving observers),
//! copper bulbs, note blocks, lamps, trapdoors, rails and their hybrids. OWNED BY TRACK clk-obs.
//!
//! Commands (`mctas clock obs <cmd>`; clockinfra's clock-cli passes every argument to [`run`]):
//! * `sim <designs...>`: period, warmup, family and waves of each design on redsim (this crate's
//!   fast runner, `sim.rs`; the official numbers are `mctas clock sim`).
//! * `rings --max-len N`: every pure observer ring (a closed self-avoiding lattice loop, each
//!   observer watching the previous one) up to N observers, simulated and counted per period.
//! * `census`: every placement of k blocks from the family palette on every polycube shape, each
//!   with every kick (self-start, each block placed last), simulated; the clocks found per period.
//! * `grow`: one block added or replaced around known clocks.

pub mod census;
pub mod cores;
pub mod divider;
pub mod grow;
pub mod pick;
pub mod report;
pub mod rings;
pub mod sim;

use anyhow::Result;
use clap::{Args as ClapArgs, Parser, Subcommand};
use std::path::PathBuf;

/// `mctas clock obs <args...>`.
#[derive(Parser, Debug)]
#[command(name = "mctas clock obs", about = "Clock study, track clk-obs: block-update clock generators and searches")]
struct Cli {
    #[command(flatten)]
    args: Args,
}

/// The entry point clockinfra's `mctas clock obs` calls with every argument after `obs`.
pub fn run(args: Vec<String>) -> Result<()> {
    let cli = match Cli::try_parse_from(std::iter::once("mctas clock obs".to_string()).chain(args)) {
        Ok(c) => c,
        Err(e) => {
            // --help and usage errors print through clap
            e.print()?;
            return if e.use_stderr() { Err(anyhow::anyhow!("bad arguments")) } else { Ok(()) };
        }
    };
    run_args(cli.args)
}

#[derive(ClapArgs, Debug)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Simulate clock designs on redsim: verdict, period, warmup, family, blocks, swept volume,
    /// and the wave of every changing cell (on ticks per period, rising edges, pulse widths)
    Sim {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 3000)]
        horizon: u32,
        /// Print every changing cell's wave
        #[arg(long)]
        cells: bool,
        /// Print the changing cells' states tick by tick for warmup + this many periods
        #[arg(long)]
        trace: Option<u32>,
    },
    /// Every pure observer ring up to --max-len observers (closed self-avoiding loops on the
    /// lattice, each observer watching the previous one, kicked by placing one observer last):
    /// simulated, counted per period up to translation and the 4 horizontal rotations
    Rings {
        #[arg(long, default_value_t = 8)]
        max_len: usize,
        /// Write one design per type (period <= this) into --out-dir
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        write_max_period: u32,
        /// Rings with one relay instead of one observer: note, lamp, bulb, trap (comma-separated)
        #[arg(long)]
        relay: Option<String>,
        /// Experiment: a 2 x n/2 ring of --max-len observers carrying pulses injected at these
        /// ticks (comma-separated): the steady period of a multi-pulse pure observer ring
        #[arg(long)]
        pulses: Option<String>,
    },
    /// Exhaustive census: every assignment of --palette states to the cells of every polycube of
    /// --k cells (face-connected, one shape per horizontal rotation class), each with every kick
    /// (self-start, each cell placed last); the distinct clocks per period (types up to
    /// translation and rotation), smallest first
    Census(census::CensusArgs),
    /// Local search around known clocks: one block added or replaced, simulated, new minimal
    /// clocks per period
    Grow(grow::GrowArgs),
    /// The smallest clocks per (period, family) from search outputs, re-run with the official
    /// redsim judge, written with names that say what they are
    Pick(pick::PickArgs),
    /// Group clocks by their steady-state CORE (the changing cells + the static blocks they
    /// need, up to translation, rotation and phase): structures vs distinct machines per period
    Cores(cores::CoresArgs),
    /// Long periods: a copper bulb + comparator divider chain out of an observer's back of a base
    /// clock (P = P0 x 2^bulbs)
    Divider(divider::DividerArgs),
    /// The verified clocks of clock verify logs, matched to their design files: a markdown table
    /// (and the coordinator's one-line format)
    Report(report::ReportArgs),
}

pub fn run_args(args: Args) -> Result<()> {
    match args.cmd {
        Cmd::Sim { designs, horizon, cells, trace } => {
            for d in designs {
                let spec = sim::Spec::load(&d)?;
                let r = sim::simulate(&spec, &sim::Opts { horizon, ..Default::default() });
                let out = r
                    .output
                    .as_ref()
                    .map(|o| format!(" output {} on {}/{} rising {} widths {:?}", o.pos, o.on_ticks.unwrap_or(0), r.period, o.rising, o.widths))
                    .unwrap_or_default();
                println!(
                    "{}: {:?} P={} warmup={} family={} blocks={} volume={}{}",
                    d.display(),
                    r.verdict,
                    r.period,
                    r.warmup,
                    r.family_name(),
                    r.blocks,
                    r.volume(),
                    out
                );
                if cells {
                    for c in &r.cells {
                        println!("    {} {:?} on={:?} rising={} widths={:?}", c.pos, c.names, c.on_ticks, c.rising, c.widths);
                    }
                }
                if let Some(np) = trace {
                    let ps: Vec<redsim::BlockPos> = r.cells.iter().map(|c| c.pos).collect();
                    println!("    cells: {}", ps.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" "));
                    for l in sim::trace_lines(&spec, r.warmup + np * r.period.max(1), &ps, 6) {
                        println!("    {l}");
                    }
                }
            }
            Ok(())
        }
        Cmd::Rings { max_len, out_dir, write_max_period, relay, pulses } => match (relay, pulses) {
            (_, Some(p)) => rings::run_pulsetrain(max_len, &p.split(',').filter_map(|t| t.trim().parse().ok()).collect::<Vec<u32>>()),
            (Some(k), None) => rings::run_relays(max_len, &k, out_dir, write_max_period),
            (None, None) => rings::run(max_len, out_dir, write_max_period),
        },
        Cmd::Census(a) => census::run(a),
        Cmd::Grow(a) => grow::run(a),
        Cmd::Pick(a) => pick::run(a),
        Cmd::Cores(a) => cores::run(a),
        Cmd::Divider(a) => divider::run(a),
        Cmd::Report(a) => report::run(a),
    }
}
