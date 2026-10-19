//! A clock runner on redsim (track clk-obs). Temporary: it follows clockinfra's decisions
//! (2026-10-19: the `port start:` kick forms, the door judge's placement for self-starting
//! clocks, the full state key) and gives way to `clock::sim::simulate` once `crates/clock`
//! lands on main. Only the generators and searches of this crate use it.

use anyhow::{Context, Result, bail};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::hash::{Hash, Hasher};

/// The optional start kick (`port start: x y z <form>`, clockinfra's three forms).
#[derive(Clone, Debug, PartialEq)]
pub enum Kick {
    /// No kick: a self-starting clock (the door judge's placement, then run).
    None,
    /// `last`: the design's own block at that cell is withheld from the silent placement and
    /// placed last with updates (UPDATE_ALL), like a player.
    Last(BlockPos),
    /// `use [T]`: right-click the design block there once (and again T gt later).
    Use(BlockPos, Option<u32>),
    /// `<state> <T>`: a temporary block placed with updates in an air cell, removed T gt later.
    Temp(BlockPos, Block, u32),
}

impl Kick {
    pub fn to_line(&self) -> Option<String> {
        match self {
            Kick::None => None,
            Kick::Last(p) => Some(format!("port start: {} {} {} last", p.x, p.y, p.z)),
            Kick::Use(p, None) => Some(format!("port start: {} {} {} use", p.x, p.y, p.z)),
            Kick::Use(p, Some(t)) => Some(format!("port start: {} {} {} use {t}", p.x, p.y, p.z)),
            Kick::Temp(p, b, t) => Some(format!("port start: {} {} {} {b} {t}", p.x, p.y, p.z)),
        }
    }
}

/// A clock design: blocks in design order, the kick, the output cell.
#[derive(Clone, Debug)]
pub struct Spec {
    pub name: String,
    pub blocks: Vec<(BlockPos, Block)>,
    pub kick: Kick,
    pub output: Option<BlockPos>,
}

fn bp(p: [i32; 3]) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

impl Spec {
    pub fn load(path: &std::path::Path) -> Result<Spec> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Spec::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    /// The door design format plus `port output: x y z` and `port start: x y z <form>` (a bare
    /// position means `last`).
    pub fn parse(text: &str) -> Result<Spec> {
        let mut kick_line = None;
        let mut rest = String::new();
        for line in text.lines() {
            let l = line.trim();
            if let Some(r) = l.strip_prefix("port start:") {
                kick_line = Some(r.trim().to_string());
                continue;
            }
            rest.push_str(line);
            rest.push('\n');
        }
        let d = mctas::design::Design::parse(&rest)?;
        let mut blocks = Vec::new();
        for (p, s) in d.block_list() {
            let b = Block::parse(&s).map_err(|e| anyhow::anyhow!("{e}"))?;
            blocks.push((bp(p), b));
        }
        let kick = match kick_line {
            None => Kick::None,
            Some(k) => {
                let t: Vec<&str> = k.split_whitespace().collect();
                if t.len() < 3 {
                    bail!("port start needs x y z");
                }
                let p = BlockPos::new(t[0].parse()?, t[1].parse()?, t[2].parse()?);
                match t.get(3).copied() {
                    None | Some("last") => Kick::Last(p),
                    Some("use") => Kick::Use(p, t.get(4).map(|s| s.parse()).transpose()?),
                    Some(state) => {
                        let b = Block::parse(state).map_err(|e| anyhow::anyhow!("{e}"))?;
                        let dt: u32 = t.get(5).context("a temporary block needs its duration")?.parse()?;
                        Kick::Temp(p, b, dt)
                    }
                }
            }
        };
        Ok(Spec { name: d.name.clone(), blocks, kick, output: d.ports.get("output").map(|p| bp(*p)) })
    }

    pub fn bounds(&self) -> (BlockPos, BlockPos) {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        let mut add = |p: BlockPos| {
            for (i, v) in p.arr().into_iter().enumerate() {
                lo[i] = lo[i].min(v);
                hi[i] = hi[i].max(v);
            }
        };
        for (p, _) in &self.blocks {
            add(*p);
        }
        if let Kick::Temp(p, _, _) = &self.kick {
            add(*p);
        }
        (bp(lo), bp(hi))
    }

    /// The design file text (layers bottom to top, rows north to south).
    pub fn to_text(&self, comment: &str) -> String {
        let (lo, hi) = self.bounds();
        let mut palette: Vec<Block> = Vec::new();
        for (_, b) in &self.blocks {
            if !palette.contains(b) {
                palette.push(*b);
            }
        }
        const CH: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let ch = |b: &Block| CH.as_bytes()[palette.iter().position(|q| q == b).unwrap()] as char;
        let mut s = format!("name: {}\n", self.name);
        for l in comment.lines() {
            s.push_str(&format!("# {l}\n"));
        }
        s.push_str(&format!("origin: {} {} {}\n", lo.x, 0, lo.z));
        if let Some(o) = self.output {
            s.push_str(&format!("port output: {} {} {}\n", o.x, o.y, o.z));
        }
        if let Some(k) = self.kick.to_line() {
            s.push_str(&k);
            s.push('\n');
        }
        s.push_str("palette:\n");
        for b in &palette {
            s.push_str(&format!("  {} {}\n", ch(b), b));
        }
        let at: HashMap<BlockPos, Block> = self.blocks.iter().copied().collect();
        for y in lo.y..=hi.y {
            s.push_str(&format!("layer {y}:\n"));
            for z in lo.z..=hi.z {
                let mut row = String::from("  ");
                for x in lo.x..=hi.x {
                    row.push(at.get(&BlockPos::new(x, y, z)).map(ch).unwrap_or('.'));
                }
                s.push_str(row.trim_end());
                s.push('\n');
            }
        }
        s
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Periodic: the whole state repeats.
    Clock,
    /// Nothing pending any more at that tick: a static structure.
    Stops(u32),
    /// No repeat within the horizon.
    NoRepeat,
    /// The model met something it does not model (or a write outside the world box).
    Unmodeled(String),
    /// An item was created (a drop) or a block escaped the watch box.
    Invalid(String),
}

/// One changing cell over one period.
#[derive(Clone, Debug)]
pub struct CellWave {
    pub pos: BlockPos,
    /// The block names the cell takes during the period.
    pub names: Vec<String>,
    /// Ticks per period with the cell "on" (powered, lit, extended, open; None if not a
    /// component with an on state).
    pub on_ticks: Option<u32>,
    pub rising: u32,
    /// The on-run lengths in period order, starting from the first rising edge.
    pub widths: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub verdict: Verdict,
    pub period: u32,
    pub warmup: u32,
    pub family: Vec<String>,
    pub cells: Vec<CellWave>,
    pub output: Option<CellWave>,
    pub blocks: usize,
    /// The swept box: every cell non-air at any tick, and the design's box.
    pub swept: (BlockPos, BlockPos),
}

impl Report {
    pub fn volume(&self) -> i64 {
        let (lo, hi) = self.swept;
        ((hi.x - lo.x + 1) * (hi.y - lo.y + 1) * (hi.z - lo.z + 1)) as i64
    }
    pub fn family_name(&self) -> String {
        self.family.join("+")
    }
}

#[derive(Clone, Debug)]
pub struct Opts {
    pub horizon: u32,
    /// Air margin around the design (writes beyond it are reported as escapes).
    pub margin: i32,
    /// Skip the per-cell waves (searches only need the period).
    pub period_only: bool,
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { horizon: 3000, margin: 6, period_only: false }
    }
}

/// "On" for components with an output state.
pub fn on(b: &Block) -> Option<bool> {
    Some(match *b {
        Block::Observer { powered, .. } => powered,
        Block::Lamp { lit } => lit,
        Block::Bulb { lit, .. } => lit,
        Block::Note { powered, .. } => powered,
        Block::Trapdoor { open, .. } => open,
        Block::Door { open, .. } => open,
        Block::FenceGate { open, .. } => open,
        Block::Rail { powered, .. } => powered,
        Block::Piston { extended, .. } => extended,
        Block::Repeater { powered, .. } => powered,
        Block::Comparator { powered, .. } => powered,
        Block::Torch { lit } | Block::WallTorch { lit, .. } => lit,
        Block::Wire { power, .. } => power > 0,
        Block::Dispenser { triggered, .. } => triggered,
        Block::Skull { powered, .. } | Block::WallSkull { powered, .. } => powered,
        Block::Target { power } => power > 0,
        _ => return None,
    })
}

/// A block's family name (`minecraft:` and properties stripped).
pub fn short(b: &Block) -> String {
    if let Block::PistonHead { sticky, .. } = b {
        return if *sticky { "sticky_piston".into() } else { "piston".into() };
    }
    let s = b.to_string();
    let s = s.strip_prefix("minecraft:").unwrap_or(&s).to_string();
    match s.find('[') {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

fn state_key(w: &World) -> u64 {
    let mut h = DefaultHasher::new();
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                let p = w.min.offset(x, y, z);
                let b = w.get(p);
                if !b.is_air() {
                    (x, y, z).hash(&mut h);
                    b.hash(&mut h);
                }
            }
        }
    }
    let mut ticks = w.ticks.all();
    ticks.sort_by_key(|t| (t.trigger, t.priority, t.sub));
    for t in &ticks {
        (t.kind, t.pos, t.trigger - w.time, t.priority).hash(&mut h);
    }
    0xffu8.hash(&mut h);
    for e in w.events.iter() {
        e.hash(&mut h);
    }
    0xfeu8.hash(&mut h);
    for (p, m) in w.piston.all() {
        (p, m.moved, m.direction, m.extending, m.source, m.progress, m.progress_o).hash(&mut h);
    }
    let mut c: Vec<(BlockPos, u8)> = w.comparator_out.iter().map(|(p, v)| (*p, *v)).collect();
    c.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    c.hash(&mut h);
    h.finish()
}

/// Every non-air cell's block, in the world box.
fn snapshot(w: &World) -> BTreeMap<(i32, i32, i32), Block> {
    let mut m = BTreeMap::new();
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                let p = w.min.offset(x, y, z);
                let b = w.get(p);
                if !b.is_air() {
                    let b = match b {
                        Block::MovingPiston { .. } => w.piston.be(p).map(|m| m.moved).unwrap_or(b),
                        _ => b,
                    };
                    m.insert((p.x, p.y, p.z), b);
                }
            }
        }
    }
    m
}

/// Place the clock (silent placement + kick at time 0) in a fresh world.
pub fn build(spec: &Spec, margin: i32) -> World {
    let (lo, hi) = spec.bounds();
    let min = lo.offset(-margin, -margin, -margin);
    let size = [hi.x - lo.x + 1 + 2 * margin, hi.y - lo.y + 1 + 2 * margin, hi.z - lo.z + 1 + 2 * margin];
    let mut w = World::new(min, size);
    let withheld = match &spec.kick {
        Kick::Last(p) => Some(*p),
        _ => None,
    };
    let silent: Vec<(BlockPos, Block)> = spec.blocks.iter().filter(|(p, _)| Some(*p) != withheld).copied().collect();
    w.place_like_judge(&silent);
    match &spec.kick {
        Kick::None => {}
        Kick::Last(p) => {
            if let Some((_, b)) = spec.blocks.iter().find(|(q, _)| q == p) {
                w.set_block(*p, *b, flags::ALL);
            }
        }
        Kick::Use(p, _) => {
            w.use_block(*p);
        }
        Kick::Temp(p, b, _) => {
            w.set_block(*p, *b, flags::ALL);
        }
    }
    w
}

/// Run a clock: period, warmup, family, waves.
pub fn simulate(spec: &Spec, opts: &Opts) -> Report {
    let (lo, hi) = spec.bounds();
    let mut w = build(spec, opts.margin);
    let kick_done_at: u32 = match &spec.kick {
        Kick::Use(_, Some(t)) => *t,
        Kick::Temp(_, _, t) => *t,
        _ => 0,
    };
    let mut seen: HashMap<u64, u32> = HashMap::new();
    let mut swept = (lo, hi);
    let mut report = Report { verdict: Verdict::NoRepeat, period: 0, warmup: 0, family: vec![], cells: vec![], output: None, blocks: spec.blocks.len(), swept };
    let grow = |sw: &mut (BlockPos, BlockPos), w: &World| {
        for y in 0..w.size[1] {
            for z in 0..w.size[2] {
                for x in 0..w.size[0] {
                    let p = w.min.offset(x, y, z);
                    if !w.get(p).is_air() {
                        sw.0 = BlockPos::new(sw.0.x.min(p.x), sw.0.y.min(p.y), sw.0.z.min(p.z));
                        sw.1 = BlockPos::new(sw.1.x.max(p.x), sw.1.y.max(p.y), sw.1.z.max(p.z));
                    }
                }
            }
        }
    };
    if kick_done_at == 0 {
        seen.insert(state_key(&w), 0);
    }
    let mut found = None;
    for t in 1..=opts.horizon {
        w.tick();
        if t == kick_done_at {
            match &spec.kick {
                Kick::Use(p, _) => {
                    w.use_block(*p);
                }
                Kick::Temp(p, _, _) => {
                    w.set_block(*p, Block::Air, flags::ALL);
                }
                _ => {}
            }
        }
        if let Some(e) = w.errors.first() {
            report.verdict = if e.contains("outside the box") { Verdict::Invalid(format!("escape: {e}")) } else { Verdict::Unmodeled(e.clone()) };
            return report;
        }
        if let Some((p, b)) = w.drops.first() {
            report.verdict = Verdict::Invalid(format!("drop at {p}: {b}"));
            return report;
        }
        if !opts.period_only {
            grow(&mut swept, &w);
        }
        if t < kick_done_at {
            continue;
        }
        if w.idle() {
            report.verdict = Verdict::Stops(t);
            return report;
        }
        let k = state_key(&w);
        if let Some(t0) = seen.get(&k) {
            found = Some((*t0, t));
            break;
        }
        seen.insert(k, t);
    }
    let Some((t0, t1)) = found else {
        return report;
    };
    let period = t1 - t0;
    report.verdict = Verdict::Clock;
    report.period = period;
    report.warmup = t0;
    if opts.period_only {
        return report;
    }
    // one more period: the family, the waves and the swept box
    let mut frames = Vec::with_capacity(period as usize + 1);
    frames.push(snapshot(&w));
    for _ in 0..period {
        w.tick();
        grow(&mut swept, &w);
        frames.push(snapshot(&w));
    }
    report.swept = swept;
    let mut cells: BTreeSet<(i32, i32, i32)> = BTreeSet::new();
    for f in &frames {
        cells.extend(f.keys().copied());
    }
    let mut family: BTreeSet<String> = BTreeSet::new();
    let air = Block::Air;
    let wave_of = |c: (i32, i32, i32)| -> Option<CellWave> {
        let seq: Vec<Block> = frames[1..].iter().map(|f| *f.get(&c).unwrap_or(&air)).collect();
        let first = *frames[0].get(&c).unwrap_or(&air);
        if seq.iter().all(|b| *b == first) {
            return None;
        }
        let mut names: Vec<String> = Vec::new();
        for b in std::iter::once(&first).chain(seq.iter()) {
            if !b.is_air() {
                let n = short(b);
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        let ons: Vec<Option<bool>> = seq.iter().map(on).collect();
        let (on_ticks, rising, widths) = if ons.iter().all(|o| o.is_some()) {
            let v: Vec<bool> = ons.iter().map(|o| o.unwrap()).collect();
            let n = v.len();
            let on_ticks = v.iter().filter(|x| **x).count() as u32;
            let rising: Vec<usize> = (0..n).filter(|&i| v[i] && !v[(i + n - 1) % n]).collect();
            let mut widths = Vec::new();
            for &r in &rising {
                let mut k = 0;
                while k < n && v[(r + k) % n] {
                    k += 1;
                }
                widths.push(k as u32);
            }
            (Some(on_ticks), rising.len() as u32, widths)
        } else {
            (None, 0, vec![])
        };
        Some(CellWave { pos: BlockPos::new(c.0, c.1, c.2), names, on_ticks, rising, widths })
    };
    for c in &cells {
        if let Some(cw) = wave_of(*c) {
            for n in &cw.names {
                family.insert(n.clone());
            }
            report.cells.push(cw);
        }
    }
    if let Some(o) = spec.output {
        report.output = wave_of((o.x, o.y, o.z)).or(Some(CellWave { pos: o, names: vec![], on_ticks: Some(0), rising: 0, widths: vec![] }));
    }
    report.family = family.into_iter().collect();
    report
}

/// One character per block for traces: observers/lamps/bulbs/notes/trapdoors/rails 1/0 (on),
/// pistons E/r, piston heads h, moving blocks ~, air ., anything else #.
pub fn probe_char(w: &World, p: BlockPos) -> char {
    let b = w.get(p);
    match b {
        Block::Air => '.',
        Block::Piston { extended, .. } => {
            if extended { 'E' } else { 'r' }
        }
        Block::PistonHead { .. } => 'h',
        Block::MovingPiston { .. } => '~',
        Block::RedstoneBlock => 'R',
        _ => match on(&b) {
            Some(true) => '1',
            Some(false) => '0',
            None => '#',
        },
    }
}

/// The state of the given cells after each tick 0..=ticks (tick 0 = right after the kick).
pub fn trace_lines(spec: &Spec, ticks: u32, cells: &[BlockPos], margin: i32) -> Vec<String> {
    let mut w = build(spec, margin);
    let line = |w: &World, t: u32| -> String {
        let mut s = format!("t={t:<4}");
        for c in cells {
            s.push(' ');
            s.push(probe_char(w, *c));
        }
        s
    };
    let mut out = vec![line(&w, 0)];
    let kick_done_at: u32 = match &spec.kick {
        Kick::Use(_, Some(t)) => *t,
        Kick::Temp(_, _, t) => *t,
        _ => 0,
    };
    for t in 1..=ticks {
        w.tick();
        if t == kick_done_at {
            match &spec.kick {
                Kick::Use(p, _) => {
                    w.use_block(*p);
                }
                Kick::Temp(p, _, _) => {
                    w.set_block(*p, Block::Air, flags::ALL);
                }
                _ => {}
            }
        }
        out.push(line(&w, t));
    }
    out
}
