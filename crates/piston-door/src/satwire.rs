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
use std::collections::{BTreeMap, HashMap, HashSet};

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
}

impl Kind {
    pub fn cond(self) -> bool {
        matches!(self, Kind::Block | Kind::Note)
    }
    pub fn sturdy(self) -> bool {
        matches!(self, Kind::Block | Kind::Obs(_) | Kind::Note | Kind::Bulb)
    }
    pub fn movable(self) -> bool {
        matches!(self, Kind::Block | Kind::Obs(_) | Kind::Note | Kind::Bulb)
    }
    pub fn has_out(self) -> bool {
        matches!(self, Kind::Rep(..) | Kind::Obs(_) | Kind::Torch | Kind::WTorch(_) | Kind::Cmp(_))
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
            Kind::Dust | Kind::Rep(..) | Kind::Cmp(_) | Kind::Torch => Some(DN),
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
            Kind::Rep(d, f) => format!("minecraft:repeater[delay={d},facing={},locked=false,powered={out}]", dname(f)),
            Kind::Obs(f) => format!("minecraft:observer[facing={},powered=false]", dname(f)),
            Kind::Torch => format!("minecraft:redstone_torch[lit={out}]"),
            Kind::WTorch(f) => format!("minecraft:redstone_wall_torch[facing={},lit={out}]", dname(f)),
            Kind::Note => format!("minecraft:note_block[instrument=harp,note=0,powered={b}]"),
            Kind::Bulb => format!("minecraft:waxed_copper_bulb[lit={a},powered={b}]"),
            Kind::Cmp(f) => format!("minecraft:comparator[facing={},mode=compare,powered={out}]", dname(f)),
        }
    }
    /// The kind of a wiring block state (None: not wiring).
    pub fn of_state(s: &str) -> Option<Kind> {
        let f = |k: &str| prop(s, k).and_then(dparse);
        Some(match short(s) {
            "smooth_stone" | "stone" => Kind::Block,
            "redstone_wire" => Kind::Dust,
            "repeater" => Kind::Rep(prop(s, "delay")?.parse().ok()?, f("facing")?),
            "observer" => Kind::Obs(f("facing")?),
            "redstone_torch" => Kind::Torch,
            "redstone_wall_torch" => Kind::WTorch(f("facing")?),
            "note_block" => Kind::Note,
            "waxed_copper_bulb" => Kind::Bulb,
            "comparator" => Kind::Cmp(f("facing")?),
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
            _ => bail!("unknown palette entry {t:?} (block,dust,rep,rep1..4,obs,torch,note,bulb,cmp)"),
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
        n if n.contains("glass") || n == "slime_block" || n == "honey_block" || n == "ice" => MK::Solid,
        _ => MK::Cond,
    }
}

fn is_wiring_state(s: &str) -> bool {
    matches!(
        short(s),
        "redstone_wire" | "repeater" | "comparator" | "observer" | "redstone_torch" | "redstone_wall_torch" | "lever" | "redstone_block" | "note_block" | "redstone_lamp" | "target" | "waxed_copper_bulb"
    )
}

#[derive(Clone, Debug, Default)]
pub struct Phase {
    /// Mechanism blocks after each tick (`frames[0]` = at the flip, before tick 1).
    pub frames: Vec<HashMap<C2, String>>,
    /// `power[j]` = power each piston base of `frames[j-1]` gets during tick j; `power[0]` = the
    /// stable power before the flip (pistons of `frames[0]`).
    pub power: Vec<HashMap<C2, bool>>,
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
    }
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
        if is_wiring_state(s) || (plain && stat) {
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
        phases[ph] = Phase { frames, power };
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
    Ok(Rec { z, door: spec.door, rest, phases, wiring, lever, src_box: Some(sbx) })
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
    let snap = |w: &mut Worker| -> Result<HashMap<C2, String>> {
        let r = w.get_region(sb, lo, hi)?;
        Ok(r.iter().filter(|(p, s)| p[2] == z && !s.ends_with(":air")).map(|(p, s)| ((p[0], p[1]), s.to_string())).collect())
    };
    let is_input = |s: &str| matches!(short(s), "redstone_block" | "lever");
    let mech = |f: &HashMap<C2, String>| -> HashMap<C2, String> { f.iter().filter(|(_, s)| !is_input(s)).map(|(c, s)| (*c, s.clone())).collect() };
    // Ideal inputs out of the plane (z-1 / z+1: families' and mech-synth's ideal plans put some
    // there) are wanted power too: directly behind / in front of a piston, or next to its quasi
    // cell. Without them the recorded power misses those pulses and every model is UNSAT.
    let oop_rb = |w: &mut Worker| -> Result<HashSet<C2>> {
        let r = w.get_region(sb, lo, hi)?;
        Ok(r.iter().filter(|(p, s)| (p[2] == z - 1 || p[2] == z + 1) && short(s) == "redstone_block").map(|(p, _)| (p[0], p[1])).collect())
    };
    let pow_of = |f: &HashMap<C2, String>, oop: &HashSet<C2>| -> HashMap<C2, bool> {
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
        let before_oop = oop_rb(w)?;
        let mut frames = Vec::new();
        let mut power = vec![pow_of(&before, &before_oop)];
        for t in 0..=ticks {
            for (et, p, st) in &evs {
                if *et == t {
                    if st.as_deref() == Some("@use") {
                        bail!("schedule `use` events are not supported");
                    }
                    let state = st.clone().unwrap_or_else(|| "minecraft:air".to_string());
                    w.set_blocks(sb, &[(*p, state)], PlaceMode::Update)?;
                }
            }
            let f = snap(w)?;
            let oop = oop_rb(w)?;
            frames.push(mech(&f));
            power.push(pow_of(&f, &oop));
            w.step(sb, 1)?;
        }
        power.truncate(frames.len());
        phases[ph] = Phase { frames, power };
        phases[ph].trim();
    }
    let _ = w.drop_sandbox(sb);
    let mut rest = BTreeMap::new();
    for (c, st) in &phases[0].frames[0] {
        rest.insert([c.0, c.1, z], st.clone());
    }
    Ok(Rec { z, door: spec.door, rest, phases, wiring: BTreeMap::new(), lever: None, src_box: None })
}

// ---------------------------------------------------------------- the SAT encoder

pub struct Enc {
    pub s: rustsat_cadical::CaDiCaL<'static, 'static>,
    n: u32,
    tru: Lit,
    pub clauses: usize,
}

impl Enc {
    pub fn new() -> Enc {
        let mut e = Enc { s: rustsat_cadical::CaDiCaL::default(), n: 1, tru: Lit::positive(0), clauses: 0 };
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
        let v: Clause = c.iter().copied().filter(|&l| l != !self.tru).collect();
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
    pub levers: Vec<LeverCand>,
    pub dmax: i32,
    pub h: i32,
    pub qc: bool,
    pub levels: u8,
    /// cells whose wiring placement is fixed (`--check`)
    pub fixed: HashMap<C2, Kind>,
    pub fixed_lever: Option<usize>,
}

impl Prob {
    pub fn is_free(&self, c: C2) -> bool {
        self.kinds.contains_key(&c)
    }
    fn can(&self, c: C2, f: impl Fn(Kind) -> bool) -> bool {
        self.kinds.get(&c).is_some_and(|ks| ks.iter().any(|k| f(*k)))
    }
    /// Frame index used for the combinational state of SAT tick k with offset d.
    fn fj(&self, tr: usize, k: i32, d: i32) -> i32 {
        if k <= 0 { 0 } else { (k - 1 - d).clamp(0, self.rec.phases[tr].k()) }
    }
    fn mk_at(&self, tr: usize, k: i32, d: i32, c: C2) -> MK {
        self.rec.phases[tr].frame(self.fj(tr, k, d)).get(&c).map_or(MK::Air, |s| mk(s))
    }
}

pub struct ProbOpts {
    pub bx: Option<(i32, i32, i32, i32)>,
    pub palette: Vec<Kind>,
    pub forbid_ring: bool,
    pub levers: Option<Vec<Pos>>,
    pub lever_oop: bool,
    pub dmax: i32,
    pub settle: i32,
    pub qc: bool,
    pub levels: u8,
}

/// Free cells, palettes per cell, lever candidates.
pub fn build_prob(rec: Rec, o: &ProbOpts) -> Result<Prob> {
    let z = rec.z;
    let (dx, dy) = (rec.door[0], rec.door[1]);
    let mut mech: HashSet<C2> = HashSet::new();
    let mut stat: HashMap<C2, Option<String>> = HashMap::new();
    for ph in &rec.phases {
        for f in &ph.frames {
            for (c, s) in f {
                mech.insert(*c);
            }
        }
    }
    for c in &mech {
        let mut v: Option<String> = None;
        let mut ok = true;
        for ph in &rec.phases {
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
    // hallway holes: ring cells some block passes through that are air in a stable state; the
    // cell right behind (outward) may only hold a plain block.
    let mut hole_back: HashSet<C2> = HashSet::new();
    for r in &ring {
        let swept = rec.phases.iter().any(|ph| ph.frames.iter().any(|f| f.get(r).is_some_and(|s| mk(s) != MK::Air)));
        let air_stable = rec.phases.iter().any(|ph| ph.frames[0].get(r).is_none());
        if swept && air_stable {
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
    let mut free = Vec::new();
    for y in bx.1..=bx.3 {
        for x in bx.0..=bx.2 {
            let c = (x, y);
            if mech.contains(&c) || in_t(c) {
                continue;
            }
            let plain_only = o.forbid_ring && (ring.contains(&c) || hole_back.contains(&c));
            let ks: Vec<Kind> = o.palette.iter().copied().filter(|k| !plain_only || matches!(k, Kind::Empty | Kind::Block)).collect();
            kinds.insert(c, ks);
            free.push(c);
        }
    }
    // sticky pistons never pull wiring: the cell two ahead of any sticky piston holds nothing movable
    for ph in &rec.phases {
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
    let sturdy_possible = |c: C2| static_sturdy.contains(&c) || kinds.get(&c).is_some_and(|ks| ks.iter().any(|k| k.sturdy()));
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
    Ok(Prob { rec, z, bx, free, kinds, mech, static_sturdy, levers, dmax: o.dmax, h, qc: o.qc, levels: o.levels, fixed: HashMap::new(), fixed_lever: None })
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

fn pred_fn(t: u8) -> fn(Kind) -> bool {
    match t {
        P_COND => |k| k.cond(),
        P_DUST => |k| k == Kind::Dust,
        P_STURDY => |k| k.sturdy(),
        P_OUT => |k| k.has_out(),
        P_OBS => |k| matches!(k, Kind::Obs(_)),
        P_REP => |k| matches!(k, Kind::Rep(..)),
        P_TORCH => |k| matches!(k, Kind::Torch | Kind::WTorch(_)),
        P_CMP => |k| matches!(k, Kind::Cmp(_)),
        P_NOTE => |k| k == Kind::Note,
        P_BULB => |k| k == Kind::Bulb,
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
}

pub struct Model<'a> {
    pub p: &'a Prob,
    pub e: Enc,
    pub kl: HashMap<(C2, Kind), Lit>,
    preds: HashMap<(C2, u8), Lit>,
    pub lever: Vec<Lit>,
    pub dsel: [Vec<Lit>; 2],
    out: HashMap<Key, Lit>,
    sched: HashMap<Key, Lit>,
    lv: HashMap<(usize, i32, C2, u8), Lit>,
    st: HashMap<Key, Lit>,
    pw: HashMap<Key, Lit>,
    bpow: HashMap<Key, Lit>,
    blit: HashMap<Key, Lit>,
    npow: HashMap<Key, Lit>,
    sigc: HashMap<(usize, i32, C2, C2, bool), Lit>,
    mc: HashMap<(usize, i32, C2, u8), Lit>,
    chgc: HashMap<(usize, i32, C2, u8), Lit>,
    causes: HashMap<Key, Vec<Lit>>,
    pub used: Vec<(C2, Lit)>,
    pub checks: Vec<PCheck>,
    pub check_mode: bool,
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
        (tr == 0) == (k >= 0)
    }
    fn out(&self, tr: usize, k: i32, c: C2) -> Lit {
        let k = if k == 0 { -1 } else { k };
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
    /// Mechanism property literal at SAT tick k over the offsets (0 cond, 1..=4 piston facing E/W/U/D).
    fn mprop(&mut self, tr: usize, k: i32, c: C2, what: u8) -> Lit {
        if let Some(l) = self.mc.get(&(tr, k, c, what)) {
            return *l;
        }
        let mut lits = Vec::new();
        for d in 0..=self.p.dmax {
            let m = self.p.mk_at(tr, k, d, c);
            let hit = match (what, m) {
                (0, MK::Cond) => true,
                (1..=4, MK::Piston { f, .. }) => f == DIRS[(what - 1) as usize],
                _ => false,
            };
            if hit {
                lits.push(self.dsel[tr][d as usize]);
            }
        }
        let l = if lits.len() == self.dsel[tr].len() { self.e.t() } else { self.e.or(&lits) };
        self.mc.insert((tr, k, c, what), l);
        l
    }
    /// Mechanism cell changes during SAT tick k (observers see it).
    fn mchg(&mut self, tr: usize, k: i32, c: C2) -> Lit {
        if let Some(l) = self.mc.get(&(tr, k, c, 9)) {
            return *l;
        }
        let ph = &self.p.rec.phases[tr];
        let mut lits = Vec::new();
        for d in 0..=self.p.dmax {
            let j = k - d;
            if j >= 1 && j <= ph.k() && ph.frames[j as usize].get(&c) != ph.frames[j as usize - 1].get(&c) {
                lits.push(self.dsel[tr][d as usize]);
            }
        }
        let l = self.e.or(&lits);
        self.mc.insert((tr, k, c, 9), l);
        l
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
    fn sturdy_static(&mut self, c: C2) -> Lit {
        if self.p.is_free(c) {
            self.pred(c, P_STURDY)
        } else {
            self.e.konst(self.p.static_sturdy.contains(&c))
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
        let v = [self.dpow(tr, k, add(c, U)), self.dpow(tr, k, add(c, E)), self.dpow(tr, k, add(c, W))];
        self.e.or(&v)
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
                terms.push(self.dpow(tr, k, a));
            }
            let lits: Vec<Lit> = ks.iter().filter(|kk| kk.out_to().iter().any(|o| add(a, *o) == b)).map(|kk| self.kl[&(a, *kk)]).collect();
            if !lits.is_empty() {
                let s = self.e.or(&lits);
                let o = self.out(tr, k, a);
                terms.push(self.e.and(&[s, o]));
            }
        } else if self.is_mech(a) {
            terms.push(self.cpow(tr, k, a, dust_rx));
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
    fn chg(&mut self, tr: usize, k: i32, w: C2, what: u8) -> Lit {
        // what: 0 = pre (repeater), 1 = ambiguous, 2 = post (all)
        if let Some(l) = self.chgc.get(&(tr, k, w, what)) {
            return *l;
        }
        let f = self.e.f();
        let l = if what == 2 {
            let a = self.chg(tr, k, w, 1);
            let m = if self.is_mech(w) { self.mchg(tr, k, w) } else { f };
            let z = self.p.z;
            let lv = if k == 0 { self.lever_on_lits(0, 0, |l| l.pos == [w.0, w.1, z]) } else { f };
            self.e.or(&[a, m, lv])
        } else if !self.p.is_free(w) || k == 0 && what == 0 {
            f
        } else {
            let o1 = self.out(tr, k, w);
            let o0 = self.out(tr, k - 1, w);
            let oc = self.e.xor(o1, o0);
            if what == 0 {
                let r = self.pred(w, P_REP);
                self.e.and(&[r, oc])
            } else {
                let mut t = Vec::new();
                let ob = self.pred(w, P_OBS);
                let to = self.pred(w, P_TORCH);
                let cm = self.pred(w, P_CMP);
                let g = self.e.or(&[ob, to, cm]);
                t.push(self.e.and(&[g, oc]));
                if self.p.can(w, |kk| kk == Kind::Dust) {
                    let mut x = Vec::new();
                    for j in self.lo()..=15 {
                        let a = self.lvl(tr, k, w, j);
                        let b = self.lvl(tr, k - 1, w, j);
                        x.push(self.e.xor(a, b));
                    }
                    t.push(self.e.or(&x));
                }
                for (m, _) in [(0, ()), (1, ())] {
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
    fn cause(&mut self, tr: usize, s: i32, c: C2, l: Lit) {
        self.causes.entry((tr, s, c)).or_default().push(l);
    }
}

/// Build the whole encoding.
pub fn build(p: &Prob, check_mode: bool) -> Model<'_> {
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
    let dsel: [Vec<Lit>; 2] = [(0..=p.dmax).map(|_| e.var()).collect(), (0..=p.dmax).map(|_| e.var()).collect()];
    for tr in 0..2 {
        e.exactly_one(&dsel[tr]);
    }
    let mut m = Model {
        p,
        e,
        kl,
        preds: HashMap::new(),
        lever,
        dsel,
        out: HashMap::new(),
        sched: HashMap::new(),
        lv: HashMap::new(),
        st: HashMap::new(),
        pw: HashMap::new(),
        bpow: HashMap::new(),
        blit: HashMap::new(),
        npow: HashMap::new(),
        sigc: HashMap::new(),
        mc: HashMap::new(),
        chgc: HashMap::new(),
        causes: HashMap::new(),
        used: Vec::new(),
        checks: Vec::new(),
        check_mode,
    };
    let h = p.h;
    let lo = m.lo();
    // ---- structural: supports, levers, fixed cells, objective
    for c in p.free.clone() {
        for k in p.kinds[&c].clone() {
            let l = m.kl[&(c, k)];
            if let Some(sd) = k.support() {
                let s = m.sturdy_static(add(c, sd));
                m.e.add(&[!l, s]);
            }
            if let Kind::Cmp(f) = k {
                let d = m.pred(add(c, f), P_DUST);
                m.e.add(&[!l, !d]);
            }
        }
        let empty = m.kl[&(c, Kind::Empty)];
        m.used.push((c, !empty));
        if let Some(fk) = p.fixed.get(&c) {
            let l = m.kind(c, *fk);
            m.e.add(&[l]);
        } else if !p.fixed.is_empty() {
            m.e.add(&[empty]);
        }
    }
    for (i, lc) in p.levers.iter().enumerate() {
        let l = m.lever[i];
        let att = (lc.att[0], lc.att[1]);
        let s = m.sturdy_static(att);
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
    // ---- allocate the dynamic variables
    for tr in 0..2 {
        for k in -1..=h {
            for c in p.free.clone() {
                if k != 0 && p.can(c, |kk| kk.has_out()) {
                    let v = m.e.var();
                    m.out.insert((tr, k, c), v);
                }
                if p.can(c, |kk| kk == Kind::Dust) {
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
                if p.can(c, |kk| kk == Kind::Note) {
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
                }
            }
        }
    }
    // ---- per trace, per tick
    for tr in 0..2 {
        for k in -1..=h {
            for c in p.free.clone() {
                let ks = p.kinds[&c].clone();
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
                if ks.contains(&Kind::Dust) {
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
                            if p.can(upn, |kk| kk == Kind::Dust) {
                                let g = m.e.and(&[nc, !up_c]);
                                let l = m.lvl(tr, k, upn, j + 1);
                                sup.push(m.e.and(&[g, l]));
                            }
                            if p.can(dnn, |kk| kk == Kind::Dust) {
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
                if ks.contains(&Kind::Note) {
                    let isn = m.pred(c, P_NOTE);
                    let i = m.any_in(tr, k, c, None);
                    let a = m.e.and(&[isn, i]);
                    let v = m.npow[&(tr, k, c)];
                    m.e.eq(v, a);
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
                                let amb = m.chg(tr, k, w, 1);
                                m.e.add(&[!g, !s_k, !o_p, !amb]);
                            }
                            let pre = m.chg(tr, k, w, 0);
                            let post = m.chg(tr, k, w, 2);
                            let onp = if k >= 1 { o_p } else { o_k };
                            let s1 = m.sched(tr, k + 1, c);
                            let a = m.e.and(&[pre, !onp]);
                            let b = m.e.and(&[post, !o_k]);
                            let tr_ = m.e.or(&[a, b]);
                            let trig = m.e.and(&[!s1, tr_]);
                            let fire_on = if k >= 1 { m.e.and(&[s_k, !o_p]) } else { f };
                            let any = m.e.or(&[fire_on, trig]);
                            let cs = m.e.and(&[g, any]);
                            m.cause(tr, k + 2, c, cs);
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
                                let fire_val = m.e.or(&[!o_p, i_p]);
                                let g1 = m.e.and(&[g, s_k]);
                                m.e.eq_if(g1, o_k, fire_val);
                                let g2 = m.e.and(&[g, !s_k]);
                                m.e.eq_if(g2, o_k, o_p);
                                why.push(m.e.and(&[s_k, !o_p, !i_p]));
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
                                m.e.eq_if(g1, o_k, !i_k);
                                let g2 = m.e.and(&[g, !s_k]);
                                m.e.eq_if(g2, o_k, o_p);
                            }
                            let s1 = m.sched(tr, k + 1, c);
                            let mis = m.e.xor(o_k, !i_k);
                            let cs = m.e.and(&[g, mis, !s1]);
                            m.cause(tr, k + 2, c, cs);
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
                                m.e.eq_if(g1, o_k, i_p);
                                let g2 = m.e.and(&[g, !s_k]);
                                m.e.eq_if(g2, o_k, o_p);
                                let race = m.e.xor(i_k, i_p);
                                m.e.add(&[!g, !s_k, !race]);
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
        // ---- pistons get exactly their recorded power
        let mech: Vec<C2> = p.mech.iter().copied().collect();
        let ks: Vec<i32> = std::iter::once(-1).chain(1..=h).collect();
        for &k in &ks {
            for &c in &mech {
                for d in 0..=p.dmax {
                    let MK::Piston { f: fc, .. } = p.mk_at(tr, k, d, c) else { continue };
                    let j = if k == -1 { 0 } else { k - d };
                    let req = p.rec.phases[tr].pow(j).get(&c).copied().unwrap_or(false);
                    let front = if fc == (0, 0) { None } else { Some(fc) };
                    let direct = m.any_in(tr, k, c, front);
                    let q = add(c, U);
                    let mut qt = Vec::new();
                    for dd in [E, W, U] {
                        qt.push(m.sig(tr, k, add(q, dd), q, false));
                    }
                    qt.push(m.oop(tr, k, q));
                    let qc = m.e.or(&qt);
                    let ds = m.dsel[tr][d as usize];
                    if check_mode {
                        m.checks.push(PCheck { tr, k, c, d, req, direct, qc });
                        continue;
                    }
                    if req {
                        if p.qc {
                            m.e.add(&[!ds, direct, qc]);
                        } else {
                            m.e.add(&[!ds, direct]);
                        }
                    } else {
                        m.e.add(&[!ds, !direct]);
                        m.e.add(&[!ds, !qc]);
                    }
                }
            }
        }
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
    let skeys: Vec<Key> = m.sched.keys().copied().collect();
    for key in skeys {
        if !m.causes.contains_key(&key) {
            let v = m.sched[&key];
            m.e.add(&[!v]);
        }
    }
    // ---- each trace ends where the other starts
    for c in p.free.clone() {
        for (a, b) in [(0usize, 1usize), (1, 0)] {
            let x = m.out(a, h, c);
            let y = m.out(b, -1, c);
            m.e.eq(x, y);
            let f = m.e.f();
            let x = Model::get(&m.blit, (a, h, c), f);
            let y = Model::get(&m.blit, (b, -1, c), f);
            m.e.eq(x, y);
        }
    }
    if check_mode {
        for tr in 0..2 {
            let d0 = m.dsel[tr][0];
            m.e.add(&[d0]);
        }
    }
    m
}

// ---------------------------------------------------------------- decode, judge, CEGAR

pub struct Sol {
    pub design: Design,
    pub kinds: BTreeMap<C2, Kind>,
    pub lever: usize,
    pub d: [i32; 2],
    pub wiring: usize,
}

pub fn decode(m: &Model, name: &str) -> Sol {
    let p = m.p;
    let z = p.z;
    let f = m.e.f();
    let mut kinds = BTreeMap::new();
    let mut d = Design { name: name.to_string(), ..Default::default() };
    for (pos, s) in &p.rec.rest {
        d.blocks.insert(*pos, s.clone());
    }
    for c in &p.free {
        let k = p.kinds[c].iter().copied().find(|k| m.e.val(m.kl[&(*c, *k)])).unwrap_or(Kind::Empty);
        if k == Kind::Empty {
            continue;
        }
        kinds.insert(*c, k);
        let out = m.e.val(m.out(0, -1, *c));
        let lvl = (m.lo()..=15).rev().find(|j| m.e.val(m.lvl(0, -1, *c, *j))).unwrap_or(0);
        let a = m.e.val(Model::get(&m.blit, (0, -1, *c), f));
        let b = m.e.val(Model::get(&m.bpow, (0, -1, *c), f)) || m.e.val(Model::get(&m.npow, (0, -1, *c), f));
        d.blocks.insert([c.0, c.1, z], k.state(out, lvl, a, b));
    }
    let lever = (0..p.levers.len()).find(|i| m.e.val(m.lever[*i])).unwrap_or(0);
    let lc = &p.levers[lever];
    d.blocks.insert(lc.pos, lc.state.clone());
    d.ports.insert("door".into(), p.rec.door);
    d.ports.insert("lever".into(), lc.pos);
    let dd = |tr: usize| (0..=p.dmax).find(|i| m.e.val(m.dsel[tr][*i as usize])).unwrap_or(0);
    let wiring = kinds.len();
    Sol { design: d, kinds, lever, d: [dd(0), dd(1)], wiring }
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
    pub minimize: bool,
    pub max_blocks: Option<usize>,
    pub extra: u32,
    pub out: Option<std::path::PathBuf>,
    pub name: String,
    pub version: String,
}

/// Judge a candidate: Ok(report line) or Err(reason).
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
    let t0 = std::time::Instant::now();
    let mut m = build(p, false);
    println!(
        "[{:6.1}s] encoded: {} free cells, {} lever candidates, offsets 0..={}, horizon {} ticks, {} vars, {} clauses",
        t0.elapsed().as_secs_f64(),
        p.free.len(),
        p.levers.len(),
        p.dmax,
        p.h,
        m.e.vars(),
        m.e.clauses
    );
    let used: Vec<Lit> = m.used.iter().map(|u| u.1).collect();
    let tot = m.e.totalizer(&used);
    let mut bound = o.max_blocks.unwrap_or(used.len()).min(used.len());
    let cfg = mctas::sim::WorkerConfig { version: o.version.clone(), heap_mb: 1024, ..Default::default() };
    let mut w = Worker::spawn(&cfg)?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut best: Option<(Design, usize)> = None;
    let mut fails = 0usize;
    for it in 0..o.iters {
        let assumps: Vec<Lit> = if bound < tot.len() { vec![!tot[bound]] } else { vec![] };
        let ts = std::time::Instant::now();
        let r = m.e.s.solve_assumps(&assumps)?;
        let dt = ts.elapsed().as_secs_f64();
        if r != SolverResult::Sat {
            match &best {
                None => println!("[{:6.1}s] UNSAT (solve {dt:.1}s): no wiring exists within this palette, box and model (wiring <= {bound}, {fails} judged designs blocked)", t0.elapsed().as_secs_f64()),
                Some((_, b)) => println!("[{:6.1}s] no wiring with <= {bound} blocks (solve {dt:.1}s): {b} wiring blocks is minimal in this model", t0.elapsed().as_secs_f64()),
            }
            break;
        }
        let s = decode(&m, &o.name);
        println!("[{:6.1}s] iter {it}: SAT (solve {dt:.1}s): {} wiring blocks, offsets open {} close {}\n{}", t0.elapsed().as_secs_f64(), s.wiring, s.d[0], s.d[1], render(p, &s));
        match judge(&mut w, sb, &s.design, 0) {
            Ok(rep) => {
                println!("  JUDGE OK (4 positions): {rep}");
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
                println!("  JUDGE FAILS: {}", e.chars().take(300).collect::<String>());
                let mut cl: Vec<Lit> = Vec::new();
                for c in &p.free {
                    let k = s.kinds.get(c).copied().unwrap_or(Kind::Empty);
                    cl.push(!m.kl[&(*c, k)]);
                }
                cl.push(!m.lever[s.lever]);
                m.e.add(&cl);
            }
        }
    }
    let _ = w.drop_sandbox(sb);
    let Some((d, _)) = best else { return Ok(None) };
    if o.extra > 0 {
        let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        match judge(&mut w, sb, &d, o.extra) {
            Ok(rep) => println!("JUDGE OK with --extra-offsets {}: {rep}", o.extra),
            Err(e) => println!("JUDGE FAILS with --extra-offsets {}: {}", o.extra, e.chars().take(300).collect::<String>()),
        }
    }
    Ok(Some(d))
}

/// `--check`: the source design's own wiring fixed; print where the model's piston power
/// differs from the recorded one.
pub fn check(p: &Prob) -> Result<()> {
    let mut m = build(p, true);
    println!("check: {} vars, {} clauses", m.e.vars(), m.e.clauses);
    let r = m.e.s.solve()?;
    if r != SolverResult::Sat {
        println!("check: the model REJECTS the source wiring (dynamics/stability UNSAT)");
        return Ok(());
    }
    let mut bad = 0;
    for c in &m.checks {
        let dir = m.e.val(c.direct);
        let qc = m.e.val(c.qc);
        let ok = if c.req { dir || (p.qc && qc) } else { !dir && !qc };
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
    }
    let (lp, ls) = rec.lever.clone().context("no lever")?;
    let i = p.levers.iter().position(|l| l.pos == lp && short(&l.state) == short(&ls) && prop(&l.state, "face") == prop(&ls, "face") && (prop(&ls, "face") != Some("wall") || prop(&l.state, "facing") == prop(&ls, "facing")));
    p.fixed_lever = Some(i.with_context(|| format!("lever {ls} at {lp:?} is not a candidate"))?);
    if p.fixed.is_empty() {
        p.fixed.insert((i32::MIN, i32::MIN), Kind::Empty);
    }
    Ok(())
}
