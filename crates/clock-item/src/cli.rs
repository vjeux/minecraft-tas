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
    /// Fill a skeleton: every filling of k of the free cells (a box minus the skeleton's cells)
    /// from a palette, judged in redsim; irreducible item clocks by period and type (designs +
    /// clocks.tsv + SUMMARY.md in --out). The skeleton keeps its `nbt:` contents.
    Fill {
        skeleton: PathBuf,
        /// Free box `x0,y0,z0:x1,y1,z1` (inclusive; the skeleton's own cells are skipped).
        #[arg(long)]
        free: String,
        /// Cells to fill, `2` or `1..3`.
        #[arg(long, default_value = "1..2")]
        k: String,
        /// Palette shorthands (see census) or block states, comma separated.
        #[arg(long, default_value = "stone,dust,repeater,torch,comparator")]
        palette: String,
        #[arg(long, default_value_t = 4000)]
        ticks: u32,
        #[arg(long, default_value_t = 64)]
        jobs: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// The constructive census (ccensus.rs): every flow cycle of containers (hoppers push into
    /// what they face and pull from above; droppers push when triggered) in a box, every
    /// comparator reading one of them (beside it or through one conductor) with a floor, up to
    /// --extra more blocks from --palette next to the structure, every contents choice of the
    /// cycle containers; irreducible item clocks by period and type.
    Ccensus {
        /// Box the cycle's containers lie in, x,y,z.
        #[arg(long, default_value = "3,2,3")]
        size: String,
        /// Cycle lengths, `2..4`.
        #[arg(long, default_value = "2..4")]
        cycle: String,
        /// Allow droppers in the cycle.
        #[arg(long)]
        droppers: bool,
        /// Comparators per design (1 or 2).
        #[arg(long, default_value_t = 1)]
        comparators: usize,
        #[arg(long, default_value_t = 0)]
        extra: usize,
        #[arg(long, default_value = "stone,dust,repeater1,torch")]
        palette: String,
        #[arg(long, default_value = "stone:1;stone:2")]
        contents: String,
        /// At most this many cycle containers hold items.
        #[arg(long, default_value_t = 2)]
        max_filled: usize,
        #[arg(long, default_value_t = 4000)]
        ticks: u32,
        #[arg(long, default_value_t = 64)]
        jobs: usize,
        #[arg(long)]
        out: PathBuf,
    },
    /// Sweep the contents of a design: every combination of item counts (one stack of --item per
    /// container, or N unstackables) in the listed containers; prints the period of each and the
    /// longest / every distinct period, and writes the best design per period to --out.
    Sweep {
        design: PathBuf,
        /// Containers `x,y,z;x,y,z` (their `nbt:` lines are replaced).
        #[arg(long)]
        at: String,
        /// Counts per container, `0..64` (0 = empty).
        #[arg(long, default_value = "0..8")]
        counts: String,
        /// Item ids to try (one kind per run of the product), comma separated.
        #[arg(long, default_value = "stone")]
        items: String,
        /// Transfer cooldowns to try per filled container (hoppers), comma separated.
        #[arg(long, default_value = "0")]
        cooldowns: String,
        #[arg(long, default_value_t = 200000)]
        ticks: u32,
        #[arg(long, default_value_t = 64)]
        jobs: usize,
        /// Print every combination (not only new periods).
        #[arg(long)]
        all: bool,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Copy a found design into the track folder under a clean name with a caption (the design's
    /// first comment block, which `clock archive` takes as the caption) and the redsim facts.
    Promote {
        /// One or more designs; with several, `--name` and `--caption` should carry `{p}` (the
        /// redsim period) so each file gets its own name.
        #[arg(required = true)]
        designs: Vec<PathBuf>,
        /// The file name (`{p}` = the redsim period).
        #[arg(long)]
        name: String,
        /// How it works (one line or more; `\n` splits lines; `{p}` = the redsim period).
        #[arg(long)]
        caption: String,
        #[arg(long, default_value = "designs/clocks/tracks/clk-item")]
        dir: PathBuf,
    },
    /// Sweep the dropper-ring family (families.rs `drop_ring`): w x 3 rings (w = 3..=--max-w), every
    /// comparator column, every dropper set around the fired cell, both directions, contents = one
    /// or two ring cells holding 1..=--max-n of one kind: the distinct periods (fewest blocks, then
    /// items) and, with --out, one design per period.
    SweepDropring {
        #[arg(long, default_value_t = 5)]
        max_w: i32,
        #[arg(long, default_value_t = 3)]
        max_n: u32,
        #[arg(long, default_value = "stone,diamond_sword")]
        items: String,
        /// Also two filled cells (else one).
        #[arg(long)]
        pairs: bool,
        /// Repeater chains between the comparator and the dropper, comma separated, each a string
        /// of delays (`-` = none): `-,1,2,3,4,11` .
        #[arg(long, default_value = "-")]
        delays: String,
        #[arg(long, default_value_t = 20000)]
        ticks: u32,
        #[arg(long, default_value_t = 64)]
        jobs: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Sweep the ring-lock family (families.rs `ring_lock`): 3-wide hopper rings of depth 3..=--max-d,
    /// every comparator row, both directions, N items of --item as one stack in one ring hopper
    /// (every hopper, N = 1..=--max-items): the periods each gives (longest and every distinct one).
    SweepRinglock {
        #[arg(long, default_value_t = 3)]
        min_d: i32,
        #[arg(long, default_value_t = 5)]
        max_d: i32,
        /// Only comparator rows up to this one (1 = the row next to the start corner).
        #[arg(long, default_value_t = 1000)]
        max_row: i32,
        /// Only ring hoppers up to this index get the items (0 = the start corner).
        #[arg(long, default_value_t = 1000)]
        max_hopper: usize,
        /// Print every case.
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = 16)]
        max_items: u32,
        #[arg(long, default_value = "stone")]
        item: String,
        #[arg(long, default_value_t = 200000)]
        ticks: u32,
        #[arg(long, default_value_t = 64)]
        jobs: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Sweep the hopper-loop family (families.rs): every rectangular loop up to --max-w x --max-d
    /// (the facing pair is 2x1), with m items of one kind placed as one stack in hopper 0
    /// (`stack`), one per hopper side by side (`row`) or evenly spaced (`spread`); prints the
    /// period, blocks and waveform of each, and writes the first design of every new period to
    /// --out.
    SweepLoop {
        #[arg(long, default_value_t = 4)]
        max_w: i32,
        #[arg(long, default_value_t = 4)]
        max_d: i32,
        #[arg(long, default_value_t = 3)]
        max_items: u32,
        /// Item id (stone, diamond_sword, snowball...).
        #[arg(long, default_value = "stone")]
        item: String,
        #[arg(long, default_value_t = 20000)]
        ticks: u32,
        #[arg(long)]
        out: Option<PathBuf>,
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

/// A sweep cell value (count * 16 + transfer cooldown) as container SNBT.
fn packed_contents(kind: &str, k: usize) -> Result<String, String> {
    let (count, cd) = (k / 16, k % 16);
    let mut s = census::contents_entry(&format!("{kind}:{count}"))?;
    if cd > 0 {
        s.pop();
        s.push_str(&format!(",TransferCooldown:{cd}}}"));
    }
    Ok(s)
}

/// A sweep cell value for humans: `count` or `count@cooldown`.
fn packed_label(k: usize) -> String {
    if k % 16 == 0 { (k / 16).to_string() } else { format!("{}@{}", k / 16, k % 16) }
}

/// One dropper-ring case: (w, comparator column, dropper mask, reversed, filled cells as
/// (ring index, count), item kind, repeater delays).
type DropCase = (i32, i32, u8, bool, Vec<(usize, u32)>, String, Vec<u8>);

fn drop_case_design(c: &DropCase) -> Option<Design> {
    let (w, xc, mask, rev, fill, kind, delays) = c;
    let l = (2 * w + 2 * (1 + delays.len() as i32)) as usize;
    let mut items: Vec<Vec<crate::families::Stack>> = vec![Vec::new(); l];
    for (i, n) in fill {
        if *i >= l {
            return None;
        }
        if kind == "diamond_sword" {
            items[*i] = (0..*n).map(|_| (kind.clone(), 1)).collect();
        } else {
            items[*i] = vec![(kind.clone(), *n)];
        }
    }
    crate::families::drop_ring_delayed(*w, *xc, *mask, *rev, delays, &items)
}

#[allow(clippy::too_many_arguments)]
fn sweep_dropring(max_w: i32, max_n: u32, items: &str, pairs: bool, chains: &[Vec<u8>], ticks: u32, jobs: usize, out: Option<&std::path::Path>) -> Result<()> {
    let kinds: Vec<String> = items.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let mut cases: Vec<DropCase> = Vec::new();
    for delays in chains {
        for w in 3..=max_w {
            let l = (2 * w + 2 * (1 + delays.len() as i32)) as usize;
            for xc in 1..=w - 2 {
                for mask in [2u8, 3, 6, 7] {
                    for rev in [false, true] {
                        for kind in &kinds {
                            for i in 0..l {
                                for n in 1..=max_n {
                                    cases.push((w, xc, mask, rev, vec![(i, n)], kind.clone(), delays.clone()));
                                    if pairs {
                                        for j in i + 1..l {
                                            for m in 1..=max_n {
                                                cases.push((w, xc, mask, rev, vec![(i, n), (j, m)], kind.clone(), delays.clone()));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    eprintln!("sweep-dropring: {} cases", cases.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let found: std::sync::Mutex<std::collections::BTreeMap<u32, (usize, u32, usize)>> = std::sync::Mutex::new(std::collections::BTreeMap::new());
    std::thread::scope(|sc| {
        for _ in 0..jobs.max(1) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= cases.len() {
                        break;
                    }
                    let c = &cases[i];
                    let Some(des) = drop_case_design(c) else { continue };
                    if let Ok((p, _, _, _)) = crate::families::measure(&des, ticks) {
                        let blocks = des.blocks.len();
                        let n_items: u32 = c.4.iter().map(|(_, n)| n).sum();
                        let mut f = found.lock().unwrap();
                        let better = f.get(&p).is_none_or(|(b, k, _)| (blocks, n_items) < (*b, *k));
                        if better {
                            f.insert(p, (blocks, n_items, i));
                        }
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    println!("{} distinct periods: {:?}", found.len(), found.keys().collect::<Vec<_>>());
    for (p, (blocks, _, i)) in &found {
        let (w, xc, mask, rev, fill, kind, delays) = &cases[*i];
        println!("P={p}: {blocks} blocks, ring {w}x{}, comparator column {xc}, repeaters {delays:?}, droppers {mask:03b}{}, {kind} {fill:?}", 3 + delays.len(), if *rev { ", reversed" } else { "" });
    }
    if let Some(dir) = out {
        std::fs::create_dir_all(dir)?;
        for (p, (_, _, i)) in &found {
            if let Some(mut des) = drop_case_design(&cases[*i]) {
                des.name = format!("dropring-{}x{}-p{p}", cases[*i].0, 3 + cases[*i].6.len());
                des.save(&dir.join(format!("{}.txt", des.name)))?;
            }
        }
    }
    Ok(())
}

/// Copy a found design into the track folder under a clean name (`{p}` = its redsim period) with a
/// caption comment block and the redsim facts.
fn promote(design: &PathBuf, name: &str, caption: &str, dir: &std::path::Path) -> Result<()> {
    let mut d = load(design)?;
    let spec = Spec::from_design(&d).map_err(|e| anyhow!("{}: {e}", design.display()))?;
    let facts = match sim::judge(&spec, 200000, output_port(&d)).map_err(|e| anyhow!(e))? {
        Ok(ch) => ch,
        Err(v) => bail!("{}: not a clock in redsim: {v:?}", design.display()),
    };
    let p = facts.period.to_string();
    let name = name.replace("{p}", &p);
    d.name = name.clone();
    for k in ["mechanism", "waveform", "family", "period", "whole-state-period"] {
        d.meta.remove(k);
    }
    let mut text = String::new();
    for line in caption.replace("{p}", &p).split("\\n") {
        text.push_str(&format!("# {line}\n"));
    }
    text.push_str(&format!(
        "# redsim: P={} gt{}, warmup {}, family {}, {} blocks, output {}\n",
        facts.period,
        if facts.whole_period != facts.period { format!(" (whole state {} gt)", facts.whole_period) } else { String::new() },
        facts.transient,
        facts.family,
        spec.blocks.len(),
        if facts.analog { "analog (strength only)".to_string() } else { format!("on {}/{} gt, {} rising edge(s)", facts.on_ticks, facts.period, facts.rising) }
    ));
    text.push('\n');
    text.push_str(&d.to_text());
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{name}.txt"));
    std::fs::write(&path, text)?;
    println!("{}: P={} gt, {} blocks", path.display(), facts.period, spec.blocks.len());
    Ok(())
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
                        let hidden = if ch.whole_period != ch.period { format!(", whole state {} gt", ch.whole_period) } else { String::new() };
                        println!(
                            "{}: {kind} P={} gt{hidden} (transient {}), {} blocks, family {}, output {:?} on {}/{} gt, {} rising",
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
        ItemClockCmd::Fill { skeleton, free, k, palette, ticks, jobs, out } => {
            let d = load(&skeleton)?;
            let base = Spec::from_design(&d).map_err(|e| anyhow!("{}: {e}", skeleton.display()))?;
            // The skeleton must place (its nbt lines on real containers): else every candidate
            // fails the same way and the fill silently finds nothing.
            let _ = sim::judge(&base, 4, None).map_err(|e| anyhow!("{}: the skeleton does not place: {e}", skeleton.display()))?;
            let (a, b) = free.split_once(':').context("--free takes x0,y0,z0:x1,y1,z1")?;
            let parse3 = |s: &str| -> Result<[i32; 3]> {
                let v: Vec<i32> = s.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
                if v.len() != 3 {
                    bail!("bad corner {s:?}");
                }
                Ok([v[0], v[1], v[2]])
            };
            let (lo, hi) = (parse3(a)?, parse3(b)?);
            let taken: std::collections::HashSet<BlockPos> = base.blocks.iter().map(|(p, _)| *p).collect();
            let mut cells = Vec::new();
            for y in lo[1].min(hi[1])..=lo[1].max(hi[1]) {
                for z in lo[2].min(hi[2])..=lo[2].max(hi[2]) {
                    for x in lo[0].min(hi[0])..=lo[0].max(hi[0]) {
                        let p = BlockPos::new(x, y, z);
                        if !taken.contains(&p) {
                            cells.push(p);
                        }
                    }
                }
            }
            let (k_min, k_max) = parse_k(&k)?;
            let mut pal = Vec::new();
            for name in palette.split(',').filter(|s| !s.is_empty()) {
                for st in census::palette_entry(name.trim()).map_err(|e| anyhow!(e))? {
                    pal.push(redsim::Block::parse(&st).map_err(|e| anyhow!("{st}: {e}"))?);
                }
            }
            eprintln!("fill: {} free cells, k {k_min}..={k_max}, {} palette states", cells.len(), pal.len());
            let o = crate::fill::FillOpts { base, free: cells, k_min, k_max, palette: pal, max_ticks: ticks, jobs };
            let found = crate::fill::run(&o, true);
            let summary = census::report(&found, &out)?;
            println!("{summary}");
            println!("{} clock variants written to {}", found.len(), out.display());
        }
        ItemClockCmd::Ccensus { size, cycle, droppers, comparators, extra, palette, contents, max_filled, ticks, jobs, out } => {
            let s: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("bad --size")?;
            if s.len() != 3 {
                bail!("--size takes x,y,z");
            }
            let (min_cycle, max_cycle) = parse_k(&cycle)?;
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
            let o = crate::ccensus::COpts {
                size: [s[0], s[1], s[2]],
                max_cycle,
                min_cycle,
                droppers,
                comparators,
                extra,
                extra_palette: pal,
                contents: cont,
                max_filled,
                max_ticks: ticks,
                jobs,
            };
            let found = crate::ccensus::run(&o, true);
            let summary = census::report(&found, &out)?;
            println!("{summary}");
            println!("{} clock variants written to {}", found.len(), out.display());
        }
        ItemClockCmd::Sweep { design, at, counts, items, cooldowns, ticks, jobs, all, out } => {
            let d = load(&design)?;
            let mut cells: Vec<[i32; 3]> = Vec::new();
            for c in at.split(';').filter(|s| !s.trim().is_empty()) {
                let v: Vec<i32> = c.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("bad --at")?;
                if v.len() != 3 {
                    bail!("bad --at cell {c:?}");
                }
                cells.push([v[0], v[1], v[2]]);
            }
            let (lo, hi) = parse_k(&counts)?;
            let kinds: Vec<String> = items.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            let cds: Vec<usize> = cooldowns.split(',').map(|s| s.trim().parse::<usize>()).collect::<Result<_, _>>().context("bad --cooldowns")?;
            if cds.iter().any(|c| *c >= 16) {
                bail!("--cooldowns must be below 16");
            }
            // One value per cell: count * 16 + transfer cooldown.
            let mut opts: Vec<usize> = Vec::new();
            for count in lo..=hi {
                if count == 0 {
                    opts.push(0);
                } else {
                    opts.extend(cds.iter().map(|cd| count * 16 + cd));
                }
            }
            let mut jobs_list: Vec<(String, Vec<usize>)> = Vec::new();
            let n = opts.len();
            for kind in &kinds {
                let total = n.pow(cells.len() as u32);
                for c in 0..total {
                    let mut x = c;
                    let mut v = Vec::new();
                    for _ in &cells {
                        v.push(opts[x % n]);
                        x /= n;
                    }
                    if v.iter().all(|k| *k == 0) {
                        continue;
                    }
                    jobs_list.push((kind.clone(), v));
                }
            }
            eprintln!("sweep: {} contents combinations", jobs_list.len());
            let results: std::sync::Mutex<Vec<(String, Vec<usize>, Result<(u32, u32, u32, u32, bool, u32), String>)>> = std::sync::Mutex::new(Vec::new());
            let next = std::sync::atomic::AtomicUsize::new(0);
            std::thread::scope(|sc| {
                for _ in 0..jobs.max(1) {
                    sc.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= jobs_list.len() {
                                break;
                            }
                            let (kind, v) = &jobs_list[i];
                            let mut dd = d.clone();
                            for (cell, k) in cells.iter().zip(v) {
                                dd.block_entities.remove(cell);
                                if *k > 0 {
                                    dd.block_entities.insert(*cell, packed_contents(kind, *k).unwrap());
                                }
                            }
                            let r = (|| -> Result<(u32, u32, u32, u32, bool, u32), String> {
                                let spec = Spec::from_design(&dd)?;
                                match sim::judge(&spec, ticks, output_port(&dd))? {
                                    Ok(ch) if ch.signal && ch.period >= 2 => Ok((ch.period, ch.whole_period, ch.on_ticks, ch.rising, ch.analog, ch.transient)),
                                    Ok(ch) => Err(format!("static (P={})", ch.period)),
                                    Err(v) => Err(format!("{v:?}")),
                                }
                            })();
                            results.lock().unwrap().push((kind.clone(), v.clone(), r));
                        }
                    });
                }
            });
            let mut results = results.into_inner().unwrap();
            results.sort_by(|a, b| (&a.0, &a.1).cmp(&(&b.0, &b.1)));
            let mut by_p: std::collections::BTreeMap<u32, (String, Vec<usize>)> = std::collections::BTreeMap::new();
            for (kind, v, r) in &results {
                let labels: Vec<String> = v.iter().map(|k| packed_label(*k)).collect();
                let labels = labels.join(", ");
                match r {
                    Ok((p, whole, on, edges, analog, tr)) => {
                        let new = !by_p.contains_key(p);
                        if all || new {
                            println!("{kind} [{labels}]: P={p} whole {whole} transient {tr} on {on} edges {edges}{}", if *analog { " analog" } else { "" });
                        }
                        // keep the fewest items per period
                        let items: usize = v.iter().map(|k| k / 16).sum();
                        let better = by_p.get(p).is_none_or(|(_, w)| items < w.iter().map(|k| k / 16).sum::<usize>());
                        if better {
                            by_p.insert(*p, (kind.clone(), v.clone()));
                        }
                    }
                    Err(e) => {
                        if all {
                            println!("{kind} [{labels}]: {e}");
                        }
                    }
                }
            }
            println!("{} distinct periods: {:?}", by_p.len(), by_p.keys().collect::<Vec<_>>());
            if let Some(dir) = &out {
                std::fs::create_dir_all(dir)?;
                for (p, (kind, v)) in &by_p {
                    let mut dd = d.clone();
                    for (cell, k) in cells.iter().zip(v) {
                        dd.block_entities.remove(cell);
                        if *k > 0 {
                            dd.block_entities.insert(*cell, packed_contents(kind, *k).map_err(|e| anyhow!(e))?);
                        }
                    }
                    let counts: Vec<String> = v.iter().map(|k| packed_label(*k)).collect();
                    dd.name = format!("{}-p{p}-{kind}-{}", d.name, counts.join("-"));
                    dd.meta.insert("period".into(), p.to_string());
                    dd.save(&dir.join(format!("{}.txt", dd.name)))?;
                }
            }
        }
        ItemClockCmd::Promote { designs, name, caption, dir } => {
            for design in designs {
                promote(&design, &name, &caption, &dir)?;
            }
        }
        ItemClockCmd::SweepDropring { max_w, max_n, items, pairs, delays, ticks, jobs, out } => {
            let chains: Vec<Vec<u8>> = delays
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| if s == "-" { Vec::new() } else { s.bytes().map(|b| b - b'0').collect() })
                .collect();
            sweep_dropring(max_w, max_n, &items, pairs, &chains, ticks, jobs, out.as_deref())?;
        }
        ItemClockCmd::SweepRinglock { min_d, max_d, max_row, max_hopper, all, max_items, item, ticks, jobs, out } => {
            use crate::families::{Stack, measure, ring_lock};
            let mut cases: Vec<(i32, i32, bool, usize, u32)> = Vec::new();
            for d in min_d.max(3)..=max_d {
                let l = (2 * d + 2) as usize;
                for zc in 1..=(d - 2).min(max_row) {
                    for rev in [false, true] {
                        for j in 0..l.min(max_hopper + 1) {
                            for n in 1..=max_items {
                                cases.push((d, zc, rev, j, n));
                            }
                        }
                    }
                }
            }
            eprintln!("sweep-ringlock: {} cases", cases.len());
            let next = std::sync::atomic::AtomicUsize::new(0);
            let res: std::sync::Mutex<Vec<((i32, i32, bool, usize, u32), Result<(u32, u32, u32, u32), String>)>> = std::sync::Mutex::new(Vec::new());
            std::thread::scope(|sc| {
                for _ in 0..jobs.max(1) {
                    sc.spawn(|| {
                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            if i >= cases.len() {
                                break;
                            }
                            let (d, zc, rev, j, n) = cases[i];
                            let l = (2 * d + 2) as usize;
                            let mut items: Vec<Vec<Stack>> = vec![Vec::new(); l];
                            items[j].push((item.clone(), n));
                            let r = match ring_lock(d, zc, rev, &items) {
                                Some(des) => measure(&des, ticks),
                                None => Err("no design".into()),
                            };
                            res.lock().unwrap().push((cases[i], r));
                        }
                    });
                }
            });
            let mut res = res.into_inner().unwrap();
            res.sort_by_key(|(c, _)| *c);
            if all {
                for ((d, zc, rev, j, n), r) in &res {
                    match r {
                        Ok((p, whole, on, edges)) => println!("3x{d} row {zc}{} hopper {j} n={n}: P={p} whole {whole} on {on} edges {edges}", if *rev { " rev" } else { "" }),
                        Err(e) => println!("3x{d} row {zc}{} hopper {j} n={n}: {e}", if *rev { " rev" } else { "" }),
                    }
                }
            }
            let mut best: std::collections::BTreeMap<u32, (i32, i32, bool, usize, u32)> = std::collections::BTreeMap::new();
            for (c, r) in &res {
                if let Ok((p, _, _, _)) = r {
                    let e = best.entry(*p).or_insert(*c);
                    // fewest blocks (smallest d), then fewest items
                    if (c.0, c.4) < (e.0, e.4) {
                        *e = *c;
                    }
                }
            }
            println!("{} distinct periods", best.len());
            for (p, (d, zc, rev, j, n)) in &best {
                println!("P={p}: ring 3x{d} ({} blocks), comparator row {zc}{}, {n} {item} in hopper {j}", 2 * d + 2 + 2, if *rev { ", reversed" } else { "" });
            }
            if let Some(dir) = &out {
                std::fs::create_dir_all(dir)?;
                for (p, (d, zc, rev, j, n)) in &best {
                    let l = (2 * d + 2) as usize;
                    let mut items: Vec<Vec<Stack>> = vec![Vec::new(); l];
                    items[*j].push((item.clone(), *n));
                    if let Some(mut des) = ring_lock(*d, *zc, *rev, &items) {
                        des.name = format!("ringlock-3x{d}-r{zc}{}-{n}{item}-h{j}-p{p}", if *rev { "r" } else { "" });
                        des.meta.insert("period".into(), p.to_string());
                        des.save(&dir.join(format!("{}.txt", des.name)))?;
                    }
                }
            }
        }
        ItemClockCmd::SweepLoop { max_w, max_d, max_items, item, ticks, out } => {
            use crate::families::{Stack, hopper_loop, loop_cells, measure};
            let mut seen_p = std::collections::BTreeMap::new();
            println!("w x d  L  items  placement  blocks  P  whole  on  edges");
            for w in 1..=max_w {
                for d in 1..=max_d {
                    let l = loop_cells(w, d).len();
                    if l == 0 || (w > d && d > 1 && false) {
                        continue;
                    }
                    for m in 1..=max_items {
                        for placement in ["stack", "row", "spread"] {
                            let mut items: Vec<Vec<Stack>> = vec![Vec::new(); l];
                            match placement {
                                "stack" => items[0].push((item.clone(), m)),
                                "row" => {
                                    if m as usize > l || m == 1 {
                                        continue;
                                    }
                                    for i in 0..m as usize {
                                        items[i].push((item.clone(), 1));
                                    }
                                }
                                _ => {
                                    if m as usize > l || m == 1 || l % m as usize != 0 {
                                        continue;
                                    }
                                    for i in 0..m as usize {
                                        items[i * l / m as usize].push((item.clone(), 1));
                                    }
                                }
                            }
                            let Some(des) = hopper_loop(w, d, &items) else { continue };
                            match measure(&des, ticks) {
                                Ok((p, whole, on, edges)) => {
                                    println!("{w} x {d}  {l}  {m} {item}  {placement}  {}  {p}  {whole}  {on}  {edges}", des.blocks.len());
                                    if let Some(dir) = &out
                                        && !seen_p.contains_key(&p)
                                    {
                                        std::fs::create_dir_all(dir)?;
                                        let mut des = des.clone();
                                        des.name = format!("loop-{w}x{d}-{m}{item}-{placement}");
                                        des.meta.insert("period".into(), p.to_string());
                                        des.save(&dir.join(format!("{}.txt", des.name)))?;
                                    }
                                    seen_p.entry(p).or_insert(des.blocks.len());
                                }
                                Err(e) => println!("{w} x {d}  {l}  {m} {item}  {placement}  {}  -  {e}", des.blocks.len()),
                            }
                        }
                    }
                }
            }
            println!("periods reached (smallest blocks): {seen_p:?}");
        }
    }
    Ok(())
}
