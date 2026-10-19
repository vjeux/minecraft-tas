//! Building candidate designs: cells, directions, the sensors and emitters of the vibration family,
//! the start kick, and the structure key (up to translation and the 4 horizontal rotations).

use clock::{Clock, Pos};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use std::collections::{BTreeMap, BTreeSet};

pub const DIRS: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1], [0, 1, 0], [0, -1, 0]];

pub fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub fn neg(a: Pos) -> Pos {
    [-a[0], -a[1], -a[2]]
}
pub fn dist2(a: Pos, b: Pos) -> i32 {
    let d = sub(a, b);
    d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
}
/// floor(distance) as the game computes it (`(float) Math.sqrt(distSqr)`), at least 1: the travel
/// time of a vibration.
pub fn travel(a: Pos, b: Pos) -> i32 {
    (((dist2(a, b) as f64).sqrt() as f32) as f64).floor().max(1.0) as i32
}

/// The block-state name of a unit direction.
pub fn dir_name(d: Pos) -> &'static str {
    match d {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 0, 1] => "south",
        [0, 0, -1] => "north",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        _ => panic!("not a unit direction: {d:?}"),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sensor {
    /// `calibrated_sculk_sensor`: active 10 gt, cooldown 10, radius 16, no output into its back.
    Cal,
    /// `sculk_sensor`: active 30, cooldown 10, radius 8.
    Plain,
}

impl Sensor {
    pub fn parse_list(s: &str) -> Vec<Sensor> {
        match s {
            "cal" => vec![Sensor::Cal],
            "plain" => vec![Sensor::Plain],
            _ => vec![Sensor::Cal, Sensor::Plain],
        }
    }
    /// The state; `back` is where its back points (calibrated only: no output there, and a
    /// signal from there sets its frequency filter).
    pub fn state(self, back: Pos) -> String {
        match self {
            Sensor::Cal => format!(
                "minecraft:calibrated_sculk_sensor[facing={},power=0,sculk_sensor_phase=inactive,waterlogged=false]",
                dir_name(neg(back))
            ),
            Sensor::Plain => "minecraft:sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]".into(),
        }
    }
    pub fn tag(self) -> &'static str {
        match self {
            Sensor::Cal => "cal",
            Sensor::Plain => "plain",
        }
    }
}

/// A vibration source driven by redstone (each one read in the source: `NoteBlock.playNote`,
/// `PistonBaseBlock.triggerEvent`, `TrapDoorBlock` / `FenceGateBlock.neighborChanged`,
/// `DispenserBlock.dispenseFrom` with nothing inside).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emitter {
    /// Plays on a rising edge (needs air above it): `note_block_play`, frequency 10.
    Note,
    /// Opens / closes on both edges (10 / 9); placed open + powered with no power, it closes on
    /// its placement update: self-starting.
    Trapdoor,
    IronTrapdoor,
    Gate,
    /// Extends / retracts (10 / 9); its head needs the cell in front.
    Piston(Pos),
    Sticky(Pos),
    /// A piston placed extended (with its head) and unpowered: retracts on its placement update.
    PistonExt(Pos),
    /// Empty dispenser: `block_activate` 4 gt after a rising edge.
    Dispenser,
}

impl Emitter {
    pub fn parse_list(s: &str) -> Vec<Emitter> {
        let mut v = Vec::new();
        for w in s.split(',') {
            match w {
                "note" => v.push(Emitter::Note),
                "trapdoor" => v.push(Emitter::Trapdoor),
                "irontrapdoor" => v.push(Emitter::IronTrapdoor),
                "gate" => v.push(Emitter::Gate),
                "piston" => v.extend(DIRS.iter().map(|d| Emitter::Piston(*d))),
                "sticky" => v.extend(DIRS.iter().map(|d| Emitter::Sticky(*d))),
                "pistonext" => v.extend(DIRS.iter().map(|d| Emitter::PistonExt(*d))),
                "dispenser" => v.push(Emitter::Dispenser),
                "all" => v.extend(Emitter::parse_list("note,trapdoor,irontrapdoor,gate,piston,sticky,pistonext,dispenser")),
                "" => {}
                other => panic!("unknown emitter {other}"),
            }
        }
        v
    }
    pub fn self_starting(self) -> bool {
        matches!(self, Emitter::Trapdoor | Emitter::IronTrapdoor | Emitter::Gate | Emitter::PistonExt(_))
    }
    pub fn tag(self) -> String {
        match self {
            Emitter::Note => "note".into(),
            Emitter::Trapdoor => "trapdoor".into(),
            Emitter::IronTrapdoor => "irontrapdoor".into(),
            Emitter::Gate => "gate".into(),
            Emitter::Piston(d) => format!("piston-{}", dir_name(d)),
            Emitter::Sticky(d) => format!("sticky-{}", dir_name(d)),
            Emitter::PistonExt(d) => format!("pistonext-{}", dir_name(d)),
            Emitter::Dispenser => "dispenser".into(),
        }
    }
}

/// A candidate under construction: blocks, cells that must stay air, the output and the kick.
#[derive(Clone, Debug, Default)]
pub struct Build {
    pub blocks: BTreeMap<Pos, String>,
    pub air: BTreeSet<Pos>,
    pub output: Option<Pos>,
    /// `port start:` (position, kick words).
    pub start: Option<(Pos, String)>,
}

impl Build {
    pub fn free(&self, p: Pos) -> bool {
        !self.blocks.contains_key(&p) && !self.air.contains(&p)
    }
    /// Put a block in a free cell (false if taken).
    pub fn put(&mut self, p: Pos, s: impl Into<String>) -> bool {
        if !self.free(p) {
            return false;
        }
        self.blocks.insert(p, s.into());
        true
    }
    pub fn keep_air(&mut self, p: Pos) -> bool {
        if self.blocks.contains_key(&p) {
            return false;
        }
        self.air.insert(p);
        true
    }
    /// Place an emitter at `p`; for a kicked emitter also choose the kick cell (a 1 gt redstone
    /// block in a free cell next to it touching nothing else). False if it does not fit.
    pub fn emitter(&mut self, p: Pos, e: Emitter) -> bool {
        let ok = match e {
            Emitter::Note => self.put(p, "minecraft:note_block[instrument=harp,note=0,powered=false]") && self.keep_air(add(p, [0, 1, 0])),
            Emitter::Trapdoor => self.put(p, "minecraft:oak_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]"),
            Emitter::IronTrapdoor => self.put(p, "minecraft:iron_trapdoor[facing=north,half=bottom,open=true,powered=true,waterlogged=false]"),
            Emitter::Gate => self.put(p, "minecraft:oak_fence_gate[facing=north,in_wall=false,open=true,powered=true]"),
            Emitter::Piston(d) => self.put(p, format!("minecraft:piston[extended=false,facing={}]", dir_name(d))) && self.keep_air(add(p, d)),
            Emitter::Sticky(d) => self.put(p, format!("minecraft:sticky_piston[extended=false,facing={}]", dir_name(d))) && self.keep_air(add(p, d)),
            Emitter::PistonExt(d) => {
                self.put(p, format!("minecraft:piston[extended=true,facing={}]", dir_name(d)))
                    && self.put(add(p, d), format!("minecraft:piston_head[facing={},short=false,type=normal]", dir_name(d)))
            }
            Emitter::Dispenser => self.put(p, "minecraft:dispenser[facing=up,triggered=false]"),
        };
        if !ok {
            return false;
        }
        if e.self_starting() {
            return true;
        }
        // The kick: a free cell beside the emitter whose other neighbors are all empty (pistons
        // are not powered through their face).
        for d in DIRS {
            let k = add(p, d);
            if let Emitter::Piston(f) | Emitter::Sticky(f) = e {
                if d == f {
                    continue;
                }
            }
            if !self.free(k) {
                continue;
            }
            if DIRS.iter().all(|d2| {
                let q = add(k, *d2);
                q == p || !self.blocks.contains_key(&q)
            }) {
                self.start = Some((k, "minecraft:redstone_block 1".into()));
                return true;
            }
        }
        false
    }
    pub fn clock(&self, name: &str) -> Option<Clock> {
        let mut d = Design { name: name.to_string(), ..Default::default() };
        d.blocks = self.blocks.clone();
        if let Some(o) = self.output {
            d.ports.insert("output".into(), o);
        }
        if let Some((p, args)) = &self.start {
            d.ports.insert("start".into(), *p);
            d.meta.insert("port start".into(), args.clone());
        }
        Clock::from_design(d).ok()
    }
}

/// The structure up to translation and the 4 horizontal rotations: the smallest text of the
/// rotated, re-anchored block list.
pub fn structure_key(blocks: &BTreeMap<Pos, String>) -> String {
    let mut best: Option<String> = None;
    for t in 0..4u8 {
        let mut v: Vec<(Pos, String)> = blocks
            .iter()
            .map(|(p, s)| {
                let q = clock::rotate_pos(*p, t);
                let st = BlockState::parse(s).map(|b| b.rotate_y(t as i32).to_string()).unwrap_or_else(|_| s.clone());
                (q, st)
            })
            .collect();
        let lo = [0, 1, 2].map(|i| v.iter().map(|(p, _)| p[i]).min().unwrap_or(0));
        for (p, _) in v.iter_mut() {
            *p = sub(*p, lo);
        }
        v.sort();
        let text: String = v.iter().map(|(p, s)| format!("{},{},{}={};", p[0], p[1], p[2], s)).collect();
        if best.as_ref().is_none_or(|b| text < *b) {
            best = Some(text);
        }
    }
    best.unwrap_or_default()
}
