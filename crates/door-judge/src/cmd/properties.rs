//! `door relever`, `door eligible`, `door properties-dyn`, `door seamless-why`, `door properties`:
//! record eligibility, relevering and the catalogue properties.

use anyhow::Result;
use mctas::design::Design;
use mctas::sim::WorkerConfig;
use piston_door::cli::load_design;
use piston_door::*;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct ReleverArgs {
    designs: Vec<PathBuf>,
    #[arg(long, default_value = "note")]
    subs: String,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 16)]
    workers: usize,
    /// Skip designs this verdict cache (verdicts-v5.tsv) marks broken.
    #[arg(long)]
    verdicts: Option<PathBuf>,
    #[arg(long, default_value_t = 400)]
    max_blocks: usize,
    /// Also accept variants that are not rotation-safe.
    #[arg(long)]
    any_rotation: bool,
    /// Write the result table (markdown) here too.
    #[arg(long)]
    table: Option<PathBuf>,
    /// Also print why each failing variant fails.
    #[arg(long)]
    why: bool,
    /// Also every design on HEAD and on the origin/explore/* branches (as the leaderboard
    /// collects them, at most --max-blocks).
    #[arg(long)]
    branches: bool,
    /// Only designs at least this seamless as they are (`full`, `semi`, ...: screened first),
    /// and every eligible variant must meet `<tier>[+hall]` too.
    #[arg(long)]
    seamless: Option<String>,
}

pub fn cmd_relever(version: &str, args: ReleverArgs) -> Result<()> {
    let ReleverArgs { designs, subs, out_dir, extra_offsets, workers, verdicts, max_blocks, any_rotation, table, why, branches, seamless } = args;
    let offsets = crate::search::judge_offsets(extra_offsets);
    let judge = VerifyOptions { offsets: offsets.clone(), ..Default::default() };
    let screen = VerifyOptions { cycles: 1, max_ticks: 200, offsets: offsets.iter().take(2).copied().collect(), keep_frames: false, empty_walkway: true, tile: None };
    let known = verdicts.as_deref().map(crate::properties_dyn::load_verdicts).unwrap_or_default();
    let seam = seamless.as_deref().map(crate::properties::SeamReq::parse).transpose()?;
    let subs: Vec<String> = subs.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
    let mut seen = std::collections::HashSet::new();
    let mut loaded: Vec<(String, Design)> = Vec::new();
    let mut sources: Vec<(String, Design)> = Vec::new();
    for p in &designs {
        match load_design(p) {
            Ok(d) => sources.push((p.display().to_string(), d)),
            Err(e) => eprintln!("{}: {e:#}", p.display()),
        }
    }
    if branches {
        sources.extend(crate::properties_dyn::collect_refs(std::path::Path::new("."), "refs/remotes/origin/explore/", max_blocks)?);
    }
    for (src, d) in sources {
        let h = format!("{:016x}", crate::leaderboard::design_hash(&d));
        if crate::block_count(&d) > max_blocks || known.get(&h) == Some(&false) || !seen.insert(d.content_key()) {
            continue;
        }
        loaded.push((src, d));
    }
    // Seamless pre-screen (the tier only, as the design is).
    if let Some(req) = seam {
        let tier_only = crate::properties::SeamReq { hallway: false, ..req };
        let pool = mctas::sim::Pool::spawn(workers.min(loaded.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
        let keep = pool.map(loaded.clone(), |w, _, (_, d)| {
            let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
            let ok = tier_only.check(w, sb, &d).is_ok();
            let _ = w.drop_sandbox(sb);
            Ok(ok)
        });
        let before = loaded.len();
        loaded = loaded.into_iter().zip(keep).filter(|(_, k)| matches!(k, Ok(true))).map(|(x, _)| x).collect();
        eprintln!("seamless pre-screen: {} of {before} designs reach the tier", loaded.len());
    }
    let mut jobs: Vec<(usize, crate::relever::Variant)> = Vec::new();
    for (i, (_, d)) in loaded.iter().enumerate() {
        match crate::relever::variants(d, &subs) {
            Ok(vs) => jobs.extend(vs.into_iter().map(|v| (i, v))),
            Err(e) => eprintln!("{}: {e:#}", loaded[i].0),
        }
    }
    eprintln!("{} designs, {} variants (as-is included), {} positions", loaded.len(), jobs.len(), offsets.len());
    let pool = mctas::sim::Pool::spawn(workers.min(jobs.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let results = pool.map(jobs, |w, _, (i, v)| Ok((i, crate::relever::judge_variant(w, v, &judge, &screen, !any_rotation, seam)?)));
    let mut per: Vec<Vec<crate::relever::Outcome>> = vec![Vec::new(); loaded.len()];
    for r in results {
        match r {
            Ok((i, o)) => per[i].push(o),
            Err(e) => eprintln!("worker error: {e:#}"),
        }
    }
    let mut order: Vec<usize> = (0..loaded.len()).collect();
    order.sort_by_key(|&i| (crate::category(&loaded[i].1) != "solid", crate::category(&loaded[i].1), crate::block_count(&loaded[i].1), loaded[i].0.clone()));
    let mut md = String::from("| design | blocks | category | as-is | eligible variants (blocks, lever open/close, repeater open/close, seamless) |\n|---|---|---|---|---|\n");
    if let Some(dir) = &out_dir {
        std::fs::create_dir_all(dir)?;
    }
    for i in order {
        let (src, d) = &loaded[i];
        let stem = std::path::Path::new(src).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let mut asis = String::from("-");
        let mut good = Vec::new();
        for o in &per[i] {
            match (&o.verdict, o.label.as_str()) {
                (Ok((lo, lc, ro, rc, t)), "as-is") => asis = format!("ELIGIBLE ({lo}/{lc}, rep {ro}/{rc}, {t})"),
                (Err(why), "as-is") => asis = format!("no: {}", why.chars().take(90).collect::<String>()),
                (Ok((lo, lc, ro, rc, t)), label) => {
                    good.push(format!("{label} ({}, {lo}/{lc}, rep {ro}/{rc}, {t})", crate::block_count(&o.design)));
                    if let Some(dir) = &out_dir {
                        let mut nd = o.design.clone();
                        nd.name = format!("{}-rl-{label}", d.name);
                        nd.save(&dir.join(format!("{stem}-rl-{label}.txt")))?;
                    }
                }
                (Err(reason), label) if why => println!("  {stem} {label}: {}", reason.chars().take(300).collect::<String>()),
                _ => {}
            }
        }
        let line = format!("| {stem} | {} | {} | {asis} | {} |", crate::block_count(d), crate::category(d), if good.is_empty() { "-".to_string() } else { good.join("; ") });
        println!("{line}");
        md.push_str(&line);
        md.push('\n');
    }
    if let Some(p) = table {
        std::fs::write(p, md)?;
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct EligibleArgs {
    designs: Vec<PathBuf>,
    #[arg(long, default_value_t = 28)]
    extra_offsets: u32,
    #[arg(long, default_value_t = 8)]
    workers: usize,
}

pub fn cmd_eligible(version: &str, args: EligibleArgs) -> Result<()> {
    let EligibleArgs { designs, extra_offsets, workers } = args;
    let offsets = crate::search::judge_offsets(extra_offsets);
    let loaded: Vec<(String, Design)> = designs.iter().map(|p| Ok((p.display().to_string(), load_design(p)?))).collect::<Result<_>>()?;
    let pool = mctas::sim::Pool::spawn(workers.min(loaded.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let lines = pool.map(loaded, |w, _, (name, d)| Ok(crate::properties_dyn::eligibility_line(w, &name, &d, &offsets)));
    let mut all_ok = true;
    for l in lines {
        let (ok, line) = l?;
        all_ok &= ok;
        println!("{line}");
    }
    if !all_ok {
        std::process::exit(1);
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct PropertiesDynArgs {
    /// Design files (always run, cached verdict or not).
    designs: Vec<PathBuf>,
    /// Folder of designs to add (the working ones per the verdict cache).
    #[arg(long)]
    archive_dir: Option<PathBuf>,
    /// Also every design on HEAD and on the explore/* branches (as the leaderboard collects
    /// them), the working ones per the verdict cache.
    #[arg(long)]
    branches: bool,
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    /// The leaderboard's verdict cache.
    #[arg(long, default_value = "designs/piston-door-3x3/archive/verdicts-v5.tsv")]
    verdicts: PathBuf,
    /// Also run designs the cache does not know (verdict "uncached").
    #[arg(long)]
    uncached: bool,
    #[arg(long, default_value_t = 200)]
    max_blocks: usize,
    #[arg(long, default_value_t = 24)]
    workers: usize,
    #[arg(long, default_value = "designs/piston-door-3x3/archive/properties-dyn-v1.tsv")]
    out_tsv: PathBuf,
}

pub fn cmd_properties_dyn(version: &str, args: PropertiesDynArgs) -> Result<()> {
    let PropertiesDynArgs { designs, archive_dir, branches, refs, verdicts, uncached, max_blocks, workers, out_tsv } = args;
    use crate::properties_dyn as pd;
    let cache = pd::load_verdicts(&verdicts);
    let mut cands: Vec<(String, Design, bool)> = Vec::new();
    for p in &designs {
        cands.push((p.display().to_string(), load_design(p)?, true));
    }
    if let Some(dir) = &archive_dir {
        cands.extend(crate::shapes::collect_dir(dir, max_blocks)?.into_iter().map(|(s, d)| (s, d, false)));
    }
    if branches {
        cands.extend(pd::collect_refs(std::path::Path::new("."), &refs, max_blocks)?.into_iter().map(|(s, d)| (s, d, false)));
    }
    let mut seen = std::collections::HashSet::new();
    let mut jobs: Vec<(String, Design, String)> = Vec::new();
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for (src, d, explicit) in cands {
        let h = format!("{:016x}", crate::leaderboard::design_hash(&d));
        if !seen.insert(h.clone()) {
            continue;
        }
        let verdict = match cache.get(&h) {
            Some(true) => "ok",
            Some(false) => "broken",
            None => "uncached",
        };
        *counts.entry(verdict).or_default() += 1;
        if explicit || verdict == "ok" || (uncached && verdict == "uncached") {
            jobs.push((src, d, verdict.to_string()));
        }
    }
    jobs.sort_by_key(|(s, d, _)| (crate::block_count(d), s.clone()));
    eprintln!(
        "properties-dyn: {counts:?} in the verdict cache; running {} designs{}",
        jobs.len(),
        if uncached { " (uncached ones only if the judge passes them at offset 0)" } else { " (uncached skipped; --uncached runs them)" }
    );
    if jobs.is_empty() {
        return Ok(());
    }
    let pool = mctas::sim::Pool::spawn(workers.min(jobs.len()).max(1), &WorkerConfig { version: version.to_string(), heap_mb: 1024, ..Default::default() })?;
    let t0 = std::time::Instant::now();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let n = jobs.len();
    let results = pool.map(jobs, |w, _, (src, d, verdict)| {
        // A design the cache does not know runs only if the judge passes it at the
        // design's own position (current rules).
        let precheck = verdict != "uncached"
            || crate::verify(w, &d, &crate::VerifyOptions { offsets: vec![[0, 0, 0]], ..Default::default() }).is_ok();
        let r = if precheck {
            let v = if verdict == "uncached" { "uncached (judge ok at offset 0)" } else { verdict.as_str() };
            pd::check_design(w, &src, &d, v).map(Some).map_err(|e| e.context(src.clone()))
        } else {
            Ok(None)
        };
        let k = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
        eprintln!("[{k}/{n} {:.0}s] {src}{}", t0.elapsed().as_secs_f64(), if precheck { "" } else { " (uncached, fails the judge: skipped)" });
        r
    });
    let mut rows = Vec::new();
    for r in results {
        match r {
            Ok(Some(row)) => rows.push(row),
            Ok(None) => {}
            Err(e) => eprintln!("error: {e:#}"),
        }
    }
    rows.sort_by_key(|r| (r.blocks, r.source.clone()));
    let mut tsv = String::from(pd::TSV_HEADER);
    tsv.push('\n');
    for r in &rows {
        tsv.push_str(&pd::tsv_line(r));
        tsv.push('\n');
    }
    if let Some(dir) = out_tsv.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&out_tsv, tsv)?;
    let table = pd::table(&rows);
    std::fs::write(out_tsv.with_extension("md"), &table)?;
    print!("{table}");
    eprintln!("wrote {} and {} ({} rows)", out_tsv.display(), out_tsv.with_extension("md").display(), rows.len());
    Ok(())
}

#[derive(clap::Args)]
pub struct SeamlessWhyArgs {
    designs: Vec<PathBuf>,
}

pub fn cmd_seamless_why(version: &str, args: SeamlessWhyArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let SeamlessWhyArgs { designs } = args;
    let mut w = mctas::sim::Worker::spawn(&cfg)?;
    for path in &designs {
        let d = load_design(path)?;
        let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        let rec = crate::properties::record(&mut w, sb, &d);
        let _ = w.drop_sandbox(sb);
        let rec = rec?;
        let s = crate::properties::seamless(&rec);
        println!("== {} ({} blocks): {}{}", path.display(), crate::block_count(&d), s.label, s.why.map(|w| format!(" -- {w}")).unwrap_or_default());
        for line in crate::properties::super_report(&rec) {
            println!("  {line}");
        }
    }
    Ok(())
}

#[derive(clap::Args)]
pub struct PropertiesArgs {
    designs: Vec<PathBuf>,
    #[arg(long, default_value = "designs/piston-door-3x3/archive")]
    archive_dir: PathBuf,
    #[arg(long, default_value = "refs/remotes/origin/explore/")]
    refs: String,
    #[arg(long, default_value_t = 12)]
    workers: usize,
    /// Also write the rows as TSV here (design, blocks, category, then every property).
    #[arg(long)]
    out_tsv: Option<PathBuf>,
    /// Write the markdown table here (also printed).
    #[arg(long)]
    out_md: Option<PathBuf>,
    /// Also run the dynamic checks (hallway-compatible, contained, snow, repeater input,
    /// player steps, uses), each at the design's own position.
    #[arg(long)]
    dynamic: bool,
    /// Recompute cached designs.
    #[arg(long)]
    redo: bool,
}

pub fn cmd_properties(version: &str, args: PropertiesArgs) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    let PropertiesArgs { designs, archive_dir, refs, workers, out_tsv, out_md, dynamic, redo } = args;
    use crate::properties as pr;
    let repo = std::path::Path::new(".");
    let verdicts = pr::cached_verdicts(&archive_dir);
    let vanilla = pr::vanilla_verdicts(repo);
    let mut cands: Vec<(String, Design)> = Vec::new();
    if designs.is_empty() {
        let all = crate::leaderboard::collect(repo, &refs, 400)?;
        let n = all.len();
        let mut unknown = 0;
        for (s, d) in all {
            match verdicts.get(&crate::leaderboard::design_hash(&d)) {
                Some((true, _)) => cands.push((s, d)),
                Some((false, _)) => {}
                None => {
                    unknown += 1;
                    cands.push((s, d));
                }
            }
        }
        eprintln!("{n} designs, {} working or not judged yet ({unknown} not judged: screened by the judge at their own position)", cands.len());
    } else {
        for p in &designs {
            cands.push((p.display().to_string(), load_design(p)?));
        }
    }
    let mut cache = pr::load_cache(&archive_dir);
    let todo: Vec<(u64, Design)> = {
        let mut seen = std::collections::HashSet::new();
        cands
            .iter()
            .map(|(_, d)| (crate::leaderboard::design_hash(d), d.clone()))
            .filter(|(h, _)| (redo || !cache.get(h).is_some_and(|p| !dynamic || p.contains_key("hallway"))) && seen.insert(*h))
            .collect()
    };
    eprintln!("{} designs to compute", todo.len());
    if !todo.is_empty() {
        let pool = mctas::sim::Pool::spawn(workers.min(todo.len()).max(1), &cfg)?;
        let results = pool.map(todo, |w, _, (h, d)| {
            if !verdicts.contains_key(&h) {
                let one = VerifyOptions { offsets: vec![[0, 0, 0]], ..Default::default() };
                if let Err(e) = crate::verify(w, &d, &one) {
                    let mut p = pr::Props::new();
                    p.insert("screen".into(), format!("broken: {}", format!("{e:#}").replace(['\t', '\n'], " ")));
                    return Ok((h, Ok(p)));
                }
            }
            Ok((h, pr::compute(w, &d, dynamic)))
        });
        for r in results {
            let (h, p) = r?;
            match p {
                Ok(p) => {
                    let mut merged = cache.remove(&h).unwrap_or_default();
                    merged.extend(p);
                    cache.insert(h, merged);
                }
                Err(e) => eprintln!("{h:016x}: {e:#}"),
            }
        }
        pr::save_cache(&archive_dir, &cache)?;
    }
    let dynp = pr::load_dyn(&archive_dir);
    if !dynp.is_empty() {
        eprintln!("joined {} designs' dynamic checks from properties-dyn-v1.tsv", dynp.len());
    }
    let mut rows = Vec::new();
    for (s, d) in &cands {
        let h = crate::leaderboard::design_hash(d);
        let Some(mut p) = cache.get(&h).cloned() else { continue };
        if let Some(x) = dynp.get(&h) {
            p.extend(x.clone());
        }
        if p.contains_key("steps") {
            isolated_and_eligible(&mut p);
        }
        if p.get("screen").is_some_and(|s| s.starts_with("broken")) {
            continue;
        }
        if let Some((ok, rot)) = verdicts.get(&h) {
            p.insert("dir".into(), if rot == "safe" { "yes".into() } else { format!("no ({rot})") });
            p.insert("loc".into(), if *ok { "yes (32 pos)".into() } else { "no".into() });
        } else {
            p.insert("dir".into(), "? (not judged yet)".into());
            p.insert("loc".into(), "? (1 pos)".into());
        }
        p.insert("vanilla".into(), vanilla.get(&h).cloned().unwrap_or_else(|| "sim only".into()));
        rows.push((pr::display_name(s), crate::block_count(d), crate::category(d), p));
    }
    rows.sort_by(|a, b| (a.2 != "solid", a.2, a.1, &a.0).cmp(&(b.2 != "solid", b.2, b.1, &b.0)));
    let leaders: Vec<_> = pr::LEADERS.iter().filter_map(|n| rows.iter().find(|r| r.0 == *n).cloned()).collect();
    let table = format!(
        "# Door properties (Redstone Squid catalogue rules)\n\nDefinitions and judgment calls: [NOTES.md](NOTES.md). One position, one orientation, one cycle per check; not directional / not locational from the leaderboard's cached verdicts.\n\n## Board leaders\n\n{}\n## Best design per property\n\n{}\n## Every working design ({})\n\n{}",
        pr::render(&leaders),
        pr::summary(&rows),
        rows.len(),
        pr::render(&rows)
    );
    print!("{table}");
    if let Some(path) = out_md {
        std::fs::write(path, &table)?;
    }
    if let Some(path) = out_tsv {
        let keys: Vec<String> = {
            let mut k = std::collections::BTreeSet::new();
            for r in &rows {
                k.extend(r.3.keys().cloned());
            }
            k.into_iter().collect()
        };
        let mut out = format!("design\tblocks\tcategory\t{}\n", keys.join("\t"));
        for (n, b, c, p) in &rows {
            let vals: Vec<String> = keys.iter().map(|k| p.get(k).cloned().unwrap_or_default()).collect();
            out.push_str(&format!("{n}\t{b}\t{c}\t{}\n", vals.join("\t")));
        }
        std::fs::write(path, out)?;
    }
    Ok(())
}

/// A design's derived properties: isolated (footsteps vs sculk) and record eligibility.
fn isolated_and_eligible(p: &mut crate::properties::Props) {
    let sculk = p.get("risky").is_some_and(|r| r.contains("sculk"));
    let steps_ok = p.get("steps").is_some_and(|s| s.starts_with("yes"));
    let risky = p.get("risky").is_some_and(|r| r != "-");
    p.insert(
        "isolated".into(),
        if sculk && !steps_ok {
            "no (hears footsteps)".into()
        } else if risky {
            "? (steps ok, outside untested)".into()
        } else {
            "yes".into()
        },
    );
    // Record eligibility (3): valid input (lever on the surface, steady support,
    // repeater input), unlimited uses, footsteps harmless, isolated, and a seamless
    // tier whose hallway can be built.
    let yes = |k: &str| p.get(k).is_some_and(|v| v.starts_with("yes"));
    let mut miss = Vec::new();
    for (k, what) in [
        ("lever_surface", "lever"),
        ("lever_support", "support"),
        ("repeater_input", "repeater input"),
        ("uses", "uses"),
        ("steps", "steps"),
        ("isolated", "isolated"),
        ("hallway", "hallway"),
    ] {
        if !yes(k) {
            miss.push(what);
        }
    }
    if p.get("seamless").is_none_or(|s| s == "-") {
        miss.push("seamless");
    }
    p.insert("eligible".into(), if miss.is_empty() { "yes".into() } else { format!("no ({})", miss.join(", ")) });
}
