//! Local search around known clocks: every seed clock with one block added (any palette state in
//! any empty cell of its box grown by --margin) or one block replaced (any other palette state),
//! simulated with the seed's kick (and, with --all-kicks, self-start and every block placed
//! last). New minimal clocks (no single block removable keeping the period) are reported per
//! period, one per type, smallest swept volume first.

use crate::census::{canon_structure, fix_instruments, kick_label, padded, palette};
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
}

fn load_seeds(paths: &[PathBuf]) -> Result<Vec<Spec>> {
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

pub fn run(a: GrowArgs) -> Result<()> {
    let seeds = load_seeds(&a.seeds)?;
    let pal = palette(&a.palette)?;
    let add = a.moves.split(',').any(|m| m == "add");
    let replace = a.moves.split(',').any(|m| m == "replace");
    let want: Option<HashSet<u32>> = a.periods.as_ref().map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect());
    // candidate structures: (seed index, blocks)
    let mut cands: Vec<(usize, Vec<(BlockPos, Block)>)> = Vec::new();
    let mut seen: HashSet<Vec<([i32; 3], String)>> = HashSet::new();
    for s in seeds.iter() {
        seen.insert(canon_structure(&s.blocks));
    }
    for (si, s) in seeds.iter().enumerate() {
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
                            if seen.insert(canon_structure(&blocks)) {
                                cands.push((si, blocks));
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
                    if seen.insert(canon_structure(&blocks)) {
                        cands.push((si, blocks));
                    }
                }
            }
        }
    }
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    eprintln!("grow: {} seeds, {} candidate structures, {threads} threads", seeds.len(), cands.len());
    let next = AtomicUsize::new(0);
    let found: Mutex<HashMap<Vec<([i32; 3], String)>, (Vec<(BlockPos, Block)>, Vec<(Kick, u32, String, i64)>)>> = Mutex::new(HashMap::new());
    let start = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= cands.len() {
                    break;
                }
                if i % 20000 == 0 && i > 0 {
                    let el = start.elapsed().as_secs_f64();
                    eprintln!("  {i}/{} ({el:.0}s, eta {:.0}s)", cands.len(), el / i as f64 * (cands.len() - i) as f64);
                }
                let (si, blocks) = &cands[i];
                let seed = &seeds[*si];
                let mut kicks = vec![];
                match &seed.kick {
                    Kick::Last(p) if blocks.iter().any(|(q, _)| q == p) => kicks.push(seed.kick.clone()),
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
                    if want.as_ref().is_some_and(|w| !w.contains(&r.period)) {
                        continue;
                    }
                    if padded(blocks, &kick, r.period, a.horizon) {
                        continue;
                    }
                    let full = sim::simulate(&spec, &sim::Opts { horizon: a.horizon, margin: 6, period_only: false });
                    let key = canon_structure(blocks);
                    let mut f = found.lock().unwrap();
                    let e = f.entry(key).or_insert_with(|| (blocks.clone(), vec![]));
                    if e.0 == *blocks && !e.1.iter().any(|(_, p, _, _)| *p == full.period) {
                        e.1.push((kick, full.period, full.family_name(), full.volume()));
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    eprintln!("grow done in {:.1}s: {} new minimal clock structures", start.elapsed().as_secs_f64(), found.len());
    let mut by_period: BTreeMap<u32, Vec<(i64, String, Vec<(BlockPos, Block)>, Kick)>> = BTreeMap::new();
    for (_, (blocks, runs)) in found {
        for (kick, p, fam, vol) in runs {
            by_period.entry(p).or_default().push((vol, fam, blocks.clone(), kick));
        }
    }
    if let Some(d) = &a.out_dir {
        std::fs::create_dir_all(d)?;
    }
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
            for (i, e) in v.iter().enumerate() {
                let name = format!("{}-p{p}-{}b-{i:04}", a.prefix, e.2.len());
                let spec = Spec { name: name.clone(), blocks: e.2.clone(), kick: e.3.clone(), output: e.2.iter().find(|(_, b)| matches!(b, Block::Observer { .. })).map(|(q, _)| *q) };
                let text = spec.to_text(&format!("clk-obs grow: P={p} (redsim), family {}, kick {}, swept volume {}.", e.1, kick_label(&e.3), e.0));
                std::fs::write(d.join(format!("{name}.txt")), text)?;
            }
        }
    }
    Ok(())
}
