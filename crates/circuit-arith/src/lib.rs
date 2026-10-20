//! Study 12 (computational redstone), track arith: adders, subtractors, incrementers and
//! comparators (designs/circuits/compute/VARIATIONS.md). `mctas circuit arith <command>`:
//! `check` runs designs through the judge's walk on redsim (the screen before the game judge);
//! `synth` searches small flat designs with a SAT model of steady-state redstone, each candidate
//! checked on redsim (CEGAR). Notes: designs/circuits/compute/tracks/arith/NOTES.md.

pub mod eval;
pub mod func;
pub mod synth;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "arith", about = "Adders, subtractors, incrementers, comparators (study 12, track arith)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The judge's walk on redsim: every output settles to the ideal in every hold; latency,
    /// glitches, blocks.
    Check {
        designs: Vec<PathBuf>,
        /// The function, when the design has no `circuit:` line (adder N, comparator N, ...).
        #[arg(long)]
        kind: Option<String>,
        /// Frames each state is held.
        #[arg(long, default_value_t = 20)]
        gap: u32,
        /// Print every transition.
        #[arg(long)]
        table: bool,
    },
    /// SAT synthesis of a small flat design for a function (see synth.rs).
    Synth(synth::SynthArgs),
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = match Cli::try_parse_from(std::iter::once("mctas circuit arith".to_string()).chain(args)) {
        Ok(c) => c,
        Err(e) => {
            e.print()?;
            return if e.use_stderr() { Err(anyhow!("bad arguments")) } else { Ok(()) };
        }
    };
    match cli.cmd {
        Cmd::Check { designs, kind, gap, table } => {
            let mut bad = 0;
            for p in &designs {
                match check_one(p, kind.as_deref(), gap, table) {
                    Ok(line) => println!("{line}"),
                    Err(e) => {
                        bad += 1;
                        println!("FAIL: {}: {e:#}", p.display());
                    }
                }
            }
            if bad > 0 {
                return Err(anyhow!("{bad} design(s) fail"));
            }
            Ok(())
        }
        Cmd::Synth(a) => synth::cmd(a),
    }
}

fn check_one(p: &std::path::Path, kind: Option<&str>, gap: u32, table: bool) -> Result<String> {
    let (d, f, rig) = eval::load(p, kind)?;
    let seq = func::walk(f.n());
    let r = eval::run(&rig, &f, &seq, gap);
    if table {
        for s in &r.steps {
            println!("  {} -> {}: {} gt{}", f.label(s.from), f.label(s.to), s.latency, if s.glitches > 0 { format!(", {} glitch(es)", s.glitches) } else { String::new() });
        }
    }
    if !r.pass {
        return Err(anyhow!("{}", r.why));
    }
    let (dims, height) = eval::dims(&rig);
    let worst = &r.steps[r.worst];
    let per: Vec<String> = f.outputs.iter().zip(&r.per_output).map(|(o, l)| format!("{o} {l}")).collect();
    Ok(format!(
        "ARITH: {} {}: latency {} gt ({} -> {}), per output [{}], glitches {}, {} blocks, {dims}, height {height}, {} transitions, redsim",
        d.name,
        f.kind,
        r.latency,
        f.label(worst.from),
        f.label(worst.to),
        per.join(", "),
        r.glitches,
        eval::blocks(&d),
        r.steps.len()
    ))
}
