//! `door mechspace`, `door families`, `door topology`, `door connect`: the families catalog of
//! flat mechanisms and its analyses.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct MechmutArgs {
    /// Seed mechanisms: a file of lines `W H x0 y0 piece piece ...` (box in door coordinates), a
    /// `door macrosat` dump (its rest layout as the mechanism, its actions as the first schedule
    /// tried), or a folder (every macrosat dump under it).
    input: PathBuf,
    /// State cap of the seeds' exact solve (the smaller-door mechanisms need 1-5M).
    #[arg(long, default_value_t = 5_000_000)]
    cap: usize,
    /// State cap of a neighbour's exact solve when its parent's schedule does not replay
    /// (0 = replay only: fast).
    #[arg(long, default_value_t = 0)]
    bfs_cap: usize,
    /// Parallel solves (each holds up to ~0.7 GB per million states).
    #[arg(long, default_value_t = 12)]
    threads: usize,
    #[arg(long, default_value_t = 4)]
    rounds: usize,
    /// Best new variants that seed the next round.
    #[arg(long, default_value_t = 6)]
    beam: usize,
    /// Ranked TSV of every kept variant (stdout when absent).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Write the best `--top` variants as ideal-input designs + plans here.
    #[arg(long)]
    ideal_out: Option<PathBuf>,
    #[arg(long, default_value_t = 5)]
    top: usize,
    /// Search from seeds that do not solve too (their neighbours may: a turned mechanism whose
    /// stacked pistons now couple by quasi-connectivity).
    #[arg(long)]
    repair: bool,
    /// Pieces never changed, space- or ';'-separated (e.g. a lever-held top trio
    /// "SD+(0,4) SD+(1,4) SD+(2,4)").
    #[arg(long)]
    keep: Option<String>,
    /// Cells no piece may cover and the cycle may never occupy, "x,y;x,y" (room for an observer).
    #[arg(long)]
    reserve: Option<String>,
    /// Keep a variant only when its schedule, as a `door ideal` plan, WORKS in redsim (4
    /// positions, between-ticks inputs): the macro model accepts moves the game refuses.
    #[arg(long)]
    validate: bool,
    /// Reject variants with a piston firing from inside the 3x3 (carried in: only door blocks or
    /// quasi-connectivity could feed it).
    #[arg(long)]
    outside_only: bool,
    /// Pistons the lever drives (space- or ';'-separated, e.g. "SW+(4,0) SW+(4,1)"): their levels
    /// flip first on both edges and they get no other event (a lever-held stack); also kept fixed.
    #[arg(long)]
    lever_held: Option<String>,
    /// Rank by unfed pistons (no free feed cell) right after the redsim verdict, before inside
    /// firings and door connect.
    #[arg(long)]
    rank_unfed: bool,
    /// Wirability bar on every solve: at most this many actions per piston per edge (0 = off).
    #[arg(long, default_value_t = 0)]
    max_actions: usize,
    /// Wirability bar on every solve: no piston fires while its base is inside the 3x3.
    #[arg(long)]
    no_inside: bool,
    /// Neighbours also ADD one piece (a piston of any kind in any free cell outside the 3x3, a
    /// stone in a free ring cell): completion search from partial designs (with --repair and
    /// --keep for the fixed part).
    #[arg(long)]
    add: bool,
    /// With --add: a redstone block may be added too (moved by pistons, it powers the pistons it
    /// touches).
    #[arg(long)]
    add_redstone: bool,
    /// With --max-actions: at most this many actions per edge beyond one per piston.
    #[arg(long)]
    extra_actions: Option<usize>,
    /// Incomparable action vectors kept per state in the constrained search (see families).
    #[arg(long, default_value_t = 64)]
    pareto: usize,
    /// Reject variants with more unfed receivers than this (after the lever allowance).
    #[arg(long)]
    max_unfed: Option<usize>,
    /// State cap of the `door connect` check per variant.
    #[arg(long, default_value_t = 100_000)]
    connect_cap: usize,
    /// macrosat seeds: keep the model's wiring cells free (no piece, never swept).
    #[arg(long)]
    reserve_wiring: bool,
    /// macrosat seeds: keep families' quasi-connectivity coupling (default off for them: their
    /// schedules list co-fired pistons as actions of their own; redsim judges either way).
    #[arg(long)]
    macrosat_qc: bool,
}

pub fn cmd_mechmut(args: MechmutArgs) -> Result<()> {
    let MechmutArgs { input, cap, bfs_cap, threads, rounds, beam, out, ideal_out, top, repair, keep, reserve, validate, outside_only, lever_held, rank_unfed, max_actions, no_inside, add, add_redstone, extra_actions, pareto, max_unfed, connect_cap, reserve_wiring, macrosat_qc } = args;
    crate::mechmut::CONNECT_CAP.store(connect_cap, std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_PARETO.store(pareto, std::sync::atomic::Ordering::Relaxed);
    crate::mechmut::ADD.store(add, std::sync::atomic::Ordering::Relaxed);
    crate::mechmut::REDSTONE.store(add_redstone, std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_EXTRA.store(extra_actions.unwrap_or(usize::MAX), std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_K.store(max_actions, std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_NO_INSIDE.store(no_inside, std::sync::atomic::Ordering::Relaxed);
    crate::mechmut::MAX_UNFED.store(max_unfed.unwrap_or(usize::MAX), std::sync::atomic::Ordering::Relaxed);
    crate::mechmut::VALIDATE.store(validate, std::sync::atomic::Ordering::Relaxed);
    crate::mechmut::OUTSIDE_ONLY.store(outside_only, std::sync::atomic::Ordering::Relaxed);
    crate::mechmut::RANK_UNFED.store(rank_unfed, std::sync::atomic::Ordering::Relaxed);
    let held: Vec<crate::families::Pc> = match lever_held {
        Some(s) => s.split(|c: char| c == ';' || c.is_whitespace()).map(str::trim).filter(|t| !t.is_empty()).map(crate::mechspace::parse_piece).collect::<Result<_>>()?,
        None => Vec::new(),
    };
    let _ = crate::mechmut::LEVER_HELD.set(held.clone());
    let mut keep: Vec<crate::families::Pc> = match keep {
        Some(s) => s.split(|c: char| c == ';' || c.is_whitespace()).map(str::trim).filter(|t| !t.is_empty()).map(crate::mechspace::parse_piece).collect::<Result<_>>()?,
        None => Vec::new(),
    };
    keep.extend(held.iter().copied());
    let reserve: Vec<(i8, i8)> = match reserve {
        Some(s) => s
            .split(';')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(|t| -> Result<(i8, i8)> {
                let (a, b) = t.split_once(',').ok_or_else(|| anyhow::anyhow!("reserve cell {t:?}: x,y"))?;
                Ok((a.trim().parse()?, b.trim().parse()?))
            })
            .collect::<Result<_>>()?,
        None => Vec::new(),
    };
    crate::mechmut::run(&input, &crate::mechmut::Opts { cap, bfs_cap, threads, rounds, beam, top, repair, keep, reserve, reserve_wiring, macrosat_qc }, out.as_deref(), ideal_out.as_deref())
}

#[derive(clap::Args)]
pub struct MechspaceArgs {
    /// selftest | validate | decide | gen
    cmd: String,
    #[arg(long, default_value = "7x8")]
    boxes: String,
    /// Candidate list (families' unresolved format: x0 y0 pieces...).
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long, default_value_t = 11)]
    pistons: usize,
    /// State cap of each reduced search.
    #[arg(long, default_value_t = 3_000_000)]
    cap: usize,
    /// families' cap (validate).
    #[arg(long, default_value_t = 300_000)]
    fcap: usize,
    #[arg(long, default_value_t = 40)]
    threads: usize,
    /// Local reach cap of an independent event group.
    #[arg(long, default_value_t = 32)]
    cap_local: usize,
    /// Groups with more events go straight to the core.
    #[arg(long, default_value_t = 6)]
    max_group: usize,
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/mechspace")]
    out_dir: PathBuf,
    /// Output tag (file names).
    #[arg(long)]
    tag: Option<String>,
    /// At most this many candidates (0 = all).
    #[arg(long, default_value_t = 0)]
    limit: usize,
    #[arg(long, default_value_t = 1)]
    two: usize,
    #[arg(long, default_value_t = 0)]
    carry: usize,
    #[arg(long)]
    vmid: Option<String>,
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/families/CATALOG.tsv")]
    catalog: PathBuf,
    /// selftest: candidates and walk length.
    #[arg(long, default_value_t = 300)]
    n: usize,
    #[arg(long, default_value_t = 200)]
    walk: usize,
    /// After a capped exact search, families' A* with this cap may still find a solution.
    #[arg(long, default_value_t = 0)]
    astar: usize,
    /// Lean second stage (visited sets only, reduced then plain) up to this many states.
    #[arg(long, default_value_t = 0)]
    cap_lean: usize,
    /// The wirable sub-model: no piston body may enter the 3x3 (heads may).
    #[arg(long)]
    no_door_bodies: bool,
    /// FULL FRAME 12/12 when open (static stones in the frame cells the cycle leaves air).
    #[arg(long)]
    full_frame: bool,
    /// Generator: non-core 1-move exits use sticky pullers RETRACTED at rest only (the old
    /// generator; by default they may also use pullers extended at rest, as c79 and the
    /// community 7x8 do).
    #[arg(long)]
    no_ext_pullers: bool,
    /// Accepted for old command lines: extended-at-rest pullers are the default now.
    #[arg(long, hide = true)]
    ext_pullers: bool,
    /// Generator: most 1-move exits by a puller EXTENDED at rest per mechanism (a lever-held top
    /// row has 3, c79 2, the community 7x8 1; each one doubles a block's choices).
    #[arg(long, default_value_t = 3)]
    ext_max: usize,
    /// Generator: extended-puller exits may sit anywhere (default: each next to an earlier one).
    #[arg(long)]
    ext_apart: bool,
    /// Generator: exactly one piston in the box's bottom row, sticky facing up.
    #[arg(long)]
    bottom_su: bool,
    /// Open = all 9 door blocks in frame cells (3 static stones in the others).
    #[arg(long)]
    frame9: bool,
    /// Only this door offset "dx,dy" (columns left of / rows below the door).
    #[arg(long)]
    door_offset: Option<String>,
    /// Generator: audit bound (a), extended-at-rest pistons reused lifted / carried.
    #[arg(long)]
    lift_ext: bool,
    /// Generator: audit bound (b), with --carry: new provider pistons carried in from 1-2
    /// cells beside their action cell.
    #[arg(long)]
    carry_in: bool,
    /// export: ticks between macro steps of the ideal plans.
    #[arg(long, default_value_t = 6)]
    gap: u32,
    /// export: simulator workers.
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// export: extra positions of the ideal replay (4 positions + this many).
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
}

pub fn cmd_mechspace(version: &str, args: MechspaceArgs) -> Result<()> {
    mechspace_switches(&args)?;
    let MechspaceArgs { cmd, boxes, input, pistons, cap, fcap, threads, cap_local, max_group, out_dir, tag, limit, two, carry, vmid, catalog, n, walk, astar, gap, workers, extra_offsets, cap_lean, bottom_su, .. } = args;
    use crate::mechspace as ms;
    let vm: Vec<Vec<(i8, i8)>> = match vmid.as_deref() {
        None => Vec::new(),
        Some("ml") => vec![vec![(0, 1)]],
        Some("mr") => vec![vec![(2, 1)]],
        Some("any") => vec![vec![(0, 1)], vec![(2, 1)]],
        Some("both") => vec![vec![(0, 1), (2, 1)]],
        Some(x) => anyhow::bail!("--vmid ml|mr|any|both, got {x}"),
    };
    let bxs = crate::families::parse_boxes(&boxes)?;
    let mk = |t: String| ms::RunOpts { cap, fcap, threads, cap_local, max_group, out: out_dir.clone(), tag: t, astar, cap_lean };
    match cmd.as_str() {
        "selftest" => {
            for &(w, h) in &bxs {
                ms::selftest(w, h, pistons, n, walk, threads)?;
            }
        }
        "validate" | "gen" => {
            let cat = ms::catalog_set(&catalog)?;
            for &(w, h) in &bxs {
                let mut items = ms::candidates(w, h, pistons, two, carry, &vm);
                eprintln!("[mechspace] {w}x{h}: {} candidates", items.len());
                if limit > 0 && items.len() > limit {
                    items.truncate(limit);
                }
                let t = tag.clone().unwrap_or_else(|| format!("{cmd}-{w}x{h}"));
                let o = mk(t);
                let found = ms::run_list(&items, &o, cmd == "validate")?;
                ms::handle_found(&found, &o, &cat)?;
            }
        }
        "decide" | "validate-list" => {
            let cat = ms::catalog_set(&catalog)?;
            let f = input.ok_or_else(|| anyhow::anyhow!("decide needs --input"))?;
            anyhow::ensure!(bxs.len() == 1, "decide needs one --boxes WxH");
            let mut items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
            if limit > 0 && items.len() > limit {
                items.truncate(limit);
            }
            let t = tag.clone().unwrap_or_else(|| format!("{cmd}-{}x{}", bxs[0].0, bxs[0].1));
            let o = mk(t);
            let found = ms::run_list(&items, &o, cmd == "validate-list")?;
            ms::handle_found(&found, &o, &cat)?;
        }
        "diag" | "diag2" | "diag3" => {
            let f = input.ok_or_else(|| anyhow::anyhow!("diag needs --input"))?;
            let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
            let (bx, v) = &items[limit.min(items.len() - 1)];
            if cmd == "diag" {
                ms::diag(*bx, v, cap, cap_local, max_group)?;
            } else if cmd == "diag3" {
                let (m, rest) = crate::families::Mech::new(*bx, v).ok_or_else(|| anyhow::anyhow!("invalid"))?;
                let cx = ms::Cx::new(&m, &rest);
                let t0 = std::time::Instant::now();
                let (g, c, nst) = ms::search_lean(&cx, std::slice::from_ref(&rest), &ms::Goal::Open, &Default::default(), cap, true, false);
                println!("plain lean open search: {nst} states, complete {c}, {} open, {:.1}s ({:.0} states/s)", g.len(), t0.elapsed().as_secs_f64(), nst as f64 / t0.elapsed().as_secs_f64());
            } else {
                ms::diag2(*bx, v, cap)?;
            }
        }
        "refit" => {
            let cands: Vec<(i8, i8, Vec<(crate::families::Bx, Vec<crate::families::Pc>)>)> = bxs.iter().map(|&(w, h)| (w, h, ms::candidates(w, h, pistons, two, carry, &vm))).collect();
            ms::refit(&catalog, &bxs, &cands, cap, threads)?;
        }
        "pretest" => {
            for &(w, h) in &bxs {
                ms::pretest(w, h, pistons, n, walk, threads)?;
            }
        }
        "coreach" => {
            let f = input.ok_or_else(|| anyhow::anyhow!("coreach needs --input"))?;
            let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
            let (bx, v) = &items[limit.min(items.len() - 1)];
            let (m, rest) = crate::families::Mech::new(*bx, v).ok_or_else(|| anyhow::anyhow!("invalid"))?;
            let t0 = std::time::Instant::now();
            let (c, complete) = ms::coreach(&m, &rest, cap);
            println!("co-reachable (can reach rest): {} states, complete {complete} ({:.1}s)", c.len(), t0.elapsed().as_secs_f64());
        }
        "catlist" => {
            for &(w, h) in &bxs {
                for l in ms::catalog_rows(&catalog, w, h)? {
                    println!("{l}");
                }
            }
        }
        "export" => {
            let f = input.ok_or_else(|| anyhow::anyhow!("export needs --input (lines: id x0 y0 pieces)"))?;
            anyhow::ensure!(bxs.len() == 1, "export needs one --boxes WxH");
            let items = ms::read_ided(&f, bxs[0].0, bxs[0].1)?;
            let t = tag.clone().unwrap_or_else(|| format!("list-{}x{}", bxs[0].0, bxs[0].1));
            ms::export(version, &items, &out_dir, &t, cap, gap, workers, extra_offsets)?;
        }
        "helpers" => {
            let cat = ms::catalog_set(&catalog)?;
            let f = input.ok_or_else(|| anyhow::anyhow!("helpers needs --input"))?;
            anyhow::ensure!(bxs.len() == 1, "helpers needs one --boxes WxH");
            let base = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
            let mut items = ms::helper_variants(&base, two.max(1));
            eprintln!("[mechspace] {} base candidates -> {} helper variants", base.len(), items.len());
            if limit > 0 && items.len() > limit {
                items.truncate(limit);
            }
            let t = tag.clone().unwrap_or_else(|| format!("helpers-{}x{}", bxs[0].0, bxs[0].1));
            let o = mk(t);
            let found = ms::run_list(&items, &o, false)?;
            ms::handle_found(&found, &o, &cat)?;
        }
        "frame-export" => {
            mechspace_frame_export(version, input, bxs, cap, cap_lean, tag, out_dir, gap, workers, extra_offsets)?;
        }
        "enum" => {
            mechspace_enum(catalog, bxs, two, carry, bottom_su, limit, tag, &mk)?;
        }
        "movable" => {
            mechspace_movable(input, bxs, threads, cap, cap_lean, tag, out_dir)?;
        }
        "reachtest" => {
            // how the reach-matching condition splits already decided sets (a partial TSV:
            // line \t verdict ...); FOUND ones must all pass
            let f = input.ok_or_else(|| anyhow::anyhow!("reachtest needs --input"))?;
            anyhow::ensure!(bxs.len() == 1, "reachtest needs one --boxes WxH");
            let tally = ms::reach_tally(&f, bxs[0].0, bxs[0].1)?;
            for ((vd, ok), n) in tally {
                println!("{vd}\t{}\t{n}", if ok { "pass" } else { "PRUNED" });
            }
        }
        "variants" => {
            mechspace_variants(input, bxs, tag, out_dir)?;
        }
        "layouts" => {
            mechspace_layouts(input, bxs, tag, out_dir)?;
        }
        "gencheck" => {
            // is each input mechanism (pistons only, or a superset) among the generator's
            // candidates with the flags given?
            let f = input.ok_or_else(|| anyhow::anyhow!("gencheck needs --input"))?;
            anyhow::ensure!(bxs.len() == 1, "gencheck needs one --boxes WxH");
            let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
            let cands = ms::candidates(bxs[0].0, bxs[0].1, pistons, two, carry, &vm);
            eprintln!("[mechspace gencheck] {} candidates", cands.len());
            for (bx, v) in &items {
                let (exact, sup) = ms::gen_member(&cands, bx, v);
                println!("{}: exact {exact}, supersets {sup}", ms::line_of(bx, v));
            }
        }
        c => anyhow::bail!("mechspace: unknown command {c}"),
    }
    Ok(())
}

/// The model's switches (families' and mechspace's atomics, --door-offset).
fn mechspace_switches(args: &MechspaceArgs) -> Result<()> {
    let MechspaceArgs { no_door_bodies, full_frame, no_ext_pullers, ext_max, ext_apart, bottom_su, frame9, ref door_offset, lift_ext, carry_in, .. } = *args;
    crate::families::LIFT_EXT.store(lift_ext, std::sync::atomic::Ordering::Relaxed);
    crate::families::CARRY_IN.store(carry_in, std::sync::atomic::Ordering::Relaxed);
    use crate::mechspace as ms;
    if no_door_bodies {
        ms::NO_DOOR_BODIES.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[mechspace] sub-model: no piston body inside the 3x3");
    }
    if std::env::var("MECHSPACE_SWEEP").is_ok() {
        ms::SWEEP.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[mechspace] sweep mode: exact on small sets, A* cycle search, exact up to --cap-lean");
    }
    if full_frame {
        ms::FULL_FRAME.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[mechspace] FULL FRAME: frame cells air when open stay air on the cycle (static stones there), frame 12/12 checked on the augmented mechanism");
    }
    crate::families::EXT_PULLERS.store(!no_ext_pullers, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_MAX.store(ext_max, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_ADJACENT.store(!ext_apart, std::sync::atomic::Ordering::Relaxed);
    if no_ext_pullers {
        eprintln!("[mechspace] generator: 1-move exits use retracted pullers only (--no-ext-pullers)");
    }
    if bottom_su {
        crate::families::BOTTOM_SU.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[mechspace] generator: exactly one piston in the bottom row, sticky facing up");
    }
    if frame9 {
        ms::FRAME9.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[mechspace] open = all 9 door blocks in frame cells");
    }
    if let Some(o) = &door_offset {
        let (a, b) = o.split_once(',').ok_or_else(|| anyhow::anyhow!("--door-offset dx,dy"))?;
        let _ = ms::DOOR_OFFSET.set((a.parse()?, b.parse()?));
    }
    Ok(())
}

/// `door mechspace frame-export`.
fn mechspace_frame_export(version: &str, input: Option<PathBuf>, bxs: Vec<(i8, i8)>, cap: usize, cap_lean: usize, tag: Option<String>, out_dir: PathBuf, gap: u32, workers: usize, extra_offsets: u32) -> Result<()> {
    use crate::mechspace as ms;
    // stone-free candidates (id x0 y0 pieces): solve, add static stones in the
    // frame cells the open state leaves air, replay-check, export catalog-style
    let f = input.ok_or_else(|| anyhow::anyhow!("frame-export needs --input"))?;
    anyhow::ensure!(bxs.len() == 1, "frame-export needs one --boxes WxH");
    let items = ms::read_ided(&f, bxs[0].0, bxs[0].1)?;
    let mut aug_items = Vec::new();
    for (id, bx, v) in &items {
        let Some((m, rest)) = crate::families::Mech::new(*bx, v) else { continue };
        let cx = ms::Cx::new(&m, &rest);
        let (vd, _) = ms::decide_all(&cx, cap.min(500_000), cap_lean.max(cap), 0);
        if let ms::Verdict::Found { open, .. } = vd {
            match ms::augment_frame(*bx, v, &open, cap, cap_lean) {
                Some((aug, _, _)) => aug_items.push((id.clone(), *bx, aug)),
                None => eprintln!("[mechspace] {id}: no static-stone frame"),
            }
        } else {
            eprintln!("[mechspace] {id}: {}", vd.tag());
        }
    }
    let t = tag.clone().unwrap_or_else(|| format!("frame-{}x{}", bxs[0].0, bxs[0].1));
    ms::export(version, &aug_items, &out_dir, &t, cap, gap, workers, extra_offsets)?;
    Ok(())
}

/// `door mechspace enum`.
fn mechspace_enum(catalog: PathBuf, bxs: Vec<(i8, i8)>, two: usize, carry: usize, bottom_su: bool, limit: usize, tag: Option<String>, mk: &dyn Fn(String) -> crate::mechspace::RunOpts) -> Result<()> {
    use crate::mechspace as ms;
    // piston-set enumeration around the door, fewest pistons first, every survivor
    // decided exactly
    let cat = ms::catalog_set(&catalog)?;
    anyhow::ensure!(bxs.len() == 1, "enum needs one --boxes WxH");
    let (dx, dy) = ms::DOOR_OFFSET.get().copied().ok_or_else(|| anyhow::anyhow!("enum needs --door-offset"))?;
    let bx = crate::families::Bx { x0: -dx, y0: -dy, w: bxs[0].0, h: bxs[0].1 };
    let eo = ms::EnumOpts { side_absent: two, free_max: carry.max(1), free_normal: std::env::var("MECHSPACE_FREE_NORMAL").is_ok(), bottom_su };
    if std::env::var("MECHSPACE_REACH").is_ok() {
        ms::REACH_PRUNE.store(true, std::sync::atomic::Ordering::Relaxed);
        eprintln!("[mechspace enum] reach-matching pruning on");
    }
    let t0 = std::time::Instant::now();
    let sets = ms::enum_sets(bx, &eo);
    if let Ok(exp) = std::env::var("MECHSPACE_EXPECT") {
        let mut want: Vec<crate::families::Pc> = exp.split_whitespace().map(ms::parse_piece).collect::<anyhow::Result<_>>()?;
        want.sort();
        println!("[mechspace enum] expected set {} generated: {}", exp, sets.iter().any(|v| *v == want));
    }
    let mut items: Vec<(crate::families::Bx, Vec<crate::families::Pc>)> = sets.into_iter().map(|v| (bx, v)).collect();
    eprintln!("[mechspace enum] {} piston sets survive pruning ({:.1}s)", items.len(), t0.elapsed().as_secs_f64());
    if limit > 0 && items.len() > limit {
        items.truncate(limit);
    }
    let t = tag.clone().unwrap_or_else(|| format!("enum-{}x{}", bxs[0].0, bxs[0].1));
    let o = mk(t);
    let found = ms::run_list(&items, &o, false)?;
    ms::handle_found(&found, &o, &cat)?;
    Ok(())
}

/// `door mechspace movable`.
fn mechspace_movable(input: Option<PathBuf>, bxs: Vec<(i8, i8)>, threads: usize, cap: usize, cap_lean: usize, tag: Option<String>, out_dir: PathBuf) -> Result<()> {
    use crate::mechspace as ms;
    // rule (A): candidates without a static-extras cycle, retried with 3 movable
    // extra door-type blocks in the open state's empty frame cells
    let f = input.ok_or_else(|| anyhow::anyhow!("movable needs --input"))?;
    anyhow::ensure!(bxs.len() == 1, "movable needs one --boxes WxH");
    let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
    let done = std::sync::atomic::AtomicUsize::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let (bx, v) = &items[i];
                if let Some((aug, op, cl)) = ms::try_movable(*bx, v, cap, cap_lean) {
                    let line = format!("{}\t{}\t{}+{}", ms::line_of(bx, &aug), ms::line_of(bx, v), op.len(), cl.len());
                    println!("MOVABLE {line}");
                    found.lock().unwrap().push(line);
                }
                let d = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if d % 100 == 0 {
                    eprintln!("[mechspace movable] {d}/{}", items.len());
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    let t = tag.clone().unwrap_or_else(|| "movable".into());
    std::fs::write(out_dir.join(format!("{t}.movable.tsv")), found.join("\n") + "\n")?;
    eprintln!("[mechspace movable] {} of {} candidates cycle with movable extras", found.len(), items.len());
    Ok(())
}

/// `door mechspace variants`.
fn mechspace_variants(input: Option<PathBuf>, bxs: Vec<(i8, i8)>, tag: Option<String>, out_dir: PathBuf) -> Result<()> {
    use crate::mechspace as ms;
    // every retracted / extended-at-rest combination of each input layout's
    // pistons, written as a mechanism list (decide / frame-export it next)
    let f = input.ok_or_else(|| anyhow::anyhow!("variants needs --input"))?;
    anyhow::ensure!(bxs.len() == 1, "variants needs one --boxes WxH");
    let items = ms::read_list(&f, bxs[0].0, bxs[0].1)?;
    let mut lines = Vec::new();
    for (bx, v) in &items {
        for w in ms::ext_variants(bx, v, 4096) {
            lines.push(ms::line_of(bx, &w));
        }
    }
    lines.sort();
    lines.dedup();
    let t = tag.clone().unwrap_or_else(|| "variants".into());
    let p = out_dir.join(format!("{t}.txt"));
    std::fs::write(&p, lines.join("\n") + "\n")?;
    eprintln!("[mechspace variants] {} layouts -> {} variants in {}", items.len(), lines.len(), p.display());
    Ok(())
}

/// `door mechspace layouts`.
fn mechspace_layouts(input: Option<PathBuf>, bxs: Vec<(i8, i8)>, tag: Option<String>, out_dir: PathBuf) -> Result<()> {
    use crate::mechspace as ms;
    // group FULL-FRAME finds (<tag>.frame.tsv: augmented line \t stone-free line ...)
    // by layout (extension states dropped, mirror images merged); one stone-free
    // representative per layout (fewest extended pistons)
    let f = input.ok_or_else(|| anyhow::anyhow!("layouts needs --input (frame.tsv files, comma-separated)"))?;
    anyhow::ensure!(bxs.len() == 1, "layouts needs one --boxes WxH");
    let paths: Vec<PathBuf> = f.to_string_lossy().split(',').map(PathBuf::from).collect();
    let groups = ms::layouts(&paths, bxs[0].0, bxs[0].1)?;
    let t = tag.clone().unwrap_or_else(|| "layouts".into());
    let mut reps = Vec::new();
    for (k, (n, rep)) in &groups {
        println!("{n}\t{k}\t{rep}");
        reps.push(rep.clone());
    }
    std::fs::write(out_dir.join(format!("{t}.txt")), reps.join("\n") + "\n")?;
    eprintln!("[mechspace layouts] {} layouts", groups.len());
    Ok(())
}

#[derive(clap::Args)]
pub struct FamiliesArgs {
    /// Boxes "WxH,..." (sorted by volume).
    #[arg(long, default_value = "7x7,7x8,8x7,8x8,7x9,9x7,8x9,9x8,8x10,10x8,9x9")]
    boxes: String,
    /// Most pistons per mechanism.
    #[arg(long, default_value_t = 10)]
    pistons: usize,
    /// State cap of the exact solve.
    #[arg(long, default_value_t = 200000)]
    cap: usize,
    /// Pair cap of the symmetric-train product BFS.
    #[arg(long, default_value_t = 200000)]
    sym_cap: usize,
    #[arg(long, default_value_t = 48)]
    threads: usize,
    /// No lifted (elevator) providers.
    #[arg(long)]
    no_lifts: bool,
    /// No push providers (pulls only).
    #[arg(long)]
    no_pushes: bool,
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/families")]
    out_dir: PathBuf,
    /// Replay every catalog entry in the simulator with ideal inputs.
    #[arg(long)]
    verify: bool,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    /// Ticks between macro steps in the ideal schedules.
    #[arg(long, default_value_t = 6)]
    gap: u32,
    /// Solve at most this many candidates per box (0 = all; for testing).
    #[arg(long, default_value_t = 0)]
    limit: usize,
    /// Non-core perimeter blocks that may take a 2-move path (each costs a provider).
    #[arg(long, default_value_t = 1)]
    two: usize,
    /// Solve one design's mechanism instead (its bounding box grown by --check-margin).
    #[arg(long)]
    check: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    check_margin: i8,
    /// Also search with level events (A*, capped) when the exact pulse model fails.
    #[arg(long)]
    levels: bool,
    /// Strip mode: every row mechanism (horizontal pistons in one door row), exhaustive.
    #[arg(long)]
    rows: bool,
    /// Strip mode: most pistons per row.
    #[arg(long, default_value_t = 4)]
    row_pistons: usize,
    /// Strip mode: longest arm (cells left or right of the door in the row).
    #[arg(long, default_value_t = 6)]
    arm: i8,
    /// Strip mode: widest row to enumerate.
    #[arg(long, default_value_t = 99)]
    row_max_width: i8,
    /// Re-check a run's unresolved candidates (unresolved/WxH.txt; --boxes WxH) by exhaustive
    /// reachability with --cap.
    #[arg(long)]
    resolve: Option<PathBuf>,
    /// General carrying depth: reused pistons displaced 1-2 cells in ANY direction (sideways
    /// too) by carriers, carriers carried themselves up to this depth (0 = elevator lifts only).
    #[arg(long, default_value_t = 0)]
    carry: usize,
    /// Middle-row side blocks forced to leave VERTICALLY: `ml`, `mr`, `any` (ML or MR) or
    /// `both`.
    #[arg(long)]
    vmid: Option<String>,
    /// Strip mode: x columns (door coordinates) no piston may fire from, e.g.
    /// `--forbid-fire=-1,1,3` (cells no wiring can reach in a 5-tall strip box).
    #[arg(long, value_delimiter = ',', allow_hyphen_values = true)]
    forbid_fire: Vec<i8>,
    /// Strip mode: redstone blocks per row (pushable pieces that power the pistons beside
    /// them, not through their front; every state settled).
    #[arg(long, default_value_t = 0)]
    rblocks: usize,
    /// Strip mode: a redstone block also powers the piston whose FRONT touches it.
    #[arg(long)]
    r_both: bool,
    /// Pullers RETRACTED at rest only for 1-move exits (the old generator; by default
    /// extended-at-rest pullers are allowed too: c79 is in the space only with them).
    #[arg(long)]
    no_ext_pullers: bool,
    /// Accepted for old command lines: extended-at-rest pullers are the default now.
    #[arg(long, hide = true)]
    ext_pullers: bool,
    /// Most 1-move exits by a puller EXTENDED at rest per mechanism (a lever-held top row has 3,
    /// c79 2, the community 7x8 1; each one doubles a block's choices).
    #[arg(long, default_value_t = 3)]
    ext_max: usize,
    /// Extended-puller exits may sit anywhere (default: each next to an earlier one, as one level
    /// holds them all, like c79's MR/TR pullers or a lever-held top row; halves the candidates).
    #[arg(long)]
    ext_apart: bool,
    /// How near (Chebyshev) an earlier extended-puller exit each later one must be.
    #[arg(long, default_value_t = 1)]
    ext_dist: usize,
    /// Audit bound (a): a piston extended at rest may be reused as a lifted / carried provider.
    #[arg(long)]
    lift_ext: bool,
    /// Audit bound (b), with --carry: a new provider piston may rest 1-2 cells beside its
    /// action cell and be carried there (fast-clean's shuttled SD(0,3)).
    #[arg(long)]
    carry_in: bool,
    /// Pistons resting IN ring cells get escape providers (carried along the ring into a corner
    /// or out of the frame by a pull / push carrier that escapes too; c77's 7x7 w1).
    #[arg(long)]
    ring_escape: bool,
    /// --ring-escape: most escapes per candidate.
    #[arg(long, default_value_t = 3)]
    ring_escape_max: usize,
    /// --ring-escape: emit only candidates with at least one ring escape (the new space).
    #[arg(long)]
    ring_escape_only: bool,
    /// Keep only candidates whose every piston has a free cell wiring could feed it from at rest
    /// (structural, optimistic: a neighbour, or the QC cell above and its neighbours).
    #[arg(long)]
    feedable: bool,
    /// Wirability bar on every solve: at most this many actions (pulses or level toggles) per
    /// piston per edge (0 = off). The constrained search is exhaustive: no solution = proven none
    /// under the bar.
    #[arg(long, default_value_t = 0)]
    max_actions: usize,
    /// Wirability bar on every solve: no piston fires while its base is inside the 3x3.
    #[arg(long)]
    no_inside: bool,
    /// With --max-actions: at most this many actions per edge beyond one per piston (1 = one
    /// piston may act twice).
    #[arg(long)]
    extra_actions: Option<usize>,
    /// Incomparable action vectors kept per state in the constrained search (more = fewer
    /// undecided candidates, slower).
    #[arg(long, default_value_t = 3)]
    pareto: usize,
    /// Keep only solved mechanisms whose every receiver has a free feed cell (the wiring
    /// analysis' unfed list; one box-edge piston may be the lever's).
    #[arg(long)]
    fed: bool,
}

pub fn cmd_families(version: &str, args: FamiliesArgs) -> Result<()> {
    let FamiliesArgs { boxes, pistons, cap, sym_cap, threads, no_lifts, no_pushes, out_dir, verify, workers, extra_offsets, gap, limit, two, check, check_margin, levels, rows, row_pistons, arm, row_max_width, resolve, carry, vmid, forbid_fire, rblocks, r_both, no_ext_pullers, ext_pullers: _, ext_max, ext_apart, ext_dist, lift_ext, carry_in, ring_escape, ring_escape_max, ring_escape_only, feedable, max_actions, no_inside, extra_actions, pareto, fed } = args;
    crate::families::CONS_PARETO.store(pareto, std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_EXTRA.store(extra_actions.unwrap_or(usize::MAX), std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_K.store(max_actions, std::sync::atomic::Ordering::Relaxed);
    crate::families::CONS_NO_INSIDE.store(no_inside, std::sync::atomic::Ordering::Relaxed);
    crate::families::FED.store(fed, std::sync::atomic::Ordering::Relaxed);
    crate::families::RING_ESCAPE_ONLY.store(ring_escape_only, std::sync::atomic::Ordering::Relaxed);
    crate::families::RING_ESCAPE.store(ring_escape, std::sync::atomic::Ordering::Relaxed);
    crate::families::RING_ESCAPE_MAX.store(ring_escape_max, std::sync::atomic::Ordering::Relaxed);
    crate::families::FEEDABLE.store(feedable, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_DIST.store(ext_dist, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_PULLERS.store(!no_ext_pullers, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_MAX.store(ext_max, std::sync::atomic::Ordering::Relaxed);
    crate::families::EXT_ADJACENT.store(!ext_apart, std::sync::atomic::Ordering::Relaxed);
    crate::families::LIFT_EXT.store(lift_ext, std::sync::atomic::Ordering::Relaxed);
    crate::families::CARRY_IN.store(carry_in, std::sync::atomic::Ordering::Relaxed);
    let vmid: Vec<Vec<(i8, i8)>> = match vmid.as_deref() {
        None => Vec::new(),
        Some("ml") => vec![vec![(0, 1)]],
        Some("mr") => vec![vec![(2, 1)]],
        Some("any") => vec![vec![(0, 1)], vec![(2, 1)]],
        Some("both") => vec![vec![(0, 1), (2, 1)]],
        Some(x) => anyhow::bail!("--vmid ml|mr|any|both, got {x}"),
    };
    if let Some(f) = resolve {
        let b = crate::families::parse_boxes(&boxes)?;
        anyhow::ensure!(b.len() == 1, "--resolve needs one --boxes WxH");
        return crate::families::resolve_cli(&f, b[0].0, b[0].1, cap, threads);
    }
    if rows {
        let _ = crate::families::RMAX.set(rblocks);
        let _ = crate::families::RBOTH.set(r_both);
        if !forbid_fire.is_empty() {
            let _ = crate::families::FORBID_X.set(forbid_fire.clone());
            eprintln!("[families rows] no piston fires from x in {forbid_fire:?}");
        }
        let o = crate::families::RowOpts { kmax: row_pistons, arm, cap, sym_cap, threads, out_dir, max_w: row_max_width, gap };
        return crate::families::rows_cli(&o);
    }
    if let Some(p) = check {
        return crate::families::check(
            &p,
            check_margin,
            cap,
            sym_cap,
            verify.then(|| crate::families::CheckVerify {
                version: version.to_string(),
                out_dir: out_dir.clone(),
                gap,
                extra: extra_offsets as u32,
                name: p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "check".into()),
            }),
        );
    }
    let o = crate::families::Opts {
        boxes: crate::families::parse_boxes(&boxes)?,
        pmax: pistons,
        cap,
        sym_cap,
        threads,
        lifts: !no_lifts,
        pushes: !no_pushes,
        out_dir,
        verify,
        workers,
        extra: extra_offsets,
        gap,
        limit,
        two,
        carry,
        vmid,
        levels,
    };
    crate::families::run(version, &o)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct TopologyArgs {
    tsv: PathBuf,
    /// Only ids starting with one of these (e.g. `7x8,8x7`)
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Print each mechanism's full report
    #[arg(long)]
    verbose: bool,
}

pub fn cmd_topology(args: TopologyArgs) -> Result<()> {
    let TopologyArgs { tsv, only, verbose } = args;
    crate::topology::catalog_cli(&tsv, &only, verbose)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct ConnectArgs {
    tsv: PathBuf,
    /// Only ids starting with one of these (e.g. `7x8,7x9-1`)
    #[arg(long, value_delimiter = ',')]
    only: Vec<String>,
    /// Boxes "WxH,..." (each entry is placed at every offset that fits)
    #[arg(long, default_value = "7x9")]
    boxes: String,
    /// State cap per edge search
    #[arg(long, default_value_t = 300000)]
    cap: usize,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// Print each placement's region report
    #[arg(long)]
    verbose: bool,
}

pub fn cmd_connect(args: ConnectArgs) -> Result<()> {
    let ConnectArgs { tsv, only, boxes, cap, threads, verbose } = args;
    let b = crate::families::parse_boxes(&boxes)?;
    crate::connect::catalog_cli(&tsv, &only, &b, cap, threads, verbose)?;
    Ok(())
}
