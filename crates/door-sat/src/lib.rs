//! `mctas door` commands: the SAT tools: satwire, satsweep, boxproof.
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
    /// SAT synthesis of a flat door's WIRING for a fixed mechanism (satwire.rs): the mechanism
    /// and each piston's power per tick come from an ideal schedule (`.ideal`) or a wired design
    /// (`--from-wired`, wiring stripped); placement one-hot per free cell of the box, two traces
    /// (lever on from rest, off from open), time-unrolled redstone semantics; every model is
    /// judged by the real simulator (CEGAR) and the wiring is minimized.
    /// Whole-box bounded model checking of flat doors (track boxproof): `fuzz` (the model vs
    /// redsim on random boxes), `case`, `bmc` (find a door / prove none fits a box).
    Boxproof(cmd::boxproof::BoxproofArgs),
    /// Macro-level bounded model checking of flat doors (macrosat.rs, the 7x7 check): every cell
    /// of the box one-hot over a palette at every macro step (one settled piston action: extend,
    /// pulse/spit, retract/pull), FREE layout or `--fix design`, door = square stone at rest,
    /// square air + 12 frame cells stone when open, back to rest on close; `--feed` requires a
    /// feed at every firing (wiring cell, redstone block, powered conductor, QC, the lever).
    /// `--box x0,y0,x1,y1 --h N [--feed] [--lever] [--fix design]`.
    Macrosat(cmd::macrosat::MacrosatArgs),
    Satwire(cmd::satwire::SatwireArgs),
    /// A systematic `door satwire` sweep (satsweep.rs, track sweep56): every ideal plan x every
    /// placement of a W x H wiring box (`--box-dims`) x every timing variant (`--variants
    /// base,gap,triple`): each variant is replayed with ideal inputs first (plans the mechanism
    /// cannot follow are INVALID), then SAT-wired; one TSV row per run (SAT / UNSAT + core / ...).
    Satsweep(cmd::satwire::SatsweepArgs),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::Boxproof(a) => cmd::boxproof::cmd_boxproof(a),
        DoorCmd::Macrosat(a) => cmd::macrosat::cmd_macrosat(a),
        DoorCmd::Satwire(a) => cmd::satwire::cmd_satwire(version, a),
        DoorCmd::Satsweep(a) => cmd::satwire::cmd_satsweep(version, a),
    }
}
