//! Falling blocks (agent pistmodel): `FallingBlock` (sand, red sand, gravel, concrete powder)
//! and `FallingBlockEntity`, read in the 26.3 source.
//!
//! * `onPlace` and `updateShape` schedule a tick 2 gt later (pistons land blocks with `onPlace`,
//!   a neighbor change gives a shape update); the tick turns the block into an entity if the cell
//!   below is free (air, or replaceable).
//! * The entity (ticked in the entity phase, insertion order): gravity 0.04, `move` with
//!   collisions against the block collision shapes of its column (full blocks, piston bases and
//!   heads, moving pistons with their progress offset), then drag 0.98F. On the ground it lands:
//!   a moving piston in its cell makes it bounce and wait; a replaceable cell where it would not
//!   keep falling gets the block (`setBlockAndUpdate`); anything else breaks it into an item.
//! * Not modeled: horizontal motion (a falling block spawned by the game has none and nothing
//!   here gives it any), entities pushed by moving pistons, fluids; `applyEffectsFromBlocks`
//!   (pressure plates, tripwires and bubble columns do not see falling blocks, and plate or
//!   tripwire counts leave them out).
//! * The entity list: falling blocks take their ids from the counter every entity shares and tick
//!   with the other entities in id order ([`crate::entity::tick_phase`]).

use crate::behavior as bh;
use crate::block::{Block, Simple};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

pub const MIN_Y: i32 = -64;
pub const MAX_Y: i32 = 319;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FallingBlock {
    pub block: Block,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vy: f64,
    pub time: i32,
    pub removed: bool,
    /// Its entity id, from the counter every entity shares ([`crate::entity::Entities::next_id`]):
    /// the entity phase ticks falling blocks and the other entities together in id order.
    pub id: u32,
}

/// `FallingBlockEntity.fall`: the entity is made (taking the next entity id), the cell becomes
/// air (flags 3), then the entity joins the entity list.
pub fn fall(w: &mut World, block: Block, p: BlockPos) {
    let id = w.entities.next_id;
    w.entities.next_id += 1;
    let e = FallingBlock { block, x: p.x as f64 + 0.5, y: p.y as f64, z: p.z as f64 + 0.5, vy: 0.0, time: 0, removed: false, id };
    w.set_block_and_update(p, Block::Air);
    w.gravity.entities.push(e);
}

/// Falling-block entities, ticked in insertion order in the entity phase.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct State {
    pub entities: Vec<FallingBlock>,
}

impl State {
    /// The live falling blocks for [`crate::world::World::state_key`] (floats by their bits).
    pub(crate) fn key_into<H: std::hash::Hasher>(&self, h: &mut H) {
        use std::hash::Hash;
        for e in self.entities.iter().filter(|e| !e.removed) {
            (e.block, e.x.to_bits(), e.y.to_bits(), e.z.to_bits(), e.vy.to_bits(), e.time).hash(h);
        }
        0xfeu8.hash(h);
    }
}

pub fn is_idle(w: &World) -> bool {
    w.gravity.entities.iter().all(|e| e.removed)
}

/// `FallingBlock.isFree`: air, fire, liquid or replaceable.
pub fn is_free(b: Block) -> bool {
    b.is_air() || bh::can_be_replaced(&b)
}

/// `FallingBlock.onPlace`.
pub fn on_place(w: &mut World, state: Block, p: BlockPos, _old: Block, _moved: bool) {
    w.schedule_tick(p, state.kind(), 2, crate::ticks::Priority::Normal);
}

/// `FallingBlock.updateShape`.
pub fn update_shape(w: &mut World, state: Block, p: BlockPos, _dir: Dir, _np: BlockPos, _ns: Block) -> Block {
    w.schedule_tick(p, state.kind(), 2, crate::ticks::Priority::Normal);
    state
}

/// `FallingBlock.tick`: `FallingBlockEntity.fall` when the cell below is free.
pub fn tick(w: &mut World, state: Block, p: BlockPos) {
    if is_free(w.get(p.below())) && p.y >= MIN_Y {
        fall(w, state, p);
    }
}

const PX: f64 = 1.0 / 16.0;

/// Vertical extents of a block's collision boxes (empty collision context) that overlap the
/// column of a falling block centered in the cell (x, z in 0.01..0.99).
fn context_free_boxes(b: &Block) -> Vec<(f64, f64)> {
    bh::collision_boxes(b)
        .into_iter()
        .filter(|c| (c[1] as f64) > 0.16 && (c[0] as f64) < 15.84 && (c[5] as f64) > 0.16 && (c[4] as f64) < 15.84)
        .map(|c| (c[2] as f64 * PX, c[3] as f64 * PX))
        .collect()
}

/// The vertical extents (relative to the cell) of the parts of a block's collision shape that
/// overlap the column of a falling block centered in it (x, z in 0.01..0.99), for an entity whose
/// bottom is at `e_bottom` (scaffolding depends on it).
fn shape_boxes(w: &World, b: Block, p: BlockPos, e_bottom: f64) -> Vec<(f64, f64)> {
    match b {
        Block::Air => vec![],
        Block::Piston { extended: false, .. } => vec![(0.0, 1.0)],
        Block::Piston { facing, extended: true, .. } => match facing {
            Dir::Up => vec![(0.0, 12.0 * PX)],
            Dir::Down => vec![(4.0 * PX, 1.0)],
            _ => vec![(0.0, 1.0)],
        },
        Block::PistonHead { facing, short, .. } => head_boxes(facing, short),
        Block::MovingPiston { .. } => moving_boxes(w, p),
        Block::Scaffolding { .. } => crate::redstone::blocks::scaffolding::entity_boxes(&b, p.y, e_bottom)
            .into_iter()
            .map(|c| (c[2] as f64 * PX, c[3] as f64 * PX))
            .collect(),
        // every other block: its collision shape (honey 15 px, soul sand 14 px, slabs,
        // trapdoors, fence gates 24 px, ...)
        _ => context_free_boxes(&b),
    }
}

/// `PistonHeadBlock` shapes: the 4 px plate at the front, the 4x4 arm behind it (4 px longer
/// than the cell unless short).
fn head_boxes(facing: Dir, short: bool) -> Vec<(f64, f64)> {
    let arm = if short { 16.0 } else { 20.0 } * PX;
    match facing {
        Dir::Up => vec![(12.0 * PX, 1.0), (12.0 * PX - (arm - 4.0 * PX), 12.0 * PX)],
        Dir::Down => vec![(0.0, 4.0 * PX), (4.0 * PX, arm)],
        // sideways: the plate spans the full height; the arm (6..10 px) is inside it vertically
        _ => vec![(0.0, 1.0)],
    }
}

/// `PistonMovingBlockEntity.getCollisionShape` (no-clip direction never set here).
fn moving_boxes(w: &World, p: BlockPos) -> Vec<(f64, f64)> {
    let Some(be) = w.piston.be(p) else { return vec![] };
    let prog = be.progress as f64 / 2.0;
    let mut out = Vec::new();
    // a retracting source piston keeps its extended base shape
    if !be.extending && be.source {
        if let Block::Piston { facing, .. } = be.moved {
            out.extend(match facing {
                Dir::Up => vec![(0.0, 12.0 * PX)],
                Dir::Down => vec![(4.0 * PX, 1.0)],
                _ => vec![(0.0, 1.0)],
            });
        }
    }
    let moved = if be.source {
        let short = be.extending != (1.0 - prog < 0.25);
        Block::PistonHead { sticky: false, facing: be.direction, short }
    } else {
        be.moved
    };
    let ext = if be.extending { prog - 1.0 } else { 1.0 - prog };
    let dy = be.direction.step()[1] as f64 * ext;
    // the moved state's collision shape with the empty context (no entity)
    let boxes = match moved {
        Block::PistonHead { facing, short, .. } => head_boxes(facing, short),
        Block::Piston { extended: false, .. } => vec![(0.0, 1.0)],
        Block::Piston { facing, extended: true, .. } => match facing {
            Dir::Up => vec![(0.0, 12.0 * PX)],
            Dir::Down => vec![(4.0 * PX, 1.0)],
            _ => vec![(0.0, 1.0)],
        },
        Block::MovingPiston { .. } => vec![],
        m => context_free_boxes(&m),
    };
    out.extend(boxes.into_iter().map(|(a, b)| (a + dy, b + dy)));
    out
}

/// `Entity.collide` for a pure vertical movement `dy` (the `VoxelShape.collide` tolerances).
/// `BlockCollisions` also visits the neighboring columns for blocks with large collision shapes:
/// a moving piston's shape (offset by its progress) can reach into the entity's column.
fn collide_y(w: &World, e: &FallingBlock, dy: f64) -> f64 {
    if dy.abs() < 1.0e-7 {
        return 0.0;
    }
    let bx = e.x.floor() as i32;
    let bz = e.z.floor() as i32;
    let (ex0, ex1, ez0, ez1) = (e.x - 0.49, e.x + 0.49, e.z - 0.49, e.z + 0.49);
    let (lo, hi) = if dy < 0.0 { (e.y + dy, e.y + 0.98) } else { (e.y, e.y + 0.98 + dy) };
    let mut allowed = dy;
    for by in (lo.floor() as i32 - 1)..=(hi.floor() as i32 + 1) {
        for (dx, dz) in [(0, 0), (-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
            let p = BlockPos::new(bx + dx, by, bz + dz);
            let b = w.get(p);
            let boxes: Vec<(f64, f64)> = if let Block::MovingPiston { .. } = b {
                crate::piston::shape::moving(w, p)
                    .into_iter()
                    .filter(|c| {
                        let c = c.0;
                        let (x0, x1) = (p.x as f64 + c[0] as f64 * PX, p.x as f64 + c[1] as f64 * PX);
                        let (z0, z1) = (p.z as f64 + c[4] as f64 * PX, p.z as f64 + c[5] as f64 * PX);
                        x0 < ex1 && x1 > ex0 && z0 < ez1 && z1 > ez0
                    })
                    .map(|c| (c.0[2] as f64 * PX, c.0[3] as f64 * PX))
                    .collect()
            } else if dx == 0 && dz == 0 {
                shape_boxes(w, b, p, e.y)
            } else {
                continue;
            };
            for (a, t) in boxes {
                let (bot, top) = (by as f64 + a, by as f64 + t);
                // BlockCollisions: only shapes touching the swept box
                if top <= lo || bot >= hi {
                    continue;
                }
                if dy < 0.0 {
                    if e.y + 1.0e-7 >= top {
                        let d = top - e.y;
                        if d <= 1.0e-7 {
                            allowed = allowed.max(d);
                        }
                    }
                } else if e.y + 0.98 - 1.0e-7 <= bot {
                    let d = bot - (e.y + 0.98);
                    if d >= -1.0e-7 {
                        allowed = allowed.min(d);
                    }
                }
                if allowed.abs() < 1.0e-7 {
                    return 0.0;
                }
            }
        }
    }
    allowed
}

/// The entity phase for falling blocks alone (`FallingBlockEntity.tick`); with other entities,
/// [`crate::entity::tick_phase`] interleaves them by id.
pub fn tick_entities(w: &mut World) {
    let n = w.gravity.entities.len();
    for i in 0..n {
        tick_one(w, i);
    }
    w.gravity.entities.retain(|e| !e.removed);
}

/// One falling block's `FallingBlockEntity.tick` (the caller drops the removed ones afterwards).
pub fn tick_one(w: &mut World, i: usize) {
    {
        let mut e = w.gravity.entities[i];
        if e.removed {
            return;
        }
        if e.block.is_air() {
            e.removed = true;
            w.gravity.entities[i] = e;
            return;
        }
        e.time += 1;
        e.vy += -0.04;
        // Entity.move(SELF)
        let dy = e.vy;
        let my = collide_y(w, &e, dy);
        if my * my > 1.0e-7 || dy * dy - my * my < 1.0e-7 {
            e.y += my;
        }
        let vertical_collision = dy != my;
        let on_ground = vertical_collision && dy < 0.0;
        if vertical_collision && dy.abs() > 0.0 {
            // restituteMovementAfterCollisions -> updateEntityMovementAfterFallOn of the block
            // under it: slime bounces (0.8 for non-living entities), the rest stops the fall
            let support = w.get(BlockPos::new(e.x.floor() as i32, (e.y - 0.2).floor() as i32, e.z.floor() as i32));
            if matches!(support, Block::Simple(Simple::SlimeBlock)) && e.vy < 0.0 {
                e.vy = -e.vy * 0.8;
            } else {
                e.vy = 0.0;
            }
        }
        let p = BlockPos::new(e.x.floor() as i32, e.y.floor() as i32, e.z.floor() as i32);
        if !on_ground {
            if (e.time > 100 && (p.y <= MIN_Y || p.y > MAX_Y)) || e.time > 600 {
                w.drops.push((p, e.block));
                e.removed = true;
            }
        } else {
            let current = w.get(p);
            e.vy *= -0.5;
            if !matches!(current, Block::MovingPiston { .. }) {
                let may_replace = is_free(current);
                let would_continue = is_free(w.get(p.below()));
                // canSurvive = blockState.canSurvive(level, pos) && !wouldContinueFalling
                let can_survive = bh::can_survive(w, &e.block, p) && !would_continue;
                e.removed = true;
                w.gravity.entities[i] = e;
                if may_replace && can_survive {
                    if !w.set_block_and_update(p, e.block) {
                        w.drops.push((p, e.block));
                    }
                } else {
                    w.drops.push((p, e.block));
                }
                return;
            }
        }
        e.vy *= 0.98f32 as f64;
        w.gravity.entities[i] = e;
    }
}
