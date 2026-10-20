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
                if signal {
                    play_note(w, instrument, pos);
                }
                w.set_block_and_update(pos, Block::Note { instrument, note, powered: signal });
            }
        }
    }

    fn play_note(w: &mut World, instrument: Instrument, pos: BlockPos) {
        if instrument.works_above_note_block() || w.get(pos.above()).is_air() {
            w.block_event(pos, Kind::Note, 0, 0);
            w.game_event("note_block_play", pos);
        }
    }

    /// `useWithoutItem`: tune (note + 1, wrapping at 25), then play.
    pub fn use_block(w: &mut World, s: Block, pos: BlockPos) {
        if let Block::Note { instrument, note, powered } = s {
            let ns = Block::Note { instrument, note: (note + 1) % 25, powered };
            w.set_block_and_update(pos, ns);
            play_note(w, instrument, pos);
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
            Block::Trapdoor { iron, facing, top, open, powered } => {
                let signal = w.has_neighbor_signal(pos);
                if signal != powered {
                    // TrapDoorBlock.playSound: the game event comes before the write
                    if open != signal {
                        w.game_event(if signal { "block_open" } else { "block_close" }, pos);
                    }
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
            // (clk-misc) curves exist only for plain rails, never here
            _ => return false,
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
            // BaseRailBlock.updateState(state, level, pos, movedByPiston): RailState.place
            // reshapes this rail (and its neighbors), then a full neighbor update of the rail with
            // the new state (straight rails).
            let current = match s {
                Block::Rail { shape, .. } => shape,
                _ => RailShape::NorthSouth,
            };
            let signal = w.has_neighbor_signal(pos);
            let ns = RailState::new(w, pos, s).place(w, signal, true, current);
            w.neighbor_changed_full(ns, pos, s.kind(), moved);
        }
    }

    fn is_rail(s: &Block) -> bool {
        matches!(s, Block::Rail { .. })
    }

    fn shape_of(s: &Block) -> RailShape {
        match *s {
            Block::Rail { shape, .. } => shape,
            _ => RailShape::NorthSouth,
        }
    }

    fn with_shape(s: Block, shape: RailShape) -> Block {
        match s {
            Block::Rail { activator, powered, .. } => Block::Rail { activator, shape, powered },
            _ => s,
        }
    }

    /// `RailState` for straight rails (powered, activator).
    struct RailState {
        pos: BlockPos,
        state: Block,
        conns: Vec<BlockPos>,
    }

    fn conns_for(pos: BlockPos, shape: RailShape) -> Vec<BlockPos> {
        let (n, s, wst, e) = (pos.rel(Dir::North), pos.rel(Dir::South), pos.rel(Dir::West), pos.rel(Dir::East));
        match shape {
            RailShape::NorthSouth => vec![n, s],
            RailShape::EastWest => vec![wst, e],
            RailShape::AscendingEast => vec![wst, e.above()],
            RailShape::AscendingWest => vec![wst.above(), e],
            RailShape::AscendingNorth => vec![n.above(), s],
            RailShape::AscendingSouth => vec![n, s.above()],
            // (clk-misc) the plain rail's curves
            RailShape::SouthEast => vec![e, s],
            RailShape::SouthWest => vec![wst, s],
            RailShape::NorthWest => vec![wst, n],
            RailShape::NorthEast => vec![e, n],
        }
    }

    fn get_rail(w: &World, p: BlockPos) -> Option<RailState> {
        for q in [p, p.above(), p.below()] {
            let s = w.get(q);
            if is_rail(&s) {
                return Some(RailState::new(w, q, s));
            }
        }
        None
    }

    impl RailState {
        fn new(_w: &World, pos: BlockPos, state: Block) -> RailState {
            RailState { pos, state, conns: conns_for(pos, shape_of(&state)) }
        }

        fn has_connection(&self, p: BlockPos) -> bool {
            self.conns.iter().any(|c| c.x == p.x && c.z == p.z)
        }

        fn remove_soft_connections(&mut self, w: &World) {
            let mut i = 0;
            while i < self.conns.len() {
                match get_rail(w, self.conns[i]) {
                    Some(r) if r.has_connection(self.pos) => {
                        self.conns[i] = r.pos;
                        i += 1;
                    }
                    _ => {
                        self.conns.remove(i);
                    }
                }
            }
        }

        fn can_connect_to(&self, p: BlockPos) -> bool {
            self.has_connection(p) || self.conns.len() != 2
        }

        fn connect_to(&mut self, w: &mut World, p: BlockPos) {
            self.conns.push(p);
            let (n, s, wst, e) = (self.pos.rel(Dir::North), self.pos.rel(Dir::South), self.pos.rel(Dir::West), self.pos.rel(Dir::East));
            let (hn, hs, hw, he) = (self.has_connection(n), self.has_connection(s), self.has_connection(wst), self.has_connection(e));
            let mut shape = None;
            if hn || hs {
                shape = Some(RailShape::NorthSouth);
            }
            if hw || he {
                shape = Some(RailShape::EastWest);
            }
            shape = ascend(w, shape, n, s, wst, e);
            let shape = shape.unwrap_or(RailShape::NorthSouth);
            self.state = with_shape(self.state, shape);
            w.set_block_and_update(self.pos, self.state);
        }

        fn has_neighbor_rail(&self, w: &World, p: BlockPos) -> bool {
            match get_rail(w, p) {
                Some(mut n) => {
                    n.remove_soft_connections(w);
                    n.can_connect_to(self.pos)
                }
                None => false,
            }
        }

        fn place(mut self, w: &mut World, _has_signal: bool, first: bool, default: RailShape) -> Block {
            let (n, s, wst, e) = (self.pos.rel(Dir::North), self.pos.rel(Dir::South), self.pos.rel(Dir::West), self.pos.rel(Dir::East));
            let hn = self.has_neighbor_rail(w, n);
            let hs = self.has_neighbor_rail(w, s);
            let hw = self.has_neighbor_rail(w, wst);
            let he = self.has_neighbor_rail(w, e);
            let ns = hn || hs;
            let ew = hw || he;
            let mut shape = None;
            if ns && !ew {
                shape = Some(RailShape::NorthSouth);
            }
            if ew && !ns {
                shape = Some(RailShape::EastWest);
            }
            if shape.is_none() {
                if ns && ew {
                    shape = Some(default);
                } else if ns {
                    shape = Some(RailShape::NorthSouth);
                } else if ew {
                    shape = Some(RailShape::EastWest);
                }
            }
            shape = ascend(w, shape, n, s, wst, e);
            let shape = shape.unwrap_or(default);
            self.conns = conns_for(self.pos, shape);
            self.state = with_shape(self.state, shape);
            if first || w.get(self.pos) != self.state {
                w.set_block_and_update(self.pos, self.state);
                for i in 0..self.conns.len() {
                    if let Some(mut nb) = get_rail(w, self.conns[i]) {
                        nb.remove_soft_connections(w);
                        if nb.can_connect_to(self.pos) {
                            nb.connect_to(w, self.pos);
                        }
                    }
                }
            }
            self.state
        }
    }

    /// The slope rule shared by `place` and `connectTo`.
    fn ascend(w: &World, shape: Option<RailShape>, n: BlockPos, s: BlockPos, wst: BlockPos, e: BlockPos) -> Option<RailShape> {
        let mut shape = shape;
        if shape == Some(RailShape::NorthSouth) {
            if is_rail(&w.get(n.above())) {
                shape = Some(RailShape::AscendingNorth);
            }
            if is_rail(&w.get(s.above())) {
                shape = Some(RailShape::AscendingSouth);
            }
        }
        if shape == Some(RailShape::EastWest) {
            if is_rail(&w.get(e.above())) {
                shape = Some(RailShape::AscendingEast);
            }
            if is_rail(&w.get(wst.above())) {
                shape = Some(RailShape::AscendingWest);
            }
        }
        shape
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

/// `ScaffoldingBlock` (26.3): a 1 gt delay element. Every shape update, and every placement
/// (its `onPlace` has no same-block check, so each of its own state changes too), schedules a
/// NORMAL tick 1 gt later. The tick recomputes `distance` (`getDistance`: 0 on a block whose top
/// is sturdy, else the distance of a scaffold below, lowered to 1 + the distance of a scaffold
/// beside it, at most 7) and `bottom` (distance > 0 and no scaffold below), then updates
/// (`setBlockAndUpdate`), or, at distance 7, breaks with a drop (`destroyBlock`) when it was
/// lower, or falls (`FallingBlockEntity.fall`, a falling-block entity: `gravity.rs`) when it
/// already was 7. Static properties (`Blocks.SCAFFOLDING`, collision shape with the empty
/// context = `SHAPE_STABLE`): non-conductor, only its top face sturdy (dust, torches, diodes,
/// rails can stand on it), collides but not a full cube, not solid (dynamic shape: no cache),
/// popped by pistons, not replaceable.
pub mod scaffolding {
    use super::*;
    use crate::behavior::Support;

    /// `ScaffoldingBlock.getDistance`.
    pub fn distance(w: &World, pos: BlockPos) -> u8 {
        let below = pos.below();
        let bs = w.get(below);
        let mut d = 7u8;
        if let Block::Scaffolding { distance, .. } = bs {
            d = distance;
        } else if bh::sturdy(w, &bs, below, Dir::Up, Support::Full) {
            return 0;
        }
        for dir in Dir::HORIZONTAL {
            if let Block::Scaffolding { distance, .. } = w.get(pos.rel(dir)) {
                d = d.min(distance + 1);
                if d == 1 {
                    break;
                }
            }
        }
        d
    }

    /// `ScaffoldingBlock.isBottom`.
    fn is_bottom(w: &World, pos: BlockPos, d: u8) -> bool {
        d > 0 && !matches!(w.get(pos.below()), Block::Scaffolding { .. })
    }

    /// `getCollisionShape` for an entity whose bottom is at `entity_bottom` (not descending, not
    /// a placement): `SHAPE_STABLE` (top plate + legs, `behavior::collision_boxes`) when it is
    /// above the cell, the 2 px bottom plate when the scaffold hangs (distance > 0, bottom) and the
    /// entity is above the cell's floor, else nothing. `isAbove` = bottom > y + max - 1e-5F.
    pub fn entity_boxes(s: &Block, cell_y: i32, entity_bottom: f64) -> Vec<bh::Box16> {
        let Block::Scaffolding { distance, bottom } = *s else { return vec![] };
        let eps = 1.0e-5f32 as f64;
        if entity_bottom > cell_y as f64 + 1.0 - eps {
            bh::collision_boxes(s)
        } else if distance != 0 && bottom && entity_bottom > cell_y as f64 - eps {
            vec![[0, 16, 0, 2, 0, 16]]
        } else {
            vec![]
        }
    }

    /// `onPlace` / `updateShape`: a tick 1 gt later.
    pub fn schedule(w: &mut World, pos: BlockPos) {
        w.schedule_tick(pos, Kind::Scaffolding, 1, Priority::Normal);
    }

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        let Block::Scaffolding { distance: old, .. } = s else { return };
        let d = distance(w, pos);
        let ns = Block::Scaffolding { distance: d, bottom: is_bottom(w, pos, d) };
        if d == 7 {
            if old == 7 {
                // FallingBlockEntity.fall: the cell becomes air (flags 3), then the entity
                // carrying the new state (distance 7) joins the entity list
                let e = crate::gravity::FallingBlock { block: ns, x: pos.x as f64 + 0.5, y: pos.y as f64, z: pos.z as f64 + 0.5, vy: 0.0, time: 0, removed: false };
                w.set_block_and_update(pos, Block::Air);
                w.gravity.entities.push(e);
            } else {
                w.destroy_block(pos, true, 512);
            }
        } else if ns != s {
            w.set_block_and_update(pos, ns);
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
