//! `mctas door satwire`: find the WIRING of a flat door for a FIXED mechanism with a SAT solver.
//!
//! The mechanism (pistons + door blocks) and the power each piston must get at each tick come
//! from one recorded run: an ideal schedule replayed in the simulator (`--schedule`), or a wired
//! design whose wiring is stripped (`--from-wired`). Wiring goes in the free cells of a box in
//! the door plane: one-hot placement over a palette (block, dust, repeater 1-4 x E/W, observer
//! E/W/U/D, torch, wall torch, note block, copper bulb, comparator), plus one lever among
//! candidate cells. Two traces share the placement: lever ON from rest, lever OFF from open.
//!
//! Semantics (game ticks, read from the 26.3 source; see docs in `satwire` NOTES):
//! * combinational at each tick: dust levels (exact, order-encoded 15..1, horizontal + diagonal
//!   wire rules of `RedstoneWireEvaluator`), strong/weak power of conductors (wiring blocks and
//!   moving door blocks at their recorded cells), signals into receivers (`getSignal`);
//! * repeaters (phase R, first in a tick): fire reads the input at the start of the tick, pulse
//!   extension (`!on -> on`, reschedule when the input is off), a mismatch schedules `2*delay`
//!   later unless a tick is pending;
//! * observers: fire toggles (on schedules off 2 gt later); a change of the watched cell
//!   schedules a tick 2 gt later when the observer is off and nothing is pending; repeater
//!   changes are seen before the observer's own fire, others after (an off-fire coinciding with
//!   an ambiguous change is forbidden);
//! * torches: fire sets lit = !input (input after repeaters/observers), mismatch schedules +2;
//! * comparators (rear only in a flat door): out = rear input 2 gt later (races forbidden);
//!   they read a copper bulb's lit state; bulbs toggle lit on each rising input (same tick);
//!   note blocks are conductors whose `powered` state observers can watch;
//! * pistons: direct power (or quasi-connectivity, `--qc`) must equal the recorded power at
//!   every tick of both traces; quasi power must be off whenever the piston must be off.
//! Constraints: supports, sticky-pull cells hold nothing movable, ring cells only plain blocks
//! (`--forbid-ring`), both stable states are fixed points and each trace ends where the other
//! starts. Every model is judged by the REAL simulator (CEGAR): a failing design is blocked and
//! the solver asked again; the block count is minimized with a totalizer.

use crate::DoorSpec;
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker};
use rustsat::solvers::{Solve, SolveIncremental, SolverResult};
use rustsat::types::{Clause, Lit, TernaryVal};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub type C2 = (i32, i32);
pub const E: C2 = (1, 0);
pub const W: C2 = (-1, 0);
pub const U: C2 = (0, 1);
pub const DN: C2 = (0, -1);
pub const DIRS: [C2; 4] = [E, W, U, DN];

pub fn add(a: C2, b: C2) -> C2 {
    (a.0 + b.0, a.1 + b.1)
}
pub fn neg(a: C2) -> C2 {
    (-a.0, -a.1)
}
pub fn dname(d: C2) -> &'static str {
    match d {
        E => "east",
        W => "west",
        U => "up",
        _ => "down",
    }
}
pub fn dparse(s: &str) -> Option<C2> {
    match s {
        "east" => Some(E),
        "west" => Some(W),
        "up" => Some(U),
        "down" => Some(DN),
        _ => None,
    }
}
fn short(s: &str) -> &str {
    let s = s.strip_prefix("minecraft:").unwrap_or(s);
    s.split('[').next().unwrap_or(s)
}
fn prop<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let inner = s.split_once('[')?.1.strip_suffix(']')?;
    inner.split(',').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v)
    })
}

// ---------------------------------------------------------------- palette

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub enum Kind {
    Empty,
    /// smooth stone: a conductor.
    Block,
    Dust,
    /// delay 1..4, the INPUT side (E or W): output on the opposite side.
    Rep(u8, C2),
    /// the WATCHED side (E/W/U/D): output on the opposite side.
    Obs(C2),
    /// standing torch (support below).
    Torch,
    /// wall torch facing f (E/W): attached to the block at -f.
    WTorch(C2),
    /// note block: conductor; observers see its `powered`.
    Note,
    /// waxed copper bulb: T flip-flop on rising input, not a conductor, read by comparators.
    Bulb,
    /// comparator (compare mode), REAR side (E/W): output on the opposite side.
    Cmp(C2),
    /// redstone lamp: conductor; lit at once when powered, off 4 gt after it loses power.
    Lamp,
    /// iron trapdoor (closed, bottom half): not a conductor; observers see its powered/open state.
    Trap,
    /// dropper facing up: conductor; `triggered` follows its power (direct or quasi-connected).
    Dropper,
    /// obsidian: conductor that pistons cannot move.
    Obsid,
    /// glass: a full block that is NOT a conductor (holds dust, repeaters, torches; never powered).
    Glass,
    /// persistent oak leaves: re-read their distance 1 gt after a neighbor changes (a 1 gt
    /// delay when a LOG door block moves next to them, `--log-doors`); observers see it.
    Leaves,
    /// an UPDATE piston (normal, facing f; its head cell stays free): extends in the tick its
    /// power arrives and retracts when it goes; observers watching its base see both (the
    /// block-update side effect on QC-armed pistons is not modeled). Costs 1 block.
    UPiston(C2),
    /// dust shaped as a dot (no connections): powers only the block below it.
    Dot,
    /// a redstone block: a constant source into its 4 in-plane neighbors (not a conductor).
    RBlock,
    /// a closed top-half iron trapdoor: like `Trap`, and its top face holds dust and diodes.
    TrapTop,
}

impl Kind {
    pub fn cond(self) -> bool {
        matches!(self, Kind::Block | Kind::Note | Kind::Lamp | Kind::Dropper | Kind::Obsid)
    }
    pub fn sturdy(self) -> bool {
        matches!(self, Kind::Block | Kind::Obs(_) | Kind::Note | Kind::Bulb | Kind::Lamp | Kind::Dropper | Kind::Obsid | Kind::Glass | Kind::RBlock | Kind::TrapTop)
    }
    pub fn movable(self) -> bool {
        matches!(self, Kind::Block | Kind::Obs(_) | Kind::Note | Kind::Bulb | Kind::Lamp | Kind::Trap | Kind::Glass | Kind::UPiston(_))
    }
    pub fn has_out(self) -> bool {
        matches!(self, Kind::Rep(..) | Kind::Obs(_) | Kind::Torch | Kind::WTorch(_) | Kind::Cmp(_) | Kind::Lamp | Kind::UPiston(_))
    }
    /// Relative cells the output (any signal) reaches.
    pub fn out_to(self) -> Vec<C2> {
        match self {
            Kind::Rep(_, f) | Kind::Obs(f) | Kind::Cmp(f) => vec![neg(f)],
            Kind::Torch => vec![E, W, U],
            Kind::WTorch(f) => DIRS.iter().copied().filter(|d| *d != neg(f)).collect(),
            _ => vec![],
        }
    }
    /// Relative cells the output strongly powers.
    pub fn strong_to(self) -> Vec<C2> {
        match self {
            Kind::Rep(_, f) | Kind::Obs(f) | Kind::Cmp(f) => vec![neg(f)],
            Kind::Torch | Kind::WTorch(_) => vec![U],
            _ => vec![],
        }
    }
    pub fn support(self) -> Option<C2> {
        match self {
            Kind::Dust | Kind::Dot | Kind::Rep(..) | Kind::Cmp(_) | Kind::Torch => Some(DN),
            Kind::WTorch(f) => Some(neg(f)),
            _ => None,
        }
    }
    /// Block state with the rest-state values (`out`: powered/lit, `lvl`: dust power, `a`: bulb lit,
    /// `b`: bulb/note powered).
    pub fn state(self, out: bool, lvl: u8, a: bool, b: bool) -> String {
        match self {
            Kind::Empty => "minecraft:air".into(),
            Kind::Block => "minecraft:smooth_stone".into(),
            Kind::Dust => format!("minecraft:redstone_wire[east=side,north=side,power={lvl},south=side,west=side]"),
            Kind::Dot => format!("minecraft:redstone_wire[east=none,north=none,power={lvl},south=none,west=none]"),
            Kind::Rep(d, f) => format!("minecraft:repeater[delay={d},facing={},locked=false,powered={out}]", dname(f)),
            Kind::Obs(f) => format!("minecraft:observer[facing={},powered=false]", dname(f)),
            Kind::Torch => format!("minecraft:redstone_torch[lit={out}]"),
            Kind::WTorch(f) => format!("minecraft:redstone_wall_torch[facing={},lit={out}]", dname(f)),
            Kind::Note => format!("minecraft:note_block[instrument=harp,note=0,powered={b}]"),
            Kind::Bulb => format!("minecraft:waxed_copper_bulb[lit={a},powered={b}]"),
            Kind::Cmp(f) => format!("minecraft:comparator[facing={},mode=compare,powered={out}]", dname(f)),
            Kind::Lamp => format!("minecraft:redstone_lamp[lit={out}]"),
            Kind::Trap => format!("minecraft:iron_trapdoor[facing=north,half=bottom,open={b},powered={b},waterlogged=false]"),
            Kind::Dropper => format!("minecraft:dropper[facing=up,triggered={b}]"),
            Kind::Obsid => "minecraft:obsidian".into(),
            Kind::Glass => "minecraft:glass".into(),
            Kind::Leaves => format!("minecraft:oak_leaves[distance={lvl},persistent=true,waterlogged=false]"),
            Kind::UPiston(f) => format!("minecraft:piston[extended=false,facing={}]", dname(f)),
            Kind::RBlock => "minecraft:redstone_block".into(),
            Kind::TrapTop => format!("minecraft:iron_trapdoor[facing=north,half=top,open={b},powered={b},waterlogged=false]"),
        }
    }
    /// The kind of a wiring block state (None: not wiring).
    pub fn of_state(s: &str) -> Option<Kind> {
        let f = |k: &str| prop(s, k).and_then(dparse);
        Some(match short(s) {
            "smooth_stone" | "stone" => Kind::Block,
            // (a log is a plain conductor: --log-doors designs use logs for static frame blocks)
            n if n.ends_with("_log") => Kind::Block,
            "redstone_wire" if ["east", "west", "north", "south"].iter().all(|k| prop(s, k) == Some("none")) => Kind::Dot,
            "redstone_wire" => Kind::Dust,
            "repeater" => Kind::Rep(prop(s, "delay")?.parse().ok()?, f("facing")?),
            "observer" => Kind::Obs(f("facing")?),
            "redstone_torch" => Kind::Torch,
            "redstone_wall_torch" => Kind::WTorch(f("facing")?),
            "note_block" => Kind::Note,
            "waxed_copper_bulb" => Kind::Bulb,
            "comparator" => Kind::Cmp(f("facing")?),
            "redstone_lamp" => Kind::Lamp,
            "iron_trapdoor" if prop(s, "half") == Some("top") => Kind::TrapTop,
            "iron_trapdoor" => Kind::Trap,
            "dropper" | "dispenser" => Kind::Dropper,
            "obsidian" | "crying_obsidian" => Kind::Obsid,
            "redstone_block" => Kind::RBlock,
            "glass" => Kind::Glass,
            n if n.ends_with("_leaves") => Kind::Leaves,
            "piston" => Kind::UPiston(f("facing")?),
            _ => return None,
        })
    }
}

/// Expand `--palette` names: block, dust, rep (rep1..rep4), obs, torch (standing + wall), note,
/// bulb, cmp.
pub fn palette(spec: &str) -> Result<Vec<Kind>> {
    let mut v = vec![Kind::Empty];
    for t in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match t {
            "block" => v.push(Kind::Block),
            "dust" => v.push(Kind::Dust),
            "dot" => v.push(Kind::Dot),
            "rep" | "rep1" | "rep2" | "rep3" | "rep4" => {
                let ds: Vec<u8> = match t.strip_prefix("rep").and_then(|d| d.parse().ok()) {
                    Some(d) => vec![d],
                    None => vec![1, 2, 3, 4],
                };
                for d in ds {
                    v.push(Kind::Rep(d, E));
                    v.push(Kind::Rep(d, W));
                }
            }
            "obs" => v.extend(DIRS.iter().map(|d| Kind::Obs(*d))),
            "torch" => v.extend([Kind::Torch, Kind::WTorch(E), Kind::WTorch(W)]),
            "note" => v.push(Kind::Note),
            "bulb" => v.push(Kind::Bulb),
            "cmp" => v.extend([Kind::Cmp(E), Kind::Cmp(W)]),
            "lamp" => v.push(Kind::Lamp),
            "trapdoor" => v.push(Kind::Trap),
            "dropper" => v.push(Kind::Dropper),
            "obsidian" => v.push(Kind::Obsid),
            "glass" => v.push(Kind::Glass),
            "leaves" => v.push(Kind::Leaves),
            "upiston" => v.extend(DIRS.iter().map(|d| Kind::UPiston(*d))),
            "rblock" => v.push(Kind::RBlock),
            "traptop" => v.push(Kind::TrapTop),
            _ => bail!("unknown palette entry {t:?} (block,dust,rep,rep1..4,obs,torch,note,bulb,cmp,lamp,trapdoor,dropper,obsidian,glass,leaves,upiston)"),
        }
    }
    v.sort();
    v.dedup();
    Ok(v)
}

// ---------------------------------------------------------------- the mechanism recording

/// What the mechanism holds at a cell.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MK {
    Air,
    /// full conductor (door blocks, stone)
    Cond,
    /// full block that does not conduct (glass, slime...)
    Solid,
    Piston { f: C2, ext: bool, sticky: bool },
    /// a redstone block moved by the mechanism (a power source for its neighbors)
    RBlock,
    /// piston head, moving piston
    Other,
}

pub fn mk(s: &str) -> MK {
    let n = short(s);
    match n {
        "air" | "cave_air" | "void_air" => MK::Air,
        "piston" | "sticky_piston" => MK::Piston {
            f: prop(s, "facing").and_then(dparse).unwrap_or((0, 0)),
            ext: prop(s, "extended") == Some("true"),
            sticky: n == "sticky_piston",
        },
        "piston_head" | "moving_piston" => MK::Other,
        "redstone_block" => MK::RBlock,
        n if n.contains("glass") || n == "slime_block" || n == "honey_block" || n == "ice" => MK::Solid,
        n if unsupported(n) => MK::Other,
        _ => MK::Cond,
    }
}

/// Blocks with redstone behavior satwire does not model (they must not pass as plain conductors).
pub fn unsupported(n: &str) -> bool {
    n.contains("sculk") || n.contains("rail") || n.ends_with("_button") || n.ends_with("pressure_plate") || n.ends_with("glazed_terracotta")
        || (n.ends_with("trapdoor") && n != "iron_trapdoor") || (n.ends_with("_door") && n != "iron_door")
        || matches!(n, "target" | "daylight_detector" | "tripwire" | "tripwire_hook" | "lectern" | "hopper" | "dispenser" | "calibrated_sculk_sensor" | "crafter" | "iron_door")
}

fn is_wiring_state(s: &str) -> bool {
    matches!(
        short(s),
        "redstone_wire" | "repeater" | "comparator" | "observer" | "redstone_torch" | "redstone_wall_torch" | "lever" | "redstone_block" | "note_block" | "redstone_lamp" | "target" | "waxed_copper_bulb" | "dropper" | "dispenser" | "iron_trapdoor" | "sculk_sensor" | "calibrated_sculk_sensor"
    )
}

#[derive(Clone, Debug, Default)]
pub struct Phase {
    /// Mechanism blocks after each tick (`frames[0]` = at the flip, before tick 1).
    pub frames: Vec<HashMap<C2, String>>,
    /// `power[j]` = power each piston base of `frames[j-1]` gets during tick j; `power[0]` = the
    /// stable power before the flip (pistons of `frames[0]`).
    pub power: Vec<HashMap<C2, bool>>,
    /// `oop[j]` = the cells with an ideal redstone block right in front of / behind them, read
    /// with `power[j]` (ideal plans only; `--keep-ideal` keeps some of them as given sources).
    pub oop: Vec<HashSet<C2>>,
}

impl Phase {
    pub fn k(&self) -> i32 {
        self.frames.len() as i32 - 1
    }
    pub fn frame(&self, j: i32) -> &HashMap<C2, String> {
        &self.frames[j.clamp(0, self.k()) as usize]
    }
    pub fn pow(&self, j: i32) -> &HashMap<C2, bool> {
        &self.power[j.clamp(0, self.power.len() as i32 - 1) as usize]
    }
    /// Drop trailing frames identical to the last change (keep 2 extra).
    fn trim(&mut self) {
        let mut last = 0;
        for j in 1..self.frames.len() {
            if self.frames[j] != self.frames[j - 1] || self.power.get(j) != self.power.get(j - 1) {
                last = j;
            }
        }
        let keep = (last + 3).min(self.frames.len());
        self.frames.truncate(keep);
        self.power.truncate(keep);
        self.oop.truncate(keep);
    }
    /// The ideal off-plane input at cell c is in place at power index j.
    pub fn oop_at(&self, j: i32, c: C2) -> bool {
        if self.oop.is_empty() {
            return false;
        }
        self.oop[j.clamp(0, self.oop.len() as i32 - 1) as usize].contains(&c)
    }
    /// Repeat the last frame (and power) until there are `n` frames.
    pub fn pad(&mut self, n: usize) {
        while self.frames.len() < n {
            let f = self.frames.last().cloned().unwrap_or_default();
            self.frames.push(f);
        }
        while self.power.len() < n {
            let p = self.power.last().cloned().unwrap_or_default();
            self.power.push(p);
        }
        while !self.oop.is_empty() && self.oop.len() < n {
            let p = self.oop.last().cloned().unwrap_or_default();
            self.oop.push(p);
        }
    }
}

/// Alternative recordings of a group of mechanism cells for one trace (`--menu`: a toggle
/// piston's pulse retimed or tripled): the solver picks one alternative per group; the group's
/// cells take their frames and piston power from it, every other cell from the base recording.
#[derive(Clone, Debug)]
pub struct AltGroup {
    pub name: String,
    pub tr: usize,
    pub cells: BTreeSet<C2>,
    /// (label, the whole trace recorded with that alternative; only `cells` are read)
    pub alts: Vec<(String, Phase)>,
}

#[derive(Clone, Debug)]
pub struct Rec {
    pub z: i32,
    pub door: Pos,
    /// Mechanism rest blocks (3D).
    pub rest: BTreeMap<Pos, String>,
    pub phases: [Phase; 2],
    /// The wiring of a wired source design (for `--check`), by cell.
    pub wiring: BTreeMap<C2, String>,
    pub lever: Option<(Pos, String)>,
    /// In-plane bounds of the source design (wired source), lever excluded.
    pub src_box: Option<(i32, i32, i32, i32)>,
    /// Wired source: every in-plane block after each tick (wiring included), per phase.
    pub full: Vec<Vec<HashMap<C2, String>>>,
    /// Alternative recordings of cell groups (`--menu`), empty by default.
    pub groups: Vec<AltGroup>,
}

impl Rec {
    /// Every recorded phase of a trace: the base and each alternative of its groups.
    pub fn trace_phases(&self, tr: usize) -> Vec<&Phase> {
        let mut v = vec![&self.phases[tr]];
        for g in self.groups.iter().filter(|g| g.tr == tr) {
            v.extend(g.alts.iter().map(|a| &a.1));
        }
        v
    }
    /// Every recorded phase of both traces.
    pub fn all_phases(&self) -> Vec<&Phase> {
        let mut v = self.trace_phases(0);
        v.extend(self.trace_phases(1));
        v
    }
}

fn place_settled(w: &mut Worker, d: &Design) -> Result<u32> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<_> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<_> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    w.step_until_idle(sb, 400)?;
    Ok(sb)
}

fn plane_cells(lo: Pos, hi: Pos, z: i32) -> Vec<Pos> {
    let mut v = Vec::new();
    for y in lo[1]..=hi[1] {
        for x in lo[0]..=hi[0] {
            v.push([x, y, z]);
        }
    }
    v
}

/// Record a WIRED design: its mechanism frames and every piston's real power per tick (the
/// signal probe into the base cell).
pub fn record_wired(w: &mut Worker, d: &Design, ticks: u32) -> Result<Rec> {
    let spec = DoorSpec::from_design(d)?;
    let z = spec.door[2];
    let sb = place_settled(w, d)?;
    let (mut lo, mut hi) = d.bounds().context("empty design")?;
    for i in 0..3 {
        lo[i] -= 2;
        hi[i] += 2;
    }
    let probes = plane_cells(lo, hi, z);
    let rest_region = w.get_region(sb, lo, hi)?;
    let sig0 = w.signals(sb, &probes)?;
    let mut full: Vec<Vec<HashMap<C2, String>>> = Vec::new();
    let mut sigs: Vec<Vec<Vec<u8>>> = Vec::new();
    let mut pre = vec![sig0.iter().map(|s| s.0).collect::<Vec<u8>>()];
    for _flip in 0..2 {
        w.use_block(sb, spec.lever)?;
        let f0 = w.get_region(sb, lo, hi)?;
        let r = w.step_probe(sb, ticks, false, &probes, Some((lo, hi)))?;
        let to_map = |reg: &mctas::sim::Region| -> HashMap<C2, String> {
            reg.iter().filter(|(p, s)| p[2] == z && !s.ends_with(":air")).map(|(p, s)| ((p[0], p[1]), s.to_string())).collect()
        };
        let mut fr = vec![to_map(&f0)];
        fr.extend(r.frames.iter().map(to_map));
        full.push(fr);
        let s: Vec<Vec<u8>> = r.signals.iter().map(|row| row.chunks(2).map(|c| c[0]).collect()).collect();
        pre.push(s.last().cloned().unwrap_or_default());
        sigs.push(s);
    }
    let _ = w.drop_sandbox(sb);
    // wiring cells: redstone components at rest, and static plain blocks outside the door
    let door_cells: HashSet<C2> = spec.door_cells().iter().map(|p| (p[0], p[1])).collect();
    let mut wiring = BTreeMap::new();
    let mut lever = None;
    for (p, s) in rest_region.iter() {
        if p[2] != z || s.ends_with(":air") {
            if short(s) == "lever" {
                lever = Some((p, s.to_string()));
            }
            continue;
        }
        let c = (p[0], p[1]);
        if short(s) == "lever" {
            lever = Some((p, s.to_string()));
            continue;
        }
        let stat = full.iter().all(|ph| ph.iter().all(|f| f.get(&c).map(String::as_str) == Some(s)));
        let plain = matches!(mk(s), MK::Cond | MK::Solid) && !door_cells.contains(&c);
        let moves = phases_full_moves(&full, c, s);
        if (is_wiring_state(s) && !(short(s) == "redstone_block" && moves)) || (plain && stat) {
            wiring.insert(c, s.to_string());
        }
    }
    let lever_c = lever.as_ref().map(|(p, _)| (p[0], p[1], p[2]));
    let mut phases: [Phase; 2] = Default::default();
    for ph in 0..2 {
        let frames: Vec<HashMap<C2, String>> = full[ph]
            .iter()
            .map(|f| f.iter().filter(|(c, s)| !wiring.contains_key(c) && short(s) != "lever" && Some((c.0, c.1, z)) != lever_c).map(|(c, s)| (*c, s.clone())).collect())
            .collect();
        let idx = |c: C2| -> Option<usize> { probes.iter().position(|p| p[0] == c.0 && p[1] == c.1) };
        let pistons = |f: &HashMap<C2, String>| -> Vec<C2> { f.iter().filter(|(_, s)| matches!(mk(s), MK::Piston { .. })).map(|(c, _)| *c).collect() };
        let mut power = Vec::new();
        power.push(pistons(&frames[0]).into_iter().map(|c| (c, pre[ph][idx(c).unwrap()] > 0 || idx(add(c, U)).is_some_and(|i| pre[ph][i] > 0))).collect());
        for j in 1..frames.len() {
            let row = &sigs[ph][j - 1];
            power.push(pistons(&frames[j - 1]).into_iter().map(|c| (c, row[idx(c).unwrap()] > 0 || idx(add(c, U)).is_some_and(|i| row[i] > 0))).collect());
        }
        phases[ph] = Phase { frames, power, oop: Vec::new() };
        phases[ph].trim();
    }
    let mut rest = BTreeMap::new();
    for (c, s) in &phases[0].frames[0] {
        rest.insert([c.0, c.1, z], s.clone());
    }
    let mut sbx = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for (p, s) in &d.blocks {
        if p[2] == z && short(s) != "lever" {
            sbx = (sbx.0.min(p[0]), sbx.1.min(p[1]), sbx.2.max(p[0]), sbx.3.max(p[1]));
        }
    }
    Ok(Rec { z, door: spec.door, rest, phases, wiring, lever, src_box: Some(sbx), full, groups: Vec::new() })
}

/// Replay an ideal schedule: the mechanism frames, and the power the redstone blocks give each
/// piston (direct or quasi-connected) per tick.
pub fn record_ideal(w: &mut Worker, s: &crate::ideal::Schedule, ticks: u32) -> Result<Rec> {
    let d = s.design().clone();
    let door = *d.ports.get("door").context("design has no 'port door'")?;
    let spec = DoorSpec { door, lever: door, along_z: false };
    let z = spec.door[2];
    let sb = place_settled(w, &d)?;
    let (mut lo, mut hi) = d.bounds().context("empty design")?;
    for i in 0..3 {
        lo[i] -= 3;
        hi[i] += 3;
    }
    // In-plane blocks, plus the redstone blocks one cell in front of / behind the plane (ideal
    // plans put some inputs right behind the piston they fire: that power is recorded too; the
    // wiring then has to give it from the plane). Fix from wire56 (6d1c760).
    let snap = |w: &mut Worker| -> Result<(HashMap<C2, String>, HashSet<C2>)> {
        let r = w.get_region(sb, lo, hi)?;
        let plane = r.iter().filter(|(p, s)| p[2] == z && !s.ends_with(":air")).map(|(p, s)| ((p[0], p[1]), s.to_string())).collect();
        let oop = r.iter().filter(|(p, s)| (p[2] - z).abs() == 1 && short(s) == "redstone_block").map(|(p, _)| (p[0], p[1])).collect();
        Ok((plane, oop))
    };
    let is_input = |s: &str| matches!(short(s), "redstone_block" | "lever");
    // What the schedule writes into a cell (redstone blocks, glass pokes, a stone put back) is
    // input, not mechanism, while it stays there: a mechanism block under it keeps its last state.
    let inputs: Vec<Pos> = s.input_cells();
    let mech = |f: &HashMap<C2, String>, prev: &HashMap<C2, String>, held: &mut HashMap<C2, String>| -> HashMap<C2, String> {
        let mut out = HashMap::new();
        held.retain(|c, st| f.get(c).map(String::as_str).unwrap_or("minecraft:air") == st.as_str());
        for (c, st) in f {
            if held.contains_key(c) {
                if let Some(m) = prev.get(c) {
                    out.insert(*c, m.clone());
                }
            } else if !(is_input(st) && (short(st) == "lever" || inputs.contains(&[c.0, c.1, z]))) {
                out.insert(*c, st.clone());
            }
        }
        for c in held.keys() {
            if !f.contains_key(c) {
                if let Some(m) = prev.get(c) {
                    out.insert(*c, m.clone());
                }
            }
        }
        out
    };
    // The power each piston gets = the REAL signal into its cell or the cell above it (quasi-
    // connectivity), from the simulator: inputs may sit anywhere (in the plane, in front of or
    // behind it). Adopted from wire64's `--real-power` (the in-plane redstone-block reading
    // below is kept as a fallback for designs without probes).
    let probes = plane_cells(lo, hi, z);
    // A piston ignores its FRONT face (`PistonBaseBlock.getNeighborSignal` skips `facing`), but the
    // simulator's best-neighbor signal counts it: a door block strongly powered in front of a
    // retracted pull piston (TL at (-1,2) in front of L2 in families 8x8-118, wire64) would be
    // recorded as power the model can never (and must never) produce. When every in-plane source
    // of the piston's cell is its front neighbor (and no input sits behind or in front of the
    // plane next to it), the direct power is dropped; quasi-connectivity is unchanged.
    let emits = |f: &HashMap<C2, String>, sig: &[(u8, u8)], n: C2, to: C2| -> bool {
        let Some(st) = f.get(&n) else { return false };
        let d = (to.0 - n.0, to.1 - n.1);
        let on = |k: &str| prop(st, k) == Some("true");
        let back = |st: &str| prop(st, "facing").and_then(dparse).map(|fc| (-fc.0, -fc.1));
        match short(st) {
            "redstone_block" => true,
            "lever" => on("powered"),
            "observer" | "repeater" | "comparator" => on("powered") && back(st) == Some(d),
            "redstone_torch" => on("lit") && d != DN,
            "redstone_wall_torch" => on("lit") && back(st) != Some(d),
            "redstone_wire" => {
                let p: u8 = prop(st, "power").and_then(|v| v.parse().ok()).unwrap_or(0);
                let side = match d {
                    E => prop(st, "east"),
                    W => prop(st, "west"),
                    _ => None,
                };
                p > 0 && (d == DN || side.is_some_and(|v| v != "none"))
            }
            _ => matches!(mk(st), MK::Cond) && probes.iter().position(|p| p[0] == n.0 && p[1] == n.1).is_some_and(|i| sig[i].1 > 0),
        }
    };
    let power_now = |w: &mut Worker, fr: &(HashMap<C2, String>, HashSet<C2>)| -> Result<HashMap<C2, bool>> {
        let (f, oop) = fr;
        let sig = w.signals(sb, &probes)?;
        let on = |c: C2| probes.iter().position(|p| p[0] == c.0 && p[1] == c.1).is_some_and(|i| sig[i].0 > 0);
        Ok(f.iter()
            .filter_map(|(c, st)| match mk(st) {
                MK::Piston { f: fc, .. } => {
                    let front = add(*c, fc);
                    let front_only = on(*c)
                        && !oop.contains(c)
                        && emits(f, &sig, front, *c)
                        && !DIRS.iter().any(|d| *d != fc && emits(f, &sig, add(*c, *d), *c));
                    Some((*c, (on(*c) && !front_only) || on(add(*c, U))))
                }
                _ => None,
            })
            .collect())
    };
    #[allow(unused)]
    let pow_of = |(f, oop): &(HashMap<C2, String>, HashSet<C2>)| -> HashMap<C2, bool> {
        let rb = |c: C2| f.get(&c).is_some_and(|s| short(s) == "redstone_block");
        f.iter()
            .filter_map(|(c, s)| match mk(s) {
                MK::Piston { f: fc, .. } => {
                    let direct = DIRS.iter().any(|d| *d != fc && rb(add(*c, *d))) || oop.contains(c);
                    let q = add(*c, U);
                    let qc = [E, W, U].iter().any(|d| rb(add(q, *d))) || oop.contains(&q);
                    Some((*c, direct || qc))
                }
                _ => None,
            })
            .collect()
    };
    let mut phases: [Phase; 2] = Default::default();
    for (ph, open) in [(0usize, true), (1usize, false)] {
        let evs = s.events_of(open, s.vars())?;
        let before = snap(w)?;
        let mut held: HashMap<C2, String> = HashMap::new();
        let mut prev = mech(&before.0, &HashMap::new(), &mut held);
        let mut frames = Vec::new();
        let mut power = vec![power_now(w, &before)?];
        let mut oops = vec![before.1.clone()];
        for t in 0..=ticks {
            for (et, p, st) in &evs {
                if *et == t {
                    if st.as_deref() == Some("@use") {
                        w.use_block(sb, *p)?;
                        continue;
                    }


                    let state = st.clone().unwrap_or_else(|| "minecraft:air".to_string());
                    if p[2] == z {
                        held.insert((p[0], p[1]), state.clone());
                    }
                    w.set_blocks(sb, &[(*p, state)], PlaceMode::Update)?;
                }
            }
            let f = snap(w)?;
            let mf = mech(&f.0, &prev, &mut held);
            prev = mf.clone();
            frames.push(mf);
            power.push(power_now(w, &f)?);
            oops.push(f.1.clone());
            w.step(sb, 1)?;
        }
        power.truncate(frames.len());
        oops.truncate(frames.len());
        phases[ph] = Phase { frames, power, oop: oops };
        phases[ph].trim();
    }
    let _ = w.drop_sandbox(sb);
    if let Ok(v) = std::env::var("SATWIRE_DEBUG_CELL") {
        let xy: Vec<i32> = v.split(',').filter_map(|t| t.parse().ok()).collect();
        for (i, ph) in phases.iter().enumerate() {
            let seq: Vec<String> = ph.frames.iter().map(|f| f.get(&(xy[0], xy[1])).map(|s| short(s).to_string()).unwrap_or("-".into())).collect();
            println!("debug cell {v} phase {i}: {}", seq.join(" "));
        }
    }
    // wiring already in the design (a partly wired plan with `use` of its real lever): those
    // cells are not mechanism; they are kept as they are (`--fixed` is implied)
    let door_cells: HashSet<C2> = (0..3).flat_map(|i| (0..3).map(move |j| (door[0] + i, door[1] + j))).collect();
    let mut wiring = BTreeMap::new();
    let mut lever = None;
    for (p, st) in &d.blocks {
        if short(st) == "lever" {
            lever = Some((*p, st.clone()));
            continue;
        }
        if p[2] != z || (short(st) == "redstone_block" && s.input_cells().contains(p)) {
            continue;
        }
        let c = (p[0], p[1]);
        let stat = phases.iter().all(|ph| ph.frames.iter().all(|f| f.get(&c).map(|s| short(s)) == Some(short(st))));
        let plain = matches!(mk(st), MK::Cond | MK::Solid) && !door_cells.contains(&c) && !matches!(short(st), "stone") ;
        if (is_wiring_state(st) && (stat || short(st) != "redstone_block")) || (plain && stat) {
            wiring.insert(c, st.clone());
        }
    }
    for ph in phases.iter_mut() {
        for fr in ph.frames.iter_mut() {
            fr.retain(|c, _| !wiring.contains_key(c));
        }
        ph.trim();
    }
    let mut rest = BTreeMap::new();
    for (c, st) in &phases[0].frames[0] {
        rest.insert([c.0, c.1, z], st.clone());
    }
    Ok(Rec { z, door: spec.door, rest, phases, wiring, lever, src_box: None, full: Vec::new(), groups: Vec::new() })
}

// ---------------------------------------------------------------- the SAT encoder

pub struct Enc {
    pub s: rustsat_cadical::CaDiCaL<'static, 'static>,
    n: u32,
    tru: Lit,
    pub clauses: usize,
    /// Diagnosis: every clause added while set also gets these literals negated (a group selector).
    pub guard: Vec<Lit>,
}

impl Enc {
    pub fn new() -> Enc {
        let mut e = Enc { s: rustsat_cadical::CaDiCaL::default(), n: 1, tru: Lit::positive(0), clauses: 0, guard: Vec::new() };
        let t = e.tru;
        e.s.add_unit(t).unwrap();
        e
    }
    pub fn var(&mut self) -> Lit {
        let l = Lit::positive(self.n);
        self.n += 1;
        l
    }
    pub fn vars(&self) -> u32 {
        self.n
    }
    pub fn t(&self) -> Lit {
        self.tru
    }
    pub fn f(&self) -> Lit {
        !self.tru
    }
    pub fn konst(&self, b: bool) -> Lit {
        if b { self.tru } else { !self.tru }
    }
    pub fn add(&mut self, c: &[Lit]) {
        if c.iter().any(|&l| l == self.tru) {
            return;
        }
        let v: Clause = c.iter().copied().filter(|&l| l != !self.tru).chain(self.guard.iter().map(|g| !*g)).collect();
        self.s.add_clause(v).unwrap();
        self.clauses += 1;
    }
    pub fn and(&mut self, xs: &[Lit]) -> Lit {
        let mut v: Vec<Lit> = Vec::new();
        for &x in xs {
            if x == !self.tru {
                return !self.tru;
            }
            if x != self.tru && !v.contains(&x) {
                if v.contains(&!x) {
                    return !self.tru;
                }
                v.push(x);
            }
        }
        match v.len() {
            0 => self.tru,
            1 => v[0],
            _ => {
                let o = self.var();
                for &x in &v {
                    self.add(&[!o, x]);
                }
                let mut c: Vec<Lit> = v.iter().map(|&x| !x).collect();
                c.push(o);
                self.add(&c);
                o
            }
        }
    }
    pub fn or(&mut self, xs: &[Lit]) -> Lit {
        let n: Vec<Lit> = xs.iter().map(|&x| !x).collect();
        !self.and(&n)
    }
    pub fn xor(&mut self, a: Lit, b: Lit) -> Lit {
        if a == self.tru {
            return !b;
        }
        if a == !self.tru {
            return b;
        }
        if b == self.tru {
            return !a;
        }
        if b == !self.tru {
            return a;
        }
        if a == b {
            return !self.tru;
        }
        if a == !b {
            return self.tru;
        }
        let o = self.var();
        self.add(&[!o, a, b]);
        self.add(&[!o, !a, !b]);
        self.add(&[o, !a, b]);
        self.add(&[o, a, !b]);
        o
    }
    /// g -> (a <-> b)
    pub fn eq_if(&mut self, g: Lit, a: Lit, b: Lit) {
        self.add(&[!g, !a, b]);
        self.add(&[!g, a, !b]);
    }
    pub fn eq(&mut self, a: Lit, b: Lit) {
        let t = self.tru;
        self.eq_if(t, a, b);
    }
    pub fn exactly_one(&mut self, xs: &[Lit]) {
        self.add(xs);
        for i in 0..xs.len() {
            for j in i + 1..xs.len() {
                self.add(&[!xs[i], !xs[j]]);
            }
        }
    }
    pub fn val(&self, l: Lit) -> bool {
        if l == self.tru {
            return true;
        }
        if l == !self.tru {
            return false;
        }
        matches!(self.s.lit_val(l), Ok(TernaryVal::True))
    }
    /// Totalizer outputs: o[j] true when at least j+1 inputs are true (upper-bound direction).
    pub fn totalizer(&mut self, xs: &[Lit]) -> Vec<Lit> {
        if xs.len() <= 1 {
            return xs.to_vec();
        }
        let (a, b) = xs.split_at(xs.len() / 2);
        let a = self.totalizer(a);
        let b = self.totalizer(b);
        let r: Vec<Lit> = (0..a.len() + b.len()).map(|_| self.var()).collect();
        for i in 0..=a.len() {
            for j in 0..=b.len() {
                if i + j == 0 {
                    continue;
                }
                let mut c = vec![r[i + j - 1]];
                if i > 0 {
                    c.push(!a[i - 1]);
                }
                if j > 0 {
                    c.push(!b[j - 1]);
                }
                self.add(&c);
            }
        }
        r
    }
}

impl Default for Enc {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------- the problem

#[derive(Clone, Debug)]
pub struct LeverCand {
    pub pos: Pos,
    pub att: Pos,
    pub state: String,
}

pub struct Prob {
    pub rec: Rec,
    pub z: i32,
    /// box in the plane: x0, y0, x1, y1
    pub bx: (i32, i32, i32, i32),
    pub free: Vec<C2>,
    pub kinds: HashMap<C2, Vec<Kind>>,
    pub mech: HashSet<C2>,
    pub static_sturdy: HashSet<C2>,
    pub sturdy_up: HashSet<C2>,
    pub levers: Vec<LeverCand>,
    /// Most stalls (recorded static ticks replayed) per trace before any tick.
    pub dmax: i32,
    /// Most recorded static ticks skipped per trace.
    pub skip: i32,
    pub h: i32,
    pub qc: bool,
    pub levels: u8,
    /// Power of a retracted piston that does not move anyway is free (see build).
    pub blocked_dc: bool,
    /// Piston requirements from the mechanism's motion (default) instead of the recorded power.
    pub motion: bool,
    /// cells whose wiring placement is fixed (`--check`)
    pub fixed: HashMap<C2, Kind>,
    pub fixed_lever: Option<usize>,
    /// Only the fixed cells hold wiring (`--check`); else (`--fixed`) the other cells are free.
    pub fixed_only: bool,
    /// The rest state of each fixed cell (its design state): pinned in the model.
    pub fixed_state: HashMap<C2, String>,
    /// The lever is ON at rest (it opens when switched off).
    pub lever_on_rest: bool,
    /// `--keep-ideal`: the ideal plan, to judge candidates with its kept inputs.
    pub plan: Option<crate::ideal::Schedule>,
    /// Independent-timing clusters of mechanism cells (`--one-warp` turns them off): each has
    /// its own time warp, so events of different clusters can be split or shifted apart.
    pub cluster: HashMap<C2, usize>,
    pub ncl: usize,
    /// (trace, cell) -> the alternative group (`rec.groups`) its frames come from.
    pub cell_group: HashMap<(usize, C2), usize>,
}

impl Prob {
    pub fn is_free(&self, c: C2) -> bool {
        self.kinds.contains_key(&c)
    }
    fn can(&self, c: C2, f: impl Fn(Kind) -> bool) -> bool {
        self.kinds.get(&c).is_some_and(|ks| ks.iter().any(|k| f(*k)))
    }
    /// Frame used for the combinational state of a SAT tick that plays recorded tick `tau`.
    fn mk_tau(&self, tr: usize, tau: i32, c: C2) -> MK {
        self.rec.phases[tr].frame(tau - 1).get(&c).map_or(MK::Air, |s| mk(s))
    }
    /// The recordings cell c of trace tr reads: the base (None), or each alternative (group,
    /// alternative) of the group holding it.
    pub fn frame_opts(&self, tr: usize, c: C2) -> Vec<(Option<(usize, usize)>, &Phase)> {
        match self.cell_group.get(&(tr, c)) {
            Some(&g) => self.rec.groups[g].alts.iter().enumerate().map(|(a, x)| (Some((g, a)), &x.1)).collect(),
            None => vec![(None, &self.rec.phases[tr])],
        }
    }
    /// Recorded tick tau changes nothing (mechanism and piston power as the tick before), in the
    /// base recording and in every alternative.
    pub fn static_tick(&self, tr: usize, tau: i32) -> bool {
        let ph = &self.rec.phases[tr];
        tau <= 0 || tau > ph.k() || self.rec.trace_phases(tr).iter().all(|ph| ph.frame(tau) == ph.frame(tau - 1) && ph.pow(tau) == ph.pow(tau - 1))
    }
    /// The independent-timing cluster of a mechanism cell (0: static cells, or clusters off).
    pub fn cl(&self, c: C2) -> usize {
        self.cluster.get(&c).copied().unwrap_or(0)
    }
    /// Recorded tick tau changes nothing in cluster `cl` (its cells' frames and powers), in the base
    /// recording and in every alternative.
    pub fn static_tick_c(&self, tr: usize, tau: i32, cl: usize) -> bool {
        if self.ncl <= 1 {
            return self.static_tick(tr, tau);
        }
        let ph0 = &self.rec.phases[tr];
        if tau <= 0 || tau > ph0.k() {
            return true;
        }
        self.rec.trace_phases(tr).iter().all(|ph| {
            let (a, b) = (ph.frame(tau - 1), ph.frame(tau));
            let (pa, pb) = (ph.pow(tau - 1), ph.pow(tau));
            let cells: HashSet<C2> = a.keys().chain(b.keys()).chain(pa.keys()).chain(pb.keys()).copied().filter(|c| self.cl(*c) == cl).collect();
            cells.iter().all(|c| a.get(c) == b.get(c) && pa.get(c).copied().unwrap_or(false) == pb.get(c).copied().unwrap_or(false))
        })
    }
    pub fn settled_from_c(&self, tr: usize, cl: usize) -> i32 {
        let k = self.rec.phases[tr].k();
        let mut t = k;
        while t >= 1 && self.static_tick_c(tr, t, cl) {
            t -= 1;
        }
        t + 1
    }
    /// First recorded tick from which everything stays static.
    pub fn settled_from(&self, tr: usize) -> i32 {
        let k = self.rec.phases[tr].k();
        let mut t = k;
        while t >= 1 && self.static_tick(tr, t) {
            t -= 1;
        }
        t + 1
    }
    /// The piston at c in recorded frame tau-1 must be powered (Some(true)), unpowered
    /// (Some(false)), or either (None), from its MOTION alone (not the recorded power): it starts
    /// extending at tau -> on; starts retracting -> off; stays extended -> on (the 2nd tick of a
    /// <= 2 gt pulse is free: 1 gt and 2 gt pulses both spit); stays retracted -> off if it could
    /// extend (its push line ends in air, a wiring cell, or the box edge), free if blocked.
    pub fn motion_req(&self, tr: usize, tau: i32, c: C2) -> Option<bool> {
        self.motion_req_ph(&self.rec.phases[tr], tau, c)
    }
    pub fn motion_req_ph(&self, ph: &Phase, tau: i32, c: C2) -> Option<bool> {

        let st = |j: i32| ph.frame(j).get(&c).map_or(MK::Air, |s| mk(s));
        let (before, after) = (st(tau - 1), st(tau));
        let changed = ph.frame(tau - 1).get(&c) != ph.frame(tau).get(&c) && tau >= 1;
        match before {
            MK::Piston { ext: false, f, .. } => {
                if changed {
                    // its own extension (the base turns extended) needs power; being pushed or
                    // pulled away by another piston does not care
                    return if matches!(after, MK::Piston { ext: true, f: g, .. } if g == f) { Some(true) } else { None };
                } else if self.can_extend(ph.frame(tau - 1), c, f) && self.can_extend(ph.frame(tau), c, f) {
                    // (blocked in frame tau too: a piston in front extended in this same tick,
                    // its block event ran first)
                    Some(false)
                } else {
                    None
                }
            }
            MK::Piston { ext: true, .. } => {
                if changed {
                    let _ = after;
                    // a 1 gt pulse (extension started in the previous tick) may as well be 2 gt:
                    // both spit; the retraction then starts one tick later
                    let one_gt = tau >= 2 && ph.frame(tau - 2).get(&c) != ph.frame(tau - 1).get(&c);
                    // the end of a long pulse (extended >= 3 gt: the retraction pulls) may come one tick
                    // later (redstone pulse widths are even: a 3 gt pulse is a 4 gt one)
                    let long = tau >= 4 && (2..=4).all(|j| ph.frame(tau - j).get(&c) == ph.frame(tau - 1).get(&c));
                    // (only for ideal plans: a wired recording's timing is the wiring's own, exact)
                    let ideal = self.rec.full.is_empty();
                    if (one_gt || long) && ideal { None } else { Some(false) }
                } else {
                    let started = tau >= 2 && ph.frame(tau - 2).get(&c) != ph.frame(tau - 1).get(&c);
                    let ends = ph.frame(tau + 1).get(&c) != ph.frame(tau).get(&c);
                    if started && ends && self.rec.full.is_empty() { None } else { Some(true) }
                }
            }
            MK::Other if tau >= 2 => {
                // the retraction that started one tick ago (the base is a moving piston now): a
                // 1 gt or a long pulse may end one tick late, but not later: off from here
                let b2 = ph.frame(tau - 2).get(&c).map_or(MK::Air, |s| mk(s));
                let moving = ph.frame(tau - 1).get(&c).is_some_and(|s| short(s) == "moving_piston");
                if moving && matches!(b2, MK::Piston { ext: true, .. }) { Some(false) } else { None }
            }
            _ => None,
        }
    }
    /// The facing of the piston whose power matters at recorded tick tau: the piston in frame
    /// tau-1, or the extended piston that started retracting at tau-1 (now a moving piston).
    pub fn req_facing(&self, ph: &Phase, tau: i32, c: C2) -> Option<C2> {
        match ph.frame(tau - 1).get(&c).map(|s| mk(s)) {
            Some(MK::Piston { f, .. }) => Some(f),
            _ if tau >= 2 && ph.frame(tau - 1).get(&c).is_some_and(|s| short(s) == "moving_piston") => match ph.frame(tau - 2).get(&c).map(|s| mk(s)) {
                Some(MK::Piston { f, ext: true, .. }) if self.motion => Some(f),
                _ => None,
            },
            _ => None,
        }
    }
    pub fn can_extend(&self, fr: &HashMap<C2, String>, c: C2, f: C2) -> bool {
        self.extend_room(fr, c, f, false)
    }
    /// The piston at c certainly has room to extend in frame fr (every cell of its line is known:
    /// no other timing cluster, no wiring cell the solver still chooses).
    pub fn can_extend_sure(&self, fr: &HashMap<C2, String>, c: C2, f: C2) -> bool {
        self.extend_room(fr, c, f, true)
    }
    fn extend_room(&self, fr: &HashMap<C2, String>, c: C2, f: C2, sure: bool) -> bool {
        let mut x = c;
        for _ in 0..13 {
            x = add(x, f);
            // a cell of another timing cluster: its state at this recorded tick is not its state
            // at this SAT tick: count it as room (the safe side: power stays off)
            if self.ncl > 1 && self.cl(x) != 0 && self.cl(x) != self.cl(c) {
                return !sure;
            }
            match fr.get(&x) {
                // a wiring cell: kept wiring is known (a solid is pushed, obsidian and droppers
                // hold, components break); a cell the solver fills is unknown
                None => match self.fixed.get(&x) {
                    Some(Kind::Block | Kind::Note | Kind::Lamp | Kind::Bulb | Kind::Glass | Kind::Obs(_) | Kind::RBlock | Kind::Leaves) => continue,
                    Some(Kind::Obsid | Kind::Dropper) => return false,
                    Some(Kind::Dust | Kind::Dot | Kind::Rep(..) | Kind::Torch | Kind::WTorch(_)) => return true,
                    Some(_) => return !sure,
                    None => return !(sure && self.kinds.contains_key(&x)),
                },
                Some(s) => match mk(s) {
                    MK::Air => return true,
                    MK::Cond | MK::Solid | MK::RBlock if !short(s).contains("obsidian") => continue,
                    MK::Piston { ext: false, .. } => continue,
                    _ => return false,
                },
            }
        }
        false
    }
    /// Recorded ticks SAT tick k may play.
    pub fn window(&self, tr: usize, k: i32) -> std::ops::RangeInclusive<i32> {
        let kk = self.rec.phases[tr].k();
        if k <= 0 { 0..=0 } else { (k - self.dmax).max(0).min(kk)..=(k + self.skip).min(kk) }
    }
}

pub struct ProbOpts {
    pub bx: Option<(i32, i32, i32, i32)>,
    pub palette: Vec<Kind>,
    pub forbid_ring: bool,
    pub full_frame: bool,
    /// Ring cells nothing ever passes through hold smooth stone (what `door properties-dyn`'s
    /// hallway check puts there): the wiring is hallway-compatible by construction.
    pub hall_ring: bool,
    pub levers: Option<Vec<Pos>>,
    pub lever_oop: bool,
    pub dmax: i32,
    pub skip: i32,
    pub settle: i32,
    pub qc: bool,
    pub levels: u8,
    pub blocked_dc: bool,
    pub motion: bool,
    pub clusters: bool,
}

/// Free cells, palettes per cell, lever candidates.
pub fn build_prob(rec: Rec, o: &ProbOpts) -> Result<Prob> {
    let mut rec = rec;
    // blocks whose redstone behavior is not modeled: refuse rather than treat them as conductors
    for ph in &rec.phases {
        for fr in &ph.frames {
            if let Some((c, s)) = fr.iter().find(|(_, s)| unsupported(short(s))) {
                bail!("satwire does not model {} (mechanism cell {c:?})", short(s));
            }
        }
    }
    // a wired source's unmodeled wiring (a sculk sensor bridge, a target block): the synthesis
    // re-wires the recorded piston power without it (`--check` / `--hint` then miss those cells)
    let unmodeled: Vec<(C2, String)> = rec.wiring.iter().filter(|(_, s)| unsupported(short(s))).map(|(c, s)| (*c, short(s).to_string())).collect();
    if !unmodeled.is_empty() {
        println!("unmodeled wiring dropped (its cells become free cells): {unmodeled:?}");
        rec.wiring.retain(|_, s| !unsupported(short(s)));
    }
    let z = rec.z;
    let (dx, dy) = (rec.door[0], rec.door[1]);
    let mut mech: HashSet<C2> = HashSet::new();
    let mut stat: HashMap<C2, Option<String>> = HashMap::new();
    for ph in rec.all_phases() {
        for f in &ph.frames {
            for (c, _s) in f {
                mech.insert(*c);
            }
        }
    }
    for c in &mech {
        let mut v: Option<String> = None;
        let mut ok = true;
        for ph in rec.all_phases() {
            for f in &ph.frames {
                match (f.get(c), &v) {
                    (None, _) => ok = false,
                    (Some(s), None) => v = Some(s.clone()),
                    (Some(s), Some(t)) => {
                        if s != t {
                            ok = false
                        }
                    }
                }
            }
        }
        stat.insert(*c, if ok { v } else { None });
    }
    let static_sturdy: HashSet<C2> = stat
        .iter()
        .filter_map(|(c, v)| v.as_ref().and_then(|s| matches!(mk(s), MK::Cond | MK::Solid | MK::Piston { ext: false, .. }).then_some(*c)))
        .collect();
    // top face sturdy at every recorded tick (a retracted piston, or an extended one facing down:
    // its back is its top): dust, repeaters, comparators and torches can stand on it
    let sturdy_up: HashSet<C2> = mech
        .iter()
        .copied()
        .filter(|c| {
            rec.all_phases().iter().all(|ph| {
                ph.frames.iter().all(|f| match f.get(c).map(|s| mk(s)) {
                    Some(MK::Cond | MK::Solid | MK::Piston { ext: false, .. }) => true,
                    Some(MK::Piston { ext: true, f: fc, .. }) => fc == DN,
                    // a retracting base is a moving piston for 2 gt (placed without shape
                    // updates: what stands on it stays)
                    Some(MK::Other) => rec.all_phases().iter().all(|p| p.frames[0].get(c).is_some_and(|s| matches!(mk(s), MK::Piston { .. }))),
                    _ => false,
                })
            })
        })
        .collect();
    let bx = match o.bx {
        Some(b) => b,
        None if rec.src_box.is_some() => rec.src_box.unwrap(),
        None => {
            let (mut x0, mut y0, mut x1, mut y1) = (dx, dy, dx + 2, dy + 2);
            for c in &mech {
                x0 = x0.min(c.0);
                y0 = y0.min(c.1);
                x1 = x1.max(c.0);
                y1 = y1.max(c.1);
            }
            (x0, y0, x1, y1)
        }
    };
    let in_t = |c: C2| c.0 >= dx && c.0 <= dx + 2 && c.1 >= dy && c.1 <= dy + 2;
    let ring: HashSet<C2> = (0..3)
        .flat_map(|i| [(dx + i, dy - 1), (dx + i, dy + 3), (dx - 1, dy + i), (dx + 3, dy + i)])
        .collect();
    // hallway holes: ring cells some block passes through that are air in the OPEN state; the
    // cell right behind (outward) may only hold a plain block. (Closed, a flat door's ring is
    // hidden behind the door blocks: `properties::seamless` skips depth-0 holes at rest, so a
    // park cell that is air only at rest leaves the cell behind it free.)
    let mut hole_back: HashSet<C2> = HashSet::new();
    for r in &ring {
        let swept = rec.all_phases().iter().any(|ph| ph.frames.iter().any(|f| f.get(r).is_some_and(|s| mk(s) != MK::Air)));
        let air_open = rec.phases[1].frames.first().is_some_and(|f| f.get(r).is_none_or(|s| mk(s) == MK::Air));
        if swept && air_open {
            let out = if r.1 == dy - 1 {
                (r.0, r.1 - 1)
            } else if r.1 == dy + 3 {
                (r.0, r.1 + 1)
            } else if r.0 == dx - 1 {
                (r.0 - 1, r.1)
            } else {
                (r.0 + 1, r.1)
            };
            hole_back.insert(out);
        }
    }
    let mut kinds: HashMap<C2, Vec<Kind>> = HashMap::new();
    let mut hall_fixed: Vec<C2> = Vec::new();
    let mut free = Vec::new();
    for y in bx.1..=bx.3 {
        for x in bx.0..=bx.2 {
            let c = (x, y);
            if mech.contains(&c) || in_t(c) {
                continue;
            }
            let plain_only = o.forbid_ring && (ring.contains(&c) || hole_back.contains(&c));
            let ks: Vec<Kind> = if o.hall_ring && ring.contains(&c) {
                hall_fixed.push(c);
                vec![Kind::Empty, Kind::Block]
            } else {
                o.palette.iter().copied().filter(|k| !plain_only || matches!(k, Kind::Empty | Kind::Block)).collect()
            };
            kinds.insert(c, ks);
            free.push(c);
        }
    }
    // --full-frame (vjeux, 03:45Z): open, the 12 frame cells around the opening hold plain blocks:
    // a door-type block of the mechanism, or static stone placed here (a free frame cell is forced
    // to a block); a frame cell the mechanism leaves empty or holds a piston/head in at open makes
    // the mechanism ineligible
    if o.full_frame {
        // the open stable state: the one whose 3x3 is empty (a door may rest open)
        let empty3 = |fr: &HashMap<C2, String>| (0..3).all(|i| (0..3).all(|j| fr.get(&(dx + i, dy + j)).is_none_or(|s| mk(s) == MK::Air)));
        let open = if empty3(&rec.phases[0].frames[0]) { rec.phases[0].frames[0].clone() } else { rec.phases[1].frames[0].clone() };
        let mut ring_v: Vec<C2> = ring.iter().copied().collect();
        ring_v.sort();
        for r in ring_v {
            let st = open.get(&r).map(|s| mk(s));
            if matches!(st, Some(MK::Cond)) {
                continue;
            }
            if let Some(ks) = kinds.get_mut(&r) {
                if mech.contains(&r) {
                    bail!("--full-frame: frame cell {r:?} is swept by the mechanism and not a block at open");
                }
                ks.retain(|k| *k == Kind::Block);
                if ks.is_empty() {
                    bail!("--full-frame: frame cell {r:?} cannot hold a block");
                }
            } else {
                bail!("--full-frame: frame cell {r:?} holds {:?} at open (not a plain block)", open.get(&r).map(|s| short(s).to_string()));
            }
        }
    }
    // sticky pistons never pull wiring: the cell two ahead of any sticky piston holds nothing movable
    for ph in rec.all_phases() {
        for f in &ph.frames {
            for (c, s) in f {
                if let MK::Piston { f: fc, sticky: true, .. } = mk(s) {
                    let x = add(add(*c, fc), fc);
                    if let Some(ks) = kinds.get_mut(&x) {
                        ks.retain(|k| !k.movable());
                    }
                }
            }
        }
    }
    // lever candidates
    let mut levers = Vec::new();
    let sturdy_possible = |c: C2| static_sturdy.contains(&c) || sturdy_up.contains(&c) || kinds.get(&c).is_some_and(|ks| ks.iter().any(|k| k.sturdy()));
    let lever_state = |face: &str, facing: &str| format!("minecraft:lever[face={face},facing={facing},powered=false]");
    let mut cand_cells: Vec<C2> = Vec::new();
    match &o.levers {
        Some(v) => {
            for p in v {
                if p[2] == z {
                    cand_cells.push((p[0], p[1]));
                } else {
                    let att = [p[0], p[1], z];
                    let facing = if p[2] < z { "north" } else { "south" };
                    if sturdy_possible((p[0], p[1])) {
                        levers.push(LeverCand { pos: *p, att, state: lever_state("wall", facing) });
                    }
                }
            }
        }
        None => {
            for y in bx.1 - 1..=bx.3 + 1 {
                for x in bx.0 - 1..=bx.2 + 1 {
                    let c = (x, y);
                    let inside = x >= bx.0 && x <= bx.2 && y >= bx.1 && y <= bx.3;
                    if (inside && kinds.contains_key(&c)) || (!inside && !mech.contains(&c) && !in_t(c)) {
                        cand_cells.push(c);
                    }
                }
            }
            if o.lever_oop {
                for c in &free {
                    if sturdy_possible(*c) {
                        for (dz, facing) in [(-1, "north"), (1, "south")] {
                            levers.push(LeverCand { pos: [c.0, c.1, z + dz], att: [c.0, c.1, z], state: lever_state("wall", facing) });
                        }
                    }
                }
            }
        }
    }
    for c in cand_cells {
        if mech.contains(&c) || in_t(c) {
            continue;
        }
        if kinds.contains_key(&c) && !kinds[&c].contains(&Kind::Empty) {
            continue;
        }
        for d in DIRS {
            let a = add(c, d);
            if !sturdy_possible(a) {
                continue;
            }
            let st = match d {
                DN => lever_state("floor", "north"),
                U => lever_state("ceiling", "north"),
                E => lever_state("wall", "west"),
                _ => lever_state("wall", "east"),
            };
            levers.push(LeverCand { pos: [c.0, c.1, z], att: [a.0, a.1, z], state: st });
        }
    }
    if levers.is_empty() {
        bail!("no lever candidate");
    }
    let kmax = rec.phases.iter().map(|p| p.k()).max().unwrap_or(0);
    let h = kmax + o.dmax + o.settle;
    // independent-timing clusters: the cells moved together by one piston motion (its base and the
    // consecutive cells of its line that change within 3 gt), merged across motions
    let mut parent: HashMap<C2, C2> = HashMap::new();
    fn find(p: &mut HashMap<C2, C2>, c: C2) -> C2 {
        let mut r = c;
        while let Some(&q) = p.get(&r) {
            if q == r {
                break;
            }
            r = q;
        }
        p.insert(c, r);
        r
    }
    if o.clusters {
        for ph in rec.phases.iter() {
            let ch = |c: C2, t: i32| ph.frame(t - 1).get(&c) != ph.frame(t).get(&c);
            for t in 1..=ph.k() {
                let (a, b) = (ph.frame(t - 1), ph.frame(t));
                let cells: Vec<C2> = a.keys().chain(b.keys()).copied().filter(|c| a.get(c) != b.get(c)).collect();
                for pc in cells {
                    parent.entry(pc).or_insert(pc);
                    // the cells of this piston's OWN motion: an extension moves the blocks in
                    // front of it up to the first air cell (frame before); a retraction moves its
                    // head and, if sticky, the block in front of the head
                    let movable = |s: Option<&String>| s.is_some_and(|s| matches!(mk(s), MK::Cond | MK::Solid | MK::Piston { ext: false, .. }) && short(s) != "obsidian");
                    let mut line: Vec<C2> = Vec::new();
                    match (a.get(&pc).map(|s| mk(s)), b.get(&pc).map(|s| mk(s))) {
                        (Some(MK::Piston { ext: false, f, .. }), Some(MK::Piston { ext: true, .. })) => {
                            let mut x = add(pc, f);
                            for _ in 0..13 {
                                line.push(x);
                                if !movable(a.get(&x)) {
                                    break;
                                }
                                x = add(x, f);
                            }
                        }
                        (Some(MK::Piston { ext: true, f, sticky }), _) => {
                            line.push(add(pc, f));
                            if sticky {
                                line.push(add(add(pc, f), f));
                            }
                        }
                        _ => continue,
                    }
                    for x in line {
                        if (t..=t + 3).any(|j| ch(x, j)) {
                            parent.entry(x).or_insert(x);
                            let (ra, rb) = (find(&mut parent, pc), find(&mut parent, x));
                            if ra != rb {
                                parent.insert(ra, rb);
                            }
                        }
                    }
                }
            }
        }
    }
    let keys: Vec<C2> = parent.keys().copied().collect();
    let mut roots: BTreeMap<C2, usize> = BTreeMap::new();
    let mut cluster: HashMap<C2, usize> = HashMap::new();
    for c in keys {
        let r = find(&mut parent, c);
        let n = roots.len() + 1;
        let id = *roots.entry(r).or_insert(n);
        cluster.insert(c, id);
    }
    let ncl = roots.len() + 1;
    if ncl > 2 {
        let mut sizes: BTreeMap<usize, Vec<C2>> = BTreeMap::new();
        for (c, id) in &cluster {
            sizes.entry(*id).or_default().push(*c);
        }
        let desc: Vec<String> = sizes.iter().map(|(id, v)| { let mut v = v.clone(); v.sort(); format!("c{id}: {}", v.iter().map(|c| format!("{},{}", c.0, c.1)).collect::<Vec<_>>().join(" ")) }).collect();
        println!("{} independent-timing clusters: {}", ncl - 1, desc.join(" | "));
    }
    let fixed: HashMap<C2, Kind> = hall_fixed.into_iter().map(|c| (c, Kind::Block)).collect();
    let mut cell_group: HashMap<(usize, C2), usize> = HashMap::new();
    for (gi, g) in rec.groups.iter().enumerate() {
        for c in &g.cells {
            cell_group.insert((g.tr, *c), gi);
        }
    }
    let lever_on_rest = rec.lever.as_ref().is_some_and(|(_, s)| prop(s, "powered") == Some("true")) || LEVER_ON.load(std::sync::atomic::Ordering::Relaxed);
    Ok(Prob { rec, z, bx, free, kinds, mech, static_sturdy, sturdy_up, levers, dmax: o.dmax, skip: o.skip, h, qc: o.qc, levels: o.levels, blocked_dc: o.blocked_dc, motion: o.motion, fixed, fixed_lever: None, fixed_only: false, fixed_state: HashMap::new(), lever_on_rest, plan: None, cell_group, cluster, ncl })
}

/// `--menu`: record each alternative schedule, keep the trace `tr` of those that start and end
/// like the base recording, and turn them into alternative groups (the cells where any
/// alternative's frames or piston power differ from the base). A group whose cells meet an earlier
/// group's (same trace) is dropped (its alternatives do not compose). Every phase of a trace is
/// then padded to one length. Returns one log line per group.
pub fn add_menu(w: &mut Worker, rec: &mut Rec, items: &[(String, usize, String, crate::ideal::Schedule)], ticks: u32) -> Result<Vec<String>> {
    let mut log = Vec::new();
    let mut by: BTreeMap<(usize, String), Vec<(String, Phase)>> = BTreeMap::new();
    for (g, tr, label, s) in items {
        let r = match record_ideal(w, s, ticks) {
            Ok(r) => r,
            Err(e) => {
                log.push(format!("menu {g} {label}: recording failed: {e:#}"));
                continue;
            }
        };
        let ph = r.phases[*tr].clone();
        let base = &rec.phases[*tr];
        if ph.frames.first() != base.frames.first() || ph.frames.last() != base.frames.last() {
            continue;
        }
        by.entry((*tr, g.clone())).or_default().push((label.clone(), ph));
    }
    let mut taken: [BTreeSet<C2>; 2] = Default::default();
    for ((tr, g), mut alts) in by {
        let n = alts.iter().map(|a| a.1.frames.len()).max().unwrap_or(0).max(rec.phases[tr].frames.len());
        let mut base = rec.phases[tr].clone();
        base.pad(n);
        for a in alts.iter_mut() {
            a.1.pad(n);
        }
        let mut cells: BTreeSet<C2> = BTreeSet::new();
        for (_, ph) in &alts {
            for j in 0..n {
                let (fa, fb) = (&ph.frames[j], &base.frames[j]);
                for c in fa.keys().chain(fb.keys()) {
                    if fa.get(c) != fb.get(c) {
                        cells.insert(*c);
                    }
                }
                let (pa, pb) = (&ph.power[j], &base.power[j]);
                for c in pa.keys().chain(pb.keys()) {
                    if pa.get(c).copied().unwrap_or(false) != pb.get(c).copied().unwrap_or(false) {
                        cells.insert(*c);
                    }
                }
            }
        }
        if cells.is_empty() {
            continue;
        }
        if !cells.is_disjoint(&taken[tr]) {
            log.push(format!("menu {g} ({}): dropped, its cells meet an earlier group's", ["open", "close"][tr]));
            continue;
        }
        taken[tr].extend(cells.iter().copied());
        let labels: Vec<String> = alts.iter().map(|a| a.0.clone()).collect();
        log.push(format!("menu {g} ({}): {} alternatives ({}), cells {:?}", ["open", "close"][tr], alts.len() + 1, labels.join(" "), cells));
        let mut all = vec![("as planned".to_string(), base)];
        all.extend(alts);
        rec.groups.push(AltGroup { name: g, tr, cells, alts: all });
    }
    for tr in 0..2 {
        let n = rec.trace_phases(tr).iter().map(|p| p.frames.len()).max().unwrap_or(0);
        rec.phases[tr].pad(n);
        for g in rec.groups.iter_mut().filter(|g| g.tr == tr) {
            for a in g.alts.iter_mut() {
                a.1.pad(n);
            }
        }
    }
    Ok(log)
}

// ---------------------------------------------------------------- the model

const P_COND: u8 = 0;
const P_DUST: u8 = 1;
const P_STURDY: u8 = 2;
const P_OUT: u8 = 3;
const P_OBS: u8 = 4;
const P_REP: u8 = 5;
const P_TORCH: u8 = 6;
const P_CMP: u8 = 7;
const P_NOTE: u8 = 8;
const P_BULB: u8 = 9;
const P_MOVABLE: u8 = 10;
const P_LAMP: u8 = 11;
const P_IND: u8 = 12;
const P_DROP: u8 = 13;
const P_LEAVES: u8 = 14;
const P_UPISTON: u8 = 15;

fn pred_fn(t: u8) -> fn(Kind) -> bool {
    match t {
        P_COND => |k| k.cond(),
        P_DUST => |k| matches!(k, Kind::Dust | Kind::Dot),
        P_STURDY => |k| k.sturdy(),
        P_OUT => |k| k.has_out(),
        P_OBS => |k| matches!(k, Kind::Obs(_)),
        P_REP => |k| matches!(k, Kind::Rep(..)),
        P_TORCH => |k| matches!(k, Kind::Torch | Kind::WTorch(_)),
        P_CMP => |k| matches!(k, Kind::Cmp(_)),
        P_NOTE => |k| k == Kind::Note,
        P_BULB => |k| k == Kind::Bulb,
        P_LAMP => |k| k == Kind::Lamp,
        P_IND => |k| matches!(k, Kind::Note | Kind::Trap | Kind::TrapTop | Kind::Dropper),
        P_DROP => |k| k == Kind::Dropper,
        P_LEAVES => |k| k == Kind::Leaves,
        P_UPISTON => |k| matches!(k, Kind::UPiston(_)),
        _ => |k| k.movable(),
    }
}

type Key = (usize, i32, C2);

/// A recorded piston check (for `--check`): trace, tick, cell, offset, required, direct, quasi.
pub struct PCheck {
    pub tr: usize,
    pub k: i32,
    pub c: C2,
    pub d: i32,
    pub req: bool,
    pub direct: Lit,
    pub qc: Lit,
    /// --sound: a QC hold is allowed here unless this update literal is true.
    pub bud: Option<Lit>,
}

pub struct Model<'a> {
    pub p: &'a Prob,
    pub e: Enc,
    pub kl: HashMap<(C2, Kind), Lit>,
    preds: HashMap<(C2, u8), Lit>,
    pub lever: Vec<Lit>,
    pub warp: Vec<Vec<Vec<Vec<(i32, Lit)>>>>,
    /// Number of traces (2; 4 with `--period2`): trace tr plays phase tr % 2.
    pub nt: usize,
    out: HashMap<Key, Lit>,
    sched: HashMap<Key, Lit>,
    /// Observers: the tick due at s was scheduled only by a block-event-phase change (a moving
    /// mechanism block or the lever flip): it runs after every tick scheduled in phase S.
    sched_e: HashMap<Key, Lit>,
    causes_e: HashMap<Key, Vec<Lit>>,
    /// Lamps: an off-on transient inside the tick (observers see it).
    ltrans: HashMap<Key, Lit>,
    lv: HashMap<(usize, i32, C2, u8), Lit>,
    st: HashMap<Key, Lit>,
    pw: HashMap<Key, Lit>,
    bpow: HashMap<Key, Lit>,
    blit: HashMap<Key, Lit>,
    npow: HashMap<Key, Lit>,
    /// Leaves: distance >= j (j = 2..7) at the end of each tick.
    leaves: HashMap<(usize, i32, C2, u8), Lit>,
    sigc: HashMap<(usize, i32, C2, C2, bool), Lit>,
    mc: HashMap<(usize, i32, C2, u8), Lit>,
    chgc: HashMap<(usize, i32, C2, u8), Lit>,
    causes: HashMap<Key, Vec<Lit>>,
    pub used: Vec<(C2, Lit)>,
    pub checks: Vec<PCheck>,
    pub check_mode: bool,
    /// Diagnosis selectors per constraint group (`--check`).
    pub gsels: Vec<(String, Lit)>,
    /// Selector per (trace, piston cell): assumed true; an UNSAT core names the pistons.
    pub sels: Vec<(usize, C2, Lit)>,
    /// One selector per alternative of each group (`rec.groups`), exactly one true per group.
    pub alt_sel: Vec<Vec<Lit>>,
    /// `--update-order`: (updating, non-updating) quasi-connected power per (trace, tick, piston).
    qcs: HashMap<Key, (Lit, Lit)>,
    /// `--explain` with `--update-order`: one selector per update-order clause (trace, tick, piston, kind).
    pub uo_sels: Vec<(usize, i32, C2, &'static str, Lit)>,
    /// Ordered pairs of touching events of different clusters (trace, cluster X, start a, cluster
    /// Y, start b, safe gap min(3, b - a)): `--retime` keeps only their order and learns gaps.
    pub pairs: Vec<(usize, usize, i32, usize, i32, i32)>,
    /// `--retime`: "b starts less than the safe gap after a" literal per pair index.
    close_lits: HashMap<usize, Lit>,
}

impl<'a> Model<'a> {
    fn lo(&self) -> u8 {
        16 - self.p.levels
    }
    fn pred(&mut self, c: C2, t: u8) -> Lit {
        if let Some(l) = self.preds.get(&(c, t)) {
            return *l;
        }
        let f = pred_fn(t);
        let lits: Vec<Lit> = match self.p.kinds.get(&c) {
            Some(ks) => ks.iter().filter(|k| f(**k)).map(|k| self.kl[&(c, *k)]).collect(),
            None => vec![],
        };
        let l = self.e.or(&lits);
        self.preds.insert((c, t), l);
        l
    }
    fn kind(&self, c: C2, k: Kind) -> Lit {
        self.kl.get(&(c, k)).copied().unwrap_or(self.e.f())
    }
    fn on(&self, tr: usize, k: i32) -> bool {
        // trace 0 = the first flip from rest; a lever ON at rest (`lever_on_rest`) flips off
        ((tr % 2 == 0) == (k >= 0)) != self.p.lever_on_rest
    }
    fn out(&self, tr: usize, k: i32, c: C2) -> Lit {
        self.out.get(&(tr, k, c)).copied().unwrap_or(self.e.f())
    }
    fn sched(&self, tr: usize, s: i32, c: C2) -> Lit {
        self.sched.get(&(tr, s, c)).copied().unwrap_or(self.e.f())
    }
    fn lvl(&self, tr: usize, k: i32, c: C2, j: u8) -> Lit {
        if j > 15 {
            return self.e.f();
        }
        self.lv.get(&(tr, k, c, j)).copied().unwrap_or(self.e.f())
    }
    fn dpow(&self, tr: usize, k: i32, c: C2) -> Lit {
        self.lvl(tr, k, c, self.lo())
    }
    fn get(m: &HashMap<Key, Lit>, key: Key, f: Lit) -> Lit {
        m.get(&key).copied().unwrap_or(f)
    }
    /// Mechanism property literal at SAT tick k over the warp (0 cond, 1..=4 piston facing E/W/U/D).
    fn mprop(&mut self, tr: usize, k: i32, c: C2, what: u8) -> Lit {
        if let Some(l) = self.mc.get(&(tr, k, c, what)) {
            return *l;
        }
        let ws = self.warp_at_c(tr, k, self.p.cl(c));
        let p = self.p;
        let opts = p.frame_opts(tr % 2, c);
        let mut lits = Vec::new();
        let mut every = opts.len() == 1;
        for (ga, ph) in &opts {
            for (tau, wl) in &ws {
                let m = ph.frame(*tau - 1).get(&c).map_or(MK::Air, |s| mk(s));
                let hit = match (what, m) {
                    (0, MK::Cond) => true,
                    (11, MK::RBlock) => true,
                    (1..=4, MK::Piston { f, .. }) => f == DIRS[(what - 1) as usize],
                    _ => false,
                };
                if hit {
                    match ga {
                        None => lits.push(*wl),
                        Some((g, a)) => {
                            let s = self.alt_sel[*g][*a];
                            lits.push(self.e.and(&[*wl, s]));
                        }
                    }
                } else {
                    every = false;
                }
            }
        }
        let l = if every { self.e.t() } else { self.e.or(&lits) };
        self.mc.insert((tr, k, c, what), l);
        l
    }
    /// Mechanism cell changes during SAT tick k (observers see it).
    /// The note-block instrument class of the mechanism cell c changes during SAT tick k (stone
    /// family: basedrum; logs: bass; glass: hat; anything else: harp).
    fn inst_chg(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        if let Some(l) = self.mc.get(&(tr, k, c, 10)) {
            return *l;
        }
        let class = |s: Option<&String>| -> u8 {
            match s.map(|s| short(s)) {
                Some(n) if n.ends_with("_log") || n.ends_with("_wood") => 2,
                Some(n) if n.contains("glass") => 3,
                Some(n) if matches!(mk(&format!("minecraft:{n}")), MK::Cond) && !n.contains("note_block") => 1,
                _ => 0,
            }
        };
        let logs = LOG_DOORS.load(std::sync::atomic::Ordering::Relaxed);
        let fix = |v: u8| if logs && v == 1 { 2 } else { v };
        let ws = self.warp_at_c(tr, k, self.p.cl(c));
        let ph = &self.p.rec.phases[tr % 2];
        let lits: Vec<Lit> = ws.iter().filter(|(tau, _)| *tau >= 1 && fix(class(ph.frame(*tau).get(&c))) != fix(class(ph.frame(*tau - 1).get(&c)))).map(|(_, l)| *l).collect();
        let l = self.e.or(&lits);
        self.mc.insert((tr, k, c, 10), l);
        l
    }
    fn mchg(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        if let Some(l) = self.mc.get(&(tr, k, c, 9)) {
            return *l;
        }
        let ws = self.warp_at_c(tr, k, self.p.cl(c));
        let p = self.p;
        let mut lits: Vec<Lit> = Vec::new();
        for (ga, ph) in p.frame_opts(tr % 2, c) {
            for (tau, wl) in &ws {
                if *tau >= 1 && ph.frame(*tau).get(&c) != ph.frame(*tau - 1).get(&c) {
                    match ga {
                        None => lits.push(*wl),
                        Some((g, a)) => {
                            let s = self.alt_sel[g][a];
                            lits.push(self.e.and(&[*wl, s]));
                        }
                    }
                }
            }
        }
        let l = self.e.or(&lits);
        self.mc.insert((tr, k, c, 9), l);
        l
    }
    /// The (recorded tick, literal) pairs SAT tick k may play.
    fn warp_at(&self, tr: usize, k: i32) -> Vec<(i32, Lit)> {
        self.warp_at_c(tr, k, 0)
    }
    fn warp_at_c(&self, tr: usize, k: i32, cl: usize) -> Vec<(i32, Lit)> {
        if k <= 0 { vec![(0, self.e.t())] } else { self.warp[tr][cl][k as usize].clone() }
    }
    /// `--keep-ideal`: the plan's own off-plane redstone block at cell c, kept as a given source
    /// (timed by the cluster `cl` of the piston it drives).
    fn given(&mut self, tr: usize, k: i32, c: C2, cl: usize) -> Lit {
        if !KEEP_IDEAL.lock().unwrap().contains(&c) {
            return self.e.f();
        }
        let ws = self.warp_at_c(tr, k, cl);
        let ph = &self.p.rec.phases[tr % 2];
        let lits: Vec<Lit> = ws.iter().filter(|(tau, _)| ph.oop_at(*tau, c)).map(|x| x.1).collect();
        self.e.or(&lits)
    }
    fn is_mech(&self, c: C2) -> bool {
        self.p.mech.contains(&c)
    }
    fn cond_at(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        if self.p.is_free(c) {
            self.pred(c, P_COND)
        } else if self.is_mech(c) {
            self.mprop(tr, k, c, 0)
        } else {
            self.e.f()
        }
    }
    /// A static block at c offers a sturdy face (`up`: its top face) at every tick.
    fn sturdy_static(&mut self, c: C2, up: bool) -> Lit {
        if self.p.is_free(c) {
            self.pred(c, P_STURDY)
        } else {
            self.e.konst(self.p.static_sturdy.contains(&c) || (up && self.p.sturdy_up.contains(&c)))
        }
    }
    fn lever_on_lits(&mut self, tr: usize, k: i32, pred: impl Fn(&LeverCand) -> bool) -> Lit {
        if !self.on(tr, k) {
            return self.e.f();
        }
        let lits: Vec<Lit> = self.p.levers.iter().enumerate().filter(|(_, l)| pred(l)).map(|(i, _)| self.lever[i]).collect();
        self.e.or(&lits)
    }
    /// Out-of-plane levers next to c (weak power into c).
    /// A block update reaches the piston at c during SAT tick k: one of its neighbors changes (a
    /// moving block, a wiring state), or a diode/observer whose output block is one of its
    /// neighbors changes (it updates that block's neighbors too).
    fn update_at(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        let mut t = Vec::new();
        for d in DIRS {
            let n = add(c, d);
            let a = self.chg(tr, k, n, 0);
            // (wiring changes only: a moving mechanism block's own updates are not counted; the
            // known BUD holds of real doors survive a neighbor piston firing)
            let b = self.chg(tr, k, n, 3);
            t.push(a);
            t.push(b);
            for d2 in DIRS {
                let n2 = add(n, d2);
                if n2 == c || !self.p.is_free(n2) {
                    continue;
                }
                let ks = self.p.kinds.get(&n2).cloned().unwrap_or_default();
                let lits: Vec<Lit> = ks.iter().filter(|kk| matches!(kk, Kind::Rep(..) | Kind::Obs(_) | Kind::Cmp(_)) && kk.out_to().contains(&neg(d2))).map(|kk| self.kl[&(n2, *kk)]).collect();
                if lits.is_empty() {
                    continue;
                }
                let isd = self.e.or(&lits);
                let o1 = self.out(tr, k, n2);
                let o0 = self.out(tr, k - 1, n2);
                let x = self.e.xor(o1, o0);
                t.push(self.e.and(&[isd, x]));
            }
        }
        self.e.or(&t)
    }
    fn oop(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        let z = self.p.z;
        self.lever_on_lits(tr, k, |l| l.pos[2] != z && l.pos[0] == c.0 && l.pos[1] == c.1)
    }
    fn strong_in(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        let z = self.p.z;
        let mut terms = Vec::new();
        for d in DIRS {
            let n = add(c, d);
            if let Some(ks) = self.p.kinds.get(&n).cloned() {
                let lits: Vec<Lit> = ks.iter().filter(|kk| kk.strong_to().contains(&neg(d))).map(|kk| self.kl[&(n, *kk)]).collect();
                if !lits.is_empty() {
                    let s = self.e.or(&lits);
                    let o = self.out(tr, k, n);
                    terms.push(self.e.and(&[s, o]));
                }
            }
        }
        terms.push(self.lever_on_lits(tr, k, |l| l.att == [c.0, c.1, z]));
        self.e.or(&terms)
    }
    fn weak_in(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        // dust on top powers the block below; only cross dust (not a dot) powers its side blocks
        let mut v = vec![self.dpow(tr, k, add(c, U))];
        for d in [E, W] {
            let n = add(c, d);
            let isx = self.kind(n, Kind::Dust);
            let l = self.dpow(tr, k, n);
            v.push(self.e.and(&[isx, l]));
        }
        self.e.or(&v)
    }
    /// `--update-order`: the quasi-connected power of the piston at c (from the cells around the cell
    /// above it, A) split by whether a change of its source delivers a block update to the piston:
    /// (updating, non-updating). A source change updates the blocks within distance 2 of a dust or a
    /// torch, the output cell of a diode/observer and that cell's neighbors, and the neighbors of a
    /// lever and of the block it is attached to. Direct power (the piston's own neighbors) always
    /// updates the piston; QC power through a conductor N next to A, powered by a diode, a dust on
    /// top of it, or a lever attached to it, does not (the piston is 2 cells from N): the piston
    /// keeps its old state (a BUD). Carried redstone blocks next to A: non-updating.
    fn qc_split(&mut self, tr: usize, k: i32, c: C2) -> (Lit, Lit) {
        if let Some(l) = self.qcs.get(&(tr, k, c)) {
            return *l;
        }
        let z = self.p.z;
        let a = add(c, U);
        let dist = |x: C2, y: C2| (x.0 - y.0).abs() + (x.1 - y.1).abs();
        let mut up = Vec::new();
        let mut nup = Vec::new();
        up.push(self.oop(tr, k, a));
        for d in [E, W, U] {
            let n = add(a, d);
            if let Some(ks) = self.p.kinds.get(&n).cloned() {
                // a component at n emitting into A: dust (cross pointing into A, or on top of A),
                // diodes/observers facing A, torches: their change updates A's neighbors
                if !ks.is_empty() {
                    if ks.contains(&Kind::Dust) && d != U {
                        let isx = self.kl[&(n, Kind::Dust)];
                        let l = self.dpow(tr, k, n);
                        up.push(self.e.and(&[isx, l]));
                    }
                    if d == U && ks.iter().any(|kk| matches!(kk, Kind::Dust | Kind::Dot)) {
                        up.push(self.dpow(tr, k, n));
                    }
                    if ks.contains(&Kind::RBlock) {
                        up.push(self.kl[&(n, Kind::RBlock)]);
                    }
                    let lits: Vec<Lit> = ks.iter().filter(|kk| kk.out_to().iter().any(|o| add(n, *o) == a)).map(|kk| self.kl[&(n, *kk)]).collect();
                    if !lits.is_empty() {
                        let s = self.e.or(&lits);
                        let o = self.out(tr, k, n);
                        up.push(self.e.and(&[s, o]));
                    }
                }
            }
            // a lever at n: updating only when attached to A or to a neighbor of the piston
            if self.on(tr, k) {
                for (i, l) in self.p.levers.clone().iter().enumerate() {
                    if l.pos == [n.0, n.1, z] {
                        let att = (l.att[0], l.att[1]);
                        let lv = self.lever[i];
                        if l.att[2] == z && (att == a || dist(att, c) == 1) {
                            up.push(lv);
                        } else {
                            nup.push(lv);
                        }
                    }
                }
            }
            // a conductor at n (wiring or mechanism block): split its power by source
            let isc = self.cond_at(tr, k, n);
            if isc == self.e.f() {
                continue;
            }
            let mut cu = Vec::new();
            let mut cn = Vec::new();
            for d2 in DIRS {
                let s = add(n, d2);
                if let Some(ks) = self.p.kinds.get(&s).cloned() {
                    // strong sources: diodes/observers facing n (non-updating), a torch below n
                    // (it is next to the piston or to A: updating)
                    let lits: Vec<Lit> = ks.iter().filter(|kk| kk.strong_to().contains(&neg(d2))).map(|kk| self.kl[&(s, *kk)]).collect();
                    if !lits.is_empty() {
                        let torch = d2 == DN;
                        let g = self.e.or(&lits);
                        let o = self.out(tr, k, s);
                        let t = self.e.and(&[g, o]);
                        if torch && dist(s, c) <= 2 {
                            cu.push(t);
                        } else {
                            cn.push(t);
                        }
                    }
                    // weak sources: dust on top of n, cross dust beside n
                    if d2 == U || ks.contains(&Kind::Dust) && d2 != DN {
                        let l = self.dpow(tr, k, s);
                        let t = if d2 == U { l } else { let isx = self.kind(s, Kind::Dust); self.e.and(&[isx, l]) };
                        if dist(s, c) <= 2 {
                            cu.push(t);
                        } else {
                            cn.push(t);
                        }
                    }
                }
            }
            // levers attached to n (in the plane or in front of / behind it)
            if self.on(tr, k) {
                for (i, l) in self.p.levers.clone().iter().enumerate() {
                    if l.att == [n.0, n.1, z] {
                        let lp = (l.pos[0], l.pos[1]);
                        let lv = self.lever[i];
                        if l.pos[2] == z && dist(lp, c) == 1 {
                            cu.push(lv);
                        } else {
                            cn.push(lv);
                        }
                    }
                }
            }
            let ou = self.e.or(&cu);
            let on = self.e.or(&cn);
            up.push(self.e.and(&[isc, ou]));
            nup.push(self.e.and(&[isc, on]));
            // a redstone block carried by the mechanism next to A
            if self.is_mech(n) {
                let kn = if k >= 1 && k < self.p.h { k + 1 } else { k };
                nup.push(self.mprop(tr, kn, n, 11));
            }
        }
        let r = (self.e.or(&up), self.e.or(&nup));
        self.qcs.insert((tr, k, c), r);
        r
    }
    /// Strong (`strong`) or any power of the conductor at c.
    fn cpow(&mut self, tr: usize, k: i32, c: C2, strong: bool) -> Lit {
        let m = if strong { &self.st } else { &self.pw };
        if let Some(l) = m.get(&(tr, k, c)) {
            return *l;
        }
        if self.p.is_free(c) || !self.is_mech(c) {
            return self.e.f();
        }
        let mc = self.mprop(tr, k, c, 0);
        if mc == self.e.f() {
            return mc;
        }
        let s = self.strong_in(tr, k, c);
        let l = if strong {
            self.e.and(&[mc, s])
        } else {
            let w = self.weak_in(tr, k, c);
            let a = self.e.or(&[s, w]);
            self.e.and(&[mc, a])
        };
        if strong {
            self.st.insert((tr, k, c), l);
        } else {
            self.pw.insert((tr, k, c), l);
        }
        l
    }
    /// The block at a gives a signal into b (`dust_rx`: as a wire computes it).
    fn sig(&mut self, tr: usize, k: i32, a: C2, b: C2, dust_rx: bool) -> Lit {
        if let Some(l) = self.sigc.get(&(tr, k, a, b, dust_rx)) {
            return *l;
        }
        let z = self.p.z;
        let mut terms = Vec::new();
        if let Some(ks) = self.p.kinds.get(&a).cloned() {
            if ks.iter().any(|kk| kk.cond()) {
                terms.push(Self::get(if dust_rx { &self.st } else { &self.pw }, (tr, k, a), self.e.f()));
            }
            if !dust_rx && ks.contains(&Kind::Dust) && (b == add(a, E) || b == add(a, W) || b == add(a, DN)) {
                let lv = self.lvl(tr, k, a, self.lo());
                let isx = self.kl[&(a, Kind::Dust)];
                let t = if b == add(a, DN) { lv } else { self.e.and(&[isx, lv]) };
                terms.push(t);
            }
            // a dot (no connections) powers only the block below it
            if !dust_rx && ks.contains(&Kind::Dot) && b == add(a, DN) {
                terms.push(self.dpow(tr, k, a));
            }
            if ks.contains(&Kind::RBlock) {
                terms.push(self.kl[&(a, Kind::RBlock)]);
            }
            let lits: Vec<Lit> = ks.iter().filter(|kk| kk.out_to().iter().any(|o| add(a, *o) == b)).map(|kk| self.kl[&(a, *kk)]).collect();
            if !lits.is_empty() {
                let s = self.e.or(&lits);
                let o = self.out(tr, k, a);
                terms.push(self.e.and(&[s, o]));
            }
        } else if self.is_mech(a) {
            terms.push(self.cpow(tr, k, a, dust_rx));
            // a redstone block carried by the mechanism powers its neighbors while it rests there
            // (its leaving or landing updates the wiring in the same tick: the frame after the tick)
            let kn = if k >= 1 && k < self.p.h { k + 1 } else { k };
            let rb = self.mprop(tr, kn, a, 11);
            terms.push(rb);
        }
        terms.push(self.lever_on_lits(tr, k, |l| l.pos == [a.0, a.1, z]));
        let l = self.e.or(&terms);
        self.sigc.insert((tr, k, a, b, dust_rx), l);
        l
    }
    /// Any neighbor signal into c (pistons without their front, bulbs, note blocks).
    fn any_in(&mut self, tr: usize, k: i32, c: C2, except: Option<C2>) -> Lit {
        let mut t = Vec::new();
        for d in DIRS {
            if Some(d) != except {
                t.push(self.sig(tr, k, add(c, d), c, false));
            }
        }
        t.push(self.oop(tr, k, c));
        self.e.or(&t)
    }
    /// Changes of the watched cell w during SAT tick k, by when they happen in the tick:
    /// 0 = repeater (first, before observers fire), 1 = ambiguous order (observers turning off or
    /// on from a phase-S schedule, torches, comparators, dust, note blocks, bulbs), 3 = phase S
    /// (1 + observers turning on from a block-event schedule: they run after every phase-S tick),
    /// 4 = block events (moving mechanism blocks, the lever flip), 2 = everything after phase R.
    fn chg(&mut self, tr: usize, k: i32, w: C2, what: u8) -> Lit {
        if let Some(l) = self.chgc.get(&(tr, k, w, what)) {
            return *l;
        }
        let f = self.e.f();
        let l = match what {
            2 => {
                let a = self.chg(tr, k, w, 3);
                let b = self.chg(tr, k, w, 4);
                self.e.or(&[a, b])
            }
            4 => {
                let m = if self.is_mech(w) { self.mchg(tr, k, w) } else { f };
                let z = self.p.z;
                // the lever flips at k = 0 in both traces (either polarity)
                let lv = if k == 0 { let ls: Vec<Lit> = self.p.levers.iter().enumerate().filter(|(_, l)| l.pos == [w.0, w.1, z]).map(|(i, _)| self.lever[i]).collect(); self.e.or(&ls) } else { f };
                // a note block's instrument follows the block below it: a mechanism change below that
                // flips its instrument class changes the note block (observers see it)
                let below = add(w, DN);
                let nb = if self.p.is_free(w) && self.p.can(w, |x| x == Kind::Note) && self.is_mech(below) {
                    let isn = self.pred(w, P_NOTE);
                    let ic = self.inst_chg(tr, k, below);
                    self.e.and(&[isn, ic])
                } else {
                    f
                };
                // an update piston's base changes in the block-event phase when it extends or retracts
                let up = if self.p.is_free(w) && self.p.can(w, |x| matches!(x, Kind::UPiston(_))) {
                    let isu = self.pred(w, P_UPISTON);
                    let o1 = self.out(tr, k, w);
                    let o0 = self.out(tr, k - 1, w);
                    let x = self.e.xor(o1, o0);
                    self.e.and(&[isu, x])
                } else {
                    f
                };
                self.e.or(&[m, lv, nb, up])
            }
            _ if !self.p.is_free(w) || k == 0 && what == 0 => f,
            0 => {
                let o1 = self.out(tr, k, w);
                let o0 = self.out(tr, k - 1, w);
                let oc = self.e.xor(o1, o0);
                let r = self.pred(w, P_REP);
                self.e.and(&[r, oc])
            }
            3 => {
                let a = self.chg(tr, k, w, 1);
                let on_e = self.chg(tr, k, w, 5);
                self.e.or(&[a, on_e])
            }
            5 => {
                let ob = self.pred(w, P_OBS);
                let o1 = self.out(tr, k, w);
                let o0 = self.out(tr, k - 1, w);
                let se = Self::get(&self.sched_e, (tr, k, w), f);
                self.e.and(&[ob, o1, !o0, se])
            }
            _ => {
                let o1 = self.out(tr, k, w);
                let o0 = self.out(tr, k - 1, w);
                let oc = self.e.xor(o1, o0);
                let mut t = Vec::new();
                let ob = self.pred(w, P_OBS);
                let to = self.pred(w, P_TORCH);
                let cm = self.pred(w, P_CMP);
                let lp = self.pred(w, P_LAMP);
                if let Some(lt) = self.ltrans.get(&(tr, k, w)).copied() {
                    t.push(lt);
                }
                let g = self.e.or(&[to, cm, lp]);
                t.push(self.e.and(&[g, oc]));
                let se = Self::get(&self.sched_e, (tr, k, w), f);
                let on_e = self.e.and(&[o1, !o0, se]);
                t.push(self.e.and(&[ob, oc, !on_e]));
                if self.p.can(w, |kk| matches!(kk, Kind::Dust | Kind::Dot)) {
                    let mut x = Vec::new();
                    for j in self.lo()..=15 {
                        let a = self.lvl(tr, k, w, j);
                        let b = self.lvl(tr, k - 1, w, j);
                        x.push(self.e.xor(a, b));
                    }
                    t.push(self.e.or(&x));
                }
                for j in 2..=7u8 {
                    if let (Some(a), Some(b)) = (self.leaves.get(&(tr, k, w, j)).copied(), self.leaves.get(&(tr, k - 1, w, j)).copied()) {
                        t.push(self.e.xor(a, b));
                    }
                }
                for m in 0..2 {
                    let map = if m == 0 { &self.npow } else { &self.bpow };
                    let a = Self::get(map, (tr, k, w), f);
                    let b = Self::get(map, (tr, k - 1, w), f);
                    if a != f {
                        t.push(self.e.xor(a, b));
                    }
                }
                self.e.or(&t)
            }
        };
        self.chgc.insert((tr, k, w, what), l);
        l
    }
    fn gsel(&mut self, name: &str) -> Lit {
        if let Some(g) = self.gsels.iter().find(|g| g.0 == name) {
            return g.1;
        }
        let v = self.e.var();
        self.gsels.push((name.to_string(), v));
        v
    }
    fn cause(&mut self, tr: usize, s: i32, c: C2, l: Lit) {
        self.causes.entry((tr, s, c)).or_default().push(l);
    }
}

/// Build the whole encoding.
/// `--diag`: guard constraint groups with selectors (UNSAT cores name the groups).
pub static DIAG: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--sound`: every remaining under-approximation that is cheap to flip is flipped (QC holds,
/// repeater and torch read orders): an UNSAT then over-approximates the game in those respects.
pub static SOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--log-doors`: the mechanism's stone blocks are oak logs (same mechanics; leaves read them).
pub static LOG_DOORS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--lever-on`: the lever is ON at rest (detected from a wired source design).
pub static LEVER_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--keep-ideal`: off-plane ideal inputs (by cell) that stay given sources instead of being wired.
pub static KEEP_IDEAL: std::sync::Mutex<Vec<C2>> = std::sync::Mutex::new(Vec::new());
/// `--local-nogood`: a failing candidate (exact timing) blocks only its choices near the first
/// misbehaving piston.
pub static LOCAL_NOGOOD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--update-order`: a piston reacts to a power change only when a block update reaches it after
/// the change (see `qc_split`): a retraction needs every non-updating source already off the tick
/// before (or a justified mechanism change next to it in the block-event phase, which runs after
/// the wiring); an extension needs an updating source rising in that tick (or power held from
/// before plus a wiring update or a mechanism change next to it).
pub static UPDATE_ORDER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--explain`: guard the update-order clauses with selectors (an UNSAT core names them).
pub static EXPLAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--period2`: four traces (two cycles) share the placement; the rest state may alternate between
/// cycles (the judge accepts a periodic rest: copper bulbs toggled once per cycle).
pub static PERIOD2: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--exact-order` (with `--sound`): keep `--sound`'s QC holds but not its free read orders of
/// repeaters and torches (torches read after the observer ticks: an observer pulse that powers a
/// torch's block ends before the torch's tick reads it, FIFO; candidates exploited the free order).
pub static EXACT_ORDER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--hint` (with `--from-wired`): phase the solver toward the source design's wiring.
pub static HINT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// `--retime` (ideal plans): a timing CEGAR. Touching events of different clusters keep only their
/// ORDER (ties allowed, no safe gap); each candidate's timing is checked by replaying the ideal plan
/// with every input moved with its piston's cluster (redsim, 4 positions, 2 cycles): a timing that
/// breaks the mechanism is blocked for every wiring (the too-close pairs it uses).
pub static RETIME: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Model<'_> {
    /// Cluster cl of trace tr plays recorded tick >= t at SAT tick k.
    fn ge_lit(&mut self, tr: usize, cl: usize, k: i32, t: i32) -> Lit {
        if k <= 0 {
            return self.e.konst(t <= 0);
        }
        let lits: Vec<Lit> = self.warp[tr][cl][k as usize].iter().filter(|x| x.0 >= t).map(|x| x.1).collect();
        self.e.or(&lits)
    }
    /// Pair i: event b (cluster y) starts less than its safe gap after event a (cluster x).
    fn close_lit(&mut self, i: usize) -> Lit {
        if let Some(l) = self.close_lits.get(&i) {
            return *l;
        }
        let (tr, x, a, y, b, d) = self.pairs[i];
        let mut terms = Vec::new();
        for k in 1..=self.p.h {
            let gy = self.ge_lit(tr, y, k, b);
            let gy0 = self.ge_lit(tr, y, k - 1, b);
            let gx = self.ge_lit(tr, x, k - d, a);
            terms.push(self.e.and(&[gy, !gy0, !gx]));
        }
        let l = self.e.or(&terms);
        self.close_lits.insert(i, l);
        l
    }
    /// The recorded tick cluster cl plays at SAT tick k in the current model.
    fn tau_val(&self, tr: usize, cl: usize, k: i32) -> i32 {
        if k <= 0 {
            return 0;
        }
        self.warp[tr][cl][k as usize].iter().find(|x| self.e.val(x.1)).map(|x| x.0).unwrap_or(0)
    }
    /// The first SAT tick at which cluster cl plays recorded tick >= t (current model).
    fn start_val(&self, tr: usize, cl: usize, t: i32) -> i32 {
        (1..=self.p.h).find(|k| self.tau_val(tr, cl, *k) >= t).unwrap_or(self.p.h + 1)
    }
}

/// `--retime`: the ideal plan with each input event moved with the cluster of the piston it drives
/// (an input set at plan tick t powers its piston at recorded tick t+1: it moves to the SAT tick
/// before the one at which that piston's cluster plays t+1).
fn retimed_plan(m: &Model, plan: &crate::ideal::Schedule) -> Result<crate::ideal::Schedule> {
    let p = m.p;
    let mut evs = Vec::new();
    for (tr, open) in [(0usize, true), (1usize, false)] {
        let ph = &p.rec.phases[tr];
        for (t, pos, st) in plan.resolved(open)? {
            let tau = t as i32 + 1;
            let c0 = (pos[0], pos[1]);
            let mut best: Option<(i32, C2)> = None;
            if st.as_deref() != Some("@use") {
                for (c, v) in ph.pow(tau) {
                    if ph.pow(tau - 1).get(c).copied().unwrap_or(false) != *v {
                        let d = (c.0 - c0.0).abs() + (c.1 - c0.1).abs();
                        if d <= 2 && best.is_none_or(|b| d < b.0) {
                            best = Some((d, *c));
                        }
                    }
                }
            }
            let nt = match best {
                Some((_, c)) => (m.start_val(tr, p.cl(c), tau) - 1).max(0) as u32,
                None => t,
            };
            evs.push((open, nt, pos, st.clone()));
        }
    }
    Ok(crate::ideal::Schedule::from_events(plan.design().clone(), &evs))
}

pub fn build(p: &Prob, check_mode: bool) -> Model<'_> {
    let diag = check_mode || DIAG.load(std::sync::atomic::Ordering::Relaxed);
    let mut e = Enc::new();
    let mut kl = HashMap::new();
    for c in &p.free {
        let lits: Vec<Lit> = p.kinds[c].iter().map(|k| {
            let l = e.var();
            kl.insert((*c, *k), l);
            l
        }).collect();
        e.exactly_one(&lits);
    }
    let lever: Vec<Lit> = p.levers.iter().map(|_| e.var()).collect();
    e.exactly_one(&lever);
    let mut alt_sel: Vec<Vec<Lit>> = Vec::new();
    for g in &p.rec.groups {
        let v: Vec<Lit> = g.alts.iter().map(|_| e.var()).collect();
        e.exactly_one(&v);
        alt_sel.push(v);
    }
    // the time warp: SAT tick k plays recorded tick tau (k <= 0: tau 0); a static recorded tick
    // may be replayed (a stall) and static ticks may be skipped; motion plays at full speed
    // --period2: four traces (open, close, open, close) share the placement; trace tr plays the
    // recording of phase tr % 2 and has its own time warp
    let nt = if PERIOD2.load(std::sync::atomic::Ordering::Relaxed) { 4 } else { 2 };
    let mut warp: Vec<Vec<Vec<Vec<(i32, Lit)>>>> = vec![Vec::new(); nt];
    for tr0 in 0..nt {
        let tr = tr0 % 2;
        for cl in 0..p.ncl {
            let mut wc: Vec<Vec<(i32, Lit)>> = vec![vec![(0, e.t())]];
            for k in 1..=p.h {
                let v: Vec<(i32, Lit)> = p.window(tr, k).map(|tau| (tau, e.var())).collect();
                let lits: Vec<Lit> = v.iter().map(|x| x.1).collect();
                e.exactly_one(&lits);
                wc.push(v);
            }
            for k in 0..p.h {
                for (tau, l) in wc[k as usize].clone() {
                    let mut next = Vec::new();
                    for (t2, l2) in &wc[k as usize + 1] {
                        let ok = if *t2 == tau {
                            p.static_tick_c(tr, tau, cl)
                        } else if *t2 > tau {
                            (tau + 1..*t2).all(|s| p.static_tick_c(tr, s, cl))
                        } else {
                            false
                        };
                        if ok {
                            next.push(*l2);
                        }
                    }
                    next.push(!l);
                    e.add(&next);
                }
            }
            let end = p.settled_from_c(tr, cl);
            for (tau, l) in wc[p.h as usize].clone() {
                if tau < end {
                    e.add(&[!l]);
                }
            }
            warp[tr0].push(wc);
        }
    }
    // cluster timings keep the recorded ORDER of events across clusters: when cluster Y starts its
    let mut pairs: Vec<(usize, usize, i32, usize, i32, i32)> = Vec::new();
    // event of recorded tick b, every cluster X has started its last event before b, at least
    // min(3, gap) ticks earlier (its motion has completed); ties of the recording stay free
    if p.ncl > 2 {
        let mut ge_cache: HashMap<(usize, usize, i32, i32), Lit> = HashMap::new();
        let mut ge = |e: &mut Enc, warp: &Vec<Vec<Vec<Vec<(i32, Lit)>>>>, tr: usize, cl: usize, k: i32, t: i32| -> Lit {
            if k <= 0 {
                return e.konst(t <= 0);
            }
            if let Some(l) = ge_cache.get(&(tr, cl, k, t)) {
                return *l;
            }
            let lits: Vec<Lit> = warp[tr][cl][k as usize].iter().filter(|x| x.0 >= t).map(|x| x.1).collect();
            let l = e.or(&lits);
            ge_cache.insert((tr, cl, k, t), l);
            l
        };
        for tr0 in 0..nt {
            let tr = tr0 % 2;
            let kk = p.rec.phases[tr].k();
            let motion: Vec<Vec<i32>> = (0..p.ncl).map(|cl| (1..=kk).filter(|t| !p.static_tick_c(tr, *t, cl)).collect()).collect();
            // event starts: a motion tick whose previous tick was static
            let starts: Vec<Vec<i32>> = motion.iter().map(|v| v.iter().copied().filter(|t| !v.contains(&(t - 1))).collect()).collect();
            // the cells an event changes (its run of motion ticks): only events that touch the
            // same or adjacent cells keep their recorded order (independent ones may swap)
            let ev_cells = |cl: usize, s: i32| -> HashSet<C2> {
                let mut out = HashSet::new();
                let mut t = s;
                while motion[cl].contains(&t) {
                    for ph in p.rec.trace_phases(tr) {
                        let (a, b) = (ph.frame(t - 1), ph.frame(t));
                        for c in a.keys().chain(b.keys()) {
                            if p.cl(*c) == cl && a.get(c) != b.get(c) {
                                out.insert(*c);
                            }
                        }
                    }
                    t += 1;
                }
                out
            };
            let touch = |a: &HashSet<C2>, b: &HashSet<C2>| a.iter().any(|c| b.contains(c) || [E, W, U, DN].iter().any(|d| b.contains(&add(*c, *d))));
            for y in 1..p.ncl {
                for &b in &starts[y] {
                    let cb = ev_cells(y, b);
                    for x in 1..p.ncl {
                        if x == y {
                            continue;
                        }
                        let Some(a) = starts[x].iter().copied().filter(|a| *a < b && touch(&ev_cells(x, *a), &cb)).max() else { continue };
                        let dmin = (b - a).min(3);
                        // --retime: only the order is kept (ties allowed); the gap is checked by
                        // replaying the retimed ideal plan (a timing nogood when it breaks)
                        let d = if RETIME.load(std::sync::atomic::Ordering::Relaxed) { 0 } else { dmin };
                        pairs.push((tr0, x, a, y, b, dmin));
                        for k in 1..=p.h {
                            let gy = ge(&mut e, &warp, tr0, y, k, b);
                            let gx = ge(&mut e, &warp, tr0, x, k - d, a);
                            e.add(&[!gy, gx]);
                        }
                    }
                }
            }
        }
    }
    let mut m = Model {
        p,
        e,
        kl,
        preds: HashMap::new(),
        lever,
        warp,
        nt,
        out: HashMap::new(),
        sched: HashMap::new(),
        sched_e: HashMap::new(),
        causes_e: HashMap::new(),
        ltrans: HashMap::new(),
        lv: HashMap::new(),
        st: HashMap::new(),
        pw: HashMap::new(),
        bpow: HashMap::new(),
        blit: HashMap::new(),
        npow: HashMap::new(),
        leaves: HashMap::new(),
        sigc: HashMap::new(),
        mc: HashMap::new(),
        chgc: HashMap::new(),
        causes: HashMap::new(),
        used: Vec::new(),
        checks: Vec::new(),
        check_mode,
        sels: Vec::new(),
        alt_sel,
        qcs: HashMap::new(),
        uo_sels: Vec::new(),
        pairs,
        close_lits: HashMap::new(),
        gsels: Vec::new(),
    };
    let h = p.h;
    let lo = m.lo();
    if diag {
        let g = m.gsel("supports, lever, fixed cells");
        m.e.guard.push(g);
    }
    // ---- structural: supports, levers, fixed cells, objective
    for c in p.free.clone() {
        for k in p.kinds[&c].clone() {
            let l = m.kl[&(c, k)];
            if let Some(sd) = k.support() {
                let s = m.sturdy_static(add(c, sd), sd == DN);
                m.e.add(&[!l, s]);
            }
            if let Kind::UPiston(fd) = k {
                // retracted at rest (the file writes it retracted, with no head)
                let o_rest = m.out(0, -1, c);
                m.e.add(&[!l, !o_rest]);
                // its head cell stays free (air at rest, never wiring, never the lever)
                let hc = add(c, fd);
                if p.is_free(hc) {
                    let em = m.kl.get(&(hc, Kind::Empty)).copied().unwrap_or(m.e.f());
                    m.e.add(&[!l, em]);
                    for (i, lc) in p.levers.iter().enumerate() {
                        if lc.pos == [hc.0, hc.1, p.z] {
                            let lv = m.lever[i];
                            m.e.add(&[!l, !lv]);
                        }
                    }
                } else {
                    m.e.add(&[!l]);
                }
            }
            if let Kind::Cmp(f) = k {
                let d = m.pred(add(c, f), P_DUST);
                m.e.add(&[!l, !d]);
            }
        }
        let empty = m.kl.get(&(c, Kind::Empty)).copied().unwrap_or(m.e.f());
        m.used.push((c, !empty));
        if let Some(fk) = p.fixed.get(&c) {
            let l = m.kind(c, *fk);
            m.e.add(&[l]);
        } else if p.fixed_only {
            m.e.add(&[empty]);
        }
    }
    for (i, lc) in p.levers.iter().enumerate() {
        let l = m.lever[i];
        let att = (lc.att[0], lc.att[1]);
        let s = m.sturdy_static(att, lc.pos[1] > lc.att[1]);
        m.e.add(&[!l, s]);
        if lc.pos[2] == p.z {
            let c = (lc.pos[0], lc.pos[1]);
            if p.is_free(c) {
                let em = m.kl[&(c, Kind::Empty)];
                m.e.add(&[!l, em]);
            }
        }
        if let Some(fi) = p.fixed_lever {
            m.e.add(&[if fi == i { l } else { !l }]);
        }
    }
    m.e.guard.clear();
    // ---- allocate the dynamic variables
    for tr in 0..nt {
        for k in -1..=h {
            for c in p.free.clone() {
                if p.can(c, |kk| kk.has_out()) {
                    let v = m.e.var();
                    m.out.insert((tr, k, c), v);
                }
                if p.can(c, |kk| matches!(kk, Kind::Dust | Kind::Dot)) {
                    for j in lo..=15 {
                        let v = m.e.var();
                        m.lv.insert((tr, k, c, j), v);
                    }
                }
                if p.can(c, |kk| kk.cond()) {
                    let a = m.e.var();
                    let b = m.e.var();
                    m.st.insert((tr, k, c), a);
                    m.pw.insert((tr, k, c), b);
                }
                if p.can(c, |kk| kk == Kind::Bulb) {
                    let a = m.e.var();
                    let b = m.e.var();
                    m.bpow.insert((tr, k, c), a);
                    m.blit.insert((tr, k, c), b);
                }
                if p.can(c, |kk| kk == Kind::Lamp) {
                    let a = m.e.var();
                    m.ltrans.insert((tr, k, c), a);
                }
                if p.can(c, |kk| kk == Kind::Leaves) {
                    for j in 2..=7u8 {
                        let a = m.e.var();
                        m.leaves.insert((tr, k, c, j), a);
                    }
                }
                if p.can(c, |kk| matches!(kk, Kind::Note | Kind::Trap | Kind::TrapTop | Kind::Dropper)) {
                    let a = m.e.var();
                    m.npow.insert((tr, k, c), a);
                }
            }
        }
        for s in 2..=h {
            for c in p.free.clone() {
                if p.can(c, |kk| kk.has_out()) {
                    let v = m.e.var();
                    m.sched.insert((tr, s, c), v);
                    if p.can(c, |kk| matches!(kk, Kind::Obs(_))) {
                        let v2 = m.e.var();
                        m.sched_e.insert((tr, s, c), v2);
                    }
                }
            }
        }
    }
    // ---- per trace, per tick
    for tr in 0..nt {
        for k in -1..=h {
            for c in p.free.clone() {
                let ks = p.kinds[&c].clone();
                m.e.guard.clear();
                if diag {
                    let g = m.gsel(&format!("cell ({},{})", c.0, c.1));
                    m.e.guard.push(g);
                }
                // conductors
                if ks.iter().any(|kk| kk.cond()) {
                    let isc = m.pred(c, P_COND);
                    let s = m.strong_in(tr, k, c);
                    let w = m.weak_in(tr, k, c);
                    let stv = m.st[&(tr, k, c)];
                    let pwv = m.pw[&(tr, k, c)];
                    let a = m.e.and(&[isc, s]);
                    m.e.eq(stv, a);
                    let o = m.e.or(&[s, w]);
                    let b = m.e.and(&[isc, o]);
                    m.e.eq(pwv, b);
                }
                // dust levels
                if ks.iter().any(|kk| matches!(kk, Kind::Dust | Kind::Dot)) {
                    let isd = m.pred(c, P_DUST);
                    let mut src = Vec::new();
                    for d in DIRS {
                        src.push(m.sig(tr, k, add(c, d), c, true));
                    }
                    src.push(m.oop(tr, k, c));
                    let src = m.e.or(&src);
                    let up_c = m.cond_at(tr, k, add(c, U));
                    for j in lo..=15 {
                        let mut sup = vec![src];
                        for s in [E, W] {
                            let n = add(c, s);
                            sup.push(m.lvl(tr, k, n, j + 1));
                            let nc = m.cond_at(tr, k, n);
                            let upn = add(n, U);
                            let dnn = add(n, DN);
                            if p.can(upn, |kk| matches!(kk, Kind::Dust | Kind::Dot)) {
                                let g = m.e.and(&[nc, !up_c]);
                                let l = m.lvl(tr, k, upn, j + 1);
                                sup.push(m.e.and(&[g, l]));
                            }
                            if p.can(dnn, |kk| matches!(kk, Kind::Dust | Kind::Dot)) {
                                let l = m.lvl(tr, k, dnn, j + 1);
                                sup.push(m.e.and(&[!nc, l]));
                            }
                        }
                        let any = m.e.or(&sup);
                        let rhs = m.e.and(&[isd, any]);
                        let lj = m.lvl(tr, k, c, j);
                        m.e.eq(lj, rhs);
                        if j == lo && lo > 1 {
                            // capped levels: a powered connected neighbor at the lowest modeled
                            // level would give c a real (unmodeled) level: forbid
                            for s in [E, W] {
                                let n = add(c, s);
                                let ln = m.lvl(tr, k, n, lo);
                                m.e.add(&[!isd, !ln, lj]);
                            }
                        }
                    }
                }
                // note blocks, bulbs
                // leaves: distance = min(7, 1 + min over neighbors (log 0, leaves d, else 7)),
                // recomputed 1 gt after any neighbor change (a moving log reads as 7)
                if ks.contains(&Kind::Leaves) {
                    let isl = m.pred(c, P_LEAVES);
                    let kk = if k >= 1 { k - 1 } else { -1 };
                    if k == 0 {
                        for j in 2..=7u8 {
                            let a = m.leaves[&(tr, 0, c, j)];
                            let b = m.leaves[&(tr, -1, c, j)];
                            m.e.eq(a, b);
                        }
                    } else {
                        let logs = LOG_DOORS.load(std::sync::atomic::Ordering::Relaxed);
                        for j in 2..=7u8 {
                            let mut all = Vec::new();
                            for d in DIRS {
                                let n = add(c, d);
                                if m.p.mech.contains(&n) {
                                    if logs {
                                        let lg = m.mprop(tr, k.max(-1), n, 0);
                                        all.push(!lg);
                                    }
                                } else if m.p.is_free(n) && m.p.can(n, |x| x == Kind::Leaves) {
                                    if j - 1 >= 2 {
                                        let nl = m.pred(n, P_LEAVES);
                                        let g = m.leaves[&(tr, kk, n, j - 1)];
                                        all.push(m.e.or(&[!nl, g]));
                                    }
                                }
                            }
                            let tgt = m.e.and(&all);
                            let rhs = m.e.and(&[isl, tgt]);
                            let a = m.leaves[&(tr, k, c, j)];
                            m.e.eq(a, rhs);
                        }
                    }
                }
                // note blocks, iron trapdoors, droppers: a powered state observers can watch
                if ks.iter().any(|kk| matches!(kk, Kind::Note | Kind::Trap | Kind::TrapTop | Kind::Dropper)) {
                    let isi = m.pred(c, P_IND);
                    let isd = m.pred(c, P_DROP);
                    let i = m.any_in(tr, k, c, None);
                    let q = add(c, U);
                    let mut qt = Vec::new();
                    for dd in [E, W, U] {
                        qt.push(m.sig(tr, k, add(q, dd), q, false));
                    }
                    qt.push(m.oop(tr, k, q));
                    let qc = m.e.or(&qt);
                    // a dropper re-reads its power (direct, or quasi-connected: the cell above)
                    // only when a block update reaches it: when a neighbor changes state, its own
                    // direct power changes, or the lever next to it (or on a neighbor) flips; it
                    // keeps its `triggered` state otherwise (a BUD)
                    let v = m.npow[&(tr, k, c)];
                    let isnt = m.e.and(&[isi, !isd]);
                    let nt = m.e.and(&[isnt, i]);
                    let dexpr = if k == -1 {
                        m.e.or(&[i, qc])
                    } else {
                        let i_p = m.any_in(tr, k - 1, c, None);
                        let mut up = vec![m.e.xor(i, i_p)];
                        for d in DIRS {
                            let n = add(c, d);
                            up.push(m.chg(tr, k, n, 0));
                            up.push(m.chg(tr, k, n, 2));
                            if k == 0 {
                                let z = m.p.z;
                                let z = m.p.z;
                                let ls: Vec<Lit> = m.p.levers.iter().enumerate().filter(|(_, l)| l.att == [n.0, n.1, z]).map(|(i, _)| m.lever[i]).collect();
                                up.push(m.e.or(&ls));
                            }
                        }
                        // a diode/observer whose output block is next to the dropper changing
                        // (it updates that block's neighbors: the dropper re-reads its QC power at
                        // once; without it a 2 gt observer loop through a dropper looked settled)
                        up.push(m.update_at(tr, k, c));
                        let upd = m.e.or(&up);
                        let rd = m.e.or(&[i, qc]);
                        let a1 = m.e.and(&[upd, rd]);
                        let tp = m.npow[&(tr, k - 1, c)];
                        let a2 = m.e.and(&[!upd, tp]);
                        m.e.or(&[a1, a2])
                    };
                    let dd = m.e.and(&[isd, dexpr]);
                    let rhs = m.e.or(&[nt, dd]);
                    m.e.eq(v, rhs);
                }
                if ks.contains(&Kind::Bulb) {
                    let isb = m.pred(c, P_BULB);
                    let i = m.any_in(tr, k, c, None);
                    let a = m.e.and(&[isb, i]);
                    let bp = m.bpow[&(tr, k, c)];
                    m.e.eq(bp, a);
                    let bl = m.blit[&(tr, k, c)];
                    m.e.add(&[isb, !bl]);
                    if k >= 0 {
                        let bp0 = m.bpow[&(tr, k - 1, c)];
                        let bl0 = m.blit[&(tr, k - 1, c)];
                        let rise = m.e.and(&[bp, !bp0]);
                        let x = m.e.xor(bl0, rise);
                        let rhs = m.e.and(&[isb, x]);
                        m.e.eq(bl, rhs);
                    }
                }
                if !ks.iter().any(|kk| kk.has_out()) {
                    continue;
                }
                let has_out = m.pred(c, P_OUT);
                let o_k = m.out(tr, k, c);
                m.e.add(&[has_out, !o_k]);
                let o_p = m.out(tr, k - 1, c);
                let s_k = m.sched(tr, k, c);
                let f = m.e.f();
                for kind in ks.clone() {
                    // at the flip (k = 0) only a lamp can change (no scheduled tick has run)
                    if k == 0 && kind.has_out() && kind != Kind::Lamp {
                        let g0 = m.kl[&(c, kind)];
                        m.e.eq_if(g0, o_k, o_p);
                    }
                    let g = m.kl[&(c, kind)];
                    match kind {
                        Kind::Obs(fd) => {
                            let w = add(c, fd);
                            if k == -1 {
                                m.e.add(&[!g, !o_k]);
                                continue;
                            }
                            if k >= 1 {
                                let x = m.e.xor(o_p, s_k);
                                m.e.eq_if(g, o_k, x);
                            }
                            let pre = m.chg(tr, k, w, 0);
                            let post_s = m.chg(tr, k, w, 3);
                            let post_e = m.chg(tr, k, w, 4);
                            let onp = if k >= 1 { o_p } else { o_k };
                            let s1 = m.sched(tr, k + 1, c);
                            let a = m.e.and(&[pre, !onp]);
                            // an ambiguous change of the watched cell coinciding with this observer's
                            // off-tick: seen before (ignored) or after it (a new pulse): either
                            let _ = post_s;
                            let offfire = if k >= 1 { m.e.and(&[s_k, o_p]) } else { f };
                            let amb = m.chg(tr, k, w, 1);
                            let one = m.chg(tr, k, w, 5);
                            let ch = m.e.var();
                            let nb = m.e.or(&[!offfire, ch]);
                            let amb_ok = m.e.and(&[amb, nb]);
                            let seen = m.e.or(&[amb_ok, one]);
                            let b = m.e.and(&[seen, !o_k]);
                            let be = m.e.and(&[post_e, !o_k]);
                            let fire_on = if k >= 1 { m.e.and(&[s_k, !o_p]) } else { f };
                            let s_part = m.e.or(&[a, b]);
                            let s_trig = m.e.and(&[!s1, s_part]);
                            let s_any = m.e.or(&[fire_on, s_trig]);
                            let e_trig = m.e.and(&[!s1, be]);
                            let any = m.e.or(&[s_any, e_trig]);
                            let cs = m.e.and(&[g, any]);
                            m.cause(tr, k + 2, c, cs);
                            let ce = m.e.and(&[g, e_trig, !s_any]);
                            m.causes_e.entry((tr, k + 2, c)).or_default().push(ce);
                        }
                        Kind::Rep(dl, fd) => {
                            let dd = 2 * dl as i32;
                            let i_k = m.sig(tr, k, add(c, fd), c, false);
                            if k == -1 {
                                m.e.eq_if(g, o_k, i_k);
                                continue;
                            }
                            let mut why = Vec::new();
                            if k >= 1 {
                                let i_p = m.sig(tr, k - 1, add(c, fd), c, false);
                                // fire: on = !on_prev | in_prev
                                // --sound: repeaters of different priorities run in order within phase R: it
                                // may read its input before or after a repeater feeding it fires
                                let i_x = if SOUND.load(std::sync::atomic::Ordering::Relaxed) && !EXACT_ORDER.load(std::sync::atomic::Ordering::Relaxed) {
                                    let ch = m.e.var();
                                    let a = m.e.and(&[ch, i_k]);
                                    let b = m.e.and(&[!ch, i_p]);
                                    m.e.or(&[a, b])
                                } else {
                                    i_p
                                };
                                let fire_val = m.e.or(&[!o_p, i_x]);
                                let g1 = m.e.and(&[g, s_k]);
                                m.e.eq_if(g1, o_k, fire_val);
                                let g2 = m.e.and(&[g, !s_k]);
                                m.e.eq_if(g2, o_k, o_p);
                                why.push(m.e.and(&[s_k, !o_p, !i_x]));
                            }
                            let mis = m.e.xor(o_k, i_k);
                            let pend: Vec<Lit> = (k + 1..k + dd).map(|j| m.sched(tr, j, c)).collect();
                            let pend = m.e.or(&pend);
                            why.push(m.e.and(&[mis, !pend]));
                            let any = m.e.or(&why);
                            let cs = m.e.and(&[g, any]);
                            m.cause(tr, k + dd, c, cs);
                        }
                        Kind::Torch | Kind::WTorch(_) => {
                            let src = match kind {
                                Kind::WTorch(fd) => add(c, neg(fd)),
                                _ => add(c, DN),
                            };
                            let i_k = m.sig(tr, k, src, c, false);
                            if k == -1 {
                                m.e.eq_if(g, o_k, !i_k);
                                continue;
                            }
                            if k >= 1 {
                                let g1 = m.e.and(&[g, s_k]);
                                if SOUND.load(std::sync::atomic::Ordering::Relaxed) && !EXACT_ORDER.load(std::sync::atomic::Ordering::Relaxed) {
                                    // its tick runs among the observer ticks (FIFO): it may read its input
                                    // before or after them
                                    let i_p = m.sig(tr, k - 1, src, c, false);
                                    m.e.add(&[!g1, o_k, i_k, i_p]);
                                    m.e.add(&[!g1, !o_k, !i_k, !i_p]);
                                } else {
                                    m.e.eq_if(g1, o_k, !i_k);
                                }
                                let g2 = m.e.and(&[g, !s_k]);
                                m.e.eq_if(g2, o_k, o_p);
                            }
                            let s1 = m.sched(tr, k + 1, c);
                            let mis = m.e.xor(o_k, !i_k);
                            let cs = m.e.and(&[g, mis, !s1]);
                            m.cause(tr, k + 2, c, cs);
                        }
                        Kind::UPiston(fd) => {
                            if k != 0 {
                                let direct = m.any_in(tr, k, c, Some(fd));
                                let q = add(c, U);
                                let mut qt = Vec::new();
                                for dd in [E, W, U] {
                                    qt.push(m.sig(tr, k, add(q, dd), q, false));
                                }
                                qt.push(m.oop(tr, k, q));
                                let qc = m.e.or(&qt);
                                let pw = m.e.or(&[direct, qc]);
                                m.e.eq_if(g, o_k, pw);
                            }
                        }
                        Kind::Lamp => {
                            let i_k = m.any_in(tr, k, c, None);
                            if k == -1 {
                                m.e.eq_if(g, o_k, i_k);
                                continue;
                            }
                            let lt = m.ltrans[&(tr, k, c)];
                            if k >= 1 {
                                // the lamp's 4 gt tick was scheduled 4 gt ago: it runs after the
                                // repeaters and BEFORE every observer/torch tick of this tick (FIFO), so
                                // it reads its input as at the start of the tick; an input rising later
                                // in the tick relights it at once (an off-on transient observers see)
                                let i_mid = m.any_in(tr, k - 1, c, None);
                                let fire = m.e.and(&[g, s_k, o_p]);
                                let a = m.e.or(&[i_mid, i_k]);
                                m.e.eq_if(fire, o_k, a);
                                let nfire = m.e.and(&[g, !fire]);
                                let b = m.e.or(&[o_p, i_k]);
                                m.e.eq_if(nfire, o_k, b);
                                let tv = m.e.and(&[g, fire, !i_mid, i_k]);
                                m.e.eq(lt, tv);
                            } else {
                                m.e.add(&[!lt]);
                                let a0 = m.e.or(&[o_p, i_k]);
                                m.e.eq_if(g, o_k, a0);
                            }
                            let pend: Vec<Lit> = (k + 1..k + 4).map(|j| m.sched(tr, j, c)).collect();
                            let pend = m.e.or(&pend);
                            let cs = m.e.and(&[g, o_k, !i_k, !pend]);
                            m.cause(tr, k + 4, c, cs);
                        }
                        Kind::Cmp(fd) => {
                            let r = add(c, fd);
                            let s0 = m.sig(tr, k, r, c, false);
                            let bb = m.pred(r, P_BULB);
                            let bl = Model::get(&m.blit, (tr, k, r), f);
                            let bl2 = m.e.and(&[bb, bl]);
                            let i_k = m.e.or(&[s0, bl2]);
                            if k == -1 {
                                m.e.eq_if(g, o_k, i_k);
                                continue;
                            }
                            if k >= 1 {
                                let s0p = m.sig(tr, k - 1, r, c, false);
                                let blp = Model::get(&m.blit, (tr, k - 1, r), f);
                                let blp2 = m.e.and(&[bb, blp]);
                                let i_p = m.e.or(&[s0p, blp2]);
                                let g1 = m.e.and(&[g, s_k]);
                                // an input change in the same tick as its tick: it reads either value
                                // (--exact-order: its NORMAL-priority tick runs after the observer and
                                // repeater changes of the tick that scheduled them first: it reads the
                                // input at the end of the tick; a 2 gt observer pulse never passes)
                                if EXACT_ORDER.load(std::sync::atomic::Ordering::Relaxed) {
                                    m.e.eq_if(g1, o_k, i_k);
                                } else {
                                    m.e.add(&[!g1, !i_p, !i_k, o_k]);
                                    m.e.add(&[!g1, i_p, i_k, !o_k]);
                                }
                                let g2 = m.e.and(&[g, !s_k]);
                                m.e.eq_if(g2, o_k, o_p);
                            }
                            let s1 = m.sched(tr, k + 1, c);
                            let mis = m.e.xor(o_k, i_k);
                            let cs = m.e.and(&[g, mis, !s1]);
                            m.cause(tr, k + 2, c, cs);
                        }
                        _ => {}
                    }
                }
            }
        }
        m.e.guard.clear();
        // ---- pistons get exactly their recorded power (in the alternative chosen for their group)
        let mech: Vec<C2> = p.mech.iter().copied().collect();
        let ks: Vec<i32> = std::iter::once(-1).chain(1..=h).collect();
        for &k in &ks {
            for &c in &mech {
                let opts = p.frame_opts(tr % 2, c);
                for (tau, wl) in m.warp_at_c(tr, k, p.cl(c)) {
                  for (ga, ph) in &opts {
                    let Some(fc) = p.req_facing(ph, tau, c) else { continue };
                    // motion mode: the power matters only where the motion needs it AND the recording
                    // agrees (a recorded disagreement is the game tolerating it: a QC-held piston
                    // without an update, the 2nd tick of a short pulse...)
                    let mraw = if p.motion { p.motion_req_ph(ph, tau, c) } else { None };
                    let recp = ph.pow(tau).get(&c).copied().unwrap_or(false);
                    let mreq = mraw.filter(|v| *v == recp);
                    // a piston that must stay off while the recording holds it powered (a QC hold the
                    // game never updated): QC power may stay, DIRECT power may not (it always comes
                    // with an update of the piston, which would fire it); only where its room to
                    // extend is certain (else the game may simply have been blocked)
                    let stays_ret = tau >= 1 && matches!(ph.frame(tau - 1).get(&c).map(|s| mk(s)), Some(MK::Piston { ext: false, .. })) && ph.frame(tau - 1).get(&c) == ph.frame(tau).get(&c);
                    let qc_only = p.motion && mraw == Some(false) && recp && k >= 1 && stays_ret && p.can_extend_sure(&ph.frame(tau - 1), c, fc) && p.can_extend_sure(&ph.frame(tau), c, fc);
                    if p.motion && mreq.is_none() && !qc_only {
                        continue;
                    }
                    let req = match mreq {
                        Some(v) if p.motion => v,
                        _ => ph.pow(tau).get(&c).copied().unwrap_or(false),
                    };
                    let d = tau;
                    let front = if fc == (0, 0) { None } else { Some(fc) };
                    let direct0 = m.any_in(tr, k, c, front);
                    let cl = p.cl(c);
                    let gd = m.given(tr, k, c, cl);
                    let direct = m.e.or(&[direct0, gd]);
                    let q = add(c, U);
                    let mut qt = Vec::new();
                    for dd in [E, W, U] {
                        qt.push(m.sig(tr, k, add(q, dd), q, false));
                    }
                    qt.push(m.oop(tr, k, q));
                    qt.push(m.given(tr, k, q, cl));
                    let qc = m.e.or(&qt);
                    let ds = match ga {
                        None => wl,
                        Some((g, a)) => {
                            let s = m.alt_sel[*g][*a];
                            m.e.and(&[wl, s])
                        }
                    };
                    let sel = match m.sels.iter().find(|s| s.0 == tr && s.1 == c) {
                        Some(s) => s.2,
                        None => {
                            let v = m.e.var();
                            m.sels.push((tr, c, v));
                            v
                        }
                    };
                    if check_mode {
                        // --sound: a QC hold of a piston that stays retracted is fine while no update
                        // reaches it
                        let stays_ret_c = matches!(ph.frame(tau - 1).get(&c).map(|s| mk(s)), Some(MK::Piston { ext: false, .. })) && ph.frame(tau - 1).get(&c) == ph.frame(tau).get(&c);
                        let bud = if !req && k >= 0 && stays_ret_c && SOUND.load(std::sync::atomic::Ordering::Relaxed) { Some(m.update_at(tr, k, c)) } else { None };
                        m.checks.push(PCheck { tr, k, c, d, req, direct, qc, bud });
                        continue;
                    }
                    // (kept for `--explain`: the model's power of each required piston-tick)
                    m.checks.push(PCheck { tr, k, c, d, req, direct, qc, bud: None });
                    // --update-order: the motion that starts here needs a block update after the
                    // power change (the lever flip at k = 0 belongs to tick 1)
                    if UPDATE_ORDER.load(std::sync::atomic::Ordering::Relaxed) && k >= 1 && tau >= 1 {
                        let kp = if k == 1 { -1 } else { k - 1 };
                        let before = ph.frame(tau - 1).get(&c).map(|s| mk(s));
                        let after = ph.frame(tau).get(&c).map(|s| mk(s));
                        let changed = ph.frame(tau - 1).get(&c) != ph.frame(tau).get(&c);
                        let start_ret = changed && matches!(before, Some(MK::Piston { ext: true, .. }));
                        let start_ext = matches!((before, after), (Some(MK::Piston { ext: false, f: f0, .. }), Some(MK::Piston { ext: true, f: f1, .. })) if f0 == f1);
                        // a piston that has just been placed (moved here, or its retraction just
                        // finished) checks its power on placement: no update needed
                        let arrived = tau >= 2 && ph.frame(tau - 2).get(&c) != ph.frame(tau - 1).get(&c);
                        let start_ext = start_ext && !arrived;
                        // a mechanism change next to the piston (not its own head) in tick k: the
                        // block-event phase runs after the wiring, so it updates the piston last;
                        // a neighbor piston starting to retract counts only if it has no
                        // non-updating source itself (no circular justification)
                        let mut esc = Vec::new();
                        if start_ret || start_ext {
                            for dd in DIRS {
                                let n = add(c, dd);
                                if dd == fc || !p.mech.contains(&n) {
                                    continue;
                                }
                                let ch = m.mchg(tr, k, n);
                                let nb = ph.frame(tau - 1).get(&n).map(|s| mk(s));
                                let na = ph.frame(tau).get(&n).map(|s| mk(s));
                                let nchg = ph.frame(tau - 1).get(&n) != ph.frame(tau).get(&n);
                                let nret = nchg && matches!(nb, Some(MK::Piston { ext: true, .. }));
                                let next = matches!((nb, na), (Some(MK::Piston { ext: false, f: f0, .. }), Some(MK::Piston { ext: true, f: f1, .. })) if f0 == f1);
                                if nret {
                                    let (_, nn) = m.qc_split(tr, kp, n);
                                    esc.push(m.e.and(&[ch, !nn]));
                                } else if next {
                                    // a neighbor piston extending in this tick: justified by its own
                                    // updating power (direct, or updating QC)
                                    let nf = match nb { Some(MK::Piston { f, .. }) => Some(f), _ => None };
                                    let nd = m.any_in(tr, k, n, nf);
                                    let (nq, _) = m.qc_split(tr, k, n);
                                    let o = m.e.or(&[nd, nq]);
                                    esc.push(m.e.and(&[ch, o]));
                                } else {
                                    esc.push(ch);
                                }
                            }
                        }
                        if start_ret {
                            let (_, nup) = m.qc_split(tr, kp, c);
                            let mut cl = vec![!ds, !sel, !nup];
                            // (or a wiring update reaching it in this tick: a diode/observer next to it
                            // changing, which may come after the non-updating source went off)
                            let ua = m.update_at(tr, k, c);
                            cl.push(ua);
                            if EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) {
                                let g = m.e.var();
                                m.uo_sels.push((tr, k, c, "retraction", g));
                                cl.push(!g);
                            }
                            cl.extend(esc.iter().copied());
                            m.e.add(&cl);
                        } else if start_ext {
                            let (qu, _) = m.qc_split(tr, k, c);
                            let (qu0, _) = m.qc_split(tr, kp, c);
                            let d0 = m.any_in(tr, kp, c, front);
                            let g0 = m.given(tr, kp, c, cl);
                            let gq0 = m.given(tr, kp, q, cl);
                            let up_k = m.e.or(&[direct, qu]);
                            let up_p = m.e.or(&[d0, g0, gq0, qu0]);
                            let rise = m.e.and(&[up_k, !up_p]);
                            // power held from before (a QC arm, or a blocked piston) fired by an update
                            let mut qt0 = Vec::new();
                            for dd in [E, W, U] {
                                qt0.push(m.sig(tr, kp, add(q, dd), q, false));
                            }
                            qt0.push(m.oop(tr, kp, q));
                            let q0 = m.e.or(&qt0);
                            let held = m.e.or(&[d0, g0, gq0, q0]);
                            let mut upd = vec![m.update_at(tr, k, c)];
                            // a moving block next to it finishing in the previous tick (the block-entity
                            // phase runs last: the piston reads that tick's final power, its event runs
                            // in this tick)
                            if tau >= 2 {
                                for dd in DIRS {
                                    let n = add(c, dd);
                                    if dd == fc || !p.mech.contains(&n) {
                                        continue;
                                    }
                                    let was_moving = ph.frame(tau - 2).get(&n).is_some_and(|s| short(s) == "moving_piston");
                                    if was_moving && ph.frame(tau - 2).get(&n) != ph.frame(tau - 1).get(&n) {
                                        let ch = if kp >= 1 { m.mchg(tr, kp, n) } else { m.e.f() };
                                        upd.push(ch);
                                    }
                                }
                            }
                            let u = m.e.or(&upd);
                            let fired = m.e.and(&[held, u]);
                            // a justified mechanism change next to it (block-event phase, after the
                            // wiring): it reads this tick's power (required on here)
                            upd.clear();
                            upd.push(rise);
                            upd.push(fired);
                            upd.extend(esc.iter().copied());
                            let mut cl = vec![!ds, !sel];
                            if EXPLAIN.load(std::sync::atomic::Ordering::Relaxed) {
                                let g = m.e.var();
                                m.uo_sels.push((tr, k, c, "extension", g));
                                cl.push(!g);
                            }
                            cl.extend(upd);
                            m.e.add(&cl);
                        }
                    }
                    // a retracted piston that stays retracted while powered (blocked, or QC-armed
                    // without an update) does not care about its power at that tick
                    let blocked = !p.motion && req && p.blocked_dc && k >= 1 && {
                        let ph = &p.rec.phases[tr % 2];
                        let a = ph.frame(tau - 1).get(&c);
                        let b = ph.frame(tau).get(&c);
                        a == b && a.is_some_and(|s| matches!(mk(s), MK::Piston { ext: false, .. }))
                    };
                    // the 2nd tick of a pulse of at most 2 gt (or the tick right after a 1 gt pulse):
                    // a 1 gt and a 2 gt pulse do the same (a push spits, a gap-extension pulls)
                    let pulse_tol = !p.motion && p.blocked_dc && k >= 1 && {
                        let ph = &p.rec.phases[tr % 2];
                        let pw = |j: i32| ph.pow(j).get(&c).copied().unwrap_or(false);
                        let same = |j: i32| ph.frame(j).get(&c) == ph.frame(tau - 1).get(&c) || j >= tau;
                        tau >= 2 && !pw(tau - 2) && pw(tau - 1) && !pw(tau + 1) && same(tau - 2)
                    };
                    if blocked || pulse_tol {
                        continue;
                    }
                    if qc_only {
                        m.e.add(&[!ds, !sel, !direct]);
                        continue;
                    }
                    if req {
                        if p.qc {
                            m.e.add(&[!ds, !sel, direct, qc]);
                        } else {
                            m.e.add(&[!ds, !sel, direct]);
                        }
                    } else {
                        m.e.add(&[!ds, !sel, !direct]);
                        // --sound: a retracted piston that stays retracted may hold QC power (no block
                        // update reaches it: a BUD); an extended one that retracts may not
                        let stays_retracted = matches!(ph.frame(tau - 1).get(&c).map(|s| mk(s)), Some(MK::Piston { ext: false, .. })) && ph.frame(tau - 1).get(&c) == ph.frame(tau).get(&c);
                        // (never at rest: placing the door updates every block, so a QC-armed piston at
                        // rest fires; vjeux: no QC-armed pistons at rest)
                        if !(SOUND.load(std::sync::atomic::Ordering::Relaxed) && stays_retracted && k >= 0) {
                            m.e.add(&[!ds, !sel, !qc]);
                        } else {
                            // a QC hold survives only while no block update reaches the piston
                            let upd = m.update_at(tr, k, c);
                            m.e.add(&[!ds, !sel, !qc, !upd]);
                        }
                    }
                  }
                }
            }
        }
    }
    m.e.guard.clear();
    if diag {
        let g = m.gsel("scheduled ticks / nothing pending at the end");
        m.e.guard.push(g);
    }
    // ---- scheduled ticks: exactly their causes; nothing pending after the horizon
    let keys: Vec<Key> = m.causes.keys().copied().collect();
    for key in keys {
        let cs = m.causes[&key].clone();
        let (tr, s, c) = key;
        let any = m.e.or(&cs);
        let sv = m.sched(tr, s, c);
        m.e.eq(sv, any);
    }
    let ekeys: Vec<Key> = m.sched_e.keys().copied().collect();
    for key in ekeys {
        let v = m.sched_e[&key];
        let cs = m.causes_e.get(&key).cloned().unwrap_or_default();
        let any = m.e.or(&cs);
        m.e.eq(v, any);
    }
    let skeys: Vec<Key> = m.sched.keys().copied().collect();
    for key in skeys {
        if !m.causes.contains_key(&key) {
            let v = m.sched[&key];
            m.e.add(&[!v]);
        }
    }
    m.e.guard.clear();
    if diag {
        let g = m.gsel("each trace ends where the other starts");
        m.e.guard.push(g);
    }
    // the rest state of fixed (kept) wiring is its design state
    for (c, st) in p.fixed_state.clone() {
        let Some(k) = p.fixed.get(&c).copied() else { continue };
        let on = |key: &str| prop(&st, key) == Some("true");
        let f0 = m.e.f();
        match k {
            Kind::Bulb => {
                let l = Model::get(&m.blit, (0, -1, c), f0);
                let v = m.e.konst(on("lit"));
                m.e.eq(l, v);
            }
            Kind::Rep(..) | Kind::Cmp(_) => {
                let o = m.out(0, -1, c);
                let v = m.e.konst(on("powered"));
                m.e.eq(o, v);
            }
            Kind::Torch | Kind::WTorch(_) | Kind::Lamp => {
                let o = m.out(0, -1, c);
                let v = m.e.konst(on("lit"));
                m.e.eq(o, v);
            }
            _ => {}
        }
    }
    // ---- each trace ends where the next starts (period 1: open <-> close; --period2: open1 ->
    // close1 -> open2 -> close2 -> open1, so a T flip-flop may alternate between cycles)
    for c in p.free.clone() {
        for a in 0..nt {
            let b = (a + 1) % nt;
            let x = m.out(a, h, c);
            let y = m.out(b, -1, c);
            m.e.eq(x, y);
            let f = m.e.f();
            let x = Model::get(&m.blit, (a, h, c), f);
            let y = Model::get(&m.blit, (b, -1, c), f);
            m.e.eq(x, y);
        }
    }
    m.e.guard.clear();
    if check_mode {
        for tr in 0..nt {
            let kk = p.rec.phases[tr % 2].k();
            for k in 1..=h {
                let want = k.min(kk);
                for cl in 0..p.ncl {
                    let l = m.warp[tr][cl][k as usize].iter().find(|x| x.0 == want).map(|x| x.1).unwrap_or(m.e.f());
                    m.e.add(&[l]);
                }
                let l = m.e.t();
                m.e.add(&[l]);
            }
        }
    }
    m
}

// ---------------------------------------------------------------- decode, judge, CEGAR

pub struct Sol {
    pub design: Design,
    pub kinds: BTreeMap<C2, Kind>,
    pub lever: usize,
    pub warp: [String; 2],
    pub wiring: usize,
    /// The alternative chosen in each group (`--menu`), "group: label".
    pub alts: Vec<String>,
}

pub fn decode(m: &Model, name: &str) -> Sol {
    let p = m.p;
    let z = p.z;
    let f = m.e.f();
    let mut kinds = BTreeMap::new();
    let mut d = Design { name: name.to_string(), ..Default::default() };
    for (pos, s) in &p.rec.rest {
        let logs = LOG_DOORS.load(std::sync::atomic::Ordering::Relaxed);
        let st = if logs && short(s) == "stone" { "minecraft:oak_log[axis=y]".to_string() } else { s.clone() };
        d.blocks.insert(*pos, st);
    }
    for c in &p.free {
        let k = p.kinds[c].iter().copied().find(|k| m.e.val(m.kl[&(*c, *k)])).unwrap_or(Kind::Empty);
        if k == Kind::Empty {
            continue;
        }
        kinds.insert(*c, k);
        let out = m.e.val(m.out(0, -1, *c));
        let lvl = if k == Kind::Leaves { (2..=7u8).rev().find(|j| m.leaves.get(&(0, -1, *c, *j)).is_some_and(|l| m.e.val(*l))).unwrap_or(1) } else { (m.lo()..=15).rev().find(|j| m.e.val(m.lvl(0, -1, *c, *j))).unwrap_or(0) };
        let a = m.e.val(Model::get(&m.blit, (0, -1, *c), f));
        let b = m.e.val(Model::get(&m.bpow, (0, -1, *c), f)) || m.e.val(Model::get(&m.npow, (0, -1, *c), f));
        d.blocks.insert([c.0, c.1, z], k.state(out, lvl, a, b));
    }
    let lever = (0..p.levers.len()).find(|i| m.e.val(m.lever[*i])).unwrap_or(0);
    let lc = &p.levers[lever];
    let lstate = if p.lever_on_rest { lc.state.replace("powered=false", "powered=true") } else { lc.state.clone() };
    d.blocks.insert(lc.pos, lstate);
    d.ports.insert("door".into(), p.rec.door);
    d.ports.insert("lever".into(), lc.pos);
    let warp = |tr: usize| -> String {
        // per cluster: each recorded motion tick tau that plays at another SAT tick k (tau@k)
        let mut parts = Vec::new();
        for cl in 0..p.ncl {
            let mut v = Vec::new();
            for k in 1..=p.h {
                let tau = m.warp[tr][cl][k as usize].iter().find(|x| m.e.val(x.1)).map(|x| x.0).unwrap_or(-1);
                if tau != k && tau >= 1 && !p.static_tick_c(tr, tau, cl) {
                    v.push(format!("{tau}@{k}"));
                }
            }
            if !v.is_empty() {
                parts.push(format!("c{cl}: {}", v.join(" ")));
            }
        }
        if parts.is_empty() { "as recorded".into() } else { parts.join("; ") }
    };
    let wiring = kinds.len();
    let alts: Vec<String> = p
        .rec
        .groups
        .iter()
        .enumerate()
        .filter_map(|(gi, g)| {
            let a = m.alt_sel[gi].iter().position(|l| m.e.val(*l))?;
            (a > 0).then(|| format!("{} {}: {}", ["open", "close"][g.tr], g.name, g.alts[a].0))
        })
        .collect();
    Sol { design: d, kinds, lever, warp: [warp(0), warp(1)], wiring, alts }
}

pub fn render(p: &Prob, s: &Sol) -> String {
    let (x0, y0, x1, y1) = p.bx;
    let mut out = String::new();
    for y in (y0 - 1..=y1 + 1).rev() {
        let mut row = format!("{y:>3} ");
        for x in x0 - 1..=x1 + 1 {
            let c = (x, y);
            let lp = p.levers[s.lever].pos;
            let ch = if lp == [x, y, p.z] {
                'L'
            } else if let Some(k) = s.kinds.get(&c) {
                match k {
                    Kind::Block => '#',
                    Kind::Dust => '+',
                    Kind::Dot => '*',
                    Kind::Rep(d, f) => {
                        let arrow = if *f == E { ['<', '{', '«', '⇐'] } else { ['>', '}', '»', '⇒'] };
                        arrow[(*d - 1) as usize]
                    }
                    Kind::Obs(f) => match *f {
                        E => 'e',
                        W => 'w',
                        U => 'u',
                        _ => 'd',
                    },
                    Kind::Torch => 'i',
                    Kind::WTorch(f) => if *f == E { 'j' } else { 'l' },
                    Kind::Note => 'N',
                    Kind::Bulb => 'B',
                    Kind::Cmp(f) => if *f == E { 'c' } else { 'C' },
                    Kind::Lamp => 'Q',
                    Kind::Trap => 't',
                    Kind::Dropper => 'D',
                    Kind::Obsid => 'X',
                    Kind::Glass => 'g',
                    Kind::Leaves => '%',
                    Kind::UPiston(f) => match *f { E => '1', W => '2', U => '3', _ => '4' },
                    Kind::RBlock => 'R',
                    Kind::TrapTop => 'T',
                    Kind::Empty => '.',
                }
            } else if let Some(st) = p.rec.phases[0].frames[0].get(&c) {
                match mk(st) {
                    MK::Piston { .. } => 'P',
                    MK::Cond => 'H',
                    _ => 'o',
                }
            } else if p.mech.contains(&c) {
                ':'
            } else if p.is_free(c) {
                '.'
            } else {
                ' '
            };
            row.push(ch);
        }
        out.push_str(&row);
        out.push('\n');
    }
    out
}

pub struct RunOpts {
    pub iters: usize,
    /// Most re-solves spent shrinking an UNSAT core to the pistons that conflict (0: raw core).
    pub core_tries: usize,
    pub minimize: bool,
    pub max_blocks: Option<usize>,
    pub extra: u32,
    pub out: Option<std::path::PathBuf>,
    pub name: String,
    pub version: String,
}

/// Judge a candidate: Ok(report line) or Err(reason).
/// Note blocks take their instrument from the block below: let the game compute it.
/// Write every dust in the shape the game gives it when placed (its connections), so placing the
/// design changes no wire state an observer could see.
fn normalize_wires(w: &mut Worker, d: &mut Design) -> Result<()> {
    let wires: Vec<Pos> = d.blocks.iter().filter(|(_, s)| short(s) == "redstone_wire").map(|(p, _)| *p).collect();
    if wires.is_empty() {
        return Ok(());
    }
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    w.set_blocks(sb, &d.block_list(), PlaceMode::Raw)?;
    w.update(sb, &wires, "shape")?;
    for p in &wires {
        let r = w.get_region(sb, *p, *p)?;
        if let Some(s) = r.get(*p) {
            if short(s) != "redstone_wire" {
                continue;
            }
            let power = prop(&d.blocks[p], "power").unwrap_or("0").to_string();
            let side = |k: &str| prop(s, k).unwrap_or("none").to_string();
            d.blocks.insert(*p, format!("minecraft:redstone_wire[east={},north={},power={power},south={},west={}]", side("east"), side("north"), side("south"), side("west")));
        }
    }
    let _ = w.drop_sandbox(sb);
    Ok(())
}

fn normalize_notes(w: &mut Worker, d: &mut Design) -> Result<()> {
    let notes: Vec<Pos> = d.blocks.iter().filter(|(_, s)| short(s) == "note_block").map(|(p, _)| *p).collect();
    if notes.is_empty() {
        return Ok(());
    }
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    w.set_blocks(sb, &d.block_list(), PlaceMode::Raw)?;
    w.update(sb, &notes, "shape")?;
    for p in &notes {
        let r = w.get_region(sb, *p, *p)?;
        if let Some(s) = r.get(*p) {
            let powered = prop(&d.blocks[p], "powered").unwrap_or("false").to_string();
            let inst = prop(s, "instrument").unwrap_or("harp").to_string();
            d.blocks.insert(*p, format!("minecraft:note_block[instrument={inst},note=0,powered={powered}]"));
        }
    }
    let _ = w.drop_sandbox(sb);
    Ok(())
}

/// A clock candidate (the judge: "still moving"): replay it and return the free wiring cells whose
/// state still changes in the last 16 recorded ticks of a phase (None: no such cell, or too many).
fn clock_cells(w: &mut Worker, p: &Prob, d: &Design) -> Option<Vec<C2>> {
    let r = record_wired(w, d, 120).ok()?;
    let mut cells: BTreeSet<C2> = BTreeSet::new();
    for ph in &r.full {
        let n = ph.len();
        if n < 20 {
            continue;
        }
        for j in n - 16..n {
            let (a, b) = (&ph[j - 1], &ph[j]);
            for c in a.keys().chain(b.keys()) {
                if p.kinds.contains_key(c) && a.get(c) != b.get(c) {
                    cells.insert(*c);
                }
            }
        }
    }
    (!cells.is_empty() && cells.len() <= 10).then(|| cells.into_iter().collect())
}

/// `--local-nogood`: replay the failing candidate, find the first recorded tick where its
/// mechanism differs from the recording, and return the cells within 3 of that difference (and of
/// the pistons next to it). None when the run cannot be compared (then the whole design is blocked).
fn local_region(w: &mut Worker, p: &Prob, d: &Design) -> Option<HashSet<C2>> {
    let r = record_wired(w, d, 80).ok()?;
    for tr in 0..2 {
        let (a, b) = (&p.rec.phases[tr], &r.phases[tr]);
        let n = a.frames.len().max(b.frames.len());
        let logs = LOG_DOORS.load(std::sync::atomic::Ordering::Relaxed);
        let norm = |f: &HashMap<C2, String>| -> HashMap<C2, String> { f.iter().map(|(c, s)| (*c, if logs && short(s) == "stone" { "minecraft:oak_log[axis=y]".to_string() } else { s.clone() })).collect() };
        for t in 0..n as i32 {
            // (--log-doors: the candidate's stone is oak log)
            let (fa, fb) = (norm(a.frame(t)), norm(b.frame(t)));
            let (fa, fb) = (&fa, &fb);
            let diff: Vec<C2> = fa.keys().chain(fb.keys()).copied().filter(|c| fa.get(c) != fb.get(c)).collect();
            if diff.is_empty() {
                continue;
            }
            // the misbehaving pistons: diff cells holding a piston base before or after (else every
            // diff cell); their neighborhood (radius 2, QC cell included) holds the cause
            let is_base = |fr: &HashMap<C2, String>, c: &C2| fr.get(c).is_some_and(|s| matches!(mk(s), MK::Piston { .. }));
            let prev = a.frame(t - 1);
            let mut centers: HashSet<C2> = diff.iter().copied().filter(|c| is_base(prev, c) || is_base(fa, c) || is_base(fb, c)).collect();
            if centers.is_empty() {
                centers = diff.iter().copied().collect();
            }
            let mut region = HashSet::new();
            for c in &centers {
                for dx in -2..=2 {
                    for dy in -2..=3 {
                        region.insert((c.0 + dx, c.1 + dy));
                    }
                }
            }
            return Some(region);
        }
    }
    None
}

/// `--keep-ideal`: replay the candidate with the plan's kept ideal inputs (and its lever flips moved
/// to the candidate's lever), like `door ideal`: 4 positions x 2 cycles.
fn ideal_judge(w: &mut Worker, p: &Prob, d: &Design) -> std::result::Result<(u32, u32), String> {
    let plan = p.plan.as_ref().ok_or("no plan")?;
    let keep = KEEP_IDEAL.lock().unwrap().clone();
    let lever = d.blocks.iter().find(|(_, s)| short(s) == "lever").map(|(pos, _)| *pos);
    let mut evs = Vec::new();
    for open in [true, false] {
        for (t, pos, st) in plan.resolved(open).map_err(|e| e.to_string())? {
            if st.as_deref() == Some("@use") {
                if let Some(lp) = lever {
                    evs.push((open, t, lp, st.clone()));
                }
            } else if keep.contains(&(pos[0], pos[1])) {
                evs.push((open, t, pos, st.clone()));
            }
        }
    }
    let s = crate::ideal::Schedule::from_events(d.clone(), &evs);
    let opts = crate::ideal::IdealOptions { cycles: 2, offsets: crate::ideal::offsets(0), trace: None };
    crate::ideal::run(w, &s, &BTreeMap::new(), &opts).map_err(|e| e.to_string())
}

fn judge(w: &mut Worker, sb: u32, d: &Design, extra: u32) -> std::result::Result<String, String> {
    let mut o = crate::VerifyOptions::default();
    if extra > 0 {
        o.offsets = crate::search::judge_offsets(extra);
    }
    match crate::verify_in(w, sb, d, &o) {
        Ok(r) => Ok(format!("{} blocks, open {} close {}", r.blocks, r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0), r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0))),
        Err(e) => Err(format!("{e:#}")),
    }
}

/// The CEGAR loop: solve, judge in the real simulator, block failures, shrink the wiring.
pub fn run(p: &Prob, o: &RunOpts) -> Result<Option<Design>> {
    run_log(p, o, &|s: String| println!("{s}"))
}

/// [`run`] with a logger (batch runs prefix and collect the lines).
pub fn run_log(p: &Prob, o: &RunOpts, log: &(dyn Fn(String) + Sync)) -> Result<Option<Design>> {
    let t0 = std::time::Instant::now();
    let mut m = build(p, false);
    log(format!(
        "[{:6.1}s] encoded: {} free cells, {} lever candidates, offsets 0..={}, horizon {} ticks, {} vars, {} clauses",
        t0.elapsed().as_secs_f64(),
        p.free.len(),
        p.levers.len(),
        p.dmax,
        p.h,
        m.e.vars(),
        m.e.clauses
    ));
    let used: Vec<Lit> = m.used.iter().map(|u| u.1).collect();
    let tot = m.e.totalizer(&used);
    // --hint: steer the solver's decisions toward the source design's wiring (a wired source:
    // its first model is found at once and the search continues from there)
    if HINT.load(std::sync::atomic::Ordering::Relaxed) && !p.rec.wiring.is_empty() {
        use rustsat::solvers::PhaseLit;
        let mut n = 0;
        for c in &p.free {
            let want = p.rec.wiring.get(c).and_then(|s| Kind::of_state(s)).unwrap_or(Kind::Empty);
            for k in &p.kinds[c] {
                let l = m.kl[&(*c, *k)];
                let _ = m.e.s.phase_lit(if *k == want { l } else { !l });
            }
            n += (want != Kind::Empty) as usize;
        }
        if let Some((lp, ls)) = &p.rec.lever {
            for (i, lc) in p.levers.iter().enumerate() {
                let same = lc.pos == *lp && prop(&lc.state, "face") == prop(ls, "face") && prop(&lc.state, "facing") == prop(ls, "facing");
                let _ = m.e.s.phase_lit(if same { m.lever[i] } else { !m.lever[i] });
            }
        }
        log(format!("--hint: decisions steered toward the source wiring ({n} cells)"));
        // solve once with the source wiring assumed: the solver keeps that full assignment as its
        // saved phases for the searches that follow
        let mut a: Vec<Lit> = Vec::new();
        for c in &p.free {
            let want = p.rec.wiring.get(c).and_then(|s| Kind::of_state(s)).unwrap_or(Kind::Empty);
            if let Some(l) = m.kl.get(&(*c, want)) {
                a.push(*l);
            }
        }
        a.extend(m.sels.iter().map(|s| s.2));
        let ts = std::time::Instant::now();
        let r = m.e.s.solve_assumps(&a)?;
        log(format!("--hint: the source wiring is {} in this model ({:.1}s)", if r == SolverResult::Sat { "accepted" } else { "REJECTED" }, ts.elapsed().as_secs_f64()));
    }
    let mut bound = o.max_blocks.unwrap_or(used.len()).min(used.len());
    let cfg = mctas::sim::WorkerConfig { version: o.version.clone(), heap_mb: 1024, ..Default::default() };
    // The judge's JVM starts with the first model (an UNSAT run never holds a worker).
    let mut ws: Option<(Worker, u32)> = None;
    let mut best: Option<(Design, usize)> = None;
    let mut fails = 0usize;
    // --retime needs the plan itself to work in redsim (else its replays prove nothing)
    let retime = RETIME.load(std::sync::atomic::Ordering::Relaxed)
        && p.plan.as_ref().is_some_and(|plan| match crate::ideal_rs::run(plan, plan.vars(), &crate::VerifyOptions::default().offsets, 2, false) {
            Ok(_) => true,
            Err(e) => {
                log(format!("--retime off: the plan itself fails in redsim ({})", e.chars().take(120).collect::<String>()));
                false
            }
        });
    for it in 0..o.iters {
        // SATWIRE_RETIME_SELFTEST: solve once without the piston requirements and print the plan
        // retimed with that (arbitrary) warp next to the original, and the redsim verdict
        if it == 0 && retime && std::env::var("SATWIRE_RETIME_SELFTEST").is_ok() {
            let neg: Vec<Lit> = m.sels.iter().map(|s| !s.2).collect();
            if m.e.s.solve_assumps(&neg)? == SolverResult::Sat {
                let plan = p.plan.as_ref().unwrap();
                let s2 = retimed_plan(&m, plan)?;
                for open in [true, false] {
                    let a = plan.resolved(open)?;
                    let b = s2.resolved(open)?;
                    let fmt = |v: &[(u32, Pos, Option<String>)]| v.iter().map(|(t, p, s)| format!("{t}:{},{},{}{}", p[0], p[1], p[2], if s.is_some() { "+" } else { "-" })).collect::<Vec<_>>().join(" ");
                    log(format!("selftest {}: plan    {}", if open { "open" } else { "close" }, fmt(&a)));
                    log(format!("selftest {}: retimed {}", if open { "open" } else { "close" }, fmt(&b)));
                }
                let v = crate::ideal_rs::run(&s2, &BTreeMap::new(), &crate::VerifyOptions::default().offsets, 2, false);
                log(format!("selftest: retimed plan in redsim: {v:?}"));
            }
        }
        let mut assumps: Vec<Lit> = if bound < tot.len() { vec![!tot[bound]] } else { vec![] };
        assumps.extend(m.gsels.iter().map(|g| g.1));
        let nb = assumps.len();
        assumps.extend(m.sels.iter().map(|s| s.2));
        let ts = std::time::Instant::now();
        let r = m.e.s.solve_assumps(&assumps)?;
        let dt = ts.elapsed().as_secs_f64();
        if r != SolverResult::Sat {
            match &best {
                None => {
                    log(format!("[{:6.1}s] UNSAT (solve {dt:.1}s): no wiring exists within this palette, box and model (wiring <= {bound}, {fails} judged designs blocked)", t0.elapsed().as_secs_f64()));
                    // which pistons' power requirements conflict: shrink the core by deletion
                    let core = m.e.s.core().unwrap_or_default();
                    let mut cs: Vec<(usize, C2, Lit)> = m.sels.iter().copied().filter(|s| core.contains(&s.2) || core.contains(&!s.2)).collect();
                    let base: Vec<Lit> = assumps[..nb].to_vec();
                    if base.is_empty() || true {
                        let mut i = 0;
                        let mut tries = 0;
                        while i < cs.len() && tries < o.core_tries {
                            tries += 1;
                            let mut a = base.clone();
                            a.extend(cs.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, s)| s.2));
                            match m.e.s.solve_assumps(&a) {
                                Ok(SolverResult::Unsat) => {
                                    let c2 = m.e.s.core().unwrap_or_default();
                                    let mut keep: Vec<(usize, C2, Lit)> = cs.iter().copied().enumerate().filter(|(j, s)| *j != i && (c2.contains(&s.2) || c2.contains(&!s.2))).map(|(_, s)| s).collect();
                                    if keep.is_empty() {
                                        keep = cs.iter().copied().enumerate().filter(|(j, _)| *j != i).map(|(_, s)| s).collect();
                                    }
                                    cs = keep;
                                }
                                _ => i += 1,
                            }
                        }
                    }
                    let names: Vec<String> = cs.iter().map(|s| format!("{} piston ({},{})", ["open", "close"][s.0], s.1 .0, s.1 .1)).collect();
                    if names.is_empty() {
                        log("  core: no piston requirement needed (the wiring/stability constraints alone are UNSAT)".into());
                    } else {
                        log(format!("  core (pistons whose recorded power cannot be produced together): {}", names.join(", ")));
                    }
                    if !m.gsels.is_empty() {
                        // which constraint groups conflict (with every piston requirement on)
                        let bound_a: Vec<Lit> = assumps[..nb - m.gsels.len()].to_vec();
                        let sel_a: Vec<Lit> = m.sels.iter().map(|s| s.2).collect();
                        let mut gs: Vec<(String, Lit)> = m.gsels.clone();
                        let mut i = 0;
                        let mut tries = 0;
                        while i < gs.len() && tries < 80 {
                            tries += 1;
                            let mut a = bound_a.clone();
                            a.extend(sel_a.iter().copied());
                            a.extend(gs.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, g)| g.1));
                            if m.e.s.solve_assumps(&a).ok() == Some(SolverResult::Unsat) {
                                gs.remove(i);
                            } else {
                                i += 1;
                            }
                        }
                        log(format!("  conflicting constraint groups (with all piston requirements): {}", gs.iter().map(|g| g.0.clone()).collect::<Vec<_>>().join(" | ")));
                    }
                }
                Some((_, b)) => log(format!("[{:6.1}s] no wiring with <= {bound} blocks (solve {dt:.1}s): {b} wiring blocks is minimal in this model", t0.elapsed().as_secs_f64())),
            }
            break;
        }
        let mut s = decode(&m, &o.name);
        if ws.is_none() {
            let mut w = Worker::spawn(&cfg)?;
            let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
            ws = Some((w, sb));
        }
        let (w, sb) = ws.as_mut().map(|(w, sb)| (w, *sb)).unwrap();
        normalize_notes(w, &mut s.design)?;
        normalize_wires(w, &mut s.design)?;
        log(format!(
            "[{:6.1}s] iter {it}: SAT (solve {dt:.1}s): {} wiring blocks; timing open: {}; close: {}{}\n{}",
            t0.elapsed().as_secs_f64(),
            s.wiring,
            s.warp[0],
            s.warp[1],
            if s.alts.is_empty() { String::new() } else { format!("; menu: {}", s.alts.join(", ")) },
            render(p, &s)
        ));
        // redsim (the judge's checks on the game-verified fast simulator) screens first
        let offs = crate::VerifyOptions::default().offsets;
        // --retime: the candidate's timing must keep the mechanism working with ideal inputs
        if retime {
            if let Some(plan) = p.plan.as_ref() {
                let s2 = retimed_plan(&m, plan)?;
                if let Err(e) = crate::ideal_rs::run(&s2, &BTreeMap::new(), &offs, 2, false) {
                    let close: Vec<usize> = (0..m.pairs.len())
                        .filter(|i| {
                            let (tr, x, a, y, b, d) = m.pairs[*i];
                            m.start_val(tr, y, b) - m.start_val(tr, x, a) < d
                        })
                        .collect();
                    let cl: Vec<Lit> = if close.is_empty() {
                        // no too-close pair: block this exact timing
                        let mut v = Vec::new();
                        for tr in 0..2 {
                            for c in 0..p.ncl {
                                for k in 1..=p.h {
                                    if let Some(x) = m.warp[tr][c][k as usize].iter().find(|x| m.e.val(x.1)) {
                                        v.push(!x.1);
                                    }
                                }
                            }
                        }
                        v
                    } else {
                        close.iter().map(|i| !m.close_lit(*i)).collect()
                    };
                    fails += 1;
                    log(format!("  TIMING FAILS (retimed ideal plan: {}): blocked {} too-close pairs", e.chars().take(160).collect::<String>(), close.len()));
                    m.e.add(&cl);
                    continue;
                }
                log("  retimed ideal plan WORKS".into());
            }
        }
        let partial = p.plan.is_some() && !KEEP_IDEAL.lock().unwrap().is_empty();
        let screened = if partial {
            // --keep-ideal: the candidate wiring + the kept ideal inputs, replayed like `door ideal`
            ideal_judge(w, p, &s.design).map(|(a, b)| format!("{} blocks, open {a} close {b} (with the kept ideal inputs)", s.design.blocks.len()))
        } else {
            match crate::redsim_screen::screen(&s.design, &offs, 3, 400, true) {
                crate::redsim_screen::Screen::Fail(msg) => Err(format!("redsim: {msg}")),
                _ => judge(w, sb, &s.design, 0),
            }
        };
        match screened {
            Ok(rep) => {
                log(format!("  JUDGE OK (4 positions): {rep}"));
                if let Some(path) = &o.out {
                    s.design.save(path)?;
                }
                best = Some((s.design.clone(), s.wiring));
                if !o.minimize || s.wiring == 0 {
                    break;
                }
                bound = s.wiring - 1;
            }
            Err(e) => {
                fails += 1;
                log(format!("  JUDGE FAILS: {}", e.chars().take(300).collect::<String>()));
                if let Some(path) = &o.out {
                    let _ = s.design.save(&path.with_extension("fail.txt"));
                }
                let mut cl: Vec<Lit> = Vec::new();
                // a clock (the run never settles): block only the wiring cells that keep changing
                // at the end of the run (the loop oscillates whatever the rest of the wiring is)
                let clock = if e.contains("still moving") { clock_cells(w, p, &s.design) } else { None };
                if let Some(cells) = &clock {
                    for c in cells {
                        let k = s.kinds.get(c).copied().unwrap_or(Kind::Empty);
                        cl.push(!m.kl[&(*c, k)]);
                    }
                    log(format!("  clock nogood: {} cells {:?}", cells.len(), cells));
                    m.e.add(&cl);
                    continue;
                }
                // --local-nogood (exact timing): block only the candidate's choices around the
                // first piston that misbehaves in its real run (the failure is local)
                let local = if LOCAL_NOGOOD.load(std::sync::atomic::Ordering::Relaxed) && p.dmax == 0 && p.skip == 0 { local_region(w, p, &s.design) } else { None };
                match &local {
                    Some(region) => {
                        for c in &p.free {
                            if region.contains(c) {
                                let k = s.kinds.get(c).copied().unwrap_or(Kind::Empty);
                                cl.push(!m.kl[&(*c, k)]);
                            }
                        }
                        let lp = p.levers[s.lever].pos;
                        if region.contains(&(lp[0], lp[1])) || region.contains(&(p.levers[s.lever].att[0], p.levers[s.lever].att[1])) {
                            cl.push(!m.lever[s.lever]);
                        }
                        log(format!("  local nogood: {} literals", cl.len()));
                    }
                    None => {
                        for c in &p.free {
                            let k = s.kinds.get(c).copied().unwrap_or(Kind::Empty);
                            cl.push(!m.kl[&(*c, k)]);
                        }
                        cl.push(!m.lever[s.lever]);
                    }
                }
                m.e.add(&cl);
            }
        }
    }
    let Some((d, _)) = best else { return Ok(None) };
    let Some((w, sb)) = ws.as_mut() else { return Ok(Some(d)) };
    let _ = w.drop_sandbox(*sb);
    let partial = p.plan.is_some() && !KEEP_IDEAL.lock().unwrap().is_empty();
    if o.extra > 0 && !partial {
        let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        match judge(w, sb, &d, o.extra) {
            Ok(rep) => log(format!("JUDGE OK with --extra-offsets {}: {rep}", o.extra)),
            Err(e) => log(format!("JUDGE FAILS with --extra-offsets {}: {}", o.extra, e.chars().take(300).collect::<String>())),
        }
    }
    Ok(Some(d))
}

/// The model's state of the fixed wiring cell c at tick k against the game's block state `real`:
/// Some((model, game)) when they differ (None: equal or not comparable).
fn wiring_div(m: &Model, tr: usize, k: i32, c: C2, kind: Kind, real: &str) -> Option<(String, String)> {
    let f0 = m.e.f();
    let rb = |key: &str| prop(real, key) == Some("true");
    let (model, rv): (String, String) = match kind {
        Kind::Rep(..) | Kind::Obs(_) | Kind::Cmp(_) => (m.e.val(m.out(tr, k, c)).to_string(), rb("powered").to_string()),
        Kind::Torch | Kind::WTorch(_) | Kind::Lamp => (m.e.val(m.out(tr, k, c)).to_string(), rb("lit").to_string()),
        Kind::UPiston(_) => (m.e.val(m.out(tr, k, c)).to_string(), rb("extended").to_string()),
        Kind::Dust | Kind::Dot => {
            let lvl = (m.lo()..=15).rev().find(|j| m.e.val(m.lvl(tr, k, c, *j))).unwrap_or(0);
            (lvl.to_string(), prop(real, "power").unwrap_or("0").to_string())
        }
        Kind::Note | Kind::Trap | Kind::TrapTop => (m.e.val(Model::get(&m.npow, (tr, k, c), f0)).to_string(), rb("powered").to_string()),
        Kind::Dropper => (m.e.val(Model::get(&m.npow, (tr, k, c), f0)).to_string(), rb("triggered").to_string()),
        Kind::Bulb => (m.e.val(Model::get(&m.blit, (tr, k, c), f0)).to_string(), rb("lit").to_string()),
        Kind::Leaves => {
            let d = (2..=7u8).rev().find(|j| m.leaves.get(&(tr, k, c, *j)).is_some_and(|l| m.e.val(*l))).unwrap_or(1);
            (d.to_string(), prop(real, "distance").unwrap_or("?").to_string())
        }
        _ => return None,
    };
    (model != rv).then_some((model, rv))
}

/// `--explain <candidate>`: the candidate's wiring fixed on the TARGET recording (the run the solver
/// believed in) against the candidate's REAL run (`real`, recorded in the game): per trace, the
/// first tick where the mechanism differs, the wiring states the model predicts vs the game's up to
/// that tick, and the power of the pistons at that tick (model direct/qc, game, required).
pub fn explain(p: &Prob, real: &Rec) -> Result<()> {
    let mut m = build(p, false);
    println!("explain: {} vars, {} clauses", m.e.vars(), m.e.clauses);
    let mut assumps: Vec<Lit> = m.sels.iter().map(|s| s.2).collect();
    assumps.extend(m.uo_sels.iter().map(|s| s.4));
    if m.e.s.solve_assumps(&assumps)? != SolverResult::Sat {
        let core = m.e.s.core().unwrap_or_default();
        let cs: Vec<String> = m.sels.iter().filter(|s| core.contains(&s.2) || core.contains(&!s.2)).map(|s| format!("{} piston ({},{})", ["open", "close"][s.0], s.1 .0, s.1 .1)).collect();
        let us: Vec<String> = m.uo_sels.iter().filter(|s| core.contains(&s.4) || core.contains(&!s.4)).map(|s| format!("{} t{} piston ({},{}) {}", ["open", "close"][s.0], s.1, s.2 .0, s.2 .1, s.3)).collect();
        println!("explain: the model REJECTS the candidate on the target recording; core: {}; update-order clauses: {}", cs.join(", "), us.join(", "));
        return Ok(());
    }
    for tr in 0..2 {
        let (a, b) = (&p.rec.phases[tr], &real.phases[tr]);
        let n = a.frames.len().max(b.frames.len()) as i32;
        let mut first = None;
        for t in 0..n {
            // (--log-doors: the candidate's stone is oak log)
            let logs = LOG_DOORS.load(std::sync::atomic::Ordering::Relaxed);
            let norm = |f: &HashMap<C2, String>| -> HashMap<C2, String> { f.iter().map(|(c, s)| (*c, if logs && short(s) == "stone" { "minecraft:oak_log[axis=y]".to_string() } else { s.clone() })).collect() };
            let (fa, fb) = (norm(a.frame(t)), norm(b.frame(t)));
            let (fa, fb) = (&fa, &fb);
            let mut diff: Vec<String> = fa.keys().chain(fb.keys()).copied().collect::<BTreeSet<C2>>().into_iter().filter(|c| fa.get(c) != fb.get(c)).map(|c| format!("{c:?} target {} game {}", fa.get(&c).map_or("air", |s| short(s)), fb.get(&c).map_or("air", |s| short(s)))).collect();
            if !diff.is_empty() {
                diff.truncate(6);
                println!("explain {}: mechanism first differs after tick {t}: {}", ["open", "close"][tr], diff.join("; "));
                first = Some(t);
                break;
            }
        }
        let last = first.unwrap_or(n.min(p.h)).min(p.h);
        if first.is_none() {
            println!("explain {}: the mechanism follows the target recording", ["open", "close"][tr]);
        }
        let full = real.full.get(tr).cloned().unwrap_or_default();
        let mut shown = 0;
        for k in 0..=last {
            let Some(fr) = full.get(k as usize) else { break };
            let mut fixed: Vec<(C2, Kind)> = p.fixed.iter().map(|(c, k)| (*c, *k)).collect();
            fixed.sort();
            let divs: Vec<String> = fixed.iter().filter_map(|(c, kind)| fr.get(c).and_then(|real| wiring_div(&m, tr, k, *c, *kind, real)).map(|(mv, gv)| format!("{c:?} {kind:?}: model {mv} game {gv}"))).collect();
            if !divs.is_empty() && shown < 4 {
                println!("explain {} t{k}: wiring differs: {}", ["open", "close"][tr], divs.join("; "));
                shown += 1;
            }
        }
        if let Some(t) = first {
            // the pistons' power during the ticks up to the divergence: model vs game
            for k in (t - 2).max(1)..=t {
                let mut rows = Vec::new();
                for ch in m.checks.iter().filter(|ch| ch.tr == tr && ch.k == k) {
                    let (dir, qc) = (m.e.val(ch.direct), m.e.val(ch.qc));
                    let game = b.pow(k).get(&ch.c).copied();
                    rows.push(format!("{:?} req {} model direct {} qc {} game {:?}", ch.c, ch.req, dir, qc, game));
                }
                // pistons whose power the game saw differently from the target recording
                for (c, v) in b.pow(k) {
                    if a.pow(k).get(c) != Some(v) {
                        rows.push(format!("{c:?} game power {v} target {:?}", a.pow(k).get(c)));
                    }
                }
                if !rows.is_empty() {
                    println!("explain {} t{k} pistons: {}", ["open", "close"][tr], rows.join("; "));
                }
            }
        }
    }
    Ok(())
}

/// `--check`: the source design's own wiring fixed; print where the model's piston power
/// differs from the recorded one.
pub fn check(p: &Prob) -> Result<()> {
    let mut m = build(p, true);
    println!("check: {} vars, {} clauses", m.e.vars(), m.e.clauses);
    let assumps: Vec<Lit> = m.gsels.iter().map(|g| g.1).collect();
    let r = m.e.s.solve_assumps(&assumps)?;
    if r != SolverResult::Sat {
        println!("check: the model REJECTS the source wiring (dynamics/stability UNSAT)");
        let core = m.e.s.core().unwrap_or_default();
        let mut cs: Vec<(String, Lit)> = m.gsels.iter().filter(|g| core.contains(&g.1) || core.contains(&!g.1)).cloned().collect();
        let mut i = 0;
        let mut tries = 0;
        while i < cs.len() && tries < 60 {
            tries += 1;
            let a: Vec<Lit> = cs.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, g)| g.1).collect();
            if m.e.s.solve_assumps(&a)? == SolverResult::Unsat {
                cs.remove(i);
            } else {
                i += 1;
            }
        }
        println!("check: conflicting constraint groups: {}", cs.iter().map(|g| g.0.clone()).collect::<Vec<_>>().join(" | "));
        return Ok(());
    }
    // the model's wiring states against the recorded ones, tick by tick: the first divergences
    if !p.rec.full.is_empty() {
        for tr in 0..2 {
            let mut div = Vec::new();
            for (k, fr) in p.rec.full[tr].iter().enumerate() {
                let k = k as i32;
                if k > p.h {
                    break;
                }
                for (c, kind) in &p.fixed {
                    let Some(real) = fr.get(c) else { continue };
                    if let Some((model, rv)) = wiring_div(&m, tr, k, *c, *kind, real) {
                        div.push(format!("t{k} {c:?} {kind:?}: model {model} game {rv}"));
                    }
                }
                if div.len() >= 6 {
                    break;
                }
            }
            if !div.is_empty() {
                println!("check: {} first divergences: {}", ["open", "close"][tr], div.join("; "));
            }
        }
    }
    let mut bad = 0;
    for c in &m.checks {
        let dir = m.e.val(c.direct);
        let qc = m.e.val(c.qc);
        let held = c.bud.is_some_and(|u| !m.e.val(u));
        let ok = if c.req { dir || (p.qc && qc) } else { !dir && (!qc || held) };
        if !ok {
            bad += 1;
            if bad <= 40 {
                println!("  {} t{:<3} piston {:?}: required {} model direct {} qc {}", ["open", "close"][c.tr], c.k, c.c, c.req, dir, qc);
            }
        }
    }
    println!("check: {} piston-tick mismatches out of {}", bad, m.checks.len());
    Ok(())
}

/// Parse "x0,y0,z0:x1,y1,z1".
pub fn parse_box(s: &str) -> Result<(i32, i32, i32, i32)> {
    let (a, b) = s.split_once(':').ok_or_else(|| anyhow!("box x0,y0,z0:x1,y1,z1"))?;
    let pa = crate::synth::parse_cells(a)?;
    let pb = crate::synth::parse_cells(b)?;
    let (a, b) = (pa[0], pb[0]);
    Ok((a[0].min(b[0]), a[1].min(b[1]), a[0].max(b[0]), a[1].max(b[1])))
}

/// Fix the source design's wiring (for `--check`).
pub fn fix_wiring(p: &mut Prob) -> Result<()> {
    let rec = &p.rec;
    for (c, s) in &rec.wiring {
        let k = Kind::of_state(s).with_context(|| format!("wiring block {s} at {c:?} not in the model's palette"))?;
        if !p.kinds.get(c).is_some_and(|ks| ks.contains(&k)) {
            bail!("wiring block {s} at {c:?}: cell not free or kind not in the palette");
        }
        p.fixed.insert(*c, k);
        p.fixed_state.insert(*c, s.clone());
    }
    let (lp, ls) = rec.lever.clone().context("no lever")?;
    let i = p.levers.iter().position(|l| l.pos == lp && short(&l.state) == short(&ls) && prop(&l.state, "face") == prop(&ls, "face") && (prop(&ls, "face") != Some("wall") || prop(&l.state, "facing") == prop(&ls, "facing")));
    p.fixed_lever = Some(i.with_context(|| format!("lever {ls} at {lp:?} is not a candidate"))?);
    p.fixed_only = true;
    for (c, k) in p.fixed.clone() {
        if let Some(sd) = k.support() {
            let s = add(c, sd);
            let ok = p.fixed.get(&s).is_some_and(|ks| ks.sturdy()) || p.static_sturdy.contains(&s) || (sd == DN && p.sturdy_up.contains(&s));
            if !ok {
                println!("check: {k:?} at {c:?} has no static sturdy support at {s:?} ({:?})", p.rec.phases[0].frames[0].get(&s));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- families catalog entries

fn parse_piece(code: &str) -> Result<crate::families::Pc> {
    use crate::families::{K, Pc};
    let (head, rest) = code.split_once('(').with_context(|| format!("piece {code:?}"))?;
    let xy: Vec<i8> = rest.trim_end_matches(')').split(',').map(|t| t.trim().parse::<i8>()).collect::<Result<_, _>>()?;
    let (x, y) = (xy[0], xy[1]);
    let k = match head {
        "stone" => K::Stone,
        "obsidian" => K::Obsidian,
        h => {
            let b = h.as_bytes();
            let sticky = b[0] == b'S';
            let f = match b[1] {
                b'E' => 0,
                b'W' => 1,
                b'U' => 2,
                b'D' => 3,
                _ => bail!("piece {code:?}"),
            };
            K::Piston { sticky, f, ext: h.ends_with('+') }
        }
    };
    Ok(Pc { x, y, k })
}

/// The ideal schedule of a families catalog entry (`CATALOG.tsv` id): the symmetric train when it
/// has one (`sym`), else the open/close schedules, `gap` ticks per macro step. Also returns the
/// entry's box in door coordinates.
pub fn families_schedule(tsv: &std::path::Path, id: &str, sym: bool, gap: u32) -> Result<(crate::ideal::Schedule, (i32, i32, i32, i32))> {
    use crate::families::{Bx, E, Mech};
    use crate::mechsynth::Act;
    let text = std::fs::read_to_string(tsv)?;
    let mut lines = text.lines();
    let hdr: Vec<&str> = lines.next().context("empty tsv")?.split('\t').collect();
    let col = |n: &str| hdr.iter().position(|h| *h == n).with_context(|| format!("no column {n}"));
    let row: Vec<&str> = lines.map(|l| l.split('\t').collect::<Vec<_>>()).find(|r| r[0] == id).with_context(|| format!("no entry {id}"))?;
    let (w, h) = row[col("box")?].split_once('x').context("box")?;
    let (ox, oy) = row[col("door_offset")?].split_once(',').context("offset")?;
    let (w, h, ox, oy): (i8, i8, i8, i8) = (w.parse()?, h.parse()?, ox.parse()?, oy.parse()?);
    let bx = Bx { x0: -ox, y0: -oy, w, h };
    let pieces: Vec<_> = row[col("pieces")?].split_whitespace().map(parse_piece).collect::<Result<_>>()?;
    let (m, _rest) = Mech::new(bx, &pieces).context("pieces overlap")?;
    let seq = |s: &str| -> Result<Vec<E>> {
        s.split_whitespace()
            .map(|t| {
                let act = if t.starts_with('P') { Act::P } else { Act::L };
                let code = &t[1..];
                let id = m.pid.iter().position(|&pi| m.pieces[pi].code() == code).with_context(|| format!("event {t}"))?;
                Ok(E { id: id as u8, act })
            })
            .collect()
    };
    let open = seq(row[col("open")?])?;
    let close = seq(row[col("close")?])?;
    let symv = row.get(col("sym")?).filter(|s| !s.trim().is_empty()).map(|s| seq(s)).transpose()?;
    let (wd, map) = crate::families::to_world(&m);
    let sol = crate::mechsynth::Solution {
        open: crate::families::ms_seq(&map, &open),
        close: crate::families::ms_seq(&map, &close),
        sym: symv.as_ref().map(|s| crate::families::ms_seq(&map, s)),
        states: 0,
    };
    let use_sym = sym && sol.sym.is_some();
    let plan = crate::mechsynth::ideal_plan(&wd, &sol, use_sym, gap, id)?;
    println!("families {id}: {} ({} schedule, gap {gap})", row[col("pieces")?], if use_sym { "symmetric" } else { "open/close" });
    let s = crate::ideal::Schedule::from_events(plan.design.clone(), &plan.events);
    Ok((s, (-(ox as i32), -(oy as i32), -(ox as i32) + w as i32 - 1, -(oy as i32) + h as i32 - 1)))
}

// ---------------------------------------------------------------- replay check over many designs

/// Why a design is outside the encoding (None: in scope).
pub fn out_of_scope(d: &Design) -> Option<String> {
    let Some(door) = d.ports.get("door") else { return Some("not a door (no door port)".into()) };
    if d.meta.get("door-axis").is_some_and(|v| v.trim() == "z") {
        return Some("turned door (door-axis z)".into());
    }
    if !d.entities.is_empty() {
        return Some("entities".into());
    }
    let z = door[2];
    let mut bad = BTreeSet::new();
    for (p, s) in &d.blocks {
        let n = short(s);
        if p[2] != z && n != "lever" {
            return Some("not flat (blocks outside the door plane)".into());
        }
        if n == "lever" {
            continue;
        }
        if is_wiring_state(s) && Kind::of_state(s).is_none() {
            bad.insert(n.to_string());
        }
        if matches!(n, "slime_block" | "honey_block") {
            bad.insert(n.to_string());
        }
        if mk(s) == MK::Cond && !matches!(n, "stone" | "smooth_stone" | "note_block" | "redstone_lamp" | "target") && !n.ends_with("_concrete") && !n.contains("terracotta") && !n.contains("planks") && !n.contains("wool") {
            // other full blocks are fine as door blocks/conductors
        }
        if (n.contains("trapdoor") && n != "iron_trapdoor") || n.contains("rail") || n.contains("sculk") || n.contains("hopper") || n == "scaffolding" || n == "target" {
            bad.insert(n.to_string());
        }
    }
    if !bad.is_empty() {
        return Some(format!("component not in the model: {}", bad.into_iter().collect::<Vec<_>>().join(",")));
    }
    None
}

/// `--check --update-order` on one design: its own wiring fixed in the SYNTHESIS model (every
/// requirement and the update-order clauses as constraints) must be satisfiable: one summary line.
pub fn selfcheck_line(w: &mut Worker, path: &std::path::Path, o: &ProbOpts) -> String {
    let d = match crate::cli::load_design(path) {
        Ok(d) => d,
        Err(e) => return format!("ERROR load: {e:#}"),
    };
    if let Some(why) = out_of_scope(&d) {
        return format!("OUT OF SCOPE: {why}");
    }
    let r = (|| -> Result<String> {
        let rec = record_wired(w, &d, 100)?;
        let mut p = build_prob(rec, o)?;
        fix_wiring(&mut p)?;
        let mut m = build(&p, false);
        let mut assumps: Vec<Lit> = m.sels.iter().map(|s| s.2).collect();
        assumps.extend(m.uo_sels.iter().map(|s| s.4));
        if m.e.s.solve_assumps(&assumps)? == SolverResult::Sat {
            return Ok("ACCEPTED".into());
        }
        let core = m.e.s.core().unwrap_or_default();
        let us: Vec<String> = m.uo_sels.iter().filter(|s| core.contains(&s.4) || core.contains(&!s.4)).map(|s| format!("{} t{} ({},{}) {}", ["open", "close"][s.0], s.1, s.2 .0, s.2 .1, s.3)).collect();
        let cs: Vec<String> = m.sels.iter().filter(|s| core.contains(&s.2) || core.contains(&!s.2)).map(|s| format!("{} ({},{})", ["open", "close"][s.0], s.1 .0, s.1 .1)).collect();
        Ok(format!("REJECTED pistons {} update-order {}", cs.join(" "), if us.is_empty() { "-".to_string() } else { us.join(", ") }))
    })();
    match r {
        Ok(s) => s,
        Err(e) => format!("ERROR: {}", format!("{e:#}").chars().take(160).collect::<String>()),
    }
}

/// `--check` on one design: one summary line.
pub fn check_line(w: &mut Worker, path: &std::path::Path, o: &ProbOpts) -> String {
    let d = match crate::cli::load_design(path) {
        Ok(d) => d,
        Err(e) => return format!("ERROR load: {e:#}"),
    };
    if let Some(why) = out_of_scope(&d) {
        return format!("OUT OF SCOPE: {why}");
    }
    let r = (|| -> Result<String> {
        let rec = record_wired(w, &d, 100)?;
        let mut p = build_prob(rec, o)?;
        fix_wiring(&mut p)?;
        let mut m = build(&p, true);
        let assumps: Vec<Lit> = m.gsels.iter().map(|g| g.1).collect();
        if m.e.s.solve_assumps(&assumps)? != SolverResult::Sat {
            return Ok("REJECTED: the model's dynamics/stability reject the wiring".into());
        }
        let mut bad = Vec::new();
        for c in &m.checks {
            let dir = m.e.val(c.direct);
            let qc = m.e.val(c.qc);
            let held = c.bud.is_some_and(|u| !m.e.val(u));
            let ok = if c.req { dir || (p.qc && qc) } else { !dir && (!qc || held) };
            if !ok {
                bad.push(format!("{} t{} {:?} req {}", ["open", "close"][c.tr], c.k, c.c, c.req));
            }
        }
        Ok(if bad.is_empty() { format!("REPRODUCED ({} piston-ticks)", m.checks.len()) } else { format!("MISMATCH {} of {}: {}", bad.len(), m.checks.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; ")) })
    })();
    match r {
        Ok(s) => s,
        Err(e) => format!("ERROR: {}", format!("{e:#}").chars().take(160).collect::<String>()),
    }
}

/// `--fixed <design>`: keep that design's wiring cells (and its lever) as they are; the solver
/// fills the other free cells. Cells outside the box become free cells with their kind.
pub fn fix_partial(p: &mut Prob, d: &Design) -> Result<()> {
    let z = p.z;
    let mut n = 0;
    for (pos, s) in &d.blocks {
        if pos[2] != z {
            continue;
        }
        let c = (pos[0], pos[1]);
        if p.mech.contains(&c) {
            continue;
        }
        if short(s) == "lever" {
            continue;
        }
        let Some(k) = Kind::of_state(s) else { bail!("--fixed: {s} at {c:?} is not a wiring kind") };
        p.kinds.entry(c).or_insert_with(|| vec![Kind::Empty, k]);
        if !p.kinds[&c].contains(&k) {
            p.kinds.get_mut(&c).unwrap().push(k);
        }
        if !p.free.contains(&c) {
            p.free.push(c);
        }
        p.fixed.insert(c, k);
        p.fixed_state.insert(c, s.clone());
        n += 1;
    }
    if let Some((lp, ls)) = d.blocks.iter().find(|(_, s)| short(s) == "lever") {
        let att = match prop(ls, "face") {
            Some("floor") => [lp[0], lp[1] - 1, lp[2]],
            Some("ceiling") => [lp[0], lp[1] + 1, lp[2]],
            _ => match prop(ls, "facing") {
                Some("east") => [lp[0] - 1, lp[1], lp[2]],
                Some("west") => [lp[0] + 1, lp[1], lp[2]],
                Some("north") => [lp[0], lp[1], lp[2] + 1],
                _ => [lp[0], lp[1], lp[2] - 1],
            },
        };
        p.levers.push(LeverCand { pos: *lp, att, state: ls.clone() });
        p.fixed_lever = Some(p.levers.len() - 1);
    }
    println!("--fixed: {n} wiring cells kept{}", if p.fixed_lever.is_some() { " + the lever" } else { "" });
    for (c, k) in p.fixed.clone() {
        if let Some(sd) = k.support() {
            let s = add(c, sd);
            let ok = p.fixed.get(&s).is_some_and(|ks| ks.sturdy()) || p.static_sturdy.contains(&s) || (sd == DN && p.sturdy_up.contains(&s));
            if !ok {
                println!("--fixed: {k:?} at {c:?} has no static sturdy support at {s:?} ({:?})", p.rec.phases[0].frames[0].get(&s));
            }
        }
        if !p.kinds.get(&c).is_some_and(|ks| ks.contains(&k)) {
            println!("--fixed: {k:?} at {c:?} not allowed there");
        }
    }
    Ok(())
}

/// A block of this type leaves cell c at some tick of the recorded phases (a carried block).
fn phases_full_moves(full: &[Vec<HashMap<C2, String>>], c: C2, s: &str) -> bool {
    full.iter().any(|ph| ph.iter().any(|f| f.get(&c).map(|x| short(x)) != Some(short(s))))
}
