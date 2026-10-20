//! `mctas door families`: the catalog of 1-wide (flat) 3x3 door MECHANISMS per box.
//!
//! A family = the pistons that move the 9 door blocks and the paths the blocks take (wiring and
//! timing aside). Everything lives in the door plane: pistons face east/west/up/down, the door
//! is the 3x3 at x 0..2, y 0..2 (door coordinates), the box W x H holds every block and every
//! piston head at every state of the cycle.
//!
//! Model: a fast grid port of mech-synth's macro piston model (`mechsynth.rs`, itself a port of
//! `PistonStructureResolver` + the sticky retract rules): one settled piston action at a time,
//! `L` = toggle the level (extend / retract with the sticky pull), `P` = a 2 gt pulse from rest
//! (push and spit; or extend into a gap and pull the block beyond). No slime or honey (the
//! default and, for now, the only mode). A level-on that cannot extend is refused (strict), and
//! a move that would leave the box is refused.
//!
//! Generation ("exit plans"): every door block gets a path of unit moves out of the 3x3 (the
//! center exactly 2 moves, the 8 others 1 or 2), and every move gets a provider: a PULL by a
//! sticky piston two cells ahead (retracted with a gap, or extended at rest with its head on
//! the gap), or a PUSH by a piston one or two cells behind; a piston already placed may serve
//! again from its rest cell or lifted one cell along its own axis by a LIFT piston right behind
//! it (the elevator). The union of the providers is a candidate mechanism (<= P pistons).
//! Each distinct candidate is then solved exactly in the macro model (the whole reachable state
//! graph): some schedule must open (3x3 empty, SEAMLESS: no piston, head or obsidian in the 12
//! ring cells around the opening, every swept ring cell that is air when open has nothing but
//! air or a door block right behind it) and close back to the exact rest state. Survivors are
//! reduced to MINIMAL mechanisms (no piston can be dropped), mirror images (x -> 2 - x) are
//! merged, and each gets its shortest open/close schedules and, when one exists, ONE pulse train
//! that both lever edges can share (product BFS of the run from rest and the run from open).

use crate::mechsynth::{self, Act, Ev, Kind, World};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const MAXC: usize = 144;
pub const AIR: u8 = 0;
pub const DOOR: u8 = 1;
pub const STONE: u8 = 2;
pub const OBS: u8 = 3;
/// A redstone block (strip track): pushable, powers the pistons beside it (not through their front).
pub const RB: u8 = 4;
pub const BODY: u8 = 16;
pub const HEAD: u8 = 48;
/// East, west, up, down (mechsynth's D indices 0..3).
pub const F: [(i8, i8); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const FNAME: [&str; 4] = ["E", "W", "U", "D"];
pub const MAX_PUSH: usize = 12;

fn opp(f: u8) -> u8 {
    f ^ 1
}
fn fdir(d: (i8, i8)) -> u8 {
    F.iter().position(|&x| x == d).unwrap() as u8
}
pub fn in_t(x: i8, y: i8) -> bool {
    (0..=2).contains(&x) && (0..=2).contains(&y)
}
/// The 12 hallway-ring cells of the door plane (face-adjacent to the 3x3).
pub fn in_r(x: i8, y: i8) -> bool {
    ((x == -1 || x == 3) && (0..=2).contains(&y)) || ((y == -1 || y == 3) && (0..=2).contains(&x))
}

/// A piece at rest (door coordinates).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum K {
    Stone,
    Obsidian,
    /// f: 0 E, 1 W, 2 U, 3 D.
    Piston { sticky: bool, f: u8, ext: bool },
    /// A redstone block carried in the row line (strip track, `--rblocks`).
    Redstone,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Pc {
    pub x: i8,
    pub y: i8,
    pub k: K,
}

impl Pc {
    pub fn code(&self) -> String {
        match self.k {
            K::Stone => format!("stone({},{})", self.x, self.y),
            K::Obsidian => format!("obsidian({},{})", self.x, self.y),
            K::Redstone => format!("R({},{})", self.x, self.y),
            K::Piston { sticky, f, ext } => format!("{}{}{}({},{})", if sticky { "S" } else { "N" }, FNAME[f as usize], if ext { "+" } else { "" }, self.x, self.y),
        }
    }
    pub(crate) fn cells(&self) -> Vec<(i8, i8)> {
        match self.k {
            K::Piston { ext: true, f, .. } => vec![(self.x, self.y), (self.x + F[f as usize].0, self.y + F[f as usize].1)],
            _ => vec![(self.x, self.y)],
        }
    }
    pub fn mirror(&self) -> Pc {
        let k = match self.k {
            K::Piston { sticky, f, ext } => K::Piston { sticky, f: if f < 2 { opp(f) } else { f }, ext },
            k => k,
        };
        Pc { x: 2 - self.x, y: self.y, k }
    }
}

/// The box in door coordinates: x in x0..x0+w, y in y0..y0+h.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Bx {
    pub x0: i8,
    pub y0: i8,
    pub w: i8,
    pub h: i8,
}

impl Bx {
    pub fn has(&self, x: i8, y: i8) -> bool {
        x >= self.x0 && x < self.x0 + self.w && y >= self.y0 && y < self.y0 + self.h
    }
    pub fn idx(&self, x: i8, y: i8) -> usize {
        ((y - self.y0) as usize) * (self.w as usize) + (x - self.x0) as usize
    }
    pub fn xy(&self, i: usize) -> (i8, i8) {
        (self.x0 + (i % self.w as usize) as i8, self.y0 + (i / self.w as usize) as i8)
    }
    pub fn n(&self) -> usize {
        (self.w as usize) * (self.h as usize)
    }
}

/// One state: the grid (codes) + the level bits; `pp` = piston positions (derived).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct St {
    pub g: [u8; MAXC],
    pub pow: u16,
    pub pp: [u8; 16],
    /// Pistons held on by quasi-connectivity (the piston right above them is powered).
    pub qc: u16,
}

/// Quasi-connectivity coupling (default on): whatever powers a piston X also gives X's cell a
/// neighbor signal, so a retracted, unpowered piston Y right BELOW X (X's cell is Y's
/// quasi-connectivity cell) is armed and fires with X's update: Y follows X's pulse, X's level-on
/// arms Y (held on QC), X's level-off releases a Y held that way.
pub static QC: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// The static part of a mechanism: box + pistons (sticky, facing) by id.
#[derive(Clone, Debug)]
pub struct Mech {
    pub bx: Bx,
    pub pieces: Vec<Pc>,
    pub info: Vec<(bool, u8)>,
    /// piece index of piston id
    pub pid: Vec<usize>,
    /// Extended (powered) at rest, by piston id.
    pub ext_rest: Vec<bool>,
    /// Cells a piston may not act from (strip track's `--forbid-fire`: door-plane x columns
    /// that no wiring can reach); empty = no restriction.
    pub forbid: Vec<bool>,
    /// The mechanism has redstone blocks: pistons next to one (not through their front) are
    /// powered, and every state is settled (powered pistons extend, unpowered ones retract).
    pub has_r: bool,
}

/// Extended-at-rest pullers (the DEFAULT since 2026-10-19; `--no-ext-pullers` turns it off): the
/// 1-move exits of non-core blocks may also use a sticky puller EXTENDED at rest (its head in the
/// frame cell; the level-off pulls the block into it). c79 and the community 7x8 exit that way
/// (TR / MR released by the lever), so the retracted-only generator left both out.
pub static EXT_PULLERS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// `--ext-max`: the most 1-move exits by a puller EXTENDED at rest per mechanism (each needs the
/// level to hold it; the lever-held top of e78 / t7b / ring56 has 3: TL/TM/TR's pullers, c79 2:
/// its MR and TR pullers, the community 7x8 1: its TR puller). Bounds the generator's blow-up:
/// every non-core block's 1-move exit has twice the choices with extended pullers allowed.
/// 7x8 (P <= 10, two 1; adjacent pullers, below): 124k candidates retracted-only, 690k with 1,
/// 845k with 2, 870k with 3; with `--ext-apart` and 2: 1.75M; unbounded and apart, the generator
/// ran > 7 min single-threaded without finishing.
pub static EXT_MAX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(3);
/// `--ext-apart` turns this off: by default every 1-move exit by an extended puller after the
/// first must sit next to an earlier one (8-neighbourhood), as one level holds them all (c79's
/// MR/TR pullers (4,1)/(4,2); the lever-held top row (0..2,4)).
pub static EXT_ADJACENT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
/// `--ext-dist`: how near an earlier extended-puller exit each later one must be (Chebyshev).
pub static EXT_DIST: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
/// `--bottom-su` (mechspace): exactly one piston in the box's bottom row, sticky facing up.
pub static BOTTOM_SU: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--lift-ext` (audit bound a): a piston extended at rest may be reused lifted / carried (it
/// retracts first).
pub static LIFT_EXT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--carry-in` (audit bound b, with --carry): a new provider piston may rest 1-2 cells beside its
/// action position and be carried there.
pub static CARRY_IN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--ring-escape`: a piston resting IN a ring cell (it must be gone when open) gets escape
/// providers: carried 1-2 cells along the ring row / column into a ring corner or straight out of
/// the frame, by a pull carrier 2 ahead or a push carrier 1 behind (that carrier escapes too when
/// it rests in the ring). c77's 7x7 w1: P (1,-1) is push-spit by Q (0,-1) to (2,-1) and pulled to
/// the corner (3,-1) by R (4,-1); Q is pulled to (-1,-1) by L (-2,-1).
pub static RING_ESCAPE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--ring-escape`: most escapes (ring-resting pistons given a way out) per candidate.
pub static RING_ESCAPE_MAX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(3);
/// `--ring-escape-only`: emit only candidates with at least one ring escape (the new space; the
/// plain candidates are the default run's).
pub static RING_ESCAPE_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--feedable`: keep only candidates whose every piston has a cell wiring could feed it from at
/// rest (see [`feedable`]).
pub static FEEDABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--fed`: keep only SOLVED mechanisms whose every receiver has a free feed cell over the cycle
/// ([`unfed_cells`]; one box-edge piston may be the lever's).
pub static FED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The `--fed` bar on a solution.
pub fn fed_ok(m: &Mech, rest: &St, sol: &Sol) -> bool {
    if !FED.load(Ordering::Relaxed) {
        return true;
    }
    let (list, lever) = unfed_cells(m, rest, sol);
    list.len() <= lever
}

/// `--feedable` (c77's filter for the 7x7 wiggle doors): every piston of `v` has a FEED cell at
/// rest: a neighbour (not its head side) or, by quasi-connectivity, the cell above it or one of
/// that cell's neighbours, which is free (in the box, outside the 3x3, no piece) and either off
/// the ring or a ring cell with a free off-ring neighbour (a static frame stone a margin
/// component can strongly power). Structural and optimistic: the real check is the solved
/// cycle's (`connect`, the catalog's wire column).
pub fn feedable(bx: Bx, v: &[Pc]) -> bool {
    let occ: HashSet<(i8, i8)> = v.iter().flat_map(|p| p.cells()).collect();
    let free0 = |c: (i8, i8)| bx.has(c.0, c.1) && !in_t(c.0, c.1) && !occ.contains(&c);
    let free = |c: (i8, i8)| free0(c) && (!in_r(c.0, c.1) || F.iter().any(|d| free0((c.0 + d.0, c.1 + d.1)) && !in_r(c.0 + d.0, c.1 + d.1)));
    v.iter().all(|p| {
        let K::Piston { f, .. } = p.k else { return true };
        let head = F[f as usize];
        let side = F.iter().filter(|&&d| d != head).any(|d| free((p.x + d.0, p.y + d.1)));
        let a = (p.x, p.y + 1);
        let qc = free(a) || F.iter().any(|d| (a.0 + d.0, a.1 + d.1) != (p.x, p.y) && free((a.0 + d.0, a.1 + d.1)));
        side || qc
    })
}

/// `families --rows --forbid-fire`: x columns (door coordinates) no piston may fire from.
pub static FORBID_X: std::sync::OnceLock<Vec<i8>> = std::sync::OnceLock::new();
/// `families --rows --rblocks N`: redstone blocks per row candidate.
pub static RMAX: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
/// `families --rows --r-both`: a redstone block powers the pistons on BOTH sides, front included
/// (lockstep rows 0 and 1: the block in the row above quasi-powers both neighbours).
pub static RBOTH: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct E {
    pub id: u8,
    pub act: Act,
}

pub enum Out {
    Ok(St),
    Noop,
    Illegal,
}

impl Mech {
    pub fn new(bx: Bx, pieces: &[Pc]) -> Option<(Mech, St)> {
        let mut pieces = pieces.to_vec();
        pieces.sort();
        let mut g = [AIR; MAXC];
        let mut pp = [0u8; 16];
        let mut pow = 0u16;
        let mut info = Vec::new();
        let mut pid = Vec::new();
        for x in 0..3 {
            for y in 0..3 {
                // a row (or column) box holds only part of the door
                if !bx.has(x, y) {
                    continue;
                }
                g[bx.idx(x, y)] = DOOR;
            }
        }
        for (pi, p) in pieces.iter().enumerate() {
            for (x, y) in p.cells() {
                if !bx.has(x, y) || g[bx.idx(x, y)] != AIR {
                    return None;
                }
            }
            match p.k {
                K::Stone => g[bx.idx(p.x, p.y)] = STONE,
                K::Obsidian => g[bx.idx(p.x, p.y)] = OBS,
                K::Redstone => g[bx.idx(p.x, p.y)] = RB,
                K::Piston { sticky, f, ext } => {
                    let id = info.len();
                    if id >= 16 {
                        return None;
                    }
                    info.push((sticky, f));
                    pid.push(pi);
                    let i = bx.idx(p.x, p.y);
                    g[i] = BODY + id as u8;
                    pp[id] = i as u8;
                    if ext {
                        let (dx, dy) = F[f as usize];
                        g[bx.idx(p.x + dx, p.y + dy)] = HEAD + id as u8;
                        pow |= 1 << id;
                    }
                }
            }
        }
        let ext_rest: Vec<bool> = (0..info.len()).map(|id| pow & (1 << id) != 0).collect();
        let forbid = match FORBID_X.get() {
            Some(xs) if !xs.is_empty() => (0..bx.n()).map(|i| xs.contains(&bx.xy(i).0)).collect(),
            _ => Vec::new(),
        };
        let has_r = pieces.iter().any(|p| matches!(p.k, K::Redstone));
        let mut m = Mech { bx, pieces, info, pid, ext_rest, forbid, has_r };
        let mut st = St { g, pow, pp, qc: 0 };
        if has_r {
            // a piston held out at rest by a redstone block beside it is not wired: no level bit
            for id in 0..m.info.len() {
                if m.ext_rest[id] && m.rpow(&st, id) {
                    st.pow &= !(1u16 << id);
                    m.ext_rest[id] = false;
                }
            }
            // the rest state must already be settled (pistons beside a block extended)
            let mut t = st.clone();
            if !m.settle(&mut t) || t != st {
                return None;
            }
        }
        if QC.load(Ordering::Relaxed) {
            // a piston held powered at rest QC-arms a retracted piston right below it: unstable
            for id in 0..m.info.len() {
                if !m.ext_rest[id] {
                    continue;
                }
                if let Some(b) = m.step(st.pp[id] as usize, 3) {
                    let c = st.g[b];
                    if (BODY..HEAD).contains(&c) && !m.ext_rest[(c - BODY) as usize] {
                        return None;
                    }
                }
            }
        }
        Some((m, st))
    }

    pub fn step(&self, i: usize, f: u8) -> Option<usize> {
        let (x, y) = self.bx.xy(i);
        let (dx, dy) = F[f as usize];
        let (nx, ny) = (x + dx, y + dy);
        if self.bx.has(nx, ny) { Some(self.bx.idx(nx, ny)) } else { None }
    }

    fn ext(&self, s: &St, id: usize) -> bool {
        let (_, f) = self.info[id];
        self.step(s.pp[id] as usize, f).is_some_and(|h| s.g[h] == HEAD + id as u8)
    }

    fn pushable(&self, s: &St, c: u8) -> bool {
        match c {
            DOOR | STONE | RB => true,
            OBS => false,
            c if (BODY..HEAD).contains(&c) => !self.ext(s, (c - BODY) as usize),
            _ => false,
        }
    }

    /// The push list from `start` along f: Ok(list) / Err(true) = out of box / Err(false) = blocked.
    fn push_list(&self, s: &St, start: Option<usize>, f: u8) -> Result<Vec<usize>, bool> {
        let mut list = Vec::new();
        let mut c = start.ok_or(true)?;
        while s.g[c] != AIR {
            if !self.pushable(s, s.g[c]) {
                return Err(false);
            }
            list.push(c);
            if list.len() > MAX_PUSH {
                return Err(false);
            }
            c = self.step(c, f).ok_or(true)?;
        }
        Ok(list)
    }

    fn move_list(&self, s: &mut St, list: &[usize], f: u8) {
        for &c in list.iter().rev() {
            let n = self.step(c, f).unwrap();
            let v = s.g[c];
            s.g[n] = v;
            s.g[c] = AIR;
            if (BODY..HEAD).contains(&v) {
                s.pp[(v - BODY) as usize] = n as u8;
            }
        }
    }

    /// Sticky pull: the block two cells ahead of the piston at `p` (head already gone).
    fn pull(&self, s: &mut St, p: usize, f: u8) {
        let Some(one) = self.step(p, f) else { return };
        let Some(two) = self.step(one, f) else { return };
        let c = s.g[two];
        if s.g[one] != AIR || c == AIR || c == OBS || !self.pushable(s, c) {
            return;
        }
        s.g[one] = c;
        s.g[two] = AIR;
        if (BODY..HEAD).contains(&c) {
            s.pp[(c - BODY) as usize] = one as u8;
        }
    }

    /// Spell out the quasi-connectivity coupling of a sequence: after each event that also fired
    /// the piston right below (see [`QC`]), that piston's own event (same act). For tools that
    /// replay sequences without QC (mech-synth's ideal plans, door topology).
    pub fn expand(&self, start: &St, seq: &[E]) -> Vec<E> {
        let mut out = Vec::new();
        let mut c = start.clone();
        for &e in seq {
            out.push(e);
            let a = self.apply(&c, e);
            let r = self.apply_raw(&c, e);
            let (Out::Ok(an), rr) = (a, r) else { continue };
            let differs = match &rr {
                Out::Ok(rn) => *rn != an,
                _ => true,
            };
            if differs {
                let p = c.pp[e.id as usize] as usize;
                if let Some(b) = self.step(p, 3) {
                    let v = c.g[b];
                    if (BODY..HEAD).contains(&v) && (v - BODY) != e.id {
                        out.push(E { id: v - BODY, act: e.act });
                    }
                }
            }
            c = an;
        }
        out
    }

    /// One event, with the quasi-connectivity coupling of [`QC`].
    pub fn apply(&self, s: &St, e: E) -> Out {
        let out = self.apply_raw(s, e);
        if !QC.load(Ordering::Relaxed) {
            return out;
        }
        let Out::Ok(mut n) = out else { return out };
        let id = e.id as usize;
        let p = s.pp[id] as usize;
        let Some(below) = self.step(p, 3) else { return Out::Ok(n) };
        let c = n.g[below];
        if !(BODY..HEAD).contains(&c) {
            return Out::Ok(n);
        }
        let j = (c - BODY) as usize;
        if j == id {
            return Out::Ok(n);
        }
        let bit = 1u16 << j;
        let ev = E { id: j as u8, act: e.act };
        match e.act {
            Act::P => {
                // Y fires while X is out (mid-pulse): an extended X cannot be pushed, so a Y
                // facing up into X is blocked (the elevator's lift under its pulled piston).
                if self.info[id].1 != 3 && n.pow & bit == 0 && !self.ext(s, j) && s.pow & (1u16 << id) == 0 && !self.ext(s, id) && s.g[below] == BODY + j as u8 {
                    let (sticky, f) = self.info[id];
                    let Ok(list) = self.push_list(s, self.step(p, f), f) else { return Out::Ok(n) };
                    let mut mid = s.clone();
                    let pull = list.is_empty();
                    self.move_list(&mut mid, &list, f);
                    let h = self.step(p, f).unwrap();
                    mid.g[h] = HEAD + id as u8;
                    match self.apply_raw(&mid, ev) {
                        Out::Ok(m) => mid = m,
                        Out::Noop => {}
                        Out::Illegal => return Out::Illegal,
                    }
                    if mid.g[h] == HEAD + id as u8 {
                        mid.g[h] = AIR;
                    }
                    if pull && sticky {
                        self.pull(&mut mid, p, f);
                    }
                    if mid == *s {
                        return Out::Noop;
                    }
                    n = mid;
                }
            }
            Act::L => {
                if n.pow & (1 << id) != 0 {
                    if n.pow & bit == 0 {
                        match self.apply_raw(&n, ev) {
                            Out::Ok(m) => {
                                n = m;
                                n.qc |= bit;
                            }
                            Out::Noop => {}
                            Out::Illegal => return Out::Illegal,
                        }
                    }
                } else if n.qc & bit != 0 {
                    if let Out::Ok(m) = self.apply_raw(&n, ev) {
                        n = m;
                    }
                    n.qc &= !bit;
                }
            }
        }
        Out::Ok(n)
    }

    pub fn apply_raw(&self, s: &St, e: E) -> Out {
        let id = e.id as usize;
        let (sticky, f) = self.info[id];
        let p = s.pp[id] as usize;
        let bit = 1u16 << id;
        let powered = s.pow & bit != 0;
        let ext = self.ext(s, id);
        if !self.forbid.is_empty() && self.forbid[p] {
            return Out::Illegal;
        }
        if self.has_r && self.rpow(s, id) {
            return Out::Illegal; // driven by its redstone block, not by wiring
        }
        let out = self.apply_inner(s, e, id, sticky, f, p, bit, powered, ext);
        match out {
            Out::Ok(mut n) if self.has_r => {
                if self.settle(&mut n) { Out::Ok(n) } else { Out::Illegal }
            }
            o => o,
        }
    }

    /// Is piston `id` powered by a redstone block beside it (its back; with `--r-both` also its
    /// front)?
    pub fn rpow(&self, s: &St, id: usize) -> bool {
        let (_, f) = self.info[id];
        let p = s.pp[id] as usize;
        let both = RBOTH.get().copied().unwrap_or(false);
        for d in 0..4u8 {
            if d == f && !both {
                continue;
            }
            if let Some(j) = self.step(p, d) {
                if s.g[j] == RB {
                    return true;
                }
            }
        }
        false
    }

    /// Settle a state with redstone blocks: a piston powered (level bit or a block beside it)
    /// and retracted extends if it can; one unpowered and extended retracts (sticky pull). False
    /// = a move out of the box or no fixpoint.
    pub fn settle(&self, s: &mut St) -> bool {
        for _ in 0..32 {
            let mut changed = false;
            for id in 0..self.info.len() {
                let (sticky, f) = self.info[id];
                let p = s.pp[id] as usize;
                let eff = s.pow & (1 << id) != 0 || self.rpow(s, id);
                let ext = self.ext(s, id);
                if eff && !ext {
                    match self.push_list(s, self.step(p, f), f) {
                        Err(true) => return false,
                        Err(false) => {}
                        Ok(list) => {
                            self.move_list(s, &list, f);
                            s.g[self.step(p, f).unwrap()] = HEAD + id as u8;
                            changed = true;
                        }
                    }
                } else if !eff && ext {
                    let h = self.step(p, f).unwrap();
                    s.g[h] = AIR;
                    if sticky {
                        self.pull(s, p, f);
                    }
                    changed = true;
                }
            }
            if !changed {
                return true;
            }
        }
        false
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_inner(&self, s: &St, e: E, id: usize, sticky: bool, f: u8, p: usize, bit: u16, powered: bool, ext: bool) -> Out {
        let mut n = s.clone();
        match e.act {
            Act::L => {
                if powered {
                    n.pow &= !bit;
                    if ext {
                        let h = self.step(p, f).unwrap();
                        n.g[h] = AIR;
                        if sticky {
                            self.pull(&mut n, p, f);
                        }
                    }
                    Out::Ok(n)
                } else {
                    match self.push_list(s, self.step(p, f), f) {
                        Err(true) => Out::Illegal,
                        Err(false) => Out::Illegal, // strict: a blocked level-on is refused
                        Ok(list) => {
                            self.move_list(&mut n, &list, f);
                            n.g[self.step(p, f).unwrap()] = HEAD + id as u8;
                            n.pow |= bit;
                            Out::Ok(n)
                        }
                    }
                }
            }
            Act::P => {
                if powered || ext {
                    return Out::Noop;
                }
                match self.push_list(s, self.step(p, f), f) {
                    Err(true) => Out::Illegal,
                    Err(false) => Out::Noop,
                    Ok(list) if !list.is_empty() => {
                        self.move_list(&mut n, &list, f);
                        Out::Ok(n)
                    }
                    Ok(_) => {
                        if !sticky {
                            return Out::Noop;
                        }
                        self.pull(&mut n, p, f);
                        if n == *s { Out::Noop } else { Out::Ok(n) }
                    }
                }
            }
        }
    }

    /// The pulse model: a 2 gt pulse on every piston, levels only on pistons extended at rest
    /// (their release and re-power).
    pub fn events_pulse(&self) -> Vec<E> {
        let mut v = Vec::new();
        for id in 0..self.info.len() {
            v.push(E { id: id as u8, act: Act::P });
            if self.ext_rest[id] {
                v.push(E { id: id as u8, act: Act::L });
            }
        }
        v
    }

    pub fn events(&self, levels: bool) -> Vec<E> {
        let mut v = Vec::new();
        for id in 0..self.info.len() {
            v.push(E { id: id as u8, act: Act::P });
            if levels {
                v.push(E { id: id as u8, act: Act::L });
            }
        }
        v
    }

    fn code_at(&self, s: &St, x: i8, y: i8) -> u8 {
        if self.bx.has(x, y) { s.g[self.bx.idx(x, y)] } else { AIR }
    }

    /// Static open test (FULL seamless for a flat door): the 3x3 empty, the ring holds only
    /// air, door blocks or plain (stone) helpers.
    pub fn is_open(&self, s: &St) -> bool {
        for x in 0..3 {
            for y in 0..3 {
                if self.code_at(s, x, y) != AIR {
                    return false;
                }
            }
        }
        for (x, y) in ring() {
            if !matches!(self.code_at(s, x, y), AIR | DOOR | STONE) {
                return false;
            }
        }
        true
    }

    /// The hole rule over a cycle: ring cells some state of the cycle fills but the open state
    /// leaves empty must have air or a door block right behind them (outward) when open.
    pub fn holes_ok(&self, cycle: &[&St], open: &St) -> bool {
        for (x, y) in ring() {
            if self.code_at(open, x, y) != AIR {
                continue;
            }
            if !cycle.iter().any(|s| self.code_at(s, x, y) != AIR) {
                continue;
            }
            let (ox, oy) = outward(x, y);
            if !matches!(self.code_at(open, ox, oy), AIR | DOOR) {
                return false;
            }
        }
        true
    }

    /// No piston, head or obsidian in the ring at rest (the coordinator's strict reading).
    pub fn rest_ring_clean(&self, rest: &St) -> bool {
        ring().into_iter().all(|(x, y)| matches!(self.code_at(rest, x, y), AIR | DOOR | STONE))
    }

    /// ASCII picture of a state (top row first): # door, s stone, O obsidian, piston letters
    /// (> < ^ v; uppercase sticky), heads = '=' or '|'.
    pub fn render(&self, s: &St) -> String {
        let mut out = String::new();
        for y in (self.bx.y0..self.bx.y0 + self.bx.h).rev() {
            out.push_str(&format!("{:>3} ", y));
            for x in self.bx.x0..self.bx.x0 + self.bx.w {
                let c = s.g[self.bx.idx(x, y)];
                let ch = match c {
                    AIR => if in_t(x, y) { ',' } else { '.' },
                    DOOR => '#',
                    STONE => 's',
                    OBS => 'O',
                    RB => 'R',
                    c if (BODY..HEAD).contains(&c) => {
                        let (sticky, f) = self.info[(c - BODY) as usize];
                        let ch = ['>', '<', '^', 'v'][f as usize];
                        if sticky { ch } else { ['}', '{', 'A', 'V'][f as usize] }
                    }
                    c => {
                        let (_, f) = self.info[(c - HEAD) as usize];
                        if f < 2 { '=' } else { '|' }
                    }
                };
                out.push(ch);
            }
            out.push('\n');
        }
        out
    }

    pub fn fmt_ev(&self, e: E) -> String {
        let p = self.pieces[self.pid[e.id as usize]];
        format!("{}{}", if e.act == Act::P { "P" } else { "L" }, p.code())
    }
    pub fn fmt_seq(&self, s: &[E]) -> String {
        s.iter().map(|e| self.fmt_ev(*e)).collect::<Vec<_>>().join(" ")
    }
    /// Bounding box (door coords) of every state in the list.
    pub fn extent(&self, states: &[&St]) -> (i8, i8, i8, i8) {
        let (mut x0, mut y0, mut x1, mut y1) = (0i8, 0i8, 2i8, 2i8);
        for s in states {
            for i in 0..self.bx.n() {
                if s.g[i] != AIR {
                    let (x, y) = self.bx.xy(i);
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }
        (x0, y0, x1, y1)
    }
}

pub fn ring() -> Vec<(i8, i8)> {
    let mut v = Vec::new();
    for t in 0..3 {
        v.push((-1, t));
        v.push((3, t));
        v.push((t, -1));
        v.push((t, 3));
    }
    v
}

pub fn outward(x: i8, y: i8) -> (i8, i8) {
    if x == -1 {
        (-2, y)
    } else if x == 3 {
        (4, y)
    } else if y == -1 {
        (x, -2)
    } else {
        (x, 4)
    }
}

// ---------------------------------------------------------------- solving

#[derive(Clone, Debug)]
pub struct Sol {
    pub open: Vec<E>,
    pub close: Vec<E>,
    pub sym: Option<Vec<E>>,
    pub sym_levels: bool,
    pub states: usize,
    pub extent: (i8, i8, i8, i8),
    /// Found only with level events (the pulse model has no open+close).
    pub levels: bool,
}

/// Weighted A* from `start` to states meeting `goal` (up to `want` goal states, in order found).
/// Err(()) when the cap stops it before `want` goals; Ok(v) with fewer goals = space exhausted.
fn astar(m: &Mech, start: &St, goal: &dyn Fn(&St) -> bool, h: &dyn Fn(&St) -> u32, cap: usize, want: usize, evs: &[E]) -> (Vec<(St, Vec<E>)>, bool) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let evs = evs.to_vec();
    let mut states: Vec<St> = vec![start.clone()];
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    let mut gs: Vec<u32> = vec![0];
    let mut index: HashMap<St, u32> = HashMap::new();
    index.insert(start.clone(), 0);
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((2 * h(start), 0u32, 0u32)));
    let mut closed = vec![false];
    let mut out = Vec::new();
    let mut capped = false;
    while let Some(Reverse((_, g, s))) = heap.pop() {
        if closed[s as usize] || g > gs[s as usize] {
            continue;
        }
        closed[s as usize] = true;
        let st = states[s as usize].clone();
        if goal(&st) {
            let mut path = Vec::new();
            let mut c = s;
            while let (p, Some(e)) = parent[c as usize] {
                path.push(e);
                c = p;
            }
            path.reverse();
            out.push((st, path));
            if out.len() >= want {
                return (out, false);
            }
            continue;
        }
        for &e in &evs {
            let Out::Ok(n) = m.apply(&st, e) else { continue };
            let ng = g + 1;
            match index.get(&n) {
                Some(&id) => {
                    if ng < gs[id as usize] && !closed[id as usize] {
                        gs[id as usize] = ng;
                        parent[id as usize] = (s, Some(e));
                        heap.push(Reverse((ng + 2 * h(&n), ng, id)));
                    }
                }
                None => {
                    if states.len() >= cap {
                        capped = true;
                        continue;
                    }
                    let id = states.len() as u32;
                    let f = ng + 2 * h(&n);
                    index.insert(n.clone(), id);
                    states.push(n);
                    parent.push((s, Some(e)));
                    gs.push(ng);
                    closed.push(false);
                    heap.push(Reverse((f, ng, id)));
                }
            }
        }
    }
    (out, capped)
}

/// Open + close by weighted A* (h = door blocks still in the 3x3, the center counting 2; then
/// cells differing from rest). Ok(None) = the reachable space has no open state (exhausted);
/// Err(()) = not found within the cap.
/// Open + close: first EXACTLY in the pulse model (the whole state graph: pulses, levels only on
/// pistons extended at rest); if that space has no open+close, weighted A* with level events too
/// (h = door blocks still in the 3x3, the center counting 2; then cells differing from rest).
/// Ok(None) = proven impossible in the pulse model and not found with levels within the cap;
/// Err(()) = the pulse graph itself hit the cap.
/// `--max-actions K` (0 = off): every solve allows at most K actions (pulses or level toggles) per
/// piston per edge (pockets3: each pulse needs its own watchable event next to the piston's feed
/// pocket; 1-2 per edge is wirable).
pub static CONS_K: AtomicUsize = AtomicUsize::new(0);
/// `--extra-actions B`: with `--max-actions`, at most B actions per edge beyond one per piston
/// (B = 1: one piston may act twice; usize::MAX = no budget). w1 needs B = 1 at K = 2.
pub static CONS_EXTRA: AtomicUsize = AtomicUsize::new(usize::MAX);
/// `--pareto N`: incomparable action vectors kept per state in the constrained search (more =
/// fewer undecided candidates, slower).
pub static CONS_PARETO: AtomicUsize = AtomicUsize::new(3);
/// `--no-inside`: no piston fires while its base is inside the 3x3 (a piston carried into the door
/// square can only be fed through door blocks).
pub static CONS_NO_INSIDE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn cons_on() -> bool {
    CONS_K.load(Ordering::Relaxed) > 0 || CONS_NO_INSIDE.load(Ordering::Relaxed)
}

/// One edge of the constrained search: BFS over (state, actions per piston, 4 bits each) from
/// `start`; a state already reached with componentwise no more actions dominates. Returns up to
/// `want` paths to distinct goal states (fewest actions first) and whether the search was complete.
fn cons_edge(m: &Mech, start: &St, goal: &dyn Fn(&St) -> bool, evs: &[E], cap: usize, want: usize) -> (Vec<(St, Vec<E>)>, bool) {
    let kk = CONS_K.load(Ordering::Relaxed);
    let track = kk > 0;
    let k = kk.clamp(1, 15) as u64;
    let extra = CONS_EXTRA.load(Ordering::Relaxed);
    // actions beyond one per piston in an action vector
    let excess = |c: u64| (0..16).map(|i| ((c >> (4 * i)) & 15).saturating_sub(1) as usize).sum::<usize>();
    // at most this many incomparable action vectors kept per state (the exact Pareto set blows
    // up): a dropped one makes a "none" verdict unproven
    let pareto = CONS_PARETO.load(Ordering::Relaxed).max(1);
    let mut pruned = false;
    let no_in = CONS_NO_INSIDE.load(Ordering::Relaxed);
    let ids = m.info.len();
    let le = |a: u64, b: u64| (0..ids).all(|i| (a >> (4 * i)) & 15 <= (b >> (4 * i)) & 15);
    let mut nodes: Vec<(St, u64)> = vec![(start.clone(), 0)];
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    let mut seen: HashMap<St, Vec<u64>> = HashMap::new();
    seen.insert(start.clone(), vec![0]);
    let mut q = VecDeque::from([0u32]);
    let mut found: Vec<(St, Vec<E>)> = Vec::new();
    let mut found_st: HashSet<St> = HashSet::new();
    let path = |mut i: u32, parent: &[(u32, Option<E>)]| -> Vec<E> {
        let mut v = Vec::new();
        while let (p, Some(e)) = parent[i as usize] {
            v.push(e);
            i = p;
        }
        v.reverse();
        v
    };
    if goal(start) {
        found.push((start.clone(), Vec::new()));
        found_st.insert(start.clone());
        if found.len() >= want {
            return (found, true);
        }
    }
    let mut complete = true;
    while let Some(i) = q.pop_front() {
        let (st, c) = nodes[i as usize].clone();
        for &e in evs {
            let id = e.id as usize;
            if track && (c >> (4 * id)) & 15 >= k {
                continue;
            }
            if no_in {
                let (x, y) = m.bx.xy(st.pp[id] as usize);
                if in_t(x, y) {
                    continue;
                }
            }
            let Out::Ok(n) = m.apply(&st, e) else { continue };
            let nc = if track { c + (1u64 << (4 * id)) } else { 0 };
            if track && extra != usize::MAX && excess(nc) > extra {
                continue;
            }
            let entry = seen.entry(n.clone()).or_default();
            if entry.iter().any(|&o| le(o, nc)) {
                continue;
            }
            if entry.len() >= pareto {
                pruned = true;
                continue;
            }
            if nodes.len() >= cap {
                complete = false;
                continue;
            }
            entry.push(nc);
            let j = nodes.len() as u32;
            nodes.push((n.clone(), nc));
            parent.push((i, Some(e)));
            q.push_back(j);
            if goal(&n) && found_st.insert(n.clone()) {
                found.push((n, path(j, &parent)));
                if found.len() >= want {
                    return (found, false);
                }
            }
        }
    }
    (found, complete && !pruned)
}

/// The constrained solve (`--max-actions` / `--no-inside`): open = a constrained search from rest
/// to an open state, close = a fresh constrained search from that open state back to rest (up to
/// 32 open states tried, fewest actions first); pulses (+ levels of the extended-at-rest pistons)
/// first, every level next when `levels`. Ok(None) = PROVEN none under the constraints.
fn solve_cons(m: &Mech, rest: &St, cap: usize, levels: bool) -> Result<Option<Sol>, ()> {
    let pulse = solve_cons_with(m, rest, cap, &m.events_pulse(), false);
    match pulse {
        Ok(Some(_)) => pulse,
        _ if !levels => pulse,
        // the pulse events are a subset of the level events: the level verdict decides
        _ => solve_cons_with(m, rest, cap, &m.events(true), true),
    }
}

fn solve_cons_with(m: &Mech, rest: &St, cap: usize, evs: &[E], levels: bool) -> Result<Option<Sol>, ()> {
    let (opens, c1) = cons_edge(m, rest, &|s| m.is_open(s), evs, cap, 32);
    if opens.is_empty() {
        return if c1 { Ok(None) } else { Err(()) };
    }
    let mut any_cap = !c1;
    for (o, open) in &opens {
        let (back, c2) = cons_edge(m, o, &|s| s == rest, evs, cap, 1);
        any_cap |= !c2 && back.is_empty();
        let Some((_, close)) = back.into_iter().next() else { continue };
        let mut cyc = vec![rest.clone()];
        let mut c = rest.clone();
        for e in open.iter().chain(close.iter()) {
            if let Out::Ok(nx) = m.apply(&c, *e) {
                c = nx;
            }
            cyc.push(c.clone());
        }
        let refs: Vec<&St> = cyc.iter().collect();
        if !m.holes_ok(&refs, o) {
            continue;
        }
        let extent = m.extent(&refs);
        return Ok(Some(Sol { open: open.clone(), close, sym: None, sym_levels: false, states: 0, extent, levels }));
    }
    if any_cap { Err(()) } else { Ok(None) }
}

pub fn solve(m: &Mech, rest: &St, cap: usize, levels: bool) -> Result<Option<Sol>, ()> {
    if cons_on() {
        return solve_cons(m, rest, cap, levels);
    }
    match solve_full(m, rest, cap, &m.events_pulse()) {
        Ok(Some(s)) => return Ok(Some(s)),
        Err(()) => return Err(()),
        Ok(None) if !levels => return Ok(None),
        Ok(None) => {}
    }
    match solve_levels(m, rest, cap, &m.events(true)) {
        Ok(s) => Ok(s),
        Err(()) => Ok(None),
    }
}

/// A* in the pulse model (for candidates whose pulse graph is too big to enumerate):
/// Ok(Some) = a real solution; Err(()) = not found within the cap (still unresolved).
/// Exhaustive reachability of an open state in the pulse model with a compact visited set (box
/// cells + levels, boxes up to 64 cells): Ok(true) = some open state is reachable, Ok(false) =
/// PROVEN none (the whole pulse graph explored), Err(()) = cap.
pub fn reach_open(m: &Mech, rest: &St, cap: usize) -> Result<bool, ()> {
    let n = m.bx.n();
    if n > 64 {
        return Err(());
    }
    let key = |s: &St| -> ([u8; 64], u16) {
        let mut k = [0u8; 64];
        k[..n].copy_from_slice(&s.g[..n]);
        (k, s.pow)
    };
    let evs = m.events_pulse();
    let mut seen: HashSet<([u8; 64], u16)> = HashSet::new();
    seen.insert(key(rest));
    let mut q: VecDeque<St> = VecDeque::from([rest.clone()]);
    while let Some(s) = q.pop_front() {
        for &e in &evs {
            let Out::Ok(nx) = m.apply(&s, e) else { continue };
            if m.is_open(&nx) {
                return Ok(true);
            }
            if seen.insert(key(&nx)) {
                if seen.len() > cap {
                    return Err(());
                }
                q.push_back(nx);
            }
        }
    }
    Ok(false)
}

/// `--resolve <file> --boxes WxH`: the unresolved candidates of a run (one per line: box x0 y0
/// then the pieces), re-checked by exhaustive reachability with a bigger cap.
pub fn resolve_cli(path: &std::path::Path, w: i8, h: i8, cap: usize, threads: usize) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let parse = |tok: &str| -> Result<Pc> {
        let (head, rest) = tok.split_once('(').context("piece")?;
        let (xs, ys) = rest.trim_end_matches(')').split_once(',').context("piece xy")?;
        let mut ch = head.chars();
        let sticky = ch.next() == Some('S');
        let f = match ch.next() {
            Some('E') => 0,
            Some('W') => 1,
            Some('U') => 2,
            _ => 3,
        };
        Ok(Pc { x: xs.parse()?, y: ys.parse()?, k: K::Piston { sticky, f, ext: head.ends_with('+') } })
    };
    let mut items: Vec<(Bx, Vec<Pc>)> = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let bx = Bx { x0: t[0].parse()?, y0: t[1].parse()?, w, h };
        let v: Vec<Pc> = t[2..].iter().map(|s| parse(s)).collect::<Result<_>>()?;
        items.push((bx, v));
    }
    let t0 = std::time::Instant::now();
    let (proven, open, capped) = (AtomicUsize::new(0), Mutex::new(Vec::new()), AtomicUsize::new(0));
    let next = AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let (bx, v) = &items[i];
                let Some((m, rest)) = Mech::new(*bx, v) else { continue };
                let r = reach_open(&m, &rest, cap);
                let d = i + 1;
                if d % 50 == 0 {
                    eprintln!("[families resolve] {d}/{} ({} proven, {} capped, {:.0}s)", items.len(), proven.load(Ordering::Relaxed), capped.load(Ordering::Relaxed), t0.elapsed().as_secs_f64());
                }
                match r {
                    Ok(false) => {
                        proven.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(true) => open.lock().unwrap().push(i),
                    Err(()) => {
                        capped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    let open = open.into_inner().unwrap();
    println!(
        "{} candidates: {} PROVEN to have no open state (whole pulse graph), {} reach an open state, {} still over the cap ({cap}) ({:.0}s)",
        items.len(),
        proven.load(Ordering::Relaxed),
        open.len(),
        capped.load(Ordering::Relaxed),
        t0.elapsed().as_secs_f64()
    );
    for i in open {
        let (bx, v) = &items[i];
        let Some((m, rest)) = Mech::new(*bx, v) else { continue };
        let r = solve_pulse_astar(&m, &rest, cap.min(2_000_000));
        println!("  open reachable: {} {} {} -> open+close: {}", bx.x0, bx.y0, v.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "), match r {
            Ok(Some(s)) => format!("YES ({} + {})", s.open.len(), s.close.len()),
            Ok(None) => "no".into(),
            Err(()) => "unknown (cap)".into(),
        });
    }
    Ok(())
}

pub fn solve_pulse_astar(m: &Mech, rest: &St, cap: usize) -> Result<Option<Sol>, ()> {
    solve_levels(m, rest, cap, &m.events_pulse()).map(|o| {
        o.map(|mut s| {
            s.levels = false;
            s
        })
    })
}

fn solve_levels(m: &Mech, rest: &St, cap: usize, evs: &[E]) -> Result<Option<Sol>, ()> {
    let bx = m.bx;
    let ci = bx.idx(1, 1);
    let tcells: Vec<usize> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).map(|(x, y)| bx.idx(x, y)).collect();
    let h_open = |s: &St| -> u32 { tcells.iter().map(|&i| if s.g[i] == AIR { 0 } else if i == ci { 2 } else { 1 }).sum() };
    let (opens, capped) = astar(m, rest, &|s| m.is_open(s), &h_open, cap, 6, evs);
    if opens.is_empty() {
        return if capped { Err(()) } else { Ok(None) };
    }
    let n = bx.n();
    let h_close = |s: &St| -> u32 {
        let mut d = 0;
        for i in 0..n {
            if (s.g[i] == AIR) != (rest.g[i] == AIR) {
                d += 1;
            }
        }
        d / 2 + (s.pow ^ rest.pow).count_ones()
    };
    let mut best: Option<Sol> = None;
    let mut any_cap = capped;
    for (o, open) in &opens {
        let (back, c2) = astar(m, o, &|s| s == rest, &h_close, cap, 1, evs);
        any_cap |= c2;
        let Some((_, close)) = back.into_iter().next() else { continue };
        // the cycle's states for the hole rule and the extent
        let mut cyc = vec![rest.clone()];
        let mut c = rest.clone();
        for e in open.iter().chain(close.iter()) {
            if let Out::Ok(nx) = m.apply(&c, *e) {
                c = nx;
            }
            cyc.push(c.clone());
        }
        let refs: Vec<&St> = cyc.iter().collect();
        if !m.holes_ok(&refs, o) {
            continue;
        }
        if best.as_ref().is_none_or(|b| open.len() + close.len() < b.open.len() + b.close.len()) {
            let extent = m.extent(&refs);
            best = Some(Sol { open: open.clone(), close, sym: None, sym_levels: false, states: 0, extent, levels: true });
        }
    }
    match best {
        Some(b) => Ok(Some(b)),
        None if any_cap => Err(()),
        None => Ok(None),
    }
}

/// Whole reachable graph (capped). None = no open+close; Err(()) = cap hit before an answer.
pub fn solve_full(m: &Mech, rest: &St, cap: usize, evs: &[E]) -> Result<Option<Sol>, ()> {
    let evs = evs.to_vec();
    let mut states: Vec<St> = vec![rest.clone()];
    let mut index: HashMap<St, u32> = HashMap::new();
    index.insert(rest.clone(), 0);
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    let mut edges: Vec<Vec<(u32, E)>> = vec![Vec::new()];
    let mut q = VecDeque::from([0u32]);
    let mut complete = true;
    while let Some(si) = q.pop_front() {
        let st = states[si as usize].clone();
        for &e in &evs {
            let Out::Ok(n) = m.apply(&st, e) else { continue };
            let ni = match index.get(&n) {
                Some(&i) => i,
                None => {
                    if states.len() >= cap {
                        complete = false;
                        continue;
                    }
                    let i = states.len() as u32;
                    index.insert(n.clone(), i);
                    states.push(n);
                    parent.push((si, Some(e)));
                    edges.push(Vec::new());
                    q.push_back(i);
                    i
                }
            };
            edges[si as usize].push((ni, e));
        }
    }
    let opens: Vec<u32> = (0..states.len() as u32).filter(|&i| m.is_open(&states[i as usize])).collect();
    if opens.is_empty() {
        return if complete { Ok(None) } else { Err(()) };
    }
    // distances to rest
    let n = states.len();
    let mut rev: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (s, es) in edges.iter().enumerate() {
        for (t, _) in es {
            rev[*t as usize].push(s as u32);
        }
    }
    let mut d2r = vec![u32::MAX; n];
    d2r[0] = 0;
    let mut q = VecDeque::from([0u32]);
    while let Some(t) = q.pop_front() {
        for &s in &rev[t as usize] {
            if d2r[s as usize] == u32::MAX {
                d2r[s as usize] = d2r[t as usize] + 1;
                q.push_back(s);
            }
        }
    }
    let depth = |mut i: u32| -> usize {
        let mut d = 0;
        while let (p, Some(_)) = parent[i as usize] {
            i = p;
            d += 1;
        }
        d
    };
    let mut ranked: Vec<(usize, u32)> = opens.iter().filter(|&&o| d2r[o as usize] != u32::MAX).map(|&o| (depth(o) + d2r[o as usize] as usize, o)).collect();
    ranked.sort();
    for &(_, o) in ranked.iter().take(64) {
        let mut open = Vec::new();
        let mut c = o;
        let mut path_states = vec![o];
        while let (p, Some(e)) = parent[c as usize] {
            open.push(e);
            c = p;
            path_states.push(c);
        }
        open.reverse();
        let mut close = Vec::new();
        let mut c = o;
        while d2r[c as usize] > 0 {
            let (t, e) = *edges[c as usize].iter().find(|(t, _)| d2r[*t as usize] == d2r[c as usize] - 1).unwrap();
            close.push(e);
            c = t;
            path_states.push(c);
        }
        let cyc: Vec<&St> = path_states.iter().map(|&i| &states[i as usize]).collect();
        if !m.holes_ok(&cyc, &states[o as usize]) {
            continue;
        }
        let extent = m.extent(&cyc);
        return Ok(Some(Sol { open, close, sym: None, sym_levels: false, states: n, extent, levels: false }));
    }
    if complete { Ok(None) } else { Err(()) }
}

/// One train S for both lever edges: S(rest) = an open state, S(that open state) = rest (door
/// blocks interchangeable). Product BFS over (run from rest, run from open); a no-op in one run
/// leaves it as is; a move out of the box (or a blocked level-on) rejects the event.
pub fn sym_train(m: &Mech, rest: &St, open: &St, levels: bool, cap: usize) -> Option<Vec<E>> {
    let evs = m.events(levels);
    let start = (rest.clone(), open.clone());
    let mut index: HashMap<(St, St), u32> = HashMap::new();
    let mut nodes: Vec<(St, St)> = vec![start.clone()];
    let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
    index.insert(start, 0);
    let mut q = VecDeque::from([0u32]);
    while let Some(i) = q.pop_front() {
        let (a, b) = nodes[i as usize].clone();
        for &e in &evs {
            let na = match m.apply(&a, e) {
                Out::Ok(n) => Some(n),
                Out::Noop => None,
                Out::Illegal => continue,
            };
            let nb = match m.apply(&b, e) {
                Out::Ok(n) => Some(n),
                Out::Noop => None,
                Out::Illegal => continue,
            };
            if na.is_none() && nb.is_none() {
                continue;
            }
            let key = (na.unwrap_or_else(|| a.clone()), nb.unwrap_or_else(|| b.clone()));
            if index.contains_key(&key) {
                continue;
            }
            let done = key.0 == *open && key.1 == *rest;
            let j = nodes.len() as u32;
            index.insert(key.clone(), j);
            nodes.push(key);
            parent.push((i, Some(e)));
            if done {
                let mut seq = Vec::new();
                let mut c = j;
                while let (p, Some(e)) = parent[c as usize] {
                    seq.push(e);
                    c = p;
                }
                seq.reverse();
                return Some(seq);
            }
            if nodes.len() >= cap {
                return None;
            }
            q.push_back(j);
        }
    }
    None
}

/// Shortest symmetric train over the open states reachable by `open`-like paths: tries the
/// solution's open state first, then every open state of the graph (up to `tries`).
pub fn best_sym(m: &Mech, rest: &St, sol: &Sol, cap: usize, _tries: usize) -> Option<(Vec<E>, bool)> {
    let mut o = rest.clone();
    for e in &sol.open {
        if let Out::Ok(n) = m.apply(&o, *e) {
            o = n;
        }
    }
    // edge pistons: one pulse on each edge in the found schedules, touching no cell any other
    // piston's action changes (independent toggles); the rest is core
    let cnt = |s: &[E], id: u8, a: Act| s.iter().filter(|e| e.id == id && e.act == a).count();
    let mut touched: Vec<HashSet<usize>> = vec![HashSet::new(); m.info.len()];
    {
        let mut c = rest.clone();
        for e in sol.open.iter().chain(sol.close.iter()) {
            if let Out::Ok(n) = m.apply(&c, *e) {
                for i in 0..m.bx.n() {
                    if n.g[i] != c.g[i] {
                        touched[e.id as usize].insert(i);
                    }
                }
                c = n;
            }
        }
    }
    let mut edges = Vec::new();
    let mut core = Vec::new();
    for id in 0..m.info.len() as u8 {
        let po = cnt(&sol.open, id, Act::P);
        let pc = cnt(&sol.close, id, Act::P);
        let used = po + pc + cnt(&sol.open, id, Act::L) + cnt(&sol.close, id, Act::L);
        if used == 0 {
            continue;
        }
        let alone = (0..m.info.len()).all(|j| j == id as usize || touched[j].is_disjoint(&touched[id as usize]));
        if po == 1 && pc == 1 && used == 2 && !m.ext_rest[id as usize] && alone {
            edges.push(E { id, act: Act::P });
        } else {
            core.push(E { id, act: Act::P });
            if m.ext_rest[id as usize] || sol.levels {
                core.push(E { id, act: Act::L });
            }
        }
    }
    let run = |s: &St, seq: &[E]| -> Option<St> {
        let mut c = s.clone();
        for e in seq {
            match m.apply(&c, *e) {
                Out::Ok(n) => c = n,
                Out::Noop => {}
                Out::Illegal => return None,
            }
        }
        Some(c)
    };
    let k = edges.len();
    let combos: Vec<u32> = if k <= 9 { (0..(1u32 << k)).collect() } else { vec![0, (1u32 << k) - 1] };
    let mut best: Option<Vec<E>> = None;
    for mask in combos {
        let start: Vec<E> = (0..k).filter(|i| mask & (1 << i) != 0).map(|i| edges[i]).collect();
        let end: Vec<E> = (0..k).filter(|i| mask & (1 << i) == 0).map(|i| edges[i]).collect();
        let (Some(x0), Some(y0)) = (run(rest, &start), run(&o, &start)) else { continue };
        // product BFS over the core events
        let fin = |x: &St, y: &St| -> bool { run(x, &end).is_some_and(|a| a == o) && run(y, &end).is_some_and(|b| b == *rest) };
        let found = if fin(&x0, &y0) {
            Some(Vec::new())
        } else {
            let mut index: HashMap<(St, St), u32> = HashMap::new();
            let mut nodes: Vec<(St, St)> = vec![(x0.clone(), y0.clone())];
            let mut parent: Vec<(u32, Option<E>)> = vec![(0, None)];
            index.insert((x0, y0), 0);
            let mut q = VecDeque::from([0u32]);
            let mut got = None;
            let limit = best.as_ref().map_or(usize::MAX, |b| b.len().saturating_sub(k));
            'bfs: while let Some(i) = q.pop_front() {
                let depth = {
                    let mut d = 0;
                    let mut c = i;
                    while let (p, Some(_)) = parent[c as usize] {
                        c = p;
                        d += 1;
                    }
                    d
                };
                if depth + 1 >= limit {
                    break;
                }
                let (a, b) = nodes[i as usize].clone();
                for &e in &core {
                    let na = match m.apply(&a, e) {
                        Out::Ok(n) => Some(n),
                        Out::Noop => None,
                        Out::Illegal => continue,
                    };
                    let nb = match m.apply(&b, e) {
                        Out::Ok(n) => Some(n),
                        Out::Noop => None,
                        Out::Illegal => continue,
                    };
                    if na.is_none() && nb.is_none() {
                        continue;
                    }
                    let key = (na.unwrap_or_else(|| a.clone()), nb.unwrap_or_else(|| b.clone()));
                    if index.contains_key(&key) {
                        continue;
                    }
                    let done = fin(&key.0, &key.1);
                    let j = nodes.len() as u32;
                    index.insert(key.clone(), j);
                    nodes.push(key);
                    parent.push((i, Some(e)));
                    if done {
                        let mut seq = Vec::new();
                        let mut c = j;
                        while let (p, Some(e)) = parent[c as usize] {
                            seq.push(e);
                            c = p;
                        }
                        seq.reverse();
                        got = Some(seq);
                        break 'bfs;
                    }
                    if nodes.len() >= cap {
                        break 'bfs;
                    }
                    q.push_back(j);
                }
            }
            got
        };
        let Some(c) = found else { continue };
        let mut full = start.clone();
        full.extend(c);
        full.extend(end.iter().copied());
        // the hole rule on the train's two runs
        let mut cyc: Vec<St> = vec![rest.clone()];
        for st in [rest, &o] {
            let mut cur = st.clone();
            for e in &full {
                if let Out::Ok(n) = m.apply(&cur, *e) {
                    cur = n;
                }
                cyc.push(cur.clone());
            }
        }
        let refs: Vec<&St> = cyc.iter().collect();
        if !m.holes_ok(&refs, &o) {
            continue;
        }
        if best.as_ref().is_none_or(|b| full.len() < b.len()) {
            best = Some(full);
        }
    }
    let lv = core.iter().any(|e| e.act == Act::L);
    best.map(|b| (b, lv))
}

// ---------------------------------------------------------------- generation (exit plans)

struct Gen {
    bx: Bx,
    pmax: usize,
    lifts: bool,
    pushes: bool,
    /// General carrying depth (0 = off): reused pistons displaced 1-2 cells in any direction.
    carry: usize,
    /// Middle-row side blocks that must leave vertically (`--vmid`).
    vmid: Vec<(i8, i8)>,
    out: HashSet<Vec<Pc>>,
    memo: HashSet<(usize, Vec<(i8, i8)>, Vec<Pc>)>,
    order: Vec<(i8, i8)>,
    /// `door audit`: only pieces of this set may be added (membership of a known door's
    /// mechanism in the generator's space, without enumerating the whole space).
    allow: Option<HashSet<Pc>>,
}

fn n_pistons(v: &[Pc]) -> usize {
    v.iter().filter(|p| matches!(p.k, K::Piston { .. })).count()
}

impl Gen {
    fn occupied(&self, v: &[Pc], x: i8, y: i8) -> bool {
        v.iter().any(|p| p.cells().contains(&(x, y)))
    }
    /// Add a piece; None when it conflicts. Same cell + same piece = reuse.
    fn add(&self, v: &[Pc], p: Pc) -> Option<Vec<Pc>> {
        if v.contains(&p) {
            return Some(v.to_vec());
        }
        if self.allow.as_ref().is_some_and(|a| !a.contains(&p)) {
            return None;
        }
        for (x, y) in p.cells() {
            if !self.bx.has(x, y) || in_t(x, y) || self.occupied(v, x, y) {
                return None;
            }
        }
        // `--bottom-su` (mechspace): the box's bottom row holds exactly one piston, sticky,
        // facing up (vjeux's hint for the community 7x8)
        if BOTTOM_SU.load(Ordering::Relaxed) && p.y == self.bx.y0 {
            if !matches!(p.k, K::Piston { sticky: true, f: 2, .. }) || v.iter().any(|q| q.y == self.bx.y0) {
                return None;
            }
        }
        let mut w = v.to_vec();
        w.push(p);
        w.sort();
        if n_pistons(&w) > self.pmax {
            return None;
        }
        Some(w)
    }

    /// Every way to provide the move c -> c + d.
    fn providers(&self, v: &[Pc], c: (i8, i8), d: (i8, i8)) -> Vec<Vec<Pc>> {
        let mut out = Vec::new();
        let fd = fdir(d);
        // PULL: sticky at c + 2d facing -d
        let a = (c.0 + 2 * d.0, c.1 + 2 * d.1);
        let fpull = opp(fd);
        for ext in [false, true] {
            if let Some(w) = self.add(v, Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: fpull, ext } }) {
                out.push(w);
            }
        }
        // PUSH: piston at c - k d facing d (k = 1, 2), retracted
        if self.pushes {
            for k in 1..=2i8 {
                let a = (c.0 - k * d.0, c.1 - k * d.1);
                if in_t(a.0, a.1) {
                    continue;
                }
                for sticky in [false, true] {
                    if let Some(w) = self.add(v, Pc { x: a.0, y: a.1, k: K::Piston { sticky, f: fd, ext: false } }) {
                        out.push(w);
                    }
                }
            }
        }
        // REUSE LIFTED: an existing piston of the right kind one cell off along its axis
        if self.lifts {
            for p in v {
                let K::Piston { sticky, f, ext } = p.k else { continue };
                let (fx, fy) = F[f as usize];
                let want: Vec<(i8, i8, bool)> = {
                    // (action position, needs sticky) for pull and push providers
                    let mut w = vec![(c.0 + 2 * d.0, c.1 + 2 * d.1, true)];
                    if f == fd {
                        w.push((c.0 - d.0, c.1 - d.1, false));
                        w.push((c.0 - 2 * d.0, c.1 - 2 * d.1, false));
                    }
                    w
                };
                for (ax, ay, need_sticky) in want {
                    if need_sticky && (!sticky || f != fpull) {
                        continue;
                    }
                    if !need_sticky && f != fd {
                        continue;
                    }
                    // lifted toward its facing: rest + f = action; lift at rest - f facing f
                    // (`LIFT_EXT`: also a piston extended at rest: it retracts, then is lifted)
                    let ext_ok = !ext || LIFT_EXT.load(Ordering::Relaxed);
                    if p.x + fx == ax && p.y + fy == ay && ext_ok {
                        if let Some(w) = self.add(v, Pc { x: p.x - fx, y: p.y - fy, k: K::Piston { sticky: true, f, ext: false } }) {
                            out.push(w);
                        }
                    }
                    // pulled back: rest - f = action; lift extended at rest - 2f facing f
                    if p.x - fx == ax && p.y - fy == ay && ext_ok {
                        if let Some(w) = self.add(v, Pc { x: p.x - 2 * fx, y: p.y - 2 * fy, k: K::Piston { sticky: true, f, ext: true } }) {
                            out.push(w);
                        }
                    }
                    // general CARRYING (--carry N): displaced by 1-2 cells in any direction
                    // (sideways too), moved there by carriers that may be carried themselves
                    if self.carry > 0 && ext_ok {
                        let (dx, dy) = (ax - p.x, ay - p.y);
                        let k = dx.abs() + dy.abs();
                        if (dx == 0 || dy == 0) && (1..=2).contains(&k) {
                            let u = (dx.signum(), dy.signum());
                            for w in self.carry_by(v, (p.x, p.y), u, k, self.carry) {
                                out.push(w);
                            }
                        }
                    }
                }
            }
        }
        // CARRY IN (`CARRY_IN`, with --carry): a NEW provider piston resting 1-2 cells beside its
        // action position, carried there first (fast-clean's SD(0,3) shuttled by SE(-1,3))
        if self.carry > 0 && CARRY_IN.load(Ordering::Relaxed) {
            let mut acts: Vec<((i8, i8), K)> = vec![((c.0 + 2 * d.0, c.1 + 2 * d.1), K::Piston { sticky: true, f: fpull, ext: false })];
            if self.pushes {
                for k in 1..=2i8 {
                    for sticky in [false, true] {
                        acts.push(((c.0 - k * d.0, c.1 - k * d.1), K::Piston { sticky, f: fd, ext: false }));
                    }
                }
            }
            for (a, kind) in acts {
                if in_t(a.0, a.1) || !self.bx.has(a.0, a.1) {
                    continue;
                }
                for u in [(1i8, 0i8), (-1, 0), (0, 1), (0, -1)] {
                    for j in 1..=2i8 {
                        let r = (a.0 - j * u.0, a.1 - j * u.1);
                        if in_t(r.0, r.1) {
                            continue;
                        }
                        if let Some(w0) = self.add(v, Pc { x: r.0, y: r.1, k: kind }) {
                            for w in self.carry_by(&w0, r, u, j, self.carry) {
                                out.push(w);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// Piece sets that move the movable piece at `r` by `k` (1 or 2) cells along `u`: a pull
    /// carrier (sticky, 2 cells ahead, retracted or extended at rest with its head between) or a
    /// push carrier (1 cell behind, sticky or normal); with depth > 1 a carrier may itself rest
    /// one cell further out and be carried in (stacked lifts). Two cells = two carries in a row.
    fn carry_by(&self, v: &[Pc], r: (i8, i8), u: (i8, i8), k: i8, depth: usize) -> Vec<Vec<Pc>> {
        if k == 2 {
            let mut out = Vec::new();
            for w in self.carry_by(v, r, u, 1, depth) {
                out.extend(self.carry_by(&w, (r.0 + u.0, r.1 + u.1), u, 1, depth));
            }
            return out;
        }
        let fu = fdir(u);
        let mut out = Vec::new();
        // pull carrier at r + 2u facing -u
        let t = (r.0 + 2 * u.0, r.1 + 2 * u.1);
        for ext in [false, true] {
            if let Some(w) = self.add(v, Pc { x: t.0, y: t.1, k: K::Piston { sticky: true, f: opp(fu), ext } }) {
                out.push(w);
            }
        }
        // push carrier at r - u facing u
        let b = (r.0 - u.0, r.1 - u.1);
        for sticky in [false, true] {
            if let Some(w) = self.add(v, Pc { x: b.0, y: b.1, k: K::Piston { sticky, f: fu, ext: false } }) {
                out.push(w);
            }
        }
        if depth > 1 {
            // the pull carrier rests one cell further out and is carried in by -u
            let t2 = (t.0 + u.0, t.1 + u.1);
            if let Some(w) = self.add(v, Pc { x: t2.0, y: t2.1, k: K::Piston { sticky: true, f: opp(fu), ext: false } }) {
                out.extend(self.carry_by(&w, t2, (-u.0, -u.1), 1, depth - 1));
            }
            // the push carrier rests one cell further back and is carried in by +u
            let b2 = (b.0 - u.0, b.1 - u.1);
            for sticky in [false, true] {
                if let Some(w) = self.add(v, Pc { x: b2.0, y: b2.1, k: K::Piston { sticky, f: fu, ext: false } }) {
                    out.extend(self.carry_by(&w, b2, u, 1, depth - 1));
                }
            }
        }
        out
    }

    fn paths(&self, b: (i8, i8)) -> Vec<Vec<(i8, i8)>> {
        let center = b == (1, 1);
        let mut out = Vec::new();
        for d1 in F {
            let c1 = (b.0 + d1.0, b.1 + d1.1);
            if !self.bx.has(c1.0, c1.1) {
                continue;
            }
            if !center && !in_t(c1.0, c1.1) {
                out.push(vec![d1]);
            }
            for d2 in F {
                if d2 == (-d1.0, -d1.1) {
                    continue;
                }
                let c2 = (c1.0 + d2.0, c1.1 + d2.1);
                if !self.bx.has(c2.0, c2.1) || in_t(c2.0, c2.1) {
                    continue;
                }
                out.push(vec![d1, d2]);
            }
        }
        out
    }

    /// Provide every move of `path` from `b` (all provider combinations).
    fn provide_path(&self, v: &[Pc], b: (i8, i8), path: &[(i8, i8)]) -> Vec<Vec<Pc>> {
        let mut frontier = vec![v.to_vec()];
        let mut c = b;
        for d in path {
            let mut next = Vec::new();
            let mut seen = HashSet::new();
            for w in &frontier {
                for w2 in self.providers(w, c, *d) {
                    if seen.insert(w2.clone()) {
                        next.push(w2);
                    }
                }
            }
            frontier = next;
            c = (c.0 + d.0, c.1 + d.1);
        }
        frontier
    }

    /// The core: the center leaves straight along d in 2 moves, and the edge block on that line
    /// leaves in 2 moves (any 2-move path not ending on the center's park cell).
    fn core(&self, d: (i8, i8)) -> Vec<Vec<Pc>> {
        let e = (1 + d.0, 1 + d.1);
        let cpark = (1 + 2 * d.0, 1 + 2 * d.1);
        if !self.bx.has(cpark.0, cpark.1) {
            return Vec::new();
        }
        let mut out: HashSet<Vec<Pc>> = HashSet::new();
        for path in self.paths(e) {
            if path.len() != 2 {
                continue;
            }
            let end = path.iter().fold(e, |c, d| (c.0 + d.0, c.1 + d.1));
            if end == cpark {
                continue;
            }
            for w in self.provide_path(&[], e, &path) {
                for w2 in self.provide_path(&w, (1, 1), &[d, d]) {
                    out.insert(w2);
                }
            }
        }
        let mut v: Vec<Vec<Pc>> = out.into_iter().collect();
        v.sort();
        v
    }

    /// `--ring-escape`: every way to give the ring-resting pistons of `v` (not yet in `done`) an
    /// escape: carried 1-2 cells along u into a cell outside the 3x3 and the ring (a ring corner,
    /// or straight out of the frame), through ring cells only; new carriers resting in the ring
    /// escape in turn, at most `left` escapes in all. Only complete sets are returned.
    fn escapes(&self, v: &[Pc], done: Vec<(i8, i8)>, left: usize) -> Vec<Vec<Pc>> {
        let Some(r) = v.iter().find(|p| matches!(p.k, K::Piston { .. }) && in_r(p.x, p.y) && !done.contains(&(p.x, p.y))).map(|p| (p.x, p.y)) else {
            return if done.is_empty() { Vec::new() } else { vec![v.to_vec()] };
        };
        if left == 0 || n_pistons(v) >= self.pmax {
            return Vec::new();
        }
        let mut done2 = done.clone();
        done2.push(r);
        let mut out = Vec::new();
        for u in F {
            for k in 1..=2i8 {
                let dest = (r.0 + k * u.0, r.1 + k * u.1);
                let via_ok = (1..k).all(|j| in_r(r.0 + j * u.0, r.1 + j * u.1));
                if !via_ok || in_t(dest.0, dest.1) || in_r(dest.0, dest.1) || !self.bx.has(dest.0, dest.1) {
                    continue;
                }
                for w in self.carry_by(v, r, u, k, 1) {
                    out.extend(self.escapes(&w, done2.clone(), left - 1));
                }
            }
        }
        out
    }

    /// The other perimeter blocks: a 1-move exit by a pull-type puller (retracted, or extended at
    /// rest: at most `--ext-max` of those, each next to an earlier one (`ext` = their cells)
    /// unless `--ext-apart`), or, while the budget allows (`two_left`), a 2-move path with any
    /// providers.
    fn others(&mut self, i: usize, v: Vec<Pc>, two_left: usize, ext: Vec<(i8, i8)>) {
        if n_pistons(&v) > self.pmax {
            return;
        }
        if i == self.order.len() {
            let feed = FEEDABLE.load(Ordering::Relaxed);
            if RING_ESCAPE.load(Ordering::Relaxed) {
                for w in self.escapes(&v, Vec::new(), RING_ESCAPE_MAX.load(Ordering::Relaxed)) {
                    if !feed || feedable(self.bx, &w) {
                        self.out.insert(w);
                    }
                }
            }
            if (!feed || feedable(self.bx, &v)) && !RING_ESCAPE_ONLY.load(Ordering::Relaxed) {
                self.out.insert(v);
            }
            return;
        }
        if !self.memo.insert((i, ext.clone(), v.clone())) {
            return;
        }
        let b = self.order[i];
        // --vmid: this middle-row side block (ML or MR) must leave VERTICALLY (2 moves, the
        // first up or down, through the corner cell its column-mate vacated)
        if self.vmid.contains(&b) {
            for path in self.paths(b) {
                if path.len() != 2 || path[0].0 != 0 {
                    continue;
                }
                for w in self.provide_path(&v, b, &path) {
                    self.others(i + 1, w, two_left, ext.clone());
                }
            }
            return;
        }
        for d in F {
            let c = (b.0 + d.0, b.1 + d.1);
            if in_t(c.0, c.1) {
                continue;
            }
            let a = (b.0 + 2 * d.0, b.1 + 2 * d.1);
            for is_ext in [false, true] {
                if is_ext {
                    if !EXT_PULLERS.load(Ordering::Relaxed) || ext.len() >= EXT_MAX.load(Ordering::Relaxed) {
                        continue;
                    }
                    let dist = EXT_DIST.load(Ordering::Relaxed) as i8;
                    if EXT_ADJACENT.load(Ordering::Relaxed) && !ext.is_empty() && !ext.iter().any(|f| (f.0 - a.0).abs() <= dist && (f.1 - a.1).abs() <= dist) {
                        continue;
                    }
                }
                let ext2 = if is_ext {
                    let mut e = ext.clone();
                    e.push(a);
                    e
                } else {
                    ext.clone()
                };
                if let Some(w) = self.add(&v, Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: opp(fdir(d)), ext: is_ext } }) {
                    self.others(i + 1, w, two_left, ext2);
                }
            }
        }
        if two_left > 0 {
            for path in self.paths(b) {
                if path.len() != 2 {
                    continue;
                }
                for w in self.provide_path(&v, b, &path) {
                    self.others(i + 1, w, two_left - 1, ext.clone());
                }
            }
        }
    }
}

/// Candidate mechanisms of one box offset: for each center exit direction, every core option
/// times every exit choice of the 7 other perimeter blocks (`two` of them may take 2 moves).
pub fn generate(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize) -> Vec<Vec<Pc>> {
    generate_v(bx, pmax, lifts, pushes, two, 0, &[])
}

/// [`generate`] with general carrying (`carry` = depth, 0 = elevator lifts only).
pub fn generate_c(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize, carry: usize) -> Vec<Vec<Pc>> {
    generate_v(bx, pmax, lifts, pushes, two, carry, &[])
}

/// [`generate`] with middle-row side blocks forced to leave vertically: `vmid` lists the
/// variants, each a set of blocks among ML (0,1) and MR (2,1); the union of the variants.
pub fn generate_v(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize, carry: usize, vmid: &[Vec<(i8, i8)>]) -> Vec<Vec<Pc>> {
    let variants: Vec<Vec<(i8, i8)>> = if vmid.is_empty() { vec![Vec::new()] } else { vmid.to_vec() };
    let mut all: HashSet<Vec<Pc>> = HashSet::new();
    for vm in &variants {
        for v in generate_one(bx, pmax, lifts, pushes, two, carry, vm) {
            all.insert(v);
        }
    }
    let mut v: Vec<Vec<Pc>> = all.into_iter().collect();
    v.sort();
    v
}

fn generate_one(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize, carry: usize, vmid: &[(i8, i8)]) -> Vec<Vec<Pc>> {
    generate_one_in(bx, pmax, lifts, pushes, two, carry, vmid, None)
}

/// [`generate_v`] restricted to pieces of `allow` (`door audit`: is this known mechanism one of
/// the generator's candidates?). Same rules, same order; only the pieces it may add are limited.
#[allow(clippy::too_many_arguments)]
pub fn generate_within(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize, carry: usize, vmid: &[Vec<(i8, i8)>], allow: &HashSet<Pc>) -> Vec<Vec<Pc>> {
    let variants: Vec<Vec<(i8, i8)>> = if vmid.is_empty() { vec![Vec::new()] } else { vmid.to_vec() };
    let mut all: HashSet<Vec<Pc>> = HashSet::new();
    for vm in &variants {
        all.extend(generate_one_in(bx, pmax, lifts, pushes, two, carry, vm, Some(allow.clone())));
    }
    let mut v: Vec<Vec<Pc>> = all.into_iter().collect();
    v.sort();
    v
}

#[allow(clippy::too_many_arguments)]
fn generate_one_in(bx: Bx, pmax: usize, lifts: bool, pushes: bool, two: usize, carry: usize, vmid: &[(i8, i8)], allow: Option<HashSet<Pc>>) -> Vec<Vec<Pc>> {
    // The 4 center exit directions are independent (each has its own memo): one thread each
    // (extended-at-rest pullers made the generation ~14x bigger, 2026-10-19).
    let outs: Vec<HashSet<Vec<Pc>>> = std::thread::scope(|s| {
        let hs: Vec<_> = F
            .iter()
            .filter(|d| !vmid.contains(&(1 + d.0, 1 + d.1))) // a forced-vertical block is not the center's line edge
            .map(|&d| {
                let allow = allow.clone();
                s.spawn(move || {
                    let e = (1 + d.0, 1 + d.1);
                    let order: Vec<(i8, i8)> = [(1, 0), (1, 2), (0, 1), (2, 1), (0, 0), (2, 0), (0, 2), (2, 2)].into_iter().filter(|&b| b != e).collect();
                    let mut g = Gen { bx, pmax, lifts, pushes, carry, vmid: vmid.to_vec(), out: HashSet::new(), memo: HashSet::new(), order, allow };
                    for c in g.core(d) {
                        if n_pistons(&c) > pmax {
                            continue;
                        }
                        g.others(0, c, two, Vec::new());
                    }
                    g.out
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().expect("generator thread")).collect()
    });
    let mut all: HashSet<Vec<Pc>> = HashSet::new();
    for o in outs {
        all.extend(o);
    }
    let mut v: Vec<Vec<Pc>> = all.into_iter().collect();
    v.sort();
    v
}

/// `door audit`: why `target` (a known door's mechanism pistons) is not one of the generator's
/// candidates in `bx`: per center exit direction, the first core move with no provider among the
/// target's pieces, else the perimeter blocks with no exit option (1-move pullers, 2-move paths),
/// the 2-move budget, and the target's pieces no option ever uses (the generator adds providers
/// only). An over-approximation per block (each block judged with every target piece present).
/// `prefer`: judge only this center exit direction (the door's real one).
pub fn explain_within(bx: Bx, pmax: usize, two: usize, carry: usize, allow: &HashSet<Pc>, prefer: Option<(i8, i8)>) -> Vec<String> {
    let mut target: Vec<Pc> = allow.iter().copied().collect();
    target.sort();
    let name = |b: (i8, i8)| -> String {
        let col = ["L", "M", "R"][b.0.clamp(0, 2) as usize];
        let row = ["B", "M", "T"][b.1.clamp(0, 2) as usize];
        if b == (1, 1) { "C".into() } else { format!("{row}{col}") }
    };
    let mut out = Vec::new();
    if n_pistons(&target) > pmax {
        out.push(format!("{} pistons > P {pmax}", n_pistons(&target)));
    }
    let mut best: Option<(usize, String)> = None;
    for d in F {
        if prefer.is_some_and(|p| p != d) {
            continue;
        }
        let e = (1 + d.0, 1 + d.1);
        let order: Vec<(i8, i8)> = [(1, 0), (1, 2), (0, 1), (2, 1), (0, 0), (2, 0), (0, 2), (2, 2)].into_iter().filter(|&b| b != e).collect();
        let g = Gen { bx, pmax: 99, lifts: true, pushes: true, carry, vmid: Vec::new(), out: HashSet::new(), memo: HashSet::new(), order: order.clone(), allow: Some(allow.clone()) };
        let dn = FNAME[fdir(d) as usize];
        let cpark = (1 + 2 * d.0, 1 + 2 * d.1);
        let mut why: Vec<String> = Vec::new();
        // the core: the edge on the center's line in 2 moves, the center straight in 2
        let edge_ok = g.paths(e).iter().filter(|p| p.len() == 2).filter(|p| p.iter().fold(e, |c, dd| (c.0 + dd.0, c.1 + dd.1)) != cpark).any(|p| !g.provide_path(&target, e, p).is_empty());
        if !edge_ok {
            why.push(format!("{} (the edge on the center's line) has no 2-move path with providers among the pieces", name(e)));
        }
        let c1 = g.providers(&target, (1, 1), d);
        if c1.is_empty() {
            why.push(format!("the center's move 1 (1,1)->({},{}) has no provider among the pieces (a puller at ({},{}), a pusher at ({},{}), or a piston RETRACTED at rest lifted 1 cell along its axis)", 1 + d.0, 1 + d.1, 1 + 2 * d.0, 1 + 2 * d.1, 1 - 2 * d.0, 1 - 2 * d.1));
        } else if g.providers(&target, (1 + d.0, 1 + d.1), d).is_empty() {
            why.push(format!("the center's move 2 ({},{})->({},{}) has no provider among the pieces", 1 + d.0, 1 + d.1, cpark.0, cpark.1));
        }
        // ORDER: the real core starts empty; a piston is reused (lifted / carried) only after an
        // earlier provider placed it, so one that acts only away from its rest cell is never placed
        if why.is_empty() && g.core(d).is_empty() {
            let mut needs = Vec::new();
            for mv in [((1, 1), d), ((1 + d.0, 1 + d.1), d)] {
                if !g.providers(&[], mv.0, mv.1).is_empty() {
                    continue;
                }
                for q in target.iter().filter(|q| matches!(q.k, K::Piston { .. })) {
                    if !g.providers(&[*q], mv.0, mv.1).is_empty() {
                        needs.push(format!("{} (for the center's move from ({},{}))", q.code(), mv.0.0, mv.0.1));
                    }
                }
            }
            let edge_real = g.paths(e).iter().filter(|p| p.len() == 2).filter(|p| p.iter().fold(e, |c, dd| (c.0 + dd.0, c.1 + dd.1)) != cpark).any(|p| !g.provide_path(&[], e, p).is_empty());
            if !needs.is_empty() {
                why.push(format!(
                    "the core needs a piston that only ever acts away from its rest cell, lifted or carried there: {}; the generator reuses only pistons an earlier provider placed (Gen::providers iterates the pieces already chosen), so it is never placed",
                    needs.join(", ")
                ));
            } else if !edge_real {
                why.push(format!("{} (the edge on the center's line) has no 2-move path whose providers the generator places in order", name(e)));
            } else {
                why.push("the core (edge + center) has options per move but no combination the generator builds in order".into());
            }
        }
        // the other blocks
        let mut twos = 0;
        for &b in &order {
            let one = F.iter().any(|&dd| {
                let c = (b.0 + dd.0, b.1 + dd.1);
                let a = (b.0 + 2 * dd.0, b.1 + 2 * dd.1);
                !in_t(c.0, c.1) && [false, true].iter().any(|&ext| (!ext || EXT_PULLERS.load(Ordering::Relaxed)) && target.contains(&Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: opp(fdir(dd)), ext } }))
            });
            if one {
                continue;
            }
            let ext_one = F.iter().any(|&dd| {
                let a = (b.0 + 2 * dd.0, b.1 + 2 * dd.1);
                target.contains(&Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: opp(fdir(dd)), ext: true } })
            });
            let two_ok = g.paths(b).iter().filter(|p| p.len() == 2).any(|p| !g.provide_path(&target, b, p).is_empty());
            if two_ok {
                twos += 1;
            } else if ext_one {
                why.push(format!("{} exits only by a puller EXTENDED at rest (1-move exits: retracted pullers only with --no-ext-pullers)", name(b)));
            } else {
                why.push(format!("{} has no exit among the pieces (no retracted sticky puller 2 ahead, no 2-move path with providers)", name(b)));
            }
        }
        if twos > two {
            why.push(format!("{twos} perimeter blocks need a 2-move path (budget --two {two})"));
        }
        // the extended-puller exits' bound (--ext-max, adjacency)
        if EXT_PULLERS.load(Ordering::Relaxed) {
            let mut cells: Vec<(i8, i8)> = Vec::new();
            for &b in &order {
                for dd in F {
                    let c = (b.0 + dd.0, b.1 + dd.1);
                    let a = (b.0 + 2 * dd.0, b.1 + 2 * dd.1);
                    if !in_t(c.0, c.1) && !cells.contains(&a) && target.contains(&Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: opp(fdir(dd)), ext: true } }) {
                        cells.push(a);
                    }
                }
            }
            let max = EXT_MAX.load(Ordering::Relaxed);
            if cells.len() > max {
                why.push(format!("{} 1-move exits by pullers EXTENDED at rest > --ext-max {max}", cells.len()));
            }
            // connected within --ext-dist (the generator adds each near an earlier one)
            let dist = EXT_DIST.load(Ordering::Relaxed) as i8;
            let mut seen = vec![false; cells.len()];
            if !cells.is_empty() {
                seen[0] = true;
                let mut grew = true;
                while grew {
                    grew = false;
                    for j in 0..cells.len() {
                        if !seen[j] && (0..cells.len()).any(|k| seen[k] && (cells[k].0 - cells[j].0).abs() <= dist && (cells[k].1 - cells[j].1).abs() <= dist) {
                            seen[j] = true;
                            grew = true;
                        }
                    }
                }
            }
            if EXT_ADJACENT.load(Ordering::Relaxed) && seen.iter().any(|s| !s) {
                why.push("its extended-puller exits are not next to each other (each must touch an earlier one unless --ext-apart)".into());
            }
        }
        // pieces no option uses
        if why.is_empty() {
            let mut used: HashSet<Pc> = HashSet::new();
            let cores = g.core(d);
            for c in &cores {
                used.extend(c.iter().copied());
            }
            let base = cores.first().cloned().unwrap_or_default();
            for &b in &order {
                for dd in F {
                    let a = (b.0 + 2 * dd.0, b.1 + 2 * dd.1);
                    for ext in [false, true] {
                        let p = Pc { x: a.0, y: a.1, k: K::Piston { sticky: true, f: opp(fdir(dd)), ext } };
                        if target.contains(&p) {
                            used.insert(p);
                        }
                    }
                }
                for p in g.paths(b).iter().filter(|p| p.len() == 2) {
                    for w in g.provide_path(&base, b, p) {
                        used.extend(w);
                    }
                }
            }
            let unused: Vec<String> = target.iter().filter(|p| !used.contains(p)).map(|p| p.code()).collect();
            if !unused.is_empty() {
                why.push(format!("pieces no exit option uses: {} (the generator adds providers only)", unused.join(" ")));
            }
        }
        let text = format!("center exits {dn}: {}", if why.is_empty() { "every block has an option (the exact union is still not produced)".to_string() } else { why.join("; ") });
        if best.as_ref().is_none_or(|(n, _)| why.len() < *n) {
            best = Some((why.len(), text));
        }
    }
    if let Some((_, t)) = best {
        out.push(t);
    }
    out
}

/// Canonical form under the left-right mirror (x -> 2 - x).
pub fn canon(v: &[Pc]) -> Vec<Pc> {
    let mut a = v.to_vec();
    a.sort();
    let mut b: Vec<Pc> = v.iter().map(|p| p.mirror()).collect();
    b.sort();
    a.min(b)
}

// ---------------------------------------------------------------- catalog

#[derive(Clone, Debug)]
pub struct Entry {
    pub pieces: Vec<Pc>,
    pub w: i8,
    pub h: i8,
    pub bx: Bx,
    pub sol: Sol,
    pub rest_clean: bool,
    pub pistons: usize,
    pub sticky: usize,
    pub free_cells: usize,
    pub ring_quasi: usize,
    pub wire: Wire,
    /// Track box56's `door topology` verdict: strict / permissive / no.
    pub topo: String,
    pub verified: Option<String>,
    /// The pieces as solved in `bx` (the canonical form or its mirror image).
    pub orient: Vec<Pc>,
}

/// Wiring feasibility (the quadrant / parity analysis of track box56, automated).
#[derive(Clone, Debug, Default)]
pub struct Wire {
    /// A: one region feeds every piston; B: links needed, each with an odd pulse count where
    /// an odd count is needed; C: some region needs a pulse merger (only even links reach it);
    /// X: a piston no free cell can feed, or two regions no link reaches.
    pub class: char,
    /// Free-cell regions (sizes, largest first).
    pub regions: Vec<usize>,
    /// Regions that some piston can only be fed from.
    pub forced: usize,
    /// Pistons fed from more than one position (riders).
    pub riders: usize,
    /// Receivers (a piston at one firing cell) whose largest feeding free-cell region has fewer
    /// than 3 cells (unfed ones included): pockets3's wirability bar.
    pub weak: usize,
    /// Pocket load x10: the worst free region's actions (receivers only it can feed) per cell.
    pub load: usize,
    pub why: String,
}

/// The receivers no free cell can feed (the wiring analysis below), minus those next to (or
/// under, by quasi-connectivity) a STATIC ring stone that a free off-ring cell can strongly power
/// (c77). Returns their firing cells and how many of them the best lever placement holds (the
/// lever stands outside the volume, on the back face of a never-moved piston; see below).
pub fn unfed_cells(m: &Mech, rest: &St, sol: &Sol) -> (Vec<(i8, i8)>, usize) {
    let bx = m.bx;
    let mut states = vec![rest.clone()];
    let mut c = rest.clone();
    for e in sol.open.iter().chain(sol.close.iter()) {
        if let Out::Ok(n) = m.apply(&c, *e) {
            c = n;
        }
        states.push(c.clone());
    }
    let pieces = &m.pieces;
    let at = |s: &St, x: i8, y: i8| if bx.has(x, y) { s.g[bx.idx(x, y)] } else { AIR };
    let w = wiring(m, rest, sol);
    let swept = |x: i8, y: i8| states.iter().any(|s| at(s, x, y) != AIR && at(s, x, y) != STONE) || pieces.iter().any(|p| p.cells().contains(&(x, y)) && !matches!(p.k, K::Stone));
    let free_off = |x: i8, y: i8| bx.has(x, y) && !in_t(x, y) && !in_r(x, y) && !swept(x, y);
    let static_ring = |x: i8, y: i8| in_r(x, y) && !swept(x, y) && F.iter().any(|d| free_off(x + d.0, y + d.1));
    let list: Vec<(i8, i8)> = w
        .why
        .split("; ")
        .filter(|s| s.ends_with("has no free cell to feed it"))
        .filter_map(|s| {
            // "<code> at (x,y) has no free cell to feed it"
            let xy = s.split(" at (").nth(1)?.split(')').next()?;
            let (xs, ys) = xy.split_once(',')?;
            Some((xs.parse::<i8>().ok()?, ys.parse::<i8>().ok()?))
        })
        .filter(|&(x, y)| {
            let near = F.iter().any(|d| static_ring(x + d.0, y + d.1)) || static_ring(x, y + 1) || F.iter().any(|d| (x + d.0, y + 1 + d.1) != (x, y) && static_ring(x + d.0, y + 1 + d.1));
            !near
        })
        .collect();
    // (the lever hangs on the BACK face of a piston that is never moved, outside the box: d67
    // game-checked that side-face attachments pop when the piston extends or retracts, and that
    // anything attached to a moved piston pops. It holds that piston, the pistons beside the lever
    // cell, and by quasi-connectivity the pistons whose above cell touches it (e88's lever on the
    // top trio's middle back face holds all three). Every lever-held piston does exactly one level
    // toggle per edge, at the flip: the schedule must still work with them moved to the front)
    let front_ok = |ids: &[usize]| -> bool {
        for &id in ids {
            let one_l = |edge: &[E]| edge.iter().filter(|e| e.id as usize == id).count() == 1 && edge.iter().any(|e| e.id as usize == id && e.act == Act::L);
            if !one_l(&sol.open) || !one_l(&sol.close) {
                return false;
            }
        }
        let front = |edge: &[E]| -> Vec<E> {
            let mut v: Vec<E> = edge.iter().copied().filter(|e| ids.contains(&(e.id as usize))).collect();
            v.extend(edge.iter().copied().filter(|e| !ids.contains(&(e.id as usize))));
            v
        };
        let mut c = rest.clone();
        for (i, edge) in [front(&sol.open), front(&sol.close)].iter().enumerate() {
            for e in edge {
                match m.apply(&c, *e) {
                    Out::Ok(n) => c = n,
                    _ => return false,
                }
            }
            if (i == 0 && !m.is_open(&c)) || (i == 1 && c != *rest) {
                return false;
            }
        }
        true
    };
    let id_at = |x: i8, y: i8| (0..m.info.len()).find(|&id| bx.xy(rest.pp[id] as usize) == (x, y));
    let still = |id: usize| states.iter().all(|s| s.pp[id] == rest.pp[id]);
    let near = |a: (i8, i8), b: (i8, i8)| (a.0 - b.0).abs() + (a.1 - b.1).abs() == 1;
    let mut covered = 0usize;
    for (id, p) in m.pid.iter().map(|&pi| m.pieces[pi]).enumerate() {
        let K::Piston { f, .. } = p.k else { continue };
        let head = F[f as usize];
        let lever = (p.x - head.0, p.y - head.1);
        if bx.has(lever.0, lever.1) || !still(id) {
            continue;
        }
        // every piston the lever powers at some state (beside the lever cell, or by QC: its above
        // cell touches it) must be lever-held: never moved, one level toggle per edge at the flip
        // (w86: a lever at (0,5) also arms (-1,4) and (1,4), which broke t1-r22 in the game)
        let powered = |q: (i8, i8)| near(q, lever) || near((q.0, q.1 + 1), lever);
        let affected: Vec<usize> = (0..m.info.len()).filter(|&j| states.iter().any(|s| powered(bx.xy(s.pp[j] as usize)))).collect();
        if affected.iter().any(|&j| !still(j)) || !front_ok(&affected) {
            continue;
        }
        let held = list.iter().filter(|&&(x, y)| id_at(x, y).is_some_and(|j| affected.contains(&j))).count();
        covered = covered.max(held);
    }
    (list, covered)
}

pub(crate) fn wiring(m: &Mech, rest: &St, sol: &Sol) -> Wire {
    let bx = m.bx;
    let n = bx.n();
    // the two runs of the train (or open then close)
    let (seq_o, seq_c) = match &sol.sym {
        Some(s) => (s.clone(), s.clone()),
        None => (sol.open.clone(), sol.close.clone()),
    };
    let mut o = rest.clone();
    for e in &sol.open {
        if let Out::Ok(nx) = m.apply(&o, *e) {
            o = nx;
        }
    }
    if let Some(s) = &sol.sym {
        // the train's open state
        let mut c = rest.clone();
        for e in s {
            if let Out::Ok(nx) = m.apply(&c, *e) {
                c = nx;
            }
        }
        o = c;
    }
    let mut swept = vec![false; n];
    // per-edge change counts of every cell (observer pulses, approximately)
    let mut cnt: [Vec<u32>; 2] = [vec![0; n], vec![0; n]];
    // firing positions and pulse counts per piston per edge
    let np = m.info.len();
    let mut fire_pos: Vec<HashSet<usize>> = vec![HashSet::new(); np];
    // receivers = (piston, the cell it fires from): pulse counts per edge, level use
    let mut recv: BTreeMap<(usize, usize), ([u32; 2], bool)> = BTreeMap::new();
    let mut need: [Vec<u32>; 2] = [vec![0; np], vec![0; np]];
    let mut levels = vec![false; np];
    for (edge, (start, seq)) in [(rest, &seq_o), (&o, &seq_c)].into_iter().enumerate() {
        let mut c = start.clone();
        for i in 0..n {
            if c.g[i] != AIR {
                swept[i] = true;
            }
        }
        for e in seq {
            let id = e.id as usize;
            let p = c.pp[id] as usize;
            fire_pos[id].insert(p);
            let r = recv.entry((id, p)).or_insert(([0, 0], false));
            if e.act == Act::P {
                need[edge][id] += 1;
                r.0[edge] += 1;
            } else {
                levels[id] = true;
                r.1 = true;
            }
            let Out::Ok(nx) = m.apply(&c, *e) else { continue };
            for i in 0..n {
                if nx.g[i] != c.g[i] {
                    cnt[edge][i] += 1;
                }
                if nx.g[i] != AIR {
                    swept[i] = true;
                }
            }
            if e.act == Act::P {
                // the head goes out and back inside one pulse: body and head cell change twice
                let (_, f) = m.info[id];
                cnt[edge][p] += 2;
                if let Some(h) = m.step(p, f) {
                    swept[h] = true;
                    cnt[edge][h] += 1;
                }
            } else {
                cnt[edge][p] += 1;
            }
            c = nx;
        }
    }
    // forbidden: the 3x3, the ring, every swept cell, the cells behind holes when open
    let mut free = vec![true; n];
    for i in 0..n {
        let (x, y) = bx.xy(i);
        if in_t(x, y) || in_r(x, y) || swept[i] {
            free[i] = false;
        }
    }
    for (x, y) in ring() {
        if !bx.has(x, y) {
            continue;
        }
        let i = bx.idx(x, y);
        if swept[i] && o.g[i] == AIR {
            let (ox, oy) = outward(x, y);
            if bx.has(ox, oy) {
                free[bx.idx(ox, oy)] = false;
            }
        }
    }
    // regions (4-connected)
    let mut reg = vec![usize::MAX; n];
    let mut sizes = Vec::new();
    for s in 0..n {
        if !free[s] || reg[s] != usize::MAX {
            continue;
        }
        let r = sizes.len();
        let mut q = vec![s];
        reg[s] = r;
        let mut k = 0;
        while let Some(i) = q.pop() {
            k += 1;
            for f in 0..4u8 {
                if let Some(j) = m.step(i, f) {
                    if free[j] && reg[j] == usize::MAX {
                        reg[j] = r;
                        q.push(j);
                    }
                }
            }
        }
        sizes.push(k);
    }
    let nr = sizes.len();
    // feed regions of each receiver (a piston at one firing cell): free cells next to it (not
    // its head side) or next to the cell above it (quasi-connectivity)
    let keys: Vec<(usize, usize)> = recv.keys().copied().collect();
    let mut feeds: Vec<HashSet<usize>> = vec![HashSet::new(); keys.len()];
    let riders = (0..np).filter(|&id| fire_pos[id].len() > 1).count();
    for (k, &(id, p)) in keys.iter().enumerate() {
        let (_, f) = m.info[id];
        for d in 0..4u8 {
            if d == f {
                continue;
            }
            if let Some(j) = m.step(p, d) {
                if free[j] {
                    feeds[k].insert(reg[j]);
                }
            }
        }
        if let Some(a) = m.step(p, 2) {
            if free[a] {
                feeds[k].insert(reg[a]);
            }
            for d in 0..4u8 {
                if let Some(j) = m.step(a, d) {
                    if j != p && free[j] {
                        feeds[k].insert(reg[j]);
                    }
                }
            }
        }
    }
    let _ = (&need, &levels);
    let used: Vec<usize> = (0..keys.len()).collect();
    let mut why = Vec::new();
    for &k in &used {
        if feeds[k].is_empty() {
            let (id, p) = keys[k];
            let (x, y) = bx.xy(p);
            why.push(format!("{} at ({x},{y}) has no free cell to feed it", m.pieces[m.pid[id]].code()));
        }
    }
    let mut sorted = sizes.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let weak = used.iter().filter(|&&k| feeds[k].iter().map(|&r| sizes[r]).max().unwrap_or(0) < 3).count();
    // pocket load (pockets3's POCKET LOAD, macro version): per free region, the actions of the
    // receivers only that region can feed (the larger edge; a level use counts 1), per cell;
    // the worst region, x10
    let load = {
        let mut per = vec![0u32; nr];
        for &k in &used {
            if feeds[k].len() == 1 {
                let r = *feeds[k].iter().next().unwrap();
                let (c, lv) = recv[&keys[k]];
                per[r] += c[0].max(c[1]) + u32::from(lv);
            }
        }
        (0..nr).map(|r| per[r] as usize * 10 / sizes[r].max(1)).max().unwrap_or(0)
    };
    if !why.is_empty() {
        return Wire { class: 'X', regions: sorted, forced: 0, riders, weak, load, why: why.join("; ") };
    }
    // one region feeding everything?
    if (0..nr).any(|r| used.iter().all(|&k| feeds[k].contains(&r))) {
        return Wire { class: 'A', regions: sorted, forced: 1, riders, weak, load, why: "one region feeds every piston".into() };
    }
    // forced regions: the only region of some receiver
    let mut forced: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for &k in &used {
        if feeds[k].len() == 1 {
            forced.entry(*feeds[k].iter().next().unwrap()).or_default().push(k);
        }
    }
    // links into a region: changing cells next to it, with their per-edge counts
    let links = |r: usize| -> Vec<(u32, u32)> {
        let mut v = Vec::new();
        for i in 0..n {
            if free[i] || (cnt[0][i] == 0 && cnt[1][i] == 0) {
                continue;
            }
            if (0..4u8).any(|d| m.step(i, d).is_some_and(|j| free[j] && reg[j] == r)) {
                v.push((cnt[0][i], cnt[1][i]));
            }
        }
        v
    };
    let mut best: Option<(char, String)> = None;
    let cands: Vec<usize> = if forced.is_empty() { (0..nr).collect() } else { forced.keys().copied().collect() };
    for &lever in &cands {
        let mut class = 'B';
        let mut notes = Vec::new();
        for (&r, ids) in &forced {
            if r == lever {
                continue;
            }
            let l = links(r);
            if l.is_empty() {
                class = 'X';
                notes.push(format!("region {r} ({} cells) has no link", sizes[r]));
                continue;
            }
            let odd_needed = ids.iter().any(|&k| {
                let (nd, _) = recv[&keys[k]];
                nd[0] % 2 == 1 || nd[1] % 2 == 1
            });
            let odd_link = l.iter().any(|&(a, b)| a % 2 == 1 && b % 2 == 1);
            if odd_needed && !odd_link && class != 'X' {
                class = 'C';
                notes.push(format!("region {r} ({} cells) needs an odd count, its links are even", sizes[r]));
            }
        }
        let rank = |c: char| match c {
            'B' => 0,
            'C' => 1,
            _ => 2,
        };
        if best.as_ref().is_none_or(|(c, _)| rank(class) < rank(*c)) {
            best = Some((class, format!("lever in region {lever}; {} forced regions{}{}", forced.len(), if notes.is_empty() { "" } else { ": " }, notes.join("; "))));
        }
    }
    let (class, w) = best.unwrap_or(('X', "no region".into()));
    Wire { class, regions: sorted, forced: forced.len(), riders, weak, load, why: w }
}

fn wireability(m: &Mech, rest: &St, sol: &Sol) -> (usize, usize) {
    // cells of the box never used by the mechanism over the cycle
    let mut used = vec![false; m.bx.n()];
    let mut c = rest.clone();
    let mut mark = |s: &St| {
        for i in 0..m.bx.n() {
            if s.g[i] != AIR {
                used[i] = true;
            }
        }
    };
    mark(&c);
    for e in sol.open.iter().chain(sol.close.iter()) {
        if let Out::Ok(n) = m.apply(&c, *e) {
            c = n;
        }
        mark(&c);
    }
    let free = used.iter().filter(|u| !**u).count();
    // pistons whose quasi cell (above) is a ring cell or the 3x3 (hard to power by QC)
    let rq = m
        .pieces
        .iter()
        .filter(|p| matches!(p.k, K::Piston { .. }) && (in_r(p.x, p.y + 1) || in_t(p.x, p.y + 1)))
        .count();
    (free, rq)
}

/// Convert to mech-synth's world (for ideal plans and simulator replays).
pub fn to_world(m: &Mech) -> (World, Vec<u8>) {
    let mut b: Vec<([i8; 3], Kind)> = Vec::new();
    for x in 0..3 {
        for y in 0..3 {
            b.push(([x, y, 0], Kind::Door));
        }
    }
    for p in &m.pieces {
        let k = match p.k {
            K::Stone => Kind::Stone,
            K::Obsidian => Kind::Obsidian,
            K::Redstone => Kind::Stone,
            K::Piston { sticky, f, ext } => Kind::Piston { sticky, f, ext },
        };
        b.push(([p.x, p.y, 0], k));
    }
    b.sort();
    let pow = World::rest_pow(&b);
    let w = World { b, pow };
    // piston id -> index in w.b
    let map: Vec<u8> = m
        .pid
        .iter()
        .map(|&pi| {
            let p = m.pieces[pi];
            w.b.iter().position(|(q, k)| *q == [p.x, p.y, 0] && matches!(k, Kind::Piston { .. })).unwrap() as u8
        })
        .collect();
    (w, map)
}

/// The ideal-input plans of a solution, QC coupling spelled out PER EDGE (the coupling depends
/// on where the pistons stand, so a train's two runs can differ): ("sym", the train's two runs)
/// when there is a train, and ("oc", the open and close paths). Each as a mech-synth open/close
/// solution (ideal_plan with sym = false).
pub fn ideal_solutions(m: &Mech, rest: &St, sol: &Sol, map: &[u8]) -> Vec<(&'static str, mechsynth::Solution)> {
    let run = |s: &St, seq: &[E]| -> St {
        let mut c = s.clone();
        for e in seq {
            if let Out::Ok(n) = m.apply(&c, *e) {
                c = n;
            }
        }
        c
    };
    let mut out = Vec::new();
    if let Some(t) = &sol.sym {
        let o = run(rest, t);
        out.push(("sym", mechsynth::Solution { open: ms_seq(map, &m.expand(rest, t)), close: ms_seq(map, &m.expand(&o, t)), sym: None, states: 0 }));
    }
    let o = run(rest, &sol.open);
    out.push(("oc", mechsynth::Solution { open: ms_seq(map, &m.expand(rest, &sol.open)), close: ms_seq(map, &m.expand(&o, &sol.close)), sym: None, states: 0 }));
    out
}

/// The solution with its quasi-connectivity coupling spelled out ([`Mech::expand`]): the open
/// path from rest, the close path from the open state, the train from rest.
pub fn expanded_sol(m: &Mech, rest: &St, sol: &Sol) -> Sol {
    let mut o = rest.clone();
    for e in &sol.open {
        if let Out::Ok(n) = m.apply(&o, *e) {
            o = n;
        }
    }
    let mut x = sol.clone();
    x.open = m.expand(rest, &sol.open);
    x.close = m.expand(&o, &sol.close);
    x.sym = sol.sym.as_ref().map(|s| m.expand(rest, s));
    x
}

/// Track box56's `door topology` verdict for a solved mechanism in its box.
pub fn topology_of(m: &Mech, sol: &Sol) -> String {
    let (wld, map) = to_world(m);
    let rest = Mech::new(m.bx, &m.pieces).map(|(_, r)| r);
    let sol = match &rest {
        Some(r) => expanded_sol(m, r, sol),
        None => sol.clone(),
    };
    let msol = mechsynth::Solution { open: ms_seq(&map, &sol.open), close: ms_seq(&map, &sol.close), sym: sol.sym.as_ref().map(|s| ms_seq(&map, s)), states: 0 };
    let bx = m.bx;
    let t = crate::topology::analyze(&wld, &msol, [bx.x0, bx.y0], [bx.x0 + bx.w - 1, bx.y0 + bx.h - 1]);
    (if t.strict { "strict" } else if t.permissive { "permissive" } else if t.memory { "memory" } else { "no" }).to_string()
}

pub fn ms_seq(map: &[u8], s: &[E]) -> Vec<Ev> {
    s.iter().map(|e| Ev { id: map[e.id as usize], act: e.act }).collect()
}

pub struct Opts {
    pub boxes: Vec<(i8, i8)>,
    pub pmax: usize,
    pub cap: usize,
    pub sym_cap: usize,
    pub threads: usize,
    pub lifts: bool,
    pub pushes: bool,
    pub out_dir: std::path::PathBuf,
    pub verify: bool,
    pub workers: usize,
    pub extra: u32,
    pub gap: u32,
    pub limit: usize,
    /// Non-core perimeter blocks that may take a 2-move path.
    pub two: usize,
    /// General carrying depth (0 = only the classic elevator lifts).
    pub carry: usize,
    /// Variants of middle-row side blocks forced to leave vertically (`--vmid`).
    pub vmid: Vec<Vec<(i8, i8)>>,
    /// Also search with level events (A*) when the pulse model fails.
    pub levels: bool,
}

/// `--check <design>`: the mechanism of a design (wiring dropped) solved in its own bounding
/// box grown by `margin`: open/close schedules, symmetric train, extent.
/// `--check <design> --verify`: write the checked mechanism's ideal-input plans (separate open/close
/// schedules, and the symmetric train when there is one) to `out_dir/ideal/` and replay each in the
/// simulator (`door ideal` format: the files can be edited and re-run by hand).
pub struct CheckVerify {
    pub version: String,
    pub out_dir: std::path::PathBuf,
    pub gap: u32,
    pub extra: u32,
    pub name: String,
}

fn verify_one(v: &CheckVerify, m: &Mech, sol: &Sol, sym: Option<&Vec<E>>) -> Result<()> {
    let cfg = mctas::sim::WorkerConfig { version: v.version.clone(), heap_mb: 768, ..Default::default() };
    let pool = mctas::sim::Pool::spawn(1, &cfg)?;
    let mut wk = pool.worker(0);
    let (w, map) = to_world(m);
    let msol = mechsynth::Solution { open: ms_seq(&map, &sol.open), close: ms_seq(&map, &sol.close), sym: sym.map(|s| ms_seq(&map, s)), states: 0 };
    let dir = v.out_dir.join("ideal");
    std::fs::create_dir_all(&dir)?;
    for use_sym in [false, true] {
        if use_sym && sym.is_none() {
            continue;
        }
        let tag = if use_sym { "sym" } else { "oc" };
        let name = format!("{}-{tag}", v.name);
        let plan = mechsynth::ideal_plan(&w, &msol, use_sym, v.gap, &name)?;
        let mut dd = plan.design.clone();
        dd.name = name.clone();
        let dpath = dir.join(format!("{name}.txt"));
        dd.save(&dpath)?;
        let text = plan.text.replace(&format!("design {name}.txt"), &format!("design {}", dpath.display()));
        let ipath = dir.join(format!("{name}.ideal"));
        std::fs::write(&ipath, text)?;
        match mechsynth::verify_plan(&mut wk, &plan, v.extra) {
            Ok(()) => println!("  {tag}: WORKS with ideal inputs ({} + {})", dpath.display(), ipath.display()),
            Err(e) => println!("  {tag}: BROKEN with ideal inputs: {e:#} ({})", ipath.display()),
        }
    }
    Ok(())
}

pub fn check(path: &std::path::Path, margin: i8, cap: usize, sym_cap: usize, verify: Option<CheckVerify>) -> Result<()> {
    let d = crate::cli::load_design(path)?;
    let w = mechsynth::world_of_design(&d)?;
    let mut pieces = Vec::new();
    for (p, k) in &w.b {
        if p[2] != 0 {
            anyhow::bail!("not flat: block at {p:?}");
        }
        let k = match k {
            Kind::Door => continue,
            // plain blocks of a wired design are its wiring (floors, droppers, lamps): dropped
            Kind::Stone | Kind::Obsidian => continue,
            Kind::Piston { sticky, f, ext } => K::Piston { sticky: *sticky, f: *f, ext: *ext },
            k => anyhow::bail!("unsupported block {k:?}"),
        };
        pieces.push(Pc { x: p[0], y: p[1], k });
    }
    let (mut x0, mut y0, mut x1, mut y1) = (0i8, 0i8, 2i8, 2i8);
    for p in &pieces {
        for (x, y) in p.cells() {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    let bx = Bx { x0: x0 - margin, y0: y0 - margin, w: x1 - x0 + 1 + 2 * margin, h: y1 - y0 + 1 + 2 * margin };
    // FAMILIES_ROW: solve door row 0 alone (a strip row mechanism)
    let (bx, pieces) = if std::env::var("FAMILIES_ROW").is_ok() {
        (Bx { x0: x0 - margin, y0: 0, w: x1 - x0 + 1 + 2 * margin, h: 1 }, pieces.into_iter().filter(|p| p.y == 0).collect::<Vec<_>>())
    } else {
        (bx, pieces)
    };
    let (m, rest) = Mech::new(bx, &pieces).context("pieces overlap")?;
    println!("{} pieces: {}", m.pieces.len(), m.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "));
    println!("box {}x{} (x {}..{}, y {}..{}), rest ring clean: {}", bx.w, bx.h, bx.x0, bx.x0 + bx.w - 1, bx.y0, bx.y0 + bx.h - 1, m.rest_ring_clean(&rest));
    match solve(&m, &rest, cap, true) {
        Ok(Some(sol)) => {
            println!("open {} + close {} ({} states), extent x {}..{} y {}..{} = {}x{}", sol.open.len(), sol.close.len(), sol.states, sol.extent.0, sol.extent.2, sol.extent.1, sol.extent.3, sol.extent.2 - sol.extent.0 + 1, sol.extent.3 - sol.extent.1 + 1);
            println!("  open:  {}\n  close: {}", m.fmt_seq(&sol.open), m.fmt_seq(&sol.close));
            if std::env::var("FAMILIES_TRACE").is_ok() {
                let mut c = rest.clone();
                println!("{}", m.render(&c));
                for e in sol.open.iter().chain(sol.close.iter()) {
                    if let Out::Ok(n) = m.apply(&c, *e) {
                        c = n;
                    }
                    println!("after {}:\n{}", m.fmt_ev(*e), m.render(&c));
                }
            }
            let mut sol = sol;
            match best_sym(&m, &rest, &sol, sym_cap, 6) {
                Some((s, lv)) => {
                    println!("  sym {}{}: {}", s.len(), if lv { " (levels)" } else { " (pulses only)" }, m.fmt_seq(&s));
                    if let Some(v) = &verify {
                        verify_one(v, &m, &sol, Some(&s))?;
                    }
                    sol.sym = Some(s);
                    sol.sym_levels = lv;
                }
                None => {
                    println!("  no symmetric train");
                    if let Some(v) = &verify {
                        verify_one(v, &m, &sol, None)?;
                    }
                }
            }
            let w = wiring(&m, &rest, &sol);
            println!("  wire {} regions {:?} forced {} riders {}: {}", w.class, w.regions, w.forced, w.riders, w.why);
            println!("  topology: {}", topology_of(&m, &sol));
            let (wld, map) = to_world(&m);
            let xs = expanded_sol(&m, &rest, &sol);
            if let Some(s) = &xs.sym {
                println!("  sym with the QC coupling spelled out ({}): {}", s.len(), m.fmt_seq(s));
            }
            for (tag, msol) in ideal_solutions(&m, &rest, &sol, &map) {
                match mechsynth::ideal_plan(&wld, &msol, false, 6, "check") {
                    Ok(p) => {
                        println!("  ideal plan ({tag}): {} input events", p.events.len());
                        if let Ok(dir) = std::env::var("FAMILIES_IDEAL_OUT") {
                            let dir = std::path::PathBuf::from(dir);
                            std::fs::create_dir_all(&dir)?;
                            let dpath = dir.join(format!("check-{tag}.txt"));
                            let mut dd = p.design.clone();
                            dd.name = format!("check-{tag}");
                            dd.save(&dpath)?;
                            std::fs::write(dir.join(format!("check-{tag}.ideal")), p.text.replace("design check.txt", &format!("design {}", dpath.display())))?;
                        }
                    }
                    Err(e) => println!("  ideal plan ({tag}): FAILS: {e:#}"),
                }
            }
        }
        Ok(None) => println!("does not open (FULL seamless) and close in the macro model"),
        Err(()) => println!("state cap hit"),
    }
    Ok(())
}

pub fn parse_boxes(s: &str) -> Result<Vec<(i8, i8)>> {
    s.split(',')
        .map(|t| {
            let (a, b) = t.trim().split_once('x').context("box WxH")?;
            Ok((a.parse()?, b.parse()?))
        })
        .collect()
}

/// Every door offset of a W x H box (door coordinates of the box).
pub fn offsets(w: i8, h: i8) -> Vec<Bx> {
    let mut v = Vec::new();
    for dx in 0..=(w - 3) {
        for dy in 0..=(h - 3) {
            v.push(Bx { x0: -dx, y0: -dy, w, h });
        }
    }
    v
}

fn minimal(m_pieces: &[Pc], bx: Bx, cap: usize, levels: bool) -> Option<Vec<Pc>> {
    // drop pieces greedily while the rest still opens and closes
    let mut cur = m_pieces.to_vec();
    loop {
        let mut dropped = false;
        for i in 0..cur.len() {
            let mut t = cur.clone();
            t.remove(i);
            let Some((m, rest)) = Mech::new(bx, &t) else { continue };
            if let Ok(Some(_)) = solve(&m, &rest, cap, levels) {
                cur = t;
                dropped = true;
                break;
            }
        }
        if !dropped {
            return Some(cur);
        }
    }
}

pub fn run(version: &str, o: &Opts) -> Result<()> {
    let log = |s: &str| eprintln!("[families] {s}");
    std::fs::create_dir_all(&o.out_dir)?;
    let mut entries: BTreeMap<Vec<Pc>, Entry> = BTreeMap::new();
    let mut per_box: Vec<(i8, i8, usize, usize, usize, usize)> = Vec::new(); // (w, h, candidates, fits, new, unresolved)
    let mut boxes = o.boxes.clone();
    boxes.sort_by_key(|&(w, h)| ((w as i32) * (h as i32), w.max(h), w));
    for &(w, h) in &boxes {
        let t0 = std::time::Instant::now();
        let mut cands: Vec<(Bx, Vec<Pc>)> = Vec::new();
        let mut seen_c: HashSet<Vec<Pc>> = HashSet::new();
        for bx in offsets(w, h) {
            // mirror: only door offsets with dx <= mirrored dx (others are mirror images)
            let dx = -bx.x0;
            if dx > (w - 3) - dx {
                continue;
            }
            let g = generate_v(bx, o.pmax, o.lifts, o.pushes, o.two, o.carry, &o.vmid);
            for v in g {
                let c = canon(&v);
                if seen_c.insert(c) {
                    cands.push((bx, v));
                }
            }
        }
        let total = cands.len();
        if o.limit > 0 && cands.len() > o.limit {
            cands.truncate(o.limit);
        }
        log(&format!("{w}x{h}: {} candidate mechanisms ({total} generated, {:.1}s)", cands.len(), t0.elapsed().as_secs_f64()));
        let found: Mutex<Vec<(Bx, Vec<Pc>)>> = Mutex::new(Vec::new());
        let unfed_n = AtomicUsize::new(0);
        let capped = AtomicUsize::new(0);
        let capped_list: Mutex<Vec<usize>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        let done = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cands.len() {
                        break;
                    }
                    let (bx, v) = &cands[i];
                    // the full box (any offset) for solving
                    if let Some((m, rest)) = Mech::new(*bx, v) {
                        match solve(&m, &rest, o.cap, o.levels) {
                            Ok(Some(sol)) => {
                                if fed_ok(&m, &rest, &sol) {
                                    found.lock().unwrap().push((*bx, v.clone()));
                                } else {
                                    unfed_n.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Ok(None) => {}
                            Err(()) => {
                                capped.fetch_add(1, Ordering::Relaxed);
                                capped_list.lock().unwrap().push(i);
                            }
                        }
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    if d % 20000 == 0 {
                        eprintln!("[families]   {w}x{h}: {d}/{} solved, {} work, {:.0}s", cands.len(), found.lock().unwrap().len(), t0.elapsed().as_secs_f64());
                    }
                });
            }
        });
        // second pass on the candidates whose pulse graph hit the cap: goal-directed A* (pulse
        // model); what it does not solve stays UNRESOLVED (reported per box)
        let capped_list = capped_list.into_inner().unwrap();
        let unresolved = AtomicUsize::new(0);
        let unresolved_list: Mutex<Vec<String>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    if k >= capped_list.len() {
                        break;
                    }
                    let (bx, v) = &cands[capped_list[k]];
                    if let Some((m, rest)) = Mech::new(*bx, v) {
                        // (the A* pass knows no wirability bars: with them, capped stays unresolved)
                        match if cons_on() { Err(()) } else { solve_pulse_astar(&m, &rest, o.cap) } {
                            Ok(Some(sol)) => {
                                if fed_ok(&m, &rest, &sol) {
                                    found.lock().unwrap().push((*bx, v.clone()));
                                } else {
                                    unfed_n.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                            Ok(None) => {}
                            Err(()) => {
                                unresolved.fetch_add(1, Ordering::Relaxed);
                                unresolved_list.lock().unwrap().push(format!("{} {} {}", bx.x0, bx.y0, v.iter().map(|p| p.code()).collect::<Vec<_>>().join(" ")));
                            }
                        }
                    }
                });
            }
        });
        let unresolved = unresolved.load(Ordering::Relaxed);
        {
            let mut v = unresolved_list.into_inner().unwrap();
            v.sort();
            let dir = o.out_dir.join("unresolved");
            std::fs::create_dir_all(&dir)?;
            std::fs::write(dir.join(format!("{w}x{h}.txt")), v.join("\n") + "\n")?;
        }
        let capped = AtomicUsize::new(unresolved);
        let found = found.into_inner().unwrap();
        log(&format!(
            "{w}x{h}: {} open+close ({} candidates unresolved: pulse graph over the cap and A* found nothing{}), minimizing",
            found.len(),
            capped.load(Ordering::Relaxed),
            if FED.load(Ordering::Relaxed) { format!("; {} solved but an unfed receiver (--fed)", unfed_n.load(Ordering::Relaxed)) } else { String::new() }
        ));
        // minimal + canonical + every offset of this box that fits
        let mins: Mutex<HashSet<Vec<Pc>>> = Mutex::new(HashSet::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= found.len() {
                        break;
                    }
                    let (bx, v) = &found[i];
                    if let Some(mv) = minimal(v, *bx, o.cap, o.levels) {
                        mins.lock().unwrap().insert(canon(&mv));
                    }
                });
            }
        });
        let mins: Vec<Vec<Pc>> = {
            let mut v: Vec<Vec<Pc>> = mins.into_inner().unwrap().into_iter().collect();
            v.sort();
            v
        };
        // describe each minimal mechanism (in this box: first offset that works)
        let described: Mutex<Vec<Entry>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= mins.len() {
                        break;
                    }
                    let v = &mins[i];
                    for vv in [v.clone(), v.iter().map(|p| p.mirror()).collect::<Vec<_>>()] {
                        let mut got = None;
                        for bx in offsets(w, h) {
                            let Some((m, rest)) = Mech::new(bx, &vv) else { continue };
                            if let Ok(Some(mut sol)) = solve(&m, &rest, o.cap, o.levels) {
                                if let Some((s, lv)) = best_sym(&m, &rest, &sol, o.sym_cap, 6) {
                                    sol.sym = Some(s);
                                    sol.sym_levels = lv;
                                }
                                let (free, rq) = wireability(&m, &rest, &sol);
                                let np = n_pistons(&vv);
                                let sticky = vv.iter().filter(|p| matches!(p.k, K::Piston { sticky: true, .. })).count();
                                let wire = wiring(&m, &rest, &sol);
                                let topo = topology_of(&m, &sol);
                                got = Some(Entry { pieces: v.clone(), w, h, bx, sol, rest_clean: m.rest_ring_clean(&rest), pistons: np, sticky, free_cells: free, ring_quasi: rq, verified: None, orient: vv.clone(), wire, topo });
                                break;
                            }
                        }
                        if let Some(e) = got {
                            described.lock().unwrap().push(e);
                            break;
                        }
                    }
                });
            }
        });
        let described = described.into_inner().unwrap();
        let fits = described.len();
        let mut new = 0;
        for e in described {
            if !entries.contains_key(&e.pieces) {
                new += 1;
                entries.insert(e.pieces.clone(), e);
            }
        }
        per_box.push((w, h, cands.len(), fits, new, capped.load(Ordering::Relaxed)));
        log(&format!("{w}x{h}: {fits} minimal mechanisms fit ({new} new) in {:.0}s", t0.elapsed().as_secs_f64()));
        write_catalog(o, &entries, &per_box)?;
    }
    if o.verify {
        verify_entries(version, o, &mut entries)?;
        write_catalog(o, &entries, &per_box)?;
    }
    Ok(())
}

fn verify_entries(version: &str, o: &Opts, entries: &mut BTreeMap<Vec<Pc>, Entry>) -> Result<()> {
    let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
    let pool = mctas::sim::Pool::spawn(o.workers, &cfg)?;
    let keys: Vec<Vec<Pc>> = entries.keys().cloned().collect();
    let items: Vec<(usize, bool)> = keys.iter().enumerate().flat_map(|(i, k)| {
        let mut v = vec![(i, false)];
        if entries[k].sol.sym.is_some() {
            v.push((i, true));
        }
        v
    }).collect();
    let ents: Vec<Entry> = keys.iter().map(|k| entries[k].clone()).collect();
    let res = pool.map(items.clone(), |wk, _, (i, sym)| {
        let e = &ents[i];
        let (m, mrest) = Mech::new(e.bx, &e.orient).context("rebuild")?;
        let (w, map) = to_world(&m);
        let want = if sym { "sym" } else { "oc" };
        let sol = ideal_solutions(&m, &mrest, &e.sol, &map).into_iter().find(|(t, _)| *t == want).map(|(_, s)| s).context("no such schedule")?;
        let name = format!("{}x{}-{:04}", e.w, e.h, i);
        let plan = mechsynth::ideal_plan(&w, &sol, false, o.gap, &name)?;
        Ok(mechsynth::verify_plan(wk, &plan, o.extra).map_err(|e| format!("{e:#}")))
    });
    let mut verdicts: HashMap<usize, Vec<String>> = HashMap::new();
    for ((i, sym), r) in items.iter().zip(res) {
        let tag = if *sym { "sym" } else { "oc" };
        let v = match r {
            Ok(Ok(())) => format!("{tag} WORKS"),
            Ok(Err(e)) => format!("{tag} BROKEN ({})", e.chars().take(80).collect::<String>().replace('\t', " ").replace('|', "/")),
            Err(e) => format!("{tag} ERROR ({})", format!("{e:#}").chars().take(80).collect::<String>().replace('|', "/")),
        };
        verdicts.entry(*i).or_default().push(v);
    }
    for (i, k) in keys.iter().enumerate() {
        if let Some(v) = verdicts.get(&i) {
            entries.get_mut(k).unwrap().verified = Some(v.join("; "));
        }
    }
    Ok(())
}

fn write_catalog(o: &Opts, entries: &BTreeMap<Vec<Pc>, Entry>, per_box: &[(i8, i8, usize, usize, usize, usize)]) -> Result<()> {
    let mut list: Vec<&Entry> = entries.values().collect();
    let wrank = |c: char| match c {
        'A' => 0,
        'B' => 1,
        'C' => 2,
        _ => 3,
    };
    let trank = |t: &str| match t {
        "strict" => 0,
        "permissive" => 1,
        _ => 2,
    };
    list.sort_by_key(|e| ((e.w as i32) * (e.h as i32), e.w.max(e.h), e.w, trank(&e.topo), wrank(e.wire.class), e.wire.forced, std::cmp::Reverse(e.wire.regions.first().copied().unwrap_or(0)), e.pistons, e.sol.sym.as_ref().map_or(99, |s| s.len()), e.sol.open.len() + e.sol.close.len()));
    let mut md = String::new();
    md.push_str("# Flat (1-wide) seamless 3x3 door mechanisms: the catalog per box\n\n");
    md.push_str("Generated by `mctas door families` (crates/piston-door/src/families.rs). Do not edit by hand.\n\n");
    md.push_str(&format!(
        "Bounds: every block in the door plane; pistons sticky or normal facing E/W/U/D (retracted or extended at rest), at most **{} pistons**, no helpers, no slime/honey; every door block leaves the 3x3 in 1-2 unit moves (the center in exactly 2), each move a pull by a sticky piston 2 cells ahead or a push by a piston 1-2 cells behind, a piston reused from its rest cell or lifted 1 cell along its axis by one lift piston (the elevator){}{}. Each candidate is solved EXACTLY in the macro piston model (whole state graph, cap {} states): FULL seamless open state (3x3 empty, only door blocks / plain blocks in the 12 ring cells, nothing but air or a door block behind a swept ring cell that is empty when open), closing back to the exact rest state, everything inside the box at every state. Only MINIMAL mechanisms (no piston can be dropped), left-right mirror images merged (up/down is not a symmetry: quasi-connectivity and gravity). The candidates the pulse graph (cap) and A* (cap) could not decide are counted as UNRESOLVED per box and listed in `unresolved/WxH.txt`.\n\n**Read before wiring:** (1) the macro model ignores quasi-connectivity and redstone: only entries whose `sim` column says WORKS replay with ideal inputs in the simulator (8x8-119's family fails: its elevator piston, pulled back above BL's puller, QC-fires it). (2) RIDER SHUFFLES (a piston dragged sideways to serve two lines, sometimes firing from inside the 3x3; schedules ~20 / 46 events; `wire` X with riders >= 2) are kept for completeness but are not wiring targets, even where `topology` says strict.\n\n",
        o.pmax,
        if o.lifts { "" } else { " (lifts off)" },
        if o.pushes { "" } else { " (pushes off)" },
        o.cap
    ));
    md.push_str("Columns: box (W x H, door offset = columns left of / rows below the door), pistons (sticky), rest-ring = no piston/head in the ring at rest (the strict reading), topology = track box56's `door topology` (crates/piston-door/src/topology.rs) on the entry in its box, a HEURISTIC RANKING, NOT A PROOF (its link model does not yet over-approximate every way two regions can talk: trapdoor crossings, a lever outside the box, QC feeds, memory links of other sizes; and its parity rule is not even a valid heuristic for timing-flexible links: an observer ignores changes while powered or with a tick pending, so a piston firing twice exactly 6 gt apart gives its watcher 3 raw pulses, and a door block's parking cell can show 3 changes per edge: wire56 wired 3 of 7x8-013's (v1 028) 4 regions that way, NOTES section 5 on explore/wire56): `strict` = wirable with links that carry their exact pulse parity (memoryless observers + pulse mergers); `permissive` = also when 3+ events may chain-merge into an odd count; `memory` = also with MEMORY links (two waxed copper bulb T flip-flops + comparators + an observer, wire56's memory-link gadget, needing >= 7 free cells in the receiving region, turn an even pulse count into one 2 gt pulse per edge); `no` = none of these (link types not modeled: piston/note-block latches reset on the other edge, sculk, entities), sym = one pulse train for both lever edges (P = 2 gt pulse, L = level toggle; `+L` = needs levels), free = box cells the mechanism never touches (room for wiring), rq = pistons whose quasi-connectivity cell (above) is in the ring or the 3x3, wire = wiring feasibility (track box56's quadrant + parity argument, automated: free cells minus the ring, the swept cells and the cells behind open holes split into REGIONS; a piston is fed from a region touching it or the cell above it (QC) at every place it fires; a region with pistons only it can feed but no lever needs a LINK: a changing mechanism cell next to it an observer can watch, whose pulse count per lever edge must be odd where a piston there needs an odd count. Class A = one region feeds every piston; B = links needed, each with the right parity; C = some region only gets even links (needs a pulse merger, designs/gadgets/pulse-merge on explore/flat-seam); X = a piston no free cell feeds, or a region with no link), sim = `door ideal` replay with ideal inputs (oc = separate open/close schedules, sym = the shared train). Within a box, rows are ranked by wire class, forced regions, largest region, pistons, train length.\n\n");
    md.push_str("| box | candidates | minimal mechanisms that fit | first fitting here | unresolved candidates |\n|---|---|---|---|---|\n");
    for (w, h, c, f, n, unres) in per_box {
        md.push_str(&format!("| {w}x{h} | {c} | {f} | {n} | {} |\n", if *unres == 0 { "0 (exhaustive within the bounds)".to_string() } else { format!("{unres} (pulse graph over the cap, A* found nothing)") }));
    }
    md.push('\n');
    let mut tsv = String::from("id\tbox\tdoor_offset\tpistons\tsticky\trest_ring_clean\tsym_len\tsym_levels\topen_len\tclose_len\tfree_cells\tring_quasi\twire_class\tregions\tforced_regions\triders\twire_why\textent\tpieces\topen\tclose\tsym\tsim\tfiles\ttopology\n");
    let mut cur_box = (0, 0);
    for (i, e) in list.iter().enumerate() {
        if (e.w, e.h) != cur_box {
            cur_box = (e.w, e.h);
            md.push_str(&format!("\n## {}x{} (volume {})\n\n| id | offset | pistons | rest-ring | topology | wire | sym | open/close | free | rq | pieces | sim |\n|---|---|---|---|---|---|---|---|---|---|---|---|\n", e.w, e.h, e.w as i32 * e.h as i32));
        }
        let (m, mrest) = Mech::new(e.bx, &e.orient).unwrap();
        // sequences as published: the QC coupling spelled out (the coupled piston's own event
        // follows the event that fires it)
        let xs = expanded_sol(&m, &mrest, &e.sol);
        let sym = xs.sym.as_ref().map(|s| format!("{}{}: {}", s.len(), if e.sol.sym_levels { "+L" } else { "" }, m.fmt_seq(s))).unwrap_or("-".into());
        let pieces = m.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" ");
        let id = format!("{}x{}-{:03}", e.w, e.h, i);
        let wire = format!("{} regions {:?}{}: {}", e.wire.class, e.wire.regions, if e.wire.riders > 0 { format!(", {} riders", e.wire.riders) } else { String::new() }, e.wire.why);
        // machine-readable files: the mechanism (door + pistons, no wiring) and its ideal
        // schedules (`door ideal` format: redstone blocks set/cleared next to each piston)
        let mdir = o.out_dir.join("mech");
        std::fs::create_dir_all(&mdir)?;
        let (wld, map) = to_world(&m);
        let md_design = mechsynth::design_of_world(&wld, &id);
        md_design.save(&mdir.join(format!("{id}.txt")))?;
        let mut files = vec![format!("mech/{id}.txt")];
        for (tag, msol) in ideal_solutions(&m, &mrest, &e.sol, &map) {
            match mechsynth::ideal_plan(&wld, &msol, false, o.gap, &id) {
                Ok(plan) => {
                    let mut dd = plan.design.clone();
                    dd.name = format!("{id}-{tag}");
                    let dpath = mdir.join(format!("{id}-{tag}.txt"));
                    dd.save(&dpath)?;
                    let text = plan.text.replace(&format!("design {id}.txt"), &format!("design {}", dpath.display()));
                    std::fs::write(mdir.join(format!("{id}-{tag}.ideal")), text)?;
                    files.push(format!("mech/{id}-{tag}.ideal"));
                }
                Err(err) => files.push(format!("(no {tag} ideal plan: {})", format!("{err:#}").replace(['\t', '|'], " "))),
            }
        }
        let files = files.join(" ");
        let topo = e.topo.as_str();
        md.push_str(&format!(
            "| {id} | {},{} | {} ({}) | {} | {topo} | {} | {} | {}/{} | {} | {} | `{}` | {} |\n",
            -e.bx.x0,
            -e.bx.y0,
            e.pistons,
            e.sticky,
            if e.rest_clean { "clean" } else { "piston" },
            wire,
            sym,
            e.sol.open.len(),
            e.sol.close.len(),
            e.free_cells,
            e.ring_quasi,
            pieces,
            e.verified.clone().unwrap_or("-".into())
        ));
        tsv.push_str(&format!(
            "{id}\t{}x{}\t{},{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{files}\t{topo}\n",
            e.w,
            e.h,
            -e.bx.x0,
            -e.bx.y0,
            e.pistons,
            e.sticky,
            e.rest_clean,
            e.sol.sym.as_ref().map_or(0, |s| s.len()),
            e.sol.sym_levels,
            e.sol.open.len(),
            e.sol.close.len(),
            e.free_cells,
            e.ring_quasi,
            e.wire.class,
            e.wire.regions,
            e.wire.forced,
            e.wire.riders,
            e.wire.why,
            e.sol.extent,
            pieces,
            m.fmt_seq(&xs.open),
            m.fmt_seq(&xs.close),
            xs.sym.as_ref().map(|s| m.fmt_seq(s)).unwrap_or_default(),
            e.verified.clone().unwrap_or_default()
        ));
    }
    std::fs::write(o.out_dir.join("CATALOG.md"), md)?;
    std::fs::write(o.out_dir.join("CATALOG.tsv"), tsv)?;
    Ok(())
}

// ---------------------------------------------------------------- strips: rows and columns

/// One row mechanism: horizontal pistons in a door row, the row's left arm `a` cells and right
/// arm `b` cells (row box x in -a..2+b).
#[derive(Clone, Debug)]
pub struct RowMech {
    pub a: i8,
    pub b: i8,
    pub pieces: Vec<Pc>,
    pub sol: Sol,
}

/// Every placement of <= kmax horizontal pistons in the arms of a row (exhaustive). Pistons
/// facing out of the row box from its last cell are skipped (they can never extend).
pub fn row_candidates(a: i8, b: i8, kmax: usize) -> Vec<Vec<Pc>> {
    let cells: Vec<i8> = (-a..0).chain(3..3 + b).collect();
    let lo = -a;
    let hi = 2 + b;
    let mut out = Vec::new();
    fn rec(i: usize, cells: &[i8], lo: i8, hi: i8, kmax: usize, cur: &mut Vec<Pc>, occ: &mut HashSet<i8>, out: &mut Vec<Vec<Pc>>) {
        if i == cells.len() {
            out.push(cur.clone());
            return;
        }
        let x = cells[i];
        rec(i + 1, cells, lo, hi, kmax, cur, occ, out);
        if occ.contains(&x) {
            return;
        }
        let nr = cur.iter().filter(|p| matches!(p.k, K::Redstone)).count();
        if nr < RMAX.get().copied().unwrap_or(0) {
            cur.push(Pc { x, y: 0, k: K::Redstone });
            occ.insert(x);
            rec(i + 1, cells, lo, hi, kmax, cur, occ, out);
            cur.pop();
            occ.remove(&x);
        }
        if cur.len() - nr >= kmax {
            return;
        }
        for sticky in [true, false] {
            for f in 0..2u8 {
                let hx = x + F[f as usize].0;
                for ext in [false, true] {
                    if hx < lo || hx > hi {
                        continue; // never extends inside the row box
                    }
                    if ext && ((0..=2).contains(&hx) || occ.contains(&hx) || cur.iter().any(|p| p.x == hx)) {
                        continue;
                    }
                    cur.push(Pc { x, y: 0, k: K::Piston { sticky, f, ext } });
                    occ.insert(x);
                    if ext {
                        occ.insert(hx);
                    }
                    rec(i + 1, cells, lo, hi, kmax, cur, occ, out);
                    cur.pop();
                    occ.remove(&x);
                    if ext {
                        occ.remove(&hx);
                    }
                }
            }
        }
    }
    rec(0, &cells, lo, hi, kmax, &mut Vec::new(), &mut HashSet::new(), &mut out);
    out
}

pub struct RowOpts {
    pub kmax: usize,
    pub arm: i8,
    pub cap: usize,
    pub sym_cap: usize,
    pub threads: usize,
    pub out_dir: std::path::PathBuf,
    /// Widest row (a + b + 3) to enumerate.
    pub max_w: i8,
    pub gap: u32,
}

/// `families --rows`: every row mechanism per (left arm, right arm) split, exhaustively (<= kmax
/// pistons, arms <= `arm`), solved exactly, minimal, mirror-merged; then the strip boxes.
pub fn rows_cli(o: &RowOpts) -> Result<()> {
    let log = |s: &str| eprintln!("[families rows] {s}");
    std::fs::create_dir_all(&o.out_dir)?;
    // canonical minimal row mechanism -> the smallest split it works in (a <= b after mirror)
    let mut all: BTreeMap<Vec<Pc>, RowMech> = BTreeMap::new();
    let mut per_split: Vec<(i8, i8, usize, usize, usize)> = Vec::new(); // a, b, candidates, working, minimal (canonical)
    let mut splits: Vec<(i8, i8)> = Vec::new();
    for a in 0..=o.arm {
        for b in a..=o.arm {
            if a + b + 3 <= o.max_w {
                splits.push((a, b));
            }
        }
    }
    splits.sort_by_key(|&(a, b)| (a + b, b - a));
    // minimal mechanisms found per split (canonical keys)
    let mut fits: BTreeMap<(i8, i8), Vec<Vec<Pc>>> = BTreeMap::new();
    for &(a, b) in &splits {
        let t0 = std::time::Instant::now();
        let bx = Bx { x0: -a, y0: 0, w: a + b + 3, h: 1 };
        let cands = row_candidates(a, b, o.kmax);
        let found: Mutex<Vec<Vec<Pc>>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cands.len() {
                        break;
                    }
                    let v = &cands[i];
                    if v.is_empty() {
                        continue;
                    }
                    let Some((m, rest)) = Mech::new(bx, v) else { continue };
                    if let Ok(Some(_)) = solve(&m, &rest, o.cap, false) {
                        found.lock().unwrap().push(v.clone());
                    }
                });
            }
        });
        let found = found.into_inner().unwrap();
        let mut mins: HashSet<Vec<Pc>> = HashSet::new();
        for v in &found {
            if let Some(mv) = minimal(v, bx, o.cap, false) {
                mins.insert(canon(&mv));
            }
        }
        let mut mins: Vec<Vec<Pc>> = mins.into_iter().collect();
        mins.sort();
        for c in &mins {
            if all.contains_key(c) {
                continue;
            }
            for vv in [c.clone(), c.iter().map(|p| p.mirror()).collect::<Vec<_>>()] {
                let Some((m, rest)) = Mech::new(bx, &vv) else { continue };
                if let Ok(Some(mut sol)) = solve(&m, &rest, o.cap, false) {
                    if let Some((s, lv)) = best_sym(&m, &rest, &sol, o.sym_cap, 4) {
                        sol.sym = Some(s);
                        sol.sym_levels = lv;
                    }
                    all.insert(c.clone(), RowMech { a, b, pieces: vv.clone(), sol });
                    break;
                }
            }
        }
        log(&format!("arms {a}+{b} (width {}): {} candidates, {} open+close, {} minimal ({:.1}s)", a + b + 3, cands.len(), found.len(), mins.len(), t0.elapsed().as_secs_f64()));
        per_split.push((a, b, cands.len(), found.len(), mins.len()));
        fits.insert((a, b), mins);
        write_rows(o, &all, &per_split, &fits)?;
    }
    Ok(())
}

fn write_rows(o: &RowOpts, all: &BTreeMap<Vec<Pc>, RowMech>, per_split: &[(i8, i8, usize, usize, usize)], fits: &BTreeMap<(i8, i8), Vec<Vec<Pc>>>) -> Result<()> {
    // the strip door: the same row mechanism in the 3 door rows, a box of height `h` (3: rows
    // 0..2; 5: rows -1..3, the ring rows inside), the rows' schedules one after the other
    let compose = |r: &RowMech, h: i8| -> Option<Wire> {
        let rbx = Bx { x0: -r.a, y0: 0, w: r.a + r.b + 3, h: 1 };
        let (rm, _) = Mech::new(rbx, &r.pieces)?;
        let mut pieces = Vec::new();
        for y in 0..3 {
            for p in &r.pieces {
                pieces.push(Pc { x: p.x, y, k: p.k });
            }
        }
        let bx = Bx { x0: -r.a, y0: if h >= 5 { -1 } else { 0 }, w: r.a + r.b + 3, h };
        let (m, rest) = Mech::new(bx, &pieces)?;
        let map = |y: i8, e: E| -> E {
            let p = rm.pieces[rm.pid[e.id as usize]];
            let id = (0..m.info.len()).find(|&i| {
                let q = m.pieces[m.pid[i]];
                q.x == p.x && q.y == y && q.k == p.k
            });
            E { id: id.unwrap_or(0) as u8, act: e.act }
        };
        let cat = |s: &[E]| -> Vec<E> { (0..3).flat_map(|y| s.iter().map(move |e| map(y, *e))).collect() };
        let sol = Sol {
            open: cat(&r.sol.open),
            close: cat(&r.sol.close),
            sym: r.sol.sym.as_ref().map(|s| cat(s)),
            sym_levels: r.sol.sym_levels,
            states: 0,
            extent: r.sol.extent,
            levels: r.sol.levels,
        };
        Some(wiring(&m, &rest, &sol))
    };
    let mut md = String::new();
    md.push_str("# Strip doors: row (and column) mechanisms\n\nGenerated by `mctas door families --rows` (crates/piston-door/src/families.rs). Do not edit by hand.\n\n");
    md.push_str(&format!(
        "In a box of height 3, 4 or 5 (width 3, 4 or 5 for columns) the door blocks can only slide sideways in their rows (up/down in their columns): nothing can pull a block out of the 3x3 vertically. Bounds: each door row is solved on its own (pistons facing E/W in the row, no vertical pistons), EVERY placement of at most **{} pistons** (sticky or normal, retracted or extended at rest) in the row's arms of at most {} cells each, no helpers, no slime; solved exactly in the macro model (pulse model state graph): the 3 door cells empty and the 2 ring cells of the row holding only air or door blocks when open (FULL seamless; the top and bottom ring rows are outside a height-3 box, the hallway fills them), back to the exact rest state, every state inside the row box. Minimal (no piston can be dropped), mirror images merged. A door = one row mechanism per row (any three, each also mirrored): a W x 3 box with arms a + b holds every row mechanism that fits a + b. A column mechanism is a row mechanism turned 90 degrees (a 3 x H box; up/down is not a symmetry for the wiring: both turns count).\n\n",
        o.kmax, o.arm
    ));
    md.push_str("| arms (left+right) | row width = strip box W (Wx3, Wx4, Wx5) | candidates | open+close | minimal row mechanisms (new / fitting) |\n|---|---|---|---|---|\n");
    let mut seen: HashSet<Vec<Pc>> = HashSet::new();
    for (a, b, c, f, n) in per_split {
        let fitting: Vec<&Vec<Pc>> = fits.iter().filter(|((aa, bb), _)| aa <= a && bb <= b).flat_map(|(_, v)| v.iter()).collect();
        let mut uniq: HashSet<&Vec<Pc>> = HashSet::new();
        for v in &fitting {
            uniq.insert(v);
        }
        let _ = n;
        let new = fits.get(&(*a, *b)).map_or(0, |v| v.iter().filter(|x| seen.insert((*x).clone())).count());
        md.push_str(&format!("| {a}+{b} | {} | {c} | {f} | {new} / {} |\n", a + b + 3, uniq.len()));
    }
    md.push_str("\n## Row mechanisms (smallest row first; STATIC = every piston fires from its rest cell only, the easy ones to wire)\n\nwire3 / wire5 = the wiring check of CATALOG.md on the strip door made of this row mechanism in all 3 rows, in a box of height 3 / 5 (5: the ring rows inside the box, their cells beside the ring free for wiring).\n\n| id | arms | pistons | movers | wire3 | wire5 | sym train | open / close | pieces (x in the row; door at x 0..2) |\n|---|---|---|---|---|---|---|---|---|\n");
    let riders_of = |m: &Mech, rest: &St, sol: &Sol| -> usize {
        let mut pos: Vec<HashSet<u8>> = vec![HashSet::new(); m.info.len()];
        let mut c = rest.clone();
        for e in sol.open.iter().chain(sol.close.iter()) {
            pos[e.id as usize].insert(c.pp[e.id as usize]);
            if let Out::Ok(n) = m.apply(&c, *e) {
                c = n;
            }
        }
        pos.iter().filter(|p| p.len() > 1).count()
    };
    let mut list: Vec<(&RowMech, usize)> = all
        .values()
        .map(|r| {
            let bx = Bx { x0: -r.a, y0: 0, w: r.a + r.b + 3, h: 1 };
            let (m, rest) = Mech::new(bx, &r.pieces).unwrap();
            (r, riders_of(&m, &rest, &r.sol))
        })
        .collect();
    list.sort_by_key(|(r, rd)| (r.a + r.b, *rd, r.b - r.a, n_pistons(&r.pieces), r.sol.sym.as_ref().map_or(99, |s| s.len())));
    let mut tsv = String::from("id\tarms\tpistons\tmovers\tsym\topen\tclose\tpieces\n");
    for (i, (r, rd)) in list.iter().enumerate() {
        let bx = Bx { x0: -r.a, y0: 0, w: r.a + r.b + 3, h: 1 };
        let (m, _) = Mech::new(bx, &r.pieces).unwrap();
        let sym = r.sol.sym.as_ref().map(|s| format!("{}{}: {}", s.len(), if r.sol.sym_levels { "+L" } else { "" }, m.fmt_seq(s))).unwrap_or("-".into());
        let pieces = m.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" ");
        let movers = if *rd == 0 { "STATIC".to_string() } else { format!("{rd} riders") };
        let w3 = compose(r, 3).map(|w| format!("{} {:?}", w.class, w.regions)).unwrap_or("-".into());
        let w5 = compose(r, 5).map(|w| format!("{} {:?}", w.class, w.regions)).unwrap_or("-".into());
        md.push_str(&format!("| r{i:03} | {}+{} | {} | {} | {w3} | {w5} | {} | {} / {} | `{}` |\n", r.a, r.b, n_pistons(&r.pieces), movers, sym, r.sol.open.len(), r.sol.close.len(), pieces));
        tsv.push_str(&format!("r{i:03}\t{}+{}\t{}\t{}\t{}\t{}\t{}\t{}\n", r.a, r.b, n_pistons(&r.pieces), rd, r.sol.sym.as_ref().map(|s| m.fmt_seq(s)).unwrap_or_default(), m.fmt_seq(&r.sol.open), m.fmt_seq(&r.sol.close), pieces));
    }
    std::fs::write(o.out_dir.join("ROWS.md"), md)?;
    std::fs::write(o.out_dir.join("ROWS.tsv"), tsv)?;
    Ok(())
}
