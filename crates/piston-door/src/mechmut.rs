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
use std::collections::HashSet;
use std::path::Path;

/// A schedule as (piece, action) pairs: portable across neighbours (piston ids are not).
pub type Sched = Vec<(Pc, Act)>;

/// One solved variant.
#[derive(Clone, Debug)]
pub struct Var {
    pub bx: Bx,
    pub pieces: Vec<Pc>,
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
    pub so: Sched,
    pub sc: Sched,
}

impl Var {
    pub fn key(&self) -> (usize, usize, u8, usize) {
        let cr = match self.class {
            'A' => 0,
            'B' => 1,
            'C' => 2,
            _ => 3,
        };
        (self.unfed, self.pistons, cr, self.events)
    }
    pub fn line(&self) -> String {
        format!("{} {} {} {} {}", self.bx.w, self.bx.h, self.bx.x0, self.bx.y0, self.pieces.iter().map(|p| p.code()).collect::<Vec<_>>().join(" "))
    }
}

/// A piece change from the parent: (old piece, its replacement or None = dropped).
pub type Change = Option<(Pc, Option<Pc>)>;

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

/// Solve `pieces` in `bx`: the parent's schedule first (`hint`), then the exact solver (`cap`, 0 =
/// none); Some when it opens, closes and holds a FULL frame.
pub fn eval(bx: Bx, pieces: &[Pc], cap: usize, hint: Option<(&Sched, &Sched, Change)>, reserve: &[(i8, i8)]) -> Option<Var> {
    let (m, rest) = Mech::new(bx, pieces)?;
    let mut found: Option<(Vec<fam::E>, Vec<fam::E>, bool)> = None;
    if let Some((so, sc, ch)) = hint {
        let (o, c) = (from_sched(&m, so, ch), from_sched(&m, sc, ch));
        if replay(&m, &rest, &o, &c).is_some() {
            found = Some((o, c, true));
        }
    }
    if found.is_none() && cap > 0 {
        if let Ok(Some(sol)) = fam::solve(&m, &rest, cap, true) {
            found = Some((sol.open, sol.close, false));
        }
    }
    let (open_e, close_e, replayed) = found?;
    let (states, open) = replay(&m, &rest, &open_e, &close_e)?;
    let at = |s: &St, x: i8, y: i8| if bx.has(x, y) { s.g[bx.idx(x, y)] } else { fam::AIR };
    if reserve.iter().any(|&(x, y)| states.iter().any(|s| at(s, x, y) != fam::AIR)) {
        return None; // the cycle sweeps a reserved cell
    }
    let mut door_ring = 0;
    for (x, y) in fam::ring() {
        let c = at(&open, x, y);
        if c == fam::DOOR {
            door_ring += 1;
        } else if c == fam::STONE {
            // a static (or moved) plain block holds the frame cell
        } else if states.iter().any(|s| at(s, x, y) != fam::AIR) {
            return None; // neither a door block, a stone nor a static-stone cell when open
        }
    }
    let sol = fam::Sol { open: open_e.clone(), close: close_e.clone(), sym: None, sym_levels: false, states: 0, extent: (0, 0, 0, 0), levels: false };
    let w = fam::wiring(&m, &rest, &sol);
    let unfed = w.why.matches("has no free cell to feed it").count();
    Some(Var {
        bx,
        pieces: m.pieces.clone(),
        unfed,
        class: w.class,
        pistons: pieces.iter().filter(|p| matches!(p.k, K::Piston { .. })).count(),
        open: m.fmt_seq(&open_e),
        close: m.fmt_seq(&close_e),
        events: open_e.len() + close_e.len(),
        door_ring,
        replayed,
        so: to_sched(&m, &open_e),
        sc: to_sched(&m, &close_e),
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
    out
}

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
}

/// Beam search from every input mechanism; returns every kept variant, best first.
pub fn search(seeds: &[(Bx, Vec<Pc>)], o: &Opts) -> Vec<Var> {
    let mut seen: HashSet<Vec<Pc>> = HashSet::new();
    let mut kept: Vec<Var> = Vec::new();
    let mut beam: Vec<Var> = Vec::new();
    for (bx, p) in seeds {
        seen.insert(p.clone());
        match eval(*bx, p, o.cap, None, &o.reserve) {
            Some(v) => {
                eprintln!("[mechmut] seed {}: unfed {} class {} pistons {} events {}", v.line(), v.unfed, v.class, v.pistons, v.events);
                beam.push(v.clone());
                kept.push(v);
            }
            None => {
                eprintln!("[mechmut] seed does not solve within the cap (or no FULL frame): {} {:?}", p.iter().map(|q| q.code()).collect::<Vec<_>>().join(" "), bx);
                if o.repair {
                    // its neighbours may solve (e.g. a turned mechanism whose stacked pistons now
                    // couple by QC): search from it anyway
                    let pistons = p.iter().filter(|q| matches!(q.k, K::Piston { .. })).count();
                    beam.push(Var { bx: *bx, pieces: p.clone(), unfed: usize::MAX / 2, class: 'X', pistons, open: String::new(), close: String::new(), events: 0, door_ring: 0, replayed: false, so: Vec::new(), sc: Vec::new() });
                }
            }
        }
    }
    for round in 1..=o.rounds {
        let mut cand: Vec<(usize, Vec<Pc>, Change)> = Vec::new();
        for (k, v) in beam.iter().enumerate() {
            for (n, ch) in neighbours(v.bx, &v.pieces, &o.keep, &o.reserve) {
                if seen.insert(n.clone()) {
                    cand.push((k, n, ch));
                }
            }
        }
        if cand.is_empty() {
            break;
        }
        let t0 = std::time::Instant::now();
        let res = cs::par_map(&cand, o.threads, |(k, p, ch)| {
            let parent = &beam[*k];
            eval(parent.bx, p, o.bfs_cap, Some((&parent.so, &parent.sc, *ch)), &o.reserve)
        });
        let new: Vec<Var> = res.into_iter().flatten().collect();
        let nrep = new.iter().filter(|v| v.replayed).count();
        eprintln!("[mechmut] round {round}: {} neighbours, {} kept ({} by replay) ({:.0}s)", cand.len(), new.len(), nrep, t0.elapsed().as_secs_f64());
        kept.extend(new.iter().cloned());
        let mut next = new;
        next.sort_by_key(|v| v.key());
        beam = next.into_iter().take(o.beam).collect();
        if beam.is_empty() {
            break;
        }
        let b = &beam[0];
        eprintln!("[mechmut]   best: unfed {} class {} pistons {} events {}: {}", b.unfed, b.class, b.pistons, b.events, b.line());
    }
    kept.sort_by_key(|v| v.key());
    kept
}

/// Write the ideal-input design + plan (open/close schedule) of a variant into `dir`; when the
/// plan cannot be built (a piston firing from inside the opening has no input cell), the bare
/// design (`{tag}.txt`) for `door fullmech --check --export`.
pub fn export(v: &Var, dir: &Path, tag: &str) -> Result<()> {
    let (m, rest) = Mech::new(v.bx, &v.pieces).context("pieces overlap")?;
    let (o, c) = (from_sched(&m, &v.so, None), from_sched(&m, &v.sc, None));
    let sol = fam::Sol { open: o, close: c, sym: None, sym_levels: false, states: 0, extent: (0, 0, 0, 0), levels: false };
    let (wld, map) = fam::to_world(&m);
    std::fs::create_dir_all(dir)?;
    crate::mechsynth::design_of_world(&wld, tag).save(&dir.join(format!("{tag}.txt")))?;
    for (stag, msol) in fam::ideal_solutions(&m, &rest, &sol, &map) {
        let p = crate::mechsynth::ideal_plan(&wld, &msol, false, 6, tag)?;
        let dpath = dir.join(format!("{tag}-{stag}.txt"));
        let mut dd = p.design.clone();
        dd.name = format!("{tag}-{stag}");
        dd.save(&dpath)?;
        std::fs::write(dir.join(format!("{tag}-{stag}.ideal")), p.text.replace(&format!("design {tag}.txt"), &format!("design {}", dpath.display())))?;
    }
    Ok(())
}

/// The command: read the seeds, search, print the ranked TSV, export the top designs.
pub fn run(input: &Path, o: &Opts, out: Option<&Path>, ideal_dir: Option<&Path>) -> Result<()> {
    let seeds = parse_lines(&std::fs::read_to_string(input).with_context(|| format!("{}", input.display()))?)?;
    let kept = search(&seeds, o);
    let mut tsv = String::from("unfed\tclass\tpistons\tevents\tdoor_ring\treplayed\tmechanism\topen\tclose\n");
    for v in &kept {
        tsv.push_str(&format!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n", v.unfed, v.class, v.pistons, v.events, v.door_ring, v.replayed, v.line(), v.open, v.close));
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
