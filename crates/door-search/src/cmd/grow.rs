//! `door regrow`, `door grow`: growing designs.

use anyhow::Result;
use mctas::design::Design;
use piston_door::cli::{SearchArgs, load_design};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct RegrowArgs {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Group size: 2 (re-grown with 1 block) or 3 (with 2).
    #[arg(long, default_value_t = 2)]
    size: usize,
    #[arg(long, default_value_t = 1)]
    margin: i32,
    /// Block states to re-grow with (see search.rs `grow_states`; `trapdoor`: the watchable
    /// relay that shortened six doors on the exotic track).
    #[arg(long, default_value = "obs,block,dust,rep,rep2,rep3,rep4,torch,glass,trapdoor")]
    states: String,
    /// Only cut groups inside this box, "x0,y0,z0:x1,y1,z1".
    #[arg(long)]
    only: Option<String>,
    #[arg(long, default_value_t = 1)]
    want: usize,
    /// Cut any group of --size control blocks, connected or not (new blocks go within
    /// --margin of any cut block).
    #[arg(long)]
    any: bool,
    /// Keep only candidates that add at least one block whose state contains one of these
    /// (comma list; shorthands as `resynth --require-any`, plus trapdoor, gate, rail, disp,
    /// crafter, skull, shelf, scaffold, grate, tinted, slab, stairs): sweep new states on a
    /// design the old ones already exhausted.
    #[arg(long, default_value = "")]
    require_any: String,
    #[command(flatten)]
    search: SearchArgs,
}

pub fn cmd_regrow(version: &str, args: RegrowArgs) -> Result<()> {
    let RegrowArgs { design, out, size, margin, states, only, want, any, require_any, search } = args;
    let d = load_design(&design)?;
    let pos = |s: &str| -> Result<mctas::sim::Pos> {
        let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "expected x,y,z, got {s:?}");
        Ok([v[0], v[1], v[2]])
    };
    let only = match only {
        Some(s) => {
            let (a, b) = s.split_once(':').ok_or_else(|| anyhow::anyhow!("--only wants x0,y0,z0:x1,y1,z1"))?;
            let (a, b) = (pos(a)?, pos(b)?);
            Some(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
        }
        None => None,
    };
    let pool = search.pool(version)?;
    let t = std::time::Instant::now();
    let opts = search.options()?;
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let st = crate::search::grow_states(&states)?;
    let req = crate::synth::require_any_names(&require_any);
    let found = crate::search::regrow(&pool, &d, size, margin, &st, only, &opts, want, any, &req, &log)?;
    for (fd, desc) in &found {
        println!("WORKS: {} blocks: {desc}", crate::block_count(fd));
    }
    match found.first() {
        Some((best, _)) => {
            let mut w = pool.worker(0);
            let canon = crate::canonicalize(&mut w, best)?;
            canon.save(&out)?;
            println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&canon), out.display());
        }
        None => println!("nothing found"),
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct GrowArgs {
    design: PathBuf,
    #[arg(long)]
    out: PathBuf,
    /// Box corner "x,y,z" (inclusive).
    #[arg(long)]
    lo: String,
    /// Opposite box corner "x,y,z" (inclusive).
    #[arg(long)]
    hi: String,
    /// Comma list: obs, block, dust, rep[1-4], torch, glass, obsidian, rblock, lamp, note, leaves
    /// (persistent), log (3 axes), or full states without commas.
    #[arg(long, default_value = "obs,block,dust")]
    states: String,
    /// Blocks added together (1 or 2; 2 also tries singles).
    #[arg(long, default_value_t = 2)]
    k: usize,
    /// Stop after this many verified designs (0 = all).
    #[arg(long, default_value_t = 1)]
    want: usize,
    /// Seed-and-exploit: run the local search (--moves, --plateau...) from EVERY verified
    /// grown design and keep the best result below the start design's block count (a block
    /// that changes the machine, e.g. an observer riding a slime structure, can make others
    /// redundant). Use with --want 0.
    #[arg(long)]
    then_search: bool,
    /// With --then-search: minutes of local search per seed (0 = until it stops improving).
    #[arg(long, default_value_t = 0.0)]
    seed_minutes: f64,
    /// With --then-search: start at this seed (1-based; skip the ones before).
    #[arg(long, default_value_t = 1)]
    seed_from: usize,
    /// Riders only: every added block must stick to a slime/honey structure (directly, or with
    /// --k 2 through an added slime/honey stuck to one).
    #[arg(long)]
    glue: bool,
    /// With --then-search: skip seeds whose open/close ticks (screen positions) equal the
    /// start design's, i.e. blocks that most likely change nothing.
    #[arg(long)]
    changed: bool,
    #[command(flatten)]
    search: SearchArgs,
}

pub fn cmd_grow(version: &str, args: GrowArgs) -> Result<()> {
    let GrowArgs { design, out, lo, hi, states, k, want, then_search, seed_minutes, seed_from, glue, changed, search } = args;
    let d = load_design(&design)?;
    let pos = |s: &str| -> Result<mctas::sim::Pos> {
        let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
        anyhow::ensure!(v.len() == 3, "expected x,y,z, got {s:?}");
        Ok([v[0], v[1], v[2]])
    };
    let (a, b) = (pos(&lo)?, pos(&hi)?);
    let g = crate::search::GrowOptions {
        lo: [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])],
        hi: [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])],
        states: crate::search::grow_states(&states)?,
        k,
        want,
        glue,
    };
    let pool = search.pool(version)?;
    let t = std::time::Instant::now();
    let opts = search.options()?;
    let log = |m: &str| println!("[{:7.1}s] {m}", t.elapsed().as_secs_f64());
    let found = crate::search::grow(&pool, &d, &g, &opts, &log)?;
    for (fd, desc) in &found {
        println!("WORKS: {} blocks: {desc}", crate::block_count(fd));
    }
    if then_search {
        let base_n = crate::block_count(&d);
        let mut best: Option<(Design, String)> = None;
        let screen = opts.screen.clone();
        // Timing signature: every cycle's open/close ticks AND settle ticks (a rider that
        // fires extra often only shows in the settle time).
        let sig = |r: &crate::Report| -> Vec<u32> {
            r.cycles.iter().flat_map(|c| [c.open_ticks, c.open_settle, c.close_ticks, c.close_settle]).collect()
        };
        let base_ticks = if changed {
            let mut w = pool.worker(0);
            Some(sig(&crate::verify(&mut w, &d, &screen)?))
        } else {
            None
        };
        // Seeds whose screen timings differ from the start's (all seeds without --changed).
        let keep: Vec<bool> = match base_ticks {
            Some(bt) => {
                let items: Vec<Design> = found.iter().map(|(fd, _)| fd.clone()).collect();
                let screen = &screen;
                let bt = &bt;
                pool.map(items, |w, _, fd| Ok(crate::verify(w, &fd, screen).map(|r| sig(&r) != *bt).unwrap_or(true)))
                    .into_iter()
                    .map(|r| r.unwrap_or(true))
                    .collect()
            }
            None => vec![true; found.len()],
        };
        log(&format!("{} of {} seeds kept", keep.iter().filter(|k| **k).count(), found.len()));
        for (i, (fd, desc)) in found.iter().enumerate() {
            if i + 1 < seed_from || !keep[i] {
                continue;
            }
            log(&format!("seed {}/{}: {desc}", i + 1, found.len()));
            let mut sopts = search.options()?;
            if seed_minutes > 0.0 {
                sopts.deadline = Some(std::time::Instant::now() + std::time::Duration::from_secs_f64(seed_minutes * 60.0));
            }
            let quiet = |_: &str| {};
            let mut on_improve = |nd: &Design, m: &str| -> Result<()> {
                log(&format!("    {} blocks: {m}", crate::block_count(nd)));
                Ok(())
            };
            let res = match crate::search::search(&pool, fd, &sopts, &quiet, &mut on_improve) {
                Ok(r) => r,
                Err(e) => {
                    log(&format!("    search failed: {e:#}"));
                    continue;
                }
            };
            let n = crate::block_count(&res);
            if n < base_n && best.as_ref().is_none_or(|(b, _)| n < crate::block_count(b)) {
                let mut w = pool.worker(0);
                let canon = crate::canonicalize(&mut w, &res)?;
                canon.save(&out)?;
                log(&format!("  BELOW START: {n} blocks from seed {desc}, written to {}", out.display()));
                best = Some((res, desc.clone()));
            }
        }
        if best.is_none() {
            println!("no seed searched below {base_n} blocks");
        }
        return Ok(());
    }
    match found.first() {
        Some((best, _)) => {
            let mut w = pool.worker(0);
            let canon = crate::canonicalize(&mut w, best)?;
            canon.save(&out)?;
            println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&canon), out.display());
        }
        None => println!("nothing found"),
    }
    Ok(())
}
