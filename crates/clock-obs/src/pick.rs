//! Pick the smallest clocks per (period, family) from search outputs (census, grow folders):
//! every design is re-run with the official redsim judge (`clock::sim::simulate`), grouped by
//! period and family, ranked by blocks, then swept volume, and the best `--top` distinct
//! structures of each group are written with a name that says what they are
//! (`p<P>-<family abbreviations>-<blocks>b[-<i>].txt`) and the judge's summary as a comment.

use crate::census::canon_structure;
use crate::sim::Spec;
use anyhow::Result;
use clap::Args as ClapArgs;
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct PickArgs {
    /// Folders or files of clock designs
    pub inputs: Vec<PathBuf>,
    #[arg(long)]
    pub out_dir: PathBuf,
    /// Designs kept per (period, family)
    #[arg(long, default_value_t = 1)]
    pub top: usize,
    /// Only these periods (comma-separated)
    #[arg(long)]
    pub periods: Option<String>,
    /// Skip (period, family) groups already present in these folders (by period and family)
    #[arg(long)]
    pub skip_known: Vec<PathBuf>,
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
}

pub fn abbrev(family: &[String]) -> String {
    let mut v: Vec<&str> = family
        .iter()
        .map(|f| match f.as_str() {
            "observer" => "obs",
            "piston" => "pis",
            "sticky_piston" => "stk",
            "note_block" => "note",
            "redstone_lamp" => "lamp",
            "copper_bulb" => "bulb",
            "redstone_block" => "rb",
            "iron_trapdoor" => "trap",
            "powered_rail" => "rail",
            "activator_rail" => "arail",
            "repeater" => "rep",
            "comparator" => "cmp",
            "redstone_wire" => "dust",
            "redstone_torch" => "torch",
            other => other,
        })
        .collect();
    v.sort();
    v.join("-")
}

pub fn files_of(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    files(paths)
}

fn files(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in paths {
        if p.is_dir() {
            let mut f: Vec<PathBuf> = std::fs::read_dir(p)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|f| f.extension().is_some_and(|x| x == "txt")).collect();
            f.sort();
            out.extend(f);
        } else {
            out.push(p.clone());
        }
    }
    Ok(out)
}

struct Row {
    period: u32,
    family: Vec<String>,
    blocks: usize,
    volume: i32,
    summary: String,
    path: PathBuf,
    spec: Spec,
    key: Vec<([i32; 3], String)>,
}

pub fn run(a: PickArgs) -> Result<()> {
    let want: Option<HashSet<u32>> = a.periods.as_ref().map(|s| s.split(',').filter_map(|t| t.trim().parse().ok()).collect());
    let inputs = files(&a.inputs)?;
    let mut known: HashSet<(u32, String)> = HashSet::new();
    for f in files(&a.skip_known)? {
        if let Ok(c) = clock::Clock::load(&f) {
            let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 20000, margin: 4 });
            if r.verdict == clock::sim::Verdict::Clock {
                known.insert((r.period, clock::wave::signature(&r.analysis.family)));
            }
        }
    }
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    let next = std::sync::atomic::AtomicUsize::new(0);
    let rows: std::sync::Mutex<Vec<Row>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if i >= inputs.len() {
                    break;
                }
                let path = &inputs[i];
                let Ok(c) = clock::Clock::load(path) else { continue };
                let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 20000, margin: 4 });
                if r.verdict != clock::sim::Verdict::Clock {
                    continue;
                }
                if want.as_ref().is_some_and(|w| !w.contains(&r.period)) {
                    continue;
                }
                if known.contains(&(r.period, clock::wave::signature(&r.analysis.family))) {
                    continue;
                }
                let Ok(spec) = Spec::load(path) else { continue };
                let key = canon_structure(&spec.blocks);
                rows.lock().unwrap().push(Row { period: r.period, family: r.analysis.family.clone(), blocks: r.blocks, volume: r.volume(), summary: r.summary(), path: path.clone(), spec, key });
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|x, y| (x.period, &x.family, x.blocks, x.volume, &x.key).cmp(&(y.period, &y.family, y.blocks, y.volume, &y.key)));
    std::fs::create_dir_all(&a.out_dir)?;
    let mut groups: BTreeMap<(u32, Vec<String>), Vec<&Row>> = BTreeMap::new();
    for r in &rows {
        let g = groups.entry((r.period, r.family.clone())).or_default();
        if g.len() < a.top && !g.iter().any(|q| q.key == r.key) {
            g.push(r);
        }
    }
    println!("| P | family | blocks | swept volume | file | from |");
    println!("|---|---|---|---|---|---|");
    for ((p, fam), g) in &groups {
        for (i, r) in g.iter().enumerate() {
            let suffix = if g.len() > 1 { format!("-{}", (b'a' + i as u8) as char) } else { String::new() };
            let name = format!("p{p}-{}-{}b{suffix}", abbrev(fam), r.blocks);
            let mut spec = r.spec.clone();
            spec.name = format!("clk-obs-{name}");
            spec.output = None;
            let from = r.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let text = spec.to_text(&format!("{}\n(clk-obs, found by {from}; output: the judge's default, the changing cell with the most rising edges)", r.summary));
            std::fs::write(a.out_dir.join(format!("{name}.txt")), text)?;
            println!("| {p} | {} | {} | {} | {name}.txt | {from} |", clock::wave::signature(fam), r.blocks, r.volume);
        }
    }
    Ok(())
}
