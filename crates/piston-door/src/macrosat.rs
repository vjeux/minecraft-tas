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
struct Opts {
    h: usize,
    feed: bool,
    lever: bool,
    seq: bool,
    fix: Option<Vec<(i32, i32, usize)>>,
    obsidian: bool,
    rblocks: bool,
    explain: bool,
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
                        // into air: the block 2 ahead comes in when it is movable
                        let q = sat.var();
                        sat.add(&[!q, a]);
                        sat.add(&[!q, mv[s][line[1]]]);
                        sat.add(&[q, !a, !mv[s][line[1]]]);
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
/// step may write them) and the cell to name in the report.
struct FeedOpt {
    u: Lit,
    cells: Vec<usize>,
    at: usize,
}

/// A selector that implies "the lever touches cell e".
fn lever_opt(sat: &mut Sat, lev_at: &[Option<Lit>], e: usize, opts: &mut Vec<FeedOpt>) {
    if let Some(at) = lev_at[e] {
        let u = sat.var();
        sat.add(&[!u, at]);
        opts.push(FeedOpt { u, cells: Vec::new(), at: e });
    }
}

/// The feed options of step `s`, per (cell * 4 + facing): a power source (wiring cell, redstone
/// block) in a feed cell; a conductor (stone, obsidian) in a feed cell with a power source or the
/// lever next to it; the lever next to the piston or to the cell above it (above the box: on the
/// top face of the piston's top-row neighbor).
fn feed_step(sat: &mut Sat, g: Grid, ty: &[Vec<Vec<Lit>>], lev_at: &[Option<Lit>], s: usize) -> HashMap<usize, Vec<FeedOpt>> {
    let n = g.n();
    let mut out = HashMap::new();
    for c in 0..n {
        for d in 0..4 {
            let mut opts: Vec<FeedOpt> = Vec::new();
            lever_opt(sat, lev_at, c, &mut opts);
            match g.step(c, 2) {
                Some(a) => lever_opt(sat, lev_at, a, &mut opts),
                None => {
                    for u in [0, 1] {
                        if let Some(e) = g.step(c, u) {
                            lever_opt(sat, lev_at, e, &mut opts);
                        }
                    }
                }
            }
            for q in g.feed_cells(c, d) {
                let u = sat.var();
                sat.add(&[!u, ty[s][q][WIRE], ty[s][q][RBLOCK]]);
                opts.push(FeedOpt { u, cells: vec![q], at: q });
                for r in (0..4).filter_map(|e| g.step(q, e)).filter(|&r| r != c) {
                    let u = sat.var();
                    sat.add(&[!u, ty[s][q][STONE], ty[s][q][OBS]]);
                    sat.add(&[!u, ty[s][r][WIRE], ty[s][r][RBLOCK]]);
                    opts.push(FeedOpt { u, cells: vec![q, r], at: q });
                }
                if let Some(at) = lev_at[q] {
                    let u = sat.var();
                    sat.add(&[!u, ty[s][q][STONE], ty[s][q][OBS]]);
                    sat.add(&[!u, at]);
                    opts.push(FeedOpt { u, cells: vec![q], at: q });
                }
            }
            out.insert(c * 4 + d, opts);
        }
    }
    out
}

/// One step: sources, actions, justification, independence, feed.
#[allow(clippy::too_many_arguments)]
fn step(sat: &mut Sat, g: Grid, o: &Opts, ty: &[Vec<Vec<Lit>>], mv: &[Vec<Lit>], lev_at: &[Option<Lit>], exempt: Option<&[Lit]>, s: usize) -> (Vec<Act>, HashMap<usize, Vec<FeedOpt>>) {
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
    let feed = if o.feed { feed_step(sat, g, ty, lev_at, s) } else { HashMap::new() };
    if o.feed {
        for a in &acts {
            let opts = &feed[&(a.c * 4 + a.d)];
            let mut cl = vec![!a.lit];
            cl.extend(opts.iter().map(|f| f.u));
            if let Some(ex) = exempt {
                cl.push(ex[a.c]);
            }
            sat.add(&cl);
            for f in opts {
                for &q in &f.cells {
                    if a.writes.contains(&q) {
                        continue;
                    }
                    if let Some(w) = wr[q] {
                        sat.add(&[!a.lit, !f.u, !w]);
                    }
                }
            }
        }
    }
    (acts, feed)
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
    for s in 0..steps {
        let (a, f) = step(&mut sat, g, o, &ty, &mv, &lev_at, exempt.as_deref(), s);
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
    door(&mut sat, g, &ty, o);
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
        WIRE => "w".into(),
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

fn report(m: &Model, o: &Opts) {
    let g = m.g;
    println!("rest:");
    for l in state_lines(m, 0) {
        println!("{l}");
    }
    for &(e, l) in &m.lv {
        if m.sat.value(l) {
            println!("lever on the block at {:?} (its front/back face, or its outer face at the box edge)", g.xy(e));
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
            println!("{} step {s:>2}: {what}{}", if s < o.h { "open " } else { "close" }, fed.map(|q| format!("  [fed from {q:?}]")).unwrap_or_default());
        }
    }
    println!("open:");
    for l in state_lines(m, o.h) {
        println!("{l}");
    }
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

pub fn cli(raw: &[String]) -> Result<()> {
    let a = Args::parse(raw);
    let bx: Vec<i32> = a.s("box").unwrap_or("-2,-2,4,4").split(',').map(|v| v.trim().parse::<i32>()).collect::<Result<_, _>>()?;
    if bx.len() != 4 {
        bail!("--box x0,y0,x1,y1");
    }
    let g = Grid { x0: bx[0], y0: bx[1], w: bx[2] - bx[0] + 1, h: bx[3] - bx[1] + 1 };
    let o = Opts {
        h: a.get("h", 8usize),
        feed: a.has("feed"),
        lever: a.has("lever"),
        seq: a.has("seq"),
        fix: match a.s("fix") {
            Some(p) => Some(read_fix(p)?),
            None => None,
        },
        obsidian: !a.has("no-obsidian"),
        rblocks: !a.has("no-rblocks"),
        explain: a.has("explain"),
    };
    let t0 = std::time::Instant::now();
    let mut m = build(g, &o);
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
    eprintln!(
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
    match r {
        SolverResult::Sat => {
            println!("SAT in {secs:.1} s: a mechanism{what} within {} steps per edge", o.h);
            report(&m, &o);
        }
        SolverResult::Unsat => {
            println!("UNSAT in {secs:.1} s: no mechanism{what} within {} steps per edge in this box", o.h);
            if let Some(ex) = &m.exempt {
                let core = rustsat::solvers::SolveIncremental::core(&mut m.sat.s)?;
                let cells: Vec<(i32, i32)> = (0..g.n()).filter(|&c| core.contains(&ex[c])).map(|c| g.xy(c)).collect();
                println!("core: exempting the pistons firing at {cells:?} from the feed would allow it");
            }
        }
        SolverResult::Interrupted => println!("INTERRUPTED after {secs:.1} s"),
    }
    Ok(())
}
