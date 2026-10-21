//! Minecart clocks (design entities: `clock verify` summons the design's `entity:` lines).
//!
//! Powered rails are powered by switched-on LEVERS (vjeux 2026-10-19: redstone blocks are too
//! expensive), one per stretch of consecutive powered rails: the lever sits on the outer face of
//! the support block under the stretch's middle rail, which it strongly powers; a powered rail
//! passes the power on to up to 8 more rails each way, so a stretch of up to 17 rails takes one
//! lever (longer stretches take one more per 17 rails). A stretch next to a detector rail takes no
//! lever: the detector powers it while the cart is on or just past it (vjeux 2026-10-20).
//!
//! `cart-pingpong` (extendable family `clk-misc-cart-pingpong`): an empty minecart bounces between
//! two stone bumpers on a straight east-west track `S P D L P..P S`: the powered rail at each end
//! launches the resting cart away from the bumper behind it (0.02 b/t, then +0.06 b/t per tick on
//! powered rails, at most 0.4); a detector rail next to the west end turns on when the cart passes it
//! and off at its next 20 gt check once the cart has left, and that detector is what powers the west
//! launcher (no lever there); the east stretch holds n + 3 powered rails (its last one is the east
//! launcher, where the cart starts: the west launcher is unpowered until the cart first passes the
//! detector). Every bounce clamps the cart against a bumper, so
//! from the second lap its motion repeats exactly. `--bare`: `S P D L L P S` (no middle powered
//! rail); `--short`: `S P D L P P S`. `--track` builds any track from the letters S (bumper),
//! P (powered), D (detector), L (plain) for exploration.
//!
//! `cart-loop` (extendable family `clk-misc-cart-loop`): an empty minecart circling a 4 x (4 + n)
//! loop, plain rails at the corners, a powered rail in the middle of each short side and n + 1 on
//! each long side (one lever per stretch: 4), a detector rail on the east side. `--three`: the
//! 3 x 3 loop (one powered rail in the middle of the north and south sides) whose short lap beats
//! against the detector rail's 20 gt checks.

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use clock::Design;
use std::path::PathBuf;

const POWERED_EW: &str = "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]";
const POWERED_NS: &str = "minecraft:powered_rail[powered=true,shape=north_south,waterlogged=false]";
const PLAIN_EW: &str = "minecraft:rail[shape=east_west,waterlogged=false]";
const PLAIN_NS: &str = "minecraft:rail[shape=north_south,waterlogged=false]";
const DETECTOR_EW: &str = "minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]";
const DETECTOR_NS: &str = "minecraft:detector_rail[powered=false,shape=north_south,waterlogged=false]";
const SUPPORT: &str = "minecraft:stone";

fn dir_name(d: [i32; 3]) -> &'static str {
    match d {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        _ => "up",
    }
}

/// Levers for one stretch of consecutive powered rails (positions at track level, in order): on
/// the `out` face of the supports under the rails at indices that leave no rail more than 8 rails
/// from a lever. Returns how many levers were placed.
fn levers(d: &mut Design, run: &[[i32; 3]], out: [i32; 3]) -> usize {
    if run.is_empty() {
        return 0;
    }
    let mut at = Vec::new();
    if run.len() <= 17 {
        at.push((run.len() - 1) / 2);
    } else {
        let mut i = 8;
        while i < run.len() {
            at.push(i);
            i += 17;
        }
        if at.last().map_or(true, |&l| l + 8 < run.len() - 1) {
            at.push(run.len() - 1);
        }
    }
    for &i in &at {
        let [x, y, z] = run[i];
        d.blocks.insert([x + out[0], y - 1 + out[1], z + out[2]], format!("minecraft:lever[face=wall,facing={},powered=true]", dir_name(out)));
    }
    at.len()
}

fn with_caption(d: &Design, caption: &str) -> String {
    let text = d.to_text();
    match text.split_once('\n') {
        Some((first, rest)) => format!("{first}\n{caption}{rest}"),
        None => format!("{caption}{text}"),
    }
}

#[derive(ClapArgs, Debug, Clone)]
pub struct PingpongArgs {
    /// The family's n: the east stretch holds n + 3 powered rails.
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// `S P D L L P S`: no middle powered rail (not a family member).
    #[arg(long)]
    pub bare: bool,
    /// `S P D L P P S`: an east stretch of 2 (P = 33, off the family's line).
    #[arg(long)]
    pub short: bool,
    /// Any straight track from the letters S (bumper), P (powered rail), D (detector rail),
    /// L (plain rail), west to east; the cart starts on the second cell (exploration).
    #[arg(long)]
    pub track: Option<String>,
    /// Write the design here (else print it).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// The straight track `cells` (letters S P D L) along +x at y = 0, supports below the rails,
/// levers on the north faces of the supports; returns the design and its lever count.
fn straight(name: &str, cells: &str) -> Result<(Design, usize)> {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    let letters: Vec<char> = cells.chars().collect();
    let mut run: Vec<[i32; 3]> = Vec::new();
    let mut n_levers = 0;
    // a stretch next to a detector rail takes no lever: the detector powers it while the cart is
    // there (vjeux 2026-10-20: "You don't need the rightmost lever. The rails gets powered by the
    // detector")
    let next_to_detector = |run: &[[i32; 3]]| -> bool {
        let (a, b) = (run[0][0] - 1, run[run.len() - 1][0] + 1);
        [a, b].iter().any(|&x| x >= 0 && letters.get(x as usize) == Some(&'D'))
    };
    // the cart starts on the last powered rail of a lever-powered stretch (the east launcher)
    let mut start: Option<i32> = None;
    let mut close = |d: &mut Design, run: &mut Vec<[i32; 3]>, start: &mut Option<i32>| -> usize {
        if run.is_empty() {
            return 0;
        }
        let n = if next_to_detector(run) {
            0
        } else {
            *start = Some(run[run.len() - 1][0]);
            levers(d, run, [0, 0, -1])
        };
        run.clear();
        n
    };
    for (i, c) in letters.iter().copied().enumerate() {
        let x = i as i32;
        if c != 'P' {
            n_levers += close(&mut d, &mut run, &mut start);
        }
        match c {
            'S' => {
                d.blocks.insert([x, 0, 0], SUPPORT.to_string());
            }
            'P' | 'D' | 'L' => {
                let rail = match c {
                    'P' => POWERED_EW,
                    'D' => DETECTOR_EW,
                    _ => PLAIN_EW,
                };
                d.blocks.insert([x, 0, 0], rail.to_string());
                d.blocks.insert([x, -1, 0], SUPPORT.to_string());
                if c == 'P' {
                    run.push([x, 0, 0]);
                }
            }
            _ => bail!("track letters are S P D L, got {c:?}"),
        }
    }
    n_levers += close(&mut d, &mut run, &mut start);
    // no declared output: the judge takes its exterior lamp cell (the detector's cell is where the
    // cart runs)
    let x = start.unwrap_or(1) as f64 + 0.5;
    d.entities.push(mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [x, 0.0625, 0.5], nbt: None });
    Ok((d, n_levers))
}

pub fn build(n: u32, bare: bool, short: bool) -> Result<String> {
    let (cells, name, kind) = if bare {
        ("SPDLLPS".to_string(), "clk-misc-cart-pingpong-bare".to_string(), "bare (track S P D L L P S")
    } else if short {
        ("SPDLPPS".to_string(), "clk-misc-cart-pingpong-short".to_string(), "short (track S P D L P P S")
    } else {
        (format!("SPDL{}S", "P".repeat(n as usize + 3)), format!("clk-misc-cart-pingpong-n{n}"), "")
    };
    let (mut d, n_levers) = straight(&name, &cells)?;
    let pieces = d.blocks.len() + d.entities.len();
    let member = !bare && !short;
    if member {
        d.meta.insert("extend family".into(), "clk-misc-cart-pingpong".into());
        d.meta.insert("extend period".into(), PINGPONG_PERIOD.into());
        d.meta.insert("extend blocks".into(), "16 + 2n".into());
        d.meta.insert("extend rule".into(), "one more powered rail (on a stone support) in the east stretch".into());
        d.meta.insert("extend n".into(), "0..14".into());
        d.meta.insert("extend base".into(), n.to_string());
        d.meta.insert("extend generator".into(), "mctas clock misc cart-pingpong --n {n} --out {out}".into());
    }
    let head = if member {
        format!(
            "Extendable family clk-misc-cart-pingpong (n = {n}): P = {PINGPONG_PERIOD} gt, 16 + 2n = {pieces} pieces (blocks, {n_levers} lever and the cart)"
        )
    } else {
        format!("Minecart ping-pong, {kind}, {pieces} pieces: blocks, {n_levers} levers and the cart)")
    };
    let caption = format!(
        "# {head}. An empty minecart bounces between two stone bumpers on a\n\
         # straight track: the powered rail at each end launches the resting cart away from the bumper behind it and speeds it up; it\n\
         # coasts over the plain rails, is sped up by the east stretch of powered rails, stops against the far bumper and is launched back.\n\
         # A switched-on lever on a support powers the east stretch; the detector rail next to the west end powers the west launcher: it\n\
         # turns on as the cart passes and off at its next 20 gt check once the cart has left. The cart starts on the east launcher. Every\n\
         # bounce clamps the cart against a bumper, so from the second lap its motion repeats exactly.\n"
    );
    Ok(with_caption(&d, &caption))
}

/// Measured in the game for the lever layout `S P D L P^(n+3) S` (n = 0, 1, 2, 5, 9, 14; the
/// stretch of 2, `--short`, gives 33, off the line).
pub const PINGPONG_PERIOD: &str = "37 + 5n";

pub fn run(a: PingpongArgs) -> Result<()> {
    let text = match &a.track {
        Some(t) => {
            let (d, _) = straight(&format!("clk-misc-cart-track-{}", t.to_lowercase()), t)?;
            d.to_text()
        }
        None => build(a.n, a.bare, a.short)?,
    };
    match &a.out {
        Some(p) => std::fs::write(p, &text)?,
        None => print!("{text}"),
    }
    Ok(())
}

#[derive(ClapArgs, Debug, Clone)]
pub struct LoopArgs {
    /// The family's n: the loop is 4 x (4 + n) cells.
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// The 3 x 3 loop (not a family member).
    #[arg(long)]
    pub three: bool,
    /// Powered rails on the long sides only (2 levers; not a family member).
    #[arg(long)]
    pub long_only: bool,
    /// Write the design here (else print it).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// One loop cell: its rail kind and, for a straight cell, its axis.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Plain,
    Powered,
    Detector,
}

/// A w x h rail loop at y = 0 (x 0..w, z 0..h), corners curved; `kind(x, z)` for the straight
/// cells; levers on the outer faces of the supports, one per stretch of powered rails per side.
fn rail_loop(name: &str, w: i32, h: i32, kind: &dyn Fn(i32, i32) -> Kind) -> (Design, usize) {
    let mut d = Design { name: name.to_string(), ..Default::default() };
    let corner = |x: i32, z: i32| -> Option<&'static str> {
        match (x == 0, x == w - 1, z == 0, z == h - 1) {
            (true, _, true, _) => Some("minecraft:rail[shape=south_east,waterlogged=false]"),
            (_, true, true, _) => Some("minecraft:rail[shape=south_west,waterlogged=false]"),
            (_, true, _, true) => Some("minecraft:rail[shape=north_west,waterlogged=false]"),
            (true, _, _, true) => Some("minecraft:rail[shape=north_east,waterlogged=false]"),
            _ => None,
        }
    };
    let mut n_levers = 0;
    // The four sides: cells in order and the outward direction.
    let sides: [(Vec<(i32, i32)>, [i32; 3], bool); 4] = [
        ((1..w - 1).map(|x| (x, 0)).collect(), [0, 0, -1], true),
        ((1..h - 1).map(|z| (w - 1, z)).collect(), [1, 0, 0], false),
        ((1..w - 1).map(|x| (x, h - 1)).collect(), [0, 0, 1], true),
        ((1..h - 1).map(|z| (0, z)).collect(), [-1, 0, 0], false),
    ];
    for x in 0..w {
        for z in 0..h {
            if let Some(c) = corner(x, z) {
                d.blocks.insert([x, 0, z], c.to_string());
                d.blocks.insert([x, -1, z], SUPPORT.to_string());
            }
        }
    }
    for (cells, out, ew) in sides.iter() {
        let mut run: Vec<[i32; 3]> = Vec::new();
        for &(x, z) in cells {
            let k = kind(x, z);
            let rail = match (k, ew) {
                (Kind::Powered, true) => POWERED_EW,
                (Kind::Powered, false) => POWERED_NS,
                (Kind::Detector, true) => DETECTOR_EW,
                (Kind::Detector, false) => DETECTOR_NS,
                (Kind::Plain, true) => PLAIN_EW,
                (Kind::Plain, false) => PLAIN_NS,
            };
            d.blocks.insert([x, 0, z], rail.to_string());
            d.blocks.insert([x, -1, z], SUPPORT.to_string());
            if k == Kind::Powered {
                run.push([x, 0, z]);
            } else if !run.is_empty() {
                n_levers += levers(&mut d, &run, *out);
                run.clear();
            }
        }
        n_levers += levers(&mut d, &run, *out);
    }
    (d, n_levers)
}

/// Measured in the game for the lever layout with a powered rail in the middle of each short side
/// (n = 0, 1, 2, 10, 16). Powered rails on the long sides only (`--long-only`, 2 levers) give
/// 75 / 33 gt at 4 x 4 / 4 x 5 (the cart slows on the short sides and beats against the detector
/// rail's 20 gt checks) and 26 + 5n only from 4 x 6 on.
pub const LOOP_PERIOD: &str = "26 + 5n";

pub fn build_loop(n: u32, three: bool, long_only: bool) -> String {
    if three {
        let (mut d, n_levers) = rail_loop("clk-misc-cart-loop3", 3, 3, &|x, z| {
            if x == 1 && (z == 0 || z == 2) {
                Kind::Powered
            } else if x == 2 && z == 1 {
                Kind::Detector
            } else {
                Kind::Plain
            }
        });
        d.ports.insert("output".into(), [2, 0, 1]);
        d.entities.push(mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [1.5, 0.0625, 0.5], nbt: Some("{Motion:[0.4d,0.0d,0.0d]}".into()) });
        let pieces = d.blocks.len() + d.entities.len();
        let caption = format!(
            "# A minecart circling a 3x3 rail loop ({pieces} pieces: blocks, {n_levers} levers and the cart): plain rails curve at the corners, a powered\n\
             # rail in the middle of the north and south sides (each powered by a switched-on lever on its support) keeps the empty cart going,\n\
             # a detector rail on the east side is powered while the cart is on it and turns off at its next 20 gt check once the cart has\n\
             # left. The short lap and the detector's 20 gt checks beat against each other, so the block states repeat only after 28 laps.\n"
        );
        return with_caption(&d, &caption);
    }
    let deep = 4 + n as i32;
    let name = if long_only { format!("clk-misc-cart-loop-long-n{n}") } else { format!("clk-misc-cart-loop-n{n}") };
    let (mut d, n_levers) = rail_loop(&name, 4, deep, &|x, z| {
        if x == 3 && z == 1 {
            Kind::Detector
        } else if (x == 0 || x == 3) && z >= 2 && z <= deep - 2 {
            Kind::Powered
        } else if !long_only && ((z == 0 && x == 1) || (z == deep - 1 && x == 2)) {
            Kind::Powered
        } else {
            Kind::Plain
        }
    });
    d.ports.insert("output".into(), [3, 0, 1]);
    d.entities.push(mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [1.5, 0.0625, 0.5], nbt: Some("{Motion:[0.4d,0.0d,0.0d]}".into()) });
    let pieces = d.blocks.len() + d.entities.len();
    if !long_only {
        d.meta.insert("extend family".into(), "clk-misc-cart-loop".into());
        d.meta.insert("extend period".into(), LOOP_PERIOD.into());
        d.meta.insert("extend blocks".into(), "29 + 4n".into());
        d.meta.insert("extend rule".into(), "lengthen both long sides by one powered rail (on a stone support)".into());
        d.meta.insert("extend n".into(), "0..16".into());
        d.meta.insert("extend base".into(), n.to_string());
        d.meta.insert("extend generator".into(), "mctas clock misc cart-loop --n {n} --out {out}".into());
    }
    let caption = format!(
        "# {}minecart loop (n = {n}: {pieces} pieces, {n_levers} levers): an empty minecart circles a 4 x {deep}\n\
         # rail loop; plain rails curve at the corners; {}{} powered rails on each long side keep it near its 0.4 b/t cap\n\
         # (a switched-on lever on a support powers each stretch of powered rails); a detector rail on the east side is powered while the\n\
         # cart is on it and turns off at its next 20 gt check once the cart has left (one off / on per lap).\n",
        if long_only { String::new() } else { format!("Extendable family clk-misc-cart-loop, P = {LOOP_PERIOD} gt, 29 + 4n pieces: ") },
        if long_only { "" } else { "one powered rail in the middle of each short side and " },
        n + 1
    );
    with_caption(&d, &caption)
}

pub fn run_loop(a: LoopArgs) -> Result<()> {
    let text = build_loop(a.n, a.three, a.long_only);
    match &a.out {
        Some(p) => std::fs::write(p, &text)?,
        None => print!("{text}"),
    }
    Ok(())
}
