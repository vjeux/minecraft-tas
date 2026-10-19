//! Constructions for EVERY period: parametric families whose period is known by design, each
//! design simulated to confirm it.
//! * P = 0 mod 4 (P >= 8): a ring of P/2 observers around a 2 x P/4 rectangle (one pulse, 2 gt
//!   per observer).
//! * P = 2 mod 4 (P >= 10): a ring of P/2 observers and one note block (the note block follows its
//!   power at once but takes a lattice cell, so an odd number of observers closes a loop).
//! * odd P (P >= 11): the odd piston stage (oddring11/13 in designs/clocks/tracks/clk-obs): an
//!   observer quasi-powers a sticky piston and a second observer keeps it powered 4 gt, so the
//!   redstone block it pushes LANDS (block-entity phase) under a normal piston that fires the next
//!   tick: 5 gt from the stage's input observer to its output observer; a tower of observers closes
//!   the ring (2 gt each). Variant A pushes the redstone block itself (P = 4h - 1), variant B
//!   pushes a slime block that drags it one cell further (P = 4h + 1), h = the tower's height.

use crate::sim::{self, Kick, Spec, Verdict};
use anyhow::Result;
use clap::Args as ClapArgs;
use redsim::{Block, BlockPos, Dir};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct EveryArgs {
    /// Smallest period to construct
    #[arg(long, default_value_t = 8)]
    pub min: u32,
    /// Largest period to construct
    #[arg(long, default_value_t = 64)]
    pub max: u32,
    /// Write the designs here
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

fn obs(facing: Dir) -> Block {
    Block::Observer { facing, powered: false }
}

/// A ring of observers along the given closed cell loop (each watches the previous cell), with
/// note blocks at the given loop indices.
fn ring(name: &str, cells: &[[i32; 3]], notes: &[usize]) -> Spec {
    ring_with(name, cells, notes, Block::parse("minecraft:note_block").unwrap())
}

/// A ring of observers with `relay` blocks at the given loop indices (each relay sits behind a
/// straight observer, which powers it, and is watched by the next observer).
pub fn ring_with(name: &str, cells: &[[i32; 3]], notes: &[usize], relay: Block) -> Spec {
    let n = cells.len();
    let mut blocks = Vec::new();
    for i in 0..n {
        let c = cells[i];
        let p = cells[(i + n - 1) % n];
        let d = [p[0] - c[0], p[1] - c[1], p[2] - c[2]];
        let b = if notes.contains(&i) { relay } else { obs(crate::rings::dir_of(d)) };
        blocks.push((BlockPos::new(c[0], c[1], c[2]), b));
    }
    let kick_at = (0..n).find(|i| !notes.contains(i) && !notes.contains(&((i + 1) % n))).unwrap_or(0);
    let k = BlockPos::new(cells[kick_at][0], cells[kick_at][1], cells[kick_at][2]);
    Spec { name: name.into(), blocks, kick: Kick::Last(k), output: Some(k) }
}

/// The rectangle loop 2 x w in the xz plane (x from 0 to w-1), in loop order.
pub fn rectangle(w: i32) -> Vec<[i32; 3]> {
    let mut c: Vec<[i32; 3]> = (0..w).map(|x| [x, 0, 0]).collect();
    c.extend((0..w).rev().map(|x| [x, 0, 1]));
    c
}

/// P = 0 mod 4: P/2 observers.
pub fn even_ring(p: u32) -> Spec {
    ring(&format!("clk-obs-ring-p{p}"), &rectangle(p as i32 / 4), &[])
}

/// P = 2 mod 4: P/2 observers + 1 note block, on a 2 x (P/2+1)/2 rectangle; the note block sits at
/// loop index 2, right after the straight top-row observer 1 (whose back faces it), and the next
/// observer watches it.
pub fn note_ring(p: u32) -> Spec {
    let n = p as i32 / 2 + 1; // cells
    let cells = rectangle(n / 2);
    ring(&format!("clk-obs-notering-p{p}"), &cells, &[2])
}

/// Odd P >= 11: the odd piston stage + an observer tower of height h.
pub fn odd_ring(p: u32) -> Option<Spec> {
    if p < 11 || p % 2 == 0 {
        return None;
    }
    let b = BlockPos::new;
    let variant_b = p % 4 == 1;
    let h: i32 = if variant_b { (p as i32 - 1) / 4 } else { (p as i32 + 1) / 4 };
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    let stk = Block::Piston { sticky: true, facing: Dir::East, extended: false };
    let piston_up = Block::Piston { sticky: false, facing: Dir::Up, extended: false };
    blocks.push((b(0, 0, 0), stk));
    blocks.push((b(0, 1, 0), obs(Dir::Up))); // O2: powers S from above
    blocks.push((b(0, 2, 0), obs(Dir::Up))); // O1: quasi-powers S, watches the tower's last cell
    let jx = if variant_b {
        blocks.push((b(1, 0, 0), Block::parse("minecraft:slime_block").unwrap()));
        blocks.push((b(2, 0, 0), Block::RedstoneBlock));
        blocks.push((b(3, 1, 0), piston_up));
        2
    } else {
        blocks.push((b(1, 0, 0), Block::RedstoneBlock));
        blocks.push((b(2, 1, 0), piston_up));
        1
    };
    blocks.push((b(jx, 2, 0), obs(Dir::East))); // J watches Q's head cell
    // tower: up column x = jx from y = 3 to h (facing down), across the top row to x = 0 (facing
    // east), down column x = 0 from y = h - 1 to 3 (facing up); O1 watches (0, 3, 0)
    for y in 3..=h {
        blocks.push((b(jx, y, 0), obs(Dir::Down)));
    }
    for x in (0..jx).rev() {
        blocks.push((b(x, h, 0), obs(Dir::East)));
    }
    for y in (3..h).rev() {
        blocks.push((b(0, y, 0), obs(Dir::Up)));
    }
    let kick = b(jx, 3, 0);
    Some(Spec { name: format!("clk-obs-oddring-p{p}"), blocks, kick: Kick::Last(kick), output: Some(b(jx, 2, 0)) })
}

pub fn construct(p: u32) -> Option<(Spec, &'static str)> {
    if p % 2 == 1 {
        odd_ring(p).map(|s| (s, "odd piston stage + observer tower"))
    } else if p % 4 == 0 && p >= 8 {
        Some((even_ring(p), "observer ring"))
    } else if p % 4 == 2 && p >= 10 {
        Some((note_ring(p), "observer ring + note block"))
    } else {
        None
    }
}

pub fn run(a: EveryArgs) -> Result<()> {
    if let Some(d) = &a.out_dir {
        std::fs::create_dir_all(d)?;
    }
    println!("| P | construction | blocks | redsim | file |");
    println!("|---|---|---|---|---|");
    for p in a.min..=a.max {
        let Some((spec, how)) = construct(p) else {
            println!("| {p} | (none: use the census) | | | |");
            continue;
        };
        let r = sim::simulate(&spec, &sim::Opts { horizon: 20000, ..Default::default() });
        let ok = r.verdict == Verdict::Clock && r.period == p;
        let file = format!("{}.txt", spec.name.trim_start_matches("clk-obs-"));
        if let (Some(d), true) = (&a.out_dir, ok) {
            std::fs::write(d.join(&file), spec.to_text(&format!("Construction for P = {p}: {how} (crates/clock-obs/src/construct.rs). redsim: P = {}, family {}.", r.period, r.family_name())))?;
        }
        println!("| {p} | {how} | {} | {} | {} |", spec.blocks.len(), if ok { format!("P={}", r.period) } else { format!("MISMATCH {:?} P={}", r.verdict, r.period) }, file);
    }
    Ok(())
}
