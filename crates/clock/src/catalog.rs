//! `mctas clock archive` (the judge, then a copy in `designs/clocks/archive/` with its verdict and
//! a caption) and `mctas clock catalog` (the generated `designs/clocks/CATALOG.md`: per period the
//! distinct types, the families and the smallest clock of each, the period x family coverage
//! matrix, and the impossibility table; plus `designs/clocks/coverage.md`, the matrix alone, which
//! the board's `minecraft-tas/clocks/coverage.md` mirrors).

use crate::verify::{VerifyOptions, verify_in_pool};
use crate::{Clock, canon, sim::Verdict, wave};
use anyhow::{Context, Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ARCHIVE_DIR: &str = "designs/clocks/archive";
pub const VERDICTS: &str = "verdicts-v1.tsv";
pub const CAPTIONS: &str = "captions.tsv";
/// Designs removed as duplicates of an archived type: kept file, alias file, alias name.
pub const ALIASES: &str = "aliases.tsv";
pub const CATALOG: &str = "designs/clocks/CATALOG.md";
pub const COVERAGE: &str = "designs/clocks/coverage.md";
/// Hand-edited: `family<TAB>periods<TAB>why` rows (a reason or the census that proves it).
pub const IMPOSSIBLE: &str = "designs/clocks/impossible.tsv";

const HEADER: &str = "hash\tfile\tperiod\twarmup\ton\tedges\tpulses\tfamily\tblocks\tdims\tvolume\trobustness\tstart\ttype\tchiral\tsummary";

/// FNV-1a 64 of a clock's content key, in hex (the archive's identity of a clock).
pub fn content_hash(c: &Clock) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in c.content_key().bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// One archived clock (a row of `verdicts-v1.tsv`).
#[derive(Clone, Debug)]
pub struct Row {
    pub hash: String,
    pub file: String,
    pub period: u32,
    pub warmup: u32,
    pub on: u32,
    pub edges: u32,
    pub pulses: String,
    pub family: String,
    pub blocks: usize,
    pub dims: String,
    pub volume: i64,
    pub robustness: String,
    pub start: String,
    pub type_key: String,
    pub chiral: bool,
    pub summary: String,
}

impl Row {
    fn to_tsv(&self) -> String {
        [
            self.hash.clone(),
            self.file.clone(),
            self.period.to_string(),
            self.warmup.to_string(),
            self.on.to_string(),
            self.edges.to_string(),
            self.pulses.clone(),
            self.family.clone(),
            self.blocks.to_string(),
            self.dims.clone(),
            self.volume.to_string(),
            self.robustness.clone(),
            self.start.clone(),
            self.type_key.clone(),
            self.chiral.to_string(),
            self.summary.clone(),
        ]
        .join("\t")
    }

    fn from_tsv(line: &str) -> Option<Row> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 16 || f[0] == "hash" {
            return None;
        }
        Some(Row {
            hash: f[0].into(),
            file: f[1].into(),
            period: f[2].parse().ok()?,
            warmup: f[3].parse().ok()?,
            on: f[4].parse().ok()?,
            edges: f[5].parse().ok()?,
            pulses: f[6].into(),
            family: f[7].into(),
            blocks: f[8].parse().ok()?,
            dims: f[9].into(),
            volume: f[10].parse().ok()?,
            robustness: f[11].into(),
            start: f[12].into(),
            type_key: f[13].into(),
            chiral: f[14] == "true",
            summary: f[15].into(),
        })
    }
}

pub fn read_rows(dir: &Path) -> Result<Vec<Row>> {
    let p = dir.join(VERDICTS);
    if !p.exists() {
        return Ok(Vec::new());
    }
    // one row per file: a union merge of two branches can repeat a file's row; the newest (last)
    // row wins, in the place of the first
    let all: Vec<Row> = std::fs::read_to_string(&p)?.lines().filter_map(Row::from_tsv).collect();
    let mut last: BTreeMap<String, usize> = BTreeMap::new();
    for (i, r) in all.iter().enumerate() {
        last.insert(r.file.clone(), i);
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut out = Vec::with_capacity(last.len());
    for r in &all {
        if seen.insert(r.file.clone()) {
            out.push(all[last[&r.file]].clone());
        }
    }
    Ok(out)
}

/// Rewrite verdicts-v1.tsv and captions.tsv with one row per file (the newest), dropping the
/// files in `gone`.
pub fn write_tables(dir: &Path, gone: &BTreeSet<String>) -> Result<()> {
    let rows = read_rows(dir)?;
    let mut s = format!("{HEADER}\n");
    for r in rows.iter().filter(|r| !gone.contains(&r.file)) {
        s.push_str(&r.to_tsv());
        s.push('\n');
    }
    std::fs::write(dir.join(VERDICTS), s)?;
    let cp = dir.join(CAPTIONS);
    if cp.exists() {
        let caps = read_captions(dir);
        let mut order: Vec<String> = Vec::new();
        for l in std::fs::read_to_string(&cp)?.lines().skip(1) {
            if let Some((f, _)) = l.split_once('\t') {
                if !order.iter().any(|o| o == f) {
                    order.push(f.to_string());
                }
            }
        }
        let mut c = String::from("file\tcaption\n");
        for f in order.iter().filter(|f| !gone.contains(*f)) {
            c.push_str(&format!("{f}\t{}\n", caps[f]));
        }
        std::fs::write(&cp, c)?;
    }
    Ok(())
}

fn read_captions(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(CAPTIONS))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .filter(|(f, _)| *f != "file")
        .map(|(f, c)| (f.to_string(), c.to_string()))
        .collect()
}

/// `YYYY-MM-DDTHH:MMZ` now (UTC).
pub fn now_utc() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (days, rem) = (secs.div_euclid(86400), secs.rem_euclid(86400));
    // civil from days (Howard Hinnant)
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}Z", rem / 3600, (rem % 3600) / 60)
}

/// The caption: the given one, else the design's first comment block.
fn default_caption(text: &str) -> String {
    let mut lines = Vec::new();
    for l in text.lines() {
        let t = l.trim();
        if let Some(c) = t.strip_prefix('#') {
            lines.push(c.trim().to_string());
        } else if !lines.is_empty() && !t.starts_with("name:") {
            break;
        }
    }
    lines.join(" ")
}

/// The type key of an archived design, from its file (the identity-normalized key: `ident`), else
/// the row's.
pub fn archived_type_key(dir: &Path, r: &Row) -> String {
    match Clock::load(&dir.join(&r.file)) {
        Ok(c) => crate::ident::type_key(&c),
        Err(_) => r.type_key.clone(),
    }
}

/// The type key of every archived row (`ident::archive_keys`: cached by file text, computed in
/// parallel).
pub fn type_keys(dir: &Path, rows: &[Row]) -> BTreeMap<String, String> {
    let files: Vec<(String, String)> = rows.iter().map(|r| (r.file.clone(), r.type_key.clone())).collect();
    crate::ident::archive_keys(dir, &files)
}

/// The dominance fold within one period (vjeux: "the left one is strictly smaller than the right
/// one"): `items[i]` = (blocks, volume, name, block kinds); for each item, the index of the item it
/// folds under: among the items with strictly fewer blocks whose kinds are a subset of its own,
/// the one with the fewest blocks (then volume, name); `None` when none. A host is never folded
/// itself (anything dominating it would dominate the item with fewer blocks).
pub fn fold_hosts(items: &[(usize, i64, String, BTreeSet<String>)]) -> Vec<Option<usize>> {
    items
        .iter()
        .map(|(b, _, _, k)| {
            items
                .iter()
                .enumerate()
                .filter(|(_, (b2, _, _, k2))| b2 < b && k2.is_subset(k))
                .min_by(|(_, x), (_, y)| (x.0, x.1, &x.2).cmp(&(y.0, y.1, &y.2)))
                .map(|(j, _)| j)
        })
        .collect()
}

/// `clock archive --dedupe`: keep ONE archived design per type (the earliest archived: the first
/// row of verdicts-v1.tsv), remove the others (design, pictures, verdict and caption rows) and
/// record them in `aliases.tsv` (kept file, alias file, alias name), which the catalog lists as
/// "also found as". Returns one line per removed design.
pub fn dedupe(dir: &Path, img_dir: &Path) -> Result<Vec<String>> {
    // read_rows already keeps one row per file (repeated rows from a union merge collapse to the
    // newest), so a file can never be "a duplicate of itself"
    let rows = read_rows(dir)?;
    let keys = type_keys(dir, &rows);
    let mut kept: BTreeMap<(u32, String), Row> = BTreeMap::new();
    let mut removed: Vec<(Row, Row)> = Vec::new();
    for r in &rows {
        // an extendable family's base is its own entry: never removed, never removes another
        if Clock::load(&dir.join(&r.file)).is_ok_and(|c| c.design.meta.contains_key("extend family")) {
            continue;
        }
        let tk = (r.period, keys.get(&r.file).cloned().unwrap_or_else(|| r.type_key.clone()));
        match kept.get(&tk) {
            Some(k) if k.file != r.file => removed.push((k.clone(), r.clone())),
            Some(_) => {}
            None => {
                kept.insert(tk, r.clone());
            }
        }
    }
    let gone: BTreeSet<String> = removed.iter().map(|(_, r)| r.file.clone()).collect();
    // the tables get one row per file in any case (repairs repeated rows)
    let repeated = std::fs::read_to_string(dir.join(VERDICTS)).map(|t| t.lines().count().saturating_sub(1)).unwrap_or(0).saturating_sub(rows.len());
    write_tables(dir, &gone)?;
    let mut lines = Vec::new();
    if repeated > 0 {
        lines.push(format!("COLLAPSED {repeated} repeated verdict row(s) (one row per file, the newest kept)"));
    }
    if removed.is_empty() {
        lines.push("no duplicate types".into());
        return Ok(lines);
    }
    let mut aliases = std::fs::read_to_string(dir.join(ALIASES)).unwrap_or_else(|_| "kept\talias_file\talias_name\n".into());
    for (k, r) in &removed {
        let name = Clock::load(&dir.join(&r.file)).map(|c| c.name().to_string()).unwrap_or_else(|_| r.file.trim_end_matches(".txt").to_string());
        // what the removed twin is built with (or without) compared with the kept one (a swap the
        // type identity ignores: sticky pistons, another solid)
        let with = match (Clock::load(&dir.join(&k.file)), Clock::load(&dir.join(&r.file))) {
            (Ok(a), Ok(b)) => {
                let (ka, kb) = (a.kinds(), b.kinds());
                let extra: Vec<&str> = kb.difference(&ka).map(|s| s.as_str()).collect();
                let missing: Vec<&str> = ka.difference(&kb).map(|s| s.as_str()).collect();
                let mut parts = Vec::new();
                if !extra.is_empty() {
                    parts.push(format!("with {}", extra.join(", ")));
                }
                if !missing.is_empty() {
                    parts.push(format!("without {}", missing.join(", ")));
                }
                if parts.is_empty() { String::new() } else { format!(" (built {})", parts.join(", ")) }
            }
            _ => String::new(),
        };
        aliases.push_str(&format!("{}\t{}\t{name}{with}\n", k.file, r.file));
        let stem = r.file.trim_end_matches(".txt");
        for p in [dir.join(&r.file), img_dir.join(format!("{stem}.jpg")), img_dir.join(format!("{stem}.strip.jpg"))] {
            let _ = std::fs::remove_file(p);
        }
        lines.push(format!("REMOVED {} ({name}{with}): same type as {} (kept, archived first)", r.file, k.file));
    }
    std::fs::write(dir.join(ALIASES), aliases)?;
    Ok(lines)
}

/// Alias rows written by [`dedupe`]: kept file -> (alias file, alias name).
pub fn read_aliases(dir: &Path) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in std::fs::read_to_string(dir.join(ALIASES)).unwrap_or_default().lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 3 {
            m.entry(f[0].to_string()).or_default().push(f[2].to_string());
        }
    }
    m
}

/// Result of [`archive`].
pub struct Archived {
    pub file: PathBuf,
    pub summary: String,
    pub new: bool,
}

/// Judge a clock in the real game and keep it: a copy in the archive (header: date, source,
/// the judge's line), its verdict row, its caption. A clock already archived (same blocks, ports,
/// kick, block entities) is not copied again. `pool`: the judge's workers
/// ([`crate::verify::spawn_pool`]).
pub fn archive(pool: &mctas::sim::Pool, path: &Path, o: &VerifyOptions, dir: &Path, caption: Option<&str>) -> Result<Archived> {
    let c = Clock::load(path)?;
    let text = std::fs::read_to_string(path)?;
    let hash = content_hash(&c);
    let rows = read_rows(dir)?;
    if let Some(r) = rows.iter().find(|r| r.hash == hash) {
        return Ok(Archived { file: dir.join(&r.file), summary: r.summary.clone(), new: false });
    }
    // one archived design per TYPE and period (the structure up to translation, rotation, mirror
    // image and the swaps that keep the run identical: `ident`; the same structure can run at
    // another period in another orientation or from another start: that is another entry)
    let tk = crate::ident::type_key(&c);
    let keys = type_keys(dir, &rows);
    let same_type: Vec<&Row> = rows.iter().filter(|r| keys.get(&r.file).is_some_and(|k| *k == tk)).collect();
    let sim_period = {
        let r = crate::sim::simulate(&c, &crate::sim::SimOptions::default());
        (r.verdict == Verdict::Clock).then_some(r.period)
    };
    // (an extendable family's base is archived even when its n = base instance already is: the
    // family card is another entry, the instance leaves the gallery)
    let is_family = c.design.meta.contains_key("extend family");
    if let Some(r) = same_type.iter().find(|r| Some(r.period) == sim_period).filter(|_| !is_family) {
        return Ok(Archived { file: dir.join(&r.file), summary: format!("same type as {} (a rotation, mirror image or move of it, same period): not archived", r.file), new: false });
    }
    let rep = verify_in_pool(pool, &c, o)?;
    if rep.verdict != Verdict::Clock {
        bail!("not archived: {}", rep.summary());
    }
    if let (Some(b), true) = (rep.base(), sim_period.is_none() && !is_family) {
        if let Some(r) = same_type.iter().find(|r| r.period == b.period) {
            return Ok(Archived { file: dir.join(&r.file), summary: format!("same type as {} (a rotation, mirror image or move of it, same period): not archived", r.file), new: false });
        }
    }
    // an extendable family: n = low, low+1, low+2, the middle and the high end judged in the game
    // first (period and blocks must follow the formulas), then recorded in extend.tsv
    let family = crate::extend::Family::of(&c)?;
    let mut family_rows = Vec::new();
    if let Some(f) = &family {
        let checked = crate::extend::check(pool, path, f, &f.sample(), o)?;
        if let Some((_, _, line)) = checked.iter().find(|(_, ok, _)| !ok) {
            bail!("not archived: extend family {}: {line}", f.id);
        }
        family_rows = checked.into_iter().map(|(i, _, _)| i).collect();
    }
    let base = rep.base().unwrap().clone();
    std::fs::create_dir_all(dir)?;
    let stem: String = c.name().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' }).collect();
    let stem = if stem.is_empty() { "clock".to_string() } else { stem };
    let mut file = format!("{stem}.txt");
    // a name taken by ANOTHER structure: name + '-' + a short hash of the type (stable: the same
    // structure gets the same name on every branch); then -2, -3...
    if dir.join(&file).exists() {
        let h = tk.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        file = format!("{stem}-{:06x}.txt", h & 0xffffff);
    }
    let base_file = file.trim_end_matches(".txt").to_string();
    let mut n = 2;
    while dir.join(&file).exists() {
        file = format!("{base_file}-{n}.txt");
        n += 1;
    }
    let summary = rep.summary();
    let header = format!("# archived {} from {} by mctas clock archive\n# {summary}\n", now_utc(), path.display());
    std::fs::write(dir.join(&file), format!("{header}{text}"))?;
    let [w, h, d] = base.dims;
    let (on, edges, pulses) = base.output.as_ref().map(|o| (o.wave.on, o.wave.edges, format!("{:?}", o.wave.widths))).unwrap_or_default();
    let row = Row {
        hash,
        file: file.clone(),
        period: base.period,
        warmup: base.warmup,
        on,
        edges,
        pulses,
        family: wave::signature(&base.analysis.family),
        blocks: c.blocks(),
        dims: format!("{w}x{h}x{d}"),
        volume: (w * h * d) as i64,
        robustness: rep.robustness.clone(),
        start: c.start_label(),
        chiral: canon::chiral(c.design.blocks.iter()),
        type_key: tk,
        summary: summary.clone(),
    };
    // one row per file: a row for the same file is replaced, never repeated
    let vp = dir.join(VERDICTS);
    let mut rows_now = read_rows(dir)?;
    rows_now.retain(|r| r.file != row.file);
    rows_now.push(row.clone());
    let mut all = format!("{HEADER}\n");
    for r in &rows_now {
        all.push_str(&r.to_tsv());
        all.push('\n');
    }
    std::fs::write(&vp, all)?;
    let cap = caption.map(|s| s.to_string()).unwrap_or_else(|| default_caption(&text));
    if !cap.is_empty() {
        let cp = dir.join(CAPTIONS);
        let old = if cp.exists() { std::fs::read_to_string(&cp)? } else { "file\tcaption\n".to_string() };
        let mut caps: String = old.lines().filter(|l| l.split('\t').next() != Some(file.as_str())).map(|l| format!("{l}\n")).collect();
        caps.push_str(&format!("{file}\t{}\n", cap.replace(['\t', '\n'], " ")));
        std::fs::write(&cp, caps)?;
    }
    if !family_rows.is_empty() {
        crate::extend::record(dir, &family_rows)?;
    }
    let summary = match &family {
        Some(f) => format!("{summary}; extend family {}: P = {} gt, n = {}..{} (judged: n = {})", f.id, f.period.text(), f.n_lo, f.n_hi, family_rows.iter().map(|r| r.n.to_string()).collect::<Vec<_>>().join(", ")),
        None => summary,
    };
    Ok(Archived { file: dir.join(file), summary, new: true })
}

/// Counts written by [`catalog`].
pub struct CatalogStats {
    pub clocks: usize,
    pub types: usize,
    pub families: usize,
    pub periods: usize,
}

fn duty(r: &Row) -> String {
    format!("{}/{}", r.on, r.period)
}

/// Write the catalog and the coverage matrix from the archive's verdict rows (no game runs).
pub fn catalog(dir: &Path, out: &Path, coverage: &Path) -> Result<CatalogStats> {
    let all_rows = read_rows(dir)?;
    let caps = read_captions(dir);
    // composite (beat) clocks, marked `# composite` in their design, have their own section
    let composite: BTreeSet<String> = all_rows
        .iter()
        .filter(|r| std::fs::read_to_string(dir.join(&r.file)).unwrap_or_default().lines().any(|l| l.trim_start().starts_with("# composite")))
        .map(|r| r.file.clone())
        .collect();
    let rows: Vec<Row> = all_rows.iter().filter(|r| !composite.contains(&r.file)).cloned().collect();
    // one instance per type: the smallest (blocks, volume, then file name)
    let mut by_type: BTreeMap<(u32, String), Row> = BTreeMap::new();
    // each archived design's type (recomputed from its file: rotations and mirror images are one
    // type) and the names of the other designs of that type (aliases, also those removed by
    // `clock archive --dedupe`)
    let type_of: BTreeMap<String, String> = {
        let keys = type_keys(dir, &rows);
        rows.iter().map(|r| (r.file.clone(), keys.get(&r.file).cloned().unwrap_or_else(|| r.type_key.clone()))).collect()
    };
    let removed_aliases = read_aliases(dir);
    for r in &rows {
        let k = (r.period, type_of[&r.file].clone());
        let better = by_type.get(&k).is_none_or(|b| (r.blocks, r.volume, &r.file) < (b.blocks, b.volume, &b.file));
        if better {
            by_type.insert(k, r.clone());
        }
    }
    let aliases_of = |r: &Row| -> Vec<String> {
        let tk = &type_of[&r.file];
        let mut v: Vec<String> = rows.iter().filter(|o| o.file != r.file && o.period == r.period && &type_of[&o.file] == tk).map(|o| o.file.trim_end_matches(".txt").to_string()).collect();
        v.extend(removed_aliases.get(&r.file).cloned().unwrap_or_default());
        v
    };
    let periods: BTreeSet<u32> = rows.iter().map(|r| r.period).collect();
    // extendable families (archived base designs with `extend` headers) and their verified
    // instances (archive/extend.tsv): one card each; their instances leave the per-period gallery
    let mut fams: Vec<(Row, crate::extend::Family)> = Vec::new();
    let mut kinds_of: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in &rows {
        if let Ok(c) = Clock::load(&dir.join(&r.file)) {
            kinds_of.insert(r.file.clone(), c.kinds());
            if let Ok(Some(f)) = crate::extend::Family::of(&c) {
                fams.push((r.clone(), f));
            }
        }
    }
    fams.sort_by(|a, b| (a.1.period.eval(a.1.base), &a.1.id).cmp(&(b.1.period.eval(b.1.base), &b.1.id)));
    let instances = crate::extend::read_instances(dir);
    let fam_files: BTreeSet<String> = fams.iter().map(|(r, _)| r.file.clone()).collect();
    let is_instance = |r: &Row| -> Option<(String, i64)> {
        if fam_files.contains(&r.file) {
            return fams.iter().find(|(fr, _)| fr.file == r.file).map(|(_, f)| (f.id.clone(), f.base));
        }
        let tk = &type_of[&r.file];
        instances.iter().find(|x| x.period == r.period && &x.type_key == tk).map(|x| (x.family.clone(), x.n))
    };
    // the dominance fold (vjeux: "the left one is strictly smaller than the right one"): within a
    // period, a clock folds under a clock with strictly fewer blocks whose block kinds are a subset
    // of its own ([`fold_hosts`]); folded clocks show as one line of variants under their host (the
    // archive keeps them all)
    let mut folded: BTreeMap<String, String> = BTreeMap::new();
    for p in &periods {
        let shown: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).filter(|r| is_instance(r).is_none()).collect();
        let items: Vec<(usize, i64, String, BTreeSet<String>)> = shown.iter().map(|r| (r.blocks, r.volume, r.file.clone(), kinds_of.get(&r.file).cloned().unwrap_or_default())).collect();
        for (i, h) in fold_hosts(&items).into_iter().enumerate() {
            if let Some(h) = h {
                folded.insert(shown[i].file.clone(), shown[h].file.clone());
            }
        }
    }
    let unfolded = || by_type.values().filter(|r| !folded.contains_key(&r.file));
    let mut fam_count: BTreeMap<String, usize> = BTreeMap::new();
    for r in unfolded() {
        *fam_count.entry(r.family.clone()).or_default() += 1;
    }
    let mut families: Vec<String> = fam_count.keys().cloned().collect();
    families.sort_by(|a, b| fam_count[b].cmp(&fam_count[a]).then(a.cmp(b)));
    // best per (period, family), folded variants left out
    let mut best: BTreeMap<(u32, String), Row> = BTreeMap::new();
    for r in unfolded() {
        let k = (r.period, r.family.clone());
        if best.get(&k).is_none_or(|b| (r.blocks, r.volume) < (b.blocks, b.volume)) {
            best.insert(k, r.clone());
        }
    }
    let matrix = |link: bool| -> String {
        let mut s = String::new();
        s.push_str(&format!("| period | {} |\n", families.join(" | ")));
        s.push_str(&format!("|---|{}\n", "---|".repeat(families.len())));
        for p in &periods {
            let cells: Vec<String> = families
                .iter()
                .map(|f| match best.get(&(*p, f.clone())) {
                    Some(r) if link => format!("[{}b](archive/{})", r.blocks, r.file),
                    Some(r) => format!("{}b", r.blocks),
                    None => String::new(),
                })
                .collect();
            s.push_str(&format!("| {} | {} |\n", crate::gt_time(*p), cells.join(" | ")));
        }
        s
    };
    // CATALOG.md is the GALLERY only (vjeux: "just keep the screenshots in this page"): per period
    // (ascending) a grid of pictures, each linked to its design, with one caption line; the
    // coverage matrix lives in coverage.md, the impossibility notes in impossible.tsv.
    let out_dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let img_dir = out_dir.join("img");
    // `pre`: the path from a page to designs/clocks/ ("" for CATALOG.md, "../" for the gallery
    // pages in designs/clocks/gallery/)
    let cell = |r: &Row, pre: &str| -> String {
        let stem = r.file.trim_end_matches(".txt");
        let al = aliases_of(r);
        let also = if al.is_empty() { String::new() } else { format!("<br><sub>also found as {}</sub>", al.join(", ")) };
        let caption = format!("{} · {} · {} blocks", crate::gt_time(r.period), r.family, r.blocks);
        if img_dir.join(format!("{stem}.jpg")).exists() {
            format!("<td align=\"center\"><a href=\"{pre}archive/{}\"><img src=\"{pre}img/{stem}.jpg\" width=\"260\" alt=\"{stem}\"></a><br><sub>{caption}</sub>{also}</td>", r.file)
        } else {
            format!("<td align=\"center\"><a href=\"{pre}archive/{}\">{stem}</a><br><sub>{caption} (picture pending)</sub>{also}</td>", r.file)
        }
    };
    let grid = |rs: &[&Row], pre: &str| -> String {
        let mut s = String::from("<table>\n");
        for chunk in rs.chunks(3) {
            s.push_str("<tr>");
            for r in chunk {
                s.push_str(&cell(r, pre));
            }
            s.push_str("</tr>\n");
        }
        s.push_str("</table>\n");
        s
    };
    // CATALOG.md is the index (the coordinator, at 1963 clocks: one 549 KB page was too big): the
    // legend, the extendable families' cards, then a table of period ranges linking to the gallery
    // pages (designs/clocks/gallery/p<lo>-<hi>.md, each under ~120 KB and 300 pictures, a period
    // never split) and the composite page.
    let mut md = String::new();
    md.push_str(&format!(
        "# Clock catalog\n\n{} clock types ({} periods, {} families) pictured in the real game at the most active moment of their period, plus {} variants (a clock of the same period with fewer blocks uses a subset of their block kinds: listed under it); **orange glass = where the output signal is** (placed only for the picture); click a picture for its design. Generated by `mctas clock catalog` ([docs/clocks.md](../../docs/clocks.md); the period x family matrix: [coverage.md](coverage.md)).\n",
        by_type.len() - folded.len(),
        periods.len(),
        families.len(),
        folded.len()
    ));
    if !fams.is_empty() {
        // one card per family (vjeux: "show the 0 version and the 1 version so we know what to
        // expect; make the gt formula bigger"): the period formula as a heading, the two smallest
        // members side by side (the base design's picture, or `img/<base stem>.n<k>.jpg` taken by
        // `clock shot` from the family's generator), then the seconds, blocks, rule and n range
        md.push_str("\n## Extendable clocks\n\nA family grows by a repeated step. Each card shows its two smallest members in the real game, its period for any n, its size, how to extend it and the n range judged in the game (`mctas clock extend-check`); the members are not repeated in the period galleries.\n");
        for (r, f) in &fams {
            let stem = r.file.trim_end_matches(".txt");
            let member = |n: i64| -> String {
                let pic_stem = if n == f.base { stem.to_string() } else { format!("{stem}.n{n}") };
                let p = f.period.eval(n).max(0) as u32;
                let label = format!("<b>n = {n}</b>: {} · {} blocks", crate::gt_time(p), f.blocks.eval(n));
                if img_dir.join(format!("{pic_stem}.jpg")).exists() {
                    let img = format!("<img src=\"img/{pic_stem}.jpg\" width=\"420\" alt=\"{} n = {n}\">", f.id);
                    let img = if n == f.base { format!("<a href=\"archive/{}\">{img}</a>", r.file) } else { img };
                    format!("<td align=\"center\" valign=\"top\">{img}<br>{label}</td>")
                } else {
                    format!("<td align=\"center\" valign=\"top\">(picture pending)<br>{label}</td>")
                }
            };
            let ns: Vec<i64> = [f.n_lo, f.n_lo + 1].into_iter().filter(|n| *n <= f.n_hi).collect();
            let mut verified: Vec<i64> = instances.iter().filter(|x| x.family == f.id).map(|x| x.n).collect();
            verified.sort();
            verified.dedup();
            let checked = if verified.is_empty() { String::new() } else { format!("; judged in the game: n = {}", verified.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")) };
            let gt = |n: i64| crate::gt_time(f.period.eval(n).max(0) as u32);
            md.push_str(&format!(
                "\n### P = {} gt\n\n**{}** ([base design](archive/{})) · P = {} · {} blocks · {}\n\n<table><tr>{}</tr></table>\n\nExtend: {}.<br>\nn = {}..{}: P = {} .. {}{checked}.\n",
                f.period.text(),
                f.id,
                r.file,
                f.period.seconds(),
                f.blocks.text(),
                r.family,
                ns.iter().map(|n| member(*n)).collect::<String>(),
                f.rule.trim_end_matches('.'),
                f.n_lo,
                f.n_hi,
                gt(f.n_lo),
                gt(f.n_hi)
            ));
        }
    }
    // one section per period (on the gallery pages: paths from designs/clocks/gallery/)
    let mut sections: Vec<(u32, String, usize, usize)> = Vec::new();
    for p in &periods {
        let all: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).filter(|r| is_instance(r).is_none()).collect();
        let mut rs: Vec<&Row> = all.iter().copied().filter(|r| !folded.contains_key(&r.file)).collect();
        rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
        // one line of variants per host: name (blocks, +the kinds the host does not use)
        let mut variants = String::new();
        for h in &rs {
            let mut vs: Vec<&Row> = all.iter().copied().filter(|r| folded.get(&r.file) == Some(&h.file)).collect();
            if vs.is_empty() {
                continue;
            }
            vs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
            let hk = kinds_of.get(&h.file).cloned().unwrap_or_default();
            let items: Vec<String> = vs
                .iter()
                .map(|v| {
                    let extra: Vec<String> = kinds_of.get(&v.file).map(|k| k.difference(&hk).map(|s| format!("+{s}")).collect()).unwrap_or_default();
                    format!("[{}](../archive/{}) ({} blocks{})", v.file.trim_end_matches(".txt"), v.file, v.blocks, if extra.is_empty() { String::new() } else { format!(", {}", extra.join(", ")) })
                })
                .collect();
            variants.push_str(&format!("\n<sub>variants of **{}** ({} blocks): {}</sub>\n", h.file.trim_end_matches(".txt"), h.blocks, items.join(" · ")));
        }
        let reach: Vec<String> = fams
            .iter()
            .filter_map(|(_, f)| (f.n_lo..=f.n_hi).find(|n| f.period.eval(*n) == *p as i64).map(|n| format!("{} (n = {n})", f.id)))
            .collect();
        if rs.is_empty() && reach.is_empty() {
            continue;
        }
        let also = if reach.is_empty() { String::new() } else { format!("\nAlso reachable by: {}.\n", reach.join(", ")) };
        let text = format!("\n## {}\n{also}\n{}{variants}", crate::gt_time(*p), if rs.is_empty() { String::new() } else { grid(&rs, "../") });
        sections.push((*p, text, rs.len(), all.len() - rs.len()));
    }
    // pages: consecutive periods while a page stays under 120 KB and 300 pictures
    let mut pages: Vec<Vec<usize>> = Vec::new();
    let (mut cur, mut size, mut count): (Vec<usize>, usize, usize) = (Vec::new(), 0, 0);
    for (i, (_, text, n, _)) in sections.iter().enumerate() {
        if !cur.is_empty() && (size + text.len() > 120_000 || count + n > 300) {
            pages.push(std::mem::take(&mut cur));
            (size, count) = (0, 0);
        }
        cur.push(i);
        size += text.len();
        count += n;
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    let gallery = out_dir.join("gallery");
    std::fs::create_dir_all(&gallery)?;
    // stale pages (other ranges than this run's) go
    if let Ok(rd) = std::fs::read_dir(&gallery) {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().ends_with(".md") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    let range = |pg: &[usize]| -> (u32, u32) { (sections[pg[0]].0, sections[*pg.last().unwrap()].0) };
    let names: Vec<String> = pages
        .iter()
        .map(|pg| match range(pg) {
            (lo, hi) if lo == hi => format!("p{lo}.md"),
            (lo, hi) => format!("p{lo}-{hi}.md"),
        })
        .collect();
    let span = |lo: u32, hi: u32| -> String { if lo == hi { crate::gt_time(lo) } else { format!("{} to {}", crate::gt_time(lo), crate::gt_time(hi)) } };
    md.push_str("\n## Galleries by period\n\n| periods | clock types (+ variants listed under them) | page |\n|---|---|---|\n");
    for (k, pg) in pages.iter().enumerate() {
        let (lo, hi) = range(pg);
        let n: usize = pg.iter().map(|i| sections[*i].2).sum();
        let v: usize = pg.iter().map(|i| sections[*i].3).sum();
        md.push_str(&format!("| {} | {n}{} | [{}](gallery/{}) |\n", span(lo, hi), if v > 0 { format!(" (+{v})") } else { String::new() }, names[k].trim_end_matches(".md"), names[k]));
    }
    if !composite.is_empty() {
        md.push_str(&format!("| composite (beat) clocks | {} | [composite](gallery/composite.md) |\n", composite.len()));
    }
    for (k, pg) in pages.iter().enumerate() {
        let (lo, hi) = range(pg);
        let mut nav = String::from("[catalog index](../CATALOG.md)");
        if k > 0 {
            nav.push_str(&format!(" · previous: [{}]({})", names[k - 1].trim_end_matches(".md"), names[k - 1]));
        }
        if k + 1 < pages.len() {
            nav.push_str(&format!(" · next: [{}]({})", names[k + 1].trim_end_matches(".md"), names[k + 1]));
        }
        let n: usize = pg.iter().map(|i| sections[*i].2).sum();
        let v: usize = pg.iter().map(|i| sections[*i].3).sum();
        let mut page = format!(
            "# Clocks: {} ({n} clock types{})\n\n{nav}\n\nOrange glass = where the output signal is (placed only for the picture); click a picture for its design. A clock whose period has a smaller clock using a subset of its block kinds is listed as a variant under that clock. Generated by `mctas clock catalog`.\n",
            span(lo, hi),
            if v > 0 { format!(" + {v} variants") } else { String::new() }
        );
        for i in pg {
            page.push_str(&sections[*i].1);
        }
        page.push_str(&format!("\n{nav}\n"));
        std::fs::write(gallery.join(&names[k]), page)?;
    }
    if !composite.is_empty() {
        let rs: Vec<&Row> = all_rows.iter().filter(|r| composite.contains(&r.file)).collect();
        let page = format!("# Composite clocks\n\n[catalog index](../CATALOG.md)\n\nBeat clocks: the output combines sub-clocks that run alone (marked `# composite`).\n\n{}", grid(&rs, "../"));
        std::fs::write(gallery.join("composite.md"), page)?;
    }
    let _ = (&caps, &best, duty as fn(&Row) -> String, IMPOSSIBLE);
    std::fs::write(out, &md).with_context(|| format!("writing {}", out.display()))?;
    let cov = format!(
        "# Clock coverage: period x family (fewest blocks)\n\nGenerated by `mctas clock catalog` (designs/clocks/CATALOG.md: the index of the picture galleries by period); {} clocks, {} types shown ({} variants folded under a smaller clock of their period that uses a subset of their block kinds, left out of the matrix), {} families.\n\n{}",
        rows.len(),
        by_type.len() - folded.len(),
        folded.len(),
        families.len(),
        matrix(false)
    );
    std::fs::write(coverage, cov)?;
    // pictures of designs no longer archived (removed duplicates, family instances): pruned, so
    // img/ holds exactly the archived designs' pictures
    let mut pruned = 0;
    if let Ok(rd) = std::fs::read_dir(&img_dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let stem = name.trim_end_matches(".jpg").trim_end_matches(".strip");
            // an extendable family member's picture: <base stem>.n<k> (kept with its base)
            let stem = match stem.rsplit_once(".n") {
                Some((base, k)) if !k.is_empty() && k.chars().all(|ch| ch.is_ascii_digit()) => base,
                _ => stem,
            };
            if name.ends_with(".jpg") && !dir.join(format!("{stem}.txt")).exists() {
                if std::fs::remove_file(e.path()).is_ok() {
                    pruned += 1;
                }
            }
        }
    }
    if pruned > 0 {
        eprintln!("pruned {pruned} picture(s) of designs no longer in the archive");
    }
    Ok(CatalogStats { clocks: rows.len(), types: by_type.len(), families: families.len(), periods: periods.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(file: &str, summary: &str) -> String {
        format!("h\t{file}\t6\t0\t2\t1\t[2]\tobserver\t2\t2x1x1\t2\trobust\tkick: last 1,0,0\tk\tfalse\t{summary}")
    }

    /// `Motion` turns with the design (the game turns `Rotation`).
    #[test]
    fn motion_turns() {
        assert_eq!(crate::turn_motion("{Motion:[0.4d,0.0d,0.0d]}", 1), "{Motion:[0.0d,0.0d,0.4d]}");
        assert_eq!(crate::turn_motion("{Motion:[0.4d,0.0d,0.0d]}", 2), "{Motion:[-0.4d,0.0d,0.0d]}");
        assert_eq!(crate::turn_motion("{Invulnerable:1b}", 1), "{Invulnerable:1b}");
    }

    /// vjeux's dominance examples (2026-10-19): the observer pair (2 blocks) folds the observer +
    /// note block clock (3); the sticky piston + slime + redstone block clock (3) folds the same
    /// with a note block (4); a clock with a kind the smaller one lacks is not folded by it.
    #[test]
    fn dominance_fold() {
        let k = |v: &[&str]| -> BTreeSet<String> { v.iter().map(|s| s.to_string()).collect() };
        let items = vec![
            (2, 2, "clk-obs-obs2".to_string(), k(&["observer"])),
            (3, 3, "census-p6-note_block-observer-3b".to_string(), k(&["note_block", "observer"])),
            (3, 4, "census-p6-redstone_block-sticky_piston-3b".to_string(), k(&["redstone_block", "slime_block", "sticky_piston"])),
            (4, 6, "census-p6-note_block-redstone_block-sticky_piston-4b".to_string(), k(&["note_block", "redstone_block", "slime_block", "sticky_piston"])),
            (3, 3, "lamp-pair".to_string(), k(&["redstone_lamp", "redstone_block"])),
        ];
        let hosts = fold_hosts(&items);
        assert_eq!(hosts, vec![None, Some(0), None, Some(2), None]);
    }

    /// A union merge repeated a file's rows: dedupe collapses them and never removes a file as a
    /// duplicate of itself (coordinator's bug, 2026-10-19: 167 archived clocks deleted); a real
    /// duplicate (the same structure turned) is still removed.
    #[test]
    fn dedupe_repeated_rows() {
        let dir = std::env::temp_dir().join(format!("clock-dedupe-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let pair_x = "name: a\nport start: 1 0 0 last\npalette:\n  E minecraft:observer[facing=east]\n  W minecraft:observer[facing=west]\nlayer 0:\n  EW\n";
        let pair_z = "name: b\nport start: 0 0 1 last\npalette:\n  S minecraft:observer[facing=south]\n  N minecraft:observer[facing=north]\nlayer 0:\n  S\n  N\n";
        std::fs::write(dir.join("a.txt"), pair_x).unwrap();
        std::fs::write(dir.join("b.txt"), pair_z).unwrap();
        std::fs::write(dir.join(VERDICTS), format!("{HEADER}\n{}\n{}\n{}\n", row("a.txt", "old"), row("a.txt", "new"), row("b.txt", "b"))).unwrap();
        std::fs::write(dir.join(CAPTIONS), "file\tcaption\na.txt\tone\na.txt\ttwo\nb.txt\tthree\n").unwrap();
        let rows = read_rows(&dir).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].summary, "new");
        let lines = dedupe(&dir, &dir.join("img")).unwrap();
        assert!(dir.join("a.txt").exists(), "{lines:?}");
        assert!(!dir.join("b.txt").exists(), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("COLLAPSED 1")), "{lines:?}");
        assert!(lines.iter().any(|l| l.starts_with("REMOVED b.txt")), "{lines:?}");
        let v = std::fs::read_to_string(dir.join(VERDICTS)).unwrap();
        assert_eq!(v.lines().count(), 2, "{v}");
        let c = std::fs::read_to_string(dir.join(CAPTIONS)).unwrap();
        assert_eq!(c, "file\tcaption\na.txt\ttwo\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
