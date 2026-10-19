//! `door world`: the world export.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct WorldArgs {
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
}

pub fn cmd_world(version: &str, args: WorldArgs) -> Result<()> {
    let WorldArgs { archive, out, name, workers, shots, verdicts, check } = args;
    let o = crate::world::WorldOptions { archive, out, name, workers, shots, verdicts, check };
    let tar = crate::world::build(version, &o)?;
    println!("world packed: {}", tar.display());
    Ok(())
}
