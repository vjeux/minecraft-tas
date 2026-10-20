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
    /// game's own doubles / floats (no rounding), plus each entity's "motion".
    #[arg(long)]
    pub exact: bool,
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

fn export_one(w: &mut Worker, path: &Path, out_dir: &Path, margin: i32, exact: bool) -> Result<(PathBuf, u32)> {
    let file = path.file_name().and_then(|s| s.to_str()).context("file name")?.to_string();
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?.to_string();
    let (period, warmup) = verdict(&file)?;
    let ticks = warmup + 2 * period;
    let c = Clock::load(path).with_context(|| format!("load {}", path.display()))?;
    let (lo, hi) = c.bounds();
    let from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    clock::verify::place(w, sb, &c)?;
    let out_path = out_dir.join(format!("{stem}.trace"));
    let mut out = std::io::BufWriter::new(std::fs::File::create(&out_path)?);
    let mut header = json!({
        "trace": 1,
        "name": c.design.name,
        "design": format!("designs/clocks/archive/{file}"),
        "period": period,
        "warmup": warmup,
        "ticks": ticks,
        "box": [from, to],
        "loop": [warmup, warmup + period],
    });
    if let Some(o) = c.output {
        header["output"] = json!(o);
    }
    writeln!(out, "{header}")?;
    let mut prev: HashMap<[i32; 3], String> = HashMap::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut actions = c.actions();
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
        writeln!(out, "{}", json!({"t": t, "blocks": blocks, "entities": ents}))?;
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
    for d in &a.designs {
        let (p, ticks) = export_one(&mut w, d, &out_dir, a.margin, a.exact)?;
        let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
        println!("{}: {ticks} ticks, {} KB", p.display(), size / 1024);
    }
    Ok(())
}
