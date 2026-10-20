//! `items`: the variants of an item-timer design (a hopper or container whose item count sets the
//! pulse length) for other item counts: the `count:` in its one `nbt:` line, the trailing `-L<n>` of
//! its name and its "This one holds ..." comment line rewritten (L = per x items + add).

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct ItemsArgs {
    /// The design (one `nbt:` line holding exactly one `count:`).
    pub design: PathBuf,
    /// The item counts to write, comma-separated (e.g. 1,2,4,8).
    #[arg(long, value_delimiter = ',')]
    pub counts: Vec<u32>,
    /// Ticks per item (L = per x items + add).
    #[arg(long, default_value_t = 8)]
    pub per: i64,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub add: i64,
    /// Where the variants go (default: beside the design).
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

/// The design text for `n` items and pulse length `l`.
pub fn variant(text: &str, n: u32, l: i64) -> Result<String> {
    let mut out: Vec<String> = Vec::new();
    let mut counts = 0;
    for line in text.lines() {
        let t = line.trim_start();
        if let Some(name) = t.strip_prefix("name:") {
            let name = name.trim();
            let stem = match name.rfind("-L") {
                Some(i) if name[i + 2..].chars().all(|c| c.is_ascii_digit()) && i + 2 < name.len() => &name[..i],
                _ => name,
            };
            out.push(format!("name: {stem}-L{l}"));
        } else if t.starts_with("nbt:") {
            let k = line.matches("count:").count();
            counts += k;
            if k != 1 {
                bail!("the nbt line holds {k} `count:` (want exactly one): {line}");
            }
            let i = line.find("count:").unwrap() + "count:".len();
            let digits = line[i..].chars().take_while(|c| c.is_ascii_digit()).count();
            out.push(format!("{}{n}{}", &line[..i], &line[i + digits..]));
        } else if t.starts_with('#') && t.contains("This one holds") {
            let prefix = &line[..line.find("This one holds").unwrap()];
            out.push(format!("{prefix}This one holds {n} item{}: {l} ticks.", if n == 1 { "" } else { "s" }));
        } else {
            out.push(line.to_string());
        }
    }
    if counts != 1 {
        bail!("want exactly one `count:` in the design's nbt lines, found {counts}");
    }
    Ok(out.join("\n") + "\n")
}

pub fn run(a: ItemsArgs) -> Result<()> {
    let text = std::fs::read_to_string(&a.design)?;
    let dir = a.out_dir.clone().unwrap_or_else(|| a.design.parent().map(|p| p.to_path_buf()).unwrap_or_default());
    std::fs::create_dir_all(&dir)?;
    for &n in &a.counts {
        let l = a.per * n as i64 + a.add;
        let v = variant(&text, n, l)?;
        let name = v.lines().find_map(|s| s.trim_start().strip_prefix("name:")).map(|s| s.trim().to_string()).unwrap_or_default();
        let path = dir.join(format!("{name}.txt"));
        std::fs::write(&path, v)?;
        println!("{}: {n} item{}, L {l} gt", path.display(), if n == 1 { "" } else { "s" });
    }
    Ok(())
}
