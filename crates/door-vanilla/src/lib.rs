//! `mctas door` commands: checks in the real game (the official server): vanilla-check, vanilla-build, vanilla-gadget, vanilla-fuzz, buildable.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
// The command bodies (cmd/*.rs) name piston-door's modules `crate::<module>` through this glob.
use piston_door::*;

mod cmd;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Run designs in an UNMODIFIED vanilla dedicated server (official jar, RCON, a bot player
    /// flips the lever) and compare with the simulator tick for tick, with the judge's rules.
    VanillaCheck(cmd::vanilla::VanillaCheckArgs),
    /// Replay survival build scripts (`door buildable`'s `*.build.txt`) in an UNMODIFIED vanilla
    /// server: a bot player selects each item, looks the script's way, right-clicks the given face
    /// (temporary dirt placed and broken by clicks too), the lever state as written; every action
    /// is compared with the simulator, the built world with the judge's rest state, then the
    /// judge's cycles run with the bot (vanilla-check's comparison). See vanilla_build.rs.
    VanillaBuild(cmd::vanilla::VanillaBuildArgs),
    /// Replay gadget drive scripts (designs/gadgets/*.drive) in the UNMODIFIED official server
    /// and compare every tick with the simulator (see vanilla_gadget.rs): the design in its rest
    /// state, `use` by a bot player, `setblock` as the game's command in both worlds, every tick
    /// of every step compared cell by cell, at the judge's 4 positions + N.
    VanillaGadget(cmd::vanilla::VanillaGadgetArgs),
    /// Differential fuzzing: random small contraptions (not doors), settled by the simulator,
    /// run in both worlds and compared tick for tick; diverging ones are saved.
    VanillaFuzz(cmd::vanilla::VanillaFuzzArgs),
    /// Can a survival player build it? Search for a build program (placement order with real
    /// player placements and normal updates, the lever state while building, temporary helper
    /// blocks) that ends exactly in the judge's rest state, then run the judge's cycles from the
    /// built state (see buildable.rs). Designs: files, `@builtin`, or `<git ref>:<path>`.
    Buildable(cmd::buildable::BuildableArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::VanillaCheck(a) => cmd::vanilla::cmd_vanilla_check(version, a),
        DoorCmd::VanillaBuild(a) => cmd::vanilla::cmd_vanilla_build(version, a),
        DoorCmd::VanillaGadget(a) => cmd::vanilla::cmd_vanilla_gadget(version, a),
        DoorCmd::VanillaFuzz(a) => cmd::vanilla::cmd_vanilla_fuzz(version, a),
        DoorCmd::Buildable(a) => cmd::buildable::cmd_buildable(version, a),
    }
}
