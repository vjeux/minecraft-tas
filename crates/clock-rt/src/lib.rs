//! Track clk-rt's clock commands: `mctas clock rt <command> ...` (torch / repeater / comparator
//! clocks: docs/clocks.md, the track's notes in designs/clocks/tracks/clk-rt/NOTES.md).
//!
//! * `uloop --head torch|corner --top 2,1* --bottom 3,d --tail dust|torch [--out F]`: one U-loop
//!   (families.rs).
//! * `sweep-uloop --kmax 2 [--heads torch,corner] [--tails dust,torch] [--alphabet ...]
//!   [--powered] [--out-dir DIR]`: every U-loop of row length <= kmax judged by `clock sim`; per
//!   period the fewest blocks and every distinct output waveform; the best of each period to DIR.
//! * `shrink <design> [--period P] [--pairs]`: local search for a smaller clock (shrink.rs).

pub mod build;
pub mod families;
pub mod kick;
pub mod shrink;
pub mod sweep;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use families::{Cell, Head, Tail, ULoop, parse_row};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas clock rt", about = "Track clk-rt: torch / repeater / comparator clocks")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Write one U-loop design.
    Uloop {
        /// torch | torch-off | corner
        #[arg(long, default_value = "torch")]
        head: String,
        /// Top row cells: 1-4 repeater delay (`*` powered at placement), c comparator, d dust,
        /// b block, T / t wall torch (lit / unlit) on the previous cell.
        #[arg(long)]
        top: String,
        #[arg(long)]
        bottom: String,
        /// dust | torch | torch-off
        #[arg(long, default_value = "dust")]
        tail: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Local search for a smaller clock of the same period (shrink.rs): single-cell changes and
    /// remove-two-add-one moves judged by clock sim; prints the best design per period it met.
    Shrink {
        seed: PathBuf,
        /// The period to shrink (default: the seed's).
        #[arg(long)]
        period: Option<u32>,
        #[arg(long, default_value_t = 30)]
        rounds: usize,
        /// Equal-size steps allowed in a row when nothing is smaller.
        #[arg(long, default_value_t = 4)]
        plateau: usize,
        /// Also try remove-two-add-one moves.
        #[arg(long)]
        pairs: bool,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Write the best design of every period met here (`shrink-p<P>.txt`).
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Only report periods up to this (the table).
        #[arg(long, default_value_t = 100000)]
        max_period: u32,
    },
    /// Kick search: a temporary redstone block next to a dust of the design, T gt long, starting
    /// at tick N (odd N = a second parity class): prints every period it makes.
    Kick {
        design: PathBuf,
        #[arg(long, default_value_t = 30)]
        max_at: u32,
        #[arg(long, default_value_t = 4)]
        max_len: u32,
        /// Only odd periods.
        #[arg(long)]
        odd: bool,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Write the first design of every period found here (`kick-p<P>.txt`).
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Odd periods: every U-loop up to a row length whose loop delay is 2 x an odd number, with ONE
    /// loaded pulse (one powered repeater or one unlit torch), kicked at an odd tick by every
    /// candidate temporary block; best design per odd period.
    SweepOdd {
        #[arg(long, default_value_t = 2)]
        kmax: usize,
        #[arg(long, default_value = "1,2,3,4,d")]
        alphabet: String,
        #[arg(long, default_value_t = 3)]
        max_len: u32,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Judge every U-loop up to a row length with clock sim; best per period + distinct waveforms.
    SweepUloop {
        #[arg(long, default_value_t = 2)]
        kmax: usize,
        #[arg(long, default_value = "torch,corner")]
        heads: String,
        #[arg(long, default_value = "dust,torch")]
        tails: String,
        #[arg(long, default_value = "1,2,3,4,c,d,b,T")]
        alphabet: String,
        /// Also every powered-at-placement variant of each repeater, and unlit torches (pulse
        /// loading, harmonics).
        #[arg(long)]
        powered: bool,
        #[arg(long, default_value_t = 20000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Write the best design of every period here.
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Print every distinct waveform, not only the best per period.
        #[arg(long)]
        waves: bool,
    },
}

fn parse_head(s: &str) -> Result<Head> {
    Ok(match s {
        "torch" | "T" => Head::Torch { lit: true },
        "torch-off" | "t" => Head::Torch { lit: false },
        "corner" | "C" => Head::Corner,
        _ => bail!("head: torch | torch-off | corner, got {s:?}"),
    })
}

fn parse_tail(s: &str) -> Result<Tail> {
    Ok(match s {
        "dust" | "d" => Tail::Dust,
        "torch" | "T" => Tail::Torch { lit: true },
        "torch-off" | "t" => Tail::Torch { lit: false },
        _ => bail!("tail: dust | torch | torch-off, got {s:?}"),
    })
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("rt".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Uloop { head, top, bottom, tail, name, out } => {
            let u = ULoop { head: parse_head(&head)?, top: parse_row(&top)?, bottom: parse_row(&bottom)?, tail: parse_tail(&tail)? };
            let name = name.unwrap_or_else(|| format!("clk-rt-uloop-{}", u.label()));
            let b = u.build(&name)?;
            if b.conflict {
                bail!("conflicting blocks");
            }
            let text = b.design().to_text();
            match out {
                Some(p) => std::fs::write(&p, &text)?,
                None => print!("{text}"),
            }
        }
        Cmd::Shrink { seed, period, rounds, plateau, pairs, horizon, threads, out_dir, max_period } => {
            let d = mctas::design::Design::load(&seed)?;
            let pal = shrink::palette_rt();
            let r = shrink::shrink(&d, period, &pal, rounds, plateau, pairs, horizon, threads, true);
            println!("| P | blocks | volume | output | family |");
            println!("|---|---|---|---|---|");
            for (p, f) in r.best.iter().filter(|(p, _)| **p <= max_period) {
                println!("| {p} | {} | {} | {} | {} |", f.blocks, f.volume, f.wave(), f.family);
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (p, f) in r.best.iter().filter(|(p, _)| **p <= max_period) {
                    let mut dd = mctas::design::Design::parse(&f.design)?;
                    dd.name = format!("clk-rt-shrink-p{p}-{}b", f.blocks);
                    std::fs::write(dir.join(format!("shrink-p{p}.txt")), dd.to_text())?;
                }
            }
        }
        Cmd::Kick { design, max_at, max_len, odd, horizon, threads, out_dir } => {
            let d = mctas::design::Design::load(&design)?;
            let cells = kick::kick_cells(&d);
            let lens: Vec<u32> = (1..=max_len).collect();
            let ats: Vec<u32> = (0..=max_at).collect();
            let found = kick::search(&d, &cells, &lens, &ats, horizon, threads);
            eprintln!("{} kick cells x {} lengths x {} ticks: {} clocks", cells.len(), lens.len(), ats.len(), found.len());
            let mut shown: std::collections::BTreeMap<(u32, String), ()> = Default::default();
            for (c, st, l, a, f) in &found {
                if odd && f.period % 2 == 0 {
                    continue;
                }
                if shown.insert((f.period, f.wave()), ()).is_some() {
                    continue;
                }
                println!("P={} {} | kick {} at {:?} for {l} gt at tick {a} | {} blocks | {}", f.period, f.wave(), clock::short(st), c, f.blocks, f.family);
                if let Some(dir) = &out_dir {
                    std::fs::create_dir_all(dir)?;
                    let path = dir.join(format!("kick-p{}.txt", f.period));
                    if !path.exists() {
                        let mut dd = mctas::design::Design::parse(&f.design)?;
                        dd.name = format!("{}-kick-p{}", d.name, f.period);
                        std::fs::write(path, dd.to_text())?;
                    }
                }
            }
        }
        Cmd::SweepOdd { kmax, alphabet, max_len, horizon, threads, out_dir } => {
            let alpha: Vec<Cell> = parse_row(&alphabet)?;
            let heads = [Head::Torch { lit: true }, Head::Corner];
            let tails = [Tail::Torch { lit: true }, Tail::Dust];
            let base = sweep::uloops(kmax, &alpha, &heads, &tails);
            // one loaded pulse: one repeater powered, or one torch unlit
            let mut loops: Vec<ULoop> = Vec::new();
            for u in &base {
                let rt = u.loop_rt();
                if rt % 2 == 0 {
                    continue; // loop delay 2 x rt gt: odd rt = 2 x odd gt
                }
                // the two-pulse ring needs an even number of torches (a non-inverting loop)
                if u.torches() % 2 == 1 {
                    continue;
                }
                let n = u.top.len() + u.bottom.len();
                for i in 0..n {
                    let mut v = u.clone();
                    let c = if i < u.top.len() { &mut v.top[i] } else { &mut v.bottom[i - u.top.len()] };
                    if let Cell::Rep { d, .. } = *c {
                        *c = Cell::Rep { d, on: true };
                        loops.push(v);
                    }
                }
                if let Head::Torch { .. } = u.head {
                    let mut v = u.clone();
                    v.head = Head::Torch { lit: false };
                    loops.push(v);
                }
                if let Tail::Torch { .. } = u.tail {
                    let mut v = u.clone();
                    v.tail = Tail::Torch { lit: false };
                    loops.push(v);
                }
            }
            eprintln!("{} loaded U-loops with an odd half loop delay", loops.len());
            let mut best: std::collections::BTreeMap<u32, (sweep::Found, String)> = Default::default();
            let t0 = std::time::Instant::now();
            for u in &loops {
                let name = format!("clk-rt-odd-{}", u.label());
                let b = u.build(&name)?;
                if b.conflict {
                    continue;
                }
                let d = b.design();
                let half = u.loop_rt(); // the odd period sought, in gt
                let cells = kick::kick_cells(&d);
                let lens: Vec<u32> = (1..=max_len).collect();
                let ats: Vec<u32> = (1..=(2 * half + 3)).filter(|a| a % 2 == 1).collect();
                for (c, st, l, a, f) in kick::search(&d, &cells, &lens, &ats, horizon, threads) {
                    if f.period % 2 == 0 {
                        continue;
                    }
                    let better = best.get(&f.period).is_none_or(|(o, _)| (f.blocks, f.volume) < (o.blocks, o.volume));
                    if better {
                        let how = format!("kick {} at {:?} for {l} gt at tick {a}", clock::short(&st), c);
                        best.insert(f.period, (f, how));
                    }
                }
            }
            eprintln!("searched in {:.1} s", t0.elapsed().as_secs_f64());
            println!("| P | blocks | volume | output | kick | design |");
            println!("|---|---|---|---|---|---|");
            for (p, (f, how)) in &best {
                println!("| {p} | {} | {} | {} | {how} | {} |", f.blocks, f.volume, f.wave(), f.label);
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (p, (f, _)) in &best {
                    let mut d = mctas::design::Design::parse(&f.design)?;
                    d.meta.insert("uloop".into(), f.label.trim_start_matches("clk-rt-odd-").to_string());
                    d.name = format!("clk-rt-odd-p{p}-{}b", f.blocks);
                    std::fs::write(dir.join(format!("odd-p{p}.txt")), d.to_text())?;
                }
            }
        }
        Cmd::SweepUloop { kmax, heads, tails, alphabet, powered, horizon, threads, out_dir, waves } => {
            let heads: Vec<Head> = heads.split(',').map(parse_head).collect::<Result<_>>()?;
            let tails: Vec<Tail> = tails.split(',').map(parse_tail).collect::<Result<_>>()?;
            let mut alpha: Vec<Cell> = parse_row(&alphabet)?;
            if powered {
                let extra: Vec<Cell> = alpha
                    .iter()
                    .filter_map(|c| match c {
                        Cell::Rep { d, .. } => Some(Cell::Rep { d: *d, on: true }),
                        Cell::Torch { .. } => Some(Cell::Torch { lit: false }),
                        _ => None,
                    })
                    .collect();
                alpha.extend(extra);
                alpha.sort();
                alpha.dedup();
            }
            let mut heads = heads;
            let mut tails = tails;
            if powered {
                if heads.contains(&Head::Torch { lit: true }) && !heads.contains(&Head::Torch { lit: false }) {
                    heads.push(Head::Torch { lit: false });
                }
                if tails.contains(&Tail::Torch { lit: true }) && !tails.contains(&Tail::Torch { lit: false }) {
                    tails.push(Tail::Torch { lit: false });
                }
            }
            let loops = sweep::uloops(kmax, &alpha, &heads, &tails);
            let cands: Vec<(String, String)> = loops
                .iter()
                .filter_map(|u| {
                    let name = format!("clk-rt-uloop-{}", u.label());
                    let b = u.build(&name).ok()?;
                    if b.conflict { None } else { Some((name, b.design().to_text())) }
                })
                .collect();
            let n = cands.len();
            let t0 = std::time::Instant::now();
            let found = sweep::run_all(cands, horizon, threads);
            eprintln!("{n} U-loops judged in {:.1} s: {} clocks", t0.elapsed().as_secs_f64(), found.len());
            let t = sweep::table(&found);
            println!("| P | blocks | volume | output | family | design |");
            println!("|---|---|---|---|---|---|");
            for (p, f) in &t.best {
                println!("| {p} | {} | {} | {} | {} | {} |", f.blocks, f.volume, f.wave(), f.family, f.label);
            }
            if waves {
                println!();
                println!("| P | waveform + family | blocks | design |");
                println!("|---|---|---|---|");
                for ((p, w), f) in &t.waves {
                    println!("| {p} | {w} | {} | {} |", f.blocks, f.label);
                }
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (p, f) in &t.best {
                    let mut d = mctas::design::Design::parse(&f.design)?;
                    d.name = format!("clk-rt-uloop-p{p}-{}b", f.blocks);
                    d.meta.insert("uloop".into(), f.label.trim_start_matches("clk-rt-uloop-").to_string());
                    std::fs::write(dir.join(format!("uloop-p{p}.txt")), d.to_text())?;
                }
            }
        }
    }
    Ok(())
}
