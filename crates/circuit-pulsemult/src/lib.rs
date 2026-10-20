//! Contraption study 11, track pulsemult: pulse multipliers, frequency dividers, monostables and
//! timers (docs/circuits.md "Study 11 timers", designs/circuits/pulsemult/VARIATIONS.md).
//! `mctas circuit pulsemult <command> ...` dispatches here.

pub mod search;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit pulsemult", about = "pulse multipliers, dividers, monostables (contraption study 11)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Random search on redsim: the seed design (its `circuit:` kind, ports and any fixed blocks)
    /// plus k blocks drawn from the palette at random cells of the box (every block standing:
    /// dust, diodes and torches on their support). A quick screen (one input pulse, or a short
    /// clock for dividers) then the judge's own redsim check; every pass is written once per block
    /// set to DIR/designs/ with its judge line in DIR/found.tsv.
    Search {
        #[arg(long)]
        seed: PathBuf,
        /// Search box `x0,y0,z0,x1,y1,z1` (inclusive).
        #[arg(long = "box")]
        bx: String,
        /// Comma list: stone, dust, repeater (all delays), repeater1..repeater4, comparator,
        /// torch (standing and wall), observer, piston, sticky, rblock, bulb, slime, or block
        /// states.
        #[arg(long, default_value = "stone,dust,repeater,comparator,torch,observer")]
        palette: String,
        #[arg(long, default_value_t = 5)]
        k: usize,
        #[arg(long, default_value_t = 2)]
        kmin: usize,
        /// Shorthands every sample must contain.
        #[arg(long, default_value = "")]
        require: String,
        #[arg(long, default_value_t = 1_000_000)]
        samples: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        /// Stop recording after this many distinct passes.
        #[arg(long, default_value_t = 500)]
        max_found: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// Local search from passing designs: add, remove, move or change blocks (box: the design's
    /// box grown by --grow), keep a change when the judge still passes with the same axis value
    /// (N, L and class, T or P) and no more blocks; writes the smallest found per start.
    Improve {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "stone,dust,repeater,comparator,torch,observer")]
        palette: String,
        #[arg(long, default_value_t = 1)]
        grow: i32,
        #[arg(long, default_value_t = 20_000)]
        iterations: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 16)]
        workers: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit pulsemult".to_string()).chain(args))?;
    match cli.cmd {
        Cmd::Search { seed, bx, palette, k, kmin, require, samples, rng, workers, max_found, out } => {
            let v: Vec<i32> = bx.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 6, "--box x0,y0,z0,x1,y1,z1");
            let o = search::SearchOptions {
                seed,
                lo: [v[0].min(v[3]), v[1].min(v[4]), v[2].min(v[5])],
                hi: [v[0].max(v[3]), v[1].max(v[4]), v[2].max(v[5])],
                palette: palette.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                k,
                kmin,
                require: require.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                samples,
                rng,
                workers,
                max_found,
                out,
            };
            let n = search::search(&o)?;
            println!("{n} distinct passes: {}", o.out.join("found.tsv").display());
        }
        Cmd::Improve { designs, palette, grow, iterations, rng, workers, out } => {
            let pal: Vec<String> = palette.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            for d in &designs {
                let r = search::improve(d, &pal, grow, iterations, rng, workers, &out)?;
                println!("{r}");
            }
        }
    }
    Ok(())
}
