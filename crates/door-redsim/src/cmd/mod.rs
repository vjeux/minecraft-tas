//! This crate's commands, one module per family: each command's clap arguments (`<Name>Args`,
//! its `DoorCmd` variant holds them) and its body (`cmd_<name>`, which `crate::run` calls).

pub mod cosearch;
pub mod fill;
pub mod fuzz;
pub mod ideal;
pub mod judge;
