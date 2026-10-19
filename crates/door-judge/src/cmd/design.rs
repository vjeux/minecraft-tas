//! `door turn`, `door gen`, `door patch`, `door blocks`, `door trace`: design files (turn,
//! generate, patch, list, trace).

use anyhow::Result;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct TurnArgs {
    design: PathBuf,
    /// Quarter turns clockwise seen from above.
    #[arg(long, default_value_t = 1)]
    turns: u8,
    /// Mirror image (east-west) before turning.
    #[arg(long)]
    mirror: bool,
    #[arg(long)]
    out: PathBuf,
}

pub fn cmd_turn(version: &str, args: TurnArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let TurnArgs { design, turns, mirror, out } = args;
    let d = load_design(&design)?;
    let mut w = Worker::spawn(&cfg)?;
    let (mut rd, spec) = crate::transform_design(&mut w, &d, turns, mirror)?;
    rd.name = format!("{}-{}{}", d.name, if spec.along_z { "ew" } else { "ns" }, if mirror { "-mirrored" } else { "" });
    rd.save(&out)?;
    println!("{} turned {} degrees{} ({}-facing door) written to {}", d.name, turns as u32 % 4 * 90, if mirror { ", mirrored" } else { "" }, if spec.along_z { "east/west" } else { "north/south" }, out.display());
    Ok(())
}

#[derive(clap::Args)]
pub struct GenArgs {
    name: String,
    /// Write to this file instead of stdout.
    #[arg(long)]
    out: Option<PathBuf>,
}

pub fn cmd_gen(version: &str, args: GenArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let GenArgs { name, out } = args;
    let d0 = crate::designs::build(&name).ok_or_else(|| anyhow::anyhow!("no design {name}; have {:?}", crate::designs::names()))?;
    let mut w = Worker::spawn(&cfg)?;
    let d = crate::canonicalize(&mut w, &d0)?;
    match out {
        Some(path) => d.save(&path)?,
        None => print!("{}", d.to_text()),
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct PatchArgs {
    design: PathBuf,
    script: PathBuf,
    #[arg(long)]
    out: PathBuf,
}

pub fn cmd_patch(version: &str, args: PatchArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let PatchArgs { design, script, out } = args;
    let d0 = load_design(&design)?;
    let text = std::fs::read_to_string(&script)?;
    let d1 = crate::patch::apply(&d0, &text)?;
    let mut w = Worker::spawn(&cfg)?;
    let d = crate::canonicalize(&mut w, &d1)?;
    d.save(&out)?;
    println!("{} -> {} blocks, written to {}", crate::block_count(&d0), crate::block_count(&d), out.display());
    Ok(())
}

#[derive(clap::Args)]
pub struct BlocksArgs {
    design: PathBuf,
    /// Only blocks inside this box, "x0,y0,z0:x1,y1,z1" (inclusive).
    #[arg(long)]
    only: Option<String>,
}

pub fn cmd_blocks(args: BlocksArgs) -> Result<()> {
    let BlocksArgs { design, only } = args;
    let d = load_design(&design)?;
    let bx: Option<([i32; 3], [i32; 3])> = match &only {
        None => None,
        Some(s) => {
            let (a, b) = anyhow::Context::context(s.split_once(':'), "--only wants x0,y0,z0:x1,y1,z1")?;
            let p = |t: &str| -> Result<[i32; 3]> {
                let v: Vec<i32> = t.split(',').map(|n| n.trim().parse()).collect::<std::result::Result<_, _>>()?;
                anyhow::ensure!(v.len() == 3, "a corner needs 3 coordinates: {t}");
                Ok([v[0], v[1], v[2]])
            };
            let (a, b) = (p(a)?, p(b)?);
            Some(([a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])], [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]))
        }
    };
    let mut ps: Vec<(&[i32; 3], &String)> = d.blocks.iter().filter(|(p, _)| bx.is_none_or(|(lo, hi)| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i]))).collect();
    ps.sort_by_key(|(p, _)| (p[1], p[2], p[0]));
    for (p, s) in ps {
        println!("{} {} {} {}", p[0], p[1], p[2], s.strip_prefix("minecraft:").unwrap_or(s));
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct TraceArgs {
    design: PathBuf,
    /// Ticks to record after each flip.
    #[arg(long, default_value_t = 40)]
    ticks: u32,
    /// Also flip the lever back and trace the closing.
    #[arg(long)]
    close: bool,
}

pub fn cmd_trace(version: &str, args: TraceArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let TraceArgs { design, ticks, close } = args;
    let d = load_design(&design)?;
    let mut w = Worker::spawn(&cfg)?;
    trace(&mut w, &d, ticks, close)?;
    Ok(())
}

fn short(s: &str) -> String {
    s.strip_prefix("minecraft:").unwrap_or(s).to_string()
}

pub fn trace(w: &mut Worker, d: &Design, ticks: u32, close: bool) -> Result<()> {
    use mctas::sim::{PlaceMode, SandboxOptions};
    let spec = DoorSpec::from_design(d)?;
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<_> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<_> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let settle = w.step_until_idle(sb, 400)?;
    println!("settled in {} ticks (idle {:?})", settle.ticked, settle.idle);
    let (mut lo, mut hi) = d.bounds().unwrap();
    for i in 0..3 {
        lo[i] -= 2;
        hi[i] += 2;
    }
    let mut prev = w.get_region(sb, lo, hi)?;
    for (p, s) in &blocks {
        if prev.get(*p) != Some(s.as_str()) {
            println!("  at rest: {p:?} {} -> {}", short(s), short(prev.get(*p).unwrap_or("?")));
        }
    }
    let flips = if close { 2 } else { 1 };
    for flip in 0..flips {
        println!("== lever {} ==", if flip == 0 { "ON" } else { "OFF" });
        w.use_block(sb, spec.lever)?;
        let r = w.step_record(sb, ticks, false, lo, hi)?;
        for (t, f) in r.frames.iter().enumerate() {
            let mut changes = Vec::new();
            for (p, s) in f.iter() {
                let before = prev.get(p).unwrap_or("?");
                if before != s {
                    changes.push(format!("{p:?} {} -> {}", short(before), short(s)));
                }
            }
            if !changes.is_empty() {
                println!("t={:<3} {}", t + 1, changes.join("\n      "));
            }
            prev = f.clone();
        }
    }
    Ok(())
}
