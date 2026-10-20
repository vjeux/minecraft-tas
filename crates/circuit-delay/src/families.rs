//! Delay-line families (generators): every chain runs east from the input cell (0, 1, 0) at
//! y = 1 on floors at y = 0; the output is the last element's own output or the marker cell after
//! it.

use crate::build::B;

/// A line of repeaters with the given delays (redstone ticks 1-4): N = 2 x sum, 2 blocks each.
pub fn repeaters(delays: &[u8], name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    for (i, d) in delays.iter().enumerate() {
        b.rep([1 + i as i32, 1, 0], "west", *d, false);
    }
    b.port("output", [delays.len() as i32, 1, 0]);
    b
}

/// A line of k compare-mode comparators: N = 2k, 2 blocks each.
pub fn comparators(k: usize, name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    for i in 0..k {
        b.cmp([1 + i as i32, 1, 0], "west", false, false);
    }
    b.port("output", [k as i32, 1, 0]);
    b
}

/// k observers in a line (the first watches the input cell) carry each input edge as a 2 gt
/// pulse; a waxed copper bulb toggles on every pulse (= the input level again) and a comparator
/// reads it: N = 2k + 2, k + 3 blocks.
pub fn observers(k: usize, name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    for i in 0..k {
        b.put([1 + i as i32, 1, 0], "observer[facing=west,powered=false]");
    }
    let k = k as i32;
    b.put([k + 1, 1, 0], "waxed_copper_bulb[lit=false,powered=false]");
    b.cmp([k + 2, 1, 0], "west", false, false);
    b.port("output", [k + 2, 1, 0]);
    b
}

/// The odd delay line: a sticky piston pushes a redstone block one cell (it arrives 3 gt after the
/// input rises and leaves 1 gt after it falls); the output cell touches the block's new cell, and a
/// repeater behind that cell feeds the output again 2 gt later through a stone and two dusts. The
/// OR of the 3/1 and 5/3 copies is a 3/3 delay. The piston must see the input change between ticks
/// (a repeater in front would trigger it inside the tick: 4/2 and an even N), so longer odd delays
/// put the OR on a dust and run repeaters (delays `tail`) after it: N = 3 + 2 x sum(tail),
/// 9 blocks, or 11 + 2 x len(tail) with a tail.
pub fn piston_or(tail: &[u8], name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, -3]);
    b.put([0, 1, -2], "sticky_piston[extended=false,facing=south]");
    b.put([0, 1, -1], "redstone_block");
    b.rep([1, 1, 0], "west", 1, false);
    b.stone([2, 1, 0]);
    b.dust([2, 1, 1]);
    b.dust([1, 1, 1]);
    if tail.is_empty() {
        b.port("output", [0, 1, 1]);
        return b;
    }
    b.dust([0, 1, 1]);
    for (i, d) in tail.iter().enumerate() {
        b.rep([-1 - i as i32, 1, 1], "east", *d, false);
    }
    b.port("output", [-(tail.len() as i32), 1, 1]);
    b
}

/// The fewest repeaters whose delays (1-4 redstone ticks) sum to `t`: 4s, then the rest.
pub fn split_ticks(t: u32) -> Vec<u8> {
    let mut v = vec![4u8; (t / 4) as usize];
    if t % 4 != 0 {
        v.insert(0, (t % 4) as u8);
    }
    v
}

/// A repeater reading the input powers block B0; a tower of k torches climbs from it (each torch
/// strongly powers the block above it, the next torch stands on that block): N = 2 + 2k,
/// non-inverting for even k.
pub fn torches(k: usize, name: &str) -> B {
    let mut b = B::new(name);
    b.port("input", [0, 1, 0]);
    b.rep([1, 1, 0], "west", 1, false);
    b.stone([2, 1, 0]);
    let mut y = 2;
    for i in 0..k {
        // the torch at y, the block above it at y + 1 (not after the last torch)
        b.torch([2, y, 0], i % 2 == 0);
        if i + 1 < k {
            b.stone([2, y + 1, 0]);
        }
        y += 2;
    }
    b.port("output", [2, y - 2, 0]);
    b
}
