//! Armor stands (clk-misc, the dripleaf-stand clock): `ArmorStand` (a `LivingEntity` without AI)
//! standing on blocks. SKELETON: the payload type, the dimensions and a tick that records a model
//! error until clk-misc fills it (`LivingEntity.tick` / `aiStep` / `travel`).

use crate::world::World;

/// `EntityTypes.ARMOR_STAND.sized(0.5F, 1.975F).eyeHeight(1.7775F)`.
pub const WIDTH: f32 = 0.5;
pub const HEIGHT: f32 = 1.975;

/// The armor stand's own state: clk-misc's.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ArmorStandState {}

/// `ArmorStand.tick` of entity `i`.
pub fn tick(w: &mut World, _i: usize) {
    w.errors.push("armor stands are not modeled yet (clk-misc)".into());
}
