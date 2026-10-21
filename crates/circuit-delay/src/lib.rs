//! Track delay (crates/circuit-delay, owned by clk-rt): delay lines, output = input shifted by N
//! gt with both edges and pulse lengths preserved. `mctas circuit delay <command>`; the redsim
//! stimulus harness here is shared with track gates (crates/circuit-gates) until clockinfra's
//! judge (crates/circuit) is merged. Notes: designs/circuits/tracks/delay/NOTES.md.

pub mod build;
pub mod census;
pub mod delay;
pub mod families;
pub mod harness;
pub mod judge;

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
    /// The odd delay line (a sticky piston's 3/1 copy OR'ed with itself 2 gt later, then a tail of
    /// repeaters): N = 3 + 2 x the tail's delays. `--n` picks the fewest repeaters for an odd N >= 3.
    PistonOr {
        #[arg(long, value_delimiter = ',')]
        tail: Vec<u8>,
        #[arg(long)]
        n: Option<u32>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// One member of an extendable delay family, with the family's `extend` headers (the base of
    /// a card): rep2 / rep4 / rep6 / rep8 (a delay r/2 repeater then n delay-4 repeaters: N = r +
    /// 8n), cmp (n + 1 comparators: N = 2 + 2n), or5 / or7 / or9 / or11 (the piston OR line with a
    /// tail of one delay (base - 3)/2 repeater then n delay-4 repeaters: N = base + 8n), torch (a
    /// tower of 2 + 2n torches: N = 6 + 4n).
    Family {
        name: String,
        #[arg(long, default_value_t = 0)]
        n: u32,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Judge every member of the families (default: all) on the judge's redsim screen over their
    /// n range: N and the block count must equal the card's formulas.
    FamilyCheck {
        names: Vec<String>,
    },
    /// Remove archived designs (covered by a family card) from a study archive: the files and
    /// their rows in verdicts.tsv / captions.tsv (and aliases.tsv when present).
    Prune {
        #[arg(long, default_value = "designs/circuits/delay/archive")]
        archive: PathBuf,
        /// File names in the archive (`delay-rep-1.txt`).
        files: Vec<String>,
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
        Cmd::PistonOr { tail, n, out } => {
            let tail = match n {
                Some(n) if n >= 3 && n % 2 == 1 => families::split_ticks((n - 3) / 2),
                Some(n) => bail!("--n {n}: the piston OR line makes odd N >= 3"),
                None => tail,
            };
            let sum: u32 = tail.iter().map(|d| *d as u32).sum();
            let n = 3 + 2 * sum;
            let tl = if tail.is_empty() { String::new() } else { format!("; the OR lands on a dust and {} repeater(s) (delays {:?}) add {} gt", tail.len(), tail, 2 * sum) };
            emit(families::piston_or(&tail, &format!("delay-piston-or-{n}")), out, &format!("Delay line N = {n} gt (track delay): a sticky piston pushes a redstone block one cell (it arrives 3 gt after the input rises and leaves 1 gt after it falls); the output touches the block's new cell, and a repeater behind that cell feeds the output again 2 gt later through a stone and two dusts: the OR of the 3/1 and 5/3 copies is a 3/3 delay{tl}"))?;
        }
        Cmd::Family { name, n, out } => {
            let (b, caption) = families::member(&name, n)?;
            let f = families::card(&name)?;
            let mut b = b;
            for (k, v) in [
                ("extend family", f.id.clone()),
                ("extend axis", f.axis.clone()),
                ("extend blocks", f.blocks.clone()),
                ("extend rule", f.rule.clone()),
                ("extend n", format!("{}..{}", f.lo, f.hi)),
                ("extend base", n.to_string()),
                ("extend generator", format!("mctas circuit delay family {name} --n {{n}} --out {{out}}")),
                ("extend group", f.group.to_string()),
                ("extend group title", f.gtitle.to_string()),
                ("extend group period", f.gaxis.to_string()),
                ("extend group note", f.gnote.to_string()),
                // a line of repeaters (the base and every member) tiles side by side at pitch 1:
                // judged in the game for all four bases (2026-10-20); regenerating dropped it once
                ("tile", if name.starts_with("rep") { "0 0 1".to_string() } else { String::new() }),
            ] {
                if !v.is_empty() {
                    b.meta.insert(k.to_string(), v);
                }
            }
            emit(b, out, &caption)?;
        }
        Cmd::FamilyCheck { names } => {
            let names: Vec<String> = if names.is_empty() { families::FAMILIES.iter().map(|s| s.to_string()).collect() } else { names };
            let mut bad = 0;
            for name in &names {
                let c = families::card(name)?;
                let eval = |f: &str, n: u32| -> Option<u32> {
                    // "a + bn"
                    let (a, b) = f.split_once('+')?;
                    let a: u32 = a.trim().parse().ok()?;
                    let b: u32 = b.trim().trim_end_matches('n').trim().parse().ok()?;
                    Some(a + b * n)
                };
                for n in c.lo..=c.hi {
                    let (b, _) = families::member(name, n)?;
                    let mut d = b.design();
                    d.meta.insert("circuit".into(), "delay".into());
                    let blocks = harness::Circuit::from_design(d.clone())?.blocks() as u32;
                    let res = judge::judge_sim(d);
                    let want_n = eval(&c.axis, n);
                    let want_b = eval(&c.blocks, n);
                    let ok = match &res {
                        Some((true, m, _)) => Some(m.starts_with(&format!("N {} gt", want_n.unwrap_or(0)))) == Some(true) && Some(blocks) == want_b,
                        _ => false,
                    };
                    if !ok {
                        bad += 1;
                    }
                    println!("{} {} n={n}: want N {:?} blocks {:?}; got {blocks} blocks, {:?}", if ok { "OK  " } else { "BAD " }, c.id, want_n, want_b, res.map(|r| r.1));
                }
            }
            if bad > 0 {
                bail!("{bad} member(s) off their formulas");
            }
        }
        Cmd::Prune { archive, files } => {
            for tsv in ["verdicts.tsv", "captions.tsv", "aliases.tsv"] {
                let p = archive.join(tsv);
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                let mut out = String::new();
                let mut dropped = 0;
                for line in text.lines() {
                    if line.split('\t').any(|cell| files.iter().any(|f| cell == f)) {
                        dropped += 1;
                        continue;
                    }
                    out.push_str(line);
                    out.push('\n');
                }
                std::fs::write(&p, out)?;
                println!("{tsv}: {dropped} row(s) dropped");
            }
            for f in &files {
                let p = archive.join(f);
                if p.exists() {
                    std::fs::remove_file(&p)?;
                    println!("removed {}", p.display());
                } else {
                    println!("not found: {}", p.display());
                }
            }
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
    // the best scores confirmed by the judge so far (a candidate that cannot improve is not judged)
    let seen: std::sync::Mutex<std::collections::BTreeMap<u32, (u32, u32)>> = Default::default();
    census::search(sp, threads, |blocks| {
        let mut v = Vec::new();
        for (oi, r, f) in judge(blocks, None) {
            if r != f {
                continue;
            }
            if seen.lock().unwrap().get(&r).is_some_and(|s| s.0 <= blocks.len() as u32) {
                continue;
            }
            // the official judge's redsim screen: every pulse and gap of its stimulus in one run
            // (and the lamp in the output cell)
            let d = judge::design("candidate", "delay", blocks, &[("input".to_string(), sp.inputs[0])], outs[oi]);
            let Some((true, metrics, _)) = judge::judge_sim(d) else { continue };
            if !metrics.starts_with(&format!("N {r} gt")) {
                continue;
            }
            // minimal: without any one of its free blocks the judge refuses it (or gives another N)
            let minimal = (nfixed..blocks.len()).all(|i| {
                let less: Vec<(harness::Pos, redsim::Block)> = blocks.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| *b).collect();
                let d = judge::design("candidate", "delay", &less, &[("input".to_string(), sp.inputs[0])], outs[oi]);
                !matches!(judge::judge_sim(d), Some((true, m, _)) if m.starts_with(&format!("N {r} gt")))
            });
            if !minimal {
                continue;
            }
            let num = |key: &str| -> u32 { metrics.split(", ").find_map(|kv| kv.strip_prefix(key)).and_then(|v| v.trim_end_matches(" gt").trim().parse().ok()).unwrap_or(99) };
            let score = (blocks.len() as u32, num("min-pulse ") + num("min-gap "));
            {
                let mut s = seen.lock().unwrap();
                if s.get(&r).is_none_or(|x| score < *x) {
                    s.insert(r, score);
                }
            }
            v.push((format!("N={r}"), score, metrics, outs[oi]));
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
