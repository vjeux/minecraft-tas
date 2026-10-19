//! `door catalog`, `door frontier`, `door shapes`, `door leaderboard`: the boards and the archive
//! files they generate.

use anyhow::Result;
use piston_door::cli::load_design;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct CatalogArgs {
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    archive: PathBuf,
    /// A local folder of rendered videos (`<stem>.mp4`, video-all's --out): linked too.
    #[arg(long)]
    videos: Option<PathBuf>,
    /// GitHub release whose `<stem>.mp4` assets are linked ("" = none).
    #[arg(long, default_value = "videos")]
    release: String,
    /// Worker JVMs for archived designs missing from the verdict cache.
    #[arg(long, default_value_t = 8)]
    workers: usize,
}

pub fn cmd_catalog(version: &str, args: CatalogArgs) -> Result<()> {
    let CatalogArgs { archive, videos, release, workers } = args;
    let o = crate::catalog::CatalogOptions { archive: &archive, workers, videos: videos.as_deref(), release: Some(release.as_str()) };
    crate::catalog::write(version, &o)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct FrontierArgs {
    /// Candidate designs to place against the frontier.
    #[arg(long, num_args = 1..)]
    check: Vec<PathBuf>,
    /// Ref prefix to scan besides HEAD.
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    #[arg(long, default_value_t = 4)]
    workers: usize,
    /// Verdict cache folder (read; new verdicts are written back).
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    cache_dir: PathBuf,
    /// All categories (default: solid only, the main Pareto board).
    #[arg(long)]
    any_category: bool,
    /// Also list every working design with its ticks, not just the frontier.
    #[arg(long)]
    all: bool,
}

pub fn cmd_frontier(version: &str, args: FrontierArgs) -> Result<()> {
    let FrontierArgs { check, refs, extra_offsets, max_blocks, workers, cache_dir, any_category, all } = args;
    let repo = std::path::Path::new(".");
    let mut cands = crate::leaderboard::collect(repo, &refs, max_blocks)?;
    let n_board = cands.len();
    for p in &check {
        cands.push((format!("check:{}", p.display()), load_design(p)?));
    }
    let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, Some(&cache_dir))?;
    let (board, checks): (Vec<_>, Vec<_>) = entries.into_iter().partition(|e| !e.source.starts_with("check:"));
    let _ = n_board;
    print!("{}", crate::leaderboard::frontier_report(&board, &checks, !any_category, all));
    Ok(())
}

#[derive(clap::Args)]
pub struct ShapesArgs {
    /// Design files to measure besides the folder.
    designs: Vec<PathBuf>,
    /// Folder of designs to measure (default: the archive).
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    dir: PathBuf,
    /// Also measure every design on HEAD and the explore/* branches (as the leaderboard does).
    #[arg(long)]
    branches: bool,
    /// Ref prefix scanned with --branches besides HEAD.
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    #[arg(long, default_value_t = 4)]
    workers: usize,
    /// Verdict cache folder (read; new verdicts are written back).
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    cache_dir: PathBuf,
    /// Extra shape rules: a preset or a comma list of key<=n (front behind thick below above
    /// left right side volume).
    #[arg(long)]
    rule: Vec<String>,
    /// The table lists designs of at most this many blocks (the rules still see all).
    #[arg(long, default_value_t = 200)]
    table_max: usize,
    /// Skip the table (rules and frontier only).
    #[arg(long)]
    no_table: bool,
}

pub fn cmd_shapes(version: &str, args: ShapesArgs) -> Result<()> {
    let ShapesArgs { designs, dir, branches, refs, extra_offsets, max_blocks, workers, cache_dir, rule, table_max, no_table } = args;
    let repo = std::path::Path::new(".");
    let mut cands = crate::shapes::collect_dir(&dir, max_blocks)?;
    if branches {
        cands.extend(crate::leaderboard::collect(repo, &refs, max_blocks)?);
    }
    for p in &designs {
        cands.push((p.display().to_string(), load_design(p)?));
    }
    let mut seen = std::collections::HashSet::new();
    cands.retain(|(_, d)| seen.insert(crate::leaderboard::design_hash(d)));
    let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, Some(&cache_dir))?;
    let rules = crate::shapes::rules(&rule)?;
    if !no_table {
        print!("{}", crate::shapes::table(&entries, &rules, table_max));
    }
    print!("{}", crate::shapes::best_report(&entries, &rules));
    Ok(())
}

#[derive(clap::Args)]
pub struct LeaderboardArgs {
    /// Ref prefix to scan besides HEAD.
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    /// Run `git fetch origin` first.
    #[arg(long)]
    fetch: bool,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    /// Skip designs with more blocks than this.
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// Write the markdown table here (also printed).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Copy every explore branch's tracks/<track> folder into the working tree first.
    #[arg(long)]
    sync_tracks: bool,
    /// Archive every working design into this folder; designs already there are trusted.
    #[arg(long)]
    archive_dir: Option<PathBuf>,
    /// Verdict cache folder (default: --archive-dir; none = verify everything).
    #[arg(long)]
    cache_dir: Option<PathBuf>,
    /// Rank only designs whose verdict is cached (simulate nothing).
    #[arg(long)]
    cached_only: bool,
    /// GitHub release with the GIFs, videos and montages to show ("" = none).
    #[arg(long, default_value = "videos")]
    release: String,
    /// Archive folder whose captions.tsv / records.tsv describe the leaders.
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    captions: PathBuf,
}

pub fn cmd_leaderboard(version: &str, args: LeaderboardArgs) -> Result<()> {
    let LeaderboardArgs { refs, fetch, extra_offsets, max_blocks, workers, out, sync_tracks, archive_dir, cache_dir, cached_only, release, captions } = args;
    let repo = std::path::Path::new(".");
    if fetch {
        let st = std::process::Command::new("git").args(["fetch", "-q", "origin"]).status()?;
        anyhow::ensure!(st.success(), "git fetch failed");
    }
    if sync_tracks {
        eprintln!("synced tracks: {:?}", crate::leaderboard::sync_tracks(repo, &refs)?);
    }
    let trusted = archive_dir.as_deref().map(crate::leaderboard::archived_keys).unwrap_or_default();
    let mut cands = crate::leaderboard::collect(repo, &refs, max_blocks)?;
    let cache = cache_dir.or_else(|| archive_dir.clone());
    let mut extras = crate::leaderboard::Extras::load(&captions, &release);
    if cached_only {
        let dir = cache.as_deref().ok_or_else(|| anyhow::anyhow!("--cached-only needs --cache-dir or --archive-dir"))?;
        extras.uncached = crate::leaderboard::only_cached(&mut cands, dir);
        eprintln!("{} designs without a cached verdict left out", extras.uncached);
    }
    eprintln!("verifying {} designs ({} archived designs trusted)", cands.len(), trusted.len());
    let entries = crate::leaderboard::judge(version, cands, extra_offsets, workers, cache.as_deref())?;
    if let Some(dir) = &archive_dir {
        for e in entries.iter().filter(|e| e.verdict.is_ok()) {
            let note = format!("verified WORKS by door leaderboard: {} blocks, category {}", e.blocks, e.category);
            if let Some(p) = crate::archive::save(std::path::Path::new(&e.source), &e.design, e.blocks, &note, Some(dir))? {
                eprintln!("archived {}", p.display());
            }
        }
    }
    let table = crate::leaderboard::render(&entries, extra_offsets, &extras);
    print!("{table}");
    if let Some(p) = out {
        std::fs::write(p, &table)?;
    }
    Ok(())
}
