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

pub mod anneal;
pub mod cascade;
pub mod game;
pub mod rs;
pub mod search;
pub mod spec;
pub mod stack;
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
        /// Also the real-use audit's lever flips (rs::flips): `flips safe` or the first flip
        /// that breaks it.
        #[arg(long)]
        flips: bool,
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
        /// Finds must pass the real-use audit's lever flips too (rs::flips).
        #[arg(long)]
        flip_safe: bool,
    },
    /// N = 2 topology sweep, 1 wide (z = 0): the lever on A's back at (0,1,0), A (sticky, east) at
    /// (1,1,0); the second sticky piston B (east) at every (bx, by) of the window with the payload
    /// east of it; `grow` adds k blocks around each base (the region spans the base and 2 cells
    /// around it). Answers "is there an N = 2 1-wide extender in 3 + k blocks" over topologies.
    Topo {
        #[arg(long, default_value_t = 3)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        /// B's x range `a..b` and y range.
        #[arg(long, default_value = "1..3")]
        bx: String,
        #[arg(long, default_value = "0..3")]
        by: String,
        #[arg(long, default_value = "stone,slime,honey,rblock,observer,piston,sticky,dust,repeater1,torch")]
        palette: String,
        #[arg(long, default_value_t = 64)]
        threads: usize,
        #[arg(long, default_value_t = 2)]
        near: i32,
        /// No lever in the base: the lever (wall, floor or ceiling, in the plane) is one of the k
        /// grown blocks (not counted), so every lever placement is searched.
        #[arg(long)]
        free_lever: bool,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Finds must pass the real-use audit's lever flips too (rs::flips).
        #[arg(long)]
        flip_safe: bool,
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
        /// The FLAT layout (everything in the line's plane).
        #[arg(long)]
        flat: bool,
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
    /// Generate a SLIME-FREE plain-stack extender from an interleaved static schedule (see
    /// crates/circuit-pext/src/stack.rs) and check it on redsim.
    Stack {
        #[arg(long)]
        n: usize,
        #[arg(long, default_value_t = 12)]
        max_train: usize,
        #[arg(long, default_value_t = 3)]
        hops: usize,
        #[arg(long, default_value_t = 0)]
        pick: usize,
        /// Use the canonical train T_N instead of enumerating trains.
        #[arg(long)]
        canonical: bool,
        /// Use the shortest single-cell schedule (any N, fewest pulses).
        #[arg(long)]
        cells: bool,
        /// Everything in the line's plane.
        #[arg(long)]
        flat: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Add an observer chain to a design: every cell of --path (`x,y,z;x,y,z;...`) gets an observer
    /// watching the previous cell (the first one watches --from), so a change at --from runs down
    /// the path 2 gt per cell.
    Spine {
        base: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long)]
        path: String,
        #[arg(long)]
        out: PathBuf,
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
        /// Interleaved delivery: the two edges' steps at different times (see trains::interleave).
        #[arg(long)]
        interleave: bool,
        /// The shortest single-cell static schedule (see trains::cell_schedule).
        #[arg(long)]
        cells: bool,
    },
    /// Greedy shrink on redsim: remove blocks, or swap one for a palette state and remove
    /// another, while the extender keeps working; writes each step to --out.
    Minimize {
        design: PathBuf,
        #[arg(long, default_value = "stone,cross,repeater,torch,observer,slime,sticky,piston")]
        palette: String,
        #[arg(long, default_value_t = 64)]
        threads: usize,
        /// Only place new blocks in this z plane (keeps a flat design flat).
        #[arg(long, allow_hyphen_values = true)]
        plane_z: Option<i32>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Simulated annealing for the smallest BOX (volume first, then blocks), judged on redsim: the
    /// volume is part of the energy (1000 per cell), so with `--slack` the box may grow for a
    /// while and then shrink. Edits: delete, re-state, add next to a block, move one cell, swap.
    Anneal {
        design: PathBuf,
        #[arg(long, default_value_t = 2000)]
        steps: usize,
        #[arg(long, default_value_t = 200)]
        batch: usize,
        #[arg(long, default_value_t = 40)]
        threads: usize,
        #[arg(long, default_value = "stone,slime,rblock,observer,sticky,piston,repeater1")]
        palette: String,
        /// Keep a flat design flat (new blocks only in this z plane).
        #[arg(long, allow_hyphen_values = true)]
        plane_z: Option<i32>,
        #[arg(long, default_value_t = 1)]
        slack: i32,
        #[arg(long, default_value_t = 3000.0)]
        temp: f64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 3)]
        moves: usize,
        /// The lever may move to a free neighbouring cell.
        #[arg(long)]
        free_lever: bool,
        /// When no candidate improves, take a random passing one instead of the batch's best
        /// (lets the box grow before it shrinks).
        #[arg(long)]
        random_pick: bool,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Every candidate must pass the real-use audit's lever flips too (rs::flips).
        #[arg(long)]
        flip_safe: bool,
    },
    /// Large-neighborhood shrink: remove every `--remove`-subset of the design's blocks (all but
    /// the payload, the input and the `--keep` cells) and grow back at most `--add` (default one
    /// fewer) blocks of the palette near what is left (`grow`): every find is a smaller design.
    Lns {
        design: PathBuf,
        #[arg(long, default_value_t = 3)]
        remove: usize,
        #[arg(long)]
        add: Option<usize>,
        #[arg(long, default_value = "stone,slime,rblock,observer,sticky,piston,repeater1")]
        palette: String,
        #[arg(long, default_value_t = 64)]
        threads: usize,
        #[arg(long, allow_hyphen_values = true)]
        plane_z: Option<i32>,
        #[arg(long, default_value_t = 2)]
        near: i32,
        /// Cells never removed, `x,y,z;...`.
        #[arg(long)]
        keep: Option<String>,
        /// Layers never removed (every block whose y is listed, e.g. `0,1` for a cascade's line and
        /// its unit observers).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        keep_y: Vec<i32>,
        /// The input lever may move: it is removable like any block, and a subset that removes it
        /// grows it back anywhere (lever states join the palette; the lever is never counted).
        #[arg(long)]
        free_lever: bool,
        /// Start again from each pass's best smaller find until a pass finds nothing smaller
        /// (each best saved as `best-<blocks>b-pass<n>.txt` in --out-dir).
        #[arg(long)]
        repeat: bool,
        /// Grow back only within this Manhattan distance of the removed cells (classic large-
        /// neighborhood search: much faster on long designs).
        #[arg(long)]
        local: Option<i32>,
        /// Region where blocks grow back (default: the design's box grown by 2).
        #[arg(long, value_parser = parse_pos, allow_hyphen_values = true)]
        lo: Option<Pos>,
        #[arg(long, value_parser = parse_pos, allow_hyphen_values = true)]
        hi: Option<Pos>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
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
        Cmd::Sim { designs, cycles, verbose, flips } => cmd_sim(&designs, cycles, verbose, flips),
        Cmd::Trace { design } => cmd_trace(&design),
        Cmd::Verify { designs, positions, no_rotations, cycles, workers, verbose } => {
            let o = game::Options { positions, rotations: !no_rotations, cycles, workers, ..Default::default() };
            cmd_verify(&designs, &o, verbose)
        }
        Cmd::Grow { base, lo, hi, palette, k, kmin, threads, plane_z, all, near, out_dir, show, flip_safe } => {
            rs::require_flips(flip_safe);
            let o = search::GrowOptions { lo, hi, palette: search::palette(&palette)?, k, kmin, threads, out_dir: out_dir.clone(), plane_z, all, near, focus: None };
            cmd_grow(&base, &o, show)
        }
        Cmd::Topo { k, kmin, bx, by, palette, threads, near, free_lever, out_dir, flip_safe } => {
            rs::require_flips(flip_safe);
            cmd_topo(k, kmin, &bx, &by, &palette, threads, near, free_lever, out_dir.as_deref())
        }
        Cmd::Anneal { design, steps, batch, threads, palette, plane_z, slack, temp, seed, moves, free_lever, random_pick, out_dir, flip_safe } => {
            rs::require_flips(flip_safe);
            let o = anneal::AnnealOptions { steps, batch, threads, palette: search::palette(&palette)?, plane_z, slack, temp, seed, moves, free_lever, random_pick, out_dir };
            anneal::anneal(&design, &o).map(|_| ())
        }
        Cmd::Lns { design, remove, add, palette, threads, plane_z, near, keep, keep_y, free_lever, repeat, local, lo, hi, out_dir } => {
            let o = LnsOptions { r: remove, add: add.unwrap_or(remove.saturating_sub(1)), palette, threads, plane_z, near, keep, keep_y, free_lever, repeat, local, region: (lo, hi), out_dir };
            cmd_lns(&design, &o)
        }
        Cmd::Cascade { n, base, step, gap, flat, family, n_max, group, out } => {
            let reach = base + step * n;
            let (mut d, log) = cascade::generate(&cascade::CascadeOptions { n: reach, gap, flat })?;
            if let Some(id) = &family {
                // blocks(N) is linear along N = base + step*n when step is a multiple of 3 (a
                // cell shared by two rounds comes every third N); measured, not assumed
                let count = |k: usize| -> Result<i64> {
                    let d = cascade::generate(&cascade::CascadeOptions { n: base + step * k, gap, flat })?.0;
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
                    format!("mctas circuit pext cascade{} --base {base} --step {step} --n {{n}} --out {{out}}", if flat { " --flat" } else { "" }),
                );
                if let Some(g) = &group {
                    m.insert("extend group".into(), g.clone());
                    m.insert(
                        "extend group title".into(),
                        if flat { "flat cascade extender (self-triggered units, 1 wide)" } else { "cascade extender (self-triggered units)" }.into(),
                    );
                    m.insert("extend group period".into(), "N = any reach >= 2".into());
                    m.insert(
                        "extend group note".into(),
                        if flat {
                            "blocks = 10N - 5 - floor((N-1)/3), extend 14N - 6 gt, retract 16N - 8 gt; one family per N mod 3 (every third N two rounds share a pulser)"
                        } else {
                            "blocks = 12N - 7 - 2 floor((N-1)/3), extend 14N - 6 gt, retract 16N - 8 gt; one family per N mod 3 (every third N two rounds share a pulser)"
                        }
                        .into(),
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
        Cmd::Stack { n, max_train, hops, pick, canonical, cells, flat, out } => {
            let (d, log) = stack::generate(&stack::StackOptions { n, max_train, hops, pick, canonical, cells, flat })?;
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
        Cmd::Spine { base, from, path, out } => {
            let mut d = mctas::design::Design::load(&base)?;
            let parse = |s: &str| -> Result<Pos> {
                let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                if v.len() != 3 {
                    bail!("bad cell {s:?}");
                }
                Ok([v[0], v[1], v[2]])
            };
            let mut prev = parse(&from)?;
            for (i, c) in path.split(';').filter(|s| !s.trim().is_empty()).enumerate() {
                let p = parse(c)?;
                let delta = [prev[0] - p[0], prev[1] - p[1], prev[2] - p[2]];
                let facing = match delta {
                    [1, 0, 0] => "east",
                    [-1, 0, 0] => "west",
                    [0, 1, 0] => "up",
                    [0, -1, 0] => "down",
                    [0, 0, 1] => "south",
                    [0, 0, -1] => "north",
                    _ => bail!("path cell {} {p:?} is not next to {prev:?}", i + 1),
                };
                if let Some(old) = d.blocks.insert(p, format!("minecraft:observer[facing={facing}]")) {
                    bail!("path cell {} {p:?} is taken by {old}", i + 1);
                }
                prev = p;
            }
            d.save(&out)?;
            println!("wrote {} ({} blocks)", out.display(), d.blocks.len());
            Ok(())
        }
        Cmd::Trains { n, max, show, shift, bfs, interleave, cells } => {
            if cells {
                trains::report_cells(n, max)
            } else if interleave {
                trains::report_interleave(n, max, show)
            } else if bfs {
                trains::report_bfs(n, shift, max)
            } else {
                trains::report(n, max, show, shift)
            }
        }
        Cmd::Minimize { design, palette, threads, plane_z, out } => {
            let p = Pext::load(&design)?;
            let steps = search::minimize(&p, &search::MinOptions { palette: search::palette(&palette)?, threads, plane_z, checkpoint: out.clone() })?;
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

/// `pext lns` settings (see `Cmd::Lns`).
struct LnsOptions {
    r: usize,
    add: usize,
    palette: String,
    threads: usize,
    plane_z: Option<i32>,
    near: i32,
    keep: Option<String>,
    keep_y: Vec<i32>,
    free_lever: bool,
    repeat: bool,
    local: Option<i32>,
    region: (Option<Pos>, Option<Pos>),
    out_dir: Option<PathBuf>,
}

/// `pext lns`: remove r blocks, grow back at most `add` (see `Cmd::Lns`).
fn cmd_lns(path: &std::path::Path, o: &LnsOptions) -> Result<()> {
    let mut p = Pext::load(path)?;
    for pass in 1.. {
        let Some(d) = lns_pass(&p, o, pass)? else { break };
        if !o.repeat {
            break;
        }
        // go on from the pass's best (fewest blocks, then fastest), saved beside the finds
        let next = Pext::from_design(d)?;
        if let Some(dir) = o.out_dir.as_deref() {
            std::fs::create_dir_all(dir)?;
            next.design.save(&dir.join(format!("best-{}b-pass{pass}.txt", next.blocks())))?;
        }
        println!("pass {pass}: going on from {} blocks", next.blocks());
        p = next;
    }
    Ok(())
}

/// One `pext lns` pass; the best smaller find (renamed), if any.
fn lns_pass(p: &Pext, o: &LnsOptions, pass: usize) -> Result<Option<mctas::design::Design>> {
    let (r, add, plane_z, region) = (o.r, o.add, o.plane_z, o.region);
    let out_dir = o.out_dir.as_deref();
    let keep: Vec<Pos> = match &o.keep {
        Some(s) => s.split(';').map(|c| parse_pos(c).map_err(|e| anyhow!("{e}"))).collect::<Result<_>>()?,
        None => Vec::new(),
    };
    let removable: Vec<Pos> =
        p.design.blocks.keys().copied().filter(|q| (o.free_lever || *q != p.input) && *q != p.payload && !keep.contains(q) && !o.keep_y.contains(&q[1])).collect();
    let mut pal = search::palette(&o.palette)?;
    if plane_z.is_some() {
        pal.retain(|s| !s.contains("facing=north") && !s.contains("facing=south"));
    }
    // a subset that removes the lever grows it back from these (in-plane when flat)
    let mut levers = search::palette("minecraft:lever[face=wall,facing=east],minecraft:lever[face=wall,facing=west],minecraft:lever[face=floor,facing=north],minecraft:lever[face=ceiling,facing=north]")?;
    if plane_z.is_none() {
        levers.extend(search::palette("minecraft:lever[face=wall,facing=north],minecraft:lever[face=wall,facing=south]")?);
    }
    let pal_lever: Vec<String> = pal.iter().cloned().chain(levers).collect();
    let (threads, near) = (o.threads, o.near);
    let (lo, hi) = p.design.bounds().unwrap_or(([0; 3], [0; 3]));
    let lo = region.0.unwrap_or([lo[0] - 2, lo[1] - 2, plane_z.unwrap_or(lo[2] - 2)]);
    let hi = region.1.unwrap_or([hi[0] + 2, hi[1] + 2, plane_z.unwrap_or(hi[2] + 2)]);
    let subsets = search::combos(removable.len(), r);
    println!("{}: {} blocks, {} removable, {} subsets of {r}, growing back <= {add}", p.name(), p.blocks(), removable.len(), subsets.len());
    let t0 = std::time::Instant::now();
    // one subset per worker, each grow single-threaded: spawning a pool per subset costs more
    // than the small grows themselves
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done = std::sync::atomic::AtomicUsize::new(0);
    let found: std::sync::Mutex<Vec<(usize, search::Find, Pext)>> = std::sync::Mutex::new(Vec::new());
    let failed: std::sync::Mutex<Option<anyhow::Error>> = std::sync::Mutex::new(None);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let si = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if si >= subsets.len() {
                    break;
                }
                let mut d = p.design.clone();
                let mut lever_out = false;
                for &i in &subsets[si] {
                    d.blocks.remove(&removable[i]);
                    if removable[i] == p.input {
                        lever_out = true;
                        d.ports.remove("input");
                        d.meta.remove("port input");
                    }
                }
                d.name = format!("{}-lns{si}", p.name());
                if let Ok(base) = Pext::base(d) {
                    let palette = if lever_out { pal_lever.clone() } else { pal.clone() };
                    // --local: grow back only near the removed cells (the lever's old cell included)
                    let focus = o.local.map(|ld| (subsets[si].iter().map(|&i| removable[i]).collect::<Vec<Pos>>(), ld));
                    let go = search::GrowOptions { lo, hi, palette, k: add, kmin: 1, threads: 1, out_dir: None, plane_z, all: false, near: Some(near), focus };
                    match search::grow(&base, &go, &|_, _, _| {}) {
                        Ok(finds) => {
                            // each find on disk at once: a long pass that is cut short keeps them
                            if let Some(dir) = out_dir {
                                for (j, f) in finds.iter().enumerate() {
                                    let name = format!("{}-{}b-p{pass}-s{si}-{}", p.name(), f.blocks, j + 1);
                                    let saved = std::fs::create_dir_all(dir).map_err(anyhow::Error::from).and_then(|_| search::find_design(&base, f, &name).save(&dir.join(format!("{name}.txt"))));
                                    if let Err(e) = saved {
                                        *failed.lock().unwrap() = Some(e);
                                    }
                                }
                            }
                            if let Some(f) = finds.first() {
                                eprintln!("  find: subset {si}, {} blocks, extend {} retract {}", f.blocks, f.report.first().extend, f.report.first().retract);
                            }
                            found.lock().unwrap().extend(finds.into_iter().map(|f| (si, f, Pext::base(base.design.clone()).expect("reparse"))))
                        }
                        Err(e) => *failed.lock().unwrap() = Some(e),
                    }
                }
                let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if n % 50 == 0 {
                    eprintln!("  {n}/{} subsets ({:.0} s)", subsets.len(), t0.elapsed().as_secs_f64());
                }
            });
        }
    });
    if let Some(e) = failed.into_inner().unwrap() {
        return Err(e);
    }
    let mut found = found.into_inner().unwrap();
    found.sort_by_key(|(si, f, _)| (f.blocks, f.report.first().extend + f.report.first().retract, *si));
    let mut best = p.blocks();
    for (si, f, _) in found.iter() {
        let c = f.report.first();
        let removed: Vec<String> = subsets[*si].iter().map(|&j| format!("{},{},{}", removable[j][0], removable[j][1], removable[j][2])).collect();
        let added: Vec<String> = f.added.iter().map(|(q, s)| format!("{},{},{} {}", q[0], q[1], q[2], clock::short(s))).collect();
        println!("  subset {si} (-{}): {} blocks, extend {} retract {}: +{}", removed.join(" "), f.blocks, c.extend, c.retract, added.join("; "));
        best = best.min(f.blocks);
    }
    println!("best: {best} blocks ({:.0} s)", t0.elapsed().as_secs_f64());
    Ok(found.first().filter(|(_, f, _)| f.blocks < p.blocks()).map(|(_, f, base)| {
        let name = p.name().split("-lns").next().unwrap_or(p.name()).to_string();
        search::find_design(base, f, &name)
    }))
}

/// `pext topo`: the N = 2 1-wide topology sweep (see `Cmd::Topo`).
#[allow(clippy::too_many_arguments)]
fn cmd_topo(k: usize, kmin: usize, bx: &str, by: &str, palette: &str, threads: usize, near: i32, free_lever: bool, out_dir: Option<&std::path::Path>) -> Result<()> {
    let range = |s: &str| -> Result<(i32, i32)> {
        let (a, b) = s.split_once("..").ok_or_else(|| anyhow!("want a..b, got {s:?}"))?;
        Ok((a.trim().parse()?, b.trim().parse()?))
    };
    let (bx0, bx1) = range(bx)?;
    let (by0, by1) = range(by)?;
    // the palette, flat: in-plane facings only (an out-of-plane observer or piston acts on air)
    let mut pal: Vec<String> = search::palette(palette)?.into_iter().filter(|s| !s.contains("facing=north") && !s.contains("facing=south")).collect();
    if free_lever {
        pal.extend(search::palette("minecraft:lever[face=wall,facing=east],minecraft:lever[face=wall,facing=west],minecraft:lever[face=floor,facing=north],minecraft:lever[face=ceiling,facing=north]")?);
    }
    let t0 = std::time::Instant::now();
    let mut all: Vec<(String, usize, u32, u32)> = Vec::new();
    for y in by0..=by1 {
        for x in bx0..=bx1 {
            // B and the payload must not overlap the lever (0,1) or A (1,1)
            if [(x, y), (x + 1, y)].iter().any(|c| *c == (0, 1) || *c == (1, 1)) {
                continue;
            }
            let mut d = mctas::design::Design { name: format!("topo2-b{x}-{y}"), ..Default::default() };
            d.meta.insert("circuit".into(), "piston-extender 2".into());
            d.meta.insert("travel".into(), "1 0 0".into());
            if !free_lever {
                d.meta.insert("port input".into(), "lever".into());
                d.ports.insert("input".into(), [0, 1, 0]);
                d.blocks.insert([0, 1, 0], "minecraft:lever[face=wall,facing=west]".into());
            }
            d.ports.insert("payload".into(), [x + 1, y, 0]);
            d.blocks.insert([1, 1, 0], "minecraft:sticky_piston[facing=east]".into());
            d.blocks.insert([x, y, 0], "minecraft:sticky_piston[facing=east]".into());
            d.blocks.insert([x + 1, y, 0], "minecraft:gold_block".into());
            let p = Pext::base(d)?;
            let lo = [-1, y.min(1) - 2, 0];
            let hi = [x + 4, y.max(1) + 2, 0];
            let o = search::GrowOptions { lo, hi, palette: pal.clone(), k, kmin, threads, out_dir: None, plane_z: Some(0), all: false, near: Some(near), focus: None };
            let finds = search::grow(&p, &o, &|_, _, _| {})?;
            println!("B at ({x},{y}): {} minimal finds ({:.0} s)", finds.len(), t0.elapsed().as_secs_f64());
            for (i, f) in finds.iter().enumerate() {
                let name = format!("{}-g{}", p.name(), i + 1);
                let c = f.report.first();
                let added: Vec<String> = f.added.iter().map(|(q, s)| format!("{},{},{} {}", q[0], q[1], q[2], clock::short(s))).collect();
                println!("  {name}: {} blocks, extend {} retract {}: {}", f.blocks, c.extend, c.retract, added.join("; "));
                if let Some(dir) = out_dir {
                    std::fs::create_dir_all(dir)?;
                    search::find_design(&p, f, &name).save(&dir.join(format!("{name}.txt")))?;
                }
                all.push((name, f.blocks, c.extend, c.retract));
            }
        }
    }
    all.sort_by_key(|(_, b, e, r)| (*b, *e + *r));
    println!("{} designs over the sweep; best: {}", all.len(), all.first().map(|(n, b, e, r)| format!("{n} {b} blocks {e}/{r} gt")).unwrap_or_else(|| "none".into()));
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

fn cmd_sim(designs: &[PathBuf], cycles: usize, verbose: bool, flips: bool) -> Result<()> {
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
        if flips && r.works() {
            println!("  flips {}", rs::flips(&prep, &params).unwrap_or_else(|| "safe".to_string()));
        }
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
