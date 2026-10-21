//! `prune`: remove archived designs that are instances of an extendable family (their family
//! card replaces them): the design file, EVERY row naming it in verdicts-v1.tsv and captions.tsv,
//! and its picture. An instance is kept when it is also the smallest archived clock of its period
//! outside the family (no non-member clock of that period with as few blocks), when its file does
//! not start with the given prefix (another track's), or when it is named with --keep.

use anyhow::{Context, Result};
use clap::Args as ClapArgs;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct PruneArgs {
    /// `members` output: <file>\t<family>\t<n>\t<P> per line
    #[arg(long)]
    pub members: Option<PathBuf>,
    /// Instead: remove the verdict and caption rows (with the prefix) whose design file is gone
    /// (rows a union merge brought back after their instance was removed)
    #[arg(long)]
    pub orphans: bool,
    /// The archive folder
    #[arg(long, default_value = "designs/clocks/archive")]
    pub dir: PathBuf,
    /// The pictures folder
    #[arg(long, default_value = "designs/clocks/img")]
    pub img: PathBuf,
    /// Only files starting with this prefix are removed
    #[arg(long, default_value = "clk-obs-")]
    pub prefix: String,
    /// File names to keep anyway
    #[arg(long)]
    pub keep: Vec<String>,
    /// Print what would go, change nothing
    #[arg(long)]
    pub dry_run: bool,
}

pub fn run(a: PruneArgs) -> Result<()> {
    let verdicts_path = a.dir.join("verdicts-v1.tsv");
    let captions_path = a.dir.join("captions.tsv");
    let verdicts = std::fs::read_to_string(&verdicts_path).context("verdicts-v1.tsv")?;
    let captions = std::fs::read_to_string(&captions_path).context("captions.tsv")?;
    if a.orphans {
        // a row is an orphan when its file (first column of captions, second of verdicts) has
        // the prefix and does not exist
        let gone = |f: &str| f.starts_with(&a.prefix) && !a.dir.join(f).exists();
        let mut n = 0usize;
        let keep_v: Vec<&str> = verdicts.lines().filter(|l| !l.split('\t').nth(1).is_some_and(|f| gone(f))).collect();
        let keep_c: Vec<&str> = captions.lines().filter(|l| !l.split('\t').next().is_some_and(|f| gone(f))).collect();
        n += verdicts.lines().count() - keep_v.len();
        n += captions.lines().count() - keep_c.len();
        println!("orphan rows: verdicts {} -> {}, captions {} -> {} ({n} removed)", verdicts.lines().count(), keep_v.len(), captions.lines().count(), keep_c.len());
        if !a.dry_run {
            std::fs::write(&verdicts_path, keep_v.join("\n") + "\n")?;
            std::fs::write(&captions_path, keep_c.join("\n") + "\n")?;
        }
        return Ok(());
    }
    let Some(members_path) = &a.members else { anyhow::bail!("prune needs --members (or --orphans)") };
    let members: Vec<(String, u32)> = std::fs::read_to_string(members_path)?
        .lines()
        .filter_map(|l| {
            let v: Vec<&str> = l.split('\t').collect();
            let file = std::path::Path::new(v.first()?).file_name()?.to_string_lossy().to_string();
            Some((file, v.get(3)?.parse().ok()?))
        })
        .collect();
    let member_set: HashSet<&str> = members.iter().map(|(f, _)| f.as_str()).collect();
    // the smallest non-member clock per period (from the verdict rows: file, period, blocks)
    let mut smallest: BTreeMap<u32, usize> = BTreeMap::new();
    let mut blocks_of: BTreeMap<String, usize> = BTreeMap::new();
    for l in verdicts.lines().skip(1) {
        let v: Vec<&str> = l.split('\t').collect();
        let (Some(file), Some(p), Some(b)) = (v.get(1), v.get(2).and_then(|s| s.parse::<u32>().ok()), v.get(8).and_then(|s| s.parse::<usize>().ok())) else { continue };
        blocks_of.insert(file.to_string(), b);
        if !member_set.contains(file) {
            let e = smallest.entry(p).or_insert(usize::MAX);
            *e = (*e).min(b);
        }
    }
    let mut gone: HashSet<String> = HashSet::new();
    for (file, p) in &members {
        if !file.starts_with(&a.prefix) || a.keep.contains(file) {
            println!("keep {file} (P{p}: not ours or kept by name)");
            continue;
        }
        let b = blocks_of.get(file).copied().unwrap_or(usize::MAX);
        // kept only when it ALSO beats (or ties) every clock of its period outside the family;
        // a period only the family reaches is shown by the family card
        if smallest.get(p).is_some_and(|m| b <= *m) {
            println!("keep {file} (P{p}, {b} blocks: the smallest clock of its period outside the family)");
            continue;
        }
        gone.insert(file.clone());
    }
    let keep_line = |l: &&str, col: usize| -> bool { l.split('\t').nth(col).is_none_or(|f| !gone.contains(f)) };
    let new_verdicts: Vec<&str> = verdicts.lines().filter(|l| keep_line(l, 1)).collect();
    let new_captions: Vec<&str> = captions.lines().filter(|l| keep_line(l, 0)).collect();
    println!(
        "{} instances to remove; verdict rows {} -> {}, caption rows {} -> {}",
        gone.len(),
        verdicts.lines().count(),
        new_verdicts.len(),
        captions.lines().count(),
        new_captions.len()
    );
    if a.dry_run {
        return Ok(());
    }
    for f in &gone {
        let _ = std::fs::remove_file(a.dir.join(f));
        let stem = f.trim_end_matches(".txt");
        let _ = std::fs::remove_file(a.img.join(format!("{stem}.jpg")));
    }
    std::fs::write(&verdicts_path, new_verdicts.join("\n") + "\n")?;
    std::fs::write(&captions_path, new_captions.join("\n") + "\n")?;
    Ok(())
}
