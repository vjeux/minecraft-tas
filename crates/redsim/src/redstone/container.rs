//! Container contents and item transfer without item entities: droppers pushing one item into a
//! facing container (`DropperBlock.dispenseFrom`), hoppers pushing into the container they face
//! and pulling from the container above (`HopperBlockEntity.pushItemsTick`: 8 gt cooldown,
//! locked while powered), and the comparator readout of a container's fill level
//! (`AbstractContainerMenu.getRedstoneSignalFromContainer`). Every `setItem` / `removeItem` /
//! `setChanged` is replayed in the game's order (each change notifies the comparators next to
//! the container or behind a conductor next to it).
//!
//! Out of scope (a model note is recorded): a dropper whose 2+ non-empty slots make it pick a
//! RANDOM slot, a dropper or dispenser spitting an item into the world (item entities), item
//! entities sucked by hoppers, composters and crafters as targets.

use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::world::World;
use std::collections::HashMap;

/// Items with their max stack sizes.
pub const ITEMS: [(&str, u8); 6] = [("stone", 64), ("redstone", 64), ("snowball", 16), ("ender_pearl", 16), ("wooden_shovel", 1), ("diamond_sword", 1)];

pub fn item_id(name: &str) -> Option<u8> {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    ITEMS.iter().position(|(i, _)| *i == n).map(|i| i as u8)
}

pub fn item_name(id: u8) -> &'static str {
    ITEMS[id as usize].0
}

fn max_stack(id: u8) -> u8 {
    ITEMS[id as usize].1
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stack {
    pub item: u8,
    pub count: u8,
}

/// A container block entity (dropper, dispenser: 9 slots; hopper: 5 slots + cooldown).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContainerBe {
    pub slots: Vec<Option<Stack>>,
    /// `HopperBlockEntity.cooldownTime` (starts at -1).
    pub cooldown: i32,
    /// `HopperBlockEntity.tickedGameTime`.
    pub ticked: i64,
    pub hopper: bool,
}

impl ContainerBe {
    fn new(hopper: bool) -> ContainerBe {
        ContainerBe { slots: vec![None; if hopper { 5 } else { 9 }], cooldown: -1, ticked: 0, hopper }
    }
    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(|s| s.is_none())
    }
    fn inventory_full(&self) -> bool {
        self.slots.iter().all(|s| s.is_some_and(|s| s.count == max_stack(s.item)))
    }
}

#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Containers {
    pub map: HashMap<BlockPos, ContainerBe>,
    /// Hopper tickers in tick order (position, live).
    tickers: Vec<(BlockPos, bool)>,
    pending: Vec<(BlockPos, bool)>,
    ticking: bool,
}

/// Blocks whose block entity is a container we model.
pub fn is_container_block(s: &Block) -> bool {
    matches!(s, Block::Dispenser { .. } | Block::Hopper { .. })
}

/// The block entity appears (`LevelChunk.setBlockState` creating it).
pub fn create(w: &mut World, pos: BlockPos, s: &Block) {
    if w.containers.map.contains_key(&pos) {
        return;
    }
    let hopper = matches!(s, Block::Hopper { .. });
    w.containers.map.insert(pos, ContainerBe::new(hopper));
    if hopper {
        let c = &mut w.containers;
        if c.ticking { c.pending.push((pos, true)) } else { c.tickers.push((pos, true)) }
    }
}

/// The block entity goes (`LevelChunk.removeBlockEntity`): its ticker is dead.
pub fn remove(w: &mut World, pos: BlockPos) {
    if w.containers.map.remove(&pos).is_some() {
        for t in w.containers.tickers.iter_mut().chain(w.containers.pending.iter_mut()) {
            if t.0 == pos {
                t.1 = false;
            }
        }
    }
}

/// `BlockEntity.setChanged`: comparators reading this container re-check.
pub fn set_changed(w: &mut World, pos: BlockPos) {
    let s = w.get(pos);
    if !s.is_air() {
        w.update_neighbour_for_output_signal(pos, s.kind());
    }
}

/// `getRedstoneSignalFromContainer` (floats as in the game).
pub fn analog_output(w: &World, pos: BlockPos) -> i32 {
    let Some(c) = w.containers.map.get(&pos) else { return 0 };
    let mut total: f32 = 0.0;
    for s in c.slots.iter().flatten() {
        total += s.count as f32 / (max_stack(s.item).min(99) as f32);
    }
    total /= c.slots.len() as f32;
    (total * 14.0).floor() as i32 + if total > 0.0 { 1 } else { 0 }
}

/// The container a dropper or hopper can reach at `p` (`HopperBlockEntity.getContainerAt`):
/// Some(true) = a modeled container, None = none (then entities would be searched: none here).
fn container_at(w: &mut World, p: BlockPos) -> Option<BlockPos> {
    let s = w.get(p);
    match s {
        Block::Composter { .. } | Block::Crafter { .. } => {
            w.errors.push(format!("item transfer into {s} at {p} is not modeled"));
            None
        }
        _ if w.containers.map.contains_key(&p) => Some(p),
        _ => None,
    }
}

/// `setItem`: a dropper notifies (`setChanged`); a hopper's override does not.
fn set_item(w: &mut World, pos: BlockPos, slot: usize, st: Option<Stack>) {
    let mut hopper = false;
    if let Some(c) = w.containers.map.get_mut(&pos) {
        c.slots[slot] = st;
        hopper = c.hopper;
    }
    if !hopper {
        set_changed(w, pos);
    }
}

/// `removeItem(slot, 1)`: the split item (the original keeps count - 1, maybe 0 = empty); a
/// dropper notifies, a hopper's override does not.
fn remove_one(w: &mut World, pos: BlockPos, slot: usize) -> Option<Stack> {
    let c = w.containers.map.get_mut(&pos)?;
    let s = c.slots[slot]?;
    c.slots[slot] = if s.count > 1 { Some(Stack { item: s.item, count: s.count - 1 }) } else { None };
    if !c.hopper {
        set_changed(w, pos);
    }
    Some(Stack { item: s.item, count: 1 })
}

/// `HopperBlockEntity.addItem(from, container, stack, direction)`: the rest that did not fit.
fn add_item(w: &mut World, from: Option<BlockPos>, into: BlockPos, mut st: Stack) -> Option<Stack> {
    let size = w.containers.map.get(&into).map(|c| c.slots.len()).unwrap_or(0);
    for slot in 0..size {
        if st.count == 0 {
            break;
        }
        // tryMoveInItem
        let (current, was_empty) = {
            let c = &w.containers.map[&into];
            (c.slots[slot], c.is_empty())
        };
        let mut success = false;
        match current {
            None => {
                set_item(w, into, slot, Some(st));
                st.count = 0;
                success = true;
            }
            Some(cur) if cur.item == st.item && cur.count <= max_stack(cur.item) => {
                let space = max_stack(st.item).saturating_sub(cur.count);
                let n = st.count.min(space);
                st.count -= n;
                if let Some(c) = w.containers.map.get_mut(&into) {
                    c.slots[slot] = Some(Stack { item: cur.item, count: cur.count + n });
                }
                success = n > 0;
            }
            _ => {}
        }
        if success {
            let from_ticked = from.and_then(|f| w.containers.map.get(&f)).filter(|c| c.hopper).map(|c| c.ticked);
            if let Some(c) = w.containers.map.get_mut(&into)
                && was_empty
                && c.hopper
                && c.cooldown <= 8
            {
                let skip = match from_ticked {
                    Some(ft) if c.ticked >= ft => 1,
                    _ => 0,
                };
                c.cooldown = 8 - skip;
            }
            set_changed(w, into);
        }
    }
    (st.count > 0).then_some(st)
}

/// `DropperBlock.dispenseFrom` (the 4 gt trigger tick of a dropper).
pub fn dropper_dispense(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get(&pos) else { return };
    let filled: Vec<usize> = c.slots.iter().enumerate().filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect();
    if filled.is_empty() {
        return; // levelEvent 1001 (click): no block change
    }
    if filled.len() > 1 {
        w.errors.push(format!("dropper at {pos} picks a random slot among {} stacks (not modeled)", filled.len()));
    }
    let slot = filled[0];
    let st = c.slots[slot].unwrap();
    let Block::Dispenser { facing, .. } = w.get(pos) else { return };
    let remaining = match container_at(w, pos.rel(facing)) {
        None => {
            // the item flies out as an entity: harmless to blocks unless a hopper can catch it
            if w.containers.map.values().any(|c| c.hopper) {
                w.errors.push(format!("dropper at {pos} spits an item into the world (item entity, not modeled)"));
            }
            (st.count > 1).then_some(Stack { item: st.item, count: st.count - 1 })
        }
        Some(into) => {
            if add_item(w, Some(pos), into, Stack { item: st.item, count: 1 }).is_none() {
                (st.count > 1).then_some(Stack { item: st.item, count: st.count - 1 })
            } else {
                Some(st)
            }
        }
    };
    set_item(w, pos, slot, remaining);
}

/// A dispenser (not a dropper) firing with items: its dispense behaviors spawn entities.
pub fn dispenser_dispense(w: &mut World, pos: BlockPos) {
    if w.containers.map.get(&pos).is_some_and(|c| !c.is_empty()) {
        w.errors.push(format!("dispenser at {pos} dispenses an item (entities, not modeled)"));
    }
}

/// The block-entity phase for hoppers (before moving blocks: hoppers of a design exist before
/// any moving block is created, so they come first in `blockEntityTickers`).
pub fn tick_hoppers(w: &mut World) {
    w.containers.ticking = true;
    let pending = std::mem::take(&mut w.containers.pending);
    w.containers.tickers.extend(pending);
    let mut i = 0;
    while i < w.containers.tickers.len() {
        let (p, live) = w.containers.tickers[i];
        if !live {
            w.containers.tickers.remove(i);
            continue;
        }
        if matches!(w.get(p), Block::Hopper { .. }) {
            push_items_tick(w, p);
        }
        i += 1;
    }
    w.containers.ticking = false;
}

/// `HopperBlockEntity.pushItemsTick`.
fn push_items_tick(w: &mut World, pos: BlockPos) {
    let time = w.time;
    let Some(c) = w.containers.map.get_mut(&pos) else { return };
    c.cooldown -= 1;
    c.ticked = time;
    if c.cooldown <= 0 {
        c.cooldown = 0;
        try_move_items(w, pos);
    }
}

fn try_move_items(w: &mut World, pos: BlockPos) -> bool {
    let Block::Hopper { facing, enabled } = w.get(pos) else { return false };
    let Some(c) = w.containers.map.get(&pos) else { return false };
    if c.cooldown > 0 || !enabled {
        return false;
    }
    let mut changed = false;
    if !c.is_empty() {
        changed = eject_items(w, pos, facing);
    }
    if !w.containers.map[&pos].inventory_full() {
        changed |= suck_in_items(w, pos);
    }
    if changed {
        if let Some(c) = w.containers.map.get_mut(&pos) {
            c.cooldown = 8;
        }
        set_changed(w, pos);
        return true;
    }
    false
}

fn eject_items(w: &mut World, pos: BlockPos, facing: Dir) -> bool {
    let Some(into) = container_at(w, pos.rel(facing)) else { return false };
    // isFullContainer
    if w.containers.map[&into].slots.iter().all(|s| s.is_some_and(|s| s.count >= max_stack(s.item))) {
        return false;
    }
    for slot in 0..5 {
        let Some(st) = w.containers.map[&pos].slots[slot] else { continue };
        let original = st.count;
        let one = remove_one(w, pos, slot).unwrap();
        if add_item(w, Some(pos), into, one).is_none() {
            set_changed(w, into);
            return true;
        }
        // restore the stack (count back); a 1-stack goes back through setItem
        if let Some(c) = w.containers.map.get_mut(&pos) {
            c.slots[slot] = Some(Stack { item: st.item, count: original });
        }
        if original == 1 {
            set_item(w, pos, slot, Some(st));
        }
    }
    false
}

fn suck_in_items(w: &mut World, pos: BlockPos) -> bool {
    let above = pos.above();
    let Some(from) = container_at(w, above) else {
        return false; // item entities above: not modeled
    };
    let size = w.containers.map[&from].slots.len();
    for slot in 0..size {
        let Some(st) = w.containers.map[&from].slots[slot] else { continue };
        let original = st.count;
        let one = remove_one(w, from, slot).unwrap();
        if add_item(w, Some(from), pos, one).is_none() {
            set_changed(w, from);
            return true;
        }
        if let Some(c) = w.containers.map.get_mut(&from) {
            c.slots[slot] = Some(Stack { item: st.item, count: original });
        }
        if original == 1 {
            set_item(w, from, slot, Some(st));
        }
    }
    false
}

/// Fill a container (the `data merge block` the fuzzer uses: contents replaced, then setChanged).
pub fn set_contents(w: &mut World, pos: BlockPos, items: &[(usize, Stack)]) {
    if let Some(c) = w.containers.map.get_mut(&pos) {
        for s in c.slots.iter_mut() {
            *s = None;
        }
        for (slot, st) in items {
            if *slot < c.slots.len() {
                c.slots[*slot] = Some(*st);
            }
        }
    } else {
        w.errors.push(format!("no container at {pos}"));
        return;
    }
    set_changed(w, pos);
}

pub fn kind_is_container(k: Kind) -> bool {
    matches!(k, Kind::Dispenser(_) | Kind::Hopper)
}
