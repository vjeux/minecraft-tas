//! `mctas door` commands: redsim-based tools: cosearch, cycle, sheet, redsim-fuzz, redsim-case, redsim-verify, pistfuzz.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Flip a design's lever N times in redsim at the judge's 4 positions and check it alternates
    /// between an open state (opening clear, frame 12/12) and a closed one (cycle.rs). Accepts a
    /// door that rests open, e.g. a community design read from a screenshot of its open state.
    Cycle {
        design: PathBuf,
        #[arg(long, default_value_t = 6)]
        flips: usize,
        /// Write the settled state after this many flips (position 0) as a design to --out.
        #[arg(long)]
        write_state: Option<usize>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Draw mechanisms by eye (sheet.rs): for each ideal schedule (or design), the door's plane at
    /// rest and once the opening settles (replayed in redsim, no Minecraft), as one markdown page.
    Sheet {
        #[arg(required = true, num_args = 1..)]
        files: Vec<PathBuf>,
        /// Write the page here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Page title.
        #[arg(long)]
        heading: Option<String>,
        /// Also test static stone in the frame cells left empty when open (2 cycles in redsim).
        #[arg(long)]
        fill: bool,
        /// With --fill: write each working filled mechanism (`<name>-ff.txt` + `.ideal`) here.
        #[arg(long)]
        fill_dir: Option<PathBuf>,
        /// Keep only mechanisms whose single piston below the opening is a sticky piston facing up.
        #[arg(long)]
        single_up_below: bool,
    },
    /// Pure-Rust door judge on redsim (no JVM: the judge's checks, FULL seamless, the hallway
    /// check, the volume), its agreement with the real judge, and the joint mechanism + wiring
    /// search on top of it (track cosearch, cosearch.rs / cosearch_search.rs).
    Cosearch {
        #[command(subcommand)]
        cmd: crate::cosearch::CosearchCmd,
    },
    /// Differential fuzzing of the redsim model (crates/redsim) against the real game: random
    /// circuits run in both, every block state compared after placement and every tick and flip;
    /// mismatches are shrunk and saved as case files (`door redsim-case` replays one).
    RedsimFuzz {
        /// Random circuits to run.
        #[arg(long, default_value_t = 1000)]
        n: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Box size x,y,z.
        #[arg(long, default_value = "6,3,6")]
        size: String,
        /// Chance that a cell of the box gets a component.
        #[arg(long, default_value_t = 0.45)]
        density: f64,
        /// Families with weights: `core`, `extra`, `all`, or `<family>[:w],...` (dust, repeater,
        /// comparator, torch, walltorch, observer, rblock, lamp, bulb, note, target, lever, button,
        /// stone, glass, simple, trapdoor, gate, skull, rail, dispenser, crafter, hopper, slab,
        /// leaves, log, analog, door).
        #[arg(long, default_value = "core")]
        palette: String,
        /// Where shrunk mismatch cases go.
        #[arg(long, default_value = "designs/redsim/fails")]
        out: PathBuf,
        /// Do not shrink mismatches.
        #[arg(long)]
        no_shrink: bool,
        /// Shrink and save at most this many mismatches.
        #[arg(long, default_value_t = 8)]
        max_report: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Replay a redsim case file in the game and in redsim; print the differing frames.
    RedsimCase {
        case: PathBuf,
        #[arg(long, default_value_t = 3)]
        max: usize,
        /// Write it as a redsim regression fixture (the game's trace as the expectation).
        #[arg(long)]
        fixture: Option<PathBuf>,
        /// One-line description for the fixture.
        #[arg(long, default_value = "")]
        note: String,
    },
    /// Judge designs in redsim (crates/redsim) instead of the game: the judge's checks at the
    /// default positions (+ --extra-offsets), microseconds per tick. With --judge, also run the
    /// real judge and report any verdict disagreement (redsim as a search pre-screen must never
    /// reject a working door).
    RedsimVerify {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 3)]
        cycles: usize,
        /// Also run the real judge (same offsets) and compare.
        #[arg(long)]
        judge: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Differential fuzzing of the redsim piston model (track pistmodel): random piston setups
    /// (7x7x1 flat or 6x6x3) with scripted power changes, every block state, moving block and
    /// queue size compared after every tick against the bridge; failures shrunk and saved.
    Pistfuzz {
        #[arg(long, default_value_t = 100)]
        count: u64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// flat, box or mix.
        #[arg(long, default_value = "mix")]
        mode: String,
        /// Slime and honey in the palette.
        #[arg(long)]
        slime: bool,
        /// Sand, gravel, red sand in the palette.
        #[arg(long)]
        gravity: bool,
        /// Observers in the palette.
        #[arg(long)]
        observers: bool,
        /// Redstone components next to the pistons (dust, torches, repeaters, comparators,
        /// levers, lamps, note blocks, targets), placed like the judge does.
        #[arg(long)]
        mixed: bool,
        /// Shaped blocks among the solids: slabs, trapdoors, scaffolding, fence gates.
        #[arg(long)]
        shapes: bool,
        /// Replay every gadget drive script of designs/gadgets (entity gadgets skipped).
        #[arg(long)]
        gadgets: bool,
        /// Replay door designs: placement, settle, then lever flips (`--cycles`).
        #[arg(long, num_args = 1..)]
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 2)]
        cycles: u32,
        /// What the bridge is compared with: `model` (redsim) or `bridge` (a second run: the
        /// harness's own determinism).
        #[arg(long, default_value = "model")]
        against: String,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Shrink every failure to a minimal scenario.
        #[arg(long)]
        shrink: bool,
        /// Replay scenario files instead of generating.
        #[arg(long, num_args = 1..)]
        replay: Vec<PathBuf>,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/pistmodel/fuzz")]
        out_dir: PathBuf,
        /// Stop after this many failures.
        #[arg(long, default_value_t = 20)]
        max_fail: usize,
        /// Print every tick's block changes of the bridge run (for single replays).
        #[arg(long)]
        trace: bool,
        /// Sculk scenarios (track sculk): sensors, calibrated sensors with frequency filters,
        /// wool, amethyst, comparators and observers on sensors, among pistons and wiring.
        #[arg(long)]
        sculk: bool,
        /// With --sculk: boxes anywhere in the sandbox (its edge chunks included).
        #[arg(long)]
        border: bool,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::Cycle { design, flips, write_state, out } => {
            crate::cycle::cli(&design, flips, write_state, out.as_deref())?;
        }
        DoorCmd::Sheet { files, out, heading, fill, fill_dir, single_up_below } => {
            crate::sheet::cli(&files, out.as_deref(), heading.as_deref(), fill, fill_dir.as_deref(), single_up_below)?;
        }
        DoorCmd::Cosearch { cmd } => crate::cosearch::run(version, cmd)?,
        DoorCmd::RedsimFuzz { n, seed, size, density, palette, out, no_shrink, max_report, workers } => {
            let sz: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            anyhow::ensure!(sz.len() == 3, "--size x,y,z");
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })?;
            let o = redsim_fuzz::FuzzOptions {
                n,
                seed,
                size: [sz[0], sz[1], sz[2]],
                density,
                palette: redsim_fuzz::palette(&palette)?,
                out,
                shrink: !no_shrink,
                max_report,
                generator: "random".into(),
            };
            redsim_fuzz::fuzz(&pool, &o, &|s| eprintln!("{s}"))?;
        }
        DoorCmd::RedsimCase { case, max, fixture, note } => {
            let mut w = Worker::spawn(&cfg)?;
            let same = redsim_fuzz::replay(&mut w, &case, max)?;
            if let Some(path) = fixture {
                anyhow::ensure!(same || !note.is_empty(), "writing a fixture of a mismatching case needs a --note");
                let c = redsim_fuzz::Case::parse(&std::fs::read_to_string(&case)?)?;
                let text = redsim_fuzz::fixture(&mut w, &c, &note)?;
                std::fs::write(&path, text)?;
                eprintln!("fixture written to {}", path.display());
            }
        }
        DoorCmd::RedsimVerify { designs, extra_offsets, cycles, judge, workers } => {
            let mut offsets = VerifyOptions::default().offsets;
            offsets.extend(crate::extra_offsets(extra_offsets));
            let loaded: Vec<(PathBuf, Design)> = designs.iter().map(|p| Ok((p.clone(), load_design(p)?))).collect::<Result<_>>()?;
            let t0 = std::time::Instant::now();
            let screens: Vec<crate::redsim_screen::Screen> = std::thread::scope(|s| {
                let hs: Vec<_> = loaded
                    .chunks(loaded.len().div_ceil(16).max(1))
                    .map(|part| s.spawn(|| part.iter().map(|(_, d)| crate::redsim_screen::screen(d, &offsets, cycles, 400, true)).collect::<Vec<_>>()))
                    .collect();
                hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
            });
            eprintln!("redsim judged {} designs in {:.2}s", loaded.len(), t0.elapsed().as_secs_f64());
            let verdicts: Vec<Option<std::result::Result<(u32, u32), String>>> = if judge {
                let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
                let opts = VerifyOptions { offsets: offsets.clone(), cycles: cycles as u32, ..Default::default() };
                let sbs: Vec<std::sync::Mutex<Option<u32>>> = (0..pool.len()).map(|_| std::sync::Mutex::new(None)).collect();
                pool.map(loaded.iter().map(|(_, d)| d.clone()).collect(), |w, wi, d| {
                    let mut s = sbs[wi].lock().unwrap();
                    let sb = match *s {
                        Some(sb) => sb,
                        None => {
                            let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-4, -4, 3, 3], seed: 0 })?;
                            *s = Some(sb);
                            sb
                        }
                    };
                    Ok(match crate::verify_in(w, sb, &d, &opts) {
                        Ok(r) => Ok((r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0), r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0))),
                        Err(e) => Err(format!("{e:#}")),
                    })
                })
                .into_iter()
                .map(|r| Some(r.unwrap_or_else(|e| Err(format!("error: {e:#}")))))
                .collect()
            } else {
                vec![None; loaded.len()]
            };
            let (mut pass, mut fail, mut unsup, mut disagree) = (0, 0, 0, 0);
            for (((p, _), s), v) in loaded.iter().zip(screens.iter()).zip(verdicts.iter()) {
                let line = match s {
                    crate::redsim_screen::Screen::Pass { open, close } => {
                        pass += 1;
                        format!("PASS open {open} close {close}")
                    }
                    crate::redsim_screen::Screen::Fail(m) => {
                        fail += 1;
                        format!("FAIL {m}")
                    }
                    crate::redsim_screen::Screen::Unsupported(m) => {
                        unsup += 1;
                        format!("UNSUPPORTED {m}")
                    }
                };
                let judge_note = match (s, v) {
                    (_, None) => String::new(),
                    (crate::redsim_screen::Screen::Unsupported(_), Some(_)) => String::new(),
                    (crate::redsim_screen::Screen::Pass { .. }, Some(Ok(_))) | (crate::redsim_screen::Screen::Fail(_), Some(Err(_))) => " [judge agrees]".into(),
                    (_, Some(Ok((o, c)))) => {
                        disagree += 1;
                        format!(" [DISAGREE: judge passes, open {o} close {c}]")
                    }
                    (_, Some(Err(e))) => {
                        disagree += 1;
                        format!(" [DISAGREE: judge fails: {}]", e.chars().take(160).collect::<String>())
                    }
                };
                println!("{}: {}{judge_note}", p.display(), line.chars().take(220).collect::<String>());
            }
            eprintln!("{pass} pass, {fail} fail, {unsup} unsupported{}", if judge { format!(", {disagree} disagreements with the judge") } else { String::new() });
        }
        DoorCmd::Pistfuzz { count, seed, mode, slime, gravity, observers, mixed, shapes, gadgets, designs, cycles, against, workers, shrink, replay, out_dir, max_fail, trace, sculk, border } => {
            let o = crate::pistfuzz::CliOpts { count, seed, mode, slime, gravity, observers, mixed, shapes, against, workers, shrink, replay, gadgets, designs, cycles, out_dir, max_fail, trace, sculk, border };
            crate::pistfuzz::cli(version, &o)?;
        }
    }
    Ok(())
}
