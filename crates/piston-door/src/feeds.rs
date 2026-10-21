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
    /// Regions that are the only feeder of a firing but see no change 2 gt before it (on that
    /// edge): no static wiring can start them.
    pub cannot_start: Vec<String>,
    /// `--strict`: why the plan fails the strict screen (empty: it passes): no firing without a
    /// feed, none with flagged options only (a conductor that moves counting as a flag), no
    /// one-cell conflict, no region that cannot start.
    pub strict_fail: Vec<String>,
    pub text: Vec<String>,
}

/// `door feeds`: print [`analyze`]'s table for one schedule file; `strict`: also the strict
/// screen's verdict, an error when it fails (a screen for searches' exports).
pub fn cli(path: &std::path::Path, bx: Option<(i32, i32, i32, i32)>, sculk: bool, strict: bool) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let r = analyze_full(&text, &path.display().to_string(), bx, sculk, strict)?;
    for l in &r.text {
        println!("{l}");
    }
    if strict && !r.strict_fail.is_empty() {
        bail!("{}: fails the strict feed screen: {}", path.display(), r.strict_fail.join("; "));
    }
    Ok(())
}

/// The feed table of an ideal schedule (its text; `label` names it in the header).
pub fn analyze(text: &str, label: &str, bx: Option<(i32, i32, i32, i32)>) -> Result<FeedReport> {
    analyze_opts(text, label, bx, false)
}

/// [`analyze`], plus (`sculk`) what a sculk sensor in each free cell would hear.
pub fn analyze_opts(text: &str, label: &str, bx: Option<(i32, i32, i32, i32)>, sculk: bool) -> Result<FeedReport> {
    analyze_full(text, label, bx, sculk, false)
}

/// [`analyze_opts`], plus (`strict`) the strict screen: a conductor a feed goes through must stay
/// the same block all cycle (dust on a block that moves pops; a component's power through it is
/// lost when it leaves), flagged `*`; the verdict in [`FeedReport::strict_fail`].
pub fn analyze_full(text: &str, label: &str, bx: Option<(i32, i32, i32, i32)>, sculk: bool, strict: bool) -> Result<FeedReport> {
    let s = Schedule::parse_with(text, &BTreeMap::new())?;
    analyze_schedule_full(&s, label, bx, sculk, strict)
}

/// [`analyze`] on a parsed (or built) schedule (c70b: mechmut ranks its variants by it).
pub fn analyze_schedule(s: &Schedule, label: &str, bx: Option<(i32, i32, i32, i32)>) -> Result<FeedReport> {
    analyze_schedule_full(s, label, bx, false, false)
}

/// [`analyze_full`] on a parsed schedule.
pub fn analyze_schedule_full(s: &Schedule, label: &str, bx: Option<(i32, i32, i32, i32)>, sculk: bool, strict: bool) -> Result<FeedReport> {
    let mut out: Vec<String> = Vec::new();
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
    // the lever: a redstone block the design holds at rest (the plan clears and sets it)
    let lever_cells: BTreeSet<Pos> = blocks.iter().filter(|(_, b)| b.to_string().trim_start_matches("minecraft:") == "redstone_block").map(|(p, _)| *p).collect();
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
    // per edge, the vibrations (game events sculk sensors hear) with their tick: inputs applied
    // between ticks t and t+1 and events of tick t both stamp t
    let mut vib: [Vec<(u32, &'static str, Pos)>; 2] = [Vec::new(), Vec::new()];
    let mut seen_ev = w.game_events.len();
    let mut firings: Vec<Firing> = Vec::new();
    for (open, evs) in [(true, &open_ev), (false, &close_ev)] {
        let es = if open { 0 } else { 1 };
        edge_snaps[es].push(snap(&w, &held));
        let (mut t, mut i) = (0u32, 0usize);
        let take = |w: &World, seen: &mut usize, t: u32, out: &mut Vec<(u32, &'static str, Pos)>| {
            for (name, p) in &w.game_events[*seen..] {
                out.push((t, *name, [p.x, p.y, p.z]));
            }
            *seen = w.game_events.len();
        };
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
            take(&w, &mut seen_ev, t, &mut vib[es]);
            if i >= evs.len() {
                break;
            }
            w.tick();
            t += 1;
            take(&w, &mut seen_ev, t, &mut vib[es]);
            let s = snap(&w, &held);
            all.push(s.clone());
            edge_snaps[es].push(s);
        }
        let es_v = &mut edge_snaps[es];
        let vib_v = &mut vib[es];
        let mut tt = t;
        let (_, idle) = cs::step_until_idle(&mut w, 300, |w| {
            tt += 1;
            take(w, &mut seen_ev, tt, vib_v);
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
    let mut letter = b'A';
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
        let l = letter as char;
        letter = if letter == b'Z' { b'a' } else { letter + 1 };
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
    // each region's cells: an area's load (how many distinct ticks it must make, from how many
    // cells) needs the cells themselves, not only their count (clk-item, 805d13559)
    for l in regions.keys() {
        let cells: Vec<String> = region.iter().filter(|(_, (rl, _))| rl == l).map(|(c, _)| format!("({},{})", c.0, c.1)).collect();
        out.push(format!("# region {l}: {}", cells.join(" ")));
    }
    out.push("# cells: (x,y) then the free region and its size; via N<-M = conductor block N strongly powered from free cell M".to_string());
    out.push("# flags: ! = the cell an observer/repeater there would watch or take input from (opposite its target) is outside the box; ? = that cell is a block that never changes (only dust or a torch could drive it)".to_string());
    if strict {
        out.push("# strict: * = the conductor of a `via` option moves during the cycle (dust on it pops; a component's power through it leaves with it): a flag".to_string());
    }
    out.push("# timing of a clean option (an observer there): @ = the cell it watches changes the tick before the firing (fires with the plan), ~ = it changes only at other ticks; none = a free cell (other wiring drives it)".to_string());
    let mut none: Vec<String> = Vec::new();
    let mut weak: Vec<String> = Vec::new();
    // cell -> (targets, firing) of the firings whose clean options are all in that cell
    let mut sole: BTreeMap<P2, Vec<(BTreeSet<P2>, String)>> = BTreeMap::new();
    // (edge, tick, region, firing) of firings only one region can feed
    let mut sole_region: Vec<(usize, u32, char, String)> = Vec::new();
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
            let mut feeders: BTreeSet<char> = BTreeSet::new();
            for (is_qc, text, m, tg) in opts {
                if let Some((l, _)) = region.get(&m) {
                    feeders.insert(*l);
                }
                // --strict: a `via` option's conductor (its target is neither X nor A) must be the
                // same block all cycle
                let moves = strict && tg != x && tg != above && !all.iter().all(|s| s.get(&tg).is_some() && s.get(&tg) == all[0].get(&tg));
                let fl = if moves { "*" } else { drive(m, tg) };
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
            // only one region can feed this firing (and no fixed observer does): it must have seen
            // something change before
            if obs.is_empty() && feeders.len() == 1 && !lever_cells.contains(&f.input) {
                sole_region.push((e, f.t, *feeders.iter().next().unwrap(), label.clone()));
            }
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
    // regions: the cells next to each (all an observer there can watch) and the first tick one of
    // them changes on each edge; a region that is the only feeder of a firing must see a change at
    // least 2 gt before it (an observer's delay), else nothing can start it (no electrical path
    // crosses a piston, a moving block or two blocks)
    let mut letters: Vec<char> = regions.keys().copied().collect();
    letters.sort();
    let mut cannot_start: Vec<String> = Vec::new();
    for l in letters {
        let cells: Vec<P2> = region.iter().filter(|(_, (r, _))| *r == l).map(|(c, _)| *c).collect();
        let adj: BTreeSet<P2> = cells.iter().flat_map(|c| nbrs(*c)).filter(|n| in_box(*n) && !free.contains(n)).collect();
        let first = |e: usize| -> Option<usize> {
            let s = &edge_snaps[e];
            (1..s.len()).find(|&t| adj.iter().any(|c| s[t].get(c) != s[t - 1].get(c)))
        };
        let (fo, fc) = (first(0), first(1));
        let fmt = |v: Option<usize>| v.map(|t| format!("t{t}")).unwrap_or_else(|| "never".into());
        out.push(format!("# region {l}{}: watches {}; first change open {} close {}", cells.len(), adj.iter().map(|c| format!("({},{})", c.0, c.1)).collect::<Vec<_>>().join(" "), fmt(fo), fmt(fc)));
        for e in 0..2 {
            let first_c = if e == 0 { fo } else { fc };
            if let Some((_, t, _, label)) = sole_region.iter().filter(|(ee, _, r, _)| *ee == e && *r == l).min_by_key(|(_, t, _, _)| *t)
                && first_c.is_none_or(|c| c + 2 > *t as usize)
            {
                cannot_start.push(format!("{l} ({label}: first change {})", fmt(first_c)));
            }
        }
    }
    out.push(format!("# regions that cannot start (only feeder of a firing, nothing seen 2 gt before): {}", if cannot_start.is_empty() { "none".to_string() } else { cannot_start.join(", ") }));
    // what a sculk sensor in each free cell would hear (on this plan's motion, the sensor itself
    // changing nothing): `VibrationSystem` as `redsim::sculk` ports it. Idle (INACTIVE, nothing in
    // flight), the first tick T with an event within the radius (8; calibrated 16) commits it to
    // that tick's closest event (ties: higher frequency); it activates at T + max(1, floor(d)) in
    // the block-entity phase (adjacent pistons move from the next tick), stays ACTIVE 30 gt
    // (calibrated 10), COOLDOWN 10 gt, and hears nothing from T on until INACTIVE again.
    if sculk {
        out.push("# sculk: per free cell, the activations of a sensor there (S normal: radius 8, active 30; C calibrated, no back signal: radius 16, active 10): tA<(x,y) = active from tick A, woken by the event at (x,y)".to_string());
        for c in &free {
            let mut parts: Vec<String> = Vec::new();
            for (name, r, active) in [("S", 8i32, 30u32), ("C", 16, 10)] {
                for (e, edge) in [(0usize, "open"), (1, "close")] {
                    let mut evs: Vec<(u32, f64, u8, Pos)> = vib[e]
                        .iter()
                        .filter_map(|(t, ev, p)| {
                            let f = redsim::sculk::frequency(ev);
                            let (dx, dy, dz) = (p[0] - c.0, p[1] - c.1, p[2] - z);
                            let d2 = dx * dx + dy * dy + dz * dz;
                            (f > 0 && d2 <= r * r && d2 > 0).then(|| (*t, (d2 as f64).sqrt(), f, *p))
                        })
                        .collect();
                    evs.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.partial_cmp(&b.1).unwrap()).then(b.2.cmp(&a.2)));
                    let mut inactive_from = 0u32;
                    let mut acts: Vec<String> = Vec::new();
                    for (t, d, _, p) in &evs {
                        if *t < inactive_from {
                            continue;
                        }
                        // the first event of this tick in sorted order is its closest one
                        let a = t + (d.floor() as u32).max(1);
                        acts.push(format!("t{a}<({},{}{})", p[0], p[1], if p[2] != z { format!(",{}", p[2]) } else { String::new() }));
                        inactive_from = a + active + 10;
                    }
                    parts.push(format!("{name} {edge} {}", if acts.is_empty() { "-".to_string() } else { acts.join(" ") }));
                }
            }
            out.push(format!("# sculk {}: {}", cell(*c), parts.join("; ")));
        }
    }
    out.push(format!("# one cell, several targets (its one component cannot feed them all): {}", if conflicts.is_empty() { "none".to_string() } else { conflicts.join("; ") }));
    let mut strict_fail: Vec<String> = Vec::new();
    if strict {
        if !none.is_empty() {
            strict_fail.push(format!("{} firing(s) without a feed ({})", none.len(), none.join(", ")));
        }
        if !weak.is_empty() {
            strict_fail.push(format!("{} firing(s) with flagged options only ({})", weak.len(), weak.join(", ")));
        }
        if !conflicts.is_empty() {
            strict_fail.push(format!("{} one-cell conflict(s) ({})", conflicts.len(), conflicts.join("; ")));
        }
        if !cannot_start.is_empty() {
            strict_fail.push(format!("{} region start(s) impossible ({})", cannot_start.len(), cannot_start.join(", ")));
        }
        out.push(if strict_fail.is_empty() {
            format!("# strict: PASS ({} firings, every one with a clean feed, no conflict, every sole region startable)", firings.len())
        } else {
            format!("# strict: FAIL: {}", strict_fail.join("; "))
        });
    }
    Ok(FeedReport { firings: firings.len(), none, weak, conflicts, cannot_start, strict_fail, text: out })
}
