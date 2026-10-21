//! `door sheet`, `door ideal-rs`, `door feeds`: ideal schedules on redsim.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct FeedsArgs {
    /// The ideal schedule (its design is replayed on redsim, one open and one close).
    schedule: PathBuf,
    /// The build box in the door plane, `X0..X1,Y0..Y1` (design coordinates; default: the
    /// design's blocks): only cells inside it that stay free all cycle can feed.
    #[arg(long, value_name = "X0..X1,Y0..Y1")]
    r#box: Option<String>,
}

pub fn cmd_feeds(args: FeedsArgs) -> Result<()> {
    let bx = args.r#box.as_deref().map(crate::feeds::parse_box).transpose()?;
    crate::feeds::cli(&args.schedule, bx)
}

#[derive(clap::Args)]
pub struct PrunePlanArgs {
    /// The `door ideal` plan.
    schedule: PathBuf,
    /// Write the pruned plan here.
    #[arg(long)]
    out: PathBuf,
    /// Never drop a pulse of this input cell, `x,y,z` (repeatable).
    #[arg(long, allow_hyphen_values = true)]
    keep: Vec<String>,
    /// Drop pulses of these input cells only, `x,y,z` (repeatable): one area's load.
    #[arg(long, allow_hyphen_values = true)]
    only: Vec<String>,
    /// Try every subset of the candidate pulses (at most 16; narrow with --only), largest first,
    /// instead of the greedy pass: the most pulses that can go together.
    #[arg(long)]
    exhaustive: bool,
    #[arg(long, default_value_t = 24)]
    threads: usize,
    /// Extra pseudo-random positions every drop is confirmed at (screened at the 4 default ones).
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    cycles: u32,
}

/// `door prune-plan`: drop the input pulses a plan works without, together.
pub fn cmd_prune_plan(args: PrunePlanArgs) -> Result<()> {
    let PrunePlanArgs { schedule, out, keep, only, exhaustive, threads, extra_offsets, cycles } = args;
    let cells = |list: &[String]| -> Result<Vec<[i32; 3]>> {
        let mut out = Vec::new();
        for k in list {
            let v: Vec<i32> = k.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "x,y,z expected: {k}");
            out.push([v[0], v[1], v[2]]);
        }
        Ok(out)
    };
    crate::ideal_rs::prune(&schedule, &out, &cells(&keep)?, &cells(&only)?, exhaustive, threads, extra_offsets, cycles)
}

#[derive(clap::Args)]
pub struct ShiftPlanArgs {
    /// The `door ideal` plan.
    schedule: PathBuf,
    /// Move every event of this input cell, `x,y,z=k` (k ticks, may be negative) or
    /// `x,y,z=@name` (a variable, `let name = 0`, for `ideal-rs --sweep name=lo..hi`); repeatable.
    #[arg(long, allow_hyphen_values = true)]
    shift: Vec<String>,
    /// Move every event of the other cells: a number of ticks or `@name`.
    #[arg(long, default_value = "0", allow_hyphen_values = true)]
    rest: String,
    /// Write the shifted plan here (default: stdout).
    #[arg(long)]
    out: Option<PathBuf>,
}

/// `door shift-plan`: re-time a plan (one area's inputs later than the lever's, say).
pub fn cmd_shift_plan(args: ShiftPlanArgs) -> Result<()> {
    let ShiftPlanArgs { schedule, shift, rest, out } = args;
    let mut shifts = Vec::new();
    for s in &shift {
        let (c, k) = s.split_once('=').ok_or_else(|| anyhow::anyhow!("--shift x,y,z=k: {s}"))?;
        let v: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--shift x,y,z=k: {s}");
        shifts.push(([v[0], v[1], v[2]], k.trim().to_string()));
    }
    let text = std::fs::read_to_string(&schedule)?;
    let shifted = crate::ideal::Schedule::shift_text(&text, &shifts, rest.trim())?;
    match out {
        Some(p) => std::fs::write(p, shifted)?,
        None => print!("{shifted}"),
    }
    Ok(())
}

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
    /// Draw the door plane after every tick in `LO..HI` (open and close, ticks after each
    /// flip) of the first cycle at the first position: where carried pistons sit and what
    /// touches them when they fire.
    #[arg(long, value_name = "LO..HI")]
    planes: Option<String>,
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
    /// Drop every plan event at this input cell, `x,y,z` (repeatable): that input left to the
    /// design's own blocks (with `--vary`, a screen of real fillings), the others ideal.
    #[arg(long)]
    without: Vec<String>,
}

pub fn cmd_ideal_rs(args: IdealRsArgs) -> Result<()> {
    let IdealRsArgs { schedule, compact_dir, set, sweep, independent, extra_offsets, cycles, trace, extent, planes, threads, vary, ok_only, drop, holds, between_ticks, holds_rank, offplane_dir, offplane_below, without } = args;
    let without: Vec<[i32; 3]> = without
        .iter()
        .map(|c| {
            let v: Vec<i32> = c.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "--without x,y,z, got {c:?}");
            Ok([v[0], v[1], v[2]])
        })
        .collect::<Result<_>>()?;
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
    let planes = match planes {
        Some(r) => {
            let (lo, hi) = r.split_once("..").ok_or_else(|| anyhow::anyhow!("--planes LO..HI"))?;
            Some((lo.parse::<u32>()?, hi.parse::<u32>()?))
        }
        None => None,
    };
    let trace = crate::ideal_rs::Trace { changes: trace, planes };
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
            crate::ideal_rs::cli(&schedule[0], &set, &sweep, independent, extra_offsets, cycles, trace, threads, &vary, ok_only, drop, holds, &without)?;
        }
    }
    Ok(())
}
