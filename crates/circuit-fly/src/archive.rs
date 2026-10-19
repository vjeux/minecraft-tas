//! `mctas circuit fly archive` and `catalog`: the track's archive of real-game verified flying
//! machines (designs/circuits/flying-machine/archive/, the clock archive's conventions until the
//! shared circuit archive lands) and its generated catalog.
//!
//! archive: every design is judged in the real game (`verify`: 4 positions x 4 rotations); a
//! machine whose base build flies (or shuttles) is copied into the archive with a header (date,
//! source, the judge's line), a row in `verdicts.tsv` and its caption in `captions.tsv`. A machine
//! of an archived TYPE (`ident`: normal form by verified swaps, rotation, mirror) with the same
//! class and speed is refused (`SAME TYPE`).
//!
//! catalog: CATALOG.md (per class, per speed, the hosts of the dominance fold: fewest blocks
//! first) and variants.md (the variants, per host), from verdicts.tsv alone.

use crate::machine::Machine;
use crate::report::Verdict;
use crate::verify::{self, VerifyOptions};
use crate::{class_of, ident};
use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const ARCHIVE: &str = "designs/circuits/flying-machine/archive";
pub const CATALOG: &str = "designs/circuits/flying-machine/CATALOG.md";
pub const VARIANTS: &str = "designs/circuits/flying-machine/variants.md";

const HEADER: &str = "file\tname\tclass\tspeed\tperiod\tcells\tdirection\tblocks\tdims\trobustness\tfirst_move\tstart\ttype\tkinds\tsource\tdate";

/// One archived machine (a verdicts.tsv row).
#[derive(Clone, Debug)]
pub struct Row {
    pub file: String,
    pub name: String,
    pub class: String,
    pub speed: String,
    pub period: u32,
    pub cells: i32,
    pub direction: String,
    pub blocks: usize,
    pub dims: String,
    pub robustness: String,
    pub first_move: String,
    pub start: String,
    pub key: String,
    pub kinds: String,
    pub source: String,
    pub date: String,
}

impl Row {
    fn line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.file, self.name, self.class, self.speed, self.period, self.cells, self.direction, self.blocks, self.dims, self.robustness, self.first_move, self.start, self.key, self.kinds, self.source, self.date
        )
    }

    fn parse(l: &str) -> Option<Row> {
        let w: Vec<&str> = l.split('\t').collect();
        if w.len() != 16 || w[0] == "file" {
            return None;
        }
        Some(Row {
            file: w[0].into(),
            name: w[1].into(),
            class: w[2].into(),
            speed: w[3].into(),
            period: w[4].parse().ok()?,
            cells: w[5].parse().ok()?,
            direction: w[6].into(),
            blocks: w[7].parse().ok()?,
            dims: w[8].into(),
            robustness: w[9].into(),
            first_move: w[10].into(),
            start: w[11].into(),
            key: w[12].into(),
            kinds: w[13].into(),
            source: w[14].into(),
            date: w[15].into(),
        })
    }

    /// Speed as a number for sorting (gt per block).
    fn speed_value(&self) -> f64 {
        let s = self.speed.trim_end_matches(" gt/block");
        match s.split_once('/') {
            Some((a, b)) => a.parse::<f64>().unwrap_or(0.0) / b.parse::<f64>().unwrap_or(1.0),
            None => s.parse().unwrap_or(0.0),
        }
    }
}

pub fn read_rows(dir: &Path) -> Vec<Row> {
    std::fs::read_to_string(dir.join("verdicts.tsv")).map(|t| t.lines().filter_map(Row::parse).collect()).unwrap_or_default()
}

fn today() -> String {
    // days since the epoch -> y-m-d (civil from days, Howard Hinnant's algorithm)
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

fn key_hash(key: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut h);
    format!("{:06x}", h.finish() & 0xffffff)
}

/// The design's first comment block (its caption by default).
fn first_comment(text: &str) -> String {
    let mut out = Vec::new();
    for l in text.lines() {
        match l.strip_prefix('#') {
            Some(c) => out.push(c.trim().to_string()),
            None if out.is_empty() => continue,
            None => break,
        }
    }
    out.retain(|c| !c.starts_with("census:") && !c.starts_with("archived") && !c.starts_with("judge:"));
    out.join(" ")
}

/// Archive the designs that pass.
pub fn archive(designs: &[PathBuf], dir: &Path, caption: Option<&str>, o: &VerifyOptions) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut rows = read_rows(dir);
    let pool = verify::spawn_pool(o)?;
    for path in designs {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let m = Machine::parse(&text).with_context(|| format!("flying machine {}", path.display()))?;
        let rep = verify::verify_in_pool(&pool, &m, o)?;
        let base = &rep.base;
        if !matches!(base.verdict, Verdict::Flies | Verdict::Shuttles) {
            println!("REFUSED: {} {}", m.name(), rep.summary());
            continue;
        }
        let class = class_of(base);
        let speed = base.speed_text();
        let (norm, _) = ident::normalized(&m);
        let key = key_hash(&clock::canon::type_key(norm.design().blocks.iter()));
        if let Some(r) = rows.iter().find(|r| r.key == key && r.class == class && r.speed == speed) {
            println!("SAME TYPE: {} is the same type as {} ({class}, {speed})", m.name(), r.file);
            continue;
        }
        // a name taken by another structure gets the type hash
        let mut name = m.name().to_string();
        if rows.iter().any(|r| r.name == name) {
            name = format!("{name}-{key}");
        }
        let file = format!("{name}.txt");
        let mut mm = m.clone();
        mm.clock.design.name = name.clone();
        let st = base.steady.as_ref();
        let row = Row {
            file: file.clone(),
            name: name.clone(),
            class: class.clone(),
            speed: speed.clone(),
            period: st.map(|s| s.period).unwrap_or(0),
            cells: st.map(|s| s.cells()).unwrap_or(0),
            direction: base.direction().map(crate::machine::travel_name).unwrap_or("in place").to_string(),
            blocks: m.blocks(),
            dims: format!("{}x{}x{}", base.dims[0], base.dims[1], base.dims[2]),
            robustness: rep.robustness.clone(),
            first_move: base.first_change.map(|t| t.to_string()).unwrap_or("-".into()),
            start: base.start.clone(),
            key: key.clone(),
            kinds: norm.clock.kinds().into_iter().collect::<Vec<_>>().join("+"),
            source: path.display().to_string(),
            date: today(),
        };
        let header = format!("# archived {} from {}\n# judge: {}\n", row.date, row.source, rep.summary());
        let body = mm.to_text();
        std::fs::write(dir.join(&file), format!("{header}{}", body))?;
        let cap = caption.map(String::from).unwrap_or_else(|| first_comment(&text));
        let caps = dir.join("captions.tsv");
        let mut c = std::fs::read_to_string(&caps).unwrap_or_default();
        c.push_str(&format!("{name}\t{cap}\n"));
        std::fs::write(&caps, c)?;
        rows.push(row);
        let mut t = String::from(HEADER);
        t.push('\n');
        for r in &rows {
            t.push_str(&r.line());
            t.push('\n');
        }
        std::fs::write(dir.join("verdicts.tsv"), t)?;
        println!("ARCHIVED: {file} {}", rep.summary());
    }
    Ok(())
}

fn captions(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join("captions.tsv")).map(|t| t.lines().filter_map(|l| l.split_once('\t')).map(|(a, b)| (a.to_string(), b.to_string())).collect()).unwrap_or_default()
}

/// Write CATALOG.md and variants.md from the archive's verdicts.
pub fn catalog(dir: &Path, out: &Path, variants_out: &Path) -> Result<()> {
    let rows = read_rows(dir);
    let caps = captions(dir);
    let class_order = ["horizontal", "up", "down", "two-way", "payload", "trail"];
    let rank = |c: &str| class_order.iter().position(|x| *x == c).unwrap_or(class_order.len());
    let kinds_of = |r: &Row| r.kinds.split('+').map(String::from).collect::<BTreeSet<String>>();
    // dominance fold: a variant has more blocks than a host of the same class and speed whose
    // kinds are a subset of its own
    let host_of = |r: &Row| rows.iter().filter(|h| h.class == r.class && h.speed == r.speed && h.blocks < r.blocks && kinds_of(h).is_subset(&kinds_of(r))).min_by_key(|h| (h.blocks, h.name.clone())).map(|h| h.name.clone());
    let rel = |r: &Row| format!("archive/{}", r.file);
    let mut classes: Vec<String> = rows.iter().map(|r| r.class.clone()).collect::<BTreeSet<_>>().into_iter().collect();
    classes.sort_by_key(|c| rank(c));
    let mut md = String::new();
    md.push_str("# Flying machines (generated by `mctas circuit fly catalog`; never edit)\n\n");
    md.push_str("Self-propelled slime/honey + observer + piston structures, every one verified in the real game\n");
    md.push_str("(`mctas circuit fly verify`: 4 positions x 4 rotations, it must move steadily and stay intact).\n");
    md.push_str("Speed = gt per block moved (the steady cycle P over the cells it moves per cycle). Hosts of the\n");
    md.push_str("dominance fold only; machines with more blocks and a superset of a host's block kinds are on\n");
    md.push_str("[variants.md](variants.md). Track notes: [tracks/fly/NOTES.md](tracks/fly/NOTES.md).\n\n");
    let total_hosts = rows.iter().filter(|r| host_of(r).is_none()).count();
    md.push_str(&format!("{} machines archived, {} hosts.\n\n", rows.len(), total_hosts));
    md.push_str("| class | speeds (gt/block): hosts |\n|---|---|\n");
    for c in &classes {
        let mut sp: BTreeMap<String, (f64, usize)> = BTreeMap::new();
        for r in rows.iter().filter(|r| &r.class == c && host_of(r).is_none()) {
            let e = sp.entry(r.speed.clone()).or_insert((r.speed_value(), 0));
            e.1 += 1;
        }
        let mut v: Vec<(String, (f64, usize))> = sp.into_iter().collect();
        v.sort_by(|a, b| a.1.0.partial_cmp(&b.1.0).unwrap());
        md.push_str(&format!("| {c} | {} |\n", v.iter().map(|(s, (_, n))| format!("{}: {n}", s.trim_end_matches(" gt/block"))).collect::<Vec<_>>().join(", ")));
    }
    md.push('\n');
    for c in &classes {
        md.push_str(&format!("## {c}\n\n"));
        let mut speeds: Vec<(f64, String)> = rows.iter().filter(|r| &r.class == c).map(|r| (r.speed_value(), r.speed.clone())).collect::<Vec<_>>();
        speeds.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        speeds.dedup_by(|a, b| a.1 == b.1);
        for (_, s) in speeds {
            let mut hosts: Vec<&Row> = rows.iter().filter(|r| &r.class == c && r.speed == s && host_of(r).is_none()).collect();
            hosts.sort_by_key(|r| (r.blocks, r.robustness != "robust", r.name.clone()));
            md.push_str(&format!("### {s} ({} types)\n\n| design | blocks | box | direction | first move | real game | kinds | how |\n|---|---|---|---|---|---|---|---|\n", hosts.len()));
            for r in hosts {
                md.push_str(&format!(
                    "| [{}]({}) | {} | {} | {} | t{} | {} | {} | {} |\n",
                    r.name,
                    rel(r),
                    r.blocks,
                    r.dims,
                    r.direction,
                    r.first_move,
                    r.robustness,
                    r.kinds.replace("_block", "").replace("sticky_piston", "sticky"),
                    caps.get(&r.name).cloned().unwrap_or_default()
                ));
            }
            md.push('\n');
        }
    }
    std::fs::write(out, md)?;
    let mut vm = String::from("# Flying machines: variants of the dominance fold (generated; never edit)\n\nA variant has more blocks than its host (same class and speed) and uses every block kind the host uses.\n\n");
    let mut by_host: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        if let Some(h) = host_of(r) {
            by_host.entry(h).or_default().push(r);
        }
    }
    for (h, vs) in by_host {
        vm.push_str(&format!("## variants of {h}\n\n| variant | blocks | class | speed | kinds |\n|---|---|---|---|---|\n"));
        for r in vs {
            vm.push_str(&format!("| [{}]({}) | {} | {} | {} | {} |\n", r.name, rel(r), r.blocks, r.class, r.speed, r.kinds));
        }
        vm.push('\n');
    }
    std::fs::write(variants_out, vm)?;
    println!("catalog: {} machines, {} hosts -> {}", rows.len(), total_hosts, out.display());
    Ok(())
}
