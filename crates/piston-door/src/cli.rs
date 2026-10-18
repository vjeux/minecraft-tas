//! `mctas door ...`

use crate::{DoorSpec, VerifyOptions, timeline, verify};
use anyhow::Result;
use clap::Subcommand;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Route a dust path in a design (prints the blocks to add).
    Route {
        design: PathBuf,
        /// Start cells (existing dust of the net), "x,y,z;x,y,z".
        #[arg(long)]
        start: String,
        /// Goal cell (becomes dust).
        #[arg(long)]
        goal: String,
        /// Extra cells the path may touch, "x,y,z;...".
        #[arg(long, default_value = "")]
        allow: String,
    },
    /// Drive a design by hand from a script (see drive.rs) and print the z=0 plane per tick.
    Drive { script: PathBuf },
    /// Greedily remove blocks while the door keeps working; writes the result.
    Minimize {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Worker JVMs.
        #[arg(long, default_value_t = 32)]
        workers: usize,
    },
    /// Record the door opening and closing with the real Minecraft client (needs Xvfb).
    Video {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        /// Simulation ticks per second while recording (20 = real time).
        #[arg(long, default_value_t = 10.0)]
        tps: f64,
        /// Upload to this GitHub release tag of vjeux/minecraft-tas.
        #[arg(long)]
        upload: Option<String>,
    },
    /// Print a built-in design (see designs.rs) in the text format.
    Gen {
        name: String,
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Place a design, settle, flip the lever and print every block change tick by tick.
    Trace {
        design: PathBuf,
        /// Ticks to record after each flip.
        #[arg(long, default_value_t = 40)]
        ticks: u32,
        /// Also flip the lever back and trace the closing.
        #[arg(long)]
        close: bool,
    },
    /// Check that a design is a working 3x3 piston door; print timings and block count.
    Verify {
        design: PathBuf,
        /// Print the tick-by-tick opening/closing of the 3x3 opening.
        #[arg(long)]
        timeline: bool,
        /// Open/close cycles per position.
        #[arg(long, default_value_t = 3)]
        cycles: u32,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::Route { design, start, goal, allow } => {
            let d = load_design(&design)?;
            let cells = |s: &str| -> Vec<[i32; 3]> {
                s.split(';').filter(|x| !x.trim().is_empty()).map(|c| {
                    let v: Vec<i32> = c.split(',').map(|n| n.trim().parse().unwrap()).collect();
                    [v[0], v[1], v[2]]
                }).collect()
            };
            let mut reserved = std::collections::HashSet::new();
            for x in -5..=7 { for y in -4..=7 { reserved.insert([x, y, 0]); } }
            for x in 0..=2 { for y in 0..=2 { for z in [-2, -1, 1, 2] { reserved.insert([x, y, z]); } } }
            reserved.insert([1, 3, 1]);
            let r = crate::route::Router { blocks: &d.blocks, reserved, allow: cells(&allow).into_iter().collect(), lo: [-12, -6, -8], hi: [12, 12, 10] };
            let goal = cells(&goal)[0];
            match r.route(&cells(&start), goal) {
                Some(path) => {
                    for (p, s) in path.iter().rev() {
                        println!("    b.set([{}, {}, {}], \"{}\");", p[0], p[1], p[2], s);
                    }
                    println!("    // {} new blocks", path.len());
                }
                None => anyhow::bail!("no route"),
            }
        }
        DoorCmd::Drive { script } => {
            let text = std::fs::read_to_string(&script)?;
            let mut w = Worker::spawn(&cfg)?;
            crate::drive::run(&mut w, &text)?;
        }
        DoorCmd::Minimize { design, out, workers } => {
            let d = load_design(&design)?;
            let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() })?;
            let t = std::time::Instant::now();
            let opts = VerifyOptions::default();
            let best = crate::minimize::minimize(&pool, &d, &opts, &|m| println!("[{:6.1}s] {m}", t.elapsed().as_secs_f64()))?;
            let mut w = pool.worker(0);
            let best = crate::canonicalize(&mut w, &best)?;
            best.save(&out)?;
            println!("{} -> {} blocks, written to {}", crate::block_count(&d), crate::block_count(&best), out.display());
        }
        DoorCmd::Video { design, out, tps, upload } => {
            let d = load_design(&design)?;
            let opts = crate::video::VideoOptions { tps, ..Default::default() };
            crate::video::record(version, &d, &out, &opts)?;
            println!("recorded {}", out.display());
            if let Some(tag) = upload {
                let url = mctas::video::upload_release_asset("vjeux/minecraft-tas", &tag, &out)?;
                println!("uploaded {url}");
            }
        }
        DoorCmd::Gen { name, out } => {
            let d0 = crate::designs::build(&name).ok_or_else(|| anyhow::anyhow!("no design {name}; have {:?}", crate::designs::names()))?;
            let mut w = Worker::spawn(&cfg)?;
            let d = crate::canonicalize(&mut w, &d0)?;
            match out {
                Some(path) => d.save(&path)?,
                None => print!("{}", d.to_text()),
            }
        }
        DoorCmd::Trace { design, ticks, close } => {
            let d = load_design(&design)?;
            let mut w = Worker::spawn(&cfg)?;
            trace(&mut w, &d, ticks, close)?;
        }
        DoorCmd::Verify { design, timeline: show, cycles } => {
            let d = load_design(&design)?;
            let spec = DoorSpec::from_design(&d)?;
            let mut w = Worker::spawn(&cfg)?;
            let opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
            match verify(&mut w, &d, &opts) {
                Ok(r) => {
                    println!("WORKS: {} ({} blocks, {}x{}x{})", d.name, r.blocks, r.size[0], r.size[1], r.size[2]);
                    for (i, c) in r.cycles.iter().enumerate() {
                        println!(
                            "  cycle {i}: open after {} ticks (idle at {}), closed after {} ticks (idle at {})",
                            c.open_ticks, c.open_settle, c.close_ticks, c.close_settle
                        );
                    }
                    for warning in &r.warnings {
                        println!("  warning: {warning}");
                    }
                    if show {
                        println!("opening (top row, middle row, bottom row; # block, ~ moving, h piston head, . air):");
                        print!("{}", timeline(&r.open_frames, &spec));
                        println!("closing:");
                        print!("{}", timeline(&r.close_frames, &spec));
                    }
                }
                Err(e) => {
                    println!("BROKEN: {}: {e:#}", d.name);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}

/// Load a design path, or a built-in design name prefixed with `@` (e.g. `@v1`).
pub fn load_design(path: &std::path::Path) -> Result<Design> {
    let s = path.to_string_lossy();
    if let Some(name) = s.strip_prefix('@') {
        return crate::designs::build(name).ok_or_else(|| anyhow::anyhow!("no built-in design {name}"));
    }
    Design::load(path)
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
