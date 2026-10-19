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

/// What to picture: the ticks (ascending), the box to frame and the output cell.
struct Plan {
    ticks: Vec<u32>,
    lo: Pos,
    hi: Pos,
    output: Option<Pos>,
}

/// The output marker (vjeux's pick): an orange stained glass block where the output signal is,
/// written with no block updates only for the picture, then removed.
pub const MARKER: &str = "minecraft:orange_stained_glass";

/// The camera looks from this direction (west, above, south of the clock).
const CAM_DIR: [f64; 3] = [-0.55, 0.6, 1.0];

/// Where the marker goes: the output cell when it is air; else the air cell the output component
/// powers (an observer's back, a repeater's or comparator's front, above a standing torch) when
/// that is air, else its air neighbor most visible from the camera. `None`: no air around.
fn marker_cell(w: &mut Worker, sb: u32, out: Pos) -> Result<Option<Pos>> {
    let region = w.get_region(sb, [out[0] - 1, out[1] - 1, out[2] - 1], [out[0] + 1, out[1] + 1, out[2] + 1])?;
    let air = |p: Pos| region.get(p).is_none_or(mctas::blockstate::is_air);
    if air(out) {
        return Ok(Some(out));
    }
    let state = region.get(out).unwrap_or("").to_string();
    let dir = |f: &str| -> Option<Pos> {
        Some(match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            "up" => [0, 1, 0],
            "down" => [0, -1, 0],
            _ => return None,
        })
    };
    let facing: Option<String> = mctas::blockstate::BlockState::parse(&state).ok().and_then(|b| b.get("facing").map(|s| s.to_string()));
    let id = mctas::blockstate::block_name(&state);
    let preferred: Option<Pos> = match id {
        "minecraft:observer" | "minecraft:repeater" | "minecraft:comparator" => facing.as_deref().and_then(dir).map(|d| [-d[0], -d[1], -d[2]]),
        "minecraft:redstone_torch" => Some([0, 1, 0]),
        _ => None,
    };
    let mut cands: Vec<Pos> = [[0, 0, 1], [0, 1, 0], [-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, -1, 0]].to_vec();
    cands.sort_by(|a, b| {
        let dot = |d: &Pos| d[0] as f64 * CAM_DIR[0] + d[1] as f64 * CAM_DIR[1] + d[2] as f64 * CAM_DIR[2];
        dot(b).total_cmp(&dot(a))
    });
    if let Some(d) = preferred {
        cands.retain(|c| *c != d);
        cands.insert(0, d);
    }
    Ok(cands.into_iter().map(|d| [out[0] + d[0], out[1] + d[1], out[2] + d[2]]).find(|p| air(*p)))
}

/// Take one picture with the marker in place. A block written between ticks reaches the client
/// only with the next tick's broadcast (none here: the clock must not move), so the marker is
/// written BEFORE the camera arrives (`camera_to`: the client then loads the chunks with it), or,
/// with the camera already there, the camera leaves beyond the view distance and comes back (the
/// chunks are sent again). Removed (raw) after the picture.
#[allow(clippy::too_many_arguments)]
fn screenshot_marked(w: &mut Worker, sb: u32, marker: Option<Pos>, camera_to: Option<(Pos3, f32, f32)>, here: (Pos3, f32, f32), drop_after: Option<u32>, display: &Display, rw: u32, rh: u32, hold_ms: u64, path: &Path) -> Result<()> {
    if let Some(m) = marker {
        w.set_blocks(sb, &[(m, MARKER)], PlaceMode::Raw)?;
    }
    match camera_to {
        Some((eye, yaw, pitch)) => w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?,
        None if marker.is_some() => {
            let (eye, yaw, pitch) = here;
            w.camera(sb, [eye[0] + 4096.0, eye[1] - 1.62, eye[2]], yaw, pitch)?;
            w.hold(1500)?;
            w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
        }
        None => {}
    }
    // the sandbox the camera left (never the one it is in: the client would lose its world)
    if let Some(old) = drop_after {
        let _ = w.drop_sandbox(old);
    }
    w.hold(hold_ms)?;
    let r = video::screenshot(display, rw, rh, path);
    if let Some(m) = marker {
        w.set_blocks(sb, &[(m, "minecraft:air")], PlaceMode::Raw)?;
    }
    r
}

type Pos3 = [f64; 3];

fn plan(version: &str, c: &Clock, o: &ShotOptions) -> Result<Plan> {
    let so = SimOptions::default();
    let r = simulate(c, &so);
    let (main, strip, lo, hi, output) = match activity(c, &so) {
        Some((w0, p, counts)) => {
            // the most active moment with nothing in motion (a moving block is invisible in a
            // still picture), else the least motion
            let best = counts
                .iter()
                .enumerate()
                .max_by(|a, b| (std::cmp::Reverse(a.1.1), a.1.0).cmp(&(std::cmp::Reverse(b.1.1), b.1.0)).then(b.0.cmp(&a.0)))
                .map(|(i, _)| i as u32)
                .unwrap_or(0);
            // strip moments: evenly spread, each nudged to the nearest tick with nothing moving
            let still = |t: u32| -> u32 {
                let n = counts.len() as u32;
                (0..n).map(|d| (t + d) % n).find(|&k| counts[k as usize].1 == 0).unwrap_or(t % n.max(1))
            };
            let strip: Vec<u32> = o.strip.map(|n| (0..n).map(|k| w0 + still(k * p / n.max(1))).collect()).unwrap_or_default();
            let mut strip = strip;
            strip.sort();
            strip.dedup();
            (o.tick.unwrap_or(w0 + best), strip, r.swept.0, r.swept.1, r.output.as_ref().map(|o| o.pos))
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
            (o.tick.unwrap_or(k.warmup + k.period / 2), strip, lo, hi, k.output.as_ref().map(|o| o.pos))
        }
    };
    let mut ticks = vec![main];
    ticks.extend(strip);
    Ok(Plan { ticks, lo, hi, output })
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
        // the main picture
        step_to(&mut w, p.ticks[0], &mut t, &mut actions)?;
        let marker = match p.output {
            Some(op) => marker_cell(&mut w, sb, op)?,
            None => None,
        };
        let (mut flo, mut fhi) = (p.lo, p.hi);
        if let Some(mk) = marker {
            crate::grow(&mut flo, &mut fhi, mk);
        }
        let lo = [flo[0] as f64 - 0.25, flo[1] as f64 - 0.25, flo[2] as f64 - 0.25];
        let hi = [fhi[0] as f64 + 1.25, fhi[1] as f64 + 1.25, fhi[2] as f64 + 1.25];
        let (eye, yaw, pitch) = video::frame_box(lo, hi, CAM_DIR, VFOV, aspect, 0.85);
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
        let raw = run.join(format!("{stem}.png"));
        // the camera arrives with the marker already written (the client loads the chunks with
        // it); the previous clock's sandbox goes once the camera has left it
        let old = prev.take();
        screenshot_marked(&mut w, sb, marker, Some((eye, yaw, pitch)), (eye, yaw, pitch), old, &display, rw, rh, if i == 0 { o.hold_ms.max(12_000) } else { o.hold_ms }, &raw)?;
        let out = shot_path(&o.out_dir, stem);
        // the camera aims at the box's center: keep the middle 64% of the frame (the clock bigger,
        // less grass and sky), scaled to the requested width
        let crop = "crop=iw*0.64:ih*0.64";
        ffmpeg_run(&["-i".into(), raw.display().to_string(), "-vf".into(), format!("{crop},scale={}:-2:flags=lanczos", o.width), "-q:v".into(), "3".into(), out.display().to_string()])?;
        // the strip: N moments of one period (the main picture's tick may be later: a fresh build)
        if p.ticks.len() > 1 {
            let mut frames: Vec<PathBuf> = Vec::new();
            let strip_ticks: Vec<u32> = p.ticks[1..].to_vec();
            {
                // the strip restarts the clock in a second sandbox at the same place
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
                    let mk = match p.output {
                        Some(op) => marker_cell(&mut w, sb2, op)?,
                        None => None,
                    };
                    let f = run.join(format!("{stem}-strip{k}.png"));
                    if k == 0 {
                        screenshot_marked(&mut w, sb2, mk, Some((eye, yaw, pitch)), (eye, yaw, pitch), Some(sb), &display, rw, rh, o.hold_ms, &f)?;
                    } else {
                        screenshot_marked(&mut w, sb2, mk, None, (eye, yaw, pitch), None, &display, rw, rh, 1500, &f)?;
                    }
                    frames.push(f);
                }
                prev = Some(sb2);
            }
            let n = frames.len();
            if n < 2 {
                bail!("{stem}: the strip has {n} distinct moment(s)");
            }
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
