//! `mctas door harvest`: run the local search on the best designs of every branch.
//!
//! Gathers designs like `door leaderboard` (every `.txt` under designs/piston-door-3x3 on HEAD
//! and on the `explore/*` branches), judges them, and for each category takes the `top` smallest
//! working designs that were not harvested yet (by block hash, listed in
//! `<out-dir>/harvested.txt`). Each gets [`crate::search::search`] for `minutes`; the result goes
//! to `<out-dir>/from-<label>.txt` and every improvement is archived in `<out-dir>/archive/`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use mctas::design::Design;
use mctas::sim::Pool;

use crate::search::{SearchOptions, design_hash, search};
use crate::{block_count, category, verify};

pub struct HarvestOptions {
    pub refs: String,
    pub max_blocks: usize,
    pub top: usize,
    pub minutes: f64,
    pub out_dir: PathBuf,
    pub moves: String,
    /// Only designs within this many blocks of their league's best.
    pub within: usize,
}

/// `origin/explore/center-mech:designs/.../tracks/center-mech/BEST.txt` -> `center-mech-BEST`.
fn label(source: &str) -> String {
    let (r, f) = source.split_once(':').unwrap_or(("", source));
    let branch = r.rsplit('/').next().unwrap_or(r);
    let stem = Path::new(f).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    format!("{branch}-{stem}").replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "-")
}

pub fn run(pool: &Pool, repo: &Path, opts: &HarvestOptions, search_opts: &SearchOptions, log: &dyn Fn(&str)) -> Result<()> {
    let cands = crate::leaderboard::collect(repo, &opts.refs, opts.max_blocks)?;
    std::fs::create_dir_all(&opts.out_dir)?;
    let seen_path = opts.out_dir.join("harvested.txt");
    let seen_text = std::fs::read_to_string(&seen_path).unwrap_or_default();
    let seen: HashSet<u64> = seen_text.lines().filter_map(|l| u64::from_str_radix(l.split_whitespace().next()?, 16).ok()).collect();
    let mine = opts.out_dir.to_string_lossy().to_string();
    log(&format!("{} designs; judging them", cands.len()));
    let verdicts = pool.map(cands.clone(), |w, _, (_, d)| Ok(verify(w, &d, &search_opts.judge).is_ok()));
    let all_working: Vec<(String, Design)> =
        cands.into_iter().zip(verdicts).filter(|(_, v)| matches!(v, Ok(true))).map(|(c, _)| c).collect();
    let all_best: Vec<(Design, usize)> = all_working.iter().map(|(_, d)| (d.clone(), block_count(d))).collect();
    // Our own results (named search-tool-*, or under the out dir) are local minima already.
    let mut working: Vec<(String, Design)> = all_working
        .into_iter()
        .filter(|(s, d)| !seen.contains(&design_hash(d)) && !s.contains(mine.as_str()) && !d.name.starts_with("search-tool"))
        .collect();
    log(&format!("{} working designs not harvested yet", working.len()));
    working.sort_by_key(|(s, d)| (category(d), block_count(d), s.clone()));
    let mut picked: Vec<(String, Design)> = Vec::new();
    let mut cats: Vec<&'static str> = working.iter().map(|(_, d)| category(d)).collect();
    cats.dedup();
    // Only designs that could plausibly beat their league: within `within` blocks of its best
    // (judged over every design, harvested or not).
    let best_of = |f: &dyn Fn(&Design) -> bool| all_best.iter().filter(|(d, _)| f(d)).map(|(_, b)| *b).min().unwrap_or(usize::MAX);
    for cat in cats {
        let best = best_of(&|d: &Design| category(d) == cat);
        picked.extend(
            working.iter().filter(|(_, d)| category(d) == cat && block_count(d) <= best.saturating_add(opts.within)).take(opts.top).cloned(),
        );
    }
    // The slime-free league too.
    let best = best_of(&|d: &Design| crate::slime_free(d) && category(d) == "solid");
    for c in working.iter().filter(|(_, d)| crate::slime_free(d) && block_count(d) <= best.saturating_add(opts.within)).take(opts.top) {
        if !picked.iter().any(|(s, _)| *s == c.0) {
            picked.push(c.clone());
        }
    }
    for (source, d0) in picked {
        let name = label(&source);
        let out = opts.out_dir.join(format!("from-{name}.txt"));
        let mut d = d0.clone();
        d.name = format!("search-tool-from-{name}");
        log(&format!("== {source}: {} blocks, {}", block_count(&d), category(&d)));
        let mut o = SearchOptions { deadline: Some(Instant::now() + Duration::from_secs_f64(opts.minutes * 60.0)), ..search_opts.clone() };
        o.seed = search_opts.seed;
        let judge_n = o.judge.offsets.len();
        let moves = opts.moves.clone();
        let mut on_improve = |best: &Design, desc: &str| -> Result<()> {
            best.save(&out)?;
            log(&format!("improved: {} blocks ({desc}), written to {}", block_count(best), out.display()));
            let note = format!(
                "verified WORKS: {} blocks (judge: {judge_n} positions x 3 cycles); door harvest (minimize --moves {moves}) from {source}",
                block_count(best)
            );
            if let Some(p) = crate::archive::save(&out, best, block_count(best), &note, None)? {
                log(&format!("archived {}", p.display()));
            }
            Ok(())
        };
        let best = search(pool, &d, &o, log, &mut on_improve)?;
        // The result is a local minimum too: never harvest it again.
        let mut line = format!("{:016x} {source} {} -> {}\n", design_hash(&d0), block_count(&d0), block_count(&best));
        if best.blocks != d0.blocks {
            line.push_str(&format!("{:016x} result of {source} {}\n", design_hash(&best), block_count(&best)));
        }
        let mut text = std::fs::read_to_string(&seen_path).unwrap_or_default();
        text.push_str(&line);
        std::fs::write(&seen_path, text)?;
        log(&format!("== {source}: {} -> {} blocks", block_count(&d0), block_count(&best)));
    }
    Ok(())
}
