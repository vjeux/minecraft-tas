//! `door vanilla-check`, `door vanilla-build`, `door vanilla-gadget`, `door vanilla-fuzz`:
//! cross-checks in the unmodified official server.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct VanillaCheckArgs {
    designs: Vec<PathBuf>,
    #[arg(long, default_value_t = 3)]
    cycles: u32,
    /// Extra pseudo-random positions on top of the default four (the judge uses 28).
    #[arg(long, default_value_t = 4)]
    extra_offsets: u32,
    /// Only the first N positions (quick runs).
    #[arg(long)]
    positions: Option<usize>,
    /// Check at this one position only, "x,y,z" (reproduce a fuzz case or one judge offset).
    #[arg(long)]
    offset: Option<String>,
    #[arg(long, default_value_t = 400)]
    max_ticks: u32,
    /// Print every position, not only divergences and failures.
    #[arg(long)]
    verbose: bool,
    /// Keep the vanilla world directory (path printed at the end).
    #[arg(long)]
    keep_world: bool,
    /// `/tick rate` of the FROZEN server: only how often its idle loop wakes up, i.e. how soon
    /// a `tick step` can start; what a tick does is unchanged (20 = vanilla's default gave
    /// identical frames on every design tried). The real-time cycle always runs at 20.
    #[arg(long, default_value_t = 100)]
    tick_rate: u32,
    /// Skip the real-time cycle (unfrozen 20 tps, at the first position).
    #[arg(long)]
    no_free_run: bool,
    /// Record pending ticks/events after every tick in both worlds; print them for diverging positions.
    #[arg(long)]
    trace: bool,
}

pub fn cmd_vanilla_check(version: &str, args: VanillaCheckArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let VanillaCheckArgs { designs, cycles, extra_offsets, positions, offset, max_ticks, verbose, keep_world, tick_rate, no_free_run, trace } = args;
    vanilla_check_cmd(version, &cfg, &designs, cycles, extra_offsets, positions, offset, max_ticks, verbose, keep_world, tick_rate, no_free_run, trace)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct VanillaBuildArgs {
    scripts: Vec<String>,
    /// The design (file, `@builtin` or `<git ref>:<path>`); default: the script's `# source:`.
    #[arg(long)]
    design: Option<String>,
    #[arg(long, default_value_t = 3)]
    cycles: u32,
    /// Extra pseudo-random positions on top of the judge's default four.
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    /// Only the first N positions.
    #[arg(long)]
    positions: Option<usize>,
    /// One position only, "x,y,z".
    #[arg(long)]
    offset: Option<String>,
    #[arg(long, default_value_t = 400)]
    max_ticks: u32,
    /// Replay only the first N actions and compare (a minimal repro of a divergence).
    #[arg(long)]
    until_step: Option<usize>,
    /// Print every position, not only failures.
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    keep_world: bool,
    /// `/tick rate` of the frozen server (see vanilla-check).
    #[arg(long, default_value_t = 100)]
    tick_rate: u32,
    /// Build with the game RUNNING at 20 tps (the bot acts while time passes, waiting the
    /// simulator's ticks + 10 after each action); the cycles still run frozen, tick for tick.
    #[arg(long)]
    real_time: bool,
    /// A slower player: N extra idle ticks after every action, in both worlds (stepped).
    #[arg(long, default_value_t = 0)]
    slow: u32,
    /// Keep the bot at its click spot while the world ticks (a player standing by; by
    /// default it waits out of every sculk sensor's range, as the simulator has no player).
    #[arg(long)]
    stay: bool,
}

pub fn cmd_vanilla_build(version: &str, args: VanillaBuildArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let VanillaBuildArgs { scripts, design, cycles, extra_offsets, positions, offset, max_ticks, until_step, verbose, keep_world, tick_rate, real_time, slow, stay } = args;
    vanilla_build_cmd(version, &cfg, &scripts, design, cycles, extra_offsets, positions, offset, max_ticks, until_step, verbose, keep_world, tick_rate, real_time, slow, stay)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct VanillaGadgetArgs {
    scripts: Vec<PathBuf>,
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    /// Only the first N positions.
    #[arg(long)]
    positions: Option<usize>,
    #[arg(long)]
    verbose: bool,
    #[arg(long)]
    keep_world: bool,
    #[arg(long, default_value_t = 100)]
    tick_rate: u32,
}

pub fn cmd_vanilla_gadget(version: &str, args: VanillaGadgetArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let VanillaGadgetArgs { scripts, extra_offsets, positions, verbose, keep_world, tick_rate } = args;
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    if let Some(n) = positions {
        offsets.truncate(n.max(1));
    }
    let mut w = Worker::spawn(&cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), keep_dir: keep_world, tick_rate, ..Default::default() };
    let t0 = std::time::Instant::now();
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    eprintln!("vanilla server up in {:.1}s ({})", t0.elapsed().as_secs_f64(), v.dir.display());
    let ok = crate::vanilla_gadget::run_cli(&mut w, &mut v, &scripts, &offsets, verbose)?;
    if keep_world {
        println!("world kept in {}", v.dir.display());
    }
    drop(v);
    if !ok {
        std::process::exit(1);
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct VanillaFuzzArgs {
    #[arg(long, default_value_t = 50)]
    count: u64,
    #[arg(long, default_value_t = 1)]
    seed: u64,
    #[arg(long, default_value_t = 1)]
    cycles: u32,
    #[arg(long, default_value_t = 120)]
    max_ticks: u32,
    /// Where diverging designs are written.
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/vanilla-check/fuzz")]
    out_dir: PathBuf,
}

pub fn cmd_vanilla_fuzz(version: &str, args: VanillaFuzzArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let VanillaFuzzArgs { count, seed, cycles, max_ticks, out_dir } = args;
    vanilla_fuzz_cmd(version, &cfg, count, seed, cycles, max_ticks, &out_dir)?;
    Ok(())
}

/// `door vanilla-check`: one vanilla server and one simulator worker for all the designs.
#[allow(clippy::too_many_arguments)]
fn vanilla_check_cmd(
    version: &str,
    cfg: &WorkerConfig,
    designs: &[PathBuf],
    cycles: u32,
    extra_offsets: u32,
    positions: Option<usize>,
    offset: Option<String>,
    max_ticks: u32,
    verbose: bool,
    keep_world: bool,
    tick_rate: u32,
    no_free_run: bool,
    trace: bool,
) -> Result<()> {
    use crate::vanilla_check::{CheckOptions, check, verdict_str};
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    if let Some(n) = positions {
        offsets.truncate(n.max(1));
    }
    if let Some(o) = offset {
        let v: Vec<i32> = o.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--offset wants x,y,z");
        offsets = vec![[v[0], v[1], v[2]]];
    }
    let opts = CheckOptions { cycles, offsets, max_ticks, verbose, free_run: !no_free_run, trace };
    let mut w = Worker::spawn(cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), keep_dir: keep_world, tick_rate, ..Default::default() };
    let t0 = std::time::Instant::now();
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    eprintln!("vanilla server up in {:.1}s ({})", t0.elapsed().as_secs_f64(), v.dir.display());
    let mut any_bad = false;
    let mut oracle = crate::vanilla_check::Oracle::default();
    for path in designs {
        let d = load_design(path)?;
        let t = std::time::Instant::now();
        let name = path.display().to_string();
        match check(&mut w, &mut v, &mut oracle, &d, &opts) {
            Ok(results) => {
                let n = results.len();
                let ticks: usize = results.iter().map(|r| r.ticks_compared).sum();
                let diverging: Vec<_> = results.iter().filter(|r| r.divergence.is_some() || r.sim.is_err() != r.vanilla.is_err()).collect();
                let vanilla_fail = results.iter().find(|r| r.vanilla.is_err());
                let free_fail = results.iter().find_map(|r| r.free_run.as_ref().and_then(|f| f.as_ref().err()).map(|e| (r.offset, e.clone())));
                let free_ok = results.iter().any(|r| matches!(r.free_run, Some(Ok(()))));
                let line = if let Some(r) = diverging.first() {
                    any_bad = true;
                    format!(
                        "DIVERGES at {}/{n} positions; first at offset {:?}: {} | sim {} | vanilla {}",
                        diverging.len(),
                        r.offset,
                        r.divergence.as_deref().unwrap_or("verdicts differ"),
                        verdict_str(&r.sim),
                        verdict_str(&r.vanilla)
                    )
                } else if let Some((off, e)) = free_fail {
                    any_bad = true;
                    format!("REAL-TIME CYCLE DIFFERS at offset {off:?} (stepped runs identical): {e}")
                } else if let Some(r) = vanilla_fail {
                    any_bad = true;
                    format!("FAIL in vanilla (the simulator agrees, frames identical) at offset {:?}: {}", r.offset, verdict_str(&r.vanilla))
                } else {
                    format!(
                        "PASS: vanilla matches the simulator tick for tick ({ticks} ticks at {n} positions x {cycles} cycles{}); {}",
                        if free_ok { ", plus a real-time 20 tps cycle" } else { "" },
                        verdict_str(&results[0].vanilla)
                    )
                };
                println!("vanilla-check: {name} ({} blocks, {}) {line} [{:.0}s]", crate::block_count(&d), crate::category(&d), t.elapsed().as_secs_f64());
                if verbose {
                    let s = &v.stats;
                    eprintln!(
                        "  vanilla time: {} steps {:.1}s ({:.0} ms each), {} snapshots {:.1}s ({:.0} ms each)",
                        s.steps,
                        s.step.as_secs_f64(),
                        s.step.as_secs_f64() * 1000.0 / s.steps.max(1) as f64,
                        s.snapshots,
                        s.snapshot.as_secs_f64(),
                        s.snapshot.as_secs_f64() * 1000.0 / s.snapshots.max(1) as f64
                    );
                }
            }
            Err(e) => {
                any_bad = true;
                println!("vanilla-check: {name} ERROR: {e:#}");
            }
        }
    }
    if keep_world {
        println!("world kept in {}", v.dir.display());
    }
    drop(v);
    if any_bad {
        std::process::exit(1);
    }
    Ok(())
}

/// `door vanilla-build`: replay build scripts in the vanilla server with the bot.
#[allow(clippy::too_many_arguments)]
fn vanilla_build_cmd(
    version: &str,
    cfg: &WorkerConfig,
    scripts: &[String],
    design: Option<String>,
    cycles: u32,
    extra_offsets: u32,
    positions: Option<usize>,
    offset: Option<String>,
    max_ticks: u32,
    until_step: Option<usize>,
    verbose: bool,
    keep_world: bool,
    tick_rate: u32,
    real_time: bool,
    slow: u32,
    stay: bool,
) -> Result<()> {
    use crate::vanilla_build::{BuildOptions, check, parse_script};
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    if let Some(n) = positions {
        offsets.truncate(n.max(1));
    }
    if let Some(o) = offset {
        let v: Vec<i32> = o.split(',').map(|t| t.trim().parse()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "--offset wants x,y,z");
        offsets = vec![[v[0], v[1], v[2]]];
    }
    let opts = BuildOptions { cycles, offsets, max_ticks, until_step, verbose, real_time, slow, stay };
    let mut w = Worker::spawn(cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), keep_dir: keep_world, tick_rate, ..Default::default() };
    let t0 = std::time::Instant::now();
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    eprintln!("vanilla server up in {:.1}s ({})", t0.elapsed().as_secs_f64(), v.dir.display());
    let mut oracle = crate::vanilla_check::Oracle::default();
    let mut any_bad = false;
    for path in scripts {
        let t = std::time::Instant::now();
        let r = (|| -> Result<String> {
            let script = parse_script(path, &std::fs::read_to_string(path)?)?;
            let spec = anyhow::Context::context(design.clone().or(script.source.clone()), "the script names no `# source:` design (pass --design)")?;
            let d = crate::buildable::load_any(&spec)?;
            let results = check(&mut w, &mut v, &mut oracle, &d, &script, &opts)?;
            let n = results.len();
            let bad: Vec<_> = results.iter().filter(|r| !r.pass()).collect();
            let actions = results.first().map(|r| r.steps).unwrap_or(0);
            let ray: usize = results.iter().map(|r| r.stats.clicks_on_ray).sum();
            let off_ray: usize = results.iter().map(|r| r.stats.clicks_off_ray).sum();
            let anchors: usize = results.iter().map(|r| r.stats.dirt_on_anchor).sum();
            let ticks: usize = results.iter().map(|r| r.ticks_compared).sum();
            let summary = format!(
                "{} blocks, {}; {actions} actions x {n} positions ({ray} clicks from the view ray, {off_ray} off it, {anchors} temporary dirt on an anchor)",
                crate::block_count(&d),
                crate::category(&d)
            );
            Ok(if let Some(r) = bad.first() {
                any_bad = true;
                format!(
                    "FAIL at {}/{n} positions ({summary}); first at offset {:?}: {}",
                    bad.len(),
                    r.offset,
                    r.step_divergence
                        .clone()
                        .map(|d| format!("action diverges {d}"))
                        .or_else(|| r.built_vs_rest.clone().map(|d| format!("built != judge rest: {d}")))
                        .or_else(|| r.divergence.clone())
                        .unwrap_or_else(|| format!("sim {} | vanilla {}", crate::vanilla_check::verdict_str(&r.sim), crate::vanilla_check::verdict_str(&r.vanilla)))
                )
            } else {
                format!(
                    "PASS ({summary}): every action left vanilla identical to the simulator, the built world equals the judge's rest state, cycles identical tick for tick ({ticks} ticks); {}",
                    crate::vanilla_check::verdict_str(&results[0].vanilla)
                )
            })
        })();
        match r {
            Ok(line) => println!("vanilla-build: {path} {line} [{:.0}s]", t.elapsed().as_secs_f64()),
            Err(e) => {
                any_bad = true;
                println!("vanilla-build: {path} ERROR: {e:#}");
            }
        }
    }
    if keep_world {
        println!("world kept in {}", v.dir.display());
    }
    drop(v);
    if any_bad {
        std::process::exit(1);
    }
    Ok(())
}

/// `door vanilla-fuzz`: random contraptions, settled in the simulator, compared tick for tick.
fn vanilla_fuzz_cmd(version: &str, cfg: &WorkerConfig, count: u64, seed: u64, cycles: u32, max_ticks: u32, out_dir: &std::path::Path) -> Result<()> {
    use crate::vanilla_check::{CheckOptions, Oracle, check, fuzz_design, verdict_str};
    let mut w = Worker::spawn(cfg)?;
    let vopts = mctas::vanilla::VanillaOptions { version: version.to_string(), tick_rate: 100, ..Default::default() };
    let mut v = mctas::vanilla::VanillaServer::start(&vopts)?;
    let mut oracle = Oracle::default();
    let offsets = crate::extra_offsets(32);
    let (mut same, mut skipped, mut diverging, mut errors) = (0, 0, 0, 0);
    for s in seed..seed + count {
        let raw = fuzz_design(s);
        let Some(canon) = crate::vanilla_check::rest_state(&mut w, &raw)? else {
            skipped += 1;
            continue;
        };
        let lever = canon.ports["lever"];
        if !canon.blocks.get(&lever).is_some_and(|b| b.starts_with("minecraft:lever")) {
            skipped += 1;
            continue;
        }
        let opts = CheckOptions { cycles, offsets: vec![offsets[(s % 32) as usize]], max_ticks, verbose: false, free_run: false, trace: true };
        match check(&mut w, &mut v, &mut oracle, &canon, &opts) {
            Ok(results) => {
                let r = &results[0];
                if r.divergence.is_some() || r.sim.is_err() != r.vanilla.is_err() {
                    diverging += 1;
                    std::fs::create_dir_all(out_dir)?;
                    let path = out_dir.join(format!("fuzz-{s}.txt"));
                    canon.save(&path)?;
                    println!(
                        "fuzz {s}: DIVERGES at offset {:?}: {} | sim {} | vanilla {} (saved {})",
                        r.offset,
                        r.divergence.as_deref().unwrap_or("verdicts differ"),
                        verdict_str(&r.sim),
                        verdict_str(&r.vanilla),
                        path.display()
                    );
                } else {
                    same += 1;
                    println!("fuzz {s}: identical ({} ticks, {} blocks)", r.ticks_compared, canon.blocks.len());
                }
            }
            Err(e) => {
                errors += 1;
                std::fs::create_dir_all(out_dir)?;
                let path = out_dir.join(format!("fuzz-{s}-error.txt"));
                canon.save(&path)?;
                println!("fuzz {s}: ERROR {e:#} (saved {})", path.display());
            }
        }
    }
    println!("vanilla-fuzz: {same} identical, {diverging} diverging, {skipped} skipped (never settle / lever lost), {errors} errors");
    Ok(())
}
