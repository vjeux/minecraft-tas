//! `door mech-synth`, `door obs-chain`, `door fullmech`: mechanism synthesis and searches.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct MechSynthArgs {
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
}

pub fn cmd_mech_synth(version: &str, args: MechSynthArgs) -> Result<()> {
    let MechSynthArgs { base, add, lo, hi, kinds, ext, cap, threads, max_blocks, sym_len, sym_tries, out_dir, top, verify, extra_offsets, gap, workers, just_base, require, pulses_only, edges, split_only, seamless, no_ring_fire, sym_bfs, sym_levels, topology, seamless_open, oc } = args;
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
    Ok(())
}

#[derive(clap::Args)]
pub struct ObsChainArgs {
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
}

pub fn cmd_obs_chain(version: &str, args: ObsChainArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let ObsChainArgs { spec, out, max_paths, secs, shift, restarts } = args;
    let mut w = Worker::spawn(&cfg)?;
    crate::obschain::run(&mut w, &spec, &out, max_paths, secs, shift, restarts)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct FullmechArgs {
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
    /// Generated candidates: pullers RETRACTED at rest only for 1-move exits (families'
    /// --no-ext-pullers; by default extended-at-rest pullers are allowed too).
    #[arg(long)]
    no_ext_pullers: bool,
    /// Accepted for old command lines: extended-at-rest pullers are the default now.
    #[arg(long, hide = true)]
    ext_pullers: bool,
    /// Generated candidates: most 1-move exits by a puller extended at rest per mechanism
    /// (families' --ext-max).
    #[arg(long, default_value_t = 3)]
    ext_max: usize,
    /// Generated candidates: extended-puller exits may sit anywhere (families' --ext-apart;
    /// default: each next to an earlier one).
    #[arg(long)]
    ext_apart: bool,
    /// Generated candidates: audit bound (a), extended-at-rest pistons reused lifted / carried.
    #[arg(long)]
    lift_ext: bool,
    /// --check: keep the design's plain stones / obsidian as pieces (static frame stones of an
    /// ideal-input design; by default plain blocks are taken for wiring and dropped).
    #[arg(long)]
    keep_stone: bool,
    /// --anneal: every restart begins from this design's pieces (pistons, slime, honey; plain
    /// stones with --keep-stone).
    #[arg(long)]
    anneal_seed: Option<PathBuf>,
    /// --anneal-seed: the seed's first N pieces (file order) are never mutated (0: none).
    #[arg(long, default_value_t = 0)]
    anneal_freeze: usize,
    /// Open also needs a door block in each of the 12 frame cells (the boards' FULL FRAME rule).
    #[arg(long)]
    frame_full: bool,
    /// Exhaustive completion of --anneal-seed: every placement of --complete-k pieces from
    /// --complete-palette in these cells (`x,y;x,y`, door coordinates), each solved exactly, in the
    /// --anneal box at the first --offsets offset.
    #[arg(long)]
    complete: Option<String>,
    #[arg(long, default_value_t = 2)]
    complete_k: usize,
    /// Comma list: pistons, sticky (sticky pistons only), slime, honey, stone, obsidian, rb, obs,
    /// all.
    #[arg(long, default_value = "pistons,slime,stone")]
    complete_palette: String,
}

pub fn cmd_fullmech(args: FullmechArgs) -> Result<()> {
    let FullmechArgs { check, boxes, pistons, two, vmid, pulses, levels, cap, threads, limit, out, macro_cap, only_new, reduce, reduce_cap, pairs, anneal, offsets, seconds, seed, helpers, export, slime, verify_ideal, groups, shrink, replay_ideal, no_ext_pullers, ext_pullers: _, ext_max, ext_apart, lift_ext, keep_stone, anneal_seed, anneal_freeze, frame_full, complete, complete_k, complete_palette } = args;
    crate::fullmech::KEEP_STONE.store(keep_stone, std::sync::atomic::Ordering::Relaxed);
    crate::fullmech::FRAME_FULL.store(frame_full, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_PULLERS.store(!no_ext_pullers, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_MAX.store(ext_max, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_ADJACENT.store(!ext_apart, std::sync::atomic::Ordering::Relaxed);
    crate::families::LIFT_EXT.store(lift_ext, std::sync::atomic::Ordering::Relaxed);
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
    let o = crate::fullmech::CliOpts { check, boxes, pmax: pistons, two, vmid, pulses, levels, cap, threads, limit, out, macro_cap, only_new, reduce, reduce_cap, pairs, anneal, offsets, seconds, seed, helpers, slime, export, verify_ideal, groups, shrink, replay_ideal, anneal_seed, anneal_freeze, complete, complete_k, complete_palette };
    crate::fullmech::cli(&o)?;
    Ok(())
}
