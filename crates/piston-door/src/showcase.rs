//! Showcase material for people looking at the results: title cards and labels drawn in the
//! game's own font, `door gallery` (a looping GIF of the best design of every board plus
//! GALLERY.md) and `door montage` (the solid record progression as one labeled video).
//! Everything is rendered by the real client (`door video`) and cut with the static ffmpeg.

use crate::video::{self as dv, Camera, Facts, Shot, VideoOptions};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::mcfont::{self, Font, Image, LINE_HEIGHT};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const YELLOW: [u8; 3] = [255, 255, 85];
const WHITE: [u8; 3] = [255, 255, 255];
const GRAY: [u8; 3] = [170, 170, 170];
const AQUA: [u8; 3] = [85, 255, 255];
const GREEN: [u8; 3] = [85, 255, 85];
const REPO: &str = "vjeux/minecraft-tas";

/// Download URL of a release asset of the repository.
pub fn asset_url(tag: &str, name: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/{tag}/{name}")
}

fn ffmpeg_run(args: &[String]) -> Result<()> {
    let out = Command::new(mctas::video::ffmpeg()?).args(["-loglevel", "error", "-y"]).args(args).output()?;
    if !out.status.success() {
        bail!("ffmpeg {} failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr));
    }
    Ok(())
}

/// Duration of a media file in seconds (ffprobe of the static build).
pub fn duration(path: &Path) -> Result<f64> {
    let probe = mctas::video::ffmpeg()?.with_file_name("ffprobe");
    let out = Command::new(probe)
        .args(["-v", "error", "-show_entries", "format=duration", "-of", "default=nw=1:nk=1"])
        .arg(path)
        .output()?;
    String::from_utf8_lossy(&out.stdout).trim().parse::<f64>().with_context(|| format!("duration of {}", path.display()))
}

/// Font and background texture of the client.
pub struct Look {
    pub font: Font,
    pub dirt: Image,
}

impl Look {
    pub fn load(version: &str) -> Result<Look> {
        Ok(Look { font: Font::minecraft(version)?, dirt: mcfont::texture(version, "block/dirt")? })
    }

    /// A card on the game's darkened dirt background with centered lines (text, scale at 720p,
    /// color), vertically centered as a block.
    pub fn card(&self, w: u32, h: u32, lines: &[(String, u32, [u8; 3])]) -> Image {
        let k = h as f64 / 720.0;
        let sc = |s: u32| ((s as f64 * k).round() as u32).max(1);
        let mut img = Image::new(w, h, [0, 0, 0, 255]);
        img.tile(&self.dirt, sc(5), 0.25);
        let gap = sc(6) as i64;
        let total: i64 = lines.iter().map(|(_, s, _)| (LINE_HEIGHT * sc(*s)) as i64 + gap).sum::<i64>() - gap;
        let mut y = (h as i64 - total) / 2;
        for (text, s, color) in lines {
            if !text.is_empty() {
                self.font.draw_centered(&mut img, w as i64 / 2, y, text, sc(*s), *color, true);
            }
            y += (LINE_HEIGHT * sc(*s)) as i64 + gap;
        }
        img
    }

    /// A transparent full-frame overlay with a box of lines (text, scale at 720p, color) at a
    /// corner: `bottom` false = top-left, true = bottom-left. Lines wider than `max_frac` of the
    /// frame are wrapped.
    pub fn label(&self, w: u32, h: u32, lines: &[(String, u32, [u8; 3])], bottom: bool, max_frac: f64) -> Image {
        let mut img = Image::new(w, h, [0, 0, 0, 0]);
        self.draw_box(&mut img, lines, bottom, max_frac, false);
        img
    }

    /// Draw a translucent box of lines on `img` at its top-left or bottom-left corner, `max_frac`
    /// of the width at most (wrapping), or the full width (`full`).
    pub fn draw_box(&self, img: &mut Image, lines: &[(String, u32, [u8; 3])], bottom: bool, max_frac: f64, full: bool) {
        let (w, h) = (img.w, img.h);
        let k = h as f64 / 720.0;
        let sc = |s: u32| ((s as f64 * k).round() as u32).max(1);
        let pad = sc(10) as i64;
        let margin = sc(20) as i64;
        let max_w = ((w as f64 * max_frac) as i64 - 2 * pad).min(w as i64 - 2 * margin - 2 * pad).max(16) as u32;
        let mut laid: Vec<(String, u32, [u8; 3])> = Vec::new();
        for (text, s, color) in lines {
            for l in self.font.wrap(text, sc(*s), max_w) {
                laid.push((l, sc(*s), *color));
            }
        }
        let box_w = if full { w as i64 - 2 * margin } else { laid.iter().map(|(t, s, _)| self.font.width(t, *s) as i64).max().unwrap_or(0) + 2 * pad };
        let box_h: i64 = laid.iter().map(|(_, s, _)| (LINE_HEIGHT * s) as i64).sum::<i64>() + 2 * pad - sc(1) as i64;
        let x = margin;
        let y = if bottom { h as i64 - margin - box_h } else { margin };
        img.fill_rect(x, y, box_w, box_h, [0, 0, 0, 150]);
        let mut ty = y + pad;
        for (t, s, color) in &laid {
            self.font.draw(img, x + pad, ty, t, *s, *color, true);
            ty += (LINE_HEIGHT * s) as i64;
        }
    }
}

/// Title card of a design video.
pub fn title_lines(f: &Facts) -> Vec<(String, u32, [u8; 3])> {
    let mut lines = vec![
        ("3x3 piston door".to_string(), 4, GRAY),
        (format!("{} blocks", f.blocks), 12, YELLOW),
        (f.kind(), 4, WHITE),
    ];
    if let Some(t) = f.timing() {
        lines.push((t, 3, GRAY));
    }
    lines.push((f.name.clone(), 3, AQUA));
    lines
}

/// Corner label of a design video.
pub fn label_lines(f: &Facts) -> Vec<(String, u32, [u8; 3])> {
    vec![(format!("{} blocks", f.blocks), 4, YELLOW), (format!("{} - {}", f.name, f.kind()), 2, WHITE)]
}

/// Final encode of a recording: title card (fading into the clip) and corner label as asked by
/// `opts`. Falls back to the plain encode when the card cannot be drawn.
pub fn finish_video(version: &str, raw: &Path, out: &Path, facts: &Facts, opts: &VideoOptions) -> Result<()> {
    let dir = raw.parent().unwrap_or(Path::new("."));
    let look = match Look::load(version) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("warning: no title card ({e:#}); plain video");
            return mctas::video::encode(raw, out, 1.0);
        }
    };
    let (w, h) = (opts.width, opts.height);
    let mut args: Vec<String> = Vec::new();
    let title = opts.title_secs > 0.0;
    if title {
        let card = dir.join("card.png");
        look.card(w, h, &title_lines(facts)).save_png(&card)?;
        args.extend(["-loop", "1", "-framerate", &opts.fps.to_string(), "-t", &format!("{:.3}", opts.title_secs), "-i"].map(String::from));
        args.push(card.display().to_string());
    }
    args.push("-i".into());
    args.push(raw.display().to_string());
    let clip_in = if title { 1 } else { 0 };
    let mut graph = format!("[{clip_in}:v]setpts=PTS-STARTPTS,fps={}", opts.fps);
    if opts.label {
        let label = dir.join("label.png");
        look.label(w, h, &label_lines(facts), true, 0.6).save_png(&label)?;
        args.push("-i".into());
        args.push(label.display().to_string());
        graph.push_str(&format!("[clip];[clip][{}:v]overlay=0:0", clip_in + 1));
    }
    graph.push_str(",setsar=1,format=yuv420p");
    if title {
        let fade = 0.35_f64.min(opts.title_secs / 2.0);
        graph.push_str(&format!(",fade=t=in:st=0:d={fade:.2}[v];[0:v]setsar=1,format=yuv420p,fade=t=out:st={:.3}:d={fade:.2}[c];[c][v]concat=n=2:v=1:a=0", opts.title_secs - fade));
    }
    graph.push_str("[o]");
    args.extend(["-filter_complex".to_string(), graph, "-map".into(), "[o]".into()]);
    args.extend(["-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"].map(String::from));
    args.push(out.display().to_string());
    ffmpeg_run(&args)
}

/// A looping GIF of a clip: `speed` times faster, `width` px wide, `fps` frames per second,
/// palette built for the clip, with an optional overlay image of the GIF's size.
pub fn make_gif(clip: &Path, overlay: Option<&Path>, out: &Path, width: u32, fps: u32, speed: f64) -> Result<()> {
    let mut args: Vec<String> = vec!["-i".into(), clip.display().to_string()];
    let mut graph = format!("[0:v]setpts=(PTS-STARTPTS)/{speed},fps={fps},scale={width}:-2:flags=lanczos");
    if let Some(o) = overlay {
        args.extend(["-i".to_string(), o.display().to_string()]);
        graph.push_str("[s];[s][1:v]overlay=0:0");
    }
    graph.push_str(",split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle[o]");
    args.extend(["-filter_complex".to_string(), graph, "-map".into(), "[o]".into(), "-loop".into(), "0".into(), out.display().to_string()]);
    ffmpeg_run(&args)
}

// ---- boards ---------------------------------------------------------------------------------

/// One archived design with its verdict.
#[derive(Clone)]
pub struct Archived {
    pub stem: String,
    pub path: PathBuf,
    pub design: Design,
    pub facts: Facts,
}

/// Every working design of the archive folder (verdicts and worst-cycle timings from the
/// leaderboard's cache, new designs verified on the spot), files marked invalid excluded.
pub fn archived(version: &str, dir: &Path, workers: usize) -> Result<Vec<Archived>> {
    let mut cands = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    for p in files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        if stem.contains("invalid") {
            continue;
        }
        let Ok(d) = Design::load(&p) else { continue };
        if !d.ports.contains_key("door") || !d.ports.contains_key("lever") {
            continue;
        }
        cands.push((p.display().to_string(), d));
    }
    let entries = crate::leaderboard::judge(version, cands, 28, workers, Some(dir))?;
    Ok(entries
        .into_iter()
        .filter(|e| e.verdict.is_ok())
        .map(|e| {
            let path = PathBuf::from(&e.source);
            let mut facts = Facts::of(&e.design);
            facts.open_ticks = e.open_ticks;
            facts.close_ticks = e.close_ticks;
            facts.rotation_safe = e.rotation_safe();
            facts.ground_safe = e.ground_safe();
            facts.shape = e.swept.or(facts.shape);
            Archived { stem: path.file_stem().unwrap().to_string_lossy().to_string(), path, design: e.design, facts }
        })
        .collect())
}

/// A board and its current best.
pub struct Board {
    pub title: &'static str,
    pub rule: &'static str,
    /// Name of its record progression (`door montage --board <key>`), if it has one.
    pub key: Option<&'static str>,
    pub best: Archived,
}

fn total_ticks(a: &Archived) -> u32 {
    a.facts.open_ticks.unwrap_or(999) + a.facts.close_ticks.unwrap_or(999)
}

/// The block-count boards: key, title, rule, membership.
pub const BLOCK_BOARDS: &[(&str, &str, &str, fn(&Facts) -> bool)] = &[
    ("solid", "Solid (main ranking)", "9 identical plain door blocks, no sculk sensors, rotation-safe", |f| f.category == "solid" && f.rotation_safe),
    ("slime-free", "Solid, no slime or honey", "not a single slime or honey block anywhere", |f| f.category == "solid" && f.slime_free && f.rotation_safe),
    (
        "flat",
        "Solid, flat (one z layer), no slime or honey",
        "slime-free, and every block in the door's plane: the whole machine is one block thick",
        |f| f.category == "solid" && f.slime_free && f.flat && f.rotation_safe,
    ),
    (
        "wall",
        "Solid, in a 3-thick wall",
        "every block within one layer of the door's plane, at rest and while it moves: the whole machine hides inside a wall 3 blocks thick",
        |f| f.category == "solid" && f.rotation_safe && f.shape.is_some_and(|s| s.front <= 1 && s.behind <= 1),
    ),
    (
        "ground",
        "Solid, on the ground",
        "nothing below the door, at rest or while it moves: it stands on flat ground, no digging (and works on a stone floor)",
        |f| f.category == "solid" && f.rotation_safe && f.ground_safe && f.shape.is_some_and(|s| s.below == 0),
    ),
    ("sculk", "Sculk", "uses sculk sensors (footsteps or mobs nearby can trigger them)", |f| f.category == "sculk" && f.rotation_safe),
    (
        "sculk-unrotated",
        "Sculk, not rotation-safe",
        "uses sculk sensors; works as built but breaks when the whole door is turned 90 degrees",
        |f| f.category == "sculk" && !f.rotation_safe,
    ),
    ("gravity", "Gravity", "the 9 door blocks are sand, gravel or concrete powder", |f| f.category == "gravity" && f.rotation_safe),
];

/// The best design of each board: solid, slime-free, sculk, gravity (fewest blocks; ties go to
/// a design of the record progression, then the faster one, then by name) and fastest solid
/// (fewest open + close ticks, ties by blocks).
pub fn boards(all: &[Archived], records: &HashMap<String, String>) -> Vec<Board> {
    let mut out = Vec::new();
    for (key, title, rule, keep) in BLOCK_BOARDS {
        let best = all
            .iter()
            .filter(|a| keep(&a.facts))
            .min_by_key(|a| (a.facts.blocks, !records.contains_key(&a.stem), total_ticks(a), a.stem.clone()))
            .cloned();
        if let Some(best) = best {
            out.push(Board { title, rule, key: Some(key), best });
        }
    }
    let fastest = all
        .iter()
        .filter(|a| a.facts.category == "solid" && a.facts.rotation_safe && a.facts.open_ticks.is_some())
        .min_by_key(|a| (total_ticks(a), a.facts.blocks, a.stem.clone()))
        .cloned();
    if let Some(best) = fastest {
        out.push(Board { title: "Fastest solid", rule: "fewest game ticks to open plus close (worst cycle), ties by blocks", key: None, best });
    }
    let smallest = all
        .iter()
        .filter(|a| a.facts.category == "solid" && a.facts.rotation_safe && a.facts.shape.is_some())
        .min_by_key(|a| (a.facts.shape.map(|s| s.volume()), a.facts.blocks, a.stem.clone()))
        .cloned();
    if let Some(best) = smallest {
        out.push(Board { title: "Smallest box (solid)", rule: "fewest cells in the build's bounding box (door included), ties by blocks", key: None, best });
    }
    out
}

/// "YYYY-MM-DD HH:MMZ" from an archive file's "# archived ..." header line ("" if none).
fn archived_at(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else { return String::new() };
    text.lines()
        .take(4)
        .find_map(|l| l.strip_prefix("# archived ").map(|r| r.split(" from ").next().unwrap_or("").to_string()))
        .unwrap_or_default()
}

/// The records of a board in archive order: every design that has fewer blocks than all the
/// designs of the board archived before it (designs archived together: largest first).
pub fn timeline_progression(all: &[Archived], keep: fn(&Facts) -> bool, caps: &HashMap<String, String>) -> Vec<(usize, String, String)> {
    let mut list: Vec<(String, usize, &Archived)> = all.iter().filter(|a| keep(&a.facts)).map(|a| (archived_at(&a.path), a.facts.blocks, a)).collect();
    list.sort_by(|a, b| (&a.0, std::cmp::Reverse(a.1), &a.2.stem).cmp(&(&b.0, std::cmp::Reverse(b.1), &b.2.stem)));
    let mut rows = Vec::new();
    let mut best = usize::MAX;
    for (_, blocks, a) in list {
        if blocks < best {
            best = blocks;
            let how = caps.get(&a.stem).cloned().or_else(|| track_of(&a.path).map(|t| format!("agent {t}"))).unwrap_or_default();
            rows.push((blocks, a.stem.clone(), how));
        }
    }
    rows
}

// ---- captions -------------------------------------------------------------------------------

/// Rows of the markdown tables of a file whose first two columns are `<file or design>` and
/// whose `how_col` column describes it: stem -> description. A cell may name several files
/// separated by `/`.
fn table_captions(path: &Path, file_col: usize, how_col: usize) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let Ok(text) = std::fs::read_to_string(path) else { return m };
    for line in text.lines() {
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(|c| c.trim()).collect();
        if cells.len() <= file_col.max(how_col) || cells[file_col].starts_with("---") {
            continue;
        }
        for f in cells[file_col].split('/') {
            let f = f.trim().trim_matches('`');
            let stem = f.strip_suffix(".txt").unwrap_or(f);
            if !stem.is_empty() && !cells[how_col].is_empty() {
                m.entry(stem.to_string()).or_insert_with(|| cells[how_col].to_string());
            }
        }
    }
    m
}

/// Captions: CAPTIONS.md next to the archive (`| design | how it works |`), then the record
/// progression table of the archive README (`| blocks | file | how |`).
pub fn captions(archive: &Path) -> HashMap<String, String> {
    let mut m = table_captions(&archive.parent().unwrap_or(Path::new(".")).join("CAPTIONS.md"), 0, 1);
    for (k, v) in table_captions(&archive.join("README.md"), 1, 2) {
        m.entry(k).or_insert(v);
    }
    m
}

/// The record progression of the archive README: (blocks, stem, how), largest first, one per
/// block count (the first file of a row).
pub fn progression(archive: &Path) -> Vec<(usize, String, String)> {
    let Ok(text) = std::fs::read_to_string(archive.join("README.md")) else { return Vec::new() };
    let mut rows: Vec<(usize, String, String)> = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        if line.starts_with("## ") {
            in_table = line.to_lowercase().contains("record progression");
            continue;
        }
        if !in_table {
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(|c| c.trim()).collect();
        if cells.len() < 3 {
            continue;
        }
        let Ok(blocks) = cells[0].parse::<usize>() else { continue };
        let file = cells[1].split('/').next().unwrap_or("").trim().trim_matches('`');
        let stem = file.strip_suffix(".txt").unwrap_or(file).to_string();
        if archive.join(format!("{stem}.txt")).exists() && !rows.iter().any(|r| r.0 == blocks) {
            rows.push((blocks, stem, cells[2].to_string()));
        }
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows
}

/// Which agent track a design came from (the archive header names its branch).
fn track_of(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.lines().take(3).find(|l| l.contains("explore/"))?;
    let rest = &line[line.find("explore/")? + 8..];
    Some(rest.split([':', ' ', '/']).next()?.to_string())
}

fn first_sentence_cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Capitalized, ending with a period.
fn sentence(s: &str) -> String {
    let s = first_sentence_cap(s.trim());
    if s.is_empty() || s.ends_with(['.', '!', '?']) { s } else { format!("{s}.") }
}

// ---- gallery --------------------------------------------------------------------------------

pub struct GalleryOptions {
    pub archive: PathBuf,
    /// Clean door-camera clips and GIFs (rendered when missing).
    pub videos: PathBuf,
    pub out: PathBuf,
    pub upload: Option<String>,
    pub jobs: usize,
    pub workers: usize,
    pub orbit: f64,
    pub gif_width: u32,
    pub gif_fps: u32,
    pub speed: f64,
    /// Re-render clips and GIFs that exist.
    pub redo: bool,
}

/// Render `todo` (stem, design, options, output) with the real client, `jobs` at a time.
fn render_all(version: &str, todo: Vec<(String, Design, VideoOptions, PathBuf)>, jobs: usize) -> Vec<String> {
    let queue = std::sync::Mutex::new(todo);
    let failed = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| loop {
                let Some((stem, d, opts, mp4)) = queue.lock().unwrap().pop() else { break };
                let tmp = mp4.with_extension("part.mp4");
                let t = std::time::Instant::now();
                match dv::record(version, &d, &tmp, &opts) {
                    Ok(()) => {
                        let _ = std::fs::rename(&tmp, &mp4);
                        let _ = std::fs::rename(tmp.with_extension("png"), mp4.with_extension("png"));
                        eprintln!("rendered {stem} ({:.0}s)", t.elapsed().as_secs_f64());
                    }
                    Err(e) => {
                        eprintln!("FAILED {stem}: {e:#}");
                        failed.lock().unwrap().push(stem);
                    }
                }
            });
        }
    });
    failed.into_inner().unwrap()
}

pub fn gallery(version: &str, o: &GalleryOptions) -> Result<()> {
    std::fs::create_dir_all(&o.videos)?;
    let all = archived(version, &o.archive, o.workers)?;
    let caps = captions(&o.archive);
    let records: HashMap<String, String> = progression(&o.archive).into_iter().map(|(_, s, h)| (s, h)).collect();
    let boards = boards(&all, &records);
    eprintln!("{} working designs; boards: {}", all.len(), boards.iter().map(|b| format!("{} = {}", b.title, b.best.stem)).collect::<Vec<_>>().join(", "));
    // Clean clips of the bests, 4:3 (door builds are taller than wide); a still camera by
    // default: every pixel of an orbiting shot changes each frame, which makes GIFs 10x larger.
    let clip_of = |stem: &str| o.videos.join(format!("{stem}.o{}.mp4", o.orbit));
    let mut todo = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for b in &boards {
        let mp4 = clip_of(&b.best.stem);
        if seen.insert(b.best.stem.clone()) && (o.redo || !mp4.exists()) {
            todo.push((b.best.stem.clone(), b.best.design.clone(), VideoOptions { width: 960, height: 720, ..VideoOptions::clean(o.orbit) }, mp4));
        }
    }
    eprintln!("{} clips to render ({} at a time)", todo.len(), o.jobs);
    let failed = render_all(version, todo, o.jobs);
    if !failed.is_empty() {
        bail!("rendering failed for {failed:?}");
    }
    // GIFs with a small label, uploaded.
    let look = Look::load(version)?;
    let gif_h = o.gif_width * 3 / 4;
    for stem in &seen {
        let b = boards.iter().find(|b| &b.best.stem == stem).unwrap();
        let gif = o.videos.join(format!("{stem}.gif"));
        if o.redo || !gif.exists() {
            let label = o.videos.join(format!("{stem}.gif-label.png"));
            let f = &b.best.facts;
            look.label(o.gif_width, gif_h, &[(format!("{} blocks", f.blocks), 5, YELLOW), (f.kind(), 3, WHITE)], true, 0.9).save_png(&label)?;
            make_gif(&clip_of(stem), Some(&label), &gif, o.gif_width, o.gif_fps, o.speed)?;
            eprintln!("gif {} ({} KB)", gif.display(), std::fs::metadata(&gif)?.len() / 1024);
        }
        if let Some(tag) = &o.upload {
            println!("uploaded {}", mctas::video::upload_release_asset(REPO, tag, &gif)?);
        }
    }
    let tag = o.upload.clone().unwrap_or_else(|| "videos".into());
    let timeline = o.videos.join(TIMELINE_ASSET);
    timeline_chart(&look, &all, &timeline)?;
    if o.upload.is_some() {
        println!("uploaded {}", mctas::video::upload_release_asset(REPO, &tag, &timeline)?);
    }
    let assets = release_assets(&tag);
    // The page.
    let rel = |p: &Path| -> String {
        let base = o.out.parent().unwrap_or(Path::new("."));
        p.strip_prefix(base).map(|r| r.display().to_string()).unwrap_or_else(|_| p.display().to_string())
    };
    let mut md = String::from("# Gallery: the best 3x3 piston door of each board\n\n");
    md.push_str(&format!(
        "Looping GIFs rendered by the real Minecraft client (`mctas door gallery`, 1.5x the recording's 10 ticks/s). \
         Rankings: [LEADERBOARD.md](LEADERBOARD.md). Every archived design has its own video: [archive/VIDEOS.md](archive/VIDEOS.md).\n\n\
         The whole solid record progression in one video ({}, `mctas door montage`; click the poster):\n\n[![every solid record]({})]({})\n\n\
         The night, board by board (records from the archive):\n\n![records over the night]({})\n\n",
        progression_title(&o.archive, &all),
        asset_url(&tag, POSTER_ASSET),
        asset_url(&tag, MONTAGE_ASSET),
        asset_url(&tag, TIMELINE_ASSET)
    ));
    for b in &boards {
        let a = &b.best;
        let f = &a.facts;
        md.push_str(&format!("## {}: {} blocks\n\n{}.\n\n", b.title, f.blocks, first_sentence_cap(b.rule)));
        md.push_str(&format!("![{}]({})\n\n", a.stem, asset_url(&tag, &format!("{}.gif", a.stem))));
        let timing = match (f.open_ticks, f.close_ticks) {
            (Some(op), Some(cl)) => format!("opens in {op} ticks, closes in {cl} (worst cycle)"),
            _ => "timing unknown".into(),
        };
        let how = caps
            .get(&a.stem)
            .cloned()
            .or_else(|| track_of(&a.path).map(|t| format!("from agent {t}; see [tracks/{t}/NOTES.md](tracks/{t}/NOTES.md)")))
            .unwrap_or_default();
        md.push_str(&format!(
            "**{} blocks**, {}, {}: [{}]({}) ([video]({})). {}\n\n",
            f.blocks,
            f.kind(),
            timing,
            a.stem,
            rel(&a.path),
            asset_url(&tag, &format!("{}.mp4", a.stem)),
            sentence(&how)
        ));
        if let Some(k) = b.key.filter(|k| *k != "solid" && assets.contains(&format!("{k}-progression.mp4"))) {
            md.push_str(&format!(
                "Every record of this board in one video: [{k}-progression.mp4]({}) ([poster]({})).\n\n",
                asset_url(&tag, &format!("{k}-progression.mp4")),
                asset_url(&tag, &format!("{k}-progression.jpg"))
            ));
        }
    }
    std::fs::write(&o.out, md)?;
    eprintln!("wrote {}", o.out.display());
    Ok(())
}

/// Names of the assets of a release of the repository (empty when `gh` cannot tell).
fn release_assets(tag: &str) -> std::collections::HashSet<String> {
    Command::new("gh")
        .args(["release", "view", tag, "--repo", REPO, "--json", "assets", "--jq", ".assets[].name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).collect())
        .unwrap_or_default()
}

// ---- night timeline -------------------------------------------------------------------------

/// Minutes since 1970 of "YYYY-MM-DD HH:MMZ".
fn minutes_of(ts: &str) -> Option<i64> {
    let (date, time) = ts.trim().trim_end_matches('Z').split_once(' ')?;
    let d: Vec<i64> = date.split('-').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    let t: Vec<i64> = time.split(':').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    if d.len() != 3 || t.len() < 2 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let (y, m, day) = (if d[1] <= 2 { d[0] - 1 } else { d[0] }, d[1], d[2]);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some((era * 146097 + doe - 719468) * 1440 + t[0] * 60 + t[1])
}

/// A chart of the night: the record of each block-count board over time (from the archive
/// headers), as step lines in the boards' colors, in the game's font.
pub fn timeline_chart(look: &Look, all: &[Archived], out: &Path) -> Result<()> {
    let colors: [[u8; 3]; 9] = [YELLOW, GREEN, [255, 255, 255], [255, 85, 255], [170, 85, 0], [85, 85, 255], AQUA, [85, 170, 170], [255, 170, 0]];
    let mut series: Vec<(&str, [u8; 3], Vec<(i64, usize)>)> = Vec::new();
    for ((key, _, _, keep), color) in BLOCK_BOARDS.iter().zip(colors.iter().copied().cycle()) {
        let mut pts: Vec<(i64, usize)> = all.iter().filter(|a| keep(&a.facts)).filter_map(|a| Some((minutes_of(&archived_at(&a.path))?, a.facts.blocks))).collect();
        pts.sort();
        let mut best = usize::MAX;
        let steps: Vec<(i64, usize)> = pts.into_iter().filter(|&(_, b)| if b < best { best = b; true } else { false }).collect();
        if !steps.is_empty() {
            series.push((key, color, steps));
        }
    }
    let t0 = series.iter().flat_map(|s| s.2.iter().map(|p| p.0)).min().context("no timestamps")?;
    let t1 = series.iter().flat_map(|s| s.2.iter().map(|p| p.0)).max().unwrap().max(t0 + 60);
    let bmax = series.iter().flat_map(|s| s.2.iter().map(|p| p.1)).max().unwrap();
    let bmin = series.iter().flat_map(|s| s.2.iter().map(|p| p.1)).min().unwrap();
    let (lo, hi) = ((bmin / 10 * 10).saturating_sub(5).max(1), bmax.div_ceil(10) * 10);
    let (w, h) = (1280u32, 640u32);
    let mut img = Image::new(w, h, [0, 0, 0, 255]);
    img.tile(&look.dirt, 4, 0.25);
    let (px0, px1, py0, py1) = (110i64, w as i64 - 190, 90i64, h as i64 - 70);
    let x_of = |t: i64| px0 + (t - t0) * (px1 - px0) / (t1 - t0);
    // Log scale: the last few blocks matter as much as the first hundred.
    let (llo, lhi) = ((lo as f64).ln(), (hi as f64).ln());
    let y_of = |b: usize| py1 - (((b as f64).ln() - llo) / (lhi - llo) * (py1 - py0) as f64).round() as i64;
    look.font.draw_centered(&mut img, w as i64 / 2, 16, "One night of agents: the record of each board (blocks, log scale; UTC)", 3, YELLOW, true);
    for b in [20, 30, 40, 50, 60, 70, 80, 100, 120, 150, 200, 250, 300] {
        if b < lo || b > hi {
            continue;
        }
        let y = y_of(b);
        img.fill_rect(px0, y, px1 - px0, 1, [255, 255, 255, 40]);
        let label = b.to_string();
        look.font.draw(&mut img, px0 - 14 - look.font.width(&label, 2) as i64, y - 8, &label, 2, GRAY, true);
    }
    let mut hour = (t0 / 60 + 1) * 60;
    while hour <= t1 {
        let x = x_of(hour);
        img.fill_rect(x, py0, 1, py1 - py0, [255, 255, 255, 40]);
        let label = format!("{:02}:00", (hour / 60).rem_euclid(24));
        look.font.draw_centered(&mut img, x, py1 + 12, &label, 2, GRAY, true);
        hour += 60;
    }
    img.fill_rect(px0, py1, px1 - px0, 2, [200, 200, 200, 255]);
    img.fill_rect(px0 - 2, py0, 2, py1 - py0 + 2, [200, 200, 200, 255]);
    // Step lines, then the final value of each board at the right end (labels kept apart).
    let mut labels: Vec<(i64, String, [u8; 3])> = Vec::new();
    for (key, color, steps) in &series {
        let c = [color[0], color[1], color[2], 255];
        for (i, &(t, b)) in steps.iter().enumerate() {
            let (x, y) = (x_of(t), y_of(b));
            let x_next = steps.get(i + 1).map(|p| x_of(p.0)).unwrap_or(px1);
            img.fill_rect(x, y - 1, (x_next - x).max(1), 4, c);
            if let Some(&(_, nb)) = steps.get(i + 1) {
                let yn = y_of(nb);
                img.fill_rect(x_next - 1, y.min(yn), 4, (y - yn).abs() + 3, c);
            }
            img.fill_rect(x - 3, y - 4, 8, 10, c);
        }
        let last = steps.last().unwrap();
        labels.push((y_of(last.1) - 8, format!("{} {key}", last.1), *color));
    }
    labels.sort_by_key(|l| l.0);
    for i in 1..labels.len() {
        labels[i].0 = labels[i].0.max(labels[i - 1].0 + 20);
    }
    for (y, text, color) in &labels {
        look.font.draw(&mut img, px1 + 12, *y, text, 2, *color, true);
    }
    img.save(out)
}

// ---- montage --------------------------------------------------------------------------------

pub const MONTAGE_ASSET: &str = "solid-progression.mp4";
pub const POSTER_ASSET: &str = "solid-progression.jpg";
pub const TIMELINE_ASSET: &str = "night-timeline.png";

/// A poster of a progression: one frame of each clip (`at` seconds in, the door open) in a
/// grid, labeled with its block count, under a title.
fn poster(look: &Look, clips: &Path, rows: &[(usize, String, String)], out: &Path, at: f64, board_title: &str) -> Result<()> {
    let n = rows.len().max(1);
    let cols = ((n as f64).sqrt().ceil() as usize).max(1);
    let grid_rows = n.div_ceil(cols);
    let (tw, th) = (384u32, 216u32);
    let header = 84u32;
    let w = tw * cols as u32;
    let mut img = Image::new(w, header + th * grid_rows as u32, [0, 0, 0, 255]);
    img.tile(&look.dirt, 4, 0.25);
    let (first, last) = (rows.first().map(|r| r.0).unwrap_or(0), rows.last().map(|r| r.0).unwrap_or(0));
    let title = format!("The smallest 3x3 piston door: {first} -> {last} blocks");
    let scale = if look.font.width(&title, 4) + 40 <= w { 4 } else { 3 };
    look.font.draw_centered(&mut img, w as i64 / 2, 12, &title, scale, YELLOW, true);
    let sub = format!("{}: every record of one night of agents, same camera, door open ({} records)", board_title.to_lowercase(), rows.len());
    look.font.draw_centered(&mut img, w as i64 / 2, 54, &sub, 2, GRAY, true);
    for (i, (blocks, stem, _)) in rows.iter().enumerate() {
        let frame = clips.join(format!("{stem}.poster.png"));
        ffmpeg_run(&[
            "-ss".into(),
            format!("{at:.2}"),
            "-i".into(),
            clips.join(format!("{stem}.mp4")).display().to_string(),
            "-frames:v".into(),
            "1".into(),
            "-vf".into(),
            format!("crop=iw*0.86:ih*0.86,scale={tw}:{th}:flags=lanczos"),
            frame.display().to_string(),
        ])?;
        let f = Image::load_png(&frame)?;
        let (x, y) = ((i % cols) as i64 * tw as i64, header as i64 + (i / cols) as i64 * th as i64);
        img.draw(&f, x, y, 1);
        let text = format!("{blocks}");
        let s = 3;
        img.fill_rect(x + 6, y + 6, look.font.width(&text, s) as i64 + 12, (LINE_HEIGHT * s) as i64 + 6, [0, 0, 0, 150]);
        look.font.draw(&mut img, x + 12, y + 10, &text, s, if i + 1 == rows.len() { YELLOW } else { WHITE }, true);
    }
    img.save(out)
}

fn progression_title(archive: &Path, all: &[Archived]) -> String {
    let rows = progression(archive);
    let first = rows.first().map(|r| r.0);
    let best = all.iter().filter(|a| a.facts.category == "solid").map(|a| a.facts.blocks).min();
    match (first, best) {
        (Some(a), Some(b)) => format!("{a} -> {b} blocks"),
        _ => "solid record progression".into(),
    }
}

pub struct MontageOptions {
    pub archive: PathBuf,
    /// Clips (one folder per camera), segments and the final video.
    pub videos: PathBuf,
    pub out: PathBuf,
    pub upload: Option<String>,
    pub jobs: usize,
    pub workers: usize,
    /// Playback speed of the clips (2 = real game speed: clips are recorded at 10 ticks/s).
    pub speed: f64,
    pub redo: bool,
    /// Which board: solid (the archive README's curated progression), slime-free, sculk or
    /// gravity (records in archive order).
    pub board: String,
}

/// One camera for every clip: the door shot of all designs together (each moved so its door
/// is at the origin), so the machine visibly shrinks around a door that stays put.
fn montage_shot(designs: &[&Design], aspect: f64) -> Result<(Shot, i32)> {
    let mut blocks = Vec::new();
    let mut floor = i32::MAX;
    for d in designs {
        let door = crate::DoorSpec::from_design(d)?.door;
        let (lo, _) = d.bounds().context("empty design")?;
        floor = floor.min(lo[1] - door[1] - 2);
        blocks.extend(d.blocks.keys().map(|p| [p[0] - door[0], p[1] - door[1], p[2] - door[2]]));
    }
    blocks.sort();
    blocks.dedup();
    Ok((dv::door_shot(&blocks, [0, 0, 0], 0.0, aspect, 0.85), floor))
}

pub fn montage(version: &str, o: &MontageOptions) -> Result<()> {
    let all = archived(version, &o.archive, o.workers)?;
    let caps = captions(&o.archive);
    let Some(&(board, board_title, board_rule, keep)) = BLOCK_BOARDS.iter().find(|b| b.0 == o.board) else {
        bail!("unknown --board {} (solid, slime-free, flat, wall, ground, sculk, sculk-unrotated, gravity)", o.board);
    };
    let mut rows = if board == "solid" { progression(&o.archive) } else { timeline_progression(&all, keep, &caps) };
    // A newer record than the last row joins at the end.
    let records: HashMap<String, String> = rows.iter().map(|(_, s, h)| (s.clone(), h.clone())).collect();
    if let Some(best) = boards(&all, &records).into_iter().find(|b| b.key == Some(board)) {
        if rows.last().is_none_or(|r| best.best.facts.blocks < r.0) {
            let how = caps.get(&best.best.stem).cloned().unwrap_or_else(|| "the current record".into());
            rows.push((best.best.facts.blocks, best.best.stem.clone(), how));
        }
    }
    if rows.is_empty() {
        bail!("no record progression found for board {board}");
    }
    let asset = format!("{board}-progression.mp4");
    let designs: Vec<Design> = rows.iter().map(|(_, s, _)| Design::load(&o.archive.join(format!("{s}.txt")))).collect::<Result<_>>()?;
    let base = VideoOptions::clean(0.0);
    let aspect = base.width as f64 / base.height as f64;
    let (shot, floor) = montage_shot(&designs.iter().collect::<Vec<_>>(), aspect)?;
    // Clips are reused while the camera (and the render look, "v3": wide floor, view distance)
    // stay the same.
    let key = format!("v3,{:.2},{:.2},{:.2},{:.3},{:.3},{:.3},{:.2},{floor}", shot.target[0], shot.target[1], shot.target[2], shot.dir[0], shot.dir[1], shot.dir[2], shot.dist);
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    let clips = o.videos.join(format!("clips-{:08x}", hash as u32));
    std::fs::create_dir_all(&clips)?;
    eprintln!("{} records ({} -> {}), camera {shot:?}, floor {floor}, clips in {}", rows.len(), rows[0].0, rows.last().unwrap().0, clips.display());
    let mut todo = Vec::new();
    for ((_, stem, _), d) in rows.iter().zip(&designs) {
        let mp4 = clips.join(format!("{stem}.mp4"));
        if o.redo || !mp4.exists() {
            let door = crate::DoorSpec::from_design(d)?.door;
            let opts = VideoOptions { camera: Camera::Shot(shot.translated(door)), floor_y: Some(floor + door[1]), ..VideoOptions::clean(0.0) };
            todo.push((stem.clone(), d.clone(), opts, mp4));
        }
    }
    eprintln!("{} clips to render ({} at a time)", todo.len(), o.jobs);
    let failed = render_all(version, todo, o.jobs);
    if !failed.is_empty() {
        bail!("rendering failed for {failed:?}");
    }
    // Segments: intro card, each clip sped up with its label, outro card; same encoding.
    let look = Look::load(version)?;
    let poster_path = o.out.with_file_name(asset.replace(".mp4", ".jpg"));
    poster(&look, &clips, &rows, &poster_path, 6.0, board_title)?;
    eprintln!("poster {} ({} KB)", poster_path.display(), std::fs::metadata(&poster_path)?.len() / 1024);
    let (w, h, fps) = (base.width, base.height, base.fps);
    let seg_dir = o.videos.join(format!("segments-{board}"));
    std::fs::create_dir_all(&seg_dir)?;
    let enc = |args: &mut Vec<String>, out: &Path| {
        args.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p", "-r", &fps.to_string(), "-video_track_timescale", "15360"].map(String::from));
        args.push(out.display().to_string());
    };
    let still = |img: &Image, secs: f64, name: &str| -> Result<PathBuf> {
        let png = seg_dir.join(format!("{name}.png"));
        img.save_png(&png)?;
        let out = seg_dir.join(format!("{name}.mp4"));
        let mut args: Vec<String> = ["-loop", "1", "-framerate", &fps.to_string(), "-t", &format!("{secs:.2}"), "-i"].map(String::from).to_vec();
        args.push(png.display().to_string());
        args.extend(["-vf".to_string(), format!("setsar=1,format=yuv420p,fade=t=in:st=0:d=0.4,fade=t=out:st={:.2}:d=0.4", secs - 0.4)]);
        enc(&mut args, &out);
        ffmpeg_run(&args)?;
        Ok(out)
    };
    let (first, last) = (rows[0].0, rows.last().unwrap().0);
    let mut segments = vec![still(
        &look.card(w, h, &[
            ("The smallest 3x3 piston door".into(), 5, YELLOW),
            (format!("board: {}", board_title.to_lowercase()), 3, AQUA),
            ("".into(), 2, WHITE),
            (format!("{first} -> {last} blocks"), 10, WHITE),
            ("".into(), 2, WHITE),
            (format!("{} records in one night of agents", rows.len()), 3, GRAY),
            (sentence(board_rule), 2, GRAY),
            ("same camera for every door, played at game speed".into(), 2, GRAY),
        ]),
        4.0,
        "intro",
    )?];
    for (i, (blocks, stem, how)) in rows.iter().enumerate() {
        let clip = clips.join(format!("{stem}.mp4"));
        let label = seg_dir.join(format!("{i:02}-{stem}.label.png"));
        let delta = if i == 0 { "first on this board".to_string() } else { format!("-{} blocks", rows[i - 1].0 - blocks) };
        let mut overlay = Image::new(w, h, [0, 0, 0, 0]);
        look.draw_box(&mut overlay, &[(format!("{blocks} blocks"), 6, YELLOW), (format!("{delta}  ({}/{})", i + 1, rows.len()), 2, GREEN)], false, 0.5, false);
        look.draw_box(&mut overlay, &[(sentence(how), 2, WHITE), (stem.clone(), 2, GRAY)], true, 1.0, true);
        overlay.save_png(&label)?;
        let secs = duration(&clip)? / o.speed;
        let out = seg_dir.join(format!("{i:02}-{stem}.mp4"));
        let mut args = vec!["-i".to_string(), clip.display().to_string(), "-i".into(), label.display().to_string()];
        args.extend([
            "-filter_complex".to_string(),
            format!(
                "[0:v]setpts=(PTS-STARTPTS)/{},fps={fps}[s];[s][1:v]overlay=0:0,setsar=1,format=yuv420p,fade=t=in:st=0:d=0.25,fade=t=out:st={:.2}:d=0.25[o]",
                o.speed,
                (secs - 0.3).max(0.0)
            ),
            "-map".into(),
            "[o]".into(),
        ]);
        enc(&mut args, &out);
        ffmpeg_run(&args)?;
        segments.push(out);
    }
    let best_stem = &rows.last().unwrap().1;
    let best_how = &rows.last().unwrap().2;
    let mut outro: Vec<(String, u32, [u8; 3])> = vec![("Current record".into(), 4, GRAY), (format!("{last} blocks"), 12, YELLOW), (best_stem.clone(), 3, AQUA), ("".into(), 2, WHITE)];
    for l in look.font.wrap(&sentence(best_how), 2, w * 4 / 5) {
        outro.push((l, 2, WHITE));
    }
    outro.push(("".into(), 2, WHITE));
    outro.push(("github.com/vjeux/minecraft-tas".into(), 2, GRAY));
    segments.push(still(&look.card(w, h, &outro), 4.0, "outro")?);
    let list = seg_dir.join("list.txt");
    std::fs::write(&list, segments.iter().map(|p| format!("file '{}'\n", p.display())).collect::<String>())?;
    let mut args: Vec<String> = ["-f", "concat", "-safe", "0", "-i"].map(String::from).to_vec();
    args.push(list.display().to_string());
    args.extend(["-c:v", "libx264", "-preset", "slow", "-crf", "21", "-pix_fmt", "yuv420p", "-movflags", "+faststart"].map(String::from));
    args.push(o.out.display().to_string());
    ffmpeg_run(&args)?;
    eprintln!("montage {} ({:.0}s, {} KB)", o.out.display(), duration(&o.out)?, std::fs::metadata(&o.out)?.len() / 1024);
    if let Some(tag) = &o.upload {
        let named = o.out.with_file_name(&asset);
        if named != o.out {
            std::fs::copy(&o.out, &named)?;
        }
        println!("uploaded {}", mctas::video::upload_release_asset(REPO, tag, &named)?);
        println!("uploaded {}", mctas::video::upload_release_asset(REPO, tag, &poster_path)?);
    }
    Ok(())
}
