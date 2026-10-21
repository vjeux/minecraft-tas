//! Item frames as comparator inputs (clk-item, study 14's combination locks): `ItemFrame` hanging
//! in an air cell on the face of the block behind it, read by a comparator through that block
//! (`ComparatorBlock.getInputSignal` / `getItemFrame`), turned one step by a right-click
//! (`ItemFrame.interact` -> `setRotation(r + 1)` -> `Level.updateNeighbourForOutputSignal`).
//!
//! Static frames only: a frame never pops here (its support block is assumed to stay; the game
//! checks it every 100 ticks, `BlockAttachedEntity.tick`), never moves and takes no damage.

use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// One frame: the cell it hangs in, the direction it faces (away from its support block), its
/// item's rotation (0..7) and whether it holds an item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub pos: BlockPos,
    pub facing: Dir,
    pub rotation: u8,
    pub item: bool,
}

impl Frame {
    /// `ItemFrame.getAnalogOutput`: rotation + 1 with an item, 0 without.
    pub fn analog(&self) -> i32 {
        if self.item { (self.rotation % 8) as i32 + 1 } else { 0 }
    }
}

/// `getItemFrame`: the frame in cell `p` facing `d`, when there is exactly one.
pub fn facing_in(w: &World, p: BlockPos, d: Dir) -> Option<Frame> {
    let mut it = w.frames.iter().filter(|f| f.pos == p && f.facing == d);
    let first = it.next().copied();
    if it.next().is_some() { None } else { first }
}

/// A design's `minecraft:item_frame` entity: its cell (the position's floor), `Facing` (0 down,
/// 1 up, 2 north, 3 south, 4 west, 5 east), `ItemRotation` and `Item`. Placing a frame with an
/// item updates the comparators reading it (`ItemFrame.setItem`), as the game judge's summon does.
pub fn spawn(w: &mut World, pos: [f64; 3], nbt: Option<&str>) -> Result<(), String> {
    let n = nbt.unwrap_or("");
    let byte = |key: &str| -> Option<u8> { n.split(&format!("{key}:")).nth(1).map(|r| r.chars().take_while(|c| c.is_ascii_digit()).collect::<String>()).and_then(|s| s.parse().ok()) };
    let facing = match byte("Facing").unwrap_or(0) {
        0 => Dir::Down,
        1 => Dir::Up,
        2 => Dir::North,
        3 => Dir::South,
        4 => Dir::West,
        _ => Dir::East,
    };
    let p = BlockPos::new(pos[0].floor() as i32, pos[1].floor() as i32, pos[2].floor() as i32);
    let f = Frame { pos: p, facing, rotation: byte("ItemRotation").unwrap_or(0) % 8, item: n.contains("Item:") };
    w.frames.push(f);
    // the bridge's summon updates the comparators for every frame (an empty one reads 0)
    w.update_neighbour_for_output_signal(p, crate::block::Kind::Air);
    Ok(())
}

/// Static frames only (clk-misc's review): at the end of every tick each frame's cell must be air
/// and its support block solid. A block moving into the cell kills the frame in the game
/// (`HangingEntity.move`), a support that stops being solid pops it at the next survival check:
/// neither is modeled, so the run is marked unmodeled instead of passing falsely.
pub fn check_static(w: &mut World) {
    let bad: Vec<String> = w
        .frames
        .iter()
        .filter_map(|f| {
            let here = w.get(f.pos);
            let support = w.get(f.pos.rel(f.facing.opposite()));
            if !here.is_air() {
                Some(format!("item frame at {} would break: {here} entered its cell (not modeled)", f.pos))
            } else if !crate::behavior::is_solid(&support) {
                Some(format!("item frame at {} would pop: its support is {support} (not modeled)", f.pos))
            } else {
                None
            }
        })
        .collect();
    w.errors.extend(bad);
}

/// A player's empty-hand right-click on the frame in cell `p` (exactly one there): a frame holding
/// an item turns it one step and updates the comparators reading it. `false` when no single frame
/// hangs there.
pub fn use_frame(w: &mut World, p: BlockPos) -> bool {
    let idx: Vec<usize> = w.frames.iter().enumerate().filter(|(_, f)| f.pos == p).map(|(i, _)| i).collect();
    let [i] = idx.as_slice() else { return false };
    if w.frames[*i].item {
        w.frames[*i].rotation = (w.frames[*i].rotation + 1) % 8;
        w.update_neighbour_for_output_signal(p, crate::block::Kind::Air);
        // `interact` also emits GameEvent.BLOCK_CHANGE at the frame entity's position (not the
        // cell's center): recorded; a sculk sensor that could hear it is not modeled
        w.game_events.push(("block_change", p));
        if !w.sculk.is_empty() {
            w.errors.push(format!("item frame at {p} turned with sculk sensors in the world: its BLOCK_CHANGE at the entity's position is not modeled"));
        }
    }
    true
}
