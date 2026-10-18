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
//!   here gives it any), entities pushed by moving pistons, fluids.

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
}

/// Falling-block entities, ticked in insertion order in the entity phase.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct State {
    pub entities: Vec<FallingBlock>,
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
        let e = FallingBlock { block: state, x: p.x as f64 + 0.5, y: p.y as f64, z: p.z as f64 + 0.5, vy: 0.0, time: 0, removed: false };
        w.set_block_and_update(p, Block::Air);
        w.gravity.entities.push(e);
    }
}

const PX: f64 = 1.0 / 16.0;

/// The vertical extents (relative to the cell) of the parts of a block's collision shape that
/// overlap the column of a falling block centered in it (x, z in 0.01..0.99).
fn shape_boxes(w: &World, b: Block, p: BlockPos) -> Vec<(f64, f64)> {
    match b {
        Block::Air => vec![],
        Block::Simple(Simple::HoneyBlock) => vec![(0.0, 15.0 * PX)],
        Block::Simple(Simple::SoulSand) => vec![(0.0, 14.0 * PX)],
        Block::Piston { extended: false, .. } => vec![(0.0, 1.0)],
        Block::Piston { facing, extended: true, .. } => match facing {
            Dir::Up => vec![(0.0, 12.0 * PX)],
            Dir::Down => vec![(4.0 * PX, 1.0)],
            _ => vec![(0.0, 1.0)],
        },
        Block::PistonHead { facing, short, .. } => head_boxes(facing, short),
        Block::MovingPiston { .. } => moving_boxes(w, p),
        _ => {
            if bh::sturdy(w, &b, p, Dir::Up, bh::Support::Full) && bh::sturdy(w, &b, p, Dir::Down, bh::Support::Full) {
                vec![(0.0, 1.0)]
            } else {
                vec![]
            }
        }
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
        if let Block::Piston { sticky, facing, .. } = be.moved {
            out.extend(shape_boxes(w, Block::Piston { sticky, facing, extended: true }, p));
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
    let boxes = match moved {
        Block::PistonHead { facing, short, .. } => head_boxes(facing, short),
        Block::MovingPiston { .. } => vec![],
        m => shape_boxes(w, m, p),
    };
    out.extend(boxes.into_iter().map(|(a, b)| (a + dy, b + dy)));
    out
}

/// `Entity.collide` for a pure vertical movement `dy` (the `VoxelShape.collide` tolerances).
fn collide_y(w: &World, e: &FallingBlock, dy: f64) -> f64 {
    if dy.abs() < 1.0e-7 {
        return 0.0;
    }
    let bx = e.x.floor() as i32;
    let bz = e.z.floor() as i32;
    let (lo, hi) = if dy < 0.0 { (e.y + dy, e.y + 0.98) } else { (e.y, e.y + 0.98 + dy) };
    let mut allowed = dy;
    for by in (lo.floor() as i32 - 1)..=(hi.floor() as i32 + 1) {
        let p = BlockPos::new(bx, by, bz);
        let b = w.get(p);
        for (a, t) in shape_boxes(w, b, p) {
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
    allowed
}

/// The entity phase for falling blocks (`FallingBlockEntity.tick`).
pub fn tick_entities(w: &mut World) {
    let n = w.gravity.entities.len();
    for i in 0..n {
        let mut e = w.gravity.entities[i];
        if e.removed {
            continue;
        }
        if e.block.is_air() {
            e.removed = true;
            w.gravity.entities[i] = e;
            continue;
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
                e.removed = true;
                w.gravity.entities[i] = e;
                if may_replace && !would_continue {
                    if !w.set_block_and_update(p, e.block) {
                        w.drops.push((p, e.block));
                    }
                } else {
                    w.drops.push((p, e.block));
                }
                continue;
            }
        }
        e.vy *= 0.98f32 as f64;
        w.gravity.entities[i] = e;
    }
    w.gravity.entities.retain(|e| !e.removed);
}
