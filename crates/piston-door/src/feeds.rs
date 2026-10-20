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

pub fn cli(path: &std::path::Path, bx: Option<(i32, i32, i32, i32)>) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let s = Schedule::parse_with(&text, &BTreeMap::new())?;
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
    let mut firings: Vec<Firing> = Vec::new();
    for (open, evs) in [(true, &open_ev), (false, &close_ev)] {
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
            all.push(snap(&w, &held));
        }
        let (_, idle) = cs::step_until_idle(&mut w, 300, |w| all.push(snap(w, &held)));
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
    let regions: BTreeMap<char, usize> = region.values().map(|(l, n)| (*l, *n)).collect();
    println!("# door feeds {} (box x {x0}..{x1}, y {y0}..{y1}; {} firings)", path.display(), firings.len());
    println!("# free all cycle: {} cells in {} regions: {}", free.len(), regions.len(), regions.iter().map(|(l, n)| format!("{l}{n}")).collect::<Vec<_>>().join(" "));
    println!("# cells: (x,y) then the free region and its size; via N<-M = conductor block N strongly powered from free cell M");
    let mut none: Vec<String> = Vec::new();
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
            for n in nbrs(x) {
                if Some(n) == front {
                    continue;
                }
                if free.contains(&n) {
                    direct.push(cell(n));
                } else if conductor(n) {
                    for m in nbrs(n) {
                        if m != x && free.contains(&m) {
                            direct.push(format!("via ({},{})<-{}", n.0, n.1, cell(m)));
                        }
                    }
                }
            }
            let mut qc: Vec<String> = Vec::new();
            for q in [(above.0, above.1 + 1), (above.0 - 1, above.1), (above.0 + 1, above.1)] {
                if free.contains(&q) {
                    qc.push(cell(q));
                } else if conductor(q) {
                    for m in nbrs(q) {
                        if m != above && free.contains(&m) {
                            qc.push(format!("via ({},{})<-{}", q.0, q.1, cell(m)));
                        }
                    }
                }
            }
            let how = if direct_i { "direct" } else { "qc" };
            let own = if in_plane && free.contains(&i2) { ", the plan's input cell is free" } else { "" };
            let verdict = if direct.is_empty() && qc.is_empty() && obs.is_empty() { "NONE" } else { "feed" };
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
            if verdict == "NONE" {
                none.push(format!("{} t{} {}({},{})", if f.open { "open" } else { "close" }, f.t, short(b), x.0, x.1));
            }
        }
        let edge = if f.open { "open" } else { "close" };
        let at = format!("({},{},{})", f.input[0], f.input[1], f.input[2]);
        if lines.is_empty() {
            println!("{edge} t{} input {at}: powers no piston in the plane", f.t);
        }
        for l in lines {
            println!("{edge} t{} input {at} -> {l}", f.t);
        }
    }
    println!("# no feed at all: {}", if none.is_empty() { "none".to_string() } else { none.join(", ") });
    Ok(())
}
