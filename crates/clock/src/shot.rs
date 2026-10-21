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
    /// One-world shooter: the wait after a camera move (plus 15 ms per block moved).
    pub settle_ms: u64,
    /// Re-take pictures that exist.
    pub redo: bool,
    /// Per design stem, the judge's (warmup, period) from an archived design's `# CLOCK:` line:
    /// used for clocks redsim does not model (no game run to find them).
    pub hints: std::collections::HashMap<String, (u32, u32)>,
    /// The design pictured exactly as written (`--still`: a still frame exported by `mctas web
    /// snapshot`, for `web compare --still`): placed like the judge, no kick, nothing stepped (one
    /// tick with the camera there when it has entities, so the client gets them), no marker, the
    /// frame its blocks' box.
    pub still: bool,
}

/// The (warmup, period) of an archived design's judge line (`# CLOCK: <name> period P gt ...,
/// warmup W, ...`), if any.
pub fn judge_hint(text: &str) -> Option<(u32, u32)> {
    let line = text.lines().find(|l| l.starts_with("# CLOCK:"))?;
    let num_after = |key: &str| -> Option<u32> {
        let i = line.find(key)? + key.len();
        line[i..].split(|ch: char| !ch.is_ascii_digit()).next()?.parse().ok()
    };
    Some((num_after("warmup ")?, num_after(" period ")?))
}

impl Default for ShotOptions {
    fn default() -> Self {
        ShotOptions { out_dir: PathBuf::from(IMG_DIR), tick: None, strip: None, width: 800, render: (1920, 1080), hold_ms: 1500, settle_ms: 700, redo: false, hints: Default::default(), still: false }
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
    /// The swept box (the floor and the sandbox cover it).
    lo: Pos,
    hi: Pos,
    output: Option<Pos>,
}

impl Plan {
    /// The same plan for the clock moved `dy` blocks up.
    fn shifted(&self, dy: i32) -> Plan {
        let up = |p: Pos| [p[0], p[1] + dy, p[2]];
        Plan { ticks: self.ticks.clone(), lo: up(self.lo), hi: up(self.hi), output: self.output.map(up) }
    }
    /// The box the camera frames: the swept box, or for an elongated clock (its longest side over
    /// 16 blocks and over 5x the next) a window of it along that side around the output, 3x the
    /// next side long (8 to 16 blocks: coordinator, 50-100-block observer lines and towers came out
    /// as thin lines).
    fn frame(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = (self.lo, self.hi);
        let ext = |k: usize| hi[k] - lo[k] + 1;
        let mut axes = [0usize, 1, 2];
        axes.sort_by_key(|&k| std::cmp::Reverse(ext(k)));
        let (long, next) = (axes[0], axes[1]);
        if ext(long) > 16 && ext(long) > 5 * ext(next) {
            let win = (3 * ext(next)).clamp(8, 16);
            let c = self.output.map(|o| o[long]).unwrap_or((lo[long] + hi[long]) / 2);
            let a = (c - win / 2).clamp(lo[long], hi[long] - win + 1);
            lo[long] = a;
            hi[long] = a + win - 1;
        }
        (lo, hi)
    }
}

/// The mean luma (0-255) of a picture (ffmpeg signalstats), if it can be read.
fn mean_luma(path: &Path) -> Option<f64> {
    band_luma(path, None)
}

/// The mean luma of the whole picture, or of its top `frac` band.
fn band_luma(path: &Path, top: Option<f64>) -> Option<f64> {
    let vf = match top {
        Some(f) => format!("crop=iw:ih*{f}:0:0,signalstats,metadata=print:key=lavfi.signalstats.YAVG"),
        None => "signalstats,metadata=print:key=lavfi.signalstats.YAVG".to_string(),
    };
    let o = Command::new(video::ffmpeg().ok()?).args(["-hide_banner", "-i"]).arg(path).args(["-vf", &vf, "-f", "null", "-"]).output().ok()?;
    let err = String::from_utf8_lossy(&o.stderr);
    let i = err.find("YAVG=")? + 5;
    err[i..].split(|ch: char| !(ch.is_ascii_digit() || ch == '.')).next()?.parse().ok()
}

/// A picture to take again: nearly uniform (its JPEG under 15 KB: sky or grass only), dark (mean
/// luma < 70: the client still loading, an error screen), or with a dark band across its top (mean
/// luma < 70 in the top 12%: the sky beyond the loaded floor drawn black below the horizon).
pub fn bad_picture(jpg: &Path) -> Option<String> {
    match std::fs::metadata(jpg) {
        Ok(m) if m.len() < 15_000 => return Some(format!("nearly uniform ({} bytes)", m.len())),
        Err(_) => return Some("missing".into()),
        _ => {}
    }
    if let Some(l) = mean_luma(jpg).filter(|l| *l < 70.0) {
        return Some(format!("dark (mean luma {l:.0})"));
    }
    band_luma(jpg, Some(0.12)).filter(|l| *l < 70.0).map(|l| format!("dark band at the top (luma {l:.0})"))
}

/// A frame to take again: see [`bad_picture`] (checked on the saved JPEG).
fn bad_frame(_raw: &Path, jpg: &Path) -> Option<String> {
    bad_picture(jpg)
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
    let debug = std::env::var_os("MCTAS_SHOT_DEBUG").is_some();
    let t0 = Instant::now();
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
    let t_cam = t0.elapsed().as_secs_f64();
    w.hold(hold_ms)?;
    let t1 = Instant::now();
    let r = video::screenshot(display, rw, rh, path);
    let t_grab = t1.elapsed().as_secs_f64();
    // the sandbox the camera left (never the one it is in: the client would lose its world),
    // once the move is done (dropping it right after the teleport waited ~8 s)
    let t2 = Instant::now();
    if let Some(old) = drop_after {
        let _ = w.drop_sandbox(old);
    }
    let t_drop = t2.elapsed().as_secs_f64();
    if let Some(m) = marker {
        w.set_blocks(sb, &[(m, "minecraft:air")], PlaceMode::Raw)?;
    }
    if debug {
        eprintln!("  camera {t_cam:.2} s, drop {t_drop:.2} s, hold {:.2} s, grab {t_grab:.2} s", hold_ms as f64 / 1000.0);
    }
    r
}

type Pos3 = [f64; 3];

fn plan(version: &str, stem: &str, c: &Clock, o: &ShotOptions) -> Result<Plan> {
    // as written: nothing to find (a still frame: its blocks' box and its entities' cells, like
    // the viewer frames it; tick 0, no marker)
    if o.still {
        let (mut lo, mut hi) = c.bounds();
        for e in &c.design.entities {
            crate::grow(&mut lo, &mut hi, [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32]);
        }
        let t = if c.design.entities.is_empty() { 0 } else { 1 };
        return Ok(Plan { ticks: vec![t], lo, hi, output: None });
    }
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
            // redsim does not model it (or says it is no clock): the archive's judge line gives W
            // and P when there is one (no game run needed), else one game build does
            if let Some(&(w0, per)) = o.hints.get(stem) {
                let (lo, hi) = c.bounds();
                let strip: Vec<u32> = o.strip.map(|n| (0..n).map(|i| w0 + i * per / n.max(1)).collect()).unwrap_or_default();
                (o.tick.unwrap_or(w0 + per / 2), strip, lo, hi, c.output)
            } else {
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

/// A picture's JPEG from its screenshot: the camera aims at the box's center, so the middle 64%
/// of the frame is kept (the clock bigger, less grass and sky; ffmpeg's integer crop), scaled
/// (lanczos) to `width`. `mctas web compare` passes the viewer's frame through it too: both sides
/// cropped, resampled and compressed alike.
pub fn encode_picture(raw: &Path, out: &Path, width: u32) -> Result<()> {
    ffmpeg_run(&["-i".into(), raw.display().to_string(), "-vf".into(), format!("crop=iw*0.64:ih*0.64,scale={width}:-2:flags=lanczos"), "-q:v".into(), "3".into(), out.display().to_string()])
}

/// Picture every clock in ONE world (vjeux: hundreds of clocks in the pipeline): the clocks are laid
/// out on a grid (4 blocks of clearance, 17 around a clock with a sculk sensor: vibrations carry),
/// each placed at a staggered start so that one frozen tick catches every clock at its chosen
/// moment; the markers go in, the client joins, and only the camera moves from clock to clock.
/// No strips (they need other ticks: `shots`). Returns one line per clock.
pub fn shots_grid(version: &str, items: &[(String, Clock)], o: &ShotOptions) -> Result<Vec<String>> {
    std::fs::create_dir_all(&o.out_dir)?;
    let t_all = Instant::now();
    let mut lines = Vec::new();
    let mut todo: Vec<(&String, &Clock, Plan)> = Vec::new();
    for (stem, c) in items {
        if !o.redo && shot_path(&o.out_dir, stem).exists() {
            lines.push(format!("HAVE {}", shot_path(&o.out_dir, stem).display()));
            continue;
        }
        match plan(version, stem, c, &ShotOptions { strip: None, ..o.clone() }) {
            Ok(p) => todo.push((stem, c, p)),
            Err(e) => lines.push(format!("SKIP {stem}: {e:#}")),
        }
    }
    if todo.is_empty() {
        return Ok(lines);
    }
    let t_plan = t_all.elapsed().as_secs_f64();
    // the layout: rows of cells along x (wrapping at ROW blocks), sculk clocks last; a batch (one
    // world) ends when its rows reach DEPTH blocks: the client gets every chunk of a batch from its
    // center (the client's render distance, 6 chunks, caps what the server sends it, and a frozen
    // level never re-centers it: so a batch fits within 80 blocks of its center)
    const ROW: i32 = 112;
    const DEPTH: i32 = 112;
    let sculk = |c: &Clock| c.design.blocks.values().any(|s| s.contains("sculk_sensor"));
    // sculk clocks last; then by their chosen tick, so a batch's world runs about as long as its
    // clocks need (a world runs to the largest chosen tick of its batch)
    todo.sort_by_key(|(_, c, p)| (sculk(c), p.ticks[0]));
    let mut batches: Vec<Vec<Cell>> = vec![Vec::new()];
    let (mut x, mut z, mut depth, mut row) = (0i32, 0i32, 0i32, 0i32);
    for (i, (_, c, p)) in todo.iter().enumerate() {
        let gap = if sculk(c) { 17 } else { 4 };
        let (w, d) = (p.hi[0] - p.lo[0] + 1 + 2 * gap, p.hi[2] - p.lo[2] + 1 + 2 * gap);
        if x > 0 && x + w > ROW {
            x = 0;
            z += depth;
            depth = 0;
            row += 1;
        }
        if z + d > DEPTH && !batches.last().unwrap().is_empty() {
            batches.push(Vec::new());
            (x, z, depth, row) = (0, 0, 0, 0);
        }
        batches.last_mut().unwrap().push(Cell { i, off: [x + gap - p.lo[0], -p.lo[1], z + gap - p.lo[2]], key: (row, if row % 2 == 0 { x } else { -x }) });
        x += w;
        depth = depth.max(d);
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
    // every batch's world is built BEFORE the client joins (the server does no network work while
    // it builds and steps: a client waiting through that times out)
    let tb = Instant::now();
    let mut worlds: Vec<(u32, Vec<Option<Pos>>)> = Vec::with_capacity(batches.len());
    let mut g_max = 0u32;
    for batch in &batches {
        let (sb, markers, g) = build_batch(&mut w, &todo, batch)?;
        g_max = g_max.max(g);
        worlds.push((sb, markers));
    }
    let t_world = tb.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let mut client = Client::launch(&client_build, &display, port, rw, rh, &run.join("client"))?;
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
    let t_join = t0.elapsed().as_secs_f64();
    let mut raws: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut marked: Vec<bool> = vec![false; todo.len()];
    let mut prev: Option<u32> = None;
    let mut t_cam = 0.0f64;
    let mut lost: Vec<usize> = Vec::new();
    for (batch, (sb, markers)) in batches.iter_mut().zip(worlds) {
        let tc = Instant::now();
        // the camera arrives from another level: the client gets every chunk of the batch, the
        // markers included (written raw on the frozen level, never broadcast otherwise)
        let mv = |p: Pos, d: Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        let (mut wlo, mut whi) = (mv(todo[batch[0].i].2.lo, batch[0].off), mv(todo[batch[0].i].2.hi, batch[0].off));
        for c in batch.iter() {
            crate::grow(&mut wlo, &mut whi, mv(todo[c.i].2.lo, c.off));
            crate::grow(&mut wlo, &mut whi, mv(todo[c.i].2.hi, c.off));
        }
        w.view_distance(sb, 12)?;
        let center = [(wlo[0] + whi[0]) as f64 / 2.0, 30.0, (wlo[2] + whi[2]) as f64 / 2.0];
        w.camera(sb, center, 0.0, 90.0)?;
        if let Some(old) = prev.take() {
            let _ = w.drop_sandbox(old);
        }
        w.hold(o.hold_ms.max(8_000))?;
        batch.sort_by_key(|c| c.key);
        let aspect = rw as f64 / rh as f64;
        let mut prev_eye: Option<[f64; 3]> = None;
        for c in batch.iter() {
            let (stem, _, p) = &todo[c.i];
            let (mut flo, mut fhi) = (mv(p.lo, c.off), mv(p.hi, c.off));
            if let Some(mk) = markers[c.i] {
                crate::grow(&mut flo, &mut fhi, mk);
            }
            marked[c.i] = markers[c.i].is_some();
            let lo = [flo[0] as f64 - 0.25, flo[1] as f64 - 0.25, flo[2] as f64 - 0.25];
            let hi = [fhi[0] as f64 + 1.25, fhi[1] as f64 + 1.25, fhi[2] as f64 + 1.25];
            let (eye, yaw, pitch) = video::frame_box(lo, hi, CAM_DIR, VFOV, aspect, 0.85);
            if std::env::var_os("MCTAS_SHOT_DEBUG").is_some() {
                eprintln!("{stem}: box {flo:?}..{fhi:?} eye {eye:?} marker {:?}", markers[c.i]);
            }
            w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
            // settle: the renderer meshes the chunks that come into its range
            let dist = prev_eye.map(|q| ((q[0] - eye[0]).powi(2) + (q[2] - eye[2]).powi(2)).sqrt()).unwrap_or(64.0);
            w.hold((o.settle_ms as f64 + dist * 15.0).min(3000.0) as u64)?;
            prev_eye = Some(eye);
            // a client that lost the connection pictures its error screen: those clocks are taken
            // again in their own sandboxes
            if w.viewers()? == 0 {
                lost.push(c.i);
                continue;
            }
            let raw = run.join(format!("{stem}.png"));
            video::screenshot(&display, rw, rh, &raw)?;
            raws.push((raw, shot_path(&o.out_dir, stem)));
        }
        t_cam += tc.elapsed().as_secs_f64();
        prev = Some(sb);
    }
    drop(client);
    // JPEGs, 8 at a time
    let t2 = Instant::now();
    let errors: Vec<String> = std::thread::scope(|s| {
        let chunks: Vec<&[(PathBuf, PathBuf)]> = raws.chunks(raws.len().div_ceil(8).max(1)).collect();
        let hs: Vec<_> = chunks
            .into_iter()
            .map(|part| {
                s.spawn(move || {
                    let mut errs = Vec::new();
                    for (raw, out) in part {
                        if let Err(e) = encode_picture(raw, out, o.width) {
                            errs.push(format!("{}: {e:#}", out.display()));
                        }
                    }
                    errs
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    if let Some(e) = errors.first() {
        bail!("encoding: {e}");
    }
    let t_enc = t2.elapsed().as_secs_f64();
    // a picture that came out (nearly) uniform (chunks the client never got): taken again in its
    // own sandbox
    let blank: Vec<usize> = (0..todo.len()).filter(|&i| lost.contains(&i) || std::fs::metadata(shot_path(&o.out_dir, todo[i].0)).map(|m| m.len() < 12_000).unwrap_or(true)).collect();
    for (i, (stem, _, p)) in todo.iter().enumerate() {
        if !blank.contains(&i) {
            lines.push(format!("SHOT {} (tick {}{})", shot_path(&o.out_dir, stem).display(), p.ticks[0], if marked[i] { ", marker" } else { ", no marker" }));
        }
    }
    if !blank.is_empty() {
        let again: Vec<(String, Clock)> = blank.iter().map(|&i| (todo[i].0.clone(), todo[i].1.clone())).collect();
        for l in shots(version, &again, &ShotOptions { redo: true, strip: None, ..o.clone() })? {
            lines.push(format!("{l} (blank in the one-world pass: taken again in its own sandbox)"));
        }
    }
    let n = todo.len() as f64;
    lines.push(format!(
        "TIMING {} clocks in {} world(s): plan {t_plan:.1} s, client start {t_join:.1} s, worlds (create, place at staggered starts, run up to {g_max} gt, markers) {t_world:.1} s, camera + capture {t_cam:.1} s ({:.2} s per clock), encode {t_enc:.1} s; total {:.1} s = {:.2} s per clock",
        todo.len(),
        batches.len(),
        t_cam / n,
        t_all.elapsed().as_secs_f64(),
        t_all.elapsed().as_secs_f64() / n
    ));
    Ok(lines)
}

/// A clock's place in a one-world batch: its index, its offset, its visiting key (row, x).
struct Cell {
    i: usize,
    off: Pos,
    key: (i32, i32),
}

/// Build one batch's world: a grass floor, every clock placed at its staggered start (clock i at
/// G - T_i) and kicked, run to G (every clock at its chosen tick), the markers written raw.
/// Returns the sandbox, the marker of each clock (indexed like `todo`) and G.
fn build_batch(w: &mut Worker, todo: &[(&String, &Clock, Plan)], batch: &[Cell]) -> Result<(u32, Vec<Option<Pos>>, u32)> {
    let mv = |p: Pos, d: Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
    let (mut wlo, mut whi) = (mv(todo[batch[0].i].2.lo, batch[0].off), mv(todo[batch[0].i].2.hi, batch[0].off));
    for c in batch {
        crate::grow(&mut wlo, &mut whi, mv(todo[c.i].2.lo, c.off));
        crate::grow(&mut wlo, &mut whi, mv(todo[c.i].2.hi, c.off));
    }
    let m = 24;
    let chunks = [(wlo[0] - m).div_euclid(16), (wlo[2] - m).div_euclid(16), (whi[0] + m).div_euclid(16), (whi[2] + m).div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    w.fill(sb, [wlo[0] - m, -2, wlo[2] - m], [whi[0] + m, -2, whi[2] + m], "minecraft:grass_block", PlaceMode::Raw)?;
    w.exec(sb, "time set noon")?;
    let g = batch.iter().map(|c| todo[c.i].2.ticks[0]).max().unwrap_or(0);
    enum Ev {
        Place(usize),
        Act(crate::Action),
    }
    let moved: Vec<(usize, Clock)> = batch.iter().map(|c| (c.i, todo[c.i].1.translated(c.off))).collect();
    let mut evs: Vec<(u32, usize, Ev)> = Vec::new();
    for (k, (i, mc)) in moved.iter().enumerate() {
        let s = g - todo[*i].2.ticks[0];
        evs.push((s, 0, Ev::Place(k)));
        for (at, a) in mc.actions() {
            evs.push((s + at, 1, Ev::Act(a)));
        }
    }
    evs.sort_by_key(|(t, k, _)| (*t, *k));
    let mut t = 0u32;
    for (at, _, ev) in evs {
        if at > t {
            w.step(sb, at - t)?;
            t = at;
        }
        match ev {
            Ev::Place(k) => place(w, sb, &moved[k].1)?,
            Ev::Act(a) => apply(w, sb, &a)?,
        }
    }
    if g > t {
        w.step(sb, g - t)?;
    }
    let mut markers: Vec<Option<Pos>> = vec![None; todo.len()];
    for c in batch {
        if let Some(op) = todo[c.i].2.output {
            markers[c.i] = marker_cell(w, sb, mv(op, c.off))?;
        }
    }
    let marks: Vec<(Pos, &str)> = markers.iter().flatten().map(|m| (*m, MARKER)).collect();
    if !marks.is_empty() {
        w.set_blocks(sb, &marks, PlaceMode::Raw)?;
    }
    Ok((sb, markers, g))
}

/// The SSIM of every picture of `dir` against the picture of the same name in `against` (ffmpeg's
/// ssim filter), lowest first: a check of one shooter against another (a cut or blank frame scores
/// far below a good one, neighbors in the background barely matter).
pub fn compare(dir: &Path, against: &Path) -> Result<Vec<(String, f64)>> {
    let mut names: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.ends_with(".jpg") && !n.ends_with(".strip.jpg") && against.join(n).exists())
        .collect();
    names.sort();
    let ff = video::ffmpeg()?;
    let mut out: Vec<(String, f64)> = std::thread::scope(|s| {
        let parts: Vec<&[String]> = names.chunks(names.len().div_ceil(8).max(1)).collect();
        let hs: Vec<_> = parts
            .into_iter()
            .map(|part| {
                let ff = ff.clone();
                s.spawn(move || {
                    part.iter()
                        .filter_map(|n| {
                            let o = Command::new(&ff).args(["-i"]).arg(dir.join(n)).args(["-i"]).arg(against.join(n)).args(["-lavfi", "ssim", "-f", "null", "-"]).output().ok()?;
                            let err = String::from_utf8_lossy(&o.stderr);
                            let i = err.find("All:")? + 4;
                            let v: f64 = err[i..].split_whitespace().next()?.parse().ok()?;
                            Some((n.clone(), v))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap_or_default()).collect()
    });
    out.sort_by(|a, b| a.1.total_cmp(&b.1));
    Ok(out)
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
        match plan(version, stem, c, o) {
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
    // two sandboxes used in turn, cleared for each clock and never dropped (dropping a level the
    // client has visited took ~10 s a clock); the camera always arrives from the other one, so
    // the client loads every chunk (and the marker) fresh
    // the floor reaches 96 blocks around each clock and the client gets chunks up to 6 chunks (its
    // render distance) from the camera: a camera looking low along a long clock saw past a nearer
    // edge into the dark sky below the horizon
    let m = 96;
    let (mut ulo, mut uhi) = (todo[0].2.lo, todo[0].2.hi);
    for (_, _, p) in &todo {
        crate::grow(&mut ulo, &mut uhi, p.lo);
        crate::grow(&mut ulo, &mut uhi, p.hi);
    }
    let chunks = [(ulo[0] - m).div_euclid(16), (ulo[2] - m).div_euclid(16), (uhi[0] + m).div_euclid(16), (uhi[2] + m).div_euclid(16)];
    let pair = [w.create(&SandboxOptions { chunks, seed: 0 })?, w.create(&SandboxOptions { chunks, seed: 0 })?];
    for sb in pair {
        w.view_distance(sb, 6)?;
    }
    let mut cam_in: Option<u32> = None;
    for (i, (stem, c, p)) in todo.iter().enumerate() {
        let t0 = Instant::now();
        // the picture is taken with the clock's lowest cell at y = 72: the camera then stands above
        // the client's horizon height (63), below which it draws the sky beyond the loaded floor
        // black (dark bands in the pictures)
        let dy = 72 - p.lo[1];
        let c = &c.translated([0, dy, 0]);
        let p = &p.shifted(dy);
        let sb = if cam_in == Some(pair[0]) { pair[1] } else { pair[0] };
        w.clear(sb)?;
        place(&mut w, sb, c)?;
        let fy = p.lo[1] - 2;
        w.fill(sb, [p.lo[0] - m, fy, p.lo[2] - m], [p.hi[0] + m, fy, p.hi[2] + m], "minecraft:grass_block", PlaceMode::Raw)?;
        w.exec(sb, "time set noon")?;
        let t_build = t0.elapsed().as_secs_f64();
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
        // the main picture. A design with entities (a minecart): the level sends its entities to a
        // viewer only in its own tick, and the picture's level is frozen, so it stops one tick short,
        // the camera arrives, and the last tick runs with the viewer there (the cart is paired and
        // drawn); the marker stands during that one tick (an air cell off the track)
        let entity_shot = !c.design.entities.is_empty() && p.ticks[0] > 0;
        step_to(&mut w, if entity_shot { p.ticks[0] - 1 } else { p.ticks[0] }, &mut t, &mut actions)?;
        let t_step = t0.elapsed().as_secs_f64() - t_build;
        let marker = match p.output {
            Some(op) => marker_cell(&mut w, sb, op)?,
            None => None,
        };
        let (mut flo, mut fhi) = p.frame();
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
        // the camera arrives with the marker already written (the client loads the chunks with it)
        let t_cam0 = t0.elapsed().as_secs_f64();
        let hold = if i == 0 { o.hold_ms.max(12_000) } else { o.hold_ms };
        if entity_shot {
            if let Some(m) = marker {
                w.set_blocks(sb, &[(m, MARKER)], PlaceMode::Raw)?;
            }
            w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
            w.hold(hold)?;
            step_to(&mut w, p.ticks[0], &mut t, &mut actions)?;
            w.hold(1500)?;
            video::screenshot(&display, rw, rh, &raw)?;
            if let Some(m) = marker {
                w.set_blocks(sb, &[(m, "minecraft:air")], PlaceMode::Raw)?;
            }
        } else {
            screenshot_marked(&mut w, sb, marker, Some((eye, yaw, pitch)), (eye, yaw, pitch), None, &display, rw, rh, hold, &raw)?;
        }
        cam_in = Some(sb);
        let t_shot = t0.elapsed().as_secs_f64() - t_cam0;
        let out = shot_path(&o.out_dir, stem);
        let t_enc0 = t0.elapsed().as_secs_f64();
        let encode = |raw: &Path, out: &Path| encode_picture(raw, out, o.width);
        encode(&raw, &out)?;
        // a dark or nearly uniform frame (the client still loading, an error screen): taken again
        // after a longer wait (the client keeps showing the marker: its removal is never sent)
        let mut retakes = 0;
        while let Some(why) = bad_frame(&raw, &out) {
            if retakes == 2 {
                lines.push(format!("BAD {}: {why} after 2 retakes", out.display()));
                break;
            }
            retakes += 1;
            eprintln!("RETAKE {stem}: {why}");
            w.hold(3000)?;
            video::screenshot(&display, rw, rh, &raw)?;
            encode(&raw, &out)?;
        }
        let t_enc = t0.elapsed().as_secs_f64() - t_enc0;
        if std::env::var_os("MCTAS_SHOT_DEBUG").is_some() {
            eprintln!("{stem}: build {t_build:.2} s, run to tick {t_step:.2} s, camera + hold + capture {t_shot:.2} s, encode {t_enc:.2} s");
        }
        // the strip: N moments of one period (the main picture's tick may be later: a fresh build)
        if p.ticks.len() > 1 {
            let mut frames: Vec<PathBuf> = Vec::new();
            let strip_ticks: Vec<u32> = p.ticks[1..].to_vec();
            {
                // the strip replays the clock in the other sandbox
                let sb2 = if sb == pair[0] { pair[1] } else { pair[0] };
                w.clear(sb2)?;
                place(&mut w, sb2, c)?;
                w.fill(sb2, [p.lo[0] - m, fy, p.lo[2] - m], [p.hi[0] + m, fy, p.hi[2] + m], "minecraft:grass_block", PlaceMode::Raw)?;
                w.exec(sb2, "time set noon")?;
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
                        screenshot_marked(&mut w, sb2, mk, Some((eye, yaw, pitch)), (eye, yaw, pitch), None, &display, rw, rh, o.hold_ms, &f)?;
                    } else {
                        screenshot_marked(&mut w, sb2, mk, None, (eye, yaw, pitch), None, &display, rw, rh, 1500, &f)?;
                    }
                    frames.push(f);
                }
                cam_in = Some(sb2);
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
        lines.push(format!("SHOT {} (tick {}{}; {:.1} s)", out.display(), p.ticks[0], if p.ticks.len() > 1 { format!(", strip of {}", p.ticks.len() - 1) } else { String::new() }, t0.elapsed().as_secs_f64()));
        eprintln!("{}", lines.last().unwrap());
    }
    drop(client);
    Ok(lines)
}
