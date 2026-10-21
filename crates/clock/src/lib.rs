//! The redstone clock study ([docs/clocks.md](../../../docs/clocks.md)).
//!
//! A clock is a design (the door design text format, `crates/mctas/src/design.rs`) with no door
//! and no lever, an optional `port output: x y z` (where its signal is read) and an optional one-time
//! start kick `port start: x y z <kick>`:
//!
//! * `last`: the design's own block in that cell is placed LAST, with updates (a player placing
//!   it), after the silent placement of everything else;
//! * `use [T]`: one right-click on the design block there (lever, button, note block, repeater,
//!   trapdoor...); with `T`, a second right-click `T` gt later (a lever pulse);
//! * `<block state> <T>`: a temporary block placed with updates in an AIR cell, removed with
//!   updates `T` gt later (`minecraft:redstone_block 1`).
//!
//! No start line = self-starting: the door judge's placement (raw writes, a shape update of every
//! wire, a `self` update of every block) and nothing else. `placement: player` places every block
//! with updates in file order instead (flagged).
//!
//! Modules: [`wave`] (period, waveform and family from a sequence of states: shared by the redsim
//! detector and the real-game judge), [`sim`] (`mctas clock sim`: the exact redsim detector),
//! [`verify`] (`mctas clock verify`: the judge in the real game).

pub mod anneal;
pub mod canon;
pub mod catalog;
pub mod census;
pub mod extend;
pub mod gaps;
pub mod ident;
pub mod kits;
pub mod period_search;
pub mod shot;
pub mod sim;
pub mod usable;
pub mod verify;
pub mod vibration;
pub mod video;
pub mod wave;
pub mod wireable;

use anyhow::{Context, Result, anyhow, bail};
pub use mctas::design::Design;
use std::path::Path;

pub type Pos = [i32; 3];

/// The one-time start kick (`port start: x y z <kick>`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kick {
    /// The design block in this cell is placed last, with updates.
    Last { pos: Pos },
    /// Right-click the design block in this cell; with `again`, right-click it again that many gt
    /// later.
    Use { pos: Pos, again: Option<u32> },
    /// A temporary block placed with updates in an air cell, removed with updates `ticks` gt later.
    Place { pos: Pos, state: String, ticks: u32 },
}

impl Kick {
    pub fn pos(&self) -> Pos {
        match self {
            Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
        }
    }

    /// The text after the position on the `port start:` line.
    pub fn args(&self) -> String {
        match self {
            Kick::Last { .. } => "last".into(),
            Kick::Use { again: None, .. } => "use".into(),
            Kick::Use { again: Some(t), .. } => format!("use {t}"),
            Kick::Place { state, ticks, .. } => format!("{state} {ticks}"),
        }
    }

    /// Parse the kick and its optional trailing `at N` (the kick starts after N ticks).
    fn parse(pos: Pos, args: &str) -> Result<(Kick, u32)> {
        let mut words: Vec<&str> = args.split_whitespace().collect();
        let mut at = 0;
        if words.len() >= 2 && words[words.len() - 2] == "at" {
            let n = words[words.len() - 1];
            at = n.parse().with_context(|| format!("bad kick tick {n:?}"))?;
            words.truncate(words.len() - 2);
        }
        let k = match words.as_slice() {
            ["last"] => Kick::Last { pos },
            ["use"] => Kick::Use { pos, again: None },
            ["use", t] => Kick::Use { pos, again: Some(t.parse().with_context(|| format!("bad tick count {t:?}"))?) },
            [state, t] => {
                let ticks: u32 = t.parse().with_context(|| format!("bad tick count {t:?}"))?;
                if ticks == 0 {
                    bail!("a temporary start block stays at least 1 gt");
                }
                let state = mctas::blockstate::BlockState::parse(state)?.to_string();
                Kick::Place { pos, state, ticks }
            }
            _ => bail!("port start: x y z (last | use [T] | <block state> <T>) [at N], got {args:?}"),
        };
        Ok((k, at))
    }
}

/// One input of the start kick, applied between ticks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// Place a block with updates (a player placing it), then merge its block-entity data (a
    /// `last` block's `nbt:` line).
    Place { pos: Pos, state: String, nbt: Option<String> },
    /// Right-click the block there.
    Use { pos: Pos },
    /// Right-click the item frame in that cell with an empty hand: its item turns one step (the
    /// lock judge's input device; the game only, redsim has no entities).
    UseEntity { pos: Pos },
    /// Remove the block there with updates (no drop).
    Clear { pos: Pos },
    /// Change the block there with SHAPE updates only (flags 2: no block update to the
    /// neighbors), the way crops grow or a dispenser marks itself triggered (the BUD judge).
    ShapeSet { pos: Pos, state: String },
    /// A pure BLOCK update around the cell (`Level.updateNeighborsAt`), nothing changes (the BUD
    /// judge).
    Update { pos: Pos },
    /// A game event at the cell's center (`Level.gameEvent`, no entity, no affected state): what
    /// sculk sensors hear (the circuit judge's vibration test: an input device's events, ambient
    /// footsteps and block changes).
    GameEvent { pos: Pos, event: &'static str },
    /// An item entity summoned at the cell's center (x + 0.5, y + 0.5, z + 0.5), at rest: `item`
    /// is its stack as SNBT (`{id:"minecraft:cobblestone",count:1}`). A stray item a player or an
    /// explosion drops (the real-use audit's ITEM INTRUSION: a hopper with an open top takes it).
    Drop { pos: Pos, item: String },
}

impl Action {
    /// A [`Action::Drop`]'s entity: kind, position and data (the game's `summon`, redsim's
    /// `entity::spawn`).
    pub fn drop_entity(pos: Pos, item: &str) -> (String, [f64; 3], String) {
        ("minecraft:item".to_string(), [pos[0] as f64 + 0.5, pos[1] as f64 + 0.5, pos[2] as f64 + 0.5], format!("{{Item:{item},Motion:[0.0d,0.0d,0.0d]}}"))
    }
}

/// How the blocks are placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Placement {
    /// The door judge's placement: raw writes, a shape update of every wire, a `self` update of
    /// every block (no observer pulses, no piston fires from the placement itself).
    Judge,
    /// Every block placed with updates (like a player), in file order.
    Player,
}

/// A clock design.
#[derive(Clone, Debug)]
pub struct Clock {
    pub design: Design,
    /// Where the output signal is read (`port output:`).
    pub output: Option<Pos>,
    pub kick: Option<Kick>,
    /// The tick the kick starts at (`at N`; 0 = right after placement).
    pub kick_at: u32,
    pub placement: Placement,
}

impl Clock {
    pub fn load(path: &Path) -> Result<Clock> {
        let d = Design::load(path)?;
        Clock::from_design(d).with_context(|| format!("clock design {}", path.display()))
    }

    pub fn from_design(design: Design) -> Result<Clock> {
        if design.blocks.is_empty() {
            bail!("empty design");
        }
        let output = design.ports.get("output").copied();
        let (kick, kick_at) = match (design.ports.get("start"), design.meta.get("port start")) {
            (None, None) => (None, 0),
            (Some(p), Some(args)) => {
                let (k, at) = Kick::parse(*p, args)?;
                (Some(k), at)
            }
            (Some(_), None) => bail!("port start needs a kick after its position: last | use [T] | <block state> <T>"),
            (None, Some(_)) => unreachable!(),
        };
        match &kick {
            Some(Kick::Last { pos }) | Some(Kick::Use { pos, .. }) if !design.blocks.contains_key(pos) => {
                bail!("port start {pos:?}: no design block there to place last / use")
            }
            Some(Kick::Place { pos, .. }) if design.blocks.contains_key(pos) => {
                bail!("port start {pos:?}: a temporary start block needs an air cell")
            }
            _ => {}
        }
        let placement = match design.meta.get("placement").map(|s| s.as_str()) {
            None | Some("judge") => Placement::Judge,
            Some("player") => Placement::Player,
            Some(v) => bail!("placement: judge | player, got {v:?}"),
        };
        Ok(Clock { design, output, kick, kick_at, placement })
    }

    pub fn name(&self) -> &str {
        &self.design.name
    }

    /// Blocks placed: every block of the design except piston heads (part of their piston), as
    /// the door boards count; a temporary start block is not counted.
    /// The kinds of blocks the clock is built with (ids without `minecraft:`; a wall torch is a
    /// torch, every waxed copper bulb a copper bulb, a piston head part of its piston) plus its
    /// entities' kinds: the catalog's dominance fold compares these sets.
    pub fn kinds(&self) -> std::collections::BTreeSet<String> {
        let norm = |s: &str| -> Option<String> {
            let id = s.split('[').next().unwrap_or(s);
            let id = id.strip_prefix("minecraft:").unwrap_or(id);
            if id == "piston_head" || id == "moving_piston" || id == "air" {
                return None;
            }
            Some(if id == "redstone_wall_torch" {
                "redstone_torch".to_string()
            } else if id.contains("copper_bulb") {
                "copper_bulb".to_string()
            } else {
                id.to_string()
            })
        };
        self.design.blocks.values().filter_map(|s| norm(s)).chain(self.design.entities.iter().filter_map(|e| norm(&e.kind))).collect()
    }

    /// Placed pieces: blocks (a piston head is part of its piston) plus design entities (a minecart
    /// counts one).
    pub fn blocks(&self) -> usize {
        self.design.blocks.values().filter(|s| !s.starts_with("minecraft:piston_head")).count() + self.design.entities.len()
    }

    /// The design's bounding box with the start cell.
    pub fn bounds(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.design.bounds().unwrap();
        if let Some(k) = &self.kick {
            grow(&mut lo, &mut hi, k.pos());
        }
        (lo, hi)
    }

    /// How it starts, for reports: `self-start`, `last x,y,z`, `use x,y,z`, `<state> T gt at x,y,z`.
    pub fn start_label(&self) -> String {
        let p = |q: Pos| format!("{},{},{}", q[0], q[1], q[2]);
        let base = match &self.kick {
            None => "self-start".to_string(),
            Some(Kick::Last { pos }) => format!("kick: last {}", p(*pos)),
            Some(Kick::Use { pos, again: None }) => format!("kick: use {}", p(*pos)),
            Some(Kick::Use { pos, again: Some(t) }) => format!("kick: use {} twice {t} gt apart", p(*pos)),
            Some(Kick::Place { pos, state, ticks }) => format!("kick: {} {ticks} gt at {}", short(state), p(*pos)),
        };
        let base = if self.kick_at > 0 { format!("{base} at tick {}", self.kick_at) } else { base };
        if self.placement == Placement::Player { format!("{base}, player placement") } else { base }
    }

    /// The same clock moved by `d`.
    pub fn translated(&self, d: Pos) -> Clock {
        let mv = |p: Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        Clock {
            design: self.design.translated(d),
            output: self.output.map(mv),
            kick: self.kick.as_ref().map(|k| match k {
                Kick::Last { pos } => Kick::Last { pos: mv(*pos) },
                Kick::Use { pos, again } => Kick::Use { pos: mv(*pos), again: *again },
                Kick::Place { pos, state, ticks } => Kick::Place { pos: mv(*pos), state: state.clone(), ticks: *ticks },
            }),
            kick_at: self.kick_at,
            placement: self.placement,
        }
    }

    /// The kick's inputs with the tick each is applied at (between ticks; tick 0 = right after
    /// placement, before the first tick).
    pub fn actions(&self) -> Vec<(u32, Action)> {
        let at = self.kick_at;
        match &self.kick {
            None => Vec::new(),
            Some(Kick::Last { pos }) => vec![(at, Action::Place { pos: *pos, state: self.design.blocks[pos].clone(), nbt: self.design.block_entities.get(pos).cloned() })],
            Some(Kick::Use { pos, again }) => {
                let mut v = vec![(at, Action::Use { pos: *pos })];
                if let Some(t) = again {
                    v.push((at + t, Action::Use { pos: *pos }));
                }
                v
            }
            Some(Kick::Place { pos, state, ticks }) => vec![(at, Action::Place { pos: *pos, state: state.clone(), nbt: None }), (at + ticks, Action::Clear { pos: *pos })],
        }
    }

    /// The tick from which the clock runs with no input at all (the end of the kick).
    pub fn free_from(&self) -> u32 {
        self.actions().iter().map(|(t, _)| *t).max().unwrap_or(0)
    }

    /// The cell left out of the silent placement (a `last` kick's block).
    pub fn withheld(&self) -> Option<Pos> {
        match &self.kick {
            Some(Kick::Last { pos }) => Some(*pos),
            _ => None,
        }
    }

    /// The design as text (ports and the kick included).
    pub fn to_text(&self) -> String {
        self.design.to_text()
    }

    /// A key of everything that decides the clock's behavior (blocks, ports, kick, placement).
    pub fn content_key(&self) -> String {
        format!("{}|{:?}|{}|{:?}", self.design.content_key(), self.kick, self.kick_at, self.placement)
    }
}

pub(crate) fn grow(lo: &mut Pos, hi: &mut Pos, p: Pos) {
    for i in 0..3 {
        lo[i] = lo[i].min(p[i]);
        hi[i] = hi[i].max(p[i]);
    }
}

/// `minecraft:observer[facing=up,powered=false]` -> `observer[facing=up,powered=false]`.
pub fn short(state: &str) -> &str {
    state.strip_prefix("minecraft:").unwrap_or(state)
}

/// `p` turned `turns` quarter turns clockwise seen from above around the origin (north -> east),
/// like the game's `Rotation.CLOCKWISE_90` (the door judge's `rotate_pos`).
pub fn rotate_pos(p: Pos, turns: u8) -> Pos {
    let mut q = p;
    for _ in 0..turns % 4 {
        q = [-q[2], q[1], q[0]];
    }
    q
}

/// An entity's SNBT with its `Motion:[x,y,z]` turned `turns` quarter turns clockwise like
/// [`rotate_pos`] turns positions ((x, z) -> (-z, x)); unchanged without a readable Motion.
pub fn turn_motion(snbt: &str, turns: u8) -> String {
    let Some(i) = snbt.find("Motion:[") else {
        return snbt.to_string();
    };
    let start = i + "Motion:[".len();
    let Some(len) = snbt[start..].find(']') else {
        return snbt.to_string();
    };
    let parts: Vec<&str> = snbt[start..start + len].split(',').map(str::trim).collect();
    let num = |s: &str| s.trim_end_matches(['d', 'D', 'f', 'F']).parse::<f64>().ok();
    let (Some(mut x), Some(y), Some(mut z)) = (parts.first().and_then(|s| num(s)), parts.get(1).and_then(|s| num(s)), parts.get(2).and_then(|s| num(s))) else {
        return snbt.to_string();
    };
    if parts.len() != 3 {
        return snbt.to_string();
    }
    for _ in 0..turns % 4 {
        (x, z) = (-z, x);
    }
    let f = |v: f64| if v == 0.0 { "0.0d".to_string() } else { format!("{v}d") };
    format!("{}{},{},{}{}", &snbt[..start], f(x), f(y), f(z), &snbt[start + len..])
}

/// `n` deterministic pseudo-random extra test positions on top of the default four (the door
/// judge's generator: the same positions).
pub fn extra_offsets(n: u32) -> Vec<Pos> {
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    let mut next = |m: i64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % (2 * m as u64 + 1)) as i32 - m as i32
    };
    (0..n).map(|_| [next(13), next(40), next(13)]).collect()
}

/// The judge's test positions: the door judge's default four, then its extra positions.
pub fn offsets(n: usize) -> Vec<Pos> {
    let mut v: Vec<Pos> = vec![[0, 0, 0], [-7, 3, 5], [5, -11, -6], [-3, 40, 2]];
    v.extend(extra_offsets(n.saturating_sub(4) as u32));
    v.truncate(n.max(1));
    v
}

/// A duration in game ticks with its real time (20 gt = 1 s): `20 gt (1 s)`, `30 gt (1.5 s)`,
/// `2304 gt (1 min 55.2 s)`, minutes past 60 s, hours past 60 min.
pub fn gt_time(gt: u32) -> String {
    format!("{gt} gt ({})", real_time(gt))
}

/// Just the real time of `gt` game ticks: `1.5 s`, `1 min 55.2 s`, `2 h 5 min`.
pub fn real_time(gt: u32) -> String {
    let num = |x: f64| -> String {
        let s = format!("{x:.2}");
        let s = s.trim_end_matches('0').trim_end_matches('.');
        s.to_string()
    };
    let secs = gt as f64 / 20.0;
    if secs < 60.0 {
        return format!("{} s", num(secs));
    }
    let total_min = (secs / 60.0).floor();
    let s = secs - total_min * 60.0;
    let (h, m) = ((total_min / 60.0).floor(), total_min % 60.0);
    let mut parts = Vec::new();
    if h > 0.0 {
        parts.push(format!("{h} h"));
    }
    if m > 0.0 || h == 0.0 {
        parts.push(format!("{m} min"));
    }
    if s > 0.0 {
        parts.push(format!("{} s", num(s)));
    }
    parts.join(" ")
}

/// Parse a block state with redsim, saying which blocks it does not model.
pub fn redsim_block(state: &str) -> Result<redsim::Block> {
    redsim::Block::parse(state).map_err(|e| anyhow!("{e}"))
}
