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
    /// Judge designs on redsim under another kind (`{n}` / `{2n}`: the design's own kind number,
    /// or twice it), e.g. study 5's counters as dividers or study 1's rising-edge detectors as
    /// monostables; prints the judge line or why it fails.
    As {
        #[arg(long)]
        kind: String,
        designs: Vec<PathBuf>,
    },
    /// Square (50% duty) /N dividers for any N, odd included: study 5's dropper ring of N with one
    /// copper bulb (its comparator toggles once every N pulses) fed on BOTH input edges: the
    /// input cell becomes a stone strongly powered by an observer watching the new input cell, so
    /// the bulb toggles every N edges = every N/2 input periods.
    SquareDivider {
        #[arg(long, value_delimiter = ',')]
        n: Vec<usize>,
        /// Most dust cells next to the ring's input (study 5's `--max-dust`).
        #[arg(long, default_value_t = 2)]
        max_dust: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// Tunable bursts for any N: the observer-chase clock (a sticky piston pushes an observer face
    /// to face with another: a pulse every 4 gt) held on by a copper-bulb gate that the edge pulse
    /// toggles on and the same pulse, delayed by a repeater loop, toggles off; the loop's delay
    /// sets N. Writes the smallest loop judged to give exactly N per N.
    BurstGate {
        #[arg(long, value_delimiter = ',')]
        n: Vec<usize>,
        /// Write the design with these loop delays (out row then back row) for the first --n,
        /// unjudged (to inspect a loop).
        #[arg(long, value_delimiter = ',')]
        delays: Vec<u32>,
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
        /// Metrics to reach first (then fewer blocks): `retrigger ignores`, `repeat <= 8`... (each
        /// matches when the metric's value starts with the text, or is at most the number).
        #[arg(long)]
        want: Vec<String>,
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
        Cmd::As { kind, designs } => {
            for p in &designs {
                println!("{}", search::judge_as(p, &kind)?);
            }
        }
        Cmd::BurstGate { n, delays, out } => {
            std::fs::create_dir_all(&out)?;
            if !delays.is_empty() {
                let k = n.first().copied().unwrap_or(2);
                let d = search::burst_gate_design(&format!("pm-burst{k}-gate"), k, &delays);
                let p = out.join(format!("{}.txt", d.name));
                d.save(&p)?;
                println!("{}", p.display());
                return Ok(());
            }
            for &k in &n {
                println!("{}", search::burst_gate(k, &out)?);
            }
        }
        Cmd::SquareDivider { n, max_dust, out } => {
            std::fs::create_dir_all(&out)?;
            for &k in &n {
                println!("{}", search::square_divider(k, max_dust, &out)?);
            }
        }
        Cmd::Improve { designs, palette, grow, iterations, rng, workers, want, out } => {
            let pal: Vec<String> = palette.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            for d in &designs {
                let r = search::improve(d, &pal, grow, iterations, rng, workers, &want, &out)?;
                println!("{r}");
            }
        }
    }
    Ok(())
}
