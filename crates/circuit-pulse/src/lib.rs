//! Contraption study 1, track pulse (docs/circuits.md): `mctas circuit pulse <args...>`.
//! Edge detectors (rising, falling, dual), pulse limiters and pulse extenders: searches and
//! families. The official judge is `mctas circuit sim|verify` (crates/circuit); `screen` is a fast
//! redsim screen that reads EVERY cell around a design at once (to find where its output is),
//! `census` enumerates small structures with an input cell and keeps the best of each (kind, L).

pub mod family;
pub mod screen;
pub mod stamp;
pub mod unarchive;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "mctas circuit pulse", about = "Track pulse: edge detectors, pulse limiters and extenders")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Screen designs on redsim: the kind (rising/falling/dual-edge, limiter, extender, delay) at
    /// the declared output, or at every cell around the design without one
    Screen(screen::PulseArgs),
    /// Every structure of k blocks + an input cell, every output cell: the best designs of each
    /// (kind, L), written as circuit designs (`circuit:` header, input and output ports)
    Census(screen::PulseCensusArgs),
    /// Extendable families: the instance for any n, or the family card (`extend` headers)
    Family(family::FamilyArgs),
    /// Remove archived designs (file + verdict and caption rows): instances a family card covers
    Unarchive(unarchive::UnarchiveArgs),
    /// Put a tunable group's `group:` header on designs (the catalog shows a group as one card)
    StampGroup(stamp::StampArgs),
    /// One input pulse tick by tick on redsim: every block that changes and the output level
    Trace(TraceArgs),
    /// Each design's densest tiling on redsim: per axis, the smallest pitch at which 3 copies side
    /// by side each still judge as the design alone (the judge's `tile:` check, each copy driven by
    /// its own input, 7 gt apart then in phase); `--write D` writes the densest with its `tile:`
    Tile(TileArgs),
}

#[derive(clap::Args, Debug)]
struct TileArgs {
    designs: Vec<std::path::PathBuf>,
    /// The largest pitch tried per axis.
    #[arg(long, default_value_t = 3)]
    max: i32,
    /// Write each design with its densest `tile:` header into this directory.
    #[arg(long)]
    write: Option<std::path::PathBuf>,
    /// Also try the output read from each design block next to the output cell (a repeater,
    /// comparator, observer or torch reports its own state: no judge's lamp, so no crosstalk
    /// between neighboring copies' lamps); a variant counts when it judges exactly as the design.
    #[arg(long)]
    self_output: bool,
}

#[derive(clap::Args, Debug)]
struct TraceArgs {
    design: std::path::PathBuf,
    /// Input pulse length (gt)
    #[arg(long, default_value_t = 10)]
    w: u32,
    #[arg(long, default_value_t = 40)]
    ticks: u32,
}

/// `mctas circuit pulse <args...>`.
pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit pulse".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Screen(a) => screen::run(a),
        Cmd::Census(a) => screen::census(a),
        Cmd::Family(a) => family::run(a),
        Cmd::Unarchive(a) => unarchive::run(a),
        Cmd::StampGroup(a) => stamp::run(a),
        Cmd::Trace(a) => {
            let c = screen::Circuit::load(&a.design)?;
            screen::trace(&c, a.w, a.ticks).ok_or_else(|| anyhow::anyhow!("empty design"))
        }
        Cmd::Tile(a) => tile(a),
    }
}

/// `pulse tile`: per design and axis, the smallest pitch the judge's `tile:` check passes on redsim.
fn tile(a: TileArgs) -> anyhow::Result<()> {
    let o = circuit::verify::Options::default();
    let metrics = |d: &mctas::design::Design| -> Option<Vec<(String, String)>> {
        let c = circuit::Circuit::from_design(d.clone()).ok()?;
        let r = circuit::verify::sim(&c, &o).ok()?;
        r.outcome.filter(|out| out.pass).map(|out| out.metrics)
    };
    for p in &a.designs {
        let mut base = mctas::design::Design::load(p)?;
        base.meta.remove("tile");
        let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        // the design, then (--self-output) its variants reading the output from a block beside it
        let mut variants: Vec<(String, mctas::design::Design)> = vec![(String::new(), base.clone())];
        if a.self_output
            && let Some(want) = metrics(&base)
            && let Some(out) = base.ports.get("output").copied()
        {
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)] {
                let q = [out[0] + dx, out[1] + dy, out[2] + dz];
                if !base.blocks.contains_key(&q) {
                    continue;
                }
                let mut v = base.clone();
                v.ports.insert("output".into(), q);
                if metrics(&v).as_ref() == Some(&want) {
                    let short = base.blocks[&q].trim_start_matches("minecraft:").split('[').next().unwrap_or("").to_string();
                    variants.push((format!(" (output read from the {short})"), v));
                }
            }
        }
        for (note, d) in variants {
            let mut per: Vec<String> = Vec::new();
            let mut densest: Option<([i32; 3], i32)> = None;
            for (axis, unit) in [("x", [1, 0, 0]), ("y", [0, 1, 0]), ("z", [0, 0, 1])] {
                let mut found = None;
                for pitch in 1..=a.max {
                    let step = [unit[0] * pitch, unit[1] * pitch, unit[2] * pitch];
                    let mut dt = d.clone();
                    dt.meta.insert("tile".into(), format!("{} {} {}", step[0], step[1], step[2]));
                    let Ok(ct) = circuit::Circuit::from_design(dt) else { continue };
                    let Ok(r) = circuit::verify::sim(&ct, &o) else { continue };
                    if r.outcome.as_ref().is_some_and(|out| out.pass && out.metrics.iter().any(|(k, _)| k == "tileable")) {
                        found = Some(pitch);
                        if densest.is_none_or(|(_, q)| pitch < q) {
                            densest = Some((step, pitch));
                        }
                        break;
                    }
                }
                per.push(format!("{axis} {}", found.map(|p| p.to_string()).unwrap_or_else(|| format!("> {}", a.max))));
            }
            println!("{stem}{note}: densest pitch per axis: {}", per.join(", "));
            if let (Some(dir), Some((step, pitch))) = (&a.write, densest) {
                std::fs::create_dir_all(dir)?;
                let mut dt = d.clone();
                dt.meta.insert("tile".into(), format!("{} {} {}", step[0], step[1], step[2]));
                let suffix = if note.is_empty() { String::new() } else { "-self".to_string() };
                std::fs::write(dir.join(format!("{stem}{suffix}-tile{pitch}.txt")), dt.to_text())?;
            }
        }
    }
    Ok(())
}
