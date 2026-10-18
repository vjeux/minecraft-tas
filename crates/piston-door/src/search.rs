//! Local search for fewer blocks: `mctas door minimize --moves ...`.
//!
//! The only oracle is the judge, [`verify_in`]; moves are plain design edits (remove blocks,
//! set blocks), so this works for any contraption the verifier accepts. Move families
//! (`--moves`, a comma list or `all`):
//! * `singles`, `pairs`, `triples`: remove 1, 2 or 3 blocks (triples are local: at least two of
//!   the three distances are within `radius`);
//! * `delays`: another delay on a repeater (neutral);
//! * `moves`: a block moves to a free face-neighbor cell (neutral);
//! * `subs`: a block becomes another component: dust <-> repeater, torch <-> observer, other
//!   facings, glass floors (neutral);
//! * `reroute`: a dust segment between two anchors (branch points, ends, cells touching
//!   components) is ripped up with its floors and re-laid by the router, from anywhere on one
//!   side of the net to the other end (improving or neutral).
//!
//! Neutral edits are also tried together with one removal within `radius` (an improving
//! compound). When nothing improves (`plateau`), every neutral edit that passes the judge is
//! followed by every single (and pair) removal; then, with `walk`, the search moves to a random
//! verified neutral design and starts over (at most `walk` such steps without improving).
//!
//! Every candidate is first screened at a few judge positions with fewer cycles (`screen`);
//! survivors get the full judge, the best one is applied, and the other survivors are re-tried
//! on top of it.

use crate::minimize::Sandboxes;
use crate::route::Router;
use crate::{DoorSpec, VerifyOptions, block_count, verify_in};
use anyhow::{Context, Result, anyhow};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{Pool, Pos};
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;
use std::time::Instant;

/// The judge's positions: the default four plus `extra` pseudo-random ones (the same sequence
/// as `door verify --extra-offsets`).
pub fn judge_offsets(extra: u32) -> Vec<Pos> {
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra));
    offsets
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    Singles,
    Pairs,
    Triples,
    Delays,
    Moves,
    Subs,
    Reroute,
    Renet,
}

const FAMILIES: [(Family, &str); 8] = [
    (Family::Singles, "singles"),
    (Family::Pairs, "pairs"),
    (Family::Triples, "triples"),
    (Family::Delays, "delays"),
    (Family::Moves, "moves"),
    (Family::Subs, "subs"),
    (Family::Reroute, "reroute"),
    (Family::Renet, "renet"),
];

/// `all` or a comma list of family names.
pub fn parse_families(s: &str) -> Result<Vec<Family>> {
    if s.trim() == "all" {
        return Ok(FAMILIES.iter().map(|(f, _)| *f).collect());
    }
    s.split(',')
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| {
            FAMILIES.iter().find(|(_, n)| *n == x).map(|(f, _)| *f).ok_or_else(|| {
                anyhow!("unknown move family {x:?}; have {} or all", FAMILIES.iter().map(|(_, n)| *n).collect::<Vec<_>>().join(","))
            })
        })
        .collect()
}

pub struct SearchOptions {
    pub families: Vec<Family>,
    /// Cheap first pass (a few positions, fewer cycles).
    pub screen: VerifyOptions,
    /// Acceptance: the judge.
    pub judge: VerifyOptions,
    /// Neighborhood radius (Chebyshev) of compounds and triples.
    pub radius: i32,
    /// When nothing improves, explore verified neutral edits one level deep.
    pub plateau: bool,
    /// ... followed by pair removals too (singles only otherwise).
    pub plateau_pairs: bool,
    /// Random-walk steps over verified neutral designs allowed without improving.
    pub walk: usize,
    pub deadline: Option<Instant>,
    pub seed: u64,
}

impl SearchOptions {
    fn has(&self, f: Family) -> bool {
        self.families.contains(&f)
    }
    fn timed_out(&self) -> bool {
        self.deadline.is_some_and(|d| Instant::now() > d)
    }
}

/// A design edit: blocks removed (an extended piston loses its head too), then blocks set.
#[derive(Clone, Debug)]
pub struct Edit {
    pub remove: Vec<Pos>,
    pub set: Vec<(Pos, String)>,
    pub desc: String,
}

impl Edit {
    fn removal(ps: &[Pos], d: &Design) -> Edit {
        let desc = ps.iter().map(|p| format!("-{} {}", fmt_pos(*p), short(&d.blocks[p]))).collect::<Vec<_>>().join(" ");
        Edit { remove: ps.to_vec(), set: Vec::new(), desc }
    }

    pub fn apply(&self, d: &Design) -> Design {
        let mut out = d.clone();
        for p in &self.remove {
            remove_block(&mut out, *p);
        }
        for (p, s) in &self.set {
            out.blocks.insert(*p, s.clone());
        }
        out
    }

    fn cells(&self) -> Vec<Pos> {
        self.remove.iter().copied().chain(self.set.iter().map(|(p, _)| *p)).collect()
    }
}

const WIRE: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const HORIZ: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const HORIZONTAL: [&str; 4] = ["north", "east", "south", "west"];
const ALL_DIRS: [&str; 6] = ["north", "east", "south", "west", "up", "down"];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cheb(a: Pos, b: Pos) -> i32 {
    (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max((a[2] - b[2]).abs())
}
fn fmt_pos(p: Pos) -> String {
    format!("({},{},{})", p[0], p[1], p[2])
}
fn dir(f: &str) -> Option<Pos> {
    Some(match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}
fn opposite(f: &str) -> &'static str {
    match f {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        "up" => "down",
        _ => "up",
    }
}
fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}
/// Block id without namespace or properties.
fn kind(s: &str) -> &str {
    short(mctas::blockstate::block_name(s))
}
/// Can hold dust or a torch (roughly: a full block).
fn is_support(s: &str) -> bool {
    !matches!(
        kind(s),
        "redstone_wire" | "repeater" | "comparator" | "redstone_torch" | "redstone_wall_torch" | "lever" | "piston_head" | "moving_piston"
    )
}
/// Plain building blocks: no behavior of their own besides holding things up and conducting.
fn is_plain(s: &str) -> bool {
    matches!(kind(s), "smooth_stone" | "stone" | "glass" | "cobblestone" | "stone_bricks" | "dirt")
}
/// Approximation of `isRedstoneConductor` (cuts dust steps).
fn is_conductor(s: &str) -> bool {
    is_support(s) && !matches!(kind(s), "glass" | "observer" | "piston" | "sticky_piston" | "slime_block" | "honey_block" | "redstone_block")
}
fn movable(k: &str) -> bool {
    !matches!(k, "piston" | "sticky_piston" | "slime_block" | "honey_block" | "piston_head" | "moving_piston" | "lever")
}

/// Remove the block at `p` in place; an extended piston loses its head.
fn remove_block(d: &mut Design, p: Pos) {
    let Some(s) = d.blocks.remove(&p) else { return };
    let Ok(b) = BlockState::parse(&s) else { return };
    if b.get("extended") == Some("true") {
        if let Some(v) = b.get("facing").and_then(dir) {
            let h = add(p, v);
            if d.blocks.get(&h).is_some_and(|s| s.starts_with("minecraft:piston_head")) {
                d.blocks.remove(&h);
            }
        }
    }
}

pub fn design_hash(d: &Design) -> u64 {
    let mut h = DefaultHasher::new();
    d.blocks.hash(&mut h);
    h.finish()
}

/// Where a wall-mounted block hangs from, if it is one.
fn attachment(p: Pos, s: &str) -> Option<Pos> {
    let b = BlockState::parse(s).ok()?;
    match b.short_name() {
        "redstone_wall_torch" => Some(sub(p, dir(b.get("facing")?)?)),
        "redstone_torch" | "redstone_wire" | "repeater" | "comparator" => Some(add(p, [0, -1, 0])),
        "lever" => match b.get("face") {
            Some("floor") => Some(add(p, [0, -1, 0])),
            Some("ceiling") => Some(add(p, [0, 1, 0])),
            _ => Some(sub(p, dir(b.get("facing")?)?)),
        },
        _ => None,
    }
}

/// Something hangs from or stands on `p`.
fn supports_something(d: &Design, p: Pos) -> bool {
    FACES.iter().any(|f| {
        let n = add(p, *f);
        d.blocks.get(&n).is_some_and(|s| attachment(n, s) == Some(p))
    })
}

/// `s` at `q` would stand (its attachment block exists in `d`).
fn support_ok(d: &Design, q: Pos, s: &str) -> bool {
    match attachment(q, s) {
        Some(a) => d.blocks.get(&a).is_some_and(|x| is_support(x)),
        None => true,
    }
}

/// What may not be touched: the door cells, the lever, piston heads; and where nothing may be
/// placed: the door cells and the walkway.
struct Ctx {
    keep: HashSet<Pos>,
    forbidden: HashSet<Pos>,
    spec: DoorSpec,
}

impl Ctx {
    fn new(d: &Design) -> Result<Ctx> {
        let spec = DoorSpec::from_design(d)?;
        let mut keep: HashSet<Pos> = spec.door_cells().into_iter().collect();
        keep.insert(spec.lever);
        for (p, s) in &d.blocks {
            if s.starts_with("minecraft:piston_head") {
                keep.insert(*p);
            }
        }
        let forbidden = spec.door_cells().into_iter().chain(spec.corridor_cells()).collect();
        Ok(Ctx { keep, forbidden, spec })
    }

    fn removable(&self, d: &Design) -> Vec<Pos> {
        d.blocks.keys().filter(|p| !self.keep.contains(*p)).copied().collect()
    }
}

/// Removal candidates: singles, pairs, local triples.
fn removals(ctx: &Ctx, d: &Design, singles: bool, pairs: bool, triples: bool, radius: i32) -> Vec<Edit> {
    let r = ctx.removable(d);
    let mut out = Vec::new();
    if singles {
        for &p in &r {
            out.push(Edit::removal(&[p], d));
        }
    }
    if pairs {
        for i in 0..r.len() {
            for j in i + 1..r.len() {
                out.push(Edit::removal(&[r[i], r[j]], d));
            }
        }
    }
    if triples {
        for i in 0..r.len() {
            for j in i + 1..r.len() {
                let ab = cheb(r[i], r[j]) <= radius;
                for k in j + 1..r.len() {
                    let near = ab as u8 + (cheb(r[i], r[k]) <= radius) as u8 + (cheb(r[j], r[k]) <= radius) as u8;
                    if near >= 2 {
                        out.push(Edit::removal(&[r[i], r[j], r[k]], d));
                    }
                }
            }
        }
    }
    out
}

/// Other components that could stand at `p` instead of `b`.
fn substitutes(d: &Design, p: Pos, b: &BlockState) -> Vec<String> {
    let mut out = Vec::new();
    let has_floor = d.blocks.get(&add(p, [0, -1, 0])).is_some_and(|s| is_support(s));
    let wall_ok = |f: &str| d.blocks.get(&sub(p, dir(f).unwrap())).is_some_and(|s| is_support(s));
    let torches = |out: &mut Vec<String>| {
        if has_floor {
            out.push("minecraft:redstone_torch[lit=true]".to_string());
        }
        for f in HORIZONTAL {
            if wall_ok(f) {
                out.push(format!("minecraft:redstone_wall_torch[facing={f},lit=true]"));
            }
        }
    };
    let observers = |out: &mut Vec<String>| {
        for f in ALL_DIRS {
            out.push(format!("minecraft:observer[facing={f},powered=false]"));
        }
    };
    match b.short_name() {
        "redstone_wire" => {
            for f in HORIZONTAL {
                out.push(format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]"));
            }
        }
        "repeater" => {
            out.push(WIRE.to_string());
            if let Some(f) = b.get("facing") {
                out.push(b.with("facing", opposite(f)).to_string());
            }
        }
        "redstone_wall_torch" | "redstone_torch" | "observer" => {
            observers(&mut out);
            torches(&mut out);
        }
        "smooth_stone" | "stone" => out.push("minecraft:glass".to_string()),
        "glass" => out.push("minecraft:smooth_stone".to_string()),
        _ => {}
    }
    let me = b.to_string();
    out.retain(|s| BlockState::parse(s).map(|x| x.to_string()).ok().as_deref() != Some(me.as_str()));
    out
}

/// Count-neutral edits: repeater delays, substitutions, moves to a neighbor cell.
fn neutral_edits(ctx: &Ctx, d: &Design, opts: &SearchOptions) -> Vec<Edit> {
    let mut out = Vec::new();
    for (&p, s) in &d.blocks {
        if ctx.keep.contains(&p) {
            continue;
        }
        let Ok(b) = BlockState::parse(s) else { continue };
        let k = b.short_name().to_string();
        if opts.has(Family::Delays) && k == "repeater" {
            let cur = b.get("delay").unwrap_or("1").to_string();
            for dl in ["1", "2", "3", "4"] {
                if dl != cur {
                    out.push(Edit { remove: vec![], set: vec![(p, b.with("delay", dl).to_string())], desc: format!("{} delay {cur}->{dl}", fmt_pos(p)) });
                }
            }
        }
        if opts.has(Family::Subs) {
            for alt in substitutes(d, p, &b) {
                out.push(Edit { remove: vec![], set: vec![(p, alt.clone())], desc: format!("{} {} => {}", fmt_pos(p), short(s), short(&alt)) });
            }
        }
        if opts.has(Family::Moves) && movable(&k) && !supports_something(d, p) {
            for f in FACES {
                let q = add(p, f);
                if d.blocks.contains_key(&q) || ctx.forbidden.contains(&q) {
                    continue;
                }
                let state = if k == "redstone_wire" { WIRE.to_string() } else { s.clone() };
                let mut after = d.clone();
                after.blocks.remove(&p);
                after.blocks.insert(q, state.clone());
                if !support_ok(&after, q, &state) {
                    continue;
                }
                out.push(Edit { remove: vec![p], set: vec![(q, state)], desc: format!("move {} {} -> {}", fmt_pos(p), short(s), fmt_pos(q)) });
            }
        }
    }
    out
}

/// Each neutral edit plus one removal within `radius` of the cells it touches.
fn compounds(ctx: &Ctx, d: &Design, neutral: &[Edit], radius: i32) -> Vec<Edit> {
    let r = ctx.removable(d);
    let mut out = Vec::new();
    for e in neutral {
        let cells = e.cells();
        for &q in &r {
            if cells.contains(&q) || cells.iter().all(|c| cheb(*c, q) > radius) {
                continue;
            }
            let mut e2 = e.clone();
            e2.remove.push(q);
            e2.desc = format!("{} -{} {}", e.desc, fmt_pos(q), short(&d.blocks[&q]));
            out.push(e2);
        }
    }
    out
}

/// Dust cells and their connections (same level, or a step up/down with clearance).
fn dust_graph(d: &Design) -> HashMap<Pos, Vec<Pos>> {
    let dust: HashSet<Pos> = d.blocks.iter().filter(|(_, s)| kind(s) == "redstone_wire").map(|(p, _)| *p).collect();
    let conductor = |p: Pos| d.blocks.get(&p).is_some_and(|s| is_conductor(s));
    let mut g = HashMap::new();
    for &c in &dust {
        let mut ns = Vec::new();
        for h in HORIZ {
            let n = add(c, h);
            if dust.contains(&n) {
                ns.push(n);
            }
            let up = add(n, [0, 1, 0]);
            if dust.contains(&up) && !conductor(add(c, [0, 1, 0])) {
                ns.push(up);
            }
            let down = add(n, [0, -1, 0]);
            if dust.contains(&down) && !conductor(n) {
                ns.push(down);
            }
        }
        g.insert(c, ns);
    }
    g
}

/// A block that does something (not plain, not dust).
fn is_component(d: &Design, p: Pos) -> bool {
    d.blocks.get(&p).is_some_and(|s| !is_plain(s) && kind(s) != "redstone_wire")
}

/// Dust that interacts with something besides dust: its floor is not plain, it touches a
/// component, or a plain block it touches (its floor included) touches a component (dust powers
/// the block below it and the blocks it points into).
fn is_terminal(d: &Design, c: Pos) -> bool {
    let floor = add(c, [0, -1, 0]);
    if !d.blocks.get(&floor).is_some_and(|s| is_plain(s)) {
        return true;
    }
    FACES.iter().any(|f| {
        let n = add(c, *f);
        if is_component(d, n) {
            return true;
        }
        d.blocks.get(&n).is_some_and(|s| is_plain(s)) && FACES.iter().any(|g| add(n, *g) != c && is_component(d, add(n, *g)))
    })
}

/// Dust segments: (anchor, inner cells in order, anchor). Inner cells have two connections and
/// are plain wiring by one of two rules: loose (no component next to the dust, a plain floor
/// touching no component) or strict (not a terminal).
fn dust_segments(d: &Design, g: &HashMap<Pos, Vec<Pos>>) -> Vec<(Pos, Vec<Pos>, Pos)> {
    let loose = |c: Pos| -> bool {
        if g[&c].len() != 2 {
            return false;
        }
        let floor = add(c, [0, -1, 0]);
        if !d.blocks.get(&floor).is_some_and(|s| is_plain(s)) {
            return false;
        }
        if FACES.iter().any(|f| *f != [0, -1, 0] && is_component(d, add(c, *f))) {
            return false;
        }
        !FACES.iter().any(|f| add(floor, *f) != c && is_component(d, add(floor, *f)))
    };
    let strict = |c: Pos| -> bool { g[&c].len() == 2 && !is_terminal(d, c) };
    let mut seen: HashSet<Vec<Pos>> = HashSet::new();
    let mut out = Vec::new();
    let rules: [&dyn Fn(Pos) -> bool; 2] = [&loose, &strict];
    let mut anchors: Vec<Pos> = g.keys().copied().collect();
    anchors.sort();
    for inner in rules {
        for &a in &anchors {
            if inner(a) {
                continue;
            }
            for &n in &g[&a] {
                if !inner(n) {
                    continue;
                }
                let mut path = vec![n];
                let (mut prev, mut cur) = (a, n);
                let end = loop {
                    let next = g[&cur].iter().copied().find(|x| *x != prev);
                    let Some(next) = next else { break None };
                    if !inner(next) {
                        break Some(next);
                    }
                    if path.contains(&next) {
                        break None;
                    }
                    path.push(next);
                    prev = cur;
                    cur = next;
                };
                let Some(b) = end else { continue };
                let mut key = path.clone();
                key.sort();
                if seen.insert(key) {
                    out.push((a, path, b));
                }
            }
        }
    }
    out
}

fn router_box(d: &Design) -> (Pos, Pos) {
    let (lo, hi) = d.bounds().unwrap();
    ([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3])
}

/// Cells new wiring may not use: what the machine sweeps while it runs, the door and walkway.
fn reserved_cells(ctx: &Ctx, d: &Design, swept: &HashSet<Pos>) -> HashSet<Pos> {
    let mut reserved: HashSet<Pos> = swept.iter().filter(|p| !d.blocks.contains_key(*p)).copied().collect();
    reserved.extend(ctx.forbidden.iter().copied());
    reserved
}

/// Sort an edit into improving / neutral by its block-count change; drops no-ops and repeats.
fn classify(d: &Design, mut e: Edit, seen: &mut HashSet<u64>, better: &mut Vec<Edit>, neutral: &mut Vec<Edit>) {
    let new = e.apply(d);
    if new.blocks == d.blocks || !seen.insert(design_hash(&new)) {
        return;
    }
    let delta = block_count(&new) as i64 - block_count(d) as i64;
    e.desc = format!("{} [{delta:+}]", e.desc);
    match delta {
        x if x < 0 => better.push(e),
        0 => neutral.push(e),
        _ => {}
    }
}

/// Dust nets: the connected components of the dust graph.
fn nets(g: &HashMap<Pos, Vec<Pos>>) -> Vec<Vec<Pos>> {
    let mut done: HashSet<Pos> = HashSet::new();
    let mut cells: Vec<Pos> = g.keys().copied().collect();
    cells.sort();
    let mut out = Vec::new();
    for &c0 in &cells {
        if !done.insert(c0) {
            continue;
        }
        let mut net = vec![c0];
        let mut q = VecDeque::from([c0]);
        while let Some(c) = q.pop_front() {
            for n in &g[&c] {
                if done.insert(*n) {
                    net.push(*n);
                    q.push_back(*n);
                }
            }
        }
        out.push(net);
    }
    out
}

/// The edit that turns `d` into `new`.
fn diff_edit(d: &Design, new: &Design, desc: String) -> Edit {
    let remove = d.blocks.keys().filter(|p| !new.blocks.contains_key(*p)).copied().collect();
    let set = new.blocks.iter().filter(|(p, s)| d.blocks.get(*p) != Some(*s)).map(|(p, s)| (*p, s.clone())).collect();
    Edit { remove, set, desc }
}

/// Net re-synthesis candidates: (improving, neutral). For each dust net with 2+ terminals: the
/// terminals reconnected from scratch ([`resynth`]), the same with one terminal dropped, and for
/// each repeater, the repeater turned into dust and its merged net re-synthesized.
fn renets(ctx: &Ctx, d: &Design, swept: &HashSet<Pos>) -> (Vec<Edit>, Vec<Edit>) {
    let reserved = reserved_cells(ctx, d, swept);
    let (mut better, mut neutral) = (Vec::new(), Vec::new());
    let mut seen: HashSet<u64> = HashSet::new();
    let g = dust_graph(d);
    for net in nets(&g) {
        let terminals: Vec<Pos> = net.iter().copied().filter(|c| is_terminal(d, *c)).collect();
        if terminals.len() < 2 {
            continue;
        }
        let mut sets = vec![(terminals.clone(), String::new())];
        for t in &terminals {
            sets.push((terminals.iter().copied().filter(|x| x != t).collect(), format!(" without {}", fmt_pos(*t))));
        }
        for (ts, note) in sets {
            for (new, desc) in resynth(d, &g, &net, &ts, &reserved) {
                classify(d, diff_edit(d, &new, format!("{desc}{note}")), &mut seen, &mut better, &mut neutral);
            }
        }
    }
    for (&p, s) in &d.blocks {
        if kind(s) != "repeater" || ctx.keep.contains(&p) {
            continue;
        }
        let mut d2 = d.clone();
        d2.blocks.insert(p, WIRE.to_string());
        let g2 = dust_graph(&d2);
        let Some(net) = nets(&g2).into_iter().find(|n| n.contains(&p)) else { continue };
        let terminals: Vec<Pos> = net.iter().copied().filter(|c| is_terminal(&d2, *c)).collect();
        if terminals.len() < 2 {
            continue;
        }
        for (new, desc) in resynth(&d2, &g2, &net, &terminals, &reserved) {
            classify(d, diff_edit(d, &new, format!("repeater {} to dust, {desc}", fmt_pos(p))), &mut seen, &mut better, &mut neutral);
        }
    }
    (better, neutral)
}

/// Rip every cell of `net` but `terminals` (with the floors of plain wiring) and reconnect the
/// terminals with the router as a greedy Steiner tree, once from each terminal as the root
/// (each step routes the tree to the cheapest remaining terminal).
fn resynth(d: &Design, g: &HashMap<Pos, Vec<Pos>>, net: &[Pos], terminals: &[Pos], reserved: &HashSet<Pos>) -> Vec<(Design, String)> {
    let (lo, hi) = router_box(d);
    let tset: HashSet<Pos> = terminals.iter().copied().collect();
    let mut rip = Vec::new();
    for &c in net {
        if tset.contains(&c) {
            continue;
        }
        rip.push(c);
        if !is_terminal(d, c) {
            rip.push(add(c, [0, -1, 0]));
        }
    }
    let mut out = Vec::new();
    if rip.is_empty() {
        return out;
    }
    let wiring = net.len() - terminals.len();
    {
        let mut after = d.clone();
        for p in &rip {
            after.blocks.remove(p);
        }
        let mut base_allow: HashSet<Pos> = HashSet::new();
        for c in net {
            for f in FACES {
                base_allow.insert(add(*c, f));
                base_allow.insert(add(add(*c, [0, -1, 0]), f));
            }
        }
        for p in &rip {
            base_allow.remove(p);
        }
        // Terminals still connected to `t` through terminals.
        let group = |t: Pos| -> Vec<Pos> {
            let mut out = vec![t];
            let mut q = VecDeque::from([t]);
            let mut seen: HashSet<Pos> = [t].into_iter().collect();
            while let Some(c) = q.pop_front() {
                for n in &g[&c] {
                    if tset.contains(n) && seen.insert(*n) {
                        out.push(*n);
                        q.push_back(*n);
                    }
                }
            }
            out
        };
        for &root in terminals {
            let mut blocks = after.blocks.clone();
            let mut tree = group(root);
            let mut remaining: Vec<Pos> = terminals.iter().copied().filter(|t| !tree.contains(t)).collect();
            let mut added: Vec<(Pos, String)> = Vec::new();
            let mut ok = true;
            while !remaining.is_empty() {
                let mut best: Option<(usize, Vec<(Pos, String)>, usize)> = None;
                for (i, &t) in remaining.iter().enumerate() {
                    let mut bl = blocks.clone();
                    bl.remove(&t);
                    let mut allow = base_allow.clone();
                    allow.extend(tree.iter().copied());
                    let r = Router { blocks: &bl, reserved: reserved.clone(), allow, lo, hi };
                    if let Some(path) = r.route(&tree, t) {
                        let cost = path.iter().filter(|(p, _)| *p != t).count();
                        if best.as_ref().is_none_or(|b| cost < b.2) {
                            best = Some((i, path, cost));
                        }
                    }
                }
                let Some((i, path, _)) = best else {
                    ok = false;
                    break;
                };
                let t = remaining.swap_remove(i);
                for (p, s) in path {
                    if p == t {
                        continue;
                    }
                    let s = if kind(&s) == "redstone_wire" {
                        tree.push(p);
                        WIRE.to_string()
                    } else {
                        s
                    };
                    blocks.insert(p, s.clone());
                    added.push((p, s));
                }
                for x in group(t) {
                    if !tree.contains(&x) {
                        tree.push(x);
                    }
                }
                remaining.retain(|x| !tree.contains(x));
            }
            if !ok {
                continue;
            }
            let mut new = after.clone();
            for (p, s) in added {
                new.blocks.insert(p, s);
            }
            out.push((new, format!("renet from {} ({} terminals, {wiring} ripped)", fmt_pos(root), terminals.len())));
        }
    }
    out
}

/// Rip-up-and-reroute candidates: (improving, neutral).
fn reroutes(ctx: &Ctx, d: &Design, swept: &HashSet<Pos>) -> (Vec<Edit>, Vec<Edit>) {
    let g = dust_graph(d);
    let segs = dust_segments(d, &g);
    let (lo, hi) = router_box(d);
    let reserved = reserved_cells(ctx, d, swept);
    let (mut better, mut neutral) = (Vec::new(), Vec::new());
    let mut seen: HashSet<u64> = HashSet::new();
    for (a, inner, b) in &segs {
        for (keep, goal) in [(*a, *b), (*b, *a)] {
            if ctx.keep.contains(&goal) {
                continue;
            }
            let mut rip: Vec<Pos> = Vec::new();
            for c in inner {
                rip.push(*c);
                rip.push(add(*c, [0, -1, 0]));
            }
            rip.push(goal);
            let mut after = d.clone();
            for p in &rip {
                after.blocks.remove(p);
            }
            // Start: every dust cell still connected to `keep` once the segment is gone.
            let mut start = vec![keep];
            let mut q = VecDeque::from([keep]);
            let mut seen_cells: HashSet<Pos> = [keep].into_iter().collect();
            while let Some(c) = q.pop_front() {
                for n in &g[&c] {
                    if after.blocks.contains_key(n) && seen_cells.insert(*n) {
                        start.push(*n);
                        q.push_back(*n);
                    }
                }
            }
            // The new path may touch what the old segment and the goal touched.
            let mut allow: HashSet<Pos> = HashSet::new();
            for c in inner.iter().chain([&goal]) {
                for f in FACES {
                    allow.insert(add(*c, f));
                    allow.insert(add(add(*c, [0, -1, 0]), f));
                }
            }
            for p in &rip {
                allow.remove(p);
            }
            let router = Router { blocks: &after.blocks, reserved: reserved.clone(), allow, lo, hi };
            let Some(path) = router.route(&start, goal) else { continue };
            let mut e = Edit { remove: rip.clone(), set: Vec::new(), desc: String::new() };
            for (p, s) in &path {
                let s = if kind(s) == "redstone_wire" { WIRE.to_string() } else { s.clone() };
                e.set.push((*p, s));
            }
            e.desc = format!("reroute {} -> {} ({} inner dust, {} new blocks)", fmt_pos(keep), fmt_pos(goal), inner.len(), path.len());
            classify(d, e, &mut seen, &mut better, &mut neutral);
        }
    }
    (better, neutral)
}

/// A candidate: an edit of `bases[base]`.
struct Cand {
    base: usize,
    edit: Edit,
}

/// Router-based candidates (reroute, renet) of each design, (improving, neutral), on threads.
fn routed(opts: &SearchOptions, designs: &[Design], swept: &[HashSet<Pos>]) -> Vec<(Vec<Edit>, Vec<Edit>)> {
    let n = designs.len();
    let results: Mutex<Vec<Option<(Vec<Edit>, Vec<Edit>)>>> = Mutex::new((0..n).map(|_| None).collect());
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..n.min(16) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let d = &designs[i];
                    let (mut b, mut nn) = (Vec::new(), Vec::new());
                    if let Ok(ctx) = Ctx::new(d) {
                        if opts.has(Family::Reroute) {
                            let (x, y) = reroutes(&ctx, d, &swept[i]);
                            b.extend(x);
                            nn.extend(y);
                        }
                        if opts.has(Family::Renet) {
                            let (x, y) = renets(&ctx, d, &swept[i]);
                            b.extend(x);
                            nn.extend(y);
                        }
                    }
                    results.lock().unwrap()[i] = Some((b, nn));
                }
            });
        }
    });
    results.into_inner().unwrap().into_iter().map(|r| r.unwrap_or_default()).collect()
}

enum Screen {
    Pass,
    Fail,
    Skip,
    Error(String),
}

/// Screen then judge `cands`; returns the judge-passing designs, best (fewest blocks) first,
/// with their descriptions and base index. Stops judging after `want` passes (0 = judge all).
fn evaluate(
    pool: &Pool,
    sandboxes: &Sandboxes,
    bases: &[Design],
    cands: Vec<Cand>,
    opts: &SearchOptions,
    failed: &Mutex<HashSet<u64>>,
    want: usize,
    log: &dyn Fn(&str),
) -> Vec<(usize, Design, String)> {
    let t = Instant::now();
    let n = cands.len();
    let batch_seen: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    let results = pool.map((0..n).collect(), |w, wi, i| {
        let c = &cands[i];
        let d = c.edit.apply(&bases[c.base]);
        let h = design_hash(&d);
        if failed.lock().unwrap().contains(&h) || !batch_seen.lock().unwrap().insert(h) {
            return Ok(Screen::Skip);
        }
        let sb = sandboxes.get(w, wi)?;
        match verify_in(w, sb, &d, &opts.screen) {
            Ok(_) => Ok(Screen::Pass),
            Err(e) => {
                let msg = format!("{e:#}");
                if msg.contains("bridge exited") || msg.contains("Broken pipe") {
                    return Ok(Screen::Error(msg));
                }
                failed.lock().unwrap().insert(h);
                Ok(Screen::Fail)
            }
        }
    });
    let mut pass: Vec<usize> = Vec::new();
    let (mut skipped, mut errors) = (0, 0);
    for (i, r) in results.into_iter().enumerate() {
        let err = match r {
            Ok(Screen::Pass) => {
                pass.push(i);
                None
            }
            Ok(Screen::Skip) => {
                skipped += 1;
                None
            }
            Ok(Screen::Fail) => None,
            Ok(Screen::Error(e)) => Some(e),
            Err(e) => Some(format!("{e:#}")),
        };
        if let Some(e) = err {
            if errors == 0 {
                log(&format!("  worker error: {e}"));
            }
            errors += 1;
        }
    }
    let screen_time = t.elapsed().as_secs_f64();
    // Judge the survivors, fewest blocks first, a pool-sized chunk at a time.
    let count = |i: usize| block_count(&cands[i].edit.apply(&bases[cands[i].base]));
    pass.sort_by_key(|&i| count(i));
    let mut verified = Vec::new();
    let chunk = pool.len().max(1);
    let mut judged = 0;
    for part in pass.chunks(chunk) {
        let res = pool.map(part.to_vec(), |w, wi, i| {
            let c = &cands[i];
            let d = c.edit.apply(&bases[c.base]);
            let sb = sandboxes.get(w, wi)?;
            Ok(verify_in(w, sb, &d, &opts.judge).map(|_| d).map_err(|e| format!("{e:#}")))
        });
        judged += part.len();
        for (&i, r) in part.iter().zip(res) {
            match r {
                Ok(Ok(d)) => verified.push((cands[i].base, d, cands[i].edit.desc.clone())),
                Ok(Err(_)) => {
                    let d = cands[i].edit.apply(&bases[cands[i].base]);
                    failed.lock().unwrap().insert(design_hash(&d));
                }
                Err(e) => log(&format!("  judge worker error: {e:#}")),
            }
        }
        if want > 0 && verified.len() >= want {
            break;
        }
        if opts.timed_out() {
            break;
        }
    }
    verified.sort_by_key(|(_, d, _)| block_count(d));
    log(&format!(
        "  {n} candidates: {skipped} seen, {} screened in {screen_time:.1}s, {} passed the screen, {judged} judged, {} passed the judge ({:.1}s){}",
        n - skipped - errors,
        pass.len(),
        verified.len(),
        t.elapsed().as_secs_f64(),
        if errors > 0 { format!(", {errors} worker errors") } else { String::new() }
    ));
    verified
}

fn rand_next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

/// Canonicalize a verified design (true rest states) and keep it if it still passes the judge.
fn finalize(pool: &Pool, sandboxes: &Sandboxes, d: Design, opts: &SearchOptions, log: &dyn Fn(&str)) -> Result<Design> {
    let mut w = pool.worker(0);
    let canon = crate::canonicalize(&mut w, &d)?;
    let sb = sandboxes.get(&mut w, 0)?;
    match verify_in(&mut w, sb, &canon, &opts.judge) {
        Ok(_) => Ok(canon),
        Err(e) => {
            log(&format!("  canonical form fails the judge ({e:#}); keeping the design as edited"));
            Ok(d)
        }
    }
}

/// The local search. `on_improve` gets every accepted (judge-verified) improvement.
pub fn search(
    pool: &Pool,
    start: &Design,
    opts: &SearchOptions,
    log: &dyn Fn(&str),
    on_improve: &mut dyn FnMut(&Design, &str) -> Result<()>,
) -> Result<Design> {
    let sandboxes = Sandboxes::new(pool);
    let mut rng = opts.seed.max(1);
    let mut cur = start.clone();
    {
        let mut w = pool.worker(0);
        let sb = sandboxes.get(&mut w, 0)?;
        verify_in(&mut w, sb, &cur, &opts.judge).context("the start design fails the judge")?;
    }
    log(&format!("start: {} blocks, passes the judge ({} positions x {} cycles)", block_count(&cur), opts.judge.offsets.len(), opts.judge.cycles));
    let failed: Mutex<HashSet<u64>> = Mutex::new(HashSet::new());
    let mut visited: HashSet<u64> = [design_hash(&cur)].into_iter().collect();
    let mut walk_left = opts.walk;
    while !opts.timed_out() {
        let ctx = Ctx::new(&cur)?;
        let neutral = neutral_edits(&ctx, &cur, opts);
        let mut improving =
            removals(&ctx, &cur, opts.has(Family::Singles), opts.has(Family::Pairs), opts.has(Family::Triples), opts.radius);
        let n_removals = improving.len();
        let mut neutral_all = neutral.clone();
        let mut n_reroute = (0, 0);
        if opts.has(Family::Reroute) || opts.has(Family::Renet) {
            let swept = {
                let mut w = pool.worker(0);
                crate::route::swept_cells(&mut w, &cur, &ctx.spec)?
            };
            let tr = Instant::now();
            let (b, n) = routed(opts, std::slice::from_ref(&cur), std::slice::from_ref(&swept)).remove(0);
            n_reroute = (b.len(), n.len());
            improving.extend(b);
            neutral_all.extend(n);
            if tr.elapsed().as_secs_f64() > 5.0 {
                log(&format!("  routing took {:.1}s", tr.elapsed().as_secs_f64()));
            }
        }
        let comp = compounds(&ctx, &cur, &neutral, opts.radius);
        let n_comp = comp.len();
        improving.extend(comp);
        log(&format!(
            "{} blocks: {} removals, {} reroutes (+{} neutral), {} compounds, {} neutral edits",
            block_count(&cur),
            n_removals,
            n_reroute.0,
            n_reroute.1,
            n_comp,
            neutral.len()
        ));
        let bases = vec![cur.clone()];
        let cands: Vec<Cand> = improving.into_iter().map(|edit| Cand { base: 0, edit }).collect();
        let edits: Vec<Edit> = cands.iter().map(|c| c.edit.clone()).collect();
        let verified = evaluate(pool, &sandboxes, &bases, cands, opts, &failed, 1, log);
        if let Some((_, best, desc)) = verified.first().cloned() {
            log(&format!("  accepted: {desc} -> {} blocks", block_count(&best)));
            let mut next = best;
            // Re-try the other judge-passing edits on top.
            for (_, _, d2) in verified.iter().skip(1) {
                let Some(e) = edits.iter().find(|e| &e.desc == d2) else { continue };
                if e.cells().iter().any(|p| next.blocks.get(p) != cur.blocks.get(p)) {
                    continue;
                }
                let cand = e.apply(&next);
                if block_count(&cand) >= block_count(&next) {
                    continue;
                }
                let mut w = pool.worker(0);
                let sb = sandboxes.get(&mut w, 0)?;
                if verify_in(&mut w, sb, &cand, &opts.judge).is_ok() {
                    log(&format!("  also: {d2} -> {} blocks", block_count(&cand)));
                    next = cand;
                }
            }
            cur = finalize(pool, &sandboxes, next, opts, log)?;
            visited.insert(design_hash(&cur));
            on_improve(&cur, &desc)?;
            walk_left = opts.walk;
            continue;
        }
        if !opts.plateau || opts.timed_out() {
            break;
        }
        // Plateau: neutral edits that pass the judge, each followed by single (and pair) removals.
        log(&format!("  no improving move; plateau: judging {} neutral edits", neutral_all.len()));
        let cands: Vec<Cand> = neutral_all.into_iter().map(|edit| Cand { base: 0, edit }).collect();
        let mut plateau: Vec<(usize, Design, String)> = evaluate(pool, &sandboxes, &bases, cands, opts, &failed, 0, log)
            .into_iter()
            .filter(|(_, d, _)| block_count(d) == block_count(&cur) && !visited.contains(&design_hash(d)))
            .collect();
        if plateau.is_empty() {
            log("  plateau: no verified neutral design left");
            break;
        }
        let pbases: Vec<Design> = plateau.iter().map(|(_, d, _)| d.clone()).collect();
        let mut cands = Vec::new();
        for (i, b) in pbases.iter().enumerate() {
            let c = Ctx::new(b)?;
            for edit in removals(&c, b, true, opts.plateau_pairs, false, opts.radius) {
                cands.push(Cand { base: i, edit });
            }
        }
        let n_removals = cands.len();
        if opts.has(Family::Reroute) || opts.has(Family::Renet) {
            let tr = Instant::now();
            let spec = ctx.spec;
            let swept: Vec<HashSet<Pos>> = pool
                .map((0..pbases.len()).collect(), |w, _, i| crate::route::swept_cells(w, &pbases[i], &spec))
                .into_iter()
                .map(|r| r.unwrap_or_default())
                .collect();
            for (i, (b, _)) in routed(opts, &pbases, &swept).into_iter().enumerate() {
                for edit in b {
                    cands.push(Cand { base: i, edit });
                }
            }
            log(&format!("  plateau: routing took {:.1}s", tr.elapsed().as_secs_f64()));
        }
        log(&format!(
            "  plateau: {} verified neutral designs, {n_removals} removals and {} reroutes after them",
            pbases.len(),
            cands.len() - n_removals
        ));
        let found = evaluate(pool, &sandboxes, &pbases, cands, opts, &failed, 1, log);
        if let Some((bi, best, desc)) = found.into_iter().next() {
            let desc = format!("{} then {desc}", plateau[bi].2);
            log(&format!("  accepted: {desc} -> {} blocks", block_count(&best)));
            cur = finalize(pool, &sandboxes, best, opts, log)?;
            visited.insert(design_hash(&cur));
            on_improve(&cur, &desc)?;
            walk_left = opts.walk;
            continue;
        }
        if walk_left == 0 {
            break;
        }
        walk_left -= 1;
        let pick = (rand_next(&mut rng) % plateau.len() as u64) as usize;
        let (_, d, desc) = plateau.swap_remove(pick);
        log(&format!("  walk ({} steps left): {desc}", walk_left));
        cur = d;
        visited.insert(design_hash(&cur));
    }
    Ok(cur)
}
