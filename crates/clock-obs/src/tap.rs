//! `mctas clock obs tap <archived files...> --out-dir D`: the best usable tap of each clock
//! (clock::usable::pick), hidden ones included (clk-obs, 2026-10-21, the usable-output batches): a
//! repeater tap moves the design's output to the tap cell; a comparator or observer tap joins the
//! design (`usable::joined`). The tapped design goes in `--out-dir` (same file name) for `clock
//! archive` or `clock verify`; one line per clock: the tap and the block count, or why none.
//! `--list F`: the files from a list (one per line); `--threads N` runs them in parallel.

use anyhow::Result;
use clap::Args as ClapArgs;
use clock::Clock;
use clock::usable::{Rule, TapKind};
use std::path::{Path, PathBuf};

#[derive(ClapArgs, Debug)]
pub struct TapArgs {
    pub files: Vec<PathBuf>,
    /// More files, one per line.
    #[arg(long)]
    pub list: Option<PathBuf>,
    #[arg(long, default_value = "/tmp/clock-tap")]
    pub out_dir: PathBuf,
    #[arg(long, default_value = "designs/clocks/archive")]
    pub archive: PathBuf,
    /// The pulse spacing a tap must carry: `period` (one pulse per period, the default), `row`
    /// (the archived row's first gap), `any` (any even spacing) or a number of gt.
    #[arg(long, default_value = "period")]
    pub gap: String,
    #[arg(long, default_value_t = 1)]
    pub threads: usize,
    /// Also seal the tapped design: a stone over every hopper whose top is open (no dropped item
    /// can reach it; the intrusion rule), kept only when the clock runs unchanged on redsim.
    #[arg(long)]
    pub seal: bool,
}

/// A stone over every hopper whose cell above is air in the design (and not the output cell);
/// None when the sealed design is not the same clock on redsim (period, a clock at all).
fn sealed(c: &Clock) -> Option<Clock> {
    let mut d = c.design.clone();
    let tops: Vec<[i32; 3]> = d.blocks.iter().filter(|(_, s)| s.starts_with("minecraft:hopper")).map(|(p, _)| [p[0], p[1] + 1, p[2]]).filter(|q| !d.blocks.contains_key(q) && c.output != Some(*q)).collect();
    if tops.is_empty() {
        return Some(c.clone());
    }
    for q in tops {
        d.blocks.insert(q, "minecraft:stone".into());
    }
    let s = Clock::from_design(d).ok()?;
    let a = clock::sim::simulate(c, &clock::sim::SimOptions::default());
    let b = clock::sim::simulate(&s, &clock::sim::SimOptions::default());
    (b.verdict == clock::sim::Verdict::Clock && b.period == a.period).then_some(s)
}

fn one(f: &Path, rows: &[clock::catalog::Row], a: &TapArgs) -> Result<String> {
    let name = f.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let c = Clock::load(f)?;
    let row = rows.iter().find(|r| r.file == name);
    // an unarchived design: its own period on redsim
    let sim_period = || {
        let r = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
        (r.verdict == clock::sim::Verdict::Clock).then_some(r.period)
    };
    let period = row.map(|r| r.period).or_else(sim_period).unwrap_or(0);
    let gap = match a.gap.as_str() {
        "any" => None,
        "row" => row.and_then(|r| clock::catalog::parse_gaps(&r.gaps)).and_then(|g| g.first().copied()),
        "period" => (period > 0).then_some(period),
        n => Some(n.parse::<u32>()?),
    };
    Ok(match clock::usable::pick(&c, Rule::default(), gap) {
        Ok((u, s)) => {
            let tapped = if u.tap.kind == TapKind::Repeater { clock::usable::with_output(&c, u.tap.out)? } else { clock::usable::joined(&c, u.tap)? };
            let tapped = if a.seal {
                match sealed(&tapped) {
                    Some(t) => t,
                    None => return Ok(format!("NOSEAL\t{period}\t{}\t{name}\tsealing changes the clock", tapped.blocks())),
                }
            } else {
                tapped
            };
            let text = std::fs::read_to_string(f)?;
            let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CLOCK: ") && !l.starts_with("port output:")).map(|l| format!("{l}\n")).collect();
            let mut d = mctas::design::Design::parse(&body)?;
            d.blocks = tapped.design.blocks.clone();
            d.ports = tapped.design.ports.clone();
            let head: String = body.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
            let out = a.out_dir.join(&name);
            std::fs::write(&out, format!("{head}# usable tap (clk-obs `clock obs tap`): a {} at {:?} ({}), the output at {:?}\n{}", u.tap.kind.word(), u.tap.out, s.stand(u.tap.out), tapped.output.unwrap_or_default(), d.to_text()))?;
            format!("TAP\t{period}\t{}\t{name}\t{}\t{}", tapped.blocks(), u.tap.kind.word(), out.display())
        }
        Err(e) => format!("NONE\t{period}\t0\t{name}\t{}", e.chars().take(160).collect::<String>()),
    })
}

pub fn run(a: TapArgs) -> Result<()> {
    std::fs::create_dir_all(&a.out_dir)?;
    let rows = clock::catalog::read_rows(&a.archive)?;
    let mut files = a.files.clone();
    if let Some(l) = &a.list {
        files.extend(std::fs::read_to_string(l)?.lines().filter(|s| !s.trim().is_empty()).map(PathBuf::from));
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..a.threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(f) = files.get(i) else { break };
                    let line = match one(f, &rows, &a) {
                        Ok(l) => l,
                        Err(e) => format!("ERROR\t0\t0\t{}\t{e:#}", f.display()),
                    };
                    println!("{line}");
                    out.lock().unwrap().push(line);
                }
            });
        }
    });
    Ok(())
}
