//! `mctas circuit timer tile <designs>`: the tightest `tile:` step each design passes on redsim
//! (along x, along z, pitch 1..`--max`): its copies side by side, each judged against its own input
//! (the judge's tiled run, crates/circuit/src/pulsemult.rs `check_tiled`). With `--out`, writes each
//! design with its tightest `tile:` header (the smaller pitch of the two axes) for the game.

use anyhow::Result;
use circuit::Circuit;
use circuit::verify::Options;
use std::path::Path;

/// Whether `text` passes on redsim with `tile: dx 0 dz`.
fn passes(text: &str, step: [i32; 3], o: &Options) -> bool {
    let Ok(mut d) = mctas::design::Design::parse(text) else { return false };
    d.meta.insert("tile".into(), format!("{} {} {}", step[0], step[1], step[2]));
    let Ok(c) = Circuit::from_design(d) else { return false };
    circuit::verify::sim(&c, o).ok().and_then(|r| r.outcome).is_some_and(|out| out.pass)
}

pub fn run(designs: &[std::path::PathBuf], max: i32, out: Option<&Path>, version: &str) -> Result<()> {
    let o = Options { version: version.to_string(), ..Default::default() };
    for p in designs {
        let text = std::fs::read_to_string(p)?;
        let name = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let best = |axis: usize| (1..=max).find(|&k| {
            let mut s = [0, 0, 0];
            s[axis] = k;
            passes(&text, s, &o)
        });
        let (bx, bz) = (best(0), best(2));
        println!("{name}: tile along x {}, along z {}", bx.map(|k| k.to_string()).unwrap_or("-".into()), bz.map(|k| k.to_string()).unwrap_or("-".into()));
        let pick = match (bx, bz) {
            (Some(x), Some(z)) if z < x => Some([0, 0, z]),
            (Some(x), _) => Some([x, 0, 0]),
            (None, Some(z)) => Some([0, 0, z]),
            (None, None) => None,
        };
        if let (Some(dir), Some(s)) = (out, pick) {
            std::fs::create_dir_all(dir)?;
            // the header right after the `circuit:` line
            let mut lines: Vec<String> = text.lines().filter(|l| !l.starts_with("tile:")).map(String::from).collect();
            let at = lines.iter().position(|l| l.starts_with("circuit:")).map(|i| i + 1).unwrap_or(1);
            lines.insert(at, format!("tile: {} {} {}", s[0], s[1], s[2]));
            let path = dir.join(format!("{name}.txt"));
            std::fs::write(&path, lines.join("\n") + "\n")?;
            println!("  -> {}", path.display());
        }
    }
    Ok(())
}
