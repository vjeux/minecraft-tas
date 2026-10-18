//! `mctas door properties`: which properties of the Redstone Squid records catalogue's piston
//! door rules (sections 2.1.4-2.1.7, 3 and 4) every working design has, for a regular 3x3 lever
//! door. Operational definitions (every choice is documented in
//! designs/piston-door-3x3/tracks/props/NOTES.md):
//!
//! * Geometry: door plane = `z = door.z` (`x = door.x` for `door-axis: z`). Tunnel T = the 3x3
//!   columns through the opening, any depth. Hallway surface S = the cells face-adjacent to T, not
//!   in T (12 per depth slice), the door plane included. Circuitry = every block except the 9 door
//!   blocks (tracked through every piston move), the lever, and quartz-like blocks resting in S.
//! * Everything runs at ONE position (the design as written), one orientation, one cycle (two
//!   where the property is about coming back); not directional / not locational come from the
//!   leaderboard's cached verdicts (rotation check, 32-position judge).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, Result, bail};
use mctas::blockstate::{block_name, is_air};
use mctas::design::Design;
use mctas::sim::{MovingBlock, PlaceMode, Pos, Region, SandboxOptions, Worker};

use crate::{DoorSpec, VerifyOptions};

/// The cache, in the archive folder: design hash -> properties.
pub const CACHE_FILE: &str = "properties-v1.tsv";

/// Properties of one design: key -> value (the cache line and the table cells).
pub type Props = BTreeMap<String, String>;

type Frame = (Region, Vec<MovingBlock>);

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn short(s: &str) -> &str {
    block_name(s).trim_start_matches("minecraft:")
}

const DIRS: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

/// Door-relative geometry: depth along the door's axis, (u, v) across the opening.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub spec: DoorSpec,
    a: usize,
    wa: usize,
}

impl Geo {
    pub fn new(spec: DoorSpec) -> Geo {
        let (a, wa) = if spec.along_z { (0, 2) } else { (2, 0) };
        Geo { spec, a, wa }
    }
    pub fn depth(&self, p: Pos) -> i32 {
        p[self.a] - self.spec.door[self.a]
    }
    pub fn u(&self, p: Pos) -> i32 {
        p[self.wa] - self.spec.door[self.wa]
    }
    pub fn v(&self, p: Pos) -> i32 {
        p[1] - self.spec.door[1]
    }
    pub fn in_t(&self, p: Pos) -> bool {
        (0..3).contains(&self.u(p)) && (0..3).contains(&self.v(p))
    }
    pub fn in_frame(&self, p: Pos) -> bool {
        self.in_t(p) && self.depth(p) == 0
    }
    pub fn in_s(&self, p: Pos) -> bool {
        let (u, v) = (self.u(p), self.v(p));
        ((0..3).contains(&u) && (v == -1 || v == 3)) || ((0..3).contains(&v) && (u == -1 || u == 3))
    }
    pub fn cell(&self, u: i32, v: i32, depth: i32) -> Pos {
        let mut p = self.spec.door;
        p[self.wa] += u;
        p[1] += v;
        p[self.a] += depth;
        p
    }
    fn axis_name(&self) -> &'static str {
        if self.a == 2 { "z" } else { "x" }
    }
    /// Side label for a viewer standing at negative (`-1`) or positive (`1`) depth.
    fn side(&self, s: i32) -> String {
        format!("{}{}", self.axis_name(), if s < 0 { "-" } else { "+" })
    }
}

/// Opaque, full, conductive, pushable, non-gravity building blocks ("quartz-like": replacing them
/// with quartz keeps a build working). Glazed terracotta is left out: it is used where slime must
/// not drag the block, which quartz would not do.
pub fn quartz_like(state: &str) -> bool {
    let n = short(state);
    if n.contains("glazed") || n.ends_with("concrete_powder") || n.contains("slab") || n.contains("stairs") || n.contains("wall") {
        return false;
    }
    matches!(
        n,
        "stone" | "smooth_stone" | "cobblestone" | "mossy_cobblestone" | "stone_bricks" | "mossy_stone_bricks" | "cracked_stone_bricks"
            | "chiseled_stone_bricks" | "granite" | "polished_granite" | "diorite" | "polished_diorite" | "andesite" | "polished_andesite"
            | "deepslate" | "cobbled_deepslate" | "polished_deepslate" | "deepslate_bricks" | "deepslate_tiles" | "tuff" | "calcite"
            | "dirt" | "coarse_dirt" | "quartz_block" | "smooth_quartz" | "quartz_bricks" | "chiseled_quartz_block" | "quartz_pillar"
            | "sandstone" | "smooth_sandstone" | "cut_sandstone" | "red_sandstone" | "bricks" | "terracotta" | "netherrack" | "end_stone"
            | "end_stone_bricks" | "blackstone" | "polished_blackstone" | "polished_blackstone_bricks" | "prismarine" | "dark_prismarine"
            | "purpur_block" | "mud_bricks" | "packed_mud" | "bone_block" | "nether_bricks" | "red_nether_bricks" | "basalt" | "smooth_basalt"
            | "polished_basalt" | "amethyst_block" | "lapis_block" | "coal_block" | "iron_block" | "gold_block" | "diamond_block" | "emerald_block"
    ) || n.ends_with("_planks")
        || n.ends_with("_wool")
        || n.ends_with("_concrete")
        || n.ends_with("_terracotta")
        || n.ends_with("_log")
        || n.ends_with("_wood")
        || (n.starts_with("waxed_") && (n.ends_with("copper") || n.ends_with("cut_copper") || n.ends_with("copper_block")))
}

fn is_moving(s: &str) -> bool {
    s.starts_with("minecraft:moving_piston")
}

fn is_gravity(n: &str) -> bool {
    n.ends_with("concrete_powder")
        || matches!(n, "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel" | "anvil" | "chipped_anvil" | "damaged_anvil" | "dragon_egg" | "pointed_dripstone" | "scaffolding")
}

fn flammable(n: &str) -> bool {
    n.ends_with("_wool") || n.ends_with("_carpet") || n.ends_with("_log") || n.ends_with("_wood") || n.ends_with("_planks") || n.ends_with("leaves")
        || matches!(n, "hay_block" | "bookshelf" | "tnt" | "scaffolding" | "bamboo_block" | "dried_kelp_block" | "target" | "composter")
}

/// Place a design the way the judge does (raw blocks, wire shapes, one self-update each,
/// entities) and settle.
pub fn place_settled(w: &mut Worker, sb: u32, d: &Design) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    if !d.entities.is_empty() {
        w.summon(sb, &d.entities)?;
    }
    let s = w.step_until_idle(sb, 400)?;
    if s.idle != Some(true) {
        bail!("the build never settles after placement");
    }
    Ok(())
}

/// Step one tick at a time until idle (at most `max`), capturing the box and its moving blocks.
fn record_until_idle(w: &mut Worker, sb: u32, lo: Pos, hi: Pos, max: u32) -> Result<Vec<Frame>> {
    let mut out = Vec::new();
    for _ in 0..max {
        let r = w.step_until_idle(sb, 1)?;
        out.push(w.get_region_moving(sb, lo, hi)?);
        if r.idle == Some(true) {
            return Ok(out);
        }
    }
    bail!("still moving {max} ticks after the flip")
}

/// The 9 door blocks followed through one operation.
#[derive(Clone, Debug, Default)]
pub struct Track {
    /// [frame][block]: where each door block is after the frame (`None` = lost track).
    pub pos: Vec<Vec<Option<Pos>>>,
    /// [frame][block]: in motion during that frame.
    pub moving: Vec<Vec<bool>>,
    /// Moves per block.
    pub moves: Vec<usize>,
    /// Cells each block rested in or travelled to, in order.
    pub paths: Vec<Vec<Pos>>,
    pub lost: usize,
    /// Moves of pistons and observers: (origin, destination), for flying-machine detection.
    pub machine_moves: Vec<(Pos, Pos)>,
}

fn holds(f: &Region, moving: &HashMap<Pos, &MovingBlock>, p: Pos, state: &str) -> bool {
    match f.get(p) {
        Some(s) if s == state => true,
        Some(s) if is_moving(s) => moving.get(&p).is_some_and(|m| m.state == state),
        _ => false,
    }
}

fn track(start: &[Pos], before: &Region, frames: &[Frame], door_state: &str) -> Track {
    let n = start.len();
    let mut t = Track { moves: vec![0; n], paths: start.iter().map(|p| vec![*p]).collect(), ..Default::default() };
    let mut cur: Vec<Option<Pos>> = start.iter().map(|p| Some(*p)).collect();
    let mut prev_keys: HashSet<(Pos, String, bool)> = HashSet::new();
    let mut prev_region = before;
    // Blocks of the door's type that are not door blocks (at rest, outside the start cells).
    let static_same: HashSet<Pos> = before.iter().filter(|(q, s)| *s == door_state && !start.contains(q)).map(|(q, _)| q).collect();
    for (f, mv) in frames {
        let mut moves: HashMap<Pos, Pos> = HashMap::new();
        let mut keys = HashSet::new();
        for m in mv.iter().filter(|m| !m.source) {
            let key = (m.pos, m.direction.clone(), m.extending);
            if !prev_keys.contains(&key) {
                moves.insert(m.origin(), m.pos);
                let nm = short(&m.state);
                if matches!(nm, "piston" | "sticky_piston" | "observer") {
                    t.machine_moves.push((m.origin(), m.pos));
                }
            }
            keys.insert(key);
        }
        prev_keys = keys;
        let moving_at: HashMap<Pos, &MovingBlock> = mv.iter().map(|m| (m.pos, m)).collect();
        for (i, c) in cur.iter_mut().enumerate() {
            if let Some(p) = *c
                && let Some(&q) = moves.get(&p)
            {
                *c = Some(q);
                t.moves[i] += 1;
                t.paths[i].push(q);
            }
        }
        // A move started and finished within one tick (a spit) leaves no moving block: look for an
        // unexplained door block next to a block that vanished.
        for i in 0..n {
            let Some(p) = cur[i] else { continue };
            if holds(f, &moving_at, p, door_state) {
                continue;
            }
            let claimed: HashSet<Pos> = cur.iter().flatten().copied().collect();
            let mut found = None;
            'search: for d in DIRS {
                for k in 1..=2 {
                    let q = [p[0] + d[0] * k, p[1] + d[1] * k, p[2] + d[2] * k];
                    if !claimed.contains(&q) && holds(f, &moving_at, q, door_state) && prev_region.get(q) != Some(door_state) {
                        found = Some(q);
                        break 'search;
                    }
                }
            }
            match found {
                Some(q) => {
                    cur[i] = Some(q);
                    t.moves[i] += 1;
                    t.paths[i].push(q);
                }
                None => {
                    cur[i] = None;
                    t.lost += 1;
                }
            }
        }
        // A door block that fell (a falling block entity) or vanished comes back when it lands:
        // match lost blocks to unclaimed door-type blocks (not the design's other blocks of that
        // type), nearest first.
        if cur.iter().any(|c| c.is_none()) {
            let claimed: HashSet<Pos> = cur.iter().flatten().copied().collect();
            let mut cands: Vec<Pos> = f
                .iter()
                .filter(|(q, _)| holds(f, &moving_at, *q, door_state) && !claimed.contains(q) && !static_same.contains(q))
                .map(|(q, _)| q)
                .collect();
            for i in 0..n {
                if cur[i].is_some() || cands.is_empty() {
                    continue;
                }
                let last = *t.paths[i].last().unwrap();
                let dist = |q: &Pos| (0..3).map(|k| (q[k] - last[k]).abs()).sum::<i32>();
                let k = (0..cands.len()).min_by_key(|k| dist(&cands[*k])).unwrap();
                let q = cands.remove(k);
                cur[i] = Some(q);
                t.moves[i] += 1;
                t.paths[i].push(q);
                t.lost -= 1;
            }
        }
        t.moving.push(cur.iter().map(|c| c.is_some_and(|p| f.get(p).is_some_and(is_moving))).collect());
        t.pos.push(cur.clone());
        prev_region = f;
    }
    t
}

/// One open/close cycle at the design's own position, tick by tick, with the moving blocks.
pub struct Recording {
    pub geo: Geo,
    pub lever: Pos,
    pub lo: Pos,
    pub hi: Pos,
    pub rest: Region,
    pub open: Vec<Frame>,
    pub close: Vec<Frame>,
    pub door_state: String,
    pub track_open: Track,
    pub track_close: Track,
}

impl Recording {
    pub fn open_state(&self) -> &Region {
        &self.open.last().unwrap().0
    }
    fn open_doors(&self) -> HashSet<Pos> {
        self.track_open.pos.last().map(|v| v.iter().flatten().copied().collect()).unwrap_or_default()
    }
}

pub fn record(w: &mut Worker, sb: u32, d: &Design) -> Result<Recording> {
    let spec = DoorSpec::from_design(d)?;
    let geo = Geo::new(spec);
    place_settled(w, sb, d)?;
    let (lo, hi) = crate::watch_box(d, &spec);
    let rest = w.get_region(sb, lo, hi)?;
    let cells = spec.door_cells();
    let door_state = rest.get(cells[0]).context("door cell outside the box")?.to_string();
    w.use_block(sb, spec.lever)?;
    let open = record_until_idle(w, sb, lo, hi, 400)?;
    w.use_block(sb, spec.lever)?;
    let close = record_until_idle(w, sb, lo, hi, 400)?;
    let track_open = track(&cells, &rest, &open, &door_state);
    let open_last = track_open.pos.last().cloned().unwrap_or_default();
    let start_close: Vec<Pos> = open_last.iter().zip(cells.iter()).map(|(p, c)| p.unwrap_or(*c)).collect();
    let track_close = track(&start_close, &open.last().unwrap().0, &close, &door_state);
    Ok(Recording { geo, lever: spec.lever, lo, hi, rest, open, close, door_state, track_open, track_close })
}

/// Circuitry cells of a frame: every block but the door blocks (`doors`), the lever and
/// quartz-like blocks resting in S. Moving non-door blocks count.
fn circuitry(geo: &Geo, f: &Region, doors: &HashSet<Pos>, lever: Pos) -> Vec<(Pos, String)> {
    f.iter()
        .filter(|(p, s)| !is_air(s) && !doors.contains(p) && *p != lever && !(geo.in_s(*p) && !is_moving(s) && quartz_like(s)))
        .map(|(p, s)| (p, s.to_string()))
        .collect()
}

fn first<'a>(v: impl IntoIterator<Item = &'a (Pos, String)>) -> Option<String> {
    v.into_iter().next().map(|(p, s)| format!("{} at {:?}", short(s), p))
}

/// Ticks (1-based) from which `ok` holds for good.
fn settled(frames: &[Frame], ok: impl Fn(&Region) -> bool) -> Option<u32> {
    let mut first = None;
    for (i, (f, _)) in frames.iter().enumerate() {
        if ok(f) {
            first.get_or_insert(i as u32 + 1);
        } else {
            first = None;
        }
    }
    first
}

/// Last tick (1-based) at which a cell of T or S moves or changes (visible movement), 0 if none.
fn last_visible(geo: &Geo, before: &Region, frames: &[Frame]) -> u32 {
    let mut last = 0;
    let mut prev = before;
    for (i, (f, _)) in frames.iter().enumerate() {
        let changed = f.iter().any(|(p, s)| (geo.in_t(p) || geo.in_s(p)) && (is_moving(s) || prev.get(p) != Some(s)));
        if changed {
            last = i as u32 + 1;
        }
        prev = f;
    }
    last
}

/// Timings of one recorded cycle (ticks after the input): opening time (T walkway and frame
/// walkable for good), opening visible time, closing time (door cells back for good), closing
/// visible time. `t0` = the tick the input reached the input block (0 for a player flip).
fn timings(rec: &Recording, t0: u32) -> [i64; 4] {
    let geo = rec.geo;
    let spec = geo.spec;
    let open_ok = |f: &Region| crate::open_problems(f, &spec).is_empty();
    let cells = spec.door_cells();
    let closed_ok = |f: &Region| cells.iter().all(|p| f.get(*p) == Some(rec.door_state.as_str()));
    let o = settled(&rec.open, open_ok).unwrap_or(0) as i64;
    let ov = last_visible(&geo, &rec.rest, &rec.open) as i64;
    let c = settled(&rec.close, closed_ok).unwrap_or(0) as i64;
    let cv = last_visible(&geo, rec.open_state(), &rec.close) as i64;
    let t0 = t0 as i64;
    [o - t0, ov - t0, c - t0, cv - t0]
}

/// The cheap properties: everything read from the recording and the design file.
pub fn cheap(rec: &Recording, d: &Design) -> Props {
    let mut p = Props::new();
    let geo = rec.geo;
    let lever = rec.lever;
    let rest_doors: HashSet<Pos> = geo.spec.door_cells().into_iter().collect();
    let open_doors = rec.open_doors();
    let circ_r = circuitry(&geo, &rec.rest, &rest_doors, lever);
    let circ_o = circuitry(&geo, rec.open_state(), &open_doors, lever);
    if rec.track_open.lost + rec.track_close.lost > 0 {
        p.insert("track".into(), format!("lost {} door blocks", rec.track_open.lost + rec.track_close.lost));
    }

    // SEAMLESS (2.1.4.1).
    let vis = |c: &(Pos, String)| geo.in_t(c.0) || geo.in_s(c.0);
    let quart = |s: i32| circ_r.iter().filter(|c| vis(c) && geo.depth(c.0) * s > 0).collect::<Vec<_>>();
    let open_vis: Vec<&(Pos, String)> = circ_o.iter().filter(|c| vis(c)).collect();
    let mut anim_vis: Option<String> = None;
    for (frames, tr) in [(&rec.open, &rec.track_open), (&rec.close, &rec.track_close)] {
        for (i, (f, _)) in frames.iter().enumerate() {
            let doors: HashSet<Pos> = tr.pos[i].iter().flatten().copied().collect();
            let c = circuitry(&geo, f, &doors, lever);
            if let Some(x) = first(c.iter().filter(|c| vis(c))) {
                anim_vis.get_or_insert(x);
            }
        }
    }
    let qs: Vec<i32> = [-1, 1].into_iter().filter(|s| quart(*s).is_empty()).collect();
    let full = qs.len() == 2 && open_vis.is_empty();
    let tier = if full && anim_vis.is_none() {
        "SUPER".to_string()
    } else if full {
        "FULL".to_string()
    } else if !qs.is_empty() && open_vis.is_empty() {
        format!("SEMI {}", qs.iter().map(|s| geo.side(*s)).collect::<Vec<_>>().join(","))
    } else if !qs.is_empty() {
        format!("QUART {}", qs.iter().map(|s| geo.side(*s)).collect::<Vec<_>>().join(","))
    } else {
        "-".to_string()
    };
    p.insert("seamless".into(), tier);
    let why = if qs.is_empty() {
        first(quart(-1)).map(|x| format!("closed: {x}"))
    } else if !open_vis.is_empty() {
        first(open_vis.iter().copied()).map(|x| format!("open: {x}"))
    } else if qs.len() < 2 {
        first(quart(-qs[0])).map(|x| format!("closed other side: {x}"))
    } else {
        anim_vis.clone().map(|x| format!("moving: {x}"))
    };
    if let Some(w) = why {
        p.insert("seamless_why".into(), w);
    }
    // Every blocker, per check: closed side z-/z+ and the open state.
    let list = |v: &[&(Pos, String)]| -> String {
        if v.is_empty() {
            return "-".into();
        }
        let mut s: Vec<String> = v.iter().take(8).map(|(q, st)| format!("{}{:?}", short(st), q)).collect();
        if v.len() > 8 {
            s.push(format!("+{}", v.len() - 8));
        }
        s.join(" ")
    };
    p.insert(format!("vis_closed_{}", geo.side(-1)), list(&quart(-1)));
    p.insert(format!("vis_closed_{}", geo.side(1)), list(&quart(1)));
    p.insert("vis_open".into(), list(&open_vis));

    // DENTLESS / MINOR DENTS ONLY (2.1.4.2-3), stable states.
    let mut dent = 0u8; // 0 dentless, 1 minor dents only, 2 dents, 3 ineligible
    let mut dent_why = None;
    let (dmin, dmax) = (geo.depth(rec.lo), geo.depth(rec.hi));
    let (dmin, dmax) = (dmin.min(dmax), dmin.max(dmax));
    for (name, f, doors) in [("closed", &rec.rest, &rest_doors), ("open", rec.open_state(), &open_doors)] {
        for (q, s) in f.iter() {
            if is_air(s) || doors.contains(&q) {
                continue;
            }
            let flags = f.flags_at(q).unwrap_or_default();
            // Closed, the door plane's S ring touches T only through door blocks: unseen.
            if name == "closed" && geo.in_s(q) && geo.depth(q) == 0 {
                continue;
            }
            let level = if geo.in_t(q) {
                if flags.collides {
                    3
                } else {
                    let sup = support(s, q);
                    if sup.is_some_and(|c| geo.in_s(c)) { 1 } else { 2 }
                }
            } else if geo.in_s(q) {
                if flags.full {
                    0
                } else if geo.v(q) == -1 {
                    2
                } else {
                    1
                }
            } else {
                0
            };
            if level > dent {
                dent = level;
                dent_why = Some(format!("{name}: {} at {:?}", short(s), q));
            }
        }
        // Holes: S cells filled at rest but empty in the open state.
        if name == "open" {
            for dd in dmin..=dmax {
                for u in -1..=3 {
                    for v in -1..=3 {
                        let q = geo.cell(u, v, dd);
                        if !geo.in_s(q) {
                            continue;
                        }
                        let was = rec.rest.get(q).is_some_and(|s| !is_air(s));
                        let now = f.get(q).is_some_and(|s| !is_air(s));
                        if was && !now {
                            let level = if v == -1 { 2 } else { 1 };
                            if level > dent {
                                dent = level;
                                dent_why = Some(format!("open: hole at {:?}", q));
                            }
                        }
                    }
                }
            }
        }
    }
    p.insert("dents".into(), ["dentless", "minor", "dents", "inelig"][dent as usize].into());
    if let Some(w) = dent_why {
        p.insert("dents_why".into(), w);
    }

    // TRAPDOOR // FLUSH // DELUXE (2.1.4.4), both stable states, per viewer side.
    let both: Vec<&(Pos, String)> = circ_r.iter().chain(circ_o.iter()).collect();
    let tier_name = ["trapdoor", "flush", "deluxe"];
    let mut flush_full = Vec::new();
    let mut flush_semi = Vec::new();
    for s in [-1, 1] {
        let behind = |c: &&(Pos, String)| geo.depth(c.0) * -s;
        let worst_full = both.iter().map(behind).min().unwrap_or(99);
        let worst_semi = both.iter().filter(|c| geo.v(c.0) > -1).map(behind).min().unwrap_or(99);
        if worst_full >= 0 {
            flush_full.push(format!("{} {}", tier_name[worst_full.min(2) as usize], geo.side(s)));
        }
        if worst_semi >= 0 {
            flush_semi.push(format!("{} {}", tier_name[worst_semi.min(2) as usize], geo.side(s)));
        }
    }
    p.insert("flush_full".into(), if flush_full.is_empty() { "-".into() } else { flush_full.join(",") });
    p.insert("flush_semi".into(), if flush_semi.is_empty() { "-".into() } else { flush_semi.join(",") });

    // HIPSTER (2.1.4.5): all circuitry beyond S (full) or within S or beyond (semi).
    let mut hip = Vec::new();
    let all = |f: &dyn Fn(Pos) -> bool| both.iter().all(|c| f(c.0));
    if all(&|q| geo.v(q) <= -2) {
        hip.push("FULL floor");
    } else if all(&|q| geo.v(q) <= -1) {
        hip.push("SEMI floor");
    }
    if all(&|q| geo.v(q) >= 4) {
        hip.push("FULL ceiling");
    } else if all(&|q| geo.v(q) >= 3) {
        hip.push("SEMI ceiling");
    }
    if all(&|q| geo.u(q) <= -2 || geo.u(q) >= 4) {
        hip.push("FULL wall");
    } else if all(&|q| geo.u(q) <= -1 || geo.u(q) >= 3) {
        hip.push("SEMI wall");
    }
    p.insert("hipster".into(), if hip.is_empty() { "-".into() } else { hip.join(",") });

    // SYMMETRICAL (2.1.5.1): the T slices (walkway depth) as empty / resting / moving, every tick.
    let grid = |f: &Region, dd: i32| -> [[u8; 3]; 3] {
        let mut g = [[0u8; 3]; 3];
        for (v, row) in g.iter_mut().enumerate() {
            for (u, c) in row.iter_mut().enumerate() {
                let q = geo.cell(u as i32, v as i32, dd);
                *c = match f.get(q) {
                    Some(s) if is_moving(s) => 2,
                    Some(s) if !is_air(s) => 1,
                    _ => 0,
                };
            }
        }
        g
    };
    let axes: [(&str, fn(usize, usize) -> (usize, usize)); 4] =
        [("V", |u, v| (2 - u, v)), ("H", |u, v| (u, 2 - v)), ("D1", |u, v| (v, u)), ("D2", |u, v| (2 - v, 2 - u))];
    let all_frames: Vec<&Region> = std::iter::once(&rec.rest).chain(rec.open.iter().map(|f| &f.0)).chain(rec.close.iter().map(|f| &f.0)).collect();
    let sym: Vec<&str> = axes
        .iter()
        .filter(|(_, m)| {
            all_frames.iter().all(|f| {
                (-3..=3).all(|dd| {
                    let g = grid(f, dd);
                    (0..3).all(|v| (0..3).all(|u| { let (mu, mv) = m(u, v); g[v][u] == g[mv][mu] }))
                })
            })
        })
        .map(|(n, _)| *n)
        .collect();
    p.insert("sym".into(), if sym.is_empty() { "-".into() } else { sym.join(" ") });

    // SYNC (2.1.5.2): sections = the 8 ring blocks and the center (door cell index 4).
    let ticks_of = |tr: &Track, i: usize| -> Vec<usize> { tr.moving.iter().enumerate().filter(|(_, m)| m[i]).map(|(t, _)| t).collect() };
    let unison = |tr: &Track| {
        let ring: Vec<Vec<usize>> = (0..9).filter(|i| *i != 4).map(|i| ticks_of(tr, i)).collect();
        ring.iter().all(|r| *r == ring[0])
    };
    let minimal = |tr: &Track| (0..9).all(|i| if i == 4 { tr.moves[i] <= 2 } else { tr.moves[i] == 1 });
    let full_sym = sym.len() == 4;
    let full_sync = full_sym && unison(&rec.track_open) && unison(&rec.track_close);
    let sync = if full_sync && minimal(&rec.track_open) && minimal(&rec.track_close) {
        "SUPER"
    } else if full_sync {
        "FULL"
    } else if !sym.is_empty() {
        "SEMI"
    } else {
        "-"
    };
    p.insert("sync".into(), sync.into());

    // CLEAN (2.1.5.3): super seamless + every door block leaves the frame once while opening and
    // enters it once while closing, never revisiting a cell (no bounce, no jerk).
    let mut unclean = None;
    for (name, tr, opening) in [("opening", &rec.track_open, true), ("closing", &rec.track_close, false)] {
        for i in 0..9 {
            let flags: Vec<bool> = std::iter::once(opening).chain(tr.pos.iter().map(|v| v[i].is_some_and(|q| geo.in_frame(q)))).collect();
            let changes = flags.windows(2).filter(|w| w[0] != w[1]).count();
            let mut seen = BTreeSet::new();
            let revisit = tr.paths[i].windows(2).any(|w| w[0] == w[1]) || tr.paths[i].iter().any(|q| !seen.insert(*q));
            if changes > 1 || revisit {
                unclean.get_or_insert(format!("{name}: door block {i} {}", if changes > 1 { "re-crosses the frame" } else { "moves back" }));
            }
        }
    }
    let super_seam = p["seamless"] == "SUPER";
    p.insert(
        "clean".into(),
        match (&unclean, super_seam) {
            (None, true) => "yes".into(),
            (None, false) => "no (not super seamless)".into(),
            (Some(w), _) => format!("no ({w})"),
        },
    );

    // COMPONENTS (2.1.6).
    let names: BTreeSet<&str> = d.blocks.values().map(|s| short(s)).collect();
    let has = |n: &str| names.contains(n);
    let base = |n: &str| matches!(n, "piston" | "sticky_piston" | "piston_head" | "lever") || quartz_like(&format!("minecraft:{n}"));
    let only = |extra: &[&str]| names.iter().all(|n| base(n) || extra.contains(n));
    let mut flying = false;
    {
        // A piston or observer carried two or more cells the same way in one operation.
        for tr in [&rec.track_open, &rec.track_close] {
            let mut chains: HashMap<Pos, (Pos, usize)> = HashMap::new();
            for (o, q) in &tr.machine_moves {
                let dir = [q[0] - o[0], q[1] - o[1], q[2] - o[2]];
                let run = match chains.remove(o) {
                    Some((d0, k)) if d0 == dir => k + 1,
                    _ => 1,
                };
                if run >= 2 {
                    flying = true;
                }
                chains.insert(*q, (dir, run));
            }
        }
    }
    let mut comp = Vec::new();
    let mut flag = |ok: bool, code: &'static str| {
        if ok {
            comp.push(code)
        }
    };
    flag(!has("slime_block"), "S");
    flag(!has("honey_block"), "H");
    flag(!names.iter().any(|n| is_gravity(n)), "G");
    flag(!has("sticky_piston") && !has("slime_block") && !has("honey_block"), "K");
    flag(!has("observer"), "O");
    flag(!has("note_block"), "N");
    flag(d.entities.is_empty(), "E");
    flag(!flying, "F");
    flag(only(&["redstone_wire", "redstone_torch", "redstone_wall_torch"]), "TD");
    flag(only(&["redstone_block"]), "RB");
    flag(only(&["hopper", "dropper", "comparator"]), "ZA");
    flag(only(&["cauldron", "water_cauldron", "lava_cauldron", "powder_snow_cauldron", "comparator"]), "ZI");
    p.insert("components".into(), comp.join(" "));

    // VOLUME (4.2): circuitry over every state and tick, frame excluded, T only when open.
    let mut cells: Vec<Pos> = circ_r.iter().map(|c| c.0).filter(|q| !geo.in_t(*q)).collect();
    for (frames, tr) in [(&rec.open, &rec.track_open), (&rec.close, &rec.track_close)] {
        for (i, (f, _)) in frames.iter().enumerate() {
            let doors: HashSet<Pos> = tr.pos[i].iter().flatten().copied().collect();
            cells.extend(circuitry(&geo, f, &doors, lever).into_iter().map(|c| c.0).filter(|q| !geo.in_t(*q)));
        }
    }
    cells.extend(circ_o.iter().map(|c| c.0).filter(|q| !geo.in_frame(*q)));
    let vbox = if cells.is_empty() {
        None
    } else {
        let mut lo = cells[0];
        let mut hi = cells[0];
        for q in &cells {
            for i in 0..3 {
                lo[i] = lo[i].min(q[i]);
                hi[i] = hi[i].max(q[i]);
            }
        }
        Some((lo, hi))
    };
    if let Some((lo, hi)) = vbox {
        let dims = [hi[geo.wa] - lo[geo.wa] + 1, hi[1] - lo[1] + 1, hi[geo.a] - lo[geo.a] + 1];
        p.insert("volume".into(), (dims[0] * dims[1] * dims[2]).to_string());
        p.insert("volume_dims".into(), format!("{}x{}x{}", dims[0], dims[1], dims[2]));
    }

    // Input (3.1): lever on the surface of the volume, support constant.
    let lever_state = rec.rest.get(lever).unwrap_or("?").to_string();
    let sup = support(&lever_state, lever);
    let support_ok = sup.is_some_and(|s| {
        let st = rec.rest.get(s).map(str::to_string);
        rec.open.iter().chain(rec.close.iter()).all(|(f, _)| f.get(s).map(str::to_string) == st)
    });
    let surface = match vbox {
        None => true,
        Some((lo, hi)) => DIRS.iter().any(|dv| {
            if sup.is_some_and(|s| add(lever, *dv) == s) {
                return false;
            }
            let mut q = add(lever, *dv);
            loop {
                if (0..3).any(|i| q[i] < lo[i] || q[i] > hi[i]) {
                    return true;
                }
                let free = |f: &Region| f.get(q).is_none_or(is_air);
                if !free(&rec.rest) || !free(rec.open_state()) {
                    return false;
                }
                q = add(q, *dv);
            }
        }),
    };
    p.insert("lever_surface".into(), yn(surface));
    p.insert("lever_support".into(), yn(support_ok));

    // Player-input timings at the design's own position.
    let t = timings(rec, 0);
    p.insert("t_player".into(), format!("{}/{}/{}/{}", t[0], t[1], t[2], t[3]));

    // Non-isolated components (3.9) and player-sensitive blocks in the hallway (3.8), statically.
    let risky: Vec<&str> = names
        .iter()
        .copied()
        .filter(|n| {
            n.contains("sculk_sensor") || n.contains("pressure_plate") || n.contains("tripwire") || matches!(*n, "daylight_detector" | "target" | "lightning_rod")
        })
        .collect();
    p.insert("risky".into(), if risky.is_empty() { "-".into() } else { risky.join(",") });
    p
}

fn yn(b: bool) -> String {
    if b { "yes".into() } else { "no".into() }
}

/// The cell a block hangs on or stands on (levers, buttons, torches, dust, repeaters, rails...).
fn support(state: &str, p: Pos) -> Option<Pos> {
    let n = short(state);
    let prop = |k: &str| mctas::blockstate::BlockState::parse(state).ok().and_then(|b| b.get(k).map(str::to_string));
    let below = [p[0], p[1] - 1, p[2]];
    let facing_back = |f: &str| -> Option<Pos> {
        Some(match f {
            "north" => [p[0], p[1], p[2] + 1],
            "south" => [p[0], p[1], p[2] - 1],
            "east" => [p[0] - 1, p[1], p[2]],
            "west" => [p[0] + 1, p[1], p[2]],
            _ => return None,
        })
    };
    if n == "lever" || n.ends_with("_button") {
        return match prop("face").as_deref() {
            Some("floor") => Some(below),
            Some("ceiling") => Some([p[0], p[1] + 1, p[2]]),
            _ => facing_back(prop("facing").as_deref().unwrap_or("")),
        };
    }
    if n.ends_with("wall_torch") {
        return facing_back(prop("facing").as_deref().unwrap_or(""));
    }
    if matches!(n, "redstone_wire" | "repeater" | "comparator" | "redstone_torch" | "torch") || n.contains("rail") || n.contains("pressure_plate") || n.ends_with("carpet") {
        return Some(below);
    }
    None
}

// ---- dynamic checks (one position, the design as written) ----------------------------------------

fn opts_one(cycles: u32) -> VerifyOptions {
    VerifyOptions { cycles, offsets: vec![[0, 0, 0]], ..Default::default() }
}

fn judge(w: &mut Worker, sb: u32, d: &Design, cycles: u32) -> std::result::Result<(), String> {
    crate::verify_in(w, sb, d, &opts_one(cycles)).map(|_| ()).map_err(|e| format!("{e:#}"))
}

fn brief(e: &str) -> String {
    let e = e.replace(['\t', '\n'], " ");
    let e = e.strip_prefix("at offset [0, 0, 0]: ").unwrap_or(&e).to_string();
    if e.len() > 110 { format!("{}...", &e[..e.char_indices().take_while(|(i, _)| *i < 107).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(0)]) } else { e }
}

/// Every empty S cell along the build's depth extent + 3 filled with smooth stone.
pub fn with_hallway(d: &Design, rec: &Recording) -> Design {
    let geo = rec.geo;
    let (lo, hi) = d.bounds().unwrap();
    let (a, b) = (geo.depth(lo).min(geo.depth(hi)), geo.depth(lo).max(geo.depth(hi)));
    let mut out = d.clone();
    for dd in (a.min(-3) - 3)..=(b.max(3) + 3) {
        for u in -1..=3 {
            for v in -1..=3 {
                let q = geo.cell(u, v, dd);
                if geo.in_s(q) && rec.rest.get(q).is_none_or(is_air) && !d.blocks.contains_key(&q) {
                    out.blocks.insert(q, "minecraft:smooth_stone".into());
                }
            }
        }
    }
    out
}

/// The swept bounding box surrounded by one shell of `fill` (T cells stay open).
fn with_shell(d: &Design, rec: &Recording, fill: &dyn Fn(usize) -> Option<&'static str>) -> Design {
    let geo = rec.geo;
    let mut cells: Vec<Pos> = d.blocks.keys().copied().collect();
    for (f, _) in std::iter::once(&(rec.rest.clone(), Vec::new())).chain(rec.open.iter()).chain(rec.close.iter()) {
        cells.extend(f.iter().filter(|(_, s)| !is_air(s)).map(|(q, _)| q));
    }
    let mut lo = cells[0];
    let mut hi = cells[0];
    for q in &cells {
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    let mut out = d.clone();
    let mut k = 0;
    for x in lo[0] - 1..=hi[0] + 1 {
        for y in lo[1] - 1..=hi[1] + 1 {
            for z in lo[2] - 1..=hi[2] + 1 {
                let q = [x, y, z];
                let inside = (0..3).all(|i| q[i] >= lo[i] && q[i] <= hi[i]);
                if inside || geo.in_t(q) {
                    continue;
                }
                if let Some(s) = fill(k) {
                    out.blocks.insert(q, format!("minecraft:{s}"));
                }
                k += 1;
            }
        }
    }
    out
}

fn contained(w: &mut Worker, sb: u32, d: &Design, rec: &Recording) -> String {
    const MIX: [Option<&str>; 5] = [None, Some("smooth_stone"), Some("slime_block"), Some("honey_block"), Some("obsidian")];
    let shells: Vec<(&str, Box<dyn Fn(usize) -> Option<&'static str>>)> = vec![
        ("smooth stone", Box::new(|_| Some("smooth_stone"))),
        ("slime", Box::new(|_| Some("slime_block"))),
        ("honey", Box::new(|_| Some("honey_block"))),
        ("obsidian", Box::new(|_| Some("obsidian"))),
        ("mix", Box::new(|k| MIX[((k as u64).wrapping_mul(0x9e3779b97f4a7c15) >> 61) as usize % 5])),
    ];
    let mut fails = Vec::new();
    for (name, f) in &shells {
        let sd = with_shell(d, rec, f.as_ref());
        if judge(w, sb, &sd, 2).is_err() {
            fails.push(*name);
        }
    }
    if fails.is_empty() { "yes".into() } else { format!("no ({})", fails.join(",")) }
}

/// Snow layers on every sky-exposed top face of the current world (placed like snowfall, with
/// updates, only where a layer can stand).
fn snow_on(w: &mut Worker, sb: u32, lo: Pos, hi: Pos) -> Result<Vec<Pos>> {
    let r = w.get_region(sb, lo, hi)?;
    let mut cand = Vec::new();
    for x in lo[0]..=hi[0] {
        for z in lo[2]..=hi[2] {
            for y in (lo[1]..hi[1]).rev() {
                let s = r.get([x, y, z]).unwrap_or("minecraft:air");
                if !is_air(s) {
                    if r.flags_at([x, y, z]).is_some_and(|f| f.full) && !is_moving(s) {
                        cand.push([x, y + 1, z]);
                    }
                    break;
                }
            }
        }
    }
    let snow = "minecraft:snow[layers=1]";
    let raw: Vec<(Pos, &str)> = cand.iter().map(|p| (*p, snow)).collect();
    w.set_blocks(sb, &raw, PlaceMode::Raw)?;
    let (bad, _) = w.static_problems(sb, lo, [hi[0], hi[1] + 1, hi[2]])?;
    let air: Vec<(Pos, &str)> = cand.iter().map(|p| (*p, "minecraft:air")).collect();
    w.set_blocks(sb, &air, PlaceMode::Raw)?;
    let ok: Vec<Pos> = cand.into_iter().filter(|p| !bad.contains(p)).collect();
    let up: Vec<(Pos, &str)> = ok.iter().map(|p| (*p, snow)).collect();
    w.set_blocks(sb, &up, PlaceMode::Update)?;
    Ok(ok)
}

fn flip_check(w: &mut Worker, sb: u32, rec: &Recording, opening: bool) -> std::result::Result<(), String> {
    let spec = rec.geo.spec;
    w.use_block(sb, spec.lever).map_err(|e| e.to_string())?;
    let s = w.step_until_idle(sb, 400).map_err(|e| e.to_string())?;
    if s.idle != Some(true) {
        return Err("never settles".into());
    }
    let f = w.get_region(sb, rec.lo, rec.hi).map_err(|e| e.to_string())?;
    if !w.entities(sb).map_err(|e| e.to_string())?.is_empty() {
        return Err("items dropped".into());
    }
    if opening {
        let pr = crate::open_problems(&f, &spec);
        if !pr.is_empty() {
            return Err(format!("not open: {}", pr[0]));
        }
    } else {
        let bad = spec.door_cells().into_iter().find(|p| f.get(*p) != Some(rec.door_state.as_str()));
        if let Some(b) = bad {
            return Err(format!("not closed: {b:?}"));
        }
        let snowy = |s: &str| short(s) == "snow";
        if let Some((p, s)) = rec.rest.iter().find(|(p, s)| !snowy(s) && f.get(*p).is_some_and(|t| !snowy(t) && t != *s)) {
            return Err(format!("{p:?} {} did not come back", short(s)));
        }
    }
    Ok(())
}

fn weatherproof(w: &mut Worker, sb: u32, d: &Design, rec: &Recording) -> Result<String> {
    // Snow on the closed door, two cycles; snow on the open door, close-open-close.
    place_settled(w, sb, d)?;
    snow_on(w, sb, rec.lo, rec.hi)?;
    let mut r = (|| {
        for _ in 0..2 {
            flip_check(w, sb, rec, true)?;
            flip_check(w, sb, rec, false)?;
        }
        Ok(())
    })()
    .map_err(|e: String| format!("snow closed: {e}"));
    if r.is_ok() {
        place_settled(w, sb, d)?;
        w.use_block(sb, rec.lever)?;
        w.step_until_idle(sb, 400)?;
        snow_on(w, sb, rec.lo, rec.hi)?;
        r = (|| {
            flip_check(w, sb, rec, false)?;
            flip_check(w, sb, rec, true)?;
            flip_check(w, sb, rec, false)
        })()
        .map_err(|e: String| format!("snow open: {e}"));
    }
    // Flammable blocks under the open sky in either stable state (lightning fire).
    let mut burn = BTreeSet::new();
    for f in [&rec.rest, rec.open_state()] {
        for x in rec.lo[0]..=rec.hi[0] {
            for z in rec.lo[2]..=rec.hi[2] {
                for y in (rec.lo[1]..=rec.hi[1]).rev() {
                    if let Some(s) = f.get([x, y, z]).filter(|s| !is_air(s)) {
                        if flammable(short(s)) {
                            burn.insert(short(s).to_string());
                        }
                        break;
                    }
                }
            }
        }
    }
    Ok(match (r, burn.is_empty()) {
        (Ok(()), true) => "yes".into(),
        (Ok(()), false) => format!("no (fire: {})", burn.into_iter().collect::<Vec<_>>().join(",")),
        (Err(e), _) => format!("no ({})", brief(&e)),
    })
}

/// Repeater input (3.1.3, MC-172213): the lever's support block powered by a repeater instead of
/// the player's flip, so the input changes in the scheduled-tick phase (`ServerLevel.tick` ->
/// `blockTicks` -> `DiodeBlock.tick`) instead of the network phase (`ServerPlayerGameMode.useItemOn`
/// -> `LeverBlock.pull`, between ticks). A wall lever is replaced by the repeater at the lever's
/// own cell facing its support block (the remote connection of 3.1.4); a floor or ceiling lever
/// keeps its place and a repeater feeds the support block from a free side. The repeater is
/// driven by a redstone block placed (open) and removed (close) behind it.
pub fn repeater_input(w: &mut Worker, sb: u32, d: &Design, rec: &Recording) -> Result<(String, Option<[i64; 4]>)> {
    let lever = rec.lever;
    let lever_state = d.blocks.get(&lever).cloned().unwrap_or_default();
    let Some(sup) = support(&lever_state, lever) else { return Ok(("no support".into(), None)) };
    let dir_name = |dv: Pos| match dv {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [1, 0, 0] => "east",
        _ => "west",
    };
    let free = |q: Pos| !d.blocks.contains_key(&q) && rec.rest.get(q).is_none_or(is_air) && rec.open.iter().chain(rec.close.iter()).all(|(f, _)| f.get(q).is_none_or(is_air));
    // (repeater cell, input cell, keep lever)
    let mut choice = None;
    let to_sup = [sup[0] - lever[0], sup[1] - lever[1], sup[2] - lever[2]];
    if to_sup[1] == 0 {
        let input = [lever[0] - to_sup[0], lever[1], lever[2] - to_sup[2]];
        if free(input) {
            choice = Some((lever, input, false, to_sup));
        }
    }
    if choice.is_none() {
        for dv in [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]] {
            let rp = [sup[0] - dv[0], sup[1], sup[2] - dv[2]];
            let input = [rp[0] - dv[0], rp[1], rp[2] - dv[2]];
            if rp != lever && free(rp) && free(input) {
                choice = Some((rp, input, true, dv));
                break;
            }
        }
    }
    let Some((rp, input, keep, to)) = choice else { return Ok(("no free cell for a repeater".into(), None)) };
    let mut md = d.clone();
    if !keep {
        md.blocks.remove(&lever);
    }
    // The repeater faces its input: facing = the direction from the repeater to its input.
    let facing = dir_name([-to[0], 0, -to[2]]);
    md.blocks.insert(rp, format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered=false]"));
    let below = [rp[0], rp[1] - 1, rp[2]];
    if !md.blocks.contains_key(&below) {
        md.blocks.insert(below, "minecraft:smooth_stone".into());
    }
    let mode = if keep { "side" } else { "lever cell" };
    place_settled(w, sb, &md)?;
    let rest = w.get_region(sb, rec.lo, rec.hi)?;
    let spec = rec.geo.spec;
    let cp = crate::closed_problems(&rest, &spec);
    if !cp.is_empty() {
        return Ok((format!("no ({mode}: not closed at rest)"), None));
    }
    w.set_blocks(sb, &[(input, "minecraft:redstone_block")], PlaceMode::Update)?;
    let open = record_until_idle(w, sb, rec.lo, rec.hi, 400)?;
    w.set_blocks(sb, &[(input, "minecraft:air")], PlaceMode::Update)?;
    let close = record_until_idle(w, sb, rec.lo, rec.hi, 400)?;
    let fire = |frames: &[Frame], on: bool| frames.iter().position(|(f, _)| f.get(rp).is_some_and(|s| s.contains(if on { "powered=true" } else { "powered=false" }))).map(|i| i as u32 + 1).unwrap_or(0);
    let (t_on, t_off) = (fire(&open, true), fire(&close, false));
    let last_open = &open.last().unwrap().0;
    let last_close = &close.last().unwrap().0;
    let op = crate::open_problems(last_open, &spec);
    let ents = w.entities(sb)?;
    let back = rest.iter().all(|(p, s)| p == input || last_close.get(p) == Some(s));
    let rrec = Recording {
        geo: rec.geo,
        lever,
        lo: rec.lo,
        hi: rec.hi,
        rest: rest.clone(),
        open,
        close,
        door_state: rec.door_state.clone(),
        track_open: Track::default(),
        track_close: Track::default(),
    };
    let to = timings(&rrec, t_on);
    let tc = timings(&rrec, t_off);
    let t = [to[0], to[1], tc[2], tc[3]];
    let verdict = if !op.is_empty() {
        format!("no ({mode}: not open)")
    } else if !back || !ents.is_empty() {
        format!("no ({mode}: not back after closing)")
    } else {
        format!("yes ({mode})")
    };
    Ok((verdict, Some(t)))
}

/// A player walking (STEP) and landing (HIT_GROUND) on every floor cell of the walkway (and the
/// frame when open), in both stable states: nothing may change. `outside` emits from the shell of
/// the build's box instead (isolation, 3.9).
fn vibrations(w: &mut Worker, sb: u32, d: &Design, rec: &Recording, outside: bool) -> Result<String> {
    let geo = rec.geo;
    for open in [false, true] {
        place_settled(w, sb, d)?;
        if open {
            w.use_block(sb, rec.lever)?;
            w.step_until_idle(sb, 400)?;
        }
        let before = w.get_region(sb, rec.lo, rec.hi)?;
        let mut pts = Vec::new();
        if outside {
            let (lo, hi) = (rec.lo, rec.hi);
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        let edge = x == lo[0] || x == hi[0] || y == lo[1] || y == hi[1] || z == lo[2] || z == hi[2];
                        if edge && (x + y + z) % 2 == 0 {
                            pts.push([x, y, z]);
                        }
                    }
                }
            }
        } else {
            for dd in -3..=3 {
                if dd == 0 && !open {
                    continue;
                }
                for u in 0..3 {
                    pts.push(geo.cell(u, 0, dd));
                }
            }
        }
        for ev in ["step", "hit_ground"] {
            w.game_event(sb, ev, &pts)?;
            w.step(sb, 40)?;
            w.step_until_idle(sb, 400)?;
            let after = w.get_region(sb, rec.lo, rec.hi)?;
            if let Some((p, s)) = before.iter().find(|(p, s)| after.get(*p) != Some(*s)) {
                return Ok(format!("no ({} {ev}: {} at {p:?} changed)", if open { "open" } else { "closed" }, short(s)));
            }
            if !w.entities(sb)?.is_empty() {
                return Ok(format!("no ({} {ev}: items dropped)", if open { "open" } else { "closed" }));
            }
        }
    }
    Ok("yes".into())
}

/// The expensive checks, each at the design's own position.
pub fn dynamic(w: &mut Worker, sb: u32, d: &Design, rec: &Recording, p: &mut Props) -> Result<()> {
    let hall = with_hallway(d, rec);
    p.insert("hallway".into(), match judge(w, sb, &hall, 1) {
        Ok(()) => "yes".into(),
        Err(e) => format!("no ({})", brief(&e)),
    });
    p.insert("contained".into(), contained(w, sb, d, rec));
    p.insert("weatherproof".into(), weatherproof(w, sb, d, rec)?);
    let (rv, rt) = repeater_input(w, sb, d, rec)?;
    p.insert("repeater_input".into(), rv);
    if let Some(t) = rt {
        p.insert("t_repeater".into(), format!("{}/{}/{}/{}", t[0], t[1], t[2], t[3]));
    }
    p.insert("steps".into(), vibrations(w, sb, d, rec, false)?);
    let iso = if p.get("risky").is_some_and(|r| r.contains("sculk")) { vibrations(w, sb, d, rec, true)? } else { "yes".into() };
    p.insert("isolated".into(), iso);
    // Unlimited uses (3.4): the judge already brings every block state back each cycle; only
    // containers can drift unseen (item counts are not block states): 20 cycles for those.
    let containers = d.blocks.values().any(|s| matches!(short(s), "hopper" | "dropper" | "dispenser" | "chest" | "barrel" | "crafter" | "trapped_chest"));
    p.insert(
        "uses".into(),
        if containers {
            match judge(w, sb, d, 20) {
                Ok(()) => "yes (20 cycles)".into(),
                Err(e) => format!("no ({})", brief(&e)),
            }
        } else {
            "yes (judge: exact return)".into()
        },
    );
    Ok(())
}

/// Properties of one design (cheap ones, plus the dynamic ones when asked).
pub fn compute(w: &mut Worker, d: &Design, with_dynamic: bool) -> Result<Props> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| {
        let rec = record(w, sb, d)?;
        let mut p = cheap(&rec, d);
        if with_dynamic {
            dynamic(w, sb, d, &rec, &mut p)?;
        }
        Ok(p)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

// ---- cache, sources, table -------------------------------------------------------------------------

pub fn load_cache(dir: &Path) -> HashMap<u64, Props> {
    let mut m = HashMap::new();
    let Ok(text) = std::fs::read_to_string(dir.join(CACHE_FILE)) else { return m };
    for line in text.lines() {
        let mut it = line.split('\t');
        let Some(Ok(h)) = it.next().map(|x| u64::from_str_radix(x, 16)) else { continue };
        let props: Props = it.filter_map(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())).collect();
        m.insert(h, props);
    }
    m
}

pub fn save_cache(dir: &Path, m: &HashMap<u64, Props>) -> Result<()> {
    let mut lines: Vec<String> = m
        .iter()
        .map(|(h, p)| {
            let kv: Vec<String> = p.iter().map(|(k, v)| format!("{k}={}", v.replace(['\t', '\n'], " "))).collect();
            format!("{h:016x}\t{}", kv.join("\t"))
        })
        .collect();
    lines.sort();
    std::fs::write(dir.join(CACHE_FILE), lines.join("\n") + "\n")?;
    Ok(())
}

/// Cached leaderboard verdicts (verdicts-v5.tsv): hash -> (works, rotation-safe tag).
pub fn cached_verdicts(dir: &Path) -> HashMap<u64, (bool, String)> {
    let mut m = HashMap::new();
    let Ok(text) = std::fs::read_to_string(dir.join("verdicts-v5.tsv")) else { return m };
    for line in text.lines() {
        let f: Vec<&str> = line.splitn(7, '\t').collect();
        if f.len() < 5 {
            continue;
        }
        if let Ok(h) = u64::from_str_radix(f[0], 16) {
            m.insert(h, (f[1] == "ok", f[4].to_string()));
        }
    }
    m
}

/// vanilla-check verdicts from the vanilla-check track's logs: design hash -> PASS / DIVERGES.
pub fn vanilla_verdicts(repo: &Path) -> HashMap<u64, String> {
    let mut m = HashMap::new();
    let dir = repo.join("designs/piston-door-3x3/tracks/vanilla-check/logs");
    let Ok(rd) = std::fs::read_dir(&dir) else { return m };
    for e in rd.flatten() {
        let Ok(text) = std::fs::read_to_string(e.path()) else { continue };
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("vanilla-check: ") else { continue };
            let Some((path, tail)) = rest.split_once(" (") else { continue };
            let verdict = if tail.contains(") PASS") { "PASS" } else if tail.contains("DIVERGES") { "DIVERGES" } else { continue };
            if let Ok(d) = Design::load(&repo.join(path)) {
                let h = crate::leaderboard::design_hash(&d);
                if verdict == "PASS" || !m.contains_key(&h) {
                    m.insert(h, verdict.to_string());
                }
            }
        }
    }
    m
}

/// Short display name of a source (`HEAD:designs/.../archive/fc2-40b.txt` -> `fc2-40b`).
pub fn display_name(source: &str) -> String {
    let (r, path) = source.split_once(':').unwrap_or(("", source));
    let file = path.rsplit('/').next().unwrap_or(path).trim_end_matches(".txt");
    let file = if file == "BEST" { path.split('/').rev().nth(1).map(|t| format!("{t}/BEST")).unwrap_or(file.to_string()) } else { file.to_string() };
    if r == "HEAD" || r.is_empty() { file } else { format!("{}:{file}", r.trim_start_matches("origin/explore/")) }
}

pub const COLUMNS: &[(&str, &str)] = &[
    ("seamless", "seamless"),
    ("hallway", "hall"),
    ("dents", "dents"),
    ("flush_full", "full trap/flush/deluxe"),
    ("flush_semi", "semi"),
    ("hipster", "hipster"),
    ("sym", "sym axes"),
    ("sync", "sync"),
    ("clean", "clean"),
    ("components", "components"),
    ("dir", "not dir."),
    ("loc", "not loc."),
    ("vanilla", "26.3 vanilla"),
    ("lever_surface", "lever surf."),
    ("repeater_input", "repeater input"),
    ("steps", "steps"),
    ("isolated", "isolated"),
    ("weatherproof", "weatherproof"),
    ("contained", "contained"),
    ("uses", "uses"),
    ("volume", "volume"),
    ("t_player", "player o/ov/c/cv"),
    ("t_repeater", "repeater o/ov/c/cv"),
];

/// Per property: how many designs have it, and the best one by the catalogue's volume and by our
/// block count (ties broken by the other). Properties no design has are the open targets.
pub fn summary(rows: &[(String, usize, &'static str, Props)]) -> String {
    type Test = Box<dyn Fn(&Props) -> bool>;
    let has = |k: &'static str, pat: &'static str| -> Test { Box::new(move |p: &Props| p.get(k).is_some_and(|v| v.contains(pat))) };
    let eq = |k: &'static str, val: &'static str| -> Test { Box::new(move |p: &Props| p.get(k).is_some_and(|v| v == val)) };
    let starts = |k: &'static str, pat: &'static str| -> Test { Box::new(move |p: &Props| p.get(k).is_some_and(|v| v.starts_with(pat))) };
    let comp = |code: &'static str| -> Test { Box::new(move |p: &Props| p.get("components").is_some_and(|v| v.split(' ').any(|c| c == code))) };
    let seam = |tiers: &'static [&'static str]| -> Test { Box::new(move |p: &Props| p.get("seamless").is_some_and(|v| tiers.iter().any(|t| v.starts_with(t)))) };
    let props: Vec<(&str, Test)> = vec![
        ("QUART SEAMLESS or better", seam(&["QUART", "SEMI", "FULL", "SUPER"])),
        ("SEMI SEAMLESS or better", seam(&["SEMI", "FULL", "SUPER"])),
        ("FULL SEAMLESS or better", seam(&["FULL", "SUPER"])),
        ("SUPER SEAMLESS", seam(&["SUPER"])),
        ("hallway-compatible", starts("hallway", "yes")),
        ("DENTLESS", eq("dents", "dentless")),
        ("MINOR DENTS ONLY or better", Box::new(|p: &Props| p.get("dents").is_some_and(|v| v == "dentless" || v == "minor"))),
        ("FULL TRAPDOOR (wall)", has("flush_full", "trapdoor")),
        ("FULL FLUSH", has("flush_full", "flush")),
        ("FULL DELUXE", has("flush_full", "deluxe")),
        ("SEMI FLUSH", has("flush_semi", "flush")),
        ("SEMI DELUXE", has("flush_semi", "deluxe")),
        ("FULL FLOOR HIPSTER", has("hipster", "FULL floor")),
        ("SEMI FLOOR HIPSTER", has("hipster", "floor")),
        ("FULL CEILING HIPSTER", has("hipster", "FULL ceiling")),
        ("SEMI CEILING HIPSTER", has("hipster", "ceiling")),
        ("FULL WALL HIPSTER", has("hipster", "FULL wall")),
        ("SEMI WALL HIPSTER", has("hipster", "wall")),
        ("SYMMETRICAL (1 axis)", Box::new(|p: &Props| p.get("sym").is_some_and(|v| v != "-"))),
        ("FULL SYMMETRICAL (4 axes)", Box::new(|p: &Props| p.get("sym").is_some_and(|v| v.split(' ').count() == 4))),
        ("SEMI SYNC or better", Box::new(|p: &Props| p.get("sync").is_some_and(|v| v != "-"))),
        ("FULL SYNC or better", Box::new(|p: &Props| p.get("sync").is_some_and(|v| v == "FULL" || v == "SUPER"))),
        ("SUPER SYNC", eq("sync", "SUPER")),
        ("CLEAN", eq("clean", "yes")),
        ("NO SLIME", comp("S")),
        ("NO HONEY", comp("H")),
        ("NO GRAVITY BLOCKS", comp("G")),
        ("NO STICKY PISTONS (no sticky surfaces)", comp("K")),
        ("NO OBSERVERS", comp("O")),
        ("NO NOTE BLOCKS", comp("N")),
        ("NO ENTITIES", comp("E")),
        ("NO FLYING MACHINES", comp("F")),
        ("TORCH AND DUST ONLY", comp("TD")),
        ("REDSTONE BLOCK ONLY", comp("RB")),
        ("ZOMBA-", comp("ZA")),
        ("ZOMBI-", comp("ZI")),
        ("CONTAINED", starts("contained", "yes")),
        ("WEATHERPROOF", starts("weatherproof", "yes")),
        ("NOT DIRECTIONAL", eq("dir", "yes")),
        ("UP-TO-DATE (vanilla PASS)", eq("vanilla", "PASS")),
        ("lever on the surface", eq("lever_surface", "yes")),
        ("repeater input works", starts("repeater_input", "yes")),
        ("player steps cannot break it", starts("steps", "yes")),
    ];
    let vol = |p: &Props| p.get("volume").and_then(|v| v.parse::<i64>().ok()).unwrap_or(i64::MAX);
    let mut out = String::from("| property | designs | best by volume (volume, blocks) | best by blocks (blocks, volume) |\n|---|---|---|---|\n");
    let mut open = Vec::new();
    for (name, test) in &props {
        let with: Vec<&(String, usize, &'static str, Props)> = rows.iter().filter(|r| test(&r.3)).collect();
        if with.is_empty() {
            let known = rows.iter().any(|r| {
                let key = match *name {
                    "hallway-compatible" => "hallway",
                    "CONTAINED" => "contained",
                    "WEATHERPROOF" => "weatherproof",
                    "repeater input works" => "repeater_input",
                    "player steps cannot break it" => "steps",
                    _ => "seamless",
                };
                r.3.contains_key(key)
            });
            out.push_str(&format!("| {name} | {} | - | - |\n", if known { "0" } else { "not run" }));
            if known {
                open.push(*name);
            }
            continue;
        }
        let bv = with.iter().min_by_key(|r| (vol(&r.3), r.1, r.0.clone())).unwrap();
        let bb = with.iter().min_by_key(|r| (r.1, vol(&r.3), r.0.clone())).unwrap();
        let v = |r: &&&(String, usize, &'static str, Props)| r.3.get("volume").cloned().unwrap_or("?".into());
        out.push_str(&format!("| {name} | {} | {} ({}, {}) | {} ({}, {}) |\n", with.len(), bv.0, v(&bv), bv.1, bb.0, bb.1, v(&bb)));
    }
    out.push_str(&format!("\nOpen targets (no working design has them yet): {}.\n", if open.is_empty() { "none".to_string() } else { open.join(", ") }));
    out
}

/// The board leaders named by the coordinator (display names).
pub const LEADERS: &[&str] = &[
    "sf-holdA-b38-38b",
    "fc2-40b",
    "lamp-sweep-flat43-43b",
    "piston-door-3x3-fresh-b-mp8-43b",
    "pareto-mid-mp8-leafU-43b",
    "pareto-mid-mp8-lampU-44b",
    "pareto-mid-mp7-leafC-46b",
    "exotic-pm45-trap-45b",
    "sculk-ew-s4r-36b",
    "gravity2-g33-33b",
    "tick-door-open4-218b",
    "fast-clean-c1r-108b",
    "fast-clean-s4c-93b",
    "b38h-obs8b-45b",
];

pub fn render(rows: &[(String, usize, &'static str, Props)]) -> String {
    let mut out = String::new();
    out.push_str("| design | blocks | cat |");
    for (_, h) in COLUMNS {
        out.push_str(&format!(" {h} |"));
    }
    out.push('\n');
    out.push_str(&"|---".repeat(3 + COLUMNS.len()));
    out.push_str("|\n");
    for (name, blocks, cat, p) in rows {
        out.push_str(&format!("| {name} | {blocks} | {cat} |"));
        for (k, _) in COLUMNS {
            let v = p.get(*k).map(String::as_str).unwrap_or("");
            let v = if *k == "volume" { p.get("volume_dims").map(|d| format!("{v} ({d})")).unwrap_or(v.to_string()) } else { v.to_string() };
            out.push_str(&format!(" {} |", v.replace('|', "/")));
        }
        out.push('\n');
    }
    out.push_str(
        "\ncomponents: S no slime, H no honey, G no gravity blocks, K no sticky surfaces, O no observers, N no note blocks, E no entities, \
         F no flying machines, TD torch and dust only, RB redstone block only, ZA zomba-, ZI zombi- (no clocks holds for every door: the judge requires idle). \
         Timings o/ov/c/cv = opening, opening visible, closing, closing visible time in ticks after the input. Definitions: tracks/props/NOTES.md.\n",
    );
    out
}
