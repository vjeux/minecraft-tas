//! `door macrosat`: macro-level bounded model checking of flat doors (free layout, one settled
//! piston action per step, optional feed condition at every firing).

use anyhow::Result;

#[derive(clap::Args)]
pub struct MacrosatArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

pub fn cmd_macrosat(args: MacrosatArgs) -> Result<()> {
    let MacrosatArgs { args } = args;
    crate::macrosat::cli(&args)
}
