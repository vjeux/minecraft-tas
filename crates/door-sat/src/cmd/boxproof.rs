//! `door boxproof`: whole-box bounded model checking.

use anyhow::Result;

#[derive(clap::Args)]
pub struct BoxproofArgs {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

pub fn cmd_boxproof(args: BoxproofArgs) -> Result<()> {
    let BoxproofArgs { args } = args;
    crate::boxproof::cli(&args)
}
