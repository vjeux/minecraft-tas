//! `mctas door layouts`: how many distinct PISTON LAYOUTS the working doors of one size use.
//!
//! The doors: every design on HEAD, `--base` and the `explore/*` branches (as the leaderboard
//! gathers them) that the leaderboard's caches judge WORKS (`verdicts-v5.tsv`) with the given
//! volume dims (`properties-v1.tsv`, e.g. `7x8x1`). A door's layout is the set of its pistons and
//! sticky pistons at rest: position relative to the door's bottom-left cell along the door's own
//! axes, facing, sticky; wiring, door material, observers and everything else are ignored.
//! Identical layouts are grouped and left-right mirrors paired (for a flat door, a mirror is the
//! same layout built from behind). Per layout: doors, best blocks, best open/close, an example, and
//! its first find: the earliest commit (author time) that carried the exact file of any of its
//! doors, and the track of that commit's path.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};
use mctas::design::Design;

use crate::leaderboard::{cat_file_batch, design_hash, git};

/// One piston at rest: (x along the door's width, y, depth) relative to the door's bottom-left
/// cell, its facing in those axes (E/W along the width, U/D, N/S along the depth), sticky.
pub type Piece = (i32, i32, i32, char, bool);

fn prop<'a>(state: &'a str, key: &str) -> Option<&'a str> {
    let inner = state.split_once('[')?.1.trim_end_matches(']');
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim()))
}

/// A door's pistons at rest (sorted), and how many of them rest extended.
pub fn layout(d: &Design) -> Option<(Vec<Piece>, usize)> {
    let door = *d.ports.get("door")?;
    let along_z = d.meta.get("door-axis").is_some_and(|v| v.trim() == "z");
    let mut v = Vec::new();
    let mut extended = 0;
    for (p, st) in &d.blocks {
        let sticky = match st.split('[').next().unwrap_or(st) {
            "minecraft:sticky_piston" => true,
            "minecraft:piston" => false,
            _ => continue,
        };
        let (x, y, w) = if along_z { (p[2] - door[2], p[1] - door[1], p[0] - door[0]) } else { (p[0] - door[0], p[1] - door[1], p[2] - door[2]) };
        // the door's width axis is x (or z for `door-axis: z`), its depth axis z (or x)
        let f = match (prop(st, "facing").unwrap_or("north"), along_z) {
            ("up", _) => 'U',
            ("down", _) => 'D',
            ("east", false) | ("south", true) => 'E',
            ("west", false) | ("north", true) => 'W',
            ("south", false) | ("east", true) => 'S',
            _ => 'N',
        };
        if prop(st, "extended") == Some("true") {
            extended += 1;
        }
        v.push((x, y, w, f, sticky));
    }
    v.sort();
    Some((v, extended))
}

/// The left-right mirror of a layout (x -> 2 - x across the 3-wide door, east <-> west).
pub fn mirror(l: &[Piece]) -> Vec<Piece> {
    let mut v: Vec<Piece> = l
        .iter()
        .map(|&(x, y, w, f, s)| {
            let f = match f {
                'E' => 'W',
                'W' => 'E',
                o => o,
            };
            (2 - x, y, w, f, s)
        })
        .collect();
    v.sort();
    v
}

/// The up-down mirror of a layout (y -> 2 - y across the 3-tall door, up <-> down).
pub fn flip(l: &[Piece]) -> Vec<Piece> {
    let mut v: Vec<Piece> = l
        .iter()
        .map(|&(x, y, w, f, s)| {
            let f = match f {
                'U' => 'D',
                'D' => 'U',
                o => o,
            };
            (x, 2 - y, w, f, s)
        })
        .collect();
    v.sort();
    v
}

/// Compact text of a layout: `s(-1,0)E p(3,1)W ...` (s = sticky, p = normal; depth shown when
/// not 0).
pub fn layout_text(l: &[Piece]) -> String {
    l.iter()
        .map(|&(x, y, w, f, s)| {
            let k = if s { 's' } else { 'p' };
            if w == 0 { format!("{k}({x},{y}){f}") } else { format!("{k}({x},{y},{w}){f}") }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Where each design file first appeared over the history of `refs`: blob id -> (author time,
/// date `YYYY-MM-DD HH:MMZ`, path).
fn first_seen(repo: &Path, refs: &[String]) -> Result<HashMap<String, (i64, String, String)>> {
    let mut args: Vec<String> = vec!["-C".into(), repo.display().to_string(), "log".into()];
    args.extend(refs.iter().cloned());
    args.extend(["--raw", "--no-abbrev", "--no-renames", "--date=format-local:%Y-%m-%d %H:%MZ", "--format=%x01%at%x09%ad", "--", "designs/piston-door-3x3"].map(String::from));
    let out = Command::new("git").args(&args).env("TZ", "UTC").output()?;
    if !out.status.success() {
        bail!("git log: {}", String::from_utf8_lossy(&out.stderr).trim());
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut m: HashMap<String, (i64, String, String)> = HashMap::new();
    let mut cur = (0i64, String::new());
    for line in text.lines() {
        if let Some(h) = line.strip_prefix('\u{1}') {
            let (t, d) = h.split_once('\t').unwrap_or((h, ""));
            cur = (t.trim().parse().unwrap_or(0), d.to_string());
            continue;
        }
        // :old_mode new_mode old_blob new_blob status<TAB>path
        let Some((meta, path)) = line.strip_prefix(':').and_then(|l| l.split_once('\t')) else { continue };
        let f: Vec<&str> = meta.split_whitespace().collect();
        if f.len() < 5 || f[4].starts_with('D') {
            continue;
        }
        let e = m.entry(f[3].to_string()).or_insert((i64::MAX, String::new(), String::new()));
        if cur.0 < e.0 {
            *e = (cur.0, cur.1.clone(), path.to_string());
        }
    }
    Ok(m)
}

/// The track a design path belongs to: `tracks/<t>/...` -> t; an archive copy -> the track of the
/// path it was archived from (its `# archived ... from <ref>:<path>` header line), marked
/// `(archive)`.
fn track_of(path: &str, text: Option<&str>) -> String {
    let from_tracks = |p: &str| p.split_once("/tracks/").map(|(_, r)| r.split('/').next().unwrap_or(r).to_string());
    if let Some(t) = from_tracks(path) {
        return t;
    }
    let archived = text.and_then(|t| t.lines().take(8).find_map(|l| l.split_once(" from ").filter(|_| l.starts_with("# archived")).map(|(_, r)| r.to_string())));
    match archived.as_deref().and_then(from_tracks) {
        Some(t) => format!("{t} (archive)"),
        None => path.trim_start_matches("designs/piston-door-3x3/").split('/').next().unwrap_or(path).to_string(),
    }
}

/// One working door of the asked size.
struct Door {
    source: String,
    blocks: usize,
    open: Option<u32>,
    close: Option<u32>,
    category: &'static str,
    /// (author time, date, track) of the first commit carrying one of its files.
    first: Option<(i64, String, String)>,
    extended: usize,
}

pub struct Options<'a> {
    pub repo: &'a Path,
    /// Ref prefix scanned besides HEAD and `base` (the leaderboard's: `refs/remotes/origin/explore/`).
    pub refs: &'a str,
    pub base: &'a str,
    /// The leaderboard's cache folder (verdicts-v5.tsv, properties-v1.tsv).
    pub cache_dir: &'a Path,
    /// Volume dims, `7x8x1`.
    pub size: &'a str,
    /// Only this category (`solid`, ...); None = every category.
    pub category: Option<&'a str>,
}

/// The working doors of the asked size, with their layouts.
fn doors(o: &Options) -> Result<(Vec<(Vec<Piece>, Door)>, usize)> {
    let mut refs = vec!["HEAD".to_string()];
    if !o.base.is_empty() {
        refs.push(o.base.to_string());
    }
    for line in git(o.repo, &["for-each-ref", "--format=%(refname:short)", o.refs])?.lines() {
        refs.push(line.trim().to_string());
    }
    // (ref, path, blob) of every design file
    let mut items: Vec<(String, String, String)> = Vec::new();
    for r in &refs {
        let Ok(list) = git(o.repo, &["ls-tree", "-r", r, "designs/piston-door-3x3"]) else { continue };
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
    let texts = cat_file_batch(o.repo, &blobs)?;
    // design hash -> (first source, design, every blob carrying it)
    let mut by_hash: HashMap<u64, (String, Design, Vec<String>)> = HashMap::new();
    for (r, path, blob) in &items {
        let Some(text) = texts.get(blob) else { continue };
        let Ok(d) = Design::parse(text) else { continue };
        if !d.ports.contains_key("door") || !d.ports.contains_key("lever") {
            continue;
        }
        let e = by_hash.entry(design_hash(&d)).or_insert_with(|| (format!("{r}:{path}"), d, Vec::new()));
        e.2.push(blob.clone());
    }
    let mut cands: Vec<(String, Design)> = by_hash.values().map(|(s, d, _)| (s.clone(), d.clone())).collect();
    cands.sort_by(|a, b| a.0.cmp(&b.0));
    let (entries, uncached) = crate::leaderboard::cached_entries(cands, o.cache_dir);
    let seen = first_seen(o.repo, &refs)?;
    let mut out = Vec::new();
    for e in entries {
        if e.verdict.is_err() || e.volume_dims.as_deref() != Some(o.size) || o.category.is_some_and(|c| c != e.category) {
            continue;
        }
        let Some((l, extended)) = layout(&e.design) else { continue };
        let blobs = &by_hash[&design_hash(&e.design)].2;
        let first = blobs
            .iter()
            .filter_map(|b| seen.get(b).map(|(t, d, p)| (*t, d.clone(), track_of(p, texts.get(b).map(|s| s.as_str())))))
            .min_by_key(|x| x.0);
        out.push((l, Door { source: e.source.clone(), blocks: e.blocks, open: e.open_ticks, close: e.close_ticks, category: e.category, first, extended }));
    }
    Ok((out, uncached.len()))
}

/// The report: a summary, one table row per layout, mirror pairs, and each layout's pistons.
pub fn report(o: &Options) -> Result<String> {
    let (doors, uncached) = doors(o)?;
    let mut groups: BTreeMap<Vec<Piece>, Vec<Door>> = BTreeMap::new();
    for (l, d) in doors {
        groups.entry(l).or_default().push(d);
    }
    // rank: best blocks, then more doors, then the earlier find
    let best = |ds: &[Door]| ds.iter().map(|d| d.blocks).min().unwrap_or(usize::MAX);
    let found = |ds: &[Door]| ds.iter().filter_map(|d| d.first.as_ref().map(|f| f.0)).min().unwrap_or(i64::MAX);
    let mut order: Vec<(&Vec<Piece>, &Vec<Door>)> = groups.iter().collect();
    order.sort_by_key(|(_, ds)| (best(ds), std::cmp::Reverse(ds.len()), found(ds)));
    let index: HashMap<&Vec<Piece>, usize> = order.iter().enumerate().map(|(i, (l, _))| (*l, i + 1)).collect();
    let n_doors: usize = groups.values().map(|v| v.len()).sum();
    let mut s = String::new();
    let cat = o.category.unwrap_or("every category");
    s.push_str(&format!("{} distinct piston layouts among {n_doors} working {} doors ({cat}; HEAD + {} + {}*; verdicts and volumes from the leaderboard caches",
        groups.len(), o.size, o.base, o.refs));
    if uncached > 0 {
        s.push_str(&format!(", {uncached} designs without a cached verdict skipped"));
    }
    s.push_str(").\n\n");
    // mirrors
    let mut pairs = Vec::new();
    let mut symmetric = 0;
    for (l, i) in &index {
        let m = mirror(l);
        if &&m == l {
            symmetric += 1;
        } else if let Some(j) = index.get(&m).filter(|j| i < j) {
            pairs.push((*i, *j));
        }
    }
    pairs.sort();
    let flips = index.iter().filter(|(l, i)| index.get(&flip(l)).is_some_and(|j| *i < j)).count();
    s.push_str(&format!(
        "Up to left-right mirror: {} layouts ({} mirror pairs, {symmetric} symmetric). Up-down mirror pairs (FYI): {flips}.\n\n",
        groups.len() - pairs.len(),
        pairs.len()
    ));
    // cores: a layout that contains another found layout (plus extra pistons, e.g. pistons used as
    // wiring parts in free cells) is listed under the smallest one it contains
    let core_of = |l: &Vec<Piece>| -> usize {
        order.iter().filter(|(c, _)| c.len() < l.len() && c.iter().all(|p| l.binary_search(p).is_ok())).min_by_key(|(c, _)| c.len()).map(|(c, _)| index[*c]).unwrap_or(index[l])
    };
    let cores: Vec<usize> = order.iter().map(|(l, _)| core_of(l)).collect();
    let mut core_ids: Vec<usize> = cores.clone();
    core_ids.sort();
    core_ids.dedup();
    let core_line: Vec<String> = core_ids
        .iter()
        .map(|&c| {
            let members: Vec<usize> = (0..order.len()).filter(|i| cores[*i] == c).collect();
            let n: usize = members.iter().map(|i| order[*i].1.len()).sum();
            let (l, _) = order[c - 1];
            format!("L{c} ({} pistons, {} sticky): {} layouts, {n} doors", l.len(), l.iter().filter(|p| p.4).count(), members.len())
        })
        .collect();
    s.push_str(&format!(
        "Core layouts (containing no other found layout; the rest are a core plus extra pistons): {} — {}.\n\n",
        core_ids.len(),
        core_line.join("; ")
    ));
    s.push_str("| layout | core | pistons (sticky) | doors | best blocks | best open/close | categories | first found | example |\n|---|---|---|---|---|---|---|---|---|\n");
    for (i, (l, ds)) in order.iter().enumerate() {
        let c = cores[i];
        let core = if c == i + 1 { "core".to_string() } else { format!("L{c} + {}", l.len() - order[c - 1].0.len()) };
        s.push_str(&row(i + 1, &core, l, ds));
    }
    if !pairs.is_empty() {
        s.push_str(&format!("\nMirror pairs: {}\n", pairs.iter().map(|(a, b)| format!("L{a} = mirror of L{b}")).collect::<Vec<_>>().join(", ")));
    }
    s.push_str("\nLayouts (x along the door's width from its bottom-left cell, y up; s = sticky, p = normal; facing E/W/U/D):\n");
    for (i, (l, ds)) in order.iter().enumerate() {
        let ext: Vec<usize> = { let mut v: Vec<usize> = ds.iter().map(|d| d.extended).collect(); v.sort(); v.dedup(); v };
        s.push_str(&format!("- L{}: {} (extended at rest: {ext:?})\n", i + 1, layout_text(l)));
    }
    Ok(s)
}

/// One table row of a layout.
fn row(i: usize, core: &str, l: &[Piece], ds: &[Door]) -> String {
    let sticky = l.iter().filter(|p| p.4).count();
    let b = ds.iter().min_by_key(|d| (d.blocks, d.open.unwrap_or(999) + d.close.unwrap_or(999))).unwrap();
    let fast = ds.iter().filter(|d| d.open.is_some()).min_by_key(|d| (d.open.unwrap_or(999) + d.close.unwrap_or(999), d.blocks));
    let oc = fast.map(|d| format!("{}/{}", d.open.unwrap_or(0), d.close.unwrap_or(0))).unwrap_or_else(|| "-".into());
    let mut cats: Vec<&str> = ds.iter().map(|d| d.category).collect();
    cats.sort();
    cats.dedup();
    let first = ds.iter().filter_map(|d| d.first.clone()).min_by_key(|f| f.0).map(|(_, d, t)| format!("{t}, {d}")).unwrap_or_else(|| "-".into());
    format!("| L{i} | {core} | {} ({sticky}) | {} | {} | {oc} | {} | {first} | {} |\n", l.len(), ds.len(), b.blocks, cats.join(", "), b.source)
}
