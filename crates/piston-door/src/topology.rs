//! Wiring topology of a mechanism inside a volume box (track box56's quadrant argument,
//! designs/piston-door-3x3/tracks/box56/NOTES.md): can its pistons be wired at all, in the door
//! plane, inside the box, with FULL seamless (no circuitry in the 12 ring cells when open)?
//!
//! Free cells = box cells of the door plane outside the opening and the ring that no mechanism
//! block, head or transient pulse head ever occupies along the run (rest -> open -> rest by the
//! symmetric sequence, else the open then the close path). Unswept ring cells hold stone only:
//! they join the components they touch (a strongly powered stone powers dust, repeaters and
//! pistons next to it). Components of free cells are the wiring regions.
//!
//! A piston can be FED from a component holding one of its neighbors (not its front) or a
//! neighbor of the cell above it (quasi-connectivity; when that cell holds another piston, the
//! feed powers it too, so it only counts if both take the same pulse count, e.g. a side stack),
//! at any position it takes. A component can
//! WATCH a piston from a free cell next to a position q: an observer there sees every change of
//! q (the piston's own pulse or level event while at q, its arrival at q, its departure). With
//! pulse mergers (gadget merger-gap) every event becomes one pulse, so a watcher relays
//! changes(q) pulses: odd counts carry the lever's toggle, even counts cannot (two events
//! seven or more ticks apart, an elevator's B window, never merge).
//!
//! Verdict: some component (the lever's) must reach a feeder of every piston: odd-count pistons
//! (one or three pulses per edge) need an odd-capable component, even-count ones (B's two
//! pulses) any reached component. `strict`: a link is odd iff changes(q) is odd; `permissive`:
//! also when changes(q) >= 3 (three events may chain-merge into an odd number); `memory`: also
//! a MEMORY link (track families, 2026-10-18): in a reached component with >= 2 free cells, a
//! waxed copper bulb T flip-flop turns any even pulse count into ONE level window. Cell rule (all
//! in that component, in the door plane): the link observer's output (its back face) powers the
//! bulb B directly (B is the cell behind the observer) or through dust; a comparator C stands
//! next to B facing away from it (it reads B through its back: signal 15 while B is lit) and its
//! output reaches the piston's feed cell (C next to the piston or its quasi-connectivity cell, or
//! dust from C). B toggles on each rising edge: lit between the 1st and the 2nd pulse, so a
//! pulse-driven piston gets one long pulse (extend at the 1st, retract with the sticky pull at
//! the 2nd). B and C cost 2 free cells; an observer watching B would fire twice again (no gain).
//! Not modeled: whether B and C fit next to the specific watcher and feeder cells (the check only
//! counts the component's free cells), latches (piston or note block reset on the other edge).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::mechsynth::{Act, D, Ev, Kind, P, Solution, World};

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

/// The run: every state in order, the events between them (state index before the event), and
/// the number of events of the first (opening) edge.
fn run(rest: &World, sol: &Solution) -> (Vec<World>, Vec<(usize, Ev)>, usize) {
    let mut states = vec![rest.clone()];
    let mut evs = Vec::new();
    let seqs: Vec<&Vec<Ev>> = match &sol.sym {
        Some(s) => vec![s, s],
        None => vec![&sol.open, &sol.close],
    };
    let first = seqs[0].len();
    for seq in seqs {
        for e in seq {
            let cur = states.last().unwrap().clone();
            evs.push((states.len() - 1, *e));
            states.push(cur.apply(*e).unwrap_or(cur));
        }
    }
    (states, evs, first)
}

pub struct Topo {
    pub strict: bool,
    pub permissive: bool,
    /// Wirable when T flip-flops (copper bulb + comparator, 2 free cells) may turn an even link
    /// into one window (memory links).
    pub memory: bool,
    pub report: String,
}

/// Analyze `rest` (with its solution) in the box x lo[0]..=hi[0], y lo[1]..=hi[1] of the door plane.
pub fn analyze(rest: &World, sol: &Solution, lo: [i8; 2], hi: [i8; 2]) -> Topo {
    let (states, evs, edge_len) = run(rest, sol);
    let in_box = |p: P| p[2] == 0 && (lo[0]..=hi[0]).contains(&p[0]) && (lo[1]..=hi[1]).contains(&p[1]);
    let mut swept: HashSet<P> = HashSet::new();
    let mut outside: BTreeSet<P> = BTreeSet::new();
    for s in &states {
        for (p, k) in &s.b {
            swept.insert(*p);
            if let Kind::Piston { ext: true, f, .. } = k {
                swept.insert(addp(*p, D[*f as usize]));
            }
            if !matches!(k, Kind::Door) && !in_box(*p) {
                outside.insert(*p);
            }
        }
    }
    for (si, e) in &evs {
        if e.act == Act::P {
            let (p, k) = states[*si].b[e.id as usize];
            if let Kind::Piston { f, .. } = k {
                swept.insert(addp(p, D[f as usize]));
            }
        }
    }
    let mut report = String::new();
    if !outside.is_empty() {
        report.push_str(&format!("mechanism blocks outside the box: {outside:?}\n"));
        return Topo { strict: false, permissive: false, memory: false, report };
    }
    // Components of free cells, joined through unswept ring cells (stone hops).
    let mut cells: Vec<P> = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            cells.push([x, y, 0]);
        }
    }
    let free = |p: P| in_box(p) && !in_t(p) && !in_ring(p) && !swept.contains(&p);
    let ring_free = |p: P| in_box(p) && in_ring(p) && !swept.contains(&p);
    let mut comp: HashMap<P, usize> = HashMap::new();
    let mut n = 0;
    for &c in &cells {
        if !free(c) || comp.contains_key(&c) {
            continue;
        }
        let mut stack = vec![c];
        comp.insert(c, n);
        while let Some(p) = stack.pop() {
            for d in PLANE {
                let q = addp(p, D[d]);
                if free(q) && !comp.contains_key(&q) {
                    comp.insert(q, n);
                    stack.push(q);
                }
            }
        }
        n += 1;
    }
    // Union-find over components touching a common free ring cell.
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut Vec<usize>, i: usize) -> usize {
        let mut r = i;
        while parent[r] != r {
            r = parent[r];
        }
        let mut j = i;
        while parent[j] != r {
            let nx = parent[j];
            parent[j] = r;
            j = nx;
        }
        r
    }
    let mut ring_comps: HashMap<P, Vec<usize>> = HashMap::new();
    for &c in &cells {
        if !ring_free(c) {
            continue;
        }
        let ks: BTreeSet<usize> = PLANE.iter().filter_map(|d| comp.get(&addp(c, D[*d])).copied()).collect();
        let ks: Vec<usize> = ks.into_iter().collect();
        for w in ks.windows(2) {
            let (a, b) = (find(&mut parent, w[0]), find(&mut parent, w[1]));
            parent[a] = b;
        }
        ring_comps.insert(c, ks);
    }
    let root = |parent: &mut Vec<usize>, k: usize| find(parent, k);
    let comp_of = |parent: &mut Vec<usize>, p: P| -> Vec<usize> {
        if let Some(k) = comp.get(&p) {
            return vec![root(parent, *k)];
        }
        if let Some(ks) = ring_comps.get(&p) {
            let mut v: Vec<usize> = ks.iter().map(|k| root(parent, *k)).collect();
            v.sort();
            v.dedup();
            return v;
        }
        Vec::new()
    };
    // Per piston: positions, own events per edge, feeders, watchers with change counts.
    let pistons: Vec<usize> = rest.b.iter().enumerate().filter(|(_, (_, k))| matches!(k, Kind::Piston { .. })).map(|(i, _)| i).collect();
    let mut feeders: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    // Own events per (edge, piston): edge 0 = opening, 1 = closing.
    let mut needs: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    // watch links: (edge, piston, component, changes, watcher cell)
    let mut watches: Vec<(usize, usize, usize, usize, P)> = Vec::new();
    let ranges = [0..edge_len, edge_len..evs.len()];
    for &i in &pistons {
        for (edge, r) in ranges.iter().enumerate() {
            let own = evs[r.clone()].iter().filter(|(_, e)| e.id as usize == i).count();
            needs.insert((edge, i), own);
        }
    }
    for &i in &pistons {
        let mut positions: Vec<(P, u8)> = Vec::new();
        let fs = feeders.entry(i).or_default();
        for s in &states {
            let (q, f) = match s.b[i] {
                (p, Kind::Piston { f, .. }) => (p, f),
                _ => continue,
            };
            if !positions.contains(&(q, f)) {
                positions.push((q, f));
            }
            let front = addp(q, D[f as usize]);
            let above = addp(q, D[2]);
            // Quasi-connectivity through a cell that holds another piston powers that piston too
            // (a block, dust or observer next to it feeds it directly): allowed only when both
            // take the same number of pulses on each edge (a side stack fired as one), not when
            // it would fire B with A's pulses.
            let qc_ok = s.b.iter().enumerate().all(|(j, (p, k))| {
                j == i || *p != above || !matches!(k, Kind::Piston { .. }) || (0..2).all(|e| needs[&(e, j)] == needs[&(e, i)])
            });
            for d in PLANE {
                let c = addp(q, D[d]);
                if c != front {
                    for k in comp_of(&mut parent, c) {
                        fs.insert(k);
                    }
                }
                let c2 = addp(above, D[d]);
                if qc_ok && c2 != q {
                    for k in comp_of(&mut parent, c2) {
                        fs.insert(k);
                    }
                }
            }
        }
        // Changes of each position cell on each edge.
        for &(q, _) in &positions {
            for (edge, r) in ranges.iter().enumerate() {
                let mut ch = 0;
                for (si, e) in &evs[r.clone()] {
                    let before = states[*si].b[i].0;
                    let after = states[*si + 1].b[i].0;
                    if e.id as usize == i && before == q {
                        ch += 1;
                    }
                    if before != after && (before == q || after == q) {
                        ch += 1;
                    }
                }
                for d in PLANE {
                    let c = addp(q, D[d]);
                    if let Some(k) = comp.get(&c) {
                        let k = root(&mut parent, *k);
                        watches.push((edge, i, k, ch, c));
                    }
                }
            }
        }
    }
    let ncomp: BTreeSet<usize> = (0..n).map(|k| root(&mut parent, k)).collect();
    let name = |i: usize| -> String {
        let (p, k) = rest.b[i];
        format!("{}@({},{})", k.code(), p[0], p[1])
    };
    report.push_str(&format!("{} wiring components (stone hops merged), {} free cells\n", ncomp.len(), comp.len()));
    for &k in &ncomp {
        let fed: Vec<String> = pistons.iter().filter(|i| feeders[i].contains(&k)).map(|i| name(*i)).collect();
        let size = comp.iter().filter(|(_, c)| root(&mut parent, **c) == k).count();
        report.push_str(&format!("  K{k}: {size} cells, feeds {}\n", if fed.is_empty() { "-".into() } else { fed.join(" ") }));
    }
    for &(edge, i, k, ch, c) in &watches {
        if feeders[&i].iter().any(|f| *f != k) {
            report.push_str(&format!("  link ({}): {} watched from ({},{}) in K{k}: {ch} changes\n", if edge == 0 { "open" } else { "close" }, name(i), c[0], c[1]));
        }
    }
    // Both edges must be wirable with the lever in the same component. mode 0 strict, 1
    // permissive (chain merges), 2 memory: also a T flip-flop (waxed copper bulb + comparator,
    // 2 free cells) in any reached component turns its even pulse count into one window.
    let csize: HashMap<usize, usize> = {
        let mut m: HashMap<usize, usize> = HashMap::new();
        let cells: Vec<usize> = comp.values().copied().collect();
        for c in cells {
            *m.entry(root(&mut parent, c)).or_default() += 1;
        }
        m
    };
    let solve = |mode: usize, k0: usize| -> Result<(), String> {
        let permissive = mode == 1;
        let mut bad: Vec<String> = Vec::new();
        for edge in 0..2 {
            let mut odd: BTreeSet<usize> = BTreeSet::from([k0]);
            let mut any: BTreeSet<usize> = BTreeSet::from([k0]);
            let drivable = |i: usize, odd: &BTreeSet<usize>, any: &BTreeSet<usize>| -> bool {
                let need = needs[&(edge, i)];
                if need % 2 == 1 { feeders[&i].iter().any(|f| odd.contains(f)) } else { feeders[&i].iter().any(|f| any.contains(f)) }
            };
            loop {
                let mut grew = false;
                for &(e, i, k, ch, _) in &watches {
                    if e != edge || needs[&(edge, i)] == 0 || !drivable(i, &odd, &any) {
                        continue;
                    }
                    let is_odd = ch % 2 == 1 || (permissive && ch >= 3);
                    if any.insert(k) {
                        grew = true;
                    }
                    if is_odd && odd.insert(k) {
                        grew = true;
                    }
                }
                if mode == 2 {
                    let even_only: Vec<usize> = any.iter().copied().filter(|k| !odd.contains(k) && csize.get(k).copied().unwrap_or(0) >= 2).collect();
                    for k in even_only {
                        odd.insert(k);
                        grew = true;
                    }
                }
                if !grew {
                    break;
                }
            }
            for &i in &pistons {
                let need = needs[&(edge, i)];
                if need > 0 && !drivable(i, &odd, &any) {
                    bad.push(format!("{} ({} pulses on {})", name(i), need, if edge == 0 { "open" } else { "close" }));
                }
            }
        }
        if bad.is_empty() { Ok(()) } else { Err(bad.join(", ")) }
    };
    let mut verdict = [false, false, false];
    for (vi, mode) in [0usize, 1, 2].into_iter().enumerate() {
        let label = ["strict", "permissive", "memory (copper-bulb T flip-flops)"][mode];
        let mut best: Option<(usize, String)> = None;
        for &k0 in &ncomp {
            match solve(mode, k0) {
                Ok(()) => {
                    verdict[vi] = true;
                    report.push_str(&format!("  {label}: WIRABLE with the lever in K{k0}\n"));
                    break;
                }
                Err(e) => {
                    let n = e.matches('@').count();
                    if best.as_ref().is_none_or(|(m, _)| n < *m) {
                        best = Some((n, format!("lever in K{k0}: unreachable {e}")));
                    }
                }
            }
        }
        if !verdict[vi] {
            report.push_str(&format!("  {label}: NOT wirable (best: {})\n", best.map(|b| b.1).unwrap_or_default()));
        }
    }
    Topo { strict: verdict[0], permissive: verdict[1], memory: verdict[2], report }
}

/// One catalog piece: `SE(-2,0)`, `SE+(-1,3)`, `ND(1,4)` (sticky/normal, facing, extended at rest).
fn parse_piece(tok: &str) -> anyhow::Result<(P, Kind)> {
    let (head, rest) = tok.split_once('(').ok_or_else(|| anyhow::anyhow!("piece {tok:?}"))?;
    let coords = rest.trim_end_matches(')');
    let (x, y) = coords.split_once(',').ok_or_else(|| anyhow::anyhow!("piece {tok:?}"))?;
    let p: P = [x.trim().parse()?, y.trim().parse()?, 0];
    let mut ch = head.chars();
    let sticky = match ch.next() {
        Some('S') => true,
        Some('N') => false,
        _ => anyhow::bail!("piece {tok:?}: S or N"),
    };
    let f = match ch.next() {
        Some('E') => 0,
        Some('W') => 1,
        Some('U') => 2,
        Some('D') => 3,
        _ => anyhow::bail!("piece {tok:?}: E/W/U/D"),
    };
    let ext = ch.next() == Some('+');
    Ok((p, Kind::Piston { sticky, f, ext }))
}

/// One catalog event: `PSD(1,4)` or `LSE+(-1,-1)` (pulse / level on the piston at that rest cell).
fn parse_event(w: &World, tok: &str) -> anyhow::Result<Ev> {
    let act = match tok.chars().next() {
        Some('P') => Act::P,
        Some('L') => Act::L,
        _ => anyhow::bail!("event {tok:?}"),
    };
    let (p, _) = parse_piece(&tok[1..])?;
    let id = w.b.iter().position(|(q, k)| *q == p && matches!(k, Kind::Piston { .. })).ok_or_else(|| anyhow::anyhow!("event {tok:?}: no piston there"))?;
    Ok(Ev { id: id as u8, act })
}

/// `mctas door topology <catalog.tsv>`: the topology verdict of every mechanism of agent families'
/// catalog (id, box, door_offset, pieces, open, close, sym columns), in its own box.
pub fn catalog_cli(path: &std::path::Path, only: &[String], verbose: bool) -> anyhow::Result<()> {
    let text = std::fs::read_to_string(path)?;
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap_or_default().split('\t').collect();
    let col = |name: &str| header.iter().position(|h| *h == name).ok_or_else(|| anyhow::anyhow!("no column {name}"));
    let (ci, cb, co, cp, cop, ccl, cs) = (col("id")?, col("box")?, col("door_offset")?, col("pieces")?, col("open")?, col("close")?, col("sym")?);
    let (mut n, mut ns, mut np, mut nm) = (0, 0, 0, 0);
    for line in lines {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() <= cs.max(cp) {
            continue;
        }
        let id = f[ci];
        if !only.is_empty() && !only.iter().any(|o| id.starts_with(o.as_str())) {
            continue;
        }
        let (bw, bh) = f[cb].split_once('x').ok_or_else(|| anyhow::anyhow!("box {}", f[cb]))?;
        let (ox, oy) = f[co].split_once(',').ok_or_else(|| anyhow::anyhow!("offset {}", f[co]))?;
        let (bw, bh, ox, oy): (i8, i8, i8, i8) = (bw.parse()?, bh.parse()?, ox.trim().parse()?, oy.trim().parse()?);
        let lo = [-ox, -oy];
        let hi = [-ox + bw - 1, -oy + bh - 1];
        let mut b: Vec<(P, Kind)> = Vec::new();
        for x in 0..3 {
            for y in 0..3 {
                b.push(([x, y, 0], Kind::Door));
            }
        }
        for tok in f[cp].split_whitespace() {
            b.push(parse_piece(tok)?);
        }
        b.sort();
        let pow = World::rest_pow(&b);
        let w = World { b, pow };
        let seq = |s: &str| -> anyhow::Result<Vec<Ev>> { s.split_whitespace().map(|t| parse_event(&w, t)).collect() };
        let sym_txt = f[cs].split_once(':').map(|(_, s)| s).unwrap_or(f[cs]).trim();
        let sol = Solution {
            open: seq(f[cop])?,
            close: seq(f[ccl])?,
            sym: if sym_txt.is_empty() { None } else { Some(seq(sym_txt)?) },
            states: 0,
        };
        let t = analyze(&w, &sol, lo, hi);
        n += 1;
        ns += t.strict as usize;
        np += t.permissive as usize;
        nm += t.memory as usize;
        let comps = t.report.lines().next().unwrap_or_default().to_string();
        let tag = |b: bool| if b { "WIRABLE" } else { "no" };
        println!("{id} {}x{} offset {},{} {}: strict {} / permissive {} / memory {} ({comps})", bw, bh, ox, oy, if sol.sym.is_some() { "sym" } else { "oc" }, tag(t.strict), tag(t.permissive), tag(t.memory));
        if verbose {
            print!("{}", t.report);
        }
    }
    println!("{n} mechanisms: {ns} wirable (strict parity), {np} (permissive), {nm} (memory links)");
    Ok(())
}
