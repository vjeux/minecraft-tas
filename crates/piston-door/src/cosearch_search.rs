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
    /// Move the plan's in-plane input positions (cells the design leaves empty) behind the
    /// pistons they feed (z+1), freeing those cells for real wiring.
    #[arg(long)]
    pub offplane: bool,
    /// Drive shaping weight per plan edge (0 = off): while an input is assisted, every on/off
    /// edge of its plan input that the real wiring alone also delivers to the input's piston
    /// (within `--drive-slack` ticks) earns this many points, every extra real edge costs half.
    #[arg(long, default_value_t = 0.0)]
    pub drive_weight: f64,
    #[arg(long, default_value_t = 2)]
    pub drive_slack: u32,
    /// Extra judge positions for the redsim judge of perfect screens and the JVM confirmation
    /// (0 = the judge's 4 default positions; 28 = the record judge's 32).
    #[arg(long, default_value_t = 0)]
    pub judge_extra: u32,
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
    /// Screen with the old EXACT rest rule (the build back to its rest state after every close)
    /// instead of the judge's periodic rule (main 96a62d6: closed, and some closed state repeats
    /// within cycles + 8; memory parts like copper bulbs may stay flipped between cycles).
    #[arg(long)]
    pub exact_rest: bool,
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
    /// Assisted inputs' blocks present at rest (an off-plane level input: its block is placed with
    /// the design while the input stays assisted): (input, position, block).
    pub assist_rest: Vec<(usize, BlockPos, Block)>,
    /// Every input's rest blocks, in the plane too (the plan's state at rest).
    pub assist_rest_all: Vec<(usize, BlockPos, Block)>,
    /// Every position each input's events touch.
    pub assist_pos: Vec<Vec<BlockPos>>,
    /// Per input, the cells whose blocks its positions feed: (cell, direct) — the in-plane cell
    /// next to each position (direct) and the one below it (a piston fed by quasi-connectivity),
    /// a QC cell dropped when it is another input's direct cell (that input owns it).
    pub assist_recv: Vec<Vec<(BlockPos, bool)>>,
    /// Drive shaping (`--drive-weight`): per assisted input, the plan's on/off edges matched by
    /// what the REAL wiring alone would feed its piston (the assist blocks removed for the check).
    pub drive_weight: f64,
    pub drive_slack: u32,
    /// Pockets mode: the seed's lever is removed; a candidate without a lever still screens (the
    /// plan drives every input by ticks after the "flip").
    pub no_lever: bool,
    /// Max drive points per input (its plan edges over the screened cycles): a released input
    /// earns them all (the screen checks the door works without it).
    pub drive_max: Vec<i32>,
    /// The screen requires the exact rest state after every close (the judge's rule before main
    /// 96a62d6); default: the periodic rule (closed, and some closed state repeats).
    pub exact_rest: bool,
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
    /// Drive shaping points (already weighted).
    pub dr: f64,
}

impl Score {
    fn energy(&self) -> f64 {
        -(self.pts as f64) - self.ev_w * self.ev as f64 + 0.02 * self.blocks as f64 + self.asw * self.assisted as f64 - self.dr
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
    let mut drive = 0.0;
    let measure = p.drive_weight > 0.0 && !p.assist_pistons.is_empty();
    let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
    let mut s = screen_core(p, g, &mut evs, &mut traces, measure);
    if measure {
        drive = drive_points(p, &traces);
    }
    if !p.reference.is_empty() {
        s.ev = match_events(&p.reference, &evs, p.ref_slack) as i32;
        s.ev_w = p.ref_weight;
    }
    if !p.assist_pistons.is_empty() {
        s.assisted = g.assist.count_ones() as i32;
        s.asw = p.assist_weight;
        s.dr = p.drive_weight * drive;
    }
    s
}

/// The plan's on/off state of input `k` after the events of ticks <= t of a phase, starting from
/// the rest state.
/// The plan's on/off state of input `k` when a phase starts (the close starts from the open's end).
fn plan_phase_start(p: &Problem, k: usize, phase: usize) -> bool {
    let mut on: HashSet<BlockPos> = p.assist_rest_all.iter().filter(|r| r.0 == k).map(|r| r.1).collect();
    if phase == 1 {
        for (_, kk, q, b) in &p.assist_events[0] {
            if *kk == k {
                if b.is_air() {
                    on.remove(q);
                } else {
                    on.insert(*q);
                }
            }
        }
    }
    !on.is_empty()
}

fn plan_bits(p: &Problem, phase: usize, k: usize, n: usize) -> Vec<bool> {
    let mut on: HashSet<BlockPos> = p.assist_rest_all.iter().filter(|r| r.0 == k).map(|r| r.1).collect();
    if phase == 1 {
        // the close starts from the open's end
        for (_, kk, q, b) in &p.assist_events[0] {
            if *kk == k {
                if b.is_air() {
                    on.remove(q);
                } else {
                    on.insert(*q);
                }
            }
        }
    }
    let mut out = Vec::with_capacity(n);
    let evs = &p.assist_events[phase];
    let mut i = 0;
    for t in 0..n as u32 {
        while i < evs.len() && evs[i].0 <= t {
            let (_, kk, q, b) = evs[i];
            if kk == k {
                if b.is_air() {
                    on.remove(&q);
                } else {
                    on.insert(q);
                }
            }
            i += 1;
        }
        out.push(!on.is_empty());
    }
    out
}

/// What the real wiring alone feeds each input's pistons now (bit k): the assist blocks of the
/// still-assisted inputs are taken out for the check (raw writes, put back after), then every
/// piston the input's positions power (directly or by quasi-connectivity) is asked
/// `getNeighborSignal`.
fn real_drive(p: &Problem, g: &Grid, w: &mut World) -> u64 {
    let z = p.spec.door[2];
    let mut saved: Vec<(BlockPos, Block)> = Vec::new();
    for k in 0..p.assist_pistons.len() {
        if (g.assist >> k) & 1 == 1 {
            for q in &p.assist_pos[k] {
                let b = w.get(*q);
                if !b.is_air() {
                    saved.push((*q, b));
                    w.put(*q, Block::Air);
                }
            }
        }
    }
    let mut real = 0u64;
    for k in 0..p.assist_pistons.len() {
        // the plan's block powers EVERY receiver next to it (a piston at the cell, the one below
        // it by QC): the real wiring stands in for it only when it powers all of them
        let mut any = false;
        let mut all = true;
        for &(c, direct) in &p.assist_recv[k] {
            let b = w.get(c);
            let fed = match b {
                Block::Piston { facing, .. } => Some(redsim::piston::base::neighbor_signal(w, c, facing)),
                // other receivers next to the input (trapdoors, lamps, note blocks, bulbs,
                // droppers...): their own neighbor-signal check
                Block::Trapdoor { .. } | Block::Lamp { .. } | Block::Note { .. } | Block::Bulb { .. } | Block::Dispenser { .. } | Block::Door { .. } | Block::FenceGate { .. } | Block::Rail { .. } if direct => Some(w.has_neighbor_signal(c)),
                _ => None,
            };
            if let Some(f) = fed {
                any = true;
                all &= f;
            }
        }
        if any && all {
            real |= 1u64 << k;
        }
    }
    let _ = z;
    for (q, b) in saved.iter().rev() {
        w.put(*q, *b);
    }
    real
}

/// Plan edges matched by real edges of the same polarity within `slack` ticks, minus half the
/// unmatched real edges.
fn edge_match(plan: &[bool], real: &[bool], slack: u32) -> f64 {
    let edges = |v: &[bool]| -> Vec<(usize, bool)> { (1..v.len()).filter(|&i| v[i] != v[i - 1]).map(|i| (i, v[i])).collect() };
    let pe = edges(plan);
    let re = edges(real);
    let mut used = vec![false; re.len()];
    let mut m = 0;
    for (t, up) in pe {
        let mut best: Option<(usize, usize)> = None;
        for (j, (rt, rup)) in re.iter().enumerate() {
            if used[j] || *rup != up {
                continue;
            }
            let d = rt.abs_diff(t);
            if d <= slack as usize && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((j, d));
            }
        }
        if let Some((j, _)) = best {
            used[j] = true;
            m += 1;
        }
    }
    m as f64 - 0.5 * used.iter().filter(|u| !**u).count() as f64
}

/// One phase after a flip: the assisted pistons' plan inputs applied at their ticks (before the
/// tick, like `door ideal`), ticking until idle (at least once) and until every input was
/// applied; `on_tick` sees the world after every tick. Returns whether it went idle. With
/// `traces`, what the real wiring feeds each input (bit k) is recorded per tick (index 0 = right
/// after the flip, i = after tick i).
fn run_phase(p: &Problem, g: &Grid, w: &mut World, phase: usize, traces: Option<&mut Vec<u64>>, mut on_tick: impl FnMut(&World)) -> bool {
    let evs = &p.assist_events[phase];
    let mut i = 0;
    let mut t = 0u32;
    let measure = traces.is_some();
    let mut real: Vec<u64> = Vec::new();
    if measure {
        real.push(real_drive(p, g, w));
    }
    let idle = loop {
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
        if measure {
            real.push(real_drive(p, g, w));
        }
        if i >= evs.len() && w.idle() {
            break true;
        }
        if t >= p.max_ticks {
            break false;
        }
    };
    if let Some(tr) = traces {
        *tr = real;
    }
    idle
}

/// Drive shaping points from the recorded traces (both phases of the first cycle).
fn drive_points(p: &Problem, traces: &[Vec<u64>; 2]) -> f64 {
    let mut d = 0.0;
    for phase in 0..2 {
        let real = &traces[phase];
        if real.is_empty() {
            continue;
        }
        let n = real.len();
        for k in 0..p.assist_pistons.len() {
            let pb = plan_bits(p, phase, k, n);
            let rb: Vec<bool> = real.iter().map(|r| (r >> k) & 1 == 1).collect();
            d += edge_match(&pb, &rb, p.drive_slack);
        }
    }
    d
}

fn screen_core(p: &Problem, g: &Grid, evs: &mut Vec<Event>, traces: &mut [Vec<u64>; 2], measure: bool) -> Score {
    let max = 11 + p.cycles as i32 * 34;
    let blocks = g.count();
    let mut pts = 0;
    let list = g.blocks();
    let spec = p.spec;
    let (wlo, whi) = cs::watch_box(&list, &spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
    let mut placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
    // still-assisted off-plane level inputs: their blocks are there at rest
    for (k, q, b) in &p.assist_rest {
        if (g.assist >> k) & 1 == 1 {
            placed.push((*q, *b));
        }
    }
    w.place_like_judge(&placed);
    let (_, idle) = cs::step_until_idle(&mut w, p.max_ticks, |_| {});
    if !idle || !w.errors.is_empty() {
        return Score { pts: 0, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
    }
    if !w.drops.is_empty() {
        // something popped at placement (a dust or a repeater without a floor...)
        return Score { pts: 1, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
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
    if !matches!(w.get(lever), Block::Lever { .. }) && !(p.no_lever && w.get(lever).is_air()) {
        return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
    }
    pts += 1;
    if door_ok < 9 || unsupported > 0 {
        return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
    }
    pts += 1;
    // the plane's cells (the box + 1) compared after each close
    let rest: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
    // closed states seen so far (the periodic rest rule)
    let mut seen: Vec<Vec<Block>> = vec![rest.clone()];
    let mut periodic = false;
    let rec_cells = if p.reference.is_empty() { Vec::new() } else { plane_cells_wide(&w, z) };
    for cycle in 0..p.cycles {
        w.use_block(lever);
        let rec = cycle == 0 && !rec_cells.is_empty();
        let mut prev = if rec { piston_cells(&w, &rec_cells) } else { Vec::new() };
        let mut t = 0;
        let measure = cycle == 0 && measure;
        let idle = run_phase(p, g, &mut w, 0, if measure { Some(&mut traces[0]) } else { None }, |w| {
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
            // the frame: the 12 cells around the opening must hold plain full blocks when open
            // (door blocks or static quartz-like blocks: vjeux 03:36Z, FULL FRAME)
            let clean = p.ring.iter().filter(|(x, y)| cs::frame_filled(&w.get(at(*x, *y)))).count() as i32;
            pts += clean;
            // nothing of the build outside the box
            let outside = plane_cells(p, z).iter().filter(|q| !in_box(p, q.x, q.y) && *q != &lever && !w.get(**q).is_air()).count() as i32;
            pts -= outside;
        }
        if !idle || !w.drops.is_empty() || !w.errors.is_empty() {
            return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
        }
        pts += 1;
        w.use_block(lever);
        let mut prev = if rec { piston_cells(&w, &rec_cells) } else { Vec::new() };
        let mut t = 0;
        let idle = run_phase(p, g, &mut w, 1, if measure { Some(&mut traces[1]) } else { None }, |w| {
            if rec {
                t += 1;
                tick_events(&prev, w, 1, t, evs);
                prev = piston_cells(w, &rec_cells);
            }
        });
        let back = p.door_cells.iter().filter(|(x, y)| w.get(at(*x, *y)) == p.door_block).count() as i32;
        pts += back;
        if !idle || !w.drops.is_empty() || !w.errors.is_empty() {
            return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
        }
        let now: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
        if p.exact_rest {
            let same = now.iter().zip(rest.iter()).filter(|(a, b)| a == b).count() as i32;
            let n = rest.len() as i32;
            if same == n {
                pts += 3;
            } else {
                pts -= (n - same).min(3);
                return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
            }
        } else {
            // PERIODIC rest (main 96a62d6, vjeux: "internal state doesn't matter as long as the
            // door works repeatedly"): a closed state may differ from rest (a copper bulb or a
            // dropper left flipped between cycles) as long as the door is closed and some closed
            // state repeats an earlier one (checked below, within cycles + MAX_PERIOD_EXTRA)
            if back < 9 {
                return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
            }
            pts += 3;
            if seen.iter().any(|s| *s == now) {
                periodic = true;
            } else {
                seen.push(now);
            }
        }
    }
    if !p.exact_rest && !periodic {
        // extra cycles (no points): each must still open (9 cells walkable, frame 12/12) and
        // close, until a closed state repeats
        for _ in 0..crate::MAX_PERIOD_EXTRA {
            w.use_block(lever);
            let idle = run_phase(p, g, &mut w, 0, None, |_| {});
            let open_n = p.door_cells.iter().filter(|(x, y)| {
                let s = w.get(at(*x, *y));
                !cs::collides(&s) && !cs::is_moving(&s)
            }).count();
            let clean = p.ring.iter().filter(|(x, y)| cs::frame_filled(&w.get(at(*x, *y)))).count();
            if !idle || open_n < 9 || clean < 12 || !w.drops.is_empty() || !w.errors.is_empty() {
                return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
            }
            w.use_block(lever);
            let idle = run_phase(p, g, &mut w, 1, None, |_| {});
            let back = p.door_cells.iter().filter(|(x, y)| w.get(at(*x, *y)) == p.door_block).count();
            if !idle || back < 9 || !w.drops.is_empty() || !w.errors.is_empty() {
                return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
            }
            let now: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
            if seen.iter().any(|s| *s == now) {
                periodic = true;
                break;
            }
            seen.push(now);
        }
        if !periodic {
            return Score { pts: pts.min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
        }
    }
    Score { pts, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 }
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
    // keep the rest polarity of the lever being moved (a door may rest with its lever on)
    let powered = matches!(g.get(g.lever.0, g.lever.1), Block::Lever { powered: true, .. });
    place_lever_p(g, x, y, powered);
}

fn place_lever_p(g: &mut Grid, x: i32, y: i32, powered: bool) {
    use redsim::block::Face;
    let lever = if !g.get(x, y + 1).is_air() {
        Block::Lever { face: Face::Ceiling, facing: Dir::West, powered }
    } else if !g.get(x, y - 1).is_air() {
        Block::Lever { face: Face::Floor, facing: Dir::West, powered }
    } else if !g.get(x - 1, y).is_air() {
        Block::Lever { face: Face::Wall, facing: Dir::East, powered }
    } else {
        Block::Lever { face: Face::Wall, facing: Dir::West, powered }
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
    let powered = matches!(g.get(ox, oy), Block::Lever { powered: true, .. });
    g.set(ox, oy, Block::Air);
    place_lever_p(g, x, y, powered);
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
    if !e.record_ok() {
        // a 1-position screen pass counts as a find to report (vjeux 05:07Z: the community door
        // may be locational): kept apart with the redsim judge's verdict
        let n = sh.finds.load(Ordering::Relaxed);
        let name = format!("cosearch-screenpass-{}-{n}-{:x}", door.block_count(), h & 0xffff);
        let mut d = door.to_design(&name);
        d.meta.insert("verdict".into(), cs::eval_line("redsim", &e, 0.0));
        let _ = d.save(&a.out_dir.join(format!("{name}.txt")));
        let msg = format!("SCREEN PASS (1 position, 2 cycles): {} -> {name}.txt", cs::eval_line(&name, &e, 0.0));
        println!("{msg}");
        sh.log.lock().unwrap().push(msg);
    }
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
    /// The best candidate with full screen points (fewest assisted inputs, then fewest blocks):
    /// a true partial (the door works with only those inputs still plan-driven).
    best_full: Mutex<Option<(Score, Grid)>>,
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

/// The search problem and its seeds from the command line (shared by `search` and `pockets`):
/// (problem, seeds, door x, door y).
pub fn setup(a: &SearchArgs) -> Result<(Problem, Vec<Grid>, i32, i32)> {
    anyhow::ensure!(!a.seeds.is_empty(), "give at least one seed design");
    let mut doors = Vec::new();
    for s in &a.seeds {
        // a mechanism file without a lever port: `--lever x,y` (door coordinates) names one
        let mut d = crate::cli::load_design(s)?;
        if !d.ports.contains_key("lever")
            && let (Some(lv), Some(door)) = (&a.lever, d.ports.get("door").copied())
        {
            let v: Vec<i32> = lv.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(v.len() == 2, "--lever x,y");
            d.ports.insert("lever".into(), [door[0] + v[0], door[1] + v[1], door[2]]);
        }
        let r = RsDoor::from_design(&d).map_err(|e| anyhow::anyhow!("{}: {e}", s.display()))?;
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
    // the 12 frame cells around the opening (FULL FRAME: plain full blocks when open)
    let ring: Vec<(i32, i32)> = crate::sheet::frame_cells(spec0.door).iter().map(|q| (q[0], q[1])).collect();
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
        assist_rest: Vec::new(),
        assist_rest_all: Vec::new(),
        assist_pos: Vec::new(),
        assist_recv: Vec::new(),
        drive_weight: a.drive_weight,
        drive_slack: a.drive_slack,
        drive_max: Vec::new(),
        no_lever: false,
        exact_rest: a.exact_rest,
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
        let asst = if a.offplane {
            let (raw, skipped) = load_assist_raw(path)?;
            let g0 = grid_from_door(&doors[0].2, &prob)?;
            assist_from_raw(&offplane_raw(&raw, &g0), spec0.door[2], skipped)
        } else {
            load_assist(path, spec0.door[2])?
        };
        prob.assist_in_plane = asst.in_plane;
        prob.assist_pistons = asst.cells;
        prob.assist_events = asst.events;
        // off-plane rest blocks only (an in-plane input's rest block is in the design)
        prob.assist_rest_all = asst.rest.clone();
        prob.assist_rest = asst.rest.into_iter().filter(|r| r.1.z != spec0.door[2]).collect();
        prob.assist_pos = asst.pos;
        {
            let z = spec0.door[2];
            let direct: Vec<Vec<BlockPos>> = prob
                .assist_pos
                .iter()
                .map(|ps| {
                    let mut v: Vec<BlockPos> = Vec::new();
                    for q in ps {
                        if q.z != z {
                            v.push(BlockPos::new(q.x, q.y, z));
                        } else {
                            for d in PLANE_DIRS {
                                let s = d.step();
                                v.push(BlockPos::new(q.x + s[0], q.y + s[1], z));
                            }
                        }
                    }
                    v.sort_by_key(|b| (b.x, b.y));
                    v.dedup();
                    v
                })
                .collect();
            prob.assist_recv = (0..direct.len())
                .map(|k| {
                    let mut r: Vec<(BlockPos, bool)> = direct[k].iter().map(|c| (*c, true)).collect();
                    for c in &direct[k] {
                        let below = BlockPos::new(c.x, c.y - 1, z);
                        let owned = (0..direct.len()).any(|j| j != k && direct[j].contains(&below));
                        if !owned && !direct[k].contains(&below) {
                            r.push((below, false));
                        }
                    }
                    r
                })
                .collect();
        }
        prob.drive_max = asst.edges.iter().map(|e| e * a.cycles as i32).collect();
        let skipped = asst.skipped;
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
    Ok((prob, seeds, dx, dy))
}

pub fn run(version: &str, a: &SearchArgs) -> Result<()> {
    let (prob, seeds, dx, dy) = setup(a)?;
    let (lo, hi) = (prob.lo, prob.hi);
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
        best_full: Mutex::new(None),
        log: Mutex::new(Vec::new()),
        improved: [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)],
    };
    let jopts = cs::JudgeOpts::with_extra(a.judge_extra);
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
        let judge_extra = a.judge_extra;
        let sh2 = &sh;
        s.spawn(move || {
            let mut w = if confirm { mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() }).ok() } else { None };
            for (path, d) in rx {
                let line = match w.as_mut() {
                    Some(w) => match jvm_confirm(w, &d, judge_extra) {
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
                let top: Vec<String> = best.iter().take(6).map(|(s, g)| format!("{}/{}+{}ev-{}as+{:.2}dr({}b)", s.pts, s.max, s.ev, s.assisted, s.dr, g.count())).collect();
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
                drop(best);
                // the best full-points partial (the door works with only these inputs assisted)
                if let Some((s, g)) = sh.best_full.lock().unwrap().as_ref() {
                    let still: Vec<String> = (0..prob.assist_pistons.len())
                        .filter(|k| (g.assist >> k) & 1 == 1)
                        .map(|k| format!("{},{}", prob.assist_pistons[k].0 - dx, prob.assist_pistons[k].1 - dy))
                        .collect();
                    println!("        best full-points partial: {} assisted [{}] drive {:.2} ({} blocks)", s.assisted, still.join(" "), s.dr, g.count());
                    let mut d = door_of(g, prob.spec).to_design(&format!("cosearch-full-{}as", s.assisted));
                    if !still.is_empty() {
                        d.meta.insert("assisted".into(), still.join(";"));
                    }
                    let _ = d.save(&a.out_dir.join("partial-full-live.txt"));
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

/// A `door ideal` plan's inputs as assist events: the input cells (x, y; an input off the plane
/// counts for the cell in front of it), per phase (0 open, 1 close) the events (tick, input index,
/// position, state set there; air = cleared), the in-plane input cells, `use` events skipped.
pub struct Assist {
    pub cells: Vec<(i32, i32)>,
    pub events: [Vec<(u32, usize, BlockPos, Block)>; 2],
    pub in_plane: HashSet<(i32, i32)>,
    pub skipped: usize,
    /// Every position each input touches, and the blocks present at rest (the state after a
    /// whole open + close cycle of events, e.g. a level input's redstone block).
    pub pos: Vec<Vec<BlockPos>>,
    pub rest: Vec<(usize, BlockPos, Block)>,
    /// Plan on/off edges per input over one open + close cycle.
    pub edges: Vec<i32>,
}

pub fn load_assist(path: &std::path::Path, door_z: i32) -> Result<Assist> {
    let (raw, skipped) = load_assist_raw(path)?;
    Ok(assist_from_raw(&raw, door_z, skipped))
}

/// A plan's input events per phase (tick, position, state; air = cleared), `use` events skipped.
pub fn load_assist_raw(path: &std::path::Path) -> Result<([Vec<(u32, BlockPos, Block)>; 2], usize)> {
    let text = std::fs::read_to_string(path)?;
    let s = crate::ideal::Schedule::parse(&text)?;
    let mut raw: [Vec<(u32, BlockPos, Block)>; 2] = [Vec::new(), Vec::new()];
    let mut skipped = 0;
    for (ph, open) in [(0usize, true), (1usize, false)] {
        for (t, pos, st) in s.resolved(open)? {
            if st.as_deref() == Some("@use") {
                skipped += 1;
                continue;
            }
            let b = match st {
                Some(x) => cs::convert_state(&x).map_err(|e| anyhow::anyhow!(e))?,
                None => Block::Air,
            };
            raw[ph].push((t, cs::bp(pos), b));
        }
    }
    Ok((raw, skipped))
}

/// `--offplane`: every in-plane input position that is not a block of the design moves behind
/// the pistons it feeds: one position (x, y, z+1) per in-plane neighbor n of it holding a piston
/// at rest or standing on one (the redstone block behind n powers the piston at n directly and
/// the one below n by quasi-connectivity, exactly as the in-plane block did). Frees the plan's
/// in-plane input cells for real wiring.
pub fn offplane_raw(raw: &[Vec<(u32, BlockPos, Block)>; 2], g: &Grid) -> [Vec<(u32, BlockPos, Block)>; 2] {
    let z = g.z;
    let is_p = |x: i32, y: i32| matches!(g.get(x, y), Block::Piston { .. });
    let mut out: [Vec<(u32, BlockPos, Block)>; 2] = [Vec::new(), Vec::new()];
    for ph in 0..2 {
        for (t, q, b) in &raw[ph] {
            if q.z != z || !g.get(q.x, q.y).is_air() {
                out[ph].push((*t, *q, *b));
                continue;
            }
            let mut moved = false;
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (q.x + dx, q.y + dy);
                if is_p(nx, ny) || is_p(nx, ny - 1) {
                    out[ph].push((*t, BlockPos::new(nx, ny, z + 1), *b));
                    moved = true;
                }
            }
            if !moved {
                out[ph].push((*t, *q, *b));
            }
        }
    }
    out
}

pub fn assist_from_raw(raw: &[Vec<(u32, BlockPos, Block)>; 2], door_z: i32, skipped: usize) -> Assist {
    let mut a = Assist { cells: Vec::new(), events: [Vec::new(), Vec::new()], in_plane: HashSet::new(), skipped, pos: Vec::new(), rest: Vec::new(), edges: Vec::new() };
    for ph in 0..2 {
        for &(t, pos, b) in &raw[ph] {
            if pos.z == door_z {
                // an in-plane input: its cell is reserved (no block may go there) while the
                // input stays assisted
                a.in_plane.insert((pos.x, pos.y));
            }
            let cell = (pos.x, pos.y);
            let k = match a.cells.iter().position(|c| *c == cell) {
                Some(k) => k,
                None => {
                    a.cells.push(cell);
                    a.cells.len() - 1
                }
            };
            a.events[ph].push((t, k, pos, b));
        }
        a.events[ph].sort_by_key(|e| e.0);
    }
    // positions, rest state (after a whole cycle), edges per cycle
    a.pos = vec![Vec::new(); a.cells.len()];
    let mut state: std::collections::HashMap<BlockPos, Block> = std::collections::HashMap::new();
    for ph in 0..2 {
        for (_, k, p, b) in &a.events[ph] {
            if !a.pos[*k].contains(p) {
                a.pos[*k].push(*p);
            }
            state.insert(*p, *b);
        }
    }
    for k in 0..a.cells.len() {
        for p in &a.pos[k] {
            if let Some(b) = state.get(p)
                && !b.is_air()
            {
                a.rest.push((k, *p, *b));
            }
        }
    }
    for k in 0..a.cells.len() {
        let mut on: HashSet<BlockPos> = a.rest.iter().filter(|r| r.0 == k).map(|r| r.1).collect();
        let mut edges = 0;
        for ph in 0..2 {
            for (_, kk, p, b) in &a.events[ph] {
                if *kk != k {
                    continue;
                }
                let was = !on.is_empty();
                if b.is_air() {
                    on.remove(p);
                } else {
                    on.insert(*p);
                }
                if was != !on.is_empty() {
                    edges += 1;
                }
            }
        }
        a.edges.push(edges);
    }
    a
}

/// Three characters for a block in a plane picture (`cosearch trace --grid`).
pub fn cell3(b: &Block) -> String {
    let d = |f: Dir| match f {
        Dir::Up => '^',
        Dir::Down => 'v',
        Dir::East => '>',
        Dir::West => '<',
        Dir::North => 'n',
        Dir::South => 's',
    };
    match *b {
        Block::Air => " . ".into(),
        Block::Simple(Simple::Glass) => "gls".into(),
        Block::Simple(_) => "###".into(),
        Block::Glazed { .. } => "GT ".into(),
        Block::Wire { power, .. } => format!("r{power:<2}"),
        Block::RedstoneBlock => "RB ".into(),
        Block::Torch { lit } => if lit { "T* ".into() } else { "T  ".into() },
        Block::WallTorch { facing, lit } => format!("T{}{}", d(facing), if lit { '*' } else { ' ' }),
        Block::Lever { powered, .. } => if powered { "L* ".into() } else { "L  ".into() },
        Block::Repeater { facing, delay, powered, .. } => format!("{}{}{delay}", if powered { 'D' } else { 'd' }, d(facing.opposite())),
        Block::Comparator { facing, subtract, powered } => format!("{}{}{}", if powered { 'C' } else { 'c' }, d(facing.opposite()), if subtract { 's' } else { ' ' }),
        Block::Observer { facing, powered } => format!("O{}{}", d(facing), if powered { '*' } else { ' ' }),
        Block::Lamp { lit } => if lit { "La*".into() } else { "La ".into() },
        Block::Bulb { lit, powered, .. } => format!("B{}{}", if lit { '*' } else { ' ' }, if powered { 'p' } else { ' ' }),
        Block::Note { powered, .. } => if powered { "N* ".into() } else { "N  ".into() },
        Block::Target { power } => format!("t{power:<2}"),
        Block::Dispenser { facing, triggered, .. } => format!("Q{}{}", d(facing), if triggered { '*' } else { ' ' }),
        Block::Piston { sticky, facing, extended } => format!("{}{}{}", if sticky { 'S' } else { 'P' }, d(facing), if extended { '+' } else { ' ' }),
        Block::PistonHead { facing, .. } => format!("h{} ", d(facing)),
        Block::MovingPiston { facing, .. } => format!("~{} ", d(facing)),
        Block::Leaves { distance, .. } => format!("lv{distance}"),
        Block::Log { .. } => "log".into(),
        _ => "???".into(),
    }
}

fn jvm_confirm(w: &mut mctas::sim::Worker, d: &mctas::design::Design, extra: u32) -> Result<String> {
    let mut opts = crate::VerifyOptions::default();
    opts.offsets.extend(crate::extra_offsets(extra));
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
                if s.pts >= s.max {
                    let mut bf = sh.best_full.lock().unwrap();
                    let better = match bf.as_ref() {
                        None => true,
                        Some((b, bg)) => (s.assisted, -s.ev, cur.count()) < (b.assisted, -b.ev, bg.count()),
                    };
                    if better {
                        *bf = Some((s, cur.clone()));
                    }
                }
                if s.perfect() {
                    sh.perfect.fetch_add(1, Ordering::Relaxed);
                    try_find(p, &cur, sh, jopts, a, Some(&tx));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------- pockets

/// `mctas door cosearch pockets`: pocket-exhaustive wiring. The free cells the plan never sweeps
/// split into pockets (connected regions); every filling of up to `--k` blocks of each pocket
/// (connected, touching the build; a moved lever counts as one block) is simulated with the rest
/// of the door driven by the plan, and kept when the door still works and the real wiring alone
/// feeds some plan input's piston every pulse / level edge inside that edge's timing window
/// (windows measured by shifting each plan edge alone). Then one filling per pocket is combined
/// to cover every input and verified in redsim (screen, then the redsim judge at 4 positions +
/// FULL + hallway), finds written and confirmed by the JVM (`--confirm`).
#[derive(clap::Args, Clone, Debug)]
pub struct PocketArgs {
    #[command(flatten)]
    pub s: SearchArgs,
    /// Most blocks per filling (a moved lever counts as one).
    #[arg(long, default_value_t = 3)]
    pub k: usize,
    /// Palette of the fillings (families as in --palette); ring cells take stone only.
    #[arg(long, default_value = "stone,glass,dust,rblock,torch,walltorch,repeater,comparator,observer,lamp,note,target,bulb,dropper,trapdoor,leaves,log,sticky,piston")]
    pub fill_palette: String,
    /// Exact block states for the fillings instead (`;`-separated, e.g.
    /// `observer[facing=east];repeater[facing=east,delay=4];redstone_wire`).
    #[arg(long)]
    pub fill_states: Option<String>,
    /// Window scan (ticks) around each plan pulse / level edge.
    #[arg(long, default_value_t = 24)]
    pub win: i32,
    /// Accepted fillings kept per (pocket, driven input set), fewest blocks first.
    #[arg(long, default_value_t = 24)]
    pub keep: usize,
    /// Most combinations verified.
    #[arg(long, default_value_t = 200000)]
    pub max_combos: usize,
    /// Skip pockets with more cells than this.
    #[arg(long, default_value_t = 16)]
    pub max_pocket: usize,
    /// Lever candidate cells (door coordinates `x,y;...`): default the air cells next to a pocket
    /// cell that the plan never sweeps (inside or outside the box).
    #[arg(long)]
    pub lever_cells: Option<String>,
    /// Pulse width tolerance (ticks over the plan's pulse width).
    #[arg(long, default_value_t = 4)]
    pub width_tol: u32,
    /// Only these pockets (indices as printed), default all.
    #[arg(long)]
    pub only_pockets: Option<String>,
    /// Keep the seed's real lever (its wiring is already real): no lever candidates.
    #[arg(long)]
    pub keep_lever: bool,
    /// A find needs the redsim judge + FULL + frame 12/12 only, not the hallway check (the
    /// community door's lever on top of its block fails redsim's hallway check).
    #[arg(long)]
    pub no_hallway: bool,
    /// Clear these cells of the seed first (door coordinates `x,y;...`; non-mechanism blocks only)
    /// so they join the pockets: re-wire a region of an existing partial.
    #[arg(long)]
    pub region_cells: Option<String>,
    /// Debug: screen this design as one filling (same assist) and print each input's real drive
    /// trace and the plan groups it matches, then stop.
    #[arg(long)]
    pub debug_fill: Option<std::path::PathBuf>,
}

/// One plan pulse (on, then off) or level edge of one input in one phase, with its window.
#[derive(Clone, Debug)]
struct PGroup {
    k: usize,
    phase: usize,
    /// Indices into `assist_events[phase]`.
    evi: Vec<usize>,
    t: u32,
    width: Option<u32>,
    up: bool,
    lo: i32,
    hi: i32,
}

#[derive(Clone, Debug)]
struct Filling {
    blocks: Vec<((i32, i32), Block)>,
    lever: Option<(i32, i32)>,
    driven: u64,
}

impl Filling {
    fn cost(&self) -> usize {
        self.blocks.len() + self.lever.is_some() as usize
    }
}

fn plan_groups(p: &Problem) -> Vec<PGroup> {
    let mut out = Vec::new();
    for phase in 0..2 {
        let evs = &p.assist_events[phase];
        let mut used = vec![false; evs.len()];
        for i in 0..evs.len() {
            if used[i] {
                continue;
            }
            used[i] = true;
            let (t, k, q, b) = evs[i];
            if !b.is_air() {
                // a pulse: the next event of the same input at the same position clears it
                if let Some(j) = (i + 1..evs.len()).find(|&j| !used[j] && evs[j].1 == k && evs[j].2 == q) {
                    if evs[j].3.is_air() {
                        used[j] = true;
                        out.push(PGroup { k, phase, evi: vec![i, j], t, width: Some(evs[j].0 - t), up: true, lo: 0, hi: 0 });
                        continue;
                    }
                }
            }
            out.push(PGroup { k, phase, evi: vec![i], t, width: None, up: !b.is_air(), lo: 0, hi: 0 });
        }
    }
    out
}

/// Each group's window: the shifts (others at the plan's ticks) that keep the screen perfect.
fn measure_windows(p: &Problem, seed: &Grid, groups: &mut [PGroup], win: i32, threads: usize) {
    let max = 11 + p.cycles as i32 * 34;
    let jobs: Vec<(usize, i32)> = (0..groups.len()).flat_map(|gi| (-win..=win).filter(|d| *d != 0).map(move |d| (gi, d))).collect();
    let res = cs::par_map(&jobs, threads, |&(gi, d)| {
        let gr = &groups[gi];
        if (gr.t as i32) + d < 0 {
            return false;
        }
        let mut p2 = p.clone();
        for &i in &gr.evi {
            let e = &mut p2.assist_events[gr.phase][i];
            e.0 = (e.0 as i32 + d) as u32;
        }
        p2.assist_events[gr.phase].sort_by_key(|e| e.0);
        p2.drive_weight = 0.0;
        screen(&p2, seed).pts >= max
    });
    for (gi, gr) in groups.iter_mut().enumerate() {
        let ok = |d: i32| d == 0 || jobs.iter().zip(res.iter()).any(|(j, r)| j.0 == gi && j.1 == d && *r);
        let mut lo = 0;
        while lo > -win && ok(lo - 1) {
            lo -= 1;
        }
        let mut hi = 0;
        while hi < win && ok(hi + 1) {
            hi += 1;
        }
        gr.lo = lo;
        gr.hi = hi;
    }
}

/// Which plan groups the real traces reproduce: every real pulse / level edge on a still-assisted
/// input must fall in the window of one of that input's groups (one real pulse per group, width
/// within tolerance); a real edge no group explains rejects the filling (None). Returns the
/// covered groups (bit = index in `groups`).
fn match_groups(p: &Problem, groups: &[PGroup], need: u64, traces: &[Vec<u64>; 2], width_tol: u32) -> Option<u64> {
    let mut covered = 0u64;
    for k in 0..p.assist_pistons.len() {
        if (need >> k) & 1 == 0 {
            continue;
        }
        // an input the filling never feeds (its real drive is off throughout) is not touched
        if traces.iter().all(|tr| tr.iter().all(|r| (r >> k) & 1 == 0)) {
            continue;
        }
        for phase in 0..2 {
            let real = &traces[phase];
            if real.is_empty() {
                return None;
            }
            // the phase's starting state (the close starts from the open's end)
            let rest_on = plan_phase_start(p, k, phase);
            let rb: Vec<bool> = real.iter().map(|r| (r >> k) & 1 == 1).collect();
            let mut edges: Vec<(i32, bool)> = Vec::new();
            if rb[0] != rest_on {
                edges.push((0, rb[0]));
            }
            for i in 1..rb.len() {
                if rb[i] != rb[i - 1] {
                    edges.push((i as i32, rb[i]));
                }
            }
            if edges.is_empty() {
                continue;
            }
            // the input's groups in this phase
            let gs: Vec<usize> = (0..groups.len()).filter(|&gi| groups[gi].k == k && groups[gi].phase == phase).collect();
            let mut e = 0;
            while e < edges.len() {
                let (t1, u1) = edges[e];
                // a pulse (rising then falling) or a single level edge
                let mut hit = None;
                for &gi in &gs {
                    let g = &groups[gi];
                    if (covered >> gi) & 1 == 1 {
                        continue;
                    }
                    let (lo, hi) = (g.t as i32 + g.lo, g.t as i32 + g.hi);
                    if t1 < lo || t1 > hi {
                        continue;
                    }
                    match g.width {
                        Some(w) => {
                            if !u1 || e + 1 >= edges.len() {
                                continue;
                            }
                            let (t2, u2) = edges[e + 1];
                            let wd = (t2 - t1) as u32;
                            if u2 || wd + 1 < w || wd > w + width_tol {
                                continue;
                            }
                            hit = Some((gi, 2));
                        }
                        None => {
                            if u1 != g.up {
                                continue;
                            }
                            hit = Some((gi, 1));
                        }
                    }
                    break;
                }
                match hit {
                    Some((gi, n)) => {
                        covered |= 1u64 << gi;
                        e += n;
                    }
                    None => return None,
                }
            }
        }
    }
    Some(covered)
}

/// Does the real trace of input `k` (both phases) reproduce its plan groups inside their windows,
/// with no extra edge?
#[allow(dead_code)]
fn accepts(p: &Problem, groups: &[PGroup], k: usize, traces: &[Vec<u64>; 2], width_tol: u32) -> bool {
    for phase in 0..2 {
        let real = &traces[phase];
        if real.is_empty() {
            return false;
        }
        let rb: Vec<bool> = real.iter().map(|r| (r >> k) & 1 == 1).collect();
        let rest_on = p.assist_rest_all.iter().any(|r| r.0 == k);
        let mut edges: Vec<(i32, bool)> = Vec::new();
        if rb[0] != rest_on {
            edges.push((0, rb[0]));
        }
        for i in 1..rb.len() {
            if rb[i] != rb[i - 1] {
                edges.push((i as i32, rb[i]));
            }
        }
        let gs: Vec<&PGroup> = groups.iter().filter(|g| g.k == k && g.phase == phase).collect();
        let mut e = 0;
        for g in gs {
            let (lo, hi) = (g.t as i32 + g.lo, g.t as i32 + g.hi);
            match g.width {
                Some(w) => {
                    if e + 1 >= edges.len() {
                        return false;
                    }
                    let (t1, u1) = edges[e];
                    if !u1 || t1 < lo || t1 > hi {
                        return false;
                    }
                    let (t2, u2) = edges[e + 1];
                    let wd = (t2 - t1) as u32;
                    if u2 || wd + 1 < w || wd > w + width_tol {
                        return false;
                    }
                    e += 2;
                }
                None => {
                    if e >= edges.len() {
                        return false;
                    }
                    let (t1, u1) = edges[e];
                    if u1 != g.up || t1 < lo || t1 > hi {
                        return false;
                    }
                    e += 1;
                }
            }
        }
        if e != edges.len() {
            return false;
        }
    }
    true
}

fn connected(cells: &[(i32, i32)]) -> bool {
    if cells.is_empty() {
        return false;
    }
    let mut seen = vec![false; cells.len()];
    let mut stack = vec![0];
    seen[0] = true;
    while let Some(i) = stack.pop() {
        for j in 0..cells.len() {
            if !seen[j] && (cells[i].0 - cells[j].0).abs() + (cells[i].1 - cells[j].1).abs() == 1 {
                seen[j] = true;
                stack.push(j);
            }
        }
    }
    seen.iter().all(|s| *s)
}

fn subsets_upto(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    fn rec(n: usize, k: usize, start: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if !cur.is_empty() {
            out.push(cur.clone());
        }
        if cur.len() == k {
            return;
        }
        for i in start..n {
            cur.push(i);
            rec(n, k, i + 1, cur, out);
            cur.pop();
        }
    }
    rec(n, k, 0, &mut Vec::new(), &mut out);
    out
}

pub fn pockets_run(version: &str, pa: &PocketArgs) -> Result<()> {
    let a = &pa.s;
    let (mut prob, seeds, dx, dy) = setup(a)?;
    let mut seed = seeds[0].clone();
    // the lever is placed by the fillings (lever candidates); without one the plan still drives
    // every input by ticks after the "flip" (`--keep-lever`: the seed's real lever stays)
    if !pa.keep_lever {
        prob.no_lever = true;
        seed.set(seed.lever.0, seed.lever.1, Block::Air);
    }
    if let Some(rc) = &pa.region_cells {
        for t in rc.split(';').map(str::trim).filter(|t| !t.is_empty()) {
            let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            anyhow::ensure!(v.len() == 2, "--region-cells x,y;x,y");
            let (x, y) = (v[0] + dx, v[1] + dy);
            let b = seed.get(x, y);
            anyhow::ensure!(!matches!(b, Block::Piston { .. } | Block::PistonHead { .. } | Block::Lever { .. }) && !prob.door_cells.contains(&(x, y)), "--region-cells: ({},{}) holds {b:?}", v[0], v[1]);
            seed.set(x, y, Block::Air);
            if !prob.free.contains(&(x, y)) && x >= prob.lo[0] && x <= prob.hi[0] && y >= prob.lo[1] && y <= prob.hi[1] {
                prob.free.push((x, y));
            }
        }
    }
    std::fs::create_dir_all(&a.out_dir)?;
    let max = 11 + prob.cycles as i32 * 34;
    let s0 = screen(&prob, &seed);
    println!("seed screen {}/{} with {} inputs assisted", s0.pts, s0.max, seed.assist.count_ones());
    anyhow::ensure!(s0.pts >= max, "the seed must screen perfect with its assisted inputs");
    let names: Vec<String> = prob.assist_pistons.iter().map(|(x, y)| format!("({},{})", x - dx, y - dy)).collect();
    let t0 = std::time::Instant::now();
    // ---- windows
    let mut groups = plan_groups(&prob);
    groups.retain(|g| (seed.assist >> g.k) & 1 == 1);
    measure_windows(&prob, &seed, &mut groups, pa.win, a.threads);
    anyhow::ensure!(groups.len() <= 64, "at most 64 plan groups ({})", groups.len());
    let glabels: Vec<String> = groups.iter().map(|g| format!("{}{}{}", names[g.k], if g.phase == 0 { "o" } else { "c" }, g.t)).collect();
    if let Some(dbg) = &pa.debug_fill {
        let (_, r) = cs::load_door(dbg)?;
        let mut g = grid_from_door(&r, &prob)?;
        g.assist = seed.assist;
        let mut evs = Vec::new();
        let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        let sc = screen_core(&prob, &g, &mut evs, &mut traces, true);
        println!("debug filling {}: screen {}/{}", dbg.display(), sc.pts, sc.max);
        for k in 0..prob.assist_pistons.len() {
            for phase in 0..2 {
                let s: String = traces[phase].iter().map(|r| if (r >> k) & 1 == 1 { '#' } else { '.' }).collect();
                let pb: String = plan_bits(&prob, phase, k, traces[phase].len()).iter().map(|b| if *b { '#' } else { '.' }).collect();
                println!("  input {} {}: real {s}\n  {:>w$}   plan {pb}", names[k], if phase == 0 { "open " } else { "close" }, "", w = names[k].len() + 8);
            }
        }
        match match_groups(&prob, &groups, seed.assist, &traces, pa.width_tol) {
            Some(c) => println!("  matched groups: {}", (0..64).filter(|i| (c >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" ")),
            None => println!("  rejected: a real edge no group explains"),
        }
        return Ok(());
    }
    println!("windows ({} groups, {:.1} s):", groups.len(), t0.elapsed().as_secs_f64());
    for g in &groups {
        println!(
            "  input {} {} t{} {}: window [{:+}, {:+}]",
            names[g.k],
            if g.phase == 0 { "open " } else { "close" },
            g.t,
            match g.width {
                Some(w) => format!("pulse {w} gt"),
                None => (if g.up { "on" } else { "off" }).to_string(),
            },
            g.lo,
            g.hi
        );
    }
    // ---- swept cells and pockets
    let z = seed.z;
    let mut rest_occ: HashSet<(i32, i32)> = HashSet::new();
    let mut swept: HashSet<(i32, i32)> = HashSet::new();
    {
        let list = seed.blocks();
        let (wlo, whi) = cs::watch_box(&list, &prob.spec);
        let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
        let mut placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
        for (k, q, b) in &prob.assist_rest {
            if (seed.assist >> k) & 1 == 1 {
                placed.push((*q, *b));
            }
        }
        w.place_like_judge(&placed);
        cs::step_until_idle(&mut w, prob.max_ticks, |_| {});
        let cells = plane_cells(&prob, z);
        for q in &cells {
            if !w.get(*q).is_air() {
                rest_occ.insert((q.x, q.y));
            }
        }
        let lever = BlockPos::new(seed.lever.0, seed.lever.1, z);
        for phase in 0..2 {
            w.use_block(lever);
            run_phase(&prob, &seed, &mut w, phase, None, |w| {
                for q in &cells {
                    if !w.get(*q).is_air() {
                        swept.insert((q.x, q.y));
                    }
                }
            });
        }
    }
    let ring: HashSet<(i32, i32)> = prob.ring.iter().copied().collect();
    let pocket_cell = |c: &(i32, i32)| prob.free.contains(c) && !rest_occ.contains(c) && !swept.contains(c) && !prob.assist_in_plane.contains(c);
    let cand: Vec<(i32, i32)> = prob.free.iter().copied().filter(|c| pocket_cell(c)).collect();
    let mut pockets: Vec<Vec<(i32, i32)>> = Vec::new();
    let mut seen: HashSet<(i32, i32)> = HashSet::new();
    for &c in &cand {
        if seen.contains(&c) {
            continue;
        }
        let mut comp = vec![c];
        seen.insert(c);
        let mut i = 0;
        while i < comp.len() {
            let (x, y) = comp[i];
            for (ddx, ddy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (x + ddx, y + ddy);
                if cand.contains(&n) && seen.insert(n) {
                    comp.push(n);
                }
            }
            i += 1;
        }
        comp.sort();
        pockets.push(comp);
    }
    let lever_cands: Vec<(i32, i32)> = match &pa.lever_cells {
        _ if pa.keep_lever => Vec::new(),
        Some(s) => s
            .split(';')
            .filter_map(|t| {
                let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (v.len() == 2).then(|| (v[0] + dx, v[1] + dy))
            })
            .collect(),
        None => {
            let mut v = Vec::new();
            for x in prob.lo[0] - 1..=prob.hi[0] + 1 {
                for y in prob.lo[1] - 1..=prob.hi[1] + 1 {
                    let c = (x, y);
                    if rest_occ.contains(&c) || swept.contains(&c) || prob.door_cells.contains(&c) || ring.contains(&c) || prob.assist_in_plane.contains(&c) {
                        continue;
                    }
                    if cand.iter().any(|q| (q.0 - x).abs() + (q.1 - y).abs() == 1) || cand.contains(&c) {
                        v.push(c);
                    }
                }
            }
            v
        }
    };
    println!("{} pocket cells in {} pockets; {} lever candidates; {} swept cells", cand.len(), pockets.len(), lever_cands.len(), swept.len());
    for (i, pk) in pockets.iter().enumerate() {
        println!("  pocket {i}: {} cells {:?}", pk.len(), pk.iter().map(|(x, y)| (x - dx, y - dy)).collect::<Vec<_>>());
    }
    let only: Option<Vec<usize>> = pa.only_pockets.as_ref().map(|s| s.split([',', ';']).filter_map(|t| t.trim().parse().ok()).collect());
    let pal = match &pa.fill_states {
        Some(s) => {
            let mut v = Vec::new();
            for t in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
                v.push(Block::parse(t).map_err(|e| anyhow::anyhow!("--fill-states {t}: {e}"))?);
            }
            v
        }
        None => palette(&pa.fill_palette)?,
    };
    let pal: Vec<Block> = pal.into_iter().filter(|b| !b.is_air()).collect();
    let stone_pal: Vec<Block> = pal.iter().copied().filter(|b| matches!(b, Block::Simple(Simple::Stone) | Block::Simple(Simple::SmoothStone))).collect();
    // ---- per pocket enumeration
    // what the seed's own (remaining) wiring already feeds the inputs, tick by tick: a filling's
    // traces are matched with those ticks taken out (the door's other wiring keeps its share)
    let base_tr: [Vec<u64>; 2] = {
        let mut evs = Vec::new();
        let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        let _ = screen_core(&prob, &seed, &mut evs, &mut traces, true);
        traces
    };
    let own = |tr: &[Vec<u64>; 2]| -> [Vec<u64>; 2] {
        let mut o = tr.clone();
        for ph in 0..2 {
            for (t, r) in o[ph].iter_mut().enumerate() {
                // the base's share, widened by 2 ticks (the mechanism's timing shifts a little
                // when the filling replaces the plan)
                let mut b = 0u64;
                for dt in t.saturating_sub(2)..=t + 2 {
                    b |= base_tr[ph].get(dt).copied().unwrap_or(0);
                }
                *r &= !b;
            }
        }
        o
    };
    let mut accepted: Vec<Vec<Filling>> = Vec::new();
    for (pi, pk) in pockets.iter().enumerate() {
        if only.as_ref().is_some_and(|o| !o.contains(&pi)) || pk.len() > pa.max_pocket {
            println!("pocket {pi}: skipped");
            accepted.push(Vec::new());
            continue;
        }
        let tp = std::time::Instant::now();
        let touches_context = |cells: &[(i32, i32)]| cells.iter().any(|(x, y)| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(ddx, ddy)| rest_occ.contains(&(x + ddx, y + ddy))));
        // work items: (subset of pocket cell indices, lever option, first cell's state when the
        // item is split: subsets of 3+ cells are split by their first cell's state so 150 threads
        // stay busy)
        let mut items: Vec<(Vec<usize>, Option<(i32, i32)>, Option<usize>)> = Vec::new();
        let mut push = |items: &mut Vec<(Vec<usize>, Option<(i32, i32)>, Option<usize>)>, sub: &Vec<usize>, lev: Option<(i32, i32)>| {
            let n0 = if ring.contains(&pk[sub[0]]) { stone_pal.len() } else { pal.len() };
            if sub.len() >= 3 {
                for f in 0..n0 {
                    items.push((sub.clone(), lev, Some(f)));
                }
            } else {
                items.push((sub.clone(), lev, None));
            }
        };
        for sub in subsets_upto(pk.len(), pa.k) {
            let cells: Vec<(i32, i32)> = sub.iter().map(|&i| pk[i]).collect();
            if !connected(&cells) {
                continue;
            }
            if touches_context(&cells) {
                push(&mut items, &sub, None);
            }
            if sub.len() < pa.k {
                for &l in &lever_cands {
                    if cells.contains(&l) {
                        continue;
                    }
                    if cells.iter().any(|q| (q.0 - l.0).abs() + (q.1 - l.1).abs() == 1) {
                        push(&mut items, &sub, Some(l));
                    }
                }
            }
        }
        let total: u64 = items
            .iter()
            .map(|(sub, _, first)| sub.iter().enumerate().map(|(j, &i)| if j == 0 && first.is_some() { 1 } else if ring.contains(&pk[i]) { stone_pal.len() as u64 } else { pal.len() as u64 }).product::<u64>())
            .sum();
        println!("pocket {pi}: {} cells, {} subset/lever items, {} fillings to screen", pk.len(), items.len(), total);
        let next = AtomicUsize::new(0);
        let screened = AtomicU64::new(0);
        let ok_door = AtomicU64::new(0);
        let found: Mutex<std::collections::HashMap<u64, Vec<Filling>>> = Mutex::new(std::collections::HashMap::new());
        std::thread::scope(|s| {
            for _ in 0..a.threads {
                s.spawn(|| {
                    let mut local: std::collections::HashMap<u64, Vec<Filling>> = std::collections::HashMap::new();
                    loop {
                        let ii = next.fetch_add(1, Ordering::Relaxed);
                        if ii >= items.len() {
                            break;
                        }
                        let (sub, lev, first) = &items[ii];
                        let cells: Vec<(i32, i32)> = sub.iter().map(|&i| pk[i]).collect();
                        let pals: Vec<&Vec<Block>> = cells.iter().map(|c| if ring.contains(c) { &stone_pal } else { &pal }).collect();
                        if pals.iter().any(|p| p.is_empty()) {
                            continue;
                        }
                        let mut idx = vec![0usize; cells.len()];
                        // a split item: the first cell's state is fixed, the odometer runs over the rest
                        let j0 = match first {
                            Some(f) => {
                                idx[0] = *f;
                                1
                            }
                            None => 0,
                        };
                        loop {
                            let mut g = seed.clone();
                            for (j, c) in cells.iter().enumerate() {
                                g.set(c.0, c.1, pals[j][idx[j]]);
                            }
                            if let Some((lx, ly)) = lev {
                                g.set(seed.lever.0, seed.lever.1, Block::Air);
                                place_lever(&mut g, *lx, *ly);
                            }
                            let mut evs = Vec::new();
                            let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
                            let sc = screen_core(&prob, &g, &mut evs, &mut traces, true);
                            screened.fetch_add(1, Ordering::Relaxed);
                            // this filling's own share (the seed's remaining wiring taken out)
                            let traces = own(&traces);
                            // the inputs this filling feeds at all (any real power in what ran)
                            let fed: u64 = traces.iter().flatten().fold(0, |acc, r| acc | r) & seed.assist;
                            let mut driven: Option<u64> = None;
                            if sc.pts >= max {
                                ok_door.fetch_add(1, Ordering::Relaxed);
                                driven = match_groups(&prob, &groups, seed.assist, &traces, pa.width_tol).filter(|d| *d != 0);
                            }
                            if fed != 0 {
                                // double drive (the plan's block AND the real wiring on the same
                                // piston) can break a door the filling alone would run: screen
                                // again with the fed inputs released
                                let mut g2 = g.clone();
                                g2.assist = seed.assist & !fed;
                                let mut evs2 = Vec::new();
                                let mut tr2: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
                                let sc2 = screen_core(&prob, &g2, &mut evs2, &mut tr2, true);
                                let tr2 = own(&tr2);
                                screened.fetch_add(1, Ordering::Relaxed);
                                if sc2.pts >= max
                                    && let Some(d2) = match_groups(&prob, &groups, seed.assist, &tr2, pa.width_tol)
                                {
                                    // every group of a released input counts as covered: the door
                                    // ran without its plan
                                    let mut d2 = d2;
                                    for (gi, gr) in groups.iter().enumerate() {
                                        if (fed >> gr.k) & 1 == 1 {
                                            d2 |= 1u64 << gi;
                                        }
                                    }
                                    if driven.is_none_or(|d| (d2 | d) != d) {
                                        driven = Some(d2 | driven.unwrap_or(0));
                                    }
                                }
                            }
                            if let Some(driven) = driven {
                                {
                                    let f = Filling { blocks: cells.iter().enumerate().map(|(j, c)| (*c, pals[j][idx[j]])).collect(), lever: *lev, driven };
                                    let v = local.entry(driven).or_default();
                                    v.push(f);
                                    if v.len() > 4 * pa.keep {
                                        v.sort_by_key(|f| f.cost());
                                        v.truncate(pa.keep);
                                    }
                                }
                            }
                            // next assignment
                            let mut j = j0;
                            loop {
                                if j == idx.len() {
                                    break;
                                }
                                idx[j] += 1;
                                if idx[j] < pals[j].len() {
                                    break;
                                }
                                idx[j] = 0;
                                j += 1;
                            }
                            if j == idx.len() {
                                break;
                            }
                        }
                    }
                    let mut f = found.lock().unwrap();
                    for (k, mut v) in local {
                        let e = f.entry(k).or_default();
                        e.append(&mut v);
                        e.sort_by_key(|f| f.cost());
                        e.truncate(pa.keep);
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        let mut sets: Vec<(&u64, &Vec<Filling>)> = found.iter().collect();
        sets.sort_by_key(|(k, _)| std::cmp::Reverse(k.count_ones()));
        println!(
            "pocket {pi}: {} screened, {} keep the door working, {:.1} s; driven input sets: {}",
            screened.load(Ordering::Relaxed),
            ok_door.load(Ordering::Relaxed),
            tp.elapsed().as_secs_f64(),
            if sets.is_empty() {
                "none".to_string()
            } else {
                sets.iter().map(|(k, v)| format!("{{{}}} x{} (best {} blocks)", (0..64).filter(|i| (**k >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "), v.len(), v[0].cost())).collect::<Vec<_>>().join(", ")
            }
        );
        // the cheapest filling of each driven set, as a design (inspection: --debug-fill)
        for (si, (cov, v)) in sets.iter().enumerate() {
            let f = &v[0];
            let mut g = seed.clone();
            for (q, b) in &f.blocks {
                g.set(q.0, q.1, *b);
            }
            if let Some((lx, ly)) = f.lever {
                place_lever(&mut g, lx, ly);
            }
            let mut d = door_of(&g, prob.spec).to_design(&format!("pocket{pi}-set{si}"));
            d.meta.insert("covers".into(), (0..64).filter(|i| (**cov >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "));
            let _ = d.save(&a.out_dir.join(format!("pocket{pi}-set{si}.txt")));
        }
        let mut all: Vec<Filling> = found.into_values().flatten().collect();
        all.sort_by_key(|f| f.cost());
        accepted.push(all);
    }
    // ---- combinations
    let need: u64 = if groups.len() >= 64 { u64::MAX } else { (1u64 << groups.len()) - 1 };
    let coverable: u64 = accepted.iter().flatten().fold(0, |acc, f| acc | f.driven);
    // groups the remaining real wiring of the seed already drives (a door whose region was
    // erased: its other wiring keeps feeding some of the plan's pulses)
    let base_cov: u64 = {
        let mut evs = Vec::new();
        let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        let _ = screen_core(&prob, &seed, &mut evs, &mut traces, true);
        match_groups(&prob, &groups, seed.assist, &traces, pa.width_tol).unwrap_or(0)
    };
    if base_cov != 0 {
        println!("groups the seed's own wiring already drives: {}", (0..64).filter(|i| (base_cov >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "));
    }
    let coverable = coverable | base_cov;
    if coverable & need != need {
        println!(
            "plan groups no filling drives: {}",
            (0..64).filter(|i| (need >> i) & 1 == 1 && (coverable >> i) & 1 == 0).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" ")
        );
    }
    // the inputs whose every group is covered by a group set
    let released_by = |cov: u64| -> u64 {
        let mut r = 0u64;
        for k in 0..prob.assist_pistons.len() {
            if (seed.assist >> k) & 1 == 0 {
                continue;
            }
            if groups.iter().enumerate().filter(|(_, g)| g.k == k).all(|(gi, _)| (cov >> gi) & 1 == 1) {
                r |= 1u64 << k;
            }
        }
        r
    };
    // combinations: a set of fillings with disjoint cells (at most one moves the lever) covering
    // every input; depth first over the lowest uncovered input, cheapest fillings first; an input
    // no filling drives stays assisted (a true partial when the screen is perfect)
    let cands: Vec<Filling> = {
        let mut v: Vec<Filling> = accepted.iter().flatten().cloned().collect();
        v.sort_by_key(|f| f.cost());
        v
    };
    let mut combos_tried = 0usize;
    let mut best_partial: Option<(u32, Grid)> = None;
    let jopts = cs::JudgeOpts::with_extra(a.judge_extra);
    let mut finds = 0usize;
    let mut verified: HashSet<Vec<usize>> = HashSet::new();
    let mut stack: Vec<(Vec<usize>, u64, u64)> = vec![(Vec::new(), base_cov, 0)];
    while let Some((chosen, cov, skipped)) = stack.pop() {
        if combos_tried >= pa.max_combos || finds >= a.max_finds {
            break;
        }
        let mut key = chosen.clone();
        key.sort();
        if !chosen.is_empty() && verified.insert(key) {
            combos_tried += 1;
            let mut g = seed.clone();
            for &c in &chosen {
                for (q, b) in &cands[c].blocks {
                    g.set(q.0, q.1, *b);
                }
            }
            // the lever last (hung on whatever block the fillings put next to it)
            if let Some((lx, ly)) = chosen.iter().find_map(|&c| cands[c].lever) {
                g.set(g.lever.0, g.lever.1, Block::Air);
                place_lever(&mut g, lx, ly);
            }
            g.assist = seed.assist & !released_by(cov);
            // a released in-plane input's cell holds nothing any more (its rest block was the plan's)
            for k in 0..prob.assist_pistons.len() {
                if (seed.assist >> k) & 1 == 1 && (g.assist >> k) & 1 == 0 {
                    for q in &prob.assist_pos[k] {
                        if q.z == z {
                            g.set(q.x, q.y, Block::Air);
                        }
                    }
                }
            }
            let has_lever = matches!(g.get(g.lever.0, g.lever.1), Block::Lever { .. });
            if g.assist == 0 && !has_lever {
                continue;
            }
            let sc = screen(&prob, &g);
            if sc.pts >= max {
                if g.assist == 0 {
                    let door = door_of(&g, prob.spec);
                    let e = cs::evaluate(&door, &jopts, false);
                    println!("combination {combos_tried}: perfect screen; redsim judge: {}", cs::eval_line("combo", &e, 0.0));
                    // every perfect screen is kept (vjeux 05:07Z: a 1-position pass is a find to
                    // report even if the 4-position judge fails)
                    {
                        let mut d = door.to_design(&format!("pockets-screenpass-{combos_tried}"));
                        d.meta.insert("verdict".into(), cs::eval_line("redsim", &e, 0.0));
                        d.save(&a.out_dir.join(format!("pockets-screenpass-{combos_tried}.txt")))?;
                    }
                    if e.record_ok() || (pa.no_hallway && e.judge.is_ok() && matches!(e.full, Some(Ok(()))) && e.frame == Some(12)) {
                        let vol = e.volume.map(|v| v.0).unwrap_or(0);
                        let name = format!("pockets-{vol}b-{}-{finds}", door.block_count());
                        let d = door.to_design(&name);
                        let path = a.out_dir.join(format!("{name}.txt"));
                        d.save(&path)?;
                        println!("FIND {}", path.display());
                        finds += 1;
                        if a.confirm {
                            let mut w = mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
                            println!("  JVM: {}", jvm_confirm(&mut w, &d, a.judge_extra)?);
                        }
                    }
                } else {
                    let left = g.assist.count_ones();
                    if best_partial.as_ref().is_none_or(|(l, _)| left < *l) {
                        let still: Vec<String> = (0..prob.assist_pistons.len()).filter(|k| (g.assist >> k) & 1 == 1).map(|k| format!("{},{}", prob.assist_pistons[k].0 - dx, prob.assist_pistons[k].1 - dy)).collect();
                        println!("combination {combos_tried}: true partial, {left} inputs still assisted [{}] ({} blocks)", still.join(" "), g.count());
                        best_partial = Some((left, g.clone()));
                        let mut d = door_of(&g, prob.spec).to_design(&format!("pockets-partial-{left}as"));
                        d.meta.insert("assisted".into(), still.join(";"));
                        d.save(&a.out_dir.join(format!("pockets-partial-{left}as.txt")))?;
                    }
                }
            } else {
                // a failing combination: nothing more along this branch
                continue;
            }
        }
        let open = need & !cov & !skipped;
        if open == 0 {
            continue;
        }
        let i = open.trailing_zeros() as usize;
        let used: HashSet<(i32, i32)> = chosen.iter().flat_map(|&c| cands[c].blocks.iter().map(|(q, _)| *q).chain(cands[c].lever)).collect();
        // one lever: fillings may share it when they hang it on the same cell
        let chosen_lever: Option<(i32, i32)> = chosen.iter().find_map(|&c| cands[c].lever);
        let mut kids: Vec<(Vec<usize>, u64, u64)> = Vec::new();
        for (c, f) in cands.iter().enumerate() {
            if (f.driven >> i) & 1 == 0 {
                continue;
            }
            if let (Some(a), Some(b)) = (chosen_lever, f.lever)
                && a != b
            {
                continue;
            }
            let shared = |q: &(i32, i32)| chosen_lever.is_some() && f.lever == chosen_lever && Some(*q) == chosen_lever;
            if f.blocks.iter().map(|(q, _)| *q).chain(f.lever).any(|q| used.contains(&q) && !shared(&q)) {
                continue;
            }
            let mut ch = chosen.clone();
            ch.push(c);
            kids.push((ch, cov | f.driven, skipped));
        }
        // leaving input i assisted is the last resort (explored after every filling for it)
        stack.push((chosen.clone(), cov, skipped | (1u64 << i)));
        for k in kids.into_iter().rev() {
            stack.push(k);
        }
    }
    println!("{combos_tried} combinations verified, {finds} finds, {:.0} s total", t0.elapsed().as_secs_f64());
    Ok(())
}
