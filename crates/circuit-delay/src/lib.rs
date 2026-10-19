//! Track delay (crates/circuit-delay, owned by clk-rt): delay lines, output = input shifted by N
//! gt with both edges and pulse lengths preserved. `mctas circuit delay <command>`; the redsim
//! stimulus harness here is shared with track gates (crates/circuit-gates) until clockinfra's
//! judge (crates/circuit) is merged. Notes: designs/circuits/tracks/delay/NOTES.md.

pub mod build;
pub mod census;
pub mod delay;
pub mod families;
pub mod harness;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "delay", about = "Delay lines (track delay)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The delay checker on redsim: N, min-pulse, blocks (or ONE-EDGE with both shifts).
    Check {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4000)]
        max_n: u32,
    },
    /// Run a design on a stimulus and print the input and output waveforms.
    Sim {
        design: PathBuf,
        /// `a 0+ 4- 20+ 22-` or `pulse 4 every 40 x 6 [from 20]` (default: the design's
        /// `stimulus:` header, else pulse 10 every 30 x 3).
        #[arg(long)]
        stimulus: Option<String>,
        #[arg(long, default_value_t = 160)]
        frames: u32,
        /// First frame shown.
        #[arg(long, default_value_t = 0)]
        from: usize,
    },
    /// A repeater line (delays 1-4, e.g. 4,4,1): N = 2 x sum.
    Repeaters {
        #[arg(long)]
        delays: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A comparator line: N = 2k.
    Comparators {
        #[arg(long)]
        n: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// An observer line + copper bulb + comparator: N = 2k + 2.
    Observers {
        #[arg(long)]
        n: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A repeater + a torch tower of k torches: N = 2 + 2k.
    Torches {
        #[arg(long)]
        n: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Exhaustive search for delay lines: every filling of a box with kmin..=k palette blocks
    /// (plus --fixed blocks) around a fixed input cell (a redstone block placed by the judge); the
    /// smallest design per N (then the shortest min-pulse), every candidate output cell read.
    Census {
        #[arg(long, default_value = "3x2x3")]
        dims: String,
        /// The input cell `x,y,z`.
        #[arg(long)]
        input: String,
        #[arg(long, default_value = "stone,rblock,dust,rep,cmp")]
        palette: String,
        /// Blocks in every filling: `x,y,z=state;x,y,z=state`.
        #[arg(long, default_value = "")]
        fixed: String,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        #[arg(long, default_value_t = 3)]
        k: usize,
        /// Longest N looked for.
        #[arg(long, default_value_t = 30)]
        max_n: u32,
        #[arg(long, default_value_t = 32)]
        threads: usize,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("delay".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Check { designs, max_n } => {
            for p in &designs {
                let c = harness::Circuit::load(p)?;
                let r = delay::analyze(&c, max_n);
                println!("{}", r.line(&c.design.name));
            }
        }
        Cmd::Sim { design, stimulus, frames, from } => {
            let c = harness::Circuit::load(&design)?;
            let s = stimulus.or_else(|| c.design.meta.get("stimulus").cloned()).unwrap_or_else(|| "pulse 10 every 30 x 3".into());
            let ch = harness::parse_stimulus(&c, &s)?;
            let tr = harness::run(&c, &ch, frames, 4).map_err(|e| anyhow::anyhow!(e))?;
            for (k, i) in c.inputs.iter().enumerate() {
                println!("{:>12} {}", i.name, harness::wave(&tr.inputs[k], from));
            }
            println!("{:>12} {}", "output", harness::wave(&tr.out(), from));
            let lv: String = tr.level.iter().skip(from).map(|l| char::from_digit(*l as u32, 16).unwrap_or('?')).collect();
            println!("{:>12} {}", "level", lv);
        }
        Cmd::Repeaters { delays, out } => {
            let ds: Vec<u8> = delays.split(',').map(|s| s.trim().parse::<u8>()).collect::<std::result::Result<_, _>>()?;
            if ds.iter().any(|d| !(1..=4).contains(d)) {
                bail!("delays are 1-4");
            }
            let n: u32 = ds.iter().map(|d| 2 * *d as u32).sum();
            let name = format!("delay-rep-{}", ds.iter().map(|d| d.to_string()).collect::<Vec<_>>().join(""));
            emit(families::repeaters(&ds, &name), out, &format!("Repeater line (track delay): {} repeater(s) with delays {delays} redstone ticks in a row, each reading the one before: N = 2 x sum = {n} gt", ds.len()))?;
        }
        Cmd::Comparators { n, out } => {
            emit(families::comparators(n, &format!("delay-cmp-{n}")), out, &format!("Comparator line (track delay): {n} compare-mode comparator(s) in a row: N = 2 gt each = {} gt", 2 * n))?;
        }
        Cmd::Observers { n, out } => {
            emit(
                families::observers(n, &format!("delay-obs-{n}")),
                out,
                &format!("Observer line + copper bulb (track delay): {n} observer(s) carry each input edge as a 2 gt pulse, a waxed copper bulb toggles on every pulse (the input level again), a comparator reads it: N = 2 x {n} + 2 = {} gt", 2 * n + 2),
            )?;
        }
        Cmd::Torches { n, out } => {
            emit(families::torches(n, &format!("delay-torch-{n}")), out, &format!("Torch tower (track delay): a repeater powers a block, {n} torches climb from it (each powers the block above it): N = 2 + 2 x {n} gt"))?;
        }
        Cmd::Census { dims, input, palette, fixed, kmin, k, max_n, threads, out_dir } => {
            let d: Vec<i32> = dims.split('x').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            let ip = parse_pos(&input)?;
            let mut fx: Vec<(harness::Pos, redsim::Block)> = Vec::new();
            for part in fixed.split(';').filter(|s| !s.trim().is_empty()) {
                let (p, s) = part.split_once('=').ok_or_else(|| anyhow::anyhow!("--fixed x,y,z=state"))?;
                fx.push((parse_pos(p)?, redsim::Block::parse(s.trim()).map_err(|e| anyhow::anyhow!("{s}: {e}"))?));
            }
            let sp = census::Space { dims: [d[0], d[1], d[2]], inputs: vec![ip], palette: census::palette(&palette)?, kmin, k, fixed: fx.clone() };
            let best = delay_census(&sp, max_n, threads);
            println!("| key | blocks | note | output |");
            println!("|---|---|---|---|");
            for (key, h) in &best {
                println!("| {key} | {} | {} | {:?} |", h.blocks.len(), h.note, h.output);
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (key, h) in &best {
                    let n = key.trim_start_matches("N=");
                    let name = format!("delay-census-n{n}-{}b", h.blocks.len());
                    let mut des = mctas::design::Design { name: name.clone(), ..Default::default() };
                    for (p, b) in &h.blocks {
                        des.blocks.insert(*p, b.to_string());
                    }
                    des.ports.insert("input".into(), ip);
                    des.ports.insert("output".into(), h.output);
                    des.meta.insert("circuit".into(), "delay".into());
                    let desc = h.blocks.iter().map(|(p, b)| format!("{} at {},{},{}", b.to_string().trim_start_matches("minecraft:"), p[0], p[1], p[2])).collect::<Vec<_>>().join("; ");
                    let text = format!("{}{}", comment(&format!("Delay line N = {n} gt (track delay census, {}): {desc}", h.note)), des.to_text());
                    std::fs::write(dir.join(format!("{name}.txt")), text)?;
                }
            }
        }
    }
    Ok(())
}

fn parse_pos(s: &str) -> Result<harness::Pos> {
    let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
    if v.len() != 3 {
        bail!("want x,y,z, got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

/// The delay census over one space: per N >= 1 the smallest minimal design (then min-pulse).
pub fn delay_census(sp: &census::Space, max_n: u32, threads: usize) -> std::collections::BTreeMap<String, census::Hit> {
    use harness::{InputKind, bp};
    let inp = vec![(bp(sp.inputs[0]), InputKind::Place { on: redsim::Block::RedstoneBlock, off: redsim::Block::Air })];
    let bounds = sp.bounds();
    let outs: Vec<harness::Pos> = sp.output_cells();
    let outs_bp: Vec<redsim::BlockPos> = outs.iter().map(|p| bp(*p)).collect();
    let (changes, end) = delay::long_stimulus(40);
    let frames = end + max_n + 10;
    let nfixed = sp.fixed.len();
    // (output index, rise, fall) of every output cell whose waveform is a delayed input
    let judge = |blocks: &[(harness::Pos, redsim::Block)], only: Option<usize>| -> Vec<(usize, u32, u32)> {
        let raw: Vec<(redsim::BlockPos, redsim::Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
        let sel: Vec<redsim::BlockPos> = match only {
            Some(i) => vec![outs_bp[i]],
            None => outs_bp.clone(),
        };
        let early = Some((delay::WARM - 1, delay::WARM + max_n.min(39)));
        let Ok((levels, ins)) = harness::run_raw_multi_early(&raw, &inp, &sel, bounds, &changes, frames, 2, early) else { return vec![] };
        let mut v = Vec::new();
        for (k, lv) in levels.iter().enumerate() {
            let out: Vec<bool> = lv.iter().map(|l| *l > 0).collect();
            if let Some((r, f)) = delay::edge_shifts(&ins[0], &out)
                && r >= 1
            {
                v.push((only.unwrap_or(k), r, f));
            }
        }
        v
    };
    census::search(sp, threads, |blocks| {
        let mut v = Vec::new();
        for (oi, r, f) in judge(blocks, None) {
            if r != f {
                continue;
            }
            // minimal: removing any free block changes the delay at that output
            let minimal = (nfixed..blocks.len()).all(|i| {
                let less: Vec<(harness::Pos, redsim::Block)> = blocks.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| *b).collect();
                !judge(&less, Some(oi)).iter().any(|&(_, r2, f2)| r2 == r && f2 == f)
            });
            if !minimal {
                continue;
            }
            let raw: Vec<(redsim::BlockPos, redsim::Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
            let runner = |ch: &[harness::Change], fr: u32| -> Option<(Vec<bool>, Vec<bool>)> {
                let (lv, ins) = harness::run_raw_multi(&raw, &inp, &[outs_bp[oi]], bounds, ch, fr, 2).ok()?;
                Some((ins[0].clone(), lv[0].iter().map(|l| *l > 0).collect()))
            };
            let m = delay::min_pulse(r, 12, &runner).unwrap_or(99);
            let free = (blocks.len() - nfixed) as u32;
            v.push((format!("N={r}"), (blocks.len() as u32, m), format!("min-pulse {m} gt ({free} free blocks)"), outs[oi]));
        }
        v
    })
}

/// Write a generated design (with `circuit: delay` and a caption comment) or print it.
fn emit(b: build::B, out: Option<PathBuf>, caption: &str) -> Result<()> {
    if b.conflict {
        bail!("conflicting blocks");
    }
    let mut d = b.design();
    d.meta.insert("circuit".into(), "delay".into());
    let text = format!("{}{}", comment(caption), d.to_text());
    match out {
        Some(p) => {
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::fs::write(p, text)?
        }
        None => print!("{text}"),
    }
    Ok(())
}

/// A caption wrapped into `# ` comment lines of at most 100 columns.
pub fn comment(caption: &str) -> String {
    let mut out = String::new();
    let mut line = String::from("#");
    for w in caption.split_whitespace() {
        if line.len() + w.len() + 1 > 100 {
            out.push_str(&line);
            out.push('\n');
            line = String::from("#");
        }
        line.push(' ');
        line.push_str(w);
    }
    if line.len() > 1 {
        out.push_str(&line);
        out.push('\n');
    }
    out
}
