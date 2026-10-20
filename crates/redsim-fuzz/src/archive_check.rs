//! `redsim-fuzz archive-check`: every archived clock and circuit run on redsim (`clock sim`,
//! `circuit sim`) and its line compared with the real-game judge's line in the design's header
//! (`# CLOCK: ...` / `# CIRCUIT: ...`), the last field aside (the judge's robustness, redsim's
//! start or `redsim`). One line per design (OK, DIFF with both lines, or what redsim says), then
//! the totals and the problems grouped by reason.

use anyhow::Result;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn strip_last(s: &str) -> &str {
    s.rsplit_once(", ").map(|(a, _)| a).unwrap_or(s)
}

/// A short reason for grouping (`block "x"`, `item x`, `entity x`...).
fn reason(why: &str) -> String {
    for (pat, label) in [("block \"", "block "), ("item minecraft:", "item "), ("entity minecraft:", "entity "), ("entity ", "entity ")] {
        if let Some(i) = why.find(pat) {
            let rest = &why[i + pat.len()..];
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !name.is_empty() {
                return format!("{label}{name}");
            }
        }
    }
    why.chars().take(90).collect()
}

enum Outcome {
    Same,
    Diff { judge: String, redsim: String },
    /// redsim's verdict word (UNMODELED, FAIL, STOPS...) and why.
    Not { word: String, why: String },
    /// No judge line in the header.
    NoJudge,
}

fn check_clock(path: &Path, judge: &str) -> Result<Outcome> {
    let c = clock::Clock::load(path)?;
    let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
    if r.verdict != clock::sim::Verdict::Clock {
        return Ok(Outcome::Not { word: r.verdict.word().to_string(), why: r.why.clone() });
    }
    let ours = r.summary();
    Ok(if strip_last(&ours) == strip_last(judge) { Outcome::Same } else { Outcome::Diff { judge: judge.to_string(), redsim: ours } })
}

fn check_circuit(path: &Path, judge: &str) -> Result<Outcome> {
    let c = circuit::Circuit::load(path)?;
    let r = circuit::verify::sim(&c, &circuit::verify::Options::default())?;
    let ours = r.summary(&c);
    if !ours.starts_with("CIRCUIT:") {
        let (word, why) = ours.split_once(": ").unwrap_or((&ours, ""));
        return Ok(Outcome::Not { word: word.to_string(), why: why.to_string() });
    }
    Ok(if strip_last(&ours) == strip_last(judge) { Outcome::Same } else { Outcome::Diff { judge: judge.to_string(), redsim: ours } })
}

pub fn run(paths: &[PathBuf], problems_only: bool) -> Result<()> {
    let (mut same, mut diff, mut total) = (0, 0, 0);
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in paths {
        let text = std::fs::read_to_string(p)?;
        let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let clock_line = text.lines().find_map(|l| l.strip_prefix("# CLOCK: ").map(|s| format!("CLOCK: {s}")));
        let circuit_line = text.lines().find_map(|l| l.strip_prefix("# CIRCUIT: ").map(|s| format!("CIRCUIT: {s}")));
        total += 1;
        let out = match (clock_line, circuit_line) {
            (Some(j), _) => check_clock(p, &j).unwrap_or_else(|e| Outcome::Not { word: "ERROR".into(), why: e.to_string() }),
            (_, Some(j)) => check_circuit(p, &j).unwrap_or_else(|e| Outcome::Not { word: "ERROR".into(), why: e.to_string() }),
            _ => Outcome::NoJudge,
        };
        match out {
            Outcome::Same => {
                same += 1;
                if !problems_only {
                    println!("OK {name}");
                }
            }
            Outcome::Diff { judge, redsim } => {
                diff += 1;
                println!("DIFF {name}\n  judge:  {judge}\n  redsim: {redsim}");
                groups.entry("DIFF (redsim's line differs from the judge's)".into()).or_default().push(name);
            }
            Outcome::Not { word, why } => {
                println!("{word} {name}: {why}");
                groups.entry(format!("{word} {}", reason(&why))).or_default().push(name);
            }
            Outcome::NoJudge => {
                println!("NO-JUDGE {name}: no `# CLOCK:` / `# CIRCUIT:` line");
                groups.entry("NO-JUDGE".into()).or_default().push(name);
            }
        }
    }
    println!("\n{same} of {total} designs: redsim prints the judge's line ({diff} differ, {} not run to a line)", total - same - diff);
    for (g, names) in &groups {
        println!("  {:>4}  {g}: {}", names.len(), names.join(" "));
    }
    Ok(())
}
