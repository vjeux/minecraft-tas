//! `archive/README.md`, the generated catalog of every archived design: size (the volume),
//! blocks placed, boards, open and close ticks, rotations, buildability (the first line of each
//! `door buildable` script), video and how it works (`captions.tsv`, then `records.tsv`). Written
//! by `mctas door catalog` and at the end of `door video-all`; never edit it by hand: edit the
//! TSVs next to the archive (or rerun `door buildable`) and regenerate.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use mctas::design::Design;

use crate::showcase::{self, Archived, BLOCK_BOARDS, asset_url, sentence};

pub struct CatalogOptions<'a> {
    pub archive: &'a Path,
    /// Worker JVMs for archived designs missing from the verdict cache.
    pub workers: usize,
    /// A local folder of rendered videos (`<stem>.mp4`, `door video-all --out`): linked.
    pub videos: Option<&'a Path>,
    /// The GitHub release the video links point to; its `<stem>.mp4` assets are linked.
    pub release: Option<&'a str>,
}

/// The verdict of each `door buildable` script next to the archive (`../tracks/buildable/scripts`),
/// by design stem: (short verdict, script path relative to the archive).
fn buildability(archive: &Path) -> HashMap<String, (String, String)> {
    let mut m = HashMap::new();
    let dir = archive.parent().unwrap_or(Path::new(".")).join("tracks/buildable/scripts");
    let Ok(rd) = std::fs::read_dir(&dir) else { return m };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".build.txt") else { continue };
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        // "# fc2: BUILDABLE (40 actions, lever off while building, ...; 32/32 positions, ...)"
        let first = text.lines().next().unwrap_or("");
        let positions = first
            .split([' ', ';', '(', ','])
            .find(|w| w.contains('/') && w.chars().all(|c| c.is_ascii_digit() || c == '/'))
            .unwrap_or("?");
        let lever = if first.contains("lever ON") { ", lever on while building" } else { "" };
        let verdict = if first.contains("NOT FOUND") {
            "no build found".to_string()
        } else if first.contains("BUILDABLE only at some positions") {
            format!("at {positions} positions{lever}")
        } else if first.contains("BUILDABLE") {
            format!("yes, {positions}{lever}")
        } else {
            continue;
        };
        m.insert(stem.to_string(), (verdict, format!("../tracks/buildable/scripts/{name}")));
    }
    m
}

/// Stems with a video: `<stem>.mp4` in the local folder and in the release.
fn video_stems(o: &CatalogOptions) -> HashSet<String> {
    let mut s = HashSet::new();
    if let Some(Ok(rd)) = o.videos.map(std::fs::read_dir) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if let Some(stem) = n.strip_suffix(".mp4").filter(|st| !st.ends_with(".part")) {
                s.insert(stem.to_string());
            }
        }
    }
    if let Some(tag) = o.release.filter(|t| !t.is_empty()) {
        let assets = showcase::release_assets(tag);
        if assets.is_empty() {
            eprintln!("warning: no assets found in release {tag} (gh): only local videos are linked");
        }
        s.extend(assets.iter().filter_map(|a| a.strip_suffix(".mp4").map(String::from)));
    }
    s
}

/// A table cell: no pipes or line breaks.
fn cell(s: &str) -> String {
    s.replace('|', "/").replace('\n', " ")
}

/// Write `<archive>/README.md`. Returns its path.
pub fn write(version: &str, o: &CatalogOptions) -> Result<PathBuf> {
    let archive = o.archive;
    let entries = showcase::archive_entries(version, archive, o.workers)?;
    let all: Vec<Archived> = entries.iter().filter(|e| e.verdict.is_ok()).map(showcase::archived_of).collect();
    let records = showcase::progression(archive);
    let record_how: HashMap<String, String> = records.iter().map(|(_, s, h)| (s.clone(), h.clone())).collect();
    let captions = showcase::captions(archive);
    let builds = buildability(archive);
    let videos = video_stems(o);
    let tag = o.release.filter(|t| !t.is_empty()).unwrap_or("videos");
    let video = |stem: &str| if videos.contains(stem) { format!("[video]({})", asset_url(tag, &format!("{stem}.mp4"))) } else { String::new() };
    // Board leaders (as `door gallery` picks them): stem -> board labels.
    let mut leads: HashMap<String, Vec<String>> = HashMap::new();
    for b in showcase::boards(&all, &record_how) {
        leads.entry(b.best.stem.clone()).or_default().push(match b.key {
            Some("sculk-unrotated") => "sculk, not rotation-safe".to_string(),
            Some(k) => k.to_string(),
            None => b.title.to_lowercase(),
        });
    }
    let how = |a: &Archived| {
        captions
            .get(&a.stem)
            .map(|h| sentence(h))
            .or_else(|| showcase::track_of(&a.path).map(|t| format!("From track {t} ([notes](../tracks/{t}/NOTES.md)).")))
            .unwrap_or_default()
    };

    let mut invalid: Vec<(String, String, String)> = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(archive)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    for p in &files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        if !stem.contains("invalid") {
            continue;
        }
        let text = std::fs::read_to_string(p).unwrap_or_default();
        let why = text.lines().next().and_then(|l| l.strip_prefix("# ")).unwrap_or("").to_string();
        let blocks = Design::parse(&text).map(|d| crate::block_count(&d).to_string()).unwrap_or_else(|_| "?".into());
        invalid.push((blocks, stem, why));
    }

    let n = |cat: &str| entries.iter().filter(|e| e.category == cat).count();
    let mut md = format!(
        "# Archive of 3x3 door designs\n\n\
         Generated by `mctas door catalog` (and at the end of `door video-all`) from the files in this folder; never edit it by hand. \
         Every design that verified, one file each, named `<name>-<blocks>b.txt` (the placed-block count it was archived with: an ID, not \
         its size). **Size** = the door's volume, as the piston door community counts it (regulation section 4.2: the bounding box of the \
         build over every state of a cycle, the lever excluded; `80b (8x10x1)`); blocks placed is secondary. Each file starts with its verify result and where it \
         came from. New designs are archived by `mctas door verify <design> --extra-offsets 28 --archive` and by \
         `door leaderboard --archive-dir`. Rules and boards: [docs/rules.md](../../../docs/rules.md); rankings: \
         [LEADERBOARD.md](../LEADERBOARD.md); the story: [docs/history.md](../../../docs/history.md).\n\n\
         Hand-edited inputs: [records.tsv](records.tsv) (the solid record progression, also read by `door montage`), \
         [captions.tsv](captions.tsv) (how each design works, also read by `door gallery`, `door montage` and `door leaderboard`), and \
         `door buildable`'s scripts in [../tracks/buildable/scripts](../tracks/buildable/scripts) (the buildable column: a survival build \
         order that ends in the judge's rest state, at how many of the 32 positions). Videos: `door video-all` (release `{tag}`). Ticks: \
         worst of 3 cycles after each lever flip.\n\n\
         {} designs: {} solid, {} gravity, {} sculk, {} entity; {} invalid ones kept for history.\n\n",
        entries.len(),
        n("solid"),
        n("gravity"),
        n("sculk"),
        n("entity"),
        invalid.len()
    );

    md.push_str(
        "## Solid record progression (blocks placed)\n\nEach row took the solid record (9 identical plain door blocks) when records counted \
         placed blocks, from `records.tsv`; sizes are volumes now (the size column below).\n\n| blocks placed | design | how | video |\n|---|---|---|---|\n",
    );
    for (blocks, stem, h) in &records {
        md.push_str(&format!("| {blocks} | [{stem}]({stem}.txt) | {} | {} |\n", cell(&sentence(h)), video(stem)));
    }
    md.push('\n');

    // One section per category (a category the list does not know yet gets its own section too).
    let mut cats: Vec<(String, String)> = [("solid", "Solid"), ("gravity", "Gravity"), ("sculk", "Sculk"), ("entity", "Entity (uses entities)")]
        .iter()
        .map(|(c, t)| (c.to_string(), t.to_string()))
        .collect();
    for e in &entries {
        if !cats.iter().any(|(c, _)| c == e.category) {
            cats.push((e.category.to_string(), format!("Category {}", e.category)));
        }
    }
    for (cat, title) in &cats {
        let mut rows: Vec<&crate::leaderboard::Entry> = entries.iter().filter(|e| e.category == cat).collect();
        if rows.is_empty() {
            continue;
        }
        rows.sort_by_key(|e| (e.size_key(), e.source.clone()));
        md.push_str(&format!(
            "## {title} ({} designs)\n\n| size | blocks | design | boards | open / close | rotations | buildable | video | how |\n|---|---|---|---|---|---|---|---|---|\n",
            rows.len()
        ));
        for e in rows {
            let a = showcase::archived_of(e);
            let f = &a.facts;
            let mut boards: Vec<String> = BLOCK_BOARDS.iter().filter(|b| b.0 != "sculk-unrotated" && (b.3)(f)).map(|b| b.0.to_string()).collect();
            if !f.rotation_safe {
                boards.push("not rotation-safe".into());
            }
            if let Some(labels) = leads.get(&a.stem) {
                boards.extend(labels.iter().map(|t| format!("**#1 {t}**")));
            }
            let ticks = match (&e.verdict, f.open_ticks, f.close_ticks) {
                (Err(why), _, _) => format!("broken today: {}", why.chars().take(80).collect::<String>()),
                (Ok(()), Some(op), Some(cl)) => format!("{op} / {cl}"),
                _ => String::new(),
            };
            let rotations = match &e.rotation {
                Some(Ok(())) => "safe".to_string(),
                Some(Err((deg, _))) => format!("breaks at {deg}"),
                None => String::new(),
            };
            let build = builds.get(&a.stem).map(|(v, link)| format!("[{v}]({link})")).unwrap_or_default();
            md.push_str(&format!(
                "| {} | {} | [{}]({}.txt) | {} | {} | {} | {} | {} | {} |\n",
                e.size(),
                e.blocks,
                a.stem,
                a.stem,
                boards.join(", "),
                cell(&ticks),
                rotations,
                build,
                video(&a.stem),
                cell(&how(&a))
            ));
        }
        md.push('\n');
    }

    md.push_str("## Invalid (kept for history)\n\nThese fail today's judge; the first line of each file says why.\n\n| blocks placed | design | why |\n|---|---|---|\n");
    for (blocks, stem, why) in &invalid {
        md.push_str(&format!("| {blocks} | [{stem}]({stem}.txt) | {} |\n", cell(why)));
    }
    let out = archive.join("README.md");
    std::fs::write(&out, md)?;
    eprintln!("wrote {} ({} designs, {} videos linked)", out.display(), entries.len(), entries.iter().filter(|e| videos.contains(&showcase::archived_of(e).stem)).count());
    Ok(out)
}
