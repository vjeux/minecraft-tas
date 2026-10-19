//! The circuits (the contraption studies, docs/circuits.md): every design of
//! `designs/circuits/<study>/archive/` (the judge's archive, as the studies fill it) and of
//! `designs/circuits/calibration/` (the judge's calibration circuits, verified in the real game),
//! with its kind, its study's axis and metrics. A verdict line recorded in the design
//! (`# CIRCUIT: ...`) or in the archive's `verdicts-v1.tsv` (`file` and `summary` columns) is the
//! judge's; otherwise the metrics come from the redsim screen (`mctas circuit sim`), run here.

use anyhow::Result;
use circuit::Circuit as Design;
use circuit::verify::{Options, sim};
use site_model::{Build, Circuit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ROOT: &str = "designs/circuits";

/// The design files to export: (study folder name, file path, from the calibration set).
fn sources(repo: &Path) -> Vec<(String, PathBuf, bool)> {
    let mut out = Vec::new();
    let root = repo.join(ROOT);
    let Ok(rd) = std::fs::read_dir(&root) else { return out };
    let mut dirs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for d in dirs {
        let name = d.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (folder, calibration) = if name == "calibration" { (d.clone(), true) } else { (d.join("archive"), false) };
        let Ok(rd) = std::fs::read_dir(&folder) else { continue };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        files.sort();
        for f in files {
            out.push((name.clone(), f, calibration));
        }
    }
    out
}

/// `file -> summary` from an archive's verdicts-v1.tsv (header row with `file` and `summary`).
fn verdicts(folder: &Path) -> BTreeMap<String, String> {
    let Ok(text) = std::fs::read_to_string(folder.join("verdicts-v1.tsv")) else { return BTreeMap::new() };
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let (Some(fi), Some(si)) = (header.iter().position(|h| *h == "file"), header.iter().position(|h| *h == "summary")) else { return BTreeMap::new() };
    lines.filter_map(|l| {
        let f: Vec<&str> = l.split('\t').collect();
        Some((f.get(fi)?.to_string(), f.get(si)?.to_string()))
    })
    .collect()
}

fn captions(folder: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(folder.join("captions.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(f, _)| *f != "file")
        .map(|(f, c)| (f.to_string(), c.trim().to_string()))
        .collect()
}

/// The design's own description: its comment lines, minus the archive's bookkeeping ones.
fn comment_caption(text: &str) -> String {
    text.lines()
        .filter_map(|l| l.trim().strip_prefix('#').map(str::trim))
        .filter(|l| !(l.starts_with("archived") || l.starts_with("verified") || l.starts_with("CIRCUIT:") || l.starts_with("FAIL:") || l.is_empty()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The metrics of a `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, <verdict>` line:
/// the comma-separated `name value` parts between the kind and the block count.
pub fn parse_verdict_metrics(line: &str, kind: &str) -> Vec<(String, String)> {
    let Some(rest) = line.trim().strip_prefix("CIRCUIT:") else { return Vec::new() };
    let Some(i) = rest.find(&format!(" {kind} ")) else { return Vec::new() };
    let body = &rest[i + kind.len() + 2..];
    let mut out = Vec::new();
    for part in body.split(", ") {
        let part = part.trim();
        if part.ends_with(" blocks") || part.chars().next().is_some_and(|c| c.is_ascii_digit()) && part.contains('x') && !part.contains(' ') {
            break;
        }
        match part.split_once(' ') {
            Some((k, v)) => out.push((k.to_string(), v.to_string())),
            None => out.push((part.to_string(), String::new())),
        }
    }
    out
}

/// The first number in a metric value (`4 gt` -> 4).
fn number(v: &str) -> Option<i64> {
    let s: String = v.trim().chars().skip_while(|c| !c.is_ascii_digit() && *c != '-').take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    s.parse().ok()
}

pub fn export(repo: &Path) -> Result<(Vec<Build>, Vec<String>)> {
    let mut builds = Vec::new();
    let mut notes = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut folder_cache: BTreeMap<PathBuf, (BTreeMap<String, String>, BTreeMap<String, String>)> = BTreeMap::new();
    for (study_dir, path, calibration) in sources(repo) {
        let folder = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let (verd, caps) = folder_cache.entry(folder.clone()).or_insert_with(|| (verdicts(&folder), captions(&folder))).clone();
        let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let stem = file.trim_end_matches(".txt").to_string();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let c = match Design::load(&path) {
            Ok(c) => c,
            Err(e) => {
                notes.push(format!("{}: {e:#}", path.display()));
                continue;
            }
        };
        let Some(kind) = c.kind.clone() else {
            notes.push(format!("{}: no `circuit:` kind", path.display()));
            continue;
        };
        let kind_name = kind.name();
        let study = kind.study().to_string();
        // the judge's line (recorded), else the redsim screen's
        let recorded = verd.get(&file).cloned().or_else(|| text.lines().take(6).find_map(|l| l.trim().strip_prefix("# ").filter(|r| r.starts_with("CIRCUIT:")).map(String::from)));
        let (metrics, verdict) = match &recorded {
            Some(line) => (parse_verdict_metrics(line, &kind_name), line.clone()),
            None => match sim(&c, &Options::default()) {
                Ok(r) => {
                    let line = r.summary(&c);
                    let m = r.outcome.as_ref().filter(|o| o.pass).map(|o| o.metrics.clone()).unwrap_or_default();
                    if r.outcome.as_ref().is_some_and(|o| !o.pass) {
                        notes.push(format!("{}: redsim screen fails: {}", path.display(), r.outcome.as_ref().map(|o| o.why.clone()).unwrap_or_default()));
                    }
                    (m, format!("{line} (redsim screen at site build time)"))
                }
                Err(e) => (Vec::new(), format!("redsim screen failed: {e:#}")),
            },
        };
        let axis = metrics.first().map(|(k, v)| if v.is_empty() { k.clone() } else { format!("{k} {v}") }).unwrap_or_else(|| kind_name.clone());
        let value = metrics.first().and_then(|(_, v)| number(v));
        let sub = if calibration { "calibration".to_string() } else { study_dir.clone() };
        let id = format!("circuits/{sub}/{stem}");
        if !seen.insert(id.clone()) {
            continue;
        }
        let (lo, hi) = c.bounds();
        let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
        let mut flags: Vec<String> = Vec::new();
        if calibration {
            flags.push("calibration".into());
        }
        if verdict.contains(", robust") {
            flags.push("robust".into());
        }
        if metrics.iter().any(|(k, v)| k == "latency" && number(v) == Some(0)) || verdict.contains("instant") {
            flags.push("instant".into());
        }
        let inputs: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
        let outputs: Vec<String> = c.outputs.iter().map(|o| o.name.clone()).collect();
        builds.push(Build {
            id: id.clone(),
            kind: "circuit".into(),
            group: kind_name.clone(),
            name: stem.clone(),
            page: format!("{id}.html"),
            design: format!("designs/circuits/{sub}/{file}"),
            blocks: c.blocks() as u32,
            volume: Some((d[0] * d[1] * d[2]).max(0) as u32),
            dims: Some(format!("{}x{}x{}", d[0], d[1], d[2])),
            kinds: c.kinds().into_iter().collect(),
            caption: caps.get(&file).cloned().unwrap_or_else(|| comment_caption(&text)),
            flags,
            door: None,
            clock: None,
            circuit: Some(Circuit { study, kind: kind_name, axis, value, metrics, verdict, inputs, outputs }),
            trace: None,
            picture: None,
        });
    }
    builds.sort_by(|a, b| (&a.group, a.blocks, &a.name).cmp(&(&b.group, b.blocks, &b.name)));
    Ok((builds, notes))
}

/// The repository path of a circuit build's design (for the GitHub link and the page's text).
pub fn repo_path(b: &Build) -> String {
    // designs/circuits/<sub>/<file> on the site; calibration designs live in the same place in
    // the repository, archived ones under <study>/archive/
    let rest = b.design.trim_start_matches("designs/circuits/");
    match rest.split_once('/') {
        Some(("calibration", f)) => format!("{ROOT}/calibration/{f}"),
        Some((study, f)) => format!("{ROOT}/{study}/archive/{f}"),
        None => b.design.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_metrics() {
        let m = parse_verdict_metrics("CIRCUIT: sub-rising-4 rising-edge L 2 gt, delay 1 gt, min-pulse 1 gt, 5 blocks, 3x2x1, robust", "rising-edge");
        assert_eq!(m, vec![("L".into(), "2 gt".into()), ("delay".into(), "1 gt".into()), ("min-pulse".into(), "1 gt".into())]);
        let g = parse_verdict_metrics("CIRCUIT: cal-torch-not gate not fn not, latency 2 gt, glitches 0, 3 blocks, 4x2x1, robust", "gate not");
        assert_eq!(g[0], ("fn".into(), "not".into()));
        assert_eq!(g.len(), 3);
        assert!(parse_verdict_metrics("FAIL: x delay: no", "delay").is_empty());
    }

    #[test]
    fn numbers_and_captions() {
        assert_eq!(number("4 gt"), Some(4));
        assert_eq!(number("not"), None);
        assert_eq!(comment_caption("# archived 2026\nname: x\n# Input -> dust -> torch.\n"), "Input -> dust -> torch.");
    }
}
