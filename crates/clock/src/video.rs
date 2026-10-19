//! `mctas clock video` (COORDINATOR ONLY: the X displays are shared): the clock running in the real
//! Minecraft client, filmed by a still camera framing its swept box, for at least 2 periods (and
//! at least `min_secs` of footage) after its warmup, with a corner label (the period big; family,
//! blocks, box, duty and start small) and an optional title card. Built on `mctas::video` (Xvfb,
//! the unmodified client, the recorder, the static ffmpeg) exactly like `door video`. The clean
//! clip (no label) is kept next to the video (`<out>.clip.mp4`) so `--relabel` recuts in seconds.

use crate::sim::{SimOptions, Verdict, simulate};
use crate::verify::{VerifyOptions, apply, place, run_one};
use crate::{Clock, wave};
use anyhow::{Context, Result, bail};
use mctas::mcfont::{self, Font, Image, LINE_HEIGHT};
use mctas::sim::{PlaceMode, SandboxOptions, Worker, WorkerConfig};
use mctas::video::{self, Client, Display, Recorder};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

const YELLOW: [u8; 3] = [255, 255, 85];
const WHITE: [u8; 3] = [255, 255, 255];
const GRAY: [u8; 3] = [170, 170, 170];
const AQUA: [u8; 3] = [85, 255, 255];
/// Vertical field of view of the client (options.txt fov:0.0 = 70 degrees).
const VFOV: f64 = 70.0;

/// Knobs of [`video`].
#[derive(Clone, Debug)]
pub struct ClockVideoOptions {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Periods filmed at least.
    pub periods: u32,
    /// Seconds of clock footage at least (more periods for fast clocks).
    pub min_secs: f64,
    /// Ticks per second while filming (default: 10, or faster for long periods: <= 60 s).
    pub tps: Option<f64>,
    /// A title card before the clip.
    pub card: bool,
    /// Cut the video again from the kept clip (no recording).
    pub relabel: bool,
    /// Print the plan and write the label (and card) images only: no display, no client.
    pub dry_run: bool,
    pub crf: u32,
}

impl Default for ClockVideoOptions {
    fn default() -> Self {
        ClockVideoOptions { width: 1280, height: 720, fps: 30, periods: 2, min_secs: 8.0, tps: None, card: false, relabel: false, dry_run: false, crf: 18 }
    }
}

/// What the video shows, measured before filming.
#[derive(Clone, Debug)]
pub struct Plan {
    pub name: String,
    pub period: u32,
    pub warmup: u32,
    pub family: String,
    pub blocks: usize,
    pub dims: [i32; 3],
    pub duty: String,
    pub start: String,
    /// The swept box (camera framing).
    pub lo: [i32; 3],
    pub hi: [i32; 3],
    /// Ticks stepped before filming (the warmup; the kick happens there).
    pub skip: u32,
    /// Ticks filmed (a whole number of periods).
    pub ticks: u32,
    pub tps: f64,
}

impl Plan {
    pub fn label_lines(&self) -> Vec<(String, u32, [u8; 3])> {
        let [w, h, d] = self.dims;
        vec![
            (format!("P = {}", crate::gt_time(self.period)), 4, YELLOW),
            (format!("{}, {} blocks, {w}x{h}x{d}", self.family.replace('+', " + "), self.blocks), 2, WHITE),
            (format!("on {}, {}", self.duty, self.start), 2, GRAY),
            (self.name.clone(), 2, AQUA),
        ]
    }

    pub fn card_lines(&self) -> Vec<(String, u32, [u8; 3])> {
        let [w, h, d] = self.dims;
        vec![
            ("redstone clock".to_string(), 4, GRAY),
            (format!("{} gt", self.period), 12, YELLOW),
            (crate::real_time(self.period), 4, YELLOW),
            (self.family.replace('+', " + "), 4, WHITE),
            (format!("{} blocks, {w}x{h}x{d}", self.blocks), 3, WHITE),
            (format!("on {}, {}", self.duty, self.start), 3, GRAY),
            (self.name.clone(), 3, AQUA),
        ]
    }
}

/// Measure the clock (redsim; the real game when redsim does not model it) and plan the film.
pub fn plan(version: &str, c: &Clock, o: &ClockVideoOptions) -> Result<Plan> {
    let r = simulate(c, &SimOptions::default());
    let (period, warmup, family, dims, duty, lo, hi) = match r.verdict {
        Verdict::Clock => {
            let duty = r.output.as_ref().map(|w| format!("{}/{}", w.wave.on, r.period)).unwrap_or_default();
            (r.period, r.state_warmup.max(r.warmup), wave::signature(&r.analysis.family), r.dims(), duty, r.swept.0, r.swept.1)
        }
        Verdict::Unmodeled => {
            let vo = VerifyOptions { positions: 1, rotations: false, version: version.to_string(), ..Default::default() };
            let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
            let k = match run_one(&mut w, c, &vo)? {
                Ok(k) => k,
                Err(e) => bail!("{}: not a clock in the game: {e}", c.name()),
            };
            let duty = k.output.as_ref().map(|w| format!("{}/{}", w.wave.on, k.period)).unwrap_or_default();
            let (lo, _) = c.bounds();
            (k.period, k.warmup, wave::signature(&k.analysis.family), k.dims, duty, lo, [lo[0] + k.dims[0] - 1, lo[1] + k.dims[1] - 1, lo[2] + k.dims[2] - 1])
        }
        _ => bail!("{}: not a clock: {}", c.name(), r.summary()),
    };
    let mut tps = o.tps.unwrap_or(if 2 * period <= 160 { 10.0 } else { 20.0 });
    let periods_for_secs = ((o.min_secs * tps) / period as f64).ceil() as u32;
    let ticks = period * o.periods.max(2).max(periods_for_secs);
    if o.tps.is_none() && ticks as f64 / tps > 60.0 {
        tps = ticks as f64 / 60.0;
    }
    Ok(Plan {
        name: c.name().to_string(),
        period,
        warmup,
        family,
        blocks: c.blocks(),
        dims,
        duty,
        start: c.start_label(),
        lo,
        hi,
        skip: warmup.max(c.free_from()),
        ticks,
        tps,
    })
}

fn ffmpeg_run(args: &[String]) -> Result<()> {
    let out = Command::new(video::ffmpeg()?).args(["-loglevel", "error", "-y"]).args(args).output()?;
    if !out.status.success() {
        bail!("ffmpeg {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// A transparent overlay with a translucent box of lines (text, scale at 720p, color) at the
/// bottom-left corner.
pub fn label_image(font: &Font, w: u32, h: u32, lines: &[(String, u32, [u8; 3])]) -> Image {
    let mut img = Image::new(w, h, [0, 0, 0, 0]);
    let k = h as f64 / 720.0;
    let sc = |s: u32| ((s as f64 * k).round() as u32).max(1);
    let pad = sc(10) as i64;
    let margin = sc(20) as i64;
    let max_w = (w as f64 * 0.7) as u32;
    let mut laid: Vec<(String, u32, [u8; 3])> = Vec::new();
    for (text, s, color) in lines {
        for l in font.wrap(text, sc(*s), max_w) {
            laid.push((l, sc(*s), *color));
        }
    }
    let box_w = laid.iter().map(|(t, s, _)| font.width(t, *s) as i64).max().unwrap_or(0) + 2 * pad;
    let box_h: i64 = laid.iter().map(|(_, s, _)| (LINE_HEIGHT * s) as i64).sum::<i64>() + 2 * pad - sc(1) as i64;
    let y0 = h as i64 - margin - box_h;
    img.fill_rect(margin, y0, box_w, box_h, [0, 0, 0, 150]);
    let mut ty = y0 + pad;
    for (t, s, color) in &laid {
        font.draw(&mut img, margin + pad, ty, t, *s, *color, true);
        ty += (LINE_HEIGHT * s) as i64;
    }
    img
}

/// A title card on the game's darkened dirt background, lines centered.
pub fn card_image(font: &Font, dirt: &Image, w: u32, h: u32, lines: &[(String, u32, [u8; 3])]) -> Image {
    let k = h as f64 / 720.0;
    let sc = |s: u32| ((s as f64 * k).round() as u32).max(1);
    let mut img = Image::new(w, h, [0, 0, 0, 255]);
    img.tile(dirt, sc(5), 0.25);
    let gap = sc(6) as i64;
    let total: i64 = lines.iter().map(|(_, s, _)| (LINE_HEIGHT * sc(*s)) as i64 + gap).sum::<i64>() - gap;
    let mut y = (h as i64 - total) / 2;
    for (text, s, color) in lines {
        font.draw_centered(&mut img, w as i64 / 2, y, text, sc(*s), *color, true);
        y += (LINE_HEIGHT * sc(*s)) as i64 + gap;
    }
    img
}

/// The clean clip's path for a video `out`.
pub fn clip_path(out: &Path) -> PathBuf {
    out.with_extension("clip.mp4")
}

/// Film the clean clip: the clock placed and kicked like the judge, its warmup stepped unseen, then
/// `plan.ticks` ticks played at `plan.tps` for a still camera; its first frame as `<clip>.png`.
pub fn record_clip(version: &str, c: &Clock, p: &Plan, clip: &Path, o: &ClockVideoOptions) -> Result<()> {
    static RUN_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let client_build = video::build_client(version)?;
    let display = Display::start(o.width, o.height)?;
    let port = {
        let l = std::net::TcpListener::bind("127.0.0.1:0")?;
        l.local_addr()?.port()
    };
    let n = RUN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let run = mctas::paths::run_dir().join(format!("clock-video-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&run)?;
    let cfg = WorkerConfig { version: version.to_string(), listen: Some(port), stderr_log: Some(run.join("server.log")), ..Default::default() };
    let mut w = Worker::spawn(&cfg)?;
    let sb = w.create(&SandboxOptions { chunks: [-7, -7, 6, 6], seed: 0 })?;
    place(&mut w, sb, c)?;
    let fy = p.lo[1] - 2;
    w.fill(sb, [-110, fy, -110], [109, fy, 109], "minecraft:grass_block", PlaceMode::Raw)?;
    w.exec(sb, "time set noon")?;
    w.view_distance(sb, 8)?;
    // the kick and the warmup, unseen
    let mut actions = c.actions();
    for t in 0..=p.skip {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            apply(&mut w, sb, &a)?;
        }
        if t < p.skip {
            w.step(sb, 1)?;
        }
    }
    let mut client = Client::launch(&client_build, &display, port, o.width, o.height, &run.join("client"))?;
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
    let aspect = o.width as f64 / o.height as f64;
    let lo = [p.lo[0] as f64 - 0.25, p.lo[1] as f64 - 0.25, p.lo[2] as f64 - 0.25];
    let hi = [p.hi[0] as f64 + 1.25, p.hi[1] as f64 + 1.25, p.hi[2] as f64 + 1.25];
    let (eye, yaw, pitch) = video::frame_box(lo, hi, [-0.55, 0.6, 1.0], VFOV, aspect, 0.8);
    w.camera(sb, [eye[0], eye[1] - 1.62, eye[2]], yaw, pitch)?;
    w.hold(12_000)?;
    video::screenshot(&display, o.width, o.height, &clip.with_extension("png"))?;
    let raw = run.join("raw.mkv");
    let rec = Recorder::start(&display, o.width, o.height, o.fps, &raw)?;
    let t0 = Instant::now();
    let wait = |w: &mut Worker, until: f64| -> Result<()> {
        loop {
            let now = t0.elapsed().as_secs_f64();
            if now >= until {
                return Ok(());
            }
            w.hold((((until - now) * 1000.0) as u64).clamp(1, 20))?;
        }
    };
    let mut clock = 1.0;
    wait(&mut w, clock)?;
    for _ in 0..p.ticks {
        w.play(sb, 1, 1000.0)?;
        clock += 1.0 / p.tps;
        wait(&mut w, clock)?;
    }
    clock += 1.0;
    wait(&mut w, clock)?;
    let raw = rec.stop()?;
    drop(client);
    video::encode_crf(&raw, clip, 1.0, o.crf)?;
    Ok(())
}

/// The published video: the clean clip with the corner label (and the title card first).
pub fn compose(version: &str, p: &Plan, clip: &Path, out: &Path, o: &ClockVideoOptions) -> Result<()> {
    let font = Font::minecraft(version)?;
    let label = out.with_extension("label.png");
    label_image(&font, o.width, o.height, &p.label_lines()).save_png(&label)?;
    let mut args: Vec<String> = Vec::new();
    let graph;
    if o.card {
        let card = out.with_extension("card.png");
        card_image(&font, &mcfont::texture(version, "block/dirt")?, o.width, o.height, &p.card_lines()).save_png(&card)?;
        let title = 2.0;
        args.extend(["-loop", "1", "-framerate", &o.fps.to_string(), "-t", "2.0", "-i"].map(String::from));
        args.push(card.display().to_string());
        args.extend(["-i".to_string(), clip.display().to_string(), "-i".to_string(), label.display().to_string()]);
        graph = format!(
            "[1:v]setpts=PTS-STARTPTS,fps={fps}[clip];[clip][2:v]overlay=0:0,setsar=1,format=yuv420p,fade=t=in:st=0:d=0.35[v];[0:v]setsar=1,format=yuv420p,fade=t=out:st={:.3}:d=0.35[c];[c][v]concat=n=2:v=1:a=0[o]",
            title - 0.35,
            fps = o.fps
        );
    } else {
        args.extend(["-i".to_string(), clip.display().to_string(), "-i".to_string(), label.display().to_string()]);
        graph = "[0:v][1:v]overlay=0:0,setsar=1,format=yuv420p[o]".to_string();
    }
    args.extend(["-filter_complex".to_string(), graph, "-map".into(), "[o]".into()]);
    args.extend(["-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"].map(String::from));
    args.push(out.display().to_string());
    let r = ffmpeg_run(&args);
    let _ = std::fs::remove_file(&label);
    let _ = std::fs::remove_file(out.with_extension("card.png"));
    r
}

/// `mctas clock video`: plan, film the clip (unless `relabel` and it exists), cut the video.
pub fn video(version: &str, c: &Clock, out: &Path, o: &ClockVideoOptions) -> Result<Plan> {
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let p = plan(version, c, o)?;
    if o.dry_run {
        let font = Font::minecraft(version)?;
        label_image(&font, o.width, o.height, &p.label_lines()).save_png(&out.with_extension("label.png"))?;
        if o.card {
            card_image(&font, &mcfont::texture(version, "block/dirt")?, o.width, o.height, &p.card_lines()).save_png(&out.with_extension("card.png"))?;
        }
        return Ok(p);
    }
    let clip = clip_path(out);
    if !(o.relabel && clip.exists()) {
        record_clip(version, c, &p, &clip, o).with_context(|| format!("filming {}", c.name()))?;
    }
    compose(version, &p, &clip, out, o)?;
    let _ = std::fs::copy(clip.with_extension("png"), out.with_extension("png"));
    Ok(p)
}

/// `mctas clock montage`: for each period, every given clock's labeled video (each clock filmed
/// alone; a clip already filmed is reused) after a title card, concatenated into
/// `<out_dir>/montage-p<P>.mp4`; the per-clock videos stay in `<out_dir>/clips/`. Returns one line
/// per montage.
pub fn montage(version: &str, items: &[(String, Clock)], out_dir: &Path, periods: Option<&[u32]>, o: &ClockVideoOptions) -> Result<Vec<String>> {
    let clips = out_dir.join("clips");
    std::fs::create_dir_all(&clips)?;
    let mut by_period: std::collections::BTreeMap<u32, Vec<(String, PathBuf, Plan)>> = std::collections::BTreeMap::new();
    let mut lines = Vec::new();
    for (stem, c) in items {
        let p = match plan(version, c, o) {
            Ok(p) => p,
            Err(e) => {
                lines.push(format!("SKIP {stem}: {e:#}"));
                continue;
            }
        };
        if periods.is_some_and(|ps| !ps.contains(&p.period)) {
            continue;
        }
        let out = clips.join(format!("{stem}.mp4"));
        let vo = ClockVideoOptions { relabel: true, card: false, dry_run: false, ..o.clone() };
        match video(version, c, &out, &vo) {
            Ok(_) => by_period.entry(p.period).or_default().push((stem.clone(), out, p)),
            Err(e) => lines.push(format!("SKIP {stem}: {e:#}")),
        }
    }
    let font = Font::minecraft(version)?;
    let dirt = mcfont::texture(version, "block/dirt")?;
    for (period, mut vids) in by_period {
        vids.sort_by(|a, b| (a.2.blocks, &a.0).cmp(&(b.2.blocks, &b.0)));
        let out = out_dir.join(format!("montage-p{period}.mp4"));
        let card = out.with_extension("card.png");
        let lines_card = vec![
            ("redstone clocks".to_string(), 4, GRAY),
            (format!("{period} gt"), 12, YELLOW),
            (crate::real_time(period), 4, YELLOW),
            (format!("{} clock{}, smallest first", vids.len(), if vids.len() == 1 { "" } else { "s" }), 4, WHITE),
        ];
        card_image(&font, &dirt, o.width, o.height, &lines_card).save_png(&card)?;
        let mut args: Vec<String> = ["-loop", "1", "-framerate", &o.fps.to_string(), "-t", "2.5", "-i"].map(String::from).to_vec();
        args.push(card.display().to_string());
        for (_, v, _) in &vids {
            args.extend(["-i".to_string(), v.display().to_string()]);
        }
        let n = vids.len() + 1;
        let norm: String = (0..n).map(|k| format!("[{k}:v]scale={}:{},setsar=1,fps={},format=yuv420p[v{k}];", o.width, o.height, o.fps)).collect();
        let cat: String = (0..n).map(|k| format!("[v{k}]")).collect();
        args.extend(["-filter_complex".to_string(), format!("{norm}{cat}concat=n={n}:v=1:a=0[o]"), "-map".into(), "[o]".into()]);
        args.extend(["-c:v", "libx264", "-preset", "slow", "-crf", "22", "-pix_fmt", "yuv420p", "-movflags", "+faststart"].map(String::from));
        args.push(out.display().to_string());
        let r = ffmpeg_run(&args);
        let _ = std::fs::remove_file(&card);
        r?;
        lines.push(format!("MONTAGE {} ({} clocks: {})", out.display(), vids.len(), vids.iter().map(|v| v.0.as_str()).collect::<Vec<_>>().join(", ")));
    }
    Ok(lines)
}
