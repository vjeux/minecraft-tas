//! Generators for the item families that extend by a repeated unit: one design per parameter set,
//! and the period each family's formula predicts (the `extend:` cards' generator commands:
//! `mctas clock item gen <family> ...`, checked by `mctas clock item gen-check`).

use crate::families::{Stack, drop_ring_delayed, hopper_loop, loop_cells, ring_lock};
use mctas::design::Design;

/// A hopper ring 2 x (n + 1) (n = 0: the hopper pair) with `m` items of `item` spread evenly
/// (m divides the ring length). Formula: P = 14 (n + 1) / m, blocks 2n + 4.
pub fn loop_ring(n: i32, m: usize, item: &str) -> Option<Design> {
    let l = loop_cells(2, n + 1).len();
    if m == 0 || l % m != 0 {
        return None;
    }
    let mut items: Vec<Vec<Stack>> = vec![Vec::new(); l];
    for k in 0..m {
        items[k * (l / m)] = vec![(item.to_string(), 1)];
    }
    let mut d = hopper_loop(2, n + 1, &items)?;
    d.name = format!("loop2x{}-{m}{item}", n + 1);
    Some(d)
}

pub fn loop_ring_period(n: i32, m: usize) -> u32 {
    (14 * (n + 1)) as u32 / m as u32
}

/// The dropper ring w x depth (depth = 3 + delays.len()), counter-clockwise: one or two droppers in
/// the bottom row at x = 1 (and x = 2), the comparator at (1, 1) reading the top hopper (1, 0), the
/// repeaters below it; one stone waits in the fired dropper (1, depth - 1), the other starts in the
/// cell `second` (a ring position).
/// Formula: P = 20 + 14 (w - 3) + 7 (depth - 3) + 7 [one dropper] + 2 sum(delays),
/// blocks 2w + 4 depth - 8.
pub fn dropring_with(w: i32, one_dropper: bool, delays: &[u8], second: (i32, i32)) -> Option<Design> {
    let depth = 3 + delays.len() as i32;
    let mut cells = loop_cells(w, depth);
    cells.reverse();
    let mut items: Vec<Vec<Stack>> = vec![Vec::new(); cells.len()];
    let fired = cells.iter().position(|c| *c == (1, depth - 1))?;
    let other = cells.iter().position(|c| *c == second)?;
    if fired == other {
        return None;
    }
    items[fired] = vec![("stone".into(), 1)];
    items[other] = vec![("stone".into(), 1)];
    let mask = if one_dropper { 0b010 } else { 0b110 };
    let mut d = drop_ring_delayed(w, 1, mask, true, delays, &items)?;
    let tag: String = delays.iter().map(|x| x.to_string()).collect();
    d.name = format!("dropring{w}x{depth}{}{}", if one_dropper { "-1d" } else { "-2d" }, if tag.is_empty() { String::new() } else { format!("-r{tag}") });
    Some(d)
}

/// [`dropring_with`] with the second stone in the top-right corner (w - 1, 0), upstream of the read
/// hopper.
pub fn dropring(w: i32, one_dropper: bool, delays: &[u8]) -> Option<Design> {
    dropring_with(w, one_dropper, delays, (w - 1, 0))
}

pub fn dropring_period(w: i32, one_dropper: bool, delays: &[u8]) -> u32 {
    let depth = 3 + delays.len() as i32;
    let s: i32 = delays.iter().map(|d| *d as i32).sum();
    (20 + 14 * (w - 3) + 7 * (depth - 3) + if one_dropper { 7 } else { 0 } + 2 * s) as u32
}

/// The ring lock 3 x d with n stones in the corner hopper. Formula for 5 <= n <= ringlock_n_max(d)
/// (L = 2d + 2): P = 7L + 8n - 34, blocks L + 2 (fewer items: P = 7L; more: saturates).
pub fn ringlock(d: i32, n: u32) -> Option<Design> {
    let l = (2 * d + 2) as usize;
    let mut items: Vec<Vec<Stack>> = vec![Vec::new(); l];
    items[0] = vec![("stone".into(), n)];
    let mut des = ring_lock(d, 1, false, &items)?;
    des.name = format!("ringlock3x{d}-n{n}");
    Some(des)
}

/// The largest stone count on the ring lock's linear range: floor((7L - 2) / 8) - 3.
pub fn ringlock_n_max(d: i32) -> u32 {
    let l = 2 * d + 2;
    ((7 * l - 2) / 8 - 3) as u32
}

pub fn ringlock_period(d: i32, n: u32) -> u32 {
    let l = 2 * d + 2;
    (7 * l + 8 * n as i32 - 34) as u32
}
