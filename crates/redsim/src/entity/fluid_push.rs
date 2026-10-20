//! Entity fluid interaction (redsim M2, clockinfra): `Entity.updateFluidInteraction` and
//! `EntityFluidInteraction` (26.3), the part of `baseTick` that measures how deep an entity is in
//! each fluid, whether its eyes are in it, and pushes it along the current.
//!
//! * Trackers per fluid TYPE (source water and flowing water are separate trackers; the WATER
//!   tag's height is the max of both), kept across ticks while they are non-zero.
//! * One current accumulator per tag with a current (water here): cells visited x, then y, then
//!   z; each cell's flow is scaled by the accumulator's height (the running max of the CURRENT
//!   tracker's height) while that is under 0.4.
//! * `applyTo` (non-players): the normalized sum times 0.014, raised to 0.0045 long when the
//!   entity is almost still horizontally.

use crate::entity::{Entity, Vec3, mth};
use crate::fluid::{self, Fluid};
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// `EntityFluidInteraction.Tracker`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tracker {
    pub fluid: Fluid,
    pub height: f64,
    pub eyes_inside: bool,
}

/// `EntityFluidInteraction` (the trackers live across ticks; the water current accumulator is
/// reset every update).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct FluidInteraction {
    pub trackers: Vec<Tracker>,
    current_height: f64,
    current: Vec3,
    current_count: i32,
}

impl FluidInteraction {
    /// `getFluidHeight(WATER)`.
    pub fn water_height(&self) -> f64 {
        let mut h = 0.0;
        for t in &self.trackers {
            if t.height > h && t.fluid.is_water() {
                h = t.height;
            }
        }
        h
    }

    /// `isInFluid(WATER)`.
    pub fn in_water(&self) -> bool {
        self.water_height() > 0.0
    }

    /// `isEyeInFluid(WATER)`.
    pub fn eye_in_water(&self) -> bool {
        self.trackers.iter().any(|t| t.eyes_inside && t.fluid.is_water())
    }

    fn tracker(&mut self, f: Fluid) -> usize {
        if let Some(i) = self.trackers.iter().position(|t| t.fluid == f) {
            return i;
        }
        self.trackers.push(Tracker { fluid: f, height: 0.0, eyes_inside: false });
        self.trackers.len() - 1
    }
}

/// `FluidState.getHeightForCamera`: 1 for a source under a block whose bottom face is sturdy.
fn height_for_camera(w: &World, p: BlockPos, f: fluid::FluidState) -> f32 {
    if f.is_source() {
        let above = p.above();
        if crate::behavior::sturdy(w, &w.get(above), above, Dir::Down, crate::behavior::Support::Full) {
            return 1.0;
        }
    }
    f.height(w, p)
}

/// `EntityFluidInteraction.update(entity, ignoreCurrent)`: true when the box touches a fluid.
pub fn update(w: &World, e: &mut Entity, ignore_current: bool) -> bool {
    let fi = &mut e.fluid;
    fi.trackers.retain_mut(|t| {
        if t.height == 0.0 && !t.eyes_inside {
            return false;
        }
        t.height = 0.0;
        t.eyes_inside = false;
        true
    });
    fi.current_height = 0.0;
    fi.current = Vec3::ZERO;
    fi.current_count = 0;
    // getFluidInteractionBox: the box deflated by 0.001 (no vehicles here)
    let bx = e.bb.deflate(0.001, 0.001, 0.001);
    let (x0, y0, z0) = (mth::floor(bx.min_x), mth::floor(bx.min_y), mth::floor(bx.min_z));
    let ceil = |v: f64| v.ceil() as i32;
    let (x1, y1, z1) = (ceil(bx.max_x) - 1, ceil(bx.max_y) - 1, ceil(bx.max_z) - 1);
    let entity_y = e.bb.min_y;
    let eye_x = mth::floor(e.pos.x);
    let eye_y = e.pos.y + e.kind.eye_height() as f64;
    let eye_z = mth::floor(e.pos.z);
    let mut last: Option<Fluid> = None;
    let mut tracker = 0usize;
    let mut has_current = false;
    for x in x0..=x1 {
        for y in y0..=y1 {
            for z in z0..=z1 {
                let p = BlockPos::new(x, y, z);
                let f = fluid::fluid_state(w, p);
                if f.is_empty() {
                    continue;
                }
                let bottom = y as f64;
                let top = bottom + f.height(w, p) as f64;
                if top < bx.min_y {
                    continue;
                }
                if last != Some(f.fluid) {
                    last = Some(f.fluid);
                    tracker = fi.tracker(f.fluid);
                    if !ignore_current {
                        // every fluid with a current is water here (the WATER tag)
                        has_current = f.fluid.is_water();
                    }
                }
                if x == eye_x && z == eye_z && eye_y >= bottom {
                    let cam = bottom + height_for_camera(w, p, f) as f64;
                    if eye_y <= cam {
                        fi.trackers[tracker].eyes_inside = true;
                    }
                }
                let th = crate::entity::vec3::jmax(top - entity_y, fi.trackers[tracker].height);
                fi.trackers[tracker].height = th;
                if has_current {
                    let mut flow = fluid::flow(w, p, f);
                    fi.current_height = crate::entity::vec3::jmax(th, fi.current_height);
                    if fi.current_height < 0.4 {
                        flow = flow.scale(fi.current_height);
                    }
                    fi.current = fi.current.add_v(flow);
                    fi.current_count += 1;
                }
            }
        }
    }
    last.is_some()
}

/// `CurrentAccumulator.applyTo(entity, scale)` for a non-player entity.
fn apply_current(e: &mut Entity, scale: f64) {
    let fi = &e.fluid;
    if fi.current_count == 0 || fi.current.length_sqr() < 1.0e-5f32 as f64 {
        return;
    }
    let old = e.delta_movement;
    let mut impulse = fi.current.normalize().scale(scale);
    if old.x.abs() < 0.003 && old.z.abs() < 0.003 && impulse.length() < 0.0045000000000000005 {
        impulse = impulse.normalize().scale(0.0045000000000000005);
    }
    e.delta_movement = e.delta_movement.add_v(impulse);
}

/// `Entity.updateFluidInteraction` of entity `i` (called by `baseTick`): the trackers, the fall
/// distance reset in water, `wasTouchingWater`, the water current (0.014; every modeled entity
/// is pushed by fluids). Returns `inAnyFluid`.
pub fn update_fluid_interaction(w: &mut World, i: usize) -> bool {
    let mut e = w.entities.list[i].clone();
    let in_any = update(w, &mut e, false);
    let in_water = e.fluid.in_water();
    if in_water {
        e.fall_distance = 0.0;
    }
    e.was_touching_water = in_water;
    e.fluid_height = [e.fluid.water_height(), 0.0];
    if in_water {
        apply_current(&mut e, 0.014);
    }
    w.entities.list[i] = e;
    in_any
}
