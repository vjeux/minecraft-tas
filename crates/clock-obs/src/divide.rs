//! `mctas clock obs divide`: copper-bulb dividers after a clock (clk-obs, 2026-10-21, the
//! usable-output work): the period x 2^K with a tap a builder can use. The clock's output cell
//! gets a copper bulb (the signal that would light the judge's lamp there toggles it on every
//! rising edge); a comparator reads the bulb from the next cell outward (a comparator reading a
//! block of the build is a usable tap) and powers the next bulb, and so on; the last comparator's
//! front cell is the new output. Each stage: a bulb, a comparator and the stone under it when that
//! cell is air (+2 or +3 blocks). The outward direction: the first horizontal direction whose
//! cells are free in the design for the whole chain.
//!
//! `--search --periods P,..`: for each period, every archived shown clock with a usable output
//! and ONE rising edge per period at P / 2^K (K = 1..4), smallest first, divided K times and run
//! on redsim; the passing designs (the period exact, one evenly spaced edge) go in `--out-dir`,
//! with a table: period, blocks, volume, base.

use anyhow::{Result, anyhow, bail};
use clap::Args as ClapArgs;
use clock::Clock;
use clock::sim::{SimOptions, Verdict, simulate};
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct DivideArgs {
    pub design: Option<PathBuf>,
    #[arg(long, default_value_t = 1)]
    pub stages: u32,
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Force the outward direction: north, south, west, east.
    #[arg(long)]
    pub dir: Option<String>,
    /// Search the archive for divided clocks at these periods.
    #[arg(long)]
    pub search: bool,
    #[arg(long, default_value = "")]
    pub periods: String,
    #[arg(long, default_value = "designs/clocks/archive")]
    pub archive: PathBuf,
    #[arg(long, default_value = "/tmp/clock-divide")]
    pub out_dir: PathBuf,
    /// Bases tried per (period, stages), smallest first.
    #[arg(long, default_value_t = 12)]
    pub tries: usize,
}

type Pos = [i32; 3];

fn add(a: Pos, b: Pos, k: i32) -> Pos {
    [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]]
}

/// The divided design's text (and its block count), or why it can't be built.
pub fn build(c: &Clock, stages: u32, dir: Option<&str>) -> Result<(String, usize)> {
    let o = c.output.ok_or_else(|| anyhow!("no `port output`"))?;
    let d = &c.design;
    let dirs: [(&str, Pos, &str); 4] = [("north", [0, 0, -1], "south"), ("south", [0, 0, 1], "north"), ("west", [-1, 0, 0], "east"), ("east", [1, 0, 0], "west")];
    let k = stages.max(1) as i32;
    let fits = |v: Pos| (0..=2 * k).all(|i| !d.blocks.contains_key(&add(o, v, i)));
    let pick = match dir {
        Some(name) => dirs.iter().find(|x| x.0 == name).copied().ok_or_else(|| anyhow!("--dir: north, south, west or east"))?,
        None => *dirs.iter().find(|x| fits(x.1)).ok_or_else(|| anyhow!("no horizontal direction from the output {o:?} is free for {k} stage(s)"))?,
    };
    let (_, v, back) = pick;
    if !fits(v) {
        bail!("the cells from {o:?} toward {} hold design blocks", pick.0);
    }
    let mut nd = d.clone();
    for i in 0..k {
        let comp = add(o, v, 2 * i + 1);
        nd.blocks.insert(add(o, v, 2 * i), "minecraft:waxed_copper_bulb[lit=false,powered=false]".into());
        nd.blocks.insert(comp, format!("minecraft:comparator[facing={back},mode=compare,powered=false]"));
        let under = add(comp, [0, -1, 0], 1);
        if !nd.blocks.contains_key(&under) {
            nd.blocks.insert(under, "minecraft:stone".into());
        }
    }
    let port = add(o, v, 2 * k);
    nd.ports.insert("output".into(), port);
    let name = nd.meta.get("name").cloned().unwrap_or_else(|| "clock".into());
    nd.meta.insert("name".into(), format!("{name}-div{}", 1u32 << k));
    let mut text = format!(
        "# {name} with {k} copper-bulb divider stage(s) (clk-obs `clock obs divide`): the bulb in the old output cell toggles on every pulse, a comparator reads it (x 2 per stage); the period is the base's x {}.\n",
        1u32 << k
    );
    text.push_str(&nd.to_text());
    Ok((text, nd.blocks.len()))
}

pub fn run(a: DivideArgs) -> Result<()> {
    if a.search {
        return search(&a);
    }
    let design = a.design.as_ref().ok_or_else(|| anyhow!("divide: a design (or --search)"))?;
    let out = a.out.as_ref().ok_or_else(|| anyhow!("divide: --out"))?;
    let c = Clock::load(design)?;
    let (text, blocks) = build(&c, a.stages, a.dir.as_deref())?;
    std::fs::write(out, text)?;
    eprintln!("{}: {} stage(s), {blocks} blocks", out.display(), a.stages);
    Ok(())
}

fn search(a: &DivideArgs) -> Result<()> {
    let rows = clock::catalog::read_rows(&a.archive)?;
    let periods: Vec<u32> = a.periods.split(',').filter(|s| !s.trim().is_empty()).map(|s| s.trim().parse::<u32>()).collect::<std::result::Result<_, _>>()?;
    std::fs::create_dir_all(&a.out_dir)?;
    for p in periods {
        let mut best: Vec<(usize, u32, String, String)> = Vec::new();
        for k in 1..=4u32 {
            let base_p = p >> k;
            if base_p << k != p || base_p < 2 {
                continue;
            }
            let mut bases: Vec<&clock::catalog::Row> = rows.iter().filter(|r| r.period == base_p && r.hidden.is_empty() && r.usable.starts_with("yes") && r.edges == 1).collect();
            bases.sort_by_key(|r| (r.blocks, r.volume));
            for r in bases.into_iter().take(a.tries) {
                let Ok(c) = Clock::load(&a.archive.join(&r.file)) else { continue };
                let Ok((text, blocks)) = build(&c, k, None) else { continue };
                let path = a.out_dir.join(format!("p{p}-{}-div{}.txt", r.file.trim_end_matches(".txt"), 1u32 << k));
                std::fs::write(&path, &text)?;
                let Ok(nc) = Clock::load(&path) else { continue };
                let s = simulate(&nc, &SimOptions::default());
                let even = s.output.as_ref().is_some_and(|o| o.wave.edges == 1);
                if s.verdict == Verdict::Clock && s.period == p && even {
                    let (lo, hi) = s.swept;
                    let vol = ((hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1)) as u32;
                    best.push((blocks, vol, r.file.clone(), path.display().to_string()));
                    break;
                }
                let _ = std::fs::remove_file(&path);
            }
        }
        best.sort();
        match best.first() {
            Some((b, v, base, path)) => println!("P {p}: {b} blocks, volume {v}, base {base} -> {path}"),
            None => println!("P {p}: no divided clock"),
        }
    }
    Ok(())
}
