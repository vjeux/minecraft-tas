//! `door video-all`, `door card`, `door video`, `door tick-video`: door videos, cut from one clip
//! per design.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct VideoAllArgs {
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    dir: PathBuf,
    /// Local folder for the videos.
    #[arg(long)]
    out: PathBuf,
    /// The clips folder (one recording per design, shared with `door video` and `door
    /// gallery`; default ~/videos/clips).
    #[arg(long)]
    clips: Option<PathBuf>,
    /// Client recordings (and cuts) at once.
    #[arg(long, default_value_t = 4)]
    jobs: usize,
    /// GitHub release tag to upload each new video to.
    #[arg(long)]
    upload: Option<String>,
    /// `card` (title card, then the clip with the size label: the volume big, the blocks
    /// placed small) or `clean` (the clip itself).
    #[arg(long, default_value = "card")]
    style: String,
    /// Orbit amplitude of the door camera in degrees each way (0 = still; part of the clip's
    /// settings: the gallery's GIFs need a still camera).
    #[arg(long, default_value_t = 0.0)]
    orbit: f64,
    /// Record every clip again.
    #[arg(long, conflicts_with = "relabel")]
    redo: bool,
    /// Record nothing: cut every video that has a clip again (new labels in seconds).
    #[arg(long)]
    relabel: bool,
    /// Only list the work (cuts and recordings); records and writes nothing.
    #[arg(long)]
    dry_run: bool,
}

pub fn cmd_video_all(version: &str, args: VideoAllArgs) -> Result<()> {
    let VideoAllArgs { dir, out, clips, jobs, upload, style, orbit, redo, relabel, dry_run } = args;
    let card = video_card(&style)?;
    let o = crate::clips::AllOptions {
        clips: clips.unwrap_or_else(crate::clips::default_dir),
        settings: crate::clips::ClipSettings { orbit, ..Default::default() },
        card,
        jobs,
        upload,
        redo,
        relabel,
        dry_run,
    };
    crate::clips::record_all(version, &dir, &out, &o)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct CardArgs {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Also write the label overlay here.
    #[arg(long)]
    label: Option<PathBuf>,
}

pub fn cmd_card(version: &str, args: CardArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let CardArgs { design, out, label } = args;
    let d = load_design(&design)?;
    let mut facts = crate::video::Facts::of(&d);
    let mut w = Worker::spawn(&cfg)?;
    if let Ok(r) = verify(&mut w, &d, &VerifyOptions { cycles: 1, offsets: vec![[0, 0, 0]], empty_walkway: false, ..Default::default() }) {
        facts.open_ticks = r.cycles.first().map(|c| c.open_ticks);
        facts.close_ticks = r.cycles.first().map(|c| c.close_ticks);
    }
    // The card's size: the volume (props cache, else measured and saved).
    facts.measure_volume(&mut w, &d)?;
    let look = crate::showcase::Look::load(version)?;
    look.card(1280, 720, &crate::showcase::title_lines(&facts)).save_png(&out)?;
    if let Some(p) = label {
        look.label(1280, 720, &crate::showcase::label_lines(&facts), true, 0.6).save_png(&p)?;
    }
    println!("wrote {}", out.display());
    Ok(())
}

#[derive(clap::Args)]
pub struct VideoArgs {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Simulation ticks per second while recording (20 = real time).
    #[arg(long, default_value_t = 10.0)]
    tps: f64,
    /// Upload to this GitHub release tag of vjeux/redstonepedia.
    #[arg(long)]
    upload: Option<String>,
    /// `card` (title card, then the clip with the size label), `clean` (the clip itself) or
    /// `classic` (still camera framing the whole build, recorded on its own, no clip).
    #[arg(long, default_value = "card")]
    style: String,
    /// Orbit amplitude of the door camera in degrees each way (0 = still; part of the clip's
    /// settings).
    #[arg(long, default_value_t = 0.0)]
    orbit: f64,
    /// Frame size, WIDTHxHEIGHT.
    #[arg(long, default_value = "1280x720")]
    size: String,
    /// The clips folder (shared with `door video-all` and `door gallery`; default
    /// ~/videos/clips).
    #[arg(long)]
    clips: Option<PathBuf>,
    /// Record the clip again even when it is there.
    #[arg(long, conflicts_with = "relabel")]
    redo: bool,
    /// Never record: cut the video from the clip there is (fails without one).
    #[arg(long)]
    relabel: bool,
}

pub fn cmd_video(version: &str, args: VideoArgs) -> Result<()> {
    let VideoArgs { design, out, tps, upload, style, orbit, size, clips, redo, relabel } = args;
    let d = load_design(&design)?;
    let (width, height) = size.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).ok_or_else(|| anyhow::anyhow!("--size wants WIDTHxHEIGHT"))?;
    if style == "classic" {
        let opts = crate::video::VideoOptions { tps, width, height, ..crate::video::VideoOptions::classic() };
        crate::video::record(version, &d, &out, &opts)?;
    } else {
        let s = crate::clips::ClipSettings { width, height, tps, orbit, ..Default::default() };
        let dir = clips.unwrap_or_else(crate::clips::default_dir);
        crate::clips::video_one(version, &dir, &d, &s, &out, video_card(&style)?, redo, relabel)?;
    }
    println!("recorded {}", out.display());
    if let Some(tag) = upload {
        let url = mctas::video::upload_release_asset("vjeux/redstonepedia", &tag, &out)?;
        println!("uploaded {url}");
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct TickVideoArgs {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Seconds each tick stays on screen.
    #[arg(long, default_value_t = 0.35)]
    secs_per_tick: f64,
    /// Milliseconds the client gets to draw each tick before the screenshot.
    #[arg(long, default_value_t = 500)]
    draw_ms: u64,
    /// Upload to this GitHub release tag of vjeux/redstonepedia.
    #[arg(long)]
    upload: Option<String>,
    /// Frame size, WIDTHxHEIGHT.
    #[arg(long, default_value = "1280x720")]
    size: String,
}

pub fn cmd_tick_video(version: &str, args: TickVideoArgs) -> Result<()> {
    let TickVideoArgs { design, out, secs_per_tick, draw_ms, upload, size } = args;
    let d = load_design(&design)?;
    let (width, height) = size.split_once('x').and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?))).ok_or_else(|| anyhow::anyhow!("--size wants WIDTHxHEIGHT"))?;
    let opts = crate::tickvideo::TickVideoOptions { width, height, secs_per_tick, draw_ms, ..Default::default() };
    crate::tickvideo::record(version, &d, &out, &opts)?;
    println!("recorded {}", out.display());
    if let Some(tag) = upload {
        let url = mctas::video::upload_release_asset("vjeux/redstonepedia", &tag, &out)?;
        println!("uploaded {url}");
    }
    Ok(())
}

/// Whether a published video's `--style` has the title card and label (`card`) or is the clip
/// itself (`clean`).
fn video_card(style: &str) -> Result<bool> {
    match style {
        "card" => Ok(true),
        "clean" => Ok(false),
        _ => anyhow::bail!("unknown --style {style} (card or clean; door video also takes classic)"),
    }
}
