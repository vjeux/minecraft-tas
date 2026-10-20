//! Container contents and item transfer without item entities: droppers pushing one item into a
//! facing container (`DropperBlock.dispenseFrom`), hoppers pushing into the container they face
//! and pulling from the container above (`HopperBlockEntity.pushItemsTick`: 8 gt cooldown,
//! locked while powered), and the comparator readout of a container's fill level
//! (`AbstractContainerMenu.getRedstoneSignalFromContainer`). Every `setItem` / `removeItem` /
//! `setChanged` is replayed in the game's order (each change notifies the comparators next to
//! the container or behind a conductor next to it).
//!
//! Containers are block entities (by position) or container entities (clk-misc: chest boats and
//! hopper minecarts, by entity id: [`CRef`]), found the game's way
//! (`HopperBlockEntity.getContainerAt`: the block's container, else a container entity touching
//! the 1x1x1 box around the point). The chiseled bookshelf (books only, one per slot, its block
//! state and `last_interacted_slot` updated on every change) and the hopper minecart's pull
//! (`MinecartHopper.suckInItems`) are clk-misc's too.
//!
//! Out of scope (a model note is recorded): a dropper whose 2+ non-empty slots make it pick a
//! RANDOM slot, a dropper or dispenser spitting an item into the world (item entities), item
//! entities sucked by hoppers, composters and crafters as targets.

use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::world::World;
use std::collections::{BTreeMap, HashMap};

/// Items with their max stack sizes (the fuzzer picks among these).
pub const ITEMS: [(&str, u8); 6] = [("stone", 64), ("redstone", 64), ("snowball", 16), ("ender_pearl", 16), ("wooden_shovel", 1), ("diamond_sword", 1)];
/// More items (ids from 6 on), for the entity clocks' contents and the crafters' recipes (clk-misc).
pub const MORE_ITEMS: [(&str, u8); 9] = [
    ("book", 64),
    ("cobblestone", 64),
    ("slime_ball", 64),
    ("slime_block", 64),
    ("iron_ingot", 64),
    ("iron_block", 64),
    ("iron_nugget", 64),
    ("heavy_weighted_pressure_plate", 64),
    ("iron_bars", 64),
];

/// Every item redsim knows, in id order.
pub fn all_items() -> impl Iterator<Item = &'static str> {
    ITEMS.iter().chain(MORE_ITEMS.iter()).map(|(n, _)| *n)
}

/// Damageable items (tools): the repair recipe (`crafting_special_repairitem`, code-defined) can
/// take two of them.
pub fn is_damageable(id: u8) -> bool {
    matches!(item_name(id), "wooden_shovel" | "diamond_sword")
}

fn item_entry(id: u8) -> (&'static str, u8) {
    let i = id as usize;
    if i < ITEMS.len() { ITEMS[i] } else { MORE_ITEMS[i - ITEMS.len()] }
}

pub fn item_id(name: &str) -> Option<u8> {
    let n = name.strip_prefix("minecraft:").unwrap_or(name);
    ITEMS.iter().chain(MORE_ITEMS.iter()).position(|(i, _)| *i == n).map(|i| i as u8)
}

pub fn item_name(id: u8) -> &'static str {
    item_entry(id).0
}

fn max_stack(id: u8) -> u8 {
    item_entry(id).1
}

/// The max stack size of an item by name (64 for an item this list does not know).
pub fn max_stack_of(name: &str) -> u8 {
    item_id(name).map(max_stack).unwrap_or(64)
}

/// `#bookshelf_books` among the modeled items.
fn is_book(id: u8) -> bool {
    item_name(id) == "book"
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stack {
    pub item: u8,
    pub count: u8,
}

/// A container block entity (dropper, dispenser: 9 slots; hopper: 5 slots + cooldown; chiseled
/// bookshelf: 6 slots + last interacted slot) or a container entity's contents.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ContainerBe {
    pub slots: Vec<Option<Stack>>,
    /// `HopperBlockEntity.cooldownTime` (starts at -1).
    pub cooldown: i32,
    /// `HopperBlockEntity.tickedGameTime`.
    pub ticked: i64,
    pub hopper: bool,
    /// A chiseled bookshelf (`ChiseledBookShelfBlockEntity`).
    pub bookshelf: bool,
    /// Its `lastInteractedSlot` (-1 at first).
    pub last_slot: i32,
    /// A crafter (`CrafterBlockEntity`): its disabled slots (bit per slot) and
    /// `craftingTicksRemaining`.
    pub crafter: bool,
    pub disabled: u16,
    pub crafting_ticks: i32,
}

impl ContainerBe {
    fn new(hopper: bool) -> ContainerBe {
        ContainerBe { slots: vec![None; if hopper { 5 } else { 9 }], cooldown: -1, ticked: 0, hopper, bookshelf: false, last_slot: -1, crafter: false, disabled: 0, crafting_ticks: 0 }
    }
    fn bookshelf() -> ContainerBe {
        ContainerBe { slots: vec![None; 6], bookshelf: true, ..ContainerBe::new(false) }
    }
    fn crafter() -> ContainerBe {
        ContainerBe { crafter: true, ..ContainerBe::new(false) }
    }
    /// A container entity's contents (chest boat: 27 slots, hopper minecart: 5).
    pub fn entity(size: usize) -> ContainerBe {
        ContainerBe { slots: vec![None; size], ..ContainerBe::new(false) }
    }
    fn slot_disabled(&self, slot: usize) -> bool {
        self.crafter && self.disabled & (1 << slot) != 0
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
    /// Container entities' contents, by entity id (clk-misc).
    pub entities: BTreeMap<u32, ContainerBe>,
    /// Hopper tickers in tick order (position, live).
    tickers: Vec<(BlockPos, bool)>,
    pending: Vec<(BlockPos, bool)>,
    ticking: bool,
}

/// A container: a block entity at a position or a container entity (by id).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CRef {
    Block(BlockPos),
    Entity(u32),
}

impl Containers {
    pub fn get(&self, r: CRef) -> Option<&ContainerBe> {
        match r {
            CRef::Block(p) => self.map.get(&p),
            CRef::Entity(id) => self.entities.get(&id),
        }
    }
    fn get_mut(&mut self, r: CRef) -> Option<&mut ContainerBe> {
        match r {
            CRef::Block(p) => self.map.get_mut(&p),
            CRef::Entity(id) => self.entities.get_mut(&id),
        }
    }

    /// Every container (by position: slots, hopper cooldown, `tickedGameTime` relative to `now`)
    /// and the live hopper ticker order, for [`crate::world::World::state_key`].
    pub(crate) fn key_into<H: std::hash::Hasher>(&self, now: i64, h: &mut H) {
        use std::hash::Hash;
        let mut all: Vec<(&BlockPos, &ContainerBe)> = self.map.iter().collect();
        all.sort_by_key(|(p, _)| **p);
        for (p, c) in all {
            // tickedGameTime only matters (and only advances) for hoppers: a dropper's stays 0
            let ticked = if c.hopper { now - c.ticked } else { 0 };
            (p, &c.slots, c.cooldown, ticked, c.hopper).hash(h);
            if c.bookshelf {
                c.last_slot.hash(h);
            }
            if c.crafter {
                (c.disabled, c.crafting_ticks).hash(h);
            }
        }
        0xfeu8.hash(h);
        let order: Vec<BlockPos> = self.tickers.iter().chain(self.pending.iter()).filter(|(_, live)| *live).map(|(p, _)| *p).collect();
        order.hash(h);
        if !self.entities.is_empty() {
            0xfdu8.hash(h);
            for (id, c) in &self.entities {
                (id, &c.slots).hash(h);
            }
        }
    }
}

/// Blocks whose block entity is a container we model.
pub fn is_container_block(s: &Block) -> bool {
    matches!(s, Block::Dispenser { .. } | Block::Hopper { .. } | Block::ChiseledBookshelf { .. } | Block::Crafter { .. })
}

/// The block entity appears (`LevelChunk.setBlockState` creating it). Hoppers and crafters tick
/// (`blockEntityTickers`, in creation order).
pub fn create(w: &mut World, pos: BlockPos, s: &Block) {
    if w.containers.map.contains_key(&pos) {
        return;
    }
    if let Block::ChiseledBookshelf { .. } = s {
        w.containers.map.insert(pos, ContainerBe::bookshelf());
        return;
    }
    let hopper = matches!(s, Block::Hopper { .. });
    let crafter = matches!(s, Block::Crafter { .. });
    w.containers.map.insert(pos, if crafter { ContainerBe::crafter() } else { ContainerBe::new(hopper) });
    if hopper || crafter {
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

/// `Container.setChanged` of any container (a container entity's does nothing).
fn changed(w: &mut World, r: CRef) {
    if let CRef::Block(p) = r {
        set_changed(w, p);
    }
}

/// `getRedstoneSignalFromContainer` (floats as in the game); a chiseled bookshelf reads
/// `lastInteractedSlot + 1`.
pub fn analog_output(w: &World, pos: BlockPos) -> i32 {
    let Some(c) = w.containers.map.get(&pos) else { return 0 };
    if c.bookshelf {
        return c.last_slot + 1;
    }
    if c.crafter {
        // CrafterBlockEntity.getRedstoneSignal: filled or disabled slots
        return (0..9).filter(|&i| c.slots[i].is_some() || c.slot_disabled(i)).count() as i32;
    }
    signal_of(c)
}

/// `AbstractContainerMenu.getRedstoneSignalFromContainer` of any container's contents.
pub fn signal_of(c: &ContainerBe) -> i32 {
    let mut total: f32 = 0.0;
    for s in c.slots.iter().flatten() {
        total += s.count as f32 / (max_stack(s.item).min(99) as f32);
    }
    total /= c.slots.len() as f32;
    (total * 14.0).floor() as i32 + if total > 0.0 { 1 } else { 0 }
}

/// The container a dropper or hopper can reach at `p` (`HopperBlockEntity.getContainerAt`).
fn container_at(w: &mut World, p: BlockPos) -> Option<CRef> {
    container_at_point(w, p, [p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5])
}

/// `getContainerAt(level, pos, state, x, y, z)`: the block's container, else a container entity
/// (alive) touching the 1x1x1 box centered on the point.
fn container_at_point(w: &mut World, p: BlockPos, c: [f64; 3]) -> Option<CRef> {
    let s = w.get(p);
    match s {
        Block::Composter { .. } => {
            w.errors.push(format!("item transfer into {s} at {p} is not modeled"));
            None
        }
        _ if w.containers.map.contains_key(&p) => Some(CRef::Block(p)),
        _ => {
            let bb = crate::entity::Aabb::new(c[0] - 0.5, c[1] - 0.5, c[2] - 0.5, c[0] + 0.5, c[1] + 0.5, c[2] + 0.5);
            let found: Vec<u32> = w.get_entities(bb, |e| w_has_container(e)).into_iter().map(|i| w.entities.list[i].id).collect();
            let found: Vec<u32> = found.into_iter().filter(|id| w.containers.entities.contains_key(id)).collect();
            if found.len() > 1 {
                w.errors.push(format!("{} container entities at {p}: the game picks one at random (not modeled)", found.len()));
            }
            found.first().map(|id| CRef::Entity(*id))
        }
    }
}

/// `EntitySelector.CONTAINER_ENTITY_SELECTOR`: a container entity, alive.
fn w_has_container(e: &crate::entity::Entity) -> bool {
    !e.removed
        && match e.kind {
            crate::entity::EntityKind::Boat(b) => b.chest,
            crate::entity::EntityKind::Minecart(m) => m.hopper,
            _ => false,
        }
}

/// `ChiseledBookShelfBlockEntity.updateState(slot)`: the slot flags follow the contents, then
/// `setBlockAndUpdate` and a BLOCK_CHANGE vibration.
fn bookshelf_update_state(w: &mut World, pos: BlockPos, slot: usize) {
    let Some(c) = w.containers.map.get_mut(&pos) else { return };
    c.last_slot = slot as i32;
    let mut flags = 0u8;
    for (n, s) in c.slots.iter().enumerate() {
        if s.is_some() {
            flags |= 1 << n;
        }
    }
    let Block::ChiseledBookshelf { facing, .. } = w.get(pos) else { return };
    let ns = Block::ChiseledBookshelf { facing, slots: flags };
    w.set_block_and_update(pos, ns);
    w.game_event_state("block_change", pos, ns);
}

/// `setItem`: a dropper notifies (`setChanged`); a hopper's override does not; a chiseled
/// bookshelf takes books only and updates its state; a container entity just stores it.
fn set_item(w: &mut World, r: CRef, slot: usize, st: Option<Stack>) {
    if let CRef::Block(pos) = r
        && w.containers.map.get(&pos).is_some_and(|c| c.bookshelf)
    {
        match st {
            Some(s) if is_book(s.item) => {
                w.containers.map.get_mut(&pos).unwrap().slots[slot] = st;
                bookshelf_update_state(w, pos, slot);
            }
            None => {
                bookshelf_remove(w, pos, slot);
            }
            Some(_) => {}
        }
        return;
    }
    let mut hopper = false;
    if let Some(c) = w.containers.get_mut(r) {
        if c.slot_disabled(slot) {
            // CrafterBlockEntity.setItem: a disabled slot is enabled first (setSlotState: setChanged)
            c.disabled &= !(1 << slot);
            changed(w, r);
        }
    }
    if let Some(c) = w.containers.get_mut(r) {
        c.slots[slot] = st;
        hopper = c.hopper;
    }
    if !hopper {
        changed(w, r);
    }
}

/// `ChiseledBookShelfBlockEntity.removeItem(slot, count)`: the whole slot, then its state.
fn bookshelf_remove(w: &mut World, pos: BlockPos, slot: usize) -> Option<Stack> {
    let c = w.containers.map.get_mut(&pos)?;
    let got = c.slots[slot].take();
    if got.is_some() {
        bookshelf_update_state(w, pos, slot);
    }
    got
}

/// `removeItem(slot, 1)`: the split item (the original keeps count - 1, maybe 0 = empty); a
/// dropper notifies, a hopper's override does not, a bookshelf gives its whole slot.
fn remove_one(w: &mut World, r: CRef, slot: usize) -> Option<Stack> {
    if let CRef::Block(pos) = r
        && w.containers.map.get(&pos).is_some_and(|c| c.bookshelf)
    {
        return bookshelf_remove(w, pos, slot);
    }
    let c = w.containers.get_mut(r)?;
    let s = c.slots[slot]?;
    c.slots[slot] = if s.count > 1 { Some(Stack { item: s.item, count: s.count - 1 }) } else { None };
    if !c.hopper {
        changed(w, r);
    }
    Some(Stack { item: s.item, count: 1 })
}

/// The item taken out by [`remove_one`] did not fit: `itemStack.setCount(originalCount)` on the
/// stack still in the slot (a bookshelf's left the slot), then `setItem` when it was a single item.
fn restore(w: &mut World, r: CRef, slot: usize, st: Stack, original: u8) {
    let bookshelf = matches!(r, CRef::Block(p) if w.containers.map.get(&p).is_some_and(|c| c.bookshelf));
    if !bookshelf && let Some(c) = w.containers.get_mut(r) {
        c.slots[slot] = Some(Stack { item: st.item, count: original });
    }
    if original == 1 {
        set_item(w, r, slot, Some(st));
    }
}

/// `canPlaceItem`: a bookshelf takes a book into an empty slot; a crafter fills its slots evenly
/// (`CrafterBlockEntity.canPlaceItem`: never a disabled slot, never a full stack, and not a slot
/// while a later enabled slot is empty or holds a smaller stack of the same item); the others
/// anything.
fn can_place_item(w: &World, r: CRef, slot: usize, st: Stack) -> bool {
    match w.containers.get(r) {
        Some(c) if c.bookshelf => is_book(st.item) && c.slots[slot].is_none(),
        Some(c) if c.crafter => {
            if c.slot_disabled(slot) {
                return false;
            }
            let Some(cur) = c.slots[slot] else { return true };
            if cur.count >= max_stack(cur.item) {
                return false;
            }
            // smallerStackExist(count, stack, slot)
            !(slot + 1..9).any(|i| !c.slot_disabled(i) && c.slots[i].is_none_or(|s| s.count < cur.count && s.item == cur.item))
        }
        _ => true,
    }
}

/// `canTakeItem(into, slot, stack)`: a bookshelf only gives an item that `into` can hold.
fn can_take_item(w: &World, from: CRef, into: CRef, st: Stack) -> bool {
    let bookshelf = w.containers.get(from).is_some_and(|c| c.bookshelf);
    if !bookshelf {
        return true;
    }
    let Some(c) = w.containers.get(into) else { return false };
    c.slots.iter().any(|t| match t {
        None => true,
        Some(t) => t.item == st.item && t.count as u32 + st.count as u32 <= max_stack(t.item).min(99) as u32,
    })
}

/// `HopperBlockEntity.addItem(from, container, stack, direction)`: the rest that did not fit.
fn add_item(w: &mut World, from: Option<CRef>, into: CRef, mut st: Stack) -> Option<Stack> {
    let size = w.containers.get(into).map(|c| c.slots.len()).unwrap_or(0);
    for slot in 0..size {
        if st.count == 0 {
            break;
        }
        // tryMoveInItem
        if !can_place_item(w, into, slot, st) {
            continue;
        }
        let (current, was_empty) = {
            let c = w.containers.get(into).unwrap();
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
                if let Some(c) = w.containers.get_mut(into) {
                    c.slots[slot] = Some(Stack { item: cur.item, count: cur.count + n });
                }
                success = n > 0;
            }
            _ => {}
        }
        if success {
            let from_ticked = from.and_then(|f| w.containers.get(f)).filter(|c| c.hopper).map(|c| c.ticked);
            if let Some(c) = w.containers.get_mut(into)
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
            changed(w, into);
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
    let me = CRef::Block(pos);
    let remaining = match container_at(w, pos.rel(facing)) {
        None => {
            // the item flies out as an entity: harmless to blocks unless a hopper can catch it
            if w.containers.map.values().any(|c| c.hopper) {
                w.errors.push(format!("dropper at {pos} spits an item into the world (item entity, not modeled)"));
            }
            (st.count > 1).then_some(Stack { item: st.item, count: st.count - 1 })
        }
        Some(into) => {
            if add_item(w, Some(me), into, Stack { item: st.item, count: 1 }).is_none() {
                (st.count > 1).then_some(Stack { item: st.item, count: st.count - 1 })
            } else {
                Some(st)
            }
        }
    };
    set_item(w, me, slot, remaining);
}

/// A dispenser (not a dropper) firing with items: its dispense behaviors spawn entities.
pub fn dispenser_dispense(w: &mut World, pos: BlockPos) {
    if w.containers.map.get(&pos).is_some_and(|c| !c.is_empty()) {
        w.errors.push(format!("dispenser at {pos} dispenses an item (entities, not modeled)"));
    } else if w.containers.map.contains_key(&pos) {
        // DispenserBlock.dispenseFrom with no item: levelEvent 1001 and BLOCK_ACTIVATE
        let s = w.get(pos);
        w.game_event_state("block_activate", pos, s);
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
        match w.get(p) {
            Block::Hopper { .. } => push_items_tick(w, p),
            Block::Crafter { .. } => crafter_server_tick(w, p),
            Block::DaylightDetector { .. } => crate::daylight::tick(w, p),
            _ => {}
        }
        i += 1;
    }
    w.containers.ticking = false;
}

/// A block entity without contents that ticks (a daylight detector) joins the tickers.
pub fn add_ticker(w: &mut World, pos: BlockPos) {
    let c = &mut w.containers;
    if c.tickers.iter().chain(c.pending.iter()).any(|&(p, live)| p == pos && live) {
        return;
    }
    if c.ticking { c.pending.push((pos, true)) } else { c.tickers.push((pos, true)) }
}

/// Its block entity goes: the ticker is dead.
pub fn remove_ticker(w: &mut World, pos: BlockPos) {
    for t in w.containers.tickers.iter_mut().chain(w.containers.pending.iter_mut()) {
        if t.0 == pos {
            t.1 = false;
        }
    }
}

/// `CrafterBlockEntity.serverTick`: the crafting flag goes off 6 gt after a craft (with block
/// updates: `setBlockAndUpdate`).
fn crafter_server_tick(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get_mut(&pos) else { return };
    let left = c.crafting_ticks - 1;
    if left >= 0 {
        c.crafting_ticks = left;
        if left == 0
            && let Block::Crafter { orientation, triggered, .. } = w.get(pos)
        {
            w.set_block_and_update(pos, Block::Crafter { orientation, triggered, crafting: false });
        }
    }
}

/// The front of a crafter's `orientation` (redsim's index into `block::ORIENTATIONS`, the
/// game's `FrontAndTop` order).
fn crafter_front(orientation: u8) -> Dir {
    // FrontAndTop order: down_east, down_north, down_south, down_west, up_east, up_north,
    // up_south, up_west, west_up, east_up, north_up, south_up
    match orientation {
        0..=3 => Dir::Down,
        4..=7 => Dir::Up,
        8 => Dir::West,
        9 => Dir::East,
        10 => Dir::North,
        _ => Dir::South,
    }
}

/// `CrafterBlock.dispenseFrom` (its 4 gt tick): the grid's recipe crafts (crafting flag on for
/// 6 gt), the result goes into the container in front (one item at a time into a crafter or
/// when it does not fit in one stack, else the whole stack), then every filled slot loses one
/// item. No recipe: nothing changes (a sound).
pub fn crafter_dispense(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get(&pos) else { return };
    let grid = c.slots.clone();
    let (item, count) = match super::crafting::find(&grid) {
        Ok(Some(r)) => r,
        Ok(None) => return, // levelEvent 1050 (a click)
        Err(e) => {
            w.errors.push(format!("crafter at {pos}: {e}"));
            return;
        }
    };
    let Block::Crafter { orientation, triggered, .. } = w.get(pos) else { return };
    w.containers.map.get_mut(&pos).unwrap().crafting_ticks = 6;
    w.set_block(pos, Block::Crafter { orientation, triggered, crafting: true }, crate::world::flags::CLIENTS);
    // dispenseItem
    let front = crafter_front(orientation);
    let me = CRef::Block(pos);
    let mut remaining = count;
    if let Some(into) = container_at(w, pos.rel(front)) {
        let into_crafter = w.containers.get(into).is_some_and(|c| c.crafter);
        if into_crafter || count > max_stack(item).min(99) {
            while remaining > 0 {
                if add_item(w, Some(me), into, Stack { item, count: 1 }).is_some() {
                    break;
                }
                remaining -= 1;
            }
        } else {
            while remaining > 0 {
                let old = remaining;
                remaining = add_item(w, Some(me), into, Stack { item, count: remaining }).map(|s| s.count).unwrap_or(0);
                if old == remaining {
                    break;
                }
            }
        }
    }
    if remaining > 0 {
        w.errors.push(format!("crafter at {pos} spits {remaining} {} into the world (an item entity with a random motion: not modeled)", item_name(item)));
    }
    // (no recipe of redsim's items leaves a crafting remainder)
    if let Some(c) = w.containers.map.get_mut(&pos) {
        for s in c.slots.iter_mut() {
            if let Some(st) = s {
                st.count -= 1;
                if st.count == 0 {
                    *s = None;
                }
            }
        }
    }
    set_changed(w, pos);
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
    if w.containers.get(into).unwrap().slots.iter().all(|s| s.is_some_and(|s| s.count >= max_stack(s.item))) {
        return false;
    }
    let me = CRef::Block(pos);
    for slot in 0..5 {
        let Some(st) = w.containers.map[&pos].slots[slot] else { continue };
        let original = st.count;
        let one = remove_one(w, me, slot).unwrap();
        if add_item(w, Some(me), into, one).is_none() {
            changed(w, into);
            return true;
        }
        restore(w, me, slot, st, original);
    }
    false
}

/// `HopperBlockEntity.suckInItems(level, hopper)` of a hopper block.
fn suck_in_items(w: &mut World, pos: BlockPos) -> bool {
    let above = pos.above();
    match container_at(w, above) {
        Some(from) => suck_from(w, CRef::Block(pos), from),
        None => {
            // isBlocked: a full-collision block above (not in #does_not_block_hoppers) hides the
            // items above a hopper block
            let s = w.get(above);
            let blocked = crate::behavior::full_cube(&s);
            !blocked && item_entities_above(w, CRef::Block(pos))
        }
    }
}

/// `suckInItems` with a source container: the first slot whose item moves (`tryTakeInItemFromSlot`
/// over the source's slots, from above: `Direction.DOWN`).
fn suck_from(w: &mut World, into: CRef, from: CRef) -> bool {
    let size = w.containers.get(from).map(|c| c.slots.len()).unwrap_or(0);
    for slot in 0..size {
        if try_take_in_item_from_slot(w, into, from, slot) {
            return true;
        }
    }
    false
}

/// `tryTakeInItemFromSlot(hopper, container, slot, direction)`.
fn try_take_in_item_from_slot(w: &mut World, into: CRef, from: CRef, slot: usize) -> bool {
    let Some(st) = w.containers.get(from).and_then(|c| c.slots[slot]) else { return false };
    if !can_take_item(w, from, into, st) {
        return false;
    }
    let original = st.count;
    let Some(one) = remove_one(w, from, slot) else { return false };
    if add_item(w, Some(from), into, one).is_none() {
        changed(w, from);
        return true;
    }
    restore(w, from, slot, st, original);
    false
}

/// ---- item entities (clockinfra): the item entities a hopper (block or minecart) with no
/// container above it pulls: `getItemsAtAndAbove` (`Hopper.SUCK_AABB` = column(16, 11, 32) moved by
/// (levelX - 0.5, levelY - 0.5, levelZ - 0.5); a hopper block's level point is its center, a
/// hopper minecart's (x, y + 0.5, z)) and `addItem(container, itemEntity)` for each, in list order,
/// until one is taken whole.
fn item_entities_above(w: &mut World, hopper: CRef) -> bool {
    let (lx, ly, lz) = match hopper {
        CRef::Block(p) => (p.x as f64 + 0.5, p.y as f64 + 0.5, p.z as f64 + 0.5),
        CRef::Entity(id) => match w.entities.list.iter().find(|e| e.id == id && !e.removed) {
            Some(e) => (e.pos.x, e.pos.y + 0.5, e.pos.z),
            None => return false,
        },
    };
    let (dx, dy, dz) = (lx - 0.5, ly - 0.5, lz - 0.5);
    let bb = crate::entity::Aabb::new(0.0 + dx, 0.6875 + dy, 0.0 + dz, 1.0 + dx, 2.0 + dy, 1.0 + dz);
    take_item_entities(w, hopper, bb)
}

/// `HopperBlockEntity.addItem(container, itemEntity)` for every live item entity in `bb`: true
/// when one went in whole (it is discarded; a partly taken stack keeps the rest).
fn take_item_entities(w: &mut World, hopper: CRef, bb: crate::entity::Aabb) -> bool {
    use crate::entity::EntityKind;
    for i in w.get_entities(bb, |e| matches!(e.kind, EntityKind::Item(_))) {
        let EntityKind::Item(s) = w.entities.list[i].kind.clone() else { continue };
        let Some(item) = item_id(&s.item) else {
            w.errors.push(format!("item entity {} is not a modeled item", s.item));
            continue;
        };
        if s.count == 0 {
            continue;
        }
        match add_item(w, None, hopper, Stack { item, count: s.count }) {
            None => {
                if let EntityKind::Item(st) = &mut w.entities.list[i].kind {
                    st.count = 0;
                }
                w.entities.list[i].removed = true;
                return true;
            }
            Some(rest) => {
                if let EntityKind::Item(st) = &mut w.entities.list[i].kind {
                    st.count = rest.count;
                }
            }
        }
    }
    false
}

/// `MinecartHopper.suckInItems` of entity `i`: `HopperBlockEntity.suckInItems` with the cart as
/// the hopper (the point `(x, y + 0.5 + 1, z)`: a container there, else the item entities in the
/// suck box), then the item entities in its box inflated by 0.25 horizontally: whether an item moved.
pub fn hopper_minecart_suck(w: &mut World, i: usize) -> bool {
    let e = &w.entities.list[i];
    let me = CRef::Entity(e.id);
    let inflated = e.bb.inflate(0.25, 0.0, 0.25);
    let (x, y, z) = (e.pos.x, e.pos.y + 0.5 + 1.0, e.pos.z);
    let p = BlockPos::new(crate::entity::mth::floor(x), crate::entity::mth::floor(y), crate::entity::mth::floor(z));
    let first = match container_at_point(w, p, [x, y, z]) {
        Some(from) if from != me => suck_from(w, me, from),
        // a minecart is not grid-aligned: never blocked
        _ => item_entities_above(w, me),
    };
    first || take_item_entities(w, me, inflated)
}

/// A container entity's contents from its `Items` NBT (clk-misc: chest boats, hopper minecarts).
pub fn entity_items(nbt: &str) -> Result<Vec<(usize, Stack)>, String> {
    let Snbt::Compound(fields) = parse_snbt(nbt)? else { return Err("not a compound".into()) };
    match fields.iter().find(|(k, _)| k == "Items") {
        Some((_, v)) => snbt_items(v),
        None => Ok(Vec::new()),
    }
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

/// A parsed SNBT value (the subset block-entity data needs: compounds, lists, strings, integers).
#[derive(Clone, Debug, PartialEq)]
pub enum Snbt {
    Compound(Vec<(String, Snbt)>),
    List(Vec<Snbt>),
    Str(String),
    /// An integer with any suffix (`0b`, `3`, `-1s`, `5L`): the value.
    Int(i64),
    /// Anything else unquoted (`true`, `1.5f`...).
    Word(String),
}

/// Parse SNBT (`TagParser` syntax: `{Items:[{Slot:0b,id:"minecraft:stone",count:3}]}`).
pub fn parse_snbt(s: &str) -> Result<Snbt, String> {
    struct P<'a> {
        b: &'a [u8],
        i: usize,
    }
    impl P<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && (self.b[self.i] as char).is_ascii_whitespace() {
                self.i += 1;
            }
        }
        fn eat(&mut self, c: u8) -> bool {
            self.ws();
            if self.b.get(self.i) == Some(&c) {
                self.i += 1;
                true
            } else {
                false
            }
        }
        fn err(&self, what: &str) -> String {
            format!("SNBT: {what} at byte {}", self.i)
        }
        fn word(&mut self) -> Result<String, String> {
            self.ws();
            if let Some(&q) = self.b.get(self.i).filter(|c| **c == b'"' || **c == b'\'') {
                self.i += 1;
                let mut out = String::new();
                while let Some(&c) = self.b.get(self.i) {
                    self.i += 1;
                    if c == q {
                        return Ok(out);
                    }
                    if c == b'\\' {
                        let Some(&n) = self.b.get(self.i) else { break };
                        self.i += 1;
                        out.push(n as char);
                    } else {
                        out.push(c as char);
                    }
                }
                return Err(self.err("unterminated string"));
            }
            let start = self.i;
            while self.i < self.b.len() && matches!(self.b[self.i], b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'_' | b'-' | b'.' | b'+') {
                self.i += 1;
            }
            if start == self.i {
                return Err(self.err("expected a word"));
            }
            Ok(String::from_utf8_lossy(&self.b[start..self.i]).into_owned())
        }
        fn value(&mut self) -> Result<Snbt, String> {
            self.ws();
            match self.b.get(self.i) {
                Some(b'{') => {
                    self.i += 1;
                    let mut out = Vec::new();
                    if self.eat(b'}') {
                        return Ok(Snbt::Compound(out));
                    }
                    loop {
                        let k = self.word()?;
                        if !self.eat(b':') {
                            return Err(self.err("expected ':'"));
                        }
                        out.push((k, self.value()?));
                        if self.eat(b'}') {
                            return Ok(Snbt::Compound(out));
                        }
                        if !self.eat(b',') {
                            return Err(self.err("expected ',' or '}'"));
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    // a typed array (`[I; 1, 2]`, `[B; ...]`, `[L; ...]`): a list of integers
                    self.ws();
                    if self.b.len() > self.i + 1 && matches!(self.b[self.i], b'I' | b'B' | b'L') && self.b[self.i + 1] == b';' {
                        self.i += 2;
                    }
                    let mut out = Vec::new();
                    if self.eat(b']') {
                        return Ok(Snbt::List(out));
                    }
                    loop {
                        out.push(self.value()?);
                        if self.eat(b']') {
                            return Ok(Snbt::List(out));
                        }
                        if !self.eat(b',') {
                            return Err(self.err("expected ',' or ']'"));
                        }
                    }
                }
                Some(b'"') | Some(b'\'') => Ok(Snbt::Str(self.word()?)),
                Some(_) => {
                    let w = self.word()?;
                    let digits = w.trim_end_matches(['b', 'B', 's', 'S', 'l', 'L', 'i', 'I']);
                    match digits.parse::<i64>() {
                        Ok(n) if !digits.is_empty() => Ok(Snbt::Int(n)),
                        _ => Ok(Snbt::Word(w)),
                    }
                }
                None => Err(self.err("unexpected end")),
            }
        }
    }
    let mut p = P { b: s.as_bytes(), i: 0 };
    let v = p.value()?;
    p.ws();
    if p.i != p.b.len() {
        return Err(p.err("trailing text"));
    }
    Ok(v)
}

/// The `Items` list of block-entity SNBT: (slot, stack) per entry (`count` defaults to 1, as in
/// `ItemStack.MAP_CODEC`).
fn snbt_items(v: &Snbt) -> Result<Vec<(usize, Stack)>, String> {
    let Snbt::List(list) = v else { return Err("Items is not a list".into()) };
    let mut out = Vec::new();
    for e in list {
        let Snbt::Compound(fields) = e else { return Err("an Items entry is not a compound".into()) };
        let (mut slot, mut id, mut count) = (None, None, 1i64);
        for (k, v) in fields {
            match (k.as_str(), v) {
                ("Slot", Snbt::Int(n)) => slot = Some(*n),
                ("id", Snbt::Str(s) | Snbt::Word(s)) => id = Some(s.clone()),
                ("count", Snbt::Int(n)) => count = *n,
                (other, _) => return Err(format!("item field {other} is not modeled")),
            }
        }
        let slot = slot.ok_or("an Items entry has no Slot")?;
        let name = id.ok_or("an Items entry has no id")?;
        let item = item_id(&name).ok_or_else(|| format!("item {name} is not modeled (redsim knows {:?})", ITEMS.map(|i| i.0)))?;
        if !(1..=max_stack(item) as i64).contains(&count) || !(0..=255).contains(&slot) {
            return Err(format!("bad Items entry: slot {slot}, {count} {name}"));
        }
        out.push((slot as usize, Stack { item, count: count as u8 }));
    }
    Ok(out)
}

/// A design's `nbt:` line on a placed container, exactly as `data merge block x y z <snbt>`
/// (`DataCommands.mergeData` + `BlockDataAccessor.setData`): `Items` replaces the contents (slots
/// beyond the container's size are dropped, as `loadAllItems` does), `TransferCooldown` sets a
/// hopper's cooldown; when something changed, `setChanged` notifies the comparators reading it.
/// Returns whether it changed. Other keys, other block entities and unknown items are errors
/// (the clock judge in the game decides those designs).
pub fn apply_block_entity(w: &mut World, pos: BlockPos, snbt: &str) -> Result<bool, String> {
    let Snbt::Compound(fields) = parse_snbt(snbt)? else { return Err(format!("nbt at {pos}: not a compound")) };
    let Some(c) = w.containers.map.get(&pos) else { return Err(format!("nbt at {pos}: {} has no modeled block entity", w.get(pos))) };
    let mut slots = c.slots.clone();
    let mut cooldown = c.cooldown;
    let mut last_slot = c.last_slot;
    let mut disabled = c.disabled;
    let mut crafting_ticks = c.crafting_ticks;
    let mut disabled_given = None;
    for (k, v) in &fields {
        match (k.as_str(), v) {
            ("Items", v) => {
                slots = vec![None; c.slots.len()];
                for (slot, st) in snbt_items(v).map_err(|e| format!("nbt at {pos}: {e}"))? {
                    if slot < slots.len() {
                        slots[slot] = Some(st);
                    }
                }
            }
            ("TransferCooldown", Snbt::Int(n)) if c.hopper => cooldown = *n as i32,
            ("last_interacted_slot", Snbt::Int(n)) if c.bookshelf => last_slot = *n as i32,
            ("crafting_ticks_remaining", Snbt::Int(n)) if c.crafter => crafting_ticks = *n as i32,
            ("disabled_slots", Snbt::List(l)) if c.crafter => {
                let mut bits = 0u16;
                for x in l {
                    let Snbt::Int(i) = x else { return Err(format!("nbt at {pos}: disabled_slots holds {x:?}")) };
                    if (0..9).contains(i) {
                        bits |= 1 << i;
                    }
                }
                disabled_given = Some(bits);
            }
            // the menu's copy of the triggered flag: nothing reads it
            ("triggered", Snbt::Int(_)) if c.crafter => {}
            (other, _) => return Err(format!("nbt at {pos}: key {other} is not modeled here")),
        }
    }
    if c.crafter {
        // loadAdditional: the disabled slots (given or kept) that are empty after the load
        let bits = disabled_given.unwrap_or(disabled);
        disabled = (0..9).filter(|&i| bits & (1 << i) != 0 && slots[i].is_none()).fold(0u16, |b, i| b | (1 << i));
    }
    let changed = slots != c.slots || cooldown != c.cooldown || last_slot != c.last_slot || disabled != c.disabled || crafting_ticks != c.crafting_ticks;
    if changed {
        let c = w.containers.map.get_mut(&pos).unwrap();
        c.slots = slots;
        c.cooldown = cooldown;
        c.last_slot = last_slot;
        c.disabled = disabled;
        c.crafting_ticks = crafting_ticks;
        set_changed(w, pos);
    }
    Ok(changed)
}

/// Every `nbt:` line of a design (position order), right after `place_raw` and before
/// `place_updates`, like the clock judge's placement in the game.
pub fn apply_block_entities<'a>(w: &mut World, lines: impl IntoIterator<Item = (BlockPos, &'a str)>) -> Result<(), String> {
    for (p, snbt) in lines {
        apply_block_entity(w, p, snbt)?;
    }
    Ok(())
}

pub fn kind_is_container(k: Kind) -> bool {
    matches!(k, Kind::Dispenser(_) | Kind::Hopper | Kind::ChiseledBookshelf | Kind::Crafter)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    #[test]
    fn snbt_items_parse() {
        let v = parse_snbt("{Items:[{Slot:0b,id:\"minecraft:stone\",count:3},{Slot:2b, id:'minecraft:snowball'}],TransferCooldown:-1}").unwrap();
        let Snbt::Compound(f) = &v else { panic!() };
        assert_eq!(f[1], ("TransferCooldown".to_string(), Snbt::Int(-1)));
        let items = snbt_items(&f[0].1).unwrap();
        assert_eq!(items, vec![(0, Stack { item: 0, count: 3 }), (2, Stack { item: 2, count: 1 })]);
        assert!(parse_snbt("{Items:[}").is_err());
        assert!(parse_snbt("{a:1} x").is_err());
        assert!(snbt_items(&parse_snbt("[{Slot:0b,id:\"minecraft:dirt_path\"}]").unwrap()).is_err());
        assert!(snbt_items(&parse_snbt("[{Slot:0b,id:\"minecraft:snowball\",count:17}]").unwrap()).is_err());
    }

    #[test]
    fn nbt_line_fills_a_hopper_like_data_merge() {
        let mut w = World::new(BlockPos::new(-4, -4, -4), [9, 9, 9]);
        let hopper = BlockPos::new(0, 0, 0);
        w.place_raw(&[(hopper, Block::parse("minecraft:hopper[enabled=true,facing=down]").unwrap())]);
        assert!(apply_block_entity(&mut w, hopper, "{Items:[{Slot:4b,id:\"minecraft:diamond_sword\",count:1}],TransferCooldown:3}").unwrap());
        let c = &w.containers.map[&hopper];
        assert_eq!(c.slots[4], Some(Stack { item: 5, count: 1 }));
        assert_eq!(c.cooldown, 3);
        assert_eq!(analog_output(&w, hopper), 3); // 1/5 full: floor(14 * 0.2) + 1
        // the same data again changes nothing (data merge's ERROR_MERGE_UNCHANGED: no setChanged)
        assert!(!apply_block_entity(&mut w, hopper, "{Items:[{Slot:4b,id:\"minecraft:diamond_sword\",count:1}]}").unwrap());
        assert!(apply_block_entity(&mut w, BlockPos::new(1, 0, 0), "{Items:[]}").is_err());
        assert!(apply_block_entity(&mut w, hopper, "{CustomName:\"x\"}").is_err());
    }
}
