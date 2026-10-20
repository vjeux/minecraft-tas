//! Relay rings: one extendable clock family per relay block (`clk-misc-<relay>-ring`).
//!
//! The ring is a 2 x h loop of cells in the x-y plane (z = 0), h = 3 + n: up the left column, down
//! the right one. The relay sits at the top left (0, h-1); every other cell holds an observer
//! facing the previous cell of the loop, so each observer watches the one before it. The observer
//! under the relay faces down: its back powers the relay and its top face carries it (rails, doors
//! and bells need that support). The observer right of the relay watches it. One pulse runs
//! around the loop, 2 gt per observer; the relay follows its power at once and adds no delay, so
//! P = 2 x (2h - 1) = 10 + 4n gt with 2h = 6 + 2n blocks (a door's upper half adds one).
//! Started by placing the observer right of the relay last.

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use clock::sim::{SimOptions, Verdict, simulate};
use clock::{Clock, Design, Pos};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct RingArgs {
    /// The relay block (`--list` shows them).
    #[arg(long, default_value = "crafter")]
    pub relay: String,
    /// The family's n: the ring is 3 + n cells high.
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// Write the design here.
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Build and simulate n = 0..=K, compare with the formula.
    #[arg(long)]
    pub check: Option<u32>,
    /// List the relays.
    #[arg(long)]
    pub list: bool,
}

/// A relay block of the ring.
pub struct Relay {
    pub name: &'static str,
    /// The block in the relay cell.
    pub state: &'static str,
    /// A second block right above it (a door's upper half).
    pub upper: Option<&'static str>,
    /// How it relays, for the caption.
    pub how: &'static str,
}

pub const RELAYS: &[Relay] = &[
    Relay {
        name: "crafter",
        state: "minecraft:crafter[crafting=false,orientation=north_up,triggered=false]",
        upper: None,
        how: "an empty crafter: its `triggered` state follows its power at once (the craft scheduled 4 gt later finds no recipe and changes nothing)",
    },
    Relay {
        name: "powered_rail",
        state: "minecraft:powered_rail[powered=false,shape=east_west,waterlogged=false]",
        upper: None,
        how: "a powered rail standing on the observer that powers it: `powered` follows its power at once",
    },
    Relay {
        name: "activator_rail",
        state: "minecraft:activator_rail[powered=false,shape=east_west,waterlogged=false]",
        upper: None,
        how: "an activator rail standing on the observer that powers it: `powered` follows its power at once",
    },
    Relay {
        name: "oak_fence_gate",
        state: "minecraft:oak_fence_gate[facing=north,in_wall=false,open=false,powered=false]",
        upper: None,
        how: "an oak fence gate: `open` and `powered` follow its power at once",
    },
    Relay {
        name: "oak_trapdoor",
        state: "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]",
        upper: None,
        how: "an oak trapdoor: `open` and `powered` follow its power at once",
    },
    Relay {
        name: "skeleton_skull",
        state: "minecraft:skeleton_skull[powered=false,rotation=0]",
        upper: None,
        how: "a skeleton skull: its `powered` state follows its power at once",
    },
    Relay {
        name: "iron_door",
        state: "minecraft:iron_door[facing=north,half=lower,hinge=left,open=false,powered=false]",
        upper: Some("minecraft:iron_door[facing=north,half=upper,hinge=left,open=false,powered=false]"),
        how: "an iron door standing on the observer that powers its lower half: both halves open and close with that power at once",
    },
    Relay {
        name: "oak_door",
        state: "minecraft:oak_door[facing=north,half=lower,hinge=left,open=false,powered=false]",
        upper: Some("minecraft:oak_door[facing=north,half=upper,hinge=left,open=false,powered=false]"),
        how: "an oak door standing on the observer that powers its lower half: both halves open and close with that power at once",
    },
];

pub fn relay(name: &str) -> Result<&'static Relay> {
    match RELAYS.iter().find(|r| r.name == name) {
        Some(r) => Ok(r),
        None => bail!("unknown relay {name:?}; relays: {}", RELAYS.iter().map(|r| r.name).collect::<Vec<_>>().join(", ")),
    }
}

/// The family's period and block count for n.
pub fn formula(r: &Relay, n: u32) -> (u32, usize) {
    (10 + 4 * n, 6 + 2 * n as usize + usize::from(r.upper.is_some()))
}

/// The loop cells in signal order, starting at the relay: (0, h-1), then down the right column,
/// then up the left one.
fn loop_cells(h: i32) -> Vec<Pos> {
    let mut c = vec![[0, h - 1, 0]];
    c.extend((0..h).rev().map(|y| [1, y, 0]));
    c.extend((0..h - 1).map(|y| [0, y, 0]));
    c
}

fn facing(d: Pos) -> &'static str {
    match d {
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        [0, -1, 0] => "down",
        [0, 0, 1] => "south",
        _ => "north",
    }
}

/// The ring for n, as design text (with the family's `extend` headers and a caption).
pub fn build(r: &Relay, n: u32) -> String {
    let h = 3 + n as i32;
    let cells = loop_cells(h);
    let mut d = Design { name: format!("clk-misc-{}-ring-n{n}", r.name), ..Default::default() };
    for (i, c) in cells.iter().enumerate() {
        if i == 0 {
            d.blocks.insert(*c, r.state.to_string());
            if let Some(u) = r.upper {
                d.blocks.insert([c[0], c[1] + 1, c[2]], u.to_string());
            }
            continue;
        }
        let p = cells[i - 1];
        d.blocks.insert(*c, format!("minecraft:observer[facing={},powered=false]", facing([p[0] - c[0], p[1] - c[1], p[2] - c[2]])));
    }
    // no declared output: a clock's output is an exterior air cell the judge picks (clockinfra,
    // 2026-10-20: `clock sim --exterior`; a component cell inside the build no longer counts)
    d.ports.insert("start".into(), cells[1]);
    d.meta.insert("port start".into(), "last".into());
    let (_, b0) = formula(r, 0);
    let extra = usize::from(r.upper.is_some());
    d.meta.insert("extend family".into(), format!("clk-misc-{}-ring", r.name));
    d.meta.insert("extend period".into(), "10 + 4n".into());
    d.meta.insert("extend blocks".into(), format!("{b0} + 2n"));
    d.meta.insert("extend rule".into(), "insert 2 observers: the 2-wide ring one row higher".into());
    d.meta.insert("extend n".into(), "0..47".into());
    d.meta.insert("extend base".into(), n.to_string());
    d.meta.insert("extend generator".into(), format!("mctas clock misc ring --relay {} --n {{n}} --out {{out}}", r.name));
    let (pn, bn) = formula(r, n);
    let caption = format!(
        "# Extendable family clk-misc-{}-ring (n = {n}): P = 10 + 4n gt = {pn} gt, {b0} + 2n = {bn} blocks. Relay: {}.\n\
         # It sits at the top of a 2-wide ring of {} observers (each watches the one before it, 2 gt per observer); the observer under it\n\
         # powers it and carries it, the next one watches it. The relay adds no delay but takes a lattice cell, so an odd number of\n\
         # observers closes the ring.{} Extension: insert 2 observers (the ring one row higher).\n\
         # Generator: mctas clock misc ring --relay {} --n N --out FILE.\n",
        r.name,
        r.how,
        bn - 1 - extra,
        if extra == 1 { " The door's upper half is one more block." } else { "" },
        r.name
    );
    let text = d.to_text();
    // The caption is the design's first comment block: right after the name line.
    match text.split_once('\n') {
        Some((first, rest)) => format!("{first}\n{caption}{rest}"),
        None => format!("{caption}{text}"),
    }
}

pub fn run(a: RingArgs) -> Result<()> {
    if a.list {
        println!("| relay | block | P | blocks |\n|---|---|---|---|");
        for r in RELAYS {
            let (_, b) = formula(r, 0);
            println!("| {} | {} | 10 + 4n | {b} + 2n |", r.name, r.state);
        }
        return Ok(());
    }
    let r = relay(&a.relay)?;
    let ns: Vec<u32> = match a.check {
        Some(k) => (0..=k).collect(),
        None => vec![a.n],
    };
    let mut bad = 0;
    for n in ns {
        let text = build(r, n);
        let c = Clock::from_design(Design::parse(&text)?)?;
        let (p, b) = formula(r, n);
        let rep = simulate(&c, &SimOptions { horizon: (40 * p).max(20_000), ..Default::default() });
        let ok = rep.verdict == Verdict::Clock && rep.period == p && c.blocks() == b;
        bad += usize::from(!ok);
        println!("{} n={n}: formula P={p} blocks={b}; redsim {} ({})", r.name, if ok { "OK" } else { "MISMATCH" }, rep.summary());
        if let Some(out) = &a.out {
            if a.check.is_none() {
                std::fs::write(out, &text)?;
            }
        }
    }
    if bad > 0 {
        bail!("{bad} instance(s) disagree with the formula");
    }
    Ok(())
}
