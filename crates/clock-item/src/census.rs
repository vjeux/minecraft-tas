//! The census: every design of k blocks in a box from a palette, container contents from a list,
//! judged in redsim ([`crate::sim`]); clocks kept when irreducible (no single block can go with the
//! same period), deduplicated by type (translation + the 4 horizontal rotations), written per
//! period.

use crate::sim::{self, Spec};
use redsim::behavior::{Support, sturdy_of};
use redsim::{Block, BlockPos, Dir};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Expand a palette shorthand into block states (redsim syntax, `minecraft:` optional).
pub fn palette_entry(name: &str) -> Result<Vec<String>, String> {
    let h = ["north", "east", "south", "west"];
    let all6 = ["down", "up", "north", "east", "south", "west"];
    let v: Vec<String> = match name {
        "hopper" => ["down", "north", "east", "south", "west"].iter().map(|f| format!("minecraft:hopper[enabled=true,facing={f}]")).collect(),
        "dropper" => all6.iter().map(|f| format!("minecraft:dropper[facing={f},triggered=false]")).collect(),
        "dispenser" => all6.iter().map(|f| format!("minecraft:dispenser[facing={f},triggered=false]")).collect(),
        "comparator" => h
            .iter()
            .flat_map(|f| ["compare", "subtract"].map(|m| format!("minecraft:comparator[facing={f},mode={m},powered=false]")))
            .collect(),
        "comparator-c" => h.iter().map(|f| format!("minecraft:comparator[facing={f},mode=compare,powered=false]")).collect(),
        "repeater" => h
            .iter()
            .flat_map(|f| (1..=4).map(move |d| format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]")))
            .collect(),
        "repeater1" => h.iter().map(|f| format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]")).collect(),
        "dust" => vec![
            "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into(),
            "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into(),
        ],
        "torch" => std::iter::once("minecraft:redstone_torch[lit=true]".to_string())
            .chain(h.iter().map(|f| format!("minecraft:redstone_wall_torch[facing={f},lit=true]")))
            .collect(),
        "observer" => all6.iter().map(|f| format!("minecraft:observer[facing={f},powered=false]")).collect(),
        "sticky" => all6.iter().map(|f| format!("minecraft:sticky_piston[extended=false,facing={f}]")).collect(),
        "piston" => all6.iter().map(|f| format!("minecraft:piston[extended=false,facing={f}]")).collect(),
        "rblock" => vec!["minecraft:redstone_block".into()],
        "lamp" => vec!["minecraft:redstone_lamp[lit=false]".into()],
        "stone" | "glass" | "target" => vec![format!("minecraft:{name}")],
        other if other.contains('[') || other.contains(':') => vec![other.to_string()],
        other => vec![format!("minecraft:{other}")],
    };
    Ok(v)
}

/// Container contents shorthand: `stone:3` (one stack of 3 in slot 0), `sword:2` (2 swords, one
/// per slot: non-stackable), any `item:count` (one stack), or raw SNBT in braces.
pub fn contents_entry(s: &str) -> Result<String, String> {
    if s.starts_with('{') {
        return Ok(s.to_string());
    }
    let (item, count) = s.split_once(':').ok_or_else(|| format!("contents {s:?}: expected item:count"))?;
    let n: u32 = count.parse().map_err(|_| format!("contents {s:?}: bad count"))?;
    let id = match item {
        "sword" => "diamond_sword",
        "shovel" => "wooden_shovel",
        "pearl" => "ender_pearl",
        other => other,
    };
    let unstackable = matches!(id, "diamond_sword" | "wooden_shovel");
    if unstackable {
        let list: Vec<String> = (0..n).map(|i| format!("{{Slot:{i}b,id:\"minecraft:{id}\",count:1}}")).collect();
        Ok(format!("{{Items:[{}]}}", list.join(",")))
    } else {
        Ok(format!("{{Items:[{{Slot:0b,id:\"minecraft:{id}\",count:{n}}}]}}"))
    }
}

pub struct Opts {
    pub size: [i32; 3],
    pub k_min: usize,
    pub k_max: usize,
    pub palette: Vec<Block>,
    pub contents: Vec<String>,
    pub max_ticks: u32,
    pub jobs: usize,
    /// Keep clocks whose items do not move too (pure redstone clocks found on the way).
    pub any_family: bool,
}

/// A clock found by the census.
#[derive(Clone, Debug)]
pub struct Found {
    pub spec: Spec,
    pub ch: sim::Character,
}

fn is_container(b: &Block) -> bool {
    matches!(b, Block::Hopper { .. } | Block::Dispenser { .. } | Block::Crafter { .. })
}

/// What a block needs below it (None: nothing).
fn needs_floor(b: &Block) -> Option<Support> {
    match b {
        Block::Comparator { .. } | Block::Repeater { .. } => Some(Support::Rigid),
        Block::Wire { .. } => Some(Support::Full),
        Block::Torch { .. } => Some(Support::Center),
        _ => None,
    }
}

fn floor_ok(below: &Block, need: Support, b: &Block) -> bool {
    if matches!(b, Block::Wire { .. }) && matches!(below, Block::Hopper { .. }) {
        return true;
    }
    sturdy_of(below, Dir::Up, need)
}

/// The block turned a quarter clockwise (seen from above): facings and dust sides.
pub fn rotate_block(b: Block) -> Block {
    let r = |d: Dir| if matches!(d, Dir::Up | Dir::Down) { d } else { d.clockwise() };
    match b {
        Block::Hopper { facing, enabled } => Block::Hopper { facing: r(facing), enabled },
        Block::Dispenser { dropper, facing, triggered } => Block::Dispenser { dropper, facing: r(facing), triggered },
        Block::Comparator { facing, subtract, powered } => Block::Comparator { facing: r(facing), subtract, powered },
        Block::Repeater { facing, delay, locked, powered } => Block::Repeater { facing: r(facing), delay, locked, powered },
        Block::WallTorch { facing, lit } => Block::WallTorch { facing: r(facing), lit },
        Block::Observer { facing, powered } => Block::Observer { facing: r(facing), powered },
        Block::Piston { sticky, facing, extended } => Block::Piston { sticky, facing: r(facing), extended },
        Block::Wire { north, east, south, west, power } => Block::Wire { north: west, east: north, south: east, west: south, power },
        other => other,
    }
}

/// A type key: the blocks (no contents) normalized to the origin, the smallest of the 4
/// rotations; with the contents too when `with_nbt`.
pub fn type_key(spec: &Spec, with_nbt: bool) -> String {
    let mut best: Option<String> = None;
    let mut blocks: Vec<(BlockPos, Block)> = spec.blocks.clone();
    let mut nbt: Vec<(BlockPos, String)> = spec.nbt.clone();
    for _ in 0..4 {
        let minx = blocks.iter().map(|(p, _)| p.x).min().unwrap();
        let miny = blocks.iter().map(|(p, _)| p.y).min().unwrap();
        let minz = blocks.iter().map(|(p, _)| p.z).min().unwrap();
        let norm = |p: BlockPos| BlockPos::new(p.x - minx, p.y - miny, p.z - minz);
        let mut items: Vec<String> = blocks.iter().map(|(p, b)| format!("{} {} {} {b}", norm(*p).x, norm(*p).y, norm(*p).z)).collect();
        if with_nbt {
            items.extend(nbt.iter().map(|(p, s)| format!("{} {} {} nbt {s}", norm(*p).x, norm(*p).y, norm(*p).z)));
        }
        items.sort();
        let key = items.join(";");
        if best.as_ref().is_none_or(|b| key < *b) {
            best = Some(key);
        }
        // a quarter turn clockwise: (x, z) -> (-z, x)
        blocks = blocks.iter().map(|(p, b)| (BlockPos::new(-p.z, p.y, p.x), rotate_block(*b))).collect();
        nbt = nbt.iter().map(|(p, s)| (BlockPos::new(-p.z, p.y, p.x), s.clone())).collect();
    }
    best.unwrap()
}

/// FNV-1a of a key (short ids for file names).
pub fn short_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", h >> 32)
}

/// Every k-subset of the box's cells that is face-connected and touches the box's three low
/// faces (one translation per shape), cells in (y, z, x) order.
fn subsets(size: [i32; 3], k: usize) -> Vec<Vec<BlockPos>> {
    let mut cells = Vec::new();
    for y in 0..size[1] {
        for z in 0..size[2] {
            for x in 0..size[0] {
                cells.push(BlockPos::new(x, y, z));
            }
        }
    }
    let mut out = Vec::new();
    let mut idx: Vec<usize> = Vec::new();
    fn rec(cells: &[BlockPos], k: usize, start: usize, idx: &mut Vec<usize>, out: &mut Vec<Vec<BlockPos>>) {
        if idx.len() == k {
            let set: Vec<BlockPos> = idx.iter().map(|i| cells[*i]).collect();
            if set.iter().any(|p| p.x == 0) && set.iter().any(|p| p.y == 0) && set.iter().any(|p| p.z == 0) && connected(&set) {
                out.push(set);
            }
            return;
        }
        for i in start..cells.len() {
            idx.push(i);
            rec(cells, k, i + 1, idx, out);
            idx.pop();
        }
    }
    rec(&cells, k, 0, &mut idx, &mut out);
    out
}

fn connected(set: &[BlockPos]) -> bool {
    let all: HashSet<BlockPos> = set.iter().copied().collect();
    let mut seen = HashSet::new();
    let mut stack = vec![set[0]];
    while let Some(p) = stack.pop() {
        if !seen.insert(p) {
            continue;
        }
        for d in Dir::ALL {
            let q = p.rel(d);
            if all.contains(&q) && !seen.contains(&q) {
                stack.push(q);
            }
        }
    }
    seen.len() == set.len()
}

/// A comparator reading a container (directly behind it, or behind one conductor behind it).
fn has_reader(blocks: &[(BlockPos, Block)]) -> bool {
    let at: HashMap<BlockPos, Block> = blocks.iter().copied().collect();
    blocks.iter().any(|(p, b)| {
        let Block::Comparator { facing, .. } = b else { return false };
        let back = p.rel(*facing);
        match at.get(&back) {
            Some(c) if is_container(c) => true,
            Some(c) if redsim::behavior::conductor_of(c) => at.get(&back.rel(*facing)).is_some_and(is_container),
            _ => false,
        }
    })
}

fn wall_torches_ok(blocks: &[(BlockPos, Block)]) -> bool {
    let at: HashMap<BlockPos, Block> = blocks.iter().copied().collect();
    blocks.iter().all(|(p, b)| match b {
        Block::WallTorch { facing, .. } => at.get(&p.rel(facing.opposite())).is_some_and(|s| sturdy_of(s, *facing, Support::Full)),
        _ => true,
    })
}

/// Judge one candidate; Some(found) for an item clock (or any clock with `any_family`).
pub fn judge_candidate(spec: &Spec, o: &Opts) -> Option<Found> {
    let r = sim::judge(spec, o.max_ticks, None).ok()?;
    let ch = r.ok()?;
    if ch.period < 2 || !ch.signal {
        return None;
    }
    let items_move = ch.waves.iter().any(|w| w.items_change);
    if !items_move && !o.any_family {
        return None;
    }
    Some(Found { spec: spec.clone(), ch })
}

/// Irreducible: removing any single block (with its contents) does not leave a clock with the
/// same period, and emptying any container does not leave the same clock (same period and
/// output waveform): no junk items.
pub fn irreducible(f: &Found, o: &Opts) -> bool {
    let any = Opts { any_family: true, ..o.clone_opts() };
    for i in 0..f.spec.blocks.len() {
        let mut s = f.spec.clone();
        let (p, _) = s.blocks.remove(i);
        s.nbt.retain(|(q, _)| *q != p);
        if s.blocks.is_empty() {
            continue;
        }
        if let Some(g) = judge_candidate(&s, &any)
            && g.ch.period == f.ch.period
        {
            return false;
        }
    }
    for i in 0..f.spec.nbt.len() {
        let mut s = f.spec.clone();
        s.nbt.remove(i);
        if let Some(g) = judge_candidate(&s, &any)
            && g.ch.period == f.ch.period
            && g.ch.on_ticks == f.ch.on_ticks
            && g.ch.rising == f.ch.rising
        {
            return false;
        }
    }
    true
}

/// The mechanism key: the type key with every block that never changes (state or contents)
/// collapsed to `#` (floors, supports, inert conductors): clocks that differ only by such blocks
/// are one mechanism.
pub fn mechanism_key(f: &Found) -> String {
    let active: HashSet<BlockPos> = f.ch.waves.iter().map(|w| w.pos).collect();
    let mut blocks = Vec::new();
    for (p, b) in &f.spec.blocks {
        if active.contains(p) {
            blocks.push((*p, *b));
        } else {
            blocks.push((*p, Block::Simple(redsim::block::Simple::Stone)));
        }
    }
    type_key(&Spec { blocks, nbt: Vec::new() }, false)
}

impl Opts {
    fn clone_opts(&self) -> Opts {
        Opts {
            size: self.size,
            k_min: self.k_min,
            k_max: self.k_max,
            palette: self.palette.clone(),
            contents: self.contents.clone(),
            max_ticks: self.max_ticks,
            jobs: self.jobs,
            any_family: self.any_family,
        }
    }
}

/// Run the census; returns every irreducible clock found (one per type and contents variant).
pub fn run(o: &Opts, progress: bool) -> Vec<Found> {
    let mut all_found: Vec<Found> = Vec::new();
    for k in o.k_min..=o.k_max {
        let subs = subsets(o.size, k);
        let next = AtomicUsize::new(0);
        let tried = AtomicUsize::new(0);
        let found: Mutex<Vec<Found>> = Mutex::new(Vec::new());
        let seen: Mutex<HashSet<String>> = Mutex::new(HashSet::new());
        let start = std::time::Instant::now();
        std::thread::scope(|sc| {
            for _ in 0..o.jobs.max(1) {
                sc.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= subs.len() {
                            break;
                        }
                        let cells = &subs[i];
                        let mut assign: Vec<Block> = Vec::with_capacity(k);
                        dfs(o, cells, &mut assign, &tried, &found, &seen);
                    }
                });
            }
            if progress {
                sc.spawn(|| {
                    let mut last = std::time::Instant::now();
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        let n = next.load(Ordering::Relaxed);
                        if n >= subs.len() {
                            break;
                        }
                        if last.elapsed().as_secs() < 30 {
                            continue;
                        }
                        last = std::time::Instant::now();
                        eprintln!(
                            "  k={k}: {}/{} shapes, {} candidates judged, {} clocks, {:.0} s",
                            n.min(subs.len()),
                            subs.len(),
                            tried.load(Ordering::Relaxed),
                            found.lock().unwrap().len(),
                            start.elapsed().as_secs_f64()
                        );
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        if progress {
            eprintln!("k={k}: {} shapes, {} candidates judged, {} clock variants in {:.1} s", subs.len(), tried.load(Ordering::Relaxed), found.len(), start.elapsed().as_secs_f64());
        }
        all_found.extend(found);
    }
    all_found
}

fn dfs(o: &Opts, cells: &[BlockPos], assign: &mut Vec<Block>, tried: &AtomicUsize, found: &Mutex<Vec<Found>>, seen: &Mutex<HashSet<String>>) {
    let i = assign.len();
    if i == cells.len() {
        let blocks: Vec<(BlockPos, Block)> = cells.iter().copied().zip(assign.iter().copied()).collect();
        if !wall_torches_ok(&blocks) || !has_reader(&blocks) {
            return;
        }
        let containers: Vec<BlockPos> = blocks.iter().filter(|(_, b)| is_container(b)).map(|(p, _)| *p).collect();
        // contents: each container empty or one of the options; at least one filled
        let m = o.contents.len() + 1;
        let combos = m.pow(containers.len() as u32);
        for c in 1..combos {
            let mut nbt = Vec::new();
            let mut x = c;
            for p in &containers {
                let pick = x % m;
                x /= m;
                if pick > 0 {
                    nbt.push((*p, o.contents[pick - 1].clone()));
                }
            }
            let spec = Spec { blocks: blocks.clone(), nbt };
            tried.fetch_add(1, Ordering::Relaxed);
            if let Some(f) = judge_candidate(&spec, o) {
                let key = type_key(&f.spec, true);
                if seen.lock().unwrap().contains(&key) {
                    continue;
                }
                if irreducible(&f, o) {
                    seen.lock().unwrap().insert(key);
                    found.lock().unwrap().push(f);
                }
            }
        }
        return;
    }
    let p = cells[i];
    for b in &o.palette {
        if let Some(need) = needs_floor(b) {
            let below = p.below();
            match cells.iter().position(|q| *q == below) {
                Some(j) if j < i && floor_ok(&assign[j], need, b) => {}
                _ => continue,
            }
        }
        assign.push(*b);
        dfs(o, cells, assign, tried, found, seen);
        assign.pop();
    }
}

/// Group found clocks by period, then type; write a design per (type, contents) and a summary.
pub fn report(found: &[Found], out_dir: &std::path::Path) -> anyhow::Result<String> {
    std::fs::create_dir_all(out_dir)?;
    let mut by_period: BTreeMap<u32, Vec<&Found>> = BTreeMap::new();
    for f in found {
        by_period.entry(f.ch.period).or_default().push(f);
    }
    let mut summary = String::new();
    summary.push_str("| period | types | variants | smallest (blocks) | families |\n|---|---|---|---|---|\n");
    let mut tsv = String::from("period\tblocks\tfamily\ttype\tvariant\ttransient\ton_ticks\trising\tfile\n");
    for (p, list) in &by_period {
        let mut types: BTreeMap<String, Vec<&Found>> = BTreeMap::new();
        for f in list {
            types.entry(type_key(&f.spec, false)).or_default().push(f);
        }
        let smallest = list.iter().map(|f| f.spec.blocks.len()).min().unwrap_or(0);
        let mut fams: Vec<String> = list.iter().map(|f| f.ch.family.clone()).collect();
        fams.sort();
        fams.dedup();
        summary.push_str(&format!("| {p} | {} | {} | {smallest} | {} |\n", types.len(), list.len(), fams.join(", ")));
        let mut sorted: Vec<&&Found> = list.iter().collect();
        sorted.sort_by_key(|f| (f.spec.blocks.len(), type_key(&f.spec, true)));
        for f in sorted {
            let tk = type_key(&f.spec, false);
            let vk = type_key(&f.spec, true);
            let file = format!("p{p}-{}b-{}-{}.txt", f.spec.blocks.len(), short_hash(&tk), short_hash(&vk));
            let mut d = f.spec.to_design(&file.trim_end_matches(".txt").to_string(), f.ch.output);
            d.meta.insert("period".into(), p.to_string());
            d.meta.insert("family".into(), f.ch.family.clone());
            d.meta.insert("waveform".into(), format!("output on {} of {} gt, {} rising edge(s)", f.ch.on_ticks, p, f.ch.rising));
            d.save(&out_dir.join(&file))?;
            tsv.push_str(&format!(
                "{p}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{file}\n",
                f.spec.blocks.len(),
                f.ch.family,
                short_hash(&tk),
                short_hash(&vk),
                f.ch.transient,
                f.ch.on_ticks,
                f.ch.rising
            ));
        }
    }
    std::fs::write(out_dir.join("clocks.tsv"), &tsv)?;
    std::fs::write(out_dir.join("SUMMARY.md"), &summary)?;
    Ok(summary)
}
