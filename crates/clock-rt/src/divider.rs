//! Divider chains (`mctas clock rt divider`): a base torch-repeater U-loop followed by
//! master-slave T flip-flop stages. Each stage halves the frequency of the stage before it.
//!
//! A stage reads the previous stage's torch T (= not S) and block A (= S, powered by the
//! previous slave), with T north of A in the stage's frame and the stage built to the west:
//!
//! ```text
//!   z=-2:  w  w  .  .       w dust (the inverter path back to the master)
//!   z=-1:  w  M  L1 T       M master latch (repeater facing north), L1 repeater reading T
//!   z= 0:  w  S  L2 A       S slave latch (reads M), L2 repeater reading A; L1/L2 point into
//!   z= 1:  T' A'            the latches' east sides (locked while on); S powers A', torch T'
//! ```
//!
//! M and S are never unlocked together (L1 follows not S, L2 follows S), so the loop
//! M -> S -> A' -> T' -> dust -> M (one inversion) moves one step per half period of the
//! previous stage: T' toggles once per previous period. The output pair (T' west of A') is
//! turned a quarter turn from the input pair; each stage may also be built mirrored (to the
//! east): the chain is laid out by a depth-first search over the 2^k choices that collide with
//! nothing and touch nothing but the pair they read.

use crate::build::B;

pub type Pos = [i32; 3];

/// Frame: origin (the A cell) and the world directions of frame-east (+x) and frame-south (+z)
/// (a mirrored frame has ex x ez of the other handedness).
#[derive(Clone, Copy, Debug)]
struct Frame {
    a: Pos,
    ex: Pos,
    ez: Pos,
}

impl Frame {
    fn at(&self, fx: i32, fz: i32, y: i32) -> Pos {
        [self.a[0] + fx * self.ex[0] + fz * self.ez[0], y, self.a[2] + fx * self.ex[2] + fz * self.ez[2]]
    }
    fn world(&self, frame_dir: &str) -> &'static str {
        let v = match frame_dir {
            "east" => self.ex,
            "west" => [-self.ex[0], 0, -self.ex[2]],
            "south" => self.ez,
            "north" => [-self.ez[0], 0, -self.ez[2]],
            _ => panic!("dir"),
        };
        match (v[0], v[2]) {
            (1, 0) => "east",
            (-1, 0) => "west",
            (0, 1) => "south",
            (0, -1) => "north",
            _ => panic!("dir"),
        }
    }
    fn mirrored(&self) -> Frame {
        Frame { a: self.a, ex: [-self.ex[0], 0, -self.ex[2]], ez: self.ez }
    }
}

const N6: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]];

/// Place one stage in frame `f`; None on a collision or a touch.
fn stage(b: &B, f: Frame, y: i32) -> Option<(B, Frame, Pos)> {
    let p = |fx, fz| f.at(fx, fz, y);
    let cells = [p(-1, -1), p(-1, 0), p(-2, -1), p(-2, 0), p(-2, 1), p(-3, 1), p(-3, 0), p(-3, -1), p(-3, -2), p(-2, -2)];
    let floored = [p(-1, -1), p(-1, 0), p(-2, -1), p(-2, 0), p(-2, 1), p(-3, -1), p(-3, -2), p(-2, -2)];
    let pair = [f.at(0, -1, y), f.at(0, 0, y)];
    let below = |c: &Pos| [c[0], c[1] - 1, c[2]];
    let mut new_cells: Vec<Pos> = cells.to_vec();
    new_cells.extend(floored.iter().map(below));
    for c in &new_cells {
        if b.blocks.contains_key(c) {
            return None;
        }
    }
    // no new cell may touch an old block, except the lock repeaters touching the pair (and the
    // floors of the pair's neighbors)
    for c in &new_cells {
        for n in N6 {
            let q = [c[0] + n[0], c[1] + n[1], c[2] + n[2]];
            if new_cells.contains(&q) {
                continue;
            }
            if b.blocks.contains_key(&q) {
                let ok = pair.contains(&q) && (*c == p(-1, -1) || *c == p(-1, 0));
                let floor_touch = c[1] == y - 1 && q[1] == y - 1;
                if !ok && !floor_touch {
                    return None;
                }
            }
        }
    }
    let mut s = b.clone();
    s.rep(p(-1, -1), f.world("east"), 1, false);
    s.rep(p(-1, 0), f.world("east"), 1, false);
    s.rep(p(-2, -1), f.world("north"), 1, false);
    s.rep(p(-2, 0), f.world("north"), 1, false);
    // the slave's output: a dust that bends into a TARGET block west of it (dust connects to a
    // target, a conductor: an L-shaped dust points into it; a plain block would leave the dust a
    // straight line pointing past it)
    s.put(p(-2, 1), "redstone_wire");
    s.floor(p(-2, 1));
    s.put(p(-3, 1), "target");
    // the stage's torch hangs on that block's north face: the output pair (T' north of A') has
    // the input pair's orientation, so the next stage is the same layout moved by (-3, +1)
    s.wall_torch(p(-3, 0), f.world("north"), true);
    for c in [p(-3, -1), p(-3, -2), p(-2, -2)] {
        s.dust(c);
    }
    if s.conflict {
        return None;
    }
    let nf = Frame { a: p(-3, 1), ex: f.ex, ez: f.ez };
    Some((s, nf, p(-3, 0)))
}

fn dfs(b: &B, f: Frame, y: i32, left: usize, out: Pos) -> Option<(B, Pos)> {
    if left == 0 {
        return Some((b.clone(), out));
    }
    for fr in [f, f.mirrored()] {
        match stage(b, fr, y) {
            Some((s, nf, o)) => {
                if let Some(r) = dfs(&s, nf, y, left - 1, o) {
                    return Some(r);
                }
            }
            None => {
                if std::env::var("CLKRT_DEBUG").is_ok() {
                    eprintln!("stage with {left} left: no room in frame {fr:?}");
                }
            }
        }
    }
    None
}

/// The base U-loop: torch head, top row `d1`, block, dust tail, bottom row `d2` (0 = dust).
pub fn chain(d1: u8, d2: u8, stages: usize, name: &str) -> Option<B> {
    let y = 1;
    let mut b = B::new(name);
    b.stone([0, y, 0]);
    b.wall_torch([0, y, -1], "north", true);
    b.rep([1, y, -1], "west", d1, false);
    b.stone([2, y, -1]);
    b.dust([2, y, 0]);
    if d2 == 0 {
        b.dust([1, y, 0]);
    } else {
        b.rep([1, y, 0], "east", d2, false);
    }
    let f = Frame { a: [0, y, 0], ex: [1, 0, 0], ez: [0, 0, 1] };
    let (mut b, out) = dfs(&b, f, y, stages, [0, y, -1])?;
    b.port("output", out);
    Some(b)
}
