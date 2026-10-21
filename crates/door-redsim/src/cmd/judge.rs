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
pub struct MaterialArgs {
    designs: Vec<PathBuf>,
    /// Write each changed design here (same file name: its original comment lines, then one
    /// material line) and material.tsv.
    #[arg(long)]
    out: PathBuf,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 3)]
    cycles: usize,
    #[arg(long, default_value_t = 32)]
    threads: usize,
    /// Also judge every changed design in the real game (the same offsets and cycles).
    #[arg(long)]
    judge: bool,
    #[arg(long, default_value_t = 8)]
    workers: usize,
    /// Skip the "nothing in front of or behind the door" rule (as `redsim-verify
    /// --legacy-walkway`): for archived designs that predate it.
    #[arg(long)]
    legacy_walkway: bool,
}

/// `door material`: obsidian -> stone wherever the door still passes with the same ticks
/// (piston-door material.rs), on redsim; `--judge` confirms the changed designs in the game.
pub fn cmd_material(version: &str, args: MaterialArgs) -> Result<()> {
    let MaterialArgs { designs, out, extra_offsets, cycles, threads, judge, workers, legacy_walkway } = args;
    let mut offsets = VerifyOptions::default().offsets;
    offsets.extend(crate::extra_offsets(extra_offsets));
    let loaded: Vec<(PathBuf, Design, String)> = designs
        .iter()
        .filter_map(|p| {
            let text = std::fs::read_to_string(p).ok()?;
            text.contains("obsidian").then_some(())?;
            load_design(p).ok().map(|d| (p.clone(), d, text))
        })
        .collect();
    eprintln!("{} designs with obsidian", loaded.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Option<piston_door::material::Material>>> = std::sync::Mutex::new((0..loaded.len()).map(|_| None).collect());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= loaded.len() {
                    break;
                }
                let m = piston_door::material::obsidian_pass(&loaded[i].1, &offsets, cycles, 400, !legacy_walkway);
                results.lock().unwrap()[i] = Some(m);
            });
        }
    });
    let results: Vec<piston_door::material::Material> = results.into_inner().unwrap().into_iter().map(|m| m.unwrap()).collect();
    std::fs::create_dir_all(&out)?;
    let mut changed: Vec<(usize, Design)> = Vec::new();
    for (i, m) in results.iter().enumerate() {
        if let Some(d) = &m.design {
            changed.push((i, d.clone()));
        }
    }
    // the game on the changed designs
    let mut game: Vec<String> = vec![String::new(); loaded.len()];
    if judge && !changed.is_empty() {
        let pool = mctas::sim::Pool::spawn(workers, &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
        let opts = VerifyOptions { offsets: offsets.clone(), cycles: cycles as u32, empty_walkway: !legacy_walkway, ..Default::default() };
        let sbs: Vec<std::sync::Mutex<Option<u32>>> = (0..pool.len()).map(|_| std::sync::Mutex::new(None)).collect();
        let verdicts = pool.map(changed.iter().map(|(_, d)| d.clone()).collect(), |w, wi, d| {
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
                Ok(r) => format!("WORKS open {} close {}", r.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0), r.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0)),
                Err(e) => format!("FAILS {}", format!("{e:#}").chars().take(160).collect::<String>()),
            })
        });
        for ((i, _), v) in changed.iter().zip(verdicts) {
            game[*i] = v.unwrap_or_else(|e| format!("error: {e:#}"));
        }
    }
    let mut tsv = String::from("design\tobsidian\tto stone\tkept\tredsim\tgame\n");
    let (mut n_changed, mut n_stone, mut n_kept, mut n_ok) = (0, 0, 0, 0);
    for (i, ((p, _, text), m)) in loaded.iter().zip(&results).enumerate() {
        let name = p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let base = match &m.base {
            Ok((o, c)) => format!("PASS open {o} close {c}"),
            Err(e) => format!("not judged: {}", e.chars().take(120).collect::<String>()),
        };
        let kept = m.obsidian.len() - m.replaced.len();
        if m.base.is_ok() {
            n_kept += kept;
        }
        if let Some(d) = &m.design {
            n_changed += 1;
            n_stone += m.replaced.len();
            if game[i].starts_with("WORKS") {
                n_ok += 1;
            }
            // the original's comment lines, then why the rest stays obsidian
            let comments: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
            let why = if kept == 0 { "no obsidian left".to_string() } else { format!("the other {kept} obsidian stay: pistons cannot move obsidian, and the door fails or changes timing with stone there") };
            std::fs::write(out.join(&name), format!("{comments}# material (clk-rt 2026-10-21, `door material`): {} obsidian -> stone (the door passes with the same ticks); {why}\n{}", m.replaced.len(), d.to_text()))?;
        }
        tsv += &format!("{name}\t{}\t{}\t{kept}\t{base}\t{}\n", m.obsidian.len(), m.replaced.len(), game[i]);
    }
    std::fs::write(out.join("material.tsv"), tsv)?;
    println!("{} designs with obsidian: {n_changed} changed ({n_stone} obsidian -> stone), {n_kept} obsidian kept (needed: pistons cannot move it){}", loaded.len(), if judge { format!("; game: {n_ok}/{n_changed} changed designs WORK") } else { String::new() });
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
