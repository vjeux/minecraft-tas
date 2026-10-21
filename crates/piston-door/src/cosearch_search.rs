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
    /// comparator, observer, sticky, piston, lamp, note, target, bulb, obsidian, trapdoor, dropper,
    /// slab, leaves, log, glazed; sculk boards: sculk, calibrated, wool, amethyst) or `all` (every
    /// non-sculk family; `all,sculk,calibrated` adds sensors).
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
    /// Apply the plan's off-plane inputs BETWEEN ticks (the old rule: a plan block appears after
    /// tick t, 1 gt optimistic for observer-driven inputs). Default: the TILE-TICK phase like
    /// `door ideal` / `ideal-rs` (main de6ecbce): the input cell holds a wall torch on a support
    /// one cell further off the plane that turns from a redstone block to stone 2 gt before t, so
    /// the torch lights inside tick t like a wired observer's output. Inputs with an event before
    /// tick 2, more than 3 pulses per edge or a taken support keep the old rule.
    #[arg(long)]
    pub between_ticks: bool,
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
    /// Plan inputs that stay assisted for good (door coordinates `x,y;x,y`): they cost nothing,
    /// are never released and do not block a find (another track wires them): to wire one part
    /// of a door while the rest of its inputs stay ideal.
    #[arg(long)]
    pub pin_assist: Option<String>,
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
    /// An OFF-PLANE lever (vjeux 05:07Z: a lever on a block's front or back face counts as 1
    /// wide): 0 = the lever is in the plane at `lever`; -1 / +1 = it hangs at z-1 / z+1 on the
    /// front / back face of the block at `lever` (that cell holds its support, part of the
    /// wiring), as `lever_block`.
    pub lever_dz: i32,
    pub lever_block: Block,
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
    /// The lever's world position (off the plane for a front / back face lever).
    pub fn lever_pos(&self) -> BlockPos {
        BlockPos::new(self.lever.0, self.lever.1, self.z + self.lever_dz)
    }
    /// The lever block (wherever it hangs), if there is one.
    pub fn lever_state(&self) -> Block {
        if self.lever_dz != 0 { self.lever_block } else { self.get(self.lever.0, self.lever.1) }
    }
    /// Take the lever out (an in-plane lever's cell becomes air; an off-plane lever leaves its
    /// support, and the lever position stays off the plane, empty).
    pub fn remove_lever(&mut self) {
        if self.lever_dz != 0 {
            self.lever_block = Block::Air;
        } else if matches!(self.get(self.lever.0, self.lever.1), Block::Lever { .. }) {
            self.set(self.lever.0, self.lever.1, Block::Air);
        }
    }
    /// The judge's placement order: design blocks sorted by (x, y, z).
    pub fn blocks(&self) -> Vec<(Pos, Block)> {
        let mut v = Vec::new();
        for x in self.gx..self.gx + self.w {
            for y in self.gy..self.gy + self.h {
                let off = self.lever_dz != 0 && (x, y) == self.lever && !self.lever_block.is_air();
                if off && self.lever_dz < 0 {
                    v.push(([x, y, self.z - 1], self.lever_block));
                }
                let b = self.get(x, y);
                if !b.is_air() {
                    v.push(([x, y, self.z], b));
                }
                if off && self.lever_dz > 0 {
                    v.push(([x, y, self.z + 1], self.lever_block));
                }
            }
        }
        v
    }
    pub fn count(&self) -> i32 {
        self.cells.iter().filter(|b| !b.is_air() && !matches!(b, Block::PistonHead { .. })).count() as i32 + (self.lever_dz != 0 && !self.lever_block.is_air()) as i32
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
    /// `--pin-assist`: plan inputs that stay assisted, free (bit k = the plan's piston k).
    pub pin_mask: u64,
    /// Plan input cells IN the door plane: reserved while their input is assisted.
    pub assist_in_plane: HashSet<(i32, i32)>,
    /// Assisted inputs' blocks present at rest (an off-plane level input: its block is placed with
    /// the design while the input stays assisted): (input, position, block).
    pub assist_rest: Vec<(usize, BlockPos, Block)>,
    /// Every input's rest blocks, in the plane too (the plan's state at rest).
    pub assist_rest_all: Vec<(usize, BlockPos, Block)>,
    /// Every position each input's events touch.
    pub assist_pos: Vec<Vec<BlockPos>>,
    /// Tile-tick inputs (the default, `--between-ticks` empties it): input position -> (support
    /// cell one further off the plane, the wall torch's facing). Their events act in the world as
    /// support changes 2 gt early (stone = on, redstone block = off), see `world_events`.
    pub assist_tile: std::collections::HashMap<BlockPos, (BlockPos, redsim::Dir)>,
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
    /// The lever port's cell lies in the box / focus (kept out of `free`: the lever hangs there).
    /// Pockets without --keep-lever removes the seed's lever and gives the cell back as an
    /// ordinary wiring cell (a port named only so the plan parses must not cost a pocket a cell).
    pub lever_cell_free: bool,
    /// The door's volume (`--focus`, else the box): what "outside the box" means for a lever that
    /// is excluded from the volume (the box may be wider to hold plan inputs).
    pub vol: ([i32; 2], [i32; 2]),
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
    // "all" may also stand among other families ("all,sculk,calibrated": the sculk board)
    let spec: String = spec.split(',').map(str::trim).map(|f| if f == "all" { all } else { f }).collect::<Vec<_>>().join(",");
    let all = format!("{all},sculk,calibrated,wool,amethyst");
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
            // sculk (its own board, redsim's sculk.rs): a sensor hears game events (pistons
            // moving, levers, note blocks...) within 8 blocks, wirelessly; active 30 gt then 10 gt
            // cooldown; weak power on every side, strong into the block below
            "sculk" => vec!["sculk_sensor".into()],
            // calibrated: 16 blocks, active 10 gt, no output into its back (facing.opposite()),
            // whose signal (if any) admits only that frequency (piston extend 10, retract 9)
            "calibrated" => ["north", "east", "south", "west"].map(|f| format!("calibrated_sculk_sensor[facing={f}]")).to_vec(),
            // wool hides a vibration whose 6 nudged lines all cross wool
            "wool" => vec!["white_wool".into()],
            // amethyst next to an activated sensor re-emits the vibration (resonate_<frequency>)
            "amethyst" => vec!["amethyst_block".into()],
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
        s.assisted = (g.assist & !p.pin_mask).count_ones() as i32;
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

/// Tile-tick eligibility of the plan's off-plane input positions (as `ideal::Schedule::realize`):
/// every event there a redstone block or a clear at tick >= 2, at most 3 sets per phase (a torch
/// burns out past that), a redstone block or nothing at rest, and a free support cell one further
/// off the plane. Value: (support, the wall torch's facing).
fn tile_inputs(p: &Problem, door_z: i32) -> std::collections::HashMap<BlockPos, (BlockPos, Dir)> {
    let mut out: std::collections::HashMap<BlockPos, (BlockPos, Dir)> = std::collections::HashMap::new();
    let mut all_pos: Vec<BlockPos> = p.assist_pos.iter().flatten().copied().collect();
    all_pos.sort_by_key(|q| (q.x, q.y, q.z));
    all_pos.dedup();
    for q in &all_pos {
        if q.z == door_z {
            continue;
        }
        let dz = (q.z - door_z).signum();
        let s = BlockPos::new(q.x, q.y, q.z + dz);
        let evs: Vec<&(u32, usize, BlockPos, Block)> = p.assist_events.iter().flatten().filter(|e| e.2 == *q).collect();
        let rb_ok = evs.iter().all(|e| e.3.is_air() || matches!(e.3, Block::RedstoneBlock));
        let early = evs.iter().any(|e| e.0 < 2);
        let sets = |ph: usize| p.assist_events[ph].iter().filter(|e| e.2 == *q && !e.3.is_air()).count();
        let taken = all_pos.contains(&s) || out.values().any(|(v, _)| *v == s);
        let rest_ok = p.assist_rest_all.iter().filter(|r| r.1 == *q).all(|r| matches!(r.2, Block::RedstoneBlock));
        if rb_ok && !early && sets(0) <= 3 && sets(1) <= 3 && !taken && rest_ok {
            out.insert(*q, (s, if dz < 0 { Dir::South } else { Dir::North }));
        }
    }
    out
}

/// The still-assisted inputs' blocks at rest (support before torch, so the torch has its wall):
/// a between-ticks input's rest block (`assist_rest`), a tile input's support + wall torch (lit
/// on stone when the plan holds the input ON at rest, unlit on a redstone block otherwise).
fn assist_world_rest(p: &Problem, assist: u64) -> Vec<(BlockPos, Block)> {
    let mut out: Vec<(BlockPos, Block)> = Vec::new();
    for (k, q, b) in &p.assist_rest {
        if (assist >> k) & 1 == 1 && !p.assist_tile.contains_key(q) {
            out.push((*q, *b));
        }
    }
    for (k, ps) in p.assist_pos.iter().enumerate() {
        if (assist >> k) & 1 == 0 {
            continue;
        }
        for q in ps {
            if let Some(&(s, facing)) = p.assist_tile.get(q) {
                if out.iter().any(|(o, _)| o == q) {
                    continue;
                }
                let on = p.assist_rest_all.iter().any(|r| r.0 == k && r.1 == *q);
                out.push((s, if on { Block::Simple(Simple::Stone) } else { Block::RedstoneBlock }));
                out.push((*q, Block::WallTorch { facing, lit: on }));
            }
        }
    }
    out
}

/// A phase's plan events as world writes, by tick: a tile input's set / clear at tick t is its
/// support turning to stone / back to a redstone block at t - 2 (the torch follows inside tick t).
fn world_events(p: &Problem, phase: usize) -> Vec<(u32, usize, BlockPos, Block)> {
    let mut v: Vec<(u32, usize, BlockPos, Block)> = p.assist_events[phase]
        .iter()
        .map(|&(t, k, q, b)| match p.assist_tile.get(&q) {
            Some(&(s, _)) => (t.saturating_sub(2), k, s, if b.is_air() { Block::RedstoneBlock } else { Block::Simple(Simple::Stone) }),
            None => (t, k, q, b),
        })
        .collect();
    v.sort_by_key(|e| e.0);
    v
}

/// One phase after a flip: the assisted pistons' plan inputs applied at their ticks (before the
/// tick, like `door ideal`), ticking until idle (at least once) and until every input was
/// applied; `on_tick` sees the world after every tick. Returns whether it went idle. With
/// `traces`, what the real wiring feeds each input (bit k) is recorded per tick (index 0 = right
/// after the flip, i = after tick i).
fn run_phase(p: &Problem, g: &Grid, w: &mut World, phase: usize, traces: Option<&mut Vec<u64>>, mut on_tick: impl FnMut(&World)) -> bool {
    let evs = world_events(p, phase);
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
    screen_core_w(p, g, evs, traces, measure, None)
}

/// `screen_core`, also handing back the grid's cells as they settled after placement (when the
/// build went idle): a filling placed unpowered next to powered wiring (a lever ON at rest) can
/// be retried with its settled states (`pockets --no-warm` turns that off).
fn screen_core_w(p: &Problem, g: &Grid, evs: &mut Vec<Event>, traces: &mut [Vec<u64>; 2], measure: bool, settled_out: Option<&mut Vec<Block>>) -> Score {
    let max = 11 + p.cycles as i32 * 34;
    let blocks = g.count();
    let mut pts = 0;
    let list = g.blocks();
    let spec = p.spec;
    let (wlo, whi) = cs::watch_box(&list, &spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
    let mut placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
    // still-assisted off-plane inputs: their blocks at rest (level inputs; tile inputs' torches)
    placed.extend(assist_world_rest(p, g.assist));
    w.place_like_judge(&placed);
    let (_, idle) = cs::step_until_idle(&mut w, p.max_ticks, |_| {});
    if !idle || !w.errors.is_empty() {
        return Score { pts: 0, max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
    }
    if let Some(out) = settled_out {
        out.clear();
        for j in 0..g.h {
            for i in 0..g.w {
                out.push(w.get(BlockPos::new(g.gx + i, g.gy + j, g.z)));
            }
        }
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
    let lever = g.lever_pos();
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
    // the ring of plane cells just outside the box (the lever excepted): the community volume
    // is the bounding box over EVERY tick of the cycle, so a piston head or a pushed block that
    // pokes out of the box mid-cycle makes the door bigger than the box (the open-state check
    // below does not see it)
    let halo: Vec<BlockPos> = {
        let mut v = Vec::new();
        for y in p.lo[1] - 1..=p.hi[1] + 1 {
            for x in p.lo[0] - 1..=p.hi[0] + 1 {
                let q = BlockPos::new(x, y, z);
                // (an in-plane plan input's cell holds the plan's own block, not the build's)
                if !in_box(p, x, y) && q != lever && !p.assist_in_plane.contains(&(x, y)) {
                    v.push(q);
                }
            }
        }
        v
    };
    let mut escaped = false;
    for cycle in 0..p.cycles {
        w.use_block(lever);
        let rec = cycle == 0 && !rec_cells.is_empty();
        let mut prev = if rec { piston_cells(&w, &rec_cells) } else { Vec::new() };
        let mut t = 0;
        let measure = cycle == 0 && measure;
        let idle = run_phase(p, g, &mut w, 0, if measure { Some(&mut traces[0]) } else { None }, |w| {
            if !escaped && halo.iter().any(|q| !w.get(*q).is_air()) {
                escaped = true;
            }
            if rec {
                t += 1;
                tick_events(&prev, w, 0, t, evs);
                prev = piston_cells(w, &rec_cells);
            }
        });
        if escaped {
            return Score { pts: (pts - 2).min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
        }
        let open_n = p.door_cells.iter().filter(|(x, y)| {
            let s = w.get(at(*x, *y));
            !cs::collides(&s) && !cs::is_moving(&s)
        }).count() as i32;
        pts += open_n;
        if open_n >= 7 {
            // the frame: the 12 cells around the opening must hold plain full blocks when open
            // (door blocks or static quartz-like blocks: vjeux 03:36Z, FULL FRAME)
            let clean = p.ring.iter().filter(|(x, y)| cs::frame_filled_door(&w.get(at(*x, *y)), &p.door_block)).count() as i32;
            pts += clean;
            // nothing of the build outside the box
            let outside = plane_cells(p, z).iter().filter(|q| !in_box(p, q.x, q.y) && *q != &lever && !p.assist_in_plane.contains(&(q.x, q.y)) && !w.get(**q).is_air()).count() as i32;
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
            if !escaped && halo.iter().any(|q| !w.get(*q).is_air()) {
                escaped = true;
            }
            if rec {
                t += 1;
                tick_events(&prev, w, 1, t, evs);
                prev = piston_cells(w, &rec_cells);
            }
        });
        if escaped {
            return Score { pts: (pts - 2).min(max - 1), max, blocks, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 };
        }
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
            let clean = p.ring.iter().filter(|(x, y)| cs::frame_filled_door(&w.get(at(*x, *y)), &p.door_block)).count();
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
        Block::Sculk { calibrated: true, phase, power, .. } => Block::Sculk { calibrated: true, facing: [Dir::North, Dir::East, Dir::South, Dir::West][rng.below(4)], phase, power },
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
        if (p.pin_mask >> k) & 1 == 1 {
            return None;
        }
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
    let mut g = Grid { gx, gy, w, h, z, cells: vec![Block::Air; (w * h) as usize], lever: (door.spec.lever[0], door.spec.lever[1]), assist: 0, lever_dz: 0, lever_block: Block::Air };
    let lever = door.spec.lever;
    for (q, b) in &door.blocks {
        if *q == lever && q[2] != z {
            // a lever on a block's front or back face (vjeux 05:07Z: still 1 wide)
            anyhow::ensure!((q[2] - z).abs() == 1, "an off-plane lever must hang on a block of the plane: {q:?}");
            g.lever_dz = q[2] - z;
            g.lever_block = *b;
            continue;
        }
        anyhow::ensure!(q[2] == z, "not flat: {q:?}");
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
    let powered = matches!(g.lever_state(), Block::Lever { powered: true, .. });
    if g.lever_dz != 0 {
        g.remove_lever();
    }
    place_lever_p(g, x, y, powered);
}

/// Hang the lever at (x, y, dz): dz = 0 in the plane (`place_lever_p`), dz = -1 / +1 on the
/// front / back face of the block at (x, y) (a wall lever facing away from it). The old lever goes.
fn place_lever_at(g: &mut Grid, x: i32, y: i32, dz: i32, powered: bool) {
    use redsim::block::Face;
    g.remove_lever();
    if dz == 0 {
        place_lever_p(g, x, y, powered);
    } else {
        g.lever = (x, y);
        g.lever_dz = dz.signum();
        g.lever_block = Block::Lever { face: Face::Wall, facing: if dz < 0 { Dir::North } else { Dir::South }, powered };
    }
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
    g.lever_dz = 0;
    g.lever_block = Block::Air;
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
    let powered = matches!(g.lever_state(), Block::Lever { powered: true, .. });
    g.remove_lever();
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
    if p.pin_mask != 0 {
        // pinned inputs stay plan-driven: the door alone cannot be judged here; save it for
        // `door ideal` with a plan holding only the pinned inputs
        let k = sh.finds.fetch_add(1, Ordering::Relaxed);
        let name = format!("cosearch-pinned-{}-{k}", door.block_count());
        let d = door.to_design(&name);
        let path = a.out_dir.join(format!("{name}.txt"));
        let _ = std::fs::create_dir_all(&a.out_dir);
        let _ = d.save(&path);
        let msg = format!("pinned find (screen perfect with the pinned inputs ideal): {}", path.display());
        println!("{msg}");
        sh.log.lock().unwrap().push(msg);
        if k + 1 >= a.max_finds {
            sh.stop.store(true, Ordering::Relaxed);
        }
        return;
    }
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
    let lp = g.lever_pos();
    spec.lever = [lp.x, lp.y, lp.z];
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
    let lever_xy = (lever_pos[0], lever_pos[1]);
    let mut free = Vec::new();
    for x in flo[0]..=fhi[0] {
        for y in flo[1]..=fhi[1] {
            if in_box_raw(lo, hi, x, y) && !door_cells.contains(&(x, y)) {
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
        free = want.into_iter().filter(|(x, y)| in_box_raw(lo, hi, *x, *y) && !door_cells.contains(&(*x, *y))).collect();
    }
    // the lever's own cell is no wiring cell (pockets without --keep-lever gives it back)
    let lever_cell_free = free.contains(&lever_xy);
    free.retain(|c| *c != lever_xy);
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
        pin_mask: 0,
        assist_in_plane: HashSet::new(),
        assist_rest: Vec::new(),
        assist_rest_all: Vec::new(),
        assist_pos: Vec::new(),
        assist_tile: std::collections::HashMap::new(),
        assist_recv: Vec::new(),
        drive_weight: a.drive_weight,
        drive_slack: a.drive_slack,
        drive_max: Vec::new(),
        no_lever: false,
        lever_cell_free,
        vol: (flo, fhi),
        exact_rest: a.exact_rest,
    };
    let mut prob = prob;
    if let Some(r) = &a.reference {
        let (_, rd) = cs::load_door(r)?;
        anyhow::ensure!(rd.spec.door == spec0.door, "the reference needs the same door port");
        let g = Grid { gx: 0, gy: 0, w: 0, h: 0, z: rd.spec.door[2], cells: Vec::new(), lever: (0, 0), assist: 0, lever_dz: 0, lever_block: Block::Air };
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
        if !a.between_ticks {
            prob.assist_tile = tile_inputs(&prob, spec0.door[2]);
            let n_pos: usize = prob.assist_pos.iter().map(|v| v.iter().filter(|q| q.z != spec0.door[2]).count()).sum();
            if prob.assist_tile.len() < n_pos {
                println!("tile-tick inputs: {} of {} off-plane input positions (the others act between ticks: an event before tick 2, > 3 pulses per edge or a taken support)", prob.assist_tile.len(), n_pos);
            }
        }
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
        if let Some(cells) = &a.pin_assist {
            for t in cells.split(';').map(str::trim).filter(|t| !t.is_empty()) {
                let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                anyhow::ensure!(v.len() == 2, "--pin-assist x,y;x,y");
                match prob.assist_pistons.iter().position(|c| *c == (v[0] + dx, v[1] + dy)) {
                    Some(k) => prob.pin_mask |= 1u64 << k,
                    None => bail!("--pin-assist: no plan input at {t}"),
                }
            }
            println!("pinned (stay ideal, free): {cells}");
        }
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
        // in-plane plan inputs ON at rest (a mechanism file without its ideal inputs): their
        // redstone blocks join the seed (released inputs clear them in the combinations)
        for (_, q, b) in &prob.assist_rest_all {
            if q.z == g.z && g.get(q.x, q.y).is_air() {
                g.set(q.x, q.y, *b);
            }
        }
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
            g.assist |= prob.pin_mask;
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
    /// Per-pocket `--k` (repeatable, `x,y=K`: the pocket holding door cell (x,y) takes fillings
    /// of up to K blocks): cross-region runs where one pocket needs a deeper search than another.
    #[arg(long)]
    pub pocket_k: Vec<String>,
    /// Per-pocket palette (repeatable, `x,y=<families>` as in --fill-palette, or
    /// `x,y=states:<states ;-separated>` as in --fill-states): the pocket holding door cell (x,y)
    /// uses it instead of the global palette.
    #[arg(long)]
    pub pocket_fill: Vec<String>,
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
    /// ISLAND START: also print, per pocket that never starts, the changes next to it before its
    /// first forced pulse and the inputs causing them (`*` its own, `?` not started yet).
    #[arg(long)]
    pub why_islands: bool,
    /// ISLAND START with SCULK (the sculk board): any pocket may hold a sensor, which starts it
    /// from a vibration within 8 blocks — a piston moving anywhere (caused by started pockets) or
    /// the lever's flip — at the vibration's tick + its travel time (floor of the distance) + 1.
    #[arg(long)]
    pub sculk_islands: bool,
    /// Keep the seed's real lever (its wiring is already real): no lever candidates.
    #[arg(long)]
    pub keep_lever: bool,
    /// Rest polarity of the lever candidates: `seed` (the seed lever's, off without one), `off`,
    /// `on` or `both` (the judge accepts either: vjeux 2026-10-19).
    #[arg(long, default_value = "seed")]
    pub lever_polarity: String,
    /// No OFF-PLANE lever candidates (by default a lever may also hang on the front face of a
    /// filling block or of a static block next to the filling: still 1 wide, vjeux 05:07Z).
    #[arg(long)]
    pub no_lever_front: bool,
    /// No warm retry (by default a filling whose blocks change state while the build settles,
    /// e.g. dust next to a lever ON at rest, is screened again placed in its settled states).
    #[arg(long)]
    pub no_warm: bool,
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
    /// Fillings may have GAPS (several pieces, each touching the build), not only one connected
    /// piece: an observer powering a quasi cell through air (c88 s26's BR: observer (4,0) up ->
    /// air (4,-1) -> piston (4,-2) up, QC'd), a sculk sensor next to a separate wool block.
    #[arg(long)]
    pub gaps: bool,
    /// `--region-cells` may erase RETRACTED pistons too (wiring pistons that move no door block, e.g.
    /// s27-trbl51's sticky piston (3,5) whose head pops for an observer); heads, extended pistons,
    /// levers and door cells stay refused.
    #[arg(long)]
    pub region_pistons: bool,
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

/// A lever candidate: (x, y, dz, powered at rest); dz = 0 in the plane, -1 on the front face of
/// the block at (x, y).
type LeverAt = (i32, i32, i32, bool);

/// A set of plan groups (bit = index in the run's `groups`): u128, so plans with up to 128
/// pulse groups screen (c70b's 6x7 mech-synth plans have 69; inputs stay u64 masks).
type GMask = u128;

#[derive(Clone, Debug)]
struct Filling {
    blocks: Vec<((i32, i32), Block)>,
    lever: Option<LeverAt>,
    driven: GMask,
}

impl Filling {
    fn cost(&self) -> usize {
        self.blocks.len() + self.lever.is_some() as usize
    }
    /// The plane cell the lever occupies (an off-plane lever occupies none).
    fn lever_cell(&self) -> Option<(i32, i32)> {
        self.lever.filter(|l| l.2 == 0).map(|l| (l.0, l.1))
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
/// Per plan input: the pockets touching its piston directly or through the cell above it, also
/// through a static conductor next to the piston / that cell (a pocket powers the block).
fn pocket_feeders(p: &Problem, pockets: &[Vec<(i32, i32)>], statics: &[(i32, i32)]) -> Vec<Vec<usize>> {
    let n4 = |x: i32, y: i32| [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)];
    p.assist_pistons
        .iter()
        .map(|&(px, py)| {
            let mut v: Vec<usize> = Vec::new();
            for c in n4(px, py).into_iter().chain(n4(px, py + 1).into_iter().filter(|c| *c != (px, py))) {
                if let Some(i) = pockets.iter().position(|pk| pk.contains(&c)) {
                    v.push(i);
                } else if statics.contains(&c) {
                    v.extend(n4(c.0, c.1).into_iter().filter_map(|d| pockets.iter().position(|pk| pk.contains(&d))));
                }
            }
            v.sort();
            v.dedup();
            v
        })
        .collect()
}

/// POCKET LOAD (after the feed report): per pocket, the plan groups it must make. An input whose
/// piston touches (directly or through the cell above it) ONE pocket only is that pocket's forced
/// load; inputs it can share with another pocket are listed apart. A filling sees only its own
/// neighbourhood, so every distinct pulse tick of its load needs its own event source: a 1-3-cell
/// pocket with 6 forced ticks is a wall before any search runs (the sub-56 screens, 10-20).
fn pocket_load_report(p: &Problem, pockets: &[Vec<(i32, i32)>], statics: &[(i32, i32)], groups: &[PGroup], names: &[String]) {
    let feeders = pocket_feeders(p, pockets, statics);
    let ticks = |ks: &[usize]| -> (Vec<u32>, Vec<u32>) {
        let mut t: [Vec<u32>; 2] = [Vec::new(), Vec::new()];
        for g in groups.iter().filter(|g| ks.contains(&g.k)) {
            t[g.phase].push(g.t);
        }
        for v in t.iter_mut() {
            v.sort();
            v.dedup();
        }
        let [o, c] = t;
        (o, c)
    };
    let fmt = |v: &[u32]| v.iter().map(|t| t.to_string()).collect::<Vec<_>>().join(" ");
    for i in 0..pockets.len() {
        let has = |k: &usize| groups.iter().any(|g| g.k == *k);
        // (--pin-assist inputs stay ideal: nobody's load)
        let pinned = |k: &usize| (p.pin_mask >> *k) & 1 == 1;
        let forced: Vec<usize> = (0..feeders.len()).filter(|k| feeders[*k] == [i] && has(k) && !pinned(k)).collect();
        let shared: Vec<usize> = (0..feeders.len()).filter(|k| feeders[*k].len() > 1 && feeders[*k].contains(&i) && has(k)).collect();
        if forced.is_empty() && shared.is_empty() {
            continue;
        }
        let (o, c) = ticks(&forced);
        let label = |ks: &[usize]| ks.iter().map(|k| names[*k].clone()).collect::<Vec<_>>().join(" ");
        println!(
            "  load of pocket {i} ({} cells): forced [{}] -> {} distinct ticks (open {} | close {}){}",
            pockets[i].len(),
            label(&forced),
            o.len() + c.len(),
            fmt(&o),
            fmt(&c),
            if shared.is_empty() { String::new() } else { format!("; can also feed [{}]", label(&shared)) }
        );
    }
}

/// The plan replayed on `g` (its assisted inputs only): the given cells' states at rest after the
/// flip (index 0) and after every tick, per phase. `suppress` = (input, phase): that input's
/// events are skipped in that phase only (its rest state and the other phase stay), so a change
/// that disappears was caused by one of its events before it. None when the build does not settle.
fn replay_cells(p: &Problem, g: &Grid, cells: &[BlockPos], suppress: Option<(usize, usize)>) -> Option<[Vec<Vec<Block>>; 2]> {
    let list = g.blocks();
    let (wlo, whi) = cs::watch_box(&list, &p.spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
    let mut placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
    placed.extend(assist_world_rest(p, g.assist));
    w.place_like_judge(&placed);
    let (_, idle) = cs::step_until_idle(&mut w, p.max_ticks, |_| {});
    if !idle {
        return None;
    }
    let lever = g.lever_pos();
    let snap = |w: &World| cells.iter().map(|q| w.get(*q)).collect::<Vec<Block>>();
    let mut out: [Vec<Vec<Block>>; 2] = [Vec::new(), Vec::new()];
    for (phase, o) in out.iter_mut().enumerate() {
        w.use_block(lever);
        o.push(snap(&w));
        // run_phase's loop, skipping the suppressed input's events in this phase
        let evs = world_events(p, phase);
        let (mut i, mut t) = (0usize, 0u32);
        loop {
            while i < evs.len() && evs[i].0 == t {
                let (_, k, pos, b) = evs[i];
                if (g.assist >> k) & 1 == 1 && suppress != Some((k, phase)) {
                    w.set_block_and_update(pos, b);
                }
                i += 1;
            }
            w.tick();
            t += 1;
            o.push(snap(&w));
            if (i >= evs.len() && w.idle()) || t >= p.max_ticks {
                break;
            }
        }
    }
    Some(out)
}

/// ISLAND START (after the load report; clk-misc's islands, 10-20): a pocket's wiring can only
/// start from a change it can observe (a cell next to it that changes: a piston's base or head, a
/// moved block) or from the lever hanging in it; pockets joined by a static conductor count as one
/// island. Every change next to each pocket is attributed to the plan inputs it depends on (the
/// replay without that one input has no change there at that tick). A pocket starts on an edge
/// when some change next to it, no later than its first forced pulse can come (its window's far
/// end), depends only on inputs fed by pockets already started and on none of its own forced
/// inputs; the lever's pocket starts at the flip. Optimistic on purpose (no observer delay, a
/// change no single input explains counts as available), so a NEVER is a wall: printed per lever
/// pocket (u0/u2-7x7: the right pockets see only their own pistons).
/// `show` = (input labels, the door offset to print door coordinates, --why-islands, --sculk-islands).
fn island_start_report(p: &Problem, seed: &Grid, pockets: &[Vec<(i32, i32)>], statics: &[(i32, i32)], groups: &[PGroup], show: (&[String], (i32, i32), bool, bool)) {
    let (names, (dx, dy), why, sculk) = show;
    let feeders = pocket_feeders(p, pockets, statics);
    // --pin-assist inputs (kept ideal: "any source") are given: available from the flip, nobody's
    // load. Flip-time inputs (every pulse at the flip, both edges: a lever-held level) are given
    // only with the lever OUTSIDE the pockets (on the pistons' backs, gadget 1); with the lever in
    // a pocket they are ordinary inputs (that pocket starts at the flip and feeds what it touches)
    let pinned: Vec<bool> = (0..feeders.len()).map(|k| (p.pin_mask >> k) & 1 == 1).collect();
    let flip_time: Vec<bool> = (0..feeders.len()).map(|k| groups.iter().filter(|g| g.k == k).all(|g| g.t == 0) && groups.iter().any(|g| g.k == k)).collect();
    let forced_of = |given: &[bool]| -> Vec<Vec<usize>> { (0..pockets.len()).map(|i| (0..feeders.len()).filter(|k| feeders[*k] == [i] && !given[*k]).collect()).collect() };
    // the latest tick each pocket's first forced pulse may come (its window's far end): a trigger
    // after it is too late on that edge
    let first_of = |forced: &[Vec<usize>]| -> Vec<[Option<u32>; 2]> {
        forced
            .iter()
            .map(|ks| {
                let f = |ph: usize| groups.iter().filter(|g| g.phase == ph && ks.contains(&g.k)).map(|g| (g.t as i32 + g.hi.max(0)) as u32).min();
                [f(0), f(1)]
            })
            .collect()
    };
    let given_out: Vec<bool> = {
        // a lever OUTSIDE the pockets holds only LEVELS (one change per edge, at the flip) of the
        // pistons it touches: beside the lever's cell (the design's lever port), or by QC (the
        // cell above the piston beside it) — the attached piston and the one below it (gadget 1)
        let lv = (p.spec.lever[0], p.spec.lever[1]);
        let touches = |c: (i32, i32)| (c.0 - lv.0).abs() + (c.1 - lv.1).abs() == 1;
        (0..feeders.len())
            .map(|k| {
                let b = p.assist_pistons[k];
                let level = groups.iter().filter(|g| g.k == k).all(|g| g.width.is_none());
                pinned[k] || (flip_time[k] && level && (touches(b) || touches((b.0, b.1 + 1))))
            })
            .collect()
    };
    let (forced_in, forced_out) = (forced_of(&pinned), forced_of(&given_out));
    let (first_in, first_out) = (first_of(&forced_in), first_of(&forced_out));
    // the cells next to each pocket
    let n4 = |x: i32, y: i32| [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)];
    let mut cells: Vec<(i32, i32)> = Vec::new();
    let near: Vec<Vec<usize>> = pockets
        .iter()
        .map(|pk| {
            let mut v = Vec::new();
            for &(x, y) in pk {
                for c in n4(x, y) {
                    if pk.contains(&c) {
                        continue;
                    }
                    let ci = cells.iter().position(|q| *q == c).unwrap_or_else(|| {
                        cells.push(c);
                        cells.len() - 1
                    });
                    if !v.contains(&ci) {
                        v.push(ci);
                    }
                }
            }
            v
        })
        .collect();
    let pos: Vec<BlockPos> = cells.iter().map(|(x, y)| BlockPos::new(*x, *y, seed.z)).collect();
    let Some(base) = replay_cells(p, seed, &pos, None) else {
        println!("  island start: the seed does not settle");
        return;
    };
    let inputs: Vec<usize> = (0..p.assist_pistons.len()).filter(|k| (seed.assist >> k) & 1 == 1).collect();
    // per input and phase: the replay with that input's events in that phase suppressed (its
    // rest state kept: a level held at rest stays), so only its earlier events can explain a change
    let without: Vec<[Option<Vec<Vec<Block>>>; 2]> = inputs
        .iter()
        .map(|&k| {
            let r = |ph: usize| replay_cells(p, seed, &pos, Some((k, ph))).map(|mut r| std::mem::take(&mut r[ph]));
            [r(0), r(1)]
        })
        .collect();
    // the changes next to the pockets: (cell, phase, tick, inputs it depends on)
    let mut changes: Vec<(usize, usize, u32, Vec<usize>)> = Vec::new();
    for ph in 0..2 {
        for t in 1..base[ph].len() {
            for ci in 0..cells.len() {
                if base[ph][t][ci] == base[ph][t - 1][ci] {
                    continue;
                }
                // an observer only needs SOME change of the cell at that tick
                let moved = |r: &[Vec<Block>]| matches!((r.get(t), r.get(t - 1)), (Some(a), Some(b)) if a[ci] != b[ci]);
                let deps: Vec<usize> = inputs.iter().zip(&without).filter(|(_, r)| r[ph].as_ref().is_some_and(|r| !moved(r))).map(|(k, _)| *k).collect();
                changes.push((ci, ph, t as u32, deps));
            }
        }
    }
    let mut all_start: Vec<String> = Vec::new();
    // sculk: every piston vibration of the replay, with its causes
    let vib = if sculk { piston_vibrations(p, seed, &inputs) } else { Vec::new() };
    // islands: pockets joined by a static conductor next to both (a block that never changes:
    // power crosses it, from a lever on its face or a dust / repeater / torch into it)
    let mut root: Vec<usize> = (0..pockets.len()).collect();
    fn find(root: &mut [usize], mut a: usize) -> usize {
        while root[a] != a {
            a = root[a];
        }
        a
    }
    for ci in 0..cells.len() {
        let s0 = base[0][0][ci];
        let constant = base.iter().all(|ph| ph.iter().all(|s| s[ci] == s0));
        if !constant || matches!(s0, Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. }) || !redsim::behavior::conductor_of(&s0) {
            continue;
        }
        let qs: Vec<usize> = (0..pockets.len()).filter(|q| near[*q].contains(&ci)).collect();
        for w in qs.windows(2) {
            let (a, b) = (find(&mut root, w[0]), find(&mut root, w[1]));
            if a != b {
                root[a] = b;
            }
        }
    }
    let roots: Vec<usize> = (0..pockets.len()).map(|q| find(&mut root, q)).collect();
    if roots.iter().enumerate().any(|(q, r)| *r != q) {
        let mut groups_of: Vec<Vec<usize>> = Vec::new();
        for r in 0..pockets.len() {
            let g: Vec<usize> = (0..pockets.len()).filter(|q| roots[*q] == r).collect();
            if g.len() > 1 {
                groups_of.push(g);
            }
        }
        println!("  island start: pockets joined through a static conductor: {groups_of:?}");
    }
    // pockets mode places the lever in a pocket (one line per choice) or outside the pockets (only
    // the lever-time inputs then); --keep-lever keeps the seed's own lever, whose effects are the
    // changes no plan input explains
    let choices: Vec<Option<usize>> = if p.no_lever { (0..pockets.len()).map(Some).chain(std::iter::once(None)).collect() } else { vec![None] };
    for lever in choices {
        // flip-time inputs are given only with the lever outside the pockets (see above)
        let outside = lever.is_none() && p.no_lever;
        let (forced, first, given): (&Vec<Vec<usize>>, &Vec<[Option<u32>; 2]>, &Vec<bool>) = if outside { (&forced_out, &first_out, &given_out) } else { (&forced_in, &first_in, &pinned) };
        // each edge on its own: a pocket must start on every edge where it has forced pulses
        let mut started: [Vec<bool>; 2] = std::array::from_fn(|_| (0..pockets.len()).map(|q| lever.is_some_and(|l| roots[q] == roots[l])).collect());
        // the change that started each pocket on each edge (index into `changes`), or the sculk
        // vibration that did (a description)
        let mut trig: [Vec<Option<usize>>; 2] = std::array::from_fn(|_| vec![None; pockets.len()]);
        let mut strig: [Vec<Option<String>>; 2] = std::array::from_fn(|_| vec![None; pockets.len()]);
        // the lever's cell, for sensors hearing its flip: in its pocket (the cell nearest to the
        // listener is taken per pocket below) or the design's lever port
        let lever_cells: Vec<(i32, i32)> = match lever {
            Some(l) => pockets[l].clone(),
            None => vec![(p.spec.lever[0], p.spec.lever[1])],
        };
        for ph in 0..2 {
            loop {
                let avail: Vec<usize> = (0..feeders.len()).filter(|k| given[*k] || feeders[*k].iter().any(|q| started[ph][*q])).collect();
                let mut grew = false;
                for q in 0..pockets.len() {
                    if started[ph][q] {
                        continue;
                    }
                    // (a change no single plan input explains comes from the seed's own wiring or
                    // lever; a pushed power source can act on its own tick, so no observer delay; a
                    // pocket with no forced pulse on this edge starts on any trigger: it may still
                    // feed shared inputs)
                    let tf = first[q][ph].unwrap_or(u32::MAX);
                    let hit = changes.iter().position(|(ci, cph, t, deps)| *cph == ph && near[q].contains(ci) && *t <= tf && deps.iter().all(|k| avail.contains(k) && !forced[q].contains(k)));
                    if let Some(h) = hit {
                        trig[ph][q] = Some(h);
                    }
                    // sculk: a sensor in the pocket hears the lever's flip or a piston moving
                    // within 8 blocks whose causes are available (not its own outputs)
                    let mut heard = None;
                    if sculk && hit.is_none() {
                        heard = lever_cells.iter().filter_map(|c| sculk_hears(&pockets[q], *c, 0)).min().filter(|s| *s <= tf).map(|s| format!("sensor hears the lever by t{s}"));
                        if heard.is_none() {
                            heard = vib
                                .iter()
                                .filter(|(_, cph, _, deps)| *cph == ph && deps.iter().all(|k| avail.contains(k) && !forced[q].contains(k)))
                                .filter_map(|(c, _, t, _)| sculk_hears(&pockets[q], *c, *t).filter(|s| *s <= tf).map(|s| (s, *c)))
                                .min()
                                .map(|(s, c)| format!("sensor hears the piston at ({},{}) by t{s}", c.0 - dx, c.1 - dy));
                        }
                        strig[ph][q] = heard.clone();
                    }
                    let ok = hit.is_some() || heard.is_some();
                    if ok {
                        for r in 0..pockets.len() {
                            if roots[r] == roots[q] {
                                started[ph][r] = true;
                            }
                        }
                        grew = true;
                    }
                }
                if !grew {
                    break;
                }
            }
        }
        let never: Vec<(usize, usize)> = (0..pockets.len()).flat_map(|q| (0..2).map(move |ph| (q, ph))).filter(|&(q, ph)| first[q][ph].is_some() && !started[ph][q]).collect();
        let label = match lever {
            Some(l) => format!("lever in pocket {l}"),
            None if p.no_lever => "lever outside the pockets (its lever-time inputs only)".to_string(),
            None => "the seed's lever".to_string(),
        };
        if never.is_empty() {
            all_start.push(label.clone());
        }
        println!(
            "  island start, {label}: {}",
            if never.is_empty() {
                "every pocket starts on both edges".to_string()
            } else {
                format!("NEVER start: {}", never.iter().map(|&(q, ph)| format!("pocket {q} on {} (first forced pulse by t{})", if ph == 0 { "open" } else { "close" }, first[q][ph].unwrap_or(0))).collect::<Vec<_>>().join(", "))
            }
        );
        // why: per pocket and edge that fail, the first changes next to it (cell, tick, the inputs
        // whose events cause it: `*` = the pocket's own, `?` = not started on that edge); and per
        // edge the change that starts each started pocket (where its first observer goes)
        if why {
            for ph in 0..2 {
                let s: Vec<String> = (0..pockets.len())
                    .filter_map(|q| {
                        if let Some(s) = &strig[ph][q] {
                            return Some(format!("pocket {q}: {s}"));
                        }
                        let (ci, _, t, deps) = &changes[trig[ph][q]?];
                        let d = deps.iter().map(|k| names[*k].clone()).collect::<Vec<_>>().join(" ");
                        Some(format!("pocket {q} by t{t} ({},{}) [{}]", cells[*ci].0 - dx, cells[*ci].1 - dy, if d.is_empty() { "lever / own wiring".to_string() } else { d }))
                    })
                    .collect();
                println!("    starts on {}: {}", if ph == 0 { "open" } else { "close" }, s.join(", "));
            }
        }
        if why {
            for &(q, ph) in &never {
                let avail: Vec<usize> = (0..feeders.len()).filter(|k| given[*k] || feeders[*k].iter().any(|r| started[ph][*r])).collect();
                let tf = first[q][ph].unwrap_or(u32::MAX);
                let seen: Vec<String> = changes
                    .iter()
                    .filter(|(ci, cph, t, _)| *cph == ph && near[q].contains(ci) && *t <= tf)
                    .take(6)
                    .map(|(ci, _, t, deps)| {
                        let d = deps.iter().map(|k| format!("{}{}", names[*k], if forced[q].contains(k) { "*" } else if avail.contains(k) { "" } else { "?" })).collect::<Vec<_>>().join(" ");
                        format!("t{t} ({},{}) [{d}]", cells[*ci].0 - dx, cells[*ci].1 - dy)
                    })
                    .collect();
                println!("    pocket {q} {}: changes next to it by t{tf}: {}", if ph == 0 { "open" } else { "close" }, if seen.is_empty() { "none".to_string() } else { seen.join(", ") });
            }
        }
    }
    if p.no_lever {
        println!("  island start: lever placements that start every pocket: {}", if all_start.is_empty() { "NONE (no static wiring can run this plan)".to_string() } else { all_start.join("; ") });
    }
}

/// Sculk islands: the piston vibrations of the plan's replay (a piston base starting to move,
/// anywhere in the plane: `(cell, phase, tick, the inputs whose events cause it)`).
fn piston_vibrations(p: &Problem, seed: &Grid, inputs: &[usize]) -> Vec<((i32, i32), usize, u32, Vec<usize>)> {
    let pos: Vec<BlockPos> = plane_cells(p, seed.z);
    let Some(base) = replay_cells(p, seed, &pos, None) else { return Vec::new() };
    let without: Vec<[Option<Vec<Vec<Block>>>; 2]> = inputs
        .iter()
        .map(|&k| {
            let r = |ph: usize| replay_cells(p, seed, &pos, Some((k, ph))).map(|mut r| std::mem::take(&mut r[ph]));
            [r(0), r(1)]
        })
        .collect();
    let base_moves = |a: &Block, b: &Block| a != b && (matches!(a, Block::Piston { .. }) || matches!(b, Block::Piston { .. }));
    let mut v = Vec::new();
    for ph in 0..2 {
        for t in 1..base[ph].len() {
            for (ci, q) in pos.iter().enumerate() {
                if !base_moves(&base[ph][t - 1][ci], &base[ph][t][ci]) {
                    continue;
                }
                let moved = |r: &[Vec<Block>]| matches!((r.get(t), r.get(t - 1)), (Some(a), Some(b)) if base_moves(&b[ci], &a[ci]));
                let deps: Vec<usize> = inputs.iter().zip(&without).filter(|(_, r)| r[ph].as_ref().is_some_and(|r| !moved(r))).map(|(k, _)| *k).collect();
                v.push(((q.x, q.y), ph, t as u32, deps));
            }
        }
    }
    v
}

/// Sculk start of pocket `pk` from a vibration at `src` at tick `t`: the tick its sensor fires
/// (placed in the pocket's nearest cell), or None out of range (8 blocks).
fn sculk_hears(pk: &[(i32, i32)], src: (i32, i32), t: u32) -> Option<u32> {
    let d = pk.iter().map(|c| (((c.0 - src.0).pow(2) + (c.1 - src.1).pow(2)) as f64).sqrt()).fold(f64::MAX, f64::min);
    (d <= 8.0).then(|| t + d.floor() as u32 + 1)
}

fn match_groups(p: &Problem, groups: &[PGroup], need: u64, traces: &[Vec<u64>; 2], width_tol: u32) -> Option<GMask> {
    let mut covered: GMask = 0;
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
                        covered |= (1 as GMask) << gi;
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
    components(cells).len() == 1
}

/// The connected pieces of a cell set (4-neighbourhood).
fn components(cells: &[(i32, i32)]) -> Vec<Vec<(i32, i32)>> {
    let mut seen = vec![false; cells.len()];
    let mut out = Vec::new();
    for s in 0..cells.len() {
        if seen[s] {
            continue;
        }
        seen[s] = true;
        let mut comp = vec![cells[s]];
        let mut stack = vec![s];
        while let Some(i) = stack.pop() {
            for j in 0..cells.len() {
                if !seen[j] && (cells[i].0 - cells[j].0).abs() + (cells[i].1 - cells[j].1).abs() == 1 {
                    seen[j] = true;
                    stack.push(j);
                    comp.push(cells[j]);
                }
            }
        }
        out.push(comp);
    }
    out
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

fn pockets_one(version: &str, pa: &PocketArgs) -> Result<PocketSummary> {
    let a = &pa.s;
    let (mut prob, seeds, dx, dy) = setup(a)?;
    let mut seed = seeds[0].clone();
    // the lever candidates' rest polarity (read before the seed's lever goes)
    let seed_on = matches!(seed.lever_state(), Block::Lever { powered: true, .. });
    let polarities: Vec<bool> = match pa.lever_polarity.as_str() {
        "seed" => vec![seed_on],
        "off" => vec![false],
        "on" => vec![true],
        "both" => vec![false, true],
        o => bail!("--lever-polarity seed|off|on|both, not {o}"),
    };
    // the lever is placed by the fillings (lever candidates); without one the plan still drives
    // every input by ticks after the "flip" (`--keep-lever`: the seed's real lever stays)
    if !pa.keep_lever {
        prob.no_lever = true;
        seed.remove_lever();
        let lc = (prob.spec.lever[0], prob.spec.lever[1]);
        if prob.lever_cell_free && seed.get(lc.0, lc.1).is_air() && !prob.free.contains(&lc) {
            prob.free.push(lc);
        }
    }
    if let Some(rc) = &pa.region_cells {
        for t in rc.split(';').map(str::trim).filter(|t| !t.is_empty()) {
            let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            anyhow::ensure!(v.len() == 2, "--region-cells x,y;x,y");
            let (x, y) = (v[0] + dx, v[1] + dy);
            let b = seed.get(x, y);
            let wiring_piston = pa.region_pistons && matches!(b, Block::Piston { extended: false, .. });
            anyhow::ensure!(wiring_piston || (!matches!(b, Block::Piston { .. } | Block::PistonHead { .. } | Block::Lever { .. }) && !prob.door_cells.contains(&(x, y))), "--region-cells: ({},{}) holds {b:?}{}", v[0], v[1], if matches!(b, Block::Piston { extended: false, .. }) { " (a wiring piston: pass --region-pistons)" } else { "" });
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
    anyhow::ensure!(groups.len() <= GMask::BITS as usize, "at most {} plan groups ({})", GMask::BITS, groups.len());
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
        // as the enumeration sees it: the seed's own share taken out (+-2 ticks), and screened
        // again with the inputs the filling feeds released
        let base_tr: [Vec<u64>; 2] = {
            let mut evs = Vec::new();
            let mut tr: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
            let _ = screen_core(&prob, &seed, &mut evs, &mut tr, true);
            tr
        };
        let own = |tr: &[Vec<u64>; 2]| -> [Vec<u64>; 2] {
            let mut o = tr.clone();
            for ph in 0..2 {
                for (t, r) in o[ph].iter_mut().enumerate() {
                    let mut b = 0u64;
                    for dt in t.saturating_sub(2)..=t + 2 {
                        b |= base_tr[ph].get(dt).copied().unwrap_or(0);
                    }
                    *r &= !b;
                }
            }
            o
        };
        let fed: u64 = own(&traces).iter().flatten().fold(0, |acc, r| acc | r) & seed.assist;
        let mut g2 = g.clone();
        g2.assist = seed.assist & !fed;
        let mut evs2 = Vec::new();
        let mut tr2: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        let sc2 = screen_core(&prob, &g2, &mut evs2, &mut tr2, true);
        let tr2 = own(&tr2);
        println!("  released inputs fed by the filling: screen {}/{}; the filling's own share:", sc2.pts, sc2.max);
        for k in 0..prob.assist_pistons.len() {
            for phase in 0..2 {
                let s: String = tr2[phase].iter().map(|r| if (r >> k) & 1 == 1 { '#' } else { '.' }).collect();
                let b: String = base_tr[phase].iter().map(|r| if (r >> k) & 1 == 1 { '#' } else { '.' }).collect();
                println!("  input {} {}: own  {s}\n  {:>w$}   base {b}", names[k], if phase == 0 { "open " } else { "close" }, "", w = names[k].len() + 8);
            }
        }
        // as the enumeration judges it: the released inputs need no plan group when the door ran
        // (sc2 perfect), the others are matched against their windows
        let rel_ok = sc2.pts >= max;
        match match_groups(&prob, &groups, if rel_ok { seed.assist & !fed } else { seed.assist }, &tr2, pa.width_tol) {
            Some(c) => {
                let mut c = c;
                if rel_ok {
                    for (gi, gr) in groups.iter().enumerate() {
                        if (fed >> gr.k) & 1 == 1 {
                            c |= (1 as GMask) << gi;
                        }
                    }
                }
                println!("  released: matched groups: {}", (0..groups.len()).filter(|i| (c >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "))
            }
            None => println!("  released: rejected: a real edge no group explains"),
        }
        return Ok(PocketSummary { note: "debug".into(), ..Default::default() });
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
    // cells whose block kind changes at some tick (a piston, a block moved through): no lever
    // support
    let mut moved: HashSet<(i32, i32)> = HashSet::new();
    // static conductors (occupied at rest, never changing kind, no door cell, no piston): a pocket
    // powering one feeds the pistons next to it, or quasi-connected through it
    let mut statics: Vec<(i32, i32)> = Vec::new();
    {
        let list = seed.blocks();
        let (wlo, whi) = cs::watch_box(&list, &prob.spec);
        let mut w = World::new(BlockPos::new(wlo[0] - 3, wlo[1] - 3, wlo[2] - 3), [whi[0] - wlo[0] + 7, whi[1] - wlo[1] + 7, whi[2] - wlo[2] + 7]);
        let mut placed: Vec<(BlockPos, Block)> = list.iter().map(|(q, b)| (cs::bp(*q), *b)).collect();
        placed.extend(assist_world_rest(&prob, seed.assist));
        w.place_like_judge(&placed);
        cs::step_until_idle(&mut w, prob.max_ticks, |_| {});
        let cells = plane_cells(&prob, z);
        for q in &cells {
            if !w.get(*q).is_air() {
                rest_occ.insert((q.x, q.y));
            }
        }
        let rest_kind: Vec<std::mem::Discriminant<Block>> = cells.iter().map(|q| std::mem::discriminant(&w.get(*q))).collect();
        let lever = seed.lever_pos();
        for phase in 0..2 {
            w.use_block(lever);
            run_phase(&prob, &seed, &mut w, phase, None, |w| {
                for (i, q) in cells.iter().enumerate() {
                    let b = w.get(*q);
                    if !b.is_air() {
                        swept.insert((q.x, q.y));
                    }
                    if std::mem::discriminant(&b) != rest_kind[i] {
                        moved.insert((q.x, q.y));
                    }
                }
            });
        }
        for q in &cells {
            let c = (q.x, q.y);
            let b = w.get(*q);
            if rest_occ.contains(&c) && !moved.contains(&c) && !prob.door_cells.contains(&c) && !matches!(b, Block::Piston { .. } | Block::PistonHead { .. } | Block::MovingPiston { .. }) && redsim::behavior::conductor_of(&b) {
                statics.push(c);
            }
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
    // in-plane lever cells (`x,y`) and front-face supports (`x,y,-1` in --lever-cells)
    let mut lever_front_fixed: Vec<(i32, i32)> = Vec::new();
    let lever_cands: Vec<(i32, i32)> = match &pa.lever_cells {
        _ if pa.keep_lever => Vec::new(),
        Some(s) => s
            .split(';')
            .filter_map(|t| {
                let v: Vec<i32> = t.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                if v.len() == 3 && v[2] != 0 {
                    lever_front_fixed.push((v[0] + dx, v[1] + dy));
                    return None;
                }
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
    // OFF-PLANE lever supports (a lever on a block's front face, vjeux 05:07Z): static blocks of
    // the seed (present at rest, never moved through) next to a pocket; a filling's own full
    // blocks are candidates too (per filling, below)
    let front_ok = !pa.keep_lever && !pa.no_lever_front;
    let static_support: Vec<(i32, i32)> = if !front_ok {
        Vec::new()
    } else if pa.lever_cells.is_some() {
        lever_front_fixed.clone()
    } else {
        let mut v: Vec<(i32, i32)> = rest_occ
            .iter()
            .copied()
            .filter(|c| !moved.contains(c) && !prob.door_cells.contains(c) && cand.iter().any(|q| (q.0 - c.0).abs() + (q.1 - c.1).abs() == 1))
            .filter(|c| cs::full_cube(&seed.get(c.0, c.1)))
            .collect();
        v.sort();
        v
    };
    println!(
        "{} pocket cells in {} pockets; {} lever candidates in the plane, {} static front-face supports{}, polarity {:?}; {} swept cells",
        cand.len(),
        pockets.len(),
        lever_cands.len(),
        static_support.len(),
        if front_ok && pa.lever_cells.is_none() { " (+ the fillings' own blocks)" } else { "" },
        polarities,
        swept.len()
    );
    for (i, pk) in pockets.iter().enumerate() {
        println!("  pocket {i}: {} cells {:?}", pk.len(), pk.iter().map(|(x, y)| (x - dx, y - dy)).collect::<Vec<_>>());
    }
    // FEED REPORT (at rest): per assisted input, the pocket cells next to its piston (direct) or
    // next to the cell above it (quasi-connectivity). An input with none can only be fed through
    // blocks (a static, a door block, another piston's body) or by a moving source: the first
    // thing to fix in a variant when a screen never drives it.
    let n4 = |x: i32, y: i32| [(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)];
    for &(px, py) in prob.assist_pistons.iter() {
        let mut feeds: Vec<String> = Vec::new();
        for (tag, cells) in [("direct", n4(px, py).to_vec()), ("QC", n4(px, py + 1).into_iter().filter(|c| *c != (px, py)).collect::<Vec<_>>())] {
            for c in cells {
                if let Some(i) = pockets.iter().position(|pk| pk.contains(&c)) {
                    feeds.push(format!("{tag} ({},{}) pocket {i}", c.0 - dx, c.1 - dy));
                } else if statics.contains(&c) {
                    for d in n4(c.0, c.1) {
                        if let Some(i) = pockets.iter().position(|pk| pk.contains(&d)) {
                            feeds.push(format!("{tag} through the static ({},{}) from pocket {i}", c.0 - dx, c.1 - dy));
                        }
                    }
                }
            }
        }
        feeds.dedup();
        // gadget (fly 10-20): a lever OUTSIDE the box on the back face of a never-moving piston
        // holds that piston, and the piston below it by QC (flip-time levels only)
        let lever_back = |x: i32, y: i32| -> Option<(i32, i32)> {
            let Block::Piston { facing, .. } = seed.get(x, y) else { return None };
            let s = facing.step();
            let b = (x - s[0], y - s[1]);
            let (vl, vh) = prob.vol;
            let outside = b.0 < vl[0] || b.0 > vh[0] || b.1 < vl[1] || b.1 > vh[1];
            if s[2] != 0 || !outside || !seed.get(b.0, b.1).is_air() {
                return None;
            }
            // never moved: its cell holds the base (a retracting base is a moving piston) all along
            let r = replay_cells(&prob, &seed, &[BlockPos::new(x, y, seed.z)], None)?;
            r.iter().flatten().all(|v| matches!(v[0], Block::Piston { .. } | Block::MovingPiston { .. })).then_some(b)
        };
        if let Some(b) = lever_back(px, py) {
            feeds.push(format!("a lever outside the box at ({},{}) on its back (flip-time level)", b.0 - dx, b.1 - dy));
        }
        if let Some(b) = lever_back(px, py + 1) {
            feeds.push(format!("QC from a lever outside the box at ({},{}) on the back of the piston above (flip-time level)", b.0 - dx, b.1 - dy));
        }
        let what = if feeds.is_empty() { "NO FREE CELL (only through door blocks or a moving source)".to_string() } else { feeds.join(", ") };
        println!("  feed of the piston at ({},{}): {what}", px - dx, py - dy);
    }
    pocket_load_report(&prob, &pockets, &statics, &groups, &names);
    island_start_report(&prob, &seed, &pockets, &statics, &groups, (&names, (dx, dy), pa.why_islands, pa.sculk_islands));
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
    // per-pocket k and palettes (`--pocket-k x,y=K`, `--pocket-fill x,y=<families>|states:<..>`),
    // keyed by a door cell inside the pocket
    let cell_key = |s: &str| -> Result<((i32, i32), String)> {
        let (c, v) = s.split_once('=').with_context(|| format!("expected x,y=..., not {s:?}"))?;
        let xy: Vec<i32> = c.split(',').filter_map(|t| t.trim().parse().ok()).collect();
        anyhow::ensure!(xy.len() == 2, "expected x,y=..., not {s:?}");
        Ok(((xy[0] + dx, xy[1] + dy), v.trim().to_string()))
    };
    let mut pocket_k: Vec<((i32, i32), usize)> = Vec::new();
    for s in &pa.pocket_k {
        let (c, v) = cell_key(s)?;
        pocket_k.push((c, v.parse().with_context(|| format!("--pocket-k {s}"))?));
    }
    let mut pocket_pal: Vec<((i32, i32), Vec<Block>)> = Vec::new();
    for s in &pa.pocket_fill {
        let (c, v) = cell_key(s)?;
        let pv = match v.strip_prefix("states:") {
            Some(st) => {
                let mut out = Vec::new();
                for t in st.split(';').map(str::trim).filter(|t| !t.is_empty()) {
                    out.push(Block::parse(t).map_err(|e| anyhow::anyhow!("--pocket-fill {t}: {e}"))?);
                }
                out
            }
            None => palette(&v)?,
        };
        pocket_pal.push((c, pv.into_iter().filter(|b| !b.is_air()).collect()));
    }
    for (c, _) in pocket_k.iter().map(|(c, _)| (c, ())).chain(pocket_pal.iter().map(|(c, _)| (c, ()))) {
        anyhow::ensure!(pockets.iter().any(|pk| pk.contains(c)), "--pocket-k / --pocket-fill: ({},{}) is in no pocket", c.0 - dx, c.1 - dy);
    }
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
    // IN-PLANE assist guard (t78's hs23-0008L, reported via pockets2): a still-assisted input whose
    // plan block sits IN the door plane powers whatever a filling puts next to it (dust lit by the
    // assist's redstone block, an observer firing when it appears); the drive check takes the
    // block out raw but the neighbor's state keeps the power, so such a filling passed for real
    // wiring (a fake partial). A screen counts only when no filling block is next to an in-plane
    // cell of an input still assisted in it (a filling that feeds that input itself is screened
    // again with it released, where its block is gone).
    let adj_mask: std::collections::HashMap<(i32, i32), u64> = {
        let mut m: std::collections::HashMap<(i32, i32), u64> = std::collections::HashMap::new();
        for k in 0..prob.assist_pistons.len() {
            for q in &prob.assist_pos[k] {
                if q.z != z {
                    continue;
                }
                for (ddx, ddy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    *m.entry((q.x + ddx, q.y + ddy)).or_default() |= 1u64 << k;
                }
            }
        }
        m
    };
    let taint_of = |cells: &[(i32, i32)]| -> u64 { cells.iter().fold(0u64, |acc, c| acc | adj_mask.get(c).copied().unwrap_or(0)) };
    let mut accepted: Vec<Vec<Filling>> = Vec::new();
    for (pi, pk) in pockets.iter().enumerate() {
        if only.as_ref().is_some_and(|o| !o.contains(&pi)) || pk.len() > pa.max_pocket {
            println!("pocket {pi}: skipped");
            accepted.push(Vec::new());
            continue;
        }
        let tp = std::time::Instant::now();
        // this pocket's k and palette (--pocket-k / --pocket-fill)
        let kp = pocket_k.iter().find(|(c, _)| pk.contains(c)).map(|(_, k)| *k).unwrap_or(pa.k);
        let (pal, stone_pal): (Vec<Block>, Vec<Block>) = match pocket_pal.iter().find(|(c, _)| pk.contains(c)) {
            Some((_, pv)) => (pv.clone(), pv.iter().copied().filter(|b| matches!(b, Block::Simple(Simple::Stone) | Block::Simple(Simple::SmoothStone))).collect()),
            None => (pal.clone(), stone_pal.clone()),
        };
        if kp != pa.k || pocket_pal.iter().any(|(c, _)| pk.contains(c)) {
            println!("pocket {pi}: k {kp}, {} states", pal.len());
        }
        let touches_context = |cells: &[(i32, i32)]| cells.iter().any(|(x, y)| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(ddx, ddy)| rest_occ.contains(&(x + ddx, y + ddy))));
        // work items: (subset of pocket cell indices, lever option, the fixed states of the first
        // cells when the item is split: big subsets are split by their first 1-3 cells' states so
        // 150 threads stay busy to the end)
        let mut items: Vec<(Vec<usize>, Option<LeverAt>, Vec<usize>)> = Vec::new();
        let mut push = |items: &mut Vec<(Vec<usize>, Option<LeverAt>, Vec<usize>)>, sub: &Vec<usize>, lev: Option<LeverAt>| {
            let mut fixed = match sub.len() {
                0..=2 => 0,
                3..=4 => 1,
                5 => 2,
                _ => 3,
            };
            // big palettes (61 states with sculk) leave few, huge items at the end of a pocket:
            // fix more cells until an item holds at most ~50k fillings (threads stay busy)
            let n_of = |i: usize| if ring.contains(&pk[i]) { stone_pal.len().max(1) } else { pal.len().max(1) };
            while fixed + 1 < sub.len() && sub.iter().skip(fixed).map(|&i| n_of(i) as u64).product::<u64>() > 50_000 {
                fixed += 1;
            }
            let ns: Vec<usize> = sub.iter().take(fixed).map(|&i| if ring.contains(&pk[i]) { stone_pal.len() } else { pal.len() }).collect();
            let mut pre = vec![0usize; fixed];
            loop {
                items.push((sub.clone(), lev, pre.clone()));
                let mut j = 0;
                while j < fixed {
                    pre[j] += 1;
                    if pre[j] < ns[j] {
                        break;
                    }
                    pre[j] = 0;
                    j += 1;
                }
                if j == fixed {
                    break;
                }
            }
        };
        for sub in subsets_upto(pk.len(), kp) {
            let cells: Vec<(i32, i32)> = sub.iter().map(|&i| pk[i]).collect();
            // one connected piece touching the build, or (--gaps) pieces that each touch it
            let ok_shape = if pa.gaps { components(&cells).iter().all(|c| touches_context(c)) } else { connected(&cells) && touches_context(&cells) };
            if !ok_shape && !(pa.gaps || connected(&cells)) {
                continue;
            }
            if ok_shape {
                push(&mut items, &sub, None);
            }
            if sub.len() < kp {
                for &pw in &polarities {
                    for &l in &lever_cands {
                        if cells.contains(&l) {
                            continue;
                        }
                        if cells.iter().any(|q| (q.0 - l.0).abs() + (q.1 - l.1).abs() == 1) {
                            push(&mut items, &sub, Some((l.0, l.1, 0, pw)));
                        }
                    }
                    if front_ok {
                        // the lever on the front face of one of the filling's own blocks (with
                        // --lever-cells: only on the named cells)
                        for c in &cells {
                            if pa.lever_cells.is_none() || lever_front_fixed.contains(c) {
                                push(&mut items, &sub, Some((c.0, c.1, -1, pw)));
                            }
                        }
                        // or of a static block next to the filling
                        for &l in &static_support {
                            if cand.contains(&l) {
                                continue;
                            }
                            if cells.iter().any(|q| (q.0 - l.0).abs() + (q.1 - l.1).abs() == 1) {
                                push(&mut items, &sub, Some((l.0, l.1, -1, pw)));
                            }
                        }
                    }
                }
            }
        }
        let total: u64 = items
            .iter()
            .map(|(sub, _, pre)| sub.iter().enumerate().map(|(j, &i)| if j < pre.len() { 1 } else if ring.contains(&pk[i]) { stone_pal.len() as u64 } else { pal.len() as u64 }).product::<u64>())
            .sum();
        println!("pocket {pi}: {} cells, {} subset/lever items, {} fillings to screen", pk.len(), items.len(), total);
        let next = AtomicUsize::new(0);
        let screened = AtomicU64::new(0);
        let ok_door = AtomicU64::new(0);
        let found: Mutex<std::collections::HashMap<GMask, Vec<Filling>>> = Mutex::new(std::collections::HashMap::new());
        let active = AtomicUsize::new(a.threads);
        std::thread::scope(|s| {
            // progress every --report seconds (long pockets)
            s.spawn(|| {
                let mut last = std::time::Instant::now();
                while active.load(Ordering::Relaxed) > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    if last.elapsed().as_secs() >= a.report.max(5) {
                        last = std::time::Instant::now();
                        println!(
                            "  pocket {pi}: {} screened ({} of {} items started), {} keep the door working, {:.0} s",
                            screened.load(Ordering::Relaxed),
                            next.load(Ordering::Relaxed).min(items.len()),
                            items.len(),
                            ok_door.load(Ordering::Relaxed),
                            tp.elapsed().as_secs_f64()
                        );
                    }
                }
            });
            for _ in 0..a.threads {
                s.spawn(|| {
                    let mut local: std::collections::HashMap<GMask, Vec<Filling>> = std::collections::HashMap::new();
                    loop {
                        let ii = next.fetch_add(1, Ordering::Relaxed);
                        if ii >= items.len() {
                            break;
                        }
                        let (sub, lev, pre) = &items[ii];
                        let cells: Vec<(i32, i32)> = sub.iter().map(|&i| pk[i]).collect();
                        // inputs with an in-plane plan cell next to this filling (see adj_mask)
                        let taint = taint_of(&cells);
                        let pals: Vec<&Vec<Block>> = cells.iter().map(|c| if ring.contains(c) { &stone_pal } else { &pal }).collect();
                        if pals.iter().any(|p| p.is_empty()) {
                            continue;
                        }
                        let mut idx = vec![0usize; cells.len()];
                        // a split item: the first cells' states are fixed, the odometer runs over the rest
                        idx[..pre.len()].copy_from_slice(pre);
                        let j0 = pre.len();
                        loop {
                            let mut g = seed.clone();
                            for (j, c) in cells.iter().enumerate() {
                                g.set(c.0, c.1, pals[j][idx[j]]);
                            }
                            // a front-face lever on one of the filling's own blocks needs a full block
                            let skip = lev.is_some_and(|(lx, ly, dz, _)| dz != 0 && cells.iter().position(|c| *c == (lx, ly)).is_some_and(|j| !cs::full_cube(&pals[j][idx[j]])));
                            if let Some((lx, ly, dz, pw)) = lev
                                && !skip
                            {
                                place_lever_at(&mut g, *lx, *ly, *dz, *pw);
                            }
                            let mut evs = Vec::new();
                            let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
                            let mut settled: Vec<Block> = Vec::new();
                            let mut sc = if skip { Score { pts: 0, max, blocks: 0, ev: 0, ev_w: 0.0, assisted: 0, asw: 0.0, dr: 0.0 } } else { screen_core_w(&prob, &g, &mut evs, &mut traces, true, if pa.no_warm { None } else { Some(&mut settled) }) };
                            // WARM retry: a filling whose blocks change state as the build settles
                            // (dust next to a lever ON at rest, a repeater turning on, a dropper
                            // triggered) is placed again in its settled states, so the design's
                            // rest state is already consistent (no settling transient)
                            let mut fstates: Vec<Block> = cells.iter().enumerate().map(|(j, _)| pals[j][idx[j]]).collect();
                            if !settled.is_empty() && sc.pts < max {
                                let mut changed = false;
                                let mut gw = g.clone();
                                for (j, c) in cells.iter().enumerate() {
                                    let i = ((c.1 - g.gy) * g.w + (c.0 - g.gx)) as usize;
                                    let s = settled[i];
                                    if s != fstates[j] && std::mem::discriminant(&s) == std::mem::discriminant(&fstates[j]) {
                                        gw.set(c.0, c.1, s);
                                        fstates[j] = s;
                                        changed = true;
                                    }
                                }
                                if changed {
                                    g = gw;
                                    evs.clear();
                                    traces = [Vec::new(), Vec::new()];
                                    sc = screen_core(&prob, &g, &mut evs, &mut traces, true);
                                    screened.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            screened.fetch_add(1, Ordering::Relaxed);
                            // this filling's own share (the seed's remaining wiring taken out)
                            let traces = if skip { traces } else { own(&traces) };
                            // the inputs this filling feeds at all (any real power in what ran)
                            let fed: u64 = traces.iter().flatten().fold(0, |acc, r| acc | r) & seed.assist;
                            let mut driven: Option<GMask> = None;
                            if sc.pts >= max {
                                ok_door.fetch_add(1, Ordering::Relaxed);
                                if taint & seed.assist == 0 {
                                    driven = match_groups(&prob, &groups, seed.assist, &traces, pa.width_tol).filter(|d| *d != 0);
                                }
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
                                // the released inputs ran on the real wiring alone and the door
                                // worked: their real edges need no plan group (an extra harmless
                                // pulse, a long pulse for a push/pull pair, ...); only the inputs
                                // still assisted are matched against their windows
                                if sc2.pts >= max
                                    && taint & (seed.assist & !fed) == 0
                                    && let Some(d2) = match_groups(&prob, &groups, seed.assist & !fed, &tr2, pa.width_tol)
                                {
                                    // every group of a released input counts as covered: the door
                                    // ran without its plan
                                    let mut d2 = d2;
                                    for (gi, gr) in groups.iter().enumerate() {
                                        if (fed >> gr.k) & 1 == 1 {
                                            d2 |= (1 as GMask) << gi;
                                        }
                                    }
                                    if driven.is_none_or(|d| (d2 | d) != d) {
                                        driven = Some(d2 | driven.unwrap_or(0));
                                    }
                                }
                            }
                            if let Some(driven) = driven {
                                {
                                    let f = Filling { blocks: cells.iter().enumerate().map(|(j, c)| (*c, fstates[j])).collect(), lever: *lev, driven };
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
                    active.fetch_sub(1, Ordering::Relaxed);
                });
            }
        });
        let found = found.into_inner().unwrap();
        let mut sets: Vec<(&GMask, &Vec<Filling>)> = found.iter().collect();
        sets.sort_by_key(|(k, _)| std::cmp::Reverse(k.count_ones()));
        println!(
            "pocket {pi}: {} screened, {} keep the door working, {:.1} s; driven input sets: {}",
            screened.load(Ordering::Relaxed),
            ok_door.load(Ordering::Relaxed),
            tp.elapsed().as_secs_f64(),
            if sets.is_empty() {
                "none".to_string()
            } else {
                sets.iter().map(|(k, v)| format!("{{{}}} x{} (best {} blocks)", (0..groups.len()).filter(|i| (**k >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "), v.len(), v[0].cost())).collect::<Vec<_>>().join(", ")
            }
        );
        // the cheapest filling of each driven set, as a design (inspection: --debug-fill)
        for (si, (cov, v)) in sets.iter().enumerate() {
            let f = &v[0];
            let mut g = seed.clone();
            for (q, b) in &f.blocks {
                g.set(q.0, q.1, *b);
            }
            if let Some((lx, ly, dz, pw)) = f.lever {
                place_lever_at(&mut g, lx, ly, dz, pw);
            }
            let mut d = door_of(&g, prob.spec).to_design(&format!("pocket{pi}-set{si}"));
            d.meta.insert("covers".into(), (0..groups.len()).filter(|i| (**cov >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "));
            let _ = d.save(&a.out_dir.join(format!("pocket{pi}-set{si}.txt")));
        }
        let mut all: Vec<Filling> = found.into_values().flatten().collect();
        all.sort_by_key(|f| f.cost());
        accepted.push(all);
    }
    // ---- combinations
    let need: GMask = if groups.len() >= GMask::BITS as usize { GMask::MAX } else { ((1 as GMask) << groups.len()) - 1 };
    let coverable: GMask = accepted.iter().flatten().fold(0, |acc, f| acc | f.driven);
    // groups the remaining real wiring of the seed already drives (a door whose region was
    // erased: its other wiring keeps feeding some of the plan's pulses)
    let base_cov: GMask = {
        let mut evs = Vec::new();
        let mut traces: [Vec<u64>; 2] = [Vec::new(), Vec::new()];
        let _ = screen_core(&prob, &seed, &mut evs, &mut traces, true);
        match_groups(&prob, &groups, seed.assist, &traces, pa.width_tol).unwrap_or(0)
    };
    if base_cov != 0 {
        println!("groups the seed's own wiring already drives: {}", (0..groups.len()).filter(|i| (base_cov >> i) & 1 == 1).map(|i| glabels[i].clone()).collect::<Vec<_>>().join(" "));
    }
    let coverable = coverable | base_cov;
    let uncovered: Vec<String> = (0..groups.len()).filter(|i| (need >> i) & 1 == 1 && (coverable >> i) & 1 == 0).map(|i| glabels[i].clone()).collect();
    if coverable & need != need {
        println!("plan groups no filling drives: {}", uncovered.join(" "));
    }
    // the inputs whose every group is covered by a group set
    let released_by = |cov: GMask| -> u64 {
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
    let mut stack: Vec<(Vec<usize>, GMask, GMask)> = vec![(Vec::new(), base_cov, 0)];
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
            if let Some((lx, ly, dz, pw)) = chosen.iter().find_map(|&c| cands[c].lever) {
                place_lever_at(&mut g, lx, ly, dz, pw);
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
            let has_lever = matches!(g.lever_state(), Block::Lever { .. });
            if g.assist == 0 && !has_lever {
                continue;
            }
            // a filling next to an in-plane plan cell of an input still assisted here is not
            // judged (its drive may come from the plan's block, see adj_mask); the branch goes on:
            // more fillings may release that input
            let ctaint = chosen.iter().fold(0u64, |acc, &c| acc | taint_of(&cands[c].blocks.iter().map(|(q, _)| *q).collect::<Vec<_>>()));
            let tainted = ctaint & g.assist != 0;
            if tainted {
            } else if screen(&prob, &g).pts >= max {
                if g.assist == 0 {
                    let door = door_of(&g, prob.spec);
                    let e = cs::evaluate(&door, &jopts, false);
                    println!("combination {combos_tried}: perfect screen; redsim judge: {}", cs::eval_line("combo", &e, 0.0));
                    // every perfect screen is kept (vjeux 05:07Z: a 1-position pass is a find to
                    // report even if the 4-position judge fails)
                    {
                        let mut d = door.to_design(&format!("pockets-screenpass-{combos_tried}"));
                        d.meta.insert("verdict".into(), cs::eval_line("redsim", &e, 0.0));
                        // audit item 5: a door working at some of the judge's positions is a
                        // find to repair, not a failure: count them
                        if e.judge.is_err() {
                            let n = jopts.offsets.len();
                            let k = jopts
                                .offsets
                                .iter()
                                .filter(|off| {
                                    let mut o1 = jopts.clone();
                                    o1.offsets = vec![**off];
                                    cs::evaluate(&door, &o1, false).judge.is_ok()
                                })
                                .count();
                            d.meta.insert("positions".into(), format!("works at {k} of {n}"));
                            println!("  works at {k} of {n} positions");
                            if k > 0 {
                                d.save(&a.out_dir.join(format!("pockets-repair-{k}of{n}-{combos_tried}.txt")))?;
                            }
                        }
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
        let used: HashSet<(i32, i32)> = chosen.iter().flat_map(|&c| cands[c].blocks.iter().map(|(q, _)| *q).chain(cands[c].lever_cell())).collect();
        // one lever: fillings may share it when they hang it the same way (an in-plane lever's
        // cell is then shared; a front-face lever occupies no plane cell)
        let chosen_lever: Option<LeverAt> = chosen.iter().find_map(|&c| cands[c].lever);
        let mut kids: Vec<(Vec<usize>, GMask, GMask)> = Vec::new();
        for (c, f) in cands.iter().enumerate() {
            if (f.driven >> i) & 1 == 0 {
                continue;
            }
            if let (Some(a), Some(b)) = (chosen_lever, f.lever)
                && a != b
            {
                continue;
            }
            let shared = |q: &(i32, i32)| chosen_lever.is_some() && f.lever == chosen_lever && f.lever_cell() == Some(*q);
            if f.blocks.iter().map(|(q, _)| *q).chain(f.lever_cell()).any(|q| used.contains(&q) && !shared(&q)) {
                continue;
            }
            let mut ch = chosen.clone();
            ch.push(c);
            kids.push((ch, cov | f.driven, skipped));
        }
        // leaving input i assisted is the last resort (explored after every filling for it)
        stack.push((chosen.clone(), cov, skipped | ((1 as GMask) << i)));
        for k in kids.into_iter().rev() {
            stack.push(k);
        }
    }
    println!("{combos_tried} combinations verified, {finds} finds, {:.0} s total", t0.elapsed().as_secs_f64());
    Ok(PocketSummary { groups: groups.len(), uncovered, best_left: best_partial.map(|b| b.0), finds, secs: t0.elapsed().as_secs_f64(), note: String::new() })
}

/// One pockets run's outcome (the batch table).
#[derive(Clone, Debug, Default)]
pub struct PocketSummary {
    pub groups: usize,
    /// Plan groups no filling drives (labels).
    pub uncovered: Vec<String>,
    /// Fewest inputs left assisted in a true partial (None: no perfect combination).
    pub best_left: Option<u32>,
    pub finds: usize,
    pub secs: f64,
    pub note: String,
}

/// The mechanism's box (door coordinates `x0,y0:x1,y1`): every plane cell any block occupies at rest
/// or at any tick of the plan's open and close (the plan's own redstone blocks excluded).
pub fn auto_box(plan: &std::path::Path) -> Result<String> {
    let text = std::fs::read_to_string(plan)?;
    let s = crate::ideal::Schedule::parse(&text)?;
    let d = s.design();
    // a mechanism file may have no lever port
    let door = *d.ports.get("door").context("design has no 'port door: x y z'")?;
    let spec = crate::DoorSpec { door, lever: d.ports.get("lever").copied().unwrap_or(door), along_z: false };
    let z = spec.door[2];
    let mut blocks: Vec<(Pos, Block)> = Vec::new();
    for (p, st) in &d.blocks {
        blocks.push((*p, cs::convert_state(st).map_err(|e| anyhow::anyhow!(e))?));
    }
    let (wlo, whi) = cs::watch_box(&blocks, &spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 6, wlo[1] - 6, wlo[2] - 4), [whi[0] - wlo[0] + 13, whi[1] - wlo[1] + 13, whi[2] - wlo[2] + 9]);
    let list: Vec<(BlockPos, Block)> = blocks.iter().map(|(p, b)| (cs::bp(*p), *b)).collect();
    w.place_like_judge(&list);
    // plan input cells (their redstone blocks are not part of the mechanism)
    let mut inputs: HashSet<(i32, i32)> = HashSet::new();
    for open in [true, false] {
        for (_, p, _) in s.events_of(open, s.vars())? {
            if p[2] == z {
                inputs.insert((p[0], p[1]));
            }
        }
    }
    let mut lo = [i32::MAX; 2];
    let mut hi = [i32::MIN; 2];
    let mut take = |w: &World| {
        for x in wlo[0] - 4..=whi[0] + 4 {
            for y in wlo[1] - 4..=whi[1] + 4 {
                if inputs.contains(&(x, y)) {
                    continue;
                }
                let b = w.get(BlockPos::new(x, y, z));
                if !b.is_air() {
                    lo = [lo[0].min(x), lo[1].min(y)];
                    hi = [hi[0].max(x), hi[1].max(y)];
                }
            }
        }
    };
    cs::step_until_idle(&mut w, 300, |w| take(w));
    take(&w);
    for open in [true, false] {
        let evs = s.events_of(open, s.vars())?;
        let mut t = 0u32;
        let mut i = 0;
        loop {
            while i < evs.len() && evs[i].0 == t {
                let (_, p, st) = &evs[i];
                if st.as_deref() == Some("@use") {
                    w.use_block(cs::bp(*p));
                } else {
                    let b = cs::convert_state(st.as_deref().unwrap_or("minecraft:air")).map_err(|e| anyhow::anyhow!(e))?;
                    w.set_block_and_update(cs::bp(*p), b);
                }
                i += 1;
            }
            w.tick();
            t += 1;
            take(&w);
            if (i >= evs.len() && w.idle()) || t > 400 {
                break;
            }
        }
    }
    anyhow::ensure!(lo[0] <= hi[0], "{}: empty mechanism", plan.display());
    Ok(format!("{},{}:{},{}", lo[0] - spec.door[0], lo[1] - spec.door[1], hi[0] - spec.door[0], hi[1] - spec.door[1]))
}

/// `pockets` on several seeds: each `.ideal` seed is a plan whose `design` line names the mechanism
/// (it becomes `--assist`), `--box auto` takes each mechanism's own box; every run writes into
/// `<out-dir>/<plan name>` and one table line sums it up.
pub fn pockets_run(version: &str, pa: &PocketArgs) -> Result<()> {
    let batch = pa.s.seeds.len() > 1 || pa.s.seeds.iter().any(|s| s.extension().is_some_and(|e| e == "ideal")) || pa.s.bx == "auto";
    if !batch {
        pockets_one(version, pa)?;
        return Ok(());
    }
    let mut rows: Vec<(String, String, Result<PocketSummary>)> = Vec::new();
    for seed in &pa.s.seeds {
        let name = seed.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut p2 = pa.clone();
        let run = (|| -> Result<(String, PocketSummary)> {
            if seed.extension().is_some_and(|e| e == "ideal") {
                let text = std::fs::read_to_string(seed)?;
                let design = text.lines().find_map(|l| l.trim().strip_prefix("design ").map(|p| p.trim().to_string())).with_context(|| format!("{}: no design line", seed.display()))?;
                p2.s.seeds = vec![std::path::PathBuf::from(design)];
                p2.s.assist = Some(seed.clone());
            } else {
                p2.s.seeds = vec![seed.clone()];
            }
            let bx = if pa.s.bx == "auto" {
                let plan = p2.s.assist.clone().context("--box auto needs a plan (an .ideal seed or --assist)")?;
                auto_box(&plan)?
            } else {
                pa.s.bx.clone()
            };
            p2.s.bx = bx.clone();
            // a mechanism file without a lever port: a lever cell just outside the box (the
            // fillings place the real lever)
            if p2.s.lever.is_none() {
                let d = crate::cli::load_design(&p2.s.seeds[0])?;
                if !d.ports.contains_key("lever") {
                    let (_, hi) = parse_box(&bx)?;
                    p2.s.lever = Some(format!("{},{}", hi[0] + 1, hi[1] + 1));
                }
            }
            p2.s.out_dir = pa.s.out_dir.join(&name);
            println!("===== {name}: box {bx}");
            Ok((bx, pockets_one(version, &p2)?))
        })();
        match run {
            Ok((bx, s)) => rows.push((name, bx, Ok(s))),
            Err(e) => {
                println!("===== {name}: ERROR {e:#}");
                rows.push((name, String::new(), Err(e)));
            }
        }
    }
    println!("\nbatch summary ({} seeds):", rows.len());
    for (name, bx, r) in &rows {
        match r {
            Ok(s) => println!(
                "  {name} [{bx}]: {} groups, {} undriven{}, best partial {}, {} finds, {:.0} s",
                s.groups,
                s.uncovered.len(),
                if s.uncovered.is_empty() { String::new() } else { format!(" ({})", s.uncovered.join(" ")) },
                s.best_left.map(|l| format!("{l} inputs left")).unwrap_or_else(|| "-".into()),
                s.finds,
                s.secs
            ),
            Err(e) => println!("  {name}: ERROR {}", format!("{e:#}").lines().next().unwrap_or("")),
        }
    }
    Ok(())
}
