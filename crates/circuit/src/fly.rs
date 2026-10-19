//! The flying-machine tracker: track fly's `crates/circuit-fly/src/track.rs` (36108674d, by the
//! fly agent), taken into the judge so `circuit verify` judges machines with the same line as every
//! other kind. Per frame it keeps the machine's TRANSLATION (the shift that maps the design's blocks
//! onto the frame, compared on structural states: piston `extended`, observer `powered`... dropped;
//! piston heads are part of their piston; a frame with a moving block is in motion) and a
//! canonical id of the frame (full states relative to the cells' box corner), so the STEADY STATE
//! is found like a clock's period: frame t + P equals frame t shifted by one d, for at least 2
//! periods. Speed = P / |d| gt per block.

use crate::Pos;
use clock::wave::Interner;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
struct StateInfo {
    /// `clock::canon::structural` of the state, interned; None for a piston head.
    structural: Option<u32>,
    moving: bool,
}

pub struct Recorder {
    int: Interner,
    info: Vec<StateInfo>,
    structural_ids: HashMap<String, u32>,
    /// Per frame: canonical frame id (machine cells relative to their corner, full states).
    pub canon: Vec<u64>,
    /// Per frame: the corner (lowest x, y, z) of the machine cells.
    pub corner: Vec<Pos>,
    /// Per frame: the design's translation; None while in motion or when the shape differs.
    pub shifts: Vec<Option<Pos>>,
    reference: Vec<(Pos, u32)>,
    ref_corner: Pos,
    /// A temporary kick block: (cell, first frame present, first frame gone).
    kick: Option<(Pos, u32, u32)>,
}

fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
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
    pub fn new(design: &BTreeMap<Pos, String>, kick: Option<(Pos, u32, u32)>) -> Recorder {
        let mut r = Recorder { int: Interner::default(), info: Vec::new(), structural_ids: HashMap::new(), canon: Vec::new(), corner: Vec::new(), shifts: Vec::new(), reference: Vec::new(), ref_corner: [0; 3], kick };
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

    /// Add frame `t` (in order from 0): every non-air cell.
    pub fn ingest<'a>(&mut self, t: u32, cells: impl Iterator<Item = (Pos, &'a str)>) {
        let mut mach: Vec<(Pos, u32)> = Vec::new();
        for (p, s) in cells {
            if mctas::blockstate::is_air(s) {
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
        let rel: Vec<u32> = mach
            .iter()
            .flat_map(|(p, id)| {
                let q = sub(*p, corner);
                [q[0] as u32, q[1] as u32, q[2] as u32, *id]
            })
            .collect();
        let canon = self.int.frame(&rel);
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
        self.canon.push(canon);
        self.corner.push(corner);
        self.shifts.push(shift);
    }
}

/// The steady state: frame t + P equals frame t shifted by `d`, from `start` on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Steady {
    pub start: u32,
    pub period: u32,
    pub d: Pos,
    /// Full periods the relation held.
    pub periods: u32,
}

impl Steady {
    /// Cells moved per period (0 = in place).
    pub fn cells(&self) -> i32 {
        self.d.iter().map(|c| c.abs()).sum()
    }
}

/// The steady state from frame `from` on: over every period p, the maximal runs where frame(t + p)
/// == frame(t) shifted by one constant d, at least 2p long; the smallest p of a MOVING run
/// (earliest first), else the smallest p of an in-place run whose frames change (a shuttle or a
/// clock). A lone repeat proves nothing (visible frames repeat on consecutive ticks while blocks
/// move): the relation must hold for whole periods.
pub fn steady(r: &Recorder, from: usize) -> Option<Steady> {
    let n = r.canon.len();
    if n < from + 3 {
        return None;
    }
    let maxp = (n - from) / 2;
    let mut best_moving: Option<Steady> = None;
    let mut best_still: Option<Steady> = None;
    for p in 1..=maxp {
        let mut t = from;
        while t + p < n {
            if r.canon[t + p] != r.canon[t] {
                t += 1;
                continue;
            }
            let d = sub(r.corner[t + p], r.corner[t]);
            let s = t;
            while t + p < n && r.canon[t + p] == r.canon[t] && sub(r.corner[t + p], r.corner[t]) == d {
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
            } else if (s..s + p).any(|k| r.canon[k] != r.canon[s]) && best_still.is_none() {
                best_still = Some(cand);
            }
        }
        if best_moving.is_some() {
            break;
        }
    }
    best_moving.or(best_still)
}
