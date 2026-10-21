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
        // exact traces: the level random's state after the frame (clk-misc: redsim's mirror is
        // checked against it every frame)
        let mut frame = json!({"t": t, "blocks": blocks, "entities": ents});
        if self.exact {
            frame["rng"] = json!(w.random_state(sb)?);
        }
        self.lines.push(frame.to_string());
        Ok(())
    }
}

fn export_one(w: &mut Worker, path: &Path, out_dir: &Path, stack: u32, exact: bool, margin: i32, stress: Option<u32>) -> Result<(PathBuf, u32, Option<String>)> {
    let c = Circuit::load(path)?;
    let kind = c.kind.clone().with_context(|| format!("{}: no `circuit:` line", path.display()))?;
    let base = Scenario::base(&kind).with_context(|| format!("{}: {} is not a shulker kind", path.display(), kind.name()))?;
    let mut sc = Scenario::with_stack(base.loader, stack);
    let mut turns = 0u8;
    // `--stress-run K`: the judge's drop stress run K (shulker::drop_stress): its drop seed, a
    // burst every other run (loaders), its rotation; the trace is written even when it fails
    if let Some(k) = stress {
        let sorts = c.stores.iter().any(|s| s.role == "reject");
        sc = Scenario::with_stack(base.loader, if sorts { base.stack } else { 1 });
        sc.seed_salt = k as i64;
        if base.loader && k % 2 == 1 {
            sc.burst = true;
            sc.spare = 0;
        }
        turns = (k % 4) as u8;
    }
    let rc = c.rotated(w, turns)?;
    let mut rec = Lines { exact, prev: HashMap::new(), ids: HashMap::new(), lines: Vec::new(), from: [0; 3], to: [0; 3] };
    let verdict = match game_run_recorded(w, &rc, &sc, margin, Some(&mut rec))? {
        Ok(log) if !log.timed_out => {
            let out = circuit::shulker::judge(&sc, &log);
            if out.pass { None } else { Some(out.why) }
        }
        Ok(log) => Some(format!("the run timed out after {} gt", log.end)),
        Err(why) => Some(why),
    };
    if let (Some(why), None) = (&verdict, stress) {
        bail!("{}: {why}", path.display());
    }
    let ticks = rec.lines.len().saturating_sub(1) as u32;
    let stem = path.file_stem().and_then(|s| s.to_str()).context("file stem")?;
    let header = json!({
        "trace": 1,
        "name": c.name(),
        "design": path.display().to_string(),
        "scenario": format!("{} 3 boxes of {}{}", if sc.loader { "load" } else { "unload" }, sc.item.trim_start_matches("minecraft:"), match stress { Some(k) => format!(", drop stress run {k} (rotation {turns}{})", if sc.burst { ", burst" } else { "" }), None => String::new() }),
        "seed": circuit::shulker::build_seed(rec.from, rec.to, sc.seed_salt),
        "chunks": [rec.from[0].div_euclid(16), rec.from[2].div_euclid(16), rec.to[0].div_euclid(16), rec.to[2].div_euclid(16)],
        "period": 0,
        "warmup": 0,
        "ticks": ticks,
        "box": [rec.from, rec.to],
        "loop": [0, ticks],
    });
    let out = out_dir.join(match stress {
        Some(k) => format!("{stem}-stress{k}.trace"),
        None => format!("{stem}.trace"),
    });
    let mut text = format!("{header}\n");
    for l in &rec.lines {
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(&out, text)?;
    Ok((out, ticks, verdict))
}

/// `circuit shulker trace-entities <trace>`: every entity of a trace, one line each: its type,
/// the first and last gt it was seen, where it appeared and where it was last (a design aid: a
/// drop that never reached its hopper shows as one alive at the end).
pub fn entities(path: &Path, only: Option<u64>) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("{}", path.display()))?;
    // id -> (type, first t, first pos, last t, last pos)
    let mut seen: std::collections::BTreeMap<u64, (String, u64, Vec<f64>, u64, Vec<f64>)> = std::collections::BTreeMap::new();
    let mut last_t = 0u64;
    let fmt = |p: &[f64]| p.iter().map(|x| format!("{x:.2}")).collect::<Vec<_>>().join(",");
    for line in text.lines().skip(1) {
        let v: Value = serde_json::from_str(line)?;
        let t = v["t"].as_u64().unwrap_or(0);
        last_t = t;
        for e in v["entities"].as_array().into_iter().flatten() {
            let id = e["id"].as_u64().unwrap_or(0);
            let kind = e["type"].as_str().unwrap_or("?").to_string();
            let pos: Vec<f64> = e["pos"].as_array().into_iter().flatten().filter_map(|x| x.as_f64()).collect();
            if only == Some(id) && pos.len() == 3 {
                // the cells that changed this gt within 2 blocks of the entity
                let near: Vec<String> = v["blocks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|b| (0..3).all(|i| (b[i].as_f64().unwrap_or(1e9) + 0.5 - pos[i]).abs() <= 2.5))
                    .map(|b| format!("{},{},{} {}", b[0], b[1], b[2], b[3].as_str().unwrap_or("").trim_start_matches("minecraft:")))
                    .collect();
                println!("{t:>5} {}{}", fmt(&pos), if near.is_empty() { String::new() } else { format!("  | {}", near.join("; ")) });
            }
            let row = seen.entry(id).or_insert_with(|| (kind, t, pos.clone(), t, pos.clone()));
            row.3 = t;
            row.4 = pos;
        }
    }
    if only.is_some() {
        return Ok(());
    }
    for (id, (kind, t0, p0, t1, p1)) in &seen {
        println!("{id:>4} {kind} t {t0}..{t1}{} from {} to {}", if *t1 == last_t { " (alive at the end)" } else { "" }, fmt(p0), fmt(p1));
    }
    Ok(())
}

/// Trace every design, one file each in `out_dir` (default designs/circuits/shulker/traces, or
/// its `exact/` with `exact`). With `stress`, the judge's drop stress run of that number instead
/// (a design aid: where a drop goes when the run fails).
#[allow(clippy::too_many_arguments)]
pub fn trace(designs: &[PathBuf], out_dir: Option<PathBuf>, stack: u32, exact: bool, workers: usize, version: &str, margin: i32, stress: Option<u32>) -> Result<()> {
    if designs.is_empty() {
        bail!("circuit shulker trace <designs>...");
    }
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from(if exact { "designs/circuits/shulker/traces/exact" } else { "designs/circuits/shulker/traces" }));
    std::fs::create_dir_all(&out_dir)?;
    let pool = Pool::spawn(workers.min(designs.len()).max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let results = pool.map(designs.to_vec(), |w, _, p| export_one(w, &p, &out_dir, stack, exact, margin, stress));
    let mut failed = 0;
    for (p, r) in designs.iter().zip(results) {
        match r {
            Ok((out, ticks, None)) => println!("TRACED {} ({ticks} gt) -> {}", p.display(), out.display()),
            Ok((out, ticks, Some(why))) => println!("TRACED {} ({ticks} gt, the run FAILS: {why}) -> {}", p.display(), out.display()),
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
