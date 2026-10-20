//! `mctas circuit tnt sim`: run a TNT design in redsim (milliseconds). Placed like the clock
//! judge (raw writes, a shape update of every wire, a `self` update of every block; a `last`
//! kick's block withheld), kicked, then run tick by tick: every TNT primed (`World::primed`), the
//! TNT blocks standing, and the first repeated state (the period).

use clock::{Action, Clock, Placement, Pos};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// What a run saw.
#[derive(Clone, Debug, Default)]
pub struct TntRun {
    /// (tick, cell) of every prime, tick 0 = the first tick after placement.
    pub primes: Vec<(u32, BlockPos)>,
    /// TNT blocks standing after each tick (index 0 = after placement).
    pub tnt: Vec<usize>,
    /// The first repeated state: (first seen at tick, period).
    pub period: Option<(u32, u32)>,
    pub ticks: u32,
    pub errors: Vec<String>,
    pub drops: usize,
}

impl TntRun {
    /// Primes per period once periodic (None before a period is found).
    pub fn per_period(&self) -> Option<usize> {
        let (start, p) = self.period?;
        Some(self.primes.iter().filter(|(t, _)| *t > start && *t <= start + p).count())
    }

    /// One line: DUPES / CONSUMES / NOTHING with the counts.
    pub fn line(&self) -> String {
        let first = self.tnt.first().copied().unwrap_or(0);
        let min = self.tnt.iter().copied().min().unwrap_or(0);
        let last = self.tnt.last().copied().unwrap_or(0);
        // the TNT blocks standing over the last period (a moved copy is in flight for 2 gt)
        let settled = match self.period {
            Some((_, p)) => self.tnt.iter().rev().take(p as usize + 1).copied().max().unwrap_or(0),
            None => last,
        };
        let verdict = if self.primes.is_empty() {
            "NOTHING"
        } else if settled >= first && self.period.is_some() && self.per_period().unwrap_or(0) > 0 {
            "DUPES"
        } else if settled >= first {
            "PRIMES"
        } else {
            "CONSUMES"
        };
        let mut s = format!("{verdict}: {} primed in {} gt, TNT blocks {first} -> {settled} (min {min})", self.primes.len(), self.ticks);
        if let Some((start, p)) = self.period {
            s += &format!(", periodic from gt {start} with period {p}");
            if let Some(n) = self.per_period() {
                s += &format!(" ({n} per period, {:.1} TNT/min)", n as f64 * 1200.0 / p as f64);
            }
        }
        let firsts: Vec<String> = self.primes.iter().take(6).map(|(t, p)| format!("gt {t} at {p}")).collect();
        if !firsts.is_empty() {
            s += &format!("; first: {}", firsts.join(", "));
        }
        if self.drops > 0 {
            s += &format!("; {} drops", self.drops);
        }
        if !self.errors.is_empty() {
            s += &format!("; UNMODELED: {}", self.errors.join("; "));
        }
        s
    }
}

/// The design's world: its box plus `margin`, placed like the judge (kick block withheld).
pub fn build(c: &Clock, margin: i32) -> Result<World, String> {
    let (lo, hi) = c.design.bounds().ok_or("empty design")?;
    let lo = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let hi = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let mut w = World::new(bp(lo), [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]);
    // the judge removes every primed TNT before it explodes
    w.remove_primed = true;
    let last = c.withheld().map(bp);
    let mut blocks = Vec::new();
    for (p, s) in &c.design.blocks {
        let b = Block::parse(s).map_err(|e| format!("not modeled by redsim: {e}"))?;
        if Some(bp(*p)) != last {
            blocks.push((bp(*p), b));
        }
    }
    match c.placement {
        Placement::Judge => {
            w.place_raw(&blocks);
            w.place_updates(&blocks);
        }
        Placement::Player => {
            for (p, b) in &blocks {
                w.set_block(*p, *b, flags::ALL);
            }
        }
    }
    Ok(w)
}

fn apply_due(w: &mut World, actions: &mut Vec<(u32, Action)>, t: u32) -> Result<(), String> {
    while actions.first().is_some_and(|(at, _)| *at == t) {
        let (_, a) = actions.remove(0);
        match a {
            Action::Place { pos, state, .. } => {
                let b = Block::parse(&state).map_err(|e| format!("kick block not modeled: {e}"))?;
                w.set_block(bp(pos), b, flags::ALL);
            }
            Action::Use { pos } => {
                if !w.use_block(bp(pos)) {
                    return Err(format!("a right-click on {} is not modeled", w.get(bp(pos))));
                }
            }
            Action::Clear { pos } => {
                w.set_block(bp(pos), Block::Air, flags::ALL);
            }
            // the BUD judge's actions (study 10), applied like clock::sim does
            Action::ShapeSet { pos, state } => {
                let b = Block::parse(&state).map_err(|e| format!("shape-set block not modeled: {e}"))?;
                w.set_block(bp(pos), b, flags::CLIENTS);
            }
            Action::Update { pos } => {
                let src = w.get(bp(pos)).kind();
                w.update_neighbors_at(bp(pos), src);
            }
        }
    }
    Ok(())
}

fn tnt_count(w: &World) -> usize {
    w.blocks().iter().filter(|(_, b)| matches!(b, Block::Tnt { .. })).count()
}

/// Run `ticks` gt (unless `full`, stops once a period is found and run 2 more periods). `trace`:
/// print every cell change of the first N ticks.
pub fn run(c: &Clock, ticks: u32, margin: i32, trace: u32, full: bool) -> TntRun {
    let mut out = TntRun::default();
    let mut w = match build(c, margin) {
        Ok(w) => w,
        Err(e) => {
            out.errors.push(e);
            return out;
        }
    };
    let mut actions = c.actions();
    let last_action = actions.last().map(|(t, _)| *t).unwrap_or(0);
    let mut seen: HashMap<Vec<u8>, u32> = HashMap::new();
    out.tnt.push(tnt_count(&w));
    let mut prev = w.blocks();
    let mut stop_at = ticks;
    for t in 0..ticks {
        let n0 = w.primed.len();
        if let Err(e) = apply_due(&mut w, &mut actions, t) {
            out.errors.push(e);
            break;
        }
        w.tick();
        for &(_, p) in &w.primed[n0..] {
            out.primes.push((t + 1, p));
        }
        out.tnt.push(tnt_count(&w));
        out.ticks = t + 1;
        if t < trace {
            let now = w.blocks();
            let before: HashMap<BlockPos, Block> = prev.iter().copied().collect();
            let after: HashMap<BlockPos, Block> = now.iter().copied().collect();
            let mut changes: Vec<String> = Vec::new();
            for (p, b) in &after {
                if before.get(p) != Some(b) {
                    changes.push(format!("{p} {}", clock::short(&b.to_string())));
                }
            }
            for p in before.keys() {
                if !after.contains_key(p) {
                    changes.push(format!("{p} air"));
                }
            }
            changes.sort();
            let primed: Vec<String> = w.primed[n0..].iter().map(|(_, p)| p.to_string()).collect();
            println!("  gt {}: {}{}", t + 1, changes.join(", "), if primed.is_empty() { String::new() } else { format!("  PRIMED {}", primed.join(" ")) });
            prev = now;
        }
        if !w.errors.is_empty() {
            out.errors = w.errors.clone();
            break;
        }
        if out.period.is_none() && t + 1 > last_action {
            let key = w.state_key();
            if let Some(&first) = seen.get(&key) {
                out.period = Some((first, t + 1 - first));
                if !full {
                    stop_at = (t + 1 + 2 * (t + 1 - first)).min(ticks);
                }
            } else {
                seen.insert(key, t + 1);
            }
        }
        if t + 1 >= stop_at {
            break;
        }
    }
    out.drops = w.drops.len();
    out
}
