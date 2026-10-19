//! Link types derived from the model: how each component turns an input pulse train into an
//! output pulse train (delays, pulses it swallows, gaps it merges, memory), measured by running
//! micro-gadgets in redsim (exact against the game). A planner that reasons about pulse COUNTS
//! (track box56's `door topology`: odd counts carry the lever's toggle) can read here which
//! components keep, double, merge or halve a count: e.g. an observer watching a level emits 2
//! pulses per input pulse (both edges), a lamp merges pulses closer than its 4 gt fall, a copper
//! bulb + comparator turns 2 pulses into 1 window (an odd link from an even source).
//!
//! Input: a floor lever at (0,1,0) toggled between ticks; the component under test reads it; the
//! output is the probe block's own state, sampled after every tick.

use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;

/// A test gadget: blocks (state strings) around the lever at (0,1,0), and the probe cell.
pub struct Gadget {
    pub name: &'static str,
    pub blocks: &'static [((i32, i32, i32), &'static str)],
    pub probe: (i32, i32, i32),
}

pub const GADGETS: &[Gadget] = &[
    Gadget { name: "repeater d1", blocks: &[((1, 0, 0), "stone"), ((1, 1, 0), "repeater[delay=1,facing=west]")], probe: (1, 1, 0) },
    Gadget { name: "repeater d2", blocks: &[((1, 0, 0), "stone"), ((1, 1, 0), "repeater[delay=2,facing=west]")], probe: (1, 1, 0) },
    Gadget { name: "repeater d4", blocks: &[((1, 0, 0), "stone"), ((1, 1, 0), "repeater[delay=4,facing=west]")], probe: (1, 1, 0) },
    Gadget { name: "comparator", blocks: &[((1, 0, 0), "stone"), ((1, 1, 0), "comparator[facing=west]")], probe: (1, 1, 0) },
    // a wall torch on the lever's floor block (the lever powers it strongly): inverts
    Gadget { name: "torch (inverts)", blocks: &[((1, 0, 0), "redstone_wall_torch[facing=east]")], probe: (1, 0, 0) },
    Gadget { name: "lamp", blocks: &[((1, 1, 0), "redstone_lamp")], probe: (1, 1, 0) },
    Gadget { name: "note block", blocks: &[((1, 1, 0), "note_block")], probe: (1, 1, 0) },
    Gadget { name: "observer on the lever", blocks: &[((1, 1, 0), "observer[facing=west]")], probe: (1, 1, 0) },
    Gadget { name: "observer on a lamp", blocks: &[((1, 1, 0), "redstone_lamp"), ((2, 1, 0), "observer[facing=west]")], probe: (2, 1, 0) },
    Gadget { name: "observer on a note block", blocks: &[((1, 1, 0), "note_block"), ((2, 1, 0), "observer[facing=west]")], probe: (2, 1, 0) },
    Gadget { name: "copper bulb", blocks: &[((1, 1, 0), "waxed_copper_bulb")], probe: (1, 1, 0) },
    Gadget {
        name: "observer -> bulb (T flip-flop)",
        blocks: &[((1, 1, 0), "observer[facing=west]"), ((2, 1, 0), "waxed_copper_bulb")],
        probe: (2, 1, 0),
    },
];

/// Is the probe's output on (powered / lit)?
fn on(b: &Block) -> bool {
    match *b {
        Block::Repeater { powered, .. } | Block::Comparator { powered, .. } | Block::Observer { powered, .. } | Block::Note { powered, .. } => powered,
        Block::Lamp { lit } | Block::Torch { lit } | Block::WallTorch { lit, .. } => lit,
        Block::Bulb { lit, .. } => lit,
        _ => false,
    }
}

/// Output windows (first tick on, first tick off) for lever toggles at the given ticks (a toggle
/// at t happens between tick t and tick t+1), over `ticks` ticks.
pub fn response(g: &Gadget, toggles: &[i64], ticks: i64) -> Vec<(i64, i64)> {
    let mut w = World::new(BlockPos::new(-4, -2, -4), [10, 6, 8]);
    let lever = BlockPos::new(0, 1, 0);
    let mut blocks: Vec<(BlockPos, Block)> = vec![(BlockPos::new(0, 0, 0), Block::parse("stone").unwrap()), (lever, Block::parse("lever[face=floor,facing=north]").unwrap())];
    for ((x, y, z), s) in g.blocks {
        blocks.push((BlockPos::new(*x, *y, *z), Block::parse(s).unwrap()));
    }
    w.place_like_judge(&blocks);
    w.run_until_idle(100);
    let probe = BlockPos::new(g.probe.0, g.probe.1, g.probe.2);
    let base = w.time;
    let mut out = Vec::new();
    let mut prev = on(&w.get(probe));
    let mut start = if prev { Some(0) } else { None };
    for t in 0..ticks {
        if toggles.contains(&t) {
            w.use_block(lever);
        }
        w.tick();
        let now = on(&w.get(probe));
        let k = w.time - base;
        if now && !prev {
            start = Some(k);
        } else if !now && prev {
            out.push((start.unwrap_or(0), k));
            start = None;
        }
        prev = now;
    }
    if let Some(s) = start {
        out.push((s, i64::MAX));
    }
    out
}

/// Input trains used for the table: name, lever toggles.
pub const TRAINS: &[(&str, &[i64])] = &[
    ("pulse 1", &[0, 1]),
    ("pulse 2", &[0, 2]),
    ("pulse 4", &[0, 4]),
    ("pulse 8", &[0, 8]),
    ("2x pulse 2, gap 1", &[0, 2, 3, 5]),
    ("2x pulse 2, gap 2", &[0, 2, 4, 6]),
    ("2x pulse 2, gap 4", &[0, 2, 6, 8]),
    ("2x pulse 2, gap 8", &[0, 2, 10, 12]),
];

/// The full table: for each gadget and train, the output windows.
pub fn table() -> Vec<(&'static str, Vec<(&'static str, Vec<(i64, i64)>)>)> {
    GADGETS.iter().map(|g| (g.name, TRAINS.iter().map(|(n, t)| (*n, response(g, t, 40))).collect())).collect()
}
