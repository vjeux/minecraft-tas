//! Comparator COUNTER clocks (`mctas clock rt counter`): a comparator fading ring (the inner
//! clock) whose refill pulse decrements a LOSSLESS comparator ring (the counter) by exactly one
//! level; the counter's own refill torch is the output: period = about 14 inner periods.
//!
//! ```text
//!   z=-1:  E  c> .. c>  F  To      counter: corner head (A + dust E: lossless), compare comparators,
//!   z= 0:  A  <s <c ..  D1 Jo      the bottom row's last comparator s in SUBTRACT mode
//!   z= 1:     x                    x: dust at strength 1 during the inner pulse (s's side)
//!   z= 2:  R w w Ce                Ce: subtract comparator, rear = the inner torch, side = 14
//!   z= 3:      Ti Ji               (redstone block R + two dust): output 1 while the torch is on
//!   z>=4:      inner fading ring   (rotated: its torch Ti hangs on its block's north face)
//! ```
//! A decrement is clean when the pulse lasts between L - 2 and L + 1 gt (L = the counter's lap:
//! 4 x comparators per row): shorter, the old level comes back around; longer, it decrements
//! twice. A fading ring's refill pulse lasts about its own lap, so inner and counter rings get the
//! same number of comparators per row.

use crate::build::{B, Pos};

pub fn build(k_in: usize, k_out: usize, name: &str) -> B {
    let y = 1;
    let mut b = B::new(name);
    let ko = k_out as i32;
    // counter ring
    b.stone([0, y, 0]);
    b.dust([0, y, -1]);
    for i in 1..=ko {
        b.cmp([i, y, -1], "west", false, false);
        b.cmp([i, y, 0], "east", i == 1, false);
    }
    b.stone([ko + 1, y, -1]);
    b.dust([ko + 1, y, 0]);
    b.wall_torch([ko + 2, y, -1], "east", true);
    b.dust([ko + 2, y, 0]);
    b.port("output", [ko + 2, y, -1]);
    // decrement input: dust at the subtract comparator's south side, fed by Ce
    b.dust([1, y, 1]);
    b.cmp([1, y, 2], "south", true, false);
    b.dust([0, y, 2]);
    b.dust([-1, y, 2]);
    b.put([-2, y, 2], "minecraft:redstone_block");
    // inner fading ring, rotated: frame (fx, fz) -> world (2 + fz, k_in + 5 - fx); frame west
    // (the top row's input) = world south, frame east = world north
    let ki = k_in as i32;
    let w = |fx: i32, fz: i32| -> Pos { [2 + fz, y, ki + 5 - fx] };
    b.dust(w(0, -1));
    b.dust(w(0, 0));
    for i in 1..=ki {
        b.cmp(w(i, -1), "south", false, false);
        b.cmp(w(i, 0), "north", false, false);
    }
    b.stone(w(ki + 1, -1));
    b.dust(w(ki + 1, 0));
    b.wall_torch(w(ki + 2, -1), "north", true);
    b.dust(w(ki + 2, 0));
    b
}
