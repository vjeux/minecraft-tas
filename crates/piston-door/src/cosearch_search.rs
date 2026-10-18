//! `mctas door cosearch search`: joint mechanism + wiring search in redsim (track cosearch).
//! (Filled in after the evaluator: see cosearch.rs for the judge it uses.)

use anyhow::Result;

#[derive(clap::Args, Clone, Debug)]
pub struct SearchArgs {
    /// Seed design(s).
    pub seeds: Vec<std::path::PathBuf>,
}

pub fn run(_version: &str, a: &SearchArgs) -> Result<()> {
    anyhow::bail!("search not implemented yet ({} seeds)", a.seeds.len())
}
