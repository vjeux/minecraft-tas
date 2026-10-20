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

pub mod cascade;
pub mod game;
pub mod rs;
pub mod search;
pub mod spec;
pub mod trains;

use anyhow::{Result, anyhow, bail};
use clap::{Parser, Subcommand};
use spec::{Pext, Pos};
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
    /// Every way to add kmin..=k blocks of a palette in a region's free cells of a base design
    /// (redsim screen, then the full judge); keeps the minimal finds (no added block removable),
    /// smallest first. With --out-dir, each find is written as a design.
    Grow {
        base: PathBuf,
        /// Region corner (x,y,z).
        #[arg(long, value_parser = parse_pos)]
        lo: Pos,
        #[arg(long, value_parser = parse_pos)]
        hi: Pos,
        /// Palette (shorthands as in `mctas clock census`, plus `cross`).
        #[arg(long, default_value = "stone,cross,repeater,torch,observer")]
        palette: String,
        #[arg(long, default_value_t = 1)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        #[arg(long, default_value_t = 64)]
        threads: usize,
        /// Only cells in this z plane (flat designs).
        #[arg(long)]
        plane_z: Option<i32>,
        /// Keep non-minimal finds too.
        #[arg(long)]
        all: bool,
        /// Only cells within this Manhattan distance of a base block (1 = face-adjacent).
        #[arg(long)]
        near: Option<i32>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Print at most this many finds.
        #[arg(long, default_value_t = 40)]
        show: usize,
    },
    /// Pulse-toggle trains for a stack of n sticky pistons (abstract 1D model): every pulse
    /// sequence that extends by n on one edge and retracts on the other, with each step's
    /// positions on both edges.
    /// Generate a CASCADE extender of reach N (units + one observer spine of round starts) and
    /// check it on redsim.
    Cascade {
        /// The reach N; with --base/--step, the family index n (N = base + step * n).
        #[arg(long)]
        n: usize,
        #[arg(long, default_value_t = 0)]
        base: usize,
        #[arg(long, default_value_t = 1)]
        step: usize,
        /// Minimum gap between consecutive round starts of one edge (gt).
        #[arg(long, default_value_t = 6)]
        gap: i32,
        /// Write the `extend` headers of this family id (an extendable family base: N = base +
        /// step * n for n in 0..=--n-max, built by this generator).
        #[arg(long)]
        family: Option<String>,
        #[arg(long, default_value_t = 4)]
        n_max: usize,
        /// The `extend group` id (with --family): one catalog card for the residue families.
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Remove archived designs (now covered by a family card) from designs/circuits/pext/archive:
    /// the files and their rows in verdicts.tsv and captions.tsv.
    Drop {
        files: Vec<String>,
        #[arg(long, default_value = "designs/circuits")]
        root: PathBuf,
    },
    Trains {
        #[arg(long, default_value_t = 3)]
        n: usize,
        #[arg(long, default_value_t = 8)]
        max: usize,
        #[arg(long, default_value_t = 5)]
        show: usize,
        /// The frame's displacement on the on-edge (rider mode: 1 = the stack rides a frame that
        /// the fixed piston moved first).
        #[arg(long, default_value_t = 0)]
        shift: i32,
        /// Breadth-first search for the shortest train whose every step static cells can deliver.
        #[arg(long)]
        bfs: bool,
    },
    /// Greedy shrink on redsim: remove blocks, or swap one for a palette state and remove
    /// another, while the extender keeps working; writes each step to --out.
    Minimize {
        design: PathBuf,
        #[arg(long, default_value = "stone,cross,repeater,torch,observer,slime,sticky,piston")]
        palette: String,
        #[arg(long, default_value_t = 64)]
        threads: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

fn parse_pos(s: &str) -> std::result::Result<Pos, String> {
    let v: Vec<i32> = s.split(',').map(|w| w.trim().parse::<i32>().map_err(|e| e.to_string())).collect::<std::result::Result<_, _>>()?;
    if v.len() != 3 {
        return Err(format!("x,y,z expected, got {s:?}"));
    }
    Ok([v[0], v[1], v[2]])
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
        Cmd::Grow { base, lo, hi, palette, k, kmin, threads, plane_z, all, near, out_dir, show } => {
            let o = search::GrowOptions { lo, hi, palette: search::palette(&palette)?, k, kmin, threads, out_dir: out_dir.clone(), plane_z, all, near };
            cmd_grow(&base, &o, show)
        }
        Cmd::Cascade { n, base, step, gap, family, n_max, group, out } => {
            let reach = base + step * n;
            let (mut d, log) = cascade::generate(&cascade::CascadeOptions { n: reach, gap })?;
            if let Some(id) = &family {
                // blocks(N) is linear along N = base + step*n when step is a multiple of 3 (a
                // cell shared by two rounds comes every third N); measured, not assumed
                let count = |k: usize| -> Result<i64> {
                    let d = cascade::generate(&cascade::CascadeOptions { n: base + step * k, gap })?.0;
                    Ok(Pext::from_design(d)?.blocks() as i64)
                };
                let (b0, b1, b2) = (count(0)?, count(1)?, count(2)?);
                if b2 - b1 != b1 - b0 || step % 3 != 0 {
                    bail!("blocks are not linear along N = {base} + {step}n ({b0}, {b1}, {b2}): use a step that is a multiple of 3");
                }
                let m = &mut d.meta;
                m.insert("extend family".into(), id.clone());
                m.insert("extend axis".into(), format!("{base} + {step}n"));
                m.insert("extend blocks".into(), format!("{b0} + {}n", b1 - b0));
                m.insert("extend n".into(), format!("0..{n_max}"));
                m.insert("extend base".into(), format!("{n}"));
                m.insert(
                    "extend rule".into(),
                    format!("every n adds {step} units (slime + sticky piston + carried observer), their round-start pulsers and {} cells of observer spine", 3 * step),
                );
                m.insert(
                    "extend generator".into(),
                    format!("mctas circuit pext cascade --base {base} --step {step} --n {{n}} --out {{out}}"),
                );
                if let Some(g) = &group {
                    m.insert("extend group".into(), g.clone());
                    m.insert("extend group title".into(), "cascade extender (self-triggered units)".into());
                    m.insert("extend group period".into(), "N = any reach >= 2".into());
                    m.insert(
                        "extend group note".into(),
                        "blocks = 12N - 7 - 2 floor((N-1)/3), extend 14N - 6 gt, retract 16N - 8 gt; one family per N mod 3 (every third N two rounds share a pulser)".into(),
                    );
                }
                d.name = format!("{id}-n{n}");
            }
            for l in &log {
                println!("  {l}");
            }
            let p = Pext::from_design(d.clone())?;
            match rs::Prepared::new(&p, 2) {
                Ok(prep) => {
                    let (r, _) = rs::judge(&prep, &rs::Params::default(), false);
                    println!("{}", summary(&p, &r));
                }
                Err(e) => println!("not modeled: {e}"),
            }
            if let Some(out) = out {
                d.save(&out)?;
                println!("wrote {}", out.display());
            }
            Ok(())
        }
        Cmd::Drop { files, root } => {
            let dir = root.join("pext").join("archive");
            for tsv in ["verdicts.tsv", "captions.tsv"] {
                let p = dir.join(tsv);
                let text = std::fs::read_to_string(&p)?;
                let kept: Vec<&str> = text
                    .lines()
                    .filter(|l| {
                        let cols: Vec<&str> = l.split('\t').collect();
                        let file = if tsv == "verdicts.tsv" { cols.get(1) } else { cols.first() };
                        !file.is_some_and(|f| files.iter().any(|x| x == f))
                    })
                    .collect();
                let removed = text.lines().count() - kept.len();
                std::fs::write(&p, kept.join("\n") + "\n")?;
                println!("{tsv}: {removed} rows removed");
            }
            for f in &files {
                let p = dir.join(f);
                if p.exists() {
                    std::fs::remove_file(&p)?;
                    println!("removed {}", p.display());
                }
            }
            Ok(())
        }
        Cmd::Trains { n, max, show, shift, bfs } => {
            if bfs {
                trains::report_bfs(n, shift, max)
            } else {
                trains::report(n, max, show, shift)
            }
        }
        Cmd::Minimize { design, palette, threads, out } => {
            let p = Pext::load(&design)?;
            let steps = search::minimize(&p, &search::MinOptions { palette: search::palette(&palette)?, threads })?;
            for (i, d) in steps.iter().enumerate() {
                let np = Pext::from_design(d.clone())?;
                let prep = rs::Prepared::new(&np, 4).map_err(|e| anyhow!("{e}"))?;
                let (r, _) = rs::judge(&prep, &rs::Params::default(), false);
                println!("step {}: {}", i + 1, summary(&np, &r));
            }
            match (steps.last(), out) {
                (Some(d), Some(path)) => {
                    let mut d = d.clone();
                    d.name = format!("{}-min", p.name());
                    d.save(&path)?;
                    println!("wrote {}", path.display());
                }
                (None, _) => println!("no smaller design found"),
                _ => {}
            }
            Ok(())
        }
    }
}

fn cmd_grow(base: &PathBuf, o: &search::GrowOptions, show: usize) -> Result<()> {
    let p = Pext::base(mctas::design::Design::load(base)?)?;
    let t0 = std::time::Instant::now();
    let finds = search::grow(&p, o, &|k, d, n| eprintln!("  k={k}: {d}/{n} units ({:.0} s)", t0.elapsed().as_secs_f64()))?;
    println!("{} minimal finds in {:.1} s", finds.len(), t0.elapsed().as_secs_f64());
    if let Some(dir) = &o.out_dir {
        std::fs::create_dir_all(dir)?;
    }
    for (i, f) in finds.iter().enumerate() {
        let name = format!("{}-g{}", p.name(), i + 1);
        let d = search::find_design(&p, f, &name);
        if let Some(dir) = &o.out_dir {
            d.save(&dir.join(format!("{name}.txt")))?;
        }
        if i < show {
            let c = f.report.first();
            let added: Vec<String> = f.added.iter().map(|(q, s)| format!("{},{},{} {}", q[0], q[1], q[2], clock::short(s))).collect();
            println!("{name}: {} blocks, extend {} retract {}: {}", f.blocks, c.extend, c.retract, added.join("; "));
        }
    }
    Ok(())
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
