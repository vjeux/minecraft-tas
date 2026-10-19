//! `door synth`, `door resynth`: exact circuit synthesis.

use anyhow::Result;
use mctas::sim::WorkerConfig;
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct SynthArgs {
    /// The context: a door design or a gadget harness (cells in --clear are emptied first).
    context: PathBuf,
    #[arg(long)]
    lo: String,
    #[arg(long)]
    hi: String,
    /// Circuit blocks (see synth::palette_states): block,gt,glass,dust,dot,rep,rep1..4,cmp,obs,torch,rblock,lamp,target,note,...
    #[arg(long, default_value = "block,dust,rep4")]
    palette: String,
    /// Blocks added under parts that need support (they count).
    #[arg(long, default_value = "block")]
    floors: String,
    /// Most blocks per circuit.
    #[arg(long, default_value_t = 4)]
    k: usize,
    /// Skip circuits smaller than this.
    #[arg(long, default_value_t = 1)]
    min_k: usize,
    /// Source cells of the context, "x,y,z;...": the lever, a powered block, an observer...
    #[arg(long)]
    from: String,
    /// Cells that change while the machine runs (moving blocks, pistons; air allowed):
    /// observers may watch them.
    #[arg(long, default_value = "")]
    watch: String,
    /// Sink cells: some circuit block must drive one of them.
    #[arg(long)]
    to: String,
    /// Context cells to empty first (the old circuit), "x,y,z;...".
    #[arg(long, default_value = "")]
    clear: String,
    /// Cells the circuit may not use, "x,y,z;...".
    #[arg(long, default_value = "")]
    forbid: String,
    /// Parts constraints: at least n of a kind, "repeater:2,observer:1".
    #[arg(long, default_value = "")]
    require: String,
    /// Parts constraints: at most n of a kind, "repeater:1".
    #[arg(long, default_value = "")]
    limit: String,
    /// Keep only circuits using at least one of these (lamp, bulb, note, target or any
    /// block-state substring), comma list.
    #[arg(long, default_value = "")]
    require_any: String,
    /// Behavior spec (see synth.rs); without it the target is the door judge.
    #[arg(long)]
    spec: Option<PathBuf>,
    /// World offsets a spec is checked at (the judge's list).
    #[arg(long, default_value_t = 2)]
    spec_offsets: usize,
    /// Keep circuits with blocks that are on no source-to-sink path.
    #[arg(long)]
    loose: bool,
    /// Only count the candidates.
    #[arg(long)]
    count: bool,
    /// Keep going after the first size with a verified circuit.
    #[arg(long)]
    all_sizes: bool,
    /// Stop after this many verified circuits (0 = every one of the smallest size).
    #[arg(long, default_value_t = 0)]
    want: usize,
    /// Write every verified circuit here (synth-<size>-<i>.txt).
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 24)]
    workers: usize,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    screen_offsets: usize,
    #[arg(long, default_value_t = 200)]
    screen_ticks: u32,
    #[arg(long, default_value_t = 50_000_000)]
    max_cands: usize,
    /// Speed bounds (speed boards): reject doors that open later than this many ticks.
    #[arg(long)]
    max_open: Option<u32>,
    /// Speed bounds: reject doors that close later than this many ticks.
    #[arg(long)]
    max_close: Option<u32>,
    /// Judge every circuit as a tile module (tile.rs; see `door verify --tile`).
    #[arg(long)]
    tile: Option<String>,
}

pub fn cmd_synth(version: &str, args: SynthArgs) -> Result<()> {
    let SynthArgs {
        context,
        lo,
        hi,
        palette,
        floors,
        k,
        min_k,
        from,
        watch,
        to,
        clear,
        forbid,
        require,
        limit,
        require_any,
        spec,
        spec_offsets,
        loose,
        count,
        all_sizes,
        want,
        out_dir,
        workers,
        extra_offsets,
        screen_offsets,
        screen_ticks,
        max_cands,
        max_open,
        max_close,
        tile,
    } = args;
    use crate::synth::*;
    let tile = tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
    let mut base = load_design(&context)?;
    for p in parse_cells(&clear)? {
        anyhow::ensure!(base.blocks.remove(&p).is_some(), "--clear {} is already air", fmt_pos(p));
    }
    let cell = |s: &str| -> Result<mctas::sim::Pos> {
        let v = parse_cells(s)?;
        anyhow::ensure!(v.len() == 1, "expected one x,y,z, got {s:?}");
        Ok(v[0])
    };
    let (a, b) = (cell(&lo)?, cell(&hi)?);
    let o = SynthOptions {
        lo: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
        hi: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        palette: palette_states(&palette)?,
        floors: palette_states(&floors)?,
        k,
        min_k,
        sources: parse_cells(&from)?,
        watch: parse_cells(&watch)?,
        sinks: parse_cells(&to)?,
        forbid: parse_cells(&forbid)?.into_iter().collect(),
        require: parse_counts(&require)?,
        limit: parse_counts(&limit)?,
        require_any: require_any_names(&require_any),
        loose,
        max_cands,
    };
    let en = Gen::new(&base, &o)?;
    let t = std::time::Instant::now();
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    log(&format!("{} atoms ({} palette states, {} floors) in the box", en.atoms.len(), o.palette.len(), o.floors.len()));
    // Enumerate before starting the JVMs (they would sit idle meanwhile).
    let buckets = en.enumerate(&log)?;
    if count {
        return Ok(());
    }
    let offsets = crate::search::judge_offsets(extra_offsets);
    let target = match &spec {
        Some(path) => Target::Spec {
            spec: Spec::parse(&std::fs::read_to_string(path)?)?,
            offsets: offsets.iter().take(spec_offsets.max(1)).copied().collect(),
        },
        None => Target::Door {
            screen: VerifyOptions {
                cycles: 1,
                max_ticks: screen_ticks,
                offsets: offsets.iter().take(screen_offsets.max(1)).copied().collect(),
                keep_frames: false,
                empty_walkway: true,
                tile: tile.clone(),
            },
            judge: VerifyOptions { offsets: offsets.clone(), tile: tile.clone(), ..Default::default() },
            max_open,
            max_close,
            rotations: false,
            seamless: None,
            repeater_input: false,
            eligible: false,
            shape: None,
        },
    };
    let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1536, ..Default::default() })?;
    log(&format!("{} workers", pool.len()));
    let found = synth(&pool, &base, &en, &buckets, &target, min_k, all_sizes, want, &log)?;
    if found.is_empty() {
        println!("nothing found");
    }
    for (i, (set, d, msg)) in found.iter().enumerate() {
        println!("FOUND {} blocks: {} [{}] -- {msg}", set.len(), en.describe(set), en.kinds(set));
        if let Some(dir) = &out_dir {
            std::fs::create_dir_all(dir)?;
            let mut w = pool.worker(0);
            let canon = match crate::canonicalize(&mut w, d) {
                Ok(c) if spec.is_some() || crate::verify(&mut w, &c, &VerifyOptions::default()).is_ok() => c,
                _ => d.clone(),
            };
            let path = dir.join(format!("synth-{}-{i}.txt", set.len()));
            canon.save(&path)?;
            println!("  written to {}", path.display());
        }
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct ResynthArgs {
    #[arg(required = true, num_args = 1..)]
    designs: Vec<PathBuf>,
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 2)]
    min_size: usize,
    #[arg(long, default_value_t = 4)]
    max_size: usize,
    /// Circuit blocks (see synth.rs `palette_states`); `trapdoor` (an iron trapdoor: a
    /// non-conducting power indicator an observer can watch) shortened six doors on the
    /// exotic track.
    #[arg(long, default_value = "block,dust,dot,rep,obs,torch,trapdoor")]
    palette: String,
    #[arg(long, default_value = "block")]
    floors: String,
    /// The search box is the group's bounding box grown by this.
    #[arg(long, default_value_t = 1)]
    margin: i32,
    /// Skip groups with more candidate circuits than this.
    #[arg(long, default_value_t = 300_000)]
    max_cands: usize,
    #[arg(long)]
    loose: bool,
    /// Only groups inside this box, "x0,y0,z0:x1,y1,z1" (inclusive).
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value_t = 24)]
    workers: usize,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 2)]
    screen_offsets: usize,
    /// Settle cap while screening (the door settles in < 60 ticks; clocks hit the cap).
    #[arg(long, default_value_t = 100)]
    screen_ticks: u32,
    /// Speed bounds (speed boards): reject doors that open later than this many ticks.
    #[arg(long)]
    max_open: Option<u32>,
    /// Speed bounds: reject doors that close later than this many ticks.
    #[arg(long)]
    max_close: Option<u32>,
    /// After an improvement, search every group again (default: only the groups whose
    /// neighborhood, the blocks within margin + 3, changed).
    #[arg(long)]
    global_retry: bool,
    /// Keep only replacement circuits using at least one of these (lamp, bulb, note,
    /// target or any block-state substring), comma list: sweeps new palette entries on a
    /// design the old palette already exhausted.
    #[arg(long, default_value = "")]
    require_any: String,
    /// Judge every candidate as a tile module (tile.rs; see `door verify --tile`).
    #[arg(long)]
    tile: Option<String>,
    /// Accept only replacements at least this seamless (quart, semi, full, supercov, super,
    /// optionally `+hall`; `door properties`' definition, one recorded cycle at the design's
    /// own position).
    #[arg(long)]
    seamless: Option<String>,
    /// With --seamless, repair mode: while the design is below the tier, cut only groups
    /// through an offending cell (single blocks too) and let each grow back with up to this
    /// many MORE blocks than it had; once it reaches the tier, the usual shrinking search.
    #[arg(long, default_value_t = 0)]
    extra: usize,
    /// Accept replacements that are not rotation-safe. By default a rotation-safe start
    /// design only accepts rotation-safe replacements (the boards rank rotation-safe doors).
    #[arg(long)]
    any_rotation: bool,
    /// Record eligibility: accept only designs that also work with repeater input (see
    /// `door minimize --repeater-input`).
    #[arg(long)]
    repeater_input: bool,
    /// --repeater-input plus the hallway check, footsteps (sculk) and QUART SEAMLESS or better.
    #[arg(long)]
    eligible: bool,
    /// Shape limits on every accepted replacement, measured on the swept shape (a preset or
    /// key<=n terms, e.g. `thick<=1,volume<=81`; see `door shapes`).
    #[arg(long)]
    shape: Option<String>,
    /// Compaction (the volume boards): keep every replacement circuit inside this box,
    /// "x0,y0,z0:x1,y1,z1" (inclusive); while removable blocks lie outside it, cut only
    /// groups through them and let each grow back inside the box with up to --extra more
    /// blocks than it had (every accepted replacement leaves fewer blocks outside). Pistons,
    /// the lever and its support are never moved: they must already lie inside.
    #[arg(long)]
    fit: Option<String>,
}

pub fn cmd_resynth(version: &str, args: ResynthArgs) -> Result<()> {
    let ResynthArgs { designs, out, min_size, max_size, palette, floors, margin, max_cands, loose, only, workers, extra_offsets, screen_offsets, screen_ticks, max_open, max_close, global_retry, require_any, tile, any_rotation, seamless, extra, repeater_input, eligible, shape, fit } = args;
    let shape = shape.as_deref().map(crate::ShapeLimits::parse).transpose()?;
    use crate::synth::*;
    let seamless = seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?;
    let tile = tile.as_deref().map(crate::tile::Tile::parse).transpose()?;
    let parse_box = |flag: &str, s: &str| -> Result<(mctas::sim::Pos, mctas::sim::Pos)> {
        let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--{flag} wants x0,y0,z0:x1,y1,z1"))?;
        let a = parse_cells(a)?.first().copied().ok_or_else(|| anyhow::anyhow!("bad --{flag} corner {a:?}"))?;
        let b = parse_cells(b)?.first().copied().ok_or_else(|| anyhow::anyhow!("bad --{flag} corner {b:?}"))?;
        Ok(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
    };
    let only = only.as_deref().map(|s| parse_box("only", s)).transpose()?;
    let fit = fit.as_deref().map(|s| parse_box("fit", s)).transpose()?;
    let sweep = designs.len() > 1;
    if sweep {
        std::fs::create_dir_all(&out)?;
    }
    let offsets = crate::search::judge_offsets(extra_offsets);
    let judge = VerifyOptions { offsets: offsets.clone(), tile: tile.clone(), ..Default::default() };
    let screen = VerifyOptions { cycles: 1, max_ticks: screen_ticks, offsets: offsets.iter().take(screen_offsets.max(1)).copied().collect(), keep_frames: false, empty_walkway: true, tile };
    let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1536, ..Default::default() })?;
    let t = std::time::Instant::now();
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let mut summary: Vec<String> = Vec::new();
    for design in &designs {
        let d = load_design(design)?;
        let target = if sweep { out.join(format!("{}-rs.txt", design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "design".into()))) } else { out.clone() };
        if sweep {
            log(&format!("=== {} ({} blocks)", design.display(), crate::block_count(&d)));
        }
        let mut o = ResynthOptions {
            min_size,
            max_size,
            palette: palette_states(&palette)?,
            floors: palette_states(&floors)?,
            margin,
            max_cands,
            loose,
            max_open,
            max_close,
            save_to: Some(target.clone()),
            global_retry,
            require_any: require_any_names(&require_any),
            only,
            keep_rotations: false,
            seamless,
            extra,
            repeater_input: repeater_input || eligible,
            eligible,
            shape,
            fit,
        };
        let start = {
            let mut w = pool.worker(0);
            match crate::verify(&mut w, &d, &judge) {
                Ok(r) => {
                    let (o, c) = crate::search::worst_ticks(&r);
                    log(&format!("start: {} blocks, open {o} / close {c}", crate::block_count(&d)));
                    if max_open.is_none_or(|m| o <= m) && max_close.is_none_or(|m| c <= m) {
                        if repeater_input || eligible {
                            // The record-eligibility filter must hold for the start design too.
                            let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
                            let r = crate::search::eligibility(&mut w, sb, &d, true, eligible, &judge.offsets);
                            let _ = w.drop_sandbox(sb);
                            match r {
                                Ok(()) => {
                                    log("the start design passes the eligibility filter (repeater input everywhere)");
                                    Ok(())
                                }
                                Err(e) => Err(format!("the start design fails the eligibility filter: {e:#}")),
                            }
                        } else {
                            Ok(())
                        }
                    } else {
                        Err(format!("the start design misses the speed bounds (open {o} / close {c})"))
                    }
                }
                Err(e) => Err(format!("the start design does not pass the judge: {e:#}")),
            }
        };
        if let Err(e) = start {
            anyhow::ensure!(sweep, "{e}");
            log(&format!("skipped: {e}"));
            summary.push(format!("{}: skipped ({e})", design.display()));
            continue;
        }
        if !any_rotation {
            let mut w = pool.worker(0);
            match crate::rotation_check(&mut w, &d, &judge)? {
                None => {
                    o.keep_rotations = true;
                    log("the start design is rotation-safe: replacements must stay rotation-safe (--any-rotation to allow others)");
                }
                Some((turns, _)) => log(&format!("the start design is NOT rotation-safe (turned {} degrees): rotations not checked", turns as u32 * 90)),
            }
        }
        let best = resynth(&pool, &d, &o, &judge, &screen, &log)?;
        if sweep && crate::block_count(&best) >= crate::block_count(&d) {
            summary.push(format!("{}: {} blocks, nothing smaller", design.display(), crate::block_count(&d)));
            continue;
        }
        // Keep the canonical rest state only if it still passes the judge (a design written
        // with stale redstone states can settle into another machine).
        let canon = {
            let mut w = pool.worker(0);
            match crate::canonicalize(&mut w, &best) {
                Ok(c) if crate::verify(&mut w, &c, &judge).is_ok() => c,
                _ => match settled_design(&mut w, &best) {
                    Ok(s) if crate::verify(&mut w, &s, &judge).is_ok() => {
                        println!("the design moves while it settles: writing its settled state");
                        s
                    }
                    _ => {
                        println!("canonical and settled forms fail the judge: writing the design as found");
                        best.clone()
                    }
                },
            }
        };
        canon.save(&target)?;
        println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&canon), target.display());
        summary.push(format!("{}: {} -> {} blocks, {}", design.display(), crate::block_count(&d), crate::block_count(&canon), target.display()));
    }
    if sweep {
        println!("sweep summary:");
        for s in &summary {
            println!("  {s}");
        }
    }
    Ok(())
}
