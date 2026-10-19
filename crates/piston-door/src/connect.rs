//! `mctas door connect`: can a flat mechanism be wired inside a box AT ALL, counting only what can
//! carry a signal from one wiring region to another (track wire64b, 2026-10-19)? A necessary
//! condition, checked before any wiring is attempted; parity and room for mergers are ignored.
//!
//! Model (door plane z = 0, box `lo..=hi`):
//! * The mechanism's FOOTPRINT is every cell a block, a head or a pulse head touches along the
//!   catalog's run (symmetric train on both edges, else the open then the close path). FREE cells =
//!   box cells outside the opening, the ring and the footprint; HOP cells = unswept ring cells (they
//!   hold plain stone: a component on one side powers it, it powers what touches it). REGIONS =
//!   components of free cells, merged through hops.
//! * A piston at its current cell is FED from a region when a cell next to it (not its front), or
//!   next to its quasi-connectivity cell (the cell above), is a free cell of that region, a hop next
//!   to that region, or a DOOR BLOCK (at that moment) next to a free cell of that region (the region
//!   strongly powers the door block, the door block powers the piston: conduction). A piston whose
//!   only quasi feed goes through another piston's cell fires together with it (a stack event).
//! * The lever is in one region (outside the box on a boundary cell, or inside on a free cell). Each
//!   edge starts with only the lever's region AWAKE. An event (a 2 gt pulse or a level toggle of one
//!   piston, the macro model of `door mech-synth`) may happen only if its piston is fed from an awake
//!   region. After it, every region with a free cell next to a cell that changed (blocks, heads, the
//!   piston's own cell, a pulse's head cell) wakes: an observer there sees the change. A door block
//!   that touches free cells of an awake region and of another region wakes the other one too
//!   (powered through the door block).
//! * Search: every event ORDER (BFS over macro states x awake sets, never entering a free cell): the
//!   open edge must reach a FULL-seamless open state from which the close edge (only the lever's
//!   region awake again) reaches the exact rest state.
//! A mechanism that fails cannot be wired with in-plane redstone under these links (observer
//! watching, conduction through plain and door blocks, quasi-connectivity); one that passes may
//! still fail on parity (mergers, flip-flops) and room.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::atomic::Ordering;

use crate::mechsynth::{self, Act, D, Ev, Kind, P, Solution, World};

fn addp(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn in_t(p: P) -> bool {
    (0..=2).contains(&p[0]) && (0..=2).contains(&p[1]) && p[2] == 0
}
fn in_ring(p: P) -> bool {
    p[2] == 0 && (((0..=2).contains(&p[0]) && (p[1] == -1 || p[1] == 3)) || ((0..=2).contains(&p[1]) && (p[0] == -1 || p[0] == 3)))
}
const PLANE: [usize; 4] = [0, 1, 2, 3];
const UP: P = [0, 1, 0];

/// Cells a state occupies: blocks and extended pistons' heads.
fn occupied(w: &World) -> HashMap<P, (usize, bool)> {
    let mut m = HashMap::new();
    for (i, (p, k)) in w.b.iter().enumerate() {
        m.insert(*p, (i, false));
        if let Kind::Piston { ext: true, f, .. } = k {
            m.insert(addp(*p, D[*f as usize]), (i, true));
        }
    }
    m
}

pub struct Layout {
    pub lo: [i8; 2],
    pub hi: [i8; 2],
    /// Free cell -> region id.
    pub comp: HashMap<P, usize>,
    /// Hop cell -> regions next to it.
    pub hops: HashMap<P, Vec<usize>>,
    pub nregions: usize,
    pub footprint: HashSet<P>,
    /// Pistons extended at rest (the only ones that take level toggles).
    pub lv: u64,
    /// The catalog run did not replay to rest in this macro model (footprint from what replayed).
    pub diverged: bool,
}

impl Layout {
    fn in_box(&self, p: P) -> bool {
        p[2] == 0 && (self.lo[0]..=self.hi[0]).contains(&p[0]) && (self.lo[1]..=self.hi[1]).contains(&p[1])
    }
    /// Regions next to a cell (its free neighbors).
    fn regions_around(&self, c: P) -> u64 {
        let mut m = 0u64;
        for d in PLANE {
            if let Some(k) = self.comp.get(&addp(c, D[d])) {
                m |= 1 << k;
            }
        }
        m
    }
    /// Regions that can power cell `c` in state `occ` (a component in `c`'s free cell, a hop, or a
    /// door block powered from a free cell next to it).
    fn sources(&self, c: P, w: &World, occ: &HashMap<P, (usize, bool)>) -> u64 {
        if let Some(k) = self.comp.get(&c) {
            return 1 << k;
        }
        if let Some(ks) = self.hops.get(&c) {
            return ks.iter().fold(0u64, |m, k| m | 1 << k);
        }
        if let Some((i, false)) = occ.get(&c) {
            if matches!(w.b[*i].1, Kind::Door) {
                return self.regions_around(c);
            }
        }
        0
    }
}

/// Footprint of the catalog run and the regions of a box.
pub fn layout(rest: &World, sol: &Solution, lo: [i8; 2], hi: [i8; 2]) -> Result<Layout, String> {
    let seqs: Vec<&Vec<Ev>> = match &sol.sym {
        Some(s) => vec![s, s],
        None => vec![&sol.open, &sol.close],
    };
    let mut fp: HashSet<P> = HashSet::new();
    let mut cur = rest.clone();
    let mark = |w: &World, fp: &mut HashSet<P>| {
        for p in occupied(w).keys() {
            fp.insert(*p);
        }
    };
    mark(&cur, &mut fp);
    for seq in seqs {
        for e in seq {
            let (p, k) = cur.b[e.id as usize];
            if let (Act::P, Kind::Piston { f, .. }) = (e.act, k) {
                fp.insert(addp(p, D[f as usize]));
            }
            if let Some(n) = cur.apply(*e) {
                cur = n;
            }
            mark(&cur, &mut fp);
        }
    }
    // families v3 plans include quasi-connectivity coupling that this macro model does not replay;
    // keep the footprint of what did replay and say so (the order search below is this model's own).
    let diverged = cur != *rest;
    let lv = rest.b.iter().enumerate().filter(|(_, (_, k))| matches!(k, Kind::Piston { ext: true, .. })).fold(0u64, |m, (i, _)| m | 1u64 << i);
    let mut l = Layout { lo, hi, comp: HashMap::new(), hops: HashMap::new(), nregions: 0, footprint: fp, lv, diverged };
    for p in &l.footprint {
        if !in_t(*p) && !l.in_box(*p) {
            return Err(format!("mechanism outside the box at {:?}", p));
        }
    }
    let mut free: Vec<P> = Vec::new();
    let mut hopc: Vec<P> = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            let p = [x, y, 0];
            if in_t(p) || l.footprint.contains(&p) {
                continue;
            }
            if in_ring(p) {
                hopc.push(p);
            } else {
                free.push(p);
            }
        }
    }
    let fset: HashSet<P> = free.iter().copied().collect();
    let mut comp: HashMap<P, usize> = HashMap::new();
    let mut n = 0;
    for &c in &free {
        if comp.contains_key(&c) {
            continue;
        }
        let mut st = vec![c];
        comp.insert(c, n);
        while let Some(p) = st.pop() {
            for d in PLANE {
                let q = addp(p, D[d]);
                if fset.contains(&q) && !comp.contains_key(&q) {
                    comp.insert(q, n);
                    st.push(q);
                }
            }
        }
        n += 1;
    }
    // merge through hops
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while p[r] != r {
            r = p[r];
        }
        let mut j = i;
        while p[j] != r {
            let nx = p[j];
            p[j] = r;
            j = nx;
        }
        r
    }
    for &h in &hopc {
        let ks: Vec<usize> = PLANE.iter().filter_map(|d| comp.get(&addp(h, D[*d])).copied()).collect();
        for w in ks.windows(2) {
            let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            parent[a] = b;
        }
    }
    let mut renum: BTreeMap<usize, usize> = BTreeMap::new();
    for k in 0..n {
        let r = find(&mut parent, k);
        let next = renum.len();
        renum.entry(r).or_insert(next);
    }
    for v in comp.values_mut() {
        let r = find(&mut parent, *v);
        *v = renum[&r];
    }
    l.nregions = renum.len();
    l.comp = comp;
    for &h in &hopc {
        let mut ks: Vec<usize> = PLANE.iter().filter_map(|d| l.comp.get(&addp(h, D[*d])).copied()).collect();
        ks.sort();
        ks.dedup();
        if !ks.is_empty() {
            l.hops.insert(h, ks);
        }
    }
    if l.nregions > 60 {
        return Err("too many regions".into());
    }
    Ok(l)
}

/// One gated event: a piston event, or a stack event (the piston below fired by quasi-connectivity
/// through the cell of the piston above, which the same source powers directly).
#[derive(Clone, Copy, Debug)]
enum GEv {
    One(Ev),
    Stack(Ev, Ev),
}

/// Regions that can fire piston `i` in state `w` directly (direct neighbors, quasi through a cell
/// that holds no piston); and the stack partner when its quasi cell holds a piston.
fn feeds(l: &Layout, w: &World, occ: &HashMap<P, (usize, bool)>, i: usize) -> (u64, Option<usize>) {
    let (pos, k) = w.b[i];
    let Kind::Piston { f, ext, .. } = k else { return (0, None) };
    let front = addp(pos, D[f as usize]);
    let mut m = 0u64;
    for d in PLANE {
        let c = addp(pos, D[d]);
        if c != front && !(ext && c == front) {
            m |= l.sources(c, w, occ);
        }
    }
    let q = addp(pos, UP);
    let partner = match occ.get(&q) {
        Some((j, false)) if matches!(w.b[*j].1, Kind::Piston { .. }) => Some(*j),
        _ => None,
    };
    if partner.is_none() && q != front {
        for d in PLANE {
            let c = addp(q, D[d]);
            if c != pos {
                m |= l.sources(c, w, occ);
            }
        }
    }
    (m, partner)
}

fn changed_regions(l: &Layout, a: &World, b: &World, ev_cells: &[P]) -> u64 {
    let oa = occupied(a);
    let ob = occupied(b);
    let mut cells: HashSet<P> = ev_cells.iter().copied().collect();
    for (p, v) in &oa {
        let same = ob.get(p).is_some_and(|u| u == v && (u.1 || a.b[u.0].1 == b.b[u.0].1));
        if !same {
            cells.insert(*p);
        }
    }
    for p in ob.keys() {
        if !oa.contains_key(p) {
            cells.insert(*p);
        }
    }
    cells.iter().fold(0u64, |m, c| m | l.regions_around(*c))
}

/// Door blocks bridging an awake region to another one.
fn bridge(l: &Layout, w: &World, awake: u64) -> u64 {
    let mut m = awake;
    loop {
        let mut grew = false;
        for (p, k) in &w.b {
            if matches!(k, Kind::Door) && !in_t(*p) {
                let r = l.regions_around(*p);
                if r & m != 0 && r & !m != 0 {
                    m |= r;
                    grew = true;
                }
            }
        }
        if !grew {
            return m;
        }
    }
}

fn legal(l: &Layout, w: &World) -> bool {
    for p in occupied(w).keys() {
        if l.comp.contains_key(p) || (!in_t(*p) && !l.in_box(*p)) {
            return false;
        }
    }
    true
}

/// Pulses on every piston, level toggles only on the pistons extended at rest (`lv`, families'
/// macro model: a level-held piston retracts and comes back once per edge).
fn gated_events(l: &Layout, w: &World, awake: u64, lv: u64) -> Vec<GEv> {
    let occ = occupied(w);
    let mut out = Vec::new();
    for i in w.pistons() {
        let mut evs = vec![Ev { id: i as u8, act: Act::P }];
        if lv & (1u64 << i) != 0 {
            evs.push(Ev { id: i as u8, act: Act::L });
        }
        for e in evs {
            let (m, partner) = feeds(l, w, &occ, i);
            if m & awake != 0 {
                out.push(GEv::One(e));
            }
            if let Some(j) = partner {
                // the source powers j directly and i through j's cell
                let (mj, _) = feeds(l, w, &occ, j);
                if mj & awake != 0 && e.act == Act::P {
                    out.push(GEv::Stack(Ev { id: j as u8, act: Act::P }, e));
                }
            }
        }
    }
    out
}

fn apply_g(w: &World, g: GEv) -> Option<(World, Vec<P>)> {
    let cells_of = |w: &World, e: Ev| -> Vec<P> {
        let (p, k) = w.b[e.id as usize];
        let mut v = vec![p];
        if let Kind::Piston { f, .. } = k {
            v.push(addp(p, D[f as usize]));
        }
        v
    };
    match g {
        GEv::One(e) => {
            let c = cells_of(w, e);
            w.apply(e).map(|n| (n, c))
        }
        GEv::Stack(a, b) => {
            let mut c = cells_of(w, a);
            c.extend(cells_of(w, b));
            let n1 = w.apply(a).unwrap_or_else(|| w.clone());
            // both fire from one source at the same moment: the lower one must still be where its
            // quasi cell is the upper one's cell
            if n1.b[b.id as usize].0 != w.b[b.id as usize].0 {
                return None;
            }
            let n2 = n1.apply(b).unwrap_or_else(|| n1.clone());
            if n2 == *w { None } else { Some((n2, c)) }
        }
    }
}

/// BFS of one edge; returns the reached goal states with one witness order each (event, awake
/// regions before it), and whether the cap was hit. A state is skipped when the same mechanism
/// state was already reached with a superset of awake regions (more awake regions never forbid
/// an event).
#[allow(clippy::type_complexity)]
fn edge(l: &Layout, start: &World, lever: usize, goal: &dyn Fn(&World) -> bool, cap: usize, want_all: bool, fired: &std::cell::RefCell<std::collections::BTreeSet<u8>>) -> (Vec<(World, Vec<(GEv, u64)>)>, bool, u64) {
    let a0 = bridge(l, start, 1u64 << lever);
    let mut seen: HashMap<World, Vec<u64>> = HashMap::new();
    let mut par: HashMap<(World, u64), (World, u64, GEv)> = HashMap::new();
    let mut nseen = 1usize;
    let mut q: VecDeque<(World, u64)> = VecDeque::new();
    seen.insert(start.clone(), vec![a0]);
    q.push_back((start.clone(), a0));
    let mut goals: Vec<(World, Vec<(GEv, u64)>)> = Vec::new();
    let mut goal_set: HashSet<World> = HashSet::new();
    let mut max_awake = a0;
    let mut capped = false;
    let path = |par: &HashMap<(World, u64), (World, u64, GEv)>, mut w: World, mut a: u64| -> Vec<(GEv, u64)> {
        let mut out = Vec::new();
        while let Some((pw, pa, g)) = par.get(&(w.clone(), a)) {
            out.push((*g, *pa));
            w = pw.clone();
            a = *pa;
        }
        out.reverse();
        out
    };
    while let Some((w, awake)) = q.pop_front() {
        // stale entry: a superset arrived since
        if seen.get(&w).is_some_and(|ms| ms.iter().any(|m| *m != awake && m & awake == awake)) {
            continue;
        }
        max_awake |= awake;
        for g in gated_events(l, &w, awake, l.lv) {
            match g {
                GEv::One(e) => fired.borrow_mut().insert(e.id),
                GEv::Stack(a, b) => {
                    fired.borrow_mut().insert(a.id);
                    fired.borrow_mut().insert(b.id)
                }
            };
            let Some((n, cells)) = apply_g(&w, g) else { continue };
            if !legal(l, &n) {
                continue;
            }
            let na = bridge(l, &n, awake | changed_regions(l, &w, &n, &cells));
            if goal(&n) {
                if goal_set.insert(n.clone()) {
                    let mut p = path(&par, w.clone(), awake);
                    p.push((g, awake));
                    goals.push((n.clone(), p));
                    if !want_all {
                        return (goals, false, max_awake | na);
                    }
                }
                continue;
            }
            let ms = seen.entry(n.clone()).or_default();
            if ms.iter().any(|m| m & na == na) {
                continue;
            }
            if nseen >= cap {
                capped = true;
                continue;
            }
            ms.retain(|m| m & na != *m);
            ms.push(na);
            nseen += 1;
            par.insert((n.clone(), na), (w.clone(), awake, g));
            q.push_back((n, na));
        }
    }
    (goals, capped, max_awake)
}

pub struct Verdict {
    pub ok: bool,
    pub lever: Option<usize>,
    pub report: String,
}

/// Check a mechanism (rest state + catalog run) in a box.
pub fn check(rest: &World, sol: &Solution, lo: [i8; 2], hi: [i8; 2], cap: usize) -> Verdict {
    let l = match layout(rest, sol, lo, hi) {
        Ok(l) => l,
        Err(e) => return Verdict { ok: false, lever: None, report: e },
    };
    let mut report = String::new();
    let mut sizes = vec![0usize; l.nregions];
    for k in l.comp.values() {
        sizes[*k] += 1;
    }
    report.push_str(&format!("{} free cells, {} regions {:?}, {} hops{}\n", l.comp.len(), l.nregions, sizes, l.hops.len(), if l.diverged { " (catalog run diverged in this macro model: footprint partial)" } else { "" }));
    // map: region digit per free cell, h = hop, # = footprint, o = opening
    for y in (l.lo[1]..=l.hi[1]).rev() {
        let mut row = format!("  {y:3} ");
        for x in l.lo[0]..=l.hi[0] {
            let p = [x, y, 0];
            let c = if let Some(k) = l.comp.get(&p) {
                char::from_digit(*k as u32 % 36, 36).unwrap_or('?')
            } else if l.hops.contains_key(&p) {
                'h'
            } else if in_t(p) {
                'o'
            } else if l.footprint.contains(&p) {
                '#'
            } else {
                '.'
            };
            row.push(c);
        }
        report.push_str(&row);
        report.push('\n');
    }
    let is_rest = |w: &World| *w == *rest;
    let mut best_fail = String::new();
    let mut undecided = false;
    let name = |i: u8| -> String {
        let (p, k) = rest.b[i as usize];
        format!("{}@({},{})", k.code(), p[0], p[1])
    };
    for lever in 0..l.nregions {
        let fired = std::cell::RefCell::new(std::collections::BTreeSet::new());
        let (opens, capped_o, aw_o) = edge(&l, rest, lever, &|w: &World| w.is_open(), cap, true, &fired);
        if opens.is_empty() {
            undecided |= capped_o;
            let never: Vec<String> = rest.pistons().into_iter().map(|i| i as u8).filter(|i| !fired.borrow().contains(i)).map(name).collect();
            let msg = format!(
                "lever K{lever}: no open state reachable{} (regions ever awake {:#b}; pistons never fed from an awake region: {})",
                if capped_o { " (cap hit)" } else { "" },
                aw_o,
                if never.is_empty() { "none".into() } else { never.join(" ") }
            );
            if best_fail.is_empty() || (!capped_o && best_fail.contains("cap hit")) {
                best_fail = msg.clone();
            }
            report.push_str(&format!("  {msg}\n"));
            continue;
        }
        let mut closed = false;
        let mut capped_c = false;
        let mut aw_c = 0u64;
        let fired_c = std::cell::RefCell::new(std::collections::BTreeSet::new());
        let fmt_g = |w0: &World, path: &[(GEv, u64)]| -> String {
            let mut w = w0.clone();
            let mut out = Vec::new();
            for (g, a) in path {
                let evs = match g {
                    GEv::One(e) => vec![*e],
                    GEv::Stack(x, y) => vec![*x, *y],
                };
                let txt: Vec<String> = evs.iter().map(|e| mechsynth::fmt_ev(&w, e)).collect();
                out.push(format!("{}[awake {:b}]", txt.join("+"), a));
                if let Some((n, _)) = apply_g(&w, *g) {
                    w = n;
                }
            }
            out.join(" ")
        };
        for (o, opath) in opens.iter().take(64) {
            let (back, cc, ac) = edge(&l, o, lever, &is_rest, cap, false, &fired_c);
            capped_c |= cc;
            aw_c |= ac;
            if let Some((_, cpath)) = back.first() {
                closed = true;
                report.push_str(&format!("  open order:  {}\n  close order: {}\n", fmt_g(rest, opath), fmt_g(o, cpath)));
                break;
            }
        }
        if closed {
            report.push_str(&format!("  lever K{lever}: WIRABLE (connectivity + order): {} open states reachable\n", opens.len()));
            return Verdict { ok: true, lever: Some(lever), report };
        }
        undecided |= capped_c;
        let never: Vec<String> = rest.pistons().into_iter().map(|i| i as u8).filter(|i| !fired_c.borrow().contains(i)).map(name).collect();
        let msg = format!(
            "lever K{lever}: opens ({} open states) but cannot close{} (regions ever awake on close {:#b}; pistons never fed on close: {})",
            opens.len(),
            if capped_c { " (cap hit)" } else { "" },
            aw_c,
            if never.is_empty() { "none".into() } else { never.join(" ") }
        );
        best_fail = msg.clone();
        report.push_str(&format!("  {msg}\n"));
    }
    let verdict = if undecided { "UNDECIDED (cap hit somewhere)" } else { "NOT wirable" };
    Verdict { ok: false, lever: None, report: format!("{report}  {verdict}: {best_fail}\n") }
}

/// `mctas door connect <catalog.tsv> --boxes 7x9 [--only ids]`: every entry, every placement of its
/// footprint in each box.
pub fn catalog_cli(path: &std::path::Path, only: &[String], boxes: &[(i8, i8)], cap: usize, threads: usize, verbose: bool) -> anyhow::Result<()> {
    mechsynth::SEAMLESS.store(true, Ordering::Relaxed);
    mechsynth::SEAMLESS_OPEN.store(true, Ordering::Relaxed);
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    let col = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| anyhow::anyhow!("no column {name}"));
    let (ci, cb, co, cp, cop, ccl, cs) = (col("id")?, col("box")?, col("door_offset")?, col("pieces")?, col("open")?, col("close")?, col("sym")?);
    struct Job {
        id: String,
        rest: World,
        sol: Solution,
        lo: [i8; 2],
        hi: [i8; 2],
        tag: String,
    }
    let mut jobs: Vec<Job> = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() <= cs.max(cp) {
            continue;
        }
        let id = f[ci];
        if !only.is_empty() && !only.iter().any(|o| id.starts_with(o.as_str())) {
            continue;
        }
        let mut b: Vec<(P, Kind)> = Vec::new();
        for x in 0..3 {
            for y in 0..3 {
                b.push(([x, y, 0], Kind::Door));
            }
        }
        let mut bad = None;
        for tok in f[cp].split_whitespace() {
            match crate::topology::parse_piece(tok) {
                Ok(pc) => b.push(pc),
                Err(e) => bad = Some(format!("{e}")),
            }
        }
        if let Some(e) = bad {
            println!("{id}: skipped ({e})");
            continue;
        }
        b.sort();
        let pow = World::rest_pow(&b);
        let w = World { b, pow };
        let seq = |s: &str| -> anyhow::Result<Vec<Ev>> { s.split_whitespace().map(|t| crate::topology::parse_event(&w, t)).collect() };
        let sym_txt = f[cs].split_once(':').map(|(_, s)| s).unwrap_or(f[cs]).trim();
        let sol = match (seq(f[cop]), seq(f[ccl])) {
            (Ok(open), Ok(close)) => Solution { open, close, sym: if sym_txt.is_empty() || sym_txt == "-" { None } else { seq(sym_txt).ok() }, states: 0 },
            _ => {
                println!("{id}: skipped (sequences)");
                continue;
            }
        };
        // footprint extent of the run, to place the mechanism in each box
        let own = (f[cb].to_string(), f[co].to_string());
        let (bw0, bh0) = own.0.split_once('x').map(|(a, b)| (a.parse::<i8>().unwrap_or(0), b.parse::<i8>().unwrap_or(0))).unwrap_or((0, 0));
        let (ox0, oy0) = own.1.split_once(',').map(|(a, b)| (a.trim().parse::<i8>().unwrap_or(0), b.trim().parse::<i8>().unwrap_or(0))).unwrap_or((0, 0));
        let lo0 = [-ox0, -oy0];
        let hi0 = [-ox0 + bw0 - 1, -oy0 + bh0 - 1];
        for &(bw, bh) in boxes {
            if bw < bw0 || bh < bh0 {
                continue;
            }
            for dx in 0..=(bw - bw0) {
                for dy in 0..=(bh - bh0) {
                    let lo = [lo0[0] - dx, lo0[1] - dy];
                    let hi = [lo[0] + bw - 1, lo[1] + bh - 1];
                    let _ = hi0;
                    jobs.push(Job { id: id.to_string(), rest: w.clone(), sol: sol.clone(), lo, hi, tag: format!("{bw}x{bh} x {}..{} y {}..{}", lo[0], hi[0], lo[1], hi[1]) });
                }
            }
        }
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads.max(1) {
            sc.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                let Some(j) = jobs.get(k) else { break };
                let v = check(&j.rest, &j.sol, j.lo, j.hi, cap);
                let line = format!("{} in {}: {}", j.id, j.tag, if v.ok { format!("WIRABLE (lever K{})", v.lever.unwrap_or(0)) } else { v.report.lines().last().unwrap_or("").trim().to_string() });
                println!("{line}");
                if verbose {
                    print!("{}", v.report);
                }
                results.lock().unwrap().push((j.id.clone(), j.tag.clone(), v.ok));
            });
        }
    });
    let res = results.into_inner().unwrap();
    let ok: Vec<String> = res.iter().filter(|r| r.2).map(|r| format!("{} ({})", r.0, r.1)).collect();
    println!("{} placements checked, {} wirable (connectivity + order): {}", res.len(), ok.len(), ok.join(", "));
    Ok(())
}
