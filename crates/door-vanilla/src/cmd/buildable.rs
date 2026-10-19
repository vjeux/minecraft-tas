//! `door buildable`: survival build programs.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct BuildableArgs {
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
}

pub fn cmd_buildable(version: &str, args: BuildableArgs) -> Result<()> {
    let BuildableArgs { designs, budget, workers, helpers, positions, seed, out_dir, table, lever, verbose } = args;
    let lever = match lever.as_str() {
        "both" => None,
        "off" => Some(false),
        "on" => Some(true),
        other => anyhow::bail!("--lever {other}: expected both, off or on"),
    };
    let o = crate::buildable::CliOptions { budget, workers, helpers, positions, seed, verbose, lever };
    crate::buildable::run_cli(version, &designs, &o, out_dir.as_deref(), table.as_deref())?;
    Ok(())
}
