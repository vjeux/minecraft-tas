//! `door verify`, `door tile-board`, `door tile-scan`: the judge, and tiling.

use anyhow::Result;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct VerifyArgs {
    design: PathBuf,
    /// Print the tick-by-tick opening/closing of the 3x3 opening.
    #[arg(long)]
    timeline: bool,
    /// Open/close cycles per position.
    #[arg(long, default_value_t = 3)]
    cycles: u32,
    /// Test at this many extra pseudo-random positions on top of the default four.
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    /// If it works, keep a copy in the archive folder next to the design (archive/<name>-<blocks>b.txt), unless an identical design is already archived.
    #[arg(long)]
    archive: bool,
    /// Archive into this folder instead (e.g. the main designs/piston-door-3x3/archive).
    #[arg(long)]
    archive_dir: Option<PathBuf>,
    /// Skip the rotation check (the door turned 90, 180 and 270 degrees, same positions).
    /// It is not part of the judge: it decides which board a working door goes on.
    #[arg(long)]
    no_rotations: bool,
    /// Also run all 8 orientations (4 turns, each as built and mirrored) and print which work.
    #[arg(long)]
    orientations: bool,
    /// Judge the design as a tile module (tile.rs): `wide:N` (openings side by side, one
    /// 3 x 3N opening), `period:P:N` (an opening every P blocks), `stack:N` (a 3N x 3
    /// opening), `vperiod:Q:N` (an opening every Q blocks up) or `step:dx,dy,dz:N`. Copy 0
    /// keeps the lever; the other copies get the module's `relay:` block in its cell.
    #[arg(long)]
    tile: Option<String>,
    /// With --tile: `one` (the tiling: one lever) or `each` (diagnostic: every copy keeps
    /// its lever, all flipped in the same tick).
    #[arg(long, default_value = "one")]
    tile_levers: String,
    /// With --tile: write the whole arrangement here (a plain design file).
    #[arg(long)]
    tile_out: Option<PathBuf>,
    /// With --tile: a shared end piece placed once in copy 0's frame (its `port lever`, if
    /// any, is the one lever; every copy then holds the relay).
    #[arg(long)]
    tile_extra: Option<PathBuf>,
    /// Also decide the "on the ground" board: the swept shape (nothing below the door's
    /// bottom row at rest or at any tick of a cycle) and the ground check (the judge's
    /// positions with the door standing on a stone floor).
    #[arg(long)]
    ground: bool,
}

pub fn cmd_verify(version: &str, args: VerifyArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let VerifyArgs { design, timeline: show, cycles, archive, extra_offsets, archive_dir, no_rotations, orientations, tile, tile_levers, tile_out, tile_extra, ground } = args;
    let d = load_design(&design)?;
    if let Some(spec) = tile {
        let mut t = crate::tile::Tile::parse(&spec)?;
        if let Some(p) = &tile_extra {
            t.extra = Some(load_design(p)?);
        }
        t.levers = match tile_levers.as_str() {
            "one" => crate::tile::LeverMode::One,
            "each" => crate::tile::LeverMode::Each,
            other => anyhow::bail!("--tile-levers {other}: expected one or each"),
        };
        let mut w = Worker::spawn(&cfg)?;
        let mut opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
        opts.offsets.extend(crate::extra_offsets(extra_offsets));
        return tile_verify_cmd(&mut w, &design, &d, &t, &opts, no_rotations, archive, archive_dir.as_deref(), tile_out.as_deref());
    }
    let spec = DoorSpec::from_design(&d)?;
    let mut w = Worker::spawn(&cfg)?;
    let mut opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
    opts.offsets.extend(crate::extra_offsets(extra_offsets));
    match verify(&mut w, &d, &opts) {
        Ok(r) => {
            println!(
                "WORKS: {} ({} blocks{}, {}x{}x{}, category {}{}{})",
                d.name,
                r.blocks,
                match crate::entity_count(&d) {
                    0 => String::new(),
                    n => format!(" + {n} entit{}", if n == 1 { "y" } else { "ies" }),
                },
                r.size[0],
                r.size[1],
                r.size[2],
                crate::category(&d),
                if crate::slime_free(&d) { ", slime-free" } else { "" },
                if crate::single_layer(&d) { ", flat" } else { "" }
            );
            if archive {
                let note = format!("verified WORKS: {} blocks, open after {} ticks, closed after {} ticks", r.blocks, r.cycles[0].open_ticks, r.cycles[0].close_ticks);
                match crate::archive::save(&design, &d, r.blocks, &note, archive_dir.as_deref())? {
                    Some(p) => println!("archived {}", p.display()),
                    None => println!("already archived"),
                }
            }
            for (i, c) in r.cycles.iter().enumerate() {
                println!(
                    "  cycle {i}: open after {} ticks (idle at {}), closed after {} ticks (idle at {})",
                    c.open_ticks, c.open_settle, c.close_ticks, c.close_settle
                );
            }
            for warning in &r.warnings {
                println!("  warning: {warning}");
            }
            if !no_rotations {
                let ropts = VerifyOptions { keep_frames: false, ..opts.clone() };
                match crate::rotation_check(&mut w, &d, &ropts)? {
                    None => println!("rotation-safe: also works turned 90, 180 and 270 degrees ({} positions each)", ropts.offsets.len()),
                    Some((turns, why)) => println!("NOT rotation-safe (listed on the separate board): turned {} degrees: {why}", turns as u32 * 90),
                }
            }
            if ground {
                let swept = crate::swept_shape(&mut w, &d)?;
                let gopts = VerifyOptions { keep_frames: false, ..opts.clone() };
                if swept.below > 0 {
                    println!("NOT on the ground: {} row(s) below the door at rest or while moving", swept.below);
                } else {
                    match crate::ground_check(&mut w, &d, &gopts)? {
                        None => println!("on the ground: nothing below the door at any tick, works on a stone floor ({} positions)", gopts.offsets.len()),
                        Some(why) => println!("NOT on the ground: fails standing on a stone floor: {}", why.chars().take(200).collect::<String>()),
                    }
                }
            }
            if orientations {
                let ropts = VerifyOptions { keep_frames: false, ..opts.clone() };
                println!("orientations ({} positions each; turns are clockwise seen from above, mirrored = east-west mirror image first):", ropts.offsets.len());
                for (turns, mirrored, r) in crate::orientation_report(&mut w, &d, &ropts)? {
                    let name = format!("{:>3} degrees{}", turns as u32 * 90, if mirrored { ", mirrored" } else { "" });
                    match r {
                        Ok(()) => println!("  {name:<22} WORKS"),
                        Err(e) => println!("  {name:<22} BROKEN: {}", e.chars().take(140).collect::<String>()),
                    }
                }
            }
            if show {
                println!("opening (top row, middle row, bottom row; # block, ~ moving, h piston head, . air):");
                print!("{}", timeline(&r.open_frames, &spec));
                println!("closing:");
                print!("{}", timeline(&r.close_frames, &spec));
            }
        }
        Err(e) => {
            println!("BROKEN: {}: {e:#}", d.name);
            std::process::exit(1);
        }
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct TileBoardArgs {
    /// Ref prefix to scan besides HEAD.
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// Skip designs with more blocks than this.
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    /// Write the markdown here (also printed), e.g. designs/piston-door-3x3/TILEBOARD.md.
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn cmd_tile_board(version: &str, args: TileBoardArgs) -> Result<()> {
    let TileBoardArgs { refs, extra_offsets, workers, max_blocks, out } = args;
    let cands = crate::leaderboard::collect(std::path::Path::new("."), &refs, max_blocks)?;
    let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let mut opts = VerifyOptions::default();
    opts.offsets.extend(crate::extra_offsets(extra_offsets));
    let (_, md) = crate::tile::board(&pool, cands, &opts);
    print!("{md}");
    if let Some(p) = out {
        std::fs::write(p, &md)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct TileScanArgs {
    /// Design files (or @builtins).
    designs: Vec<PathBuf>,
    /// Tilings, a comma list; period and vperiod take ranges: wide:3,stack:3,period:4..12:3.
    #[arg(long, default_value = "wide:3,stack:3,period:4..12:3,vperiod:4..10:3")]
    tiles: String,
    /// `one` (copy 0 keeps the lever, the others get the module's relay) or `each`
    /// (diagnostic: every copy flips its own lever in the same tick).
    #[arg(long, default_value = "each")]
    levers: String,
    /// Judge positions on top of the default four.
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 12)]
    workers: usize,
    /// Print the reason of every failure (default: the first 160 characters).
    #[arg(long)]
    verbose: bool,
}

pub fn cmd_tile_scan(version: &str, args: TileScanArgs) -> Result<()> {
    let TileScanArgs { designs, tiles, levers, extra_offsets, workers, verbose } = args;
    let mut ts = crate::tile::parse_tile_list(&tiles)?;
    let mode = match levers.as_str() {
        "one" => crate::tile::LeverMode::One,
        "each" => crate::tile::LeverMode::Each,
        other => anyhow::bail!("--levers {other}: expected one or each"),
    };
    for t in ts.iter_mut() {
        t.levers = mode;
    }
    let ds: Vec<(String, Design)> = designs.iter().map(|p| Ok((p.display().to_string(), load_design(p)?))).collect::<Result<_>>()?;
    let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let mut opts = VerifyOptions::default();
    opts.offsets.extend(crate::extra_offsets(extra_offsets));
    let rows = crate::tile::scan(&pool, &ds, &ts, &opts);
    for (i, (name, d)) in ds.iter().enumerate() {
        println!("== {name} ({} blocks, category {}{})", crate::block_count(d), crate::category(d), if crate::slime_free(d) { ", slime-free" } else { "" });
        let mut best: std::collections::BTreeMap<String, String> = Default::default();
        for (j, t, r) in rows.iter().filter(|(j, _, _)| *j == i) {
            let _ = j;
            let kind = t.label.split(':').next().unwrap_or("").to_string();
            match r {
                Ok((total, marginal, o, c)) => {
                    println!("  {:<16} WORKS {total} blocks, +{marginal} per copy, open {o} / close {c}", t.label);
                    best.entry(kind).or_insert_with(|| format!("{} (+{marginal} per copy)", t.label));
                }
                Err(e) => {
                    let e = if verbose { e.clone() } else { e.chars().take(160).collect() };
                    println!("  {:<16} fails: {e}", t.label);
                }
            }
        }
        for (k, v) in &best {
            println!("  smallest working {k}: {v}");
        }
    }
    Ok(())
}

/// `door verify --tile`: the judge on every opening of a tiled module, with per-opening and
/// per-module block counts and timings.
#[allow(clippy::too_many_arguments)]
fn tile_verify_cmd(
    w: &mut Worker,
    path: &std::path::Path,
    d: &Design,
    t: &crate::tile::Tile,
    opts: &VerifyOptions,
    no_rotations: bool,
    archive: bool,
    archive_dir: Option<&std::path::Path>,
    tile_out: Option<&std::path::Path>,
) -> Result<()> {
    use crate::tile::*;
    let lever_mode = match t.levers {
        LeverMode::One => "one lever",
        LeverMode::Each => "DIAGNOSTIC: every copy flips its own lever",
    };
    let tiled = expand(d, t)?;
    if let Some(out) = tile_out {
        tiled.design.save(out)?;
        println!("arrangement written to {}", out.display());
    }
    let openings = tiled.specs.len();
    let total = tiled_blocks(&tiled);
    let marginal = marginal_blocks(d, t)?;
    let per_copy = openings / tiled.copies;
    let relay = relay_state(d)?.unwrap_or_else(|| "none (the lever cell stays empty)".into());
    match verify_tiled(w, d, t, opts) {
        Ok((r, per, _)) => {
            println!(
                "WORKS (tile {}, {lever_mode}): {}: {} copies, {openings} openings, {} blocks total ({:.1} per opening); module {} blocks, +{marginal} per extra copy ({:.1} per opening in a long row); category {}{}{}",
                t.label,
                d.name,
                tiled.copies,
                total,
                total as f64 / openings as f64,
                crate::block_count(d),
                marginal as f64 / per_copy as f64,
                crate::category(d),
                if crate::slime_free(d) { ", slime-free" } else { "" },
                if crate::single_layer(d) { ", flat" } else { "" }
            );
            println!("  relay in the copies without the lever: {relay}; cells shared by copies: {}", tiled.shared);
            let o = r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0);
            let c = r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0);
            println!("  all openings: open after {o} ticks, closed after {c} ticks (worst cycle, first position)");
            let line: Vec<String> = per.iter().enumerate().map(|(i, (a, b))| format!("#{i} {a}/{b}")).collect();
            println!("  per opening (open/close): {}", line.join(", "));
            for (i, cy) in r.cycles.iter().enumerate() {
                println!("  cycle {i}: open after {} ticks (idle at {}), closed after {} ticks (idle at {})", cy.open_ticks, cy.open_settle, cy.close_ticks, cy.close_settle);
            }
            for warning in &r.warnings {
                println!("  warning: {warning}");
            }
            if archive && t.levers == LeverMode::One {
                let note = format!(
                    "verified WORKS as a tile module: --tile {} ({} openings, {} blocks total, +{marginal} per extra copy), open {o} / close {c}",
                    t.label, openings, total
                );
                match crate::archive::save(path, d, marginal, &note, archive_dir)? {
                    Some(p) => println!("archived {}", p.display()),
                    None => println!("already archived"),
                }
            }
            if !no_rotations {
                let ropts = VerifyOptions { keep_frames: false, ..opts.clone() };
                match tiled_rotation_check(w, &tiled, &ropts)? {
                    None => println!("rotation-safe: the arrangement also works turned 90, 180 and 270 degrees ({} positions each)", ropts.offsets.len()),
                    Some((turns, why)) => println!("NOT rotation-safe: turned {} degrees: {why}", turns as u32 * 90),
                }
            }
            if opts.keep_frames {
                for (i, spec) in tiled.specs.iter().enumerate() {
                    println!("opening #{i} (top row, middle row, bottom row; # block, ~ moving, h piston head, . air):");
                    print!("{}", timeline(&r.open_frames, spec));
                    println!("closing #{i}:");
                    print!("{}", timeline(&r.close_frames, spec));
                }
            }
        }
        Err(e) => {
            println!("BROKEN (tile {}, {lever_mode}): {}: {e:#}", t.label, d.name);
            std::process::exit(1);
        }
    }
    Ok(())
}
