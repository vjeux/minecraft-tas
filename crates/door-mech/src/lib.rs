//! `mctas door` commands: mechanisms and their wiring: mech-synth, mech-wire, mechspace, families, topology, connect, fullmech, obs-chain, ideal, drive, rows-fire.
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
    /// Where each strip-row mechanism's pistons fire (rowsfire.rs): replays every row of agent
    /// families' ROWS.tsv (`door families --rows`) in the 1-D macro model and prints, per piston
    /// action of the open and close schedules, the cell the piston stands on when it fires.
    /// In a 5-tall strip box rows 0 and 1 cannot be fired from the door columns x 0..2 (the floor
    /// and ceiling ring are there): `--wirable` keeps the rows with no such firing.
    RowsFire(cmd::drive::RowsFireArgs),
    /// Drive a design by hand from a script (see drive.rs) and print the z=0 plane per tick.
    /// Several scripts run one after the other; any failed `expect` makes the command fail.
    Drive(cmd::drive::DriveArgs),
    /// Judge a mechanism driven by ideal inputs from a schedule (see ideal.rs); sweep its timing.
    Ideal(cmd::drive::IdealArgs),
    /// Wire a mechanism from its ideal-input schedule (mechwire.rs): replay it with ideal inputs,
    /// route every piston cell's wave from a lever at each lever spot with route2's router, rank
    /// mechanism + lever + wiring by blocks, judge the best. --retime re-times a mech-synth
    /// schedule to chain ticks first (every working assignment is wired).
    MechWire(cmd::mechwire::MechWireArgs),
    /// Exact decision procedure for families' macro pulse model (mechspace.rs): partial-order
    /// reduction with stubborn sets (footprint groups, landmark condition), hole rule exact.
    /// `selftest` (tracked transitions vs Mech::apply, independence claims), `validate` (every
    /// candidate of --boxes, compared with families' full-graph solver), `decide --input
    /// unresolved/WxH.txt --boxes WxH`, `gen` (families' generator with --two/--carry/--vmid,
    /// every candidate decided).
    Mechspace(cmd::families::MechspaceArgs),
    /// The catalog of flat (1-wide) seamless mechanisms per box (families.rs): every door block
    /// leaves the 3x3 in 1-2 unit moves (center 2), each move provided by a pull or push piston
    /// (reused, or lifted by one elevator piston); every candidate solved exactly in the macro
    /// piston model with the FULL seamless open test, reduced to minimal mechanisms, mirror
    /// images merged; writes CATALOG.md + CATALOG.tsv; `--verify` replays each in the simulator.
    Families(cmd::families::FamiliesArgs),
    /// Local search around known flat mechanisms (mechmut.rs) for variants with fewer pistons and
    /// better feedability: drop / shift / turn a piston, toggle sticky / extended; every neighbour
    /// solved exactly in families' macro model and kept with a FULL frame; ranked by (pistons no
    /// free cell can feed, pistons, wiring class, events); `--ideal-out` writes the best as
    /// ideal-input designs + plans.
    Mechmut(cmd::families::MechmutArgs),
    /// Mechanism synthesis (mechsynth.rs): enumerate up to --add blocks (pistons, slime...) on a
    /// base mechanism, solve each in a macro piston model (open + close paths, symmetric
    /// schedules), rank by (blocks, control events), write ideal-input designs + schedules and
    /// replay the best in the simulator (`--verify N`).
    /// Wiring topology (topology.rs) of every mechanism in agent families' catalog TSV, each in its
    /// own box: wiring components, piston links, pulse parity; WIRABLE or not (strict / permissive)
    Topology(cmd::families::TopologyArgs),
    /// Connectivity check (connect.rs): can a flat mechanism be wired inside a box at all? Free-cell
    /// regions after the run's footprint and the ring; each piston fed from a region next to it or
    /// its quasi cell (directly, through a plain ring stone or a door block); a region wakes when it
    /// can watch a change; every event order searched, both edges starting from the lever's region
    /// only. Parity and room are ignored: a necessary condition. Reads families' CATALOG.tsv and
    /// tries every placement of each entry in each box.
    Connect(cmd::families::ConnectArgs),
    MechSynth(cmd::mechsynth::MechSynthArgs),
    /// Apply an edit script to a design and write the (canonicalized) result. Script lines:
    /// `set x y z <state>` (an extended piston gets its head too), `clear x y z` (a piston
    /// takes its head along), `port <name> x y z`; `#` starts a comment line.
    /// Wire a pulse-driven mechanism with a tree of observers rooted at an observer watching the
    /// lever (obschain.rs): every driver observer (spec options) fires at its time, watching a
    /// tree observer one step earlier; exact-length observer paths through free cells, fewest
    /// observers (branch and bound within --secs).
    ObsChain(cmd::mechsynth::ObsChainArgs),
    /// Mechanism search on the EXACT model (track pistmodel, crates/piston-door/src/fullmech.rs):
    /// every transition is the real tick-level game (redsim: spitting, 0-ticks, event order,
    /// carried pistons and power sources), inputs tracked per piston; FULL seamless open +
    /// close back to rest. `--check` solves the mechanism of designs; `--boxes` re-solves
    /// families' candidates per box.
    Fullmech(cmd::mechsynth::FullmechArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::RowsFire(a) => cmd::drive::cmd_rows_fire(a),
        DoorCmd::Drive(a) => cmd::drive::cmd_drive(version, a),
        DoorCmd::Ideal(a) => cmd::drive::cmd_ideal(version, a),
        DoorCmd::MechWire(a) => cmd::mechwire::cmd_mech_wire(version, a),
        DoorCmd::Mechspace(a) => cmd::families::cmd_mechspace(version, a),
        DoorCmd::Families(a) => cmd::families::cmd_families(version, a),
        DoorCmd::Mechmut(a) => cmd::families::cmd_mechmut(a),
        DoorCmd::Topology(a) => cmd::families::cmd_topology(a),
        DoorCmd::Connect(a) => cmd::families::cmd_connect(a),
        DoorCmd::MechSynth(a) => cmd::mechsynth::cmd_mech_synth(version, a),
        DoorCmd::ObsChain(a) => cmd::mechsynth::cmd_obs_chain(version, a),
        DoorCmd::Fullmech(a) => cmd::mechsynth::cmd_fullmech(a),
    }
}
