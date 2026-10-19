//! `mctas door fill`: exhaustive REGION FILLING on redsim, judged as a real door (no ideal plan,
//! no JVM). The region's cells are cleared, then every filling of `kmin..=k` of them with blocks
//! from a palette (plus `--fix` cells that are always filled) is placed into the design and run
//! through cosearch's redsim judge at the first judge position (`--screen-cycles` cycles); a pass
//! is judged at the 4 default positions + `--extra` more, with FULL seamless, the hallway check and
//! frame 12/12 (`cosearch::evaluate`), and written to `--out-dir`.
//!
//! Why: `door cosearch pockets` screens a pocket with the rest of the door driven by an ideal plan,
//! so a region whose output other REAL wiring watches (e88-t7b's bottom-right merger feeds the
//! bottom-left chain) has to be closed into one huge region first. Here the rest of the door stays
//! real: a filling either makes the whole door work or it does not.
//!
//! Pruning (before any simulation): a filling's blocks must form face-connected pieces that touch
//! the rest of the build (one piece unless `--gaps`), dust / repeaters / comparators / standing
//! torches / floor levers need a full block under them, wall torches the block they hang on.

use crate::DoorSpec;
use crate::cosearch::{self as cs, JudgeOpts, Pos, RsDoor};
use anyhow::{Context, Result, bail};
use redsim::Block;
use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

/// Options of one fill run.
#[derive(Clone, Debug)]
pub struct FillOpts {
    pub design: std::path::PathBuf,
    /// Region cells in door coordinates (x, y relative to the door's min corner).
    pub cells: Vec<(i32, i32)>,
    /// Cells always filled with one of these states (door coordinates); counted in the block total
    /// but not in `k`.
    pub fixed: Vec<((i32, i32), Vec<String>)>,
    /// Per-cell palettes overriding the global one (door coordinates).
    pub cell_palette: Vec<((i32, i32), Vec<String>)>,
    pub k: usize,
    pub kmin: usize,
    pub palette: Vec<String>,
    pub gaps: bool,
    pub threads: usize,
    pub screen_cycles: u32,
    pub extra: u32,
    pub out_dir: std::path::PathBuf,
    pub max_finds: usize,
    /// Accept a find only with at most this many blocks placed (0: any).
    pub max_blocks: usize,
    /// Count the fillings only.
    pub count: bool,
    pub report: u64,
    /// Accept doors that pass the judge without FULL / hallway / frame 12 too.
    pub any_seamless: bool,
}

/// Palette shorthands -> exact block states.
pub fn expand_palette(items: &[String]) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let hz = ["east", "west"];
    for it in items {
        let it = it.trim();
        if it.is_empty() {
            continue;
        }
        let mut add = |s: String| {
            if !out.contains(&s) {
                out.push(s);
            }
        };
        match it {
            "stone" => add("minecraft:stone".into()),
            "glass" => add("minecraft:glass".into()),
            "obsidian" => add("minecraft:obsidian".into()),
            "wool" => add("minecraft:white_wool".into()),
            "obs" => {
                for f in ["east", "west", "up", "down"] {
                    add(format!("minecraft:observer[facing={f},powered=false]"));
                }
            }
            "obsh" => {
                for f in hz {
                    add(format!("minecraft:observer[facing={f},powered=false]"));
                }
            }
            "obsv" => {
                for f in ["up", "down"] {
                    add(format!("minecraft:observer[facing={f},powered=false]"));
                }
            }
            "rep" | "rep1" | "rep2" | "rep3" | "rep4" | "rep12" => {
                let delays: Vec<u8> = match it {
                    "rep" => vec![1, 2, 3, 4],
                    "rep12" => vec![1, 2],
                    _ => vec![it[3..].parse().unwrap()],
                };
                for d in delays {
                    for f in hz {
                        add(format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]"));
                    }
                }
            }
            "dust" => add("minecraft:redstone_wire[east=side,north=none,power=0,south=none,west=side]".into()),
            "note" => add("minecraft:note_block[instrument=harp,note=0,powered=false]".into()),
            "lamp" => add("minecraft:redstone_lamp[lit=false]".into()),
            "target" => add("minecraft:target[power=0]".into()),
            "torch" => add("minecraft:redstone_torch[lit=true]".into()),
            "wtorch" => {
                for f in hz {
                    add(format!("minecraft:redstone_wall_torch[facing={f},lit=true]"));
                }
            }
            "cmp" => {
                for f in hz {
                    for m in ["compare", "subtract"] {
                        add(format!("minecraft:comparator[facing={f},mode={m},powered=false]"));
                    }
                }
            }
            "bulb" => {
                add("minecraft:waxed_copper_bulb[lit=false,powered=false]".into());
                add("minecraft:waxed_copper_bulb[lit=true,powered=false]".into());
            }
            "trap" => add("minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]".into()),
            "dropper" => {
                for f in ["east", "west", "up", "down"] {
                    add(format!("minecraft:dropper[facing={f},triggered=false]"));
                }
            }
            "sculk" => add("minecraft:sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]".into()),
            "calib" => {
                for f in hz {
                    add(format!("minecraft:calibrated_sculk_sensor[facing={f},power=0,sculk_sensor_phase=inactive,waterlogged=false]"));
                }
            }
            "redblock" => add("minecraft:redstone_block".into()),
            s if s.contains(':') || s.contains('[') => add(if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") }),
            s => add(format!("minecraft:{s}")),
        }
    }
    Ok(out)
}

fn parse_cell(t: &str) -> Result<(i32, i32)> {
    let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("cell {t:?}"))?;
    anyhow::ensure!(v.len() == 2, "cell x,y: {t:?}");
    Ok((v[0], v[1]))
}

/// `x,y;x,y;...` (also `x0..x1,y` and `x,y0..y1` ranges).
pub fn parse_cells(s: &str) -> Result<Vec<(i32, i32)>> {
    let mut out = Vec::new();
    for t in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let (a, b) = t.split_once(',').with_context(|| format!("cell x,y: {t:?}"))?;
        let range = |s: &str| -> Result<Vec<i32>> {
            if let Some((lo, hi)) = s.split_once("..") {
                let (lo, hi): (i32, i32) = (lo.trim().parse()?, hi.trim().parse()?);
                Ok((lo.min(hi)..=lo.max(hi)).collect())
            } else {
                Ok(vec![s.trim().parse()?])
            }
        };
        for x in range(a)? {
            for y in range(b)? {
                if !out.contains(&(x, y)) {
                    out.push((x, y));
                }
            }
        }
    }
    Ok(out)
}

/// `x,y=state|state;x,y=...` (shorthands allowed).
pub fn parse_cell_states(s: &str) -> Result<Vec<((i32, i32), Vec<String>)>> {
    let mut out = Vec::new();
    for t in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let (c, st) = t.split_once('=').with_context(|| format!("x,y=state|state: {t:?}"))?;
        let items: Vec<String> = st.split('|').map(|x| x.trim().to_string()).collect();
        out.push((parse_cell(c)?, expand_palette(&items)?));
    }
    Ok(out)
}

/// The cell a block needs as support (a full block), if any.
fn support_of(b: &Block, p: Pos) -> Option<Pos> {
    let below = [p[0], p[1] - 1, p[2]];
    match b {
        Block::Wire { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Torch { .. } | Block::Rail { .. } => Some(below),
        Block::Lever { face, facing, .. } | Block::Button { face, facing, .. } => match face {
            redsim::block::Face::Floor => Some(below),
            redsim::block::Face::Ceiling => Some([p[0], p[1] + 1, p[2]]),
            redsim::block::Face::Wall => {
                let s = facing.opposite().step();
                Some([p[0] + s[0], p[1] + s[1], p[2] + s[2]])
            }
        },
        Block::WallTorch { facing, .. } => {
            let s = facing.opposite().step();
            Some([p[0] + s[0], p[1] + s[1], p[2] + s[2]])
        }
        _ => None,
    }
}

fn combos(n: usize, s: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    fn rec(n: usize, s: usize, start: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == s {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            if n - i < s - cur.len() {
                break;
            }
            cur.push(i);
            rec(n, s, i + 1, cur, out);
            cur.pop();
        }
    }
    rec(n, s, 0, &mut Vec::new(), &mut out);
    out
}

/// One unit of work: a subset of free cells, each with its palette; the first `split` cells
/// fixed to the given palette indices.
struct Item {
    cells: Vec<Pos>,
    pals: Vec<usize>,
    prefix: Vec<usize>,
}

pub fn run(o: &FillOpts) -> Result<()> {
    let d = crate::cli::load_design(&o.design)?;
    let spec = DoorSpec::from_design(&d)?;
    anyhow::ensure!(!spec.along_z, "door-axis z designs are not supported");
    let door = spec.door;
    let w = |c: (i32, i32)| -> Pos { [door[0] + c.0, door[1] + c.1, door[2]] };
    let global = expand_palette(&o.palette)?;
    // palettes: index 0 = the global one, then one per cell override / fixed cell
    let mut pal_states: Vec<Vec<(String, Block)>> = Vec::new();
    let conv = |list: &[String]| -> Result<Vec<(String, Block)>> {
        list.iter().map(|s| cs::convert_state(s).map(|b| (s.clone(), b)).map_err(|e| anyhow::anyhow!("{s}: {e}"))).collect()
    };
    pal_states.push(conv(&global)?);
    let mut cell_pal: BTreeMap<Pos, usize> = BTreeMap::new();
    for (c, list) in &o.cell_palette {
        pal_states.push(conv(list)?);
        cell_pal.insert(w(*c), pal_states.len() - 1);
    }
    let mut fixed: Vec<(Pos, usize)> = Vec::new();
    for (c, list) in &o.fixed {
        pal_states.push(conv(list)?);
        fixed.push((w(*c), pal_states.len() - 1));
    }
    let fixed_set: HashSet<Pos> = fixed.iter().map(|(p, _)| *p).collect();
    let region: Vec<Pos> = o.cells.iter().map(|c| w(*c)).collect();
    let free: Vec<Pos> = region.iter().copied().filter(|p| !fixed_set.contains(p)).collect();
    // the base: the design without the region and the fixed cells
    let mut base: BTreeMap<Pos, Block> = BTreeMap::new();
    for (p, s) in d.block_list() {
        if region.contains(&p) || fixed_set.contains(&p) {
            continue;
        }
        let b = cs::convert_state(&s).map_err(|e| anyhow::anyhow!("{s} at {p:?}: {e}"))?;
        base.insert(p, b);
    }
    for p in region.iter().chain(fixed_set.iter()) {
        let c = d.blocks.get(p);
        if let Some(s) = c {
            let b = cs::convert_state(s).map_err(|e| anyhow::anyhow!("{s}: {e}"))?;
            if matches!(b, Block::Piston { .. } | Block::PistonHead { .. } | Block::Lever { .. }) || spec.door_cells().contains(p) {
                bail!("region cell {p:?} holds {s}: pistons, heads, the lever and door cells stay");
            }
        }
    }
    let base_count = base.values().filter(|b| !matches!(b, Block::PistonHead { .. })).count();
    let orig_count = d.blocks.values().filter(|s| !s.contains("piston_head")).count();
    println!(
        "fill {}: {} region cells ({} free, {} fixed), base {} blocks (design {}), k {}..={}, global palette {} states{}",
        o.design.display(),
        region.len(),
        free.len(),
        fixed.len(),
        base_count,
        orig_count,
        o.kmin,
        o.k,
        pal_states[0].len(),
        if o.gaps { ", gaps allowed" } else { "" }
    );
    // subsets
    let n4 = |p: Pos| -> [Pos; 4] { [[p[0] + 1, p[1], p[2]], [p[0] - 1, p[1], p[2]], [p[0], p[1] + 1, p[2]], [p[0], p[1] - 1, p[2]]] };
    let touches_base = |p: Pos, filled: &HashSet<Pos>| -> bool {
        n4(p).iter().any(|q| !filled.contains(q) && base.get(q).is_some_and(|b| !b.is_air()))
            || [[p[0], p[1], p[2] + 1], [p[0], p[1], p[2] - 1]].iter().any(|q| base.get(q).is_some_and(|b| !b.is_air()))
    };
    let mut items: Vec<Item> = Vec::new();
    let mut total: u128 = 0;
    let mut subsets_ok = 0usize;
    for s in o.kmin..=o.k.min(free.len()) {
        for combo in combos(free.len(), s) {
            let cells: Vec<Pos> = combo.iter().map(|&i| free[i]).collect();
            let mut filled: HashSet<Pos> = cells.iter().copied().collect();
            filled.extend(fixed_set.iter().copied());
            if filled.is_empty() {
                continue;
            }
            // connectivity: pieces (4-neighborhood) each touching the base; one piece without gaps
            let mut seen: HashSet<Pos> = HashSet::new();
            let mut pieces = 0;
            let mut ok = true;
            for &st in filled.iter() {
                if seen.contains(&st) {
                    continue;
                }
                pieces += 1;
                let mut stack = vec![st];
                seen.insert(st);
                let mut touch = false;
                while let Some(p) = stack.pop() {
                    touch |= touches_base(p, &filled);
                    for q in n4(p) {
                        if filled.contains(&q) && seen.insert(q) {
                            stack.push(q);
                        }
                    }
                }
                if !touch {
                    ok = false;
                }
            }
            if !ok || (!o.gaps && pieces > 1) {
                continue;
            }
            subsets_ok += 1;
            let mut all_cells: Vec<Pos> = fixed.iter().map(|(p, _)| *p).collect();
            all_cells.extend(cells.iter().copied());
            let pals: Vec<usize> = fixed.iter().map(|(_, k)| *k).chain(cells.iter().map(|p| *cell_pal.get(p).unwrap_or(&0))).collect();
            let prod: u128 = pals.iter().map(|k| pal_states[*k].len() as u128).product();
            total += prod;
            // split into items of at most ~4096 fillings
            let mut split = 0;
            let mut rest = prod;
            while rest > 4096 && split < pals.len() {
                rest /= pal_states[pals[split]].len() as u128;
                split += 1;
            }
            let mut prefixes: Vec<Vec<usize>> = vec![Vec::new()];
            for i in 0..split {
                let mut next = Vec::new();
                for pre in &prefixes {
                    for v in 0..pal_states[pals[i]].len() {
                        let mut p2 = pre.clone();
                        p2.push(v);
                        next.push(p2);
                    }
                }
                prefixes = next;
            }
            for prefix in prefixes {
                items.push(Item { cells: all_cells.clone(), pals: pals.clone(), prefix });
            }
        }
    }
    println!("{subsets_ok} subsets pass the connectivity rule; {total} fillings to screen in {} work items", items.len());
    if o.count {
        return Ok(());
    }
    std::fs::create_dir_all(&o.out_dir)?;
    let screen_opts = JudgeOpts { cycles: o.screen_cycles, max_ticks: 200, offsets: vec![JudgeOpts::with_extra(0).offsets[0]], empty_walkway: true };
    let full_opts = JudgeOpts::with_extra(o.extra);
    let next = AtomicUsize::new(0);
    let screened = AtomicU64::new(0);
    let pruned = AtomicU64::new(0);
    let passed = AtomicU64::new(0);
    let finds = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let seen_finds: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
    let t0 = std::time::Instant::now();
    let stem = o.design.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "fill".into());
    std::thread::scope(|sc| {
        // progress
        sc.spawn(|| {
            let mut last = std::time::Instant::now();
            while !stop.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(200));
                if last.elapsed().as_secs() >= o.report {
                    last = std::time::Instant::now();
                    let el = t0.elapsed().as_secs_f64();
                    let s = screened.load(Ordering::Relaxed);
                    println!(
                        "[{el:>6.0}s] {s} screened ({:.0}/s), {} pruned, item {}/{}, {} pass the 1-position screen, {} finds",
                        s as f64 / el.max(1e-9),
                        pruned.load(Ordering::Relaxed),
                        next.load(Ordering::Relaxed).min(items.len()),
                        items.len(),
                        passed.load(Ordering::Relaxed),
                        finds.load(Ordering::Relaxed)
                    );
                }
            }
        });
        let mut hs = Vec::new();
        for _ in 0..o.threads.max(1) {
            hs.push(sc.spawn(|| {
                loop {
                    if stop.load(Ordering::Relaxed) {
                        break;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let it = &items[i];
                    let n = it.cells.len();
                    let mut idx: Vec<usize> = vec![0; n];
                    idx[..it.prefix.len()].copy_from_slice(&it.prefix);
                    let lens: Vec<usize> = it.pals.iter().map(|k| pal_states[*k].len()).collect();
                    'outer: loop {
                        // build the filling
                        let mut map = base.clone();
                        for j in 0..n {
                            map.insert(it.cells[j], pal_states[it.pals[j]][idx[j]].1);
                        }
                        let mut ok = true;
                        for j in 0..n {
                            let b = &pal_states[it.pals[j]][idx[j]].1;
                            if let Some(sp) = support_of(b, it.cells[j])
                                && !map.get(&sp).is_some_and(|x| cs::full_cube(x))
                            {
                                ok = false;
                                break;
                            }
                        }
                        if ok {
                            let door = RsDoor { spec, blocks: map.iter().map(|(p, b)| (*p, *b)).collect() };
                            screened.fetch_add(1, Ordering::Relaxed);
                            if cs::judge(&door, &screen_opts).is_ok() {
                                passed.fetch_add(1, Ordering::Relaxed);
                                let e = cs::evaluate(&door, &full_opts, false);
                                let good = e.judge.is_ok() && (o.any_seamless || e.record_ok()) && (o.max_blocks == 0 || e.blocks <= o.max_blocks);
                                let desc: Vec<String> = (0..n).map(|j| format!("({},{}) {}", it.cells[j][0] - spec.door[0], it.cells[j][1] - spec.door[1], pal_states[it.pals[j]][idx[j]].0.trim_start_matches("minecraft:"))).collect();
                                if good {
                                    let key = desc.join(" ");
                                    if seen_finds.lock().unwrap().insert(key.clone()) {
                                        let k = finds.fetch_add(1, Ordering::Relaxed);
                                        let name = format!("{stem}-fill-{}b-{k}", e.blocks);
                                        let path = o.out_dir.join(format!("{name}.txt"));
                                        let mut dd = door.to_design(&name);
                                        dd.meta.insert("note".into(), format!("door fill of {}: {key}", o.design.display()));
                                        let _ = std::fs::write(&path, dd.to_text());
                                        println!("FIND {}: {} | {key}", path.display(), cs::eval_line(&name, &e, 0.0));
                                        if k + 1 >= o.max_finds {
                                            stop.store(true, Ordering::Relaxed);
                                        }
                                    }
                                } else {
                                    println!("screen pass, rejected: {} | {}", cs::eval_line("cand", &e, 0.0), desc.join(" "));
                                }
                            }
                        } else {
                            pruned.fetch_add(1, Ordering::Relaxed);
                        }
                        // next assignment (odometer over the cells after the prefix)
                        let mut j = n;
                        loop {
                            if j == it.prefix.len() {
                                break 'outer;
                            }
                            j -= 1;
                            idx[j] += 1;
                            if idx[j] < lens[j] {
                                break;
                            }
                            idx[j] = 0;
                        }
                        if stop.load(Ordering::Relaxed) {
                            break;
                        }
                    }
                }
            }));
        }
        for h in hs {
            let _ = h.join();
        }
        stop.store(true, Ordering::Relaxed);
    });
    let el = t0.elapsed().as_secs_f64();
    println!(
        "done in {el:.0} s: {} screened, {} pruned, {} passed the screen, {} finds",
        screened.load(Ordering::Relaxed),
        pruned.load(Ordering::Relaxed),
        passed.load(Ordering::Relaxed),
        finds.load(Ordering::Relaxed)
    );
    Ok(())
}
