//! `door rows-fire`, `door drive`, `door ideal`: mechanisms driven by scripts and ideal inputs.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct RowsFireArgs {
    tsv: PathBuf,
    /// Only rows with no firing from x 0..2.
    #[arg(long)]
    wirable: bool,
    /// Only ids starting with one of these (e.g. `r046,r075`).
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
}

pub fn cmd_rows_fire(args: RowsFireArgs) -> Result<()> {
    let RowsFireArgs { tsv, wirable, only } = args;
    crate::rowsfire::cli(&tsv, wirable, &only)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct DriveArgs {
    #[arg(required = true)]
    scripts: Vec<PathBuf>,
    /// Print only one PASS/FAIL line per script (a gadget test run).
    #[arg(long)]
    quiet: bool,
}

pub fn cmd_drive(version: &str, args: DriveArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let DriveArgs { scripts, quiet } = args;
    let mut w = Worker::spawn(&cfg)?;
    let mut failed = 0;
    for script in &scripts {
        let text = std::fs::read_to_string(script)?;
        match crate::drive::run_opts(&mut w, &text, !quiet) {
            Ok(o) if o.failures.is_empty() => {
                if quiet || o.checks > 0 {
                    println!("PASS {} ({} checks)", script.display(), o.checks);
                }
            }
            Ok(o) => {
                failed += 1;
                println!("FAIL {} ({} of {} checks failed)", script.display(), o.failures.len(), o.checks);
                for f in &o.failures {
                    println!("  {f}");
                }
            }
            Err(e) => {
                failed += 1;
                println!("FAIL {}: {e:#}", script.display());
            }
        }
    }
    if failed > 0 {
        anyhow::bail!("{failed} of {} scripts failed", scripts.len());
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct IdealArgs {
    schedule: PathBuf,
    /// Override a schedule variable, name=value (repeatable).
    #[arg(long)]
    set: Vec<String>,
    /// Sweep a variable, name=lo..hi (repeatable: every combination).
    #[arg(long)]
    sweep: Vec<String>,
    /// Sweep each --sweep variable on its own (others at their values) instead of the grid.
    #[arg(long)]
    independent: bool,
    /// Extra pseudo-random positions on top of the default four.
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    cycles: u32,
    /// Print the plane z,x0,y0,x1,y1 every tick of the first cycle.
    #[arg(long)]
    trace: Option<String>,
    /// Worker JVMs for sweeps.
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// The old input rule: plan blocks appear between ticks, after tick t (default: in tick
    /// t's tile-tick phase, where an observer's output arrives; see ideal.rs).
    #[arg(long)]
    between_ticks: bool,
}

pub fn cmd_ideal(version: &str, args: IdealArgs) -> Result<()> {
    let IdealArgs { schedule, set, sweep, independent, extra_offsets, cycles, trace, workers, between_ticks } = args;
    crate::ideal::cli_phase(between_ticks);
    crate::ideal::cli(version, &schedule, &set, &sweep, independent, extra_offsets, cycles, trace, workers)?;
    Ok(())
}
