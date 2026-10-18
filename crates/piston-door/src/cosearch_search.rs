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
    /// Allow piston moves (move / re-face / add / remove pistons): the mechanism changes too.
    #[arg(long, default_value_t = true)]
    pub mech: bool,
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
    let all = "stone,glass,dust,rblock,torch,walltorch,repeater,comparator,observer,sticky,piston,lamp,note,target,bulb";
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
            f => bail!("unknown palette family {f:?} ({all})"),
        };
        for s in states {
            out.push(Block::parse(&s).map_err(|e| anyhow::anyhow!(e))?);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------- the screen

/// Screen result: points (higher is better), then fewer blocks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub pts: i32,
    pub max: i32,
    pub blocks: i32,
}

impl Score {
    fn energy(&self) -> f64 {
        -(self.pts as f64) + 0.02 * self.blocks as f64
    }
    pub fn perfect(&self) -> bool {
        self.pts >= self.max
    }
}

fn in_box(p: &Problem, x: i32, y: i32) -> bool {
    x >= p.lo[0] && x <= p.hi[0] && y >= p.lo[1] && y <= p.hi[1]
}

/// One position (the door as written), `p.cycles` lever cycles: partial credit.
pub fn screen(p: &Problem, g: &Grid) -> Score {
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
        return Score { pts: 0, max, blocks };
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
    let lever = cs::bp(spec.lever);
    if !matches!(w.get(lever), Block::Lever { powered: false, .. }) {
        return Score { pts: pts.min(max - 1), max, blocks };
    }
    pts += 1;
    if door_ok < 9 || unsupported > 0 {
        return Score { pts: pts.min(max - 1), max, blocks };
    }
    pts += 1;
    // the plane's cells (the box + 1) compared after each close
    let rest: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
    for _cycle in 0..p.cycles {
        w.use_block(lever);
        let (_, idle) = cs::step_until_idle(&mut w, p.max_ticks, |_| {});
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
            return Score { pts: pts.min(max - 1), max, blocks };
        }
        pts += 1;
        w.use_block(lever);
        let (_, idle) = cs::step_until_idle(&mut w, p.max_ticks, |_| {});
        let back = p.door_cells.iter().filter(|(x, y)| w.get(at(*x, *y)) == p.door_block).count() as i32;
        pts += back;
        if !idle || !w.drops.is_empty() || !w.errors.is_empty() {
            return Score { pts: pts.min(max - 1), max, blocks };
        }
        let now: Vec<Block> = plane_cells(p, z).iter().map(|q| w.get(*q)).collect();
        let same = now.iter().zip(rest.iter()).filter(|(a, b)| a == b).count() as i32;
        let n = rest.len() as i32;
        if same == n {
            pts += 3;
        } else {
            pts -= (n - same).min(3);
            return Score { pts: pts.min(max - 1), max, blocks };
        }
    }
    Score { pts, max, blocks }
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
        _ => return None,
    })
}

/// One random move inside the focus cells. Returns false when nothing changed.
fn mutate(p: &Problem, g: &mut Grid, rng: &mut Rng) -> bool {
    let (x, y) = p.free[rng.below(p.free.len())];
    let cur = g.get(x, y);
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
        if !p.free.contains(&(nx, ny)) || !g.get(nx, ny).is_air() {
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
        if !p.free.contains(&(nx, ny)) || other == cur || head_of(&cur, x, y).is_some() || head_of(&other, nx, ny).is_some() || matches!(other, Block::PistonHead { .. }) {
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
        if !p.free.contains(&(cx, cy)) || !g.get(cx, cy).is_air() {
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
                if !p.free.contains(&(hx, hy)) || !g.get(hx, hy).is_air() {
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
    let mut g = Grid { gx, gy, w, h, z, cells: vec![Block::Air; (w * h) as usize] };
    let lever = door.spec.lever;
    for (q, b) in &door.blocks {
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
}

fn door_of(g: &Grid, spec: crate::DoorSpec) -> RsDoor {
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
    let door_block = doors[0].2.blocks.iter().find(|(q, _)| q == &spec0.door).map(|(_, b)| *b).unwrap_or(Block::Simple(Simple::Stone));
    let mut spec = spec0;
    spec.lever = lever_pos;
    let prob = Problem { spec, lo, hi, free, door_cells, ring, door_block, palette: palette(&a.palette)?, mech: a.mech, cycles: a.cycles, max_ticks: a.max_ticks };
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
        let s = screen(&prob, &g);
        println!("seed {name}: {} blocks in the box, screen {}/{}", g.count(), s.pts, s.max);
        seeds.push(g);
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
    };
    let jopts = cs::JudgeOpts::with_extra(28);
    let t0 = std::time::Instant::now();
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
                let top: Vec<String> = best.iter().take(6).map(|(s, g)| format!("{}/{}({}b)", s.pts, s.max, g.count())).collect();
                println!(
                    "[{el:>5}s] {} screens ({:.0}/s), {} perfect screens, {} redsim judge runs, {} finds; best {}",
                    sh.evals.load(Ordering::Relaxed),
                    sh.evals.load(Ordering::Relaxed) as f64 / t0.elapsed().as_secs_f64(),
                    sh.perfect.load(Ordering::Relaxed),
                    sh.full_evals.load(Ordering::Relaxed),
                    sh.finds.load(Ordering::Relaxed),
                    top.join(" ")
                );
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
        println!("best partial {i}: {}/{} ({} blocks) -> {}", s.pts, s.max, g.count(), path.display());
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
            let nm = 1 + rng.below(2);
            let mut changed = false;
            for _ in 0..nm {
                changed |= mutate(p, &mut cand, &mut rng);
            }
            if !changed {
                continue;
            }
            let s = screen(p, &cand);
            sh.evals.fetch_add(1, Ordering::Relaxed);
            let de = s.energy() - cs_.energy();
            if de <= 0.0 || rng.f64() < (-de / temp).exp() {
                cur = cand;
                cs_ = s;
                if s.pts > best_local.pts || (s.pts == best_local.pts && s.blocks < best_local.blocks) {
                    best_local = s;
                    let mut best = sh.best.lock().unwrap();
                    best.push((s, cur.clone()));
                    best.sort_by(|x, y| y.0.pts.cmp(&x.0.pts).then(x.0.blocks.cmp(&y.0.blocks)));
                    best.dedup_by(|x, y| x.1 == y.1);
                    best.truncate(32);
                }
                if s.perfect() {
                    sh.perfect.fetch_add(1, Ordering::Relaxed);
                    let h = grid_hash(&cur);
                    if sh.seen.lock().unwrap().insert(h) {
                        sh.full_evals.fetch_add(1, Ordering::Relaxed);
                        let door = door_of(&cur, p.spec);
                        let e = cs::evaluate(&door, jopts, false);
                        if e.record_ok() {
                            let k = sh.finds.fetch_add(1, Ordering::Relaxed);
                            let vol = e.volume.map(|v| v.0).unwrap_or(0);
                            let name = format!("cosearch-{vol}b-{}-{k}", door.block_count());
                            let d = door.to_design(&name);
                            let path = a.out_dir.join(format!("{name}.txt"));
                            let _ = d.save(&path);
                            println!("redsim find: {}", cs::eval_line(&name, &e, 0.0));
                            let _ = tx.send((path, d));
                            if k + 1 >= a.max_finds {
                                sh.stop.store(true, Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        }
    }
}
