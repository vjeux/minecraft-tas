//! `door shuttle`: feed a plan's inputs from a shuttled observer (piston-door shuttle.rs).

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct ShuttleArgs {
    /// The `door ideal` plan(s); several = batch mode, one line per plan (and target group).
    #[arg(required = true, num_args = 1..)]
    schedule: Vec<PathBuf>,
    /// Search each target cell on its own (with --unfed: every unfed input separately).
    #[arg(long)]
    per_cell: bool,
    /// Plan input cells the shuttle replaces, `x,y,z` (repeatable, or `;`-separated): every pulse
    /// of these cells must come from the shuttled observer.
    #[arg(long)]
    target: Vec<String>,
    /// Also target every input with a firing `door feeds` finds no option for.
    #[arg(long)]
    unfed: bool,
    /// Only the target pulses of one edge: `open` or `close` (default: both).
    #[arg(long)]
    edge: Option<String>,
    /// Also keep shuttles that replace only some of the target pulses (most first; the rest stay
    /// ideal): load relief for an overloaded area.
    #[arg(long)]
    partial: bool,
    /// The plan is ONE pulse train for both edges (`door interleave`): spare ticks are dropped only
    /// with both edges' copies together.
    #[arg(long)]
    train: bool,
    /// Ticks the shuttle piston stays powered per pulse (>= 3: a sticky piston drops its block on
    /// shorter pulses), comma separated.
    #[arg(long, default_value = "3,4,5,6,8,10,12")]
    holds: String,
    /// Ticks from the shuttle piston's switch to the plan tick it stands for (4 = 2 gt travel +
    /// the observer's 2 gt), comma separated.
    #[arg(long, default_value = "4")]
    leads: String,
    /// The same for a carried redstone block (it powers on landing).
    #[arg(long, default_value = "2,3")]
    rb_leads: String,
    /// What the shuttle carries: `observer` (every in-plane facing), `redstone_block` (powers what
    /// touches its landing cell while it sits there), comma separated.
    #[arg(long, default_value = "observer")]
    payload: String,
    /// The shuttle's cells within this in-plane distance of a piston the targets feed (0 = the
    /// whole box).
    #[arg(long, default_value_t = 3)]
    radius: i32,
    /// Grow the design's box in the plane by this many cells (0 keeps the volume).
    #[arg(long, default_value_t = 0)]
    margin: i32,
    /// Extra pseudo-random positions for confirming (screening uses the four default ones).
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    cycles: u32,
    #[arg(long, default_value_t = 32)]
    threads: usize,
    /// Combinations of the per-pulse options judged per placement.
    #[arg(long, default_value_t = 400)]
    max_combos: usize,
    /// Then drop other plan pulses that the observer's free pulses (the other end of each shuttle
    /// pulse) or extra shuttle pulses give instead (greedy, earliest first).
    #[arg(long)]
    absorb: bool,
    /// Write each working variant here (`<stem>-sh<k>.txt` + `.ideal`).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Variants confirmed, printed and written.
    #[arg(long, default_value_t = 10)]
    top: usize,
    /// Every input between ticks (the old rule), like `door ideal-rs --between-ticks`.
    #[arg(long)]
    between_ticks: bool,
}

fn list<T: std::str::FromStr>(s: &str) -> Result<Vec<T>>
where
    T::Err: std::fmt::Display,
{
    s.split(',').map(|t| t.trim().parse::<T>().map_err(|e| anyhow::anyhow!("{t:?}: {e}"))).collect()
}

pub fn cmd_shuttle(args: ShuttleArgs) -> Result<()> {
    let ShuttleArgs { schedule, per_cell, target, unfed, edge, partial, train, holds, leads, rb_leads, payload, radius, margin, extra_offsets, cycles, threads, max_combos, absorb, out_dir, top, between_ticks } = args;
    let edge = match edge.as_deref() {
        None => None,
        Some("open") => Some(true),
        Some("close") => Some(false),
        Some(e) => anyhow::bail!("--edge open|close, not {e}"),
    };
    crate::ideal::cli_phase(between_ticks);
    let mut targets = Vec::new();
    for t in target.iter().flat_map(|t| t.split(';')) {
        let v: Vec<i32> = list(t)?;
        anyhow::ensure!(v.len() == 3, "--target x,y,z");
        targets.push([v[0], v[1], v[2]]);
    }
    anyhow::ensure!(unfed || !targets.is_empty(), "give --target cells or --unfed");
    let holds: Vec<u32> = list(&holds)?;
    anyhow::ensure!(holds.iter().all(|&h| h >= 3), "--holds: every hold >= 3 (a sticky piston drops its block on a 1-2 gt pulse)");
    let payloads: Vec<String> = payload.split(',').map(|s| s.trim().to_string()).collect();
    anyhow::ensure!(payloads.iter().all(|p| p == "observer" || p == "redstone_block"), "--payload: observer, redstone_block");
    let o = crate::shuttle::Options { targets, edge, unfed, partial, train, holds, leads: list(&leads)?, rb_leads: list(&rb_leads)?, radius, margin, extra_offsets, cycles, threads, max_combos, absorb, out_dir, top, payloads };
    crate::shuttle::cli(&schedule, &o, per_cell)
}
