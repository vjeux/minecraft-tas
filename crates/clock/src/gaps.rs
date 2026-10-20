//! Evenly spaced outputs (coordinator, 2026-10-20, from vjeux's site critique C38): a clock on the
//! site must deliver evenly spaced pulses at its declared output.
//!
//! * `clock gaps [--write]`: the `gaps` column of the verdict table (the ticks between the
//!   output's rising edges over one period), backfilled from redsim for every row whose redsim
//!   output reads like the judge's line (`on`, `edges`, `pulses`); the others are listed (the game
//!   measures them: `clock retap --game`).
//! * `clock retap FILES... [--write]`: a clock whose output pulses unevenly but has another cell
//!   that pulses evenly (the longest such period) gets that cell as its `port output`, is judged
//!   again in the game, and with `--write` replaces its archived design and verdict row when the
//!   judge confirms the same period with an even output. A clock with no even cell stays as it is:
//!   uneven, out of the catalog and the site.

use crate::catalog::{Row, VERDICTS, content_hash, read_rows, uneven};
use crate::sim::{SimOptions, Verdict, simulate};
use crate::verify::{VerifyOptions, verify_in_pool};
use crate::{Clock, Pos};
use anyhow::{Context, Result};
use std::path::Path;

/// What `backfill` did.
pub struct Backfill {
    pub filled: usize,
    pub uneven: usize,
    /// Rows redsim could not measure like the judge (UNMODELED, or its output reads differently).
    pub unknown: Vec<String>,
}

/// Fill the `gaps` column from redsim (rows that already have one are kept).
pub fn backfill(dir: &Path, write: bool) -> Result<Backfill> {
    let mut rows = read_rows(dir)?;
    let mut out = Backfill { filled: 0, uneven: 0, unknown: Vec::new() };
    for r in rows.iter_mut() {
        if !r.gaps.is_empty() {
            out.filled += 1;
            out.uneven += r.uneven() as usize;
            continue;
        }
        match measured(dir, r) {
            Some(g) => {
                r.gaps = format!("{g:?}");
                out.filled += 1;
                out.uneven += uneven(&g) as usize;
            }
            None => out.unknown.push(r.file.clone()),
        }
    }
    if write {
        write_rows(dir, &rows)?;
    }
    Ok(out)
}

/// redsim's output gaps for an archived row, when redsim reads its output like the judge did.
fn measured(dir: &Path, r: &Row) -> Option<Vec<u32>> {
    let c = Clock::load(&dir.join(&r.file)).ok()?;
    let s = simulate(&c, &SimOptions::default());
    if s.verdict != Verdict::Clock || s.period != r.period {
        return None;
    }
    let w = &s.output.as_ref()?.wave;
    (w.on == r.on && w.edges == r.edges && format!("{:?}", w.widths) == r.pulses).then(|| w.gaps.clone())
}

/// Rewrite the verdict table with these rows (header first).
pub fn write_rows(dir: &Path, rows: &[Row]) -> Result<()> {
    let mut all = String::new();
    all.push_str(crate::catalog::header());
    all.push('\n');
    for r in rows {
        all.push_str(&r.tsv());
        all.push('\n');
    }
    std::fs::write(dir.join(VERDICTS), all)?;
    Ok(())
}

/// What `retap` did to one archived clock.
pub enum Retap {
    /// Its output already pulses evenly.
    Even,
    /// The output moved to an even cell, confirmed in the game: the new judge line and row fields.
    Moved { from: Option<Pos>, to: Pos, line: String, warmup: u32, on: u32, edges: u32, pulses: String, gaps: String, robustness: String },
    /// No cell of it pulses evenly.
    NoTap(String),
    /// The game did not confirm (another period, still uneven, or not a clock): its line.
    Refused { to: Pos, line: String },
    /// Not a survival clock by its design (command-only entities, daylight, tripwire).
    Hide(String),
    Skip(String),
}

impl Retap {
    /// One line of a results file (`clock retap --results`), for `--apply`.
    pub fn result_line(&self, file: &str) -> String {
        let p = |q: &Pos| format!("{},{},{}", q[0], q[1], q[2]);
        let clean = |s: &str| s.replace(['\t', '\n'], " ");
        match self {
            Retap::Even => format!("{file}\teven"),
            Retap::Moved { from, to, line, warmup, on, edges, pulses, gaps, robustness } => {
                format!("{file}\tmove\t{}\t{}\t{warmup}\t{on}\t{edges}\t{pulses}\t{gaps}\t{robustness}\t{}", p(to), from.as_ref().map(p).unwrap_or_else(|| "-".into()), clean(line))
            }
            Retap::NoTap(why) => format!("{file}\tnotap\t{}", clean(why)),
            Retap::Refused { to, line } => format!("{file}\trefused\t{}\t{}", p(to), clean(line)),
            Retap::Hide(why) => format!("{file}\thide\t{}", clean(why)),
            Retap::Skip(why) => format!("{file}\tskip\t{}", clean(why)),
        }
    }
}

/// Why a clock's DESIGN cannot run in a normal survival world (coordinator's rulings on the
/// critique, 2026-10-20), or None: entities with data a player cannot give (a never-despawning or
/// unpickable item, a minecart started with a push from a command, no gravity, invulnerable); a
/// daylight detector (it stops at night or in rain); tripwire (any passing mob, player or item
/// fires it).
pub fn design_hide(c: &Clock) -> Option<String> {
    for e in &c.design.entities {
        let nbt = e.nbt.clone().unwrap_or_default();
        for (k, why) in [("Age:-32768", "an item that never despawns"), ("PickupDelay:32767", "an item no one can pick up"), ("NoGravity:1", "no gravity"), ("Invulnerable:1", "invulnerable")] {
            if nbt.contains(k) {
                return Some(format!("command-only: {} ({why}): not buildable in survival", crate::short(&e.kind)));
            }
        }
        if let Some(i) = nbt.find("Motion:[") {
            let list = &nbt[i + 8..nbt[i..].find(']').map(|j| i + j).unwrap_or(nbt.len())];
            if list.split(',').any(|x| x.trim().trim_end_matches(['d', 'f', 'D', 'F']).parse::<f64>().is_ok_and(|v| v != 0.0)) {
                return Some(format!("command-only: {} started with a Motion from a command: not buildable in survival", crate::short(&e.kind)));
            }
        }
    }
    let blocks: Vec<&String> = c.design.blocks.values().collect();
    if blocks.iter().any(|s| s.contains("daylight_detector")) {
        return Some("daytime only: a daylight detector (it stops or changes at night and in rain)".into());
    }
    if blocks.iter().any(|s| s.contains("tripwire")) {
        return Some("tripwire: any passing mob, player or dropped item fires it".into());
    }
    None
}

/// The design text with its `port output` at `pos` (the old line replaced, else a new one after the
/// last `port` line or the `name:` line), with a comment, and its `# CLOCK:` header line replaced by
/// `line` when given.
pub fn with_output(text: &str, pos: Pos, from: Option<Pos>, line: Option<&str>) -> Result<String> {
    let lines: Vec<&str> = text.lines().collect();
    let old = lines.iter().position(|l| l.starts_with("port output:"));
    let at = old.or_else(|| lines.iter().rposition(|l| l.starts_with("port "))).or_else(|| lines.iter().position(|l| l.starts_with("name:"))).context("no `name:` line")?;
    let was = match from {
        Some(p) => format!(" (was {} {} {}: the old output was uneven, entered during the cycle, or not an exterior air cell)", p[0], p[1], p[2]),
        None => String::new(),
    };
    let mut out = String::with_capacity(text.len() + 200);
    for (i, l) in lines.iter().enumerate() {
        if let (Some(new), true) = (line, l.starts_with("# CLOCK: ")) {
            out.push_str(&format!("# {new}\n"));
            continue;
        }
        if Some(i) == old {
            out.push_str(&format!("# port output moved 2026-10-20 (clockinfra): an exterior air cell where the judge's lamp lights at evenly spaced ticks{was}\n"));
            out.push_str(&format!("port output: {} {} {}\n", pos[0], pos[1], pos[2]));
            continue;
        }
        out.push_str(l);
        out.push('\n');
        if old.is_none() && i == at {
            out.push_str(&format!("# port output declared 2026-10-20 (clockinfra): an exterior air cell where the judge's lamp lights at evenly spaced ticks{was}\n"));
            out.push_str(&format!("port output: {} {} {}\n", pos[0], pos[1], pos[2]));
        }
    }
    Ok(out)
}

/// Give one archived clock an output a builder can use (vjeux / coordinator, 2026-10-20): an
/// exterior air cell where the judge's lamp lights at evenly spaced ticks, free all cycle, not
/// above tripwire, rails or water. A clock whose declared output already passes is left alone;
/// otherwise redsim ranks the candidate cells ([`crate::sim::lamp_outputs`]) and the game judges
/// the best ones in turn, the first it confirms (same period, an even lamp) replacing the design
/// and its verdict row. A clock redsim does not model is skipped (the game alone cannot search).
pub fn retap(pool: &mctas::sim::Pool, o: &VerifyOptions, dir: &Path, file: &str, write: bool) -> Result<Retap> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let c = Clock::load(&path)?;
    if let Some(why) = design_hide(&c) {
        return Ok(Retap::Hide(why));
    }
    let s = simulate(&c, &SimOptions::default());
    let even = s.output.as_ref().is_some_and(|o| !o.wave.gaps.is_empty() && o.wave.irregular().is_none());
    if s.verdict == Verdict::Clock && crate::sim::judge_lamp(&c).is_some() && even {
        return Ok(Retap::Even);
    }
    let mut d = c.design.clone();
    d.ports.remove("output");
    let free = Clock::from_design(d)?;
    let fs = simulate(&free, &SimOptions::default());
    if fs.verdict != Verdict::Clock {
        return Ok(Retap::Skip(format!("clock sim (no output): {}", fs.summary())));
    }
    let from = c.output;
    let cands = crate::sim::lamp_outputs(&free, 3);
    if cands.is_empty() {
        return Ok(Retap::NoTap(format!("no exterior air cell where a lamp lights evenly ({})", if s.verdict == Verdict::Clock { "its output is uneven or inside" } else { s.why.as_str() })));
    }
    let mut last = None;
    for (to, _) in cands {
        let draft = with_output(&text, to, from, None)?;
        let nc = Clock::from_design(mctas::design::Design::parse(&draft)?)?;
        let rep = verify_in_pool(pool, &nc, o)?;
        let line = rep.summary();
        let ok = rep.base().filter(|_| rep.verdict == Verdict::Clock).and_then(|b| b.output.as_ref().map(|o| (b, &o.wave))).filter(|(b, w)| b.period == fs.period && w.irregular().is_none() && !w.gaps.is_empty());
        let Some((b, bw)) = ok else {
            last = Some(Retap::Refused { to, line });
            continue;
        };
        if write {
            let mut m = std::collections::BTreeMap::new();
            m.insert(file.to_string(), Retap::Moved { from, to, line: line.clone(), warmup: b.warmup, on: bw.on, edges: bw.edges, pulses: format!("{:?}", bw.widths), gaps: format!("{:?}", bw.gaps), robustness: rep.robustness.clone() });
            apply(dir, &m)?;
        }
        return Ok(Retap::Moved { from, to, line, warmup: b.warmup, on: bw.on, edges: bw.edges, pulses: format!("{:?}", bw.widths), gaps: format!("{:?}", bw.gaps), robustness: rep.robustness.clone() });
    }
    Ok(last.unwrap_or(Retap::NoTap("no candidate".into())))
}

/// The chunk-reload pass over a clock still shown: judged in the game with a chunk reload mid-run
/// (`clock verify --reload`); one that does not stay a robust clock of its period is hidden.
pub fn reload_check(pool: &mctas::sim::Pool, o: &VerifyOptions, dir: &Path, file: &str) -> Result<Retap> {
    let c = Clock::load(&dir.join(file))?;
    let row = read_rows(dir)?.into_iter().find(|r| r.file == file);
    let mut ro = o.clone();
    ro.reload = true;
    let rep = verify_in_pool(pool, &c, &ro)?;
    let same = rep.verdict == Verdict::Clock && rep.robustness == "robust" && row.as_ref().is_none_or(|r| rep.base().is_some_and(|b| b.period == r.period));
    if same {
        Ok(Retap::Even)
    } else {
        Ok(Retap::Hide(format!("does not survive a chunk reload: {}", rep.summary())))
    }
}

/// Read results files (`clock retap --results`): the last result per file.
pub fn read_results(paths: &[std::path::PathBuf]) -> Result<std::collections::BTreeMap<String, Retap>> {
    let mut out = std::collections::BTreeMap::new();
    for p in paths {
        let text = std::fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        for l in text.lines() {
            let f: Vec<&str> = l.split('\t').collect();
            if f.len() < 2 {
                continue;
            }
            let pos = |s: &str| -> Option<Pos> {
                let v: Vec<i32> = s.split(',').filter_map(|x| x.parse().ok()).collect();
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            };
            let r = match f[1] {
                "even" => Retap::Even,
                "move" if f.len() >= 11 => Retap::Moved {
                    to: pos(f[2]).context("move: to")?,
                    from: pos(f[3]),
                    warmup: f[4].parse()?,
                    on: f[5].parse()?,
                    edges: f[6].parse()?,
                    pulses: f[7].into(),
                    gaps: f[8].into(),
                    robustness: f[9].into(),
                    line: f[10].into(),
                },
                "notap" => Retap::NoTap(f.get(2).copied().unwrap_or("").into()),
                "refused" if f.len() >= 4 => Retap::Refused { to: pos(f[2]).context("refused: to")?, line: f[3].into() },
                "hide" => Retap::Hide(f.get(2).copied().unwrap_or("").into()),
                "skip" => Retap::Skip(f.get(2).copied().unwrap_or("").into()),
                _ => continue,
            };
            out.insert(f[0].to_string(), r);
        }
    }
    Ok(out)
}

/// Counts of an `apply`: per outcome.
#[derive(Default, Debug)]
pub struct Applied {
    pub moved: usize,
    pub kept: usize,
    pub hidden: std::collections::BTreeMap<String, Vec<String>>,
}

/// Apply results to the archive: a moved clock's design (its `port output` and `# CLOCK:` line) and
/// verdict row (output, gaps, robustness, line, hash) are rewritten; every row's `hidden` column is
/// set: the reason it is not a survival clock, or empty. A clock that only works in some rotations
/// or positions is hidden too (coordinator: any direction, anywhere).
pub fn apply(dir: &Path, results: &std::collections::BTreeMap<String, Retap>) -> Result<Applied> {
    let mut rows = read_rows(dir)?;
    let mut out = Applied::default();
    let hide = |out: &mut Applied, r: &mut Row, kind: &str, why: String| {
        r.hidden = why;
        out.hidden.entry(kind.to_string()).or_default().push(r.file.clone());
    };
    for r in rows.iter_mut() {
        let Some(res) = results.get(&r.file) else { continue };
        match res {
            Retap::Moved { from, to, line, warmup, on, edges, pulses, gaps, robustness } => {
                let path = dir.join(&r.file);
                let text = std::fs::read_to_string(&path)?;
                let final_text = with_output(&text, *to, *from, Some(line))?;
                std::fs::write(&path, &final_text)?;
                let fc = Clock::from_design(mctas::design::Design::parse(&final_text)?)?;
                r.hash = content_hash(&fc);
                r.warmup = *warmup;
                r.on = *on;
                r.edges = *edges;
                r.pulses = pulses.clone();
                r.gaps = gaps.clone();
                r.robustness = robustness.clone();
                r.summary = line.clone();
                r.hidden.clear();
                out.moved += 1;
            }
            Retap::Even => {
                r.hidden.clear();
                out.kept += 1;
            }
            Retap::NoTap(why) => hide(&mut out, r, "no output", format!("no output: {why}")),
            Retap::Refused { line, .. } => hide(&mut out, r, "refused", format!("no output the game confirms: {line}")),
            Retap::Hide(why) => {
                let kind = why.split(':').next().unwrap_or("hidden").to_string();
                hide(&mut out, r, &kind, why.clone());
            }
            Retap::Skip(why) => hide(&mut out, r, "not judged", format!("not judged on redsim: {why}")),
        }
        if r.hidden.is_empty() && r.robustness != "robust" {
            let why = format!("{}: it must work in any direction, anywhere", r.robustness);
            hide(&mut out, r, "not robust", why);
        }
    }
    write_rows(dir, &rows)?;
    Ok(out)
}
