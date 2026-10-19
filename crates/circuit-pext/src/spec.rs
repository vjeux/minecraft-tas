//! A piston-extender design: the door/clock design format with clockinfra's circuit headers
//! (2026-10-19, docs/circuits.md):
//!
//! ```text
//! name: pext-n2-example
//! circuit: piston-extender 2        # the reach N
//! travel: 1 0 0                     # the push direction (a unit vector)
//! port input: 0 1 0 lever           # the design's lever (toggled) or button (pressed) there
//! port payload: 3 0 0               # the pushed block (its state must be unique in the design)
//! ```
//!
//! Blocks counted: every design block except the input device and piston heads (part of their
//! piston); the payload counts.

use anyhow::{Context, Result, anyhow, bail};
use mctas::design::Design;
use std::path::Path;

pub type Pos = [i32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind {
    /// A lever: on edge = extend, off edge = retract.
    Lever,
    /// A button: one press = extend, then come back by itself.
    Button,
}

impl InputKind {
    pub fn word(self) -> &'static str {
        match self {
            InputKind::Lever => "lever",
            InputKind::Button => "button",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Pext {
    pub design: Design,
    pub reach: i32,
    pub travel: Pos,
    pub input: Pos,
    pub input_kind: InputKind,
    pub payload: Pos,
    pub payload_state: String,
}

pub fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn scale(a: Pos, k: i32) -> Pos {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// `travel:` as a unit vector: `1 0 0` or a direction name (`east`, `up`, ...).
pub fn parse_travel(s: &str) -> Result<Pos> {
    let named = match s.trim() {
        "east" => Some([1, 0, 0]),
        "west" => Some([-1, 0, 0]),
        "up" => Some([0, 1, 0]),
        "down" => Some([0, -1, 0]),
        "south" => Some([0, 0, 1]),
        "north" => Some([0, 0, -1]),
        _ => None,
    };
    if let Some(v) = named {
        return Ok(v);
    }
    let v: Vec<i32> = s.split_whitespace().map(|w| w.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("travel: dx dy dz, got {s:?}"))?;
    if v.len() != 3 || v.iter().map(|x| x.abs()).sum::<i32>() != 1 {
        bail!("travel: a unit vector along one axis (dx dy dz), got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

pub fn travel_name(t: Pos) -> &'static str {
    match t {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        [0, 0, -1] => "north",
        _ => "?",
    }
}

impl Pext {
    pub fn load(path: &Path) -> Result<Pext> {
        let d = Design::load(path)?;
        Pext::from_design(d).with_context(|| format!("piston-extender design {}", path.display()))
    }

    pub fn from_design(design: Design) -> Result<Pext> {
        let kind = design.meta.get("circuit").ok_or_else(|| anyhow!("no `circuit: piston-extender N` header"))?;
        let mut words = kind.split_whitespace();
        if words.next() != Some("piston-extender") {
            bail!("circuit: {kind:?} is not a piston-extender");
        }
        let reach: i32 = words.next().ok_or_else(|| anyhow!("circuit: piston-extender N (the reach)"))?.parse().context("the reach N")?;
        if reach < 1 {
            bail!("reach {reach} < 1");
        }
        let travel = parse_travel(design.meta.get("travel").ok_or_else(|| anyhow!("no `travel: dx dy dz` header"))?)?;
        let input = *design.ports.get("input").ok_or_else(|| anyhow!("no `port input: x y z lever|button`"))?;
        let input_word = design.meta.get("port input").map(|s| s.trim().to_string()).unwrap_or_default();
        let at_input = design.blocks.get(&input).cloned().unwrap_or_default();
        let input_kind = match input_word.as_str() {
            "lever" => InputKind::Lever,
            "button" => InputKind::Button,
            "" if at_input.contains("lever") => InputKind::Lever,
            "" if at_input.contains("button") => InputKind::Button,
            w => bail!("port input: x y z lever|button, got {w:?}"),
        };
        match input_kind {
            InputKind::Lever if !at_input.contains("minecraft:lever") => bail!("port input {input:?}: no lever there ({at_input:?})"),
            InputKind::Button if !at_input.contains("_button") => bail!("port input {input:?}: no button there ({at_input:?})"),
            _ => {}
        }
        if at_input.contains("powered=true") {
            bail!("the input device must be off at rest");
        }
        let payload = *design.ports.get("payload").ok_or_else(|| anyhow!("no `port payload: x y z`"))?;
        let payload_state = design.blocks.get(&payload).cloned().ok_or_else(|| anyhow!("port payload {payload:?}: no block there"))?;
        let twins: Vec<Pos> = design.blocks.iter().filter(|(p, s)| **p != payload && **s == payload_state).map(|(p, _)| *p).collect();
        if !twins.is_empty() {
            bail!("the payload's state {payload_state} must be unique in the design (also at {twins:?}): pick a block used nowhere else (gold_block, emerald_block...)");
        }
        Ok(Pext { design, reach, travel, input, input_kind, payload, payload_state })
    }

    pub fn name(&self) -> &str {
        &self.design.name
    }

    /// Where the payload must sit when extended.
    pub fn target(&self) -> Pos {
        add(self.payload, scale(self.travel, self.reach))
    }

    /// Blocks counted: every block but the input device and piston heads.
    pub fn blocks(&self) -> usize {
        self.design.blocks.iter().filter(|(p, s)| **p != self.input && !s.starts_with("minecraft:piston_head")).count()
    }

    /// The design's box with the target cell.
    pub fn bounds(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.design.bounds().unwrap_or(([0; 3], [0; 3]));
        let t = self.target();
        for i in 0..3 {
            lo[i] = lo[i].min(t[i]);
            hi[i] = hi[i].max(t[i]);
        }
        (lo, hi)
    }

    /// The block kinds used (ids without `minecraft:`; wall torches are torches, heads part of
    /// their piston), the input device excluded.
    pub fn kinds(&self) -> std::collections::BTreeSet<String> {
        self.design
            .blocks
            .iter()
            .filter(|(p, _)| **p != self.input)
            .filter_map(|(_, s)| {
                let id = s.split('[').next().unwrap_or(s);
                let id = id.strip_prefix("minecraft:").unwrap_or(id);
                if id == "piston_head" || id == "moving_piston" || id == "air" {
                    return None;
                }
                Some(if id == "redstone_wall_torch" { "redstone_torch".to_string() } else { id.to_string() })
            })
            .collect()
    }

    /// Slime or honey anywhere (the study's categories).
    pub fn sticky_blocks(&self) -> bool {
        self.design.blocks.values().any(|s| s.contains("slime_block") || s.contains("honey_block"))
    }
}
