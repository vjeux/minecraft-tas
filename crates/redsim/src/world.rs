//! The level: block storage, `Level.setBlock` with the game's update flags, the
//! `CollectingNeighborUpdater` (every neighbor and shape update goes through it, depth first in
//! layers), the signal queries (`SignalGetter`), scheduled ticks and block events.

use crate::behavior as bh;
use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::{LevelTicks, Priority};
use std::cell::Cell;
use std::collections::{HashMap, HashSet, VecDeque};

/// `Block.UPDATE_*` flags.
pub mod flags {
    pub const NEIGHBORS: u32 = 1;
    pub const CLIENTS: u32 = 2;
    pub const INVISIBLE: u32 = 4;
    pub const IMMEDIATE: u32 = 8;
    pub const KNOWN_SHAPE: u32 = 16;
    pub const SUPPRESS_DROPS: u32 = 32;
    pub const MOVE_BY_PISTON: u32 = 64;
    pub const SKIP_SHAPE_UPDATE_ON_WIRE: u32 = 128;
    pub const SKIP_BLOCK_ENTITY_SIDEEFFECTS: u32 = 256;
    pub const SKIP_ON_PLACE: u32 = 512;
    pub const ALL: u32 = 3;
    /// The bridge's "raw" placement (`Sandbox.RAW_FLAGS`).
    pub const RAW: u32 = CLIENTS | KNOWN_SHAPE | SUPPRESS_DROPS | SKIP_BLOCK_ENTITY_SIDEEFFECTS | SKIP_ON_PLACE;
}

/// `Block.UPDATE_LIMIT`.
pub const UPDATE_LIMIT: i32 = 512;
/// Game rule `maxChainedNeighborUpdates` default.
pub const MAX_CHAINED_NEIGHBOR_UPDATES: u32 = 1_000_000;

/// One pending entry of the neighbor updater (`CollectingNeighborUpdater.NeighborUpdates`).
#[derive(Clone, Copy, Debug)]
enum Nu {
    Full { state: Block, pos: BlockPos, source: Kind, moved: bool },
    Multi { pos: BlockPos, source: Kind, skip: Option<Dir>, idx: u8 },
    Shape { dir: Dir, neighbor_state: Block, pos: BlockPos, neighbor_pos: BlockPos, flags: u32, limit: i32 },
    Simple { pos: BlockPos, source: Kind },
}

/// `BlockEventData`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct BlockEvent {
    pub pos: BlockPos,
    pub kind: Kind,
    pub a: i32,
    pub b: i32,
}

/// The block-event queue: an insertion-ordered set (`ObjectLinkedOpenHashSet`), identical
/// pending events are dropped.
#[derive(Default, Clone)]
pub struct BlockEvents {
    queue: VecDeque<BlockEvent>,
    set: HashSet<BlockEvent>,
}
impl BlockEvents {
    pub fn add(&mut self, e: BlockEvent) {
        if self.set.insert(e) {
            self.queue.push_back(e);
        }
    }
    pub fn pop(&mut self) -> Option<BlockEvent> {
        let e = self.queue.pop_front()?;
        self.set.remove(&e);
        Some(e)
    }
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = &BlockEvent> {
        self.queue.iter()
    }
}

/// The simulated level: a box of cells (outside: air, writes dropped and reported).
#[derive(Clone)]
pub struct World {
    pub min: BlockPos,
    pub size: [i32; 3],
    cells: Vec<Block>,
    /// Game time (`getGameTime`), incremented at the start of each tick.
    pub time: i64,
    sub_tick: i64,
    pub ticks: LevelTicks,
    pub events: BlockEvents,
    nu_stack: Vec<Nu>,
    nu_added: Vec<Nu>,
    nu_count: u32,
    /// `ServerLevel.handlingTick`: true from the tick's start through the block-event phase.
    pub handling_tick: bool,
    /// Comparator block entities: their output signal.
    pub comparator_out: HashMap<BlockPos, u8>,
    /// `RedstoneTorchBlock.RECENT_TOGGLES` of this level.
    pub torch_toggles: Vec<(BlockPos, i64)>,
    /// `RedstoneWireBlock.shouldSignal`.
    pub wire_signal: Cell<bool>,
    /// Piston block entities (moving blocks) and their tickers (agent pistmodel's side).
    pub piston: crate::piston::State,
    /// Falling-block entities (agent pistmodel's side).
    pub gravity: crate::gravity::State,
    /// Blocks destroyed with drops (an item entity in the game): (where, what).
    pub drops: Vec<(BlockPos, Block)>,
    /// Game events (vibrations), recorded only: (name, where).
    pub game_events: Vec<(&'static str, BlockPos)>,
    /// Problems the model met (a write outside the box, an unmodeled case): a run with any is
    /// not trustworthy.
    pub errors: Vec<String>,
}

impl World {
    /// An empty world covering `min..min+size` (exclusive).
    pub fn new(min: BlockPos, size: [i32; 3]) -> World {
        let n = (size[0] * size[1] * size[2]) as usize;
        World {
            min,
            size,
            cells: vec![Block::Air; n],
            time: 0,
            sub_tick: 0,
            ticks: LevelTicks::default(),
            events: BlockEvents::default(),
            nu_stack: Vec::new(),
            nu_added: Vec::new(),
            nu_count: 0,
            handling_tick: false,
            comparator_out: HashMap::new(),
            torch_toggles: Vec::new(),
            wire_signal: Cell::new(true),
            piston: Default::default(),
            gravity: Default::default(),
            drops: Vec::new(),
            game_events: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn index(&self, p: BlockPos) -> Option<usize> {
        let x = p.x - self.min.x;
        let y = p.y - self.min.y;
        let z = p.z - self.min.z;
        if x < 0 || y < 0 || z < 0 || x >= self.size[0] || y >= self.size[1] || z >= self.size[2] {
            return None;
        }
        Some(((y * self.size[2] + z) * self.size[0] + x) as usize)
    }

    pub fn contains(&self, p: BlockPos) -> bool {
        self.index(p).is_some()
    }

    pub fn get(&self, p: BlockPos) -> Block {
        match self.index(p) {
            Some(i) => self.cells[i],
            None => Block::Air,
        }
    }

    /// Write a state with no side effect at all (the chunk section write).
    pub fn put(&mut self, p: BlockPos, s: Block) {
        match self.index(p) {
            Some(i) => self.cells[i] = s,
            None => {
                if !s.is_air() {
                    self.errors.push(format!("write outside the box at {p}: {s}"));
                }
            }
        }
    }

    /// Every non-air cell.
    pub fn blocks(&self) -> Vec<(BlockPos, Block)> {
        let mut v = Vec::new();
        for y in 0..self.size[1] {
            for z in 0..self.size[2] {
                for x in 0..self.size[0] {
                    let p = self.min.offset(x, y, z);
                    let s = self.get(p);
                    if !s.is_air() {
                        v.push((p, s));
                    }
                }
            }
        }
        v
    }

    // ------------------------------------------------------------------ setBlock

    pub fn set_block(&mut self, pos: BlockPos, state: Block, fl: u32) -> bool {
        self.set_block_limit(pos, state, fl, UPDATE_LIMIT)
    }

    /// `Level.setBlock` (+ `LevelChunk.setBlockState`).
    pub fn set_block_limit(&mut self, pos: BlockPos, state: Block, fl: u32, limit: i32) -> bool {
        if !self.contains(pos) {
            if !state.is_air() {
                self.errors.push(format!("setBlock outside the box at {pos}: {state}"));
            }
            return false;
        }
        let old = self.get(pos);
        if old == state {
            return false;
        }
        self.put(pos, state);
        let new_kind = state.kind();
        let block_changed = old.kind() != new_kind;
        let moved = fl & flags::MOVE_BY_PISTON != 0;
        let side_effects = fl & flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS == 0;
        if block_changed && bh::has_block_entity(&old) {
            if side_effects {
                crate::piston::pre_remove_side_effects(self, pos, old);
            }
            self.remove_block_entity(pos, old);
        }
        if (block_changed || matches!(state, Block::Rail { .. })) && (fl & flags::NEIGHBORS != 0 || moved) {
            bh::affect_neighbors_after_removal(self, old, pos, moved);
        }
        if self.get(pos).kind() != new_kind {
            return false;
        }
        if fl & flags::SKIP_ON_PLACE == 0 {
            bh::on_place(self, state, pos, old, moved);
        }
        if bh::has_block_entity(&state) && self.get(pos).kind() == new_kind {
            self.ensure_block_entity(pos, state);
        }
        // Level.setBlock after the chunk write
        if self.get(pos) == state {
            if fl & flags::NEIGHBORS != 0 {
                self.update_neighbors_at(pos, old.kind());
                if bh::has_analog_output(&state) {
                    self.update_neighbour_for_output_signal(pos, new_kind);
                }
            }
            if fl & flags::KNOWN_SHAPE == 0 && limit > 0 {
                let nf = fl & !(flags::NEIGHBORS | flags::SUPPRESS_DROPS);
                bh::update_indirect_neighbour_shapes(self, old, pos, nf, limit - 1);
                self.update_neighbour_shapes(state, pos, nf, limit - 1);
                bh::update_indirect_neighbour_shapes(self, state, pos, nf, limit - 1);
            }
        }
        true
    }

    fn remove_block_entity(&mut self, pos: BlockPos, old: Block) {
        match old.kind() {
            Kind::Comparator => {
                self.comparator_out.remove(&pos);
            }
            Kind::MovingPiston => crate::piston::remove_block_entity(self, pos),
            _ => {}
        }
    }

    fn ensure_block_entity(&mut self, pos: BlockPos, state: Block) {
        if let Kind::Comparator = state.kind() {
            self.comparator_out.entry(pos).or_insert(0);
        }
    }

    /// `Level.setBlockAndUpdate` (flags 3).
    pub fn set_block_and_update(&mut self, pos: BlockPos, state: Block) -> bool {
        self.set_block(pos, state, flags::ALL)
    }

    /// `Level.removeBlock` (no fluids: air).
    pub fn remove_block(&mut self, pos: BlockPos, moved: bool) -> bool {
        self.set_block(pos, Block::Air, flags::ALL | if moved { flags::MOVE_BY_PISTON } else { 0 })
    }

    /// `Level.destroyBlock`: the drop (an item entity) is recorded in `drops`.
    pub fn destroy_block(&mut self, pos: BlockPos, drop: bool, limit: i32) -> bool {
        let s = self.get(pos);
        if s.is_air() {
            return false;
        }
        if drop && bh::drops_item(&s) {
            self.drops.push((pos, s));
        }
        self.set_block_limit(pos, Block::Air, flags::ALL, limit)
    }

    /// `BlockStateBase.updateNeighbourShapes`.
    pub fn update_neighbour_shapes(&mut self, state: Block, pos: BlockPos, fl: u32, limit: i32) {
        for d in Dir::SHAPE_ORDER {
            self.neighbor_shape_changed(d.opposite(), pos.rel(d), pos, state, fl, limit);
        }
    }

    /// `Level.updateNeighbourForOutputSignal`: comparators next to the block, or behind a
    /// conductor next to it.
    pub fn update_neighbour_for_output_signal(&mut self, pos: BlockPos, source: Kind) {
        for d in Dir::HORIZONTAL {
            let mut p = pos.rel(d);
            let mut s = self.get(p);
            if s.kind() == Kind::Comparator {
                self.neighbor_changed_full(s, p, source, false);
            } else if bh::is_conductor(self, &s, p) {
                p = p.rel(d);
                s = self.get(p);
                if s.kind() == Kind::Comparator {
                    self.neighbor_changed_full(s, p, source, false);
                }
            }
        }
    }

    // ------------------------------------------------------------------ neighbor updater

    pub fn update_neighbors_at(&mut self, pos: BlockPos, source: Kind) {
        self.add_and_run(Nu::Multi { pos, source, skip: None, idx: if Dir::UPDATE_ORDER[0] == Dir::West { 0 } else { 0 } });
    }

    pub fn update_neighbors_except(&mut self, pos: BlockPos, source: Kind, skip: Dir) {
        let idx = if Dir::UPDATE_ORDER[0] == skip { 1 } else { 0 };
        self.add_and_run(Nu::Multi { pos, source, skip: Some(skip), idx });
    }

    pub fn neighbor_changed(&mut self, pos: BlockPos, source: Kind) {
        self.add_and_run(Nu::Simple { pos, source });
    }

    pub fn neighbor_changed_full(&mut self, state: Block, pos: BlockPos, source: Kind, moved: bool) {
        self.add_and_run(Nu::Full { state, pos, source, moved });
    }

    pub fn neighbor_shape_changed(&mut self, dir: Dir, pos: BlockPos, neighbor_pos: BlockPos, neighbor_state: Block, fl: u32, limit: i32) {
        self.add_and_run(Nu::Shape { dir, neighbor_state, pos, neighbor_pos, flags: fl, limit });
    }

    fn add_and_run(&mut self, u: Nu) {
        let running = self.nu_count > 0;
        let too_many = self.nu_count >= MAX_CHAINED_NEIGHBOR_UPDATES;
        self.nu_count += 1;
        if !too_many {
            if running {
                self.nu_added.push(u);
            } else {
                self.nu_stack.push(u);
            }
        } else if self.nu_count - 1 == MAX_CHAINED_NEIGHBOR_UPDATES {
            self.errors.push("too many chained neighbor updates".into());
        }
        if !running {
            self.run_updates();
        }
    }

    fn run_updates(&mut self) {
        while !self.nu_stack.is_empty() || !self.nu_added.is_empty() {
            while let Some(u) = self.nu_added.pop() {
                // pushed in reverse order: the first added ends on top
                self.nu_stack.push(u);
            }
            // (nu_added was drained from its end, i.e. last added pushed first: the first added
            // is now on top, as the game's reverse loop does)
            let top = self.nu_stack.len() - 1;
            while self.nu_added.is_empty() {
                let mut u = self.nu_stack[top];
                let more = self.run_next(&mut u);
                self.nu_stack[top] = u;
                if !more {
                    self.nu_stack.pop();
                    break;
                }
            }
        }
        self.nu_stack.clear();
        self.nu_added.clear();
        self.nu_count = 0;
    }

    /// One step of an entry; false when it is finished.
    fn run_next(&mut self, u: &mut Nu) -> bool {
        match u {
            Nu::Full { state, pos, source, moved } => {
                bh::neighbor_changed(self, *state, *pos, *source, *moved);
                false
            }
            Nu::Simple { pos, source } => {
                let s = self.get(*pos);
                bh::neighbor_changed(self, s, *pos, *source, false);
                false
            }
            Nu::Shape { dir, neighbor_state, pos, neighbor_pos, flags: fl, limit } => {
                let current = self.get(*pos);
                if *fl & flags::SKIP_SHAPE_UPDATE_ON_WIRE == 0 || current.kind() != Kind::Wire {
                    let new = bh::update_shape(self, current, *pos, *dir, *neighbor_pos, *neighbor_state);
                    self.update_or_destroy(current, new, *pos, *fl, *limit);
                }
                false
            }
            Nu::Multi { pos, source, skip, idx } => {
                let d = Dir::UPDATE_ORDER[*idx as usize];
                *idx += 1;
                let np = pos.rel(d);
                let s = self.get(np);
                bh::neighbor_changed(self, s, np, *source, false);
                if (*idx as usize) < 6 && Some(Dir::UPDATE_ORDER[*idx as usize]) == *skip {
                    *idx += 1;
                }
                (*idx as usize) < 6
            }
        }
    }

    /// `Block.updateOrDestroy`.
    pub fn update_or_destroy(&mut self, old: Block, new: Block, pos: BlockPos, fl: u32, limit: i32) {
        if new != old {
            if new.is_air() {
                self.destroy_block(pos, fl & flags::SUPPRESS_DROPS == 0, limit);
            } else {
                self.set_block_limit(pos, new, fl & !flags::SUPPRESS_DROPS, limit);
            }
        }
    }

    // ------------------------------------------------------------------ signals (SignalGetter)

    /// `getSignal(pos, direction)`: the block at `pos` asked by a neighbor `direction` away
    /// from it (the neighbor is at `pos.relative(direction.opposite())`).
    pub fn signal(&self, pos: BlockPos, dir: Dir) -> i32 {
        let s = self.get(pos);
        let sig = bh::get_signal(self, &s, pos, dir);
        if bh::is_conductor(self, &s, pos) { sig.max(self.direct_signal_to(pos)) } else { sig }
    }

    pub fn direct_signal(&self, pos: BlockPos, dir: Dir) -> i32 {
        let s = self.get(pos);
        bh::get_direct_signal(self, &s, pos, dir)
    }

    /// `getDirectSignalTo`: the strong power a block receives.
    pub fn direct_signal_to(&self, pos: BlockPos) -> i32 {
        let mut r = 0;
        for d in [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East] {
            r = r.max(self.direct_signal(pos.rel(d), d));
            if r >= 15 {
                return r;
            }
        }
        r
    }

    pub fn has_signal(&self, pos: BlockPos, dir: Dir) -> bool {
        self.signal(pos, dir) > 0
    }

    pub fn has_neighbor_signal(&self, pos: BlockPos) -> bool {
        [Dir::Down, Dir::Up, Dir::North, Dir::South, Dir::West, Dir::East].into_iter().any(|d| self.signal(pos.rel(d), d) > 0)
    }

    pub fn best_neighbor_signal(&self, pos: BlockPos) -> i32 {
        let mut best = 0;
        for d in Dir::ALL {
            let s = self.signal(pos.rel(d), d);
            if s >= 15 {
                return 15;
            }
            best = best.max(s);
        }
        best
    }

    /// `getControlInputSignal` (diode side inputs).
    pub fn control_input_signal(&self, pos: BlockPos, dir: Dir, only_diodes: bool) -> i32 {
        let s = self.get(pos);
        if only_diodes {
            if bh::is_diode(&s) { self.direct_signal(pos, dir) } else { 0 }
        } else {
            match s {
                Block::RedstoneBlock => 15,
                Block::Wire { power, .. } => power as i32,
                _ => {
                    if bh::is_signal_source(self, &s) {
                        self.direct_signal(pos, dir)
                    } else {
                        0
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ scheduled ticks, events

    /// `scheduleTick(pos, block, delay, priority)` (ignored when one is pending for the same
    /// position and block).
    pub fn schedule_tick(&mut self, pos: BlockPos, kind: Kind, delay: i32, priority: Priority) {
        let sub = self.sub_tick;
        self.sub_tick += 1;
        self.ticks.schedule(kind, pos, self.time + delay as i64, priority, sub);
    }

    pub fn has_scheduled_tick(&self, pos: BlockPos, kind: Kind) -> bool {
        self.ticks.has_scheduled_tick(pos, kind)
    }

    pub fn will_tick_this_tick(&self, pos: BlockPos, kind: Kind) -> bool {
        self.ticks.will_tick_this_tick(pos, kind)
    }

    /// `Level.blockEvent`.
    pub fn block_event(&mut self, pos: BlockPos, kind: Kind, a: i32, b: i32) {
        self.events.add(BlockEvent { pos, kind, a, b });
    }

    // ------------------------------------------------------------------ the tick

    /// `ServerLevel.tick` for a world with no entities, fluids or random ticks.
    pub fn tick(&mut self) {
        self.handling_tick = true;
        self.time += 1;
        let due = self.ticks.collect(self.time);
        for t in due {
            self.ticks.start_running(&t);
            let s = self.get(t.pos);
            if s.kind() == t.kind {
                bh::tick(self, s, t.pos);
            }
        }
        self.ticks.end_tick();
        while let Some(e) = self.events.pop() {
            let s = self.get(e.pos);
            if s.kind() == e.kind {
                bh::trigger_event(self, s, e.pos, e.a, e.b);
            }
        }
        self.handling_tick = false;
        crate::gravity::tick_entities(self);
        crate::piston::tick_block_entities(self);
    }

    /// Nothing pending: no scheduled ticks, no block events, no moving blocks, no falling
    /// blocks (the bridge's `Sandbox.idle` for a design without entities).
    pub fn idle(&self) -> bool {
        self.ticks.count() == 0 && self.events.is_empty() && crate::piston::is_idle(self) && crate::gravity::is_idle(self)
    }

    /// `Level.gameEvent` (recorded only).
    pub fn game_event(&mut self, name: &'static str, pos: BlockPos) {
        self.game_events.push((name, pos));
    }

    /// Step until idle (at most `max` ticks); returns the ticks run.
    pub fn run_until_idle(&mut self, max: u32) -> u32 {
        let mut n = 0;
        while n < max && !self.idle() {
            self.tick();
            n += 1;
        }
        n
    }

    // ------------------------------------------------------------------ inputs

    /// Right-click a lever or a button (the bridge's `use`): `LeverBlock.pull` /
    /// `ButtonBlock.press`. Returns false for other blocks.
    pub fn use_block(&mut self, pos: BlockPos) -> bool {
        let s = self.get(pos);
        match s {
            Block::Lever { .. } => {
                crate::redstone::lever::pull(self, s, pos);
                true
            }
            Block::Button { .. } => {
                crate::redstone::button::press(self, s, pos);
                true
            }
            _ => false,
        }
    }

    /// The judge's placement (`piston-door/src/video.rs::place`): raw writes in this order, a
    /// shape update of every wire, a `self` update of every block (in this order); then the
    /// caller settles (`run_until_idle`).
    pub fn place_like_judge(&mut self, blocks: &[(BlockPos, Block)]) {
        for (p, s) in blocks {
            self.set_block(*p, *s, flags::RAW);
        }
        for (p, s) in blocks {
            if matches!(s, Block::Wire { .. }) {
                let cur = self.get(*p);
                let mut st = cur;
                for d in Dir::SHAPE_ORDER {
                    let np = p.rel(d);
                    let ns = self.get(np);
                    st = bh::update_shape(self, st, *p, d, np, ns);
                }
                if st != cur {
                    self.set_block(*p, st, flags::RAW);
                }
            }
        }
        for (p, _) in blocks {
            let k = self.get(*p).kind();
            self.neighbor_changed(*p, k);
        }
    }
}
