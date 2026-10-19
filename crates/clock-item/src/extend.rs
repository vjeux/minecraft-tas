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

/// An extendable family card: the generator arguments (after `mctas clock item gen`, `{n}` and
/// `{out}` replaced), P(n) = p0 + dp n, blocks(n) = b0 + db n over n_lo..=n_hi, the extension rule
/// and the caption.
pub struct Card {
    pub id: &'static str,
    pub args: &'static str,
    pub p0: i64,
    pub dp: i64,
    pub b0: i64,
    pub db: i64,
    pub n_lo: i64,
    pub n_hi: i64,
    pub redsim: bool,
    pub rule: &'static str,
    pub caption: &'static str,
}

const DROP_CAPTION: &str = "Dropper ring (clk-item): a hopper ring with dropper(s) in its bottom row; the comparator in the inner column reads the top-row hopper above it and fires the dropper below it (through the repeaters, each on stone). One stone waits in the dropper until the other passes the read hopper; then they swap roles. The ring runs counter-clockwise; P = 20 + 14 (W - 3) + 7 (depth - 3) + 7 [one dropper] + 2 x (sum of repeater delays): fourteen cards of this ring (one per P mod 14) reach every P from 42 up.";

/// Every card of track clk-item (FAMILIES.md).
pub fn cards() -> Vec<Card> {
    let mut v = vec![
        Card { id: "clk-item-hopper-ring", args: "loop --n {n} --out {out}", p0: 14, dp: 14, b0: 4, db: 2, n_lo: 0, n_hi: 12, redsim: true, rule: "lengthen the 2-wide hopper ring by one column (2 hoppers)", caption: "Hopper ring with one item (clk-item): hoppers in a 2-wide ring, each facing the next, one item, a comparator (on stone) reading one hopper. A hop through an empty hopper is 7 gt (cooldown 8, minus 1 because the sender already ticked), so the item goes around in 7 L gt; n = 0 is the hopper pair (P 14, 4 blocks)." },
        Card { id: "clk-item-hopper-ring-2", args: "loop --m 2 --n {n} --out {out}", p0: 7, dp: 7, b0: 4, db: 2, n_lo: 1, n_hi: 12, redsim: true, rule: "lengthen the 2-wide hopper ring by one column (2 hoppers); the two items stay half a ring apart", caption: "Hopper ring with two items half a ring apart (clk-item): the state repeats every half turn, 7 L / 2 gt; odd multiples of 7 included (21, 35, 49 ...)." },
        Card { id: "clk-item-ringlock-rows", args: "ringlockd --n0 5 --n {n} --out {out}", p0: 76, dp: 14, b0: 12, db: 2, n_lo: 0, n_hi: 12, redsim: true, rule: "lengthen the 3-wide ring by one row (2 hoppers), still 5 stones", caption: "Ring lock with 5 stones (clk-item): a 3-wide hopper ring 3 x (4 + n), the comparator in the inner column (on stone) reads the west hopper of row 1 and points into the east one, locking it while the read hopper holds items; the 5 stones start in the corner hopper and queue in front of the locked hopper: P = 7 L + 8 x 5 - 34." },
        Card { id: "clk-item-ringlock-stones-3x6", args: "ringlock --d 6 --n0 5 --n {n} --out {out}", p0: 104, dp: 8, b0: 16, db: 0, n_lo: 0, n_hi: 4, redsim: true, rule: "one more stone in the corner hopper (same blocks)", caption: "Ring lock 3 x 6, item-count tuned (clk-item): the ring lock of 14 hoppers with 5 + n stones in the corner hopper: each stone adds one 8-gt hopper move to the queue in front of the locked hopper, P = 7 L + 8 N - 34, up to N = floor((7 L - 2) / 8) - 3 = 9." },
        Card { id: "clk-item-ringlock-stones-3x20", args: "ringlock --d 20 --n0 5 --n {n} --out {out}", p0: 300, dp: 8, b0: 44, db: 0, n_lo: 0, n_hi: 28, redsim: true, rule: "one more stone in the corner hopper (same blocks)", caption: "Ring lock 3 x 20, item-count tuned (clk-item): the ring lock of 42 hoppers with 5 + n stones in the corner hopper: each stone adds one 8-gt hopper move to the queue in front of the locked hopper, P = 7 L + 8 N - 34, up to N = 33 (P 524)." },
        Card { id: "clk-item-snowdisp", args: "snowdisp --n {n} --out {out}", p0: 16, dp: 4, b0: 5, db: 0, n_lo: 0, n_hi: 2, redsim: false, rule: "one more tick of delay on the repeater (same blocks)", caption: "Powder snow dispenser with a repeater (clk-item): a dispenser facing down holds a powder snow bucket; each fire toggles the cell under it (place / pick up); an observer watching that cell pulses into a stone whose dust feeds a repeater (delay n + 1) on the observer pointing into the dispenser: P = 2 x (2 + 2 (n + 1) + 4). Kicked once (a stone flashed 1 gt in the cell). Delay 4 stops: the repeater swallows the next pulse." },
        Card { id: "clk-item-lockpair", args: "lockpair --n {n} --out {out}", p0: 11, dp: 2, b0: 9, db: 0, n_lo: 0, n_hi: 2, redsim: true, rule: "one more tick of delay on the lock repeater (same blocks)", caption: "Hopper pair with a delayed lock (clk-item): 2 swords start in the west-facing hopper B; the comparator reading the east-facing hopper A powers a stone, the dust beside it feeds a repeater (delay n + 1) pointing into B, which locks B from 2 + 2 (n + 1) gt after A gets a sword until that long after it empties: P = 9 + 2 (n + 1). Delay 4 gives 25 (the lock misses a move)." },
    ];
    // The dropper rings, one card per residue of P mod 14: (id, args, P0, blocks0).
    let drops: [(&'static str, &'static str, i64, i64); 14] = [
        ("clk-item-dropring-2d", "dropring --w0 4 --n {n} --out {out}", 34, 12),
        ("clk-item-dropring-1d", "dropring --one --n {n} --out {out}", 27, 10),
        ("clk-item-dropring-2d-r1", "dropring --w0 4 --delays 1 --n {n} --out {out}", 43, 16),
        ("clk-item-dropring-2d-r2", "dropring --delays 2 --n {n} --out {out}", 31, 14),
        ("clk-item-dropring-2d-r3", "dropring --delays 3 --n {n} --out {out}", 33, 14),
        ("clk-item-dropring-2d-r4", "dropring --delays 4 --n {n} --out {out}", 35, 14),
        ("clk-item-dropring-1d-r1", "dropring --one --delays 1 --n {n} --out {out}", 36, 14),
        ("clk-item-dropring-1d-r2", "dropring --one --delays 2 --n {n} --out {out}", 38, 14),
        ("clk-item-dropring-1d-r3", "dropring --one --delays 3 --n {n} --out {out}", 40, 14),
        ("clk-item-dropring-1d-r4", "dropring --one --delays 4 --n {n} --out {out}", 42, 14),
        ("clk-item-dropring-2d-r14", "dropring --delays 14 --n {n} --out {out}", 44, 18),
        ("clk-item-dropring-2d-r24", "dropring --delays 24 --n {n} --out {out}", 46, 18),
        ("clk-item-dropring-1d-r14", "dropring --one --delays 14 --n {n} --out {out}", 51, 18),
        ("clk-item-dropring-1d-r24", "dropring --one --delays 24 --n {n} --out {out}", 53, 18),
    ];
    for (id, args, p0, b0) in drops {
        v.push(Card { id, args, p0, dp: 14, b0, db: 2, n_lo: 0, n_hi: 7, redsim: true, rule: "widen the dropper ring by one column (2 hoppers)", caption: DROP_CAPTION });
    }
    v
}

pub fn ringlock_period(d: i32, n: u32) -> u32 {
    let l = 2 * d + 2;
    (7 * l + 8 * n as i32 - 34) as u32
}

fn design_from_rows(name: &str, layers: &[(i32, &[&str])], palette: &[(char, &str)], origin: [i32; 3]) -> Design {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    for (y, rows) in layers {
        for (dz, row) in rows.iter().enumerate() {
            for (dx, ch) in row.chars().enumerate() {
                if ch == '.' {
                    continue;
                }
                let state = palette.iter().find(|(c, _)| *c == ch).map(|(_, s)| s.to_string()).expect("palette char");
                d.blocks.insert([origin[0] + dx as i32, *y, origin[2] + dz as i32], state);
            }
        }
    }
    d
}

/// The powder snow dispenser clock with a repeater of this delay (1..=3) on the observer:
/// P = 12 + 4 delay, 5 blocks; kicked by a stone flashed 1 gt in the toggled cell. JVM only.
pub fn snowdisp(delay: u8) -> Option<Design> {
    if !(1..=3).contains(&delay) {
        return None;
    }
    let rep = format!("minecraft:repeater[delay={delay},facing=east,locked=false,powered=false]");
    let palette: [(char, &str); 5] = [
        ('S', "minecraft:stone"),
        ('D', "minecraft:dispenser[facing=down,triggered=false]"),
        ('O', "minecraft:observer[facing=west,powered=false]"),
        ('R', &rep),
        ('r', "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"),
    ];
    let mut d = design_from_rows(&format!("snowdisp-r{delay}"), &[(0, &[".OS"]), (1, &["DRr"])], &palette, [0, 0, 0]);
    d.block_entities.insert([0, 1, 0], "{Items:[{Slot:0b,id:\"minecraft:powder_snow_bucket\",count:1}]}".into());
    d.ports.insert("output".into(), [0, 0, 0]);
    d.ports.insert("start".into(), [0, 0, 0]);
    d.meta.insert("port start".into(), "minecraft:stone 1".into());
    Some(d)
}

/// The hopper pair with a delayed lock: 2 swords in the west-facing hopper B, the comparator
/// reading A powers a stone, the dust beside it feeds a repeater of this delay (1..=3) into B:
/// P = 9 + 2 delay, 9 blocks.
pub fn lockpair(delay: u8) -> Option<Design> {
    if !(1..=3).contains(&delay) {
        return None;
    }
    let rep = format!("minecraft:repeater[delay={delay},facing=north,locked=false,powered=false]");
    let palette: [(char, &str); 6] = [
        ('C', "minecraft:comparator[facing=south,mode=compare,powered=false]"),
        ('H', "minecraft:hopper[enabled=true,facing=east]"),
        ('R', "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"),
        ('S', "minecraft:stone"),
        ('h', "minecraft:hopper[enabled=true,facing=west]"),
        ('r', &rep),
    ];
    let mut d = design_from_rows(&format!("lockpair-r{delay}"), &[(0, &[".S", "SS", "."]), (1, &["SR", "Cr", "Hh"])], &palette, [0, 0, -2]);
    d.block_entities.insert([1, 1, 0], "{Items:[{Slot:0b,id:\"minecraft:diamond_sword\",count:1},{Slot:1b,id:\"minecraft:diamond_sword\",count:1}]}".into());
    d.ports.insert("output".into(), [0, 1, -1]);
    Some(d)
}
