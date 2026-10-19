//! The real-game runner (the JVM bridge): the same cycles as [`crate::rs`] at N positions x 4
//! rotations, each build in a fresh sandbox, every tick of the watched box recorded. A build
//! passes when every cycle gives the payload at the target (placed) after the on edge and the
//! exact rest state back after the off edge (button: after the press), with no entity left (a
//! drop, a popped block) and nothing reaching the edge of the watched box. The extender is
//! `robust` when every build gives the same extend / retract ticks (else `position-dependent` /
//! `rotation-dependent`, flagged), and redsim's numbers are compared.

use crate::rs::Cycle;
use crate::spec::{InputKind, Pext, Pos, add, scale};
use anyhow::{Context, Result, bail};
use mctas::sim::{PlaceMode, Pool, Region, SandboxOptions, Worker, WorkerConfig};

#[derive(Clone, Debug)]
pub struct Options {
    pub positions: usize,
    pub rotations: bool,
    pub cycles: usize,
    pub max_phase: u32,
    pub margin: i32,
    pub workers: usize,
    pub version: String,
}

impl Default for Options {
    fn default() -> Self {
        Options { positions: 4, rotations: true, cycles: 3, max_phase: 400, margin: 3, workers: 8, version: mctas::DEFAULT_VERSION.to_string() }
    }
}

#[derive(Clone, Debug)]
pub struct Build {
    pub turns: u8,
    pub offset: Pos,
    pub result: std::result::Result<Vec<Cycle>, String>,
}

#[derive(Clone, Debug)]
pub struct GameReport {
    pub name: String,
    pub builds: Vec<Build>,
    /// `robust`, `position-dependent`, `rotation-dependent`, or why it fails.
    pub robustness: String,
    pub works: bool,
}

impl GameReport {
    pub fn base(&self) -> Option<&Vec<Cycle>> {
        self.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0]).and_then(|b| b.result.as_ref().ok())
    }
}

/// The design turned `turns` quarter turns clockwise (seen from above) by the game, its ports and
/// travel turned with it.
pub fn rotated(w: &mut Worker, p: &Pext, turns: u8) -> Result<Pext> {
    let turns = turns % 4;
    if turns == 0 {
        return Ok(p.clone());
    }
    let states: Vec<String> = p.design.blocks.values().cloned().collect();
    let turned = w.transform_states(&states, turns, false)?;
    let mut d = p.design.clone();
    d.blocks = p.design.blocks.keys().zip(turned.iter()).map(|(q, s)| (clock::rotate_pos(*q, turns), s.clone())).collect();
    d.ports = p.design.ports.iter().map(|(k, q)| (k.clone(), clock::rotate_pos(*q, turns))).collect();
    let t = clock::rotate_pos(p.travel, turns);
    d.meta.insert("travel".into(), format!("{} {} {}", t[0], t[1], t[2]));
    Pext::from_design(d)
}

pub fn translated(p: &Pext, off: Pos) -> Result<Pext> {
    Pext::from_design(p.design.translated(off))
}

fn cells(r: &Region) -> Vec<(Pos, String)> {
    r.iter().filter(|(_, s)| !mctas::blockstate::is_air(s)).map(|(p, s)| (p, s.to_string())).collect()
}

fn diff(a: &[(Pos, String)], b: &[(Pos, String)]) -> String {
    let am: std::collections::BTreeMap<Pos, &str> = a.iter().map(|(p, s)| (*p, s.as_str())).collect();
    let bm: std::collections::BTreeMap<Pos, &str> = b.iter().map(|(p, s)| (*p, s.as_str())).collect();
    let mut keys: Vec<Pos> = am.keys().chain(bm.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    let out: Vec<String> = keys
        .into_iter()
        .filter_map(|k| {
            let x = am.get(&k).copied().unwrap_or("minecraft:air");
            let y = bm.get(&k).copied().unwrap_or("minecraft:air");
            (x != y).then(|| format!("{},{},{} {}", k[0], k[1], k[2], clock::short(y)))
        })
        .collect();
    if out.is_empty() { "-".into() } else { out.join("; ") }
}

/// Build one turned and moved design in a fresh sandbox and run its cycles.
pub fn run_one(w: &mut Worker, p: &Pext, o: &Options) -> Result<std::result::Result<Vec<Cycle>, String>> {
    let (lo, hi) = p.bounds();
    let m = o.margin;
    let from = [lo[0] - m, lo[1] - m, lo[2] - m];
    let to = [hi[0] + m, hi[1] + m, hi[2] + m];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = run_in(w, sb, p, o, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

fn edge_problem(frame: &Region, from: Pos, to: Pos) -> Option<String> {
    frame.iter().find(|(q, s)| !mctas::blockstate::is_air(s) && (0..3).any(|i| q[i] == from[i] || q[i] == to[i])).map(|(q, s)| format!("{} at {q:?} reached the edge of the watched box", clock::short(s)))
}

fn run_in(w: &mut Worker, sb: u32, p: &Pext, o: &Options, from: Pos, to: Pos) -> Result<std::result::Result<Vec<Cycle>, String>> {
    let blocks = p.design.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(q, _)| *q).collect();
    w.update(sb, &wires, "shape")?;
    let all: Vec<Pos> = blocks.iter().map(|(q, _)| *q).collect();
    w.update(sb, &all, "self")?;
    let st = w.step_until_idle(sb, o.max_phase)?;
    if st.idle == Some(false) {
        return Ok(Err(format!("the placement never settles ({} gt)", o.max_phase)));
    }
    let rest = cells(&w.get_region(sb, from, to)?);
    let target = add(p.payload, scale(p.travel, p.reach));
    let payload = p.payload_state.as_str();
    if !rest.iter().any(|(q, s)| *q == p.payload && s == payload) {
        return Ok(Err("the payload is not at its cell at rest".into()));
    }
    let mut out = Vec::new();
    for c in 0..o.cycles {
        let cy = match p.input_kind {
            InputKind::Lever => {
                w.use_block(sb, p.input)?;
                let on = w.step_record(sb, o.max_phase, true, from, to)?;
                if on.idle == Some(false) {
                    return Ok(Err(format!("cycle {}: on: still active after {} gt", c + 1, o.max_phase)));
                }
                if let Some(e) = on.frames.iter().find_map(|f| edge_problem(f, from, to)) {
                    return Ok(Err(format!("cycle {}: on: {e}", c + 1)));
                }
                let at: Vec<bool> = on.frames.iter().map(|f| extended(f, target, payload)).collect();
                if !at.last().copied().unwrap_or(false) {
                    return Ok(Err(format!("cycle {}: on: settled after {} gt without the payload at the target {target:?}", c + 1, at.len())));
                }
                let n = at.len() as u32;
                let extend = at.iter().rposition(|a| !*a).map(|i| i as u32 + 2).unwrap_or(1);
                w.use_block(sb, p.input)?;
                let off = w.step_record(sb, o.max_phase, true, from, to)?;
                if off.idle == Some(false) {
                    return Ok(Err(format!("cycle {}: off: still active after {} gt", c + 1, o.max_phase)));
                }
                if let Some(e) = off.frames.iter().find_map(|f| edge_problem(f, from, to)) {
                    return Ok(Err(format!("cycle {}: off: {e}", c + 1)));
                }
                let snaps: Vec<Vec<(Pos, String)>> = off.frames.iter().map(cells).collect();
                let last = snaps.last().cloned().unwrap_or_default();
                if last != rest {
                    return Ok(Err(format!("cycle {}: off: not back at rest: {}", c + 1, diff(&rest, &last))));
                }
                let retract = last_change(&snaps, &cells_after(on.frames.last()));
                Cycle { extend, settle: n, retract, hold: n + 1 - extend }
            }
            InputKind::Button => {
                let before = cells(&w.get_region(sb, from, to)?);
                w.use_block(sb, p.input)?;
                let ph = w.step_record(sb, o.max_phase, true, from, to)?;
                if ph.idle == Some(false) {
                    return Ok(Err(format!("cycle {}: still active after {} gt", c + 1, o.max_phase)));
                }
                if let Some(e) = ph.frames.iter().find_map(|f| edge_problem(f, from, to)) {
                    return Ok(Err(format!("cycle {}: {e}", c + 1)));
                }
                let at: Vec<bool> = ph.frames.iter().map(|f| extended(f, target, payload)).collect();
                let Some(first) = at.iter().position(|a| *a) else {
                    return Ok(Err(format!("cycle {}: the payload never reached the target {target:?}", c + 1)));
                };
                let hold = at[first..].iter().take_while(|a| **a).count() as u32;
                let snaps: Vec<Vec<(Pos, String)>> = ph.frames.iter().map(cells).collect();
                let last = snaps.last().cloned().unwrap_or_default();
                if last != rest {
                    return Ok(Err(format!("cycle {}: not back at rest: {}", c + 1, diff(&rest, &last))));
                }
                Cycle { extend: first as u32 + 1, settle: at.len() as u32, retract: last_change(&snaps, &before), hold }
            }
        };
        out.push(cy);
    }
    let ents = w.entities(sb)?;
    if let Some(e) = ents.first() {
        return Ok(Err(format!("{} entities at the end ({} at {:.1?}): a drop or a popped block", ents.len(), clock::short(&e.kind), e.pos)));
    }
    Ok(Ok(out))
}

fn cells_after(f: Option<&Region>) -> Vec<(Pos, String)> {
    f.map(cells).unwrap_or_default()
}

/// The payload placed at the target with nothing moving (no moving_piston in the frame): the
/// judge's "extended".
fn extended(f: &Region, target: Pos, payload: &str) -> bool {
    f.get(target) == Some(payload) && !f.iter().any(|(_, s)| s.starts_with("minecraft:moving_piston"))
}

/// The tick (1-based) of the last frame that differs from the one before it (`start` = the state
/// before the first frame).
fn last_change(snaps: &[Vec<(Pos, String)>], start: &[(Pos, String)]) -> u32 {
    let mut last = 0;
    let mut prev: &[(Pos, String)] = start;
    for (i, s) in snaps.iter().enumerate() {
        if s.as_slice() != prev {
            last = i as u32 + 1;
        }
        prev = s;
    }
    last
}

pub fn spawn_pool(o: &Options) -> Result<Pool> {
    let builds = o.positions * if o.rotations { 4 } else { 1 };
    Pool::spawn(o.workers.min(builds).max(1), &WorkerConfig { version: o.version.clone(), ..Default::default() })
}

/// The judge in the real game.
pub fn verify_in_pool(pool: &Pool, p: &Pext, o: &Options) -> Result<GameReport> {
    if p.design.blocks.is_empty() {
        bail!("empty design");
    }
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rp = translated(&rotated(w, p, t)?, off)?;
        run_one(w, &rp, o)
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let result = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        builds.push(Build { turns: t, offset: off, result });
    }
    let mut rep = GameReport { name: p.name().to_string(), builds, robustness: String::new(), works: false };
    if let Some(b) = rep.builds.iter().find(|b| b.result.is_err()) {
        rep.robustness = format!("rotation {} at {:?}: {}", b.turns, b.offset, b.result.as_ref().err().unwrap());
        return Ok(rep);
    }
    rep.works = true;
    let base = rep.base().cloned().unwrap_or_default();
    let differs = |b: &Build| b.result.as_ref().map(|c| *c != base).unwrap_or(true);
    let pos_dep = rep.builds.iter().filter(|b| b.turns == 0).any(differs);
    let rot_dep = rep.builds.iter().filter(|b| b.turns != 0).any(differs);
    rep.robustness = match (pos_dep, rot_dep) {
        (false, false) => "robust".into(),
        (true, _) => "position-dependent".into(),
        (false, true) => "rotation-dependent".into(),
    };
    Ok(rep)
}
