//! `door feeds` (track d67): for every firing of an ideal plan (each redstone block the plan
//! places), the pistons that block powers and where a FLAT build could deliver the same pulse
//! instead, from the door plane's contents at that tick and the cells that stay free all cycle.
//!
//! A piston (facing `f`, cell X) takes power from (`PistonBaseBlock.getNeighborSignal`):
//! * DIRECT: a component in a neighbour N of X other than its front X + f pointing into X, or a
//!   conductor block in N strongly powered by a component in one of N's other neighbours M
//!   ("via N<-M": door, frame and static blocks conduct; power never hops block to block);
//! * QUASI-CONNECTIVITY: a signal into the cell above it, A = X + up, from A's other neighbours
//!   (A + up, A +- x): a component there pointing into A, or a strongly powered conductor there
//!   ("qc Q<-M"). A is X's front for a piston facing up: still counted (QC ignores the facing).
//!
//! Only cells inside the box that are air at EVERY tick of the open and the close (and at rest)
//! can hold a static component; they are grouped into 4-connected free regions (letters, with
//! their size: a single sealed cell can only hold an observer, which outputs opposite the cell it
//! watches). A firing with no option is a core for any static wiring: it needs a carried
//! component, a mechanism change, or a feed cell reserved by the mechanism search.

use crate::DoorSpec;
use crate::cosearch::{self as cs, Pos};
use crate::ideal::Schedule;
use anyhow::{Context, Result, bail, ensure};
use redsim::pos::Dir;
use redsim::{Block, BlockPos, World};
use std::collections::{BTreeMap, BTreeSet};

type P2 = (i32, i32);

struct Firing {
    open: bool,
    t: u32,
    input: Pos,
    /// The door plane just before the redstone block is placed.
    plane: BTreeMap<P2, Block>,
}

fn nbrs(c: P2) -> [P2; 4] {
    [(c.0 - 1, c.1), (c.0 + 1, c.1), (c.0, c.1 - 1), (c.0, c.1 + 1)]
}

fn step2(c: P2, d: Dir) -> Option<P2> {
    let s = d.step();
    if s[2] != 0 { None } else { Some((c.0 + s[0], c.1 + s[1])) }
}

fn short(b: &Block) -> String {
    match *b {
        Block::Piston { sticky, facing, .. } => format!("{}{}", if sticky { "S" } else { "N" }, &format!("{facing:?}")[..1]),
        _ => b.to_string().trim_start_matches("minecraft:").chars().take(10).collect(),
    }
}

/// `--box X0..X1,Y0..Y1` (design coordinates of the door plane).
pub fn parse_box(s: &str) -> Result<(i32, i32, i32, i32)> {
    let (xs, ys) = s.split_once(',').context("--box X0..X1,Y0..Y1")?;
    let r = |v: &str| -> Result<(i32, i32)> {
        let (a, b) = v.split_once("..").context("--box X0..X1,Y0..Y1")?;
        Ok((a.trim().parse()?, b.trim().parse()?))
    };
    let ((x0, x1), (y0, y1)) = (r(xs)?, r(ys)?);
    Ok((x0, y0, x1, y1))
}

/// What [`analyze`] finds: the per-firing table (`text`, the lines `door feeds` prints) and its
/// summary, for searches that rank mechanisms by how feedable their firings are.
#[derive(Clone, Debug, Default)]
pub struct FeedReport {
    pub firings: usize,
    /// Firings with no option at all.
    pub none: Vec<String>,
    /// Firings whose options are all flagged (no observer/repeater can drive them from inside
    /// the box: dust or a torch at best).
    pub weak: Vec<String>,
    /// Cells that are the only clean option of firings needing different targets.
    pub conflicts: Vec<String>,
    pub text: Vec<String>,
}

/// `door feeds`: print [`analyze`]'s table for one schedule file.
pub fn cli(path: &std::path::Path, bx: Option<(i32, i32, i32, i32)>) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let r = analyze(&text, &path.display().to_string(), bx)?;
    for l in &r.text {
        println!("{l}");
    }
    Ok(())
}

/// The feed table of an ideal schedule (its text; `label` names it in the header).
pub fn analyze(text: &str, label: &str, bx: Option<(i32, i32, i32, i32)>) -> Result<FeedReport> {
    let mut out: Vec<String> = Vec::new();
    let s = Schedule::parse_with(text, &BTreeMap::new())?;
    let vars = s.vars().clone();
    let (d, open_ev, close_ev, _) = s.realize(&vars)?;
    let door = *d.ports.get("door").context("design has no door port")?;
    if d.meta.get("door-axis").is_some_and(|v| v == "z") {
        bail!("door feeds: doors along z are not supported");
    }
    let spec = DoorSpec { door, lever: [0, 0, 0], along_z: false };
    let z = door[2];
    let mut blocks = Vec::new();
    for (p, st) in &d.blocks {
        blocks.push((*p, cs::convert_state(st).map_err(|e| anyhow::anyhow!("{st} at {p:?}: {e}"))?));
    }
    let (x0, y0, x1, y1) = match bx {
        Some(b) => b,
        None => {
            let ps: Vec<&Pos> = blocks.iter().map(|(p, _)| p).filter(|p| p[2] == z).collect();
            ensure!(!ps.is_empty(), "no block in the door plane");
            (ps.iter().map(|p| p[0]).min().unwrap(), ps.iter().map(|p| p[1]).min().unwrap(), ps.iter().map(|p| p[0]).max().unwrap(), ps.iter().map(|p| p[1]).max().unwrap())
        }
    };
    let (mut w, _, idle) = cs::place_and_settle(&blocks, &spec, 400);
    ensure!(idle && w.errors.is_empty(), "the design does not settle after placement: {}", w.errors.join("; "));
    // cells holding what the plan itself placed (ideal inputs, triggers) do not count as taken
    let snap = move |w: &World, held: &BTreeMap<P2, Block>| -> BTreeMap<P2, Block> {
        let mut m = BTreeMap::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let b = w.get(BlockPos::new(x, y, z));
                if !b.is_air() && held.get(&(x, y)) != Some(&b) {
                    m.insert((x, y), b);
                }
            }
        }
        m
    };
    let mut held: BTreeMap<P2, Block> = BTreeMap::new();
    let mut all: Vec<BTreeMap<P2, Block>> = vec![snap(&w, &held)];
    // per edge, the plane after t ticks (index t), for the observer timing
    let mut edge_snaps: [Vec<BTreeMap<P2, Block>>; 2] = [Vec::new(), Vec::new()];
    let mut firings: Vec<Firing> = Vec::new();
    for (open, evs) in [(true, &open_ev), (false, &close_ev)] {
        let es = if open { 0 } else { 1 };
        edge_snaps[es].push(snap(&w, &held));
        let (mut t, mut i) = (0u32, 0usize);
        while i < evs.len() {
            while i < evs.len() && evs[i].0 == t {
                let (_, q, st) = &evs[i];
                if st.as_deref() == Some("@use") {
                    w.use_block(cs::bp(*q));
                } else {
                    let b = match st {
                        Some(s) => cs::convert_state(s).map_err(|e| anyhow::anyhow!("{s}: {e}"))?,
                        None => Block::Air,
                    };
                    if st.as_deref().is_some_and(|s| s.trim_start_matches("minecraft:") == "redstone_block") {
                        firings.push(Firing { open, t, input: *q, plane: snap(&w, &held) });
                    }
                    w.set_block(cs::bp(*q), b, redsim::world::flags::ALL);
                    if q[2] == z {
                        if b.is_air() {
                            held.remove(&(q[0], q[1]));
                        } else {
                            held.insert((q[0], q[1]), b);
                        }
                    }
                }
                i += 1;
            }
            if i >= evs.len() {
                break;
            }
            w.tick();
            t += 1;
            let s = snap(&w, &held);
            all.push(s.clone());
            edge_snaps[es].push(s);
        }
        let es_v = &mut edge_snaps[es];
        let (_, idle) = cs::step_until_idle(&mut w, 300, |w| {
            let s = snap(w, &held);
            all.push(s.clone());
            es_v.push(s);
        });
        ensure!(idle && w.errors.is_empty(), "still moving 300 ticks after the last event: {}", w.errors.join("; "));
    }
    // components that never move (observers of the design): their output cell
    let mut fixed_out: Vec<(P2, P2, P2)> = Vec::new();
    for (&c, b) in &all[0] {
        if let Block::Observer { facing, .. } = *b
            && all.iter().all(|m| matches!(m.get(&c), Some(Block::Observer { facing: f, .. }) if *f == facing))
            && let (Some(out), Some(watch)) = (step2(c, facing.opposite()), step2(c, facing))
        {
            fixed_out.push((c, out, watch));
        }
    }
    // cells free all cycle, and their regions
    let mut free: BTreeSet<P2> = BTreeSet::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            if all.iter().all(|m| !m.contains_key(&(x, y))) {
                free.insert((x, y));
            }
        }
    }
    let mut region: BTreeMap<P2, (char, usize)> = BTreeMap::new();
    let mut label = b'A';
    for c in free.iter() {
        if region.contains_key(c) {
            continue;
        }
        let mut comp = vec![*c];
        let mut seen: BTreeSet<P2> = [*c].into();
        let mut k = 0;
        while k < comp.len() {
            for n in nbrs(comp[k]) {
                if free.contains(&n) && seen.insert(n) {
                    comp.push(n);
                }
            }
            k += 1;
        }
        let l = label as char;
        label = if label == b'Z' { b'a' } else { label + 1 };
        for q in &comp {
            region.insert(*q, (l, comp.len()));
        }
    }
    let cell = |c: P2| -> String {
        match region.get(&c) {
            Some((l, n)) => format!("({},{}){l}{n}", c.0, c.1),
            None => format!("({},{})", c.0, c.1),
        }
    };
    // cells whose contents change during the cycle (an observer watching one can fire)
    let dynamic: BTreeSet<P2> = {
        let mut s = BTreeSet::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let first = all[0].get(&(x, y));
                if all.iter().any(|m| m.get(&(x, y)) != first) {
                    s.insert((x, y));
                }
            }
        }
        s
    };
    let in_box = |c: P2| c.0 >= x0 && c.0 <= x1 && c.1 >= y0 && c.1 <= y1;
    // a component in m pointing into t (observer, repeater, comparator) watches / takes its input
    // from the cell opposite t: "!" when that cell is outside the box, "?" when it never changes
    // and holds a block (only dust or a torch could then drive it)
    let drive = |m: P2, t: P2| -> &'static str {
        let w = (2 * m.0 - t.0, 2 * m.1 - t.1);
        if !in_box(w) {
            "!"
        } else if free.contains(&w) || dynamic.contains(&w) {
            ""
        } else {
            "?"
        }
    };
    let regions: BTreeMap<char, usize> = region.values().map(|(l, n)| (*l, *n)).collect();
    out.push(format!("# door feeds {label} (box x {x0}..{x1}, y {y0}..{y1}; {} firings)", firings.len()));
    out.push(format!("# free all cycle: {} cells in {} regions: {}", free.len(), regions.len(), regions.iter().map(|(l, n)| format!("{l}{n}")).collect::<Vec<_>>().join(" ")));
    out.push("# cells: (x,y) then the free region and its size; via N<-M = conductor block N strongly powered from free cell M".to_string());
    out.push("# flags: ! = the cell an observer/repeater there would watch or take input from (opposite its target) is outside the box; ? = that cell is a block that never changes (only dust or a torch could drive it)".to_string());
    out.push("# timing of a clean option (an observer there): @ = the cell it watches changes the tick before the firing (fires with the plan), ~ = it changes only at other ticks; none = a free cell (other wiring drives it)".to_string());
    let mut none: Vec<String> = Vec::new();
    let mut weak: Vec<String> = Vec::new();
    // cell -> (targets, firing) of the firings whose clean options are all in that cell
    let mut sole: BTreeMap<P2, Vec<(BTreeSet<P2>, String)>> = BTreeMap::new();
    // an observer in m pointing into t watches w = 2m - t: "@" if w changes in the tick before the
    // firing's (it then fires with the plan's input), "~" if w changes only at other ticks (the
    // schedule would have to follow w), nothing if w is a free cell (other wiring drives it)
    let timing = |e: usize, t: u32, m: P2, tg: P2| -> &'static str {
        let w = (2 * m.0 - tg.0, 2 * m.1 - tg.1);
        if !dynamic.contains(&w) {
            return "";
        }
        let s = &edge_snaps[e];
        let t = t as usize;
        if t >= 2 && t < s.len() + 1 && s.get(t - 1).map(|m| m.get(&w)) != s.get(t - 2).map(|m| m.get(&w)) { "@" } else { "~" }
    };
    for f in &firings {
        let i2 = (f.input[0], f.input[1]);
        let in_plane = f.input[2] == z;
        let mut lines: Vec<String> = Vec::new();
        for (&x, b) in &f.plane {
            let Block::Piston { facing, .. } = *b else { continue };
            let above = (x.0, x.1 + 1);
            let front = step2(x, facing);
            let (direct_i, qc_i) = if in_plane {
                (nbrs(x).contains(&i2) && Some(i2) != front, nbrs(above).contains(&i2) && i2 != x)
            } else {
                (i2 == x, i2 == above)
            };
            if !direct_i && !qc_i {
                continue;
            }
            let conductor = |c: P2| f.plane.get(&c).is_some_and(redsim::behavior::conductor_of);
            let mut direct: Vec<String> = Vec::new();
            // the design's fixed observers: output into X, into a conductor next to X, or into A
            let mut obs: Vec<String> = Vec::new();
            for &(c, out, watch) in &fixed_out {
                let via = if out == x && Some(c) != front {
                    Some("direct".to_string())
                } else if out == above && c != x {
                    Some("qc".to_string())
                } else if out != x && nbrs(x).contains(&out) && Some(out) != front && conductor(out) {
                    Some(format!("via ({},{})", out.0, out.1))
                } else {
                    None
                };
                if let Some(v) = via {
                    obs.push(format!("observer ({},{}) {v}, watching {}", c.0, c.1, cell(watch)));
                }
            }
            // options (QC?, text, component cell M, its target T): direct ones into X or into a
            // conductor next to X, QC ones into A or into a conductor next to A
            let mut opts: Vec<(bool, String, P2, P2)> = Vec::new();
            for n in nbrs(x) {
                if Some(n) == front {
                    continue;
                }
                if free.contains(&n) {
                    opts.push((false, cell(n), n, x));
                } else if conductor(n) {
                    for m in nbrs(n) {
                        if m != x && free.contains(&m) {
                            opts.push((false, format!("via ({},{})<-{}", n.0, n.1, cell(m)), m, n));
                        }
                    }
                }
            }
            for q in [(above.0, above.1 + 1), (above.0 - 1, above.1), (above.0 + 1, above.1)] {
                if free.contains(&q) {
                    opts.push((true, cell(q), q, above));
                } else if conductor(q) {
                    for m in nbrs(q) {
                        if m != above && free.contains(&m) {
                            opts.push((true, format!("via ({},{})<-{}", q.0, q.1, cell(m)), m, q));
                        }
                    }
                }
            }
            let e = if f.open { 0 } else { 1 };
            let mut ok_any = !obs.is_empty();
            let mut flagged_any = false;
            let mut clean: Vec<(P2, P2)> = Vec::new();
            let mut qc: Vec<String> = Vec::new();
            for (is_qc, text, m, tg) in opts {
                let fl = drive(m, tg);
                let s = if fl.is_empty() {
                    ok_any = true;
                    clean.push((m, tg));
                    format!("{text}{}", timing(e, f.t, m, tg))
                } else {
                    flagged_any = true;
                    format!("{text}{fl}")
                };
                if is_qc { qc.push(s) } else { direct.push(s) }
            }
            let how = if direct_i { "direct" } else { "qc" };
            let own = if in_plane && free.contains(&i2) { ", the plan's input cell is free" } else { "" };
            let verdict = if ok_any {
                "feed"
            } else if flagged_any {
                "weak"
            } else {
                "NONE"
            };
            lines.push(format!(
                "{} {}({},{}) [{how}{own}]: {verdict}; direct: {}; qc: {}{}",
                short(b),
                "",
                x.0,
                x.1,
                if direct.is_empty() { "-".into() } else { direct.join(" ") },
                if qc.is_empty() { "-".into() } else { qc.join(" ") },
                if obs.is_empty() { String::new() } else { format!("; {}", obs.join("; ")) }
            ));
            let label = format!("{} t{} {}({},{})", if f.open { "open" } else { "close" }, f.t, short(b), x.0, x.1);
            // a firing whose clean options all sit in one cell depends on that cell's one component
            if obs.is_empty() && !clean.is_empty() {
                let cells: BTreeSet<P2> = clean.iter().map(|c| c.0).collect();
                if cells.len() == 1 {
                    let targets: BTreeSet<P2> = clean.iter().map(|c| c.1).collect();
                    sole.entry(clean[0].0).or_default().push((targets, label.clone()));
                }
            }
            match verdict {
                "NONE" => none.push(label),
                "weak" => weak.push(label),
                _ => {}
            }
        }
        let edge = if f.open { "open" } else { "close" };
        let at = format!("({},{},{})", f.input[0], f.input[1], f.input[2]);
        if lines.is_empty() {
            out.push(format!("{edge} t{} input {at}: powers no piston in the plane", f.t));
        }
        for l in lines {
            out.push(format!("{edge} t{} input {at} -> {l}", f.t));
        }
    }
    out.push(format!("# no feed at all: {}", if none.is_empty() { "none".to_string() } else { none.join(", ") }));
    out.push(format!("# only flagged options (weak): {}", if weak.is_empty() { "none".to_string() } else { weak.join(", ") }));
    let mut conflicts: Vec<String> = Vec::new();
    for (m, deps) in &sole {
        let common = deps.iter().skip(1).fold(deps[0].0.clone(), |acc, d| acc.intersection(&d.0).copied().collect::<BTreeSet<P2>>());
        if deps.len() > 1 && common.is_empty() {
            let who: Vec<String> = deps.iter().map(|(ts, l)| format!("{l} -> {}", ts.iter().map(|t| format!("({},{})", t.0, t.1)).collect::<Vec<_>>().join("/"))).collect();
            conflicts.push(format!("({},{}): {}", m.0, m.1, who.join(", ")));
        }
    }
    out.push(format!("# one cell, several targets (its one component cannot feed them all): {}", if conflicts.is_empty() { "none".to_string() } else { conflicts.join("; ") }));
    Ok(FeedReport { firings: firings.len(), none, weak, conflicts, text: out })
}
