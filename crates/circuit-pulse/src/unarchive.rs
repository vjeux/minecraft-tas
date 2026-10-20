//! `unarchive`: remove archived circuit designs (the file, its rows in verdicts.tsv and
//! captions.tsv), e.g. the single instances an extendable family card now covers.

use anyhow::Result;
use clap::Args as ClapArgs;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct UnarchiveArgs {
    /// The study's archive folder
    #[arg(long, default_value = "designs/circuits/pulse/archive")]
    pub dir: PathBuf,
    /// File names (as in verdicts.tsv) to remove
    pub files: Vec<String>,
}

pub fn run(a: UnarchiveArgs) -> Result<()> {
    let gone: HashSet<String> = a.files.iter().map(|f| std::path::Path::new(f).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()).collect();
    for (table, col) in [("verdicts.tsv", 1usize), ("captions.tsv", 0usize)] {
        let path = a.dir.join(table);
        let text = std::fs::read_to_string(&path)?;
        let kept: Vec<&str> = text.lines().filter(|l| l.split('\t').nth(col).is_none_or(|f| !gone.contains(f))).collect();
        println!("{table}: {} -> {} rows", text.lines().count(), kept.len());
        std::fs::write(&path, kept.join("\n") + "\n")?;
    }
    for f in &gone {
        if std::fs::remove_file(a.dir.join(f)).is_ok() {
            println!("removed {f}");
        }
    }
    Ok(())
}
