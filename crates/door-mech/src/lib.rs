//! `mctas door` commands: mechanisms and their wiring: mech-synth, mech-wire, mechspace, families, topology, connect, fullmech, obs-chain, ideal, drive, rows-fire.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::{SearchArgs, load_design};
use piston_door::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Where each strip-row mechanism's pistons fire (rowsfire.rs): replays every row of agent
    /// families' ROWS.tsv (`door families --rows`) in the 1-D macro model and prints, per piston
    /// action of the open and close schedules, the cell the piston stands on when it fires.
    /// In a 5-tall strip box rows 0 and 1 cannot be fired from the door columns x 0..2 (the floor
    /// and ceiling ring are there): `--wirable` keeps the rows with no such firing.
    RowsFire {
        tsv: PathBuf,
        /// Only rows with no firing from x 0..2.
        #[arg(long)]
        wirable: bool,
        /// Only ids starting with one of these (e.g. `r046,r075`).
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
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
        /// Routing box "x0,y0,z0:x1,y1,z1" (world coordinates; lever spots too) instead of the
        /// design's bounds +- --margin: e.g. a flat box (z0 = z1 = the door's z) for flat doors.
        #[arg(long = "box")]
        region_box: Option<String>,
        /// Cells no wiring may use "x,y,z;...".
        #[arg(long, default_value = "")]
        forbid: String,
        /// Also forbid the door plane's hallway ring (the 12 cells face-adjacent to the opening at
        /// the door's z): static wiring there shows in the open doorway (not seamless).
        #[arg(long)]
        forbid_ring: bool,
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
    /// Exact decision procedure for families' macro pulse model (mechspace.rs): partial-order
    /// reduction with stubborn sets (footprint groups, landmark condition), hole rule exact.
    /// `selftest` (tracked transitions vs Mech::apply, independence claims), `validate` (every
    /// candidate of --boxes, compared with families' full-graph solver), `decide --input
    /// unresolved/WxH.txt --boxes WxH`, `gen` (families' generator with --two/--carry/--vmid,
    /// every candidate decided).
    Mechspace {
        /// selftest | validate | decide | gen
        cmd: String,
        #[arg(long, default_value = "7x8")]
        boxes: String,
        /// Candidate list (families' unresolved format: x0 y0 pieces...).
        #[arg(long)]
        input: Option<PathBuf>,
        #[arg(long, default_value_t = 11)]
        pistons: usize,
        /// State cap of each reduced search.
        #[arg(long, default_value_t = 3_000_000)]
        cap: usize,
        /// families' cap (validate).
        #[arg(long, default_value_t = 300_000)]
        fcap: usize,
        #[arg(long, default_value_t = 40)]
        threads: usize,
        /// Local reach cap of an independent event group.
        #[arg(long, default_value_t = 32)]
        cap_local: usize,
        /// Groups with more events go straight to the core.
        #[arg(long, default_value_t = 6)]
        max_group: usize,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/mechspace")]
        out_dir: PathBuf,
        /// Output tag (file names).
        #[arg(long)]
        tag: Option<String>,
        /// At most this many candidates (0 = all).
        #[arg(long, default_value_t = 0)]
        limit: usize,
        #[arg(long, default_value_t = 1)]
        two: usize,
        #[arg(long, default_value_t = 0)]
        carry: usize,
        #[arg(long)]
        vmid: Option<String>,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/families/CATALOG.tsv")]
        catalog: PathBuf,
        /// selftest: candidates and walk length.
        #[arg(long, default_value_t = 300)]
        n: usize,
        #[arg(long, default_value_t = 200)]
        walk: usize,
        /// After a capped exact search, families' A* with this cap may still find a solution.
        #[arg(long, default_value_t = 0)]
        astar: usize,
        /// Lean second stage (visited sets only, reduced then plain) up to this many states.
        #[arg(long, default_value_t = 0)]
        cap_lean: usize,
        /// The wirable sub-model: no piston body may enter the 3x3 (heads may).
        #[arg(long)]
        no_door_bodies: bool,
        /// FULL FRAME 12/12 when open (static stones in the frame cells the cycle leaves air).
        #[arg(long)]
        full_frame: bool,
        /// Generator: non-core 1-move exits may use sticky pullers extended at rest.
        #[arg(long)]
        ext_pullers: bool,
        /// Generator: exactly one piston in the box's bottom row, sticky facing up.
        #[arg(long)]
        bottom_su: bool,
        /// Open = all 9 door blocks in frame cells (3 static stones in the others).
        #[arg(long)]
        frame9: bool,
        /// Only this door offset "dx,dy" (columns left of / rows below the door).
        #[arg(long)]
        door_offset: Option<String>,
        /// export: ticks between macro steps of the ideal plans.
        #[arg(long, default_value_t = 6)]
        gap: u32,
        /// export: simulator workers.
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// export: extra positions of the ideal replay (4 positions + this many).
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
    },
    /// The catalog of flat (1-wide) seamless mechanisms per box (families.rs): every door block
    /// leaves the 3x3 in 1-2 unit moves (center 2), each move provided by a pull or push piston
    /// (reused, or lifted by one elevator piston); every candidate solved exactly in the macro
    /// piston model with the FULL seamless open test, reduced to minimal mechanisms, mirror
    /// images merged; writes CATALOG.md + CATALOG.tsv; `--verify` replays each in the simulator.
    Families {
        /// Boxes "WxH,..." (sorted by volume).
        #[arg(long, default_value = "7x7,7x8,8x7,8x8,7x9,9x7,8x9,9x8,8x10,10x8,9x9")]
        boxes: String,
        /// Most pistons per mechanism.
        #[arg(long, default_value_t = 10)]
        pistons: usize,
        /// State cap of the exact solve.
        #[arg(long, default_value_t = 200000)]
        cap: usize,
        /// Pair cap of the symmetric-train product BFS.
        #[arg(long, default_value_t = 200000)]
        sym_cap: usize,
        #[arg(long, default_value_t = 48)]
        threads: usize,
        /// No lifted (elevator) providers.
        #[arg(long)]
        no_lifts: bool,
        /// No push providers (pulls only).
        #[arg(long)]
        no_pushes: bool,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/families")]
        out_dir: PathBuf,
        /// Replay every catalog entry in the simulator with ideal inputs.
        #[arg(long)]
        verify: bool,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        #[arg(long, default_value_t = 0)]
        extra_offsets: u32,
        /// Ticks between macro steps in the ideal schedules.
        #[arg(long, default_value_t = 6)]
        gap: u32,
        /// Solve at most this many candidates per box (0 = all; for testing).
        #[arg(long, default_value_t = 0)]
        limit: usize,
        /// Non-core perimeter blocks that may take a 2-move path (each costs a provider).
        #[arg(long, default_value_t = 1)]
        two: usize,
        /// Solve one design's mechanism instead (its bounding box grown by --check-margin).
        #[arg(long)]
        check: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        check_margin: i8,
        /// Also search with level events (A*, capped) when the exact pulse model fails.
        #[arg(long)]
        levels: bool,
        /// Strip mode: every row mechanism (horizontal pistons in one door row), exhaustive.
        #[arg(long)]
        rows: bool,
        /// Strip mode: most pistons per row.
        #[arg(long, default_value_t = 4)]
        row_pistons: usize,
        /// Strip mode: longest arm (cells left or right of the door in the row).
        #[arg(long, default_value_t = 6)]
        arm: i8,
        /// Strip mode: widest row to enumerate.
        #[arg(long, default_value_t = 99)]
        row_max_width: i8,
        /// Re-check a run's unresolved candidates (unresolved/WxH.txt; --boxes WxH) by exhaustive
        /// reachability with --cap.
        #[arg(long)]
        resolve: Option<PathBuf>,
        /// General carrying depth: reused pistons displaced 1-2 cells in ANY direction (sideways
        /// too) by carriers, carriers carried themselves up to this depth (0 = elevator lifts only).
        #[arg(long, default_value_t = 0)]
        carry: usize,
        /// Middle-row side blocks forced to leave VERTICALLY: `ml`, `mr`, `any` (ML or MR) or
        /// `both`.
        #[arg(long)]
        vmid: Option<String>,
        /// Strip mode: x columns (door coordinates) no piston may fire from, e.g.
        /// `--forbid-fire=-1,1,3` (cells no wiring can reach in a 5-tall strip box).
        #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
        forbid_fire: Vec<i8>,
        /// Strip mode: redstone blocks per row (pushable pieces that power the pistons beside
        /// them, not through their front; every state settled).
        #[arg(long, default_value_t = 0)]
        rblocks: usize,
        /// Strip mode: a redstone block also powers the piston whose FRONT touches it.
        #[arg(long)]
        r_both: bool,
    },
    /// Mechanism synthesis (mechsynth.rs): enumerate up to --add blocks (pistons, slime...) on a
    /// base mechanism, solve each in a macro piston model (open + close paths, symmetric
    /// schedules), rank by (blocks, control events), write ideal-input designs + schedules and
    /// replay the best in the simulator (`--verify N`).
    /// Wiring topology (topology.rs) of every mechanism in agent families' catalog TSV, each in its
    /// own box: wiring components, piston links, pulse parity; WIRABLE or not (strict / permissive)
    Topology {
        tsv: PathBuf,
        /// Only ids starting with one of these (e.g. `7x8,8x7`)
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Print each mechanism's full report
        #[arg(long)]
        verbose: bool,
    },
    /// Connectivity check (connect.rs): can a flat mechanism be wired inside a box at all? Free-cell
    /// regions after the run's footprint and the ring; each piston fed from a region next to it or
    /// its quasi cell (directly, through a plain ring stone or a door block); a region wakes when it
    /// can watch a change; every event order searched, both edges starting from the lever's region
    /// only. Parity and room are ignored: a necessary condition. Reads families' CATALOG.tsv and
    /// tries every placement of each entry in each box.
    Connect {
        tsv: PathBuf,
        /// Only ids starting with one of these (e.g. `7x8,7x9-1`)
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        /// Boxes "WxH,..." (each entry is placed at every offset that fits)
        #[arg(long, default_value = "7x9")]
        boxes: String,
        /// State cap per edge search
        #[arg(long, default_value_t = 300000)]
        cap: usize,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Print each placement's region report
        #[arg(long)]
        verbose: bool,
    },
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
        /// Wiring topology in a volume box of the door plane, "x0,y0:x1,y1" (relative to the door
        /// port): can each mechanism be wired inside it with FULL seamless (topology.rs: wiring
        /// components, piston links, pulse parity)? With --just-base prints the full report; in an
        /// enumeration keeps only the wirable mechanisms.
        #[arg(long)]
        topology: Option<String>,
        /// Open states must also be FULL seamless for a flat door (macro proxy): the door plane's
        /// 12 hallway-ring cells hold only door blocks, plain stone or air (no heads, no slime or
        /// honey), and an empty ring cell has no circuitry right behind it.
        #[arg(long)]
        seamless_open: bool,
        /// Replay a hand-written mechanism path instead of searching: "<open events> | <close
        /// events>" (same event syntax as --sym-bfs); writes oc.txt/oc.ideal to --out-dir and
        /// replays the ideal inputs in the simulator with --verify 1.
        #[arg(long)]
        oc: Option<String>,
    },
    /// Apply an edit script to a design and write the (canonicalized) result. Script lines:
    /// `set x y z <state>` (an extended piston gets its head too), `clear x y z` (a piston
    /// takes its head along), `port <name> x y z`; `#` starts a comment line.
    /// Wire a pulse-driven mechanism with a tree of observers rooted at an observer watching the
    /// lever (obschain.rs): every driver observer (spec options) fires at its time, watching a
    /// tree observer one step earlier; exact-length observer paths through free cells, fewest
    /// observers (branch and bound within --secs).
    ObsChain {
        spec: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Paths tried per driver.
        #[arg(long, default_value_t = 12)]
        max_paths: usize,
        #[arg(long, default_value_t = 60)]
        secs: u64,
        /// Added to every group's time (the root's distance budget).
        #[arg(long, default_value_t = 0)]
        shift: i32,
        /// Randomized restarts (option and direction orders shuffled), --secs split between them;
        /// the best wiring over all restarts is kept.
        #[arg(long, default_value_t = 1)]
        restarts: usize,
    },
    /// Mechanism search on the EXACT model (track pistmodel, crates/piston-door/src/fullmech.rs):
    /// every transition is the real tick-level game (redsim: spitting, 0-ticks, event order,
    /// carried pistons and power sources), inputs tracked per piston; FULL seamless open +
    /// close back to rest. `--check` solves the mechanism of designs; `--boxes` re-solves
    /// families' candidates per box.
    Fullmech {
        #[arg(long, num_args = 1..)]
        check: Vec<PathBuf>,
        /// Boxes W x H, comma separated (e.g. 7x7,7x8).
        #[arg(long)]
        boxes: Option<String>,
        #[arg(long, default_value_t = 10)]
        pistons: usize,
        /// Non-core perimeter blocks that may take a 2-move path (families' --two).
        #[arg(long, default_value_t = 0)]
        two: usize,
        /// Middle-row side blocks forced to leave VERTICALLY in the generated candidates
        /// (families' --vmid): `ml`, `mr`, `any` or `both`.
        #[arg(long)]
        vmid: Option<String>,
        /// Pulse lengths (ticks) offered as actions.
        #[arg(long, default_value = "1,3")]
        pulses: String,
        /// Also level toggles.
        #[arg(long)]
        levels: bool,
        /// States per A* search.
        #[arg(long, default_value_t = 3000)]
        cap: usize,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        #[arg(long, default_value_t = 0)]
        limit: usize,
        #[arg(long)]
        out: Option<PathBuf>,
        /// State cap of the macro model's verdict per candidate.
        #[arg(long, default_value_t = 20000)]
        macro_cap: usize,
        /// Skip the candidates the macro model already solves.
        #[arg(long)]
        only_new: bool,
        /// Input reduction: for each working mechanism, try to replace each piston's input by
        /// one helper (redstone block or observer) in a free cell of the box.
        #[arg(long)]
        reduce: bool,
        #[arg(long, default_value_t = 4000)]
        reduce_cap: usize,
        /// Also overlapping pulse pairs of nearby pistons as actions (same-tick timing).
        #[arg(long)]
        pairs: bool,
        /// Simulated annealing over mechanisms in this box (W x H), not tied to families' exit
        /// plans: pistons anywhere (carried, stacked), helpers with --helpers.
        #[arg(long)]
        anneal: Option<String>,
        /// Door offsets for --anneal: "dx,dy;dx,dy" or all.
        #[arg(long)]
        offsets: Option<String>,
        #[arg(long, default_value_t = 600)]
        seconds: u64,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Helpers (redstone blocks, observers, stone, obsidian) in --anneal.
        #[arg(long)]
        helpers: bool,
        /// Write every exact mechanism of --boxes (design + `door ideal` plan) under DIR/WxH/.
        #[arg(long)]
        export: Option<PathBuf>,
        /// Slime and honey in --anneal (not for the flat board, which is slime-free).
        #[arg(long)]
        slime: bool,
        /// Replay exported `door ideal` plans in the real simulator (2 cycles, 4 positions).
        #[arg(long, num_args = 1..)]
        verify_ideal: Vec<PathBuf>,
        /// Input merging: the fewest independent input signals per solved mechanism (pistons
        /// switched together), written to GROUPS.tsv + *-groups.ideal with --export.
        #[arg(long)]
        groups: bool,
        /// Piston removal + one helper (stone, obsidian, redstone block, observer) in each free
        /// cell, for each --check mechanism: variants with one piston fewer.
        #[arg(long)]
        shrink: bool,
        /// Replay a `door ideal` plan in the solver's world for each --check design.
        #[arg(long)]
        replay_ideal: Option<PathBuf>,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::RowsFire { tsv, wirable, only } => {
            crate::rowsfire::cli(&tsv, wirable, &only)?;
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
            region_box,
            forbid,
            forbid_ring,
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
            if region_box.is_some() || !forbid.is_empty() || forbid_ring {
                let first = crate::ideal::Schedule::parse(&text)?;
                let d = first.design();
                let (blo, bhi) = d.bounds().context("empty design")?;
                let (lo, hi) = match &region_box {
                    Some(b) => {
                        let (a, z) = b.split_once(':').context("--box x0,y0,z0:x1,y1,z1")?;
                        let p = |s: &str| -> Result<[i32; 3]> {
                            let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>().map_err(anyhow::Error::from)).collect::<Result<_>>()?;
                            anyhow::ensure!(v.len() == 3, "--box corner {s:?}: expected x,y,z");
                            Ok([v[0], v[1], v[2]])
                        };
                        (p(a)?, p(z)?)
                    }
                    None => ([blo[0] - margin, blo[1] - margin, blo[2] - margin], [bhi[0] + margin, bhi[1] + margin, bhi[2] + margin]),
                };
                let mut cells = crate::synth::parse_cells(&forbid)?;
                if forbid_ring {
                    let door = *d.ports.get("door").context("--forbid-ring: the design has no door port")?;
                    for t in 0..=2 {
                        for c in [[-1, t, 0], [3, t, 0], [t, -1, 0], [t, 3, 0]] {
                            cells.push([door[0] + c[0], door[1] + c[1], door[2] + c[2]]);
                        }
                    }
                }
                println!("routing region {lo:?}..{hi:?}, {} forbidden cells", cells.len());
                *mechwire::REGION.write().unwrap() = Some(mechwire::Region { lo, hi, forbid: cells });
            }
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
        DoorCmd::Mechspace { cmd, boxes, input, pistons, cap, fcap, threads, cap_local, max_group, out_dir, tag, limit, two, carry, vmid, catalog, n, walk, astar, gap, workers, extra_offsets, cap_lean, no_door_bodies, full_frame, ext_pullers, bottom_su, frame9, door_offset } => {
            use crate::mechspace as ms;
            if no_door_bodies {
                ms::NO_DOOR_BODIES.store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!("[mechspace] sub-model: no piston body inside the 3x3");
            }
            if std::env::var("MECHSPACE_SWEEP").is_ok() {
                ms::SWEEP.store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!("[mechspace] sweep mode: exact on small sets, A* cycle search, exact up to --cap-lean");
            }
            if full_frame {
                ms::FULL_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!("[mechspace] FULL FRAME: frame cells air when open stay air on the cycle (static stones there), frame 12/12 checked on the augmented mechanism");
            }
            if ext_pullers {
                crate::families::EXT_PULLERS.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            if bottom_su {
                crate::families::BOTTOM_SU.store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!("[mechspace] generator: exactly one piston in the bottom row, sticky facing up");
            }
            if frame9 {
                ms::FRAME9.store(true, std::sync::atomic::Ordering::Relaxed);
                eprintln!("[mechspace] open = all 9 door blocks in frame cells");
            }
            if let Some(o) = &door_offset {
                let (a, b) = o.split_once(',').ok_or_else(|| anyhow::anyhow!("--door-offset dx,dy"))?;
                let _ = ms::DOOR_OFFSET.set((a.parse()?, b.parse()?));
            }
            let vm: Vec<Vec<(i8, i8)>> = match vmid.as_deref() {
                None => Vec::new(),
                Some("ml") => vec![vec![(0, 1)]],
                Some("mr") => vec![vec![(2, 1)]],
                Some("any") => vec![vec![(0, 1)], vec![(2, 1)]],
                Some("both") => vec![vec![(0, 1), (2, 1)]],
                Some(x) => anyhow::bail!("--vmid ml|mr|any|both, got {x}"),
            };
            let bxs = crate::families::parse_boxes(&boxes)?;
            let mk = |t: String| ms::RunOpts { cap, fcap, threads, cap_local, max_group, out: out_dir.clone(), tag: t, astar, cap_lean };
            match cmd.as_str() {
                "selftest" => {
                    for &(w, h) in &bxs {
                        ms::selftest(w, h, pistons, n, walk, threads)?;
                    }
                }
                "validate" | "gen" => {
                    let cat = ms::catalog_set(&catalog)?;
                    for &(w, h) in &bxs {
                        let mut items = ms::candidates(w, h, pistons, two, carry, &vm);
                        eprintln!("[mechspace] {w}x{h}: {} candidates", items.len());
                        if limit > 0 && items.len() > limit {
                            items.truncate(limit);
                        }
                        let t = tag.clone().unwrap_or_else(|| format!("{cmd}-{w}x{h}"));
                        let o = mk(t);
                        let found = ms::run_list(&items, &o, cmd == "validate")?;
                        ms::handle_found(&found, &o, &cat)?;
                    }
                }
                "decide" | "validate-list" => {
                    let cat = ms::catalog_set(&catalog)?;
                    let f = input.ok_or_else(|| anyhow::anyhow!("decide needs --input"))?;
                    anyhow::ensure!(bxs.len() == 1, "decide needs one --boxes WxH");
                    let mut items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
                    if limit > 0 && items.len() > limit {
                        items.truncate(limit);
                    }
                    let t = tag.clone().unwrap_or_else(|| format!("{cmd}-{}x{}", bxs[0].0, bxs[0].1));
                    let o = mk(t);
                    let found = ms::run_list(&items, &o, cmd == "validate-list")?;
                    ms::handle_found(&found, &o, &cat)?;
                }
                "diag" | "diag2" | "diag3" => {
                    let f = input.ok_or_else(|| anyhow::anyhow!("diag needs --input"))?;
                    let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
                    let (bx, v) = &items[limit.min(items.len() - 1)];
                    if cmd == "diag" {
                        ms::diag(*bx, v, cap, cap_local, max_group)?;
                    } else if cmd == "diag3" {
                        let (m, rest) = crate::families::Mech::new(*bx, v).ok_or_else(|| anyhow::anyhow!("invalid"))?;
                        let cx = ms::Cx::new(&m, &rest);
                        let t0 = std::time::Instant::now();
                        let (g, c, nst) = ms::search_lean(&cx, std::slice::from_ref(&rest), &ms::Goal::Open, &Default::default(), cap, true, false);
                        println!("plain lean open search: {nst} states, complete {c}, {} open, {:.1}s ({:.0} states/s)", g.len(), t0.elapsed().as_secs_f64(), nst as f64 / t0.elapsed().as_secs_f64());
                    } else {
                        ms::diag2(*bx, v, cap)?;
                    }
                }
                "refit" => {
                    let cands: Vec<(i8, i8, Vec<(crate::families::Bx, Vec<crate::families::Pc>)>)> = bxs.iter().map(|&(w, h)| (w, h, ms::candidates(w, h, pistons, two, carry, &vm))).collect();
                    ms::refit(&catalog, &bxs, &cands, cap, threads)?;
                }
                "pretest" => {
                    for &(w, h) in &bxs {
                        ms::pretest(w, h, pistons, n, walk, threads)?;
                    }
                }
                "coreach" => {
                    let f = input.ok_or_else(|| anyhow::anyhow!("coreach needs --input"))?;
                    let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
                    let (bx, v) = &items[limit.min(items.len() - 1)];
                    let (m, rest) = crate::families::Mech::new(*bx, v).ok_or_else(|| anyhow::anyhow!("invalid"))?;
                    let t0 = std::time::Instant::now();
                    let (c, complete) = ms::coreach(&m, &rest, cap);
                    println!("co-reachable (can reach rest): {} states, complete {complete} ({:.1}s)", c.len(), t0.elapsed().as_secs_f64());
                }
                "catlist" => {
                    for &(w, h) in &bxs {
                        for l in ms::catalog_rows(&catalog, w, h)? {
                            println!("{l}");
                        }
                    }
                }
                "export" => {
                    let f = input.ok_or_else(|| anyhow::anyhow!("export needs --input (lines: id x0 y0 pieces)"))?;
                    anyhow::ensure!(bxs.len() == 1, "export needs one --boxes WxH");
                    let items = ms::read_ided(&f, bxs[0].0, bxs[0].1)?;
                    let t = tag.clone().unwrap_or_else(|| format!("list-{}x{}", bxs[0].0, bxs[0].1));
                    ms::export(version, &items, &out_dir, &t, cap, gap, workers, extra_offsets)?;
                }
                "helpers" => {
                    let cat = ms::catalog_set(&catalog)?;
                    let f = input.ok_or_else(|| anyhow::anyhow!("helpers needs --input"))?;
                    anyhow::ensure!(bxs.len() == 1, "helpers needs one --boxes WxH");
                    let base = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
                    let mut items = ms::helper_variants(&base, two.max(1));
                    eprintln!("[mechspace] {} base candidates -> {} helper variants", base.len(), items.len());
                    if limit > 0 && items.len() > limit {
                        items.truncate(limit);
                    }
                    let t = tag.clone().unwrap_or_else(|| format!("helpers-{}x{}", bxs[0].0, bxs[0].1));
                    let o = mk(t);
                    let found = ms::run_list(&items, &o, false)?;
                    ms::handle_found(&found, &o, &cat)?;
                }
                "frame-export" => {
                    // stone-free candidates (id x0 y0 pieces): solve, add static stones in the
                    // frame cells the open state leaves air, replay-check, export catalog-style
                    let f = input.ok_or_else(|| anyhow::anyhow!("frame-export needs --input"))?;
                    anyhow::ensure!(bxs.len() == 1, "frame-export needs one --boxes WxH");
                    let items = ms::read_ided(&f, bxs[0].0, bxs[0].1)?;
                    let mut aug_items = Vec::new();
                    for (id, bx, v) in &items {
                        let Some((m, rest)) = crate::families::Mech::new(*bx, v) else { continue };
                        let cx = ms::Cx::new(&m, &rest);
                        let (vd, _) = ms::decide_all(&cx, cap.min(500_000), cap_lean.max(cap), 0);
                        if let ms::Verdict::Found { open, .. } = vd {
                            match ms::augment_frame(*bx, v, &open, cap, cap_lean) {
                                Some((aug, _, _)) => aug_items.push((id.clone(), *bx, aug)),
                                None => eprintln!("[mechspace] {id}: no static-stone frame"),
                            }
                        } else {
                            eprintln!("[mechspace] {id}: {}", vd.tag());
                        }
                    }
                    let t = tag.clone().unwrap_or_else(|| format!("frame-{}x{}", bxs[0].0, bxs[0].1));
                    ms::export(version, &aug_items, &out_dir, &t, cap, gap, workers, extra_offsets)?;
                }
                "enum" => {
                    // piston-set enumeration around the door, fewest pistons first, every survivor
                    // decided exactly
                    let cat = ms::catalog_set(&catalog)?;
                    anyhow::ensure!(bxs.len() == 1, "enum needs one --boxes WxH");
                    let (dx, dy) = ms::DOOR_OFFSET.get().copied().ok_or_else(|| anyhow::anyhow!("enum needs --door-offset"))?;
                    let bx = crate::families::Bx { x0: -dx, y0: -dy, w: bxs[0].0, h: bxs[0].1 };
                    let eo = ms::EnumOpts { side_absent: two, free_max: carry.max(1), free_normal: std::env::var("MECHSPACE_FREE_NORMAL").is_ok(), bottom_su };
                    let t0 = std::time::Instant::now();
                    let sets = ms::enum_sets(bx, &eo);
                    if let Ok(exp) = std::env::var("MECHSPACE_EXPECT") {
                        let mut want: Vec<crate::families::Pc> = exp.split_whitespace().map(ms::parse_piece).collect::<anyhow::Result<_>>()?;
                        want.sort();
                        println!("[mechspace enum] expected set {} generated: {}", exp, sets.iter().any(|v| *v == want));
                    }
                    let mut items: Vec<(crate::families::Bx, Vec<crate::families::Pc>)> = sets.into_iter().map(|v| (bx, v)).collect();
                    eprintln!("[mechspace enum] {} piston sets survive pruning ({:.1}s)", items.len(), t0.elapsed().as_secs_f64());
                    if limit > 0 && items.len() > limit {
                        items.truncate(limit);
                    }
                    let t = tag.clone().unwrap_or_else(|| format!("enum-{}x{}", bxs[0].0, bxs[0].1));
                    let o = mk(t);
                    let found = ms::run_list(&items, &o, false)?;
                    ms::handle_found(&found, &o, &cat)?;
                }
                "movable" => {
                    // rule (A): candidates without a static-extras cycle, retried with 3 movable
                    // extra door-type blocks in the open state's empty frame cells
                    let f = input.ok_or_else(|| anyhow::anyhow!("movable needs --input"))?;
                    anyhow::ensure!(bxs.len() == 1, "movable needs one --boxes WxH");
                    let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
                    let done = std::sync::atomic::AtomicUsize::new(0);
                    let found = std::sync::Mutex::new(Vec::new());
                    let next = std::sync::atomic::AtomicUsize::new(0);
                    std::thread::scope(|s| {
                        for _ in 0..threads.max(1) {
                            s.spawn(|| loop {
                                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                if i >= items.len() {
                                    break;
                                }
                                let (bx, v) = &items[i];
                                if let Some((aug, op, cl)) = ms::try_movable(*bx, v, cap, cap_lean) {
                                    let line = format!("{}\t{}\t{}+{}", ms::line_of(bx, &aug), ms::line_of(bx, v), op.len(), cl.len());
                                    println!("MOVABLE {line}");
                                    found.lock().unwrap().push(line);
                                }
                                let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                                if d % 100 == 0 {
                                    eprintln!("[mechspace movable] {d}/{}", items.len());
                                }
                            });
                        }
                    });
                    let found = found.into_inner().unwrap();
                    let t = tag.clone().unwrap_or_else(|| "movable".into());
                    std::fs::write(out_dir.join(format!("{t}.movable.tsv")), found.join("\n") + "\n")?;
                    eprintln!("[mechspace movable] {} of {} candidates cycle with movable extras", found.len(), items.len());
                }
                c => anyhow::bail!("mechspace: unknown command {c}"),
            }
        }
        DoorCmd::Families { boxes, pistons, cap, sym_cap, threads, no_lifts, no_pushes, out_dir, verify, workers, extra_offsets, gap, limit, two, check, check_margin, levels, rows, row_pistons, arm, row_max_width, resolve, carry, vmid, forbid_fire, rblocks, r_both } => {
            let vmid: Vec<Vec<(i8, i8)>> = match vmid.as_deref() {
                None => Vec::new(),
                Some("ml") => vec![vec![(0, 1)]],
                Some("mr") => vec![vec![(2, 1)]],
                Some("any") => vec![vec![(0, 1)], vec![(2, 1)]],
                Some("both") => vec![vec![(0, 1), (2, 1)]],
                Some(x) => anyhow::bail!("--vmid ml|mr|any|both, got {x}"),
            };
            if let Some(f) = resolve {
                let b = crate::families::parse_boxes(&boxes)?;
                anyhow::ensure!(b.len() == 1, "--resolve needs one --boxes WxH");
                return crate::families::resolve_cli(&f, b[0].0, b[0].1, cap, threads);
            }
            if rows {
                let _ = crate::families::RMAX.set(rblocks);
                let _ = crate::families::RBOTH.set(r_both);
                if !forbid_fire.is_empty() {
                    let _ = crate::families::FORBID_X.set(forbid_fire.clone());
                    eprintln!("[families rows] no piston fires from x in {forbid_fire:?}");
                }
                let o = crate::families::RowOpts { kmax: row_pistons, arm, cap, sym_cap, threads, out_dir, max_w: row_max_width, gap };
                return crate::families::rows_cli(&o);
            }
            if let Some(p) = check {
                return crate::families::check(
                    &p,
                    check_margin,
                    cap,
                    sym_cap,
                    verify.then(|| crate::families::CheckVerify {
                        version: version.to_string(),
                        out_dir: out_dir.clone(),
                        gap,
                        extra: extra_offsets as u32,
                        name: p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "check".into()),
                    }),
                );
            }
            let o = crate::families::Opts {
                boxes: crate::families::parse_boxes(&boxes)?,
                pmax: pistons,
                cap,
                sym_cap,
                threads,
                lifts: !no_lifts,
                pushes: !no_pushes,
                out_dir,
                verify,
                workers,
                extra: extra_offsets,
                gap,
                limit,
                two,
                carry,
                vmid,
                levels,
            };
            crate::families::run(version, &o)?;
        }
        DoorCmd::Topology { tsv, only, verbose } => {
            crate::topology::catalog_cli(&tsv, &only, verbose)?;
        }
        DoorCmd::Connect { tsv, only, boxes, cap, threads, verbose } => {
            let b = crate::families::parse_boxes(&boxes)?;
            crate::connect::catalog_cli(&tsv, &only, &b, cap, threads, verbose)?;
        }
        DoorCmd::MechSynth { base, add, lo, hi, kinds, ext, cap, threads, max_blocks, sym_len, sym_tries, out_dir, top, verify, extra_offsets, gap, workers, just_base, require, pulses_only, edges, split_only, seamless, no_ring_fire, sym_bfs, sym_levels, topology, seamless_open, oc } => {
            crate::mechsynth::SEAMLESS.store(seamless, std::sync::atomic::Ordering::Relaxed);
            crate::mechsynth::NO_RING_FIRE.store(no_ring_fire, std::sync::atomic::Ordering::Relaxed);
            crate::mechsynth::SEAMLESS_OPEN.store(seamless_open, std::sync::atomic::Ordering::Relaxed);
            if let Some(oc) = &oc {
                return crate::mechsynth::oc_cli(version, &base, oc, out_dir.as_deref(), verify, extra_offsets, gap);
            }
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
                topology: match topology.as_deref() {
                    None => None,
                    Some(s) => {
                        let v: Vec<i8> = s.split([',', ':']).map(|t| t.trim().parse::<i8>()).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!("--topology x0,y0:x1,y1: {e}"))?;
                        anyhow::ensure!(v.len() == 4, "--topology x0,y0:x1,y1, got {s:?}");
                        Some(([v[0].min(v[2]), v[1].min(v[3])], [v[0].max(v[2]), v[1].max(v[3])]))
                    }
                },
            };
            crate::mechsynth::cli(version, &base, &o, out_dir.as_deref(), top, verify, extra_offsets, gap, workers, just_base)?;
        }
        DoorCmd::ObsChain { spec, out, max_paths, secs, shift, restarts } => {
            let mut w = Worker::spawn(&cfg)?;
            crate::obschain::run(&mut w, &spec, &out, max_paths, secs, shift, restarts)?;
        }
        DoorCmd::Fullmech { check, boxes, pistons, two, vmid, pulses, levels, cap, threads, limit, out, macro_cap, only_new, reduce, reduce_cap, pairs, anneal, offsets, seconds, seed, helpers, export, slime, verify_ideal, groups, shrink, replay_ideal } => {
            let boxes = match boxes {
                Some(b) => crate::families::parse_boxes(&b)?,
                None => Vec::new(),
            };
            let vmid: Vec<Vec<(i8, i8)>> = match vmid.as_deref() {
                None => Vec::new(),
                Some("ml") => vec![vec![(0, 1)]],
                Some("mr") => vec![vec![(2, 1)]],
                Some("any") => vec![vec![(0, 1)], vec![(2, 1)]],
                Some("both") => vec![vec![(0, 1), (2, 1)]],
                Some(x) => anyhow::bail!("--vmid ml|mr|any|both, got {x}"),
            };
            let anneal = match anneal {
                Some(b) => crate::families::parse_boxes(&b)?.first().copied(),
                None => None,
            };
            let pulses: Vec<u8> = pulses.split(',').filter(|s| !s.is_empty()).map(|s| s.trim().parse::<u8>()).collect::<std::result::Result<_, _>>()?;
            let o = crate::fullmech::CliOpts { check, boxes, pmax: pistons, two, vmid, pulses, levels, cap, threads, limit, out, macro_cap, only_new, reduce, reduce_cap, pairs, anneal, offsets, seconds, seed, helpers, slime, export, verify_ideal, groups, shrink, replay_ideal };
            crate::fullmech::cli(&o)?;
        }
    }
    Ok(())
}
