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
//! RANDOM slot (a dispenser whose filled slots all hold the same stack takes the first: every
//! pick gives the same world), a dropper or dispenser spitting an item into the world (item
//! entities), composters and crafters as targets. Shulker boxes and single chests (27 slots) are
//! the shulker study's (clk-rt, `shulker.rs`): a box item's contents ride on its stack's
//! `payload`.

use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::world::World;
use std::collections::{BTreeMap, HashMap};

/// Items with their max stack sizes (the fuzzer picks among these).
pub const ITEMS: [(&str, u8); 6] = [("stone", 64), ("redstone", 64), ("snowball", 16), ("ender_pearl", 16), ("wooden_shovel", 1), ("diamond_sword", 1)];
/// More items (ids from 6 on), for the entity clocks' contents and the crafters' recipes (clk-misc),
/// then the shulker study's (clk-rt; ids stay stable: append only).
pub const MORE_ITEMS: [(&str, u8); 18] = [
    ("book", 64),
    ("cobblestone", 64),
    ("slime_ball", 64),
    ("slime_block", 64),
    ("iron_ingot", 64),
    ("iron_block", 64),
    ("iron_nugget", 64),
    ("heavy_weighted_pressure_plate", 64),
    ("iron_bars", 64),
    ("bucket", 16),
    ("powder_snow_bucket", 1),
    ("music_disc_11", 1),
    ("music_disc_blocks", 1),
    ("music_disc_precipice", 1),
    ("music_disc_ward", 1),
    ("iron_shovel", 1),
    ("shulker_box", 1),
    ("paper", 64),
];

/// A disc's `JukeboxSong` (`data/minecraft/jukebox_song`): `lengthInTicks` = ceil(seconds * 20)
/// and its comparator output.
fn song_of(item: u8) -> Option<(i64, i32)> {
    let (seconds, comparator) = match item_name(item) {
        "music_disc_11" => (71.0f32, 11),
        "music_disc_blocks" => (345.0, 3),
        "music_disc_precipice" => (299.0, 13),
        "music_disc_ward" => (251.0, 10),
        _ => return None,
    };
    Some(((seconds * 20.0f32).ceil() as i64, comparator))
}

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

/// An item stack. `payload` is a shulker box item's contents (clk-rt): 0 = none, else 1 + an index
/// into `World::payloads` (27 slots). Hash and Debug leave a zero payload out, so every stack
/// without one hashes and prints as before the field existed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Stack {
    pub item: u8,
    pub count: u8,
    pub payload: u16,
}

impl Stack {
    /// A plain stack (no contents).
    pub const fn new(item: u8, count: u8) -> Stack {
        Stack { item, count, payload: 0 }
    }

    /// The same item (and contents) with another count.
    pub const fn with_count(self, count: u8) -> Stack {
        Stack { count, ..self }
    }
}

impl std::hash::Hash for Stack {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        self.item.hash(h);
        self.count.hash(h);
        if self.payload != 0 {
            self.payload.hash(h);
        }
    }
}

impl std::fmt::Debug for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("Stack");
        d.field("item", &self.item).field("count", &self.count);
        if self.payload != 0 {
            d.field("payload", &self.payload);
        }
        d.finish()
    }
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
    /// A jukebox (`JukeboxBlockEntity`): the disc whose song plays (`JukeboxSongPlayer.song`) and
    /// `ticksSinceSongStarted`.
    pub jukebox: bool,
    pub song: Option<u8>,
    pub song_ticks: i64,
}

impl ContainerBe {
    fn new(hopper: bool) -> ContainerBe {
        ContainerBe {
            slots: vec![None; if hopper { 5 } else { 9 }],
            cooldown: -1,
            ticked: 0,
            hopper,
            bookshelf: false,
            last_slot: -1,
            crafter: false,
            disabled: 0,
            crafting_ticks: 0,
            jukebox: false,
            song: None,
            song_ticks: 0,
        }
    }
    fn jukebox() -> ContainerBe {
        ContainerBe { slots: vec![None; 1], jukebox: true, ..ContainerBe::new(false) }
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
            if c.jukebox {
                (c.song, c.song_ticks).hash(h);
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
    matches!(s, Block::Dispenser { .. } | Block::Hopper { .. } | Block::ChiseledBookshelf { .. } | Block::Crafter { .. } | Block::Jukebox { .. })
}

/// The block entity appears (`LevelChunk.setBlockState` creating it). Hoppers and crafters tick
/// (`blockEntityTickers`, in creation order); a jukebox ticks while it holds a record
/// ([`jukebox_update_ticker`]).
pub fn create(w: &mut World, pos: BlockPos, s: &Block) {
    if w.containers.map.contains_key(&pos) {
        return;
    }
    if let Block::ChiseledBookshelf { .. } = s {
        w.containers.map.insert(pos, ContainerBe::bookshelf());
        return;
    }
    if let Block::Jukebox { .. } = s {
        w.containers.map.insert(pos, ContainerBe::jukebox());
        return;
    }
    // ---- shulker study (clk-rt): 27 slots, no ticker ----
    if let Block::ShulkerBox { .. } | Block::Chest { .. } = s {
        w.containers.map.insert(pos, ContainerBe::entity(27));
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
    if c.jukebox {
        // JukeboxBlockEntity.getComparatorOutput: the disc's song's
        return c.slots[0].and_then(|s| song_of(s.item)).map(|(_, out)| out).unwrap_or(0);
    }
    signal_of(c)
}

/// `JukeboxBlock.ownSignal`: 15 while a song plays.
pub fn jukebox_signal(w: &World, pos: BlockPos) -> i32 {
    if w.containers.map.get(&pos).is_some_and(|c| c.song.is_some()) { 15 } else { 0 }
}

/// `LevelChunk.updateBlockEntityTicker` of a jukebox: a ticker only with a record; one added
/// again (a record put back) goes to the end of the tickers.
pub fn jukebox_update_ticker(w: &mut World, pos: BlockPos, s: &Block) {
    match s {
        Block::Jukebox { has_record: true } => add_ticker(w, pos),
        Block::Jukebox { has_record: false } => remove_ticker(w, pos),
        _ => {}
    }
}

/// `JukeboxBlockEntity.onSongChanged`: neighbor updates and `setChanged`.
fn jukebox_song_changed(w: &mut World, pos: BlockPos) {
    w.update_neighbors_at(pos, Kind::Jukebox);
    set_changed(w, pos);
}

/// `JukeboxSongPlayer.stop`.
fn jukebox_stop(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get_mut(&pos) else { return };
    if c.song.is_some() {
        c.song = None;
        c.song_ticks = 0;
        let s = w.get(pos);
        w.game_event_state("jukebox_stop_play", pos, s);
        jukebox_song_changed(w, pos);
    }
}

/// `JukeboxBlockEntity.setTheItem`: `has_record` follows (flags 2, BLOCK_CHANGE), then the song
/// plays from the start (`play`) or stops.
fn jukebox_set_the_item(w: &mut World, pos: BlockPos, st: Option<Stack>) {
    if let Some(c) = w.containers.map.get_mut(&pos) {
        c.slots[0] = st;
    }
    let inserted = st.is_some();
    // notifyItemChangedInJukebox (the block entity's state is the world's)
    if let Block::Jukebox { .. } = w.get(pos) {
        let ns = Block::Jukebox { has_record: inserted };
        w.set_block(pos, ns, crate::world::flags::CLIENTS);
        w.game_event_state("block_change", pos, ns);
    }
    match st.filter(|s| song_of(s.item).is_some()) {
        Some(s) => {
            if let Some(c) = w.containers.map.get_mut(&pos) {
                c.song = Some(s.item);
                c.song_ticks = 0;
            }
            // levelEvent 1010 (the song), then onSongChanged
            jukebox_song_changed(w, pos);
        }
        None => jukebox_stop(w, pos),
    }
}

/// `JukeboxBlockEntity.tick` (`JukeboxSongPlayer.tick`): the song ends 20 gt after its length;
/// JUKEBOX_PLAY every 20 gt while it plays.
fn jukebox_tick(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get(&pos) else { return };
    let Some(disc) = c.song else { return };
    let ticks = c.song_ticks;
    let (length, _) = song_of(disc).unwrap_or((0, 0));
    if ticks >= length + 20 {
        jukebox_stop(w, pos);
    } else {
        if ticks % 20 == 0 {
            let s = w.get(pos);
            w.game_event_state("jukebox_play", pos, s);
            // spawnMusicParticles: the note's colour is `level.getRandom().nextInt(4)`
            if let Some(r) = w.rng() {
                r.next_int(4);
            }
        }
        if let Some(c) = w.containers.map.get_mut(&pos) {
            c.song_ticks += 1;
        }
    }
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
            if found.is_empty() {
                return None;
            }
            // getEntityContainer: entities.get(level.getRandom().nextInt(entities.size())) (a draw
            // even for one; the list in entity-section order: one section here, insertion order)
            if let Some(r) = w.rng() {
                let k = r.next_int(found.len() as i32) as usize;
                return Some(CRef::Entity(found[k]));
            }
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
pub(crate) fn set_item(w: &mut World, r: CRef, slot: usize, st: Option<Stack>) {
    if let CRef::Block(pos) = r
        && w.containers.map.get(&pos).is_some_and(|c| c.jukebox)
    {
        // ContainerSingleItem.setItem -> JukeboxBlockEntity.setTheItem
        if slot == 0 {
            jukebox_set_the_item(w, pos, st);
        }
        return;
    }
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
        && w.containers.map.get(&pos).is_some_and(|c| c.jukebox)
    {
        // JukeboxBlockEntity.splitTheItem: the whole item, then setTheItem(EMPTY)
        let got = w.containers.map[&pos].slots[0];
        if got.is_some() {
            jukebox_set_the_item(w, pos, None);
        }
        return got;
    }
    if let CRef::Block(pos) = r
        && w.containers.map.get(&pos).is_some_and(|c| c.bookshelf)
    {
        return bookshelf_remove(w, pos, slot);
    }
    let c = w.containers.get_mut(r)?;
    let s = c.slots[slot]?;
    c.slots[slot] = if s.count > 1 { Some(s.with_count(s.count - 1)) } else { None };
    if !c.hopper {
        changed(w, r);
    }
    Some(s.with_count(1))
}

/// The item taken out by [`remove_one`] did not fit: `itemStack.setCount(originalCount)` on the
/// stack still in the slot (a bookshelf's left the slot), then `setItem` when it was a single item.
fn restore(w: &mut World, r: CRef, slot: usize, st: Stack, original: u8) {
    // a bookshelf's or jukebox's item left the slot: only setItem puts it back
    let left = matches!(r, CRef::Block(p) if w.containers.map.get(&p).is_some_and(|c| c.bookshelf || c.jukebox));
    if !left && let Some(c) = w.containers.get_mut(r) {
        c.slots[slot] = Some(st.with_count(original));
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
    // ShulkerBoxBlockEntity.canPlaceItemThroughFace: never a box into a box (clk-rt)
    if let CRef::Block(p) = r
        && is_box_item(st.item)
        && matches!(w.get(p), Block::ShulkerBox { .. })
    {
        return false;
    }
    match w.containers.get(r) {
        Some(c) if c.bookshelf => is_book(st.item) && c.slots[slot].is_none(),
        // JukeboxBlockEntity.canPlaceItem: a playable disc into the empty jukebox
        Some(c) if c.jukebox => song_of(st.item).is_some() && c.slots[slot].is_none(),
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
    if w.containers.get(from).is_some_and(|c| c.jukebox) {
        // JukeboxBlockEntity.canTakeItem: the target has an empty slot
        return w.containers.get(into).is_some_and(|c| c.slots.iter().any(|t| t.is_none()));
    }
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
            Some(cur) if cur.item == st.item && cur.payload == st.payload && cur.count <= max_stack(cur.item) => {
                let space = max_stack(st.item).saturating_sub(cur.count);
                let n = st.count.min(space);
                st.count -= n;
                if let Some(c) = w.containers.get_mut(into) {
                    c.slots[slot] = Some(cur.with_count(cur.count + n));
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

/// `DispenserBlockEntity.getRandomSlot(level.getRandom())` with the level random mirrored: a
/// `nextInt(k)` for the k-th filled slot (from 1: a draw even for one), the slot kept when it is 0.
/// `None` without the mirror (callers keep their own rule).
fn random_slot(w: &mut World, filled: &[usize]) -> Option<usize> {
    let r = w.rng()?;
    let mut slot = None;
    for (k, &i) in filled.iter().enumerate() {
        if r.next_int(k as i32 + 1) == 0 {
            slot = Some(i);
        }
    }
    slot
}

/// `DropperBlock.dispenseFrom` (the 4 gt trigger tick of a dropper).
pub fn dropper_dispense(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get(&pos) else { return };
    let filled: Vec<usize> = c.slots.iter().enumerate().filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect();
    if filled.is_empty() {
        return; // levelEvent 1001 (click): no block change
    }
    let slot = match random_slot(w, &filled) {
        Some(s) => s,
        None => {
            if filled.len() > 1 {
                // getRandomSlot draws from the level random (not mirrored); when every filled slot
                // holds the same item (and contents), every pick moves the same item and leaves
                // the same total, so the world (blocks, comparators reading the fill level, the
                // receiving container) is the same whichever slot goes: take the first (coordinator
                // 2026-10-21: the item counters' droppers hold 2+ stacks of one item)
                let Some(c) = w.containers.map.get(&pos) else { return };
                let first = c.slots[filled[0]].unwrap();
                let same = filled.iter().all(|&i| {
                    let s = c.slots[i].unwrap();
                    s.item == first.item && same_payload(w, s.payload, first.payload)
                });
                if !same {
                    w.errors.push(format!("dropper at {pos} picks a random slot among {} stacks of different items (not modeled without the level random)", filled.len()));
                }
            }
            filled[0]
        }
    };
    let Some(c) = w.containers.map.get(&pos) else { return };
    let st = c.slots[slot].unwrap();
    let Block::Dispenser { facing, .. } = w.get(pos) else { return };
    let me = CRef::Block(pos);
    let remaining = match container_at(w, pos.rel(facing)) {
        None => {
            // the item flies out as an entity: harmless to blocks unless a hopper can catch it
            if w.containers.map.values().any(|c| c.hopper) {
                w.errors.push(format!("dropper at {pos} spits an item into the world (item entity, not modeled)"));
            }
            (st.count > 1).then_some(st.with_count(st.count - 1))
        }
        Some(into) => {
            if add_item(w, Some(me), into, st.with_count(1)).is_none() {
                (st.count > 1).then_some(st.with_count(st.count - 1))
            } else {
                Some(st)
            }
        }
    };
    set_item(w, me, slot, remaining);
}

/// A dispenser (not a dropper) firing (`DispenserBlock.dispenseFrom`): empty, a click and
/// BLOCK_ACTIVATE; a powder snow bucket places powder snow on the air in front (FLUID_PLACE) and
/// becomes a bucket; a bucket picks powder snow up (FLUID_PICKUP) and becomes a powder snow bucket.
/// Anything else (or a bucket that cannot act: it would fly out as an item entity) is noted.
pub fn dispenser_dispense(w: &mut World, pos: BlockPos) {
    let Some(c) = w.containers.map.get(&pos) else { return };
    let filled: Vec<usize> = c.slots.iter().enumerate().filter(|(_, s)| s.is_some()).map(|(i, _)| i).collect();
    if filled.is_empty() {
        // levelEvent 1001 and BLOCK_ACTIVATE
        let s = w.get(pos);
        w.game_event_state("block_activate", pos, s);
        return;
    }
    let slot = if let Some(s) = random_slot(w, &filled) {
        s
    } else {
        let Some(c) = w.containers.map.get(&pos) else { return };
        if filled.len() > 1 {
            // getRandomSlot draws from the level random (not mirrored), but when every filled slot
            // holds the same stack (item, count and contents: a loader's empty boxes, an unloader's
            // full ones) every pick gives the same world: take the first (clk-rt)
            let first = c.slots[filled[0]].unwrap();
            let same = filled.iter().all(|&i| {
                let s = c.slots[i].unwrap();
                s.item == first.item && s.count == first.count && same_payload(w, s.payload, first.payload)
            });
            if !same {
                w.errors.push(format!("dispenser at {pos} picks a random slot among {} stacks (not modeled)", filled.len()));
                return;
            }
        }
        filled[0]
    };
    let Some(c) = w.containers.map.get(&pos) else { return };
    let st = c.slots[slot].unwrap();
    let Block::Dispenser { facing, .. } = w.get(pos) else { return };
    if is_box_item(st.item) {
        crate::redstone::shulker::dispense_box(w, pos, slot, st, facing);
        return;
    }
    let target = pos.rel(facing);
    let me = CRef::Block(pos);
    match item_name(st.item) {
        // the filled-bucket behavior: SolidBucketItem.emptyContents on air, then consumeWithRemainder
        "powder_snow_bucket" if w.get(target).is_air() => {
            w.set_block_and_update(target, Block::PowderSnow);
            w.game_event("fluid_place", target);
            set_item(w, me, slot, Some(Stack::new(item_id("bucket").unwrap(), 1)));
        }
        // the empty-bucket behavior: PowderSnowBlock.pickupBlock (setBlock air, flags 11)
        "bucket" if matches!(w.get(target), Block::PowderSnow) => {
            w.set_block(target, Block::Air, crate::world::flags::NEIGHBORS | crate::world::flags::CLIENTS | crate::world::flags::IMMEDIATE);
            w.game_event("fluid_pickup", target);
            let filled = Stack::new(item_id("powder_snow_bucket").unwrap(), 1);
            // consumeWithRemainder: the stack shrinks; an emptied stack takes the remainder
            let rest = if st.count > 1 { Some(st.with_count(st.count - 1)) } else { None };
            if rest.is_some() {
                w.errors.push(format!("dispenser at {pos}: a stack of buckets fills one (insertItem of the remainder not modeled)"));
                return;
            }
            set_item(w, me, slot, Some(filled));
        }
        name => w.errors.push(format!("dispenser at {pos} dispenses {name} (its behavior or the item entity it throws is not modeled)")),
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
            Block::Jukebox { .. } => jukebox_tick(w, p),
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
                if add_item(w, Some(me), into, Stack::new(item, 1)).is_some() {
                    break;
                }
                remaining -= 1;
            }
        } else {
            while remaining > 0 {
                let old = remaining;
                remaining = add_item(w, Some(me), into, Stack::new(item, remaining)).map(|s| s.count).unwrap_or(0);
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
        match add_item(w, None, hopper, Stack { item, count: s.count, payload: s.payload }) {
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
/// `ItemStack.MAP_CODEC`). Components need a world ([`snbt_items_full`]).
fn snbt_items(v: &Snbt) -> Result<Vec<(usize, Stack)>, String> {
    snbt_items_full(v)?
        .into_iter()
        .map(|e| if e.contents.is_empty() && e.name.is_none() { Ok((e.slot, e.stack)) } else { Err("item components are only modeled in a block entity".to_string()) })
        .collect()
}

/// An `Items` entry (clk-rt): its slot, its stack (no payload yet) and the components redsim
/// models: a shulker box's contents (`"minecraft:container":[{slot:N,item:{id,count}},..]`) and a
/// custom name (`"minecraft:custom_name"`, kept as its SNBT text).
struct ItemEntry {
    slot: usize,
    stack: Stack,
    contents: Vec<(usize, Stack)>,
    name: Option<String>,
}

fn snbt_items_full(v: &Snbt) -> Result<Vec<ItemEntry>, String> {
    let Snbt::List(list) = v else { return Err("Items is not a list".into()) };
    let mut out = Vec::new();
    for e in list {
        let Snbt::Compound(fields) = e else { return Err("an Items entry is not a compound".into()) };
        let mut slot = None;
        let mut rest: Vec<(String, Snbt)> = Vec::new();
        for (k, v) in fields {
            match (k.as_str(), v) {
                ("Slot", Snbt::Int(n)) => slot = Some(*n),
                _ => rest.push((k.clone(), v.clone())),
            }
        }
        let slot = slot.ok_or("an Items entry has no Slot")?;
        let mut entry = snbt_stack(&rest)?;
        if !(0..=255).contains(&slot) {
            return Err(format!("bad Items entry: slot {slot}, {} {}", entry.stack.count, item_name(entry.stack.item)));
        }
        entry.slot = slot as usize;
        out.push(entry);
    }
    Ok(out)
}

/// An item compound's fields: `id`, `count` (default 1) and `components` (a shulker box's
/// `minecraft:container`, a `minecraft:custom_name`): its stack and components (slot 0).
fn snbt_stack(fields: &[(String, Snbt)]) -> Result<ItemEntry, String> {
    let (mut id, mut count, mut contents, mut label) = (None, 1i64, Vec::new(), None);
    for (k, v) in fields {
        match (k.as_str(), v) {
            ("id", Snbt::Str(s) | Snbt::Word(s)) => id = Some(s.clone()),
            ("count", Snbt::Int(n)) => count = *n,
            ("components", Snbt::Compound(comps)) => {
                for (ck, cv) in comps {
                    match (ck.as_str(), cv) {
                        ("minecraft:custom_name", v) => label = Some(format!("{v:?}")),
                        ("minecraft:container", Snbt::List(entries)) => {
                            for x in entries {
                                let Snbt::Compound(xf) = x else { return Err("a container entry is not a compound".into()) };
                                let (mut s, mut item) = (None, None);
                                for (xk, xv) in xf {
                                    match (xk.as_str(), xv) {
                                        ("slot", Snbt::Int(n)) => s = Some(*n),
                                        ("item", Snbt::Compound(i)) => item = Some(i.clone()),
                                        (o, _) => return Err(format!("container entry field {o} is not modeled")),
                                    }
                                }
                                let inner = snbt_stack(&item.ok_or("a container entry has no item")?)?;
                                let s = s.ok_or("a container entry has no slot")?;
                                if !inner.contents.is_empty() || inner.name.is_some() || !(0..27).contains(&s) {
                                    return Err(format!("bad container entry: slot {s} (a box inside a box, a renamed item in a box: not modeled)"));
                                }
                                contents.push((s as usize, inner.stack));
                            }
                        }
                        (o, _) => return Err(format!("item component {o} is not modeled")),
                    }
                }
            }
            (other, _) => return Err(format!("item field {other} is not modeled")),
        }
    }
    let name = id.ok_or("an item has no id")?;
    let item = item_id(&name).ok_or_else(|| format!("item {name} is not modeled (redsim knows {})", all_items().collect::<Vec<_>>().join(", ")))?;
    if !(1..=max_stack(item) as i64).contains(&count) {
        return Err(format!("bad item: {count} {name}"));
    }
    if !contents.is_empty() && !is_box_item(item) {
        return Err(format!("{name} with contents is not modeled"));
    }
    if label.is_some() && is_box_item(item) {
        // its block entity would carry the name through a placement and a break: not modeled
        return Err(format!("a renamed {name} is not modeled"));
    }
    Ok(ItemEntry { slot: 0, stack: Stack::new(item, count as u8), contents, name: label })
}

/// A shulker box item (any colour).
pub fn is_box_item(id: u8) -> bool {
    item_name(id).ends_with("shulker_box")
}

/// An item's components redsim models (`World::payloads`, a `Stack::payload` > 0): a shulker
/// box's contents (27 slots) and a custom name (its SNBT, compared as text; a renamed item stacks
/// only with the same stack's own items here, not with another stack renamed alike).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Payload {
    pub slots: Vec<Option<Stack>>,
    pub name: Option<String>,
}

/// Register a shulker box item's contents in `World::payloads`: its payload (0 when empty).
pub fn new_payload(w: &mut World, contents: &[(usize, Stack)]) -> u16 {
    new_payload_named(w, contents, None)
}

/// [`new_payload`] with a custom name (0 when neither).
pub fn new_payload_named(w: &mut World, contents: &[(usize, Stack)], name: Option<String>) -> u16 {
    if contents.is_empty() && name.is_none() {
        return 0;
    }
    let mut slots = vec![None; 27];
    for (s, st) in contents {
        if *s < 27 {
            slots[*s] = Some(*st);
        }
    }
    w.payloads.push(Payload { slots, name });
    w.payloads.len() as u16
}

/// A payload's 27 slots (all empty for 0).
pub fn payload_slots(w: &World, payload: u16) -> Vec<Option<Stack>> {
    if payload == 0 { vec![None; 27] } else { w.payloads[payload as usize - 1].slots.clone() }
}

/// Two payloads with the same components (contents and name).
pub fn same_payload(w: &World, a: u16, b: u16) -> bool {
    let get = |p: u16| if p == 0 { Payload { slots: vec![None; 27], name: None } } else { w.payloads[p as usize - 1].clone() };
    a == b || get(a) == get(b)
}

/// A design's `nbt:` line on a placed container, exactly as `data merge block x y z <snbt>`
/// (`DataCommands.mergeData` + `BlockDataAccessor.setData`): `Items` replaces the contents (slots
/// beyond the container's size are dropped, as `loadAllItems` does), `TransferCooldown` sets a
/// hopper's cooldown; when something changed, `setChanged` notifies the comparators reading it.
/// Returns whether it changed. Other keys, other block entities and unknown items are errors
/// (the clock judge in the game decides those designs).
pub fn apply_block_entity(w: &mut World, pos: BlockPos, snbt: &str) -> Result<bool, String> {
    let Snbt::Compound(fields) = parse_snbt(snbt)? else { return Err(format!("nbt at {pos}: not a compound")) };
    // a comparator's block entity holds only its output signal (`ComparatorBlockEntity`'s
    // "OutputSignal"); `setChanged` notifies nobody, since a comparator has no analog output
    // (clk-rt: a design whose comparator starts at a container's level as placed)
    if matches!(w.get(pos), Block::Comparator { .. }) {
        let mut changed = false;
        for (k, v) in &fields {
            match (k.as_str(), v) {
                ("OutputSignal", Snbt::Int(n)) => {
                    let n = (*n).clamp(0, 15) as u8;
                    changed |= w.comparator_out.insert(pos, n) != Some(n);
                }
                _ => return Err(format!("nbt at {pos}: comparator key `{k}` is not modeled")),
            }
        }
        return Ok(changed);
    }
    let Some(c) = w.containers.map.get(&pos) else { return Err(format!("nbt at {pos}: {} has no modeled block entity", w.get(pos))) };
    let mut slots = c.slots.clone();
    let mut cooldown = c.cooldown;
    let mut last_slot = c.last_slot;
    let mut disabled = c.disabled;
    let mut crafting_ticks = c.crafting_ticks;
    let mut disabled_given = None;
    let mut song_ticks: Option<i64> = None;
    // shulker box items with contents (clk-rt): their payloads are registered once `c` is done
    let mut boxes: Vec<(usize, Vec<(usize, Stack)>, Option<String>)> = Vec::new();
    for (k, v) in &fields {
        match (k.as_str(), v) {
            ("Items", v) => {
                slots = vec![None; c.slots.len()];
                boxes.clear();
                for e in snbt_items_full(v).map_err(|e| format!("nbt at {pos}: {e}"))? {
                    if e.slot < slots.len() {
                        slots[e.slot] = Some(e.stack);
                        if !e.contents.is_empty() || e.name.is_some() {
                            boxes.push((e.slot, e.contents, e.name));
                        }
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
            // JukeboxBlockEntity.loadAdditional: the record, and the song's progress
            // (setSongWithoutPlaying: no notification)
            ("RecordItem", Snbt::Compound(f)) if c.jukebox => {
                let id = f.iter().find_map(|(k, v)| match (k.as_str(), v) {
                    ("id", Snbt::Str(s) | Snbt::Word(s)) => Some(s.clone()),
                    _ => None,
                });
                let name = id.ok_or_else(|| format!("nbt at {pos}: RecordItem without an id"))?;
                let item = item_id(&name).ok_or_else(|| format!("nbt at {pos}: item {name} is not modeled"))?;
                slots = vec![Some(Stack::new(item, 1))];
            }
            ("ticks_since_song_started", Snbt::Int(n)) if c.jukebox => song_ticks = Some(*n),
            (other, _) => return Err(format!("nbt at {pos}: key {other} is not modeled here")),
        }
    }
    if c.jukebox {
        if c.slots[0].is_some() && c.slots[0].map(|s| s.item) != slots[0].map(|s| s.item) {
            return Err(format!("nbt at {pos}: replacing a playing record is not modeled"));
        }
        if let (Some(t), Some(st)) = (song_ticks, slots[0])
            && let Some((length, _)) = song_of(st.item)
            && t < length + 20
        {
            let jc = w.containers.map.get_mut(&pos).unwrap();
            jc.song = Some(st.item);
            jc.song_ticks = t;
        }
    }
    for (slot, contents, name) in &boxes {
        let payload = new_payload_named(w, contents, name.clone());
        if let Some(s) = slots[*slot].as_mut() {
            s.payload = payload;
        }
    }
    let c = &w.containers.map[&pos];
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
    matches!(k, Kind::Dispenser(_) | Kind::Hopper | Kind::ChiseledBookshelf | Kind::Crafter | Kind::Jukebox | Kind::ShulkerBox(_) | Kind::Chest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::Block;

    /// A stack without a payload hashes and prints exactly as the struct did before `payload`
    /// (a derived `Hash` of `{ item, count }`: the fields in order): state keys and model notes
    /// of every design without a shulker box stay what they were. Keep the manual impls.
    #[test]
    fn stack_without_payload_hashes_as_before() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let s = Stack::new(3, 17);
        let mut a = DefaultHasher::new();
        s.hash(&mut a);
        let mut b = DefaultHasher::new();
        3u8.hash(&mut b);
        17u8.hash(&mut b);
        assert_eq!(a.finish(), b.finish());
        assert_eq!(format!("{s:?}"), "Stack { item: 3, count: 17 }");
        let p = Stack { payload: 2, ..s };
        let mut c = DefaultHasher::new();
        p.hash(&mut c);
        assert_ne!(c.finish(), b.finish());
        assert_eq!(format!("{p:?}"), "Stack { item: 3, count: 17, payload: 2 }");
    }

    #[test]
    fn snbt_items_parse() {
        let v = parse_snbt("{Items:[{Slot:0b,id:\"minecraft:stone\",count:3},{Slot:2b, id:'minecraft:snowball'}],TransferCooldown:-1}").unwrap();
        let Snbt::Compound(f) = &v else { panic!() };
        assert_eq!(f[1], ("TransferCooldown".to_string(), Snbt::Int(-1)));
        let items = snbt_items(&f[0].1).unwrap();
        assert_eq!(items, vec![(0, Stack::new(0, 3)), (2, Stack::new(2, 1))]);
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
        assert_eq!(c.slots[4], Some(Stack::new(5, 1)));
        assert_eq!(c.cooldown, 3);
        assert_eq!(analog_output(&w, hopper), 3); // 1/5 full: floor(14 * 0.2) + 1
        // the same data again changes nothing (data merge's ERROR_MERGE_UNCHANGED: no setChanged)
        assert!(!apply_block_entity(&mut w, hopper, "{Items:[{Slot:4b,id:\"minecraft:diamond_sword\",count:1}]}").unwrap());
        assert!(apply_block_entity(&mut w, BlockPos::new(1, 0, 0), "{Items:[]}").is_err());
        assert!(apply_block_entity(&mut w, hopper, "{CustomName:\"x\"}").is_err());
    }
}
