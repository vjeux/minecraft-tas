//! `mctas door` commands: block-count and speed searches on whole designs: minimize, grow, regrow, harvest, transplant, synth, resynth, route, route2, place.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::{SearchArgs, load_design};
use piston_door::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
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
        /// Take the swept cells from this working design instead (e.g. the design before a net
        /// was cut out of it to be re-laid elsewhere): only the cells its moving parts visit, so
        /// the cut net's own cells are free again.
        #[arg(long)]
        swept_design: Option<PathBuf>,
        /// Append the path to this file as `door patch` lines (`set x y z <state>`) instead of
        /// printing builder code.
        #[arg(long)]
        patch: Option<PathBuf>,
    },
    /// Try every combination of alternative patch groups (see place.rs): for wiring a mechanism
    /// whose part placements are a 3D puzzle. Screened at one position, judged at the default
    /// positions; every working combination is written next to --out.
    Place {
        design: PathBuf,
        #[arg(long)]
        groups: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Stop after this many working designs (0 = all).
        #[arg(long, default_value_t = 0)]
        want: usize,
        /// Extra pseudo-random judge positions on top of the default four.
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
    },
    /// Port-aware, timing-aware net router (route2.rs). Rips every connected group of up to
    /// --max-rip control blocks (or the --rip groups) and re-routes the receivers whose signal
    /// changed (taps anywhere, enters each piston any way, places dust, repeaters, observers and
    /// conductor blocks to reproduce every piston cell's signal tick for tick in a static model),
    /// with fewer blocks; candidates are screened and judged. With --need, routes a new net:
    /// the listed piston cells must get the given waves (see `Wave::parse`).
    Route2 {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Largest group of control blocks ripped at once.
        #[arg(long, default_value_t = 5)]
        max_rip: usize,
        /// Rip exactly these groups: cells "x,y,z;x,y,z", groups separated by '|'.
        #[arg(long, default_value = "")]
        rip: String,
        /// New net: "x,y,z=WAVE;..." piston cells and the signal each must get (level+D,
        /// pulse+D/L, on, off, open:A-B;close:C-). Every other piston cell keeps its signal.
        #[arg(long, default_value = "")]
        need: String,
        /// Only tap these cells ("x,y,z;..."; default: any powered block or watched cell).
        #[arg(long, default_value = "")]
        taps: String,
        /// What the router may place: dust, rep, obs, block, gt, torch or full states.
        #[arg(long, default_value = "dust,rep,obs,block,gt")]
        palette: String,
        /// Accept a piston cell's signal shifted by up to this many ticks.
        #[arg(long, default_value_t = 0)]
        tolerance: i32,
        #[arg(long, default_value_t = 6)]
        beam: usize,
        #[arg(long, default_value_t = 4)]
        hits: usize,
        #[arg(long, default_value_t = 60000)]
        max_states: usize,
        /// Routing box: the design's bounds grown by this much.
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Do not place observers watching cells the machine changes.
        #[arg(long)]
        no_watch: bool,
        /// Simulate candidates even when the static model rejects them.
        #[arg(long)]
        no_check: bool,
        /// Print the model: every piston cell's signal and a calibration against the simulator.
        #[arg(long)]
        verbose: bool,
        /// Print the model and stop.
        #[arg(long)]
        model_only: bool,
        /// Allow re-routes with this many more blocks than the rip removes (1 = same size: a
        /// sanity check that the router re-finds the existing wiring; such designs are reported,
        /// not kept).
        #[arg(long, default_value_t = 0)]
        extra_blocks: u32,
        /// Also rip pairs of separate groups (two nets re-routed together, e.g. from one shared
        /// source), up to --max-rip blocks in all.
        #[arg(long)]
        pairs: bool,
        /// Plateau: re-route at equal size too, judge those designs, and give each a local search
        /// (--moves, --polish-secs each, at most --polish-max designs).
        #[arg(long)]
        polish: bool,
        #[arg(long, default_value_t = 60)]
        polish_secs: u64,
        #[arg(long, default_value_t = 20)]
        polish_max: usize,
        #[arg(long)]
        archive: bool,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Cut every face-connected group of --size control blocks (pistons excluded) and re-grow it
    /// with size-1 blocks from --states inside the group's box grown by --margin (see search.rs
    /// `regrow`). The best verified design is written to --out.
    Regrow {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Group size: 2 (re-grown with 1 block) or 3 (with 2).
        #[arg(long, default_value_t = 2)]
        size: usize,
        #[arg(long, default_value_t = 1)]
        margin: i32,
        /// Block states to re-grow with (see search.rs `grow_states`; `trapdoor`: the watchable
        /// relay that shortened six doors on the exotic track).
        #[arg(long, default_value = "obs,block,dust,rep,rep2,rep3,rep4,torch,glass,trapdoor")]
        states: String,
        /// Only cut groups inside this box, "x0,y0,z0:x1,y1,z1".
        #[arg(long)]
        only: Option<String>,
        #[arg(long, default_value_t = 1)]
        want: usize,
        /// Cut any group of --size control blocks, connected or not (new blocks go within
        /// --margin of any cut block).
        #[arg(long)]
        any: bool,
        /// Keep only candidates that add at least one block whose state contains one of these
        /// (comma list; shorthands as `resynth --require-any`, plus trapdoor, gate, rail, disp,
        /// crafter, skull, shelf, scaffold, grate, tinted, slab, stairs): sweep new states on a
        /// design the old ones already exhausted.
        #[arg(long, default_value = "")]
        require_any: String,
        #[command(flatten)]
        search: SearchArgs,
    },
    /// Add 1 or 2 blocks (from --states, in the free cells of --lo..--hi) to a design, usually
    /// one whose old wiring was removed, screening every combination and judging the survivors
    /// (see search.rs `grow`). The best verified design is written to --out.
    Grow {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Box corner "x,y,z" (inclusive).
        #[arg(long)]
        lo: String,
        /// Opposite box corner "x,y,z" (inclusive).
        #[arg(long)]
        hi: String,
        /// Comma list: obs, block, dust, rep[1-4], torch, glass, obsidian, rblock, lamp, note, leaves
        /// (persistent), log (3 axes), or full states without commas.
        #[arg(long, default_value = "obs,block,dust")]
        states: String,
        /// Blocks added together (1 or 2; 2 also tries singles).
        #[arg(long, default_value_t = 2)]
        k: usize,
        /// Stop after this many verified designs (0 = all).
        #[arg(long, default_value_t = 1)]
        want: usize,
        /// Seed-and-exploit: run the local search (--moves, --plateau...) from EVERY verified
        /// grown design and keep the best result below the start design's block count (a block
        /// that changes the machine, e.g. an observer riding a slime structure, can make others
        /// redundant). Use with --want 0.
        #[arg(long)]
        then_search: bool,
        /// With --then-search: minutes of local search per seed (0 = until it stops improving).
        #[arg(long, default_value_t = 0.0)]
        seed_minutes: f64,
        /// With --then-search: start at this seed (1-based; skip the ones before).
        #[arg(long, default_value_t = 1)]
        seed_from: usize,
        /// Riders only: every added block must stick to a slime/honey structure (directly, or with
        /// --k 2 through an added slime/honey stuck to one).
        #[arg(long)]
        glue: bool,
        /// With --then-search: skip seeds whose open/close ticks (screen positions) equal the
        /// start design's, i.e. blocks that most likely change nothing.
        #[arg(long)]
        changed: bool,
        #[command(flatten)]
        search: SearchArgs,
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
    /// Circuit synthesis by exhaustive simulation (see synth.rs): every circuit of up to --k
    /// blocks from --palette in the free cells of --lo..--hi that carries a signal from the
    /// --from cells to the --to cells is simulated against a target (the door judge by default,
    /// or a --spec behavior script); the smallest verified circuits are reported and written.
    Synth {
        /// The context: a door design or a gadget harness (cells in --clear are emptied first).
        context: PathBuf,
        #[arg(long)]
        lo: String,
        #[arg(long)]
        hi: String,
        /// Circuit blocks (see synth::palette_states): block,gt,glass,dust,dot,rep,rep1..4,cmp,obs,torch,rblock,lamp,target,note,...
        #[arg(long, default_value = "block,dust,rep4")]
        palette: String,
        /// Blocks added under parts that need support (they count).
        #[arg(long, default_value = "block")]
        floors: String,
        /// Most blocks per circuit.
        #[arg(long, default_value_t = 4)]
        k: usize,
        /// Skip circuits smaller than this.
        #[arg(long, default_value_t = 1)]
        min_k: usize,
        /// Source cells of the context, "x,y,z;...": the lever, a powered block, an observer...
        #[arg(long)]
        from: String,
        /// Cells that change while the machine runs (moving blocks, pistons; air allowed):
        /// observers may watch them.
        #[arg(long, default_value = "")]
        watch: String,
        /// Sink cells: some circuit block must drive one of them.
        #[arg(long)]
        to: String,
        /// Context cells to empty first (the old circuit), "x,y,z;...".
        #[arg(long, default_value = "")]
        clear: String,
        /// Cells the circuit may not use, "x,y,z;...".
        #[arg(long, default_value = "")]
        forbid: String,
        /// Parts constraints: at least n of a kind, "repeater:2,observer:1".
        #[arg(long, default_value = "")]
        require: String,
        /// Parts constraints: at most n of a kind, "repeater:1".
        #[arg(long, default_value = "")]
        limit: String,
        /// Keep only circuits using at least one of these (lamp, bulb, note, target or any
        /// block-state substring), comma list.
        #[arg(long, default_value = "")]
        require_any: String,
        /// Behavior spec (see synth.rs); without it the target is the door judge.
        #[arg(long)]
        spec: Option<PathBuf>,
        /// World offsets a spec is checked at (the judge's list).
        #[arg(long, default_value_t = 2)]
        spec_offsets: usize,
        /// Keep circuits with blocks that are on no source-to-sink path.
        #[arg(long)]
        loose: bool,
        /// Only count the candidates.
        #[arg(long)]
        count: bool,
        /// Keep going after the first size with a verified circuit.
        #[arg(long)]
        all_sizes: bool,
        /// Stop after this many verified circuits (0 = every one of the smallest size).
        #[arg(long, default_value_t = 0)]
        want: usize,
        /// Write every verified circuit here (synth-<size>-<i>.txt).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 24)]
        workers: usize,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 2)]
        screen_offsets: usize,
        #[arg(long, default_value_t = 200)]
        screen_ticks: u32,
        #[arg(long, default_value_t = 50_000_000)]
        max_cands: usize,
        /// Speed bounds (speed boards): reject doors that open later than this many ticks.
        #[arg(long)]
        max_open: Option<u32>,
        /// Speed bounds: reject doors that close later than this many ticks.
        #[arg(long)]
        max_close: Option<u32>,
        /// Judge every circuit as a tile module (tile.rs; see `door verify --tile`).
        #[arg(long)]
        tile: Option<String>,
    },
    /// Large-neighborhood search by exact synthesis (see synth.rs `resynth`): every connected
    /// group of 2..--max-size removable blocks (wiring, still plain blocks) is cleared and every
    /// circuit of fewer blocks that could reconnect its surroundings is screened and judged; an
    /// accepted replacement is applied and the search restarts. Writes the best design to --out.
    /// Several designs: a sweep (one worker pool), --out is a folder, each improved design is
    /// written there as `<name>-rs.txt`, and a design that fails the judge or its speed bounds
    /// at the start is skipped.
    Resynth {
        #[arg(required = true, num_args = 1..)]
        designs: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 2)]
        min_size: usize,
        #[arg(long, default_value_t = 4)]
        max_size: usize,
        /// Circuit blocks (see synth.rs `palette_states`); `trapdoor` (an iron trapdoor: a
        /// non-conducting power indicator an observer can watch) shortened six doors on the
        /// exotic track.
        #[arg(long, default_value = "block,dust,dot,rep,obs,torch,trapdoor")]
        palette: String,
        #[arg(long, default_value = "block")]
        floors: String,
        /// The search box is the group's bounding box grown by this.
        #[arg(long, default_value_t = 1)]
        margin: i32,
        /// Skip groups with more candidate circuits than this.
        #[arg(long, default_value_t = 300_000)]
        max_cands: usize,
        #[arg(long)]
        loose: bool,
        /// Only groups inside this box, "x0,y0,z0:x1,y1,z1" (inclusive).
        #[arg(long)]
        only: Option<String>,
        #[arg(long, default_value_t = 24)]
        workers: usize,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long, default_value_t = 2)]
        screen_offsets: usize,
        /// Settle cap while screening (the door settles in < 60 ticks; clocks hit the cap).
        #[arg(long, default_value_t = 100)]
        screen_ticks: u32,
        /// Speed bounds (speed boards): reject doors that open later than this many ticks.
        #[arg(long)]
        max_open: Option<u32>,
        /// Speed bounds: reject doors that close later than this many ticks.
        #[arg(long)]
        max_close: Option<u32>,
        /// After an improvement, search every group again (default: only the groups whose
        /// neighborhood, the blocks within margin + 3, changed).
        #[arg(long)]
        global_retry: bool,
        /// Keep only replacement circuits using at least one of these (lamp, bulb, note,
        /// target or any block-state substring), comma list: sweeps new palette entries on a
        /// design the old palette already exhausted.
        #[arg(long, default_value = "")]
        require_any: String,
        /// Judge every candidate as a tile module (tile.rs; see `door verify --tile`).
        #[arg(long)]
        tile: Option<String>,
        /// Accept only replacements at least this seamless (quart, semi, full, supercov, super,
        /// optionally `+hall`; `door properties`' definition, one recorded cycle at the design's
        /// own position).
        #[arg(long)]
        seamless: Option<String>,
        /// With --seamless, repair mode: while the design is below the tier, cut only groups
        /// through an offending cell (single blocks too) and let each grow back with up to this
        /// many MORE blocks than it had; once it reaches the tier, the usual shrinking search.
        #[arg(long, default_value_t = 0)]
        extra: usize,
        /// Accept replacements that are not rotation-safe. By default a rotation-safe start
        /// design only accepts rotation-safe replacements (the boards rank rotation-safe doors).
        #[arg(long)]
        any_rotation: bool,
        /// Record eligibility: accept only designs that also work with repeater input (see
        /// `door minimize --repeater-input`).
        #[arg(long)]
        repeater_input: bool,
        /// --repeater-input plus the hallway check, footsteps (sculk) and QUART SEAMLESS or better.
        #[arg(long)]
        eligible: bool,
        /// Shape limits on every accepted replacement, measured on the swept shape (a preset or
        /// key<=n terms, e.g. `thick<=1,volume<=81`; see `door shapes`).
        #[arg(long)]
        shape: Option<String>,
        /// Compaction (the volume boards): keep every replacement circuit inside this box,
        /// "x0,y0,z0:x1,y1,z1" (inclusive); while removable blocks lie outside it, cut only
        /// groups through them and let each grow back inside the box with up to --extra more
        /// blocks than it had (every accepted replacement leaves fewer blocks outside). Pistons,
        /// the lever and its support are never moved: they must already lie inside.
        #[arg(long)]
        fit: Option<String>,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::Route { design, start, goal, allow, swept_script, swept_design, patch } => {
            let d = load_design(&design)?;
            let cells = |s: &str| -> Vec<[i32; 3]> {
                s.split(';').filter(|x| !x.trim().is_empty()).map(|c| {
                    let v: Vec<i32> = c.split(',').map(|n| n.trim().parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                }).collect()
            };
            let spec = crate::DoorSpec::from_design(&d)?;
            let mut w = Worker::spawn(&cfg)?;
            let mut reserved = match (&swept_script, &swept_design) {
                (Some(s), _) => crate::drive::swept(&mut w, &std::fs::read_to_string(s)?)?,
                (None, Some(sd)) => {
                    let sd = load_design(sd)?;
                    let sspec = crate::DoorSpec::from_design(&sd)?;
                    crate::route::swept_moving_cells(&mut w, &sd, &sspec)?
                }
                (None, None) => crate::route::swept_cells(&mut w, &d, &spec)?,
            };
            reserved.retain(|p| !d.blocks.contains_key(p));
            reserved.extend(crate::route::keep_clear(&spec));
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
        DoorCmd::Place { design, groups, out, workers, want, extra_offsets } => {
            let d = load_design(&design)?;
            let text = std::fs::read_to_string(&groups)?;
            let gs = crate::place::parse_groups(&text)?;
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })?;
            let t = std::time::Instant::now();
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let offsets = crate::search::judge_offsets(extra_offsets);
            let opts = crate::place::PlaceOptions {
                screen: VerifyOptions { cycles: 1, offsets: offsets.iter().take(1).copied().collect(), ..Default::default() },
                judge: VerifyOptions { offsets, ..Default::default() },
                want,
            };
            let found = crate::place::place(&pool, &d, &gs, &opts, &out, &log)?;
            println!("{} working combinations", found.len());
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
        DoorCmd::Route2 {
            design,
            out,
            max_rip,
            rip,
            need,
            taps,
            palette,
            tolerance,
            beam,
            hits,
            max_states,
            margin,
            no_watch,
            no_check,
            verbose,
            model_only,
            extra_blocks,
            pairs,
            polish,
            polish_secs,
            polish_max,
            archive,
            search,
        } => {
            use crate::route2;
            let d = load_design(&design)?;
            let pool = search.pool(version)?;
            let t = std::time::Instant::now();
            let sopts = search.options()?;
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let groups = rip.split('|').map(str::trim).filter(|g| !g.is_empty()).map(crate::synth::parse_cells).collect::<Result<Vec<_>>>()?;
            let o = route2::RipOptions {
                max_rip,
                route: route2::RouteOptions {
                    palette: route2::palette(&palette)?,
                    tolerance,
                    beam,
                    hits,
                    max_states,
                    margin,
                    taps: crate::synth::parse_cells(&taps)?,
                    watch: !no_watch,
                    partial: false,
                    order_limit: 0,
                    threads: 1,
                    astar: false,
                },
                no_check,
                rip: groups,
                verbose: verbose || model_only,
                extra_blocks: if polish { extra_blocks.max(1) } else { extra_blocks },
                pairs,
                polish,
                polish_secs,
                polish_max,
            };
            println!("{} workers; judge {} positions", pool.len(), sopts.judge.offsets.len());
            if model_only {
                let meas = route2::measure(&mut pool.worker(0), &d)?;
                route2::model_report(&d, &meas, &log);
                return Ok(());
            }
            if !need.is_empty() {
                let meas = route2::measure(&mut pool.worker(0), &d)?;
                let mut needs = Vec::new();
                for item in need.split(';').map(str::trim).filter(|x| !x.is_empty()) {
                    let (c, w) = item.split_once('=').ok_or_else(|| anyhow::anyhow!("expected x,y,z=WAVE, got {item:?}"))?;
                    let p = crate::synth::parse_cells(c)?.first().copied().ok_or_else(|| anyhow::anyhow!("no cell in {item:?}"))?;
                    needs.push((p, route2::Wave::parse(w)?));
                }
                let found = route2::route_new(&pool, &d, &meas, &needs, &o, &sopts, &log)?;
                match found.first() {
                    Some((best, desc)) => {
                        best.save(&out)?;
                        println!("WORKS: {} blocks: {desc}; written to {}", crate::block_count(best), out.display());
                    }
                    None => println!("nothing found"),
                }
                return Ok(());
            }
            let source = design.display().to_string();
            let judge_n = sopts.judge.offsets.len();
            let mut on_improve = |best: &mctas::design::Design, desc: &str| -> Result<()> {
                best.save(&out)?;
                log(&format!("written to {}: {} blocks ({desc})", out.display(), crate::block_count(best)));
                if archive {
                    let note = format!("verified WORKS: {} blocks (judge: {judge_n} positions x 3 cycles); door route2 from {source}", crate::block_count(best));
                    if let Some(p) = crate::archive::save(&out, best, crate::block_count(best), &note, None)? {
                        log(&format!("archived {}", p.display()));
                    }
                }
                Ok(())
            };
            let best = route2::rip_and_reroute(&pool, &d, &o, &sopts, &log, &mut on_improve)?;
            println!("{} -> {} blocks", crate::block_count(&d), crate::block_count(&best));
        }
        DoorCmd::Regrow { design, out, size, margin, states, only, want, any, require_any, search } => {
            let d = load_design(&design)?;
            let pos = |s: &str| -> Result<mctas::sim::Pos> {
                let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "expected x,y,z, got {s:?}");
                Ok([v[0], v[1], v[2]])
            };
            let only = match only {
                Some(s) => {
                    let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--only wants x0,y0,z0:x1,y1,z1"))?;
                    let (a, b) = (pos(a)?, pos(b)?);
                    Some(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
                }
                None => None,
            };
            let pool = search.pool(version)?;
            let t = std::time::Instant::now();
            let opts = search.options()?;
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let st = crate::search::grow_states(&states)?;
            let req = crate::synth::require_any_names(&require_any);
            let found = crate::search::regrow(&pool, &d, size, margin, &st, only, &opts, want, any, &req, &log)?;
            for (fd, desc) in &found {
                println!("WORKS: {} blocks: {desc}", crate::block_count(fd));
            }
            match found.first() {
                Some((best, _)) => {
                    let mut w = pool.worker(0);
                    let canon = crate::canonicalize(&mut w, best)?;
                    canon.save(&out)?;
                    println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&canon), out.display());
                }
                None => println!("nothing found"),
            }
        }
        DoorCmd::Grow { design, out, lo, hi, states, k, want, then_search, seed_minutes, seed_from, glue, changed, search } => {
            let d = load_design(&design)?;
            let pos = |s: &str| -> Result<mctas::sim::Pos> {
                let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "expected x,y,z, got {s:?}");
                Ok([v[0], v[1], v[2]])
            };
            let (a, b) = (pos(&lo)?, pos(&hi)?);
            let g = crate::search::GrowOptions {
                lo: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
                hi: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
                states: crate::search::grow_states(&states)?,
                k,
                want,
                glue,
            };
            let pool = search.pool(version)?;
            let t = std::time::Instant::now();
            let opts = search.options()?;
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let found = crate::search::grow(&pool, &d, &g, &opts, &log)?;
            for (fd, desc) in &found {
                println!("WORKS: {} blocks: {desc}", crate::block_count(fd));
            }
            if then_search {
                let base_n = crate::block_count(&d);
                let mut best: Option<(Design, String)> = None;
                let screen = opts.screen.clone();
                // Timing signature: every cycle's open/close ticks AND settle ticks (a rider that
                // fires extra often only shows in the settle time).
                let sig = |r: &crate::Report| -> Vec<u32> {
                    r.cycles.iter().flat_map(|c| [c.open_ticks, c.open_settle, c.close_ticks, c.close_settle]).collect()
                };
                let base_ticks = if changed {
                    let mut w = pool.worker(0);
                    Some(sig(&crate::verify(&mut w, &d, &screen)?))
                } else {
                    None
                };
                // Seeds whose screen timings differ from the start's (all seeds without --changed).
                let keep: Vec<bool> = match base_ticks {
                    Some(bt) => {
                        let items: Vec<Design> = found.iter().map(|(fd, _)| fd.clone()).collect();
                        let screen = &screen;
                        let bt = &bt;
                        pool.map(items, |w, _, fd| Ok(crate::verify(w, &fd, screen).map(|r| sig(&r) != *bt).unwrap_or(true)))
                            .into_iter()
                            .map(|r| r.unwrap_or(true))
                            .collect()
                    }
                    None => vec![true; found.len()],
                };
                log(&format!("{} of {} seeds kept", keep.iter().filter(|k| **k).count(), found.len()));
                for (i, (fd, desc)) in found.iter().enumerate() {
                    if i + 1 < seed_from || !keep[i] {
                        continue;
                    }
                    log(&format!("seed {}/{}: {desc}", i + 1, found.len()));
                    let mut sopts = search.options()?;
                    if seed_minutes > 0.0 {
                        sopts.deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(seed_minutes * 60.0));
                    }
                    let quiet = |_: &str| {};
                    let mut on_improve = |nd: &Design, m: &str| -> Result<()> {
                        log(&format!("    {} blocks: {m}", crate::block_count(nd)));
                        Ok(())
                    };
                    let res = match crate::search::search(&pool, fd, &sopts, &quiet, &mut on_improve) {
                        Ok(r) => r,
                        Err(e) => {
                            log(&format!("    search failed: {e:#}"));
                            continue;
                        }
                    };
                    let n = crate::block_count(&res);
                    if n < base_n && best.as_ref().is_none_or(|(b, _)| n < crate::block_count(b)) {
                        let mut w = pool.worker(0);
                        let canon = crate::canonicalize(&mut w, &res)?;
                        canon.save(&out)?;
                        log(&format!("  BELOW START: {n} blocks from seed {desc}, written to {}", out.display()));
                        best = Some((res, desc.clone()));
                    }
                }
                if best.is_none() {
                    println!("no seed searched below {base_n} blocks");
                }
                return Ok(());
            }
            match found.first() {
                Some((best, _)) => {
                    let mut w = pool.worker(0);
                    let canon = crate::canonicalize(&mut w, best)?;
                    canon.save(&out)?;
                    println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&canon), out.display());
                }
                None => println!("nothing found"),
            }
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
        DoorCmd::Synth {
            context,
            lo,
            hi,
            palette,
            floors,
            k,
            min_k,
            from,
            watch,
            to,
            clear,
            forbid,
            require,
            limit,
            require_any,
            spec,
            spec_offsets,
            loose,
            count,
            all_sizes,
            want,
            out_dir,
            workers,
            extra_offsets,
            screen_offsets,
            screen_ticks,
            max_cands,
            max_open,
            max_close,
            tile,
        } => {
            use crate::synth::*;
            let tile = tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
            let mut base = load_design(&context)?;
            for p in parse_cells(&clear)? {
                anyhow::ensure!(base.blocks.remove(&p).is_some(), "--clear {} is already air", fmt_pos(p));
            }
            let cell = |s: &str| -> Result<mctas::sim::Pos> {
                let v = parse_cells(s)?;
                anyhow::ensure!(v.len() == 1, "expected one x,y,z, got {s:?}");
                Ok(v[0])
            };
            let (a, b) = (cell(&lo)?, cell(&hi)?);
            let o = SynthOptions {
                lo: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
                hi: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
                palette: palette_states(&palette)?,
                floors: palette_states(&floors)?,
                k,
                min_k,
                sources: parse_cells(&from)?,
                watch: parse_cells(&watch)?,
                sinks: parse_cells(&to)?,
                forbid: parse_cells(&forbid)?.into_iter().collect(),
                require: parse_counts(&require)?,
                limit: parse_counts(&limit)?,
                require_any: require_any_names(&require_any),
                loose,
                max_cands,
            };
            let en = Gen::new(&base, &o)?;
            let t = std::time::Instant::now();
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            log(&format!("{} atoms ({} palette states, {} floors) in the box", en.atoms.len(), o.palette.len(), o.floors.len()));
            // Enumerate before starting the JVMs (they would sit idle meanwhile).
            let buckets = en.enumerate(&log)?;
            if count {
                return Ok(());
            }
            let offsets = crate::search::judge_offsets(extra_offsets);
            let target = match &spec {
                Some(path) => Target::Spec {
                    spec: Spec::parse(&std::fs::read_to_string(path)?)?,
                    offsets: offsets.iter().take(spec_offsets.max(1)).copied().collect(),
                },
                None => Target::Door {
                    screen: VerifyOptions {
                        cycles: 1,
                        max_ticks: screen_ticks,
                        offsets: offsets.iter().take(screen_offsets.max(1)).copied().collect(),
                        keep_frames: false,
                        empty_walkway: true,
                        tile: tile.clone(),
                    },
                    judge: VerifyOptions { offsets: offsets.clone(), tile: tile.clone(), ..Default::default() },
                    max_open,
                    max_close,
                    rotations: false,
                    seamless: None,
                    repeater_input: false,
                    eligible: false,
                    shape: None,
                },
            };
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1536, ..Default::default() })?;
            log(&format!("{} workers", pool.len()));
            let found = synth(&pool, &base, &en, &buckets, &target, min_k, all_sizes, want, &log)?;
            if found.is_empty() {
                println!("nothing found");
            }
            for (i, (set, d, msg)) in found.iter().enumerate() {
                println!("FOUND {} blocks: {} [{}] -- {msg}", set.len(), en.describe(set), en.kinds(set));
                if let Some(dir) = &out_dir {
                    std::fs::create_dir_all(dir)?;
                    let mut w = pool.worker(0);
                    let canon = match crate::canonicalize(&mut w, d) {
                        Ok(c) if spec.is_some() || crate::verify(&mut w, &c, &VerifyOptions::default()).is_ok() => c,
                        _ => d.clone(),
                    };
                    let path = dir.join(format!("synth-{}-{i}.txt", set.len()));
                    canon.save(&path)?;
                    println!("  written to {}", path.display());
                }
            }
        }
        DoorCmd::Resynth { designs, out, min_size, max_size, palette, floors, margin, max_cands, loose, only, workers, extra_offsets, screen_offsets, screen_ticks, max_open, max_close, global_retry, require_any, tile, any_rotation, seamless, extra, repeater_input, eligible, shape, fit } => {
            let shape = shape.as_deref().map(crate::ShapeLimits::parse).transpose()?;
            use crate::synth::*;
            let seamless = seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?;
            let tile = tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
            let parse_box = |flag: &str, s: &str| -> Result<(mctas::sim::Pos, mctas::sim::Pos)> {
                let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--{flag} wants x0,y0,z0:x1,y1,z1"))?;
                let a = parse_cells(a)?.first().copied().ok_or_else(|| anyhow::anyhow!("bad --{flag} corner {a:?}"))?;
                let b = parse_cells(b)?.first().copied().ok_or_else(|| anyhow::anyhow!("bad --{flag} corner {b:?}"))?;
                Ok(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
            };
            let only = only.as_deref().map(|s| parse_box("only", s)).transpose()?;
            let fit = fit.as_deref().map(|s| parse_box("fit", s)).transpose()?;
            let sweep = designs.len() > 1;
            if sweep {
                std::fs::create_dir_all(&out)?;
            }
            let offsets = crate::search::judge_offsets(extra_offsets);
            let judge = VerifyOptions { offsets: offsets.clone(), tile: tile.clone(), ..Default::default() };
            let screen = VerifyOptions { cycles: 1, max_ticks: screen_ticks, offsets: offsets.iter().take(screen_offsets.max(1)).copied().collect(), keep_frames: false, empty_walkway: true, tile };
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1536, ..Default::default() })?;
            let t = std::time::Instant::now();
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let mut summary: Vec<String> = Vec::new();
            for design in &designs {
                let d = load_design(design)?;
                let target = if sweep { out.join(format!("{}-rs.txt", design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "design".into()))) } else { out.clone() };
                if sweep {
                    log(&format!("=== {} ({} blocks)", design.display(), crate::block_count(&d)));
                }
                let mut o = ResynthOptions {
                    min_size,
                    max_size,
                    palette: palette_states(&palette)?,
                    floors: palette_states(&floors)?,
                    margin,
                    max_cands,
                    loose,
                    max_open,
                    max_close,
                    save_to: Some(target.clone()),
                    global_retry,
                    require_any: require_any_names(&require_any),
                    only,
                    keep_rotations: false,
                    seamless,
                    extra,
                    repeater_input: repeater_input || eligible,
                    eligible,
                    shape,
                    fit,
                };
                let start = {
                    let mut w = pool.worker(0);
                    match crate::verify(&mut w, &d, &judge) {
                        Ok(r) => {
                            let (o, c) = crate::search::worst_ticks(&r);
                            log(&format!("start: {} blocks, open {o} / close {c}", crate::block_count(&d)));
                            if max_open.is_none_or(|m| o <= m) && max_close.is_none_or(|m| c <= m) {
                                if repeater_input || eligible {
                                    // The record-eligibility filter must hold for the start design too.
                                    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
                                    let r = crate::search::eligibility(&mut w, sb, &d, true, eligible, &judge.offsets);
                                    let _ = w.drop_sandbox(sb);
                                    match r {
                                        Ok(()) => {
                                            log("the start design passes the eligibility filter (repeater input everywhere)");
                                            Ok(())
                                        }
                                        Err(e) => Err(format!("the start design fails the eligibility filter: {e:#}")),
                                    }
                                } else {
                                    Ok(())
                                }
                            } else {
                                Err(format!("the start design misses the speed bounds (open {o} / close {c})"))
                            }
                        }
                        Err(e) => Err(format!("the start design does not pass the judge: {e:#}")),
                    }
                };
                if let Err(e) = start {
                    anyhow::ensure!(sweep, "{e}");
                    log(&format!("skipped: {e}"));
                    summary.push(format!("{}: skipped ({e})", design.display()));
                    continue;
                }
                if !any_rotation {
                    let mut w = pool.worker(0);
                    match crate::rotation_check(&mut w, &d, &judge)? {
                        None => {
                            o.keep_rotations = true;
                            log("the start design is rotation-safe: replacements must stay rotation-safe (--any-rotation to allow others)");
                        }
                        Some((turns, _)) => log(&format!("the start design is NOT rotation-safe (turned {} degrees): rotations not checked", turns as u32 * 90)),
                    }
                }
                let best = resynth(&pool, &d, &o, &judge, &screen, &log)?;
                if sweep && crate::block_count(&best) >= crate::block_count(&d) {
                    summary.push(format!("{}: {} blocks, nothing smaller", design.display(), crate::block_count(&d)));
                    continue;
                }
                // Keep the canonical rest state only if it still passes the judge (a design written
                // with stale redstone states can settle into another machine).
                let canon = {
                    let mut w = pool.worker(0);
                    match crate::canonicalize(&mut w, &best) {
                        Ok(c) if crate::verify(&mut w, &c, &judge).is_ok() => c,
                        _ => match settled_design(&mut w, &best) {
                            Ok(s) if crate::verify(&mut w, &s, &judge).is_ok() => {
                                println!("the design moves while it settles: writing its settled state");
                                s
                            }
                            _ => {
                                println!("canonical and settled forms fail the judge: writing the design as found");
                                best.clone()
                            }
                        },
                    }
                };
                canon.save(&target)?;
                println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&canon), target.display());
                summary.push(format!("{}: {} -> {} blocks, {}", design.display(), crate::block_count(&d), crate::block_count(&canon), target.display()));
            }
            if sweep {
                println!("sweep summary:");
                for s in &summary {
                    println!("  {s}");
                }
            }
        }
    }
    Ok(())
}
