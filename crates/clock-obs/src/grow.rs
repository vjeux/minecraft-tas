//! Local search around known clocks: every seed clock with one block added (any palette state in
//! any empty cell of its box grown by --margin) or one block replaced (any other palette state),
//! simulated with the seed's kick (and, with --all-kicks, self-start and every block placed
//! last). New minimal clocks (no single block removable keeping the period) are reported per
//! period, one per type, smallest swept volume first. Candidates are generated per seed and
//! deduplicated by a 128-bit canonical digest (translation + rotation), so tens of thousands of
//! seeds fit in memory.

use crate::census::{canon_hash, fix_instruments, kick_label, padded, palette};
use crate::sim::{self, Kick, Spec, Verdict};
use anyhow::Result;
use clap::Args as ClapArgs;
use redsim::{Block, BlockPos};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(ClapArgs, Debug, Clone)]
pub struct GrowArgs {
    /// Seed clock designs (files, or folders of .txt designs)
    pub seeds: Vec<PathBuf>,
    #[arg(long, default_value = "obs,piston,sticky,note,lamp,bulb,stone,rblock")]
    pub palette: String,
    /// Moves: add (a block in an empty cell), replace (a block by another state)
    #[arg(long, default_value = "add,replace")]
    pub moves: String,
    /// Empty cells considered: the seed's box grown by this many cells
    #[arg(long, default_value_t = 1)]
    pub margin: i32,
    #[arg(long, default_value_t = 600)]
    pub horizon: u32,
    /// Also try self-start and every block placed last
    #[arg(long)]
    pub all_kicks: bool,
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    #[arg(long, default_value = "grow")]
    pub prefix: String,
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    /// Report only these periods (comma-separated); default all
    #[arg(long)]
    pub periods: Option<String>,
    /// Write at most this many designs per (period, family)
    #[arg(long, default_value_t = 1000000)]
    pub write_per_family: usize,
    /// Rounds: the new minimal clocks of a round are the seeds of the next (k, k+1, k+2 blocks...)
    #[arg(long, default_value_t = 1)]
    pub rounds: usize,
}

pub fn load_seeds(paths: &[PathBuf]) -> Result<Vec<Spec>> {
    let mut v = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut files: Vec<PathBuf> = std::fs::read_dir(p)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().is_some_and(|x| x == "txt")).collect();
            files.sort();
            for f in files {
                v.push(Spec::load(&f)?);
            }
        } else {
            v.push(Spec::load(p)?);
        }
    }
    Ok(v)
}

const SHARDS: usize = 64;

pub fn run(a: GrowArgs) -> Result<()> {
    let mut seeds = load_seeds(&a.seeds)?;
    let pal = palette(&a.palette)?;
    let add = a.moves.split(',').any(|m| m == "add");
    let replace = a.moves.split(',').any(|m| m == "replace");
    let want: Option<HashSet<u32>> = a.periods.as_ref().map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect());
    let seen: Vec<Mutex<HashSet<u128>>> = (0..SHARDS).map(|_| Mutex::new(HashSet::new())).collect();
    let mark = |h: u128| -> bool { seen[(h as usize) % SHARDS].lock().unwrap().insert(h) };
    for s in seeds.iter() {
        mark(canon_hash(&s.blocks));
    }
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    if let Some(d) = &a.out_dir {
        std::fs::create_dir_all(d)?;
    }
    for round in 1..=a.rounds {
    eprintln!("grow round {round}: {} seeds, {threads} threads", seeds.len());
    let next = AtomicUsize::new(0);
    let tried = AtomicUsize::new(0);
    let found: Mutex<HashMap<u128, (Vec<(BlockPos, Block)>, Vec<(Kick, u32, String, i64)>)>> = Mutex::new(HashMap::new());
    let start = std::time::Instant::now();
    let seeds_ref = &seeds;
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let seeds = seeds_ref;
                let si = next.fetch_add(1, Ordering::Relaxed);
                if si >= seeds.len() {
                    break;
                }
                if si % 500 == 0 && si > 0 {
                    let el = start.elapsed().as_secs_f64();
                    eprintln!("  seed {si}/{} ({el:.0}s, {} candidates, eta {:.0}s)", seeds.len(), tried.load(Ordering::Relaxed), el / si as f64 * (seeds.len() - si) as f64);
                }
                let s = &seeds[si];
                // this seed's candidate structures
                let mut cands: Vec<Vec<(BlockPos, Block)>> = Vec::new();
                let (lo, hi) = s.bounds();
                let occupied: HashSet<BlockPos> = s.blocks.iter().map(|(p, _)| *p).collect();
                if add {
                    for y in lo.y - a.margin..=hi.y + a.margin {
                        for z in lo.z - a.margin..=hi.z + a.margin {
                            for x in lo.x - a.margin..=hi.x + a.margin {
                                let p = BlockPos::new(x, y, z);
                                if occupied.contains(&p) {
                                    continue;
                                }
                                for b in &pal {
                                    let mut blocks = s.blocks.clone();
                                    blocks.push((p, *b));
                                    fix_instruments(&mut blocks);
                                    if mark(canon_hash(&blocks)) {
                                        cands.push(blocks);
                                    }
                                }
                            }
                        }
                    }
                }
                if replace {
                    for i in 0..s.blocks.len() {
                        for b in &pal {
                            if *b == s.blocks[i].1 {
                                continue;
                            }
                            let mut blocks = s.blocks.clone();
                            blocks[i].1 = *b;
                            fix_instruments(&mut blocks);
                            if mark(canon_hash(&blocks)) {
                                cands.push(blocks);
                            }
                        }
                    }
                }
                tried.fetch_add(cands.len(), Ordering::Relaxed);
                for blocks in &cands {
                    let mut kicks = vec![];
                    match &s.kick {
                        Kick::Last(p) if blocks.iter().any(|(q, _)| q == p) => kicks.push(s.kick.clone()),
                        Kick::Last(_) => {}
                        k => kicks.push(k.clone()),
                    }
                    if a.all_kicks {
                        if !kicks.contains(&Kick::None) {
                            kicks.push(Kick::None);
                        }
                        for (p, _) in blocks {
                            let k = Kick::Last(*p);
                            if !kicks.contains(&k) {
                                kicks.push(k);
                            }
                        }
                    }
                    for kick in kicks {
                        let spec = Spec { name: String::new(), blocks: blocks.clone(), kick: kick.clone(), output: None };
                        let r = sim::simulate(&spec, &sim::Opts { horizon: a.horizon, margin: 6, period_only: true });
                        if r.verdict != Verdict::Clock {
                            continue;
                        }
                        if padded(blocks, &kick, r.period, a.horizon) {
                            continue;
                        }
                        let full = sim::simulate(&spec, &sim::Opts { horizon: a.horizon, margin: 6, period_only: false });
                        let key = canon_hash(blocks);
                        let mut f = found.lock().unwrap();
                        let e = f.entry(key).or_insert_with(|| (blocks.clone(), vec![]));
                        if e.0 == *blocks && !e.1.iter().any(|(_, p, _, _)| *p == full.period) {
                            e.1.push((kick, full.period, full.family_name(), full.volume()));
                        }
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    eprintln!("grow round {round} done in {:.1}s: {} candidates, {} new minimal clock structures", start.elapsed().as_secs_f64(), tried.load(Ordering::Relaxed), found.len());
    let mut by_period: BTreeMap<u32, Vec<(i64, String, Vec<(BlockPos, Block)>, Kick)>> = BTreeMap::new();
    let mut next_seeds: Vec<Spec> = Vec::new();
    for (_, (blocks, runs)) in found {
        if let Some((k, _, _, _)) = runs.first() {
            next_seeds.push(Spec { name: String::new(), blocks: blocks.clone(), kick: k.clone(), output: None });
        }
        for (kick, p, fam, vol) in runs {
            if want.as_ref().is_some_and(|w| !w.contains(&p)) {
                continue;
            }
            by_period.entry(p).or_default().push((vol, fam, blocks.clone(), kick));
        }
    }
    println!("round {round}:");
    println!("| period | new minimal types | blocks (min) | families |");
    println!("|---|---|---|---|");
    for (p, v) in by_period.iter_mut() {
        v.sort_by_key(|e| (e.2.len(), e.0, e.1.clone()));
        let mut fams: BTreeMap<String, usize> = BTreeMap::new();
        for e in v.iter() {
            *fams.entry(e.1.clone()).or_default() += 1;
        }
        let fs: Vec<String> = fams.iter().map(|(f, c)| format!("{f} x{c}")).collect();
        println!("| {p} | {} | {} | {} |", v.len(), v[0].2.len(), fs.join(", "));
        if let Some(d) = &a.out_dir {
            let mut written: HashMap<String, usize> = HashMap::new();
            for (i, e) in v.iter().enumerate() {
                let w = written.entry(e.1.clone()).or_default();
                if *w >= a.write_per_family {
                    continue;
                }
                *w += 1;
                let name = format!("{}-r{round}-p{p}-{}b-{i:04}", a.prefix, e.2.len());
                let spec = Spec { name: name.clone(), blocks: e.2.clone(), kick: e.3.clone(), output: None };
                let text = spec.to_text(&format!("clk-obs grow: P={p} (redsim), family {}, kick {}, swept volume {}.", e.1, kick_label(&e.3), e.0));
                std::fs::write(d.join(format!("{name}.txt")), text)?;
            }
        }
    }
    next_seeds.sort_by_key(|s| canon_hash(&s.blocks));
    seeds = next_seeds;
    }
    Ok(())
}
