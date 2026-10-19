//! Exhaustive census of small block-update clocks: every assignment of palette states to the
//! cells of every polycube of k cells (face-connected; one shape per class of the 4 horizontal
//! rotations), each tried with every kick (self-start, each cell placed last), simulated on
//! redsim. A clock is reported once per type (structure up to translation and rotation), with
//! every kick and period it shows, and flagged MINIMAL when no single block can be removed
//! keeping a clock of the same period.

use crate::sim::{self, Kick, Spec, Verdict};
use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use redsim::block::Instrument;
use redsim::{Block, BlockPos, Dir};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(ClapArgs, Debug, Clone)]
pub struct CensusArgs {
    /// Blocks per structure
    #[arg(long, default_value_t = 3)]
    pub k: usize,
    /// Palette shorthands, comma-separated: obs, piston, sticky, note, lamp, bulb, stone, rblock,
    /// glass, slime, honey, trap (iron trapdoors, 8 states), obsidian
    #[arg(long, default_value = "obs,piston,sticky,note,lamp,bulb,stone,rblock")]
    pub palette: String,
    /// Kick forms to try: self (no kick), last (each cell placed last)
    #[arg(long, default_value = "self,last")]
    pub kicks: String,
    /// Ticks simulated per candidate
    #[arg(long, default_value_t = 400)]
    pub horizon: u32,
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    /// Write one design per minimal type into this folder
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// Name prefix of written designs
    #[arg(long, default_value = "census")]
    pub prefix: String,
    /// Keep only structures that contain every one of these block names (e.g. lamp)
    #[arg(long)]
    pub require: Option<String>,
    /// Shapes: one polycube class only, as `x,y,z;x,y,z;...` (for a targeted run)
    #[arg(long)]
    pub shape: Option<String>,
}

type P3 = [i32; 3];

pub fn rot(p: P3) -> P3 {
    [-p[2], p[1], p[0]]
}

fn normalize(mut cells: Vec<P3>) -> Vec<P3> {
    let mut lo = [i32::MAX; 3];
    for c in &cells {
        for i in 0..3 {
            lo[i] = lo[i].min(c[i]);
        }
    }
    for c in cells.iter_mut() {
        for i in 0..3 {
            c[i] -= lo[i];
        }
    }
    cells.sort();
    cells
}

fn canon_shape(cells: &[P3]) -> Vec<P3> {
    let mut best = normalize(cells.to_vec());
    let mut cur = cells.to_vec();
    for _ in 0..3 {
        cur = cur.iter().map(|c| rot(*c)).collect();
        let n = normalize(cur.clone());
        if n < best {
            best = n;
        }
    }
    best
}

/// Fixed polycubes of k cells (up to translation), then one per rotation class.
pub fn shape_classes(k: usize) -> Vec<Vec<P3>> {
    let mut level: HashSet<Vec<P3>> = HashSet::from([vec![[0, 0, 0]]]);
    for _ in 1..k {
        let mut next = HashSet::new();
        for s in &level {
            for c in s {
                for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                    let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                    if s.contains(&n) {
                        continue;
                    }
                    let mut t = s.clone();
                    t.push(n);
                    next.insert(normalize(t));
                }
            }
        }
        level = next;
    }
    let classes: BTreeSet<Vec<P3>> = level.iter().map(|s| canon_shape(s)).collect();
    classes.into_iter().collect()
}

pub fn rotate_block(b: Block) -> Block {
    match b {
        Block::Observer { facing, powered } => Block::Observer { facing: facing.clockwise(), powered },
        Block::Piston { sticky, facing, extended } => Block::Piston { sticky, facing: facing.clockwise(), extended },
        Block::PistonHead { sticky, facing, short } => Block::PistonHead { sticky, facing: facing.clockwise(), short },
        Block::Trapdoor { iron, facing, top, open, powered } => Block::Trapdoor { iron, facing: facing.clockwise(), top, open, powered },
        Block::FenceGate { facing, in_wall, open, powered } => Block::FenceGate { facing: facing.clockwise(), in_wall, open, powered },
        Block::Door { iron, facing, upper, hinge_right, open, powered } => Block::Door { iron, facing: facing.clockwise(), upper, hinge_right, open, powered },
        Block::Repeater { facing, delay, locked, powered } => Block::Repeater { facing: facing.clockwise(), delay, locked, powered },
        Block::Comparator { facing, subtract, powered } => Block::Comparator { facing: facing.clockwise(), subtract, powered },
        Block::Glazed { facing } => Block::Glazed { facing: facing.clockwise() },
        Block::WallTorch { facing, lit } => Block::WallTorch { facing: facing.clockwise(), lit },
        b => b,
    }
}

/// A compact canonical key (128-bit digest) under translation and the 4 horizontal rotations: the
/// smallest digest of the 4 rotated, translated, position-sorted forms.
pub fn canon_hash(blocks: &[(BlockPos, Block)]) -> u128 {
    use std::hash::{Hash, Hasher};
    let mut cur: Vec<(P3, Block)> = blocks.iter().map(|(p, b)| (p.arr(), *b)).collect();
    let mut best = u128::MAX;
    for _ in 0..4 {
        cur = cur.iter().map(|(p, b)| (rot(*p), rotate_block(*b))).collect();
        let mut lo = [i32::MAX; 3];
        for (p, _) in &cur {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
            }
        }
        let mut v: Vec<(P3, Block)> = cur.iter().map(|(p, b)| ([p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]], *b)).collect();
        v.sort_by_key(|(p, _)| *p);
        let mut h1 = std::collections::hash_map::DefaultHasher::new();
        v.hash(&mut h1);
        let mut h2 = std::collections::hash_map::DefaultHasher::new();
        0x5eedu32.hash(&mut h2);
        v.hash(&mut h2);
        let d = ((h1.finish() as u128) << 64) | h2.finish() as u128;
        best = best.min(d);
    }
    best
}

/// A structure's canonical key under translation and the 4 horizontal rotations.
pub fn canon_structure(blocks: &[(BlockPos, Block)]) -> Vec<(P3, String)> {
    let mut cur: Vec<(P3, Block)> = blocks.iter().map(|(p, b)| (p.arr(), *b)).collect();
    let mut best: Option<Vec<(P3, String)>> = None;
    for _ in 0..4 {
        cur = cur.iter().map(|(p, b)| (rot(*p), rotate_block(*b))).collect();
        let mut lo = [i32::MAX; 3];
        for (p, _) in &cur {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
            }
        }
        let mut v: Vec<(P3, String)> = cur.iter().map(|(p, b)| ([p[0] - lo[0], p[1] - lo[1], p[2] - lo[2]], b.to_string())).collect();
        v.sort();
        if best.as_ref().is_none_or(|b| v < *b) {
            best = Some(v);
        }
    }
    best.unwrap()
}

const DIRS: [Dir; 6] = [Dir::North, Dir::East, Dir::South, Dir::West, Dir::Up, Dir::Down];

pub fn palette(spec: &str) -> Result<Vec<Block>> {
    let mut v = Vec::new();
    for name in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match name {
            "obs" => v.extend(DIRS.iter().map(|d| Block::Observer { facing: *d, powered: false })),
            "piston" => v.extend(DIRS.iter().map(|d| Block::Piston { sticky: false, facing: *d, extended: false })),
            "sticky" => v.extend(DIRS.iter().map(|d| Block::Piston { sticky: true, facing: *d, extended: false })),
            "note" => v.push(Block::parse("minecraft:note_block").unwrap()),
            "lamp" => v.push(Block::Lamp { lit: false }),
            "bulb" => v.push(Block::parse("minecraft:waxed_copper_bulb[lit=false,powered=false]").unwrap()),
            "stone" => v.push(Block::parse("minecraft:stone").unwrap()),
            "glass" => v.push(Block::parse("minecraft:glass").unwrap()),
            "obsidian" => v.push(Block::parse("minecraft:obsidian").unwrap()),
            "slime" => v.push(Block::parse("minecraft:slime_block").unwrap()),
            "honey" => v.push(Block::parse("minecraft:honey_block").unwrap()),
            "rblock" => v.push(Block::RedstoneBlock),
            "trap" => {
                for d in [Dir::North, Dir::East, Dir::South, Dir::West] {
                    for top in [false, true] {
                        v.push(Block::Trapdoor { iron: true, facing: d, top, open: false, powered: false });
                    }
                }
            }
            other => {
                if let Ok(b) = Block::parse(other) {
                    v.push(b);
                } else {
                    bail!("unknown palette entry {other}");
                }
            }
        }
    }
    Ok(v)
}

pub fn is_dynamic(b: &Block) -> bool {
    matches!(b, Block::Observer { .. } | Block::Piston { .. })
}

/// Note blocks take the instrument of the block below (as placed by a player).
pub fn fix_instruments(blocks: &mut [(BlockPos, Block)]) {
    let at: HashMap<BlockPos, Block> = blocks.iter().copied().collect();
    for (p, b) in blocks.iter_mut() {
        if let Block::Note { note, powered, .. } = *b {
            let below = at.get(&p.below()).copied().unwrap_or(Block::Air);
            let i = redsim::behavior::instrument(&below);
            let instrument = if i.works_above_note_block() { Instrument::Harp } else { i };
            *b = Block::Note { instrument, note, powered };
        }
    }
}

#[derive(Clone, Debug)]
pub struct Found {
    pub blocks: Vec<(BlockPos, Block)>,
    /// (kick, period, warmup, family, volume)
    pub runs: Vec<(Kick, u32, u32, String, i64)>,
}

pub fn kick_label(k: &Kick) -> String {
    match k {
        Kick::None => "self".into(),
        Kick::Last(p) => format!("last {} {} {}", p.x, p.y, p.z),
        other => format!("{other:?}"),
    }
}

/// Is a clock with this period still there after removing one block? (padding test)
pub fn padded(blocks: &[(BlockPos, Block)], kick: &Kick, period: u32, horizon: u32) -> bool {
    for i in 0..blocks.len() {
        let p = blocks[i].0;
        let k = match kick {
            Kick::Last(q) if *q == p => continue,
            k => k.clone(),
        };
        let mut b: Vec<(BlockPos, Block)> = blocks.to_vec();
        b.remove(i);
        fix_instruments(&mut b);
        let spec = Spec { name: String::new(), blocks: b, kick: k, output: None };
        let r = sim::simulate(&spec, &sim::Opts { horizon, margin: 5, period_only: true });
        if r.verdict == Verdict::Clock && r.period == period {
            return true;
        }
    }
    false
}

pub fn run(a: CensusArgs) -> Result<()> {
    let pal = palette(&a.palette)?;
    let shapes: Vec<Vec<P3>> = match &a.shape {
        Some(s) => vec![s.split(';').map(|c| {
            let v: Vec<i32> = c.split(',').map(|t| t.trim().parse().unwrap()).collect();
            [v[0], v[1], v[2]]
        }).collect()],
        None => shape_classes(a.k),
    };
    let kick_self = a.kicks.split(',').any(|s| s == "self");
    let kick_last = a.kicks.split(',').any(|s| s == "last");
    let require: Vec<String> = a.require.as_deref().unwrap_or("").split(',').filter(|s| !s.is_empty()).map(String::from).collect();
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    let k = shapes[0].len();
    let n = pal.len();
    // work units: (shape, first two cells' states)
    let units: Vec<(usize, usize)> = (0..shapes.len()).flat_map(|s| (0..n * n.min(if k > 1 { n } else { 1 })).map(move |u| (s, u))).collect();
    let total: u64 = shapes.len() as u64 * (n as u64).pow(k as u32);
    eprintln!("census: k={k}, {} shape classes, palette {n} states, {total} assignments, kicks self={kick_self} last={kick_last}, {threads} threads", shapes.len());
    let next = AtomicUsize::new(0);
    let sims = AtomicUsize::new(0);
    let found: Mutex<HashMap<Vec<(P3, String)>, Found>> = Mutex::new(HashMap::new());
    let padded_runs: Mutex<BTreeMap<u32, u64>> = Mutex::new(BTreeMap::new());
    let start = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                loop {
                    let ui = next.fetch_add(1, Ordering::Relaxed);
                    if ui >= units.len() {
                        break;
                    }
                    if ui % 2000 == 0 && ui > 0 {
                        let el = start.elapsed().as_secs_f64();
                        eprintln!("  unit {ui}/{} ({el:.0}s, {} sims, eta {:.0}s)", units.len(), sims.load(Ordering::Relaxed), el / ui as f64 * (units.len() - ui) as f64);
                    }
                    let (si, u) = units[ui];
                    let shape = &shapes[si];
                    let first = if k > 1 { [u / n, u % n] } else { [u, 0] };
                    let rest = k.saturating_sub(2);
                    let combos = n.pow(rest as u32);
                    for c in 0..combos {
                        let mut idx = vec![first[0]];
                        if k > 1 {
                            idx.push(first[1]);
                        }
                        let mut cc = c;
                        for _ in 0..rest {
                            idx.push(cc % n);
                            cc /= n;
                        }
                        let mut blocks: Vec<(BlockPos, Block)> = shape.iter().zip(idx.iter()).map(|(p, i)| (BlockPos::new(p[0], p[1], p[2]), pal[*i])).collect();
                        if !blocks.iter().any(|(_, b)| is_dynamic(b)) {
                            continue;
                        }
                        if !require.is_empty() && !require.iter().all(|r| blocks.iter().any(|(_, b)| sim::short(b) == *r)) {
                            continue;
                        }
                        // without pistons nothing moves: an observer must watch a structure cell
                        let has_piston = blocks.iter().any(|(_, b)| matches!(b, Block::Piston { .. }));
                        if !has_piston {
                            let cells: HashSet<BlockPos> = blocks.iter().map(|(p, _)| *p).collect();
                            if blocks.iter().any(|(p, b)| matches!(b, Block::Observer { facing, .. } if !cells.contains(&p.rel(*facing)))) {
                                continue;
                            }
                        }
                        fix_instruments(&mut blocks);
                        let mut kicks = Vec::new();
                        if kick_self {
                            kicks.push(Kick::None);
                        }
                        if kick_last {
                            kicks.extend(blocks.iter().map(|(p, _)| Kick::Last(*p)));
                        }
                        for kick in kicks {
                            sims.fetch_add(1, Ordering::Relaxed);
                            let spec = Spec { name: String::new(), blocks: blocks.clone(), kick: kick.clone(), output: None };
                            let r = sim::simulate(&spec, &sim::Opts { horizon: a.horizon, margin: 5, period_only: true });
                            if r.verdict != Verdict::Clock {
                                continue;
                            }
                            if padded(&blocks, &kick, r.period, a.horizon) {
                                *padded_runs.lock().unwrap().entry(r.period).or_default() += 1;
                                continue;
                            }
                            let full = sim::simulate(&spec, &sim::Opts { horizon: a.horizon, margin: 5, period_only: false });
                            let key = canon_structure(&blocks);
                            let mut f = found.lock().unwrap();
                            let e = f.entry(key).or_insert_with(|| Found { blocks: blocks.clone(), runs: vec![] });
                            // record runs of the stored representative only (rotated copies give the same runs)
                            if e.blocks == blocks {
                                e.runs.push((kick, full.period, full.warmup, full.family_name(), full.volume()));
                            }
                        }
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    eprintln!("census done: {} sims in {:.1}s, {} clock structures", sims.load(Ordering::Relaxed), start.elapsed().as_secs_f64(), found.len());
    // the table (every stored clock is minimal: padded runs were only counted)
    let padded_runs = padded_runs.into_inner().unwrap();
    let mut by_period: BTreeMap<u32, Vec<(String, i64, String, bool, Vec<(BlockPos, Block)>, Kick)>> = BTreeMap::new();
    let mut entries: Vec<&Found> = found.values().collect();
    entries.sort_by_key(|f| canon_structure(&f.blocks));
    for f in entries {
        let mut periods_seen = BTreeSet::new();
        for (kick, period, _warm, fam, vol) in &f.runs {
            if !periods_seen.insert(*period) {
                continue;
            }
            by_period.entry(*period).or_default().push((fam.clone(), *vol, kick_label(kick), true, f.blocks.clone(), kick.clone()));
        }
    }
    for p in padded_runs.keys() {
        by_period.entry(*p).or_default();
    }
    if let Some(d) = &a.out_dir {
        std::fs::create_dir_all(d)?;
    }
    println!("| period | minimal types | padded clock runs (a smaller clock plus inert blocks) | families (minimal types) |");
    println!("|---|---|---|---|");
    for (p, v) in &by_period {
        let mins: Vec<_> = v.iter().filter(|e| e.3).collect();
        let mut fams: BTreeMap<String, usize> = BTreeMap::new();
        for e in &mins {
            *fams.entry(e.0.clone()).or_default() += 1;
        }
        let fs: Vec<String> = fams.iter().map(|(f, c)| format!("{f} x{c}")).collect();
        println!("| {p} | {} | {} | {} |", mins.len(), padded_runs.get(p).copied().unwrap_or(0), fs.join(", "));
        if let Some(d) = &a.out_dir {
            let mut i = 0;
            let mut sorted: Vec<_> = mins.clone();
            sorted.sort_by_key(|e| (e.1, e.0.clone()));
            for e in sorted {
                let name = format!("{}-k{k}-p{p}-{i:04}", a.prefix);
                let spec = Spec { name: name.clone(), blocks: e.4.clone(), kick: e.5.clone(), output: e.4.iter().find(|(_, b)| matches!(b, Block::Observer { .. })).map(|(p, _)| *p) };
                let text = spec.to_text(&format!("clk-obs census (k={k}, palette {}): P={p} (redsim), family {}, kick {}, swept volume {}.", a.palette, e.0, e.2, e.1));
                std::fs::write(d.join(format!("{name}.txt")), text)?;
                i += 1;
            }
        }
    }
    Ok(())
}
