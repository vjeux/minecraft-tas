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
    /// The output moved to an even cell, confirmed in the game: the new judge line.
    Moved { from: Option<Pos>, to: Pos, line: String },
    /// No cell of it pulses evenly.
    NoTap(String),
    /// The game did not confirm (another period, still uneven, or not a clock): its line.
    Refused { to: Pos, line: String },
    Skip(String),
}

/// The design text with its `port output` at `pos` (the old line replaced, else a new one after the
/// last `port` line or the `name:` line), with a comment, and its `# CLOCK:` header line replaced by
/// `line` when given.
pub fn with_output(text: &str, pos: Pos, from: Option<Pos>, line: Option<&str>) -> Result<String> {
    let lines: Vec<&str> = text.lines().collect();
    let old = lines.iter().position(|l| l.starts_with("port output:"));
    let at = old.or_else(|| lines.iter().rposition(|l| l.starts_with("port "))).or_else(|| lines.iter().position(|l| l.starts_with("name:"))).context("no `name:` line")?;
    let was = match from {
        Some(p) => format!(" (was {} {} {}, whose pulses were unevenly spaced)", p[0], p[1], p[2]),
        None => String::new(),
    };
    let mut out = String::with_capacity(text.len() + 200);
    for (i, l) in lines.iter().enumerate() {
        if let (Some(new), true) = (line, l.starts_with("# CLOCK: ")) {
            out.push_str(&format!("# {new}\n"));
            continue;
        }
        if Some(i) == old {
            out.push_str(&format!("# port output moved 2026-10-20 (clockinfra): a cell whose pulses are evenly spaced{was}\n"));
            out.push_str(&format!("port output: {} {} {}\n", pos[0], pos[1], pos[2]));
            continue;
        }
        out.push_str(l);
        out.push('\n');
        if old.is_none() && i == at {
            out.push_str(&format!("# port output declared 2026-10-20 (clockinfra): a cell whose pulses are evenly spaced{was}\n"));
            out.push_str(&format!("port output: {} {} {}\n", pos[0], pos[1], pos[2]));
        }
    }
    Ok(out)
}

/// Move one archived clock's output to an even cell (redsim picks it, or the game when redsim
/// does not model the clock; the game confirms).
pub fn retap(pool: &mctas::sim::Pool, o: &VerifyOptions, dir: &Path, file: &str, write: bool) -> Result<Retap> {
    let path = dir.join(file);
    let text = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let c = Clock::load(&path)?;
    let s = simulate(&c, &SimOptions::default());
    // (period, output position, output wave, the changing cells): redsim's, else the game's
    let (period, from, w, cells) = if s.verdict == Verdict::Clock {
        let Some(out) = s.output.clone() else { return Ok(Retap::Skip("no output".into())) };
        (s.period, Some(out.pos), out.wave, s.analysis.cells.clone())
    } else if s.verdict == Verdict::Unmodeled {
        let rep = verify_in_pool(pool, &c, o)?;
        let Some(b) = rep.base().filter(|_| rep.verdict == Verdict::Clock) else { return Ok(Retap::Skip(format!("the game: {}", rep.summary()))) };
        let Some(out) = b.output.clone() else { return Ok(Retap::Skip("no output".into())) };
        (b.period, Some(out.pos), out.wave, b.analysis.cells.clone())
    } else {
        return Ok(Retap::Skip(format!("clock sim: {}", s.summary())));
    };
    if write && s.verdict == Verdict::Unmodeled {
        // the game measured what redsim cannot: its gaps go in the row now (replaced below if the
        // output moves)
        let mut rows = read_rows(dir)?;
        if let Some(r) = rows.iter_mut().find(|r| r.file == file) {
            r.gaps = format!("{:?}", w.gaps);
        }
        write_rows(dir, &rows)?;
    }
    if w.irregular().is_none() {
        return Ok(Retap::Even);
    }
    let Some((_, to)) = cells.iter().filter_map(|c| c.output_period().map(|p| (p, c.pos))).max() else {
        return Ok(Retap::NoTap(format!("gaps {:?}", w.gaps)));
    };
    let draft = with_output(&text, to, from, None)?;
    let nc = Clock::from_design(mctas::design::Design::parse(&draft)?)?;
    let rep = verify_in_pool(pool, &nc, o)?;
    let line = rep.summary();
    let Some(b) = rep.base().filter(|_| rep.verdict == Verdict::Clock) else { return Ok(Retap::Refused { to, line }) };
    let Some(bw) = b.output.as_ref().map(|o| &o.wave) else { return Ok(Retap::Refused { to, line }) };
    if b.period != period || bw.irregular().is_some() || bw.gaps.is_empty() {
        return Ok(Retap::Refused { to, line });
    }
    if write {
        let final_text = with_output(&text, to, from, Some(&line))?;
        std::fs::write(&path, &final_text)?;
        let fc = Clock::from_design(mctas::design::Design::parse(&final_text)?)?;
        let mut rows = read_rows(dir)?;
        if let Some(r) = rows.iter_mut().find(|r| r.file == file) {
            r.hash = content_hash(&fc);
            r.warmup = b.warmup;
            r.on = bw.on;
            r.edges = bw.edges;
            r.pulses = format!("{:?}", bw.widths);
            r.gaps = format!("{:?}", bw.gaps);
            r.robustness = rep.robustness.clone();
            r.summary = line.clone();
        }
        write_rows(dir, &rows)?;
    }
    Ok(Retap::Moved { from, to, line })
}
