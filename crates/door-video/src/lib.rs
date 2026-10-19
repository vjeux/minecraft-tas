//! `mctas door` commands: videos, cards and worlds: video-all, card, gallery, montage, video, tick-video, world.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Publish a video of every design in a folder (`<out>/<stem>.mp4`, uploaded), cut from each
    /// design's one clip (recorded first when missing or stale, several at once), then write the
    /// folder's catalog (README.md, see `door catalog`). A video is cut again only when its clip,
    /// card or label changed (seconds, no Minecraft).
    VideoAll {
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
    /// The best design of each board (size boards, speed boards) as a looping GIF (labeled with
    /// its size, the volume, big and its blocks placed small), cut from the design's one clip (the
    /// clips of `door video-all`, recorded here only when missing), uploaded to the release, plus
    /// the night's timeline chart. No page of its own: `door leaderboard` shows each board
    /// leader's GIF.
    Gallery {
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
    },
    /// A board's record progression by blocks placed (solid: archive/records.tsv, plus a newer
    /// record; the history before sizes were volumes) as one video: every design filmed by the
    /// same camera, labeled with its blocks placed and how it got there.
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
    /// Record the door opening and closing with the real Minecraft client (needs Xvfb) and publish
    /// it: the design's one clip (recorded only when missing or stale) cut into the video.
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
    /// in rows by board, smallest first, its size (the volume) above each in red concrete and its
    /// name on a floating label; built by the official server (see world.rs).
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
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::VideoAll { dir, out, clips, jobs, upload, style, orbit, redo, relabel, dry_run } => {
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
        }
        DoorCmd::Card { design, out, label } => {
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
        }
        DoorCmd::Gallery { archive, videos, clips, upload, jobs, workers, orbit, gif_width, gif_fps, speed, redo, gif_only } => {
            let clips = clips.unwrap_or_else(crate::clips::default_dir);
            let o = crate::showcase::GalleryOptions { archive, videos, clips, upload, jobs, workers, orbit, gif_width, gif_fps, speed, redo, gif_only };
            crate::showcase::gallery(version, &o)?;
        }
        DoorCmd::Montage { archive, videos, out, upload, jobs, workers, speed, redo, board } => {
            let o = crate::showcase::MontageOptions { archive, videos, out, upload, jobs, workers, speed, redo, board };
            crate::showcase::montage(version, &o)?;
        }
        DoorCmd::Video { design, out, tps, upload, style, orbit, size, clips, redo, relabel } => {
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
