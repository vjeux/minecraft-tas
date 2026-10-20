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

/// The judge's consumer (`circuit::CONSUMER`): a lamp in every air output cell.
pub const CONSUMER: &str = "minecraft:redstone_lamp[lit=false]";

/// The output cells that get the judge's consumer (`circuit::Circuit::consumer_cells`): air in
/// the design and not an input cell, for every kind read through its outputs (not piston
/// extenders and flying machines), and not where a piston may move a block in: a piston's head
/// cell, or the cell after an unbroken row of blocks in front of a piston (up to 12); with slime
/// or honey in the design, any cell within 13 in front of a piston.
pub fn consumer_cells(d: &Design) -> Vec<Pos> {
    let Some(kind) = d.meta.get("circuit") else { return Vec::new() };
    let kind = kind.split_whitespace().next().unwrap_or("");
    if matches!(kind, "piston-extender" | "flying-machine" | "shulker-loader" | "shulker-unloader") {
        return Vec::new();
    }
    let ins: Vec<Pos> = inputs(d).iter().map(|i| i.pos).collect();
    let blocks = &d.blocks;
    let sticky_blocks = blocks.values().any(|s| s.starts_with("minecraft:slime_block") || s.starts_with("minecraft:honey_block"));
    let mut lines: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for (p, s) in blocks {
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
        let at = |k: i32| [p[0] + k * f[0], p[1] + k * f[1], p[2] + k * f[2]];
        if sticky_blocks {
            lines.extend((1..=13).map(at));
            continue;
        }
        lines.insert(at(1));
        let mut k = 1;
        while k <= 12 && blocks.contains_key(&at(k)) {
            lines.insert(at(k + 1));
            k += 1;
        }
    }
    let mut v: Vec<Pos> = outputs(d).into_iter().map(|(_, p)| p).filter(|p| !ins.contains(p) && !lines.contains(p) && !blocks.contains_key(p)).collect();
    v.sort();
    v.dedup();
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

/// A shulker loader or unloader (`circuit: shulker-loader|shulker-unloader`): the viewer runs it
/// with the judge's chests on its ports ([`shulker_chests`]).
pub fn is_shulker(d: &Design) -> bool {
    d.meta.get("circuit").is_some_and(|k| matches!(k.split_whitespace().next(), Some("shulker-loader" | "shulker-unloader")))
}

/// The judge's chest on each `items`, `boxes`, `output` and `reject` port of a shulker design
/// (`circuit::shulker::CHEST`; air cells, not counted).
pub const CHEST: &str = "minecraft:chest[facing=north,type=single,waterlogged=false]";

/// The cells of a shulker design's judge chests (its `items[.k]`, `boxes[.k]`, `output[.k]`,
/// `reject[.k]` ports on air cells).
pub fn chest_cells(d: &Design) -> Vec<Pos> {
    if !is_shulker(d) {
        return Vec::new();
    }
    let mut v: Vec<Pos> = d
        .ports
        .iter()
        .filter(|(n, _)| ["items", "boxes", "output", "reject"].iter().any(|k| is_named(n, k)))
        .map(|(_, (p, _))| *p)
        .filter(|p| !d.blocks.contains_key(p))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The judge's chests of a shulker design as the viewer runs them (redsim's
/// `redstone::shulker::Chests`, the driver `mctas circuit shulker compare` checks against the real
/// game): endless (a loader's sources and every box chest kept full, the outputs emptied every
/// tick), with unstackables (27 per box: a box every few seconds; every archived design but the
/// filter also passes with them) unless the design sorts (a `reject` port: its kind's own item).
pub fn shulker_chests(d: &Design) -> Option<redsim::redstone::shulker::Chests> {
    if !is_shulker(d) {
        return None;
    }
    let mut words = d.meta.get("circuit")?.split_whitespace();
    let loader = words.next() == Some("shulker-loader");
    let stack: u32 = words.next().and_then(|n| n.parse().ok()).unwrap_or(64);
    let cells = |name: &str| -> Vec<redsim::pos::BlockPos> {
        let mut v: Vec<Pos> = d.ports.iter().filter(|(n, _)| is_named(n, name)).map(|(_, (p, _))| *p).collect();
        v.sort();
        v.into_iter().map(crate::session::bp).collect()
    };
    let rejects = cells("reject");
    let item = match (rejects.is_empty(), stack) {
        (true, _) | (false, 1) => "iron_shovel",
        (false, 16) => "snowball",
        _ => "cobblestone",
    };
    let item = redsim::redstone::container::item_id(item)?;
    Some(redsim::redstone::shulker::Chests::endless(loader, item, cells("items"), cells("boxes"), cells("output"), rejects))
}

/// Where a shulker design's box stands while it is filled or emptied: the cell in front of its
/// dispenser (the "box cell X" under the dispenser of the reference designs), when it is air: a
/// still shulker box there makes the machine readable (it is not simulated).
pub fn box_cells(d: &Design) -> Vec<Pos> {
    if !is_shulker(d) {
        return Vec::new();
    }
    let mut v = Vec::new();
    for (p, s) in &d.blocks {
        if !s.starts_with("minecraft:dispenser[") {
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
        let q = [p[0] + f[0], p[1] + f[1], p[2] + f[2]];
        if !d.blocks.contains_key(&q) && !d.ports.values().any(|(c, _)| *c == q) {
            v.push(q);
        }
    }
    v.sort();
    v.dedup();
    v
}

/// `name` or `name.<k>`.
fn is_named(port: &str, name: &str) -> bool {
    port == name || port.strip_prefix(name).is_some_and(|r| r.starts_with('.'))
}

/// The tags a circuit's viewer shows on its ports besides the inputs: (label, cell, colour). The
/// outputs (OUT, orange), a shulker design's ITEMS and BOXES chests, a BUD switch's TARGET cells.
pub fn port_tags(d: &Design) -> Vec<(String, Pos, &'static str)> {
    if !d.meta.contains_key("circuit") {
        return Vec::new();
    }
    let mut v: Vec<(String, Pos, &'static str)> = outputs(d).into_iter().map(|(l, p)| (l, p, "#ffb04a")).collect();
    for (name, label, colour) in [("items", "ITEMS", "#ffd27a"), ("boxes", "BOXES", "#d9a8ff"), ("target", "TARGET", "#5ad1f0")] {
        let mut ports: Vec<(&String, Pos)> = d.ports.iter().filter(|(n, _)| is_named(n, name)).map(|(n, (p, _))| (n, *p)).collect();
        ports.sort();
        for (n, p) in ports {
            let tag = match n.strip_prefix(name).and_then(|r| r.strip_prefix('.')) {
                Some(k) => format!("{label} {}", k.to_uppercase()),
                None => label.to_string(),
            };
            v.push((tag, p, colour));
        }
    }
    v
}

/// What the judge does at a cell between ticks (`circuit::stimulus::TargetOp`, the BUD kind's
/// target and footprint actions), from a page's `data-events`.
#[derive(Clone, Debug, PartialEq)]
pub enum EventOp {
    /// Place this state with updates (flags 3, like a player).
    Place(Block),
    /// Remove the block with updates.
    Clear,
    /// Change to this state with shape updates only (flags 2).
    Shape(Block),
    /// A pure block update around the cell (`updateNeighborsAt`), nothing changes.
    Update,
}

/// Parse `data-events`: `<frame> <op> <x> <y> <z> [<state>]; ...` (design coordinates, frames
/// counted like the stimulus: after placement), op = place | clear | shape | update.
pub fn parse_events(text: &str) -> Result<Vec<(u32, Pos, EventOp)>, String> {
    let mut v = Vec::new();
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let w: Vec<&str> = part.split_whitespace().collect();
        let bad = || format!("event {part:?}: want `<frame> place|clear|shape|update <x> <y> <z> [<state>]`");
        if w.len() < 5 {
            return Err(bad());
        }
        let t: u32 = w[0].parse().map_err(|_| bad())?;
        let n = |k: usize| w[k].parse::<i32>().map_err(|_| bad());
        let p = [n(2)?, n(3)?, n(4)?];
        let state = |k: usize| -> Result<Block, String> {
            let s = w.get(k).ok_or_else(bad)?;
            let s = crate::design::normalize_state(s).map_err(|e| format!("event {part:?}: {e}"))?;
            Block::parse(&s).map_err(|e| format!("event {part:?}: {e}"))
        };
        let op = match w[1] {
            "place" => EventOp::Place(state(5)?),
            "clear" => EventOp::Clear,
            "shape" => EventOp::Shape(state(5)?),
            "update" => EventOp::Update,
            _ => return Err(bad()),
        };
        v.push((t, p, op));
    }
    v.sort_by_key(|(t, _, _)| *t);
    Ok(v)
}

/// The inputs' schedule (`circuit::stimulus::Schedule`): per input (in [`inputs`] order) its
/// edges (frame, on), alternating from on; frame t = after t ticks from placement, an edge applied
/// between ticks before frame t is shown. `ticks`: the last frame of the run.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schedule {
    pub edges: Vec<Vec<(u32, bool)>>,
    pub ticks: u32,
    /// The judge's actions at cells between ticks (`data-events`: a BUD switch's target and
    /// footprint probes): frame, cell, what.
    pub events: Vec<(u32, Pos, EventOp)>,
}

/// Frames the circuit stands at rest before the first input (`circuit::stimulus::SETTLE`).
pub const SETTLE: u32 = 40;

/// Parse a stimulus (`circuit::stimulus::parse`): `;`-separated parts, each `[<input>] <edges>`
/// with edges `10+ 14-` or `pulse L every T x K [from S]`; the input may be left out when there
/// is one. `ticks` = the last edge + `tail`.
pub fn parse_stimulus(text: &str, names: &[String], tail: u32) -> Result<Schedule, String> {
    let mut s = Schedule { edges: vec![Vec::new(); names.len()], ticks: 0, events: Vec::new() };
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

    /// A shulker loader gets the judge's chests on its items / boxes / output ports, a box under
    /// its dispenser, its port tags, and no consumer lamp; a BUD switch's target is tagged; the
    /// events parse like the site writes them.
    #[test]
    fn shulker_and_bud_ports() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/shulker/archive/shulker-ref-loader-1.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        assert!(is_shulker(&d));
        assert_eq!(chest_cells(&d), vec![[0, 2, 1], [0, 3, -1], [2, 0, 0]]);
        assert!(consumer_cells(&d).is_empty());
        let boxes = box_cells(&d);
        assert_eq!(boxes.len(), 1, "{boxes:?}");
        let disp = d.blocks.iter().find(|(_, s)| s.starts_with("minecraft:dispenser[facing=down")).map(|(p, _)| *p).unwrap();
        assert_eq!(boxes[0], [disp[0], disp[1] - 1, disp[2]]);
        let tags: Vec<String> = port_tags(&d).into_iter().map(|(l, _, _)| l).collect();
        assert_eq!(tags, vec!["OUT", "ITEMS", "BOXES"]);
        let bud = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/bud/archive/bud-slime-qc.txt")).unwrap();
        let tags: Vec<(String, Pos)> = port_tags(&Design::parse(&bud).unwrap()).into_iter().map(|(l, p, _)| (l, p)).collect();
        assert_eq!(tags, vec![("OUT".to_string(), [1, 3, 0]), ("TARGET".to_string(), [1, 0, 0])]);
        let ev = parse_events("60 update 1 0 0; 41 place 1 0 0 minecraft:stone; 50 shape 1 0 0 oak_trapdoor[open=true]; 45 clear 1 0 0").unwrap();
        assert_eq!(ev.iter().map(|e| e.0).collect::<Vec<_>>(), vec![41, 45, 50, 60]);
        assert!(matches!(ev[0].2, EventOp::Place(_)) && ev[1].2 == EventOp::Clear && matches!(ev[2].2, EventOp::Shape(_)) && ev[3].2 == EventOp::Update);
        assert!(parse_events("41 smash 1 0 0").is_err());
    }

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
