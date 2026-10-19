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
pub mod divider;
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
    /// Burnout clocks: every loading of every U-loop with a fast loop (torches toggling more than
    /// 7 times in 60 gt), self-starting and with every temporary-block kick up to a tick; every
    /// period found (the smallest design each).
    SweepBurnout {
        #[arg(long, default_value_t = 2)]
        kmax: usize,
        #[arg(long, default_value = "1,c,d")]
        alphabet: String,
        /// Longest loop delay (redstone ticks) kept: 4 = 8 gt.
        #[arg(long, default_value_t = 4)]
        max_rt: u32,
        /// Kick ticks 0..=max_at (none = self-starting only).
        #[arg(long)]
        max_at: Option<u32>,
        #[arg(long, default_value_t = 2)]
        max_len: u32,
        #[arg(long, default_value_t = 20000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Best known design per period over design files / directories (clock sim on each): the
    /// fewest blocks, then the smallest swept volume; with --verify-list, only designs a
    /// `clock verify` output file reports `robust`.
    Table {
        paths: Vec<PathBuf>,
        #[arg(long, default_value_t = 100000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Only periods up to this.
        #[arg(long, default_value_t = 100000)]
        max_period: u32,
        /// Also list every (period, family) pair's smallest design.
        #[arg(long)]
        families: bool,
    },
    /// A divider chain: a base torch-repeater U-loop (period 4 (1 + d1 + d2)) and master-slave
    /// T flip-flop stages, each doubling the period (divider.rs).
    Divider {
        #[arg(long, default_value_t = 2)]
        d1: u8,
        #[arg(long, default_value_t = 2)]
        d2: u8,
        /// Base = a comparator fading ring with this many comparators per row (instead of the
        /// torch-repeater U-loop).
        #[arg(long)]
        fade: Option<usize>,
        #[arg(long, default_value_t = 1)]
        stages: usize,
        #[arg(long)]
        out: Option<PathBuf>,
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
        /// Also write the smallest design of every (period, family) pair.
        #[arg(long)]
        per_family: bool,
    },
}

fn parse_head(s: &str) -> Result<Head> {
    Ok(match s {
        "torch" | "T" => Head::Torch { lit: true },
        "torch-off" | "t" => Head::Torch { lit: false },
        "corner" | "C" => Head::Corner,
        "dust2" | "D" => Head::Dust2,
        _ => bail!("head: torch | torch-off | corner | dust2, got {s:?}"),
    })
}

fn parse_tail(s: &str) -> Result<Tail> {
    Ok(match s {
        "dust" | "d" => Tail::Dust,
        "torch" | "T" => Tail::Torch { lit: true },
        "torch-off" | "t" => Tail::Torch { lit: false },
        "fade" | "F" => Tail::Fade { lit: true },
        "fade-off" | "f" => Tail::Fade { lit: false },
        _ => bail!("tail: dust | torch | torch-off | fade | fade-off, got {s:?}"),
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
        Cmd::Table { paths, horizon, threads, max_period, families } => {
            let mut files: Vec<PathBuf> = Vec::new();
            for p in &paths {
                if p.is_dir() {
                    let mut v: Vec<PathBuf> = std::fs::read_dir(p)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|q| q.extension().is_some_and(|x| x == "txt")).collect();
                    v.sort();
                    files.extend(v);
                } else {
                    files.push(p.clone());
                }
            }
            let cands: Vec<(String, String)> = files.iter().filter_map(|f| Some((f.display().to_string(), std::fs::read_to_string(f).ok()?))).collect();
            let found = sweep::run_all(cands, horizon, threads);
            let t = sweep::table(&found);
            println!("| P | blocks | volume | output | family | design |");
            println!("|---|---|---|---|---|---|");
            for (p, f) in t.best.iter().filter(|(p, _)| **p <= max_period) {
                println!("| {p} | {} | {} | {} | {} | {} |", f.blocks, f.volume, f.wave(), f.family, f.label);
            }
            if families {
                let mut fam: std::collections::BTreeMap<(u32, String), &sweep::Found> = Default::default();
                for f in &found {
                    let k = (f.period, f.family.clone());
                    if fam.get(&k).is_none_or(|o| (f.blocks, f.volume) < (o.blocks, o.volume)) {
                        fam.insert(k, f);
                    }
                }
                println!();
                println!("| P | family | blocks | design |");
                println!("|---|---|---|---|");
                for ((p, family), f) in fam.iter().filter(|((p, _), _)| *p <= max_period) {
                    println!("| {p} | {family} | {} | {} |", f.blocks, f.label);
                }
            }
        }
        Cmd::SweepBurnout { kmax, alphabet, max_rt, max_at, max_len, horizon, threads, out_dir } => {
            let alpha: Vec<Cell> = parse_row(&alphabet)?;
            let heads = [Head::Torch { lit: true }, Head::Corner];
            let tails = [Tail::Torch { lit: true }, Tail::Dust];
            let base = sweep::uloops(kmax, &alpha, &heads, &tails);
            let mut cands: Vec<(String, String)> = Vec::new();
            let mut caps: std::collections::HashMap<String, String> = Default::default();
            for u in &base {
                if u.loop_rt() > max_rt || u.torches() == 0 {
                    continue;
                }
                for v in loadings(u) {
                    let name = format!("clk-rt-burn-{}", v.label());
                    let Ok(b) = v.build(&name) else { continue };
                    if b.conflict {
                        continue;
                    }
                    let d = b.design();
                    caps.insert(name.clone(), v.caption());
                    cands.push((name.clone(), d.to_text()));
                    if let Some(ma) = max_at {
                        for (c, st) in kick::kick_cells(&d) {
                            for l in 1..=max_len {
                                for a in 0..=ma {
                                    let kn = format!("{name}+k{}{}{}-{l}-{a}", c[0], c[1], c[2]);
                                    caps.insert(kn.clone(), format!("{}; kicked by a {} at {:?} for {l} gt at tick {a}", v.caption(), clock::short(&st), c));
                                    cands.push((kn, kick::with_kick(&d, c, &st, l, a).to_text()));
                                }
                            }
                        }
                    }
                }
            }
            eprintln!("{} candidates", cands.len());
            let t0 = std::time::Instant::now();
            let found = sweep::run_all(cands, horizon, threads);
            eprintln!("judged in {:.1} s: {} clocks", t0.elapsed().as_secs_f64(), found.len());
            let t = sweep::table(&found);
            println!("| P | blocks | volume | output | family | design |");
            println!("|---|---|---|---|---|---|");
            for (p, f) in &t.best {
                println!("| {p} | {} | {} | {} | {} | {} |", f.blocks, f.volume, f.wave(), f.family, f.label);
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (p, f) in &t.best {
                    let cap = caps.get(&f.label).cloned().unwrap_or_default();
                    write_design(&dir.join(format!("burn-p{p}.txt")), &f.design, &format!("clk-rt-burn-p{p}-{}b", f.blocks), "", &format!("BURNOUT clock: {cap}"))?;
                }
            }
        }
        Cmd::SweepOdd { kmax, alphabet, max_len, horizon, threads, out_dir } => {
            let alpha: Vec<Cell> = parse_row(&alphabet)?;
            let heads = [Head::Torch { lit: true }, Head::Corner];
            let tails = [Tail::Torch { lit: true }, Tail::Dust];
            let base = sweep::uloops(kmax, &alpha, &heads, &tails);
            // Every loading (powered repeaters, lit torches) of every base loop with an odd half
            // loop delay and an even number of torches (a non-inverting ring can hold two
            // pulses); keep the loadings that run ONE pulse around the loop (period = loop delay,
            // one edge), the narrowest few per base.
            let mut loops: Vec<ULoop> = Vec::new();
            let mut pre: Vec<(String, String)> = Vec::new();
            let mut by_name: std::collections::HashMap<String, ULoop> = Default::default();
            for u in &base {
                let rt = u.loop_rt();
                if rt % 2 == 0 || u.torches() % 2 == 1 {
                    continue;
                }
                let mut slots: Vec<usize> = Vec::new(); // indices of repeaters in top ++ bottom
                for (i, c) in u.top.iter().chain(&u.bottom).enumerate() {
                    if matches!(c, Cell::Rep { .. }) {
                        slots.push(i);
                    }
                }
                let th = matches!(u.head, Head::Torch { .. });
                let tt = matches!(u.tail, Tail::Torch { .. });
                let nbits = slots.len() + th as usize + tt as usize;
                for mask in 0u32..(1 << nbits) {
                    let mut v = u.clone();
                    let k = u.top.len();
                    for (b, &i) in slots.iter().enumerate() {
                        let on = mask >> b & 1 == 1;
                        let c = if i < k { &mut v.top[i] } else { &mut v.bottom[i - k] };
                        if let Cell::Rep { d, .. } = *c {
                            *c = Cell::Rep { d, on };
                        }
                    }
                    let mut b = slots.len();
                    if th {
                        v.head = Head::Torch { lit: mask >> b & 1 == 1 };
                        b += 1;
                    }
                    if tt {
                        v.tail = Tail::Torch { lit: mask >> b & 1 == 1 };
                    }
                    let name = format!("clk-rt-odd-{}", v.label());
                    if let Ok(bb) = v.build(&name)
                        && !bb.conflict
                    {
                        pre.push((name.clone(), bb.design().to_text()));
                        by_name.insert(name, v);
                    }
                }
            }
            eprintln!("{} loadings of {} base loops", pre.len(), base.len());
            let found = sweep::run_all(pre, horizon, threads);
            // per base loop (label without loading marks): the narrowest one-pulse loadings
            let mut per_base: std::collections::BTreeMap<String, Vec<(u32, String)>> = Default::default();
            for f in &found {
                let Some(u) = by_name.get(&f.label) else { continue };
                if f.period != 2 * u.loop_rt() || f.edges != 1 {
                    continue;
                }
                let key = u.label().replace('*', "").replace('t', "T");
                let pulse = f.on.min(f.period - f.on);
                per_base.entry(key).or_default().push((pulse, f.label.clone()));
            }
            for (_, mut v) in per_base {
                v.sort();
                v.dedup_by(|a, b| a.0 == b.0);
                for (_, name) in v.into_iter().take(3) {
                    loops.push(by_name[&name].clone());
                }
            }
            eprintln!("{} one-pulse loadings to kick", loops.len());
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
                for (p, (f, how)) in &best {
                    let label = f.label.trim_start_matches("clk-rt-odd-").to_string();
                    let cap = loops.iter().find(|u| u.label() == label).map(|u| u.caption()).unwrap_or_default();
                    let cap = format!("{cap}; ODD period: a second pulse started by a {how} (a second parity class: every redstone-tick delay is even)");
                    write_design(&dir.join(format!("odd-p{p}.txt")), &f.design, &format!("clk-rt-odd-p{p}-{}b", f.blocks), &label, &cap)?;
                }
            }
        }
        Cmd::Divider { d1, d2, fade, stages, out } => {
            if let Some(c) = fade {
                let name = format!("clk-rt-div-fade{c}-x{stages}");
                let Some(b) = divider::chain_fade(c, stages, &name) else { bail!("stage {stages} collides with the chain") };
                let cap = format!(
                    "Divider chain (clk-rt): a base comparator FADING ring ({c} + {c} compare comparators keep a strength that drops one level per lap; a wall torch refills the empty ring) and {stages} master-slave T flip-flop stage(s) (two latch repeaters locked alternately by repeaters reading the previous torch and its block; the slave's dust bends into a target block holding a torch whose output runs back to the master): period = the ring's x 2^{stages}"
                );
                let text = b.design().to_text();
                match out {
                    Some(p) => write_design(&p, &text, &name, "", &cap)?,
                    None => print!("{text}"),
                }
                return Ok(());
            }
            let name = format!("clk-rt-div-{d1}{d2}-x{stages}");
            let Some(b) = divider::chain(d1, d2, stages, &name) else { bail!("stage {stages} collides with the chain") };
            let base = 4 * (1 + d1 as u32 + d2 as u32);
            let cap = format!(
                "Divider chain (clk-rt): a base torch + repeater U-loop (delays {d1}/{d2}: period {base} gt) and {stages} master-slave T flip-flop stage(s); each stage = two latch repeaters (master, slave) locked alternately by repeaters reading the previous stage's torch (not S) and block (S), the slave's block holds a torch whose output runs back (dust) to the master: one toggle per period of the previous stage, period {base} x 2^{stages} = {} gt",
                base << stages
            );
            let text = b.design().to_text();
            match out {
                Some(p) => write_design(&p, &text, &name, "", &cap)?,
                None => print!("{text}"),
            }
        }
        Cmd::SweepUloop { kmax, heads, tails, alphabet, powered, horizon, threads, out_dir, waves, per_family } => {
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
                let cap_of = |label: &str| -> String {
                    let l = label.trim_start_matches("clk-rt-uloop-");
                    loops.iter().find(|u| u.label() == l).map(|u| u.caption()).unwrap_or_default()
                };
                for (p, f) in &t.best {
                    let l = f.label.trim_start_matches("clk-rt-uloop-");
                    write_design(&dir.join(format!("uloop-p{p}.txt")), &f.design, &format!("clk-rt-uloop-p{p}-{}b", f.blocks), l, &cap_of(&f.label))?;
                }
                if per_family {
                    // the smallest design of every (period, family) pair
                    let mut fam: std::collections::BTreeMap<(u32, String), &sweep::Found> = Default::default();
                    for f in &found {
                        let k = (f.period, f.family.clone());
                        if fam.get(&k).is_none_or(|o| (f.blocks, f.volume) < (o.blocks, o.volume)) {
                            fam.insert(k, f);
                        }
                    }
                    for ((p, family), f) in fam {
                        let short: String = family.split('+').map(|w| w.split('_').map(|s| &s[..1]).collect::<String>()).collect::<Vec<_>>().join("-");
                        let l = f.label.trim_start_matches("clk-rt-uloop-");
                        write_design(&dir.join(format!("uloop-p{p}-{short}.txt")), &f.design, &format!("clk-rt-uloop-p{p}-{short}-{}b", f.blocks), l, &cap_of(&f.label))?;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Every loading of a U-loop: each repeater powered or not, each head / tail torch lit or not.
fn loadings(u: &ULoop) -> Vec<ULoop> {
    let mut slots: Vec<usize> = Vec::new();
    for (i, c) in u.top.iter().chain(&u.bottom).enumerate() {
        if matches!(c, Cell::Rep { .. } | Cell::Torch { .. }) {
            slots.push(i);
        }
    }
    let th = matches!(u.head, Head::Torch { .. });
    let tt = matches!(u.tail, Tail::Torch { .. });
    let nbits = slots.len() + th as usize + tt as usize;
    let k = u.top.len();
    let mut out = Vec::new();
    for mask in 0u32..(1 << nbits) {
        let mut v = u.clone();
        for (b, &i) in slots.iter().enumerate() {
            let on = mask >> b & 1 == 1;
            let c = if i < k { &mut v.top[i] } else { &mut v.bottom[i - k] };
            *c = match *c {
                Cell::Rep { d, .. } => Cell::Rep { d, on },
                Cell::Torch { .. } => Cell::Torch { lit: on },
                x => x,
            };
        }
        let mut b = slots.len();
        if th {
            v.head = Head::Torch { lit: mask >> b & 1 == 1 };
            b += 1;
        }
        if tt {
            v.tail = Tail::Torch { lit: mask >> b & 1 == 1 };
        }
        out.push(v);
    }
    out
}

/// Write a generated design with its name, its `uloop:` spec and a leading comment (the catalog's
/// "how it works").
fn write_design(path: &std::path::Path, text: &str, name: &str, spec: &str, caption: &str) -> Result<()> {
    let mut d = mctas::design::Design::parse(text)?;
    d.name = name.to_string();
    if !spec.is_empty() {
        d.meta.insert("uloop".into(), spec.to_string());
    }
    let mut out = String::new();
    // wrap the caption at ~100 columns
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
    out.push_str(&d.to_text());
    std::fs::write(path, out)?;
    Ok(())
}
