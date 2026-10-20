//! `mctas circuit compute census`: every filling of a box around fixed input cells, screened on
//! redsim with the study 12 judge's stimulus (`circuit::compute::stimulus`) and ideal outputs
//! (`circuit::compute::ideal`). Every candidate output cell (the box and a ring around it) is read
//! at once; an output passes in a cell when, at the end of every hold, the cell's level gives the
//! ideal value and it held it since it settled; the outputs need distinct cells. A first pass runs
//! the Gray-code section only (every state once); survivors run the whole stimulus. The smallest
//! design (blocks, then latency) and the fastest (latency, then blocks) are written as design
//! files for the real judge (`mctas circuit verify`).

use anyhow::{Context, Result, bail};
use circuit::Circuit;
use circuit::check::Sizing;
use circuit::compute::Spec;
use circuit::stimulus::Schedule;
use circuit_delay::census::{self, Space};
use circuit_delay::harness::{self, Change, InputKind, Pos, bp};
use redsim::Block;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(clap::Args, Debug)]
pub struct CensusArgs {
    /// The kind (`mux 1`, `decoder 2`, `encoder 2 priority`...).
    #[arg(long)]
    pub kind: String,
    /// Box AxBxC (x, y, z), cells 0..A etc.
    #[arg(long, default_value = "3x2x3")]
    pub dims: String,
    /// Input cells `name=x,y,z;name=x,y,z` (the kind's input names: s0, d0, d1...).
    #[arg(long)]
    pub inputs: String,
    /// Track delay's palette shorthands: stone, rblock, dust, torch, rep, rep4, cmp, obs, lamp,
    /// bulb, note, target, or block states, comma-separated.
    #[arg(long, default_value = "stone,dust,torch,rep,cmp")]
    pub palette: String,
    #[arg(long, default_value_t = 1)]
    pub kmin: usize,
    #[arg(long, default_value_t = 4)]
    pub k: usize,
    #[arg(long, default_value_t = 6)]
    pub threads: usize,
    /// The gt each state is held.
    #[arg(long, default_value_t = 16)]
    pub hold: u32,
    /// Write the designs here.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    /// The written designs' name prefix.
    #[arg(long, default_value = "census")]
    pub name: String,
    /// Blocks every filling keeps (not counted in k): `x,y,z=<state>;...` (redsim block states).
    #[arg(long)]
    pub fixed: Option<String>,
    /// The palette of the floor layer (y = 0) when it differs (`stone`: floors only).
    #[arg(long)]
    pub floor: Option<String>,
}

fn parse_pos(s: &str) -> Result<Pos> {
    let v: Vec<i32> = s.split(',').map(|x| x.trim().parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("position {s:?}"))?;
    if v.len() != 3 {
        bail!("position {s:?}: want x,y,z");
    }
    Ok([v[0], v[1], v[2]])
}

/// The judge's view of the kind: a template circuit with the inputs at their cells and the
/// outputs at far placeholder cells (only their names matter for the stimulus and the ideal).
fn template(kind: &str, inputs: &[(String, Pos)]) -> Result<(Circuit, Spec)> {
    let words: Vec<&str> = kind.split_whitespace().collect();
    let spec = Spec::parse(&words).ok_or_else(|| anyhow::anyhow!("{kind}: not a study 12 kind"))??;
    let mut text = format!("name: census-template\ncircuit: {kind}\norigin: 0 0 0\n");
    for (n, p) in inputs {
        text.push_str(&format!("port input.{n}: {} {} {}\n", p[0], p[1], p[2]));
    }
    // the outputs the kind needs, as the judge names them: try the required set
    for (k, n) in output_names(&spec).iter().enumerate() {
        text.push_str(&format!("port output.{n}: {} 100 {}\n", k * 2, 0));
    }
    text.push_str("palette:\n  S minecraft:stone\nlayer 99:\n  S\n");
    let d = mctas::design::Design::parse(&text)?;
    Ok((Circuit::from_design(d)?, spec))
}

/// The outputs the census places: the kind's required outputs (comparators: eq, lt, gt).
fn output_names(spec: &Spec) -> Vec<String> {
    use circuit::compute::Op;
    let n = spec.n;
    let w = 1u32 << n;
    match spec.op {
        Op::Adder | Op::Incrementer => (0..n).map(|k| format!("s{k}")).chain(std::iter::once("cout".to_string())).collect(),
        Op::Subtractor => (0..n).map(|k| format!("d{k}")).chain(std::iter::once("bout".to_string())).collect(),
        Op::Comparator => vec!["eq".into(), "lt".into(), "gt".into()],
        Op::Mux => vec!["q".into()],
        Op::Demux | Op::Decoder => (0..w).map(|k| format!("q{k}")).collect(),
        Op::Encoder | Op::PriorityEncoder | Op::BinaryCounter => (0..n).map(|k| format!("q{k}")).collect(),
    }
}

/// The schedule's input changes for the harness (input order = the template's).
fn changes(s: &Schedule) -> Vec<Change> {
    let mut v: Vec<Change> = Vec::new();
    for (i, e) in s.edges.iter().enumerate() {
        for &(t, on) in e {
            v.push(Change { at: t, input: i, on });
        }
    }
    v.sort_by_key(|c| c.at);
    v
}

/// Per output (template order): for each candidate cell that matches it over `phases`, its worst
/// latency.
fn matches(tpl: &Circuit, spec: &Spec, s: &Schedule, phases: usize, levels: &[Vec<i32>], cells: &[Pos]) -> Vec<Vec<(usize, u32)>> {
    let outs = tpl.outputs.len();
    let rest_from = (s.phases.first().map(|p| p.0).unwrap_or(0) / 2) as usize;
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(0);
    let ideals: Vec<(u32, u32, Vec<u8>)> = s.phases.iter().take(phases).map(|(a, b, l)| (*a, *b, circuit::compute::ideal(tpl, spec, l.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0)))).collect();
    let e0 = circuit::compute::ideal(tpl, spec, 0);
    let mut out: Vec<Vec<(usize, u32)>> = vec![Vec::new(); outs];
    for (ci, lv) in levels.iter().enumerate() {
        let _ = cells;
        for o in 0..outs {
            let on = |f: usize| (lv.get(f).copied().unwrap_or(0) > 0) as u8;
            if (rest_from..first).any(|f| on(f) != e0[o]) {
                continue;
            }
            let mut worst = 0u32;
            let mut ok = true;
            for (a, b, e) in &ideals {
                let (a, b) = (*a as usize, *b as usize);
                if b == 0 || on(b - 1) != e[o] {
                    ok = false;
                    break;
                }
                let mut f0 = a;
                for f in (a..b).rev() {
                    if on(f) != e[o] {
                        f0 = f + 1;
                        break;
                    }
                }
                worst = worst.max((f0 - a) as u32);
            }
            if ok {
                out[o].push((ci, worst));
            }
        }
    }
    out
}

/// Distinct cells for every output, minimizing the worst latency (small backtracking).
fn assign(per: &[Vec<(usize, u32)>]) -> Option<(Vec<usize>, u32)> {
    fn rec(per: &[Vec<(usize, u32)>], o: usize, used: &mut Vec<usize>, worst: u32, best: &mut Option<(Vec<usize>, u32)>) {
        if best.as_ref().is_some_and(|b| worst >= b.1) && o > 0 {
            return;
        }
        if o == per.len() {
            *best = Some((used.clone(), worst));
            return;
        }
        let mut cands = per[o].clone();
        cands.sort_by_key(|c| c.1);
        for (ci, l) in cands {
            if used.contains(&ci) {
                continue;
            }
            used.push(ci);
            rec(per, o + 1, used, worst.max(l), best);
            used.pop();
        }
    }
    let mut best = None;
    rec(per, 0, &mut Vec::new(), 0, &mut best);
    best
}

pub fn run(a: CensusArgs) -> Result<()> {
    let dims: Vec<i32> = a.dims.split('x').map(|x| x.parse::<i32>()).collect::<std::result::Result<_, _>>().with_context(|| format!("--dims {:?}", a.dims))?;
    if dims.len() != 3 {
        bail!("--dims AxBxC");
    }
    let mut inputs: Vec<(String, Pos)> = Vec::new();
    for part in a.inputs.split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (n, p) = part.split_once('=').ok_or_else(|| anyhow::anyhow!("--inputs {part:?}: want name=x,y,z"))?;
        inputs.push((n.trim().to_string(), parse_pos(p)?));
    }
    let (tpl, spec) = template(&a.kind, &inputs)?;
    // the harness inputs in the template's order (sorted by name)
    let hin: Vec<(redsim::BlockPos, InputKind)> = tpl.inputs.iter().map(|i| (bp(i.pos), InputKind::Place { on: Block::RedstoneBlock, off: Block::Air })).collect();
    let full = circuit::compute::stimulus(&tpl, spec, &Sizing { gap: a.hold });
    let ch_full = changes(&full);
    let gray_n = full.phases.iter().take_while(|p| p.2.starts_with("gray")).count().max(1);
    let gray_end = full.phases.get(gray_n - 1).map(|p| p.1).unwrap_or(full.ticks);
    let ch_gray: Vec<Change> = ch_full.iter().copied().filter(|c| c.at < gray_end).collect();
    let mut fixed: Vec<(Pos, Block)> = Vec::new();
    for part in a.fixed.as_deref().unwrap_or("").split(';').map(str::trim).filter(|p| !p.is_empty()) {
        let (p, s) = part.split_once('=').ok_or_else(|| anyhow::anyhow!("--fixed {part:?}: want x,y,z=<state>"))?;
        let b = Block::parse(s.trim()).map_err(|e| anyhow::anyhow!("--fixed {s}: {e}"))?;
        fixed.push((parse_pos(p)?, b));
    }
    let sp = Space { dims: [dims[0], dims[1], dims[2]], inputs: tpl.inputs.iter().map(|i| i.pos).collect(), palette: census::palette(&a.palette)?, kmin: a.kmin, k: a.k, fixed };
    let cells = sp.output_cells();
    let outs_bp: Vec<redsim::BlockPos> = cells.iter().map(|p| bp(*p)).collect();
    let bounds = sp.bounds();
    eprintln!("census {}: {} inputs, {} holds ({} gray), {} gt per run, {} candidate output cells", a.kind, tpl.inputs.len(), full.phases.len(), gray_n, full.ticks, cells.len());
    let floor = match &a.floor {
        Some(f) => Some(census::palette(f)?),
        None => None,
    };
    let best = search(&sp, floor.as_deref(), a.threads, |blocks: &[(Pos, Block)]| {
        let bl: Vec<(redsim::BlockPos, Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
        // pass 1: the Gray code (every state once)
        let Ok((lv, _)) = harness::run_raw_multi(&bl, &hin, &outs_bp, bounds, &ch_gray, gray_end, 2) else { return Vec::new() };
        let per = matches(&tpl, &spec, &full, gray_n, &lv, &cells);
        if per.iter().any(|v| v.is_empty()) || assign(&per).is_none() {
            return Vec::new();
        }
        // pass 2: the whole stimulus
        let Ok((lv, _)) = harness::run_raw_multi(&bl, &hin, &outs_bp, bounds, &ch_full, full.ticks + 1, 2) else { return Vec::new() };
        let per = matches(&tpl, &spec, &full, full.phases.len(), &lv, &cells);
        let Some((cells_idx, lat)) = assign(&per) else { return Vec::new() };
        let note = tpl.outputs.iter().zip(&cells_idx).map(|(o, ci)| format!("{}={},{},{}", o.name, cells[*ci][0], cells[*ci][1], cells[*ci][2])).collect::<Vec<_>>().join(";");
        let n = blocks.len() as u32;
        vec![("smallest".to_string(), (n, lat), note.clone(), cells[cells_idx[0]]), ("fastest".to_string(), (lat, n), note, cells[cells_idx[0]])]
    });
    if best.is_empty() {
        println!("census {}: nothing passes", a.kind);
        return Ok(());
    }
    for (key, hit) in &best {
        println!("{key}: {} blocks, score {:?}, outputs {}", hit.blocks.len(), hit.score, hit.note);
        if let Some(dir) = &a.out_dir {
            std::fs::create_dir_all(dir)?;
            let name = format!("{}-{key}", a.name);
            std::fs::write(dir.join(format!("{name}.txt")), design_text(&name, &a.kind, &inputs, &hit.note, &hit.blocks))?;
        }
    }
    Ok(())
}

/// Track delay's search (`circuit_delay::census::search`) with a separate palette for the floor
/// layer (y = 0): every k-subset of the free box cells, every assignment, the unsupported skipped.
fn search<F>(sp: &Space, floor: Option<&[Block]>, threads: usize, eval: F) -> BTreeMap<String, census::Hit>
where
    F: Fn(&[(Pos, Block)]) -> Vec<(String, (u32, u32), String, Pos)> + Sync,
{
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let mut cells: Vec<Pos> = Vec::new();
    for y in 0..sp.dims[1] {
        for z in 0..sp.dims[2] {
            for x in 0..sp.dims[0] {
                let p = [x, y, z];
                if !sp.inputs.contains(&p) && !sp.fixed.iter().any(|(q, _)| *q == p) {
                    cells.push(p);
                }
            }
        }
    }
    let pal = |p: &Pos| -> &[Block] {
        match floor {
            Some(f) if p[1] == 0 => f,
            _ => &sp.palette,
        }
    };
    let mut jobs: Vec<Vec<usize>> = Vec::new();
    for k in sp.kmin..=sp.k {
        subsets(cells.len(), k, &mut jobs);
    }
    let best: Mutex<BTreeMap<String, census::Hit>> = Mutex::new(BTreeMap::new());
    let next = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    let total = jobs.len();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let j = next.fetch_add(1, Ordering::Relaxed);
                    if j >= jobs.len() {
                        break;
                    }
                    let pos: Vec<Pos> = jobs[j].iter().map(|&i| cells[i]).collect();
                    let k = pos.len();
                    let pals: Vec<&[Block]> = pos.iter().map(|p| pal(p)).collect();
                    let mut idx = vec![0usize; k];
                    'assign: loop {
                        let blocks: Vec<(Pos, Block)> = sp.fixed.iter().copied().chain(pos.iter().zip(&idx).map(|(q, &i)| (*q, pals_at(&pals, q, &pos, i)))).collect();
                        if supported(&blocks, &sp.inputs) {
                            for (key, score, note, output) in eval(&blocks) {
                                let mut b = best.lock().unwrap();
                                if b.get(&key).is_none_or(|h| score < h.score) {
                                    b.insert(key, census::Hit { blocks: blocks.clone(), output, score, note });
                                }
                            }
                        }
                        let mut i = 0;
                        loop {
                            if i == k {
                                break 'assign;
                            }
                            idx[i] += 1;
                            if idx[i] < pals[i].len() {
                                break;
                            }
                            idx[i] = 0;
                            i += 1;
                        }
                    }
                    if j % 2000 == 0 {
                        eprintln!("  {j}/{total} cell sets, {:.0} s", t0.elapsed().as_secs_f64());
                    }
                }
            });
        }
    });
    best.into_inner().unwrap()
}

fn pals_at(pals: &[&[Block]], q: &Pos, pos: &[Pos], i: usize) -> Block {
    let k = pos.iter().position(|p| p == q).unwrap_or(0);
    pals[k][i]
}

fn subsets(n: usize, k: usize, out: &mut Vec<Vec<usize>>) {
    fn rec(start: usize, n: usize, k: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            rec(i + 1, n, k, cur, out);
            cur.pop();
        }
    }
    rec(0, n, k, &mut Vec::new(), out);
}

/// Floor components stand on a solid block, wall torches hang on one, every block touches
/// (26-neighborhood) another block or an input (track delay's rule).
fn supported(blocks: &[(Pos, Block)], inputs: &[Pos]) -> bool {
    let at = |p: Pos| blocks.iter().find(|(q, _)| *q == p).map(|(_, b)| *b);
    let near = |a: Pos, b: Pos| (a[0] - b[0]).abs() <= 1 && (a[1] - b[1]).abs() <= 1 && (a[2] - b[2]).abs() <= 1;
    for (i, (p, b)) in blocks.iter().enumerate() {
        let floor_needed = matches!(b, Block::Wire { .. } | Block::Repeater { .. } | Block::Comparator { .. } | Block::Torch { .. });
        if floor_needed && !at([p[0], p[1] - 1, p[2]]).is_some_and(|f| census::solid(&f)) {
            return false;
        }
        if let Block::WallTorch { facing, .. } = b {
            let v = match facing {
                redsim::Dir::North => (0, 0, -1),
                redsim::Dir::South => (0, 0, 1),
                redsim::Dir::East => (1, 0, 0),
                redsim::Dir::West => (-1, 0, 0),
                redsim::Dir::Up => (0, 1, 0),
                redsim::Dir::Down => (0, -1, 0),
            };
            if !at([p[0] - v.0, p[1] - v.1, p[2] - v.2]).is_some_and(|f| census::solid(&f)) {
                return false;
            }
        }
        if !inputs.iter().any(|q| near(*p, *q)) && !blocks.iter().enumerate().any(|(j, (q, _))| j != i && near(*p, *q)) {
            return false;
        }
    }
    true
}

/// A design file for a census hit.
fn design_text(name: &str, kind: &str, inputs: &[(String, Pos)], outputs: &str, blocks: &[(Pos, Block)]) -> String {
    let mut s = format!("name: {name}\ncircuit: {kind}\norigin: 0 0 0\n");
    for (n, p) in inputs {
        s.push_str(&format!("port input.{n}: {} {} {}\n", p[0], p[1], p[2]));
    }
    for part in outputs.split(';') {
        if let Some((n, p)) = part.split_once('=') {
            s.push_str(&format!("port output.{n}: {}\n", p.replace(',', " ")));
        }
    }
    let mut d = mctas::design::Design::parse(&s).expect("ports");
    for (p, b) in blocks {
        d.blocks.insert(*p, b.to_string());
    }
    let _ = BTreeMap::<u8, u8>::new();
    d.to_text()
}
