//! `mctas door` commands: the judge and the boards: verify, blocks, properties, props-dyn, eligibility, relever, leaderboard, frontier, shapes, tiles, catalog, turn, trace, patch, gen, audit.
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
    /// Write the archive catalog (archive/README.md): every archived design with its size (the
    /// volume) and blocks placed, boards, open/close ticks, rotations, buildability (`door
    /// buildable`'s scripts), video link and how it works (archive/captions.tsv,
    /// archive/records.tsv). Records nothing; verdicts come from the verdict cache, sizes from the
    /// properties cache. `door video-all` writes it too.
    Catalog {
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        archive: PathBuf,
        /// A local folder of rendered videos (`<stem>.mp4`, video-all's --out): linked too.
        #[arg(long)]
        videos: Option<PathBuf>,
        /// GitHub release whose `<stem>.mp4` assets are linked ("" = none).
        #[arg(long, default_value = "videos")]
        release: String,
        /// Worker JVMs for archived designs missing from the verdict cache.
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// The Pareto frontier over (open ticks, close ticks, size: the volume): every working design
    /// on HEAD and the explore/* branches (verdicts from the leaderboard cache, uncached ones
    /// verified), then each --check design verified with the judge and placed against it: on the
    /// frontier, or the doors that dominate it (see leaderboard.rs `frontier_report`).
    Frontier {
        /// Candidate designs to place against the frontier.
        #[arg(long, num_args = 1..)]
        check: Vec<PathBuf>,
        /// Ref prefix to scan besides HEAD.
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        #[arg(long, default_value_t = 4)]
        workers: usize,
        /// Verdict cache folder (read; new verdicts are written back).
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        cache_dir: PathBuf,
        /// All categories (default: solid only, the main Pareto board).
        #[arg(long)]
        any_category: bool,
        /// Also list every working design with its ticks, not just the frontier.
        #[arg(long)]
        all: bool,
    },
    /// The shape of every working design relative to its door: layers in front of and behind the
    /// door plane (thickness), rows below and above the opening, columns left and right of it,
    /// bounding box; then for each shape rule (wall, wall2, flat, ground, no-above, narrow1,
    /// narrow2, wall-ground, plus --rule) the best rotation-safe design today per league, and the
    /// (volume, blocks) frontier. Verdicts come from the leaderboard cache (uncached designs are
    /// verified with the judge).
    Shapes {
        /// Design files to measure besides the folder.
        designs: Vec<PathBuf>,
        /// Folder of designs to measure (default: the archive).
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        dir: PathBuf,
        /// Also measure every design on HEAD and the explore/* branches (as the leaderboard does).
        #[arg(long)]
        branches: bool,
        /// Ref prefix scanned with --branches besides HEAD.
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        #[arg(long, default_value_t = 4)]
        workers: usize,
        /// Verdict cache folder (read; new verdicts are written back).
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        cache_dir: PathBuf,
        /// Extra shape rules: a preset or a comma list of key<=n (front behind thick below above
        /// left right side volume).
        #[arg(long)]
        rule: Vec<String>,
        /// The table lists designs of at most this many blocks (the rules still see all).
        #[arg(long, default_value_t = 200)]
        table_max: usize,
        /// Skip the table (rules and frontier only).
        #[arg(long)]
        no_table: bool,
    },
    /// Verify every design on HEAD and on the explore/* branches and write the boards
    /// (LEADERBOARD.md: each board's leader with its GIF, video and caption, the rest collapsed).
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
        /// Verdict cache folder (default: --archive-dir; none = verify everything).
        #[arg(long)]
        cache_dir: Option<PathBuf>,
        /// Rank only designs whose verdict is cached (simulate nothing).
        #[arg(long)]
        cached_only: bool,
        /// GitHub release with the GIFs, videos and montages to show ("" = none).
        #[arg(long, default_value = "videos")]
        release: String,
        /// Archive folder whose captions.tsv / records.tsv describe the leaders.
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        captions: PathBuf,
    },
    /// What our rules, checks, generators and wiring tools silently exclude (audit.rs, track
    /// audit): on working doors (default: the community 7x9, tracks/c79/c79-closed.txt), (1) every
    /// check that rejects or mislabels it, (2) its mechanism extracted from a redsim cycle and its
    /// membership in each generator's space (families' own generator restricted to its pistons,
    /// every bound relaxed in turn) with the bounds its real moves break, (3) the blocks and
    /// mechanics each wiring tool cannot represent, (4) every hard-coded rule (file:line, live).
    Audit {
        /// Design files or folders (default: tracks/c79/c79-closed.txt).
        designs: Vec<PathBuf>,
        /// Comma list of checks, mech, wiring, rules (or all).
        #[arg(long, default_value = "all")]
        sections: String,
        /// Also run the JVM checks (the real judge, props, props-dyn, eligible) in section 1.
        #[arg(long)]
        jvm: bool,
        /// Write the markdown here instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Write every extracted flat mechanism with a macro solution as a families catalog TSV.
        #[arg(long)]
        tsv: Option<PathBuf>,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// State cap of the macro solve.
        #[arg(long, default_value_t = 200_000)]
        cap: usize,
        /// Sources scanned by the rules section.
        #[arg(long, default_value = "crates/piston-door/src")]
        src: PathBuf,
        /// Section 2: skip designs with no flat mechanism.
        #[arg(long)]
        only_flat: bool,
        /// Print one design's extracted mechanism (paths, moves, providers, membership) instead.
        #[arg(long)]
        explain: bool,
    },
    /// Write a design turned and/or mirrored (the game's own block-state rotate/mirror). A turn
    /// of 90 or 270 degrees gives an east/west-facing door (`door-axis: z` in the file), which
    /// every tool judges in that orientation: the way to build a variant for that direction.
    Turn {
        design: PathBuf,
        /// Quarter turns clockwise seen from above.
        #[arg(long, default_value_t = 1)]
        turns: u8,
        /// Mirror image (east-west) before turning.
        #[arg(long)]
        mirror: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Print a built-in design (see designs.rs) in the text format.
    Gen {
        name: String,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Patch {
        design: PathBuf,
        script: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// List a design's blocks with their coordinates, one `x y z state` line each (the same
    /// coordinates `door patch` and `door drive` use), optionally only those inside a box.
    Blocks {
        design: PathBuf,
        /// Only blocks inside this box, "x0,y0,z0:x1,y1,z1" (inclusive).
        #[arg(long)]
        only: Option<String>,
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
    /// Make working doors record-eligible for repeater input (relever.rs): the lever's cell becomes
    /// a note block (or --subs dropper,dispenser,lamp,bulb) with the lever hung on each of its free
    /// faces. Every variant (and the design as it is) is judged like a record (positions, cycles,
    /// rotation check) and must pass the --eligible checks (repeater input at every position,
    /// hallway, footsteps for sculk, QUART SEAMLESS+, lever place). Prints the passing variants
    /// per design and writes them to --out-dir.
    Relever {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "note")]
        subs: String,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 16)]
        workers: usize,
        /// Skip designs this verdict cache (verdicts-v5.tsv) marks broken.
        #[arg(long)]
        verdicts: Option<PathBuf>,
        #[arg(long, default_value_t = 400)]
        max_blocks: usize,
        /// Also accept variants that are not rotation-safe.
        #[arg(long)]
        any_rotation: bool,
        /// Write the result table (markdown) here too.
        #[arg(long)]
        table: Option<PathBuf>,
        /// Also print why each failing variant fails.
        #[arg(long)]
        why: bool,
        /// Also every design on HEAD and on the origin/explore/* branches (as the leaderboard
        /// collects them, at most --max-blocks).
        #[arg(long)]
        branches: bool,
        /// Only designs at least this seamless as they are (`full`, `semi`, ...: screened first),
        /// and every eligible variant must meet `<tier>[+hall]` too.
        #[arg(long)]
        seamless: Option<String>,
    },
    /// Record eligibility of designs, one line each (what `--eligible` searches keep): the judge
    /// (the lever, every judge position) and the rotation check, repeater input at every judge
    /// position (props-dyn's rig: the lever left off, a delay-1 repeater into its support block,
    /// both edges; worst open/close ticks after the input), the hallway check, footsteps (sculk
    /// designs only) and the SEAMLESS tier (props).
    Eligible {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Dynamic properties of working designs (properties_dyn.rs): hallway-compatible, contained
    /// (shells of air, smooth stone, slime, honey, obsidian, a mix), snow-proof (+ flammable
    /// blocks on top), repeater input (open/close ticks), footsteps, 20 cycles. One position,
    /// one orientation. Writes a TSV (joins properties.rs on the design hash) and prints a table.
    PropertiesDyn {
        /// Design files (always run, cached verdict or not).
        designs: Vec<PathBuf>,
        /// Folder of designs to add (the working ones per the verdict cache).
        #[arg(long)]
        archive_dir: Option<PathBuf>,
        /// Also every design on HEAD and on the explore/* branches (as the leaderboard collects
        /// them), the working ones per the verdict cache.
        #[arg(long)]
        branches: bool,
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        /// The leaderboard's verdict cache.
        #[arg(long, default_value = "designs/piston-door-3x3/archive/verdicts-v5.tsv")]
        verdicts: PathBuf,
        /// Also run designs the cache does not know (verdict "uncached").
        #[arg(long)]
        uncached: bool,
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        #[arg(long, default_value_t = 24)]
        workers: usize,
        #[arg(long, default_value = "designs/piston-door-3x3/archive/properties-dyn-v1.tsv")]
        out_tsv: PathBuf,
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
    /// Every tick of one open/close cycle (design position) that breaks SUPER seamless: the
    /// circuitry seen in the tunnel T or the hallway ring S (props' reading); `*` marks what is
    /// still seen when a ring cell counts only while its T neighbour holds no door block (the
    /// covered reading, tier `supercov`).
    SeamlessWhy {
        designs: Vec<PathBuf>,
    },
    /// Redstone Squid catalogue properties (properties.rs) of the given designs, or of every
    /// working design on HEAD and the explore/* branches (the leaderboard's list, verdicts from
    /// the archive's verdicts-v5.tsv): seamless tiers, dents, trapdoor/flush/deluxe, hipster,
    /// symmetry, sync, clean, component restrictions, volume, timings. Cached by design hash in
    /// <archive-dir>/properties-v1.tsv; prints a markdown table.
    Properties {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        archive_dir: PathBuf,
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        #[arg(long, default_value_t = 12)]
        workers: usize,
        /// Also write the rows as TSV here (design, blocks, category, then every property).
        #[arg(long)]
        out_tsv: Option<PathBuf>,
        /// Write the markdown table here (also printed).
        #[arg(long)]
        out_md: Option<PathBuf>,
        /// Also run the dynamic checks (hallway-compatible, contained, snow, repeater input,
        /// player steps, uses), each at the design's own position.
        #[arg(long)]
        dynamic: bool,
        /// Recompute cached designs.
        #[arg(long)]
        redo: bool,
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
        /// Skip the rotation check (the door turned 90, 180 and 270 degrees, same positions).
        /// It is not part of the judge: it decides which board a working door goes on.
        #[arg(long)]
        no_rotations: bool,
        /// Also run all 8 orientations (4 turns, each as built and mirrored) and print which work.
        #[arg(long)]
        orientations: bool,
        /// Judge the design as a tile module (tile.rs): `wide:N` (openings side by side, one
        /// 3 x 3N opening), `period:P:N` (an opening every P blocks), `stack:N` (a 3N x 3
        /// opening), `vperiod:Q:N` (an opening every Q blocks up) or `step:dx,dy,dz:N`. Copy 0
        /// keeps the lever; the other copies get the module's `relay:` block in its cell.
        #[arg(long)]
        tile: Option<String>,
        /// With --tile: `one` (the tiling: one lever) or `each` (diagnostic: every copy keeps
        /// its lever, all flipped in the same tick).
        #[arg(long, default_value = "one")]
        tile_levers: String,
        /// With --tile: write the whole arrangement here (a plain design file).
        #[arg(long)]
        tile_out: Option<PathBuf>,
        /// With --tile: a shared end piece placed once in copy 0's frame (its `port lever`, if
        /// any, is the one lever; every copy then holds the relay).
        #[arg(long)]
        tile_extra: Option<PathBuf>,
        /// Also decide the "on the ground" board: the swept shape (nothing below the door's
        /// bottom row at rest or at any tick of a cycle) and the ground check (the judge's
        /// positions with the door standing on a stone floor).
        #[arg(long)]
        ground: bool,
    },
    /// Tile board (tile.rs): every design with a `tiling:` header (e.g. `tiling: period:8:3`) on
    /// HEAD and the explore/* branches, judged as a tile module; ranked per tiling kind by blocks
    /// per opening in a long row (TILEBOARD.md).
    TileBoard {
        /// Ref prefix to scan besides HEAD.
        #[arg(long, default_value = "refs/remotes/origin/explore/")]
        refs: String,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Skip designs with more blocks than this.
        #[arg(long, default_value_t = 200)]
        max_blocks: usize,
        /// Write the markdown here (also printed), e.g. designs/piston-door-3x3/TILEBOARD.md.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Tileability scan (tile.rs): every design under every tiling of --tiles, in parallel; prints
    /// per design which tilings work and the smallest working period.
    TileScan {
        /// Design files (or @builtins).
        designs: Vec<PathBuf>,
        /// Tilings, a comma list; period and vperiod take ranges: wide:3,stack:3,period:4..12:3.
        #[arg(long, default_value = "wide:3,stack:3,period:4..12:3,vperiod:4..10:3")]
        tiles: String,
        /// `one` (copy 0 keeps the lever, the others get the module's relay) or `each`
        /// (diagnostic: every copy flips its own lever in the same tick).
        #[arg(long, default_value = "each")]
        levers: String,
        /// Judge positions on top of the default four.
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 12)]
        workers: usize,
        /// Print the reason of every failure (default: the first 160 characters).
        #[arg(long)]
        verbose: bool,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::TileBoard { refs, extra_offsets, workers, max_blocks, out } => {
            let cands = crate::leaderboard::collect(std::path::Path::new("."), &refs, max_blocks)?;
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
            let mut opts = VerifyOptions::default();
            opts.offsets.extend(crate::extra_offsets(extra_offsets));
            let (_, md) = crate::tile::board(&pool, cands, &opts);
            print!("{md}");
            if let Some(p) = out {
                std::fs::write(p, &md)?;
            }
        }
        DoorCmd::TileScan { designs, tiles, levers, extra_offsets, workers, verbose } => {
            let mut ts = crate::tile::parse_tile_list(&tiles)?;
            let mode = match levers.as_str() {
                "one" => crate::tile::LeverMode::One,
                "each" => crate::tile::LeverMode::Each,
                other => anyhow::bail!("--levers {other}: expected one or each"),
            };
            for t in ts.iter_mut() {
                t.levers = mode;
            }
            let ds: Vec<(String, Design)> = designs.iter().map(|p| Ok((p.display().to_string(), load_design(p)?))).collect::<Result<_>>()?;
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
            let mut opts = VerifyOptions::default();
            opts.offsets.extend(crate::extra_offsets(extra_offsets));
            let rows = crate::tile::scan(&pool, &ds, &ts, &opts);
            for (i, (name, d)) in ds.iter().enumerate() {
                println!("== {name} ({} blocks, category {}{})", crate::block_count(d), crate::category(d), if crate::slime_free(d) { ", slime-free" } else { "" });
                let mut best: std::collections::BTreeMap<String, String> = Default::default();
                for (j, t, r) in rows.iter().filter(|(j, _, _)| *j == i) {
                    let _ = j;
                    let kind = t.label.split(':').next().unwrap_or("").to_string();
                    match r {
                        Ok((total, marginal, o, c)) => {
                            println!("  {:<16} WORKS {total} blocks, +{marginal} per copy, open {o} / close {c}", t.label);
                            best.entry(kind).or_insert_with(|| format!("{} (+{marginal} per copy)", t.label));
                        }
                        Err(e) => {
                            let e = if verbose { e.clone() } else { e.chars().take(160).collect() };
                            println!("  {:<16} fails: {e}", t.label);
                        }
                    }
                }
                for (k, v) in &best {
                    println!("  smallest working {k}: {v}");
                }
            }
        }
        DoorCmd::Catalog { archive, videos, release, workers } => {
            let o = crate::catalog::CatalogOptions { archive: &archive, workers, videos: videos.as_deref(), release: Some(release.as_str()) };
            crate::catalog::write(version, &o)?;
        }
        DoorCmd::Frontier { check, refs, extra_offsets, max_blocks, workers, cache_dir, any_category, all } => {
            let repo = std::path::Path::new(".");
            let mut cands = crate::leaderboard::collect(repo, &refs, max_blocks)?;
            let n_board = cands.len();
            for p in &check {
                cands.push((format!("check:{}", p.display()), load_design(p)?));
            }
            let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, Some(&cache_dir))?;
            let (board, checks): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| !e.source.starts_with("check:"));
            let _ = n_board;
            print!("{}", crate::leaderboard::frontier_report(&board, &checks, !any_category, all));
        }
        DoorCmd::Shapes { designs, dir, branches, refs, extra_offsets, max_blocks, workers, cache_dir, rule, table_max, no_table } => {
            let repo = std::path::Path::new(".");
            let mut cands = crate::shapes::collect_dir(&dir, max_blocks)?;
            if branches {
                cands.extend(crate::leaderboard::collect(repo, &refs, max_blocks)?);
            }
            for p in &designs {
                cands.push((p.display().to_string(), load_design(p)?));
            }
            let mut seen = std::collections::HashSet::new();
            cands.retain(|(_, d)| seen.insert(crate::leaderboard::design_hash(d)));
            let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, Some(&cache_dir))?;
            let rules = crate::shapes::rules(&rule)?;
            if !no_table {
                print!("{}", crate::shapes::table(&entries, &rules, table_max));
            }
            print!("{}", crate::shapes::best_report(&entries, &rules));
        }
        DoorCmd::Leaderboard { refs, fetch, extra_offsets, max_blocks, workers, out, sync_tracks, archive_dir, cache_dir, cached_only, release, captions } => {
            let repo = std::path::Path::new(".");
            if fetch {
                let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
                anyhow::ensure!(st.success(), "git fetch failed");
            }
            if sync_tracks {
                eprintln!("synced tracks: {:?}", crate::leaderboard::sync_tracks(repo, &refs)?);
            }
            let trusted = archive_dir.as_deref().map(crate::leaderboard::archived_keys).unwrap_or_default();
            let mut cands = crate::leaderboard::collect(repo, &refs, max_blocks)?;
            let cache = cache_dir.or_else(|| archive_dir.clone());
            let mut extras = crate::leaderboard::Extras::load(&captions, &release);
            if cached_only {
                let dir = cache.as_deref().ok_or_else(|| anyhow::anyhow!("--cached-only needs --cache-dir or --archive-dir"))?;
                extras.uncached = crate::leaderboard::only_cached(&mut cands, dir);
                eprintln!("{} designs without a cached verdict left out", extras.uncached);
            }
            eprintln!("verifying {} designs ({} archived designs trusted)", cands.len(), trusted.len());
            let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, cache.as_deref())?;
            if let Some(dir) = &archive_dir {
                for e in entries.iter().filter(|e| e.verdict.is_ok()) {
                    let note = format!("verified WORKS by door leaderboard: {} blocks, category {}", e.blocks, e.category);
                    if let Some(p) = crate::archive::save(std::path::Path::new(&e.source), &e.design, e.blocks, &note, Some(dir))? {
                        eprintln!("archived {}", p.display());
                    }
                }
            }
            let table = crate::leaderboard::render(&entries, extra_offsets, &extras);
            print!("{table}");
            if let Some(p) = out {
                std::fs::write(p, &table)?;
            }
        }
        DoorCmd::Audit { designs, sections, jvm, out, tsv, threads, cap, src, only_flat, explain } => {
            let designs = if designs.is_empty() { vec![PathBuf::from("designs/piston-door-3x3/tracks/c79/c79-closed.txt")] } else { designs };
            if explain {
                for d in &designs {
                    crate::audit::explain(d)?;
                }
            } else {
                crate::audit::run(&crate::audit::Opts { designs, sections, jvm, out, tsv, threads, cap, version: version.to_string(), src, only_flat })?;
            }
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
        DoorCmd::Turn { design, turns, mirror, out } => {
            let d = load_design(&design)?;
            let mut w = Worker::spawn(&cfg)?;
            let (mut rd, spec) = crate::transform_design(&mut w, &d, turns, mirror)?;
            rd.name = format!("{}-{}{}", d.name, if spec.along_z { "ew" } else { "ns" }, if mirror { "-mirrored" } else { "" });
            rd.save(&out)?;
            println!("{} turned {} degrees{} ({}-facing door) written to {}", d.name, turns as u32 % 4 * 90, if mirror { ", mirrored" } else { "" }, if spec.along_z { "east/west" } else { "north/south" }, out.display());
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
        DoorCmd::Blocks { design, only } => {
            let d = load_design(&design)?;
            let bx: Option<([i32; 3], [i32; 3])> = match &only {
                None => None,
                Some(s) => {
                    let (a, b) = anyhow::Context::context(s.split_once(':'), "--only wants x0,y0,z0:x1,y1,z1")?;
                    let p = |t: &str| -> Result<[i32; 3]> {
                        let v: Vec<i32> = t.split(',').map(|n| n.trim().parse()).collect::<std::result::Result<_, _>>()?;
                        anyhow::ensure!(v.len() == 3, "a corner needs 3 coordinates: {t}");
                        Ok([v[0], v[1], v[2]])
                    };
                    let (a, b) = (p(a)?, p(b)?);
                    Some(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
                }
            };
            let mut ps: Vec<(&[i32; 3], &String)> = d.blocks.iter().filter(|(p, _)| bx.is_none_or(|(lo, hi)| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]))).collect();
            ps.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
            for (p, s) in ps {
                println!("{} {} {} {}", p[0], p[1], p[2], s.strip_prefix("minecraft:").unwrap_or(s));
            }
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
        DoorCmd::Relever { designs, subs, out_dir, extra_offsets, workers, verdicts, max_blocks, any_rotation, table, why, branches, seamless } => {
            let offsets = crate::search::judge_offsets(extra_offsets);
            let judge = VerifyOptions { offsets: offsets.clone(), ..Default::default() };
            let screen = VerifyOptions { cycles: 1, max_ticks: 200, offsets: offsets.iter().take(2).copied().collect(), keep_frames: false, empty_walkway: true, tile: None };
            let known = verdicts.as_deref().map(crate::properties_dyn::load_verdicts).unwrap_or_default();
            let seam = seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?;
            let subs: Vec<String> = subs.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let mut seen = std::collections::HashSet::new();
            let mut loaded: Vec<(String, Design)> = Vec::new();
            let mut sources: Vec<(String, Design)> = Vec::new();
            for p in &designs {
                match load_design(p) {
                    Ok(d) => sources.push((p.display().to_string(), d)),
                    Err(e) => eprintln!("{}: {e:#}", p.display()),
                }
            }
            if branches {
                sources.extend(crate::properties_dyn::collect_refs(std::path::Path::new("."), "refs/remotes/origin/explore/", max_blocks)?);
            }
            for (src, d) in sources {
                let h = format!("{:016x}", crate::leaderboard::design_hash(&d));
                if crate::block_count(&d) > max_blocks || known.get(&h) == Some(&false) || !seen.insert(d.content_key()) {
                    continue;
                }
                loaded.push((src, d));
            }
            // Seamless pre-screen (the tier only, as the design is).
            if let Some(req) = seam {
                let tier_only = crate::properties::SeamReq { hallway: false, ..req };
                let pool = mctas::sim::Pool::spawn(workers.min(loaded.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
                let keep = pool.map(loaded.clone(), |w, _, (_, d)| {
                    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
                    let ok = tier_only.check(w, sb, &d).is_ok();
                    let _ = w.drop_sandbox(sb);
                    Ok(ok)
                });
                let before = loaded.len();
                loaded = loaded.into_iter().zip(keep).filter(|(_, k)| matches!(k, Ok(true))).map(|(x, _)| x).collect();
                eprintln!("seamless pre-screen: {} of {before} designs reach the tier", loaded.len());
            }
            let mut jobs: Vec<(usize, crate::relever::Variant)> = Vec::new();
            for (i, (_, d)) in loaded.iter().enumerate() {
                match crate::relever::variants(d, &subs) {
                    Ok(vs) => jobs.extend(vs.into_iter().map(|v| (i, v))),
                    Err(e) => eprintln!("{}: {e:#}", loaded[i].0),
                }
            }
            eprintln!("{} designs, {} variants (as-is included), {} positions", loaded.len(), jobs.len(), offsets.len());
            let pool = mctas::sim::Pool::spawn(workers.min(jobs.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
            let results = pool.map(jobs, |w, _, (i, v)| Ok((i, crate::relever::judge_variant(w, v, &judge, &screen, !any_rotation, seam)?)));
            let mut per: Vec<Vec<crate::relever::Outcome>> = vec![Vec::new(); loaded.len()];
            for r in results {
                match r {
                    Ok((i, o)) => per[i].push(o),
                    Err(e) => eprintln!("worker error: {e:#}"),
                }
            }
            let mut order: Vec<usize> = (0..loaded.len()).collect();
            order.sort_by_key(|&i| (crate::category(&loaded[i].1) != "solid", crate::category(&loaded[i].1), crate::block_count(&loaded[i].1), loaded[i].0.clone()));
            let mut md = String::from("| design | blocks | category | as-is | eligible variants (blocks, lever open/close, repeater open/close, seamless) |\n|---|---|---|---|---|\n");
            if let Some(dir) = &out_dir {
                std::fs::create_dir_all(dir)?;
            }
            for i in order {
                let (src, d) = &loaded[i];
                let stem = std::path::Path::new(src).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let mut asis = String::from("-");
                let mut good = Vec::new();
                for o in &per[i] {
                    match (&o.verdict, o.label.as_str()) {
                        (Ok((lo, lc, ro, rc, t)), "as-is") => asis = format!("ELIGIBLE ({lo}/{lc}, rep {ro}/{rc}, {t})"),
                        (Err(why), "as-is") => asis = format!("no: {}", why.chars().take(90).collect::<String>()),
                        (Ok((lo, lc, ro, rc, t)), label) => {
                            good.push(format!("{label} ({}, {lo}/{lc}, rep {ro}/{rc}, {t})", crate::block_count(&o.design)));
                            if let Some(dir) = &out_dir {
                                let mut nd = o.design.clone();
                                nd.name = format!("{}-rl-{label}", d.name);
                                nd.save(&dir.join(format!("{stem}-rl-{label}.txt")))?;
                            }
                        }
                        (Err(reason), label) if why => println!("  {stem} {label}: {}", reason.chars().take(300).collect::<String>()),
                        _ => {}
                    }
                }
                let line = format!("| {stem} | {} | {} | {asis} | {} |", crate::block_count(d), crate::category(d), if good.is_empty() { "-".to_string() } else { good.join("; ") });
                println!("{line}");
                md.push_str(&line);
                md.push('\n');
            }
            if let Some(p) = table {
                std::fs::write(p, md)?;
            }
        }
        DoorCmd::Eligible { designs, extra_offsets, workers } => {
            let offsets = crate::search::judge_offsets(extra_offsets);
            let loaded: Vec<(String, Design)> = designs.iter().map(|p| Ok((p.display().to_string(), load_design(p)?))).collect::<Result<_>>()?;
            let pool = mctas::sim::Pool::spawn(workers.min(loaded.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
            let lines = pool.map(loaded, |w, _, (name, d)| Ok(crate::properties_dyn::eligibility_line(w, &name, &d, &offsets)));
            let mut all_ok = true;
            for l in lines {
                let (ok, line) = l?;
                all_ok &= ok;
                println!("{line}");
            }
            if !all_ok {
                std::process::exit(1);
            }
        }
        DoorCmd::PropertiesDyn { designs, archive_dir, branches, refs, verdicts, uncached, max_blocks, workers, out_tsv } => {
            use crate::properties_dyn as pd;
            let cache = pd::load_verdicts(&verdicts);
            let mut cands: Vec<(String, Design, bool)> = Vec::new();
            for p in &designs {
                cands.push((p.display().to_string(), load_design(p)?, true));
            }
            if let Some(dir) = &archive_dir {
                cands.extend(crate::shapes::collect_dir(dir, max_blocks)?.into_iter().map(|(s, d)| (s, d, false)));
            }
            if branches {
                cands.extend(pd::collect_refs(std::path::Path::new("."), &refs, max_blocks)?.into_iter().map(|(s, d)| (s, d, false)));
            }
            let mut seen = std::collections::HashSet::new();
            let mut jobs: Vec<(String, Design, String)> = Vec::new();
            let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
            for (src, d, explicit) in cands {
                let h = format!("{:016x}", crate::leaderboard::design_hash(&d));
                if !seen.insert(h.clone()) {
                    continue;
                }
                let verdict = match cache.get(&h) {
                    Some(true) => "ok",
                    Some(false) => "broken",
                    None => "uncached",
                };
                *counts.entry(verdict).or_default() += 1;
                if explicit || verdict == "ok" || (uncached && verdict == "uncached") {
                    jobs.push((src, d, verdict.to_string()));
                }
            }
            jobs.sort_by_key(|(s, d, _)| (crate::block_count(d), s.clone()));
            eprintln!(
                "properties-dyn: {counts:?} in the verdict cache; running {} designs{}",
                jobs.len(),
                if uncached { " (uncached ones only if the judge passes them at offset 0)" } else { " (uncached skipped; --uncached runs them)" }
            );
            if jobs.is_empty() {
                return Ok(());
            }
            let pool = mctas::sim::Pool::spawn(workers.min(jobs.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
            let t0 = std::time::Instant::now();
            let done = std::sync::atomic::AtomicUsize::new(0);
            let n = jobs.len();
            let results = pool.map(jobs, |w, _, (src, d, verdict)| {
                // A design the cache does not know runs only if the judge passes it at the
                // design's own position (current rules).
                let precheck = verdict != "uncached"
                    || crate::verify(w, &d, &crate::VerifyOptions { offsets: vec![[0, 0, 0]], ..Default::default() }).is_ok();
                let r = if precheck {
                    let v = if verdict == "uncached" { "uncached (judge ok at offset 0)" } else { verdict.as_str() };
                    pd::check_design(w, &src, &d, v).map(Some).map_err(|e| e.context(src.clone()))
                } else {
                    Ok(None)
                };
                let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                eprintln!("[{k}/{n} {:.0}s] {src}{}", t0.elapsed().as_secs_f64(), if precheck { "" } else { " (uncached, fails the judge: skipped)" });
                r
            });
            let mut rows = Vec::new();
            for r in results {
                match r {
                    Ok(Some(row)) => rows.push(row),
                    Ok(None) => {}
                    Err(e) => eprintln!("error: {e:#}"),
                }
            }
            rows.sort_by_key(|r| (r.blocks, r.source.clone()));
            let mut tsv = String::from(pd::TSV_HEADER);
            tsv.push('\n');
            for r in &rows {
                tsv.push_str(&pd::tsv_line(r));
                tsv.push('\n');
            }
            if let Some(dir) = out_tsv.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(&out_tsv, tsv)?;
            let table = pd::table(&rows);
            std::fs::write(out_tsv.with_extension("md"), &table)?;
            print!("{table}");
            eprintln!("wrote {} and {} ({} rows)", out_tsv.display(), out_tsv.with_extension("md").display(), rows.len());
        }
        DoorCmd::SeamlessWhy { designs } => {
            let mut w = mctas::sim::Worker::spawn(&cfg)?;
            for path in &designs {
                let d = load_design(path)?;
                let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
                let rec = crate::properties::record(&mut w, sb, &d);
                let _ = w.drop_sandbox(sb);
                let rec = rec?;
                let s = crate::properties::seamless(&rec);
                println!("== {} ({} blocks): {}{}", path.display(), crate::block_count(&d), s.label, s.why.map(|w| format!(" -- {w}")).unwrap_or_default());
                for line in crate::properties::super_report(&rec) {
                    println!("  {line}");
                }
            }
        }
        DoorCmd::Properties { designs, archive_dir, refs, workers, out_tsv, out_md, dynamic, redo } => {
            use crate::properties as pr;
            let repo = std::path::Path::new(".");
            let verdicts = pr::cached_verdicts(&archive_dir);
            let vanilla = pr::vanilla_verdicts(repo);
            let mut cands: Vec<(String, Design)> = Vec::new();
            if designs.is_empty() {
                let all = crate::leaderboard::collect(repo, &refs, 400)?;
                let n = all.len();
                let mut unknown = 0;
                for (s, d) in all {
                    match verdicts.get(&crate::leaderboard::design_hash(&d)) {
                        Some((true, _)) => cands.push((s, d)),
                        Some((false, _)) => {}
                        None => {
                            unknown += 1;
                            cands.push((s, d));
                        }
                    }
                }
                eprintln!("{n} designs, {} working or not judged yet ({unknown} not judged: screened by the judge at their own position)", cands.len());
            } else {
                for p in &designs {
                    cands.push((p.display().to_string(), load_design(p)?));
                }
            }
            let mut cache = pr::load_cache(&archive_dir);
            let todo: Vec<(u64, Design)> = {
                let mut seen = std::collections::HashSet::new();
                cands
                    .iter()
                    .map(|(_, d)| (crate::leaderboard::design_hash(d), d.clone()))
                    .filter(|(h, _)| (redo || !cache.get(h).is_some_and(|p| !dynamic || p.contains_key("hallway"))) && seen.insert(*h))
                    .collect()
            };
            eprintln!("{} designs to compute", todo.len());
            if !todo.is_empty() {
                let pool = mctas::sim::Pool::spawn(workers.min(todo.len()).max(1), &cfg)?;
                let results = pool.map(todo, |w, _, (h, d)| {
                    if !verdicts.contains_key(&h) {
                        let one = VerifyOptions { offsets: vec![[0, 0, 0]], ..Default::default() };
                        if let Err(e) = crate::verify(w, &d, &one) {
                            let mut p = pr::Props::new();
                            p.insert("screen".into(), format!("broken: {}", format!("{e:#}").replace(['\t', '\n'], " ")));
                            return Ok((h, Ok(p)));
                        }
                    }
                    Ok((h, pr::compute(w, &d, dynamic)))
                });
                for r in results {
                    let (h, p) = r?;
                    match p {
                        Ok(p) => {
                            let mut merged = cache.remove(&h).unwrap_or_default();
                            merged.extend(p);
                            cache.insert(h, merged);
                        }
                        Err(e) => eprintln!("{h:016x}: {e:#}"),
                    }
                }
                pr::save_cache(&archive_dir, &cache)?;
            }
            let dynp = pr::load_dyn(&archive_dir);
            if !dynp.is_empty() {
                eprintln!("joined {} designs' dynamic checks from properties-dyn-v1.tsv", dynp.len());
            }
            let mut rows = Vec::new();
            for (s, d) in &cands {
                let h = crate::leaderboard::design_hash(d);
                let Some(mut p) = cache.get(&h).cloned() else { continue };
                if let Some(x) = dynp.get(&h) {
                    p.extend(x.clone());
                }
                if p.contains_key("steps") {
                    let sculk = p.get("risky").is_some_and(|r| r.contains("sculk"));
                    let steps_ok = p.get("steps").is_some_and(|s| s.starts_with("yes"));
                    let risky = p.get("risky").is_some_and(|r| r != "-");
                    p.insert(
                        "isolated".into(),
                        if sculk && !steps_ok {
                            "no (hears footsteps)".into()
                        } else if risky {
                            "? (steps ok, outside untested)".into()
                        } else {
                            "yes".into()
                        },
                    );
                    // Record eligibility (3): valid input (lever on the surface, steady support,
                    // repeater input), unlimited uses, footsteps harmless, isolated, and a seamless
                    // tier whose hallway can be built.
                    let yes = |k: &str| p.get(k).is_some_and(|v| v.starts_with("yes"));
                    let mut miss = Vec::new();
                    for (k, what) in [
                        ("lever_surface", "lever"),
                        ("lever_support", "support"),
                        ("repeater_input", "repeater input"),
                        ("uses", "uses"),
                        ("steps", "steps"),
                        ("isolated", "isolated"),
                        ("hallway", "hallway"),
                    ] {
                        if !yes(k) {
                            miss.push(what);
                        }
                    }
                    if p.get("seamless").is_none_or(|s| s == "-") {
                        miss.push("seamless");
                    }
                    p.insert("eligible".into(), if miss.is_empty() { "yes".into() } else { format!("no ({})", miss.join(", ")) });
                }
                if p.get("screen").is_some_and(|s| s.starts_with("broken")) {
                    continue;
                }
                if let Some((ok, rot)) = verdicts.get(&h) {
                    p.insert("dir".into(), if rot == "safe" { "yes".into() } else { format!("no ({rot})") });
                    p.insert("loc".into(), if *ok { "yes (32 pos)".into() } else { "no".into() });
                } else {
                    p.insert("dir".into(), "? (not judged yet)".into());
                    p.insert("loc".into(), "? (1 pos)".into());
                }
                p.insert("vanilla".into(), vanilla.get(&h).cloned().unwrap_or_else(|| "sim only".into()));
                rows.push((pr::display_name(s), crate::block_count(d), crate::category(d), p));
            }
            rows.sort_by(|a, b| (a.2 != "solid", a.2, a.1, &a.0).cmp(&(b.2 != "solid", b.2, b.1, &b.0)));
            let leaders: Vec<_> = pr::LEADERS.iter().filter_map(|n| rows.iter().find(|r| r.0 == *n).cloned()).collect();
            let table = format!(
                "# Door properties (Redstone Squid catalogue rules)\n\nDefinitions and judgment calls: [NOTES.md](NOTES.md). One position, one orientation, one cycle per check; not directional / not locational from the leaderboard's cached verdicts.\n\n## Board leaders\n\n{}\n## Best design per property\n\n{}\n## Every working design ({})\n\n{}",
                pr::render(&leaders),
                pr::summary(&rows),
                rows.len(),
                pr::render(&rows)
            );
            print!("{table}");
            if let Some(path) = out_md {
                std::fs::write(path, &table)?;
            }
            if let Some(path) = out_tsv {
                let keys: Vec<String> = {
                    let mut k = std::collections::BTreeSet::new();
                    for r in &rows {
                        k.extend(r.3.keys().cloned());
                    }
                    k.into_iter().collect()
                };
                let mut out = format!("design\tblocks\tcategory\t{}\n", keys.join("\t"));
                for (n, b, c, p) in &rows {
                    let vals: Vec<String> = keys.iter().map(|k| p.get(k).cloned().unwrap_or_default()).collect();
                    out.push_str(&format!("{n}\t{b}\t{c}\t{}\n", vals.join("\t")));
                }
                std::fs::write(path, out)?;
            }
        }
        DoorCmd::Verify { design, timeline: show, cycles, archive, extra_offsets, archive_dir, no_rotations, orientations, tile, tile_levers, tile_out, tile_extra, ground } => {
            let d = load_design(&design)?;
            if let Some(spec) = tile {
                let mut t = crate::tile::Tile::parse(&spec)?;
                if let Some(p) = &tile_extra {
                    t.extra = Some(load_design(p)?);
                }
                t.levers = match tile_levers.as_str() {
                    "one" => crate::tile::LeverMode::One,
                    "each" => crate::tile::LeverMode::Each,
                    other => anyhow::bail!("--tile-levers {other}: expected one or each"),
                };
                let mut w = Worker::spawn(&cfg)?;
                let mut opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
                opts.offsets.extend(crate::extra_offsets(extra_offsets));
                return tile_verify_cmd(&mut w, &design, &d, &t, &opts, no_rotations, archive, archive_dir.as_deref(), tile_out.as_deref());
            }
            let spec = DoorSpec::from_design(&d)?;
            let mut w = Worker::spawn(&cfg)?;
            let mut opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
            opts.offsets.extend(crate::extra_offsets(extra_offsets));
            match verify(&mut w, &d, &opts) {
                Ok(r) => {
                    println!(
                        "WORKS: {} ({} blocks{}, {}x{}x{}, category {}{}{})",
                        d.name,
                        r.blocks,
                        match crate::entity_count(&d) {
                            0 => String::new(),
                            n => format!(" + {n} entit{}", if n == 1 { "y" } else { "ies" }),
                        },
                        r.size[0],
                        r.size[1],
                        r.size[2],
                        crate::category(&d),
                        if crate::slime_free(&d) { ", slime-free" } else { "" },
                        if crate::single_layer(&d) { ", flat" } else { "" }
                    );
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
                    if !no_rotations {
                        let ropts = VerifyOptions { keep_frames: false, ..opts.clone() };
                        match crate::rotation_check(&mut w, &d, &ropts)? {
                            None => println!("rotation-safe: also works turned 90, 180 and 270 degrees ({} positions each)", ropts.offsets.len()),
                            Some((turns, why)) => println!("NOT rotation-safe (listed on the separate board): turned {} degrees: {why}", turns as u32 * 90),
                        }
                    }
                    if ground {
                        let swept = crate::swept_shape(&mut w, &d)?;
                        let gopts = VerifyOptions { keep_frames: false, ..opts.clone() };
                        if swept.below > 0 {
                            println!("NOT on the ground: {} row(s) below the door at rest or while moving", swept.below);
                        } else {
                            match crate::ground_check(&mut w, &d, &gopts)? {
                                None => println!("on the ground: nothing below the door at any tick, works on a stone floor ({} positions)", gopts.offsets.len()),
                                Some(why) => println!("NOT on the ground: fails standing on a stone floor: {}", why.chars().take(200).collect::<String>()),
                            }
                        }
                    }
                    if orientations {
                        let ropts = VerifyOptions { keep_frames: false, ..opts.clone() };
                        println!("orientations ({} positions each; turns are clockwise seen from above, mirrored = east-west mirror image first):", ropts.offsets.len());
                        for (turns, mirrored, r) in crate::orientation_report(&mut w, &d, &ropts)? {
                            let name = format!("{:>3} degrees{}", turns as u32 * 90, if mirrored { ", mirrored" } else { "" });
                            match r {
                                Ok(()) => println!("  {name:<22} WORKS"),
                                Err(e) => println!("  {name:<22} BROKEN: {}", e.chars().take(140).collect::<String>()),
                            }
                        }
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

/// `door verify --tile`: the judge on every opening of a tiled module, with per-opening and
/// per-module block counts and timings.
#[allow(clippy::too_many_arguments)]
fn tile_verify_cmd(
    w: &mut Worker,
    path: &std::path::Path,
    d: &Design,
    t: &crate::tile::Tile,
    opts: &VerifyOptions,
    no_rotations: bool,
    archive: bool,
    archive_dir: Option<&std::path::Path>,
    tile_out: Option<&std::path::Path>,
) -> Result<()> {
    use crate::tile::*;
    let lever_mode = match t.levers {
        LeverMode::One => "one lever",
        LeverMode::Each => "DIAGNOSTIC: every copy flips its own lever",
    };
    let tiled = expand(d, t)?;
    if let Some(out) = tile_out {
        tiled.design.save(out)?;
        println!("arrangement written to {}", out.display());
    }
    let openings = tiled.specs.len();
    let total = tiled_blocks(&tiled);
    let marginal = marginal_blocks(d, t)?;
    let per_copy = openings / tiled.copies;
    let relay = relay_state(d)?.unwrap_or_else(|| "none (the lever cell stays empty)".into());
    match verify_tiled(w, d, t, opts) {
        Ok((r, per, _)) => {
            println!(
                "WORKS (tile {}, {lever_mode}): {}: {} copies, {openings} openings, {} blocks total ({:.1} per opening); module {} blocks, +{marginal} per extra copy ({:.1} per opening in a long row); category {}{}{}",
                t.label,
                d.name,
                tiled.copies,
                total,
                total as f64 / openings as f64,
                crate::block_count(d),
                marginal as f64 / per_copy as f64,
                crate::category(d),
                if crate::slime_free(d) { ", slime-free" } else { "" },
                if crate::single_layer(d) { ", flat" } else { "" }
            );
            println!("  relay in the copies without the lever: {relay}; cells shared by copies: {}", tiled.shared);
            let o = r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0);
            let c = r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0);
            println!("  all openings: open after {o} ticks, closed after {c} ticks (worst cycle, first position)");
            let line: Vec<String> = per.iter().enumerate().map(|(i, (a, b))| format!("#{i} {a}/{b}")).collect();
            println!("  per opening (open/close): {}", line.join(", "));
            for (i, cy) in r.cycles.iter().enumerate() {
                println!("  cycle {i}: open after {} ticks (idle at {}), closed after {} ticks (idle at {})", cy.open_ticks, cy.open_settle, cy.close_ticks, cy.close_settle);
            }
            for warning in &r.warnings {
                println!("  warning: {warning}");
            }
            if archive && t.levers == LeverMode::One {
                let note = format!(
                    "verified WORKS as a tile module: --tile {} ({} openings, {} blocks total, +{marginal} per extra copy), open {o} / close {c}",
                    t.label, openings, total
                );
                match crate::archive::save(path, d, marginal, &note, archive_dir)? {
                    Some(p) => println!("archived {}", p.display()),
                    None => println!("already archived"),
                }
            }
            if !no_rotations {
                let ropts = VerifyOptions { keep_frames: false, ..opts.clone() };
                match tiled_rotation_check(w, &tiled, &ropts)? {
                    None => println!("rotation-safe: the arrangement also works turned 90, 180 and 270 degrees ({} positions each)", ropts.offsets.len()),
                    Some((turns, why)) => println!("NOT rotation-safe: turned {} degrees: {why}", turns as u32 * 90),
                }
            }
            if opts.keep_frames {
                for (i, spec) in tiled.specs.iter().enumerate() {
                    println!("opening #{i} (top row, middle row, bottom row; # block, ~ moving, h piston head, . air):");
                    print!("{}", timeline(&r.open_frames, spec));
                    println!("closing #{i}:");
                    print!("{}", timeline(&r.close_frames, spec));
                }
            }
        }
        Err(e) => {
            println!("BROKEN (tile {}, {lever_mode}): {}: {e:#}", t.label, d.name);
            std::process::exit(1);
        }
    }
    Ok(())
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
