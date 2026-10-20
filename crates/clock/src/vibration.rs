//! The vibration test's shared parts (the circuit judge's `circuit::vibration`; the clock judge can
//! use the same events): a sculk sensor hears game events, so a design that holds one is also run
//! with AMBIENT game events around it, seeded by the build itself (the design as rotated and
//! placed) so redsim and the game see the same events for the same build. Every event is posted at
//! its cell's center (redsim measures block centers).

use crate::Pos;
use mctas::design::Design;

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
