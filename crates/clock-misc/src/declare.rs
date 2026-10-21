//! `mctas clock misc declare-output FILES... [--write]`: archived clocks with no `port output`
//! get one at the cell the judge measures. With no port, `clock verify` takes redsim's automatic
//! output cell and carries it into the game as a declared port (it reads a component's own state,
//! any other block by the power it receives), while `clock sim` reads its auto-picked cell by the
//! cell's own state changes: their `on` / `edges` can differ. Declaring the port makes both read
//! the same cell the same way. Each candidate is judged again in the game (`clock verify`, all
//! positions and rotations); with `--write`, a design whose judge line is unchanged (period
//! seconds aside) is rewritten in place under its archive name, a comment and the port line added,
//! its verdict row's content hash updated (every other column, the caption and the type key stay);
//! a design whose line changes is reported and left alone. Run from the repository root.

use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use clock::sim::{SimOptions, Verdict, simulate};
use clock::verify::{VerifyOptions, spawn_pool, verify_in_pool};
use std::path::{Path, PathBuf};

#[derive(ClapArgs, Debug, Clone)]
pub struct DeclareArgs {
    /// Archived designs (designs/clocks/archive/<name>.txt).
    pub files: Vec<PathBuf>,
    /// Rewrite the designs whose judge line is unchanged (default: report only).
    #[arg(long)]
    pub write: bool,
    /// The judge's game workers (capped by MCTAS_MAX_WORKERS).
    #[arg(long, default_value_t = 8)]
    pub workers: usize,
}

const DIR: &str = "designs/clocks/archive";

/// The judge's line with the period's seconds left out (older archived lines print
/// `period 90 gt,`, newer ones `period 90 gt (4.5 s),`).
pub fn without_seconds(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(" gt (") {
        let after = &rest[i + 5..];
        match after.find(')') {
            Some(j) if after[..j].ends_with(" s") => {
                out.push_str(&rest[..i + 3]);
                rest = &after[j + 1..];
            }
            _ => {
                out.push_str(&rest[..i + 5]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The line without its last field (the judge's robustness, redsim's start).
fn without_last(s: &str) -> String {
    let s = without_seconds(s);
    s.rsplit_once(", ").map(|(a, _)| a.to_string()).unwrap_or(s)
}

/// The design text with `port output: x y z` (and a comment) after its last `port` line, or after
/// its `name:` line.
fn with_port(text: &str, pos: [i32; 3]) -> Result<String> {
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .rposition(|l| l.starts_with("port "))
        .or_else(|| lines.iter().position(|l| l.starts_with("name:")))
        .context("no `name:` line")?;
    let mut out = String::with_capacity(text.len() + 200);
    for (i, l) in lines.iter().enumerate() {
        out.push_str(l);
        out.push('\n');
        if i == at {
            out.push_str("# port output declared 2026-10-20 (clk-misc): the cell the judge measures as this clock's automatic output, so\n");
            out.push_str("# clock sim reads it the same way (re-judged in the game: its CLOCK line is unchanged)\n");
            out.push_str(&format!("port output: {} {} {}\n", pos[0], pos[1], pos[2]));
        }
    }
    Ok(out)
}

/// Replace column 0 (the content hash) of the verdict row whose column 1 is `file`.
fn set_row_hash(tsv: &Path, file: &str, hash: &str) -> Result<()> {
    let text = std::fs::read_to_string(tsv).with_context(|| format!("read {}", tsv.display()))?;
    let mut out = String::with_capacity(text.len());
    let mut found = false;
    for line in text.lines() {
        let mut cols: Vec<&str> = line.split('\t').collect();
        if cols.len() > 1 && cols[1] == file {
            found = true;
            cols[0] = hash;
            out.push_str(&cols.join("\t"));
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    if !found {
        bail!("{file}: no row in {}", tsv.display());
    }
    std::fs::write(tsv, out)?;
    Ok(())
}

enum Outcome {
    Kept { pos: [i32; 3], line: String, sim_same: bool },
    Changed { pos: [i32; 3], archived: String, now: String },
    Skip(String),
}

fn one(pool: &mctas::sim::Pool, o: &VerifyOptions, path: &Path, write: bool) -> Result<Outcome> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let Some(archived) = text.lines().find_map(|l| l.strip_prefix("# CLOCK: ")).map(|s| format!("CLOCK: {s}")) else {
        return Ok(Outcome::Skip("no `# CLOCK:` judge line".into()));
    };
    let c = clock::Clock::load(path)?;
    if c.output.is_some() {
        return Ok(Outcome::Skip("already has a port output".into()));
    }
    let r = simulate(&c, &SimOptions::default());
    let Some(pos) = r.output.as_ref().filter(|_| r.verdict == Verdict::Clock).map(|o| o.pos) else {
        return Ok(Outcome::Skip(format!("clock sim: {}", r.summary())));
    };
    let new_text = with_port(&text, pos)?;
    let nc = clock::Clock::from_design(mctas::design::Design::parse(&new_text)?)?;
    let sim_line = simulate(&nc, &SimOptions::default()).summary();
    let rep = verify_in_pool(pool, &nc, o)?;
    let now = rep.summary();
    if without_seconds(&now) != without_seconds(&archived) {
        return Ok(Outcome::Changed { pos, archived, now });
    }
    if write {
        let file = path.file_name().context("file name")?.to_string_lossy().to_string();
        let dir = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new(DIR));
        std::fs::write(path, &new_text)?;
        set_row_hash(&dir.join(clock::catalog::VERDICTS), &file, &clock::catalog::content_hash(&nc))?;
    }
    Ok(Outcome::Kept { pos, line: now, sim_same: without_last(&sim_line) == without_last(&archived) })
}

pub fn run(a: DeclareArgs) -> Result<()> {
    if a.files.is_empty() {
        bail!("clock misc declare-output FILES... [--write]");
    }
    let o = VerifyOptions { workers: a.workers, ..Default::default() };
    let pool = spawn_pool(&o)?;
    let (mut kept, mut changed, mut skipped, mut sim_still) = (0, 0, 0, 0);
    for p in &a.files {
        let name = p.file_stem().unwrap_or_default().to_string_lossy().to_string();
        match one(&pool, &o, p, a.write)? {
            Outcome::Kept { pos, line, sim_same } => {
                kept += 1;
                if !sim_same {
                    sim_still += 1;
                }
                let verb = if a.write { "KEPT" } else { "SAME" };
                let note = if sim_same { "" } else { " (clock sim still differs)" };
                println!("{verb} {name}: port output {} {} {}{note}; {line}", pos[0], pos[1], pos[2]);
            }
            Outcome::Changed { pos, archived, now } => {
                changed += 1;
                println!("CHANGED {name}: with port output {} {} {}\n  archived: {archived}\n  now:      {now}", pos[0], pos[1], pos[2]);
            }
            Outcome::Skip(why) => {
                skipped += 1;
                println!("SKIP {name}: {why}");
            }
        }
    }
    let verb = if a.write { "rewritten" } else { "would be rewritten (--write)" };
    println!("{kept} {verb} with their judge line unchanged ({sim_still} where clock sim still differs), {changed} whose line changes (left alone), {skipped} skipped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_dropped() {
        assert_eq!(without_seconds("CLOCK: a period 90 gt (4.5 s), on 1/90"), "CLOCK: a period 90 gt, on 1/90");
        assert_eq!(without_seconds("CLOCK: a period 2380 gt (1 min 59 s), on 14/2380"), "CLOCK: a period 2380 gt, on 14/2380");
        assert_eq!(without_seconds("CLOCK: a period 90 gt, on 1/90"), "CLOCK: a period 90 gt, on 1/90");
    }

    #[test]
    fn port_inserted_after_ports() {
        let t = "# x\nname: a\nport start: 0 0 0 last\npalette:\n  S minecraft:stone\n";
        let n = with_port(t, [1, 2, 3]).unwrap();
        assert!(n.contains("port start: 0 0 0 last\n# port output declared"));
        assert!(n.contains("port output: 1 2 3\npalette:"));
    }
}
