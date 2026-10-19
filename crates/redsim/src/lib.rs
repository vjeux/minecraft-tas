//! `redsim`: an exact, tick-level Rust model of Minecraft 26.3's redstone and engine queues
//! (neighbor updates, shape updates, scheduled ticks, block events, the tick's phases), checked
//! cell by cell against the real game (the bridge sandbox) by differential fuzzing.
//!
//! Ownership: agent redmodel owns the engine and the redstone components; agent pistmodel owns
//! `piston/` (pistons, heads, moving blocks, the structure resolver) and `gravity.rs`.
//! The model, component by component with source references: `docs/mechanics.md`.

pub mod behavior;
pub mod block;
pub mod classes;
pub mod gravity;
pub mod links;
pub mod piston;
pub mod pos;
pub mod redstone;
pub mod sculk;
pub mod snapshot;
pub mod ticks;
pub mod world;

pub use block::{Block, Kind};
pub use pos::{BlockPos, Dir};
pub use world::World;
