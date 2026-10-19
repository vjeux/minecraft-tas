//! `mctas clock sculk caption`: turn a generated design's header into a one-line caption (the
//! first comment block is what `clock archive` keeps as the caption): the `# CLOCK:` line goes
//! (the judge writes its own), the `# built by` line becomes plain words. Census designs get
//! "found by clock census" + their family.

use anyhow::Result;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct CaptionArgs {
    files: Vec<PathBuf>,
    /// Directories holding the original generated files (same file names): their `built by` line
    /// is used when a file's own header is already gone.
    #[arg(long, num_args = 1..)]
    how_from: Vec<PathBuf>,
}

fn sensor_word(s: &str) -> &'static str {
    if s == "plain" { "plain sculk sensor" } else { "calibrated sculk sensor" }
}

fn emitter_word(e: &str) -> String {
    match e.split('-').next().unwrap_or(e) {
        "note" => "a note block".into(),
        "trapdoor" => "an oak trapdoor (placed open+powered: it closes on placement, the self-start)".into(),
        "irontrapdoor" => "an iron trapdoor".into(),
        "gate" => "an oak fence gate".into(),
        "piston" => "a piston".into(),
        "sticky" => "a sticky piston".into(),
        "pistonext" => "a piston placed extended (retracts on placement)".into(),
        "dispenser" => "an empty dispenser (fires 4 gt after a rising edge)".into(),
        other => other.to_string(),
    }
}

/// Words for one `built by` line.
pub fn words(how: &str) -> String {
    let w: Vec<&str> = how.split_whitespace().collect();
    let travel = how.rsplit_once("(travel ").and_then(|(_, t)| t.split(')').next()).unwrap_or("?");
    let extra = how.split_once(" + ").map(|(_, e)| {
        let block = e.split('[').next().unwrap_or(e).replace('_', " ");
        format!("; plus a {block} beside the chain (moves parts of it: the hybrid)")
    });
    let mut s = match w.first().copied() {
        Some("obs") => {
            let k = w.iter().find_map(|t| t.strip_prefix("k=")).unwrap_or("?");
            let emitter = w.iter().position(|t| *t == "emitter").and_then(|i| w.get(i + 1)).copied().unwrap_or("?");
            let stone = how.contains("stone at the output");
            format!(
                "A {} watched by a chain of {k} observer(s) (each watching the previous); {}{}, {travel} from the sensor (travel time): the observers' pulse after the sensor re-arms is heard",
                sensor_word(w.get(1).copied().unwrap_or("")),
                if stone { "the last observer powers a stone and " } else { "" },
                if stone { format!("{} beside the stone", emitter_word(emitter)) } else { format!("{} at the last output", emitter_word(emitter)) }
            )
        }
        Some("chain") => {
            let reps = how.split_once("reps [").and_then(|(_, r)| r.split(']').next()).unwrap_or("");
            let below = w.get(2) == Some(&"below");
            let parts: Vec<&str> = how.split(", ").collect();
            let term = parts[1..].join(", ");
            let term = term.split(" (travel").next().unwrap_or(&term).to_string();
            let sensor = sensor_word(w.get(1).copied().unwrap_or(""));
            if reps.is_empty() {
                format!("A {sensor}, then {term}, {travel} from the sensor (travel time)")
            } else {
                format!(
                    "A {sensor} {} repeater(s) of delay [{reps}]{}, then {term}, {travel} from the sensor (travel time)",
                    if below { "strongly powers the stone under it, read by" } else { "drives" },
                    if how.contains("placed powered") { " placed powered (their first turn-off is the self-start)" } else { "" }
                )
            }
        }
        Some("ring") => {
            let mut t = how.to_string();
            t.replace_range(0..1, "R");
            format!("{t}: each sensor's resonator (amethyst, the first a note block) rings when it fires and the next sensor hears it floor(distance) gt later; the sensor two hops ahead is still dead")
        }
        Some("climb") => format!("A sculk sensor + observer + sticky piston hybrid found by a long-period hunt ({how})"),
        _ => how.to_string(),
    };
    if let Some(e) = extra {
        s.push_str(&e);
    }
    s
}

pub fn caption(a: &CaptionArgs) -> Result<()> {
    for f in &a.files {
        let text = std::fs::read_to_string(f)?;
        // The generator line: this file's `# built by` / `# how:` line, or the original's.
        let own = text.lines().find_map(|l| l.strip_prefix("# built by `mctas clock sculk`: ").or_else(|| l.strip_prefix("# how: "))).map(str::to_string);
        let how = own.or_else(|| {
            let name = f.file_name()?;
            a.how_from.iter().find_map(|d| {
                let t = std::fs::read_to_string(d.join(name)).ok()?;
                t.lines().find_map(|l| l.strip_prefix("# built by `mctas clock sculk`: ").map(str::to_string))
            })
        });
        // Drop every leading comment line (old captions, the CLOCK line, the generator line).
        let body: Vec<&str> = text.lines().skip_while(|l| l.starts_with('#')).collect();
        let cap = match &how {
            Some(h) => Some(words(h)),
            None => text.lines().find_map(|l| l.strip_prefix("census: ")).map(|c| {
                let bx = c.split(" (").next().unwrap_or(c);
                let fam = f.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                format!("Found by `mctas clock census` ({bx}) with a sculk palette: {fam}")
            }),
        };
        let Some(cap) = cap else { continue };
        // The caption is the first comment block (what `clock archive` keeps); the generator line
        // goes after the name line.
        let mut final_text = format!("# {cap}\n");
        for l in &body {
            final_text.push_str(l);
            final_text.push('\n');
            if l.starts_with("name:") {
                if let Some(h) = &how {
                    final_text.push_str(&format!("# how: {h}\n"));
                }
            }
        }
        std::fs::write(f, final_text)?;
        println!("{}: {cap}", f.display());
    }
    Ok(())
}
