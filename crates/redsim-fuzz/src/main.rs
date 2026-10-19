//! `redsim-fuzz`: the differential runner as a small binary (fast to rebuild while fixing the
//! model; `mctas door redsim-fuzz` / `door redsim-case` run the same code).

use anyhow::Result;
use clap::{Parser, Subcommand};
use mctas::sim::{Pool, Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(long, default_value = mctas::DEFAULT_VERSION)]
    version: String,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Random circuits in the game and in redsim, every cell compared every tick.
    Fuzz {
        #[arg(long, default_value_t = 1000)]
        n: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value = "6,3,6")]
        size: String,
        #[arg(long, default_value_t = 0.45)]
        density: f64,
        #[arg(long, default_value = "core")]
        palette: String,
        #[arg(long, default_value = "designs/redsim/fails")]
        out: PathBuf,
        #[arg(long)]
        no_shrink: bool,
        #[arg(long, default_value_t = 8)]
        max_report: usize,
        #[arg(long, default_value_t = 8)]
        workers: usize,
        /// Run this many batches of n (seeds seed, seed+1, ...), printing the running total.
        #[arg(long, default_value_t = 1)]
        batches: usize,
    },
    /// Print redsim's block equivalence classes (representative, members, properties).
    Classes,
    /// Prove the classes in the GAME: random circuits and their class-swapped variants must give
    /// the same traces modulo classes.
    Classfuzz {
        #[arg(long, default_value_t = 2000)]
        n: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        #[arg(long, default_value = "6,3,6")]
        size: String,
        #[arg(long, default_value_t = 0.5)]
        density: f64,
        #[arg(long, default_value = "core,plain:14,piston:4,note:4")]
        palette: String,
        #[arg(long, default_value = "designs/redsim/class-fails")]
        out: PathBuf,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Shrink a mismatching case file (redsim vs the game) and print it.
    Shrink {
        case: PathBuf,
    },
    /// Shrink a class violation pair (files from classfuzz) and print the minimal pair.
    Classshrink {
        a: PathBuf,
        b: PathBuf,
    },
    /// Replay every case file of a folder (in parallel); one line per case.
    Cases {
        dir: PathBuf,
        #[arg(long, default_value_t = 8)]
        workers: usize,
    },
    /// Replay a case file in both; print differing frames.
    Case {
        case: PathBuf,
        #[arg(long, default_value_t = 3)]
        max: usize,
        #[arg(long)]
        fixture: Option<PathBuf>,
        #[arg(long, default_value = "")]
        note: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = WorkerConfig { version: cli.version.clone(), heap_mb: 768, ..Default::default() };
    match cli.cmd {
        Cmd::Fuzz { n, seed, size, density, palette, out, no_shrink, max_report, workers, batches } => {
            let sz: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            anyhow::ensure!(sz.len() == 3, "--size x,y,z");
            let pool = Pool::spawn(workers, &cfg)?;
            let (mut total, mut bad) = (0, 0);
            for b in 0..batches {
                let o = redsim_fuzz::FuzzOptions {
                    n,
                    seed: seed + b as u64,
                    size: [sz[0], sz[1], sz[2]],
                    density,
                    palette: redsim_fuzz::palette(&palette)?,
                    out: out.clone(),
                    shrink: !no_shrink,
                    max_report,
                };
                let (t, m) = redsim_fuzz::fuzz(&pool, &o, &|s| eprintln!("{s}"))?;
                total += t;
                bad += m;
                if batches > 1 {
                    eprintln!("running total: {bad} mismatches in {total} cases ({:.4}%)", 100.0 * bad as f64 / total.max(1) as f64);
                }
            }
        }
        Cmd::Classes => {
            let cls = redsim::classes::classes();
            for (i, c) in cls.iter().enumerate() {
                let members: Vec<String> = c.members.iter().map(|b| b.to_string().trim_start_matches("minecraft:").to_string()).collect();
                println!("class {i}: {}\n  rep {}\n  members ({}): {}", c.describe(), c.representative, members.len(), members.join(", "));
            }
            // classes that differ only by the note-block instrument
            let mut groups: Vec<(redsim::classes::Signature, Vec<usize>)> = Vec::new();
            for (i, c) in cls.iter().enumerate() {
                let f = c.instrument_free();
                match groups.iter_mut().find(|(s, _)| *s == f) {
                    Some((_, v)) => v.push(i),
                    None => groups.push((f, vec![i])),
                }
            }
            for (_, v) in groups.iter().filter(|(_, v)| v.len() > 1) {
                println!("same except next to a note block: classes {v:?}");
            }
            eprintln!("{} blocks in {} classes ({} ignoring note-block instruments)", cls.iter().map(|c| c.members.len()).sum::<usize>(), cls.len(), groups.len());
        }
        Cmd::Classfuzz { n, seed, size, density, palette, out, workers } => {
            let sz: Vec<i32> = size.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            let pool = Pool::spawn(workers, &cfg)?;
            redsim_fuzz::classfuzz(&pool, n, seed, &redsim_fuzz::palette(&palette)?, [sz[0], sz[1], sz[2]], density, &out, &|s| eprintln!("{s}"))?;
        }
        Cmd::Shrink { case } => {
            let mut w = Worker::spawn(&cfg)?;
            let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
            let c = redsim_fuzz::Case::parse(&std::fs::read_to_string(&case)?)?;
            let s = redsim_fuzz::shrink(&mut w, sb, c)?;
            print!("{}", s.to_text());
        }
        Cmd::Classshrink { a, b } => {
            let mut w = Worker::spawn(&cfg)?;
            let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
            let ca = redsim_fuzz::Case::parse(&std::fs::read_to_string(&a)?)?;
            let cb = redsim_fuzz::Case::parse(&std::fs::read_to_string(&b)?)?;
            let (sa, sbb) = redsim_fuzz::class_shrink(&mut w, sb, ca, cb)?;
            println!("--- a\n{}--- b\n{}", sa.to_text(), sbb.to_text());
        }
        Cmd::Cases { dir, workers } => {
            let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
            paths.sort();
            let pool = Pool::spawn(workers, &cfg)?;
            let sbs: Vec<std::sync::Mutex<Option<u32>>> = (0..pool.len()).map(|_| std::sync::Mutex::new(None)).collect();
            let res = pool.map(paths.clone(), |w, wi, p| {
                let mut s = sbs[wi].lock().unwrap();
                let sb = match *s {
                    Some(sb) => sb,
                    None => {
                        let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-1, -1, 0, 0], seed: 0 })?;
                        *s = Some(sb);
                        sb
                    }
                };
                let c = redsim_fuzz::Case::parse(&std::fs::read_to_string(&p)?)?;
                let out = redsim_fuzz::check(w, sb, &c)?;
                Ok(match out.diff {
                    None => "identical".to_string(),
                    Some((f, d)) => {
                        let first = d.first().map(|(p, s, g)| format!("{} {} {}: redsim {s} / game {g}", p[0], p[1], p[2])).unwrap_or_default();
                        format!("DIFF {} ({} cells): {first}{}", redsim_fuzz::frame_label(&c, f), d.len(), if out.sim_errors.is_empty() { String::new() } else { format!(" [note: {}]", out.sim_errors[0]) })
                    }
                })
            });
            let mut bad = 0;
            for (p, r) in paths.iter().zip(res) {
                let line = r.unwrap_or_else(|e| format!("error: {e:#}"));
                if !line.starts_with("identical") {
                    bad += 1;
                }
                println!("{}: {line}", p.display());
            }
            eprintln!("{bad} of {} cases differ", paths.len());
        }
        Cmd::Case { case, max, fixture, note } => {
            let mut w = Worker::spawn(&cfg)?;
            let same = redsim_fuzz::replay(&mut w, &case, max)?;
            if let Some(path) = fixture {
                anyhow::ensure!(same || !note.is_empty(), "a fixture of a mismatching case needs a --note");
                let c = redsim_fuzz::Case::parse(&std::fs::read_to_string(&case)?)?;
                let text = redsim_fuzz::fixture(&mut w, &c, &note)?;
                std::fs::write(&path, text)?;
                eprintln!("fixture written to {}", path.display());
            }
        }
    }
    Ok(())
}
