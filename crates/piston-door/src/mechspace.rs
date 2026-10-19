//! `mctas door mechspace`: an EXACT and scalable decision procedure for families' macro pulse
//! model (`families::Mech::apply`, quasi-connectivity coupling included), and the drivers that
//! run it over families' candidates (validation against the full-graph solver, the unresolved
//! lists, widened generators).
//!
//! The question per candidate: is there an open state `o` (FULL seamless, `Mech::is_open`) with
//! rest ->* o ->* rest in the pulse model (2 gt pulses on every piston, levels on pistons
//! extended at rest), such that the hole rule (`Mech::holes_ok`) holds on that cycle?
//!
//! Method: partial-order reduction with STUBBORN SETS for goal reachability (Valmari; the
//! landmark form of Wehrle & Helmert). At a non-goal state s the search expands only a set T(s)
//! of events such that
//!   (D1) every event of T(s) commutes with every sequence of events outside T(s), and
//!   (L)  no goal state is reachable from s with events outside T(s) only.
//! Then every goal state reachable from s is reachable in the reduced graph (induction on the
//! path length: the first T-event of a path to the goal can be moved to the front). Goal states
//! themselves are fully expanded. Hence the reduced open search finds EVERY reachable open state,
//! and the reduced close search (goal = the exact rest state) decides closing exactly.
//!
//! T(s) is computed from exact FOOTPRINTS: a re-implementation of `Mech::apply` that records
//! every cell (and power / QC bit) it reads; the write set is the diff of the states. Events
//! are grouped (union-find) when one writes what the other reads; each small group's LOCAL
//! reach (only its own events, from s) is explored exactly and its reads / writes over that
//! reach collected; groups whose reaches conflict are merged, a group whose reach exceeds the
//! local cap becomes part of the unbounded core U (only U's footprints AT s matter: U is always
//! in T). The groups left outside are independent and their joint reach is the product of their
//! local reaches, so (L) is checked exactly per group, and (D1) holds by disjointness.
//! The hole rule is exact too: open states whose holes need cells kept air are re-searched in
//! the restricted system where those cells must stay air at every state of the cycle.

use crate::families::{self, Bx, Mech, Out, Pc, St, E, K, AIR, BODY, DOOR, HEAD, MAXC, MAX_PUSH, OBS, RB, STONE};
use crate::mechsynth::{self, Act};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

// ---------------------------------------------------------------- footprints

const NB: usize = 3;
/// Bit set over cells (0..MAXC), power bits (POWB + id) and QC bits (QCB + id).
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Fp(pub [u64; NB]);
const POWB: usize = MAXC;
const QCB: usize = MAXC + 16;

impl Fp {
    fn set(&mut self, i: usize) {
        self.0[i >> 6] |= 1u64 << (i & 63);
    }
    fn has(&self, i: usize) -> bool {
        (self.0[i >> 6] >> (i & 63)) & 1 == 1
    }
    fn or(&mut self, o: &Fp) {
        for k in 0..NB {
            self.0[k] |= o.0[k];
        }
    }
    fn meets(&self, o: &Fp) -> bool {
        (0..NB).any(|k| self.0[k] & o.0[k] != 0)
    }
    fn is_empty(&self) -> bool {
        self.0.iter().all(|&w| w == 0)
    }
    fn ones(&self) -> impl Iterator<Item = usize> + '_ {
        (0..NB).flat_map(move |k| {
            let mut w = self.0[k];
            std::iter::from_fn(move || {
                if w == 0 {
                    return None;
                }
                let t = w.trailing_zeros() as usize;
                w &= w - 1;
                Some(k * 64 + t)
            })
        })
    }
}

fn g_t(s: &St, i: usize, t: &mut Fp) -> u8 {
    t.set(i);
    s.g[i]
}
fn pp_t(s: &St, id: usize, t: &mut Fp) -> usize {
    let p = s.pp[id] as usize;
    t.set(p);
    p
}

fn ext_t(m: &Mech, s: &St, id: usize, t: &mut Fp) -> bool {
    let (_, f) = m.info[id];
    let p = pp_t(s, id, t);
    match m.step(p, f) {
        Some(h) => g_t(s, h, t) == HEAD + id as u8,
        None => false,
    }
}

fn pushable_t(m: &Mech, s: &St, c: u8, t: &mut Fp) -> bool {
    match c {
        DOOR | STONE | RB => true,
        OBS => false,
        c if (BODY..HEAD).contains(&c) => !ext_t(m, s, (c - BODY) as usize, t),
        _ => false,
    }
}

fn push_list_t(m: &Mech, s: &St, start: Option<usize>, f: u8, t: &mut Fp) -> Result<Vec<usize>, bool> {
    let mut list = Vec::new();
    let mut c = start.ok_or(true)?;
    while g_t(s, c, t) != AIR {
        if !pushable_t(m, s, s.g[c], t) {
            return Err(false);
        }
        list.push(c);
        if list.len() > MAX_PUSH {
            return Err(false);
        }
        c = m.step(c, f).ok_or(true)?;
    }
    Ok(list)
}

fn move_list(m: &Mech, s: &mut St, list: &[usize], f: u8) {
    for &c in list.iter().rev() {
        let n = m.step(c, f).unwrap();
        let v = s.g[c];
        s.g[n] = v;
        s.g[c] = AIR;
        if (BODY..HEAD).contains(&v) {
            s.pp[(v - BODY) as usize] = n as u8;
        }
    }
}

fn pull_t(m: &Mech, s: &mut St, p: usize, f: u8, t: &mut Fp) {
    let Some(one) = m.step(p, f) else { return };
    let Some(two) = m.step(one, f) else { return };
    let c = g_t(s, two, t);
    let o = g_t(s, one, t);
    if o != AIR || c == AIR || c == OBS || !pushable_t(m, s, c, t) {
        return;
    }
    s.g[one] = c;
    s.g[two] = AIR;
    if (BODY..HEAD).contains(&c) {
        s.pp[(c - BODY) as usize] = one as u8;
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_inner_t(m: &Mech, s: &St, e: E, id: usize, sticky: bool, f: u8, p: usize, bit: u16, powered: bool, ext: bool, t: &mut Fp) -> Out {
    let mut n = s.clone();
    match e.act {
        Act::L => {
            if powered {
                n.pow &= !bit;
                if ext {
                    let h = m.step(p, f).unwrap();
                    n.g[h] = AIR;
                    if sticky {
                        pull_t(m, &mut n, p, f, t);
                    }
                }
                Out::Ok(n)
            } else {
                match push_list_t(m, s, m.step(p, f), f, t) {
                    Err(_) => Out::Illegal,
                    Ok(list) => {
                        move_list(m, &mut n, &list, f);
                        n.g[m.step(p, f).unwrap()] = HEAD + id as u8;
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
            match push_list_t(m, s, m.step(p, f), f, t) {
                Err(true) => Out::Illegal,
                Err(false) => Out::Noop,
                Ok(list) if !list.is_empty() => {
                    move_list(m, &mut n, &list, f);
                    Out::Ok(n)
                }
                Ok(_) => {
                    if !sticky {
                        return Out::Noop;
                    }
                    pull_t(m, &mut n, p, f, t);
                    if n == *s { Out::Noop } else { Out::Ok(n) }
                }
            }
        }
    }
}

fn apply_raw_t(m: &Mech, s: &St, e: E, t: &mut Fp) -> Out {
    let id = e.id as usize;
    let (sticky, f) = m.info[id];
    let p = pp_t(s, id, t);
    let bit = 1u16 << id;
    t.set(POWB + id);
    let powered = s.pow & bit != 0;
    let ext = ext_t(m, s, id, t);
    apply_inner_t(m, s, e, id, sticky, f, p, bit, powered, ext, t)
}

/// `Mech::apply` (QC coupling on) with every read recorded in `t`.
pub fn apply_t(m: &Mech, s: &St, e: E, t: &mut Fp) -> Out {
    let out = apply_raw_t(m, s, e, t);
    let Out::Ok(mut n) = out else { return out };
    let id = e.id as usize;
    let p = pp_t(s, id, t);
    let Some(below) = m.step(p, 3) else { return Out::Ok(n) };
    let c = g_t(&n, below, t);
    if !(BODY..HEAD).contains(&c) {
        return Out::Ok(n);
    }
    let j = (c - BODY) as usize;
    if j == id {
        return Out::Ok(n);
    }
    let bit = 1u16 << j;
    let ev = E { id: j as u8, act: e.act };
    t.set(POWB + j);
    t.set(POWB + id);
    t.set(QCB + j);
    match e.act {
        Act::P => {
            if m.info[id].1 != 3 && n.pow & bit == 0 && !ext_t(m, s, j, t) && s.pow & (1u16 << id) == 0 && !ext_t(m, s, id, t) && g_t(s, below, t) == BODY + j as u8 {
                let (sticky, f) = m.info[id];
                let Ok(list) = push_list_t(m, s, m.step(p, f), f, t) else { return Out::Ok(n) };
                let mut mid = s.clone();
                let pull = list.is_empty();
                move_list(m, &mut mid, &list, f);
                let h = m.step(p, f).unwrap();
                mid.g[h] = HEAD + id as u8;
                match apply_raw_t(m, &mid, ev, t) {
                    Out::Ok(x) => mid = x,
                    Out::Noop => {}
                    Out::Illegal => return Out::Illegal,
                }
                if g_t(&mid, h, t) == HEAD + id as u8 {
                    mid.g[h] = AIR;
                }
                if pull && sticky {
                    pull_t(m, &mut mid, p, f, t);
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
                    match apply_raw_t(m, &n, ev, t) {
                        Out::Ok(x) => {
                            n = x;
                            n.qc |= bit;
                        }
                        Out::Noop => {}
                        Out::Illegal => return Out::Illegal,
                    }
                }
            } else if n.qc & bit != 0 {
                if let Out::Ok(x) = apply_raw_t(m, &n, ev, t) {
                    n = x;
                }
                n.qc &= !bit;
            }
        }
    }
    Out::Ok(n)
}

fn writes(m: &Mech, a: &St, b: &St, r: &Fp) -> Fp {
    // W is a subset of the reads: only read cells can change
    let mut w = Fp::default();
    let _ = m;
    for i in r.ones() {
        if i < MAXC {
            if a.g[i] != b.g[i] {
                w.set(i);
            }
        } else if i < QCB {
            if ((a.pow ^ b.pow) >> (i - POWB)) & 1 == 1 {
                w.set(i);
            }
        } else if ((a.qc ^ b.qc) >> (i - QCB)) & 1 == 1 {
            w.set(i);
        }
    }
    w
}

#[derive(Clone)]
struct Step {
    next: Option<St>,
    r: Fp,
    w: Fp,
}

/// One event at s: its successor (None = disabled: noop, illegal, or a forbidden cell filled),
/// reads and writes.
/// The WIRABLE sub-model (`--no-door-bodies`): a transition that leaves a piston BODY inside the
/// 3x3 is disabled (such a piston could only be powered from the doorway: every catalog
/// mechanism with a working ideal plan keeps bodies out). Heads may still enter the 3x3.
pub static NO_DOOR_BODIES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn body_in_door(m: &Mech, n: &St) -> bool {
    if !NO_DOOR_BODIES.load(Ordering::Relaxed) {
        return false;
    }
    let bx = m.bx;
    for x in 0..3 {
        for y in 0..3 {
            if bx.has(x, y) && (BODY..HEAD).contains(&n.g[bx.idx(x, y)]) {
                return true;
            }
        }
    }
    false
}

/// Every state of a cycle respects the sub-model in force.
pub fn cycle_allowed(m: &Mech, rest: &St, open: &[E], close: &[E]) -> bool {
    let mut c = rest.clone();
    for e in open.iter().chain(close.iter()) {
        if let Out::Ok(n) = m.apply(&c, *e) {
            c = n;
        }
        if body_in_door(m, &c) {
            return false;
        }
    }
    true
}

fn step_ev(m: &Mech, s: &St, e: E, forbid: &Fp) -> Step {
    let mut r = Fp::default();
    match apply_t(m, s, e, &mut r) {
        Out::Ok(n) if n != *s => {
            if (!forbid.is_empty() && forbid.ones().any(|i| i < MAXC && n.g[i] != AIR)) || body_in_door(m, &n) {
                return Step { next: None, r, w: Fp::default() };
            }
            let w = writes(m, s, &n, &r);
            Step { next: Some(n), r, w }
        }
        _ => Step { next: None, r, w: Fp::default() },
    }
}

fn conflict(a: &Step, b: &Step) -> bool {
    a.w.meets(&b.r) || b.w.meets(&a.r)
}

// ---------------------------------------------------------------- goals

#[derive(Clone)]
pub enum Goal {
    /// FULL seamless open (`Mech::is_open`).
    Open,
    /// Exactly this state (the rest state for closing).
    Exact(St),
}

pub struct Cx<'a> {
    pub m: &'a Mech,
    pub rest: St,
    pub evs: Vec<E>,
    pub cap_local: usize,
    pub max_group: usize,
    pub no_por: bool,
    /// Indices of every cell and bit of the box (for the landmark checks).
    idx: Vec<usize>,
}

impl<'a> Cx<'a> {
    pub fn new(m: &'a Mech, rest: &St) -> Cx<'a> {
        assert!(packable(m), "mechspace: boxes of at most 64 cells, at most 13 pistons, no redstone blocks");
        let mut idx: Vec<usize> = (0..m.bx.n()).collect();
        for id in 0..m.info.len() {
            idx.push(POWB + id);
            idx.push(QCB + id);
        }
        Cx { m, rest: rest.clone(), evs: m.events_pulse(), cap_local: 32, max_group: 6, no_por: false, idx }
    }

    fn compat(&self, goal: &Goal, t: &St, i: usize) -> bool {
        match goal {
            Goal::Open => {
                if i >= MAXC {
                    return true;
                }
                let (x, y) = self.m.bx.xy(i);
                if families::in_t(x, y) {
                    t.g[i] == AIR
                } else if families::in_r(x, y) {
                    matches!(t.g[i], AIR | DOOR | STONE)
                } else {
                    // FRAME9: every door block sits in a frame cell when open
                    !(FRAME9.load(Ordering::Relaxed) && t.g[i] == DOOR)
                }
            }
            Goal::Exact(r) => {
                if i < MAXC {
                    t.g[i] == r.g[i]
                } else if i < QCB {
                    ((t.pow ^ r.pow) >> (i - POWB)) & 1 == 0
                } else {
                    ((t.qc ^ r.qc) >> (i - QCB)) & 1 == 0
                }
            }
        }
    }

    fn is_goal(&self, goal: &Goal, s: &St) -> bool {
        match goal {
            Goal::Open => self.m.is_open(s) && (!FRAME9.load(Ordering::Relaxed) || frame9_ok(self.m, s)),
            Goal::Exact(r) => s == r,
        }
    }
}

/// `--frame9`: when open, all 9 door blocks sit in frame cells (the 3 others take static stones).
pub static FRAME9: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn frame9_ok(m: &Mech, s: &St) -> bool {
    (0..m.bx.n()).all(|i| {
        let (x, y) = m.bx.xy(i);
        s.g[i] != DOOR || families::in_r(x, y)
    })
}

struct Grp {
    evs: Vec<usize>,
    bounded: bool,
    r: Fp,
    w: Fp,
    reach: Vec<St>,
}

impl Grp {
    /// Explore the group's local reach from s (only its events); bounded when it fits the cap.
    fn compute(&mut self, cx: &Cx, s: &St, st: &[Step], forbid: &Fp) {
        self.r = Fp::default();
        self.w = Fp::default();
        self.reach.clear();
        for &k in &self.evs {
            self.r.or(&st[k].r);
            self.w.or(&st[k].w);
        }
        if self.evs.len() > cx.max_group {
            self.bounded = false;
            return;
        }
        let mut seen: KeySet = KeySet::default();
        seen.insert(pack(cx.m, s));
        let mut q: VecDeque<St> = VecDeque::new();
        // successors of s from the precomputed steps
        for &k in &self.evs {
            if let Some(n) = &st[k].next {
                if seen.insert(pack(cx.m, n)) {
                    q.push_back(n.clone());
                }
            }
        }
        self.reach.push(s.clone());
        while let Some(t) = q.pop_front() {
            if seen.len() > cx.cap_local {
                self.bounded = false;
                self.reach.clear();
                return;
            }
            for &k in &self.evs {
                let x = step_ev(cx.m, &t, cx.evs[k], forbid);
                self.r.or(&x.r);
                self.w.or(&x.w);
                if let Some(n) = x.next {
                    if seen.insert(pack(cx.m, &n)) {
                        q.push_back(n);
                    }
                }
            }
            self.reach.push(t);
        }
        self.bounded = true;
    }
}

/// The successors the reduced graph keeps at s: (event index, next state).
fn choose(cx: &Cx, s: &St, goal: &Goal, forbid: &Fp) -> Vec<(u8, St)> {
    let ne = cx.evs.len();
    let st: Vec<Step> = cx.evs.iter().map(|&e| step_ev(cx.m, s, e, forbid)).collect();
    let all = |sel: &[usize]| -> Vec<(u8, St)> { sel.iter().filter_map(|&k| st[k].next.clone().map(|n| (k as u8, n))).collect() };
    let every: Vec<usize> = (0..ne).collect();
    if cx.no_por || cx.is_goal(goal, s) {
        return all(&every);
    }
    // union-find by conflict at s
    let mut uf: Vec<usize> = (0..ne).collect();
    fn find(uf: &mut [usize], x: usize) -> usize {
        let mut r = x;
        while uf[r] != r {
            r = uf[r];
        }
        let mut c = x;
        while uf[c] != r {
            let n = uf[c];
            uf[c] = r;
            c = n;
        }
        r
    }
    for a in 0..ne {
        for b in a + 1..ne {
            if conflict(&st[a], &st[b]) {
                let (ra, rb) = (find(&mut uf, a), find(&mut uf, b));
                if ra != rb {
                    uf[ra] = rb;
                }
            }
        }
    }
    let mut cls: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for k in 0..ne {
        let r = find(&mut uf, k);
        cls.entry(r).or_default().push(k);
    }
    // a single class holds every enabled event: no reduction possible (full expansion is always
    // a valid stubborn set), skip the group analysis
    if cls.values().filter(|c| c.iter().any(|&k| st[k].next.is_some())).count() <= 1 {
        return all(&every);
    }
    let mut groups: Vec<Grp> = cls.into_values().map(|evs| Grp { evs, bounded: false, r: Fp::default(), w: Fp::default(), reach: Vec::new() }).collect();
    for g in groups.iter_mut() {
        g.compute(cx, s, &st, forbid);
    }
    // footprint at s of a set of events
    let at_s = |evs: &[usize]| -> (Fp, Fp) {
        let (mut r, mut w) = (Fp::default(), Fp::default());
        for &k in evs {
            r.or(&st[k].r);
            w.or(&st[k].w);
        }
        (r, w)
    };
    loop {
        let mut changed = false;
        // all unbounded groups form the core U (always expanded)
        let ub: Vec<usize> = (0..groups.len()).filter(|&i| !groups[i].bounded).collect();
        if ub.len() > 1 {
            let mut evs = Vec::new();
            for &i in ub.iter().rev() {
                evs.extend(groups.remove(i).evs);
            }
            evs.sort_unstable();
            let (r, w) = at_s(&evs);
            groups.push(Grp { evs, bounded: false, r, w, reach: Vec::new() });
            changed = true;
        }
        if let Some(u) = groups.iter().position(|g| !g.bounded) {
            let (ur, uw) = (groups[u].r, groups[u].w);
            if let Some(h) = (0..groups.len()).find(|&h| h != u && (groups[h].w.meets(&ur) || uw.meets(&groups[h].r))) {
                let hg = groups.remove(h);
                let u = groups.iter().position(|g| !g.bounded).unwrap();
                groups[u].evs.extend(hg.evs);
                groups[u].evs.sort_unstable();
                let (r, w) = at_s(&groups[u].evs);
                groups[u].r = r;
                groups[u].w = w;
                continue;
            }
        }
        // bounded vs bounded over their reaches
        'pairs: for a in 0..groups.len() {
            if !groups[a].bounded {
                continue;
            }
            for b in a + 1..groups.len() {
                if !groups[b].bounded {
                    continue;
                }
                if groups[a].w.meets(&groups[b].r) || groups[b].w.meets(&groups[a].r) {
                    let gb = groups.remove(b);
                    groups[a].evs.extend(gb.evs);
                    groups[a].evs.sort_unstable();
                    let mut g = std::mem::replace(&mut groups[a], Grp { evs: Vec::new(), bounded: false, r: Fp::default(), w: Fp::default(), reach: Vec::new() });
                    g.compute(cx, s, &st, forbid);
                    if !g.bounded {
                        let (r, w) = at_s(&g.evs);
                        g.r = r;
                        g.w = w;
                    }
                    groups[a] = g;
                    changed = true;
                    break 'pairs;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // the landmark condition
    let u = groups.iter().position(|g| !g.bounded);
    let nt: Vec<usize> = (0..groups.len()).filter(|&i| groups[i].bounded).collect();
    let mut wall = Fp::default();
    for &h in &nt {
        wall.or(&groups[h].w);
    }
    let outside_ok = cx.idx.iter().all(|&i| wall.has(i) || cx.compat(goal, s, i));
    let reach_ok = |g: &Grp| g.reach.iter().any(|t| g.w.ones().all(|i| cx.compat(goal, t, i)));
    let goal_possible = outside_ok && nt.iter().all(|&h| reach_ok(&groups[h]));
    let incompat = nt.iter().copied().filter(|&h| groups[h].w.ones().any(|i| !cx.compat(goal, s, i))).min_by_key(|&h| (groups[h].reach.len(), groups[h].evs.len()));
    let out = match u {
        Some(u) => {
            if !goal_possible {
                all(&groups[u].evs)
            } else {
                match incompat {
                    Some(h) => {
                        let mut sel = groups[u].evs.clone();
                        sel.extend(groups[h].evs.iter().copied());
                        all(&sel)
                    }
                    None => all(&every),
                }
            }
        }
        None => {
            if !goal_possible {
                // every event is in a bounded group: the product of their reaches is the whole
                // reachable set from s, and it holds no goal state
                Vec::new()
            } else {
                match incompat {
                    Some(h) => all(&groups[h].evs),
                    None => all(&every),
                }
            }
        }
    };
    if DIAG_ON.load(Ordering::Relaxed) {
        let en = st.iter().filter(|x| x.next.is_some()).count();
        DIAG[0].fetch_add(1, Ordering::Relaxed);
        DIAG[1].fetch_add(en, Ordering::Relaxed);
        DIAG[2].fetch_add(out.len(), Ordering::Relaxed);
        if u.is_some() {
            DIAG[3].fetch_add(1, Ordering::Relaxed);
            DIAG[4].fetch_add(groups[u.unwrap()].evs.len(), Ordering::Relaxed);
        }
        if out.len() == en && en >= 3 {
            DIAG[5].fetch_add(1, Ordering::Relaxed);
            let mut sm = DIAG_SAMPLES.lock().unwrap();
            if sm.len() < 4 && DIAG[5].load(Ordering::Relaxed) % 997 == 1 {
                let gs: Vec<String> = groups.iter().map(|g| format!("{}[{}]", if g.bounded { "B" } else { "U" }, g.evs.iter().map(|&k| cx.m.fmt_ev(cx.evs[k])).collect::<Vec<_>>().join(" "))).collect();
                sm.push(format!("{}groups: {}\nexpanded {} of {} enabled", cx.m.render(s), gs.join(" | "), out.len(), en));
            }
        }
    }
    out
}

pub static DIAG_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub static DIAG: [AtomicUsize; 6] = [AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0)];
pub static DIAG_SAMPLES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// `mechspace diag`: the reduced open search of one candidate with reduction statistics.
pub fn diag(bx: Bx, v: &[Pc], cap: usize, cap_local: usize, max_group: usize) -> Result<()> {
    let (m, rest) = Mech::new(bx, v).context("invalid mechanism")?;
    let mut cx = Cx::new(&m, &rest);
    cx.cap_local = cap_local;
    cx.max_group = max_group;
    DIAG_ON.store(true, Ordering::Relaxed);
    let t0 = std::time::Instant::now();
    let g = search(&cx, std::slice::from_ref(&rest), &Goal::Open, &Fp::default(), cap, true);
    let d: Vec<usize> = DIAG.iter().map(|a| a.load(Ordering::Relaxed)).collect();
    println!("{}\nopen search: {} states (complete {}), {} open states, {:.1}s", m.render(&rest), g.len(), g.complete, g.goals.len(), t0.elapsed().as_secs_f64());
    println!("expanded {} states: avg enabled {:.2}, avg kept {:.2}; core present at {} states (avg {:.1} events); no reduction at {} states", d[0], d[1] as f64 / d[0].max(1) as f64, d[2] as f64 / d[0].max(1) as f64, d[3], d[4] as f64 / d[3].max(1) as f64, d[5]);
    for s in DIAG_SAMPLES.lock().unwrap().iter() {
        println!("---\n{s}");
    }
    Ok(())
}

// ---------------------------------------------------------------- reduced search

/// Compact state key (boxes of at most 64 cells, at most 13 pistons): door / stone / redstone
/// block masks, then 9 bits per piston (cell, extended, powered, QC-held). Obsidian never moves
/// (taken from the pieces); heads sit in front of extended bodies.
const KW: usize = 5;
type Key = [u64; KW];

/// A fast multiplicative hasher for the 40-byte keys (SipHash dominated the lean searches).
#[derive(Default, Clone, Copy)]
pub struct KeyHasher(u64);
impl std::hash::Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        let mut h = self.0;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h
    }
    fn write(&mut self, bytes: &[u8]) {
        for c in bytes.chunks(8) {
            let mut b = [0u8; 8];
            b[..c.len()].copy_from_slice(c);
            self.write_u64(u64::from_le_bytes(b));
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x9E3779B97F4A7C15);
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64);
    }
}
type KB = std::hash::BuildHasherDefault<KeyHasher>;
type KeySet = HashSet<Key, KB>;
type KeyMap = HashMap<Key, u32, KB>;

fn pack(m: &Mech, s: &St) -> Key {
    let n = m.bx.n();
    let mut k = [0u64; KW];
    for i in 0..n {
        match s.g[i] {
            DOOR => k[0] |= 1u64 << i,
            STONE => k[1] |= 1u64 << i,
            RB => k[2] |= 1u64 << i,
            _ => {}
        }
    }
    let mut bit = 0usize;
    for id in 0..m.info.len() {
        let p = s.pp[id] as usize;
        let (_, f) = m.info[id];
        let ext = m.step(p, f).is_some_and(|h| s.g[h] == HEAD + id as u8) as u64;
        let v = (p as u64) | (ext << 6) | ((((s.pow >> id) & 1) as u64) << 7) | ((((s.qc >> id) & 1) as u64) << 8);
        let (w, o) = (3 + bit / 64, bit % 64);
        k[w] |= v << o;
        if o > 55 {
            k[w + 1] |= v >> (64 - o);
        }
        bit += 9;
    }
    k
}

fn unpack(m: &Mech, k: &Key) -> St {
    let n = m.bx.n();
    let mut s = St { g: [AIR; MAXC], pow: 0, pp: [0; 16], qc: 0 };
    for p in &m.pieces {
        if let K::Obsidian = p.k {
            s.g[m.bx.idx(p.x, p.y)] = OBS;
        }
    }
    for i in 0..n {
        if (k[0] >> i) & 1 == 1 {
            s.g[i] = DOOR;
        } else if (k[1] >> i) & 1 == 1 {
            s.g[i] = STONE;
        } else if (k[2] >> i) & 1 == 1 {
            s.g[i] = RB;
        }
    }
    let mut bit = 0usize;
    for id in 0..m.info.len() {
        let (w, o) = (3 + bit / 64, bit % 64);
        let mut v = k[w] >> o;
        if o > 55 {
            v |= k[w + 1] << (64 - o);
        }
        let p = (v & 63) as usize;
        s.g[p] = BODY + id as u8;
        s.pp[id] = p as u8;
        if (v >> 6) & 1 == 1 {
            let (_, f) = m.info[id];
            s.g[m.step(p, f).unwrap()] = HEAD + id as u8;
        }
        s.pow |= (((v >> 7) & 1) as u16) << id;
        s.qc |= (((v >> 8) & 1) as u16) << id;
        bit += 9;
    }
    s
}

/// Can this mechanism use the compact key?
pub fn packable(m: &Mech) -> bool {
    m.bx.n() <= 64 && m.info.len() <= 13 && !m.has_r
}

pub struct Graph {
    keys: Vec<Key>,
    parent: Vec<(u32, u8)>,
    pub goals: Vec<u32>,
    pub complete: bool,
}

const ROOT: u32 = u32::MAX;

impl Graph {
    /// (index of the root start, events from it).
    fn path(&self, mut i: u32) -> (u32, Vec<u8>) {
        let mut v = Vec::new();
        while self.parent[i as usize].0 != ROOT {
            let (p, e) = self.parent[i as usize];
            v.push(e);
            i = p;
        }
        v.reverse();
        (i, v)
    }
    pub fn len(&self) -> usize {
        self.keys.len()
    }
}

/// Reduced BFS from `starts`. `all_goals`: keep going through goal states (open search: every
/// reachable open state); otherwise stop at the first goal.
pub fn search(cx: &Cx, starts: &[St], goal: &Goal, forbid: &Fp, cap: usize, all_goals: bool) -> Graph {
    let m = cx.m;
    let mut g = Graph { keys: Vec::new(), parent: Vec::new(), goals: Vec::new(), complete: true };
    let mut index: KeyMap = KeyMap::default();
    let mut q: VecDeque<u32> = VecDeque::new();
    for (si, s) in starts.iter().enumerate() {
        let k = pack(m, s);
        if index.contains_key(&k) {
            continue;
        }
        let i = g.keys.len() as u32;
        index.insert(k, i);
        g.keys.push(k);
        g.parent.push((ROOT, (si & 0xff) as u8));
        q.push_back(i);
    }
    while let Some(i) = q.pop_front() {
        let s = unpack(m, &g.keys[i as usize]);
        if cx.is_goal(goal, &s) {
            g.goals.push(i);
            if !all_goals {
                return g;
            }
        }
        for (e, n) in choose(cx, &s, goal, forbid) {
            let k = pack(m, &n);
            if index.contains_key(&k) {
                continue;
            }
            if g.keys.len() >= cap {
                g.complete = false;
                continue;
            }
            let j = g.keys.len() as u32;
            index.insert(k, j);
            g.keys.push(k);
            g.parent.push((i, e));
            q.push_back(j);
        }
    }
    g
}

/// FULL FRAME (vjeux 03:36Z): when open, all 12 frame cells hold door blocks or STATIC plain
/// blocks. The search keeps mechanisms stone-free: a frame cell that is air when open must stay
/// air on the whole cycle (so a static stone can sit there without changing a transition, which
/// the replay of the stone-augmented mechanism re-checks); the box must hold the whole frame.
pub static FULL_FRAME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn bad_holes(m: &Mech, o: &St) -> Fp {
    let mut b = Fp::default();
    let bx = m.bx;
    let full = FULL_FRAME.load(Ordering::Relaxed);
    for (x, y) in families::ring() {
        if !bx.has(x, y) || o.g[bx.idx(x, y)] != AIR {
            continue;
        }
        if full {
            b.set(bx.idx(x, y));
            continue;
        }
        let (ox, oy) = families::outward(x, y);
        if bx.has(ox, oy) && !matches!(o.g[bx.idx(ox, oy)], AIR | DOOR) {
            b.set(bx.idx(x, y));
        }
    }
    b
}

/// The frame inside the box (needed for FULL FRAME).
pub fn frame_in_box(bx: &Bx) -> bool {
    families::ring().into_iter().all(|(x, y)| bx.has(x, y))
}

/// Frame cells holding a door block or a plain block in state s (out of 12).
pub fn frame_count(m: &Mech, s: &St) -> usize {
    families::ring().into_iter().filter(|&(x, y)| m.bx.has(x, y) && matches!(s.g[m.bx.idx(x, y)], DOOR | STONE)).count()
}

/// FULL FRAME augmentation: static stones in the frame cells left air by the found cycle's open
/// state, then the augmented mechanism re-decided exactly and its cycle replayed: open state
/// frame 12/12, every stone static on the cycle. Some(pieces, open, close).
pub fn augment_frame(bx: Bx, v: &[Pc], open: &[E], cap: usize, cap_lean: usize) -> Option<(Vec<Pc>, Vec<E>, Vec<E>)> {
    let (m, rest) = Mech::new(bx, v)?;
    let mut o = rest.clone();
    for e in open {
        if let Out::Ok(n) = m.apply(&o, *e) {
            o = n;
        }
    }
    let mut aug = v.to_vec();
    for (x, y) in families::ring() {
        if bx.has(x, y) && o.g[bx.idx(x, y)] == AIR {
            aug.push(Pc { x, y, k: K::Stone });
        }
    }
    let (m2, rest2) = Mech::new(bx, &aug)?;
    let cx2 = Cx::new(&m2, &rest2);
    let (op, cl) = match decide_all(&cx2, cap.min(500_000), cap_lean.max(cap), 0).0 {
        Verdict::Found { open, close } if !open.is_empty() => (open, close),
        _ => return None,
    };
    // replay: frame 12/12 when open, stones static
    let stones: Vec<usize> = aug.iter().filter(|p| matches!(p.k, K::Stone)).map(|p| bx.idx(p.x, p.y)).collect();
    let mut c = rest2.clone();
    for (k, e) in op.iter().chain(cl.iter()).enumerate() {
        match m2.apply(&c, *e) {
            Out::Ok(n) => c = n,
            _ => return None,
        }
        if stones.iter().any(|&i| c.g[i] != STONE) {
            return None;
        }
        if k + 1 == op.len() && (!m2.is_open(&c) || frame_count(&m2, &c) != 12) {
            return None;
        }
    }
    Some((aug, op, cl))
}

/// Rule (A) (vjeux 05:19Z): the 3 extra frame blocks are door-type blocks that MAY MOVE, as long
/// as they are in place when open and when closed. For a candidate without a static-extras cycle:
/// for each hole mask of the open states that can come back to rest (fewest first, at most 16),
/// put 3 extra door-type blocks in those frame cells at rest (interchangeable with the door
/// blocks) and decide the variant exactly; open = 12 door-type blocks in the 12 frame cells.
pub fn try_movable(bx: Bx, v: &[Pc], cap: usize, cap_lean: usize) -> Option<(Vec<Pc>, Vec<E>, Vec<E>)> {
    let (m, rest) = Mech::new(bx, v)?;
    let cx = Cx::new(&m, &rest);
    let none = Fp::default();
    let (opens, complete, visited) = search_lean_set(&cx, std::slice::from_ref(&rest), &Goal::Open, &none, cap_lean, true, false);
    if !complete || opens.is_empty() {
        return None;
    }
    let live = coreach_within(&m, &rest, &visited);
    drop(visited);
    let mut masks: std::collections::BTreeSet<(u32, Fp)> = std::collections::BTreeSet::new();
    for k in opens.iter().filter(|k| live.contains(*k)) {
        let b = bad_holes(&m, &unpack(&m, k));
        let c: u32 = b.0.iter().map(|w| w.count_ones()).sum();
        masks.insert((c, b));
    }
    for (_, mask) in masks.iter().take(16) {
        let mut aug = v.to_vec();
        for i in mask.ones() {
            let (x, y) = bx.xy(i);
            aug.push(Pc { x, y, k: K::Stone });
        }
        let Some((m2, mut rest2)) = Mech::new(bx, &aug) else { continue };
        for i in mask.ones() {
            rest2.g[i] = DOOR;
        }
        let cx2 = Cx::new(&m2, &rest2);
        if let Verdict::Found { open, close } = decide_all(&cx2, cap.min(500_000), cap_lean.max(cap), 0).0 {
            if open.is_empty() {
                continue;
            }
            let mut c = rest2.clone();
            let mut ok = true;
            for (k, e) in open.iter().chain(close.iter()).enumerate() {
                match m2.apply(&c, *e) {
                    Out::Ok(n) => c = n,
                    _ => {
                        ok = false;
                        break;
                    }
                }
                if k + 1 == open.len() && (!m2.is_open(&c) || frame_count(&m2, &c) != 12) {
                    ok = false;
                    break;
                }
            }
            if ok && c == rest2 {
                return Some((aug, open, close));
            }
        }
    }
    None
}

/// Lean search (no paths): the visited set only, goals returned as keys. `por` = the stubborn-set
/// reduction (else every enabled event, through `Mech::apply` directly).
pub fn search_lean(cx: &Cx, starts: &[St], goal: &Goal, forbid: &Fp, cap: usize, all_goals: bool, por: bool) -> (Vec<Key>, bool, usize) {
    let (g, c, set) = search_lean_set(cx, starts, goal, forbid, cap, all_goals, por);
    (g, c, set.len())
}

/// [`search_lean`] that also returns the visited set.
pub fn search_lean_set(cx: &Cx, starts: &[St], goal: &Goal, forbid: &Fp, cap: usize, all_goals: bool, por: bool) -> (Vec<Key>, bool, KeySet) {
    let m = cx.m;
    let mut seen: KeySet = KeySet::with_capacity_and_hasher(1 << 16, KB::default());
    let mut q: VecDeque<Key> = VecDeque::new();
    let mut goals = Vec::new();
    for s in starts {
        let k = pack(m, s);
        if seen.insert(k) {
            q.push_back(k);
        }
    }
    let mut complete = true;
    while let Some(k) = q.pop_front() {
        let s = unpack(m, &k);
        if cx.is_goal(goal, &s) {
            goals.push(k);
            if !all_goals {
                return (goals, complete, seen);
            }
        }
        let succ: Vec<St> = if por {
            choose(cx, &s, goal, forbid).into_iter().map(|(_, n)| n).collect()
        } else {
            let mut v = Vec::with_capacity(cx.evs.len());
            for &e in &cx.evs {
                if let Out::Ok(n) = m.apply(&s, e) {
                    if n != s && (forbid.is_empty() || !forbid.ones().any(|i| i < MAXC && n.g[i] != AIR)) && !body_in_door(m, &n) {
                        v.push(n);
                    }
                }
            }
            v
        };
        for n in succ {
            let kn = pack(m, &n);
            if seen.contains(&kn) {
                continue;
            }
            if seen.len() >= cap {
                complete = false;
                continue;
            }
            seen.insert(kn);
            q.push_back(kn);
        }
    }
    (goals, complete, seen)
}

/// States of `within` (a complete forward set) that can reach rest: backward BFS from rest over
/// pre-images, staying inside `within`.
pub fn coreach_within(m: &Mech, rest: &St, within: &KeySet) -> KeySet {
    let evs = m.events_pulse();
    let mut live: KeySet = KeySet::default();
    let k0 = pack(m, rest);
    live.insert(k0);
    let mut q: VecDeque<Key> = VecDeque::from([k0]);
    while let Some(k) = q.pop_front() {
        let t = unpack(m, &k);
        for &e in &evs {
            for s in preimages(m, &t, e) {
                let ks = pack(m, &s);
                if within.contains(&ks) && live.insert(ks) {
                    q.push_back(ks);
                }
            }
        }
    }
    live
}

/// The exact decision with a second, lean stage: when the reduced search with paths hits
/// `cap`, the open and close questions are re-decided by lean searches (visited sets only, up
/// to `cap_lean` states; the reduced one first, then the plain one), and a found cycle gets its
/// event sequences from a goal-directed search.
pub fn decide2(cx: &Cx, cap: usize, cap_lean: usize, astar: usize) -> (Verdict, Stats) {
    let (vd, st) = decide(cx, cap);
    if vd.decided() || cap_lean <= cap {
        return (vd, st);
    }
    let m = cx.m;
    let none = Fp::default();
    let rest = cx.rest.clone();
    let mut stats = st;
    // all reachable open states: the plain lean search first (fast per state), then the reduced
    // one (it preserves every open state; it wins where independent toggles multiply states)
    let (mut opens, mut complete, mut n) = search_lean(cx, std::slice::from_ref(&rest), &Goal::Open, &none, cap_lean, true, false);
    if !complete {
        let r = search_lean(cx, std::slice::from_ref(&rest), &Goal::Open, &none, (cap_lean / 4).max(cap), true, true);
        if r.1 {
            opens = r.0;
            complete = true;
            n = r.2;
        }
    }
    stats.open_states = n;
    if opens.is_empty() {
        return (if complete { Verdict::NoOpen } else { Verdict::Capped { opens: 0 } }, stats);
    }
    let nopen = opens.len();
    let mut capped = !complete;
    let mut by_mask: BTreeMap<Fp, Vec<St>> = BTreeMap::new();
    for k in &opens {
        let o = unpack(m, k);
        by_mask.entry(bad_holes(m, &o)).or_default().push(o);
    }
    for (mask, list) in &by_mask {
        let starts: Vec<St> = if mask.is_empty() {
            list.clone()
        } else {
            if mask.ones().any(|i| rest.g[i] != AIR) {
                continue;
            }
            let (g2, c2, _) = search_lean(cx, std::slice::from_ref(&rest), &Goal::Open, mask, cap_lean, true, true);
            capped |= !c2;
            g2.iter().map(|k| unpack(m, k)).filter(|o| bad_holes(m, o) == *mask).collect()
        };
        if starts.is_empty() {
            continue;
        }
        let goal = Goal::Exact(rest.clone());
        let (mut back, mut cc, mut nc) = search_lean(cx, &starts, &goal, mask, cap_lean, false, false);
        if back.is_empty() && !cc {
            let r = search_lean(cx, &starts, &goal, mask, (cap_lean / 4).max(cap), false, true);
            if !r.0.is_empty() || r.1 {
                back = r.0;
                cc = r.1;
                nc = r.2;
            }
        }
        stats.close_states += nc;
        capped |= !cc;
        if !back.is_empty() {
            // a cycle exists: get sequences (A*, then the reduced search with paths)
            if astar > 0 {
                if let Ok(Some(sol)) = families::solve_pulse_astar(m, &rest, astar) {
                    if replay_ok(m, &rest, &sol.open, &sol.close) {
                        return (Verdict::Found { open: sol.open, close: sol.close }, stats);
                    }
                }
            }
            return (Verdict::Found { open: Vec::new(), close: Vec::new() }, stats);
        }
    }
    (if capped { Verdict::Capped { opens: nopen } } else { Verdict::NoClose { opens: nopen } }, stats)
}

pub fn bad_holes_pub(m: &Mech, o: &St) -> bool {
    !bad_holes(m, o).is_empty()
}

#[derive(Clone, Debug)]
pub enum Verdict {
    /// An open + close cycle (events of the pulse model), replayed and checked with `Mech::apply`.
    Found { open: Vec<E>, close: Vec<E> },
    /// PROVEN: no open state is reachable.
    NoOpen,
    /// PROVEN: open states are reachable, none closes back (hole rule included).
    NoClose { opens: usize },
    /// A reduced search hit the state cap.
    Capped { opens: usize },
}

impl Verdict {
    pub fn tag(&self) -> &'static str {
        match self {
            Verdict::Found { .. } => "FOUND",
            Verdict::NoOpen => "no-open",
            Verdict::NoClose { .. } => "no-close",
            Verdict::Capped { .. } => "CAPPED",
        }
    }
    pub fn decided(&self) -> bool {
        !matches!(self, Verdict::Capped { .. })
    }
}

pub struct Stats {
    pub open_states: usize,
    pub close_states: usize,
}

/// The exact decision for one mechanism.
pub fn decide(cx: &Cx, cap: usize) -> (Verdict, Stats) {
    let m = cx.m;
    let none = Fp::default();
    let rest = cx.rest.clone();
    let og = search(cx, std::slice::from_ref(&rest), &Goal::Open, &none, cap, true);
    let mut stats = Stats { open_states: og.len(), close_states: 0 };
    if og.goals.is_empty() {
        return (if og.complete { Verdict::NoOpen } else { Verdict::Capped { opens: 0 } }, stats);
    }
    let mut capped = !og.complete;
    let mut by_mask: BTreeMap<Fp, Vec<u32>> = BTreeMap::new();
    for &gi in &og.goals {
        let o = unpack(m, &og.keys[gi as usize]);
        by_mask.entry(bad_holes(m, &o)).or_default().push(gi);
    }
    let evs = &cx.evs;
    let ev = |v: &[u8]| -> Vec<E> { v.iter().map(|&k| evs[k as usize]).collect() };
    for (mask, list) in &by_mask {
        let (og2, starts_idx): (Option<Graph>, Vec<u32>) = if mask.is_empty() {
            (None, list.clone())
        } else {
            if mask.ones().any(|i| rest.g[i] != AIR) {
                continue;
            }
            let g2 = search(cx, std::slice::from_ref(&rest), &Goal::Open, mask, cap, true);
            capped |= !g2.complete;
            let s: Vec<u32> = g2.goals.iter().copied().filter(|&gi| bad_holes(m, &unpack(m, &g2.keys[gi as usize])) == *mask).collect();
            (Some(g2), s)
        };
        if starts_idx.is_empty() {
            continue;
        }
        let src = og2.as_ref().unwrap_or(&og);
        let starts: Vec<St> = starts_idx.iter().map(|&gi| unpack(m, &src.keys[gi as usize])).collect();
        let cg = search(cx, &starts, &Goal::Exact(rest.clone()), mask, cap, false);
        stats.close_states += cg.len();
        capped |= !cg.complete;
        if let Some(&gr) = cg.goals.first() {
            let (root, close) = cg.path(gr);
            let rk = cg.keys[root as usize];
            let oi = starts_idx.iter().copied().find(|&gi| src.keys[gi as usize] == rk).unwrap();
            let (_, open) = src.path(oi);
            let (open, close) = (ev(&open), ev(&close));
            if replay_ok(m, &rest, &open, &close) {
                return (Verdict::Found { open, close }, stats);
            }
            eprintln!("[mechspace] BUG: a found cycle fails the replay check: {} | {}", m.fmt_seq(&open), m.fmt_seq(&close));
        }
    }
    let opens = og.goals.len();
    (if capped { Verdict::Capped { opens } } else { Verdict::NoClose { opens } }, stats)
}

/// Replay open + close with families' own `Mech::apply`: open state FULL seamless, back at the
/// exact rest state, hole rule on the cycle.
pub fn replay_ok(m: &Mech, rest: &St, open: &[E], close: &[E]) -> bool {
    let mut c = rest.clone();
    let mut cyc = vec![c.clone()];
    for e in open {
        match m.apply(&c, *e) {
            Out::Ok(n) => c = n,
            _ => return false,
        }
        cyc.push(c.clone());
    }
    if !m.is_open(&c) {
        return false;
    }
    let o = c.clone();
    for e in close {
        match m.apply(&c, *e) {
            Out::Ok(n) => c = n,
            _ => return false,
        }
        cyc.push(c.clone());
    }
    let refs: Vec<&St> = cyc.iter().collect();
    c == *rest && m.holes_ok(&refs, &o) && !cyc.iter().any(|s| body_in_door(m, s))
}

// ---------------------------------------------------------------- pre-images (backward search)

fn set_cell(y: &mut St, i: usize, v: u8) {
    y.g[i] = v;
    if (BODY..HEAD).contains(&v) {
        y.pp[(v - BODY) as usize] = i as u8;
    }
}

/// The cells of piston q's line (q's position + 1.. along its facing), in the box.
fn line_cells(m: &Mech, p: usize, f: u8, n: usize) -> Vec<usize> {
    let mut v = Vec::new();
    let mut c = p;
    for _ in 0..n {
        match m.step(c, f) {
            Some(x) => {
                v.push(x);
                c = x;
            }
            None => break,
        }
    }
    v
}

/// Candidate states before piston q's own (raw) action `act` produced x: every way its line
/// could have been (push / pull undone, head and power restored). A superset: callers verify
/// each candidate by applying the event forward.
fn undo_raw(m: &Mech, x: &St, q: usize, act: Act) -> Vec<St> {
    let (sticky, f) = m.info[q];
    let p = x.pp[q] as usize;
    let l = line_cells(m, p, f, MAX_PUSH + 2);
    let mut out = Vec::new();
    let qb = 1u16 << q;
    let shift_back = |y: &mut St, k: usize| {
        // y[l1..lk] = x[l2..l(k+1)], y[l(k+1)] = AIR
        for mm in 0..k {
            set_cell(y, l[mm], x.g[l[mm + 1]]);
        }
        y.g[l[k]] = AIR;
    };
    match act {
        Act::P => {
            if !l.is_empty() && x.g[l[0]] == AIR {
                for k in 1..=MAX_PUSH {
                    if k >= l.len() || x.g[l[k]] == AIR {
                        break;
                    }
                    let mut y = x.clone();
                    shift_back(&mut y, k);
                    out.push(y);
                }
            }
            if sticky && l.len() >= 2 && x.g[l[0]] != AIR && x.g[l[1]] == AIR {
                let mut y = x.clone();
                set_cell(&mut y, l[1], x.g[l[0]]);
                y.g[l[0]] = AIR;
                out.push(y);
            }
        }
        Act::L => {
            if x.pow & qb != 0 {
                // undo an extension: the head at l1, the line pushed by k
                if !l.is_empty() && x.g[l[0]] == HEAD + q as u8 {
                    for k in 0..=MAX_PUSH {
                        if k + 1 > l.len() {
                            break;
                        }
                        if k >= 1 && x.g[l[k]] == AIR {
                            break;
                        }
                        let mut y = x.clone();
                        y.pow &= !qb;
                        if k == 0 {
                            y.g[l[0]] = AIR;
                        } else {
                            shift_back(&mut y, k);
                        }
                        out.push(y);
                    }
                }
                // a powered piston that could not extend (QC-held)
                let mut y = x.clone();
                y.pow &= !qb;
                out.push(y);
            } else {
                // undo a retraction (with or without the sticky pull), or a power-off alone
                let mut y = x.clone();
                y.pow |= qb;
                out.push(y.clone());
                if !l.is_empty() && x.g[l[0]] == AIR {
                    let mut z = y.clone();
                    z.g[l[0]] = HEAD + q as u8;
                    out.push(z);
                }
                if l.len() >= 2 && x.g[l[0]] != AIR && x.g[l[1]] == AIR {
                    let mut z = y.clone();
                    set_cell(&mut z, l[1], x.g[l[0]]);
                    z.g[l[0]] = HEAD + q as u8;
                    out.push(z);
                }
            }
        }
    }
    out
}

/// Every state s with `Mech::apply(s, e) == Ok(t)`, s != t (candidates verified forward).
pub fn preimages(m: &Mech, t: &St, e: E) -> Vec<St> {
    let i = e.id as usize;
    let p = t.pp[i] as usize;
    let mut cands: Vec<St> = Vec::new();
    let below = m.step(p, 3).and_then(|b| {
        let c = t.g[b];
        if (BODY..HEAD).contains(&c) && (c - BODY) as usize != i { Some((c - BODY) as usize) } else { None }
    });
    for u in undo_raw(m, t, i, e.act) {
        let mut js = vec![u.clone()];
        if let Some(j) = below {
            for v in undo_raw(m, &u, j, e.act) {
                js.push(v);
            }
        }
        for v in js {
            match below {
                Some(j) => {
                    for qcb in [false, true] {
                        let mut w = v.clone();
                        if qcb {
                            w.qc |= 1 << j;
                        } else {
                            w.qc &= !(1 << j);
                        }
                        cands.push(w);
                    }
                }
                None => cands.push(v),
            }
        }
    }
    let mut out: Vec<St> = Vec::new();
    for s in cands {
        if s == *t || out.contains(&s) {
            continue;
        }
        if let Out::Ok(n) = m.apply(&s, e) {
            if n == *t {
                out.push(s);
            }
        }
    }
    out
}

/// Backward BFS from rest over pre-images: the set of states that can reach rest (capped).
pub fn coreach(m: &Mech, rest: &St, cap: usize) -> (KeySet, bool) {
    let evs = m.events_pulse();
    let mut seen: KeySet = KeySet::default();
    seen.insert(pack(m, rest));
    let mut q: VecDeque<Key> = VecDeque::from([pack(m, rest)]);
    while let Some(k) = q.pop_front() {
        let t = unpack(m, &k);
        for &e in &evs {
            for s in preimages(m, &t, e) {
                let ks = pack(m, &s);
                if seen.contains(&ks) {
                    continue;
                }
                if seen.len() >= cap {
                    return (seen, false);
                }
                seen.insert(ks);
                q.push_back(ks);
            }
        }
    }
    (seen, true)
}

/// Pre-image completeness test: every forward edge s -e-> t of random walks must have s among
/// preimages(t, e).
pub fn pretest(w: i8, h: i8, pmax: usize, n_cands: usize, walk: usize, threads: usize) -> Result<()> {
    let cands = candidates(w, h, pmax, 0, 0, &[]);
    let step = (cands.len() / n_cands.max(1)).max(1);
    let picked: Vec<&(Bx, Vec<Pc>)> = cands.iter().step_by(step).take(n_cands).collect();
    let (edges, missing) = (AtomicUsize::new(0), AtomicUsize::new(0));
    let next = AtomicUsize::new(0);
    let msgs: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= picked.len() {
                    break;
                }
                let (bx, v) = picked[i];
                let Some((m, rest)) = Mech::new(*bx, v) else { continue };
                let evs = m.events_pulse();
                let mut rng = Rng(0x2545F4914F6CDD1D ^ (i as u64 + 7));
                let mut s = rest.clone();
                for _ in 0..walk {
                    let mut succ = Vec::new();
                    for &e in &evs {
                        if let Out::Ok(t) = m.apply(&s, e) {
                            if t == s {
                                continue;
                            }
                            edges.fetch_add(1, Ordering::Relaxed);
                            if !preimages(&m, &t, e).contains(&s) {
                                missing.fetch_add(1, Ordering::Relaxed);
                                let mut ms = msgs.lock().unwrap();
                                if ms.len() < 10 {
                                    ms.push(format!("missing pre-image: {} event {}\nfrom\n{}to\n{}", line_of(bx, v), m.fmt_ev(e), m.render(&s), m.render(&t)));
                                }
                            }
                            succ.push(t);
                        }
                    }
                    if succ.is_empty() || rng.below(50) == 0 {
                        s = rest.clone();
                    } else {
                        s = succ[rng.below(succ.len())].clone();
                    }
                }
            });
        }
    });
    for mm in msgs.into_inner().unwrap() {
        println!("{mm}");
    }
    println!("pretest {w}x{h}: {} candidates, {} forward edges, {} pre-images missing", picked.len(), edges.load(Ordering::Relaxed), missing.load(Ordering::Relaxed));
    Ok(())
}

// ---------------------------------------------------------------- piston-set enumeration

/// Piston-set enumeration (coordinator 04:53Z): pistons in the cells around the door (the side
/// columns, the rows above / below, the 4 diagonal corners), facing toward the door or along the
/// frame (the head cell or the one after it is a frame or door cell), sticky or normal, retracted
/// or extended at rest; necessary-condition pruning; every survivor is then decided exactly.
/// Structure kept to make it tractable: the 6 side pullers (rows 0..2 at the two side columns,
/// sticky, facing the door) with at most `side_absent` missing and one extension state per side;
/// `bottom_su`: the box's bottom row holds exactly one piston, sticky facing up; at most
/// `free_max` other pistons (sticky only unless `free_normal`).
pub struct EnumOpts {
    pub side_absent: usize,
    pub free_max: usize,
    pub free_normal: bool,
    pub bottom_su: bool,
}

pub fn enum_sets(bx: Bx, o: &EnumOpts) -> Vec<Vec<Pc>> {
    // MECHSPACE_RET_ONLY: every piston retracted at rest (layout discovery; the extended
    // variants of each found layout are decided afterwards)
    let ret_only = std::env::var("MECHSPACE_RET_ONLY").is_ok();
    let frame: HashSet<(i8, i8)> = families::ring().into_iter().collect();
    let in_t = |x: i8, y: i8| families::in_t(x, y);
    let target = |x: i8, y: i8| in_t(x, y) || frame.contains(&(x, y));
    let side_x = [-2i8, 4];
    let mut side_cells: Vec<(i8, i8, u8)> = Vec::new(); // (x, y, facing)
    for (k, &sx) in side_x.iter().enumerate() {
        for y in 0..3 {
            if bx.has(sx, y) {
                side_cells.push((sx, y, if k == 0 { 0 } else { 1 }));
            }
        }
    }
    // free slots: every other cell outside the 3x3 and the frame with a facing toward the door
    // or along the frame
    let mut free_slots: Vec<Vec<Pc>> = Vec::new(); // per cell, its piston options
    let mut bottom_opts: Vec<Pc> = Vec::new();
    for i in 0..bx.n() {
        let (x, y) = bx.xy(i);
        // MECHSPACE_FRAME_SLOTS: pistons may also sit in frame cells at rest (they must leave
        // before the door opens: the open check needs door-type blocks there)
        let frame_slots = std::env::var("MECHSPACE_FRAME_SLOTS").is_ok();
        if in_t(x, y) || (frame.contains(&(x, y)) && !frame_slots) || side_cells.iter().any(|&(a, b, _)| a == x && b == y) {
            continue;
        }
        let mut opts = Vec::new();
        for f in 0..4u8 {
            let (dx, dy) = families::F[f as usize];
            let (h1, h2) = ((x + dx, y + dy), (x + 2 * dx, y + 2 * dy));
            // MECHSPACE_K3: also a piston whose third cell ahead is a frame or door cell (the top
            // of a 3-piston elevator)
            let h3 = (x + 3 * dx, y + 3 * dy);
            let k3 = std::env::var("MECHSPACE_K3").is_ok() && target(h3.0, h3.1);
            if !(target(h1.0, h1.1) || target(h2.0, h2.1) || k3) || !bx.has(h1.0, h1.1) {
                continue;
            }
            for sticky in [true, false] {
                if !sticky && !o.free_normal {
                    continue;
                }
                for ext in [false, true] {
                    if ext && (in_t(h1.0, h1.1) || ret_only) {
                        continue;
                    }
                    opts.push(Pc { x, y, k: K::Piston { sticky, f, ext } });
                }
            }
        }
        if o.bottom_su && y == bx.y0 {
            bottom_opts.extend(opts.into_iter().filter(|p| matches!(p.k, K::Piston { sticky: true, f: 2, .. })));
            continue;
        }
        if !opts.is_empty() {
            free_slots.push(opts);
        }
    }
    let mut out = Vec::new();
    // side combos: presence (at most side_absent missing) x one extension state per side
    let ns = side_cells.len();
    let mut side_sets: Vec<Vec<Pc>> = Vec::new();
    for mask in 0u32..(1 << ns) {
        if (ns as u32 - mask.count_ones()) as usize > o.side_absent {
            continue;
        }
        for exts in 0..(if ret_only { 1u8 } else { 4u8 }) {
            let mut v = Vec::new();
            for (j, &(x, y, f)) in side_cells.iter().enumerate() {
                if mask & (1 << j) == 0 {
                    continue;
                }
                let ext = if x < 0 { exts & 1 == 1 } else { exts & 2 == 2 };
                v.push(Pc { x, y, k: K::Piston { sticky: true, f, ext } });
            }
            side_sets.push(v);
        }
    }
    let bottoms: Vec<Option<Pc>> = if o.bottom_su { bottom_opts.iter().map(|p| Some(*p)).collect() } else { vec![None] };
    // free subsets, fewest first
    fn rec(slots: &[Vec<Pc>], start: usize, left: usize, cur: &mut Vec<Pc>, out: &mut Vec<Vec<Pc>>) {
        out.push(cur.clone());
        if left == 0 {
            return;
        }
        for s in start..slots.len() {
            for &p in &slots[s] {
                // a head cell may not hold another piece
                cur.push(p);
                rec(slots, s + 1, left - 1, cur, out);
                cur.pop();
            }
        }
    }
    let mut frees: Vec<Vec<Pc>> = Vec::new();
    rec(&free_slots, 0, o.free_max, &mut Vec::new(), &mut frees);
    frees.sort_by_key(|v| v.len());
    // pruning in parallel (the reach-matching condition costs ~0.5 ms a set)
    let nth = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(64);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let parts: Vec<Vec<Vec<Pc>>> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..nth)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if i >= frees.len() {
                            break;
                        }
                        let fr = &frees[i];
                        for ss in &side_sets {
                            for b in &bottoms {
                                let mut v: Vec<Pc> = ss.clone();
                                v.extend(fr.iter().copied());
                                if let Some(p) = b {
                                    v.push(*p);
                                }
                                if !prune_ok(&bx, &v, &frame) {
                                    continue;
                                }
                                v.sort();
                                mine.push(v);
                            }
                        }
                    }
                    mine
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    for p in parts {
        out.extend(p);
    }
    out.sort();
    out.sort_by_key(|v| v.len());
    out.dedup();
    // mirror images (x -> 2 - x) are the same mechanism: keep the smaller of each pair when the
    // box is symmetric about the door
    if bx.x0 + bx.w - 1 == 2 - bx.x0 {
        out.retain(|v| {
            let mut mv: Vec<Pc> = v.iter().map(|p| p.mirror()).collect();
            mv.sort();
            *v <= mv
        });
    }
    out
}

/// Necessary conditions: pieces fit (heads included) and are a valid rest state; every door cell
/// lies on the forward line of some piston; at least 9 frame cells can receive a block (on a
/// piston's forward line).
fn prune_ok(bx: &Bx, v: &[Pc], frame: &HashSet<(i8, i8)>) -> bool {
    let mut occ: HashSet<(i8, i8)> = HashSet::new();
    for p in v {
        let K::Piston { f, ext, .. } = p.k else { continue };
        if !occ.insert((p.x, p.y)) {
            return false;
        }
        if ext {
            let (dx, dy) = families::F[f as usize];
            if !bx.has(p.x + dx, p.y + dy) || !occ.insert((p.x + dx, p.y + dy)) {
                return false;
            }
        }
    }
    let on_line = |c: (i8, i8)| -> bool {
        v.iter().any(|p| {
            let K::Piston { f, .. } = p.k else { return false };
            let (dx, dy) = families::F[f as usize];
            (1..=13).any(|k| (p.x + k * dx, p.y + k * dy) == c)
        })
    };
    for x in 0..3 {
        for y in 0..3 {
            if !on_line((x, y)) {
                return false;
            }
        }
    }
    let recv = frame.iter().filter(|&&c| on_line(c)).count();
    recv >= 9 && (!REACH_PRUNE.load(Ordering::Relaxed) || reach_ok(bx, v, frame))
}

/// `MECHSPACE_REACH`: a stronger necessary condition for FRAME9. Relaxed block moves (no blocking,
/// no timing): a block on a piston's forward line can be pushed one cell along it; a block two
/// cells ahead of a sticky piston can be pulled into its head cell. Pistons that lie on another
/// piston's line may be displaced 1-2 cells along it, so their lines are also taken from those
/// cells. Each door block's reachable cells; the 9 door blocks must be matched to 9 distinct
/// frame cells they can reach (bipartite matching).
pub static REACH_PRUNE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn reach_ok(bx: &Bx, v: &[Pc], frame: &HashSet<(i8, i8)>) -> bool {
    // lines: (x, y, facing, sticky)
    let mut lines: Vec<(i8, i8, u8, bool)> = Vec::new();
    let ps: Vec<(i8, i8, u8, bool)> = v.iter().filter_map(|p| if let K::Piston { sticky, f, .. } = p.k { Some((p.x, p.y, f, sticky)) } else { None }).collect();
    for &(x, y, f, s) in &ps {
        lines.push((x, y, f, s));
        for &(px, py, pf, _) in &ps {
            if (px, py) == (x, y) {
                continue;
            }
            let (dx, dy) = families::F[pf as usize];
            if (1..=13).any(|k| (px + k * dx, py + k * dy) == (x, y)) {
                for k in [-2i8, -1, 1, 2] {
                    lines.push((x + k * dx, y + k * dy, f, s));
                }
            }
        }
    }
    let cells: Vec<(i8, i8)> = frame.iter().copied().collect();
    let mut reach: Vec<Vec<usize>> = Vec::new(); // per door block: indices of reachable frame cells
    for bxx in 0..3i8 {
        for byy in 0..3i8 {
            let mut seen: HashSet<(i8, i8)> = HashSet::new();
            let mut st = vec![(bxx, byy)];
            seen.insert((bxx, byy));
            while let Some(c) = st.pop() {
                for &(x, y, f, s) in &lines {
                    let (dx, dy) = families::F[f as usize];
                    let mut nx = Vec::with_capacity(2);
                    // push: c on the forward line (k >= 1) moves one cell along it
                    let (rx, ry) = (c.0 - x, c.1 - y);
                    let k = if dx != 0 { if ry == 0 && rx * dx > 0 { rx * dx } else { 0 } } else if rx == 0 && ry * dy > 0 { ry * dy } else { 0 };
                    if k >= 1 {
                        nx.push((c.0 + dx, c.1 + dy));
                    }
                    if s && k == 2 {
                        nx.push((x + dx, y + dy));
                    }
                    for n in nx {
                        if bx.has(n.0, n.1) && seen.insert(n) {
                            st.push(n);
                        }
                    }
                }
            }
            reach.push(cells.iter().enumerate().filter(|(_, c)| seen.contains(c)).map(|(i, _)| i).collect());
        }
    }
    // Kuhn's matching: 9 blocks -> distinct frame cells
    fn aug(b: usize, reach: &[Vec<usize>], used: &mut [bool], owner: &mut [Option<usize>]) -> bool {
        for &c in &reach[b] {
            if used[c] {
                continue;
            }
            used[c] = true;
            if owner[c].is_none() || aug(owner[c].unwrap(), reach, used, owner) {
                owner[c] = Some(b);
                return true;
            }
        }
        false
    }
    let mut owner: Vec<Option<usize>> = vec![None; cells.len()];
    for b in 0..9 {
        let mut used = vec![false; cells.len()];
        if !aug(b, &reach, &mut used, &mut owner) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------- candidates

pub fn parse_piece(tok: &str) -> Result<Pc> {
    let (head, rest) = tok.split_once('(').context("piece")?;
    let (xs, ys) = rest.trim_end_matches(')').split_once(',').context("piece xy")?;
    let (x, y): (i8, i8) = (xs.parse()?, ys.parse()?);
    let k = match head {
        "stone" => K::Stone,
        "obsidian" => K::Obsidian,
        "R" => K::Redstone,
        _ => {
            let mut ch = head.chars();
            let sticky = ch.next() == Some('S');
            let f = match ch.next() {
                Some('E') => 0,
                Some('W') => 1,
                Some('U') => 2,
                _ => 3,
            };
            K::Piston { sticky, f, ext: head.ends_with('+') }
        }
    };
    Ok(Pc { x, y, k })
}

/// Lines "x0 y0 piece piece ..." (families' unresolved/WxH.txt).
pub fn read_list(path: &std::path::Path, w: i8, h: i8) -> Result<Vec<(Bx, Vec<Pc>)>> {
    let text = std::fs::read_to_string(path)?;
    parse_lines(&text, w, h)
}

/// Mechanism lines `x0 y0 pieces...` (one per line).
pub fn parse_lines(text: &str, w: i8, h: i8) -> Result<Vec<(Bx, Vec<Pc>)>> {
    let mut items = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let t: Vec<&str> = line.split_whitespace().collect();
        anyhow::ensure!(t.len() >= 2, "short line");
        let bx = Bx { x0: t[0].parse()?, y0: t[1].parse()?, w, h };
        let v: Vec<Pc> = t[2..].iter().map(|s| parse_piece(s)).collect::<Result<_>>()?;
        items.push((bx, v));
    }
    Ok(items)
}

/// `layouts`: FULL-FRAME finds grouped by layout: key = pistons with extension states dropped,
/// the smaller of the layout and its mirror image; value = (cycles found, the stone-free
/// representative with the fewest extended pistons).
pub fn layouts(paths: &[std::path::PathBuf], w: i8, h: i8) -> Result<BTreeMap<String, (usize, String)>> {
    let mut g: BTreeMap<String, (usize, String, usize)> = BTreeMap::new();
    for p in paths {
        for l in std::fs::read_to_string(p)?.lines() {
            let cols: Vec<&str> = l.split('\t').collect();
            if cols.len() < 2 {
                continue;
            }
            let Ok(items) = parse_lines(cols[1], w, h) else { continue };
            let Some((bx, v)) = items.into_iter().next() else { continue };
            let strip = |v: &[Pc]| -> Vec<Pc> {
                let mut s: Vec<Pc> = v.iter().map(|p| match p.k {
                    K::Piston { sticky, f, .. } => Pc { x: p.x, y: p.y, k: K::Piston { sticky, f, ext: false } },
                    _ => *p,
                }).collect();
                s.sort();
                s
            };
            let a = strip(&v);
            let b = strip(&v.iter().map(|p| p.mirror()).collect::<Vec<_>>());
            let key = line_of(&bx, if a <= b { &a } else { &b });
            let next = v.iter().filter(|p| matches!(p.k, K::Piston { ext: true, .. })).count();
            let e = g.entry(key).or_insert((0, cols[1].to_string(), usize::MAX));
            e.0 += 1;
            if next < e.2 {
                e.1 = cols[1].to_string();
                e.2 = next;
            }
        }
    }
    Ok(g.into_iter().map(|(k, (n, r, _))| (k, (n, r))).collect())
}

/// `gencheck`: is `v` (its pistons) a generated candidate (mirror included), and how many
/// candidates of the same box contain it?
pub fn gen_member(cands: &[(Bx, Vec<Pc>)], bx: &Bx, v: &[Pc]) -> (bool, usize) {
    let pist: Vec<Pc> = v.iter().copied().filter(|p| matches!(p.k, K::Piston { .. })).collect();
    let c = families::canon(&pist);
    let mir: Vec<Pc> = pist.iter().map(|p| p.mirror()).collect();
    let exact = cands.iter().any(|(_, w)| families::canon(w) == c);
    let sup = cands.iter().filter(|(b, w)| b == bx && (pist.iter().all(|p| w.contains(p)) || mir.iter().all(|p| w.contains(p)))).count();
    (exact, sup)
}

/// The reach-matching condition alone (see `reach_ok`).
pub fn reach_test(bx: &Bx, v: &[Pc]) -> bool {
    let frame: HashSet<(i8, i8)> = families::ring().into_iter().collect();
    reach_ok(bx, v, &frame)
}

/// Every retracted / extended-at-rest combination of a layout's pistons (a head needs a free cell
/// in the box outside the 3x3), as `x0 y0 pieces` lines; at most `max` of them.
pub fn ext_variants(bx: &Bx, v: &[Pc], max: usize) -> Vec<Vec<Pc>> {
    let base: Vec<Pc> = v.iter().map(|p| match p.k {
        K::Piston { sticky, f, .. } => Pc { x: p.x, y: p.y, k: K::Piston { sticky, f, ext: false } },
        _ => *p,
    }).collect();
    let pist: Vec<usize> = (0..base.len()).filter(|&i| matches!(base[i].k, K::Piston { .. })).collect();
    let mut out = Vec::new();
    for mask in 0u32..(1u32 << pist.len().min(20)) {
        let mut w = base.clone();
        let mut ok = true;
        let mut occ: HashSet<(i8, i8)> = w.iter().map(|p| (p.x, p.y)).collect();
        for (j, &i) in pist.iter().enumerate() {
            if mask & (1 << j) == 0 {
                continue;
            }
            let K::Piston { sticky, f, .. } = w[i].k else { continue };
            let (dx, dy) = families::F[f as usize];
            let h = (w[i].x + dx, w[i].y + dy);
            if !bx.has(h.0, h.1) || families::in_t(h.0, h.1) || !occ.insert(h) {
                ok = false;
                break;
            }
            w[i].k = K::Piston { sticky, f, ext: true };
        }
        if ok && Mech::new(*bx, &w).is_some() {
            w.sort();
            out.push(w);
            if out.len() >= max {
                break;
            }
        }
    }
    out
}

/// `reachtest`: tally (verdict, passes) over a partial TSV (`line \t verdict ...`).
pub fn reach_tally(path: &std::path::Path, w: i8, h: i8) -> Result<BTreeMap<(String, bool), usize>> {
    let mut tally: BTreeMap<(String, bool), usize> = BTreeMap::new();
    for l in std::fs::read_to_string(path)?.lines() {
        let mut it = l.split('\t');
        let (Some(line), Some(vd)) = (it.next(), it.next()) else { continue };
        let Ok(items) = parse_lines(line, w, h) else { continue };
        for (bx, v) in items {
            let ok = reach_test(&bx, &v);
            *tally.entry((vd.to_string(), ok)).or_default() += 1;
        }
    }
    Ok(tally)
}

/// `--door-offset dx,dy`: only this door offset in the box (columns left of / rows below the door).
pub static DOOR_OFFSET: std::sync::OnceLock<(i8, i8)> = std::sync::OnceLock::new();

/// families' candidate list of a box (the same enumeration and mirror dedup as `families::run`).
pub fn candidates(w: i8, h: i8, pmax: usize, two: usize, carry: usize, vmid: &[Vec<(i8, i8)>]) -> Vec<(Bx, Vec<Pc>)> {
    let mut cands = Vec::new();
    let mut seen: HashSet<Vec<Pc>> = HashSet::new();
    for bx in families::offsets(w, h) {
        let dx = -bx.x0;
        if let Some(&(ox, oy)) = DOOR_OFFSET.get() {
            if -bx.x0 != ox || -bx.y0 != oy {
                continue;
            }
        } else if dx > (w - 3) - dx {
            continue;
        }
        for v in families::generate_v(bx, pmax, true, true, two, carry, vmid) {
            if seen.insert(families::canon(&v)) {
                cands.push((bx, v));
            }
        }
    }
    cands
}

pub fn line_of(bx: &Bx, v: &[Pc]) -> String {
    format!("{} {} {}", bx.x0, bx.y0, v.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "))
}

/// Greedy minimal mechanism (exact decisions): drop pieces while it still opens and closes.
pub fn minimal(bx: Bx, v: &[Pc], cap: usize) -> Vec<Pc> {
    let mut cur = v.to_vec();
    loop {
        let mut dropped = false;
        for i in 0..cur.len() {
            let mut t = cur.clone();
            t.remove(i);
            let Some((m, rest)) = Mech::new(bx, &t) else { continue };
            // a goal-directed search first (fast when it works), then the exact decision
            // the exact decision (fast in the wirable sub-model); families' A* only when the
            // exact search is capped
            let cx = Cx::new(&m, &rest);
            let works = match decide_all(&cx, cap.min(500_000), cap, 0).0 {
                Verdict::Found { .. } => true,
                Verdict::Capped { .. } => matches!(families::solve_pulse_astar(&m, &rest, 300_000), Ok(Some(s)) if replay_ok(&m, &rest, &s.open, &s.close)),
                _ => false,
            };
            if works {
                cur = t;
                dropped = true;
                break;
            }
        }
        if !dropped {
            return cur;
        }
    }
}

// ---------------------------------------------------------------- self test

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn same(a: &Out, b: &Out) -> bool {
    match (a, b) {
        (Out::Ok(x), Out::Ok(y)) => x == y,
        (Out::Noop, Out::Noop) | (Out::Illegal, Out::Illegal) => true,
        _ => false,
    }
}

/// Differential test of the tracked transition against `Mech::apply`, and of the footprint
/// independence claim (non-conflicting events commute and never enable / disable each other),
/// on random walks over a box's candidates.
pub fn selftest(w: i8, h: i8, pmax: usize, n_cands: usize, walk: usize, threads: usize) -> Result<()> {
    let cands = candidates(w, h, pmax, 1, 0, &[]);
    let step = (cands.len() / n_cands.max(1)).max(1);
    let picked: Vec<&(Bx, Vec<Pc>)> = cands.iter().step_by(step).take(n_cands).collect();
    let (applies, pairs, bad_apply, bad_pair) = (AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0));
    let next = AtomicUsize::new(0);
    let msgs: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= picked.len() {
                    break;
                }
                let (bx, v) = picked[i];
                let Some((m, rest)) = Mech::new(*bx, v) else { continue };
                let evs = m.events_pulse();
                let mut rng = Rng(0x9E3779B97F4A7C15 ^ (i as u64 + 1));
                let mut s = rest.clone();
                let none = Fp::default();
                for _ in 0..walk {
                    if packable(&m) && unpack(&m, &pack(&m, &s)) != s {
                        bad_apply.fetch_add(1, Ordering::Relaxed);
                        msgs.lock().unwrap().push(format!("pack roundtrip mismatch {}\n{}", line_of(bx, v), m.render(&s)));
                    }
                    let steps: Vec<Step> = evs.iter().map(|&e| step_ev(&m, &s, e, &none)).collect();
                    for (k, &e) in evs.iter().enumerate() {
                        let mut t = Fp::default();
                        let a = apply_t(&m, &s, e, &mut t);
                        let b = m.apply(&s, e);
                        applies.fetch_add(1, Ordering::Relaxed);
                        if !same(&a, &b) {
                            bad_apply.fetch_add(1, Ordering::Relaxed);
                            msgs.lock().unwrap().push(format!("apply mismatch {} {}", line_of(bx, v), m.fmt_ev(e)));
                        }
                        let _ = k;
                    }
                    for a in 0..evs.len() {
                        for b in 0..evs.len() {
                            if a == b || conflict(&steps[a], &steps[b]) {
                                continue;
                            }
                            pairs.fetch_add(1, Ordering::Relaxed);
                            // a after b behaves as a at s; b after a as b at s; same final state
                            let ok = match (&steps[a].next, &steps[b].next) {
                                (Some(sa), Some(sb)) => {
                                    let ab = step_ev(&m, sa, evs[b], &none).next;
                                    let ba = step_ev(&m, sb, evs[a], &none).next;
                                    matches!((ab, ba), (Some(x), Some(y)) if x == y)
                                }
                                (Some(_), None) => step_ev(&m, steps[a].next.as_ref().unwrap(), evs[b], &none).next.is_none(),
                                (None, Some(sb)) => step_ev(&m, sb, evs[a], &none).next.is_none(),
                                (None, None) => true,
                            };
                            if !ok {
                                bad_pair.fetch_add(1, Ordering::Relaxed);
                                let mut ms = msgs.lock().unwrap();
                                if ms.len() < 20 {
                                    ms.push(format!("independence violated: {} | {} vs {}\n{}", line_of(bx, v), m.fmt_ev(evs[a]), m.fmt_ev(evs[b]), m.render(&s)));
                                }
                            }
                        }
                    }
                    let en: Vec<usize> = (0..evs.len()).filter(|&k| steps[k].next.is_some()).collect();
                    if en.is_empty() {
                        s = rest.clone();
                        continue;
                    }
                    s = steps[en[rng.below(en.len())]].next.clone().unwrap();
                    if rng.below(40) == 0 {
                        s = rest.clone();
                    }
                }
            });
        }
    });
    for mm in msgs.into_inner().unwrap().iter().take(20) {
        println!("{mm}");
    }
    println!(
        "selftest {w}x{h}: {} candidates, {} applies ({} mismatches), {} independent pairs ({} violations)",
        picked.len(),
        applies.load(Ordering::Relaxed),
        bad_apply.load(Ordering::Relaxed),
        pairs.load(Ordering::Relaxed),
        bad_pair.load(Ordering::Relaxed)
    );
    Ok(())
}

// ---------------------------------------------------------------- drivers

pub struct RunOpts {
    pub cap: usize,
    pub fcap: usize,
    pub threads: usize,
    pub cap_local: usize,
    pub max_group: usize,
    pub out: std::path::PathBuf,
    pub tag: String,
    /// After a capped exact search, families' goal-directed A* (pulse model) with this cap
    /// looks for a solution (0 = off).
    pub astar: usize,
    /// Lean second stage (visited sets only) up to this many states (0 = off).
    pub cap_lean: usize,
}

/// The exact decision, then (when capped) a goal-directed A* that can still FIND a solution.
pub fn decide_fast(cx: &Cx, cap: usize, astar: usize) -> (Verdict, Stats) {
    decide_all(cx, cap, 0, astar)
}

/// Given the complete reachable set `visited` (in the system restricted by `forbid`) and its open
/// states with no other bad hole than `forbid`'s cells: the open states that can come back to
/// rest. Some -> Found (sequences from A* or from reduced path searches); None -> none closes.
fn cycle_in(cx: &Cx, opens: &[Key], visited: &KeySet, forbid: &Fp, astar: usize) -> Option<Verdict> {
    let m = cx.m;
    let live = coreach_within(m, &cx.rest, visited);
    let lo: Vec<St> = opens.iter().filter(|k| live.contains(*k)).map(|k| unpack(m, k)).filter(|o| bad_holes(m, o) == *forbid).collect();
    let o = lo.first()?;
    if forbid.is_empty() && astar > 0 {
        if let Ok(Some(sol)) = families::solve_pulse_astar(m, &cx.rest, astar) {
            if replay_ok(m, &cx.rest, &sol.open, &sol.close) {
                return Some(Verdict::Found { open: sol.open, close: sol.close });
            }
        }
    }
    // exact sequences: rest -> o and o -> rest by reduced path searches
    let cap = visited.len() + 10;
    let g1 = search(cx, std::slice::from_ref(&cx.rest), &Goal::Exact(o.clone()), forbid, cap, false);
    let g2 = search(cx, std::slice::from_ref(o), &Goal::Exact(cx.rest.clone()), forbid, cap, false);
    if let (Some(&a), Some(&b)) = (g1.goals.first(), g2.goals.first()) {
        let ev = |v: Vec<u8>| -> Vec<E> { v.into_iter().map(|k| cx.evs[k as usize]).collect() };
        let open = ev(g1.path(a).1);
        let close = ev(g2.path(b).1);
        if replay_ok(m, &cx.rest, &open, &close) {
            return Some(Verdict::Found { open, close });
        }
    }
    Some(Verdict::Found { open: Vec::new(), close: Vec::new() })
}

/// Weighted A* in the model in force (sub-model filters included): from `start` to a state
/// meeting `goal`; up to `want` goal states with their paths. (found, capped)
fn astar_sub(cx: &Cx, start: &St, goal: &dyn Fn(&St) -> bool, h: &dyn Fn(&St) -> u32, cap: usize, want: usize, forbid: &Fp) -> (Vec<(St, Vec<E>)>, bool) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let m = cx.m;
    let mut keys: Vec<Key> = vec![pack(m, start)];
    let mut parent: Vec<(u32, u8)> = vec![(u32::MAX, 0)];
    let mut gs: Vec<u32> = vec![0];
    let mut closed: Vec<bool> = vec![false];
    let mut index: KeyMap = KeyMap::default();
    index.insert(keys[0], 0);
    let mut heap = BinaryHeap::new();
    heap.push(Reverse((2 * h(start), 0u32, 0u32)));
    let mut out = Vec::new();
    let mut capped = false;
    while let Some(Reverse((_, g, si))) = heap.pop() {
        if closed[si as usize] || g > gs[si as usize] {
            continue;
        }
        closed[si as usize] = true;
        let s = unpack(m, &keys[si as usize]);
        if goal(&s) {
            let mut path = Vec::new();
            let mut c = si;
            while parent[c as usize].0 != u32::MAX {
                path.push(cx.evs[parent[c as usize].1 as usize]);
                c = parent[c as usize].0;
            }
            path.reverse();
            out.push((s, path));
            if out.len() >= want {
                return (out, false);
            }
            continue;
        }
        for (k, &e) in cx.evs.iter().enumerate() {
            let Out::Ok(n) = m.apply(&s, e) else { continue };
            if n == s || body_in_door(m, &n) || (!forbid.is_empty() && forbid.ones().any(|i| i < MAXC && n.g[i] != AIR)) {
                continue;
            }
            let kn = pack(m, &n);
            let ng = g + 1;
            match index.get(&kn) {
                Some(&j) => {
                    if ng < gs[j as usize] && !closed[j as usize] {
                        gs[j as usize] = ng;
                        parent[j as usize] = (si, k as u8);
                        heap.push(Reverse((ng + 2 * h(&n), ng, j)));
                    }
                }
                None => {
                    if keys.len() >= cap {
                        capped = true;
                        continue;
                    }
                    let j = keys.len() as u32;
                    index.insert(kn, j);
                    keys.push(kn);
                    parent.push((si, k as u8));
                    gs.push(ng);
                    closed.push(false);
                    heap.push(Reverse((ng + 2 * h(&n), ng, j)));
                }
            }
        }
    }
    (out, capped)
}

/// Goal-directed search for an open + close cycle in the model in force: A* to up to 8 open
/// states (h = door cells still filled, the center counting 2), then A* back to rest from each
/// (h = cells differing from rest). Some(open, close) when a replay-checked cycle is found.
pub fn find_cycle(cx: &Cx, cap: usize) -> Option<(Vec<E>, Vec<E>)> {
    let m = cx.m;
    let bx = m.bx;
    let ci = bx.idx(1, 1);
    let tcells: Vec<usize> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).filter(|&(x, y)| bx.has(x, y)).map(|(x, y)| bx.idx(x, y)).collect();
    let h_open = |s: &St| -> u32 { tcells.iter().map(|&i| if s.g[i] == AIR { 0 } else if i == ci { 2 } else { 1 }).sum() };
    let none = Fp::default();
    let (opens, _) = astar_sub(cx, &cx.rest, &|s| cx.is_goal(&Goal::Open, s), &h_open, cap, 8, &none);
    let rest = cx.rest.clone();
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
    for (o, open) in &opens {
        let mask = bad_holes(m, o);
        // FULL FRAME: the cells kept air must be air on the open path too (static stones go there)
        if FULL_FRAME.load(Ordering::Relaxed) && !mask.is_empty() {
            let mut s = rest.clone();
            let mut ok = mask.ones().all(|i| s.g[i] == AIR);
            for e in open {
                if !ok {
                    break;
                }
                match m.apply(&s, *e) {
                    Out::Ok(nx) => s = nx,
                    _ => ok = false,
                }
                ok = ok && mask.ones().all(|i| s.g[i] == AIR);
            }
            if !ok {
                continue;
            }
        }
        let (back, _) = astar_sub(cx, o, &|s| *s == rest, &h_close, cap, 1, &mask);
        if let Some((_, close)) = back.into_iter().next() {
            if replay_ok(m, &rest, open, &close) {
                return Some((open.clone(), close));
            }
        }
    }
    None
}

/// [`decide2`] (exact, lean second stage up to `cap_lean`), then families' A* when still capped.
pub fn decide_all(cx: &Cx, cap: usize, cap_lean: usize, astar: usize) -> (Verdict, Stats) {
    if SWEEP.load(Ordering::Relaxed) {
        return decide_sweep(cx, cap_lean, astar.max(200_000));
    }
    if let Some(r) = main_path(cx, cap_lean.max(200_000), astar) {
        return r;
    }
    decide_rest(cx, cap, cap_lean, astar)
}

/// Sweep mode (`MECHSPACE_SWEEP`): exact on small reachable sets, a goal-directed cycle search
/// (A* in the model in force) next, the exact enumeration up to `cap_lean` last.
pub static SWEEP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn decide_sweep(cx: &Cx, cap_lean: usize, astar: usize) -> (Verdict, Stats) {
    // a goal-directed reduced search (stubborn sets) proves "no open state" with far fewer states
    if std::env::var("MECHSPACE_POR_NOOPEN").is_ok() {
        let none = Fp::default();
        let (g, complete, n) = search_lean(cx, std::slice::from_ref(&cx.rest), &Goal::Open, &none, cap_lean, false, true);
        if complete && g.is_empty() {
            return (Verdict::NoOpen, Stats { open_states: n, close_states: 0 });
        }
    }
    let first = cap_lean.min(100_000);
    if let Some(r) = main_path(cx, first, 0) {
        return r;
    }
    if let Some((open, close)) = find_cycle(cx, astar) {
        return (Verdict::Found { open, close }, Stats { open_states: 0, close_states: 0 });
    }
    if cap_lean > first {
        if let Some(r) = main_path(cx, cap_lean, 0) {
            return r;
        }
    }
    (Verdict::Capped { opens: 0 }, Stats { open_states: cap_lean, close_states: 0 })
}

/// The exact main path: a plain search (cheap per state) enumerates the whole reachable set R
/// (None when it exceeds `lim`); the states of R that can come back to rest are found BACKWARD
/// from rest (pre-images kept inside R): an open state among them is a cycle (the hole rule in
/// the restricted system when needed).
fn main_path(cx: &Cx, lim: usize, astar: usize) -> Option<(Verdict, Stats)> {
    let none = Fp::default();
    let (opens, complete, visited) = search_lean_set(cx, std::slice::from_ref(&cx.rest), &Goal::Open, &none, lim, true, false);
    if !complete {
        return None;
    }
    let n = visited.len();
    let st = Stats { open_states: n, close_states: 0 };
    if opens.is_empty() {
        return Some((Verdict::NoOpen, st));
    }
    let m = cx.m;
    let nopen = opens.len();
    // open states that can come back to rest at all (unrestricted): every cycle uses one
    let live = coreach_within(m, &cx.rest, &visited);
    drop(visited);
    let mut by_mask: BTreeMap<(u32, Fp), Vec<Key>> = BTreeMap::new();
    for k in opens.iter().filter(|k| live.contains(*k)) {
        let b = bad_holes(m, &unpack(m, k));
        let c: u32 = b.0.iter().map(|w| w.count_ones()).sum();
        by_mask.entry((c, b)).or_default().push(*k);
    }
    drop(live);
    let mut capped = false;
    // fewest cells to keep air first (FULL FRAME: fewest static stones)
    for ((_, mask), list) in &by_mask {
        if mask.is_empty() {
            return Some((found_seq(cx, &unpack(m, &list[0]), &none, n + 10, astar), st));
        }
        if mask.ones().any(|i| cx.rest.g[i] != AIR) {
            continue;
        }
        // the restricted system: those cells air at every state of the cycle
        let (_, c2, v2) = search_lean_set(cx, std::slice::from_ref(&cx.rest), &Goal::Open, mask, lim, true, false);
        if !c2 {
            capped = true;
            continue;
        }
        let live2 = coreach_within(m, &cx.rest, &v2);
        if let Some(k) = list.iter().find(|k| v2.contains(*k) && live2.contains(*k)) {
            return Some((found_seq(cx, &unpack(m, k), mask, v2.len() + 10, 0), st));
        }
    }
    Some((if capped { Verdict::Capped { opens: nopen } } else { Verdict::NoClose { opens: nopen } }, st))
}

/// Event sequences of a cycle through the open state `o` known to exist (in the system
/// restricted by `forbid`): families' A* when nothing is restricted, else reduced path searches.
fn found_seq(cx: &Cx, o: &St, forbid: &Fp, cap: usize, astar: usize) -> Verdict {
    let m = cx.m;
    if forbid.is_empty() && astar > 0 {
        if let Ok(Some(sol)) = families::solve_pulse_astar(m, &cx.rest, astar) {
            if replay_ok(m, &cx.rest, &sol.open, &sol.close) {
                return Verdict::Found { open: sol.open, close: sol.close };
            }
        }
    }
    let g1 = search(cx, std::slice::from_ref(&cx.rest), &Goal::Exact(o.clone()), forbid, cap, false);
    let g2 = search(cx, std::slice::from_ref(o), &Goal::Exact(cx.rest.clone()), forbid, cap, false);
    if let (Some(&a), Some(&b)) = (g1.goals.first(), g2.goals.first()) {
        let ev = |v: Vec<u8>| -> Vec<E> { v.into_iter().map(|k| cx.evs[k as usize]).collect() };
        let open = ev(g1.path(a).1);
        let close = ev(g2.path(b).1);
        if replay_ok(m, &cx.rest, &open, &close) {
            return Verdict::Found { open, close };
        }
    }
    Verdict::Found { open: Vec::new(), close: Vec::new() }
}

fn decide_rest(cx: &Cx, cap: usize, cap_lean: usize, astar: usize) -> (Verdict, Stats) {
    let (vd, st) = decide2(cx, cap, cap_lean, astar);
    if astar == 0 || vd.decided() {
        return (vd, st);
    }
    if let Ok(Some(sol)) = families::solve_pulse_astar(cx.m, &cx.rest, astar) {
        if replay_ok(cx.m, &cx.rest, &sol.open, &sol.close) {
            return (Verdict::Found { open: sol.open, close: sol.close }, st);
        }
    }
    (vd, st)
}

/// Decide a list of candidates; writes `<out>/<tag>.tsv` (one verdict per candidate) and
/// returns the found ones.
pub fn run_list(items_all: &[(Bx, Vec<Pc>)], o: &RunOpts, families_cmp: bool) -> Result<Vec<(Bx, Vec<Pc>, Vec<E>, Vec<E>)>> {
    std::fs::create_dir_all(&o.out)?;
    // resume: candidates already in <tag>.partial.tsv are skipped (their rows are kept there)
    let ppath = o.out.join(format!("{}.partial.tsv", o.tag));
    let done_before: HashSet<String> = if std::env::var("MECHSPACE_RESUME").is_ok() {
        std::fs::read_to_string(&ppath).unwrap_or_default().lines().filter_map(|l| l.split('\t').next().map(|s| s.to_string())).collect()
    } else {
        HashSet::new()
    };
    // MECHSPACE_SHARD="k/n": only every n-th candidate of the FULL list starting at k (split a
    // sweep across boxes; applied before the resume filter so shards stay consistent)
    let shard: Option<(usize, usize)> = std::env::var("MECHSPACE_SHARD").ok().and_then(|s| s.split_once('/').and_then(|(a, b)| Some((a.parse::<usize>().ok()?, b.parse::<usize>().ok()?)))).filter(|&(_, n)| n > 0);
    let items: Vec<(Bx, Vec<Pc>)> = items_all.iter().enumerate().filter(|(i, _)| shard.is_none_or(|(k, n)| i % n == k)).map(|(_, x)| x).filter(|(b, v)| !done_before.contains(&line_of(b, v))).cloned().collect();
    if !done_before.is_empty() {
        eprintln!("[mechspace {}] resume: {} already decided, {} left", o.tag, items_all.len() - items.len(), items.len());
    }
    let items = &items[..];
    let t0 = std::time::Instant::now();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let rows: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
    // rows also stream to <tag>.partial.tsv as they finish (long runs)
    let partial = Mutex::new(if done_before.is_empty() { std::fs::File::create(&ppath)? } else { std::fs::OpenOptions::new().append(true).open(&ppath)? });
    let frame_out = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(o.out.join(format!("{}.frame.tsv", o.tag)))?);
    let found: Mutex<Vec<(usize, Bx, Vec<Pc>, Vec<E>, Vec<E>)>> = Mutex::new(Vec::new());
    let counts: Mutex<BTreeMap<String, usize>> = Mutex::new(BTreeMap::new());
    std::thread::scope(|sc| {
        for _ in 0..o.threads.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let (bx, v) = &items[i];
                let ti = std::time::Instant::now();
                let full = FULL_FRAME.load(Ordering::Relaxed);
                let row = match Mech::new(*bx, v) {
                    None => "invalid".to_string(),
                    Some(_) if full && !frame_in_box(bx) => "no-frame".to_string(),
                    Some((m, rest)) => {
                        let mut cx = Cx::new(&m, &rest);
                        cx.cap_local = o.cap_local;
                        cx.max_group = o.max_group;
                        let (mut vd, st) = decide_all(&cx, o.cap, o.cap_lean, o.astar);
                        // FULL FRAME: the found cycle must also work with static stones in the
                        // frame cells it leaves air (streamed to <tag>.frame.tsv)
                        if full {
                            if let Verdict::Found { open, .. } = &vd {
                                match augment_frame(*bx, v, open, o.cap, o.cap_lean) {
                                    Some((aug, op, cl)) => {
                                        use std::io::Write;
                                        let (m2, _) = Mech::new(*bx, &aug).unwrap();
                                        let mut f = frame_out.lock().unwrap();
                                        let _ = writeln!(f, "{}\t{}\t{}\t{}+{}\t{}\t{}", line_of(bx, &aug), line_of(bx, v), aug.iter().filter(|p| matches!(p.k, K::Stone)).count(), op.len(), cl.len(), m2.fmt_seq(&op), m2.fmt_seq(&cl));
                                    }
                                    None => vd = Verdict::NoClose { opens: 0 },
                                }
                            }
                        }
                        let fam = if families_cmp {
                            match families::solve(&m, &rest, o.fcap, false) {
                                Ok(Some(_)) => "found",
                                Ok(None) => "none",
                                Err(()) => match families::solve_pulse_astar(&m, &rest, o.fcap) {
                                    Ok(Some(_)) => "found(A*)",
                                    _ => "unresolved",
                                },
                            }
                        } else {
                            "-"
                        };
                        let key = format!("{} / families {fam}", vd.tag());
                        *counts.lock().unwrap().entry(key).or_default() += 1;
                        if let Verdict::Found { open, close } = &vd {
                            found.lock().unwrap().push((i, *bx, v.clone(), open.clone(), close.clone()));
                        }
                        let extra = match &vd {
                            Verdict::Found { open, close } => format!("{}+{}", open.len(), close.len()),
                            Verdict::NoClose { opens } | Verdict::Capped { opens } => format!("opens {opens}"),
                            Verdict::NoOpen => String::new(),
                        };
                        format!("{}\t{fam}\t{extra}\t{}\t{}\t{:.2}", vd.tag(), st.open_states, st.close_states, ti.elapsed().as_secs_f64())
                    }
                };
                {
                    use std::io::Write;
                    let _ = writeln!(partial.lock().unwrap(), "{}\t{row}", line_of(bx, v));
                }
                rows.lock().unwrap().push((i, format!("{}\t{row}", line_of(bx, v))));
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                if d % 100 == 0 || d == items.len() {
                    eprintln!("[mechspace {}] {d}/{} ({:.0}s) {:?}", o.tag, items.len(), t0.elapsed().as_secs_f64(), counts.lock().unwrap());
                }
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort();
    let mut tsv = String::from("candidate\tverdict\tfamilies\tdetail\topen_states\tclose_states\tseconds\n");
    for (_, r) in &rows {
        tsv.push_str(r);
        tsv.push('\n');
    }
    std::fs::write(o.out.join(format!("{}.tsv", o.tag)), tsv)?;
    println!("[mechspace {}] {} candidates in {:.0}s:", o.tag, items.len(), t0.elapsed().as_secs_f64());
    for (k, n) in counts.into_inner().unwrap() {
        println!("  {k}: {n}");
    }
    let mut f = found.into_inner().unwrap();
    f.sort_by_key(|x| x.0);
    Ok(f.into_iter().map(|(_, b, v, op, cl)| (b, v, op, cl)).collect())
}

/// `mechspace diag2`: the FULL forward graph (no reduction, capped) and how much of it can come
/// back to rest (the part a cycle through rest can use).
pub fn diag2(bx: Bx, v: &[Pc], cap: usize) -> Result<()> {
    let (m, rest) = Mech::new(bx, v).context("invalid mechanism")?;
    let evs = m.events_pulse();
    let t0 = std::time::Instant::now();
    let mut index: KeyMap = KeyMap::default();
    let mut keys: Vec<Key> = vec![pack(&m, &rest)];
    index.insert(keys[0], 0);
    let mut rev: Vec<Vec<u32>> = vec![Vec::new()];
    let mut q = VecDeque::from([0u32]);
    let mut complete = true;
    while let Some(i) = q.pop_front() {
        let s = unpack(&m, &keys[i as usize]);
        for &e in &evs {
            let Out::Ok(n) = m.apply(&s, e) else { continue };
            if n == s {
                continue;
            }
            let k = pack(&m, &n);
            let j = match index.get(&k) {
                Some(&j) => j,
                None => {
                    if keys.len() >= cap {
                        complete = false;
                        continue;
                    }
                    let j = keys.len() as u32;
                    index.insert(k, j);
                    keys.push(k);
                    rev.push(Vec::new());
                    q.push_back(j);
                    j
                }
            };
            rev[j as usize].push(i);
        }
    }
    let n = keys.len();
    let mut back = vec![false; n];
    back[0] = true;
    let mut q = VecDeque::from([0u32]);
    while let Some(t) = q.pop_front() {
        for &s in &rev[t as usize] {
            if !back[s as usize] {
                back[s as usize] = true;
                q.push_back(s);
            }
        }
    }
    let nb = back.iter().filter(|b| **b).count();
    if std::env::var("MECHSPACE_SAMPLES").is_ok() {
        let mut rng = Rng(12345);
        let dead: Vec<usize> = (0..n).filter(|&i| !back[i]).collect();
        let live: Vec<usize> = (0..n).filter(|&i| back[i]).collect();
        println!("rest:\n{}", m.render(&rest));
        for (name, list) in [("DEAD (cannot reach rest)", &dead), ("LIVE (can reach rest)", &live)] {
            for _ in 0..5 {
                if list.is_empty() {
                    break;
                }
                let i = list[rng.below(list.len())];
                println!("{name}:\n{}", m.render(&unpack(&m, &keys[i])));
            }
        }
    }
    let opens = (0..n).filter(|&i| m.is_open(&unpack(&m, &keys[i]))).count();
    let opens_b = (0..n).filter(|&i| back[i] && m.is_open(&unpack(&m, &keys[i]))).count();
    println!("full forward: {n} states (complete {complete}), {nb} can reach rest; open states {opens}, of which {opens_b} can reach rest ({:.1}s)", t0.elapsed().as_secs_f64());
    Ok(())
}

/// `mechspace refit`: every catalog mechanism (any box) re-solved in each target box at every door
/// offset and both mirror orientations; reports the ones that fit a target box smaller than the
/// box the catalog files them under, and whether families' candidate list of the target box
/// holds them (or a superset of their pieces).
pub fn refit(catalog: &std::path::Path, targets: &[(i8, i8)], cands: &[(i8, i8, Vec<(Bx, Vec<Pc>)>)], cap: usize, threads: usize) -> Result<()> {
    let text = std::fs::read_to_string(catalog)?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().context("empty catalog")?.split('\t').collect();
    let ip = head.iter().position(|h| *h == "pieces").context("pieces column")?;
    let ib = head.iter().position(|h| *h == "box").context("box column")?;
    let mut items: Vec<(String, (i8, i8), Vec<Pc>)> = Vec::new();
    for l in lines {
        let t: Vec<&str> = l.split('\t').collect();
        if t.len() <= ip {
            continue;
        }
        let v: Vec<Pc> = t[ip].split_whitespace().map(parse_piece).collect::<Result<_>>()?;
        let b = families::parse_boxes(t[ib])?[0];
        items.push((t[0].to_string(), b, v));
    }
    let jobs: Vec<(usize, (i8, i8))> = items.iter().enumerate().flat_map(|(i, (_, b, _))| targets.iter().filter(move |&&(w, h)| (w as i32) * (h as i32) < (b.0 as i32) * (b.1 as i32)).map(move |&t| (i, t))).collect();
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                if k >= jobs.len() {
                    break;
                }
                let (i, (w, h)) = jobs[k];
                let (id, _, v) = &items[i];
                let mut fits = None;
                let mut capped = 0;
                'o: for vv in [v.clone(), v.iter().map(|p| p.mirror()).collect::<Vec<_>>()] {
                    for bx in families::offsets(w, h) {
                        let Some((m, rest)) = Mech::new(bx, &vv) else { continue };
                        let cx = Cx::new(&m, &rest);
                        match decide(&cx, cap).0 {
                            Verdict::Found { open, close } => {
                                fits = Some((bx, vv.clone(), open.len(), close.len()));
                                break 'o;
                            }
                            Verdict::Capped { .. } => capped += 1,
                            _ => {}
                        }
                    }
                }
                let Some((bx, vv, no, nc)) = fits else {
                    if capped > 0 {
                        out.lock().unwrap().push(format!("{id}\t{w}x{h}\tUNDECIDED ({capped} offsets capped)"));
                    }
                    continue;
                };
                // is it (or a superset) among families' candidates of that box?
                let c = families::canon(&vv);
                let mut exact = false;
                let mut sup = 0;
                if let Some((_, _, list)) = cands.iter().find(|(cw, ch, _)| *cw == w && *ch == h) {
                    for (_, cv) in list {
                        let cc = families::canon(cv);
                        if cc == c {
                            exact = true;
                        }
                        let cm: Vec<Pc> = cv.iter().map(|p| p.mirror()).collect();
                        if vv.iter().all(|p| cv.contains(p)) || vv.iter().all(|p| cm.contains(p)) {
                            sup += 1;
                        }
                    }
                }
                out.lock().unwrap().push(format!("{id}\t{w}x{h}\tFITS at door offset {},{} (open {no} + close {nc})\tin {w}x{h} candidates: {}\tsupersets: {sup}\t{}", -bx.x0, -bx.y0, if exact { "yes" } else { "no" }, line_of(&bx, &vv)));
            });
        }
    });
    let mut out = out.into_inner().unwrap();
    out.sort();
    for l in &out {
        println!("{l}");
    }
    println!("{} catalog mechanisms checked against {targets:?}: {} fit a smaller box", items.len(), out.iter().filter(|l| l.contains("FITS")).count());
    Ok(())
}

// ---------------------------------------------------------------- export (catalog-style lists)

/// Rows "id x0 y0 piece piece ..." (an id, then families' candidate format).
pub fn read_ided(path: &std::path::Path, w: i8, h: i8) -> Result<Vec<(String, Bx, Vec<Pc>)>> {
    let text = std::fs::read_to_string(path)?;
    let mut out = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let bx = Bx { x0: t[1].parse()?, y0: t[2].parse()?, w, h };
        let v: Vec<Pc> = t[3..].iter().map(|s| parse_piece(s)).collect::<Result<_>>()?;
        out.push((t[0].to_string(), bx, v));
    }
    Ok(out)
}

/// families' CATALOG.tsv rows of one box as "id x0 y0 pieces" lines.
pub fn catalog_rows(path: &std::path::Path, w: i8, h: i8) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().context("empty catalog")?.split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n).with_context(|| format!("column {n}"));
    let (ib, io, ip) = (col("box")?, col("door_offset")?, col("pieces")?);
    let mut out = Vec::new();
    for l in lines {
        let t: Vec<&str> = l.split('\t').collect();
        if t.len() <= ip || t[ib] != format!("{w}x{h}") {
            continue;
        }
        let (dx, dy) = t[io].split_once(',').context("offset")?;
        out.push(format!("{} -{dx} -{dy} {}", t[0], t[ip]));
    }
    Ok(out)
}

/// Export mechanisms catalog-style: `<out>/mech/<id>.txt` (door + pistons), `<id>-oc` and (when a
/// symmetric train exists) `<id>-sym` ideal plans (design + `.ideal`), each replayed in the
/// simulator with ideal inputs; `<out>/<tag>.tsv` indexes them.
pub fn export(version: &str, items: &[(String, Bx, Vec<Pc>)], out: &std::path::Path, tag: &str, cap: usize, gap: u32, workers: usize, extra: u32) -> Result<()> {
    let mdir = out.join("mech");
    std::fs::create_dir_all(&mdir)?;
    // solve each (families' A*, else the exact procedure), symmetric train, plans
    struct Ex {
        id: String,
        bx: Bx,
        pieces: Vec<Pc>,
        sol: families::Sol,
        plans: Vec<(&'static str, mechsynth::IdealPlan)>,
        note: String,
    }
    let next = AtomicUsize::new(0);
    let exs: Mutex<Vec<(usize, Result<Ex>)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..workers.max(1) * 3 {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= items.len() {
                    break;
                }
                let (id, bx, v) = &items[i];
                let r = (|| -> Result<Ex> {
                    let (m, rest) = Mech::new(*bx, v).context("invalid mechanism")?;
                    let (open, close) = match families::solve(&m, &rest, 300_000, false) {
                        Ok(Some(s)) if replay_ok(&m, &rest, &s.open, &s.close) => (s.open, s.close),
                        _ => match families::solve_pulse_astar(&m, &rest, cap.min(2_000_000)) {
                            Ok(Some(s)) if replay_ok(&m, &rest, &s.open, &s.close) => (s.open, s.close),
                            _ => match decide(&Cx::new(&m, &rest), cap).0 {
                                Verdict::Found { open, close } => (open, close),
                                other => anyhow::bail!("no open+close: {}", other.tag()),
                            },
                        },
                    };
                    let mut cyc = vec![rest.clone()];
                    let mut c = rest.clone();
                    for e in open.iter().chain(close.iter()) {
                        if let Out::Ok(n) = m.apply(&c, *e) {
                            c = n;
                        }
                        cyc.push(c.clone());
                    }
                    let refs: Vec<&St> = cyc.iter().collect();
                    let extent = m.extent(&refs);
                    let mut sol = families::Sol { open, close, sym: None, sym_levels: false, states: 0, extent, levels: false };
                    if let Some((s, lv)) = families::best_sym(&m, &rest, &sol, 200_000, 6) {
                        sol.sym = Some(s);
                        sol.sym_levels = lv;
                    }
                    let (wld, map) = families::to_world(&m);
                    mechsynth::design_of_world(&wld, id).save(&mdir.join(format!("{id}.txt")))?;
                    let mut plans = Vec::new();
                    let mut note = String::new();
                    for (t, msol) in families::ideal_solutions(&m, &rest, &sol, &map) {
                        match mechsynth::ideal_plan(&wld, &msol, false, gap, id) {
                            Ok(plan) => {
                                let mut dd = plan.design.clone();
                                dd.name = format!("{id}-{t}");
                                let dpath = mdir.join(format!("{id}-{t}.txt"));
                                dd.save(&dpath)?;
                                std::fs::write(mdir.join(format!("{id}-{t}.ideal")), plan.text.replace(&format!("design {id}.txt"), &format!("design {}", dpath.display())))?;
                                plans.push((t, plan));
                            }
                            Err(e) => note.push_str(&format!("{t}: no ideal plan ({}); ", format!("{e:#}").replace(['\t', '\n'], " "))),
                        }
                    }
                    Ok(Ex { id: id.clone(), bx: *bx, pieces: v.clone(), sol, plans, note })
                })();
                exs.lock().unwrap().push((i, r));
            });
        }
    });
    let mut exs = exs.into_inner().unwrap();
    exs.sort_by_key(|x| x.0);
    // replay every plan in the simulator
    let jobs: Vec<(usize, usize)> = exs.iter().enumerate().flat_map(|(k, (_, r))| match r {
        Ok(e) => (0..e.plans.len()).map(|p| (k, p)).collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    }).collect();
    let cfg = mctas::sim::WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
    let pool = mctas::sim::Pool::spawn(workers.max(1), &cfg)?;
    let res = pool.map(jobs.clone(), |wk, _, (k, p)| {
        let Ok(e) = &exs[k].1 else { anyhow::bail!("no entry") };
        Ok(mechsynth::verify_plan(wk, &e.plans[p].1, extra).map_err(|e| format!("{e:#}")))
    });
    let mut verdicts: HashMap<usize, Vec<String>> = HashMap::new();
    for ((k, p), r) in jobs.iter().zip(res) {
        let Ok(e) = &exs[*k].1 else { continue };
        let t = e.plans[*p].0;
        let v = match r {
            Ok(Ok(())) => format!("{t} WORKS"),
            Ok(Err(x)) => format!("{t} BROKEN ({})", x.chars().take(90).collect::<String>().replace(['\t', '\n'], " ")),
            Err(x) => format!("{t} ERROR ({})", format!("{x:#}").chars().take(90).collect::<String>().replace(['\t', '\n'], " ")),
        };
        verdicts.entry(*k).or_default().push(v);
    }
    let mut tsv = String::from("id\tbox\tdoor_offset\tpistons\topen_len\tclose_len\tsym_len\textent\tpieces\topen\tclose\tsym\tsim\tfiles\tframe\n");
    let (mut ok, mut bad) = (0, 0);
    for (k, (i, r)) in exs.iter().enumerate() {
        match r {
            Ok(e) => {
                let (m, mrest) = Mech::new(e.bx, &e.pieces).unwrap();
                // sequences as families' catalog publishes them: the QC coupling spelled out
                let xs = families::expanded_sol(&m, &mrest, &e.sol);
                let sim = verdicts.get(&k).map(|v| v.join("; ")).unwrap_or_default();
                if sim.contains("WORKS") {
                    ok += 1;
                } else {
                    bad += 1;
                }
                // frame cells filled when open (door blocks + plain blocks), out of 12
                let mut o = mrest.clone();
                for ev in &e.sol.open {
                    if let Out::Ok(n) = m.apply(&o, *ev) {
                        o = n;
                    }
                }
                let frame = frame_count(&m, &o);
                let files: Vec<String> = std::iter::once(format!("mech/{}.txt", e.id)).chain(e.plans.iter().map(|(t, _)| format!("mech/{}-{t}.ideal", e.id))).collect();
                let ex = e.sol.extent;
                tsv.push_str(&format!(
                    "{}\t{}x{}\t{},{}\t{}\t{}\t{}\t{}\t{}x{}\t{}\t{}\t{}\t{}\t{}{}\t{}\t{frame}/12\n",
                    e.id,
                    e.bx.w,
                    e.bx.h,
                    -e.bx.x0,
                    -e.bx.y0,
                    m.info.len(),
                    e.sol.open.len(),
                    e.sol.close.len(),
                    e.sol.sym.as_ref().map_or(0, |s| s.len()),
                    ex.2 - ex.0 + 1,
                    ex.3 - ex.1 + 1,
                    e.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "),
                    m.fmt_seq(&xs.open),
                    m.fmt_seq(&xs.close),
                    xs.sym.as_ref().map(|s| m.fmt_seq(s)).unwrap_or_default(),
                    e.note,
                    sim,
                    files.join(" ")
                ));
            }
            Err(err) => {
                bad += 1;
                tsv.push_str(&format!("{}\t\t\t\t\t\t\t\t\t\t\t\tERROR {err:#}\t\n", items[*i].0));
            }
        }
    }
    std::fs::write(out.join(format!("{tag}.tsv")), tsv)?;
    println!("[mechspace export {tag}] {} mechanisms: {ok} with a plan that WORKS in the simulator, {bad} without", items.len());
    Ok(())
}

/// Helper variants of each candidate: one (or two) plain blocks (stone = pushable filler,
/// obsidian = immovable stopper) added at every free cell of the box outside the 3x3.
pub fn helper_variants(items: &[(Bx, Vec<Pc>)], count: usize) -> Vec<(Bx, Vec<Pc>)> {
    let mut out = Vec::new();
    let mut seen: HashSet<Vec<Pc>> = HashSet::new();
    for (bx, v) in items {
        let occ: HashSet<(i8, i8)> = v
            .iter()
            .flat_map(|p| match p.k {
                K::Piston { ext: true, f, .. } => vec![(p.x, p.y), (p.x + families::F[f as usize].0, p.y + families::F[f as usize].1)],
                _ => vec![(p.x, p.y)],
            })
            .collect();
        let free: Vec<(i8, i8)> = (0..bx.n()).map(|i| bx.xy(i)).filter(|&(x, y)| !families::in_t(x, y) && !occ.contains(&(x, y))).collect();
        let kinds = [K::Stone, K::Obsidian];
        for (ai, &a) in free.iter().enumerate() {
            for ka in kinds {
                let mut w = v.clone();
                w.push(Pc { x: a.0, y: a.1, k: ka });
                w.sort();
                if count >= 2 {
                    for &b in &free[ai + 1..] {
                        for kb in kinds {
                            let mut w2 = w.clone();
                            w2.push(Pc { x: b.0, y: b.1, k: kb });
                            w2.sort();
                            if seen.insert(w2.clone()) {
                                out.push((*bx, w2));
                            }
                        }
                    }
                }
                if seen.insert(w.clone()) {
                    out.push((*bx, w));
                }
            }
        }
    }
    out
}

/// The canonical piece lists of every mechanism of families' CATALOG.tsv.
pub fn catalog_set(path: &std::path::Path) -> Result<HashMap<Vec<Pc>, String>> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().context("empty catalog")?.split('\t').collect();
    let ip = head.iter().position(|h| *h == "pieces").context("pieces column")?;
    let mut out = HashMap::new();
    for l in lines {
        let t: Vec<&str> = l.split('\t').collect();
        if t.len() <= ip {
            continue;
        }
        let v: Vec<Pc> = t[ip].split_whitespace().map(parse_piece).collect::<Result<_>>()?;
        out.insert(families::canon(&v), t[0].to_string());
    }
    Ok(out)
}

/// For each found candidate: its minimal mechanism (exact decisions), canonical form, and
/// whether families' catalog already lists it. Writes `<out>/<tag>-found.tsv`; returns the NEW
/// minimal mechanisms (box, oriented pieces, open, close).
pub fn handle_found(found: &[(Bx, Vec<Pc>, Vec<E>, Vec<E>)], o: &RunOpts, catalog: &HashMap<Vec<Pc>, String>) -> Result<Vec<(Bx, Vec<Pc>, Vec<E>, Vec<E>)>> {
    let next = AtomicUsize::new(0);
    let res: Mutex<Vec<(usize, Bx, Vec<Pc>, Vec<Pc>)>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..o.threads.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= found.len() {
                    break;
                }
                let (bx, v, _, _) = &found[i];
                let mv = minimal(*bx, v, o.cap);
                res.lock().unwrap().push((i, *bx, v.clone(), mv));
            });
        }
    });
    let mut res = res.into_inner().unwrap();
    res.sort_by_key(|x| x.0);
    let mut tsv = String::from("candidate\tminimal\tcatalog\topen\tclose\n");
    let mut new: BTreeMap<Vec<Pc>, (Bx, Vec<Pc>, Vec<E>, Vec<E>)> = BTreeMap::new();
    for (i, bx, v, mv) in &res {
        let c = families::canon(mv);
        let cat = catalog.get(&c).cloned().unwrap_or_else(|| "NEW".into());
        let (m, rest) = Mech::new(*bx, mv).context("minimal rebuild")?;
        let cx = Cx::new(&m, &rest);
        let (op, cl) = match decide_all(&cx, o.cap.min(500_000), o.cap_lean.max(o.cap), 0).0 {
            Verdict::Found { open, close } if !open.is_empty() => (open, close),
            _ => match families::solve_pulse_astar(&m, &rest, 2_000_000) {
                Ok(Some(s)) if replay_ok(&m, &rest, &s.open, &s.close) => (s.open, s.close),
                _ => (Vec::new(), Vec::new()),
            },
        };
        tsv.push_str(&format!("{}\t{}\t{cat}\t{}\t{}\n", line_of(bx, v), line_of(bx, mv), m.fmt_seq(&op), m.fmt_seq(&cl)));
        if cat == "NEW" && !new.contains_key(&c) {
            new.insert(c, (*bx, mv.clone(), op, cl));
        }
        let _ = i;
    }
    std::fs::write(o.out.join(format!("{}-found.tsv", o.tag)), tsv)?;
    println!("[mechspace {}] {} found candidates -> {} distinct NEW minimal mechanisms (not in the catalog)", o.tag, found.len(), new.len());
    for (bx, v, op, cl) in new.values() {
        println!("  NEW {}  (open {} + close {})", line_of(bx, v), op.len(), cl.len());
    }
    Ok(new.into_values().collect())
}
