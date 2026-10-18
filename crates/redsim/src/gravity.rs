//! Falling blocks (sand, gravel, concrete powder) and their entities.
//!
//! OWNED BY AGENT PISTMODEL: redmodel's stub with the entry points the world calls.

use crate::world::World;

/// Falling-block entities, ticked in insertion order in the entity phase.
#[derive(Default, Clone)]
pub struct State {}

pub fn is_idle(_w: &World) -> bool {
    true
}

/// The entity phase (after block events, before block entities).
pub fn tick_entities(_w: &mut World) {}
