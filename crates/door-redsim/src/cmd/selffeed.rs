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
    /// The `door ideal` plan (open and close sequences).
    schedule: PathBuf,
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
}

pub fn cmd_interleave(args: InterleaveArgs) -> Result<()> {
    let InterleaveArgs { schedule, extra_offsets, cycles, gap, out, between_ticks, split, search, weight } = args;
    crate::ideal::cli_phase(between_ticks);
    crate::interleave::cli(&schedule, &crate::interleave::Options { extra_offsets, cycles, gap, out, split, search, weight })
}
