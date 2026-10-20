//! `mctas circuit tnt run`: a TNT design in the real game, tick by tick. Placed like the clock
//! judge (`clock::verify::place`), kicked (`clock::verify::apply`); after every tick each new
//! primed TNT is recorded (tick, position, motion, saved data) and killed by its UUID, so it never
//! explodes; the TNT blocks standing are counted and the box's frames give the period.

use crate::sim::TntRun;
use anyhow::Result;
use clock::{Clock, Pos};
use mctas::sim::{SandboxOptions, Worker};
use redsim::BlockPos;
use std::collections::HashSet;

/// One primed TNT as the game spawned it.
#[derive(Clone, Debug)]
pub struct Spawn {
    /// The tick after which it was first seen (tick 1 = after the first tick).
    pub tick: u32,
    pub pos: [f64; 3],
    pub motion: [f64; 3],
    pub state: String,
}

/// A real-game run: the summary (same shape as redsim's) and every spawn.
pub struct GameRun {
    pub run: TntRun,
    pub spawns: Vec<Spawn>,
}

/// Run one build (already turned and moved) for `ticks` gt in a fresh sandbox.
pub fn run_one(w: &mut Worker, c: &Clock, ticks: u32, margin: i32) -> Result<GameRun> {
    let (lo, hi) = c.bounds();
    let from: Pos = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let to: Pos = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = run_in(w, sb, c, ticks, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in(w: &mut Worker, sb: u32, c: &Clock, ticks: u32, from: Pos, to: Pos) -> Result<GameRun> {
    clock::verify::place(w, sb, c)?;
    let mut actions = c.actions();
    let last_action = actions.last().map(|(t, _)| *t).unwrap_or(0);
    let mut out = TntRun::default();
    let mut spawns: Vec<Spawn> = Vec::new();
    let mut seen_tnt: HashSet<String> = HashSet::new();
    // block frames only (moving blocks' progress is not in them): the period is read at the end,
    // from frames that repeat through the whole second half of the run
    let mut frames: Vec<u64> = Vec::new();
    let count = |f: &mctas::sim::Region| f.iter().filter(|(_, s)| s.starts_with("minecraft:tnt")).count();
    out.tnt.push(count(&w.get_region(sb, from, to)?));
    for t in 0..ticks {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            clock::verify::apply(w, sb, &a)?;
        }
        w.step(sb, 1)?;
        out.ticks = t + 1;
        // a TNT primed by the kick itself shows up after this tick too (same as redsim's count)
        for e in w.entities(sb)? {
            if e.kind.trim_start_matches("minecraft:") == "tnt" && seen_tnt.insert(e.uuid.clone()) {
                let cell = BlockPos::new(e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32);
                out.primes.push((t + 1, cell));
                spawns.push(Spawn { tick: t + 1, pos: e.pos, motion: e.motion, state: e.state.clone() });
                w.exec(sb, &format!("kill {}", e.uuid))?;
            }
        }
        let f = w.get_region(sb, from, to)?;
        out.tnt.push(count(&f));
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for (p, s) in f.iter() {
            if !mctas::blockstate::is_air(s) {
                std::hash::Hash::hash(&(p, s), &mut h);
            }
        }
        frames.push(std::hash::Hasher::finish(&h));
    }
    out.period = frame_period(&frames, last_action);
    Ok(GameRun { run: out, spawns })
}

/// The smallest period p (and the first tick from which frames repeat with it) such that every
/// frame of the run's second half equals the one p ticks later; frames[i] = after tick i + 1.
pub fn frame_period(frames: &[u64], last_action: u32) -> Option<(u32, u32)> {
    let n = frames.len();
    let half = (n / 2).max(last_action as usize);
    for p in 1..=(n.saturating_sub(half)) / 2 {
        if (half..n - p).all(|i| frames[i] == frames[i + p]) {
            let mut start = half;
            while start > last_action as usize && frames[start - 1] == frames[start - 1 + p] {
                start -= 1;
            }
            return Some((start as u32 + 1, p as u32));
        }
    }
    None
}
