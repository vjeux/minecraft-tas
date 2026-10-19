//! `mctas clock shot`: pictures of clocks in the REAL client (vjeux: "take screenshots of all these
//! clocks and put them in the catalog"). One client session for every clock: each clock gets its
//! own sandbox (placed and kicked like the judge, on a grass floor at noon, the door videos'
//! look), is stepped to the moment of its period where the most of its changing cells are ON (or
//! `--tick`), and a still camera framing its whole swept box with a margin takes the picture,
//! saved small (JPEG, `--width` px). `--strip N`: N moments evenly spread over one period tiled into
//! one image (the motion). The catalog (`clock catalog`) shows them.

use crate::sim::{SimOptions, activity, simulate};
use crate::verify::{VerifyOptions, apply, place, run_one};
use crate::{Clock, Pos};
use anyhow::{Context, Result, bail};
use mctas::sim::{PlaceMode, SandboxOptions, Worker, WorkerConfig};
use mctas::video::{self, Client, Display};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

pub const IMG_DIR: &str = "designs/clocks/img";
const VFOV: f64 = 70.0;

/// Knobs of [`shots`].
#[derive(Clone, Debug)]
pub struct ShotOptions {
    pub out_dir: PathBuf,
    /// The tick to picture (default: the most active moment of a period after the warmup).
    pub tick: Option<u32>,
    /// N moments of one period tiled into one image (`<name>.strip.jpg`) as well.
    pub strip: Option<u32>,
    /// Width of the saved picture (px).
    pub width: u32,
    /// Render size of the client.
    pub render: (u32, u32),
    /// Wait after moving the camera to a clock (chunks arrive, the renderer settles).
    pub hold_ms: u64,
    /// Re-take pictures that exist.
    pub redo: bool,
}

impl Default for ShotOptions {
    fn default() -> Self {
        ShotOptions { out_dir: PathBuf::from(IMG_DIR), tick: None, strip: None, width: 800, render: (1920, 1080), hold_ms: 5000, redo: false }
    }
}

/// Where a clock's picture goes: `<out_dir>/<stem>.jpg` (the design file's stem).
pub fn shot_path(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.jpg"))
}

pub fn strip_path(dir: &Path, stem: &str) -> PathBuf {
    dir.join(format!("{stem}.strip.jpg"))
}

/// What to picture: the ticks (ascending) and the box to frame.
struct Plan {
    ticks: Vec<u32>,
    lo: Pos,
    hi: Pos,
}

fn plan(version: &str, c: &Clock, o: &ShotOptions) -> Result<Plan> {
    let so = SimOptions::default();
    let r = simulate(c, &so);
    let (main, strip, lo, hi) = match activity(c, &so) {
        Some((w0, p, counts)) => {
            let best = counts.iter().enumerate().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(&a.0))).map(|(i, _)| i as u32).unwrap_or(0);
            let strip: Vec<u32> = o.strip.map(|n| (0..n).map(|k| w0 + k * p / n.max(1)).collect()).unwrap_or_default();
            (o.tick.unwrap_or(w0 + best), strip, r.swept.0, r.swept.1)
        }
        None => {
            // redsim does not model it (or says it is no clock): one game build gives W and P
            let vo = VerifyOptions { positions: 1, rotations: false, version: version.to_string(), ..Default::default() };
            let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
            let k = match run_one(&mut w, c, &vo)? {
                Ok(k) => k,
                Err(e) => bail!("{}: not a clock in the game: {e}", c.name()),
            };
            let (lo, _) = c.bounds();
            let hi = [lo[0] + k.dims[0] - 1, lo[1] + k.dims[1] - 1, lo[2] + k.dims[2] - 1];
            let strip: Vec<u32> = o.strip.map(|n| (0..n).map(|i| k.warmup + i * k.period / n.max(1)).collect()).unwrap_or_default();
            (o.tick.unwrap_or(k.warmup + k.period / 2), strip, lo, hi)
        }
    };
    let mut ticks = vec![main];
    ticks.extend(strip);
    Ok(Plan { ticks, lo, hi })
}

fn ffmpeg_run(args: &[String]) -> Result<()> {
    let out = Command::new(video::ffmpeg()?).args(["-loglevel", "error", "-y"]).args(args).output()?;
    if !out.status.success() {
        bail!("ffmpeg {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// Picture every clock (`(stem, clock)`): see the module doc. Returns one line per clock.
pub fn shots(version: &str, items: &[(String, Clock)], o: &ShotOptions) -> Result<Vec<String>> {
    std::fs::create_dir_all(&o.out_dir)?;
    let mut lines = Vec::new();
    let mut todo: Vec<(&String, &Clock, Plan)> = Vec::new();
    for (stem, c) in items {
        if !o.redo && shot_path(&o.out_dir, stem).exists() && (o.strip.is_none() || strip_path(&o.out_dir, stem).exists()) {
            lines.push(format!("HAVE {}", shot_path(&o.out_dir, stem).display()));
            continue;
        }
        match plan(version, c, o) {
            Ok(p) => todo.push((stem, c, p)),
            Err(e) => lines.push(format!("SKIP {stem}: {e:#}")),
        }
    }
    if todo.is_empty() {
        return Ok(lines);
    }
    let (rw, rh) = o.render;
    let client_build = video::build_client(version)?;
    let display = Display::start(rw, rh)?;
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0")?;
        l.local_addr()?.port()
    };
    let run = mctas::paths::run_dir().join(format!("clock-shot-{}", std::process::id()));
    std::fs::create_dir_all(&run)?;
    let cfg = WorkerConfig { version: version.to_string(), listen: Some(port), stderr_log: Some(run.join("server.log")), ..Default::default() };
    let mut w = Worker::spawn(&cfg)?;
    let mut client: Option<Client> = None;
    let mut prev: Option<u32> = None;
    for (i, (stem, c, p)) in todo.iter().enumerate() {
        let t0 = Instant::now();
        let m = 40;
        let chunks = [(p.lo[0] - m).div_euclid(16), (p.lo[2] - m).div_euclid(16), (p.hi[0] + m).div_euclid(16), (p.hi[2] + m).div_euclid(16)];
        let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
        place(&mut w, sb, c)?;
        let fy = p.lo[1] - 2;
        w.fill(sb, [p.lo[0] - m, fy, p.lo[2] - m], [p.hi[0] + m, fy, p.hi[2] + m], "minecraft:grass_block", PlaceMode::Raw)?;
        w.exec(sb, "time set noon")?;
        w.view_distance(sb, 4)?;
        let mut actions = c.actions();
        let mut t = 0u32;
        let step_to = |w: &mut Worker, target: u32, t: &mut u32, actions: &mut Vec<(u32, crate::Action)>| -> Result<()> {
            loop {
                while actions.first().is_some_and(|(at, _)| *at == *t) {
                    let (_, a) = actions.remove(0);
                    apply(w, sb, &a)?;
                }
                if *t >= target {
                    return Ok(());
                }
                let next = actions.first().map(|(at, _)| *at).unwrap_or(target).min(target).max(*t + 1);
                w.step(sb, next - *t)?;
                *t = next;
            }
        };
        let aspect = rw as f64 / rh as f64;
        let lo = [p.lo[0] as f64 - 0.25, p.lo[1] as f64 - 0.25, p.lo[2] as f64 - 0.25];
        let hi = [p.hi[0] as f64 + 1.25, p.hi[1] as f64 + 1.25, p.hi[2] as f64 + 1.25];
        let (eye, yaw, pitch) = video::frame_box(lo, hi, [-0.55, 0.6, 1.0], VFOV, aspect, 0.85);
        // the main picture
        step_to(&mut w, p.ticks[0], &mut t, &mut actions)?;
        if client.is_none() {
            let cl = Client::launch(&client_build, &display, port, rw, rh, &run.join("client"))?;
            client = Some(cl);
            let deadline = Instant::now() + Duration::from_secs(240);
            while w.viewers()? == 0 {
                if !client.as_mut().unwrap().alive() {
                    bail!("the client exited; see {}", client.as_ref().unwrap().log.display());
                }
                if Instant::now() > deadline {
                    bail!("the client never joined; see {}", client.as_ref().unwrap().log.display());
                }
                w.hold(500)?;
            }
        }
        w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
        if let Some(old) = prev.take() {
            let _ = w.drop_sandbox(old);
        }
        w.hold(if i == 0 { o.hold_ms.max(12_000) } else { o.hold_ms })?;
        let raw = run.join(format!("{stem}.png"));
        video::screenshot(&display, rw, rh, &raw)?;
        let out = shot_path(&o.out_dir, stem);
        // the camera aims at the box's center: keep the middle 64% of the frame (the clock bigger,
        // less grass and sky), scaled to the requested width
        let crop = "crop=iw*0.64:ih*0.64";
        ffmpeg_run(&["-i".into(), raw.display().to_string(), "-vf".into(), format!("{crop},scale={}:-2:flags=lanczos", o.width), "-q:v".into(), "3".into(), out.display().to_string()])?;
        // the strip: N moments of one period (the main picture's tick may be later: a fresh build)
        if p.ticks.len() > 1 {
            let mut frames: Vec<PathBuf> = Vec::new();
            let strip_ticks: Vec<u32> = p.ticks[1..].to_vec();
            if strip_ticks[0] < t {
                // restart the clock in a second sandbox at the same place
                let sb2 = w.create(&SandboxOptions { chunks, seed: 0 })?;
                place(&mut w, sb2, c)?;
                w.fill(sb2, [p.lo[0] - m, fy, p.lo[2] - m], [p.hi[0] + m, fy, p.hi[2] + m], "minecraft:grass_block", PlaceMode::Raw)?;
                w.exec(sb2, "time set noon")?;
                w.view_distance(sb2, 4)?;
                let mut acts = c.actions();
                let mut t2 = 0u32;
                for (k, &tk) in strip_ticks.iter().enumerate() {
                    loop {
                        while acts.first().is_some_and(|(at, _)| *at == t2) {
                            let (_, a) = acts.remove(0);
                            apply(&mut w, sb2, &a)?;
                        }
                        if t2 >= tk {
                            break;
                        }
                        let next = acts.first().map(|(at, _)| *at).unwrap_or(tk).min(tk).max(t2 + 1);
                        w.step(sb2, next - t2)?;
                        t2 = next;
                    }
                    if k == 0 {
                        w.camera(sb2, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
                        let _ = w.drop_sandbox(sb);
                        w.hold(o.hold_ms)?;
                    } else {
                        w.hold(700)?;
                    }
                    let f = run.join(format!("{stem}-strip{k}.png"));
                    video::screenshot(&display, rw, rh, &f)?;
                    frames.push(f);
                }
                prev = Some(sb2);
            }
            let n = frames.len();
            let each = (o.width * 2 / 3).max(200);
            let mut args: Vec<String> = Vec::new();
            for f in &frames {
                args.extend(["-i".to_string(), f.display().to_string()]);
            }
            let scaled: String = (0..n).map(|k| format!("[{k}:v]crop=iw*0.64:ih*0.64,scale={each}:-2:flags=lanczos[s{k}];")).collect();
            let inputs: String = (0..n).map(|k| format!("[s{k}]")).collect();
            args.extend(["-filter_complex".to_string(), format!("{scaled}{inputs}hstack=inputs={n}[o]"), "-map".into(), "[o]".into(), "-q:v".into(), "3".into()]);
            args.push(strip_path(&o.out_dir, stem).display().to_string());
            ffmpeg_run(&args).context("tiling the strip")?;
        }
        if prev.is_none() {
            prev = Some(sb);
        }
        lines.push(format!("SHOT {} (tick {}{}; {:.1} s)", out.display(), p.ticks[0], if p.ticks.len() > 1 { format!(", strip of {}", p.ticks.len() - 1) } else { String::new() }, t0.elapsed().as_secs_f64()));
        eprintln!("{}", lines.last().unwrap());
    }
    drop(client);
    Ok(lines)
}
