//! `mctas door ...`: the 3x3 piston door commands. They live in one crate per area
//! (crates/door-*), each with its own `DoorCmd` and `run`, flattened here into one command list,
//! so every command line stays `mctas door <command>` while a change to one area's commands only
//! rebuilds that crate and this binary (one giant enum and match made every build ~2 minutes).
//! A new command goes in the area crate it belongs to.

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum DoorCmd {
    #[command(flatten)]
    Judge(door_judge::DoorCmd),
    #[command(flatten)]
    Search(door_search::DoorCmd),
    #[command(flatten)]
    Mech(door_mech::DoorCmd),
    #[command(flatten)]
    Sat(door_sat::DoorCmd),
    #[command(flatten)]
    Redsim(door_redsim::DoorCmd),
    #[command(flatten)]
    Vanilla(door_vanilla::DoorCmd),
    #[command(flatten)]
    Video(door_video::DoorCmd),
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::Judge(c) => door_judge::run(version, c),
        DoorCmd::Search(c) => door_search::run(version, c),
        DoorCmd::Mech(c) => door_mech::run(version, c),
        DoorCmd::Sat(c) => door_sat::run(version, c),
        DoorCmd::Redsim(c) => door_redsim::run(version, c),
        DoorCmd::Vanilla(c) => door_vanilla::run(version, c),
        DoorCmd::Video(c) => door_video::run(version, c),
    }
}
