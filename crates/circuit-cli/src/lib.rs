//! `mctas circuit ...`: the contraption studies (docs/circuits.md). Clap definitions and dispatch
//! only; the judge is crates/circuit, each track's own commands are its crate
//! (`mctas circuit <track> <args...>` -> `circuit_<track>::run`).

use anyhow::Result;
use circuit::Circuit;
use circuit::verify::Options;
use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum CircuitCmd {
    /// Parse designs and print their kind, inputs (and how the judge drives them), outputs,
    /// payloads and block count.
    Info { designs: Vec<PathBuf> },
    /// The redsim screen (milliseconds): the kind's stimulus (sized from redsim's recovery time),
    /// one run, the kind's checker; prints the test table and `CIRCUIT: <name> <kind> <metrics>,
    /// <blocks> blocks, <WxHxD>, redsim` (or why it fails). With --stimulus (or a design without
    /// `circuit:`), the waveform of that stimulus.
    Sim {
        designs: Vec<PathBuf>,
        /// An explicit stimulus: `a 10+ 14-; b pulse 2 every 10 x 3`.
        #[arg(long)]
        stimulus: Option<String>,
        /// The test spacing in gt (default: redsim's recovery time + 10).
        #[arg(long)]
        gap: Option<u32>,
        /// Print the waveform (inputs and outputs per gt) of the whole run.
        #[arg(long)]
        wave: bool,
        /// Print the checker's table (one line per test).
        #[arg(long)]
        table: bool,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// The judge in the real game: every build (N positions x 4 rotations) runs the kind's
    /// stimulus in its own sandbox; each run is checked (no entity, nothing reaching the edge of
    /// the watched box, the kind's spec) and the builds compared. Prints `CIRCUIT: <name> <kind>
    /// <metrics>, <blocks> blocks, <WxHxD>, robust|position-dependent|rotation-dependent` or
    /// `FAIL: ...`; exits 1 unless every design passes.
    Verify {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        /// Only as built (no rotations).
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long)]
        stimulus: Option<String>,
        #[arg(long)]
        gap: Option<u32>,
        /// Print the base build's checker table.
        #[arg(long)]
        table: bool,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Judge designs in the real game and keep the passing ones in their study's archive
    /// (designs/circuits/<study>/archive/: the design with its judge line, a verdict row, a
    /// caption); a structure already archived as the same kind is refused. `--dedupe` (no
    /// designs, no game): keep one design per kind and type in every study, the others listed in
    /// aliases.tsv.
    Archive {
        designs: Vec<PathBuf>,
        /// The caption (default: the design's first comment block).
        #[arg(long)]
        caption: Option<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long)]
        dedupe: bool,
        /// Judge every archived design again with the current judge (in the game, family bases
        /// with their sampled members): passing ones get their new metrics, failing ones leave
        /// the archive and go back to their track (`update-less/` or `rejudged/`).
        #[arg(long)]
        rejudge: bool,
        /// With --rejudge: only these studies (default: all).
        #[arg(long)]
        study: Vec<String>,
        /// Take designs out of their archive first (`<study>/<file>`, e.g. one another design now
        /// dominates): the file, its verdict row and its caption.
        #[arg(long)]
        remove: Vec<String>,
    },
    /// Write every study's CATALOG.md and variants.md, the index designs/circuits/CATALOG.md and
    /// the website's data designs/circuits/catalog.tsv from the archives (no game).
    Catalog {
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
    },
    /// Calibration family generators (the judge's own test families): `repeater-line` = n + 1
    /// delay-1 repeaters in a row on stone (a delay line, N = 2 + 2n gt, 2 + 2n blocks).
    Gen {
        family: String,
        #[arg(long)]
        n: i64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Track pulse's commands (crates/circuit-pulse).
    Pulse {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track bud's commands (crates/circuit-bud): BUD switches, study 10.
    Bud {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track tff's commands (crates/circuit-tff).
    Tff {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track memory's commands (crates/circuit-memory).
    Memory {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track delay's commands (crates/circuit-delay).
    Delay {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track gates's commands (crates/circuit-gates).
    Gates {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track pext's commands (crates/circuit-pext).
    Pext {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track fly's commands (crates/circuit-fly).
    Fly {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track shulker's commands (crates/circuit-shulker).
    Shulker {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track tnt's commands (crates/circuit-tnt).
    Tnt {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Track pulsemult's commands (crates/circuit-pulsemult): study 11's searches.
    Pulsemult {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn info(c: &Circuit) -> String {
    let mut out = format!("{}: {}, {} blocks", c.name(), c.kind.as_ref().map(|k| format!("{} (study {})", k.name(), k.study())).unwrap_or_else(|| "no kind".into()), c.blocks());
    for i in &c.inputs {
        let how = match &i.how {
            circuit::InputHow::Block { on, off } => format!("{} / {}", clock::short(on), clock::short(off)),
            circuit::InputHow::Lever => "lever".into(),
            circuit::InputHow::Button => "button".into(),
        };
        out.push_str(&format!("\n  input {} at {:?}: {how}", i.name, i.pos));
    }
    for o in &c.outputs {
        out.push_str(&format!("\n  output {} at {:?}", o.name, o.pos));
    }
    for p in &c.payloads {
        out.push_str(&format!("\n  payload {} at {:?}", p.name, p.pos));
    }
    if let Some(t) = c.travel {
        out.push_str(&format!("\n  travel {t:?}{}", c.distance.map(|d| format!(", distance {d}")).unwrap_or_default()));
    }
    out
}

pub fn run(version: &str, cmd: CircuitCmd) -> Result<()> {
    match cmd {
        CircuitCmd::Info { designs } => {
            for p in &designs {
                match Circuit::load(p) {
                    Ok(c) => println!("{}", info(&c)),
                    Err(e) => println!("ERROR {}: {e:#}", p.display()),
                }
            }
        }
        CircuitCmd::Sim { designs, stimulus, gap, wave, table, margin } => {
            let o = Options { stimulus, gap, margin, version: version.to_string(), ..Default::default() };
            for p in &designs {
                let c = Circuit::load(p)?;
                let r = circuit::verify::sim(&c, &o)?;
                if let (true, Some(out)) = (table, &r.outcome) {
                    for l in &out.table {
                        println!("  {l}");
                    }
                }
                if let Ok(t) = &r.trace
                    && (wave || r.outcome.is_none())
                {
                    for l in t.strip(&c, 0, t.frames()) {
                        println!("  {l}");
                    }
                }
                println!("{}", r.summary(&c));
            }
        }
        CircuitCmd::Verify { designs, positions, no_rotations, workers, stimulus, gap, table, margin } => {
            let o = Options { positions, rotations: !no_rotations, workers, stimulus, gap, margin, version: version.to_string() };
            let pool = circuit::verify::spawn_pool(&o)?;
            let mut failed = 0;
            for p in &designs {
                let c = Circuit::load(p)?;
                let rep = circuit::verify::verify_in_pool(&pool, &c, &o)?;
                if table {
                    // the base build's table, else (nothing passes) the first build's: why it fails
                    let b = rep.base().or_else(|| rep.builds.first().and_then(|b| b.result.as_ref().ok()));
                    for l in b.map(|b| b.outcome.table.as_slice()).unwrap_or(&[]) {
                        println!("  {l}");
                    }
                    // and the first failing build's, when some pass
                    if rep.base().is_some()
                        && let Some(f) = rep.builds.iter().find(|b| b.result.as_ref().is_ok_and(|b| !b.outcome.pass))
                        && let Ok(fo) = &f.result
                    {
                        println!("  == failing build: rotation {} at {:?}", f.turns, f.offset);
                        for l in &fo.outcome.table {
                            println!("  {l}");
                        }
                    }
                }
                if !rep.pass {
                    failed += 1;
                    for b in rep.builds.iter().filter(|b| !b.result.as_ref().is_ok_and(|b| b.outcome.pass)).take(3) {
                        let why = match &b.result {
                            Ok(b) => b.outcome.why.clone(),
                            Err(e) => e.clone(),
                        };
                        println!("  rotation {} at {:?}: {why}", b.turns, b.offset);
                    }
                }
                if !rep.note.is_empty() {
                    println!("  {}", rep.note);
                }
                println!("{}", rep.summary(&c));
            }
            if failed > 0 {
                std::process::exit(1);
            }
        }
        CircuitCmd::Archive { designs, caption, root, positions, workers, dedupe, rejudge, study, remove } => {
            for spec in &remove {
                println!("{}", circuit::archive::unarchive(&root, spec)?);
            }
            if !remove.is_empty() && designs.is_empty() && !rejudge && !dedupe {
                return Ok(());
            }
            if dedupe {
                for l in circuit::archive::dedupe(&root)? {
                    println!("{l}");
                }
                if designs.is_empty() && !rejudge {
                    return Ok(());
                }
            }
            let o = Options { positions, workers, version: version.to_string(), ..Default::default() };
            let pool = circuit::verify::spawn_pool(&o)?;
            if rejudge {
                for l in circuit::archive::rejudge(&pool, &root, &o, &study)? {
                    println!("{l}");
                }
            }
            for p in &designs {
                match circuit::archive::archive(&pool, p, &o, &root, caption.as_deref()) {
                    Ok(a) => println!("{} {}: {}", if a.new { "ARCHIVED" } else { "SKIPPED" }, a.file.display(), a.summary),
                    Err(e) => println!("REFUSED {}: {e:#}", p.display()),
                }
            }
        }
        CircuitCmd::Catalog { root } => {
            for l in circuit::archive::catalog(&root)? {
                println!("{l}");
            }
            println!("{}/CATALOG.md, {}/catalog.tsv", root.display(), root.display());
        }
        CircuitCmd::Gen { family, n, out } => {
            if family != "repeater-line" {
                anyhow::bail!("unknown family {family:?} (repeater-line)");
            }
            let k = (n + 1).max(1) as usize;
            let text = format!(
                "name: cal-repeater-line-n{n}\ncircuit: delay\n# n + 1 delay-1 repeaters in a row on stone: a delay line of N = 2 + 2n gt.\nextend family: cal-repeater-line\nextend axis: 2 + 2n\nextend blocks: 2 + 2n\nextend rule: one more repeater (and its stone) at the end of the line\nextend n: 0..8\nextend base: {n}\nextend generator: mctas circuit gen repeater-line --n {{n}} --out {{out}}\nport input: 0 1 0\nport output: {} 1 0\npalette:\n  S minecraft:stone\n  r minecraft:repeater[delay=1,facing=west,locked=false,powered=false]\nlayer 0:\n  .{}\nlayer 1:\n  .{}\n",
                k + 1,
                "S".repeat(k),
                "r".repeat(k)
            );
            std::fs::write(&out, text)?;
            println!("{}", out.display());
        }
        CircuitCmd::Pulse { args } => circuit_pulse::run(args)?,
        CircuitCmd::Bud { args } => circuit_bud::run(args)?,
        CircuitCmd::Tff { args } => circuit_tff::run(args)?,
        CircuitCmd::Memory { args } => circuit_memory::run(args)?,
        CircuitCmd::Delay { args } => circuit_delay::run(args)?,
        CircuitCmd::Gates { args } => circuit_gates::run(args)?,
        CircuitCmd::Pext { args } => circuit_pext::run(args)?,
        CircuitCmd::Fly { args } => circuit_fly::run(args)?,
        CircuitCmd::Shulker { args } => circuit_shulker::run(args)?,
        CircuitCmd::Tnt { args } => circuit_tnt::run(args)?,
        CircuitCmd::Pulsemult { args } => circuit_pulsemult::run(args)?,
    }
    Ok(())
}
