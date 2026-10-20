//! A recorded run: per frame, each input's state, each output's level and every non-air cell.

use crate::{Circuit, InputHow, Pos};
use std::collections::HashMap;

/// One run of a circuit, frames 0..=ticks (the state after t ticks, with the inputs due at t
/// applied).
#[derive(Clone, Debug, Default)]
pub struct Trace {
    /// [input][frame]: whether the input is on, read from its cell (the on-state written there, a
    /// powered lever or button).
    pub inputs: Vec<Vec<bool>>,
    /// [output][frame]: the output's level ([`level`]).
    pub levels: Vec<Vec<u8>>,
    /// The cell states seen ([`Trace::cells`] holds indices into it).
    pub names: Vec<String>,
    /// [frame]: every non-air cell (position, index into `names`; full state: block state +
    /// comparator strength + container contents), sorted by position.
    pub cells: Vec<Vec<(Pos, u32)>>,
    /// The run ended early on purpose: a flying machine reached the far end of its corridor.
    pub ended: Option<String>,
    /// Primed TNT spawned during the run (TNT dupers; the judge removes each one before it can
    /// explode), in spawn order.
    pub primed: Vec<Primed>,
    /// TNT dupers with `explode: on` (the SURVIVABLE variation, real game only): how many primed TNT
    /// exploded during the run.
    pub exploded: u32,
}

/// One primed TNT: the frame it first appears in, and where it is and how it moves there (the
/// real game: one tick after it spawned; redsim: its spawn point, no motion).
#[derive(Clone, Debug, Default)]
pub struct Primed {
    pub frame: u32,
    pub pos: [f64; 3],
    pub motion: Option<[f64; 3]>,
    /// The real game: its position and motion at fuse 1, the tick before it would explode (the
    /// judge removes it there); `None` when the run ended first, and on redsim.
    pub end: Option<([f64; 3], [f64; 3])>,
    /// The real game: its motion when its horizontal speed was largest, if that reached 0.4
    /// blocks per gt (a launch: the LAUNCHED direction).
    pub launch: Option<[f64; 3]>,
    /// The real game: the last frame its position changed (at rest since then).
    pub moved: u32,
    pub last_pos: [f64; 3],
    /// The frame `end` was recorded at.
    pub end_frame: u32,
}

impl Trace {
    /// Build a trace from recorded cells and the best signal into each output cell, per frame.
    pub fn new(c: &Circuit, cells: Vec<Vec<(Pos, String)>>, signals: Vec<Vec<u8>>) -> Trace {
        let mut names: Vec<String> = Vec::new();
        let mut ids: HashMap<String, u32> = HashMap::new();
        let mut t = Trace { inputs: vec![Vec::with_capacity(cells.len()); c.inputs.len()], levels: vec![Vec::with_capacity(cells.len()); c.outputs.len()], ..Default::default() };
        for (f, frame) in cells.into_iter().enumerate() {
            let get = |p: Pos| frame.binary_search_by_key(&p, |(q, _)| *q).ok().map(|i| frame[i].1.as_str());
            for (k, inp) in c.inputs.iter().enumerate() {
                let s = get(inp.pos);
                let on = match &inp.how {
                    InputHow::Block { on, .. } => s.is_some_and(|s| state_matches(s, on)),
                    InputHow::Lever | InputHow::Button => s.is_some_and(|s| s.contains("powered=true")),
                };
                t.inputs[k].push(on);
            }
            for (k, out) in c.outputs.iter().enumerate() {
                let best = signals.get(f).and_then(|v| v.get(k)).copied().unwrap_or(0);
                t.levels[k].push(level(get(out.pos), best));
            }
            let v: Vec<(Pos, u32)> = frame
                .into_iter()
                .map(|(p, s)| {
                    let id = match ids.get(&s) {
                        Some(id) => *id,
                        None => {
                            let id = names.len() as u32;
                            ids.insert(s.clone(), id);
                            names.push(s);
                            id
                        }
                    };
                    (p, id)
                })
                .collect();
            t.cells.push(v);
        }
        t.names = names;
        t
    }

    pub fn frames(&self) -> usize {
        self.cells.len()
    }

    /// Whether output `k` is on (level > 0) at frame `t`.
    pub fn on(&self, k: usize, t: usize) -> bool {
        self.levels[k].get(t).is_some_and(|l| *l > 0)
    }

    /// Output `k` on/off over frames `from..to`.
    pub fn out_seq(&self, k: usize) -> Vec<bool> {
        self.levels[k].iter().map(|l| *l > 0).collect()
    }

    /// The state of the cell at `p` in frame `t` (`None`: air).
    pub fn state(&self, t: usize, p: Pos) -> Option<&str> {
        let f = &self.cells[t];
        f.binary_search_by_key(&p, |(q, _)| *q).ok().map(|i| self.names[f[i].1 as usize].as_str())
    }

    /// Frames `from..to` as text, one line per input and output: `#` on, `.` off (an output's level
    /// as a hex digit when it is not 15).
    pub fn strip(&self, c: &Circuit, from: usize, to: usize) -> Vec<String> {
        let to = to.min(self.frames());
        let w = c.inputs.iter().map(|i| i.name.len()).chain(c.outputs.iter().map(|o| o.name.len())).max().unwrap_or(3).max(3);
        let mut out = vec![format!("{:w$}  frames {from}..{}", "", to.saturating_sub(1))];
        for (k, i) in c.inputs.iter().enumerate() {
            out.push(format!("{:w$}  {}", i.name, self.inputs[k][from..to].iter().map(|b| if *b { '#' } else { '.' }).collect::<String>()));
        }
        for (k, o) in c.outputs.iter().enumerate() {
            out.push(format!(
                "{:w$}  {}",
                o.name,
                self.levels[k][from..to].iter().map(|l| match *l { 0 => '.', 15 => '#', l => char::from_digit(l as u32, 16).unwrap_or('?') }).collect::<String>()
            ));
        }
        out
    }
}

/// Whether a recorded state is a lit redstone lamp (the judge's consumer, [`crate::CONSUMER`]).
pub fn lamp_lit(state: Option<&str>) -> bool {
    state.is_some_and(|s| s.starts_with("minecraft:redstone_lamp") && s.contains("lit=true"))
}

/// The level of an output cell holding `state` (`None`: air) with `best` the best signal its
/// neighbors give into it: redstone dust gives its power; a repeater, torch or observer its own
/// output (15 when on); a comparator its strength; any other cell (air, a solid block, a lamp, a
/// piston) the best signal into it (what a lamp there would see).
pub fn level(state: Option<&str>, best: u8) -> u8 {
    let Some(s) = state else {
        return best;
    };
    let id = s.split(['[', '{']).next().unwrap_or(s);
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    let prop = |k: &str| -> Option<&str> {
        let i = s.find(&format!("{k}="))? + k.len() + 1;
        s[i..].split([',', ']']).next()
    };
    match id {
        "redstone_wire" => prop("power").and_then(|p| p.parse().ok()).unwrap_or(0),
        "repeater" | "observer" => {
            if prop("powered") == Some("true") {
                15
            } else {
                0
            }
        }
        "redstone_torch" | "redstone_wall_torch" => {
            if prop("lit") == Some("true") {
                15
            } else {
                0
            }
        }
        "comparator" => match s.find("{out=") {
            Some(i) => s[i + 5..].split('}').next().and_then(|n| n.parse().ok()).unwrap_or(0),
            None => {
                if prop("powered") == Some("true") {
                    15
                } else {
                    0
                }
            }
        },
        _ => best,
    }
}

/// Whether a recorded state is `wanted`: the same block, and every property `wanted` names has the
/// same value (the game reports every property; a design may name only some).
pub fn state_matches(actual: &str, wanted: &str) -> bool {
    let split = |s: &str| -> (String, Vec<(String, String)>) {
        let s = s.split('{').next().unwrap_or(s);
        let (id, props) = match s.split_once('[') {
            Some((id, rest)) => (id, rest.trim_end_matches(']')),
            None => (s, ""),
        };
        let id = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        let props = props.split(',').filter(|p| !p.is_empty()).filter_map(|p| p.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
        (id, props)
    };
    let (ia, pa) = split(actual);
    let (iw, pw) = split(wanted);
    ia == iw && pw.iter().all(|kv| pa.contains(kv))
}

/// The maximal runs of `true` in `seq`: (start, length).
pub fn runs(seq: &[bool]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = None;
    for (i, b) in seq.iter().enumerate() {
        match (b, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                out.push((s, i - s));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, seq.len() - s));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        assert_eq!(level(None, 7), 7);
        assert_eq!(level(Some("minecraft:redstone_wire[east=side,north=none,power=9,south=none,west=side]"), 0), 9);
        assert_eq!(level(Some("minecraft:repeater[delay=1,facing=north,locked=false,powered=true]"), 0), 15);
        assert_eq!(level(Some("minecraft:redstone_wall_torch[facing=east,lit=false]"), 15), 0);
        assert_eq!(level(Some("minecraft:comparator[facing=north,mode=compare,powered=true]{out=11}"), 0), 11);
        assert_eq!(level(Some("minecraft:redstone_lamp[lit=true]"), 0), 0);
        assert_eq!(level(Some("minecraft:stone"), 13), 13);
    }

    #[test]
    fn matching() {
        assert!(state_matches("minecraft:redstone_block", "minecraft:redstone_block"));
        assert!(state_matches("minecraft:target[power=0]", "minecraft:target"));
        assert!(!state_matches("minecraft:target[power=3]", "minecraft:target[power=0]"));
        assert!(state_matches("minecraft:air", "air"));
        assert!(!state_matches("minecraft:stone", "minecraft:air"));
    }

    #[test]
    fn run_lengths() {
        assert_eq!(runs(&[false, true, true, false, true]), vec![(1, 2), (4, 1)]);
        assert!(runs(&[false, false]).is_empty());
    }
}
