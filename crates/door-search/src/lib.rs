//! `mctas door` commands: block-count and speed searches on whole designs: minimize, grow, regrow, harvest, transplant, synth, resynth, route, route2, place.
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
    /// Route a dust path in a design (prints the blocks to add).
    Route(cmd::route::RouteArgs),
    /// Try every combination of alternative patch groups (see place.rs): for wiring a mechanism
    /// whose part placements are a 3D puzzle. Screened at one position, judged at the default
    /// positions; every working combination is written next to --out.
    Place(cmd::route::PlaceArgs),
    /// Port-aware, timing-aware net router (route2.rs). Rips every connected group of up to
    /// --max-rip control blocks (or the --rip groups) and re-routes the receivers whose signal
    /// changed (taps anywhere, enters each piston any way, places dust, repeaters, observers and
    /// conductor blocks to reproduce every piston cell's signal tick for tick in a static model),
    /// with fewer blocks; candidates are screened and judged. With --need, routes a new net:
    /// the listed piston cells must get the given waves (see `Wave::parse`).
    Route2(cmd::route::Route2Args),
    /// Cut every face-connected group of --size control blocks (pistons excluded) and re-grow it
    /// with size-1 blocks from --states inside the group's box grown by --margin (see search.rs
    /// `regrow`). The best verified design is written to --out.
    Regrow(cmd::grow::RegrowArgs),
    /// Add 1 or 2 blocks (from --states, in the free cells of --lo..--hi) to a design, usually
    /// one whose old wiring was removed, screening every combination and judging the survivors
    /// (see search.rs `grow`). The best verified design is written to --out.
    Grow(cmd::grow::GrowArgs),
    /// Local search for fewer blocks (see search.rs): removals, repeater delays, moves,
    /// substitutions and dust re-routing. Candidates are screened at a few positions and accepted
    /// only if the full judge passes; every improvement is written to --out.
    Minimize(cmd::minimize::MinimizeArgs),
    /// Run the local search on the best designs of every branch (see harvest.rs): the `top`
    /// smallest working designs per category not harvested yet, `minutes` each.
    Harvest(cmd::minimize::HarvestArgs),
    /// Learn edits from pairs of designs (a -> b, b a smaller variant of a) and try each on the
    /// best designs of every league (see harvest.rs); improvements are archived and polished.
    Transplant(cmd::minimize::TransplantArgs),
    /// Circuit synthesis by exhaustive simulation (see synth.rs): every circuit of up to --k
    /// blocks from --palette in the free cells of --lo..--hi that carries a signal from the
    /// --from cells to the --to cells is simulated against a target (the door judge by default,
    /// or a --spec behavior script); the smallest verified circuits are reported and written.
    Synth(cmd::synth::SynthArgs),
    /// Large-neighborhood search by exact synthesis (see synth.rs `resynth`): every connected
    /// group of 2..--max-size removable blocks (wiring, still plain blocks) is cleared and every
    /// circuit of fewer blocks that could reconnect its surroundings is screened and judged; an
    /// accepted replacement is applied and the search restarts. Writes the best design to --out.
    /// Several designs: a sweep (one worker pool), --out is a folder, each improved design is
    /// written there as `<name>-rs.txt`, and a design that fails the judge or its speed bounds
    /// at the start is skipped.
    Resynth(cmd::synth::ResynthArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::Route(a) => cmd::route::cmd_route(version, a),
        DoorCmd::Place(a) => cmd::route::cmd_place(version, a),
        DoorCmd::Route2(a) => cmd::route::cmd_route2(version, a),
        DoorCmd::Regrow(a) => cmd::grow::cmd_regrow(version, a),
        DoorCmd::Grow(a) => cmd::grow::cmd_grow(version, a),
        DoorCmd::Minimize(a) => cmd::minimize::cmd_minimize(version, a),
        DoorCmd::Harvest(a) => cmd::minimize::cmd_harvest(version, a),
        DoorCmd::Transplant(a) => cmd::minimize::cmd_transplant(version, a),
        DoorCmd::Synth(a) => cmd::synth::cmd_synth(version, a),
        DoorCmd::Resynth(a) => cmd::synth::cmd_resynth(version, a),
    }
}
