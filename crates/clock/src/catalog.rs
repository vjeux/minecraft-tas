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
    let tk = canon::type_key(c.design.blocks.iter());
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
        chiral: canon::mirror_type_key(c.design.blocks.iter()) != tk,
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
    let rows = read_rows(dir)?;
    let caps = read_captions(dir);
    // one instance per type: the smallest (blocks, volume, then file name)
    let mut by_type: BTreeMap<(u32, String), Row> = BTreeMap::new();
    for r in &rows {
        let k = (r.period, r.type_key.clone());
        let better = by_type.get(&k).is_none_or(|b| (r.blocks, r.volume, &r.file) < (b.blocks, b.volume, &b.file));
        if better {
            by_type.insert(k, r.clone());
        }
    }
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
    md.push_str("\n## By period\n");
    for p in &periods {
        let types: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).collect();
        let fams: BTreeSet<&str> = types.iter().map(|r| r.family.as_str()).collect();
        md.push_str(&format!("\n### P = {p} gt: {} type{}, {} famil{}\n\n", types.len(), if types.len() == 1 { "" } else { "s" }, fams.len(), if fams.len() == 1 { "y" } else { "ies" }));
        md.push_str("| family | blocks | box | volume | duty | edges | start | judge | design | how it works |\n|---|---|---|---|---|---|---|---|---|---|\n");
        let mut sorted = types.clone();
        sorted.sort_by(|a, b| (a.family.as_str(), a.blocks, a.volume).cmp(&(b.family.as_str(), b.blocks, b.volume)));
        for r in sorted {
            let smallest = best.get(&(*p, r.family.clone())).is_some_and(|b| b.hash == r.hash);
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} | {} | {}{} | [{}](archive/{}) | {} |\n",
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
                caps.get(&r.file).cloned().unwrap_or_default().replace('|', "/")
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
