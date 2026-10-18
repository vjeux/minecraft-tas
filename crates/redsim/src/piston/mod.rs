//! Pistons, piston heads and moving blocks (`PistonBaseBlock`, `PistonHeadBlock`,
//! `MovingPistonBlock` + `PistonMovingBlockEntity`, `PistonStructureResolver`).
//!
//! OWNED BY AGENT PISTMODEL: this file is redmodel's stub with the entry points the dispatcher
//! (`behavior.rs`) and the world (`world.rs`) call; pistmodel restructures freely inside
//! `piston/`. Until it lands, pistons are inert blocks (no moves).

use crate::behavior::Support;
use crate::block::{Block, Kind, PushReaction};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// Piston block entities (moving blocks) and their tickers.
#[derive(Default, Clone)]
pub struct State {}

pub fn is_idle(_w: &World) -> bool {
    true
}

/// `PistonBaseBlock.neighborChanged` / `PistonHeadBlock.neighborChanged`.
pub fn neighbor_changed(_w: &mut World, _state: Block, _pos: BlockPos, _src: Kind, _moved: bool) {}

/// `onPlace` of the piston blocks.
pub fn on_place(_w: &mut World, _state: Block, _pos: BlockPos, _old: Block, _moved: bool) {}

/// `updateShape` of the piston blocks (heads survive only in front of their base...).
pub fn update_shape(_w: &mut World, state: Block, _pos: BlockPos, _dir: Dir, _np: BlockPos, _ns: Block) -> Block {
    state
}

pub fn can_survive(_w: &World, _state: Block, _pos: BlockPos) -> bool {
    true
}

pub fn affect_neighbors_after_removal(_w: &mut World, _state: Block, _pos: BlockPos, _moved: bool) {}

/// `PistonBaseBlock.triggerEvent` (block events 0 extend, 1 retract-and-pull, 2 retract-and-drop).
pub fn trigger_event(_w: &mut World, _state: Block, _pos: BlockPos, _a: i32, _b: i32) -> bool {
    false
}

/// Scheduled ticks of the piston blocks (none in vanilla).
pub fn tick(_w: &mut World, _state: Block, _pos: BlockPos) {}

/// `PistonMovingBlockEntity.preRemoveSideEffects` (a moving block whose cell is overwritten is
/// finalTick'd).
pub fn pre_remove_side_effects(_w: &mut World, _pos: BlockPos, _old: Block) {}

pub fn remove_block_entity(_w: &mut World, _pos: BlockPos) {}

/// The block-entity phase (`Level.tickBlockEntities`).
pub fn tick_block_entities(_w: &mut World) {}

/// `isRedstoneConductor` of the piston blocks: never.
pub fn is_conductor(_state: &Block) -> bool {
    false
}

/// `isFaceSturdy` of the piston blocks.
pub fn sturdy(state: &Block, face: Dir, _support: Support) -> bool {
    match *state {
        Block::Piston { extended: false, .. } => true,
        // The extended base is a 12 px box: only its back face is full.
        Block::Piston { facing, extended: true, .. } => face == facing.opposite(),
        // The head's 4 px plate fills its front face.
        Block::PistonHead { facing, .. } => face == facing,
        _ => false,
    }
}

pub fn push_reaction(state: &Block) -> PushReaction {
    match *state {
        Block::Piston { extended: true, .. } => PushReaction::Immoveable,
        Block::Piston { .. } => PushReaction::PushPull,
        _ => PushReaction::Immoveable,
    }
}
