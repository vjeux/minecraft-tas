//! `mctas door buildable`: can a survival player actually build a design?
//!
//! The judge pastes every block raw (no updates), sends a shape update to the dust and a self
//! update to every block, then lets the machine settle. That rest state can be one a player never
//! reaches by placing blocks one at a time with normal updates: a quasi-powered piston left
//! retracted (a BUD), a silent hold, an observer that fires when its neighbour is placed, a piston
//! that fires mid-build and pushes half the door away...
//!
//! This searches for a build program: an order of player placements (the bridge's `build` op: a
//! real `BlockItem.place` with a click on a neighbouring block's face and the look direction that
//! gives the wanted orientation, flags 11, `setPlacedBy`, the place game event, then right-clicks
//! for repeater delays and levers), optionally the door lever switched on while building, and
//! temporary helper blocks placed and broken later. The world settles after every action (a player
//! is slow). A program counts when the build ends exactly in the judge's rest state (every block
//! state of the judge's watch box, no entities) and the judge's open/close cycles pass from there.
//!
//! Search: heuristic orders first (mechanism before wiring, power last, bottom-up...), then a
//! greedy construction that never accepts damage (a block pushed out of place, a piston head or
//! block where the rest state has air, a drop), then simulated annealing over whole programs
//! (moves, swaps, the lever, helper blocks), all within a time budget.

use crate::{DoorSpec, open_problems, settled_from, watch_box};
use anyhow::{Context, Result, bail};
use mctas::blockstate::{block_name, is_air};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pool, Pos, Region, SandboxOptions, Worker, WorkerConfig};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The judge's sandbox shape (`verify`).
fn sandbox_options() -> SandboxOptions {
    SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 }
}

/// One player action.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    /// Place this block state (orientation and click-set properties are honoured; power,
    /// connections and the like are whatever the world makes of it).
    Place(String),
    /// Right-click (a lever...).
    Use,
    /// Break the block (no drops: the player picks them up).
    Break,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub pos: Pos,
    pub action: Action,
    /// Why (for the build script): "", "helper", "lever off"...
    pub note: String,
}

/// What the bridge reported for one action.
#[derive(Clone, Debug, Default)]
pub struct StepReport {
    pub ok: bool,
    pub placed: Option<String>,
    pub click: Option<String>,
    pub supported: bool,
    pub error: Option<String>,
    pub ticks: u32,
}

/// A simulated build.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub steps: Vec<StepReport>,
    /// The judge's watch box after the build settled.
    pub region: Region,
    pub entities: usize,
    pub idle: bool,
}

impl Outcome {
    pub fn failed(&self) -> usize {
        self.steps.iter().filter(|s| !s.ok).count()
    }
    pub fn unsupported(&self) -> usize {
        self.steps.iter().filter(|s| s.ok && s.placed.is_some() && !s.supported).count()
    }
}

/// Knobs of a simulated build.
#[derive(Clone, Debug)]
pub struct SimOptions {
    /// Ticks the world may take to settle after each action (cap).
    pub settle: u32,
    /// Ticks that pass after each action at least (a player clicks at most every 4 ticks).
    pub min_ticks: u32,
}

impl Default for SimOptions {
    fn default() -> Self {
        SimOptions { settle: 200, min_ticks: 4 }
    }
}

/// The judge's rest state: raw paste, dust shapes, a self update to every block, settle; the
/// judge's watch box read back.
pub fn judge_rest(w: &mut Worker, sb: u32, d: &Design) -> Result<Region> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let settle = w.step_until_idle(sb, 400)?;
    if settle.idle != Some(true) {
        bail!("the build never settles after placement ({} ticks)", settle.ticked);
    }
    let spec = DoorSpec::from_design(d)?;
    let (lo, hi) = watch_box(d, &spec);
    w.get_region(sb, lo, hi)
}

/// Run a build program in a cleared sandbox, settle, read the judge's watch box of `d`.
pub fn simulate(w: &mut Worker, sb: u32, d: &Design, steps: &[Step], opts: &SimOptions) -> Result<Outcome> {
    w.clear(sb)?;
    let list: Vec<Value> = steps
        .iter()
        .map(|s| match &s.action {
            Action::Place(st) => json!({"do": "place", "pos": s.pos, "state": st}),
            Action::Use => json!({"do": "use", "pos": s.pos}),
            Action::Break => json!({"do": "break", "pos": s.pos}),
        })
        .collect();
    let v = w.call(json!({
        "op": "build", "sandbox": sb, "steps": list, "settle": opts.settle, "min_ticks": opts.min_ticks,
        "stop_on_fail": false
    }))?;
    let mut reports = Vec::new();
    for r in v["results"].as_array().context("no results")? {
        let s = |k: &str| r[k].as_str().map(|x| x.to_string());
        reports.push(StepReport {
            ok: r["ok"].as_bool().unwrap_or(false),
            placed: s("placed"),
            click: s("click"),
            supported: r["supported"].as_bool().unwrap_or(true),
            error: s("error"),
            ticks: r["ticks"].as_u64().unwrap_or(0) as u32,
        });
    }
    // Vibrations still travelling, last pistons: let everything finish.
    w.step(sb, 20)?;
    let settled = w.step_until_idle(sb, 400)?;
    let spec = DoorSpec::from_design(d)?;
    let (lo, hi) = watch_box(d, &spec);
    let region = w.get_region(sb, lo, hi)?;
    let entities = w.entities(sb)?.len();
    Ok(Outcome { steps: reports, region, entities, idle: settled.idle == Some(true) })
}

/// Cells where the build differs from the rest state: (pos, rest, built).
pub fn diffs(rest: &Region, got: &Region) -> Vec<(Pos, String, String)> {
    rest.iter()
        .filter_map(|(p, r)| {
            let g = got.get(p).unwrap_or("?");
            (g != r).then(|| (p, r.to_string(), g.to_string()))
        })
        .collect()
}

/// Lower is better; 0 means the build ends exactly in the rest state with nothing broken.
pub fn cost(rest: &Region, out: &Outcome) -> f64 {
    let mut c = 0.0;
    for (_, r, g) in diffs(rest, &out.region) {
        c += if block_name(&r) == block_name(&g) { 1.0 } else { 3.0 };
    }
    c += 10.0 * out.entities as f64 + 2.0 * out.failed() as f64;
    if !out.idle {
        c += 5.0;
    }
    c
}

/// Damage of a partial build: things placing more blocks cannot repair (a block where the rest
/// state has air, a placed block gone or replaced, a drop, a failed placement), plus a small
/// weight for state differences on placed cells.
/// The cells [`damage`] counts as hard damage, described (for reports).
fn damage_cells(rest: &Region, out: &Outcome, placed: &BTreeSet<Pos>) -> Vec<String> {
    let mut v = Vec::new();
    for (p, r) in rest.iter() {
        let g = out.region.get(p).unwrap_or("?");
        if g == r {
            continue;
        }
        let what = if is_air(r) {
            Some("should be air")
        } else if placed.contains(&p) && block_name(g) != block_name(r) {
            Some("moved or broken")
        } else if !placed.contains(&p) && !is_air(g) && block_name(g) != block_name(r) {
            Some("something else there")
        } else {
            None
        };
        if let Some(w) = what {
            v.push(format!("{} {} {}: {w} (rest {}, built {})", p[0], p[1], p[2], short(r), short(g)));
        }
    }
    if out.entities > 0 {
        v.push(format!("{} dropped items", out.entities));
    }
    for s in out.steps.iter().filter(|s| !s.ok) {
        v.push(s.error.clone().unwrap_or_else(|| "a placement failed".into()));
    }
    v
}

fn damage(rest: &Region, out: &Outcome, placed: &BTreeSet<Pos>) -> (f64, f64) {
    let mut hard = 10.0 * out.entities as f64 + 3.0 * out.failed() as f64;
    let mut soft = 0.0;
    for (p, r) in rest.iter() {
        let g = out.region.get(p).unwrap_or("?");
        if g == r {
            continue;
        }
        if is_air(r) {
            hard += 3.0;
        } else if placed.contains(&p) {
            if block_name(g) != block_name(r) {
                hard += 3.0;
            } else {
                soft += 1.0;
            }
        } else if !is_air(g) && block_name(g) != block_name(r) {
            hard += 3.0;
        }
    }
    (hard, soft)
}

/// A block the player places (piston heads come from extending their piston).
#[derive(Clone, Debug)]
pub struct Item {
    pub pos: Pos,
    pub state: String,
}

/// A temporary block: placed right before the order's `place_at`-th item, broken right after its
/// `break_after`-th item (`>= place_at`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Helper {
    pub pos: Pos,
    pub state: String,
    pub place_at: usize,
    pub break_after: usize,
}

/// A build program in search form.
#[derive(Clone, Debug)]
pub struct Plan {
    pub order: Vec<usize>,
    /// Place the door lever switched on and switch it off as the last action.
    pub lever_on: bool,
    pub helpers: Vec<Helper>,
}

/// The design's blocks to place: the rest state's non-air blocks minus piston heads.
pub fn items(rest: &Region) -> Vec<Item> {
    rest.iter()
        .filter(|(_, s)| !is_air(s) && !s.starts_with("minecraft:piston_head"))
        .map(|(p, s)| Item { pos: p, state: s.to_string() })
        .collect()
}

fn with_prop(state: &str, key: &str, value: &str) -> String {
    match mctas::blockstate::BlockState::parse(state) {
        Ok(b) => b.with(key, value).to_string(),
        Err(_) => state.to_string(),
    }
}

/// The steps of a plan.
pub fn program(plan: &Plan, items: &[Item], lever: Pos) -> Vec<Step> {
    let mut out = Vec::new();
    for (k, &i) in plan.order.iter().enumerate() {
        for h in plan.helpers.iter().filter(|h| h.place_at == k) {
            out.push(Step { pos: h.pos, action: Action::Place(h.state.clone()), note: "temporary".into() });
        }
        let it = &items[i];
        let mut state = it.state.clone();
        let mut note = String::new();
        if plan.lever_on && it.pos == lever {
            state = with_prop(&state, "powered", "true");
            note = "the door lever, switched ON while building".into();
        }
        out.push(Step { pos: it.pos, action: Action::Place(state), note });
        for h in plan.helpers.iter().filter(|h| h.break_after == k) {
            out.push(Step { pos: h.pos, action: Action::Break, note: "remove the temporary block".into() });
        }
    }
    if plan.lever_on {
        out.push(Step { pos: lever, action: Action::Use, note: "switch the door lever OFF".into() });
    }
    out
}

/// Heuristic class of a block: 0 plain blocks, 1 pistons, 2 dust/repeaters/comparators,
/// 3 observers, 4 power sources and sensors.
fn class(state: &str) -> u8 {
    match block_name(state).trim_start_matches("minecraft:") {
        "piston" | "sticky_piston" => 1,
        "redstone_wire" | "repeater" | "comparator" => 2,
        "observer" => 3,
        "redstone_torch" | "redstone_wall_torch" | "redstone_block" | "lever" | "sculk_sensor" | "calibrated_sculk_sensor"
        | "target" | "daylight_detector" | "stone_button" | "tripwire_hook" => 4,
        _ => 0,
    }
}

/// Orders from simple rules (each sorted by class rank, then y, z, x).
fn heuristic_orders(items: &[Item]) -> Vec<(String, Vec<usize>)> {
    let ranks: &[(&str, [u8; 5])] = &[
        ("mechanism, pistons, wiring, observers, power", [0, 1, 2, 3, 4]),
        ("pistons, mechanism, wiring, observers, power", [1, 0, 2, 3, 4]),
        ("mechanism, pistons, observers, wiring, power", [0, 1, 3, 2, 4]),
        ("bottom-up", [0, 0, 0, 0, 0]),
        ("mechanism, wiring, power, pistons, observers", [0, 3, 1, 4, 2]),
        ("power, wiring, mechanism, pistons, observers", [2, 3, 1, 4, 0]),
    ];
    let mut out: Vec<(String, Vec<usize>)> = Vec::new();
    for (name, rank) in ranks {
        let key = |i: usize| {
            let p = items[i].pos;
            (rank[class(&items[i].state) as usize], p[1], p[2], p[0])
        };
        let mut o: Vec<usize> = (0..items.len()).collect();
        o.sort_by_key(|&i| key(i));
        out.push((name.to_string(), supports_first(items, o)));
        // Connected growth: the same ranking, but always a block touching what is built when
        // there is one (fewer temporary blocks).
        let mut placed: BTreeSet<Pos> = BTreeSet::new();
        let mut rest: Vec<usize> = (0..items.len()).collect();
        let mut o = Vec::new();
        while !rest.is_empty() {
            let touching = |i: usize| {
                let q = items[i].pos;
                [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                    .iter()
                    .any(|d| placed.contains(&[q[0] + d[0], q[1] + d[1], q[2] + d[2]]))
            };
            let pick = rest
                .iter()
                .copied()
                .filter(|&i| touching(i))
                .min_by_key(|&i| key(i))
                .unwrap_or_else(|| rest.iter().copied().min_by_key(|&i| key(i)).unwrap());
            placed.insert(items[pick].pos);
            rest.retain(|&x| x != pick);
            o.push(pick);
        }
        out.push((format!("{name}, connected"), supports_first(items, o)));
    }
    out
}

/// The block an attached block stands on (dust, repeaters, torches, levers...), if any.
fn support_of(it: &Item) -> Option<Pos> {
    let s = mctas::blockstate::BlockState::parse(&it.state).ok()?;
    let p = it.pos;
    let below = [p[0], p[1] - 1, p[2]];
    let behind = |f: &str| match f {
        "north" => Some([p[0], p[1], p[2] + 1]),
        "south" => Some([p[0], p[1], p[2] - 1]),
        "east" => Some([p[0] - 1, p[1], p[2]]),
        "west" => Some([p[0] + 1, p[1], p[2]]),
        _ => None,
    };
    match block_name(&it.state).trim_start_matches("minecraft:") {
        "redstone_wire" | "repeater" | "comparator" | "redstone_torch" | "torch" | "rail" | "powered_rail" | "detector_rail"
        | "activator_rail" => Some(below),
        "redstone_wall_torch" | "wall_torch" => behind(s.get("facing")?),
        n if n == "lever" || n.ends_with("_button") => match s.get("face")? {
            "floor" => Some(below),
            "ceiling" => Some([p[0], p[1] + 1, p[2]]),
            _ => behind(s.get("facing")?),
        },
        _ => None,
    }
}

/// Move every attached block right after the block it stands on when that comes later.
fn supports_first(items: &[Item], mut order: Vec<usize>) -> Vec<usize> {
    let index: BTreeMap<Pos, usize> = items.iter().enumerate().map(|(i, it)| (it.pos, i)).collect();
    for _ in 0..items.len() {
        let mut moved = false;
        let pos_in: BTreeMap<usize, usize> = order.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        for k in 0..order.len() {
            let i = order[k];
            let Some(sup) = support_of(&items[i]).and_then(|q| index.get(&q).copied()) else { continue };
            let ks = pos_in[&sup];
            if ks > k {
                let x = order.remove(k);
                order.insert(ks, x);
                moved = true;
                break;
            }
        }
        if !moved {
            break;
        }
    }
    order
}

/// xorshift64*.
#[derive(Clone)]
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E3779B97F4A7C15) | 1)
    }
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Search knobs.
#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub budget: Duration,
    pub workers: usize,
    pub sim: SimOptions,
    /// Allow temporary helper blocks in the annealing.
    pub helpers: bool,
    pub seed: u64,
    pub verbose: bool,
    /// Lever state while building: Some(false) = always off, Some(true) = on, None = try both.
    pub lever: Option<bool>,
}

impl SearchOptions {
    fn lever_states(&self, has_lever: bool) -> Vec<bool> {
        match (has_lever, self.lever) {
            (false, _) => vec![false],
            (true, Some(b)) => vec![b],
            (true, None) => vec![false, true],
        }
    }
}

/// Result of [`search`].
#[derive(Clone, Debug)]
pub struct Found {
    pub plan: Plan,
    pub steps: Vec<Step>,
    pub outcome: Outcome,
    pub cost: f64,
    /// Which stage found it ("heuristic: ...", "greedy", "anneal").
    pub how: String,
    pub evaluations: u64,
    /// What the simplification pass learned (which helpers and lever setting are needed, why).
    pub notes: Vec<String>,
}

/// One worker's sandbox per pool slot.
struct Lab {
    pool: Pool,
    sandboxes: Vec<u32>,
}

impl Lab {
    fn new(version: &str, workers: usize) -> Result<Lab> {
        let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
        let pool = Pool::spawn(workers.max(1), &cfg)?;
        let mut sandboxes = Vec::new();
        for i in 0..pool.len() {
            sandboxes.push(pool.worker(i).create(&sandbox_options())?);
        }
        Ok(Lab { pool, sandboxes })
    }
}

/// Everything a search needs about one design.
pub struct Problem {
    pub design: Design,
    pub spec: DoorSpec,
    pub rest: Region,
    pub items: Vec<Item>,
}

impl Problem {
    pub fn new(w: &mut Worker, d: &Design) -> Result<Problem> {
        let sb = w.create(&sandbox_options())?;
        let rest = judge_rest(w, sb, d);
        let _ = w.drop_sandbox(sb);
        let rest = rest?;
        let spec = DoorSpec::from_design(d)?;
        let items = items(&rest);
        Ok(Problem { design: d.clone(), spec, rest, items })
    }

    fn steps(&self, plan: &Plan) -> Vec<Step> {
        program(plan, &self.items, self.spec.lever)
    }

    fn has_lever(&self) -> bool {
        self.items.iter().any(|it| it.pos == self.spec.lever)
    }
}

/// Air cells next to the build (inside the watch box): where a helper block may go.
fn helper_cells(p: &Problem) -> Vec<Pos> {
    let mut out = BTreeSet::new();
    for it in &p.items {
        for d in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
            let q = [it.pos[0] + d[0], it.pos[1] + d[1], it.pos[2] + d[2]];
            if p.rest.get(q).is_some_and(is_air) {
                out.insert(q);
            }
        }
    }
    out.into_iter().collect()
}

const HELPER_STATES: &[&str] = &["minecraft:stone", "minecraft:glass", "minecraft:obsidian", "minecraft:redstone_block"];

/// Search for a build program reaching the rest state. Returns the best program found (cost 0
/// means it reaches the rest state; the caller then checks the cycles).
/// Search, then simplify a found program: drop every helper block it can do without, prefer the
/// lever off while building, and note why what remains is needed.
pub fn search(version: &str, p: &Problem, opts: &SearchOptions, log: &(dyn Fn(&str) + Sync)) -> Result<Found> {
    let mut found = search_raw(version, p, opts, log)?;
    if found.cost != 0.0 {
        return Ok(found);
    }
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let mut w = Worker::spawn(&cfg)?;
    let sb = w.create(&sandbox_options())?;
    let summary = |out: &Outcome| -> String {
        let b = blocking(&p.rest, out);
        let mut s: Vec<String> = b.iter().take(3).cloned().collect();
        if out.entities > 0 {
            s.push(format!("{} dropped items", out.entities));
        }
        if b.len() > 3 {
            s.push(format!("{} more", b.len() - 3));
        }
        s.join(" | ")
    };
    // How the plain rule order ends, for the report, and the first placement that does damage.
    if let Some((name, order)) = heuristic_orders(&p.items).into_iter().next() {
        let plan = Plan { order: order.clone(), lever_on: false, helpers: vec![] };
        let out = simulate(&mut w, sb, &p.design, &p.steps(&plan), &opts.sim)?;
        if cost(&p.rest, &out) == 0.0 {
            found.notes.push(format!("the plain order ({name}) works too"));
        } else {
            let mut first = String::new();
            let mut first_k: Option<usize> = None;
            let mut placed = BTreeSet::new();
            for k in 1..=order.len() {
                placed.insert(p.items[order[k - 1]].pos);
                let pre = Plan { order: order[..k].to_vec(), lever_on: false, helpers: vec![] };
                let o2 = simulate(&mut w, sb, &p.design, &p.steps(&pre), &opts.sim)?;
                found.evaluations += 1;
                if damage(&p.rest, &o2, &placed).0 > 0.0 {
                    first_k = Some(k);
                    let it = &p.items[order[k - 1]];
                    let ticks = o2.steps.last().map(|s| s.ticks).unwrap_or(0);
                    let cells = damage_cells(&p.rest, &o2, &placed);
                    let shown: Vec<String> = cells.iter().take(4).cloned().collect();
                    first = format!(
                        "first damage at step {k}/{}: placing {} at {} {} {} (things move for {ticks} ticks): {}{}; ",
                        order.len(),
                        short(&it.state),
                        it.pos[0],
                        it.pos[1],
                        it.pos[2],
                        shown.join(" | "),
                        if cells.len() > 4 { format!(" | {} more", cells.len() - 4) } else { String::new() }
                    );
                    break;
                }
            }
            found.notes.push(format!("the plain order ({name}) does not: {first}at the end: {}", summary(&out)));
            if let Some(k) = first_k {
                let pre = Plan { order: order[..k - 1].to_vec(), lever_on: false, helpers: vec![] };
                let last = Plan { order: order[..k].to_vec(), lever_on: false, helpers: vec![] };
                let steps = p.steps(&last);
                let n_pre = p.steps(&pre).len();
                for line in trace_step(&mut w, sb, p, &steps[..n_pre], &steps[n_pre..], &opts.sim)? {
                    found.notes.push(format!("  trace of that step: {line}"));
                }
            }
        }
    }
    let mut i = found.plan.helpers.len();
    while i > 0 {
        i -= 1;
        let mut plan = found.plan.clone();
        let h = plan.helpers.remove(i);
        let out = simulate(&mut w, sb, &p.design, &p.steps(&plan), &opts.sim)?;
        let c = cost(&p.rest, &out);
        found.evaluations += 1;
        if c == 0.0 {
            found.steps = p.steps(&plan);
            found.plan = plan;
            found.outcome = out;
        } else {
            found.notes.push(format!("temporary {} at {:?} is needed; without it: {}", short(&h.state), h.pos, summary(&out)));
        }
    }
    if found.plan.lever_on {
        let mut plan = found.plan.clone();
        plan.lever_on = false;
        let out = simulate(&mut w, sb, &p.design, &p.steps(&plan), &opts.sim)?;
        found.evaluations += 1;
        if cost(&p.rest, &out) == 0.0 {
            found.steps = p.steps(&plan);
            found.plan = plan;
            found.outcome = out;
        } else {
            found.notes.push(format!("this order needs the lever ON while building; with it off: {}", summary(&out)));
        }
    }
    let _ = w.drop_sandbox(sb);
    Ok(found)
}

fn search_raw(version: &str, p: &Problem, opts: &SearchOptions, log: &(dyn Fn(&str) + Sync)) -> Result<Found> {
    let lab = Lab::new(version, opts.workers)?;
    let deadline = Instant::now() + opts.budget;
    let evals = std::sync::atomic::AtomicU64::new(0);
    let eval = |w: &mut Worker, sb: u32, plan: &Plan| -> Result<(f64, Outcome)> {
        evals.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let out = simulate(w, sb, &p.design, &p.steps(plan), &opts.sim)?;
        Ok((cost(&p.rest, &out), out))
    };
    let mut best: Option<Found> = None;
    let consider = |best: &mut Option<Found>, plan: Plan, c: f64, out: Outcome, how: &str| {
        let better = best.as_ref().is_none_or(|b| c < b.cost);
        if better {
            *best = Some(Found {
                steps: p.steps(&plan),
                plan,
                outcome: out,
                cost: c,
                how: how.to_string(),
                evaluations: evals.load(std::sync::atomic::Ordering::Relaxed),
                notes: Vec::new(),
            });
        }
    };

    // 1. Heuristic orders, lever off and on.
    let mut plans = Vec::new();
    for (name, order) in heuristic_orders(&p.items) {
        for lever_on in opts.lever_states(p.has_lever()) {
            plans.push((format!("heuristic: {name}{}", if lever_on { ", lever on while building" } else { "" }), Plan { order: order.clone(), lever_on, helpers: vec![] }));
        }
    }
    let results = lab.pool.map(plans, |w, wi, (name, plan)| {
        let (c, out) = eval(w, lab.sandboxes[wi], &plan)?;
        Ok((name, plan, c, out))
    });
    for r in results {
        let (name, plan, c, out) = r?;
        log(&format!("  {name}: cost {c} ({} cells differ, {} failed placements, {} entities)", diffs(&p.rest, &out.region).len(), out.failed(), out.entities));
        consider(&mut best, plan, c, out, &name);
    }
    if best.as_ref().is_some_and(|b| b.cost == 0.0) {
        return Ok(best.unwrap());
    }

    // 2. Greedy construction that never accepts damage, randomized restarts.
    let mut restart = 0u64;
    while Instant::now() < deadline && restart < 6 {
        let mut rng = Rng::new(opts.seed ^ (restart + 1) * 7919);
        let levers = opts.lever_states(p.has_lever());
        let lever_on = levers[restart as usize % levers.len()];
        let mut order: Vec<usize> = Vec::new();
        let mut remaining: Vec<usize> = (0..p.items.len()).collect();
        let mut placed: BTreeSet<Pos> = BTreeSet::new();
        let mut stuck = false;
        while !remaining.is_empty() {
            if Instant::now() > deadline {
                stuck = true;
                break;
            }
            let cands: Vec<usize> = remaining.clone();
            let base = order.clone();
            let res = lab.pool.map(cands.clone(), |w, wi, c| {
                let mut o = base.clone();
                o.push(c);
                let plan = Plan { order: o, lever_on: false, helpers: vec![] };
                let mut steps = program(&plan, &p.items, p.spec.lever);
                if lever_on {
                    // the lever goes in switched on (when placed), as in the final program
                    for s in steps.iter_mut() {
                        if s.pos == p.spec.lever {
                            if let Action::Place(st) = &s.action {
                                s.action = Action::Place(with_prop(st, "powered", "true"));
                            }
                        }
                    }
                }
                evals.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let out = simulate(w, lab.sandboxes[wi], &p.design, &steps, &opts.sim)?;
                let mut pl = placed.clone();
                pl.insert(p.items[c].pos);
                let (hard, soft) = damage(&p.rest, &out, &pl);
                Ok((c, hard, soft))
            });
            let mut scored: Vec<(f64, f64, usize)> = Vec::new();
            for r in res {
                let (c, hard, soft) = r?;
                let noise = rng.unit() * 0.5;
                let q = p.items[c].pos;
                let touching = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]]
                    .iter()
                    .any(|d| placed.contains(&[q[0] + d[0], q[1] + d[1], q[2] + d[2]]));
                let prio = class(&p.items[c].state) as f64 * 0.3 + q[1] as f64 * 0.01 + if touching || placed.is_empty() { 0.0 } else { 0.6 };
                scored.push((hard, soft * 0.01 + prio + noise, c));
            }
            scored.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let (hard, _, c) = scored[0];
            if hard > 0.0 && opts.verbose {
                log(&format!("    greedy {restart}: every next block does damage at step {} (best {hard})", order.len()));
            }
            order.push(c);
            placed.insert(p.items[c].pos);
            remaining.retain(|&x| x != c);
        }
        if !stuck {
            let plan = Plan { order, lever_on, helpers: vec![] };
            let mut w = lab.pool.worker(0);
            let (c, out) = eval(&mut w, lab.sandboxes[0], &plan)?;
            drop(w);
            log(&format!("  greedy restart {restart}{}: cost {c}", if lever_on { " (lever on)" } else { "" }));
            consider(&mut best, plan, c, out, "greedy");
            if c == 0.0 {
                return Ok(best.unwrap());
            }
        }
        restart += 1;
    }

    // 3. Annealing over whole programs, one chain per worker.
    let start = best.clone().context("no plan evaluated")?;
    let cells = helper_cells(p);
    let shared: Mutex<Option<Found>> = Mutex::new(best.clone());
    let chains: Vec<u64> = (0..lab.pool.len() as u64).collect();
    let _ = lab.pool.map(chains, |w, wi, chain| {
        let sb = lab.sandboxes[wi];
        let mut rng = Rng::new(opts.seed ^ (chain + 101) * 104729);
        let mut cur = start.plan.clone();
        let mut cur_cost = start.cost;
        let mut temp = 2.0;
        let mut iter = 0u64;
        while Instant::now() < deadline {
            if shared.lock().unwrap().as_ref().is_some_and(|b| b.cost == 0.0) {
                break;
            }
            iter += 1;
            let mut cand = cur.clone();
            let n = cand.order.len();
            match rng.below(if opts.helpers { 10 } else { 8 }) {
                0..=3 => {
                    // move one item elsewhere
                    let i = rng.below(n);
                    let x = cand.order.remove(i);
                    let j = rng.below(n);
                    cand.order.insert(j, x);
                }
                4 | 5 => {
                    let (i, j) = (rng.below(n), rng.below(n));
                    cand.order.swap(i, j);
                }
                6 => {
                    // move a block of items
                    let i = rng.below(n);
                    let len = 1 + rng.below(4.min(n - i));
                    let seg: Vec<usize> = cand.order.drain(i..i + len).collect();
                    let j = rng.below(cand.order.len() + 1);
                    for (k, x) in seg.into_iter().enumerate() {
                        cand.order.insert(j + k, x);
                    }
                }
                7 => {
                    if opts.lever_states(p.has_lever()).len() > 1 {
                        cand.lever_on = !cand.lever_on;
                    } else {
                        continue;
                    }
                }
                _ => {
                    // add, move or drop a helper block
                    if !cand.helpers.is_empty() && rng.below(2) == 0 {
                        let h = rng.below(cand.helpers.len());
                        if rng.below(2) == 0 {
                            cand.helpers.remove(h);
                        } else {
                            let a = rng.below(n);
                            let b = a + rng.below(n - a);
                            cand.helpers[h].place_at = a;
                            cand.helpers[h].break_after = b;
                        }
                    } else if !cells.is_empty() && cand.helpers.len() < 3 {
                        let a = rng.below(n);
                        let b = a + rng.below(n - a);
                        cand.helpers.push(Helper {
                            pos: cells[rng.below(cells.len())],
                            state: HELPER_STATES[rng.below(HELPER_STATES.len())].to_string(),
                            place_at: a,
                            break_after: b,
                        });
                    }
                }
            }
            let Ok((c, out)) = eval(w, sb, &cand) else { continue };
            let c = c + 0.5 * cand.helpers.len() as f64;
            if c <= cur_cost || rng.unit() < ((cur_cost - c) / temp).exp() {
                cur = cand.clone();
                cur_cost = c;
                let mut g = shared.lock().unwrap();
                if g.as_ref().is_none_or(|b| c < b.cost) {
                    let real = c - 0.5 * cand.helpers.len() as f64;
                    *g = Some(Found {
                        steps: p.steps(&cand),
                        plan: cand,
                        outcome: out,
                        cost: if real == 0.0 { 0.0 } else { c },
                        how: format!("anneal (chain {chain}, iteration {iter})"),
                        evaluations: evals.load(std::sync::atomic::Ordering::Relaxed),
                        notes: Vec::new(),
                    });
                    if opts.verbose {
                        log(&format!("    anneal chain {chain} iter {iter}: cost {c}"));
                    }
                }
            }
            temp = (temp * 0.999).max(0.05);
        }
        Ok(())
    });
    let mut found = shared.into_inner().unwrap().context("nothing found")?;
    found.evaluations = evals.load(std::sync::atomic::Ordering::Relaxed);
    Ok(found)
}

/// The judge's open/close cycles from the current (built) state of the sandbox: `rest` is the
/// region the build must keep returning to.
pub fn cycles_from_here(w: &mut Worker, sb: u32, d: &Design, rest: &Region, cycles: u32) -> Result<Vec<(u32, u32)>> {
    let spec = DoorSpec::from_design(d)?;
    let (wlo, whi) = watch_box(d, &spec);
    let now = w.get_region(sb, wlo, whi)?;
    let problems = crate::closed_problems(&now, &spec);
    if !problems.is_empty() {
        bail!("not closed after building: {}", problems.join("; "));
    }
    let lever = now.get(spec.lever).unwrap_or("?");
    if !lever.starts_with("minecraft:lever") {
        bail!("port lever {:?} is {lever}, expected a lever", spec.lever);
    }
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| now.get(*p).unwrap().to_string()).collect();
    let mut out = Vec::new();
    for cycle in 0..cycles {
        w.use_block(sb, spec.lever)?;
        let open = w.step_record(sb, 400, true, wlo, whi)?;
        if open.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after opening", open.ticked);
        }
        let last = open.frames.last().context("no frames")?;
        let problems = open_problems(last, &spec);
        if !problems.is_empty() {
            bail!("cycle {cycle}: not open after the lever was switched on: {}", problems.join("; "));
        }
        let open_ticks = settled_from(&open.frames, |f| open_problems(f, &spec).is_empty()).unwrap_or(0);
        if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while opening");
        }
        w.use_block(sb, spec.lever)?;
        let close = w.step_record(sb, 400, true, wlo, whi)?;
        if close.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after closing", close.ticked);
        }
        let last = close.frames.last().context("no frames")?;
        let d2 = diffs(rest, last);
        if !d2.is_empty() {
            let shown: Vec<String> = d2.iter().take(6).map(|(p, a, b)| format!("{p:?} {a} -> {b}")).collect();
            bail!("cycle {cycle}: the build did not return to its rest state: {}", shown.join("; "));
        }
        if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while closing");
        }
        let close_ticks = settled_from(&close.frames, |f| {
            spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str()))
        })
        .unwrap_or(0);
        out.push((open_ticks, close_ticks));
    }
    Ok(out)
}

/// Translate a program.
pub fn translated(steps: &[Step], off: Pos) -> Vec<Step> {
    steps.iter().map(|s| Step { pos: [s.pos[0] + off[0], s.pos[1] + off[1], s.pos[2] + off[2]], ..s.clone() }).collect()
}

/// Build with `steps` at offset `off`, compare with the judge's rest state there, run the cycles.
pub fn check_at(w: &mut Worker, d: &Design, steps: &[Step], off: Pos, opts: &SimOptions, cycles: u32) -> Result<Vec<(u32, u32)>> {
    let dt = d.translated(off);
    let sb = w.create(&sandbox_options())?;
    let r = (|| {
        let rest = judge_rest(w, sb, &dt)?;
        let out = simulate(w, sb, &dt, &translated(steps, off), opts)?;
        let d2 = diffs(&rest, &out.region);
        if !d2.is_empty() || out.entities > 0 {
            let shown: Vec<String> = d2.iter().take(6).map(|(p, a, b)| format!("{p:?} rest {a}, built {b}")).collect();
            bail!("the build does not end in the rest state ({} cells, {} entities): {}", d2.len(), out.entities, shown.join("; "));
        }
        cycles_from_here(w, sb, &dt, &rest, cycles)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// What happens tick by tick when `step` runs after `prefix` (built and settled first): one
/// line per tick that changed something, "t<n>: x y z old -> new; ...".
pub fn trace_step(w: &mut Worker, sb: u32, p: &Problem, prefix: &[Step], step: &[Step], opts: &SimOptions) -> Result<Vec<String>> {
    simulate(w, sb, &p.design, prefix, opts)?;
    let (lo, hi) = watch_box(&p.design, &p.spec);
    let before = w.get_region(sb, lo, hi)?;
    let list: Vec<Value> = step
        .iter()
        .map(|s| match &s.action {
            Action::Place(st) => json!({"do": "place", "pos": s.pos, "state": st}),
            Action::Use => json!({"do": "use", "pos": s.pos}),
            Action::Break => json!({"do": "break", "pos": s.pos}),
        })
        .collect();
    let v = w.call(json!({"op": "build", "sandbox": sb, "steps": list, "settle": 0, "min_ticks": 0, "stop_on_fail": false}))?;
    let click = v["results"][0]["click"].as_str().unwrap_or("").to_string();
    let placed = w.get_region(sb, lo, hi)?;
    let rec = w.step_record(sb, 60, true, lo, hi)?;
    let mut lines = Vec::new();
    let change = |a: &Region, b: &Region| -> Vec<String> {
        a.iter()
            .filter(|(q, s)| b.get(*q) != Some(*s))
            .map(|(q, s)| format!("{} {} {} {} -> {}", q[0], q[1], q[2], short(s), short(b.get(q).unwrap_or("?"))))
            .collect()
    };
    let c0 = change(&before, &placed);
    if !c0.is_empty() {
        lines.push(format!("t0 ({click}): {}", c0.join("; ")));
    }
    let mut prev = placed;
    for (t, f) in rec.frames.iter().enumerate() {
        let c = change(&prev, f);
        if !c.is_empty() {
            let shown: Vec<String> = c.iter().take(6).cloned().collect();
            lines.push(format!("t{}: {}{}", t + 1, shown.join("; "), if c.len() > 6 { format!("; {} more", c.len() - 6) } else { String::new() }));
        }
        prev = f.clone();
        if lines.len() >= 12 {
            lines.push("...".into());
            break;
        }
    }
    Ok(lines)
}

fn short(s: &str) -> String {
    s.strip_prefix("minecraft:").unwrap_or(s).to_string()
}

/// Human build script: one line per action.
pub fn script(steps: &[Step], out: &Outcome) -> String {
    let mut s = String::new();
    for (i, st) in steps.iter().enumerate() {
        let r = out.steps.get(i);
        let p = st.pos;
        let what = match &st.action {
            Action::Place(state) => format!("place {} at {} {} {}", short(state), p[0], p[1], p[2]),
            Action::Use => format!("right-click {} {} {}", p[0], p[1], p[2]),
            Action::Break => format!("break {} {} {}", p[0], p[1], p[2]),
        };
        let how = r.and_then(|r| r.click.clone()).map(|c| format!(": {c}")).unwrap_or_default();
        let note = if st.note.is_empty() { String::new() } else { format!(" ({})", st.note) };
        let bad = match r {
            Some(r) if !r.ok => format!("  !! {}", r.error.clone().unwrap_or_default()),
            _ => String::new(),
        };
        // Something moved or fired after this action (pistons, a sculk sensor...).
        let busy = match r {
            Some(r) if r.ticks > 4 => format!(" [things move for {} ticks]", r.ticks),
            _ => String::new(),
        };
        s.push_str(&format!("{:3}. {what}{how}{note}{busy}{bad}\n", i + 1));
    }
    s
}

/// Which components a failed best program leaves wrong, grouped by block.
pub fn blocking(rest: &Region, out: &Outcome) -> Vec<String> {
    let mut by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (p, r, g) in diffs(rest, &out.region) {
        let key = if is_air(&r) { short(block_name(&g)) } else { short(block_name(&r)) };
        by.entry(key).or_default().push(format!("{} {} {}: rest {}, built {}", p[0], p[1], p[2], short(&r), short(&g)));
    }
    by.into_iter().map(|(k, v)| format!("{k}: {}", v.join("; "))).collect()
}

/// Load a design from a file, `@builtin`, or `<git ref>:<path>` (e.g.
/// `origin/explore/speed:designs/piston-door-3x3/tracks/speed/BEST.txt`).
pub fn load_any(spec: &str) -> Result<Design> {
    let path = std::path::Path::new(spec);
    if spec.starts_with('@') || path.exists() {
        return crate::cli::load_design(path);
    }
    if spec.contains(':') {
        let out = std::process::Command::new("git").args(["show", spec]).output()?;
        if !out.status.success() {
            bail!("git show {spec}: {}", String::from_utf8_lossy(&out.stderr));
        }
        return Design::parse(&String::from_utf8_lossy(&out.stdout)).with_context(|| format!("parsing {spec}"));
    }
    bail!("no such design {spec}")
}

/// Verdict for one design.
pub struct Verdict {
    pub name: String,
    pub buildable: bool,
    pub summary: String,
    pub text: String,
}

/// Options of `door buildable`.
#[derive(Clone, Debug)]
pub struct CliOptions {
    pub budget: u64,
    pub workers: usize,
    pub helpers: bool,
    pub positions: usize,
    pub seed: u64,
    pub verbose: bool,
    pub lever: Option<bool>,
}

pub fn judge_design(version: &str, spec: &str, o: &CliOptions) -> Result<Verdict> {
    let d = load_any(spec)?;
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let mut w = Worker::spawn(&cfg)?;
    let p = Problem::new(&mut w, &d)?;
    let log = |s: &str| eprintln!("{s}");
    eprintln!("{spec}: {} blocks to place ({} in the design file), searching up to {}s", p.items.len(), crate::block_count(&d), o.budget);
    let opts = SearchOptions {
        budget: Duration::from_secs(o.budget),
        workers: o.workers,
        sim: SimOptions::default(),
        helpers: o.helpers,
        seed: o.seed,
        verbose: o.verbose,
        lever: o.lever,
    };
    let t0 = Instant::now();
    let found = search(version, &p, &opts, &log)?;
    let secs = t0.elapsed().as_secs_f64();
    let mut text = String::new();
    let name = d.name.clone();
    if found.cost == 0.0 {
        // Re-check the program from scratch at the judge's positions, with the cycles.
        let mut offs = crate::VerifyOptions::default().offsets;
        offs.extend(crate::extra_offsets(28));
        offs.truncate(o.positions.max(1));
        let mut ok = 0;
        let mut fails = Vec::new();
        let mut timing = None;
        // One position per worker (each job builds in its own sandbox).
        let pool = Pool::spawn(o.workers.min(offs.len()).max(1), &cfg)?;
        let results = pool.map(offs.clone(), |pw, _, off| check_at(pw, &d, &found.steps, off, &opts.sim, 3));
        drop(pool);
        for (off, r) in offs.iter().zip(results) {
            match r {
                Ok(t) => {
                    ok += 1;
                    if timing.is_none() {
                        timing = t.first().copied();
                    }
                }
                Err(e) => fails.push(format!("{off:?}: {e:#}")),
            }
        }
        let helpers_used = found.plan.helpers.len();
        let scaffolds = found.outcome.unsupported();
        let buildable = ok == offs.len();
        let summary = format!(
            "{} ({} actions{}{}{}; {}/{} positions{})",
            if buildable { "BUILDABLE" } else { "BUILDABLE only at some positions" },
            found.steps.len(),
            if found.plan.lever_on { ", lever ON while building" } else { ", lever off while building" },
            if helpers_used > 0 { format!(", {helpers_used} temporary helper blocks") } else { String::new() },
            if scaffolds > 0 { format!(", {scaffolds} placed against a temporary dirt block") } else { String::new() },
            ok,
            offs.len(),
            timing.map(|(op, cl)| format!(", the built door opens in {op} / closes in {cl} ticks")).unwrap_or_default(),
        );
        text.push_str(&format!("# {name}: {summary}\n# source: {spec}\n# found by {} after {} simulated builds in {secs:.0}s\n", found.how, found.evaluations));
        for n in &found.notes {
            text.push_str(&format!("# note: {n}\n"));
        }
        for f in &fails {
            text.push_str(&format!("# fails at {f}\n"));
        }
        text.push_str("# Build script (design coordinates; the world settles after every action):\n");
        text.push_str(&script(&found.steps, &found.outcome));
        Ok(Verdict { name, buildable, summary, text })
    } else {
        let mut blk = blocking(&p.rest, &found.outcome);
        let mut restless = String::new();
        if !found.outcome.idle {
            // The build never comes to rest (a clock): which blocks keep changing?
            let sb = w.create(&sandbox_options())?;
            simulate(&mut w, sb, &p.design, &found.steps, &opts.sim)?;
            let (lo, hi) = watch_box(&p.design, &p.spec);
            let rec = w.step_record(sb, 20, false, lo, hi)?;
            let _ = w.drop_sandbox(sb);
            let mut moving: BTreeMap<String, BTreeSet<Pos>> = BTreeMap::new();
            for pair in rec.frames.windows(2) {
                for (q, s) in pair[0].iter() {
                    if pair[1].get(q) != Some(s) {
                        moving.entry(short(block_name(s))).or_default().insert(q);
                    }
                }
            }
            for (k, v) in &moving {
                blk.push(format!("{k}: never stops changing at {}", v.iter().map(|q| format!("{} {} {}", q[0], q[1], q[2])).collect::<Vec<_>>().join("; ")));
            }
            restless = ", the build never comes to rest (a clock)".to_string();
        }
        let failed: Vec<String> = found
            .steps
            .iter()
            .zip(found.outcome.steps.iter())
            .filter(|(_, r)| !r.ok)
            .map(|(s, r)| format!("{:?}: {}", s.pos, r.error.clone().unwrap_or_default()))
            .collect();
        let mut blocking_names: Vec<String> = blk.iter().map(|b| b.split(':').next().unwrap_or("").to_string()).collect();
        blocking_names.dedup();
        let summary = format!(
            "NOT FOUND (best program leaves {} cells wrong, {} failed placements, {} entities{restless}; blocking: {})",
            diffs(&p.rest, &found.outcome.region).len(),
            found.outcome.failed(),
            found.outcome.entities,
            blocking_names.join(", ")
        );
        text.push_str(&format!("# {name}: {summary}\n# source: {spec}\n# searched {} simulated builds in {secs:.0}s; best by {}\n", found.evaluations, found.how));
        text.push_str("# Cells the best program gets wrong:\n");
        for b in &blk {
            text.push_str(&format!("#   {b}\n"));
        }
        for f in &failed {
            text.push_str(&format!("# failed placement {f}\n"));
        }
        text.push_str("# Best program:\n");
        text.push_str(&script(&found.steps, &found.outcome));
        Ok(Verdict { name, buildable: false, summary, text })
    }
}

pub fn run_cli(version: &str, designs: &[String], o: &CliOptions, out_dir: Option<&std::path::Path>, table: Option<&std::path::Path>) -> Result<()> {
    for spec in designs {
        let v = match judge_design(version, spec, o) {
            Ok(v) => v,
            Err(e) => Verdict { name: spec.clone(), buildable: false, summary: format!("ERROR: {e:#}"), text: format!("# {spec}: ERROR: {e:#}\n") },
        };
        println!("{spec}: {}", v.summary);
        if o.verbose || out_dir.is_none() {
            print!("{}", v.text);
        }
        if let Some(dir) = out_dir {
            std::fs::create_dir_all(dir)?;
            // Named after the source file (several files can share a design name).
            let stem = std::path::Path::new(spec.rsplit(':').next().unwrap_or(spec))
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| v.name.replace('/', "_"));
            let file = dir.join(format!("{stem}.build.txt"));
            std::fs::write(&file, &v.text)?;
            println!("  wrote {}", file.display());
        }
        if let Some(t) = table {
            use std::io::Write;
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(t)?;
            writeln!(f, "| `{spec}` | {} | {} |", if v.buildable { "yes" } else { "no" }, v.summary.replace('|', "/"))?;
        }
    }
    Ok(())
}
