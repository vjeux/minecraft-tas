//! `mctas itemclock ...`: the item-clock tools (run in redsim, verify in the game, census).

use crate::census;
use crate::jvm;
use crate::sim::{self, Spec};
use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use redsim::BlockPos;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum ItemClockCmd {
    /// Run designs in redsim until their whole state repeats (container contents and hopper
    /// cooldowns included): period, transient, family (block types whose state or contents
    /// change), the output's on ticks and rising edges, and every active cell's values over one
    /// period (comparators: output signal; containers: item count; others: on/off).
    Run {
        designs: Vec<PathBuf>,
        /// Give up after this many ticks without a repeat.
        #[arg(long, default_value_t = 20000)]
        ticks: u32,
        /// Print each active cell's per-tick values.
        #[arg(long)]
        waves: bool,
    },
    /// Measure designs in the REAL GAME: placed like the clock judge at several positions and in
    /// the 4 rotations, every block state and block entity recorded each tick; the period there
    /// (first repeat that holds to the end of the recording) and the first tick where redsim
    /// disagrees (when redsim models the design).
    Verify {
        designs: Vec<PathBuf>,
        /// Ticks to record per placement (at least 3 periods + the transient).
        #[arg(long, default_value_t = 400)]
        ticks: u32,
        /// Extra world offsets `dx,dy,dz;...` (the origin is always tried).
        #[arg(long, default_value = "17,3,-9;-30,0,21")]
        offsets: String,
        /// Skip the 3 turned copies.
        #[arg(long)]
        no_rotations: bool,
    },
    /// Every design of k blocks in a box from a palette with container contents from a list,
    /// judged in redsim: irreducible item clocks by period and type (designs + clocks.tsv +
    /// SUMMARY.md in --out).
    Census {
        /// Box size x,y,z.
        #[arg(long, default_value = "3,2,2")]
        size: String,
        /// Block counts, `4` or `3..5`.
        #[arg(long, default_value = "4")]
        k: String,
        /// Palette shorthands (hopper, dropper, dispenser, comparator, comparator-c, repeater,
        /// repeater1, dust, torch, observer, sticky, piston, rblock, lamp, stone, glass, target)
        /// or block states, comma separated.
        #[arg(long, default_value = "hopper,dropper,comparator,stone")]
        palette: String,
        /// Container contents options (each container: empty or one of these): `stone:1`,
        /// `sword:2` (unstackables, one per slot), `snowball:3`, raw SNBT; `;` separated.
        #[arg(long, default_value = "stone:1;stone:2")]
        contents: String,
        /// Max ticks to wait for a repeat per candidate.
        #[arg(long, default_value_t = 2000)]
        ticks: u32,
        #[arg(long, default_value_t = 16)]
        jobs: usize,
        /// Keep clocks whose items do not move (pure redstone clocks met on the way).
        #[arg(long)]
        any_family: bool,
        #[arg(long)]
        out: PathBuf,
    },
}

fn load(p: &PathBuf) -> Result<Design> {
    Design::load(p)
}

fn parse_k(s: &str) -> Result<(usize, usize)> {
    if let Some((a, b)) = s.split_once("..") {
        Ok((a.trim().parse()?, b.trim().trim_start_matches('=').parse()?))
    } else {
        let k = s.trim().parse()?;
        Ok((k, k))
    }
}

fn output_port(d: &Design) -> Option<BlockPos> {
    d.ports.get("output").map(|p| BlockPos::from_arr(*p))
}

pub fn run(version: &str, cmd: ItemClockCmd) -> Result<()> {
    match cmd {
        ItemClockCmd::Run { designs, ticks, waves } => {
            for path in &designs {
                let d = load(path)?;
                let spec = Spec::from_design(&d).map_err(|e| anyhow!("{}: {e}", path.display()))?;
                match sim::judge(&spec, ticks, output_port(&d)).map_err(|e| anyhow!("{}: {e}", path.display()))? {
                    Ok(ch) => {
                        let kind = if ch.period < 2 || !ch.signal { "STATIC" } else { "CLOCK" };
                        println!(
                            "{}: {kind} P={} gt (transient {}), {} blocks, family {}, output {:?} on {}/{} gt, {} rising",
                            path.display(),
                            ch.period,
                            ch.transient,
                            spec.blocks.len(),
                            ch.family,
                            ch.output.map(|p| p.arr()),
                            ch.on_ticks,
                            ch.period,
                            ch.rising
                        );
                        if waves {
                            for w in &ch.waves {
                                let vals: Vec<String> = w.values.iter().map(|v| v.to_string()).collect();
                                println!("  {:?} {}{}: {}", w.pos.arr(), w.name, if w.items_change { " (items)" } else { "" }, vals.join(" "));
                            }
                        }
                    }
                    Err(v) => println!("{}: {v:?}", path.display()),
                }
            }
        }
        ItemClockCmd::Verify { designs, ticks, offsets, no_rotations } => {
            let mut offs: Vec<[i32; 3]> = vec![[0, 0, 0]];
            for o in offsets.split(';').filter(|s| !s.trim().is_empty()) {
                let v: Vec<i32> = o.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("bad --offsets")?;
                if v.len() != 3 {
                    bail!("bad offset {o:?}");
                }
                offs.push([v[0], v[1], v[2]]);
            }
            let mut w = Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
            for path in &designs {
                let d = load(path)?;
                let mut periods = Vec::new();
                let mut lines = Vec::new();
                let turns: Vec<u8> = if no_rotations { vec![0] } else { vec![0, 1, 2, 3] };
                for t in &turns {
                    let td = jvm::turned(&mut w, &d, *t)?;
                    for o in &offs {
                        let pd = td.translated(*o);
                        let frames = jvm::record(&mut w, &pd, ticks)?;
                        let per = jvm::period(&frames);
                        let cmp = match jvm::redsim_frames(&pd, ticks) {
                            Ok(m) => match jvm::first_mismatch(&pd, &frames, &m) {
                                Some(x) => format!("redsim DIFFERS: {x}"),
                                None => "redsim identical".to_string(),
                            },
                            Err(e) => format!("redsim: {e}"),
                        };
                        periods.push(per.map(|p| p.0));
                        lines.push(format!("  turn {t} offset {o:?}: {} ; {cmp}", per.map(|(p, tr)| format!("P={p} (transient {tr})")).unwrap_or("no period".into())));
                    }
                }
                let all_same = periods.iter().all(|p| p.is_some() && *p == periods[0]);
                println!(
                    "{}: {} ({} blocks, {} placements)",
                    path.display(),
                    if all_same { format!("PERIODIC P={} gt everywhere", periods[0].unwrap()) } else { "NOT THE SAME EVERYWHERE".to_string() },
                    d.blocks.len(),
                    periods.len()
                );
                for l in lines {
                    println!("{l}");
                }
            }
        }
        ItemClockCmd::Census { size, k, palette, contents, ticks, jobs, any_family, out } => {
            let s: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("bad --size")?;
            if s.len() != 3 {
                bail!("--size takes x,y,z");
            }
            let (k_min, k_max) = parse_k(&k)?;
            let mut pal = Vec::new();
            for name in palette.split(',').filter(|s| !s.is_empty()) {
                for st in census::palette_entry(name.trim()).map_err(|e| anyhow!(e))? {
                    pal.push(redsim::Block::parse(&st).map_err(|e| anyhow!("{st}: {e}"))?);
                }
            }
            let mut cont = Vec::new();
            for c in contents.split(';').filter(|s| !s.trim().is_empty()) {
                cont.push(census::contents_entry(c.trim()).map_err(|e| anyhow!(e))?);
            }
            let o = census::Opts { size: [s[0], s[1], s[2]], k_min, k_max, palette: pal, contents: cont, max_ticks: ticks, jobs, any_family };
            eprintln!("census: box {size}, k {k_min}..={k_max}, {} palette states, {} contents options", o.palette.len(), o.contents.len());
            let found = census::run(&o, true);
            let summary = census::report(&found, &out)?;
            println!("{summary}");
            println!("{} clock variants written to {}", found.len(), out.display());
        }
    }
    Ok(())
}
