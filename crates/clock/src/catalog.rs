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

/// The type key of an archived design, from its file (the current canonical form; rows written by
/// older versions kept an older key), else the row's.
pub fn archived_type_key(dir: &Path, r: &Row) -> String {
    match Clock::load(&dir.join(&r.file)) {
        Ok(c) => canon::design_type_key(&c.design),
        Err(_) => r.type_key.clone(),
    }
}

/// `clock archive --dedupe`: keep ONE archived design per type (the earliest archived: the first
/// row of verdicts-v1.tsv), remove the others (design, pictures, verdict and caption rows) and
/// record them in `aliases.tsv` (kept file, alias file, alias name), which the catalog lists as
/// "also found as". Returns one line per removed design.
pub fn dedupe(dir: &Path, img_dir: &Path) -> Result<Vec<String>> {
    // read_rows already keeps one row per file (repeated rows from a union merge collapse to the
    // newest), so a file can never be "a duplicate of itself"
    let rows = read_rows(dir)?;
    let mut kept: BTreeMap<(u32, String), Row> = BTreeMap::new();
    let mut removed: Vec<(Row, Row)> = Vec::new();
    for r in &rows {
        // an extendable family's base is its own entry: never removed, never removes another
        if Clock::load(&dir.join(&r.file)).is_ok_and(|c| c.design.meta.contains_key("extend family")) {
            continue;
        }
        let tk = (r.period, archived_type_key(dir, r));
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
        aliases.push_str(&format!("{}\t{}\t{name}\n", k.file, r.file));
        let stem = r.file.trim_end_matches(".txt");
        for p in [dir.join(&r.file), img_dir.join(format!("{stem}.jpg")), img_dir.join(format!("{stem}.strip.jpg"))] {
            let _ = std::fs::remove_file(p);
        }
        lines.push(format!("REMOVED {} ({name}): same type as {} (kept, archived first)", r.file, k.file));
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
    // one archived design per TYPE and period (the structure up to translation, rotation and mirror
    // image; the same structure can run at another period in another orientation or from another
    // start: that is another catalog entry)
    let tk = canon::design_type_key(&c.design);
    let same_type: Vec<&Row> = rows.iter().filter(|r| archived_type_key(dir, r) == tk).collect();
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
    let type_of: BTreeMap<String, String> = rows.iter().map(|r| (r.file.clone(), archived_type_key(dir, r))).collect();
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
    let mut fam_count: BTreeMap<String, usize> = BTreeMap::new();
    for r in by_type.values() {
        *fam_count.entry(r.family.clone()).or_default() += 1;
    }
    let mut families: Vec<String> = fam_count.keys().cloned().collect();
    families.sort_by(|a, b| fam_count[b].cmp(&fam_count[a]).then(a.cmp(b)));
    // best per (period, family)
    let mut best: BTreeMap<(u32, String), Row> = BTreeMap::new();
    for r in by_type.values() {
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
    let cell = |r: &Row| -> String {
        let stem = r.file.trim_end_matches(".txt");
        let al = aliases_of(r);
        let also = if al.is_empty() { String::new() } else { format!("<br><sub>also found as {}</sub>", al.join(", ")) };
        let caption = format!("{} · {} · {} blocks", crate::gt_time(r.period), r.family, r.blocks);
        if img_dir.join(format!("{stem}.jpg")).exists() {
            format!("<td align=\"center\"><a href=\"archive/{}\"><img src=\"img/{stem}.jpg\" width=\"260\" alt=\"{stem}\"></a><br><sub>{caption}</sub>{also}</td>", r.file)
        } else {
            format!("<td align=\"center\"><a href=\"archive/{}\">{stem}</a><br><sub>{caption} (picture pending)</sub>{also}</td>", r.file)
        }
    };
    let grid = |rs: &[&Row]| -> String {
        let mut s = String::from("<table>\n");
        for chunk in rs.chunks(3) {
            s.push_str("<tr>");
            for r in chunk {
                s.push_str(&cell(r));
            }
            s.push_str("</tr>\n");
        }
        s.push_str("</table>\n");
        s
    };
    let mut md = String::new();
    md.push_str(&format!(
        "# Clock gallery\n\n{} clocks ({} periods, {} families), each in the real game at the most active moment of its period; **orange glass = where the output signal is** (placed only for the picture). Click a picture for its design. Generated by `mctas clock catalog` ([docs/clocks.md](../../docs/clocks.md); the period x family matrix: [coverage.md](coverage.md)).\n",
        by_type.len(),
        periods.len(),
        families.len()
    ));
    // extendable families (archived base designs with `extend` headers) and their verified
    // instances (archive/extend.tsv): one card each; their instances leave the per-period gallery
    let mut fams: Vec<(Row, crate::extend::Family)> = Vec::new();
    for r in &rows {
        if let Ok(c) = Clock::load(&dir.join(&r.file)) {
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
    if !fams.is_empty() {
        md.push_str("\n## Extendable clocks\n\nOne card per family: the base design, its period and size for any n, how to extend it, the n range verified in the game (`mctas clock extend-check`); their instances are not repeated in the periods below.\n\n<table>\n");
        for chunk in fams.chunks(2) {
            md.push_str("<tr>");
            for (r, f) in chunk {
                let stem = r.file.trim_end_matches(".txt");
                let pic = if img_dir.join(format!("{stem}.jpg")).exists() { format!("<a href=\"archive/{}\"><img src=\"img/{stem}.jpg\" width=\"360\" alt=\"{stem}\"></a><br>", r.file) } else { format!("<a href=\"archive/{}\">{stem}</a> (picture pending)<br>", r.file) };
                let verified: Vec<i64> = instances.iter().filter(|x| x.family == f.id).map(|x| x.n).collect();
                let checked = if verified.is_empty() { String::new() } else { format!(" (judged in the game: n = {})", verified.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")) };
                md.push_str(&format!(
                    "<td align=\"center\" valign=\"top\">{pic}<b>{}</b><br><sub>P = {} gt ({}) · {} blocks · {}<br>extend: {}<br>n = {}..{}: P = {}..{} gt{checked}</sub></td>",
                    f.id,
                    f.period.text(),
                    f.period.seconds(),
                    f.blocks.text(),
                    r.family,
                    f.rule,
                    f.n_lo,
                    f.n_hi,
                    f.period.eval(f.n_lo),
                    f.period.eval(f.n_hi)
                ));
            }
            md.push_str("</tr>\n");
        }
        md.push_str("</table>\n");
    }
    for p in &periods {
        let mut rs: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).filter(|r| is_instance(r).is_none()).collect();
        rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
        let reach: Vec<String> = fams
            .iter()
            .filter_map(|(_, f)| (f.n_lo..=f.n_hi).find(|n| f.period.eval(*n) == *p as i64).map(|n| format!("{} (n = {n})", f.id)))
            .collect();
        if rs.is_empty() && reach.is_empty() {
            continue;
        }
        let also = if reach.is_empty() { String::new() } else { format!("\nAlso reachable by: {}.\n", reach.join(", ")) };
        md.push_str(&format!("\n## {}\n{also}\n{}", crate::gt_time(*p), if rs.is_empty() { String::new() } else { grid(&rs) }));
    }
    if !composite.is_empty() {
        let rs: Vec<&Row> = all_rows.iter().filter(|r| composite.contains(&r.file)).collect();
        md.push_str(&format!("\n## Composite clocks\n\nBeat clocks: the output combines sub-clocks that run alone (marked `# composite`).\n\n{}", grid(&rs)));
    }
    let _ = (&caps, &best, duty as fn(&Row) -> String, IMPOSSIBLE);
    std::fs::write(out, &md).with_context(|| format!("writing {}", out.display()))?;
    let cov = format!(
        "# Clock coverage: period x family (fewest blocks)\n\nGenerated by `mctas clock catalog` (designs/clocks/CATALOG.md has every type, the boxes and the links); {} clocks, {} types, {} families.\n\n{}",
        rows.len(),
        by_type.len(),
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
