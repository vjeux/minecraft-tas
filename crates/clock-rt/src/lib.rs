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
pub mod counter;
pub mod kick;
pub mod odd;
pub mod ring;
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
    /// TEMPORARY (until clockinfra's `mctas circuit delay` dispatch is merged): track delay.
    Delay {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// TEMPORARY (until clockinfra's `mctas circuit gates` dispatch is merged): track gates.
    Gates {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
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
    /// A comparator counter over an arbitrary inner clock design (counter.rs `over`): the inner
    /// design's --tap cell (a block or torch carrying its pulse, on its edge) feeds the shaper.
    CounterOver {
        #[arg(long)]
        inner: PathBuf,
        /// x,y,z of the tap cell in the inner design's coordinates.
        #[arg(long)]
        tap: String,
        #[arg(long, default_value_t = 2)]
        k: usize,
        /// Levels per decrement: count = ceil(14 / dec) (1 -> 14, 2 -> 7, 3 -> 5).
        #[arg(long, default_value_t = 1)]
        dec: usize,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// A comparator COUNTER clock (counter.rs): an inner fading ring decrements a lossless
    /// comparator ring by one level per refill pulse; the counter's refill torch is the output.
    Counter {
        #[arg(long, default_value_t = 1)]
        k_in: usize,
        #[arg(long, default_value_t = 1)]
        k_out: usize,
        /// Nested counters: sizes innermost first (overrides --k-out), e.g. 2,2.
        #[arg(long)]
        ks: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// The n-th member of an extendable family (EXTEND.md): uloop2t (P 20 + 16n), uloop1t
    /// (36 + 32n), rect-pulse (32 + 16n), rect-inv (52 + 32n), odd-rect (23 + 8n), fade
    /// (114 + 56n), div (36 x 2^n), div-fade (114 x 2^n), counter (114 x 14^n, n >= 1).
    Family {
        name: String,
        #[arg(long)]
        n: u32,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Also write the family's `extend` headers (the catalog card's base design).
        #[arg(long)]
        card: bool,
    },
    /// Remove from the archive every clk-rt clock that is exactly an instance of one of the
    /// extendable families (same type key and period as the generator's design for some n in the
    /// family's range): the family card covers it. Dry run unless --apply.
    PruneMembers {
        /// Family names (default: all).
        names: Vec<String>,
        #[arg(long, default_value = "designs/clocks/archive")]
        dir: PathBuf,
        #[arg(long)]
        apply: bool,
    },
    /// Even periods from..=to on RECTANGLE rings (ring.rs): a 0-torch pulse ring (one loaded
    /// pulse, repeater delays summing to P / 2) and, for P = 0 mod 4, a 1-torch inverter ring
    /// (torch on a corner block, a dust corner after it, delays summing to P / 4 - 1); the
    /// smallest that clock sim confirms is written.
    EvenRect {
        #[arg(long, default_value_t = 4)]
        from: u32,
        #[arg(long, default_value_t = 100)]
        to: u32,
        /// best | pulse (the 0-torch pulse ring only) | inv (the 1-torch inverter only)
        #[arg(long, default_value = "best")]
        kind: String,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Odd periods from..=to on RECTANGLE rings (ring.rs): 3 x h perimeter, corner blocks, the
    /// straight cells repeaters with delays summing to P (an odd count leaves one relay block or
    /// dust), one pulse loaded at placement, the second started by the best odd-tick kick.
    OddRect {
        #[arg(long, default_value_t = 5)]
        from: u32,
        #[arg(long, default_value_t = 99)]
        to: u32,
        #[arg(long, default_value_t = 2)]
        max_len: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Odd periods from..=to: for each odd P a corner-head U-loop pulse ring of repeaters with
    /// delays summing to P (two pulses P apart in a ring of delay 2P), one pulse loaded at
    /// placement, the second started by the best odd-tick kick; the smallest kick per P is written.
    OddRange {
        #[arg(long, default_value_t = 25)]
        from: u32,
        #[arg(long, default_value_t = 99)]
        to: u32,
        #[arg(long, default_value_t = 4)]
        max_len: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Odd periods on rectangle rings (ring.rs): every assignment with an odd half loop delay and
    /// an even torch count, every loading; one-pulse loadings kicked at odd ticks (odd.rs).
    SweepOddRing {
        #[arg(long, default_value_t = 3)]
        w: i32,
        #[arg(long, default_value_t = 4)]
        h: i32,
        #[arg(long, default_value = "1,2,b,d")]
        straight: String,
        #[arg(long, default_value = "b,d")]
        corner: String,
        #[arg(long, default_value_t = 2)]
        max_len: u32,
        #[arg(long, default_value_t = 2)]
        per_base: usize,
        /// Only loadings with exactly one powered repeater (much fewer than every loading).
        #[arg(long)]
        single_load: bool,
        #[arg(long, default_value_t = 4000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// One rectangle ring design (ring.rs): --cells = the perimeter clockwise from the north-west
    /// corner (b, d, c, 1-4[*], T/t).
    Ring {
        #[arg(long, default_value_t = 3)]
        w: i32,
        #[arg(long, default_value_t = 3)]
        h: i32,
        #[arg(long)]
        cells: String,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Rectangle rings (ring.rs): every assignment of a w x h perimeter (straight cells from
    /// --straight, corners from --corner, at most --max-powered loaded repeaters) judged by clock
    /// sim; best per period, every waveform, best per (period, family).
    SweepRing {
        #[arg(long, default_value_t = 3)]
        w: i32,
        #[arg(long, default_value_t = 3)]
        h: i32,
        #[arg(long, default_value = "1,2,3,4,1*,2*,3*,4*,b,d,c,T")]
        straight: String,
        #[arg(long, default_value = "b,d,T")]
        corner: String,
        #[arg(long, default_value_t = 2)]
        max_powered: usize,
        /// At most this many delay cells (repeaters, comparators, torches): fast loops (burnout).
        #[arg(long)]
        max_delay_cells: Option<usize>,
        #[arg(long, default_value_t = 20000)]
        horizon: u32,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        #[arg(long)]
        out_dir: Option<PathBuf>,
        #[arg(long)]
        waves: bool,
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
        Cmd::Delay { args } => circuit_delay::run(args)?,
        Cmd::Gates { args } => circuit_gates::run(args)?,
        Cmd::Uloop { head, top, bottom, tail, name, out } => {
            let u = ULoop { head: parse_head(&head)?, top: parse_row(&top)?, bottom: parse_row(&bottom)?, tail: parse_tail(&tail)? };
            let name = name.unwrap_or_else(|| format!("clk-rt-{}-{}", u.kind(), u.label()));
            let b = u.build(&name)?;
            if b.conflict {
                bail!("conflicting blocks");
            }
            let text = b.design().to_text();
            match out {
                Some(p) => write_design(&p, &text, &name, &u.label(), &u.caption())?,
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
                        // caption: the source design's leading comment + how the kick works
                        let src = std::fs::read_to_string(&design).unwrap_or_default();
                        let lead: Vec<&str> = src.lines().take_while(|l| l.starts_with('#')).map(|l| l.trim_start_matches('#').trim()).collect();
                        let parity = if f.period % 2 == 1 { "ODD period: " } else { "" };
                        let cap = format!(
                            "{}{}; kicked by a {} at {:?} for {l} gt at tick {a}{}",
                            parity,
                            lead.join(" "),
                            clock::short(st),
                            c,
                            if a % 2 == 1 { " (an odd tick: a second parity class; every redstone-tick delay is even)" } else { "" }
                        );
                        let name = format!("{}-kick-p{}", d.name, f.period);
                        write_design(&path, &f.design, &name, "", &cap)?;
                    }
                }
            }
        }
        Cmd::CounterOver { inner, tap, k, dec, name, out } => {
            let d = mctas::design::Design::load(&inner)?;
            let t: Vec<i32> = tap.split(',').map(|s| s.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
            anyhow::ensure!(t.len() == 3, "--tap x,y,z");
            let count = 14usize.div_ceil(dec);
            let name = name.unwrap_or_else(|| format!("clk-rt-count{count}-k{k}-over-{}", d.name.trim_start_matches("clk-rt-")));
            let Some(m) = counter::over(&d, [t[0], t[1], t[2]], k, dec, &name) else { bail!("no turn of the inner design fits beyond the shaper") };
            let src = std::fs::read_to_string(&inner).unwrap_or_default();
            let lead: Vec<&str> = src.lines().take_while(|l| l.starts_with('#')).map(|l| l.trim_start_matches('#').trim()).collect();
            let cap = format!(
                "Comparator COUNTER over another clock (clk-rt): a lossless comparator ring ({k} + {k} comparators, one in subtract mode) loses {dec} level(s) per pulse of the inner clock (its pulse cut to strength {dec} by a subtract comparator with side {}) and its torch refills it to 14 at 0: period = {count} inner periods. Inner clock: {}",
                15 - dec,
                lead.join(" ")
            );
            match out {
                Some(p) => write_design(&p, &m.to_text(), &name, "", &cap)?,
                None => print!("{}", m.to_text()),
            }
        }
        Cmd::Counter { k_in, k_out, ks, out } => {
            let ks: Vec<usize> = match ks {
                Some(s) => s.split(',').map(|t| t.trim().parse::<usize>()).collect::<std::result::Result<_, _>>()?,
                None => vec![k_out],
            };
            let tag = ks.iter().map(|k| k.to_string()).collect::<Vec<_>>().join("-");
            let name = if ks.len() == 1 { format!("clk-rt-count-i{k_in}-o{}", ks[0]) } else { format!("clk-rt-count-i{k_in}-o{tag}") };
            let b = counter::build_nested(k_in, &ks, &name);
            if b.conflict {
                bail!("conflicting blocks");
            }
            let cap = if ks.len() == 1 {
                format!(
                    "Comparator COUNTER clock (clk-rt): an inner comparator fading ring ({k_in} + {k_in} compare comparators, one level lost per lap, a wall torch refills it) whose refill pulse, turned into strength 1 by a subtract comparator (side = 14 from a redstone block + 2 dust), decrements a LOSSLESS comparator ring ({k} + {k} comparators, one in subtract mode) by exactly one level; when the counter is empty its own torch refills it: period = about 14 inner periods",
                    k = ks[0]
                )
            } else {
                format!(
                    "NESTED comparator COUNTER clock (clk-rt): an inner comparator fading ring ({k_in} + {k_in} comparators) and {} lossless counter rings (sizes {tag}, innermost first), each decremented by one level per refill pulse of the level inside it (the pulse cut to strength 1 by a subtract comparator with side 14) and refilled by its own torch at 0: period = about 14^{} inner periods",
                    ks.len(),
                    ks.len()
                )
            };
            let text = b.design().to_text();
            match out {
                Some(p) => write_design(&p, &text, &name, "", &cap)?,
                None => print!("{text}"),
            }
        }
        Cmd::Family { name, n, out, card } => {
            let (dname, text, cap) = family_member(&name, n)?;
            let text = if card {
                let c = family_card(&name)?;
                let mut d = mctas::design::Design::parse(&text)?;
                d.meta.insert("extend family".into(), c.id.clone());
                d.meta.insert("extend period".into(), c.period.to_string());
                d.meta.insert("extend blocks".into(), c.blocks.to_string());
                d.meta.insert("extend rule".into(), c.rule.to_string());
                d.meta.insert("extend n".into(), format!("{}..{}", c.lo, c.hi));
                d.meta.insert("extend base".into(), n.to_string());
                d.meta.insert("extend generator".into(), format!("mctas clock rt family {name} --n {{n}} --out {{out}}"));
                d.to_text()
            } else {
                text
            };
            let dname = if card { family_card(&name)?.id } else { dname };
            match out {
                Some(p) => write_design(&p, &text, &dname, "", &cap)?,
                None => print!("{text}"),
            }
        }
        Cmd::PruneMembers { names, dir, apply } => {
            let names: Vec<String> = if names.is_empty() { FAMILIES.iter().map(|s| s.to_string()).collect() } else { names };
            let vpath = dir.join("verdicts-v1.tsv");
            let verdicts = std::fs::read_to_string(&vpath)?;
            // (file, period, type key) of every archived clk-rt clock: the key recomputed from the
            // archived design like the catalog does (stored keys may predate canon changes)
            let rows: Vec<(String, u32, String)> = verdicts
                .lines()
                .skip(1)
                .filter_map(|l| {
                    let f: Vec<&str> = l.split('\t').collect();
                    if f.len() <= 13 || !f[1].starts_with("clk-rt-") {
                        return None;
                    }
                    let key = match mctas::design::Design::load(&dir.join(f[1])) {
                        // a family card's base design is the card, not an instance to remove
                        Ok(d) if d.meta.contains_key("extend family") => return None,
                        Ok(d) => clock::canon::design_type_key(&d),
                        Err(_) => f[13].to_string(),
                    };
                    Some((f[1].to_string(), f[2].parse().unwrap_or(0), key))
                })
                .collect();
            let mut remove: std::collections::BTreeSet<String> = Default::default();
            for name in &names {
                let c = family_card(name)?;
                let pf = clock::extend::Formula::parse(c.period)?;
                for n in c.lo..=c.hi {
                    let (_, text, _) = family_member(name, n)?;
                    let d = mctas::design::Design::parse(&text)?;
                    let key = clock::canon::design_type_key(&d);
                    let p = pf.eval(n as i64) as u32;
                    for (file, period, k) in &rows {
                        if *period == p && *k == key {
                            println!("{}: n = {n} (P{p}) = {file}", c.id);
                            remove.insert(file.clone());
                        }
                    }
                }
            }
            println!("{} archived clk-rt clock(s) are family instances", remove.len());
            if apply {
                let keep_line = |l: &str, col: usize| -> bool { l.split('\t').nth(col).is_none_or(|f| !remove.contains(f)) };
                let v: String = verdicts.lines().filter(|l| keep_line(l, 1)).map(|l| format!("{l}\n")).collect();
                std::fs::write(&vpath, v)?;
                let cpath = dir.join("captions.tsv");
                let caps = std::fs::read_to_string(&cpath)?;
                let c: String = caps.lines().filter(|l| keep_line(l, 0)).map(|l| format!("{l}\n")).collect();
                std::fs::write(&cpath, c)?;
                let apath = dir.join("aliases.tsv");
                if let Ok(al) = std::fs::read_to_string(&apath) {
                    let a: String = al.lines().filter(|l| keep_line(l, 0)).map(|l| format!("{l}\n")).collect();
                    std::fs::write(&apath, a)?;
                }
                for f in &remove {
                    let _ = std::fs::remove_file(dir.join(f));
                }
                println!("removed {} design(s) and their verdicts/captions rows", remove.len());
            }
        }
        Cmd::EvenRect { from, to, kind, out_dir } => {
            std::fs::create_dir_all(&out_dir)?;
            for p in (from..=to).filter(|p| p % 2 == 0) {
                let mut cands: Vec<(String, String, String)> = Vec::new(); // (name, text, caption)
                // 0-torch pulse ring: one loaded pulse of 2 x dmax gt, delays summing to P / 2
                if kind != "inv" {
                    let dmax = ((p / 2 - 1) / 4).clamp(1, 4);
                    let mut ds: Vec<Cell> = split_delays(p / 2, dmax).into_iter().map(|d| Cell::Rep { d, on: false }).collect();
                    if let Some(Cell::Rep { d, .. }) = ds.first().copied() {
                        ds[0] = Cell::Rep { d, on: true };
                    }
                    if let Some((w, h, cells)) = rect_cells(&ds, &[]) {
                        let name = format!("clk-rt-rect{w}x{h}-p{p}");
                        if let Some(b) = ring::build(w, h, &cells, &name) {
                            cands.push((name, b.design().to_text(), ring::caption(w, h, &cells)));
                        }
                    }
                }
                // 1-torch inverter ring (P = 0 mod 4): the torch on the bottom side's straight
                // cell (hanging on the corner block before it), the next corner dust, repeaters
                // on the other straight cells (delays summing to P / 4 - 1)
                if p % 4 == 0 && p >= 12 && kind != "pulse" {
                    let ds = split_delays(p / 4 - 1, 4);
                    let w = 3i32;
                    let mut h = 3i32;
                    while ((2 * h - 3) as usize) < ds.len() {
                        h += 1;
                    }
                    let per = ring::perimeter(w, h);
                    let len = per.len();
                    let is_corner = |i: usize| -> bool {
                        let (x, z) = per[i];
                        let a = per[(i + len - 1) % len];
                        let b = per[(i + 1) % len];
                        !((a.0 - x) == -(b.0 - x) && (a.1 - z) == -(b.1 - z))
                    };
                    let ti = (h + 2) as usize; // the bottom side's straight cell
                    let mut cells = vec![Cell::Block; len];
                    cells[ti] = Cell::Torch { lit: true };
                    cells[(ti + 1) % len] = Cell::Dust;
                    let mut di = 0;
                    for i in 0..len {
                        if is_corner(i) || i == ti {
                            continue;
                        }
                        cells[i] = if di < ds.len() {
                            di += 1;
                            Cell::Rep { d: ds[di - 1], on: false }
                        } else {
                            Cell::Dust
                        };
                    }
                    let name = format!("clk-rt-rect{w}x{h}-inv-p{p}");
                    if let Some(b) = ring::build(w, h, &cells, &name) {
                        cands.push((name, b.design().to_text(), ring::caption(w, h, &cells)));
                    }
                }
                let found = sweep::run_all(cands.iter().map(|(n, t, _)| (n.clone(), t.clone())).collect(), 4 * p + 400, 4);
                let best = found.iter().filter(|f| f.period == p).min_by_key(|f| (f.blocks, f.volume));
                match best {
                    Some(f) => {
                        let cap = cands.iter().find(|c| c.0 == f.label).map(|c| c.2.clone()).unwrap_or_default();
                        println!("P={p} {} blocks {} | {}", f.blocks, f.wave(), f.label);
                        write_design(&out_dir.join(format!("rect-p{p}.txt")), &f.design, &format!("{}-{}b", f.label, f.blocks), "", &cap)?;
                    }
                    None => println!("P={p}: none ({})", found.iter().map(|f| format!("{} -> P{}", f.label, f.period)).collect::<Vec<_>>().join(", ")),
                }
            }
        }
        Cmd::OddRect { from, to, max_len, threads, out_dir } => {
            std::fs::create_dir_all(&out_dir)?;
            for p in (from..=to).filter(|p| p % 2 == 1) {
                // pulses and gaps of at least 2d (+1 between chained repeaters) must fit in P
                let dmax: u32 = ((p - 1) / 4).clamp(1, 4);
                let mut ds: Vec<u8> = Vec::new();
                let mut left = p;
                while left > 0 {
                    let d = left.min(dmax);
                    ds.push(d as u8);
                    left -= d;
                }
                let n = ds.len();
                let w = 3i32;
                let h = (n.div_ceil(2) + 1).max(3) as i32;
                let per = ring::perimeter(w, h);
                let len = per.len();
                let corner = |i: usize| -> bool {
                    let (x, z) = per[i];
                    let a = per[(i + len - 1) % len];
                    let b = per[(i + 1) % len];
                    !((a.0 - x) == -(b.0 - x) && (a.1 - z) == -(b.1 - z))
                };
                let straights: Vec<usize> = (0..len).filter(|&i| !corner(i)).collect();
                let extra = straights.len() - n; // 0 or 1 (2 when h was raised to 3)
                // a relay block needs repeaters on both sides along a straight run: the middle of
                // the right side (indices w .. w + h - 3)
                let right: Vec<usize> = straights.iter().copied().filter(|&i| i >= w as usize && i < (w + h - 2) as usize).collect();
                let relay: Vec<usize> = if extra >= 1 && right.len() >= 3 { vec![right[right.len() / 2]] } else { vec![] };
                let mut cells: Vec<Cell> = vec![Cell::Block; len];
                let mut di = 0;
                let mut fill_left = extra - relay.len();
                for &i in &straights {
                    if relay.contains(&i) {
                        cells[i] = Cell::Block;
                    } else if di < n {
                        cells[i] = Cell::Rep { d: ds[di], on: di == 0 };
                        di += 1;
                    } else if fill_left > 0 {
                        cells[i] = Cell::Dust;
                        fill_left -= 1;
                    }
                }
                let name = format!("clk-rt-odd-{}", ring::label(w, h, &cells));
                let Some(b) = ring::build(w, h, &cells, &name) else {
                    println!("P={p}: ring does not build ({})", ring::label(w, h, &cells));
                    continue;
                };
                let d = b.design();
                let kc = kick::kick_cells(&d);
                let lens: Vec<u32> = (1..=max_len).collect();
                let ats: Vec<u32> = (1..=(2 * p + 3)).filter(|a| a % 2 == 1).collect();
                let mut best: Option<(sweep::Found, String)> = None;
                for (c, st, l, a, f) in kick::search(&d, &kc, &lens, &ats, 4 * p + 400, threads) {
                    if f.period != p {
                        continue;
                    }
                    if best.as_ref().is_none_or(|(o, _)| (f.blocks, f.volume) < (o.blocks, o.volume)) {
                        best = Some((f, format!("{} at {:?} for {l} gt at tick {a}", clock::short(&st), c)));
                    }
                }
                match best {
                    Some((f, how)) => {
                        println!("P={p} {} blocks {} | kick {how} | {}", f.blocks, f.wave(), ring::label(w, h, &cells));
                        let cap = format!(
                            "ODD period {p}: {}; two pulses {p} gt apart: one loaded at placement, the other started by a {how} (an odd tick: a second parity class; every redstone-tick delay is even)",
                            ring::caption(w, h, &cells)
                        );
                        write_design(&out_dir.join(format!("odd-rect-p{p}.txt")), &f.design, &format!("clk-rt-odd-rect-p{p}-{}b", f.blocks), "", &cap)?;
                    }
                    None => println!("P={p}: no kick found ({})", ring::label(w, h, &cells)),
                }
            }
        }
        Cmd::OddRange { from, to, max_len, threads, out_dir } => {
            std::fs::create_dir_all(&out_dir)?;
            for p in (from..=to).filter(|p| p % 2 == 1) {
                // delays: as many 4s as possible (pulses and gaps of 8-9 gt fit when P >= 17)
                let dmax: u32 = ((p - 1) / 4).clamp(1, 4);
                let mut ds: Vec<u8> = Vec::new();
                let mut left = p;
                while left > 0 {
                    let d = left.min(dmax);
                    ds.push(d as u8);
                    left -= d;
                }
                let k = ds.len().div_ceil(2);
                let mut cells: Vec<Cell> = ds.iter().map(|&d| Cell::Rep { d, on: false }).collect();
                cells.sort_by_key(|c| std::cmp::Reverse(c.rt()));
                while cells.len() < 2 * k {
                    cells.push(Cell::Dust);
                }
                cells[0] = match cells[0] {
                    Cell::Rep { d, .. } => Cell::Rep { d, on: true },
                    x => x,
                };
                let u = ULoop { head: Head::Corner, top: cells[..k].to_vec(), bottom: cells[k..].to_vec(), tail: Tail::Dust };
                let name = format!("clk-rt-odd-{}", u.label());
                let b = u.build(&name)?;
                let d = b.design();
                let kc = kick::kick_cells(&d);
                let lens: Vec<u32> = (1..=max_len).collect();
                let ats: Vec<u32> = (1..=(2 * p + 3)).filter(|a| a % 2 == 1).collect();
                let mut best: Option<(sweep::Found, String)> = None;
                for (c, st, l, a, f) in kick::search(&d, &kc, &lens, &ats, 4 * p + 400, threads) {
                    if f.period != p {
                        continue;
                    }
                    if best.as_ref().is_none_or(|(o, _)| (f.blocks, f.volume) < (o.blocks, o.volume)) {
                        best = Some((f, format!("{} at {:?} for {l} gt at tick {a}", clock::short(&st), c)));
                    }
                }
                match best {
                    Some((f, how)) => {
                        println!("P={p} {} blocks {} | kick {how} | {}", f.blocks, f.wave(), u.label());
                        let cap = format!(
                            "ODD period {p}: {}; two pulses {p} gt apart: one loaded at placement, the other started by a {how} (an odd tick: a second parity class; every redstone-tick delay is even)",
                            u.caption()
                        );
                        write_design(&out_dir.join(format!("odd-p{p}.txt")), &f.design, &format!("clk-rt-odd-p{p}-{}b", f.blocks), &u.label(), &cap)?;
                    }
                    None => println!("P={p}: no kick found ({})", u.label()),
                }
            }
        }
        Cmd::SweepOddRing { w, h, straight, corner, max_len, per_base, single_load, horizon, threads, out_dir } => {
            let st = parse_row(&straight)?;
            let co = parse_row(&corner)?;
            let all = ring::assignments(w, h, &st, &co, 0);
            let mut cands: Vec<odd::Cand> = Vec::new();
            for cells in &all {
                let torches = cells.iter().filter(|c| matches!(c, Cell::Torch { .. })).count();
                let half: u32 = cells.iter().map(|c| c.rt()).sum();
                if half % 2 == 0 || torches % 2 == 1 {
                    continue;
                }
                let base = ring::label(w, h, cells);
                // every loading: each repeater powered or not, each torch lit or not
                let slots: Vec<usize> = (0..cells.len()).filter(|&i| matches!(cells[i], Cell::Rep { .. } | Cell::Torch { .. })).collect();
                for mask in 0u32..(1 << slots.len()) {
                    if single_load && mask.count_ones() != 1 {
                        continue;
                    }
                    let mut v = cells.clone();
                    for (b, &i) in slots.iter().enumerate() {
                        let on = mask >> b & 1 == 1;
                        v[i] = match v[i] {
                            Cell::Rep { d, .. } => Cell::Rep { d, on },
                            Cell::Torch { .. } => Cell::Torch { lit: on },
                            x => x,
                        };
                    }
                    let name = format!("clk-rt-odd-{}", ring::label(w, h, &v));
                    let Some(b) = ring::build(w, h, &v, &name) else { continue };
                    if b.conflict {
                        continue;
                    }
                    cands.push(odd::Cand { name, base: base.clone(), text: b.design().to_text(), half, caption: ring::caption(w, h, &v) });
                }
            }
            let t0 = std::time::Instant::now();
            let best = odd::search(cands, max_len, horizon, threads, per_base);
            eprintln!("searched in {:.1} s", t0.elapsed().as_secs_f64());
            println!("| P | blocks | volume | output | kick | design |");
            println!("|---|---|---|---|---|---|");
            for (p, hit) in &best {
                let f = &hit.found;
                println!("| {p} | {} | {} | {} | {} | {} |", f.blocks, f.volume, f.wave(), hit.how, f.label);
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                for (p, hit) in &best {
                    let f = &hit.found;
                    let cap = format!("ODD period {p}: {}; a second pulse started by a {} (a second parity class: every redstone-tick delay is even)", hit.caption, hit.how);
                    write_design(&dir.join(format!("odd-ring{w}x{h}-p{p}.txt")), &f.design, &format!("clk-rt-odd-ring{w}x{h}-p{p}-{}b", f.blocks), "", &cap)?;
                }
            }
        }
        Cmd::Ring { w, h, cells, out } => {
            let cells = parse_row(&cells)?;
            let name = format!("clk-rt-{}", ring::label(w, h, &cells));
            let Some(b) = ring::build(w, h, &cells, &name) else { bail!("the ring does not build (perimeter {} cells)", ring::perimeter(w, h).len()) };
            let text = b.design().to_text();
            match out {
                Some(p) => write_design(&p, &text, &name, "", &ring::caption(w, h, &cells))?,
                None => print!("{text}"),
            }
        }
        Cmd::SweepRing { w, h, straight, corner, max_powered, max_delay_cells, horizon, threads, out_dir, waves } => {
            let st = parse_row(&straight)?;
            let co = parse_row(&corner)?;
            let all = ring::assignments_max(w, h, &st, &co, max_powered, max_delay_cells.unwrap_or(usize::MAX));
            let mut caps: std::collections::HashMap<String, String> = Default::default();
            let cands: Vec<(String, String)> = all
                .iter()
                .filter_map(|cells| {
                    let name = format!("clk-rt-{}", ring::label(w, h, cells));
                    let b = ring::build(w, h, cells, &name)?;
                    if b.conflict {
                        return None;
                    }
                    caps.insert(name.clone(), ring::caption(w, h, cells));
                    Some((name, b.design().to_text()))
                })
                .collect();
            let n = cands.len();
            let t0 = std::time::Instant::now();
            let found = sweep::run_all(cands, horizon, threads);
            eprintln!("{n} rings judged in {:.1} s: {} clocks", t0.elapsed().as_secs_f64(), found.len());
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
                for ((p, wv), f) in &t.waves {
                    println!("| {p} | {wv} | {} | {} |", f.blocks, f.label);
                }
            }
            if let Some(dir) = out_dir {
                std::fs::create_dir_all(&dir)?;
                let mut fam: std::collections::BTreeMap<(u32, String), &sweep::Found> = Default::default();
                for f in &found {
                    let k = (f.period, f.family.clone());
                    if fam.get(&k).is_none_or(|o| (f.blocks, f.volume) < (o.blocks, o.volume)) {
                        fam.insert(k, f);
                    }
                }
                for ((p, family), f) in fam {
                    let short: String = family.split('+').map(|w| w.split('_').map(|s| &s[..1]).collect::<String>()).collect::<Vec<_>>().join("-");
                    let cap = caps.get(&f.label).cloned().unwrap_or_default();
                    write_design(&dir.join(format!("ring{w}x{h}-p{p}-{short}.txt")), &f.design, &format!("clk-rt-ring{w}x{h}-p{p}-{short}-{}b", f.blocks), "", &cap)?;
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
                let of = |label: &str| -> Option<&ULoop> {
                    let l = label.trim_start_matches("clk-rt-uloop-");
                    loops.iter().find(|u| u.label() == l)
                };
                let cap_of = |label: &str| -> String { of(label).map(|u| u.caption()).unwrap_or_default() };
                // family-specific name prefix (fade, uloop1t, uloop2t, ...): archive names never collide
                let kind_of = |label: &str| -> String { of(label).map(|u| u.kind()).unwrap_or_else(|| "uloop".into()) };
                for (p, f) in &t.best {
                    let l = f.label.trim_start_matches("clk-rt-uloop-");
                    write_design(&dir.join(format!("uloop-p{p}.txt")), &f.design, &format!("clk-rt-{}-p{p}-{}b", kind_of(&f.label), f.blocks), l, &cap_of(&f.label))?;
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
                        write_design(&dir.join(format!("uloop-p{p}-{short}.txt")), &f.design, &format!("clk-rt-{}-p{p}-{short}-{}b", kind_of(&f.label), f.blocks), l, &cap_of(&f.label))?;
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

/// Delays (1..=dmax each) summing to `sum`, as few as possible, largest first.
fn split_delays(sum: u32, dmax: u32) -> Vec<u8> {
    let mut ds = Vec::new();
    let mut left = sum;
    while left > 0 {
        let d = left.min(dmax);
        ds.push(d as u8);
        left -= d;
    }
    ds
}

/// A 3 x h rectangle ring: corner blocks, the given straight-cell components in perimeter order
/// (from the first straight cell after the north-west corner); extra straight cells become relay
/// blocks (between two diodes on a long side) or dust. `corner_override`: (perimeter index, cell).
fn rect_cells(straight: &[Cell], corner_override: &[(usize, Cell)]) -> Option<(i32, i32, Vec<Cell>)> {
    let w = 3i32;
    let h = (straight.len().div_ceil(2) + 1).max(3) as i32;
    let per = ring::perimeter(w, h);
    let len = per.len();
    let is_corner = |i: usize| -> bool {
        let (x, z) = per[i];
        let a = per[(i + len - 1) % len];
        let b = per[(i + 1) % len];
        !((a.0 - x) == -(b.0 - x) && (a.1 - z) == -(b.1 - z))
    };
    let straights: Vec<usize> = (0..len).filter(|&i| !is_corner(i)).collect();
    if straights.len() < straight.len() {
        return None;
    }
    let extra = straights.len() - straight.len();
    let right: Vec<usize> = straights.iter().copied().filter(|&i| i >= w as usize && i < (w + h - 2) as usize).collect();
    let relay: Vec<usize> = if extra >= 1 && right.len() >= 3 { vec![right[right.len() / 2]] } else { vec![] };
    let mut cells = vec![Cell::Block; len];
    let mut si = 0;
    let mut fill = extra - relay.len();
    for &i in &straights {
        if relay.contains(&i) {
            cells[i] = Cell::Block;
        } else if si < straight.len() {
            cells[i] = straight[si];
            si += 1;
        } else if fill > 0 {
            cells[i] = Cell::Dust;
            fill -= 1;
        }
    }
    for &(i, c) in corner_override {
        cells[i % len] = c;
    }
    Some((w, h, cells))
}

/// The extendable families (`family <name>`), in card order.
const FAMILIES: [&str; 9] = ["uloop2t", "uloop1t", "rect-pulse", "rect-inv", "odd-rect", "fade", "div", "div-fade", "counter"];

/// A family's catalog card: id, formulas, rule, judged n range.
struct Card {
    id: String,
    period: &'static str,
    blocks: &'static str,
    rule: &'static str,
    lo: u32,
    hi: u32,
}

fn family_card(name: &str) -> Result<Card> {
    let (period, blocks, rule, lo, hi) = match name {
        "uloop2t" => ("20 + 16n", "8 + 4n", "add one delay-4 repeater (powered at placement) to each row of the 2-torch U-ring (wall torch on block A -> top row -> block F -> wall torch on F -> bottom row -> A)", 0, 10),
        "uloop1t" => ("36 + 32n", "9 + 4n", "add one delay-4 repeater (powered at placement) to each row of the 1-torch inverter U-loop (block A + a dust corner -> top row -> block F -> wall torch on F -> bottom row -> A)", 0, 8),
        "rect-pulse" => ("32 + 16n", "12 + 4n", "lengthen the 3 x (3+n) ring of delay-4 repeaters with corner blocks by one row: one more repeater on each long side (one pulse loaded at placement)", 0, 10),
        "rect-inv" => ("52 + 32n", "12 + 4n", "lengthen the 3 x (3+n) inverter ring (delay-4 repeaters, corner blocks, a wall torch on one corner block, the next corner dust) by one row: one more repeater on each long side", 0, 8),
        "odd-rect" => ("23 + 8n", "16 + 4n", "lengthen the 3 x (4+n) ring (delay-4 repeaters, one delay-3, corner blocks) by one row: one more repeater on each long side; two pulses 23 + 8n gt apart, the second started by a temporary torch at an odd tick", 0, 9),
        "fade" => ("114 + 56n", "18 + 4n", "add one compare-mode comparator to each row of the comparator fading ring (2 + 2 comparators at n = 0, a lossy two-dust turn, a refill torch)", 0, 10),
        "div" => ("36 * 2^n", "9 + 18n", "append one master-slave T flip-flop stage (18 blocks: 2 latch repeaters, 2 lock repeaters, a target block with a wall torch, dust) to the chain on the P36 torch + repeater loop: the period doubles", 1, 7),
        "div-fade" => ("114 * 2^n", "18 + 18n", "append one master-slave T flip-flop stage (18 blocks) to the chain on the P114 comparator fading ring: the period doubles", 1, 5),
        "counter" => ("114 * 14^n", "18 + 26n", "wrap one more counter level around the clock (a lossless 2 + 2 comparator ring with one subtract comparator, its pulse shaper and refill torch: 26 blocks): the period x 14", 1, 3),
        _ => bail!("family: {}", FAMILIES.join(" | ")),
    };
    Ok(Card { id: format!("clk-rt-{name}"), period, blocks, rule, lo, hi })
}

/// The n-th member of an extendable family: (design name, design text, caption).
fn family_member(name: &str, n: u32) -> Result<(String, String, String)> {
    let k = n as usize;
    let rep4 = |on_first: bool, count: usize| -> Vec<Cell> { (0..count).map(|i| Cell::Rep { d: 4, on: on_first && i == 0 }).collect() };
    let all4 = |count: usize| -> Vec<Cell> { (0..count).map(|_| Cell::Rep { d: 4, on: true }).collect() };
    let uloop = |u: ULoop, dname: String, what: &str| -> Result<(String, String, String)> {
        let b = u.build(&dname)?;
        Ok((dname, b.design().to_text(), format!("{what}: {}", u.caption())))
    };
    match name {
        "uloop2t" => {
            let u = ULoop { head: Head::Torch { lit: true }, top: all4(k + 1), bottom: all4(k + 1), tail: Tail::Torch { lit: true } };
            uloop(u, format!("clk-rt-uloop2t-x{n}"), &format!("EXTENDABLE 2-torch U-ring, n = {n} (P = 20 + 16n)"))
        }
        "uloop1t" => {
            let u = ULoop { head: Head::Corner, top: all4(k + 1), bottom: all4(k + 1), tail: Tail::Torch { lit: true } };
            uloop(u, format!("clk-rt-uloop1t-x{n}"), &format!("EXTENDABLE 1-torch inverter U-loop, n = {n} (P = 36 + 32n)"))
        }
        "fade" => {
            let row = vec![Cell::Cmp; k + 2];
            let u = ULoop { head: Head::Dust2, top: row.clone(), bottom: row, tail: Tail::Fade { lit: true } };
            uloop(u, format!("clk-rt-fade-x{n}"), &format!("EXTENDABLE comparator fading ring, n = {n} (P = 114 + 56n)"))
        }
        "rect-pulse" => {
            let st = rep4(true, 4 + 2 * k);
            let (w, h, cells) = rect_cells(&st, &[]).ok_or_else(|| anyhow::anyhow!("rect"))?;
            let dname = format!("clk-rt-rect-pulse-x{n}");
            let b = ring::build(w, h, &cells, &dname).ok_or_else(|| anyhow::anyhow!("ring"))?;
            Ok((dname, b.design().to_text(), format!("EXTENDABLE repeater pulse ring, n = {n} (P = 32 + 16n): {}", ring::caption(w, h, &cells))))
        }
        "rect-inv" => {
            let (w, h) = (3i32, 3 + n as i32);
            let per = ring::perimeter(w, h);
            let len = per.len();
            let is_corner = |i: usize| -> bool {
                let (x, z) = per[i];
                let a = per[(i + len - 1) % len];
                let b = per[(i + 1) % len];
                !((a.0 - x) == -(b.0 - x) && (a.1 - z) == -(b.1 - z))
            };
            let ti = (h + 2) as usize;
            let mut cells = vec![Cell::Block; len];
            cells[ti] = Cell::Torch { lit: true };
            cells[(ti + 1) % len] = Cell::Dust;
            for i in 0..len {
                if !is_corner(i) && i != ti {
                    cells[i] = Cell::Rep { d: 4, on: false };
                }
            }
            let dname = format!("clk-rt-rect-inv-x{n}");
            let b = ring::build(w, h, &cells, &dname).ok_or_else(|| anyhow::anyhow!("ring"))?;
            Ok((dname, b.design().to_text(), format!("EXTENDABLE 1-torch inverter rectangle ring, n = {n} (P = 52 + 32n): {}", ring::caption(w, h, &cells))))
        }
        "div" => {
            let dname = format!("clk-rt-div-44-x{n}");
            let b = divider::chain(4, 4, k, &dname).ok_or_else(|| anyhow::anyhow!("stage collides"))?;
            Ok((dname, b.design().to_text(), format!("EXTENDABLE divider chain, n = {n} stages on a 1-torch U-loop (delays 4/4, P36): P = 36 x 2^n; each stage a master-slave T flip-flop (18 blocks)")))
        }
        "div-fade" => {
            let dname = format!("clk-rt-div-fade2-x{n}");
            let b = divider::chain_fade(2, k, &dname).ok_or_else(|| anyhow::anyhow!("stage collides"))?;
            Ok((dname, b.design().to_text(), format!("EXTENDABLE divider chain, n = {n} stages on a comparator fading ring (P114): P = 114 x 2^n; each stage a master-slave T flip-flop (18 blocks)")))
        }
        "counter" => {
            anyhow::ensure!(n >= 1, "counter: n >= 1");
            let ks = vec![2usize; k];
            let dname = format!("clk-rt-count-x{n}");
            let b = counter::build_nested(2, &ks, &dname);
            Ok((dname, b.design().to_text(), format!("EXTENDABLE comparator counters, n = {n} levels over a fading ring (P114): P = 114 x 14^n; each level a lossless 2 + 2 comparator ring decremented by one level per pulse of the level inside it and refilled by its own torch (26 blocks)")))
        }
        "odd-rect" => {
            // P = 23 + 8n: delays 4 x (5 + 2n) and a 3, two pulses; the kick found by search
            let p = 23 + 8 * n;
            let mut ds: Vec<Cell> = (0..5 + 2 * k).map(|i| Cell::Rep { d: 4, on: i == 0 }).collect();
            ds.push(Cell::Rep { d: 3, on: false });
            let (w, h, cells) = rect_cells(&ds, &[]).ok_or_else(|| anyhow::anyhow!("rect"))?;
            let dname = format!("clk-rt-odd-rect-x{n}");
            let b = ring::build(w, h, &cells, &dname).ok_or_else(|| anyhow::anyhow!("ring"))?;
            let d = b.design();
            let kc = kick::kick_cells(&d);
            let ats: Vec<u32> = (1..=(2 * p + 3)).filter(|a| a % 2 == 1).collect();
            let best = kick::search(&d, &kc, &[1, 2], &ats, 4 * p + 400, 8)
                .into_iter()
                .filter(|r| r.4.period == p)
                .min_by(|a, b| (a.4.blocks, a.4.volume, a.3, a.2, a.0, &a.1).cmp(&(b.4.blocks, b.4.volume, b.3, b.2, b.0, &b.1)))
                .ok_or_else(|| anyhow::anyhow!("no odd kick for P{p}"))?;
            let how = format!("{} at {:?} for {} gt at tick {}", clock::short(&best.1), best.0, best.2, best.3);
            Ok((dname, best.4.design, format!("EXTENDABLE odd two-pulse ring, n = {n} (P = 23 + 8n): {}; the second pulse started by a {how} (an odd tick)", ring::caption(w, h, &cells))))
        }
        _ => bail!("family: uloop2t | uloop1t | fade | rect-pulse | rect-inv | odd-rect | div | div-fade | counter"),
    }
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
