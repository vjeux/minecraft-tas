//! `mctas door video`: the door opening and closing, rendered by the real Minecraft client.
//!
//! Default style: a short title card (block count, category, timings), then the door seen by a
//! camera centered between the 3x3 opening and the machine that swings slowly around it (it
//! ends where it started, so clips loop), with a block-count label in the corner.
//! `--style classic` is the original still camera framing the whole build, no card, no label.

use crate::DoorSpec;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker, WorkerConfig};
use mctas::video::{self, Client, Display, Recorder};
use std::path::Path;
use std::time::{Duration, Instant};

/// How the camera moves.
#[derive(Clone, Copy, Debug)]
pub enum Camera {
    /// The whole build from the front-left, slightly above (the original framing).
    Classic,
    /// Door-centric shot ([`door_shot`]) swinging `orbit` degrees each way (0 = still).
    Door { orbit: f64 },
    /// An explicit shot in the design's coordinates (e.g. one camera for a whole montage).
    Shot(Shot),
}

pub struct VideoOptions {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Simulation ticks per second while recording (20 = real time).
    pub tps: f64,
    /// Ticks to play after each lever flip.
    pub ticks: u32,
    pub camera: Camera,
    /// Seconds of title card before the clip (0 = none).
    pub title_secs: f64,
    /// Burn a block-count label into the bottom-left corner.
    pub label: bool,
    /// Height of the grass floor (design coordinates); default two blocks under the build.
    pub floor_y: Option<i32>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        VideoOptions {
            width: 1280,
            height: 720,
            fps: 30,
            tps: 10.0,
            ticks: 40,
            camera: Camera::Door { orbit: 20.0 },
            title_secs: 2.0,
            label: true,
            floor_y: None,
        }
    }
}

impl VideoOptions {
    /// The original look: still camera framing the whole build, no title card, no label.
    pub fn classic() -> VideoOptions {
        VideoOptions { camera: Camera::Classic, title_secs: 0.0, label: false, ..Default::default() }
    }

    /// Door camera without title card or label (raw clips for galleries and montages).
    pub fn clean(orbit: f64) -> VideoOptions {
        VideoOptions { camera: Camera::Door { orbit }, title_secs: 0.0, label: false, ..Default::default() }
    }

    /// Seconds of door footage per clip (holds + both plays) for a door that settles within
    /// `ticks`; `record` plays longer for a door that is still moving then.
    pub fn clip_secs(&self) -> f64 {
        HOLD_START + HOLD_OPEN + HOLD_END + 2.0 * self.ticks as f64 / self.tps
    }
}

const HOLD_START: f64 = 1.5;
const HOLD_OPEN: f64 = 1.2;
const HOLD_END: f64 = 1.5;
/// Vertical field of view of the client (options.txt fov:0.0 = 70 degrees).
const VFOV: f64 = 70.0;

/// A camera aimed at `target` from direction `dir`, `dist` blocks away, swinging around the
/// vertical axis through the target by `orbit * sin(2 pi t)` degrees over a clip (t = 0..1):
/// it starts and ends at the same pose, so a clip loops seamlessly.
#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub target: [f64; 3],
    /// Unit vector from the target toward the eye at t = 0.
    pub dir: [f64; 3],
    pub dist: f64,
    pub orbit: f64,
}

impl Shot {
    pub fn direction(&self, t: f64) -> [f64; 3] {
        let a = (self.orbit * (2.0 * std::f64::consts::PI * t).sin()).to_radians();
        let (s, c) = a.sin_cos();
        [self.dir[0] * c + self.dir[2] * s, self.dir[1], -self.dir[0] * s + self.dir[2] * c]
    }

    /// Eye position, yaw and pitch at clip time `t` (0..1).
    pub fn pose(&self, t: f64) -> ([f64; 3], f32, f32) {
        let d = self.direction(t);
        let eye = [self.target[0] + d[0] * self.dist, self.target[1] + d[1] * self.dist, self.target[2] + d[2] * self.dist];
        let (yaw, pitch) = video::look_at(eye, self.target);
        (eye, yaw, pitch)
    }

    /// The same shot moved by `d` blocks (a design placed elsewhere).
    pub fn translated(&self, d: Pos) -> Shot {
        Shot { target: [self.target[0] + d[0] as f64, self.target[1] + d[1] as f64, self.target[2] + d[2] as f64], ..*self }
    }
}

/// Corners of every block (unit cubes at the positions).
fn corners(blocks: &[Pos]) -> Vec<[f64; 3]> {
    let mut out = Vec::with_capacity(blocks.len() * 8);
    for p in blocks {
        for i in 0..8 {
            out.push([(p[0] + (i & 1)) as f64, (p[1] + ((i >> 1) & 1)) as f64, (p[2] + ((i >> 2) & 1)) as f64]);
        }
    }
    out
}

/// Screen-space bounds (x0, x1, y0, y1; 1.0 = frame edge) of `points` seen from `eye` looking
/// at `target`, or None when a point is behind the camera.
fn bounds(points: &[[f64; 3]], eye: [f64; 3], target: [f64; 3], aspect: f64) -> Option<(f64, f64, f64, f64)> {
    let (f, right, up) = basis(eye, target);
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let tan_v = (VFOV.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for p in points {
        let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
        let z = dot(v, f);
        if z <= 0.3 {
            return None;
        }
        let (x, y) = (dot(v, right) / z / tan_h, dot(v, up) / z / tan_v);
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    Some((x0, x1, y0, y1))
}

/// Forward, right and up unit vectors of a camera at `eye` looking at `target`.
fn basis(eye: [f64; 3], target: [f64; 3]) -> ([f64; 3], [f64; 3], [f64; 3]) {
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let norm = |a: [f64; 3]| {
        let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let f = norm([target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]]);
    let right = norm(cross(f, [0.0, 1.0, 0.0]));
    let up = cross(right, f);
    (f, right, up)
}

/// How much of the 3x3 opening other blocks hide from a camera looking from direction `dir`
/// (swinging `orbit` degrees): rays from points of the door's face toward the camera, counting
/// the blocks they cross (door cells excluded).
fn occlusion(solid: &std::collections::HashSet<Pos>, door: Pos, dir: [f64; 3], orbit: f64, dist: f64) -> usize {
    let shot = Shot { target: [0.0; 3], dir, dist, orbit };
    let mut hits = 0;
    for t in [0.0, 0.25, 0.5, 0.75] {
        let d = shot.direction(t);
        for cx in 0..3 {
            for cy in 0..3 {
                for (qx, qy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                    let z = door[2] as f64 + if d[2] > 0.0 { 1.01 } else { -0.01 };
                    let p = [(door[0] + cx) as f64 + qx, (door[1] + cy) as f64 + qy, z];
                    let mut last: Option<Pos> = None;
                    let mut s = 0.0;
                    while s < dist {
                        let q = [p[0] + d[0] * s, p[1] + d[1] * s, p[2] + d[2] * s];
                        let cell = [q[0].floor() as i32, q[1].floor() as i32, q[2].floor() as i32];
                        if last != Some(cell) {
                            last = Some(cell);
                            let in_door = cell[2] == door[2] && (0..3).contains(&(cell[0] - door[0])) && (0..3).contains(&(cell[1] - door[1]));
                            if !in_door && solid.contains(&cell) {
                                hits += 1;
                            }
                        }
                        s += 0.1;
                    }
                }
            }
        }
    }
    hits
}

/// Door-centric shot of `blocks` (design coordinates, door port `door`): seen from the front or
/// the back of the door, from its left or its right, 15 or 30 degrees above, whichever hides the
/// 3x3 opening least over the orbit (ties: front-left, lower). The whole build is centered on
/// screen over the orbit (so it is as large as possible), then the aim moves a quarter of the
/// way toward the center of the opening, and the camera backs off just enough that every block
/// stays inside `fill` of the frame during the whole orbit.
pub fn door_shot(blocks: &[Pos], door: Pos, orbit: f64, aspect: f64, fill: f64) -> Shot {
    let door_c = [door[0] as f64 + 1.5, door[1] as f64 + 1.5, door[2] as f64 + 0.5];
    let pts = corners(blocks);
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in &pts {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let solid: std::collections::HashSet<Pos> = blocks.iter().copied().collect();
    let size = (hi[0] - lo[0]).max(hi[1] - lo[1]).max(hi[2] - lo[2]);
    let mut best: Option<(usize, usize, [f64; 3])> = None;
    let mut rank = 0;
    for sz in [1.0, -1.0] {
        for sx in [-1.0, 1.0] {
            for y in [0.3, 0.6] {
                let raw: [f64; 3] = [0.4 * sx * sz, y, sz];
                let l = (raw[0] * raw[0] + raw[1] * raw[1] + raw[2] * raw[2]).sqrt();
                let dir = [raw[0] / l, raw[1] / l, raw[2] / l];
                let score = occlusion(&solid, door, dir, orbit, 2.0 * size + 10.0);
                if best.is_none_or(|b| (score, rank) < (b.0, b.1)) {
                    best = Some((score, rank, dir));
                }
                rank += 1;
            }
        }
    }
    let dir = best.unwrap().2;
    let base = Shot { target: [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0], dir, dist: 10.0, orbit };
    let samples: Vec<f64> = (0..=16).map(|i| i as f64 / 16.0).collect();
    // Union of the screen bounds over the orbit; None when a block gets behind the camera.
    let union = |target: [f64; 3], dist: f64| -> Option<(f64, f64, f64, f64)> {
        let s = Shot { target, dist, ..base };
        let mut u = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for &t in &samples {
            let (eye, _, _) = s.pose(t);
            let (x0, x1, y0, y1) = bounds(&pts, eye, target, aspect)?;
            u = (u.0.min(x0), u.1.max(x1), u.2.min(y0), u.3.max(y1));
        }
        Some(u)
    };
    let fits = |target: [f64; 3], dist: f64| union(target, dist).is_some_and(|(x0, x1, y0, y1)| x0.abs().max(x1.abs()).max(y0.abs()).max(y1.abs()) <= fill);
    let solve = |target: [f64; 3]| -> f64 {
        let (mut a, mut b) = (1.0, 500.0);
        for _ in 0..60 {
            let mid = (a + b) / 2.0;
            if fits(target, mid) { b = mid } else { a = mid }
        }
        b
    };
    let tan_v = (VFOV.to_radians() / 2.0).tan();
    let tan_h = tan_v * aspect;
    let mut target = base.target;
    for _ in 0..6 {
        let dist = solve(target);
        let Some((x0, x1, y0, y1)) = union(target, dist) else { break };
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (eye, _, _) = Shot { target, dist, ..base }.pose(0.0);
        let (_, right, up) = basis(eye, target);
        for i in 0..3 {
            target[i] += right[i] * cx * dist * tan_h + up[i] * cy * dist * tan_v;
        }
    }
    let k = 0.25;
    for i in 0..3 {
        target[i] += k * (door_c[i] - target[i]);
    }
    Shot { target, dist: solve(target), ..base }
}

/// The door shot of one design.
pub fn design_shot(d: &Design, orbit: f64, aspect: f64) -> Result<Shot> {
    let spec = DoorSpec::from_design(d)?;
    let blocks: Vec<Pos> = d.blocks.keys().copied().collect();
    Ok(door_shot(&blocks, spec.door, orbit, aspect, 0.9))
}

pub(crate) fn free_port() -> Result<u16> {
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
    // A design with entities (category entity): summoned like the judge does, once the blocks stand.
    if !d.entities.is_empty() {
        w.summon(sb, &d.entities)?;
    }
    w.step_until_idle(sb, 400)?;
    Ok(())
}

/// Facts shown on the title card and the label.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub name: String,
    pub blocks: usize,
    pub category: String,
    pub slime_free: bool,
    /// Every block in the door's plane (one block thick).
    pub flat: bool,
    /// Also works turned 90, 180 and 270 degrees (from the leaderboard's cache; true when
    /// unknown, e.g. a title card made outside the boards).
    pub rotation_safe: bool,
    /// Ticks after the lever flip until open / closed (first cycle), when the door works.
    pub open_ticks: Option<u32>,
    pub close_ticks: Option<u32>,
    /// The build's shape around its door ([`crate::shape`] at rest; the boards' swept shape,
    /// [`crate::swept_shape`], when it comes from the leaderboard cache).
    pub shape: Option<crate::Shape>,
    /// Nothing below the door and it works standing on a stone floor (from the leaderboard's
    /// ground check; false when unknown).
    pub ground_safe: bool,
}

impl Facts {
    pub fn of(d: &Design) -> Facts {
        Facts {
            name: d.name.clone(),
            blocks: crate::block_count(d),
            category: crate::category(d).to_string(),
            slime_free: crate::slime_free(d),
            flat: crate::single_layer(d),
            rotation_safe: true,
            open_ticks: None,
            close_ticks: None,
            shape: crate::shape(d),
            ground_safe: false,
        }
    }

    /// "solid, slime-free" etc.
    pub fn kind(&self) -> String {
        let mut s = self.category.clone();
        if self.slime_free {
            s.push_str(", slime-free");
        }
        if self.flat {
            s.push_str(", flat");
        }
        s
    }

    pub fn timing(&self) -> Option<String> {
        Some(format!("opens in {} ticks, closes in {}", self.open_ticks?, self.close_ticks?))
    }
}

enum Step {
    Hold(f64),
    Lever,
    Ticks(u32),
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
    let mut facts = Facts::of(d);
    // One cycle at the origin (the judge's first position): the card's timings, and how long to
    // play after each flip: at least until nothing moves anymore, so the second flip never comes
    // while the machine is still moving (a fixed 40 ticks cut the slow doors short).
    let mut ticks = opts.ticks;
    let vo = crate::VerifyOptions { cycles: 1, offsets: vec![[0, 0, 0]], empty_walkway: false, ..Default::default() };
    if let Ok(r) = crate::verify(&mut w, d, &vo) {
        if let Some(c) = r.cycles.first() {
            facts.open_ticks = Some(c.open_ticks);
            facts.close_ticks = Some(c.close_ticks);
            ticks = ticks.max(c.open_settle).max(c.close_settle);
        }
    }
    let sb = w.create(&SandboxOptions { chunks: [-7, -7, 6, 6], seed: 0 })?;
    place(&mut w, sb, d)?;
    // A grass floor under the build (never touching it) out to the client's render distance
    // (6 chunks), so the horizon is fog rather than the floor's edge; daylight.
    let (lo, _) = d.bounds().context("empty design")?;
    let fy = opts.floor_y.unwrap_or(lo[1] - 2).min(lo[1] - 2);
    w.fill(sb, [-110, fy, -110], [109, fy, 109], "minecraft:grass_block", PlaceMode::Raw)?;
    w.exec(sb, "time set noon")?;
    // Send the client every chunk it renders (6), not the level's default 2.
    w.view_distance(sb, 8)?;

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
    let aspect = opts.width as f64 / opts.height as f64;
    let shot = match opts.camera {
        Camera::Classic => None,
        Camera::Door { orbit } => Some(design_shot(d, orbit, aspect)?),
        Camera::Shot(s) => Some(s),
    };
    if let Some(s) = &shot {
        eprintln!("{}: camera aims at {:.1?} from {:.2?}, {:.1} blocks away, orbit {} degrees", d.name, s.target, s.dir, s.dist, s.orbit);
    }
    let (eye, yaw, pitch) = match shot {
        Some(s) => s.pose(0.0),
        None => {
            // Frame the whole build (block corners) from the front-left, slightly above. The
            // client runs with fov 70 (options.txt fov:0.0) at 16:9.
            let pts = corners(&d.blocks.keys().copied().collect::<Vec<_>>());
            // View direction (from the build toward the camera); MCTAS_VIEW="x,y,z" overrides.
            let view: Vec<f64> = std::env::var("MCTAS_VIEW")
                .ok()
                .map(|v| v.split(',').filter_map(|n| n.trim().parse().ok()).collect())
                .filter(|v: &Vec<f64>| v.len() == 3)
                .unwrap_or(vec![-0.45, 0.2, 1.0]);
            video::frame_points(&pts, [view[0], view[1], view[2]], VFOV, aspect, 0.92)
        }
    };
    w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
    // Let chunks arrive and the renderer warm up.
    w.hold(12_000)?;
    video::screenshot(&display, opts.width, opts.height, &out.with_extension("png"))?;

    let raw = run.join("raw.mkv");
    let rec = Recorder::start(&display, opts.width, opts.height, opts.fps, &raw)?;
    let plan = [Step::Hold(HOLD_START), Step::Lever, Step::Ticks(ticks), Step::Hold(HOLD_OPEN), Step::Lever, Step::Ticks(ticks), Step::Hold(HOLD_END)];
    let total = HOLD_START + HOLD_OPEN + HOLD_END + 2.0 * ticks as f64 / opts.tps;
    let moving = shot.filter(|s| s.orbit.abs() > 1e-9);
    let t0 = Instant::now();
    // Serve the client until `until` seconds after t0, moving the camera along the orbit.
    let wait = |w: &mut Worker, until: f64| -> Result<()> {
        loop {
            let now = t0.elapsed().as_secs_f64();
            if now >= until {
                return Ok(());
            }
            if let Some(s) = moving {
                let (eye, yaw, pitch) = s.pose((now / total).min(1.0));
                w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
            }
            w.hold((((until - now) * 1000.0) as u64).clamp(1, 20))?;
        }
    };
    let mut clock = 0.0;
    for step in &plan {
        match *step {
            Step::Hold(s) => {
                clock += s;
                wait(&mut w, clock)?;
            }
            Step::Lever => {
                w.use_block(sb, spec.lever)?;
            }
            Step::Ticks(n) => {
                for _ in 0..n {
                    w.play(sb, 1, 1000.0)?;
                    clock += 1.0 / opts.tps;
                    wait(&mut w, clock)?;
                }
            }
        }
    }
    if let Some(s) = moving {
        let (eye, yaw, pitch) = s.pose(1.0);
        w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
        w.hold(100)?;
    }
    let raw = rec.stop()?;
    drop(client);
    if opts.title_secs > 0.0 || opts.label {
        crate::showcase::finish_video(version, &raw, out, &facts, opts)?;
    } else {
        video::encode(&raw, out, 1.0)?;
    }
    Ok(())
}

/// Render a video of every design in `dir` (each `.txt`, README excluded) into `out/<stem>.mp4`,
/// `jobs` at a time, skipping videos that already exist (unless `redo`); upload each new one to
/// the GitHub release `upload` (as `<stem>.mp4`). With `redo_cut`, existing videos of doors that
/// are still moving after `opts.ticks` are recorded again (clips used to stop there). Writes
/// `dir/VIDEOS.md` linking every design to its video.
pub fn record_all(version: &str, dir: &Path, out: &Path, jobs: usize, upload: Option<&str>, opts: &VideoOptions, redo: bool, redo_cut: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    let has_video = |p: &std::path::PathBuf| out.join(format!("{}.mp4", p.file_stem().unwrap().to_string_lossy())).exists();
    let mut todo: Vec<std::path::PathBuf> = files.iter().filter(|p| redo || !has_video(p)).cloned().collect();
    if redo_cut && !redo {
        let have: Vec<std::path::PathBuf> = files.iter().filter(|p| has_video(p)).cloned().collect();
        let pool = mctas::sim::Pool::spawn(jobs.max(1) * 2, &WorkerConfig { version: version.to_string(), ..Default::default() })?;
        let vo = crate::VerifyOptions { cycles: 1, offsets: vec![[0, 0, 0]], empty_walkway: false, ..Default::default() };
        let settle = pool.map(have.clone(), |w, _, p| {
            let d = Design::load(&p)?;
            let r = crate::verify(w, &d, &vo)?;
            Ok(r.cycles.first().map(|c| c.open_settle.max(c.close_settle)).unwrap_or(0))
        });
        for (p, s) in have.into_iter().zip(settle) {
            if let Ok(s) = s
                && s > opts.ticks
            {
                eprintln!("{}: still moving after {} ticks (settles at {s}), re-recording", p.display(), opts.ticks);
                todo.push(p);
            }
        }
    }
    eprintln!("{} designs, {} videos to record ({} at a time)", files.len(), todo.len(), jobs);
    let queue = std::sync::Mutex::new(todo);
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| loop {
                let Some(p) = queue.lock().unwrap().pop() else { break };
                let stem = p.file_stem().unwrap().to_string_lossy().to_string();
                let mp4 = out.join(format!("{stem}.mp4"));
                let tmp = out.join(format!("{stem}.part.mp4"));
                let result = Design::load(&p).and_then(|d| record(version, &d, &tmp, opts));
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
    let mut md = String::from("# Videos of every archived design\n\nRendered by the real Minecraft client (`mctas door video-all`). The best of each board, as looping GIFs: [../GALLERY.md](../GALLERY.md).\n\n| blocks | category | design | video |\n|---|---|---|---|\n");
    for (b, stem, cat, link) in rows {
        md.push_str(&format!("| {b} | {cat} | [{stem}]({stem}.txt) | {link} |\n"));
    }
    std::fs::write(dir.join("VIDEOS.md"), md)?;
    Ok(())
}
