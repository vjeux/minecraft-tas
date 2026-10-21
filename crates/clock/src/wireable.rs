//! `mctas clock wireable` (coordinator 2026-10-21, the wireable-clocks job): per period, the
//! record a player finds (the smallest shown clock, as the site picks it) and whether a builder
//! can tap its output (`clock::usable`: a repeater, comparator or observer tap at a usable cell);
//! when it cannot, the smallest clock of that period that can, and how many blocks that costs.
//! Hidden clocks with a usable tap are listed too: un-hiding one (its output moved to the tap) can
//! give a period its first wireable clock.

use crate::catalog::Row;
use std::collections::BTreeMap;

/// One period's line.
#[derive(Clone, Debug)]
pub struct PeriodLine {
    pub period: u32,
    /// The record: the smallest shown clock.
    pub record: Row,
    pub record_usable: bool,
    /// The smallest shown clock with a usable tap (the record itself when it is usable).
    pub best_usable: Option<Row>,
    /// The smallest hidden clock with a usable tap, when it beats `best_usable`.
    pub hidden_usable: Option<Row>,
}

impl PeriodLine {
    /// Blocks a player pays for a wireable clock of this period over the record (none: no clock
    /// of the period has a usable tap yet).
    pub fn added(&self) -> Option<i64> {
        let b = [self.best_usable.as_ref(), self.hidden_usable.as_ref()].into_iter().flatten().map(|r| r.blocks).min()?;
        Some(b as i64 - self.record.blocks as i64)
    }
}

/// The record order: fewer blocks, then a smaller box, then a named clock over a census one, then
/// the file name (the site: "the smallest; a named clock wins a tie").
fn key(r: &Row) -> (usize, i64, bool, String) {
    (r.blocks, r.volume, r.file.starts_with("census-"), r.file.clone())
}

/// Per period of the shown clocks, the record's pulse spacing (its archived output's even gap):
/// a wireable clock of that period must pulse at the same spacing (a 70-tick clock that pulses
/// every 7 ticks is no 70-tick clock to a builder). A period with no shown clock: its period.
pub fn record_gaps(rows: &[Row]) -> std::collections::HashMap<u32, u32> {
    let mut best: std::collections::HashMap<u32, &Row> = std::collections::HashMap::new();
    for r in rows.iter().filter(|r| r.shown()) {
        let e = best.entry(r.period).or_insert(r);
        if key(r) < key(e) {
            *e = r;
        }
    }
    best.into_iter().map(|(p, r)| (p, crate::usable::row_gap(r).unwrap_or(p))).collect()
}

/// The pulse spacing a wireable clock of period `p` must have.
pub fn target_gap(gaps: &std::collections::HashMap<u32, u32>, p: u32) -> u32 {
    gaps.get(&p).copied().unwrap_or(p)
}

/// Every period of the shown clocks, with `usable(row)` the tap verdict of a row (None: not
/// judged, counted as not usable).
pub fn report(rows: &[Row], usable: impl Fn(&Row) -> Option<bool>) -> Vec<PeriodLine> {
    let mut by_p: BTreeMap<u32, Vec<&Row>> = BTreeMap::new();
    for r in rows {
        by_p.entry(r.period).or_default().push(r);
    }
    let mut out = Vec::new();
    for (p, mut rs) in by_p {
        rs.sort_by_key(|r| key(r));
        let Some(record) = rs.iter().find(|r| r.shown()) else { continue };
        // a wireable clock pulses at the record's spacing
        let g = crate::usable::row_gap(record).unwrap_or(p);
        let ok = |r: &Row| usable(r) == Some(true) && crate::usable::row_gap(r).unwrap_or(p) == g;
        let best_usable = rs.iter().find(|r| r.shown() && ok(r)).map(|r| (*r).clone());
        let hidden_usable = rs.iter().find(|r| !r.shown() && ok(r)).filter(|h| best_usable.as_ref().is_none_or(|b| key(h) < key(b))).map(|r| (*r).clone());
        out.push(PeriodLine { period: p, record: (*record).clone(), record_usable: usable(record) == Some(true), best_usable, hidden_usable });
    }
    out
}

/// The report as Markdown: the counts, then one row per period whose record cannot be tapped.
pub fn markdown(lines: &[PeriodLine]) -> String {
    let n = lines.len();
    let ok = lines.iter().filter(|l| l.record_usable).count();
    let fixed: Vec<&PeriodLine> = lines.iter().filter(|l| !l.record_usable && l.added().is_some()).collect();
    let none = lines.iter().filter(|l| l.added().is_none()).count();
    let mut s = format!("# Wireable clocks per period\n\n{n} periods: the record can be tapped in {ok}; in {} a larger clock of the period can ({} of them only a hidden one, once its output moves to the tap); in {none} no clock of the period can yet.\n\n", fixed.len(), fixed.iter().filter(|l| l.best_usable.is_none()).count());
    s += "| P | record | blocks | wireable clock | blocks | added |\n|---|---|---|---|---|---|\n";
    for l in lines.iter().filter(|l| !l.record_usable) {
        let w = [l.best_usable.as_ref(), l.hidden_usable.as_ref()].into_iter().flatten().min_by_key(|r| key(r));
        let (wn, wb) = match w {
            Some(r) => (format!("{}{}", r.file.trim_end_matches(".txt"), if r.shown() { "" } else { " (hidden)" }), r.blocks.to_string()),
            None => ("none yet".into(), String::new()),
        };
        let add = l.added().map(|a| format!("+{a}")).unwrap_or_default();
        s += &format!("| {} | {} | {} | {wn} | {wb} | {add} |\n", l.period, l.record.file.trim_end_matches(".txt"), l.record.blocks);
    }
    s
}

/// A seed's palette for the wireable search: every block state of the seed (each one with a
/// `facing` in all six facings; a state a block cannot take fails on redsim and scores nothing),
/// plus stone (a support under a tap or a dust line).
pub fn seed_palette(d: &mctas::design::Design) -> Vec<String> {
    let mut v: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for st in d.blocks.values() {
        match st.find("facing=") {
            Some(i) => {
                let rest = &st[i + 7..];
                let end = rest.find([',', ']']).unwrap_or(rest.len());
                for f in ["north", "south", "east", "west", "up", "down"] {
                    v.insert(format!("{}{f}{}", &st[..i + 7], &rest[end..]));
                }
            }
            None => {
                v.insert(st.clone());
            }
        }
    }
    v.insert("minecraft:stone".into());
    v.into_iter().collect()
}

/// The knobs of [`search`] (`period_search`'s, per seed).
pub struct SearchOptions {
    pub seeds: usize,
    pub grow: i32,
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    pub stale: usize,
    pub repair: usize,
    pub horizon: u32,
    /// Block states added to every seed's palette.
    pub extra: Vec<String>,
}

/// One period's search: the smallest found wireable design (its file and blocks with the tap) and
/// how many designs were found.
#[derive(Clone, Debug)]
pub struct Find {
    pub period: u32,
    pub best: Option<(std::path::PathBuf, usize)>,
    pub found: usize,
}

/// `clock wire-search`: for each period, `period_search` from the period's smallest archived clocks
/// (`o.seeds` of them, hidden ones included; their own block palette) with the usable tier, so
/// only designs a builder can tap count; then the smallest find, its blocks counted with its tap.
pub fn search(dir: &std::path::Path, rows: &[Row], periods: &[u32], o: &SearchOptions, out: &std::path::Path) -> anyhow::Result<Vec<Find>> {
    use crate::Clock;
    let gaps = record_gaps(rows);
    let mut finds = Vec::new();
    for &p in periods {
        // a find must pulse at the period's spacing (the record's gap)
        let g = target_gap(&gaps, p);
        let mut rs: Vec<&Row> = rows.iter().filter(|r| r.period == p && !r.hidden.starts_with("command-only")).collect();
        rs.sort_by_key(|r| key(r));
        let mut best: Option<(std::path::PathBuf, usize)> = None;
        let mut found = 0;
        for r in rs.iter().take(o.seeds) {
            let Ok(c) = Clock::load(&dir.join(&r.file)) else { continue };
            if !c.design.entities.is_empty() {
                continue;
            }
            let stem = r.file.trim_end_matches(".txt");
            let sdir = out.join(format!("P{p}")).join(stem);
            // the seed without its declared output (a tap is the output; a hidden clock's port can
            // sit in a block, which the judge refuses)
            std::fs::create_dir_all(&sdir)?;
            let mut d = c.design.clone();
            d.ports.remove("output");
            let seed = sdir.join("seed.txt");
            std::fs::write(&seed, d.to_text())?;
            let mut palette = seed_palette(&c.design);
            for s in &o.extra {
                if !palette.contains(s) {
                    palette.push(s.clone());
                }
            }
            let po = crate::period_search::Options {
                period: p,
                palette,
                grow: o.grow,
                iterations: o.iterations,
                workers: o.workers,
                rng: o.rng,
                stale: o.stale,
                repair: o.repair,
                horizon: o.horizon.max(3 * p + 200),
                usable: Some(Box::new(move |c: &Clock| crate::usable::pick(c, crate::usable::Rule::default(), Some(g)).is_ok())),
            };
            match crate::period_search::search(&seed, &po, &sdir) {
                Ok((_, v, n)) => {
                    eprintln!("P{p} {stem}: {n} wireable designs; best: {v}");
                    found += n;
                }
                Err(e) => {
                    eprintln!("P{p} {stem}: {e:#}");
                    continue;
                }
            }
            // the smallest finds, counted with their tap
            let mut files: Vec<(usize, std::path::PathBuf)> = std::fs::read_dir(sdir.join("found")).map(|it| it.filter_map(|e| e.ok()).map(|e| e.path()).filter(|q| q.extension().is_some_and(|x| x == "txt")).filter_map(|q| Clock::load(&q).ok().map(|c| (c.blocks(), q))).collect()).unwrap_or_default();
            files.sort();
            for (_, q) in files.into_iter().take(5) {
                let Ok(c) = Clock::load(&q) else { continue };
                let Ok((u, _)) = crate::usable::pick(&c, crate::usable::Rule::default(), Some(g)) else { continue };
                let Ok(t) = crate::usable::tapped_design(&c, u.tap) else { continue };
                if best.as_ref().is_none_or(|(_, b)| t.blocks() < *b) {
                    best = Some((q.clone(), t.blocks()));
                }
            }
        }
        finds.push(Find { period: p, best, found });
    }
    Ok(finds)
}

/// `clock wire-census --pool DIR`: every design file under the directories (3 levels deep), its
/// declared output dropped, run on redsim; those of the given periods through the usable pick; per
/// period the smallest a builder can tap, its tapped design written to `out/P<period>-<stem>.txt`.
/// Returns (period, blocks, file) per period with a find.
pub fn pool_screen(dirs: &[std::path::PathBuf], targets: &std::collections::HashMap<u32, u32>, threads: usize, out: &std::path::Path) -> anyhow::Result<Vec<(u32, usize, std::path::PathBuf)>> {
    use crate::Clock;
    use crate::sim::{SimOptions, Verdict, simulate};
    fn walk(d: &std::path::Path, depth: u32, acc: &mut Vec<std::path::PathBuf>) {
        let Ok(it) = std::fs::read_dir(d) else { return };
        for e in it.filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                if depth > 0 {
                    walk(&p, depth - 1, acc);
                }
            } else if p.extension().is_some_and(|x| x == "txt") {
                acc.push(p);
            }
        }
    }
    let mut files = Vec::new();
    for d in dirs {
        walk(d, 3, &mut files);
    }
    files.sort();
    eprintln!("wire-census --pool: {} design files", files.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let res: std::sync::Mutex<Vec<(u32, usize, usize, mctas::design::Design)>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(f) = files.get(i) else { break };
                    let Ok(text) = std::fs::read_to_string(f) else { continue };
                    let Ok(mut d) = mctas::design::Design::parse(&text) else { continue };
                    if !d.entities.is_empty() || d.blocks.len() > 40 {
                        continue;
                    }
                    d.ports.remove("output");
                    let Ok(c) = Clock::from_design(d) else { continue };
                    let r = simulate(&c, &SimOptions::default());
                    let Some(&g) = targets.get(&r.period).filter(|_| r.verdict == Verdict::Clock) else { continue };
                    let Ok((u, _)) = crate::usable::pick(&c, crate::usable::Rule::default(), Some(g)) else { continue };
                    let Ok(t) = crate::usable::tapped_design(&c, u.tap) else { continue };
                    res.lock().unwrap().push((r.period, t.blocks(), i, t.design.clone()));
                }
            });
        }
    });
    let mut res = res.into_inner().unwrap();
    res.sort_by_key(|r| (r.0, r.1, r.2));
    std::fs::create_dir_all(out)?;
    let mut best = Vec::new();
    for r in &res {
        if best.last().is_some_and(|(p, _, _)| *p == r.0) {
            continue;
        }
        let stem = files[r.2].file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let mut d = r.3.clone();
        d.name = format!("{stem}-usable");
        d.meta.insert("from".into(), format!("{} (tapped by clock::usable)", files[r.2].display()));
        let path = out.join(format!("P{}-{stem}.txt", r.0));
        d.save(&path)?;
        best.push((r.0, r.1, path));
    }
    Ok(best)
}

/// `clock wire-census`: every MINIMAL census clock of the given periods (designs/clocks/census/*/
/// minimal.tsv: its encoding and start) through `clock::usable`'s pick on redsim, `threads` at a
/// time; per period the smallest one a builder can tap (blocks with its tap), its tapped design
/// written to `out/P<period>-<n>.txt`. Returns (period, blocks, file) per period with a find.
pub fn census_screen(census_dir: &std::path::Path, targets: &std::collections::HashMap<u32, u32>, threads: usize, out: &std::path::Path) -> anyhow::Result<Vec<(u32, usize, std::path::PathBuf)>> {
    use crate::Clock;
    let want: std::collections::BTreeSet<u32> = targets.keys().copied().collect();
    // (period, blocks, census, design)
    let mut cands: Vec<(u32, usize, String, mctas::design::Design)> = Vec::new();
    let mut dirs: Vec<std::path::PathBuf> = std::fs::read_dir(census_dir)?.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.join("minimal.tsv").exists()).collect();
    dirs.sort();
    for d in &dirs {
        let text = std::fs::read_to_string(d.join("minimal.tsv"))?;
        let head: Vec<&str> = text.lines().next().unwrap_or_default().split('\t').collect();
        let col = |name: &str| head.iter().position(|h| *h == name);
        let (Some(cp), Some(cb), Some(cs), Some(ce)) = (col("period"), col("blocks"), col("start"), col("encoding")) else { continue };
        for line in text.lines().skip(1) {
            let f: Vec<&str> = line.split('\t').collect();
            let (Some(p), Some(b)) = (f.get(cp).and_then(|x| x.parse::<u32>().ok()), f.get(cb).and_then(|x| x.parse::<usize>().ok())) else { continue };
            if !want.contains(&p) {
                continue;
            }
            let mut des = mctas::design::Design { name: format!("census-p{p}-{b}b"), ..Default::default() };
            let mut ok = true;
            for cell in f.get(ce).copied().unwrap_or_default().split(';') {
                let Some((xyz, s)) = cell.split_once('=') else {
                    ok = false;
                    break;
                };
                let v: Vec<i32> = xyz.split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() != 3 {
                    ok = false;
                    break;
                }
                des.blocks.insert([v[0], v[1], v[2]], format!("minecraft:{s}"));
            }
            if let Some(rest) = f.get(cs).and_then(|s| s.strip_prefix("last ")) {
                let v: Vec<i32> = rest.split(',').filter_map(|x| x.parse().ok()).collect();
                if v.len() == 3 {
                    des.ports.insert("start".into(), [v[0], v[1], v[2]]);
                    des.meta.insert("port start".into(), "last".into());
                }
            }
            if ok {
                cands.push((p, b, d.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default(), des));
            }
        }
    }
    cands.sort_by_key(|c| (c.0, c.1));
    eprintln!("wire-census: {} minimal census clocks in {} periods", cands.len(), cands.iter().map(|c| c.0).collect::<std::collections::BTreeSet<_>>().len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let res: std::sync::Mutex<Vec<(u32, usize, usize, mctas::design::Design)>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((p, _, _, des)) = cands.get(i) else { break };
                    let Ok(c) = Clock::from_design(des.clone()) else { continue };
                    let Ok((u, _)) = crate::usable::pick(&c, crate::usable::Rule::default(), targets.get(p).copied()) else { continue };
                    let Ok(t) = crate::usable::tapped_design(&c, u.tap) else { continue };
                    res.lock().unwrap().push((*p, t.blocks(), i, t.design.clone()));
                }
            });
        }
    });
    let mut res = res.into_inner().unwrap();
    res.sort_by_key(|r| (r.0, r.1, r.2));
    std::fs::create_dir_all(out)?;
    let mut best = Vec::new();
    for r in &res {
        if best.last().is_some_and(|(p, _, _)| *p == r.0) {
            continue;
        }
        let mut d = r.3.clone();
        d.name = format!("census-p{}-usable-{}b", r.0, r.1);
        d.meta.insert("census".into(), format!("{} (minimal clock {}), tapped by clock::usable", cands[r.2].2, cands[r.2].1));
        let path = out.join(format!("P{}-{}b.txt", r.0, r.1));
        d.save(&path)?;
        best.push((r.0, r.1, path));
    }
    Ok(best)
}
