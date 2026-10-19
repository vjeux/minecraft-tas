//! `mctas circuit sim` (the redsim screen) and `mctas circuit verify` (the judge, in the real game
//! at N positions x 4 rotations): the kind's stimulus, one recorded run per build, the kind's
//! checker on each, and the builds compared (`robust`, else position- or rotation-dependent).

use crate::check::{self, Outcome, Sizing};
use crate::run;
use crate::stimulus::{SETTLE, Schedule};
use crate::trace::Trace;
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow};
use mctas::sim::{Pool, WorkerConfig};

/// Knobs of [`sim`] and [`verify`].
#[derive(Clone, Debug)]
pub struct Options {
    pub positions: usize,
    pub rotations: bool,
    pub workers: usize,
    pub version: String,
    /// Watched cells around the circuit.
    pub margin: i32,
    /// The test spacing; `None`: sized from redsim's recovery time ([`sizing`]).
    pub gap: Option<u32>,
    /// An explicit stimulus instead of the kind's (no checker: the waveform is printed).
    pub stimulus: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options { positions: 4, rotations: true, workers: 8, version: mctas::DEFAULT_VERSION.to_string(), margin: 3, gap: None, stimulus: None }
    }
}

/// The checkers' test spacing: the time the circuit takes to come back to rest after every input
/// turns on for 40 gt and after they turn off (on redsim), plus 10 gt; `None` when redsim does not
/// model it or it never comes back to rest within 2000 gt.
pub fn sizing(c: &Circuit, margin: i32) -> Option<u32> {
    if c.inputs.is_empty() {
        return None;
    }
    let mut s = Schedule::new(c.inputs.len());
    let on = c.clock.free_from() + SETTLE;
    for i in 0..c.inputs.len() {
        s.pulse(i, on, 40);
    }
    s.ticks = on + 40 + 2000;
    let t = run::sim_trace(c, &s, margin).ok()?;
    let last_change = |from: usize, to: usize| -> Option<usize> { (from + 1..to).rev().find(|&f| t.cells[f] != t.cells[f - 1]) };
    let (on, off, end) = (on as usize, on as usize + 40, s.ticks as usize + 1);
    let r_on = last_change(on, off).map(|f| f - on).unwrap_or(0);
    let r_off = last_change(off, end).map(|f| f - off).unwrap_or(0);
    if off + r_off >= end - 1 {
        return None;
    }
    Some((r_on.max(r_off) + 10) as u32)
}

/// What a run is judged against: the explicit stimulus, or the kind's.
fn schedule(c: &Circuit, o: &Options) -> Result<(Schedule, Option<Kind>)> {
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    if let Some(st) = o.stimulus.as_ref().or(if c.kind.is_none() { c.stimulus.as_ref() } else { None }) {
        return Ok((crate::stimulus::parse(st, &names, 100)?, None));
    }
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no `circuit: <kind>` line and no stimulus: nothing to judge"))?;
    let gap = o.gap.or_else(|| sizing(c, o.margin)).unwrap_or(Sizing::default().gap);
    Ok((check::stimulus(c, &kind, &Sizing { gap }), Some(kind)))
}

/// Result of [`sim`].
pub struct SimReport {
    pub schedule: Schedule,
    pub kind: Option<Kind>,
    pub trace: std::result::Result<Trace, String>,
    pub outcome: Option<Outcome>,
}

impl SimReport {
    /// `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD> (redsim)` or why it fails.
    pub fn summary(&self, c: &Circuit) -> String {
        match (&self.trace, &self.outcome, &self.kind) {
            (Err(e), _, _) => format!("UNMODELED: {}: {e}", c.name()),
            (Ok(_), None, _) => format!("RUN: {} ({} frames)", c.name(), self.schedule.ticks + 1),
            (Ok(t), Some(out), Some(k)) => line(c, k, out, dims(c, t), "redsim"),
            _ => String::new(),
        }
    }
}

/// Run the kind's stimulus (or an explicit one) on redsim and judge it.
pub fn sim(c: &Circuit, o: &Options) -> Result<SimReport> {
    let (schedule, kind) = schedule(c, o)?;
    let trace = run::sim_trace(c, &schedule, o.margin);
    let outcome = match (&trace, &kind) {
        (Ok(t), Some(k)) => Some(check::check(c, k, &schedule, t)),
        _ => None,
    };
    Ok(SimReport { schedule, kind, trace, outcome })
}

/// The box of every non-air cell over the run, the input cells left out: width x height x depth.
pub fn dims(c: &Circuit, t: &Trace) -> [i32; 3] {
    let ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for f in &t.cells {
        for (p, _) in f.iter().filter(|(p, _)| !ins.contains(p)) {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    if lo[0] == i32::MAX { [0, 0, 0] } else { [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1] }
}

fn line(c: &Circuit, k: &Kind, out: &Outcome, d: [i32; 3], how: &str) -> String {
    if out.pass {
        format!(
            "CIRCUIT: {} {} {}, {} blocks, {}x{}x{}, {how}{}",
            c.name(),
            k.name(),
            out.metrics_text(),
            c.blocks(),
            d[0],
            d[1],
            d[2],
            if out.why.is_empty() { String::new() } else { format!(" ({})", out.why) }
        )
    } else {
        format!("FAIL: {} {}: {}", c.name(), k.name(), out.why)
    }
}

/// One build of [`verify`].
#[derive(Clone, Debug)]
pub struct Build {
    pub turns: u8,
    pub offset: Pos,
    /// The checker's verdict, or why the run broke a rule.
    pub result: std::result::Result<(Outcome, [i32; 3]), String>,
}

/// Result of [`verify`].
pub struct VerifyReport {
    pub name: String,
    pub kind: Option<Kind>,
    pub blocks: usize,
    pub schedule: Schedule,
    pub builds: Vec<Build>,
    /// `robust`, `position-dependent`, `rotation-dependent`, or why it fails.
    pub verdict: String,
    pub pass: bool,
}

impl VerifyReport {
    /// The reference build: as built at the first position, else the first passing one.
    pub fn base(&self) -> Option<&(Outcome, [i32; 3])> {
        self.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0]).and_then(|b| b.result.as_ref().ok()).filter(|(o, _)| o.pass).or_else(|| self.builds.iter().find_map(|b| b.result.as_ref().ok().filter(|(o, _)| o.pass)))
    }

    /// `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, robust` or `FAIL: ...`.
    pub fn summary(&self, c: &Circuit) -> String {
        match (&self.kind, self.base()) {
            (Some(k), Some((o, d))) if self.pass => line(c, k, o, *d, &self.verdict),
            (Some(k), _) => format!("FAIL: {} {}: {}", self.name, k.name(), self.verdict),
            (None, _) => format!("RUN: {}: {}", self.name, self.verdict),
        }
    }
}

/// The judge: see the module doc.
pub fn verify(c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let pool = spawn_pool(o)?;
    verify_in_pool(&pool, c, o)
}

/// The judge's JVM workers (`o.workers`, capped by MCTAS_MAX_WORKERS), to judge many circuits.
pub fn spawn_pool(o: &Options) -> Result<Pool> {
    let builds = o.positions * if o.rotations { 4 } else { 1 };
    Pool::spawn(o.workers.min(builds).max(1), &WorkerConfig { version: o.version.clone(), ..Default::default() })
}

/// [`verify`] with existing workers.
pub fn verify_in_pool(pool: &Pool, c: &Circuit, o: &Options) -> Result<VerifyReport> {
    let (schedule, kind) = schedule(c, o)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let r = run::game_trace(w, &rc, &schedule, o.margin)?;
        Ok(r.map(|tr| {
            let out = match &kind {
                Some(k) => check::check(&rc, k, &schedule, &tr),
                None => Outcome { pass: true, why: String::new(), metrics: Vec::new(), table: tr.strip(&rc, 0, tr.frames()) },
            };
            (out, dims(&rc, &tr))
        }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let result = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        builds.push(Build { turns: t, offset: off, result });
    }
    let mut rep = VerifyReport { name: c.name().to_string(), kind, blocks: c.blocks(), schedule, builds, verdict: String::new(), pass: false };
    let failed = rep.builds.iter().find(|b| !b.result.as_ref().is_ok_and(|(o, _)| o.pass));
    let ok_count = rep.builds.iter().filter(|b| b.result.as_ref().is_ok_and(|(o, _)| o.pass)).count();
    if ok_count == 0 {
        let b = &rep.builds[0];
        rep.verdict = match &b.result {
            Ok((o, _)) => o.why.clone(),
            Err(e) => e.clone(),
        };
        return Ok(rep);
    }
    if let Some(b) = failed {
        // some builds pass, some do not: allowed only as a flagged dependence when the metrics
        // agree among the passing ones; a failing build is reported
        let why = match &b.result {
            Ok((o, _)) => o.why.clone(),
            Err(e) => e.clone(),
        };
        rep.verdict = format!("fails at rotation {} offset {:?}: {why}", b.turns, b.offset);
        return Ok(rep);
    }
    let sig = |b: &Build| b.result.as_ref().map(|(o, _)| o.metrics_text()).unwrap_or_default();
    let base = sig(rep.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0]).unwrap_or(&rep.builds[0]));
    let pos_dep = rep.builds.iter().filter(|b| b.turns == 0).any(|b| sig(b) != base);
    let rot_dep = rep.builds.iter().filter(|b| b.turns != 0).any(|b| sig(b) != base);
    rep.verdict = match (pos_dep, rot_dep) {
        (false, false) => "robust".into(),
        (true, _) => "position-dependent".into(),
        (false, true) => "rotation-dependent".into(),
    };
    rep.pass = true;
    Ok(rep)
}
