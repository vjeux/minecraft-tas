//! The 0-9 display of one signal strength (`mctas circuit display compact7`, clk-obs 2026-10-22):
//! designs/briefs/display-7seg.md layouts 4 + 2 with cascades. Two dust ladders from the chest's
//! comparator (cell d holds s - d); per segment its runs, chained so that each segment ends in ONE
//! signal that one routed lane carries to the face module:
//! - a top run [lo, 15] is a repeater tap reading ladder cell lo - 1, pointing into a block R;
//! - a bottom run [0, hi] is a torch hung on the support of cell hi;
//! - a single value v is local: a tap at cell v - 1 powers R, a subtract comparator W reads R on its
//!   rear and the tap at cell v points into its side (W lit iff s = v);
//! - a longer run [lo, hi] is W reading a tap at cell lo - 1 on its rear and a fixed repeater on its
//!   side, fed through a socket by a lane from a tap at cell hi;
//! - a cascade: a run's output pointing into the next run's rear block R (its upper bound higher)
//!   makes the next one output the union.
//! Lanes are routed by crate::route in strict mode where the target is a block read by a diode.

use crate::abuild::{B, D, P};
use crate::route::{Cell, Kind, Lane};
use anyhow::{Result, anyhow};
use circuit::compute::{CORNERS, FACE, INTERIOR};

const CROSS: &str = "redstone_wire[east=side,north=side,south=side,west=side]";

/// The face module (crate::obvious): lamps at z = fz facing -z, col c at x = fx - c; returns the
/// seven face sockets (segments a..g), each a walled cell whose dust goes in after routing.
/// With `direct`, no face repeaters or sockets: the returned targets are the segment stones
/// themselves, for strict lanes whose last repeater points into the stone from z + 3.
pub fn face_with(b: &mut B, fx: i32, fy: i32, fz: i32, direct: bool) -> Vec<P> {
    if !direct {
        return face(b, fx, fy, fz);
    }
    let at = |col: i32, row: i32, out: i32| -> P { [fx - col, fy + 4 - row, fz + out] };
    let mut targets = Vec::new();
    for s in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
        let (_, col, row) = FACE.iter().find(|(n, _, _)| n.starts_with(s)).copied().unwrap();
        b.stone(at(col, row, 2));
        b.set(at(col, row, 1), CROSS);
        b.needs.push(at(col, row, 1));
        targets.push(at(col, row, 2));
    }
    for (_, col, row) in CORNERS {
        b.stone(at(col, row, 1));
        b.set(at(col, row, 2), CROSS);
        b.needs.push(at(col, row, 2));
    }
    for (col, row) in INTERIOR {
        b.stone(at(col, row, 1));
    }
    b.stone(at(1, 5, 1));
    // (no stones behind the corner dusts: the router already keeps lane dust and blocks off them, and
    // a stone beside a goal counts as a diode at the last repeater's side, refusing the goal)
    for (s, col, row) in FACE.iter().chain(CORNERS.iter()) {
        b.port(&format!("output.{s}"), at(*col, *row, 0));
    }
    targets
}

/// The face module (crate::obvious): lamps at z = fz facing -z, col c at x = fx - c; returns the
/// seven face sockets (segments a..g), each a walled cell whose dust goes in after routing.
pub fn face(b: &mut B, fx: i32, fy: i32, fz: i32) -> Vec<P> {
    let at = |col: i32, row: i32, out: i32| -> P { [fx - col, fy + 4 - row, fz + out] };
    let mut sockets = Vec::new();
    for s in ['a', 'b', 'c', 'd', 'e', 'f', 'g'] {
        let (_, col, row) = FACE.iter().find(|(n, _, _)| n.starts_with(s)).copied().unwrap();
        b.stone(at(col, row, 2));
        b.set(at(col, row, 1), CROSS);
        b.needs.push(at(col, row, 1));
        b.rep(at(col, row, 3), D::N, 1);
        let socket = at(col, row, 4);
        b.stone([socket[0], socket[1] - 1, socket[2]]);
        for p in [[socket[0] - 1, socket[1], socket[2]], [socket[0] + 1, socket[1], socket[2]], [socket[0], socket[1] + 1, socket[2]]] {
            b.blocks.entry(p).or_insert_with(|| "minecraft:stone".into());
        }
        sockets.push(socket);
    }
    for (_, col, row) in CORNERS {
        b.stone(at(col, row, 1));
        b.set(at(col, row, 2), CROSS);
        b.needs.push(at(col, row, 2));
    }
    for (col, row) in INTERIOR {
        b.stone(at(col, row, 1));
    }
    b.stone(at(1, 5, 1));
    for (col, row) in [(0, 0), (2, 0), (0, 2), (2, 2), (0, 4), (2, 4), (1, 1), (1, 3), (-1, 1), (3, 1), (-1, 3), (3, 3)] {
        b.blocks.entry(at(col, row, 3)).or_insert_with(|| "minecraft:stone".into());
    }
    for (s, col, row) in FACE.iter().chain(CORNERS.iter()) {
        b.port(&format!("output.{s}"), at(*col, *row, 0));
    }
    sockets
}

/// A ladder track: ladder 0 runs +x from the reader's block, ladder 1 runs -x; side +1 / -1 is the
/// track's z side.
#[derive(Clone, Copy)]
pub struct Track {
    pub ladder: u8,
    pub side: i32,
}

impl Track {
    pub fn x(self, d: i32) -> i32 {
        if self.ladder == 0 { 1 + d } else { -1 - d }
    }
    /// Row k (1: the slot beside the support, 2, 3, ... outward) at cell d.
    pub fn at(self, d: i32, k: i32) -> P {
        [self.x(d), 1, k * self.side]
    }
    pub fn out(self) -> D {
        if self.side > 0 { D::S } else { D::N }
    }
    pub fn along(self) -> D {
        if self.ladder == 0 { D::E } else { D::W }
    }
}

/// A signal ready to leave: a repeater (position, facing) whose output starts a lane, or a block a
/// cascade can feed.
#[derive(Clone, Copy, Debug)]
pub struct Src {
    pub pos: P,
    pub dir: D,
}

pub struct Ctx {
    pub b: B,
    pub lanes: Vec<Lane>,
}

impl Ctx {
    /// A torch on the support of cell d (lit iff s <= d); returns the repeater reading it.
    pub fn torch(&mut self, t: Track, d: i32) -> Src {
        let f = if t.side > 0 { "south" } else { "north" };
        self.b.set(t.at(d, 1), &format!("redstone_wall_torch[facing={f},lit=true]"));
        self.b.rep(t.at(d, 2), t.out(), 1);
        Src { pos: t.at(d, 2), dir: t.out() }
    }
    /// A tap reading cell d (on iff s > d) pointing into its block R (row 2); returns R.
    pub fn tap_block(&mut self, t: Track, d: i32) -> P {
        self.b.rep(t.at(d, 1), t.out(), 1);
        self.b.stone(t.at(d, 2));
        t.at(d, 2)
    }
    /// A bare tap (its own output starts a lane).
    pub fn tap(&mut self, t: Track, d: i32) -> Src {
        self.b.rep(t.at(d, 1), t.out(), 1);
        Src { pos: t.at(d, 1), dir: t.out() }
    }
    /// A top run [lo, 15] whose block R can take a cascade; returns (R, the repeater reading R).
    pub fn top(&mut self, t: Track, lo: i32) -> (P, Src) {
        let r = self.tap_block(t, lo - 1);
        self.b.rep(t.at(lo - 1, 3), t.out(), 1);
        (r, Src { pos: t.at(lo - 1, 3), dir: t.out() })
    }
    /// A single value v: (R, W's output cell, as a src = W itself facing along).
    pub fn single(&mut self, t: Track, v: i32) -> (P, Src) {
        let r = self.tap_block(t, v - 1);
        self.b.comp(t.at(v, 2), t.along(), true);
        self.b.rep(t.at(v, 1), t.out(), 1);
        (r, Src { pos: t.at(v, 2), dir: t.along() })
    }
    /// A run [lo, hi] (hi > lo): rear tap at cell lo - 1 into R, W at row 3 (rear R), its side
    /// repeater at row 3 one cell along `m`, reading the socket stone two cells along (a lane from a
    /// tap at cell hi lands there). Returns (R, socket, the repeater reading W's output).
    pub fn window(&mut self, t: Track, lo: i32, m: i32) -> (P, P, Src) {
        let r = self.tap_block(t, lo - 1);
        let w = t.at(lo - 1, 3);
        self.b.comp(w, t.out(), true);
        let step = |d: i32, k: i32| -> P { t.at(lo - 1 + d, k) };
        let back_dir = if (m > 0) == (t.ladder == 0) { t.along().opp() } else { t.along() };
        self.b.rep(step(m, 3), back_dir, 1);
        self.b.stone(step(2 * m, 3));
        self.b.rep(t.at(lo - 1, 4), t.out(), 1);
        (r, step(2 * m, 3), Src { pos: t.at(lo - 1, 4), dir: t.out() })
    }
    /// A lane from `s` into the block `target`, its last repeater pointing `into`.
    pub fn link(&mut self, s: Src, target: P, into: D) {
        self.lanes.push(Lane { src: s.pos, src_dir: s.dir, lamp: target, into });
    }
    /// A lane into `target` from whichever horizontal side is free (preferring `pref`).
    pub fn link_free(&mut self, s: Src, target: P, pref: &[D]) -> Result<()> {
        let free = |p: P| !self.b.blocks.contains_key(&p);
        for &into in pref.iter().chain([D::N, D::S, D::E, D::W].iter()) {
            let v = into.v();
            let goal = [target[0] - v[0], target[1], target[2] - v[2]];
            if free(goal) && free([goal[0], goal[1] - 1, goal[2]]) && !self.lanes.iter().any(|l| l.lamp == target) {
                self.link(s, target, into);
                return Ok(());
            }
        }
        Err(anyhow!("no free side into {target:?}"))
    }
}

/// The 0-9 display: chains per segment on their own ladder tracks, then the face.
/// First try (00:30): stages on shared tracks, every cascade routed. Every lane failed even alone
/// (stages on each other's lane starts); kept as the record of the dead end.
pub fn build_v0(name: &str, out: &std::path::Path, expand: usize) -> Result<()> {
    let mut c = Ctx { b: B::default(), lanes: Vec::new() };
    // the chest, its reader and the trunk: P0 (0,2,0) holds s; ladders L0 (+x) and L1 (-x) start
    // beside it; a copy chain carries s to P2 (0,2,8), where L2 starts along +z
    c.b.set([0, 2, -2], "chest[facing=north,type=single,waterlogged=false]");
    c.b.port("input.a", [0, 2, -2]);
    c.b.comp([0, 2, -1], D::S, false);
    c.b.stone([0, 2, 0]);
    for z in 1..8 {
        if z % 2 == 1 {
            c.b.comp([0, 2, z], D::S, false);
        } else {
            c.b.stone([0, 2, z]);
        }
    }
    c.b.stone([0, 2, 8]);
    for d in 0..9 {
        c.b.dust([1 + d, 2, 0]);
        c.b.dust([-1 - d, 2, 0]);
        c.b.dust([0, 2, 9 + d]);
    }
    let l0 = |side: i32| Track3 { start: [1, 2, 0], along: D::E, out: if side > 0 { D::S } else { D::N } };
    let l1 = |side: i32| Track3 { start: [-1, 2, 0], along: D::W, out: if side > 0 { D::S } else { D::N } };
    let l2 = |side: i32| Track3 { start: [0, 2, 9], along: D::S, out: if side > 0 { D::E } else { D::W } };
    let (ta, tb, tc, td, te, tf) = (l0(1), l0(-1), l1(1), l1(-1), l2(1), l2(-1));
    let mut outs: Vec<(char, Src)> = Vec::new();
    // a: [0] -> [2,3] -> [5,15]
    {
        let s0 = c.torch3(ta, 0);
        let (r, sock, w) = c.window3(ta, 2, -1);
        let side = c.tap3(ta, 3);
        c.link_free(side, sock, &[])?;
        c.link_free(s0, r, &[])?;
        let (r5, o) = c.top3(ta, 5);
        c.link_free(w, r5, &[])?;
        outs.push(('a', o));
    }
    // d: [0] -> [2,3] -> [5,6] -> [8,15]
    {
        let s0 = c.torch3(tb, 0);
        let (r, sock, w) = c.window3(tb, 2, -1);
        let side = c.tap3(tb, 3);
        c.link_free(side, sock, &[])?;
        c.link_free(s0, r, &[])?;
        let (r56, sock56, w56) = c.window3(tb, 5, -1);
        let side56 = c.tap3(tb, 6);
        c.link_free(side56, sock56, &[])?;
        c.link_free(w, r56, &[])?;
        let (r8, o) = c.top3(tb, 8);
        c.link_free(w56, r8, &[])?;
        outs.push(('d', o));
    }
    // e: [0] -> [2] -> [6] -> [8] (the last two adjacent: [6]'s output is [8]'s rear block)
    {
        let s0 = c.torch3(tc, 0);
        let (r2, w2) = c.single3(tc, 2);
        c.link_free(s0, r2, &[])?;
        let (r6, _w6) = c.single3(tc, 6);
        c.link_free(w2, r6, &[])?;
        let (_r8, w8) = c.single3(tc, 8);
        outs.push(('e', w8));
    }
    // f: [0] -> [4,6] -> [8,15]
    {
        let s0 = c.torch3(td, 0);
        let (r, sock, w) = c.window3(td, 4, -1);
        let side = c.tap3(td, 6);
        c.link_free(side, sock, &[])?;
        c.link_free(s0, r, &[])?;
        let (r8, o) = c.top3(td, 8);
        c.link_free(w, r8, &[])?;
        outs.push(('f', o));
    }
    // g: [2,6] -> [8,15]
    {
        let (_r, sock, w) = c.window3(te, 2, -1);
        let side = c.tap3(te, 6);
        c.link_free(side, sock, &[])?;
        let (r8, o) = c.top3(te, 8);
        c.link_free(w, r8, &[])?;
        outs.push(('g', o));
    }
    // c: [0,1] -> [3,15]; b: [0,4] -> [7,15] (one track, cells 1-2 and 4-6)
    {
        let s1 = c.torch3(tf, 1);
        let (r3, o) = c.top3(tf, 3);
        c.link_free(s1, r3, &[])?;
        outs.push(('c', o));
        let s4 = c.torch3(tf, 4);
        let (r7, o) = c.top3(tf, 7);
        c.link_free(s4, r7, &[])?;
        outs.push(('b', o));
    }
    // the face, north of the chest (lamps facing -z)
    let (fx, fy, fz) = (1, 3, -16);
    let sockets = face(&mut c.b, fx, fy, fz);
    for (seg, o) in &outs {
        let j = "abcdefg".find(*seg).unwrap();
        c.link(*o, sockets[j], D::N);
    }
    c.b.supports();
    route_and_write(c, name, out, expand, [-16, -3, fz - 1], [14, 9, 26], &sockets)
}

/// A ladder track in any direction: cell d's dust at start + d * along (y = 2), its support below;
/// row k beside the support, k cells toward `out`.
#[derive(Clone, Copy)]
pub struct Track3 {
    pub start: P,
    pub along: D,
    pub out: D,
}

impl Track3 {
    pub fn at(self, d: i32, k: i32) -> P {
        let (a, o) = (self.along.v(), self.out.v());
        [self.start[0] + d * a[0] + k * o[0], self.start[1] - 1, self.start[2] + d * a[2] + k * o[2]]
    }
}

impl Ctx {
    pub fn torch3(&mut self, t: Track3, d: i32) -> Src {
        self.b.set(t.at(d, 1), &format!("redstone_wall_torch[facing={},lit=true]", t.out.name()));
        self.b.rep(t.at(d, 2), t.out, 1);
        Src { pos: t.at(d, 2), dir: t.out }
    }
    pub fn tap3(&mut self, t: Track3, d: i32) -> Src {
        self.b.rep(t.at(d, 1), t.out, 1);
        Src { pos: t.at(d, 1), dir: t.out }
    }
    pub fn top3(&mut self, t: Track3, lo: i32) -> (P, Src) {
        self.b.rep(t.at(lo - 1, 1), t.out, 1);
        self.b.stone(t.at(lo - 1, 2));
        self.b.rep(t.at(lo - 1, 3), t.out, 1);
        (t.at(lo - 1, 2), Src { pos: t.at(lo - 1, 3), dir: t.out })
    }
    pub fn single3(&mut self, t: Track3, v: i32) -> (P, Src) {
        self.b.rep(t.at(v - 1, 1), t.out, 1);
        self.b.stone(t.at(v - 1, 2));
        self.b.comp(t.at(v, 2), t.along, true);
        self.b.rep(t.at(v, 1), t.out, 1);
        (t.at(v - 1, 2), Src { pos: t.at(v, 2), dir: t.along })
    }
    /// A run [lo, hi]: returns (R, the socket stone its side repeater reads, W's output repeater).
    pub fn window3(&mut self, t: Track3, lo: i32, m: i32) -> (P, P, Src) {
        self.b.rep(t.at(lo - 1, 1), t.out, 1);
        self.b.stone(t.at(lo - 1, 2));
        self.b.comp(t.at(lo - 1, 3), t.out, true);
        let face = if m > 0 { t.along.opp() } else { t.along };
        self.b.rep(t.at(lo - 1 + m, 3), face, 1);
        self.b.stone(t.at(lo - 1 + 2 * m, 3));
        self.b.rep(t.at(lo - 1, 4), t.out, 1);
        (t.at(lo - 1, 2), t.at(lo - 1 + 2 * m, 3), Src { pos: t.at(lo - 1, 4), dir: t.out })
    }
}

/// Route every lane (strict mode, shuffled orders on failure), write the design.
fn route_and_write(c: Ctx, name: &str, out: &std::path::Path, expand: usize, lo: P, hi: P, sockets: &[P]) -> Result<()> {
    let Ctx { mut b, lanes } = c;
    let base = Cell { kind: Kind::Base, lane: -1, idx: 0, support: false, source: false };
    let mut order: Vec<usize> = (0..lanes.len()).collect();
    order.sort_by_key(|&k| (lanes[k].src[0] - lanes[k].lamp[0]).abs() + (lanes[k].src[2] - lanes[k].lamp[2]).abs());
    // each lane alone first: one that cannot route on an empty field is a layout bug, not congestion
    // dust sockets take a lane ending in a powered block (loose); stone targets read by the face's
    // dots need the last repeater pointing in (strict)
    let direct = std::env::var("DECIMAL7_TFACE").is_ok();
    let strict = direct;
    let mut alone_bad = Vec::new();
    for k in 0..lanes.len() {
        let mut w = crate::route::world_from(&b, &lanes, lo, hi);
        w.strict = strict;
        if w.route(k as i32, &lanes[k], expand).is_none() {
            alone_bad.push(k);
            eprintln!("lane {k} alone does not route: {:?} -> {:?} into {:?}", lanes[k].src, lanes[k].lamp, lanes[k].into.name());
        }
    }
    if !alone_bad.is_empty() {
        return Err(anyhow!("{} lanes do not route even alone", alone_bad.len()));
    }
    let mut rng: u64 = 0x2545_f491_4f6c_dd1d;
    let mut routed = None;
    for attempt in 0..80 {
        let mut w = crate::route::world_from(&b, &lanes, lo, hi);
        w.strict = strict;
        for z in [lo[2]] {
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    w.cells.entry([x, y, z]).or_insert(base);
                }
            }
        }
        match crate::route::route_all(&mut w, &lanes, &order, expand) {
            Ok(n) => {
                routed = Some((w, n));
                break;
            }
            Err(e) => {
                if attempt % 10 == 0 {
                    eprintln!("order {attempt}: {e}");
                }
                for i in (1..order.len()).rev() {
                    rng ^= rng << 13;
                    rng ^= rng >> 7;
                    rng ^= rng << 17;
                    order.swap(i, (rng % (i as u64 + 1)) as usize);
                }
            }
        }
    }
    let (w, n) = routed.ok_or_else(|| anyhow!("no routing order routes every lane ({} lanes)", lanes.len()))?;
    crate::route::write_lanes(&w, &mut b);
    if !direct {
        for s in sockets {
            b.blocks.insert(*s, format!("minecraft:{CROSS}"));
        }
    }
    if let Ok(bx) = std::env::var("DECIMAL7_DUMP") {
        let n: Vec<i32> = bx.split(',').map(|s| s.trim().parse().unwrap_or(0)).collect();
        let mut v: Vec<_> = b.blocks.iter().filter(|(p, _)| p[0] >= n[0] && p[0] <= n[1] && p[2] >= n[2] && p[2] <= n[3]).collect();
        v.sort_by_key(|(p, _)| (p[2], p[1], p[0]));
        for (p, s) in v {
            eprintln!("{p:?} {s}");
        }
    }
    let mut d = b.design(name, Some("seven-segment 4 hex decimal"));
    d.meta.insert("port input.a".into(), "strength".into());
    std::fs::write(out, format!("# {name} (clk-obs, 2026-10-22): the 0-9 display, designs/briefs/display-7seg.md (`mctas circuit display decimal7`)\n{}", d.to_text()))?;
    println!("{}: {} blocks ({n} lane blocks, {} lanes)", out.display(), d.blocks.len(), lanes.len());
    Ok(())
}

/// One segment's stages on its own track, all local: every stage ends in a repeater at row 3
/// pointing into the segment's dust bus at row 4 (an OR); the bus end feeds a repeater at row 5,
/// the face lane's source.
pub enum Stage {
    /// [0, hi]: a torch on the support of cell hi.
    Bottom(i32),
    /// [lo, 15]: a tap reading cell lo - 1.
    Top(i32),
    /// [lo, hi]: W at row 2 reading the tap at cell lo - 1 on its rear; its side fed by the tap at
    /// cell hi (a stone and a repeater chain along row 2; one dust when hi = lo).
    Run(i32, i32),
    /// {6, 8} (segment e): [6] facing along into [8]'s rear block; [8] points into the bus itself.
    SixEight,
}

impl Ctx {
    /// Lay out `stages` on track `t`; returns the face lane's source.
    pub fn chain(&mut self, t: Track3, stages: &[Stage]) -> Src {
        self.chain_on(t, stages, 4, false)
    }
    /// `bus` is the bus's row (3 or 4); stage outputs climb to it through repeaters. With `along`,
    /// the source repeater continues the bus along the track (toward a face at the ladder's far
    /// end); otherwise it faces out at row bus + 1.
    pub fn chain_on(&mut self, t: Track3, stages: &[Stage], bus: i32, along: bool) -> Src {
        let mut cells: Vec<i32> = Vec::new();
        // cells where row 2 holds a side line's stone or dust: a row-3 bus steps up over them
        let mut raise: Vec<i32> = Vec::new();
        let climb = |c: &mut Ctx, d: i32| {
            for k in 3..bus {
                c.b.rep(t.at(d, k), t.out, 1);
            }
        };
        for s in stages {
            match *s {
                Stage::Bottom(hi) => {
                    self.b.set(t.at(hi, 1), &format!("redstone_wall_torch[facing={},lit=true]", t.out.name()));
                    self.b.rep(t.at(hi, 2), t.out, 1);
                    climb(self, hi);
                    cells.push(hi);
                }
                Stage::Top(lo) => {
                    self.b.rep(t.at(lo - 1, 1), t.out, 1);
                    self.b.rep(t.at(lo - 1, 2), t.out, 1);
                    climb(self, lo - 1);
                    cells.push(lo - 1);
                }
                Stage::Run(lo, hi) => {
                    self.b.rep(t.at(lo - 1, 1), t.out, 1);
                    self.b.comp(t.at(lo - 1, 2), t.out, true);
                    self.b.rep(t.at(hi, 1), t.out, 1);
                    if hi == lo {
                        self.b.dust(t.at(hi, 2));
                        raise.push(hi);
                    } else if hi == lo + 1 {
                        self.b.stone(t.at(hi, 2));
                        self.b.rep(t.at(lo, 2), t.along.opp(), 1);
                        raise.push(hi);
                    } else {
                        // a dust line and one repeater into W's side: the side arrives within W's own
                        // 2 gt, no later (a repeater chain made W flash on: on-blips on x -> 7)
                        for j in lo + 1..=hi {
                            self.b.dust(t.at(j, 2));
                            raise.push(j);
                        }
                        self.b.rep(t.at(lo, 2), t.along.opp(), 1);
                    }
                    climb(self, lo - 1);
                    cells.push(lo - 1);
                }
                Stage::SixEight => {
                    self.b.rep(t.at(5, 1), t.out, 1);
                    self.b.stone(t.at(5, 2));
                    self.b.comp(t.at(6, 2), t.along, true);
                    self.b.rep(t.at(6, 1), t.out, 1);
                    self.b.rep(t.at(7, 1), t.out, 1);
                    self.b.stone(t.at(7, 2));
                    self.b.comp(t.at(7, 3), t.out, true);
                    self.b.rep(t.at(8, 1), t.out, 1);
                    self.b.rep(t.at(8, 2), t.out, 1);
                    self.b.dust(t.at(8, 3));
                    cells.push(7);
                }
            }
        }
        let (lo, hi) = (*cells.iter().min().unwrap(), *cells.iter().max().unwrap());
        for j in lo..=hi {
            let p = t.at(j, bus);
            if bus == 3 && raise.contains(&j) {
                // one block up: beside a powered stone (or a dust) a bus dust would read it
                self.b.stone(p);
                self.b.dust([p[0], p[1] + 1, p[2]]);
                if !self.b.blocks.get(&t.at(j, 2)).is_some_and(|s| s == "minecraft:stone") {
                    let q = t.at(j, 2);
                    self.b.stone([q[0], q[1] + 1, q[2]]);
                }
            } else {
                self.b.dust(p);
            }
        }
        if along {
            self.b.rep(t.at(hi + 1, bus), t.along, 1);
            Src { pos: t.at(hi + 1, bus), dir: t.along }
        } else {
            self.b.rep(t.at(hi, bus + 1), t.out, 1);
            Src { pos: t.at(hi, bus + 1), dir: t.out }
        }
    }
}

/// The 0-9 display (01:10): two ladders stacked at the reader (level 1 reached by a lossless climb:
/// a comparator's block powers the dust on top of it at full strength) and one copied west; the bus
/// ends gather in front of the face.
pub fn build(name: &str, out: &std::path::Path, expand: usize) -> Result<()> {
    use Stage::*;
    let mut c = Ctx { b: B::default(), lanes: Vec::new() };
    let gap: i32 = std::env::var("DECIMAL7_GAP").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    c.b.set([0, 2, 2], "chest[facing=south,type=single,waterlogged=false]");
    c.b.port("input.a", [0, 2, 2]);
    c.b.comp([0, 2, 1], D::N, false);
    c.b.stone([0, 2, 0]);
    // level 0: the reader's ladder north of P0
    for d in 0..9 {
        c.b.dust([0, 2, -1 - d]);
    }
    // the climb to level 1 (y = 5): P0's top dust, C1 east into B1, B1's top dust, C2 north into B2,
    // B2's top dust = level 1's cell 0
    c.b.dust([0, 3, 0]);
    c.b.comp([1, 3, 0], D::E, false);
    c.b.stone([1, 2, 0]);
    c.b.stone([2, 3, 0]);
    c.b.dust([2, 4, 0]);
    c.b.comp([2, 4, -1], D::N, false);
    c.b.stone([2, 3, -1]);
    c.b.stone([2, 4, -2]);
    for d in 0..9 {
        c.b.dust([2, 5, -2 - d]);
    }
    // the west copy: compare comparators and stones along -x into P2, its ladder north
    let stack3 = std::env::var("DECIMAL7_WEST").is_err();
    if stack3 {
        // the second climb, from D1 (2,4,0) east and up to level 2 (y = 8, cell 0 at (6,8,-4))
        c.b.comp([3, 4, 0], D::E, false);
        c.b.stone([3, 3, 0]);
        c.b.stone([4, 4, 0]);
        c.b.dust([4, 5, 0]);
        c.b.comp([4, 5, -1], D::N, false);
        c.b.stone([4, 4, -1]);
        c.b.stone([4, 5, -2]);
        c.b.dust([4, 6, -2]);
        c.b.comp([5, 6, -2], D::E, false);
        c.b.stone([5, 5, -2]);
        c.b.stone([6, 6, -2]);
        c.b.dust([6, 7, -2]);
        c.b.comp([6, 7, -3], D::N, false);
        c.b.stone([6, 6, -3]);
        c.b.stone([6, 7, -4]);
        for d in 0..9 {
            c.b.dust([6, 8, -4 - d]);
        }
    } else {
        for i in 1..gap {
            let p = [-i, 2, 0];
            if i % 2 == 1 {
                c.b.comp(p, D::W, false);
            } else {
                c.b.stone(p);
            }
        }
        c.b.stone([-gap, 2, 0]);
        for d in 0..9 {
            c.b.dust([-gap, 2, -1 - d]);
        }
    }
    let l0 = |side: i32| Track3 { start: [0, 2, -1], along: D::N, out: if side > 0 { D::E } else { D::W } };
    let l1 = |side: i32| Track3 { start: [2, 5, -2], along: D::N, out: if side > 0 { D::E } else { D::W } };
    let l2 = |side: i32| {
        let start = if stack3 { [6, 8, -4] } else { [-gap, 2, -1] };
        Track3 { start, along: D::N, out: if side > 0 { D::E } else { D::W } }
    };
    let tracks = [l0(1), l0(-1), l1(1), l1(-1), l2(1), l2(-1)];
    let assign = std::env::var("DECIMAL7_ASSIGN").unwrap_or_else(|_| "a,d,e,f,g,cb".into());
    let stages_of = |s: char| -> (Vec<Stage>, i32) {
        match s {
            'a' => (vec![Bottom(0), Run(2, 3), Top(5)], 3),
            'b' => (vec![Bottom(4), Top(7)], 3),
            'c' => (vec![Bottom(1), Top(3)], 3),
            'd' => (vec![Bottom(0), Run(2, 3), Run(5, 6), Top(8)], 3),
            'e' => (vec![Bottom(0), Run(2, 2), SixEight], 4),
            'f' => (vec![Bottom(0), Run(4, 6), Top(8)], 3),
            _ => (vec![Run(2, 6), Top(8)], 3),
        }
    };
    let mut outs = Vec::new();
    for (t, segs) in tracks.iter().zip(assign.split(',')) {
        let n = segs.chars().count();
        for (i, s) in segs.chars().enumerate() {
            let (st, bus) = stages_of(s);
            // two segments on one track: the first (low cells) faces out, the last runs along
            outs.push((s, c.chain_on(*t, &st, bus, i + 1 == n)));
        }
    }
    let (fx, fy, fz) = match std::env::var("DECIMAL7_FACE") {
        Ok(v) => {
            let n: Vec<i32> = v.split(',').map(|s| s.trim().parse().unwrap_or(0)).collect();
            (n[0], n[1], n[2])
        }
        Err(_) => (2, 4, -21),
    };
    let sockets = face_with(&mut c.b, fx, fy, fz, std::env::var("DECIMAL7_TFACE").is_ok());
    for (seg, o) in &outs {
        let j = "abcdefg".find(*seg).unwrap();
        c.link(*o, sockets[j], D::N);
    }
    c.b.supports();
    route_and_write(c, name, out, expand, [-20, -3, fz - 1], [12, 12, 6], &sockets)
}

/// The 0-9 display (00:55): three parallel ladders running north from the chest's reader (the side
/// two fed by lossless comparator copies), six tracks, every bus ending north toward the face.
pub fn build_parallel(name: &str, out: &std::path::Path, expand: usize) -> Result<()> {
    use Stage::*;
    let mut c = Ctx { b: B::default(), lanes: Vec::new() };
    let gap: i32 = std::env::var("DECIMAL7_GAP").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    c.b.set([0, 2, 2], "chest[facing=south,type=single,waterlogged=false]");
    c.b.port("input.a", [0, 2, 2]);
    c.b.comp([0, 2, 1], D::N, false);
    c.b.stone([0, 2, 0]);
    for i in 1..gap {
        for sx in [1, -1] {
            let p = [sx * i, 2, 0];
            if i % 2 == 1 {
                c.b.comp(p, if sx > 0 { D::E } else { D::W }, false);
            } else {
                c.b.stone(p);
            }
        }
    }
    c.b.stone([gap, 2, 0]);
    c.b.stone([-gap, 2, 0]);
    for x in [0, gap, -gap] {
        for d in 0..9 {
            c.b.dust([x, 2, -1 - d]);
        }
    }
    let lad = |x: i32, side: i32| Track3 { start: [x, 2, -1], along: D::N, out: if side > 0 { D::E } else { D::W } };
    let outs = vec![
        ('a', c.chain_on(lad(0, 1), &[Bottom(0), Run(2, 3), Top(5)], 3, true)),
        ('d', c.chain_on(lad(0, -1), &[Bottom(0), Run(2, 3), Run(5, 6), Top(8)], 3, true)),
        ('e', c.chain_on(lad(gap, -1), &[Bottom(0), Run(2, 2), SixEight], 4, true)),
        ('f', c.chain_on(lad(gap, 1), &[Bottom(0), Run(4, 6), Top(8)], 3, true)),
        ('g', c.chain_on(lad(-gap, 1), &[Run(2, 6), Top(8)], 3, true)),
        ('c', c.chain_on(lad(-gap, -1), &[Bottom(1), Top(3)], 3, false)),
        ('b', c.chain_on(lad(-gap, -1), &[Bottom(4), Top(7)], 3, true)),
    ];
    let (fx, fy, fz) = match std::env::var("DECIMAL7_FACE") {
        Ok(v) => {
            let n: Vec<i32> = v.split(',').map(|s| s.trim().parse().unwrap_or(0)).collect();
            (n[0], n[1], n[2])
        }
        Err(_) => (1, 3, -22),
    };
    let sockets = face(&mut c.b, fx, fy, fz);
    for (seg, o) in &outs {
        let j = "abcdefg".find(*seg).unwrap();
        c.link(*o, sockets[j], D::N);
    }
    c.b.supports();
    route_and_write(c, name, out, expand, [-20, -3, fz - 1], [20, 12, 6], &sockets)
}

/// The star layout (00:45): ladders east, west and south of the reader; 634-667 blocks, lanes
/// ~35 blocks each because the sources point six ways.
pub fn build_star(name: &str, out: &std::path::Path, expand: usize) -> Result<()> {
    use Stage::*;
    let mut c = Ctx { b: B::default(), lanes: Vec::new() };
    c.b.set([0, 2, -2], "chest[facing=north,type=single,waterlogged=false]");
    c.b.port("input.a", [0, 2, -2]);
    c.b.comp([0, 2, -1], D::S, false);
    c.b.stone([0, 2, 0]);
    for z in 1..8 {
        if z % 2 == 1 {
            c.b.comp([0, 2, z], D::S, false);
        } else {
            c.b.stone([0, 2, z]);
        }
    }
    c.b.stone([0, 2, 8]);
    for d in 0..9 {
        c.b.dust([1 + d, 2, 0]);
        c.b.dust([-1 - d, 2, 0]);
        c.b.dust([0, 2, 9 + d]);
    }
    let l0 = |side: i32| Track3 { start: [1, 2, 0], along: D::E, out: if side > 0 { D::S } else { D::N } };
    let l1 = |side: i32| Track3 { start: [-1, 2, 0], along: D::W, out: if side > 0 { D::S } else { D::N } };
    let l2 = |side: i32| Track3 { start: [0, 2, 9], along: D::S, out: if side > 0 { D::E } else { D::W } };
    let outs = vec![
        ('a', c.chain(l0(1), &[Bottom(0), Run(2, 3), Top(5)])),
        ('d', c.chain(l0(-1), &[Bottom(0), Run(2, 3), Run(5, 6), Top(8)])),
        ('e', c.chain(l1(1), &[Bottom(0), Run(2, 2), SixEight])),
        ('f', c.chain(l1(-1), &[Bottom(0), Run(4, 6), Top(8)])),
        ('g', c.chain(l2(1), &[Run(2, 6), Top(8)])),
        ('c', c.chain(l2(-1), &[Bottom(1), Top(3)])),
        ('b', c.chain(l2(-1), &[Bottom(4), Top(7)])),
    ];
    let (fx, fy, fz) = match std::env::var("DECIMAL7_FACE") {
        Ok(v) => {
            let n: Vec<i32> = v.split(',').map(|s| s.trim().parse().unwrap_or(0)).collect();
            (n[0], n[1], n[2])
        }
        Err(_) => (1, 3, -16),
    };
    let sockets = face(&mut c.b, fx, fy, fz);
    for (seg, o) in &outs {
        let j = "abcdefg".find(*seg).unwrap();
        c.link(*o, sockets[j], D::N);
    }
    c.b.supports();
    route_and_write(c, name, out, expand, [-18, -3, fz - 1], [18, 12, 28], &sockets)
}

/// The picture check's designs: the display with its input chest replaced by a hopper holding each
/// value's level (0..=top), one design per value in `dir` (`<stem>-<v>.txt`), for `mctas web shot`.
pub fn faces(design: &std::path::Path, dir: &std::path::Path, top: u8) -> Result<()> {
    let text = std::fs::read_to_string(design)?;
    let base = mctas::design::Design::parse(&text)?;
    let input = *base.ports.iter().find(|(k, _)| k.starts_with("input")).map(|(_, p)| p).ok_or_else(|| anyhow!("no input port"))?;
    std::fs::create_dir_all(dir)?;
    let stem = design.file_stem().and_then(|s| s.to_str()).unwrap_or("display");
    for v in 0..=top {
        let mut d = base.clone();
        d.blocks.insert(input, "minecraft:hopper[facing=down,enabled=false]".into());
        d.block_entities.remove(&input);
        if v > 0 {
            // strength = floor(14 f) + 1 for f > 0, f = items / 320 (a hopper's five slots of 64)
            let n = (1..=320u32).find(|&n| (14 * n) / 320 + 1 >= v as u32).unwrap_or(320);
            let mut items = Vec::new();
            let (mut left, mut slot) = (n, 0);
            while left > 0 {
                let c = left.min(64);
                items.push(format!("{{Slot:{slot}b,id:\"minecraft:cobblestone\",count:{c}}}"));
                left -= c;
                slot += 1;
            }
            d.block_entities.insert(input, format!("{{Items:[{}]}}", items.join(",")));
        }
        d.name = format!("{}-{v}", base.name);
        std::fs::write(dir.join(format!("{stem}-{v}.txt")), d.to_text())?;
    }
    println!("{}: {} designs", dir.display(), top as u32 + 1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One straight lane into a stone in strict mode: its last repeater must point into the stone.
    #[test]
    fn strict_lane_straight() {
        let mut b = B::default();
        b.stone([0, 1, 0]);
        b.rep([0, 1, 10], D::N, 1);
        b.stone([0, 0, 10]);
        let lanes = vec![Lane { src: [0, 1, 10], src_dir: D::N, lamp: [0, 1, 0], into: D::N }];
        for strict in [false, true] {
            let mut w = crate::route::world_from(&b, &lanes, [-5, -2, -2], [5, 5, 14]);
            w.strict = strict;
            let r = w.route(0, &lanes[0], 100000);
            eprintln!("strict {strict}: {r:?}");
            assert!(r.is_some(), "strict {strict} does not route");
        }
    }

    /// A strict lane into the direct face's segment stone: does a design stone beside the goal (the
    /// face's protective stones at z + 3) refuse the last repeater?
    #[test]
    fn strict_lane_face() {
        for keep in [true, false] {
            let mut b = B::default();
            let targets = face_with(&mut b, 0, 3, 0, true);
            if keep {
                for p in [[0, 7, 3], [-2, 7, 3]] {
                    b.stone(p);
                }
            }
            b.rep([-1, 1, 12], D::N, 1);
            b.stone([-1, 0, 12]);
            b.supports();
            let lanes = vec![Lane { src: [-1, 1, 12], src_dir: D::N, lamp: targets[0], into: D::N }];
            let mut w = crate::route::world_from(&b, &lanes, [-8, -3, -1], [8, 10, 16]);
            w.strict = true;
            let r = w.route(0, &lanes[0], 200000);
            eprintln!("face stones at z+3 kept {keep}: {r:?}");
        }
    }
}
