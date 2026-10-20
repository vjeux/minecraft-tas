//! `mctas circuit fly grow`: add 1 or 2 blocks around known machines (every empty cell next to
//! the machine, every palette state), run every candidate on redsim in every quarter turn with
//! the design's own kick, and keep those that fly at another SPEED or in another DIRECTION class
//! than the base: the way to slower and faster machines (an observer that waits for the other
//! part, a pulse stretched by a second observer, a landing piston firing by itself...).

use crate::census::{CensusOptions, Kick, corner, run, turn_block};
use crate::machine::Machine;
use anyhow::{Result, bail};
use clock::Pos;
use redsim::Block;
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Debug)]
pub struct GrowOptions {
    pub add: usize,
    pub palette: Vec<String>,
    pub horizon: u32,
    pub margin: i32,
    pub workers: usize,
    /// Keep only the candidates FASTER than their base.
    pub faster: bool,
}

/// A grown machine that flies differently from its base.
#[derive(Clone, Debug)]
pub struct Grown {
    pub base: String,
    pub cells: Vec<(Pos, Block)>,
    pub kick: Kick,
    pub period: u32,
    pub d: Pos,
    pub first: u32,
    /// Quarter turns that fly (bit r).
    pub mask: u8,
}

impl Grown {
    pub fn speed(&self) -> String {
        let n = self.d.iter().map(|c| c.abs()).sum::<i32>().max(1) as u32;
        if self.period % n == 0 { format!("{}", self.period / n) } else { format!("{}/{}", self.period, n) }
    }

    pub fn class(&self) -> &'static str {
        match self.d {
            [_, y, _] if y > 0 => "up",
            [_, y, _] if y < 0 => "down",
            _ => "horizontal",
        }
    }
}

fn kick_of(m: &Machine) -> Result<Kick> {
    Ok(match &m.clock.kick {
        Some(clock::Kick::Last { pos }) => Kick::Last(*pos),
        Some(clock::Kick::Place { pos, state, ticks: 1 }) if state.ends_with("stone") => Kick::Temp(*pos),
        _ => bail!("{}: grow needs a `last` kick or a 1 gt stone", m.name()),
    })
}

/// The cells moved to touch the origin, with the kick moved alike.
fn normalized(cells: &[(Pos, Block)], kick: Kick) -> (Vec<(Pos, Block)>, Kick) {
    let c = corner(cells.iter().map(|(p, _)| *p));
    let mv = |p: Pos| [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
    let k = match kick {
        Kick::Last(p) => Kick::Last(mv(p)),
        Kick::Temp(p) => Kick::Temp(mv(p)),
    };
    (cells.iter().map(|(p, b)| (mv(*p), *b)).collect(), k)
}

/// Quarter turn `t` of the cells and the kick (redsim block states turned).
fn turned(cells: &[(Pos, Block)], kick: Kick, t: u8) -> Option<(Vec<(Pos, Block)>, Kick)> {
    let mut v = Vec::new();
    for (p, b) in cells {
        let mut bb = *b;
        for _ in 0..t {
            bb = turn_block(bb)?;
        }
        v.push((clock::rotate_pos(*p, t), bb));
    }
    let k = match kick {
        Kick::Last(p) => Kick::Last(clock::rotate_pos(p, t)),
        Kick::Temp(p) => Kick::Temp(clock::rotate_pos(p, t)),
    };
    Some(normalized(&v, k))
}

/// Run a candidate in every quarter turn: (mask, first flying turn's (period, d, first)).
fn fly(cells: &[(Pos, Block)], kick: Kick, o: &CensusOptions) -> Option<(u8, u32, Pos, u32)> {
    let mut mask = 0u8;
    let mut first: Option<(u32, Pos, u32)> = None;
    for t in 0..4u8 {
        let (c, k) = turned(cells, kick, t)?;
        if let Some((p, d, f)) = run(&c, k, o) {
            mask |= 1 << t;
            if first.is_none() {
                // the direction in the design's own frame: turn d back
                first = Some((p, clock::rotate_pos(d, (4 - t) % 4), f));
            }
        }
    }
    first.map(|(p, d, f)| (mask, p, d, f))
}

/// Grow every design; the candidates that fly at a new (speed, class).
pub fn grow(designs: &[Machine], o: &GrowOptions) -> Result<Vec<Grown>> {
    let mut pal: Vec<Block> = Vec::new();
    for s in &o.palette {
        let b = Block::parse(s).map_err(|e| anyhow::anyhow!("{e}"))?;
        if turn_block(b).is_none() {
            bail!("grow palette: {s} cannot be turned");
        }
        if !pal.contains(&b) {
            pal.push(b);
        }
    }
    let co = CensusOptions {
        dims: [1, 1, 1],
        k: 0,
        kmin: 0,
        palette: Vec::new(),
        max_pistons: 0,
        max_observers: 0,
        horizon: o.horizon,
        margin: o.margin,
        workers: 1,
        out: std::path::PathBuf::new(),
        loose: false,
        shard: (0, 1),
        procs: 1,
    };
    // per design: its cells, kick, base (speed, class), empty cells around it
    struct Base {
        name: String,
        cells: Vec<(Pos, Block)>,
        kick: Kick,
        speed: (u32, u32),
        free: Vec<Pos>,
    }
    let mut bases = Vec::new();
    for m in designs {
        let mut cells = Vec::new();
        for (p, s) in &m.design().blocks {
            cells.push((*p, Block::parse(s).map_err(|e| anyhow::anyhow!("{e}"))?));
        }
        let kick = kick_of(m)?;
        let (cells, kick) = normalized(&cells, kick);
        let Some((_, p, d, _)) = fly(&cells, kick, &co) else {
            eprintln!("grow: {} does not fly on redsim, skipped", m.name());
            continue;
        };
        let n = d.iter().map(|c| c.abs()).sum::<i32>().max(1) as u32;
        let kp = match kick {
            Kick::Last(p) | Kick::Temp(p) => p,
        };
        let taken: std::collections::HashSet<Pos> = cells.iter().map(|(p, _)| *p).collect();
        let mut free = Vec::new();
        let hi = [0, 1, 2].map(|i| cells.iter().map(|(p, _)| p[i]).max().unwrap());
        for x in -1..=hi[0] + 1 {
            for y in -1..=hi[1] + 1 {
                for z in -1..=hi[2] + 1 {
                    let q = [x, y, z];
                    let near = cells.iter().any(|(p, _)| (0..3).map(|i| (p[i] - q[i]).abs()).sum::<i32>() == 1);
                    if near && !taken.contains(&q) && q != kp {
                        free.push(q);
                    }
                }
            }
        }
        bases.push(Base { name: m.name().to_string(), cells, kick, speed: (p, n), free });
    }
    // jobs: (base, first added cell, its state)
    let mut jobs: Vec<(usize, usize, usize)> = Vec::new();
    for (bi, b) in bases.iter().enumerate() {
        for i in 0..b.free.len() {
            for s in 0..pal.len() {
                jobs.push((bi, i, s));
            }
        }
    }
    eprintln!("grow: {} designs, {} first moves, add {}", bases.len(), jobs.len(), o.add);
    let next = AtomicUsize::new(0);
    let found: Mutex<Vec<Grown>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..o.workers.max(1) {
            sc.spawn(|| {
                loop {
                    let j = next.fetch_add(1, Ordering::Relaxed);
                    if j >= jobs.len() {
                        break;
                    }
                    let (bi, i, s) = jobs[j];
                    let b = &bases[bi];
                    let mut tries: Vec<Vec<(Pos, Block)>> = Vec::new();
                    let mut one = b.cells.clone();
                    one.push((b.free[i], pal[s]));
                    if o.add >= 2 {
                        for i2 in i + 1..b.free.len() {
                            for s2 in &pal {
                                let mut two = one.clone();
                                two.push((b.free[i2], *s2));
                                tries.push(two);
                            }
                        }
                    }
                    tries.push(one);
                    for cells in tries {
                        let (c, k) = normalized(&cells, b.kick);
                        if let Some((mask, p, d, f)) = fly(&c, k, &co) {
                            let n = d.iter().map(|x| x.abs()).sum::<i32>().max(1) as u32;
                            // a new speed: P/n differs from the base's
                            let (new, old) = ((p as u64) * (b.speed.1 as u64), (b.speed.0 as u64) * (n as u64));
                            if if o.faster { new < old } else { new != old } {
                                found.lock().unwrap().push(Grown { base: b.name.clone(), cells: c, kick: k, period: p, d, first: f, mask });
                            }
                        }
                    }
                }
            });
        }
    });
    let mut v = found.into_inner().unwrap();
    v.sort_by_key(|g| (g.cells.len(), g.period * 1000 / g.d.iter().map(|c| c.abs()).sum::<i32>().max(1) as u32, std::cmp::Reverse(g.mask.count_ones())));
    Ok(v)
}

/// A grown machine as a design.
pub fn design_text(g: &Grown, name: &str) -> String {
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".chars().collect();
    let mut used: BTreeMap<String, char> = BTreeMap::new();
    for (_, b) in &g.cells {
        let n = used.len();
        used.entry(b.to_string()).or_insert(chars[n]);
    }
    let kp = match g.kick {
        Kick::Last(p) | Kick::Temp(p) => p,
    };
    let mut lo = corner(g.cells.iter().map(|(p, _)| *p).chain(std::iter::once(kp)));
    let hi = [0, 1, 2].map(|i| g.cells.iter().map(|(p, _)| p[i]).chain(std::iter::once(kp[i])).max().unwrap());
    lo = [lo[0], lo[1], lo[2]];
    let n = g.d.iter().map(|c| c.abs()).sum::<i32>().max(1);
    let dir = g.d.map(|c| c / n);
    let mut t = format!("# grow of {}: {} blocks, {} gt per {n} cell(s), first move t{}, flies turned {}\n", g.base, g.cells.len(), g.period, g.first, crate::census::mask_text(g.mask));
    t.push_str(&format!("name: {name}\ncircuit: flying-machine\ntravel: {} {} {}\ndistance: 24\norigin: {} {} {}\n", dir[0], dir[1], dir[2], lo[0], lo[1], lo[2]));
    match g.kick {
        Kick::Last(p) => t.push_str(&format!("port start: {} {} {} last\n", p[0], p[1], p[2])),
        Kick::Temp(p) => t.push_str(&format!("port start: {} {} {} minecraft:stone 1\n", p[0], p[1], p[2])),
    }
    t.push_str("palette:\n");
    for (s, c) in &used {
        t.push_str(&format!("  {c} {s}\n"));
    }
    let map: std::collections::HashMap<Pos, String> = g.cells.iter().map(|(p, b)| (*p, b.to_string())).collect();
    for y in lo[1]..=hi[1] {
        t.push_str(&format!("layer {}:\n", y - lo[1]));
        for z in lo[2]..=hi[2] {
            let row: String = (lo[0]..=hi[0]).map(|x| map.get(&[x, y, z]).map(|s| used[s]).unwrap_or('.')).collect();
            t.push_str(&format!("  {row}\n"));
        }
    }
    t
}
