//! `mctas circuit arith synth`: SAT synthesis of small flat redstone designs for a combinational
//! function (study 12's arithmetic kinds), CEGAR against redsim.
//!
//! The model is redstone's STEADY state, read in the 26.3 source (the classes named below), for
//! every input combination at once, over a box of W x H x D cells (H = 2: a floor layer y = 0 and a
//! component layer y = 1, the study's "flat"). Every grid cell holds one of: air, stone (a
//! conductor), a redstone block, dust, a repeater or comparator (4 facings; compare or subtract),
//! a standing torch, a wall torch (4 facings). Port cells sit in the ring around the grid on the
//! component layer (fixed with `--in` / `--out`, or chosen by the solver with `--auto-ports`): an
//! input holds a redstone block when on (air when off), an output the judge's lamp (a conductor).
//! Per combination, every cell has a level 0..15 (order-encoded) whose meaning depends on its kind:
//! * a conductor (stone, the lamp): the strongest DIRECT signal into it (`getDirectSignalTo`:
//!   repeaters and comparators facing it, a torch below it, dust on it or pointing into it), and
//!   apart from that its STRONG level (the same without dust: what dust sees, since
//!   `RedStoneWireBlock.shouldSignal` is off while a wire computes);
//! * dust: max(what its 6 neighbors give it with dust hidden, a horizontal wire's level - 1)
//!   (`calculateTargetStrength`); it points where `getConnectionState` connects it (a side with a
//!   wire, a repeater on that axis, a comparator, a torch, a redstone block; none on an axis: both
//!   sides of the other axis), and powers the block below and the sides it points to;
//! * a repeater: 15 when its back gives any signal (`DiodeBlock.getInputSignal`), no side locks
//!   allowed; a comparator: back B and side S (`getControlInputSignal`: dust, redstone blocks,
//!   diodes facing it), compare B >= S ? B : 0 or subtract B - S;
//! * a torch: 15 unless the block it hangs on has a signal (`hasSignal`, dust included).
//!   The output is on when its lamp gets a signal (`getBestNeighborSignal` > 0).
//!
//! Outside the steady model, two rules keep the solver honest: designs are feed-forward (ranks:
//! no loop through a torch, a diode or a block: in-phase torch pairs oscillate and latches remember)
//! and a dust next to an input may not change its shape without changing its power (an update-less
//! redirection: what it now points to never gets a block update). The solver minimizes the grid
//! blocks; each model is decoded and run through the judge's walk on redsim (crate::eval): a failing
//! one is blocked and the solver asked again.

use crate::eval::{self, Pos, Rig};
use crate::func::Func;
use anyhow::{Result, anyhow, bail};
use rustsat::solvers::{Solve, SolveIncremental, SolverResult};
use rustsat::types::{Clause, Lit, TernaryVal};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct SynthArgs {
    /// The function: adder N, subtractor N, incrementer N, comparator N (or xor, and, maj...).
    #[arg(long)]
    kind: String,
    /// Input ports `name=x,y,z` (repeatable; in the grid's ring on the component layer). With
    /// --auto-ports: just the names (`a0`).
    #[arg(long = "in")]
    ins: Vec<String>,
    /// Output ports `name=x,y,z` (repeatable); with --auto-ports just the names.
    #[arg(long = "out")]
    outs: Vec<String>,
    /// The solver places the ports in the ring (any cell of the component layer around the grid).
    #[arg(long)]
    auto_ports: bool,
    /// With --auto-ports: the sides inputs may use (n e s w), and outputs.
    #[arg(long, default_value = "nesw")]
    in_sides: String,
    #[arg(long, default_value = "nesw")]
    out_sides: String,
    /// The grid W x D (x 0..W, z 0..D; y 0..H).
    #[arg(long, default_value = "4x4")]
    grid: String,
    /// Layers (2: flat, floor + components).
    #[arg(long, default_value_t = 2)]
    height: i32,
    /// Kinds offered: stone, rblock, dust, rep, cmp, torch, wtorch.
    #[arg(long, default_value = "stone,rblock,dust,rep,cmp,torch,wtorch")]
    palette: String,
    /// Start from at most this many grid blocks.
    #[arg(long)]
    max_blocks: Option<usize>,
    /// CEGAR rounds at most.
    #[arg(long, default_value_t = 2000)]
    iters: usize,
    /// Hold per state of the redsim walk.
    #[arg(long, default_value_t = 20)]
    gap: u32,
    /// Write every judged design here.
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Name prefix of the written designs.
    #[arg(long, default_value = "syn")]
    name: String,
    /// Keep searching for more designs of the best size (that many) instead of stopping.
    #[arg(long, default_value_t = 1)]
    keep: usize,
    /// Allow feedback loops (latches); default: feed-forward designs only.
    #[arg(long)]
    feedback: bool,
    /// Combinations in the model from the start (all when there are at most this many; else
    /// these plus every counterexample redsim finds).
    #[arg(long, default_value_t = 64)]
    sample: usize,
    /// R7 inputs (the judge's real input devices, docs/circuits.md): an input cell is the judge's
    /// smooth stone, strongly powered by its repeater when on. It is a conductor: dust beside it
    /// takes its 15 but never points at it, a comparator's side does not read it, a torch hung on
    /// it inverts it. Each model is then judged by the circuit judge's own redsim run (`circuit
    /// sim`: R7 drivers, the judge's lamps, every rule) instead of the track's walk. Off: the old
    /// placement (a redstone block written into the cell) and the walk.
    #[arg(long)]
    r7: bool,
    /// Interchangeable inputs (`a0,b0,cin` for a full adder: s and cout are symmetric in them):
    /// with --auto-ports their ring cells are kept in this order (symmetry breaking).
    #[arg(long, value_delimiter = ',')]
    sym: Vec<String>,
    /// A bit slice: `cin,cout` (both given fixed cells, cout = cin + the step). The chain output
    /// is a redstone lamp of the design (the next copy's chain input: a conductor) whose DIRECT
    /// power must equal the output; designs are written with `slice:` and judged as slices.
    #[arg(long, value_delimiter = ',')]
    chain: Vec<String>,
    /// A partial design whose blocks are kept (each grid cell it fills keeps that kind; the
    /// solver completes the rest): a known sub-circuit, e.g. a synthesized slice to grow.
    #[arg(long)]
    fix: Option<PathBuf>,
}

/// The synthesizer's kind for a block state (None: not in its palette).
fn kind_of(state: &str) -> Option<K> {
    let id = state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:");
    let prop = |k: &str| state.split(&format!("{k}=")).nth(1).and_then(|r| r.split([',', ']']).next());
    let dir = |s: &str| DNAME.iter().position(|d| *d == s);
    Some(match id {
        "stone" | "smooth_stone" => K::Solid,
        "redstone_block" => K::RBlock,
        "redstone_wire" => K::Dust,
        "redstone_torch" => K::Torch,
        "redstone_wall_torch" => K::WTorch(dir(prop("facing")?)?),
        "repeater" => K::Rep(opp(dir(prop("facing")?)?)),
        "comparator" => K::Cmp(opp(dir(prop("facing")?)?), prop("mode")? == "subtract"),
        _ => return None,
    })
}

// ------------------------------------------------------------------------------------- encoder

struct Enc {
    s: rustsat_cadical::CaDiCaL<'static, 'static>,
    n: u32,
    t: Lit,
    clauses: usize,
}

impl Enc {
    fn new() -> Enc {
        let mut e = Enc { s: rustsat_cadical::CaDiCaL::default(), n: 1, t: Lit::positive(0), clauses: 0 };
        let t = e.t;
        e.s.add_unit(t).unwrap();
        e
    }
    fn var(&mut self) -> Lit {
        let l = Lit::positive(self.n);
        self.n += 1;
        l
    }
    fn f(&self) -> Lit {
        !self.t
    }
    fn konst(&self, b: bool) -> Lit {
        if b { self.t } else { !self.t }
    }
    fn add(&mut self, c: &[Lit]) {
        if c.iter().any(|&l| l == self.t) {
            return;
        }
        let v: Clause = c.iter().copied().filter(|&l| l != !self.t).collect();
        self.s.add_clause(v).unwrap();
        self.clauses += 1;
    }
    fn and(&mut self, xs: &[Lit]) -> Lit {
        let mut v: Vec<Lit> = Vec::new();
        for &x in xs {
            if x == !self.t {
                return !self.t;
            }
            if x != self.t && !v.contains(&x) {
                if v.contains(&!x) {
                    return !self.t;
                }
                v.push(x);
            }
        }
        match v.len() {
            0 => self.t,
            1 => v[0],
            _ => {
                let o = self.var();
                for &x in &v {
                    self.add(&[!o, x]);
                }
                let mut c: Vec<Lit> = v.iter().map(|&x| !x).collect();
                c.push(o);
                self.add(&c);
                o
            }
        }
    }
    fn or(&mut self, xs: &[Lit]) -> Lit {
        let n: Vec<Lit> = xs.iter().map(|&x| !x).collect();
        !self.and(&n)
    }
    fn val(&self, l: Lit) -> bool {
        if l == self.t {
            return true;
        }
        if l == !self.t {
            return false;
        }
        matches!(self.s.lit_val(l), Ok(TernaryVal::True))
    }
    fn exactly_one(&mut self, xs: &[Lit]) {
        self.add(xs);
        self.at_most_one(xs);
    }
    fn at_most_one(&mut self, xs: &[Lit]) {
        for i in 0..xs.len() {
            for j in i + 1..xs.len() {
                self.add(&[!xs[i], !xs[j]]);
            }
        }
    }
    fn totalizer(&mut self, xs: &[Lit]) -> Vec<Lit> {
        if xs.len() <= 1 {
            return xs.to_vec();
        }
        let (a, b) = xs.split_at(xs.len() / 2);
        let a = self.totalizer(a);
        let b = self.totalizer(b);
        let r: Vec<Lit> = (0..a.len() + b.len()).map(|_| self.var()).collect();
        for i in 0..=a.len() {
            for j in 0..=b.len() {
                if i + j == 0 {
                    continue;
                }
                let mut c = vec![r[i + j - 1]];
                if i > 0 {
                    c.push(!a[i - 1]);
                }
                if j > 0 {
                    c.push(!b[j - 1]);
                }
                self.add(&c);
            }
        }
        r
    }
}

/// An order-encoded level 0..15: `n[k]` = (level >= k), `n[0]` true.
type Num = Vec<Lit>;

fn num(e: &mut Enc) -> Num {
    let mut v = vec![e.t];
    for _ in 1..16 {
        v.push(e.var());
    }
    for k in 1..15 {
        e.add(&[!v[k + 1], v[k]]);
    }
    v
}

fn knum(e: &Enc, x: u8) -> Num {
    (0..16).map(|k| e.konst(k as u8 <= x)).collect()
}

/// A level that is 15 when `b`, else 0.
fn bnum(e: &Enc, b: Lit) -> Num {
    let mut v = vec![e.t];
    v.extend(std::iter::repeat_n(b, 15));
    v
}

/// `g -> q = max over terms (cond and (value - shift))` (a term counts only when its cond holds;
/// values below 0 count as 0).
fn gate_max(e: &mut Enc, g: Lit, q: &Num, terms: &[(Lit, &Num, usize)]) {
    for k in 1..16 {
        let mut ors = vec![!g, !q[k]];
        for (cond, v, shift) in terms {
            if k + shift > 15 {
                continue;
            }
            let x = v[k + shift];
            e.add(&[!g, !*cond, !x, q[k]]);
            let a = e.and(&[*cond, x]);
            ors.push(a);
        }
        e.add(&ors);
    }
}

/// `g -> q = v`.
fn gate_eq(e: &mut Enc, g: Lit, q: &Num, v: &Num) {
    for k in 1..16 {
        e.add(&[!g, !q[k], v[k]]);
        e.add(&[!g, q[k], !v[k]]);
    }
}

// ----------------------------------------------------------------------------------- the model

/// Directions: N E S W (horizontal, clockwise), Up, Down.
const DIRS: [Pos; 6] = [[0, 0, -1], [1, 0, 0], [0, 0, 1], [-1, 0, 0], [0, 1, 0], [0, -1, 0]];
const UP: usize = 4;
const DOWN: usize = 5;
const DNAME: [&str; 6] = ["north", "east", "south", "west", "up", "down"];

fn opp(d: usize) -> usize {
    match d {
        0 => 2,
        1 => 3,
        2 => 0,
        3 => 1,
        4 => 5,
        _ => 4,
    }
}

fn add(p: Pos, d: usize) -> Pos {
    [p[0] + DIRS[d][0], p[1] + DIRS[d][1], p[2] + DIRS[d][2]]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum K {
    Air,
    Solid,
    RBlock,
    Dust,
    /// Output toward the direction.
    Rep(usize),
    /// Output toward the direction; subtract mode.
    Cmp(usize, bool),
    Torch,
    /// Pointing toward the direction (hung on the block behind it).
    WTorch(usize),
}

impl K {
    fn state(&self) -> Option<String> {
        Some(match self {
            K::Air => return None,
            K::Solid => "minecraft:stone".into(),
            K::RBlock => "minecraft:redstone_block".into(),
            K::Dust => "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into(),
            K::Rep(o) => format!("minecraft:repeater[delay=1,facing={},locked=false,powered=false]", DNAME[opp(*o)]),
            K::Cmp(o, sub) => format!("minecraft:comparator[facing={},mode={},powered=false]", DNAME[opp(*o)], if *sub { "subtract" } else { "compare" }),
            K::Torch => "minecraft:redstone_torch[lit=true]".into(),
            K::WTorch(o) => format!("minecraft:redstone_wall_torch[facing={},lit=true]", DNAME[*o]),
        })
    }
    fn needs_floor(&self) -> bool {
        matches!(self, K::Dust | K::Rep(_) | K::Cmp(..) | K::Torch)
    }
}

/// What a cell is: a grid cell (kinds chosen by the solver), a port cell (which input or output
/// it is: literals, constant for fixed ports), or fixed air.
#[derive(Clone, Debug, PartialEq)]
enum Role {
    Grid,
    Port { ins: Vec<Lit>, outs: Vec<Lit> },
    Air,
}

struct Cell {
    pos: Pos,
    role: Role,
    /// (kind, its literal) for grid cells.
    kinds: Vec<(K, Lit)>,
    /// Port cells: "an output lamp stands here".
    lamp: Option<Lit>,
}

impl Cell {
    fn lit(&self, k: K) -> Option<Lit> {
        self.kinds.iter().find(|(x, _)| *x == k).map(|(_, l)| *l)
    }
}

struct Model {
    e: Enc,
    cells: Vec<Cell>,
    at: HashMap<Pos, usize>,
    /// Every grid cell's non-air literal (the block count).
    nonair: Vec<Lit>,
    total: Vec<Lit>,
    port_cells: Vec<usize>,
    /// R7 input cells (the judge's driven stone) instead of a redstone block written in.
    r7: bool,
    /// A bit slice's chain output (output index): a lamp block judged on its direct power.
    chain_out: Option<usize>,
    /// The slice step (chain output cell - chain input cell).
    slice: Option<Pos>,
}

fn parse_port(s: &str) -> Result<(String, Option<Pos>)> {
    let Some((n, p)) = s.split_once('=') else { return Ok((s.to_string(), None)) };
    let v: Vec<i32> = p.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
    if v.len() != 3 {
        bail!("want name=x,y,z, got {s:?}");
    }
    Ok((n.to_string(), Some([v[0], v[1], v[2]])))
}

impl Model {
    /// Literal "the cell holds kind k" (constant false when it cannot).
    fn is(&self, c: usize, k: K) -> Lit {
        self.cells[c].lit(k).unwrap_or(self.e.f())
    }
    /// OR over a cell's kinds matching `pred`.
    fn any(&mut self, c: usize, pred: impl Fn(&K) -> bool) -> Lit {
        let ls: Vec<Lit> = self.cells[c].kinds.iter().filter(|(k, _)| pred(k)).map(|(_, l)| *l).collect();
        self.e.or(&ls)
    }
    /// The cell is a conductor (stone, or the judge's lamp).
    fn cond(&self, c: usize) -> Lit {
        match &self.cells[c].role {
            Role::Grid => self.is(c, K::Solid),
            Role::Port { .. } => self.cells[c].lamp.unwrap_or(self.e.f()),
            Role::Air => self.e.f(),
        }
    }
    /// "A redstone block stands in this port cell in this combination".
    fn rb(&mut self, c: usize, combo: usize, fun: &Func) -> Lit {
        match &self.cells[c].role {
            Role::Port { ins, .. } => {
                let on: Vec<Lit> = ins.iter().enumerate().filter(|(i, _)| !fun.is_hex(*i) && fun.value(combo, *i) == 1).map(|(_, l)| *l).collect();
                self.e.or(&on)
            }
            _ => self.e.f(),
        }
    }
}

/// Per combination: the levels and helper values of every cell.
struct Frame {
    lvl: Vec<Num>,
    /// [cell][horizontal dir]: dust connection (power goes that way).
    conn: Vec<[Lit; 4]>,
}

struct Spec<'a> {
    fun: &'a Func,
    ins: &'a [(String, Option<Pos>)],
    outs: &'a [(String, Option<Pos>)],
    w: i32,
    d: i32,
    h: i32,
    palette: &'a [String],
    feedback: bool,
    auto: bool,
    in_sides: &'a str,
    out_sides: &'a str,
    r7: bool,
    /// Interchangeable input indices (auto ports: their ring cells in this order).
    sym: Vec<usize>,
    /// Bit slice: the chain output's index and the step.
    chain_out: Option<usize>,
    slice: Option<Pos>,
    /// Grid cells whose kind is fixed (a partial design to complete).
    fixed_kinds: Vec<(Pos, K)>,
}

fn side_of(p: Pos, w: i32, d: i32) -> Option<char> {
    if p[2] == -1 && p[0] >= 0 && p[0] < w {
        Some('n')
    } else if p[2] == d && p[0] >= 0 && p[0] < w {
        Some('s')
    } else if p[0] == -1 && p[2] >= 0 && p[2] < d {
        Some('w')
    } else if p[0] == w && p[2] >= 0 && p[2] < d {
        Some('e')
    } else {
        None
    }
}

fn build(sp: &Spec, active: &[usize]) -> Result<(Model, HashMap<usize, Frame>)> {
    let (fun, w, d, h) = (sp.fun, sp.w, sp.d, sp.h);
    let mut e = Enc::new();
    let mut cells: Vec<Cell> = Vec::new();
    let mut at: HashMap<Pos, usize> = HashMap::new();
    let ni = fun.inputs.len();
    let no = fun.outputs.len();
    let idx_in = |n: &str| fun.inputs.iter().position(|x| x == n).ok_or_else(|| anyhow!("no input {n} in {}", fun.kind));
    let idx_out = |n: &str| fun.outputs.iter().position(|x| x == n).ok_or_else(|| anyhow!("no output {n} in {}", fun.kind));
    let mut fixed: HashMap<Pos, (Option<usize>, Option<usize>)> = HashMap::new();
    // fixed ports: every port without --auto-ports; with it, the ports that name a cell (a bit
    // slice's chain ports), the others placed by the solver
    for (n, p) in sp.ins {
        match p {
            Some(p) => fixed.entry(*p).or_default().0 = Some(idx_in(n)?),
            None if !sp.auto => bail!("input {n}: give its cell (name=x,y,z) or use --auto-ports"),
            None => {}
        }
    }
    for (n, p) in sp.outs {
        match p {
            Some(p) => fixed.entry(*p).or_default().1 = Some(idx_out(n)?),
            None if !sp.auto => bail!("output {n}: give its cell (name=x,y,z) or use --auto-ports"),
            None => {}
        }
    }
    let in_fixed: Vec<bool> = (0..ni).map(|i| fixed.values().any(|(a, _)| *a == Some(i))).collect();
    let out_fixed: Vec<bool> = (0..no).map(|o| fixed.values().any(|(_, b)| *b == Some(o))).collect();
    let has = |s: &str| sp.palette.iter().any(|p| p == s);
    let mut port_cells = Vec::new();
    // the box: the grid plus a ring and a layer above and below (fixed air unless a port)
    for y in -1..=h {
        for z in -1..=d {
            for x in -1..=w {
                let p = [x, y, z];
                let in_grid = x >= 0 && x < w && z >= 0 && z < d && y >= 0 && y < h;
                let ring = !in_grid && y == 1 && side_of(p, w, d).is_some();
                let mut lamp = None;
                let role = if let Some((i, o)) = fixed.get(&p) {
                    let ins: Vec<Lit> = (0..ni).map(|k| e.konst(*i == Some(k))).collect();
                    let outs: Vec<Lit> = (0..no).map(|k| e.konst(*o == Some(k))).collect();
                    lamp = Some(e.konst(o.is_some()));
                    port_cells.push(cells.len());
                    Role::Port { ins, outs }
                } else if sp.auto && ring {
                    let side = side_of(p, w, d).unwrap();
                    let ins: Vec<Lit> = (0..ni).map(|k| if sp.in_sides.contains(side) && !in_fixed[k] { e.var() } else { e.f() }).collect();
                    let outs: Vec<Lit> = (0..no).map(|k| if sp.out_sides.contains(side) && !out_fixed[k] { e.var() } else { e.f() }).collect();
                    let all: Vec<Lit> = ins.iter().chain(&outs).copied().filter(|l| *l != e.f()).collect();
                    e.at_most_one(&all);
                    lamp = Some(e.or(&outs));
                    port_cells.push(cells.len());
                    Role::Port { ins, outs }
                } else if in_grid {
                    Role::Grid
                } else {
                    Role::Air
                };
                let mut kinds = Vec::new();
                if role == Role::Grid {
                    let mut ks = vec![K::Air];
                    if has("stone") {
                        ks.push(K::Solid);
                    }
                    if has("rblock") {
                        ks.push(K::RBlock);
                    }
                    if y > 0 {
                        if has("dust") {
                            ks.push(K::Dust);
                        }
                        for o in 0..4 {
                            if has("rep") {
                                ks.push(K::Rep(o));
                            }
                            if has("cmp") {
                                ks.push(K::Cmp(o, false));
                                ks.push(K::Cmp(o, true));
                            }
                        }
                        if has("torch") {
                            ks.push(K::Torch);
                        }
                    }
                    if has("wtorch") {
                        for o in 0..4 {
                            ks.push(K::WTorch(o));
                        }
                    }
                    for k in ks {
                        let l = e.var();
                        kinds.push((k, l));
                    }
                }
                at.insert(p, cells.len());
                cells.push(Cell { pos: p, role, kinds, lamp });
            }
        }
    }
    for p in fixed.keys() {
        if !at.contains_key(p) {
            bail!("port at {p:?} is outside the grid's ring");
        }
    }
    let mut m = Model { e, cells, at, nonair: Vec::new(), total: Vec::new(), port_cells, r7: sp.r7, chain_out: sp.chain_out, slice: sp.slice };
    // a partial design: its grid cells keep their kinds
    for (p, k) in &sp.fixed_kinds {
        let Some(&c) = m.at.get(p) else { bail!("--fix: {p:?} is outside the box") };
        if m.cells[c].role != Role::Grid {
            continue;
        }
        let Some(l) = m.cells[c].lit(*k) else { bail!("--fix: {p:?}: {k:?} is not offered there (palette, floor layer?)") };
        m.e.add(&[l]);
    }
    if sp.auto {
        // symmetry breaking: interchangeable inputs take ring cells in their given order (input
        // sym[t + 1] in ring cell k forbids input sym[t] in any ring cell at or after k)
        let pcs = m.port_cells.clone();
        for t in 0..sp.sym.len().saturating_sub(1) {
            let (i, j) = (sp.sym[t], sp.sym[t + 1]);
            for (k, &ck) in pcs.iter().enumerate() {
                let Role::Port { ins: ink, .. } = m.cells[ck].role.clone() else { continue };
                if ink[j] == m.e.f() {
                    continue;
                }
                for &cl in &pcs[k..] {
                    let Role::Port { ins: inl, .. } = m.cells[cl].role.clone() else { continue };
                    if inl[i] != m.e.f() {
                        m.e.add(&[!ink[j], !inl[i]]);
                    }
                }
            }
        }
    }
    if sp.auto {
        // every port in exactly one ring cell
        for i in 0..ni {
            let ls: Vec<Lit> = m.port_cells.iter().filter_map(|&c| if let Role::Port { ins, .. } = &m.cells[c].role { Some(ins[i]) } else { None }).collect();
            m.e.exactly_one(&ls);
        }
        for o in 0..no {
            let ls: Vec<Lit> = m.port_cells.iter().filter_map(|&c| if let Role::Port { outs, .. } = &m.cells[c].role { Some(outs[o]) } else { None }).collect();
            m.e.exactly_one(&ls);
        }
    }
    structure(&mut m);
    let na = m.nonair.clone();
    m.total = m.e.totalizer(&na);
    if !sp.feedback {
        no_feedback(&mut m);
    }
    let mut frames = HashMap::new();
    for &combo in active {
        add_combo(&mut m, &mut frames, fun, combo)?;
    }
    Ok((m, frames))
}

/// Add a combination's steady state to the model (and the update-less rule for the tested
/// transitions between it and the combinations already there).
fn add_combo(m: &mut Model, frames: &mut HashMap<usize, Frame>, fun: &Func, combo: usize) -> Result<()> {
    if frames.contains_key(&combo) {
        return Ok(());
    }
    let f = frame(m, fun, combo)?;
    frames.insert(combo, f);
    let n = fun.n();
    let all = (1usize << n) - 1;
    let tested = |x: usize, y: usize| (x ^ y).count_ones() == 1 || (x ^ y) == all;
    let others: Vec<usize> = frames.keys().copied().filter(|&o| o != combo && tested(o, combo)).collect();
    for o in others {
        let (x, y) = (combo.min(o), combo.max(o));
        no_updateless_pair(m, &frames[&x], &frames[&y]);
    }
    Ok(())
}

/// One kind per grid cell; supports; no repeater locks.
fn structure(m: &mut Model) {
    for c in 0..m.cells.len() {
        if m.cells[c].role != Role::Grid {
            continue;
        }
        let ls: Vec<Lit> = m.cells[c].kinds.iter().map(|(_, l)| *l).collect();
        m.e.exactly_one(&ls);
        let air = m.is(c, K::Air);
        m.nonair.push(!air);
        let pos = m.cells[c].pos;
        let below = m.at.get(&add(pos, DOWN)).copied();
        let kinds = m.cells[c].kinds.clone();
        for (k, l) in kinds {
            if k.needs_floor() {
                let sup: Vec<Lit> = match below {
                    Some(b) if m.cells[b].role == Role::Grid => {
                        if k == K::Torch {
                            vec![m.is(b, K::Solid)]
                        } else {
                            vec![m.is(b, K::Solid), m.is(b, K::RBlock)]
                        }
                    }
                    _ => vec![],
                };
                let mut cl = vec![!l];
                cl.extend(sup);
                m.e.add(&cl);
            }
            if let K::WTorch(o) = k {
                let behind = m.at.get(&add(pos, opp(o))).copied();
                let sup = match behind {
                    Some(b) if m.cells[b].role == Role::Grid => m.is(b, K::Solid),
                    // R7: a torch may hang on an input cell (the judge's stone, there on and off)
                    Some(b) if m.r7 => match &m.cells[b].role {
                        Role::Port { ins, .. } => {
                            let v = ins.clone();
                            m.e.or(&v)
                        }
                        _ => m.e.f(),
                    },
                    _ => m.e.f(),
                };
                m.e.add(&[!l, sup]);
            }
            if let K::Rep(o) = k {
                for s in [(o + 1) % 4, (o + 3) % 4] {
                    if let Some(&n) = m.at.get(&add(pos, s)) {
                        let into = opp(s);
                        for kk in [K::Rep(into), K::Cmp(into, false), K::Cmp(into, true)] {
                            let x = m.is(n, kk);
                            m.e.add(&[!l, !x]);
                        }
                    }
                }
            }
        }
    }
}

/// The steady state of one input combination.
fn frame(m: &mut Model, fun: &Func, combo: usize) -> Result<Frame> {
    let n = m.cells.len();
    // redstone blocks in port cells
    let mut rb: Vec<Lit> = vec![m.e.f(); n];
    for c in m.port_cells.clone() {
        rb[c] = m.rb(c, combo, fun);
    }
    // levels (and the strong level of conductors)
    let mut lvl: Vec<Num> = Vec::with_capacity(n);
    let mut spow: Vec<Num> = Vec::with_capacity(n);
    for c in 0..n {
        let (l, s) = match m.cells[c].role {
            Role::Air => (knum(&m.e, 0), knum(&m.e, 0)),
            Role::Port { .. } => (num(&mut m.e), num(&mut m.e)),
            Role::Grid => {
                let l = num(&mut m.e);
                let s = if m.cells[c].lit(K::Solid).is_some() { num(&mut m.e) } else { knum(&m.e, 0) };
                (l, s)
            }
        };
        lvl.push(l);
        spow.push(s);
    }
    // dust connections
    let mut conn: Vec<[Lit; 4]> = vec![[m.e.f(); 4]; n];
    for c in 0..n {
        if m.cells[c].lit(K::Dust).is_none() {
            continue;
        }
        let pos = m.cells[c].pos;
        let mut raw = [m.e.f(); 4];
        for (d, r) in raw.iter_mut().enumerate() {
            let Some(&nb) = m.at.get(&add(pos, d)) else { continue };
            *r = match m.cells[nb].role {
                // R7: the input cell is the judge's stone, which dust never points at
                Role::Port { .. } => {
                    if m.r7 {
                        m.e.f()
                    } else {
                        rb[nb]
                    }
                }
                Role::Grid => m.any(nb, |k| match k {
                    K::Dust | K::RBlock | K::Torch | K::WTorch(_) | K::Cmp(..) => true,
                    K::Rep(o) => *o == d || *o == opp(d),
                    _ => false,
                }),
                Role::Air => m.e.f(),
            };
        }
        for d in 0..4 {
            let other = [raw[(d + 1) % 4], raw[(d + 3) % 4]];
            let none = m.e.and(&[!other[0], !other[1]]);
            conn[c][d] = m.e.or(&[raw[d], none]);
        }
    }
    // emissions toward each neighbor: em (getSignal), dem (getDirectSignal), cem (side input)
    let mut em: Vec<[Option<Num>; 6]> = vec![Default::default(); n];
    let mut dem: Vec<[Option<Num>; 6]> = vec![Default::default(); n];
    let mut cem: Vec<[Option<Num>; 6]> = vec![Default::default(); n];
    for c in 0..n {
        let pos = m.cells[c].pos;
        for d in 0..6 {
            if !m.at.contains_key(&add(pos, d)) {
                continue;
            }
            match m.cells[c].role {
                Role::Air => {}
                Role::Port { .. } => {
                    // a redstone block (15 everywhere, a side input too) or a lamp (a conductor);
                    // R7: the judge's driven stone, a conductor too (a comparator's side reads
                    // only signal sources)
                    em[c][d] = Some(lvl[c].clone());
                    cem[c][d] = Some(if m.r7 { knum(&m.e, 0) } else { bnum(&m.e, rb[c]) });
                }
                Role::Grid => emit(m, c, d, &lvl[c], &conn[c], &mut em, &mut dem, &mut cem),
            }
        }
    }
    // hex inputs: the chest's analog value per port cell (literal "input i's chest is here")
    let mut chest: Vec<Vec<(Lit, Num)>> = vec![Vec::new(); n];
    for c in m.port_cells.clone() {
        if let Role::Port { ins, .. } = &m.cells[c].role {
            for (i, l) in ins.iter().enumerate() {
                if fun.is_hex(i) && *l != m.e.f() {
                    chest[c].push((*l, knum(&m.e, fun.value(combo, i) as u8)));
                }
            }
        }
    }
    // each cell's level from its kind
    for c in 0..n {
        cell_level(m, c, &lvl, &spow, &rb, &em, &dem, &cem, &chest);
    }
    // the outputs: a lamp gets a signal exactly when the function says so
    for c in m.port_cells.clone() {
        let Role::Port { outs, .. } = m.cells[c].role.clone() else { continue };
        let pos = m.cells[c].pos;
        // the level into the lamp: max over what its neighbors give it
        let mut terms: Vec<Num> = Vec::new();
        for d in 0..6 {
            let Some(&nb) = m.at.get(&add(pos, d)) else { continue };
            if let Some(v) = &em[nb][opp(d)] {
                terms.push(v.clone());
            }
        }
        for (o, here) in outs.iter().enumerate() {
            if *here == m.e.f() {
                continue;
            }
            let want = fun.table[combo][o];
            // a bit slice's chain output: the lamp block's own direct power (the next copy reads
            // it as a powered conductor: its dust takes the strong level, its torch goes out)
            if m.chain_out == Some(o) {
                if want > 0 {
                    m.e.add(&[!*here, lvl[c][1]]);
                } else {
                    m.e.add(&[!*here, !lvl[c][1]]);
                }
                continue;
            }
            if fun.levels[o] {
                // exactly the value: >= k for k <= want, and never above
                for k in 1..16 {
                    let ors: Vec<Lit> = terms.iter().map(|v| v[k]).collect();
                    if k as u8 <= want {
                        let mut cl = vec![!*here];
                        cl.extend(ors);
                        m.e.add(&cl);
                    } else {
                        for l in ors {
                            m.e.add(&[!*here, !l]);
                        }
                    }
                }
            } else if want > 0 {
                let mut cl = vec![!*here];
                cl.extend(terms.iter().map(|v| v[1]));
                m.e.add(&cl);
            } else {
                for v in &terms {
                    m.e.add(&[!*here, !v[1]]);
                }
            }
        }
    }
    Ok(Frame { lvl, conn })
}

/// A grid cell's emissions toward direction `d`, per kind.
#[allow(clippy::too_many_arguments)]
fn emit(m: &mut Model, c: usize, d: usize, l: &Num, conn: &[Lit; 4], em: &mut [[Option<Num>; 6]], dem: &mut [[Option<Num>; 6]], cem: &mut [[Option<Num>; 6]]) {
    let zero = knum(&m.e, 0);
    let full = knum(&m.e, 15);
    let dust_side: Option<Num> = if d < 4 && m.cells[c].lit(K::Dust).is_some() {
        let mut v = vec![m.e.t];
        for k in 1..16 {
            let a = m.e.and(&[l[k], conn[d]]);
            v.push(a);
        }
        Some(v)
    } else {
        None
    };
    // group the kinds by what they emit: (value, kinds' literals)
    let mut sig: Vec<(Num, Vec<Lit>)> = Vec::new();
    let mut dsig: Vec<(Num, Vec<Lit>)> = Vec::new();
    let mut csig: Vec<(Num, Vec<Lit>)> = Vec::new();
    let push = |v: &mut Vec<(Num, Vec<Lit>)>, x: &Num, g: Lit| match v.iter_mut().find(|(y, _)| y == x) {
        Some((_, gs)) => gs.push(g),
        None => v.push((x.clone(), vec![g])),
    };
    for (k, g) in m.cells[c].kinds.clone() {
        let s: Num = match k {
            K::Air => zero.clone(),
            K::Solid => l.clone(),
            K::RBlock => full.clone(),
            K::Dust => {
                if d == UP {
                    zero.clone()
                } else if d == DOWN {
                    l.clone()
                } else {
                    dust_side.clone().unwrap()
                }
            }
            K::Rep(o) | K::Cmp(o, _) => {
                if d == o {
                    l.clone()
                } else {
                    zero.clone()
                }
            }
            K::Torch => {
                if d == DOWN {
                    zero.clone()
                } else {
                    l.clone()
                }
            }
            K::WTorch(o) => {
                if d == opp(o) {
                    zero.clone()
                } else {
                    l.clone()
                }
            }
        };
        let ds: Num = match k {
            K::Rep(_) | K::Cmp(..) | K::Dust => s.clone(),
            K::Torch | K::WTorch(_) => {
                if d == UP {
                    l.clone()
                } else {
                    zero.clone()
                }
            }
            _ => zero.clone(),
        };
        let cs: Num = match k {
            K::RBlock => full.clone(),
            K::Dust => l.clone(),
            K::Rep(o) | K::Cmp(o, _) => {
                if d == o {
                    l.clone()
                } else {
                    zero.clone()
                }
            }
            _ => zero.clone(),
        };
        push(&mut sig, &s, g);
        push(&mut dsig, &ds, g);
        push(&mut csig, &cs, g);
    }
    let mut out = |groups: Vec<(Num, Vec<Lit>)>| -> Num {
        if groups.len() == 1 {
            return groups[0].0.clone();
        }
        let q = num(&mut m.e);
        for (v, gs) in groups {
            let g = m.e.or(&gs);
            gate_eq(&mut m.e, g, &q, &v);
        }
        q
    };
    em[c][d] = Some(out(sig));
    dem[c][d] = Some(out(dsig));
    cem[c][d] = Some(out(csig));
}

/// A cell's level from its kind (conductors: the direct signals into them).
#[allow(clippy::too_many_arguments)]
fn cell_level(m: &mut Model, c: usize, lvl: &[Num], spow: &[Num], rb: &[Lit], em: &[[Option<Num>; 6]], dem: &[[Option<Num>; 6]], cem: &[[Option<Num>; 6]], chest: &[Vec<(Lit, Num)>]) {
    let pos = m.cells[c].pos;
    let role = m.cells[c].role.clone();
    if role == Role::Air {
        return;
    }
    // the neighbors that give into c: (neighbor, the direction from it to c)
    let nbs: Vec<(usize, usize)> = (0..6).filter_map(|d| m.at.get(&add(pos, d)).map(|&nb| (nb, opp(d)))).collect();
    let g_cond = m.cond(c);
    if g_cond != m.e.f() {
        let mut terms: Vec<(Lit, Num, usize)> = Vec::new();
        let mut sterms: Vec<(Lit, Num, usize)> = Vec::new();
        for &(nb, dir) in &nbs {
            if let Some(v) = &dem[nb][dir] {
                terms.push((m.e.t, v.clone(), 0));
                let nd = !m.is(nb, K::Dust);
                sterms.push((nd, v.clone(), 0));
            }
        }
        let t: Vec<(Lit, &Num, usize)> = terms.iter().map(|(a, b, c)| (*a, b, *c)).collect();
        gate_max(&mut m.e, g_cond, &lvl[c], &t);
        let st: Vec<(Lit, &Num, usize)> = sterms.iter().map(|(a, b, c)| (*a, b, *c)).collect();
        gate_max(&mut m.e, g_cond, &spow[c], &st);
    }
    if let Role::Port { .. } = role {
        // not a lamp: 15 with a redstone block, else 0
        let r = rb[c];
        for k in 1..16 {
            m.e.add(&[g_cond, !lvl[c][k], r]);
            m.e.add(&[g_cond, lvl[c][k], !r]);
        }
        return;
    }
    let q = lvl[c].clone();
    for (k, g) in m.cells[c].kinds.clone() {
        match k {
            K::Air => {
                let z = knum(&m.e, 0);
                gate_eq(&mut m.e, g, &q, &z)
            }
            K::RBlock => {
                let f = knum(&m.e, 15);
                gate_eq(&mut m.e, g, &q, &f)
            }
            K::Solid => {}
            K::Dust => {
                let mut terms: Vec<(Lit, Num, usize)> = Vec::new();
                for &(nb, dir) in &nbs {
                    let ncond = m.cond(nb);
                    let ndust = m.is(nb, K::Dust);
                    if ncond != m.e.f() {
                        terms.push((ncond, spow[nb].clone(), 0));
                    }
                    if let Some(v) = &em[nb][dir] {
                        let other = m.e.and(&[!ncond, !ndust]);
                        terms.push((other, v.clone(), 0));
                    }
                    if dir < 4 && ndust != m.e.f() {
                        terms.push((ndust, lvl[nb].clone(), 1));
                    }
                }
                let t: Vec<(Lit, &Num, usize)> = terms.iter().map(|(a, b, c)| (*a, b, *c)).collect();
                gate_max(&mut m.e, g, &q, &t);
            }
            K::Rep(o) | K::Cmp(o, _) => {
                // the back: the cell behind (opposite the output)
                let Some(&b) = m.at.get(&add(pos, opp(o))) else {
                    m.e.add(&[!g]);
                    continue;
                };
                let bdust = m.is(b, K::Dust);
                let bv = num(&mut m.e);
                let mut bt: Vec<(Lit, Num, usize)> = vec![(bdust, lvl[b].clone(), 0)];
                if let Some(v) = &em[b][o] {
                    bt.push((!bdust, v.clone(), 0));
                }
                // a comparator reads a hex input's chest at its back (`getAnalogOutputSignal`)
                if matches!(k, K::Cmp(..)) {
                    for (here, v) in &chest[b] {
                        bt.push((*here, v.clone(), 0));
                    }
                }
                let t: Vec<(Lit, &Num, usize)> = bt.iter().map(|(a, b, c)| (*a, b, *c)).collect();
                gate_max(&mut m.e, g, &bv, &t);
                if let K::Cmp(_, sub) = k {
                    let sv = num(&mut m.e);
                    let mut st: Vec<(Lit, Num, usize)> = Vec::new();
                    for s in [(o + 1) % 4, (o + 3) % 4] {
                        if let Some(&sn) = m.at.get(&add(pos, s))
                            && let Some(v) = &cem[sn][opp(s)]
                        {
                            st.push((m.e.t, v.clone(), 0));
                        }
                    }
                    let t: Vec<(Lit, &Num, usize)> = st.iter().map(|(a, b, c)| (*a, b, *c)).collect();
                    gate_max(&mut m.e, g, &sv, &t);
                    comparator(&mut m.e, g, &q, &bv, &sv, sub);
                } else {
                    let b15 = bnum(&m.e, bv[1]);
                    gate_eq(&mut m.e, g, &q, &b15);
                }
            }
            K::Torch | K::WTorch(_) => {
                let apos = match k {
                    K::WTorch(o) => add(pos, opp(o)),
                    _ => add(pos, DOWN),
                };
                let a = m.at[&apos];
                let lit = bnum(&m.e, !lvl[a][1]);
                gate_eq(&mut m.e, g, &q, &lit);
            }
        }
    }
}

/// A dust next to an input cell changes shape when the input's redstone block comes or goes; if
/// its power stays the same, nothing gets a block update (`updateShape` only), so whatever it now
/// points to never reacts: the judge's update-less output (docs/circuits.md). For every tested
/// transition (a single flip or a complement jump), a dust next to a port cell whose shape changes
/// must change its power too (applied per tested pair as combinations join the model).
fn no_updateless_pair(m: &mut Model, fx: &Frame, fy: &Frame) {
    for c in 0..m.cells.len() {
        let Some(dust) = m.cells[c].lit(K::Dust) else { continue };
        let pos = m.cells[c].pos;
        let near_port = (0..4).any(|d| m.at.get(&add(pos, d)).is_some_and(|&nb| matches!(m.cells[nb].role, Role::Port { .. })));
        if !near_port {
            continue;
        }
        let mut sd = Vec::new();
        for d in 0..4 {
            let (a, b) = (fx.conn[c][d], fy.conn[c][d]);
            if a == b {
                continue;
            }
            let xa = m.e.and(&[a, !b]);
            let xb = m.e.and(&[!a, b]);
            sd.push(xa);
            sd.push(xb);
        }
        if sd.is_empty() {
            continue;
        }
        let shape_diff = m.e.or(&sd);
        let mut ld = vec![!dust, !shape_diff];
        for k in 1..16 {
            let (a, b) = (fx.lvl[c][k], fy.lvl[c][k]);
            let xa = m.e.and(&[a, !b]);
            let xb = m.e.and(&[!a, b]);
            ld.push(xa);
            ld.push(xb);
        }
        m.e.add(&ld);
    }
}

/// Feed-forward only: every cell gets a rank, and every cell a signal can flow from has a lower
/// rank than the cell it flows into (dust into dust: no higher), so no loop goes through a torch,
/// a diode or a block. Steady-state latches and loops (a torch pair started in phase oscillates)
/// are outside the model; this keeps the solver away from them. Edges are structural and
/// conservative: dust next to a block counts as powering it whatever its shape.
fn no_feedback(m: &mut Model) {
    const R: usize = 24;
    let n = m.cells.len();
    // two ranks per grid and port cell (order-encoded: r[k] = rank >= k): `rd` its level, `rs` a
    // conductor's strong level (what dust reads: the dust it powers does not feed it back)
    let mut rd: Vec<Option<Vec<Lit>>> = vec![None; n];
    let mut rs: Vec<Option<Vec<Lit>>> = vec![None; n];
    for c in 0..n {
        if matches!(m.cells[c].role, Role::Grid | Role::Port { .. }) {
            for r in [&mut rd, &mut rs] {
                let mut v = vec![m.e.t];
                for _ in 1..=R {
                    v.push(m.e.var());
                }
                for k in 1..R {
                    m.e.add(&[!v[k + 1], v[k]]);
                }
                r[c] = Some(v);
            }
            let (a, b) = (rd[c].clone().unwrap(), rs[c].clone().unwrap());
            for k in 1..=R {
                m.e.add(&[!b[k], a[k]]);
            }
        }
    }
    for v in 0..n {
        if rd[v].is_none() {
            continue;
        }
        let pos = m.cells[v].pos;
        // (source cell, from its strong rank, into v's strong rank, edge literal, strict)
        let mut edges: Vec<(usize, bool, bool, Lit, bool)> = Vec::new();
        let vcond = m.cond(v);
        let vdust = m.is(v, K::Dust);
        for d in 0..6 {
            let Some(&u) = m.at.get(&add(pos, d)) else { continue };
            if rd[u].is_none() {
                continue;
            }
            let into = opp(d); // the direction from u to v
            let u_diode_into = m.any(u, |k| matches!(k, K::Rep(o) | K::Cmp(o, _) if *o == into));
            let u_torch_into = m.any(u, |k| match k {
                K::Torch => into != DOWN,
                K::WTorch(o) => into != opp(*o),
                _ => false,
            });
            let u_torch_up = m.any(u, |k| matches!(k, K::Torch | K::WTorch(_)) && into == UP);
            let u_dust = m.is(u, K::Dust);
            let u_cond = m.cond(u);
            // v a conductor: direct signals (diodes, torches below: its strong level; dust: only
            // its level)
            let a = m.e.or(&[u_diode_into, u_torch_up]);
            let e1 = m.e.and(&[vcond, a]);
            edges.push((u, false, true, e1, true));
            if into != UP {
                let e2 = m.e.and(&[vcond, u_dust]);
                edges.push((u, false, false, e2, true));
            }
            // v dust: a conductor's strong level, diodes and torches, horizontal wires
            let e3 = m.e.and(&[vdust, u_cond]);
            edges.push((u, true, false, e3, true));
            let b = m.e.or(&[u_diode_into, u_torch_into]);
            let e3b = m.e.and(&[vdust, b]);
            edges.push((u, false, false, e3b, true));
            if d < 4 {
                let e4 = m.e.and(&[vdust, u_dust]);
                edges.push((u, false, false, e4, false));
            }
            // v a diode: its back, and a comparator's sides; a torch: its block
            for (k, g) in m.cells[v].kinds.clone() {
                match k {
                    K::Rep(o) | K::Cmp(o, _) => {
                        if d == opp(o) {
                            edges.push((u, false, false, g, true));
                        }
                        if matches!(k, K::Cmp(..)) && (d == (o + 1) % 4 || d == (o + 3) % 4) {
                            let side = m.e.or(&[u_dust, u_diode_into]);
                            let e5 = m.e.and(&[g, side]);
                            edges.push((u, false, false, e5, true));
                        }
                    }
                    K::Torch if d == DOWN => edges.push((u, false, false, g, true)),
                    K::WTorch(o) if d == opp(o) => edges.push((u, false, false, g, true)),
                    _ => {}
                }
            }
        }
        for (u, from_strong, into_strong, e, strict) in edges {
            if e == m.e.f() {
                continue;
            }
            let ru = if from_strong { rs[u].clone().unwrap() } else { rd[u].clone().unwrap() };
            let rv = if into_strong { rs[v].clone().unwrap() } else { rd[v].clone().unwrap() };
            if strict {
                m.e.add(&[!e, rv[1]]);
                for k in 1..=R {
                    if k == R {
                        m.e.add(&[!e, !ru[k]]);
                    } else {
                        m.e.add(&[!e, !ru[k], rv[k + 1]]);
                    }
                }
            } else {
                for k in 1..=R {
                    m.e.add(&[!e, !ru[k], rv[k]]);
                }
            }
        }
    }
}

/// `g -> q = (sub ? max(b - s, 0) : (b >= s ? b : 0))`.
fn comparator(e: &mut Enc, g: Lit, q: &Num, b: &Num, s: &Num, sub: bool) {
    if sub {
        // q >= k <-> exists v: s == v and b >= v + k
        for k in 1..16 {
            let mut ors = vec![!g, !q[k]];
            for v in 0..16 {
                if v + k > 15 {
                    break;
                }
                let s_eq = if v < 15 { e.and(&[s[v], !s[v + 1]]) } else { s[15] };
                let a = e.and(&[s_eq, b[v + k]]);
                e.add(&[!g, !a, q[k]]);
                ors.push(a);
            }
            e.add(&ors);
        }
    } else {
        // ge = (b >= s): for every level v, s >= v -> b >= v
        let mut parts = Vec::new();
        for v in 1..16 {
            let p = e.or(&[!s[v], b[v]]);
            parts.push(p);
        }
        let ge = e.and(&parts);
        for k in 1..16 {
            let a = e.and(&[ge, b[k]]);
            e.add(&[!g, !q[k], a]);
            e.add(&[!g, !a, q[k]]);
        }
    }
}

/// Decode the model's grid and ports into a design.
fn decode(m: &Model, fun: &Func, name: &str) -> mctas::design::Design {
    let mut d = mctas::design::Design { name: name.to_string(), ..Default::default() };
    d.meta.insert("circuit".into(), fun.kind.clone());
    for c in &m.cells {
        match &c.role {
            Role::Grid => {
                for (k, l) in &c.kinds {
                    if m.e.val(*l)
                        && let Some(s) = k.state()
                    {
                        d.blocks.insert(c.pos, s);
                    }
                }
            }
            Role::Port { ins, outs } => {
                for (i, l) in ins.iter().enumerate() {
                    if m.e.val(*l) {
                        d.ports.insert(format!("input.{}", fun.inputs[i]), c.pos);
                        if fun.is_hex(i) {
                            // a strength input: the judge refills a chest of the design
                            d.blocks.insert(c.pos, "minecraft:chest[facing=north,type=single,waterlogged=false]".into());
                            d.meta.insert(format!("port input.{}", fun.inputs[i]), "strength".into());
                        }
                    }
                }
                for (o, l) in outs.iter().enumerate() {
                    if m.e.val(*l) {
                        d.ports.insert(format!("output.{}", fun.outputs[o]), c.pos);
                        if m.chain_out == Some(o) {
                            d.blocks.insert(c.pos, "minecraft:redstone_lamp[lit=false]".into());
                        }
                    }
                }
            }
            Role::Air => {}
        }
    }
    if let Some(s) = m.slice {
        d.meta.insert("slice".into(), format!("{} {} {}", s[0], s[1], s[2]));
    }
    d
}

pub fn cmd(a: SynthArgs) -> Result<()> {
    let ins: Vec<(String, Option<Pos>)> = a.ins.iter().map(|s| parse_port(s)).collect::<Result<_>>()?;
    let outs: Vec<(String, Option<Pos>)> = a.outs.iter().map(|s| parse_port(s)).collect::<Result<_>>()?;
    let in_names: Vec<String> = ins.iter().map(|(n, _)| n.clone()).collect();
    let out_names: Vec<String> = outs.iter().map(|(n, _)| n.clone()).collect();
    let fun = Func::of(&a.kind, &in_names, &out_names)?;
    let (w, d) = a.grid.split_once('x').ok_or_else(|| anyhow!("--grid WxD"))?;
    let (w, d): (i32, i32) = (w.parse()?, d.parse()?);
    let palette: Vec<String> = a.palette.split(',').map(|s| s.trim().to_string()).collect();
    let t0 = std::time::Instant::now();
    let sym: Vec<usize> = a.sym.iter().map(|n| fun.inputs.iter().position(|x| x == n).ok_or_else(|| anyhow!("--sym {n}: no such input"))).collect::<Result<_>>()?;
    let (chain_out, slice) = match a.chain.as_slice() {
        [] => (None, None),
        [ci, co] => {
            let pi = ins.iter().find(|(n, _)| n == ci).and_then(|(_, p)| *p).ok_or_else(|| anyhow!("--chain: input {ci} needs a fixed cell (--in {ci}=x,y,z)"))?;
            let po = outs.iter().find(|(n, _)| n == co).and_then(|(_, p)| *p).ok_or_else(|| anyhow!("--chain: output {co} needs a fixed cell (--out {co}=x,y,z)"))?;
            let oi = fun.outputs.iter().position(|x| x == co).ok_or_else(|| anyhow!("--chain: no output {co}"))?;
            (Some(oi), Some([po[0] - pi[0], po[1] - pi[1], po[2] - pi[2]]))
        }
        _ => bail!("--chain cin,cout"),
    };
    let fixed_kinds: Vec<(Pos, K)> = match &a.fix {
        Some(p) => {
            let fd = mctas::design::Design::load(p)?;
            let mut v = Vec::new();
            for (pos, s) in &fd.blocks {
                match kind_of(s) {
                    Some(k) => v.push((*pos, k)),
                    None if s.contains("redstone_lamp") => {}
                    None => bail!("--fix: {pos:?} {s}: not a kind the synthesizer offers"),
                }
            }
            v
        }
        None => Vec::new(),
    };
    let sp = Spec { fun: &fun, ins: &ins, outs: &outs, w, d, h: a.height, palette: &palette, feedback: a.feedback, auto: a.auto_ports, in_sides: &a.in_sides, out_sides: &a.out_sides, r7: a.r7, sym, chain_out, slice, fixed_kinds };
    // the combinations in the model: all of them when few, else a sample grown by the
    // counterexamples redsim finds (CEGIS)
    let total = fun.table.len();
    let mut active: Vec<usize> = if total <= a.sample.max(1) {
        (0..total).collect()
    } else {
        let mut v: Vec<usize> = vec![0, total - 1];
        for b in 0..fun.n() {
            v.push(1 << b);
            v.push((total - 1) ^ (1 << b));
        }
        let mut x: u64 = 0x9e3779b97f4a7c15;
        while v.len() < a.sample {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            v.push((x % total as u64) as usize);
        }
        v.sort();
        v.dedup();
        v
    };
    let (mut m, mut frames) = build(&sp, &active)?;
    println!("model: {} cells, {} vars, {} clauses, {} of {} combinations ({:.1} s)", m.cells.len(), m.e.n, m.e.clauses, active.len(), total, t0.elapsed().as_secs_f64());
    let seq = crate::func::walk(fun.n());
    let mut bound = a.max_blocks.unwrap_or(m.nonair.len());
    let mut best: Option<usize> = None;
    let mut found = 0usize;
    if let Some(dir) = &a.out_dir {
        std::fs::create_dir_all(dir)?;
    }
    for it in 0..a.iters {
        let mut assumps = Vec::new();
        if bound < m.total.len() {
            assumps.push(!m.total[bound]);
        }
        let ts = std::time::Instant::now();
        let r = m.e.s.solve_assumps(&assumps)?;
        if r != SolverResult::Sat {
            println!("round {it}: no design with at most {bound} blocks ({:.1} s): {}", ts.elapsed().as_secs_f64(), if best.is_some() { "the last one is the smallest in this grid" } else { "none in this grid" });
            break;
        }
        let blocks = m.nonair.iter().filter(|l| m.e.val(**l)).count();
        let name = format!("{}-{blocks}b-{it}", a.name);
        let design = decode(&m, &fun, &name);
        if a.r7 {
            // the circuit judge's own redsim run (R7 drivers, lamps, every rule)
            let verdict = circuit::Circuit::from_design(design.clone()).and_then(|c| {
                let rep = circuit::verify::sim(&c, &circuit::verify::Options::default())?;
                let pass = rep.outcome.as_ref().is_some_and(|o| o.pass);
                Ok((pass, rep.summary(&c)))
            });
            match verdict {
                Ok((true, line)) => {
                    found += 1;
                    println!("round {it}: {blocks} blocks: PASS {line} ({:.1} s)", ts.elapsed().as_secs_f64());
                    if let Some(dir) = &a.out_dir {
                        let p = dir.join(format!("{name}.txt"));
                        design.save(&p)?;
                        println!("  wrote {}", p.display());
                    } else {
                        println!("{}", design.to_text());
                    }
                    best = Some(blocks);
                    if found >= a.keep {
                        bound = blocks.saturating_sub(1);
                        found = 0;
                    }
                }
                Ok((false, line)) => {
                    println!("round {it}: {blocks} blocks: circuit sim rejects: {line} ({:.1} s)", ts.elapsed().as_secs_f64());
                    if std::env::var("SYNTH_DEBUG").is_ok() {
                        println!("{}", design.to_text());
                    }
                }
                Err(e) => println!("round {it}: {blocks} blocks: not a circuit: {e:#}"),
            }
            block_structure(&mut m);
            continue;
        }
        let rig = Rig::from_design(&design, &fun)?;
        let run = eval::run(&rig, &fun, &seq, a.gap);
        if run.pass {
            found += 1;
            println!("round {it}: {blocks} blocks: PASS redsim, latency {} gt, glitches {} ({:.1} s)", run.latency, run.glitches, ts.elapsed().as_secs_f64());
            if let Some(dir) = &a.out_dir {
                let p = dir.join(format!("{name}.txt"));
                design.save(&p)?;
                println!("  wrote {}", p.display());
            } else {
                println!("{}", design.to_text());
            }
            best = Some(blocks);
            if found >= a.keep {
                bound = blocks.saturating_sub(1);
                found = 0;
            }
        } else {
            // a failing combination the model has not seen yet: add it, the design goes with it
            if let Some(s) = run.bad
                && !frames.contains_key(&s)
            {
                add_combo(&mut m, &mut frames, &fun, s)?;
                active.push(s);
                println!("round {it}: {blocks} blocks: redsim fails at {} (not in the model yet: added, {} combinations) ({:.1} s)", fun.label(s), active.len(), ts.elapsed().as_secs_f64());
                continue;
            }
            println!("round {it}: {blocks} blocks: redsim rejects: {} ({:.1} s)", run.why, ts.elapsed().as_secs_f64());
            if std::env::var("SYNTH_DEBUG").is_ok() {
                println!("{}", design.to_text());
            }
        }
        // block this structure (and port placement) either way
        block_structure(&mut m);
    }
    Ok(())
}

/// Forbid the model's current structure (every chosen kind and port placement) from now on.
fn block_structure(m: &mut Model) {
    let mut cl = Vec::new();
    for c in &m.cells {
        for (_, l) in &c.kinds {
            if m.e.val(*l) {
                cl.push(!*l);
            }
        }
        if let Role::Port { ins, outs } = &c.role {
            for l in ins.iter().chain(outs) {
                if *l != m.e.t && *l != m.e.f() && m.e.val(*l) {
                    cl.push(!*l);
                }
            }
        }
    }
    m.e.add(&cl);
}
