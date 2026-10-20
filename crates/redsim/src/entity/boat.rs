//! Boats (clk-misc, redsim M3): `AbstractBoat`. SKELETON: the payload type, the dimensions and
//! a tick that records a model error until M3 fills it.

use crate::world::World;

/// `EntityTypes.OAK_BOAT.sized(1.375F, 0.5625F)`.
pub const WIDTH: f32 = 1.375;
pub const HEIGHT: f32 = 0.5625;

/// The boat's own state (status, deltaRotation, paddles, ...): clk-misc's.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct BoatState {}

/// `AbstractBoat.tick` of entity `i`.
pub fn tick(w: &mut World, _i: usize) {
    w.errors.push("boats are not modeled yet (redsim M3)".into());
}
