//! A flying-machine design: the door/clock design format (`crates/mctas/src/design.rs`) plus the
//! circuit headers (docs/circuits.md, clockinfra's format):
//!
//! * `circuit: flying-machine` (or `circuit: flying-machine trail`: left-behind blocks allowed);
//! * `port start: x y z <kick>`: the clock kick (`last` | `use [T]` | `<state> T`, docs/clocks.md);
//! * `travel: dx dy dz`: the travel direction (a unit vector), `distance: k`: the corridor length;
//! * `port payload: x y z` (more: `port payload.<n>:`): cells that must travel with the machine;
//! * `stop: x y z <state>` (any number of lines): environment blocks (a stop wall, a return
//!   station), placed before the machine, never counted as its blocks.

use anyhow::{Context, Result, anyhow, bail};
use clock::{Clock, Pos};
use mctas::design::Design;
use std::path::Path;

#[derive(Clone, Debug)]
pub struct Machine {
    /// The design and its kick (parsed by the clock crate).
    pub clock: Clock,
    /// The words after `circuit:` (`flying-machine`, `flying-machine trail`).
    pub kind: String,
    pub travel: Option<Pos>,
    pub distance: Option<i32>,
    pub payload: Vec<Pos>,
    /// Environment blocks (`stop:` lines), in file order.
    pub stops: Vec<(Pos, String)>,
}

fn parse_vec(s: &str) -> Result<Pos> {
    let v: Vec<i32> = s.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()).map(|w| w.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("bad vector {s:?}"))?;
    if v.len() != 3 {
        bail!("expected 3 numbers, got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

/// `+x`, `-y`, `east`, `up`, or `dx dy dz`.
pub fn parse_travel(s: &str) -> Result<Pos> {
    let t = s.trim();
    let named = match t {
        "+x" | "x" | "east" => Some([1, 0, 0]),
        "-x" | "west" => Some([-1, 0, 0]),
        "+y" | "y" | "up" => Some([0, 1, 0]),
        "-y" | "down" => Some([0, -1, 0]),
        "+z" | "z" | "south" => Some([0, 0, 1]),
        "-z" | "north" => Some([0, 0, -1]),
        _ => None,
    };
    let v = match named {
        Some(v) => v,
        None => parse_vec(t)?,
    };
    if v.iter().map(|c| c.abs()).sum::<i32>() != 1 {
        bail!("travel must be a unit vector along one axis, got {s:?}");
    }
    Ok(v)
}

/// A direction's name for reports: `+x (east)`.
pub fn travel_name(v: Pos) -> &'static str {
    match v {
        [1, 0, 0] => "+x (east)",
        [-1, 0, 0] => "-x (west)",
        [0, 1, 0] => "+y (up)",
        [0, -1, 0] => "-y (down)",
        [0, 0, 1] => "+z (south)",
        [0, 0, -1] => "-z (north)",
        _ => "diagonal",
    }
}

impl Machine {
    pub fn load(path: &Path) -> Result<Machine> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Machine::parse(&text).with_context(|| format!("flying machine {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Machine> {
        let mut design = Design::parse(text)?;
        // every `stop:` line (the design parser keeps only the last header of a name)
        let mut stops = Vec::new();
        for raw in text.lines() {
            if raw.starts_with(' ') {
                continue;
            }
            let line = raw.split('#').next().unwrap_or("").trim();
            if let Some(rest) = line.strip_prefix("stop:") {
                let words: Vec<&str> = rest.split_whitespace().collect();
                if words.len() != 4 {
                    bail!("stop: x y z <state>, got {raw:?}");
                }
                let p = parse_vec(&words[..3].join(" "))?;
                let state = mctas::blockstate::BlockState::parse(words[3])?.to_string();
                if design.blocks.contains_key(&p) {
                    bail!("stop block at {p:?} overlaps a machine block");
                }
                stops.push((p, state));
            }
        }
        design.meta.remove("stop");
        let kind = design.meta.get("circuit").cloned().unwrap_or_else(|| "flying-machine".into());
        let travel = design.meta.get("travel").map(|s| parse_travel(s)).transpose()?;
        let distance = design.meta.get("distance").map(|s| s.trim().parse::<i32>().map_err(|e| anyhow!("distance: {e}"))).transpose()?;
        let payload: Vec<Pos> = design.ports.iter().filter(|(k, _)| *k == "payload" || k.starts_with("payload.")).map(|(_, p)| *p).collect();
        for p in &payload {
            if !design.blocks.contains_key(p) {
                bail!("port payload {p:?}: no machine block there");
            }
        }
        let clock = Clock::from_design(design)?;
        Ok(Machine { clock, kind, travel, distance, payload, stops })
    }

    pub fn name(&self) -> &str {
        self.clock.name()
    }

    pub fn design(&self) -> &Design {
        &self.clock.design
    }

    /// Blocks placed (piston heads are part of their piston; stop blocks and a temporary kick block
    /// are not counted).
    pub fn blocks(&self) -> usize {
        self.clock.blocks()
    }

    pub fn trail_allowed(&self) -> bool {
        self.kind.split_whitespace().any(|w| w == "trail")
    }

    /// The design as text, every `stop:` line included.
    pub fn to_text(&self) -> String {
        let mut t = self.clock.to_text();
        if !self.stops.is_empty() {
            let mut lines: Vec<String> = t.lines().map(String::from).collect();
            let at = lines.iter().position(|l| l.starts_with("palette:")).unwrap_or(lines.len());
            for (i, (p, s)) in self.stops.iter().enumerate() {
                lines.insert(at + i, format!("stop: {} {} {} {s}", p[0], p[1], p[2]));
            }
            t = lines.join("\n") + "\n";
        }
        t
    }

    /// The same machine moved by `d` (stops and payload too).
    pub fn translated(&self, d: Pos) -> Machine {
        let mv = |p: Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        Machine {
            clock: self.clock.translated(d),
            kind: self.kind.clone(),
            travel: self.travel,
            distance: self.distance,
            payload: self.payload.iter().map(|p| mv(*p)).collect(),
            stops: self.stops.iter().map(|(p, s)| (mv(*p), s.clone())).collect(),
        }
    }

    /// The machine's box with the kick cell and the stop blocks.
    pub fn bounds(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.clock.bounds();
        for (p, _) in &self.stops {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        (lo, hi)
    }

    /// The corridor the judge watches: the box swept `distance` cells along `travel` (every
    /// direction when the travel is unknown), plus `margin`.
    pub fn corridor(&self, default_distance: i32, margin: i32) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.bounds();
        let k = self.distance.unwrap_or(default_distance);
        match self.travel {
            Some(v) => {
                for i in 0..3 {
                    if v[i] > 0 {
                        hi[i] += k;
                    } else if v[i] < 0 {
                        lo[i] -= k;
                    }
                }
            }
            None => {
                for i in 0..3 {
                    lo[i] -= k;
                    hi[i] += k;
                }
            }
        }
        for i in 0..3 {
            lo[i] -= margin;
            hi[i] += margin;
        }
        (lo, hi)
    }
}
