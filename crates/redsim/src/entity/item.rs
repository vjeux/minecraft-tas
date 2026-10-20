//! Item entities (redsim M5): `ItemEntity`. SKELETON: the payload type, the dimensions and a
//! tick that records a model error until it is filled.

use crate::world::World;

/// `EntityTypes.ITEM.sized(0.25F, 0.25F)`.
pub const WIDTH: f32 = 0.25;
pub const HEIGHT: f32 = 0.25;

/// The item entity's own state (the stack, age, pickup delay): to come.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct ItemState {}

/// `ItemEntity.tick` of entity `i`.
pub fn tick(w: &mut World, _i: usize) {
    w.errors.push("item entities are not modeled yet (redsim M5)".into());
}
