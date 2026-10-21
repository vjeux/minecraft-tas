//! Contraption study 8, track shulker: shulker loaders and unloaders (docs/circuits.md, the
//! variations in designs/circuits/shulker/VARIATIONS.md). `mctas circuit shulker <command> ...`
//! dispatches here. The judge itself (kinds `shulker-loader` / `shulker-unloader`, the badges) is
//! in crates/circuit/src/shulker.rs; this crate holds the searches, judged in the game, the
//! real-game traces and their redsim replay (`compare`: redsim models shulker boxes, chests, the
//! dispenser's box placement and the piston-broken drop as the bridge seeds it).

pub mod compare;
pub mod lns;
pub mod rs;
pub mod shrink;
pub mod trace;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit shulker", about = "Shulker loaders and unloaders (contraption study 8)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Greedy block removal judged in the game: each round tries removing every design block (the
    /// start lever stays), first on the plain scenario at one rotation, then the survivors on the
    /// plain scenario and every kept badge at 4 rotations; the first full survivor is removed.
    /// Ends with a full `circuit verify` of the result.
    Shrink {
        design: PathBuf,
        /// Where the smaller design goes (default: `<design>-shrunk.txt` beside it).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Badges every removal must keep (comma-separated: colours, restart, top-up, flush,
        /// box-starved, drop-free, tileable, 16-stack, unstackable); `always-on` is kept anyway.
        #[arg(long, default_value = "")]
        badges: String,
        /// Every removal must also keep the plain scenario's items/s (64-stack, rotation 0) at or
        /// above this. Without it a speed design loses a feed line: the badges still pass at a
        /// lower rate.
        #[arg(long)]
        min_rate: Option<f64>,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Real-game traces for the in-browser viewer (the clock traces' JSON Lines format), one file
    /// per design in designs/circuits/shulker/traces (or traces/exact with --exact): the judge's
    /// first build running 3 boxes of a short scenario (unstackables by default), from the filled
    /// chests to the last box.
    Trace {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// The scenario item's max stack (1: iron shovels, a short replay; 16; 64).
        #[arg(long, default_value_t = 1)]
        stack: u32,
        /// The game's own doubles and floats, each entity's motion and network id.
        #[arg(long)]
        exact: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value = "26.3")]
        version: String,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// Replay exact real-game traces (traces/exact/*.trace) in redsim, every cell and entity
    /// compared every tick (the judge's run: chests, settle, supply, lever, then each gt a tick,
    /// the frame, the judge's visit; drops from the trace's seed): redsim's shulker gate. One line
    /// per trace (OK, the first divergence, UNMODELED).
    Compare {
        traces: Vec<PathBuf>,
        /// Differences shown at the first diverging tick.
        #[arg(long, default_value_t = 6)]
        max: usize,
    },
    /// The judge's scenarios run in redsim (crates/circuit-shulker/src/rs.rs: the base scenario
    /// and every badge scenario but colours, rotation 0, the build's own drop seed): one line per
    /// scenario, OK with the judge's metrics or why not. The screen of the searches; the game
    /// (`circuit verify`) keeps the last word.
    Rs {
        designs: Vec<PathBuf>,
        /// Only these scenarios (comma-separated: base, 16-stack, unstackable, restart, top-up,
        /// flush, mixed, box-starved, drop-free, tileable).
        #[arg(long)]
        only: Option<String>,
    },
    /// The state changes of some cells during a redsim run of one scenario (default unstackable),
    /// one line per change: `t x,y,z state` (for captions: what fires when).
    Watch {
        design: PathBuf,
        /// Cells, `x,y,z;x,y,z;...`.
        #[arg(long, allow_hyphen_values = true)]
        cells: String,
        #[arg(long, default_value = "unstackable")]
        scenario: String,
        /// Stop printing after this gt.
        #[arg(long, default_value_t = 700)]
        until: u32,
        /// The site viewer's endless supply: the box chests set back to their starting boxes
        /// before every gt.
        #[arg(long)]
        endless_boxes: bool,
        /// `box-starved` with this many boxes at the start instead of 2.
        #[arg(long)]
        starve_first: Option<usize>,
        /// The one-box trickle (scenarios one-box, one-box-burst) at its own pace: `EVERY` or
        /// `EVERY,PHASE` (the next box at each gt t with (t + PHASE) % EVERY == 0); with
        /// `--sweep` every phase 0..EVERY, one line each.
        #[arg(long)]
        trickle: Option<String>,
        /// With `--trickle EVERY`: run every phase and print one result line per phase.
        #[arg(long)]
        sweep: bool,
    },
    /// Large-neighborhood shrink on redsim (lns.rs): every r-subset of the design's blocks removed
    /// (and every block outside `--box`: the box compaction), at most `--add` blocks of the
    /// palette grown back within `--local` of the removed cells, inside the box. A candidate must
    /// pass every scenario the original passes on redsim at its rate (less `--tol`); the finds
    /// smaller in (volume, blocks) are printed and saved. The game verifies them afterwards.
    Lns {
        design: PathBuf,
        #[arg(long, default_value_t = 2)]
        remove: usize,
        #[arg(long, default_value_t = 1)]
        add: usize,
        #[arg(long, default_value_t = 2)]
        local: i32,
        /// `onewide` (every block that suits a design tiled along x) or census shorthands.
        #[arg(long, default_value = "onewide")]
        palette: String,
        #[arg(long, default_value_t = 40)]
        threads: usize,
        /// The box the result must fit, `x,y,z:x,y,z` (default: the design's own).
        #[arg(long, value_name = "LO:HI", allow_hyphen_values = true)]
        r#box: Option<String>,
        /// Blocks never removed, `x,y,z;...`.
        #[arg(long, allow_hyphen_values = true)]
        keep: Option<String>,
        #[arg(long, default_value_t = 0.04)]
        tol: f64,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Random local search on redsim (lns.rs): batches of random edits (delete, re-state, add,
    /// move a block, move a port chest), the best passing candidate accepted when it does not
    /// make (volume, outer-layer blocks, blocks) worse. Every improvement is saved.
    Anneal {
        design: PathBuf,
        #[arg(long, default_value_t = 2000)]
        steps: usize,
        #[arg(long, default_value_t = 200)]
        batch: usize,
        #[arg(long, default_value_t = 40)]
        threads: usize,
        #[arg(long, default_value = "onewide")]
        palette: String,
        #[arg(long, default_value_t = 0.04)]
        tol: f64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Edits per candidate (1..=this).
        #[arg(long, default_value_t = 3)]
        moves: usize,
        /// Port chests may move.
        #[arg(long)]
        move_ports: bool,
        /// Cells the design may grow into around its box.
        #[arg(long, default_value_t = 0)]
        slack: i32,
        /// Starting temperature of the annealing (energy: 10 per block, 100 x sqrt(blocks) per
        /// box face; 0 = greedy).
        #[arg(long, default_value_t = 0.0)]
        temp: f64,
        /// A target face, `axis,plane` (e.g. `y,-3`): the energy becomes 1000 per block left on
        /// that plane (plus 10 per block), to empty it.
        #[arg(long, allow_hyphen_values = true)]
        face: Option<String>,
        /// The volume counts in the energy (1000 per cell) instead of bounding the search: with
        /// `--slack` the box may grow for a while to rearrange the design.
        #[arg(long)]
        soft_volume: bool,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Box compaction by repair (lns.rs): the design cut down to `--box` (its blocks outside
    /// removed), then random edits inside the box guided by a graded screen (boxes and items the
    /// cut design still gets done) until candidates pass every scenario the original passes on
    /// redsim; each is saved.
    Repair {
        design: PathBuf,
        #[arg(long, value_name = "LO:HI", allow_hyphen_values = true)]
        r#box: String,
        #[arg(long, default_value_t = 2000)]
        steps: usize,
        #[arg(long, default_value_t = 200)]
        batch: usize,
        #[arg(long, default_value_t = 40)]
        threads: usize,
        #[arg(long, default_value = "onewide")]
        palette: String,
        #[arg(long, default_value_t = 0.04)]
        tol: f64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 3)]
        moves: usize,
        /// Starting temperature (grade units: 3000 per good box, 10 per item moved).
        #[arg(long, default_value_t = 2000.0)]
        temp: f64,
        /// Edits only within this distance of the cut blocks.
        #[arg(long, default_value_t = 3)]
        local: i32,
        /// The design whose scenarios and rate the result must match (default: the input's own;
        /// give it to repair a hand seed that does not pass yet).
        #[arg(long)]
        target: Option<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
}

fn parse_pos(s: &str) -> Result<[i32; 3]> {
    let v: Vec<i32> = s.split(',').map(|w| w.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
    if v.len() != 3 {
        anyhow::bail!("x,y,z expected, got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit shulker".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Shrink { design, out, badges, min_rate, workers, version, margin } => {
            let keep: Vec<String> = badges.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let out = out.unwrap_or_else(|| {
                let stem = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                design.with_file_name(format!("{stem}-shrunk.txt"))
            });
            shrink::shrink(&design, &out, &keep, min_rate, workers, &version, margin)
        }
        Cmd::Trace { designs, out_dir, stack, exact, workers, version, margin } => trace::trace(&designs, out_dir, stack, exact, workers, &version, margin),
        Cmd::Lns { design, remove, add, local, palette, threads, r#box, keep, tol, out_dir } => {
            let bbox = match r#box {
                Some(s) => {
                    let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--box wants x,y,z:x,y,z"))?;
                    Some((parse_pos(a)?, parse_pos(b)?))
                }
                None => None,
            };
            let keep: Vec<[i32; 3]> = match keep {
                Some(s) => s.split(';').map(parse_pos).collect::<Result<_>>()?,
                None => Vec::new(),
            };
            let o = lns::LnsOptions { r: remove, add, local, palette: lns::palette(&palette)?, threads, bbox, tol, out_dir, keep };
            let found = lns::lns(&design, &o)?;
            println!("{} finds smaller in (volume, blocks)", found.len());
            Ok(())
        }
        Cmd::Anneal { design, steps, batch, threads, palette, tol, seed, moves, move_ports, slack, temp, face, soft_volume, out_dir } => {
            lns::set_soft_volume(soft_volume);
            let face = match face {
                Some(s) => {
                    let (a, b) = s.split_once(',').ok_or_else(|| anyhow::anyhow!("--face wants axis,plane (axis x, y or z)"))?;
                    let axis = match a.trim() {
                        "x" => 0,
                        "y" => 1,
                        "z" => 2,
                        other => anyhow::bail!("--face axis {other:?}: x, y or z"),
                    };
                    Some((axis, b.trim().parse::<i32>()?))
                }
                None => None,
            };
            let o = lns::AnnealOptions { steps, batch, threads, palette: lns::palette(&palette)?, tol, seed, moves, move_ports, slack, temp, face, out_dir: out_dir.clone() };
            let (s, d) = lns::anneal(&design, &o)?;
            println!("end: {} blocks, {}x{}x{} = {}, {:.2} items/s", s.blocks, s.dims[0], s.dims[1], s.dims[2], s.volume, s.rate);
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                d.save(&dir.join(format!("{}.txt", d.name)))?;
            }
            Ok(())
        }
        Cmd::Repair { design, r#box, steps, batch, threads, palette, tol, seed, moves, temp, local, target, out_dir } => {
            let (a, b) = r#box.split_once(':').ok_or_else(|| anyhow::anyhow!("--box wants x,y,z:x,y,z"))?;
            let o = lns::RepairOptions { bbox: (parse_pos(a)?, parse_pos(b)?), steps, batch, threads, palette: lns::palette(&palette)?, tol, seed, moves, temp, local, target, out_dir };
            let finds = lns::repair(&design, &o)?;
            println!("{} passing designs in the box", finds.len());
            for (s, _) in finds.iter().take(10) {
                println!("  {} blocks, {}x{}x{} = {}, {:.2} items/s", s.blocks, s.dims[0], s.dims[1], s.dims[2], s.volume, s.rate);
            }
            Ok(())
        }
        Cmd::Watch { design, cells, scenario, until, endless_boxes, starve_first, trickle, sweep } => {
            let c = circuit::Circuit::load(&design)?;
            let cells: Vec<[i32; 3]> = cells.split(';').map(parse_pos).collect::<Result<_>>()?;
            let (_, sc) = rs::scenarios(&c).into_iter().find(|(n, _)| *n == scenario).ok_or_else(|| anyhow::anyhow!("no scenario {scenario}"))?;
            let trickle: Option<(u32, u32)> = match trickle.as_deref() {
                None => None,
                Some(s) => {
                    let v: Vec<u32> = s.split(',').map(|x| x.trim().parse::<u32>()).collect::<std::result::Result<_, _>>()?;
                    Some((v[0].max(1), v.get(1).copied().unwrap_or(0)))
                }
            };
            if sweep {
                let (every, _) = trickle.ok_or_else(|| anyhow::anyhow!("--sweep needs --trickle EVERY"))?;
                let mut fails = 0;
                for phase in 0..every {
                    let o = rs::RunOpts { endless_boxes, starve_first, trickle: Some((every, phase)), ..Default::default() };
                    match rs::check(&c, &scenario, &sc, o) {
                        Ok(_) => println!("phase {phase}: OK"),
                        Err(why) => {
                            fails += 1;
                            println!("phase {phase}: FAIL {why}");
                        }
                    }
                }
                println!("{fails} of {every} phases fail");
                return Ok(());
            }
            let mut last: Vec<String> = vec![String::new(); cells.len()];
            let mut hook = |t: u32, w: &redsim::world::World| {
                for (i, p) in cells.iter().enumerate() {
                    let s = w.get(redsim::pos::BlockPos::new(p[0], p[1], p[2])).to_string();
                    if s != last[i] {
                        if t <= until {
                            println!("{t:>5} {},{},{} {}", p[0], p[1], p[2], clock::short(&s));
                        }
                        last[i] = s;
                    }
                }
            };
            let log = rs::run_with(&c, &sc, rs::RunOpts { endless_boxes, starve_first, trickle, ..Default::default() }, Some(&mut hook)).map_err(|e| anyhow::anyhow!("{e}"))?;
            for d in log.deliveries.iter().filter(|d| d.t <= until) {
                println!("{:>5} delivered {}", d.t, circuit::shulker::describe(&d.stack, &sc));
            }
            println!("{} boxes delivered in {} gt{}", log.deliveries.iter().filter(|d| d.stack.is_box()).count(), log.end, if log.timed_out { " (the cap)" } else { "" });
            Ok(())
        }
        Cmd::Compare { traces, max } => compare::compare(&traces, max),
        Cmd::Rs { designs, only } => {
            let only: Option<Vec<String>> = only.map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
            for d in &designs {
                let c = circuit::Circuit::load(d)?;
                for (name, sc) in rs::scenarios(&c) {
                    if only.as_ref().is_some_and(|o| !o.contains(&name)) {
                        continue;
                    }
                    let t0 = std::time::Instant::now();
                    let r = rs::check(&c, &name, &sc, rs::RunOpts::default());
                    let secs = t0.elapsed().as_secs_f64();
                    match r {
                        Ok(m) => println!("OK {} {name} ({secs:.2} s): {}", c.name(), m.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join(", ")),
                        Err(why) => println!("FAIL {} {name} ({secs:.2} s): {why}", c.name()),
                    }
                }
            }
            Ok(())
        }
    }
}
