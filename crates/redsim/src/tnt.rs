//! TNT and the dead coral fans of TNT dupers (track tnt, study 9; docs/circuits.md), read from the
//! 26.3 source.
//!
//! * `TntBlock` 44-97: `onPlace` (a different old block) and `neighborChanged` prime the TNT when
//!   it has a neighbor signal, then `removeBlock(pos, false)` (flags 3). `prime` adds a
//!   `PrimedTnt` (here: a record in `World::primed`, the entity's motion draws from
//!   `level.random`, and the judge removes primed TNT before it could explode) and emits
//!   `PRIME_FUSE` with no source entity. `UNSTABLE` only matters to players breaking it;
//!   `wasExploded` and burning projectiles never happen in redsim (no explosions, no projectiles).
//!   TNT is not a conductor (`isRedstoneConductor(Blocks::never)`), a full cube, pushable, and
//!   drops itself unless unstable (`VanillaBlockLoot`).
//! * Dead coral fans (`BaseCoralPlantTypeBlock` 65-87, `BaseCoralWallFanBlock` 51-74): pushable
//!   (no `POPPED`: slime drags them along), no collision, forced solid, drop nothing without silk
//!   touch (`dropWhenSilkTouch`), BASEDRUM. A floor fan needs a sturdy top face below and pops
//!   only on a DOWN shape update; a wall fan needs the `facing` face of the block behind it and
//!   pops only on the update from that side. A waterlogged fan schedules a water tick on every
//!   shape update first (`fluid::waterlogged_update_shape`). Dead fans schedule no dying tick.
//!
//! Why dupers work: `PistonBaseBlock.moveBlocks` copies the pushed states first (274) and blanks
//! the sources last (324); a fan riding the structure pops when the block under it turns
//! `moving_piston` (the flag-324 write updates shapes before the block entity exists: no support),
//! its `destroyBlock` updates a TNT still standing at its source, and a powered TNT primes while
//! its moving copy lands later.

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// The dead coral fan kinds (`dead_<kind>_coral_fan`), by `Block::CoralFan::kind`.
pub const DEAD_CORALS: [&str; 5] = ["tube", "brain", "bubble", "fire", "horn"];

/// `TntBlock.prime(level, pos)` with no source: the primed TNT and `PRIME_FUSE`. The `PrimedTnt`
/// is an entity only with `World::tnt_kick` (its random kick replaced by that value); else a
/// record that still takes its entity id (later entities keep the game's ids).
fn prime(w: &mut World, p: BlockPos) -> bool {
    w.primed.push((w.time, p));
    // `new PrimedTnt(...)` draws its kick from the level random (`nextDouble()`): with the level
    // random mirrored, that draw happens (and is the kick an entity takes); else `tnt_kick` stands in
    let drawn = w.rng().map(|r| r.next_double());
    match w.tnt_kick {
        Some(u) => {
            let pos = crate::entity::Vec3::new(p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5);
            crate::entity::primed_tnt::spawn(w, pos, drawn.unwrap_or(u));
        }
        None => w.entities.next_id += 1,
    }
    w.game_event("prime_fuse", p);
    true
}

/// `TntBlock.onPlace`.
pub fn on_place(w: &mut World, p: BlockPos, old: Block) {
    if old.kind() != Kind::Tnt && w.has_neighbor_signal(p) && prime(w, p) {
        w.remove_block(p, false);
    }
}

/// `TntBlock.neighborChanged`.
pub fn neighbor_changed(w: &mut World, p: BlockPos) {
    if w.has_neighbor_signal(p) && prime(w, p) {
        w.remove_block(p, false);
    }
}

/// `canSurvive` of a dead coral fan (floor or wall).
pub fn fan_can_survive(w: &World, s: &Block, p: BlockPos) -> bool {
    match *s {
        Block::CoralFan { .. } => {
            let below = p.below();
            bh::sturdy(w, &w.get(below), below, Dir::Up, Support::Full)
        }
        Block::CoralWallFan { facing, .. } => {
            let q = p.rel(facing.opposite());
            bh::sturdy(w, &w.get(q), q, facing, Support::Full)
        }
        _ => true,
    }
}

/// `updateShape` of a dead coral fan (floor or wall).
pub fn fan_update_shape(w: &mut World, s: Block, p: BlockPos, d: Dir) -> Block {
    crate::fluid::waterlogged_update_shape(w, &s, p);
    let pops = match s {
        Block::CoralFan { .. } => d == Dir::Down,
        Block::CoralWallFan { facing, .. } => d.opposite() == facing,
        _ => false,
    };
    if pops && !fan_can_survive(w, &s, p) { Block::Air } else { s }
}
