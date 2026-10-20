//! `mctas circuit pulsemult search` / `improve`: random and local searches for study 11's kinds
//! on redsim. A candidate is the seed design plus blocks from the palette; it must stand (every
//! dust, diode and torch on its support, redsim's `can_survive`), pass a quick screen (one 40 gt
//! input pulse: off at rest, an output, back off; a short clock for dividers), then the judge's own
//! redsim check (`circuit::verify::sim`). Passes are written once per block set.

use anyhow::{Result, bail};
use circuit::{Circuit, Kind};
use circuit::stimulus::Schedule;
use mctas::design::Design;
use redsim::{Block, BlockPos, World};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

pub type Pos = [i32; 3];

#[derive(Clone, Debug)]
pub struct SearchOptions {
    pub seed: PathBuf,
    pub lo: Pos,
    pub hi: Pos,
    pub palette: Vec<String>,
    pub k: usize,
    pub kmin: usize,
    pub require: Vec<String>,
    pub samples: usize,
    pub rng: u64,
    pub workers: usize,
    pub max_found: usize,
    pub out: PathBuf,
}

const DIRS4: [&str; 4] = ["north", "east", "south", "west"];
const DIRS6: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

/// A palette shorthand's block states.
pub fn expand(name: &str) -> Result<Vec<String>> {
    let rep = |d: u32| DIRS4.iter().map(move |f| format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]"));
    Ok(match name {
        "stone" => vec!["minecraft:stone".into()],
        "dust" => vec!["minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into()],
        "repeater" => (1..=4).flat_map(rep).collect(),
        "repeater1" => rep(1).collect(),
        "repeater2" => rep(2).collect(),
        "repeater3" => rep(3).collect(),
        "repeater4" => rep(4).collect(),
        "comparator" => DIRS4.iter().flat_map(|f| ["compare", "subtract"].map(|m| format!("minecraft:comparator[facing={f},mode={m},powered=false]"))).collect(),
        "torch" => std::iter::once("minecraft:redstone_torch[lit=true]".to_string()).chain(DIRS4.iter().map(|f| format!("minecraft:redstone_wall_torch[facing={f},lit=true]"))).collect(),
        "observer" => DIRS6.iter().map(|f| format!("minecraft:observer[facing={f},powered=false]")).collect(),
        "piston" => DIRS6.iter().map(|f| format!("minecraft:piston[extended=false,facing={f}]")).collect(),
        "sticky" => DIRS6.iter().map(|f| format!("minecraft:sticky_piston[extended=false,facing={f}]")).collect(),
        "rblock" => vec!["minecraft:redstone_block".into()],
        "bulb" => vec!["minecraft:waxed_copper_bulb[lit=false,powered=false]".into()],
        "slime" => vec!["minecraft:slime_block".into()],
        s if s.contains(':') || s.contains('[') => vec![if s.contains(':') { s.to_string() } else { format!("minecraft:{s}") }],
        _ => bail!("unknown palette entry {name:?}"),
    })
}

/// The shorthand a state belongs to (for `--require`).
fn group(state: &str) -> &'static str {
    let id = state.trim_start_matches("minecraft:");
    for (p, g) in [
        ("redstone_wire", "dust"),
        ("repeater", "repeater"),
        ("comparator", "comparator"),
        ("redstone_torch", "torch"),
        ("redstone_wall_torch", "torch"),
        ("observer", "observer"),
        ("sticky_piston", "sticky"),
        ("piston", "piston"),
        ("redstone_block", "rblock"),
        ("waxed_copper_bulb", "bulb"),
        ("slime_block", "slime"),
        ("stone", "stone"),
    ] {
        if id.starts_with(p) {
            return g;
        }
    }
    "other"
}

pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Every block stands (redsim's `can_survive` with the others placed).
pub fn standing(blocks: &BTreeMap<Pos, String>) -> bool {
    let Some(lo) = blocks.keys().copied().reduce(|a, b| [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]) else { return true };
    let hi = blocks.keys().copied().reduce(|a, b| [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]).unwrap();
    let mut placed: Vec<(BlockPos, Block)> = Vec::new();
    for (p, s) in blocks {
        let Ok(b) = Block::parse(s) else { return false };
        placed.push((BlockPos::new(p[0], p[1], p[2]), b));
    }
    let mut w = World::new(BlockPos::new(lo[0] - 2, lo[1] - 2, lo[2] - 2), [hi[0] - lo[0] + 5, hi[1] - lo[1] + 5, hi[2] - lo[2] + 5]);
    w.place_raw(&placed);
    placed.iter().all(|(p, b)| redsim::behavior::can_survive(&w, b, *p))
}

fn rises(v: &[bool]) -> usize {
    (1..v.len()).filter(|&i| v[i] && !v[i - 1]).count()
}

/// A full block a dust, diode or torch can stand on or hang from.
fn full_state(s: &str) -> bool {
    let id = s.trim_start_matches("minecraft:");
    ["stone", "observer", "redstone_block", "waxed_copper_bulb", "slime_block", "piston", "sticky_piston"].iter().any(|p| id.starts_with(p))
}

/// The cell a block needs a full block in (below a dust, diode or standing torch; behind a wall
/// torch), if any.
fn support_cell(p: Pos, s: &str) -> Option<Pos> {
    let id = s.trim_start_matches("minecraft:");
    if id.starts_with("redstone_wall_torch") {
        let f = id.split("facing=").nth(1)?.split([',', ']']).next()?;
        let d = match f {
            "north" => [0, 0, 1],
            "south" => [0, 0, -1],
            "east" => [-1, 0, 0],
            _ => [1, 0, 0],
        };
        return Some([p[0] + d[0], p[1], p[2] + d[2]]);
    }
    ["redstone_wire", "repeater", "comparator", "redstone_torch"].iter().any(|q| id.starts_with(q)).then_some([p[0], p[1] - 1, p[2]])
}

/// Add `s` at `p` with the stone it needs (when its support cell is free and allowed); false
/// when it cannot stand.
fn place_supported(blocks: &mut BTreeMap<Pos, String>, p: Pos, s: &str, allowed: &dyn Fn(Pos) -> bool) -> bool {
    if let Some(q) = support_cell(p, s) {
        match blocks.get(&q) {
            Some(b) if full_state(b) => {}
            Some(_) => return false,
            None if allowed(q) => {
                blocks.insert(q, "minecraft:stone".into());
            }
            None => return false,
        }
    }
    blocks.insert(p, s.to_string());
    true
}

/// The quick screen: one 40 gt input pulse (a 6 x 8 gt clock for dividers) on redsim.
pub fn screen(c: &Circuit) -> bool {
    let Some(kind) = &c.kind else { return false };
    let st = c.clock.free_from() + 40;
    let mut s = Schedule::new(c.inputs.len());
    if matches!(kind, Kind::Divider(_)) {
        for k in 0..12 {
            s.pulse(0, st + k * 8, 4);
        }
        s.ticks = st + 12 * 8 + 40;
    } else {
        s.pulse(0, st, 40);
        s.ticks = st + 140;
    }
    let Ok(t) = circuit::run::sim_trace(c, &s, 2) else { return false };
    let Some(o) = c.outputs.first() else { return false };
    let k = c.outputs.iter().position(|x| x.name == o.name).unwrap_or(0);
    let out = t.out_seq(k);
    let st = st as usize;
    if out.len() < s.ticks as usize {
        return false;
    }
    let after = &out[st..];
    match kind {
        Kind::Divider(n) => {
            let r = rises(after);
            r >= 1 && r <= 12 / *n as usize + 1
        }
        _ => {
            if out[st - 20..st].iter().any(|x| *x) || *out.last().unwrap() {
                return false;
            }
            let r = rises(after);
            match kind {
                Kind::Burst(n) => r == *n as usize,
                Kind::PulseTrain => r >= 2,
                _ => r >= 1,
            }
        }
    }
}

/// The judge's redsim verdict: Some((summary, metrics)) when it passes.
pub fn judge(c: &Circuit) -> Option<(String, Vec<(String, String)>)> {
    let rep = circuit::verify::sim(c, &circuit::verify::Options::default()).ok()?;
    let out = rep.outcome?;
    if !out.pass {
        return None;
    }
    let kind = c.kind.as_ref()?.name();
    let m: Vec<String> = out.metrics.iter().map(|(a, b)| format!("{a} {b}")).collect();
    let flags = if out.why.is_empty() { String::new() } else { format!(" ({})", out.why) };
    Some((format!("{kind} {}{flags}", m.join(", ")), out.metrics))
}

/// The axis value a local search keeps (N; L and class; T; P).
pub fn axis_key(kind: &Kind, metrics: &[(String, String)]) -> String {
    let m = |n: &str| metrics.iter().find(|(a, _)| a == n).map(|(_, b)| b.clone()).unwrap_or_default();
    match kind {
        Kind::Burst(n) | Kind::Divider(n) => n.to_string(),
        Kind::Monostable(_) => format!("L {} {}", m("L"), m("retrigger")),
        Kind::OffDelay => format!("L {}", m("L")),
        Kind::OnDelay => format!("T {}", m("T")),
        Kind::PulseTrain => format!("P {}", m("P")),
        _ => String::new(),
    }
}

/// A file-name stem for a pass: kind and axis value.
fn slug(kind: &Kind, metrics: &[(String, String)]) -> String {
    let m = |n: &str| metrics.iter().find(|(a, _)| a == n).map(|(_, b)| b.trim_end_matches(" gt").to_string()).unwrap_or_default();
    match kind {
        Kind::Burst(n) => format!("burst{n}"),
        Kind::Divider(n) => format!("div{n}"),
        Kind::Monostable(_) => format!("mono-L{}-{}", m("L"), if m("retrigger") == "retriggerable" { "r" } else { "nr" }),
        Kind::OffDelay => format!("offdelay-L{}", m("L")),
        Kind::OnDelay => format!("ondelay-T{}", m("T")),
        Kind::PulseTrain => format!("train-P{}", m("P")),
        _ => "x".into(),
    }
}

/// `circuit pulsemult square-divider`: study 5's ring of N + one bulb, fed on both input edges
/// (see `Cmd::SquareDivider`); the first observer placement whose cells are free wins.
pub fn square_divider(n: usize, max_dust: usize, out: &Path) -> Result<String> {
    let rings = circuit_memory::ring::search(n, max_dust, 1, 1);
    let Some(mut r) = rings.into_iter().next() else { bail!("no ring of {n} with at most {max_dust} dust") };
    r.divider = true;
    let name = format!("pm-div{n}-square");
    let mut d = r.build(&name);
    let input = d.ports.get("input").copied().unwrap_or([0, 0, 0]);
    let near = |d: &Design, p: Pos, but: Pos| DIRS.iter().any(|s| {
        let q = [p[0] + s[0], p[1] + s[1], p[2] + s[2]];
        q != but && d.blocks.contains_key(&q)
    });
    let mut placed = false;
    for s in DIRS {
        let obs = [input[0] + s[0], input[1] + s[1], input[2] + s[2]];
        let cell = [input[0] + 2 * s[0], input[1] + 2 * s[1], input[2] + 2 * s[2]];
        if d.blocks.contains_key(&obs) || d.blocks.contains_key(&cell) || near(&d, cell, obs) || d.ports.values().any(|p| *p == obs || *p == cell) {
            continue;
        }
        let face = match s {
            [1, 0, 0] => "east",
            [-1, 0, 0] => "west",
            [0, 1, 0] => "up",
            [0, -1, 0] => "down",
            [0, 0, 1] => "south",
            _ => "north",
        };
        d.blocks.insert(input, "minecraft:stone".into());
        d.blocks.insert(obs, format!("minecraft:observer[facing={face},powered=false]"));
        d.ports.insert("input".into(), cell);
        placed = true;
        break;
    }
    if !placed {
        bail!("ring of {n}: no free cell for the observer");
    }
    d.meta.insert("circuit".into(), format!("divider {n}"));
    d.meta.insert("group".into(), "pm-square-divider".into());
    d.meta.insert("group title".into(), "Square /N divider (dropper ring + bulb, fed on both edges)".into());
    d.meta.insert(
        "group note".into(),
        "study 5's dropper ring of N with one copper bulb toggles once every N pulses; an observer feeds it on BOTH input edges, so the bulb toggles every N/2 input periods: a 50% duty for odd N too (`mctas circuit pulsemult square-divider --n N`)".into(),
    );
    let c = Circuit::from_design(d.clone())?;
    let line = match judge(&c) {
        Some((summary, _)) => summary,
        None => bail!("ring of {n}: the square divider does not pass on redsim"),
    };
    let p = out.join(format!("{name}.txt"));
    d.save(&p)?;
    Ok(format!("{} blocks: {line} -> {}", d.blocks.len(), p.display()))
}

const DIRS: [Pos; 6] = [[0, -1, 0], [0, 0, 1], [0, 0, -1], [1, 0, 0], [-1, 0, 0], [0, 1, 0]];

/// Seed cells that must stay empty: the ports (the judge's input block, its lamps).
fn port_cells(d: &Design) -> Vec<Pos> {
    d.ports.values().copied().collect()
}

/// `circuit pulsemult as`: one design judged on redsim under `kind` (see `Cmd::As`).
pub fn judge_as(path: &Path, kind: &str) -> Result<String> {
    let mut d = Design::load(path)?;
    let own = d.meta.get("circuit").cloned().unwrap_or_default();
    let n: u32 = own.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
    let k = kind.replace("{2n}", &(2 * n).to_string()).replace("{n}", &n.to_string());
    d.meta.insert("circuit".into(), k.clone());
    let c = Circuit::from_design(d)?;
    let rep = circuit::verify::sim(&c, &circuit::verify::Options::default())?;
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    Ok(match rep.outcome {
        Some(o) if o.pass => {
            let m: Vec<String> = o.metrics.iter().map(|(a, b)| format!("{a} {b}")).collect();
            format!("PASS {name} ({own}) as {k}: {}", m.join(", "))
        }
        Some(o) => format!("FAIL {name} ({own}) as {k}: {}", o.why),
        None => format!("FAIL {name} ({own}) as {k}: {}", rep.trace.err().unwrap_or_default()),
    })
}

pub fn search(o: &SearchOptions) -> Result<usize> {
    let seed = Design::load(&o.seed)?;
    let c0 = Circuit::from_design(seed.clone())?;
    let Some(kind) = c0.kind.clone() else { bail!("the seed has no `circuit:` kind") };
    if kind.study() != "pulsemult" {
        bail!("the seed's kind {} is not a study-11 kind", kind.name());
    }
    let mut pal: Vec<String> = Vec::new();
    for p in &o.palette {
        pal.extend(expand(p)?);
    }
    let ports = port_cells(&seed);
    let mut cells: Vec<Pos> = Vec::new();
    for x in o.lo[0]..=o.hi[0] {
        for y in o.lo[1]..=o.hi[1] {
            for z in o.lo[2]..=o.hi[2] {
                let p = [x, y, z];
                if !seed.blocks.contains_key(&p) && !ports.contains(&p) {
                    cells.push(p);
                }
            }
        }
    }
    if cells.len() < o.k {
        bail!("the box holds {} free cells, fewer than k = {}", cells.len(), o.k);
    }
    std::fs::create_dir_all(o.out.join("designs"))?;
    let found: Mutex<HashMap<Vec<(Pos, String)>, String>> = Mutex::new(HashMap::new());
    let lines: Mutex<Vec<String>> = Mutex::new(vec!["name\tblocks\tjudge".into()]);
    let next = AtomicUsize::new(0);
    let screened = AtomicUsize::new(0);
    let stood = AtomicUsize::new(0);
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for wi in 0..o.workers.max(1) {
            let (seed, pal, cells, found, lines, next, screened, stood, kind) = (&seed, &pal, &cells, &found, &lines, &next, &screened, &stood, &kind);
            sc.spawn(move || {
                let mut r = Rng(o.rng.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 * 7919 + 13) | 1);
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= o.samples || found.lock().unwrap().len() >= o.max_found {
                        break;
                    }
                    if o.samples >= 10 && i % (o.samples / 10) == 0 && i > 0 {
                        eprintln!("{i} samples, {} standing, {} screened, {} found, {:.0} s", stood.load(Ordering::Relaxed), screened.load(Ordering::Relaxed), found.lock().unwrap().len(), t0.elapsed().as_secs_f64());
                    }
                    let kk = o.kmin + r.below(o.k - o.kmin + 1);
                    let mut d = seed.clone();
                    let mut tries = 0;
                    let allowed = |q: Pos| cells.contains(&q);
                    while d.blocks.len() < seed.blocks.len() + kk && tries < 100 {
                        tries += 1;
                        let p = cells[r.below(cells.len())];
                        if d.blocks.contains_key(&p) {
                            continue;
                        }
                        let s = pal[r.below(pal.len())].clone();
                        place_supported(&mut d.blocks, p, &s, &allowed);
                    }
                    if !o.require.iter().all(|g| d.blocks.values().any(|s| group(s) == g)) || !standing(&d.blocks) {
                        continue;
                    }
                    stood.fetch_add(1, Ordering::Relaxed);
                    let Ok(c) = Circuit::from_design(d.clone()) else { continue };
                    if !screen(&c) {
                        continue;
                    }
                    screened.fetch_add(1, Ordering::Relaxed);
                    let Some((summary, metrics)) = judge(&c) else { continue };
                    let key: Vec<(Pos, String)> = d.blocks.iter().map(|(p, s)| (*p, s.clone())).collect();
                    let mut f = found.lock().unwrap();
                    if f.contains_key(&key) {
                        continue;
                    }
                    let name = format!("pm-{}-{}b-{}", slug(kind, &metrics), d.blocks.len(), f.len());
                    d.name = name.clone();
                    if d.save(&o.out.join("designs").join(format!("{name}.txt"))).is_err() {
                        continue;
                    }
                    lines.lock().unwrap().push(format!("{name}\t{}\t{summary}", d.blocks.len()));
                    f.insert(key, name);
                }
            });
        }
    });
    let lines = lines.into_inner().unwrap();
    std::fs::write(o.out.join("found.tsv"), lines.join("\n") + "\n")?;
    Ok(lines.len() - 1)
}

/// Local search from one passing design (see `Cmd::Improve`).
pub fn improve(path: &Path, palette: &[String], grow: i32, iterations: usize, rng: u64, workers: usize, out: &Path) -> Result<String> {
    let d0 = Design::load(path)?;
    let c0 = Circuit::from_design(d0.clone())?;
    let Some(kind) = c0.kind.clone() else { bail!("{}: no `circuit:` kind", path.display()) };
    let Some((_, m0)) = judge(&c0) else { bail!("{}: does not pass on redsim", path.display()) };
    let key0 = axis_key(&kind, &m0);
    let mut pal: Vec<String> = Vec::new();
    for p in palette {
        pal.extend(expand(p)?);
    }
    let (lo, hi) = d0.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
    let ports = port_cells(&d0);
    let mut cells: Vec<Pos> = Vec::new();
    for x in lo[0] - grow..=hi[0] + grow {
        for y in lo[1] - grow..=hi[1] + grow {
            for z in lo[2] - grow..=hi[2] + grow {
                if !ports.contains(&[x, y, z]) {
                    cells.push([x, y, z]);
                }
            }
        }
    }
    let best: Mutex<BTreeMap<Pos, String>> = Mutex::new(d0.blocks.clone());
    let next = AtomicUsize::new(0);
    std::thread::scope(|sc| {
        for wi in 0..workers.max(1) {
            let (d0, pal, cells, best, next, kind, key0) = (&d0, &pal, &cells, &best, &next, &kind, &key0);
            sc.spawn(move || {
                let mut r = Rng(rng.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(wi as u64 * 104729 + 7) | 1);
                let mut cur = best.lock().unwrap().clone();
                while next.fetch_add(1, Ordering::Relaxed) < iterations {
                    let mut b = cur.clone();
                    let keys: Vec<Pos> = b.keys().copied().collect();
                    let allowed = |q: Pos| cells.contains(&q);
                    match r.below(4) {
                        0 | 1 if !keys.is_empty() => {
                            b.remove(&keys[r.below(keys.len())]);
                        }
                        2 => {
                            let p = cells[r.below(cells.len())];
                            place_supported(&mut b, p, &pal[r.below(pal.len())].clone(), &allowed);
                        }
                        _ if !keys.is_empty() => {
                            let p = keys[r.below(keys.len())];
                            place_supported(&mut b, p, &pal[r.below(pal.len())].clone(), &allowed);
                        }
                        _ => continue,
                    }
                    if b.len() > cur.len() + 2 || !standing(&b) {
                        continue;
                    }
                    let mut d = d0.clone();
                    d.blocks = b.clone();
                    let Ok(c) = Circuit::from_design(d) else { continue };
                    if !screen(&c) {
                        continue;
                    }
                    let Some((_, m)) = judge(&c) else { continue };
                    if axis_key(kind, &m) != *key0 {
                        continue;
                    }
                    // a sideways move is kept (it can open a later removal); fewer blocks is shared
                    if b.len() <= cur.len() {
                        cur = b.clone();
                    }
                    let mut g = best.lock().unwrap();
                    if b.len() < g.len() {
                        *g = b;
                    } else if g.len() < cur.len() {
                        cur = g.clone();
                    }
                }
            });
        }
    });
    let b = best.into_inner().unwrap();
    let mut d = d0.clone();
    let n0 = d0.blocks.len();
    d.blocks = b;
    let c = Circuit::from_design(d.clone())?;
    let (summary, metrics) = judge(&c).ok_or_else(|| anyhow::anyhow!("the best design no longer passes"))?;
    std::fs::create_dir_all(out)?;
    d.name = format!("pm-{}-{}b", slug(&kind, &metrics), d.blocks.len());
    let p = out.join(format!("{}.txt", d.name));
    d.save(&p)?;
    Ok(format!("{}: {n0} -> {} blocks: {summary} -> {}", path.display(), d.blocks.len(), p.display()))
}
