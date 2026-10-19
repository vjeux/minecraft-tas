//! Contraption study 7, track fly: flying machines (docs/circuits.md, the study brief in
//! designs/circuits/fly/README.md). `mctas circuit fly <command> ...` dispatches here.
//!
//! Modules: [`machine`] (the design format's machine headers), [`track`] (per-tick translation
//! and steady-state tracking, shared by both runners), [`report`] (the verdict), [`sim`] (redsim
//! runner).

pub mod archive;
pub mod census;
pub mod ident;
pub mod machine;
pub mod report;
pub mod sim;
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
    /// Judge designs in the real game and archive those that fly (or shuttle): a copy with a
    /// header in the archive, a verdicts.tsv row, a caption; an archived type with the same class
    /// and speed is refused (SAME TYPE).
    Archive {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = archive::ARCHIVE)]
        dir: PathBuf,
        /// Caption (default: the design's first comment block).
        #[arg(long)]
        caption: Option<String>,
        #[arg(long, default_value_t = 16)]
        workers: usize,
    },
    /// Write the catalog (CATALOG.md + variants.md) from the archive's verdicts (no game).
    Catalog {
        #[arg(long, default_value = archive::ARCHIVE)]
        dir: PathBuf,
        #[arg(long, default_value = archive::CATALOG)]
        out: PathBuf,
        #[arg(long, default_value = archive::VARIANTS)]
        variants: PathBuf,
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
        Cmd::Archive { designs, dir, caption, workers } => {
            let o = verify::VerifyOptions { workers, ..Default::default() };
            archive::archive(&designs, &dir, caption.as_deref(), &o)?;
        }
        Cmd::Catalog { dir, out, variants } => archive::catalog(&dir, &out, &variants)?,
    }
    Ok(())
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
