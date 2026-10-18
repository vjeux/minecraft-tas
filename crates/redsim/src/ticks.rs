//! Scheduled block ticks: `LevelTicks` + `LevelChunkTicks` + `ScheduledTick`.
//!
//! * One container per chunk, a queue ordered by (trigger tick, priority, sub-tick order) and a
//!   set of (position, block) that refuses a second pending tick for the same pair.
//! * Each tick, every due tick is collected first (containers merged by (priority, sub-tick
//!   order) of their next tick: `INTRA_TICK_DRAIN_ORDER`), then run in that order; ticks
//!   scheduled meanwhile wait for a later tick even with delay 0.
//! * `willTickThisTick` = collected for this tick and not run yet; `hasScheduledTick` = still in
//!   its container (a collected tick no longer is).

use crate::block::Kind;
use crate::pos::BlockPos;
use std::collections::{BTreeSet, HashMap, HashSet};

/// `TickPriority` (lower runs first).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Priority {
    ExtremelyHigh = -3,
    VeryHigh = -2,
    High = -1,
    Normal = 0,
    Low = 1,
    VeryLow = 2,
    ExtremelyLow = 3,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Tick {
    pub kind: Kind,
    pub pos: BlockPos,
    pub trigger: i64,
    pub priority: Priority,
    pub sub: i64,
}

#[derive(Default, Clone)]
struct Container {
    /// (trigger, priority, sub) -> tick: `DRAIN_ORDER`.
    queue: BTreeSet<(i64, Priority, i64, BlockPos, Kind)>,
    pending: HashSet<(BlockPos, Kind)>,
}

#[derive(Default, Clone)]
pub struct LevelTicks {
    containers: HashMap<(i32, i32), Container>,
    to_run: HashSet<(BlockPos, Kind)>,
}

impl LevelTicks {
    pub fn schedule(&mut self, kind: Kind, pos: BlockPos, trigger: i64, priority: Priority, sub: i64) {
        let c = self.containers.entry(pos.chunk()).or_default();
        if c.pending.insert((pos, kind)) {
            c.queue.insert((trigger, priority, sub, pos, kind));
        }
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, kind: Kind) -> bool {
        self.containers.get(&pos.chunk()).is_some_and(|c| c.pending.contains(&(pos, kind)))
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, kind: Kind) -> bool {
        self.to_run.contains(&(pos, kind))
    }

    /// Pending ticks (all containers).
    pub fn count(&self) -> usize {
        self.containers.values().map(|c| c.queue.len()).sum()
    }

    /// Every pending tick, for debugging and comparisons.
    pub fn all(&self) -> Vec<Tick> {
        let mut v: Vec<Tick> = self
            .containers
            .values()
            .flat_map(|c| c.queue.iter().map(|&(trigger, priority, sub, pos, kind)| Tick { kind, pos, trigger, priority, sub }))
            .collect();
        v.sort_by_key(|t| (t.trigger, t.priority, t.sub));
        v
    }

    /// Collect the ticks due at `now`, in run order (`collectTicks`).
    pub fn collect(&mut self, now: i64) -> Vec<Tick> {
        let mut out = Vec::new();
        // Containers whose head is due, merged by the head's (priority, sub).
        loop {
            let mut best: Option<((Priority, i64), (i32, i32))> = None;
            for (key, c) in &self.containers {
                if let Some(&(trigger, pr, sub, _, _)) = c.queue.first()
                    && trigger <= now
                    && best.is_none_or(|(b, _)| (pr, sub) < b)
                {
                    best = Some(((pr, sub), *key));
                }
            }
            let Some((_, key)) = best else { break };
            // Take the head, then keep draining this container while its next tick is due and
            // not after the best head of the other containers (`drainFromCurrentContainer`).
            let other_best: Option<(Priority, i64)> = self
                .containers
                .iter()
                .filter(|(k, _)| **k != key)
                .filter_map(|(_, c)| c.queue.first().filter(|h| h.0 <= now).map(|h| (h.1, h.2)))
                .min();
            let c = self.containers.get_mut(&key).unwrap();
            let first = c.queue.pop_first().unwrap();
            c.pending.remove(&(first.3, first.4));
            out.push(Tick { kind: first.4, pos: first.3, trigger: first.0, priority: first.1, sub: first.2 });
            while let Some(&h) = c.queue.first() {
                if h.0 > now || other_best.is_some_and(|ob| (h.1, h.2) > ob) {
                    break;
                }
                c.queue.pop_first();
                c.pending.remove(&(h.3, h.4));
                out.push(Tick { kind: h.4, pos: h.3, trigger: h.0, priority: h.1, sub: h.2 });
            }
        }
        self.to_run = out.iter().map(|t| (t.pos, t.kind)).collect();
        out
    }

    /// A collected tick is about to run (`runCollectedTicks`).
    pub fn start_running(&mut self, t: &Tick) {
        self.to_run.remove(&(t.pos, t.kind));
    }

    pub fn end_tick(&mut self) {
        self.to_run.clear();
    }
}
