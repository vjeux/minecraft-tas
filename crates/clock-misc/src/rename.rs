//! `mctas clock misc rename --prefix P FILES...`: set each design's `name:` line to `P-<file
//! stem>` (search tools write designs named after their own track; the archive keys designs by
//! name, so a track renames its picks before archiving them).

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct RenameArgs {
    /// The new name prefix (the name becomes `<prefix>-<file stem>`).
    #[arg(long)]
    pub prefix: String,
    pub files: Vec<PathBuf>,
}

pub fn run(a: RenameArgs) -> Result<()> {
    if a.files.is_empty() {
        bail!("clock misc rename --prefix P FILES...");
    }
    for f in &a.files {
        let text = std::fs::read_to_string(f).with_context(|| format!("read {}", f.display()))?;
        let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("design");
        let name = format!("{}-{stem}", a.prefix);
        let mut found = false;
        let out: Vec<String> = text
            .lines()
            .map(|l| {
                if !found && l.starts_with("name:") {
                    found = true;
                    format!("name: {name}")
                } else {
                    l.to_string()
                }
            })
            .collect();
        let mut body = out.join("\n");
        body.push('\n');
        if !found {
            body = format!("name: {name}\n{body}");
        }
        std::fs::write(f, body)?;
        println!("{}: {name}", f.display());
    }
    Ok(())
}
