//! Cores: what a clock IS in its steady state, apart from how it was started. From the state at
//! the warmup, every block that never changes during a period is tried away (removed raw, no
//! updates); it belongs to the core only if the rest then behaves differently over the next
//! periods. The core = the changing cells plus the static blocks they need. Two clocks with the
//! same core (up to translation, rotation and phase) are the same machine with different
//! starters or leftovers; `cores` groups search outputs by core and reports per period the
//! structures found and the distinct cores, with the smallest design of each core.

use crate::census::canon_hash;
use crate::sim::{self, Spec, Verdict};
use anyhow::Result;
use clap::Args as ClapArgs;
use redsim::{Block, BlockPos, World};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct CoresArgs {
    /// Folders or files of clock designs
    pub inputs: Vec<PathBuf>,
    /// Write the smallest design of every core into this folder
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    /// Print every core (not only the per-period counts)
    #[arg(long)]
    pub list: bool,
}

/// Every non-air cell of the world box with its state (moving blocks as `moving_piston`: the
/// key includes the moving entities through the evolution test, not here).
fn frame(w: &World) -> Vec<(BlockPos, Block)> {
    let mut v = Vec::new();
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                let p = w.min.offset(x, y, z);
                let b = w.get(p);
                if !b.is_air() {
                    v.push((p, b));
                }
            }
        }
    }
    v
}

/// The core of a clock: (core key, core block count, period), or None if it is not a clock.
pub fn core_of(spec: &Spec) -> Option<(u128, usize, u32)> {
    let r = sim::simulate(spec, &sim::Opts { horizon: 20000, margin: 6, period_only: true });
    if r.verdict != Verdict::Clock {
        return None;
    }
    let (p, wu) = (r.period, r.warmup);
    let mut w = sim::build(spec, 6);
    for _ in 0..wu {
        w.tick();
    }
    // the reference evolution: frames over 3 periods
    let run = |mut w: World| -> Vec<Vec<(BlockPos, Block)>> {
        let mut out = Vec::with_capacity(3 * p as usize);
        for _ in 0..3 * p {
            w.tick();
            out.push(frame(&w));
        }
        out
    };
    let reference = run(w.clone());
    let f0 = frame(&w);
    let mut changing: Vec<BlockPos> = Vec::new();
    for (pos, b) in &f0 {
        if reference.iter().any(|f| f.iter().find(|(q, _)| q == pos).map(|(_, s)| s) != Some(b)) {
            changing.push(*pos);
        }
    }
    for f in &reference {
        for (pos, _) in f {
            if !f0.iter().any(|(q, _)| q == pos) && !changing.contains(pos) {
                changing.push(*pos);
            }
        }
    }
    // the reference restricted to the changing cells
    let restrict = |frames: &[Vec<(BlockPos, Block)>], cells: &[BlockPos]| -> Vec<Vec<Block>> {
        frames.iter().map(|f| cells.iter().map(|c| f.iter().find(|(q, _)| q == c).map(|(_, b)| *b).unwrap_or(Block::Air)).collect()).collect()
    };
    let target = restrict(&reference, &changing);
    let statics: Vec<BlockPos> = f0.iter().map(|(q, _)| *q).filter(|q| !changing.contains(q)).collect();
    let mut removed: Vec<BlockPos> = Vec::new();
    for s in &statics {
        let mut t = w.clone();
        for q in removed.iter().chain(std::iter::once(s)) {
            t.put(*q, Block::Air);
        }
        if restrict(&run(t), &changing) == target {
            removed.push(*s);
        }
    }
    // the core's blocks at each phase; the key is the smallest canonical digest over the phases
    let mut key = u128::MAX;
    let mut size = 0;
    let mut t = w.clone();
    for q in &removed {
        t.put(*q, Block::Air);
    }
    for _ in 0..p {
        let f: Vec<(BlockPos, Block)> = frame(&t);
        size = size.max(f.len());
        key = key.min(canon_hash(&f));
        t.tick();
    }
    Some((key, statics.len() - removed.len() + changing.len(), p))
}

pub fn run(a: CoresArgs) -> Result<()> {
    let files = crate::pick::files_of(&a.inputs)?;
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let rows: std::sync::Mutex<Vec<(u32, u128, usize, usize, PathBuf, String)>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= files.len() {
                    break;
                }
                let Ok(spec) = Spec::load(&files[i]) else { continue };
                let Some((key, core_size, p)) = core_of(&spec) else { continue };
                let fam = sim::simulate(&spec, &sim::Opts { horizon: 20000, margin: 6, period_only: false }).family_name();
                rows.lock().unwrap().push((p, key, core_size, spec.blocks.len(), files[i].clone(), fam));
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|x, y| (x.0, x.1, x.3, &x.4).cmp(&(y.0, y.1, y.3, &y.4)));
    let mut by_period: BTreeMap<u32, (usize, HashMap<u128, Vec<usize>>)> = BTreeMap::new();
    for (i, r) in rows.iter().enumerate() {
        let e = by_period.entry(r.0).or_default();
        e.0 += 1;
        e.1.entry(r.1).or_default().push(i);
    }
    if let Some(d) = &a.out_dir {
        std::fs::create_dir_all(d)?;
    }
    println!("| period | structures | distinct cores | core sizes (blocks: count) |");
    println!("|---|---|---|---|");
    for (p, (n, cores)) in &by_period {
        let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
        for idx in cores.values() {
            *sizes.entry(rows[idx[0]].2).or_default() += 1;
        }
        let s: Vec<String> = sizes.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        println!("| {p} | {n} | {} | {} |", cores.len(), s.join(", "));
        let mut list: Vec<&Vec<usize>> = cores.values().collect();
        list.sort_by_key(|idx| (rows[idx[0]].2, rows[idx[0]].3));
        for (ci, idx) in list.iter().enumerate() {
            let best = idx.iter().min_by_key(|&&i| (rows[i].3, rows[i].4.clone())).unwrap();
            let r = &rows[*best];
            if a.list {
                println!("    core {ci}: {} core blocks, {} structures, smallest {} blocks {} ({})", r.2, idx.len(), r.3, r.4.display(), r.5);
            }
            if let Some(d) = &a.out_dir {
                let name = format!("p{p}-core{ci:03}-{}b", r.3);
                std::fs::copy(&r.4, d.join(format!("{name}.txt")))?;
            }
        }
    }
    Ok(())
}
