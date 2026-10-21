//! Kelp (clk-misc, 2026-10-21: the farm judge's third crop): `KelpBlock` (the top, a
//! `GrowingPlantHeadBlock` with its `age`) and `KelpPlantBlock` (the stalk under it, a
//! `GrowingPlantBodyBlock`), 26.3. Both hold a water source (`getFluidState`; liquid containers
//! that take no liquid) and schedule a water tick on every shape update.
//!
//! A kelp block stands on kelp or on a face sturdy upward (not magma) and breaks a tick after it
//! cannot. The top's random tick (age < 25) draws `nextDouble() < 0.14` from the level random and
//! grows into water above (`cycle(AGE)`); a top with kelp above turns into stalk, and a stalk
//! whose block above is no longer kelp turns into a top of random age (`getStateForPlacement`:
//! `nextInt(25)` from the level random, the one handed to shape updates). Pistons pop both.

use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

fn is_kelp(b: &Block) -> bool {
    matches!(b, Block::Kelp { .. } | Block::KelpPlant)
}

/// `GrowingPlantBlock.canSurvive` (kelp: `canAttachTo` refuses #cannot_support_kelp = magma).
pub fn can_survive(w: &World, p: BlockPos) -> bool {
    let below = w.get(p.below());
    if matches!(below, Block::Simple(crate::block::Simple::MagmaBlock)) {
        return false;
    }
    is_kelp(&below) || crate::behavior::sturdy(w, &below, p.below(), Dir::Up, crate::behavior::Support::Full)
}

/// `GrowingPlantBlock.tick`: `destroyBlock(pos, true)` when it cannot stand (its water stays).
pub fn tick(w: &mut World, p: BlockPos) {
    if !can_survive(w, p) {
        w.destroy_block(p, true, crate::world::UPDATE_LIMIT);
    }
}

fn water_tick(w: &mut World, p: BlockPos) {
    w.schedule_fluid_tick(p, crate::fluid::Fluid::Water, 5);
}

/// `GrowingPlantHeadBlock.updateShape` (growth direction up).
pub fn head_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    if d == Dir::Down {
        if !can_survive(w, p) {
            w.schedule_tick(p, Kind::Kelp, 1, Priority::Normal);
        } else if is_kelp(&w.get(p.above())) {
            return Block::KelpPlant;
        }
    }
    if d != Dir::Up || !is_kelp(&ns) {
        water_tick(w, p);
        return s;
    }
    Block::KelpPlant
}

/// `GrowingPlantBodyBlock.updateShape` (growth direction up).
pub fn body_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir, ns: Block) -> Block {
    if d == Dir::Down && !can_survive(w, p) {
        w.schedule_tick(p, Kind::KelpPlant, 1, Priority::Normal);
    }
    if d == Dir::Up && !is_kelp(&ns) {
        // KelpBlock.getStateForPlacement(random): a random age from the level random
        let Some(r) = w.level_random.as_mut() else {
            let why = "a kelp stalk turning into a top draws its age from the level random (World::mirror_level_random)".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            return Block::Kelp { age: 0 };
        };
        return Block::Kelp { age: r.next_int(25) as u8 };
    }
    water_tick(w, p);
    s
}

/// `GrowingPlantHeadBlock.randomTick`: age < 25 and `nextDouble() < 0.14`, into water above.
pub fn random_tick(w: &mut World, s: Block, p: BlockPos) {
    let Block::Kelp { age } = s else { return };
    if age >= 25 {
        return;
    }
    let Some(r) = w.level_random.as_mut() else {
        let why = "kelp's random tick draws from the level random (World::mirror_level_random)".to_string();
        if !w.errors.contains(&why) {
            w.errors.push(why);
        }
        return;
    };
    if r.next_double() < 0.14 && matches!(w.get(p.above()), Block::Water { .. }) {
        w.set_block(p.above(), Block::Kelp { age: age + 1 }, flags::ALL);
    }
}
