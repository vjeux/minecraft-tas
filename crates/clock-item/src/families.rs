//! Parametric item-clock families built from first principles (each a design generator), and
//! sweeps over their parameters: the period each parameter gives, with the block count.

use crate::sim::{self, Spec};
use mctas::design::Design;
use redsim::BlockPos;

/// The cells of a rectangular loop (w x d in x, z) in order, each hopper facing the next one
/// (clockwise seen from above, starting at the north-west corner). `w = 2, d = 1` is the facing
/// pair.
pub fn loop_cells(w: i32, d: i32) -> Vec<(i32, i32)> {
    if d == 1 {
        return if w == 2 { vec![(0, 0), (1, 0)] } else { Vec::new() };
    }
    if w == 1 {
        return if d == 2 { vec![(0, 0), (0, 1)] } else { Vec::new() };
    }
    let mut v = Vec::new();
    for x in 0..w {
        v.push((x, 0));
    }
    for z in 1..d {
        v.push((w - 1, z));
    }
    for x in (0..w - 1).rev() {
        v.push((x, d - 1));
    }
    for z in (1..d - 1).rev() {
        v.push((0, z));
    }
    v
}

fn facing(from: (i32, i32), to: (i32, i32)) -> &'static str {
    match (to.0 - from.0, to.1 - from.1) {
        (1, 0) => "east",
        (-1, 0) => "west",
        (0, 1) => "south",
        (0, -1) => "north",
        _ => panic!("loop cells not adjacent"),
    }
}

/// One stack: (item id without namespace, count).
pub type Stack = (String, u32);

/// SNBT for a hopper holding these stacks (one per slot).
pub fn items_snbt(stacks: &[Stack]) -> String {
    let list: Vec<String> = stacks.iter().enumerate().map(|(i, (id, n))| format!("{{Slot:{i}b,id:\"minecraft:{id}\",count:{n}}}")).collect();
    format!("{{Items:[{}]}}", list.join(","))
}

/// A hopper loop at y = 1 with a comparator reading hopper 0 from the north (standing on stone):
/// `items[i]` = the stacks in hopper i of the loop order.
pub fn hopper_loop(w: i32, d: i32, items: &[Vec<Stack>]) -> Option<Design> {
    let cells = loop_cells(w, d);
    if cells.is_empty() {
        return None;
    }
    let mut des = Design { name: format!("loop-{w}x{d}"), ..Default::default() };
    let n = cells.len();
    for (i, c) in cells.iter().enumerate() {
        let next = cells[(i + 1) % n];
        des.blocks.insert([c.0, 1, c.1], format!("minecraft:hopper[enabled=true,facing={}]", facing(*c, next)));
        if let Some(st) = items.get(i)
            && !st.is_empty()
        {
            des.block_entities.insert([c.0, 1, c.1], items_snbt(st));
        }
    }
    des.blocks.insert([0, 1, -1], "minecraft:comparator[facing=south,mode=compare,powered=false]".into());
    des.blocks.insert([0, 0, -1], "minecraft:stone".into());
    des.ports.insert("output".into(), [0, 1, -1]);
    Some(des)
}

/// A dropper ring: a hopper ring `w` wide and 3 deep (x 0..w-1, z 0..2) at y = 1 with a comparator
/// in the inner row at x = `xc` (on stone) that reads the top-row container (xc, 0) and points into
/// the bottom-row one (xc, 2). Bottom-row cells xc-1, xc, xc+1 are droppers where `drop_mask` bits
/// 0, 1, 2 are set (the side ones fire through the strongly powered middle one), the rest hoppers;
/// all face the next cell. `reversed` runs the ring counter-clockwise. `items[i]` = ring cell i.
pub fn drop_ring(w: i32, xc: i32, drop_mask: u8, reversed: bool, items: &[Vec<Stack>]) -> Option<Design> {
    if w < 3 || xc < 1 || xc > w - 2 {
        return None;
    }
    let mut cells = loop_cells(w, 3);
    if reversed {
        cells.reverse();
    }
    let mut des = Design { name: format!("dropring-{w}x3"), ..Default::default() };
    let n = cells.len();
    for (i, c) in cells.iter().enumerate() {
        let next = cells[(i + 1) % n];
        let f = facing(*c, next);
        let dropper = c.1 == 2 && (c.0 - xc).abs() <= 1 && drop_mask & (1 << (c.0 - xc + 1)) != 0;
        let state = if dropper {
            format!("minecraft:dropper[facing={f},triggered=false]")
        } else {
            format!("minecraft:hopper[enabled=true,facing={f}]")
        };
        des.blocks.insert([c.0, 1, c.1], state);
        if let Some(st) = items.get(i)
            && !st.is_empty()
        {
            des.block_entities.insert([c.0, 1, c.1], items_snbt(st));
        }
    }
    des.blocks.insert([xc, 1, 1], "minecraft:comparator[facing=north,mode=compare,powered=false]".into());
    des.blocks.insert([xc, 0, 1], "minecraft:stone".into());
    des.ports.insert("output".into(), [xc, 1, 1]);
    Some(des)
}

/// A ring lock: a rectangular hopper ring 3 wide (x 0..2) and `d` deep (z), clockwise, with a
/// comparator in the inner column at z = `zc` (standing on stone below it) that reads the west
/// ring hopper (0, zc) and points into the east one (2, zc), locking it while the west one holds
/// items. `reversed` runs the ring counter-clockwise. `items[i]` = stacks of ring hopper i.
pub fn ring_lock(d: i32, zc: i32, reversed: bool, items: &[Vec<Stack>]) -> Option<Design> {
    if d < 3 || zc < 1 || zc > d - 2 {
        return None;
    }
    let mut cells = loop_cells(3, d);
    if reversed {
        cells.reverse();
    }
    let mut des = Design { name: format!("ringlock-3x{d}"), ..Default::default() };
    let n = cells.len();
    for (i, c) in cells.iter().enumerate() {
        let next = cells[(i + 1) % n];
        des.blocks.insert([c.0, 1, c.1], format!("minecraft:hopper[enabled=true,facing={}]", facing(*c, next)));
        if let Some(st) = items.get(i)
            && !st.is_empty()
        {
            des.block_entities.insert([c.0, 1, c.1], items_snbt(st));
        }
    }
    des.blocks.insert([1, 1, zc], "minecraft:comparator[facing=west,mode=compare,powered=false]".into());
    des.blocks.insert([1, 0, zc], "minecraft:stone".into());
    des.ports.insert("output".into(), [1, 1, zc]);
    Some(des)
}

/// Judge a design in redsim: (visible period, whole-state period, on ticks, rising edges) or why
/// not.
pub fn measure(d: &Design, max_ticks: u32) -> Result<(u32, u32, u32, u32), String> {
    let spec = Spec::from_design(d)?;
    match sim::judge(&spec, max_ticks, d.ports.get("output").map(|p| BlockPos::from_arr(*p)))? {
        Ok(ch) if ch.signal && ch.period >= 2 => Ok((ch.period, ch.whole_period, ch.on_ticks, ch.rising)),
        Ok(ch) => Err(format!("static (P={})", ch.period)),
        Err(v) => Err(format!("{v:?}")),
    }
}
