//! One recording per design. Every door is filmed once by the real client ([`record`]): a clean
//! clip (the door camera, still, no title card, no label; 16:9 with the build framed for a
//! centered 4:3 crop), cached by design content and recording settings in the clips folder
//! (`~/videos/clips/<design hash>.clip.mp4`, its first frame `.clip.png` and its sidecar
//! `.clip.txt`). It is recorded again only when the design or the recording settings change (or
//! `--redo`). Everything published is cut from it with ffmpeg, without starting Minecraft: the
//! video of `door video` / `video-all` ([`video`]: title card + corner label) and the GIF of `door
//! gallery` (the 4:3 crop + label, [`crate::showcase::gallery`]). A product is rebuilt when what it
//! is made of changed (the clip, its card and label lines: the `.stamp` file next to it), so a new
//! label costs seconds per door.

use crate::properties::Volume;
use crate::showcase::{self, Look};
use crate::video::{self as dv, Facts, VideoOptions};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;

/// Bump when the recording itself changes (floor, holds, warm-up, encode): every clip is recorded
/// again.
const LOOK: &str = "clip-v1";
/// Seconds of title card before a published video.
pub const TITLE_SECS: f64 = 2.0;
const REPO: &str = "vjeux/minecraft-tas";

/// How clips are recorded (part of each clip's cache key).
#[derive(Clone, Copy, Debug)]
pub struct ClipSettings {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// Simulation ticks per second while recording (20 = real time).
    pub tps: f64,
    /// Door camera swing in degrees each way (0 = still: GIFs of a moving camera are ~10x larger).
    pub orbit: f64,
    /// The build is framed for a centered crop of this aspect (4:3: the GIF's crop of the 16:9
    /// clip keeps the whole build).
    pub frame: (u32, u32),
}

impl Default for ClipSettings {
    fn default() -> Self {
        ClipSettings { width: 1280, height: 720, fps: 30, tps: 10.0, orbit: 0.0, frame: (4, 3) }
    }
}

impl ClipSettings {
    pub fn key(&self) -> String {
        format!("{LOOK} {}x{} fps{} tps{} orbit{} frame{}:{}", self.width, self.height, self.fps, self.tps, self.orbit, self.frame.0, self.frame.1)
    }

    fn options(&self) -> VideoOptions {
        VideoOptions {
            width: self.width,
            height: self.height,
            fps: self.fps,
            tps: self.tps,
            shot_aspect: Some(self.frame.0 as f64 / self.frame.1 as f64),
            crf: 16,
            ..VideoOptions::clean(self.orbit)
        }
    }
}

/// The clips folder by default: `~/videos/clips`.
pub fn default_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")).join("videos").join("clips")
}

/// A recorded clip, as its sidecar describes it.
#[derive(Clone, Debug)]
pub struct ClipInfo {
    pub mp4: PathBuf,
    pub hash: u64,
    pub settings: String,
    /// Ticks played after each flip (until the door settled).
    pub ticks: u32,
    /// Ticks after the flip until open / closed (first cycle at the origin), for the title card.
    pub open_ticks: Option<u32>,
    pub close_ticks: Option<u32>,
    /// The sidecar's text: a product's stamp includes it, so a new recording rebuilds them.
    pub text: String,
}

fn paths(dir: &Path, hash: u64) -> (PathBuf, PathBuf) {
    (dir.join(format!("{hash:016x}.clip.mp4")), dir.join(format!("{hash:016x}.clip.txt")))
}

fn parse(text: &str, mp4: PathBuf) -> Option<ClipInfo> {
    let get = |k: &str| text.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('=')));
    Some(ClipInfo {
        hash: u64::from_str_radix(get("hash")?, 16).ok()?,
        settings: get("settings")?.to_string(),
        ticks: get("ticks").and_then(|v| v.parse().ok()).unwrap_or(0),
        open_ticks: get("open_ticks").and_then(|v| v.parse().ok()),
        close_ticks: get("close_ticks").and_then(|v| v.parse().ok()),
        text: text.to_string(),
        mp4,
    })
}

/// The clip of `d` recorded with `s`, if the clips folder `dir` has it.
pub fn find(dir: &Path, d: &Design, s: &ClipSettings) -> Option<ClipInfo> {
    let hash = crate::leaderboard::design_hash(d);
    let (mp4, txt) = paths(dir, hash);
    let info = parse(&std::fs::read_to_string(txt).ok()?, mp4)?;
    (info.hash == hash && info.settings == s.key() && info.mp4.exists()).then_some(info)
}

/// Record the clip of `d` with the real client into the clips folder `dir` (replacing an older
/// one), with its sidecar.
pub fn record(version: &str, dir: &Path, d: &Design, s: &ClipSettings) -> Result<ClipInfo> {
    static RUN: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::fs::create_dir_all(dir)?;
    let hash = crate::leaderboard::design_hash(d);
    let (mp4, txt) = paths(dir, hash);
    let n = RUN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!("{hash:016x}.part-{}-{n}.mp4", std::process::id()));
    let t = Instant::now();
    let r = match dv::record(version, d, &tmp, &s.options()) {
        Ok(r) => r,
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            let _ = std::fs::remove_file(tmp.with_extension("png"));
            return Err(e);
        }
    };
    std::fs::rename(&tmp, &mp4)?;
    let _ = std::fs::rename(tmp.with_extension("png"), mp4.with_extension("png"));
    let opt = |v: Option<u32>| v.map(|v| v.to_string()).unwrap_or_default();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|t| t.as_secs()).unwrap_or(0);
    let text = format!(
        "# the one recording of this design (door video, video-all and gallery cut theirs from it; crates/piston-door/src/clips.rs)\n\
         hash={hash:016x}\nsettings={}\nname={}\nblocks={}\nticks={}\nopen_ticks={}\nclose_ticks={}\nrecorded={now}\nsecs={:.0}\n",
        s.key(),
        d.name,
        r.facts.blocks,
        r.ticks,
        opt(r.facts.open_ticks),
        opt(r.facts.close_ticks),
        t.elapsed().as_secs_f64()
    );
    std::fs::write(&txt, &text)?;
    parse(&text, mp4).context("clip sidecar")
}

/// Record the clips of `todo` (name, design), `jobs` at a time; returns the names that failed.
pub fn record_many(version: &str, dir: &Path, todo: Vec<(String, Design)>, s: &ClipSettings, jobs: usize) -> Vec<String> {
    let queue = Mutex::new(todo);
    let failed = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..jobs.max(1) {
            sc.spawn(|| {
                loop {
                    let Some((name, d)) = queue.lock().unwrap().pop() else { break };
                    let t = Instant::now();
                    match record(version, dir, &d, s) {
                        Ok(c) => eprintln!("recorded {name} ({:.0}s): {}", t.elapsed().as_secs_f64(), c.mp4.display()),
                        Err(e) => {
                            eprintln!("FAILED {name}: {e:#}");
                            failed.lock().unwrap().push(name);
                        }
                    }
                }
            });
        }
    });
    failed.into_inner().unwrap()
}

/// What a design's title card and label show: its own facts, its clip's timings and its volume.
pub fn facts(d: &Design, c: &ClipInfo, volume: Option<&Volume>) -> Facts {
    let mut f = Facts::of(d);
    f.open_ticks = c.open_ticks;
    f.close_ticks = c.close_ticks;
    if let Some((v, dims)) = volume {
        f.volume = Some(*v);
        f.volume_dims = Some(dims.clone());
    }
    f
}

/// The archive's properties cache (THE sizes), when commands run from the repository root.
fn volume_cache() -> Option<&'static Path> {
    let dir = Path::new(dv::ARCHIVE_DIR);
    dir.is_dir().then_some(dir)
}

/// A product's stamp: a hash of what it is made of.
pub fn stamp(parts: &[&str]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for p in parts {
        for b in p.bytes().chain([0u8]) {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
    }
    format!("{h:016x}")
}

fn stamp_path(out: &Path) -> PathBuf {
    let mut s = out.as_os_str().to_owned();
    s.push(".stamp");
    PathBuf::from(s)
}

/// `out` exists and was made of what `stamp` describes.
pub fn up_to_date(out: &Path, stamp: &str) -> bool {
    out.exists() && std::fs::read_to_string(stamp_path(out)).is_ok_and(|s| s.trim() == stamp)
}

pub fn write_stamp(out: &Path, stamp: &str) -> Result<()> {
    Ok(std::fs::write(stamp_path(out), format!("{stamp}\n"))?)
}

/// The stamp of a published video: the clip, the card and label lines, the style.
pub fn video_stamp(c: &ClipInfo, f: &Facts, card: bool) -> String {
    let lines = if card { format!("{:?} {:?} {TITLE_SECS}", showcase::title_lines(f), showcase::label_lines(f)) } else { "clean".into() };
    stamp(&["video-v1", &c.text, &lines])
}

/// The published video cut from the clip `c` into `out`: with the title card and the corner label
/// of `f` (`card`), else the clip itself.
pub fn video(look: &Look, c: &ClipInfo, f: &Facts, s: &ClipSettings, out: &Path, card: bool) -> Result<()> {
    if card {
        showcase::compose_video(look, &c.mp4, out, f, s.width, s.height, s.fps, TITLE_SECS)
    } else {
        std::fs::copy(&c.mp4, out)?;
        Ok(())
    }
}

/// `door video`: the published video of one design into `out` (its clip in `dir` recorded first
/// when missing or stale, or with `redo`; `relabel`: never record), the clip's first frame as
/// `out.png`.
#[allow(clippy::too_many_arguments)]
pub fn video_one(version: &str, dir: &Path, d: &Design, s: &ClipSettings, out: &Path, card: bool, redo: bool, relabel: bool) -> Result<()> {
    let c = match find(dir, d, s) {
        Some(c) if !redo => {
            eprintln!("{}: clip {} (already recorded)", d.name, c.mp4.display());
            c
        }
        _ if relabel => bail!("{}: no clip with these settings in {} (run door video without --relabel)", d.name, dir.display()),
        _ => {
            let t = Instant::now();
            let c = record(version, dir, d, s)?;
            eprintln!("{}: recorded {} ({:.0}s)", d.name, c.mp4.display(), t.elapsed().as_secs_f64());
            c
        }
    };
    let t = Instant::now();
    let volume = if card {
        let h = crate::leaderboard::design_hash(d);
        Some(crate::properties::volumes(version, vec![d.clone()], 1, volume_cache())?.remove(&h).with_context(|| format!("{}: the title card needs the door's volume", d.name))?)
    } else {
        None
    };
    let f = facts(d, &c, volume.as_ref());
    video(&Look::load(version)?, &c, &f, s, out, card)?;
    let _ = std::fs::copy(c.mp4.with_extension("png"), out.with_extension("png"));
    eprintln!("{}: cut {} from the clip ({:.1}s)", d.name, out.display(), t.elapsed().as_secs_f64());
    Ok(())
}

/// `door video-all`.
pub struct AllOptions {
    /// The clips folder.
    pub clips: PathBuf,
    pub settings: ClipSettings,
    /// Title card and corner label (`--style card`), else the clip itself (`clean`).
    pub card: bool,
    pub jobs: usize,
    pub upload: Option<String>,
    /// Record every clip again.
    pub redo: bool,
    /// Record nothing: cut every video that has a clip again.
    pub relabel: bool,
    pub dry_run: bool,
}

/// `door video-all`: the published video of every design in `dir` (each `.txt`) in
/// `out/<stem>.mp4`, cut from its clip. Clips missing or stale are recorded first (`jobs` at a
/// time); a video is cut again when what it is made of changed (clip, card, label: its stamp),
/// then uploaded to the release `upload` as `<stem>.mp4`. Then writes the catalog `dir/README.md`
/// (see catalog.rs). `dry_run` only lists the work. Fails at the end when a video could not be
/// made.
pub fn record_all(version: &str, dir: &Path, out: &Path, o: &AllOptions) -> Result<()> {
    let t0 = Instant::now();
    std::fs::create_dir_all(out)?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    let mut failed: Vec<String> = Vec::new();
    let mut designs: Vec<(String, Design)> = Vec::new();
    for p in &files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        match Design::load(p) {
            Ok(d) => designs.push((stem, d)),
            Err(e) => {
                eprintln!("FAILED {stem}: {e:#}");
                failed.push(stem);
            }
        }
    }
    // The cards' sizes: the properties cache (designs missing from it are measured and saved; a
    // dry run only reads the cache).
    let vols: HashMap<u64, Volume> = if !o.card {
        HashMap::new()
    } else if o.dry_run {
        let cache = volume_cache().map(crate::properties::load_cache).unwrap_or_default();
        designs.iter().filter_map(|(_, d)| { let h = crate::leaderboard::design_hash(d); Some((h, crate::properties::cached_volume(cache.get(&h)?)?)) }).collect()
    } else {
        crate::properties::volumes(version, designs.iter().map(|(_, d)| d.clone()).collect(), o.jobs.max(1) * 2, volume_cache())?
    };
    // (stem, design, its clip when recorded): the cuts, then the recordings (each cut right after).
    let mut cuts: Vec<(String, Design, Option<ClipInfo>)> = Vec::new();
    let mut recs: Vec<(String, Design, Option<ClipInfo>)> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut fresh = 0;
    for (stem, d) in designs {
        match find(&o.clips, &d, &o.settings).filter(|_| !o.redo) {
            Some(c) => {
                let f = facts(&d, &c, vols.get(&crate::leaderboard::design_hash(&d)));
                if o.relabel || !up_to_date(&out.join(format!("{stem}.mp4")), &video_stamp(&c, &f, o.card)) {
                    cuts.push((stem, d, Some(c)));
                } else {
                    fresh += 1;
                }
            }
            None if o.relabel => missing.push(stem),
            None => recs.push((stem, d, None)),
        }
    }
    eprintln!(
        "{} designs: {fresh} videos up to date, {} to cut from their clips, {} clips to record ({} at a time); clips in {}",
        files.len(),
        cuts.len(),
        recs.len(),
        o.jobs,
        o.clips.display()
    );
    if o.dry_run {
        for (stem, _, _) in &cuts {
            println!("would cut {stem}");
        }
        for (stem, _, _) in &recs {
            println!("would record {stem}");
        }
        for stem in &missing {
            println!("no clip (not relabeled) {stem}");
        }
        return Ok(());
    }
    let look = Look::load(version)?;
    let total = cuts.len() + recs.len();
    // pop() takes from the end: the cuts (seconds each) go first.
    recs.extend(cuts);
    let queue = Mutex::new(recs);
    let failed = Mutex::new(failed);
    let done = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|sc| {
        for _ in 0..o.jobs.max(1) {
            sc.spawn(|| {
                loop {
                    let Some((stem, d, clip)) = queue.lock().unwrap().pop() else { break };
                    let t = Instant::now();
                    let result = (|| -> Result<String> {
                        let recorded = clip.is_none();
                        let c = match clip {
                            Some(c) => c,
                            None => record(version, &o.clips, &d, &o.settings)?,
                        };
                        let rec_secs = t.elapsed().as_secs_f64();
                        let f = facts(&d, &c, vols.get(&crate::leaderboard::design_hash(&d)));
                        if o.card && f.volume.is_none() {
                            bail!("no volume for the title card (door properties could not measure it)");
                        }
                        let mp4 = out.join(format!("{stem}.mp4"));
                        let tmp = out.join(format!("{stem}.part.mp4"));
                        video(&look, &c, &f, &o.settings, &tmp, o.card)?;
                        std::fs::rename(&tmp, &mp4)?;
                        write_stamp(&mp4, &video_stamp(&c, &f, o.card))?;
                        let what = if recorded { format!("recorded ({rec_secs:.0}s) and cut ({:.1}s)", t.elapsed().as_secs_f64() - rec_secs) } else { format!("cut ({:.1}s)", t.elapsed().as_secs_f64()) };
                        let up = match &o.upload {
                            Some(tag) => match mctas::video::upload_release_asset(REPO, tag, &mp4) {
                                Ok(_) => ", uploaded".to_string(),
                                Err(e) => format!(", UPLOAD FAILED: {e:#}"),
                            },
                            None => String::new(),
                        };
                        Ok(format!("{what}{up}"))
                    })();
                    let n = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    match result {
                        Ok(msg) => eprintln!("[{n}/{total}] {stem}: {msg}"),
                        Err(e) => {
                            eprintln!("[{n}/{total}] FAILED {stem}: {e:#}");
                            failed.lock().unwrap().push(stem);
                        }
                    }
                }
            });
        }
    });
    eprintln!("{total} videos made in {:.0}s", t0.elapsed().as_secs_f64());
    // The catalog: every design with its block count, boards, ticks and video link.
    let catalog = crate::catalog::CatalogOptions { archive: dir, workers: o.jobs.max(1) * 2, videos: Some(out), release: Some(o.upload.as_deref().unwrap_or("videos")) };
    crate::catalog::write(version, &catalog)?;
    let failed = failed.into_inner().unwrap();
    if !failed.is_empty() {
        bail!("{} videos could not be made: {failed:?}", failed.len());
    }
    if !missing.is_empty() {
        bail!("{} designs have no clip, their videos were not relabeled (run door video-all without --relabel to record them): {missing:?}", missing.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn design(name: &str, lever_y: i32) -> Design {
        Design::parse(&format!("name: {name}\nport door: 0 0 0\nport lever: 0 {lever_y} 0\npalette:\n  S stone\nlayer 0:\n  S\n")).unwrap()
    }

    /// A clip serves every product of its design (any file name, any label) and only of its
    /// design and recording settings; a product's stamp changes exactly when its label does.
    #[test]
    fn one_clip_per_design_and_settings() {
        let dir = std::env::temp_dir().join(format!("mctas-clips-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, renamed, other) = (design("a", -4), design("a-renamed", -4), design("a", -3));
        let s = ClipSettings::default();
        let hash = crate::leaderboard::design_hash(&a);
        let (mp4, txt) = paths(&dir, hash);
        std::fs::write(&mp4, b"").unwrap();
        std::fs::write(&txt, format!("hash={hash:016x}\nsettings={}\nticks=40\nopen_ticks=30\nclose_ticks=26\nrecorded=1\n", s.key())).unwrap();
        let c = find(&dir, &a, &s).expect("the clip of a");
        assert_eq!((c.ticks, c.open_ticks, c.close_ticks), (40, Some(30), Some(26)));
        assert!(find(&dir, &renamed, &s).is_some(), "the name is not part of the recording");
        assert!(find(&dir, &other, &s).is_none(), "another design");
        assert!(find(&dir, &a, &ClipSettings { orbit: 20.0, ..s }).is_none(), "other settings");
        let vol: Volume = (80, "8x10x1".into());
        let st = |d: &Design, v: &Volume| video_stamp(&c, &facts(d, &c, Some(v)), true);
        assert_eq!(st(&a, &vol), st(&a, &vol));
        assert_ne!(st(&a, &vol), st(&renamed, &vol), "the label shows the name");
        assert_ne!(st(&a, &vol), st(&a, &(81, "8x10x1".into())), "the card shows the volume");
        assert_ne!(video_stamp(&c, &facts(&a, &c, Some(&vol)), true), video_stamp(&c, &facts(&a, &c, Some(&vol)), false));
        let out = dir.join("a.mp4");
        std::fs::write(&out, b"").unwrap();
        assert!(!up_to_date(&out, &st(&a, &vol)));
        write_stamp(&out, &st(&a, &vol)).unwrap();
        assert!(up_to_date(&out, &st(&a, &vol)) && !up_to_date(&out, &st(&renamed, &vol)));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
