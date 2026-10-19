//! An exhaustive search over small designs with fixed input cells (tracks delay and gates): every
//! filling of a box with kmin..=k blocks from a palette, judged by a caller's evaluator on redsim
//! (which reads every candidate output cell at once: the box and a 1-cell ring around it); the
//! best design per key (e.g. a gate function, a delay N) is kept with its output cell.
//! Fillings whose floor-standing components (dust, repeaters, comparators, standing torches) or
//! wall torches lack a solid block from the filling under / behind them are skipped before any
//! simulation.

use crate::harness::Pos;
use redsim::{Block, Dir};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Space {
    /// The box: cells (x, y, z) with 0 <= x < dims[0], ...
    pub dims: [i32; 3],
    /// Input cells (kept free), inside or outside the box.
    pub inputs: Vec<Pos>,
    pub palette: Vec<Block>,
    pub kmin: usize,
    pub k: usize,
    /// Blocks in every filling (not counted in k; their cells are not free).
    pub fixed: Vec<(Pos, Block)>,
}

impl Space {
    /// Every candidate output cell: the box and a 1-cell ring around it, minus the inputs.
    pub fn output_cells(&self) -> Vec<Pos> {
        let mut v = Vec::new();
        for y in -1..=self.dims[1] {
            for z in -1..=self.dims[2] {
                for x in -1..=self.dims[0] {
                    let p = [x, y, z];
                    if !self.inputs.contains(&p) {
                        v.push(p);
                    }
                }
            }
        }
        v
    }

    /// The bounds of the box, its ring and the inputs.
    pub fn bounds(&self) -> (Pos, Pos) {
        let mut lo = [-1, -1, -1];
        let mut hi = [self.dims[0], self.dims[1], self.dims[2]];
        for p in &self.inputs {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
        }
        (lo, hi)
    }
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub blocks: Vec<(Pos, Block)>,
    pub output: Pos,
    /// Lower is better: (blocks, secondary).
    pub score: (u32, u32),
    pub note: String,
}

/// A full block a floor component can stand on (and a wall torch can hang on).
pub fn solid(b: &Block) -> bool {
    matches!(b, Block::Simple(_) | Block::RedstoneBlock | Block::Lamp { .. } | Block::Bulb { .. } | Block::Note { .. } | Block::Target { .. } | Block::Observer { .. } | Block::Log { .. })
}

fn needs_floor(b: &Block) -> bool {
    matches!(b, Block::Wire { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Torch { .. })
}

fn dir_vec(d: Dir) -> Pos {
    match d {
        Dir::North => [0, 0, -1],
        Dir::South => [0, 0, 1],
        Dir::East => [1, 0, 0],
        Dir::West => [-1, 0, 0],
        Dir::Up => [0, 1, 0],
        Dir::Down => [0, -1, 0],
    }
}

/// Every k-subset of `n` cells, as index lists.
fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = Vec::new();
    fn rec(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            if n - i < k - cur.len() {
                break;
            }
            cur.push(i);
            rec(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    rec(0, n, k, &mut cur, &mut out);
    out
}

/// Run the search; `eval` returns (key, score, note, output cell) entries for a filling (none =
/// rejected).
pub fn search<F>(sp: &Space, threads: usize, eval: F) -> BTreeMap<String, Hit>
where
    F: Fn(&[(Pos, Block)]) -> Vec<(String, (u32, u32), String, Pos)> + Sync,
{
    let mut cells: Vec<Pos> = Vec::new();
    for y in 0..sp.dims[1] {
        for z in 0..sp.dims[2] {
            for x in 0..sp.dims[0] {
                let p = [x, y, z];
                if !sp.inputs.contains(&p) && !sp.fixed.iter().any(|(q, _)| *q == p) {
                    cells.push(p);
                }
            }
        }
    }
    let mut jobs: Vec<Vec<usize>> = Vec::new();
    for k in sp.kmin..=sp.k {
        jobs.extend(subsets(cells.len(), k));
    }
    let best: Mutex<BTreeMap<String, Hit>> = Mutex::new(BTreeMap::new());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    let total = jobs.len();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let j = next.fetch_add(1, Ordering::Relaxed);
                    if j >= jobs.len() {
                        break;
                    }
                    let sub = &jobs[j];
                    let pos: Vec<Pos> = sub.iter().map(|&i| cells[i]).collect();
                    let k = pos.len();
                    let p = sp.palette.len();
                    let mut idx = vec![0usize; k];
                    'assign: loop {
                        // the filling (fixed blocks first)
                        let blocks: Vec<(Pos, Block)> = sp.fixed.iter().copied().chain(pos.iter().zip(&idx).map(|(q, &i)| (*q, sp.palette[i]))).collect();
                        if supported(&blocks, &sp.inputs) {
                            for (key, score, note, output) in eval(&blocks) {
                                let mut b = best.lock().unwrap();
                                let better = b.get(&key).is_none_or(|h| score < h.score);
                                if better {
                                    b.insert(key, Hit { blocks: blocks.clone(), output, score, note });
                                }
                            }
                        }
                        // next assignment
                        let mut i = 0;
                        loop {
                            if i == k {
                                break 'assign;
                            }
                            idx[i] += 1;
                            if idx[i] < p {
                                break;
                            }
                            idx[i] = 0;
                            i += 1;
                        }
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d % 500 == 0 || d == total {
                        eprintln!("  {d}/{total} cell sets, {:.0} s", t0.elapsed().as_secs_f64());
                    }
                }
            });
        }
    });
    best.into_inner().unwrap()
}

/// Floor components stand on a solid block of the filling, wall torches hang on one, and every
/// block touches (26-neighborhood) another block or an input (an isolated block changes nothing).
fn supported(blocks: &[(Pos, Block)], inputs: &[Pos]) -> bool {
    let at = |p: Pos| blocks.iter().find(|(q, _)| *q == p).map(|(_, b)| *b);
    let near = |a: Pos, b: Pos| (a[0] - b[0]).abs() <= 1 && (a[1] - b[1]).abs() <= 1 && (a[2] - b[2]).abs() <= 1;
    for (i, (p, b)) in blocks.iter().enumerate() {
        if needs_floor(b) && !at([p[0], p[1] - 1, p[2]]).is_some_and(|f| solid(&f)) {
            return false;
        }
        if let Block::WallTorch { facing, .. } = b {
            let v = dir_vec(*facing);
            if !at([p[0] - v[0], p[1] - v[1], p[2] - v[2]]).is_some_and(|f| solid(&f)) {
                return false;
            }
        }
        if !inputs.iter().any(|q| near(*p, *q)) && !blocks.iter().enumerate().any(|(j, (q, _))| j != i && near(*p, *q)) {
            return false;
        }
    }
    true
}

/// Palette shorthands: stone, rblock, dust, torch (standing + 4 wall), rep (4 facings, delay 1),
/// rep4 (4 facings x 4 delays), cmp (4 facings x compare/subtract), obs (6 facings), lamp,
/// bulb (waxed), note, target, or block states.
pub fn palette(spec: &str) -> anyhow::Result<Vec<Block>> {
    let mut out: Vec<String> = Vec::new();
    let h = ["north", "south", "east", "west"];
    for w in spec.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        match w {
            "stone" => out.push("minecraft:stone".into()),
            "rblock" => out.push("minecraft:redstone_block".into()),
            "dust" => out.push("minecraft:redstone_wire".into()),
            "torch" => {
                out.push("minecraft:redstone_torch[lit=true]".into());
                for f in h {
                    out.push(format!("minecraft:redstone_wall_torch[facing={f},lit=true]"));
                }
            }
            "rep" => {
                for f in h {
                    out.push(format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]"));
                }
            }
            "rep4" => {
                for f in h {
                    for d in 1..=4 {
                        out.push(format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]"));
                    }
                }
            }
            "cmp" => {
                for f in h {
                    for m in ["compare", "subtract"] {
                        out.push(format!("minecraft:comparator[facing={f},mode={m},powered=false]"));
                    }
                }
            }
            "obs" => {
                for f in ["north", "south", "east", "west", "up", "down"] {
                    out.push(format!("minecraft:observer[facing={f},powered=false]"));
                }
            }
            "lamp" => out.push("minecraft:redstone_lamp[lit=false]".into()),
            "bulb" => out.push("minecraft:waxed_copper_bulb[lit=false,powered=false]".into()),
            "note" => out.push("minecraft:note_block".into()),
            "target" => out.push("minecraft:target".into()),
            "piston" | "sticky" => {
                let name = if w == "piston" { "piston" } else { "sticky_piston" };
                for f in ["north", "south", "east", "west", "up", "down"] {
                    out.push(format!("minecraft:{name}[extended=false,facing={f}]"));
                }
            }
            "hpiston" | "hsticky" => {
                let name = if w == "hpiston" { "piston" } else { "sticky_piston" };
                for f in h {
                    out.push(format!("minecraft:{name}[extended=false,facing={f}]"));
                }
            }
            "slime" => out.push("minecraft:slime_block".into()),
            "scaffold" => out.push("minecraft:scaffolding[bottom=false,distance=0,waterlogged=false]".into()),
            "leaves" => out.push("minecraft:oak_leaves[distance=7,persistent=true,waterlogged=false]".into()),
            "log" => out.push("minecraft:oak_log[axis=y]".into()),
            "itrap" => {
                for f in h {
                    for half in ["top", "bottom"] {
                        out.push(format!("minecraft:iron_trapdoor[facing={f},half={half},open=false,powered=false,waterlogged=false]"));
                    }
                }
            }
            s => out.push(s.to_string()),
        }
    }
    out.iter().map(|s| Block::parse(s).map_err(|e| anyhow::anyhow!("{s}: {e}"))).collect()
}
