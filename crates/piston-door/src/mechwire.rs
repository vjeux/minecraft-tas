//! `mctas door mech-wire`: wire a mechanism from its ideal-input schedule, i.e. estimate (and
//! build) the control a mechanism found by `door mech-synth` (or drawn by hand) costs.
//!
//! mech-synth ranks mechanisms by their own blocks and fewest control *events*; route2 routes
//! the nets of a wired design. This joins them:
//!
//! 1. **Measure the mechanism with ideal inputs.** The schedule (`door ideal` format: redstone
//!    blocks set and cleared next to each piston at chosen ticks, the same on both edges for a
//!    symmetric schedule) is replayed for one open and one close phase in the simulator, and the
//!    recorded frames go through route2's analysis ([`crate::route2::analyze`]): which cells hold
//!    a piston when, which cells the machine sweeps, the cells next to moving slime, and what an
//!    observer watching any cell the machine changes would emit (motion triggers). The input
//!    cells themselves are ignored (they become free cells for wiring).
//! 2. **Wanted waves.** Every piston cell must get exactly the power its ideal input gave it
//!    (bit t = an input redstone block touches the piston at the end of tick t); every other
//!    piston cell must stay unpowered (route2's receivers, masked by occupancy).
//! 3. **A lever.** For every lever spot (a free cell near the receivers, attached to an existing
//!    unmoving block or to a new one), route2's router ([`crate::route2::route_net`]) connects
//!    every receiver from the lever (and from observers watching the machine) with dust,
//!    repeaters, observers and conductor blocks, reproducing each wave tick for tick (up to
//!    `--tolerance`). With `--partial`, a receiver that needs pulses from several sources (an
//!    elevator piston pulsed early by the lever and twice by the elevator's window) is
//!    connected in several steps.
//! 4. **Rank and judge.** Mechanism + lever + wiring, fewest blocks first; the best are screened
//!    and judged (`--verify`), the passing ones written out.
//!
//! **Retiming** (`--retime`): a mech-synth schedule fires its steps at `gap`-tick intervals
//! from tick 0, which no wiring produces (an observer on the lever fires 2 gt after the flip).
//! The schedule's steps (distinct set ticks) are re-timed to chain ticks (step k at an odd set
//! tick: an observer turning on at T acts like a block set at T-1), every assignment with
//! step gaps from `--gaps` is replayed with ideal inputs, and the working ones are wired.

use crate::ideal::Schedule;
use crate::route2::{self, Measured, Model, Problem, READ_LATENT, Receiver, RouteOptions, T, Wave};
use crate::synth::{Kind, Node};
use crate::block_count;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pool, Pos, SandboxOptions, Worker};
use std::collections::{BTreeMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const FLOOR: &str = "minecraft:smooth_stone";
const NONSTICK: &str = "minecraft:white_glazed_terracotta[facing=north]";

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn manhattan(a: Pos, b: Pos) -> i32 {
    (0..3).map(|i| (a[i] - b[i]).abs()).sum()
}
fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}
fn bname(s: &str) -> &str {
    short(mctas::blockstate::block_name(s))
}
fn is_piston(s: &str) -> bool {
    matches!(bname(s), "piston" | "sticky_piston")
}
fn facing_name(d: Pos) -> &'static str {
    match d {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}
fn piston_facing(s: &str) -> Option<Pos> {
    let b = mctas::blockstate::BlockState::parse(s).ok()?;
    Some(match b.get("facing")? {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}

/// A mechanism measured with its ideal inputs.
#[derive(Clone)]
pub struct Mech {
    /// The mechanism without its input cells (door port kept, no lever).
    pub bare: Design,
    pub meas: Measured,
    /// Every receiver cell: (cell, reader code, wanted wave).
    pub wants: Vec<(Pos, Pos, Wave)>,
    pub inputs: HashSet<Pos>,
}

impl Mech {
    /// The wanted waves of the cells that need power at some tick.
    pub fn describe(&self) -> Vec<String> {
        self.wants
            .iter()
            .filter(|(_, _, w)| !w.is_off())
            .map(|(p, f, w)| format!("{} {} [{}]", route2::fmt_pos(*p), if *f == READ_LATENT { "qc".to_string() } else { route2::fmt_pos(*f) }, w.describe()))
            .collect()
    }
}

/// Replay `s` (one open and one close phase) and measure the machine.
pub fn measure_ideal(w: &mut Worker, s: &Schedule, vars: &BTreeMap<String, i64>) -> Result<Mech> {
    let d = s.design().clone();
    let inputs: HashSet<Pos> = s.input_cells().into_iter().collect();
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Mech> {
        crate::video::place(w, sb, &d)?;
        let (lo, hi) = d.bounds().context("empty design")?;
        let (lo, hi) = ([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]);
        let rest = w.get_region(sb, lo, hi)?;
        let mut frames: Vec<Vec<mctas::sim::Region>> = Vec::new();
        let mut idle = true;
        for open in [true, false] {
            let evs = s.events_of(open, vars)?;
            let mut f = Vec::with_capacity(T);
            let mut i = 0;
            for t in 0..T {
                f.push(w.get_region(sb, lo, hi)?);
                while i < evs.len() && evs[i].0 as usize == t {
                    let (_, p, st) = &evs[i];
                    let state = st.clone().unwrap_or_else(|| "minecraft:air".to_string());
                    w.set_blocks(sb, &[(*p, state)], PlaceMode::Update)?;
                    i += 1;
                }
                if t + 1 < T {
                    w.step(sb, 1)?;
                }
            }
            if i < evs.len() {
                bail!("schedule has events after tick {}", T - 1);
            }
            idle &= w.status(sb)?.idle;
            frames.push(f);
        }
        let mut bare = d.clone();
        for p in &inputs {
            bare.blocks.remove(p);
        }
        bare.ports.remove("lever");
        let meas = route2::analyze(&bare, &rest, &frames, idle, &inputs);
        // Wanted waves: an input redstone block touching the piston (not on its face).
        let mut wants = Vec::new();
        for &(p, f) in &meas.pistons {
            let mut wv = Wave::OFF;
            if f != READ_LATENT && f != route2::READ_DIRECT && f != route2::READ_QC {
                for ph in 0..2 {
                    for t in 0..T {
                        let fr = &frames[ph][t];
                        if !fr.get(p).is_some_and(is_piston) {
                            continue;
                        }
                        let face = fr.get(p).and_then(piston_facing).map(|d| add(p, d));
                        let up = add(p, [0, 1, 0]);
                        let is_input = |c: Pos| inputs.contains(&c) && fr.get(c).is_some_and(|s| bname(s) == "redstone_block");
                        // Direct, or through the quasi-connectivity cell above the piston.
                        let powered = FACES.iter().map(|d| add(p, *d)).any(|c| Some(c) != face && is_input(c))
                            || FACES.iter().map(|d| add(up, *d)).any(|c| c != p && is_input(c));
                        if powered {
                            wv.0[ph] |= 1 << t;
                        }
                    }
                }
            }
            wants.push((p, f, wv));
        }
        Ok(Mech { bare, meas, wants, inputs })
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// Where the lever goes: its cell and state, and its attachment (a new block, or None when it
/// hangs on an existing one).
#[derive(Clone, Debug)]
pub struct LeverSpot {
    pub lever: Pos,
    pub state: String,
    pub attach: Pos,
    pub new_block: Option<String>,
}

impl LeverSpot {
    pub fn describe(&self) -> String {
        format!(
            "lever {} on {}{}",
            route2::fmt_pos(self.lever),
            route2::fmt_pos(self.attach),
            match &self.new_block {
                Some(b) if b.contains("observer") => " (new observer)",
                Some(_) => " (new)",
                None => "",
            }
        )
    }
    /// Parse "x,y,z@ax,ay,az" (lever cell @ attachment cell; ":obs" = a new observer there).
    pub fn parse(s: &str, mech: &Mech) -> Result<LeverSpot> {
        let (s, obs) = match s.strip_suffix(":obs") {
            Some(x) => (x, true),
            None => (s, false),
        };
        let (a, b) = s.split_once('@').context("lever spot: x,y,z@ax,ay,az")?;
        let p = |t: &str| -> Result<Pos> {
            let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "expected x,y,z, got {t:?}");
            Ok([v[0], v[1], v[2]])
        };
        let (l, k) = (p(a)?, p(b)?);
        let d = [k[0] - l[0], k[1] - l[1], k[2] - l[2]];
        anyhow::ensure!(manhattan(d, [0, 0, 0]) == 1, "the attachment must touch the lever");
        let new_block = if obs {
            Some(format!("minecraft:observer[facing={},powered=false]", facing_name([-d[0], -d[1], -d[2]])))
        } else if mech.bare.blocks.contains_key(&k) {
            None
        } else {
            Some(if mech.meas.drag.contains(&k) { NONSTICK } else { FLOOR }.to_string())
        };
        Ok(LeverSpot { lever: l, state: lever_state(d), attach: k, new_block })
    }
}

/// The lever state hanging on the block at `lever + d`.
fn lever_state(d: Pos) -> String {
    match d {
        [0, -1, 0] => "minecraft:lever[face=floor,facing=north,powered=false]".to_string(),
        [0, 1, 0] => "minecraft:lever[face=ceiling,facing=north,powered=false]".to_string(),
        _ => format!("minecraft:lever[face=wall,facing={},powered=false]", facing_name([-d[0], -d[1], -d[2]])),
    }
}

/// Lever spots within `radius` (Manhattan) of the `near` cells, on free cells the machine never
/// uses, attached to an unmoving conductor block of the mechanism or to a new block.
pub fn lever_spots(mech: &Mech, near: &[Pos], radius: i32) -> Result<Vec<LeverSpot>> {
    let forbidden = route2::forbidden_cells(&mech.bare)?;
    let used = |c: Pos| mech.bare.blocks.contains_key(&c) || mech.meas.occupied.contains(&c) || forbidden.contains(&c) || mech.meas.moving.contains(&c);
    let mut cells: HashSet<Pos> = HashSet::new();
    for &n in near {
        for dx in -radius..=radius {
            for dy in -radius..=radius {
                for dz in -radius..=radius {
                    let c = add(n, [dx, dy, dz]);
                    if manhattan(c, n) <= radius && !used(c) {
                        cells.insert(c);
                    }
                }
            }
        }
    }
    let mut cells: Vec<Pos> = cells.into_iter().collect();
    cells.sort();
    // A lever (or its block, which it powers) next to a piston or the cell above one would
    // power that piston at every flip: skip those spots at once.
    let piston_cells: Vec<Pos> = mech.meas.pistons.iter().map(|x| x.0).collect();
    let powers_piston = |c: Pos| piston_cells.iter().any(|&q| manhattan(q, c) == 1 || manhattan(add(q, [0, 1, 0]), c) == 1);
    cells.retain(|&c| !powers_piston(c));
    let mut out = Vec::new();
    for l in cells {
        for d in FACES {
            let k = add(l, d);
            if powers_piston(k) && !mech.bare.blocks.contains_key(&k) {
                // A new block there: an observer still outputs only out of its back, so only
                // the plain-block variant is useless.
                let d2 = [-d[0], -d[1], -d[2]];
                let back = add(k, [-d2[0], -d2[1], -d2[2]]);
                if !used(k) && !mech.meas.drag.contains(&k) {
                    let _ = back;
                    let obs = format!("minecraft:observer[facing={},powered=false]", facing_name(d2));
                    out.push(LeverSpot { lever: l, state: lever_state(d), attach: k, new_block: Some(obs) });
                }
                continue;
            }
            let new_block = match mech.bare.blocks.get(&k) {
                Some(s) => {
                    let n = Node::new(k, s);
                    if n.kind != Kind::Block || !n.sturdy || mech.meas.moving.contains(&k) {
                        continue;
                    }
                    None
                }
                None => {
                    if used(k) {
                        continue;
                    }
                    // Also an observer as the lever's block: it watches the lever (one pulse
                    // 2 gt after each flip, out of its back): the lever-observer gadget.
                    if !mech.meas.drag.contains(&k) {
                        let obs = format!("minecraft:observer[facing={},powered=false]", facing_name([-d[0], -d[1], -d[2]]));
                        out.push(LeverSpot { lever: l, state: lever_state(d), attach: k, new_block: Some(obs) });
                    }
                    Some(if mech.meas.drag.contains(&k) { NONSTICK } else { FLOOR }.to_string())
                }
            };
            out.push(LeverSpot { lever: l, state: lever_state(d), attach: k, new_block });
        }
    }
    Ok(out)
}

/// The mechanism with the lever placed.
pub fn with_lever(mech: &Mech, spot: &LeverSpot) -> Design {
    let mut d = mech.bare.clone();
    d.ports.insert("lever".to_string(), spot.lever);
    d.blocks.insert(spot.lever, spot.state.clone());
    if let Some(b) = &spot.new_block {
        d.blocks.insert(spot.attach, b.clone());
    }
    d
}

/// Knobs of one wiring run.
#[derive(Clone)]
pub struct WireOptions {
    pub route: RouteOptions,
    pub budget: u32,
    pub threads: usize,
    /// Free events: receivers (cells) that accept a single 2 gt pulse at any tick in this range
    /// instead of their wanted wave ((cell, lo, hi)).
    pub free: Vec<(Pos, usize, usize)>,
    /// Timing alternatives per receiver cell (see [`timing_alts`]).
    pub alts: Vec<(Pos, Vec<Wave>)>,
    /// Free receivers also accept 3 pulses per edge (the last in range).
    pub odd: bool,
    /// Print what the router finds for each receiver alone.
    pub debug: bool,
    /// Screen every lever spot cheaply first (each receiver alone) and fully route only the
    /// best this many (0: route every spot fully).
    pub screen_top: usize,
}

/// A wired candidate.
#[derive(Clone)]
pub struct Wired {
    pub design: Design,
    pub blocks: usize,
    pub desc: String,
}

/// Cheap screen of a lever spot: each receiver routed alone (no sharing); the sum of the best
/// costs, None if some receiver cannot be connected at all.
pub fn spot_estimate(mech: &Mech, spot: &LeverSpot, o: &WireOptions) -> Option<usize> {
    let mut quick = o.clone();
    quick.route.beam = 2;
    quick.route.hits = 1;
    quick.route.threads = 1;
    quick.debug = false;
    quick.screen_top = 0;
    let base = with_lever(mech, spot);
    // Reuse wire_spot's receiver setup by routing each receiver as the only one.
    let mut total = 0usize;
    let recv = receivers_of(mech, spot, &quick)?;
    let (pb_base, wants) = (base, recv.1);
    let (lo, hi) = pb_base.bounds()?;
    let mg = quick.route.margin;
    let pb = Problem {
        base: &pb_base,
        meas: &mech.meas,
        lever: Some(spot.lever),
        forbidden: route2::forbidden_cells(&pb_base).ok()?,
        lo: [lo[0] - mg, lo[1] - mg, lo[2] - mg],
        hi: [hi[0] + mg, hi[1] + mg, hi[2] + mg],
        wants,
    };
    for r in &recv.0 {
        let found = route2::route_net(&pb, std::slice::from_ref(r), quick.budget, &quick.route);
        total += found.first()?.len();
    }
    Some(total)
}

/// The receivers `wire_spot` routes for this spot and the wanted (allowed) waves, or None if the
/// lever already powers a piston it must not.
fn receivers_of(mech: &Mech, spot: &LeverSpot, o: &WireOptions) -> Option<(Vec<Receiver>, Vec<(Pos, Pos, Wave)>)> {
    let base = with_lever(mech, spot);
    let free_of = |p: Pos| -> Option<(usize, usize, bool)> { o.free.iter().find(|x| x.0 == p).map(|&(_, a, b)| (a, b, o.odd)) };
    let alts_of = |p: Pos| -> Vec<Wave> { o.alts.iter().find(|x| x.0 == p).map(|x| x.1.clone()).unwrap_or_default() };
    let allowed_of = |p: Pos, w: Wave| -> Wave {
        let w = alts_of(p).into_iter().fold(w, |a, x| a.or(x));
        match free_of(p) {
            Some((a, b, odd)) => {
                let from = if odd { 1 } else { a };
                let mut x = w;
                for t in from..=(b + 1).min(T - 1) {
                    x.0[0] |= 1 << t;
                    x.0[1] |= 1 << t;
                }
                x
            }
            None => w,
        }
    };
    let wants: Vec<(Pos, Pos, Wave)> = mech.wants.iter().map(|&(p, f, w)| (p, f, allowed_of(p, w))).collect();
    let m = Model::new(&base.blocks, Some(spot.lever), &mech.meas.fixed, &mech.meas.extra);
    let mut recv = Vec::new();
    for &(p, f, w) in &mech.wants {
        let r = Receiver { pos: p, facing: f, want: w, rest: m.piston_wave(p, f), mask: mech.meas.mask(p), alts: alts_of(p), free: free_of(p) };
        if r.accepts(r.rest, o.route.tolerance) {
            continue;
        }
        let extra = r.rest.minus(allowed_of(p, w));
        if extra.0[0] & r.mask.0[0] != 0 || extra.0[1] & r.mask.0[1] != 0 {
            return None;
        }
        recv.push(r);
    }
    let first_on = |r: &Receiver| -> u32 {
        let w = Wave([r.want.0[0] & r.mask.0[0], r.want.0[1] & r.mask.0[1]]);
        w.0[0].trailing_zeros().min(w.0[1].trailing_zeros())
    };
    recv.sort_by_key(|r| (first_on(r), r.pos));
    Some((recv, wants))
}

/// Spot outcomes of the last [`wire_all`] (rejected before routing: the lever powers a piston;
/// no routing found; routings the full static model rejects).
pub static SPOT_EARLY: AtomicUsize = AtomicUsize::new(0);
pub static SPOT_NONE: AtomicUsize = AtomicUsize::new(0);
pub static SPOT_MODEL: AtomicUsize = AtomicUsize::new(0);

/// Route every receiver of `mech` from the lever at `spot`; candidates the static model accepts.
pub fn wire_spot(mech: &Mech, spot: &LeverSpot, o: &WireOptions) -> Result<Vec<Wired>> {
    let base = with_lever(mech, spot);
    let (lo, hi) = base.bounds().context("empty design")?;
    let mg = o.route.margin;
    // A free receiver accepts 1 (or 3, `o.odd`) 2 gt pulses per edge, the last in its range.
    let free_of = |p: Pos| -> Option<(usize, usize, bool)> { o.free.iter().find(|x| x.0 == p).map(|&(_, a, b)| (a, b, o.odd)) };
    let alts_of = |p: Pos| -> Vec<Wave> { o.alts.iter().find(|x| x.0 == p).map(|x| x.1.clone()).unwrap_or_default() };
    let allowed_of = |p: Pos, w: Wave| -> Wave {
        let w = alts_of(p).into_iter().fold(w, |a, x| a.or(x));
        match free_of(p) {
            Some((a, b, odd)) => {
                let from = if odd { 1 } else { a };
                let mut x = w;
                for t in from..=(b + 1).min(T - 1) {
                    x.0[0] |= 1 << t;
                    x.0[1] |= 1 << t;
                }
                x
            }
            None => w,
        }
    };
    let wants: Vec<(Pos, Pos, Wave)> = mech.wants.iter().map(|&(p, f, w)| (p, f, allowed_of(p, w))).collect();
    let pb = Problem {
        base: &base,
        meas: &mech.meas,
        lever: Some(spot.lever),
        forbidden: route2::forbidden_cells(&base)?,
        lo: [lo[0] - mg, lo[1] - mg, lo[2] - mg],
        hi: [hi[0] + mg, hi[1] + mg, hi[2] + mg],
        wants: wants.clone(),
    };
    let m = pb.model(&base.blocks);
    let mut recv = Vec::new();
    for &(p, f, w) in &mech.wants {
        let r = Receiver { pos: p, facing: f, want: w, rest: m.piston_wave(p, f), mask: mech.meas.mask(p), alts: alts_of(p), free: free_of(p) };
        if r.accepts(r.rest, o.route.tolerance) {
            continue;
        }
        // Power the receiver already gets and must not (the lever next to a piston...): no
        // wiring can take it away.
        let allowed = allowed_of(p, w);
        let extra = r.rest.minus(allowed);
        if extra.0[0] & r.mask.0[0] != 0 || extra.0[1] & r.mask.0[1] != 0 {
            SPOT_EARLY.fetch_add(1, Ordering::Relaxed);
            return Ok(Vec::new());
        }
        recv.push(r);
    }
    // Earliest power first (with --order-limit, the order the router follows).
    let first_on = |r: &Receiver| -> u32 {
        let w = Wave([r.want.0[0] & r.mask.0[0], r.want.0[1] & r.mask.0[1]]);
        w.0[0].trailing_zeros().min(w.0[1].trailing_zeros())
    };
    recv.sort_by_key(|r| (first_on(r), r.pos));
    if o.debug {
        // Each receiver alone from the bare lever: what the router finds for it.
        for r in &recv {
            let alone = route2::route_net(&pb, std::slice::from_ref(r), o.budget, &o.route);
            eprintln!(
                "  [debug] {} {}: want [{}] has [{}] mask [{}]: {} routings, best {}",
                spot.describe(),
                route2::fmt_pos(r.pos),
                r.want.describe(),
                r.rest.describe(),
                r.mask.describe(),
                alone.len(),
                alone.first().map(|c| c.iter().map(|(p, x)| format!("{} {}", route2::fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" ")).unwrap_or("-".into())
            );
        }
    }
    let cands = route2::route_net(&pb, &recv, o.budget, &o.route);
    let n_cands = cands.len();
    if o.debug && cands.is_empty() {
        // Greedy diagnosis: connect the receivers one at a time (first routing each), report the
        // first one that cannot be connected on top of the others.
        let mut cur = base.clone();
        let mut total = 0usize;
        for r in &recv {
            let pb2 = Problem { base: &cur, meas: pb.meas, lever: pb.lever, forbidden: pb.forbidden.clone(), lo: pb.lo, hi: pb.hi, wants: pb.wants.clone() };
            let m2 = pb2.model(&cur.blocks);
            let r2 = Receiver { rest: m2.piston_wave(r.pos, r.facing), ..r.clone() };
            if r2.accepts(r2.rest, o.route.tolerance) {
                eprintln!("  [greedy] {} already served", route2::fmt_pos(r.pos));
                continue;
            }
            let found = route2::route_net(&pb2, std::slice::from_ref(&r2), o.budget.saturating_sub(total as u32), &o.route);
            match found.first() {
                Some(c) => {
                    total += c.len();
                    eprintln!("  [greedy] {} [{}] +{}: {}", route2::fmt_pos(r.pos), r.want.describe(), c.len(), c.iter().map(|(p, x)| format!("{} {}", route2::fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" "));
                    let mut next = cur.clone();
                    for (p, x) in c {
                        next.blocks.insert(*p, x.clone());
                    }
                    cur = next;
                }
                None => {
                    eprintln!("  [greedy] {} [{}] has [{}]: NO ROUTE (after {total} blocks)", route2::fmt_pos(r.pos), r.want.describe(), r2.rest.describe());
                }
            }
        }
    }
    let mut out = Vec::new();
    for c in cands {
        let mut new = base.clone();
        for (p, x) in &c {
            new.blocks.insert(*p, x.clone());
        }
        let nm = pb.model(&new.blocks);
        let bad: Vec<String> = mech
            .wants
            .iter()
            .filter(|&&(p, f, w)| {
                let r = Receiver { pos: p, facing: f, want: w, rest: Wave::OFF, mask: mech.meas.mask(p), alts: alts_of(p), free: free_of(p) };
                !r.accepts(nm.piston_wave(p, f), o.route.tolerance)
            })
            .map(|&(p, f, w)| format!("{}{} gets [{}] wants [{}]", route2::fmt_pos(p), if f == READ_LATENT { " qc" } else { "" }, nm.piston_wave(p, f).describe(), w.describe()))
            .collect();
        if !bad.is_empty() {
            if o.debug {
                eprintln!("  [reject] {} blocks: {}", c.len(), bad.join("; "));
            }
            continue;
        }
        let desc = format!(
            "{}; +{}: {}",
            spot.describe(),
            c.len(),
            c.iter().map(|(p, x)| format!("{} {}", route2::fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" ")
        );
        out.push(Wired { blocks: block_count(&new), design: new, desc });
    }
    if out.is_empty() {
        if n_cands == 0 { &SPOT_NONE } else { &SPOT_MODEL }.fetch_add(1, Ordering::Relaxed);
    }
    Ok(out)
}

/// Wire `mech` from every spot (in parallel); all candidates, fewest blocks first.
pub fn wire_all(mech: &Mech, spots: &[LeverSpot], o: &WireOptions, log: &(dyn Fn(&str) + Sync)) -> Vec<Wired> {
    for c in [&SPOT_EARLY, &SPOT_NONE, &SPOT_MODEL] {
        c.store(0, Ordering::Relaxed);
    }
    let spots: Vec<LeverSpot> = if o.screen_top > 0 && spots.len() > o.screen_top {
        // Cheap screen in parallel, then the full routing on the best spots only.
        let est: Mutex<Vec<(usize, usize)>> = Mutex::new(Vec::new());
        let next = AtomicUsize::new(0);
        std::thread::scope(|s| {
            for _ in 0..o.threads.max(1) {
                s.spawn(|| loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= spots.len() {
                        break;
                    }
                    if let Some(c) = spot_estimate(mech, &spots[i], o) {
                        est.lock().unwrap().push((c, i));
                    }
                });
            }
        });
        let mut est = est.into_inner().unwrap();
        est.sort();
        log(&format!(
            "  screen: {} of {} spots reach every receiver alone; best estimates {:?}",
            est.len(),
            spots.len(),
            est.iter().take(8).map(|(c, i)| format!("{c} ({})", spots[*i].describe())).collect::<Vec<_>>()
        ));
        est.into_iter().take(o.screen_top).map(|(_, i)| spots[i].clone()).collect()
    } else {
        spots.to_vec()
    };
    let v = wire_all_inner(mech, &spots, o, log);
    log(&format!(
        "  spots: {} rejected (the lever powers a piston), {} without routing, {} with routings the model rejects",
        SPOT_EARLY.load(Ordering::Relaxed),
        SPOT_NONE.load(Ordering::Relaxed),
        SPOT_MODEL.load(Ordering::Relaxed)
    ));
    v
}

fn wire_all_inner(mech: &Mech, spots: &[LeverSpot], o: &WireOptions, log: &(dyn Fn(&str) + Sync)) -> Vec<Wired> {
    // Few spots: the threads go to each spot's searches instead.
    let mut o = o.clone();
    o.route.threads = (o.threads / spots.len().max(1)).max(1);
    let o = &o;
    let out: Mutex<Vec<Wired>> = Mutex::new(Vec::new());
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let best = Mutex::new(usize::MAX);
    let t0 = std::time::Instant::now();
    let last = Mutex::new(std::time::Instant::now());
    std::thread::scope(|s| {
        for _ in 0..o.threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= spots.len() {
                    break;
                }
                let r = wire_spot(mech, &spots[i], o).unwrap_or_default();
                if let Some(b) = r.iter().map(|w| w.blocks).min() {
                    let mut bb = best.lock().unwrap();
                    if b < *bb {
                        *bb = b;
                    }
                }
                out.lock().unwrap().extend(r);
                let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                let mut l = last.lock().unwrap();
                if l.elapsed().as_secs_f64() > 30.0 {
                    *l = std::time::Instant::now();
                    log(&format!("  {d} / {} lever spots routed, best {} blocks, {:.0}s", spots.len(), *best.lock().unwrap(), t0.elapsed().as_secs_f64()));
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by(|a, b| a.blocks.cmp(&b.blocks).then_with(|| a.desc.cmp(&b.desc)));
    let mut seen = HashSet::new();
    v.retain(|w| seen.insert(crate::search::design_hash(&w.design)));
    v
}

// ---- catalogs -----------------------------------------------------------------------------

/// Parse one mech-synth catalog entry ("#355 22 blocks | ... | SYM 10 events: PSeast@(-1,3,0)
/// ... [M pulse...]" followed by "    added: Seast@(-1,3,0) slime@(0,3,0) ...") on top of the
/// base design: the mechanism and its symmetric sequence.
pub fn catalog_entry(text: &str, id: usize, base: &Design) -> Result<(crate::mechsynth::World, Vec<crate::mechsynth::Ev>)> {
    use crate::mechsynth::{Act, Ev, Kind as MKind, World};
    let head = format!("#{id} ");
    let mut lines = text.lines();
    let line = lines.by_ref().find(|l| l.starts_with(&head)).with_context(|| format!("no entry #{id}"))?;
    let added_line = lines.next().context("entry without an 'added:' line")?;
    let added = added_line.trim().strip_prefix("added:").context("expected 'added:'")?;
    let parse_item = |tok: &str| -> Result<(String, [i8; 3])> {
        let (code, pos) = tok.split_once('@').with_context(|| format!("bad item {tok:?}"))?;
        let pos = pos.trim_start_matches('(').trim_end_matches(')');
        let p = crate::mechsynth::parse_p(pos)?;
        Ok((code.to_string(), p))
    };
    let kind_of = |code: &str| -> Result<MKind> {
        Ok(match code {
            "slime" => MKind::Slime,
            "honey" => MKind::Honey,
            "obsidian" => MKind::Obsidian,
            "gt" => MKind::Gt,
            "stone" => MKind::Stone,
            "door" => MKind::Door,
            c => {
                let sticky = c.starts_with('S');
                let ext = c.ends_with('+');
                let name = c[1..].trim_end_matches('+');
                let f = ["east", "west", "up", "down", "south", "north"].iter().position(|d| *d == name).with_context(|| format!("bad piston code {c:?}"))? as u8;
                MKind::Piston { sticky, f, ext }
            }
        })
    };
    let w0 = crate::mechsynth::world_of_design(base)?;
    let mut b = w0.b.clone();
    for tok in added.split_whitespace() {
        let (code, p) = parse_item(tok)?;
        b.push((p, kind_of(&code)?));
    }
    b.sort();
    let world = World { pow: World::rest_pow(&b), b };
    let sym = line.split("SYM").nth(1).context("entry has no symmetric sequence")?;
    let sym = sym.split_once(':').context("SYM n events: ...")?.1;
    let sym = sym.split('[').next().unwrap_or(sym);
    let mut seq = Vec::new();
    for tok in sym.split_whitespace() {
        let act = match tok.chars().next() {
            Some('P') => Act::P,
            Some('L') => Act::L,
            _ => bail!("bad event {tok:?}"),
        };
        let (_, p) = parse_item(&tok[1..])?;
        let id = world.b.iter().position(|(q, _)| *q == p).with_context(|| format!("no piston at {p:?}"))?;
        seq.push(Ev { id: id as u8, act });
    }
    Ok((world, seq))
}

/// The ideal schedule (symmetric, `gap`-tick steps) of a catalog entry.
pub fn catalog_schedule(text: &str, id: usize, base: &Design, gap: u32) -> Result<Schedule> {
    let (world, seq) = catalog_entry(text, id, base)?;
    let sol = crate::mechsynth::Solution { open: seq.clone(), close: seq.clone(), sym: Some(seq), states: 0 };
    let plan = crate::mechsynth::ideal_plan(&world, &sol, true, gap, &format!("k{id}"))?;
    Ok(Schedule::from_events(plan.design, &plan.events))
}

/// Greedy compression of a schedule's step times: steps at `first`, then `start_gap` apart;
/// each gap in turn shrunk to the smallest of `gaps` that still works with ideal inputs.
pub fn compress(pool: &Pool, steps: &Steps, first: u32, start_gap: u32, gaps: &[u32], log: &(dyn Fn(&str) + Sync)) -> Option<Vec<u32>> {
    let n = steps.ticks.len();
    let mut cur: Vec<u32> = (0..n).map(|k| first + start_gap * k as u32).collect();
    let works = |a: &Vec<u32>| -> bool { !working_retimes(pool, steps, vec![a.clone()], 0, 2).is_empty() };
    if !works(&cur) {
        log(&format!("  retime {cur:?} does not work"));
        return None;
    }
    let mut sorted: Vec<u32> = gaps.to_vec();
    sorted.sort();
    for i in 1..n {
        let g0 = cur[i] - cur[i - 1];
        // Try every smaller gap at once (in parallel), keep the smallest that works.
        let tries: Vec<Vec<u32>> = sorted
            .iter()
            .filter(|&&g| g < g0)
            .map(|&g| {
                let mut a = cur.clone();
                let d = g0 - g;
                for x in a.iter_mut().skip(i) {
                    *x -= d;
                }
                a
            })
            .collect();
        if tries.is_empty() {
            continue;
        }
        let ok = working_retimes(pool, steps, tries, 0, 2);
        if let Some((a, _)) = ok.into_iter().min_by_key(|(a, _)| a[i]) {
            cur = a;
        }
    }
    Some(cur)
}

/// Events of a symmetric schedule: (cell, step) pairs set on both edges. For each, the shifts
/// (in ticks, both edges together) that keep the door working with ideal inputs.
pub fn event_windows(pool: &Pool, steps: &Steps, at: &[u32], shifts: &[i32]) -> Vec<((Pos, usize), Vec<i32>)> {
    let mut evs: Vec<(Pos, usize)> = steps.events.iter().map(|&(_, p, s, _)| (p, s)).collect();
    evs.sort();
    evs.dedup();
    let mut items: Vec<((Pos, usize), i32)> = Vec::new();
    for e in &evs {
        for &d in shifts {
            items.push((*e, d));
        }
    }
    let opts = crate::ideal::IdealOptions { cycles: 2, offsets: crate::ideal::offsets(0).into_iter().take(2).collect(), trace: None };
    let res = pool.map(items.clone(), |w, _, ((p, s), d)| {
        let sc = steps.retimed_shift(at, p, s, d);
        Ok(match sc {
            Some(sc) => crate::ideal::run(w, &sc, &BTreeMap::new(), &opts).is_ok(),
            None => false,
        })
    });
    let mut out: Vec<((Pos, usize), Vec<i32>)> = evs.iter().map(|e| (*e, Vec::new())).collect();
    for ((e, d), r) in items.into_iter().zip(res) {
        if r.unwrap_or(false) {
            if let Some(x) = out.iter_mut().find(|x| x.0 == e) {
                x.1.push(d);
            }
        }
    }
    out
}

/// Timing alternatives per receiver cell: every event (input cell, step) that powers the cell may
/// move by any shift of its ideal-input window (`win`, others fixed); the cell accepts every
/// combination (product over its events, at most `cap` waves).
pub fn timing_alts(mech: &Mech, steps: &Steps, at: &[u32], win: &[((Pos, usize), Vec<i32>)], cap: usize) -> Vec<(Pos, Vec<Wave>)> {
    let mut out = Vec::new();
    for &(p, f, want) in &mech.wants {
        if want.is_off() || f == READ_LATENT {
            continue;
        }
        // The events behind this cell's wave and what each contributes.
        let mut parts: Vec<(Wave, Vec<i32>)> = Vec::new();
        let mut explained = Wave::OFF;
        for ((c, s), ds) in win {
            let touches = manhattan(p, *c) == 1 || (manhattan(add(p, [0, 1, 0]), *c) == 1 && p != *c);
            if !touches {
                continue;
            }
            let bits = steps.event_bits(at, *c, *s);
            let contrib = Wave([bits.0[0] & want.0[0], bits.0[1] & want.0[1]]);
            if contrib.is_off() {
                continue;
            }
            explained = explained.or(contrib);
            let ds: Vec<i32> = if ds.is_empty() { vec![0] } else { ds.clone() };
            parts.push((contrib, ds));
        }
        let fixed = want.minus(explained);
        let mut alts: Vec<Wave> = vec![fixed];
        for (contrib, ds) in &parts {
            let mut next = Vec::new();
            for a in &alts {
                for &d in ds {
                    next.push(a.or(contrib.shift(d)));
                }
            }
            next.sort_by_key(|w| w.0);
            next.dedup();
            alts = next;
            if alts.len() > cap {
                break;
            }
        }
        if alts.len() > 1 && alts.len() <= cap {
            out.push((p, alts));
        }
    }
    out
}

/// Receivers whose only power is one 2 gt pulse per edge from an event that may move by at least
/// `min_width` ticks: (cell, first allowed start bit, last allowed start bit).
pub fn free_receivers(mech: &Mech, steps: &Steps, at: &[u32], win: &[((Pos, usize), Vec<i32>)], min_width: i32) -> Vec<(Pos, usize, usize)> {
    let mut out = Vec::new();
    for ((c, s), ds) in win {
        let (Some(&dmin), Some(&dmax)) = (ds.iter().min(), ds.iter().max()) else { continue };
        if dmax - dmin < min_width {
            continue;
        }
        // Only plain pulses.
        let pulse = steps.events.iter().filter(|e| e.1 == *c && e.2 == *s).all(|e| matches!(e.3, End::After(2)));
        if !pulse {
            continue;
        }
        let t = at[*s] as i32 + 1;
        let bits = 3u64 << t;
        for &(p, _, w) in &mech.wants {
            if manhattan(p, *c) == 1 && w.0[0] == bits && w.0[1] == bits {
                let lo = (t + dmin).max(1) as usize;
                let hi = (t + dmax).max(1) as usize;
                out.push((p, lo, hi));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Events that do nothing: the door still works without them (ideal inputs), removed greedily.
pub fn noop_events(pool: &Pool, steps: &Steps, at: &[u32], cands: &[(Pos, usize)]) -> Vec<(Pos, usize)> {
    let opts = crate::ideal::IdealOptions { cycles: 2, offsets: crate::ideal::offsets(0).into_iter().take(2).collect(), trace: None };
    // Each alone first (in parallel), then greedily together.
    let alone = pool.map(cands.to_vec(), |w, _, e| Ok(crate::ideal::run(w, &steps.retimed_without_all(at, &[e]), &BTreeMap::new(), &opts).is_ok()));
    let mut out: Vec<(Pos, usize)> = Vec::new();
    for (e, ok) in cands.iter().zip(alone) {
        if !ok.unwrap_or(false) {
            continue;
        }
        let mut trial = out.clone();
        trial.push(*e);
        let mut w = pool.worker(0);
        if crate::ideal::run(&mut w, &steps.retimed_without_all(at, &trial), &BTreeMap::new(), &opts).is_ok() {
            out = trial;
        }
    }
    out
}

/// Make the ticks a dropped (no-op) event powered don't-care for every piston cell it touched
/// (directly or through the quasi-connectivity cell): whatever the wiring gives there then.
pub fn dont_care(mech: &mut Mech, steps: &Steps, at: &[u32], noops: &[(Pos, usize)]) {
    for &(c, s) in noops {
        let bits = steps.event_bits(at, c, s);
        let cells: Vec<Pos> = mech.meas.pistons.iter().map(|x| x.0).filter(|&q| manhattan(q, c) == 1 || (manhattan(add(q, [0, 1, 0]), c) == 1 && q != c)).collect();
        for q in cells {
            if let Some(m) = mech.meas.occ.get_mut(&q) {
                *m = m.minus(bits);
            }
        }
    }
}

// ---- retiming -----------------------------------------------------------------------------

/// A schedule as steps: every distinct set tick is a step; clear events follow their set (a
/// pulse keeps its length) or land on a later step (a window).
#[derive(Clone, Debug)]
pub struct Steps {
    pub design: Design,
    /// Old set tick of each step, ascending.
    pub ticks: Vec<u32>,
    /// (open?, cell, step of the set, end: Ok(len) = cleared len ticks after the set, Err(step) =
    /// cleared at that step's tick (+ offset))
    pub events: Vec<(bool, Pos, usize, End)>,
}

#[derive(Clone, Copy, Debug)]
pub enum End {
    /// Cleared `len` ticks after its set.
    After(u32),
    /// Cleared at step `s`'s tick plus `off`.
    AtStep(usize, i32),
    /// Never cleared within the phase.
    Never,
    /// Not a set: a clear of a cell that holds its block at rest (a level input turned off).
    ClearOnly,
}

impl Steps {
    pub fn from_schedule(s: &Schedule, vars: &BTreeMap<String, i64>) -> Result<Steps> {
        let mut ticks: Vec<u32> = Vec::new();
        let mut per: Vec<(bool, Vec<(u32, Pos, Option<String>)>)> = Vec::new();
        // Which clears end a set of the same phase.
        let ends_a_set = |evs: &[(u32, Pos, Option<String>)], i: usize| -> bool {
            let (_, p, _) = &evs[i];
            evs[..i].iter().rev().find(|(_, q, _)| q == p).is_some_and(|(_, _, x)| x.is_some())
        };
        for open in [true, false] {
            let evs = s.events_of(open, vars)?;
            for (i, (t, _, st)) in evs.iter().enumerate() {
                if st.is_some() || !ends_a_set(&evs, i) {
                    ticks.push(*t);
                }
            }
            per.push((open, evs));
        }
        ticks.sort();
        ticks.dedup();
        let step = |t: u32| ticks.iter().position(|x| *x == t);
        let mut events = Vec::new();
        for (open, evs) in &per {
            for (i, (t, p, st)) in evs.iter().enumerate() {
                let s0 = step(*t);
                if st.is_none() {
                    if !ends_a_set(evs, i) {
                        events.push((*open, *p, s0.unwrap(), End::ClearOnly));
                    }
                    continue;
                }
                let s0 = s0.unwrap();
                // Its clear: the next event of the same cell in this phase, if a clear.
                let clear = evs.iter().skip(i + 1).find(|(_, q, _)| q == p).filter(|e| e.2.is_none()).map(|e| e.0);
                let end = match clear {
                    None => End::Never,
                    Some(c) if c - t <= 3 => End::After(c - t),
                    Some(c) => match step(c) {
                        Some(sc) => End::AtStep(sc, 0),
                        None => {
                            let sc = ticks.iter().rposition(|x| *x <= c).unwrap();
                            End::AtStep(sc, c as i32 - ticks[sc] as i32)
                        }
                    },
                };
                events.push((*open, *p, s0, end));
            }
        }
        Ok(Steps { design: s.design().clone(), ticks, events })
    }

    /// The schedule with step k at tick `new[k]`.
    pub fn retimed(&self, new: &[u32]) -> Schedule {
        self.retimed_shift(new, [i32::MIN, 0, 0], usize::MAX, 0).unwrap()
    }

    /// [`Steps::retimed`] with the events of cell `cell` set at step `step` moved by `d` ticks
    /// (None if that makes a tick negative).
    pub fn retimed_shift(&self, new: &[u32], cell: Pos, step: usize, d: i32) -> Option<Schedule> {
        self.retimed_edit(new, cell, step, Some(d))
    }

    /// [`Steps::retimed`] without the events of cell `cell` set at step `step`.
    pub fn retimed_without(&self, new: &[u32], cell: Pos, step: usize) -> Schedule {
        self.retimed_edit(new, cell, step, None).unwrap()
    }

    /// Pulse pairs as windows: a cell set twice per phase with plain short pulses becomes one
    /// level window from the first set to the second (a pulse-toggled piston pushed then
    /// pulled = a sticky piston on a window, which one repeater makes). Only the `cells` given.
    pub fn with_windows(&self, cells_ok: &[Pos]) -> Steps {
        let mut s = self.clone();
        let mut out = Vec::new();
        let cells: Vec<(bool, Pos)> = {
            let mut v: Vec<(bool, Pos)> = self.events.iter().map(|e| (e.0, e.1)).collect();
            v.sort();
            v.dedup();
            v
        };
        for (open, c) in cells {
            let mine: Vec<&(bool, Pos, usize, End)> = self.events.iter().filter(|e| e.0 == open && e.1 == c).collect();
            let pulses = cells_ok.contains(&c) && mine.len() == 2 && mine.iter().all(|e| matches!(e.3, End::After(l) if l <= 3));
            if pulses {
                let (a, b) = (mine[0].2.min(mine[1].2), mine[0].2.max(mine[1].2));
                out.push((open, c, a, End::AtStep(b, 0)));
            } else {
                out.extend(mine.into_iter().cloned());
            }
        }
        s.events = out;
        s
    }

    /// Cells set twice per phase with short pulses (window candidates).
    pub fn pulse_pair_cells(&self) -> Vec<Pos> {
        let mut v: Vec<Pos> = self.events.iter().map(|e| e.1).collect();
        v.sort();
        v.dedup();
        v.retain(|c| {
            [true, false].iter().all(|&open| {
                let mine: Vec<&(bool, Pos, usize, End)> = self.events.iter().filter(|e| e.0 == open && e.1 == *c).collect();
                mine.len() == 2 && mine.iter().all(|e| matches!(e.3, End::After(l) if l <= 3))
            })
        });
        v
    }

    /// [`Steps::retimed`] without several events.
    pub fn retimed_without_all(&self, new: &[u32], drop: &[(Pos, usize)]) -> Schedule {
        let mut s = self.clone();
        s.events.retain(|e| !drop.contains(&(e.1, e.2)));
        s.retimed(new)
    }

    /// Ticks (per phase, as wave bits) that the event (cell, step) powers its piston, for
    /// `new` step times.
    pub fn event_bits(&self, new: &[u32], cell: Pos, step: usize) -> Wave {
        let mut w = Wave::OFF;
        for &(open, p, s, end) in &self.events {
            if p != cell || s != step {
                continue;
            }
            let t = new[s] as usize;
            let c = match end {
                End::After(l) => t + l as usize,
                End::AtStep(sc, off) => (new[sc] as i32 + off).max(t as i32 + 1) as usize,
                End::Never => T - 1,
                End::ClearOnly => continue,
            };
            for b in (t + 1)..=c.min(T - 1) {
                w.0[if open { 0 } else { 1 }] |= 1 << b;
            }
        }
        w
    }

    fn retimed_edit(&self, new: &[u32], cell: Pos, step: usize, d: Option<i32>) -> Option<Schedule> {
        let mut ev: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
        for &(open, p, s, end) in &self.events {
            let hit = p == cell && s == step;
            if hit && d.is_none() {
                continue;
            }
            let shift = if hit { d.unwrap_or(0) } else { 0 };
            let t = new[s] as i32 + shift;
            if t < 0 {
                return None;
            }
            let t = t as u32;
            if let End::ClearOnly = end {
                ev.push((open, t, p, None));
                continue;
            }
            ev.push((open, t, p, Some("minecraft:redstone_block".to_string())));
            match end {
                End::After(l) => ev.push((open, t + l, p, None)),
                End::AtStep(sc, off) => ev.push((open, (new[sc] as i32 + off + shift).max(t as i32 + 1) as u32, p, None)),
                End::Never | End::ClearOnly => {}
            }
        }
        Some(Schedule::from_events(self.design.clone(), &ev))
    }

    /// Every assignment: step 0 at one of `first`, each next step a gap from `gaps` later.
    pub fn assignments(&self, first: &[u32], gaps: &[u32], max_tick: u32) -> Vec<Vec<u32>> {
        let n = self.ticks.len();
        let mut out = Vec::new();
        let mut cur: Vec<u32> = Vec::new();
        fn rec(n: usize, gaps: &[u32], max_tick: u32, cur: &mut Vec<u32>, out: &mut Vec<Vec<u32>>) {
            if cur.len() == n {
                out.push(cur.clone());
                return;
            }
            let last = *cur.last().unwrap();
            for g in gaps {
                if last + g > max_tick {
                    continue;
                }
                cur.push(last + g);
                rec(n, gaps, max_tick, cur, out);
                cur.pop();
            }
        }
        for &f in first {
            cur.clear();
            cur.push(f);
            rec(n, gaps, max_tick, &mut cur, &mut out);
        }
        out
    }
}

/// Replay retimed schedules with ideal inputs (`offsets` positions, 2 cycles); the working
/// assignments with their (open, close) ticks.
pub fn working_retimes(pool: &Pool, steps: &Steps, assigns: Vec<Vec<u32>>, extra: u32, positions: usize) -> Vec<(Vec<u32>, (u32, u32))> {
    let opts = crate::ideal::IdealOptions { cycles: 2, offsets: crate::ideal::offsets(extra).into_iter().take(positions.max(1)).collect(), trace: None };
    let res = pool.map(assigns.clone(), |w, _, a| {
        let s = steps.retimed(&a);
        Ok(crate::ideal::run(w, &s, &BTreeMap::new(), &opts).ok())
    });
    let mut out = Vec::new();
    for (a, r) in assigns.into_iter().zip(res) {
        if let Ok(Some(t)) = r {
            out.push((a, t));
        }
    }
    out
}

/// Cells receiving power, per input cell's piston: used to pick lever spots near the first
/// receivers (the ones that need power first).
pub fn early_receivers(mech: &Mech, k: usize) -> Vec<Pos> {
    let mut v: Vec<(u32, Pos)> = mech
        .wants
        .iter()
        .filter(|(_, _, w)| !w.is_off())
        .map(|(p, _, w)| (w.0[0].trailing_zeros().min(w.0[1].trailing_zeros()), *p))
        .collect();
    v.sort();
    v.dedup_by_key(|x| x.1);
    v.into_iter().take(k).map(|x| x.1).collect()
}

/// Judge candidates in the simulator (screen, then the full judge); the passing ones, fewest
/// blocks first.
pub fn judge(pool: &Pool, cands: &[Wired], sopts: &crate::search::SearchOptions, log: &(dyn Fn(&str) + Sync)) -> Vec<(Design, String)> {
    let v: Vec<(Design, String)> = cands.iter().map(|w| (w.design.clone(), w.desc.clone())).collect();
    route2::validate(pool, v, sopts, log)
}

/// Polish a verified wired design: route2's rip-up and re-route (groups of up to 6 control
/// blocks, pairs too), then the local search (`--moves all`: removals, swaps, moves, renets)
/// until `secs` run out. Returns the smallest verified design.
pub fn polish(pool: &Pool, d: &Design, route: &RouteOptions, sopts: &crate::search::SearchOptions, secs: u64, log: &(dyn Fn(&str) + Sync)) -> Result<Design> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    let quiet = |m: &str| {
        if m.contains("improved") {
            log(m);
        }
    };
    let ro = route2::RipOptions {
        max_rip: 6,
        route: RouteOptions { partial: false, order_limit: 0, ..route.clone() },
        no_check: false,
        rip: Vec::new(),
        verbose: false,
        extra_blocks: 0,
        pairs: true,
        polish: false,
        polish_secs: 0,
        polish_max: 0,
    };
    let mut noop = |_: &Design, _: &str| -> Result<()> { Ok(()) };
    let mut cur = route2::rip_and_reroute(pool, d, &ro, sopts, &quiet, &mut noop).unwrap_or_else(|_| d.clone());
    if std::time::Instant::now() < deadline {
        let mut o = sopts.clone();
        o.families = crate::search::parse_families("all")?;
        o.deadline = Some(deadline);
        let r = crate::search::search(pool, &cur, &o, &quiet, &mut noop)?;
        if block_count(&r) < block_count(&cur) {
            cur = r;
        }
    }
    Ok(cur)
}
