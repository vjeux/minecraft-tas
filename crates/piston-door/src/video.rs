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
    let spec = DoorSpec::from_design(d)?;
    let client_build = video::build_client(version)?;
    let display = Display::start(opts.width, opts.height)?;
    let port = free_port()?;
    let run = mctas::paths::run_dir().join(format!("video-{}", std::process::id()));
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
