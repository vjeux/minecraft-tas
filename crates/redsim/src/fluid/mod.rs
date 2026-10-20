//! Fluids (redsim M1, clockinfra; docs: designs/clocks/tracks/clockinfra/REDSIM-FLUIDS-ENTITIES.md):
//! `FluidState` of a block, heights and flow (the entity side reads them), and water's own
//! behavior (`FlowingFluid`, `WaterFluid`, `LiquidBlock`, waterlogged blocks, bubble columns).
//!
//! STATUS: API skeleton: no block holds a fluid yet ([`fluid_state`] is always empty); M1 adds the
//! water block, the fluid tick list and the spreading rules.

use crate::entity::Vec3;
use crate::pos::BlockPos;
use crate::world::World;

/// The fluid types (`Fluids`); `Water` = source, `FlowingWater` = flowing.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Fluid {
    Empty,
    Water,
    FlowingWater,
}

impl Fluid {
    /// `Fluid.isSame` for water.
    pub fn is_water(self) -> bool {
        matches!(self, Fluid::Water | Fluid::FlowingWater)
    }
}

/// `FluidState`: the fluid, its amount (8 for a source or a falling column, 1..7 flowing) and
/// `FALLING`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FluidState {
    pub fluid: Fluid,
    pub amount: u8,
    pub falling: bool,
}

impl FluidState {
    pub const EMPTY: FluidState = FluidState { fluid: Fluid::Empty, amount: 0, falling: false };

    pub fn is_empty(self) -> bool {
        self.fluid == Fluid::Empty
    }

    pub fn is_source(self) -> bool {
        self.fluid == Fluid::Water
    }

    /// `getOwnHeight`: amount / 9.0F.
    pub fn own_height(self) -> f32 {
        if self.is_empty() { 0.0 } else { self.amount as f32 / 9.0 }
    }

    /// `getHeight(level, pos)`: 1 under the same fluid, else its own height.
    pub fn height(self, w: &World, p: BlockPos) -> f32 {
        if !self.is_empty() && fluid_state(w, p.above()).fluid.is_water() == self.fluid.is_water() && !fluid_state(w, p.above()).is_empty() {
            1.0
        } else {
            self.own_height()
        }
    }
}

/// The fluid state of the block at `p` (`BlockState.getFluidState`).
pub fn fluid_state(w: &World, p: BlockPos) -> FluidState {
    let _ = w.get(p);
    FluidState::EMPTY
}

/// `FlowingFluid.getFlow(level, pos, state)`: the normalized flow vector (ZERO for no fluid).
pub fn flow(w: &World, p: BlockPos, s: FluidState) -> Vec3 {
    let _ = (w, p, s);
    Vec3::ZERO
}
