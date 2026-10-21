//! `mctas circuit ...`: the contraption studies (docs/circuits.md). Clap definitions and dispatch
//! only; the judge is crates/circuit, each track's own commands are its crate
//! (`mctas circuit <track> <args...>` -> `circuit_<track>::run`).

pub mod shrink;

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
    /// The exact gate of a redsim model (any kind): the kind's stimulus on the base build in redsim
    /// AND in the game, every cell of the watched box compared at every tick. Prints `IDENTICAL:
    /// <name>, <frames> frames` or the first divergence.
    TraceCompare {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
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
        /// Print the cells that change, frame by frame, over `<from>..<to>` (the waveform's frame
        /// numbers), with the inputs and output levels: how a design works, tick by tick.
        #[arg(long)]
        cells: Option<String>,
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
    /// Greedy block removal judged in the real game: each round tries removing every block and
    /// keeps the first removal whose verdict still passes with the SAME metrics. Ends with the
    /// smaller design written beside the original (or `--out`).
    Shrink {
        design: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Also try removing the blocks at the port cells (an input's receiving block, an output lamp cell).
        #[arg(long)]
        free_ports: bool,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// MINIMUM VOLUME for any kind (circuit::compact, track clk-rt): box-bounded annealing from a
    /// seed on the kind's own redsim judge; a candidate passes with the seed's kind and axis value,
    /// ranked by the judge's box then blocks; the front goes to DIR/front/, `--game N` judges the
    /// smallest that beat the seed in the real game (robust, same axis). `--all` (no seed): every
    /// study's minimum-volume robust design per (kind, axis) from the archive, DIR/sweep.tsv.
    Compact {
        /// One seed (a verbose run into --out), or several (one run each, DIR/sweep.tsv).
        seeds: Vec<PathBuf>,
        #[arg(long)]
        all: bool,
        /// Collect a finished (or running) sweep's game-OK designs into --out/<study>/ with
        /// captions from their seeds' (then `circuit archive` them).
        #[arg(long)]
        collect: Option<PathBuf>,
        /// With --all: these studies (default: every study but tnt, which has `circuit tnt compact`,
        /// and shulker).
        #[arg(long, value_delimiter = ',')]
        studies: Vec<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// The region `x0,y0,z0,x1,y1,z1` (default: the seed's box).
        #[arg(long)]
        region: Option<String>,
        /// Palette block states (default: the seed's own states plus stone), `;`-separated.
        #[arg(long)]
        palette: Option<String>,
        #[arg(long, default_value_t = 4000)]
        iterations: usize,
        #[arg(long, default_value_t = 24)]
        workers: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 1500)]
        stale: usize,
        #[arg(long, default_value_t = 40)]
        repair: usize,
        #[arg(long, default_value_t = 2)]
        game: usize,
        /// Block ids never deleted or replaced, comma-separated (e.g. `hopper,dropper,comparator`:
        /// a hopper ring's hop count sets a precise drop's burst point).
        #[arg(long, value_delimiter = ',')]
        keep: Vec<String>,
        #[arg(long)]
        out: PathBuf,
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
        /// With --rejudge or --fill-families: only these studies (default: all).
        #[arg(long)]
        study: Vec<String>,
        /// Judge in the game every member of each archived extendable family that extend.tsv
        /// does not list yet (every n of its range, up to 16 members), and add the passing ones.
        #[arg(long)]
        fill_families: bool,
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
    /// Replace archived designs' captions from a TSV file, one `<study>/<file><TAB><caption>` per
    /// line (the site shows a caption's first sentence as the plain explanation, the rest under
    /// details); `#` lines are comments. No game.
    Captions {
        file: PathBuf,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
    },
    /// R7 comparison (redsim, no game): every archived design with a plain input run with the
    /// judge's old block placement and with real input devices (the judge's stone powered by its
    /// repeater); lists the ones passing only by the block placement, refused or changed.
    R7 {
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// Only these studies (default: all).
        #[arg(long)]
        study: Vec<String>,
    },
    /// Set an archived design's caption (captions.tsv of its study's archive; no game): for the
    /// plain-language pass (one sentence a non-redstoner follows first, the detail after it).
    Caption {
        /// The study directory name (designs/circuits/<study>).
        study: String,
        /// The archived file (`tnt-launch-14b.txt` or without `.txt`).
        file: Option<String>,
        #[arg(long)]
        text: Option<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
        /// Set every `file<TAB>caption` row of this file instead (after a merge took the other
        /// side's captions.tsv, for instance).
        #[arg(long)]
        from: Option<PathBuf>,
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
    /// Track arith's commands (crates/circuit-arith, study 12: adders, subtractors, incrementers,
    /// comparators).
    Arith {
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
    /// Track timer's commands (crates/circuit-timer): study 11's monostables, off-delays and
    /// on-delays.
    Timer {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Study 15's 7-segment decoders (crates/circuit-display): design generators.
    Display {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Study 14: the 3D interleave search. Dial 0 fixed, dial 1 at every offset (dx, dy, dz) and
    /// mirror, cells two dials share must be stone in both; every candidate judged on redsim
    /// (lock 2), the passing ones ranked by blocks then box; the best pattern repeated for 3 dials.
    LockSearch {
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Study 14: write the `lock N` family's member (lock3-alt's pattern for N dials,
    /// circuit::lock::alt_design): two comparators per dial, the frames on the west wall. With
    /// `--hidden`, the hidden-code family (circuit::lock::hidden_design): the code frames on the
    /// back face, facing away from the player.
    LockGen {
        #[arg(long)]
        n: u32,
        #[arg(long)]
        hidden: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Study 13, item sorters: ONE run of a sorter's base scenario (or a badge's tiled run) in the
    /// game at rotation 0, its table printed (what reached each chest) and the outcome: a design
    /// aid; `mctas circuit verify` is the judge.
    SorterRun {
        design: PathBuf,
        /// `tileable` or `overflow`: that badge's tiled run instead of the base scenario.
        #[arg(long)]
        badge: Option<String>,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// On redsim (milliseconds) instead of the game.
        #[arg(long)]
        sim: bool,
    },
    /// Study 13: a local search for smaller sorters on redsim (circuit::sorter::census): every
    /// design with up to --remove of the base's blocks removed (hoppers and comparators stay) and
    /// fewer palette blocks added in the air cells of --cells; prints those passing the base, tiled
    /// and overflow runs and writes the smallest to --out.
    SorterCensus {
        base: PathBuf,
        #[arg(long, default_value_t = 2)]
        remove: usize,
        /// The box the added blocks go in: `x,y,z..x,y,z`.
        #[arg(long)]
        cells: String,
        /// Block states, comma-separated (`stone,glass,wire,torch,wtorch-n,wtorch-s,rep-n,rep-s`
        /// are shorthands).
        #[arg(long, default_value = "stone,wire,torch,wtorch-n,wtorch-s,rep-n,rep-s")]
        palette: String,
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// The local census for ANY kind (circuit::local): every design with up to --remove of the
    /// base's blocks removed (none whose state contains a --keep word) and fewer palette blocks
    /// added in the air cells of --cells, judged on redsim: a sorter by its runs, any other kind by
    /// its checker with the base's main value (its first metric: L, N, the function...). Prints the
    /// passing designs and writes the smallest to --out; the game judge comes after.
    LocalCensus {
        base: PathBuf,
        #[arg(long, default_value_t = 2)]
        remove: usize,
        /// The box the added blocks go in: `x,y,z..x,y,z`, or `grow:N` (the base's bounds grown by
        /// N, the default `grow:1`).
        #[arg(long, default_value = "grow:1")]
        cells: String,
        /// Block states, comma-separated (shorthands: stone, glass, wire, torch, wtorch-n/s/e/w,
        /// rep-n/s/e/w, obs-n/s/e/w/u/d).
        #[arg(long, default_value = "stone,wire,torch,wtorch-n,wtorch-s,rep-n,rep-s")]
        palette: String,
        /// Blocks never removed: words of their states, comma-separated.
        #[arg(long, default_value = "")]
        keep: String,
        /// Metrics a candidate must share with the base besides its main value
        /// (comma-separated: `delay` keeps an instant design instant).
        #[arg(long, default_value = "")]
        same: String,
        /// Look for another main value than the base's (the first metric's value as the checker
        /// prints it, e.g. `3 gt`): the base only seeds the search.
        #[arg(long)]
        value: Option<String>,
        #[arg(long, default_value_t = 0)]
        threads: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn info(c: &Circuit) -> String {
    let mut out = format!("{}: {}, {} blocks", c.name(), c.kind.as_ref().map(|k| format!("{} (study {})", k.name(), k.study())).unwrap_or_else(|| "no kind".into()), c.blocks());
    for i in &c.inputs {
        let how = match &i.how {
            circuit::InputHow::Block { on, off } => format!("{} / {}", clock::short(on), clock::short(off)),
            circuit::InputHow::Driven { dir, through } => format!(
                "driven: {}, powered by the judge's repeater {dir:?} from it{}",
                c.clock.design.blocks.get(&i.pos).map(|s| format!("the design's {}", clock::short(s))).unwrap_or_else(|| "the judge's stone".into()),
                if *through { " (fed through the build)" } else { "" }
            ),
            circuit::InputHow::Lever => "lever".into(),
            circuit::InputHow::Button => "button".into(),
            circuit::InputHow::Strength { .. } => "strength (a chest the judge fills)".into(),
            circuit::InputHow::Frame { rotation } => format!("item frame at rotation {rotation} (the judge turns it, one right-click per step)"),
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
        CircuitCmd::TraceCompare { designs, margin } => {
            let o = Options { margin, version: version.to_string(), ..Default::default() };
            for p in &designs {
                let c = Circuit::load(p)?;
                match circuit::verify::trace_compare(&c, &o)? {
                    Ok(frames) => println!("IDENTICAL: {}, {frames} frames (every cell of the watched box, redsim = the game)", c.name()),
                    Err(why) => println!("DIVERGES: {}: {why}", c.name()),
                }
            }
        }
        CircuitCmd::Sim { designs, stimulus, gap, wave, cells, table, margin } => {
            let o = Options { stimulus, gap, margin, version: version.to_string(), ..Default::default() };
            for p in &designs {
                // a design the judge refuses to load (R7: no way in for its input repeater) is
                // reported and the others still run
                let c = match Circuit::load(p) {
                    Ok(c) => c,
                    Err(e) => {
                        println!("REFUSED: {}: {:#}", p.display(), e);
                        continue;
                    }
                };
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
                if let (Some(range), Ok(t)) = (&cells, &r.trace) {
                    print_cell_changes(t, range);
                }
                println!("{}", r.summary(&c));
            }
        }
        CircuitCmd::Compact { seeds, all, collect, studies, root, region, palette, iterations, workers, rng, stale, repair, game, keep, out } => {
            if let Some(sweep) = collect {
                for p in circuit::compact::collect(&root, &sweep, &out)? {
                    println!("{}", p.display());
                }
                return Ok(());
            }
            let region = match region {
                Some(s) => {
                    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 6, "--region x0,y0,z0,x1,y1,z1");
                    Some(([v[0], v[1], v[2]], [v[3], v[4], v[5]]))
                }
                None => None,
            };
            let palette: Vec<String> = palette.map(|p| p.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default();
            let o = circuit::compact::Options { region, palette, iterations, workers, rng, stale, repair, game, version: version.to_string(), keep };
            if all {
                let studies: Vec<String> = if studies.is_empty() { circuit::archive::STUDIES.iter().map(|(s, _)| s.to_string()).filter(|s| s != "tnt" && s != "shulker").collect() } else { studies };
                circuit::compact::compact_all(&root, &studies, &o, &out)?;
            } else if seeds.len() > 1 {
                circuit::compact::compact_list(&seeds, &o, &out)?;
            } else {
                let seed = seeds.first().cloned().ok_or_else(|| anyhow::anyhow!("a seed design, or --all"))?;
                let r = circuit::compact::compact_one(&seed, &o, &out, true)?;
                println!("{}: {} {}: seed volume {}, {} blocks", seed.display(), r.kind, r.axis, r.seed_volume, r.seed_blocks);
                for (v, b, f, g) in &r.best {
                    println!("  {v} volume, {b} blocks: {} {}", f.display(), g.chars().take(220).collect::<String>());
                }
                if r.best.is_empty() {
                    println!("  no smaller design");
                }
            }
        }
        CircuitCmd::Shrink { design, out, free_ports, positions, no_rotations, workers, margin } => {
            let o = Options { positions, rotations: !no_rotations, workers, stimulus: None, gap: None, margin, version: version.to_string() };
            let out = out.unwrap_or_else(|| {
                let stem = design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                design.with_file_name(format!("{stem}-shrunk.txt"))
            });
            shrink::shrink(&design, &out, !free_ports, &o)?;
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
        CircuitCmd::Archive { designs, caption, root, positions, workers, dedupe, rejudge, study, fill_families, remove } => {
            for spec in &remove {
                println!("{}", circuit::archive::unarchive(&root, spec)?);
            }
            if !remove.is_empty() && designs.is_empty() && !rejudge && !dedupe && !fill_families {
                return Ok(());
            }
            if dedupe {
                for l in circuit::archive::dedupe(&root)? {
                    println!("{l}");
                }
                if designs.is_empty() && !rejudge && !fill_families {
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
            if fill_families {
                for l in circuit::archive::fill_families(&pool, &root, &o, &study)? {
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
        CircuitCmd::R7 { root, study } => {
            let o = Options { version: version.to_string(), ..Default::default() };
            for l in circuit::archive::r7_compare(&root, &study, &o)? {
                println!("{l}");
            }
        }
        CircuitCmd::Captions { file, root } => {
            for l in circuit::archive::set_captions(&root, &std::fs::read_to_string(&file)?)? {
                println!("{l}");
            }
        }
        CircuitCmd::Caption { study, file, text, root, from } => {
            // the rows to set: one from the flags, or every `file<TAB>caption` row of --from
            let rows: Vec<(String, String)> = match &from {
                Some(f) => std::fs::read_to_string(f)?.lines().filter_map(|l| l.split_once('\t')).filter(|(f, _)| *f != "file").map(|(f, c)| (f.to_string(), c.to_string())).collect(),
                None => vec![(file.clone().unwrap_or_default(), text.clone().unwrap_or_default())],
            };
            let dir = root.join(&study).join("archive");
            let path = dir.join("captions.tsv");
            for (file, text) in rows {
                let file = if file.ends_with(".txt") { file } else { format!("{file}.txt") };
                anyhow::ensure!(dir.join(&file).exists(), "{} is not archived", dir.join(&file).display());
                anyhow::ensure!(!text.is_empty() && !text.contains('\t') && !text.contains('\n'), "a caption is one non-empty line without tabs");
                let old = std::fs::read_to_string(&path).unwrap_or_else(|_| "file\tcaption\n".into());
                let mut out = String::new();
                let mut done = false;
                for (i, line) in old.lines().enumerate() {
                    if i > 0 && line.split('\t').next() == Some(file.as_str()) {
                        out.push_str(&format!("{file}\t{text}\n"));
                        done = true;
                    } else {
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                if !done {
                    out.push_str(&format!("{file}\t{text}\n"));
                }
                std::fs::write(&path, out)?;
                println!("{}: caption of {file} set", path.display());
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
        CircuitCmd::Arith { args } => circuit_arith::run(args)?,
        CircuitCmd::Fly { args } => circuit_fly::run(args)?,
        CircuitCmd::Shulker { args } => circuit_shulker::run(args)?,
        CircuitCmd::Tnt { args } => circuit_tnt::run(args)?,
        CircuitCmd::Pulsemult { args } => circuit_pulsemult::run(args)?,
        CircuitCmd::Timer { args } => circuit_timer::run(args)?,
        CircuitCmd::Display { args } => circuit_display::run(args)?,
        CircuitCmd::SorterCensus { base, remove, cells, palette, threads, out } => census_cmd(&base, remove, &cells, &palette, "minecraft:hopper,minecraft:comparator", "", None, threads, out.as_deref())?,
        CircuitCmd::LocalCensus { base, remove, cells, palette, keep, same, value, threads, out } => census_cmd(&base, remove, &cells, &palette, &keep, &same, value.as_deref(), threads, out.as_deref())?,
        CircuitCmd::LockSearch { out_dir } => {
            std::fs::create_dir_all(&out_dir)?;
            let o = Options { version: version.to_string(), ..Default::default() };
            let judge = |text: &str| -> bool {
                let Ok(d) = mctas::design::Design::parse(text) else { return false };
                let Ok(c) = Circuit::from_design(d) else { return false };
                circuit::verify::sim(&c, &o).ok().and_then(|r| r.outcome).is_some_and(|out| out.pass)
            };
            let mut found: Vec<(usize, i64, (i32, i32, i32, bool), String)> = Vec::new();
            let mut tried = 0;
            // the known 2-dial pattern (lock2-alt) must pass: a check of the search itself
            if let Some((text, _, _)) = circuit::lock::placed_design(&[(0, 3, 0, false), (0, 2, 2, false)]) {
                std::fs::write(out_dir.join("lock2-known.txt"), &text)?;
                let why = match mctas::design::Design::parse(&text).map_err(|e| format!("{e:#}")).and_then(|d| Circuit::from_design(d).map_err(|e| format!("{e:#}"))) {
                    Err(e) => e,
                    Ok(c) => match circuit::verify::sim(&c, &o) {
                        Err(e) => format!("{e:#}"),
                        Ok(r) => r.summary(&c),
                    },
                };
                println!("lock-search: the known lock2-alt pattern: {why}");
            }
            for dx in -3..=3 {
                for dy in -2..=2 {
                    for dz in -3..=3 {
                        for m in [false, true] {
                            let pl = [(0, 3, 0, false), (dx, 3 + dy, dz, m)];
                            let Some((text, blocks, vol)) = circuit::lock::placed_design(&pl) else { continue };
                            tried += 1;
                            if judge(&text) {
                                found.push((blocks, vol, (dx, dy, dz, m), text));
                            }
                        }
                    }
                }
            }
            found.sort_by_key(|f| (f.0, f.1));
            println!("lock-search: {tried} 2-dial placements screened, {} pass", found.len());
            for f in found.iter().take(8) {
                println!("  2 dials: {} blocks, box {}: dial 1 at dx {} dy {} dz {} mirror {}", f.0, f.1, f.2 .0, f.2 .1, f.2 .2, f.2 .3);
            }
            // each passing 2-dial pattern repeated: dial 2 = dial 0 moved by twice the offset
            // (same mirror as dial 0), or by the offset from dial 1 (dial 1's mirror)
            let mut best3: Vec<(usize, i64, String, String)> = Vec::new();
            for f in &found {
                let (dx, dy, dz, m) = f.2;
                for third in [(2 * dx, 3 + 2 * dy, 2 * dz, false), (2 * dx, 3 + 2 * dy, 2 * dz, m), (2 * dx, 3, 2 * dz, false)] {
                    let pl = [(0, 3, 0, false), (dx, 3 + dy, dz, m), third];
                    let Some((text, blocks, vol)) = circuit::lock::placed_design(&pl) else { continue };
                    if judge(&text) {
                        best3.push((blocks, vol, format!("{pl:?}"), text));
                    }
                }
            }
            best3.sort_by_key(|b| (b.0, b.1));
            println!("lock-search: {} 3-dial repeats pass", best3.len());
            for (k, b) in best3.iter().take(5).enumerate() {
                println!("  3 dials: {} blocks, box {}: {}", b.0, b.1, b.2);
                std::fs::write(out_dir.join(format!("lock3-search-{k}.txt")), &b.3)?;
            }
        }
        CircuitCmd::LockGen { n, hidden, out } => {
            if n == 0 || n > 6 {
                anyhow::bail!("lock-gen: 1 to 6 dials");
            }
            std::fs::write(&out, if hidden { circuit::lock::hidden_design(n) } else { circuit::lock::alt_design(n) })?;
            println!("wrote {} (lock {n})", out.display());
        }
        CircuitCmd::SorterRun { design, badge, margin, sim } => {
            let c = Circuit::load(&design)?;
            let out = if sim {
                circuit::sorter::sim_one(&c, badge.as_deref(), margin)?
            } else {
                let pool = mctas::sim::Pool::spawn(1, &mctas::sim::WorkerConfig { version: version.to_string(), ..Default::default() })?;
                circuit::sorter::run_one(&pool, &c, badge.as_deref(), margin)?
            };
            for l in &out.table {
                println!("  {l}");
            }
            if out.pass {
                let m: Vec<String> = out.metrics.iter().map(|(k, v)| format!("{k} {v}")).collect();
                println!("SORTER: {} {}{}: {}", c.name(), c.kind.as_ref().map(|k| k.name()).unwrap_or_default(), badge.map(|b| format!(" ({b} run)")).unwrap_or_default(), m.join(", "));
            } else {
                println!("FAIL: {}: {}", c.name(), out.why);
            }
        }
    }
    Ok(())
}

/// `circuit sim --cells <from>..<to>`: per frame, the cells that changed since the frame before
/// (`[x, y, z] block`, air when a block left), with every input (0/1) and output level.
fn print_cell_changes(t: &circuit::trace::Trace, range: &str) {
    let n = t.cells.len();
    let (a, b) = match range.split_once("..") {
        Some((a, b)) => (a.trim().parse::<usize>().unwrap_or(0), b.trim().parse::<usize>().unwrap_or(n)),
        None => (0, n),
    };
    let short = |s: &str| s.strip_prefix("minecraft:").unwrap_or(s).to_string();
    for f in a.max(1)..b.min(n) {
        let prev: std::collections::HashMap<_, u32> = t.cells[f - 1].iter().copied().collect();
        let cur: std::collections::HashMap<_, u32> = t.cells[f].iter().copied().collect();
        let mut ch: Vec<String> = cur.iter().filter(|(p, id)| prev.get(*p) != Some(*id)).map(|(p, id)| format!("{p:?} {}", short(&t.names[*id as usize]))).collect();
        ch.extend(prev.keys().filter(|p| !cur.contains_key(*p)).map(|p| format!("{p:?} air")));
        ch.sort();
        let ins: String = t.inputs.iter().map(|v| if v.get(f).copied().unwrap_or(false) { '1' } else { '0' }).collect();
        let outs: Vec<String> = t.levels.iter().map(|v| v.get(f).copied().unwrap_or(0).to_string()).collect();
        if !ch.is_empty() || f == a.max(1) {
            println!("  frame {f} in {ins} out {}: {}", outs.join(","), ch.join("; "));
        }
    }
}

/// A palette shorthand's block state (the local census): stone, glass, wire, torch, wtorch-<d>,
/// rep-<d> (delay 1), obs-<d> (d = n, s, e, w; observers also u, d); anything else is a state.
fn palette_state(s: &str) -> String {
    let dir = |c: &str| match c {
        "n" => "north",
        "s" => "south",
        "e" => "east",
        "w" => "west",
        "u" => "up",
        _ => "down",
    };
    match s.trim() {
        "stone" => "minecraft:stone".into(),
        "glass" => "minecraft:glass".into(),
        "wire" => "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into(),
        "torch" => "minecraft:redstone_torch[lit=true]".into(),
        t if t.starts_with("wtorch-") => format!("minecraft:redstone_wall_torch[facing={},lit=true]", dir(&t[7..])),
        t if t.starts_with("rep-") => format!("minecraft:repeater[delay=1,facing={},locked=false,powered=false]", dir(&t[4..])),
        t if t.starts_with("obs-") => format!("minecraft:observer[facing={},powered=false]", dir(&t[4..])),
        other => other.to_string(),
    }
}

/// `circuit local-census` / `sorter-census`: the census, its passing designs, the smallest
/// written to `out`.
#[allow(clippy::too_many_arguments)]
fn census_cmd(base: &std::path::Path, remove: usize, cells: &str, palette: &str, keep: &str, same: &str, value: Option<&str>, threads: usize, out: Option<&std::path::Path>) -> Result<()> {
    let c = Circuit::load(base)?;
    let pos = |s: &str| -> Result<[i32; 3]> {
        let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "a cell is x,y,z");
        Ok([v[0], v[1], v[2]])
    };
    let box_ = if let Some(n) = cells.strip_prefix("grow:") {
        let n: i32 = n.parse()?;
        let (lo, hi) = c.bounds();
        ([lo[0] - n, lo[1] - n, lo[2] - n], [hi[0] + n, hi[1] + n, hi[2] + n])
    } else {
        let (a, b) = cells.split_once("..").ok_or_else(|| anyhow::anyhow!("--cells x,y,z..x,y,z or grow:N"))?;
        (pos(a)?, pos(b)?)
    };
    let states: Vec<String> = palette.split(',').map(palette_state).collect();
    let keep: Vec<String> = keep.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let threads = if threads > 0 { threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    let found = if matches!(c.kind, Some(circuit::Kind::Sorter(_))) {
        circuit::sorter::census(&c, remove, box_, &states, threads)?
    } else {
        let same: Vec<String> = same.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        let judge = circuit::local::trace_judge_value(&c, &same, value)?;
        circuit::local::census(&c, remove, box_, &states, &keep, threads, &judge)
    };
    println!("local census: {} designs pass (base {} blocks)", found.len(), c.blocks());
    if let (Some(path), Some((n, d))) = (out, found.first()) {
        let mut d = d.clone();
        d.meta.insert("name".into(), format!("{}-{n}", c.name()));
        std::fs::write(path, d.to_text())?;
        println!("wrote {} ({n} blocks)", path.display());
    }
    Ok(())
}
