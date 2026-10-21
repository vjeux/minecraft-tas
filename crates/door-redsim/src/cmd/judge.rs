//! `door cycle`, `door redsim-verify`: the judge on redsim.

use anyhow::Result;
use mctas::design::Design;
use mctas::sim::WorkerConfig;
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct CycleArgs {
    design: PathBuf,
    #[arg(long, default_value_t = 6)]
    flips: usize,
    /// Write the settled state after this many flips (position 0) as a design to --out.
    #[arg(long)]
    write_state: Option<usize>,
    #[arg(long)]
    out: Option<PathBuf>,
    /// Uncertain cells of a transcription: `x,y[,z]=state|state|...` (`air`, `keep`, block
    /// states as in `door ideal-rs --vary`); every combination is cycled and judged.
    #[arg(long)]
    vary: Vec<String>,
    #[arg(long, default_value_t = 16)]
    threads: usize,
    /// With --vary: print only the combinations that cycle.
    #[arg(long)]
    ok_only: bool,
}

pub fn cmd_cycle(args: CycleArgs) -> Result<()> {
    let CycleArgs { design, flips, write_state, out, vary, threads, ok_only } = args;
    crate::cycle::cli(&design, flips, write_state, out.as_deref(), &vary, threads, ok_only)?;
    Ok(())
}

#[derive(clap::Args)]
pub struct RedsimVerifyArgs {
    designs: Vec<PathBuf>,
    #[arg(long, default_value_t = 0)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 3)]
    cycles: usize,
    /// Also run the real judge (same offsets) and compare.
    #[arg(long)]
    judge: bool,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// Skip the "nothing in front of or behind the door" rule (2026-10-18 16:40Z) in both judges:
    /// for designs whose cached verdicts predate it (e.g. the gravity doors' front levers), to
    /// compare the mechanism models alone.
    #[arg(long)]
    legacy_walkway: bool,
}

pub fn cmd_redsim_verify(version: &str, args: RedsimVerifyArgs) -> Result<()> {
    let RedsimVerifyArgs { designs, extra_offsets, cycles, judge, workers, legacy_walkway } = args;
    let empty_walkway = !legacy_walkway;
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    let loaded: Vec<(PathBuf, Design)> = designs.iter().map(|p| Ok((p.clone(), load_design(p)?))).collect::<Result<_>>()?;
    let t0 = std::time::Instant::now();
    let screens: Vec<crate::redsim_screen::Screen> = std::thread::scope(|s| {
        let hs: Vec<_> = loaded
            .chunks(loaded.len().div_ceil(16).max(1))
            .map(|part| s.spawn(|| part.iter().map(|(_, d)| crate::redsim_screen::screen(d, &offsets, cycles, 400, empty_walkway)).collect::<Vec<_>>()))
            .collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    eprintln!("redsim judged {} designs in {:.2}s", loaded.len(), t0.elapsed().as_secs_f64());
    let verdicts: Vec<Option<std::result::Result<(u32, u32), String>>> = if judge {
        let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
        let opts = VerifyOptions { offsets: offsets.clone(), cycles: cycles as u32, empty_walkway, ..Default::default() };
        let sbs: Vec<std::sync::Mutex<Option<u32>>> = (0..pool.len()).map(|_| std::sync::Mutex::new(None)).collect();
        pool.map(loaded.iter().map(|(_, d)| d.clone()).collect(), |w, wi, d| {
            let mut s = sbs[wi].lock().unwrap();
            let sb = match *s {
                Some(sb) => sb,
                None => {
                    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-4, -4, 3, 3], seed: 0 })?;
                    *s = Some(sb);
                    sb
                }
            };
            Ok(match crate::verify_in(w, sb, &d, &opts) {
                Ok(r) => Ok((r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0), r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0))),
                Err(e) => Err(format!("{e:#}")),
            })
        })
        .into_iter()
        .map(|r| Some(r.unwrap_or_else(|e| Err(format!("error: {e:#}")))))
        .collect()
    } else {
        vec![None; loaded.len()]
    };
    let (mut pass, mut fail, mut unsup, mut disagree) = (0, 0, 0, 0);
    for (((p, _), s), v) in loaded.iter().zip(screens.iter()).zip(verdicts.iter()) {
        let line = match s {
            crate::redsim_screen::Screen::Pass { open, close } => {
                pass += 1;
                format!("PASS open {open} close {close}")
            }
            crate::redsim_screen::Screen::Fail(m) => {
                fail += 1;
                format!("FAIL {m}")
            }
            crate::redsim_screen::Screen::Unsupported(m) => {
                unsup += 1;
                format!("UNSUPPORTED {m}")
            }
        };
        let judge_note = match (s, v) {
            (_, None) => String::new(),
            (crate::redsim_screen::Screen::Unsupported(_), Some(_)) => String::new(),
            (crate::redsim_screen::Screen::Pass { .. }, Some(Ok(_))) | (crate::redsim_screen::Screen::Fail(_), Some(Err(_))) => " [judge agrees]".into(),
            (_, Some(Ok((o, c)))) => {
                disagree += 1;
                format!(" [DISAGREE: judge passes, open {o} close {c}]")
            }
            (_, Some(Err(e))) => {
                disagree += 1;
                format!(" [DISAGREE: judge fails: {}]", e.chars().take(160).collect::<String>())
            }
        };
        println!("{}: {}{judge_note}", p.display(), line.chars().take(220).collect::<String>());
    }
    eprintln!("{pass} pass, {fail} fail, {unsup} unsupported{}", if judge { format!(", {disagree} disagreements with the judge") } else { String::new() });
    Ok(())
}
