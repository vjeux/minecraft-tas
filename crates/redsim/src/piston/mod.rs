//! Pistons, piston heads and moving blocks (`PistonBaseBlock`, `PistonHeadBlock`,
//! `MovingPistonBlock` + `PistonMovingBlockEntity`, `PistonStructureResolver`), read in the 26.3
//! source: designs/piston-door-3x3/tracks/pistmodel/INVENTORY.md has every rule with its line.
//!
//! OWNED BY AGENT PISTMODEL. The dispatcher (`behavior.rs`) and the world (`world.rs`) call the
//! hooks below; the moving-piston block entities and their ticker list live in `World::piston`
//! ([`State`], `moving.rs`).

pub mod base;
pub mod head;
pub mod moving;
pub mod resolver;

pub use moving::{MovingBe, State};

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind, PushReaction};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// Block event numbers (`PistonBaseBlock.TRIGGER_*`).
pub const TRIGGER_EXTEND: i32 = 0;
pub const TRIGGER_CONTRACT: i32 = 1;
pub const TRIGGER_DROP: i32 = 2;

/// `pos.relative(direction, n)`.
pub fn rel_n(p: BlockPos, d: Dir, n: i32) -> BlockPos {
    let [dx, dy, dz] = d.step();
    p.offset(dx * n, dy * n, dz * n)
}

/// `Direction.get3DDataValue()`.
pub fn dir_index(d: Dir) -> i32 {
    d as u8 as i32
}

/// `Direction.from3DDataValue(i)` (`BY_3D_DATA[abs(i % 6)]`).
pub fn dir_from_index(i: i32) -> Dir {
    Dir::ALL[(i % 6).unsigned_abs() as usize]
}

pub fn is_piston(b: Block) -> bool {
    matches!(b, Block::Piston { .. })
}

/// `PistonBaseBlock.isPushable` (no world border or build-height limits: a door never reaches
/// them).
pub fn is_pushable(state: Block, direction: Dir, allow_destroyable: bool, connection: Dir) -> bool {
    if state.is_air() {
        return true;
    }
    if let Block::Piston { extended, .. } = state {
        if extended {
            return false;
        }
    } else {
        if bh::unbreakable(&state) {
            return false;
        }
        match bh::push_reaction(&state) {
            PushReaction::Immoveable => return false,
            PushReaction::Popped => return allow_destroyable,
            PushReaction::Push => return direction == connection,
            PushReaction::PushPull | PushReaction::IgnoreEntity => {}
        }
    }
    !bh::has_block_entity(&state)
}

// ---- hooks called by the dispatcher and the world ----

pub fn is_idle(w: &World) -> bool {
    w.piston.live_count() == 0
}

/// `PistonBaseBlock.neighborChanged` / `PistonHeadBlock.neighborChanged`.
pub fn neighbor_changed(w: &mut World, state: Block, pos: BlockPos, src: Kind, _moved: bool) {
    match state {
        Block::Piston { .. } => base::check_if_extend(w, pos, state),
        Block::PistonHead { .. } => head::neighbor_changed(w, state, pos, src),
        _ => {}
    }
}

/// `PistonBaseBlock.onPlace`: a new piston (not a state change of one) with no block entity in
/// its cell checks its power (a landing piston does).
pub fn on_place(w: &mut World, state: Block, pos: BlockPos, old: Block, _moved: bool) {
    if let Block::Piston { .. } = state {
        if old.kind() != state.kind() && w.piston.be(pos).is_none() {
            base::check_if_extend(w, pos, state);
        }
    }
}

/// `PistonHeadBlock.updateShape`: a head whose base side changed and that cannot survive is
/// air.
pub fn update_shape(w: &mut World, state: Block, pos: BlockPos, dir: Dir, _np: BlockPos, _ns: Block) -> Block {
    match state {
        Block::PistonHead { facing, .. } if dir.opposite() == facing && !head::can_survive(w, state, pos) => Block::Air,
        _ => state,
    }
}

pub fn can_survive(w: &World, state: Block, pos: BlockPos) -> bool {
    match state {
        Block::PistonHead { .. } => head::can_survive(w, state, pos),
        _ => true,
    }
}

pub fn affect_neighbors_after_removal(w: &mut World, state: Block, pos: BlockPos, _moved: bool) {
    if let Block::PistonHead { .. } = state {
        head::affect_neighbors_after_removal(w, state, pos);
    }
}

/// `PistonBaseBlock.triggerEvent` (block events 0 extend, 1 retract-and-pull, 2 retract-and-drop).
pub fn trigger_event(w: &mut World, state: Block, pos: BlockPos, a: i32, b: i32) -> bool {
    match state {
        Block::Piston { .. } => base::trigger_event(w, state, pos, a, b),
        _ => false,
    }
}

/// Scheduled ticks of the piston blocks (none in vanilla).
pub fn tick(_w: &mut World, _state: Block, _pos: BlockPos) {}

/// `PistonMovingBlockEntity.preRemoveSideEffects`: a moving block whose cell is overwritten is
/// finalTick'd.
pub fn pre_remove_side_effects(w: &mut World, pos: BlockPos, _old: Block) {
    moving::final_tick(w, pos);
}

/// `LevelChunk.removeBlockEntity`.
pub fn remove_block_entity(w: &mut World, pos: BlockPos) {
    w.piston.remove(pos);
}

/// The block-entity phase (`Level.tickBlockEntities`).
pub fn tick_block_entities(w: &mut World) {
    moving::tick_all(w);
}

/// `isRedstoneConductor` of the piston blocks: never.
pub fn is_conductor(_state: &Block) -> bool {
    false
}

/// `isFaceSturdy` of the piston blocks (support shapes = collision shapes): a retracted base is a
/// full cube; an extended base is a 12 px box (back face full, the 4 sides cover the center
/// column a torch needs, the front is empty); a head is a 4 px plate (front face full) on a 4x4
/// arm (too thin for the center column); a moving piston has no sturdy face here.
pub fn sturdy(state: &Block, face: Dir, support: Support) -> bool {
    match *state {
        Block::Piston { extended: false, .. } => true,
        Block::Piston { facing, extended: true, .. } => {
            face == facing.opposite() || (support == Support::Center && face != facing && face != facing.opposite())
        }
        Block::PistonHead { facing, .. } => face == facing,
        _ => false,
    }
}

/// The push reaction of the piston blocks (`pistonProperties()`: IMMOVEABLE; `isPushable`
/// special-cases retracted bases before reading it).
pub fn push_reaction(_state: &Block) -> PushReaction {
    PushReaction::Immoveable
}
