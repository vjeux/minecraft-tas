//! Per-kind overrides the entity core calls (clk-misc): `maxUpStep`, `getGravity`,
//! `checkFallDamage`, read in the 26.3 source (`Entity` defaults, `AbstractBoat`,
//! `AbstractMinecart`, `LivingEntity` for armor stands).

use super::EntityKind;
use crate::world::World;

/// `Entity.maxUpStep()` (armor stands: the `step_height` attribute, 0.6).
pub fn max_up_step(w: &World, i: usize) -> f32 {
    match w.entities.list[i].kind {
        EntityKind::ArmorStand(_) => 0.6,
        _ => 0.0,
    }
}

/// `Entity.getGravity()` (`getDefaultGravity` unless no gravity): 0.04, living entities 0.08,
/// minecarts in water 0.005.
pub fn gravity(w: &World, i: usize) -> f64 {
    let e = &w.entities.list[i];
    match e.kind {
        EntityKind::ArmorStand(_) => 0.08,
        EntityKind::Minecart(_) if e.was_touching_water => 0.005,
        _ => 0.04,
    }
}

/// `Entity.getAirDrag()` as the restitution after a bounce reads it: 0.98F; boats 1.0F, minecarts
/// 0.95F, living entities `computeModifiedFriction(0.98F, 1)` (not an omnidirectional air mover).
pub fn air_drag(w: &World, i: usize) -> f64 {
    match w.entities.list[i].kind {
        EntityKind::Boat(_) => 1.0,
        EntityKind::Minecart(_) => 0.95f32 as f64,
        EntityKind::ArmorStand(_) => (1.0f32 - (1.0f32 - 0.98f32) * 1.0f32).clamp(0.0, 1.0) as f64,
        _ => 0.98f32 as f64,
    }
}

/// `checkFallDamage(ya, onGround, onState, pos)` at the end of `Entity.move`.
pub fn check_fall_damage(w: &mut World, i: usize, ya: f64, on_ground: bool) {
    match w.entities.list[i].kind {
        EntityKind::Boat(_) => super::boat::check_fall_damage(w, i, ya, on_ground),
        _ => {
            // `Entity.checkFallDamage`: no modeled block reacts to a fall (`fallOn`: farmland,
            // slime, beds, ...) and no modeled entity takes fall damage that changes its motion
            let e = &mut w.entities.list[i];
            if !e.was_touching_water && ya < 0.0 {
                e.fall_distance -= ya as f32 as f64;
            }
            if on_ground {
                e.fall_distance = 0.0;
            }
        }
    }
}
