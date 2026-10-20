//! Minecarts (clk-misc, redsim M4): `AbstractMinecart` with `OldMinecartBehavior` (the default
//! movement, the experimental one is off), read in the 26.3 source; the hopper minecart's item
//! side (`MinecartHopper`) is not modeled yet.
//!
//! The tick, in the game's order (`AbstractMinecart.tick` does not run `Entity.baseTick`):
//! `checkBelowWorld`, `computeSpeed`, the behavior's tick (gravity; on a rail `moveAlongTrack`:
//! slope push, direction from the rail's exits, powered-rail halt, snap onto the rail's line,
//! `move` clamped to 0.4, slope exits, natural slowdown, height-change speed, corner
//! redirection, powered-rail boost or kick; off a rail `comeOffTrack`; then the blocks at its
//! position (`applyEffectsFromBlocks(pos, pos)`), the yaw from its motion with the flip rule),
//! then `updateFluidInteraction`.

use crate::block::{Block, RailShape};
use crate::entity::{self, MoverType, Vec3, mth};
use crate::pos::BlockPos;
use crate::rails;
use crate::world::World;

/// `EntityTypes.MINECART.sized(0.98F, 0.7F)` (hopper and chest minecarts too).
pub const WIDTH: f32 = 0.98;
pub const HEIGHT: f32 = 0.7;

/// The minecart's own fields.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MinecartState {
    /// A hopper minecart (`MinecartHopper`).
    pub hopper: bool,
    pub on_rails: bool,
    pub flipped: bool,
}

fn st_mut(w: &mut World, i: usize) -> &mut MinecartState {
    match &mut w.entities.list[i].kind {
        entity::EntityKind::Minecart(m) => m,
        _ => unreachable!("not a minecart"),
    }
}

fn st(w: &World, i: usize) -> MinecartState {
    match &w.entities.list[i].kind {
        entity::EntityKind::Minecart(m) => *m,
        _ => MinecartState::default(),
    }
}

fn in_water(w: &World, i: usize) -> bool {
    w.entities.list[i].was_touching_water
}

/// `getCurrentBlockPosOrRailBelow` (old movement).
fn current_block_pos_or_rail_below(w: &World, p: Vec3) -> BlockPos {
    let (xt, mut yt, zt) = (mth::floor(p.x), mth::floor(p.y), mth::floor(p.z));
    if rails::is_rail(&w.get(BlockPos::new(xt, yt - 1, zt))) {
        yt -= 1;
    }
    BlockPos::new(xt, yt, zt)
}

/// `OldMinecartBehavior.getPos(x, y, z)`: the point of the rail's line under (x, z), if on a rail.
fn get_pos(w: &World, x: f64, y: f64, z: f64) -> Option<Vec3> {
    let (xt, mut yt, zt) = (mth::floor(x), mth::floor(y), mth::floor(z));
    if rails::is_rail(&w.get(BlockPos::new(xt, yt - 1, zt))) {
        yt -= 1;
    }
    let s = w.get(BlockPos::new(xt, yt, zt));
    if !rails::is_rail(&s) {
        return None;
    }
    let (e0, e1) = rails::exits(rails::shape_of(&s));
    let x0 = xt as f64 + 0.5 + e0[0] as f64 * 0.5;
    let y0 = yt as f64 + 0.0625 + e0[1] as f64 * 0.5;
    let z0 = zt as f64 + 0.5 + e0[2] as f64 * 0.5;
    let x1 = xt as f64 + 0.5 + e1[0] as f64 * 0.5;
    let y1 = yt as f64 + 0.0625 + e1[1] as f64 * 0.5;
    let z1 = zt as f64 + 0.5 + e1[2] as f64 * 0.5;
    let xd = x1 - x0;
    let yd = (y1 - y0) * 2.0;
    let zd = z1 - z0;
    let progress = if xd == 0.0 {
        z - zt as f64
    } else if zd == 0.0 {
        x - xt as f64
    } else {
        let (xx, zz) = (x - x0, z - z0);
        (xx * xd + zz * zd) * 2.0
    };
    let x = x0 + xd * progress;
    let mut y = y0 + yd * progress;
    let z = z0 + zd * progress;
    if yd < 0.0 {
        y += 1.0;
    } else if yd > 0.0 {
        y += 0.5;
    }
    Some(Vec3::new(x, y, z))
}

fn set_pos(w: &mut World, i: usize, x: f64, y: f64, z: f64) {
    w.entities.list[i].set_pos(Vec3::new(x, y, z));
}

fn dm(w: &World, i: usize) -> Vec3 {
    w.entities.list[i].delta_movement
}

fn set_dm(w: &mut World, i: usize, v: Vec3) {
    w.entities.list[i].delta_movement = v;
}

/// `AbstractMinecart.move` (old movement): `Entity.move`, then the blocks at the new position.
fn cart_move(w: &mut World, i: usize, delta: Vec3) {
    entity::move_(w, i, MoverType::Self_, delta);
    apply_effects_from_blocks(w, i);
}

/// `AbstractMinecart.applyEffectsFromBlocks` (old movement): `applyEffectsFromBlocks(pos, pos)`
/// and the recorded moves dropped.
fn apply_effects_from_blocks(w: &mut World, i: usize) {
    let p = w.entities.list[i].pos;
    entity::collide::apply_effects_from_blocks_between(w, i, p, p);
    w.entities.list[i].movement_this_tick.clear();
}

fn is_conductor(w: &World, p: BlockPos) -> bool {
    crate::behavior::is_conductor(w, &w.get(p), p)
}

/// `OldMinecartBehavior.moveAlongTrack`.
fn move_along_track(w: &mut World, i: usize) {
    let p0 = w.entities.list[i].pos;
    let pos = current_block_pos_or_rail_below(w, p0);
    let s = w.get(pos);
    w.entities.list[i].fall_distance = 0.0;
    let (mut x, mut y, mut z) = (p0.x, p0.y, p0.z);
    let old_pos = get_pos(w, x, y, z);
    y = pos.y as f64;
    let (power_track, mut halt_track) = match s {
        Block::Rail { activator: false, powered, .. } => (powered, !powered),
        _ => (false, false),
    };
    let mut slide = 0.0078125;
    if in_water(w, i) {
        slide *= 0.2;
    }
    let m = dm(w, i);
    let shape = rails::shape_of(&s);
    match shape {
        RailShape::AscendingEast => {
            set_dm(w, i, m.add(-slide, 0.0, 0.0));
            y += 1.0;
        }
        RailShape::AscendingWest => {
            set_dm(w, i, m.add(slide, 0.0, 0.0));
            y += 1.0;
        }
        RailShape::AscendingNorth => {
            set_dm(w, i, m.add(0.0, 0.0, slide));
            y += 1.0;
        }
        RailShape::AscendingSouth => {
            set_dm(w, i, m.add(0.0, 0.0, -slide));
            y += 1.0;
        }
        _ => {}
    }
    let m = dm(w, i);
    let (e0, e1) = rails::exits(shape);
    let mut xd = (e1[0] - e0[0]) as f64;
    let mut zd = (e1[2] - e0[2]) as f64;
    let length = (xd * xd + zd * zd).sqrt();
    let flip = m.x * xd + m.z * zd;
    if flip < 0.0 {
        xd = -xd;
        zd = -zd;
    }
    let pow = entity::vec3::jmin(2.0, m.horizontal_distance());
    set_dm(w, i, Vec3::new(pow * xd / length, m.y, pow * zd / length));
    // (no passenger: no rider input)
    if halt_track {
        let speed = dm(w, i).horizontal_distance();
        if speed < 0.03 {
            set_dm(w, i, Vec3::ZERO);
        } else {
            let d = dm(w, i);
            set_dm(w, i, d.multiply(0.5, 0.0, 0.5));
        }
        halt_track = false;
    }
    let _ = halt_track;
    let x0 = pos.x as f64 + 0.5 + e0[0] as f64 * 0.5;
    let z0 = pos.z as f64 + 0.5 + e0[2] as f64 * 0.5;
    let x1 = pos.x as f64 + 0.5 + e1[0] as f64 * 0.5;
    let z1 = pos.z as f64 + 0.5 + e1[2] as f64 * 0.5;
    let xd = x1 - x0;
    let zd = z1 - z0;
    let progress = if xd == 0.0 {
        z - pos.z as f64
    } else if zd == 0.0 {
        x - pos.x as f64
    } else {
        let (xx, zz) = (x - x0, z - z0);
        (xx * xd + zz * zd) * 2.0
    };
    x = x0 + xd * progress;
    z = z0 + zd * progress;
    set_pos(w, i, x, y, z);
    let scale = 1.0; // `isVehicle() ? 0.75 : 1.0`: no passengers
    let max_speed = if in_water(w, i) { 0.2 } else { 0.4 };
    let m = dm(w, i);
    cart_move(w, i, Vec3::new(mth::clamp(scale * m.x, -max_speed, max_speed), 0.0, mth::clamp(scale * m.z, -max_speed, max_speed)));
    let p = w.entities.list[i].pos;
    if e0[1] != 0 && mth::floor(p.x) - pos.x == e0[0] && mth::floor(p.z) - pos.z == e0[2] {
        set_pos(w, i, p.x, p.y + e0[1] as f64, p.z);
    } else if e1[1] != 0 && mth::floor(p.x) - pos.x == e1[0] && mth::floor(p.z) - pos.z == e1[2] {
        set_pos(w, i, p.x, p.y + e1[1] as f64, p.z);
    }
    // `applyNaturalSlowdown`
    let slow = 0.96; // `isVehicle() ? 0.997 : 0.96`
    let mut nm = dm(w, i).multiply(slow, 0.0, slow);
    if in_water(w, i) {
        nm = nm.scale(0.95f32 as f64);
    }
    set_dm(w, i, nm);
    let p = w.entities.list[i].pos;
    let new_pos = get_pos(w, p.x, p.y, p.z);
    if let (Some(np), Some(op)) = (new_pos, old_pos) {
        let speed = (op.y - np.y) * 0.05;
        let v = dm(w, i);
        let other = v.horizontal_distance();
        if other > 0.0 {
            set_dm(w, i, v.multiply((other + speed) / other, 1.0, (other + speed) / other));
        }
        let p = w.entities.list[i].pos;
        set_pos(w, i, p.x, np.y, p.z);
    }
    let p = w.entities.list[i].pos;
    let (xn, zn) = (mth::floor(p.x), mth::floor(p.z));
    if xn != pos.x || zn != pos.z {
        let v = dm(w, i);
        let other = v.horizontal_distance();
        set_dm(w, i, Vec3::new(other * (xn - pos.x) as f64, v.y, other * (zn - pos.z) as f64));
    }
    if power_track {
        let v = dm(w, i);
        let speed = v.horizontal_distance();
        if speed > 0.01 {
            set_dm(w, i, v.add(v.x / speed * 0.06, 0.0, v.z / speed * 0.06));
        } else {
            let (mut dx, mut dz) = (v.x, v.z);
            if shape == RailShape::EastWest {
                if is_conductor(w, pos.rel(crate::pos::Dir::West)) {
                    dx = 0.02;
                } else if is_conductor(w, pos.rel(crate::pos::Dir::East)) {
                    dx = -0.02;
                }
            } else {
                if shape != RailShape::NorthSouth {
                    return;
                }
                if is_conductor(w, pos.rel(crate::pos::Dir::North)) {
                    dz = 0.02;
                } else if is_conductor(w, pos.rel(crate::pos::Dir::South)) {
                    dz = -0.02;
                }
            }
            set_dm(w, i, Vec3::new(dx, v.y, dz));
        }
    }
}

/// `AbstractMinecart.comeOffTrack`.
fn come_off_track(w: &mut World, i: usize) {
    let max = if in_water(w, i) { 0.2 } else { 0.4 };
    let m = dm(w, i);
    set_dm(w, i, Vec3::new(mth::clamp(m.x, -max, max), m.y, mth::clamp(m.z, -max, max)));
    if w.entities.list[i].on_ground {
        let d = dm(w, i);
        set_dm(w, i, d.scale(0.5));
    }
    let d = dm(w, i);
    cart_move(w, i, d);
    if !w.entities.list[i].on_ground {
        let d = dm(w, i);
        set_dm(w, i, d.scale(0.95f32 as f64));
    }
}

/// `AbstractMinecart.tick` of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    if w.entities.list[i].pos.y < (crate::gravity::MIN_Y - 64) as f64 {
        w.entities.list[i].removed = true;
        return;
    }
    entity::compute_speed(w, i);
    // OldMinecartBehavior.tick (server side)
    let g = entity::kinds::gravity(w, i);
    if g != 0.0 {
        let d = dm(w, i);
        set_dm(w, i, d.add(0.0, -g, 0.0));
    }
    let p = current_block_pos_or_rail_below(w, w.entities.list[i].pos);
    let s = w.get(p);
    let on_rails = rails::is_rail(&s);
    st_mut(w, i).on_rails = on_rails;
    if on_rails {
        move_along_track(w, i);
        if let Block::Rail { activator: true, .. } = s
            && st(w, i).hopper
        {
            w.errors.push("hopper minecart on an activator rail: not modeled yet".into());
        }
    } else {
        come_off_track(w, i);
    }
    apply_effects_from_blocks(w, i);
    let e = &mut w.entities.list[i];
    e.x_rot = 0.0;
    let x_diff = e.xo - e.pos.x;
    let z_diff = e.zo - e.pos.z;
    let flipped = st(w, i).flipped;
    let e = &mut w.entities.list[i];
    if x_diff * x_diff + z_diff * z_diff > 0.001 {
        e.y_rot = (mth::atan2(z_diff, x_diff) * 180.0 / std::f64::consts::PI) as f32;
        if flipped {
            e.y_rot += 180.0;
        }
    }
    let rot_diff = mth::wrap_degrees(e.y_rot - e.y_rot_o) as f64;
    let mut flip = false;
    if rot_diff < -170.0 || rot_diff >= 170.0 {
        e.y_rot += 180.0;
        flip = true;
    }
    e.x_rot %= 360.0;
    e.y_rot %= 360.0;
    if flip {
        let m = st_mut(w, i);
        m.flipped = !m.flipped;
    }
    // pushAndPickupEntities: nothing else around in the traced designs
    let bb = w.entities.list[i].bb.inflate(0.2f32 as f64, 0.0, 0.2f32 as f64);
    if w.get_entities(bb, |_| true).iter().any(|&j| j != i) {
        w.errors.push("minecart: pushing other entities is not modeled yet".into());
    }
    entity::fluid_push::update_fluid_interaction(w, i);
    w.entities.list[i].first_tick = false;
    if st(w, i).hopper {
        w.errors.push("hopper minecarts' items are not modeled yet".into());
    }
}
