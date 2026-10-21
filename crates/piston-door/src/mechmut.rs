//! `mctas door mechmut`: local search around known flat mechanisms (piece lists + box) for
//! variants with fewer pistons and better FEEDABILITY (wiring is the bottleneck of the smaller
//! doors, 2026-10-19). Neighbours of a mechanism: drop one piston, shift one by a cell, turn it,
//! toggle sticky / extended at rest. A neighbour is first REPLAYED with its parent's schedule
//! (events of a dropped piston removed, a changed piston's events moved to its new piece): when
//! that still opens and closes it is kept at once; otherwise it is solved exactly in families'
//! macro model (`families::solve`, `--bfs-cap`, 0 = replay only). Kept = opens (3x3 empty,
//! seamless ring), closes back to rest, FULL frame (every ring cell a door block or a stone when
//! open, or a cell no state of the cycle ever occupies, so a static stone fits). Ranked by
//! (pistons no free cell can feed, pistons, wiring class, events); a beam of the best seeds the
//! next round.

use crate::cosearch as cs;
use crate::families::{self as fam, Bx, K, Mech, Out, Pc, St};
use crate::mechsynth::Act;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// A schedule as (piece, action) pairs: portable across neighbours (piston ids are not).
pub type Sched = Vec<(Pc, Act)>;

/// One solved variant.
#[derive(Clone, Debug)]
pub struct Var {
    pub bx: Bx,
    pub pieces: Vec<Pc>,
    /// `--add` growth: 3x3 cells still filled in the best reachable state of a mechanism that
    /// does not solve yet (0 = solved; partial variants only steer the beam, never kept).
    pub missing: usize,
    /// Pistons no free cell can feed (families' wiring analysis of the solved cycle).
    pub unfed: usize,
    pub class: char,
    pub pistons: usize,
    pub open: String,
    pub close: String,
    pub events: usize,
    /// Ring cells holding a door block when open (the rest are stone cells).
    pub door_ring: usize,
    /// Found by replaying the parent's schedule (no search).
    pub replayed: bool,
    /// `door connect`'s verdict in the variant's own box ("WIRABLE" or "no: why").
    pub connect: String,
    /// Pistons no lever-woken free-cell region ever feeds (connect; w86's metric).
    pub never: usize,
    /// `--validate`: the schedule WORKS in redsim (None = not checked).
    pub real: Option<bool>,
    /// Piston actions fired from inside the 3x3 (carried pistons; only door blocks / QC feed them).
    pub inside: usize,
    /// Receivers (piston at a firing cell) whose largest feeding free region has < 3 cells.
    pub weak: usize,
    /// The most actions one piston takes on one edge.
    pub maxp: usize,
    /// Pocket load x10 (families' wiring analysis): the worst free region's forced actions per cell.
    pub load: usize,
    /// Island start (families' wiring analysis): firings without a started feed region, best
    /// lever region (0 = every region can start).
    pub start: usize,
    /// d67's `door feeds` on the redsim-working plan: (firings with no option, regions that
    /// cannot start, one-cell conflicts, firings with only flagged options).
    pub feeds: (usize, usize, usize, usize),
    /// d67's `door feeds --strict` verdict on the working plan: reasons it fails (0 = passes).
    pub strict: usize,
    /// The firing cells of the unfed receivers (families' wiring analysis).
    pub unfed_at: String,
    pub so: Sched,
    pub sc: Sched,
    /// Cells this variant must keep free (the global `--reserve` + its seed's own, e.g. a
    /// macrosat model's wiring cells with `--reserve-wiring`).
    pub reserve: Vec<(i8, i8)>,
    /// w86's `ideal-rs --extent` on the redsim-working plan: the door-plane bounding box of every
    /// non-air cell over the cycle, inputs and lever excluded, as (w, h); (0, 0) when unknown.
    pub extent: (i32, i32),
    /// Where the seed came from (a macrosat dump's file name, or "").
    pub origin: String,
    /// Solved without quasi-connectivity coupling (its seed's `qc_off`).
    pub qc_off: bool,
}

impl Var {
    pub fn key(&self) -> Vec<usize> {
        let cr = match self.class {
            'A' => 0,
            'B' => 1,
            'C' => 2,
            _ => 3,
        };
        let real_bad = usize::from(self.real == Some(false));
        let conn = usize::from(self.connect != "WIRABLE");
        if RANK_UNFED.load(std::sync::atomic::Ordering::Relaxed) {
            // --rank-unfed: the fewest pistons with no free feed cell first (pockets3's bar), then
            // d67's feed table (no option, one-cell conflicts, only flagged options), the pocket
            // load and the most actions of one piston on one edge
            let (fn_, fs, fc, fw) = self.feeds;
            // the extent leaving the box (the judge's volume grows) ranks right after redsim
            let out = usize::from(self.extent.0 > 0 && (self.extent.0 > self.bx.w as i32 || self.extent.1 > self.bx.h as i32));
            let vol = (self.extent.0 * self.extent.1) as usize;
            return vec![self.missing, real_bad, out, self.unfed, self.start, self.strict, fn_, fs, fc, conn, fw, self.load, self.maxp, vol, self.inside, self.never, self.weak, cr, self.events, self.pistons];
        }
        vec![self.missing, real_bad, self.inside, conn, self.never, self.weak, self.unfed, self.load, self.pistons, cr, self.events]
    }
    pub fn line(&self) -> String {
        format!("{} {} {} {} {}", self.bx.w, self.bx.h, self.bx.x0, self.bx.y0, self.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "))
    }
}

/// A piece change from the parent: (old piece, its replacement or None = dropped).
pub type Change = Option<(Pc, Option<Pc>)>;

/// `--validate`: keep a variant only when its macro schedule, as a `door ideal` plan (between-
/// ticks inputs; hallway test inputs allowed), WORKS in redsim at the 4 base positions.
pub static VALIDATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--outside-only`: reject variants with a piston firing from inside the 3x3.
pub static OUTSIDE_ONLY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--rank-unfed`: rank by unfed pistons right after the redsim verdict (instead of inside firings).
pub static RANK_UNFED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--max-unfed N`: reject variants with more unfed receivers (after the lever allowance).
pub static MAX_UNFED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(usize::MAX);
/// `--connect-cap N`: state cap of the `door connect` check per variant (default 100k; d67's
/// island argument shows u2-7x7 unwirable where 100k leaves it undecided).
pub static CONNECT_CAP: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(100_000);

/// The synthesizer writes helper stones as smooth stone; in a FULL-frame door the statics are
/// door material (pockets3: frame 9/12 under the record rule otherwise) and stones are
/// interchangeable (macrosat's model): write them as stone.
fn stone_statics(d: &mut mctas::design::Design) {
    for st in d.blocks.values_mut() {
        if st == "minecraft:smooth_stone" {
            *st = "minecraft:stone".into();
        }
    }
}

fn redsim_works(m: &Mech, rest: &St, sol: &fam::Sol) -> Option<crate::ideal::Schedule> {
    crate::mechsynth::CORRIDOR_INPUTS.store(true, std::sync::atomic::Ordering::Relaxed);
    let (wld, map) = fam::to_world(m);
    for (_, msol) in fam::ideal_solutions(m, rest, sol, &map) {
        let Ok(mut p) = crate::mechsynth::ideal_plan(&wld, &msol, false, 6, "validate") else { continue };
        stone_statics(&mut p.design);
        let s = crate::ideal::Schedule::from_events(p.design, &p.events);
        if crate::ideal_rs::run(&s, &std::collections::BTreeMap::new(), &crate::ideal::offsets(0), 1, false).is_ok() {
            return Some(s);
        }
    }
    None
}

fn to_sched(m: &Mech, seq: &[fam::E]) -> Sched {
    seq.iter().map(|e| (m.pieces[m.pid[e.id as usize]], e.act)).collect()
}

fn from_sched(m: &Mech, s: &Sched, ch: Change) -> Vec<fam::E> {
    s.iter()
        .filter_map(|&(p, act)| {
            let q = match ch {
                Some((old, new)) if old == p => new?,
                _ => p,
            };
            let id = m.pid.iter().position(|&pi| m.pieces[pi] == q)?;
            Some(fam::E { id: id as u8, act })
        })
        .collect()
}

/// Replay open then close from rest: Some(cycle states, open state) when the open state is open
/// (with the hole rule) and the close ends at rest.
fn replay(m: &Mech, rest: &St, open: &[fam::E], close: &[fam::E]) -> Option<(Vec<St>, St)> {
    let mut states = vec![rest.clone()];
    let mut c = rest.clone();
    for e in open {
        match m.apply(&c, *e) {
            Out::Ok(n) => c = n,
            Out::Noop => {}
            Out::Illegal => return None,
        }
        states.push(c.clone());
    }
    let o = c.clone();
    for e in close {
        match m.apply(&c, *e) {
            Out::Ok(n) => c = n,
            Out::Noop => {}
            Out::Illegal => return None,
        }
        states.push(c.clone());
    }
    let refs: Vec<&St> = states.iter().collect();
    (m.is_open(&o) && m.holes_ok(&refs, &o) && c == *rest).then_some((states, o))
}

/// `--lever-held`: piston ids the lever drives (their level only, toggled first on both edges).
pub static LEVER_HELD: std::sync::OnceLock<Vec<Pc>> = std::sync::OnceLock::new();

/// The lever-held phase rule: open = the lever flips (every held piston's level toggles at once)
/// then a search over the other pistons' events to an open state; close = the lever flips back
/// then a search back to rest. Shortest open first; up to 64 open states tried for a close.
/// The most actions one piston takes on one edge (pockets3's wirability measure).
fn max_per_piston(edges: &[&Vec<fam::E>]) -> usize {
    edges
        .iter()
        .map(|edge| {
            let mut n = [0usize; 32];
            for e in edge.iter() {
                n[e.id as usize] += 1;
            }
            n.into_iter().max().unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
}

fn solve_lever(m: &Mech, rest: &St, cap: usize, held: &[usize]) -> Option<(Vec<fam::E>, Vec<fam::E>)> {
    use std::collections::{HashMap, VecDeque};
    let flips: Vec<fam::E> = held.iter().map(|&id| fam::E { id: id as u8, act: Act::L }).collect();
    let flip = |s: &St| -> Option<St> {
        let mut c = s.clone();
        for e in &flips {
            match m.apply(&c, *e) {
                Out::Ok(n) => c = n,
                _ => return None,
            }
        }
        Some(c)
    };
    let evs: Vec<fam::E> = m.events(true).into_iter().filter(|e| !held.contains(&(e.id as usize))).collect();
    // `--max-actions K` (families' CONS_K, 0 = off): at most K actions per piston per edge, each
    // BFS node then being a (state, actions per piston: 4 bits each) pair (the lever's flips are
    // one action of the held pistons, which take no other event)
    let kk = fam::CONS_K.load(std::sync::atomic::Ordering::Relaxed);
    let k = kk.min(15) as u128;
    // BFS from `start`; returns the path to the first states meeting `goal` (up to `want`)
    let bfs = |start: &St, goal: &dyn Fn(&St) -> bool, want: usize| -> Vec<(St, Vec<fam::E>)> {
        let mut states: Vec<St> = vec![start.clone()];
        let mut counts: Vec<u128> = vec![0];
        let mut parent: Vec<(u32, Option<fam::E>)> = vec![(0, None)];
        let mut index: HashMap<(St, u128), u32> = HashMap::new();
        index.insert((start.clone(), 0), 0);
        let mut q = VecDeque::from([0u32]);
        let mut found = Vec::new();
        let path = |mut i: u32, parent: &[(u32, Option<fam::E>)]| -> Vec<fam::E> {
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
            if found.len() >= want {
                return found;
            }
        }
        while let Some(si) = q.pop_front() {
            let st = states[si as usize].clone();
            for &e in &evs {
                let mut c = counts[si as usize];
                if kk > 0 {
                    let sh = 4 * e.id as u32;
                    if ((c >> sh) & 15) + 1 > k {
                        continue;
                    }
                    c += 1u128 << sh;
                }
                let Out::Ok(n) = m.apply(&st, e) else { continue };
                let key = (n.clone(), c);
                if index.contains_key(&key) || states.len() >= cap {
                    continue;
                }
                let i = states.len() as u32;
                index.insert(key, i);
                states.push(n.clone());
                counts.push(c);
                parent.push((si, Some(e)));
                q.push_back(i);
                if goal(&n) {
                    found.push((n, path(i, &parent)));
                    if found.len() >= want {
                        return found;
                    }
                }
            }
        }
        found
    };
    let s1 = flip(rest)?;
    for (o, p1) in bfs(&s1, &|s: &St| m.is_open(s), 64) {
        let Some(o2) = flip(&o) else { continue };
        if let Some((_, p2)) = bfs(&o2, &|s: &St| s == rest, 1).into_iter().next() {
            let open: Vec<fam::E> = flips.iter().copied().chain(p1).collect();
            let close: Vec<fam::E> = flips.iter().copied().chain(p2).collect();
            return Some((open, close));
        }
    }
    None
}

/// Solve `pieces` in `bx`: the parent's schedule first (`hint`), then the exact solver (`cap`, 0 =
/// none); Some when it opens, closes and holds a FULL frame.
pub fn eval(bx: Bx, pieces: &[Pc], cap: usize, hint: Option<(&Sched, &Sched, Change)>, reserve: &[(i8, i8)]) -> Option<Var> {
    let (m, rest) = Mech::new(bx, pieces)?;
    let mut found: Option<(Vec<fam::E>, Vec<fam::E>, bool)> = None;
    let kk = fam::CONS_K.load(std::sync::atomic::Ordering::Relaxed);
    if let Some((so, sc, ch)) = hint {
        let (o, c) = (from_sched(&m, so, ch), from_sched(&m, sc, ch));
        // a replayed schedule over the `--max-actions` bar falls through to the (constrained) solver
        if replay(&m, &rest, &o, &c).is_some() && (kk == 0 || max_per_piston(&[&o, &c]) <= kk) {
            found = Some((o, c, true));
        }
    }
    if found.is_none() && cap > 0 {
        match LEVER_HELD.get().filter(|h| !h.is_empty()) {
            Some(held) => {
                // the lever drives these pistons: their levels flip first on both edges
                let ids: Vec<usize> = held.iter().filter_map(|h| m.pid.iter().position(|&pi| m.pieces[pi] == *h)).collect();
                if ids.len() == held.len()
                    && let Some((o, c)) = solve_lever(&m, &rest, cap, &ids)
                {
                    found = Some((o, c, false));
                }
            }
            None => {
                if let Ok(Some(sol)) = fam::solve(&m, &rest, cap, true) {
                    found = Some((sol.open, sol.close, false));
                }
            }
        }
    }
    let (open_e, close_e, replayed) = found?;
    let (states, open) = replay(&m, &rest, &open_e, &close_e)?;
    let at = |s: &St, x: i8, y: i8| if bx.has(x, y) { s.g[bx.idx(x, y)] } else { fam::AIR };
    if reserve.iter().any(|&(x, y)| states.iter().any(|s| at(s, x, y) != fam::AIR)) {
        return None; // the cycle sweeps a reserved cell
    }
    let mut door_ring = 0;
    let mut fill: Vec<(i8, i8)> = Vec::new();
    for (x, y) in fam::ring() {
        let c = at(&open, x, y);
        if c == fam::DOOR {
            door_ring += 1;
        } else if c == fam::STONE {
            // a static (or moved) plain block holds the frame cell
        } else if states.iter().any(|s| at(s, x, y) != fam::AIR) {
            return None; // neither a door block, a stone nor a static-stone cell when open
        } else {
            fill.push((x, y));
        }
    }
    if !fill.is_empty() {
        // FULL frame: a static stone in every frame cell the cycle never touches and leaves air
        // when open (it changes nothing in the macro cycle); judged again with them
        let mut with: Vec<Pc> = pieces.to_vec();
        with.extend(fill.iter().map(|&(x, y)| Pc { x, y, k: K::Stone }));
        with.sort();
        let (so, sc) = (to_sched(&m, &open_e), to_sched(&m, &close_e));
        return eval(bx, &with, 0, Some((&so, &sc, None)), reserve);
    }
    // piston actions fired from inside the 3x3 (a carried piston): only door blocks or
    // quasi-connectivity could feed them
    let mut inside = 0;
    {
        let mut c = rest.clone();
        for e in open_e.iter().chain(close_e.iter()) {
            let (x, y) = bx.xy(c.pp[e.id as usize] as usize);
            if fam::in_t(x, y) {
                inside += 1;
            }
            if let Out::Ok(n) = m.apply(&c, *e) {
                c = n;
            }
        }
    }
    if OUTSIDE_ONLY.load(std::sync::atomic::Ordering::Relaxed) && inside > 0 {
        return None;
    }
    // the most actions one piston takes on one edge (pockets3: each distinct pulse needs its own
    // watchable event next to the piston's feed pocket; 1-2 per edge is wirable)
    let maxp = max_per_piston(&[&open_e, &close_e]);
    if kk > 0 && maxp > kk {
        return None;
    }
    let sol = fam::Sol { open: open_e.clone(), close: close_e.clone(), sym: None, sym_levels: false, states: 0, extent: (0, 0, 0, 0), levels: false };
    // (--validate: does the macro schedule replay in the simulator? kept either way, ranked first)
    let validated = VALIDATE.load(std::sync::atomic::Ordering::Relaxed).then(|| redsim_works(&m, &rest, &sol));
    let real = validated.as_ref().map(|s| s.is_some());
    // d67's feed table on the working plan (none / conflicts / weak firings; usize::MAX / 2 when
    // not validated or not analysable)
    let bad4 = (usize::MAX / 2, usize::MAX / 2, usize::MAX / 2, usize::MAX / 2);
    let mut strict = usize::MAX / 2;
    let feeds = match validated.as_ref().and_then(|s| s.as_ref()) {
        Some(s) => match crate::feeds::analyze_schedule_full(s, "mechmut", Some((bx.x0 as i32, bx.y0 as i32, (bx.x0 + bx.w - 1) as i32, (bx.y0 + bx.h - 1) as i32)), false, true) {
            Ok(r) => {
                strict = r.strict_fail.len();
                (r.none.len(), r.cannot_start.len(), r.conflicts.len(), r.weak.len())
            }
            Err(_) => bad4,
        },
        None => bad4,
    };
    // w86's --extent on the working plan (the volume the judge will see)
    let extent = match validated.as_ref().and_then(|s| s.as_ref()) {
        Some(s) => match crate::ideal_rs::extent(s, s.vars()) {
            Ok((lo, hi)) => (hi[0] - lo[0] + 1, hi[1] - lo[1] + 1),
            Err(_) => (0, 0),
        },
        None => (0, 0),
    };
    let w = fam::wiring(&m, &rest, &sol);
    let (unfed_list, lever_held) = fam::unfed_cells(&m, &rest, &sol);
    let unfed = unfed_list.len() - lever_held;
    if unfed > MAX_UNFED.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let unfed_at = unfed_list.iter().map(|(x, y)| format!("({x},{y})")).collect::<Vec<_>>().join(" ") + &if lever_held > 0 { format!(" [{lever_held} held by a lever outside the box]") } else { String::new() };
    // `door connect` (connect.rs): free-cell regions after the run's footprint, each piston fed
    // from a region next to it (or its quasi cell, a plain ring stone, a door block), a region
    // wakes only when it can watch a change, every event order from the lever's region: the
    // reachability w86 asked for (a pocket only late pistons touch cannot start)
    let (wld, map) = fam::to_world(&m);
    crate::mechsynth::SEAMLESS.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::mechsynth::SEAMLESS_OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
    let msol = crate::mechsynth::Solution { open: fam::ms_seq(&map, &open_e), close: fam::ms_seq(&map, &close_e), sym: None, states: 0 };
    let conn = crate::connect::check(&wld, &msol, [bx.x0, bx.y0], [bx.x0 + bx.w - 1, bx.y0 + bx.h - 1], CONNECT_CAP.load(std::sync::atomic::Ordering::Relaxed));
    let connect = if conn.ok { "WIRABLE".to_string() } else { format!("no: {}", conn.report.lines().last().unwrap_or("").trim()) };
    // pistons no region the lever can wake ever feeds (connect's best failure; 0 when WIRABLE)
    let never = if conn.ok {
        0
    } else {
        let last = conn.report.lines().last().unwrap_or("");
        match last.split("never fed").nth(1).and_then(|r| r.split(": ").nth(1)) {
            Some(list) => {
                let list = list.trim_end_matches(')').trim();
                if list == "none" { 0 } else { list.split_whitespace().count() }
            }
            None => usize::MAX / 4,
        }
    };
    Some(Var {
        bx,
        pieces: m.pieces.clone(),
        missing: 0,
        unfed,
        class: w.class,
        pistons: pieces.iter().filter(|p| matches!(p.k, K::Piston { .. })).count(),
        open: m.fmt_seq(&open_e),
        close: m.fmt_seq(&close_e),
        events: open_e.len() + close_e.len(),
        door_ring,
        replayed,
        connect,
        never,
        real,
        inside,
        weak: w.weak,
        load: w.load,
        start: w.start,
        feeds,
        strict,
        maxp,
        unfed_at,
        so: to_sched(&m, &open_e),
        sc: to_sched(&m, &close_e),
        reserve: reserve.to_vec(),
        extent,
        origin: String::new(),
        qc_off: !m.qc,
    })
}

/// Every one-step neighbour of `pieces` in `bx`, with the change that made it. Pieces in `keep`
/// are never changed; no piece may cover a `reserve` cell.
pub fn neighbours(bx: Bx, pieces: &[Pc], keep: &[Pc], reserve: &[(i8, i8)]) -> Vec<(Vec<Pc>, Change)> {
    let mut out = Vec::new();
    let fits = |v: &[Pc]| -> bool {
        let mut seen = HashSet::new();
        v.iter().all(|p| p.cells().iter().all(|&(x, y)| bx.has(x, y) && !fam::in_t(x, y) && !reserve.contains(&(x, y)) && seen.insert((x, y))))
    };
    for i in 0..pieces.len() {
        let K::Piston { sticky, f, ext } = pieces[i].k else { continue };
        let p = pieces[i];
        if keep.contains(&p) {
            continue;
        }
        let rest: Vec<Pc> = pieces.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, q)| *q).collect();
        // drop
        out.push((rest.clone(), Some((p, None))));
        let mut with = |q: Pc| {
            let mut v = rest.clone();
            v.push(q);
            v.sort();
            if fits(&v) {
                out.push((v, Some((p, Some(q)))));
            }
        };
        // shift by one cell
        for d in fam::F {
            with(Pc { x: p.x + d.0, y: p.y + d.1, k: p.k });
        }
        // turn
        for f2 in 0..4u8 {
            if f2 != f {
                with(Pc { x: p.x, y: p.y, k: K::Piston { sticky, f: f2, ext } });
            }
        }
        // sticky <-> normal, retracted <-> extended at rest
        with(Pc { x: p.x, y: p.y, k: K::Piston { sticky: !sticky, f, ext } });
        with(Pc { x: p.x, y: p.y, k: K::Piston { sticky, f, ext: !ext } });
    }
    if ADD.load(std::sync::atomic::Ordering::Relaxed) {
        // add one piece: a piston (sticky or normal, any facing, retracted or extended at rest)
        // in any free cell outside the 3x3, or a stone in a free ring cell (completion search
        // from partial designs: w86's fixed lever trio + side pullers)
        let used: HashSet<(i8, i8)> = pieces.iter().flat_map(|p| p.cells()).collect();
        for y in bx.y0..bx.y0 + bx.h {
            for x in bx.x0..bx.x0 + bx.w {
                if fam::in_t(x, y) || used.contains(&(x, y)) || reserve.contains(&(x, y)) {
                    continue;
                }
                let mut adds: Vec<Pc> = Vec::new();
                for sticky in [true, false] {
                    for f in 0..4u8 {
                        for ext in [false, true] {
                            adds.push(Pc { x, y, k: K::Piston { sticky, f, ext } });
                        }
                    }
                }
                if fam::in_r(x, y) {
                    adds.push(Pc { x, y, k: K::Stone });
                }
                // a redstone block (moved by pistons, it powers the pistons it touches: w86's
                // shuttle feeding the middle frame piston from below)
                if REDSTONE.load(std::sync::atomic::Ordering::Relaxed) {
                    adds.push(Pc { x, y, k: K::Redstone });
                }
                for q in adds {
                    let mut v = pieces.to_vec();
                    v.push(q);
                    v.sort();
                    if fits(&v) {
                        out.push((v, Some((q, Some(q)))));
                    }
                }
            }
        }
    }
    out
}

/// `--add`: neighbours also add one piece (see [`neighbours`]).
pub static ADD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--add-redstone`: `--add` may also add a redstone block.
pub static REDSTONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Lines `W H x0 y0 piece piece ...` (`#` comments).
pub fn parse_lines(text: &str) -> Result<Vec<(Bx, Vec<Pc>)>> {
    let mut v = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let t: Vec<&str> = line.split_whitespace().collect();
        anyhow::ensure!(t.len() > 4, "line {line:?}: expected W H x0 y0 pieces...");
        let bx = Bx { w: t[0].parse()?, h: t[1].parse()?, x0: t[2].parse()?, y0: t[3].parse()? };
        let mut pieces: Vec<Pc> = t[4..].iter().map(|s| crate::mechspace::parse_piece(s)).collect::<Result<_>>()?;
        pieces.sort();
        v.push((bx, pieces));
    }
    Ok(v)
}

pub struct Opts {
    /// State cap of the seeds' exact solve.
    pub cap: usize,
    /// State cap of a neighbour's exact solve when the replay fails (0 = replay only).
    pub bfs_cap: usize,
    pub threads: usize,
    pub rounds: usize,
    pub beam: usize,
    pub top: usize,
    /// Search from seeds that do not solve too (their neighbours may).
    pub repair: bool,
    /// Pieces never changed (e.g. a lever-held top trio).
    pub keep: Vec<Pc>,
    /// Cells no piece may cover and no state of the cycle may occupy (room for an observer).
    pub reserve: Vec<(i8, i8)>,
    /// A macrosat seed keeps its model's wiring cells free too.
    pub reserve_wiring: bool,
    /// macrosat seeds keep families' quasi-connectivity coupling (default: off for them).
    pub macrosat_qc: bool,
}

/// A placeholder variant for a mechanism that does not solve (beam steering only; fewer pieces
/// first among equal growth).
fn partial(bx: Bx, p: &[Pc], missing: usize) -> Var {
    let pistons = p.iter().filter(|q| matches!(q.k, K::Piston { .. })).count();
    Var { bx, pieces: p.to_vec(), missing, unfed: usize::MAX / 2, class: 'X', pistons, open: String::new(), close: String::new(), events: p.len(), door_ring: 0, replayed: false, connect: "-".into(), never: usize::MAX / 2, real: None, inside: usize::MAX / 2, weak: usize::MAX / 2, maxp: usize::MAX / 2, load: usize::MAX / 2, start: usize::MAX / 2, feeds: (usize::MAX / 2, usize::MAX / 2, usize::MAX / 2, usize::MAX / 2), strict: usize::MAX / 2, unfed_at: String::new(), so: Vec::new(), sc: Vec::new(), reserve: Vec::new(), extent: (0, 0), origin: String::new(), qc_off: fam::NO_QC_HERE.with(|c| c.get()) }
}

/// `--add` growth measure: 3x3 cells still filled in the best state reachable from rest (plain
/// BFS over every pulse / level, the `--no-inside` bar applied, `cap` states) + 1 (1 = opens fully
/// but does not solve; 10 = nothing moves).
fn progress(bx: Bx, p: &[Pc], cap: usize) -> usize {
    let Some((m, rest)) = Mech::new(bx, p) else { return 10 };
    let no_in = fam::CONS_NO_INSIDE.load(std::sync::atomic::Ordering::Relaxed);
    let evs = m.events(true);
    let filled = |s: &St| (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).filter(|&(x, y)| s.g[bx.idx(x, y)] != fam::AIR).count();
    let mut seen: HashSet<St> = HashSet::new();
    let mut q = std::collections::VecDeque::from([rest.clone()]);
    seen.insert(rest.clone());
    let mut best = filled(&rest);
    while let Some(s) = q.pop_front() {
        for &e in &evs {
            if no_in {
                let (x, y) = bx.xy(s.pp[e.id as usize] as usize);
                if fam::in_t(x, y) {
                    continue;
                }
            }
            let Out::Ok(n) = m.apply(&s, e) else { continue };
            if seen.len() >= cap || !seen.insert(n.clone()) {
                continue;
            }
            best = best.min(filled(&n));
            q.push_back(n);
        }
    }
    // + 1: a partial variant ranks after every solved one even when it opens fully (no close)
    best + 1
}

/// A search seed: a mechanism, optionally its own schedule (macrosat's actions, replayed first),
/// the cells it must keep free, and where it came from.
pub struct Seed {
    pub bx: Bx,
    pub pieces: Vec<Pc>,
    pub hint: Option<(Sched, Sched)>,
    pub reserve: Vec<(i8, i8)>,
    pub origin: String,
    /// Solve (and replay the hint) without quasi-connectivity coupling (macrosat seeds: its
    /// schedules list co-fired pistons as their own actions); redsim stays the judge.
    pub qc_off: bool,
}

/// A `door macrosat` dump (clockinfra's model printout: the `macrosat: box WxH at (x0,y0)` line,
/// the `rest:` and `open:` grids, `open|close step N: KIND (x, y) facing ...` action lines, and,
/// from 2026-10-20 11:18Z, `%` wiring cells and a `wiring:` line). Returns the box, the pieces
/// (pistons, stones outside the 3x3, obsidian, redstone blocks; door blocks implied), the wiring
/// cells, and the schedule as (cell it fires from, actions) per edge. In older dumps a wiring cell
/// and a retracted normal west piston are both `w`: a `w` in both grids at the same cell that no
/// action names is wiring.
/// The box of a `door macrosat` dump (its `macrosat: box WxH at (x0,y0)` line; for the
/// header-less `.report` files (2026-10-21) inferred from the `rest:` grid and the `wiring:`
/// line: the column offset that puts every listed wiring cell on a `%`); None when the text is not
/// a dump.
fn macrosat_box(text: &str) -> Result<Option<Bx>> {
    let Some(head) = text.lines().find(|l| l.starts_with("macrosat: box ")) else { return macrosat_box_inferred(text) };
    // "macrosat: box 6x8 at (-1,-2), h 6 ..."
    let rest = head.trim_start_matches("macrosat: box ");
    let (wh, after) = rest.split_once(" at (").context("macrosat header: box")?;
    let (w, h) = wh.split_once('x').context("macrosat header: WxH")?;
    let (xy, _) = after.split_once(')').context("macrosat header: (x0,y0)")?;
    let (x0, y0) = xy.split_once(',').context("macrosat header: x0,y0")?;
    Ok(Some(Bx { w: w.trim().parse()?, h: h.trim().parse()?, x0: x0.trim().parse()?, y0: y0.trim().parse()? }))
}

/// [`macrosat_box`] for a header-less dump: h and y from the `rest:` grid rows, w from their token
/// count, x0 from the `wiring:` line (every wiring cell must be a `%` in the rest grid; the offset
/// must be unique).
fn macrosat_box_inferred(text: &str) -> Result<Option<Bx>> {
    if !text.lines().any(|l| l.trim() == "rest:") {
        return Ok(None);
    }
    let Some(wl) = text.lines().find(|l| l.starts_with("wiring:")) else { return Ok(None) };
    let wiring: Vec<(i8, i8)> = wl
        .trim_start_matches("wiring:")
        .split(')')
        .filter_map(|s| {
            let (a, b) = s.trim().trim_start_matches('(').split_once(',')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
        })
        .collect();
    let mut rows: Vec<(i8, Vec<String>)> = Vec::new();
    for l in text.lines().skip_while(|l| l.trim() != "rest:").skip(1) {
        let mut t = l.split_whitespace();
        let Some(Ok(y)) = t.next().map(|v| v.parse::<i8>()) else { break };
        let toks: Vec<String> = t.map(str::to_string).collect();
        if toks.is_empty() {
            break;
        }
        rows.push((y, toks));
    }
    anyhow::ensure!(!rows.is_empty() && !wiring.is_empty(), "header-less macrosat dump: no rest grid or no wiring cells");
    let w = rows[0].1.len() as i8;
    anyhow::ensure!(rows.iter().all(|(_, t)| t.len() as i8 == w), "header-less macrosat dump: ragged rest grid");
    let y0 = rows.iter().map(|(y, _)| *y).min().unwrap();
    let h = rows.len() as i8;
    let minx = wiring.iter().map(|c| c.0).min().unwrap();
    let fits = |x0: i8| {
        wiring.iter().all(|&(x, y)| {
            let col = x - x0;
            col >= 0 && col < w && rows.iter().find(|(ry, _)| *ry == y).is_some_and(|(_, t)| t[col as usize] == "%")
        })
    };
    let cands: Vec<i8> = (minx - w + 1..=minx).filter(|&x0| fits(x0)).collect();
    anyhow::ensure!(cands.len() == 1, "header-less macrosat dump: the wiring cells fit {} column offsets ({cands:?})", cands.len());
    Ok(Some(Bx { w, h, x0: cands[0], y0 }))
}

pub fn parse_macrosat(text: &str) -> Result<Option<(Bx, Vec<Pc>, Vec<(i8, i8)>, [Vec<((i8, i8), Vec<Act>)>; 2])>> {
    let Some(bx) = macrosat_box(text)? else { return Ok(None) };
    // a grid after a "<name>:" line: rows "  y tok tok ..." (y descending)
    let grid = |name: &str| -> Option<HashMap<(i8, i8), String>> {
        let mut lines = text.lines().skip_while(|l| l.trim() != format!("{name}:")).skip(1);
        let mut g = HashMap::new();
        for _ in 0..bx.h {
            let l = lines.next()?;
            let mut t = l.split_whitespace();
            let y: i8 = t.next()?.parse().ok()?;
            for (i, tok) in t.enumerate() {
                g.insert((bx.x0 + i as i8, y), tok.to_string());
            }
        }
        Some(g)
    };
    let r = grid("rest").context("macrosat dump: no rest grid")?;
    let o = grid("open");
    // actions: "open  step  0: EXTEND (4, 2) west line 0  [fed from ...]"
    let mut sched: [Vec<((i8, i8), Vec<Act>)>; 2] = [Vec::new(), Vec::new()];
    let mut named: HashSet<(i8, i8)> = HashSet::new();
    for l in text.lines() {
        let edge = if l.starts_with("open ") && l.contains(" step ") {
            0
        } else if l.starts_with("close ") && l.contains(" step ") {
            1
        } else {
            continue;
        };
        let Some((_, body)) = l.split_once(": ") else { continue };
        let mut t = body.split_whitespace();
        let kind = t.next().unwrap_or("");
        let Some((cx, cy)) = body.split_once('(').and_then(|(_, r)| r.split_once(')')).and_then(|(c, _)| c.split_once(',')) else { continue };
        let (Ok(x), Ok(y)) = (cx.trim().parse::<i8>(), cy.trim().parse::<i8>()) else { continue };
        let acts = match kind {
            "EXTEND" | "RETRACT" => vec![Act::L],
            "PULSE" => vec![Act::P],
            "PULSE-PULL" => vec![Act::L, Act::L],
            _ => continue,
        };
        // older dumps list actions that change nothing (macrosat leaves them out since
        // 2026-10-20): a pulse into air, a pulse-pull of one block or one that pulls nothing
        let line_len: Option<usize> = body.split(" line ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|v| v.parse().ok());
        let pulls = body.contains("(pulls)");
        let junk = match (kind, line_len) {
            ("PULSE", Some(0)) => true,
            ("PULSE-PULL", Some(1)) => true,
            ("PULSE-PULL", Some(0)) => !pulls,
            _ => false,
        };
        if junk {
            continue;
        }
        named.insert((x, y));
        sched[edge].push(((x, y), acts));
    }
    let wiring_line: Vec<(i8, i8)> = text
        .lines()
        .find(|l| l.starts_with("wiring:"))
        .map(|l| {
            l.trim_start_matches("wiring:")
                .split(')')
                .filter_map(|s| {
                    let (a, b) = s.trim().trim_start_matches('(').split_once(',')?;
                    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
                })
                .collect()
        })
        .unwrap_or_default();
    let mut pieces = Vec::new();
    let mut wiring = wiring_line.clone();
    for (&(x, y), tok) in &r {
        let tok = tok.as_str();
        let ext = tok.ends_with('*');
        let c = tok.trim_end_matches('*');
        let piston = |ch: char| -> Option<(bool, u8)> {
            let f = match ch.to_ascii_lowercase() {
                'e' => 0,
                'w' => 1,
                'u' => 2,
                'd' => 3,
                _ => return None,
            };
            Some((ch.is_ascii_uppercase(), f))
        };
        match c {
            "." | ">" | "<" | "'" | "," | "}" | "{" | "^" | "v" => {}
            "%" => wiring.push((x, y)),
            "#" => {
                if !fam::in_t(x, y) {
                    pieces.push(Pc { x, y, k: K::Stone });
                }
            }
            "O" => pieces.push(Pc { x, y, k: K::Obsidian }),
            "R" => pieces.push(Pc { x, y, k: K::Redstone }),
            "w" if !ext
                && !named.contains(&(x, y))
                && (wiring_line.contains(&(x, y)) || o.as_ref().is_some_and(|g| g.get(&(x, y)).is_some_and(|t| t == "w"))) =>
            {
                if !wiring.contains(&(x, y)) {
                    wiring.push((x, y));
                }
            }
            _ => {
                let mut ch = c.chars();
                let (Some(ch0), None) = (ch.next(), ch.next()) else { anyhow::bail!("macrosat dump: unknown glyph {tok:?} at ({x},{y})") };
                let (sticky, f) = piston(ch0).with_context(|| format!("macrosat dump: unknown glyph {tok:?} at ({x},{y})"))?;
                pieces.push(Pc { x, y, k: K::Piston { sticky, f, ext } });
            }
        }
    }
    pieces.sort();
    wiring.sort();
    wiring.dedup();
    let pieces = prune_idle(&pieces, &named, o.as_ref());
    Ok(Some((bx, pieces, wiring, sched)))
}

/// macrosat's schedule as families events: per action, the piston standing at the named cell
/// in the replayed state. Err (why) when the replay fails (a cell holds no piston, an action is
/// refused): the seed is then solved from scratch.
fn macrosat_sched(bx: Bx, pieces: &[Pc], sched: &[Vec<((i8, i8), Vec<Act>)>; 2]) -> std::result::Result<(Sched, Sched), String> {
    let (m, rest) = Mech::new(bx, pieces).ok_or_else(|| format!("families refuses the rest state: {}", refusal(bx, pieces)))?;
    let mut c = rest.clone();
    let mut out: [Vec<fam::E>; 2] = [Vec::new(), Vec::new()];
    for (edge, acts) in sched.iter().enumerate() {
        for (k, ((x, y), list)) in acts.iter().enumerate() {
            let id = (0..m.info.len()).find(|&id| bx.xy(c.pp[id] as usize) == (*x, *y)).ok_or_else(|| format!("{} action {k}: no piston at ({x},{y})", ["open", "close"][edge]))?;
            for &act in list {
                let e = fam::E { id: id as u8, act };
                match m.apply(&c, e) {
                    Out::Ok(n) => c = n,
                    Out::Noop => return Err(format!("{} action {k}: {:?} of {} at ({x},{y}) changes nothing in families' model", ["open", "close"][edge], act, m.pieces[m.pid[id]].code())),
                    Out::Illegal => return Err(format!("{} action {k}: {:?} of {} at ({x},{y}) is illegal in families' model", ["open", "close"][edge], act, m.pieces[m.pid[id]].code())),
                }
                out[edge].push(e);
            }
        }
        if edge == 0 && !m.is_open(&c) {
            return Err("not open after macrosat's open actions in families' model".into());
        }
    }
    if c != rest {
        return Err("not back at rest after macrosat's close actions in families' model".into());
    }
    Ok((to_sched(&m, &out[0]), to_sched(&m, &out[1])))
}

/// Why `Mech::new` refuses a piece list (for the log).
fn refusal(bx: Bx, pieces: &[Pc]) -> String {
    let np = pieces.iter().filter(|p| matches!(p.k, K::Piston { .. })).count();
    if np > 32 {
        return format!("{np} pistons (families' model holds at most 32)");
    }
    let mut seen: HashSet<(i8, i8)> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).collect();
    for p in pieces {
        for c in p.cells() {
            if !bx.has(c.0, c.1) {
                return format!("{} leaves the box", p.code());
            }
            if !seen.insert(c) {
                return format!("{} overlaps another piece or the door", p.code());
            }
        }
    }
    if pieces.iter().any(|p| matches!(p.k, K::Redstone)) {
        return "a redstone block's rest state is not settled (a piston beside it would move)".into();
    }
    "a piston held at rest QC-arms a retracted piston right below it".into()
}

/// macrosat models are not minimal (nothing in the SAT model penalizes extra pieces): drop the
/// pistons no action names that stand at the same cell in the rest and open grids (they never
/// fire and were not moved by the open edge).
fn prune_idle(pieces: &[Pc], named: &HashSet<(i8, i8)>, open: Option<&HashMap<(i8, i8), String>>) -> Vec<Pc> {
    pieces
        .iter()
        .filter(|p| {
            let K::Piston { sticky, f, ext } = p.k else { return true };
            if named.contains(&(p.x, p.y)) {
                return true;
            }
            let glyph = {
                let c = ['e', 'w', 'u', 'd'][f as usize];
                let c = if sticky { c.to_ascii_uppercase() } else { c };
                if ext { format!("{c}*") } else { c.to_string() }
            };
            !open.is_some_and(|g| g.get(&(p.x, p.y)).is_some_and(|t| *t == glyph))
        })
        .copied()
        .collect()
}

/// A macrosat dump's own `--export` pair (`export/c-<dump>.txt` + `.ideal` beside it): its
/// feed-faithful plan (QC feeds through the cell above + a glass update, conductor feeds via a
/// lever) judged directly: redsim at 32 positions x 2 cycles, d67's strict feed screen (no feed,
/// weak only, one-cell conflict, a region that cannot start, a moving via conductor), w86's
/// extent. Returns (summary, passes: WORKS + strict clean + extent inside the box).
fn macrosat_export_check(dump: &Path, bx: Bx) -> Option<(String, bool)> {
    let stem = dump.file_stem()?.to_string_lossy().to_string();
    let parent = dump.parent()?;
    // the export pair: `export/c-<dump>.*` (sweep dumps), else `<dump>-L.*` (a hand-patched
    // lever representation), else `<dump>.*` beside a `.report`
    let pairs = [
        (parent.join("export").join(format!("c-{stem}.txt")), parent.join("export").join(format!("c-{stem}.ideal"))),
        (parent.join(format!("{stem}-L.txt")), parent.join(format!("{stem}-L.ideal"))),
        (parent.join(format!("{stem}.txt")), parent.join(format!("{stem}.ideal"))),
    ];
    let (dtxt, dideal) = pairs.into_iter().find(|(t, i)| t.exists() && i.exists() && t != dump)?;
    // the design line names the exporting box's path: point it at the file beside the plan
    let text: String = std::fs::read_to_string(&dideal)
        .ok()?
        .lines()
        .map(|l| if l.starts_with("design ") { format!("design {}", dtxt.display()) } else { l.to_string() })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let s = match crate::ideal::Schedule::parse_with(&text, &std::collections::BTreeMap::new()) {
        Ok(s) => s,
        Err(e) => return Some((format!("export does not parse: {e:#}"), false)),
    };
    let redsim = crate::ideal_rs::run(&s, s.vars(), &crate::ideal::offsets(28), 2, false);
    let bxs = Some((bx.x0 as i32, bx.y0 as i32, (bx.x0 + bx.w - 1) as i32, (bx.y0 + bx.h - 1) as i32));
    let strict = crate::feeds::analyze_full(&text, &stem, bxs, false, true);
    let ext = crate::ideal_rs::extent(&s, s.vars());
    let (ew, eh) = match &ext {
        Ok((lo, hi)) => (hi[0] - lo[0] + 1, hi[1] - lo[1] + 1),
        Err(_) => (0, 0),
    };
    let works = redsim.is_ok();
    let clean = strict.as_ref().is_ok_and(|r| r.strict_fail.is_empty());
    let inside = ew > 0 && ew <= bx.w as i32 && eh <= bx.h as i32;
    let summary = format!(
        "export {}: redsim {}; strict {}; extent {ew}x{eh}{}",
        dideal.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
        match &redsim {
            Ok((o, c)) => format!("WORKS (open {o} / close {c})"),
            Err(e) => format!("BROKEN ({})", e.chars().take(100).collect::<String>()),
        },
        match &strict {
            Ok(r) if r.strict_fail.is_empty() => "clean".to_string(),
            Ok(r) => format!("FAILS: {}", r.strict_fail.join("; ").chars().take(400).collect::<String>()),
            Err(e) => format!("not analysable ({e:#})"),
        },
        if inside { "" } else { " (leaves the box)" }
    );
    Some((summary, works && clean && inside))
}

/// The seeds in `input`: a file of `W H x0 y0 pieces` lines, a macrosat dump, or a folder (every
/// macrosat dump under it, recursively). `reserve_wiring`: a macrosat seed keeps its wiring cells
/// free; `macrosat_qc_off`: macrosat seeds are solved without quasi-connectivity coupling.
pub fn read_seeds(input: &Path, reserve: &[(i8, i8)], reserve_wiring: bool, macrosat_qc_off: bool) -> Result<Vec<Seed>> {
    let mut files = Vec::new();
    if input.is_dir() {
        let mut stack = vec![input.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d)? {
                let p = e?.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "txt" || x == "report") {
                    files.push(p);
                }
            }
        }
        files.sort();
    } else {
        files.push(input.to_path_buf());
    }
    let mut seeds = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).with_context(|| format!("{}", f.display()))?;
        let label = f.strip_prefix(input).unwrap_or(f).display().to_string();
        let parsed = match parse_macrosat(&text) {
            Ok(p) => p,
            Err(e) => {
                // a dump families cannot hold (e.g. macrosat --observers' movable observers): its
                // own export pair is still judged; it is no repair seed (one dump never aborts a
                // folder)
                eprintln!("[mechmut] macrosat {label}: not a families seed ({e:#}); judging its export only");
                if let Ok(Some(bx)) = macrosat_box(&text)
                    && let Some((summary, pass)) = macrosat_export_check(f, bx)
                {
                    eprintln!("[mechmut] macrosat {label}: {summary}{}", if pass { "  ** CANDIDATE: send to w86 **" } else { "" });
                }
                continue;
            }
        };
        if let Some((bx, pieces, wiring, sched)) = parsed {
            let hint = fam::with_qc_off(macrosat_qc_off, || macrosat_sched(bx, &pieces, &sched));
            let mut res = reserve.to_vec();
            if reserve_wiring {
                res.extend(wiring.iter().copied());
            }
            eprintln!(
                "[mechmut] macrosat {label}: {}x{} at ({},{}), {} pieces ({} pistons), wiring {:?}{}",
                bx.w,
                bx.h,
                bx.x0,
                bx.y0,
                pieces.len(),
                pieces.iter().filter(|p| matches!(p.k, K::Piston { .. })).count(),
                wiring,
                match &hint {
                    Ok(_) => String::new(),
                    Err(e) => format!("; its schedule does not replay: {e}"),
                }
            );
            seeds.push(Seed { bx, pieces, hint: hint.ok(), reserve: res, origin: label.clone(), qc_off: macrosat_qc_off });
            if let Some((summary, pass)) = macrosat_export_check(f, bx) {
                eprintln!("[mechmut] macrosat {label}: {summary}{}", if pass { "  ** CANDIDATE: send to w86 **" } else { "" });
            }
        } else if !input.is_dir() {
            for (bx, pieces) in parse_lines(&text)? {
                seeds.push(Seed { bx, pieces, hint: None, reserve: reserve.to_vec(), origin: String::new(), qc_off: false });
            }
        }
    }
    Ok(seeds)
}

/// Beam search from every input mechanism; returns every kept variant, best first.
pub fn search(seeds: &[Seed], o: &Opts) -> Vec<Var> {
    let mut seen: HashSet<Vec<Pc>> = HashSet::new();
    let mut kept: Vec<Var> = Vec::new();
    let mut beam: Vec<Var> = Vec::new();
    let seed_res = cs::par_map(seeds, o.threads, |s| {
        let hint = s.hint.as_ref().map(|(so, sc)| (so, sc, None));
        fam::with_qc_off(s.qc_off, || eval(s.bx, &s.pieces, o.cap, hint, &s.reserve))
    });
    for (s, res) in seeds.iter().zip(seed_res) {
        let (bx, p) = (&s.bx, &s.pieces);
        seen.insert(p.clone());
        match res {
            Some(mut v) => {
                v.origin = s.origin.clone();
                eprintln!(
                    "[mechmut] seed {}{}: redsim {:?} connect {} never-fed {} unfed {} class {} pistons {} events {}{}",
                    if s.origin.is_empty() { String::new() } else { format!("[{}] ", s.origin) },
                    v.line(),
                    v.real,
                    v.connect,
                    v.never,
                    v.unfed,
                    v.class,
                    v.pistons,
                    v.events,
                    if v.replayed { " (its own schedule replays)" } else { "" }
                );
                beam.push(v.clone());
                kept.push(v);
            }
            None => {
                // why (without re-solving: the seed's solve just ran)
                let why = fam::with_qc_off(s.qc_off, || match Mech::new(*bx, p) {
                    None => format!("families refuses the rest state ({})", refusal(*bx, p)),
                    Some(_) => format!("no schedule within the cap {}, or one a filter drops (FULL frame, reserve, inside, unfed)", o.cap),
                });
                eprintln!("[mechmut] seed fails{}: {why}: {} {:?}", if s.origin.is_empty() { String::new() } else { format!(" [{}]", s.origin) }, p.iter().map(|q| q.code()).collect::<Vec<_>>().join(" "), bx);
                if o.repair {
                    // its neighbours may solve (e.g. a turned mechanism whose stacked pistons now
                    // couple by QC): search from it anyway
                    let mut v = fam::with_qc_off(s.qc_off, || partial(*bx, p, 10));
                    v.reserve = s.reserve.clone();
                    v.origin = s.origin.clone();
                    if ADD.load(std::sync::atomic::Ordering::Relaxed) {
                        v.missing = fam::with_qc_off(s.qc_off, || progress(*bx, p, o.bfs_cap.max(100_000)));
                        eprintln!("[mechmut]   (growth from it: {} 3x3 cells stay filled at best)", v.missing);
                    }
                    beam.push(v);
                }
            }
        }
    }
    for round in 1..=o.rounds {
        let mut cand: Vec<(usize, Vec<Pc>, Change)> = Vec::new();
        for (k, v) in beam.iter().enumerate() {
            for (n, ch) in neighbours(v.bx, &v.pieces, &o.keep, &v.reserve) {
                if seen.insert(n.clone()) {
                    cand.push((k, n, ch));
                }
            }
        }
        if cand.is_empty() {
            break;
        }
        let t0 = std::time::Instant::now();
        let grow = ADD.load(std::sync::atomic::Ordering::Relaxed);
        let res = cs::par_map(&cand, o.threads, |(k, p, ch)| {
            let parent = &beam[*k];
            fam::with_qc_off(parent.qc_off, || {
                let r = match eval(parent.bx, p, o.bfs_cap, Some((&parent.so, &parent.sc, *ch)), &parent.reserve) {
                    Some(v) => Some(v),
                    // --add growth: an unsolved neighbour steers the beam by how far it opens, only
                    // when it opens strictly further than its (unsolved) parent
                    None if grow && parent.missing > 0 => {
                        let mi = progress(parent.bx, p, o.bfs_cap.clamp(20_000, 200_000));
                        (mi < parent.missing || (mi == parent.missing && mi < 10)).then(|| {
                            let mut v = partial(parent.bx, p, mi);
                            v.reserve = parent.reserve.clone();
                            v
                        })
                    }
                    None => None,
                };
                r.map(|mut v| {
                    v.origin = parent.origin.clone();
                    v
                })
            })
        });
        let new: Vec<Var> = res.into_iter().flatten().collect();
        let nrep = new.iter().filter(|v| v.replayed).count();
        let npart = new.iter().filter(|v| v.missing > 0).count();
        eprintln!("[mechmut] round {round}: {} neighbours, {} kept ({} by replay{}) ({:.0}s)", cand.len(), new.len() - npart, nrep, if grow { format!("; {npart} partial growths") } else { String::new() }, t0.elapsed().as_secs_f64());
        kept.extend(new.iter().filter(|v| v.missing == 0).cloned());
        let mut next = new;
        next.sort_by_key(|v| v.key());
        beam = next.into_iter().take(o.beam).collect();
        if beam.is_empty() {
            break;
        }
        let b = &beam[0];
        let nreal = beam.iter().filter(|v| v.real == Some(true)).count();
        if b.missing > 0 {
            eprintln!("[mechmut]   best is partial ({} 3x3 cells stay filled at best, + 1): {}", b.missing, b.line());
        } else {
            eprintln!("[mechmut]   best ({nreal} of the beam WORK in redsim): redsim {:?} connect {} never-fed {} unfed {} ({}) max-actions {} class {} pistons {} events {}: {}", b.real, b.connect, b.never, b.unfed, b.unfed_at, b.maxp, b.class, b.pistons, b.events, b.line());
        }
    }
    kept.sort_by_key(|v| v.key());
    kept
}

/// Write the ideal-input design + plan (open/close schedule) of a variant into `dir`; when the
/// plan cannot be built (a piston firing from inside the opening has no input cell), the bare
/// design (`{tag}.txt`) for `door fullmech --check --export`. Inputs may sit in the hallway (test
/// only, see `mechsynth::CORRIDOR_INPUTS`): the plan header says so.
pub fn export(v: &Var, dir: &Path, tag: &str) -> Result<()> {
    fam::with_qc_off(v.qc_off, || export_inner(v, dir, tag))
}

fn export_inner(v: &Var, dir: &Path, tag: &str) -> Result<()> {
    crate::mechsynth::CORRIDOR_INPUTS.store(true, std::sync::atomic::Ordering::Relaxed);
    let (m, rest) = fam::with_qc_off(v.qc_off, || Mech::new(v.bx, &v.pieces)).context("pieces overlap")?;
    let (o, c) = (from_sched(&m, &v.so, None), from_sched(&m, &v.sc, None));
    let sol = fam::Sol { open: o, close: c, sym: None, sym_levels: false, states: 0, extent: (0, 0, 0, 0), levels: false };
    let (wld, map) = fam::to_world(&m);
    std::fs::create_dir_all(dir)?;
    let mut bare = crate::mechsynth::design_of_world(&wld, tag);
    stone_statics(&mut bare);
    bare.save(&dir.join(format!("{tag}.txt")))?;
    for (stag, msol) in fam::ideal_solutions(&m, &rest, &sol, &map) {
        let p = crate::mechsynth::ideal_plan(&wld, &msol, false, 6, tag)?;
        let dpath = dir.join(format!("{tag}-{stag}.txt"));
        let mut dd = p.design.clone();
        stone_statics(&mut dd);
        dd.name = format!("{tag}-{stag}");
        dd.save(&dpath)?;
        let ipath = dir.join(format!("{tag}-{stag}.ideal"));
        let text = format!("phase between\n{}", p.text.replace(&format!("design {tag}.txt"), &format!("design {}", dpath.display())));
        std::fs::write(&ipath, &text)?;
        // replay it in redsim (between-ticks inputs, 32 positions, 2 cycles): the macro model
        // and the simulator can disagree (quasi-connectivity, timing)
        let verdict = match crate::ideal::Schedule::parse(&text) {
            Ok(s) => match crate::ideal_rs::run(&s, &std::collections::BTreeMap::new(), &crate::ideal::offsets(28), 2, false) {
                Ok((o, c)) => format!("WORKS in redsim (32 positions x 2 cycles): open after {o} ticks / closed after {c} ticks"),
                Err(e) => format!("BROKEN in redsim: {}", e.chars().take(160).collect::<String>()),
            },
            Err(e) => format!("plan does not parse: {e:#}"),
        };
        eprintln!("[mechmut] {}: {verdict}", ipath.display());
    }
    Ok(())
}

/// The command: read the seeds, search, print the ranked TSV, export the top designs.
pub fn run(input: &Path, o: &Opts, out: Option<&Path>, ideal_dir: Option<&Path>) -> Result<()> {
    let seeds = read_seeds(input, &o.reserve, o.reserve_wiring, !o.macrosat_qc)?;
    let kept = search(&seeds, o);
    let mut tsv = String::from("redsim\tinside\tweak\tconnect\tnever_fed\tunfed\tclass\tpistons\tevents\tdoor_ring\treplayed\tmechanism\topen\tclose\tunfed_at\tmax_actions\tpocket_load_x10\tfeeds_none\tfeeds_conflicts\tfeeds_weak\tisland_start\tfeeds_cannot_start\textent\torigin\tstrict_fail\n");
    for v in &kept {
        let rs = match v.real {
            Some(true) => "WORKS",
            Some(false) => "BROKEN",
            None => "-",
        };
        tsv.push_str(&format!(
            "{rs}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}x{}\t{}\t{}\n",
            v.inside, v.weak, v.connect, v.never, v.unfed, v.class, v.pistons, v.events, v.door_ring, v.replayed, v.line(), v.open, v.close, v.unfed_at, v.maxp, v.load, v.feeds.0, v.feeds.2, v.feeds.3, v.start, v.feeds.1, v.extent.0, v.extent.1, v.origin, v.strict
        ));
    }
    match out {
        Some(p) => std::fs::write(p, &tsv)?,
        None => print!("{tsv}"),
    }
    if let Some(dir) = ideal_dir {
        for (k, v) in kept.iter().take(o.top).enumerate() {
            if let Err(e) = export(v, dir, &format!("mm{:02}", k + 1)) {
                eprintln!("[mechmut] export {}: {e:#}", k + 1);
            }
        }
    }
    eprintln!("[mechmut] {} variants kept", kept.len());
    Ok(())
}
