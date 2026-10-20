//! Exact state keys (the clock judge, `crates/clock`, docs/clocks.md): everything that decides a
//! world's future at a tick boundary, with game times made relative to now and insertion counters
//! reduced to the order they impose. Two worlds with equal keys run identically from then on, so
//! the first key seen twice gives a clock's exact warmup and period.
//!
//! What a key holds: every cell; the pending scheduled ticks as (delay, priority, rank among the
//! ticks due the same tick, position, block) (the sub-tick counter only orders ticks due together);
//! pending block events in queue order; comparator outputs; containers (slots, hopper cooldown,
//! `tickedGameTime` relative to now) and the hopper ticker order; torch burnout memory younger than
//! the game's 60 gt window (older entries are pruned before any count); moving-piston block
//! entities in ticker order (moved state, direction, progress, last tick relative to now) and the
//! ideal-input bookkeeping; falling blocks; sculk sensor block entities (vibration in flight,
//! selector candidate with its relative time) and their ticker order; how many drops and model
//! errors happened (a run with any is not a clean clock).

use crate::world::World;
use std::hash::{Hash, Hasher};

/// A [`Hasher`] that keeps every byte written: hashing a value into it gives an exact, injective
/// encoding of the value (for the derived `Hash` of fixed-shape types), not a digest.
#[derive(Default)]
pub struct Collect(pub Vec<u8>);

impl Hasher for Collect {
    fn finish(&self) -> u64 {
        // FNV-1a of the bytes (only for convenience; equality uses the bytes).
        let mut h: u64 = 0xcbf29ce484222325;
        for b in &self.0 {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }
}

/// Torch burnout memory older than this is pruned before it is counted (`RedstoneTorchBlock.tick`).
pub const TORCH_WINDOW: i64 = 60;

impl World {
    /// The exact state key at a tick boundary (call it between ticks).
    pub fn state_key(&self) -> Vec<u8> {
        let mut h = Collect(Vec::with_capacity(self.cells.len() * 8 + 256));
        self.state_key_into(&mut h);
        h.0
    }

    /// [`World::state_key`] into any hasher (a digest for a fast first screen).
    pub fn state_key_into<H: Hasher>(&self, h: &mut H) {
        let now = self.time;
        self.cells.hash(h);
        // Scheduled ticks: all() is sorted by (trigger, priority, sub); the rank inside a trigger
        // group is the order the sub-tick counter imposes on ticks due together.
        let ticks = self.ticks.all();
        (ticks.len() as u32).hash(h);
        let mut rank = 0u32;
        let mut last_trigger = i64::MIN;
        for t in &ticks {
            if t.trigger != last_trigger {
                rank = 0;
                last_trigger = t.trigger;
            }
            (t.trigger - now, t.priority, rank, t.pos, t.kind).hash(h);
            rank += 1;
        }
        (self.events.len() as u32).hash(h);
        for e in self.events.iter() {
            e.hash(h);
        }
        // fluid ticks (only when there are any: designs without water keep their keys)
        let fticks = self.fluid_ticks.all();
        if !fticks.is_empty() {
            0xf1u8.hash(h);
            (fticks.len() as u32).hash(h);
            let mut rank = 0u32;
            let mut last_trigger = i64::MIN;
            for t in &fticks {
                if t.trigger != last_trigger {
                    rank = 0;
                    last_trigger = t.trigger;
                }
                (t.trigger - now, rank, t.pos, t.kind).hash(h);
                rank += 1;
            }
        }
        let mut comps: Vec<_> = self.comparator_out.iter().map(|(p, v)| (*p, *v)).collect();
        comps.sort();
        comps.hash(h);
        self.containers.key_into(now, h);
        let toggles: Vec<_> = self.torch_toggles.iter().filter(|(_, when)| now - when <= TORCH_WINDOW).map(|(p, when)| (*p, now - when)).collect();
        toggles.hash(h);
        self.piston.key_into(now, h);
        self.gravity.key_into(h);
        // entities (only when there are any: designs without entities keep their keys); every
        // field by its exact bits (Debug prints round-trip floats), payloads included
        if !self.entities.list.is_empty() {
            0xe7u8.hash(h);
            for e in self.entities.list.iter().filter(|e| !e.removed) {
                let mut k = e.clone();
                k.id = 0;
                k.tick_count = 0;
                format!("{k:?}").hash(h);
            }
        }
        self.sculk.key_into(now, h);
        (self.drops.len() as u32).hash(h);
        (self.errors.len() as u32).hash(h);
    }
}
