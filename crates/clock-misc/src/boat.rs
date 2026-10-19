//! Redstone-less boat clocks (vjeux 2026-10-19: water, a boat and wooden pressure plates).
//!
//! `boat-ring --a A --b B`: a boat circles a 2-wide water ring around an A x B island (A along x,
//! B along z), pushed by four one-way currents. Pressure plates block water, so a row of plates
//! at the end of each side cuts the ring into four currents; the first cells of the next corner,
//! right after the plate row, are water sources, whose water fills the corner and runs down the
//! next side to its plate row (at most 7 cells: A, B <= 6). The boat rides each current
//! (submerged), is thrown against the outer wall at each corner and slid along it by the turn,
//! and crawls over each plate row, pressing it. Stone, water, 8 oak pressure plates and the boat.
//! A = B = 2 is clk-misc-boat-ring4c.

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use clock::Design;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct RingArgs {
    /// Island size along x (the north and south sides' middle cells).
    #[arg(long, default_value_t = 2)]
    pub a: i32,
    /// Island size along z (the east and west sides' middle cells).
    #[arg(long, default_value_t = 2)]
    pub b: i32,
    /// Design name (default clk-misc-boat-ring-<a>x<b>).
    #[arg(long)]
    pub name: Option<String>,
    /// Write the design here (else print it).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

const STONE: &str = "minecraft:stone";
const SOURCE: &str = "minecraft:water[level=0]";
const PLATE: &str = "minecraft:oak_pressure_plate[powered=false]";

pub fn build(a: i32, b: i32, name: &str) -> Result<Design> {
    if !(1..=6).contains(&a) || !(1..=6).contains(&b) {
        bail!("island sides 1..=6 (a current reaches 7 cells)");
    }
    let (w, h) = (a + 4, b + 4); // channel cells x 0..w, z 0..h
    let mut d = Design { name: name.to_string(), ..Default::default() };
    let island = |x: i32, z: i32| (2..2 + a).contains(&x) && (2..2 + b).contains(&z);
    // the cells of each kind (plates: the last middle row of each side, in travel order: north
    // side +x, east side +z, south side -x, west side -z; sources: the next corner's first row)
    let mut plates = Vec::new();
    let mut sources = Vec::new();
    for t in 0..2 {
        plates.push([1 + a, t]); // north side, last middle column
        sources.push([2 + a, t]); // NE corner's first column
        plates.push([w - 2 + t, 1 + b]); // east side, last middle row
        sources.push([w - 2 + t, 2 + b]); // SE corner's first row
        plates.push([2, h - 2 + t]); // south side (travel -x): last middle column x = 2
        sources.push([1, h - 2 + t]); // SW corner's first column
        plates.push([t, 2]); // west side (travel -z): last middle row z = 2
        sources.push([t, 1]); // NW corner's first row
    }
    for x in -1..=w {
        for z in -1..=h {
            let inside = (0..w).contains(&x) && (0..h).contains(&z);
            let corner_out = (x == -1 || x == w) && (z == -1 || z == h);
            if !inside {
                if !corner_out {
                    d.blocks.insert([x, 0, z], STONE.to_string());
                }
                continue;
            }
            if island(x, z) {
                d.blocks.insert([x, 0, z], STONE.to_string());
                continue;
            }
            d.blocks.insert([x, -1, z], STONE.to_string());
            if plates.contains(&[x, z]) {
                d.blocks.insert([x, 0, z], PLATE.to_string());
            } else if sources.contains(&[x, z]) {
                d.blocks.insert([x, 0, z], SOURCE.to_string());
            }
        }
    }
    d.ports.insert("output".into(), [1 + a, 0, 0]);
    d.entities.push(mctas::design::EntitySpec { kind: "minecraft:oak_boat".into(), pos: [a as f64 + 2.8, 0.0, 1.0], nbt: None });
    Ok(d)
}

pub fn run(r: RingArgs) -> Result<()> {
    let name = r.name.clone().unwrap_or_else(|| format!("clk-misc-boat-ring-{}x{}", r.a, r.b));
    let d = build(r.a, r.b, &name)?;
    let caption = format!(
        "# A REDSTONE-LESS clock: a boat circles a 2-wide water ring around a {}x{} island, pushed by water currents, and presses\n\
         # wooden pressure plates. Pressure plates block water, so four rows of plates cut the ring into four one-way currents: the first\n\
         # cells of each corner (right after a plate row) are water sources, whose water fills the corner and runs down the next side to\n\
         # the next plate row. The boat rides each current (submerged), is thrown against the outer wall at each corner and slid along it by\n\
         # the turn, and crawls over each plate row (pressing it). Only stone, water, 8 oak pressure plates and the boat.\n",
        r.a, r.b
    );
    let text = d.to_text();
    let text = match text.split_once('\n') {
        Some((first, rest)) => format!("{first}\n{caption}{rest}"),
        None => text,
    };
    match &r.out {
        Some(p) => std::fs::write(p, &text)?,
        None => print!("{text}"),
    }
    Ok(())
}

#[derive(ClapArgs, Debug, Clone)]
pub struct LiftArgs {
    /// Length of the upper floor (the lower channel is L + 1 long, the drop gap 2 cells).
    #[arg(long, default_value_t = 4)]
    pub l: i32,
    /// Height of the upper water level above the lower one (the column is H + 1 sources tall).
    #[arg(long, default_value_t = 2)]
    pub h: i32,
    /// Design name (default clk-misc-boat-lift-l<L>h<H>).
    #[arg(long)]
    pub name: Option<String>,
    /// Hang a tripwire across the boat's flight above the column (hooks on posts at z = -1, 2).
    #[arg(long)]
    pub tripwire: bool,
    /// Write the design here (else print it).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

/// `boat-lift`: the vertical boat loop. Lower channel (x = 1..=L+1, y = 0) flowing -x, a plate row
/// at x = 0, a 2x2 upward bubble column (x = -2..=-1, y = 0..=H, soul sand below), wall banners
/// at x = 0, y = 1..H (water curtains the boat rises through, hung on blocks at x = 1), the upper
/// channel (y = H, over a floor x = 1..=L) fed by the column's top sources, a 2-cell drop gap
/// (x = L+1..=L+2), a landing plate row at x = L+2 (y = 0), end wall x = L+3, a ceiling 2 above the
/// upper water over the column. Lanes z = 0..=1.
pub fn build_lift(l: i32, h: i32, tripwire: bool, name: &str) -> Result<Design> {
    if !(1..=5).contains(&l) || !(2..=6).contains(&h) {
        bail!("L 1..=5 (the upper current reaches 7 cells), H 2..=6");
    }
    let mut d = Design { name: name.to_string(), ..Default::default() };
    let mut put = |x: i32, y: i32, z: i32, s: &str| {
        d.blocks.insert([x, y, z], s.to_string());
    };
    let end = l + 3;
    for z in 0..2 {
        // floors
        put(-2, -1, z, "minecraft:soul_sand");
        put(-1, -1, z, "minecraft:soul_sand");
        for x in 0..=l + 2 {
            put(x, -1, z, STONE);
        }
        // column, plates, banners
        for y in 0..=h {
            put(-2, y, z, SOURCE);
            put(-1, y, z, SOURCE);
            put(-3, y, z, STONE);
            put(end, y, z, STONE);
        }
        put(0, 0, z, PLATE);
        put(l + 2, 0, z, PLATE);
        for y in 1..h {
            put(0, y, z, "minecraft:white_wall_banner[facing=west]");
            put(1, y, z, STONE);
        }
        // upper floor
        for x in 1..=l {
            put(x, h - 1, z, STONE);
        }
        // ceiling over the column
        for x in -2..=1 {
            put(x, h + 2, z, STONE);
        }
    }
    // side walls: column (all levels), lower channel and plate rows (y = 0), upper channel (y = h)
    for zw in [-1, 2] {
        for y in 0..=h {
            put(-2, y, zw, STONE);
            put(-1, y, zw, STONE);
        }
        for x in 0..=l + 2 {
            put(x, 0, zw, STONE);
            put(x, h, zw, STONE);
        }
    }
    d.ports.insert("output".into(), [0, 0, 0]);
    if tripwire {
        // the boat flies up through y = h + 1 at x ~ 0: a string across both lanes there, hooks on
        // posts beside the column's side walls
        let y = h + 1;
        d.blocks.insert([0, y, -2], STONE.to_string());
        d.blocks.insert([0, y, 3], STONE.to_string());
        d.blocks.insert([0, y, -1], "minecraft:tripwire_hook[attached=true,facing=south,powered=false]".to_string());
        d.blocks.insert([0, y, 2], "minecraft:tripwire_hook[attached=true,facing=north,powered=false]".to_string());
        for z in 0..2 {
            d.blocks.insert(
                [0, y, z],
                "minecraft:tripwire[attached=true,disarmed=false,east=false,north=true,powered=false,south=true,west=false]".to_string(),
            );
        }
    }
    d.entities.push(mctas::design::EntitySpec { kind: "minecraft:oak_boat".into(), pos: [(l as f64 + 2.0) / 2.0 + 0.5, 0.0, 1.0], nbt: None });
    Ok(d)
}

pub fn run_lift(a: LiftArgs) -> Result<()> {
    let suffix = if a.tripwire { "-tripwire" } else { "" };
    let name = a.name.clone().unwrap_or_else(|| format!("clk-misc-boat-lift-l{}h{}{suffix}", a.l, a.h));
    let d = build_lift(a.l, a.h, a.tripwire, &name)?;
    let trip = if a.tripwire {
        "\n# A tripwire string hangs across the boat's flight just above the column: the boat trips it going up and coming down."
    } else {
        ""
    };
    let caption = format!(
        "# A REDSTONE-LESS vertical boat loop with a bubble-column lift (L = {}, H = {}): a boat rides a 2-wide water channel to a row\n\
         # of wooden pressure plates, crosses it into a 2x2 upward bubble column (water sources over soul sand) and is shot up (a ceiling\n\
         # stops it); it falls back on the column's top, whose sources feed an upper channel {} blocks higher that carries it to a 2-wide\n\
         # gap, where it drops back into the lower channel onto a second plate row (which keeps the landing water flowing back). Wall\n\
         # banners above the first plate row hold the column's water in while the boat rises through them (no collision, water cannot\n\
         # enter them). Stone, soul sand, water, wall banners, 4 oak pressure plates and the boat.{trip}\n",
        a.l, a.h, a.h
    );
    let text = d.to_text();
    let text = match text.split_once('\n') {
        Some((first, rest)) => format!("{first}\n{caption}{rest}"),
        None => text,
    };
    match &a.out {
        Some(p) => std::fs::write(p, &text)?,
        None => print!("{text}"),
    }
    Ok(())
}
