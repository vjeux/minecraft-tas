//! `mctas circuit fly station`: search for RETURN STATIONS on redsim. A reversible machine (one
//! that flies backward when its other piston fires first: two sticky pistons, each part pushing
//! with a 2 gt spit or reach-pulling) flies `approach` cells toward a wall; the search tries every
//! wall cell in front of its leading face and every filling of up to `k` more station blocks in
//! the cells around its stopping place (not on its path), and keeps the stations that send it
//! back: it must stand intact at the wall, then intact 4 cells back. Stationary blocks may be
//! anything (observers, repeaters, dust, plain blocks): repeaters and dust next to a moving slime
//! block are left in place, other blocks next to slime get dragged (the run then fails).

use crate::census::Kick;
use crate::machine::Machine;
use anyhow::{Result, bail};
use clock::Pos;
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The machine kicked from its other end: a 2 gt redstone block next to the sticky piston that
/// faces against `travel` (the first free side cell touching no other piston), `travel` reversed.
/// None when it has no such piston or no free cell for the kick.
pub fn reversed(m: &Machine) -> Option<Machine> {
    reversed_all(m).into_iter().next()
}

/// Every way to kick the machine from its other end: a 2 gt redstone block in each free side cell
/// of the back-facing sticky piston (cells touching another piston last).
pub fn reversed_all(m: &Machine) -> Vec<Machine> {
    let Some(t) = m.travel else {
        return Vec::new();
    };
    let back = [-t[0], -t[1], -t[2]];
    let face = |d: Pos| match d {
        [-1, 0, 0] => "west",
        [1, 0, 0] => "east",
        [0, -1, 0] => "down",
        [0, 1, 0] => "up",
        [0, 0, -1] => "north",
        _ => "south",
    };
    let want = format!("facing={}", face(back));
    let blocks = &m.design().blocks;
    let pistons: Vec<Pos> = blocks.iter().filter(|(_, s)| s.contains("piston[") && !s.contains("piston_head")).map(|(p, _)| *p).collect();
    let Some(s) = blocks.iter().find(|(_, st)| st.starts_with("minecraft:sticky_piston[") && st.contains(&want) && !st.contains("extended=true")).map(|(p, _)| *p) else {
        return Vec::new();
    };
    let sides: [Pos; 6] = [[-1, 0, 0], [1, 0, 0], [0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1]];
    let near = |a: Pos, b: Pos| (0..3).map(|i| (a[i] - b[i]).abs()).sum::<i32>() == 1;
    let mut cells: Vec<Pos> = sides.iter().map(|d| add(s, *d)).filter(|c| *c != add(s, back) && !blocks.contains_key(c) && !m.stops.iter().any(|(q, _)| q == c)).collect();
    cells.sort_by_key(|c| pistons.iter().any(|p| *p != s && near(*p, *c)));
    let mut out = Vec::new();
    for cell in cells {
        let mut d = m.design().clone();
        d.name = format!("rev-{}", m.name());
        d.ports.insert("start".into(), cell);
        d.meta.insert("port start".into(), "minecraft:redstone_block 2".into());
        d.meta.insert("travel".into(), format!("{} {} {}", back[0], back[1], back[2]));
        let mut mm = m.clone();
        let Ok(c) = clock::Clock::from_design(d) else {
            continue;
        };
        mm.clock = c;
        mm.travel = Some(back);
        out.push(mm);
    }
    out
}

#[derive(Clone, Debug)]
pub struct StationOptions {
    pub approach: i32,
    pub k: usize,
    pub radius: i32,
    pub palette: Vec<String>,
    pub horizon: u32,
    pub workers: usize,
    /// Cells it must come back past the wall.
    pub back: i32,
    /// This process's share of the jobs: (k, n) = every job whose index is k mod n.
    pub shard: (usize, usize),
}

/// A found station as one line (shard files): wall tick, back tick, cells.
pub fn station_line(s: &Station) -> String {
    let cells: Vec<String> = s.blocks.iter().map(|(p, b)| format!("{},{},{}={b}", p[0], p[1], p[2])).collect();
    format!("{}\t{}\t{}", s.at_wall, s.back_at, cells.join(";"))
}

pub fn parse_station(line: &str) -> Result<Station> {
    let w: Vec<&str> = line.split('\t').collect();
    if w.len() != 3 {
        bail!("bad station line {line:?}");
    }
    let mut blocks = Vec::new();
    for c in w[2].split(';') {
        let (p, b) = c.split_once('=').ok_or_else(|| anyhow::anyhow!("bad cell {c:?}"))?;
        let v: Vec<i32> = p.split(',').map(|x| x.parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        blocks.push(([v[0], v[1], v[2]], Block::parse(b).map_err(|e| anyhow::anyhow!("{e}"))?));
    }
    Ok(Station { blocks, at_wall: w[0].parse()?, back_at: w[1].parse()? })
}

/// A station that works.
#[derive(Clone, Debug)]
pub struct Station {
    pub blocks: Vec<(Pos, Block)>,
    /// Tick it stands at the wall, tick it stands `back` cells back.
    pub at_wall: u32,
    pub back_at: u32,
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: Pos, k: i32) -> Pos {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn rest(b: Block) -> Option<Block> {
    match b {
        Block::PistonHead { .. } => None,
        Block::Piston { sticky, facing, .. } => Some(Block::Piston { sticky, facing, extended: false }),
        Block::Observer { facing, .. } => Some(Block::Observer { facing, powered: false }),
        Block::Repeater { facing, delay, .. } => Some(Block::Repeater { facing, delay, locked: false, powered: false }),
        Block::Wire { north, east, south, west, .. } => Some(Block::Wire { north, east, south, west, power: 0 }),
        Block::Torch { .. } => Some(Block::Torch { lit: true }),
        Block::WallTorch { facing, .. } => Some(Block::WallTorch { facing, lit: true }),
        Block::Lamp { .. } => Some(Block::Lamp { lit: false }),
        other => Some(other),
    }
}

/// The machine's cells, kick and travel, as redsim blocks.
struct Engine {
    cells: Vec<(Pos, Block)>,
    kick: Kick,
    kick_block: Block,
    /// A temporary kick block stands this many gt.
    kick_ticks: u32,
    travel: Pos,
    /// The box the machine is looked for in each tick (set by the search: its path, the way
    /// back and one cell around).
    scan: (Pos, Pos),
}

fn engine(m: &Machine) -> Result<Engine> {
    let travel = m.travel.ok_or_else(|| anyhow::anyhow!("the machine needs `travel:`"))?;
    let mut cells = Vec::new();
    for (p, s) in &m.design().blocks {
        cells.push((*p, Block::parse(s).map_err(|e| anyhow::anyhow!("{e}"))?));
    }
    let (kick, kick_block, kick_ticks) = match &m.clock.kick {
        Some(clock::Kick::Last { pos }) => (Kick::Last(*pos), Block::Air, 0),
        Some(clock::Kick::Place { pos, state, ticks }) => (Kick::Temp(*pos), Block::parse(state).map_err(|e| anyhow::anyhow!("{e}"))?, *ticks),
        _ => bail!("station search: the machine's kick must be `last` or a temporary block"),
    };
    if m.clock.kick_at != 0 {
        bail!("station search: the kick must start at tick 0");
    }
    Ok(Engine { cells, kick, kick_block, kick_ticks, travel, scan: ([0; 3], [0; 3]) })
}

/// Run the machine toward a station; Some((tick at the wall, tick back)) when it comes back.
fn run(e: &Engine, station: &[(Pos, Block)], o: &StationOptions, lo: Pos, hi: Pos) -> Option<(u32, u32)> {
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut w = World::new(bp(lo), size);
    let st: Vec<(BlockPos, Block)> = station.iter().map(|(p, b)| (bp(*p), *b)).collect();
    w.place_raw(&st);
    w.place_updates(&st);
    let withheld = match e.kick {
        Kick::Last(p) => Some(p),
        Kick::Temp(_) => None,
    };
    let kept: Vec<(BlockPos, Block)> = e.cells.iter().filter(|(p, _)| Some(*p) != withheld).map(|(p, b)| (bp(*p), *b)).collect();
    w.place_raw(&kept);
    w.place_updates(&kept);
    match e.kick {
        Kick::Last(p) => {
            let b = e.cells.iter().find(|(q, _)| *q == p).unwrap().1;
            w.set_block(bp(p), b, flags::ALL);
        }
        Kick::Temp(p) => {
            w.set_block(bp(p), e.kick_block, flags::ALL);
        }
    }
    let station_at: HashMap<Pos, Block> = station.iter().copied().collect();
    let mut reference: Vec<(Pos, Block)> = e.cells.iter().filter_map(|(p, b)| rest(*b).map(|r| (*p, r))).collect();
    reference.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
    let rc = corner(reference.iter().map(|(p, _)| *p));
    for (p, _) in reference.iter_mut() {
        *p = [p[0] - rc[0], p[1] - rc[1], p[2] - rc[2]];
    }
    let along = |s: Pos| s[0] * e.travel[0] + s[1] * e.travel[1] + s[2] * e.travel[2];
    let mut at_wall: Option<u32> = None;
    let mut shape: Vec<(Pos, Block)> = Vec::new();
    for t in 0..=o.horizon {
        if t == e.kick_ticks
            && let Kick::Temp(p) = e.kick
        {
            w.set_block(bp(p), Block::Air, flags::ALL);
        }
        if !w.drops.is_empty() || !w.errors.is_empty() {
            if std::env::var("MCTAS_FLY_DEBUG").is_ok() {
                eprintln!("t{t}: drops {:?} errors {:?}", w.drops.first(), w.errors.first());
            }
            return None;
        }
        // the machine: every non-air cell of the scan box (its path and one cell around: all
        // motion is along the travel) that is not a station block in its own state (a station
        // block that changed state stays out: observers firing, repeaters, dust)
        shape.clear();
        let mut moving = false;
        for y in e.scan.0[1]..=e.scan.1[1] {
            for z in e.scan.0[2]..=e.scan.1[2] {
                for x in e.scan.0[0]..=e.scan.1[0] {
                    let b = w.get(BlockPos::new(x, y, z));
                    if b.is_air() {
                        continue;
                    }
                    let pa = [x, y, z];
                    if let Some(sb) = station_at.get(&pa)
                        && rest(*sb) == rest(b)
                    {
                        continue;
                    }
                    if t < e.kick_ticks
                        && let Kick::Temp(kp) = e.kick
                        && kp == pa
                    {
                        continue;
                    }
                    moving |= matches!(b, Block::MovingPiston { .. });
                    if let Some(r) = rest(b) {
                        shape.push((pa, r));
                    }
                }
            }
        }
        if !moving && shape.len() == reference.len() {
            shape.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
            let c = corner(shape.iter().map(|(p, _)| *p));
            if shape.iter().zip(&reference).all(|((p, s), (q, r))| s == r && [p[0] - c[0], p[1] - c[1], p[2] - c[2]] == *q) {
                let a = along([c[0] - rc[0], c[1] - rc[1], c[2] - rc[2]]);
                if a >= o.approach && at_wall.is_none() {
                    at_wall = Some(t);
                }
                if let Some(t1) = at_wall
                    && a <= o.approach - o.back
                {
                    return Some((t1, t));
                }
            }
        }
        if t > 2 && w.idle() {
            if std::env::var("MCTAS_FLY_DEBUG").is_ok() {
                eprintln!("t{t}: idle, at wall {at_wall:?}, {} machine cells", shape.len());
            }
            return None;
        }
        w.tick();
    }
    None
}

fn corner(ps: impl Iterator<Item = Pos>) -> Pos {
    let mut lo = [i32::MAX; 3];
    for p in ps {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
        }
    }
    lo
}

/// The search.
pub fn search(m: &Machine, o: &StationOptions) -> Result<Vec<Station>> {
    let mut e = engine(m)?;
    let mut pal: Vec<Block> = Vec::new();
    for s in &o.palette {
        let b = Block::parse(s).map_err(|er| anyhow::anyhow!("{er}"))?;
        if !pal.contains(&b) {
            pal.push(b);
        }
    }
    let t = e.travel;
    // the machine's path (every cell it occupies from the start to the wall, heads included) and
    // its place at the wall
    let mut path: BTreeSet<Pos> = BTreeSet::new();
    for k in 0..=o.approach {
        for (p, _) in &e.cells {
            let q = add(*p, scale(t, k));
            path.insert(q);
            path.insert(add(q, t));
        }
    }
    let fin: Vec<Pos> = e.cells.iter().map(|(p, _)| add(*p, scale(t, o.approach))).collect();
    let walls: Vec<Pos> = fin.iter().map(|p| add(*p, t)).filter(|q| !fin.contains(q)).collect::<BTreeSet<_>>().into_iter().collect();
    let flo = corner(fin.iter().copied());
    let fhi = [0, 1, 2].map(|i| fin.iter().map(|p| p[i]).max().unwrap());
    let mut region: Vec<Pos> = Vec::new();
    for x in flo[0] - o.radius..=fhi[0] + o.radius {
        for y in flo[1] - o.radius..=fhi[1] + o.radius {
            for z in flo[2] - o.radius..=fhi[2] + o.radius {
                let p = [x, y, z];
                // not on the path, not beyond the wall line (the machine must not need them)
                if !path.contains(&p) || walls.contains(&p) {
                    region.push(p);
                }
            }
        }
    }
    region.retain(|p| !path.contains(p) || walls.contains(p));
    // the world: the start box to the wall + radius + margin, every direction 3 more
    let mlo = corner(e.cells.iter().map(|(p, _)| *p));
    let mut lo = [0, 1, 2].map(|i| mlo[i].min(flo[i] - o.radius) - 4);
    let mut hi = [0, 1, 2].map(|i| fhi[i].max(e.cells.iter().map(|(p, _)| p[i]).max().unwrap()) + o.radius + 4);
    for i in 0..3 {
        if t[i] < 0 {
            hi[i] += o.back + 2;
        } else if t[i] > 0 {
            lo[i] -= o.back + 2;
        }
    }
    // the scan box: the path (and the way back) with one cell around
    let plo = corner(path.iter().copied());
    let phi = [0, 1, 2].map(|i| path.iter().map(|p| p[i]).max().unwrap());
    e.scan = ([0, 1, 2].map(|i| plo[i] - 1 - if t[i] > 0 { o.back + 1 } else { 0 }), [0, 1, 2].map(|i| phi[i] + 1 + if t[i] < 0 { o.back + 1 } else { 0 }));
    let e = e;
    eprintln!("station search: {} wall cells, {} region cells, palette {}, k <= {}", walls.len(), region.len(), pal.len(), o.k);
    // jobs: (wall, first station cell index, its state) + the k = 0 wall alone
    let mut jobs: Vec<(Pos, Option<(usize, usize)>)> = Vec::new();
    for w in &walls {
        jobs.push((*w, None));
        if o.k >= 1 {
            for i in 0..region.len() {
                if region[i] == *w {
                    continue;
                }
                for s in 0..pal.len() {
                    jobs.push((*w, Some((i, s))));
                }
            }
        }
    }
    let jobs: Vec<(Pos, Option<(usize, usize)>)> = jobs.into_iter().enumerate().filter(|(i, _)| i % o.shard.1.max(1) == o.shard.0).map(|(_, j)| j).collect();
    let next = AtomicUsize::new(0);
    let found: Mutex<Vec<Station>> = Mutex::new(Vec::new());
    let sims = AtomicUsize::new(0);
    let obsidian = Block::parse("minecraft:obsidian").unwrap();
    std::thread::scope(|sc| {
        for _ in 0..o.workers.max(1) {
            sc.spawn(|| {
                loop {
                    let j = next.fetch_add(1, Ordering::Relaxed);
                    if j >= jobs.len() {
                        break;
                    }
                    let (w, first) = jobs[j];
                    let mut st: Vec<(Pos, Block)> = vec![(w, obsidian)];
                    match first {
                        None => {
                            sims.fetch_add(1, Ordering::Relaxed);
                            if let Some((a, b)) = run(&e, &st, o, lo, hi) {
                                found.lock().unwrap().push(Station { blocks: st.clone(), at_wall: a, back_at: b });
                            }
                        }
                        Some((i, s)) => {
                            st.push((region[i], pal[s]));
                            rec(&e, o, &region, &pal, &mut st, i + 1, lo, hi, &found, &sims);
                        }
                    }
                }
            });
        }
    });
    eprintln!("station search: {} sims", sims.load(Ordering::Relaxed));
    let mut v = found.into_inner().unwrap();
    v.sort_by_key(|s| (s.blocks.len(), s.back_at));
    Ok(v)
}

#[allow(clippy::too_many_arguments)]
fn rec(e: &Engine, o: &StationOptions, region: &[Pos], pal: &[Block], st: &mut Vec<(Pos, Block)>, from: usize, lo: Pos, hi: Pos, found: &Mutex<Vec<Station>>, sims: &AtomicUsize) {
    sims.fetch_add(1, Ordering::Relaxed);
    if let Some((a, b)) = run(e, st, o, lo, hi) {
        found.lock().unwrap().push(Station { blocks: st.clone(), at_wall: a, back_at: b });
        return;
    }
    // st holds the wall plus the station blocks so far
    if st.len() > o.k {
        return;
    }
    for i in from..region.len() {
        if st.iter().any(|(p, _)| *p == region[i]) {
            continue;
        }
        for s in pal {
            st.push((region[i], *s));
            rec(e, o, region, pal, st, i + 1, lo, hi, found, sims);
            st.pop();
        }
    }
}

/// The machine with the station as one design text (`station:` lines: they count).
pub fn design_text(m: &Machine, s: &Station, name: &str) -> String {
    let mut mm = m.clone();
    for (p, b) in &s.blocks {
        mm.stops.push((*p, b.to_string()));
        mm.stations.insert(*p);
    }
    mm.clock.design.name = name.to_string();
    format!("# return station search: at the wall t{}, back t{}\n{}", s.at_wall, s.back_at, mm.to_text())
}
