//! `door gallery`, `door montage`: board GIFs and record montages.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct GalleryArgs {
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    archive: PathBuf,
    /// Folder for the GIFs.
    #[arg(long)]
    videos: PathBuf,
    /// The clips folder (shared with `door video` and `video-all`; default ~/videos/clips).
    #[arg(long)]
    clips: Option<PathBuf>,
    /// GitHub release tag to upload the GIFs to.
    #[arg(long)]
    upload: Option<String>,
    /// Client recordings at once (leaders without a clip).
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
    /// Record the leaders' clips again (and rebuild their GIFs). Not needed for new labels:
    /// a GIF is rebuilt whenever its clip or label changed.
    #[arg(long, conflicts_with = "gif_only")]
    redo: bool,
    /// Record nothing: rebuild every GIF from the clips there are.
    #[arg(long)]
    gif_only: bool,
}

pub fn cmd_gallery(version: &str, args: GalleryArgs) -> Result<()> {
    let GalleryArgs { archive, videos, clips, upload, jobs, workers, orbit, gif_width, gif_fps, speed, redo, gif_only } = args;
    let clips = clips.unwrap_or_else(crate::clips::default_dir);
    let o = crate::showcase::GalleryOptions { archive, videos, clips, upload, jobs, workers, orbit, gif_width, gif_fps, speed, redo, gif_only };
    crate::showcase::gallery(version, &o)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct MontageArgs {
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
}

pub fn cmd_montage(version: &str, args: MontageArgs) -> Result<()> {
    let MontageArgs { archive, videos, out, upload, jobs, workers, speed, redo, board } = args;
    let o = crate::showcase::MontageOptions { archive, videos, out, upload, jobs, workers, speed, redo, board };
    crate::showcase::montage(version, &o)?;
    Ok(())
}
