//! A TNT engine held by a catalog clock (clk-misc, 2026-10-20: "ONE TNT duper from catalog parts").
//! The duper engine (tnt-triggered-launch-16b's) dupes one TNT each time its sticky piston loses
//! power for a moment: held extended, released, it retracts, its frame's redstone block comes back
//! next to its dust and re-extends it, priming a TNT. A plain pulse on the piston primes nothing.
//! So the clock must give a HOLD signal: on except one short window per period. This scans the
//! archived clocks for a cell where a piston would be powered like that, puts the engine's piston
//! in that cell and judges the whole machine on redsim (`circuit sim`).

use clock::Clock;
use mctas::design::Design;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

type Pos = [i32; 3];

/// What to scan.
pub struct HoldOpts {
    /// The engine's piston (the cell the hold signal must power), in the engine's coordinates.
    pub piston: Pos,
    /// The release window: one run of unpowered ticks per period, this long.
    pub min_off: u32,
    pub max_off: u32,
    /// The clock's size and period range.
    pub max_blocks: usize,
    pub min_period: u32,
    pub max_period: u32,
    /// Judge every placement on redsim (`circuit sim`); otherwise list the hold cells only.
    pub judge: bool,
    /// Where passing machines are written.
    pub out: Option<PathBuf>,
}

/// One archived clock turned `turns` quarter turns clockwise (seen from above): blocks, ports,
/// block entities and a temporary start block turned like the game turns them.
pub fn turned(c: &Clock, turns: u8) -> anyhow::Result<Clock> {
    if turns % 4 == 0 {
        return Ok(c.clone());
    }
    let mut d = c.design.clone();
    d.blocks = c.design.blocks.iter().map(|(p, s)| (clock::rotate_pos(*p, turns), clock::canon::rotate_state(s, turns))).collect();
    d.ports = c.design.ports.iter().map(|(k, p)| (k.clone(), clock::rotate_pos(*p, turns))).collect();
    d.block_entities = c.design.block_entities.iter().map(|(p, s)| (clock::rotate_pos(*p, turns), s.clone())).collect();
    if let Some(args) = c.design.meta.get("port start")
        && args.starts_with("minecraft:")
    {
        let mut it = args.splitn(2, ' ');
        let state = it.next().unwrap_or_default();
        let rest = it.next().unwrap_or_default();
        d.meta.insert("port start".into(), format!("{} {rest}", clock::canon::rotate_state(state, turns)).trim().to_string());
    }
    Clock::from_design(d)
}

/// The cells next to the clock where a piston would be powered on every tick of the period but one
/// run of `min_off..=max_off` ticks: (cell, period, the window's length, the window's start).
pub fn hold_cells(c: &Clock, o: &HoldOpts) -> Vec<(Pos, u32, u32, u32)> {
    let rep = clock::sim::simulate(c, &clock::sim::SimOptions::default());
    if rep.verdict != clock::sim::Verdict::Clock || rep.period < o.min_period || rep.period > o.max_period {
        return Vec::new();
    }
    let (w, period) = (rep.state_warmup.max(rep.warmup), rep.period);
    let (slo, shi) = rep.swept;
    let mut probes = Vec::new();
    for x in slo[0] - 1..=shi[0] + 1 {
        for y in slo[1] - 1..=shi[1] + 1 {
            for z in slo[2] - 1..=shi[2] + 1 {
                probes.push([x, y, z]);
            }
        }
    }
    let (from, to) = ([slo[0] - 4, slo[1] - 4, slo[2] - 4], [shi[0] + 4, shi[1] + 4, shi[2] + 4]);
    let Ok(d) = clock::sim::drive(c, &[], w + 2 * period, &probes, from, to) else { return Vec::new() };
    if d.stopped.is_some() {
        return Vec::new();
    }
    let filled: HashSet<Pos> = d.cells.iter().flat_map(|f| f.iter().map(|(p, _)| *p)).collect();
    let mut out = Vec::new();
    for (i, q) in probes.iter().enumerate() {
        if filled.contains(q) {
            continue;
        }
        let seq: Vec<bool> = (w + period..w + 2 * period).map(|t| d.signals[t as usize][i] > 0).collect();
        if let Some((len, start)) = one_window(&seq)
            && len >= o.min_off
            && len <= o.max_off
        {
            out.push((*q, period, len, start));
        }
    }
    out
}

/// A cyclic waveform's single run of `false` (its length and first tick), or None when it has no
/// run or several.
fn one_window(seq: &[bool]) -> Option<(u32, u32)> {
    let n = seq.len();
    let on = seq.iter().position(|&b| b)?;
    let mut runs = Vec::new();
    let mut i = 0;
    while i < n {
        let k = (on + i) % n;
        if !seq[k] {
            let start = k;
            let mut len = 0;
            while i < n && !seq[(on + i) % n] {
                len += 1;
                i += 1;
            }
            runs.push((len as u32, start as u32));
        } else {
            i += 1;
        }
    }
    if runs.len() == 1 { Some(runs[0]) } else { None }
}

/// The engine with its piston moved to `cell`, merged with the clock: None when a block of one
/// stands where the other has a block, or a clock block touches the engine's slime (in either frame
/// position: the frame moves one cell along the piston's facing).
pub fn merge(engine: &Design, piston: Pos, c: &Clock, cell: Pos, clock_cells: &HashSet<Pos>, facing: Pos, name: &str) -> Option<Design> {
    let off = [cell[0] - piston[0], cell[1] - piston[1], cell[2] - piston[2]];
    let e = engine.translated(off);
    let mut eng_cells: BTreeSet<Pos> = BTreeSet::new();
    let mut slime: Vec<Pos> = Vec::new();
    for (p, s) in &e.blocks {
        eng_cells.insert(*p);
        if *p != cell {
            eng_cells.insert([p[0] + facing[0], p[1] + facing[1], p[2] + facing[2]]);
        }
        if s.contains("slime_block") {
            slime.push(*p);
            slime.push([p[0] + facing[0], p[1] + facing[1], p[2] + facing[2]]);
        }
    }
    for q in clock_cells {
        if eng_cells.contains(q) {
            return None;
        }
        if slime.iter().any(|s| (0..3).map(|k| (s[k] - q[k]).abs()).sum::<i32>() == 1) {
            return None;
        }
    }
    let mut d = e.clone();
    d.name = name.to_string();
    for (p, s) in &c.design.blocks {
        d.blocks.insert(*p, s.clone());
    }
    for (p, s) in &c.design.block_entities {
        d.block_entities.insert(*p, s.clone());
    }
    if let Some(p) = c.design.ports.get("start") {
        d.ports.insert("start".into(), *p);
        if let Some(a) = c.design.meta.get("port start") {
            d.meta.insert("port start".into(), a.clone());
        }
    }
    if let Some(pl) = c.design.meta.get("placement") {
        d.meta.insert("placement".into(), pl.clone());
    }
    Some(d)
}

/// The archived clocks worth trying: robust, shown on the catalog (not hidden), no sculk (a TNT
/// blast is a vibration), no entities, at most `max_blocks` blocks: (file, blocks).
pub fn candidates(root: &Path, max_blocks: usize) -> anyhow::Result<Vec<(PathBuf, usize)>> {
    let tsv = std::fs::read_to_string(root.join("verdicts-v1.tsv"))?;
    let mut out = Vec::new();
    for line in tsv.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 18 {
            continue;
        }
        let (file, family, blocks, robust, hidden) = (f[1], f[7], f[8].parse::<usize>().unwrap_or(usize::MAX), f[11], f.get(17).copied().unwrap_or(""));
        if robust != "robust" || !hidden.trim().is_empty() || blocks > max_blocks || family.contains("sculk") {
            continue;
        }
        out.push((root.join(file), blocks));
    }
    out.sort_by_key(|(p, b)| (*b, p.clone()));
    out.dedup();
    Ok(out)
}

/// Scan, place, judge: one line per placement tried.
pub fn scan(engine_path: &Path, archive: &Path, o: &HoldOpts) -> anyhow::Result<()> {
    let engine = Design::load(engine_path)?;
    let facing = match engine.blocks.get(&o.piston).map(|s| s.as_str()) {
        Some(s) if s.contains("facing=north") => [0, 0, -1],
        Some(s) if s.contains("facing=south") => [0, 0, 1],
        Some(s) if s.contains("facing=east") => [1, 0, 0],
        Some(s) if s.contains("facing=west") => [-1, 0, 0],
        Some(s) if s.contains("facing=up") => [0, 1, 0],
        Some(s) if s.contains("facing=down") => [0, -1, 0],
        _ => anyhow::bail!("no piston at {:?} in {}", o.piston, engine_path.display()),
    };
    if let Some(dir) = &o.out {
        std::fs::create_dir_all(dir)?;
    }
    let mut seen: HashSet<String> = HashSet::new();
    for (file, blocks) in candidates(archive, o.max_blocks)? {
        let Ok(base) = Clock::load(&file) else { continue };
        if !base.design.entities.is_empty() {
            continue;
        }
        for turns in 0..4u8 {
            let Ok(c) = turned(&base, turns) else { continue };
            let rep_cells = {
                let rep = clock::sim::simulate(&c, &clock::sim::SimOptions::default());
                let (slo, shi) = rep.swept;
                let (from, to) = ([slo[0] - 4, slo[1] - 4, slo[2] - 4], [shi[0] + 4, shi[1] + 4, shi[2] + 4]);
                match clock::sim::drive(&c, &[], rep.warmup + rep.period, &[], from, to) {
                    Ok(d) => d.cells.iter().flat_map(|f| f.iter().map(|(p, _)| *p)).collect::<HashSet<Pos>>(),
                    Err(_) => continue,
                }
            };
            for (cell, period, len, start) in hold_cells(&c, o) {
                let name = format!("hold-{}-r{turns}", c.name());
                let Some(d) = merge(&engine, o.piston, &c, cell, &rep_cells, facing, &name) else { continue };
                let key = d.content_key();
                if !seen.insert(key) {
                    continue;
                }
                let head = format!("{} r{turns} ({blocks} blocks, P {period}, release {len} gt from {start}) at {:?}", c.name(), cell);
                if !o.judge {
                    println!("HOLD {head}");
                    continue;
                }
                let line = match circuit::Circuit::from_design(d.clone()) {
                    Ok(cc) => match circuit::verify::sim(&cc, &circuit::verify::Options::default()) {
                        Ok(r) => r.summary(&cc),
                        Err(e) => format!("ERROR {e}"),
                    },
                    Err(e) => format!("ERROR {e}"),
                };
                println!("{head}: {line}");
                if line.starts_with("CIRCUIT") {
                    if let Some(dir) = &o.out {
                        let mut d = d.clone();
                        d.meta.insert("circuit".into(), "tnt-duper".into());
                        std::fs::write(dir.join(format!("{name}-{}-{}-{}.txt", cell[0], cell[1], cell[2])), d.to_text())?;
                    }
                }
            }
        }
    }
    Ok(())
}
