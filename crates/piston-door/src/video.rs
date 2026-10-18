//! `mctas door video`: the door opening and closing, rendered by the real Minecraft client.

use crate::DoorSpec;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker, WorkerConfig};
use mctas::video::{self, Client, Display, Recorder};
use std::path::Path;
use std::time::{Duration, Instant};

pub struct VideoOptions {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Simulation ticks per second while recording (20 = real time).
    pub tps: f64,
    /// Ticks to play after each lever flip.
    pub ticks: u32,
}

impl Default for VideoOptions {
    fn default() -> Self {
        VideoOptions { width: 1280, height: 720, fps: 30, tps: 10.0, ticks: 40 }
    }
}

fn free_port() -> Result<u16> {
    let l = std::net::TcpListener::bind("127.0.0.1:0")?;
    Ok(l.local_addr()?.port())
}

/// Place a design exactly as written, compute wire shapes, let blocks check their inputs, settle.
pub fn place(w: &mut Worker, sb: u32, d: &Design) -> Result<()> {
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    w.step_until_idle(sb, 400)?;
    Ok(())
}

pub fn record(version: &str, d: &Design, out: &Path, opts: &VideoOptions) -> Result<()> {
    static RUN_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let spec = DoorSpec::from_design(d)?;
    let client_build = video::build_client(version)?;
    let display = Display::start(opts.width, opts.height)?;
    let port = free_port()?;
    // Unique per call: several recordings can run at once in one process (door video-all).
    let n = RUN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let run = mctas::paths::run_dir().join(format!("video-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&run)?;
    let cfg = WorkerConfig {
        version: version.to_string(),
        listen: Some(port),
        stderr_log: Some(run.join("server.log")),
        ..Default::default()
    };
    let mut w = Worker::spawn(&cfg)?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    place(&mut w, sb, d)?;
    // A grass floor two blocks under the build (never touching it), daylight.
    let (lo, hi) = d.bounds().context("empty design")?;
    let fy = lo[1] - 2;
    w.fill(sb, [lo[0] - 8, fy, lo[2] - 14], [hi[0] + 8, fy, hi[2] + 8], "minecraft:grass_block", PlaceMode::Raw)?;
    w.exec(sb, "time set noon")?;

    let mut client = Client::launch(&client_build, &display, port, opts.width, opts.height, &run.join("client"))?;
    let deadline = Instant::now() + Duration::from_secs(240);
    while w.viewers()? == 0 {
        if !client.alive() {
            bail!("the client exited; see {}", client.log.display());
        }
        if Instant::now() > deadline {
            bail!("the client never joined; see {}", client.log.display());
        }
        w.hold(500)?;
    }
    // Frame the whole build (block corners) from the front-left, slightly above. The client
    // runs with fov 70 (options.txt fov:0.0) at 16:9.
    let mut corners = Vec::new();
    for p in d.blocks.keys() {
        for i in 0..8 {
            corners.push([(p[0] + (i & 1)) as f64, (p[1] + ((i >> 1) & 1)) as f64, (p[2] + ((i >> 2) & 1)) as f64]);
        }
    }
    let _ = (lo, hi);
    // View direction (from the build toward the camera); MCTAS_VIEW="x,y,z" overrides.
    let view: Vec<f64> = std::env::var("MCTAS_VIEW").ok().map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect()).filter(|v: &Vec<f64>| v.len() == 3).unwrap_or(vec![-0.45, 0.2, 1.0]);
    let (eye, yaw, pitch) = video::frame_points(&corners, [view[0], view[1], view[2]], 70.0, opts.width as f64 / opts.height as f64, 0.92);
    w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
    eprintln!("camera requested eye={eye:?} yaw={yaw} pitch={pitch}");
    w.hold(3000)?;
    eprintln!("viewers: {}", w.call(serde_json::json!({"op": "viewers"}))?);
    // Let chunks arrive and the renderer warm up.
    w.hold(12_000)?;
    video::screenshot(&display, opts.width, opts.height, &out.with_extension("png"))?;

    let raw = run.join("raw.mkv");
    let rec = Recorder::start(&display, opts.width, opts.height, opts.fps, &raw)?;
    w.hold(1500)?;
    w.use_block(sb, spec.lever)?;
    w.play(sb, opts.ticks, opts.tps)?;
    w.hold(1200)?;
    w.use_block(sb, spec.lever)?;
    w.play(sb, opts.ticks, opts.tps)?;
    w.hold(1500)?;
    let raw = rec.stop()?;
    video::encode(&raw, out, 1.0)?;
    drop(client);
    Ok(())
}

/// Render a video of every design in `dir` (each `.txt`, README excluded) into `out/<stem>.mp4`,
/// `jobs` at a time, skipping videos that already exist; upload each new one to the GitHub
/// release `upload` (as `<stem>.mp4`). Writes `dir/VIDEOS.md` linking every design to its video.
pub fn record_all(version: &str, dir: &Path, out: &Path, jobs: usize, upload: Option<&str>) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    let todo: Vec<std::path::PathBuf> = files
        .iter()
        .filter(|p| !out.join(format!("{}.mp4", p.file_stem().unwrap().to_string_lossy())).exists())
        .cloned()
        .collect();
    eprintln!("{} designs, {} videos to record ({} at a time)", files.len(), todo.len(), jobs);
    let queue = std::sync::Mutex::new(todo);
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| loop {
                let Some(p) = queue.lock().unwrap().pop() else { break };
                let stem = p.file_stem().unwrap().to_string_lossy().to_string();
                let mp4 = out.join(format!("{stem}.mp4"));
                let tmp = out.join(format!("{stem}.part.mp4"));
                let result = Design::load(&p).and_then(|d| record(version, &d, &tmp, &VideoOptions::default()));
                match result {
                    Ok(()) => {
                        let _ = std::fs::rename(&tmp, &mp4);
                        let up = upload.map(|tag| video::upload_release_asset("vjeux/minecraft-tas", tag, &mp4));
                        match up {
                            Some(Err(e)) => eprintln!("recorded {stem}, upload failed: {e:#}"),
                            _ => eprintln!("recorded {stem}"),
                        }
                    }
                    Err(e) => eprintln!("FAILED {stem}: {e:#}"),
                }
            });
        }
    });
    // Index: every design with its block count, category and video link.
    let mut rows = Vec::new();
    for p in &files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let Ok(d) = Design::load(p) else { continue };
        let has = out.join(format!("{stem}.mp4")).exists();
        let link = if has {
            format!("[video](https://github.com/vjeux/minecraft-tas/releases/download/videos/{stem}.mp4)")
        } else {
            "missing".to_string()
        };
        rows.push((crate::block_count(&d), stem, crate::category(&d), link));
    }
    rows.sort();
    let mut md = String::from("# Videos of every archived design\n\nRendered by the real Minecraft client (`mctas door video-all`).\n\n| blocks | category | design | video |\n|---|---|---|---|\n");
    for (b, stem, cat, link) in rows {
        md.push_str(&format!("| {b} | {cat} | [{stem}]({stem}.txt) | {link} |\n"));
    }
    std::fs::write(dir.join("VIDEOS.md"), md)?;
    Ok(())
}
