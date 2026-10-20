//! `stamp-group`: put a tunable group's `group:` header (and optional `group title:` / `group
//! note:`) on designs, right after their `circuit:` line (replacing older ones): the catalog shows
//! every design of one group as one card.

use anyhow::Result;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct StampArgs {
    #[arg(long)]
    pub group: String,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub note: Option<String>,
    pub files: Vec<PathBuf>,
}

/// The design text with the group headers set.
pub fn stamp(text: &str, group: &str, title: Option<&str>, note: Option<&str>) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut done = false;
    let headers = {
        let mut h = vec![format!("group: {group}")];
        if let Some(t) = title {
            h.push(format!("group title: {t}"));
        }
        if let Some(n) = note {
            h.push(format!("group note: {n}"));
        }
        h
    };
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("group:") || t.starts_with("group title:") || t.starts_with("group note:") {
            continue;
        }
        out.push(line.to_string());
        if !done && t.starts_with("circuit:") {
            out.extend(headers.iter().cloned());
            done = true;
        }
    }
    if !done {
        // no circuit line: after the name
        let at = out.iter().position(|l| l.starts_with("name:")).map(|i| i + 1).unwrap_or(0);
        for (j, h) in headers.iter().enumerate() {
            out.insert(at + j, h.clone());
        }
    }
    out.join("\n") + "\n"
}

pub fn run(a: StampArgs) -> Result<()> {
    for f in &a.files {
        let text = std::fs::read_to_string(f)?;
        std::fs::write(f, stamp(&text, &a.group, a.title.as_deref(), a.note.as_deref()))?;
        println!("stamped {}", f.display());
    }
    Ok(())
}
