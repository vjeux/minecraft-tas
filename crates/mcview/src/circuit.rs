//! Circuits (`circuit:` designs, docs/circuits.md): their inputs driven exactly like the judge's
//! (`circuit::Circuit::actions`: an input cell written with updates, or its lever / button used),
//! the judge's own stimulus (`circuit::stimulus`, carried by the site as `data-stimulus`) replayed
//! as the auto-demo, and the checker's phase labels.

use crate::design::{Design, Pos};
use redsim::Block;

/// How an input cell is driven (`circuit::InputHow`).
#[derive(Clone, Debug, PartialEq)]
pub enum InputHow {
    /// The cell is written with updates: `on` while the input is on, `off` (default air) while off.
    Block { on: Block, off: Block },
    /// The design's lever in this cell, used at every input edge.
    Lever,
    /// The design's button in this cell, used at every rising edge.
    Button,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CircuitInput {
    /// The port's name after `input.` (`in` for a bare `port input:`).
    pub name: String,
    pub pos: Pos,
    pub how: InputHow,
}

/// The inputs of a circuit design, in name order like the judge's; empty for other designs.
pub fn inputs(d: &Design) -> Vec<CircuitInput> {
    if !d.meta.contains_key("circuit") {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (name, (pos, args)) in &d.ports {
        let short = match name.as_str() {
            "input" => "in".to_string(),
            n => match n.strip_prefix("input.") {
                Some(s) => s.to_string(),
                None => continue,
            },
        };
        let words: Vec<&str> = args.split_whitespace().collect();
        let how = match words.as_slice() {
            ["lever"] => InputHow::Lever,
            ["button"] => InputHow::Button,
            ws => {
                let state = |s: &str| crate::design::normalize_state(s).ok().and_then(|n| Block::parse(&n).ok());
                let on = ws.first().and_then(|s| state(s)).unwrap_or(Block::parse("minecraft:redstone_block").unwrap_or(Block::Air));
                let off = ws.get(1).and_then(|s| state(s)).unwrap_or(Block::Air);
                InputHow::Block { on, off }
            }
        };
        v.push(CircuitInput { name: short, pos: *pos, how });
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

/// The outputs of a circuit (`port output[.<name>]`): (label, cell), `OUT` for a bare one.
pub fn outputs(d: &Design) -> Vec<(String, Pos)> {
    let mut v: Vec<(String, Pos)> = d
        .ports
        .iter()
        .filter_map(|(n, (p, _))| match n.as_str() {
            "output" => Some(("OUT".to_string(), *p)),
            n => n.strip_prefix("output.").map(|s| (s.to_uppercase(), *p)),
        })
        .collect();
    v.sort();
    v
}

/// The inputs' schedule (`circuit::stimulus::Schedule`): per input (in [`inputs`] order) its
/// edges (frame, on), alternating from on; frame t = after t ticks from placement, an edge applied
/// between ticks before frame t is shown. `ticks`: the last frame of the run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schedule {
    pub edges: Vec<Vec<(u32, bool)>>,
    pub ticks: u32,
}

/// Frames the circuit stands at rest before the first input (`circuit::stimulus::SETTLE`).
pub const SETTLE: u32 = 40;

/// Parse a stimulus (`circuit::stimulus::parse`): `;`-separated parts, each `[<input>] <edges>`
/// with edges `10+ 14-` or `pulse L every T x K [from S]`; the input may be left out when there
/// is one. `ticks` = the last edge + `tail`.
pub fn parse_stimulus(text: &str, names: &[String], tail: u32) -> Result<Schedule, String> {
    let mut s = Schedule { edges: vec![Vec::new(); names.len()], ticks: 0 };
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let words: Vec<&str> = part.split_whitespace().collect();
        let (i, rest) = match names.iter().position(|n| n == words[0]) {
            Some(i) => (i, &words[1..]),
            None if names.len() == 1 => (0, &words[..]),
            None => return Err(format!("stimulus {part:?}: name the input ({})", names.join(", "))),
        };
        if rest.first() == Some(&"pulse") {
            let num = |k: usize| -> Result<u32, String> { rest.get(k).and_then(|w| w.parse::<u32>().ok()).ok_or_else(|| format!("stimulus {part:?}: want `pulse L every T x K [from S]`")) };
            if rest.get(2) != Some(&"every") || rest.get(4) != Some(&"x") {
                return Err(format!("stimulus {part:?}: want `pulse L every T x K [from S]`"));
            }
            let (l, every, k) = (num(1)?, num(3)?, num(5)?);
            let from = if rest.get(6) == Some(&"from") { num(7)? } else { SETTLE };
            if l == 0 || every <= l {
                return Err(format!("stimulus {part:?}: want 0 < L < T"));
            }
            for j in 0..k {
                s.edges[i].push((from + j * every, true));
                s.edges[i].push((from + j * every + l, false));
            }
        } else {
            for w in rest {
                let (t, on) = if let Some(t) = w.strip_suffix('+') {
                    (t, true)
                } else if let Some(t) = w.strip_suffix('-') {
                    (t, false)
                } else {
                    return Err(format!("stimulus {part:?}: an edge is <frame>+ or <frame>-, got {w:?}"));
                };
                s.edges[i].push((t.parse().map_err(|_| format!("stimulus {part:?}: bad frame {t:?}"))?, on));
            }
        }
    }
    for e in &mut s.edges {
        e.sort_by_key(|(t, _)| *t);
    }
    s.ticks = s.edges.iter().flatten().map(|(t, _)| *t).max().unwrap_or(0) + tail;
    Ok(s)
}

/// `from-to label; ...` (the checker's phases, `data-phases`).
pub fn parse_phases(text: &str) -> Vec<(u32, u32, String)> {
    text.split(';')
        .filter_map(|p| {
            let p = p.trim();
            let (range, label) = p.split_once(' ')?;
            let (a, b) = range.split_once('-')?;
            Some((a.trim().parse().ok()?, b.trim().parse().ok()?, label.trim().to_string()))
        })
        .collect()
}

/// Kinds whose inputs are pulses (a tap = a short pulse); the others (gates, the D latch, piston
/// extenders) toggle (a tap flips it).
pub fn pulse_kind(d: &Design) -> bool {
    let k = d.meta.get("circuit").map(|s| s.as_str()).unwrap_or("");
    let k = k.split_whitespace().next().unwrap_or("");
    matches!(k, "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "tff" | "delay" | "rs-latch" | "counter")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stimulus_like_the_judge() {
        let names = vec!["a".to_string(), "b".to_string()];
        let s = parse_stimulus("a 40+ 44-; b pulse 2 every 10 x 2 from 50", &names, 20).unwrap();
        assert_eq!(s.edges[0], vec![(40, true), (44, false)]);
        assert_eq!(s.edges[1], vec![(50, true), (52, false), (60, true), (62, false)]);
        assert_eq!(s.ticks, 82);
        assert_eq!(parse_phases("40-60 pulse 4 gt; 60-90 ab 01"), vec![(40, 60, "pulse 4 gt".to_string()), (60, 90, "ab 01".to_string())]);
    }

    #[test]
    fn inputs_in_name_order() {
        let d = Design::parse("name: g\ncircuit: gate and\nport input.b: 2 0 0\nport input.a: 0 0 0\nport output: 1 1 0\npalette:\n  S minecraft:stone\nlayer 0:\n  .S.\n").unwrap();
        let v = inputs(&d);
        assert_eq!(v.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
        assert!(matches!(v[0].how, InputHow::Block { .. }));
        assert_eq!(outputs(&d), vec![("OUT".to_string(), [1, 1, 0])]);
    }
}
