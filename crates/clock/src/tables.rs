//! The archive tables (TSV files written by `clock archive`, `circuit archive` and friends) and
//! their duplicate rows (coordinator 2026-10-21, after a merge left every clock's verdict row twice
//! and hid the new `usable` column).
//!
//! The tables merge with git's union driver (.gitattributes): two branches that each append rows
//! both keep theirs. A row one branch UPDATES (a re-judge, a new column filled) is a different line
//! on each side, so a union merge keeps BOTH versions, and a reader that takes the last line may
//! take the stale one. This module names each table's key ([`tables`]), lists the keys with several
//! rows ([`duplicates`]), and keeps one row per key deterministically ([`dedupe`], the readers in
//! `catalog::read_rows` and `circuit::archive::read_rows` use it), in this order:
//!  1. the row with the most columns (a newer schema: a column added on one side),
//!  2. the table's own freshness signal: the newest `archived` stamp (circuit verdicts), or the row
//!     whose content hash matches the design file as it is now (clock verdicts),
//!  3. the row that is NOT in the merge base (the side that changed it; `mctas clock tables --fix`
//!     only, from git),
//!  4. the last row (reported as ambiguous).
//!
//! The kept row takes the place of the key's first row. `mctas clock tables` lists the duplicate
//! keys, `--fix` rewrites the tables (the command to run after a merge; `clock catalog` and
//! `circuit catalog` run it too), and the quick selftest fails while any table has a duplicate key.

use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

/// One archive table: its file, its key columns and its freshness signal.
#[derive(Clone, Debug)]
pub struct Table {
    pub path: PathBuf,
    /// The key columns (0-based).
    pub key: Vec<usize>,
    /// A column holding a sortable stamp: the newest wins.
    pub stamp: Option<usize>,
    /// Clock verdicts: column 0 is the design's content hash; the row matching the design file
    /// (next to the table) as it is now wins.
    pub clock_hash: bool,
}

/// Every archive table under a `designs` folder: the clock archive's and each circuit study's.
pub fn tables(designs: &Path) -> Vec<Table> {
    let mut v = Vec::new();
    let ca = designs.join("clocks").join("archive");
    for (f, key, hash) in [
        ("verdicts-v1.tsv", vec![1], true),
        ("captions.tsv", vec![0], false),
        ("extend.tsv", vec![0, 1], false),
        ("aliases.tsv", vec![1], false),
        ("vibration.tsv", vec![0], false),
        ("type-keys.tsv", vec![0, 1], false),
    ] {
        let p = ca.join(f);
        if p.exists() {
            v.push(Table { path: p, key, stamp: None, clock_hash: hash });
        }
    }
    if let Ok(rd) = std::fs::read_dir(designs.join("circuits")) {
        let mut studies: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path().join("archive"))).filter(|p| p.is_dir()).collect();
        studies.sort();
        for a in studies {
            for (f, key, stamp) in [("verdicts.tsv", vec![1], Some(0)), ("captions.tsv", vec![0], None), ("extend.tsv", vec![0, 1], None)] {
                let p = a.join(f);
                if p.exists() {
                    v.push(Table { path: p, key, stamp, clock_hash: false });
                }
            }
        }
    }
    v
}

/// A clock verdict table at `dir` (the readers' own table).
pub fn clock_verdicts(dir: &Path) -> Table {
    Table { path: dir.join(crate::catalog::VERDICTS), key: vec![1], stamp: None, clock_hash: true }
}

fn key_of(t: &Table, line: &str) -> String {
    let f: Vec<&str> = line.split('\t').collect();
    t.key.iter().map(|&i| f.get(i).copied().unwrap_or("")).collect::<Vec<_>>().join("\t")
}

/// The keys with several distinct rows (or a row repeated), with their row count, in file order.
pub fn duplicates(t: &Table) -> Result<Vec<(String, usize)>> {
    let text = std::fs::read_to_string(&t.path).with_context(|| format!("reading {}", t.path.display()))?;
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    let mut count: BTreeMap<String, usize> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for l in lines.filter(|l| !l.trim().is_empty() && *l != header) {
        let k = key_of(t, l);
        let n = count.entry(k.clone()).or_insert(0);
        if *n == 0 {
            order.push(k);
        }
        *n += 1;
    }
    Ok(order.into_iter().filter_map(|k| (count[&k] > 1).then(|| (k.clone(), count[&k]))).collect())
}

/// Why a row was kept (for the notes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Identical,
    Columns,
    Stamp,
    Hash,
    MergeBase,
    Last,
}

impl Rule {
    pub fn word(self) -> &'static str {
        match self {
            Rule::Identical => "identical rows",
            Rule::Columns => "the most columns",
            Rule::Stamp => "the newest stamp",
            Rule::Hash => "the design file's current hash",
            Rule::MergeBase => "the row the merge changed (not in the merge base)",
            Rule::Last => "AMBIGUOUS: the last row",
        }
    }
}

/// Pick one of several rows for one key (see the module doc).
fn pick<'a>(t: &Table, rows: &[&'a str], base: Option<&HashSet<&str>>) -> (&'a str, Rule) {
    let mut cand: Vec<&str> = Vec::new();
    for r in rows {
        if !cand.contains(r) {
            cand.push(r);
        }
    }
    if cand.len() == 1 {
        return (cand[0], Rule::Identical);
    }
    let narrow = |c: &mut Vec<&'a str>, keep: &dyn Fn(&&'a str) -> bool| -> bool {
        let k: Vec<&str> = c.iter().copied().filter(|r| keep(r)).collect();
        if !k.is_empty() && k.len() < c.len() {
            *c = k;
        }
        c.len() == 1
    };
    let most = cand.iter().map(|r| r.split('\t').count()).max().unwrap_or(0);
    if narrow(&mut cand, &|r| r.split('\t').count() == most) {
        return (cand[0], Rule::Columns);
    }
    if let Some(s) = t.stamp {
        let col = |r: &str| r.split('\t').nth(s).unwrap_or("").to_string();
        let newest = cand.iter().map(|r| col(r)).max().unwrap_or_default();
        if narrow(&mut cand, &|r| col(r) == newest) {
            return (cand[0], Rule::Stamp);
        }
    }
    if t.clock_hash {
        let file = cand[0].split('\t').nth(1).unwrap_or("");
        let dir = t.path.parent().unwrap_or(Path::new("."));
        if let Ok(c) = crate::Clock::load(&dir.join(file)) {
            let h = crate::catalog::content_hash(&c);
            if narrow(&mut cand, &|r| r.split('\t').next() == Some(h.as_str())) {
                return (cand[0], Rule::Hash);
            }
        }
    }
    if let Some(b) = base
        && narrow(&mut cand, &|r| !b.contains(*r))
    {
        return (cand[0], Rule::MergeBase);
    }
    (cand[cand.len() - 1], Rule::Last)
}

/// One row per key: the table's text rewritten (the header, then each key's kept row at its first
/// row's place; stray header lines and blank lines dropped), the rows removed, and a note per key
/// that had several (`key: n rows -> rule`). `base`: the table's text at the merge base.
pub fn dedupe(t: &Table, text: &str, base: Option<&str>) -> (String, usize, Vec<String>) {
    let mut lines = text.lines();
    let header = lines.next().unwrap_or_default();
    let rows: Vec<&str> = lines.filter(|l| !l.trim().is_empty() && *l != header).collect();
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    for r in &rows {
        let k = key_of(t, r);
        let g = groups.entry(k.clone()).or_default();
        if g.is_empty() {
            order.push(k);
        }
        g.push(r);
    }
    let base_set: Option<HashSet<&str>> = base.map(|b| b.lines().collect());
    let mut out = format!("{header}\n");
    let mut notes = Vec::new();
    for k in &order {
        let g = &groups[k];
        let (row, rule) = if g.len() == 1 { (g[0], Rule::Identical) } else { pick(t, g, base_set.as_ref()) };
        if g.len() > 1 {
            notes.push(format!("{}: {} rows -> {}", k.replace('\t', " "), g.len(), rule.word()));
        }
        out.push_str(row);
        out.push('\n');
    }
    let removed = rows.len() - order.len();
    (out, removed, notes)
}

/// The table's text at the merge base of the merge in progress (`MERGE_HEAD`) or of a merge commit
/// at HEAD, from git; None outside a merge.
pub fn merge_base_text(path: &Path) -> Option<String> {
    let git = |args: &[&str]| -> Option<String> {
        let o = std::process::Command::new("git").args(args).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim_end_matches('\n').to_string())
    };
    let base = if git(&["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_some() {
        git(&["merge-base", "HEAD", "MERGE_HEAD"])?
    } else if git(&["rev-parse", "-q", "--verify", "HEAD^2"]).is_some() {
        git(&["merge-base", "HEAD^1", "HEAD^2"])?
    } else {
        return None;
    };
    let top = git(&["rev-parse", "--show-toplevel"])?;
    let abs = std::fs::canonicalize(path).ok()?;
    let rel = abs.strip_prefix(&top).ok()?.to_string_lossy().to_string();
    let o = std::process::Command::new("git").args(["show", &format!("{base}:{rel}")]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).to_string())
}

/// `--fix`: dedupe every table under `designs` (with the merge base when in a merge); `dry_run`
/// writes nothing. Returns one line per table changed and its notes.
pub fn fix_all(designs: &Path, dry_run: bool) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for t in tables(designs) {
        let text = std::fs::read_to_string(&t.path).with_context(|| format!("reading {}", t.path.display()))?;
        let base = merge_base_text(&t.path);
        let (new, removed, notes) = dedupe(&t, &text, base.as_deref());
        if new == text {
            continue;
        }
        out.push(format!("FIXED {}: {removed} row(s) removed", t.path.display()));
        out.extend(notes.into_iter().map(|n| format!("  {n}")));
        if !dry_run {
            std::fs::write(&t.path, new)?;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules() {
        let t = Table { path: PathBuf::from("/nonexistent/verdicts.tsv"), key: vec![1], stamp: Some(0), clock_hash: false };
        let text = "archived\tfile\tx\n2026-10-21T09:00Z\ta.txt\tnew\n2026-10-20T09:00Z\tb.txt\t1\n2026-10-20T10:00Z\ta.txt\told\n2026-10-20T09:00Z\tb.txt\t1\t2\narchived\tfile\tx\n";
        let (out, removed, notes) = dedupe(&t, text, None);
        assert_eq!(out, "archived\tfile\tx\n2026-10-21T09:00Z\ta.txt\tnew\n2026-10-20T09:00Z\tb.txt\t1\t2\n");
        assert_eq!(removed, 2);
        assert_eq!(notes.len(), 2);
        // no freshness signal: the merge base decides, else the last row
        let t2 = Table { stamp: None, ..t };
        let text2 = "h\tfile\nold\ta.txt\nnew\ta.txt\n".replace("old\ta", "x\ta").replace("new\ta", "y\ta");
        let (o, _, n) = dedupe(&t2, &text2, Some("h\tfile\ny\ta.txt\n"));
        assert_eq!(o, "h\tfile\nx\ta.txt\n");
        assert!(n[0].contains("merge base"));
        let (o, _, n) = dedupe(&t2, &text2, None);
        assert_eq!(o, "h\tfile\ny\ta.txt\n");
        assert!(n[0].contains("AMBIGUOUS"));
    }
}
