//! Armor stands (clk-misc): `ArmorStand` (a `LivingEntity` without AI) standing on blocks — the
//! big dripleaf clock's passenger. SKELETON: dimensions and payload; the tick records a model
//! error until it is ported (`LivingEntity.tick` / `aiStep` / `travel`).

use crate::world::World;

/// `EntityTypes.ARMOR_STAND.sized(0.5F, 1.975F)`.
pub const WIDTH: f32 = 0.5;
pub const HEIGHT: f32 = 1.975;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ArmorStandState {}

/// `ArmorStand.tick` of entity `i`.
pub fn tick(w: &mut World, _i: usize) {
    w.errors.push("armor stands are not modeled yet".into());
}
