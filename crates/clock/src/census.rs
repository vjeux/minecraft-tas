//! `mctas clock census`: the ground truth of the small clocks. Every filling of an AxBxC box with
//! 1..=k blocks from a palette (every facing of every component), one per class up to translation
//! (the filling touches the box's three low faces) and the 4 rotations (the smallest encoding among
//! the rotations that fit the box), judged by redsim: placed like the judge (self-start), and when
//! that does not run, with each of its blocks placed last (a player's last block). Every clock found
//! goes to a TSV (period, waveform, family, blocks, box, start, type key, canonical encoding); the
//! smallest clock of each (period, family) gets a design file. Resumable: the work is split in
//! chunks (a set of cells and the state of its first cell), each written to `parts/` when done.

use crate::sim::{SimOptions, Verdict, quick_period, simulate};
use crate::{Clock, Design, Pos, canon, wave};
use anyhow::{Result, bail};
use redsim::{Block, BlockPos};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/// The census palette shorthands (each expands to every placeable facing / variant).
pub fn expand_palette(spec: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let h4 = ["north", "east", "south", "west"];
    let d6 = ["down", "up", "north", "south", "west", "east"];
    for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let mut add = |v: Vec<String>| {
            for s in v {
                if !out.contains(&s) {
                    out.push(s);
                }
            }
        };
        match item {
            "core" => add(expand_palette("stone,glass,rblock,dust,torch,repeater,comparator,observer,piston,sticky,lamp,bulb,note,target,slime")?),
            "stone" => add(vec!["minecraft:stone".into()]),
            "glass" => add(vec!["minecraft:glass".into()]),
            "obsidian" => add(vec!["minecraft:obsidian".into()]),
            "slime" => add(vec!["minecraft:slime_block".into()]),
            "honey" => add(vec!["minecraft:honey_block".into()]),
            "wool" => add(vec!["minecraft:white_wool".into()]),
            "rblock" => add(vec!["minecraft:redstone_block".into()]),
            "dust" => add(vec!["minecraft:redstone_wire".into()]),
            "torch" => {
                let mut v = vec!["minecraft:redstone_torch".to_string()];
                v.extend(h4.iter().map(|f| format!("minecraft:redstone_wall_torch[facing={f}]")));
                add(v)
            }
            "repeater" => add(h4.iter().flat_map(|f| (1..=4).map(move |d| format!("minecraft:repeater[delay={d},facing={f}]"))).collect()),
            "repeater1" => add(h4.iter().map(|f| format!("minecraft:repeater[delay=1,facing={f}]")).collect()),
            "comparator" => add(h4.iter().flat_map(|f| ["compare", "subtract"].into_iter().map(move |m| format!("minecraft:comparator[facing={f},mode={m}]"))).collect()),
            "observer" => add(d6.iter().map(|f| format!("minecraft:observer[facing={f}]")).collect()),
            "piston" => add(d6.iter().map(|f| format!("minecraft:piston[facing={f}]")).collect()),
            "sticky" => add(d6.iter().map(|f| format!("minecraft:sticky_piston[facing={f}]")).collect()),
            "lamp" => add(vec!["minecraft:redstone_lamp".into()]),
            "bulb" => add(vec!["minecraft:waxed_copper_bulb".into()]),
            "note" => add(vec!["minecraft:note_block".into()]),
            "target" => add(vec!["minecraft:target".into()]),
            "sculk" => add(vec!["minecraft:sculk_sensor".into()]),
            "csculk" => add(h4.iter().map(|f| format!("minecraft:calibrated_sculk_sensor[facing={f}]")).collect()),
            "amethyst" => add(vec!["minecraft:amethyst_block".into()]),
            "hopper" => add(["down", "north", "east", "south", "west"].iter().map(|f| format!("minecraft:hopper[facing={f}]")).collect()),
            "dropper" => add(d6.iter().map(|f| format!("minecraft:dropper[facing={f}]")).collect()),
            "dispenser" => add(d6.iter().map(|f| format!("minecraft:dispenser[facing={f}]")).collect()),
            "trapdoor" => add(h4.iter().flat_map(|f| ["bottom", "top"].into_iter().map(move |hf| format!("minecraft:oak_trapdoor[facing={f},half={hf}]"))).collect()),
            "itrapdoor" => add(h4.iter().flat_map(|f| ["bottom", "top"].into_iter().map(move |hf| format!("minecraft:iron_trapdoor[facing={f},half={hf}]"))).collect()),
            "leaves" => add(vec!["minecraft:oak_leaves[persistent=true]".into()]),
            "scaffold" => add(vec!["minecraft:scaffolding".into()]),
            s if s.contains(':') || s.contains('[') => add(vec![mctas::blockstate::BlockState::parse(s)?.to_string()]),
            s => bail!("unknown palette entry {s:?} (shorthands: core, stone, glass, obsidian, slime, honey, wool, rblock, dust, torch, repeater, repeater1, comparator, observer, piston, sticky, lamp, bulb, note, target, sculk, csculk, amethyst, hopper, dropper, dispenser, trapdoor, itrapdoor, leaves, scaffold, or a block state)"),
        }
    }
    // canonical text (the game's property order) through redsim, so rotations map onto entries
    let mut canon_out = Vec::new();
    for s in out {
        let b = Block::parse(&s).map_err(|e| anyhow::anyhow!("{e}"))?;
        let t = b.to_string();
        if !canon_out.contains(&t) {
            canon_out.push(t);
        }
    }
    Ok(canon_out)
}

/// A block that can delay or schedule anything: without one, every change settles in the same
/// tick and nothing can oscillate.
fn is_driver(b: &Block) -> bool {
    matches!(
        b,
        Block::Torch { .. }
            | Block::WallTorch { .. }
            | Block::Repeater { .. }
            | Block::Comparator { .. }
            | Block::Observer { .. }
            | Block::Piston { .. }
            | Block::Lamp { .. }
            | Block::Sculk { .. }
            | Block::Hopper { .. }
            | Block::Dispenser { .. }
            | Block::Crafter { .. }
            | Block::Leaves { .. }
            | Block::Scaffolding { .. }
            | Block::Button { .. }
            | Block::Falling { .. }
    )
}

/// Knobs of [`census`].
#[derive(Clone, Debug)]
pub struct CensusOptions {
    pub dims: [i32; 3],
    pub palette: Vec<String>,
    pub k: usize,
    /// Only fillings with at least this many blocks.
    pub kmin: usize,
    pub horizon: u32,
    pub margin: i32,
    /// Also try each block placed last (with updates) when the silent placement does not run.
    pub kick_last: bool,
    pub workers: usize,
    pub out: PathBuf,
}

/// One clock found.
#[derive(Clone, Debug)]
pub struct Found {
    pub period: u32,
    pub state_period: u32,
    pub warmup: u32,
    pub family: String,
    pub blocks: usize,
    pub dims: [i32; 3],
    pub on: u32,
    pub edges: u32,
    pub pulses: String,
    pub start: String,
    pub type_key: String,
    pub encoding: String,
    /// No single block can be removed with the clock keeping its period.
    pub minimal: bool,
}

const TSV_HEADER: &str = "period\tstate_period\twarmup\tfamily\tblocks\tdims\tvolume\ton\tedges\tpulses\tstart\ttype\tencoding\tminimal";

impl Found {
    fn tsv(&self) -> String {
        let [w, h, d] = self.dims;
        format!(
            "{}\t{}\t{}\t{}\t{}\t{w}x{h}x{d}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            self.period,
            self.state_period,
            self.warmup,
            self.family,
            self.blocks,
            w * h * d,
            self.on,
            self.edges,
            self.pulses,
            self.start,
            self.type_key,
            self.encoding,
            self.minimal
        )
    }
}

/// The census's cells (x fastest, then z, then y) and the subsets that touch the box's low faces.
fn subsets(dims: [i32; 3], kmin: usize, k: usize) -> Vec<Vec<usize>> {
    let n = (dims[0] * dims[1] * dims[2]) as usize;
    let pos = |i: usize| cell_pos(dims, i);
    let mut out = Vec::new();
    let mut cur: Vec<usize> = Vec::new();
    fn rec(n: usize, start: usize, k: usize, kmin: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>, ok: &dyn Fn(&[usize]) -> bool) {
        if cur.len() >= kmin.max(1) && ok(cur) {
            out.push(cur.clone());
        }
        if cur.len() == k {
            return;
        }
        for i in start..n {
            cur.push(i);
            rec(n, i + 1, k, kmin, cur, out, ok);
            cur.pop();
        }
    }
    let ok = |s: &[usize]| (0..3).all(|a| s.iter().any(|&i| pos(i)[a] == 0));
    rec(n, 0, k, kmin, &mut cur, &mut out, &ok);
    out
}

fn cell_pos(dims: [i32; 3], i: usize) -> Pos {
    let i = i as i32;
    [i % dims[0], i / (dims[0] * dims[2]), (i / dims[0]) % dims[2]]
}

/// The filling as sorted (pos, palette index) relative to its min corner, rotated `t` turns.
fn rotated_sorted(cells: &[(Pos, usize)], t: u8, rot: &[Vec<usize>]) -> Vec<(Pos, usize)> {
    let mut v: Vec<(Pos, usize)> = cells.iter().map(|(p, s)| (crate::rotate_pos(*p, t), rot[t as usize][*s])).collect();
    let lo = [0, 1, 2].map(|a| v.iter().map(|(p, _)| p[a]).min().unwrap());
    for (p, _) in v.iter_mut() {
        for a in 0..3 {
            p[a] -= lo[a];
        }
    }
    v.sort();
    v
}

fn fits(v: &[(Pos, usize)], dims: [i32; 3]) -> bool {
    v.iter().all(|(p, _)| (0..3).all(|a| p[a] < dims[a]))
}

/// Run the census (resumable): see the module doc. Returns the clocks found in this run's new
/// chunks plus the ones already in `parts/`.
pub fn census(o: &CensusOptions, progress: &(dyn Fn(usize, usize, usize) + Sync)) -> Result<Vec<Found>> {
    let pal = &o.palette;
    let blocks: Vec<Block> = pal.iter().map(|s| Block::parse(s).map_err(|e| anyhow::anyhow!("{e}"))).collect::<Result<_>>()?;
    // rotation tables over palette indices (the palette must be closed under rotation)
    let mut rot: Vec<Vec<usize>> = vec![(0..pal.len()).collect()];
    for t in 1..4u8 {
        let mut v = Vec::new();
        for s in pal {
            let r = Block::parse(&canon::rotate_state(s, t)).map_err(|e| anyhow::anyhow!("{e}"))?.to_string();
            match pal.iter().position(|q| *q == r) {
                Some(i) => v.push(i),
                None => bail!("the palette is not closed under rotation: {s} turned {t}x is {r}, not in it (use the shorthands)"),
            }
        }
        rot.push(v);
    }
    let subs = subsets(o.dims, o.kmin, o.k);
    // chunks: (subset index, palette index of the subset's first cell)
    let chunks: Vec<(usize, usize)> = (0..subs.len()).flat_map(|s| (0..pal.len()).map(move |p| (s, p))).collect();
    let parts = o.out.join("parts");
    std::fs::create_dir_all(&parts)?;
    // the parameters, so a resumed census is the same census
    let params = format!("dims {:?}\nk {}..={}\nhorizon {}\nmargin {}\nkick_last {}\npalette {}\n", o.dims, o.kmin, o.k, o.horizon, o.margin, o.kick_last, pal.join(" "));
    let pp = o.out.join("params.txt");
    if pp.exists() && std::fs::read_to_string(&pp)? != params {
        bail!("{} holds another census ({}): use another --out", o.out.display(), pp.display());
    }
    std::fs::write(&pp, &params)?;
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let found_n = AtomicUsize::new(0);
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..o.workers.max(1) {
            sc.spawn(|| {
                loop {
                    let ci = next.fetch_add(1, Ordering::Relaxed);
                    if ci >= chunks.len() {
                        break;
                    }
                    let (si, p0) = chunks[ci];
                    let part = parts.join(format!("{si:06}-{p0:03}.tsv"));
                    if part.exists() {
                        done.fetch_add(1, Ordering::Relaxed);
                        continue;
                    }
                    match run_chunk(o, &subs[si], p0, &blocks, &rot) {
                        Ok(rows) => {
                            found_n.fetch_add(rows.len(), Ordering::Relaxed);
                            let text: String = rows.iter().map(|r| r.tsv() + "\n").collect();
                            let tmp = part.with_extension("tmp");
                            if let Err(e) = std::fs::write(&tmp, text).and_then(|_| std::fs::rename(&tmp, &part)) {
                                errors.lock().unwrap().push(format!("{}: {e}", part.display()));
                            }
                        }
                        Err(e) => errors.lock().unwrap().push(format!("chunk {si}-{p0}: {e:#}")),
                    }
                    let d = done.fetch_add(1, Ordering::Relaxed) + 1;
                    progress(d, chunks.len(), found_n.load(Ordering::Relaxed));
                }
            });
        }
    });
    if let Some(e) = errors.lock().unwrap().first() {
        bail!("{e}");
    }
    read_parts(&parts)
}

fn read_parts(parts: &Path) -> Result<Vec<Found>> {
    let mut out = Vec::new();
    let mut names: Vec<PathBuf> = std::fs::read_dir(parts)?.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "tsv")).collect();
    names.sort();
    for p in names {
        for line in std::fs::read_to_string(&p)?.lines() {
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() < 13 {
                continue;
            }
            let dims: Vec<i32> = f[5].split('x').filter_map(|x| x.parse().ok()).collect();
            out.push(Found {
                period: f[0].parse()?,
                state_period: f[1].parse()?,
                warmup: f[2].parse()?,
                family: f[3].into(),
                blocks: f[4].parse()?,
                dims: [dims[0], dims[1], dims[2]],
                on: f[7].parse()?,
                edges: f[8].parse()?,
                pulses: f[9].into(),
                start: f[10].into(),
                type_key: f[11].into(),
                encoding: f[12].into(),
                minimal: f.get(13).is_some_and(|m| *m == "true"),
            });
        }
    }
    Ok(out)
}

fn run_chunk(o: &CensusOptions, sub: &[usize], p0: usize, blocks: &[Block], rot: &[Vec<usize>]) -> Result<Vec<Found>> {
    let k = sub.len();
    let n = blocks.len();
    let mut rows = Vec::new();
    let mut idx = vec![0usize; k];
    idx[0] = p0;
    let positions: Vec<Pos> = sub.iter().map(|&i| cell_pos(o.dims, i)).collect();
    loop {
        if let Some(f) = judge_filling(o, &positions, &idx, blocks, rot) {
            rows.push(f);
        }
        // odometer over idx[1..]
        let mut j = k;
        loop {
            if j == 1 {
                return Ok(rows);
            }
            j -= 1;
            idx[j] += 1;
            if idx[j] < n {
                break;
            }
            idx[j] = 0;
        }
        if k == 1 {
            return Ok(rows);
        }
    }
}

fn judge_filling(o: &CensusOptions, positions: &[Pos], idx: &[usize], blocks: &[Block], rot: &[Vec<usize>]) -> Option<Found> {
    if !idx.iter().any(|&s| is_driver(&blocks[s])) {
        return None;
    }
    let cells: Vec<(Pos, usize)> = positions.iter().copied().zip(idx.iter().copied()).collect();
    // one per rotation class: rotation 0 must be the smallest encoding among the rotations that fit
    let base = rotated_sorted(&cells, 0, rot);
    for t in 1..4u8 {
        let r = rotated_sorted(&cells, t, rot);
        if fits(&r, o.dims) && r < base {
            return None;
        }
    }
    let placed: Vec<(BlockPos, Block)> = cells.iter().map(|(p, s)| (BlockPos::new(p[0], p[1], p[2]), blocks[*s])).collect();
    let mut start: Option<Option<usize>> = None;
    if quick_period(&placed, None, o.margin, o.horizon).is_some() {
        start = Some(None);
    } else if o.kick_last {
        for i in 0..placed.len() {
            if quick_period(&placed, Some(i), o.margin, o.horizon).is_some() {
                start = Some(Some(i));
                break;
            }
        }
    }
    let start = start?;
    // the full judge on redsim (visible period, family, waveform, swept box)
    let mut d = Design { name: "census".into(), ..Default::default() };
    for (p, s) in &cells {
        d.blocks.insert(*p, blocks[*s].to_string());
    }
    if let Some(i) = start {
        let p = cells[i].0;
        d.ports.insert("start".into(), p);
        d.meta.insert("port start".into(), "last".into());
    }
    let c = Clock::from_design(d).ok()?;
    let r = simulate(&c, &SimOptions { horizon: o.horizon, margin: o.margin });
    if r.verdict != Verdict::Clock {
        return None;
    }
    let (on, edges, pulses) = r.output.as_ref().map(|w| (w.wave.on, w.wave.edges, format!("{:?}", w.wave.widths))).unwrap_or_default();
    let encoding = base.iter().map(|(p, s)| format!("{},{},{}={}", p[0], p[1], p[2], crate::short(&blocks[*s].to_string()))).collect::<Vec<_>>().join(";");
    // minimal: no single block can go with the clock keeping its whole-state period (else it is a
    // smaller clock plus an idle block)
    let minimal = placed.len() == 1
        || (0..placed.len()).all(|i| {
            let rest: Vec<(BlockPos, Block)> = placed.iter().enumerate().filter(|(j, _)| *j != i).map(|(_, b)| *b).collect();
            let starts: Vec<Option<usize>> = std::iter::once(None).chain((0..rest.len()).map(Some)).collect();
            !starts.into_iter().any(|s| quick_period(&rest, s, o.margin, o.horizon).is_some_and(|(_, p)| p == r.state_period))
        });
    Some(Found {
        period: r.period,
        state_period: r.state_period,
        warmup: r.warmup,
        family: wave::signature(&r.analysis.family),
        blocks: c.blocks(),
        dims: r.dims(),
        on,
        edges,
        pulses,
        start: match start {
            None => "self".into(),
            Some(i) => format!("last {},{},{}", cells[i].0[0], cells[i].0[1], cells[i].0[2]),
        },
        type_key: canon::type_key(c.design.blocks.iter()),
        encoding,
        minimal,
    })
}

/// Merge: `clocks.tsv` (every clock, sorted by period, family, blocks), a design file for the
/// smallest clock of each (period, family) in `designs/`, and `summary.md` (counts per period and
/// family). Returns (clocks, types, (period, family) pairs).
pub fn write_outputs(o: &CensusOptions, found: &[Found]) -> Result<(usize, usize, usize)> {
    let mut all = found.to_vec();
    all.sort_by(|a, b| (a.period, &a.family, a.blocks, a.dims[0] * a.dims[1] * a.dims[2], &a.encoding).cmp(&(b.period, &b.family, b.blocks, b.dims[0] * b.dims[1] * b.dims[2], &b.encoding)));
    let mut tsv = format!("{TSV_HEADER}\n");
    for f in &all {
        tsv.push_str(&f.tsv());
        tsv.push('\n');
    }
    std::fs::write(o.out.join("clocks.tsv"), tsv)?;
    let mut mt = format!("{TSV_HEADER}\n");
    for f in all.iter().filter(|f| f.minimal) {
        mt.push_str(&f.tsv());
        mt.push('\n');
    }
    std::fs::write(o.out.join("minimal.tsv"), mt)?;
    let types: BTreeSet<&str> = all.iter().map(|f| f.type_key.as_str()).collect();
    let mut best: BTreeMap<(u32, String), &Found> = BTreeMap::new();
    for f in &all {
        let key = (f.period, f.family.clone());
        let vol = |x: &Found| x.dims[0] * x.dims[1] * x.dims[2];
        if best.get(&key).is_none_or(|b| (f.blocks, vol(f)) < (b.blocks, vol(b))) {
            best.insert(key, f);
        }
    }
    let ddir = o.out.join("designs");
    std::fs::create_dir_all(&ddir)?;
    for ((p, fam), f) in &best {
        let mut d = Design { name: format!("census-p{p}-{}-{}b", fam.replace('+', "-"), f.blocks), ..Default::default() };
        d.meta.insert("census".into(), format!("box {}x{}x{} k<={} ({})", o.dims[0], o.dims[1], o.dims[2], o.k, f.encoding));
        for cell in f.encoding.split(';') {
            let (xyz, s) = cell.split_once('=').unwrap();
            let v: Vec<i32> = xyz.split(',').map(|x| x.parse().unwrap()).collect();
            d.blocks.insert([v[0], v[1], v[2]], format!("minecraft:{s}"));
        }
        if let Some(rest) = f.start.strip_prefix("last ") {
            let v: Vec<i32> = rest.split(',').map(|x| x.parse().unwrap()).collect();
            d.ports.insert("start".into(), [v[0], v[1], v[2]]);
            d.meta.insert("port start".into(), "last".into());
        }
        d.save(&ddir.join(format!("{}.txt", d.name)))?;
    }
    let periods: BTreeSet<u32> = all.iter().map(|f| f.period).collect();
    let fams: BTreeSet<&str> = all.iter().map(|f| f.family.as_str()).collect();
    let mut md = format!(
        "# Census {}x{}x{}, k <= {}\n\nGenerated by `mctas clock census` (redsim; self-start, then each block placed last). Palette ({} states): {}\n\n{} clocks, {} types ({} minimal: no block can go with the period kept), {} (period, family) pairs.\n\n| P | family | types | minimal types | smallest (blocks, box) | start |\n|---|---|---|---|---|---|\n",
        o.dims[0],
        o.dims[1],
        o.dims[2],
        o.k,
        o.palette.len(),
        o.palette.iter().map(|s| crate::short(s)).collect::<Vec<_>>().join(" "),
        all.len(),
        types.len(),
        all.iter().filter(|f| f.minimal).map(|f| f.type_key.as_str()).collect::<BTreeSet<_>>().len(),
        best.len()
    );
    for p in &periods {
        for fam in &fams {
            let n: BTreeSet<&str> = all.iter().filter(|f| f.period == *p && f.family == *fam).map(|f| f.type_key.as_str()).collect();
            let nm: BTreeSet<&str> = all.iter().filter(|f| f.period == *p && f.family == *fam && f.minimal).map(|f| f.type_key.as_str()).collect();
            if let Some(b) = best.get(&(*p, fam.to_string())) {
                md.push_str(&format!("| {p} | {fam} | {} | {} | {}, {}x{}x{} | {} |\n", n.len(), nm.len(), b.blocks, b.dims[0], b.dims[1], b.dims[2], b.start));
            }
        }
    }
    std::fs::write(o.out.join("summary.md"), md)?;
    Ok((all.len(), types.len(), best.len()))
}

/// `clock census-report`: the minimal clocks of several census runs merged (each run's
/// `minimal.tsv` and `params.txt`): per (period, family) the fewest blocks seen, its box and
/// census, and how many minimal types; the periods found and the ones never found below the
/// largest. Markdown.
pub fn report(dirs: &[PathBuf]) -> Result<String> {
    struct Best {
        blocks: usize,
        dims: [i32; 3],
        census: String,
        encoding: String,
        start: String,
    }
    let mut best: BTreeMap<(u32, String), Best> = BTreeMap::new();
    let mut types: BTreeMap<(u32, String), BTreeSet<String>> = BTreeMap::new();
    let mut runs: Vec<String> = Vec::new();
    for d in dirs {
        let name = d.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let params = std::fs::read_to_string(d.join("params.txt")).unwrap_or_default();
        let kline = params.lines().find(|l| l.starts_with("k ")).unwrap_or("k ?").to_string();
        let dline = params.lines().find(|l| l.starts_with("dims ")).unwrap_or("dims ?").to_string();
        let found = read_parts(&d.join("parts"))?;
        let minimal: Vec<&Found> = found.iter().filter(|f| f.minimal).collect();
        runs.push(format!("| {name} | {} | {} | {} | {} |", dline.trim_start_matches("dims "), kline.trim_start_matches("k "), found.len(), minimal.len()));
        for f in minimal {
            let key = (f.period, f.family.clone());
            types.entry(key.clone()).or_default().insert(f.type_key.clone());
            let better = best.get(&key).is_none_or(|b| (f.blocks, f.dims[0] * f.dims[1] * f.dims[2]) < (b.blocks, b.dims[0] * b.dims[1] * b.dims[2]));
            if better {
                best.insert(key, Best { blocks: f.blocks, dims: f.dims, census: name.clone(), encoding: f.encoding.clone(), start: f.start.clone() });
            }
        }
    }
    let periods: BTreeSet<u32> = best.keys().map(|(p, _)| *p).collect();
    let max_p = periods.iter().max().copied().unwrap_or(0);
    let missing: Vec<String> = (1..max_p).filter(|p| !periods.contains(p)).map(|p| p.to_string()).collect();
    let mut md = String::from("# Census report: every minimal clock found by the exhaustive censuses\n\nGenerated by `mctas clock census-report` (docs/clocks.md#census-results): redsim, `core` palette, self-start or one block placed last; MINIMAL = no single block can go with the period kept. Composite designs (independent sub-clocks) are rejected by the judge.\n\n| census | box | k | clocks | minimal |\n|---|---|---|---|---|\n");
    md.push_str(&runs.join("\n"));
    md.push_str(&format!(
        "\n\nPeriods found: {}. Periods below {max_p} never found (in these boxes, with these blocks): {}.\n\n| P | family | minimal types | fewest blocks | box | census | start | blocks (x,y,z=state) |\n|---|---|---|---|---|---|---|---|\n",
        periods.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "),
        if missing.is_empty() { "none".to_string() } else { missing.join(", ") }
    ));
    for ((p, fam), b) in &best {
        md.push_str(&format!(
            "| {p} | {fam} | {} | {} | {}x{}x{} | {} | {} | `{}` |\n",
            types[&(*p, fam.clone())].len(),
            b.blocks,
            b.dims[0],
            b.dims[1],
            b.dims[2],
            b.census,
            b.start,
            b.encoding
        ));
    }
    Ok(md)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsets_touch_the_low_faces() {
        let s = subsets([2, 1, 1], 1, 2);
        // {0}, {0,1}: {1} alone does not touch x=0
        assert_eq!(s, vec![vec![0], vec![0, 1]]);
        assert_eq!(cell_pos([3, 2, 2], 5), [2, 0, 1]);
        assert_eq!(cell_pos([3, 2, 2], 6), [0, 1, 0]);
    }

    #[test]
    fn palette_is_closed() {
        let p = expand_palette("core").unwrap();
        assert!(p.len() > 50);
        for s in &p {
            for t in 1..4 {
                let r = Block::parse(&canon::rotate_state(s, t)).unwrap().to_string();
                assert!(p.contains(&r), "{s} turned {t}: {r}");
            }
        }
    }
}
