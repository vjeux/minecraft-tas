//! Minecarts (clk-misc, redsim M4): `AbstractMinecart` + `OldMinecartBehavior`, hopper
//! minecarts. SKELETON: the payload type, the dimensions and a tick that records a model error
//! until M4 fills it.

use crate::world::World;

/// `EntityTypes.MINECART.sized(0.98F, 0.7F)` (hopper and chest minecarts too).
pub const WIDTH: f32 = 0.98;
pub const HEIGHT: f32 = 0.7;

/// The minecart's own state (variant, flipped, onRails, hopper contents, ...): clk-misc's.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MinecartState {}

/// `AbstractMinecart.tick` of entity `i`.
pub fn tick(w: &mut World, _i: usize) {
    w.errors.push("minecarts are not modeled yet (redsim M4)".into());
}
