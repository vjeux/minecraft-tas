//! Armor stands (clk-misc, the dripleaf-stand clock): `ArmorStand`, a `LivingEntity` with no AI
//! and no input, read in the 26.3 source: `LivingEntity.tick` = `baseTick`, then `aiStep` (the
//! 0.003 motion floors, `travel` = `travelInAir`: block friction when on the ground, the zero
//! input added to the motion, `move`, gravity 0.08 after the move, air drag 0.91 / 0.98, then the
//! blocks it stands in: `applyEffectsFromBlocks`). Head and body rotations do not change the
//! stand's yaw. Not modeled: a stand in water or lava (`travelInFluid`), pushing minecarts.

use crate::entity::{self, MoverType, Vec3};
use crate::world::World;

/// `EntityTypes.ARMOR_STAND.sized(0.5F, 1.975F).eyeHeight(1.7775F)`.
pub const WIDTH: f32 = 0.5;
pub const HEIGHT: f32 = 1.975;

/// The armor stand's own state: clk-misc's.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ArmorStandState {}

/// `LivingEntity.computeModifiedFriction` (attribute modifiers at their default 1.0).
fn modified_friction(friction: f32, modifier: f32) -> f32 {
    (1.0f32 - (1.0f32 - friction) * modifier).clamp(0.0, 1.0)
}

/// `ArmorStand.tick` (`LivingEntity.tick`) of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    entity::base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    // aiStep: tiny motions are floored to 0 (not a player)
    {
        let e = &mut w.entities.list[i];
        let m = e.delta_movement;
        let dx = if m.x.abs() < 0.003 { 0.0 } else { m.x };
        let dy = if m.y.abs() < 0.003 { 0.0 } else { m.y };
        let dz = if m.z.abs() < 0.003 { 0.0 } else { m.z };
        e.delta_movement = Vec3::new(dx, dy, dz);
    }
    if w.entities.list[i].was_touching_water {
        w.errors.push("armor stand in water: travelInFluid is not modeled".into());
        return;
    }
    // travelInAir
    let on_ground = w.entities.list[i].on_ground;
    let block_friction = if on_ground {
        let below = entity::collide::get_on_pos(w, &w.entities.list[i], 0.500001);
        modified_friction(entity::boat::block_friction(&w.get(below)), 1.0)
    } else {
        1.0
    };
    // handleRelativeFrictionAndCalculateMovement: moveRelative with no input adds Vec3.ZERO
    let d = w.entities.list[i].delta_movement.add(0.0, 0.0, 0.0);
    w.entities.list[i].delta_movement = d;
    entity::move_(w, i, MoverType::Self_, d);
    let movement = w.entities.list[i].delta_movement;
    let movement_y = movement.y - entity::kinds::gravity(w, i);
    let air_drag = modified_friction(0.91, 1.0);
    let friction = block_friction * air_drag;
    let vertical = modified_friction(0.98, 1.0);
    w.entities.list[i].delta_movement = Vec3::new(movement.x * friction as f64, movement_y * vertical as f64, movement.z * friction as f64);
    entity::apply_effects_from_blocks(w, i);
}
