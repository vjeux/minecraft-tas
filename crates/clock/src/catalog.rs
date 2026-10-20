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

const HEADER: &str = "hash\tfile\tperiod\twarmup\ton\tedges\tpulses\tfamily\tblocks\tdims\tvolume\trobustness\tstart\ttype\tchiral\tsummary\tgaps";

/// The verdict table's header line.
pub fn header() -> &'static str {
    HEADER
}

/// A `gaps` cell of the verdict table (`[52, 71, 53]`, the ticks between the output's rising edges
/// over one period; empty = not measured yet) parsed; None when empty or unreadable.
pub fn parse_gaps(s: &str) -> Option<Vec<u32>> {
    let t = s.trim().strip_prefix('[')?.strip_suffix(']')?;
    if t.trim().is_empty() {
        return Some(Vec::new());
    }
    t.split(',').map(|x| x.trim().parse().ok()).collect()
}

/// An output whose rising edges are unevenly spaced (coordinator, 2026-10-20: a clock on the site
/// must deliver evenly spaced pulses at its declared output; such rows are kept in the repository
/// but left out of the catalog and the site).
pub fn uneven(gaps: &[u32]) -> bool {
    gaps.len() >= 2 && gaps.iter().any(|&g| g != gaps[0])
}

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
    /// The ticks between the output's rising edges over one period (`[6, 6]`; empty: not measured).
    pub gaps: String,
}

impl Row {
    /// The output's rising edges are unevenly spaced (see [`uneven`]).
    pub fn uneven(&self) -> bool {
        parse_gaps(&self.gaps).is_some_and(|g| uneven(&g))
    }

    /// The row as a line of the verdict table.
    pub fn tsv(&self) -> String {
        self.to_tsv()
    }

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
            self.gaps.clone(),
        ]
        .join("\t")
    }

    pub fn from_tsv(line: &str) -> Option<Row> {
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
            gaps: f.get(16).copied().unwrap_or("").into(),
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
/// A design's caption: its first comment block.
pub fn default_caption(text: &str) -> String {
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

/// One extendable family for [`card_groups`]: its period formula, its base design's block kinds,
/// its size key (blocks at its smallest n, volume, archive order) and its `extend group`.
pub struct CardItem {
    pub formula: String,
    pub kinds: BTreeSet<String>,
    pub size: (i64, i64, usize),
    pub group: Option<String>,
}

/// One extendable card: its families (indices; the pictured one, the simplest, first), the block
/// kinds they all share, and the `extend group` it renders (None: a lone family, or families
/// grouped by the one-kind rule).
pub struct Card {
    pub members: Vec<usize>,
    pub core: BTreeSet<String>,
    pub group: Option<String>,
}

/// Group extendable families into cards. Families with an `extend group:` header form one card
/// per group (vjeux: "can we not have 300 variations of this one?"). The others: families with the
/// same period formula whose base designs differ by one block kind (a relay: note block, crafter,
/// rail, door...) share one card (coordinator, 2026-10-19): each joins the (formula, its kinds minus
/// one kind) group with the most families (at least 2; ties: the first core in order); the rest
/// are cards of their own. Members are ordered simplest first (fewest blocks, volume, archived
/// first).
pub fn card_groups(items: &[CardItem]) -> Vec<Card> {
    let core_of = |members: &[usize]| -> BTreeSet<String> {
        let mut it = members.iter().map(|&i| items[i].kinds.clone());
        let first = it.next().unwrap_or_default();
        it.fold(first, |a, b| a.intersection(&b).cloned().collect())
    };
    let mut cards: Vec<Card> = Vec::new();
    let mut explicit: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        if let Some(g) = &it.group {
            explicit.entry(g.clone()).or_default().push(i);
        }
    }
    for (g, mut members) in explicit {
        members.sort_by_key(|&i| items[i].size);
        let core = core_of(&members);
        cards.push(Card { members, core, group: Some(g) });
    }
    let free: Vec<usize> = (0..items.len()).filter(|&i| items[i].group.is_none()).collect();
    let mut by_core: BTreeMap<(String, Vec<String>), Vec<usize>> = BTreeMap::new();
    for &i in &free {
        let it = &items[i];
        for x in &it.kinds {
            let core: Vec<String> = it.kinds.iter().filter(|k| *k != x).cloned().collect();
            if !core.is_empty() {
                by_core.entry((it.formula.clone(), core)).or_default().push(i);
            }
        }
    }
    let mut chosen: BTreeMap<(String, Vec<String>), Vec<usize>> = BTreeMap::new();
    let mut alone: Vec<usize> = Vec::new();
    for &i in &free {
        let it = &items[i];
        let best = it
            .kinds
            .iter()
            .filter_map(|x| {
                let core: Vec<String> = it.kinds.iter().filter(|k| *k != x).cloned().collect();
                let key = (it.formula.clone(), core);
                by_core.get(&key).filter(|v| v.len() >= 2).map(|v| (v.len(), key))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        match best {
            Some((_, key)) => chosen.entry(key).or_default().push(i),
            None => alone.push(i),
        }
    }
    for ((_, core), mut members) in chosen {
        if members.len() == 1 {
            alone.extend(members);
            continue;
        }
        members.sort_by_key(|&i| items[i].size);
        cards.push(Card { members, core: core.into_iter().collect(), group: None });
    }
    for i in alone {
        cards.push(Card { members: vec![i], core: items[i].kinds.clone(), group: None });
    }
    cards
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
    // a builder needs evenly spaced pulses at the output (coordinator, 2026-10-20)
    if let Some(w) = rep.base().and_then(|b| b.output.as_ref()).map(|o| &o.wave)
        && let Some((lo, hi)) = w.irregular()
    {
        let tap = rep.base().and_then(|b| b.analysis.cells.iter().filter(|c| c.tappable()).filter_map(|c| c.output_period().map(|p| (p, c.pos))).max());
        let hint = match tap {
            Some((p, pos)) => format!("; the cell {} {} {} pulses every {p} gt: declare it as `port output`", pos[0], pos[1], pos[2]),
            None => "; no cell of it pulses evenly".into(),
        };
        bail!("not archived: uneven output ({} rising edges per {} gt, gaps {:?}, {lo}..{hi} gt){hint}", w.edges, rep.base().map(|b| b.period).unwrap_or(0), w.gaps);
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
    let gaps = base.output.as_ref().map(|o| format!("{:?}", o.wave.gaps)).unwrap_or_default();
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
        gaps,
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
    let rows: Vec<Row> = all_rows.iter().filter(|r| !composite.contains(&r.file) && !r.uneven()).cloned().collect();
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
    // the extendable cards: families sharing a period formula whose base designs differ by one
    // block kind share a card, the smallest family pictured
    let order: BTreeMap<&str, usize> = rows.iter().enumerate().map(|(i, r)| (r.file.as_str(), i)).collect();
    let fam_items: Vec<CardItem> = fams
        .iter()
        .map(|(r, f)| CardItem {
            formula: f.period.text(),
            kinds: kinds_of.get(&r.file).cloned().unwrap_or_default(),
            size: (f.blocks.eval(f.n_lo), r.volume, order.get(r.file.as_str()).copied().unwrap_or(usize::MAX)),
            group: f.group.clone(),
        })
        .collect();
    let mut cards = card_groups(&fam_items);
    cards.sort_by(|a, b| {
        let (fa, fb) = (&fams[a.members[0]].1, &fams[b.members[0]].1);
        (fa.period.eval(fa.base), &fa.id).cmp(&(fb.period.eval(fb.base), &fb.id))
    });
    // a card's name: the family's, the group's `extend group title:` (else its id), or "<shared
    // kinds> + one of N blocks"
    let card_title = |card: &Card| -> String {
        if card.members.len() == 1 {
            return fams[card.members[0]].1.id.clone();
        }
        if let Some(t) = card.members.iter().find_map(|&i| fams[i].1.group_title.clone()) {
            return t;
        }
        if let Some(g) = &card.group {
            return g.clone();
        }
        format!("{} (one block swapped: {} versions)", fams[card.members[0]].1.id, card.members.len())
    };
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
        "# Clock catalog\n\n{} clock types ({} periods, {} families) pictured in the real game at the most active moment of their period, plus {} variants (larger clocks of a period that only add block kinds to a smaller one: [listed apart](gallery/variants.md)); **orange glass = where the output signal is** (placed only for the picture); click a picture for its design. Generated by `mctas clock catalog` ([docs/clocks.md](../../docs/clocks.md); the period x family matrix: [coverage.md](coverage.md)).\n",
        by_type.len() - folded.len(),
        periods.len(),
        families.len(),
        folded.len()
    ));
    if !fams.is_empty() {
        // one card per family (vjeux: "show the 0 version and the 1 version so we know what to
        // expect; make the gt formula bigger"): the period formula as a heading, the two smallest
        // members side by side (the base design's picture, or `img/<base stem>.n<k>.jpg` taken by
        // `clock shot` from the family's generator), then the seconds, blocks, rule and n range.
        // Families with the same period formula whose base designs differ by one block kind (a
        // relay: note block, crafter, rail, door...) share ONE card ([`card_groups`]), pictured with
        // the smallest of them, the others listed with links.
        md.push_str("\n## Extendable clocks\n\nA family grows by a repeated step. Each card shows its two smallest members in the real game, its period for any n, its size, how to extend it and the n range judged in the game (`mctas clock extend-check`); the members are not repeated in the period galleries. Families that differ by one block kind only share a card.\n");
        for card in &cards {
            let (r, f) = &fams[card.members[0]];
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
            let judged = |f: &crate::extend::Family| -> String {
                let mut v: Vec<i64> = instances.iter().filter(|x| x.family == f.id).map(|x| x.n).collect();
                v.sort();
                v.dedup();
                if v.is_empty() { String::new() } else { format!("; judged in the game: n = {}", v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")) }
            };
            let ns: Vec<i64> = [f.n_lo, f.n_lo + 1].into_iter().filter(|n| *n <= f.n_hi).collect();
            let gt = |f: &crate::extend::Family, n: i64| crate::gt_time(f.period.eval(n).max(0) as u32);
            let pictures = ns.iter().map(|n| member(*n)).collect::<String>();
            if card.members.len() == 1 {
                md.push_str(&format!(
                    "\n### P = {} gt\n\n**{}** ([base design](archive/{})) · P = {} · {} blocks · {}\n\n<table><tr>{pictures}</tr></table>\n\nExtend: {}.<br>\nn = {}..{}: P = {} .. {}{}.\n",
                    f.period.text(),
                    f.id,
                    r.file,
                    f.period.seconds(),
                    f.blocks.text(),
                    r.family,
                    f.rule.trim_end_matches('.'),
                    f.n_lo,
                    f.n_hi,
                    gt(f, f.n_lo),
                    gt(f, f.n_hi),
                    judged(f)
                ));
                continue;
            }
            // a card of several families: the general formula (`extend group period:`, else the one
            // formula they share, else `a + b·n` over their a, else each formula) as the heading, the
            // simplest family's two smallest members pictured, then every family in a collapsed table
            let fs: Vec<&crate::extend::Family> = card.members.iter().map(|&i| &fams[i].1).collect();
            let mut texts: Vec<String> = Vec::new();
            for g in &fs {
                if !texts.contains(&g.period.text()) {
                    texts.push(g.period.text());
                }
            }
            let lin: Vec<(i64, i64)> = fs.iter().filter_map(|g| match g.period { crate::extend::Formula::Linear { a, b } => Some((a, b)), _ => None }).collect();
            let heading = match fs.iter().find_map(|g| g.group_period.clone()) {
                Some(gp) => gp,
                None if texts.len() == 1 => texts[0].clone(),
                None if lin.len() == fs.len() && lin.iter().all(|x| x.1 == lin[0].1) => {
                    let mut a: Vec<i64> = lin.iter().map(|x| x.0).collect();
                    a.sort();
                    a.dedup();
                    format!("a + {}n gt, a in {{{}}}", lin[0].1, a.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "))
                }
                None => texts.join(" · "),
            };
            let heading = if heading.ends_with(" gt") || heading.contains(" gt,") { heading } else { format!("{heading} gt") };
            let note = fs.iter().find_map(|g| g.group_note.clone()).map(|n| format!(" · {n}")).unwrap_or_default();
            let secs = if texts.len() == 1 { format!(" · P = {}", f.period.seconds()) } else { String::new() };
            // families that differ by one swapped block each (a relay): the blocks that work, in a
            // line (vjeux: "write down the list of items that work in the description")
            let swaps: Vec<Vec<String>> = card.members.iter().map(|&i| kinds_of.get(&fams[i].0.file).map(|k| k.difference(&card.core).cloned().collect()).unwrap_or_default()).collect();
            let works = if swaps.iter().all(|s| s.len() == 1) {
                let names: Vec<String> = swaps.iter().map(|s| s[0].replace('_', " ")).collect();
                format!("Works with any one of these {} blocks in place of the {}: {}.\n\n", names.len(), names[0], names.join(", "))
            } else {
                String::new()
            };
            // the periods the cards reach together (each over its judged n range)
            let together = {
                let mut ps: Vec<i64> = fs.iter().flat_map(|g| (g.n_lo..=g.n_hi.min(g.n_lo + 400)).map(|n| g.period.eval(n))).filter(|p| *p > 0).collect();
                ps.sort();
                ps.dedup();
                let (mut best, mut start) = ((0usize, 0usize), 0usize);
                for i in 1..=ps.len() {
                    if i == ps.len() || ps[i] != ps[i - 1] + 1 {
                        if i - start > best.1 - best.0 {
                            best = (start, i);
                        }
                        start = i;
                    }
                }
                if best.1 - best.0 >= 5 {
                    let (lo, hi) = (ps[best.0], ps[best.1 - 1]);
                    let below: Vec<i64> = ps.iter().copied().filter(|p| *p < lo).collect();
                    let mut parts: Vec<String> = Vec::new();
                    let mut i = 0;
                    while i < below.len() {
                        let mut j = i;
                        while j + 1 < below.len() && below[j + 1] == below[j] + 1 {
                            j += 1;
                        }
                        parts.push(if j > i { format!("{}-{}", below[i], below[j]) } else { below[i].to_string() });
                        i = j + 1;
                    }
                    let also = if parts.is_empty() { String::new() } else if parts.len() <= 12 { format!(", and {} gt", parts.join(", ")) } else { format!(", and {} periods below", below.len()) };
                    format!("Together they run at every period from {} to {}{also}.\n\n", crate::gt_time(lo as u32), crate::gt_time(hi as u32))
                } else {
                    String::new()
                }
            };
            let extra = |file: &str| -> String {
                let x: Vec<String> = kinds_of.get(file).map(|k| k.difference(&card.core).map(|s| format!("+{s}")).collect()).unwrap_or_default();
                if x.is_empty() { String::new() } else { format!(" ({})", x.join(", ")) }
            };
            let judged_list = |g: &crate::extend::Family| -> String {
                let mut v: Vec<i64> = instances.iter().filter(|x| x.family == g.id).map(|x| x.n).collect();
                v.sort();
                v.dedup();
                if v.is_empty() { "-".to_string() } else { v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ") }
            };
            let mut table: Vec<usize> = card.members.clone();
            table.sort_by(|&a, &b| {
                let (fa, fb) = (&fams[a].1, &fams[b].1);
                (fa.period.eval(fa.n_lo), &fa.id).cmp(&(fb.period.eval(fb.n_lo), &fb.id))
            });
            let rows_md: String = table
                .iter()
                .map(|&i| {
                    let (mr, mf) = &fams[i];
                    format!(
                        "| [{}](archive/{}){} | {} gt | {} | {}..{} ({} .. {}) | {} |\n",
                        mf.id,
                        mr.file,
                        extra(&mr.file),
                        mf.period.text(),
                        mf.blocks.text(),
                        mf.n_lo,
                        mf.n_hi,
                        gt(mf, mf.n_lo),
                        gt(mf, mf.n_hi),
                        judged_list(mf)
                    )
                })
                .collect();
            md.push_str(&format!(
                "\n### P = {heading}\n\n**{}** · {} cards{note}{secs}\n\n<table><tr>{pictures}</tr></table>\n\n{works}{together}Pictured: [{}](archive/{}){}: P = {} gt, {} blocks, n = {}..{}{}. Extend: {}.\n\n<details><summary>All {} cards of {} (formula, size, n range, design)</summary>\n\n| card | P | blocks | n (P) | judged in the game: n |\n|---|---|---|---|---|\n{rows_md}\n</details>\n",
                card_title(card),
                card.members.len(),
                f.id,
                r.file,
                extra(&r.file),
                f.period.text(),
                f.blocks.text(),
                f.n_lo,
                f.n_hi,
                judged(f),
                f.rule.trim_end_matches('.'),
                card.members.len(),
                card_title(card)
            ));
        }
    }
    // one section per period (on the gallery pages: paths from designs/clocks/gallery/)
    let mut sections: Vec<(u32, String, usize, usize)> = Vec::new();
    let mut variants_md = String::new();
    for p in &periods {
        let all: Vec<&Row> = by_type.iter().filter(|((pp, _), _)| pp == p).map(|(_, r)| r).filter(|r| is_instance(r).is_none()).collect();
        let mut rs: Vec<&Row> = all.iter().copied().filter(|r| !folded.contains_key(&r.file)).collect();
        rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
        // the variants of each host go to ONE separate page, gallery/variants.md (vjeux: the inline
        // lines were "completely unreadable"): per period, per host a small table (variant, blocks,
        // the kinds it adds)
        let mut vsec = String::new();
        for h in &rs {
            let mut vs: Vec<&Row> = all.iter().copied().filter(|r| folded.get(&r.file) == Some(&h.file)).collect();
            if vs.is_empty() {
                continue;
            }
            vs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
            let hk = kinds_of.get(&h.file).cloned().unwrap_or_default();
            vsec.push_str(&format!(
                "\n**[{}](../archive/{})** ({} blocks: {}), {} variant{}:\n\n| variant | blocks | adds |\n|---|---|---|\n",
                h.file.trim_end_matches(".txt"),
                h.file,
                h.blocks,
                hk.iter().cloned().collect::<Vec<_>>().join(", "),
                vs.len(),
                if vs.len() == 1 { "" } else { "s" }
            ));
            for v in &vs {
                let extra: Vec<String> = kinds_of.get(&v.file).map(|k| k.difference(&hk).cloned().collect()).unwrap_or_default();
                vsec.push_str(&format!("| [{}](../archive/{}) | {} | {} |\n", v.file.trim_end_matches(".txt"), v.file, v.blocks, if extra.is_empty() { "-".to_string() } else { extra.join(", ") }));
            }
        }
        if !vsec.is_empty() {
            variants_md.push_str(&format!("\n## {}\n{vsec}", crate::gt_time(*p)));
        }
        let reach: Vec<String> = cards
            .iter()
            .filter_map(|card| {
                card.members.iter().find_map(|&i| {
                    let f = &fams[i].1;
                    (f.n_lo..=f.n_hi).find(|n| f.period.eval(*n) == *p as i64).map(|n| (n, f))
                })
                .map(|(n, f)| if card.members.len() == 1 { format!("{} (n = {n})", f.id) } else { format!("{} ({}, n = {n})", card_title(card), f.id) })
            })
            .collect();
        if rs.is_empty() && reach.is_empty() {
            continue;
        }
        let also = if reach.is_empty() { String::new() } else { format!("\nAlso reachable by: {}.\n", reach.join(", ")) };
        let text = format!("\n## {}\n{also}\n{}", crate::gt_time(*p), if rs.is_empty() { String::new() } else { grid(&rs, "../") });
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
    md.push_str("\n## Galleries by period\n\n| periods | clock types | page |\n|---|---|---|\n");
    for (k, pg) in pages.iter().enumerate() {
        let (lo, hi) = range(pg);
        let n: usize = pg.iter().map(|i| sections[*i].2).sum();
        md.push_str(&format!("| {} | {n} | [{}](gallery/{}) |\n", span(lo, hi), names[k].trim_end_matches(".md"), names[k]));
    }
    if !composite.is_empty() {
        md.push_str(&format!("| composite (beat) clocks | {} | [composite](gallery/composite.md) |\n", composite.len()));
    }
    if !folded.is_empty() {
        md.push_str(&format!("\nVariants: larger clocks of the same period that only add block kinds to a smaller one ({}): [list](gallery/variants.md).\n", folded.len()));
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
        let mut page = format!(
            "# Clocks: {} ({n} clock types)\n\n{nav}\n\nOrange glass = where the output signal is (placed only for the picture); click a picture for its design. Larger clocks of a period that only add block kinds to a smaller one are listed on [variants](variants.md). Generated by `mctas clock catalog`.\n",
            span(lo, hi)
        );
        for i in pg {
            page.push_str(&sections[*i].1);
        }
        page.push_str(&format!("\n{nav}\n"));
        std::fs::write(gallery.join(&names[k]), page)?;
    }
    if !variants_md.is_empty() {
        let page = format!(
            "# Clock variants\n\n[catalog index](../CATALOG.md)\n\nA variant is a clock with the same period as a smaller clock (its host: fewer blocks) whose block kinds it all uses, plus others; the galleries show only the hosts. {} variants of {} hosts, by period. Generated by `mctas clock catalog`.\n{variants_md}\n[catalog index](../CATALOG.md)\n",
            folded.len(),
            folded.values().collect::<BTreeSet<_>>().len()
        );
        std::fs::write(gallery.join("variants.md"), page)?;
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

    /// The coordinator's example: the ten observer rings with one relay block each (P = 10 + 4n)
    /// share one card, the smallest pictured; the plain observer ring (P = 8 + 4n) and a family
    /// with another formula keep their own; an `extend group` joins families whatever their
    /// formulas (vjeux: "can we not have 300 variations of this one?").
    #[test]
    fn extendable_card_groups() {
        let k = |v: &[&str]| -> BTreeSet<String> { v.iter().map(|s| s.to_string()).collect() };
        let mut items = Vec::new();
        for (i, relay) in ["note_block", "crafter", "oak_door", "powered_rail"].iter().enumerate() {
            items.push(CardItem { formula: "10 + 4n".into(), kinds: k(&["observer", relay]), size: (if *relay == "oak_door" { 7 } else { 6 }, 6, 10 - i), group: None });
        }
        items.push(CardItem { formula: "8 + 4n".into(), kinds: k(&["observer"]), size: (4, 4, 0), group: None });
        items.push(CardItem { formula: "20 + 8n".into(), kinds: k(&["observer", "copper_bulb"]), size: (5, 5, 1), group: None });
        let drop = |f: &str, extra: &[&str], size: i64| {
            let mut kinds = k(&["comparator", "dropper", "hopper", "stone"]);
            kinds.extend(extra.iter().map(|s| s.to_string()));
            CardItem { formula: f.into(), kinds, size: (size, size, 20), group: Some("dropring".into()) }
        };
        items.push(drop("27 + 14n", &[], 10));
        items.push(drop("35 + 14n", &["repeater"], 14));
        items.push(drop("51 + 14n", &["repeater"], 18));
        let cards = card_groups(&items);
        assert_eq!(cards.len(), 4);
        let relay = cards.iter().find(|c| c.members.len() == 4).unwrap();
        assert_eq!(relay.core, k(&["observer"]));
        assert_eq!(relay.group, None);
        // the smallest (6 blocks), then volume, then the earliest archived: powered_rail (order 7)
        assert_eq!(relay.members[0], 3);
        let group = cards.iter().find(|c| c.group.as_deref() == Some("dropring")).unwrap();
        assert_eq!(group.members, vec![6, 7, 8]);
        assert_eq!(group.core, k(&["comparator", "dropper", "hopper", "stone"]));
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
