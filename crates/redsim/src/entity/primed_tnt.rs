//! Primed TNT (track tnt): `PrimedTnt`, read in the 26.3 source (`PrimedTnt` 66-125,
//! `EntityTypes.TNT`: 0.98 x 0.98, eye height 0.15, gravity 0.04, air drag 0.98F, no movement
//! emission). Spawned by `tnt::prime` only when [`World::tnt_kick`] is set: the game's random
//! horizontal kick (`level.getRandom().nextDouble()`) is replaced by that value, so a run is
//! reproducible (searches try several kicks; trace-compare injects the game's own TNT state at
//! its first sighting). An explosion is not modeled: a TNT whose fuse runs out is a model error,
//! unless the caller removes primed TNT before that (`World::remove_primed`, the TNT judge).

use crate::entity::{self, EntityKind, MoverType, Vec3};
use crate::world::World;

/// `EntityTypes.TNT.sized(0.98F, 0.98F).eyeHeight(0.15F)`.
pub const WIDTH: f32 = 0.98;
pub const HEIGHT: f32 = 0.98;
pub const EYE_HEIGHT: f32 = 0.15;
/// `setFuse(80)`.
pub const FUSE: i32 = 80;

/// The primed TNT's own state.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PrimedTntState {
    pub fuse: i32,
}

/// The constructor's motion for `nextDouble()` = `u` in [0, 1): `rot = u * (float)(2 pi)`,
/// `(-Math.sin(rot) * 0.02, 0.2F, -Math.cos(rot) * 0.02)`, with the JVM's own sine and cosine
/// ([`crate::entity::jmath`]: one ulp from the correctly rounded ones for some angles).
pub fn kick(u: f64) -> Vec3 {
    let rot = u * (std::f64::consts::PI * 2.0) as f32 as f64;
    Vec3::new(-crate::entity::jmath::sin(rot) * 0.02, 0.2f32 as f64, -crate::entity::jmath::cos(rot) * 0.02)
}

/// `new PrimedTnt(level, x, y, z, null)` + `addFreshEntity`: at `pos` with the kick of `u`.
pub fn spawn(w: &mut World, pos: Vec3, u: f64) -> u32 {
    let id = w.entities.add(EntityKind::PrimedTnt(PrimedTntState { fuse: FUSE }), pos, 0.0);
    let e = w.entities.list.last_mut().unwrap();
    e.delta_movement = kick(u);
    id
}

/// `PrimedTnt.tick` of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    // handlePortal: no portals; applyGravity
    let g = entity::kinds::gravity(w, i);
    if g != 0.0 {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.add(0.0, -g, 0.0);
    }
    let d = w.entities.list[i].delta_movement;
    entity::move_(w, i, MoverType::Self_, d);
    if w.entities.list[i].removed {
        return;
    }
    entity::apply_effects_from_blocks(w, i);
    let fuse = {
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.scale(0.98f32 as f64);
        if e.on_ground {
            e.delta_movement = e.delta_movement.multiply(0.7, -0.5, 0.7);
        }
        let EntityKind::PrimedTnt(s) = &mut e.kind else { return };
        s.fuse -= 1;
        s.fuse
    };
    if fuse <= 0 {
        // discard, then (server side) explode: `level.explode(this, .., getX(), getY(0.0625),
        // getZ(), 4.0F, false, TNT)` (getY(0.0625) = y + bbHeight (0.98F) * 0.0625), unless the
        // caller removes primed TNT before they explode (the TNT judge)
        w.entities.list[i].removed = true;
        if !w.remove_primed {
            let (p, id) = (w.entities.list[i].pos, w.entities.list[i].id);
            let center = Vec3::new(p.x, p.y + HEIGHT as f64 * 0.0625, p.z);
            crate::explosion::explode(w, Some(id), center, crate::explosion::TNT_POWER);
        }
    } else {
        entity::fluid_push::update_fluid_interaction(w, i);
    }
}
