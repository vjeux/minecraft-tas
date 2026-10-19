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
    Ok(std::fs::read_to_string(&p)?.lines().filter_map(Row::from_tsv).collect())
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
    let rows = read_rows(dir)?;
    let mut kept: BTreeMap<String, Row> = BTreeMap::new();
    let mut removed: Vec<(Row, Row)> = Vec::new();
    for r in &rows {
        let tk = archived_type_key(dir, r);
        match kept.get(&tk) {
            Some(k) => removed.push((k.clone(), r.clone())),
            None => {
                kept.insert(tk, r.clone());
            }
        }
    }
    if removed.is_empty() {
        return Ok(vec!["no duplicate types".into()]);
    }
    let gone: BTreeSet<String> = removed.iter().map(|(_, r)| r.file.clone()).collect();
    let mut lines = Vec::new();
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
    // rewrite the verdict and caption files without the removed designs
    let vp = dir.join(VERDICTS);
    let text = std::fs::read_to_string(&vp)?;
    let kept_lines: Vec<&str> = text.lines().filter(|l| l.split('\t').nth(1).is_none_or(|f| !gone.contains(f))).collect();
    std::fs::write(&vp, kept_lines.join("\n") + "\n")?;
    let cp = dir.join(CAPTIONS);
    if let Ok(text) = std::fs::read_to_string(&cp) {
        let kept_lines: Vec<&str> = text.lines().filter(|l| l.split('\t').next().is_none_or(|f| !gone.contains(f))).collect();
        std::fs::write(&cp, kept_lines.join("\n") + "\n")?;
    }
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
    // one archived design per TYPE (the structure up to translation, rotation and mirror image)
    let tk = canon::design_type_key(&c.design);
    if let Some(r) = rows.iter().find(|r| archived_type_key(dir, r) == tk) {
        return Ok(Archived { file: dir.join(&r.file), summary: format!("same type as {} (a rotation, mirror image or move of it): not archived", r.file), new: false });
    }
    let rep = verify_in_pool(pool, &c, o)?;
    if rep.verdict != Verdict::Clock {
        bail!("not archived: {}", rep.summary());
    }
    let base = rep.base().unwrap().clone();
    std::fs::create_dir_all(dir)?;
    let stem: String = c.name().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' }).collect();
    let stem = if stem.is_empty() { "clock".to_string() } else { stem };
    let mut file = format!("{stem}.txt");
    let mut n = 2;
    while dir.join(&file).exists() {
        file = format!("{stem}-{n}.txt");
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
    let vp = dir.join(VERDICTS);
    let mut all = if vp.exists() { std::fs::read_to_string(&vp)? } else { format!("{HEADER}\n") };
    all.push_str(&row.to_tsv());
    all.push('\n');
    std::fs::write(&vp, all)?;
    let cap = caption.map(|s| s.to_string()).unwrap_or_else(|| default_caption(&text));
    if !cap.is_empty() {
        let cp = dir.join(CAPTIONS);
        let mut caps = if cp.exists() { std::fs::read_to_string(&cp)? } else { "file\tcaption\n".to_string() };
        caps.push_str(&format!("{file}\t{}\n", cap.replace(['\t', '\n'], " ")));
        std::fs::write(&cp, caps)?;
    }
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
        let mut v: Vec<String> = rows.iter().filter(|o| o.file != r.file && &type_of[&o.file] == tk).map(|o| o.file.trim_end_matches(".txt").to_string()).collect();
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
        s.push_str(&format!("| P (gt) | {} |\n", families.join(" | ")));
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
            s.push_str(&format!("| {p} | {} |\n", cells.join(" | ")));
        }
        s
    };
    let mut md = String::new();
    md.push_str("# Clock catalog\n\nGenerated by `mctas clock catalog` from `designs/clocks/archive/` (verdicts-v1.tsv, captions.tsv): never edit by hand. What a clock is and how it is judged: [docs/clocks.md](../../docs/clocks.md).\n\n");
    md.push_str(&format!(
        "{} archived clocks, {} distinct types (up to translation and rotation), {} families, {} periods.\n\n",
        rows.len(),
        by_type.len(),
        families.len(),
        periods.len()
    ));
    md.push_str("## Coverage: period x family (fewest blocks)\n\n");
    md.push_str(&matrix(true));
    let imp = std::fs::read_to_string(IMPOSSIBLE).unwrap_or_default();
    let imp_rows: Vec<Vec<&str>> = imp.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("family\t") && !l.starts_with('#')).map(|l| l.split('\t').collect()).filter(|v: &Vec<&str>| v.len() >= 3).collect();
    md.push_str("\n## Periods a family cannot reach\n\n");
    if imp_rows.is_empty() {
        md.push_str("None recorded yet (`designs/clocks/impossible.tsv`: family, periods, why: a reason from the source or the census that proves it).\n");
    } else {
        md.push_str("| family | periods | why |\n|---|---|---|\n");
        for v in imp_rows {
            md.push_str(&format!("| {} | {} | {} |\n", v[0], v[1], v[2]));
        }
    }
    // pictures (`clock shot`): designs/clocks/img/<archive file stem>.jpg, linked relative to the
    // catalog's folder
    let out_dir = out.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let img_dir = out_dir.join("img");
    let pic = |file: &str, width: u32| -> String {
        let stem = file.trim_end_matches(".txt");
        if img_dir.join(format!("{stem}.jpg")).exists() { format!("<img src=\"img/{stem}.jpg\" width=\"{width}\">") } else { String::new() }
    };
    let strip = |file: &str| -> String {
        let stem = file.trim_end_matches(".txt");
        if img_dir.join(format!("{stem}.strip.jpg")).exists() { format!(" [motion](img/{stem}.strip.jpg)") } else { String::new() }
    };
    md.push_str("\n## Gallery\n\nEvery archived clock in the real game (`mctas clock shot`: the most active moment of a period), by period; click a picture for its design.\n");
    for p in &periods {
        let mut rs: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).collect();
        rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
        let cells: Vec<String> = rs
            .iter()
            .filter_map(|r| {
                let stem = r.file.trim_end_matches(".txt");
                let al = aliases_of(r);
                let also = if al.is_empty() { String::new() } else { format!("; also found as {}", al.join(", ")) };
                img_dir.join(format!("{stem}.jpg")).exists().then(|| {
                    format!("<a href=\"archive/{}\"><img src=\"img/{stem}.jpg\" width=\"260\" title=\"P {} gt, {}, {} blocks, {}{also}\"></a>", r.file, r.period, r.family, r.blocks, r.dims)
                })
            })
            .collect();
        if !cells.is_empty() {
            md.push_str(&format!("\n**P = {p} gt**\n\n{}\n", cells.join("\n")));
        }
    }
    if !composite.is_empty() {
        md.push_str("\n## Composite clocks (beat clocks: the output combines sub-clocks that run alone)\n\nMarked `# composite` in their design; not in the coverage matrix.\n\n| period | family | blocks | box | picture | design |\n|---|---|---|---|---|---|\n");
        for r in all_rows.iter().filter(|r| composite.contains(&r.file)) {
            md.push_str(&format!("| {} | {} | {} | {} | {} | [{}](archive/{}) |\n", r.period, r.family, r.blocks, r.dims, pic(&r.file, 160), r.file.trim_end_matches(".txt"), r.file));
        }
    }
    md.push_str("\n## By period\n");
    for p in &periods {
        let types: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).collect();
        let fams: BTreeSet<&str> = types.iter().map(|r| r.family.as_str()).collect();
        md.push_str(&format!("\n### P = {p} gt: {} type{}, {} famil{}\n\n", types.len(), if types.len() == 1 { "" } else { "s" }, fams.len(), if fams.len() == 1 { "y" } else { "ies" }));
        md.push_str("| picture | family | blocks | box | volume | duty | edges | start | judge | design | how it works |\n|---|---|---|---|---|---|---|---|---|---|---|\n");
        let mut sorted = types.clone();
        sorted.sort_by(|a, b| (a.family.as_str(), a.blocks, a.volume).cmp(&(b.family.as_str(), b.blocks, b.volume)));
        for r in sorted {
            let smallest = best.get(&(*p, r.family.clone())).is_some_and(|b| b.hash == r.hash);
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {}{} | [{}](archive/{}){} | {} |\n",
                pic(&r.file, 200),
                if smallest { format!("**{}**", r.family) } else { r.family.clone() },
                r.blocks,
                r.dims,
                r.volume,
                duty(r),
                r.edges,
                r.start,
                r.robustness,
                if r.chiral { ", chiral" } else { "" },
                r.file.trim_end_matches(".txt"),
                r.file,
                strip(&r.file),
                {
                    let al = aliases_of(r);
                    let cap = caps.get(&r.file).cloned().unwrap_or_default().replace('|', "/");
                    if al.is_empty() { cap } else { format!("{cap} (also found as {})", al.join(", ")) }
                }
            ));
        }
    }
    std::fs::write(out, &md).with_context(|| format!("writing {}", out.display()))?;
    let cov = format!(
        "# Clock coverage: period x family (fewest blocks)\n\nGenerated by `mctas clock catalog` (designs/clocks/CATALOG.md has every type, the boxes and the links); {} clocks, {} types, {} families.\n\n{}",
        rows.len(),
        by_type.len(),
        families.len(),
        matrix(false)
    );
    std::fs::write(coverage, cov)?;
    Ok(CatalogStats { clocks: rows.len(), types: by_type.len(), families: families.len(), periods: periods.len() })
}
