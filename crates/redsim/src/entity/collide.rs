//! The entity core's movement (clk-misc, filling clockinfra's M2 skeleton; review: clockinfra):
//! `Entity.move` (collide with the block collision shapes in the game's axis order, step-up, the
//! collision flags, the supporting block, fall damage, restitution after collisions, block speed
//! factor), the movement recording and `applyEffectsFromBlocks` / `checkInsideBlocks` (the blocks
//! an entity sweeps through, in `BlockGetter.forEachBlockIntersectedBetween`'s order), read in
//! the 26.3 source (`Entity`, `BlockCollisions`, `Shapes`, `VoxelShape`, `BlockGetter`,
//! `BlockPos`, `AABB`, `CollisionGetter`).
//!
//! Not modeled (each case records a model error or is noted): a block shape that an entity
//! already overlaps on a multi-box shape (the voxel scan of `VoxelShape.collideX` is replaced by
//! the per-box minimum, exact whenever the entity is outside the shape), entity-dependent
//! collision shapes (scaffolding, powder snow, moving pistons), the fall-distance raycast for a
//! move of a block or more while falling, movement emission (step sounds and their game events),
//! `stepOn` and the fire / freeze effects of blocks.

use super::{Aabb, Entity, MoverType, Vec3, mth};
use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;
use std::collections::HashSet;

/// One recorded move (`Entity.Movement`): from, to, and the delta asked for (its axis order
/// replays the collision order in `checkInsideBlocks`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Movement {
    pub from: Vec3,
    pub to: Vec3,
    pub axis_dependent: Option<Vec3>,
}

/// `Direction.axisStepOrder(v)`: Y first, then the larger of X and Z last.
pub fn axis_step_order(v: Vec3) -> [usize; 3] {
    if v.x.abs() < v.z.abs() { [1, 2, 0] } else { [1, 0, 2] }
}

/// The other two axes of `a` in `AxisCycle` order (x -> y, z; y -> z, x; z -> x, y).
fn others(a: usize) -> (usize, usize) {
    match a {
        0 => (1, 2),
        1 => (2, 0),
        _ => (0, 1),
    }
}

/// `VoxelShape.collideX` of one block's shape (its boxes, moved to the block) along `a`.
fn shape_collide(a: usize, moving: Aabb, shape: &[Aabb], mut distance: f64) -> f64 {
    if shape.is_empty() {
        return distance;
    }
    if distance.abs() < 1.0e-7 {
        return 0.0;
    }
    let (b, c) = others(a);
    let overlaps = |bx: &Aabb| {
        moving.max(b) - 1.0e-7 >= bx.min(b) && moving.min(b) + 1.0e-7 < bx.max(b) && moving.max(c) - 1.0e-7 >= bx.min(c) && moving.min(c) + 1.0e-7 < bx.max(c)
    };
    if distance > 0.0 {
        let max_a = moving.max(a);
        for bx in shape.iter().filter(|bx| overlaps(bx)) {
            if max_a - 1.0e-7 < bx.min(a) {
                distance = super::vec3::jmin(distance, bx.min(a) - max_a);
            }
        }
    } else if distance < 0.0 {
        let min_a = moving.min(a);
        for bx in shape.iter().filter(|bx| overlaps(bx)) {
            if min_a + 1.0e-7 >= bx.max(a) {
                distance = super::vec3::jmax(distance, bx.max(a) - min_a);
            }
        }
    }
    distance
}

/// `Shapes.collide(axis, moving, shapes, distance)`.
fn shapes_collide(a: usize, moving: Aabb, shapes: &[Vec<Aabb>], mut distance: f64) -> f64 {
    for s in shapes {
        if distance.abs() < 1.0e-7 {
            return 0.0;
        }
        distance = shape_collide(a, moving, s, distance);
    }
    distance
}

/// `Entity.collideWithShapes`.
fn collide_with_shapes(movement: Vec3, bb: Aabb, shapes: &[Vec<Aabb>]) -> Vec3 {
    if shapes.is_empty() {
        return movement;
    }
    let mut resolved = Vec3::ZERO;
    for axis in axis_step_order(movement) {
        let m = movement.get(axis);
        if m != 0.0 {
            let c = shapes_collide(axis, bb.move_v(resolved), shapes, m);
            resolved = resolved.with_axis(axis, c);
        }
    }
    resolved
}

/// `BlockStateBase.hasLargeCollisionShape`: a collision shape leaving the cell, or no shape
/// cache (dynamic shapes).
fn has_large_collision_shape(s: &Block) -> bool {
    matches!(s, Block::FenceGate { open: false, .. } | Block::MovingPiston { .. } | Block::Scaffolding { .. })
}

/// One block's collision boxes moved to `p`.
fn boxes_at(s: &Block, p: BlockPos) -> Vec<Aabb> {
    let f = |v: i32| v as f64 / 16.0;
    let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
    crate::behavior::collision_boxes(s).iter().map(|c| Aabb::new(x + f(c[0]), y + f(c[2]), z + f(c[4]), x + f(c[1]), y + f(c[3]), z + f(c[5]))).collect()
}

/// `BlockCollisions` over `bb` (the entity collision context): every block shape that meets
/// `bb`, with its position (the cursor skips corner cells, face cells unless the block's shape
/// leaves its cell, edge cells unless it is a moving piston).
pub fn block_collisions(w: &World, bb: Aabb) -> Vec<(BlockPos, Vec<Aabb>)> {
    let x0 = mth::floor(bb.min_x - 1.0e-7) - 1;
    let x1 = mth::floor(bb.max_x + 1.0e-7) + 1;
    let y0 = mth::floor(bb.min_y - 1.0e-7) - 1;
    let y1 = mth::floor(bb.max_y + 1.0e-7) + 1;
    let z0 = mth::floor(bb.min_z - 1.0e-7) - 1;
    let z1 = mth::floor(bb.max_z + 1.0e-7) + 1;
    let mut out = Vec::new();
    for z in z0..=z1 {
        for y in y0..=y1 {
            for x in x0..=x1 {
                let face = (x == x0 || x == x1) as u8 + (y == y0 || y == y1) as u8 + (z == z0 || z == z1) as u8;
                if face == 3 {
                    continue;
                }
                let p = BlockPos::new(x, y, z);
                let s = w.get(p);
                if s.is_air() || (face == 1 && !has_large_collision_shape(&s)) || (face == 2 && !matches!(s, Block::MovingPiston { .. })) {
                    continue;
                }
                let boxes = boxes_at(&s, p);
                if boxes.is_empty() {
                    continue;
                }
                if boxes.iter().any(|b| b.intersects(bb)) {
                    out.push((p, boxes));
                }
            }
        }
    }
    out
}

/// `Level.getEntityCollisions(source, box)`: the boxes of the entities `source` collides with
/// (boats: `canVehicleCollide`; other kinds: entities that `canBeCollidedWith` it, i.e. boats).
pub fn entity_collisions(w: &World, i: usize, bb: Aabb) -> Vec<Vec<Aabb>> {
    if w.entities.list.len() < 2 || bb.x_size() * bb.y_size() * bb.z_size() < 1.0e-7 {
        return Vec::new();
    }
    let query = bb.inflate(1.0e-7, 1.0e-7, 1.0e-7);
    w.get_entities(query, |e| matches!(e.kind, super::EntityKind::Boat(_)))
        .into_iter()
        .filter(|&j| j != i)
        .map(|j| vec![w.entities.list[j].bb])
        .collect()
}

/// `Entity.collectCandidateStepUpHeights` (each shape's Y coordinates: its boxes' bottoms and
/// tops).
fn candidate_step_up_heights(bb: Aabb, colliders: &[Vec<Aabb>], max_step: f32, skip: f32) -> Vec<f32> {
    let mut cands: Vec<f32> = Vec::new();
    for c in colliders {
        let mut coords: Vec<f64> = c.iter().flat_map(|b| [b.min_y, b.max_y]).collect();
        coords.sort_by(|a, b| a.partial_cmp(b).unwrap());
        coords.dedup();
        for coord in coords {
            let rel = (coord - bb.min_y) as f32;
            if !(rel < 0.0) && rel != skip {
                if rel > max_step {
                    break;
                }
                if !cands.contains(&rel) {
                    cands.push(rel);
                }
            }
        }
    }
    cands.sort_by(|a, b| a.partial_cmp(b).unwrap());
    cands
}

/// `Entity.collide(movement)`.
pub fn collide(w: &World, i: usize, movement: Vec3) -> Vec3 {
    let e = &w.entities.list[i];
    let bb = e.bb;
    let max_up = super::kinds::max_up_step(w, i);
    let entity_colliders = entity_collisions(w, i, bb.expand_towards(movement.x, movement.y, movement.z).expand_towards(0.0, max_up as f64, 0.0));
    let step = if movement.length_sqr() == 0.0 {
        movement
    } else {
        let mut colliders = entity_colliders.clone();
        colliders.extend(block_collisions(w, bb.expand_towards(movement.x, movement.y, movement.z)).into_iter().map(|(_, s)| s));
        collide_with_shapes(movement, bb, &colliders)
    };
    let x_col = movement.x != step.x;
    let y_col = movement.y != step.y;
    let z_col = movement.z != step.z;
    let on_ground_after = y_col && movement.y < 0.0;
    if max_up > 0.0 && (on_ground_after || e.on_ground) && (x_col || z_col) {
        let grounded = if on_ground_after { bb.move_by(0.0, step.y, 0.0) } else { bb };
        let mut step_up = grounded.expand_towards(movement.x, max_up as f64, movement.z);
        if !on_ground_after {
            step_up = step_up.expand_towards(0.0, -1.0e-5f32 as f64, 0.0);
        }
        let mut colliders = entity_colliders.clone();
        colliders.extend(block_collisions(w, step_up).into_iter().map(|(_, s)| s));
        let skip = step.y as f32;
        for h in candidate_step_up_heights(grounded, &colliders, max_up, skip) {
            let from_ground = collide_with_shapes(Vec3::new(movement.x, h as f64, movement.z), grounded, &colliders);
            if from_ground.horizontal_distance_sqr() > step.horizontal_distance_sqr() {
                let d = bb.min_y - grounded.min_y;
                return from_ground.subtract(0.0, d, 0.0);
            }
        }
    }
    step
}

/// `CollisionGetter.findSupportingBlock`: the colliding block nearest the entity's position
/// (ties: the greater position by `Vec3i.compareTo`: y, then z, then x).
fn find_supporting_block(w: &World, pos: Vec3, bb: Aabb) -> Option<BlockPos> {
    let mut best: Option<BlockPos> = None;
    let mut best_d = f64::MAX;
    for (p, _) in block_collisions(w, bb) {
        let (dx, dy, dz) = (p.x as f64 + 0.5 - pos.x, p.y as f64 + 0.5 - pos.y, p.z as f64 + 0.5 - pos.z);
        let d = dx * dx + dy * dy + dz * dz;
        let greater = |a: BlockPos, b: BlockPos| (a.y, a.z, a.x) < (b.y, b.z, b.x);
        if d < best_d || (d == best_d && best.is_none_or(|m| greater(m, p))) {
            best = Some(p);
            best_d = d;
        }
    }
    best
}

/// `Entity.checkSupportingBlock`.
fn check_supporting_block(w: &mut World, i: usize, on_ground: bool, movement: Option<Vec3>) {
    if on_ground {
        let e = &w.entities.list[i];
        let bb = e.bb;
        let test = Aabb::new(bb.min_x, bb.min_y - 1.0e-6, bb.min_z, bb.max_x, bb.min_y, bb.max_z);
        let mut support = find_supporting_block(w, e.pos, test);
        if support.is_some() || e.on_ground_no_blocks {
            w.entities.list[i].main_support = support;
        } else if let Some(m) = movement {
            support = find_supporting_block(w, e.pos, test.move_by(-m.x, 0.0, -m.z));
            w.entities.list[i].main_support = support;
        }
        w.entities.list[i].on_ground_no_blocks = support.is_none();
    } else {
        let e = &mut w.entities.list[i];
        e.on_ground_no_blocks = false;
        e.main_support = None;
    }
}

/// `Entity.getOnPos(offset)`.
pub fn get_on_pos(w: &World, e: &Entity, offset: f32) -> BlockPos {
    if let Some(p) = e.main_support {
        if !(offset > 1.0e-5) {
            return p;
        }
        let below = w.get(p);
        // fences and walls are not modeled; fence gates keep the support
        if matches!(below, Block::FenceGate { .. }) {
            return p;
        }
        return BlockPos::new(p.x, mth::floor(e.pos.y - offset as f64), p.z);
    }
    BlockPos::new(mth::floor(e.pos.x), mth::floor(e.pos.y - offset as f64), mth::floor(e.pos.z))
}

/// `Block.getSpeedFactor`.
fn speed_factor(s: &Block) -> f32 {
    use crate::block::Simple;
    match *s {
        Block::Simple(Simple::SoulSand) | Block::Simple(Simple::HoneyBlock) => 0.4,
        _ => 1.0,
    }
}

/// `state.is(Blocks.WATER) || state.is(Blocks.BUBBLE_COLUMN)` (the fluid blocks of M1).
fn is_water_or_bubble_column(_s: &Block) -> bool {
    false
}

/// `Entity.getBlockSpeedFactor`.
fn block_speed_factor(w: &World, i: usize) -> f32 {
    let e = &w.entities.list[i];
    let here = w.get(e.block_position());
    let f = speed_factor(&here);
    if !is_water_or_bubble_column(&here) {
        if f == 1.0 { speed_factor(&w.get(get_on_pos(w, e, 0.500001))) } else { f }
    } else {
        f
    }
}

/// `Entity.restituteMovementAfterCollisions` (no modeled block bounces: restitution 0, so a
/// collided axis keeps `-v * 0`, a signed zero).
fn restitute(w: &mut World, i: usize, x_col: bool, z_col: bool, movement: Vec3) {
    let e = &mut w.entities.list[i];
    let restitution = 0.0f64;
    let cur = e.delta_movement;
    let mut after = cur;
    if x_col {
        after = after.with_axis(0, -cur.x * restitution);
    }
    if z_col {
        after = after.with_axis(2, -cur.z * restitution);
    }
    if e.vertical_collision {
        let _ = movement;
        let (gravity_comp, drag) = (0.0f64, 1.0f64);
        after = after.with_axis(1, (gravity_comp - cur.y) * drag * restitution);
    }
    e.delta_movement = after;
}

/// `Entity.move(moverType, delta)` of entity `i`.
pub fn move_(w: &mut World, i: usize, mover: MoverType, delta: Vec3) {
    if w.entities.list[i].no_physics {
        let e = &mut w.entities.list[i];
        let p = e.pos.add_v(delta);
        e.set_pos(p);
        e.horizontal_collision = false;
        e.vertical_collision = false;
        e.vertical_collision_below = false;
        e.minor_horizontal_collision = false;
        return;
    }
    let mut delta = delta;
    if mover == MoverType::Piston {
        w.errors.push("entities pushed by pistons are not modeled yet".into());
        return;
    }
    if w.entities.list[i].stuck_speed.length_sqr() > 1.0e-7 {
        let e = &mut w.entities.list[i];
        delta = delta.multiply(e.stuck_speed.x, e.stuck_speed.y, e.stuck_speed.z);
        e.stuck_speed = Vec3::ZERO;
        e.delta_movement = Vec3::ZERO;
    }
    let movement = collide(w, i, delta);
    let len = movement.length_sqr();
    if len > 1.0e-7 || delta.length_sqr() - len < 1.0e-7 {
        if w.entities.list[i].fall_distance != 0.0 && len >= 1.0 {
            w.errors.push("entity: the fall-distance raycast of a long move is not modeled".into());
        }
        let e = &mut w.entities.list[i];
        let pos = e.pos;
        let np = pos.add_v(movement);
        add_movement_this_tick(e, Movement { from: pos, to: np, axis_dependent: Some(delta) });
        e.set_pos(np);
    }
    let x_col = !mth::equal(delta.x, movement.x);
    let z_col = !mth::equal(delta.z, movement.z);
    let moved_vertically = delta.y.abs() > 0.0;
    {
        let e = &mut w.entities.list[i];
        e.horizontal_collision = x_col || z_col;
        // `movedVertically || isLocalInstanceAuthoritative()`: always true on the server
        e.vertical_collision = delta.y != movement.y;
        e.vertical_collision_below = e.vertical_collision && delta.y < 0.0;
        e.on_ground = e.vertical_collision_below;
    }
    let below_hit = w.entities.list[i].vertical_collision_below;
    check_supporting_block(w, i, below_hit, Some(movement));
    w.entities.list[i].minor_horizontal_collision = false;
    let on_ground = w.entities.list[i].on_ground;
    super::kinds::check_fall_damage(w, i, movement.y, on_ground);
    if w.entities.list[i].removed {
        return;
    }
    let (vc, hc) = (w.entities.list[i].vertical_collision, w.entities.list[i].horizontal_collision);
    if (moved_vertically && vc) || hc {
        restitute(w, i, x_col, z_col, movement);
    }
    let f = block_speed_factor(w, i) as f64;
    let e = &mut w.entities.list[i];
    e.delta_movement = e.delta_movement.multiply(f, 1.0, f);
}

/// `Entity.addMovementThisTick`.
fn add_movement_this_tick(e: &mut Entity, m: Movement) {
    if e.movement_this_tick.len() >= 100 {
        let first = e.movement_this_tick.remove(0);
        let second = e.movement_this_tick.remove(0);
        e.movement_this_tick.insert(0, Movement { from: first.from, to: second.to, axis_dependent: None });
    }
    e.movement_this_tick.push(m);
}

// ---------------------------------------------------------------------------- blocks swept

/// `AABB.contains(Vec3)`.
fn contains(b: Aabb, v: Vec3) -> bool {
    v.x >= b.min_x && v.x < b.max_x && v.y >= b.min_y && v.y < b.max_y && v.z >= b.min_z && v.z < b.max_z
}

/// `AABB.clipPoint`.
#[allow(clippy::too_many_arguments)]
fn clip_point(scale: &mut f64, dir: Option<u8>, da: f64, db: f64, dc: f64, point: f64, min_b: f64, max_b: f64, min_c: f64, max_c: f64, nd: u8, fa: f64, fb: f64, fc: f64) -> Option<u8> {
    let s = (point - fa) / da;
    let pb = fb + s * db;
    let pc = fc + s * dc;
    if 0.0 < s && s < *scale && min_b - 1.0e-7 < pb && pb < max_b + 1.0e-7 && min_c - 1.0e-7 < pc && pc < max_c + 1.0e-7 {
        *scale = s;
        Some(nd)
    } else {
        dir
    }
}

/// `AABB.clip(minX, ..., maxZ, from, to)`: the first hit point, if any.
pub fn clip(b: Aabb, from: Vec3, to: Vec3) -> Option<Vec3> {
    let mut scale = 1.0f64;
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    let mut dir = None;
    if dx > 1.0e-7 {
        dir = clip_point(&mut scale, dir, dx, dy, dz, b.min_x, b.min_y, b.max_y, b.min_z, b.max_z, 0, from.x, from.y, from.z);
    } else if dx < -1.0e-7 {
        dir = clip_point(&mut scale, dir, dx, dy, dz, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z, 1, from.x, from.y, from.z);
    }
    if dy > 1.0e-7 {
        dir = clip_point(&mut scale, dir, dy, dz, dx, b.min_y, b.min_z, b.max_z, b.min_x, b.max_x, 2, from.y, from.z, from.x);
    } else if dy < -1.0e-7 {
        dir = clip_point(&mut scale, dir, dy, dz, dx, b.max_y, b.min_z, b.max_z, b.min_x, b.max_x, 3, from.y, from.z, from.x);
    }
    if dz > 1.0e-7 {
        dir = clip_point(&mut scale, dir, dz, dx, dy, b.min_z, b.min_x, b.max_x, b.min_y, b.max_y, 4, from.z, from.x, from.y);
    } else if dz < -1.0e-7 {
        dir = clip_point(&mut scale, dir, dz, dx, dy, b.max_z, b.min_x, b.max_x, b.min_y, b.max_y, 5, from.z, from.x, from.y);
    }
    dir.map(|_| from.add(scale * dx, scale * dy, scale * dz))
}

/// `AABB.collidedAlongVector`.
fn collided_along_vector(b: Aabb, vector: Vec3, parts: &[Aabb]) -> bool {
    let from = b.center();
    let to = from.add_v(vector);
    for part in parts {
        let inflated = part.inflate(b.x_size() * 0.5 - 1.0e-7, b.y_size() * 0.5 - 1.0e-7, b.z_size() * 0.5 - 1.0e-7);
        if contains(inflated, to) || contains(inflated, from) {
            return true;
        }
        if clip(inflated, from, to).is_some() {
            return true;
        }
    }
    false
}

/// `BlockPos.betweenClosed(minX..maxZ)`: x fastest, then y, then z.
fn between_closed(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for z in z0..=z1 {
        for y in y0..=y1 {
            for x in x0..=x1 {
                v.push(BlockPos::new(x, y, z));
            }
        }
    }
    v
}

/// `BlockPos.betweenCornersInDirection(corners, direction)`.
fn between_corners_in_direction(a: [i32; 3], b: [i32; 3], dir: Vec3) -> Vec<BlockPos> {
    let mn = [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])];
    let mx = [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])];
    let diff = [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]];
    let start = [if dir.x >= 0.0 { mn[0] } else { mx[0] }, if dir.y >= 0.0 { mn[1] } else { mx[1] }, if dir.z >= 0.0 { mn[2] } else { mx[2] }];
    let axes = axis_step_order(dir);
    let step = |ax: usize| -> [i32; 3] {
        let s = if dir.get(ax) >= 0.0 { 1 } else { -1 };
        let mut v = [0; 3];
        v[ax] = s;
        v
    };
    let (d1, d2, d3) = (step(axes[0]), step(axes[1]), step(axes[2]));
    let (m1, m2, m3) = (diff[axes[0]], diff[axes[1]], diff[axes[2]]);
    let mut out = Vec::new();
    for i1 in 0..=m1 {
        for i2 in 0..=m2 {
            for i3 in 0..=m3 {
                out.push(BlockPos::new(
                    start[0] + d1[0] * i1 + d2[0] * i2 + d3[0] * i3,
                    start[1] + d1[1] * i1 + d2[1] * i2 + d3[1] * i3,
                    start[2] + d1[2] * i1 + d2[2] * i2 + d3[2] * i3,
                ));
            }
        }
    }
    out
}

fn floor_corners(b: Aabb) -> ([i32; 3], [i32; 3]) {
    ([mth::floor(b.min_x), mth::floor(b.min_y), mth::floor(b.min_z)], [mth::floor(b.max_x), mth::floor(b.max_y), mth::floor(b.max_z)])
}

/// `BlockGetter.getFurthestCorner` (verbatim, its component order included).
fn furthest_corner(d: Vec3) -> [i32; 3] {
    let (xd, yd, zd) = (d.x.abs(), d.y.abs(), d.z.abs());
    let xs = if d.x >= 0.0 { 1 } else { -1 };
    let ys = if d.y >= 0.0 { 1 } else { -1 };
    let zs = if d.z >= 0.0 { 1 } else { -1 };
    if xd <= yd && xd <= zd {
        [-xs, -zs, ys]
    } else if yd <= zd {
        [zs, -ys, -xs]
    } else {
        [-ys, xs, -zs]
    }
}

/// `BlockGetter.addCollisionsAlongTravel`; -1 when the visitor stopped.
fn add_collisions_along_travel(visited: &mut HashSet<BlockPos>, delta: Vec3, target: Aabb, visit: &mut dyn FnMut(BlockPos, i32) -> bool) -> i32 {
    let (sx, sy, sz) = (target.x_size(), target.y_size(), target.z_size());
    let cd = furthest_corner(delta);
    let c = target.center();
    let to_corner = Vec3::new(c.x + sx * 0.5 * cd[0] as f64, c.y + sy * 0.5 * cd[1] as f64, c.z + sz * 0.5 * cd[2] as f64);
    let from_corner = to_corner.subtract_v(delta);
    let (mut bx, mut by, mut bz) = (mth::floor(from_corner.x), mth::floor(from_corner.y), mth::floor(from_corner.z));
    let (gx, gy, gz) = (mth::sign(delta.x), mth::sign(delta.y), mth::sign(delta.z));
    let td = |g: i32, d: f64| if g == 0 { f64::MAX } else { g as f64 / d };
    let (tdx, tdy, tdz) = (td(gx, delta.x), td(gy, delta.y), td(gz, delta.z));
    let t0 = |g: i32, tdv: f64, v: f64| tdv * if g > 0 { 1.0 - mth::frac(v) } else { mth::frac(v) };
    let (mut tx, mut ty, mut tz) = (t0(gx, tdx, from_corner.x), t0(gy, tdy, from_corner.y), t0(gz, tdz, from_corner.z));
    let mut iterations = 0;
    while tx <= 1.0 || ty <= 1.0 || tz <= 1.0 {
        if tx < ty {
            if tx < tz {
                bx += gx;
                tx += tdx;
            } else {
                bz += gz;
                tz += tdz;
            }
        } else if ty < tz {
            by += gy;
            ty += tdy;
        } else {
            bz += gz;
            tz += tdz;
        }
        let cell = Aabb::new(bx as f64, by as f64, bz as f64, (bx + 1) as f64, (by + 1) as f64, (bz + 1) as f64);
        if let Some(hit) = clip(cell, from_corner, to_corner) {
            iterations += 1;
            let lo = |b: i32| (b as f32 + 1.0e-5f32) as f64;
            let hi = |b: i32| b as f64 + 1.0 - 1.0e-5f32 as f64;
            let hx = mth::clamp(hit.x, lo(bx), hi(bx));
            let hy = mth::clamp(hit.y, lo(by), hi(by));
            let hz = mth::clamp(hit.z, lo(bz), hi(bz));
            let ox = mth::floor(hx - sx * cd[0] as f64);
            let oy = mth::floor(hy - sy * cd[1] as f64);
            let oz = mth::floor(hz - sz * cd[2] as f64);
            for p in between_corners_in_direction([bx, by, bz], [ox, oy, oz], delta) {
                if visited.insert(p) && !visit(p, iterations) {
                    return -1;
                }
            }
        }
    }
    iterations
}

/// `BlockGetter.forEachBlockIntersectedBetween`.
fn for_each_block_intersected_between(from: Vec3, to: Vec3, target: Aabb, visit: &mut dyn FnMut(BlockPos, i32) -> bool) -> bool {
    let travel = to.subtract_v(from);
    if travel.length_sqr() < (1.0e-5f32 * 1.0e-5f32) as f64 {
        let lo = [mth::floor(target.min_x), mth::floor(target.min_y), mth::floor(target.min_z)];
        let hi = [mth::floor(target.max_x), mth::floor(target.max_y), mth::floor(target.max_z)];
        for p in between_closed(lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]) {
            if !visit(p, 0) {
                return false;
            }
        }
        return true;
    }
    let mut visited = HashSet::new();
    let back = target.move_v(travel.scale(-1.0));
    let (a, b) = floor_corners(back);
    for p in between_corners_in_direction(a, b, travel) {
        if !visit(p, 0) {
            return false;
        }
        visited.insert(p);
    }
    let iterations = add_collisions_along_travel(&mut visited, travel, target, visit);
    if iterations < 0 {
        return false;
    }
    let (a, b) = floor_corners(target);
    for p in between_corners_in_direction(a, b, travel) {
        if visited.insert(p) && !visit(p, iterations + 1) {
            return false;
        }
    }
    true
}

/// `Entity.collidedWithShapeMovingFrom`.
fn collided_with_shape_moving_from(e: &Entity, from: Vec3, to: Vec3, parts: &[Aabb]) -> bool {
    let (wd, ht) = e.kind.dims();
    let at_from = super::make_bounding_box(from, wd, ht);
    collided_along_vector(at_from, to.subtract_v(from), parts)
}

/// `Entity.checkInsideBlocks(from, to, collector, visited, maxIterations)`: the iterations used.
fn check_inside_blocks_step(w: &mut World, i: usize, from: Vec3, to: Vec3, visited: &mut HashSet<BlockPos>, max_iter: i32) -> i32 {
    let (wd, ht) = w.entities.list[i].kind.dims();
    let d = 1.0e-5f32 as f64;
    let target = super::make_bounding_box(to, wd, ht).deflate(d, d, d);
    let far = 0.9999900000002526f64;
    let moved_far = from.distance_to_sqr(to) > far * far;
    let mut iterations = 0;
    // the sweep reads no world state: list its visits first, then run them in order (a visit
    // returning false stops the sweep, as in the game)
    let mut seq: Vec<(BlockPos, i32)> = Vec::new();
    for_each_block_intersected_between(from, to, target, &mut |p, it| {
        seq.push((p, it));
        true
    });
    for (p, iteration) in seq {
        if w.entities.list[i].removed {
            break;
        }
        if iteration >= max_iter {
            break;
        }
        iterations = iteration;
        let s = w.get(p);
        if s.is_air() {
            continue;
        }
        let e = &w.entities.list[i];
        let inside_block = match crate::react::entity_inside_shape(&s) {
            None => true,
            Some(b) => {
                let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
                collided_with_shape_moving_from(e, from, to, &[Aabb::new(b[0] + x, b[1] + y, b[2] + z, b[3] + x, b[4] + y, b[5] + z)])
            }
        };
        let fs = crate::fluid::fluid_state(w, p);
        let inside_fluid = !fs.is_empty() && {
            let h = fs.height(w, p) as f64;
            collided_with_shape_moving_from(e, from, to, &[Aabb::new(p.x as f64, p.y as f64, p.z as f64, (p.x + 1) as f64, p.y as f64 + h, (p.z + 1) as f64)])
        };
        if (inside_block || inside_fluid) && visited.insert(p) && inside_block {
            let precise = moved_far || target.intersects(Aabb::of_block(p));
            crate::react::entity_inside(w, i, s, p, precise);
        }
    }
    iterations + 1
}

/// `Entity.checkInsideBlocks(movements, collector)`.
fn check_inside_blocks(w: &mut World, i: usize, moves: &[Movement]) {
    if w.entities.list[i].removed {
        return;
    }
    let mut visited = HashSet::new();
    for m in moves {
        let mut pos = m.from;
        let delta = m.to.subtract_v(m.from);
        let mut max_iter = 16;
        match m.axis_dependent {
            Some(orig) if delta.length_sqr() > 0.0 => {
                for axis in axis_step_order(orig) {
                    let am = delta.get(axis);
                    if am != 0.0 {
                        let to = pos.with_axis(axis, pos.get(axis) + am);
                        max_iter -= check_inside_blocks_step(w, i, pos, to, &mut visited, max_iter);
                        pos = to;
                    }
                }
            }
            _ => {
                max_iter -= check_inside_blocks_step(w, i, m.from, m.to, &mut visited, 16);
            }
        }
        if max_iter <= 0 {
            check_inside_blocks_step(w, i, m.to, m.to, &mut visited, 1);
        }
    }
}

/// `Entity.applyEffectsFromBlocks()`: this tick's recorded moves (else old -> current position),
/// replayed through the blocks they sweep.
pub fn apply_effects_from_blocks(w: &mut World, i: usize) {
    let moves = {
        let e = &mut w.entities.list[i];
        e.final_movements = std::mem::take(&mut e.movement_this_tick);
        let pos = e.pos;
        let old = Vec3::new(e.x_old, e.y_old, e.z_old);
        match e.final_movements.last().copied() {
            None => e.final_movements.push(Movement { from: old, to: pos, axis_dependent: None }),
            Some(last) if last.to.distance_to_sqr(pos) > 9.9999994e-11f32 as f64 => e.final_movements.push(Movement { from: last.to, to: pos, axis_dependent: None }),
            _ => {}
        }
        e.final_movements.clone()
    };
    if w.entities.list[i].removed || w.entities.list[i].no_physics {
        return;
    }
    check_inside_blocks(w, i, &moves);
}

/// `CollisionGetter.noCollision(entity, box)`: no block shape and no collidable entity in `bb`.
pub fn no_collision(w: &World, i: usize, bb: Aabb) -> bool {
    block_collisions(w, bb).is_empty() && entity_collisions(w, i, bb).is_empty()
}
