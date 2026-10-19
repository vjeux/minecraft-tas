//! `door route`, `door place`, `door route2`: routing and placing wiring.

use anyhow::Result;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::{SearchArgs, load_design};
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct RouteArgs {
    design: PathBuf,
    /// Start cells (existing dust of the net), "x,y,z;x,y,z".
    #[arg(long)]
    start: String,
    /// Goal cell (becomes dust).
    #[arg(long)]
    goal: String,
    /// Extra cells the path may touch, "x,y,z;...".
    #[arg(long, default_value = "")]
    allow: String,
    /// Take the swept cells from this drive script (ideal inputs) instead of flipping the
    /// design's lever: for wiring a mechanism that does not run on its own yet.
    #[arg(long)]
    swept_script: Option<PathBuf>,
    /// Take the swept cells from this working design instead (e.g. the design before a net
    /// was cut out of it to be re-laid elsewhere): only the cells its moving parts visit, so
    /// the cut net's own cells are free again.
    #[arg(long)]
    swept_design: Option<PathBuf>,
    /// Append the path to this file as `door patch` lines (`set x y z <state>`) instead of
    /// printing builder code.
    #[arg(long)]
    patch: Option<PathBuf>,
}

pub fn cmd_route(version: &str, args: RouteArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let RouteArgs { design, start, goal, allow, swept_script, swept_design, patch } = args;
    let d = load_design(&design)?;
    let cells = |s: &str| -> Vec<[i32; 3]> {
        s.split(';').filter(|x| !x.trim().is_empty()).map(|c| {
            let v: Vec<i32> = c.split(',').map(|n| n.trim().parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        }).collect()
    };
    let spec = crate::DoorSpec::from_design(&d)?;
    let mut w = Worker::spawn(&cfg)?;
    let mut reserved = match (&swept_script, &swept_design) {
        (Some(s), _) => crate::drive::swept(&mut w, &std::fs::read_to_string(s)?)?,
        (None, Some(sd)) => {
            let sd = load_design(sd)?;
            let sspec = crate::DoorSpec::from_design(&sd)?;
            crate::route::swept_moving_cells(&mut w, &sd, &sspec)?
        }
        (None, None) => crate::route::swept_cells(&mut w, &d, &spec)?,
    };
    reserved.retain(|p| !d.blocks.contains_key(p));
    reserved.extend(crate::route::keep_clear(&spec));
    reserved.insert([1, 3, 1]);
    let r = crate::route::Router { blocks: &d.blocks, reserved, allow: cells(&allow).into_iter().collect(), lo: [-12, -6, -8], hi: [12, 12, 10], floor_only: Default::default() };
    let goal = cells(&goal)[0];
    match r.route(&cells(&start), goal) {
        Some(path) => {
            if let Some(file) = &patch {
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new().create(true).append(true).open(file)?;
                writeln!(f, "# route to {},{},{} ({} new blocks)", goal[0], goal[1], goal[2], path.len())?;
                for (p, s) in path.iter().rev() {
                    writeln!(f, "set {} {} {} {}", p[0], p[1], p[2], s)?;
                }
                println!("{} new blocks appended to {}", path.len(), file.display());
            } else {
                for (p, s) in path.iter().rev() {
                    println!("    b.set([{}, {}, {}], \"{}\");", p[0], p[1], p[2], s);
                }
                println!("    // {} new blocks", path.len());
            }
        }
        None => anyhow::bail!("no route"),
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct PlaceArgs {
    design: PathBuf,
    #[arg(long)]
    groups: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// Stop after this many working designs (0 = all).
    #[arg(long, default_value_t = 0)]
    want: usize,
    /// Extra pseudo-random judge positions on top of the default four.
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
}

pub fn cmd_place(version: &str, args: PlaceArgs) -> Result<()> {
    let PlaceArgs { design, groups, out, workers, want, extra_offsets } = args;
    let d = load_design(&design)?;
    let text = std::fs::read_to_string(&groups)?;
    let gs = crate::place::parse_groups(&text)?;
    let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })?;
    let t = std::time::Instant::now();
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let offsets = crate::search::judge_offsets(extra_offsets);
    let opts = crate::place::PlaceOptions {
        screen: VerifyOptions { cycles: 1, offsets: offsets.iter().take(1).copied().collect(), ..Default::default() },
        judge: VerifyOptions { offsets, ..Default::default() },
        want,
    };
    let found = crate::place::place(&pool, &d, &gs, &opts, &out, &log)?;
    println!("{} working combinations", found.len());
    Ok(())
}

#[derive(clap::Args)]
pub struct Route2Args {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Largest group of control blocks ripped at once.
    #[arg(long, default_value_t = 5)]
    max_rip: usize,
    /// Rip exactly these groups: cells "x,y,z;x,y,z", groups separated by '|'.
    #[arg(long, default_value = "")]
    rip: String,
    /// New net: "x,y,z=WAVE;..." piston cells and the signal each must get (level+D,
    /// pulse+D/L, on, off, open:A-B;close:C-). Every other piston cell keeps its signal.
    #[arg(long, default_value = "")]
    need: String,
    /// Only tap these cells ("x,y,z;..."; default: any powered block or watched cell).
    #[arg(long, default_value = "")]
    taps: String,
    /// What the router may place: dust, rep, obs, block, gt, torch or full states.
    #[arg(long, default_value = "dust,rep,obs,block,gt")]
    palette: String,
    /// Accept a piston cell's signal shifted by up to this many ticks.
    #[arg(long, default_value_t = 0)]
    tolerance: i32,
    #[arg(long, default_value_t = 6)]
    beam: usize,
    #[arg(long, default_value_t = 4)]
    hits: usize,
    #[arg(long, default_value_t = 60000)]
    max_states: usize,
    /// Routing box: the design's bounds grown by this much.
    #[arg(long, default_value_t = 3)]
    margin: i32,
    /// Do not place observers watching cells the machine changes.
    #[arg(long)]
    no_watch: bool,
    /// Simulate candidates even when the static model rejects them.
    #[arg(long)]
    no_check: bool,
    /// Print the model: every piston cell's signal and a calibration against the simulator.
    #[arg(long)]
    verbose: bool,
    /// Print the model and stop.
    #[arg(long)]
    model_only: bool,
    /// Allow re-routes with this many more blocks than the rip removes (1 = same size: a
    /// sanity check that the router re-finds the existing wiring; such designs are reported,
    /// not kept).
    #[arg(long, default_value_t = 0)]
    extra_blocks: u32,
    /// Also rip pairs of separate groups (two nets re-routed together, e.g. from one shared
    /// source), up to --max-rip blocks in all.
    #[arg(long)]
    pairs: bool,
    /// Plateau: re-route at equal size too, judge those designs, and give each a local search
    /// (--moves, --polish-secs each, at most --polish-max designs).
    #[arg(long)]
    polish: bool,
    #[arg(long, default_value_t = 60)]
    polish_secs: u64,
    #[arg(long, default_value_t = 20)]
    polish_max: usize,
    #[arg(long)]
    archive: bool,
    #[command(flatten)]
    search: SearchArgs,
}

pub fn cmd_route2(version: &str, args: Route2Args) -> Result<()> {
    let Route2Args {
        design,
        out,
        max_rip,
        rip,
        need,
        taps,
        palette,
        tolerance,
        beam,
        hits,
        max_states,
        margin,
        no_watch,
        no_check,
        verbose,
        model_only,
        extra_blocks,
        pairs,
        polish,
        polish_secs,
        polish_max,
        archive,
        search,
    } = args;
    use crate::route2;
    let d = load_design(&design)?;
    let pool = search.pool(version)?;
    let t = std::time::Instant::now();
    let sopts = search.options()?;
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let groups = rip.split('|').map(str::trim).filter(|g| !g.is_empty()).map(crate::synth::parse_cells).collect::<Result<Vec<_>>>()?;
    let o = route2::RipOptions {
        max_rip,
        route: route2::RouteOptions {
            palette: route2::palette(&palette)?,
            tolerance,
            beam,
            hits,
            max_states,
            margin,
            taps: crate::synth::parse_cells(&taps)?,
            watch: !no_watch,
            partial: false,
            order_limit: 0,
            threads: 1,
            astar: false,
        },
        no_check,
        rip: groups,
        verbose: verbose || model_only,
        extra_blocks: if polish { extra_blocks.max(1) } else { extra_blocks },
        pairs,
        polish,
        polish_secs,
        polish_max,
    };
    println!("{} workers; judge {} positions", pool.len(), sopts.judge.offsets.len());
    if model_only {
        let meas = route2::measure(&mut pool.worker(0), &d)?;
        route2::model_report(&d, &meas, &log);
        return Ok(());
    }
    if !need.is_empty() {
        let meas = route2::measure(&mut pool.worker(0), &d)?;
        let mut needs = Vec::new();
        for item in need.split(';').map(str::trim).filter(|x| !x.is_empty()) {
            let (c, w) = item.split_once('=').ok_or_else(|| anyhow::anyhow!("expected x,y,z=WAVE, got {item:?}"))?;
            let p = crate::synth::parse_cells(c)?.first().copied().ok_or_else(|| anyhow::anyhow!("no cell in {item:?}"))?;
            needs.push((p, route2::Wave::parse(w)?));
        }
        let found = route2::route_new(&pool, &d, &meas, &needs, &o, &sopts, &log)?;
        match found.first() {
            Some((best, desc)) => {
                best.save(&out)?;
                println!("WORKS: {} blocks: {desc}; written to {}", crate::block_count(best), out.display());
            }
            None => println!("nothing found"),
        }
        return Ok(());
    }
    let source = design.display().to_string();
    let judge_n = sopts.judge.offsets.len();
    let mut on_improve = |best: &mctas::design::Design, desc: &str| -> Result<()> {
        best.save(&out)?;
        log(&format!("written to {}: {} blocks ({desc})", out.display(), crate::block_count(best)));
        if archive {
            let note = format!("verified WORKS: {} blocks (judge: {judge_n} positions x 3 cycles); door route2 from {source}", crate::block_count(best));
            if let Some(p) = crate::archive::save(&out, best, crate::block_count(best), &note, None)? {
                log(&format!("archived {}", p.display()));
            }
        }
        Ok(())
    };
    let best = route2::rip_and_reroute(&pool, &d, &o, &sopts, &log, &mut on_improve)?;
    println!("{} -> {} blocks", crate::block_count(&d), crate::block_count(&best));
    Ok(())
}
