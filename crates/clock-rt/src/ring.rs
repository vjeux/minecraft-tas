//! Rectangle rings (`mctas clock rt sweep-ring`): a closed path around a w x h rectangle (the
//! cells of its perimeter, clockwise from the north-west corner), one component per cell:
//!
//! * a repeater (delay 1-4, optionally powered at placement) on a straight cell, reading the
//!   previous cell and pointing into the next;
//! * a comparator (compare mode) on a straight cell, the same way;
//! * a plain block (powered by the repeater / comparator / dust before it);
//! * dust (it joins its neighbors on the path);
//! * a wall torch hanging on the previous cell's block (an inversion; the next cell reads it).
//!
//! Corners hold blocks, dust or torches (a diode cannot turn). Turning at plain blocks costs one
//! block and no floor, so a 3x3 ring (4 corner blocks + 4 repeaters) is a 12-block pulse ring.

use crate::build::{B, Pos};
use crate::families::Cell;

/// The perimeter of a w x h rectangle, clockwise from (0, 0) (x east, z south).
pub fn perimeter(w: i32, h: i32) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for x in 0..w {
        v.push((x, 0));
    }
    for z in 1..h {
        v.push((w - 1, z));
    }
    for x in (0..w - 1).rev() {
        v.push((x, h - 1));
    }
    for z in (1..h - 1).rev() {
        v.push((0, z));
    }
    v
}

fn dir_name(d: (i32, i32)) -> &'static str {
    match d {
        (1, 0) => "east",
        (-1, 0) => "west",
        (0, 1) => "south",
        (0, -1) => "north",
        _ => panic!("dir"),
    }
}

/// Build the ring; None when a cell's component does not fit (a diode on a corner, a torch not
/// after a block).
pub fn build(w: i32, h: i32, cells: &[Cell], name: &str) -> Option<B> {
    let p = perimeter(w, h);
    if p.len() != cells.len() {
        return None;
    }
    let n = p.len();
    let y = 1;
    let mut b = B::new(name);
    for i in 0..n {
        let (x, z) = p[i];
        let prev = p[(i + n - 1) % n];
        let next = p[(i + 1) % n];
        let to_prev = (prev.0 - x, prev.1 - z);
        let to_next = (next.0 - x, next.1 - z);
        let straight = to_prev.0 == -to_next.0 && to_prev.1 == -to_next.1;
        let at: Pos = [x, y, z];
        match cells[i] {
            Cell::Rep { d, on } => {
                if !straight {
                    return None;
                }
                b.rep(at, dir_name(to_prev), d, on);
            }
            Cell::Cmp => {
                if !straight {
                    return None;
                }
                b.cmp(at, dir_name(to_prev), false, false);
            }
            Cell::Block => b.stone(at),
            Cell::Dust => b.dust(at),
            Cell::Torch { lit } => {
                if !matches!(cells[(i + n - 1) % n], Cell::Block) {
                    return None;
                }
                // a wall torch faces away from the block it hangs on
                b.wall_torch(at, dir_name((-to_prev.0, -to_prev.1)), lit);
            }
        }
    }
    // no declared output: a clock's output is an exterior air cell the judge picks (clockinfra,
    // 2026-10-20: `clock sim --exterior`; a component cell inside the build no longer counts)
    let _ = (n, &cells);
    Some(b)
}

pub fn label(w: i32, h: i32, cells: &[Cell]) -> String {
    format!("ring{w}x{h}-{}", cells.iter().map(|c| c.label()).collect::<Vec<_>>().join(","))
}

pub fn caption(w: i32, h: i32, cells: &[Cell]) -> String {
    let torches = cells.iter().filter(|c| matches!(c, Cell::Torch { .. })).count();
    let rt: u32 = cells.iter().map(|c| c.rt()).sum();
    let kind = if torches % 2 == 1 { format!("{torches} torch(es): an inverter loop, period 2 x {} gt", 2 * rt) } else { format!("{torches} torches: a pulse ring of loop delay {} gt", 2 * rt) };
    format!(
        "Rectangle ring (clk-rt): the {w}x{h} perimeter clockwise from the north-west corner = [{}] (number = repeater delay, * = powered at placement, b = block, d = dust, T = wall torch on the block before it, c = comparator); {kind}",
        cells.iter().map(|c| c.label()).collect::<Vec<_>>().join(", ")
    )
}

/// Every assignment of the perimeter from `straight` (straight cells) and `corner` (corners),
/// with at most `max_powered` powered repeaters (and at most `max_delay` delay cells).
pub fn assignments(w: i32, h: i32, straight: &[Cell], corner: &[Cell], max_powered: usize) -> Vec<Vec<Cell>> {
    assignments_max(w, h, straight, corner, max_powered, usize::MAX)
}

pub fn assignments_max(w: i32, h: i32, straight: &[Cell], corner: &[Cell], max_powered: usize, max_delay: usize) -> Vec<Vec<Cell>> {
    let p = perimeter(w, h);
    let n = p.len();
    let is_corner: Vec<bool> = (0..n)
        .map(|i| {
            let (x, z) = p[i];
            let prev = p[(i + n - 1) % n];
            let next = p[(i + 1) % n];
            !((prev.0 - x) == -(next.0 - x) && (prev.1 - z) == -(next.1 - z))
        })
        .collect();
    let mut out: Vec<Vec<Cell>> = vec![Vec::new()];
    for i in 0..n {
        let opts = if is_corner[i] { corner } else { straight };
        let mut next = Vec::new();
        for a in &out {
            for c in opts {
                let mut v = a.clone();
                v.push(*c);
                let powered = v.iter().filter(|c| matches!(c, Cell::Rep { on: true, .. })).count();
                if powered > max_powered {
                    continue;
                }
                if v.iter().filter(|c| c.rt() > 0).count() > max_delay {
                    continue;
                }
                // a block after a block or a torch never gets power
                if v.len() >= 2 && matches!(c, Cell::Block) && matches!(v[v.len() - 2], Cell::Block | Cell::Torch { .. }) {
                    continue;
                }
                // a torch must hang on a block
                if matches!(c, Cell::Torch { .. }) && (v.len() < 2 || !matches!(v[v.len() - 2], Cell::Block)) {
                    continue;
                }
                next.push(v);
            }
        }
        out = next;
    }
    out
}
