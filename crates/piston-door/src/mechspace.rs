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
fn step_ev(m: &Mech, s: &St, e: E, forbid: &Fp) -> Step {
    let mut r = Fp::default();
    match apply_t(m, s, e, &mut r) {
        Out::Ok(n) if n != *s => {
            if !forbid.is_empty() && forbid.ones().any(|i| i < MAXC && n.g[i] != AIR) {
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
                    true
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
            Goal::Open => self.m.is_open(s),
            Goal::Exact(r) => s == r,
        }
    }
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
        let mut seen: HashSet<Key> = HashSet::new();
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
    let mut index: HashMap<Key, u32> = HashMap::new();
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

fn bad_holes(m: &Mech, o: &St) -> Fp {
    let mut b = Fp::default();
    let bx = m.bx;
    for (x, y) in families::ring() {
        if !bx.has(x, y) || o.g[bx.idx(x, y)] != AIR {
            continue;
        }
        let (ox, oy) = families::outward(x, y);
        if bx.has(ox, oy) && !matches!(o.g[bx.idx(ox, oy)], AIR | DOOR) {
            b.set(bx.idx(x, y));
        }
    }
    b
}

/// Lean search (no paths): the visited set only, goals returned as keys. `por` = the stubborn-set
/// reduction (else every enabled event, through `Mech::apply` directly).
pub fn search_lean(cx: &Cx, starts: &[St], goal: &Goal, forbid: &Fp, cap: usize, all_goals: bool, por: bool) -> (Vec<Key>, bool, usize) {
    let m = cx.m;
    let mut seen: HashSet<Key> = HashSet::with_capacity(1 << 16);
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
                return (goals, complete, seen.len());
            }
        }
        let succ: Vec<St> = if por {
            choose(cx, &s, goal, forbid).into_iter().map(|(_, n)| n).collect()
        } else {
            let mut v = Vec::with_capacity(cx.evs.len());
            for &e in &cx.evs {
                if let Out::Ok(n) = m.apply(&s, e) {
                    if n != s && (forbid.is_empty() || !forbid.ones().any(|i| i < MAXC && n.g[i] != AIR)) {
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
    (goals, complete, seen.len())
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
        let r = search_lean(cx, std::slice::from_ref(&rest), &Goal::Open, &none, cap_lean, true, true);
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
            let r = search_lean(cx, &starts, &goal, mask, cap_lean, false, true);
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
    c == *rest && m.holes_ok(&refs, &o)
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
pub fn coreach(m: &Mech, rest: &St, cap: usize) -> (HashSet<Key>, bool) {
    let evs = m.events_pulse();
    let mut seen: HashSet<Key> = HashSet::new();
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
    let mut items = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let t: Vec<&str> = line.split_whitespace().collect();
        let bx = Bx { x0: t[0].parse()?, y0: t[1].parse()?, w, h };
        let v: Vec<Pc> = t[2..].iter().map(|s| parse_piece(s)).collect::<Result<_>>()?;
        items.push((bx, v));
    }
    Ok(items)
}

/// families' candidate list of a box (the same enumeration and mirror dedup as `families::run`).
pub fn candidates(w: i8, h: i8, pmax: usize, two: usize, carry: usize, vmid: &[Vec<(i8, i8)>]) -> Vec<(Bx, Vec<Pc>)> {
    let mut cands = Vec::new();
    let mut seen: HashSet<Vec<Pc>> = HashSet::new();
    for bx in families::offsets(w, h) {
        let dx = -bx.x0;
        if dx > (w - 3) - dx {
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
            let works = match families::solve_pulse_astar(&m, &rest, 300_000) {
                Ok(Some(s)) if replay_ok(&m, &rest, &s.open, &s.close) => true,
                _ => matches!(decide(&Cx::new(&m, &rest), cap).0, Verdict::Found { .. }),
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

/// [`decide2`] (exact, lean second stage up to `cap_lean`), then families' A* when still capped.
pub fn decide_all(cx: &Cx, cap: usize, cap_lean: usize, astar: usize) -> (Verdict, Stats) {
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
pub fn run_list(items: &[(Bx, Vec<Pc>)], o: &RunOpts, families_cmp: bool) -> Result<Vec<(Bx, Vec<Pc>, Vec<E>, Vec<E>)>> {
    std::fs::create_dir_all(&o.out)?;
    let t0 = std::time::Instant::now();
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let rows: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
    // rows also stream to <tag>.partial.tsv as they finish (long runs)
    let partial = Mutex::new(std::fs::File::create(o.out.join(format!("{}.partial.tsv", o.tag)))?);
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
                let row = match Mech::new(*bx, v) {
                    None => "invalid".to_string(),
                    Some((m, rest)) => {
                        let mut cx = Cx::new(&m, &rest);
                        cx.cap_local = o.cap_local;
                        cx.max_group = o.max_group;
                        let (vd, st) = decide_all(&cx, o.cap, o.cap_lean, o.astar);
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
    let mut index: HashMap<Key, u32> = HashMap::new();
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
    let mut tsv = String::from("id\tbox\tdoor_offset\tpistons\topen_len\tclose_len\tsym_len\textent\tpieces\topen\tclose\tsym\tsim\tfiles\n");
    let (mut ok, mut bad) = (0, 0);
    for (k, (i, r)) in exs.iter().enumerate() {
        match r {
            Ok(e) => {
                let (m, _) = Mech::new(e.bx, &e.pieces).unwrap();
                let sim = verdicts.get(&k).map(|v| v.join("; ")).unwrap_or_default();
                if sim.contains("WORKS") {
                    ok += 1;
                } else {
                    bad += 1;
                }
                let files: Vec<String> = std::iter::once(format!("mech/{}.txt", e.id)).chain(e.plans.iter().map(|(t, _)| format!("mech/{}-{t}.ideal", e.id))).collect();
                let ex = e.sol.extent;
                tsv.push_str(&format!(
                    "{}\t{}x{}\t{},{}\t{}\t{}\t{}\t{}\t{}x{}\t{}\t{}\t{}\t{}\t{}{}\t{}\n",
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
                    m.fmt_seq(&e.sol.open),
                    m.fmt_seq(&e.sol.close),
                    e.sol.sym.as_ref().map(|s| m.fmt_seq(s)).unwrap_or_default(),
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
        let (op, cl) = match families::solve_pulse_astar(&m, &rest, 2_000_000) {
            Ok(Some(s)) if replay_ok(&m, &rest, &s.open, &s.close) => (s.open, s.close),
            _ => match decide_all(&cx, o.cap, o.cap_lean, o.astar).0 {
                Verdict::Found { open, close } => (open, close),
                _ => anyhow::bail!("minimal mechanism lost its solution: {}", line_of(bx, mv)),
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
