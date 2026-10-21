//! `mctas clock period-search` (track clk-rt, coordinator 2026-10-21 for vjeux's "Don't we have 3
//! tick clocks?"): `anneal`'s chains over clock designs on redsim toward a target period `P`
//! (default 3), from seed clocks, inside a box (default: the seed's box grown by `--grow` cells).
//! Score tiers: 0 not a clock on redsim (still, unmodeled, escapes, no repeat within the horizon);
//! 1 a clock of another period: for a multiple of P, `partial` = 1000 x the share of its changing
//! cells whose own sequence already repeats every P gt (all of them would make the period P),
//! minus the period; any other period ranks below every multiple of P; 2 a clock of period P on
//! redsim; 3 the same with an output (the judge's output rule, `SimReport.output`). Every tier-2
//! design goes to DIR/<seed>/found/ (`anneal`), the best to DIR/<seed>/best.txt.

use crate::anneal::{self, Chains, Space, St};
use crate::sim::{self, SimOptions, Verdict};
use crate::{Clock, Pos};
use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};

pub struct Options {
    pub period: u32,
    pub palette: Vec<String>,
    pub grow: i32,
    pub iterations: usize,
    pub workers: usize,
    pub rng: u64,
    pub stale: usize,
    pub repair: usize,
    /// Redsim horizon of the screen.
    pub horizon: u32,
}

/// The default palette: what a period-3 clock can be built from (clk-obs's necessary structure:
/// pistons 0-ticking every 3 gt and the elements they move) plus dust and obsidian.
pub fn default_palette() -> Vec<String> {
    let mut p = Vec::new();
    for id in ["piston", "sticky_piston"] {
        for f in ["north", "east", "south", "west", "up", "down"] {
            p.push(format!("minecraft:{id}[extended=false,facing={f}]"));
        }
    }
    for s in ["minecraft:redstone_block", "minecraft:slime_block", "minecraft:honey_block", "minecraft:stone", "minecraft:obsidian", "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"] {
        p.push(s.to_string());
    }
    p
}

/// The score of one design.
fn evaluate(seed: &Clock, st: &St, o: &Options, region: (Pos, Pos)) -> (anneal::Score, String) {
    let outside = -st.outside(region);
    // below the target period only the period counts (growing must not cost anything)
    let mk = |tier: u8, partial: i64| {
        let (nv, nb) = if tier >= 2 { (-anneal::volume(&st.b).0, -(st.b.len() as i64)) } else { (0, 0) };
        anneal::Score { tier, partial, outside, neg_volume: nv, neg_blocks: nb }
    };
    let Ok(c) = Clock::from_design(st.design(&seed.design)) else { return (mk(0, 0), "not a design".into()) };
    let so = SimOptions { horizon: o.horizon, margin: 3 };
    let r = sim::simulate(&c, &so);
    if r.verdict != Verdict::Clock {
        return (mk(0, 0), format!("{:?}: {}", r.verdict, r.why.chars().take(80).collect::<String>()));
    }
    let p = r.period;
    if p == o.period {
        let out = r.output.is_some();
        return (mk(if out { 3 } else { 2 }, 0), format!("PERIOD {p}{}: {}", if out { " with an output" } else { " (no output)" }, r.summary()));
    }
    if p % o.period != 0 {
        return (mk(1, -100_000 - p as i64), format!("period {p}"));
    }
    // the share of changing cells already repeating every `o.period` gt
    let w = r.state_warmup.max(r.warmup);
    let Some(frames) = sim::run_states(&c, &so, w + p) else { return (mk(1, -(p as i64)), format!("period {p}")) };
    let mut seqs: BTreeMap<redsim::BlockPos, Vec<String>> = BTreeMap::new();
    for (i, f) in frames.iter().enumerate().skip(w as usize).take(p as usize) {
        let here: HashMap<redsim::BlockPos, &String> = f.iter().map(|(q, s)| (*q, s)).collect();
        for q in here.keys() {
            seqs.entry(*q).or_insert_with(|| vec!["air".to_string(); i - w as usize]);
        }
        for (q, v) in seqs.iter_mut() {
            v.push(here.get(q).map(|s| s.to_string()).unwrap_or_else(|| "air".into()));
        }
    }
    // the drivers only (pistons, redstone blocks, moving blocks): followers (dust, slime riding
    // along) repeat every P gt too easily
    let changing: Vec<&Vec<String>> = seqs.values().filter(|v| v.len() == p as usize && v.iter().any(|s| s != &v[0]) && v.iter().any(|s| s.contains("piston") || s.contains("redstone_block"))).collect();
    let k = o.period as usize;
    let good = changing.iter().filter(|v| (0..v.len()).all(|i| v[i] == v[(i + k) % v.len()])).count();
    let share = if changing.is_empty() { 0 } else { 1000 * good / changing.len() };
    (mk(1, share as i64 - p as i64), format!("period {p}, {good}/{} changing cells repeat every {}", changing.len(), o.period))
}

/// One seed's run: the best score and its verdict.
pub fn search(seed_path: &Path, o: &Options, out: &Path) -> Result<(anneal::Score, String, usize)> {
    let seed = Clock::load(seed_path)?;
    let (lo, hi) = seed.design.bounds().ok_or_else(|| anyhow::anyhow!("empty seed"))?;
    let g = o.grow;
    let region = ([lo[0] - g, lo[1] - g, lo[2] - g], [hi[0] + g, hi[1] + g, hi[2] + g]);
    let pal = if o.palette.is_empty() { default_palette() } else { o.palette.clone() };
    let frozen: BTreeSet<Pos> = BTreeSet::new();
    let st0 = St::of(&seed.design);
    let (s0, v0) = evaluate(&seed, &st0, o, region);
    eprintln!("{}: seed tier {} partial {}: {v0}", seed_path.display(), s0.tier, s0.partial);
    let keep = |_: &BTreeMap<Pos, String>, _: Pos| false;
    let chains = Chains {
        seed: &seed.design,
        space: Space { region, frozen: &frozen },
        palette: &pal,
        iterations: o.iterations,
        workers: o.workers,
        rng: o.rng,
        stale: o.stale,
        repair: o.repair,
        pass_tier: 2,
        keep: &keep,
        out,
        verbose: true,
    };
    let seed_r = &seed;
    let make_eval = || -> anneal::Eval<'_> {
        let mut cache: HashMap<St, (anneal::Score, String)> = HashMap::new();
        Box::new(move |cand: &St, _cur: anneal::Score, _walk: bool| {
            if let Some(v) = cache.get(cand) {
                return v.clone();
            }
            let v = evaluate(seed_r, cand, o, region);
            if cache.len() > 100_000 {
                cache.clear();
            }
            cache.insert(cand.clone(), v.clone());
            v
        })
    };
    let (found, best) = anneal::run(&chains, &st0, s0, &v0, make_eval);
    let front = anneal::front(&found, 3);
    if !front.is_empty() {
        anneal::write_front(&seed.design, &front, out)?;
    }
    let (_, bv) = evaluate(&seed, &best.1, o, region);
    Ok((best.0, bv, found.len()))
}

/// Every seed in turn: DIR/<seed stem>/ each, one summary line per seed.
pub fn search_all(seeds: &[PathBuf], o: &Options, out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    let mut tsv = String::from("seed\tbest tier\tpartial\tfinds\tverdict\n");
    for s in seeds {
        let stem = s.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
        let dir = out.join(&stem);
        match search(s, o, &dir) {
            Ok((sc, v, n)) => {
                println!("{stem}: best tier {} partial {}, {n} period-{} designs: {v}", sc.tier, sc.partial, o.period);
                tsv += &format!("{stem}\t{}\t{}\t{n}\t{v}\n", sc.tier, sc.partial);
            }
            Err(e) => {
                println!("{stem}: {e:#}");
                tsv += &format!("{stem}\t\t\t\t{e:#}\n");
            }
        }
        std::fs::write(out.join("summary.tsv"), &tsv)?;
    }
    Ok(())
}
