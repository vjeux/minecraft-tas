//! Contraption study 5, track memory (docs/circuits.md): `mctas circuit memory <args...>`. OWNED BY
//! THE TRACK (the tff explorer runs both).
//!
//! * [`specs`]: census specs for rs-latch, d-latch and counter N mirroring the judge's stimuli, run
//!   on track tff's census engine (`circuit_tff::census`).

pub mod specs;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit memory", about = "Memory circuits (track memory): census for RS / D latches and counters")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Census: every filling of k cells of a box (x 0..A, y 0..B, z 0..C) with palette states,
    /// the named inputs redstone blocks in air cells (`--inputs 'r=-1,0,0;s=3,0,0'`), judged on
    /// redsim with the kind's stimulus (`rs-latch`, `d-latch`, `counter N`); designs + summary.tsv.
    Census {
        /// `rs-latch`, `d-latch` or `counter N`.
        #[arg(long)]
        kind: String,
        #[arg(long = "box")]
        dims: String,
        /// `name=x,y,z;...` in the kind's input names (r, s / d, e / one input for counters).
        #[arg(long)]
        inputs: String,
        #[arg(long, default_value = "1..3")]
        k: String,
        #[arg(long, default_value = "stone,dust,torch,repeater1,comparator")]
        palette: String,
        #[arg(long, default_value_t = 40)]
        gap: u32,
        #[arg(long, default_value_t = 160)]
        jobs: usize,
        #[arg(long)]
        out: PathBuf,
    },
}

/// `mctas circuit memory <args...>`.
pub fn run(args: Vec<String>) -> Result<()> {
    let a = match Args::try_parse_from(std::iter::once("mctas circuit memory".to_string()).chain(args)) {
        Ok(a) => a,
        Err(e) if matches!(e.kind(), clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion) => {
            print!("{e}");
            return Ok(());
        }
        Err(e) => bail!("{e}"),
    };
    match a.cmd {
        Cmd::Census { kind, dims, inputs, k, palette, gap, jobs, out } => {
            let d: Vec<i32> = dims.split('x').map(|t| t.parse::<i32>()).collect::<Result<_, _>>().context("--box AxBxC")?;
            if d.len() != 3 {
                bail!("--box AxBxC");
            }
            let mut free = Vec::new();
            for y in 0..d[1] {
                for z in 0..d[2] {
                    for x in 0..d[0] {
                        free.push(redsim::BlockPos::new(x, y, z));
                    }
                }
            }
            let mut named: Vec<(String, redsim::BlockPos)> = Vec::new();
            for part in inputs.split(';').filter(|s| !s.trim().is_empty()) {
                let (n, p) = part.split_once('=').ok_or_else(|| anyhow!("--inputs name=x,y,z;..."))?;
                let v: Vec<i32> = p.split(',').map(|t| t.trim().parse::<i32>()).collect::<Result<_, _>>().context("--inputs")?;
                named.push((n.trim().to_string(), redsim::BlockPos::new(v[0], v[1], v[2])));
            }
            // the judge orders inputs by name
            named.sort_by(|a, b| a.0.cmp(&b.0));
            let words: Vec<&str> = kind.split_whitespace().collect();
            let spec: Box<dyn circuit_tff::census::Spec> = match words.as_slice() {
                ["rs-latch"] => Box::new(specs::RsSpec { gap }),
                ["d-latch"] => Box::new(specs::DSpec { gap }),
                ["counter", n] => Box::new(specs::CounterSpec { n: n.parse().context("counter N")?, gap }),
                _ => bail!("--kind rs-latch | d-latch | counter N"),
            };
            let want_inputs = match words[0] {
                "rs-latch" => vec!["r", "s"],
                "d-latch" => vec!["d", "e"],
                _ => vec![""],
            };
            let names: Vec<&str> = named.iter().map(|(n, _)| n.as_str()).collect();
            if names != want_inputs && !(want_inputs == vec![""] && names.len() == 1) {
                bail!("--inputs for {kind}: {want_inputs:?}, got {names:?}");
            }
            let (pal, pal_be) = circuit_tff::cli::parse_palette(&palette)?;
            let (k_min, k_max) = circuit_tff::cli::parse_k(&k)?;
            let free: Vec<redsim::BlockPos> = free.into_iter().filter(|p| !named.iter().any(|(_, q)| q == p)).collect();
            eprintln!("{kind}: {} free cells, {} palette states, k {k_min}..={k_max}", free.len(), pal.len());
            let c = circuit_tff::census::Census {
                inputs: named.iter().map(|(_, p)| *p).collect(),
                fixed: Vec::new(),
                fixed_be: Vec::new(),
                free,
                palette: pal,
                palette_be: pal_be,
                k_min,
                k_max,
                outputs: None,
                jobs,
                margin: 3,
            };
            let found = c.run(spec.as_ref(), true);
            std::fs::create_dir_all(&out)?;
            let mut rows: Vec<(String, usize, u32, String)> = Vec::new();
            let short = words.join("");
            for f in &found {
                let fam = circuit_tff::cli::family_of(&f.blocks);
                let h = circuit_tff::cli::fnv(&f.key);
                let name = format!("{short}-{}-{}b-{}", if fam.is_empty() { "none".to_string() } else { fam.replace('+', "-") }, f.blocks.len(), &h[..8]);
                let named_in: Vec<(String, redsim::BlockPos)> = named.iter().map(|(n, p)| (if want_inputs == vec![""] { String::new() } else { n.clone() }, *p)).collect();
                let des = circuit_tff::cli::found_design(&kind, &name, f, &named_in);
                des.save(&out.join(format!("{name}.txt")))?;
                rows.push((fam, f.blocks.len(), f.score, name));
            }
            rows.sort();
            let mut tsv = String::from("family\tblocks\tscore\tdesign\n");
            for (fam, b, s, n) in &rows {
                tsv.push_str(&format!("{fam}\t{b}\t{s}\t{n}\n"));
            }
            std::fs::write(out.join("summary.tsv"), &tsv)?;
            println!("{} {kind} types found (summary.tsv in {})", rows.len(), out.display());
            for (fam, b, s, n) in rows.iter().take(30) {
                println!("  {b} blocks, {fam}, score {s}: {n}");
            }
        }
    }
    Ok(())
}
