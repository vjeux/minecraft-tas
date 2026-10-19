//! The archive and the catalog of the contraption studies. `mctas circuit archive` judges a design
//! in the real game and keeps it in `designs/circuits/<study>/archive/` (the design with its judge
//! line, a row of `verdicts.tsv`, a caption in `captions.tsv`); `mctas circuit catalog` writes, per
//! study, `CATALOG.md` (by the study's axis, smallest first; a design that only adds block kinds to
//! a smaller one with the same axis value is a variant, listed on `variants.md`), the index
//! `designs/circuits/CATALOG.md`, and the data the website renders: `designs/circuits/catalog.tsv`.

use crate::check::Outcome;
use crate::verify::{self, Options};
use crate::{Circuit, Kind};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::Pool;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const VERDICTS: &str = "verdicts.tsv";
pub const CAPTIONS: &str = "captions.tsv";
const HEADER: &str = "archived\tfile\tkind\taxis\tmetrics\tblocks\tdims\tvolume\tfamily\trobustness\tkinds\ttype_key\thash\tsummary";

/// The studies, in order, with their titles.
pub const STUDIES: [(&str, &str); 7] = [
    ("pulse", "Pulse circuits"),
    ("tff", "T flip-flops"),
    ("delay", "Delay lines"),
    ("gates", "Logic gates"),
    ("memory", "Memory"),
    ("pext", "Piston extenders"),
    ("fly", "Flying machines"),
];

/// One archived circuit (a row of `verdicts.tsv`).
#[derive(Clone, Debug)]
pub struct Row {
    pub archived: String,
    pub file: String,
    pub kind: String,
    /// The study's axis value (`L = 4 gt`, `xor`, `N = 3`...).
    pub axis: String,
    pub metrics: String,
    pub blocks: usize,
    pub dims: String,
    pub volume: i64,
    /// The kinds of the blocks whose state changes during the run.
    pub family: String,
    pub robustness: String,
    /// The block kinds it is built with (`+`-joined), for the dominance fold.
    pub kinds: String,
    pub type_key: String,
    pub hash: String,
    pub summary: String,
}

impl Row {
    fn to_tsv(&self) -> String {
        [
            self.archived.clone(),
            self.file.clone(),
            self.kind.clone(),
            self.axis.clone(),
            self.metrics.clone(),
            self.blocks.to_string(),
            self.dims.clone(),
            self.volume.to_string(),
            self.family.clone(),
            self.robustness.clone(),
            self.kinds.clone(),
            self.type_key.clone(),
            self.hash.clone(),
            self.summary.clone(),
        ]
        .map(|s| s.replace(['\t', '\n'], " "))
        .join("\t")
    }

    fn parse(line: &str) -> Option<Row> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 14 {
            return None;
        }
        Some(Row {
            archived: f[0].into(),
            file: f[1].into(),
            kind: f[2].into(),
            axis: f[3].into(),
            metrics: f[4].into(),
            blocks: f[5].parse().ok()?,
            dims: f[6].into(),
            volume: f[7].parse().unwrap_or(0),
            family: f[8].into(),
            robustness: f[9].into(),
            kinds: f[10].into(),
            type_key: f[11].into(),
            hash: f[12].into(),
            summary: f[13].into(),
        })
    }

    fn kind_set(&self) -> BTreeSet<String> {
        self.kinds.split('+').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    }
}

/// A study's archive directory.
pub fn archive_dir(root: &Path, study: &str) -> PathBuf {
    root.join(study).join("archive")
}

/// The rows of an archive, one per file (a repeated file keeps its newest row).
pub fn read_rows(dir: &Path) -> Result<Vec<Row>> {
    let p = dir.join(VERDICTS);
    if !p.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    let mut rows: Vec<Row> = Vec::new();
    for r in text.lines().skip(1).filter_map(Row::parse) {
        rows.retain(|o| o.file != r.file);
        rows.push(r);
    }
    Ok(rows)
}

fn write_rows(dir: &Path, rows: &[Row]) -> Result<()> {
    let mut all = format!("{HEADER}\n");
    for r in rows {
        all.push_str(&r.to_tsv());
        all.push('\n');
    }
    std::fs::write(dir.join(VERDICTS), all)?;
    Ok(())
}

fn read_captions(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(CAPTIONS))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once('\t'))
        .map(|(f, c)| (f.to_string(), c.to_string()))
        .collect()
}

/// The structure's type key: the design up to translation, the 4 rotations and the mirror image
/// (`clock::canon::design_type_key`), with its input cells marked (whatever drives them: a lever
/// and a redstone-block input are the same circuit) and its output and payload cells marked.
pub fn type_key(c: &Circuit) -> String {
    let mut d = c.clock.design.clone();
    for i in &c.inputs {
        d.blocks.insert(i.pos, "circuit:input".into());
        d.block_entities.remove(&i.pos);
    }
    for (tag, probes) in [("circuit:output", &c.outputs), ("circuit:payload", &c.payloads)] {
        for p in probes {
            d.entities.push(mctas::design::EntitySpec { kind: tag.into(), pos: [p.pos[0] as f64 + 0.5, p.pos[1] as f64, p.pos[2] as f64 + 0.5], nbt: None });
        }
    }
    clock::canon::design_type_key(&d)
}

/// A circuit's value on its study's axis.
pub fn axis(kind: &Kind, out: &Outcome, family: &str) -> String {
    let m = |k: &str| out.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
    match kind {
        Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => format!("L = {}", m("L")),
        Kind::Delay => format!("N = {}", m("N")),
        Kind::Gate(_) => m("function"),
        Kind::Tff => family.to_string(),
        Kind::RsLatch | Kind::DLatch => kind.name(),
        Kind::Counter(n) => format!("counter {n}"),
        Kind::PistonExtender(n) => format!("N = {n}"),
        Kind::FlyingMachine => {
            let s = m("speed");
            if s.is_empty() { "two-way".into() } else { s }
        }
    }
}

/// Result of [`archive`].
pub struct Archived {
    pub file: PathBuf,
    pub summary: String,
    pub new: bool,
}

/// `mctas circuit archive`: the judge, then the design kept in its study's archive. Refused: a
/// failing design, or one whose structure (type key) is already archived as the same kind.
pub fn archive(pool: &Pool, path: &Path, o: &Options, root: &Path, caption: Option<&str>) -> Result<Archived> {
    let c = Circuit::load(path)?;
    let kind = c.kind.clone().ok_or_else(|| anyhow!("{}: no `circuit: <kind>` line", path.display()))?;
    let dir = archive_dir(root, kind.study());
    let text = std::fs::read_to_string(path)?;
    // the same blocks and ports under another kind (a claim) are another entry
    let hash = format!("{}-{}", clock::catalog::content_hash(&c.clock), kind.name().replace(' ', "_"));
    let rows = read_rows(&dir)?;
    if let Some(r) = rows.iter().find(|r| r.hash == hash) {
        return Ok(Archived { file: dir.join(&r.file), summary: r.summary.clone(), new: false });
    }
    let tk = type_key(&c);
    if let Some(r) = rows.iter().find(|r| r.type_key == tk && r.kind == kind.name()) {
        return Ok(Archived { file: dir.join(&r.file), summary: format!("same type as {} (a rotation, mirror image or move of it, same kind): not archived", r.file), new: false });
    }
    let rep = verify::verify_in_pool(pool, &c, o)?;
    if !rep.pass {
        bail!("not archived: {}", rep.summary(&c));
    }
    let base = rep.base().ok_or_else(|| anyhow!("no passing build"))?.clone();
    std::fs::create_dir_all(&dir)?;
    let stem: String = c.name().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' }).collect();
    let stem = if stem.is_empty() { "circuit".to_string() } else { stem };
    let mut file = format!("{stem}.txt");
    // a name taken by ANOTHER structure: name + '-' + a short hash of the type, then -2, -3...
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
    let summary = rep.summary(&c);
    let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
    let now = clock::catalog::now_utc();
    std::fs::write(dir.join(&file), format!("# archived {now} from {} by mctas circuit archive\n# {summary}\n{body}", path.display()))?;
    let [w, h, d] = base.dims;
    let row = Row {
        archived: now,
        file: file.clone(),
        kind: kind.name(),
        axis: axis(&kind, &base.outcome, &base.family),
        metrics: base.outcome.metrics_text(),
        blocks: c.blocks(),
        dims: format!("{w}x{h}x{d}"),
        volume: (w * h * d) as i64,
        family: base.family.clone(),
        robustness: rep.verdict.clone(),
        kinds: c.kinds().into_iter().collect::<Vec<_>>().join("+"),
        type_key: tk,
        hash,
        summary: summary.clone(),
    };
    let mut now_rows = read_rows(&dir)?;
    now_rows.retain(|r| r.file != row.file);
    now_rows.push(row);
    write_rows(&dir, &now_rows)?;
    let cap = caption.map(|s| s.to_string()).unwrap_or_else(|| clock::catalog::default_caption(&body));
    if !cap.is_empty() {
        let cp = dir.join(CAPTIONS);
        let old = if cp.exists() { std::fs::read_to_string(&cp)? } else { "file\tcaption\n".to_string() };
        let mut caps: String = old.lines().filter(|l| l.split('\t').next() != Some(file.as_str())).map(|l| format!("{l}\n")).collect();
        caps.push_str(&format!("{file}\t{}\n", cap.replace(['\t', '\n'], " ")));
        std::fs::write(&cp, caps)?;
    }
    Ok(Archived { file: dir.join(file), summary, new: true })
}

/// The sort key of an axis value: its first number (if any), then the text.
fn axis_key(a: &str) -> (i64, String) {
    let digits: String = a.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(i64::MAX), a.to_string())
}

/// `mctas circuit catalog`: every study's CATALOG.md and variants.md, the index and catalog.tsv,
/// from the archives' rows (no game). Returns one line per study.
pub fn catalog(root: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    let mut index = String::from(
        "# Contraption studies\n\nEvery design judged in the real game (`mctas circuit verify`: 4 positions x 4 rotations, [docs/circuits.md](../../docs/circuits.md)), by study; the data the website renders is [catalog.tsv](catalog.tsv). Generated by `mctas circuit catalog`.\n\n| study | designs | shown | variants |\n|---|---|---|---|\n",
    );
    let mut tsv = String::from("study\tkind\taxis\tname\tpath\tmetrics\tblocks\tdims\tfamily\trobustness\tvariant_of\tcaption\n");
    for (study, title) in STUDIES {
        let dir = archive_dir(root, study);
        let rows = read_rows(&dir)?;
        let caps = read_captions(&dir);
        // the dominance fold within one kind and axis value: a design with more blocks that only
        // adds block kinds to a smaller one is its variant
        let mut groups: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
        for r in &rows {
            groups.entry((r.kind.clone(), r.axis.clone())).or_default().push(r);
        }
        let mut host: BTreeMap<String, String> = BTreeMap::new();
        for g in groups.values() {
            let items: Vec<(usize, i64, String, BTreeSet<String>)> = g.iter().map(|r| (r.blocks, r.volume, r.file.clone(), r.kind_set())).collect();
            for (i, h) in clock::catalog::fold_hosts(&items).into_iter().enumerate() {
                if let Some(h) = h {
                    host.insert(g[i].file.clone(), g[h].file.clone());
                }
            }
        }
        let shown: Vec<&Row> = rows.iter().filter(|r| !host.contains_key(&r.file)).collect();
        let sdir = root.join(study);
        if rows.is_empty() {
            index.push_str(&format!("| {title} | 0 | 0 | 0 |\n"));
            continue;
        }
        std::fs::create_dir_all(&sdir)?;
        // by axis value (numbers ascending), then the smallest first
        let mut by_axis: BTreeMap<(i64, String), Vec<&Row>> = BTreeMap::new();
        for r in &shown {
            by_axis.entry(axis_key(&r.axis)).or_default().push(r);
        }
        let mut md = format!(
            "# {title}\n\n{} designs judged in the real game: {} shown, {} variants (more blocks that only add block kinds to a smaller design with the same value: [variants](variants.md)). [All studies](../CATALOG.md) · [docs/circuits.md](../../../docs/circuits.md). Generated by `mctas circuit catalog`.\n",
            rows.len(),
            shown.len(),
            rows.len() - shown.len()
        );
        for ((_, ax), rs) in &mut by_axis {
            rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
            md.push_str(&format!("\n## {ax}\n\n| design | kind | metrics | blocks | box | robustness | how it works |\n|---|---|---|---|---|---|---|\n"));
            for r in rs.iter() {
                md.push_str(&format!(
                    "| [{}](archive/{}) | {} | {} | {} | {} | {} | {} |\n",
                    r.file.trim_end_matches(".txt"),
                    r.file,
                    r.kind,
                    r.metrics,
                    r.blocks,
                    r.dims,
                    r.robustness,
                    caps.get(&r.file).map(|s| s.replace('|', "/")).unwrap_or_default()
                ));
            }
        }
        std::fs::write(sdir.join("CATALOG.md"), md)?;
        let mut vmd = format!("# {title}: variants\n\n[{title}](CATALOG.md)\n\nA variant has the same kind and value as a smaller design (its host) and uses every block kind the host does, plus others.\n");
        let mut hosts: BTreeMap<&String, Vec<&Row>> = BTreeMap::new();
        for r in &rows {
            if let Some(h) = host.get(&r.file) {
                hosts.entry(h).or_default().push(r);
            }
        }
        for (h, vs) in &hosts {
            let hr = rows.iter().find(|r| &&r.file == h).unwrap();
            vmd.push_str(&format!("\n**[{}](archive/{})** ({}, {}; {} blocks: {}):\n\n| variant | blocks | adds |\n|---|---|---|\n", hr.file.trim_end_matches(".txt"), hr.file, hr.kind, hr.axis, hr.blocks, hr.kinds.replace('+', ", ")));
            let hk = hr.kind_set();
            for v in vs {
                let adds: Vec<String> = v.kind_set().difference(&hk).cloned().collect();
                vmd.push_str(&format!("| [{}](archive/{}) | {} | {} |\n", v.file.trim_end_matches(".txt"), v.file, v.blocks, if adds.is_empty() { "-".to_string() } else { adds.join(", ") }));
            }
        }
        std::fs::write(sdir.join("variants.md"), vmd)?;
        for r in &rows {
            tsv.push_str(
                &[
                    study.to_string(),
                    r.kind.clone(),
                    r.axis.clone(),
                    r.file.trim_end_matches(".txt").to_string(),
                    format!("{study}/archive/{}", r.file),
                    r.metrics.clone(),
                    r.blocks.to_string(),
                    r.dims.clone(),
                    r.family.clone(),
                    r.robustness.clone(),
                    host.get(&r.file).cloned().unwrap_or_default(),
                    caps.get(&r.file).cloned().unwrap_or_default(),
                ]
                .map(|s| s.replace(['\t', '\n'], " "))
                .join("\t"),
            );
            tsv.push('\n');
        }
        index.push_str(&format!("| [{title}]({study}/CATALOG.md) | {} | {} | {} |\n", rows.len(), shown.len(), rows.len() - shown.len()));
        lines.push(format!("{study}: {} designs, {} shown, {} variants, {} values", rows.len(), shown.len(), rows.len() - shown.len(), by_axis.len()));
    }
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("CATALOG.md"), index)?;
    std::fs::write(root.join("catalog.tsv"), tsv)?;
    Ok(lines)
}
