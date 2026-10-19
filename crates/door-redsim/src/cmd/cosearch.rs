//! `door cosearch`: the redsim co-search tools.

use anyhow::Result;

#[derive(clap::Args)]
pub struct CosearchArgs {
    #[command(subcommand)]
    cmd: crate::cosearch::CosearchCmd,
}

pub fn cmd_cosearch(version: &str, args: CosearchArgs) -> Result<()> {
    let CosearchArgs { cmd } = args;
    crate::cosearch::run(version, cmd)?;
    Ok(())
}
