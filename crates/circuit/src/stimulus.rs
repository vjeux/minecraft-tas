//! When the inputs change: a [`Schedule`] (per input, its edges), the explicit `stimulus:` header
//! parser, and the building blocks the checkers use to generate each kind's stimulus.

use anyhow::{Context, Result, anyhow, bail};

/// Frames the circuit stands at rest after placement (and after its kick) before the first input.
pub const SETTLE: u32 = 40;

/// The inputs' schedule. Frame t is the state after t ticks; an edge at frame t is applied between
/// ticks, after t ticks and before frame t is recorded (an input on at frames t..t+L-1 is an L gt
/// pulse). Inputs start off.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    /// Per input (in `Circuit::inputs` order): its edges (frame, on), in order, alternating from on.
    pub edges: Vec<Vec<(u32, bool)>>,
    /// Frames recorded: 0..=ticks.
    pub ticks: u32,
    /// Labeled spans for the checker and the report: (from, to, label).
    pub phases: Vec<(u32, u32, String)>,
    /// Other actions the judge performs between ticks at a design's TARGET cells or at the cells
    /// around it (the BUD kind's block updates), by index (the cells move and turn with each
    /// build): frame, where, what.
    pub events: Vec<(u32, EventAt, TargetOp)>,
}

/// Where a [`Schedule::events`] action happens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventAt {
    /// The design's i-th target cell (`Circuit::targets`).
    Target(usize),
    /// The k-th cell around the design (`Circuit::probe_cells`): the BUD footprint probes.
    Probe(usize),
}

/// What the judge does at a target cell ([`Schedule::events`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetOp {
    /// Place this state with updates (flags 3, like a player).
    Place(String),
    /// Remove the block with updates.
    Clear,
    /// Change to this state with shape updates only (flags 2).
    Shape(String),
    /// A pure block update around the cell.
    Update,
}

impl Schedule {
    pub fn new(inputs: usize) -> Schedule {
        Schedule { edges: vec![Vec::new(); inputs], ticks: 0, phases: Vec::new(), events: Vec::new() }
    }

    /// Drop every phase whose inputs pulse or pause for less than `min` frames inside it, with its
    /// edges (the frames stay idle). R7: a real input device (the judge's repeater, a button, an
    /// observer line) cannot make a pulse or a gap shorter than 2 gt. A phase that leaves an input
    /// in another state than it found it is kept.
    pub fn drop_short(&mut self, min: u32) {
        let mut keep = Vec::new();
        for (from, to, label) in std::mem::take(&mut self.phases) {
            let inside = |e: &(u32, bool)| e.0 >= from && e.0 < to;
            let short = self.edges.iter().any(|es| {
                let v: Vec<u32> = es.iter().filter(|e| inside(e)).map(|e| e.0).collect();
                v.len() % 2 == 0 && v.windows(2).any(|w| w[1] - w[0] < min)
            });
            if short {
                for es in &mut self.edges {
                    es.retain(|e| !inside(e));
                }
            } else {
                keep.push((from, to, label));
            }
        }
        self.phases = keep;
    }

    /// Input `i` on at frame `at` for `len` frames.
    pub fn pulse(&mut self, i: usize, at: u32, len: u32) {
        self.edges[i].push((at, true));
        self.edges[i].push((at + len, false));
    }

    /// Set input `i` to `on` at frame `at` (nothing when it already is).
    pub fn set(&mut self, i: usize, at: u32, on: bool) {
        let now = self.edges[i].last().map(|e| e.1).unwrap_or(false);
        if now != on {
            self.edges[i].push((at, on));
        }
    }

    pub fn phase(&mut self, from: u32, to: u32, label: impl Into<String>) {
        self.phases.push((from, to, label.into()));
    }

    /// Whether input `i` is on at frame `t` (by the schedule).
    pub fn on_at(&self, i: usize, t: u32) -> bool {
        self.edges[i].iter().take_while(|(at, _)| *at <= t).last().map(|e| e.1).unwrap_or(false)
    }

    /// The schedule as an explicit `stimulus:` header.
    pub fn text(&self, names: &[String]) -> String {
        names
            .iter()
            .zip(&self.edges)
            .map(|(n, e)| format!("{n} {}", e.iter().map(|(t, on)| format!("{t}{}", if *on { '+' } else { '-' })).collect::<Vec<_>>().join(" ")))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Parse an explicit stimulus: `;`-separated parts, each `[<input>] <edges>` with edges `10+ 14-`
/// (on at frame 10, off at 14) or `pulse L every T x K [from S]` (default from SETTLE); the input
/// name may be left out when the circuit has one input. `ticks`: the last edge + `tail`.
pub fn parse(text: &str, names: &[String], tail: u32) -> Result<Schedule> {
    let mut s = Schedule::new(names.len());
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let words: Vec<&str> = part.split_whitespace().collect();
        let first = words[0];
        let named = names.iter().position(|n| n == first);
        let (i, rest) = match named {
            Some(i) => (i, &words[1..]),
            None if names.len() == 1 => (0, &words[..]),
            None => bail!("stimulus {part:?}: name the input ({})", names.join(", ")),
        };
        if rest.first() == Some(&"pulse") {
            // pulse L every T x K [from S]
            let num = |k: usize| -> Result<u32> {
                rest.get(k).ok_or_else(|| anyhow!("stimulus {part:?}: want `pulse L every T x K [from S]`"))?.parse::<u32>().with_context(|| format!("stimulus {part:?}"))
            };
            if rest.get(2) != Some(&"every") || rest.get(4) != Some(&"x") {
                bail!("stimulus {part:?}: want `pulse L every T x K [from S]`");
            }
            let (l, every, k) = (num(1)?, num(3)?, num(5)?);
            let from = if rest.get(6) == Some(&"from") { num(7)? } else { SETTLE };
            if l == 0 || every <= l {
                bail!("stimulus {part:?}: want 0 < L < T");
            }
            for j in 0..k {
                s.pulse(i, from + j * every, l);
            }
        } else {
            for w in rest {
                let (t, on) = if let Some(t) = w.strip_suffix('+') {
                    (t, true)
                } else if let Some(t) = w.strip_suffix('-') {
                    (t, false)
                } else {
                    bail!("stimulus {part:?}: an edge is <frame>+ or <frame>-, got {w:?}");
                };
                let t: u32 = t.parse().with_context(|| format!("stimulus {part:?}: bad frame {t:?}"))?;
                s.edges[i].push((t, on));
            }
        }
    }
    for (n, e) in names.iter().zip(&s.edges) {
        let mut want = true;
        let mut last = None;
        for (t, on) in e {
            if *on != want {
                bail!("stimulus: input {n}: edges must alternate on, off, on... starting from on (inputs start off)");
            }
            if last.is_some_and(|l| *t <= l) {
                bail!("stimulus: input {n}: edges must be in increasing frames");
            }
            want = !want;
            last = Some(*t);
        }
    }
    s.ticks = s.edges.iter().flatten().map(|(t, _)| *t).max().unwrap_or(0) + tail;
    Ok(s)
}

/// A walk through all `m` states (input combinations) that takes every ordered transition between
/// two different states exactly once, from state 0 back to 0 (an Euler circuit of the complete
/// digraph): 4 states give 13 states, 12 transitions.
pub fn euler_walk(m: usize) -> Vec<usize> {
    let mut next: Vec<Vec<usize>> = (0..m).map(|u| (0..m).filter(|&v| v != u).rev().collect()).collect();
    let mut stack = vec![0];
    let mut path = Vec::new();
    while let Some(&u) = stack.last() {
        match next[u].pop() {
            Some(v) => stack.push(v),
            None => path.push(stack.pop().unwrap()),
        }
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn walk_covers_every_transition() {
        let w = euler_walk(4);
        assert_eq!(w.len(), 13);
        assert_eq!((w[0], w[12]), (0, 0));
        let mut seen: Vec<(usize, usize)> = w.windows(2).map(|p| (p[0], p[1])).collect();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), 12);
        assert_eq!(euler_walk(2), vec![0, 1, 0]);
    }

    #[test]
    fn explicit_stimulus() {
        let names = vec!["a".to_string(), "b".to_string()];
        let s = parse("a 10+ 14- 40+ 41-; b pulse 2 every 10 x 3 from 20", &names, 50).unwrap();
        assert_eq!(s.edges[0], vec![(10, true), (14, false), (40, true), (41, false)]);
        assert_eq!(s.edges[1], vec![(20, true), (22, false), (30, true), (32, false), (40, true), (42, false)]);
        assert_eq!(s.ticks, 92);
        assert!(s.on_at(0, 13) && !s.on_at(0, 14) && s.on_at(0, 40));
        assert!(parse("10+ 14-", &names, 10).is_err());
        let one = vec!["in".to_string()];
        assert!(parse("10- 14+", &one, 10).is_err());
        assert_eq!(parse("10+ 14-", &one, 10).unwrap().ticks, 24);
    }
}
