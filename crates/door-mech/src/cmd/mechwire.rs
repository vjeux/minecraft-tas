//! `door mech-wire`: wiring a mechanism from its ideal-input schedule.

use anyhow::Result;
use piston_door::cli::{SearchArgs, load_design};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct MechWireArgs {
    /// Ideal schedules (`door ideal` format), or one mech-synth catalog with --catalog-ids.
    #[arg(required = true, num_args = 1..)]
    schedules: Vec<PathBuf>,
    /// Schedule variables "name=value".
    #[arg(long)]
    set: Vec<String>,
    /// Lever spots "x,y,z@ax,ay,az;..." (lever cell @ the cell it hangs on). Default: every
    /// spot within --lever-radius of the --lever-near receivers that need power first.
    #[arg(long, default_value = "")]
    levers: String,
    #[arg(long, default_value_t = 2)]
    lever_radius: i32,
    #[arg(long, default_value_t = 1)]
    lever_near: usize,
    /// Most wiring blocks per candidate (lever and its block excluded).
    #[arg(long, default_value_t = 24)]
    budget: u32,
    #[arg(long, default_value = "dust,rep,obs,block,gt")]
    palette: String,
    #[arg(long, default_value_t = 0)]
    tolerance: i32,
    #[arg(long, default_value_t = 6)]
    beam: usize,
    #[arg(long, default_value_t = 4)]
    hits: usize,
    #[arg(long, default_value_t = 60000)]
    max_states: usize,
    #[arg(long, default_value_t = 3)]
    margin: i32,
    /// Do not place observers watching cells the machine changes.
    #[arg(long)]
    no_watch: bool,
    /// Connect receivers that need pulses from several sources in several steps.
    #[arg(long)]
    partial: bool,
    /// Per partial solution, route only the first N receivers left (earliest power first;
    /// 0: all, a beam over every order).
    #[arg(long, default_value_t = 2)]
    order_limit: usize,
    /// Free receivers "x,y,z;...": accept a single 2 gt pulse at any tick of --free-range.
    #[arg(long, default_value = "")]
    free: String,
    #[arg(long, default_value = "2..40")]
    free_range: String,
    /// Routing threads (lever spots in parallel).
    #[arg(long, default_value_t = 24)]
    threads: usize,
    /// Re-time the schedule's steps (step 0 at one of --first, then gaps from --gaps).
    #[arg(long)]
    retime: bool,
    #[arg(long, default_value = "1")]
    first: String,
    #[arg(long, default_value = "2,4,6")]
    gaps: String,
    #[arg(long, default_value_t = 50)]
    max_tick: u32,
    /// Wire at most this many working retimings (fastest open + close first).
    #[arg(long, default_value_t = 12)]
    retime_max: usize,
    /// Judge the best N candidates (0: none).
    #[arg(long, default_value_t = 12)]
    verify: usize,
    /// Screen every lever spot cheaply (each receiver routed alone) and fully route only the
    /// best N (0: every spot fully).
    #[arg(long, default_value_t = 6)]
    screen_top: usize,
    /// Print what the router finds for each receiver alone (per lever spot).
    #[arg(long)]
    debug: bool,
    /// Routing box "x0,y0,z0:x1,y1,z1" (world coordinates; lever spots too) instead of the
    /// design's bounds +- --margin: e.g. a flat box (z0 = z1 = the door's z) for flat doors.
    #[arg(long = "box")]
    region_box: Option<String>,
    /// Cells no wiring may use "x,y,z;...".
    #[arg(long, default_value = "")]
    forbid: String,
    /// Also forbid the door plane's hallway ring (the 12 cells face-adjacent to the opening at
    /// the door's z): static wiring there shows in the open doorway (not seamless).
    #[arg(long)]
    forbid_ring: bool,
    /// The positional file is a mech-synth catalog: wire these entries ("355,356").
    #[arg(long, default_value = "")]
    catalog_ids: String,
    /// The catalog's base design.
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/mech-synth/bases/sidesM.txt")]
    catalog_base: PathBuf,
    /// Compress the schedule's steps: step 0 at --first, each gap shrunk to the smallest of
    /// --gaps that still works with ideal inputs.
    #[arg(long)]
    compress: bool,
    /// Single-pulse receivers whose pulse may move by at least this many ticks (ideal-input
    /// sweep) become free: any 2 gt pulse in that window (0: off).
    #[arg(long, default_value_t = 0)]
    auto_free: i32,
    /// Free receivers also accept 3 pulses per edge (a pulse-driven piston toggles per
    /// pulse: one net move, timed by the last pulse).
    #[arg(long)]
    odd: bool,
    /// With --auto-free: every receiver also accepts any combination of its events' timing
    /// windows (ideal-input sweep, others fixed).
    #[arg(long)]
    timing: bool,
    /// Pulse pairs on one cell become one level window (a repeater instead of two pulses).
    #[arg(long)]
    windows: bool,
    /// Polish each schedule's verified design for this many seconds (route2 rip-up, then
    /// the local search with every move family); 0: off.
    #[arg(long, default_value_t = 0)]
    polish_secs: u64,
    /// Re-time every step to tick first + gap * k before anything else (0: keep).
    #[arg(long, default_value_t = 0)]
    uniform_gap: u32,
    /// Write candidates and verified designs here.
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long)]
    archive: bool,
    #[command(flatten)]
    search: SearchArgs,
}

pub fn cmd_mech_wire(version: &str, args: MechWireArgs) -> Result<()> {
    let MechWireArgs {
        schedules, set, levers, lever_radius, lever_near, budget, palette, tolerance, beam, hits, max_states, margin,
        no_watch, partial, order_limit, free, free_range, threads, retime, first, gaps, max_tick, retime_max, verify,
        screen_top, debug, region_box, forbid, forbid_ring, catalog_ids, catalog_base, compress, auto_free, odd,
        timing, windows, polish_secs, uniform_gap, out_dir, archive, search,
    } = args;
    use crate::mechwire;
    use crate::route2;
    use anyhow::Context;
    let pool = search.pool(version)?;
    let sopts = search.options()?;
    let t = std::time::Instant::now();
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let nums = |s: &str| -> Result<Vec<u32>> { s.split(',').map(|x| x.trim().parse::<u32>().map_err(anyhow::Error::from)).collect() };
    let (flo, fhi) = free_range.split_once("..").context("--free-range lo..hi")?;
    let o = mechwire::WireOptions {
        route: route2::RouteOptions {
            palette: route2::palette(&palette)?,
            tolerance,
            beam,
            hits,
            max_states,
            margin,
            taps: Vec::new(),
            watch: !no_watch,
            partial,
            order_limit,
            threads: 1,
            astar: true,
        },
        budget,
        threads,
        free: crate::synth::parse_cells(&free)?.into_iter().map(|c| (c, flo.parse().unwrap_or(2), fhi.parse().unwrap_or(40))).collect(),
        alts: Vec::new(),
        odd,
        forbid: Vec::new(),
        debug,
        screen_top,
    };
    // The source schedules: the files themselves, or catalog entries of the first file.
    let schedule = schedules.first().cloned().context("no schedule file")?;
    let text = std::fs::read_to_string(&schedule).with_context(|| format!("reading {}", schedule.display()))?;
    if region_box.is_some() || !forbid.is_empty() || forbid_ring {
        mech_wire_region(&text, &region_box, &forbid, forbid_ring, margin)?;
    }
    let mut sources: Vec<(String, crate::ideal::Schedule, std::collections::BTreeMap<String, i64>)> = Vec::new();
    if catalog_ids.is_empty() {
        for path in &schedules {
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let s = crate::ideal::Schedule::parse(&text)?;
            let mut vars = s.vars().clone();
            for kv in &set {
                let (k, v) = kv.split_once('=').context("--set name=value")?;
                vars.insert(k.to_string(), v.parse()?);
            }
            let name = path.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            sources.push((name, s, vars));
        }
    } else {
        let base = load_design(&catalog_base)?;
        for id in nums(&catalog_ids)? {
            match mechwire::catalog_schedule(&text, id as usize, &base, 6) {
                Ok(s) => sources.push((format!("#{id}"), s, Default::default())),
                Err(e) => log(&format!("#{id}: {e:#}")),
            }
        }
    }
    // Each source: its schedules are built, then wired at once (results as they come).
    let mut all: Vec<mechwire::Wired> = Vec::new();
    let mut ranking: Vec<(usize, String, usize)> = Vec::new();
    let steps_of = |s: &crate::ideal::Schedule, vars: &std::collections::BTreeMap<String, i64>| -> Result<mechwire::Steps> {
        let st = mechwire::Steps::from_schedule(s, vars)?;
        if !windows {
            return Ok(st);
        }
        // Each pulse pair alone as a window, kept if the door still works with ideal
        // inputs at the schedule's own timing.
        let mut ok_cells = Vec::new();
        for c in st.pulse_pair_cells() {
            let w = st.with_windows(&[c]);
            if !mechwire::working_retimes(&pool, &w, vec![st.ticks.clone()], 0, 2).is_empty() {
                ok_cells.push(c);
            }
        }
        log(&format!("  windows: pulse pairs at {:?} become windows", ok_cells.iter().map(|c| route2::fmt_pos(*c)).collect::<Vec<_>>()));
        Ok(st.with_windows(&ok_cells))
    };
    for (name, s, vars) in &sources {
        let mut scheds: Vec<(String, crate::ideal::Schedule, std::collections::BTreeMap<String, i64>)> = Vec::new();
        if retime {
            let steps = steps_of(s, vars)?;
            let assigns = steps.assignments(&nums(&first)?, &nums(&gaps)?, max_tick);
            log(&format!("{name}: {} steps (old ticks {:?}), {} retimings to replay", steps.ticks.len(), steps.ticks, assigns.len()));
            let mut ok = mechwire::working_retimes(&pool, &steps, assigns, 0, 2);
            ok.sort_by_key(|(a, (op, cl))| (op + cl, a.clone()));
            log(&format!("{name}: {} retimings work with ideal inputs", ok.len()));
            for (a, (op, cl)) in ok.iter().take(retime_max) {
                log(&format!("  steps at {a:?}: open {op} / close {cl}"));
                scheds.push((format!("{name} steps {a:?}"), steps.retimed(a), Default::default()));
            }
        } else if compress {
            let steps = steps_of(s, vars)?;
            let f = nums(&first)?.first().copied().unwrap_or(1);
            match mechwire::compress(&pool, &steps, f, 6, &nums(&gaps)?, &log) {
                Some(a) => {
                    log(&format!("{name}: {} steps compressed to {a:?}", steps.ticks.len()));
                    scheds.push((format!("{name} steps {a:?}"), steps.retimed(&a), Default::default()));
                }
                None => log(&format!("{name}: no working retiming")),
            }
        } else if uniform_gap > 0 {
            let steps = steps_of(s, vars)?;
            let f = nums(&first)?.first().copied().unwrap_or(1);
            let a: Vec<u32> = (0..steps.ticks.len()).map(|k| f + uniform_gap * k as u32).collect();
            log(&format!("{name}: {} steps re-timed to {a:?}", steps.ticks.len()));
            scheds.push((format!("{name} steps {a:?}"), steps.retimed(&a), Default::default()));
        } else {
            scheds.push((name.clone(), s.clone(), vars.clone()));
        }
    for (label, sc, v) in &scheds {
        mech_wire_schedule(&pool, &sopts, &o, &log, (label, sc, v), &levers, lever_near, lever_radius, auto_free, timing, verify, polish_secs, &out_dir, archive, &mut ranking, &mut all)?;
    }
    }
    ranking.sort();
    println!("== ranking (verified mechanism + wiring) ==");
    for (b, label, mb) in &ranking {
        println!("{:>6} total, {mb} mechanism: {label}", if *b == usize::MAX { "-".to_string() } else { b.to_string() });
    }
    all.sort_by(|a, b| a.blocks.cmp(&b.blocks).then_with(|| a.desc.cmp(&b.desc)));
    let mut seen = std::collections::HashSet::new();
    all.retain(|w| seen.insert(crate::search::design_hash(&w.design)));
    for w in all.iter().take(20) {
        println!("candidate {} blocks: {}", w.blocks, w.desc);
    }
    if let Some(dir) = &out_dir {
        std::fs::create_dir_all(dir)?;
        for (i, w) in all.iter().take(verify.max(5)).enumerate() {
            let mut d = w.design.clone();
            d.name = format!("cand{i:02}-{}b", w.blocks);
            d.save(&dir.join(format!("cand{i:02}-{}b.txt", w.blocks)))?;
        }
    }
    let _ = &schedule;
    Ok(())
}

/// --box, --forbid, --forbid-ring: the routing region and its forbidden cells.
fn mech_wire_region(text: &str, region_box: &Option<String>, forbid: &str, forbid_ring: bool, margin: i32) -> Result<()> {
    use crate::mechwire;
    use anyhow::Context;
    let first = crate::ideal::Schedule::parse(&text)?;
    let d = first.design();
    let (blo, bhi) = d.bounds().context("empty design")?;
    let (lo, hi) = match &region_box {
        Some(b) => {
            let (a, z) = b.split_once(':').context("--box x0,y0,z0:x1,y1,z1")?;
            let p = |s: &str| -> Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>().map_err(anyhow::Error::from)).collect::<Result<_>>()?;
                anyhow::ensure!(v.len() == 3, "--box corner {s:?}: expected x,y,z");
                Ok([v[0], v[1], v[2]])
            };
            (p(a)?, p(z)?)
        }
        None => ([blo[0] - margin, blo[1] - margin, blo[2] - margin], [bhi[0] + margin, bhi[1] + margin, bhi[2] + margin]),
    };
    let mut cells = crate::synth::parse_cells(&forbid)?;
    if forbid_ring {
        let door = *d.ports.get("door").context("--forbid-ring: the design has no door port")?;
        for t in 0..=2 {
            for c in [[-1, t, 0], [3, t, 0], [t, -1, 0], [t, 3, 0]] {
                cells.push([door[0] + c[0], door[1] + c[1], door[2] + c[2]]);
            }
        }
    }
    println!("routing region {lo:?}..{hi:?}, {} forbidden cells", cells.len());
    *mechwire::REGION.write().unwrap() = Some(mechwire::Region { lo, hi, forbid: cells });
    Ok(())
}

/// One schedule of `door mech-wire`: measured with ideal inputs, wired from its lever spots, its
/// best candidates judged (then polished, saved, archived).
fn mech_wire_schedule(
    pool: &mctas::sim::Pool,
    sopts: &crate::search::SearchOptions,
    o: &crate::mechwire::WireOptions,
    log: &(dyn Fn(&str) + Sync),
    (label, sc, v): (&String, &crate::ideal::Schedule, &std::collections::BTreeMap<String, i64>),
    levers: &str,
    lever_near: usize,
    lever_radius: i32,
    auto_free: i32,
    timing: bool,
    verify: usize,
    polish_secs: u64,
    out_dir: &Option<PathBuf>,
    archive: bool,
    ranking: &mut Vec<(usize, String, usize)>,
    all: &mut Vec<crate::mechwire::Wired>,
) -> Result<()> {
    use crate::mechwire;
    use crate::route2;
    let mech = match mechwire::measure_ideal(&mut pool.worker(0), sc, v) {
        Ok(m) => m,
        Err(e) => {
            log(&format!("{label}: cannot measure: {e:#}"));
            return Ok(());
        }
    };
    if !mech.meas.idle {
        log(&format!("{label}: warning: not idle within the recorded ticks"));
    }
    let spots = if levers.is_empty() {
        mechwire::lever_spots(&mech, &mechwire::early_receivers(&mech, lever_near), lever_radius)?
    } else {
        levers.split(';').map(str::trim).filter(|x| !x.is_empty()).map(|x| mechwire::LeverSpot::parse(x, &mech)).collect::<Result<Vec<_>>>()?
    };
    log(&format!("{label}: {} mechanism blocks, {} lever spots; wanted waves:", crate::block_count(&mech.bare), spots.len()));
    for l in mech.describe() {
        log(&format!("    {l}"));
    }
    let mut ow = o.clone();
    let mut mech = mech;
    if auto_free > 0 {
        let steps = mechwire::Steps::from_schedule(sc, v)?;
        let at: Vec<u32> = steps.ticks.clone();
        let shifts: Vec<i32> = (-12..=24).step_by(2).collect();
        let win = mechwire::event_windows(&pool, &steps, &at, &shifts);
        for ((c, s), ds) in &win {
            log(&format!("    event {} at step {s} (tick {}): works shifted by {:?}", route2::fmt_pos(*c), at[*s], ds));
        }
        let span = |ds: &Vec<i32>| ds.iter().max().zip(ds.iter().min()).map(|(a, b)| a - b).unwrap_or(-1);
        let wide: Vec<(mctas::sim::Pos, usize)> = win.iter().filter(|(_, ds)| span(ds) >= auto_free).map(|(e, _)| *e).collect();
        let noops = mechwire::noop_events(&pool, &steps, &at, &wide);
        if !noops.is_empty() {
            for (c, s) in &noops {
                log(&format!("    no-op event {} at step {s}: dropped, its ticks are don't-care", route2::fmt_pos(*c)));
            }
            let reduced = steps.retimed_without_all(&at, &noops);
            match mechwire::measure_ideal(&mut pool.worker(0), &reduced, &Default::default()) {
                Ok(m) => {
                    mech = m;
                    mechwire::dont_care(&mut mech, &steps, &at, &noops);
                    log("    wanted waves without them:");
                    for l in mech.describe() {
                        log(&format!("    {l}"));
                    }
                }
                Err(e) => log(&format!("    cannot measure without the no-op events: {e:#}")),
            }
        }
        let win2: Vec<((mctas::sim::Pos, usize), Vec<i32>)> = win.into_iter().filter(|(e, _)| !noops.contains(e)).collect();
        let free = mechwire::free_receivers(&mech, &steps, &at, &win2, auto_free);
        for (c, a, b) in &free {
            log(&format!("    free: {} any 2 gt pulse starting {a}..{b}", route2::fmt_pos(*c)));
        }
        // What depends on each free receiver's action: its timing is the router's
        // choice, so no receiver may watch those motions (their measured times are
        // not the real ones).
        let mut all_dep: Vec<mctas::sim::Pos> = Vec::new();
        for (c, _, _) in &free {
            let dep = mechwire::own_motion(&mut pool.worker(0), &mech, &steps.clone_without(&noops), &at, *c).unwrap_or_default();
            log(&format!("    {} drives {} watched cells", route2::fmt_pos(*c), dep.len()));
            all_dep.extend(dep);
        }
        all_dep.sort();
        all_dep.dedup();
        ow.forbid = mech.wants.iter().map(|w| (w.0, all_dep.clone())).collect();
        if timing {
            // The other events keep their own (narrower) windows: every receiver
            // accepts each combination of its events' shifts.
            let narrow: Vec<((mctas::sim::Pos, usize), Vec<i32>)> = win2.iter().filter(|(_, ds)| span(ds) < auto_free).cloned().collect();
            let alts = mechwire::timing_alts(&mech, &steps, &at, &narrow, 400);
            for (c, a) in &alts {
                log(&format!("    timing: {} accepts {} waves", route2::fmt_pos(*c), a.len()));
            }
            ow.alts = alts;
        }
        ow.free.extend(free);
    }
    let r = mechwire::wire_all(&mech, &spots, &ow, &log);
    log(&format!("{label}: {} candidates, best {}", r.len(), r.first().map(|w| w.blocks.to_string()).unwrap_or("-".into())));
    // Judge this schedule's best candidates now (one lever spot's near-duplicates
    // would crowd a global list).
    if verify > 0 && !r.is_empty() {
        let n = verify.min(r.len());
        let ok = mechwire::judge(&pool, &r[..n], &sopts, &log);
        match ok.first() {
            Some((d0, desc)) => {
                log(&format!("{label}: VERIFIED {} blocks: {desc}", crate::block_count(d0)));
                let polished;
                let d = if polish_secs > 0 {
                    polished = mechwire::polish(&pool, d0, &o.route, &sopts, polish_secs, &log)?;
                    log(&format!("{label}: polished {} -> {} blocks", crate::block_count(d0), crate::block_count(&polished)));
                    &polished
                } else {
                    d0
                };
                ranking.push((crate::block_count(d), label.clone(), crate::block_count(&mech.bare)));
                if let Some(dir) = &out_dir {
                    std::fs::create_dir_all(dir)?;
                    let stem: String = label.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
                    let mut dd = d.clone();
                    dd.name = format!("{stem}-{}b", crate::block_count(d));
                    let path = dir.join(format!("{}.txt", dd.name));
                    dd.save(&path)?;
                    if archive {
                        let note = format!("verified WORKS: {} blocks; door mech-wire from {label}: {desc}", crate::block_count(d));
                        if let Some(p) = crate::archive::save(&path, &dd, crate::block_count(d), &note, None)? {
                            log(&format!("archived {}", p.display()));
                        }
                    }
                }
            }
            None => {
                log(&format!("{label}: none of the best {n} candidates passes the judge"));
                ranking.push((usize::MAX, label.clone(), crate::block_count(&mech.bare)));
            }
        }
    }
    all.extend(r.into_iter().map(|w| mechwire::Wired { desc: format!("{label}: {}", w.desc), ..w }));
    Ok(())
}
