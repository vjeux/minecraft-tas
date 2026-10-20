//! `door selffeed`: replace a plan's ideal inputs by observers watching the mechanism.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct SelffeedArgs {
    /// The `door ideal` plan.
    schedule: PathBuf,
    /// Extra pseudo-random positions for confirming a substitution (screening uses the four
    /// default ones).
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    cycles: u32,
    #[arg(long, default_value_t = 32)]
    threads: usize,
    /// Grow the design's own box in the plane by this many cells for observer cells.
    #[arg(long, default_value_t = 0)]
    margin: i32,
    /// Write `<out>.ideal` (the plan left) and `<out>.txt` (the design with its observers).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Only list every working substitution of each input (no greedy rounds).
    #[arg(long)]
    list: bool,
    /// Inputs never replaced, `x,y,z` (repeatable).
    #[arg(long)]
    keep: Vec<String>,
    /// Every input between ticks (the old rule), like `door ideal-rs --between-ticks`.
    #[arg(long)]
    between_ticks: bool,
    /// Print why each rejected placement fails.
    #[arg(long)]
    why: bool,
    /// Only print, for each input, its pulses and the change ticks of the cell each candidate
    /// observer would watch; FITS = a change 2 gt before every pulse within this slack.
    #[arg(long)]
    align: Option<i64>,
    /// Shifts tried for the rest of the plan (every event from the replaced input's first pulse
    /// on), per edge, comma separated.
    #[arg(long, default_value = "0,-2,2,-4,4,6,8")]
    shifts: String,
}

pub fn cmd_selffeed(args: SelffeedArgs) -> Result<()> {
    let SelffeedArgs { schedule, extra_offsets, cycles, threads, margin, out, list, keep, between_ticks, why, align, shifts } = args;
    let shifts: Vec<i64> = shifts.split(',').map(|t| t.trim().parse::<i64>()).collect::<std::result::Result<_, _>>()?;
    crate::ideal::cli_phase(between_ticks);
    let mut kept = Vec::new();
    for k in keep {
        let v: Vec<i32> = k.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--keep x,y,z");
        kept.push([v[0], v[1], v[2]]);
    }
    let o = crate::selffeed::Options { extra_offsets, cycles, threads, margin, out, list, keep: kept, why, align, shifts };
    crate::selffeed::cli(&schedule, &o)
}

#[derive(clap::Args)]
pub struct InterleaveArgs {
    /// The `door ideal` plan(s) (open and close sequences); several = batch mode, one summary
    /// line each, in parallel.
    #[arg(required = true, num_args = 1..)]
    schedule: Vec<PathBuf>,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    cycles: u32,
    /// Ticks between two steps of the merged train.
    #[arg(long, default_value_t = 4)]
    gap: u32,
    /// Write the merged plan (the same events on both edges).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Batch mode: write `<stem>-train.ideal` here for every plan with a train.
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Batch mode: plans searched in parallel.
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// Every input between ticks (the old rule), like `door ideal-rs --between-ticks`.
    #[arg(long)]
    between_ticks: bool,
    /// One step per input cell instead of per tick group.
    #[arg(long)]
    split: bool,
    /// Best-first search over ANY input cell pulses on both edges at once (not the plan's own
    /// steps), up to this many world pairs.
    #[arg(long)]
    search: Option<usize>,
    /// Weight of the heuristic in --search (f = steps + weight * cells off the targets).
    #[arg(long, default_value_t = 2)]
    weight: usize,
    /// --search also offers the plan's own tick groups (several inputs with their relative
    /// timing) as moves.
    #[arg(long)]
    groups: bool,
    /// --search also offers every pair of inputs pulsed together (same tick) as moves.
    #[arg(long)]
    pairs: bool,
    /// Input cells always pulsed together, `x,y,z+x,y,z[+...]` (repeatable): one source feeding
    /// several pistons (e.g. a strongly powered block between them).
    #[arg(long)]
    merge: Vec<String>,
    /// Search cost of a move that pulses this cell, `x,y,z=k` (repeatable; default 1): trains that
    /// pulse it less often.
    #[arg(long)]
    cost: Vec<String>,
    /// A small pocket: the input cells it feeds and the ticks it can make in the whole train,
    /// `x,y,z+x,y,z[+...]=k` (repeatable); any subset of them fires together as one tick.
    #[arg(long)]
    pocket: Vec<String>,
    /// Search, wire with `door satwire <--sat-args>`, and on UNSAT make the inputs next to the
    /// core pistons dearer and search again, up to this many rounds (needs --search, --out-dir).
    #[arg(long)]
    sat_loop: Option<usize>,
    /// The satwire arguments for --sat-loop (one string, split on spaces; no --out).
    #[arg(long, default_value = "")]
    sat_args: String,
    /// Blocks added to the design, `x,y,z=state` (repeatable): raw in both rest states.
    #[arg(long)]
    fix: Vec<String>,
    /// Inputs the train may not use, `x,y,z` (repeatable).
    #[arg(long)]
    drop: Vec<String>,
    /// Every filling of these pocket cells (`x,y,z;x,y,z`) with air or an in-plane observer is
    /// tried as `--fix` with the `--drop` inputs taken away (one search each, in parallel).
    #[arg(long)]
    observe: Option<String>,
}

pub fn cmd_interleave(args: InterleaveArgs) -> Result<()> {
    let InterleaveArgs { schedule, extra_offsets, cycles, gap, out, out_dir, threads, between_ticks, split, search, weight, groups, pairs, merge, cost, pocket, sat_loop, sat_args, fix, drop, observe } = args;
    crate::ideal::cli_phase(between_ticks);
    let parse_pos = |c: &str| -> Result<[i32; 3]> {
        let v: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "a cell is x,y,z: {c}");
        Ok([v[0], v[1], v[2]])
    };
    let mut costs = Vec::new();
    for c in &cost {
        let (p, k) = c.split_once('=').ok_or_else(|| anyhow::anyhow!("--cost x,y,z=k"))?;
        costs.push((parse_pos(p)?, k.trim().parse::<usize>()?));
    }
    let mut pockets = Vec::new();
    for c in &pocket {
        let (cells, k) = c.split_once('=').ok_or_else(|| anyhow::anyhow!("--pocket x,y,z+x,y,z=k"))?;
        let set: Vec<[i32; 3]> = cells.split('+').map(parse_pos).collect::<Result<_>>()?;
        pockets.push((set, k.trim().parse::<usize>()?));
    }
    let mut fixes = Vec::new();
    for f in &fix {
        let (p, st) = f.split_once('=').ok_or_else(|| anyhow::anyhow!("--fix x,y,z=state"))?;
        let st = if st.contains(':') { st.to_string() } else { format!("minecraft:{st}") };
        fixes.push((parse_pos(p)?, st));
    }
    let drops: Vec<[i32; 3]> = drop.iter().map(|d| parse_pos(d)).collect::<Result<_>>()?;
    let mut sets = Vec::new();
    for m in &merge {
        let mut set = Vec::new();
        for c in m.split('+') {
            let v: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "--merge x,y,z+x,y,z");
            set.push([v[0], v[1], v[2]]);
        }
        sets.push(set);
    }
    let quiet = schedule.len() > 1;
    let o = crate::interleave::Options { extra_offsets, cycles, gap, out, split, search, weight, quiet, groups, pairs, merge: sets, cost: costs, pockets, fix: fixes, drop: drops };
    if let Some(cells) = observe {
        let pocket: Vec<[i32; 3]> = cells.split(';').map(parse_pos).collect::<Result<_>>()?;
        anyhow::ensure!(search.is_some(), "--observe needs --search");
        return crate::interleave::observe(&schedule[0], &o, &pocket, threads, out_dir.as_deref());
    }
    if let Some(n) = sat_loop {
        let dir = out_dir.ok_or_else(|| anyhow::anyhow!("--sat-loop needs --out-dir"))?;
        anyhow::ensure!(search.is_some(), "--sat-loop needs --search");
        return crate::interleave::sat_loop(&schedule[0], &o, n, &sat_args, &dir);
    }
    if quiet {
        return crate::interleave::batch(&schedule, &o, out_dir.as_deref(), threads);
    }
    crate::interleave::cli(&schedule[0], &o)
}
