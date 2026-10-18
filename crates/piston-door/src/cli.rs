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
    /// Also accept candidates that add a sculk sensor to a solid design (category sculk): put
    /// `minecraft:sculk_sensor` (or `calibrated_sculk_sensor[facing=..]`) in the states.
    #[arg(long)]
    pub to_sculk: bool,
    /// Shape boards: only accept designs within these shape limits relative to the door: a
    /// preset (wall, wall2, flat, ground, no-above, narrow1, narrow2, wall-ground) or a comma list
    /// of key<=n (front behind thick below above left right side volume).
    #[arg(long)]
    pub shape: Option<String>,
    /// Judge every candidate as a tile module (tile.rs): wide:N, period:P:N, stack:N,
    /// vperiod:Q:N or step:dx,dy,dz:N (copies of the design, one lever, every opening checked).
    #[arg(long)]
    pub tile: Option<String>,
    /// Accept only rotation-safe designs (the judge also passes turned 90, 180 and 270 degrees;
    /// the boards rank rotation-safe designs). The start design must be rotation-safe too.
    #[arg(long)]
    pub rotation_safe: bool,
    /// Accept only designs at least this seamless (quart, semi, full, supercov, super, optionally
    /// `+hall`: the coordinator's hallway must build too; `door properties`' definition, one
    /// recorded cycle at the design's own position).
    #[arg(long)]
    pub seamless: Option<String>,
    /// Also accept candidates that add entities to a solid design (category entity, the "Entities
    /// allowed" board): put `cart`, `chestcart` or `stand` in `door grow --states`.
    #[arg(long)]
    pub to_entity: bool,
    /// Accept candidates that are not rotation-safe. By default a rotation-safe start design
    /// only accepts rotation-safe candidates (the boards rank rotation-safe doors).
    #[arg(long)]
    pub any_rotation: bool,
    /// With --tile: a shared end piece (see `door verify --tile-extra`).
    #[arg(long)]
    pub tile_extra: Option<PathBuf>,
    /// Record eligibility (Squid regulation 3.1.3): accept only designs that also work with
    /// repeater input (the lever's support block powered by a delay-1 repeater, both edges; the
    /// props-dyn check) at every judge position. The start design must pass it too.
    #[arg(long)]
    pub repeater_input: bool,
    /// --repeater-input plus props-dyn's hallway check, footsteps (sculk designs) and QUART
    /// SEAMLESS or better: every record-eligibility check a search can keep.
    #[arg(long)]
    pub eligible: bool,
}

impl SearchArgs {
    pub fn options(&self) -> Result<crate::search::SearchOptions> {
        let offsets = crate::search::judge_offsets(self.extra_offsets);
        let mut tile = self.tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
        if let (Some(t), Some(p)) = (tile.as_mut(), &self.tile_extra) {
            t.extra = Some(load_design(p)?);
        }
        Ok(crate::search::SearchOptions {
            families: crate::search::parse_families(&self.moves)?,
            screen: VerifyOptions {
                cycles: self.screen_cycles,
                max_ticks: self.screen_ticks,
                offsets: offsets.iter().take(self.screen_offsets.max(1)).copied().collect(),
                keep_frames: false,
                empty_walkway: true,
                tile: tile.clone(),
            },
            judge: VerifyOptions { offsets, tile, ..Default::default() },
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
            to_sculk: self.to_sculk,
            shape: self.shape.as_deref().map(crate::ShapeLimits::parse).transpose()?,
            rotation_safe: self.rotation_safe,
            seamless: self.seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?,
            to_entity: self.to_entity,
            keep_rotations: !self.any_rotation,
            repeater_input: self.repeater_input || self.eligible,
            eligible: self.eligible,
        })
    }

    pub fn pool(&self, version: &str) -> Result<mctas::sim::Pool> {
        mctas::sim::Pool::spawn(self.workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })
    }
}
#[derive(Subcommand)]
pub enum DoorCmd {
    /// Record a video of every design in a folder (several at once), upload each, then write the
    /// folder's catalog (README.md, see `door catalog`).
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
        /// Look of the videos: `card` (title card, door camera swinging around the door,
        /// block-count label), `clean` (the same camera, no card or label) or `classic` (the
        /// original still camera framing the whole build).
        #[arg(long, default_value = "card")]
        style: String,
        /// Orbit amplitude of the door camera in degrees each way (0 = still).
        #[arg(long, default_value_t = 20.0)]
        orbit: f64,
        /// Re-record videos that already exist.
        #[arg(long)]
        redo: bool,
        /// Re-record existing videos of doors still moving when the clip used to stop (40 ticks
        /// after each flip); videos now always play until the door has settled.
        #[arg(long)]
        redo_cut: bool,
        /// Only list the videos that would be recorded (records and writes nothing).
        #[arg(long)]
        dry_run: bool,
    },
    /// Write the archive catalog (archive/README.md): every archived design with its blocks,
    /// boards, open/close ticks, rotations, buildability (`door buildable`'s scripts), video link
    /// and how it works (archive/captions.tsv, archive/records.tsv). Records nothing; verdicts
    /// come from the verdict cache. `door video-all` writes it too.
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
    /// Draw the title card (and the corner label, as a transparent PNG) of a design's video,
    /// without recording anything.
    Card {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Also write the label overlay here.
        #[arg(long)]
        label: Option<PathBuf>,
    },
    /// The best design of each board (block-count boards, speed boards) as a looping GIF (door
    /// camera, real client), uploaded to the release, plus the night's timeline chart. No page of
    /// its own: `door leaderboard` shows each board leader's GIF.
    Gallery {
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        archive: PathBuf,
        /// Folder for the clips and GIFs (rendered when missing).
        #[arg(long)]
        videos: PathBuf,
        /// GitHub release tag to upload the GIFs and clips to.
        #[arg(long)]
        upload: Option<String>,
        /// Client renders at once.
        #[arg(long, default_value_t = 2)]
        jobs: usize,
        /// Worker JVMs to verify archived designs missing from the verdict cache.
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Camera orbit in degrees each way (0 = still: an orbiting GIF is ~10x larger).
        #[arg(long, default_value_t = 0.0)]
        orbit: f64,
        #[arg(long, default_value_t = 640)]
        gif_width: u32,
        #[arg(long, default_value_t = 15)]
        gif_fps: u32,
        /// GIF playback speed (clips are recorded at 10 ticks/s: 2 = game speed).
        #[arg(long, default_value_t = 1.5)]
        speed: f64,
        /// Re-render clips and GIFs that exist.
        #[arg(long)]
        redo: bool,
    },
    /// A board's record progression (solid: archive/records.tsv, plus a newer record) as one video:
    /// every design filmed by the same camera, labeled with its block count and how it got there.
    Montage {
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        archive: PathBuf,
        /// Folder for the clips and segments.
        #[arg(long)]
        videos: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// GitHub release tag to upload the video and its poster to (as <board>-progression.mp4/.jpg).
        #[arg(long)]
        upload: Option<String>,
        #[arg(long, default_value_t = 2)]
        jobs: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Playback speed of the clips (2 = game speed).
        #[arg(long, default_value_t = 2.0)]
        speed: f64,
        #[arg(long)]
        redo: bool,
        /// solid (archive/records.tsv), slime-free, flat, sculk, sculk-unrotated or gravity (records
        /// in archive order).
        #[arg(long, default_value = "solid")]
        board: String,
    },
    /// The Pareto frontier over (open ticks, close ticks, blocks): every working design on HEAD
    /// and the explore/* branches (verdicts from the leaderboard cache, uncached ones verified),
    /// then each --check design verified with the judge and placed against it: on the frontier,
    /// or the doors that dominate it (see leaderboard.rs `frontier_report`).
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
    /// Wire a mechanism from its ideal-input schedule (mechwire.rs): replay it with ideal inputs,
    /// route every piston cell's wave from a lever at each lever spot with route2's router, rank
    /// mechanism + lever + wiring by blocks, judge the best. --retime re-times a mech-synth
    /// schedule to chain ticks first (every working assignment is wired).
    MechWire {
        /// Ideal schedules (`door ideal` format), or one mech-synth catalog with --catalog-ids.
        #[arg(required = true, num_args = 1..)]
        schedules: Vec<PathBuf>,
        /// Schedule variables "name=value".
        #[arg(long)]
        set: Vec<String>,
        /// Lever spots "x,y,z@ax,ay,az;..." (lever cell @ the cell it hangs on). Default: every
        /// spot within --lever-radius of the --lever-near receivers that need power first.
        #[arg(long, default_value = "")]
        levers: String,
        #[arg(long, default_value_t = 2)]
        lever_radius: i32,
        #[arg(long, default_value_t = 1)]
        lever_near: usize,
        /// Most wiring blocks per candidate (lever and its block excluded).
        #[arg(long, default_value_t = 24)]
        budget: u32,
        #[arg(long, default_value = "dust,rep,obs,block,gt")]
        palette: String,
        #[arg(long, default_value_t = 0)]
        tolerance: i32,
        #[arg(long, default_value_t = 6)]
        beam: usize,
        #[arg(long, default_value_t = 4)]
        hits: usize,
        #[arg(long, default_value_t = 60000)]
        max_states: usize,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Do not place observers watching cells the machine changes.
        #[arg(long)]
        no_watch: bool,
        /// Connect receivers that need pulses from several sources in several steps.
        #[arg(long)]
        partial: bool,
        /// Per partial solution, route only the first N receivers left (earliest power first;
        /// 0: all, a beam over every order).
        #[arg(long, default_value_t = 2)]
        order_limit: usize,
        /// Free receivers "x,y,z;...": accept a single 2 gt pulse at any tick of --free-range.
        #[arg(long, default_value = "")]
        free: String,
        #[arg(long, default_value = "2..40")]
        free_range: String,
        /// Routing threads (lever spots in parallel).
        #[arg(long, default_value_t = 24)]
        threads: usize,
        /// Re-time the schedule's steps (step 0 at one of --first, then gaps from --gaps).
        #[arg(long)]
        retime: bool,
        #[arg(long, default_value = "1")]
        first: String,
        #[arg(long, default_value = "2,4,6")]
        gaps: String,
        #[arg(long, default_value_t = 50)]
        max_tick: u32,
        /// Wire at most this many working retimings (fastest open + close first).
        #[arg(long, default_value_t = 12)]
        retime_max: usize,
        /// Judge the best N candidates (0: none).
        #[arg(long, default_value_t = 12)]
        verify: usize,
        /// Screen every lever spot cheaply (each receiver routed alone) and fully route only the
        /// best N (0: every spot fully).
        #[arg(long, default_value_t = 6)]
        screen_top: usize,
        /// Print what the router finds for each receiver alone (per lever spot).
        #[arg(long)]
        debug: bool,
        /// The positional file is a mech-synth catalog: wire these entries ("355,356").
        #[arg(long, default_value = "")]
        catalog_ids: String,
        /// The catalog's base design.
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/mech-synth/bases/sidesM.txt")]
        catalog_base: PathBuf,
        /// Compress the schedule's steps: step 0 at --first, each gap shrunk to the smallest of
        /// --gaps that still works with ideal inputs.
        #[arg(long)]
        compress: bool,
        /// Single-pulse receivers whose pulse may move by at least this many ticks (ideal-input
        /// sweep) become free: any 2 gt pulse in that window (0: off).
        #[arg(long, default_value_t = 0)]
        auto_free: i32,
        /// Free receivers also accept 3 pulses per edge (a pulse-driven piston toggles per
        /// pulse: one net move, timed by the last pulse).
        #[arg(long)]
        odd: bool,
        /// With --auto-free: every receiver also accepts any combination of its events' timing
        /// windows (ideal-input sweep, others fixed).
        #[arg(long)]
        timing: bool,
        /// Pulse pairs on one cell become one level window (a repeater instead of two pulses).
        #[arg(long)]
        windows: bool,
        /// Polish each schedule's verified design for this many seconds (route2 rip-up, then
        /// the local search with every move family); 0: off.
        #[arg(long, default_value_t = 0)]
        polish_secs: u64,
        /// Re-time every step to tick first + gap * k before anything else (0: keep).
        #[arg(long, default_value_t = 0)]
        uniform_gap: u32,
        /// Write candidates and verified designs here.
        #[arg(long)]
        out_dir: Option<PathBuf>,
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
    /// Mechanism synthesis (mechsynth.rs): enumerate up to --add blocks (pistons, slime...) on a
    /// base mechanism, solve each in a macro piston model (open + close paths, symmetric
    /// schedules), rank by (blocks, control events), write ideal-input designs + schedules and
    /// replay the best in the simulator (`--verify N`).
    MechSynth {
        /// Base design (its mechanism blocks; wiring is dropped). Door port = min corner.
        base: PathBuf,
        #[arg(long, default_value_t = 3)]
        add: usize,
        /// Candidate box corners "x,y,z" relative to the door port.
        #[arg(long, default_value = "-4,-4,0")]
        lo: String,
        #[arg(long, default_value = "6,6,0")]
        hi: String,
        /// Candidate kinds: slime,honey,obsidian,gt,stone,sticky,normal.
        #[arg(long, default_value = "slime,sticky")]
        kinds: String,
        /// Also try pistons extended at rest.
        #[arg(long, default_value_t = true)]
        ext: bool,
        /// State-graph cap per mechanism.
        #[arg(long, default_value_t = 3000)]
        cap: usize,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        /// Skip mechanisms with more blocks than this (door included).
        #[arg(long, default_value_t = 25)]
        max_blocks: usize,
        /// Longest symmetric (same on both edges) sequence to look for.
        #[arg(long, default_value_t = 9)]
        sym_len: usize,
        /// Open states tried per mechanism by the symmetric search.
        #[arg(long, default_value_t = 6)]
        sym_tries: usize,
        /// Write the best N as ideal designs + schedules here.
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 20)]
        top: usize,
        /// Replay the best N in the simulator with ideal inputs (4 + --extra-offsets positions).
        #[arg(long, default_value_t = 0)]
        verify: usize,
        #[arg(long, default_value_t = 4)]
        extra_offsets: u32,
        /// Ticks between macro steps in the ideal schedules.
        #[arg(long, default_value_t = 6)]
        gap: u32,
        #[arg(long, default_value_t = 16)]
        workers: usize,
        /// Only solve the base itself and print its paths.
        #[arg(long)]
        just_base: bool,
        /// Requirement "kinds:x0,y0,z0:x1,y1,z1" (repeatable): some block of these kinds in the box.
        #[arg(long)]
        require: Vec<String>,
        /// Pulse-driven pistons only (no level signals) for pistons retracted at rest.
        #[arg(long)]
        pulses_only: bool,
        /// Split symmetric search: put the edge pistons' pulses at the start, end or both.
        #[arg(long, default_value = "both")]
        edges: String,
        /// Only the split symmetric search (edges block + searched core), not the graph one.
        #[arg(long)]
        split_only: bool,
        /// FULL seamless + hallway in the macro model: nothing ever enters the hallway surface S
        /// off the door plane, no candidate there, the open state holds only door blocks in S.
        #[arg(long)]
        seamless: bool,
        /// A piston standing in the hallway ring S cannot be fired (bottom row: no power cell).
        #[arg(long)]
        no_ring_fire: bool,
        /// Symmetric control by product BFS for the base itself: the open state reached by these
        /// events ("P:x,y,z L:x,y,z ...", pistons named by their rest position), then the
        /// shortest single sequence S with S(rest) = open and S(open) = rest (`--cap` pairs).
        #[arg(long)]
        sym_bfs: Option<String>,
        /// With --sym-bfs: also allow level toggles (default: 2 gt pulses only).
        #[arg(long)]
        sym_levels: bool,
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
        /// `card` (title card, door camera swinging around the door, block-count label),
        /// `clean` (same camera, no card or label) or `classic` (still camera, whole build).
        #[arg(long, default_value = "card")]
        style: String,
        /// Orbit amplitude of the door camera in degrees each way (0 = still).
        #[arg(long, default_value_t = 20.0)]
        orbit: f64,
        /// Frame size, WIDTHxHEIGHT.
        #[arg(long, default_value = "1280x720")]
        size: String,
    },
    /// One open/close cycle tick by tick, still camera, with captions: the lever flips, every
    /// tick after each flip until nothing moves anymore (the state the judge checks), a 3x3 map
    /// of the doorway as the server sees it, and a strip of the whole phase (see tickvideo.rs).
    TickVideo {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Seconds each tick stays on screen.
        #[arg(long, default_value_t = 0.35)]
        secs_per_tick: f64,
        /// Milliseconds the client gets to draw each tick before the screenshot.
        #[arg(long, default_value_t = 500)]
        draw_ms: u64,
        /// Upload to this GitHub release tag of vjeux/minecraft-tas.
        #[arg(long)]
        upload: Option<String>,
        /// Frame size, WIDTHxHEIGHT.
        #[arg(long, default_value = "1280x720")]
        size: String,
    },
    /// One Minecraft world (a save folder for the real game) with every working archived design
    /// in rows by board, smallest first, the block count above each in red concrete and its name
    /// on a floating label; built by the official server (see world.rs).
    World {
        #[arg(long, default_value = "designs/piston-door-3x3/archive")]
        archive: PathBuf,
        /// Folder for the save (`<out>/<name>`) and its `.tar.gz`.
        #[arg(long)]
        out: PathBuf,
        /// The save's folder name, as the game lists it.
        #[arg(long, default_value = "minecraft-tas doors")]
        name: String,
        #[arg(long, default_value_t = 12)]
        workers: usize,
        /// Then screenshot the spawn and the first N designs of each row with the real client
        /// joined to the saved world (0 = none).
        #[arg(long, default_value_t = 0)]
        shots: usize,
        /// Take verdicts from this verdict-cache file instead of judging (e.g.
        /// designs/piston-door-3x3/archive/verdicts-v4.tsv).
        #[arg(long)]
        verdicts: Option<PathBuf>,
        /// Read every design back from the saved region files and fail on any difference.
        #[arg(long)]
        check: bool,
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
    /// Apply an edit script to a design and write the (canonicalized) result. Script lines:
    /// `set x y z <state>` (an extended piston gets its head too), `clear x y z` (a piston
    /// takes its head along), `port <name> x y z`; `#` starts a comment line.
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
    /// Run designs in an UNMODIFIED vanilla dedicated server (official jar, RCON, a bot player
    /// flips the lever) and compare with the simulator tick for tick, with the judge's rules.
    VanillaCheck {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 3)]
        cycles: u32,
        /// Extra pseudo-random positions on top of the default four (the judge uses 28).
        #[arg(long, default_value_t = 4)]
        extra_offsets: u32,
        /// Only the first N positions (quick runs).
        #[arg(long)]
        positions: Option<usize>,
        /// Check at this one position only, "x,y,z" (reproduce a fuzz case or one judge offset).
        #[arg(long)]
        offset: Option<String>,
        #[arg(long, default_value_t = 400)]
        max_ticks: u32,
        /// Print every position, not only divergences and failures.
        #[arg(long)]
        verbose: bool,
        /// Keep the vanilla world directory (path printed at the end).
        #[arg(long)]
        keep_world: bool,
        /// `/tick rate` of the FROZEN server: only how often its idle loop wakes up, i.e. how soon
        /// a `tick step` can start; what a tick does is unchanged (20 = vanilla's default gave
        /// identical frames on every design tried). The real-time cycle always runs at 20.
        #[arg(long, default_value_t = 100)]
        tick_rate: u32,
        /// Skip the real-time cycle (unfrozen 20 tps, at the first position).
        #[arg(long)]
        no_free_run: bool,
        /// Record pending ticks/events after every tick in both worlds; print them for diverging positions.
        #[arg(long)]
        trace: bool,
    },
    /// Replay survival build scripts (`door buildable`'s `*.build.txt`) in an UNMODIFIED vanilla
    /// server: a bot player selects each item, looks the script's way, right-clicks the given face
    /// (temporary dirt placed and broken by clicks too), the lever state as written; every action
    /// is compared with the simulator, the built world with the judge's rest state, then the
    /// judge's cycles run with the bot (vanilla-check's comparison). See vanilla_build.rs.
    VanillaBuild {
        scripts: Vec<String>,
        /// The design (file, `@builtin` or `<git ref>:<path>`); default: the script's `# source:`.
        #[arg(long)]
        design: Option<String>,
        #[arg(long, default_value_t = 3)]
        cycles: u32,
        /// Extra pseudo-random positions on top of the judge's default four.
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        /// Only the first N positions.
        #[arg(long)]
        positions: Option<usize>,
        /// One position only, "x,y,z".
        #[arg(long)]
        offset: Option<String>,
        #[arg(long, default_value_t = 400)]
        max_ticks: u32,
        /// Replay only the first N actions and compare (a minimal repro of a divergence).
        #[arg(long)]
        until_step: Option<usize>,
        /// Print every position, not only failures.
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        keep_world: bool,
        /// `/tick rate` of the frozen server (see vanilla-check).
        #[arg(long, default_value_t = 100)]
        tick_rate: u32,
        /// Build with the game RUNNING at 20 tps (the bot acts while time passes, waiting the
        /// simulator's ticks + 10 after each action); the cycles still run frozen, tick for tick.
        #[arg(long)]
        real_time: bool,
        /// A slower player: N extra idle ticks after every action, in both worlds (stepped).
        #[arg(long, default_value_t = 0)]
        slow: u32,
        /// Keep the bot at its click spot while the world ticks (a player standing by; by
        /// default it waits out of every sculk sensor's range, as the simulator has no player).
        #[arg(long)]
        stay: bool,
    },
    /// Replay gadget drive scripts (designs/gadgets/*.drive) in the UNMODIFIED official server
    /// and compare every tick with the simulator (see vanilla_gadget.rs): the design in its rest
    /// state, `use` by a bot player, `setblock` as the game's command in both worlds, every tick
    /// of every step compared cell by cell, at the judge's 4 positions + N.
    VanillaGadget {
        scripts: Vec<PathBuf>,
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        /// Only the first N positions.
        #[arg(long)]
        positions: Option<usize>,
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        keep_world: bool,
        #[arg(long, default_value_t = 100)]
        tick_rate: u32,
    },
    /// Differential fuzzing: random small contraptions (not doors), settled by the simulator,
    /// run in both worlds and compared tick for tick; diverging ones are saved.
    VanillaFuzz {
        #[arg(long, default_value_t = 50)]
        count: u64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value_t = 1)]
        cycles: u32,
        #[arg(long, default_value_t = 120)]
        max_ticks: u32,
        /// Where diverging designs are written.
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/vanilla-check/fuzz")]
        out_dir: PathBuf,
    },
    /// Can a survival player build it? Search for a build program (placement order with real
    /// player placements and normal updates, the lever state while building, temporary helper
    /// blocks) that ends exactly in the judge's rest state, then run the judge's cycles from the
    /// built state (see buildable.rs). Designs: files, `@builtin`, or `<git ref>:<path>`.
    Buildable {
        designs: Vec<String>,
        /// Search time per design, in seconds.
        #[arg(long, default_value_t = 300)]
        budget: u64,
        /// Worker JVMs.
        #[arg(long, default_value_t = 16)]
        workers: usize,
        /// Allow temporary helper blocks in the search.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        helpers: bool,
        /// Re-check a found program at this many positions (the judge's default four first).
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Write each design's build script / diagnosis here (one file per design).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Append one markdown table row per design here.
        #[arg(long)]
        table: Option<PathBuf>,
        /// Lever state while building: `both` (search both), `off`, `on`.
        #[arg(long, default_value = "both")]
        lever: String,
        #[arg(long)]
        verbose: bool,
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
        DoorCmd::VideoAll { dir, out, jobs, upload, style, orbit, redo, redo_cut, dry_run } => {
            let opts = video_style(&style, orbit)?;
            crate::video::record_all(version, &dir, &out, jobs, upload.as_deref(), &opts, redo, redo_cut, dry_run)?;
        }
        DoorCmd::Catalog { archive, videos, release, workers } => {
            let o = crate::catalog::CatalogOptions { archive: &archive, workers, videos: videos.as_deref(), release: Some(release.as_str()) };
            crate::catalog::write(version, &o)?;
        }
        DoorCmd::Card { design, out, label } => {
            let d = load_design(&design)?;
            let mut facts = crate::video::Facts::of(&d);
            let mut w = Worker::spawn(&cfg)?;
            if let Ok(r) = verify(&mut w, &d, &VerifyOptions { cycles: 1, offsets: vec![[0, 0, 0]], empty_walkway: false, ..Default::default() }) {
                facts.open_ticks = r.cycles.first().map(|c| c.open_ticks);
                facts.close_ticks = r.cycles.first().map(|c| c.close_ticks);
            }
            let look = crate::showcase::Look::load(version)?;
            look.card(1280, 720, &crate::showcase::title_lines(&facts)).save_png(&out)?;
            if let Some(p) = label {
                look.label(1280, 720, &crate::showcase::label_lines(&facts), true, 0.6).save_png(&p)?;
            }
            println!("wrote {}", out.display());
        }
        DoorCmd::Gallery { archive, videos, upload, jobs, workers, orbit, gif_width, gif_fps, speed, redo } => {
            let o = crate::showcase::GalleryOptions { archive, videos, upload, jobs, workers, orbit, gif_width, gif_fps, speed, redo };
            crate::showcase::gallery(version, &o)?;
        }
        DoorCmd::Montage { archive, videos, out, upload, jobs, workers, speed, redo, board } => {
            let o = crate::showcase::MontageOptions { archive, videos, out, upload, jobs, workers, speed, redo, board };
            crate::showcase::montage(version, &o)?;
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
        DoorCmd::MechWire {
            schedules,
            set,
            levers,
            lever_radius,
            lever_near,
            budget,
            palette,
            tolerance,
            beam,
            hits,
            max_states,
            margin,
            no_watch,
            partial,
            order_limit,
            free,
            free_range,
            threads,
            retime,
            first,
            gaps,
            max_tick,
            retime_max,
            verify,
            screen_top,
            debug,
            catalog_ids,
            catalog_base,
            compress,
            auto_free,
            odd,
            timing,
            windows,
            polish_secs,
            uniform_gap,
            out_dir,
            archive,
            search,
        } => {
            use crate::mechwire;
            use crate::route2;
            use anyhow::Context;
            let pool = search.pool(version)?;
            let sopts = search.options()?;
            let t = std::time::Instant::now();
            let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
            let nums = |s: &str| -> Result<Vec<u32>> { s.split(',').map(|x| x.trim().parse::<u32>().map_err(anyhow::Error::from)).collect() };
            let (flo, fhi) = free_range.split_once("..").context("--free-range lo..hi")?;
            let o = mechwire::WireOptions {
                route: route2::RouteOptions {
                    palette: route2::palette(&palette)?,
                    tolerance,
                    beam,
                    hits,
                    max_states,
                    margin,
                    taps: Vec::new(),
                    watch: !no_watch,
                    partial,
                    order_limit,
                    threads: 1,
                    astar: true,
                },
                budget,
                threads,
                free: crate::synth::parse_cells(&free)?.into_iter().map(|c| (c, flo.parse().unwrap_or(2), fhi.parse().unwrap_or(40))).collect(),
                alts: Vec::new(),
                odd,
                forbid: Vec::new(),
                debug,
                screen_top,
            };
            // The source schedules: the files themselves, or catalog entries of the first file.
            let schedule = schedules.first().cloned().context("no schedule file")?;
            let text = std::fs::read_to_string(&schedule).with_context(|| format!("reading {}", schedule.display()))?;
            let mut sources: Vec<(String, crate::ideal::Schedule, std::collections::BTreeMap<String, i64>)> = Vec::new();
            if catalog_ids.is_empty() {
                for path in &schedules {
                    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
                    let s = crate::ideal::Schedule::parse(&text)?;
                    let mut vars = s.vars().clone();
                    for kv in &set {
                        let (k, v) = kv.split_once('=').context("--set name=value")?;
                        vars.insert(k.to_string(), v.parse()?);
                    }
                    let name = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                    sources.push((name, s, vars));
                }
            } else {
                let base = load_design(&catalog_base)?;
                for id in nums(&catalog_ids)? {
                    match mechwire::catalog_schedule(&text, id as usize, &base, 6) {
                        Ok(s) => sources.push((format!("#{id}"), s, Default::default())),
                        Err(e) => log(&format!("#{id}: {e:#}")),
                    }
                }
            }
            // Each source: its schedules are built, then wired at once (results as they come).
            let mut all: Vec<mechwire::Wired> = Vec::new();
            let mut ranking: Vec<(usize, String, usize)> = Vec::new();
            let steps_of = |s: &crate::ideal::Schedule, vars: &std::collections::BTreeMap<String, i64>| -> Result<mechwire::Steps> {
                let st = mechwire::Steps::from_schedule(s, vars)?;
                if !windows {
                    return Ok(st);
                }
                // Each pulse pair alone as a window, kept if the door still works with ideal
                // inputs at the schedule's own timing.
                let mut ok_cells = Vec::new();
                for c in st.pulse_pair_cells() {
                    let w = st.with_windows(&[c]);
                    if !mechwire::working_retimes(&pool, &w, vec![st.ticks.clone()], 0, 2).is_empty() {
                        ok_cells.push(c);
                    }
                }
                log(&format!("  windows: pulse pairs at {:?} become windows", ok_cells.iter().map(|c| route2::fmt_pos(*c)).collect::<Vec<_>>()));
                Ok(st.with_windows(&ok_cells))
            };
            for (name, s, vars) in &sources {
                let mut scheds: Vec<(String, crate::ideal::Schedule, std::collections::BTreeMap<String, i64>)> = Vec::new();
                if retime {
                    let steps = steps_of(s, vars)?;
                    let assigns = steps.assignments(&nums(&first)?, &nums(&gaps)?, max_tick);
                    log(&format!("{name}: {} steps (old ticks {:?}), {} retimings to replay", steps.ticks.len(), steps.ticks, assigns.len()));
                    let mut ok = mechwire::working_retimes(&pool, &steps, assigns, 0, 2);
                    ok.sort_by_key(|(a, (op, cl))| (op + cl, a.clone()));
                    log(&format!("{name}: {} retimings work with ideal inputs", ok.len()));
                    for (a, (op, cl)) in ok.iter().take(retime_max) {
                        log(&format!("  steps at {a:?}: open {op} / close {cl}"));
                        scheds.push((format!("{name} steps {a:?}"), steps.retimed(a), Default::default()));
                    }
                } else if compress {
                    let steps = steps_of(s, vars)?;
                    let f = nums(&first)?.first().copied().unwrap_or(1);
                    match mechwire::compress(&pool, &steps, f, 6, &nums(&gaps)?, &log) {
                        Some(a) => {
                            log(&format!("{name}: {} steps compressed to {a:?}", steps.ticks.len()));
                            scheds.push((format!("{name} steps {a:?}"), steps.retimed(&a), Default::default()));
                        }
                        None => log(&format!("{name}: no working retiming")),
                    }
                } else if uniform_gap > 0 {
                    let steps = steps_of(s, vars)?;
                    let f = nums(&first)?.first().copied().unwrap_or(1);
                    let a: Vec<u32> = (0..steps.ticks.len()).map(|k| f + uniform_gap * k as u32).collect();
                    log(&format!("{name}: {} steps re-timed to {a:?}", steps.ticks.len()));
                    scheds.push((format!("{name} steps {a:?}"), steps.retimed(&a), Default::default()));
                } else {
                    scheds.push((name.clone(), s.clone(), vars.clone()));
                }
            for (label, sc, v) in &scheds {
                let mech = match mechwire::measure_ideal(&mut pool.worker(0), sc, v) {
                    Ok(m) => m,
                    Err(e) => {
                        log(&format!("{label}: cannot measure: {e:#}"));
                        continue;
                    }
                };
                if !mech.meas.idle {
                    log(&format!("{label}: warning: not idle within the recorded ticks"));
                }
                let spots = if levers.is_empty() {
                    mechwire::lever_spots(&mech, &mechwire::early_receivers(&mech, lever_near), lever_radius)?
                } else {
                    levers.split(';').map(str::trim).filter(|x| !x.is_empty()).map(|x| mechwire::LeverSpot::parse(x, &mech)).collect::<Result<Vec<_>>>()?
                };
                log(&format!("{label}: {} mechanism blocks, {} lever spots; wanted waves:", crate::block_count(&mech.bare), spots.len()));
                for l in mech.describe() {
                    log(&format!("    {l}"));
                }
                let mut ow = o.clone();
                let mut mech = mech;
                if auto_free > 0 {
                    let steps = mechwire::Steps::from_schedule(sc, v)?;
                    let at: Vec<u32> = steps.ticks.clone();
                    let shifts: Vec<i32> = (-12..=24).step_by(2).collect();
                    let win = mechwire::event_windows(&pool, &steps, &at, &shifts);
                    for ((c, s), ds) in &win {
                        log(&format!("    event {} at step {s} (tick {}): works shifted by {:?}", route2::fmt_pos(*c), at[*s], ds));
                    }
                    let span = |ds: &Vec<i32>| ds.iter().max().zip(ds.iter().min()).map(|(a, b)| a - b).unwrap_or(-1);
                    let wide: Vec<(mctas::sim::Pos, usize)> = win.iter().filter(|(_, ds)| span(ds) >= auto_free).map(|(e, _)| *e).collect();
                    let noops = mechwire::noop_events(&pool, &steps, &at, &wide);
                    if !noops.is_empty() {
                        for (c, s) in &noops {
                            log(&format!("    no-op event {} at step {s}: dropped, its ticks are don't-care", route2::fmt_pos(*c)));
                        }
                        let reduced = steps.retimed_without_all(&at, &noops);
                        match mechwire::measure_ideal(&mut pool.worker(0), &reduced, &Default::default()) {
                            Ok(m) => {
                                mech = m;
                                mechwire::dont_care(&mut mech, &steps, &at, &noops);
                                log("    wanted waves without them:");
                                for l in mech.describe() {
                                    log(&format!("    {l}"));
                                }
                            }
                            Err(e) => log(&format!("    cannot measure without the no-op events: {e:#}")),
                        }
                    }
                    let win2: Vec<((mctas::sim::Pos, usize), Vec<i32>)> = win.into_iter().filter(|(e, _)| !noops.contains(e)).collect();
                    let free = mechwire::free_receivers(&mech, &steps, &at, &win2, auto_free);
                    for (c, a, b) in &free {
                        log(&format!("    free: {} any 2 gt pulse starting {a}..{b}", route2::fmt_pos(*c)));
                    }
                    // What depends on each free receiver's action: its timing is the router's
                    // choice, so no receiver may watch those motions (their measured times are
                    // not the real ones).
                    let mut all_dep: Vec<mctas::sim::Pos> = Vec::new();
                    for (c, _, _) in &free {
                        let dep = mechwire::own_motion(&mut pool.worker(0), &mech, &steps.clone_without(&noops), &at, *c).unwrap_or_default();
                        log(&format!("    {} drives {} watched cells", route2::fmt_pos(*c), dep.len()));
                        all_dep.extend(dep);
                    }
                    all_dep.sort();
                    all_dep.dedup();
                    ow.forbid = mech.wants.iter().map(|w| (w.0, all_dep.clone())).collect();
                    if timing {
                        // The other events keep their own (narrower) windows: every receiver
                        // accepts each combination of its events' shifts.
                        let narrow: Vec<((mctas::sim::Pos, usize), Vec<i32>)> = win2.iter().filter(|(_, ds)| span(ds) < auto_free).cloned().collect();
                        let alts = mechwire::timing_alts(&mech, &steps, &at, &narrow, 400);
                        for (c, a) in &alts {
                            log(&format!("    timing: {} accepts {} waves", route2::fmt_pos(*c), a.len()));
                        }
                        ow.alts = alts;
                    }
                    ow.free.extend(free);
                }
                let r = mechwire::wire_all(&mech, &spots, &ow, &log);
                log(&format!("{label}: {} candidates, best {}", r.len(), r.first().map(|w| w.blocks.to_string()).unwrap_or("-".into())));
                // Judge this schedule's best candidates now (one lever spot's near-duplicates
                // would crowd a global list).
                if verify > 0 && !r.is_empty() {
                    let n = verify.min(r.len());
                    let ok = mechwire::judge(&pool, &r[..n], &sopts, &log);
                    match ok.first() {
                        Some((d0, desc)) => {
                            log(&format!("{label}: VERIFIED {} blocks: {desc}", crate::block_count(d0)));
                            let polished;
                            let d = if polish_secs > 0 {
                                polished = mechwire::polish(&pool, d0, &o.route, &sopts, polish_secs, &log)?;
                                log(&format!("{label}: polished {} -> {} blocks", crate::block_count(d0), crate::block_count(&polished)));
                                &polished
                            } else {
                                d0
                            };
                            ranking.push((crate::block_count(d), label.clone(), crate::block_count(&mech.bare)));
                            if let Some(dir) = &out_dir {
                                std::fs::create_dir_all(dir)?;
                                let stem: String = label.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
                                let mut dd = d.clone();
                                dd.name = format!("{stem}-{}b", crate::block_count(d));
                                let path = dir.join(format!("{}.txt", dd.name));
                                dd.save(&path)?;
                                if archive {
                                    let note = format!("verified WORKS: {} blocks; door mech-wire from {label}: {desc}", crate::block_count(d));
                                    if let Some(p) = crate::archive::save(&path, &dd, crate::block_count(d), &note, None)? {
                                        log(&format!("archived {}", p.display()));
                                    }
                                }
                            }
                        }
                        None => {
                            log(&format!("{label}: none of the best {n} candidates passes the judge"));
                            ranking.push((usize::MAX, label.clone(), crate::block_count(&mech.bare)));
                        }
                    }
                }
                all.extend(r.into_iter().map(|w| mechwire::Wired { desc: format!("{label}: {}", w.desc), ..w }));
            }
            }
            ranking.sort();
            println!("== ranking (verified mechanism + wiring) ==");
            for (b, label, mb) in &ranking {
                println!("{:>6} total, {mb} mechanism: {label}", if *b == usize::MAX { "-".to_string() } else { b.to_string() });
            }
            all.sort_by(|a, b| a.blocks.cmp(&b.blocks).then_with(|| a.desc.cmp(&b.desc)));
            let mut seen = std::collections::HashSet::new();
            all.retain(|w| seen.insert(crate::search::design_hash(&w.design)));
            for w in all.iter().take(20) {
                println!("candidate {} blocks: {}", w.blocks, w.desc);
            }
            if let Some(dir) = &out_dir {
                std::fs::create_dir_all(dir)?;
                for (i, w) in all.iter().take(verify.max(5)).enumerate() {
                    let mut d = w.design.clone();
                    d.name = format!("cand{i:02}-{}b", w.blocks);
                    d.save(&dir.join(format!("cand{i:02}-{}b.txt", w.blocks)))?;
                }
            }
            let _ = &schedule;
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
        DoorCmd::MechSynth { base, add, lo, hi, kinds, ext, cap, threads, max_blocks, sym_len, sym_tries, out_dir, top, verify, extra_offsets, gap, workers, just_base, require, pulses_only, edges, split_only, seamless, no_ring_fire, sym_bfs, sym_levels } => {
            crate::mechsynth::SEAMLESS.store(seamless, std::sync::atomic::Ordering::Relaxed);
            crate::mechsynth::NO_RING_FIRE.store(no_ring_fire, std::sync::atomic::Ordering::Relaxed);
            if let Some(seq) = &sym_bfs {
                return crate::mechsynth::sym_cli(version, &base, seq, sym_levels, cap, out_dir.as_deref(), verify, extra_offsets, gap);
            }
            crate::mechsynth::PULSES_ONLY.store(pulses_only, std::sync::atomic::Ordering::Relaxed);
            crate::mechsynth::SPLIT_ONLY.store(split_only, std::sync::atomic::Ordering::Relaxed);
            let e = match edges.as_str() {
                "both" => 0,
                "start" => 1,
                "end" => 2,
                other => anyhow::bail!("--edges start|end|both, got {other}"),
            };
            crate::mechsynth::EDGES.store(e, std::sync::atomic::Ordering::Relaxed);
            let o = crate::mechsynth::EnumOptions {
                require: require.iter().map(|r| crate::mechsynth::Require::parse(r)).collect::<Result<_>>()?,
                add,
                lo: crate::mechsynth::parse_p(&lo)?,
                hi: crate::mechsynth::parse_p(&hi)?,
                kinds: kinds.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
                ext,
                cap,
                threads,
                max_blocks,
                sym_len,
                sym_tries,
            };
            crate::mechsynth::cli(version, &base, &o, out_dir.as_deref(), top, verify, extra_offsets, gap, workers, just_base)?;
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
        DoorCmd::Video { design, out, tps, upload, style, orbit, size } => {
            let d = load_design(&design)?;
            let (width, height) = size.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).ok_or_else(|| anyhow::anyhow!("--size wants WIDTHxHEIGHT"))?;
            let opts = crate::video::VideoOptions { tps, width, height, ..video_style(&style, orbit)? };
            crate::video::record(version, &d, &out, &opts)?;
            println!("recorded {}", out.display());
            if let Some(tag) = upload {
                let url = mctas::video::upload_release_asset("vjeux/minecraft-tas", &tag, &out)?;
                println!("uploaded {url}");
            }
        }
        DoorCmd::TickVideo { design, out, secs_per_tick, draw_ms, upload, size } => {
            let d = load_design(&design)?;
            let (width, height) = size.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).ok_or_else(|| anyhow::anyhow!("--size wants WIDTHxHEIGHT"))?;
            let opts = crate::tickvideo::TickVideoOptions { width, height, secs_per_tick, draw_ms, ..Default::default() };
            crate::tickvideo::record(version, &d, &out, &opts)?;
            println!("recorded {}", out.display());
            if let Some(tag) = upload {
                let url = mctas::video::upload_release_asset("vjeux/minecraft-tas", &tag, &out)?;
                println!("uploaded {url}");
            }
        }
        DoorCmd::World { archive, out, name, workers, shots, verdicts, check } => {
            let o = crate::world::WorldOptions { archive, out, name, workers, shots, verdicts, check };
            let tar = crate::world::build(version, &o)?;
            println!("world packed: {}", tar.display());
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
        DoorCmd::Relever { designs, subs, out_dir, extra_offsets, workers, verdicts, max_blocks, any_rotation, table, why } => {
            let offsets = crate::search::judge_offsets(extra_offsets);
            let judge = VerifyOptions { offsets: offsets.clone(), ..Default::default() };
            let screen = VerifyOptions { cycles: 1, max_ticks: 200, offsets: offsets.iter().take(2).copied().collect(), keep_frames: false, empty_walkway: true, tile: None };
            let known = verdicts.as_deref().map(crate::properties_dyn::load_verdicts).unwrap_or_default();
            let subs: Vec<String> = subs.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let mut seen = std::collections::HashSet::new();
            let mut loaded: Vec<(String, Design)> = Vec::new();
            for p in &designs {
                let d = match load_design(p) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("{}: {e:#}", p.display());
                        continue;
                    }
                };
                let h = format!("{:016x}", crate::leaderboard::design_hash(&d));
                if crate::block_count(&d) > max_blocks || known.get(&h) == Some(&false) || !seen.insert(d.content_key()) {
                    continue;
                }
                loaded.push((p.display().to_string(), d));
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
            let results = pool.map(jobs, |w, _, (i, v)| Ok((i, crate::relever::judge_variant(w, v, &judge, &screen, !any_rotation)?)));
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
        DoorCmd::VanillaCheck { designs, cycles, extra_offsets, positions, offset, max_ticks, verbose, keep_world, tick_rate, no_free_run, trace } => {
            vanilla_check_cmd(version, &cfg, &designs, cycles, extra_offsets, positions, offset, max_ticks, verbose, keep_world, tick_rate, no_free_run, trace)?;
        }
        DoorCmd::VanillaBuild { scripts, design, cycles, extra_offsets, positions, offset, max_ticks, until_step, verbose, keep_world, tick_rate, real_time, slow, stay } => {
            vanilla_build_cmd(version, &cfg, &scripts, design, cycles, extra_offsets, positions, offset, max_ticks, until_step, verbose, keep_world, tick_rate, real_time, slow, stay)?;
        }
        DoorCmd::VanillaGadget { scripts, extra_offsets, positions, verbose, keep_world, tick_rate } => {
            let mut offsets = VerifyOptions::default().offsets;
            offsets.extend(crate::extra_offsets(extra_offsets));
            if let Some(n) = positions {
                offsets.truncate(n.max(1));
            }
            let mut w = Worker::spawn(&cfg)?;
            let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), keep_dir: keep_world, tick_rate, ..Default::default() };
            let t0 = std::time::Instant::now();
            let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
            eprintln!("vanilla server up in {:.1}s ({})", t0.elapsed().as_secs_f64(), v.dir.display());
            let ok = crate::vanilla_gadget::run_cli(&mut w, &mut v, &scripts, &offsets, verbose)?;
            if keep_world {
                println!("world kept in {}", v.dir.display());
            }
            drop(v);
            if !ok {
                std::process::exit(1);
            }
        }
        DoorCmd::VanillaFuzz { count, seed, cycles, max_ticks, out_dir } => {
            vanilla_fuzz_cmd(version, &cfg, count, seed, cycles, max_ticks, &out_dir)?;
        }
        DoorCmd::Buildable { designs, budget, workers, helpers, positions, seed, out_dir, table, lever, verbose } => {
            let lever = match lever.as_str() {
                "both" => None,
                "off" => Some(false),
                "on" => Some(true),
                other => anyhow::bail!("--lever {other}: expected both, off or on"),
            };
            let o = crate::buildable::CliOptions { budget, workers, helpers, positions, seed, verbose, lever };
            crate::buildable::run_cli(version, &designs, &o, out_dir.as_deref(), table.as_deref())?;
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
        DoorCmd::Verify { design, timeline: show, cycles, archive, extra_offsets, archive_dir, no_rotations, orientations, tile, tile_levers, tile_out, tile_extra } => {
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
        DoorCmd::Resynth { designs, out, min_size, max_size, palette, floors, margin, max_cands, loose, only, workers, extra_offsets, screen_offsets, screen_ticks, max_open, max_close, global_retry, require_any, tile, any_rotation, seamless, extra, repeater_input, eligible } => {
            use crate::synth::*;
            let seamless = seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?;
            let tile = tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
            let only = match only {
                Some(s) => {
                    let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--only wants x0,y0,z0:x1,y1,z1"))?;
                    let a = parse_cells(a)?.first().copied().ok_or_else(|| anyhow::anyhow!("bad --only corner {a:?}"))?;
                    let b = parse_cells(b)?.first().copied().ok_or_else(|| anyhow::anyhow!("bad --only corner {b:?}"))?;
                    Some(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
                }
                None => None,
            };
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

/// Video options of a `--style` name.
fn video_style(style: &str, orbit: f64) -> Result<crate::video::VideoOptions> {
    use crate::video::{Camera, VideoOptions};
    Ok(match style {
        "card" => VideoOptions { camera: Camera::Door { orbit }, ..Default::default() },
        "clean" => VideoOptions::clean(orbit),
        "classic" => VideoOptions::classic(),
        _ => anyhow::bail!("unknown --style {style} (card, clean or classic)"),
    })
}

/// `door vanilla-check`: one vanilla server and one simulator worker for all the designs.
#[allow(clippy::too_many_arguments)]
fn vanilla_check_cmd(
    version: &str,
    cfg: &WorkerConfig,
    designs: &[PathBuf],
    cycles: u32,
    extra_offsets: u32,
    positions: Option<usize>,
    offset: Option<String>,
    max_ticks: u32,
    verbose: bool,
    keep_world: bool,
    tick_rate: u32,
    no_free_run: bool,
    trace: bool,
) -> Result<()> {
    use crate::vanilla_check::{CheckOptions, check, verdict_str};
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    if let Some(n) = positions {
        offsets.truncate(n.max(1));
    }
    if let Some(o) = offset {
        let v: Vec<i32> = o.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--offset wants x,y,z");
        offsets = vec![[v[0], v[1], v[2]]];
    }
    let opts = CheckOptions { cycles, offsets, max_ticks, verbose, free_run: !no_free_run, trace };
    let mut w = Worker::spawn(cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), keep_dir: keep_world, tick_rate, ..Default::default() };
    let t0 = std::time::Instant::now();
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    eprintln!("vanilla server up in {:.1}s ({})", t0.elapsed().as_secs_f64(), v.dir.display());
    let mut any_bad = false;
    let mut oracle = crate::vanilla_check::Oracle::default();
    for path in designs {
        let d = load_design(path)?;
        let t = std::time::Instant::now();
        let name = path.display().to_string();
        match check(&mut w, &mut v, &mut oracle, &d, &opts) {
            Ok(results) => {
                let n = results.len();
                let ticks: usize = results.iter().map(|r| r.ticks_compared).sum();
                let diverging: Vec<_> = results.iter().filter(|r| r.divergence.is_some() || r.sim.is_err() != r.vanilla.is_err()).collect();
                let vanilla_fail = results.iter().find(|r| r.vanilla.is_err());
                let free_fail = results.iter().find_map(|r| r.free_run.as_ref().and_then(|f| f.as_ref().err()).map(|e| (r.offset, e.clone())));
                let free_ok = results.iter().any(|r| matches!(r.free_run, Some(Ok(()))));
                let line = if let Some(r) = diverging.first() {
                    any_bad = true;
                    format!(
                        "DIVERGES at {}/{n} positions; first at offset {:?}: {} | sim {} | vanilla {}",
                        diverging.len(),
                        r.offset,
                        r.divergence.as_deref().unwrap_or("verdicts differ"),
                        verdict_str(&r.sim),
                        verdict_str(&r.vanilla)
                    )
                } else if let Some((off, e)) = free_fail {
                    any_bad = true;
                    format!("REAL-TIME CYCLE DIFFERS at offset {off:?} (stepped runs identical): {e}")
                } else if let Some(r) = vanilla_fail {
                    any_bad = true;
                    format!("FAIL in vanilla (the simulator agrees, frames identical) at offset {:?}: {}", r.offset, verdict_str(&r.vanilla))
                } else {
                    format!(
                        "PASS: vanilla matches the simulator tick for tick ({ticks} ticks at {n} positions x {cycles} cycles{}); {}",
                        if free_ok { ", plus a real-time 20 tps cycle" } else { "" },
                        verdict_str(&results[0].vanilla)
                    )
                };
                println!("vanilla-check: {name} ({} blocks, {}) {line} [{:.0}s]", crate::block_count(&d), crate::category(&d), t.elapsed().as_secs_f64());
                if verbose {
                    let s = &v.stats;
                    eprintln!(
                        "  vanilla time: {} steps {:.1}s ({:.0} ms each), {} snapshots {:.1}s ({:.0} ms each)",
                        s.steps,
                        s.step.as_secs_f64(),
                        s.step.as_secs_f64() * 1000.0 / s.steps.max(1) as f64,
                        s.snapshots,
                        s.snapshot.as_secs_f64(),
                        s.snapshot.as_secs_f64() * 1000.0 / s.snapshots.max(1) as f64
                    );
                }
            }
            Err(e) => {
                any_bad = true;
                println!("vanilla-check: {name} ERROR: {e:#}");
            }
        }
    }
    if keep_world {
        println!("world kept in {}", v.dir.display());
    }
    drop(v);
    if any_bad {
        std::process::exit(1);
    }
    Ok(())
}

/// `door vanilla-build`: replay build scripts in the vanilla server with the bot.
#[allow(clippy::too_many_arguments)]
fn vanilla_build_cmd(
    version: &str,
    cfg: &WorkerConfig,
    scripts: &[String],
    design: Option<String>,
    cycles: u32,
    extra_offsets: u32,
    positions: Option<usize>,
    offset: Option<String>,
    max_ticks: u32,
    until_step: Option<usize>,
    verbose: bool,
    keep_world: bool,
    tick_rate: u32,
    real_time: bool,
    slow: u32,
    stay: bool,
) -> Result<()> {
    use crate::vanilla_build::{BuildOptions, check, parse_script};
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    if let Some(n) = positions {
        offsets.truncate(n.max(1));
    }
    if let Some(o) = offset {
        let v: Vec<i32> = o.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--offset wants x,y,z");
        offsets = vec![[v[0], v[1], v[2]]];
    }
    let opts = BuildOptions { cycles, offsets, max_ticks, until_step, verbose, real_time, slow, stay };
    let mut w = Worker::spawn(cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), keep_dir: keep_world, tick_rate, ..Default::default() };
    let t0 = std::time::Instant::now();
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    eprintln!("vanilla server up in {:.1}s ({})", t0.elapsed().as_secs_f64(), v.dir.display());
    let mut oracle = crate::vanilla_check::Oracle::default();
    let mut any_bad = false;
    for path in scripts {
        let t = std::time::Instant::now();
        let r = (|| -> Result<String> {
            let script = parse_script(path, &std::fs::read_to_string(path)?)?;
            let spec = anyhow::Context::context(design.clone().or(script.source.clone()), "the script names no `# source:` design (pass --design)")?;
            let d = crate::buildable::load_any(&spec)?;
            let results = check(&mut w, &mut v, &mut oracle, &d, &script, &opts)?;
            let n = results.len();
            let bad: Vec<_> = results.iter().filter(|r| !r.pass()).collect();
            let actions = results.first().map(|r| r.steps).unwrap_or(0);
            let ray: usize = results.iter().map(|r| r.stats.clicks_on_ray).sum();
            let off_ray: usize = results.iter().map(|r| r.stats.clicks_off_ray).sum();
            let anchors: usize = results.iter().map(|r| r.stats.dirt_on_anchor).sum();
            let ticks: usize = results.iter().map(|r| r.ticks_compared).sum();
            let summary = format!(
                "{} blocks, {}; {actions} actions x {n} positions ({ray} clicks from the view ray, {off_ray} off it, {anchors} temporary dirt on an anchor)",
                crate::block_count(&d),
                crate::category(&d)
            );
            Ok(if let Some(r) = bad.first() {
                any_bad = true;
                format!(
                    "FAIL at {}/{n} positions ({summary}); first at offset {:?}: {}",
                    bad.len(),
                    r.offset,
                    r.step_divergence
                        .clone()
                        .map(|d| format!("action diverges {d}"))
                        .or_else(|| r.built_vs_rest.clone().map(|d| format!("built != judge rest: {d}")))
                        .or_else(|| r.divergence.clone())
                        .unwrap_or_else(|| format!("sim {} | vanilla {}", crate::vanilla_check::verdict_str(&r.sim), crate::vanilla_check::verdict_str(&r.vanilla)))
                )
            } else {
                format!(
                    "PASS ({summary}): every action left vanilla identical to the simulator, the built world equals the judge's rest state, cycles identical tick for tick ({ticks} ticks); {}",
                    crate::vanilla_check::verdict_str(&results[0].vanilla)
                )
            })
        })();
        match r {
            Ok(line) => println!("vanilla-build: {path} {line} [{:.0}s]", t.elapsed().as_secs_f64()),
            Err(e) => {
                any_bad = true;
                println!("vanilla-build: {path} ERROR: {e:#}");
            }
        }
    }
    if keep_world {
        println!("world kept in {}", v.dir.display());
    }
    drop(v);
    if any_bad {
        std::process::exit(1);
    }
    Ok(())
}

/// `door vanilla-fuzz`: random contraptions, settled in the simulator, compared tick for tick.
fn vanilla_fuzz_cmd(version: &str, cfg: &WorkerConfig, count: u64, seed: u64, cycles: u32, max_ticks: u32, out_dir: &std::path::Path) -> Result<()> {
    use crate::vanilla_check::{CheckOptions, Oracle, check, fuzz_design, verdict_str};
    let mut w = Worker::spawn(cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), tick_rate: 100, ..Default::default() };
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    let mut oracle = Oracle::default();
    let offsets = crate::extra_offsets(32);
    let (mut same, mut skipped, mut diverging, mut errors) = (0, 0, 0, 0);
    for s in seed..seed + count {
        let raw = fuzz_design(s);
        let Some(canon) = crate::vanilla_check::rest_state(&mut w, &raw)? else {
            skipped += 1;
            continue;
        };
        let lever = canon.ports["lever"];
        if !canon.blocks.get(&lever).is_some_and(|b| b.starts_with("minecraft:lever") && b.contains("powered=false")) {
            skipped += 1;
            continue;
        }
        let opts = CheckOptions { cycles, offsets: vec![offsets[(s % 32) as usize]], max_ticks, verbose: false, free_run: false, trace: true };
        match check(&mut w, &mut v, &mut oracle, &canon, &opts) {
            Ok(results) => {
                let r = &results[0];
                if r.divergence.is_some() || r.sim.is_err() != r.vanilla.is_err() {
                    diverging += 1;
                    std::fs::create_dir_all(out_dir)?;
                    let path = out_dir.join(format!("fuzz-{s}.txt"));
                    canon.save(&path)?;
                    println!(
                        "fuzz {s}: DIVERGES at offset {:?}: {} | sim {} | vanilla {} (saved {})",
                        r.offset,
                        r.divergence.as_deref().unwrap_or("verdicts differ"),
                        verdict_str(&r.sim),
                        verdict_str(&r.vanilla),
                        path.display()
                    );
                } else {
                    same += 1;
                    println!("fuzz {s}: identical ({} ticks, {} blocks)", r.ticks_compared, canon.blocks.len());
                }
            }
            Err(e) => {
                errors += 1;
                std::fs::create_dir_all(out_dir)?;
                let path = out_dir.join(format!("fuzz-{s}-error.txt"));
                canon.save(&path)?;
                println!("fuzz {s}: ERROR {e:#} (saved {})", path.display());
            }
        }
    }
    println!("vanilla-fuzz: {same} identical, {diverging} diverging, {skipped} skipped (never settle / lever lost), {errors} errors");
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
