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
            d.summary = format!("simulation failed: {e:#}");
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
                    ..Default::default()
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
        let verdict = format!("{} (simulated at site build time)", det.summary.trim_end().trim_end_matches("redsim").trim_end_matches([',', ' ']));
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
    apply_catalog(&root, &mut builds, &mut notes);
    builds.extend(calibration(&root, &mut notes));
    builds.sort_by(|a, b| {
        let s = |x: &Build| x.circuit.as_ref().map(|c| (c.study.clone(), c.value.unwrap_or(i64::MAX))).unwrap_or_default();
        (s(a), &a.group, a.blocks, &a.name).cmp(&(s(b), &b.group, b.blocks, &b.name))
    });
    Ok((builds, notes))
}

/// `designs/circuits/catalog.tsv` (`mctas circuit catalog`): per design (stem) its tunable group
/// and the shown design it is a variant or an equivalent layout of (stem).
fn read_catalog(root: &Path) -> BTreeMap<String, (String, String)> {
    let text = std::fs::read_to_string(root.join("catalog.tsv")).unwrap_or_default();
    let mut lines = text.lines();
    let Some(head) = lines.next() else { return BTreeMap::new() };
    let cols: Vec<&str> = head.split('\t').collect();
    let col = |n: &str| cols.iter().position(|c| *c == n);
    let (Some(ni), vi, gi) = (col("name"), col("variant_of"), col("group")) else { return BTreeMap::new() };
    let stem = |s: &str| s.rsplit('/').next().unwrap_or(s).trim_end_matches(".txt").to_string();
    lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let get = |i: Option<usize>| i.and_then(|i| f.get(i)).map(|s| s.trim()).unwrap_or("");
            let name = f.get(ni)?.trim();
            Some((stem(name), (get(gi).to_string(), if get(vi).is_empty() { String::new() } else { stem(get(vi)) })))
        })
        .collect()
}

/// The catalog's folds on the archived circuits: equivalent layouts (same kind, value, block count
/// and block kinds as a shown design: `layout_of`, hidden under it), variants (`host`: the
/// dominance fold), and tunable groups (`tunable`; the group's smallest design shows with its
/// span, the others are hidden until a value search hits them).
fn apply_catalog(root: &Path, builds: &mut [Build], notes: &mut Vec<String>) {
    let cat = read_catalog(root);
    if cat.is_empty() {
        notes.push("no designs/circuits/catalog.tsv: no tunable groups, the site's own variant fold".into());
        return;
    }
    let by_name: BTreeMap<String, (u32, Vec<String>, String)> = builds.iter().filter_map(|b| b.circuit.as_ref().map(|c| (b.name.clone(), (b.blocks, b.kinds.clone(), format!("circuits/{}/{}", c.study, b.name))))).collect();
    for b in builds.iter_mut() {
        let Some((group, of)) = cat.get(&b.name) else { continue };
        let (blocks, kinds) = (b.blocks, b.kinds.clone());
        let Some(c) = b.circuit.as_mut() else { continue };
        c.tunable = group.clone();
        // the catalog's fold replaces the site's own
        c.host = None;
        c.layout_of = None;
        b.flags.retain(|f| f != "variant" && f != "layout");
        if of.is_empty() {
            continue;
        }
        match by_name.get(of) {
            Some((hb, hk, id)) if *hb == blocks && *hk == kinds => {
                c.layout_of = Some(id.clone());
                b.flags.push("layout".into());
            }
            Some((_, _, id)) => {
                c.host = Some(id.clone());
                b.flags.push("variant".into());
            }
            None => notes.push(format!("{}: variant_of {of} is not an archived circuit", b.name)),
        }
    }
    // tunable groups: the smallest shown design stands for the group
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        if let Some(c) = &b.circuit {
            if !c.tunable.is_empty() && c.host.is_none() && c.layout_of.is_none() {
                groups.entry(c.tunable.clone()).or_default().push(i);
            }
        }
    }
    for (g, members) in groups {
        if members.len() < 2 {
            if let Some(c) = builds[members[0]].circuit.as_mut() {
                c.tunable.clear();
            }
            continue;
        }
        let rep = *members.iter().min_by_key(|&&i| (builds[i].blocks, builds[i].circuit.as_ref().and_then(|c| c.value).unwrap_or(i64::MAX), builds[i].name.clone())).unwrap_or(&members[0]);
        let mut values: Vec<i64> = members.iter().filter_map(|&i| builds[i].circuit.as_ref().and_then(|c| c.value)).collect();
        values.sort();
        values.dedup();
        let axis = builds[rep].circuit.as_ref().map(|c| c.axis.clone()).unwrap_or_default();
        let (prefix, unit) = match axis.split_once(" = ") {
            Some((p, v)) => (p.to_string(), if v.trim().ends_with(" gt") { " gt" } else { "" }),
            None => (String::new(), ""),
        };
        let span = match (values.first(), values.last()) {
            (Some(lo), Some(hi)) if values.len() > 1 => format!("{prefix} {lo}-{hi}{unit}, {} values", values.len()).trim().to_string(),
            _ => format!("{} versions", members.len()),
        };
        // the group's title and note: the first member's `group title:` / `group note:` headers
        let (mut title, mut note) = (String::new(), String::new());
        for &i in &members {
            let text = std::fs::read_to_string(root.join(repo_path_rel(&builds[i]))).unwrap_or_default();
            for l in text.lines() {
                if let Some(t) = l.strip_prefix("group title:").filter(|_| title.is_empty()) {
                    title = t.trim().to_string();
                }
                if let Some(t) = l.strip_prefix("group note:").filter(|_| note.is_empty()) {
                    note = t.trim().to_string();
                }
            }
        }
        for &i in &members {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.group_title = if title.is_empty() { g.clone() } else { title.clone() };
                c.group_note = note.clone();
            }
        }
        if let Some(c) = builds[rep].circuit.as_mut() {
            c.span = span;
        }
        builds[rep].flags.push("tunable".into());
    }
}

/// A circuit build's design path relative to designs/circuits.
fn repo_path_rel(b: &Build) -> String {
    repo_path(b).trim_start_matches(&format!("{ROOT}/")).to_string()
}

/// How the extendable circuit families are expanded at build time.
#[derive(Clone, Debug)]
pub struct FamilyOptions {
    /// Where the generated members go (`<site>/designs/circuits/members`); `None`: the formula only.
    pub members_dir: Option<PathBuf>,
    pub threads: usize,
}

/// The extendable circuit families (`extend` headers on archived designs, docs/circuits.md): one
/// card per family group (`extend group:`), every member of the verified n range generated with
/// the family's own generator and screened in redsim (`judged` when `extend.tsv` has it, `redsim`
/// when the screen passes with the formula's axis value, else `formula`), for the family pages and
/// the circuit value search.
pub fn families(repo: &Path, builds: &[Build], clock_cards: &[site_model::Card], o: &FamilyOptions) -> (Vec<site_model::Card>, Vec<site_model::Member>, Vec<String>) {
    let root = repo.join(ROOT);
    let mut notes = Vec::new();
    struct Fam {
        build: usize,
        path: PathBuf,
        f: clock::extend::Family,
        letter: String,
        unit: &'static str,
        judged: Vec<i64>,
    }
    let mut fams: Vec<Fam> = Vec::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(c) = &b.circuit else { continue };
        if c.study.is_empty() || b.page.starts_with("circuits/calibration/") {
            continue;
        }
        let path = repo.join(repo_path(b));
        let Ok(design) = Design::load(&path) else { continue };
        let f = match circuit::archive::family_of(&design) {
            Ok(Some(f)) => f,
            Ok(None) => continue,
            Err(e) => {
                notes.push(format!("{}: extend headers: {e:#}", b.name));
                continue;
            }
        };
        let (letter, unit) = match c.axis.split_once(" = ") {
            Some((l, v)) => (l.trim().to_string(), if v.trim().ends_with(" gt") { " gt" } else { "" }),
            None if c.kind.starts_with("counter") || c.kind.starts_with("piston-extender") => ("N".to_string(), ""),
            None => (String::new(), if c.axis.contains("gt") { " gt" } else { "" }),
        };
        let judged: Vec<i64> = std::fs::read_to_string(root.join(&c.study).join("archive").join(circuit::archive::EXTEND_TSV))
            .unwrap_or_default()
            .lines()
            .skip(1)
            .filter_map(|l| {
                let x: Vec<&str> = l.split('\t').collect();
                (x.first() == Some(&f.id.as_str())).then(|| x.get(1).and_then(|n| n.parse().ok())).flatten()
            })
            .collect();
        fams.push(Fam { build: i, path, f, letter, unit, judged });
    }
    // the cards: one per group (else per family), ids clear of the clock cards'
    let mut cards: Vec<site_model::Card> = Vec::new();
    let mut card_of: Vec<usize> = Vec::new();
    for fam in &fams {
        let key = fam.f.group.clone().unwrap_or_else(|| fam.f.id.clone());
        let id = if clock_cards.iter().any(|c| c.id == key) { format!("circuit-{key}") } else { key.clone() };
        let ci = match cards.iter().position(|c| c.id == id) {
            Some(ci) => ci,
            None => {
                let b = &builds[fam.build];
                let heading = match &fam.f.group_period {
                    Some(p) if p.contains('=') => p.clone(),
                    Some(p) => format!("{} = {p}{}", fam.letter, fam.unit),
                    None => format!("{} = {}{}", fam.letter, fam.f.period.text(), fam.unit),
                };
                cards.push(site_model::Card {
                    id: id.clone(),
                    title: fam.f.group_title.clone().unwrap_or_else(|| format!("{} ({})", key, crate::explain::kind_title(&b.group).to_lowercase())),
                    heading: heading.trim_start_matches(" = ").to_string(),
                    page: format!("families/{id}.html"),
                    note: fam.f.group_note.clone().unwrap_or_default(),
                    kind: "circuit".into(),
                    study: b.circuit.as_ref().map(|c| c.study.clone()).unwrap_or_default(),
                    ..Default::default()
                });
                cards.len() - 1
            }
        };
        let b = &builds[fam.build];
        cards[ci].families.push(site_model::Family {
            id: fam.f.id.clone(),
            build: b.id.clone(),
            period: crate::clocks::formula(&fam.f.period),
            period_text: fam.f.period.text(),
            blocks: crate::clocks::formula(&fam.f.blocks),
            blocks_text: fam.f.blocks.text(),
            rule: fam.f.rule.clone(),
            n_lo: fam.f.n_lo,
            n_hi: fam.f.n_hi,
            base: fam.f.base,
            judged: fam.judged.clone(),
            extra: Vec::new(),
        });
        card_of.push(ci);
    }
    // the members: every n of each family, generated in parallel
    let jobs: Vec<(usize, i64)> = fams.iter().enumerate().flat_map(|(k, fam)| (fam.f.n_lo..=fam.f.n_hi).map(move |n| (k, n))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<site_model::Member>>> = jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    let job_notes: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let threads = o.threads.max(1).min(jobs.len().max(1));
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= jobs.len() {
                    break;
                }
                let (k, n) = jobs[i];
                let fam = &fams[k];
                let b = &builds[fam.build];
                let value = fam.f.period.eval(n);
                let mut m = site_model::Member {
                    card: cards[card_of[k]].id.clone(),
                    family: fam.f.id.clone(),
                    n,
                    period: value.max(0) as u32,
                    blocks: fam.f.blocks.eval(n).max(0) as u32,
                    design: String::new(),
                    dims: None,
                    volume: None,
                    status: if fam.judged.contains(&n) { site_model::STATUS_JUDGED.into() } else { site_model::STATUS_FORMULA.into() },
                    build: None,
                    picture: None,
                    axis: format!("{} = {value}{}", fam.letter, fam.unit).trim_start_matches(" = ").to_string(),
                };
                if n == fam.f.base {
                    m.design = b.design.clone();
                    m.dims = b.dims.clone();
                    m.volume = b.volume;
                    m.build = Some(b.id.clone());
                    m.status = site_model::STATUS_JUDGED.into();
                } else if let Some(dir) = &o.members_dir {
                    let tmp = std::env::temp_dir().join(format!("mctas-site-cf-{}-{k}", std::process::id()));
                    match fam.f.build(&fam.path, n, &tmp) {
                        Ok(p) => {
                            let name = format!("{}-n{n}.txt", fam.f.id);
                            if std::fs::create_dir_all(dir).and_then(|_| std::fs::copy(&p, dir.join(&name)).map(|_| ())).is_ok() {
                                m.design = format!("designs/circuits/members/{name}");
                                if let Some((c, parsed)) = parse(&p) {
                                    if let Some((d, v)) = parsed.dims {
                                        m.dims = Some(d);
                                        m.volume = Some(v);
                                    }
                                    // the redsim screen: the formula's axis value, exactly
                                    let ok = sim(&c, &Options::default()).ok().and_then(|r| r.outcome).filter(|o| o.pass).is_some_and(|o| {
                                        let kind = c.kind.clone();
                                        kind.is_some_and(|k| number(&circuit::archive::axis(&k, &o, &parsed.kinds.join("+"))) == Some(value))
                                    });
                                    if ok && m.status != site_model::STATUS_JUDGED {
                                        m.status = site_model::STATUS_REDSIM.into();
                                    } else if !ok {
                                        job_notes.lock().unwrap().push(format!("{} n = {n}: the redsim screen does not give {} (kept as formula)", fam.f.id, m.axis));
                                    }
                                }
                            }
                            let _ = std::fs::remove_file(&p);
                        }
                        Err(e) => job_notes.lock().unwrap().push(format!("{} n = {n}: {e:#}", fam.f.id)),
                    }
                }
                *slots[i].lock().unwrap() = Some(m);
            });
        }
    });
    notes.extend(job_notes.into_inner().unwrap_or_default());
    let members: Vec<site_model::Member> = slots.into_iter().filter_map(|s| s.into_inner().ok().flatten()).collect();
    (cards, members, notes)
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
