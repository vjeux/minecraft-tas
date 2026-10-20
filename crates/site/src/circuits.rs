//! The circuits (the contraption studies, docs/circuits.md): every design the judge archived in
//! `designs/circuits/<study>/archive/` (its `verdicts.tsv` row: kind, axis, metrics, robustness,
//! the judge line; `captions.tsv`), with the variants folded as `mctas circuit catalog` folds them
//! (within one kind and axis value, a design that only adds block kinds to a smaller one), plus
//! the judge's calibration circuits in `designs/circuits/calibration/` (metrics from the redsim
//! screen, `mctas circuit sim`, run here).

use anyhow::Result;
use circuit::Circuit as Design;
use circuit::archive::{STUDIES, archive_dir, read_rows};
use circuit::verify::{Options, sim};
use site_model::{Build, Circuit};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ROOT: &str = "designs/circuits";

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

/// `L 2 gt, delay 1 gt, min-pulse 1 gt` -> [(L, 2 gt), (delay, 1 gt), (min-pulse, 1 gt)].
pub fn parse_metrics(s: &str) -> Vec<(String, String)> {
    s.split(", ")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once(' ') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (p.to_string(), String::new()),
        })
        .collect()
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

/// The first number in a text (`L = 4 gt` -> 4).
fn number(v: &str) -> Option<i64> {
    let s: String = v.trim().chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    s.parse().ok()
}

/// What a design file adds to its row: block kinds, input and output names, its box.
struct Parsed {
    kinds: Vec<String>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    dims: Option<(String, u32)>,
}

fn parse(path: &Path) -> Option<(Design, Parsed)> {
    let c = Design::load(path).ok()?;
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let p = Parsed {
        kinds: c.kinds().into_iter().collect(),
        inputs: c.inputs.iter().map(|i| i.name.clone()).collect(),
        outputs: c.outputs.iter().map(|o| o.name.clone()).collect(),
        dims: Some((format!("{}x{}x{}", d[0], d[1], d[2]), (d[0] * d[1] * d[2]).max(0) as u32)),
    };
    Some((c, p))
}

fn flags_of(metrics: &[(String, String)], robustness: &str, verdict: &str) -> Vec<String> {
    let mut f = Vec::new();
    if robustness == "robust" || verdict.trim_end().ends_with(", robust") {
        f.push("robust".to_string());
    }
    if metrics.iter().any(|(k, v)| k == "latency" && number(v) == Some(0)) || verdict.contains("instant") {
        f.push("instant".to_string());
    }
    f
}

/// The judge's test as the viewer and the page show it: how each input is driven, the stimulus
/// (`stimulus:` text) and its phases, the redsim run as a timing diagram, the checker's report.
struct Details {
    devices: Vec<String>,
    stimulus: String,
    phases: String,
    wave: Option<site_model::Wave>,
    table: Vec<String>,
    /// The redsim screen's metrics when it passes (the calibration designs show these).
    metrics: Option<Vec<(String, String)>>,
    /// The redsim screen's one-line verdict.
    summary: String,
    /// Why the screen fails, when it does.
    fails: Option<String>,
}

fn details(c: &Design) -> Details {
    let devices: Vec<String> = c
        .inputs
        .iter()
        .map(|i| match &i.how {
            circuit::InputHow::Lever => "lever".to_string(),
            circuit::InputHow::Button => "button".to_string(),
            circuit::InputHow::Block { on, .. } if on == circuit::INPUT_ON => "redstone block".to_string(),
            circuit::InputHow::Block { on, .. } => on.trim_start_matches("minecraft:").to_string(),
        })
        .collect();
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let mut d = Details { devices, stimulus: String::new(), phases: String::new(), wave: None, table: Vec::new(), metrics: None, summary: String::new(), fails: None };
    let r = match sim(c, &Options::default()) {
        Ok(r) => r,
        Err(e) => {
            d.summary = format!("redsim screen failed: {e:#}");
            return d;
        }
    };
    d.summary = r.summary(c);
    if !names.is_empty() && r.schedule.edges.iter().any(|e| !e.is_empty()) {
        d.stimulus = r.schedule.text(&names);
    }
    d.phases = r.schedule.phases.iter().map(|(a, b, l)| format!("{a}-{b} {}", l.replace(';', ","))).collect::<Vec<_>>().join("; ");
    if let Some(o) = &r.outcome {
        d.table = o.table.clone();
        if o.pass {
            d.metrics = Some(o.metrics.clone());
        } else {
            d.fails = Some(o.why.clone());
        }
    }
    if let Ok(t) = &r.trace {
        let mut rows: Vec<site_model::WaveRow> = Vec::new();
        for (k, i) in c.inputs.iter().enumerate() {
            let lv: Vec<u8> = t.inputs[k].iter().map(|on| if *on { 15 } else { 0 }).collect();
            rows.push(site_model::WaveRow::from_levels(&i.name, true, &lv));
        }
        for (k, o) in c.outputs.iter().enumerate() {
            rows.push(site_model::WaveRow::from_levels(&o.name, false, &t.levels[k]));
        }
        if !rows.is_empty() && t.frames() > 1 {
            d.wave = Some(site_model::Wave { ticks: t.frames() as u32, rows, phases: r.schedule.phases.clone() });
        }
    }
    d
}

/// The archived circuits of every study.
fn archived(root: &Path, notes: &mut Vec<String>) -> Vec<Build> {
    let mut out = Vec::new();
    for (study, _title) in STUDIES {
        let dir = archive_dir(root, study);
        let rows = match read_rows(&dir) {
            Ok(r) => r,
            Err(e) => {
                notes.push(format!("{}: {e:#}", dir.display()));
                continue;
            }
        };
        if rows.is_empty() {
            continue;
        }
        let caps = captions(&dir);
        // the catalog's fold: within one kind and axis value
        let mut groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for (i, r) in rows.iter().enumerate() {
            groups.entry((r.kind.clone(), r.axis.clone())).or_default().push(i);
        }
        let mut host: BTreeMap<usize, usize> = BTreeMap::new();
        for g in groups.values() {
            let items: Vec<(usize, i64, String, BTreeSet<String>)> = g.iter().map(|&i| (rows[i].blocks, rows[i].volume, rows[i].file.clone(), rows[i].kinds.split('+').filter(|k| !k.is_empty()).map(String::from).collect())).collect();
            for (k, h) in clock::catalog::fold_hosts(&items).into_iter().enumerate() {
                if let Some(h) = h {
                    host.insert(g[k], g[h]);
                }
            }
        }
        let id_of = |r: &circuit::archive::Row| format!("circuits/{study}/{}", r.file.trim_end_matches(".txt"));
        for (i, r) in rows.iter().enumerate() {
            let path = dir.join(&r.file);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let loaded = parse(&path);
            let det = loaded.as_ref().map(|(c, _)| details(c));
            let parsed = loaded.map(|(_, p)| p);
            let metrics = parse_metrics(&r.metrics);
            let mut flags = flags_of(&metrics, &r.robustness, &r.summary);
            let host_id = host.get(&i).map(|&h| id_of(&rows[h]));
            if host_id.is_some() {
                flags.push("variant".into());
            }
            let id = id_of(r);
            let kinds = parsed.as_ref().map(|p| p.kinds.clone()).unwrap_or_else(|| r.kinds.split('+').filter(|k| !k.is_empty()).map(String::from).collect());
            out.push(Build {
                id: id.clone(),
                kind: "circuit".into(),
                group: r.kind.clone(),
                name: r.file.trim_end_matches(".txt").to_string(),
                page: format!("{id}.html"),
                design: format!("designs/circuits/{study}/{}", r.file),
                blocks: r.blocks as u32,
                volume: Some(r.volume.max(0) as u32),
                dims: Some(r.dims.clone()).filter(|d| !d.is_empty()),
                kinds,
                caption: caps.get(&r.file).cloned().unwrap_or_else(|| comment_caption(&text)),
                flags,
                circuit: Some(Circuit {
                    study: study.to_string(),
                    kind: r.kind.clone(),
                    axis: r.axis.clone(),
                    value: number(&r.axis),
                    metrics,
                    verdict: r.summary.clone(),
                    inputs: parsed.as_ref().map(|p| p.inputs.clone()).unwrap_or_default(),
                    outputs: parsed.as_ref().map(|p| p.outputs.clone()).unwrap_or_default(),
                    family: r.family.clone(),
                    robustness: r.robustness.clone(),
                    host: host_id,
                    archived: r.archived.clone(),
                    devices: det.as_ref().map(|d| d.devices.clone()).unwrap_or_default(),
                    stimulus: det.as_ref().map(|d| d.stimulus.clone()).unwrap_or_default(),
                    phases: det.as_ref().map(|d| d.phases.clone()).unwrap_or_default(),
                    wave: det.as_ref().and_then(|d| d.wave.clone()),
                    table: det.as_ref().map(|d| d.table.clone()).unwrap_or_default(),
                }),
                archived: site_model::normalize_time(&r.archived),
                ..Default::default()
            });
        }
    }
    out
}

/// The judge's calibration circuits (verified in the game by clockinfra; metrics from redsim).
fn calibration(root: &Path, notes: &mut Vec<String>) -> Vec<Build> {
    let mut out = Vec::new();
    let dir = root.join("calibration");
    let Ok(rd) = std::fs::read_dir(&dir) else { return out };
    let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    for path in files {
        let file = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let stem = file.trim_end_matches(".txt").to_string();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let Some((c, p)) = parse(&path) else {
            notes.push(format!("{}: does not parse as a circuit", path.display()));
            continue;
        };
        let Some(kind) = c.kind.clone() else {
            notes.push(format!("{}: no `circuit:` kind", path.display()));
            continue;
        };
        let kind_name = kind.name();
        let det = details(&c);
        if let Some(why) = &det.fails {
            notes.push(format!("{}: redsim screen fails: {why}", path.display()));
        }
        let metrics = det.metrics.clone().unwrap_or_default();
        let verdict = format!("{} (redsim screen at site build time)", det.summary);
        // the axis as the archive writes it (`L = 2 gt`, `N = 4 gt`, the gate's function)
        let outcome = det.metrics.clone().map(|m| circuit::check::Outcome { pass: true, metrics: m, ..Default::default() });
        let axis = match &outcome {
            Some(o) => circuit::archive::axis(&kind, o, &p.kinds.join("+")),
            None => kind_name.clone(),
        };
        let mut flags = flags_of(&metrics, "", &verdict);
        flags.push("calibration".into());
        let id = format!("circuits/calibration/{stem}");
        let (dims, volume) = p.dims.clone().unzip();
        out.push(Build {
            id: id.clone(),
            kind: "circuit".into(),
            group: kind_name.clone(),
            name: stem,
            page: format!("{id}.html"),
            design: format!("designs/circuits/calibration/{file}"),
            blocks: c.blocks() as u32,
            volume,
            dims,
            kinds: p.kinds.clone(),
            caption: comment_caption(&text),
            flags,
            circuit: Some(Circuit {
                study: kind.study().to_string(),
                kind: kind_name,
                value: number(&axis),
                axis,
                metrics,
                verdict,
                inputs: p.inputs,
                outputs: p.outputs,
                devices: det.devices,
                stimulus: det.stimulus,
                phases: det.phases,
                wave: det.wave,
                table: det.table,
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    out
}

pub fn export(repo: &Path) -> Result<(Vec<Build>, Vec<String>)> {
    let root = repo.join(ROOT);
    let mut notes = Vec::new();
    let mut builds = archived(&root, &mut notes);
    builds.extend(calibration(&root, &mut notes));
    builds.sort_by(|a, b| {
        let s = |x: &Build| x.circuit.as_ref().map(|c| (c.study.clone(), c.value.unwrap_or(i64::MAX))).unwrap_or_default();
        (s(a), &a.group, a.blocks, &a.name).cmp(&(s(b), &b.group, b.blocks, &b.name))
    });
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
        assert_eq!(parse_metrics("L 2 gt, delay 1 gt"), m[..2].to_vec());
        let g = parse_verdict_metrics("CIRCUIT: cal-torch-not gate not fn not, latency 2 gt, glitches 0, 3 blocks, 4x2x1, robust", "gate not");
        assert_eq!(g[0], ("fn".into(), "not".into()));
        assert_eq!(g.len(), 3);
        assert!(parse_verdict_metrics("FAIL: x delay: no", "delay").is_empty());
    }

    #[test]
    fn numbers_and_captions() {
        assert_eq!(number("L = 4 gt"), Some(4));
        assert_eq!(number("xor"), None);
        assert_eq!(comment_caption("# archived 2026\nname: x\n# Input -> dust -> torch.\n"), "Input -> dust -> torch.");
        assert_eq!(flags_of(&[("latency".into(), "0 gt".into())], "robust", ""), vec!["robust", "instant"]);
    }
}
