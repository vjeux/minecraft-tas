//! Pick the smallest clocks per (period, family) from search outputs (census, grow folders):
//! every design is re-run with the official redsim judge (`clock::sim::simulate`), grouped by
//! period and family, ranked by blocks, then swept volume, and the best `--top` distinct
//! structures of each group are written with a name that says what they are
//! (`p<P>-<family abbreviations>-<blocks>b[-<i>].txt`) and the judge's summary as a comment.

use crate::census::canon_structure;
use crate::sim::{self, Spec};
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
    /// Skip designs whose structure (up to rotation and translation) is one of these designs'
    /// (files or folders): e.g. what another batch already archives
    #[arg(long)]
    pub skip_structures: Vec<PathBuf>,
    /// Keep designs of a group only when their steady-state MECHANISMS differ (`cores`: the
    /// changing cells and the static blocks they need, canonical over phases), not just their
    /// structures (a different starter or inert block is the same mechanism)
    #[arg(long)]
    pub mechanisms: bool,
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
    /// Keep only clocks whose changing cells have at most this many rising edges per period
    /// (1 = single-pulse machines: every component fires once per period)
    #[arg(long)]
    pub max_edges: Option<u32>,
    /// One group per period (the smallest clock of each period, whatever its family)
    #[arg(long)]
    pub by_period: bool,
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

/// The spec turned by `turns` quarter turns about the vertical axis (blocks, kick, output).
pub fn rotated(spec: &Spec, turns: u8) -> Spec {
    use crate::census::{rot, rotate_block};
    let rp = |p: redsim::BlockPos| -> redsim::BlockPos {
        let mut a = p.arr();
        for _ in 0..turns {
            a = rot(a);
        }
        redsim::BlockPos::new(a[0], a[1], a[2])
    };
    let rb = |b: redsim::Block| -> redsim::Block {
        let mut b = b;
        for _ in 0..turns {
            b = rotate_block(b);
        }
        b
    };
    let kick = match &spec.kick {
        sim::Kick::None => sim::Kick::None,
        sim::Kick::Last(p) => sim::Kick::Last(rp(*p)),
        sim::Kick::Use(p, t) => sim::Kick::Use(rp(*p), *t),
        sim::Kick::Temp(p, b, t) => sim::Kick::Temp(rp(*p), rb(*b), *t),
    };
    Spec { name: spec.name.clone(), blocks: spec.blocks.iter().map(|(p, b)| (rp(*p), rb(*b))).collect(), kick, output: spec.output.map(rp) }
}

/// Same period in all 4 rotations (redsim; the update order W, E, D, U, N, S makes some
/// clocks rotation-dependent)
pub fn rotation_robust(spec: &Spec, period: u32) -> bool {
    (1..4).all(|t| {
        let r = sim::simulate(&rotated(spec, t), &sim::Opts { horizon: 20000, margin: 6, period_only: true });
        r.verdict == sim::Verdict::Clock && r.period == period
    })
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
    /// The judge's output cell on the design as written (declared, or its auto choice).
    output: Option<[i32; 3]>,
    /// Same period in all 4 rotations (redsim).
    robust: bool,
    /// The steady-state mechanism (`cores::core_of`), with --mechanisms.
    core: Option<u128>,
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
    let mut skip_keys: HashSet<u128> = HashSet::new();
    for f in files(&a.skip_structures)? {
        if let Ok(s) = Spec::load(&f) {
            skip_keys.insert(crate::census::canon_hash(&s.blocks));
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
                if let Some(m) = a.max_edges {
                    if r.analysis.cells.iter().any(|c| c.edges > m) {
                        continue;
                    }
                }
                let Ok(spec) = Spec::load(path) else { continue };
                if !skip_keys.is_empty() && skip_keys.contains(&crate::census::canon_hash(&spec.blocks)) {
                    continue;
                }
                let key = canon_structure(&spec.blocks);
                let robust = rotation_robust(&spec, r.period);
                // declared output: the changing observer with the most edges (self-reporting in
                // every rotation), else the judge's auto choice
                let obs_out = r.analysis.cells.iter().filter(|c| c.kinds.iter().any(|k| k == "observer") && c.states.iter().all(|s| s.starts_with("observer"))).max_by(|a, b| (a.edges, a.on).cmp(&(b.edges, b.on)).then(b.pos.cmp(&a.pos))).map(|c| c.pos);
                let output = obs_out.or(r.output.as_ref().map(|o| o.pos));
                let core = if a.mechanisms { crate::cores::core_of(&spec).map(|c| c.0) } else { None };
                rows.lock().unwrap().push(Row { period: r.period, family: r.analysis.family.clone(), blocks: r.blocks, volume: r.volume(), summary: r.summary(), path: path.clone(), spec, key, output, robust, core });
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by(|x, y| (x.period, &x.family, x.blocks, !x.robust, x.volume, &x.key).cmp(&(y.period, &y.family, y.blocks, !y.robust, y.volume, &y.key)));
    if a.by_period {
        // one group per period: fewest blocks, then fewest block types in the family, then volume
        rows.sort_by(|x, y| (x.period, x.blocks, !x.robust, x.family.len(), x.volume, &x.family, &x.key).cmp(&(y.period, y.blocks, !y.robust, y.family.len(), y.volume, &y.family, &y.key)));
    }
    std::fs::create_dir_all(&a.out_dir)?;
    let mut groups: BTreeMap<(u32, Vec<String>), Vec<&Row>> = BTreeMap::new();
    let mut period_groups: BTreeMap<u32, Vec<String>> = BTreeMap::new();
    for r in &rows {
        let fam_key = if a.by_period { period_groups.entry(r.period).or_insert_with(|| r.family.clone()).clone() } else { r.family.clone() };
        let g = groups.entry((r.period, fam_key)).or_default();
        let same = |q: &&Row| if a.mechanisms { q.core.is_some() && q.core == r.core } else { q.key == r.key };
        if g.len() < a.top && !g.iter().any(|q| same(q)) {
            g.push(r);
        }
    }
    println!("| P | family | blocks | swept volume | rotations (redsim) | file | from |");
    println!("|---|---|---|---|---|---|---|");
    for ((p, fam), g) in &groups {
        for (i, r) in g.iter().enumerate() {
            let suffix = if g.len() > 1 { format!("-{}", (b'a' + i as u8) as char) } else { String::new() };
            let tag = crate::census::canon_hash(&r.spec.blocks) as u32;
            let name = format!("p{p}-{}-{}b{suffix}-{:06x}", abbrev(fam), r.blocks, tag & 0xff_ffff);
            let mut spec = r.spec.clone();
            spec.name = format!("clk-obs-{name}");
            spec.output = r.output.map(|o| redsim::BlockPos::new(o[0], o[1], o[2]));
            let from = r.path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let text = spec.to_text(&caption(&r.spec, &r.path, &from, *p, &r.family));
            std::fs::write(a.out_dir.join(format!("{name}.txt")), text)?;
            println!("| {p} | {} | {} | {} | {} | {name}.txt | {from} |", clock::wave::signature(fam), r.blocks, r.volume, if r.robust { "robust" } else { "DEPENDENT" });
        }
    }
    Ok(())
}

/// A one-paragraph caption for a picked design: a hand-written source keeps its own first comment
/// block; census and grow outputs get their parts, what changes in place and how they were found.
pub fn caption(spec: &Spec, path: &std::path::Path, from: &str, period: u32, family: &[String]) -> String {
    let census = from.starts_with("census-");
    let grow = from.starts_with("grow-");
    if !census && !grow {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let mut lines = Vec::new();
        for l in text.lines() {
            let t = l.trim();
            if let Some(c) = t.strip_prefix('#') {
                lines.push(c.trim().to_string());
            } else if !lines.is_empty() && !t.starts_with("name:") {
                break;
            }
        }
        if !lines.is_empty() {
            return lines.join("\n");
        }
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for (_, b) in &spec.blocks {
        let n = match b {
            redsim::Block::Piston { sticky: true, .. } => "sticky piston".to_string(),
            redsim::Block::Piston { sticky: false, .. } => "piston".to_string(),
            _ => sim::short(b).replace("waxed_", "").replace('_', " "),
        };
        *counts.entry(n).or_default() += 1;
    }
    let parts: Vec<String> = counts.iter().map(|(n, c)| if *c == 1 { format!("1 {n}") } else { format!("{c} {n}s") }).collect();
    let how = if census {
        let k = from.trim_start_matches("census-k").split('-').next().unwrap_or("?");
        format!("Found by the clk-obs exhaustive census of every {k}-block structure (all orientations, every block placed last as a kick)")
    } else {
        "Found by the clk-obs grow search (minimal clocks extended or changed one block at a time)".to_string()
    };
    let moved: Vec<String> = counts.keys().filter(|n| !family.iter().any(|f| f.replace("waxed_", "").replace('_', " ") == **n)).cloned().collect();
    let moved = if moved.is_empty() { String::new() } else { format!(" ({} only moved or static)", moved.join(", ")) };
    format!(
        "P{period}: {}; changing in place: {}{moved}. {how}; minimal: no single block can be removed keeping the period. Output: the changing observer with the most rising edges (declared, so every rotation reads the same cell).",
        parts.join(", "),
        family.join(", ")
    )
}
