//! `mctas circuit timer as <kind> <designs>`: designs of any study judged as another kind on
//! redsim, their `circuit:` line replaced (and `name:` prefixed when written out).

use anyhow::{Context, Result};
use circuit::Circuit;
use circuit::verify::Options;
use std::path::Path;

/// The design's text with its `circuit:` line set to `kind` (added when missing) and its name
/// prefixed.
pub fn reheaded(text: &str, kind: &str, prefix: &str) -> String {
    let mut out = String::new();
    let mut seen = false;
    for l in text.lines() {
        if l.starts_with("circuit:") {
            if !seen {
                out.push_str(&format!("circuit: {kind}\n"));
                seen = true;
            }
            continue;
        }
        // the old kind's own headers (claims, axis words) stay: the judge ignores what it does not read
        if let Some(n) = l.strip_prefix("name:") {
            out.push_str(&format!("name: {prefix}{}\n", n.trim()));
            continue;
        }
        out.push_str(l);
        out.push('\n');
    }
    if !seen {
        out = format!("circuit: {kind}\n{out}");
    }
    out
}

pub fn run(kind: &str, designs: &[std::path::PathBuf], out: Option<&Path>, prefix: &str, quiet: bool, margin: i32, version: &str) -> Result<()> {
    let o = Options { margin, version: version.to_string(), ..Default::default() };
    if let Some(d) = out {
        std::fs::create_dir_all(d)?;
    }
    let (mut pass, mut n) = (0, 0);
    for p in designs {
        n += 1;
        let text = std::fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
        let new = reheaded(&text, kind, prefix);
        let d = match mctas::design::Design::parse(&new) {
            Ok(d) => d,
            Err(e) => {
                if !quiet {
                    println!("FAIL: {}: {e:#}", p.display());
                }
                continue;
            }
        };
        let c = match Circuit::from_design(d) {
            Ok(c) => c,
            Err(e) => {
                if !quiet {
                    println!("FAIL: {}: {e:#}", p.display());
                }
                continue;
            }
        };
        let r = circuit::verify::sim(&c, &o)?;
        let line = r.summary(&c);
        let ok = r.outcome.as_ref().is_some_and(|x| x.pass);
        if ok {
            pass += 1;
            if let Some(d) = out {
                let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("design");
                std::fs::write(d.join(format!("{prefix}{stem}.txt")), &new)?;
            }
        }
        if ok || !quiet {
            println!("{line}  [{}]", p.display());
        }
    }
    println!("{pass} of {n} pass as {kind}");
    Ok(())
}
