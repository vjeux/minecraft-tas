//! `door sheet`, `door ideal-rs`: ideal schedules on redsim.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct SheetArgs {
    #[arg(required = true, num_args = 1..)]
    files: Vec<PathBuf>,
    /// Write the page here instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
    /// Page title.
    #[arg(long)]
    heading: Option<String>,
    /// Also test static stone in the frame cells left empty when open (2 cycles in redsim).
    #[arg(long)]
    fill: bool,
    /// With --fill: write each working filled mechanism (`<name>-ff.txt` + `.ideal`) here.
    #[arg(long)]
    fill_dir: Option<PathBuf>,
    /// Keep only mechanisms whose single piston below the opening is a sticky piston facing up.
    #[arg(long)]
    single_up_below: bool,
}

pub fn cmd_sheet(args: SheetArgs) -> Result<()> {
    let SheetArgs { files, out, heading, fill, fill_dir, single_up_below } = args;
    crate::sheet::cli(&files, out.as_deref(), heading.as_deref(), fill, fill_dir.as_deref(), single_up_below)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct IdealRsArgs {
    /// One schedule (several with `--compact-dir`).
    #[arg(required = true)]
    schedule: Vec<PathBuf>,
    /// Compact each schedule (every pulse / level edge as early as it goes, the others
    /// fixed, two passes) and write `<dir>/<name>-cpt.ideal` with each group's window.
    #[arg(long)]
    compact_dir: Option<PathBuf>,
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
    /// Print every block change of the first cycle at the first position.
    #[arg(long)]
    trace: bool,
    /// Print the door-plane bounding box (and volume) of every non-air cell over one replayed
    /// cycle at the plan's own position, its input cells and lever excluded: what the judge's
    /// volume will be once the inputs are real wiring inside that box.
    #[arg(long)]
    extent: bool,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// Block variants, `x,y[,z]=state|state|...` (repeatable: every combination, times the
    /// --sweep grid): the design's cell is replaced in memory before each run (`air`
    /// clears it, `keep` leaves the design's block, `minecraft:` is optional).
    #[arg(long)]
    vary: Vec<String>,
    /// Print only the combinations that work (and a count on stderr).
    #[arg(long)]
    ok_only: bool,
    /// Also try every subset of up to N blocks removed (door cells, the lever and piston
    /// heads excepted), on top of each --vary combination: a redsim block minimizer.
    #[arg(long, default_value_t = 0)]
    drop: usize,
    /// For every ideal input of the plan, try what a sculk sensor can give it instead (a 10
    /// or 30 tick hold, 3 taps 10 apart, 2 taps 14 apart, one pulse), every start 0..=N on
    /// each edge (or none), and print which starts work: which inputs a sensor could drive.
    #[arg(long)]
    holds: Option<i64>,
    /// The old input rule: plan blocks appear between ticks, after tick t (default: in tick
    /// t's tile-tick phase, where an observer's output arrives; see ideal.rs).
    #[arg(long)]
    between_ticks: bool,
    /// Screen many schedules for sensor wake-ups: for every ideal input, which sensor modes
    /// (hold10, hold30, taps3, taps2) work with the SAME start on both edges, starts 0..=N
    /// (others at their plan events); one line per schedule: inputs, sensor-friendly
    /// inputs with their modes and starts, exact-only inputs. Sorted by exact-only count.
    #[arg(long)]
    holds_rank: Option<i64>,
    /// Write each schedule to this directory with its in-plane inputs below the opening
    /// (rows < door.y + --offplane-below) moved behind the pistons they feed (z+1), and
    /// print the `satwire --keep-ideal` cell list: the sealed bottom left to sensors.
    #[arg(long)]
    offplane_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    offplane_below: i32,
}

pub fn cmd_ideal_rs(args: IdealRsArgs) -> Result<()> {
    let IdealRsArgs { schedule, compact_dir, set, sweep, independent, extra_offsets, cycles, trace, extent, threads, vary, ok_only, drop, holds, between_ticks, holds_rank, offplane_dir, offplane_below } = args;
    if extent {
        crate::ideal::cli_phase(between_ticks);
        for path in &schedule {
            let text = std::fs::read_to_string(path)?;
            let s = crate::ideal::Schedule::parse_with(&text, &std::collections::BTreeMap::new())?;
            match crate::ideal_rs::extent(&s, s.vars()) {
                Ok((lo, hi)) => {
                    let (w, h) = (hi[0] - lo[0] + 1, hi[1] - lo[1] + 1);
                    println!("{}: extent x {}..{} y {}..{} = {w}x{h} ({} cells), inputs and lever excluded", path.display(), lo[0], hi[0], lo[1], hi[1], w * h);
                }
                Err(e) => println!("{}: {e}", path.display()),
            }
        }
        return Ok(());
    }
    match { crate::ideal::cli_phase(between_ticks); (compact_dir, holds_rank) } {
        _ if offplane_dir.is_some() => crate::ideal_rs::offplane_dir(&schedule, offplane_dir.as_deref().unwrap(), offplane_below)?,
        (_, Some(max_tick)) => crate::ideal_rs::holds_rank(&schedule, extra_offsets, cycles, threads, max_tick)?,
        (Some(dir), None) => {
            std::fs::create_dir_all(&dir)?;
            for s in &schedule {
                let stem = s.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                if let Err(e) = crate::ideal_rs::compact(s, &dir.join(format!("{stem}-cpt.ideal")), extra_offsets, cycles, threads) {
                    println!("{}: {e:#}", s.display());
                }
            }
        }
        (None, None) => {
            anyhow::ensure!(schedule.len() == 1, "one schedule (several only with --compact-dir or --holds-rank)");
            crate::ideal_rs::cli(&schedule[0], &set, &sweep, independent, extra_offsets, cycles, trace, threads, &vary, ok_only, drop, holds)?;
        }
    }
    Ok(())
}
