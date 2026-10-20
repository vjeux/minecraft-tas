//! Contraption study 7, track fly: flying machines (docs/circuits.md, the study brief in
//! designs/circuits/fly/README.md). `mctas circuit fly <command> ...` dispatches here.
//!
//! Modules: [`machine`] (the design format's machine headers), [`track`] (per-tick translation
//! and steady-state tracking, shared by both runners), [`report`] (the verdict), [`sim`] (redsim
//! runner).

pub mod census;
pub mod family;
pub mod grow;
pub mod ident;
pub mod machine;
pub mod orient;
pub mod prep;
pub mod report;
pub mod sim;
pub mod station;
pub mod track;
pub mod verify;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit fly", about = "Flying machines (contraption study 7)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run machines in redsim (milliseconds): place like the judge, kick, track the translation
    /// every gt; prints FLIES / SHUTTLES / MOVES / STILL / BROKEN with the steady speed (gt per
    /// block = P/d), the first move, the end state.
    Sim {
        designs: Vec<PathBuf>,
        /// Ticks to run at most (0 = 200 + 60 gt per corridor cell).
        #[arg(long, default_value_t = 0)]
        horizon: u32,
        /// Corridor length for designs without `distance:`.
        #[arg(long, default_value_t = 32)]
        distance: i32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        /// Print every cell change of the first N ticks.
        #[arg(long, default_value_t = 0)]
        trace: u32,
        /// Print the translations the machine takes (tick: shift).
        #[arg(long)]
        visits: bool,
    },
    /// The real-game check: every build (4 positions x 4 rotations) in a corridor sandbox,
    /// tracked like `sim`; prints the base build's line, the builds that differ, and whether
    /// redsim agrees. Exits 1 unless every build flies (or shuttles) the same way.
    Verify {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        positions: usize,
        #[arg(long)]
        no_rotations: bool,
        #[arg(long, default_value_t = 0)]
        horizon: u32,
        #[arg(long, default_value_t = 32)]
        distance: i32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
        #[arg(long, default_value_t = 16)]
        workers: usize,
        /// Print every build's line.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Exhaustive census on redsim: every face-connected filling of the box with kmin..=k blocks
    /// of the palette (one per rotation class), kicked once per observer; writes DIR/found.tsv and
    /// one design per type to DIR/designs/.
    Census {
        /// Box AxBxC (x, y, z), or several: 4x2x1,2x2x2.
        #[arg(long = "box")]
        dims: String,
        #[arg(long)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        kmin: usize,
        /// Palette shorthands (clock census names) or block states.
        #[arg(long, default_value = "slime,honey,stone,observer,piston,sticky")]
        palette: String,
        #[arg(long, default_value_t = 3)]
        max_pistons: usize,
        #[arg(long, default_value_t = 3)]
        max_observers: usize,
        #[arg(long, default_value_t = 160)]
        horizon: u32,
        #[arg(long, default_value_t = 6)]
        margin: i32,
        #[arg(long, default_value_t = 96)]
        workers: usize,
        /// Keep observers whose output feeds nothing.
        #[arg(long)]
        loose: bool,
        #[arg(long)]
        out: PathBuf,
        /// Split the work into this many child processes (each `--workers` threads).
        #[arg(long, default_value_t = 1)]
        procs: usize,
        /// (internal) this process's share: k/n.
        #[arg(long)]
        shard: Option<String>,
    },
    /// Type identity on redsim (normal form by verified block swaps, then rotation + mirror),
    /// the dominance fold (a design with more blocks whose kinds include a smaller design's of the
    /// same kind and speed is a variant): one line per design, hosts first. `--copy-hosts DIR`
    /// copies one design per host type there.
    Types {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long)]
        copy_hosts: Option<PathBuf>,
    },
    /// Get census finds ready for the archive: a readable name (fly-<class>-<blocks>b-<speed>gt-
    /// <type hash>) and a caption (what faces where) as the first comment block, written to DIR;
    /// then `mctas circuit archive` keeps them.
    Prep {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: PathBuf,
        /// Skip designs whose prepared name (its type) is already in this archive directory.
        #[arg(long)]
        skip_archived: Option<PathBuf>,
        /// Keep only the first design of each (class, blocks, speed, caption).
        #[arg(long)]
        one_per_caption: bool,
    },
    /// Every one of the 24 orientations of each design on redsim (tilted to fly up or down,
    /// rolled about the travel axis): one line per orientation that flies; `--out-dir` writes the
    /// first design of each new class (up, down, horizontal) found.
    Orient {
        designs: Vec<PathBuf>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Add 1 or 2 blocks around each design (every empty cell next to it, every palette state),
    /// run each candidate in every quarter turn on redsim with the design's kick; print and write
    /// the ones that fly at ANOTHER speed than their base.
    Grow {
        designs: Vec<PathBuf>,
        #[arg(long, default_value_t = 1)]
        add: usize,
        #[arg(long, default_value = "slime,honey,stone,observer,piston,sticky")]
        palette: String,
        #[arg(long, default_value_t = 160)]
        horizon: u32,
        #[arg(long, default_value_t = 6)]
        margin: i32,
        #[arg(long, default_value_t = 32)]
        workers: usize,
        #[arg(long)]
        out_dir: PathBuf,
        /// Designs written per (speed, class, blocks).
        #[arg(long, default_value_t = 1000)]
        keep: usize,
    },
    /// Generators for extendable families: `gen payload --base <design> --from x,y,z --dir up
    /// --n N [--block <state>] --out f` (a payload column of N blocks, slime, the last one
    /// `--block`, growing from the cell next to `from`).
    Gen {
        family: String,
        #[arg(long)]
        base: PathBuf,
        #[arg(long)]
        from: String,
        #[arg(long, default_value = "up")]
        dir: String,
        #[arg(long)]
        n: usize,
        #[arg(long, default_value = "minecraft:slime_block")]
        block: String,
        /// The first column holds at most this many blocks; the rest go in the second column.
        #[arg(long, default_value_t = 9)]
        cap: usize,
        /// The second column's start cell (a block of the other part).
        #[arg(long)]
        from2: Option<String>,
        #[arg(long, default_value = "down")]
        dir2: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Two-way machines: for each reversible machine (kicked from its back-facing sticky piston it
    /// flies backward), the station search ahead and behind, then every pair of the best stations
    /// (`--pairs` each) composed into one design (`port station.<n>`), kept when it shuttles.
    TwoWay {
        machines: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        approach: i32,
        #[arg(long, default_value_t = 3)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        radius: i32,
        #[arg(long, default_value = "obsidian,stone,observer,repeater1,minecraft:redstone_wire[east=side,north=side,south=side,west=side]")]
        palette: String,
        #[arg(long, default_value_t = 300)]
        horizon: u32,
        #[arg(long, default_value_t = 12)]
        workers: usize,
        #[arg(long, default_value_t = 8)]
        procs: usize,
        #[arg(long, default_value_t = 2)]
        pairs: usize,
        /// Only say which machines fly backward when kicked from their other end (no search).
        #[arg(long)]
        reversible_only: bool,
        #[arg(long)]
        out_dir: PathBuf,
    },
    /// Return-station search on redsim: the machine flies `approach` cells toward an obsidian
    /// wall (every wall cell in front of its leading face) with up to k more station blocks
    /// around its stopping place; keeps the stations that send it back `back` cells intact.
    Station {
        machine: PathBuf,
        #[arg(long, default_value_t = 4)]
        approach: i32,
        #[arg(long, default_value_t = 2)]
        k: usize,
        #[arg(long, default_value_t = 1)]
        radius: i32,
        #[arg(long, default_value = "obsidian,stone,observer,repeater,minecraft:redstone_wire[east=side,north=side,south=side,west=side],rblock,torch")]
        palette: String,
        #[arg(long, default_value_t = 300)]
        horizon: u32,
        #[arg(long, default_value_t = 4)]
        back: i32,
        #[arg(long, default_value_t = 48)]
        workers: usize,
        #[arg(long)]
        out_dir: PathBuf,
        /// Designs to write (the best first).
        #[arg(long, default_value_t = 20)]
        keep: usize,
        /// Child processes (each `--workers` threads).
        #[arg(long, default_value_t = 1)]
        procs: usize,
        /// (internal) this process's share: k/n.
        #[arg(long)]
        shard: Option<String>,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let argv: Vec<String> = args.clone();
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit fly".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Sim { designs, horizon, distance, margin, trace, visits } => {
            if designs.is_empty() {
                bail!("circuit fly sim <design>...");
            }
            let o = sim::SimOptions { horizon, distance, margin, trace, idle_grace: 0 };
            for path in &designs {
                let m = machine::Machine::load(path)?;
                let mut log = Vec::new();
                let r = sim::simulate_logged(&m, &o, &mut log);
                for l in &log {
                    println!("{l}");
                }
                println!("{}", r.summary());
                if visits {
                    for (t, s) in &r.visits {
                        println!("  t{t}: {},{},{}", s[0], s[1], s[2]);
                    }
                }
            }
        }
        Cmd::Verify { designs, positions, no_rotations, horizon, distance, margin, workers, verbose } => {
            if designs.is_empty() {
                bail!("circuit fly verify <design>...");
            }
            let o = verify::VerifyOptions { positions, rotations: !no_rotations, horizon, distance, margin, workers, ..Default::default() };
            let pool = verify::spawn_pool(&o)?;
            let mut all_ok = true;
            for path in &designs {
                let m = machine::Machine::load(path)?;
                let rep = verify::verify_in_pool(&pool, &m, &o)?;
                println!("{}", rep.summary());
                let base_sig = rep.builds.first().map(|b| (b.report.verdict, b.report.speed_text())).unwrap();
                for b in &rep.builds {
                    if verbose || (b.report.verdict, b.report.speed_text()) != base_sig {
                        println!("  rotation {} at {:?}: {}", b.turns, b.offset, b.report.summary());
                    }
                }
                let rs = sim::simulate(&m, &sim::SimOptions { horizon, distance, margin, trace: 0, idle_grace: 0 });
                let agree = rs.verdict == rep.base.verdict && rs.steady.as_ref().map(|s| (s.period, s.d)) == rep.base.steady.as_ref().map(|s| (s.period, s.d)) && rs.first_change == rep.base.first_change && rs.final_shift == rep.base.final_shift;
                println!("  redsim {}", if agree { "AGREES".to_string() } else { format!("DIFFERS: {}", rs.summary()) });
                all_ok &= rep.ok();
            }
            if !all_ok {
                std::process::exit(1);
            }
        }
        Cmd::Census { dims, k, kmin, palette, max_pistons, max_observers, horizon, margin, workers, loose, out, procs, shard } => {
            let mut boxes: Vec<[i32; 3]> = Vec::new();
            for b in dims.split(',') {
                let d: Vec<i32> = b.split('x').map(|s| s.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().map_err(|e| anyhow::anyhow!("--box AxBxC[,AxBxC...]: {e}"))?;
                if d.len() != 3 {
                    bail!("--box AxBxC[,AxBxC...]");
                }
                boxes.push([d[0], d[1], d[2]]);
            }
            let shard = match shard {
                Some(s) => {
                    let (a, b) = s.split_once('/').ok_or_else(|| anyhow::anyhow!("--shard k/n"))?;
                    (a.parse::<usize>()?, b.parse::<usize>()?)
                }
                None => (0, 1),
            };
            let o = census::CensusOptions { dims: boxes[0], k, kmin, palette: clock::census::expand_palette(&palette)?, max_pistons, max_observers, horizon, margin, workers, out, loose, shard, procs };
            census::run_census(&o, &boxes, &argv)?;
        }
        Cmd::Types { designs, workers, copy_hosts } => types(&designs, workers, copy_hosts.as_deref())?,
        Cmd::Prep { designs, out_dir, skip_archived, one_per_caption } => {
            std::fs::create_dir_all(&out_dir)?;
            let archived: Vec<String> = match &skip_archived {
                Some(d) => std::fs::read_dir(d)?.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().trim_end_matches(".txt").to_string()).collect(),
                None => Vec::new(),
            };
            let mut seen: std::collections::BTreeSet<(String, String)> = std::collections::BTreeSet::new();
            for p in &designs {
                let text = std::fs::read_to_string(p)?;
                let m = machine::Machine::parse(&text)?;
                let (name, out) = prep::prep(&m, &text);
                if archived.iter().any(|a| a == &name || a.starts_with(&format!("{name}-"))) {
                    println!("{} -> {name} (archived already)", p.display());
                    continue;
                }
                if one_per_caption {
                    let prefix = name.rsplit_once('-').map(|(a, _)| a.to_string()).unwrap_or_default();
                    let cap = out.lines().next().unwrap_or("").to_string();
                    if !seen.insert((prefix, cap)) {
                        continue;
                    }
                }
                std::fs::write(out_dir.join(format!("{name}.txt")), out)?;
                println!("{} -> {name}", p.display());
            }
        }
        Cmd::Orient { designs, out_dir } => {
            for p in &designs {
                let m = machine::Machine::load(p)?;
                let base = class_of(&sim::simulate(&m, &sim::SimOptions { distance: 16, ..Default::default() }));
                let mut written: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                written.insert(base.clone());
                for o in orient::orient(&m) {
                    let class = class_of(&o.report);
                    if !matches!(o.report.verdict, report::Verdict::Flies | report::Verdict::Shuttles) {
                        continue;
                    }
                    let expect = m.travel.map(|t| orient::apply(&orient::rotations()[o.index], t));
                    let way = match (expect, o.report.direction()) {
                        (Some(e), Some(d)) if e == d => "forward",
                        (Some(e), Some(d)) if e == [-d[0], -d[1], -d[2]] => "BACKWARD",
                        (_, Some(_)) => "sideways",
                        _ => "-",
                    };
                    println!("{} o{}: {} {} {way}", m.name(), o.index, class, o.report.speed_text());
                    if let Some(dir) = &out_dir
                        && written.insert(class.clone())
                    {
                        std::fs::create_dir_all(dir)?;
                        let mut mm = o.machine.clone();
                        mm.clock.design.name = format!("{}-o{}", m.name(), o.index);
                        mm.travel = o.report.direction();
                        if let Some(t) = mm.travel {
                            mm.clock.design.meta.insert("travel".into(), format!("{} {} {}", t[0], t[1], t[2]));
                        }
                        std::fs::write(dir.join(format!("{}.txt", mm.name())), mm.to_text())?;
                    }
                }
            }
        }
        Cmd::Grow { designs, add, palette, horizon, margin, workers, out_dir, keep } => {
            let ms: Vec<machine::Machine> = designs.iter().map(|p| machine::Machine::load(p)).collect::<Result<_>>()?;
            let o = grow::GrowOptions { add, palette: clock::census::expand_palette(&palette)?, horizon, margin, workers };
            let found = grow::grow(&ms, &o)?;
            std::fs::create_dir_all(&out_dir)?;
            let mut seen: std::collections::BTreeMap<(String, String, usize), usize> = std::collections::BTreeMap::new();
            for g in &found {
                let key = (g.speed(), g.class().to_string(), g.cells.len());
                let k = seen.entry(key.clone()).or_insert(0);
                if *k < keep {
                    let name = format!("grow-{}b-{}gt-{}-{}", g.cells.len(), g.speed().replace('/', "_"), g.class(), *k);
                    std::fs::write(out_dir.join(format!("{name}.txt")), grow::design_text(g, &name))?;
                }
                *k += 1;
            }
            for ((speed, class, blocks), n) in &seen {
                println!("{blocks} blocks, {speed} gt/block, {class}: {n} candidates");
            }
        }
        Cmd::Gen { family, base, from, dir, n, block, cap, from2, dir2, out } => {
            let pos = |s: &str| -> Result<clock::Pos> {
                let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>()?;
                if v.len() != 3 {
                    bail!("x,y,z expected, got {s:?}");
                }
                Ok([v[0], v[1], v[2]])
            };
            let block = mctas::blockstate::BlockState::parse(&block)?.to_string();
            let second = match &from2 {
                Some(s) => Some((pos(s)?, dir2.as_str())),
                None => None,
            };
            let m = match family.as_str() {
                "payload" => family::payload(&base, (pos(&from)?, dir.as_str()), second, cap, n, &block)?,
                f => bail!("unknown family {f:?} (payload)"),
            };
            std::fs::write(&out, m.to_text())?;
        }
        Cmd::Station { machine, approach, k, radius, palette, horizon, back, workers, out_dir, keep, procs, shard } => {
            let m = machine::Machine::load(&machine)?;
            let shard = match shard {
                Some(s) => {
                    let (a, b) = s.split_once('/').ok_or_else(|| anyhow::anyhow!("--shard k/n"))?;
                    (a.parse::<usize>()?, b.parse::<usize>()?)
                }
                None => (0, 1),
            };
            let o = station::StationOptions { approach, k, radius, palette: clock::census::expand_palette(&palette)?, horizon, workers, back, shard };
            std::fs::create_dir_all(&out_dir)?;
            let found: Vec<station::Station> = if procs > 1 && shard.1 <= 1 {
                station_search_procs(&machine, &o, &palette, procs, &out_dir)?
            } else {
                let f = station::search(&m, &o)?;
                if shard.1 > 1 {
                    let raw: String = f.iter().map(|s| station::station_line(s) + "\n").collect();
                    std::fs::write(out_dir.join("raw.tsv"), raw)?;
                    return Ok(());
                }
                f
            };
            println!("{} stations found", found.len());
            for (i, s) in found.iter().take(keep).enumerate() {
                let name = format!("{}-st{}-{i}", m.name(), s.blocks.len());
                let blocks: Vec<String> = s.blocks.iter().map(|(p, b)| format!("{},{},{} {}", p[0], p[1], p[2], clock::short(&b.to_string()))).collect();
                println!("{name}: wall t{}, back t{}: {}", s.at_wall, s.back_at, blocks.join("; "));
                std::fs::write(out_dir.join(format!("{name}.txt")), station::design_text(&m, s, &name))?;
            }
        }
        Cmd::TwoWay { machines, approach, k, radius, palette, horizon, workers, procs, out_dir, pairs, reversible_only } => {
            std::fs::create_dir_all(&out_dir)?;
            let o = station::StationOptions { approach, k, radius, palette: clock::census::expand_palette(&palette)?, horizon, workers, back: 4, shard: (0, 1) };
            for path in &machines {
                let m = machine::Machine::load(path)?;
                let Some(rev) = station::reversed(&m) else {
                    println!("{}: no back-facing sticky piston to kick: not reversible", m.name());
                    continue;
                };
                let dir = out_dir.join(m.name());
                std::fs::create_dir_all(&dir)?;
                let rev_path = dir.join(format!("{}-rev.txt", m.name()));
                std::fs::write(&rev_path, rev.to_text())?;
                // does it fly backward at all?
                let rr = sim::simulate(&rev, &sim::SimOptions { distance: 8, ..Default::default() });
                let back = m.travel.map(|t| [-t[0], -t[1], -t[2]]);
                if rr.direction() != back {
                    println!("{}: kicked from the other piston it does not fly backward ({})", m.name(), rr.verdict.word());
                    continue;
                }
                if reversible_only {
                    println!("{}: reversible ({} backward: {})", m.name(), rr.speed_text(), rev_path.display());
                    continue;
                }
                let fwd = station_search_procs(path, &o, &palette, procs, &dir.join("fwd"))?;
                let bwd = station_search_procs(&rev_path, &o, &palette, procs, &dir.join("bwd"))?;
                println!("{}: {} forward stations, {} backward stations", m.name(), fwd.len(), bwd.len());
                let mut made = 0;
                'pairs: for f in fwd.iter().take(pairs) {
                    for b in bwd.iter().take(pairs) {
                        let name = format!("fly-two-way-{}-{made}", m.name().trim_start_matches("fly-"));
                        let mut mm = m.clone();
                        for (p, blk) in f.blocks.iter().chain(&b.blocks) {
                            if mm.stops.iter().any(|(q, _)| q == p) {
                                continue 'pairs;
                            }
                            mm.stops.push((*p, blk.to_string()));
                            mm.stations.insert(*p);
                        }
                        mm.clock.design.name = name.clone();
                        let r = sim::simulate(&mm, &sim::SimOptions { horizon: 800, ..Default::default() });
                        println!("  {name}: {}", r.summary());
                        if r.verdict == report::Verdict::Shuttles {
                            std::fs::write(out_dir.join(format!("{name}.txt")), mm.to_text())?;
                            made += 1;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// The station search in `procs` child processes (each `o.workers` threads, a share of the jobs),
/// merged, best first.
fn station_search_procs(machine: &std::path::Path, o: &station::StationOptions, palette: &str, procs: usize, out_dir: &std::path::Path) -> Result<Vec<station::Station>> {
    std::fs::create_dir_all(out_dir)?;
    // a rebuilt binary leaves the running one as "<path> (deleted)": spawn the new one
    let exe = std::path::PathBuf::from(std::env::current_exe()?.display().to_string().trim_end_matches(" (deleted)"));
    let mut children = Vec::new();
    for i in 0..procs.max(1) {
        let dir = out_dir.join(format!("shard-{i}"));
        let args: Vec<String> = vec![
            "circuit".into(),
            "fly".into(),
            "station".into(),
            machine.display().to_string(),
            "--approach".into(),
            o.approach.to_string(),
            "--k".into(),
            o.k.to_string(),
            "--radius".into(),
            o.radius.to_string(),
            "--palette".into(),
            palette.to_string(),
            "--horizon".into(),
            o.horizon.to_string(),
            "--back".into(),
            o.back.to_string(),
            "--workers".into(),
            o.workers.to_string(),
            "--shard".into(),
            format!("{i}/{}", procs.max(1)),
            "--out-dir".into(),
            dir.display().to_string(),
        ];
        children.push((std::process::Command::new(&exe).args(&args).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn()?, dir));
    }
    let mut found = Vec::new();
    for (mut c, dir) in children {
        if !c.wait()?.success() {
            bail!("station shard {} failed", dir.display());
        }
        for l in std::fs::read_to_string(dir.join("raw.tsv"))?.lines() {
            found.push(station::parse_station(l)?);
        }
    }
    found.sort_by_key(|s: &station::Station| (s.blocks.len(), s.back_at));
    Ok(found)
}

/// One design's identity facts.
struct TypeRow {
    path: PathBuf,
    name: String,
    blocks: usize,
    kinds: std::collections::BTreeSet<String>,
    class: String,
    speed: String,
    key: String,
    swaps: Vec<String>,
}

/// The kind class of a run: `one-way +x`-style direction classes fold to horizontal / up / down.
pub(crate) fn class_of(r: &report::FlyReport) -> String {
    match (r.verdict, r.direction()) {
        (report::Verdict::Flies, Some([_, 1, _])) => "up".into(),
        (report::Verdict::Flies, Some([_, -1, _])) => "down".into(),
        (report::Verdict::Flies, Some(_)) => "horizontal".into(),
        (report::Verdict::Shuttles, _) => "two-way".into(),
        (v, _) => v.word().to_lowercase(),
    }
}

fn types(designs: &[PathBuf], workers: usize, copy_hosts: Option<&std::path::Path>) -> Result<()> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let rows: std::sync::Mutex<Vec<TypeRow>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..workers.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= designs.len() {
                        break;
                    }
                    let Ok(m) = machine::Machine::load(&designs[i]) else { continue };
                    let r = sim::simulate(&m, &sim::SimOptions { distance: 16, ..Default::default() });
                    let (n, swaps) = ident::normalized(&m);
                    let key = clock::canon::type_key(n.design().blocks.iter());
                    rows.lock().unwrap().push(TypeRow { path: designs[i].clone(), name: m.name().to_string(), blocks: m.blocks(), kinds: n.clock.kinds(), class: class_of(&r), speed: r.speed_text(), key, swaps });
                }
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|a, b| (a.class.clone(), a.speed.clone(), a.blocks, a.name.clone()).cmp(&(b.class.clone(), b.speed.clone(), b.blocks, b.name.clone())));
    // one row per type (the first = fewest blocks), then the dominance fold
    let mut seen = std::collections::HashSet::new();
    let uniq: Vec<&TypeRow> = rows.iter().filter(|r| seen.insert((r.class.clone(), r.speed.clone(), r.key.clone()))).collect();
    let host_of = |r: &TypeRow| uniq.iter().find(|h| h.class == r.class && h.speed == r.speed && h.blocks < r.blocks && h.kinds.is_subset(&r.kinds)).map(|h| h.name.clone());
    let mut hosts = 0;
    for r in &uniq {
        let host = host_of(r);
        if host.is_none() {
            hosts += 1;
            if let Some(dir) = copy_hosts {
                std::fs::create_dir_all(dir)?;
                std::fs::copy(&r.path, dir.join(r.path.file_name().unwrap()))?;
            }
        }
        let kinds = r.kinds.iter().cloned().collect::<Vec<_>>().join("+");
        let h = {
            use std::hash::{Hash, Hasher};
            let mut hh = std::collections::hash_map::DefaultHasher::new();
            r.key.hash(&mut hh);
            hh.finish() & 0xffffff
        };
        println!("{}\t{}\t{} blocks\t{}\ttype {h:06x}\t{}\t{}\t{}", r.class, r.speed, r.blocks, kinds, r.name, host.map(|h| format!("variant of {h}")).unwrap_or_else(|| "HOST".into()), r.swaps.join("; "));
    }
    println!("{} designs, {} types, {} hosts", rows.len(), uniq.len(), hosts);
    Ok(())
}
