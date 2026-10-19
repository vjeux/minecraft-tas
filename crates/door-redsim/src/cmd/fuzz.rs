//! `door redsim-fuzz`, `door redsim-case`, `door pistfuzz`: differential fuzzing of redsim
//! against the game.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct RedsimFuzzArgs {
    /// Random circuits to run.
    #[arg(long, default_value_t = 1000)]
    n: usize,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// Box size x,y,z.
    #[arg(long, default_value = "6,3,6")]
    size: String,
    /// Chance that a cell of the box gets a component.
    #[arg(long, default_value_t = 0.45)]
    density: f64,
    /// Families with weights: `core`, `extra`, `all`, or `<family>[:w],...` (dust, repeater,
    /// comparator, torch, walltorch, observer, rblock, lamp, bulb, note, target, lever, button,
    /// stone, glass, simple, trapdoor, gate, skull, rail, dispenser, crafter, hopper, slab,
    /// leaves, log, analog, door).
    #[arg(long, default_value = "core")]
    palette: String,
    /// Where shrunk mismatch cases go.
    #[arg(long, default_value = "designs/redsim/fails")]
    out: PathBuf,
    /// Do not shrink mismatches.
    #[arg(long)]
    no_shrink: bool,
    /// Shrink and save at most this many mismatches.
    #[arg(long, default_value_t = 8)]
    max_report: usize,
    #[arg(long, default_value_t = 8)]
    workers: usize,
}

pub fn cmd_redsim_fuzz(version: &str, args: RedsimFuzzArgs) -> Result<()> {
    let RedsimFuzzArgs { n, seed, size, density, palette, out, no_shrink, max_report, workers } = args;
    let sz: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
    anyhow::ensure!(sz.len() == 3, "--size x,y,z");
    let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })?;
    let o = redsim_fuzz::FuzzOptions {
        n,
        seed,
        size: [sz[0], sz[1], sz[2]],
        density,
        palette: redsim_fuzz::palette(&palette)?,
        out,
        shrink: !no_shrink,
        max_report,
        generator: "random".into(),
    };
    redsim_fuzz::fuzz(&pool, &o, &|s| eprintln!("{s}"))?;
    Ok(())
}

#[derive(clap::Args)]
pub struct RedsimCaseArgs {
    case: PathBuf,
    #[arg(long, default_value_t = 3)]
    max: usize,
    /// Write it as a redsim regression fixture (the game's trace as the expectation).
    #[arg(long)]
    fixture: Option<PathBuf>,
    /// One-line description for the fixture.
    #[arg(long, default_value = "")]
    note: String,
}

pub fn cmd_redsim_case(version: &str, args: RedsimCaseArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let RedsimCaseArgs { case, max, fixture, note } = args;
    let mut w = Worker::spawn(&cfg)?;
    let same = redsim_fuzz::replay(&mut w, &case, max)?;
    if let Some(path) = fixture {
        anyhow::ensure!(same || !note.is_empty(), "writing a fixture of a mismatching case needs a --note");
        let c = redsim_fuzz::Case::parse(&std::fs::read_to_string(&case)?)?;
        let text = redsim_fuzz::fixture(&mut w, &c, &note)?;
        std::fs::write(&path, text)?;
        eprintln!("fixture written to {}", path.display());
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct PistfuzzArgs {
    #[arg(long, default_value_t = 100)]
    count: u64,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    /// flat, box or mix.
    #[arg(long, default_value = "mix")]
    mode: String,
    /// Slime and honey in the palette.
    #[arg(long)]
    slime: bool,
    /// Sand, gravel, red sand in the palette.
    #[arg(long)]
    gravity: bool,
    /// Observers in the palette.
    #[arg(long)]
    observers: bool,
    /// Redstone components next to the pistons (dust, torches, repeaters, comparators,
    /// levers, lamps, note blocks, targets), placed like the judge does.
    #[arg(long)]
    mixed: bool,
    /// Shaped blocks among the solids: slabs, trapdoors, scaffolding, fence gates.
    #[arg(long)]
    shapes: bool,
    /// Replay every gadget drive script of designs/gadgets (entity gadgets skipped).
    #[arg(long)]
    gadgets: bool,
    /// Replay door designs: placement, settle, then lever flips (`--cycles`).
    #[arg(long, num_args = 1..)]
    designs: Vec<PathBuf>,
    #[arg(long, default_value_t = 2)]
    cycles: u32,
    /// What the bridge is compared with: `model` (redsim) or `bridge` (a second run: the
    /// harness's own determinism).
    #[arg(long, default_value = "model")]
    against: String,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// Shrink every failure to a minimal scenario.
    #[arg(long)]
    shrink: bool,
    /// Replay scenario files instead of generating.
    #[arg(long, num_args = 1..)]
    replay: Vec<PathBuf>,
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/pistmodel/fuzz")]
    out_dir: PathBuf,
    /// Stop after this many failures.
    #[arg(long, default_value_t = 20)]
    max_fail: usize,
    /// Print every tick's block changes of the bridge run (for single replays).
    #[arg(long)]
    trace: bool,
    /// Sculk scenarios (track sculk): sensors, calibrated sensors with frequency filters,
    /// wool, amethyst, comparators and observers on sensors, among pistons and wiring.
    #[arg(long)]
    sculk: bool,
    /// With --sculk: boxes anywhere in the sandbox (its edge chunks included).
    #[arg(long)]
    border: bool,
}

pub fn cmd_pistfuzz(version: &str, args: PistfuzzArgs) -> Result<()> {
    let PistfuzzArgs { count, seed, mode, slime, gravity, observers, mixed, shapes, gadgets, designs, cycles, against, workers, shrink, replay, out_dir, max_fail, trace, sculk, border } = args;
    let o = crate::pistfuzz::CliOpts { count, seed, mode, slime, gravity, observers, mixed, shapes, against, workers, shrink, replay, gadgets, designs, cycles, out_dir, max_fail, trace, sculk, border };
    crate::pistfuzz::cli(version, &o)?;
    Ok(())
}
