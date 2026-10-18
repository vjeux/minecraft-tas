//! Observers, lamps, copper bulbs, note blocks, targets, power indicators (trapdoors, doors,
//! fence gates, skulls), rails, dispensers/droppers/crafters, hoppers, leaves, composters.

use crate::behavior::{self as bh};
use crate::block::{Block, Instrument, Kind, RailShape};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

pub mod observer {
    use super::*;

    fn facing(s: &Block) -> Dir {
        match *s {
            Block::Observer { facing, .. } => facing,
            _ => Dir::North,
        }
    }

    fn update_neighbors_in_front(w: &mut World, pos: BlockPos, s: &Block) {
        let d = facing(s);
        let op = pos.rel(d.opposite());
        w.neighbor_changed(op, Kind::Observer);
        w.update_neighbors_except(op, Kind::Observer, d);
    }

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Observer { facing, powered } = s {
            if powered {
                w.set_block(pos, Block::Observer { facing, powered: false }, flags::CLIENTS);
            } else {
                w.set_block(pos, Block::Observer { facing, powered: true }, flags::CLIENTS);
                w.schedule_tick(pos, Kind::Observer, 2, Priority::Normal);
            }
            update_neighbors_in_front(w, pos, &s);
        }
    }

    pub fn update_shape(w: &mut World, s: Block, pos: BlockPos, d: Dir) -> Block {
        if let Block::Observer { facing, powered: false } = s
            && facing == d
            && !w.has_scheduled_tick(pos, Kind::Observer)
        {
            w.schedule_tick(pos, Kind::Observer, 2, Priority::Normal);
        }
        s
    }

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos, old: Block) {
        if old.kind() != Kind::Observer
            && let Block::Observer { facing, powered: true } = s
            && !w.has_scheduled_tick(pos, Kind::Observer)
        {
            let ns = Block::Observer { facing, powered: false };
            w.set_block(pos, ns, flags::CLIENTS | flags::KNOWN_SHAPE);
            update_neighbors_in_front(w, pos, &ns);
        }
    }

    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos, _moved: bool) {
        if let Block::Observer { facing, powered: true } = s
            && w.has_scheduled_tick(pos, Kind::Observer)
        {
            update_neighbors_in_front(w, pos, &Block::Observer { facing, powered: false });
        }
    }

    pub fn get_signal(s: &Block, d: Dir) -> i32 {
        match *s {
            Block::Observer { facing, powered: true } if facing == d => 15,
            _ => 0,
        }
    }
}

pub mod lamp {
    use super::*;

    pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Lamp { lit } = s
            && lit != w.has_neighbor_signal(pos)
        {
            if lit {
                w.schedule_tick(pos, Kind::Lamp, 4, Priority::Normal);
            } else {
                w.set_block(pos, Block::Lamp { lit: true }, flags::CLIENTS);
            }
        }
    }

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Lamp { lit: true } = s
            && !w.has_neighbor_signal(pos)
        {
            w.set_block(pos, Block::Lamp { lit: false }, flags::CLIENTS);
        }
    }
}

pub mod bulb {
    use super::*;

    /// `CopperBulbBlock.checkAndFlip`.
    pub fn check_and_flip(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Bulb { oxid, lit, powered } = s {
            let signal = w.has_neighbor_signal(pos);
            if signal != powered {
                let lit = if !powered { !lit } else { lit };
                w.set_block_and_update(pos, Block::Bulb { oxid, lit, powered: signal });
            }
        }
    }

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos, old: Block) {
        if old.kind() != s.kind() {
            check_and_flip(w, s, pos);
        }
    }
}

pub mod note {
    use super::*;

    /// `NoteBlock.setInstrument`.
    pub fn with_instrument(w: &World, pos: BlockPos, s: Block) -> Block {
        if let Block::Note { note, powered, .. } = s {
            let above = bh::instrument(&w.get(pos.above()));
            let instrument = if above.works_above_note_block() {
                above
            } else {
                let below = bh::instrument(&w.get(pos.below()));
                if below.works_above_note_block() { Instrument::Harp } else { below }
            };
            Block::Note { instrument, note, powered }
        } else {
            s
        }
    }

    pub fn update_shape(w: &mut World, s: Block, pos: BlockPos, d: Dir) -> Block {
        if d.axis() == crate::pos::Axis::Y { with_instrument(w, pos, s) } else { s }
    }

    pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Note { instrument, note, powered } = s {
            let signal = w.has_neighbor_signal(pos);
            if signal != powered {
                if signal && (instrument.works_above_note_block() || w.get(pos.above()).is_air()) {
                    w.block_event(pos, Kind::Note, 0, 0);
                    w.game_event("note_block_play", pos);
                }
                w.set_block_and_update(pos, Block::Note { instrument, note, powered: signal });
            }
        }
    }
}

pub mod target {
    use super::*;

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Target { power } = s
            && power != 0
        {
            w.set_block_and_update(pos, Block::Target { power: 0 });
        }
    }

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos, old: Block) {
        if old.kind() != Kind::Target
            && let Block::Target { power } = s
            && power > 0
            && !w.has_scheduled_tick(pos, Kind::Target)
        {
            w.set_block(pos, Block::Target { power: 0 }, flags::CLIENTS | flags::KNOWN_SHAPE);
        }
    }
}

/// Blocks whose `powered` (and `open`) follow `hasNeighborSignal` in the same update.
pub mod indicator {
    use super::*;

    pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos, src: Kind) {
        match s {
            Block::Trapdoor { iron, facing, top, powered, .. } => {
                let signal = w.has_neighbor_signal(pos);
                if signal != powered {
                    w.set_block(pos, Block::Trapdoor { iron, facing, top, open: signal, powered: signal }, flags::CLIENTS);
                }
            }
            Block::Door { iron, facing, upper, hinge_right, open, powered } => {
                let other = pos.rel(if upper { Dir::Down } else { Dir::Up });
                let signal = w.has_neighbor_signal(pos) || w.has_neighbor_signal(other);
                if src != s.kind() && signal != powered {
                    if signal != open {
                        w.game_event(if signal { "block_open" } else { "block_close" }, pos);
                    }
                    w.set_block(pos, Block::Door { iron, facing, upper, hinge_right, open: signal, powered: signal }, flags::CLIENTS);
                }
            }
            Block::FenceGate { facing, in_wall, open, powered } => {
                let has = w.has_neighbor_signal(pos);
                if powered != has {
                    w.set_block(pos, Block::FenceGate { facing, in_wall, open: has, powered: has }, flags::CLIENTS);
                    if open != has {
                        w.game_event(if has { "block_open" } else { "block_close" }, pos);
                    }
                }
            }
            Block::Skull { rotation, powered } => {
                let signal = w.has_neighbor_signal(pos);
                if signal != powered {
                    w.set_block(pos, Block::Skull { rotation, powered: signal }, flags::CLIENTS);
                }
            }
            Block::WallSkull { facing, powered } => {
                let signal = w.has_neighbor_signal(pos);
                if signal != powered {
                    w.set_block(pos, Block::WallSkull { facing, powered: signal }, flags::CLIENTS);
                }
            }
            _ => {}
        }
    }

    /// Doors: the two halves follow each other (`DoorBlock.updateShape`); the lower half needs
    /// a sturdy floor.
    pub fn door_update_shape(w: &mut World, s: Block, pos: BlockPos, d: Dir, ns: Block) -> Block {
        if let Block::Door { upper, .. } = s {
            let lower = !upper;
            if d.axis() != crate::pos::Axis::Y || lower != (d == Dir::Up) {
                if lower && d == Dir::Down && !door_can_survive(w, &s, pos) {
                    return Block::Air;
                }
                return s;
            }
            // the other half: any door, copied with this half
            match ns {
                Block::Door { iron, facing, upper: nupper, hinge_right, open, powered } if nupper != upper => {
                    Block::Door { iron, facing, upper, hinge_right, open, powered }
                }
                _ => Block::Air,
            }
        } else {
            s
        }
    }

    pub fn door_can_survive(w: &World, s: &Block, pos: BlockPos) -> bool {
        if let Block::Door { upper, .. } = *s {
            let b = pos.below();
            let bs = w.get(b);
            if !upper { bh::sturdy(w, &bs, b, Dir::Up, bh::Support::Full) } else { bs.kind() == s.kind() }
        } else {
            true
        }
    }

    /// `FenceGateBlock.updateShape`: `in_wall` next to walls (none are modeled: always false).
    pub fn gate_update_shape(s: Block, d: Dir) -> Block {
        if let Block::FenceGate { facing, open, powered, .. } = s
            && facing.clockwise().axis() == d.axis()
        {
            return Block::FenceGate { facing, in_wall: false, open, powered };
        }
        s
    }
}

pub mod rail {
    use super::*;

    fn rigid_below(w: &World, p: BlockPos) -> bool {
        bh::sturdy(w, &w.get(p), p, Dir::Up, bh::Support::Rigid)
    }

    pub fn can_survive(w: &World, pos: BlockPos) -> bool {
        rigid_below(w, pos.below())
    }

    fn should_be_removed(w: &World, pos: BlockPos, shape: RailShape) -> bool {
        if !rigid_below(w, pos.below()) {
            return true;
        }
        match shape {
            RailShape::AscendingEast => !rigid_below(w, pos.rel(Dir::East)),
            RailShape::AscendingWest => !rigid_below(w, pos.rel(Dir::West)),
            RailShape::AscendingNorth => !rigid_below(w, pos.rel(Dir::North)),
            RailShape::AscendingSouth => !rigid_below(w, pos.rel(Dir::South)),
            _ => false,
        }
    }

    pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos, moved: bool) {
        if w.get(pos).kind() != s.kind() {
            return;
        }
        if let Block::Rail { shape, .. } = s {
            if should_be_removed(w, pos, shape) {
                w.drops.push((pos, s));
                w.remove_block(pos, moved);
            } else {
                update_state(w, s, pos);
            }
        }
    }

    fn find_powered_rail_signal(w: &World, pos: BlockPos, s: &Block, forward: bool, depth: i32) -> bool {
        if depth >= 8 {
            return false;
        }
        let Block::Rail { shape, .. } = *s else { return false };
        let (mut x, mut y, mut z) = (pos.x, pos.y, pos.z);
        let mut check_below = true;
        let mut shape = shape;
        match shape {
            RailShape::NorthSouth => {
                if forward {
                    z += 1
                } else {
                    z -= 1
                }
            }
            RailShape::EastWest => {
                if forward {
                    x -= 1
                } else {
                    x += 1
                }
            }
            RailShape::AscendingEast => {
                if forward {
                    x -= 1;
                } else {
                    x += 1;
                    y += 1;
                    check_below = false;
                }
                shape = RailShape::EastWest;
            }
            RailShape::AscendingWest => {
                if forward {
                    x -= 1;
                    y += 1;
                    check_below = false;
                } else {
                    x += 1;
                }
                shape = RailShape::EastWest;
            }
            RailShape::AscendingNorth => {
                if forward {
                    z += 1;
                } else {
                    z -= 1;
                    y += 1;
                    check_below = false;
                }
                shape = RailShape::NorthSouth;
            }
            RailShape::AscendingSouth => {
                if forward {
                    z += 1;
                    y += 1;
                    check_below = false;
                } else {
                    z -= 1;
                }
                shape = RailShape::NorthSouth;
            }
        }
        let k = s.kind();
        same_rail_with_power(w, BlockPos::new(x, y, z), forward, depth, shape, k)
            || (check_below && same_rail_with_power(w, BlockPos::new(x, y - 1, z), forward, depth, shape, k))
    }

    fn same_rail_with_power(w: &World, pos: BlockPos, forward: bool, depth: i32, dir: RailShape, k: Kind) -> bool {
        let s = w.get(pos);
        if s.kind() != k {
            return false;
        }
        let Block::Rail { shape: my, powered, .. } = s else { return false };
        if dir == RailShape::EastWest && matches!(my, RailShape::NorthSouth | RailShape::AscendingNorth | RailShape::AscendingSouth) {
            return false;
        }
        if dir == RailShape::NorthSouth && matches!(my, RailShape::EastWest | RailShape::AscendingEast | RailShape::AscendingWest) {
            return false;
        }
        if !powered {
            return false;
        }
        w.has_neighbor_signal(pos) || find_powered_rail_signal(w, pos, &s, forward, depth + 1)
    }

    /// `PoweredRailBlock.updateState`.
    fn update_state(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Rail { activator, shape, powered } = s {
            let should = w.has_neighbor_signal(pos) || find_powered_rail_signal(w, pos, &s, true, 0) || find_powered_rail_signal(w, pos, &s, false, 0);
            if should != powered {
                w.set_block_and_update(pos, Block::Rail { activator, shape, powered: should });
                w.update_neighbors_at(pos.below(), s.kind());
                if shape.is_ascending() {
                    w.update_neighbors_at(pos.above(), s.kind());
                }
            }
        }
    }

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos, old: Block, moved: bool) {
        if old.kind() != s.kind() {
            // BaseRailBlock.updateState(state, level, pos, movedByPiston): RailState.place may
            // reshape this rail and its neighbors (not modeled yet), then a full neighbor
            // update of the rail itself.
            w.errors.push(format!("rail onPlace at {pos} (RailState.place is not modeled)"));
            w.neighbor_changed_full(s, pos, s.kind(), moved);
        }
    }

    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos, moved: bool) {
        if !moved && let Block::Rail { shape, .. } = s {
            if shape.is_ascending() {
                w.update_neighbors_at(pos.above(), s.kind());
            }
            w.update_neighbors_at(pos, s.kind());
            w.update_neighbors_at(pos.below(), s.kind());
        }
    }
}

/// Dispensers, droppers and crafters: `triggered` follows the power (dispensers and droppers
/// also read the cell above: quasi-connectivity); a rising edge schedules the 4 gt dispense,
/// which does nothing to blocks when empty.
pub mod dispenser {
    use super::*;

    pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos) {
        match s {
            Block::Dispenser { dropper, facing, triggered } => {
                let should = w.has_neighbor_signal(pos) || w.has_neighbor_signal(pos.above());
                if should && !triggered {
                    w.schedule_tick(pos, s.kind(), 4, Priority::Normal);
                    w.set_block(pos, Block::Dispenser { dropper, facing, triggered: true }, flags::CLIENTS);
                } else if !should && triggered {
                    w.set_block(pos, Block::Dispenser { dropper, facing, triggered: false }, flags::CLIENTS);
                }
            }
            Block::Crafter { orientation, triggered, .. } => {
                let should = w.has_neighbor_signal(pos);
                if should && !triggered {
                    w.schedule_tick(pos, Kind::Crafter, 4, Priority::Normal);
                    if let Block::Crafter { crafting, .. } = s {
                        w.set_block(pos, Block::Crafter { orientation, triggered: true, crafting }, flags::CLIENTS);
                    }
                } else if !should && triggered {
                    w.set_block(pos, Block::Crafter { orientation, triggered: false, crafting: false }, flags::CLIENTS);
                }
            }
            _ => {}
        }
    }

    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos) {
        w.update_neighbour_for_output_signal(pos, s.kind());
    }
}

pub mod hopper {
    use super::*;

    pub fn check_powered_state(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Hopper { facing, enabled } = s {
            let should = !w.has_neighbor_signal(pos);
            if should != enabled {
                w.set_block(pos, Block::Hopper { facing, enabled: should }, flags::CLIENTS);
            }
        }
    }

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos, old: Block) {
        if old.kind() != Kind::Hopper {
            check_powered_state(w, s, pos);
        }
    }
}

pub mod leaves {
    use super::*;

    /// `LeavesBlock.getDistanceAt`: logs 0, leaves their distance, anything else 7.
    fn distance_at(s: &Block) -> u8 {
        match *s {
            Block::Log { .. } => 0,
            Block::Leaves { distance, .. } => distance,
            _ => 7,
        }
    }

    pub fn update_shape(w: &mut World, s: Block, pos: BlockPos, ns: Block) -> Block {
        if let Block::Leaves { distance, .. } = s {
            let from = distance_at(&ns) + 1;
            if from != 1 || distance != from {
                w.schedule_tick(pos, Kind::Leaves, 1, Priority::Normal);
            }
        }
        s
    }

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Leaves { persistent, .. } = s {
            let mut d = 7u8;
            for dir in Dir::ALL {
                d = d.min(distance_at(&w.get(pos.rel(dir))) + 1);
                if d == 1 {
                    break;
                }
            }
            w.set_block_and_update(pos, Block::Leaves { distance: d, persistent });
        }
    }
}

pub mod composter {
    use super::*;

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Composter { level: 7 } = s {
            w.schedule_tick(pos, Kind::Composter, 20, Priority::Normal);
        }
    }

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Composter { level: 7 } = s {
            w.set_block(pos, Block::Composter { level: 8 }, flags::ALL);
        }
    }
}
