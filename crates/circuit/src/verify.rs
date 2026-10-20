//! `mctas circuit sim` (the redsim screen) and `mctas circuit verify` (the judge, in the real game
//! at N positions x 4 rotations): the kind's stimulus, one recorded run per build, the kind's
//! checker on each, and the builds compared (`robust`, else position- or rotation-dependent).

use crate::check::{self, Outcome, Sizing};
use crate::run;
use crate::stimulus::{SETTLE, Schedule};
use crate::trace::Trace;
use crate::{Circuit, Kind, Pos};
use anyhow::{Context, Result, anyhow};
use mctas::sim::{Pool, Worker, WorkerConfig};

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
/// turns on (held until rest, at most 1000 gt) and after they turn off (on redsim), plus 10 gt,
/// longer when an event of the kind's own stimulus (run spaced by 1000 gt) takes longer to settle
/// (then that settling + 10 gt); `None` when redsim does not model it or it never rests.
pub fn sizing(c: &Circuit, margin: i32) -> Option<u32> {
    if c.inputs.is_empty() {
        return None;
    }
    let n = c.inputs.len();
    let on = c.clock.free_from() + SETTLE;
    let hold = 1000u32;
    let last_change = |t: &Trace, from: u32, to: u32| -> u32 { (from as usize + 1..=to as usize).rev().find(|&f| t.cells[f] != t.cells[f - 1]).map(|f| f as u32 - from).unwrap_or(0) };
    // every input on and held: the time to rest
    let mut s = Schedule::new(n);
    for i in 0..n {
        s.set(i, on, true);
    }
    s.ticks = on + hold;
    let t = run::sim_trace(c, &s, margin).ok()?;
    let r_on = last_change(&t, on, on + hold);
    if r_on + 10 >= hold {
        return None;
    }
    // then every input off: the time to rest again
    let off = on + r_on + 10;
    for i in 0..n {
        s.set(i, off, false);
    }
    s.ticks = off + hold;
    let t = run::sim_trace(c, &s, margin).ok()?;
    let r_off = last_change(&t, off, off + hold);
    if r_off + 10 >= hold {
        return None;
    }
    // the kind's own stimulus, spaced by `hold`: the longest settling after any input change (a
    // slow path the all-on / all-off pair never takes, such as a crafter's nine ingots coming back
    // one by one through a hopper after the reverse craft)
    let mut r_stim = 0;
    if let Some(kind) = &c.kind {
        let long = check::stimulus(c, kind, &Sizing { gap: hold });
        let t = run::sim_trace(c, &long, margin).ok()?;
        let frames = t.cells.len();
        let changes: Vec<usize> = (1..frames).filter(|&f| t.inputs.iter().any(|i| i[f] != i[f - 1])).collect();
        for (k, &f) in changes.iter().enumerate() {
            // the window up to the frame before the next input change (or the end)
            let end = changes.get(k + 1).map(|&g| g - 1).unwrap_or(frames - 1);
            let settle = last_change(&t, f as u32, end as u32);
            if end - f >= hold as usize && settle + 10 >= hold {
                return None;
            }
            r_stim = r_stim.max(settle);
        }
    }
    // kept when every event of the kind's own stimulus settles within it; else the longest
    // settling + 10 gt
    let gap = r_on.max(r_off) + 10;
    Some(if r_stim > gap { r_stim + 10 } else { gap })
}

/// What a run is judged against: the explicit stimulus, or the kind's.
pub(crate) fn schedule(c: &Circuit, o: &Options) -> Result<(Schedule, Option<Kind>)> {
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    if let Some(st) = o.stimulus.as_ref().or(if c.kind.is_none() { c.stimulus.as_ref() } else { None }) {
        return Ok((crate::stimulus::parse(st, &names, 100)?, None));
    }
    let kind = c.kind.clone().ok_or_else(|| anyhow!("no `circuit: <kind>` line and no stimulus: nothing to judge"))?;
    let gap = o.gap.or_else(|| sizing(c, o.margin)).unwrap_or(Sizing::default().gap);
    let mut s = check::stimulus(c, &kind, &Sizing { gap });
    let _ = &mut s;
    Ok((s, Some(kind)))
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
            (Err(e), _, Some(k)) if e.contains("no support") || e.starts_with("blown up") || c.input_refused.as_deref() == Some(e.as_str()) => format!("FAIL: {} {}: {e}", c.name(), k.name()),
            (Err(e), _, _) => format!("UNMODELED: {}: {e}", c.name()),
            (Ok(_), None, _) => format!("RUN: {} ({} frames)", c.name(), self.schedule.ticks + 1),
            (Ok(t), Some(out), Some(k)) => line(c, k, out, dims(c, t), "redsim"),
            _ => String::new(),
        }
    }
}

/// Run the kind's stimulus (or an explicit one) on redsim and judge it.
pub fn sim(c: &Circuit, o: &Options) -> Result<SimReport> {
    // R7: an input the judge cannot drive like a real build refuses the design
    if let Some(why) = &c.input_refused {
        let schedule = Schedule::new(c.inputs.len());
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err(why.clone()), outcome: c.kind.as_ref().map(|_| Outcome::fail(why.clone(), Vec::new())) });
    }
    if c.kind.as_ref().is_some_and(|k| k.is_shulker()) && o.stimulus.is_none() {
        let mut schedule = Schedule::new(c.inputs.len());
        schedule.ticks = 0;
        return Ok(SimReport { schedule, kind: c.kind.clone(), trace: Err("redsim does not judge shulker loaders (study 8: judged in the game, `mctas circuit verify`; `mctas circuit shulker compare` replays their real-game traces in redsim)".into()), outcome: None });
    }
    let (schedule, kind) = schedule(c, o)?;
    let trace = run::sim_trace(c, &schedule, o.margin);
    let outcome = match (&trace, &kind) {
        (Ok(t), Some(k)) => {
            let out = check::check(c, k, &schedule, t);
            let out = match lamp_part(c, &schedule, o.margin, t) {
                Some(why) if out.pass => Outcome::fail(why, out.table),
                _ => out,
            };
            // R7: the judge's input stones are never moved
            let out = match c.input_entered(t) {
                Some(why) if out.pass => Outcome::fail(why, out.table),
                _ => out,
            };
            Some(if out.pass && tiles(c, k) { tiled_sim(c, k, &schedule, o.margin, &out) } else { out })
        }
        _ => None,
    };
    // a BUD's `tile:` claim: the copies side by side (crate::bud::tile_check)
    let outcome = match (outcome, c.tile) {
        (Some(out), Some(tl)) if out.pass && kind == Some(Kind::Bud) && o.stimulus.is_none() => {
            Some(crate::bud::tile_check(c, &out, crate::bud::gap_of(&schedule), &crate::bud::tile_label(tl.step), |tc, ts| Ok(run::sim_trace(tc, ts, o.margin)))?)
        }
        (out, _) => out,
    };
    // a sculk design: the device and ambient runs (crate::vibration)
    let outcome = match (outcome, &kind, &trace) {
        (Some(out), Some(k), Ok(qt)) if out.pass && c.has_sculk() && o.stimulus.is_none() => Some(crate::vibration::check(c, k, &out, &schedule, qt, |vc, vs| Ok(run::sim_trace(vc, vs, o.margin)))?),
        (out, _, _) => out,
    };
    // a study 12 bit slice: its stacks of 2..4 copies judged as the N-bit kind
    let outcome = match (outcome, &kind, c.slice) {
        (Some(out), Some(Kind::Compute(spec)), Some(_)) if out.pass && o.stimulus.is_none() => {
            let margin = o.margin;
            let gap0 = crate::compute::gap_of(&schedule);
            Some(crate::compute::slice_check(c, *spec, &out, |sc| sizing(sc, margin).unwrap_or(gap0).max(gap0), |sc, ss| Ok(run::sim_trace(sc, ss, margin)))?)
        }
        (out, _, _) => out,
    };
    Ok(SimReport { schedule, kind, trace, outcome })
}

/// A design with a `tile:` header whose copies are judged too ([`tiled_check`]): study 11, and
/// the sequential kinds builders tile into registers and counter rows (T flip-flops, latches,
/// D flip-flops, shift cells, counters). The check itself is generic - every copy must pass its
/// kind's own check and measure what the design measures alone.
fn tiles(c: &Circuit, k: &Kind) -> bool {
    // study 11's tile check (crate::pulsemult::check_tiled) judges each copy with the kind's own
    // checker: study 1's pulse circuits and the sequential kinds tile the same way
    c.tile.is_some() && matches!(k.study(), "pulsemult" | "pulse" | "tff" | "memory" | "delay" | "gates")
}

/// The tiled runs of a study-11 design on redsim ([`crate::pulsemult::check_tiled`]): the copies
/// driven 7 gt apart, then in phase ([`crate::pulsemult::TILE_SHIFTS`]); the first failure fails.
fn tiled_sim(c: &Circuit, k: &Kind, s: &Schedule, margin: i32, alone: &Outcome) -> Outcome {
    let tc = match crate::tiled(c) {
        Ok(tc) => tc,
        Err(e) => return Outcome::fail(format!("{e}"), alone.table.clone()),
    };
    let label = c.tile.map(|t| crate::bud::tile_label(t.step)).unwrap_or_default();
    let mut out = alone.clone();
    for shift in crate::pulsemult::TILE_SHIFTS {
        let ts = crate::pulsemult::tiled_schedule(c, &tc, s, shift);
        out = match run::sim_trace(&tc, &ts, margin) {
            Ok(t) => crate::pulsemult::check_tiled(c, k, &tc, s, &t, alone, shift, &label),
            Err(e) => Outcome::fail(format!("tile: {e}"), alone.table.clone()),
        };
        if !out.pass {
            break;
        }
    }
    out
}

/// The tiled runs of a study-11 design in one build of the game (`rc`: the build's circuit).
fn tiled_check(w: &mut Worker, rc: &Circuit, k: &Kind, s: &Schedule, margin: i32, alone: &Outcome, label: &str) -> Result<Outcome> {
    let tc = match crate::tiled(rc) {
        Ok(tc) => tc,
        Err(e) => return Ok(Outcome::fail(format!("{e}"), alone.table.clone())),
    };
    let mut out = alone.clone();
    for shift in crate::pulsemult::TILE_SHIFTS {
        let ts = crate::pulsemult::tiled_schedule(rc, &tc, s, shift);
        out = match run::game_trace(w, &tc, &ts, margin)? {
            Ok(t) => crate::pulsemult::check_tiled(rc, k, &tc, s, &t, alone, shift, label),
            Err(e) => Outcome::fail(format!("tile: {e}"), alone.table.clone()),
        };
        if !out.pass {
            break;
        }
    }
    Ok(out)
}

/// The judge's consumer lamps observe the outputs, never take part in the circuit (clockinfra
/// 2026-10-20): run again on redsim with every consumer cell left as AIR; every output in such a
/// cell must see the same signal in every frame, else the lamp closes a loop or feeds a block (it
/// is a conductor) and the design would not run with another consumer.
pub fn lamp_part(c: &Circuit, s: &Schedule, margin: i32, with: &Trace) -> Option<String> {
    let cells = c.consumer_cells();
    if cells.is_empty() {
        return None;
    }
    let bare = run::sim_trace_with(c, s, margin, false).ok()?;
    for (k, o) in c.outputs.iter().enumerate() {
        if !cells.contains(&o.pos) {
            continue;
        }
        let (a, b) = (with.levels.get(k)?, bare.levels.get(k)?);
        if let Some(f) = (0..a.len().min(b.len())).find(|&f| a[f] != b[f]) {
            return Some(format!(
                "the judge's lamp is part of the circuit: with output {} left as air its signal differs from frame {f} ({} with the lamp, {} without)",
                o.name, a[f], b[f]
            ));
        }
    }
    None
}

/// The box of every non-air cell over the run, the input cells left out: width x height x depth (a
/// flying machine: the design's own box, not its path).
pub fn dims(c: &Circuit, t: &Trace) -> [i32; 3] {
    if c.kind == Some(Kind::FlyingMachine)
        && let Some((lo, hi)) = c.clock.design.bounds()
    {
        return [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    }
    let mut ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    // the judge's consumers are not the circuit's
    ins.extend(c.consumer_cells());
    // nor are the judge's input devices (R7): a driven input's repeater, support and switch
    for i in &c.inputs {
        if let crate::InputHow::Driven { dir, .. } = i.how {
            let (r, sup, sw) = crate::driver_cells(i.pos, dir);
            ins.extend([r, sup, sw]);
        }
    }
    // nor are the blocks the BUD judge places at the targets and around the design (its probe
    // block there; the design's own blocks moving through those cells count)
    let mut probes: Vec<Pos> = Vec::new();
    let probe_id = c.targets.first().map(|t| t.a.split('[').next().unwrap_or("").to_string()).unwrap_or_default();
    if c.kind == Some(Kind::Bud) {
        ins.extend(c.targets.iter().map(|t| t.pos));
        probes = c.probe_cells();
    }
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for f in &t.cells {
        for (p, _) in f.iter().filter(|(p, s)| !ins.contains(p) && !(probes.contains(p) && t.names[*s as usize].split('[').next() == Some(probe_id.as_str()))) {
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
    /// The checker's verdict, the box and the family, or why the run broke a rule.
    pub result: std::result::Result<BuildOk, String>,
}

/// A build that ran to the end.
#[derive(Clone, Debug)]
pub struct BuildOk {
    pub outcome: Outcome,
    pub dims: [i32; 3],
    /// The kinds of the blocks whose state changes during the run ([`family`]).
    pub family: String,
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
    /// On a position- or rotation-dependent pass: which builds fail (empty otherwise).
    pub note: String,
}

impl VerifyReport {
    /// The reference build: as built at the first position, else the first passing one.
    pub fn base(&self) -> Option<&BuildOk> {
        self.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0]).and_then(|b| b.result.as_ref().ok()).filter(|b| b.outcome.pass).or_else(|| self.builds.iter().find_map(|b| b.result.as_ref().ok().filter(|b| b.outcome.pass)))
    }

    /// `CIRCUIT: <name> <kind> <metrics>, <blocks> blocks, <WxHxD>, robust` or `FAIL: ...`.
    pub fn summary(&self, c: &Circuit) -> String {
        match (&self.kind, self.base()) {
            (Some(k), Some(b)) if self.pass => line(c, k, &b.outcome, b.dims, &self.verdict),
            (Some(k), _) => format!("FAIL: {} {}: {}", self.name, k.name(), self.verdict),
            (None, _) => format!("RUN: {}: {}", self.name, self.verdict),
        }
    }
}

/// The family of a run: the kinds of the blocks whose state changes (or that appear or vanish),
/// input cells left out, as `Clock::kinds` names them (a wall torch is a torch, every copper bulb
/// a copper bulb; piston heads and moving blocks are their piston's), `+`-joined.
pub fn family(c: &Circuit, t: &Trace) -> String {
    let mut ins: std::collections::BTreeSet<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    // the judge's consumers (lamps in air output cells) are not the circuit's
    ins.extend(c.consumer_cells());
    // a temporary kick block's cell is the judge's, not the design's
    if let Some(clock::Kick::Place { pos, .. }) = &c.clock.kick {
        ins.insert(*pos);
    }
    let mut first: std::collections::HashMap<Pos, u32> = std::collections::HashMap::new();
    let mut seen: std::collections::HashMap<Pos, usize> = std::collections::HashMap::new();
    let mut varying: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for f in &t.cells {
        for (p, id) in f {
            *seen.entry(*p).or_default() += 1;
            if *first.entry(*p).or_insert(*id) != *id {
                varying.insert(*p);
            }
        }
    }
    varying.extend(seen.iter().filter(|(_, n)| **n < t.frames()).map(|(p, _)| *p));
    let mut kinds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for f in &t.cells {
        for (p, id) in f {
            if varying.contains(p) && !ins.contains(p) {
                let s = &t.names[*id as usize];
                let k = s.split(['[', '{']).next().unwrap_or(s);
                let k = k.strip_prefix("minecraft:").unwrap_or(k);
                let k = if k == "redstone_wall_torch" {
                    "redstone_torch"
                } else if k.contains("copper_bulb") {
                    "copper_bulb"
                } else {
                    k
                };
                if !matches!(k, "piston_head" | "moving_piston" | "air") {
                    kinds.insert(k.to_string());
                }
            }
        }
    }
    kinds.into_iter().collect::<Vec<_>>().join("+")
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
    // R7: an input the judge cannot drive like a real build refuses the design
    if let Some(why) = &c.input_refused {
        let builds = vec![Build { turns: 0, offset: [0, 0, 0], result: Err(why.clone()) }];
        return Ok(verdict(VerifyReport { name: c.name().to_string(), kind: c.kind.clone(), blocks: c.blocks(), schedule: Schedule::new(c.inputs.len()), builds, verdict: String::new(), pass: false, note: String::new() }));
    }
    if c.kind.as_ref().is_some_and(|k| k.is_shulker()) && o.stimulus.is_none() {
        return crate::shulker::verify_in_pool(pool, c, o);
    }
    let (schedule, kind) = schedule(c, o)?;
    let offsets = clock::offsets(o.positions);
    let turns: Vec<u8> = if o.rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |off| (*t, *off))).collect();
    let results = pool.map(jobs.clone(), |w, _, (t, off)| {
        let rc = c.rotated(w, t)?.translated(off)?;
        let tr = match run::game_trace(w, &rc, &schedule, o.margin)? {
            Ok(tr) => tr,
            Err(why) => return Ok(Err(why)),
        };
        let mut out = match &kind {
            Some(k) => check::check(&rc, k, &schedule, &tr),
            None => Outcome { pass: true, why: String::new(), metrics: Vec::new(), table: tr.strip(&rc, 0, tr.frames()) },
        };
        // R7: the judge's input stones are never moved
        if out.pass
            && kind.is_some()
            && let Some(why) = rc.input_entered(&tr)
        {
            out = Outcome::fail(why, out.table);
        }
        // a BUD's `tile:` claim: the copies side by side in this build (crate::bud::tile_check);
        // the metric names the design's own step (the same in every build)
        if out.pass
            && kind == Some(Kind::Bud)
            && o.stimulus.is_none()
            && let Some(tl) = c.tile
        {
            out = crate::bud::tile_check(&rc, &out, crate::bud::gap_of(&schedule), &crate::bud::tile_label(tl.step), |tc, ts| run::game_trace(w, tc, ts, o.margin))?;
        }
        // a study 12 bit slice: its stacks of 2..4 copies in this build (holds sized on redsim)
        if out.pass
            && o.stimulus.is_none()
            && let (Some(Kind::Compute(spec)), Some(_)) = (&kind, rc.slice)
        {
            let gap0 = crate::compute::gap_of(&schedule);
            out = crate::compute::slice_check(&rc, *spec, &out, |sc| sizing(sc, o.margin).unwrap_or(gap0).max(gap0), |sc, ss| run::game_trace(w, sc, ss, o.margin))?;
        }
        // a tiled study-11 design: its copies side by side, each judged on its own (the metric
        // names the design's own step, the same in every build)
        if out.pass
            && let Some(k) = &kind
            && tiles(&rc, k)
            && let Some(tl) = c.tile
        {
            out = tiled_check(w, &rc, k, &schedule, o.margin, &out, &crate::bud::tile_label(tl.step))?;
        }
        // a sculk design: the device and ambient runs in this build (crate::vibration)
        if out.pass
            && rc.has_sculk()
            && o.stimulus.is_none()
            && let Some(k) = &kind
        {
            out = crate::vibration::check(&rc, k, &out, &schedule, &tr, |vc, vs| run::game_trace(w, vc, vs, o.margin))?;
        }
        Ok(Ok(BuildOk { outcome: out, dims: dims(&rc, &tr), family: family(&rc, &tr) }))
    });
    let mut builds = Vec::new();
    for ((t, off), r) in jobs.into_iter().zip(results) {
        let result = r.with_context(|| format!("rotation {t} at {off:?}"))?;
        builds.push(Build { turns: t, offset: off, result });
    }
    let mut rep = verdict(VerifyReport { name: c.name().to_string(), kind: kind.clone(), blocks: c.blocks(), schedule: schedule.clone(), builds, verdict: String::new(), pass: false, note: String::new() });
    // the judge's lamp must not take part (lamp_part, on redsim: one more run without it)
    if rep.pass
        && kind.is_some()
        && let Ok(t) = run::sim_trace(c, &schedule, o.margin)
        && let Some(why) = lamp_part(c, &schedule, o.margin, &t)
    {
        rep.pass = false;
        rep.verdict = why;
    }
    // TNT dupers (explosions on): every kick angle on redsim (crate::robust; vjeux: "pass every
    // possible random number")
    if rep.pass && c.explodes() {
        let workers = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).min(32);
        if let Err(why) = crate::robust::tnt_robust(c, crate::robust::SAMPLES, workers) {
            rep.pass = false;
            rep.verdict = format!("not robust over every kick (redsim): {why}");
        }
    }
    Ok(rep)
}

/// The verdict of a report's builds: no passing build fails it; builds that disagree (other
/// metrics, or failing) make it position- or rotation-dependent (allowed, flagged, as for clocks;
/// the failing builds are listed in `note`), else `robust`.
pub fn verdict(mut rep: VerifyReport) -> VerifyReport {
    let ok_count = rep.builds.iter().filter(|b| b.result.as_ref().is_ok_and(|b| b.outcome.pass)).count();
    if ok_count == 0 {
        let b = &rep.builds[0];
        rep.verdict = match &b.result {
            Ok(b) => b.outcome.why.clone(),
            Err(e) => e.clone(),
        };
        return rep;
    }
    // as for clocks: builds that disagree (other metrics, or failing) make the circuit position-
    // or rotation-dependent (allowed, flagged; the failing builds are listed in `note`)
    let sig = |b: &Build| match &b.result {
        Ok(k) if k.outcome.pass => k.outcome.metrics_text(),
        _ => "FAIL".to_string(),
    };
    let base_build = rep.builds.iter().find(|b| b.turns == 0 && b.offset == [0, 0, 0] && sig(b) != "FAIL").or_else(|| rep.builds.iter().find(|b| sig(b) != "FAIL")).unwrap();
    let base = sig(base_build);
    let pos_dep = rep.builds.iter().filter(|b| b.turns == 0).any(|b| sig(b) != base);
    let rot_dep = rep.builds.iter().filter(|b| b.turns != 0).any(|b| sig(b) != base);
    rep.verdict = match (pos_dep, rot_dep) {
        (false, false) => "robust".into(),
        (true, _) => "position-dependent".into(),
        (false, true) => "rotation-dependent".into(),
    };
    let fails: Vec<String> = rep
        .builds
        .iter()
        .filter(|b| sig(b) == "FAIL")
        .map(|b| {
            let why = match &b.result {
                Ok(k) => k.outcome.why.clone(),
                Err(e) => e.clone(),
            };
            format!("rotation {} at {:?}: {why}", b.turns, b.offset)
        })
        .collect();
    if !fails.is_empty() {
        rep.note = format!("{} of {} builds fail: {}", fails.len(), rep.builds.len(), fails[0]);
    } else if let Some(b) = rep.builds.iter().find(|b| sig(b) != base) {
        // no build fails but they disagree: name the first build whose metrics differ
        rep.note = format!("rotation {} at {:?}: {} (base build: {base})", b.turns, b.offset, sig(b));
    }
    rep.pass = true;
    rep
}
