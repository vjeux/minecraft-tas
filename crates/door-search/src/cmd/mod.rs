//! This crate's commands, one module per family: each command's clap arguments (`<Name>Args`,
//! its `DoorCmd` variant holds them) and its body (`cmd_<name>`, which `crate::run` calls).

pub mod grow;
pub mod minimize;
pub mod route;
pub mod synth;
