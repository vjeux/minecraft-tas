//! Shulker boxes (shulker study, clk-rt): a dispenser places one (`ShulkerBoxDispenseBehavior` +
//! `BlockItem.place`), a piston destroys one into an item entity carrying its contents
//! (`ShulkerBoxBlock.getDrops`, `Block.popResource` as the bridge seeds it: `rng::Drop`). The box
//! itself is a 27-slot container block entity (`container.rs`), read by comparators and filled
//! or emptied by hoppers like any container; a hopper or dropper never puts a box into a box
//! (`ShulkerBoxBlockEntity.canPlaceItemThroughFace`).

use crate::block::{Block, DYES};
use crate::pos::{BlockPos, Dir};
use crate::redstone::container::{self, CRef, Stack};
use crate::world::{World, flags};

/// The block a shulker box item places (`colour` from its name: `shulker_box` or
/// `<dye>_shulker_box`).
pub fn box_block_of(item: &str, facing: Dir) -> Option<Block> {
    let n = item.strip_prefix("minecraft:").unwrap_or(item);
    if n == "shulker_box" {
        return Some(Block::ShulkerBox { colour: 0, facing });
    }
    let dye = n.strip_suffix("_shulker_box")?;
    let i = DYES.iter().position(|d| *d == dye)?;
    Some(Block::ShulkerBox { colour: 1 + i as u8, facing })
}

/// The item a shulker box block drops.
pub fn box_item_of(s: &Block) -> Option<String> {
    match *s {
        Block::ShulkerBox { colour: 0, .. } => Some("minecraft:shulker_box".to_string()),
        Block::ShulkerBox { colour, .. } => Some(format!("minecraft:{}_shulker_box", DYES[colour as usize - 1])),
        _ => None,
    }
}

/// `ShulkerBoxDispenseBehavior.execute` for the dispenser at `pos` facing `facing`, its `slot`
/// holding `st` (a shulker box item): `BlockItem.place` with a `DirectionalPlaceContext` on the
/// cell in front (clicked face: the dispenser's facing over air, else UP; the cell must be
/// replaceable): `setBlock` (flags 11, an empty block entity), the item's contents into the block
/// entity and `setChanged`, the BLOCK_PLACE game event, the item consumed. Then (success or not)
/// `DispenserBlock.dispenseFrom`'s `setItem(slot, what is left)`.
pub fn dispense_box(w: &mut World, pos: BlockPos, slot: usize, st: Stack, facing: Dir) {
    let me = CRef::Block(pos);
    let target = pos.rel(facing);
    let name = container::item_name(st.item);
    let clicked = if w.get(target.rel(Dir::Down)).is_air() { facing } else { Dir::Up };
    let placed = match box_block_of(name, clicked) {
        Some(b) if crate::behavior::can_be_replaced(&w.get(target)) => Some(b),
        _ => None,
    };
    let Some(b) = placed else {
        container::set_item(w, me, slot, Some(st));
        return;
    };
    w.set_block(target, b, flags::NEIGHBORS | flags::CLIENTS | flags::IMMEDIATE);
    // updateBlockEntityComponents: applyComponentsFromItemStack (the contents), then setChanged
    let slots = container::payload_slots(w, st.payload);
    if let Some(c) = w.containers.map.get_mut(&target) {
        c.slots = slots;
    }
    container::set_changed(w, target);
    w.game_event("block_place", target);
    let rest = (st.count > 1).then(|| st.with_count(st.count - 1));
    container::set_item(w, me, slot, rest);
}

/// `Block.dropResources` of a shulker box a piston destroys (its block entity still there): with
/// a drop seed, the item entity the bridge spawns (`rng::Drop::seeded`, the world's n-th drop):
/// the box item with its contents, pickup delay 10 (`setDefaultPickUpDelay`), appended to the
/// entities. Without one, nothing (the caller records the drop).
pub fn drop_box(w: &mut World, p: BlockPos, s: &Block) -> bool {
    let (Some(seed), Some(item)) = (w.drop_seed, box_item_of(s)) else { return false };
    let slots = w.containers.map.get(&p).map(|c| c.slots.clone()).unwrap_or_default();
    let contents: Vec<(usize, Stack)> = slots.iter().enumerate().filter_map(|(i, s)| s.map(|s| (i, s))).collect();
    let payload = container::new_payload(w, &contents);
    let d = crate::rng::Drop::seeded(seed, w.drop_count, p);
    w.drop_count += 1;
    let state = crate::entity::item::ItemState { item, count: 1, pickup_delay: 10, age: 0, payload };
    let id = w.entities.add(crate::entity::EntityKind::Item(state), crate::entity::Vec3::new(d.pos[0], d.pos[1], d.pos[2]), d.y_rot);
    if let Some(e) = w.entities.list.iter_mut().rev().find(|e| e.id == id) {
        e.delta_movement = crate::entity::Vec3::new(d.motion[0], d.motion[1], d.motion[2]);
    }
    true
}
