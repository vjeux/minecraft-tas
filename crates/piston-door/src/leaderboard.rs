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

/// Verify every candidate in parallel; designs whose key is in `trusted` (already archived,
/// hence verified when archived) are not re-verified.
pub fn judge(version: &str, cands: Vec<(String, Design)>, extra_offsets: u32, workers: usize, trusted: &HashSet<String>) -> Result<Vec<Entry>> {
    let pool = Pool::spawn(workers.min(cands.len()).max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let mut opts = VerifyOptions::default();
    opts.offsets.extend(crate::extra_offsets(extra_offsets));
    let results = pool.map(cands, |w, _, (source, d)| {
        let key = format!("{:?}|{:?}", d.blocks, d.ports);
        let verdict = if trusted.contains(&key) {
            Ok(())
        } else {
            crate::verify(w, &d, &opts).map(|_| ()).map_err(|e| format!("{e:#}"))
        };
        Ok(Entry { source, blocks: block_count(&d), category: category(&d), verdict, design: d })
    });
    let mut entries: Vec<Entry> = results.into_iter().collect::<Result<_>>()?;
    entries.sort_by(|a, b| {
        (a.verdict.is_err(), a.category, a.blocks, a.source.as_str()).cmp(&(b.verdict.is_err(), b.category, b.blocks, b.source.as_str()))
    });
    Ok(entries)
}

/// Markdown table of the entries.
pub fn render(entries: &[Entry], extra_offsets: u32) -> String {
    let mut s = format!(
        "# 3x3 piston door leaderboard\n\nEvery design on main and on the `explore/*` branches, verified with main's verifier \
         (3 cycles at {} positions). Working designs first, by category then block count.\n\n\
         | # | blocks | category | verdict | source |\n|---|---|---|---|---|\n",
        4 + extra_offsets
    );
    for (i, e) in entries.iter().enumerate() {
        let verdict = match &e.verdict {
            Ok(()) => "WORKS".to_string(),
            Err(m) => format!("broken: {}", m.chars().take(90).collect::<String>().replace('|', "/")),
        };
        s.push_str(&format!("| {} | {} | {} | {} | `{}` |\n", i + 1, e.blocks, e.category, verdict, e.source));
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
            if let Ok(d) = Design::load(&e.path()) {
                keys.insert(format!("{:?}|{:?}", d.blocks, d.ports));
            }
        }
    }
    keys
}
