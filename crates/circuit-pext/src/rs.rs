//! The redsim runner for piston extenders (fast screen; the real game is [`crate::game`]).
//!
//! Placement like the door/clock judge (raw writes, a shape update of every wire, a `self`
//! update of every block), then settle: the settled state is the REST state. A cycle:
//! * lever: one right-click (on), run until idle: the payload must be placed at
//!   `payload + N * travel` from some tick on until the end (`extend` = that tick, counted from the
//!   click: the first tick after it is 1); a second right-click (off), run until idle: the whole
//!   state must be the rest state again (`retract` = the tick of the last change);
//! * button: one press, run until idle (the button releases itself): the payload must reach the
//!   target (placed) at some tick (`extend`, `hold` = ticks it stays) and the whole state must end
//!   at rest (`retract` = the tick of the last change, counted from the press).
//!
//! Every cycle must give the same numbers; no drops (a popped block), nothing unmodeled.

use crate::spec::{InputKind, Pext, Pos};
use redsim::{Block, BlockPos, World};

#[derive(Clone, Debug)]
pub struct Prepared {
    pub name: String,
    pub blocks: Vec<(BlockPos, Block)>,
    pub input: BlockPos,
    pub kind: InputKind,
    pub payload: BlockPos,
    pub payload_block: Block,
    pub target: BlockPos,
    pub min: BlockPos,
    pub size: [i32; 3],
    /// Exploration only (`ideal:` header): scripted writes (phase "on"/"off"/"both", tick, cell,
    /// state; air = clear), applied with updates right before that tick of the phase.
    pub ideal: Vec<(String, u32, BlockPos, Block)>,
}

/// `ideal: both 5 set 4 1 1 minecraft:redstone_block; both 7 clear 4 1 1` (phases on, off, both).
pub fn parse_ideal(s: &str) -> Result<Vec<(String, u32, BlockPos, Block)>, String> {
    let mut out = Vec::new();
    for item in s.split(';').map(str::trim).filter(|x| !x.is_empty()) {
        let w: Vec<&str> = item.split_whitespace().collect();
        let bad = || format!("ideal item {item:?}: <on|off|both> <tick> set x y z <state> | clear x y z");
        if w.len() < 6 {
            return Err(bad());
        }
        let t: u32 = w[1].parse().map_err(|_| bad())?;
        let n = |i: usize| w[i].parse::<i32>().map_err(|_| bad());
        let p = BlockPos::new(n(3)?, n(4)?, n(5)?);
        let b = match (w[2], w.get(6)) {
            ("set", Some(st)) => Block::parse(st)?,
            ("clear", None) => Block::Air,
            _ => return Err(bad()),
        };
        out.push((w[0].to_string(), t, p, b));
    }
    Ok(out)
}

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

impl Prepared {
    /// The design as redsim blocks in a world of its box (target included) plus `margin`; Err =
    /// what redsim does not model.
    pub fn new(p: &Pext, margin: i32) -> Result<Prepared, String> {
        let mut blocks = Vec::new();
        let mut bad: Vec<String> = Vec::new();
        for (q, s) in p.design.block_list() {
            match Block::parse(&s) {
                Ok(b) => blocks.push((bp(q), b)),
                Err(e) => {
                    if !bad.contains(&e) {
                        bad.push(e)
                    }
                }
            }
        }
        if !bad.is_empty() {
            return Err(bad.join("; "));
        }
        let payload_block = Block::parse(&p.payload_state).map_err(|e| e.to_string())?;
        let (lo, hi) = p.bounds();
        Ok(Prepared {
            name: p.name().to_string(),
            blocks,
            input: bp(p.input),
            kind: p.input_kind,
            payload: bp(p.payload),
            payload_block,
            target: bp(p.target()),
            min: BlockPos::new(lo[0] - margin, lo[1] - margin, lo[2] - margin),
            size: [hi[0] - lo[0] + 1 + 2 * margin, hi[1] - lo[1] + 1 + 2 * margin, hi[2] - lo[2] + 1 + 2 * margin],
            ideal: match p.design.meta.get("ideal") {
                Some(s) => parse_ideal(s)?,
                None => Vec::new(),
            },
        })
    }
}

#[derive(Clone, Debug)]
pub struct Params {
    pub cycles: usize,
    /// Ticks a phase may take before it counts as never settling.
    pub max_phase: u32,
    /// Ticks the placement may take to settle.
    pub max_settle: u32,
    /// Snapshot every tick (the retract tick); off = a fast screen (retract reads 0).
    pub track: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params { cycles: 3, max_phase: 2000, max_settle: 400, track: true }
    }
}

impl Params {
    /// The search screen: 2 cycles, short phases, no per-tick snapshots.
    pub fn screen() -> Params {
        Params { cycles: 2, max_phase: 120, max_settle: 60, track: false }
    }
}

/// One cycle's numbers (ticks counted from the input event; the first tick after it is 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Cycle {
    /// The payload placed at the target from this tick on (lever: until the off click).
    pub extend: u32,
    /// Lever: the on phase is idle after this tick. Button: the run is idle after this tick.
    pub settle: u32,
    /// Lever: ticks after the off click until the last change (the rest state from then on).
    /// Button: ticks after the press until the last change.
    pub retract: u32,
    /// Ticks the payload stays at the target (button; lever: until the off click).
    pub hold: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Works,
    Fails,
    Unmodeled,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Works => "WORKS",
            Verdict::Fails => "FAILS",
            Verdict::Unmodeled => "UNMODELED",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Report {
    pub verdict: Verdict,
    pub why: String,
    pub cycles: Vec<Cycle>,
    pub warnings: Vec<String>,
    /// With `track`: the box of every non-air cell over the first cycle, the input left out (the
    /// game judge's `dims`: moving blocks and extended heads count).
    pub run_box: Option<([i32; 3], [i32; 3])>,
}

impl Report {
    fn fail(v: Verdict, why: String) -> Report {
        Report { verdict: v, why, cycles: Vec::new(), warnings: Vec::new(), run_box: None }
    }

    pub fn works(&self) -> bool {
        self.verdict == Verdict::Works
    }

    pub fn first(&self) -> Cycle {
        self.cycles.first().copied().unwrap_or_default()
    }
}

/// One phase's per-tick record.
struct Phase {
    /// `at[t - 1]`: the payload placed at the target after tick t.
    at: Vec<bool>,
    /// `changed[t - 1]`: some cell differs after tick t from after tick t - 1.
    changed: Vec<bool>,
    /// With tracking: the box of the non-air cells seen (the input left out).
    lo: [i32; 3],
    hi: [i32; 3],
}

impl Phase {
    fn ticks(&self) -> u32 {
        self.at.len() as u32
    }

    /// The tick of the last change (0 = nothing changed).
    fn last_change(&self) -> u32 {
        self.changed.iter().rposition(|c| *c).map(|i| i as u32 + 1).unwrap_or(0)
    }
}

fn short(b: &Block) -> String {
    let s = b.to_string();
    s.strip_prefix("minecraft:").unwrap_or(&s).to_string()
}

fn check_world(w: &World, t: u32, what: &str) -> Result<(), (Verdict, String)> {
    if let Some(e) = w.errors.first() {
        let v = if e.contains("outside the box") { Verdict::Fails } else { Verdict::Unmodeled };
        return Err((v, format!("{what} tick {t}: {e}")));
    }
    if let Some((p, b)) = w.drops.first() {
        return Err((Verdict::Fails, format!("{what} tick {t}: {} at {p} was destroyed with a drop", short(b))));
    }
    Ok(())
}

fn run_phase(w: &mut World, prep: &Prepared, params: &Params, what: &str, trace: &mut Option<Vec<String>>) -> Result<Phase, (Verdict, String)> {
    let max = params.max_phase;
    let track = params.track || trace.is_some();
    let mut prev = if track { w.blocks() } else { Vec::new() };
    let mut ph = Phase { at: Vec::new(), changed: Vec::new(), lo: [i32::MAX; 3], hi: [i32::MIN; 3] };
    let grow = |ph: &mut Phase, cells: &[(BlockPos, Block)]| {
        for (p, _) in cells.iter().filter(|(p, _)| *p != prep.input) {
            for (i, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                ph.lo[i] = ph.lo[i].min(v);
                ph.hi[i] = ph.hi[i].max(v);
            }
        }
    };
    if track {
        grow(&mut ph, &prev);
    }
    for t in 1..=max {
        for (when, at, p, b) in &prep.ideal {
            if *at == t && (when == what || when == "both" || (when == "on" && what == "press")) {
                w.set_block(*p, *b, redsim::world::flags::ALL);
            }
        }
        w.tick();
        check_world(w, t, what)?;
        // extended = the payload placed at the target with nothing moving (no moving_piston
        // anywhere: the judge's definition, docs/circuits.md)
        ph.at.push(w.get(prep.target) == prep.payload_block && w.piston.all().next().is_none());
        if track {
            let now = w.blocks();
            grow(&mut ph, &now);
            ph.changed.push(now != prev);
            if let Some(lines) = trace {
                lines.push(format!("{what} t={t}: {}", diff(&prev, &now)));
            }
            prev = now;
        } else {
            ph.changed.push(false);
        }
        let pending_ideal = prep.ideal.iter().any(|(when, at, _, _)| *at > t && (when == what || when == "both" || (when == "on" && what == "press")));
        if w.idle() && !pending_ideal {
            return Ok(ph);
        }
    }
    Err((Verdict::Fails, format!("{what}: still active after {max} gt")))
}

/// The cells that differ between two snapshots, as `x,y,z state` items.
pub fn diff(a: &[(BlockPos, Block)], b: &[(BlockPos, Block)]) -> String {
    let am: std::collections::BTreeMap<(i32, i32, i32), Block> = a.iter().map(|(p, s)| ((p.x, p.y, p.z), *s)).collect();
    let bm: std::collections::BTreeMap<(i32, i32, i32), Block> = b.iter().map(|(p, s)| ((p.x, p.y, p.z), *s)).collect();
    let mut keys: Vec<(i32, i32, i32)> = am.keys().chain(bm.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    let mut out = Vec::new();
    for k in keys {
        let x = am.get(&k).copied().unwrap_or(Block::Air);
        let y = bm.get(&k).copied().unwrap_or(Block::Air);
        if x != y {
            out.push(format!("{},{},{} {}", k.0, k.1, k.2, short(&y)));
        }
    }
    if out.is_empty() { "-".into() } else { out.join("; ") }
}

/// Place like the judge and settle. Returns the world and its rest snapshot.
pub fn place(prep: &Prepared, params: &Params) -> Result<(World, Vec<(BlockPos, Block)>), (Verdict, String)> {
    let mut w = World::new(prep.min, prep.size);
    w.place_like_judge(&prep.blocks);
    let n = w.run_until_idle(params.max_settle);
    check_world(&w, n, "placement")?;
    if !w.idle() {
        return Err((Verdict::Fails, format!("the placement never settles ({} gt)", params.max_settle)));
    }
    let rest = w.blocks();
    if w.get(prep.payload) != prep.payload_block {
        return Err((Verdict::Fails, "the payload is not at its cell at rest".into()));
    }
    // the support rule: every block can stand where it is (a lever or a torch on a sturdy face,
    // dust and diodes on a floor): a player could build it
    if let Some((p, b)) = rest.iter().find(|(p, b)| !redsim::behavior::can_survive(&w, b, *p)) {
        return Err((Verdict::Fails, format!("{} at {},{},{} has no support (it could not be built)", short(b), p.x, p.y, p.z)));
    }
    Ok((w, rest))
}

/// The judge on redsim (see the module doc); with `trace`, the per-tick changes of the first
/// cycle (and the placement) are returned too.
pub fn judge(prep: &Prepared, params: &Params, want_trace: bool) -> (Report, Vec<String>) {
    let mut trace: Option<Vec<String>> = if want_trace { Some(Vec::new()) } else { None };
    let r = judge_inner(prep, params, &mut trace);
    (r, trace.unwrap_or_default())
}

fn judge_inner(prep: &Prepared, params: &Params, trace: &mut Option<Vec<String>>) -> Report {
    let (mut w, rest) = match place(prep, params) {
        Ok(x) => x,
        Err((v, why)) => return Report::fail(v, why),
    };
    let mut warnings = Vec::new();
    let differs: Vec<String> = prep.blocks.iter().filter(|(p, b)| w.get(*p) != *b).map(|(p, b)| format!("{},{},{} {} -> {}", p.x, p.y, p.z, short(b), short(&w.get(*p)))).collect();
    if !differs.is_empty() {
        warnings.push(format!("the settled rest state differs from the design: {}", differs.join("; ")));
    }
    let mut cycles = Vec::new();
    let mut bx: Bx = ([i32::MAX; 3], [i32::MIN; 3]);
    for c in 0..params.cycles {
        let mut tr = if c == 0 { trace.take() } else { None };
        let mut cb: Bx = ([i32::MAX; 3], [i32::MIN; 3]);
        let r = match prep.kind {
            InputKind::Lever => lever_cycle(&mut w, prep, params, &rest, &mut tr, &mut cb),
            InputKind::Button => button_cycle(&mut w, prep, params, &rest, &mut tr, &mut cb),
        };
        if c == 0 {
            *trace = tr;
            bx = cb;
        }
        match r {
            Ok(cy) => cycles.push(cy),
            Err((v, why)) => {
                let mut rep = Report::fail(v, if params.cycles > 1 { format!("cycle {}: {why}", c + 1) } else { why });
                rep.cycles = cycles;
                rep.warnings = warnings;
                return rep;
            }
        }
    }
    if cycles.windows(2).any(|p| p[0] != p[1]) {
        warnings.push(format!("the cycles differ: {:?}", cycles));
    }
    let run_box = (params.track && bx.0[0] <= bx.1[0]).then_some(bx);
    Report { verdict: Verdict::Works, why: String::new(), cycles, warnings, run_box }
}

/// A box (lo, hi).
type Bx = ([i32; 3], [i32; 3]);

fn widen(bx: &mut Bx, ph: &Phase) {
    for i in 0..3 {
        bx.0[i] = bx.0[i].min(ph.lo[i]);
        bx.1[i] = bx.1[i].max(ph.hi[i]);
    }
}

fn use_input(w: &mut World, prep: &Prepared, what: &str) -> Result<(), (Verdict, String)> {
    if !w.use_block(prep.input) {
        return Err((Verdict::Unmodeled, format!("{what}: a right-click on {} is not modeled", short(&w.get(prep.input)))));
    }
    Ok(())
}

fn count_payload(w: &World, prep: &Prepared) -> usize {
    w.blocks().iter().filter(|(_, b)| *b == prep.payload_block).count()
}

fn lever_cycle(w: &mut World, prep: &Prepared, params: &Params, rest: &[(BlockPos, Block)], trace: &mut Option<Vec<String>>, bx: &mut Bx) -> Result<Cycle, (Verdict, String)> {
    use_input(w, prep, "on")?;
    let on = run_phase(w, prep, params, "on", trace)?;
    widen(bx, &on);
    let n = on.ticks();
    if !on.at.last().copied().unwrap_or(false) {
        let at = w.blocks().into_iter().find(|(_, b)| *b == prep.payload_block).map(|(p, _)| format!("{},{},{}", p.x, p.y, p.z)).unwrap_or_else(|| "nowhere".into());
        return Err((Verdict::Fails, format!("on: settled after {n} gt with the payload at {at}, not at the target {},{},{}", prep.target.x, prep.target.y, prep.target.z)));
    }
    if count_payload(w, prep) != 1 {
        return Err((Verdict::Fails, "on: the payload's state appears more than once".into()));
    }
    let extend = on.at.iter().rposition(|a| !*a).map(|i| i as u32 + 2).unwrap_or(1);
    // the judge's rule: once extended (payload at the target, nothing moving), it must stay
    if let Some(first) = on.at.iter().position(|a| *a)
        && let Some(left) = on.at[first..].iter().position(|a| !*a)
    {
        return Err((Verdict::Fails, format!("on: extended at +{} but the payload leaves the target at +{} while the input is still on", first + 1, first + left + 1)));
    }
    use_input(w, prep, "off")?;
    let off = run_phase(w, prep, params, "off", trace)?;
    widen(bx, &off);
    let now = w.blocks();
    if now != rest {
        return Err((Verdict::Fails, format!("off: settled after {} gt but not at rest: {}", off.ticks(), diff(rest, &now))));
    }
    Ok(Cycle { extend, settle: n, retract: off.last_change(), hold: n + 1 - extend })
}

fn button_cycle(w: &mut World, prep: &Prepared, params: &Params, rest: &[(BlockPos, Block)], trace: &mut Option<Vec<String>>, bx: &mut Bx) -> Result<Cycle, (Verdict, String)> {
    use_input(w, prep, "press")?;
    let ph = run_phase(w, prep, params, "press", trace)?;
    widen(bx, &ph);
    let Some(first) = ph.at.iter().position(|a| *a) else {
        return Err((Verdict::Fails, "press: the payload never reached the target".into()));
    };
    let hold = ph.at[first..].iter().take_while(|a| **a).count() as u32;
    let now = w.blocks();
    if now != rest {
        return Err((Verdict::Fails, format!("press: settled after {} gt but not at rest: {}", ph.ticks(), diff(rest, &now))));
    }
    Ok(Cycle { extend: first as u32 + 1, settle: ph.ticks(), retract: ph.last_change(), hold })
}

/// How many of the lever-flip tests of [`flips`] fail (each flip and each reflip, then the full
/// cycle), every test from a fresh placement: a soft objective for searches (`anneal --flip-soft`).
pub fn flip_failures(prep: &Prepared, params: &Params) -> usize {
    if prep.kind != InputKind::Lever {
        return 0;
    }
    let mut bad = 0;
    for &k in LEVER_FLIPS.iter().rev() {
        for kind in ["off", "on"] {
            let ok = flip_trace_inner(prep, params, kind, k, &mut None).is_ok_and(|rest| rest);
            if !ok {
                bad += 1;
            }
        }
    }
    bad
}

/// One flip of [`flip_trace`] from a fresh placement: `Ok(true)` when the rest state came back
/// (and, for `on`, it extended again before the last off).
fn flip_trace_inner(prep: &Prepared, params: &Params, kind: &str, k: u32, lines: &mut Option<Vec<String>>) -> Result<bool, (Verdict, String)> {
    let (mut w, rest) = place(prep, params)?;
    let ticks = |w: &mut World, n: u32, what: &str, lines: &mut Option<Vec<String>>| -> Result<(), (Verdict, String)> {
        let mut prev = if lines.is_some() { w.blocks() } else { Vec::new() };
        for t in 1..=n {
            w.tick();
            check_world(w, t, what)?;
            if let Some(l) = lines.as_mut() {
                let now = w.blocks();
                l.push(format!("{what} t={t}: {}", diff(&prev, &now)));
                prev = now;
            }
        }
        Ok(())
    };
    match kind {
        "off" => {
            use_input(&mut w, prep, "on")?;
            ticks(&mut w, k, "on", lines)?;
            use_input(&mut w, prep, "off")?;
            run_phase(&mut w, prep, params, "off", lines)?;
        }
        _ => {
            use_input(&mut w, prep, "on")?;
            let on = run_phase(&mut w, prep, params, "on", lines)?;
            if !on.at.last().copied().unwrap_or(false) {
                return Ok(false);
            }
            use_input(&mut w, prep, "off")?;
            ticks(&mut w, k, "off", lines)?;
            use_input(&mut w, prep, "on")?;
            let again = run_phase(&mut w, prep, params, "on again", lines)?;
            if !again.at.last().copied().unwrap_or(false) {
                return Ok(false);
            }
            use_input(&mut w, prep, "off")?;
            run_phase(&mut w, prep, params, "off again", lines)?;
        }
    }
    Ok(w.blocks() == rest)
}

/// One lever flip of [`flips`] traced from the placement, every cell change per tick: `off:K`
/// (on, K gt, off, until idle) or `on:K` (on until idle, off, K gt, on until idle, off until
/// idle). Ends with whether the rest state came back.
pub fn flip_trace(prep: &Prepared, params: &Params, flip: &str) -> Result<Vec<String>, String> {
    let (kind, k) = flip.split_once(':').ok_or("--flip wants off:K or on:K")?;
    let k: u32 = k.parse().map_err(|_| "--flip wants off:K or on:K")?;
    let mut lines: Option<Vec<String>> = Some(Vec::new());
    let r = flip_trace_inner(prep, params, kind, k, &mut lines);
    let mut out = lines.unwrap_or_default();
    out.push(match r {
        Ok(true) => "at rest".to_string(),
        Ok(false) => "NOT at rest (or not extended again)".to_string(),
        Err((_, why)) => format!("FAILS: {why}"),
    });
    Ok(out)
}

/// The lever flips of the real-use audit (clk-misc 2026-10-21, `circuit::check::LEVER_FLIPS`: no
/// hand flips a lever back within 4 gt).
pub const LEVER_FLIPS: [u32; 8] = [4, 5, 6, 8, 10, 12, 16, 20];

static FLIP_SAFE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Searches (`grow`, `topo`, `anneal` with `--flip-safe`) require the lever flips too.
pub fn require_flips(on: bool) {
    FLIP_SAFE.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Whether a candidate passes the lever flips when the search requires them (always otherwise).
pub fn flips_ok(prep: &Prepared, params: &Params) -> bool {
    !FLIP_SAFE.load(std::sync::atomic::Ordering::Relaxed) || flips(prep, params).is_none()
}

/// The lever flipped back too soon (the real-use audit's `flips` metric, on redsim): for every k
/// of [`LEVER_FLIPS`], longest first, on k gt then off must bring the rest state back; then on
/// (extended), off k gt, on again must extend again, and off bring the rest state back; then a
/// full cycle must work. `None` when safe (a button design: always), else the first that breaks
/// it, in the audit's words.
pub fn flips(prep: &Prepared, params: &Params) -> Option<String> {
    if prep.kind != InputKind::Lever {
        return None;
    }
    let (mut w, rest) = match place(prep, params) {
        Ok(x) => x,
        Err((_, why)) => return Some(format!("placement: {why}")),
    };
    let tick_k = |w: &mut World, k: u32, what: &str| -> Result<(), (Verdict, String)> {
        for t in 1..=k {
            w.tick();
            check_world(w, t, what)?;
        }
        Ok(())
    };
    for &k in LEVER_FLIPS.iter().rev() {
        let r = (|| -> Result<bool, (Verdict, String)> {
            use_input(&mut w, prep, "on")?;
            tick_k(&mut w, k, "on")?;
            use_input(&mut w, prep, "off")?;
            run_phase(&mut w, prep, params, "off", &mut None)?;
            Ok(w.blocks() == rest)
        })();
        match r {
            Ok(true) => {}
            Ok(false) => return Some(format!("breaks: off {k} gt after on strands blocks")),
            Err((_, why)) => return Some(format!("breaks: off {k} gt after on: {why}")),
        }
    }
    for &k in LEVER_FLIPS.iter().rev() {
        let r = (|| -> Result<Option<&'static str>, (Verdict, String)> {
            use_input(&mut w, prep, "on")?;
            let on = run_phase(&mut w, prep, params, "on", &mut None)?;
            if !on.at.last().copied().unwrap_or(false) {
                return Ok(Some("not extended before the first off"));
            }
            use_input(&mut w, prep, "off")?;
            tick_k(&mut w, k, "off")?;
            use_input(&mut w, prep, "on")?;
            let again = run_phase(&mut w, prep, params, "on", &mut None)?;
            if !again.at.last().copied().unwrap_or(false) {
                return Ok(Some("not extended again"));
            }
            use_input(&mut w, prep, "off")?;
            run_phase(&mut w, prep, params, "off", &mut None)?;
            Ok((w.blocks() != rest).then_some("the rest state does not come back"))
        })();
        match r {
            Ok(None) => {}
            Ok(Some(_)) => return Some(format!("breaks: on again {k} gt after off strands blocks")),
            Err((_, why)) => return Some(format!("breaks: on again {k} gt after off: {why}")),
        }
    }
    let mut bx: Bx = ([i32::MAX; 3], [i32::MIN; 3]);
    match lever_cycle(&mut w, prep, params, &rest, &mut None, &mut bx) {
        Ok(_) => None,
        Err((_, why)) => Some(format!("breaks: a full cycle after the flips fails ({why})")),
    }
}
