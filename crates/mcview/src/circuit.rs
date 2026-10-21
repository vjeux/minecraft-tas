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
    /// A value 0..15 as a signal strength (`port input.a: x y z strength`, the judge's hex kinds):
    /// the design's chest in this cell, whose contents the judge rewrites to the value's fill
    /// ([`strength_items`]): placed again (its state unchanged), then its contents set. A tap
    /// flips it between 0 and 15.
    Strength { chest: Block },
    /// A plain input driven like a real build drives it (the judge's R7, `circuit::InputHow::
    /// Driven`): the judge's stone (or the design's own receiving block) in the input cell, its
    /// delay-1 repeater in the cell `dir` from it with a stone under it, switched by a redstone
    /// block placed `2 dir` from the input cell 2 gt before each input edge.
    Driven { dir: Pos, through: bool },
}

/// A plain input's on-state, written into its cell before R7 (`circuit::INPUT_ON`).
pub const INPUT_ON: &str = "minecraft:redstone_block";
/// The judge's stone in a driven input's cell and under its repeater (`circuit::DRIVEN_STONE`).
pub const DRIVEN_STONE: &str = "minecraft:smooth_stone";
/// The directions a driven input's repeater may stand in, in the judge's order
/// (`circuit::DRIVE_DIRS`).
pub const DRIVE_DIRS: [Pos; 4] = [[0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// A driven input's judge cells: its repeater, the repeater's support and the switch
/// (`circuit::driver_cells`).
pub fn driver_cells(pos: Pos, dir: Pos) -> (Pos, Pos, Pos) {
    let r = add(pos, dir);
    (r, add(r, [0, -1, 0]), add(r, dir))
}

/// The judge's repeater of a driven input, facing its direction (`circuit::Circuit::with_drivers`).
pub fn driver_repeater(dir: Pos) -> String {
    let facing = match dir {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [-1, 0, 0] => "west",
        _ => "east",
    };
    format!("minecraft:repeater[delay=1,facing={facing},locked=false,powered=false]")
}

/// The box a circuit occupies during its run (`circuit::Circuit::swept_box`): its blocks' box,
/// swept along its travel by a piston extender's reach or a flying machine's distance.
fn swept_box(d: &Design) -> Option<(Pos, Pos)> {
    let mut it = d.blocks.keys();
    let first = *it.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in it {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    let kind = d.meta.get("circuit").map(|s| s.as_str()).unwrap_or("");
    let first_word = kind.split_whitespace().next().unwrap_or("");
    let reach = match first_word {
        "piston-extender" => kind.split_whitespace().nth(1).and_then(|w| w.parse::<i32>().ok()).unwrap_or(1) + 1,
        "flying-machine" => d.meta.get("distance").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(0) + 2,
        _ => 0,
    };
    let travel: Option<Pos> = d.meta.get("travel").and_then(|t| {
        let v: Vec<i32> = t.split_whitespace().filter_map(|x| x.parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    });
    if let (Some(t), true) = (travel, reach > 0) {
        for i in 0..3 {
            lo[i] = lo[i].min(lo[i] + t[i] * reach);
            hi[i] = hi[i].max(hi[i] + t[i] * reach);
        }
    }
    Some((lo, hi))
}

/// Turn each plain input into a driven one like the judge (`circuit::Circuit::assign_drivers`):
/// the first direction of [`DRIVE_DIRS`] whose repeater, support and switch cells are outside the
/// box (a tiled run: free) and free of design blocks, ports and other drivers, touching no
/// observer or diode beside the repeater and no design cell beside the switch (a diode in the
/// input cell: at its back); else one through the build's air cells, its switch outside the box
/// (static designs only). An input with no way in keeps its block (the judge refuses the design).
fn assign_drivers(d: &Design, inputs: &mut [CircuitInput]) {
    let Some((lo, hi)) = swept_box(d) else { return };
    let tiled = d.meta.contains_key("tiled");
    let outside = |p: Pos| (0..3).any(|i| p[i] < lo[i] || p[i] > hi[i]);
    let mut taken: std::collections::BTreeSet<Pos> = d.ports.values().map(|(p, _)| *p).collect();
    taken.extend(d.blocks.keys().copied());
    let design_cells = taken.clone();
    let on_block = Block::parse(INPUT_ON).ok();
    let moving = d.meta.get("circuit").is_some_and(|k| k.starts_with("piston-extender") || k.starts_with("flying-machine"));
    for inp in inputs.iter_mut() {
        let InputHow::Block { on, .. } = &inp.how else { continue };
        if Some(*on) != on_block {
            continue;
        }
        let p = inp.pos;
        let here = d.blocks.get(&p).map(|s| s.as_str()).unwrap_or("");
        let back: Option<Pos> = if here.starts_with("minecraft:repeater") || here.starts_with("minecraft:comparator") {
            match here.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()) {
                Some("north") => Some([0, 0, -1]),
                Some("south") => Some([0, 0, 1]),
                Some("west") => Some([-1, 0, 0]),
                Some("east") => Some([1, 0, 0]),
                _ => None,
            }
        } else {
            None
        };
        let free = |q: Pos, taken: &std::collections::BTreeSet<Pos>| !taken.contains(&q);
        let alone = |dir: &Pos| {
            let (r, sup, sw) = driver_cells(p, *dir);
            let quiet = DRIVE_DIRS.iter().chain(&[[0, 1, 0], [0, -1, 0]]).map(|n| add(r, *n)).filter(|q| *q != p && *q != sup && *q != sw).all(|q| d.blocks.get(&q).is_none_or(|s| !(s.starts_with("minecraft:observer") || s.starts_with("minecraft:repeater") || s.starts_with("minecraft:comparator"))));
            let isolated = DRIVE_DIRS.iter().chain(&[[0, 1, 0], [0, -1, 0]]).map(|n| add(sw, *n)).filter(|q| *q != r).all(|q| !design_cells.contains(&q));
            quiet && isolated && back.is_none_or(|b| b == *dir)
        };
        let straight = DRIVE_DIRS.iter().copied().find(|dir| {
            let (r, sup, sw) = driver_cells(p, *dir);
            [r, sup, sw].iter().all(|q| free(*q, &taken) && (tiled || outside(*q))) && alone(dir)
        });
        let found = straight.map(|dir| (dir, false)).or_else(|| {
            DRIVE_DIRS
                .iter()
                .copied()
                .find(|dir| {
                    let (r, sup, sw) = driver_cells(p, *dir);
                    [r, sup, sw].iter().all(|q| free(*q, &taken) && (!moving || outside(*q))) && outside(sw) && alone(dir)
                })
                .map(|dir| (dir, true))
        });
        if let Some((dir, through)) = found {
            let (r, sup, sw) = driver_cells(p, dir);
            taken.extend([r, sup, sw]);
            inp.how = InputHow::Driven { dir, through };
        }
    }
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
        let state = |s: &str| crate::design::normalize_state(s).ok().and_then(|n| Block::parse(&n).ok());
        let how = match words.as_slice() {
            ["lever"] => InputHow::Lever,
            ["button"] => InputHow::Button,
            ["strength"] => InputHow::Strength { chest: d.blocks.get(pos).and_then(|s| state(s)).unwrap_or(Block::Chest { facing: redsim::Dir::North, waterlogged: false }) },
            ws => {
                let on = ws.first().and_then(|s| state(s)).unwrap_or(Block::parse("minecraft:redstone_block").unwrap_or(Block::Air));
                let off = ws.get(1).and_then(|s| state(s)).unwrap_or(Block::Air);
                InputHow::Block { on, off }
            }
        };
        v.push(CircuitInput { name: short, pos: *pos, how });
    }
    v.sort_by(|a, b| a.name.cmp(&b.name));
    // R7: plain inputs driven by the judge's repeater from outside
    assign_drivers(d, &mut v);
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
            // `OUT A`: a bare letter would read like an input's tag
            n => n.strip_prefix("output.").map(|s| (format!("OUT {}", s.to_uppercase()), *p)),
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

/// The watch box the shulker judge builds a design in (`circuit::run::watch_box(c, 3)`: its blocks
/// and ports, plus the judge's margin): the sandbox's seed and forced chunks come from it
/// (`redsim::redstone::shulker::judge_sandbox`), so the viewer draws the judge's level random
/// (`web check` compares it with the circuit crate's box for every shulker design).
pub fn judge_box(d: &Design) -> Option<(Pos, Pos)> {
    let m = redsim::redstone::shulker::JUDGE_MARGIN;
    let mut pts = d.blocks.keys().copied().chain(d.ports.values().map(|(p, _)| *p));
    let first = pts.next()?;
    let (mut lo, mut hi) = (first, first);
    for p in pts {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    Some(([lo[0] - m, lo[1] - m, lo[2] - m], [hi[0] + m, hi[1] + m, hi[2] + m]))
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
    for (name, label, colour) in [("items", "ITEMS", "#ffd27a"), ("boxes", "BOXES", "#d9a8ff"), ("reject", "REJECT", "#ff8f8f"), ("target", "TARGET", "#5ad1f0")] {
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

/// One tag over a group of a circuit's ports: its label, its cells, its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct PortGroup {
    pub label: String,
    pub cells: Vec<Pos>,
    pub colour: &'static str,
}

/// The tags of a circuit's ports (besides the inputs): one per port ([`port_tags`]), except in a
/// shulker design, where the same kind's chests that stand together (within 2 cells of one
/// another) share ONE tag without letters (`ITEMS A`..`ITEMS F` in two rows of three: two
/// `ITEMS` tags; the two output chests far apart: two `OUT` tags).
pub fn port_groups(d: &Design) -> Vec<PortGroup> {
    let tags = port_tags(d);
    if !is_shulker(d) {
        return tags.into_iter().map(|(label, p, colour)| PortGroup { label, cells: vec![p], colour }).collect();
    }
    let mut groups: Vec<PortGroup> = Vec::new();
    for (base, label, colour) in [("output", "OUT", "#ffb04a"), ("items", "ITEMS", "#ffd27a"), ("boxes", "BOXES", "#d9a8ff"), ("reject", "REJECT", "#ff8f8f")] {
        // each port's cell and letter (`items.a` -> A; a bare `items` port has none)
        let mut ports: Vec<(Pos, Option<String>)> = d.ports.iter().filter(|(n, _)| is_named(n, base)).map(|(n, (p, _))| (*p, n.strip_prefix(base).and_then(|r| r.strip_prefix('.')).map(|k| k.to_uppercase()))).collect();
        ports.sort();
        ports.dedup_by(|a, b| a.0 == b.0);
        // clusters: cells within 2 (Chebyshev) of a cell of the cluster
        let mut left = ports;
        while let Some(first) = left.pop() {
            let mut cluster = vec![first];
            loop {
                let near: Vec<usize> = (0..left.len()).filter(|&i| cluster.iter().any(|c| (0..3).all(|k| (c.0[k] - left[i].0[k]).abs() <= 2))).collect();
                if near.is_empty() {
                    break;
                }
                for i in near.into_iter().rev() {
                    cluster.push(left.remove(i));
                }
            }
            cluster.sort();
            // its letters: `OUT A`, `ITEMS A–C` (a run), `ITEMS A,C`
            let mut letters: Vec<String> = cluster.iter().filter_map(|(_, k)| k.clone()).collect();
            letters.sort();
            let run = letters.len() > 2 && letters.iter().all(|l| l.len() == 1) && letters.windows(2).all(|w| w[1].as_bytes()[0] == w[0].as_bytes()[0] + 1);
            let name = if letters.len() != cluster.len() || letters.is_empty() {
                label.to_string()
            } else if run {
                format!("{label} {}–{}", letters[0], letters[letters.len() - 1])
            } else {
                format!("{label} {}", letters.join(","))
            };
            groups.push(PortGroup { label: name, cells: cluster.into_iter().map(|(p, _)| p).collect(), colour });
        }
    }
    groups.sort_by(|a, b| (a.label.as_str(), a.cells[0]).cmp(&(b.label.as_str(), b.cells[0])));
    groups
}

/// `1` -> `1st`.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A tick count in the site's words: `6 ticks (0.3 s)`, `1 tick (0.05 s)`, `1,300 ticks (1 min
/// 5 s)` (critique: the viewer never says "gt"; the site's `real_time` rule: seconds with up to 2
/// decimals, minutes past 60 s).
pub fn ticks_words(n: u32) -> String {
    let num = |x: f64| -> String {
        let s = format!("{x:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let secs = n as f64 / 20.0;
    let time = if secs < 60.0 {
        format!("{} s", num(secs))
    } else {
        let mins = (secs / 60.0).floor();
        let rest = secs - mins * 60.0;
        if rest > 0.0 { format!("{mins} min {} s", num(rest)) } else { format!("{mins} min") }
    };
    format!("{} tick{} ({time})", crate::labels::thousands(n), if n == 1 { "" } else { "s" })
}

/// A demo phase label (the checker's: `10`, `pulse 3`, `cycle 2`, `place 1 t`...) in words for
/// a visitor (`A on, B off`, `an input pulse of 3 ticks (0.15 s)`, `2nd cycle`, `a block placed
/// on TARGET`). `kind`: the design's circuit kind; `inputs`: its input names. Unknown labels stay
/// as they are.
pub fn phase_words(kind: &str, label: &str, inputs: &[String]) -> String {
    let kind = kind.split_whitespace().next().unwrap_or("");
    let w: Vec<&str> = label.split_whitespace().collect();
    let num = |i: usize| w.get(i).and_then(|s| s.parse::<u32>().ok());
    let name = |i: usize| inputs.get(i).map(|n| n.to_uppercase()).unwrap_or_else(|| format!("input {}", i + 1));
    let bits = |s: &str| !inputs.is_empty() && s.len() == inputs.len() && s.chars().all(|c| c == '0' || c == '1');
    // a gate's input states
    if w.len() == 1 && bits(w[0]) {
        return w[0].chars().enumerate().map(|(i, c)| format!("{} {}", name(i), if c == '1' { "on" } else { "off" })).collect::<Vec<_>>().join(", ");
    }
    if w.len() == 2 && bits(w[0]) && w[1] == "(rest)" {
        return "all inputs off".into();
    }
    match (w.as_slice(), num(1)) {
        (["press", _], Some(n)) => return format!("{} button press", ordinal(n)),
        (["pulse", _], Some(n)) if matches!(kind, "counter" | "tnt-duper-triggered") => return format!("{} input pulse", ordinal(n)),
        (["pulse", _], Some(n)) => return format!("an input pulse of {}", ticks_words(n)),
        (["gap", _], Some(g)) => return format!("two pulses {} apart", ticks_words(g)),
        (["pair", _], Some(g)) => return format!("two pulses {} apart", ticks_words(g)),
        (["cycle", _], Some(n)) => return format!("{} cycle: out and back", ordinal(n)),
        (["train", _], Some(r)) => return format!("a pulse train: {}", ticks_words(r)),
        (["clock", _], Some(p)) => return format!("a clock, every {}", ticks_words(p)),
        (["on", _], Some(_)) | (["on"], _) => return "input on".into(),
        (["off", _], Some(_)) | (["off"], _) => return "input off".into(),
        (["idle"], _) => return "waiting".into(),
        (["flight"], _) => return "flying".into(),
        (["run"], _) => return "running".into(),
        _ => {}
    }
    // T flip-flops, RS latches: `A 1 pulse 4`, `B pulse 4`, `C period 8`, `S pulse 2`
    let n = |s: &str| s.parse::<u32>().ok();
    match w.as_slice() {
        [inp, k, "pulse", len] if n(k).is_some() && n(len).is_some() => return format!("{inp}: {} pulse, {}", ordinal(n(k).unwrap_or(1)), ticks_words(n(len).unwrap_or(0))),
        [inp, "pulse", len] if n(len).is_some() => return format!("{inp}: a pulse of {}", ticks_words(n(len).unwrap_or(0))),
        [inp, "period", p] if n(p).is_some() => return format!("{inp}: a clock, every {}", ticks_words(n(p).unwrap_or(0))),
        _ => {}
    }
    // a BUD switch's judge actions
    if kind == "bud" {
        let what = match w.first().copied() {
            Some("place") => Some("a block placed on TARGET"),
            Some("remove") => Some("the block on TARGET removed"),
            Some("state") => Some("TARGET's block changes state"),
            Some("shape") => Some("a shape update at TARGET"),
            Some("update") => Some("a block update at TARGET"),
            Some("setup") => Some("setting up"),
            Some("relatch") => Some("re-arming"),
            Some("rearm") => Some("re-arm test"),
            Some("probe") => match w.get(2).copied() {
                Some("place") => Some("a block placed next to the build"),
                Some("remove") => Some("a block removed next to the build"),
                _ => Some("a block update next to the build"),
            },
            _ => None,
        };
        if let Some(s) = what {
            return s.into();
        }
    }
    // `d=1 e=0`, `M 2 d=1 clk=0`: the levels in words
    if w.iter().any(|t| t.contains('=')) {
        let parts: Vec<String> = w
            .iter()
            .filter_map(|t| {
                let (k, v) = t.split_once('=')?;
                let k = if k == "clk" { "clock".to_string() } else { k.to_uppercase() };
                Some(format!("{k} {}", if v == "1" { "on" } else { "off" }))
            })
            .collect();
        if !parts.is_empty() {
            return parts.join(", ");
        }
    }
    label.to_string()
}
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
    /// Strength inputs' values (`data-levels`, `circuit::stimulus::Schedule::levels`): frame,
    /// input index, value 0..15.
    pub levels: Vec<(u32, usize, u8)>,
}

/// The item a strength input's chest is filled with (`circuit::compute::STRENGTH_ITEM`).
pub const STRENGTH_ITEM: &str = "minecraft:cobblestone";

/// A strength input's chest contents for a value (`circuit::compute::strength_items`): the fewest
/// items of [`STRENGTH_ITEM`] whose comparator signal is the value, as SNBT.
pub fn strength_items(v: u8) -> String {
    let v = v.min(15) as u64;
    let n = if v == 0 { 0 } else { ((v - 1) * 1728).div_ceil(14).max(1) };
    let mut items = Vec::new();
    let mut left = n;
    let mut slot = 0;
    while left > 0 {
        let k = left.min(64);
        items.push(format!("{{Slot:{slot}b,id:\"{STRENGTH_ITEM}\",count:{k}}}"));
        left -= k;
        slot += 1;
    }
    format!("{{Items:[{}]}}", items.join(","))
}

/// Parse `data-levels`: `<frame> <input> <value>; ...` (an input by its name, as in
/// `data-stimulus`).
pub fn parse_levels(text: &str, names: &[String]) -> Result<Vec<(u32, usize, u8)>, String> {
    let mut v = Vec::new();
    for part in text.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let w: Vec<&str> = part.split_whitespace().collect();
        let [t, name, value] = w.as_slice() else { return Err(format!("level `{part}`: want <frame> <input> <value>")) };
        let t: u32 = t.parse().map_err(|_| format!("level `{part}`: bad frame"))?;
        let i = names.iter().position(|n| n == name).ok_or_else(|| format!("level `{part}`: no input `{name}`"))?;
        let value: u8 = value.parse().ok().filter(|x| *x <= 15).ok_or_else(|| format!("level `{part}`: the value is 0..15"))?;
        v.push((t, i, value));
    }
    Ok(v)
}

/// Frames the circuit stands at rest before the first input (`circuit::stimulus::SETTLE`).
pub const SETTLE: u32 = 40;

/// Parse a stimulus (`circuit::stimulus::parse`): `;`-separated parts, each `[<input>] <edges>`
/// with edges `10+ 14-` or `pulse L every T x K [from S]`; the input may be left out when there
/// is one. `ticks` = the last edge + `tail`.
pub fn parse_stimulus(text: &str, names: &[String], tail: u32) -> Result<Schedule, String> {
    let mut s = Schedule { edges: vec![Vec::new(); names.len()], ticks: 0, events: Vec::new(), levels: Vec::new() };
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
    /// A strength input (the hex kinds): the design's chest, filled like the judge
    /// (`compute::strength_items`: 0 empty, 15 a full chest of 1728), its values parsed from
    /// `data-levels`.
    #[test]
    fn strength_inputs() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/calibration/hex-gt.txt")).unwrap();
        let d = Design::parse(&text).unwrap();
        let v = inputs(&d);
        assert_eq!(v.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
        assert!(v.iter().all(|i| matches!(i.how, InputHow::Strength { chest: Block::Chest { .. } })), "{v:?}");
        assert_eq!(strength_items(0), "{Items:[]}");
        assert_eq!(strength_items(1), "{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:1}]}");
        assert_eq!(strength_items(15).matches("count:64").count(), 27);
        let names = vec!["a".to_string(), "b".to_string()];
        assert_eq!(parse_levels("40 a 5; 40 b 3; 80 a 0", &names).unwrap(), vec![(40, 0, 5), (40, 1, 3), (80, 0, 0)]);
        assert!(parse_levels("40 c 5", &names).is_err() && parse_levels("40 a 16", &names).is_err());
    }

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
        // rail2: two rows of three ITEMS chests 6 apart, two OUT chests: 2 ITEMS + 2 OUT + BOXES
        let rail2 = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/shulker/archive/shulker-speed-unloader-rail2.txt")).unwrap();
        let g = port_groups(&Design::parse(&rail2).unwrap());
        let labels: Vec<(&str, usize)> = g.iter().map(|g| (g.label.as_str(), g.cells.len())).collect();
        assert_eq!(labels, vec![("BOXES", 1), ("ITEMS A–C", 3), ("ITEMS D–F", 3), ("OUT A", 1), ("OUT B", 1)]);
        assert_eq!(phase_words("gate and", "10", &["a".into(), "b".into()]), "A on, B off");
        assert_eq!(phase_words("gate and", "00 (rest)", &["a".into(), "b".into()]), "all inputs off");
        assert_eq!(phase_words("pulse-limiter", "pulse 3", &["in".into()]), "an input pulse of 3 ticks (0.15 s)");
        assert_eq!(phase_words("tff", "A 2 pulse 4", &["a".into()]), "A: 2nd pulse, 4 ticks (0.2 s)");
        assert_eq!(phase_words("pulsemult", "clock 1300", &["in".into()]), "a clock, every 1,300 ticks (1 min 5 s)");
        assert_eq!(ticks_words(1), "1 tick (0.05 s)");
        assert_eq!(ticks_words(6), "6 ticks (0.3 s)");
        assert_eq!(phase_words("counter 2", "pulse 2", &["in".into()]), "2nd input pulse");
        assert_eq!(phase_words("piston-extender 2", "cycle 2", &["in".into()]), "2nd cycle: out and back");
        assert_eq!(phase_words("bud", "place 1 t", &[]), "a block placed on TARGET");
        assert_eq!(phase_words("d-latch", "d=1 e=0", &["d".into(), "e".into()]), "D on, E off");
        assert_eq!(phase_words("x", "something new", &[]), "something new");
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
        // R7: driven by the judge's repeater from outside, the first direction out of the build
        assert!(v.iter().all(|i| i.how == InputHow::Driven { dir: [0, 0, -1], through: false }), "{v:?}");
        assert_eq!(driver_cells(v[0].pos, [0, 0, -1]), ([0, 0, -1], [0, -1, -1], [0, 0, -2]));
        assert_eq!(outputs(&d), vec![("OUT".to_string(), [1, 1, 0])]);
    }
}
