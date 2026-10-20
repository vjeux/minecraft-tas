//! The contraption studies ([docs/circuits.md](../../../docs/circuits.md)): pulse circuits, T
//! flip-flops, delay lines, logic gates, memory, piston extenders and flying machines.
//!
//! A circuit is a design in the door/clock design text format (`crates/mctas/src/design.rs`) with:
//!
//! * `circuit: <kind> [args]`: what it claims to be ([`Kind`]); the kind's checker generates the
//!   stimulus ([`stimulus`]) and judges the recorded run ([`check`]);
//! * `port input[.<name>]: x y z [lever | button | <on-state> [<off-state>]]`: the cells the judge
//!   drives ([`InputHow`]);
//! * `port output[.<name>]: x y z`: the cells whose level is recorded every gt ([`trace::level`]);
//! * machines: `port payload[.<name>]: x y z` (tracked blocks), `travel: dx dy dz`, `distance: k`,
//!   and the clock's one-time `port start:` kick.
//!
//! [`run`] records a [`trace::Trace`] on redsim (the screen) or in the real game (the judge);
//! [`verify`] runs the kind's stimulus at 4 positions x 4 rotations and compares the builds.

pub mod archive;
pub mod check;
pub mod fly;
pub mod run;
pub mod stimulus;
pub mod trace;
pub mod verify;

use anyhow::{Context, Result, anyhow, bail};
use clock::{Action, Clock};
use mctas::sim::Worker;
use std::collections::BTreeSet;
use std::path::Path;

pub use clock::Pos;

/// The default on-state of an input cell: a redstone block written with updates.
pub const INPUT_ON: &str = "minecraft:redstone_block";

/// The judge's consumer, placed in every air output cell (vjeux/coordinator 2026-10-19: a circuit
/// must DRIVE something): a redstone lamp. The output's level is still the best signal into the
/// cell, and the lamp must follow it ([`check::update_less`]): an output that changes without a
/// block update (a dust redirected without a power change) leaves the lamp stale and fails.
pub const CONSUMER: &str = "minecraft:redstone_lamp[lit=false]";

/// How the judge drives an input cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputHow {
    /// The cell is written with updates: `on` while the input is on, `off` (default air) while off;
    /// the design holds `off` there.
    Block { on: String, off: String },
    /// The design's lever in this cell (unpowered at rest), right-clicked at every input edge.
    Lever,
    /// The design's button in this cell, right-clicked at every rising input edge (the button's
    /// own pulse length; falling edges are the button's).
    Button,
}

/// An input cell.
#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub pos: Pos,
    pub how: InputHow,
}

/// An output or payload cell.
#[derive(Clone, Debug)]
pub struct Probe {
    pub name: String,
    pub pos: Pos,
}

/// What a circuit claims to be: the axis of its study and the checker it is judged by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    RisingEdge,
    FallingEdge,
    DualEdge,
    PulseLimiter,
    PulseExtender,
    Tff,
    Delay,
    /// A steady-state gate; the claimed function (`gate xor`) is checked when given.
    Gate(Option<String>),
    RsLatch,
    DLatch,
    /// Output changes once every N input pulses.
    Counter(u32),
    /// Pushes its payload N cells along `travel` and brings it back.
    PistonExtender(u32),
    FlyingMachine,
}

/// The gate functions by truth table (inputs a, b in name order; bit index 2a + b), 2 inputs.
pub const GATES2: [(&str, [bool; 4]); 16] = [
    ("false", [false, false, false, false]),
    ("and", [false, false, false, true]),
    ("nimply", [false, false, true, false]),
    ("a", [false, false, true, true]),
    ("nimply-ba", [false, true, false, false]),
    ("b", [false, true, false, true]),
    ("xor", [false, true, true, false]),
    ("or", [false, true, true, true]),
    ("nor", [true, false, false, false]),
    ("xnor", [true, false, false, true]),
    ("not-b", [true, false, true, false]),
    ("imply-ba", [true, false, true, true]),
    ("not-a", [true, true, false, false]),
    ("imply", [true, true, false, true]),
    ("nand", [true, true, true, false]),
    ("true", [true, true, true, true]),
];

impl Kind {
    pub fn parse(s: &str) -> Result<Kind> {
        let w: Vec<&str> = s.split_whitespace().collect();
        let num = |i: usize| -> Result<u32> {
            let v = w.get(i).ok_or_else(|| anyhow!("circuit: {s:?}: missing its number"))?;
            v.parse().with_context(|| format!("circuit: {s:?}: bad number {v:?}"))
        };
        Ok(match w.first().copied().unwrap_or("") {
            "rising-edge" => Kind::RisingEdge,
            "falling-edge" => Kind::FallingEdge,
            "dual-edge" => Kind::DualEdge,
            "pulse-limiter" => Kind::PulseLimiter,
            "pulse-extender" => Kind::PulseExtender,
            "tff" => Kind::Tff,
            "delay" => Kind::Delay,
            "gate" => {
                let f = w.get(1).map(|f| f.to_ascii_lowercase());
                if let Some(f) = &f
                    && f != "not"
                    && f != "buffer"
                    && !GATES2.iter().any(|(n, _)| n == f)
                {
                    bail!("circuit: gate {f:?}: not a gate function (not, buffer, and, or, xor, xnor, nand, nor, imply, nimply, imply-ba, nimply-ba)");
                }
                Kind::Gate(f)
            }
            "rs-latch" => Kind::RsLatch,
            "d-latch" => Kind::DLatch,
            "counter" => Kind::Counter(num(1)?),
            "piston-extender" => Kind::PistonExtender(num(1)?),
            "flying-machine" => Kind::FlyingMachine,
            k => bail!(
                "circuit: unknown kind {k:?} (rising-edge, falling-edge, dual-edge, pulse-limiter, pulse-extender, tff, delay, gate [fn], rs-latch, d-latch, counter N, piston-extender N, flying-machine)"
            ),
        })
    }

    /// The kind as written on the `circuit:` line.
    pub fn name(&self) -> String {
        match self {
            Kind::RisingEdge => "rising-edge".into(),
            Kind::FallingEdge => "falling-edge".into(),
            Kind::DualEdge => "dual-edge".into(),
            Kind::PulseLimiter => "pulse-limiter".into(),
            Kind::PulseExtender => "pulse-extender".into(),
            Kind::Tff => "tff".into(),
            Kind::Delay => "delay".into(),
            Kind::Gate(None) => "gate".into(),
            Kind::Gate(Some(f)) => format!("gate {f}"),
            Kind::RsLatch => "rs-latch".into(),
            Kind::DLatch => "d-latch".into(),
            Kind::Counter(n) => format!("counter {n}"),
            Kind::PistonExtender(n) => format!("piston-extender {n}"),
            Kind::FlyingMachine => "flying-machine".into(),
        }
    }

    /// The study the kind belongs to (its catalog: `designs/circuits/<study>/`).
    pub fn study(&self) -> &'static str {
        match self {
            Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => "pulse",
            Kind::Tff => "tff",
            Kind::Delay => "delay",
            Kind::Gate(_) => "gates",
            Kind::RsLatch | Kind::DLatch | Kind::Counter(_) => "memory",
            Kind::PistonExtender(_) => "pext",
            Kind::FlyingMachine => "fly",
        }
    }

    /// The input names the kind needs (`None`: any one input).
    fn inputs_needed(&self) -> Option<&'static [&'static str]> {
        match self {
            Kind::RsLatch => Some(&["r", "s"]),
            Kind::DLatch => Some(&["d", "e"]),
            _ => None,
        }
    }
}

/// The canonical input name: long aliases of the latch inputs (`set`, `reset`, `data`, `enable`,
/// `clock`) map to `s`, `r`, `d`, `e`.
fn input_alias(name: &str) -> String {
    match name {
        "set" => "s".into(),
        "reset" => "r".into(),
        "data" => "d".into(),
        "enable" | "clock" | "clk" => "e".into(),
        n => n.to_string(),
    }
}

/// A circuit design.
#[derive(Clone, Debug)]
pub struct Circuit {
    /// The design with its placement and its optional one-time kick (machines), as a clock.
    pub clock: Clock,
    /// `None`: no `circuit:` line (an explicit `stimulus:` only, for exploring).
    pub kind: Option<Kind>,
    /// In name order (a bare `port input:` is named `in`).
    pub inputs: Vec<Input>,
    /// In name order (a bare `port output:` is named `out`).
    pub outputs: Vec<Probe>,
    pub payloads: Vec<Probe>,
    /// The push or travel direction (a unit vector).
    pub travel: Option<Pos>,
    /// How far a flying machine must travel.
    pub distance: Option<u32>,
    /// The explicit `stimulus:` header.
    pub stimulus: Option<String>,
}

impl Circuit {
    pub fn load(path: &Path) -> Result<Circuit> {
        let d = mctas::design::Design::load(path)?;
        Circuit::from_design(d).with_context(|| format!("circuit design {}", path.display()))
    }

    pub fn from_design(design: mctas::design::Design) -> Result<Circuit> {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();
        let mut payloads = Vec::new();
        for (name, pos) in &design.ports {
            let args = design.meta.get(&format!("port {name}")).cloned().unwrap_or_default();
            let (base, sub) = match name.split_once('.') {
                Some((b, s)) => (b, Some(s)),
                None => (name.as_str(), None),
            };
            match base {
                "input" => {
                    let iname = input_alias(sub.unwrap_or("in"));
                    let how = parse_input(&design, *pos, &args).with_context(|| format!("port {name}"))?;
                    inputs.push(Input { name: iname, pos: *pos, how });
                }
                "output" => outputs.push(Probe { name: sub.unwrap_or("out").to_string(), pos: *pos }),
                "payload" => payloads.push(Probe { name: sub.unwrap_or("payload").to_string(), pos: *pos }),
                "start" => {}
                _ => bail!("port {name}: unknown port (input[.name], output[.name], payload[.name], start)"),
            }
        }
        inputs.sort_by(|a, b| a.name.cmp(&b.name));
        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        payloads.sort_by(|a, b| a.name.cmp(&b.name));
        for w in inputs.windows(2) {
            if w[0].name == w[1].name {
                bail!("two inputs named {:?}", w[0].name);
            }
        }
        let kind = design.meta.get("circuit").map(|s| Kind::parse(s)).transpose()?;
        let travel = match design.meta.get("travel") {
            Some(t) => {
                let v: Vec<i32> = t.split_whitespace().map(|x| x.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("travel: {t:?}"))?;
                if v.len() != 3 || v.iter().map(|x| x.abs()).sum::<i32>() != 1 {
                    bail!("travel: {t:?}: want a unit vector dx dy dz (one of them 1 or -1)");
                }
                Some([v[0], v[1], v[2]])
            }
            None => None,
        };
        let distance = design.meta.get("distance").map(|d| d.trim().parse::<u32>().with_context(|| format!("distance: {d:?}"))).transpose()?;
        let stimulus = design.meta.get("stimulus").cloned();
        let clock = Clock::from_design(design)?;
        let c = Circuit { clock, kind, inputs, outputs, payloads, travel, distance, stimulus };
        c.check_ports()?;
        Ok(c)
    }

    /// The ports the kind needs.
    fn check_ports(&self) -> Result<()> {
        let Some(k) = &self.kind else {
            return Ok(());
        };
        let names: Vec<&str> = self.inputs.iter().map(|i| i.name.as_str()).collect();
        match k.inputs_needed() {
            Some(need) => {
                let mut want: Vec<&str> = need.to_vec();
                want.sort();
                if names != want {
                    bail!("{}: needs inputs {} (port input.<name>), has {:?}", k.name(), want.join(", "), names);
                }
            }
            None => match k {
                Kind::FlyingMachine => {}
                Kind::Gate(_) if names.len() == 1 || names.len() == 2 => {}
                Kind::Gate(_) => bail!("gate: needs 1 or 2 inputs (port input.a, port input.b), has {}", names.len()),
                _ if names.len() == 1 => {}
                _ => bail!("{}: needs exactly one input (port input:), has {}", k.name(), names.len()),
            },
        }
        match k {
            Kind::PistonExtender(_) => {
                if self.payloads.is_empty() {
                    bail!("piston-extender: needs `port payload: x y z` (the pushed block)");
                }
                if self.travel.is_none() {
                    bail!("piston-extender: needs `travel: dx dy dz` (the push direction)");
                }
            }
            Kind::FlyingMachine => {
                if self.travel.is_none() {
                    bail!("flying-machine: needs `travel: dx dy dz`");
                }
            }
            _ => {
                if self.outputs.is_empty() {
                    bail!("{}: needs `port output: x y z`", k.name());
                }
            }
        }
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.clock.design.name
    }

    /// The output cells that get the judge's consumer ([`CONSUMER`]): air in the design and not
    /// an input cell, for every kind read through its outputs (piston extenders and flying
    /// machines are read through their payload and travel), and not on a piston's line (within 13
    /// cells in front of any piston: a pushed or pulled block may move into the cell, and the
    /// block there is what the output reads).
    pub fn consumer_cells(&self) -> Vec<Pos> {
        if matches!(self.kind, Some(Kind::PistonExtender(_)) | Some(Kind::FlyingMachine)) {
            return Vec::new();
        }
        let ins: BTreeSet<Pos> = self.inputs.iter().map(|i| i.pos).collect();
        let mut lines: BTreeSet<Pos> = BTreeSet::new();
        for (p, s) in &self.clock.design.blocks {
            if !(s.starts_with("minecraft:piston[") || s.starts_with("minecraft:sticky_piston[")) {
                continue;
            }
            let f = match s.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
                Some("north") => [0, 0, -1],
                Some("south") => [0, 0, 1],
                Some("east") => [1, 0, 0],
                Some("west") => [-1, 0, 0],
                Some("up") => [0, 1, 0],
                Some("down") => [0, -1, 0],
                _ => continue,
            };
            for k in 1..=13 {
                lines.insert([p[0] + k * f[0], p[1] + k * f[1], p[2] + k * f[2]]);
            }
        }
        let mut v: Vec<Pos> = self.outputs.iter().map(|o| o.pos).filter(|p| !ins.contains(p) && !lines.contains(p) && !self.clock.design.blocks.contains_key(p)).collect();
        v.sort();
        v.dedup();
        v
    }

    /// The circuit as the judge builds it: its consumer ([`CONSUMER`]) in every consumer cell
    /// (not one of the design's blocks: not counted, not part of its type).
    pub fn with_consumers(&self) -> Result<Circuit> {
        let cells = self.consumer_cells();
        if cells.is_empty() {
            return Ok(self.clone());
        }
        let mut d = self.clock.design.clone();
        for p in cells {
            d.blocks.insert(p, CONSUMER.to_string());
        }
        Circuit::from_design(d)
    }

    /// The design's blocks, except piston heads (part of their piston) and the blocks in input
    /// cells (a lever or button there is the stimulus device), plus design entities.
    pub fn blocks(&self) -> usize {
        let ins: BTreeSet<Pos> = self.inputs.iter().map(|i| i.pos).collect();
        self.clock.design.blocks.iter().filter(|(p, s)| !ins.contains(*p) && !s.starts_with("minecraft:piston_head")).count() + self.clock.design.entities.len()
    }

    /// The kinds of blocks it is built with (as `Clock::kinds`), input cells left out.
    pub fn kinds(&self) -> BTreeSet<String> {
        let mut d = self.clock.design.clone();
        for i in &self.inputs {
            d.blocks.remove(&i.pos);
        }
        match Clock::from_design(d) {
            Ok(c) => c.kinds(),
            Err(_) => self.clock.kinds(),
        }
    }

    /// The box of its blocks and every port (inputs, outputs, payloads, the kick cell).
    pub fn bounds(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.clock.bounds();
        for p in self.clock.design.ports.values() {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        (lo, hi)
    }

    /// The same circuit moved by `d`.
    pub fn translated(&self, d: Pos) -> Result<Circuit> {
        Circuit::from_design(self.clock.design.translated(d))
    }

    /// The circuit turned `turns` quarter turns clockwise (seen from above) around the origin: the
    /// game turns every block state, the ports and the travel vector turn with it (custom input
    /// states are written as given).
    pub fn rotated(&self, w: &mut Worker, turns: u8) -> Result<Circuit> {
        let rc = clock::verify::rotated(w, &self.clock, turns)?;
        let mut d = rc.design;
        if let Some(t) = self.travel {
            let r = clock::rotate_pos(t, turns);
            d.meta.insert("travel".into(), format!("{} {} {}", r[0], r[1], r[2]));
        }
        Circuit::from_design(d)
    }

    /// The judge's inputs for a schedule: at each input edge, the action applied between ticks
    /// (input order within a tick).
    pub fn actions(&self, s: &stimulus::Schedule) -> Vec<(u32, Action)> {
        let mut v = Vec::new();
        for (i, inp) in self.inputs.iter().enumerate() {
            for &(t, on) in s.edges.get(i).map(|e| e.as_slice()).unwrap_or(&[]) {
                let a = match (&inp.how, on) {
                    (InputHow::Block { on: st, .. }, true) => Action::Place { pos: inp.pos, state: st.clone(), nbt: None },
                    (InputHow::Block { off, .. }, false) => {
                        if mctas::blockstate::is_air(off) {
                            Action::Clear { pos: inp.pos }
                        } else {
                            Action::Place { pos: inp.pos, state: off.clone(), nbt: None }
                        }
                    }
                    (InputHow::Lever, _) => Action::Use { pos: inp.pos },
                    (InputHow::Button, true) => Action::Use { pos: inp.pos },
                    (InputHow::Button, false) => continue,
                };
                v.push((t, a));
            }
        }
        v.sort_by_key(|(t, _)| *t);
        v
    }
}

/// An input port's driver from its arguments, checked against the design's block in that cell.
fn parse_input(d: &mctas::design::Design, pos: Pos, args: &str) -> Result<InputHow> {
    let here = d.blocks.get(&pos).map(|s| s.as_str());
    let words: Vec<&str> = args.split_whitespace().collect();
    match words.as_slice() {
        ["lever"] => {
            let s = here.ok_or_else(|| anyhow!("lever input at {pos:?}: no design block there"))?;
            if !s.starts_with("minecraft:lever") {
                bail!("lever input at {pos:?}: the design has {s} there");
            }
            if s.contains("powered=true") {
                bail!("lever input at {pos:?}: the lever must be off at rest (powered=false)");
            }
            Ok(InputHow::Lever)
        }
        ["button"] => {
            let s = here.ok_or_else(|| anyhow!("button input at {pos:?}: no design block there"))?;
            if !s.contains("_button") {
                bail!("button input at {pos:?}: the design has {s} there");
            }
            Ok(InputHow::Button)
        }
        _ => {
            let (on, off) = match words.as_slice() {
                [] => (INPUT_ON.to_string(), "minecraft:air".to_string()),
                [on] => (mctas::blockstate::BlockState::parse(on)?.to_string(), "minecraft:air".to_string()),
                [on, off] => (mctas::blockstate::BlockState::parse(on)?.to_string(), mctas::blockstate::BlockState::parse(off)?.to_string()),
                _ => bail!("want `lever`, `button` or `<on-state> [<off-state>]`, got {args:?}"),
            };
            let rest = here.unwrap_or("minecraft:air");
            if !trace::state_matches(rest, &off) {
                bail!("input at {pos:?}: the design must hold its off-state {off} there (it has {rest})");
            }
            Ok(InputHow::Block { on, off })
        }
    }
}
