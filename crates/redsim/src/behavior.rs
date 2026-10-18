//! The dispatcher: every `Block`/`BlockBehaviour` method the engine calls, by block family, plus
//! the static properties (conductor, face sturdiness, signal source, analog output, push
//! reaction, block entity, note block instrument).

use crate::block::{Block, Face, Instrument, Kind, PushReaction, Simple};
use crate::pos::{BlockPos, Dir};
use crate::redstone::attach::{attached, button, torch};
use crate::redstone::blocks::{bulb, composter, dispenser, hopper, indicator, lamp, leaves, note, observer, rail, target};
use crate::redstone::{diode, wire};
use crate::world::World;

/// `SupportType`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Support {
    Full,
    Center,
    Rigid,
}

// ---------------------------------------------------------------------- static properties

/// `BlockState.isSolid()` (`calculateSolid`): forced on, or a collision box whose mean size is
/// at least 0.7291666 (a 3 px slab is exactly that) or a full block tall.
pub fn is_solid(s: &Block) -> bool {
    match *s {
        Block::Air
        | Block::Wire { .. }
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Rail { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Skull { .. }
        | Block::WallSkull { .. } => false,
        _ => true,
    }
}

/// `isRedstoneConductor`.
pub fn is_conductor(_w: &World, s: &Block, _p: BlockPos) -> bool {
    match *s {
        Block::Simple(x) => x.conductor(),
        Block::Glazed { .. } | Block::Lamp { .. } | Block::Note { .. } | Block::Target { .. } | Block::Dispenser { .. } | Block::Crafter { .. } => true,
        Block::Log { .. } | Block::RespawnAnchor { .. } => true,
        Block::Slab { kind } => kind == 2,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::is_conductor(s),
        _ => false,
    }
}

/// `isFaceSturdy(level, pos, face, support)`: the block's support shape covers that face.
pub fn sturdy(_w: &World, s: &Block, _p: BlockPos, face: Dir, support: Support) -> bool {
    match *s {
        Block::Simple(Simple::HoneyBlock) => face == Dir::Down && support == Support::Center,
        Block::Simple(x) => x.full(),
        Block::Glazed { .. }
        | Block::RedstoneBlock
        | Block::Observer { .. }
        | Block::Lamp { .. }
        | Block::Bulb { .. }
        | Block::Note { .. }
        | Block::Target { .. }
        | Block::Dispenser { .. }
        | Block::Crafter { .. }
        | Block::RespawnAnchor { .. }
        | Block::Log { .. } => true,
        Block::Repeater { .. } | Block::Comparator { .. } => face == Dir::Down,
        Block::Trapdoor { facing, top, open, .. } => {
            let full = if open {
                facing.opposite()
            } else if top {
                Dir::Up
            } else {
                Dir::Down
            };
            face == full
        }
        Block::Door { facing, hinge_right, open, .. } => {
            let d = if open {
                if hinge_right { facing.counter_clockwise() } else { facing.clockwise() }
            } else {
                facing
            };
            face == d.opposite()
        }
        Block::FenceGate { open, .. } => !open && face == Dir::Up && support == Support::Center,
        Block::Hopper { facing, .. } => (face == Dir::Up && support == Support::Rigid) || (face == Dir::Down && facing == Dir::Down && support == Support::Center),
        Block::Slab { kind } => kind == 2 || (kind == 0 && face == Dir::Down) || (kind == 1 && face == Dir::Up),
        Block::Skull { .. } => face == Dir::Down && support == Support::Center,
        Block::WallSkull { facing, .. } => face == facing.opposite() && support == Support::Center,
        Block::Cake { bites } => face == Dir::Down && support == Support::Center && bites <= 2,
        Block::EndPortalFrame { eye, .. } => face == Dir::Down || (eye && face == Dir::Up && support == Support::Center),
        Block::Composter { .. } => face != Dir::Up || support == Support::Rigid,
        Block::Piston { .. } | Block::PistonHead { .. } => crate::piston::sturdy(s, face, support),
        _ => false,
    }
}

/// `isSignalSource`.
pub fn is_signal_source(w: &World, s: &Block) -> bool {
    match *s {
        Block::Wire { .. } => w.wire_signal.get(),
        Block::RedstoneBlock
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Observer { .. }
        | Block::Target { .. } => true,
        _ => false,
    }
}

pub fn is_diode(s: &Block) -> bool {
    matches!(s, Block::Repeater { .. } | Block::Comparator { .. })
}

/// `getSignal(state, level, pos, direction)`.
pub fn get_signal(w: &World, s: &Block, p: BlockPos, d: Dir) -> i32 {
    match *s {
        Block::Wire { .. } => wire::get_signal(w, s, p, d),
        Block::RedstoneBlock => 15,
        Block::Torch { .. } | Block::WallTorch { .. } => torch::get_signal(s, d),
        Block::Lever { .. } | Block::Button { .. } => attached::get_signal(s),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::get_signal(w, s, p, d),
        Block::Observer { .. } => observer::get_signal(s, d),
        Block::Target { power } => power as i32,
        _ => 0,
    }
}

/// `getDirectSignal` (strong power).
pub fn get_direct_signal(w: &World, s: &Block, p: BlockPos, d: Dir) -> i32 {
    match *s {
        Block::Wire { .. } => wire::get_direct_signal(w, s, p, d),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::get_direct_signal(s, d),
        Block::Lever { .. } | Block::Button { .. } => attached::get_direct_signal(s, d),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::get_signal(w, s, p, d),
        Block::Observer { .. } => observer::get_signal(s, d),
        _ => 0,
    }
}

/// `shouldRedstoneWireConnectTo(state, level, pos, direction)`.
pub fn should_wire_connect(w: &World, s: &Block, _p: BlockPos, d: Option<Dir>) -> bool {
    match *s {
        Block::Wire { .. } => true,
        Block::Repeater { facing, .. } => Some(facing) == d || Some(facing.opposite()) == d,
        Block::Observer { facing, .. } => Some(facing) == d,
        _ => is_signal_source(w, s) && d.is_some(),
    }
}

pub fn has_analog_output(s: &Block) -> bool {
    matches!(
        s,
        Block::Bulb { .. }
            | Block::Dispenser { .. }
            | Block::Crafter { .. }
            | Block::Hopper { .. }
            | Block::Cake { .. }
            | Block::EndPortalFrame { .. }
            | Block::Composter { .. }
            | Block::RespawnAnchor { .. }
    )
}

/// `getAnalogOutputSignal` (containers are empty: 0).
pub fn analog_output(w: &World, s: &Block, p: BlockPos, _d: Dir) -> i32 {
    match *s {
        Block::Bulb { .. } => match w.get(p) {
            Block::Bulb { lit: true, .. } => 15,
            _ => 0,
        },
        Block::Cake { bites } => (7 - bites as i32) * 2,
        Block::EndPortalFrame { eye, .. } => {
            if eye {
                15
            } else {
                0
            }
        }
        Block::Composter { level } => level as i32,
        Block::RespawnAnchor { charges } => (charges as f32 / 4.0 * 15.0).floor() as i32,
        _ => 0,
    }
}

/// `BlockState.instrument()`.
pub fn instrument(s: &Block) -> Instrument {
    match *s {
        Block::Simple(x) => x.instrument(),
        Block::Glazed { .. } | Block::Observer { .. } | Block::Dispenser { .. } | Block::Slab { .. } | Block::EndPortalFrame { .. } | Block::RespawnAnchor { .. } => {
            Instrument::Basedrum
        }
        Block::Note { .. } | Block::Log { .. } | Block::Composter { .. } => Instrument::Bass,
        Block::Trapdoor { iron: false, .. } | Block::Door { iron: false, .. } | Block::FenceGate { .. } => Instrument::Bass,
        // (the wall variant has fresh properties: harp)
        Block::Skull { .. } => Instrument::Skeleton,
        _ => Instrument::Harp,
    }
}

/// `getPistonPushReaction` (pistons themselves: see `piston::push_reaction`).
pub fn push_reaction(s: &Block) -> PushReaction {
    match *s {
        Block::Simple(x) => x.push_reaction(),
        Block::Glazed { .. } => PushReaction::Push,
        Block::Wire { .. }
        | Block::Torch { .. }
        | Block::WallTorch { .. }
        | Block::Lever { .. }
        | Block::Button { .. }
        | Block::Repeater { .. }
        | Block::Comparator { .. }
        | Block::Skull { .. }
        | Block::WallSkull { .. }
        | Block::Cake { .. }
        | Block::Leaves { .. }
        | Block::Door { .. } => PushReaction::Popped,
        Block::RespawnAnchor { .. } => PushReaction::Immoveable,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::push_reaction(s),
        _ => PushReaction::PushPull,
    }
}

/// Leaves a drop (an item entity) when destroyed with drops: false for blocks without a loot
/// table (`noLootTable`: piston heads, moving pistons, end portal frames) and air.
pub fn drops_item(s: &Block) -> bool {
    !matches!(s, Block::Air | Block::PistonHead { .. } | Block::MovingPiston { .. } | Block::EndPortalFrame { .. })
}

/// Destroy speed -1 (unbreakable: pistons cannot move it).
pub fn unbreakable(s: &Block) -> bool {
    matches!(s, Block::EndPortalFrame { .. } | Block::MovingPiston { .. })
}

/// `hasBlockEntity`.
pub fn has_block_entity(s: &Block) -> bool {
    matches!(
        s,
        Block::Comparator { .. } | Block::Dispenser { .. } | Block::Crafter { .. } | Block::Hopper { .. } | Block::Skull { .. } | Block::WallSkull { .. } | Block::MovingPiston { .. }
    )
}

/// `canBeReplaced` (by a moving block / a falling block): air only among the modeled blocks.
pub fn can_be_replaced(s: &Block) -> bool {
    s.is_air()
}

/// Destroying it drops an item (its loot table gives something without a tool).
pub fn drops_item(s: &Block) -> bool {
    !matches!(s, Block::Air | Block::PistonHead { .. } | Block::MovingPiston { .. })
}

// ---------------------------------------------------------------------- behavior

pub fn can_survive(w: &World, s: &Block, p: BlockPos) -> bool {
    match *s {
        Block::Wire { .. } => wire::can_survive(w, p),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::can_survive(w, p),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::can_survive(w, s, p),
        Block::Lever { .. } | Block::Button { .. } => attached::can_survive(w, s, p),
        Block::Rail { .. } => rail::can_survive(w, p),
        Block::Door { .. } => indicator::door_can_survive(w, s, p),
        Block::Cake { .. } => is_solid(&w.get(p.below())),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::can_survive(w, *s, p),
        _ => true,
    }
}

/// `neighborChanged`.
pub fn neighbor_changed(w: &mut World, s: Block, p: BlockPos, src: Kind, moved: bool) {
    match s {
        Block::Wire { .. } => wire::neighbor_changed(w, s, p, src, moved),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::neighbor_changed(w, s, p, src, moved),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::neighbor_changed(w, s, p),
        Block::Lamp { .. } => lamp::neighbor_changed(w, s, p),
        Block::Bulb { .. } => bulb::check_and_flip(w, s, p),
        Block::Note { .. } => note::neighbor_changed(w, s, p),
        Block::Trapdoor { .. } | Block::Door { .. } | Block::FenceGate { .. } | Block::Skull { .. } | Block::WallSkull { .. } => indicator::neighbor_changed(w, s, p, src),
        Block::Rail { .. } => rail::neighbor_changed(w, s, p, moved),
        Block::Dispenser { .. } | Block::Crafter { .. } => dispenser::neighbor_changed(w, s, p),
        Block::Hopper { .. } => hopper::check_powered_state(w, s, p),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::neighbor_changed(w, s, p, src, moved),
        _ => {}
    }
}

/// `updateShape` (may schedule ticks: observers, leaves).
pub fn update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    match s {
        Block::Wire { .. } => wire::update_shape(w, s, p, d, np, ns),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::update_shape(w, s, p, d, np, ns),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::update_shape(w, s, p, d),
        Block::Lever { .. } | Block::Button { .. } => attached::update_shape(w, s, p, d),
        Block::Observer { .. } => observer::update_shape(w, s, p, d),
        Block::Note { .. } => note::update_shape(w, s, p, d),
        Block::Door { .. } => indicator::door_update_shape(w, s, p, d, ns),
        Block::FenceGate { .. } => indicator::gate_update_shape(s, d),
        Block::Leaves { .. } => leaves::update_shape(w, s, p, ns),
        // CakeBlock: falls off a non-solid floor
        Block::Cake { .. } => {
            if d == Dir::Down && !is_solid(&w.get(p.below())) {
                Block::Air
            } else {
                s
            }
        }
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::update_shape(w, s, p, d, np, ns),
        _ => s,
    }
}

/// `Block.updateFromNeighbourShapes`.
pub fn update_from_neighbour_shapes(w: &mut World, s: Block, p: BlockPos) -> Block {
    let mut st = s;
    for d in Dir::SHAPE_ORDER {
        let np = p.rel(d);
        let ns = w.get(np);
        st = update_shape(w, st, p, d, np, ns);
    }
    st
}

/// `onPlace`.
pub fn on_place(w: &mut World, s: Block, p: BlockPos, old: Block, moved: bool) {
    match s {
        Block::Wire { .. } => wire::on_place(w, s, p, old, moved),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::on_place(w, s, p, old, moved),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::on_place(w, s, p),
        Block::Observer { .. } => observer::on_place(w, s, p, old),
        Block::Bulb { .. } => bulb::on_place(w, s, p, old),
        Block::Target { .. } => target::on_place(w, s, p, old),
        Block::Rail { .. } => rail::on_place(w, s, p, old, moved),
        Block::Hopper { .. } => hopper::on_place(w, s, p, old),
        Block::Composter { .. } => composter::on_place(w, s, p),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::on_place(w, s, p, old, moved),
        _ => {}
    }
}

/// `affectNeighborsAfterRemoval`.
pub fn affect_neighbors_after_removal(w: &mut World, s: Block, p: BlockPos, moved: bool) {
    match s {
        Block::Wire { .. } => wire::affect_neighbors_after_removal(w, s, p, moved),
        Block::Repeater { .. } | Block::Comparator { .. } => diode::affect_neighbors_after_removal(w, s, p, moved),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::affect_neighbors_after_removal(w, s, p, moved),
        Block::Lever { .. } | Block::Button { .. } => attached::affect_neighbors_after_removal(w, s, p, moved),
        Block::Observer { .. } => observer::affect_neighbors_after_removal(w, s, p, moved),
        Block::Rail { .. } => rail::affect_neighbors_after_removal(w, s, p, moved),
        Block::Dispenser { .. } | Block::Crafter { .. } | Block::Hopper { .. } => dispenser::affect_neighbors_after_removal(w, s, p),
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::affect_neighbors_after_removal(w, s, p, moved),
        _ => {}
    }
}

/// `updateIndirectNeighbourShapes` (only dust has one).
pub fn update_indirect_neighbour_shapes(w: &mut World, s: Block, p: BlockPos, fl: u32, limit: i32) {
    if let Block::Wire { .. } = s {
        wire::update_indirect_neighbour_shapes(w, s, p, fl, limit);
    }
}

/// A scheduled tick (`tick`).
pub fn tick(w: &mut World, s: Block, p: BlockPos) {
    match s {
        Block::Repeater { .. } | Block::Comparator { .. } => diode::tick(w, s, p),
        Block::Torch { .. } | Block::WallTorch { .. } => torch::tick(w, s, p),
        Block::Button { .. } => button::tick(w, s, p),
        Block::Observer { .. } => observer::tick(w, s, p),
        Block::Lamp { .. } => lamp::tick(w, s, p),
        Block::Target { .. } => target::tick(w, s, p),
        Block::Leaves { .. } => leaves::tick(w, s, p),
        Block::Composter { .. } => composter::tick(w, s, p),
        // Dispensers, droppers, crafters: dispensing from an empty inventory changes no block.
        Block::Dispenser { .. } | Block::Crafter { .. } => {}
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::tick(w, s, p),
        _ => {}
    }
}

/// A block event (`triggerEvent`).
pub fn trigger_event(w: &mut World, s: Block, p: BlockPos, a: i32, b: i32) -> bool {
    match s {
        // Note blocks: the sound only.
        Block::Note { .. } => true,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. } => crate::piston::trigger_event(w, s, p, a, b),
        _ => false,
    }
}

/// Does a lever/button at this state hang on `support`?
pub fn attached_to(s: &Block, p: BlockPos, support: BlockPos) -> bool {
    match *s {
        Block::Lever { face, facing, .. } | Block::Button { face, facing, .. } => p.rel(Face::connected(face, facing).opposite()) == support,
        _ => false,
    }
}
