//! The study registry (coordinator 2026-10-21: "every new study now conflicts in the same core
//! files"). A study lives in its own module, `src/studies/<module>.rs`, which registers itself and
//! its kinds in `pub static STUDY: StudyDef`; `build.rs` declares every such file as a crate-root
//! module (`crate::<module>`) and collects their `STUDY` into [`all_studies`]. Nothing shared names a
//! registered study or kind: `Kind::parse`, `Kind::name`, `Kind::study`, the port checks
//! (`Circuit::check_ports`), `circuit verify` and `circuit sim` (crate::verify), the circuit list
//! (crate::archive: [`crate::archive::studies`], the axis), `mctas circuit run` (circuit-cli) and
//! the site (crates/site: the study's page and the kind's words) all read the [`StudyDef`]. Item ids
//! a study adds to redsim go in its redsim module's `MODULE_ITEMS` list (crates/redsim/build.rs
//! collects them), never in `container::MORE_ITEMS`.
//!
//! The kinds older than the registry keep their `Kind` variants and their arms in the shared
//! files; one moves here by turning its variant into a [`KindDef`] (crate::lift, the item elevator,
//! is the worked example: docs/circuits.md "Adding a study").

use crate::check::Outcome;
use crate::verify::{Build, Options, VerifyReport};
use crate::{Circuit, Pos};
use anyhow::{Result, bail};
use mctas::sim::Pool;
use std::path::Path;

/// A study, as its module registers it.
pub struct StudyDef {
    /// Its catalog directory, `designs/circuits/<id>/`, and its row in the circuit list.
    pub id: &'static str,
    /// Its title in the circuit list (`designs/circuits/CATALOG.md`).
    pub title: &'static str,
    /// The site's words (crates/site/src/explain.rs asks these first): its page's title and
    /// paragraph (`STUDIES`), what a visitor uses its builds for (`STUDY_USES`) and its line on
    /// the circuits index (`study_blurb`).
    pub site_title: &'static str,
    pub site_intro: &'static str,
    pub site_uses: &'static str,
    pub site_blurb: &'static str,
    /// Its kinds.
    pub kinds: &'static [KindDef],
}

/// One kind of a registered study.
pub struct KindDef {
    /// The `circuit:` line's first word (`item-elevator`).
    pub word: &'static str,
    /// The words after `word` on the `circuit:` line: their canonical form (what `Kind::name`
    /// prints after the word), or why the line is refused.
    pub parse: fn(&[&str]) -> Result<Vec<String>>,
    /// The input ports the kind takes (`port input.<name>`, in name order); none: it runs by
    /// itself.
    pub inputs: &'static [&'static str],
    /// The kind's own port checks (after the inputs'), on a loaded circuit and the kind's words.
    pub check: fn(&Circuit, &[String]) -> Result<()>,
    /// The study's judge in the game (`mctas circuit verify`).
    pub verify: fn(&Pool, &Circuit, &Options) -> Result<VerifyReport>,
    /// Its redsim twin (`mctas circuit sim`).
    pub twin: Twin,
    /// How the judge's builds agree (crate::verify::verdict: position- or rotation-dependence).
    pub agree: Agree,
    /// The schedule length `circuit sim` reports for the kind (its run, in gt).
    pub ticks: fn(&Circuit) -> u32,
    /// The circuit list's axis value (crate::archive): from the kind's words and a build's metrics
    /// (`m(name)`: the metric's value, empty if none).
    pub axis: fn(&[String], &dyn Fn(&str) -> String) -> String,
    /// `mctas circuit run --run`'s runs (the first is the default).
    pub runs: &'static [&'static str],
    /// One run of `runs` (`mctas circuit run`): in the game (`pool`) or on the twin (`None`), the
    /// build turned and moved as [`RunAt`] says.
    pub run: fn(Option<&Pool>, &Circuit, &RunAt) -> Result<Outcome>,
    /// The site's words for the kind (crates/site/src/explain.rs asks these first).
    pub site: Site,
}

/// A registered kind's redsim twin.
pub enum Twin {
    /// The whole judge on redsim, build by build like `verify` (`circuit sim`, `circuit run
    /// --builds`, `--compare-verify`): `Err` when redsim does not model the design.
    Exact(fn(&Circuit, &Options) -> std::result::Result<VerifyReport, String>),
    /// No twin: `circuit sim` says this.
    GameOnly(&'static str),
}

/// How the judge's builds agree (crate::verify::verdict).
pub enum Agree {
    /// Every metric equal.
    Metrics,
    /// Every build passes, rests alike (`at rest`) and its `key` metric (a number) is within `tol`
    /// (a fraction) of the base build's.
    Rate { key: &'static str, tol: f64 },
    /// Every metric but those whose names start with one of these (random ones).
    Except(&'static [&'static str]),
}

/// One run of `mctas circuit run`.
pub struct RunAt<'a> {
    /// One of the kind's `runs`.
    pub run: &'a str,
    pub turns: u8,
    pub offset: Pos,
    pub margin: i32,
    /// `--watch`: the kind's own per-tick note (stderr).
    pub watch: Option<&'a str>,
    /// `--trace DIR`: record the game run as an exact trace (kinds that record).
    pub trace: Option<&'a Path>,
}

/// The site's words for a registered kind; each gets the kind's whole `circuit:` text
/// (`item-elevator 5`).
pub struct Site {
    /// The kind's title (`kind_title`).
    pub title: fn(&str) -> String,
    /// What it does, in one sentence (`kind_summary`).
    pub summary: fn(&str) -> String,
    /// A metric's definition (`metric_definition`): `None` leaves it to the site.
    pub metric: fn(&str) -> Option<&'static str>,
}

/// Every registered study (`src/studies/*.rs`, in file name order).
pub fn all_studies() -> impl Iterator<Item = &'static StudyDef> {
    crate::STUDY_DEFS.iter().copied()
}

/// Every registered kind with its study.
pub fn all() -> impl Iterator<Item = (&'static StudyDef, &'static KindDef)> {
    all_studies().flat_map(|s| s.kinds.iter().map(move |k| (s, k)))
}

/// The kind registered under the `circuit:` line's first word, with its study.
pub fn find(word: &str) -> Option<(&'static StudyDef, &'static KindDef)> {
    all().find(|(_, d)| d.word == word)
}

/// The registered study whose catalog directory is `id`.
pub fn study(id: &str) -> Option<&'static StudyDef> {
    all_studies().find(|s| s.id == id)
}

/// A registered kind as a circuit claims it: its word and canonical words after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StudyKind {
    pub word: &'static str,
    pub args: Vec<String>,
}

impl StudyKind {
    /// The registered kind behind it.
    pub fn def(&self) -> &'static KindDef {
        find(self.word).expect("a StudyKind is only made from a registered word").1
    }

    /// Its study.
    pub fn study(&self) -> &'static StudyDef {
        find(self.word).expect("a StudyKind is only made from a registered word").0
    }

    /// The kind as written on the `circuit:` line.
    pub fn name(&self) -> String {
        std::iter::once(self.word.to_string()).chain(self.args.iter().cloned()).collect::<Vec<_>>().join(" ")
    }

    /// The `i`th word after the kind's word, as a number.
    pub fn num(&self, i: usize) -> Result<u32> {
        let Some(v) = self.args.get(i) else { bail!("{}: no word {}", self.name(), i + 1) };
        Ok(v.parse()?)
    }
}

/// Parse a `circuit:` line's words for a registered kind (`None`: the word is not registered).
pub fn parse(words: &[&str]) -> Option<Result<StudyKind>> {
    let (_, d) = find(words.first().copied()?)?;
    Some((d.parse)(&words[1..]).map(|args| StudyKind { word: d.word, args }))
}

/// A build disagrees with the base build (a passing one) under a registered kind's rule
/// (crate::verify::verdict: position- or rotation-dependence); a failing build always does.
pub fn builds_differ(agree: &Agree, b: &Build, base: &Build) -> bool {
    let ok = |b: &Build| b.result.as_ref().ok().filter(|k| k.outcome.pass).map(|k| k.outcome.clone());
    let (Some(o), Some(o0)) = (ok(b), ok(base)) else { return true };
    let metric = |o: &Outcome, key: &str| o.metrics.iter().find(|(n, _)| n == key).map(|(_, v)| v.clone());
    match agree {
        Agree::Metrics => o.metrics_text() != o0.metrics_text(),
        Agree::Except(skip) => {
            let keep = |o: &Outcome| o.metrics.iter().filter(|(n, _)| !skip.iter().any(|s| n.starts_with(s))).map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", ");
            keep(&o) != keep(&o0)
        }
        Agree::Rate { key, tol } => {
            let rate = |o: &Outcome| metric(o, key).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
            let (r, r0) = (rate(&o), rate(&o0));
            metric(&o, "at rest") != metric(&o0, "at rest") || (r - r0).abs() > tol * r.max(r0)
        }
    }
}
