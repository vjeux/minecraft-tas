//! `mctas clock misc trace-export <archived designs>... [--out-dir designs/clocks/traces]`: record
//! real-game traces of archived clocks for the in-browser viewer (webrender), one JSON Lines file
//! per clock at `<out-dir>/<design stem>.trace` (format agreed with webrender, 2026-10-19):
//!
//! * line 1, the header: `{"trace":1,"name":..,"design":"designs/clocks/archive/<file>","period":P,
//!   "warmup":W,"ticks":N,"box":[[x0,y0,z0],[x1,y1,z1]],"loop":[W,W+P],"output":[x,y,z]}` (output
//!   only when the design has one); ticks = warmup + 2 periods, from the judge's verdict row;
//! * then one line per game tick t = 0..=ticks: `{"t":t,"blocks":[[x,y,z,"state"],..],
//!   "entities":[{"id":0,"type":"minecraft:oak_boat","pos":[x,y,z],"yaw":a,"pitch":b}]}`: at t = 0
//!   every non-air cell of the watched box (the design plus a 3-cell margin), later only the cells
//!   that changed (air included); every entity every tick, with a stable id; positions rounded to
//!   4 decimals, yaw / pitch to 2.
//!
//! Coordinates are the design's own: the build is the judge's first one (rotation 0, position 0),
//! placed and kicked exactly like `clock verify`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use clock::Clock;
use mctas::sim::{SandboxOptions, Worker, WorkerConfig};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(ClapArgs, Debug, Clone)]
pub struct ExportArgs {
    /// Archived designs (designs/clocks/archive/<name>.txt).
    pub designs: Vec<PathBuf>,
    /// Where the traces go (default designs/clocks/traces, or designs/clocks/traces/exact with --exact).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// Margin of the watched box around the design (the judge's default).
    #[arg(long, default_value_t = 3)]
    pub margin: i32,
    /// Full precision (for redsim's trace-compare gate): positions, motion and rotations as the
    /// game's own doubles / floats (no rounding), plus each entity's "motion" and "net_id" (the
    /// game's network id, which ItemEntity's ticks read).
    #[arg(long)]
    pub exact: bool,
    /// Record this many ticks of any design (not only archived ones: a test scene for redsim);
    /// the header's period and warmup are then 0 (or `--period` / `--warmup`) and "design" is the
    /// path given.
    #[arg(long)]
    pub ticks: Option<u32>,
    /// With `--ticks`: the period the header states (an un-archived design whose loop is known,
    /// e.g. a re-export of a trace that had one).
    #[arg(long, default_value_t = 0, requires = "ticks")]
    pub period: u32,
    /// With `--ticks`: the warmup the header states.
    #[arg(long, default_value_t = 0, requires = "ticks")]
    pub warmup: u32,
    /// Save, unload and load the sandbox's chunks again right after this frame (the clock
    /// judge's chunk-reload check, `clock verify --reload`), to see what a reload changes.
    #[arg(long)]
    pub reload_at: Option<u32>,
    /// Set the sandbox's randomTickSpeed game rule (header `random_tick_speed`): 0 for a scene
    /// with grass, whose random ticks redsim does not model (they draw from the level random at
    /// positions chosen by the level's unseeded randValue).
    #[arg(long)]
    pub random_tick_speed: Option<u32>,
    /// Extra inputs for a test scene (with `--ticks`): `T:use:x,y,z` (a right-click on the block
    /// there) or `T:use_entity:x,y,z` (an empty-hand right-click on the item frame in that cell),
    /// applied before tick T like the design's own kick and listed in the header's `actions`, which
    /// redsim's trace-compare replays.
    #[arg(long = "action", requires = "ticks")]
    pub extra_actions: Vec<String>,
    /// One trace per sandbox seed `A..B` (`<stem>-s<seed>.trace`, header `seed`): another level
    /// random each time (TNT kicks, item drops...), which redsim's trace-compare mirrors.
    #[arg(long)]
    pub seeds: Option<String>,
}

/// A `--action` input: `T:use:x,y,z` or `T:use_entity:x,y,z`.
pub fn parse_action(s: &str) -> Result<(u32, clock::Action, Value)> {
    let parts: Vec<&str> = s.split(':').collect();
    let [t, kind, at] = parts.as_slice() else { bail!("--action T:use|use_entity:x,y,z, got {s:?}") };
    let t: u32 = t.parse().with_context(|| format!("--action tick in {s:?}"))?;
    let p: Vec<i32> = at.split(',').map(|v| v.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--action cell in {s:?}"))?;
    let [x, y, z] = p.as_slice() else { bail!("--action cell x,y,z in {s:?}") };
    let pos = [*x, *y, *z];
    let a = match *kind {
        "use" => clock::Action::Use { pos },
        "use_entity" => clock::Action::UseEntity { pos },
        other => bail!("--action kind use or use_entity, not {other:?}"),
    };
    Ok((t, a, json!([t, kind, pos])))
}

/// (period, warmup) of an archived design, from verdicts-v1.tsv (columns: hash, file, period,
/// warmup, ...).
fn verdict(file: &str) -> Result<(u32, u32)> {
    let tsv = std::fs::read_to_string("designs/clocks/archive/verdicts-v1.tsv").context("read verdicts-v1.tsv (run from the repository root)")?;
    for line in tsv.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() > 3 && f[1] == file {
            return Ok((f[2].parse()?, f[3].parse()?));
        }
    }
    bail!("{file}: no verdict row (archive it first)")
}

fn round(v: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (v * m).round() / m
}

fn export_one(w: &mut Worker, path: &Path, out_dir: &Path, a: &ExportArgs, extra: &[(u32, clock::Action, Value)], seed_override: Option<i64>) -> Result<(PathBuf, u32)> {
    let (margin, exact, reload_at, random_tick_speed) = (a.margin, a.exact, a.reload_at, a.random_tick_speed);
    let fixed_ticks = a.ticks.map(|n| (n, a.period, a.warmup));
    let file = path.file_name().and_then(|s| s.to_str()).context("file name")?.to_string();
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?.to_string();
    let (period, warmup, ticks, design) = match fixed_ticks {
        Some((n, period, warmup)) => (period, warmup, n, path.display().to_string()),
        None => {
            let (period, warmup) = verdict(&file)?;
            (period, warmup, warmup + 2 * period, format!("designs/clocks/archive/{file}"))
        }
    };
    let c = Clock::load(path).with_context(|| format!("load {}", path.display()))?;
    let (lo, hi) = c.bounds();
    let from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let seed = seed_override.unwrap_or(0);
    let sb = w.create(&SandboxOptions { chunks, seed })?;
    if let Some(n) = random_tick_speed {
        w.exec(sb, &format!("gamerule random_tick_speed {n}"))?;
    }
    clock::verify::place(w, sb, &c)?;
    let out_path = out_dir.join(match seed_override {
        Some(s) => format!("{stem}-s{s}.trace"),
        None => format!("{stem}.trace"),
    });
    let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path)?);
    // the sandbox's forced chunk box and seed: what redsim's level-random mirror needs (the chunk
    // ticks draw randomTickSpeed x nextInt(48) per forced chunk every tick)
    let mut header = json!({
        "trace": 1,
        "name": c.design.name,
        "design": design,
        "period": period,
        "warmup": warmup,
        "ticks": ticks,
        "box": [from, to],
        "chunks": chunks,
        "seed": seed,
        "loop": [warmup, warmup + period],
    });
    if let Some(o) = c.output {
        header["output"] = json!(o);
    }
    if let Some(n) = random_tick_speed {
        header["random_tick_speed"] = json!(n);
    }
    if !extra.is_empty() {
        header["actions"] = json!(extra.iter().map(|(_, _, v)| v.clone()).collect::<Vec<Value>>());
    }
    writeln!(out, "{header}")?;
    let mut prev: HashMap<[i32; 3], String> = HashMap::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut actions = c.actions();
    // the --action inputs, after the design's own at the same tick
    actions.extend(extra.iter().map(|(t, act, _)| (*t, act.clone())));
    actions.sort_by_key(|(t, _)| *t);
    for t in 0..=ticks {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, act) = actions.remove(0);
            clock::verify::apply(w, sb, &act)?;
        }
        if t > 0 {
            w.step(sb, 1)?;
        }
        let region = w.get_region(sb, from, to)?;
        let mut blocks: Vec<Value> = Vec::new();
        for (p, s) in region.iter() {
            let air = mctas::blockstate::is_air(s);
            let changed = match prev.get(&p) {
                Some(x) => x != s,
                None => !air,
            };
            if changed {
                blocks.push(json!([p[0], p[1], p[2], s]));
                prev.insert(p, s.to_string());
            }
        }
        let mut ents: Vec<Value> = Vec::new();
        for e in w.entities(sb)? {
            let n = ids.len();
            let id = *ids.entry(e.uuid.clone()).or_insert(n);
            ents.push(if exact {
                json!({
                    "id": id,
                    "net_id": e.net_id,
                    "type": e.kind,
                    "pos": e.pos,
                    "motion": e.motion,
                    "yaw": e.yaw,
                    "pitch": e.pitch,
                })
            } else {
                json!({
                    "id": id,
                    "type": e.kind,
                    "pos": [round(e.pos[0], 4), round(e.pos[1], 4), round(e.pos[2], 4)],
                    "yaw": round(e.yaw as f64, 2),
                    "pitch": round(e.pitch as f64, 2),
                })
            });
        }
        // exact traces: the level random's state after this frame (redsim's mirror is checked
        // against it every frame)
        let mut frame = json!({"t": t, "blocks": blocks, "entities": ents});
        if exact {
            frame["rng"] = json!(w.random_state(sb)?);
        }
        writeln!(out, "{frame}")?;
        if reload_at == Some(t) {
            // (clockinfra: the judge's chunk-reload check, frame by frame) saved, unloaded and
            // loaded again right after this frame, without a tick
            w.reload(sb)?;
        }
    }
    out.flush()?;
    let _ = w.drop_sandbox(sb);
    Ok((out_path, ticks))
}

pub fn run(a: ExportArgs) -> Result<()> {
    if a.designs.is_empty() {
        bail!("clock misc trace-export <archived designs>...");
    }
    let out_dir = a.out_dir.clone().unwrap_or_else(|| PathBuf::from(if a.exact { "designs/clocks/traces/exact" } else { "designs/clocks/traces" }));
    std::fs::create_dir_all(&out_dir)?;
    let mut w = Worker::spawn(&WorkerConfig::default())?;
    let extra: Vec<(u32, clock::Action, Value)> = a.extra_actions.iter().map(|s| parse_action(s)).collect::<Result<_>>()?;
    for d in &a.designs {
        // --seeds A..B: one trace per sandbox seed (<stem>-s<seed>.trace): a different level random,
        // so different TNT kicks and other random draws, each one checkable by trace-compare
        let seeds: Vec<Option<i64>> = match &a.seeds {
            Some(r) => {
                let (lo, hi) = r.split_once("..").with_context(|| format!("--seeds A..B, got {r:?}"))?;
                let (lo, hi): (i64, i64) = (lo.trim().parse()?, hi.trim().parse()?);
                (lo..=hi).map(Some).collect()
            }
            None => vec![None],
        };
        for s in seeds {
            let (p, ticks) = export_one(&mut w, d, &out_dir, &a, &extra, s)?;
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            println!("{}: {ticks} ticks, {} KB", p.display(), size / 1024);
        }
    }
    Ok(())
}
