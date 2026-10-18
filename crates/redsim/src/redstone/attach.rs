//! Redstone torches (`RedstoneTorchBlock`, `RedstoneWallTorchBlock`) with burnout, levers
//! (`LeverBlock`) and buttons (`ButtonBlock`).

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Face, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::World;

pub mod torch {
    use super::*;

    fn lit(s: &Block) -> bool {
        match *s {
            Block::Torch { lit } | Block::WallTorch { lit, .. } => lit,
            _ => false,
        }
    }

    fn with_lit(s: Block, lit: bool) -> Block {
        match s {
            Block::Torch { .. } => Block::Torch { lit },
            Block::WallTorch { facing, .. } => Block::WallTorch { facing, lit },
            _ => s,
        }
    }

    pub fn can_survive(w: &World, s: &Block, pos: BlockPos) -> bool {
        match *s {
            // BaseTorchBlock: canSupportCenter(below, UP)
            Block::Torch { .. } => {
                let b = pos.below();
                bh::sturdy(w, &w.get(b), b, Dir::Up, Support::Center)
            }
            // WallTorchBlock.canSurvive: the block behind is sturdy on that face
            Block::WallTorch { facing, .. } => {
                let behind = pos.rel(facing.opposite());
                bh::sturdy(w, &w.get(behind), behind, facing, Support::Full)
            }
            _ => true,
        }
    }

    pub fn update_shape(w: &mut World, s: Block, pos: BlockPos, d: Dir) -> Block {
        match s {
            Block::Torch { .. } => {
                if d == Dir::Down && !can_survive(w, &s, pos) {
                    Block::Air
                } else {
                    s
                }
            }
            Block::WallTorch { facing, .. } => {
                if d.opposite() == facing && !can_survive(w, &s, pos) {
                    Block::Air
                } else {
                    s
                }
            }
            _ => s,
        }
    }

    fn notify_neighbors(w: &mut World, s: &Block, pos: BlockPos) {
        for d in Dir::ALL {
            w.update_neighbors_at(pos.rel(d), s.kind());
        }
    }

    pub fn on_place(w: &mut World, s: Block, pos: BlockPos) {
        notify_neighbors(w, &s, pos);
    }

    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos, moved: bool) {
        if !moved {
            notify_neighbors(w, &s, pos);
        }
    }

    fn has_neighbor_signal(w: &World, s: &Block, pos: BlockPos) -> bool {
        match *s {
            Block::WallTorch { facing, .. } => {
                let o = facing.opposite();
                w.has_signal(pos.rel(o), o)
            }
            _ => w.has_signal(pos.below(), Dir::Down),
        }
    }

    fn toggled_too_frequently(w: &mut World, pos: BlockPos, add: bool) -> bool {
        if add {
            w.torch_toggles.push((pos, w.time));
        }
        w.torch_toggles.iter().filter(|(p, _)| *p == pos).count() >= 8
    }

    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        let signal = has_neighbor_signal(w, &s, pos);
        while let Some(&(_, when)) = w.torch_toggles.first() {
            if w.time - when > 60 {
                w.torch_toggles.remove(0);
            } else {
                break;
            }
        }
        if lit(&s) {
            if signal {
                w.set_block_and_update(pos, with_lit(s, false));
                if toggled_too_frequently(w, pos, true) {
                    let k = w.get(pos).kind();
                    w.schedule_tick(pos, k, 160, Priority::Normal);
                }
            }
        } else if !signal && !toggled_too_frequently(w, pos, false) {
            w.set_block_and_update(pos, with_lit(s, true));
        }
    }

    pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos) {
        if lit(&s) == has_neighbor_signal(w, &s, pos) && !w.will_tick_this_tick(pos, s.kind()) {
            w.schedule_tick(pos, s.kind(), 2, Priority::Normal);
        }
    }

    pub fn get_signal(s: &Block, d: Dir) -> i32 {
        if !lit(s) {
            return 0;
        }
        match *s {
            Block::WallTorch { facing, .. } => {
                if facing != d {
                    15
                } else {
                    0
                }
            }
            _ => {
                if d != Dir::Up {
                    15
                } else {
                    0
                }
            }
        }
    }

    pub fn get_direct_signal(s: &Block, d: Dir) -> i32 {
        if d == Dir::Down { get_signal(s, d) } else { 0 }
    }
}

/// Levers and buttons hang on a face (`FaceAttachedHorizontalDirectionalBlock`).
pub mod attached {
    use super::*;

    pub fn face_facing(s: &Block) -> (Face, Dir) {
        match *s {
            Block::Lever { face, facing, .. } | Block::Button { face, facing, .. } => (face, facing),
            _ => (Face::Wall, Dir::North),
        }
    }

    /// `getConnectedDirection`.
    pub fn connected(s: &Block) -> Dir {
        let (face, facing) = face_facing(s);
        face.connected(facing)
    }

    pub fn can_survive(w: &World, s: &Block, pos: BlockPos) -> bool {
        let d = connected(s).opposite();
        let rel = pos.rel(d);
        bh::sturdy(w, &w.get(rel), rel, d.opposite(), Support::Full)
    }

    pub fn update_shape(w: &mut World, s: Block, pos: BlockPos, d: Dir) -> Block {
        if connected(&s).opposite() == d && !can_survive(w, &s, pos) { Block::Air } else { s }
    }

    pub fn powered(s: &Block) -> bool {
        match *s {
            Block::Lever { powered, .. } | Block::Button { powered, .. } => powered,
            _ => false,
        }
    }

    pub fn with_powered(s: Block, p: bool) -> Block {
        match s {
            Block::Lever { face, facing, .. } => Block::Lever { face, facing, powered: p },
            Block::Button { wood, face, facing, .. } => Block::Button { wood, face, facing, powered: p },
            _ => s,
        }
    }

    /// `updateNeighbours`: its own cell, then its support.
    pub fn update_neighbours(w: &mut World, s: &Block, pos: BlockPos) {
        let front = connected(s).opposite();
        w.update_neighbors_at(pos, s.kind());
        w.update_neighbors_at(pos.rel(front), s.kind());
    }

    pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos, moved: bool) {
        if !moved && powered(&s) {
            update_neighbours(w, &s, pos);
        }
    }

    pub fn get_signal(s: &Block) -> i32 {
        if powered(s) { 15 } else { 0 }
    }

    pub fn get_direct_signal(s: &Block, d: Dir) -> i32 {
        if powered(s) && connected(s) == d { 15 } else { 0 }
    }
}

pub mod lever {
    use super::*;

    /// `LeverBlock.pull`.
    pub fn pull(w: &mut World, s: Block, pos: BlockPos) {
        let ns = attached::with_powered(s, !attached::powered(&s));
        w.set_block_and_update(pos, ns);
        attached::update_neighbours(w, &ns, pos);
        w.game_event(if attached::powered(&ns) { "block_activate" } else { "block_deactivate" }, pos);
    }
}

pub mod button {
    use super::*;

    fn ticks_to_stay_pressed(s: &Block) -> i32 {
        match *s {
            Block::Button { wood: true, .. } => 30,
            _ => 20,
        }
    }

    /// `ButtonBlock.press` (the bridge's `use` calls it even on a pressed button: the state is
    /// unchanged, the neighbors are updated again, the pending release stays).
    pub fn press(w: &mut World, s: Block, pos: BlockPos) {
        w.set_block_and_update(pos, attached::with_powered(s, true));
        attached::update_neighbours(w, &s, pos);
        w.schedule_tick(pos, s.kind(), ticks_to_stay_pressed(&s), Priority::Normal);
        w.game_event("block_activate", pos);
    }

    /// `tick` -> `checkPressed` with no arrow in the cell.
    pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
        if attached::powered(&s) {
            w.set_block_and_update(pos, attached::with_powered(s, false));
            attached::update_neighbours(w, &s, pos);
            w.game_event("block_deactivate", pos);
        }
    }

    pub fn kind_ok(_k: Kind) -> bool {
        true
    }
}
