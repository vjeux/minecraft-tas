//! `mctas clock obs at-periods`: the archived extendable clock families at the periods players ask
//! for (1 s, 2 s, 5 s, 10 s, 30 s, 1 min, 2 min, 5 min, 10 min by default): every family whose
//! period formula gives exactly that period at an integer n (from its low end on, inside or past
//! its judged range), the member's block count, and with `--sim` the member built with the
//! family's own generator and its exact period on redsim (`clock sim`). The members go in
//! `--out-dir` for `clock extend-check --n` / `clock archive` (the game).

use anyhow::Result;
use clap::Args as ClapArgs;
use clock::extend::{Family, Formula};
use clock::{Clock, gt_time};
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct AtPeriodsArgs {
    /// Periods in gt, comma-separated.
    #[arg(long, default_value = "20,40,100,200,600,1200,2400,6000,12000")]
    pub periods: String,
    /// The clock archive (the family bases: `extend family:` designs).
    #[arg(long, default_value = "designs/clocks/archive")]
    pub dir: PathBuf,
    /// Build each member with its family's generator and run it on redsim.
    #[arg(long)]
    pub sim: bool,
    /// Where the built members go (with --sim).
    #[arg(long, default_value = "/tmp/clock-at-periods")]
    pub out_dir: PathBuf,
    /// Skip members with more blocks than this.
    #[arg(long, default_value_t = 400)]
    pub max_blocks: i64,
    /// Judge in the game (`clock verify`) the K smallest members per period that pass redsim.
    #[arg(long, default_value_t = 0)]
    pub verify: usize,
    /// Record the members the game confirms in the archive's extend.tsv (family, n, period,
    /// blocks, type, verdict).
    #[arg(long)]
    pub record: bool,
    /// Only these periods' members are judged in the game (default: every period).
    #[arg(long)]
    pub verify_periods: Option<String>,
}

/// The n >= `lo` with `f(n) == p`, if any.
pub fn solve(f: &Formula, lo: i64, p: i64) -> Option<i64> {
    match *f {
        Formula::Linear { a, b } => {
            if b == 0 {
                return (a == p).then_some(lo);
            }
            let d = p - a;
            (d % b == 0 && d / b >= lo).then(|| d / b)
        }
        Formula::Geometric { .. } => (lo..lo + 40).find(|n| f.eval(*n) == p).filter(|_| true),
    }
}

pub fn run(a: AtPeriodsArgs) -> Result<()> {
    let periods: Vec<i64> = a.periods.split(',').map(|s| s.trim().parse::<i64>()).collect::<std::result::Result<_, _>>()?;
    let mut fams: Vec<(PathBuf, Family)> = Vec::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&a.dir)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    entries.sort();
    for p in entries {
        let Ok(c) = Clock::load(&p) else { continue };
        if let Ok(Some(f)) = Family::of(&c) {
            fams.push((p, f));
        }
    }
    println!("{} families in {}", fams.len(), a.dir.display());
    let mut passed: Vec<(i64, i64, String, i64, PathBuf)> = Vec::new();
    for &p in &periods {
        let mut rows: Vec<(i64, String, i64, bool, PathBuf)> = Vec::new();
        for (path, f) in &fams {
            if let Some(n) = solve(&f.period, f.n_lo, p) {
                rows.push((f.blocks.eval(n), f.id.clone(), n, n <= f.n_hi, path.clone()));
            }
        }
        rows.sort();
        println!("== {} ({p} gt): {} families", gt_time(p as u32), rows.len());
        for (blocks, id, n, inside, path) in rows {
            let range = if inside { "inside its range" } else { "PAST its range" };
            if blocks > a.max_blocks {
                println!("  {id} n = {n}: {blocks} blocks ({range}), skipped (> --max-blocks)");
                continue;
            }
            if !a.sim {
                println!("  {id} n = {n}: {blocks} blocks ({range})");
                continue;
            }
            let f = &fams.iter().find(|(_, f)| f.id == id).expect("family").1;
            let line = match f.build(&path, n, &a.out_dir) {
                Ok(out) => match retapped(&out, p) {
                    Ok((ok, line)) => {
                        if ok {
                            passed.push((p, blocks, id.clone(), n, out.clone()));
                        }
                        format!("{} {line}, {}", if ok { "OK  " } else { "FAIL" }, out.display())
                    }
                    Err(e) => format!("FAIL {e:#}"),
                },
                Err(e) => format!("FAIL build: {e:#}"),
            };
            println!("  {id} n = {n} ({range}): {line}");
        }
    }
    if a.verify == 0 {
        return Ok(());
    }
    let only: Option<Vec<i64>> = a.verify_periods.as_ref().map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
    let mut picked: Vec<(i64, i64, String, i64, PathBuf)> = Vec::new();
    for &p in &periods {
        if only.as_ref().is_some_and(|o| !o.contains(&p)) {
            continue;
        }
        let mut v: Vec<_> = passed.iter().filter(|x| x.0 == p).cloned().collect();
        v.sort_by_key(|x| x.1);
        picked.extend(v.into_iter().take(a.verify));
    }
    let o = clock::verify::VerifyOptions::default();
    let pool = clock::verify::spawn_pool(&o)?;
    let mut rows = Vec::new();
    for (p, blocks, id, n, path) in picked {
        let c = Clock::load(&path)?;
        let rep = clock::verify::verify_in_pool(&pool, &c, &o)?;
        let line = rep.summary();
        let ok = rep.verdict == clock::sim::Verdict::Clock && rep.base().is_some_and(|b| b.period as i64 == p);
        println!("{} {} ({id} n = {n}, {blocks} blocks): {line}", if ok { "GAME OK  " } else { "GAME FAIL" }, gt_time(p as u32));
        if ok {
            rows.push(clock::extend::Instance { family: id, n, period: p as u32, blocks: c.blocks(), type_key: clock::ident::type_key(&c), verdict: rep.robustness.clone() });
        }
    }
    if a.record && !rows.is_empty() {
        clock::extend::record(&a.dir, &rows)?;
        println!("recorded {} members in {}", rows.len(), a.dir.join(clock::extend::EXTEND_TSV).display());
    }
    Ok(())
}

/// The member at `out` with its output moved to an exterior air cell where the judge's lamp lights
/// evenly (the clock judge's output rule; the generators predate it), written back, and its period
/// on redsim: (period == `p`, the line).
fn retapped(out: &std::path::Path, p: i64) -> Result<(bool, String)> {
    let c = Clock::load(out)?;
    let text = std::fs::read_to_string(out)?;
    let mut d = c.design.clone();
    d.ports.remove("output");
    let free = Clock::from_design(d)?;
    let Some((to, r)) = clock::sim::lamp_outputs(&free, 1).into_iter().next() else {
        let s = clock::sim::simulate(&free, &clock::sim::SimOptions::default());
        return Ok((false, format!("no exterior air cell where a lamp lights evenly (redsim without an output: {} {})", s.verdict.word(), if s.period > 0 { gt_time(s.period) } else { s.why })));
    };
    std::fs::write(out, clock::gaps::with_output(&text, to, c.output, None)?)?;
    let ok = r.verdict == clock::sim::Verdict::Clock && r.period as i64 == p;
    Ok((ok, format!("{} blocks, output at {to:?}, redsim {} {}", c.blocks(), r.verdict.word(), if r.period > 0 { gt_time(r.period) } else { r.why.clone() })))
}
