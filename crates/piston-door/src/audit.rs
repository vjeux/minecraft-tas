//! `mctas door audit` (track audit, 2026-10-19): what our rules, checks, generators and wiring
//! tools silently exclude, found mechanically on doors that WORK (the community 7x9 `c79`, every
//! archived design, the gadget library) instead of waiting for vjeux to show us one.
//!
//! Sections (each a markdown table; docs/audit.md is written from them):
//! 1. `checks`: every judge / property check on a design (redsim in-process: the redsim judge at
//!    32 positions, FULL seamless, the hallway, the volume, `door cycle`'s alternation + frame
//!    12/12, `door sheet`'s labels, the flat label; with `--jvm` also the real judge, `door
//!    properties`, `door properties-dyn` and `door eligible`): every check that rejects or
//!    mislabels a working door is a finding.
//! 2. `mech`: the design's MECHANISM extracted from one recorded redsim cycle (every door block
//!    and piston followed through the moving-block entities: paths, which piston pushed or pulled
//!    what, carried pistons, signal pistons that move nothing, static frame blocks), then its
//!    membership in each mechanism generator's space by running families' OWN generator
//!    (`families::generate_within`) restricted to the design's pistons, with each bound relaxed in
//!    turn (families v3, mechspace's flags, fullmech), the macro solver on the piece set, and
//!    the path / provider rules each real move breaks (the exact bound, with its source line).
//! 3. `wiring`: the block states and mechanics each wiring tool cannot represent (satwire's
//!    encoding, cosearch's palette and screen, boxproof's cells).
//! 4. `rules`: every hard-coded rule or assumption, found by pattern in the crate's sources
//!    (file:line printed live, so the table follows the code), with what it would exclude.

use crate::DoorSpec;
use crate::cosearch::{self as cs, Frame, RsDoor, Snap};
use crate::families::{self as fam, Bx, K, Pc};
use anyhow::{Context, Result};
use mctas::design::Design;
use redsim::{Block, Dir};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub type Pos = [i32; 3];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn mul(a: Pos, k: i32) -> Pos {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Door-plane coordinates (u along the opening's width, v up, depth through the door) of a world
/// offset from the door's min corner.
fn local(spec: &DoorSpec, p: Pos) -> Pos {
    let q = sub(p, spec.door);
    if spec.along_z { [q[2], q[1], q[0]] } else { q }
}
fn local_dir(spec: &DoorSpec, d: Pos) -> Pos {
    if spec.along_z { [d[2], d[1], d[0]] } else { d }
}
/// families' facing index (0 E, 1 W, 2 U, 3 D) of a door-plane direction; None = off the plane.
fn fam_facing(d: Pos) -> Option<u8> {
    match d {
        [1, 0, 0] => Some(0),
        [-1, 0, 0] => Some(1),
        [0, 1, 0] => Some(2),
        [0, -1, 0] => Some(3),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------
// Section 2: the mechanism of a working door

/// A piston followed through one open/close cycle.
#[derive(Clone, Debug)]
pub struct PTrack {
    pub rest: Pos,
    pub sticky: bool,
    pub facing: Dir,
    pub ext_rest: bool,
    /// Every cell its base occupied (rest first).
    pub visited: Vec<Pos>,
    pub fires: usize,
    pub moved_door: usize,
    pub moved_pistons: BTreeSet<usize>,
    pub moved_other: Vec<String>,
    /// Lifted / carried: displaced from its rest cell at some point.
    pub carried: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Door(usize),
    Piston(usize),
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// Pushed by a piston whose base was `k` cells behind the block.
    Push(i32),
    /// Pulled by a sticky piston (the block two cells ahead of its base).
    Pull,
    /// Moved with a piston's structure but not on its line (dragged by slime / honey).
    Drag,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct Move {
    /// 0 = opening, 1 = closing.
    pub phase: u8,
    pub tick: usize,
    pub item: Item,
    pub from: Pos,
    pub to: Pos,
    pub by: Option<usize>,
    pub how: How,
    /// The acting piston's base was off its rest cell (it was carried there).
    pub by_displaced: bool,
    /// The acting piston was extended at rest (a level-held puller releasing).
    pub by_ext_rest: bool,
    pub moved: Block,
}

/// What one recorded cycle shows about a door's machine.
#[derive(Clone, Debug)]
pub struct Mechanism {
    pub name: String,
    pub spec: DoorSpec,
    pub door_state: Block,
    /// Door-relative paths (door-plane coordinates) of the 9 door blocks, opening then closing.
    pub paths_open: Vec<Vec<Pos>>,
    pub paths_close: Vec<Vec<Pos>>,
    pub pistons: Vec<PTrack>,
    /// Pistons that move a door block, or carry a piston that does (transitively).
    pub mech: Vec<usize>,
    pub moves: Vec<Move>,
    /// Door-plane frame cells holding the same non-air block at rest and at every tick.
    pub static_frame: Vec<Pos>,
    /// Frame cells holding a plain full block when open.
    pub frame_open: usize,
    pub lost: usize,
    /// Door-plane cells the mechanism (door blocks, mechanism pistons and heads) touches.
    pub footprint: BTreeSet<(i32, i32)>,
    /// Off the door plane: some mechanism block or piston leaves depth 0.
    pub off_plane: Vec<String>,
    /// Non-piston blocks the mechanism pistons move besides door blocks (slime, honey, redstone
    /// blocks, observers...).
    pub carried_parts: BTreeSet<String>,
    /// A piston base ever inside the 3x3 (mechspace's `--no-door-bodies` sub-model forbids it).
    pub body_in_door: bool,
    /// Door-plane bounding box (x0, y0, x1, y1, door coordinates) of every block of the build at
    /// rest and at every tick of the cycle, the lever excluded (the community volume's box);
    /// None when some block leaves the plane.
    pub build_box: Option<[i32; 4]>,
}

fn block_name(b: &Block) -> String {
    let s = b.to_string();
    let s = s.trim_start_matches("minecraft:");
    s.split('[').next().unwrap_or(s).to_string()
}

fn holds_piston(f: &Frame, moving: &HashMap<Pos, redsim::piston::MovingBe>, p: Pos, sticky: bool, facing: Dir) -> bool {
    match f.snap.get(p) {
        Some(Block::Piston { sticky: s, facing: fc, .. }) => s == sticky && fc == facing,
        Some(b) if cs::is_moving(&b) => moving.get(&p).is_some_and(|m| matches!(m.moved, Block::Piston { sticky: s, facing: fc, .. } if s == sticky && fc == facing)),
        _ => false,
    }
}

fn holds_state(f: &Frame, moving: &HashMap<Pos, redsim::piston::MovingBe>, p: Pos, state: Block) -> bool {
    match f.snap.get(p) {
        Some(s) if s == state => true,
        Some(s) if cs::is_moving(&s) => moving.get(&p).is_some_and(|m| m.moved == state),
        _ => false,
    }
}

const DIRS6: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Follow the door blocks and pistons through one phase's frames.
#[allow(clippy::too_many_arguments)]
fn track_phase(phase: u8, frames: &[Frame], before: &Snap, door_state: Block, door: &mut [Pos], pist: &mut [Pos], pt: &mut [PTrack], moves: &mut Vec<Move>, lost: &mut usize, foot: &mut Vec<(Pos, bool)>) {
    let mut prev: HashMap<Pos, redsim::piston::MovingBe> = HashMap::new();
    let mut prev_snap = before;
    for (ti, f) in frames.iter().enumerate() {
        let moving: HashMap<Pos, redsim::piston::MovingBe> = f.moving.iter().map(|(p, m)| (*p, *m)).collect();
        let is_new = |p: &Pos, m: &redsim::piston::MovingBe| {
            m.progress_o == 0 && !prev.get(p).is_some_and(|q| q.progress == 0 && q.direction == m.direction && q.extending == m.extending && q.moved == m.moved)
        };
        // pistons acting this tick: (piston id, extending)
        let mut fired: Vec<(usize, bool)> = Vec::new();
        for (p, m) in f.moving.iter().filter(|(p, m)| m.source && is_new(p, m)) {
            let f0 = m.direction.step();
            let base = if m.extending { sub(*p, f0) } else { *p };
            if let Some(j) = pist.iter().position(|q| *q == base) {
                if pt[j].facing == m.direction {
                    fired.push((j, m.extending));
                    pt[j].fires += 1;
                }
            }
        }
        // blocks moved this tick (origin -> destination), attributed to an acting piston
        let mut updates: Vec<(Item, Pos, Pos)> = Vec::new();
        for (p, m) in f.moving.iter().filter(|(p, m)| !m.source && is_new(p, m)) {
            let mv = m.push_direction().step();
            let o = sub(*p, mv);
            let f0 = m.direction.step();
            let mut by = None;
            let mut how = How::Unknown;
            for &(j, ext) in &fired {
                if ext != m.extending || pt[j].facing != m.direction {
                    continue;
                }
                let b = pist[j];
                if ext {
                    for k in 1..=13 {
                        if add(b, mul(f0, k)) == o {
                            if by.is_none() || matches!(how, How::Push(k0) if k < k0) {
                                by = Some(j);
                                how = How::Push(k);
                            }
                            break;
                        }
                    }
                } else if add(b, mul(f0, 2)) == o {
                    by = Some(j);
                    how = How::Pull;
                }
            }
            if by.is_none() {
                // off every acting piston's line: dragged with a structure (slime / honey)
                let same: Vec<usize> = fired.iter().filter(|(j, ext)| *ext == m.extending && pt[*j].facing == m.direction).map(|(j, _)| *j).collect();
                if let Some(&j) = same.first() {
                    by = Some(j);
                    how = How::Drag;
                }
            }
            let item = if let Some(i) = door.iter().position(|q| *q == o) {
                Item::Door(i)
            } else if let Some(j) = pist.iter().position(|q| *q == o) {
                Item::Piston(j)
            } else {
                Item::Other
            };
            if let Some(j) = by {
                match item {
                    Item::Door(_) => pt[j].moved_door += 1,
                    Item::Piston(k) => {
                        pt[j].moved_pistons.insert(k);
                    }
                    Item::Other => {
                        if !matches!(m.moved, Block::PistonHead { .. }) {
                            pt[j].moved_other.push(block_name(&m.moved));
                        }
                    }
                }
            }
            let (by_displaced, by_ext_rest) = match by {
                Some(j) => (pist[j] != pt[j].rest, pt[j].ext_rest),
                None => (false, false),
            };
            moves.push(Move { phase, tick: ti + 1, item, from: o, to: *p, by, how, by_displaced, by_ext_rest, moved: m.moved });
            if !matches!(item, Item::Other) {
                updates.push((item, o, *p));
            }
        }
        for (item, _, to) in &updates {
            match item {
                Item::Door(i) => door[*i] = *to,
                Item::Piston(j) => {
                    pist[*j] = *to;
                    if !pt[*j].visited.contains(to) {
                        pt[*j].visited.push(*to);
                    }
                    pt[*j].carried = true;
                }
                Item::Other => {}
            }
        }
        // fallback: an item not where we think it is (0-tick moves leave no entity)
        for i in 0..door.len() {
            let p = door[i];
            if holds_state(f, &moving, p, door_state) {
                continue;
            }
            let claimed: HashSet<Pos> = door.iter().copied().collect();
            let mut found = None;
            'search: for d in DIRS6 {
                for k in 1..=2 {
                    let q = add(p, mul(d, k));
                    if !claimed.contains(&q) && holds_state(f, &moving, q, door_state) && prev_snap.get(q) != Some(door_state) {
                        found = Some(q);
                        break 'search;
                    }
                }
            }
            match found {
                Some(q) => door[i] = q,
                None => *lost += 1,
            }
        }
        for j in 0..pist.len() {
            let p = pist[j];
            let (sticky, facing) = (pt[j].sticky, pt[j].facing);
            if holds_piston(f, &moving, p, sticky, facing) {
                continue;
            }
            let claimed: HashSet<Pos> = pist.iter().copied().collect();
            let mut found = None;
            'search2: for d in DIRS6 {
                for k in 1..=2 {
                    let q = add(p, mul(d, k));
                    if !claimed.contains(&q) && holds_piston(f, &moving, q, sticky, facing) {
                        found = Some(q);
                        break 'search2;
                    }
                }
            }
            match found {
                Some(q) => {
                    pist[j] = q;
                    if !pt[j].visited.contains(&q) {
                        pt[j].visited.push(q);
                    }
                    pt[j].carried = true;
                }
                None => *lost += 1,
            }
        }
        // footprint candidates: door blocks, piston bases and heads at this tick
        for p in door.iter() {
            foot.push((*p, true));
        }
        for (j, p) in pist.iter().enumerate() {
            foot.push((*p, false));
            let h = add(*p, pt[j].facing.step());
            let head_here = match f.snap.get(h) {
                Some(Block::PistonHead { facing, .. }) => facing == pt[j].facing,
                Some(b) if cs::is_moving(&b) => moving.get(&h).is_some_and(|m| m.direction == pt[j].facing && (m.source || matches!(m.moved, Block::PistonHead { .. }))),
                _ => false,
            };
            if head_here {
                // tag the head with its piston: encoded as a negative-z sentinel list below
                foot.push((h, false));
            }
        }
        prev = moving;
        prev_snap = &f.snap;
    }
}

/// Extract the mechanism of a working door from one recorded redsim cycle.
pub fn mechanism(name: &str, door: &RsDoor) -> std::result::Result<Mechanism, String> {
    let rec = cs::record(door)?;
    let spec = door.spec;
    let cells = spec.door_cells();
    let door_state = rec.door_state;
    // pistons at rest
    let mut pt: Vec<PTrack> = Vec::new();
    for (p, b) in rec.rest.iter() {
        if let Block::Piston { sticky, facing, extended } = b {
            pt.push(PTrack { rest: p, sticky, facing, ext_rest: extended, visited: vec![p], fires: 0, moved_door: 0, moved_pistons: BTreeSet::new(), moved_other: Vec::new(), carried: false });
        }
    }
    let mut dpos: Vec<Pos> = cells.clone();
    let mut ppos: Vec<Pos> = pt.iter().map(|p| p.rest).collect();
    let mut moves = Vec::new();
    let mut lost = 0;
    let mut foot: Vec<(Pos, bool)> = Vec::new();
    let mut open_paths: Vec<Vec<Pos>> = cells.iter().map(|c| vec![*c]).collect();
    let mut close_paths: Vec<Vec<Pos>> = Vec::new();
    let ph_frames = [&rec.open, &rec.close];
    let mut before = &rec.rest;
    for phase in 0..2u8 {
        let frames = ph_frames[phase as usize];
        // per-frame positions, for the paths
        let mut d2 = dpos.clone();
        if phase == 1 {
            close_paths = d2.iter().map(|c| vec![*c]).collect();
        }
        for (k, f) in frames.iter().enumerate() {
            let mut sub_moves = Vec::new();
            track_phase(phase, std::slice::from_ref(f), if k == 0 { before } else { &frames[k - 1].snap }, door_state, &mut d2, &mut ppos, &mut pt, &mut sub_moves, &mut lost, &mut foot);
            for m in &mut sub_moves {
                m.tick = k + 1;
            }
            moves.extend(sub_moves);
            let paths = if phase == 0 { &mut open_paths } else { &mut close_paths };
            for (i, p) in d2.iter().enumerate() {
                if paths[i].last() != Some(p) {
                    paths[i].push(*p);
                }
            }
        }
        dpos = d2;
        before = frames.last().map(|f| &f.snap).unwrap_or(&rec.rest);
    }
    // mechanism pistons: move a door block, or carry one that does (transitively)
    let mut mech: BTreeSet<usize> = (0..pt.len()).filter(|j| pt[*j].moved_door > 0).collect();
    loop {
        let more: Vec<usize> = (0..pt.len()).filter(|j| !mech.contains(j) && pt[*j].moved_pistons.iter().any(|k| mech.contains(k))).collect();
        if more.is_empty() {
            break;
        }
        mech.extend(more);
    }
    let mech: Vec<usize> = mech.into_iter().collect();
    let loc = |p: Pos| local(&spec, p);
    let paths_open: Vec<Vec<Pos>> = open_paths.iter().map(|v| v.iter().map(|p| loc(*p)).collect()).collect();
    let paths_close: Vec<Vec<Pos>> = close_paths.iter().map(|v| v.iter().map(|p| loc(*p)).collect()).collect();
    // static frame cells and the open frame
    let frame = crate::sheet::frame_cells(spec.door);
    let open_snap = &rec.open.last().map(|f| f.snap.clone()).unwrap_or(rec.rest.clone());
    let mut static_frame = Vec::new();
    let mut frame_open = 0;
    for c in &frame {
        let c = if spec.along_z { let q = local(&spec, *c); let _ = q; *c } else { *c };
        let r = rec.rest.get(c).unwrap_or(Block::Air);
        if open_snap.get(c).is_some_and(|b| crate::sheet::filled_frame(&b, &door_state)) {
            frame_open += 1;
        }
        if r.is_air() {
            continue;
        }
        let same = rec.open.iter().chain(rec.close.iter()).all(|f| f.snap.get(c) == Some(r));
        if same {
            static_frame.push(loc(c));
        }
    }
    // footprint of the mechanism (door plane)
    let mech_set: HashSet<Pos> = HashSet::new();
    let _ = mech_set;
    let mut footprint: BTreeSet<(i32, i32)> = BTreeSet::new();
    let mut off_plane = Vec::new();
    for p in cells.iter() {
        let q = loc(*p);
        footprint.insert((q[0], q[1]));
    }
    // door blocks along their paths, mechanism pistons at every visited cell, their heads
    for v in paths_open.iter().chain(paths_close.iter()) {
        for q in v {
            footprint.insert((q[0], q[1]));
            if q[2] != 0 {
                off_plane.push(format!("door block at depth {}", q[2]));
            }
        }
    }
    let mut body_in_door = false;
    for &j in &mech {
        let fq = local_dir(&spec, pt[j].facing.step());
        if fam_facing(fq).is_none() {
            off_plane.push(format!("piston at {:?} faces {}", loc(pt[j].rest), pt[j].facing.name()));
        }
        for c in &pt[j].visited {
            let q = loc(*c);
            footprint.insert((q[0], q[1]));
            if q[2] != 0 {
                off_plane.push(format!("piston at depth {}", q[2]));
            }
            if (0..3).contains(&q[0]) && (0..3).contains(&q[1]) && q[2] == 0 {
                body_in_door = true;
            }
        }
    }
    // heads of mechanism pistons seen in the frames
    let mech_cells: HashSet<Pos> = mech.iter().flat_map(|j| pt[*j].visited.iter().copied()).collect();
    for (p, is_door) in &foot {
        if *is_door {
            continue;
        }
        // a head cell is recorded right after its base: keep cells next to a mechanism base
        if mech_cells.contains(p) || DIRS6.iter().any(|d| mech_cells.contains(&sub(*p, *d))) {
            let q = loc(*p);
            if q[2] == 0 {
                footprint.insert((q[0], q[1]));
            }
        }
    }
    // the build's door-plane box over the cycle (the lever excluded)
    let mut bb = [0i32, 0, 2, 2];
    let mut flat_build = true;
    for s in std::iter::once(&rec.rest).chain(rec.open.iter().map(|f| &f.snap)).chain(rec.close.iter().map(|f| &f.snap)) {
        for (p, b) in s.iter() {
            if b.is_air() || p == spec.lever {
                continue;
            }
            let q = loc(p);
            if q[2] != 0 {
                flat_build = false;
                continue;
            }
            bb = [bb[0].min(q[0]), bb[1].min(q[1]), bb[2].max(q[0]), bb[3].max(q[1])];
        }
    }
    let build_box = flat_build.then_some(bb);
    let mut carried_parts = BTreeSet::new();
    for &j in &mech {
        for o in &pt[j].moved_other {
            carried_parts.insert(o.clone());
        }
    }
    off_plane.sort();
    off_plane.dedup();
    Ok(Mechanism {
        name: name.to_string(),
        spec,
        door_state,
        paths_open,
        paths_close,
        pistons: pt,
        mech,
        moves,
        static_frame,
        frame_open,
        lost,
        footprint,
        off_plane,
        carried_parts,
        body_in_door,
        build_box,
    })
}

impl Mechanism {
    /// The mechanism pistons as [local rest x, y, z, facing dx, dy, dz, sticky, extended at rest],
    /// sorted (wiring ignored): `door provenance` compares mechanisms with it, off-plane ones too.
    pub fn signature(&self) -> Vec<[i32; 8]> {
        let mut v: Vec<[i32; 8]> = self
            .mech
            .iter()
            .map(|&j| {
                let p = &self.pistons[j];
                let q = local(&self.spec, p.rest);
                let d = local_dir(&self.spec, p.facing.step());
                [q[0], q[1], q[2], d[0], d[1], d[2], i32::from(p.sticky), i32::from(p.ext_rest)]
            })
            .collect();
        v.sort();
        v
    }
    /// The door blocks' opening paths (door-plane cells, consecutive repeats dropped), sorted by
    /// start cell: the choreography, independent of where the pistons stand (`door provenance`:
    /// the community 7x9's moves re-laid in an 8x8 box keep it).
    pub fn path_signature(&self) -> Vec<Vec<[i32; 3]>> {
        let mut v: Vec<Vec<[i32; 3]>> = self
            .paths_open
            .iter()
            .map(|p| {
                let mut q: Vec<[i32; 3]> = Vec::new();
                for c in p {
                    if q.last() != Some(c) {
                        q.push(*c);
                    }
                }
                q
            })
            .collect();
        v.sort();
        v
    }
    /// The mechanism pistons as families pieces (None when one is off the door plane).
    pub fn pieces(&self) -> Option<Vec<Pc>> {
        let mut v = Vec::new();
        for &j in &self.mech {
            let p = &self.pistons[j];
            let q = local(&self.spec, p.rest);
            let f = fam_facing(local_dir(&self.spec, p.facing.step()))?;
            if q[2] != 0 {
                return None;
            }
            v.push(Pc { x: q[0] as i8, y: q[1] as i8, k: K::Piston { sticky: p.sticky, f, ext: p.ext_rest } });
        }
        v.sort();
        Some(v)
    }
    /// The static frame stones as families pieces.
    pub fn stones(&self) -> Vec<Pc> {
        self.static_frame.iter().filter(|q| q[2] == 0).map(|q| Pc { x: q[0] as i8, y: q[1] as i8, k: K::Stone }).collect()
    }
    /// families' box: the bounding box of the mechanism's footprint over the cycle.
    pub fn bx(&self) -> Bx {
        let (mut x0, mut y0, mut x1, mut y1) = (0, 0, 2, 2);
        for (x, y) in &self.footprint {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        Bx { x0: x0 as i8, y0: y0 as i8, w: (x1 - x0 + 1) as i8, h: (y1 - y0 + 1) as i8 }
    }
    pub fn signal_pistons(&self) -> Vec<usize> {
        (0..self.pistons.len()).filter(|j| !self.mech.contains(j)).collect()
    }
    fn pname(&self, j: usize) -> String {
        let p = &self.pistons[j];
        let q = local(&self.spec, p.rest);
        let f = match fam_facing(local_dir(&self.spec, p.facing.step())) {
            Some(f) => fam::FNAME[f as usize].to_string(),
            None => p.facing.name().to_string(),
        };
        format!("{}{}{}({},{}{})", if p.sticky { "S" } else { "N" }, f, if p.ext_rest { "+" } else { "" }, q[0], q[1], if q[2] != 0 { format!(",z{}", q[2]) } else { String::new() })
    }
    /// Door block names: BL BM BR / ML C MR / TL TM TR by their rest cell.
    fn bname(&self, i: usize) -> String {
        let q = self.paths_open[i][0];
        let col = ["L", "M", "R"][q[0].clamp(0, 2) as usize];
        let row = ["B", "M", "T"][q[1].clamp(0, 2) as usize];
        if q[0] == 1 && q[1] == 1 { "C".into() } else { format!("{row}{col}") }
    }
    fn fmt_path(v: &[Pos]) -> String {
        v.iter().map(|q| format!("({},{})", q[0], q[1])).collect::<Vec<_>>().join("->")
    }
}

/// One generator configuration (a bound setting) to test membership under.
#[derive(Clone, Debug)]
pub struct GenCfg {
    pub name: &'static str,
    pub tool: &'static str,
    pub pmax: usize,
    pub two: usize,
    pub carry: usize,
    pub vmid: &'static str,
    pub ext_pullers: bool,
    /// families' `--ext-max` / `--ext-apart` (0 / false = the generator's defaults).
    pub ext_max: usize,
    pub ext_apart: bool,
    /// families' `--ring-escape` (ring-resting pistons carried into a corner / out of the frame).
    pub ring_escape: bool,
}

pub fn gen_cfgs() -> Vec<GenCfg> {
    // g1 is the catalog generator as it was before 2026-10-19 (1-move exits by RETRACTED pullers
    // only); since then extended-at-rest pullers are every generator's default (g2..g5, bounded:
    // --ext-max 3, each next to an earlier one), g6 drops that bound too, g7 adds --ring-escape
    // (c77's 7x7 w1: pistons resting in ring cells escape into the corners).
    vec![
        GenCfg { name: "families v3 --no-ext-pullers (the catalog before 2026-10-19)", tool: "families / fullmech", pmax: 10, two: 1, carry: 0, vmid: "", ext_pullers: false, ext_max: 0, ext_apart: false, ring_escape: false },
        GenCfg { name: "families v3 (catalog: P<=10, two 1, lifts, pushes)", tool: "families / fullmech", pmax: 10, two: 1, carry: 0, vmid: "", ext_pullers: true, ext_max: 0, ext_apart: false, ring_escape: false },
        GenCfg { name: "mechspace gen default (P<=11, two 1)", tool: "mechspace", pmax: 11, two: 1, carry: 0, vmid: "", ext_pullers: true, ext_max: 0, ext_apart: false, ring_escape: false },
        GenCfg { name: "mechspace --two 7", tool: "mechspace", pmax: 11, two: 7, carry: 0, vmid: "", ext_pullers: true, ext_max: 0, ext_apart: false, ring_escape: false },
        GenCfg { name: "mechspace --two 7 --carry 2 --vmid any", tool: "mechspace", pmax: 11, two: 7, carry: 2, vmid: "any", ext_pullers: true, ext_max: 0, ext_apart: false, ring_escape: false },
        GenCfg { name: "every flag, P<=16 (--ext-max 12 --ext-apart)", tool: "(widest)", pmax: 16, two: 7, carry: 2, vmid: "any", ext_pullers: true, ext_max: 12, ext_apart: true, ring_escape: false },
        GenCfg { name: "every flag + --ring-escape, P<=16", tool: "families --ring-escape", pmax: 16, two: 7, carry: 2, vmid: "any", ext_pullers: true, ext_max: 12, ext_apart: true, ring_escape: true },
    ]
}

/// Set families' extended-puller switches for `c` (the defaults where `c` leaves them 0 / false).
fn set_ext(c: &GenCfg) {
    use std::sync::atomic::Ordering::Relaxed;
    fam::EXT_PULLERS.store(c.ext_pullers, Relaxed);
    fam::EXT_MAX.store(if c.ext_max == 0 { 3 } else { c.ext_max }, Relaxed);
    fam::EXT_ADJACENT.store(!c.ext_apart, Relaxed);
    fam::RING_ESCAPE.store(c.ring_escape, Relaxed);
    fam::RING_ESCAPE_MAX.store(4, Relaxed);
}

/// families' extended-puller switches back to their defaults.
fn reset_ext() {
    use std::sync::atomic::Ordering::Relaxed;
    fam::EXT_PULLERS.store(true, Relaxed);
    fam::EXT_MAX.store(3, Relaxed);
    fam::EXT_ADJACENT.store(true, Relaxed);
    fam::RING_ESCAPE.store(false, Relaxed);
    fam::RING_ESCAPE_MAX.store(3, Relaxed);
}

fn vmid_of(s: &str) -> Vec<Vec<(i8, i8)>> {
    match s {
        "ml" => vec![vec![(0, 1)]],
        "mr" => vec![vec![(2, 1)]],
        "any" => vec![vec![], vec![(0, 1)], vec![(2, 1)]],
        "both" => vec![vec![(0, 1), (2, 1)]],
        _ => Vec::new(),
    }
}

/// Is `target` one of the generator's candidates in box `bx` under `c`? (families' own code,
/// restricted to the target's pieces; the mirror image too.)
pub fn member(target: &[Pc], bx: Bx, c: &GenCfg) -> bool {
    if target.is_empty() {
        return false;
    }
    let try_one = |t: &[Pc], b: Bx| -> bool {
        let allow: HashSet<Pc> = t.iter().copied().collect();
        let out = fam::generate_within(b, c.pmax, true, true, c.two, c.carry, &vmid_of(c.vmid), &allow);
        let mut want = t.to_vec();
        want.sort();
        out.iter().any(|v| *v == want)
    };
    let _guard = GEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    set_ext(c);
    let mirrored: Vec<Pc> = target.iter().map(|p| p.mirror()).collect();
    let mbx = Bx { x0: 2 - (bx.x0 + bx.w - 1), y0: bx.y0, w: bx.w, h: bx.h };
    let r = try_one(target, bx) || try_one(&mirrored, mbx);
    reset_ext();
    r
}

/// families' `EXT_PULLERS` is a process-wide flag: audit threads take turns with the generator.
static GEN_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The path / provider rules of families' generator each real move breaks (with source refs).
pub fn path_rules(m: &Mechanism) -> Vec<String> {
    let mut out = Vec::new();
    let mut two_others = 0;
    for i in 0..9 {
        let p = &m.paths_open[i];
        let n = p.len() - 1;
        let name = m.bname(i);
        let center = p[0][0] == 1 && p[0][1] == 1;
        if n > 2 {
            out.push(format!("{name} moves {n} times while opening {} (generator paths are 1-2 unit moves: families.rs Gen::paths)", Mechanism::fmt_path(p)));
        }
        // reversal
        for w in p.windows(3) {
            let d1 = sub(w[1], w[0]);
            let d2 = sub(w[2], w[1]);
            if add(d1, d2) == [0, 0, 0] {
                out.push(format!("{name} moves back ({}) (paths never reverse: Gen::paths skips d2 == -d1)", Mechanism::fmt_path(p)));
                break;
            }
        }
        if center && !(n == 2 && sub(p[1], p[0]) == sub(p[2], p[1])) {
            out.push(format!("C leaves in {n} moves {} (the core: the center leaves STRAIGHT in exactly 2: Gen::core)", Mechanism::fmt_path(p)));
        }
        if !center && n == 2 {
            two_others += 1;
        }
        if p.iter().skip(1).any(|q| q[2] != 0) {
            out.push(format!("{name} leaves the door plane"));
        }
        if p.windows(2).any(|w| (w[1][0] - w[0][0]).abs() + (w[1][1] - w[0][1]).abs() > 1) {
            out.push(format!("{name} jumps more than one cell at once {} (falls: families' model has no gravity blocks)", Mechanism::fmt_path(p)));
        }
    }
    // the center's line edge takes 2 moves in the core; the others count against --two
    if two_others > 2 {
        out.push(format!("{} non-center blocks take 2 moves (the core's edge + `--two` others; families' default two = 1)", two_others));
    }
    // providers of the door blocks' moves
    for mv in m.moves.iter().filter(|mv| mv.phase == 0) {
        let Item::Door(i) = mv.item else { continue };
        let name = m.bname(i);
        let from = local(&m.spec, mv.from);
        let to = local(&m.spec, mv.to);
        match (mv.by, mv.how) {
            (Some(j), How::Push(k)) if k > 2 => out.push(format!(
                "{name} ({},{})->({},{}) is pushed by {} from {k} cells behind, through other blocks (providers: a pusher 1-2 behind: Gen::providers)",
                from[0], from[1], to[0], to[1], m.pname(j)
            )),
            (Some(j), How::Pull) if mv.by_ext_rest && !mv.by_displaced => {
                let first = m.paths_open[i].len() == 2;
                if first {
                    out.push(format!(
                        "{name} exits in 1 move pulled by {} EXTENDED at rest (only the --no-ext-pullers generator, g1, refuses that: Gen::others)",
                        m.pname(j)
                    ))
                }
            }
            (Some(j), _) if mv.by_displaced => {
                let p = &m.pistons[j];
                let cur = m.moves.iter().filter(|x| x.item == Item::Piston(j) && x.tick <= mv.tick && x.phase == 0).last().map(|x| x.to).unwrap_or(p.rest);
                let d = local_dir(&m.spec, sub(cur, p.rest));
                let along = fam_facing(local_dir(&m.spec, p.facing.step())).map(|f| fam::F[f as usize]).map(|(fx, fy)| d[0] * fy as i32 == d[1] * fx as i32).unwrap_or(false);
                let dist = d[0].abs() + d[1].abs();
                if p.ext_rest {
                    out.push(format!(
                        "{name} is moved by {} which is EXTENDED at rest and then carried (lifts and --carry reuse only pistons RETRACTED at rest: Gen::providers `&& !ext`)",
                        m.pname(j)
                    ));
                }
                if dist > 1 || !along {
                    out.push(format!(
                        "{name} is moved by {} displaced {dist} cell(s) {} its axis (lifts: 1 cell along the axis; --carry: 1-2 cells any direction)",
                        m.pname(j),
                        if along { "along" } else { "across" }
                    ));
                }
            }
            (Some(j), How::Drag) => out.push(format!("{name} is dragged by {}'s structure (slime / honey: not in families' model)", m.pname(j))),
            (None, _) => out.push(format!("{name} ({},{})->({},{}): no acting piston found (0-tick / spit?)", from[0], from[1], to[0], to[1])),
            _ => {}
        }
    }
    out.sort();
    out.dedup();
    out
}

/// `door connect`'s verdict (connect.rs: regions, conduction, wake-ups, every event order) on a
/// mechanism (families pieces + its macro run) placed in the box `bb` (door coordinates).
fn connect_own_box(pieces: &[Pc], open: &str, close: &str, sym: Option<String>, bb: [i32; 4], cap: usize) -> String {
    use crate::mechsynth::{Kind as MKind, Solution, World as MWorld};
    crate::mechsynth::SEAMLESS.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::mechsynth::SEAMLESS_OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
    let mut b = Vec::new();
    for x in 0..3 {
        for y in 0..3 {
            b.push(([x, y, 0], MKind::Door));
        }
    }
    for p in pieces {
        match crate::topology::parse_piece(&p.code()) {
            Ok(pc) => b.push(pc),
            Err(e) => return format!("- ({e})"),
        }
    }
    b.sort();
    let pow = MWorld::rest_pow(&b);
    let w = MWorld { b, pow };
    let seq = |s: &str| -> anyhow::Result<Vec<crate::mechsynth::Ev>> { s.split_whitespace().map(|t| crate::topology::parse_event(&w, t)).collect() };
    let (Ok(o), Ok(c)) = (seq(open), seq(close)) else { return "- (sequences)".into() };
    let sol = Solution { open: o, close: c, sym: sym.and_then(|s| seq(&s).ok()), states: 0 };
    let v = crate::connect::check(&w, &sol, [bb[0] as i8, bb[1] as i8], [bb[2] as i8, bb[3] as i8], cap);
    if v.ok { "WIRABLE".into() } else { format!("NOT wirable ({})", v.report.lines().last().unwrap_or("").trim()) }
}

/// One door's section-2 row.
pub struct MechRow {
    pub name: String,
    pub summary: String,
    pub members: Vec<(String, String)>,
    pub solver: String,
    pub rules: Vec<String>,
    pub tsv: Option<String>,
}

pub fn mech_row(m: &Mechanism, cap: usize) -> MechRow {
    let mut summary = String::new();
    let pieces = m.pieces();
    let bx = m.bx();
    let _ = write!(
        summary,
        "{} mechanism pistons ({}), {} signal pistons ({}), static frame {:?}, frame {}/12 open, box {}x{} (x {}..{}, y {}..{}){}{}",
        m.mech.len(),
        m.mech.iter().map(|j| m.pname(*j)).collect::<Vec<_>>().join(" "),
        m.signal_pistons().len(),
        m.signal_pistons().iter().map(|j| m.pname(*j)).collect::<Vec<_>>().join(" "),
        m.static_frame.iter().map(|q| (q[0], q[1])).collect::<Vec<_>>(),
        m.frame_open,
        bx.w,
        bx.h,
        bx.x0,
        bx.x0 + bx.w - 1,
        bx.y0,
        bx.y0 + bx.h - 1,
        if m.lost > 0 { format!(", {} tracking losses", m.lost) } else { String::new() },
        if m.body_in_door { ", a piston body enters the 3x3" } else { "" }
    );
    let mut rules = path_rules(m);
    let mut members = Vec::new();
    let mut hard: Vec<String> = Vec::new();
    if !m.off_plane.is_empty() {
        hard.push(format!("not flat: {}", m.off_plane.join(", ")));
    }
    if !m.carried_parts.is_empty() {
        hard.push(format!("the mechanism pistons also move {} (families' pieces are pistons, stone and obsidian only)", m.carried_parts.iter().cloned().collect::<Vec<_>>().join(", ")));
    }
    if m.mech.is_empty() {
        hard.push("no mechanism piston found".into());
    }
    if (bx.w as usize) * (bx.h as usize) > fam::MAXC {
        hard.push(format!("box {}x{} > families' MAXC {} cells", bx.w, bx.h, fam::MAXC));
    }
    if m.mech.len() > 16 {
        hard.push(format!("{} pistons > 16 (families' state holds 16)", m.mech.len()));
    }
    let mut solver = "-".to_string();
    let mut tsv = None;
    match (&pieces, hard.is_empty()) {
        (Some(p), true) => {
            for c in gen_cfgs() {
                let ok = member(p, bx, &c);
                members.push((c.name.to_string(), if ok { "IN".to_string() } else { "out".to_string() }));
            }
            // the generator's own diagnosis (widest bounds) when even they leave it out, and for
            // the catalog defaults when only they do
            let widest = gen_cfgs().last().cloned().unwrap();
            let allow: HashSet<Pc> = p.iter().copied().collect();
            // the door's real center exit direction (its first move while opening)
            let prefer = m.paths_open.iter().find(|v| v[0][0] == 1 && v[0][1] == 1).filter(|v| v.len() > 1).map(|v| ((v[1][0] - v[0][0]) as i8, (v[1][1] - v[0][1]) as i8));
            if members.iter().all(|(_, v)| v == "out") {
                let _guard = GEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                set_ext(&widest);
                for w in fam::explain_within(bx, widest.pmax, widest.two, widest.carry, &allow, prefer) {
                    rules.push(format!("generator (every flag): {w}"));
                }
                reset_ext();
            } else if members.get(1).is_some_and(|(_, v)| v == "out") {
                // the catalog's default generator (g2) leaves it out
                let _guard = GEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                reset_ext();
                for w in fam::explain_within(bx, 10, 1, 0, &allow, prefer) {
                    rules.push(format!("generator (catalog defaults): {w}"));
                }
            } else if members.first().is_some_and(|(_, v)| v == "out") {
                // only the retracted-pullers generator (before 2026-10-19) leaves it out
                let _guard = GEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                set_ext(&gen_cfgs()[0]);
                for w in fam::explain_within(bx, 10, 1, 0, &allow, prefer) {
                    rules.push(format!("generator (--no-ext-pullers): {w}"));
                }
                reset_ext();
            }
            // the macro model on the piece set (+ the static frame stones)
            let mut with_st = p.clone();
            with_st.extend(m.stones());
            match fam::Mech::new(bx, p) {
                None => {
                    solver = "macro model refuses the rest state (pieces overlap, or a piston held on at rest QC-arms a retracted one below it: Mech::new)".into();
                    rules.push(solver.clone());
                }
                Some((mm, rest)) => {
                    let ring = mm.rest_ring_clean(&rest);
                    match fam::solve(&mm, &rest, cap, true) {
                        Ok(Some(sol)) => {
                            solver = format!("macro solve: open {} + close {}{}{}", sol.open.len(), sol.close.len(), if sol.sym.is_some() { ", sym" } else { "" }, if ring { "" } else { "; rest ring NOT clean (pistons/heads in the frame at rest)" });
                            let sym = sol.sym.as_ref().map(|s| mm.fmt_seq(s)).unwrap_or_else(|| "-".into());
                            tsv = Some(format!(
                                "{}\t{}x{}\t{},{}\t{}\t{}\t{}\t{}\t{}x{}\t{}\t{}\t{}\t{}\t-\t-\t{}/12",
                                m.name,
                                bx.w,
                                bx.h,
                                -bx.x0,
                                -bx.y0,
                                p.len(),
                                sol.open.len(),
                                sol.close.len(),
                                sol.sym.as_ref().map(|s| s.len()).unwrap_or(0),
                                bx.w,
                                bx.h,
                                p.iter().map(|q| q.code()).collect::<Vec<_>>().join(" "),
                                mm.fmt_seq(&sol.open),
                                mm.fmt_seq(&sol.close),
                                sym,
                                m.frame_open
                            ));
                            // the topology heuristic and connect's region model on the door's own
                            // mechanism in the door's own box (a working door must pass both)
                            if let Some(bb) = m.build_box {
                                let bbx = Bx { x0: bb[0] as i8, y0: bb[1] as i8, w: (bb[2] - bb[0] + 1) as i8, h: (bb[3] - bb[1] + 1) as i8 };
                                if let Some((mm2, _)) = fam::Mech::new(bbx, p) {
                                    solver.push_str(&format!(", topology in its own box: {}", fam::topology_of(&mm2, &sol)));
                                }
                                let v = connect_own_box(p, &mm.fmt_seq(&sol.open), &mm.fmt_seq(&sol.close), sol.sym.as_ref().map(|s| mm.fmt_seq(s)), bb, cap);
                                solver.push_str(&format!(", connect in its own {}x{} box: {v}", bb[2] - bb[0] + 1, bb[3] - bb[1] + 1));
                            } else {
                                solver.push_str(&format!(", topology in the mechanism box: {}", fam::topology_of(&mm, &sol)));
                            }
                        }
                        Ok(None) => solver = "macro solve: NO open+close (FULL seamless) in families' model".into(),
                        Err(()) => solver = format!("macro solve: state cap {cap} hit"),
                    }
                }
            }
        }
        _ => {
            for c in gen_cfgs() {
                members.push((c.name.to_string(), "out".to_string()));
            }
            rules.splice(0..0, hard.iter().cloned());
        }
    }
    MechRow { name: m.name.clone(), summary, members, solver, rules, tsv }
}

// ---------------------------------------------------------------------------------------------
// Section 3: what the wiring uses that each wiring tool cannot represent

fn prop<'a>(s: &'a str, k: &str) -> Option<&'a str> {
    let inner = s.split_once('[')?.1.trim_end_matches(']');
    inner.split(',').find_map(|kv| kv.split_once('=').filter(|(a, _)| *a == k).map(|(_, v)| v))
}

fn short(s: &str) -> &str {
    let s = s.trim_start_matches("minecraft:");
    s.split('[').next().unwrap_or(s)
}

/// A wiring tool's verdict on one block state: Ok, or the reason it cannot hold it.
type Verdict = std::result::Result<(), String>;

/// satwire (satwire.rs): mechanism blocks by `mk`, wiring by `Kind::of_state`, the door plane only.
fn satwire_block(s: &str, depth: i32, is_lever: bool) -> Verdict {
    let n = short(s);
    if is_lever {
        // either rest polarity: main's satwire auto-detects a lever ON at rest (`lever_on_rest`)
        if depth != 0 {
            return if prop(s, "face") == Some("wall") { Ok(()) } else { Err("lever off the plane not hung on a block's front face".into()) };
        }
        return Ok(());
    }
    if depth != 0 {
        return Err(format!("{n} off the door plane (one z layer)"));
    }
    match crate::satwire::mk(s) {
        crate::satwire::MK::Piston { f, .. } if f.0 != 0 && f.1 != 0 || f == (0, 0) => return Err(format!("{n} facing out of the plane")),
        crate::satwire::MK::Piston { .. } | crate::satwire::MK::Other | crate::satwire::MK::Air => return Ok(()),
        _ => {}
    }
    match crate::satwire::Kind::of_state(s) {
        Some(k) => {
            let f = prop(s, "facing");
            let ok_f = match k {
                crate::satwire::Kind::Rep(..) | crate::satwire::Kind::Cmp(_) | crate::satwire::Kind::WTorch(_) => matches!(f, Some("east" | "west")),
                crate::satwire::Kind::Obs(_) => matches!(f, Some("east" | "west" | "up" | "down")),
                _ => true,
            };
            if ok_f { Ok(()) } else { Err(format!("{n} facing {}", f.unwrap_or("?"))) }
        }
        None => {
            // not a wiring kind: the encoding treats it as a mechanism block by `mk`
            let plain = matches!(crate::satwire::mk(s), crate::satwire::MK::Cond | crate::satwire::MK::Solid);
            let known_plain = cs::convert_state(s).ok().is_some_and(|b| cs::full_cube(&b) && cs::quartz_like(&b) || matches!(b, Block::Simple(_)));
            if plain && known_plain {
                Ok(())
            } else if ["redstone_block", "target"].contains(&n) {
                Err(format!("{n}: stripped as wiring but not in the wiring palette (Kind::of_state)"))
            } else if ["repeater", "observer", "comparator", "redstone_wall_torch"].contains(&n) {
                Err(format!("{n} facing {} (in-plane facings only)", prop(s, "facing").unwrap_or("?")))
            } else {
                Err(format!("{n}: not a wiring kind, modeled as a {} mechanism block (`mk`)", if plain { "plain conductor" } else { "non-conductor" }))
            }
        }
    }
}

/// cosearch search (cosearch_search.rs `palette("all")`, the door plane, in-plane facings).
fn cosearch_block(s: &str, depth: i32, is_lever: bool, pal: &HashSet<(String, Option<String>)>) -> Verdict {
    let n = short(s);
    if is_lever {
        // main (pockets 3d3f5c27): either polarity; a lever hung on a plane block's front or back
        // face (`Grid.lever_dz`)
        if depth != 0 && !(depth.abs() == 1 && prop(s, "face") == Some("wall")) {
            return Err("lever off the door plane, not on a block's front / back face".into());
        }
        return Ok(());
    }
    if depth != 0 {
        return Err(format!("{n} off the door plane"));
    }
    if matches!(n, "piston_head" | "moving_piston") {
        return Ok(());
    }
    if n == "redstone_wire" {
        return Ok(());
    }
    let b = match cs::convert_state(s) {
        Ok(b) => b,
        Err(e) => return Err(format!("{n}: redsim has no model ({e})")),
    };
    if matches!(b, Block::Simple(_)) && cs::full_cube(&b) {
        return Ok(()); // plain blocks: stone's class (redsim::classes) or glass/obsidian in the palette
    }
    let f = prop(s, "facing").map(|x| x.to_string());
    if pal.contains(&(n.to_string(), f.clone())) || pal.contains(&(n.to_string(), None)) {
        return Ok(());
    }
    if matches!(n, "dropper" | "dispenser") && pal.iter().any(|(k, _)| k == "dropper") {
        return if n == "dropper" { Ok(()) } else { Err("dispenser (palette: droppers only)".into()) };
    }
    if pal.iter().any(|(k, _)| k == n) {
        Err(format!("{n} facing {} (palette: in-plane facings only)", f.unwrap_or_default()))
    } else {
        Err(format!("{n}: not in the palette"))
    }
}

fn cosearch_palette() -> HashSet<(String, Option<String>)> {
    let mut out = HashSet::new();
    if let Ok(v) = crate::cosearch_search::palette("all") {
        for b in v {
            let s = b.to_string();
            out.insert((short(&s).to_string(), prop(&s, "facing").map(|x| x.to_string())));
        }
    }
    out
}

/// boxproof (boxproof/diff.rs `cell_of_state`, one z layer, in-plane facings).
fn boxproof_block(s: &str, depth: i32, is_lever: bool) -> Verdict {
    let n = short(s);
    if is_lever {
        // main (fd8c52b3): `lever_on_rest`; in the box, outside it, or out of the plane (--oop)
        return Ok(());
    }
    if depth != 0 {
        return Err(format!("{n} off the door plane"));
    }
    if crate::boxproof::diff::cell_of_state(s).is_none() {
        // plain blocks of other kinds: stone's class would do, but the cell set is fixed
        let plain = cs::convert_state(s).ok().is_some_and(|b| matches!(b, Block::Simple(_)) && cs::full_cube(&b));
        return Err(if plain { format!("{n}: not a boxproof cell (stone/smooth stone/cobblestone, obsidian, glass only, class-equivalent to stone)") } else { format!("{n}: not a boxproof cell") });
    }
    if let Some(f) = prop(s, "facing") {
        if matches!(f, "north" | "south") && !matches!(n, "lever" | "dropper") {
            return Err(format!("{n} facing {f} (in-plane facings only)"));
        }
    }
    Ok(())
}

/// Mechanics a design's wiring relies on, read from the design and its recorded mechanism.
pub fn mechanics(d: &Design, m: Option<&Mechanism>) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let Ok(spec) = DoorSpec::from_design(d) else { return out };
    let lever = d.blocks.get(&spec.lever).cloned().unwrap_or_default();
    if prop(&lever, "powered") == Some("true") {
        out.push(("lever-on", "the lever is ON at rest (the first flip turns it off)".into()));
    }
    if local(&spec, spec.lever)[2] != 0 {
        out.push(("lever-oop", format!("the lever is off the door plane ({})", prop(&lever, "face").unwrap_or("?"))));
    }
    let mut steps = 0;
    for (p, s) in &d.blocks {
        let n = short(s);
        if n == "redstone_wire" && ["north", "east", "south", "west"].iter().any(|k| prop(s, k) == Some("up")) {
            steps += 1;
        }
        if n == "observer" {
            if let Some(f) = prop(s, "facing").and_then(dir_of) {
                let w = add(*p, f);
                if let Some(t) = d.blocks.get(&w) {
                    let tn = short(t);
                    if matches!(tn, "dropper" | "dispenser" | "note_block" | "redstone_lamp" | "waxed_copper_bulb" | "iron_trapdoor" | "crafter" | "hopper" | "target") || tn.ends_with("trapdoor") {
                        let kind = if matches!(tn, "dropper" | "dispenser") { "obs-dropper" } else if matches!(tn, "note_block" | "redstone_lamp") { "obs-note-lamp" } else { "obs-state" };
                        out.push((kind, format!("observer {:?} reads the {tn} at {:?} (a state change, not a move)", local(&spec, *p), local(&spec, w))));
                    }
                }
            }
        }
        if matches!(n, "dropper" | "dispenser") {
            out.push(("dropper", format!("{n} at {:?} (an immovable conductor / power sensor)", local(&spec, *p))));
        }
    }
    if steps > 0 {
        out.push(("dust-step", format!("{steps} dust connect up a block (steps)")));
    }
    if let Some(m) = m {
        for j in m.signal_pistons() {
            let p = &m.pistons[j];
            let h = add(p.rest, p.facing.step());
            let watched = d.blocks.iter().any(|(q, s)| short(s) == "observer" && prop(s, "facing").and_then(dir_of).is_some_and(|f| add(*q, f) == h));
            let what = if p.moved_other.is_empty() { "moves nothing".to_string() } else { format!("moves {}", p.moved_other.join(", ")) };
            out.push((
                "signal-piston",
                format!("{} {} piston {} {what}{}", if p.sticky { "sticky" } else { "normal" }, if p.fires > 0 { "firing" } else { "idle" }, m.pname(j), if watched { ", its head cell watched by an observer" } else { "" }),
            ));
        }
        for &j in &m.mech {
            if m.pistons[j].ext_rest {
                out.push(("level-held", format!("{} is extended at rest (held by a level)", m.pname(j))));
            }
            if m.pistons[j].carried {
                out.push(("carried", format!("{} is carried ({} cells visited)", m.pname(j), m.pistons[j].visited.len())));
            }
        }
        for i in 0..9 {
            let n = m.paths_open[i].len() - 1;
            if n >= 3 {
                out.push(("moves3", format!("{} moves {n} times while opening", m.bname(i))));
            }
        }
        if m.frame_open == 12 && !m.static_frame.is_empty() {
            out.push(("static-frame", format!("{} static frame blocks", m.static_frame.len())));
        }
        for o in &m.carried_parts {
            out.push(("carried-part", format!("the mechanism moves a {o}")));
        }
    }
    out
}

fn dir_of(f: &str) -> Option<Pos> {
    Some(match f {
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        _ => return None,
    })
}

/// Per mechanic: (satwire, cosearch, boxproof) — Ok or why not.
fn mechanic_support(kind: &str) -> [Verdict; 3] {
    let ok = || Ok(());
    let no = |s: &str| Err(s.to_string());
    match kind {
        "lever-on" => [ok(), ok(), ok()],
        "lever-oop" => [ok(), ok(), ok()],
        "dust-step" => [ok(), ok(), ok()],
        "dropper" => [ok(), ok(), ok()],
        "obs-dropper" | "obs-note-lamp" => [ok(), ok(), ok()],
        "obs-state" => [ok(), ok(), no("no cell for bulbs / trapdoors / crafters / hoppers / targets")],
        "signal-piston" => [no("plan mode: no pistons in the wiring palette (from-wired records it as mechanism)"), ok(), ok()],
        "level-held" => [ok(), ok(), ok()],
        "carried" => [ok(), ok(), ok()],
        "moves3" => [ok(), ok(), ok()],
        "static-frame" => [ok(), ok(), ok()],
        "carried-part" => [no("mechanism recorded; a carried redstone block / observer is not a wiring kind"), ok(), ok()],
        _ => [ok(), ok(), ok()],
    }
}

pub struct WireRow {
    pub name: String,
    pub blocks: [Vec<String>; 3],
    pub mechs: Vec<(&'static str, String, [Verdict; 3])>,
}

pub fn wire_row(name: &str, d: &Design, m: Option<&Mechanism>, pal: &HashSet<(String, Option<String>)>) -> WireRow {
    let mut blocks: [BTreeMap<String, usize>; 3] = Default::default();
    let spec = DoorSpec::from_design(d).ok();
    let door_cells: HashSet<Pos> = spec.map(|s| s.door_cells().into_iter().collect()).unwrap_or_default();
    for (p, s) in &d.blocks {
        if door_cells.contains(p) {
            continue;
        }
        let depth = spec.map(|sp| local(&sp, *p)[2]).unwrap_or(0);
        // the lever port (gadgets have no door: any lever is their input)
        let is_lever = spec.map_or(short(s) == "lever", |sp| sp.lever == *p);
        let v = [satwire_block(s, depth, is_lever), cosearch_block(s, depth, is_lever, pal), boxproof_block(s, depth, is_lever)];
        for (k, r) in v.into_iter().enumerate() {
            if let Err(e) = r {
                *blocks[k].entry(e).or_default() += 1;
            }
        }
    }
    let to_vec = |b: &BTreeMap<String, usize>| b.iter().map(|(k, n)| if *n > 1 { format!("{k} (x{n})") } else { k.clone() }).collect::<Vec<_>>();
    let mechs = mechanics(d, m).into_iter().map(|(k, w)| (k, w, mechanic_support(k))).collect();
    WireRow { name: name.to_string(), blocks: [to_vec(&blocks[0]), to_vec(&blocks[1]), to_vec(&blocks[2])], mechs }
}

// ---------------------------------------------------------------------------------------------
// Section 4: hard-coded rules, found by pattern in the sources

pub struct Rule {
    pub name: &'static str,
    pub file: &'static str,
    pub pattern: &'static str,
    pub excludes: &'static str,
}

pub fn rules() -> Vec<Rule> {
    vec![
        Rule { name: "judge: door cells = 9 identical plain full blocks, no mechanism block", file: "lib.rs", pattern: r"fn closed_problems", excludes: "doors whose closed face mixes blocks (glass + stone), or uses a mechanism block (observer/slime) as a door block; the community allows any 'full block' set (rules: identical)" },
        Rule { name: "judge: lever port must hold a lever (either polarity since cc4686d)", file: "lib.rs", pattern: r#"starts_with\("minecraft:lever"\)"#, excludes: "buttons, pressure plates, or a repeater/observer input as the port (the community's input device is any switch)" },
        Rule { name: "judge: nothing in front of / behind the door at any distance", file: "lib.rs", pattern: r"in_front_and_behind\(", excludes: "a lever or dust placed in the tunnel columns (vjeux's rule: intended)" },
        Rule { name: "judge: settle at placement (raw paste, wire shapes, one self update each, then idle)", file: "lib.rs", pattern: r#""self"\)"#, excludes: "designs resting with a BUD- or QC-armed piston: INTENDED (vjeux 2026-10-19: no QC-armed piston at rest); a rest state that depends on placement order is buildable's job" },
        Rule { name: "judge: every block passes canSurvive at rest", file: "lib.rs", pattern: r"static_problems\(", excludes: "nothing real (a floating attachment pops in survival)" },
        Rule { name: "judge: no random ticks (isRandomlyTicking)", file: "lib.rs", pattern: r"random", excludes: "unwaxed copper, leaves (non-persistent), crops: intended" },
        Rule { name: "judge: open = 9 door cells + walkway walkable, no moving block", file: "lib.rs", pattern: r"fn open_problems", excludes: "doors that are open while a block is still moving there (bouncing allowed only between settled states)" },
        Rule { name: "judge: close = some closed state repeats an earlier one (PERIODIC rest, main 96a62d6; before: identical to the rest state)", file: "lib.rs", pattern: r"periodic = true|if !periodic", excludes: "nothing now in the judge; doors whose internal state alternates were rejected before 96a62d6" },
        Rule { name: "door ideal: periodic rest (FIXED explore/audit 9040a14; was exact rest after each close)", file: "ideal.rs", pattern: r"seen_closed.iter\(\).any", excludes: "before 9040a14: periodic-rest mechanisms failed `door ideal` and every tool replaying plans with it (fullmech --verify-ideal, families --verify, satsweep validation)" },
        Rule { name: "props-dyn env_run: periodic rest, surroundings exact (FIXED explore/audit 5026010)", file: "properties_dyn.rs", pattern: r"seen_closed.iter\(\).any", excludes: "before 5026010: a periodic-rest door failed every dynamic property (record eligibility, --eligible / --repeater-input / --seamless +hall searches)" },
        Rule { name: "local-search screen_in: periodic rest (FIXED explore/audit 38f4b6f)", file: "search.rs", pattern: r"seen_closed.iter\(\).any", excludes: "before 38f4b6f: periodic-rest candidates of minimize/regrow/grow/harvest were screened out before the judge saw them" },
        Rule { name: "redsim screen: periodic rest (FIXED explore/audit a4c5a84)", file: "redsim_screen.rs", pattern: r"seen_closed.iter\(\).any", excludes: "before a4c5a84: same for the redsim pre-screen / redsim-verify" },
        Rule { name: "tile judge: periodic rest (FIXED explore/audit 610a8ab)", file: "tile.rs", pattern: r"seen_closed.iter\(\).any", excludes: "before 610a8ab: periodic tile modules" },
        Rule { name: "cosearch screen: periodic rest (FIXED main 3d3f5c27, pockets)", file: "cosearch_search.rs", pattern: r"the periodic rest rule", excludes: "before 3d3f5c27: periodic-rest candidates lost points in the annealing screen" },
        Rule { name: "STILL exact-rest: boxproof (close ends at exactly the rest state)", file: "boxproof/bmc.rs", pattern: r"exactly the rest state", excludes: "periodic-rest doors are outside boxproof's space (its UNSATs do not cover them)" },
        Rule { name: "STILL exact-rest: satwire (both stable states fixed points, each trace ends where the other starts)", file: "satwire.rs", pattern: r"each trace ends where the other", excludes: "periodic-rest wirings (e.g. T-flip-flop phases)" },
        Rule { name: "STILL exact-rest: fullmech / mech-synth / families macro (close back to the exact rest state)", file: "fullmech.rs", pattern: r"Close = back to the exact rest state", excludes: "mechanisms that close into a different but equivalent internal state (two-phase machines)" },
        Rule { name: "placement = video.rs::place (raw paste in block_list order)", file: "video.rs", pattern: r"pub fn place", excludes: "states only reachable by a build order (e.g. a piston extended without power): judged unstable" },
        Rule { name: "legality: FORBIDDEN list + no orphan piston heads", file: "lib.rs", pattern: r"FORBIDDEN", excludes: "nothing real (survival-only blocks)" },
        Rule { name: "category: solid = not gravity, not sculk, no entities", file: "lib.rs", pattern: r"pub fn category", excludes: "a solid door with a sculk sensor anywhere is ranked 'sculk' even if the sensor is unused" },
        Rule { name: "flat label (single_layer): every block in the door plane, the lever cell ignored (main d975c15)", file: "lib.rs", pattern: r"pub fn single_layer", excludes: "fixed: before d975c15 the community 7x9 as built (lever on (4,5)'s front face) was 7x9x2, not flat, off the flat boards and out of flat-only searches" },
        Rule { name: "cosearch search grid: an off-plane lever on a block's front/back face (FIXED main 3d3f5c27, pockets' Grid.lever_dz)", file: "cosearch_search.rs", pattern: r"pub lever_dz: i32", excludes: "before 3d3f5c27: c79's real lever (front face of (4,5)) could not seed or move in cosearch" },
        Rule { name: "redsim judge: falling blocks are full cubes (FIXED explore/audit 088f15e)", file: "cosearch.rs", pattern: r"Block::Falling \{ \.\. \} => true", excludes: "before 088f15e cosearch eval/search/agree rejected EVERY gravity door ('not closed at rest: door cell is minecraft:sand': 33/33 archived gravity doors)" },
        Rule { name: "acceptance positions: 4 + extra (default 28) everywhere, never fewer than the 4 default positions", file: "search.rs", pattern: r"pub fn judge_offsets", excludes: "a door that works at 1-3 positions (vjeux: a 1-position pass should count as a find) is never reported by minimize/grow/regrow/resynth/synth (min 4), cosearch (main: hard-coded 32, cosearch_search.rs extra_offsets(28)), satwire (--extra-offsets 28), families/fullmech ideal replays (4)" },
        Rule { name: "boxproof bmc: the door placed at dx = (w-3)/2, dy = (h-3)/2 unless --dx/--dy", file: "boxproof/bmc.rs", pattern: r#"a.get\("dx""#, excludes: "off-center placements (dx 0,1,3,4 in 7 wide; dy 3 in 8 tall): a 7x8 run covers 1 of 10 placements" },
        Rule { name: "mechspace --door-offset (the single-U sweep ran with 2,2 only)", file: "mechspace.rs", pattern: r"DOOR_OFFSET.get\(\)", excludes: "off-center 7-wide / 8-tall placements of the c78 single-U sweep" },
        Rule { name: "flat-only seeds: cosearch search refuses non-flat seeds", file: "cosearch_search.rs", pattern: r"not flat", excludes: "fixed by d975c15 for front-face levers (single_layer ignores the lever)" },
        Rule { name: "frame cells must hold the DOOR block itself when open (d975c15); the 3 extras may move", file: "sheet.rs", pattern: r"pub fn filled_frame", excludes: "frame-12 tests that add STATIC stone (sheet --fill, mechspace --full-frame: 'frame cells air when open stay air on the cycle') exclude doors whose 3 extra frame blocks move" },
        Rule { name: "mechspace --full-frame: static stones in the frame cells left air when open; movable extras (rule A, try_movable) since main 4f8f0604", file: "mechspace.rs", pattern: r"pub fn try_movable", excludes: "before 4f8f0604: doors whose 3 extra frame blocks are moved into place (vjeux: the extras may move)" },
        Rule { name: "verify prints the bounding box with the lever", file: "../../door-judge/src/lib.rs", pattern: r"if crate::single_layer\(", excludes: "the printed box (7x9x2) disagrees with the community volume (7x9x1, lever excluded)" },
        Rule { name: "props-dyn repeater rig + door properties' repeater input: polarity (FIXED explore/audit 4c483b9)", file: "properties_dyn.rs", pattern: r"let rest_on = lever_state", excludes: "before 4c483b9 every lever-ON door failed repeater input ('not open'): c79 was not record-eligible and --eligible / --repeater-input searches rejected its family; now ELIGIBLE (x32, 15/15)" },
        Rule { name: "cosearch screen: either lever polarity (FIXED main, cosearch2 c85570e)", file: "cosearch_search.rs", pattern: r"Block::Lever \{ \.\. \}\) && !\(p\.no_lever", excludes: "before: lever-ON seeds (c79) scored as broken (max-1 points)" },
        Rule { name: "minimize lever moves keep the design's polarity (FIXED explore/audit 9b1074d)", file: "search.rs", pattern: r#"let pw = if ls.contains\("powered=true"\)"#, excludes: "before 9b1074d every lever move of a lever-ON door turned its lever off and broke it" },
        Rule { name: "relever keeps the design's polarity (FIXED explore/audit 9b1074d)", file: "relever.rs", pattern: r"let polar = ", excludes: "before 9b1074d a lever-ON door's relever variants were all broken" },
        Rule { name: "mech-wire: levers placed unpowered (its waves assume lever-off)", file: "mechwire.rs", pattern: r"lever\[face=floor,facing=north,powered=false\]", excludes: "wirings that need the lever's level at rest (level-held pullers released by the lever, c79's right side): a --lever-on option must invert every wave" },
        Rule { name: "satwire: lever polarity auto-detected (FIXED main, `lever_on_rest`)", file: "satwire.rs", pattern: r"!= self\.p\.lever_on_rest", excludes: "before: lever-ON wirings; c79's `--from-wired --check` went from 157/485 piston-tick mismatches to 0/482" },
        Rule { name: "satwire: written levers keep the rest polarity (FIXED main)", file: "satwire.rs", pattern: r"if p\.lever_on_rest \{ lc\.state\.replace", excludes: "-" },
        Rule { name: "boxproof: rest lever ON option (FIXED main fd8c52b3)", file: "boxproof/bmc.rs", pattern: r"lever_on_rest", excludes: "before fd8c52b3: lever-ON doors" },
        Rule { name: "boxproof: cells = air/stone/obsidian/glass/note/lamp/rblock/dust/rep/obs/torch/piston + dropper, copper bulb, comparator (fd8c52b3)", file: "boxproof/diff.rs", pattern: r"pub fn cell_of_state", excludes: "slime, honey, iron trapdoors, slabs, subtract-mode comparators, leaves and logs, rails, sculk (droppers, bulbs, comparators added in fd8c52b3)" },
        Rule { name: "families: 1-move exits by RETRACTED sticky pullers only with --no-ext-pullers (extended-at-rest pullers are every generator's default since 2026-10-19)", file: "families.rs", pattern: r"if !EXT_PULLERS\.load\(Ordering::Relaxed\) \|\| ext\.0 == 0", excludes: "with --no-ext-pullers only: level-held pullers released by the lever (c79's MR/TR, SW+(4,1) SW+(4,2); the community 7x8's TR, SD+(2,4)); by default at most --ext-max (3) such exits per mechanism, each next to an earlier one (--ext-apart)" },
        Rule { name: "families: paths of 1-2 unit moves, no reversal", file: "families.rs", pattern: r"fn paths\(", excludes: "a door block moving 3 times or back (c79's TM: down, up, up) - only harmless when the pieces are generated for another nominal path" },
        Rule { name: "families: the center leaves straight in exactly 2 moves", file: "families.rs", pattern: r"fn core\(", excludes: "a center that exits in 1 move sideways, 3 moves, or bends" },
        Rule { name: "families: providers = a sticky puller 2 ahead or a pusher 1-2 behind", file: "families.rs", pattern: r"fn providers\(", excludes: "a block pushed through a chain from 3+ cells (c79's BM is pushed from (1,3) through TM and C; generated anyway via its (1,-2) pull)" },
        Rule { name: "families: a lifted or carried provider must be RETRACTED at rest (`!ext` in the lift / pull-back / carry branches)", file: "families.rs", pattern: r"p\.x \+ fx == ax && p\.y \+ fy == ay && !ext|p\.x - fx == ax && p\.y - fy == ay && !ext|if self\.carry > 0 && !ext", excludes: "19 archived flat doors under every flag (hA-full-rl-dropper 14, eligible-s4ff 3, fresh-a-sf1-w2 2): SD+(1,4) extended at rest retracts, is pushed down by SD(1,5), then pulls C" },
        Rule { name: "families: only pieces an earlier provider PLACED are reused (lifted / carried)", file: "families.rs", pattern: r"lifted toward its facing: rest \+ f = action", excludes: "39 archived flat doors under every flag (fast-clean 21, speed-wire 18): SD(0,3) acts only after SE(-1,3) carries it to (1,3); (0,3) is no provider position, so it is never placed" },
        Rule { name: "families: pistons only (no stone, obsidian, slime, droppers, carried sources) in generated sets", file: "families.rs", pattern: r"fn add\(&self, v: &\[Pc\], p: Pc\)", excludes: "static frame stones (frame 12/12: mechspace --full-frame adds them after the solve), helper blocks, carried redstone blocks / observers" },
        Rule { name: "families: a piston held on at rest above a retracted piston is refused (QC-armed rest)", file: "families.rs", pattern: r"QC-arms a retracted piston right below it", excludes: "rests where the QC-armed piston is meant to stay (blocked) or fire on the first update" },
        Rule { name: "families: box <= 144 cells, <= 16 pistons", file: "families.rs", pattern: r"pub const MAXC", excludes: "boxes over 12x12" },
        Rule { name: "families: default P <= 10 (catalog), mechspace 11, fullmech 10", file: "../../door-mech/src/lib.rs", pattern: r"default_value_t = 10\)\]", excludes: "mechanisms with 11+ pistons (c79 has 10 mechanism pistons + 1 signal piston: families --check counts 11)" },
        Rule { name: "families/mechspace hole rule (swept ring cells air when open)", file: "families.rs", pattern: r"hole", excludes: "replaced by frame 12/12 (vjeux 03:36Z)" },
        Rule { name: "mechspace sub-model: no piston body inside the 3x3", file: "mechspace.rs", pattern: r"no_door_bodies|NO_DOOR_BODIES", excludes: "doors whose piston bodies cross the opening (none known among working flat doors; see the audit table)" },
        Rule { name: "topology heuristic (parity, regions); `families --check` runs it in the MECHANISM's own box", file: "families.rs", pattern: r"topology: \{\}", excludes: "label: `families --check` prints 'topology: no' for c79 (no free cell in the mechanism's bounding box); in each door's own build box topology says 'strict' for c79 and every flat archived door (68 on main dfe893a)" },
        Rule { name: "connect: only the lever's region awake at each flip (or the lever outside the box on a never-moved piston's back face / a static's outer face); regions = free cells outside the footprint; sealed single cells hold an observer only", file: "connect.rs", pattern: r"pub fn check\(", excludes: "no known door: WIRABLE for c79 (real 7x9 placement; NOT in 7x8 or 7x9 shifted up) and every flat archived door in its own box (1396 on main 0303f70e6 + d67's connect; 6 of 16 sampled were NOT wirable before the outside lever: the lever-held top trio)" },
        Rule { name: "sheet piston count excludes heads (FIXED explore/audit add7f90)", file: "sheet.rs", pattern: r"piston bases only", excludes: "label only: c79 printed '14 pistons' (11 + 3 heads)" },
        Rule { name: "sheet (designs): flip 1 = open", file: "sheet.rs", pattern: r"fn draw\(", excludes: "a design file of an OPEN state (c79-open-b) draws 'frame 3/12' (door cycle handles it)" },
        Rule { name: "properties CLEAN: never moves back", file: "properties.rs", pattern: r"moves back", excludes: "label: c79's TM moves down then up (correct per regulation 2.1.5.3)" },
    ]
}

pub fn scan_rules(src: &Path) -> Vec<(String, Vec<String>, String)> {
    let mut out = Vec::new();
    for r in rules() {
        let path = src.join(r.file);
        let mut hits = Vec::new();
        if let (Ok(text), Ok(re)) = (std::fs::read_to_string(&path), regex::Regex::new(r.pattern)) {
            for (i, l) in text.lines().enumerate() {
                if re.is_match(l) {
                    hits.push(format!("{}:{}", r.file.trim_start_matches("../../"), i + 1));
                    if hits.len() >= 4 {
                        break;
                    }
                }
            }
        }
        out.push((r.name.to_string(), hits, r.excludes.to_string()));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Section 1: checks (redsim in-process; the JVM ones with --jvm)

pub fn redsim_checks(name: &str, d: &Design, door: &RsDoor) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let t = std::time::Instant::now();
    let e = cs::evaluate(door, &cs::JudgeOpts::with_extra(28), true);
    out.push(("redsim judge + FULL + hallway + volume (cosearch eval)".into(), cs::eval_line(name, &e, t.elapsed().as_secs_f64() * 1000.0)));
    // door cycle: alternation + frame 12/12 at the judge's 4 positions
    let mut cyc = Vec::new();
    for off in crate::VerifyOptions::default().offsets {
        match cs::flip_states(door, off, 6) {
            Ok(snaps) => {
                let spec = door.spec.translated(off);
                let frame = crate::sheet::frame_cells(spec.door);
                let open_at = |s: &Snap| spec.door_cells().iter().all(|c| s.get(*c).is_some_and(|b| b.is_air()));
                let door_block = snaps.iter().find(|s| !open_at(s)).and_then(|s| s.get(spec.door_cells()[0])).unwrap_or(Block::Air);
                let rest_open = open_at(&snaps[0]);
                let mut ok = true;
                let mut why = String::new();
                for (k, s) in snaps.iter().enumerate() {
                    let want_open = (k % 2 == 1) != rest_open;
                    if open_at(s) != want_open {
                        ok = false;
                        why = format!("flip {k}: {}", if want_open { "not open" } else { "not closed" });
                        break;
                    }
                    if want_open {
                        let n = frame.iter().filter(|c| s.get(**c).is_some_and(|b| crate::sheet::filled_frame(&b, &door_block))).count();
                        if n < 12 {
                            ok = false;
                            why = format!("flip {k}: frame {n}/12");
                            break;
                        }
                    }
                }
                cyc.push(if ok { "ok".to_string() } else { why });
            }
            Err(e) => cyc.push(e),
        }
    }
    out.push(("door cycle (6 flips x 4 positions, frame 12/12 open)".into(), if cyc.iter().all(|c| c == "ok") { "CYCLES".into() } else { cyc.join("; ") }));
    let pistons = d.blocks.values().filter(|s| matches!(short(s), "piston" | "sticky_piston")).count();
    let heads = d.blocks.values().filter(|s| short(s) == "piston_head").count();
    out.push(("door sheet piston label".into(), format!("{pistons} pistons (heads excluded since add7f90; before: {} with the {heads} heads)", pistons + heads)));
    let flat = crate::single_layer(d);
    let spec = door.spec;
    let flat_no_lever = d.blocks.keys().filter(|p| **p != spec.lever).all(|p| local(&spec, *p)[2] == 0);
    out.push((
        "flat label (single_layer)".into(),
        match (flat, flat_no_lever) {
            (true, _) => "flat".into(),
            (false, true) => "MISLABEL: not flat only because of the lever (off the plane); the volume excludes the lever".into(),
            _ => "not flat".into(),
        },
    ));
    out.push(("category".into(), format!("{}{}", crate::category(d), if crate::slime_free(d) { ", slime-free" } else { "" })));
    out
}

pub fn jvm_checks(w: &mut mctas::sim::Worker, name: &str, d: &Design) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let opts = crate::VerifyOptions::default();
    out.push((
        "door verify (real judge, 4 positions)".into(),
        match crate::verify(w, d, &opts) {
            Ok(r) => format!("WORKS ({} blocks, {}x{}x{}{}{})", r.blocks, r.size[0], r.size[1], r.size[2], if crate::single_layer(d) { ", flat" } else { "" }, if crate::slime_free(d) { ", slime-free" } else { "" }),
            Err(e) => format!("FAILS: {e:#}"),
        },
    ));
    match crate::properties::compute(w, d, false) {
        Ok(p) => {
            let g = |k: &str| p.get(k).cloned().unwrap_or_default();
            out.push(("door properties".into(), format!("seamless {} | volume {} ({}) | clean {} | lever surface {}", g("seamless"), g("volume"), g("volume_dims"), g("clean"), g("lever_surface"))));
        }
        Err(e) => out.push(("door properties".into(), format!("ERROR: {e:#}"))),
    }
    match crate::properties_dyn::check_design(w, name, d, "working") {
        Ok(r) => {
            let m = |c: &std::result::Result<(), String>| match c {
                Ok(()) => "yes".to_string(),
                Err(e) => format!("NO ({e})"),
            };
            out.push(("props-dyn hallway".into(), m(&r.hallway)));
            out.push(("props-dyn snow".into(), m(&r.snow)));
            out.push(("props-dyn repeater input".into(), m(&r.repeater)));
            out.push(("props-dyn steps".into(), m(&r.steps)));
            out.push(("props-dyn 20 cycles".into(), m(&r.cycles20)));
            out.push(("props-dyn contained".into(), r.contained.iter().map(|(k, c)| format!("{k} {}", if c.is_ok() { "ok" } else { "NO" })).collect::<Vec<_>>().join(", ")));
        }
        Err(e) => out.push(("props-dyn".into(), format!("ERROR: {e:#}"))),
    }
    let (ok, line) = crate::properties_dyn::eligibility_line(w, name, d, &crate::search::judge_offsets(0));
    out.push(("door eligible (4 positions)".into(), format!("{}: {line}", if ok { "ELIGIBLE" } else { "not eligible" })));
    out
}

// ---------------------------------------------------------------------------------------------
// The command

pub struct Opts {
    pub designs: Vec<PathBuf>,
    pub sections: String,
    pub jvm: bool,
    pub out: Option<PathBuf>,
    pub tsv: Option<PathBuf>,
    pub threads: usize,
    pub cap: usize,
    pub version: String,
    pub src: PathBuf,
    pub only_flat: bool,
}

fn expand(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p).into_iter().flatten().flatten().map(|e| e.path()).filter(|q| q.extension().is_some_and(|x| x == "txt")).collect();
            v.sort();
            out.extend(v);
        } else {
            out.push(p.clone());
        }
    }
    out
}

fn cell(v: &Verdict) -> String {
    match v {
        Ok(()) => "ok".into(),
        Err(e) => e.replace('|', "/"),
    }
}

pub fn run(o: &Opts) -> Result<()> {
    let files = expand(&o.designs);
    let want = |s: &str| o.sections.split(',').any(|x| x.trim() == s || x.trim() == "all");
    let mut md = String::new();
    let _ = writeln!(md, "<!-- generated by `mctas door audit` -->\n");
    // load every design as a redsim door (designs redsim cannot model are listed)
    let loaded: Vec<(String, std::result::Result<(Design, RsDoor), String>)> = files
        .iter()
        .map(|f| {
            let name = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            (name, cs::load_door(f).map_err(|e| format!("{e:#}")))
        })
        .collect();
    if want("checks") {
        let _ = writeln!(md, "## 1. Checks\n\n| design | check | result |\n|---|---|---|");
        let mut worker = if o.jvm { Some(mctas::sim::Worker::spawn(&mctas::sim::WorkerConfig { version: o.version.clone(), heap_mb: 768, ..Default::default() })?) } else { None };
        for (name, l) in &loaded {
            match l {
                Ok((d, door)) => {
                    let mut rows = redsim_checks(name, d, door);
                    if let Some(w) = worker.as_mut() {
                        rows.extend(jvm_checks(w, name, d));
                    }
                    for (c, r) in rows {
                        let _ = writeln!(md, "| {name} | {c} | {} |", r.replace('|', "/"));
                    }
                }
                Err(e) => {
                    let _ = writeln!(md, "| {name} | load | {} |", e.replace('|', "/"));
                }
            }
        }
        let _ = writeln!(md);
    }
    let mechs: Vec<(String, std::result::Result<Mechanism, String>)> = if want("mech") || want("wiring") {
        let items: Vec<&(String, std::result::Result<(Design, RsDoor), String>)> = loaded.iter().collect();
        let res = cs::par_map(&items, o.threads, |(name, l)| match l {
            Ok((_, door)) => mechanism(name, door),
            Err(e) => Err(e.clone()),
        });
        loaded.iter().map(|(n, _)| n.clone()).zip(res).collect()
    } else {
        Vec::new()
    };
    if want("mech") {
        let cfgs = gen_cfgs();
        let _ = writeln!(md, "## 2. Mechanism generators\n");
        let _ = writeln!(
            md,
            "Membership = families' own generator (`families::generate_within`) restricted to the door's mechanism pistons, in the box its mechanism sweeps; `IN` = the exact piece set is one of the generator's candidates (mirror included). Columns:\n"
        );
        for (k, c) in cfgs.iter().enumerate() {
            let _ = writeln!(md, "* g{}: {} [{}]", k + 1, c.name, c.tool);
        }
        let _ = writeln!(md, "\n| design | mechanism | {} | macro solver | bounds the real moves break |\n|---|---|{}---|---|", (1..=cfgs.len()).map(|k| format!("g{k}")).collect::<Vec<_>>().join(" | "), "---|".repeat(cfgs.len()));
        let rows: Vec<(String, std::result::Result<MechRow, String>)> = {
            let items: Vec<&(String, std::result::Result<Mechanism, String>)> = mechs.iter().collect();
            let res = cs::par_map(&items, o.threads, |(_, m)| match m {
                Ok(m) => Ok(mech_row(m, o.cap)),
                Err(e) => Err(e.clone()),
            });
            mechs.iter().map(|(n, _)| n.clone()).zip(res).collect()
        };
        let mut tsv = String::from("id\tbox\tdoor_offset\tpistons\topen_len\tclose_len\tsym_len\textent\tpieces\topen\tclose\tsym\tsim\tfiles\tframe\n");
        let mut counts = vec![0usize; cfgs.len()];
        let mut flat_n = 0;
        for (name, r) in &rows {
            match r {
                Ok(row) => {
                    if o.only_flat && row.members.iter().all(|(_, v)| v == "out") && row.rules.iter().any(|r| r.starts_with("not flat")) {
                        continue;
                    }
                    if !row.rules.iter().any(|r| r.starts_with("not flat")) {
                        flat_n += 1;
                    }
                    for (k, (_, v)) in row.members.iter().enumerate() {
                        if v == "IN" {
                            counts[k] += 1;
                        }
                    }
                    let _ = writeln!(
                        md,
                        "| {name} | {} | {} | {} | {} |",
                        row.summary.replace('|', "/"),
                        row.members.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>().join(" | "),
                        row.solver.replace('|', "/"),
                        if row.rules.is_empty() { "-".to_string() } else { row.rules.join("; ").replace('|', "/") }
                    );
                    if let Some(t) = &row.tsv {
                        tsv.push_str(t);
                        tsv.push('\n');
                    }
                }
                Err(e) => {
                    if !o.only_flat {
                        let _ = writeln!(md, "| {name} | no mechanism: {} |{} - | - |", e.replace('|', "/"), " - |".repeat(cfgs.len()));
                    }
                }
            }
        }
        let _ = writeln!(md, "\n{} designs with a flat mechanism; IN per column: {}\n", flat_n, counts.iter().enumerate().map(|(k, c)| format!("g{} {c}", k + 1)).collect::<Vec<_>>().join(", "));
        if let Some(p) = &o.tsv {
            std::fs::write(p, &tsv).with_context(|| format!("write {}", p.display()))?;
            eprintln!("wrote {} (families catalog format: `door connect`, `door topology`, `boxproof catalog`)", p.display());
        }
    }
    if want("wiring") {
        let pal = cosearch_palette();
        let _ = writeln!(md, "## 3. Wiring tools\n\n| design | satwire cannot hold | cosearch cannot hold | boxproof cannot hold | mechanics used (satwire / cosearch / boxproof) |\n|---|---|---|---|---|");
        for ((name, l), (_, m)) in loaded.iter().zip(mechs.iter()) {
            let Ok((d, _)) = l else {
                // gadgets and designs redsim cannot load: palette only, from the file
                if let Ok(d) = crate::cli::load_design(&files[loaded.iter().position(|x| x.0 == *name).unwrap_or(0)]) {
                    let row = wire_row(name, &d, None, &pal);
                    let _ = writeln!(md, "| {name} | {} | {} | {} | (not a door: palette only) |", row.blocks[0].join("; "), row.blocks[1].join("; "), row.blocks[2].join("; "));
                }
                continue;
            };
            let row = wire_row(name, d, m.as_ref().ok(), &pal);
            let mech = row
                .mechs
                .iter()
                .map(|(k, w, v)| format!("{k}: {w} ({} / {} / {})", cell(&v[0]), cell(&v[1]), cell(&v[2])))
                .collect::<Vec<_>>()
                .join("; ");
            let e = |v: &Vec<String>| if v.is_empty() { "-".to_string() } else { v.join("; ").replace('|', "/") };
            let _ = writeln!(md, "| {name} | {} | {} | {} | {} |", e(&row.blocks[0]), e(&row.blocks[1]), e(&row.blocks[2]), mech.replace('|', "/"));
        }
        let _ = writeln!(md);
    }
    if want("rules") {
        let _ = writeln!(md, "## 4. Hard-coded rules\n\n| rule | where (live) | what real designs it excludes |\n|---|---|---|");
        for (n, hits, ex) in scan_rules(&o.src) {
            let _ = writeln!(md, "| {n} | {} | {} |", if hits.is_empty() { "(pattern not found)".to_string() } else { hits.join(", ") }, ex.replace('|', "/"));
        }
        let _ = writeln!(md);
    }
    match &o.out {
        Some(p) => {
            std::fs::write(p, &md)?;
            eprintln!("wrote {}", p.display());
        }
        None => print!("{md}"),
    }
    Ok(())
}

/// Print one design's extracted mechanism (paths, moves, providers) for a human.
/// `door audit --member FILE`: generator membership of mechanisms given as piece lists (ideal-input
/// designs have no wiring to cycle): lines `W H x0 y0 piece piece ...` (the box in door
/// coordinates; `#` comments). Prints one row per line with g1..gN and, when every generator
/// leaves it out, why (the widest generator's explanation).
pub fn member_file(path: &Path) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("{}", path.display()))?;
    let cfgs = gen_cfgs();
    println!("| mechanism | {} | why (widest generator) |", (1..=cfgs.len()).map(|k| format!("g{k}")).collect::<Vec<_>>().join(" | "));
    println!("|---|{}---|", "---|".repeat(cfgs.len()));
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let t: Vec<&str> = line.split_whitespace().collect();
        anyhow::ensure!(t.len() > 4, "line {line:?}: expected W H x0 y0 pieces...");
        let (w, h, x0, y0): (i8, i8, i8, i8) = (t[0].parse()?, t[1].parse()?, t[2].parse()?, t[3].parse()?);
        let bx = Bx { x0, y0, w, h };
        // static frame stones (`stone(x,y)`) are not generator pieces: membership of the pistons
        let target: Vec<Pc> = t[4..].iter().map(|s| crate::mechspace::parse_piece(s)).collect::<Result<Vec<_>>>()?.into_iter().filter(|p| !matches!(p.k, K::Stone)).collect();
        let cols: Vec<&str> = cfgs.iter().map(|c| if member(&target, bx, c) { "IN" } else { "out" }).collect();
        let why = if cols.iter().all(|c| *c == "out") {
            let widest = cfgs.last().cloned().unwrap();
            let allow: HashSet<Pc> = target.iter().copied().collect();
            let _guard = GEN_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            set_ext(&widest);
            let w = fam::explain_within(bx, widest.pmax, widest.two, widest.carry, &allow, None);
            reset_ext();
            w.join("; ")
        } else {
            String::new()
        };
        println!("| {w}x{h} ({x0},{y0}) {} | {} | {why} |", t[4..].join(" "), cols.join(" | "));
    }
    Ok(())
}

pub fn explain(path: &Path) -> Result<()> {
    let (_, door) = cs::load_door(path)?;
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let m = mechanism(&name, &door).map_err(|e| anyhow::anyhow!(e))?;
    println!("{name}: door block {}", m.door_state);
    for i in 0..9 {
        println!("  {:>2} open {}  close {}", m.bname(i), Mechanism::fmt_path(&m.paths_open[i]), Mechanism::fmt_path(&m.paths_close[i]));
    }
    for (j, p) in m.pistons.iter().enumerate() {
        println!(
            "  piston {} {} fires {} moved door {} pistons {:?} other {:?} visited {:?}",
            m.pname(j),
            if m.mech.contains(&j) { "MECH" } else { "signal" },
            p.fires,
            p.moved_door,
            p.moved_pistons.iter().map(|k| m.pname(*k)).collect::<Vec<_>>(),
            p.moved_other,
            p.visited.iter().map(|c| { let q = local(&m.spec, *c); (q[0], q[1]) }).collect::<Vec<_>>()
        );
    }
    for mv in &m.moves {
        let what = match mv.item {
            Item::Door(i) => m.bname(i),
            Item::Piston(j) => m.pname(j),
            Item::Other => block_name(&mv.moved),
        };
        let f = local(&m.spec, mv.from);
        let t = local(&m.spec, mv.to);
        println!(
            "  {} t{:>2} {what} ({},{})->({},{}) by {} {:?}{}",
            if mv.phase == 0 { "open " } else { "close" },
            mv.tick,
            f[0],
            f[1],
            t[0],
            t[1],
            mv.by.map(|j| m.pname(j)).unwrap_or("?".into()),
            mv.how,
            if mv.by_displaced { " (displaced)" } else { "" }
        );
    }
    let row = mech_row(&m, 200_000);
    println!("{}", row.summary);
    for (c, v) in &row.members {
        println!("  {v:>3}  {c}");
    }
    println!("  {}", row.solver);
    for r in &row.rules {
        println!("  - {r}");
    }
    Ok(())
}
