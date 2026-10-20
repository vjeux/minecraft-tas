//! `mctas circuit shulker trace`: real-game traces of shulker designs for the in-browser viewer,
//! in the clock traces' format (crates/clock-misc/src/trace_export.rs): line 1 a header
//! (`{"trace":1,"name","design","scenario","seed","period":0,"warmup":0,"ticks":N,
//! "box":[from,to],"loop":[0,N]}`; `seed` is the sandbox's, which the bridge's drops draw from),
//! then one line per gt t = 0..=N: `{"t":t,"blocks":[[x,y,z,"state"],..],
//! "entities":[{"id","type","pos","yaw","pitch"}]}`: at t = 0 every non-air cell of the watched
//! box, later the cells that changed; every entity every tick. The run is the judge's first build
//! (rotation 0, position 0, its seeded drops) on a scenario that fits a replay: 3 boxes of
//! unstackables by default (`--stack`). t = 0 is when the judge's chests are filled and the lever
//! is on; the trace ends when the judge's run does (the last box delivered). The judge's chests
//! are blocks of the trace, their contents are not. `--exact` keeps the game's own doubles and
//! floats plus each entity's motion and network id (for a future redsim gate).

use anyhow::{Context, Result, bail};
use circuit::shulker::{Recorder, Scenario, game_run_recorded};
use circuit::{Circuit, Pos};
use mctas::sim::{Pool, Worker, WorkerConfig};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Lines {
    exact: bool,
    prev: HashMap<Pos, String>,
    ids: HashMap<String, usize>,
    lines: Vec<String>,
    from: Pos,
    to: Pos,
}

fn round(v: f64, digits: i32) -> f64 {
    let m = 10f64.powi(digits);
    (v * m).round() / m
}

impl Recorder for Lines {
    fn tick(&mut self, w: &mut Worker, sb: u32, t: u32, from: Pos, to: Pos) -> Result<()> {
        self.from = from;
        self.to = to;
        let region = w.get_region(sb, from, to)?;
        let mut blocks: Vec<Value> = Vec::new();
        for (p, s) in region.iter() {
            let air = mctas::blockstate::is_air(s);
            let changed = match self.prev.get(&p) {
                Some(x) => x != s,
                None => !air,
            };
            if changed {
                blocks.push(json!([p[0], p[1], p[2], s]));
                self.prev.insert(p, s.to_string());
            }
        }
        let mut ents: Vec<Value> = Vec::new();
        for e in w.entities(sb)? {
            let n = self.ids.len();
            let id = *self.ids.entry(e.uuid.clone()).or_insert(n);
            ents.push(if self.exact {
                json!({"id": id, "net_id": e.net_id, "type": e.kind, "pos": e.pos, "motion": e.motion, "yaw": e.yaw, "pitch": e.pitch})
            } else {
                json!({"id": id, "type": e.kind, "pos": [round(e.pos[0], 4), round(e.pos[1], 4), round(e.pos[2], 4)], "yaw": round(e.yaw as f64, 2), "pitch": round(e.pitch as f64, 2)})
            });
        }
        self.lines.push(json!({"t": t, "blocks": blocks, "entities": ents}).to_string());
        Ok(())
    }
}

fn export_one(w: &mut Worker, path: &Path, out_dir: &Path, stack: u32, exact: bool, margin: i32) -> Result<(PathBuf, u32)> {
    let c = Circuit::load(path)?;
    let kind = c.kind.clone().with_context(|| format!("{}: no `circuit:` line", path.display()))?;
    let base = Scenario::base(&kind).with_context(|| format!("{}: {} is not a shulker kind", path.display(), kind.name()))?;
    let sc = Scenario::with_stack(base.loader, stack);
    let mut rec = Lines { exact, prev: HashMap::new(), ids: HashMap::new(), lines: Vec::new(), from: [0; 3], to: [0; 3] };
    match game_run_recorded(w, &c, &sc, margin, Some(&mut rec))? {
        Ok(log) if !log.timed_out => {}
        Ok(log) => bail!("{}: the run timed out after {} gt", path.display(), log.end),
        Err(why) => bail!("{}: {why}", path.display()),
    }
    let ticks = rec.lines.len().saturating_sub(1) as u32;
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?;
    let header = json!({
        "trace": 1,
        "name": c.name(),
        "design": path.display().to_string(),
        "scenario": format!("{} 3 boxes of {}", if sc.loader { "load" } else { "unload" }, sc.item.trim_start_matches("minecraft:")),
        "seed": circuit::shulker::build_seed(rec.from, rec.to, 0),
        "period": 0,
        "warmup": 0,
        "ticks": ticks,
        "box": [rec.from, rec.to],
        "loop": [0, ticks],
    });
    let out = out_dir.join(format!("{stem}.trace"));
    let mut text = format!("{header}\n");
    for l in &rec.lines {
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(&out, text)?;
    Ok((out, ticks))
}

/// Trace every design, one file each in `out_dir` (default designs/circuits/shulker/traces, or
/// its `exact/` with `exact`).
pub fn trace(designs: &[PathBuf], out_dir: Option<PathBuf>, stack: u32, exact: bool, workers: usize, version: &str, margin: i32) -> Result<()> {
    if designs.is_empty() {
        bail!("circuit shulker trace <designs>...");
    }
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from(if exact { "designs/circuits/shulker/traces/exact" } else { "designs/circuits/shulker/traces" }));
    std::fs::create_dir_all(&out_dir)?;
    let pool = Pool::spawn(workers.min(designs.len()).max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let results = pool.map(designs.to_vec(), |w, _, p| export_one(w, &p, &out_dir, stack, exact, margin));
    let mut failed = 0;
    for (p, r) in designs.iter().zip(results) {
        match r {
            Ok((out, ticks)) => println!("TRACED {} ({ticks} gt) -> {}", p.display(), out.display()),
            Err(e) => {
                failed += 1;
                println!("FAILED {}: {e:#}", p.display());
            }
        }
    }
    if failed > 0 {
        bail!("{failed} of {} designs not traced", designs.len());
    }
    Ok(())
}
