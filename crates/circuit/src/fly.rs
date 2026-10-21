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
    /// Return-station cells (`port station.<n>`): the structural id of the station block there.
    /// A station cell holding its own block (any dynamic state) is left out of the machine's
    /// shape, never out of the canonical frame (the whole state, stations included, must repeat).
    stations: HashMap<Pos, u32>,
    /// Station cells empty at rest: a part of a station that a switch moves into place (the
    /// observer an on/off lever pushes into its working cell). Whatever stands there is left out
    /// of the machine's shape; the machine itself never enters a station cell.
    free_stations: std::collections::HashSet<Pos>,
    /// Leave the station cells out of `canon` / `corner` too ([`Recorder::machine`]): a one-way
    /// machine leaves a static launch station (its start lever) behind, and only the machine itself
    /// repeats shifted.
    machine_only: bool,
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
    /// `stations`: the return-station cells (their blocks are design blocks, left out of the
    /// machine's shape).
    pub fn new(design: &BTreeMap<Pos, String>, kick: Option<(Pos, u32, u32)>, stations: &[Pos]) -> Recorder {
        let mut r = Recorder { int: Interner::default(), info: Vec::new(), structural_ids: HashMap::new(), canon: Vec::new(), corner: Vec::new(), shifts: Vec::new(), reference: Vec::new(), ref_corner: [0; 3], kick, stations: HashMap::new(), free_stations: stations.iter().filter(|p| !design.contains_key(*p)).copied().collect(), machine_only: false };
        let mut cells: Vec<(Pos, u32)> = Vec::new();
        for (p, s) in design {
            let id = r.state_id(s);
            if let Some(st) = r.info[id as usize].structural {
                if stations.contains(p) {
                    r.stations.insert(*p, st);
                    continue;
                }
                cells.push((*p, st));
            }
        }
        let c = corner_of(cells.iter().map(|(p, _)| p));
        r.ref_corner = c;
        r.reference = cells.into_iter().map(|(p, s)| (sub(p, c), s)).collect();
        r.reference.sort();
        r
    }

    /// Like [`Recorder::new`], with the station cells left out of every frame (`canon`, `corner`)
    /// as well as out of the shape: [`steady`] then sees the machine alone flying away from a
    /// static launch station (a one-way machine's start lever and its support).
    pub fn machine(design: &BTreeMap<Pos, String>, kick: Option<(Pos, u32, u32)>, stations: &[Pos]) -> Recorder {
        let mut r = Recorder::new(design, kick, stations);
        r.machine_only = true;
        r
    }

    /// Is this (cell, state) a station block standing in its own station cell (or anything in a
    /// station cell empty at rest)? With `machine_only`, anything in any station cell is: the
    /// machine never enters one, and a station's dust or diode changes its shape and power.
    fn is_station(&self, p: &Pos, id: u32) -> bool {
        self.free_stations.contains(p) || (self.machine_only && self.stations.contains_key(p)) || self.info[id as usize].structural.is_some_and(|s| self.stations.get(p) == Some(&s))
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
        if self.machine_only {
            mach.retain(|(p, id)| !self.is_station(p, *id));
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
            // the machine's shape: station cells holding their own block left out
            let mut st: Vec<(Pos, u32)> = mach
                .iter()
                .filter_map(|(p, id)| self.info[*id as usize].structural.map(|s| (*p, s)))
                .filter(|(p, s)| self.stations.get(p) != Some(s) && !self.free_stations.contains(p))
                .collect();
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
/// == frame(t) shifted by one constant d, at least 2p long; MOVING runs (d != 0: the smallest p,
/// earliest first) and IN-PLACE runs whose frames change (d = 0: a shuttle or a clock). An in-place
/// run that lasts at least as long as the best moving run wins: a two-way machine flies steadily
/// between its stations for a few periods only, then the whole round trip repeats in place. A lone
/// repeat proves nothing (visible frames repeat on consecutive ticks while blocks move): the
/// relation must hold for whole periods.
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
        if best_moving.is_some() && best_still.is_some() {
            break;
        }
    }
    match (best_moving, best_still) {
        (Some(m), Some(s)) if s.periods * s.period >= m.periods * m.period => Some(s),
        (Some(m), _) => Some(m),
        (None, s) => s,
    }
}
