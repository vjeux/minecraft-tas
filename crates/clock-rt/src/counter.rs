//! Comparator COUNTER clocks (`mctas clock rt counter`): a comparator fading ring (the inner
//! clock) whose refill pulse decrements a LOSSLESS comparator ring (the counter) by exactly one
//! level; the counter's own refill torch is the output: period = about 14 inner periods. Counters
//! nest: each level counts the refill pulses of the level inside it.
//!
//! ```text
//!   z=-1:  E  c> .. c>  F  To      counter: corner head (A + dust E: lossless), compare comparators,
//!   z= 0:  A  <s <c ..  D1 Jo      the bottom row's last comparator s in SUBTRACT mode
//!   z= 1:     x                    x: dust at strength 1 during the inner pulse (s's side)
//!   z= 2:  R w w Ce                Ce: subtract comparator, rear = the inner torch, side = 14
//!   z= 3:      Ti Ji               (redstone block R + two dust): output 1 while the torch is on
//!   z>=4:      inner level         (rotated: its output torch Ti hangs on its block's north face)
//! ```
//! A decrement is clean when the pulse lasts L - 1, L or L + 1 gt (L = the counter's lap: 4 gt
//! per comparator pair): shorter, the old level comes back around; longer, it decrements twice. A
//! fading ring's refill pulse lasts 4k gt (k comparators per row), a counter's 4k + 2.

use crate::build::{B, Pos};

/// A placement: local (x, z) -> world, local facings -> world facings (rotations by 90 degrees
/// counter-clockwise seen from above: east -> north).
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub o: [i32; 2],
    /// quarter turns counter-clockwise
    pub r: u8,
}

const DIRS: [&str; 4] = ["east", "north", "west", "south"];

impl Xf {
    pub fn id() -> Xf {
        Xf { o: [0, 0], r: 0 }
    }

    fn rot(r: u8, x: i32, z: i32) -> (i32, i32) {
        let (mut x, mut z) = (x, z);
        for _ in 0..r % 4 {
            (x, z) = (z, -x); // east (1,0) -> north (0,-1)
        }
        (x, z)
    }

    pub fn p(&self, x: i32, y: i32, z: i32) -> Pos {
        let (a, c) = Xf::rot(self.r, x, z);
        [self.o[0] + a, y, self.o[1] + c]
    }

    pub fn f(&self, facing: &str) -> &'static str {
        let i = DIRS.iter().position(|d| *d == facing).expect("facing");
        DIRS[(i + self.r as usize) % 4]
    }

    /// self after `inner` (inner coordinates first mapped by `inner`, then by self).
    pub fn then(&self, inner: Xf) -> Xf {
        let o = self.p(inner.o[0], 0, inner.o[1]);
        Xf { o: [o[0], o[2]], r: (self.r + inner.r) % 4 }
    }
}

/// The inner fading ring (head dust2, k comparators per row, fade tail), its output torch at
/// local (k + 2, -1) facing east.
fn fading(b: &mut B, t: &Xf, k: i32) {
    let y = 1;
    b.dust(t.p(0, y, -1));
    b.dust(t.p(0, y, 0));
    for i in 1..=k {
        b.cmp(t.p(i, y, -1), t.f("west"), false, false);
        b.cmp(t.p(i, y, 0), t.f("east"), false, false);
    }
    b.stone(t.p(k + 1, y, -1));
    b.dust(t.p(k + 1, y, 0));
    b.wall_torch(t.p(k + 2, y, -1), t.f("east"), true);
    b.dust(t.p(k + 2, y, 0));
}

/// One counter level (k comparators per row) with its decrement shaper; its output torch at local
/// (k + 2, -1) facing east.
fn counter_level(b: &mut B, t: &Xf, k: i32) {
    let y = 1;
    b.stone(t.p(0, y, 0));
    b.dust(t.p(0, y, -1));
    for i in 1..=k {
        b.cmp(t.p(i, y, -1), t.f("west"), false, false);
        b.cmp(t.p(i, y, 0), t.f("east"), i == 1, false);
    }
    b.stone(t.p(k + 1, y, -1));
    b.dust(t.p(k + 1, y, 0));
    b.wall_torch(t.p(k + 2, y, -1), t.f("east"), true);
    b.dust(t.p(k + 2, y, 0));
    b.dust(t.p(1, y, 1));
    b.cmp(t.p(1, y, 2), t.f("south"), true, false);
    b.dust(t.p(0, y, 2));
    b.dust(t.p(-1, y, 2));
    b.put(t.p(-2, y, 2), "minecraft:redstone_block");
}

/// The placement of a level whose output torch is at local (k + 2, -1) facing east, so that the
/// torch lands at (1, 3) facing north in the enclosing counter's frame.
fn inner_xf(k: i32) -> Xf {
    // rotate once (east -> north): (k + 2, -1) -> (-1, -(k + 2)); then translate to (1, 3)
    Xf { o: [2, k + 5], r: 1 }
}

/// `k_in` = the fading ring's comparators per row; `ks` = the counters' sizes, innermost first.
pub fn build_nested(k_in: usize, ks: &[usize], name: &str) -> B {
    let mut b = B::new(name);
    let mut t = Xf::id();
    // outermost first
    for (n, &k) in ks.iter().enumerate().rev() {
        counter_level(&mut b, &t, k as i32);
        if n == ks.len() - 1 {
            b.port("output", t.p(k as i32 + 2, 1, -1));
        }
        let k_inner = if n == 0 { k_in } else { ks[n - 1] } as i32;
        t = t.then(inner_xf(k_inner));
    }
    fading(&mut b, &t, k_in as i32);
    if ks.is_empty() {
        b.port("output", t.p(k_in as i32 + 2, 1, -1));
    }
    b
}

pub fn build(k_in: usize, k_out: usize, name: &str) -> B {
    build_nested(k_in, &[k_out], name)
}
