//! `mctas door macrosat`: an independent, exhaustive check of flat 3x3 doors at the MACRO level
//! (coordinator 2026-10-20: can a 7x7 flush door be wired at all?). Bounded model checking with
//! CaDiCaL (through boxproof's `alg::Sat`): every cell of a W x H x 1 box is one-hot over a palette
//! at every macro step, the layout is FREE (or fixed with `--fix`), and a step fires a set of
//! INDEPENDENT pistons (none writes a cell another one reads or writes, so any order gives the
//! same result: a step is a sequential run in any order; `--seq` allows one action per step).
//! Nothing comes from a mechanism generator: every layout and every action sequence within the
//! bounds is in the search. Assumptions and results:
//! designs/piston-door-3x3/tracks/clockinfra/MACROSAT.md.
//!
//! * Cells: air, stone (door stone: movable conductor), obsidian (immovable conductor), redstone
//!   block (movable power source), a WIRING cell (a static component: nothing may push into it or
//!   move it), normal / sticky pistons in the 4 in-plane facings, retracted or extended, heads.
//! * Actions of a retracted piston: a PUSH of the straight line in front of it (movable = stone,
//!   redstone blocks, retracted pistons; at most 12; it must end on an air cell inside the box;
//!   obsidian, wiring, extended pistons and heads stop it), as an EXTEND (the piston stays out,
//!   its head in front), a PULSE (it comes back at once: the line stays pushed, a sticky piston
//!   spits) or a PULSE-PULL (a sticky piston out and back with nothing in between: its head pulls
//!   the first block back; into air it pulls the block 2 ahead); of an extended piston: a RETRACT
//!   (its head goes; a sticky one pulls the movable block in front of its head).
//! * Door: the 3x3 at (0..2, 0..2) is stone at rest (step 0); after `--h` steps (the opening) it
//!   is air and the 12 frame cells are stone (FULL frame); after `--h` more (the closing) the box
//!   is exactly the rest state again.
//! * Feed (`--feed`, an optimistic NECESSARY condition for wiring): every action of a piston needs,
//!   in the state it fires from, a cell next to it on a non-front face or next to the cell above
//!   it (quasi-connectivity) that is a wiring cell or a redstone block, or a conductor (stone,
//!   obsidian) with a wiring cell, a redstone block or the lever next to it (strongly powered); or
//!   the single lever (`--lever`), hung outside the box on an edge block that never moves, next to
//!   the piston or to the cell above it.

use crate::boxproof::Args;
use crate::boxproof::alg::{FlagMode, Sat};
use anyhow::{Result, anyhow, bail};
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::Lit;
use std::collections::HashMap;

const AIR: usize = 0;
const STONE: usize = 1;
const OBS: usize = 2;
const RBLOCK: usize = 3;
const WIRE: usize = 4;
const NT: usize = 29;
/// Facings in the plane: +x (east), -x (west), +y (up), -y (down).
const DX: [i32; 4] = [1, -1, 0, 0];
const DY: [i32; 4] = [0, 0, 1, -1];
const DNAME: [&str; 4] = ["east", "west", "up", "down"];

fn piston(sticky: bool, d: usize, ext: bool) -> usize {
    5 + (sticky as usize) * 8 + d * 2 + ext as usize
}
fn head(sticky: bool, d: usize) -> usize {
    21 + (sticky as usize) * 4 + d
}
fn opp(d: usize) -> usize {
    d ^ 1
}

/// Source options of a cell's next content.
const STAY: usize = 0;
/// FROM + d: the content of the neighbor at `-d` moves in.
const FROM: usize = 1;
const TO_AIR: usize = 5;
/// HEAD + d: a head facing d appears (its piston at `-d`).
const HEAD: usize = 6;
const EXT_SELF: usize = 10;
const RET_SELF: usize = 11;
const NSRC: usize = 12;

/// The box: x in x0..x0+w, y in y0..y0+h.
#[derive(Clone, Copy)]
struct Grid {
    x0: i32,
    y0: i32,
    w: i32,
    h: i32,
}

impl Grid {
    fn idx(&self, x: i32, y: i32) -> Option<usize> {
        if x < self.x0 || y < self.y0 || x >= self.x0 + self.w || y >= self.y0 + self.h {
            return None;
        }
        Some(((y - self.y0) * self.w + (x - self.x0)) as usize)
    }
    fn xy(&self, i: usize) -> (i32, i32) {
        (self.x0 + i as i32 % self.w, self.y0 + i as i32 / self.w)
    }
    fn n(&self) -> usize {
        (self.w * self.h) as usize
    }
    fn step(&self, i: usize, d: usize) -> Option<usize> {
        let (x, y) = self.xy(i);
        self.idx(x + DX[d], y + DY[d])
    }
    /// The cells in front of `c` facing `d`, nearest first.
    fn line(&self, c: usize, d: usize) -> Vec<usize> {
        let mut line = Vec::new();
        let mut cur = c;
        while let Some(nx) = self.step(cur, d) {
            line.push(nx);
            cur = nx;
        }
        line
    }
    /// The cells that can feed a piston at `c` facing `d`: its non-front neighbors and the
    /// neighbors of the cell above it (quasi-connectivity), the piston itself excluded.
    fn feed_cells(&self, c: usize, d: usize) -> Vec<usize> {
        let mut cells: Vec<usize> = (0..4).filter(|&u| u != d).filter_map(|u| self.step(c, u)).collect();
        if let Some(a) = self.step(c, 2) {
            for u in 0..4 {
                if let Some(q) = self.step(a, u)
                    && q != c
                    && !cells.contains(&q)
                {
                    cells.push(q);
                }
            }
        }
        cells
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Extend,
    Pulse,
    PulsePull,
    Retract,
}

/// One possible action of a step.
struct Act {
    kind: Kind,
    c: usize,
    d: usize,
    /// Pushed line length (pushes).
    l: usize,
    lit: Lit,
    /// A sticky pull happens (retract, pulse-pull into air).
    pull: Option<Lit>,
    /// The cells it may write.
    writes: Vec<usize>,
}

/// Options of a run.
#[derive(Clone)]
struct Opts {
    h: usize,
    feed: bool,
    lever: bool,
    seq: bool,
    fix: Option<Vec<(i32, i32, usize)>>,
    obsidian: bool,
    rblocks: bool,
    explain: bool,
    /// Mirror symmetry breaking on free layouts of boxes symmetric about x = 1 (`--no-sym` off).
    sym: bool,
    /// `--private`: a wiring cell feeds one piston cell only (a candidate filter).
    private: bool,
    /// Island start (on with `--feed` unless `--no-start`): a wiring region acts only after the
    /// lever or a change next to it.
    start: bool,
    /// `--strict` (w86's wirability rules, filters): --private, conductor co-firing, the lever's
    /// pistons toggle only at an edge's first step and never move, --budget 2, --static-cond,
    /// --frame3.
    strict: bool,
    /// `--budget K`: a wiring cell feeds firings in at most K steps per edge.
    budget: Option<usize>,
    /// `--static-cond`: conductors feed only when they never move.
    static_cond: bool,
    /// `--frame3`: exactly 3 frame cells hold stone at rest.
    frame3: bool,
    /// `--max-pistons N`: at most N pistons (c70b: leaner seeds).
    max_pistons: Option<usize>,
    /// `--physics` (c70b: replayable seeds): co-firing and rest QC stability without the
    /// wirability filters of `--strict`.
    physics: bool,
}

struct Model {
    sat: Sat,
    g: Grid,
    /// [step][cell][type]
    ty: Vec<Vec<Vec<Lit>>>,
    acts: Vec<Vec<Act>>,
    /// The lever's possible spots: (the cell it touches, literal).
    lv: Vec<(usize, Lit)>,
    /// Per step, per (piston cell * 4 + facing): the literals that feed it, with their cell.
    feed: Vec<HashMap<usize, Vec<FeedOpt>>>,
    /// `--explain`: per cell, "a piston firing there needs no feed".
    exempt: Option<Vec<Lit>>,
}

/// The type variables of every state: one type per cell, pistons and heads consistent, wiring
/// static, the palette options.
fn states(sat: &mut Sat, g: Grid, o: &Opts, steps: usize) -> Vec<Vec<Vec<Lit>>> {
    let n = g.n();
    let ty: Vec<Vec<Vec<Lit>>> = (0..=steps).map(|_| (0..n).map(|_| (0..NT).map(|_| sat.var()).collect()).collect()).collect();
    for s in 0..=steps {
        for c in 0..n {
            sat.exactly_one(&ty[s][c]);
            if !o.obsidian {
                sat.add(&[!ty[s][c][OBS]]);
            }
            if !o.rblocks {
                sat.add(&[!ty[s][c][RBLOCK]]);
            }
            for k in [false, true] {
                for d in 0..4 {
                    match g.step(c, d) {
                        Some(f) => sat.add(&[!ty[s][c][piston(k, d, true)], ty[s][f][head(k, d)]]),
                        None => sat.add(&[!ty[s][c][piston(k, d, true)]]),
                    }
                    match g.step(c, opp(d)) {
                        Some(b) => sat.add(&[!ty[s][c][head(k, d)], ty[s][b][piston(k, d, true)]]),
                        None => sat.add(&[!ty[s][c][head(k, d)]]),
                    }
                }
            }
            if s > 0 {
                sat.add(&[!ty[s][c][WIRE], ty[0][c][WIRE]]);
                sat.add(&[ty[s][c][WIRE], !ty[0][c][WIRE]]);
            }
        }
    }
    ty
}

/// `movable` (pushable / pullable: stone, redstone block, retracted pistons) per state and cell.
fn movable(sat: &mut Sat, ty: &[Vec<Vec<Lit>>]) -> Vec<Vec<Lit>> {
    ty.iter()
        .map(|st| {
            st.iter()
                .map(|cell| {
                    let m = sat.var();
                    let mut ors = vec![cell[STONE], cell[RBLOCK]];
                    for k in [false, true] {
                        for d in 0..4 {
                            ors.push(cell[piston(k, d, false)]);
                        }
                    }
                    let mut cl = vec![!m];
                    cl.extend(ors.iter().copied());
                    sat.add(&cl);
                    for &x in &ors {
                        sat.add(&[!x, m]);
                    }
                    m
                })
                .collect()
        })
        .collect()
}

/// The single lever: hung on the front or back face of a block in the plane (rules.md: still 1
/// wide) or on the outer face of an edge block (the input device is not part of the volume); its
/// block never moves and is a full block (stone, obsidian, a piston). Either way the lever touches
/// exactly one cell of the plane, so one literal per cell: "the lever touches this cell".
fn levers(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], o: &Opts) -> (Vec<(usize, Lit)>, Vec<Option<Lit>>) {
    let n = g.n();
    let mut lv = Vec::new();
    let mut lev_at = vec![None; n];
    for e in 0..n {
        let l = sat.var();
        if !o.lever {
            sat.add(&[!l]);
        }
        let mut full = vec![!l, ty[0][e][STONE], ty[0][e][OBS]];
        for k in [false, true] {
            for d in 0..4 {
                full.push(ty[0][e][piston(k, d, false)]);
                full.push(ty[0][e][piston(k, d, true)]);
            }
        }
        sat.add(&full);
        lv.push((e, l));
        lev_at[e] = Some(l);
    }
    let all: Vec<Lit> = lv.iter().map(|x| x.1).collect();
    sat.at_most_one(&all);
    (lv, lev_at)
}

/// The source options of every cell at step `s` and what each does to the next state.
fn sources(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], s: usize) -> Vec<Vec<Lit>> {
    let n = g.n();
    let src: Vec<Vec<Lit>> = (0..n).map(|_| (0..NSRC).map(|_| sat.var()).collect()).collect();
    for x in 0..n {
        sat.exactly_one(&src[x]);
        for t in 0..NT {
            sat.add(&[!src[x][STAY], !ty[s][x][t], ty[s + 1][x][t]]);
            sat.add(&[!src[x][STAY], ty[s][x][t], !ty[s + 1][x][t]]);
        }
        for d in 0..4 {
            let (from, hd) = (src[x][FROM + d], src[x][HEAD + d]);
            match g.step(x, opp(d)) {
                Some(y) => {
                    for t in 0..NT {
                        sat.add(&[!from, !ty[s][y][t], ty[s + 1][x][t]]);
                        sat.add(&[!from, ty[s][y][t], !ty[s + 1][x][t]]);
                    }
                    for k in [false, true] {
                        sat.add(&[!hd, !ty[s][y][piston(k, d, false)], ty[s + 1][x][head(k, d)]]);
                    }
                    sat.add(&[!hd, ty[s][y][piston(false, d, false)], ty[s][y][piston(true, d, false)]]);
                }
                None => {
                    sat.add(&[!from]);
                    sat.add(&[!hd]);
                }
            }
        }
        sat.add(&[!src[x][TO_AIR], ty[s + 1][x][AIR]]);
        let mut ext_any = vec![!src[x][EXT_SELF]];
        let mut ret_any = vec![!src[x][RET_SELF]];
        for k in [false, true] {
            for d in 0..4 {
                sat.add(&[!src[x][EXT_SELF], !ty[s][x][piston(k, d, false)], ty[s + 1][x][piston(k, d, true)]]);
                sat.add(&[!src[x][RET_SELF], !ty[s][x][piston(k, d, true)], ty[s + 1][x][piston(k, d, false)]]);
                ext_any.push(ty[s][x][piston(k, d, false)]);
                ret_any.push(ty[s][x][piston(k, d, true)]);
            }
        }
        sat.add(&ext_any);
        sat.add(&ret_any);
    }
    src
}

/// Bookkeeping of one step's encoding: per (cell, option) the literals that justify it, per cell
/// the actions writing it, per action the cells it reads.
#[derive(Default)]
struct StepBook {
    why: HashMap<(usize, usize), Vec<Lit>>,
    writers: HashMap<usize, Vec<Lit>>,
    reads: Vec<(Lit, Vec<usize>)>,
}

impl StepBook {
    fn just(&mut self, cell: usize, opt: usize, l: Lit) {
        self.why.entry((cell, opt)).or_default().push(l);
    }
    fn touch(&mut self, a: Lit, writes: &[usize], reads: &[usize]) {
        for &w in writes {
            self.writers.entry(w).or_default().push(a);
        }
        let rs: Vec<usize> = reads.iter().copied().filter(|r| !writes.contains(r)).collect();
        self.reads.push((a, rs));
    }
}

/// The pushes of a retracted piston at `c` facing `d` (every line length, each in the three
/// modes) at step `s`.
#[allow(clippy::too_many_arguments)]
fn push_acts(sat: &mut Sat, ty: &[Vec<Vec<Lit>>], mv: &[Vec<Lit>], src: &[Vec<Lit>], s: usize, c: usize, d: usize, line: &[usize], book: &mut StepBook, acts: &mut Vec<Act>) {
    for l in 0..line.len().min(13) {
        for kind in [Kind::Extend, Kind::Pulse, Kind::PulsePull] {
            // actions that change nothing are left out (dropping one from a plan changes no
            // state and only removes a feed requirement): a pulse into air, a pulse-pull of one
            // block (it comes straight back), a pulse-pull into air at the box edge
            if (kind == Kind::Pulse && l == 0) || (kind == Kind::PulsePull && (l == 1 || (l == 0 && line.len() < 2))) {
                continue;
            }
            let a = sat.var();
            if kind == Kind::PulsePull {
                sat.add(&[!a, ty[s][c][piston(true, d, false)]]);
            } else {
                sat.add(&[!a, ty[s][c][piston(false, d, false)], ty[s][c][piston(true, d, false)]]);
            }
            for k in 0..l {
                sat.add(&[!a, mv[s][line[k]]]);
            }
            sat.add(&[!a, ty[s][line[l]][AIR]]);
            let mut reads: Vec<usize> = std::iter::once(c).chain(line[..=l].iter().copied()).collect();
            let mut writes: Vec<usize> = line[..=l].to_vec();
            let mut pull = None;
            match kind {
                Kind::Extend => {
                    sat.add(&[!a, src[c][EXT_SELF]]);
                    sat.add(&[!a, src[line[0]][HEAD + d]]);
                    book.just(c, EXT_SELF, a);
                    book.just(line[0], HEAD + d, a);
                    for k in 1..=l {
                        sat.add(&[!a, src[line[k]][FROM + d]]);
                        book.just(line[k], FROM + d, a);
                    }
                    writes.push(c);
                }
                Kind::Pulse => {
                    if l > 0 {
                        sat.add(&[!a, src[line[0]][TO_AIR]]);
                        book.just(line[0], TO_AIR, a);
                    }
                    for k in 1..=l {
                        sat.add(&[!a, src[line[k]][FROM + d]]);
                        book.just(line[k], FROM + d, a);
                    }
                }
                Kind::PulsePull => {
                    if l == 0 && line.len() >= 2 {
                        // into air: the block 2 ahead comes in (it must be movable: otherwise
                        // the action changes nothing)
                        let q = sat.var();
                        sat.add(&[!q, a]);
                        sat.add(&[!q, mv[s][line[1]]]);
                        sat.add(&[q, !a, !mv[s][line[1]]]);
                        sat.add(&[!a, q]);
                        sat.add(&[!q, src[line[0]][FROM + opp(d)]]);
                        sat.add(&[!q, src[line[1]][TO_AIR]]);
                        book.just(line[0], FROM + opp(d), q);
                        book.just(line[1], TO_AIR, q);
                        reads.push(line[1]);
                        writes.push(line[1]);
                        pull = Some(q);
                    } else if l >= 2 {
                        // the first block comes back, the rest stays pushed
                        sat.add(&[!a, src[line[1]][TO_AIR]]);
                        book.just(line[1], TO_AIR, a);
                        for k in 2..=l {
                            sat.add(&[!a, src[line[k]][FROM + d]]);
                            book.just(line[k], FROM + d, a);
                        }
                    }
                }
                Kind::Retract => unreachable!(),
            }
            book.touch(a, &writes, &reads);
            acts.push(Act { kind, c, d, l, lit: a, pull, writes });
        }
    }
}

/// The retract of an extended piston at `c` facing `d` at step `s`.
#[allow(clippy::too_many_arguments)]
fn ret_act(sat: &mut Sat, ty: &[Vec<Vec<Lit>>], mv: &[Vec<Lit>], src: &[Vec<Lit>], s: usize, c: usize, d: usize, line: &[usize], book: &mut StepBook, acts: &mut Vec<Act>) {
    let r = sat.var();
    sat.add(&[!r, ty[s][c][piston(false, d, true)], ty[s][c][piston(true, d, true)]]);
    sat.add(&[!r, src[c][RET_SELF]]);
    book.just(c, RET_SELF, r);
    let mut cells = vec![c, line[0]];
    let pull = if line.len() >= 2 {
        let p = sat.var();
        sat.add(&[!p, r]);
        sat.add(&[!p, ty[s][c][piston(true, d, true)]]);
        sat.add(&[!p, mv[s][line[1]]]);
        sat.add(&[p, !r, !ty[s][c][piston(true, d, true)], !mv[s][line[1]]]);
        sat.add(&[!p, src[line[0]][FROM + opp(d)]]);
        sat.add(&[!p, src[line[1]][TO_AIR]]);
        sat.add(&[!r, p, src[line[0]][TO_AIR]]);
        book.just(line[0], FROM + opp(d), p);
        book.just(line[1], TO_AIR, p);
        book.just(line[0], TO_AIR, r);
        cells.push(line[1]);
        Some(p)
    } else {
        sat.add(&[!r, src[line[0]][TO_AIR]]);
        book.just(line[0], TO_AIR, r);
        None
    };
    book.touch(r, &cells, &cells);
    acts.push(Act { kind: Kind::Retract, c, d, l: 0, lit: r, pull, writes: cells });
}

/// The feed literals of step `s`: per (cell * 4 + facing), the literals that feed a piston there.
/// One way to feed a piston at one step: its selector, the cells it reads (no other action of the
/// step may write them), the cell to name in the report and the action kinds it can fire
/// (`K_*` bits).
struct FeedOpt {
    u: Lit,
    cells: Vec<usize>,
    at: usize,
    kinds: u8,
    /// The wiring cell it draws on (`--private`: a wiring cell feeds one piston cell only).
    wire: Option<usize>,
    /// The conductor it powers the piston through (`--strict` co-firing: it powers every piston
    /// next to it).
    cond: Option<usize>,
}

const K_ALL: u8 = 0b1111;
const K_EXTEND: u8 = 0b0001;
const K_RETRACT: u8 = 0b1000;

fn kind_bit(k: Kind) -> u8 {
    match k {
        Kind::Extend => 0b0001,
        Kind::Pulse => 0b0010,
        Kind::PulsePull => 0b0100,
        Kind::Retract => 0b1000,
    }
}

/// A selector that implies "the lever touches cell e".
fn lever_opt(sat: &mut Sat, lev_at: &[Option<Lit>], e: usize, opts: &mut Vec<FeedOpt>, lp: Lit, first: bool) {
    // the lever is a LEVEL for the whole edge (pockets3): it extends a piston it powers (lp: on in
    // this edge), and retracts it only by turning off, at the first step of an edge
    if let Some(at) = lev_at[e] {
        let u = sat.var();
        sat.add(&[!u, at]);
        sat.add(&[!u, lp]);
        opts.push(FeedOpt { u, cells: Vec::new(), at: e, kinds: K_EXTEND, wire: None, cond: None });
        if first {
            let u = sat.var();
            sat.add(&[!u, at]);
            sat.add(&[!u, !lp]);
            opts.push(FeedOpt { u, cells: Vec::new(), at: e, kinds: K_RETRACT, wire: None, cond: None });
        }
    }
}

/// The feed options of step `s`, per (cell * 4 + facing): a wiring cell (a controllable source)
/// in a feed cell; a conductor (stone, obsidian) in a feed cell with a wiring cell or the lever
/// next to it (a redstone block does not power blocks); the lever next to the piston or to the
/// cell above it (above the box: on the top face of the piston's top-row neighbor). A redstone
/// block is a CONSTANT source: it fires a piston only by arriving (an extend: it was not in that
/// feed cell one step earlier, or the piston just moved there) or by leaving (a retract); neither
/// at the first step of an edge, which starts from a settled state (`prev` = the state and the
/// sources one step earlier, inside the edge).
fn feed_step(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], lev_at: &[Option<Lit>], s: usize, prev: Option<&[Vec<Lit>]>, lp: Lit, stat: Option<&[Lit]>) -> HashMap<usize, Vec<FeedOpt>> {
    let n = g.n();
    let first = prev.is_none();
    let mut out = HashMap::new();
    for c in 0..n {
        for d in 0..4 {
            let mut opts: Vec<FeedOpt> = Vec::new();
            lever_opt(sat, lev_at, c, &mut opts, lp, first);
            match g.step(c, 2) {
                Some(a) => lever_opt(sat, lev_at, a, &mut opts, lp, first),
                None => {
                    for u in [0, 1] {
                        if let Some(e) = g.step(c, u) {
                            lever_opt(sat, lev_at, e, &mut opts, lp, first);
                        }
                    }
                }
            }
            for q in g.feed_cells(c, d) {
                let u = sat.var();
                sat.add(&[!u, ty[s][q][WIRE]]);
                opts.push(FeedOpt { u, cells: vec![q], at: q, kinds: K_ALL, wire: Some(q), cond: None });
                for r in (0..4).filter_map(|e| g.step(q, e)).filter(|&r| r != c) {
                    // what can strongly power a block in this vertical plane (clk-misc, w86): a
                    // horizontal repeater / comparator / observer pointing into it, its input or
                    // watched cell inside the box, or dust pointing into it fed from above or below
                    // (clk-misc); a torch in the cell below it; dust on top of it
                    let (qx, qy) = g.xy(q);
                    let (rx, ry) = g.xy(r);
                    let ok = if ry == qy { g.idx(2 * rx - qx, ry).is_some() || g.idx(rx, ry + 1).is_some() || g.idx(rx, ry - 1).is_some() } else { true };
                    if !ok {
                        continue;
                    }
                    let u = sat.var();
                    sat.add(&[!u, ty[s][q][STONE], ty[s][q][OBS]]);
                    sat.add(&[!u, ty[s][r][WIRE]]);
                    if let Some(st) = stat {
                        // --static-cond: a conductor that never moves (the same block all cycle)
                        sat.add(&[!u, st[q]]);
                    }
                    opts.push(FeedOpt { u, cells: vec![q, r], at: q, kinds: K_ALL, wire: Some(r), cond: Some(q) });
                }
                if let Some(at) = lev_at[q] {
                    // a conductor the lever is on: a level too
                    let u = sat.var();
                    sat.add(&[!u, ty[s][q][STONE], ty[s][q][OBS]]);
                    sat.add(&[!u, at]);
                    sat.add(&[!u, lp]);
                    opts.push(FeedOpt { u, cells: vec![q], at: q, kinds: K_EXTEND, wire: None, cond: Some(q) });
                    if first {
                        let u = sat.var();
                        sat.add(&[!u, ty[s][q][STONE], ty[s][q][OBS]]);
                        sat.add(&[!u, at]);
                        sat.add(&[!u, !lp]);
                        opts.push(FeedOpt { u, cells: vec![q], at: q, kinds: K_RETRACT, wire: None, cond: Some(q) });
                    }
                }
                if let Some(psrc) = prev {
                    // a redstone block arriving in the feed cell (or the piston arriving next to it)
                    let u = sat.var();
                    sat.add(&[!u, ty[s][q][RBLOCK]]);
                    let mut cl = vec![!u, !ty[s - 1][q][RBLOCK]];
                    cl.extend((0..4).map(|e| psrc[c][FROM + e]));
                    sat.add(&cl);
                    opts.push(FeedOpt { u, cells: vec![q], at: q, kinds: K_EXTEND, wire: None, cond: None });
                    // a redstone block leaving it
                    let u = sat.var();
                    sat.add(&[!u, ty[s - 1][q][RBLOCK]]);
                    sat.add(&[!u, !ty[s][q][RBLOCK]]);
                    opts.push(FeedOpt { u, cells: vec![q], at: q, kinds: K_RETRACT, wire: None, cond: None });
                }
            }
            out.insert(c * 4 + d, opts);
        }
    }
    out
}

/// `--strict` CO-FIRING (w86): a conductor powered to fire one piston powers every piston next to
/// it on a non-front side and QC-powers every piston whose above cell touches it (and the piston
/// right below it either way): a retracted one must act in the same step too (one that cannot
/// push is left out: a filter, not a proof rule).
fn cofire(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], s: usize, acts: &[Act], feed: &HashMap<usize, Vec<FeedOpt>>) {
    let n = g.n();
    let mut pq: Vec<Option<Lit>> = vec![None; n];
    // a wiring cell's signal into a cell (the piston's own for a direct feed, its above cell for a
    // QC feed): keyed by (that cell, the wiring cell)
    let mut px: HashMap<(usize, usize), Lit> = HashMap::new();
    for a in acts {
        // a rising signal (extend, pulse) fires and holds its neighbours; a falling one (a
        // retract's feed turning off) fires nothing
        if a.kind == Kind::Retract {
            continue;
        }
        let bit = kind_bit(a.kind);
        for f in feed[&(a.c * 4 + a.d)].iter().filter(|f| f.kinds & bit != 0) {
            if let Some(q) = f.cond {
                let v = *pq[q].get_or_insert_with(|| sat.var());
                sat.add(&[!a.lit, !f.u, v]);
            } else if let Some(w) = f.wire {
                let direct = (0..4).any(|d| g.step(a.c, d) == Some(w));
                let x = if direct { Some(a.c) } else { g.step(a.c, 2) };
                if let Some(x) = x {
                    let v = *px.entry((x, w)).or_insert_with(|| sat.var());
                    sat.add(&[!a.lit, !f.u, v]);
                }
            }
        }
    }
    let must = |sat: &mut Sat, v: Lit, x: usize, d: usize| {
        let mine: Vec<Lit> = acts.iter().filter(|b| b.c == x && b.d == d && b.kind != Kind::Retract).map(|b| b.lit).collect();
        let retracts: Vec<Lit> = acts.iter().filter(|b| b.c == x && b.d == d && b.kind == Kind::Retract).map(|b| b.lit).collect();
        for k in [false, true] {
            let mut cl = vec![!v, !ty[s][x][piston(k, d, false)]];
            cl.extend(mine.iter().copied());
            sat.add(&cl);
            // an extended piston the signal reaches is held out: it cannot retract in this step
            // (one held by QC only does not even get an update when the signal ends: the 7x7
            // v8 (-2,-2) export's (4,0) under the lever-pulsed conductor (4,2))
            for &r in &retracts {
                sat.add(&[!v, !ty[s][x][piston(k, d, true)], !r]);
            }
        }
    };
    // the signal into x powers a piston at x (unless x faces the source) and, by QC, the piston
    // right below x (stacked pistons fire together: m86-a's A, w1's (-2,2) over (-2,1))
    for (&(x, w), &v) in &px {
        for d in 0..4 {
            if g.step(x, d) != Some(w) {
                must(sat, v, x, d);
            }
        }
        if let Some(b) = g.step(x, 3) {
            for d in 0..4 {
                must(sat, v, b, d);
            }
        }
    }
    for q in 0..n {
        let Some(v) = pq[q] else { continue };
        // (cell, facing) of every piston the powered conductor q reaches
        let mut hit: Vec<(usize, usize)> = Vec::new();
        for x in (0..4).filter_map(|d| g.step(q, d)) {
            for d in 0..4 {
                if g.step(x, d) != Some(q) || g.step(x, 2) == Some(q) {
                    hit.push((x, d));
                }
            }
        }
        for x in 0..n {
            if let Some(a) = g.step(x, 2)
                && a != q
                && (0..4).any(|d| g.step(a, d) == Some(q))
            {
                for d in 0..4 {
                    if !hit.contains(&(x, d)) {
                        hit.push((x, d));
                    }
                }
            }
        }
        for (x, d) in hit {
            must(sat, v, x, d);
        }
    }
}

/// Filters shared by the steps: per cell "a conductor that never moves" (`--static-cond`).
#[derive(Default)]
struct Filters {
    stat: Option<Vec<Lit>>,
}

/// At most `k` of `xs`, Sinz's sequential counter (n·k auxiliaries; any k).
fn at_most_k_seq(sat: &mut Sat, xs: &[Lit], k: usize) {
    let n = xs.len();
    if n <= k {
        return;
    }
    if k == 0 {
        for &x in xs {
            sat.add(&[!x]);
        }
        return;
    }
    // r[i][j]: at least j+1 of xs[0..=i] are true
    let r: Vec<Vec<Lit>> = (0..n).map(|_| (0..k).map(|_| sat.var()).collect()).collect();
    sat.add(&[!xs[0], r[0][0]]);
    for j in 1..k {
        sat.add(&[!r[0][j]]);
    }
    for i in 1..n {
        sat.add(&[!xs[i], r[i][0]]);
        sat.add(&[!r[i - 1][0], r[i][0]]);
        for j in 1..k {
            sat.add(&[!xs[i], !r[i - 1][j - 1], r[i][j]]);
            sat.add(&[!r[i - 1][j], r[i][j]]);
        }
        sat.add(&[!xs[i], !r[i - 1][k - 1]]);
    }
}

/// At most `k` of `xs` (every k+1-subset has a false one; for the small counts used here).
fn at_most_k(sat: &mut Sat, xs: &[Lit], k: usize) {
    fn rec(sat: &mut Sat, xs: &[Lit], k: usize, start: usize, pick: &mut Vec<Lit>) {
        if pick.len() == k + 1 {
            let cl: Vec<Lit> = pick.iter().map(|l| !*l).collect();
            sat.add(&cl);
            return;
        }
        for i in start..xs.len() {
            pick.push(xs[i]);
            rec(sat, xs, k, i + 1, pick);
            pick.pop();
        }
    }
    if xs.len() > k {
        rec(sat, xs, k, 0, &mut Vec::new());
    }
}

/// One step: sources, actions, justification, independence, feed.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn step(sat: &mut Sat, g: Grid, o: &Opts, ty: &[Vec<Vec<Lit>>], mv: &[Vec<Lit>], lev_at: &[Option<Lit>], exempt: Option<&[Lit]>, s: usize, prev_src: Option<&[Vec<Lit>]>, private: Option<&[Vec<Lit>]>, started: Option<&[Lit]>, lp: Lit, filt: &Filters, uses: Option<&[Lit]>) -> (Vec<Act>, HashMap<usize, Vec<FeedOpt>>, Vec<Vec<Lit>>) {
    let n = g.n();
    let src = sources(sat, g, ty, s);
    let mut book = StepBook::default();
    let mut acts = Vec::new();
    for c in 0..n {
        for d in 0..4 {
            let line = g.line(c, d);
            if line.is_empty() {
                continue;
            }
            push_acts(sat, ty, mv, &src, s, c, d, &line, &mut book, &mut acts);
            ret_act(sat, ty, mv, &src, s, c, d, &line, &mut book, &mut acts);
        }
    }
    // a cell changes only when an action says so
    for x in 0..n {
        for opt in 1..NSRC {
            let mut cl = vec![!src[x][opt]];
            if let Some(v) = book.why.get(&(x, opt)) {
                cl.extend(v.iter().copied());
            }
            sat.add(&cl);
        }
    }
    // the lever's block never moves
    for x in 0..n {
        if let Some(at) = lev_at[x] {
            for opt in [FROM, FROM + 1, FROM + 2, FROM + 3, TO_AIR, HEAD, HEAD + 1, HEAD + 2, HEAD + 3] {
                sat.add(&[!at, !src[x][opt]]);
            }
        }
    }
    // the feed, read in the state the step starts from: an action fires only through one of its
    // options, whose cells no other action of the step writes
    let wr = independence(sat, o, &acts, &book, n);
    let feed = if o.feed { feed_step(sat, g, ty, lev_at, s, prev_src, lp, filt.stat.as_deref()) } else { HashMap::new() };
    if o.feed {
        for a in &acts {
            let opts = &feed[&(a.c * 4 + a.d)];
            let bit = kind_bit(a.kind);
            let mut cl = vec![!a.lit];
            cl.extend(opts.iter().filter(|f| f.kinds & bit != 0).map(|f| f.u));
            if let Some(ex) = exempt {
                cl.push(ex[a.c]);
            }
            sat.add(&cl);
            for f in opts.iter().filter(|f| f.kinds & bit != 0) {
                // --private: the wiring cell is this piston cell's alone
                if let (Some(uw), Some(w)) = (private, f.wire) {
                    sat.add(&[!a.lit, !f.u, uw[w][a.c]]);
                }
                // the wiring cell's region must have started (the lever, or a change it saw)
                if let (Some(st), Some(w)) = (started, f.wire) {
                    sat.add(&[!a.lit, !f.u, st[w]]);
                }
                for &q in &f.cells {
                    if a.writes.contains(&q) {
                        continue;
                    }
                    if let Some(w) = wr[q] {
                        sat.add(&[!a.lit, !f.u, !w]);
                    }
                }
            }
            // a redstone block in a feed cell keeps the piston powered: no retract
            if a.kind == Kind::Retract {
                for q in g.feed_cells(a.c, a.d) {
                    sat.add(&[!a.lit, !ty[s][q][RBLOCK]]);
                }
            }
            // the lever powers a piston it touches (or a conductor it is on next to it) for the
            // whole edge it is on in: such a piston cannot retract or pulse in that edge; --strict
            // (w86): its only action is the toggle at an edge's first step
            let first = prev_src.is_none();
            let strict_lever = o.strict && (!first || matches!(a.kind, Kind::Pulse | Kind::PulsePull));
            if a.kind != Kind::Extend || strict_lever {
                let mut touch: Vec<usize> = vec![a.c];
                match g.step(a.c, 2) {
                    Some(up) => touch.push(up),
                    None => touch.extend([0, 1].iter().filter_map(|&u| g.step(a.c, u))),
                }
                for e in touch {
                    if let Some(at) = lev_at[e] {
                        if strict_lever {
                            sat.add(&[!a.lit, !at]);
                        } else {
                            sat.add(&[!a.lit, !lp, !at]);
                        }
                    }
                }
                for q in g.feed_cells(a.c, a.d) {
                    if let Some(at) = lev_at[q] {
                        if strict_lever {
                            sat.add(&[!a.lit, !at, !ty[s][q][STONE]]);
                            sat.add(&[!a.lit, !at, !ty[s][q][OBS]]);
                        } else {
                            sat.add(&[!a.lit, !lp, !at, !ty[s][q][STONE]]);
                            sat.add(&[!a.lit, !lp, !at, !ty[s][q][OBS]]);
                        }
                    }
                }
            }
        }
        if o.physics && !o.strict {
            cofire(sat, g, ty, s, &acts, &feed);
        }
        if o.strict {
            cofire(sat, g, ty, s, &acts, &feed);
            // a lever-touched piston never moves (w86): a piston standing in a cell the lever
            // powers directly or by quasi-connectivity keeps its cell
            for e in 0..n {
                let Some(at) = lev_at[e] else { continue };
                let mut cells: Vec<usize> = vec![e];
                cells.extend(g.step(e, 3));
                if g.step(e, 2).is_none() {
                    cells.extend([0, 1].iter().filter_map(|&u| g.step(e, u)));
                }
                for x in cells {
                    let pa = sat.var();
                    let mut cl = vec![!pa];
                    for k in [false, true] {
                        for d in 0..4 {
                            for ext in [false, true] {
                                cl.push(ty[s][x][piston(k, d, ext)]);
                                sat.add(&[!ty[s][x][piston(k, d, ext)], pa]);
                            }
                        }
                    }
                    sat.add(&cl);
                    for opt in [FROM, FROM + 1, FROM + 2, FROM + 3, TO_AIR, HEAD, HEAD + 1, HEAD + 2, HEAD + 3] {
                        sat.add(&[!at, !pa, !src[x][opt]]);
                    }
                }
            }
        }
        if let Some(uw) = uses {
            for a in &acts {
                for f in feed[&(a.c * 4 + a.d)].iter().filter(|f| f.kinds & kind_bit(a.kind) != 0) {
                    if let Some(w) = f.wire {
                        sat.add(&[!a.lit, !f.u, uw[w]]);
                    }
                }
            }
        }
    }
    (acts, feed, src)
}

/// The actions of one step are independent: at most one writes a cell, nobody reads a cell
/// another one writes (`--seq`: at most one action at all). Returns, per cell, "an action of the
/// step writes it".
fn independence(sat: &mut Sat, o: &Opts, acts: &[Act], book: &StepBook, n: usize) -> Vec<Option<Lit>> {
    let mut wr: Vec<Option<Lit>> = vec![None; n];
    if o.seq {
        let all: Vec<Lit> = acts.iter().map(|a| a.lit).collect();
        sat.at_most_one(&all);
        return wr;
    }
    for x in 0..n {
        if let Some(ws) = book.writers.get(&x) {
            sat.at_most_one(ws);
            let v = sat.var();
            let mut cl = vec![!v];
            cl.extend(ws.iter().copied());
            sat.add(&cl);
            for &w in ws {
                sat.add(&[!w, v]);
            }
            wr[x] = Some(v);
        }
    }
    for (a, rs) in &book.reads {
        let mut seen = Vec::new();
        for &r in rs {
            if seen.contains(&r) {
                continue;
            }
            seen.push(r);
            if let Some(v) = wr[r] {
                sat.add(&[!*a, !v]);
            }
        }
    }
    wr
}

/// Mirror symmetry breaking for a box symmetric about x = 1 (the door's middle column): every
/// solution's mirror image (east and west swapped) is one too, so keep the rest layouts that are
/// lexicographically <= their mirror image (cells of the left half paired with their mirror cell,
/// row by row; types in index order, the mirror cell's type with east and west swapped).
fn mirror_break(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>]) {
    if g.x0 + (g.x0 + g.w - 1) != 2 {
        return;
    }
    let swap = |t: usize| -> usize {
        match t {
            5..=20 => {
                let i = t - 5;
                let (sticky, d, ext) = (i >= 8, (i % 8) / 2, i % 2 == 1);
                piston(sticky, if d < 2 { d ^ 1 } else { d }, ext)
            }
            21..=28 => {
                let i = t - 21;
                let (sticky, d) = (i >= 4, i % 4);
                head(sticky, if d < 2 { d ^ 1 } else { d })
            }
            _ => t,
        }
    };
    let mut eq = sat.tru();
    for y in g.y0..g.y0 + g.h {
        for x in g.x0..1 {
            let (c, m) = (g.idx(x, y).unwrap(), g.idx(2 - x, y).unwrap());
            // while equal so far: type(c) <= swap(type(m))
            for v in 0..NT {
                let mut cl = vec![!eq, !ty[0][c][v]];
                cl.extend((0..NT).filter(|&w| swap(w) >= v).map(|w| ty[0][m][w]));
                sat.add(&cl);
            }
            let next = sat.var();
            for v in 0..NT {
                // equal here (type(c) = swap(type(m))) and equal so far -> equal so far next
                let w = (0..NT).find(|&w| swap(w) == v).unwrap();
                sat.add(&[!eq, !ty[0][c][v], !ty[0][m][w], next]);
            }
            eq = next;
        }
    }
}

fn build(g: Grid, o: &Opts) -> Model {
    let mut sat = Sat::new(FlagMode::Forbid);
    let n = g.n();
    let steps = 2 * o.h;
    let ty = states(&mut sat, g, o, steps);
    let mv = movable(&mut sat, &ty);
    let (lv, lev_at) = levers(&mut sat, g, &ty, o);
    // --explain: one exemption literal per cell (a piston firing there needs no feed), assumed
    // false; an UNSAT core names the cells whose pistons cannot be fed
    let exempt: Option<Vec<Lit>> = if o.explain { Some((0..n).map(|_| sat.var()).collect()) } else { None };
    let mut acts = Vec::with_capacity(steps);
    let mut feed = Vec::with_capacity(steps);
    let mut any: Vec<Lit> = Vec::new();
    let mut prev_src: Option<Vec<Vec<Lit>>> = None;
    // --private: per (wiring cell, piston cell) "this wiring cell feeds that piston cell", at most
    // one piston cell per wiring cell (a candidate filter: a component feeding several pistons at
    // different times is what wiring usually cannot do; not a proof rule)
    let private: Option<Vec<Vec<Lit>>> = if o.private {
        let uw: Vec<Vec<Lit>> = (0..n).map(|_| (0..n).map(|_| sat.var()).collect()).collect();
        for row in &uw {
            sat.at_most_one(row);
        }
        Some(uw)
    } else {
        None
    };
    let mut cum: Vec<Lit> = vec![!sat.tru(); n];
    // the lever's polarity: on in the open edge (and off in the close edge) or the reverse
    let lon = sat.var();
    // --static-cond: per cell "a conductor that keeps its block all cycle"
    let mut filt = Filters::default();
    if o.static_cond {
        let stat: Vec<Lit> = (0..n)
            .map(|q| {
                let v = sat.var();
                sat.add(&[!v, ty[0][q][STONE], ty[0][q][OBS]]);
                for t in [STONE, OBS] {
                    for st in ty.iter() {
                        sat.add(&[!v, !ty[0][q][t], st[q][t]]);
                    }
                }
                v
            })
            .collect();
        filt.stat = Some(stat);
    }
    // --budget K: per step and wiring cell "it feeds a firing in this step"; at most K steps per
    // edge (a cell's component cannot make many distinct pulses: pockets3)
    let uses: Option<Vec<Vec<Lit>>> = o.budget.map(|_| (0..steps).map(|_| (0..n).map(|_| sat.var()).collect()).collect());
    for s in 0..steps {
        // the sources one step earlier inside the same edge (each edge starts settled)
        let prev = if s % o.h != 0 { prev_src.as_deref() } else { None };
        // what each cell's neighbors changed so far in this edge (an observer there could see it)
        cum = match prev {
            None => vec![!sat.tru(); n],
            Some(psrc) => seen_changes(&mut sat, g, psrc, &cum),
        };
        let started = if o.feed && o.start { Some(region_start(&mut sat, g, &ty, &lev_at, &cum, s)) } else { None };
        // the lever's power in this edge: on in the open edge (lon) or in the close edge
        let lp = if s < o.h { lon } else { !lon };
        let (a, f, src) = step(&mut sat, g, o, &ty, &mv, &lev_at, exempt.as_deref(), s, prev, private.as_deref(), started.as_deref(), lp, &filt, uses.as_ref().map(|u| u[s].as_slice()));
        prev_src = Some(src);
        // an idle step is followed by idle steps only (within an edge)
        let v = sat.var();
        let mut cl = vec![!v];
        cl.extend(a.iter().map(|x| x.lit));
        sat.add(&cl);
        for x in &a {
            sat.add(&[!x.lit, v]);
        }
        if s % o.h != 0 {
            sat.add(&[any[s - 1], !v]);
        }
        any.push(v);
        acts.push(a);
        feed.push(f);
    }
    if let (Some(k), Some(u)) = (o.budget, &uses) {
        for edge in 0..2 {
            for w in 0..n {
                let xs: Vec<Lit> = (edge * o.h..(edge + 1) * o.h).map(|s| u[s][w]).collect();
                at_most_k(&mut sat, &xs, k);
            }
        }
    }
    if o.physics || o.strict {
        // redstone-block stability: a retracted piston next to a redstone block (any side but its
        // front) is powered and extends at once: it extends in the next step, and never sits so at
        // rest (the 7x7 v8 (-2,-2) export's (1,-2), powered again when (4,-2) pushes the redstone
        // block back next to it); by QC (the block next to its above cell), at rest it fires at
        // the edge's first step
        for s in 0..2 * o.h {
            let rest = s % o.h == 0;
            for c in 0..n {
                for d in 0..4 {
                    let fire: Vec<Lit> = acts[s].iter().filter(|a| a.c == c && a.d == d && a.kind != Kind::Retract).map(|a| a.lit).collect();
                    for q in (0..4).filter(|&e| e != d).filter_map(|e| g.step(c, e)) {
                        for k in [false, true] {
                            let mut cl = vec![!ty[s][c][piston(k, d, false)], !ty[s][q][RBLOCK]];
                            if !rest {
                                cl.extend(fire.iter().copied());
                            }
                            sat.add(&cl);
                        }
                    }
                    // QC (w86: armed, it fires on the first block update it gets, maybe steps
                    // later; forbidding a lingering arm keeps replays faithful): a retracted piston
                    // whose above cell has a redstone block next to it fires in that very step
                    if let Some(a) = g.step(c, 2) {
                        for q in (0..4).filter_map(|e| g.step(a, e)).filter(|&q| q != c) {
                            for k in [false, true] {
                                let mut cl = vec![!ty[s][c][piston(k, d, false)], !ty[s][q][RBLOCK]];
                                cl.extend(fire.iter().copied());
                                sat.add(&cl);
                            }
                        }
                    }
                }
            }
        }
        // QC stability (c70b, at rest; every settled state since w86's arming note): a piston
        // extended is powered in its own cell, which QC-arms a retracted piston right below it:
        // that one must fire in the next step (at rest: the edge's first step)
        for rest in 0..2 * o.h {
            for c in 0..n {
                let Some(b) = g.step(c, 3) else { continue };
                for d2 in 0..4 {
                    let mine: Vec<Lit> = acts[rest].iter().filter(|a| a.c == b && a.d == d2).map(|a| a.lit).collect();
                    for k1 in [false, true] {
                        for d1 in 0..4 {
                            for k2 in [false, true] {
                                let mut cl = vec![!ty[rest][c][piston(k1, d1, true)], !ty[rest][b][piston(k2, d2, false)]];
                                cl.extend(mine.iter().copied());
                                sat.add(&cl);
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(k) = o.max_pistons {
        // --max-pistons N (c70b: lean seeds for mechmut): at most N pistons at rest
        let ps: Vec<Lit> = (0..n)
            .map(|c| {
                let v = sat.var();
                for st in [false, true] {
                    for d in 0..4 {
                        for ext in [false, true] {
                            sat.add(&[!ty[0][c][piston(st, d, ext)], v]);
                        }
                    }
                }
                v
            })
            .collect();
        at_most_k_seq(&mut sat, &ps, k);
    }
    if o.frame3 {
        // --frame3 (w86's strict reading): exactly 3 of the 12 frame cells hold stone at rest (the
        // extras, in place at both rest states)
        let frame: Vec<Lit> = [(0, -1), (1, -1), (2, -1), (0, 3), (1, 3), (2, 3), (-1, 0), (-1, 1), (-1, 2), (3, 0), (3, 1), (3, 2)].iter().filter_map(|&(x, y)| g.idx(x, y)).map(|c| ty[0][c][STONE]).collect();
        at_most_k(&mut sat, &frame, 3);
        let nots: Vec<Lit> = frame.iter().map(|l| !*l).collect();
        at_most_k(&mut sat, &nots, frame.len() - 3);
    }
    door(&mut sat, g, &ty, o);
    if o.fix.is_none() && o.sym {
        mirror_break(&mut sat, g, &ty);
    }
    if let Some(fix) = &o.fix {
        let mut fixed = vec![None; n];
        for &(x, y, t) in fix {
            if let Some(c) = g.idx(x, y) {
                fixed[c] = Some(t);
            }
        }
        for c in 0..n {
            match fixed[c] {
                Some(t) => sat.add(&[ty[0][c][t]]),
                None => sat.add(&[ty[0][c][AIR], ty[0][c][WIRE]]),
            }
        }
    }
    Model { sat, g, ty, acts, lv, feed, exempt }
}

/// Per cell: "a neighbor of this cell changed in this edge so far" (any source option other than
/// STAY moves or toggles the neighbor: it is a moving block or a new state for 2 gt, which an
/// observer sees), from the previous value and the sources of the step just done.
fn seen_changes(sat: &mut Sat, g: Grid, psrc: &[Vec<Lit>], cum: &[Lit]) -> Vec<Lit> {
    (0..g.n())
        .map(|w| {
            let v = sat.var();
            let mut cl = vec![!v, cum[w]];
            for c in (0..4).filter_map(|d| g.step(w, d)) {
                cl.extend((1..NSRC).map(|opt| psrc[c][opt]));
            }
            sat.add(&cl);
            v
        })
        .collect()
}

/// ISLAND START (clk-misc + pockets3, d67's `door feeds` cannot_start): wiring cells form
/// electrically isolated regions (4-connected, joined through one conductor block between two of
/// them: a component powers it, a component on its other side reads it); a region can only act
/// once it has started: one of its cells is next to the lever's block, or next to a cell that
/// changed earlier in this edge (an observer's delay: a change in the same step is too late).
/// Returns per cell "a started wiring cell" (propagated by layers, one per cell of the box).
/// Not encoded: "a region that sees the same changes on both edges acts the same on both".
fn region_start(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], lev_at: &[Option<Lit>], cum: &[Lit], s: usize) -> Vec<Lit> {
    let n = g.n();
    let mut st: Vec<Lit> = (0..n)
        .map(|w| {
            let v = sat.var();
            sat.add(&[!v, ty[0][w][WIRE]]);
            let mut cl = vec![!v, cum[w]];
            // next to the lever's block when it is a conductor (the lever strongly powers it; a
            // piston or a head passes nothing on)
            for e in (0..4).filter_map(|d| g.step(w, d)) {
                if let Some(at) = lev_at[e] {
                    let le = sat.var();
                    sat.add(&[!le, at]);
                    sat.add(&[!le, ty[s][e][STONE], ty[s][e][OBS]]);
                    cl.push(le);
                }
            }
            sat.add(&cl);
            v
        })
        .collect();
    for _ in 0..n {
        // a conductor between two wiring cells passes power on (a component strongly powers it,
        // a component on its other side reads it): one block, never block to block
        let bridge: Vec<Lit> = (0..n)
            .map(|q| {
                let v = sat.var();
                sat.add(&[!v, ty[s][q][STONE], ty[s][q][OBS]]);
                let mut cl = vec![!v];
                cl.extend((0..4).filter_map(|d| g.step(q, d)).map(|w| st[w]));
                sat.add(&cl);
                v
            })
            .collect();
        let next: Vec<Lit> = (0..n)
            .map(|w| {
                let v = sat.var();
                sat.add(&[!v, ty[0][w][WIRE]]);
                let mut cl = vec![!v, st[w]];
                for q in (0..4).filter_map(|d| g.step(w, d)) {
                    cl.push(st[q]);
                    cl.push(bridge[q]);
                }
                sat.add(&cl);
                v
            })
            .collect();
        st = next;
    }
    st
}

/// The door: square stone at rest, air when open with the 12 frame cells stone, back to rest.
fn door(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], o: &Opts) {
    let steps = 2 * o.h;
    let square: Vec<usize> = (0..3).flat_map(|y| (0..3).map(move |x| (x, y))).filter_map(|(x, y)| g.idx(x, y)).collect();
    let frame: Vec<usize> = [(0, -1), (1, -1), (2, -1), (0, 3), (1, 3), (2, 3), (-1, 0), (-1, 1), (-1, 2), (3, 0), (3, 1), (3, 2)].iter().filter_map(|&(x, y)| g.idx(x, y)).collect();
    for &c in &square {
        sat.add(&[ty[0][c][STONE]]);
        sat.add(&[ty[o.h][c][AIR]]);
    }
    for &c in &frame {
        sat.add(&[ty[o.h][c][STONE]]);
    }
    for c in 0..g.n() {
        for t in 0..NT {
            sat.add(&[!ty[steps][c][t], ty[0][c][t]]);
            sat.add(&[ty[steps][c][t], !ty[0][c][t]]);
        }
    }
}

fn type_char(t: usize) -> String {
    match t {
        AIR => ".".into(),
        STONE => "#".into(),
        OBS => "O".into(),
        RBLOCK => "R".into(),
        WIRE => "%".into(),
        5..=20 => {
            let i = t - 5;
            let sticky = i >= 8;
            let d = (i % 8) / 2;
            let ext = i % 2 == 1;
            let c = ["e", "w", "u", "d"][d];
            let c = if sticky { c.to_uppercase() } else { c.to_string() };
            if ext { format!("{c}*") } else { c }
        }
        _ => {
            let i = t - 21;
            let sticky = i >= 4;
            let d = i % 4;
            if sticky { ["}", "{", "^", "v"][d].into() } else { [">", "<", "'", ","][d].into() }
        }
    }
}

fn state_lines(m: &Model, s: usize) -> Vec<String> {
    let g = m.g;
    let mut out = Vec::new();
    for y in (g.y0..g.y0 + g.h).rev() {
        let mut row = format!("{y:>3} ");
        for x in g.x0..g.x0 + g.w {
            let c = g.idx(x, y).unwrap();
            let t = (0..NT).find(|&t| m.sat.value(m.ty[s][c][t])).unwrap_or(AIR);
            row.push_str(&format!("{:<3}", type_char(t)));
        }
        out.push(row);
    }
    out
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Extend => "EXTEND",
        Kind::Pulse => "PULSE",
        Kind::PulsePull => "PULSE-PULL",
        Kind::Retract => "RETRACT",
    }
}

fn report(m: &Model, o: &Opts) -> String {
    use std::fmt::Write as _;
    let g = m.g;
    let mut out = String::new();
    let _ = writeln!(out, "rest:");
    for l in state_lines(m, 0) {
        let _ = writeln!(out, "{l}");
    }
    // wiring cells (static all cycle; `%` in the grids, `w` before 2026-10-20 = also a normal
    // west piston: c70b's reader)
    let val = |s: usize, c: usize| (0..NT).find(|&t| m.sat.value(m.ty[s][c][t])).unwrap_or(AIR);
    let wires: Vec<(i32, i32)> = (0..g.n()).filter(|&c| val(0, c) == WIRE).map(|c| g.xy(c)).collect();
    let _ = writeln!(out, "wiring: {}", wires.iter().map(|(x, y)| format!("({x},{y})")).collect::<Vec<_>>().join(" "));
    for &(e, l) in &m.lv {
        if m.sat.value(l) {
            let _ = writeln!(out, "lever on the block at {:?} (its front/back face, or its outer face at the box edge)", g.xy(e));
        }
    }
    for s in 0..2 * o.h {
        for a in m.acts[s].iter().filter(|a| m.sat.value(a.lit)) {
            let pulled = a.pull.is_some_and(|p| m.sat.value(p));
            let what = match a.kind {
                Kind::Retract => format!("RETRACT {:?} {}{}", g.xy(a.c), DNAME[a.d], if pulled { " (pulls)" } else { "" }),
                k => format!("{} {:?} {} line {}{}", kind_name(k), g.xy(a.c), DNAME[a.d], a.l, if pulled { " (pulls)" } else { "" }),
            };
            let fed = m.feed.get(s).and_then(|f| f.get(&(a.c * 4 + a.d))).and_then(|v| v.iter().find(|f| m.sat.value(f.u)).map(|f| g.xy(f.at)));
            let _ = writeln!(out, "{} step {s:>2}: {what}{}", if s < o.h { "open " } else { "close" }, fed.map(|q| format!("  [fed from {q:?}]")).unwrap_or_default());
        }
    }
    let _ = writeln!(out, "open:");
    for l in state_lines(m, o.h) {
        let _ = writeln!(out, "{l}");
    }
    out
}

/// A design file's blocks in the plane z = door.z as model types (levers and blocks outside the
/// box ignored; any other component is a wiring cell).
fn read_fix(path: &str) -> Result<Vec<(i32, i32, usize)>> {
    let text = std::fs::read_to_string(path)?;
    let d = mctas::design::Design::parse(&text)?;
    let z0 = d.ports.get("door").map(|p| p[2]).unwrap_or(0);
    let mut out = Vec::new();
    for (p, s) in &d.blocks {
        if p[2] != z0 {
            // an exported model's rest power (a redstone block behind a held piston) is an ideal
            // input, not part of the layout
            if s.trim_start_matches("minecraft:") == "redstone_block" {
                continue;
            }
            bail!("{path}: block at {p:?} is not in the door's plane (a flat door only)");
        }
        let name = s.split('[').next().unwrap_or("").trim_start_matches("minecraft:");
        let prop = |k: &str| -> Option<String> {
            let inner = s.split_once('[').map(|(_, r)| r.trim_end_matches(']')).unwrap_or("");
            inner.split(',').find_map(|kv| kv.split_once('=').filter(|(a, _)| a.trim() == k).map(|(_, b)| b.trim().to_string()))
        };
        let dir = |f: &str| -> Result<usize> {
            match f {
                "east" => Ok(0),
                "west" => Ok(1),
                "up" => Ok(2),
                "down" => Ok(3),
                other => Err(anyhow!("{path}: facing {other} at {p:?} is out of the plane")),
            }
        };
        let t = match name {
            "lever" => continue,
            "obsidian" | "crying_obsidian" => OBS,
            "redstone_block" => RBLOCK,
            "piston" | "sticky_piston" => {
                let f = prop("facing").unwrap_or_default();
                piston(name == "sticky_piston", dir(&f)?, prop("extended").as_deref() == Some("true"))
            }
            "piston_head" => {
                let f = prop("facing").unwrap_or_default();
                head(prop("type").as_deref() == Some("sticky"), dir(&f)?)
            }
            // movable full conductors that only pass power on
            "redstone_lamp" | "note_block" => STONE,
            // wiring: every other redstone component (dust, torches, repeaters, comparators,
            // observers, droppers, dispensers, targets, buttons, plates, sensors...)
            n if n.starts_with("redstone_")
                || ["repeater", "comparator", "observer", "dropper", "dispenser", "target", "button", "pressure_plate", "tripwire", "sculk", "daylight", "hopper", "crafter", "copper_bulb", "lightning_rod"].iter().any(|k| n.contains(k)) =>
            {
                WIRE
            }
            n if n.contains("stone") || n.ends_with("_planks") || n.ends_with("_wool") || n.ends_with("_concrete") || n.ends_with("terracotta") || n == "glass" || n.contains("deepslate") || n.ends_with("_bricks") => STONE,
            other => bail!("{path}: {other} at {p:?}: not in the macro palette (add it to read_fix)"),
        };
        out.push((p[0], p[1], t));
    }
    Ok(out)
}

/// A solved model as an ideal-input design + plan (`door ideal-rs` / `door ideal` replay them):
/// the rest layout in the plane (wiring cells left as air: the free cells for the wiring), a
/// redstone block at z = 1 behind every piston extended at rest (its rest power), and per action
/// an input at z = 1 behind the piston: EXTEND sets it, RETRACT clears it, PULSE is a 2 gt pulse
/// (a sticky piston spits), PULSE-PULL a 4 gt pulse; steps 10 gt apart (`let sK` per step, so a
/// replay can retime them with `--set`). Inputs at z = 1 also power
/// the cell above the piston below by quasi-connectivity: a replay can misfire where the model
/// assumed independent inputs.
fn export(m: &Model, o: &Opts, prefix: &str) -> (String, String) {
    use std::fmt::Write as _;
    let g = m.g;
    let val = |s: usize, c: usize| (0..NT).find(|&t| m.sat.value(m.ty[s][c][t])).unwrap_or(AIR);
    let state = |t: usize| -> String {
        match t {
            STONE => "minecraft:stone".into(),
            OBS => "minecraft:obsidian".into(),
            RBLOCK => "minecraft:redstone_block".into(),
            5..=20 => {
                let i = t - 5;
                let (sticky, d, ext) = (i >= 8, (i % 8) / 2, i % 2 == 1);
                format!("minecraft:{}[extended={ext},facing={}]", if sticky { "sticky_piston" } else { "piston" }, DNAME[d])
            }
            21..=28 => {
                let i = t - 21;
                let (sticky, d) = (i >= 4, i % 4);
                format!("minecraft:piston_head[facing={},short=false,type={}]", DNAME[d], if sticky { "sticky" } else { "normal" })
            }
            _ => "minecraft:air".into(),
        }
    };
    let mut pal: Vec<(String, char)> = Vec::new();
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".chars().collect();
    let char_of = |pal: &mut Vec<(String, char)>, st: String| -> char {
        match pal.iter().find(|p| p.0 == st) {
            Some(p) => p.1,
            None => {
                let ch = chars[pal.len()];
                pal.push((st, ch));
                ch
            }
        }
    };
    let held: Vec<usize> = (0..g.n()).filter(|&c| (5..=20).contains(&val(0, c)) && (val(0, c) - 5) % 2 == 1).collect();
    let rb = if held.is_empty() { ' ' } else { char_of(&mut pal, "minecraft:redstone_block".into()) };
    let mut layers = String::new();
    let mut wires = Vec::new();
    for y in g.y0..g.y0 + g.h {
        let (mut row, mut back) = (String::new(), String::new());
        for x in g.x0..g.x0 + g.w {
            let c = g.idx(x, y).unwrap();
            let t = val(0, c);
            if t == WIRE {
                wires.push((x, y));
            }
            back.push(if held.contains(&c) { rb } else { '.' });
            let st = state(t);
            if st == "minecraft:air" {
                row.push('.');
            } else {
                row.push(char_of(&mut pal, st));
            }
        }
        if held.is_empty() {
            let _ = writeln!(layers, "layer {y}:\n  {row}");
        } else {
            let _ = writeln!(layers, "layer {y}:\n  {row}\n  {back}");
        }
    }
    let mut d = String::new();
    let name = prefix.rsplit('/').next().unwrap_or(prefix);
    let _ = writeln!(d, "name: {name}\n# macrosat (h {} per edge{}): wiring cells (left as air) {wires:?}; pistons extended at rest hold a redstone block at z = 1", o.h, if o.feed { ", every firing fed" } else { "" });
    let _ = writeln!(d, "port door: 0 0 0\norigin: {} 0 0\npalette:", g.x0);
    for (st, ch) in &pal {
        let _ = writeln!(d, "  {ch} {st}");
    }
    d.push_str(&layers);
    let mut p = String::new();
    let _ = writeln!(p, "phase between\ndesign {prefix}.txt");
    // one variable per step of each edge (open sK / close cK), 10 gt apart: `--set` retimes them
    for k in 0..o.h {
        let _ = writeln!(p, "let s{k} = {}\nlet c{k} = {}", 1 + 10 * k, 1 + 10 * k);
    }
    // per extended piston, the input holding it (pistons extended at rest: the redstone block
    // behind them at z = 1)
    let mut hold: HashMap<usize, ((i32, i32, i32), bool)> = HashMap::new();
    for s in 0..2 * o.h {
        let edge = if s < o.h { "open" } else { "close" };
        let t = format!("{}{}", if s < o.h { "s" } else { "c" }, s % o.h);
        // each input once per step (pistons fired together share it)
        let mut placed: Vec<(i32, i32, i32)> = Vec::new();
        for a in m.acts[s].iter().filter(|a| m.sat.value(a.lit)) {
            let (x, y) = g.xy(a.c);
            let input = feed_input(m, o, s, a);
            let set = |p: &mut String, placed: &mut Vec<(i32, i32, i32)>, pos: (i32, i32, i32), st: &str, at: &str| {
                if !placed.contains(&pos) {
                    placed.push(pos);
                    let _ = writeln!(p, "{edge} {at} set {} {} {} {st}", pos.0, pos.1, pos.2);
                }
            };
            // a QC feed reaches the piston only with a block update (w86's rule 4: real dust gives
            // it, an ideal input behind another cell does not): glass placed at z = -1 next to the
            // piston right after the input changes, removed a tick later
            let bud = |p: &mut String, off: u32| {
                let at = if off == 0 { t.clone() } else { format!("{t}+{off}") };
                let _ = writeln!(p, "{edge} {at} set {x} {y} -1 minecraft:glass\n{edge} {t}+{} clear {x} {y} -1", off + 1);
            };
            match a.kind {
                Kind::Extend => {
                    if let Some((pos, st, qc)) = input {
                        set(&mut p, &mut placed, pos, &st, &t);
                        if qc {
                            bud(&mut p, 0);
                        }
                        hold.insert(a.c, (pos, qc));
                    }
                }
                Kind::Retract => {
                    let (pos, qc) = hold.remove(&a.c).unwrap_or(((x, y, 1), false));
                    if !placed.contains(&pos) {
                        placed.push(pos);
                        let _ = writeln!(p, "{edge} {t} clear {} {} {}", pos.0, pos.1, pos.2);
                    }
                    if qc {
                        bud(&mut p, 0);
                    }
                }
                Kind::Pulse | Kind::PulsePull => {
                    if let Some((pos, st, qc)) = input {
                        let len = if a.kind == Kind::Pulse { 2 } else { 4 };
                        if !placed.contains(&pos) {
                            set(&mut p, &mut placed, pos, &st, &t);
                            let _ = writeln!(p, "{edge} {t}+{len} clear {} {} {}", pos.0, pos.1, pos.2);
                        }
                        if qc {
                            bud(&mut p, 0);
                            bud(&mut p, len);
                        }
                    }
                }
            }
        }
    }
    (d, p)
}

/// Where the ideal input of a firing goes, from the feed the model chose (w86: inputs where the real
/// feed is, so nothing else gets powered). Every input sits out of the plane at z = 1 except the
/// lever on a top-row block: a redstone block behind the piston (a direct feed: power into its
/// cell, as any real source pointing into it gives) or behind its above cell (a QC feed); a powered
/// lever on the conductor's back face (a conductor feed: it strongly powers that block and nothing
/// else in the plane); for the lever, a redstone block on the face of the block it is on, or on
/// top of the top-row block it is on. None: a redstone block arriving or leaving fires it by
/// itself. The flag: the input does not touch the piston, so it needs a block update (QC).
fn feed_input(m: &Model, o: &Opts, s: usize, a: &Act) -> Option<((i32, i32, i32), String, bool)> {
    let g = m.g;
    let rb = "minecraft:redstone_block".to_string();
    let (x, y) = g.xy(a.c);
    if !o.feed {
        return Some(((x, y, 1), rb, false));
    }
    let f = m.feed[s].get(&(a.c * 4 + a.d))?.iter().find(|f| f.kinds & kind_bit(a.kind) != 0 && m.sat.value(f.u))?;
    let above = g.step(a.c, 2);
    let next_to = |q: usize| (0..4).any(|d| g.step(a.c, d) == Some(q));
    match (f.wire, f.cond) {
        (Some(w), None) => match above {
            Some(u) if !next_to(w) => {
                let (ux, uy) = g.xy(u);
                Some(((ux, uy, 1), rb, true))
            }
            _ => Some(((x, y, 1), rb, false)),
        },
        (_, Some(q)) => {
            let (qx, qy) = g.xy(q);
            Some(((qx, qy, 1), "minecraft:lever[face=wall,facing=south,powered=true]".into(), !next_to(q)))
        }
        (None, None) if f.cells.is_empty() => {
            let e = f.at;
            let (ex, ey) = g.xy(e);
            if e == a.c {
                Some(((ex, ey, 1), rb, false))
            } else if above == Some(e) {
                Some(((ex, ey, 1), rb, true))
            } else {
                Some(((ex, ey + 1, 0), rb, true))
            }
        }
        _ => None,
    }
}

/// `--plan FILE`: force a known action sequence, one action per step (a check of the encoding:
/// a plan the model's semantics accept must solve at once). Lines `open|close EXT|PULSE|PP|RET x
/// y facing`; the steps after each edge's lines are idle.
fn force_plan(m: &mut Model, path: &str, h: usize) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let mut next = [0usize, h];
    for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let w: Vec<&str> = line.split_whitespace().collect();
        if w.len() != 5 {
            bail!("{path}: `{line}`: want `open|close MODE x y facing`");
        }
        let edge = match w[0] {
            "open" => 0,
            "close" => 1,
            e => bail!("{path}: edge {e}"),
        };
        let s = next[edge];
        if s >= (edge + 1) * h {
            bail!("{path}: more than {h} actions on the {} edge", w[0]);
        }
        next[edge] += 1;
        let (x, y): (i32, i32) = (w[2].parse()?, w[3].parse()?);
        let d = DNAME.iter().position(|n| *n == w[4]).ok_or_else(|| anyhow!("{path}: facing {}", w[4]))?;
        let c = m.g.idx(x, y).ok_or_else(|| anyhow!("{path}: ({x},{y}) outside the box"))?;
        let kind = match w[1] {
            "EXT" => Kind::Extend,
            "PULSE" => Kind::Pulse,
            "PP" => Kind::PulsePull,
            "RET" => Kind::Retract,
            other => bail!("{path}: mode {other}"),
        };
        let mine: Vec<Lit> = m.acts[s].iter().filter(|a| a.c == c && a.d == d && a.kind == kind).map(|a| a.lit).collect();
        if mine.is_empty() {
            bail!("{path}: no {} at ({x},{y}) {}", w[1], w[4]);
        }
        let others: Vec<Lit> = m.acts[s].iter().filter(|a| !(a.c == c && a.d == d && a.kind == kind)).map(|a| a.lit).collect();
        m.sat.add(&mine);
        for l in others {
            m.sat.add(&[!l]);
        }
    }
    for edge in 0..2 {
        for s in next[edge]..(edge + 1) * h {
            let all: Vec<Lit> = m.acts[s].iter().map(|a| a.lit).collect();
            for l in all {
                m.sat.add(&[!l]);
            }
        }
    }
    Ok(())
}

fn grid_of(s: &str) -> Result<Grid> {
    let bx: Vec<i32> = s.split(',').map(|v| v.trim().parse::<i32>()).collect::<Result<_, _>>()?;
    if bx.len() != 4 {
        bail!("box {s:?}: want x0,y0,x1,y1");
    }
    Ok(Grid { x0: bx[0], y0: bx[1], w: bx[2] - bx[0] + 1, h: bx[3] - bx[1] + 1 })
}

fn opts_of(a: &Args) -> Result<Opts> {
    let strict = a.has("strict");
    Ok(Opts {
        h: a.get("h", 8usize),
        feed: a.has("feed") || strict,
        lever: a.has("lever") || strict,
        seq: a.has("seq"),
        fix: match a.s("fix") {
            Some(p) => Some(read_fix(p)?),
            None => None,
        },
        obsidian: !a.has("no-obsidian"),
        rblocks: !a.has("no-rblocks"),
        explain: a.has("explain"),
        sym: !a.has("no-sym"),
        private: a.has("private") || strict,
        start: !a.has("no-start"),
        strict,
        budget: a.s("budget").and_then(|v| v.parse().ok()).or(if strict { Some(2) } else { None }),
        static_cond: a.has("static-cond") || strict,
        frame3: a.has("frame3") || strict,
        max_pistons: a.s("max-pistons").and_then(|v| v.parse().ok()),
        physics: a.has("physics"),
    })
}

/// Build and solve one instance: its header line, then the verdict and the report.
fn run_one(g: Grid, o: &Opts, a: &Args) -> Result<(String, String)> {
    use std::fmt::Write as _;
    let t0 = std::time::Instant::now();
    let mut m = build(g, o);
    if let Some(p) = a.s("plan") {
        force_plan(&mut m, p, o.h)?;
    }
    if let Some(c) = a.s("cfg") {
        let cfg = match c {
            "sat" => rustsat_cadical::Config::Sat,
            "unsat" => rustsat_cadical::Config::Unsat,
            other => bail!("--cfg sat|unsat, not {other}"),
        };
        let _ = m.sat.s.set_configuration(cfg);
    }
    if let Some(seed) = a.s("seed").and_then(|s| s.parse::<i32>().ok()) {
        let _ = m.sat.s.set_option("seed", seed);
    }
    m.sat.drop_caches();
    let head = format!(
        "macrosat: box {}x{} at ({},{}), h {} per edge, feed {}, lever {}, {}, fix {}: {} vars, {} clauses, built in {:.1} s",
        g.w,
        g.h,
        g.x0,
        g.y0,
        o.h,
        o.feed,
        o.lever,
        if o.seq { "sequential" } else { "parallel steps" },
        o.fix.is_some(),
        m.sat.vars(),
        m.sat.clauses,
        t0.elapsed().as_secs_f64()
    );
    let t1 = std::time::Instant::now();
    let assumps: Vec<Lit> = m.exempt.as_ref().map(|ex| ex.iter().map(|l| !*l).collect()).unwrap_or_default();
    let r = if assumps.is_empty() { m.sat.s.solve()? } else { rustsat::solvers::SolveIncremental::solve_assumps(&mut m.sat.s, &assumps)? };
    let secs = t1.elapsed().as_secs_f64();
    let what = if o.feed { " with every firing fed" } else { "" };
    let mut out = String::new();
    match r {
        SolverResult::Sat => {
            let _ = writeln!(out, "SAT in {secs:.1} s: a mechanism{what} within {} steps per edge", o.h);
            out.push_str(&report(&m, o));
            if let Some(prefix) = a.s("export") {
                let prefix = if a.pos.first().map(|s| s.as_str()) == Some("sweep") { format!("{prefix}-{}_{}_{}_{}-h{}", g.x0, g.y0, g.x0 + g.w - 1, g.y0 + g.h - 1, o.h) } else { prefix.to_string() };
                let (design, plan) = export(&m, o, &prefix);
                std::fs::write(format!("{prefix}.txt"), design)?;
                std::fs::write(format!("{prefix}.ideal"), plan)?;
                let _ = writeln!(out, "exported {prefix}.txt + {prefix}.ideal");
            }
        }
        SolverResult::Unsat => {
            let _ = writeln!(out, "UNSAT in {secs:.1} s: no mechanism{what} within {} steps per edge in this box", o.h);
            if let Some(ex) = &m.exempt {
                let core = rustsat::solvers::SolveIncremental::core(&mut m.sat.s)?;
                let cells: Vec<(i32, i32)> = (0..g.n()).filter(|&c| core.contains(&ex[c])).map(|c| g.xy(c)).collect();
                let _ = writeln!(out, "core: exempting the pistons firing at {cells:?} from the feed would allow it");
            }
        }
        SolverResult::Interrupted => {
            let _ = writeln!(out, "INTERRUPTED after {secs:.1} s");
        }
    }
    Ok((head, out))
}

/// `macrosat sweep --boxes "x0,y0,x1,y1;..." --hs 8,12 --jobs N --out DIR [model flags]`: every
/// box x horizon in its own thread, `DIR/<x0>_<y0>_<x1>_<y1>-h<h>.txt` each, one summary line per
/// instance as it finishes.
fn sweep(a: &Args) -> Result<()> {
    let boxes: Vec<(String, Grid)> = a.s("boxes").ok_or_else(|| anyhow!("--boxes"))?.split(';').filter(|s| !s.trim().is_empty()).map(|s| Ok((s.trim().to_string(), grid_of(s)?))).collect::<Result<_>>()?;
    let hs: Vec<usize> = a.s("hs").unwrap_or("8").split(',').map(|v| v.trim().parse::<usize>()).collect::<Result<_, _>>()?;
    let out = std::path::PathBuf::from(a.s("out").unwrap_or("/tmp/macrosat"));
    std::fs::create_dir_all(&out)?;
    let base = opts_of(a)?;
    let mut queue: Vec<(String, Grid, usize)> = Vec::new();
    for &h in &hs {
        for (label, g) in &boxes {
            queue.push((label.clone(), *g, h));
        }
    }
    queue.reverse();
    let queue = std::sync::Arc::new(std::sync::Mutex::new(queue));
    let jobs = a.get("jobs", 4usize).max(1);
    std::thread::scope(|sc| {
        for _ in 0..jobs {
            let (queue, base, out) = (queue.clone(), base.clone(), out.clone());
            sc.spawn(move || {
                loop {
                    let Some((label, g, h)) = queue.lock().unwrap().pop() else { break };
                    let o = Opts { h, ..base.clone() };
                    let file = out.join(format!("{}-h{h}.txt", label.replace(',', "_")));
                    match run_one(g, &o, a) {
                        Ok((head, text)) => {
                            let _ = std::fs::write(&file, format!("{head}\n{text}"));
                            println!("{label} h{h}: {}", text.lines().next().unwrap_or(""));
                        }
                        Err(e) => println!("{label} h{h}: ERROR {e}"),
                    }
                }
            });
        }
    });
    Ok(())
}

pub fn cli(raw: &[String]) -> Result<()> {
    let a = Args::parse(raw);
    if a.pos.first().map(|s| s.as_str()) == Some("sweep") {
        return sweep(&a);
    }
    let g = grid_of(a.s("box").unwrap_or("-2,-2,4,4"))?;
    let o = opts_of(&a)?;
    let (head, text) = run_one(g, &o, &a)?;
    eprintln!("{head}");
    print!("{text}");
    Ok(())
}
