//! `mctas circuit fly verify`: the real-game check (the JVM bridge every judge uses). Each build
//! (4 positions x 4 rotations by default; the game turns every block state, the travel vector
//! turns with the design) runs in its own sandbox sized to the corridor: stop blocks placed and
//! updated first, then the machine exactly like the clock judge (`clock::verify::place`), the
//! kick, and every tick of the corridor recorded into the same [`Recorder`] the redsim runner
//! uses, until the machine comes to rest, reaches the corridor's end or the horizon. Entities at
//! the end (a drop, a popped block) break the run. Builds that agree on the verdict, the steady
//! period and cells per period (the direction turned back), the first move and the end make a
//! `robust` machine; differences are reported as position- or rotation-dependent.

use crate::machine::Machine;
use crate::report::{self, End, FlyReport, Verdict};
use crate::track::Recorder;
use anyhow::{Context, Result};
use clock::Pos;
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker, WorkerConfig};

#[derive(Clone, Debug)]
pub struct VerifyOptions {
    pub positions: usize,
    pub rotations: bool,
    /// Ticks to run at most (0 = 200 + 60 gt per corridor cell).
    pub horizon: u32,
    /// Corridor length when the design has no `distance:`.
    pub distance: i32,
    pub margin: i32,
    pub workers: usize,
    pub version: String,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions { positions: 4, rotations: true, horizon: 0, distance: 32, margin: 3, workers: 16, version: mctas::DEFAULT_VERSION.to_string() }
    }
}

/// The machine turned `turns` quarter turns clockwise seen from above (the game turns the block
/// states; travel, payload and stops turn with it).
pub fn rotated(w: &mut Worker, m: &Machine, turns: u8) -> Result<Machine> {
    let turns = turns % 4;
    if turns == 0 {
        return Ok(m.clone());
    }
    let clock = clock::verify::rotated(w, &m.clock, turns)?;
    let states: Vec<String> = m.stops.iter().map(|(_, s)| s.clone()).collect();
    let turned = if states.is_empty() { Vec::new() } else { w.transform_states(&states, turns, false)? };
    Ok(Machine {
        clock,
        kind: m.kind.clone(),
        travel: m.travel.map(|v| clock::rotate_pos(v, turns)),
        distance: m.distance,
        payload: m.payload.iter().map(|p| clock::rotate_pos(*p, turns)).collect(),
        stops: m.stops.iter().zip(turned).map(|((p, _), s)| (clock::rotate_pos(*p, turns), s)).collect(),
    })
}

/// One build (already turned and moved) in a fresh sandbox.
pub fn run_one(w: &mut Worker, m: &Machine, o: &VerifyOptions) -> Result<FlyReport> {
    let (lo, hi) = m.corridor(o.distance, o.margin);
    let chunks = [lo[0].div_euclid(16), lo[2].div_euclid(16), hi[0].div_euclid(16), hi[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = run_in(w, sb, m, o, lo, hi);
    let _ = w.drop_sandbox(sb);
    r
}

fn run_in(w: &mut Worker, sb: u32, m: &Machine, o: &VerifyOptions, lo: Pos, hi: Pos) -> Result<FlyReport> {
    if !m.stops.is_empty() {
        w.set_blocks(sb, &m.stops, PlaceMode::Raw)?;
        let ps: Vec<Pos> = m.stops.iter().map(|(p, _)| *p).collect();
        w.update(sb, &ps, "self")?;
    }
    clock::verify::place(w, sb, &m.clock)?;
    let mut rec = Recorder::new(&m.design().blocks, &m.stops, crate::sim::kick_window(m));
    let k = m.distance.unwrap_or(o.distance).max(1) as u32;
    let horizon = if o.horizon > 0 { o.horizon } else { 200 + 60 * k };
    let edge = |p: Pos| (0..3).any(|i| p[i] <= lo[i] || p[i] >= hi[i]);
    let stop_cells: std::collections::HashMap<Pos, &str> = m.stops.iter().map(|(p, s)| (*p, s.as_str())).collect();
    let at_edge = |f: &mctas::sim::Region| f.iter().any(|(p, s)| !mctas::blockstate::is_air(s) && stop_cells.get(&p) != Some(&s) && edge(p));
    let mut actions = m.clock.actions();
    let mut t = 0u32;
    let volume = ((hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1)).max(1) as u32;
    let seg = (400_000 / volume).clamp(10, 500);
    let end = 'run: loop {
        while actions.first().is_some_and(|(at, _)| *at == t) {
            let (_, a) = actions.remove(0);
            clock::verify::apply(w, sb, &a)?;
        }
        // the state at t after its inputs (replaces the frame recorded for t, if any)
        let f = w.get_region(sb, lo, hi)?;
        let replace = rec.ticks() as u32 == t + 1;
        rec.ingest(t, f.iter(), replace);
        if at_edge(&f) {
            break End::Edge(t);
        }
        let next = actions.first().map(|(at, _)| *at).unwrap_or(horizon).min(horizon).min(t + seg);
        if next <= t {
            break End::Horizon(t);
        }
        let quiet = actions.is_empty();
        let st = w.step_record(sb, next - t, quiet, lo, hi)?;
        for (i, f) in st.frames.iter().enumerate() {
            let tt = t + 1 + i as u32;
            rec.ingest(tt, f.iter(), false);
            if at_edge(f) {
                break 'run End::Edge(tt);
            }
        }
        t += st.frames.len() as u32;
        if quiet && st.idle == Some(true) {
            break End::Idle(t);
        }
        if t >= horizon && actions.is_empty() {
            break End::Horizon(t);
        }
    };
    let ents = w.entities(sb)?;
    let end = match ents.first() {
        Some(e) => End::Drop(rec.ticks() as u32, format!("{} entity at {:.1?}: a drop or a popped block", e.kind, e.pos)),
        None => end,
    };
    Ok(report::classify(m, &rec, end))
}

/// One build's outcome.
#[derive(Clone, Debug)]
pub struct Build {
    pub turns: u8,
    pub offset: Pos,
    pub report: FlyReport,
}

#[derive(Clone, Debug)]
pub struct VerifyReport {
    pub base: FlyReport,
    pub builds: Vec<Build>,
    /// `robust`, `position-dependent`, `rotation-dependent`, or why it fails.
    pub robustness: String,
}

impl VerifyReport {
    pub fn ok(&self) -> bool {
        matches!(self.base.verdict, Verdict::Flies | Verdict::Shuttles) && self.builds.iter().all(|b| b.report.verdict == self.base.verdict)
    }

    pub fn summary(&self) -> String {
        format!("{} [game: {} builds, {}]", self.base.summary(), self.builds.len(), self.robustness)
    }
}

/// What must agree between builds: verdict, steady period, cells per period, direction (turned
/// back to the design's frame), first move, final rest shift (turned back).
fn signature(b: &Build) -> String {
    let r = &b.report;
    let back = |v: Pos| clock::rotate_pos(v, (4 - b.turns % 4) % 4);
    let st = r.steady.as_ref().map(|s| format!("P{} d{:?}", s.period, back(s.d))).unwrap_or_default();
    let rest = r.final_shift.map(|s| format!("{:?}", back(s))).unwrap_or_default();
    format!("{} {st} first {:?} rest {rest}", r.verdict.word(), r.first_change)
}

pub fn spawn_pool(o: &VerifyOptions) -> Result<Pool> {
    let builds = o.positions * if o.rotations { 4 } else { 1 };
    Pool::spawn(o.workers.min(builds).max(1), &WorkerConfig { version: o.version.clone(), ..Default::default() })
}

/// The judge: every build, then agreement.
pub fn verify_in_pool(pool: &Pool, m: &Machine, o: &VerifyOptions) -> Result<VerifyReport> {
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rm = rotated(w, m, t)?.translated(off);
        run_one(w, &rm, o)
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let report = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        builds.push(Build { turns: t, offset: off, report });
    }
    let base = builds[0].report.clone();
    let base_sig = signature(&builds[0]);
    let differs = |b: &Build| signature(b) != base_sig;
    let pos_dep = builds.iter().filter(|b| b.turns == 0).any(differs);
    let rot_dep = builds.iter().filter(|b| b.turns != 0).any(differs);
    let robustness = if !matches!(base.verdict, Verdict::Flies | Verdict::Shuttles) {
        format!("as built: {}", base.verdict.word())
    } else {
        match (pos_dep, rot_dep) {
            (false, false) => "robust".into(),
            (true, _) => "position-dependent".into(),
            (false, true) => "rotation-dependent".into(),
        }
    };
    Ok(VerifyReport { base, builds, robustness })
}
