//! `mctas circuit arith synth`: SAT synthesis of small flat redstone designs for a combinational
//! function (study 12's arithmetic kinds), CEGAR against redsim.
//!
//! The model is redstone's STEADY state, read in the 26.3 source (the classes named below), for
//! every input combination at once, over a box of W x H x D cells (H = 2: a floor layer y = 0 and a
//! component layer y = 1, the study's "flat"). Every grid cell holds one of: air, stone (a
//! conductor), a redstone block, dust, a repeater or comparator (4 facings; compare or subtract),
//! a standing torch, a wall torch (4 facings). The input cells hold a redstone block when on (air
//! when off), the output cells the judge's lamp (a conductor). Per combination, every cell has a
//! level 0..15 (order-encoded) whose meaning depends on its kind:
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
//! The solver minimizes the number of grid blocks; each model is decoded to a design and run
//! through the judge's walk on redsim (crate::eval): a failing one is blocked and the solver asked
//! again (timing, update order and latches are outside the steady model).

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
    /// Input ports `name=x,y,z` (repeatable; in-grid or next to it, a component-layer cell).
    #[arg(long = "in")]
    ins: Vec<String>,
    /// Output ports `name=x,y,z` (repeatable).
    #[arg(long = "out")]
    outs: Vec<String>,
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

/// `g -> q = (b ? 15 : 0)`.
fn gate_bool(e: &mut Enc, g: Lit, q: &Num, b: Lit) {
    for k in 1..16 {
        e.add(&[!g, !q[k], b]);
        e.add(&[!g, q[k], !b]);
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

/// What a cell is: a grid cell (kinds chosen by the solver), an input port, an output lamp, or
/// fixed air.
#[derive(Clone, Debug, PartialEq)]
enum Role {
    Grid,
    Input(usize),
    Lamp(usize),
    Air,
}

struct Cell {
    pos: Pos,
    role: Role,
    /// (kind, its literal) for grid cells.
    kinds: Vec<(K, Lit)>,
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
}

fn parse_port(s: &str) -> Result<(String, Pos)> {
    let (n, p) = s.split_once('=').ok_or_else(|| anyhow!("want name=x,y,z, got {s:?}"))?;
    let v: Vec<i32> = p.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
    if v.len() != 3 {
        bail!("want name=x,y,z, got {s:?}");
    }
    Ok((n.to_string(), [v[0], v[1], v[2]]))
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
}

/// Per combination: the levels and helper values of every cell.
struct Frame {
    lvl: Vec<Num>,
    /// Strong level of conductors (dust hidden).
    spow: Vec<Num>,
    /// [cell][dir]: the signal the cell gives the neighbor in that direction (`getSignal`).
    em: Vec<[Option<Num>; 6]>,
    /// [cell][dir]: its direct signal there (`getDirectSignal`).
    dem: Vec<[Option<Num>; 6]>,
    /// [cell][dir]: its side input to a diode there (`getControlInputSignal`).
    cem: Vec<[Option<Num>; 6]>,
    /// [cell][horizontal dir]: dust connection (power goes that way).
    conn: Vec<[Lit; 4]>,
}

fn build(fun: &Func, ins: &[(String, Pos)], outs: &[(String, Pos)], w: i32, d: i32, h: i32, palette: &[String], feedback: bool) -> Result<(Model, Vec<Frame>)> {
    let mut e = Enc::new();
    let mut cells: Vec<Cell> = Vec::new();
    let mut at: HashMap<Pos, usize> = HashMap::new();
    let port_in: HashMap<Pos, usize> = ins.iter().map(|(n, p)| (*p, fun.inputs.iter().position(|x| x == n).unwrap())).collect();
    let port_out: HashMap<Pos, usize> = outs.iter().map(|(n, p)| (*p, fun.outputs.iter().position(|x| x == n).unwrap())).collect();
    let has = |s: &str| palette.iter().any(|p| p == s);
    // the box: the grid plus a ring and a layer above (fixed air unless a port)
    for y in -1..=h {
        for z in -1..=d {
            for x in -1..=w {
                let p = [x, y, z];
                let in_grid = x >= 0 && x < w && z >= 0 && z < d && y >= 0 && y < h;
                let role = if let Some(i) = port_in.get(&p) {
                    Role::Input(*i)
                } else if let Some(o) = port_out.get(&p) {
                    Role::Lamp(*o)
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
                cells.push(Cell { pos: p, role, kinds });
            }
        }
    }
    for (n, p) in ins.iter().chain(outs) {
        if !at.contains_key(p) {
            bail!("port {n} at {p:?} is outside the grid's ring");
        }
    }
    let mut m = Model { e, cells, at, nonair: Vec::new(), total: Vec::new() };
    // structure: one kind per grid cell; supports; no repeater locks
    for c in 0..m.cells.len() {
        if m.cells[c].role != Role::Grid {
            continue;
        }
        let ls: Vec<Lit> = m.cells[c].kinds.iter().map(|(_, l)| *l).collect();
        m.e.add(&ls);
        for i in 0..ls.len() {
            for j in i + 1..ls.len() {
                m.e.add(&[!ls[i], !ls[j]]);
            }
        }
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
                    _ => m.e.f(),
                };
                m.e.add(&[!l, sup]);
            }
            if let K::Rep(o) = k {
                for s in [(o + 1) % 4, (o + 3) % 4] {
                    if let Some(&n) = m.at.get(&add(pos, s)) {
                        let into = opp(s);
                        let a = m.is(n, K::Rep(into));
                        let b = m.is(n, K::Cmp(into, false));
                        let cc = m.is(n, K::Cmp(into, true));
                        m.e.add(&[!l, !a]);
                        m.e.add(&[!l, !b]);
                        m.e.add(&[!l, !cc]);
                    }
                }
            }
        }
    }
    let na = m.nonair.clone();
    m.total = m.e.totalizer(&na);
    if !feedback {
        no_feedback(&mut m);
    }
    let mut frames = Vec::new();
    for combo in 0..fun.table.len() {
        frames.push(frame(&mut m, fun, combo)?);
    }
    no_updateless(&mut m, &frames, fun.n());
    Ok((m, frames))
}

/// A dust next to an input cell changes shape when the input's redstone block comes or goes; if
/// its power stays the same, nothing gets a block update (`updateShape` only), so whatever it now
/// points to never reacts: the judge's update-less output (docs/circuits.md). For every tested
/// transition that flips such an input (a single flip or a complement jump), a dust whose shape
/// changes must change its power too.
fn no_updateless(m: &mut Model, frames: &[Frame], n: usize) {
    let inputs: Vec<(usize, Pos)> = m.cells.iter().filter_map(|c| if let Role::Input(i) = c.role { Some((i, c.pos)) } else { None }).collect();
    let all = (1usize << n) - 1;
    for c in 0..m.cells.len() {
        let Some(dust) = m.cells[c].lit(K::Dust) else { continue };
        let pos = m.cells[c].pos;
        let near: Vec<usize> = inputs.iter().filter(|(_, p)| (0..4).any(|d| add(pos, d) == *p)).map(|(i, _)| *i).collect();
        if near.is_empty() {
            continue;
        }
        let mut pairs: Vec<(usize, usize)> = Vec::new();
        for &i in &near {
            for x in 0..=all {
                if (x >> i) & 1 == 0 {
                    pairs.push((x, x | (1 << i)));
                }
            }
        }
        for x in 0..=all {
            if x < (!x & all) {
                pairs.push((x, !x & all));
            }
        }
        pairs.sort();
        pairs.dedup();
        for (x, y) in pairs {
            let mut sd = Vec::new();
            for d in 0..4 {
                let (a, b) = (frames[x].conn[c][d], frames[y].conn[c][d]);
                let xa = m.e.and(&[a, !b]);
                let xb = m.e.and(&[!a, b]);
                sd.push(xa);
                sd.push(xb);
            }
            let shape_diff = m.e.or(&sd);
            let mut ld = vec![!dust, !shape_diff];
            for k in 1..16 {
                let (a, b) = (frames[x].lvl[c][k], frames[y].lvl[c][k]);
                let xa = m.e.and(&[a, !b]);
                let xb = m.e.and(&[!a, b]);
                ld.push(xa);
                ld.push(xb);
            }
            m.e.add(&ld);
        }
    }
}

/// The steady state of one input combination.
fn frame(m: &mut Model, fun: &Func, combo: usize) -> Result<Frame> {
    let n = m.cells.len();
    let on_in = |role: &Role| matches!(role, Role::Input(i) if (combo >> i) & 1 == 1);
    // levels
    let mut lvl: Vec<Num> = Vec::with_capacity(n);
    let mut spow: Vec<Num> = Vec::with_capacity(n);
    for c in 0..n {
        let role = m.cells[c].role.clone();
        let (l, s) = match role {
            Role::Air => (knum(&m.e, 0), knum(&m.e, 0)),
            Role::Input(_) => (knum(&m.e, if on_in(&role) { 15 } else { 0 }), knum(&m.e, 0)),
            Role::Lamp(_) => (num(&mut m.e), num(&mut m.e)),
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
                Role::Input(_) => m.e.konst(on_in(&m.cells[nb].role)),
                Role::Grid => m.any(nb, |k| match k {
                    K::Dust | K::RBlock | K::Torch | K::WTorch(_) | K::Cmp(..) => true,
                    K::Rep(o) => *o == d || *o == opp(d),
                    _ => false,
                }),
                _ => m.e.f(),
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
        let role = m.cells[c].role.clone();
        for d in 0..6 {
            if !m.at.contains_key(&add(pos, d)) {
                continue;
            }
            match role {
                Role::Air => {}
                Role::Input(_) => {
                    let v = knum(&m.e, if on_in(&role) { 15 } else { 0 });
                    em[c][d] = Some(v.clone());
                    cem[c][d] = Some(v);
                }
                Role::Lamp(_) => {
                    em[c][d] = Some(lvl[c].clone());
                }
                Role::Grid => {
                    let q = num(&mut m.e);
                    let dq = num(&mut m.e);
                    let cq = num(&mut m.e);
                    let kinds = m.cells[c].kinds.clone();
                    let zero = knum(&m.e, 0);
                    let full = knum(&m.e, 15);
                    for (k, g) in kinds {
                        let l = lvl[c].clone();
                        // getSignal toward d
                        let sig: Num = match k {
                            K::Air => zero.clone(),
                            K::Solid => l.clone(),
                            K::RBlock => full.clone(),
                            K::Dust => {
                                if d == UP {
                                    zero.clone()
                                } else if d == DOWN {
                                    l.clone()
                                } else {
                                    let cd = conn[c][d];
                                    let mut v = vec![m.e.t];
                                    for kk in 1..16 {
                                        let a = m.e.and(&[l[kk], cd]);
                                        v.push(a);
                                    }
                                    v
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
                        gate_eq(&mut m.e, g, &q, &sig);
                        // getDirectSignal toward d
                        let dsig: Num = match k {
                            K::Rep(_) | K::Cmp(..) | K::Dust => sig.clone(),
                            K::Torch | K::WTorch(_) => {
                                if d == UP {
                                    l.clone()
                                } else {
                                    zero.clone()
                                }
                            }
                            _ => zero.clone(),
                        };
                        gate_eq(&mut m.e, g, &dq, &dsig);
                        // getControlInputSignal toward d (a diode's side)
                        let csig: Num = match k {
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
                        gate_eq(&mut m.e, g, &cq, &csig);
                    }
                    em[c][d] = Some(q);
                    dem[c][d] = Some(dq);
                    cem[c][d] = Some(cq);
                }
            }
        }
    }
    // each cell's level from its kind
    for c in 0..n {
        let pos = m.cells[c].pos;
        let role = m.cells[c].role.clone();
        let is_conductor = matches!(role, Role::Lamp(_)) || m.cells[c].lit(K::Solid).is_some();
        // the neighbors that give into c: (neighbor, the direction from it to c)
        let nbs: Vec<(usize, usize)> = (0..6).filter_map(|d| m.at.get(&add(pos, d)).map(|&nb| (nb, opp(d)))).collect();
        if is_conductor {
            let g = match role {
                Role::Lamp(_) => m.e.t,
                _ => m.is(c, K::Solid),
            };
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
            let q = lvl[c].clone();
            gate_max(&mut m.e, g, &q, &t);
            let st: Vec<(Lit, &Num, usize)> = sterms.iter().map(|(a, b, c)| (*a, b, *c)).collect();
            let sq = spow[c].clone();
            gate_max(&mut m.e, g, &sq, &st);
        }
        if role != Role::Grid {
            continue;
        }
        let kinds = m.cells[c].kinds.clone();
        for (k, g) in kinds {
            let q = lvl[c].clone();
            match k {
                K::Air => {
                    let f = m.e.f();
                    gate_bool(&mut m.e, g, &q, f)
                }
                K::RBlock => {
                    let t = m.e.t;
                    gate_bool(&mut m.e, g, &q, t)
                }
                K::Solid => {}
                K::Dust => {
                    let mut terms: Vec<(Lit, Num, usize)> = Vec::new();
                    for &(nb, dir) in &nbs {
                        let nrole = m.cells[nb].role.clone();
                        let ncond = match nrole {
                            Role::Lamp(_) => m.e.t,
                            Role::Grid => m.is(nb, K::Solid),
                            _ => m.e.f(),
                        };
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
                    let bpos = add(pos, opp(o));
                    let Some(&b) = m.at.get(&bpos) else {
                        m.e.add(&[!g]);
                        continue;
                    };
                    let bdust = m.is(b, K::Dust);
                    let bv = num(&mut m.e);
                    let mut bt: Vec<(Lit, Num, usize)> = vec![(bdust, lvl[b].clone(), 0)];
                    if let Some(v) = &em[b][o] {
                        bt.push((!bdust, v.clone(), 0));
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
                        gate_bool(&mut m.e, g, &q, bv[1]);
                    }
                }
                K::Torch | K::WTorch(_) => {
                    let apos = match k {
                        K::WTorch(o) => add(pos, opp(o)),
                        _ => add(pos, DOWN),
                    };
                    let a = m.at[&apos];
                    let lit = !lvl[a][1];
                    gate_bool(&mut m.e, g, &q, lit);
                }
            }
        }
    }
    // the outputs: the lamp gets a signal exactly when the function says so
    for c in 0..n {
        let Role::Lamp(o) = m.cells[c].role else { continue };
        let pos = m.cells[c].pos;
        let mut ors = Vec::new();
        for d in 0..6 {
            let Some(&nb) = m.at.get(&add(pos, d)) else { continue };
            if let Some(v) = &em[nb][opp(d)] {
                ors.push(v[1]);
            }
        }
        if fun.table[combo][o] {
            m.e.add(&ors);
        } else {
            for l in ors {
                m.e.add(&[!l]);
            }
        }
    }
    Ok(Frame { lvl, spow, em, dem, cem, conn })
}

/// Feed-forward only: every cell gets a rank, and every cell a signal can flow from has a lower
/// rank than the cell it flows into (dust into dust: no higher), so no loop goes through a torch,
/// a diode or a block. Steady-state latches and loops (a torch pair started in phase oscillates)
/// are outside the model; this keeps the solver away from them. Edges are structural and
/// conservative: dust next to a block counts as powering it whatever its shape.
fn no_feedback(m: &mut Model) {
    const R: usize = 24;
    let n = m.cells.len();
    // two ranks per grid and lamp cell (order-encoded: r[k] = rank >= k): `rd` its level, `rs` a
    // conductor's strong level (what dust reads: the dust it powers does not feed it back)
    let mut rd: Vec<Option<Vec<Lit>>> = vec![None; n];
    let mut rs: Vec<Option<Vec<Lit>>> = vec![None; n];
    for c in 0..n {
        if matches!(m.cells[c].role, Role::Grid | Role::Lamp(_)) {
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
            // rd >= rs
            let (a, b) = (rd[c].clone().unwrap(), rs[c].clone().unwrap());
            for k in 1..=R {
                m.e.add(&[!b[k], a[k]]);
            }
        }
    }
    let cond = |m: &Model, c: usize| -> Lit {
        match m.cells[c].role {
            Role::Lamp(_) => m.e.t,
            Role::Grid => m.is(c, K::Solid),
            _ => m.e.f(),
        }
    };
    for v in 0..n {
        if rd[v].is_none() {
            continue;
        }
        let pos = m.cells[v].pos;
        // (source cell, from its strong rank, into v's strong rank, edge literal, strict)
        let mut edges: Vec<(usize, bool, bool, Lit, bool)> = Vec::new();
        let vcond = cond(m, v);
        let vdust = m.is(v, K::Dust);
        for d in 0..6 {
            let Some(&u) = m.at.get(&add(pos, d)) else { continue };
            if rd[u].is_none() {
                continue;
            }
            let into = opp(d); // the direction from u to v
            // u's kinds that send a signal into v
            let u_diode_into = m.any(u, |k| matches!(k, K::Rep(o) | K::Cmp(o, _) if *o == into));
            let u_torch_into = m.any(u, |k| match k {
                K::Torch => into != DOWN,
                K::WTorch(o) => into != opp(*o),
                _ => false,
            });
            let u_torch_up = m.any(u, |k| matches!(k, K::Torch | K::WTorch(_)) && into == UP);
            let u_dust = m.is(u, K::Dust);
            let u_cond = cond(m, u);
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
            let kinds = m.cells[v].kinds.clone();
            for (k, g) in kinds {
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

/// Decode the model's grid into a design.
fn decode(m: &Model, fun: &Func, ins: &[(String, Pos)], outs: &[(String, Pos)], name: &str) -> mctas::design::Design {
    let mut d = mctas::design::Design { name: name.to_string(), ..Default::default() };
    d.meta.insert("circuit".into(), fun.kind.clone());
    for c in &m.cells {
        if c.role != Role::Grid {
            continue;
        }
        for (k, l) in &c.kinds {
            if m.e.val(*l)
                && let Some(s) = k.state()
            {
                d.blocks.insert(c.pos, s);
            }
        }
    }
    for (n, p) in ins {
        d.ports.insert(format!("input.{n}"), *p);
    }
    for (n, p) in outs {
        d.ports.insert(format!("output.{n}"), *p);
    }
    d
}

pub fn cmd(a: SynthArgs) -> Result<()> {
    let ins: Vec<(String, Pos)> = a.ins.iter().map(|s| parse_port(s)).collect::<Result<_>>()?;
    let outs: Vec<(String, Pos)> = a.outs.iter().map(|s| parse_port(s)).collect::<Result<_>>()?;
    let in_names: Vec<String> = ins.iter().map(|(n, _)| n.clone()).collect();
    let out_names: Vec<String> = outs.iter().map(|(n, _)| n.clone()).collect();
    let fun = Func::of(&a.kind, &in_names, &out_names)?;
    let (w, d) = a.grid.split_once('x').ok_or_else(|| anyhow!("--grid WxD"))?;
    let (w, d): (i32, i32) = (w.parse()?, d.parse()?);
    let palette: Vec<String> = a.palette.split(',').map(|s| s.trim().to_string()).collect();
    let t0 = std::time::Instant::now();
    let (mut m, _frames) = build(&fun, &ins, &outs, w, d, a.height, &palette, a.feedback)?;
    println!("model: {} cells, {} vars, {} clauses, {} combinations ({:.1} s)", m.cells.len(), m.e.n, m.e.clauses, fun.table.len(), t0.elapsed().as_secs_f64());
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
        let design = decode(&m, &fun, &ins, &outs, &name);
        let rig = Rig::from_design(&design, &fun)?;
        let run = eval::run(&rig, &fun, &seq, a.gap);
        if run.pass {
            found += 1;
            println!("round {it}: {blocks} blocks: PASS redsim, latency {} gt, glitches {} ({:.1} s)", run.latency, run.glitches, ts.elapsed().as_secs_f64());
            if let Some(dir) = &a.out_dir {
                let p = dir.join(format!("{name}.txt"));
                let mut dd = design.clone();
                dd.meta.insert("#".into(), format!("synthesized by mctas circuit arith synth (grid {}x{}x{}): redsim latency {} gt", w, a.height, d, run.latency));
                dd.save(&p)?;
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
            println!("round {it}: {blocks} blocks: redsim rejects: {} ({:.1} s)", run.why, ts.elapsed().as_secs_f64());
            if std::env::var("SYNTH_DEBUG").is_ok() {
                println!("{}", design.to_text());
            }
        }
        // block this structure either way (a passing one: look for others)
        let mut cl = Vec::new();
        for c in &m.cells {
            for (_, l) in &c.kinds {
                if m.e.val(*l) {
                    cl.push(!*l);
                }
            }
        }
        m.e.add(&cl);
    }
    Ok(())
}
