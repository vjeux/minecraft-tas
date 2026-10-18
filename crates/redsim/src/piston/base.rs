//! `PistonBaseBlock`: the power check, block events, extension and retraction.

use super::moving::{self, MovingBe};
use super::{TRIGGER_EXTEND, dir_from_index, dir_index, is_piston, is_pushable, rel_n, resolver};
use crate::behavior as bh;
use crate::block::{Block, Kind, PushReaction};
use crate::pos::{BlockPos, Dir, java_hashset_order};
use crate::world::World;

/// `getNeighborSignal`: the 5 non-front sides, the dead `hasSignal(pos, DOWN)` (a piston is
/// neither a source nor a conductor), then quasi-connectivity (the 6 neighbors of the cell above
/// except the piston itself). Ideal inputs (`State::ideal`) power it as a neighbor would.
pub fn neighbor_signal(w: &World, p: BlockPos, facing: Dir) -> bool {
    if w.piston.ideal_powered(p) {
        return true;
    }
    for d in Dir::ALL {
        if d != facing && w.has_signal(p.rel(d), d) {
            return true;
        }
    }
    if w.has_signal(p, Dir::Down) {
        return true;
    }
    let above = p.above();
    for d in Dir::ALL {
        if d != Dir::Down && w.has_signal(above.rel(d), d) {
            return true;
        }
    }
    false
}

/// `checkIfExtend`: queue a block event: 0 extend (only if the push resolves now: no retry), 1
/// retract and pull, 2 retract and drop the block still moving two cells ahead.
pub fn check_if_extend(w: &mut World, p: BlockPos, state: Block) {
    let Block::Piston { sticky, facing, extended } = state else { return };
    let extend = neighbor_signal(w, p, facing);
    if extend && !extended {
        if resolver::resolve(w, p, facing, true).is_some() {
            w.block_event(p, Kind::Piston(sticky), TRIGGER_EXTEND, dir_index(facing));
        }
    } else if !extend && extended {
        let pushed = rel_n(p, facing, 2);
        let mut event = 1;
        if let Block::MovingPiston { facing: f2, .. } = w.get(pushed) {
            if f2 == facing {
                if let Some(be) = w.piston.be(pushed) {
                    if be.extending && (be.progress_o < 1 || w.time == be.last_ticked || w.handling_tick) {
                        event = 2;
                    }
                }
            }
        }
        w.block_event(p, Kind::Piston(sticky), event, dir_index(facing));
    }
}

/// `triggerEvent`.
pub fn trigger_event(w: &mut World, state: Block, p: BlockPos, b0: i32, b1: i32) -> bool {
    let Block::Piston { sticky, facing, .. } = state else { return false };
    let extended_state = Block::Piston { sticky, facing, extended: true };
    let extend = neighbor_signal(w, p, facing);
    if extend && (b0 == 1 || b0 == 2) {
        w.set_block(p, extended_state, 2);
        return false;
    }
    if !extend && b0 == 0 {
        return false;
    }
    if b0 == 0 {
        if !move_blocks(w, p, facing, true, sticky) {
            return false;
        }
        w.set_block(p, extended_state, 67);
        w.game_event("block_activate", p);
    } else if b0 == 1 || b0 == 2 {
        let front = p.rel(facing);
        if w.piston.be(front).is_some() {
            moving::final_tick(w, front);
        }
        let mp = Block::MovingPiston { sticky, facing };
        w.set_block(p, mp, 276);
        let moved = Block::Piston { sticky, facing: dir_from_index(b1 & 7), extended: false };
        moving::set_block_entity(w, p, MovingBe::new(moved, facing, false, true));
        w.update_neighbors_at(p, Kind::MovingPiston);
        w.update_neighbour_shapes(mp, p, 2, 512);
        if sticky {
            let two = rel_n(p, facing, 2);
            let moving_state = w.get(two);
            let mut piston_piece = false;
            if let Block::MovingPiston { .. } = moving_state {
                if let Some(be) = w.piston.be(two) {
                    if be.direction == facing && be.extending {
                        moving::final_tick(w, two);
                        piston_piece = true;
                    }
                }
            }
            if !piston_piece {
                if b0 != 1
                    || moving_state.is_air()
                    || !is_pushable(moving_state, facing.opposite(), false, facing)
                    || (bh::push_reaction(&moving_state) != PushReaction::PushPull && !is_piston(moving_state))
                {
                    w.remove_block(front, false);
                } else {
                    move_blocks(w, p, facing, false, sticky);
                }
            }
        } else {
            w.remove_block(front, false);
        }
        w.game_event("block_deactivate", p);
    }
    true
}

/// `Block.dropResources` of a block a piston destroys: an item entity unless its loot table
/// gives nothing without a tool.
fn drop_resources(w: &mut World, state: Block, p: BlockPos) {
    if !matches!(state, Block::PistonHead { .. } | Block::MovingPiston { .. }) {
        w.drops.push((p, state));
    }
}

/// `moveBlocks`.
pub fn move_blocks(w: &mut World, piston: BlockPos, direction: Dir, extending: bool, sticky: bool) -> bool {
    let arm = piston.rel(direction);
    if !extending && matches!(w.get(arm), Block::PistonHead { .. }) {
        w.set_block(arm, Block::Air, 276);
    }
    let Some(res) = resolver::resolve(w, piston, direction, extending) else { return false };
    let to_push = res.to_push;
    let to_destroy = res.to_destroy;
    // deleteAfterMove: a HashMap<BlockPos, BlockState>: every key put first, then removals.
    let shapes: Vec<Block> = to_push.iter().map(|q| w.get(*q)).collect();
    let mut delete: Vec<(BlockPos, Block)> = to_push.iter().copied().zip(shapes.iter().copied()).collect();
    let mut to_update: Vec<Block> = Vec::with_capacity(to_push.len() + to_destroy.len());
    let push_dir = if extending { direction } else { direction.opposite() };
    for &q in to_destroy.iter().rev() {
        let state = w.get(q);
        drop_resources(w, state, q);
        w.set_block(q, Block::Air, 18);
        w.game_event("block_destroy", q);
        to_update.push(state);
    }
    for i in (0..to_push.len()).rev() {
        let q = to_push[i];
        let block_state = w.get(q);
        let dest = q.rel(push_dir);
        delete.retain(|(k, _)| *k != dest);
        let actual = Block::MovingPiston { sticky: false, facing: direction };
        w.set_block(dest, actual, 324);
        moving::set_block_entity(w, dest, MovingBe::new(shapes[i], direction, extending, false));
        to_update.push(block_state);
    }
    if extending {
        let head = Block::PistonHead { sticky, facing: direction, short: false };
        let mp = Block::MovingPiston { sticky, facing: direction };
        delete.retain(|(k, _)| *k != arm);
        w.set_block(arm, mp, 324);
        moving::set_block_entity(w, arm, MovingBe::new(head, direction, true, true));
    }
    let keys: Vec<BlockPos> = delete.iter().map(|(k, _)| *k).collect();
    let order = java_hashset_order(&keys);
    for &q in &order {
        w.set_block(q, Block::Air, 82);
    }
    for &q in &order {
        let old = delete.iter().find(|(k, _)| *k == q).unwrap().1;
        bh::update_indirect_neighbour_shapes(w, old, q, 2, 512);
        w.update_neighbour_shapes(Block::Air, q, 2, 512);
        bh::update_indirect_neighbour_shapes(w, Block::Air, q, 2, 512);
    }
    let mut ui = 0;
    for i in (0..to_destroy.len()).rev() {
        let state = to_update[ui];
        ui += 1;
        let q = to_destroy[i];
        bh::affect_neighbors_after_removal(w, state, q, false);
        bh::update_indirect_neighbour_shapes(w, state, q, 2, 512);
        w.update_neighbors_at(q, state.kind());
    }
    for i in (0..to_push.len()).rev() {
        let k = to_update[ui].kind();
        ui += 1;
        w.update_neighbors_at(to_push[i], k);
    }
    if extending {
        w.update_neighbors_at(arm, Kind::PistonHead);
    }
    true
}
