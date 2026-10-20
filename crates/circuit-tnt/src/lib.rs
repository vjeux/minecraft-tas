//! Contraption study 9, track tnt: TNT dupers (docs/circuits.md, the variations and the
//! mechanism in designs/circuits/tnt/VARIATIONS.md). `mctas circuit tnt <command> ...`
//! dispatches here.

pub mod align;
pub mod game;
pub mod improve;
pub mod precise;
pub mod precision;
pub mod search;
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
        /// TILEABLE: run N copies side by side (copy k moved by k x --offset).
        #[arg(long, default_value_t = 1)]
        tile: usize,
        /// The tiling step `dx,dy,dz` (default: the box width along x).
        #[arg(long)]
        offset: Option<String>,
    },
    /// Local search on redsim for a PRECISE duper: adds / removes / replaces aligner blocks in a
    /// box (never the design's own cells), keeping changes that do not shrink the largest group of
    /// identical fuse-1 positions over the kicks.
    Align {
        design: PathBuf,
        /// `x0,y0,z0,x1,y1,z1` (inclusive).
        #[arg(long = "box")]
        bx: String,
        #[arg(long, default_value = "stone,obsidian,slime,minecraft:water[level=0]")]
        palette: String,
        #[arg(long, default_value_t = 20000)]
        iterations: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 220)]
        ticks: u32,
        #[arg(long, default_value_t = 3)]
        nth: usize,
        /// Kicks per evaluation (the default kick list, evenly sampled).
        #[arg(long, default_value_t = 16)]
        kicks: usize,
        #[arg(long)]
        out: PathBuf,
        /// A design whose cells are never touched (default: every cell of the start design).
        #[arg(long)]
        fixed: Option<PathBuf>,
    },
    /// Shrink a PRECISE design on redsim: drop blocks one at a time (never a TNT) while it still
    /// dupes and every kick still ends at one fuse-1 position; writes DIR/<name>-<blocks>b.txt.
    PreciseShrink {
        design: PathBuf,
        #[arg(long, default_value_t = 220)]
        ticks: u32,
        #[arg(long, default_value_t = 3)]
        nth: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// PRECISE on redsim: one run per fixed kick (the game's random one replaced), every TNT an
    /// entity; groups the fuse-1 positions of the Nth TNT of each run.
    PreciseSim {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 220)]
        ticks: u32,
        /// Which TNT of each run (0 = the first; later ones are in the steady state).
        #[arg(long, default_value_t = 3)]
        nth: usize,
        /// Print the first TNT's state every tick for this kick (and nothing else).
        #[arg(long)]
        follow: Option<f64>,
    },
    /// The hard evidence of survival (circuit::robust::tnt_blasts): every explosion on redsim with
    /// no out-of-range removal (each TNT flies on until it explodes), at --kicks kick angles, rays at
    /// their largest roll; once in the void (the judge's sandbox) and once on an obsidian floor one
    /// block under the machine: the explosion centers' distances and the blocks destroyed.
    Blasts {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 64)]
        kicks: usize,
        /// Air blocks between the machine and the ground (default: its safe drop height, from the
        /// screen).
        #[arg(long)]
        drop: Option<i32>,
        /// Run this many ticks instead of the judge's run (a long-run check: craters pile up).
        #[arg(long)]
        ticks: Option<u32>,
    },
    /// The all-randoms rule (circuit::robust): judge the design on redsim with its explosions at
    /// every kick angle (a sweep of --samples values of the kick's nextDouble(), every change
    /// bisected to 1e-12, every piece and boundary judged, rays at their largest roll).
    Robust {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4096)]
        samples: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
    },
    /// PRECISE / direction: follow each primed TNT in the real game from its first sighting to
    /// fuse 1 (killed there, nothing explodes): the spread of position and motion at both moments,
    /// the mean motion, the direction class.
    Precision {
        designs: Vec<PathBuf>,
        /// TNT to follow.
        #[arg(long, default_value_t = 16)]
        n: usize,
        #[arg(long, default_value_t = 3000)]
        max_ticks: u32,
        #[arg(long, default_value_t = 8)]
        margin: i32,
        /// Print every followed TNT (first sighting and fuse 1).
        #[arg(long)]
        list: bool,
    },
    /// Write a design repeated N times side by side (copy k moved by k x --offset), to judge the
    /// TILEABLE variation with `circuit verify`.
    Tile {
        design: PathBuf,
        #[arg(long, default_value_t = 4)]
        n: usize,
        /// `dx,dy,dz`.
        #[arg(long)]
        offset: String,
        #[arg(long)]
        out: PathBuf,
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
    /// Seeded random search for dupers on redsim: the seed design stays, each sample adds k
    /// palette blocks in the box; keeps the ones that dupe (periodic, primes every period, no TNT
    /// block lost). Writes DIR/found.tsv and DIR/designs/.
    Search {
        #[arg(long)]
        seed: PathBuf,
        /// Search box `x0,y0,z0,x1,y1,z1` (inclusive, the seed's coordinates).
        #[arg(long = "box")]
        bx: String,
        /// Comma list: slime, honey, stone, obsidian, tnt, fan, wallfan, dust, rblock, observer,
        /// piston, sticky, or block states.
        #[arg(long, default_value = "slime,stone,obsidian,tnt,fan,wallfan,dust,rblock")]
        palette: String,
        #[arg(long, default_value_t = 6)]
        k: usize,
        #[arg(long, default_value_t = 3)]
        kmin: usize,
        #[arg(long, default_value = "tnt,fan,dust")]
        require: String,
        #[arg(long, default_value_t = 1_000_000)]
        samples: usize,
        #[arg(long, default_value_t = 120)]
        ticks: u32,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        /// Keep only dupers with at least this many TNT per period (2+: bursts from a seed that
        /// already dupes).
        #[arg(long, default_value_t = 1)]
        min_per: usize,
        /// Stop recording after this many distinct dupers.
        #[arg(long, default_value_t = 2000)]
        max_found: usize,
        /// Cells that must stay empty: `x,y,z;x,y,z`.
        #[arg(long, default_value = "")]
        forbid: String,
        #[arg(long)]
        out: PathBuf,
        /// Keep only dupers surviving their own explosions on redsim at this many kicks (0: off).
        #[arg(long, default_value_t = 0)]
        survive: usize,
    },
    /// Shrink dupers on redsim: drop blocks one at a time (never a TNT) while they still dupe at
    /// least as fast; writes each distinct result to DIR (name: tnt-p<P>-x<per>-<blocks>b-...).
    Shrink {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 120)]
        ticks: u32,
        #[arg(long)]
        out: PathBuf,
    },
    /// Local search from working dupers on redsim (delete / move / replace / swap / add moves,
    /// accepted when no bigger and no slower); writes the Pareto front (blocks vs TNT per minute)
    /// to DIR/front/ and DIR/front.tsv.
    Improve {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "slime,stone,obsidian,fan,wallfan,dust,rblock,observer,sticky,piston,honey")]
        palette: String,
        #[arg(long, default_value_t = 2_000_000)]
        iterations: usize,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long, default_value_t = 1)]
        rng: u64,
        #[arg(long, default_value_t = 120)]
        ticks: u32,
        #[arg(long, default_value_t = 2)]
        reach: i32,
        #[arg(long)]
        out: PathBuf,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit tnt".to_string()).chain(args))?;
    match cli.cmd {
        Cmd::Sim { designs, ticks, margin, trace, tile, offset } => {
            for d in &designs {
                let mut c = clock::Clock::load(d)?;
                if tile > 1 {
                    let off: clock::Pos = match &offset {
                        Some(s) => {
                            let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                            anyhow::ensure!(v.len() == 3, "--offset dx,dy,dz");
                            [v[0], v[1], v[2]]
                        }
                        None => {
                            let (lo, hi) = c.design.bounds().ok_or_else(|| anyhow::anyhow!("empty design"))?;
                            [hi[0] - lo[0] + 1, 0, 0]
                        }
                    };
                    c = sim::tiled(&c, tile, off)?;
                    println!("{}: {tile} copies, step {off:?}", d.display());
                }
                let r = sim::run(&c, ticks, margin, trace, false);
                println!("{}: {}", d.display(), r.line());
            }
        }
        Cmd::Align { design, bx, palette, iterations, workers, rng, ticks, nth, kicks, out, fixed } => {
            let v: Vec<i32> = bx.split(',').map(|s| s.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 6, "--box x0,y0,z0,x1,y1,z1");
            let o = align::AlignOptions {
                design,
                fixed,
                lo: [v[0], v[1], v[2]],
                hi: [v[3], v[4], v[5]],
                palette: palette.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                iterations,
                workers,
                rng,
                ticks,
                nth,
                kicks,
                out,
            };
            if let Some(p) = align::align(&o)? {
                println!("{}", p.display());
            }
        }
        Cmd::PreciseShrink { design, ticks, nth, out } => {
            let c = clock::Clock::load(&design)?;
            let n0 = c.design.blocks.len();
            match precise::shrink(&c, &precise::default_kicks(), ticks, nth) {
                None => println!("{}: not PRECISE on redsim (or does not dupe)", design.display()),
                Some(b) => {
                    std::fs::create_dir_all(&out)?;
                    let mut d = c.design.clone();
                    d.name = format!("{}-{}b", c.design.name, b.len());
                    d.blocks = b;
                    let p = out.join(format!("{}.txt", d.name));
                    std::fs::write(&p, d.to_text())?;
                    println!("{}: {n0} -> {} blocks, still PRECISE on redsim: {}", design.display(), d.blocks.len(), p.display());
                }
            }
        }
        Cmd::Blasts { designs, kicks, drop, ticks } => {
            for d in &designs {
                let mut c = circuit::Circuit::load(d)?;
                c.drop = match drop {
                    Some(h) => h,
                    None => circuit::robust::safe_drop_screen(&c, 32).unwrap_or(0),
                };
                println!("{}: {} ({} air blocks under it)", c.name(), circuit::robust::drop_words(c.drop), c.drop);
                for ground in [None, Some(circuit::Ground::Grass), Some(circuit::Ground::Obsidian)] {
                    match circuit::robust::tnt_blasts_for(&c, kicks, ground, ticks) {
                        Ok(r) => println!(
                            "BLASTS {} {}: {} runs, {} explosions, centers {:.1}..{:.1} blocks from the machine, machine blocks destroyed {}, ground blocks destroyed {}{}",
                            c.name(),
                            ground.map(|g| format!("on {}", g.name())).unwrap_or_else(|| "in the void".into()),
                            r.runs,
                            r.blasts,
                            r.min_dist,
                            r.max_dist,
                            r.machine_destroyed,
                            r.floor_destroyed,
                            r.broken.map(|(k, why)| format!("; BROKEN at kick {k:.3}: {why}")).unwrap_or_default()
                        ),
                        Err(e) => println!("FAIL: {} {ground:?}: {e}", c.name()),
                    }
                }
            }
        }
        Cmd::Robust { designs, samples, workers } => {
            for d in &designs {
                let c = circuit::Circuit::load(d)?;
                match circuit::robust::tnt_robust(&c, samples, workers) {
                    Ok(r) => println!("ROBUST: {}: {r}", c.name()),
                    Err(e) => println!("FAIL: {}: {e}", c.name()),
                }
            }
        }
        Cmd::PreciseSim { designs, ticks, nth, follow } => {
            let kicks = precise::default_kicks();
            for d in &designs {
                let c = clock::Clock::load(d)?;
                if let Some(u) = follow {
                    let r = precise::ends_traced(&c, u, ticks, true);
                    println!("{}: kick {u}: {:?}", d.display(), r.map(|v| v.first().copied()));
                    continue;
                }
                let (g, bad) = precise::groups(&c, &kicks, ticks, nth);
                let n: usize = g.iter().map(|x| x.1).sum();
                let verdict = if g.len() == 1 && bad.is_empty() { "PRECISE" } else { "not precise" };
                println!("{}: {verdict} on redsim: {} distinct fuse-1 positions over {n} kicks{}", d.display(), g.len(), if bad.is_empty() { String::new() } else { format!(", {} runs failed ({})", bad.len(), bad[0]) });
                for (p, k) in g.iter().take(6) {
                    println!("  {k} kicks: ({:.6}, {:.6}, {:.6})", p[0], p[1], p[2]);
                }
            }
        }
        Cmd::Precision { designs, n, max_ticks, margin, list } => {
            let mut w = mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig::default())?;
            for d in &designs {
                let c = clock::Clock::load(d)?;
                // a design with an input (a triggered duper): one 4 gt pulse every 40 gt
                let cc = circuit::Circuit::from_design(c.design.clone())?;
                let extra = if cc.inputs.is_empty() {
                    Vec::new()
                } else {
                    let mut s = circuit::stimulus::Schedule::new(cc.inputs.len());
                    for k in 0..(max_ticks / 40) {
                        s.pulse(0, cc.clock.free_from() + 20 + 40 * k, 4);
                    }
                    s.ticks = max_ticks;
                    cc.actions(&s)
                };
                let tr = precision::follow(&mut w, &c, &extra, n, max_ticks, margin)?;
                println!("{}:", d.display());
                for l in precision::report(&tr) {
                    println!("  {l}");
                }
                if list {
                    for t in &tr {
                        let end = t.end.map(|(p, m)| format!("fuse 1 at ({:.4}, {:.4}, {:.4}) motion ({:.4}, {:.4}, {:.4})", p[0], p[1], p[2], m[0], m[1], m[2])).unwrap_or_else(|| "not at fuse 1".into());
                        println!("    gt {}: first at ({:.4}, {:.4}, {:.4}); {end}", t.first_tick, t.first_pos[0], t.first_pos[1], t.first_pos[2]);
                    }
                }
            }
        }
        Cmd::Tile { design, n, offset, out } => {
            let c = clock::Clock::load(&design)?;
            let v: Vec<i32> = offset.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "--offset dx,dy,dz");
            let t = sim::tiled(&c, n, [v[0], v[1], v[2]])?;
            let mut d = t.design.clone();
            d.name = format!("{}-x{n}", c.design.name);
            std::fs::write(&out, d.to_text())?;
            println!("{}: {n} copies of {} ({} blocks)", out.display(), c.design.name, d.blocks.len());
        }
        Cmd::Run { designs, ticks, margin, spawns } => {
            let mut w = mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig::default())?;
            for d in &designs {
                let c = clock::Clock::load(d)?;
                let g = game::run_one(&mut w, &c, ticks, margin)?;
                println!("{}: game: {}", d.display(), g.run.line());
                if spawns {
                    for s in &g.spawns {
                        println!("  gt {}: pos {:?} motion {:?} {}", s.tick, s.pos, s.motion, s.state);
                    }
                }
                let r = sim::run(&c, ticks, margin, 0, true);
                // over the ticks both runs saw; primes by tick, cells within 1 (the game sees a
                // primed TNT one tick after it spawned, possibly already pushed by the machine)
                let n = r.ticks.min(g.run.ticks);
                let ticks_of = |v: &[(u32, redsim::BlockPos)]| v.iter().filter(|(t, _)| *t <= n).map(|(t, _)| *t).collect::<Vec<_>>();
                let near = r.primes.iter().zip(&g.run.primes).all(|((_, a), (_, b))| (a.x - b.x).abs() <= 1 && (a.y - b.y).abs() <= 1 && (a.z - b.z).abs() <= 1);
                let same = ticks_of(&r.primes) == ticks_of(&g.run.primes) && near && r.tnt[..=n as usize] == g.run.tnt[..=n as usize];
                println!("  redsim: {} [{}]", r.line(), if same { format!("prime ticks and TNT counts identical over {n} gt") } else { "DIFFERS".into() });
            }
        }
        Cmd::Search { seed, bx, palette, k, kmin, require, samples, ticks, rng, workers, min_per, max_found, forbid, out, survive } => {
            let mut forbid_cells: Vec<clock::Pos> = Vec::new();
            for c in forbid.split(';').map(str::trim).filter(|c| !c.is_empty()) {
                let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "--forbid x,y,z;x,y,z");
                forbid_cells.push([v[0], v[1], v[2]]);
            }
            let v: Vec<i32> = bx.split(',').map(|s| s.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 6, "--box x0,y0,z0,x1,y1,z1");
            let list = |s: &str| s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect::<Vec<_>>();
            let o = search::SearchOptions {
                seed,
                lo: [v[0], v[1], v[2]],
                hi: [v[3], v[4], v[5]],
                palette: list(&palette),
                k,
                kmin: kmin.min(k),
                require: list(&require),
                samples,
                min_per,
                max_found,
                forbid: forbid_cells,
                ticks,
                rng,
                workers,
                out: out.clone(),
                survive,
            };
            let t = std::time::Instant::now();
            let n = search::search(&o)?;
            println!("{n} distinct dupers in {samples} samples ({:.0} s): {}", t.elapsed().as_secs_f64(), out.join("found.tsv").display());
        }
        Cmd::Shrink { designs, ticks, out } => {
            std::fs::create_dir_all(&out)?;
            let mut seen: std::collections::HashSet<Vec<(clock::Pos, String)>> = std::collections::HashSet::new();
            for d in &designs {
                let c = clock::Clock::load(d)?;
                let n0 = c.design.blocks.len();
                match search::shrink(&c, ticks) {
                    None => println!("{}: does not dupe on redsim", d.display()),
                    Some((blocks, p, per)) => {
                        // the same block set up to translation is one result
                        let lo = blocks.keys().fold([i32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
                        let key: Vec<(clock::Pos, String)> = blocks.iter().map(|(p, s)| ([p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]], s.clone())).collect();
                        let fresh = seen.insert(key);
                        let name = format!("tnt-p{p}-x{per}-{}b-{}", blocks.len(), seen.len());
                        if fresh {
                            let mut dd = c.design.clone();
                            dd.name = name.clone();
                            dd.blocks = blocks.clone();
                            std::fs::write(out.join(format!("{name}.txt")), dd.to_text())?;
                        }
                        println!("{}: {n0} -> {} blocks, period {p}, {per} per period{}", d.display(), blocks.len(), if fresh { format!(" -> {name}") } else { " (seen)".into() });
                    }
                }
            }
        }
        Cmd::Improve { designs, palette, iterations, workers, rng, ticks, reach, out } => {
            let o = improve::ImproveOptions {
                designs,
                palette: palette.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                iterations,
                workers,
                rng,
                ticks,
                out: out.clone(),
                reach,
            };
            let n = improve::improve(&o)?;
            println!("{n} designs on the front: {}", out.join("front.tsv").display());
        }
    }
    Ok(())
}
