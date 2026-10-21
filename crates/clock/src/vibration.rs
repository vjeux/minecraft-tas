//! The vibration test's shared parts (the circuit judge's `circuit::vibration`, and the clock
//! judge's ambient property here): a sculk sensor hears game events, so a design that holds one is
//! also run with AMBIENT game events around it, seeded by the build itself (the design as rotated
//! and placed) so redsim and the game see the same events for the same build. Every event is posted
//! at its cell's center (redsim measures block centers).
//!
//! For a clock (no input device) the property is `vibration-proof yes / no`, never a failure: the
//! quiet run and the ambient run must give the same output on/off sequence from the judged warmup to
//! the horizon (the same period, the same phase). [`sim_property`] on redsim, [`game_property`] in
//! the game; `mctas clock vibration` writes them to `designs/clocks/archive/vibration.tsv`.

use crate::{Action, Clock, Pos};
use anyhow::Result;
use mctas::design::Design;
use mctas::sim::{SandboxOptions, Worker};

/// The ambient events (26.3 `VibrationSystem` frequencies 1, 2, 13, 12, 14): a footstep, an item
/// hitting the ground, a block placed, a block broken, an entity placed.
pub const AMBIENT_EVENTS: [&str; 5] = ["step", "hit_ground", "block_place", "block_destroy", "entity_place"];

/// A plain sculk sensor's listener radius (a calibrated one hears twice as far).
pub const AMBIENT_REACH: i32 = 8;

/// Whether a design holds a sculk sensor (plain or calibrated).
pub fn has_sculk(d: &Design) -> bool {
    d.blocks.values().any(|s| s.contains("sculk_sensor"))
}

/// The cells around a swept box (every cell a run ever held a block in) where ambient events
/// happen: within the design's sensors' listener radius of the box, never inside it.
pub fn ambient_cells(d: &Design, swept: (Pos, Pos)) -> Vec<Pos> {
    let r = if d.blocks.values().any(|s| s.contains("calibrated_sculk_sensor")) { 2 * AMBIENT_REACH } else { AMBIENT_REACH };
    let (lo, hi) = swept;
    let mut v = Vec::new();
    for x in lo[0] - r..=hi[0] + r {
        for y in lo[1] - r..=hi[1] + r {
            for z in lo[2] - r..=hi[2] + r {
                let inside = (lo[0]..=hi[0]).contains(&x) && (lo[1]..=hi[1]).contains(&y) && (lo[2]..=hi[2]).contains(&z);
                if !inside {
                    v.push([x, y, z]);
                }
            }
        }
    }
    v
}

/// The build's own seed: its blocks as placed (positions and states: rotation and position
/// included).
pub fn seed(d: &Design) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: &[u8]| {
        for x in b {
            h ^= *x as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    for (p, s) in &d.blocks {
        for a in p {
            eat(&a.to_le_bytes());
        }
        eat(s.as_bytes());
    }
    h
}

/// Ambient events from tick `from` to `to`: one every 5..15 gt at a cell of `cells`, seeded by the
/// build (`seed`) and the horizon: (tick, cell, event).
pub fn ambient_events(seed: u64, from: u32, to: u32, cells: &[Pos]) -> Vec<(u32, Pos, &'static str)> {
    let mut out = Vec::new();
    if cells.is_empty() {
        return out;
    }
    let mut state = seed ^ (to as u64);
    let mut next = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 33) as usize
    };
    let mut t = from;
    loop {
        t += 5 + (next() % 11) as u32;
        if t >= to {
            break;
        }
        let ev = AMBIENT_EVENTS[next() % AMBIENT_EVENTS.len()];
        out.push((t, cells[next() % cells.len()], ev));
    }
    out
}

/// A clock's ambient property: whether ambient game events leave its output alone from the judged
/// warmup to the horizon, and the first tick where they do not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    pub proof: bool,
    pub first_diff: Option<u32>,
}

impl Property {
    pub fn word(&self) -> &'static str {
        if self.proof { "yes" } else { "no" }
    }
}

/// How long the property runs: four periods after the warmup (at least 40 gt), at most 4000 gt.
pub fn horizon(warmup: u32, period: u32) -> u32 {
    (warmup + 4 * period.max(1)).max(warmup + 40).min(4000)
}

/// The clock's ambient events as judge actions (the swept box from its quiet run).
pub fn ambient_actions(c: &Clock, swept: (Pos, Pos), ticks: u32) -> Vec<(u32, Action)> {
    let cells = ambient_cells(&c.design, swept);
    ambient_events(seed(&c.design), c.free_from(), ticks, &cells).into_iter().map(|(t, pos, event)| (t, Action::GameEvent { pos, event })).collect()
}

/// The output's on/off (the judge's rule: a self-reporting component's own state, else the best
/// signal into the cell). A recorded state may carry its block-entity part (`...]{items=}`, as
/// redsim's frames do): only the block state is read (clk-sculk caught the last property reading
/// `true]{items=}` and every dispenser output off).
pub fn output_on(state: Option<&str>, best: u8) -> bool {
    match state.map(crate::wave::block_part) {
        Some(s) if crate::wave::family_name(s).is_some() => crate::wave::active(s),
        _ => best > 0,
    }
}

fn first_diff(a: &[bool], b: &[bool], from: u32) -> Option<u32> {
    (from as usize..a.len().max(b.len())).find(|&t| a.get(t) != b.get(t)).map(|t| t as u32)
}

/// The output on/off sequence of a redsim run with `extra` inputs, ticks 0..=ticks.
fn sim_output(c: &Clock, extra: &[(u32, Action)], ticks: u32, out: Pos, from: Pos, to: Pos) -> Option<Vec<bool>> {
    let d = crate::sim::drive(c, extra, ticks, &[out], from, to).ok()?;
    if d.stopped.is_some() {
        return None;
    }
    Some(d.cells.iter().zip(&d.signals).map(|(cells, sg)| output_on(cells.iter().find(|(p, _)| *p == out).map(|(_, s)| s.as_str()), sg[0])).collect())
}

/// The property on redsim (`warmup`, `period`: the quiet judge's; `swept`: its swept box). `None`
/// when the design holds no sensor, declares no output or redsim cannot run it.
pub fn sim_property(c: &Clock, margin: i32, warmup: u32, period: u32, swept: (Pos, Pos), out: Option<Pos>) -> Option<Property> {
    if !has_sculk(&c.design) {
        return None;
    }
    let out = out?;
    let (lo, hi) = c.bounds();
    let from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let ticks = horizon(warmup, period);
    let quiet = sim_output(c, &[], ticks, out, from, to)?;
    let loud = sim_output(c, &ambient_actions(c, swept, ticks), ticks, out, from, to)?;
    let d = first_diff(&quiet, &loud, warmup);
    Some(Property { proof: d.is_none(), first_diff: d })
}

/// The output on/off sequence of a game run (a fresh sandbox) with `extra` inputs, ticks 0..=ticks.
fn game_output(w: &mut Worker, c: &Clock, extra: &[(u32, Action)], ticks: u32, out: Pos, margin: i32) -> Result<Vec<bool>> {
    let (lo, hi) = c.bounds();
    let from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = (|| -> Result<Vec<bool>> {
        crate::verify::place(w, sb, c)?;
        let mut actions = c.actions();
        actions.extend(extra.iter().cloned());
        actions.sort_by_key(|(t, _)| *t);
        let mut seq: Vec<bool> = Vec::new();
        let mut t = 0u32;
        loop {
            while actions.first().is_some_and(|(at, _)| *at == t) {
                let (_, a) = actions.remove(0);
                crate::verify::apply(w, sb, &a)?;
            }
            let f = w.get_region(sb, out, out)?;
            let st = f.palette.get(*f.data.first().unwrap_or(&0) as usize).cloned();
            let sg = w.signals(sb, &[out])?.first().map(|(a, _)| *a).unwrap_or(0);
            let on = output_on(st.as_deref(), sg);
            if seq.len() as u32 == t + 1 {
                *seq.last_mut().unwrap() = on;
            } else {
                seq.push(on);
            }
            let next = actions.first().map(|(at, _)| *at).unwrap_or(ticks).min(ticks).min(t + 500);
            if next <= t {
                break;
            }
            let r = w.step_probe(sb, next - t, false, &[out], Some((out, out)))?;
            for (f, sg) in r.frames.iter().zip(&r.signals) {
                let st = f.palette.get(*f.data.first().unwrap_or(&0) as usize).cloned();
                seq.push(output_on(st.as_deref(), sg.first().copied().unwrap_or(0)));
            }
            t = next;
        }
        Ok(seq)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// The property in the game, for the clock as built (`swept`: its swept box, from redsim or the
/// quiet game run). `None` when the design holds no sensor or declares no output.
pub fn game_property(w: &mut Worker, c: &Clock, margin: i32, warmup: u32, period: u32, swept: (Pos, Pos), out: Option<Pos>) -> Result<Option<Property>> {
    if !has_sculk(&c.design) {
        return Ok(None);
    }
    let Some(out) = out else { return Ok(None) };
    let ticks = horizon(warmup, period);
    let quiet = game_output(w, c, &[], ticks, out, margin)?;
    let loud = game_output(w, c, &ambient_actions(c, swept, ticks), ticks, out, margin)?;
    let d = first_diff(&quiet, &loud, warmup);
    Ok(Some(Property { proof: d.is_none(), first_diff: d }))
}

/// One row of `designs/clocks/archive/vibration.tsv`: a sculk clock and its property on redsim
/// (and in the game when spot-checked).
#[derive(Clone, Debug)]
pub struct Row {
    pub file: String,
    pub period: u32,
    pub warmup: u32,
    pub swept: (Pos, Pos),
    /// The judged output cell (declared, or the judge's pick).
    pub out: Option<Pos>,
    pub sim: Option<Property>,
    pub game: Option<Property>,
    /// Why there is no property (not a clock on redsim, no output, a run redsim stops).
    pub note: String,
}

impl Row {
    fn cell(p: &Option<Property>) -> String {
        match p {
            Some(p) => match p.first_diff {
                Some(t) => format!("no\t{t}"),
                None => "yes\t-".to_string(),
            },
            None => "-\t-".to_string(),
        }
    }

    pub fn line(&self) -> String {
        format!("{}\t{}\t{}\t{}\t{}", self.file, self.period, Row::cell(&self.sim), Row::cell(&self.game), self.note)
    }
}

/// The TSV header of `vibration.tsv`.
pub const TSV_HEADER: &str = "file\tperiod\tredsim proof\tredsim first difference\tgame proof\tgame first difference\tnote";

/// The redsim property of every sculk clock among `paths`, on `threads` threads (designs without a
/// sensor are skipped).
pub fn sim_rows(paths: &[std::path::PathBuf], threads: usize) -> Vec<Row> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let rows: Mutex<Vec<Row>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = paths.get(i) else { break };
                    let Ok(c) = Clock::load(path) else { continue };
                    if !has_sculk(&c.design) {
                        continue;
                    }
                    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                    let r = crate::sim::simulate(&c, &crate::sim::SimOptions::default());
                    let out = r.output.as_ref().map(|o| o.pos);
                    let note = if r.verdict != crate::sim::Verdict::Clock {
                        format!("not a clock on redsim: {}", r.why)
                    } else if out.is_none() {
                        "no output".to_string()
                    } else if r.vibration.is_none() {
                        "redsim could not run the ambient comparison".to_string()
                    } else {
                        String::new()
                    };
                    rows.lock().unwrap().push(Row { file, period: r.period, warmup: r.warmup, swept: r.swept, out, sim: r.vibration.clone(), game: None, note });
                }
            });
        }
    });
    let mut v = rows.into_inner().unwrap();
    v.sort_by(|a, b| a.file.cmp(&b.file));
    v
}

/// The game property of the rows at `picks` (indices), each clock as built, on a worker pool.
pub fn game_rows(rows: &mut [Row], dir: &std::path::Path, picks: &[usize], workers: usize, version: &str) -> Result<()> {
    let o = crate::verify::VerifyOptions { workers, version: version.to_string(), ..Default::default() };
    let pool = crate::verify::spawn_pool(&o)?;
    let jobs: Vec<(usize, std::path::PathBuf, u32, u32, (Pos, Pos), Option<Pos>)> = picks.iter().filter_map(|&i| rows.get(i).map(|r| (i, dir.join(&r.file), r.warmup, r.period, r.swept, r.out))).collect();
    let results = pool.map(jobs.clone(), |w, _, (_, path, warmup, period, swept, out)| {
        let c = Clock::load(&path)?;
        game_property(w, &c, o.margin, warmup, period, swept, out)
    });
    for ((i, ..), r) in jobs.into_iter().zip(results) {
        match r {
            Ok(p) => rows[i].game = p,
            Err(e) => rows[i].note = format!("game: {e}"),
        }
    }
    Ok(())
}
