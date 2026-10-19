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
