//! `mctas door cosearch search`: a joint mechanism + wiring search for flat doors, evaluated in
//! redsim (no JVM in the loop), finds confirmed by the redsim judge (32 positions, FULL seamless,
//! hallway) and then by the real judge (track cosearch).
//!
//! A candidate is a grid of blocks in the door plane (z = door.z): the 9 door cells hold the door
//! blocks, the lever port keeps its lever (it may hang outside the box: the community volume
//! excludes the lever), every other cell of the target box is free for the search. Moves: set a
//! cell to a palette block (air removes), re-face a directional block, change a repeater delay /
//! comparator mode, move or swap blocks, put a redstone block or an observer in front of a piston
//! (a power source the piston carries), move or re-face a piston, toggle a piston between
//! retracted and extended (with its head). Simulated annealing, many independent chains in
//! parallel, restarts from the seeds.
//!
//! Screen (every candidate, one position, `--cycles` lever cycles): partial credit per door cell
//! reaching its target state per phase and per cycle (closed at rest, walkable after opening,
//! back after closing), the 12 ring cells clean in the open state (FULL seamless), the whole box
//! back to rest; then fewer blocks. A candidate with full marks goes to the redsim judge
//! (`cosearch::evaluate`: 32 positions x 3 cycles, FULL, hallway, volume) and, when it passes,
//! is written to `--out-dir` and judged by the JVM (`verify --extra-offsets 28`, seamless tier,
//! hallway).

use crate::cosearch::{self as cs, Pos, RsDoor};
use anyhow::{Context, Result, bail};
use redsim::block::Simple;
use redsim::{Block, BlockPos, Dir, World};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(clap::Args, Clone, Debug)]
pub struct SearchArgs {
    /// Seed designs (flat doors; blocks outside the box are dropped, the lever is kept).
    pub seeds: Vec<std::path::PathBuf>,
    /// Target box in door coordinates (relative to the door's min corner) `x0,y0:x1,y1`,
    /// inclusive, e.g. `-3,-3:4,4` = 8 x 8 for a door at x 0..2, y 0..2.
    #[arg(long = "box")]
    pub bx: String,
    /// Moves only in these cells (door coordinates `x0,y0:x1,y1`, inclusive), default the box.
    #[arg(long)]
    pub focus: Option<String>,
    /// Moves only in these cells (door coordinates `x,y;x,y;...`), overrides --focus.
    #[arg(long)]
    pub focus_cells: Option<String>,
    /// Move the lever: `x,y` in door coordinates (it is re-hung on the block it touches first:
    /// the block above (ceiling lever), below (floor), west or east (wall)).
    #[arg(long)]
    pub lever: Option<String>,
    #[arg(long, default_value_t = 48)]
    pub threads: usize,
    /// Wall-clock limit (seconds).
    #[arg(long, default_value_t = 600)]
    pub secs: u64,
    /// Iterations per annealing run before a restart.
    #[arg(long, default_value_t = 200_000)]
    pub iters: u64,
    /// Start / end temperature (in screen points).
    #[arg(long, default_value_t = 2.0)]
    pub t0: f64,
    #[arg(long, default_value_t = 0.05)]
    pub t1: f64,
    /// Lever cycles in the screen.
    #[arg(long, default_value_t = 2)]
    pub cycles: u32,
    /// Tick cap per phase in the screen.
    #[arg(long, default_value_t = 160)]
    pub max_ticks: u32,
    /// Palette: comma list of families (stone, glass, dust, rblock, torch, walltorch, repeater,
    /// comparator, observer, sticky, piston, lamp, note, target, bulb) or `all`.
    #[arg(long, default_value = "all")]
    pub palette: String,
    /// Lock the mechanism: no piston is added, removed, moved or re-faced (wiring only).
    #[arg(long)]
    pub no_mech: bool,
    /// Keep the seed's pistons where they are (their cells leave the move set); new pistons may
    /// still be added (moving power sources, relays).
    #[arg(long)]
    pub lock_pistons: bool,
    /// Let the lever move too (to any empty cell next to a block, inside or around the box: the
    /// community volume excludes the lever), hung on that block.
    #[arg(long)]
    pub lever_moves: bool,
    /// Ideal assist: the pistons a `door ideal` plan drives with inputs OFF the door plane start
    /// driven by the plan; a move releases (or re-assists) one piston; every assisted piston costs
    /// `--assist-weight` points and a find needs none left. The search wires the door piston by
    /// piston with the rest of the mechanism still moving as planned.
    #[arg(long)]
    pub assist: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = 4.0)]
    pub assist_weight: f64,
    /// Start with only these plan inputs assisted (door coordinates `x,y;x,y`; default all): to
    /// continue from a partial whose other inputs are already wired.
    #[arg(long)]
    pub assist_cells: Option<String>,
    /// Shaping: a working door whose piston schedule the candidates should follow (each piston
    /// extension / retraction of the reference, same cell, within `--ref-slack` ticks, earns
    /// `--ref-weight` points; a perfect screen still only needs the door checks).
    #[arg(long)]
    pub reference: Option<std::path::PathBuf>,
    /// Shaping from a `door ideal` plan instead: its mechanism run in redsim with the plan's
    /// inputs gives the piston events to follow.
    #[arg(long)]
    pub reference_ideal: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = 3)]
    pub ref_slack: u32,
    #[arg(long, default_value_t = 0.5)]
    pub ref_weight: f64,
    #[arg(long, default_value_t = 1)]
    pub seed: u64,
    /// Where finds go.
    #[arg(long, default_value = "designs/piston-door-3x3/tracks/cosearch/finds")]
    pub out_dir: std::path::PathBuf,
    /// Confirm finds with the JVM (verify --extra-offsets 28 + seamless + hallway).
    #[arg(long)]
    pub confirm: bool,
    /// Print progress every N seconds.
    #[arg(long, default_value_t = 30)]
    pub report: u64,
    /// Stop after this many confirmed finds.
    #[arg(long, default_value_t = 4)]
    pub max_finds: usize,
    /// First enumerate every candidate with up to K changed focus cells (K <= 3), then anneal
    /// from the best `--enum-keep` of them (0: anneal from the seeds).
    #[arg(long, default_value_t = 0)]
    pub enum_k: usize,
    #[arg(long, default_value_t = 64)]
    pub enum_keep: usize,
    /// Write the cleaned seeds (blocks outside the box and blocks that pop dropped) here.
    #[arg(long)]
    pub write_seeds: Option<std::path::PathBuf>,
}

// ---------------------------------------------------------------------- the grid

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Grid {
    /// Grid origin (world x, y) and size; z is the door plane.
    pub gx: i32,
    pub gy: i32,
    pub w: i32,
    pub h: i32,
    pub z: i32,
    pub cells: Vec<Block>,
    /// The lever's cell (world x, y); it may move (`--lever-moves`).
    pub lever: (i32, i32),
    /// Pistons still driven by the `--assist` plan (bit i = the plan's piston i).
    pub assist: u64,
}

impl Grid {
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        let (i, j) = (x - self.gx, y - self.gy);
        (i >= 0 && j >= 0 && i < self.w && j < self.h).then(|| (j * self.w + i) as usize)
    }
    pub fn get(&self, x: i32, y: i32) -> Block {
        self.idx(x, y).map(|i| self.cells[i]).unwrap_or(Block::Air)
    }
    pub fn set(&mut self, x: i32, y: i32, b: Block) {
        if let Some(i) = self.idx(x, y) {
            self.cells[i] = b;
        }
    }
    /// The judge's placement order: design blocks sorted by (x, y, z).
    pub fn blocks(&self) -> Vec<(Pos, Block)> {
        let mut v = Vec::new();
        for x in self.gx..self.gx + self.w {
            for y in self.gy..self.gy + self.h {
                let b = self.get(x, y);
                if !b.is_air() {
                    v.push(([x, y, self.z], b));
                }
            }
        }
        v
    }
    pub fn count(&self) -> i32 {
        self.cells.iter().filter(|b| !b.is_air() && !matches!(b, Block::PistonHead { .. })).count() as i32
    }
}

/// The search problem: geometry and constraints.
#[derive(Clone)]
pub struct Problem {
    pub spec: crate::DoorSpec,
    /// Box (world coordinates, inclusive) the build must stay in (the lever excepted).
    pub lo: [i32; 2],
    pub hi: [i32; 2],
    /// Cells moves may change.
    pub free: Vec<(i32, i32)>,
    pub door_cells: Vec<(i32, i32)>,
    pub ring: Vec<(i32, i32)>,
    pub door_block: Block,
    pub palette: Vec<Block>,
    pub mech: bool,
    pub cycles: u32,
    pub max_ticks: u32,
    /// Reference piston events (phase 0 open / 1 close, extending?, cell, tick after the flip).
    pub reference: Vec<Event>,
    pub ref_slack: u32,
    pub ref_weight: f64,
    pub lever_moves: bool,
    /// `--assist`: the plan's driven pistons (cells) and its input events per phase (tick,
    /// piston index, input cell, state set there).
    pub assist_pistons: Vec<(i32, i32)>,
    pub assist_events: [Vec<(u32, usize, BlockPos, Block)>; 2],
    pub assist_weight: f64,
    /// Plan input cells IN the door plane: reserved while their input is assisted.
    pub assist_in_plane: HashSet<(i32, i32)>,
}

/// A piston starting to extend or retract: (phase, extending, cell, tick).
pub type Event = (u8, bool, (i32, i32), u32);

/// Piston events of one tick: a base turning extended (extension starts) or a base cell turning
/// into a moving block while its piston was extended (retraction starts).
fn tick_events(prev: &[(BlockPos, Block)], w: &World, phase: u8, t: u32, out: &mut Vec<Event>) {
    for (q, b) in prev {
        let now = w.get(*q);
        match (b, now) {
            (Block::Piston { extended: false, .. }, Block::Piston { extended: true, .. }) => out.push((phase, true, (q.x, q.y), t)),
            (Block::Piston { extended: true, .. }, Block::MovingPiston { .. }) | (Block::Piston { extended: true, .. }, Block::Piston { extended: false, .. }) => {
                out.push((phase, false, (q.x, q.y), t))
            }
            _ => {}
        }
    }
}

fn piston_cells(w: &World, cells: &[BlockPos]) -> Vec<(BlockPos, Block)> {
    cells.iter().filter_map(|q| {
        let b = w.get(*q);
        matches!(b, Block::Piston { .. }).then_some((*q, b))
    }).collect()
}

/// One lever cycle of a door at its own position: its piston events.
fn piston_events(p: &Problem, blocks: &[(Pos, Block)], spec: crate::DoorSpec) -> Vec<Event> {
    let (wlo, whi) = cs::watch_box(blocks, &spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
    let placed: Vec<(BlockPos, Block)> = blocks.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
    w.place_like_judge(&placed);
    cs::step_until_idle(&mut w, p.max_ticks, |_| {});
    let cells = plane_cells_wide(&w, spec.door[2]);
    let mut out = Vec::new();
    for phase in 0..2u8 {
        w.use_block(cs::bp(spec.lever));
        let mut t = 0;
        let mut prev = piston_cells(&w, &cells);
        cs::step_until_idle(&mut w, p.max_ticks, |w| {
            t += 1;
            tick_events(&prev, w, phase, t, &mut out);
            prev = piston_cells(w, &cells);
        });
    }
    out
}

/// The piston events of a `door ideal` plan run in redsim (its design at rest, the plan's inputs
/// applied between ticks like `door ideal` does: setBlock with updates, `use` = right-click).
fn ideal_events(p: &Problem, path: &std::path::Path) -> Result<Vec<Event>> {
    let text = std::fs::read_to_string(path)?;
    let s = crate::ideal::Schedule::parse(&text)?;
    let d = s.design();
    let door = RsDoor::from_design(d).map_err(|e| anyhow::anyhow!(e)).or_else(|_| -> Result<RsDoor> {
        // a mechanism file without a lever port
        let mut blocks = Vec::new();
        for (q, st) in d.block_list() {
            blocks.push((q, cs::convert_state(&st).map_err(|e| anyhow::anyhow!(e))?));
        }
        Ok(RsDoor { spec: p.spec, blocks })
    })?;
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    let mut all: Vec<Pos> = door.blocks.iter().map(|(q, _)| *q).collect();
    for open in [true, false] {
        all.extend(s.resolved(open)?.iter().map(|e| e.1));
    }
    all.extend(p.spec.door_cells());
    for q in &all {
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    let mut w = World::new(BlockPos::new(lo[0] - 4, lo[1] - 4, lo[2] - 4), [hi[0] - lo[0] + 9, hi[1] - lo[1] + 9, hi[2] - lo[2] + 9]);
    let placed: Vec<(BlockPos, Block)> = door.blocks.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
    w.place_like_judge(&placed);
    cs::step_until_idle(&mut w, 400, |_| {});
    let cells = plane_cells_wide(&w, p.spec.door[2]);
    let mut out = Vec::new();
    for (phase, open) in [(0u8, true), (1u8, false)] {
        let evs = s.resolved(open)?;
        let mut t = 0u32;
        let mut i = 0;
        let mut prev = piston_cells(&w, &cells);
        loop {
            while i < evs.len() && evs[i].0 == t {
                let (_, q, st) = &evs[i];
                match st.as_deref() {
                    Some("@use") => {
                        w.use_block(cs::bp(*q));
                    }
                    Some(x) => {
                        let b = cs::convert_state(x).map_err(|e| anyhow::anyhow!(e))?;
                        w.set_block_and_update(cs::bp(*q), b);
                    }
                    None => {
                        w.set_block_and_update(cs::bp(*q), Block::Air);
                    }
                }
                i += 1;
            }
            w.tick();
            t += 1;
            tick_events(&prev, &w, phase, t, &mut out);
            prev = piston_cells(&w, &cells);
            if (i >= evs.len() && w.idle()) || t > 600 {
                break;
            }
        }
    }
    Ok(out)
}

fn plane_cells_wide(w: &World, z: i32) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for y in w.min.y..w.min.y + w.size[1] {
        for x in w.min.x..w.min.x + w.size[0] {
            v.push(BlockPos::new(x, y, z));
        }
    }
    v
}

/// How many reference events a run's events match (same phase, kind and cell, within slack).
fn match_events(reference: &[Event], got: &[Event], slack: u32) -> usize {
    let mut used = vec![false; got.len()];
    let mut n = 0;
    for r in reference {
        let mut best: Option<(usize, u32)> = None;
        for (i, g) in got.iter().enumerate() {
            if used[i] || g.0 != r.0 || g.1 != r.1 || g.2 != r.2 {
                continue;
            }
            let d = g.3.abs_diff(r.3);
            if d <= slack && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        if let Some((i, _)) = best {
            used[i] = true;
            n += 1;
        }
    }
    n
}

fn parse_box(s: &str) -> Result<([i32; 2], [i32; 2])> {
    let (a, b) = s.split_once(':').context("box: x0,y0:x1,y1")?;
    let p = |t: &str| -> Result<[i32; 2]> {
        let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
        anyhow::ensure!(v.len() == 2, "box corner {t:?}: x,y");
        Ok([v[0], v[1]])
    };
    let (a, b) = (p(a)?, p(b)?);
    Ok(([a[0].min(b[0]), a[1].min(b[1])], [a[0].max(b[0]), a[1].max(b[1])]))
}

pub fn palette(spec: &str) -> Result<Vec<Block>> {
    let all = "stone,glass,dust,rblock,torch,walltorch,repeater,comparator,observer,sticky,piston,lamp,note,target,bulb,obsidian,trapdoor,dropper,slab,leaves,log,glazed";
    let spec = if spec == "all" { all } else { spec };
    let mut out = vec![Block::Air];
    for fam in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let states: Vec<String> = match fam {
            "stone" => vec!["stone".into(), "smooth_stone".into()],
            "glass" => vec!["glass".into()],
            "dust" => vec!["redstone_wire".into()],
            "rblock" => vec!["redstone_block".into()],
            "torch" => vec!["redstone_torch".into()],
            "walltorch" => vec!["redstone_wall_torch[facing=east]".into(), "redstone_wall_torch[facing=west]".into()],
            "repeater" => (1..=4).flat_map(|d| ["east", "west"].map(|f| format!("repeater[facing={f},delay={d}]"))).collect(),
            "comparator" => ["east", "west"].iter().flat_map(|f| [format!("comparator[facing={f}]"), format!("comparator[facing={f},mode=subtract]")]).collect(),
            "observer" => ["up", "down", "east", "west"].map(|f| format!("observer[facing={f}]")).to_vec(),
            "sticky" => ["up", "down", "east", "west"].map(|f| format!("sticky_piston[facing={f}]")).to_vec(),
            "piston" => ["up", "down", "east", "west"].map(|f| format!("piston[facing={f}]")).to_vec(),
            "lamp" => vec!["redstone_lamp".into()],
            "note" => vec!["note_block".into()],
            "target" => vec!["target".into()],
            "bulb" => vec!["waxed_copper_bulb".into()],
            // immovable conductors (pistons cannot push them)
            "obsidian" => vec!["obsidian".into(), "crying_obsidian".into()],
            // iron trapdoor: opens while powered (a non-conductor component; in the door plane it
            // faces east or west, bottom or top half)
            "trapdoor" => ["east", "west"].iter().flat_map(|f| ["bottom", "top"].map(|h| format!("iron_trapdoor[facing={f},half={h}]"))).collect(),
            "dropper" => ["up", "down", "east", "west"].map(|f| format!("dropper[facing={f}]")).to_vec(),
            // non-conductor floors: a top slab holds dust, a bottom slab does not
            "slab" => vec!["smooth_stone_slab[type=bottom]".into(), "smooth_stone_slab[type=top]".into()],
            // persistent leaves: their distance follows a log moved next to them (1 gt delay)
            "leaves" => vec!["oak_leaves[persistent=true,distance=7]".into()],
            "log" => vec!["oak_log[axis=x]".into(), "oak_log[axis=y]".into()],
            // pushed but never pulled (sticky pistons leave it, slime cannot drag it)
            "glazed" => ["north", "east", "south", "west"].map(|f| format!("white_glazed_terracotta[facing={f}]")).to_vec(),
            f => bail!("unknown palette family {f:?} ({all})"),
        };
        for s in states {
            out.push(Block::parse(&s).map_err(|e| anyhow::anyhow!(e))?);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------- the screen

/// Screen result: points (higher is better), the reference-schedule shaping, then fewer blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub pts: i32,
    pub max: i32,
    pub blocks: i32,
    /// Reference piston events matched, and their weight.
    pub ev: i32,
    pub ev_w: f64,
    /// Pistons still driven by the assist plan, and their cost.
    pub assisted: i32,
    pub asw: f64,
}

impl Score {
    fn energy(&self) -> f64 {
        -(self.pts as f64) - self.ev_w * self.ev as f64 + 0.02 * self.blocks as f64 + self.asw * self.assisted as f64
    }
    pub fn perfect(&self) -> bool {
        self.pts >= self.max && self.assisted == 0
    }
}

fn in_box(p: &Problem, x: i32, y: i32) -> bool {
    x >= p.lo[0] && x <= p.hi[0] && y >= p.lo[1] && y <= p.hi[1]
}

/// One position (the door as written), `p.cycles` lever cycles: partial credit (+ the
/// reference-schedule shaping when there is a reference).
pub fn screen(p: &Problem, g: &Grid) -> Score {
    let mut evs = Vec::new();
    let mut s = screen_core(p, g, &mut evs);
    if !p.reference.is_empty() {
        s.ev = match_events(&p.reference, &evs, p.ref_slack) as i32;
        s.ev_w = p.ref_weight;
    }
    if !p.assist_pistons.is_empty() {
        s.assisted = g.assist.count_ones() as i32;
        s.asw = p.assist_weight;
    }
    s
}

/// One phase after a flip: the assisted pistons' plan inputs applied at their ticks (before the
/// tick, like `door ideal`), ticking until idle (at least once) and until every input was
/// applied; `on_tick` sees the world after every tick. Returns whether it went idle.
fn run_phase(p: &Problem, g: &Grid, w: &mut World, phase: usize, mut on_tick: impl FnMut(&World)) -> bool {
    let evs = &p.assist_events[phase];
    let mut i = 0;
    let mut t = 0u32;
    loop {
        while i < evs.len() && evs[i].0 == t {
            let (_, k, pos, b) = evs[i];
            if (g.assist >> k) & 1 == 1 {
                w.set_block_and_update(pos, b);
            }
            i += 1;
        }
        w.tick();
        t += 1;
        on_tick(w);
        if i >= evs.len() && w.idle() {
            return true;
        }
        if t >= p.max_ticks {
            return false;
        }
    }
}

fn screen_core(p: &Problem, g: &Grid, evs: &mut Vec<Event>) -> Score {
    let max = 11 + p.cycles as i32 * 34;
    let blocks = g.count();
    let mut pts = 0;
    let list = g.blocks();
    let spec = p.spec;
    let (wlo, whi) = cs::watch_box(&list, &spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
    let placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
    w.place_like_judge(&placed);
    let (_, idle) = cs::step_until_idle(&mut w, p.max_ticks, |_| {});
    if !idle || !w.errors.is_empty() {
        return Score { pts: 0, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
    }
    if !w.drops.is_empty() {
        // something popped at placement (a dust or a repeater without a floor...)
        return Score { pts: 1, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
    }
    let z = g.z;
    let at = |x: i32, y: i32| BlockPos::new(x, y, z);
    // rest
    let door_ok = p.door_cells.iter().filter(|(x, y)| w.get(at(*x, *y)) == p.door_block).count() as i32;
    pts += door_ok;
    let mut unsupported = 0;
    for (q, b) in &list {
        let s = w.get(cs::bp(*q));
        let _ = b;
        if !s.is_air() && !redsim::behavior::can_survive(&w, &s, cs::bp(*q)) {
            unsupported += 1;
        }
    }
    pts -= 2 * unsupported;
    let lever = BlockPos::new(g.lever.0, g.lever.1, g.z);
    if !matches!(w.get(lever), Block::Lever { powered: false, .. }) {
        return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
    }
    pts += 1;
    if door_ok < 9 || unsupported > 0 {
        return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
    }
    pts += 1;
    // the plane's cells (the box + 1) compared after each close
    let rest: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
    let rec_cells = if p.reference.is_empty() { Vec::new() } else { plane_cells_wide(&w, z) };
    for cycle in 0..p.cycles {
        w.use_block(lever);
        let rec = cycle == 0 && !rec_cells.is_empty();
        let mut prev = if rec { piston_cells(&w, &rec_cells) } else { Vec::new() };
        let mut t = 0;
        let idle = run_phase(p, g, &mut w, 0, |w| {
            if rec {
                t += 1;
                tick_events(&prev, w, 0, t, evs);
                prev = piston_cells(w, &rec_cells);
            }
        });
        let open_n = p.door_cells.iter().filter(|(x, y)| {
            let s = w.get(at(*x, *y));
            !cs::collides(&s) && !cs::is_moving(&s)
        }).count() as i32;
        pts += open_n;
        if open_n >= 7 {
            // the ring: only air, door blocks or quartz-like blocks (FULL seamless, open)
            let clean = p.ring.iter().filter(|(x, y)| {
                let s = w.get(at(*x, *y));
                s.is_air() || (cs::quartz_like(&s) && !cs::is_moving(&s))
            }).count() as i32;
            pts += clean;
            // nothing of the build outside the box
            let outside = plane_cells(p, z).iter().filter(|q| !in_box(p, q.x, q.y) && *q != &lever && !w.get(**q).is_air()).count() as i32;
            pts -= outside;
        }
        if !idle || !w.drops.is_empty() || !w.errors.is_empty() {
            return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
        }
        pts += 1;
        w.use_block(lever);
        let mut prev = if rec { piston_cells(&w, &rec_cells) } else { Vec::new() };
        let mut t = 0;
        let idle = run_phase(p, g, &mut w, 1, |w| {
            if rec {
                t += 1;
                tick_events(&prev, w, 1, t, evs);
                prev = piston_cells(w, &rec_cells);
            }
        });
        let back = p.door_cells.iter().filter(|(x, y)| w.get(at(*x, *y)) == p.door_block).count() as i32;
        pts += back;
        if !idle || !w.drops.is_empty() || !w.errors.is_empty() {
            return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
        }
        let now: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
        let same = now.iter().zip(rest.iter()).filter(|(a, b)| a == b).count() as i32;
        let n = rest.len() as i32;
        if same == n {
            pts += 3;
        } else {
            pts -= (n - same).min(3);
            return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 };
        }
    }
    Score { pts, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0 }
}

fn plane_cells(p: &Problem, z: i32) -> Vec<BlockPos> {
    let mut v = Vec::new();
    for y in p.lo[1] - 2..=p.hi[1] + 2 {
        for x in p.lo[0] - 2..=p.hi[0] + 2 {
            v.push(BlockPos::new(x, y, z));
        }
    }
    v
}

// ---------------------------------------------------------------------- moves

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
    fn f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn is_piston(b: &Block) -> bool {
    matches!(b, Block::Piston { .. })
}

fn head_of(b: &Block, x: i32, y: i32) -> Option<(i32, i32)> {
    match *b {
        Block::Piston { facing, extended: true, .. } => {
            let s = facing.step();
            Some((x + s[0], y + s[1]))
        }
        _ => None,
    }
}

/// Clear a cell (a piston takes its head along; a head is never cleared alone).
fn clear(g: &mut Grid, x: i32, y: i32) -> bool {
    let b = g.get(x, y);
    if matches!(b, Block::PistonHead { .. }) {
        return false;
    }
    if let Some((hx, hy)) = head_of(&b, x, y) {
        g.set(hx, hy, Block::Air);
    }
    g.set(x, y, Block::Air);
    true
}

const PLANE_DIRS: [Dir; 4] = [Dir::Up, Dir::Down, Dir::East, Dir::West];

fn refaced(b: &Block, rng: &mut Rng) -> Option<Block> {
    let f = PLANE_DIRS[rng.below(4)];
    let h = if rng.below(2) == 0 { Dir::East } else { Dir::West };
    Some(match *b {
        Block::Observer { powered, .. } => Block::Observer { facing: f, powered },
        Block::Piston { sticky, extended: false, .. } => Block::Piston { sticky, facing: f, extended: false },
        Block::Repeater { delay, locked, powered, .. } => Block::Repeater { facing: h, delay, locked, powered },
        Block::Comparator { subtract, powered, .. } => Block::Comparator { facing: h, subtract, powered },
        Block::WallTorch { lit, .. } => Block::WallTorch { facing: h, lit },
        Block::Dispenser { dropper, triggered, .. } => Block::Dispenser { dropper, facing: f, triggered },
        Block::Trapdoor { iron, top, open, powered, .. } => {
            if rng.below(2) == 0 {
                Block::Trapdoor { iron, facing: h, top, open, powered }
            } else {
                Block::Trapdoor { iron, facing: h, top: !top, open, powered }
            }
        }
        Block::Slab { kind } if kind < 2 => Block::Slab { kind: 1 - kind },
        Block::Log { axis } => Block::Log { axis: if axis == 1 { 0 } else { 1 } },
        Block::Glazed { .. } => Block::Glazed { facing: [Dir::North, Dir::East, Dir::South, Dir::West][rng.below(4)] },
        Block::Simple(Simple::Obsidian) => Block::Simple(Simple::CryingObsidian),
        Block::Simple(Simple::CryingObsidian) => Block::Simple(Simple::Obsidian),
        _ => return None,
    })
}

/// The update-piston proposal (vjeux): a normal or sticky piston facing an EMPTY free cell,
/// placed so its base or head cell touches a sensor: a piston's quasi cell (the cell above it), an
/// observer's watched cell, a dropper or a note block. Its extension / retraction is then a block
/// update (a BUD trigger) or an observer pulse.
fn update_piston(p: &Problem, g: &mut Grid, rng: &mut Rng) -> Option<(i32, i32)> {
    let mut sensors: Vec<(i32, i32)> = Vec::new();
    for &(x, y) in &p.free {
        match g.get(x, y) {
            Block::Piston { .. } => sensors.push((x, y + 1)),
            Block::Observer { facing, .. } => {
                let s = facing.step();
                sensors.push((x + s[0], y + s[1]));
            }
            Block::Dispenser { .. } | Block::Note { .. } => sensors.push((x, y)),
            _ => {}
        }
    }
    // the seed's locked pistons count too
    for x in g.gx..g.gx + g.w {
        for y in g.gy..g.gy + g.h {
            if matches!(g.get(x, y), Block::Piston { .. }) && !p.free.contains(&(x, y)) {
                sensors.push((x, y + 1));
            }
        }
    }
    if sensors.is_empty() {
        return None;
    }
    let (sx, sy) = sensors[rng.below(sensors.len())];
    // the piston's base (b) or head (h) cell next to the sensor
    let d = PLANE_DIRS[rng.below(4)].step();
    let f = PLANE_DIRS[rng.below(4)];
    let fs = f.step();
    let (bx, by) = if rng.below(2) == 0 {
        (sx + d[0], sy + d[1])
    } else {
        // head next to the sensor: base one step back
        (sx + d[0] - fs[0], sy + d[1] - fs[1])
    };
    let (hx, hy) = (bx + fs[0], by + fs[1]);
    let ok = |x: i32, y: i32| p.free.contains(&(x, y)) && g.get(x, y).is_air() && (x, y) != g.lever && !reserved(p, g, x, y);
    if !ok(bx, by) || !ok(hx, hy) || p.ring.contains(&(hx, hy)) {
        return None;
    }
    g.set(bx, by, Block::Piston { sticky: rng.below(2) == 0, facing: f, extended: false });
    Some((bx, by))
}

/// Move kinds (for the improvement statistics): cell edit, assist toggle, lever move, update piston.
pub const MOVE_KINDS: [&str; 4] = ["cell", "assist", "lever", "update-piston"];

/// One random move at `at` (or a random focus cell). Returns the cell when something changed.
fn mutate(p: &Problem, g: &mut Grid, rng: &mut Rng, at: Option<(i32, i32)>, kind: &mut usize) -> Option<(i32, i32)> {
    *kind = 0;
    if p.mech && at.is_none() && rng.below(8) == 0 {
        *kind = 3;
        return update_piston(p, g, rng);
    }
    if !p.assist_pistons.is_empty() && at.is_none() && rng.below(10) == 0 {
        // release (or re-assist) one planned input
        *kind = 1;
        let k = rng.below(p.assist_pistons.len());
        let (cx, cy) = p.assist_pistons[k];
        if (g.assist >> k) & 1 == 0 && p.assist_in_plane.contains(&(cx, cy)) && !g.get(cx, cy).is_air() {
            // re-assisting an in-plane input needs its cell free again
            return None;
        }
        g.assist ^= 1u64 << k;
        return Some(p.assist_pistons[k]);
    }
    if p.lever_moves && at.is_none() && rng.below(25) == 0 {
        *kind = 2;
        return move_lever(p, g, rng).then_some(g.lever);
    }
    let (x, y) = match at {
        Some(c) if p.free.contains(&c) => c,
        _ => p.free[rng.below(p.free.len())],
    };
    if mutate_cell(p, g, rng, x, y) { Some((x, y)) } else { None }
}

/// An in-plane plan input cell whose input is still assisted: no block may go there.
fn reserved(p: &Problem, g: &Grid, x: i32, y: i32) -> bool {
    if p.assist_in_plane.is_empty() || !p.assist_in_plane.contains(&(x, y)) {
        return false;
    }
    p.assist_pistons.iter().position(|c| *c == (x, y)).is_some_and(|k| (g.assist >> k) & 1 == 1)
}

fn mutate_cell(p: &Problem, g: &mut Grid, rng: &mut Rng, x: i32, y: i32) -> bool {
    let cur = g.get(x, y);
    if (x, y) == g.lever || reserved(p, g, x, y) {
        return false;
    }
    if matches!(cur, Block::PistonHead { .. }) {
        return false;
    }
    if is_piston(&cur) && !p.mech {
        return false;
    }
    let r = rng.below(100);
    if r < 40 {
        // set to a palette block (air = remove)
        let b = p.palette[rng.below(p.palette.len())];
        if is_piston(&b) && !p.mech {
            return false;
        }
        if b == cur {
            return false;
        }
        if !clear(g, x, y) {
            return false;
        }
        g.set(x, y, b);
        true
    } else if r < 55 {
        // re-face / re-delay / mode
        let nb = match cur {
            Block::Repeater { facing, delay, locked, powered } if rng.below(2) == 0 => {
                Block::Repeater { facing, delay: 1 + ((delay as usize + rng.below(3)) % 4) as u8, locked, powered }
            }
            Block::Comparator { facing, subtract, powered } if rng.below(2) == 0 => Block::Comparator { facing, subtract: !subtract, powered },
            _ => match refaced(&cur, rng) {
                Some(b) => b,
                None => return false,
            },
        };
        if nb == cur {
            return false;
        }
        g.set(x, y, nb);
        true
    } else if r < 70 {
        // move one cell (pistons move with their head only when retracted)
        if cur.is_air() || head_of(&cur, x, y).is_some() {
            return false;
        }
        let d = PLANE_DIRS[rng.below(4)].step();
        let (nx, ny) = (x + d[0], y + d[1]);
        if !p.free.contains(&(nx, ny)) || !g.get(nx, ny).is_air() || reserved(p, g, nx, ny) {
            return false;
        }
        g.set(x, y, Block::Air);
        g.set(nx, ny, cur);
        true
    } else if r < 80 {
        // swap with a neighbour
        let d = PLANE_DIRS[rng.below(4)].step();
        let (nx, ny) = (x + d[0], y + d[1]);
        let other = g.get(nx, ny);
        if !p.free.contains(&(nx, ny)) || reserved(p, g, nx, ny) || other == cur || head_of(&cur, x, y).is_some() || head_of(&other, nx, ny).is_some() || matches!(other, Block::PistonHead { .. }) {
            return false;
        }
        if !p.mech && (is_piston(&cur) || is_piston(&other)) {
            return false;
        }
        if (nx, ny) == g.lever {
            return false;
        }
        g.set(x, y, other);
        g.set(nx, ny, cur);
        true
    } else if r < 90 {
        // a power source a piston carries: a redstone block or an observer in front of a piston
        let pistons: Vec<(i32, i32, Dir)> = p
            .free
            .iter()
            .filter_map(|&(px, py)| match g.get(px, py) {
                Block::Piston { facing, extended: false, .. } => Some((px, py, facing)),
                _ => None,
            })
            .collect();
        if pistons.is_empty() {
            return false;
        }
        let (px, py, f) = pistons[rng.below(pistons.len())];
        let s = f.step();
        let k = 1 + rng.below(2) as i32;
        let (cx, cy) = (px + s[0] * k, py + s[1] * k);
        if !p.free.contains(&(cx, cy)) || !g.get(cx, cy).is_air() || reserved(p, g, cx, cy) {
            return false;
        }
        let b = if rng.below(2) == 0 { Block::RedstoneBlock } else { Block::Observer { facing: PLANE_DIRS[rng.below(4)], powered: false } };
        g.set(cx, cy, b);
        true
    } else {
        // toggle a piston extended / retracted (a level piston held out at rest)
        if !p.mech {
            return false;
        }
        match cur {
            Block::Piston { sticky, facing, extended: false } => {
                let s = facing.step();
                let (hx, hy) = (x + s[0], y + s[1]);
                if !p.free.contains(&(hx, hy)) || !g.get(hx, hy).is_air() || reserved(p, g, hx, hy) {
                    return false;
                }
                g.set(x, y, Block::Piston { sticky, facing, extended: true });
                g.set(hx, hy, Block::PistonHead { sticky, facing, short: false });
                true
            }
            Block::Piston { sticky, facing, extended: true } => {
                let s = facing.step();
                g.set(x + s[0], y + s[1], Block::Air);
                g.set(x, y, Block::Piston { sticky, facing, extended: false });
                true
            }
            _ => false,
        }
    }
}

// ---------------------------------------------------------------------- setup

fn grid_from_door(door: &RsDoor, p: &Problem) -> Result<Grid> {
    let z = door.spec.door[2];
    let gx = p.lo[0] - 2;
    let gy = p.lo[1] - 2;
    let w = p.hi[0] - p.lo[0] + 5;
    let h = p.hi[1] - p.lo[1] + 5;
    let mut g = Grid { gx, gy, w, h, z, cells: vec![Block::Air; (w * h) as usize], lever: (door.spec.lever[0], door.spec.lever[1]), assist: 0 };
    let lever = door.spec.lever;
    for (q, b) in &door.blocks {
        anyhow::ensure!(q[2] == z, "not flat: {q:?} (an off-plane lever is not supported by the search grid yet)");
        let keep = in_box(p, q[0], q[1]) || *q == lever;
        if keep {
            g.set(q[0], q[1], *b);
        }
    }
    // a piston whose head was dropped (outside the box) is retracted; a head without its piston goes
    for x in gx..gx + w {
        for y in gy..gy + h {
            let b = g.get(x, y);
            if let Some((hx, hy)) = head_of(&b, x, y)
                && !matches!(g.get(hx, hy), Block::PistonHead { .. })
                && let Block::Piston { sticky, facing, .. } = b
            {
                g.set(x, y, Block::Piston { sticky, facing, extended: false });
            }
            if let Block::PistonHead { facing, .. } = b {
                let s = facing.step();
                if !matches!(g.get(x - s[0], y - s[1]), Block::Piston { extended: true, .. }) {
                    g.set(x, y, Block::Air);
                }
            }
        }
    }
    Ok(g)
}

/// Re-hang the lever at (x, y): on the first sturdy neighbour (above, below, west, east).
fn place_lever(g: &mut Grid, x: i32, y: i32) {
    use redsim::block::Face;
    let lever = if !g.get(x, y + 1).is_air() {
        Block::Lever { face: Face::Ceiling, facing: Dir::West, powered: false }
    } else if !g.get(x, y - 1).is_air() {
        Block::Lever { face: Face::Floor, facing: Dir::West, powered: false }
    } else if !g.get(x - 1, y).is_air() {
        Block::Lever { face: Face::Wall, facing: Dir::East, powered: false }
    } else {
        Block::Lever { face: Face::Wall, facing: Dir::West, powered: false }
    };
    g.set(x, y, lever);
    g.lever = (x, y);
}

/// Move the lever to a random empty cell next to a block (inside the box or around it: the
/// community volume excludes the lever), hung on that block.
fn move_lever(p: &Problem, g: &mut Grid, rng: &mut Rng) -> bool {
    let (x, y) = (rng.below((p.hi[0] - p.lo[0] + 3) as usize) as i32 + p.lo[0] - 1, rng.below((p.hi[1] - p.lo[1] + 3) as usize) as i32 + p.lo[1] - 1);
    if (x, y) == g.lever || !g.get(x, y).is_air() || p.door_cells.contains(&(x, y)) {
        return false;
    }
    let solid = |b: Block| !b.is_air() && !matches!(b, Block::Lever { .. } | Block::Wire { .. } | Block::Torch { .. } | Block::WallTorch { .. } | Block::PistonHead { .. });
    if ![(0, 1), (0, -1), (1, 0), (-1, 0)].iter().any(|(dx, dy)| solid(g.get(x + dx, y + dy))) {
        return false;
    }
    let (ox, oy) = g.lever;
    g.set(ox, oy, Block::Air);
    place_lever(g, x, y);
    true
}

/// Drop the blocks that pop at placement (a dust or a repeater whose floor was cut away).
fn clean_seed(p: &Problem, g: &mut Grid) {
    for _ in 0..6 {
        let list = g.blocks();
        let (wlo, whi) = cs::watch_box(&list, &p.spec);
        let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
        let placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
        w.place_like_judge(&placed);
        cs::step_until_idle(&mut w, p.max_ticks, |_| {});
        if w.drops.is_empty() {
            return;
        }
        for (q, _) in &w.drops {
            if q.z == g.z && (q.x, q.y) != (p.spec.lever[0], p.spec.lever[1]) {
                clear(g, q.x, q.y);
            }
        }
    }
}

/// Every candidate with at most `k` changed focus cells (each set to a palette block or air),
/// screened; returns the best `keep` (score, grid), best first.
fn enumerate(p: &Problem, seed: &Grid, k: usize, keep: usize, threads: usize, sh: &Shared, jopts: &cs::JudgeOpts, a: &SearchArgs) -> Vec<(Score, Grid)> {
    let cells: Vec<(i32, i32)> = p.free.iter().copied().filter(|(x, y)| !matches!(seed.get(*x, *y), Block::PistonHead { .. })).collect();
    let pal: Vec<Block> = p.palette.iter().copied().filter(|b| p.mech || !is_piston(b)).collect();
    let best: Mutex<Vec<(Score, Grid)>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    let n = cells.len();
    let consider = |g: &Grid, local: &mut Vec<(Score, Grid)>| {
        let s = screen(p, g);
        sh.evals.fetch_add(1, Ordering::Relaxed);
        if s.perfect() {
            sh.perfect.fetch_add(1, Ordering::Relaxed);
            try_find(p, g, sh, jopts, a, None);
        }
        local.push((s, g.clone()));
        if local.len() > 4 * keep {
            local.sort_by(|x, y| x.0.energy().partial_cmp(&y.0.energy()).unwrap());
            local.truncate(keep);
        }
    };
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut local: Vec<(Score, Grid)> = Vec::new();
                loop {
                    let wi = next.fetch_add(1, Ordering::Relaxed);
                    if wi >= n * pal.len() || sh.stop.load(Ordering::Relaxed) {
                        break;
                    }
                    // one work item = the first changed cell and its new block
                    let (i, bi) = (wi / pal.len(), wi % pal.len());
                    let (x1, y1) = cells[i];
                    {
                        let b1 = &pal[bi];
                        let mut g1 = seed.clone();
                        if g1.get(x1, y1) == *b1 || !clear(&mut g1, x1, y1) {
                            continue;
                        }
                        g1.set(x1, y1, *b1);
                        consider(&g1, &mut local);
                        if k >= 2 {
                            enum_rec(&g1, &cells, i + 1, k - 1, &pal, sh, &mut |g| consider(g, &mut local));
                        }
                    }
                }
                let mut b = best.lock().unwrap();
                b.extend(local);
                b.sort_by(|x, y| x.0.energy().partial_cmp(&y.0.energy()).unwrap());
                b.dedup_by(|x, y| x.1 == y.1);
                b.truncate(keep);
            });
        }
    });
    best.into_inner().unwrap()
}

/// Every way to change `k` more cells among `cells[start..]` (each to a palette block other than
/// its current one), depth first.
fn enum_rec(g: &Grid, cells: &[(i32, i32)], start: usize, k: usize, pal: &[Block], sh: &Shared, consider: &mut dyn FnMut(&Grid)) {
    for j in start..cells.len() {
        if sh.stop.load(Ordering::Relaxed) {
            return;
        }
        let (x, y) = cells[j];
        for b in pal {
            let mut g2 = g.clone();
            if g2.get(x, y) == *b || matches!(g2.get(x, y), Block::PistonHead { .. }) || !clear(&mut g2, x, y) {
                continue;
            }
            g2.set(x, y, *b);
            consider(&g2);
            if k >= 2 {
                enum_rec(&g2, cells, j + 1, k - 1, pal, sh, consider);
            }
        }
    }
}

/// A perfect screen: judge it in redsim (32 positions, FULL, hallway); a pass is a find.
fn try_find(p: &Problem, g: &Grid, sh: &Shared, jopts: &cs::JudgeOpts, a: &SearchArgs, tx: Option<&std::sync::mpsc::Sender<(std::path::PathBuf, mctas::design::Design)>>) {
    let h = grid_hash(g);
    if !sh.seen.lock().unwrap().insert(h) {
        return;
    }
    sh.full_evals.fetch_add(1, Ordering::Relaxed);
    let door = door_of(g, p.spec);
    let e = cs::evaluate(&door, jopts, false);
    if e.record_ok() {
        let k = sh.finds.fetch_add(1, Ordering::Relaxed);
        let vol = e.volume.map(|v| v.0).unwrap_or(0);
        let name = format!("cosearch-{vol}b-{}-{k}", door.block_count());
        let d = door.to_design(&name);
        let path = a.out_dir.join(format!("{name}.txt"));
        let _ = d.save(&path);
        let msg = format!("redsim find: {}", cs::eval_line(&name, &e, 0.0));
        println!("{msg}");
        sh.log.lock().unwrap().push(msg);
        if let Some(tx) = tx {
            let _ = tx.send((path, d));
        }
        if k + 1 >= a.max_finds {
            sh.stop.store(true, Ordering::Relaxed);
        }
    }
}

fn door_of(g: &Grid, spec: crate::DoorSpec) -> RsDoor {
    let mut spec = spec;
    spec.lever = [g.lever.0, g.lever.1, g.z];
    RsDoor { spec, blocks: g.blocks() }
}

// ---------------------------------------------------------------------- the run

struct Shared {
    stop: AtomicBool,
    evals: AtomicU64,
    perfect: AtomicU64,
    full_evals: AtomicU64,
    finds: AtomicUsize,
    seen: Mutex<HashSet<u64>>,
    best: Mutex<Vec<(Score, Grid)>>,
    log: Mutex<Vec<String>>,
    /// Strict screen improvements per move kind (MOVE_KINDS).
    improved: [AtomicU64; 4],
}

fn grid_hash(g: &Grid) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.hash(&mut h);
    h.finish()
}

pub fn run(version: &str, a: &SearchArgs) -> Result<()> {
    anyhow::ensure!(!a.seeds.is_empty(), "give at least one seed design");
    let mut doors = Vec::new();
    for s in &a.seeds {
        let (d, r) = cs::load_door(s)?;
        anyhow::ensure!(crate::single_layer(&d), "{}: not flat", s.display());
        doors.push((s.display().to_string(), d, r));
    }
    let spec0 = doors[0].2.spec;
    let (blo, bhi) = parse_box(&a.bx)?;
    let dx = spec0.door[0];
    let dy = spec0.door[1];
    let lo = [blo[0] + dx, blo[1] + dy];
    let hi = [bhi[0] + dx, bhi[1] + dy];
    let (flo, fhi) = match &a.focus {
        Some(f) => {
            let (l, h) = parse_box(f)?;
            ([l[0] + dx, l[1] + dy], [h[0] + dx, h[1] + dy])
        }
        None => (lo, hi),
    };
    let door_cells: Vec<(i32, i32)> = spec0.door_cells().iter().map(|q| (q[0], q[1])).collect();
    let geo = crate::properties::Geo::new(spec0);
    let mut ring = Vec::new();
    for x in lo[0] - 1..=hi[0] + 1 {
        for y in lo[1] - 1..=hi[1] + 1 {
            if geo.in_s([x, y, spec0.door[2]]) {
                ring.push((x, y));
            }
        }
    }
    let lever_cell = match &a.lever {
        Some(s) => {
            let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 2, "--lever x,y");
            Some((v[0] + dx, v[1] + dy))
        }
        None => None,
    };
    let lever_pos = lever_cell.map(|(x, y)| [x, y, spec0.door[2]]).unwrap_or(spec0.lever);
    let mut free = Vec::new();
    for x in flo[0]..=fhi[0] {
        for y in flo[1]..=fhi[1] {
            if in_box_raw(lo, hi, x, y) && !door_cells.contains(&(x, y)) && (x, y) != (lever_pos[0], lever_pos[1]) {
                free.push((x, y));
            }
        }
    }
    if let Some(fc) = &a.focus_cells {
        let mut want: Vec<(i32, i32)> = Vec::new();
        for t in fc.split(';').map(str::trim).filter(|t| !t.is_empty()) {
            let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 2, "--focus-cells x,y;x,y");
            want.push((v[0] + dx, v[1] + dy));
        }
        free = want.into_iter().filter(|(x, y)| in_box_raw(lo, hi, *x, *y) && !door_cells.contains(&(*x, *y)) && (*x, *y) != (lever_pos[0], lever_pos[1])).collect();
    }
    let door_block = doors[0].2.blocks.iter().find(|(q, _)| q == &spec0.door).map(|(_, b)| *b).unwrap_or(Block::Simple(Simple::Stone));
    let mut spec = spec0;
    spec.lever = lever_pos;
    let prob = Problem {
        spec,
        lo,
        hi,
        free,
        door_cells,
        ring,
        door_block,
        palette: palette(&a.palette)?,
        mech: !a.no_mech,
        cycles: a.cycles,
        max_ticks: a.max_ticks,
        reference: Vec::new(),
        ref_slack: a.ref_slack,
        ref_weight: a.ref_weight,
        lever_moves: a.lever_moves,
        assist_pistons: Vec::new(),
        assist_events: [Vec::new(), Vec::new()],
        assist_weight: a.assist_weight,
        assist_in_plane: HashSet::new(),
    };
    let mut prob = prob;
    if let Some(r) = &a.reference {
        let (_, rd) = cs::load_door(r)?;
        anyhow::ensure!(rd.spec.door == spec0.door, "the reference needs the same door port");
        let g = Grid { gx: 0, gy: 0, w: 0, h: 0, z: rd.spec.door[2], cells: Vec::new(), lever: (0, 0), assist: 0 };
        let _ = g;
        let ev = piston_events(&prob, &rd.blocks, rd.spec);
        println!("reference {}: {} piston events per cycle", r.display(), ev.len());
        prob.reference = ev;
    }
    if let Some(r) = &a.reference_ideal {
        let ev = ideal_events(&prob, r)?;
        println!("ideal reference {}: {} piston events per cycle", r.display(), ev.len());
        prob.reference = ev;
    }
    if let Some(path) = &a.assist {
        let text = std::fs::read_to_string(path)?;
        let s = crate::ideal::Schedule::parse(&text)?;
        let mut skipped = 0;
        for (ph, open) in [(0usize, true), (1usize, false)] {
            for (t, pos, st) in s.resolved(open)? {
                if st.as_deref() == Some("@use") {
                    skipped += 1;
                    continue;
                }
                if pos[2] == spec0.door[2] {
                    // an in-plane input: its cell is reserved (no block may go there) while the
                    // input stays assisted
                    prob.assist_in_plane.insert((pos[0], pos[1]));
                }
                let cell = (pos[0], pos[1]);
                let k = match prob.assist_pistons.iter().position(|c| *c == cell) {
                    Some(k) => k,
                    None => {
                        prob.assist_pistons.push(cell);
                        prob.assist_pistons.len() - 1
                    }
                };
                let b = match st {
                    Some(x) => cs::convert_state(&x).map_err(|e| anyhow::anyhow!(e))?,
                    None => Block::Air,
                };
                prob.assist_events[ph].push((t, k, cs::bp(pos), b));
            }
            prob.assist_events[ph].sort_by_key(|e| e.0);
        }
        anyhow::ensure!(prob.assist_pistons.len() <= 64, "at most 64 assisted inputs");
        println!(
            "assist {}: {} input cells off the plane ({:?}), {} / {} events, {skipped} in-plane or `use` events ignored",
            path.display(),
            prob.assist_pistons.len(),
            prob.assist_pistons.iter().map(|(x, y)| (x - dx, y - dy)).collect::<Vec<_>>(),
            prob.assist_events[0].len(),
            prob.assist_events[1].len()
        );
    }
    let mut seeds = Vec::new();
    for (name, _, r) in &doors {
        anyhow::ensure!(r.spec.door == spec0.door, "{name}: every seed needs the same door port");
        let mut g = grid_from_door(r, &prob)?;
        if let Some((x, y)) = lever_cell {
            let (ox, oy) = (r.spec.lever[0], r.spec.lever[1]);
            if !in_box(&prob, ox, oy) {
                g.set(ox, oy, Block::Air);
            }
            place_lever(&mut g, x, y);
        }
        clean_seed(&prob, &mut g);
        if !prob.assist_pistons.is_empty() {
            g.assist = if prob.assist_pistons.len() == 64 { u64::MAX } else { (1u64 << prob.assist_pistons.len()) - 1 };
            if let Some(cells) = &a.assist_cells {
                g.assist = 0;
                for t in cells.split(';').map(str::trim).filter(|t| !t.is_empty()) {
                    let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 2, "--assist-cells x,y;x,y");
                    match prob.assist_pistons.iter().position(|c| *c == (v[0] + dx, v[1] + dy)) {
                        Some(k) => g.assist |= 1u64 << k,
                        None => bail!("--assist-cells: no plan input at {t}"),
                    }
                }
            }
        }
        let s = screen(&prob, &g);
        println!("seed {name}: {} blocks in the box, screen {}/{}", g.count(), s.pts, s.max);
        if let Some(dir) = &a.write_seeds {
            std::fs::create_dir_all(dir)?;
            let stem = std::path::Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            door_of(&g, prob.spec).to_design(&format!("{stem}-cut")).save(&dir.join(format!("{stem}-cut.txt")))?;
        }
        seeds.push(g);
    }
    if a.lock_pistons {
        // the seed pistons (and their heads) stay put: their cells leave the move set
        let mut locked: HashSet<(i32, i32)> = HashSet::new();
        for g in &seeds {
            for x in g.gx..g.gx + g.w {
                for y in g.gy..g.gy + g.h {
                    if matches!(g.get(x, y), Block::Piston { .. } | Block::PistonHead { .. }) {
                        locked.insert((x, y));
                    }
                }
            }
        }
        prob.free.retain(|c| !locked.contains(c));
        println!("{} piston cells locked, {} cells free", locked.len(), prob.free.len());
    }
    std::fs::create_dir_all(&a.out_dir)?;
    println!(
        "box x {}..{}, y {}..{} ({}x{} = {}), {} free cells, palette {} states, {} threads, {} s",
        lo[0] - dx,
        hi[0] - dx,
        lo[1] - dy,
        hi[1] - dy,
        hi[0] - lo[0] + 1,
        hi[1] - lo[1] + 1,
        (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1),
        prob.free.len(),
        prob.palette.len(),
        a.threads,
        a.secs
    );
    let sh = Shared {
        stop: AtomicBool::new(false),
        evals: AtomicU64::new(0),
        perfect: AtomicU64::new(0),
        full_evals: AtomicU64::new(0),
        finds: AtomicUsize::new(0),
        seen: Mutex::new(HashSet::new()),
        best: Mutex::new(Vec::new()),
        log: Mutex::new(Vec::new()),
        improved: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
    };
    let jopts = cs::JudgeOpts::with_extra(28);
    let t0 = std::time::Instant::now();
    let mut seeds = seeds;
    if a.enum_k > 0 {
        let mut pool: Vec<(Score, Grid)> = Vec::new();
        for (i, g) in seeds.iter().enumerate() {
            let te = std::time::Instant::now();
            let r = enumerate(&prob, g, a.enum_k.min(5), a.enum_keep, a.threads, &sh, &jopts, a);
            println!(
                "enumeration from seed {i} (k = {}): {} screens in {:.1} s; best {}",
                a.enum_k,
                sh.evals.load(Ordering::Relaxed),
                te.elapsed().as_secs_f64(),
                r.iter().take(8).map(|(s, g)| format!("{}/{}+{}ev({}b)", s.pts, s.max, s.ev, g.count())).collect::<Vec<_>>().join(" ")
            );
            pool.extend(r);
        }
        pool.sort_by(|x, y| x.0.energy().partial_cmp(&y.0.energy()).unwrap());
        for (i, (s, g)) in pool.iter().take(3).enumerate() {
            let path = a.out_dir.join(format!("enum-best-{i}.txt"));
            door_of(g, prob.spec).to_design(&format!("cosearch-enum-{i}-{}of{}", s.pts, s.max)).save(&path)?;
        }
        if !pool.is_empty() {
            seeds = pool.into_iter().take(a.enum_keep.max(1)).map(|(_, g)| g).collect();
        }
    }
    let (tx, rx) = std::sync::mpsc::channel::<(std::path::PathBuf, mctas::design::Design)>();
    std::thread::scope(|s| {
        for ti in 0..a.threads {
            let (prob, seeds, sh, jopts, tx) = (&prob, &seeds, &sh, &jopts, tx.clone());
            s.spawn(move || anneal(ti, a, prob, seeds, sh, jopts, tx));
        }
        drop(tx);
        // confirmer: JVM checks of finds, in this thread
        let confirm = a.confirm;
        let sh2 = &sh;
        s.spawn(move || {
            let mut w = if confirm { mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() }).ok() } else { None };
            for (path, d) in rx {
                let line = match w.as_mut() {
                    Some(w) => match jvm_confirm(w, &d) {
                        Ok(l) => l,
                        Err(e) => format!("JVM error: {e:#}"),
                    },
                    None => "not confirmed (run with --confirm)".into(),
                };
                let msg = format!("FIND {}: {line}", path.display());
                println!("{msg}");
                sh2.log.lock().unwrap().push(msg);
            }
        });
        // progress
        let mut last = 0;
        while t0.elapsed().as_secs() < a.secs && !sh.stop.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let el = t0.elapsed().as_secs();
            if el >= last + a.report {
                last = el;
                let best = sh.best.lock().unwrap();
                let top: Vec<String> = best.iter().take(6).map(|(s, g)| format!("{}/{}+{}ev-{}as({}b)", s.pts, s.max, s.ev, s.assisted, g.count())).collect();
                println!(
                    "[{el:>5}s] {} screens ({:.0}/s), {} perfect screens, {} redsim judge runs, {} finds; improvements by move {}; best {}",
                    sh.evals.load(Ordering::Relaxed),
                    sh.evals.load(Ordering::Relaxed) as f64 / t0.elapsed().as_secs_f64(),
                    sh.perfect.load(Ordering::Relaxed),
                    sh.full_evals.load(Ordering::Relaxed),
                    sh.finds.load(Ordering::Relaxed),
                    MOVE_KINDS.iter().zip(sh.improved.iter()).map(|(n, c)| format!("{n}={}", c.load(Ordering::Relaxed))).collect::<Vec<_>>().join(" "),
                    top.join(" ")
                );
                // the current best, for inspection while the run goes on
                if let Some((s, g)) = best.first() {
                    let still: Vec<String> = (0..prob.assist_pistons.len())
                        .filter(|k| (g.assist >> k) & 1 == 1)
                        .map(|k| format!("{},{}", prob.assist_pistons[k].0 - dx, prob.assist_pistons[k].1 - dy))
                        .collect();
                    let mut d = door_of(g, prob.spec).to_design(&format!("cosearch-live-{}of{}", s.pts, s.max));
                    if !still.is_empty() {
                        d.meta.insert("assisted".into(), still.join(";"));
                    }
                    let _ = d.save(&a.out_dir.join("partial-live.txt"));
                }
            }
        }
        sh.stop.store(true, Ordering::Relaxed);
    });
    // the best partial candidates, for inspection
    let best = sh.best.lock().unwrap();
    for (i, (s, g)) in best.iter().take(3).enumerate() {
        let d = door_of(g, prob.spec).to_design(&format!("cosearch-partial-{i}-{}of{}", s.pts, s.max));
        let path = a.out_dir.join(format!("partial-{i}.txt"));
        d.save(&path)?;
        let still: Vec<(i32, i32)> = (0..prob.assist_pistons.len()).filter(|k| (g.assist >> k) & 1 == 1).map(|k| (prob.assist_pistons[k].0 - dx, prob.assist_pistons[k].1 - dy)).collect();
        println!("best partial {i}: {}/{} +{} events, {} assisted {still:?} ({} blocks) -> {}", s.pts, s.max, s.ev, s.assisted, g.count(), path.display());
    }
    println!("{} screens in {:.0} s; {} finds", sh.evals.load(Ordering::Relaxed), t0.elapsed().as_secs_f64(), sh.finds.load(Ordering::Relaxed));
    Ok(())
}

fn in_box_raw(lo: [i32; 2], hi: [i32; 2], x: i32, y: i32) -> bool {
    x >= lo[0] && x <= hi[0] && y >= lo[1] && y <= hi[1]
}

fn jvm_confirm(w: &mut mctas::sim::Worker, d: &mctas::design::Design) -> Result<String> {
    let mut opts = crate::VerifyOptions::default();
    opts.offsets.extend(crate::extra_offsets(28));
    let r = crate::verify(w, d, &opts);
    let j = match &r {
        Ok(rep) => format!("JVM WORKS {} blocks open {} close {}", rep.blocks, rep.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0), rep.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0)),
        Err(e) => return Ok(format!("JVM FAILS: {e:#}")),
    };
    let p = crate::properties::compute(w, d, false)?;
    Ok(format!("{j}; seamless {}; volume {} ({})", p.get("seamless").cloned().unwrap_or_default(), p.get("volume").cloned().unwrap_or_default(), p.get("volume_dims").cloned().unwrap_or_default()))
}

fn anneal(
    ti: usize,
    a: &SearchArgs,
    p: &Problem,
    seeds: &[Grid],
    sh: &Shared,
    jopts: &cs::JudgeOpts,
    tx: std::sync::mpsc::Sender<(std::path::PathBuf, mctas::design::Design)>,
) {
    let mut rng = Rng(a.seed.wrapping_mul(0x2545F4914F6CDD1D) ^ (ti as u64 + 1).wrapping_mul(0x9E3779B97F4A7C15));
    let mut run = 0u64;
    while !sh.stop.load(Ordering::Relaxed) {
        let mut cur = seeds[(ti + run as usize) % seeds.len()].clone();
        run += 1;
        let mut cs_ = screen(p, &cur);
        let mut best_local = cs_;
        for it in 0..a.iters {
            if sh.stop.load(Ordering::Relaxed) {
                return;
            }
            let frac = it as f64 / a.iters as f64;
            let temp = a.t0 * (a.t1 / a.t0).powf(frac);
            let mut cand = cur.clone();
            let nm = 1 + rng.below(3);
            let mut changed = false;
            let mut last: Option<(i32, i32)> = None;
            let mut kinds = [false; 4];
            for _ in 0..nm {
                // the next move is next to the previous one half of the time (local pairs)
                let at = match last {
                    Some((lx, ly)) if rng.below(2) == 0 => {
                        let d = PLANE_DIRS[rng.below(4)].step();
                        Some((lx + d[0], ly + d[1]))
                    }
                    _ => None,
                };
                let mut kind = 0;
                if let Some(c) = mutate(p, &mut cand, &mut rng, at, &mut kind) {
                    changed = true;
                    last = Some(c);
                    kinds[kind] = true;
                }
            }
            if !changed {
                continue;
            }
            let s = screen(p, &cand);
            sh.evals.fetch_add(1, Ordering::Relaxed);
            let de = s.energy() - cs_.energy();
            if de < 0.0 {
                // a strict improvement: credit the move kinds that made it
                for (k, used) in kinds.iter().enumerate() {
                    if *used {
                        sh.improved[k].fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
            if de <= 0.0 || rng.f64() < (-de / temp).exp() {
                cur = cand;
                cs_ = s;
                if s.energy() < best_local.energy() {
                    best_local = s;
                    let mut best = sh.best.lock().unwrap();
                    best.push((s, cur.clone()));
                    best.sort_by(|x, y| x.0.energy().partial_cmp(&y.0.energy()).unwrap());
                    best.dedup_by(|x, y| x.1 == y.1);
                    best.truncate(32);
                }
                if s.perfect() {
                    sh.perfect.fetch_add(1, Ordering::Relaxed);
                    try_find(p, &cur, sh, jopts, a, Some(&tx));
                }
            }
        }
    }
}
