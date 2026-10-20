//! The circuits (the contraption studies, docs/circuits.md): every design the judge archived in
//! `designs/circuits/<study>/archive/` (its `verdicts.tsv` row: kind, axis, metrics, robustness,
//! the judge line; `captions.tsv`), with the catalog's folds (`catalog.tsv`: equivalent layouts,
//! tunable groups; the dominance fold within one kind and axis value), the judge's own test run
//! here for each page (the viewer's demo and the timing diagram), and the extendable families
//! expanded to every member. The judge's calibration and selftest designs are not published.

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

/// Metric names of more than one word.
const MULTI_WORD_METRICS: [&str; 4] = ["first box", "per pulse", "per period", "hoppers awake"];

/// `L 2 gt, delay 1 gt, min-pulse 1 gt` -> [(L, 2 gt), (delay, 1 gt), (min-pulse, 1 gt)]. A part
/// that starts with a number continues the previous value (`bill 16 iron, 3 quartz`, `period 10
/// gt, 1 cell(s)`); `first box 13847 gt`, `per period 1`... are one name.
pub fn parse_metrics(s: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for p in s.split(", ").map(str::trim).filter(|p| !p.is_empty()) {
        if p.starts_with(|c: char| c.is_ascii_digit()) {
            if let Some(last) = out.last_mut() {
                last.1.push_str(", ");
                last.1.push_str(p);
                continue;
            }
        }
        if let Some(m) = MULTI_WORD_METRICS.iter().find(|m| p.strip_prefix(**m).is_some_and(|r| r.starts_with(' '))) {
            out.push((m.to_string(), p[m.len() + 1..].to_string()));
            continue;
        }
        out.push(match p.split_once(' ') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (p.to_string(), String::new()),
        });
    }
    out
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

/// What a design file adds to its row: block kinds, input and output names, its box, a machine's
/// travel direction in words.
struct Parsed {
    kinds: Vec<String>,
    inputs: Vec<String>,
    outputs: Vec<String>,
    dims: Option<(String, u32)>,
    direction: String,
}

fn parse(path: &Path) -> Option<(Design, Parsed)> {
    let c = Design::load(path).ok()?;
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    // only flying machines travel on their own; an extender's `travel` is its push direction
    let direction = match (&c.kind, c.travel) {
        (Some(circuit::Kind::FlyingMachine), Some(t)) => match t[1] {
            1 => "up",
            -1 => "down",
            _ => "horizontal",
        }
        .to_string(),
        _ => String::new(),
    };
    let p = Parsed {
        kinds: c.kinds().into_iter().collect(),
        inputs: c.inputs.iter().map(|i| i.name.clone()).collect(),
        outputs: c.outputs.iter().map(|o| o.name.clone()).collect(),
        dims: Some((format!("{}x{}x{}", d[0], d[1], d[2]), (d[0] * d[1] * d[2]).max(0) as u32)),
        direction,
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
    /// The judge's metrics when its run on the redsim screen passes.
    metrics: Option<Vec<(String, String)>>,
    /// The redsim screen's one-line verdict.
    summary: String,
    /// Why the screen fails, when it does.
    fails: Option<String>,
    /// Every output cell gets the judge's lamp in the 3D view.
    output_lamp: bool,
    /// The judge's actions at the target / probe cells (`data-events`).
    events: String,
    /// Why redsim cannot run the design (empty: it can).
    unmodeled: String,
    /// A shulker machine run like its 3D view runs it.
    view_run: Option<site_model::ViewRun>,
}

/// Game ticks a shulker machine is run at most the way its 3D view runs it, until it has sent out
/// four boxes (a 64-stack filter loader takes ~13800 gt a box).
const VIEW_RUN: u32 = 60_000;
/// Boxes the view's run waits for.
const VIEW_BOXES: usize = 4;

/// Run a shulker machine the way the 3D view does (the viewer's own session: the judge's chests
/// kept full / emptied, its start lever turned on like the visitor and the judge do) and count
/// the boxes that reach the OUT chests: the page's numbers, and proof the view's machine runs.
fn view_run(s: &mut mcview::session::Session, ticks: u32) -> site_model::ViewRun {
    use redsim::redstone::{container, shulker};
    let levers: Vec<usize> = s
        .circuit_inputs
        .iter()
        .enumerate()
        .filter(|(_, ci)| ci.how == mcview::circuit::InputHow::Lever && !(ci.name == "mode" || ci.name.starts_with("mode.")))
        .map(|(i, _)| i)
        .collect();
    for i in levers {
        s.set_input(i, true);
    }
    let outs = s.shulker.as_ref().map(|c| c.outputs.clone()).unwrap_or_default();
    let rejects = s.shulker.as_ref().map(|c| c.rejects.clone()).unwrap_or_default();
    let item = s.shulker.as_ref().map(|c| container::item_name(c.item).to_string()).unwrap_or_default();
    let mut arrivals: Vec<u32> = Vec::new();
    let mut rejected = 0u32;
    let mut run = ticks;
    for k in 0..ticks {
        // an OUT chest holds what the last tick delivered until the next visit empties it
        let n: u32 = outs
            .iter()
            .map(|p| shulker::chest_stacks(&s.world, *p).iter().filter(|st| container::item_name(st.item).ends_with("shulker_box")).map(|st| st.count as u32).sum::<u32>())
            .sum();
        arrivals.extend(std::iter::repeat_n(k, n as usize));
        rejected += rejects.iter().map(|p| shulker::chest_stacks(&s.world, *p).iter().map(|st| st.count as u32).sum::<u32>()).sum::<u32>();
        if arrivals.len() >= VIEW_BOXES {
            run = k;
            break;
        }
        s.step();
    }
    let cycle = match (arrivals.first(), arrivals.last()) {
        (Some(a), Some(b)) if arrivals.len() >= 2 => Some((b - a) as f64 / (arrivals.len() - 1) as f64),
        _ => None,
    };
    site_model::ViewRun { ticks: run, boxes: arrivals.len() as u32, first: arrivals.first().copied(), cycle, item, rejected }
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
    // the viewer puts the judge's lamp in every air output cell (webrender bbe3bf7ba): then the
    // lamp shows the output, else the orange marker does
    let consumers = c.consumer_cells();
    let output_lamp = !c.outputs.is_empty() && c.outputs.iter().all(|o| consumers.contains(&o.pos));
    let mut d = Details { devices, stimulus: String::new(), phases: String::new(), wave: None, table: Vec::new(), metrics: None, summary: String::new(), fails: None, output_lamp, events: String::new(), unmodeled: String::new(), view_run: None };
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
    // the judge's actions at the BUD's target and probe cells, in the design's coordinates
    let probes = c.probe_cells();
    d.events = r
        .schedule
        .events
        .iter()
        .filter_map(|(frame, at, op)| {
            let p = match at {
                circuit::stimulus::EventAt::Target(i) => c.targets.get(*i)?.pos,
                circuit::stimulus::EventAt::Probe(k) => *probes.get(*k)?,
            };
            let what = match op {
                circuit::stimulus::TargetOp::Place(s) => format!("place {} {} {} {s}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Clear => format!("clear {} {} {}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Shape(s) => format!("shape {} {} {} {s}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Update => format!("update {} {} {}", p[0], p[1], p[2]),
            };
            Some(format!("{frame} {what}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
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
        // a BUD's judge acts at its target cells: one row of marks (the footprint probes left out)
        let marks: Vec<(u32, String)> = r
            .schedule
            .events
            .iter()
            .filter(|(_, at, _)| matches!(at, circuit::stimulus::EventAt::Target(_)))
            .map(|(f, _, op)| {
                let what = match op {
                    circuit::stimulus::TargetOp::Place(_) => "place",
                    circuit::stimulus::TargetOp::Clear => "clear",
                    circuit::stimulus::TargetOp::Shape(_) => "shape",
                    circuit::stimulus::TargetOp::Update => "update",
                };
                (*f, what.to_string())
            })
            .collect();
        if !marks.is_empty() {
            rows.push(site_model::WaveRow { name: "target".into(), input: true, marks, ..Default::default() });
        }
        for (k, o) in c.outputs.iter().enumerate() {
            rows.push(site_model::WaveRow::from_levels(&o.name, false, &t.levels[k]));
        }
        // the machines' position tracks (vjeux: "Can you put the position of the block?")
        let metric = |k: &str| r.outcome.as_ref().and_then(|o| o.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())).unwrap_or_default();
        match &c.kind {
            Some(circuit::Kind::PistonExtender(n)) => {
                if let Some(row) = payload_track(c, t, *n) {
                    let mut row = row;
                    let (ext, ret) = (metric("extend"), metric("retract"));
                    if !ext.is_empty() {
                        row.note = format!("out to N = {n} in {ext}, back in {ret}");
                    }
                    rows.push(row);
                }
            }
            Some(circuit::Kind::FlyingMachine) => {
                if let Some(mut row) = flight_track(c, t) {
                    let speed = metric("speed");
                    row.note = if speed.is_empty() { metric("two-way") } else { format!("1 block every {}", speed.trim_end_matches("/block")) };
                    rows.push(row);
                }
            }
            _ => {}
        }
        if !rows.is_empty() && t.frames() > 1 {
            d.wave = Some(site_model::Wave { ticks: t.frames() as u32, rows, phases: r.schedule.phases.clone() });
        }
    }
    d
}

/// A piston extender's payload position per frame along its push direction (0 = home .. N =
/// extended); while it moves (carried inside a moving piston) the last position holds.
fn payload_track(c: &Design, t: &circuit::trace::Trace, n: u32) -> Option<site_model::WaveRow> {
    let d = c.travel?;
    let p0 = c.payloads.first()?.pos;
    let rest_at = (c.clock.free_from() + circuit::stimulus::SETTLE).saturating_sub(1) as usize;
    let payload = t.state(rest_at.min(t.frames().saturating_sub(1)), p0)?.to_string();
    let id = t.names.iter().position(|s| *s == payload)? as u32;
    let mut last = 0i32;
    let mut levels: Vec<u8> = Vec::with_capacity(t.frames());
    let mut top = n as i32;
    for f in 0..t.frames() {
        let here: Vec<&circuit::Pos> = t.cells[f].iter().filter(|(_, x)| *x == id).map(|(p, _)| p).collect();
        if here.len() == 1 {
            let p = here[0];
            last = (p[0] - p0[0]) * d[0] + (p[1] - p0[1]) * d[1] + (p[2] - p0[2]) * d[2];
        }
        top = top.max(last);
        levels.push(last.clamp(0, 255) as u8);
    }
    let mut row = site_model::WaveRow::from_levels("payload", false, &levels);
    row.scale = top.max(1) as u32;
    row.target = Some(n);
    Some(row)
}

/// A flying machine's displacement along its travel per frame (cells from where it was built);
/// while it moves between intact positions the last one holds.
fn flight_track(c: &Design, t: &circuit::trace::Trace) -> Option<site_model::WaveRow> {
    let travel = c.travel.unwrap_or([1, 0, 0]);
    let kick = match &c.clock.kick {
        Some(clock::Kick::Place { pos, ticks, .. }) => Some((*pos, c.clock.kick_at, c.clock.kick_at + ticks)),
        _ => None,
    };
    let mut rec = circuit::fly::Recorder::new(&c.clock.design.blocks, kick, &c.stations);
    for f in 0..t.frames() {
        rec.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    let mut last = 0i32;
    let mut top = 1i32;
    let mut levels: Vec<u8> = Vec::with_capacity(t.frames());
    for s in &rec.shifts {
        if let Some(s) = s {
            last = (s[0] * travel[0] + s[1] * travel[1] + s[2] * travel[2]).abs();
        }
        top = top.max(last);
        levels.push(last.clamp(0, 255) as u8);
    }
    if top <= 1 && levels.iter().all(|l| *l == 0) {
        return None;
    }
    let mut row = site_model::WaveRow::from_levels("moved", false, &levels);
    row.scale = top as u32;
    Some(row)
}

/// `f` over `items` on every core, results in order.
fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 64).min(items.len().max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let slots: Vec<std::sync::Mutex<Option<R>>> = items.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let r = f(&items[i]);
                if let Ok(mut slot) = slots[i].lock() {
                    *slot = Some(r);
                }
            });
        }
    });
    slots.into_iter().map(|m| m.into_inner().ok().flatten().expect("every item mapped")).collect()
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
            // as `mctas circuit catalog` folds (palette alone is never a distinction)
            let items: Vec<(usize, i64, String)> = g.iter().map(|&i| (rows[i].blocks, rows[i].volume, rows[i].file.clone())).collect();
            let robust: Vec<bool> = g.iter().map(|&i| circuit::archive::is_robust(&rows[i])).collect();
            for (k, h) in clock::catalog::fold_hosts_robust(&items, &robust, |a, b| circuit::archive::beats_on_variation(&rows[g[a]], &rows[g[b]])).into_iter().enumerate() {
                if let Some(h) = h {
                    host.insert(g[k], g[h]);
                }
            }
        }
        let id_of = |r: &circuit::archive::Row| format!("circuits/{study}/{}", r.file.trim_end_matches(".txt"));
        // the judge's test of every design, simulated in parallel (big counters take seconds),
        // and whether the 3D view runs it (the viewer's own session decides: shown as built when
        // the browser's simulator cannot follow it)
        let loaded_all: Vec<(Option<(Design, Parsed)>, Option<Details>, f64, Option<String>)> = par_map(&rows, |r| {
            let loaded = parse(&dir.join(&r.file));
            let t_sim = std::time::Instant::now();
            let mut det = loaded.as_ref().map(|(c, _)| details(c));
            let mut raw = None;
            if let Some(d) = det.as_mut() {
                let text = std::fs::read_to_string(dir.join(&r.file)).unwrap_or_default();
                raw = match mcview::session::Session::new(&text, &Default::default()) {
                    Ok(mut s) => {
                        if s.unsimulated.is_none() && s.shulker.is_some() {
                            d.view_run = Some(view_run(&mut s, VIEW_RUN));
                        }
                        s.unsimulated
                    }
                    Err(e) => Some(format!("the viewer cannot load it: {e}")),
                };
                d.unmodeled = raw.as_deref().map(crate::explain::plain_reason).unwrap_or_default();
            }
            (loaded, det, t_sim.elapsed().as_secs_f64(), raw)
        });
        for (i, (r, (loaded, det, took, raw))) in rows.iter().zip(loaded_all).enumerate() {
            let path = dir.join(&r.file);
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if let Some(raw) = raw {
                notes.push(format!("{study}/{}: the 3D view shows it as built: {raw}", r.file));
            }
            if let Some(v) = det.as_ref().and_then(|d| d.view_run.as_ref()) {
                notes.push(match (v.first, v.cycle) {
                    (Some(f), Some(c)) => format!("{study}/{}: the 3D view's run sends out {} boxes in {} gt (the first at {f} gt, then one every {c:.0} gt)", r.file, v.boxes, v.ticks),
                    (Some(f), None) => format!("{study}/{}: the 3D view's run sends out 1 box in {} gt (at {f} gt)", r.file, v.ticks),
                    _ => format!("{study}/{}: WARNING the 3D view's run sends out NO box in {} gt (its chests supply {}; {} items reached the reject chests)", r.file, v.ticks, v.item, v.rejected),
                });
            }
            if took > 10.0 {
                notes.push(format!("{study}/{}: the judge's test took {took:.1} s to simulate", r.file));
            }
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
                    direction: parsed.as_ref().map(|p| p.direction.clone()).unwrap_or_default(),
                    family: r.family.clone(),
                    robustness: r.robustness.clone(),
                    host: host_id,
                    archived: r.archived.clone(),
                    devices: det.as_ref().map(|d| d.devices.clone()).unwrap_or_default(),
                    stimulus: det.as_ref().map(|d| d.stimulus.clone()).unwrap_or_default(),
                    phases: det.as_ref().map(|d| d.phases.clone()).unwrap_or_default(),
                    wave: det.as_ref().and_then(|d| d.wave.clone()),
                    table: det.as_ref().map(|d| d.table.clone()).unwrap_or_default(),
                    output_lamp: det.as_ref().is_some_and(|d| d.output_lamp),
                    events: det.as_ref().map(|d| d.events.clone()).unwrap_or_default(),
                    unmodeled: det.as_ref().map(|d| d.unmodeled.clone()).unwrap_or_default(),
                    view_run: det.as_ref().and_then(|d| d.view_run.clone()),
                    ..Default::default()
                }),
                archived: site_model::normalize_time(&r.archived),
                ..Default::default()
            });
        }
    }
    out
}

/// Every archived circuit of the studies. The judge's calibration and selftest designs
/// (`designs/circuits/calibration/`, `selftest/`) stay in the repository for the judge and are
/// not published (vjeux: "They are not useful to people").
pub fn export(repo: &Path) -> Result<(Vec<Build>, Vec<String>)> {
    let root = repo.join(ROOT);
    let mut notes = Vec::new();
    let mut builds = archived(&root, &mut notes);
    apply_catalog(&root, &mut builds, &mut notes);
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
    for (_group, members) in groups {
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
                // without a `group title:` header: the kind for people (`Piston extender (reach 2)`)
                c.group_title = if title.is_empty() { crate::explain::kind_title(&c.kind) } else { title.clone() };
                c.group_note = note.clone();
            }
        }
        // what the members differ in, from their designs (vjeux: say WHAT differs)
        let feats: Vec<VariantFeatures> = members.iter().map(|&i| Design::load(&root.join(repo_path_rel(&builds[i]))).map(|d| variant_features(&d)).unwrap_or_default()).collect();
        let (differs, labels) = variant_differences(&feats);
        for (k, &i) in members.iter().enumerate() {
            if let Some(c) = builds[i].circuit.as_mut() {
                c.group_differs = differs.clone();
                c.variant_label = labels[k].clone();
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

/// What can differ between the members of a tunable group, read from one member's design.
#[derive(Clone, Debug, Default, PartialEq)]
struct VariantFeatures {
    /// How the inputs are driven: `lever`, `stone button`, `oak button`, `redstone block`.
    device: String,
    /// Every repeater's delay, sorted.
    repeaters: Vec<u32>,
    /// Comparators in subtract mode, of how many.
    subtract: usize,
    comparators: usize,
    blocks: usize,
}

fn variant_features(c: &Design) -> VariantFeatures {
    let blocks = &c.clock.design.blocks;
    let word = |s: &str| s.split(['[', '{']).next().unwrap_or(s).trim_start_matches("minecraft:").replace('_', " ");
    let device = c
        .inputs
        .iter()
        .map(|i| match &i.how {
            circuit::InputHow::Lever => "lever".to_string(),
            circuit::InputHow::Button => blocks.get(&i.pos).map(|s| word(s)).unwrap_or_else(|| "button".into()),
            circuit::InputHow::Block { on, .. } => word(on),
        })
        .collect::<Vec<_>>()
        .join(", ");
    let mut repeaters: Vec<u32> = blocks
        .values()
        .filter(|s| s.starts_with("minecraft:repeater"))
        .map(|s| s.split("delay=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|d| d.parse().ok()).unwrap_or(1))
        .collect();
    repeaters.sort();
    let comparators: Vec<&String> = blocks.values().filter(|s| s.starts_with("minecraft:comparator")).collect();
    VariantFeatures { device, repeaters, subtract: comparators.iter().filter(|s| s.contains("mode=subtract")).count(), comparators: comparators.len(), blocks: c.blocks() }
}

/// What a group's members differ in, and each member's label for it (in `feats` order).
fn variant_differences(feats: &[VariantFeatures]) -> (String, Vec<String>) {
    let varies = |f: &dyn Fn(&VariantFeatures) -> String| feats.iter().map(f).collect::<BTreeSet<_>>().len() > 1;
    let device = varies(&|x| x.device.clone());
    let rep_count = varies(&|x| x.repeaters.len().to_string());
    // the delays differ beyond the count: two members with as many repeaters, other delays
    let rep_delays = feats.iter().enumerate().any(|(i, a)| feats[i + 1..].iter().any(|b| a.repeaters.len() == b.repeaters.len() && a.repeaters != b.repeaters)) || (!rep_count && varies(&|x| format!("{:?}", x.repeaters)));
    let cmp = varies(&|x| format!("{}/{}", x.subtract, x.comparators));
    let length = varies(&|x| x.blocks.to_string());
    let mut aspects: Vec<&str> = Vec::new();
    if device {
        aspects.push("the input device");
    }
    if rep_delays {
        aspects.push("the repeater delays");
    }
    if rep_count {
        aspects.push("the length of the repeater line");
    }
    if cmp {
        aspects.push("the comparator modes");
    }
    if aspects.is_empty() && length {
        aspects.push("the length");
    }
    if aspects.is_empty() {
        aspects.push("their layout");
    }
    let text = match aspects.len() {
        1 => aspects[0].to_string(),
        n => format!("{} and {}", aspects[..n - 1].join(", "), aspects[n - 1]),
    };
    let labels = feats
        .iter()
        .map(|x| {
            let mut parts: Vec<String> = Vec::new();
            if device {
                parts.push(x.device.clone());
            }
            if rep_count || rep_delays {
                parts.push(if x.repeaters.is_empty() { "no repeater".to_string() } else { format!("delays {}", x.repeaters.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("+")) });
            }
            if cmp {
                parts.push(if x.subtract == 0 { "compare".to_string() } else { format!("{} subtract", x.subtract) });
            }
            if parts.is_empty() {
                parts.push(format!("{} blocks", x.blocks));
            }
            parts.join(", ")
        })
        .collect();
    (text, labels)
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
        if c.study.is_empty() {
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
    // designs/circuits/<study>/<file> on the site, <study>/archive/<file> in the repository
    let rest = b.design.trim_start_matches("designs/circuits/");
    match rest.split_once('/') {
        Some((study, f)) => format!("{ROOT}/{study}/archive/{f}"),
        None => b.design.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_differences_name_what_varies() {
        let f = |device: &str, reps: &[u32], blocks: usize| VariantFeatures { device: device.into(), repeaters: reps.to_vec(), blocks, ..Default::default() };
        let (t, l) = variant_differences(&[f("lever", &[], 7), f("stone button", &[], 7), f("oak button", &[], 7)]);
        assert_eq!(t, "the input device");
        assert_eq!(l, vec!["lever", "stone button", "oak button"]);
        let (t, l) = variant_differences(&[f("redstone block", &[1, 4], 17), f("redstone block", &[4, 4], 17), f("redstone block", &[4, 4, 4], 21)]);
        assert_eq!(t, "the repeater delays and the length of the repeater line");
        assert_eq!(l[2], "delays 4+4+4");
        let (t, _) = variant_differences(&[f("redstone block", &[2], 5), f("redstone block", &[4], 5)]);
        assert_eq!(t, "the repeater delays");
    }

    #[test]
    fn verdict_metrics() {
        let m = parse_verdict_metrics("CIRCUIT: sub-rising-4 rising-edge L 2 gt, delay 1 gt, min-pulse 1 gt, 5 blocks, 3x2x1, robust", "rising-edge");
        assert_eq!(m, vec![("L".into(), "2 gt".into()), ("delay".into(), "1 gt".into()), ("min-pulse".into(), "1 gt".into())]);
        assert_eq!(parse_metrics("L 2 gt, delay 1 gt"), m[..2].to_vec());
        let s = parse_metrics("boxes/min 0.087, first box 13847 gt, hoppers awake 3, badges 16-stack+drop-free, bill 16 iron, 3 quartz, 1 slime");
        assert_eq!(s[1], ("first box".to_string(), "13847 gt".to_string()));
        assert_eq!(s[2], ("hoppers awake".to_string(), "3".to_string()));
        assert_eq!(s[4], ("bill".to_string(), "16 iron, 3 quartz, 1 slime".to_string()));
        assert_eq!(parse_metrics("rate 200 TNT/min, period 6 gt, per period 1")[2], ("per period".to_string(), "1".to_string()));
        assert_eq!(parse_metrics("speed 10 gt/block, period 10 gt, 1 cell(s), latency 2 gt")[1], ("period".to_string(), "10 gt, 1 cell(s)".to_string()));
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
