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
        let ok = |r: &Row| usable(r) == Some(true);
        let best_usable = rs.iter().find(|r| r.shown() && ok(r)).map(|r| (*r).clone());
        let hidden_usable = rs.iter().find(|r| !r.shown() && ok(r)).filter(|h| best_usable.as_ref().is_none_or(|b| key(h) < key(b))).map(|r| (*r).clone());
        out.push(PeriodLine { period: p, record: (*record).clone(), record_usable: ok(record), best_usable, hidden_usable });
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
