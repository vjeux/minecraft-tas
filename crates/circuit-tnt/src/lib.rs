//! Contraption study 9, track tnt: TNT dupers (docs/circuits.md, the variations and the
//! mechanism in designs/circuits/tnt/VARIATIONS.md). `mctas circuit tnt <command> ...`
//! dispatches here.

pub mod game;
pub mod sim;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit tnt", about = "TNT dupers (contraption study 9)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run designs in redsim: every TNT primed, the TNT blocks standing, the period; prints
    /// DUPES (primes while the TNT blocks stay, periodic) / PRIMES / CONSUMES / NOTHING.
    Sim {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 400)]
        ticks: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Print every cell change of the first N ticks.
        #[arg(long, default_value_t = 0)]
        trace: u32,
    },
    /// Run designs in the real game (the base build): every primed TNT recorded (tick, position,
    /// motion) and killed before it can explode, the TNT blocks standing, the period; then redsim's
    /// line for the same design.
    Run {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 200)]
        ticks: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Print every spawn.
        #[arg(long)]
        spawns: bool,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit tnt".to_string()).chain(args))?;
    match cli.cmd {
        Cmd::Sim { designs, ticks, margin, trace } => {
            for d in &designs {
                let c = clock::Clock::load(d)?;
                let r = sim::run(&c, ticks, margin, trace);
                println!("{}: {}", d.display(), r.line());
            }
        }
        Cmd::Run { designs, ticks, margin, spawns } => {
            let mut w = mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig::default())?;
            for d in &designs {
                let c = clock::Clock::load(d)?;
                let g = game::run_one(&mut w, &c, ticks, margin)?;
                println!("{}: game: {}", d.display(), g.run.line());
                if spawns {
                    for s in &g.spawns {
                        println!("  gt {}: pos {:?} motion {:?}", s.tick, s.pos, s.motion);
                    }
                }
                let r = sim::run(&c, ticks, margin, 0);
                // over the ticks both runs saw (each stops 2 periods after its own period check)
                let n = r.ticks.min(g.run.ticks);
                let pr = |v: &[(u32, redsim::BlockPos)]| v.iter().filter(|(t, _)| *t <= n).copied().collect::<Vec<_>>();
                let same = pr(&r.primes) == pr(&g.run.primes) && r.tnt[..=n as usize] == g.run.tnt[..=n as usize];
                println!("  redsim: {} [{}]", r.line(), if same { format!("primes and TNT counts identical over {n} gt") } else { "DIFFERS".into() });
            }
        }
    }
    Ok(())
}
