//! `door minimize`, `door harvest`, `door transplant`: local search for fewer blocks.

use anyhow::Result;
use piston_door::cli::{SearchArgs, load_design};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct MinimizeArgs {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    #[command(flatten)]
    search: SearchArgs,
    /// Stop after this many minutes.
    #[arg(long)]
    minutes: Option<f64>,
    /// Archive every improvement (archive/ next to --out).
    #[arg(long)]
    archive: bool,
    /// Rename the design (used for archive file names).
    #[arg(long)]
    name: Option<String>,
}

pub fn cmd_minimize(version: &str, args: MinimizeArgs) -> Result<()> {
    let MinimizeArgs { design, out, search, minutes, archive, name } = args;
    let mut d = load_design(&design)?;
    if let Some(n) = name {
        d.name = n;
    }
    let pool = search.pool(version)?;
    let t = std::time::Instant::now();
    let mut opts = search.options()?;
    opts.deadline = minutes.map(|m| t + std::time::Duration::from_secs_f64(m * 60.0));
    let moves = search.moves.clone();
    let judge_n = opts.judge.offsets.len();
    println!("{} workers; moves {moves}; judge {judge_n} positions", pool.len());
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let source = design.display().to_string();
    let mut on_improve = |best: &mctas::design::Design, desc: &str| -> Result<()> {
        best.save(&out)?;
        log(&format!("improved: {} blocks ({desc}), written to {}", crate::block_count(best), out.display()));
        if archive {
            let note = format!(
                "verified WORKS: {} blocks (judge: {judge_n} positions x 3 cycles); door minimize --moves {moves} from {source}",
                crate::block_count(best),
            );
            if let Some(p) = crate::archive::save(&out, best, crate::block_count(best), &note, None)? {
                log(&format!("archived {}", p.display()));
            }
        }
        Ok(())
    };
    let best = crate::search::search(&pool, &d, &opts, &log, &mut on_improve)?;
    best.save(&out)?;
    println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&best), out.display());
    Ok(())
}

#[derive(clap::Args)]
pub struct HarvestArgs {
    /// Ref prefix to scan besides HEAD.
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    /// Run `git fetch origin` first.
    #[arg(long)]
    fetch: bool,
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    /// Designs per category.
    #[arg(long, default_value_t = 2)]
    top: usize,
    /// Only designs within this many blocks of their league's best.
    #[arg(long, default_value_t = 8)]
    within: usize,
    /// Search time per design.
    #[arg(long, default_value_t = 15.0)]
    minutes: f64,
    /// Keep going: fetch and harvest again every this many minutes (0 = one pass).
    #[arg(long, default_value_t = 0.0)]
    every: f64,
    /// With --every, stop after this many hours.
    #[arg(long, default_value_t = 4.0)]
    hours: f64,
    /// Results (from-<label>.txt), archive/ and harvested.txt go here.
    #[arg(long)]
    out_dir: PathBuf,
    #[command(flatten)]
    search: SearchArgs,
}

pub fn cmd_harvest(version: &str, args: HarvestArgs) -> Result<()> {
    let HarvestArgs { refs, fetch, max_blocks, top, within, minutes, every, hours, out_dir, search } = args;
    let t = std::time::Instant::now();
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let hopts = crate::harvest::HarvestOptions { refs, max_blocks, top, minutes, out_dir, moves: search.moves.clone(), within };
    loop {
        let pass = std::time::Instant::now();
        if fetch || every > 0.0 {
            let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
            if !st.success() {
                log("git fetch failed; harvesting what we have");
            }
        }
        // A fresh pool per pass: no idle JVMs held between passes.
        let pool = search.pool(version)?;
        crate::harvest::run(&pool, std::path::Path::new("."), &hopts, &search.options()?, &log)?;
        drop(pool);
        if every <= 0.0 || t.elapsed().as_secs_f64() > hours * 3600.0 {
            break;
        }
        let wait = (every * 60.0 - pass.elapsed().as_secs_f64()).max(0.0);
        log(&format!("pass done; next in {:.1} min", wait / 60.0));
        std::thread::sleep(std::time::Duration::from_secs_f64(wait));
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct TransplantArgs {
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    #[arg(long)]
    fetch: bool,
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    /// Targets per league.
    #[arg(long, default_value_t = 4)]
    top: usize,
    /// Targets within this many blocks of their league's best.
    #[arg(long, default_value_t = 4)]
    within: usize,
    /// Largest patch (cells changed) to learn.
    #[arg(long, default_value_t = 12)]
    max_cells: usize,
    /// Local search after each improvement, minutes.
    #[arg(long, default_value_t = 5.0)]
    minutes: f64,
    #[arg(long)]
    out_dir: PathBuf,
    #[command(flatten)]
    search: SearchArgs,
}

pub fn cmd_transplant(version: &str, args: TransplantArgs) -> Result<()> {
    let TransplantArgs { refs, fetch, max_blocks, top, within, max_cells, minutes, out_dir, search } = args;
    if fetch {
        let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
        anyhow::ensure!(st.success(), "git fetch failed");
    }
    let pool = search.pool(version)?;
    let t = std::time::Instant::now();
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let hopts = crate::harvest::HarvestOptions { refs, max_blocks, top, minutes, out_dir, moves: search.moves.clone(), within };
    crate::harvest::transplant(&pool, std::path::Path::new("."), &hopts, &search.options()?, max_cells, &log)?;
    Ok(())
}
