//! `mctas clock verify`: the official clock judge, in the real game (the JVM bridge the door judge
//! uses). The clock is built at N positions x 4 rotations (the game turns every block state), each
//! in a cleared sandbox, placed exactly like `clock sim` places it, kicked, and run to a long horizon
//! while every tick of its box (plus a margin) is recorded. Each run must be exactly periodic for
//! at least 3 periods after its warmup (block states compared cell by cell), create no entity (a
//! drop, a popped block) and never reach the edge of the watched box. Runs that agree on period,
//! output waveform and family make a `robust` clock; differences are reported as
//! position-dependent or rotation-dependent (allowed, flagged).

use crate::sim::{Output, Verdict};
use crate::wave::{self, CellWave, Interner, PeriodAnalysis};
use crate::{Action, Clock, Kick, Placement, Pos};
use anyhow::{Context, Result, anyhow};
use mctas::sim::{PlaceMode, Pool, Region, SandboxOptions, Worker, WorkerConfig};

/// Knobs of [`verify`].
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Test positions (the door judge's offsets: its default four, then its extra ones).
    pub positions: usize,
    /// All 4 rotations (else only as built).
    pub rotations: bool,
    /// Ticks to run each build.
    pub horizon: u32,
    /// Watched cells around the clock's box (anything reaching the outer layer fails).
    pub margin: i32,
    /// Full periods required after the warmup.
    pub min_periods: usize,
    /// JVM workers (capped by MCTAS_MAX_WORKERS).
    pub workers: usize,
    pub version: String,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions { positions: 8, rotations: true, horizon: 4000, margin: 3, min_periods: 3, workers: 8, version: mctas::DEFAULT_VERSION.to_string() }
    }
}

/// One build (a rotation at a position) that ran periodically.
#[derive(Clone, Debug)]
pub struct RunOk {
    pub warmup: u32,
    pub period: u32,
    pub analysis: PeriodAnalysis,
    pub output: Option<Output>,
    /// Width x height x depth of every cell non-air during the run (and the design's box).
    pub dims: [i32; 3],
}

impl RunOk {
    /// What must agree between builds: period, output waveform, family.
    fn signature(&self) -> String {
        let (on, edges, widths) = self.output.as_ref().map(|o| (o.wave.on, o.wave.edges, o.wave.widths.clone())).unwrap_or_default();
        format!("P={} on {on} edges {edges} pulses {widths:?} family {}", self.period, wave::signature(&self.analysis.family))
    }
}

#[derive(Clone, Debug)]
pub struct Run {
    pub turns: u8,
    pub offset: Pos,
    pub result: std::result::Result<RunOk, String>,
}

/// Result of [`verify`].
#[derive(Clone, Debug)]
pub struct VerifyReport {
    pub name: String,
    pub blocks: usize,
    pub start: String,
    pub runs: Vec<Run>,
    /// `robust`, `position-dependent` or `rotation-dependent` (a clock), else why it fails.
    pub robustness: String,
    pub verdict: Verdict,
}

impl VerifyReport {
    /// The reference build: as built at the first position, else the first periodic build.
    pub fn base(&self) -> Option<&RunOk> {
        self.runs.iter().find(|r| r.turns == 0 && r.offset == [0, 0, 0]).and_then(|r| r.result.as_ref().ok()).or_else(|| self.runs.iter().find_map(|r| r.result.as_ref().ok()))
    }

    /// `CLOCK: <name> period P gt, on Q/P, edges E, warmup W, family <sig>, <blocks> blocks, <WxHxD>, robust`.
    pub fn summary(&self) -> String {
        match (self.verdict, self.base()) {
            (Verdict::Clock, Some(b)) => {
                let (on, edges) = b.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
                let [w, h, d] = b.dims;
                format!(
                    "CLOCK: {} period {} gt, on {on}/{}, edges {edges}, warmup {}, family {}, {} blocks, {w}x{h}x{d}, {}",
                    self.name,
                    b.period,
                    b.period,
                    b.warmup,
                    wave::signature(&b.analysis.family),
                    self.blocks,
                    self.robustness
                )
            }
            (v, _) => format!("{}: {} {}", if v == Verdict::Clock { "FAIL" } else { v.word() }, self.name, self.robustness),
        }
    }
}

/// The clock turned `turns` quarter turns clockwise (seen from above) around the origin; the game
/// turns every block state (and a temporary start block).
pub fn rotated(w: &mut Worker, c: &Clock, turns: u8) -> Result<Clock> {
    let turns = turns % 4;
    if turns == 0 {
        return Ok(c.clone());
    }
    let mut states: Vec<String> = c.design.blocks.values().cloned().collect();
    let kick_state = match &c.kick {
        Some(Kick::Place { state, .. }) => Some(state.clone()),
        _ => None,
    };
    if let Some(s) = &kick_state {
        states.push(s.clone());
    }
    let turned = w.transform_states(&states, turns, false)?;
    let mut d = c.design.clone();
    d.blocks = c.design.blocks.keys().zip(turned.iter()).map(|(p, s)| (crate::rotate_pos(*p, turns), s.clone())).collect();
    d.ports = c.design.ports.iter().map(|(k, p)| (k.clone(), crate::rotate_pos(*p, turns))).collect();
    if let Some(Kick::Place { ticks, .. }) = &c.kick {
        d.meta.insert("port start".into(), format!("{} {ticks}", turned.last().unwrap()));
    }
    Clock::from_design(d)
}

fn place(w: &mut Worker, sb: u32, c: &Clock) -> Result<()> {
    let blocks = c.design.block_list();
    let last = c.withheld();
    let kept: Vec<(Pos, String)> = blocks.iter().filter(|(p, _)| Some(*p) != last).cloned().collect();
    match c.placement {
        Placement::Judge => {
            w.set_blocks(sb, &kept, PlaceMode::Raw)?;
            let wires: Vec<Pos> = kept.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
            w.update(sb, &wires, "shape")?;
            let positions: Vec<Pos> = kept.iter().map(|(p, _)| *p).collect();
            w.update(sb, &positions, "self")?;
        }
        Placement::Player => {
            for b in &kept {
                w.set_blocks(sb, std::slice::from_ref(b), PlaceMode::Update)?;
            }
        }
    }
    Ok(())
}

fn apply(w: &mut Worker, sb: u32, a: &Action) -> Result<()> {
    match a {
        Action::Place { pos, state } => {
            w.set_blocks(sb, &[(*pos, state.clone())], PlaceMode::Update)?;
        }
        Action::Use { pos } => {
            w.use_block(sb, *pos)?;
        }
        Action::Clear { pos } => {
            w.set_blocks(sb, &[(*pos, "minecraft:air".to_string())], PlaceMode::Update)?;
        }
    }
    Ok(())
}

/// Build, kick and run one clock (already turned and moved) in a fresh sandbox.
pub fn run_one(w: &mut Worker, c: &Clock, o: &VerifyOptions) -> Result<std::result::Result<RunOk, String>> {
    let (lo, hi) = c.bounds();
    let m = o.margin;
    let from = [lo[0] - m, lo[1] - m, lo[2] - m];
    let to = [hi[0] + m, hi[1] + m, hi[2] + m];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = run_in(w, sb, c, o, from, to);
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in(w: &mut Worker, sb: u32, c: &Clock, o: &VerifyOptions, from: Pos, to: Pos) -> Result<std::result::Result<RunOk, String>> {
    place(w, sb, c)?;
    let probes: Vec<Pos> = c.output.into_iter().collect();
    // Frame t = the state after t ticks, with the kick's inputs due at t applied (frame 0: right
    // after placement and the inputs due at 0): exactly what `clock sim` records.
    let mut frames: Vec<Region> = Vec::new();
    let mut signals: Vec<Vec<u8>> = Vec::new();
    let mut actions = c.actions();
    let mut t = 0u32;
    loop {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            apply(w, sb, &a)?;
        }
        // the state at t (after its inputs): replace the recorded frame when inputs were applied
        let f = w.get_region(sb, from, to)?;
        let sg = probe_now(w, sb, &probes)?;
        if frames.len() as u32 == t + 1 {
            *frames.last_mut().unwrap() = f;
            *signals.last_mut().unwrap() = sg;
        } else {
            frames.push(f);
            signals.push(sg);
        }
        let next = actions.first().map(|(at, _)| *at).unwrap_or(o.horizon).min(o.horizon);
        if next <= t {
            break;
        }
        let st = w.step_probe(sb, next - t, false, &probes, Some((from, to)))?;
        frames.extend(st.frames);
        signals.extend(st.signals);
        t = next;
        if t >= o.horizon && actions.is_empty() {
            break;
        }
    }
    let ents = w.entities(sb)?;
    if let Some(e) = ents.first() {
        return Ok(Err(format!("{} entit{} appeared (a drop or a popped block): {} at {:?}", ents.len(), if ents.len() == 1 { "y" } else { "ies" }, e.kind, e.pos)));
    }
    // Frames to ids; the swept box; the edge check.
    let mut int = Interner::default();
    let mut ids: Vec<u64> = Vec::with_capacity(frames.len());
    let mut cells: Vec<Pos> = Vec::new();
    let (mut slo, mut shi) = c.bounds();
    let mut per_frame: Vec<Vec<u32>> = Vec::with_capacity(frames.len());
    for (t, f) in frames.iter().enumerate() {
        let mut v = Vec::with_capacity(f.data.len());
        let first = cells.is_empty();
        for (p, s) in f.iter() {
            if first {
                cells.push(p);
            }
            if !mctas::blockstate::is_air(s) {
                if (0..3).any(|i| p[i] == from[i] || p[i] == to[i]) {
                    return Ok(Err(format!("tick {t}: {} at {p:?} reached the edge of the watched box (margin {}): rerun with a larger --margin", crate::short(s), o.margin)));
                }
                crate::grow(&mut slo, &mut shi, p);
            }
            v.push(int.state(s));
        }
        ids.push(int.frame(&v));
        per_frame.push(v);
    }
    let end = c.free_from() as usize;
    let Some((w0, p)) = wave::detect_period(&ids, end, o.min_periods) else {
        return Ok(Err(format!("not periodic for {} periods within {} gt", o.min_periods, o.horizon)));
    };
    let states: Vec<Vec<&str>> = (w0..w0 + p).map(|t| per_frame[t].iter().map(|id| int.name(*id)).collect()).collect();
    let analysis = wave::analyze(&cells, &states);
    if analysis.cells.is_empty() {
        return Ok(Err(if p == 1 { format!("goes still at tick {w0}") } else { format!("no block changes in its period ({p} gt)") }));
    }
    let output = match c.output {
        Some(op) => {
            let idx = cells.iter().position(|q| *q == op).ok_or_else(|| anyhow!("output outside the box"))?;
            let seq: Vec<bool> = (w0..w0 + p)
                .map(|t| {
                    let s = int.name(per_frame[t][idx]);
                    if wave::family_name(s).is_some() { wave::active(s) } else { signals.get(t).and_then(|v| v.first()).copied().unwrap_or(0) > 0 }
                })
                .collect();
            let (on, edges, widths) = wave::bool_wave(&seq);
            let states = analysis.cells.iter().find(|w| w.pos == op).map(|w| w.states.clone()).unwrap_or_default();
            Some(Output { pos: op, auto: false, wave: CellWave { pos: op, states, kinds: Vec::new(), on, edges, widths } })
        }
        None => analysis.cells.iter().max_by(|a, b| (a.edges, a.on).cmp(&(b.edges, b.on)).then(b.pos.cmp(&a.pos))).map(|w| Output { pos: w.pos, auto: true, wave: w.clone() }),
    };
    let dims = [shi[0] - slo[0] + 1, shi[1] - slo[1] + 1, shi[2] - slo[2] + 1];
    Ok(Ok(RunOk { warmup: w0 as u32, period: p as u32, analysis, output, dims }))
}

fn probe_now(w: &mut Worker, sb: u32, probes: &[Pos]) -> Result<Vec<u8>> {
    if probes.is_empty() {
        return Ok(Vec::new());
    }
    Ok(w.signals(sb, probes)?.into_iter().flat_map(|(a, b)| [a, b]).collect())
}

/// The judge: see the module doc.
pub fn verify(c: &Clock, o: &VerifyOptions) -> Result<VerifyReport> {
    let offsets = crate::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let pool = Pool::spawn(o.workers.min(jobs.len()).max(1), &WorkerConfig { version: o.version.clone(), ..Default::default() })?;
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = rotated(w, c, t)?.translated(off);
        run_one(w, &rc, o)
    });
    let mut runs = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let result = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        runs.push(Run { turns: t, offset: off, result });
    }
    let mut rep = VerifyReport { name: c.name().to_string(), blocks: c.blocks(), start: c.start_label(), runs, robustness: String::new(), verdict: Verdict::Clock };
    let ok: Vec<&Run> = rep.runs.iter().filter(|r| r.result.is_ok()).collect();
    if ok.is_empty() {
        rep.verdict = Verdict::Invalid;
        rep.robustness = rep.runs.first().and_then(|r| r.result.as_ref().err().cloned()).unwrap_or_default();
        return Ok(rep);
    }
    let base_sig = rep.base().unwrap().signature();
    let differs = |r: &Run| r.result.as_ref().map(|k| k.signature() != base_sig).unwrap_or(true);
    let pos_dep = rep.runs.iter().filter(|r| r.turns == 0).any(differs);
    let rot_dep = rep.runs.iter().filter(|r| r.turns != 0).any(differs);
    rep.robustness = match (pos_dep, rot_dep) {
        (false, false) => "robust".into(),
        (true, _) => "position-dependent".into(),
        (false, true) => "rotation-dependent".into(),
    };
    Ok(rep)
}
