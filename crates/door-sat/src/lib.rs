//! `mctas door` commands: the SAT tools: satwire, satsweep, boxproof.
//! One of the door CLI crates (crates/door-*), flattened into `mctas door` by
//! crates/mctas-cli/src/door.rs: a change here rebuilds only this crate and the binary.
//! Compiled unoptimized (Cargo.toml: argument parsing and dispatch only): put any heavy
//! loop in a piston-door function, which is optimized.

use anyhow::Result;
use clap::Subcommand;
use mctas::sim::Worker;
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// SAT synthesis of a flat door's WIRING for a fixed mechanism (satwire.rs): the mechanism
    /// and each piston's power per tick come from an ideal schedule (`.ideal`) or a wired design
    /// (`--from-wired`, wiring stripped); placement one-hot per free cell of the box, two traces
    /// (lever on from rest, off from open), time-unrolled redstone semantics; every model is
    /// judged by the real simulator (CEGAR) and the wiring is minimized.
    /// Whole-box bounded model checking of flat doors (track boxproof): `fuzz` (the model vs
    /// redsim on random boxes), `case`, `bmc` (find a door / prove none fits a box).
    Boxproof {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    Satwire {
        /// An ideal schedule (`.ideal`) or a design (with --from-wired).
        #[arg(required = true, num_args = 1..)]
        mechanism: Vec<PathBuf>,
        /// Parallel jobs (--check over many designs).
        #[arg(long, default_value_t = 8)]
        jobs: usize,
        /// The mechanism is a wired design: record its pistons' real power, strip its wiring.
        #[arg(long)]
        from_wired: bool,
        /// Wiring box "x0,y0,z0:x1,y1,z1" (default: the mechanism's bounding box).
        #[arg(long = "box")]
        bx: Option<String>,
        /// Ring cells (and the cells behind hallway holes) hold only plain blocks.
        #[arg(long)]
        forbid_ring: bool,
        /// Ring cells nothing passes through hold smooth stone (props-dyn's hallway blocks), so
        /// the wiring is hallway-compatible by construction.
        #[arg(long)]
        hall_ring: bool,
        /// Lever candidate cells "x,y,z;..." (default: free cells and the cells around the box).
        #[arg(long)]
        lever: Option<String>,
        /// Also offer levers in front of / behind wiring blocks (out of the plane).
        #[arg(long)]
        lever_oop: bool,
        /// Palette: block,dust,rep (rep1..rep4),obs,torch,note,bulb,cmp.
        #[arg(long, default_value = "block,dust,rep,obs,torch")]
        palette: String,
        #[arg(long)]
        max_blocks: Option<usize>,
        /// Most recorded static ticks replayed (stalls) per trace: the solver retimes the schedule.
        #[arg(long, default_value_t = 8)]
        dmax: i32,
        /// Most recorded static ticks skipped per trace (pauses of the schedule shortened).
        #[arg(long, default_value_t = 8)]
        skip: i32,
        /// Ticks after the mechanism settles that the wiring must also settle in.
        #[arg(long, default_value_t = 6)]
        settle: i32,
        /// Ticks recorded per flip.
        #[arg(long, default_value_t = 80)]
        rec_ticks: u32,
        /// Forbid quasi-connected power as a piston's wanted power.
        #[arg(long)]
        no_qc: bool,
        /// Dust levels modeled (15 = exact; fewer = conservative, shorter dust runs).
        #[arg(long, default_value_t = 15)]
        levels: u8,
        /// Fix the source design's own wiring and report where the model disagrees with it.
        #[arg(long)]
        check: bool,
        /// Re-solves spent shrinking an UNSAT core to the conflicting pistons (0: the raw core).
        #[arg(long, default_value_t = 40)]
        core_tries: usize,
        /// CEGAR iterations.
        #[arg(long, default_value_t = 300)]
        iters: usize,
        /// Diagnose UNSAT: guard constraint groups (supports, each cell's semantics, scheduling,
        /// trace ends) with selectors and print the conflicting groups.
        #[arg(long)]
        diag: bool,
        /// A partly wired ideal plan: keep the wiring already in its design (and its lever).
        #[arg(long)]
        keep_wiring: bool,
        /// Exact recorded power everywhere (default: a retracted piston that stays retracted while
        /// powered, blocked or QC-armed, may get any power at that tick).
        #[arg(long)]
        exact: bool,
        /// Require the RECORDED piston power per tick (old behavior) instead of the power implied
        /// by the mechanism's motion (on to extend or stay extended, off to retract or to stay
        /// retracted when it could extend, free when blocked).
        #[arg(long)]
        recorded_power: bool,
        /// Over-approximate the game where cheap: QC-held pistons without an update, repeater and
        /// torch read orders inside a tick (UNSAT is then stronger evidence; CEGAR rejects the rest).
        #[arg(long)]
        sound: bool,
        /// The mechanism's stone blocks are oak logs (same mechanics): leaves (palette `leaves`) next
        /// to their paths re-read their distance 1 gt after a log moves (a 1 gt delay).
        #[arg(long)]
        log_doors: bool,
        /// One global time warp for the whole mechanism (default: one per independent-timing
        /// cluster of cells, so events of different clusters may be split or shifted apart).
        #[arg(long)]
        one_warp: bool,
        /// FULL FRAME at open: the 12 frame cells around the opening hold plain blocks when open
        /// (a mechanism door block, or static stone forced into a free frame cell).
        #[arg(long)]
        full_frame: bool,
        /// The lever is ON at rest (the door opens when it is switched off); detected from a wired
        /// source design.
        #[arg(long)]
        lever_on: bool,
        /// Ideal plans: off-plane ideal inputs that stay GIVEN sources (their recorded on/off,
        /// timed with the piston they drive) instead of being wired: cells "x,y;x,y" (the input's
        /// in-plane cell, e.g. the piston it sits behind).
        #[arg(long)]
        keep_ideal: Option<String>,
        /// Exact timing (--dmax 0 --skip 0): a candidate the judge rejects blocks only its choices
        /// within 3 cells of the first piston that misbehaves in its real run.
        #[arg(long)]
        local_nogood: bool,
        /// Keep this design's wiring cells and lever as they are; solve only the other cells.
        #[arg(long)]
        fixed: Option<PathBuf>,
        /// The mechanism is a families catalog entry: MECHANISM = CATALOG.tsv, this = the entry id.
        #[arg(long)]
        families: Option<String>,
        /// With --families: ticks per macro step of the ideal schedule.
        #[arg(long, default_value_t = 6)]
        gap: u32,
        /// With --families: use the open/close schedules even when a symmetric train exists.
        #[arg(long)]
        oc: bool,
        /// Stop at the first judged design (no minimization).
        #[arg(long)]
        first: bool,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A systematic `door satwire` sweep (satsweep.rs, track sweep56): every ideal plan x every
    /// placement of a W x H wiring box (`--box-dims`) x every timing variant (`--variants
    /// base,gap,triple`): each variant is replayed with ideal inputs first (plans the mechanism
    /// cannot follow are INVALID), then SAT-wired; one TSV row per run (SAT / UNSAT + core / ...).
    Satsweep {
        /// Ideal schedules (`.ideal`).
        #[arg(required = true, num_args = 1..)]
        plans: Vec<PathBuf>,
        #[arg(long, default_value_t = 8)]
        jobs: usize,
        #[arg(long, default_value = "block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,obsidian")]
        palette: String,
        /// Ring cells (and cells behind open-state holes) hold only plain blocks.
        #[arg(long)]
        forbid_ring: bool,
        /// Ring cells nothing passes through hold smooth stone (props-dyn's hallway blocks).
        #[arg(long)]
        hall_ring: bool,
        #[arg(long)]
        lever_oop: bool,
        #[arg(long, default_value_t = 8)]
        dmax: i32,
        #[arg(long, default_value_t = 8)]
        skip: i32,
        #[arg(long, default_value_t = 6)]
        settle: i32,
        #[arg(long, default_value_t = 6)]
        core_tries: usize,
        /// Wiring box size "WxH" (every placement around the mechanism); default: its bounding box.
        #[arg(long)]
        box_dims: Option<String>,
        /// base,gap,triple
        #[arg(long, default_value = "base")]
        variants: String,
        /// Gap values (ticks) for the `gap` variants.
        #[arg(long, default_value = "2,3,4,5")]
        gaps: String,
        /// Pulse spacings (ticks) for the `triple` variants.
        #[arg(long, default_value = "4,6")]
        triple_gaps: String,
        /// Stretch pulses shorter than N ticks to N (pistmodel's 1 gt pulses -> 2 gt).
        #[arg(long, default_value_t = 0)]
        widen: u32,
        /// Round every pulse width up to an even number of ticks (3 gt -> 4 gt).
        #[arg(long)]
        even: bool,
        /// Per-toggle timing MENU: every input pulsed once in a phase may also be shifted by each
        /// of these ticks (e.g. `--menu-shifts=-4,-2,2,4,6,8`); the solver picks per toggle.
        #[arg(long, default_value = "", allow_hyphen_values = true)]
        menu_shifts: String,
        /// The menu also offers each toggle tripled (`--triple-gaps` spacings).
        #[arg(long)]
        menu_triples: bool,
        /// The menu also offers each toggle's pulse at these widths (e.g. `1,2,3,4`).
        #[arg(long, default_value = "")]
        menu_widths: String,
        /// The menu also offers CORE alternatives (one core pulse shifted -3..+6 or re-widthed 1/3/4,
        /// or two pulses of one input held as one level), the solver picking one.
        #[arg(long)]
        menu_core: bool,
        /// Skip the ideal-input replay of each variant.
        #[arg(long)]
        no_validate: bool,
        /// Minimize the wiring of every SAT model (default: stop at the first judged design).
        #[arg(long)]
        minimize: bool,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/sweep56/SWEEP.tsv")]
        tsv: PathBuf,
        #[arg(long, default_value = "designs/piston-door-3x3/tracks/sweep56/runs")]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 28)]
        extra_offsets: u32,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    match cmd {
        DoorCmd::Boxproof { args } => {
            return crate::boxproof::cli(&args);
        }
        DoorCmd::Satwire { mechanism, jobs, fixed, diag, sound, log_doors, one_warp, full_frame, lever_on, keep_ideal, local_nogood, keep_wiring, exact, recorded_power, from_wired, bx, forbid_ring, hall_ring, lever, lever_oop, palette, max_blocks, dmax, skip, settle, rec_ticks, no_qc, levels, check, core_tries, iters, families, gap, oc, first, extra_offsets, out } => {
            use crate::satwire as sw;
            sw::DIAG.store(diag, std::sync::atomic::Ordering::Relaxed);
            sw::SOUND.store(sound, std::sync::atomic::Ordering::Relaxed);
            sw::LOG_DOORS.store(log_doors, std::sync::atomic::Ordering::Relaxed);
            sw::LEVER_ON.store(lever_on, std::sync::atomic::Ordering::Relaxed);
            sw::LOCAL_NOGOOD.store(local_nogood, std::sync::atomic::Ordering::Relaxed);
            if let Some(ki) = &keep_ideal {
                let mut cells = Vec::new();
                for t in ki.split(';').map(str::trim).filter(|t| !t.is_empty()) {
                    let v: Vec<i32> = t.split(',').map(|x| x.trim().parse()).collect::<Result<_, _>>()?;
                    anyhow::ensure!(v.len() >= 2, "--keep-ideal x,y;x,y");
                    cells.push((v[0], v[1]));
                }
                *sw::KEEP_IDEAL.lock().unwrap() = cells;
            }
            if check && mechanism.len() > 1 {
                let po = sw::ProbOpts { bx: None, palette: sw::palette("block,dust,rep,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,obsidian")?, forbid_ring: false, hall_ring: false, levers: None, lever_oop: true, dmax: 0, skip: 0, settle, qc: !no_qc, levels, blocked_dc: false, motion: !recorded_power, clusters: false, full_frame: false };
                let next = std::sync::atomic::AtomicUsize::new(0);
                let out = std::sync::Mutex::new(Vec::new());
                std::thread::scope(|sc| {
                    for _ in 0..jobs.min(mechanism.len()) {
                        sc.spawn(|| {
                            let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() };
                            let Ok(mut w) = Worker::spawn(&cfg) else { return };
                            loop {
                                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                                let Some(p) = mechanism.get(i) else { break };
                                let line = sw::check_line(&mut w, p, &po);
                                println!("{}: {line}", p.display());
                                out.lock().unwrap().push((p.clone(), line));
                            }
                        });
                    }
                });
                let res = out.into_inner().unwrap();
                let mut groups: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
                for (_, l) in &res {
                    let key = if l.starts_with("MISMATCH") { "MISMATCH".to_string() } else { l.split(" (").next().unwrap_or(l).to_string() };
                    *groups.entry(key).or_default() += 1;
                }
                println!("== {} designs", res.len());
                for (k, n) in groups {
                    println!("{n:5}  {k}");
                }
                return Ok(());
            }
            // batch synthesis: every plan in its own thread (own JVM), logs and doors in --out-dir
            if mechanism.len() > 1 {
                let dir = out.clone().ok_or_else(|| anyhow::anyhow!("several plans: --out <dir> (one .log per plan, a .txt per judged door)"))?;
                std::fs::create_dir_all(&dir)?;
                let next = std::sync::atomic::AtomicUsize::new(0);
                let po = sw::ProbOpts {
                    bx: bx.as_deref().map(sw::parse_box).transpose()?,
                    palette: sw::palette(&palette)?,
                    forbid_ring,
                    hall_ring,
                    levers: lever.as_deref().map(crate::synth::parse_cells).transpose()?,
                    lever_oop,
                    dmax,
                    skip,
                    settle,
                    qc: !no_qc,
                    levels,
                    blocked_dc: !exact,
                    motion: !recorded_power,
                    clusters: !one_warp,
                    full_frame,
                };
                std::thread::scope(|sc| {
                    for _ in 0..jobs.min(mechanism.len()) {
                        sc.spawn(|| loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(m) = mechanism.get(i) else { break };
                            let stem = m.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                            let logf = std::sync::Mutex::new(std::fs::File::create(dir.join(format!("{stem}.log"))).ok());
                            let log = |s: String| {
                                use std::io::Write;
                                if let Some(f) = logf.lock().unwrap().as_mut() {
                                    let _ = writeln!(f, "{s}");
                                }
                            };
                            let res = (|| -> Result<String> {
                                let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() };
                                let rec = {
                                    let mut w = Worker::spawn(&cfg)?;
                                    if from_wired {
                                        sw::record_wired(&mut w, &load_design(m)?, rec_ticks)?
                                    } else {
                                        let s = crate::ideal::Schedule::parse(&std::fs::read_to_string(m)?)?;
                                        sw::record_ideal(&mut w, &s, rec_ticks)?
                                    }
                                };
                                let mut p = sw::build_prob(rec, &po)?;
                                if keep_ideal.is_some() && !from_wired {
                                    p.plan = Some(crate::ideal::Schedule::parse(&std::fs::read_to_string(m)?)?);
                                }
                                if let Some(fp) = &fixed {
                                    sw::fix_partial(&mut p, &load_design(fp)?)?;
                                }
                                if keep_wiring && fixed.is_none() && !from_wired && !p.rec.wiring.is_empty() {
                                    let mut d = mctas::design::Design::default();
                                    for (cc, st) in &p.rec.wiring {
                                        d.blocks.insert([cc.0, cc.1, p.z], st.clone());
                                    }
                                    if let Some((lp, ls)) = &p.rec.lever {
                                        d.blocks.insert(*lp, ls.clone());
                                    }
                                    sw::fix_partial(&mut p, &d)?;
                                }
                                let o = sw::RunOpts { iters, core_tries, minimize: !first, max_blocks, extra: extra_offsets, out: Some(dir.join(format!("{stem}.txt"))), name: format!("satwire-{stem}"), version: version.to_string() };
                                Ok(match sw::run_log(&p, &o, &log)? {
                                    Some(d) => format!("DOOR {} blocks", d.blocks.len()),
                                    None => "no door".to_string(),
                                })
                            })();
                            let line = match res {
                                Ok(s) => s,
                                Err(e) => format!("ERROR {}", e.to_string().lines().next().unwrap_or("")),
                            };
                            log(format!("RESULT {line}"));
                            println!("{stem}: {line}");
                        });
                    }
                });
                return Ok(());
            }
            let mechanism = mechanism[0].clone();
            let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() };
            let mut fam_box = None;
            let rec = {
                let mut w = Worker::spawn(&cfg)?;
                if let Some(id) = &families {
                    let (s, b) = sw::families_schedule(&mechanism, id, !oc, gap)?;
                    fam_box = Some(b);
                    sw::record_ideal(&mut w, &s, rec_ticks)?
                } else if from_wired || check {
                    let d = load_design(&mechanism)?;
                    sw::record_wired(&mut w, &d, rec_ticks)?
                } else {
                    let text = std::fs::read_to_string(&mechanism)?;
                    let s = crate::ideal::Schedule::parse(&text)?;
                    sw::record_ideal(&mut w, &s, rec_ticks)?
                }
            };
            for (i, ph) in rec.phases.iter().enumerate() {
                let on: Vec<String> = (1..ph.power.len())
                    .flat_map(|j| ph.power[j].iter().filter(|(_, v)| **v).map(move |(c, _)| format!("t{j}:{},{}", c.0, c.1)))
                    .collect();
                println!("{}: {} frames, powered piston-ticks {}", ["open", "close"][i], ph.frames.len(), on.join(" "));
            }
            let po = sw::ProbOpts {
                bx: bx.as_deref().map(sw::parse_box).transpose()?.or(fam_box),
                palette: sw::palette(&palette)?,
                forbid_ring,
                hall_ring: hall_ring && !check,
                levers: lever.as_deref().map(crate::synth::parse_cells).transpose()?,
                lever_oop,
                dmax: if check { 0 } else { dmax },
                skip: if check { 0 } else { skip },
                settle,
                qc: !no_qc,
                levels,
                blocked_dc: !exact && !check,
                motion: !recorded_power,
                clusters: !one_warp && !check,
                full_frame,
            };
            let mut p = sw::build_prob(rec, &po)?;
            if keep_ideal.is_some() && !from_wired && families.is_none() {
                p.plan = Some(crate::ideal::Schedule::parse(&std::fs::read_to_string(&mechanism)?)?);
            }
            if let Some(fp) = &fixed {
                sw::fix_partial(&mut p, &load_design(fp)?)?;
            }
            if keep_wiring && fixed.is_none() && !from_wired && !check && !p.rec.wiring.is_empty() {
                let mut d = mctas::design::Design::default();
                for (cc, st) in &p.rec.wiring {
                    d.blocks.insert([cc.0, cc.1, p.z], st.clone());
                }
                if let Some((lp, ls)) = &p.rec.lever {
                    d.blocks.insert(*lp, ls.clone());
                }
                sw::fix_partial(&mut p, &d)?;
            }
            if check {
                sw::fix_wiring(&mut p)?;
                return sw::check(&p);
            }
            let name = match &families { Some(id) => format!("satwire-{id}"), None => format!("satwire-{}", mechanism.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()) };
            let o = sw::RunOpts { iters, core_tries, minimize: !first, max_blocks, extra: extra_offsets, out, name, version: version.to_string() };
            sw::run(&p, &o)?;
        }
        DoorCmd::Satsweep { plans, jobs, palette, forbid_ring, hall_ring, lever_oop, dmax, skip, settle, core_tries, box_dims, variants, gaps, triple_gaps, widen, even, menu_shifts, menu_triples, menu_widths, menu_core, no_validate, minimize, tsv, out_dir, extra_offsets } => {
            let nums = |s: &str| -> Result<Vec<u32>> { s.split(',').filter(|t| !t.trim().is_empty()).map(|t| t.trim().parse::<u32>().map_err(anyhow::Error::from)).collect() };
            let box_dims = match box_dims.as_deref() {
                Some(b) => {
                    let (w, h) = b.split_once('x').ok_or_else(|| anyhow::anyhow!("--box-dims WxH"))?;
                    Some((w.parse::<i32>()?, h.parse::<i32>()?))
                }
                None => None,
            };
            let o = crate::satsweep::SweepOpts {
                jobs,
                palette,
                forbid_ring,
                hall_ring,
                lever_oop,
                dmax,
                skip,
                settle,
                core_tries,
                box_dims,
                variants: variants.split(',').map(|s| s.trim().to_string()).collect(),
                gaps: nums(&gaps)?,
                triple_gaps: nums(&triple_gaps)?,
                widen,
                even,
                menu_shifts: menu_shifts.split(',').filter(|t| !t.trim().is_empty()).map(|t| t.trim().parse::<i64>().map_err(anyhow::Error::from)).collect::<Result<Vec<i64>>>()?,
                menu_triples,
                menu_widths: nums(&menu_widths)?,
                menu_core,
                no_validate,
                minimize,
                tsv,
                out_dir,
                version: version.to_string(),
                extra_offsets,
            };
            crate::satsweep::cli(&plans, &o)?;
        }
    }
    Ok(())
}
