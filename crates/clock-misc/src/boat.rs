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
