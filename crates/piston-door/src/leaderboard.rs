//! `mctas door leaderboard`: gather every door design on main (HEAD) and on the agents'
//! `explore/*` branches, verify each one with the judge (main's verifier, extra offsets), and
//! write a ranked table. Identical designs are verified once; the first source is kept.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Result, bail};
use mctas::design::Design;
use mctas::sim::{Pool, WorkerConfig};

use crate::showcase::{asset_url, sentence};
use crate::{VerifyOptions, block_count, category};

/// One candidate and its verdict.
pub struct Entry {
    pub source: String,
    pub design: Design,
    pub blocks: usize,
    pub category: &'static str,
    pub verdict: std::result::Result<(), String>,
    /// Worst-case ticks after the lever flip until open / closed (over the reported cycles).
    pub open_ticks: Option<u32>,
    pub close_ticks: Option<u32>,
    /// For a working design: `Ok(())` if it also works turned 90, 180 and 270 degrees
    /// ([`crate::rotation_check`]), else the first failing turn in degrees and the reason.
    /// `None` for a broken design.
    pub rotation: Option<std::result::Result<(), (u32, String)>>,
    /// For a working design with nothing below its door ([`crate::on_ground`]): `Ok(())` if it
    /// also works standing on a stone floor ([`crate::ground_check`]), else the judge's reason.
    /// `None` otherwise.
    pub ground: Option<std::result::Result<(), String>>,
    /// For a working design: the space it needs at rest and while it moves
    /// ([`crate::swept_shape`]); the shape boards use it.
    pub swept: Option<crate::Shape>,
}

impl Entry {
    pub fn rotation_safe(&self) -> bool {
        matches!(self.rotation, Some(Ok(())))
    }

    /// Nothing below the door at rest or while it moves, and it works standing on the ground
    /// (the "on the ground" board).
    pub fn ground_safe(&self) -> bool {
        matches!(self.ground, Some(Ok(()))) && crate::on_ground(&self.design) && self.swept.is_some_and(|s| s.below == 0)
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output()?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Candidate designs as (source label, design): every `.txt` under designs/piston-door-3x3 on
/// HEAD and on every ref under `refs_prefix`, parsed, with door+lever ports, deduplicated. Blobs
/// are read through one `git cat-file --batch` (each distinct file once, however many branches
/// carry it).
pub fn collect(repo: &Path, refs_prefix: &str, max_blocks: usize) -> Result<Vec<(String, Design)>> {
    let mut refs = vec!["HEAD".to_string()];
    for line in git(repo, &["for-each-ref", "--format=%(refname:short)", refs_prefix])?.lines() {
        refs.push(line.trim().to_string());
    }
    // (ref, path, blob id)
    let mut items: Vec<(String, String, String)> = Vec::new();
    for r in &refs {
        let Ok(list) = git(repo, &["ls-tree", "-r", r, "designs/piston-door-3x3"]) else { continue };
        for line in list.lines() {
            let Some((meta, path)) = line.split_once('\t') else { continue };
            let f: Vec<&str> = meta.split_whitespace().collect();
            if f.len() == 3 && f[1] == "blob" && path.ends_with(".txt") {
                items.push((r.clone(), path.to_string(), f[2].to_string()));
            }
        }
    }
    let mut blobs: Vec<String> = items.iter().map(|i| i.2.clone()).collect();
    blobs.sort();
    blobs.dedup();
    let texts = cat_file_batch(repo, &blobs)?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (r, f, blob) in items {
        let Some(text) = texts.get(&blob) else { continue };
        let Ok(d) = Design::parse(text) else { continue };
        if !d.ports.contains_key("door") || !d.ports.contains_key("lever") || block_count(&d) > max_blocks {
            continue;
        }
        let key = d.content_key();
        if seen.insert(key) {
            out.push((format!("{r}:{f}"), d));
        }
    }
    Ok(out)
}

/// The contents of git blobs, through one `git cat-file --batch` process.
fn cat_file_batch(repo: &Path, blobs: &[String]) -> Result<std::collections::HashMap<String, String>> {
    use std::io::{BufRead, Read, Write};
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let list: Vec<String> = blobs.to_vec();
    let writer = std::thread::spawn(move || {
        let mut w = std::io::BufWriter::new(&mut stdin);
        for b in &list {
            let _ = writeln!(w, "{b}");
        }
    });
    let mut rd = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut out = std::collections::HashMap::new();
    for b in blobs {
        let mut header = String::new();
        rd.read_line(&mut header)?;
        let f: Vec<&str> = header.split_whitespace().collect();
        if f.len() < 3 {
            continue;
        }
        let size: usize = f[2].parse().unwrap_or(0);
        let mut buf = vec![0u8; size + 1];
        rd.read_exact(&mut buf)?;
        buf.truncate(size);
        out.insert(b.clone(), String::from_utf8_lossy(&buf).into_owned());
    }
    let _ = writer.join();
    let _ = child.wait();
    Ok(out)
}

/// Content hash of a design (blocks + ports), stable across runs (FNV-1a 64).
pub fn design_hash(d: &Design) -> u64 {
    let key = d.content_key();
    let mut h: u64 = 0xcbf29ce484222325;
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Verdict cache: design hash -> (verdict, open ticks, close ticks, rotation result). Bump the
/// file name when the rules change so old verdicts are dropped. Only the coordinator's
/// leaderboard writes it: each design is verified once, by main's verifier (agents' own runs are
/// not trusted), and every later run reuses the verdict.
const CACHE_FILE: &str = "verdicts-v5.tsv";
type Rotation = Option<std::result::Result<(), (u32, String)>>;
type Cached = (std::result::Result<(), String>, Option<u32>, Option<u32>, Rotation);

fn load_cache(dir: &Path) -> std::collections::HashMap<u64, Cached> {
    let mut m = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(dir.join(CACHE_FILE)) else { return m };
    for line in text.lines() {
        // hash, ok|broken, open, close, safe|<degrees>|-, rotation reason, verdict reason
        let f: Vec<&str> = line.splitn(7, '\t').collect();
        if f.len() < 7 {
            continue;
        }
        let Ok(h) = u64::from_str_radix(f[0], 16) else { continue };
        let verdict = if f[1] == "ok" { Ok(()) } else { Err(f[6].to_string()) };
        let rotation = match f[4] {
            "safe" => Some(Ok(())),
            "-" => None,
            deg => Some(Err((deg.parse().unwrap_or(0), f[5].to_string()))),
        };
        m.insert(h, (verdict, f[2].parse().ok(), f[3].parse().ok(), rotation));
    }
    m
}

/// Shape-facts cache: design hash -> the swept shape ([`crate::swept_shape`]: front behind below
/// above left right blocks-below blocks-above) and the ground check ([`crate::ground_check`]:
/// `safe`, `-` when something stands below the door, else the judge's reason), for working
/// designs. Main's verifier and simulator are deterministic. v2: east/west-facing doors
/// (`door-axis: z`) measured along their own axes.
const SHAPES_CACHE_FILE: &str = "shapes-v2.tsv";
type ShapeFacts = (crate::Shape, Option<std::result::Result<(), String>>);

fn load_shapes(dir: &Path) -> std::collections::HashMap<u64, ShapeFacts> {
    let mut m = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(dir.join(SHAPES_CACHE_FILE)) else { return m };
    for line in text.lines() {
        let f: Vec<&str> = line.splitn(3, '\t').collect();
        if f.len() < 3 {
            continue;
        }
        let Ok(h) = u64::from_str_radix(f[0], 16) else { continue };
        let n: Vec<i64> = f[1].split(' ').filter_map(|x| x.parse().ok()).collect();
        if n.len() != 8 {
            continue;
        }
        let shape = crate::Shape {
            front: n[0] as i32,
            behind: n[1] as i32,
            below: n[2] as i32,
            above: n[3] as i32,
            left: n[4] as i32,
            right: n[5] as i32,
            blocks_below: n[6] as usize,
            blocks_above: n[7] as usize,
        };
        let ground = match f[2] {
            "safe" => Some(Ok(())),
            "-" => None,
            why => Some(Err(why.to_string())),
        };
        m.insert(h, (shape, ground));
    }
    m
}

fn save_shapes(dir: &Path, m: &std::collections::HashMap<u64, ShapeFacts>) -> Result<()> {
    let mut lines: Vec<String> = m
        .iter()
        .map(|(h, (s, g))| {
            let ground = match g {
                Some(Ok(())) => "safe".to_string(),
                None => "-".to_string(),
                Some(Err(e)) => e.replace(['\t', '\n'], " "),
            };
            format!(
                "{h:016x}\t{} {} {} {} {} {} {} {}\t{ground}",
                s.front, s.behind, s.below, s.above, s.left, s.right, s.blocks_below, s.blocks_above
            )
        })
        .collect();
    lines.sort();
    std::fs::write(dir.join(SHAPES_CACHE_FILE), lines.join("\n") + "\n")?;
    Ok(())
}

/// Fill in [`Entry::swept`] and [`Entry::ground`] for the working designs: from the cache in
/// `cache_dir`, the others simulated (one cycle for the swept shape; the judge's positions on a
/// stone floor for designs with nothing below the door).
fn judge_shapes(version: &str, entries: &mut [Entry], extra_offsets: u32, workers: usize, cache_dir: Option<&Path>) -> Result<()> {
    let mut cache = cache_dir.map(load_shapes).unwrap_or_default();
    let todo: Vec<(u64, Design)> = {
        let mut seen = HashSet::new();
        entries
            .iter()
            .filter(|e| e.verdict.is_ok())
            .map(|e| (design_hash(&e.design), e.design.clone()))
            .filter(|(h, _)| !cache.contains_key(h) && seen.insert(*h))
            .collect()
    };
    if !todo.is_empty() {
        eprintln!("{} working designs to measure (swept shape, ground check)", todo.len());
        let pool = Pool::spawn(workers.min(todo.len()).max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
        let mut opts = VerifyOptions::default();
        opts.offsets.extend(crate::extra_offsets(extra_offsets));
        let results = pool.map(todo, |w, _, (h, d)| {
            let swept = crate::swept_shape(w, &d)?;
            let ground = if crate::on_ground(&d) { Some(crate::ground_check(w, &d, &opts)?.map_or(Ok(()), Err)) } else { None };
            Ok((h, (swept, ground)))
        });
        for r in results {
            let (h, facts) = r?;
            cache.insert(h, facts);
        }
        if let Some(dir) = cache_dir {
            save_shapes(dir, &cache)?;
        }
    }
    for e in entries.iter_mut().filter(|e| e.verdict.is_ok()) {
        if let Some((s, g)) = cache.get(&design_hash(&e.design)) {
            e.swept = Some(*s);
            e.ground = g.clone();
        }
    }
    Ok(())
}

fn save_cache(dir: &Path, m: &std::collections::HashMap<u64, Cached>) -> Result<()> {
    let mut lines: Vec<String> = m
        .iter()
        .map(|(h, (v, o, c, rot))| {
            let (tag, err) = match v {
                Ok(()) => ("ok", String::new()),
                Err(e) => ("broken", e.replace(['\t', '\n'], " ")),
            };
            let num = |x: &Option<u32>| x.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
            let (rtag, rwhy) = match rot {
                Some(Ok(())) => ("safe".to_string(), String::new()),
                Some(Err((deg, why))) => (deg.to_string(), why.replace(['\t', '\n'], " ")),
                None => ("-".to_string(), String::new()),
            };
            format!("{h:016x}\t{tag}\t{}\t{}\t{rtag}\t{rwhy}\t{err}", num(o), num(c))
        })
        .collect();
    lines.sort();
    std::fs::write(dir.join(CACHE_FILE), lines.join("\n") + "\n")?;
    Ok(())
}

/// Keep only the candidates whose verdict is cached in `dir` (nothing is simulated); returns how
/// many were dropped.
pub fn only_cached(cands: &mut Vec<(String, Design)>, dir: &Path) -> usize {
    let cache = load_cache(dir);
    let before = cands.len();
    cands.retain(|(_, d)| cache.contains_key(&design_hash(d)));
    before - cands.len()
}

/// Verify every candidate in parallel, then run the rotation check on the working ones. With
/// `cache_dir`, verdicts and timings are cached by design content (main's verifier is
/// deterministic), so only new designs are simulated.
pub fn judge(version: &str, cands: Vec<(String, Design)>, extra_offsets: u32, workers: usize, cache_dir: Option<&Path>) -> Result<Vec<Entry>> {
    let mut cache = cache_dir.map(load_cache).unwrap_or_default();
    let (hit, miss): (Vec<_>, Vec<_>) = cands.into_iter().partition(|(_, d)| cache.contains_key(&design_hash(d)));
    eprintln!("{} cached verdicts, {} designs to verify", hit.len(), miss.len());
    let mut entries: Vec<Entry> = hit
        .into_iter()
        .map(|(source, d)| {
            let (verdict, open_ticks, close_ticks, rotation) = cache[&design_hash(&d)].clone();
            Entry { source, blocks: block_count(&d), category: category(&d), verdict, design: d, open_ticks, close_ticks, rotation, ground: None, swept: None }
        })
        .collect();
    if !miss.is_empty() {
        let pool = Pool::spawn(workers.min(miss.len()).max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
        let mut opts = VerifyOptions::default();
        opts.offsets.extend(crate::extra_offsets(extra_offsets));
        let results = pool.map(miss, |w, _, (source, d)| {
            let (verdict, open_ticks, close_ticks, rotation) = match crate::verify(w, &d, &opts) {
                Ok(r) => {
                    let rotation = match crate::rotation_check(w, &d, &opts)? {
                        None => Ok(()),
                        Some((turns, why)) => Err((turns as u32 * 90, why)),
                    };
                    (Ok(()), r.cycles.iter().map(|c| c.open_ticks).max(), r.cycles.iter().map(|c| c.close_ticks).max(), Some(rotation))
                }
                Err(e) => (Err(format!("{e:#}")), None, None, None),
            };
            Ok(Entry { source, blocks: block_count(&d), category: category(&d), verdict, design: d, open_ticks, close_ticks, rotation, ground: None, swept: None })
        });
        for e in results.into_iter().collect::<Result<Vec<Entry>>>()? {
            cache.insert(design_hash(&e.design), (e.verdict.clone(), e.open_ticks, e.close_ticks, e.rotation.clone()));
            entries.push(e);
        }
    }
    if let Some(dir) = cache_dir {
        save_cache(dir, &cache)?;
    }
    judge_shapes(version, &mut entries, extra_offsets, workers, cache_dir)?;
    entries.sort_by(|a, b| {
        let rank = |e: &Entry| (e.verdict.is_err(), e.category != "solid", e.blocks);
        (rank(a), a.source.as_str()).cmp(&(rank(b), b.source.as_str()))
    });
    Ok(entries)
}

/// What the boards show besides their tables: the assets of a GitHub release (each board
/// leader's GIF from `door gallery`, design videos from `door video-all`, the montages and the
/// timeline chart), and how each design works (`captions.tsv` / `records.tsv` of the archive).
pub struct Extras {
    pub tag: String,
    pub assets: HashSet<String>,
    pub captions: HashMap<String, String>,
    pub archive: PathBuf,
    /// Designs left out because their verdict was not cached (`--cached-only`).
    pub uncached: usize,
}

impl Extras {
    /// Captions from `archive`, asset names of the release `tag` (`gh`; none when `tag` is empty).
    pub fn load(archive: &Path, tag: &str) -> Extras {
        let assets = if tag.is_empty() { HashSet::new() } else { crate::showcase::release_assets(tag) };
        if assets.is_empty() && !tag.is_empty() {
            eprintln!("warning: no assets found in release {tag} (gh): the boards get no GIFs or video links");
        }
        Extras { tag: tag.to_string(), assets, captions: crate::showcase::captions(archive), archive: archive.to_path_buf(), uncached: 0 }
    }

    fn has(&self, name: &str) -> bool {
        self.assets.contains(name)
    }
}

/// The stem of a source's file: `HEAD:designs/piston-door-3x3/archive/fc2-40b.txt` -> `fc2-40b`.
fn stem(source: &str) -> String {
    let f = source.rsplit(['/', ':']).next().unwrap_or(source);
    f.strip_suffix(".txt").unwrap_or(f).to_string()
}

/// A readable name for a source: its stem, prefixed by its track for a file in a track's folder
/// (`speed-2/BEST`).
fn label(source: &str) -> String {
    let path = source.split_once(':').map_or(source, |(_, p)| p);
    match path.strip_prefix("designs/piston-door-3x3/tracks/").and_then(|rest| rest.split_once('/')) {
        Some((track, rest)) if !rest.starts_with("archive/") => format!("{track}/{}", stem(source)),
        _ => stem(source),
    }
}

/// A markdown link target for a source, from LEADERBOARD.md's folder: a relative path for HEAD
/// files, the file on GitHub for a branch.
fn source_link(source: &str) -> String {
    let (r, path) = source.split_once(':').unwrap_or(("HEAD", source));
    if r == "HEAD" {
        path.strip_prefix("designs/piston-door-3x3/").map(String::from).unwrap_or_else(|| format!("../../{path}"))
    } else {
        format!("https://github.com/vjeux/minecraft-tas/blob/{}/{path}", r.strip_prefix("origin/").unwrap_or(r))
    }
}

/// The leader of a board, above its table: the GIF of the first of `tied` (the leader and the
/// designs tied with it) that has one, then one line about that design (blocks, kind, ticks,
/// file, video, how it works) and the board's record progression video when there is one.
/// The community regulation's volume of a design (a door labeled "140b" is 10 x 14 x 1): `door
/// properties`'s `volume` (W x H x D of the build over every state) when cached, else the swept
/// bounding box (at rest and while it moves, the door included).
fn volume_of(e: &Entry, props: &HashMap<u64, crate::properties::Props>) -> String {
    if let Some(p) = props.get(&design_hash(&e.design)) {
        if let Some(v) = p.get("volume").filter(|v| !v.is_empty() && v.as_str() != "-") {
            return match p.get("volume_dims") {
                Some(d) => format!("{v} ({d})"),
                None => v.clone(),
            };
        }
    }
    match e.swept {
        Some(s) => format!("{} ({}x{}x{})", s.volume(), s.width(), s.height(), s.thickness()),
        None => "-".into(),
    }
}

fn volume_num(e: &Entry, props: &HashMap<u64, crate::properties::Props>) -> i64 {
    props
        .get(&design_hash(&e.design))
        .and_then(|p| p.get("volume"))
        .and_then(|v| v.parse().ok())
        .or_else(|| e.swept.map(|s| s.volume() as i64))
        .unwrap_or(i64::MAX)
}

/// Seamless tier rank from props' `seamless` value ("QUART z-,z+", "FULL"...): SUPER 4, FULL 3,
/// SEMI 2, QUART 1, none 0.
pub fn tier_rank(t: &str) -> u8 {
    match t.split_whitespace().next().unwrap_or("") {
        "SUPER" => 4,
        "FULL" => 3,
        "SEMI" => 2,
        "QUART" => 1,
        _ => 0,
    }
}

/// The boards that follow the piston door community's regulation (vjeux: seamless is what the
/// community cares about most; sizes are volumes): "Seamless", "Seamless, flat, no slime or
/// honey" (ranked by tier, then hallway-compatible first, then volume, then blocks) and
/// "Record-eligible" (ranked by volume, then blocks). Facts come from the `door properties` /
/// `door properties-dyn` caches; designs not measured yet are left out.
fn seamless_boards(
    s: &mut String,
    x: &Extras,
    entries: &[Entry],
    props: &HashMap<u64, crate::properties::Props>,
    dyn_props: &HashMap<u64, crate::properties::Props>,
    shown: &mut HashMap<u64, String>,
) {
    let get = |e: &Entry, k: &str| -> String {
        let h = design_hash(&e.design);
        dyn_props.get(&h).and_then(|p| p.get(k)).or_else(|| props.get(&h).and_then(|p| p.get(k))).cloned().unwrap_or_default()
    };
    let yes = |v: String| v.starts_with("yes");
    let tier = |e: &Entry| get(e, "seamless");
    let hallway = |e: &Entry| yes(get(e, "hallway"));
    let eligible = |e: &Entry| {
        tier_rank(&tier(e)) > 0
            && yes(get(e, "lever_surface"))
            && yes(get(e, "lever_support"))
            && yes(get(e, "repeater_input"))
            && yes(get(e, "uses"))
            && yes(get(e, "steps"))
            && hallway(e)
    };
    let working = |e: &&Entry| e.verdict.is_ok() && e.rotation_safe();
    let seamless_rows = |list: &[&Entry]| -> Vec<String> {
        list.iter()
            .take(10)
            .enumerate()
            .map(|(i, e)| {
                let t = tier(e);
                let hall = if hallway(e) { "yes" } else { "no" };
                format!("| {} | {} | {hall} | {} | {} | `{}` |\n", i + 1, if t.is_empty() { "-".into() } else { t }, volume_of(e, props), e.blocks, e.source)
            })
            .collect()
    };
    let boards: [(&str, &str, Box<dyn Fn(&Entry) -> bool>); 2] = [
        (
            "Seamless",
            "solid doors with a SEAMLESS tier of the community regulation (QUART: nothing visible on the front side when closed; SEMI: also \
             nothing in the hallway when open; FULL: also clean from the back when closed; SUPER: also clean at every tick while moving), \
             ranked by tier, then whether the hallway can be built around it, then volume (the community's size: the bounding box over every \
             state), then blocks",
            Box::new(|e: &Entry| e.category == "solid"),
        ),
        (
            "Seamless, flat (one block thick), no slime or honey",
            "the same ranking for flat, slime-free doors (every block in the door's plane)",
            Box::new(|e: &Entry| e.category == "solid" && crate::slime_free(&e.design) && crate::single_layer(&e.design)),
        ),
    ];
    for (title, rule, keep) in &boards {
        s.push_str(&format!("## {title}\n\n{rule}; rotation-safe.\n\n"));
        let mut list: Vec<&Entry> = entries.iter().filter(working).filter(|e| keep(e) && tier_rank(&tier(e)) > 0).collect();
        list.sort_by_key(|e| (std::cmp::Reverse(tier_rank(&tier(e))), !hallway(e), volume_num(e, props), e.blocks));
        if let Some(first) = list.first() {
            push_leader(s, x, &[*first], None, title, shown);
        }
        push_collapsed(s, "| # | seamless | hallway | volume | blocks | source |\n|---|---|---|---|---|---|\n", &seamless_rows(&list), "| - | - | no entry yet |\n");
    }
    let title = "Record-eligible (community regulation)";
    s.push_str(&format!(
        "## {title}\n\ndoors that meet the regulation's record eligibility (section 3): a SEAMLESS tier, the hallway can be built around \
         them, the lever on the surface with steady support, works with repeater input too, unlimited uses, footsteps in the hallway \
         harmless; ranked by volume, then blocks; rotation-safe.\n\n"
    ));
    let mut list: Vec<&Entry> = entries.iter().filter(working).filter(|e| e.category != "entity" && eligible(e)).collect();
    list.sort_by_key(|e| (volume_num(e, props), e.blocks));
    if let Some(first) = list.first() {
        push_leader(s, x, &[*first], None, title, shown);
    }
    let rows: Vec<String> = list
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, e)| {
            let ticks = match (e.open_ticks, e.close_ticks) {
                (Some(o), Some(c)) => format!("{o} / {c}"),
                _ => "-".into(),
            };
            format!("| {} | {} | {} | {} | {ticks} | `{}` |\n", i + 1, volume_of(e, props), e.blocks, tier(e), e.source)
        })
        .collect();
    push_collapsed(s, "| # | volume | blocks | seamless | open / close | source |\n|---|---|---|---|---|---|\n", &rows, "| - | - | no entry yet |\n");
}

/// GitHub's anchor for a markdown heading (lowercase, spaces to hyphens, other punctuation dropped).
fn anchor(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// The leader block of a board: its GIF, video and how it works. A design that already leads an
/// earlier board (`shown`: design hash -> that board's title) gets one line pointing there instead
/// of the same GIF again.
fn push_leader(s: &mut String, x: &Extras, tied: &[&Entry], progression: Option<&str>, title: &str, shown: &mut HashMap<u64, String>) {
    let Some(first) = tied.first() else { return };
    let progression_line = |s: &mut String| {
        if let Some(k) = progression.filter(|k| x.has(&format!("{k}-progression.mp4"))) {
            s.push_str(&format!(
                "Every record of this board in one video: [{k}-progression.mp4]({}) ([poster]({})).\n\n",
                asset_url(&x.tag, &format!("{k}-progression.mp4")),
                asset_url(&x.tag, &format!("{k}-progression.jpg"))
            ));
        }
    };
    let h = design_hash(&first.design);
    if let Some(prev) = shown.get(&h) {
        let timing = match (first.open_ticks, first.close_ticks) {
            (Some(o), Some(c)) => format!(", opens in {o} ticks, closes in {c}"),
            _ => String::new(),
        };
        s.push_str(&format!(
            "Same leader as [{prev}](#{}): **{} blocks**{timing}, [{}]({}).\n\n",
            anchor(prev),
            first.blocks,
            label(&first.source),
            source_link(&first.source)
        ));
        progression_line(s);
        return;
    }
    shown.insert(h, title.to_string());
    let with_gif = tied.iter().find(|e| x.has(&format!("{}.gif", stem(&e.source)))).copied();
    let e = with_gif.unwrap_or(first);
    let st = stem(&e.source);
    if with_gif.is_some() {
        s.push_str(&format!("![{st}]({})\n\n", asset_url(&x.tag, &format!("{st}.gif"))));
    }
    let mut kind = e.category.to_string();
    if crate::slime_free(&e.design) {
        kind.push_str(", slime-free");
    }
    if crate::single_layer(&e.design) {
        kind.push_str(", flat");
    }
    let timing = match (e.open_ticks, e.close_ticks) {
        (Some(o), Some(c)) => format!(", opens in {o} ticks, closes in {c} (worst cycle)"),
        _ => String::new(),
    };
    let video = if x.has(&format!("{st}.mp4")) { format!(" ([video]({}))", asset_url(&x.tag, &format!("{st}.mp4"))) } else { String::new() };
    let how = x.captions.get(&st).map(|h| sentence(h)).unwrap_or_else(|| {
        let track = match e.source.split_once(':') {
            Some(("HEAD", path)) => crate::showcase::track_of(Path::new(path)),
            Some((r, _)) => r.rsplit('/').next().map(String::from),
            None => None,
        };
        track.map(|t| format!("From track {t}: [tracks/{t}/NOTES.md](tracks/{t}/NOTES.md).")).unwrap_or_default()
    });
    s.push_str(&format!("**{} blocks**, {kind}{timing}: [{}]({}){video}. {how}\n\n", e.blocks, label(&e.source), source_link(&e.source)));
    if !std::ptr::eq(e, *first) {
        s.push_str(&format!("(Tied with the #1 row, [{}]({}), which has no GIF yet.)\n\n", label(&first.source), source_link(&first.source)));
    }
    progression_line(s);
}

/// Scopes of the Pareto and speed boards: every category but entity (entities are banned
/// everywhere but their own board), solid only, and solid with entities allowed.
const SCOPES: [(&str, fn(&Entry) -> bool); 3] = [
    ("any category", |e: &Entry| e.category != "entity"),
    ("solid only", |e: &Entry| e.category == "solid"),
    ("entities allowed", |e: &Entry| crate::entities_allowed_board(&e.design)),
];

/// LEADERBOARD.md: every board, its leader with a GIF, the rest collapsed.
pub fn render(entries: &[Entry], extra_offsets: u32, x: &Extras) -> String {
    // Design hash -> the first board it leads: a door leading several boards shows its GIF once.
    let mut shown: HashMap<u64, String> = HashMap::new();
    // The community regulation's properties (`door properties` / `door properties-dyn` caches next
    // to the archive): seamless tiers, their volume, hallway, record eligibility.
    let props = crate::properties::load_cache(&x.archive);
    let dyn_props = crate::properties::load_dyn(&x.archive);
    let mut s = format!(
        "# 3x3 piston door leaderboards\n\nGenerated by `mctas door leaderboard`; never edit it by hand. Every design on main and on the `explore/*` \
         branches, judged once by main's verifier (3 open/close cycles; new designs at {} positions, older verdicts at up to 32). Each board shows its leader (GIF from `door gallery`, \
         video, how it works; a door leading several boards is shown once); click \"more\" for the rest. What every board means and the rules: [docs/rules.md](../../docs/rules.md). \
         Every archived design with its video: [archive/README.md](archive/README.md). The story: [docs/history.md](../../docs/history.md).\n\n\
         Walkway \"clean\" = nothing in the 3-deep walkway at rest (designs judged before the 2026-10-18 16:40Z \"nothing in front of or behind \
         the door\" rule keep their verdicts). The boards rank rotation-safe designs; the others are on [Not rotation-safe](#not-rotation-safe).\n\n",
        4 + extra_offsets
    );
    if x.uncached > 0 {
        s.push_str(&format!("({} designs without a cached verdict are not judged yet.)\n\n", x.uncached));
    }
    let best_solid = entries.iter().find(|e| e.verdict.is_ok() && e.rotation_safe() && e.category == "solid").map(|e| e.blocks);
    if x.has(crate::showcase::MONTAGE_ASSET) {
        s.push_str(&format!(
            "The whole solid record progression in one video ({}, `mctas door montage`; click the poster):\n\n[![every solid record]({})]({})\n\n",
            crate::showcase::progression_title(&x.archive, best_solid),
            asset_url(&x.tag, crate::showcase::POSTER_ASSET),
            asset_url(&x.tag, crate::showcase::MONTAGE_ASSET)
        ));
    }
    if x.has(crate::showcase::TIMELINE_ASSET) {
        s.push_str(&format!("The night, board by board (records from the archive):\n\n![records over the night]({})\n\n", asset_url(&x.tag, crate::showcase::TIMELINE_ASSET)));
    }
    let boards: Vec<(&str, &str, &str, Box<dyn Fn(&Entry) -> bool>)> = vec![
        ("Solid (main ranking)", "solid", "9 identical plain door blocks, no sculk sensors", Box::new(|e: &Entry| e.category == "solid")),
        ("Solid, no slime or honey", "slime-free", "same, and not a single slime or honey block anywhere", Box::new(|e: &Entry| e.category == "solid" && crate::slime_free(&e.design))),
        (
            "Solid, flat (one z layer), no slime or honey",
            "flat",
            "slime-free, and every block (door, pistons, wiring, lever) in the door's plane: the whole machine is one block thick",
            Box::new(|e: &Entry| e.category == "solid" && crate::slime_free(&e.design) && crate::single_layer(&e.design)),
        ),
        (
            "Solid, in a 3-thick wall",
            "wall",
            "every block within one layer of the door's plane (z = door.z-1..door.z+1), at rest and while it moves: the whole machine hides inside a wall 3 blocks thick",
            Box::new(|e: &Entry| e.category == "solid" && e.swept.is_some_and(|s| s.front <= 1 && s.behind <= 1)),
        ),
        (
            "Solid, on the ground (nothing below the door)",
            "ground",
            "no block below the door's bottom row, at rest or while it moves, so it stands on flat ground with no digging; it must also work standing on a stone floor (slime may not drag the ground)",
            Box::new(|e: &Entry| e.category == "solid" && e.ground_safe()),
        ),
        ("Sculk", "sculk", "uses sculk sensors (footsteps or mobs nearby can trigger them)", Box::new(|e: &Entry| e.category == "sculk")),
        ("Gravity", "gravity", "the 9 door blocks are sand, gravel or concrete powder", Box::new(|e: &Entry| e.category == "gravity")),
        (
            "Entities allowed",
            "entities",
            "solid doors that may also use entities (minecarts, armor stands, boats...; category `entity`). Entities are counted apart from \
             blocks: ranked by blocks, then entities. Every entity must be back exactly where it rested, at rest, after each close, and \
             nothing else may appear; every other board bans entities. Solid designs compete here too (0 entities)",
            Box::new(|e: &Entry| crate::entities_allowed_board(&e.design)),
        ),
    ];
    for (title, key, rule, keep) in &boards {
        s.push_str(&format!("## {title}\n\n{rule}; rotation-safe.\n\n"));
        let entities_col = title.starts_with("Entities");
        let mut members: Vec<&Entry> = entries.iter().filter(|e| e.verdict.is_ok() && e.rotation_safe() && keep(e)).collect();
        if entities_col {
            // Blocks first, entities break ties (stable: the usual order otherwise).
            members.sort_by_key(|e| (e.blocks, crate::entity_count(&e.design)));
        }
        let board: Vec<&Entry> = members.into_iter().take(10).collect();
        let tied: Vec<&Entry> = board
            .iter()
            .take_while(|e| e.blocks == board[0].blocks && crate::entity_count(&e.design) == crate::entity_count(&board[0].design))
            .copied()
            .collect();
        push_leader(&mut s, x, &tied, Some(key), title, &mut shown);
        let rows: Vec<String> = board
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let clutter = crate::walkway_clutter(&e.design);
                let walkway = if clutter == 0 { "clean".to_string() } else { format!("{clutter} blocks in it") };
                let vol = volume_of(e, &props);
                if entities_col {
                    let ticks = match (e.open_ticks, e.close_ticks) {
                        (Some(o), Some(c)) => format!("{o} / {c}"),
                        _ => "-".to_string(),
                    };
                    format!("| {} | {} | {} | {vol} | {ticks} | {walkway} | `{}` |\n", i + 1, e.blocks, e.design.entities.len(), e.source)
                } else {
                    format!("| {} | {} | {vol} | {walkway} | `{}` |\n", i + 1, e.blocks, e.source)
                }
            })
            .collect();
        let header = if entities_col {
            "| # | blocks | entities | volume | open / close | walkway | source |\n|---|---|---|---|---|---|---|\n"
        } else {
            "| # | blocks | volume | walkway | source |\n|---|---|---|---|---|\n"
        };
        push_collapsed(&mut s, header, &rows, "| - | - | no entry yet |\n");
        if entities_col {
            // The doors that do use entities, even when solid ones beat them.
            let ent_rows: Vec<String> = entries
                .iter()
                .filter(|e| e.verdict.is_ok() && e.rotation_safe() && e.category == "entity" && keep(e))
                .take(5)
                .enumerate()
                .map(|(i, e)| {
                    let ticks = match (e.open_ticks, e.close_ticks) {
                        (Some(o), Some(c)) => format!("{o} / {c}"),
                        _ => "-".to_string(),
                    };
                    let kinds: Vec<String> = e.design.entities.iter().map(|x| x.kind.trim_start_matches("minecraft:").to_string()).collect();
                    format!("| {} | {} | {} | {ticks} | `{}` |\n", i + 1, e.blocks, kinds.join(", "), e.source)
                })
                .collect();
            s.push_str("Doors that use entities (category `entity`):\n\n");
            push_collapsed(&mut s, "| # | blocks | entities | open / close | source |\n|---|---|---|---|---|\n", &ent_rows, "| - | - | no entry yet |\n");
        }
    }
    seamless_boards(&mut s, x, entries, &props, &dyn_props, &mut shown);
    // Smallest box: the bounding-box volume of the space the build needs (at rest and while it
    // moves, the door included), ties by blocks.
    s.push_str(
        "## Solid, smallest box\n\nthe fewest cells in the bounding box of the space the build needs, at rest and while it moves (width x height x thickness, \
         the door included), ties by blocks; rotation-safe.\n\n",
    );
    let mut boxed: Vec<(&Entry, crate::Shape)> = entries
        .iter()
        .filter(|e| e.verdict.is_ok() && e.rotation_safe() && e.category == "solid")
        .filter_map(|e| Some((e, e.swept?)))
        .collect();
    boxed.sort_by(|a, b| (a.1.volume(), a.0.blocks, &a.0.source).cmp(&(b.1.volume(), b.0.blocks, &b.0.source)));
    let tied: Vec<&Entry> = boxed.iter().take_while(|(e, sh)| (sh.volume(), e.blocks) == (boxed[0].1.volume(), boxed[0].0.blocks)).map(|(e, _)| *e).collect();
    push_leader(&mut s, x, &tied, None, "Solid, smallest box", &mut shown);
    let rows: Vec<String> = boxed
        .iter()
        .take(10)
        .enumerate()
        .map(|(i, (e, sh))| format!("| {} | {} | {} x {} x {} | {} | `{}` |\n", i + 1, sh.volume(), sh.width(), sh.height(), sh.thickness(), e.blocks, e.source))
        .collect();
    push_collapsed(&mut s, "| # | volume | box (W x H x T) | blocks | source |\n|---|---|---|---|---|\n", &rows, "| - | - | no entry yet |\n");
    // Working designs that break when turned: their own board, every category.
    s.push_str(
        "## Not rotation-safe\n\nWorking designs (they pass the judge as built) that break when turned 90, 180 or 270 degrees; the first \
         failing turn and the judge's reason.\n\n",
    );
    let unrotated: Vec<&Entry> = entries.iter().filter(|e| e.verdict.is_ok() && !e.rotation_safe()).take(30).collect();
    let tied: Vec<&Entry> = unrotated.iter().take_while(|e| e.blocks == unrotated[0].blocks).copied().collect();
    push_leader(&mut s, x, &tied, None, "Not rotation-safe", &mut shown);
    let rows: Vec<String> = unrotated
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let (deg, why) = match &e.rotation {
                Some(Err((deg, why))) => (format!("{deg} degrees"), why.chars().take(110).collect::<String>().replace('|', "/")),
                _ => ("not checked".to_string(), String::new()),
            };
            let yes = |b: bool| if b { "yes" } else { "" };
            format!(
                "| {} | {} | {} | {} | {} | {deg} | {why} | `{}` |\n",
                i + 1,
                e.blocks,
                e.category,
                yes(crate::slime_free(&e.design)),
                yes(crate::single_layer(&e.design)),
                e.source
            )
        })
        .collect();
    push_collapsed(
        &mut s,
        "| # | blocks | category | slime-free | flat | breaks turned | why | source |\n|---|---|---|---|---|---|---|---|\n",
        &rows,
        "| - | - | none |\n",
    );
    // Pareto frontier over (open ticks, close ticks, blocks): the doors no other door beats on
    // all three at once (as good on each, better on one). Designs with entities only count when
    // entities are allowed (category entity has its own scope; every other board bans them).
    for (scope, in_scope) in SCOPES {
        let cands: Vec<&Entry> = entries.iter().filter(|e| e.verdict.is_ok() && e.open_ticks.is_some() && e.close_ticks.is_some() && in_scope(e)).collect();
        let key = |e: &Entry| (e.open_ticks.unwrap(), e.close_ticks.unwrap(), e.blocks);
        let dominated = |a: &Entry| {
            let (ao, ac, ab) = key(a);
            cands.iter().any(|b| {
                let (bo, bc, bb) = key(b);
                bo <= ao && bc <= ac && bb <= ab && (bo < ao || bc < ac || bb < ab)
            })
        };
        let mut front: Vec<&Entry> = cands.iter().copied().filter(|a| !dominated(a)).collect();
        front.sort_by_key(|e| (e.blocks, e.open_ticks, e.close_ticks, e.source.clone()));
        front.dedup_by_key(|e| key(e));
        s.push_str(&format!(
            "## Pareto frontier: open, close, blocks ({scope})\n\nEvery door that no other door beats on open ticks, close ticks and blocks at once \
             (worst cycle; one door per identical triple).\n\n| blocks | open | close | category | slime-free | rotations | source |\n|---|---|---|---|---|---|---|\n"
        ));
        for e in &front {
            let rot = match &e.rotation {
                Some(Ok(())) => "safe".to_string(),
                Some(Err((deg, _))) => format!("breaks at {deg}"),
                None => String::new(),
            };
            let sf = if crate::slime_free(&e.design) { "yes" } else { "" };
            s.push_str(&format!(
                "| {} | {} | {} | {} | {sf} | {rot} | `{}` |\n",
                e.blocks,
                e.open_ticks.unwrap(),
                e.close_ticks.unwrap(),
                e.category,
                e.source
            ));
        }
        s.push('\n');
    }
    // Speed boards: ticks after the lever flip (worst cycle), ties broken by block count.
    let speed: [(&str, Box<dyn Fn(&Entry) -> Option<u32>>); 3] = [
        ("Fastest open", Box::new(|e: &Entry| e.open_ticks)),
        ("Fastest close", Box::new(|e: &Entry| e.close_ticks)),
        ("Fastest open + close", Box::new(|e: &Entry| Some(e.open_ticks? + e.close_ticks?))),
    ];
    for (title, ticks) in &speed {
        for (scope, in_scope) in SCOPES {
            let solid_only = scope == "solid only";
            s.push_str(&format!("## {title} ({scope})\n\nGame ticks after the lever flip (worst of the cycles); ties by block count.\n\n"));
            let mut sorted: Vec<&Entry> = entries.iter().filter(|e| e.verdict.is_ok() && e.rotation_safe() && ticks(e).is_some() && in_scope(e)).collect();
            sorted.sort_by_key(|e| (ticks(e), e.blocks));
            let tied: Vec<&Entry> = sorted.iter().take_while(|e| (ticks(e), e.blocks) == (ticks(sorted[0]), sorted[0].blocks)).copied().collect();
            push_leader(&mut s, x, &tied, None, &format!("{title} ({scope})"), &mut shown);
            let rows: Vec<String> = sorted
                .iter()
                .take(if solid_only { 10 } else { 5 })
                .enumerate()
                .map(|(i, e)| {
                    format!(
                        "| {} | {} | {} | {} | {} | {} | `{}` |\n",
                        i + 1,
                        ticks(e).unwrap(),
                        e.open_ticks.unwrap_or(0),
                        e.close_ticks.unwrap_or(0),
                        e.blocks,
                        e.category,
                        e.source
                    )
                })
                .collect();
            push_collapsed(&mut s, "| # | ticks | open | close | blocks | category | source |\n|---|---|---|---|---|---|---|\n", &rows, "| - | - | no entry yet |\n");
        }
    }
    s.push_str(&format!(
        "## Everything\n\n<details><summary>All {} designs (working and broken) with their verdicts</summary>\n\n| # | blocks | category | slime-free | rotations | verdict | source |\n|---|---|---|---|---|---|---|\n",
        entries.len()
    ));
    for (i, e) in entries.iter().enumerate() {
        let verdict = match &e.verdict {
            Ok(()) => "WORKS".to_string(),
            Err(m) => format!("broken: {}", m.chars().take(90).collect::<String>().replace('|', "/")),
        };
        let sf = if crate::slime_free(&e.design) { "yes" } else { "" };
        let rot = match &e.rotation {
            Some(Ok(())) => "safe".to_string(),
            Some(Err((deg, _))) => format!("breaks at {deg}"),
            None => String::new(),
        };
        s.push_str(&format!("| {} | {} | {} | {sf} | {rot} | {} | `{}` |\n", i + 1, e.blocks, e.category, verdict, e.source));
    }
    s.push_str("\n</details>\n");
    s
}

/// A markdown table showing only its first row; the other rows are in a collapsed `<details>`
/// block (GitHub renders it closed until clicked). `empty` is the row printed when there is none.
fn push_collapsed(s: &mut String, header: &str, rows: &[String], empty: &str) {
    s.push_str(header);
    match rows.first() {
        None => s.push_str(empty),
        Some(first) => s.push_str(first),
    }
    s.push('\n');
    if rows.len() > 1 {
        s.push_str(&format!("<details><summary>{} more</summary>\n\n", rows.len() - 1));
        s.push_str(header);
        for r in &rows[1..] {
            s.push_str(r);
        }
        s.push_str("\n</details>\n\n");
    }
}

/// The Pareto frontier of the working `board` designs over (open ticks, close ticks, blocks), as
/// the leaderboard's frontier boards compute it (worst cycle, every rotation verdict), then each
/// working `check` design placed against it: "ON THE FRONTIER" (no board door is as good on all
/// three and better on one), or the board doors that dominate it. `all` also lists every working
/// board design with its ticks.
pub fn frontier_report(board: &[Entry], checks: &[Entry], solid_only: bool, all: bool) -> String {
    let keep = |e: &&Entry| e.verdict.is_ok() && e.open_ticks.is_some() && e.close_ticks.is_some() && (!solid_only || e.category == "solid");
    let key = |e: &Entry| (e.open_ticks.unwrap(), e.close_ticks.unwrap(), e.blocks);
    let beats = |b: (u32, u32, usize), a: (u32, u32, usize)| b.0 <= a.0 && b.1 <= a.1 && b.2 <= a.2 && b != a;
    let cands: Vec<&Entry> = board.iter().filter(keep).collect();
    let mut s = String::new();
    let row = |e: &Entry| {
        let (o, c, b) = key(e);
        let rot = match &e.rotation {
            Some(Ok(())) => "safe".to_string(),
            Some(Err((deg, _))) => format!("breaks at {deg}"),
            None => "-".to_string(),
        };
        format!("{b:>4} blocks  open {o:>3}  close {c:>3}  {:<7} {rot:<13} {}\n", e.category, e.source)
    };
    let mut front: Vec<&Entry> = cands.iter().copied().filter(|a| !cands.iter().any(|b| beats(key(b), key(a)))).collect();
    front.sort_by_key(|e| (e.blocks, e.open_ticks, e.close_ticks, e.source.clone()));
    front.dedup_by_key(|e| key(e));
    s.push_str(&format!("Pareto frontier ({}), {} working designs:\n", if solid_only { "solid only" } else { "any category" }, cands.len()));
    for e in &front {
        s.push_str(&row(e));
    }
    if all {
        let mut rest = cands.clone();
        rest.sort_by_key(|e| (e.blocks, e.open_ticks, e.close_ticks, e.source.clone()));
        s.push_str("\nEvery working design:\n");
        for e in rest {
            s.push_str(&row(e));
        }
    }
    for e in checks {
        s.push('\n');
        match (&e.verdict, e.open_ticks, e.close_ticks) {
            (Ok(()), Some(_), Some(_)) => {
                s.push_str(&row(e));
                if solid_only && e.category != "solid" {
                    s.push_str("  not solid: off this board\n");
                    continue;
                }
                let doms: Vec<&&Entry> = cands.iter().filter(|b| beats(key(b), key(e))).collect();
                if doms.is_empty() {
                    let ties: Vec<&&Entry> = cands.iter().filter(|b| key(b) == key(e)).collect();
                    s.push_str(&format!("  ON THE FRONTIER{}\n", if ties.is_empty() { String::new() } else { format!(" (ties {})", ties[0].source) }));
                    for b in cands.iter().filter(|b| beats(key(e), key(b))).filter(|b| front.iter().any(|f| f.source == b.source)) {
                        s.push_str(&format!("  pushes off the frontier: {}", row(b)));
                    }
                } else {
                    for b in doms.iter().take(5) {
                        s.push_str(&format!("  dominated by {}", row(b)));
                    }
                }
            }
            (Err(m), _, _) => s.push_str(&format!("{}: BROKEN {}\n", e.source, m.chars().take(200).collect::<String>())),
            _ => s.push_str(&format!("{}: no timings\n", e.source)),
        }
    }
    s
}

/// Copy every `explore/<track>` branch's `designs/piston-door-3x3/tracks/<track>` folder into
/// the working tree (so main carries every agent's variants). Returns the tracks synced.
pub fn sync_tracks(repo: &Path, refs_prefix: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for r in git(repo, &["for-each-ref", "--format=%(refname:short)", refs_prefix])?.lines() {
        let r = r.trim();
        let Some(track) = r.rsplit('/').next() else { continue };
        let path = format!("designs/piston-door-3x3/tracks/{track}");
        if git(repo, &["ls-tree", "--name-only", r, &path]).map(|s| s.trim().is_empty()).unwrap_or(true) {
            continue;
        }
        git(repo, &["checkout", r, "--", &path])?;
        out.push(track.to_string());
    }
    Ok(out)
}

/// Keys of the designs already in an archive folder (verified when they were archived).
pub fn archived_keys(dir: &Path) -> HashSet<String> {
    let mut keys = HashSet::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            // Files kept for history as invalid are not trusted.
            if e.file_name().to_string_lossy().contains("invalid") {
                continue;
            }
            if let Ok(d) = Design::load(&e.path()) {
                keys.insert(d.content_key());
            }
        }
    }
    keys
}
