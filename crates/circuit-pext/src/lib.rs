//! Contraption study 6, track pext: PISTON EXTENDERS (designs/circuits/pext/). Push a block out
//! N cells and bring it back exactly, from one lever (on = out, off = back) or one button press
//! (out, then back by itself), nothing left behind. Axis: the reach N; metrics: blocks, extend gt,
//! retract gt, input. OWNED BY TRACK pext.
//!
//! Commands (`mctas circuit pext <cmd>`; clockinfra's circuit CLI passes every argument to
//! [`run`]):
//! * `sim <designs...>`: the redsim screen ([`rs`]): extend / retract ticks of every cycle.
//! * `trace <design>`: every cell change, tick by tick, through placement and the first cycle.
//! * `verify <designs...>`: the real game at 4 positions x 4 rotations ([`game`]).

pub mod game;
pub mod rs;
pub mod spec;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use spec::Pext;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "mctas circuit pext", about = "Contraption study 6 (track pext): piston extenders")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// The redsim screen: place like the judge, settle, run the input's cycles; prints the
    /// extend / retract ticks (counted from the input event) or why it fails.
    Sim {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 3)]
        cycles: usize,
        /// Print every warning (rest state differs from the design, cycles differ).
        #[arg(short, long)]
        verbose: bool,
    },
    /// Every cell change, tick by tick, of the placement's settling and the first cycle (redsim).
    Trace { design: PathBuf },
    /// The real game: N positions x 4 rotations, each build in a fresh sandbox, cycles recorded
    /// tick by tick; robust / position-dependent / rotation-dependent, and redsim's agreement.
    Verify {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 3)]
        cycles: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Print every build's numbers.
        #[arg(short, long)]
        verbose: bool,
    },
}

/// The entry point clockinfra's `mctas circuit pext` calls with every argument after `pext`.
pub fn run(args: Vec<String>) -> Result<()> {
    let cli = match Cli::try_parse_from(std::iter::once("mctas circuit pext".to_string()).chain(args)) {
        Ok(c) => c,
        Err(e) => {
            e.print()?;
            return if e.use_stderr() { Err(anyhow!("bad arguments")) } else { Ok(()) };
        }
    };
    match cli.cmd {
        Cmd::Sim { designs, cycles, verbose } => cmd_sim(&designs, cycles, verbose),
        Cmd::Trace { design } => cmd_trace(&design),
        Cmd::Verify { designs, positions, no_rotations, cycles, workers, verbose } => {
            let o = game::Options { positions, rotations: !no_rotations, cycles, workers, ..Default::default() };
            cmd_verify(&designs, &o, verbose)
        }
    }
}

/// `4x3x1` of the design's box with the target cell.
pub fn dims(p: &Pext) -> String {
    let (lo, hi) = p.bounds();
    format!("{}x{}x{}", hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1)
}

/// The category words: slime/honey or slime-free, and flat when the build is one block wide
/// across the travel axis.
pub fn category(p: &Pext) -> String {
    let mut v = vec![if p.sticky_blocks() { "slime/honey" } else { "slime-free" }];
    let (lo, hi) = p.design.bounds().unwrap_or(([0; 3], [0; 3]));
    let ext: Vec<i32> = (0..3).map(|i| hi[i] - lo[i] + 1).collect();
    // flat = 1 wide on a horizontal axis across the travel (a wall slot)
    let across: Vec<usize> = (0..3).filter(|&i| i != 1 && p.travel[i] == 0).collect();
    if across.iter().any(|&i| ext[i] == 1) {
        v.push("flat");
    }
    if p.kinds().contains("observer") {
        v.push("observer");
    }
    v.join(", ")
}

pub fn summary(p: &Pext, r: &rs::Report) -> String {
    let c = r.first();
    match r.verdict {
        rs::Verdict::Works => format!(
            "PEXT: {} N={} {}, {}, extend {} gt, retract {} gt{}, {} blocks, {}, {}",
            p.name(),
            p.reach,
            spec::travel_name(p.travel),
            p.input_kind.word(),
            c.extend,
            c.retract,
            if p.input_kind == spec::InputKind::Button { format!(" (from the press), hold {} gt", c.hold) } else { String::new() },
            p.blocks(),
            dims(p),
            category(p)
        ),
        v => format!("{}: {} {}", v.word(), p.name(), r.why),
    }
}

fn cmd_sim(designs: &[PathBuf], cycles: usize, verbose: bool) -> Result<()> {
    let params = rs::Params { cycles, ..Default::default() };
    for path in designs {
        let p = match Pext::load(path) {
            Ok(p) => p,
            Err(e) => {
                println!("INVALID: {}: {e:#}", path.display());
                continue;
            }
        };
        let prep = match rs::Prepared::new(&p, 4) {
            Ok(x) => x,
            Err(e) => {
                println!("UNMODELED: {} {e}", p.name());
                continue;
            }
        };
        let (r, _) = rs::judge(&prep, &params, false);
        println!("{}", summary(&p, &r));
        if verbose || !r.works() {
            for w in &r.warnings {
                println!("  warning: {w}");
            }
        }
    }
    Ok(())
}

fn cmd_trace(path: &PathBuf) -> Result<()> {
    let p = Pext::load(path)?;
    let prep = rs::Prepared::new(&p, 4).map_err(|e| anyhow!("not modeled by redsim: {e}"))?;
    let params = rs::Params { cycles: 1, ..Default::default() };
    let (r, lines) = rs::judge(&prep, &params, true);
    for l in lines {
        println!("{l}");
    }
    println!("{}", summary(&p, &r));
    for w in &r.warnings {
        println!("  warning: {w}");
    }
    Ok(())
}

fn cmd_verify(designs: &[PathBuf], o: &game::Options, verbose: bool) -> Result<()> {
    let pool = game::spawn_pool(o)?;
    for path in designs {
        let p = match Pext::load(path) {
            Ok(p) => p,
            Err(e) => {
                println!("INVALID: {}: {e:#}", path.display());
                continue;
            }
        };
        let rep = game::verify_in_pool(&pool, &p, o)?;
        if verbose {
            for b in &rep.builds {
                match &b.result {
                    Ok(c) => println!("  rotation {} at {:?}: {:?}", b.turns, b.offset, c),
                    Err(e) => println!("  rotation {} at {:?}: FAIL {e}", b.turns, b.offset),
                }
            }
        }
        let redsim = match rs::Prepared::new(&p, 4) {
            Ok(prep) => {
                let (r, _) = rs::judge(&prep, &rs::Params { cycles: o.cycles, ..Default::default() }, false);
                match (r.works(), rep.base()) {
                    (true, Some(b)) if r.cycles == *b => "redsim AGREES".to_string(),
                    (true, Some(b)) => format!("redsim DIFFERS: {:?} vs game {:?}", r.first(), b.first()),
                    (true, None) => "redsim: WORKS".to_string(),
                    (false, _) => format!("redsim: {} {}", r.verdict.word(), r.why),
                }
            }
            Err(e) => format!("redsim: UNMODELED {e}"),
        };
        match (rep.works, rep.base()) {
            (true, Some(b)) => {
                let c = b[0];
                println!(
                    "PEXT: {} N={} {}, {}, extend {} gt, retract {} gt{}, {} blocks, {}, {}, {} ({} builds), {}",
                    p.name(),
                    p.reach,
                    spec::travel_name(p.travel),
                    p.input_kind.word(),
                    c.extend,
                    c.retract,
                    if p.input_kind == spec::InputKind::Button { format!(" (from the press), hold {} gt", c.hold) } else { String::new() },
                    p.blocks(),
                    dims(&p),
                    category(&p),
                    rep.robustness,
                    rep.builds.len(),
                    redsim
                );
            }
            _ => println!("FAILS: {} {} ({})", p.name(), rep.robustness, redsim),
        }
    }
    Ok(())
}
