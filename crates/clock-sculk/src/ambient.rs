//! `mctas clock sculk ambient-trace`: one clock's quiet and ambient runs, tick by tick, on redsim
//! and in the game (the design's cells), to find where an ambient game event changes the run
//! (the clock judge's vibration property, `clock::vibration`). Cells are shown short: a sensor as
//! phase + power (`a15`, `c0`, `i0`), a dispenser `T`/`-` (triggered), an observer `1`/`0`. A
//! line ends with `<-` where the quiet and ambient runs differ, or redsim and the game do;
//! `--only` / `--drop` keep or remove ambient events by index to find the one that matters.
//! Found the census-p90 disagreement of 2026-10-20 (redsim and the game agreed on every cell; the
//! property read a block-entity suffix as part of the dispenser's state).

use anyhow::Result;
use clock::{Action, Clock, Pos};
use mctas::sim::{Pool, SandboxOptions, WorkerConfig};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct AmbientArgs {
    design: PathBuf,
    /// Ticks to print (default: the property's horizon).
    #[arg(long)]
    ticks: Option<u32>,
    /// Also run it in the game.
    #[arg(long)]
    game: bool,
    /// Margin of the game sandbox (the judge's 3).
    #[arg(long, default_value_t = 3)]
    margin: i32,
    /// Drop these ambient events (indices, comma separated) before running.
    #[arg(long, default_value = "")]
    drop: String,
    /// Keep only these ambient events (indices, comma separated).
    #[arg(long, default_value = "")]
    only: String,
}

fn short(s: &str) -> String {
    let name = s.trim_start_matches("minecraft:");
    let base = name.split('[').next().unwrap_or(name);
    let prop = |k: &str| name.split(&format!("{k}=")).nth(1).map(|r| r.split([',', ']']).next().unwrap_or("").to_string());
    match base {
        "calibrated_sculk_sensor" | "sculk_sensor" => format!("{}{}", prop("sculk_sensor_phase").unwrap_or_default().chars().next().unwrap_or('?'), prop("power").unwrap_or_default()),
        "dispenser" => (if prop("triggered").as_deref() == Some("true") { "T" } else { "-" }).to_string(),
        "observer" => (if prop("powered").as_deref() == Some("true") { "1" } else { "0" }).to_string(),
        _ => base.chars().take(3).collect(),
    }
}

fn row(cells: &[Pos], get: impl Fn(Pos) -> Option<String>) -> String {
    cells.iter().map(|p| get(*p).map(|s| short(&s)).unwrap_or_else(|| ".".into())).collect::<Vec<_>>().join(" ")
}

fn game_run(pool: &Pool, c: &Clock, extra: &[(u32, Action)], ticks: u32, margin: i32, cells: &[Pos]) -> Result<Vec<String>> {
    let jobs = vec![()];
    let out = pool.map(jobs, |w, _, ()| {
        let (lo, hi) = c.bounds();
        let from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
        let to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
        let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
        let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
        let r = (|| -> Result<Vec<String>> {
            clock::verify::place(w, sb, c)?;
            let mut actions = c.actions();
            actions.extend(extra.iter().cloned());
            actions.sort_by_key(|(t, _)| *t);
            let mut rows = Vec::new();
            for t in 0..=ticks {
                while actions.first().is_some_and(|(at, _)| *at == t) {
                    let (_, a) = actions.remove(0);
                    clock::verify::apply(w, sb, &a)?;
                }
                let f = w.get_region(sb, lo, hi)?;
                rows.push(row(cells, |p| f.get(p).map(str::to_string)));
                if t < ticks {
                    w.step_probe(sb, 1, false, &[], None)?;
                }
            }
            Ok(rows)
        })();
        let _ = w.drop_sandbox(sb);
        r
    });
    out.into_iter().next().unwrap()
}

pub fn ambient(a: &AmbientArgs) -> Result<()> {
    let c = Clock::load(&a.design)?;
    let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
    let horizon = clock::vibration::horizon(r.warmup, r.period);
    let ticks = a.ticks.unwrap_or(horizon);
    let idx = |s: &str| -> Vec<usize> { s.split(',').filter_map(|x| x.trim().parse().ok()).collect() };
    let (drop, only) = (idx(&a.drop), idx(&a.only));
    let extra: Vec<(u32, Action)> = clock::vibration::ambient_actions(&c, r.swept, horizon)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| !drop.contains(i) && (only.is_empty() || only.contains(i)))
        .map(|(_, e)| e)
        .collect();
    let mut cells: Vec<Pos> = c.design.blocks.keys().copied().collect();
    cells.sort();
    println!("{}: warmup {} period {} swept {:?}; cells {:?}", c.name(), r.warmup, r.period, r.swept, cells);
    for (t, e) in &extra {
        if let Action::GameEvent { pos, event } = e {
            if *t <= ticks {
                println!("  event at tick {t}: {event} {pos:?}");
            }
        }
    }
    let (lo, hi) = c.bounds();
    let m = 4;
    let (from, to) = ([lo[0] - m, lo[1] - m, lo[2] - m], [hi[0] + m, hi[1] + m, hi[2] + m]);
    let sim_rows = |extra: &[(u32, Action)]| -> Vec<String> {
        match clock::sim::drive(&c, extra, ticks, &[], from, to) {
            Ok(d) => d.cells.iter().map(|v| row(&cells, |p| v.iter().find(|(q, _)| *q == p).map(|(_, s)| s.clone()))).collect(),
            Err((_, why)) => vec![format!("redsim: {why}")],
        }
    };
    let sq = sim_rows(&[]);
    let sl = sim_rows(&extra);
    println!("simulate's vibration property: {:?}", r.vibration);
    if let Some(o) = r.output.as_ref().map(|o| o.pos) {
        let seq = |extra: &[(u32, Action)]| -> String {
            match clock::sim::drive(&c, extra, ticks, &[o], from, to) {
                Ok(d) => d
                    .cells
                    .iter()
                    .zip(&d.signals)
                    // the bare block state (redsim's cell state carries a block-entity suffix,
                    // `{items=}`, that a property parser must not see)
                    .map(|(cells, sg)| if clock::vibration::output_on(cells.iter().find(|(p, _)| *p == o).map(|(_, s)| clock::wave::block_part(s)), sg[0]) { '1' } else { '0' })
                    .collect(),
                Err((_, why)) => why,
            }
        };
        let (a1, b1) = (seq(&[]), seq(&extra));
        println!("output {o:?} quiet  : {a1}");
        println!("output {o:?} ambient: {b1}");
        let first = a1.chars().zip(b1.chars()).enumerate().skip(r.warmup as usize).find(|(_, (x, y))| x != y).map(|(t, _)| t);
        println!("output first difference after the warmup (this trace): {first:?}");
        let p = clock::vibration::sim_property(&c, 4, r.warmup, r.period, r.swept, Some(o));
        println!("clock::vibration::sim_property (the judge's library): {p:?}");
    }
    let (gq, gl) = if a.game {
        let pool = Pool::spawn(2, &WorkerConfig { version: mctas::DEFAULT_VERSION.to_string(), ..Default::default() })?;
        (game_run(&pool, &c, &[], ticks, a.margin, &cells)?, game_run(&pool, &c, &extra, ticks, a.margin, &cells)?)
    } else {
        (Vec::new(), Vec::new())
    };
    println!("tick | redsim quiet | redsim ambient | game quiet | game ambient");
    for t in 0..=ticks as usize {
        let ev: Vec<String> = extra.iter().filter(|(at, _)| *at as usize == t).filter_map(|(_, e)| if let Action::GameEvent { event, pos } = e { Some(format!("{event}@{pos:?}")) } else { None }).collect();
        let g = |v: &Vec<String>| v.get(t).cloned().unwrap_or_default();
        let line = format!("{t:4} | {} | {} | {} | {}", g(&sq), g(&sl), g(&gq), g(&gl));
        let differs = g(&sq) != g(&sl) || (a.game && (g(&gq) != g(&gl) || g(&sl) != g(&gl)));
        println!("{line}{}{}", if differs { "  <-" } else { "" }, if ev.is_empty() { String::new() } else { format!("   [{}]", ev.join(", ")) });
    }
    Ok(())
}
