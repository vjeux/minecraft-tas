//! Turn `mctas clock verify` logs into the track's list of verified clocks: every `CLOCK:` line
//! matched to its design file (by the design's `name:`), as a markdown table sorted by period,
//! family and blocks, plus the coordinator's one-line format per clock.

use anyhow::Result;
use clap::Args as ClapArgs;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct ReportArgs {
    /// clock verify logs
    #[arg(long, num_args = 1..)]
    pub logs: Vec<PathBuf>,
    /// Folders (or files) of the designs the logs verified
    #[arg(long, num_args = 1..)]
    pub designs: Vec<PathBuf>,
    /// Write the markdown table here
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Print the coordinator lines (`clk-obs: clock P=..gt <family> <blocks> blocks, <path>, <note>`)
    #[arg(long)]
    pub lines: bool,
}

pub struct Verified {
    pub name: String,
    pub period: u32,
    pub family: String,
    pub blocks: u32,
    pub dims: String,
    pub flag: String,
    pub wave: String,
    pub start: String,
    pub path: String,
}

pub fn parse_logs(logs: &[PathBuf], names: &HashMap<String, String>) -> Result<Vec<Verified>> {
    let mut out = Vec::new();
    for l in logs {
        let text = std::fs::read_to_string(l)?;
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let Some(rest) = line.strip_prefix("CLOCK: ") else { continue };
            // CLOCK: <name> period P gt, on Q/P, edges E, warmup W, family F, B blocks, WxHxD, flag
            let name = rest.split_whitespace().next().unwrap_or("").to_string();
            let field = |key: &str| -> Option<String> {
                rest.split(", ").find_map(|f| f.strip_prefix(key).map(|v| v.trim().to_string()))
            };
            let period: u32 = rest.split(" period ").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok()).unwrap_or(0);
            let parts: Vec<&str> = rest.split(", ").collect();
            let blocks: u32 = parts.iter().find(|p| p.ends_with(" blocks")).and_then(|p| p.split_whitespace().next()).and_then(|s| s.parse().ok()).unwrap_or(0);
            let dims = parts.iter().find(|p| p.chars().next().is_some_and(|c| c.is_ascii_digit()) && p.contains('x') && !p.contains(' ')).map(|s| s.to_string()).unwrap_or_default();
            let flag = parts.last().map(|s| s.to_string()).unwrap_or_default();
            let wave = format!("on {}, edges {}", field("on ").unwrap_or_default(), field("edges ").unwrap_or_default());
            let start = lines.get(i + 1).map(|s| s.trim().split(';').next().unwrap_or("").to_string()).unwrap_or_default();
            out.push(Verified {
                path: names.get(&name).cloned().unwrap_or_default(),
                name,
                period,
                family: field("family ").unwrap_or_default(),
                blocks,
                dims,
                flag,
                wave,
                start,
            });
        }
    }
    out.sort_by(|a, b| (a.period, &a.family, a.blocks, &a.name).cmp(&(b.period, &b.family, b.blocks, &b.name)));
    Ok(out)
}

pub fn run(a: ReportArgs) -> Result<()> {
    let mut names: HashMap<String, String> = HashMap::new();
    for f in crate::pick::files_of(&a.designs)? {
        if let Ok(text) = std::fs::read_to_string(&f) {
            if let Some(n) = text.lines().find_map(|l| l.strip_prefix("name:")) {
                names.insert(n.trim().to_string(), f.to_string_lossy().to_string());
            }
        }
    }
    let v = parse_logs(&a.logs, &names)?;
    let mut md = String::from("| P (gt) | family | blocks | box | output wave | start | judge | design |\n|---|---|---|---|---|---|---|---|\n");
    for r in &v {
        md.push_str(&format!("| {} | {} | {} | {} | {} | {} | {} | {} |\n", r.period, r.family, r.blocks, r.dims, r.wave, r.start, r.flag, r.path));
    }
    if let Some(o) = &a.out {
        std::fs::write(o, &md)?;
    } else {
        print!("{md}");
    }
    if a.lines {
        for r in &v {
            println!("clk-obs: clock P={}gt {} {} blocks, {}, {} ({}, {})", r.period, r.family, r.blocks, r.path, r.flag, r.dims, r.start);
        }
    }
    eprintln!("{} verified clocks", v.len());
    Ok(())
}
