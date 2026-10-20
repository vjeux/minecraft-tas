//! The tracker shared by the redsim runner ([`crate::sim`]) and the real-game judge
//! ([`crate::verify`]): it ingests one frame per game tick (every non-air cell of the watched
//! corridor) and keeps, per tick,
//!
//! * the machine's TRANSLATION: the shift that maps the design's blocks onto the frame, compared
//!   on structural states (`clock::canon::structural`: piston `extended`, observer `powered`...
//!   dropped; piston heads are part of their piston; a frame with a moving block is "in motion");
//! * a canonical id of the frame (full states, cells relative to their box) and the box corner, so
//!   that the STEADY STATE is found exactly like a clock's period: the first frame n2 equal to an
//!   earlier frame n1 up to a translation d; then P = n2 - n1, d = corner(n2) - corner(n1), and the
//!   relation must hold tick for tick for at least 2 periods. Speed = P / |d| gt per block.
//!
//! Stop blocks (environment) holding their own state are left out of the machine; a temporary kick
//! block is left out while it stands.

use clock::Pos;
use clock::wave::Interner;
use std::collections::{BTreeMap, HashMap};

/// Per-state facts, cached by interned id.
#[derive(Clone)]
struct StateInfo {
    /// `clock::canon::structural` of the state; None for a piston head (part of its piston).
    structural: Option<u32>,
    moving: bool,
}

pub struct Recorder {
    pub int: Interner,
    info: Vec<StateInfo>,
    structural_ids: HashMap<String, u32>,
    /// Per tick: canonical frame id (machine cells relative to their box corner, full states).
    pub canon: Vec<u64>,
    /// Per tick: the corner (lowest x, y, z) of the machine cells.
    pub corner: Vec<Pos>,
    /// Per tick: the design's translation, None while in motion or when the shape differs.
    pub shifts: Vec<Option<Pos>>,
    /// Per tick: number of machine cells.
    pub counts: Vec<usize>,
    /// The design (structural states, piston heads left out), relative to its corner, sorted.
    reference: Vec<(Pos, u32)>,
    ref_corner: Pos,
    stops: HashMap<Pos, String>,
    /// A temporary kick block: (cell, first tick present, first tick gone).
    kick: Option<(Pos, u32, u32)>,
    /// The last frame's machine cells (full states).
    pub last: Vec<(Pos, String)>,
    /// The first frame's machine cells (full states).
    pub first: Vec<(Pos, String)>,
}

fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn corner_of<'a>(cells: impl Iterator<Item = &'a Pos>) -> Pos {
    let mut lo = [i32::MAX; 3];
    for p in cells {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
        }
    }
    lo
}

impl Recorder {
    /// `design`: the machine's blocks (as written); `stops`: environment blocks; `kick`: a
    /// temporary kick block (cell, placed at, removed at).
    pub fn new(design: &BTreeMap<Pos, String>, stops: &[(Pos, String)], kick: Option<(Pos, u32, u32)>) -> Recorder {
        let mut r = Recorder {
            int: Interner::default(),
            info: Vec::new(),
            structural_ids: HashMap::new(),
            canon: Vec::new(),
            corner: Vec::new(),
            shifts: Vec::new(),
            counts: Vec::new(),
            reference: Vec::new(),
            ref_corner: [0; 3],
            stops: stops.iter().map(|(p, s)| (*p, s.clone())).collect(),
            kick,
            last: Vec::new(),
            first: Vec::new(),
        };
        let mut cells: Vec<(Pos, u32)> = Vec::new();
        for (p, s) in design {
            let id = r.state_id(s);
            if let Some(st) = r.info[id as usize].structural {
                cells.push((*p, st));
            }
        }
        let c = corner_of(cells.iter().map(|(p, _)| p));
        r.ref_corner = c;
        r.reference = cells.into_iter().map(|(p, s)| (sub(p, c), s)).collect();
        r.reference.sort();
        r
    }

    fn state_id(&mut self, s: &str) -> u32 {
        let id = self.int.state(s);
        while self.info.len() <= id as usize {
            let name = self.int.name(self.info.len() as u32).to_string();
            let st = clock::canon::structural(&name);
            let head = st.starts_with("piston_head");
            let moving = st.starts_with("moving_piston");
            let sid = if head {
                None
            } else {
                let n = self.structural_ids.len() as u32;
                Some(*self.structural_ids.entry(st).or_insert(n))
            };
            self.info.push(StateInfo { structural: sid, moving });
        }
        id
    }

    /// Ingest the frame of tick `t` (ticks must come in order, from 0): every non-air cell.
    /// `replace` replaces the last frame (the state at t after inputs applied at t).
    pub fn ingest<'a>(&mut self, t: u32, cells: impl Iterator<Item = (Pos, &'a str)>, replace: bool) {
        let mut mach: Vec<(Pos, u32)> = Vec::new();
        for (p, s) in cells {
            if mctas::blockstate::is_air(s) {
                continue;
            }
            if let Some(st) = self.stops.get(&p)
                && (st == s || clock::canon::structural(st) == clock::canon::structural(s))
            {
                continue;
            }
            if let Some((kp, from, to)) = self.kick
                && kp == p
                && t >= from
                && t < to
            {
                continue;
            }
            let id = self.state_id(s);
            mach.push((p, id));
        }
        mach.sort();
        let corner = if mach.is_empty() { [0; 3] } else { corner_of(mach.iter().map(|(p, _)| p)) };
        let rel: Vec<u32> = mach.iter().flat_map(|(p, id)| {
            let q = sub(*p, corner);
            [q[0] as u32, q[1] as u32, q[2] as u32, *id]
        }).collect();
        let canon = self.int.frame(&rel);
        // the structural shape: piston heads left out; a moving block = in motion
        let moving = mach.iter().any(|(_, id)| self.info[*id as usize].moving);
        let shift = if moving {
            None
        } else {
            let mut st: Vec<(Pos, u32)> = mach.iter().filter_map(|(p, id)| self.info[*id as usize].structural.map(|s| (*p, s))).collect();
            if st.len() != self.reference.len() || st.is_empty() {
                None
            } else {
                let c = corner_of(st.iter().map(|(p, _)| p));
                for (p, _) in st.iter_mut() {
                    *p = sub(*p, c);
                }
                st.sort();
                if st == self.reference { Some(sub(c, self.ref_corner)) } else { None }
            }
        };
        let full: Vec<(Pos, String)> = mach.iter().map(|(p, id)| (*p, self.int.name(*id).to_string())).collect();
        if replace && !self.canon.is_empty() {
            *self.canon.last_mut().unwrap() = canon;
            *self.corner.last_mut().unwrap() = corner;
            *self.shifts.last_mut().unwrap() = shift;
            *self.counts.last_mut().unwrap() = mach.len();
        } else {
            debug_assert_eq!(self.canon.len() as u32, t);
            self.canon.push(canon);
            self.corner.push(corner);
            self.shifts.push(shift);
            self.counts.push(mach.len());
        }
        if self.canon.len() == 1 {
            self.first = full.clone();
        }
        self.last = full;
    }

    pub fn ticks(&self) -> usize {
        self.canon.len()
    }
}

/// The steady state: frames n and n + P equal up to the translation `d`, from `start` on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Steady {
    pub start: u32,
    pub period: u32,
    pub d: Pos,
    /// How many full periods the relation held (to the end of the run or until it broke).
    pub periods: u32,
}

impl Steady {
    /// Cells moved per period along the motion (the d vector's length on its axis; 0 = in place).
    pub fn cells(&self) -> i32 {
        self.d.iter().map(|c| c.abs()).sum()
    }
}

/// Find the steady state from tick `from` on (the end of the kick): over every period p, the
/// maximal runs of ticks t where frame(t + p) == frame(t) shifted by one constant d, at least 2p
/// long; the answer is the smallest p of a MOVING run (d != 0), earliest start first, else the
/// smallest p of an in-place run whose frames change (a shuttle or a clock). Visible frames can
/// repeat on consecutive ticks while blocks are in motion (the moving blocks' progress is hidden),
/// so a lone repeat proves nothing: the relation must hold for whole periods.
pub fn steady(r: &Recorder, from: usize) -> Option<Steady> {
    steady_of(&r.canon, &r.corner, from)
}

/// [`steady`] over raw per-tick frame ids and corners (the census's fast tracker).
pub fn steady_of(canon: &[u64], corner: &[Pos], from: usize) -> Option<Steady> {
    let n = canon.len();
    if n < from + 3 {
        return None;
    }
    let maxp = (n - from) / 2;
    let mut best_moving: Option<Steady> = None;
    let mut best_still: Option<Steady> = None;
    for p in 1..=maxp {
        let mut t = from;
        while t + p < n {
            if canon[t + p] != canon[t] {
                t += 1;
                continue;
            }
            let d = sub(corner[t + p], corner[t]);
            let s = t;
            while t + p < n && canon[t + p] == canon[t] && sub(corner[t + p], corner[t]) == d {
                t += 1;
            }
            let len = t - s;
            if len < 2 * p {
                continue;
            }
            let cand = Steady { start: s as u32, period: p as u32, d, periods: (len / p) as u32 };
            if d != [0, 0, 0] {
                if best_moving.is_none() {
                    best_moving = Some(cand);
                }
            } else {
                // an in-place run counts only when its frames change within a period
                let changes = (s..s + p).any(|k| canon[k] != canon[s]);
                if changes && best_still.is_none() {
                    best_still = Some(cand);
                }
            }
        }
        if best_moving.is_some() {
            break;
        }
    }
    best_moving.or(best_still)
}

/// The distinct translations the machine takes, in order, with the first tick of each visit.
pub fn visits(r: &Recorder) -> Vec<(u32, Pos)> {
    let mut v: Vec<(u32, Pos)> = Vec::new();
    for (t, s) in r.shifts.iter().enumerate() {
        if let Some(s) = s
            && v.last().is_none_or(|(_, last)| last != s)
        {
            v.push((t as u32, *s));
        }
    }
    v
}

/// The end state against the design moved by `shift`: (cells left behind or extra, cells
/// missing), full states compared structurally.
pub fn end_diff(r: &Recorder, design: &BTreeMap<Pos, String>, shift: Pos) -> (Vec<(Pos, String)>, Vec<(Pos, String)>) {
    let want: HashMap<Pos, String> = design.iter().filter(|(_, s)| !s.contains("piston_head")).map(|(p, s)| (add(*p, shift), clock::canon::structural(s))).collect();
    let have: HashMap<Pos, String> = r.last.iter().filter(|(_, s)| !s.contains("piston_head")).map(|(p, s)| (*p, clock::canon::structural(s))).collect();
    let mut extra: Vec<(Pos, String)> = have.iter().filter(|(p, s)| want.get(*p) != Some(*s)).map(|(p, s)| (*p, s.clone())).collect();
    let mut missing: Vec<(Pos, String)> = want.iter().filter(|(p, s)| have.get(*p) != Some(*s)).map(|(p, s)| (*p, s.clone())).collect();
    extra.sort();
    missing.sort();
    (extra, missing)
}
