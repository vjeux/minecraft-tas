//! Item clocks (the clock study's track clk-item: hoppers, droppers, comparators reading
//! containers, item-count-tuned periods).
//!
//! * [`sim`]: a design (blocks + `nbt:` container contents) run in redsim, the exact model, until
//!   its WHOLE state repeats (block states, comparator outputs, container slots, hopper cooldowns,
//!   pending ticks, torch memory, moving blocks): the first repeat gives the exact period and the
//!   transient; then one period is replayed to get the active set (blocks whose state or
//!   block-entity data change: the family) and the output waveforms.
//! * [`census`]: every design of k blocks in a box from a palette, with container contents from a
//!   list, judged by [`sim`]; clocks deduplicated by type (translation + the 4 rotations) and
//!   written per period, smallest first.
//! * [`jvm`]: the same design placed in the real game (the bridge), its block states and
//!   block-entity data recorded every tick and compared with redsim; the period measured there.
//!
//! The official judge is clockinfra's `mctas clock verify`; this crate is the item family's
//! search and measuring tool.

pub mod census;
pub mod cli;
pub mod jvm;
pub mod sim;
