//! Crop farms (webrender, 2026-10-21: the 3D view of w86's harvesters, study 16): sugar cane
//! (`SugarCaneBlock`, 26.3) and the random ticks that grow it (`ServerLevel.tickChunk`'s
//! `tickBlocks`: `randomTickSpeed` positions in every randomly ticking section of a chunk, each
//! from `Level.getBlockRandomPos`'s generator).
//!
//! The game seeds that generator (`Level.randValue`) from an unseeded random when the level is
//! made, so no two runs grow alike and nothing reproduces a game run's growth: [`RandomTicks`]
//! runs the very same process (the generator, the positions, `randomTickSpeed` of them a section)
//! from a seed of its own, so growth is as random as the game's, and deterministic per seed.

use crate::block::{Block, Kind, Simple};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

/// The random ticks of a world (`gamerule random_tick_speed`, vanilla 3) and its position
/// generator (`Level.randValue`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RandomTicks {
    pub speed: u32,
    pub value: i32,
}

impl RandomTicks {
    pub fn new(speed: u32, seed: i64) -> RandomTicks {
        RandomTicks { speed, value: (seed ^ (seed >> 32)) as i32 }
    }

    /// `Level.getBlockRandomPos(xo, yo, zo, 15)`.
    pub fn position(&mut self, xo: i32, yo: i32, zo: i32) -> BlockPos {
        self.value = self.value.wrapping_mul(3).wrapping_add(1013904223);
        let val = self.value >> 2;
        BlockPos::new(xo + (val & 15), yo + ((val >> 16) & 15), zo + ((val >> 8) & 15))
    }
}

/// Does the block tick randomly (`isRandomlyTicking`, the ones redsim models)?
pub fn randomly_ticking(b: &Block) -> bool {
    matches!(b, Block::SugarCane { .. })
}

/// `tickBlocks` of every chunk with a randomly ticking section: each such section (chunks by x
/// then z, sections bottom up) gets `speed` positions, and a randomly ticking block at one gets
/// its `randomTick`. Called by `World::tick` in `tickChunks` (after the precipitation draws).
pub fn tick_blocks(w: &mut World) {
    let Some(mut rt) = w.random_ticks.take() else { return };
    let mut sections: Vec<(i32, i32, i32)> = w.blocks().into_iter().filter(|(_, b)| randomly_ticking(b)).map(|(p, _)| (p.x.div_euclid(16), p.z.div_euclid(16), p.y.div_euclid(16))).collect();
    sections.sort();
    sections.dedup();
    for (cx, cz, sy) in sections {
        for _ in 0..rt.speed {
            let p = rt.position(cx * 16, sy * 16, cz * 16);
            let s = w.get(p);
            if randomly_ticking(&s) {
                random_tick(w, s, p);
            }
        }
    }
    w.random_ticks = Some(rt);
}

/// `randomTick`.
pub fn random_tick(w: &mut World, s: Block, p: BlockPos) {
    if let Block::SugarCane { age } = s {
        // SugarCaneBlock.randomTick: under air, a cane under 3 blocks tall ages; at 15 it grows the
        // block above (setBlockAndUpdate) and starts again at 0 (flags 260: no neighbour updates,
        // its shape updates still reach an observer)
        if !w.get(p.above()).is_air() {
            return;
        }
        let mut height = 1;
        while matches!(w.get(p.offset(0, -height, 0)), Block::SugarCane { .. }) {
            height += 1;
        }
        if height < 3 {
            if age == 15 {
                w.set_block(p.above(), Block::SugarCane { age: 0 }, flags::ALL);
                w.set_block(p, Block::SugarCane { age: 0 }, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
            } else {
                w.set_block(p, Block::SugarCane { age: age + 1 }, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
            }
        }
    }
}

/// `SugarCaneBlock.canSurvive`: on sugar cane, or on a block of #supports_sugar_cane beside a block
/// whose fluid is water (#supports_sugar_cane_adjacently: water, flowing water, a waterlogged
/// block; frosted ice is not modeled).
pub fn can_survive(w: &World, p: BlockPos) -> bool {
    let below = w.get(p.below());
    if matches!(below, Block::SugarCane { .. }) {
        return true;
    }
    if !supports_cane(&below) {
        return false;
    }
    let b = p.below();
    Dir::HORIZONTAL.iter().any(|d| crate::fluid::fluid_state(w, b.rel(*d)).fluid.is_water())
}

/// #supports_sugar_cane (#substrate_overworld: #dirt, #mud, #moss_blocks, #grass_blocks; #sand):
/// the ones redsim builds.
fn supports_cane(b: &Block) -> bool {
    match b {
        Block::Simple(s) => matches!(s, Simple::Dirt | Simple::GrassBlock | Simple::Mud),
        Block::Falling { id } => matches!(crate::block::FALLING[*id as usize], "sand" | "red_sand"),
        _ => false,
    }
}

/// `SugarCaneBlock.updateShape`: a cane that can no longer stand breaks a tick later ([`tick`]).
pub fn update_shape(w: &mut World, s: Block, p: BlockPos) -> Block {
    if !can_survive(w, p) {
        w.schedule_tick(p, Kind::SugarCane, 1, Priority::Normal);
    }
    s
}

/// `SugarCaneBlock.tick`: `destroyBlock(pos, true)` when it can no longer stand (the item drops).
pub fn tick(w: &mut World, p: BlockPos) {
    if !can_survive(w, p) {
        w.destroy_block(p, true, crate::world::UPDATE_LIMIT);
    }
}
