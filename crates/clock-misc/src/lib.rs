//! Track clk-misc's clock commands: `mctas clock misc <args...>` calls [`run`] with every argument
//! after `misc`. OWNED BY TRACK clk-misc: the blocks no other clock track covers (crafters,
//! powered and activator rails, doors, fence gates, heads, fluids, daylight detectors...).
//!
//! * [`ring`]: relay rings, one extendable family per relay block. A 2-wide observer ring stands
//!   in a vertical plane with ONE relay block at its top, on the observer that powers it (so a
//!   rail, a door or a bell has its support); the next observer watches the relay. A relay whose
//!   state follows its power at once (a crafter's `triggered`, a rail's `powered`, a door's or a
//!   fence gate's `open`) takes a lattice cell without adding delay, so the ring closes with an odd
//!   number of observers: P = 2 gt x observers = 10 + 4n.
//!
//! The official judge is `mctas clock verify` (crates/clock, docs/clocks.md); this crate builds
//! and searches.

pub mod boat;
pub mod cart;
pub mod crafter;
pub mod rename;
pub mod ring;
pub mod trace;
pub mod unarchive;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "mctas clock misc", about = "Track clk-misc's clocks: crafters, rails, doors, fence gates, heads and other blocks no other track covers")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// A relay ring (extendable family `clk-misc-<relay>-ring`, P = 10 + 4n): a 2-wide observer
    /// ring of height 3 + n in the x-y plane with one relay block at its top left, on the observer
    /// that powers it; checked on redsim. `--list` shows the relays, `--check K` runs n = 0..=K.
    Ring(ring::RingArgs),
    /// A crafter item loop (extendable family `clk-misc-crafter-loop`): a crafter crafting back
    /// and forth between a slime block and 9 slime balls, 2n + 1 hoppers carrying the results back,
    /// a comparator on the feeding hopper and two observers triggering the crafter. Prints or
    /// writes the design (the real game judges it: redsim does not model crafting).
    CrafterLoop(crafter::LoopArgs),
    /// A minecart ping-pong (extendable family `clk-misc-cart-pingpong`): an empty minecart
    /// bouncing between two bumpers, powered rails at the ends, a detector rail, n more powered
    /// rails in the middle. Prints or writes the design (judge it with `clock verify`: entities).
    CartPingpong(cart::PingpongArgs),
    /// A minecart loop (extendable family `clk-misc-cart-loop`): an empty minecart circling a
    /// 4 x (4 + n) rail loop with powered rails and a detector rail.
    CartLoop(cart::LoopArgs),
    /// Set each design's `name:` to `<prefix>-<file stem>` (search tools name their picks after
    /// their own track; the archive keys designs by name).
    Rename(rename::RenameArgs),
    /// Take archived clocks back out (design, verdict and caption rows, the family's extend rows)
    /// to re-archive a rebuilt version under the same name.
    Unarchive(unarchive::UnarchiveArgs),
    /// Build a design in the real game like `clock verify` and print every entity's position and
    /// motion and the changed cells, tick by tick (boats, minecarts).
    Trace(trace::TraceArgs),
    /// A redstone-less boat ring: a boat circling a water ring around an A x B island, pushed by
    /// four one-way currents cut by rows of wooden pressure plates (which it presses).
    BoatRing(boat::RingArgs),
}

/// `mctas clock misc <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let a = match Args::try_parse_from(std::iter::once("mctas clock misc".to_string()).chain(args)) {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => {
            print!("{e}");
            return Ok(());
        }
        Err(e) => anyhow::bail!("{e}"),
    };
    match a.cmd {
        Cmd::Ring(r) => ring::run(r),
        Cmd::CrafterLoop(l) => crafter::run(l),
        Cmd::CartPingpong(p) => cart::run(p),
        Cmd::CartLoop(l) => cart::run_loop(l),
        Cmd::Rename(r) => rename::run(r),
        Cmd::Unarchive(u) => unarchive::run(u),
        Cmd::Trace(t) => trace::run(t),
        Cmd::BoatRing(r) => boat::run(r),
    }
}
