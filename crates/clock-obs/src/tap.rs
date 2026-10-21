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
    /// The pulse spacing a tap must carry: `auto` (the archived row's spacing when its gaps are
    /// all equal, else one pulse per period; the default), `period`, `row` (the row's first gap),
    /// `any` (any even spacing) or a number of gt.
    #[arg(long, default_value = "auto")]
    pub gap: String,
    #[arg(long, default_value_t = 1)]
    pub threads: usize,
    /// Also seal the tapped design: glass on every open hopper top (clock::intrusion::intakes, no
    /// dropped item can reach a hopper), kept only when the clock runs unchanged on redsim.
    #[arg(long)]
    pub seal: bool,
    /// List the periods whose smallest shown clocks all have an open hopper top (`SENSITIVE
    /// period blocks file`, one line per such clock), then stop: the input of `--list`.
    #[arg(long)]
    pub scan: bool,
}

/// `--scan`: per period, the smallest shown clocks (hidden column empty); a period whose smallest
/// all have an open intake is listed with each of them and the smallest SEALED build of the same
/// period (a shown clock with no open intake, or a recorded member of a family whose base has
/// none), or `STILL SENSITIVE`.
fn scan(archive: &Path) -> Result<()> {
    let rows = clock::catalog::read_rows(archive)?;
    let mut by: std::collections::BTreeMap<u32, Vec<&clock::catalog::Row>> = std::collections::BTreeMap::new();
    for r in rows.iter().filter(|r| r.hidden.is_empty()) {
        by.entry(r.period).or_default().push(r);
    }
    let is_open = |p: &Path| Clock::load(p).map(|c| !clock::intrusion::intakes(&c.design, &[]).is_empty()).unwrap_or(false);
    let instances = clock::extend::read_instances(archive);
    let bases = clock::tables::family_bases(archive);
    let mut fam_open: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
    for i in &instances {
        if !fam_open.contains_key(&i.family) {
            // a family no design declares counts as open (unknown: never as sealed)
            fam_open.insert(i.family.clone(), bases.get(&i.family).is_none_or(|b| is_open(b)));
        }
    }
    let family_open = |f: &str| fam_open.get(f).copied().unwrap_or(false);
    // the smallest sealed build of a period: `<name> (<blocks> blocks)`
    let sealed_at = |p: u32| -> Option<String> {
        let mut best: Option<(usize, String)> = None;
        for r in by.get(&p).into_iter().flatten() {
            if best.as_ref().is_none_or(|(b, _)| r.blocks < *b) && !is_open(&archive.join(&r.file)) {
                best = Some((r.blocks, r.file.trim_end_matches(".txt").to_string()));
            }
        }
        for i in instances.iter().filter(|i| i.period == p && i.verdict != "FAIL" && !family_open(&i.family)) {
            if best.as_ref().is_none_or(|(b, _)| i.blocks < *b) {
                best = Some((i.blocks, format!("{} n = {}", i.family, i.n)));
            }
        }
        best.map(|(b, n)| format!("sealed: {n} ({b} blocks)"))
    };
    let (mut periods, mut clocks, mut still) = (0, 0, 0);
    for (p, rs) in &by {
        let min = rs.iter().map(|r| r.blocks).min().unwrap_or(0);
        let small: Vec<&&clock::catalog::Row> = rs.iter().filter(|r| r.blocks == min).collect();
        let open: Vec<&&clock::catalog::Row> = small.iter().copied().filter(|r| is_open(&archive.join(&r.file))).collect();
        if !open.is_empty() && open.len() == small.len() {
            periods += 1;
            let by_sealed = sealed_at(*p);
            if by_sealed.is_none() {
                still += 1;
            }
            for r in open {
                clocks += 1;
                println!("SENSITIVE\t{p}\t{min}\t{}\t{}", by_sealed.clone().unwrap_or_else(|| "STILL SENSITIVE".into()), archive.join(&r.file).display());
            }
        }
    }
    eprintln!("{periods} periods whose smallest shown clocks all have an open hopper top ({clocks} clocks; {still} without a sealed build of the same period)");
    // recorded family members (extend.tsv) of a family whose base has an open hopper top, smaller
    // than every shown clock of their period: the period's smallest build is sensitive too
    let (mut members, mut still_m) = (0, 0);
    for i in &instances {
        let min = by.get(&i.period).and_then(|rs| rs.iter().map(|r| r.blocks).min());
        if family_open(&i.family) && i.verdict != "FAIL" && min.is_none_or(|m| i.blocks < m) {
            members += 1;
            let by_sealed = sealed_at(i.period);
            if by_sealed.is_none() {
                still_m += 1;
            }
            println!("SENSITIVE-MEMBER\t{}\t{}\t{}\t{} n = {}", i.period, i.blocks, by_sealed.unwrap_or_else(|| "STILL SENSITIVE".into()), i.family, i.n);
        }
    }
    eprintln!("{members} recorded family members of an open-topped family that are their period's smallest ({still_m} without a sealed build of the same period)");
    Ok(())
}

/// The design with its open hopper tops sealed (clock::intrusion::intakes: a cell above a hopper a
/// dropped item can reach): glass in an intake cell that is air, or on each free cell around one
/// holding a thin block (as circuit::intrusion::seal), never on the output or the tap's lamp cell;
/// repeated until no intake is left (4 rounds at most). None when an intake stays open or the
/// sealed design is not the same clock on redsim (period, a clock at all).
pub(crate) fn sealed(c: &Clock) -> Option<Clock> {
    let mut d = c.design.clone();
    let mut keep: Vec<[i32; 3]> = d.ports.values().copied().collect();
    if let Some(o) = c.output {
        keep.push(o);
    }
    let mut added = 0;
    for _ in 0..4 {
        let at = clock::intrusion::intakes(&d, &[]);
        if at.is_empty() {
            break;
        }
        for u in at {
            let cells: Vec<[i32; 3]> = if d.blocks.contains_key(&u) {
                [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, 0, 1], [0, 0, -1]].iter().map(|dd| [u[0] + dd[0], u[1] + dd[1], u[2] + dd[2]]).filter(|n| !d.blocks.contains_key(n) && !keep.contains(n)).collect()
            } else if keep.contains(&u) {
                Vec::new()
            } else {
                vec![u]
            };
            for n in cells {
                d.blocks.insert(n, "minecraft:glass".into());
                added += 1;
            }
        }
    }
    if !clock::intrusion::intakes(&d, &[]).is_empty() {
        return None;
    }
    if added == 0 {
        return Some(c.clone());
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
        // the clock's own spacing when it pulses evenly (P81 pulses every 27 gt), else per period
        "auto" => match row.and_then(|r| clock::catalog::parse_gaps(&r.gaps)) {
            Some(g) if !g.is_empty() && g.iter().all(|x| *x == g[0]) => Some(g[0]),
            _ => (period > 0).then_some(period),
        },
        n => Some(n.parse::<u32>()?),
    };
    let picked = clock::usable::judge(&c, Rule::default(), gap).map(|(t, s)| (Some(t), Some(s), true)).or_else(|_| clock::usable::pick(&c, Rule::default(), gap).map(|(u, s)| (Some(u.tap), Some(s), false)));
    // sealing a clock with no usable tap keeps its declared output (the period still gets a sealed
    // build; the output stays as unusable as before)
    let picked = match picked {
        Err(e) if a.seal && c.output.is_some() => Ok((None, None, true)).map_err(|_: ()| e),
        p => p,
    };
    Ok(match picked {
        Ok((tap, s, kept)) => {
            let tapped = match tap {
                None => c.clone(),
                Some(t) if kept || t.kind == TapKind::Repeater => clock::usable::with_output(&c, t.out)?,
                Some(t) => clock::usable::joined(&c, t)?,
            };
            let tapped = if a.seal {
                match sealed(&tapped) {
                    Some(t) => t,
                    None => return Ok(format!("NOSEAL\t{period}\t{}\t{name}\tsealing leaves an intake open or changes the clock", tapped.blocks())),
                }
            } else {
                tapped
            };
            let text = std::fs::read_to_string(f)?;
            let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CLOCK: ") && !l.starts_with("port output:")).map(|l| format!("{l}\n")).collect();
            let mut d = mctas::design::Design::parse(&body)?;
            d.blocks = tapped.design.blocks.clone();
            d.ports = tapped.design.ports.clone();
            // a tapped or sealed copy is not its family's base (its own blocks, its own clock)
            d.meta.retain(|k, _| !k.starts_with("extend"));
            let head: String = body.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
            // a sealed design is a new clock: `<name>-sealed`
            let added = tapped.blocks() as i64 - c.blocks() as i64;
            let name = if a.seal && added > 0 && !kept_name(&name) {
                d.name = format!("{}-sealed", d.name);
                name.replace(".txt", "-sealed.txt")
            } else {
                name
            };
            let out = a.out_dir.join(&name);
            let how = if a.seal { format!("; sealed: {added} block(s) added (glass on the open hopper tops)") } else { String::new() };
            let (what, word) = match (tap, s.as_ref()) {
                (Some(t), Some(s)) => (format!("usable tap (clk-obs `clock obs tap`): a {} at {:?} ({}), the output at {:?}", t.kind.word(), t.out, s.stand(t.out), tapped.output.unwrap_or_default()), t.kind.word()),
                _ => ("no usable tap: the declared output kept (clk-obs `clock obs tap`)".to_string(), "declared (not usable)"),
            };
            std::fs::write(&out, format!("{head}# {what}{how}\n{}", d.to_text()))?;
            format!("TAP\t{period}\t{}\t{name}\t{word}\t{}", tapped.blocks(), out.display())
        }
        Err(e) => format!("NONE\t{period}\t0\t{name}\t{}", e.chars().take(160).collect::<String>()),
    })
}

/// A file name that already says it is sealed.
fn kept_name(name: &str) -> bool {
    name.contains("-sealed")
}

pub fn run(a: TapArgs) -> Result<()> {
    if a.scan {
        return scan(&a.archive);
    }
    std::fs::create_dir_all(&a.out_dir)?;
    let rows = clock::catalog::read_rows(&a.archive)?;
    let mut files = a.files.clone();
    if let Some(l) = &a.list {
        // a path per line, or `--scan`'s lines (the path last)
        files.extend(std::fs::read_to_string(l)?.lines().filter(|s| !s.trim().is_empty()).map(|s| PathBuf::from(s.rsplit('\t').next().unwrap_or(s))));
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
