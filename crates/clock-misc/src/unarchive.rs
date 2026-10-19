//! `mctas clock misc unarchive FILES...`: take a track's own clocks back out of the archive (to
//! re-archive a rebuilt version under the same name): removes `designs/clocks/archive/<file>`, its
//! row in verdicts-v1.tsv (column 2) and captions.tsv (column 1), and, when the design heads an
//! extendable family (`extend family:`), that family's rows in extend.tsv. Run from the
//! repository root; then `clock archive` the new version and `clock catalog`.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::path::{Path, PathBuf};

#[derive(ClapArgs, Debug, Clone)]
pub struct UnarchiveArgs {
    /// Archive file names (e.g. clk-misc-cart-loop-n0.txt) or paths to them.
    pub files: Vec<PathBuf>,
}

const DIR: &str = "designs/clocks/archive";

/// Remove the lines of `tsv` whose column `col` (0-based) is `key`; returns how many.
fn drop_rows(tsv: &Path, col: usize, key: &str) -> Result<usize> {
    let text = std::fs::read_to_string(tsv).with_context(|| format!("read {}", tsv.display()))?;
    let mut kept = String::with_capacity(text.len());
    let mut dropped = 0;
    for line in text.lines() {
        if line.split('\t').nth(col) == Some(key) {
            dropped += 1;
        } else {
            kept.push_str(line);
            kept.push('\n');
        }
    }
    if dropped > 0 {
        std::fs::write(tsv, kept)?;
    }
    Ok(dropped)
}

pub fn run(a: UnarchiveArgs) -> Result<()> {
    if a.files.is_empty() {
        bail!("clock misc unarchive FILES...");
    }
    let dir = Path::new(DIR);
    if !dir.is_dir() {
        bail!("run from the repository root ({DIR} not found)");
    }
    for f in &a.files {
        let name = f.file_name().and_then(|s| s.to_str()).context("file name")?.to_string();
        let path = dir.join(&name);
        let family = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.lines().find_map(|l| l.strip_prefix("extend family:").map(|v| v.trim().to_string())));
        let v = drop_rows(&dir.join("verdicts-v1.tsv"), 1, &name)?;
        let c = drop_rows(&dir.join("captions.tsv"), 0, &name)?;
        let e = match &family {
            Some(fam) => drop_rows(&dir.join("extend.tsv"), 0, fam)?,
            None => 0,
        };
        let removed = if path.exists() {
            std::fs::remove_file(&path)?;
            true
        } else {
            false
        };
        println!(
            "{name}: design {}, {v} verdict row(s), {c} caption row(s){}",
            if removed { "removed" } else { "not found" },
            family.map(|f| format!(", {e} extend row(s) of family {f}")).unwrap_or_default()
        );
    }
    Ok(())
}
