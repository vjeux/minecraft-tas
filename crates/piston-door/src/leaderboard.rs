//! `mctas door leaderboard`: gather every door design on main (HEAD) and on the agents'
//! `explore/*` branches, verify each one with the judge (main's verifier, extra offsets), and
//! write a ranked table. Identical designs are verified once; the first source is kept.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use anyhow::{Result, bail};
use mctas::design::Design;
use mctas::sim::{Pool, WorkerConfig};

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
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output()?;
    if !out.status.success() {
        bail!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Candidate designs as (source label, design): every `.txt` under designs/piston-door-3x3 on
/// HEAD and on every ref under `refs_prefix`, parsed, with door+lever ports, deduplicated.
pub fn collect(repo: &Path, refs_prefix: &str, max_blocks: usize) -> Result<Vec<(String, Design)>> {
    let mut refs = vec!["HEAD".to_string()];
    for line in git(repo, &["for-each-ref", "--format=%(refname:short)", refs_prefix])?.lines() {
        refs.push(line.trim().to_string());
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in refs {
        let Ok(files) = git(repo, &["ls-tree", "-r", "--name-only", &r, "designs/piston-door-3x3"]) else { continue };
        for f in files.lines().filter(|f| f.ends_with(".txt")) {
            let Ok(text) = git(repo, &["show", &format!("{r}:{f}")]) else { continue };
            let Ok(d) = Design::parse(&text) else { continue };
            if !d.ports.contains_key("door") || !d.ports.contains_key("lever") || block_count(&d) > max_blocks {
                continue;
            }
            let key = format!("{:?}|{:?}", d.blocks, d.ports);
            if seen.insert(key) {
                out.push((format!("{r}:{f}"), d));
            }
        }
    }
    Ok(out)
}

/// Content hash of a design (blocks + ports), stable across runs (FNV-1a 64).
pub fn design_hash(d: &Design) -> u64 {
    let key = format!("{:?}|{:?}", d.blocks, d.ports);
    let mut h: u64 = 0xcbf29ce484222325;
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Verdict cache: design hash -> (verdict, open ticks, close ticks). Bump the file name when the
/// rules change so old verdicts are dropped.
const CACHE_FILE: &str = "verdicts-v2.tsv";
type Cached = (std::result::Result<(), String>, Option<u32>, Option<u32>);

fn load_cache(dir: &Path) -> std::collections::HashMap<u64, Cached> {
    let mut m = std::collections::HashMap::new();
    let Ok(text) = std::fs::read_to_string(dir.join(CACHE_FILE)) else { return m };
    for line in text.lines() {
        let f: Vec<&str> = line.splitn(5, '\t').collect();
        if f.len() < 5 {
            continue;
        }
        let Ok(h) = u64::from_str_radix(f[0], 16) else { continue };
        let verdict = if f[1] == "ok" { Ok(()) } else { Err(f[4].to_string()) };
        m.insert(h, (verdict, f[2].parse().ok(), f[3].parse().ok()));
    }
    m
}

fn save_cache(dir: &Path, m: &std::collections::HashMap<u64, Cached>) -> Result<()> {
    let mut lines: Vec<String> = m
        .iter()
        .map(|(h, (v, o, c))| {
            let (tag, err) = match v {
                Ok(()) => ("ok", String::new()),
                Err(e) => ("broken", e.replace(['\t', '\n'], " ")),
            };
            let num = |x: &Option<u32>| x.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
            format!("{h:016x}\t{tag}\t{}\t{}\t{err}", num(o), num(c))
        })
        .collect();
    lines.sort();
    std::fs::write(dir.join(CACHE_FILE), lines.join("\n") + "\n")?;
    Ok(())
}

/// Verify every candidate in parallel. With `cache_dir`, verdicts and timings are cached by
/// design content (main's verifier is deterministic), so only new designs are simulated.
pub fn judge(version: &str, cands: Vec<(String, Design)>, extra_offsets: u32, workers: usize, cache_dir: Option<&Path>) -> Result<Vec<Entry>> {
    let mut cache = cache_dir.map(load_cache).unwrap_or_default();
    let (hit, miss): (Vec<_>, Vec<_>) = cands.into_iter().partition(|(_, d)| cache.contains_key(&design_hash(d)));
    eprintln!("{} cached verdicts, {} designs to verify", hit.len(), miss.len());
    let mut entries: Vec<Entry> = hit
        .into_iter()
        .map(|(source, d)| {
            let (verdict, open_ticks, close_ticks) = cache[&design_hash(&d)].clone();
            Entry { source, blocks: block_count(&d), category: category(&d), verdict, design: d, open_ticks, close_ticks }
        })
        .collect();
    if !miss.is_empty() {
        let pool = Pool::spawn(workers.min(miss.len()).max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
        let mut opts = VerifyOptions::default();
        opts.offsets.extend(crate::extra_offsets(extra_offsets));
        let results = pool.map(miss, |w, _, (source, d)| {
            let (verdict, open_ticks, close_ticks) = match crate::verify(w, &d, &opts) {
                Ok(r) => (
                    Ok(()),
                    r.cycles.iter().map(|c| c.open_ticks).max(),
                    r.cycles.iter().map(|c| c.close_ticks).max(),
                ),
                Err(e) => (Err(format!("{e:#}")), None, None),
            };
            Ok(Entry { source, blocks: block_count(&d), category: category(&d), verdict, design: d, open_ticks, close_ticks })
        });
        for e in results.into_iter().collect::<Result<Vec<Entry>>>()? {
            cache.insert(design_hash(&e.design), (e.verdict.clone(), e.open_ticks, e.close_ticks));
            entries.push(e);
        }
    }
    if let Some(dir) = cache_dir {
        save_cache(dir, &cache)?;
    }
    entries.sort_by(|a, b| {
        let rank = |e: &Entry| (e.verdict.is_err(), e.category != "solid", e.blocks);
        (rank(a), a.source.as_str()).cmp(&(rank(b), b.source.as_str()))
    });
    Ok(entries)
}

/// Markdown table of the entries.
pub fn render(entries: &[Entry], extra_offsets: u32) -> String {
    let mut s = format!(
        "# 3x3 piston door leaderboards\n\nEvery design on main and on the `explore/*` branches, verified with main's verifier \
         (3 cycles at {} positions). Videos of every archived design: [archive/VIDEOS.md](archive/VIDEOS.md).\n\n",
        4 + extra_offsets
    );
    let boards: [(&str, &str, Box<dyn Fn(&Entry) -> bool>); 4] = [
        ("Solid (main ranking)", "9 identical plain door blocks, no sculk sensors", Box::new(|e: &Entry| e.category == "solid")),
        ("Solid, no slime or honey", "same, and not a single slime or honey block anywhere", Box::new(|e: &Entry| e.category == "solid" && crate::slime_free(&e.design))),
        ("Sculk", "uses sculk sensors (footsteps or mobs nearby can trigger them)", Box::new(|e: &Entry| e.category == "sculk")),
        ("Gravity", "the 9 door blocks are sand, gravel or concrete powder", Box::new(|e: &Entry| e.category == "gravity")),
    ];
    for (title, rule, keep) in &boards {
        s.push_str(&format!("## {title}\n\n{rule}.\n\n| # | blocks | source |\n|---|---|---|\n"));
        let mut n = 0;
        for e in entries.iter().filter(|e| e.verdict.is_ok() && keep(e)) {
            n += 1;
            if n > 10 {
                break;
            }
            s.push_str(&format!("| {n} | {} | `{}` |\n", e.blocks, e.source));
        }
        if n == 0 {
            s.push_str("| - | - | no entry yet |\n");
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
        for (scope, solid_only) in [("any category", false), ("solid only", true)] {
            s.push_str(&format!(
                "## {title} ({scope})\n\nGame ticks after the lever flip (worst of the cycles); ties by block count.\n\n\
                 | # | ticks | open | close | blocks | category | source |\n|---|---|---|---|---|---|---|\n"
            ));
            let mut rows: Vec<&Entry> = entries
                .iter()
                .filter(|e| e.verdict.is_ok() && ticks(e).is_some() && (!solid_only || e.category == "solid"))
                .collect();
            rows.sort_by_key(|e| (ticks(e), e.blocks));
            for (i, e) in rows.iter().take(if solid_only { 10 } else { 5 }).enumerate() {
                s.push_str(&format!(
                    "| {} | {} | {} | {} | {} | {} | `{}` |\n",
                    i + 1,
                    ticks(e).unwrap(),
                    e.open_ticks.unwrap_or(0),
                    e.close_ticks.unwrap_or(0),
                    e.blocks,
                    e.category,
                    e.source
                ));
            }
            s.push('\n');
        }
    }
    s.push_str("## Everything\n\n| # | blocks | category | slime-free | verdict | source |\n|---|---|---|---|---|---|\n");
    for (i, e) in entries.iter().enumerate() {
        let verdict = match &e.verdict {
            Ok(()) => "WORKS".to_string(),
            Err(m) => format!("broken: {}", m.chars().take(90).collect::<String>().replace('|', "/")),
        };
        let sf = if crate::slime_free(&e.design) { "yes" } else { "" };
        s.push_str(&format!("| {} | {} | {} | {sf} | {} | `{}` |\n", i + 1, e.blocks, e.category, verdict, e.source));
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
                keys.insert(format!("{:?}|{:?}", d.blocks, d.ports));
            }
        }
    }
    keys
}
