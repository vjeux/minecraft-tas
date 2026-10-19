//! Track gates (crates/circuit-gates, owned by clk-rt): logic gates, NOT and every 2-input
//! function as steady-state gates. `mctas circuit gates <command>`; the redsim harness is track
//! delay's (crates/circuit-delay) until clockinfra's judge is merged. Notes:
//! designs/circuits/tracks/gates/NOTES.md.

pub mod gate;

use anyhow::{Result, anyhow};
use circuit_delay::census::{self, Space};
use circuit_delay::harness::{self, InputKind, Pos, bp};
use clap::{Parser, Subcommand};
use redsim::Block;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gates", about = "Logic gates (track gates)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The gate checker on redsim: function, latency, glitches, blocks.
    Check {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 30)]
        hold: u32,
    },
    /// Exhaustive search: every filling of a box with kmin..=k palette blocks around fixed input
    /// cells (redstone blocks placed by the judge) and an output marker; the smallest design per
    /// function (then the lowest latency) and the fastest per function (then the fewest blocks).
    Census {
        /// Box AxBxC (x, y, z), cells 0..A etc.
        #[arg(long, default_value = "3x2x3")]
        dims: String,
        /// Input cells `x,y,z;x,y,z` (input.a first).
        #[arg(long)]
        inputs: String,
        #[arg(long, default_value = "stone,rblock,dust,torch,rep,cmp")]
        palette: String,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        #[arg(long, default_value_t = 3)]
        k: usize,
        #[arg(long, default_value_t = 20)]
        hold: u32,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        /// Write the best design of every key here.
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
}

fn parse_pos(s: &str) -> Result<Pos> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
    if v.len() != 3 {
        return Err(anyhow!("want x,y,z, got {s:?}"));
    }
    Ok([v[0], v[1], v[2]])
}

/// The functions not worth keeping (constants and wires).
fn trivial(f: &str) -> bool {
    matches!(f, "FALSE" | "TRUE" | "BUFFER" | "BUFFER a" | "BUFFER b")
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("gates".to_string()).chain(args)).map_err(|e| anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Check { designs, hold } => {
            for p in &designs {
                let c = harness::Circuit::load(p)?;
                let r = gate::analyze(&c, hold);
                println!("{}", r.line(&c.design.name));
            }
        }
        Cmd::Census { dims, inputs, palette, kmin, k, hold, threads, out_dir } => {
            let d: Vec<i32> = dims.split('x').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            let pal = census::palette(&palette)?;
            // best per key over every input configuration: (hit, its inputs)
            let mut best: std::collections::BTreeMap<String, (census::Hit, Vec<Pos>)> = Default::default();
            let t0 = std::time::Instant::now();
            for cfg in inputs.split('|').filter(|s| !s.trim().is_empty()) {
                let ins: Vec<Pos> = cfg.split(';').map(parse_pos).collect::<Result<_>>()?;
                let sp = Space { dims: [d[0], d[1], d[2]], inputs: ins.clone(), palette: pal.clone(), kmin, k, fixed: vec![] };
                eprintln!("inputs {cfg}:");
                for (key, h) in census_one(&sp, hold, threads)? {
                    if best.get(&key).is_none_or(|(b, _)| h.score < b.score) {
                        best.insert(key, (h, ins.clone()));
                    }
                }
            }
            println!("searched in {:.1} s", t0.elapsed().as_secs_f64());
            println!("| key | blocks | note | inputs | output |");
            println!("|---|---|---|---|---|");
            for (key, (h, ins)) in &best {
                println!("| {key} | {} | {} | {:?} | {:?} |", h.blocks.len(), h.note, ins, h.output);
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (key, (h, ins)) in &best {
                    let stem: String = key.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect::<String>();
                    let stem = stem.split('-').filter(|s| !s.is_empty()).collect::<Vec<_>>().join("-");
                    let name = format!("gate-{stem}-{}b", h.blocks.len());
                    let mut des = mctas::design::Design { name: name.clone(), ..Default::default() };
                    for (p, b) in &h.blocks {
                        des.blocks.insert(*p, b.to_string());
                    }
                    if ins.len() == 1 {
                        des.ports.insert("input".into(), ins[0]);
                    } else {
                        for (i, p) in ins.iter().enumerate() {
                            des.ports.insert(format!("input.{}", (b'a' + i as u8) as char), *p);
                        }
                    }
                    des.ports.insert("output".into(), h.output);
                    let fun = key.trim_end_matches(" fastest").split_whitespace().next().unwrap_or("").to_lowercase();
                    des.meta.insert("circuit".into(), format!("gate {fun}"));
                    let text = format!("{}{}", circuit_delay::comment(&format!("{key} gate (track gates census, {}): {}", h.note, describe(&h.blocks))), des.to_text());
                    std::fs::write(dir.join(format!("{name}.txt")), text)?;
                }
            }
        }
    }
    Ok(())
}

/// The census for one input configuration: the smallest design per function (then latency) and
/// the fastest per function (then blocks), minimal ones only (removing any block changes the
/// function at that output).
fn census_one(sp: &Space, hold: u32, threads: usize) -> Result<std::collections::BTreeMap<String, census::Hit>> {
    let ins = &sp.inputs;
    let inp: Vec<(redsim::BlockPos, InputKind)> = ins.iter().map(|p| (bp(*p), InputKind::Place { on: Block::RedstoneBlock, off: Block::Air })).collect();
    let bounds = sp.bounds();
    let outs: Vec<Pos> = sp.output_cells();
    let outs_bp: Vec<redsim::BlockPos> = outs.iter().map(|p| bp(*p)).collect();
    let n_in = ins.len();
    let (changes, frames, walk) = gate::stimulus(n_in, hold).ok_or_else(|| anyhow!("1 or 2 inputs"))?;
    // the gates (function, latency) of every output cell of a filling
    let judge = |blocks: &[(Pos, Block)], only: Option<usize>| -> Vec<(usize, gate::GateReport)> {
        let raw: Vec<(redsim::BlockPos, Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
        let sel: Vec<redsim::BlockPos> = match only {
            Some(i) => vec![outs_bp[i]],
            None => outs_bp.clone(),
        };
        let early = if only.is_none() { Some((gate::WARM - 1, gate::WARM + 6 * hold)) } else { None };
        let Ok((levels, _)) = harness::run_raw_multi_early(&raw, &inp, &sel, bounds, &changes, frames, 2, early) else { return vec![] };
        let mut v = Vec::new();
        for (k, lv) in levels.iter().enumerate() {
            let out: Vec<bool> = lv.iter().map(|l| *l > 0).collect();
            let r = gate::analyze_levels(n_in, &walk, hold, &out);
            if r.verdict == "GATE" && !trivial(&r.function) {
                v.push((only.unwrap_or(k), r));
            }
        }
        v
    };
    Ok(census::search(sp, threads, |blocks| {
        let mut v = Vec::new();
        for (oi, r) in judge(blocks, None) {
            let minimal = (0..blocks.len()).all(|i| {
                let less: Vec<(Pos, Block)> = blocks.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| *b).collect();
                !judge(&less, Some(oi)).iter().any(|(_, r2)| r2.function == r.function)
            });
            if !minimal {
                continue;
            }
            let b = blocks.len() as u32;
            let note = format!("latency {} gt, glitches {}", r.latency, r.glitches);
            v.push((r.function.clone(), (b, r.latency), note.clone(), outs[oi]));
            v.push((format!("{} fastest", r.function), (r.latency, b), note, outs[oi]));
        }
        v
    }))
}

/// A one-line list of the design's blocks.
fn describe(blocks: &[(Pos, Block)]) -> String {
    blocks.iter().map(|(p, b)| format!("{} at {},{},{}", b.to_string().trim_start_matches("minecraft:"), p[0], p[1], p[2])).collect::<Vec<_>>().join("; ")
}
