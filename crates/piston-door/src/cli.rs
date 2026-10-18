//! `mctas door ...`

use crate::{DoorSpec, VerifyOptions, timeline, verify};
use anyhow::Result;
use clap::Subcommand;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

/// Local-search knobs shared by `minimize` and `harvest` (see search.rs).
#[derive(clap::Args, Clone)]
pub struct SearchArgs {
    /// Worker JVMs (capped by MCTAS_MAX_WORKERS).
    #[arg(long, default_value_t = 32)]
    pub workers: usize,
    /// Move families: a comma list of singles,pairs,triples,delays,moves,subs,reroute,renet, or all.
    #[arg(long, default_value = "singles")]
    pub moves: String,
    /// Acceptance: the judge's four positions plus this many (as `door verify --extra-offsets`).
    #[arg(long, default_value_t = 28)]
    pub extra_offsets: u32,
    /// Screen candidates at the first N judge positions before judging them.
    #[arg(long, default_value_t = 2)]
    pub screen_offsets: usize,
    /// Open/close cycles per screening position.
    #[arg(long, default_value_t = 1)]
    pub screen_cycles: u32,
    /// Settle cap (ticks) while screening.
    #[arg(long, default_value_t = 200)]
    pub screen_ticks: u32,
    /// Neighborhood radius of compounds (neutral edit + one removal) and triples.
    #[arg(long, default_value_t = 2)]
    pub radius: i32,
    /// When nothing improves, try every verified neutral edit followed by removals and re-routes.
    #[arg(long)]
    pub plateau: bool,
    /// With --plateau, also try pair removals after each verified neutral edit.
    #[arg(long)]
    pub plateau_pairs: bool,
    /// Plateau depth (2 implies --plateau): also a second neutral edit after each first.
    #[arg(long, default_value_t = 1)]
    pub depth: usize,
    /// Random-walk steps over verified neutral designs without improving (implies --plateau).
    #[arg(long, default_value_t = 0)]
    pub walk: usize,
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// What to minimize: blocks, or worst-cycle ticks then blocks: open, close, total (open+close).
    #[arg(long, default_value = "blocks")]
    pub objective: String,
    /// Speed objectives: allow up to this many blocks more than the start design.
    #[arg(long, default_value_t = 0)]
    pub slack: usize,
    /// Screen with the judge's own code (per-tick frames) instead of the faster frame-less copy.
    #[arg(long)]
    pub screen_frames: bool,
    /// Speed boards: only accept designs that open within this many ticks (every cycle).
    #[arg(long)]
    pub max_open: Option<u32>,
    /// Speed boards: only accept designs that close within this many ticks (every cycle).
    #[arg(long)]
    pub max_close: Option<u32>,
}

impl SearchArgs {
    pub fn options(&self) -> Result<crate::search::SearchOptions> {
        let offsets = crate::search::judge_offsets(self.extra_offsets);
        Ok(crate::search::SearchOptions {
            families: crate::search::parse_families(&self.moves)?,
            screen: VerifyOptions {
                cycles: self.screen_cycles,
                max_ticks: self.screen_ticks,
                offsets: offsets.iter().take(self.screen_offsets.max(1)).copied().collect(),
                keep_frames: false,
            },
            judge: VerifyOptions { offsets, ..Default::default() },
            radius: self.radius,
            plateau: self.plateau || self.plateau_pairs || self.walk > 0 || self.depth > 1,
            plateau_pairs: self.plateau_pairs,
            depth: self.depth,
            walk: self.walk,
            deadline: None,
            seed: self.seed,
            objective: crate::search::parse_objective(&self.objective)?,
            slack: self.slack,
            screen_frames: self.screen_frames,
            max_open: self.max_open,
            max_close: self.max_close,
        })
    }

    pub fn pool(&self, version: &str) -> Result<mctas::sim::Pool> {
        mctas::sim::Pool::spawn(self.workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })
    }
}
#[derive(Subcommand)]
pub enum DoorCmd {
    /// Record a video of every design in a folder (several at once), upload each, write VIDEOS.md.
    VideoAll {
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        dir: PathBuf,
        /// Local folder for the videos (existing ones are skipped).
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 4)]
        jobs: usize,
        /// GitHub release tag to upload each new video to.
        #[arg(long)]
        upload: Option<String>,
    },
    /// Verify every design on HEAD and on the explore/* branches and write a ranked table.
    Leaderboard {
        /// Ref prefix to scan besides HEAD.
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        /// Run `git fetch origin` first.
        #[arg(long)]
        fetch: bool,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        /// Skip designs with more blocks than this.
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Write the markdown table here (also printed).
        #[arg(long)]
        out: Option<PathBuf>,
        /// Copy every explore branch's tracks/<track> folder into the working tree first.
        #[arg(long)]
        sync_tracks: bool,
        /// Archive every working design into this folder; designs already there are trusted.
        #[arg(long)]
        archive_dir: Option<PathBuf>,
    },
    /// Route a dust path in a design (prints the blocks to add).
    Route {
        design: PathBuf,
        /// Start cells (existing dust of the net), "x,y,z;x,y,z".
        #[arg(long)]
        start: String,
        /// Goal cell (becomes dust).
        #[arg(long)]
        goal: String,
        /// Extra cells the path may touch, "x,y,z;...".
        #[arg(long, default_value = "")]
        allow: String,
        /// Take the swept cells from this drive script (ideal inputs) instead of flipping the
        /// design's lever: for wiring a mechanism that does not run on its own yet.
        #[arg(long)]
        swept_script: Option<PathBuf>,
        /// Append the path to this file as `door patch` lines (`set x y z <state>`) instead of
        /// printing builder code.
        #[arg(long)]
        patch: Option<PathBuf>,
    },
    /// Drive a design by hand from a script (see drive.rs) and print the z=0 plane per tick.
    /// Several scripts run one after the other; any failed `expect` makes the command fail.
    Drive {
        #[arg(required = true)]
        scripts: Vec<PathBuf>,
        /// Print only one PASS/FAIL line per script (a gadget test run).
        #[arg(long)]
        quiet: bool,
    },
    /// Judge a mechanism driven by ideal inputs from a schedule (see ideal.rs); sweep its timing.
    Ideal {
        schedule: PathBuf,
        /// Override a schedule variable, name=value (repeatable).
        #[arg(long)]
        set: Vec<String>,
        /// Sweep a variable, name=lo..hi (repeatable: every combination).
        #[arg(long)]
        sweep: Vec<String>,
        /// Sweep each --sweep variable on its own (others at their values) instead of the grid.
        #[arg(long)]
        independent: bool,
        /// Extra pseudo-random positions on top of the default four.
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 2)]
        cycles: u32,
        /// Print the plane z,x0,y0,x1,y1 every tick of the first cycle.
        #[arg(long)]
        trace: Option<String>,
        /// Worker JVMs for sweeps.
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Local search for fewer blocks (see search.rs): removals, repeater delays, moves,
    /// substitutions and dust re-routing. Candidates are screened at a few positions and accepted
    /// only if the full judge passes; every improvement is written to --out.
    Minimize {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[command(flatten)]
        search: SearchArgs,
        /// Stop after this many minutes.
        #[arg(long)]
        minutes: Option<f64>,
        /// Archive every improvement (archive/ next to --out).
        #[arg(long)]
        archive: bool,
        /// Rename the design (used for archive file names).
        #[arg(long)]
        name: Option<String>,
    },
    /// Run the local search on the best designs of every branch (see harvest.rs): the `top`
    /// smallest working designs per category not harvested yet, `minutes` each.
    Harvest {
        /// Ref prefix to scan besides HEAD.
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        /// Run `git fetch origin` first.
        #[arg(long)]
        fetch: bool,
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        /// Designs per category.
        #[arg(long, default_value_t = 2)]
        top: usize,
        /// Only designs within this many blocks of their league's best.
        #[arg(long, default_value_t = 8)]
        within: usize,
        /// Search time per design.
        #[arg(long, default_value_t = 15.0)]
        minutes: f64,
        /// Keep going: fetch and harvest again every this many minutes (0 = one pass).
        #[arg(long, default_value_t = 0.0)]
        every: f64,
        /// With --every, stop after this many hours.
        #[arg(long, default_value_t = 4.0)]
        hours: f64,
        /// Results (from-<label>.txt), archive/ and harvested.txt go here.
        #[arg(long)]
        out_dir: PathBuf,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Record the door opening and closing with the real Minecraft client (needs Xvfb).
    Video {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Simulation ticks per second while recording (20 = real time).
        #[arg(long, default_value_t = 10.0)]
        tps: f64,
        /// Upload to this GitHub release tag of vjeux/minecraft-tas.
        #[arg(long)]
        upload: Option<String>,
    },
    /// Print a built-in design (see designs.rs) in the text format.
    Gen {
        name: String,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Apply an edit script to a design and write the (canonicalized) result. Script lines:
    /// `set x y z <state>` (an extended piston gets its head too), `clear x y z` (a piston
    /// takes its head along), `port <name> x y z`; `#` starts a comment line.
    Patch {
        design: PathBuf,
        script: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Place a design, settle, flip the lever and print every block change tick by tick.
    Trace {
        design: PathBuf,
        /// Ticks to record after each flip.
        #[arg(long, default_value_t = 40)]
        ticks: u32,
        /// Also flip the lever back and trace the closing.
        #[arg(long)]
        close: bool,
    },
    /// Learn edits from pairs of designs (a -> b, b a smaller variant of a) and try each on the
    /// best designs of every league (see harvest.rs); improvements are archived and polished.
    Transplant {
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        #[arg(long)]
        fetch: bool,
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        /// Targets per league.
        #[arg(long, default_value_t = 4)]
        top: usize,
        /// Targets within this many blocks of their league's best.
        #[arg(long, default_value_t = 4)]
        within: usize,
        /// Largest patch (cells changed) to learn.
        #[arg(long, default_value_t = 12)]
        max_cells: usize,
        /// Local search after each improvement, minutes.
        #[arg(long, default_value_t = 5.0)]
        minutes: f64,
        #[arg(long)]
        out_dir: PathBuf,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Verify a design many times in ONE reused sandbox (as the search does) and count passes:
    /// shows state that leaks across `clear` (e.g. redstone torch burnout memory).
    ReuseCheck {
        design: PathBuf,
        #[arg(long, default_value_t = 30)]
        times: u32,
        #[arg(long, default_value_t = 1)]
        cycles: u32,
        /// Recreate the sandbox before every run instead of reusing it.
        #[arg(long)]
        fresh: bool,
    },
    /// Check that a design is a working 3x3 piston door; print timings and block count.
    Verify {
        design: PathBuf,
        /// Print the tick-by-tick opening/closing of the 3x3 opening.
        #[arg(long)]
        timeline: bool,
        /// Open/close cycles per position.
        #[arg(long, default_value_t = 3)]
        cycles: u32,
        /// Test at this many extra pseudo-random positions on top of the default four.
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        /// If it works, keep a copy in the archive folder next to the design (archive/<name>-<blocks>b.txt), unless an identical design is already archived.
        #[arg(long)]
        archive: bool,
        /// Archive into this folder instead (e.g. the main designs/piston-door-3x3/archive).
        #[arg(long)]
        archive_dir: Option<PathBuf>,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::VideoAll { dir, out, jobs, upload } => {
            crate::video::record_all(version, &dir, &out, jobs, upload.as_deref())?;
        }
        DoorCmd::Leaderboard { refs, fetch, extra_offsets, max_blocks, workers, out, sync_tracks, archive_dir } => {
            let repo = std::path::Path::new(".");
            if fetch {
                let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
                anyhow::ensure!(st.success(), "git fetch failed");
            }
            if sync_tracks {
                eprintln!("synced tracks: {:?}", crate::leaderboard::sync_tracks(repo, &refs)?);
            }
            let trusted = archive_dir.as_deref().map(crate::leaderboard::archived_keys).unwrap_or_default();
            let cands = crate::leaderboard::collect(repo, &refs, max_blocks)?;
            eprintln!("verifying {} designs ({} archived designs trusted)", cands.len(), trusted.len());
            let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, archive_dir.as_deref())?;
            if let Some(dir) = &archive_dir {
                for e in entries.iter().filter(|e| e.verdict.is_ok()) {
                    let note = format!("verified WORKS by door leaderboard: {} blocks, category {}", e.blocks, e.category);
                    if let Some(p) = crate::archive::save(std::path::Path::new(&e.source), &e.design, e.blocks, &note, Some(dir))? {
                        eprintln!("archived {}", p.display());
                    }
                }
            }
            let table = crate::leaderboard::render(&entries, extra_offsets);
            print!("{table}");
            if let Some(p) = out {
                std::fs::write(p, &table)?;
            }
        }
        DoorCmd::Route { design, start, goal, allow, swept_script, patch } => {
            let d = load_design(&design)?;
            let cells = |s: &str| -> Vec<[i32; 3]> {
                s.split(';').filter(|x| !x.trim().is_empty()).map(|c| {
                    let v: Vec<i32> = c.split(',').map(|n| n.trim().parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                }).collect()
            };
            let spec = crate::DoorSpec::from_design(&d)?;
            let mut w = Worker::spawn(&cfg)?;
            let mut reserved = match &swept_script {
                Some(s) => crate::drive::swept(&mut w, &std::fs::read_to_string(s)?)?,
                None => crate::route::swept_cells(&mut w, &d, &spec)?,
            };
            reserved.retain(|p| !d.blocks.contains_key(p));
            for x in 0..=2 { for y in 0..=2 { for z in -4..=4 { reserved.insert([x, y, z]); } } }
            reserved.insert([1, 3, 1]);
            let r = crate::route::Router { blocks: &d.blocks, reserved, allow: cells(&allow).into_iter().collect(), lo: [-12, -6, -8], hi: [12, 12, 10], floor_only: Default::default() };
            let goal = cells(&goal)[0];
            match r.route(&cells(&start), goal) {
                Some(path) => {
                    if let Some(file) = &patch {
                        use std::io::Write;
                        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file)?;
                        writeln!(f, "# route to {},{},{} ({} new blocks)", goal[0], goal[1], goal[2], path.len())?;
                        for (p, s) in path.iter().rev() {
                            writeln!(f, "set {} {} {} {}", p[0], p[1], p[2], s)?;
                        }
                        println!("{} new blocks appended to {}", path.len(), file.display());
                    } else {
                        for (p, s) in path.iter().rev() {
                            println!("    b.set([{}, {}, {}], \"{}\");", p[0], p[1], p[2], s);
                        }
                        println!("    // {} new blocks", path.len());
                    }
                }
                None => anyhow::bail!("no route"),
            }
        }
        DoorCmd::Ideal { schedule, set, sweep, independent, extra_offsets, cycles, trace, workers } => {
            crate::ideal::cli(version, &schedule, &set, &sweep, independent, extra_offsets, cycles, trace, workers)?;
        }
        DoorCmd::Drive { scripts, quiet } => {
            let mut w = Worker::spawn(&cfg)?;
            let mut failed = 0;
            for script in &scripts {
                let text = std::fs::read_to_string(script)?;
                match crate::drive::run_opts(&mut w, &text, !quiet) {
                    Ok(o) if o.failures.is_empty() => {
                        if quiet || o.checks > 0 {
                            println!("PASS {} ({} checks)", script.display(), o.checks);
                        }
                    }
                    Ok(o) => {
                        failed += 1;
                        println!("FAIL {} ({} of {} checks failed)", script.display(), o.failures.len(), o.checks);
                        for f in &o.failures {
                            println!("  {f}");
                        }
                    }
                    Err(e) => {
                        failed += 1;
                        println!("FAIL {}: {e:#}", script.display());
                    }
                }
            }
            if failed > 0 {
                anyhow::bail!("{failed} of {} scripts failed", scripts.len());
            }
        }
        DoorCmd::Minimize { design, out, search, minutes, archive, name } => {
            let mut d = load_design(&design)?;
            if let Some(n) = name {
                d.name = n;
            }
            let pool = search.pool(version)?;
            let t = std::time::Instant::now();
            let mut opts = search.options()?;
            opts.deadline = minutes.map(|m| t + std::time::Duration::from_secs_f64(m * 60.0));
            let moves = search.moves.clone();
            let judge_n = opts.judge.offsets.len();
            println!("{} workers; moves {moves}; judge {judge_n} positions", pool.len());
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let source = design.display().to_string();
            let mut on_improve = |best: &mctas::design::Design, desc: &str| -> Result<()> {
                best.save(&out)?;
                log(&format!("improved: {} blocks ({desc}), written to {}", crate::block_count(best), out.display()));
                if archive {
                    let note = format!(
                        "verified WORKS: {} blocks (judge: {judge_n} positions x 3 cycles); door minimize --moves {moves} from {source}",
                        crate::block_count(best),
                    );
                    if let Some(p) = crate::archive::save(&out, best, crate::block_count(best), &note, None)? {
                        log(&format!("archived {}", p.display()));
                    }
                }
                Ok(())
            };
            let best = crate::search::search(&pool, &d, &opts, &log, &mut on_improve)?;
            best.save(&out)?;
            println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&best), out.display());
        }
        DoorCmd::Harvest { refs, fetch, max_blocks, top, within, minutes, every, hours, out_dir, search } => {
            let t = std::time::Instant::now();
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let hopts = crate::harvest::HarvestOptions { refs, max_blocks, top, minutes, out_dir, moves: search.moves.clone(), within };
            loop {
                let pass = std::time::Instant::now();
                if fetch || every > 0.0 {
                    let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
                    if !st.success() {
                        log("git fetch failed; harvesting what we have");
                    }
                }
                // A fresh pool per pass: no idle JVMs held between passes.
                let pool = search.pool(version)?;
                crate::harvest::run(&pool, std::path::Path::new("."), &hopts, &search.options()?, &log)?;
                drop(pool);
                if every <= 0.0 || t.elapsed().as_secs_f64() > hours * 3600.0 {
                    break;
                }
                let wait = (every * 60.0 - pass.elapsed().as_secs_f64()).max(0.0);
                log(&format!("pass done; next in {:.1} min", wait / 60.0));
                std::thread::sleep(std::time::Duration::from_secs_f64(wait));
            }
        }
        DoorCmd::Transplant { refs, fetch, max_blocks, top, within, max_cells, minutes, out_dir, search } => {
            if fetch {
                let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
                anyhow::ensure!(st.success(), "git fetch failed");
            }
            let pool = search.pool(version)?;
            let t = std::time::Instant::now();
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let hopts = crate::harvest::HarvestOptions { refs, max_blocks, top, minutes, out_dir, moves: search.moves.clone(), within };
            crate::harvest::transplant(&pool, std::path::Path::new("."), &hopts, &search.options()?, max_cells, &log)?;
        }
        DoorCmd::ReuseCheck { design, times, cycles, fresh } => {
            let d = load_design(&design)?;
            let mut w = Worker::spawn(&cfg)?;
            let opts = VerifyOptions { cycles, offsets: vec![[0, 0, 0]], ..Default::default() };
            let mut sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
            let mut line = String::new();
            let mut passes = 0;
            for _ in 0..times {
                if fresh {
                    w.drop_sandbox(sb)?;
                    sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
                }
                let ok = crate::verify_in(&mut w, sb, &d, &opts).is_ok();
                passes += ok as u32;
                line.push(if ok { '+' } else { '.' });
            }
            println!("{line}\n{passes}/{times} passed in {} sandbox(es)", if fresh { "fresh" } else { "one reused" });
        }
        DoorCmd::Video { design, out, tps, upload } => {
            let d = load_design(&design)?;
            let opts = crate::video::VideoOptions { tps, ..Default::default() };
            crate::video::record(version, &d, &out, &opts)?;
            println!("recorded {}", out.display());
            if let Some(tag) = upload {
                let url = mctas::video::upload_release_asset("vjeux/minecraft-tas", &tag, &out)?;
                println!("uploaded {url}");
            }
        }
        DoorCmd::Patch { design, script, out } => {
            let d0 = load_design(&design)?;
            let text = std::fs::read_to_string(&script)?;
            let d1 = crate::patch::apply(&d0, &text)?;
            let mut w = Worker::spawn(&cfg)?;
            let d = crate::canonicalize(&mut w, &d1)?;
            d.save(&out)?;
            println!("{} -> {} blocks, written to {}", crate::block_count(&d0), crate::block_count(&d), out.display());
        }
        DoorCmd::Gen { name, out } => {
            let d0 = crate::designs::build(&name).ok_or_else(|| anyhow::anyhow!("no design {name}; have {:?}", crate::designs::names()))?;
            let mut w = Worker::spawn(&cfg)?;
            let d = crate::canonicalize(&mut w, &d0)?;
            match out {
                Some(path) => d.save(&path)?,
                None => print!("{}", d.to_text()),
            }
        }
        DoorCmd::Trace { design, ticks, close } => {
            let d = load_design(&design)?;
            let mut w = Worker::spawn(&cfg)?;
            trace(&mut w, &d, ticks, close)?;
        }
        DoorCmd::Verify { design, timeline: show, cycles, archive, extra_offsets, archive_dir } => {
            let d = load_design(&design)?;
            let spec = DoorSpec::from_design(&d)?;
            let mut w = Worker::spawn(&cfg)?;
            let mut opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
            opts.offsets.extend(crate::extra_offsets(extra_offsets));
            match verify(&mut w, &d, &opts) {
                Ok(r) => {
                    println!("WORKS: {} ({} blocks, {}x{}x{}, category {}{})", d.name, r.blocks, r.size[0], r.size[1], r.size[2], crate::category(&d), if crate::slime_free(&d) { ", slime-free" } else { "" });
                    if archive {
                        let note = format!("verified WORKS: {} blocks, open after {} ticks, closed after {} ticks", r.blocks, r.cycles[0].open_ticks, r.cycles[0].close_ticks);
                        match crate::archive::save(&design, &d, r.blocks, &note, archive_dir.as_deref())? {
                            Some(p) => println!("archived {}", p.display()),
                            None => println!("already archived"),
                        }
                    }
                    for (i, c) in r.cycles.iter().enumerate() {
                        println!(
                            "  cycle {i}: open after {} ticks (idle at {}), closed after {} ticks (idle at {})",
                            c.open_ticks, c.open_settle, c.close_ticks, c.close_settle
                        );
                    }
                    for warning in &r.warnings {
                        println!("  warning: {warning}");
                    }
                    if show {
                        println!("opening (top row, middle row, bottom row; # block, ~ moving, h piston head, . air):");
                        print!("{}", timeline(&r.open_frames, &spec));
                        println!("closing:");
                        print!("{}", timeline(&r.close_frames, &spec));
                    }
                }
                Err(e) => {
                    println!("BROKEN: {}: {e:#}", d.name);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Load a design path, or a built-in design name prefixed with `@` (e.g. `@v1`).
pub fn load_design(path: &std::path::Path) -> Result<Design> {
    let s = path.to_string_lossy();
    if let Some(name) = s.strip_prefix('@') {
        return crate::designs::build(name).ok_or_else(|| anyhow::anyhow!("no built-in design {name}"));
    }
    Design::load(path)
}

fn short(s: &str) -> String {
    s.strip_prefix("minecraft:").unwrap_or(s).to_string()
}

pub fn trace(w: &mut Worker, d: &Design, ticks: u32, close: bool) -> Result<()> {
    use mctas::sim::{PlaceMode, SandboxOptions};
    let spec = DoorSpec::from_design(d)?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<_> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<_> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let settle = w.step_until_idle(sb, 400)?;
    println!("settled in {} ticks (idle {:?})", settle.ticked, settle.idle);
    let (mut lo, mut hi) = d.bounds().unwrap();
    for i in 0..3 {
        lo[i] -= 2;
        hi[i] += 2;
    }
    let mut prev = w.get_region(sb, lo, hi)?;
    for (p, s) in &blocks {
        if prev.get(*p) != Some(s.as_str()) {
            println!("  at rest: {p:?} {} -> {}", short(s), short(prev.get(*p).unwrap_or("?")));
        }
    }
    let flips = if close { 2 } else { 1 };
    for flip in 0..flips {
        println!("== lever {} ==", if flip == 0 { "ON" } else { "OFF" });
        w.use_block(sb, spec.lever)?;
        let r = w.step_record(sb, ticks, false, lo, hi)?;
        for (t, f) in r.frames.iter().enumerate() {
            let mut changes = Vec::new();
            for (p, s) in f.iter() {
                let before = prev.get(p).unwrap_or("?");
                if before != s {
                    changes.push(format!("{p:?} {} -> {}", short(before), short(s)));
                }
            }
            if !changes.is_empty() {
                println!("t={:<3} {}", t + 1, changes.join("\n      "));
            }
            prev = f.clone();
        }
    }
    Ok(())
}
