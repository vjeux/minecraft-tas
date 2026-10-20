//! `mctas door ideal <schedule>`: run a mechanism with *ideal* inputs (blocks set and cleared by
//! script at chosen ticks after each lever flip) through open/close cycles at several world
//! positions, and judge it like the verifier does (open: door + walkway walkable; closed: every
//! block back where it was, no entities). `--sweep name=lo..hi` re-runs it for every value of
//! one schedule variable (in parallel), which is how the timing tolerances of a mechanism are
//! measured before any wiring exists.
//!
//! Schedule format (`#` comments):
//! ```text
//! design designs/piston-door-3x3/tracks/logic-min/ideal-mech.txt   # rest state, `door` port
//! let a_off = 5                     # a variable; --set a_off=3 overrides it
//! open a_off clear 1 4 -1           # after a_off ticks of the opening, remove that block
//! open b_on set 1 6 0 minecraft:redstone_block
//! open b_on+8 clear 1 6 0           # ticks are sums/differences of integers and variables
//! close 3 set 1 4 -1 minecraft:redstone_block
//! open 0 use -2 4 0                 # right-click a block of the design (its real lever), so
//!                                   # real wiring and ideal inputs can be mixed
//! ```
//! An event at tick `t` is applied after `t` game ticks of its phase (0 = at the flip), before
//! the next tick runs, like a player acting between ticks: that was the only rule until
//! 2026-10-19 (`phase between`, `--between-ticks`). `door ideal` and `door ideal-rs` now default
//! to `phase tile` (other tools that parse plans keep the old rule unless the file says `phase
//! tile`): an off-plane redstone block input set or cleared at `t >= 2` acts in the TILE-TICK
//! phase of tick `t`,
//! where an observer's (repeater's, torch's) output arrives in the game: plans were 1 gt
//! optimistic for observer-driven inputs (a plan block appeared between ticks, after tick `t`;
//! a wired observer firing at `t` powers the piston inside tick `t`, before its block events).
//! It is built from real blocks, so `door ideal` (the game) and `door ideal-rs` (redsim) run it
//! alike: the input cell holds a wall redstone torch on a support one cell further off the plane;
//! the support is a redstone block while the input is off (torch unlit) and turns to stone 2 gt
//! before `t` (the torch lights in tick `t`'s tile ticks, and powers the piston and its QC cell
//! like the plan block did); `clear` at `t` puts the redstone block back at `t - 2`. Inputs with
//! an event before tick 2 (lever-adjacent, between ticks like the lever), in-plane inputs, other
//! states, more than 3 pulses per edge (a torch would burn out) or a taken support cell keep the
//! old rule (`door ideal` prints which).

/// Marker state of a `use` event.
const USE: &str = "@use";

/// When a plan's inputs act (see the module doc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanPhase {
    /// In the tile-tick phase of tick `t` (default for `.ideal` files).
    Tile,
    /// Between ticks, after tick `t` (the old rule; plans built in code).
    Between,
}

/// A resolved event: (tick, pos, Some(state) = set / None = clear; `@use` = right-click).
pub type Ev = (u32, Pos, Option<String>);

/// The phase rule of schedules parsed in this process: 0 = library default (between ticks unless
/// the file says `phase tile`: tools that build or replay plans keep their rule), 1 = `door ideal`
/// / `door ideal-rs` (tile unless the file says `phase between`), 2 = `--between-ticks` (every
/// schedule between ticks).
static CLI_PHASE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// `door ideal` / `door ideal-rs`: tile phase by default, or every schedule between ticks.
pub fn cli_phase(between_ticks: bool) {
    CLI_PHASE.store(if between_ticks { 2 } else { 1 }, std::sync::atomic::Ordering::Relaxed);
}

use crate::DoorSpec;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pool, Pos, SandboxOptions, Worker, WorkerConfig};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
struct Event {
    open: bool,
    tick: String,
    pos: Pos,
    /// None = clear (air).
    state: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Schedule {
    design: Design,
    vars: BTreeMap<String, i64>,
    events: Vec<Event>,
    /// When the inputs act: `phase tile|between` in the file, default tile; `from_events` builds
    /// between-ticks schedules (the tools that build plans in code keep their rule).
    pub phase: PlanPhase,
}

fn eval(expr: &str, vars: &BTreeMap<String, i64>) -> Result<i64> {
    // a+b-c with integers and variable names, no spaces.
    let mut total = 0i64;
    let mut sign = 1i64;
    let mut tok = String::new();
    let flush = |tok: &mut String, sign: i64, total: &mut i64| -> Result<()> {
        if tok.is_empty() {
            return Ok(());
        }
        let v = match tok.parse::<i64>() {
            Ok(n) => n,
            Err(_) => *vars.get(tok.as_str()).with_context(|| format!("unknown variable {tok:?}"))?,
        };
        *total += sign * v;
        tok.clear();
        Ok(())
    };
    for c in expr.chars() {
        match c {
            '+' | '-' => {
                flush(&mut tok, sign, &mut total)?;
                sign = if c == '+' { 1 } else { -1 };
            }
            c if c.is_whitespace() => {}
            c => tok.push(c),
        }
    }
    flush(&mut tok, sign, &mut total)?;
    Ok(total)
}

impl Schedule {
    /// The rest-state design (ideal inputs present at rest included).
    pub fn design(&self) -> &Design {
        &self.design
    }
    /// The schedule's own variables (`let`).
    pub fn vars(&self) -> &BTreeMap<String, i64> {
        &self.vars
    }
    /// Resolved events of one phase: (tick, pos, Some(state) = set / None = clear), sorted by
    /// tick. `use` events carry the state `@use`.
    pub fn events_of(&self, open: bool, vars: &BTreeMap<String, i64>) -> Result<Vec<(u32, Pos, Option<String>)>> {
        self.phase(open, vars)
    }
    /// Cells the schedule sets or clears (its ideal inputs).
    pub fn input_cells(&self) -> Vec<Pos> {
        let mut v: Vec<Pos> = self.events.iter().filter(|e| e.state.as_deref() != Some(USE)).map(|e| e.pos).collect();
        v.sort();
        v.dedup();
        v
    }

    /// A schedule built in code: (open?, tick, pos, Some(state) = set / None = clear).
    pub fn from_events(design: Design, events: &[(bool, u32, Pos, Option<String>)]) -> Schedule {
        let events = events
            .iter()
            .map(|(open, t, p, s)| Event { open: *open, tick: t.to_string(), pos: *p, state: s.clone() })
            .collect();
        Schedule { design, vars: BTreeMap::new(), events, phase: PlanPhase::Between }
    }

    pub fn parse(text: &str) -> Result<Schedule> {
        Self::parse_with(text, &BTreeMap::new())
    }

    /// A plan's text with every event's tick moved: events on a cell listed in `shifts` by its
    /// shift, the others by `rest`. Ticks are evaluated with the plan's `let`s and written as
    /// numbers; other lines are kept. Fails on a tick that would become negative. For trying a
    /// re-timed plan (one area started later than the lever, say) without editing it by hand.
    pub fn shift_text(text: &str, shifts: &[(Pos, i64)], rest: i64) -> Result<String> {
        let mut vars = BTreeMap::new();
        let mut out = String::new();
        for (i, raw) in text.lines().enumerate() {
            let (body, comment) = match raw.split_once('#') {
                Some((b, c)) => (b, Some(c)),
                None => (raw, None),
            };
            let p: Vec<&str> = body.split_whitespace().collect();
            let ctx = || format!("line {}: {raw:?}", i + 1);
            if p.first() == Some(&"let") {
                let (k, v) = body.trim()[3..].split_once('=').with_context(ctx)?;
                vars.insert(k.trim().to_string(), eval(v.trim(), &vars).with_context(ctx)?);
            }
            if p.len() >= 6 && (p[0] == "open" || p[0] == "close") {
                let int = |s: &str| s.parse::<i32>().with_context(ctx);
                let pos = [int(p[3])?, int(p[4])?, int(p[5])?];
                let k = shifts.iter().find(|(c, _)| *c == pos).map_or(rest, |(_, k)| *k);
                let t = eval(p[1], &vars).with_context(ctx)? + k;
                anyhow::ensure!(t >= 0, "tick {t} < 0 after the shift: {}", ctx());
                let tick = t.to_string();
                let mut q = p.clone();
                q[1] = &tick;
                out += &q.join(" ");
                if let Some(c) = comment {
                    out += " #";
                    out += c;
                }
            } else {
                out += raw;
            }
            out.push('\n');
        }
        Ok(out)
    }

    /// [`Schedule::parse`] with variables set from outside (`--set`): a `let` of an overridden
    /// name keeps the override, and later `let`s built from it see it.
    pub fn parse_with(text: &str, overrides: &BTreeMap<String, i64>) -> Result<Schedule> {
        let mut design = None;
        let mut vars = overrides.clone();
        let mut events = Vec::new();
        let cli = CLI_PHASE.load(std::sync::atomic::Ordering::Relaxed);
        let mut phase = if cli == 1 { PlanPhase::Tile } else { PlanPhase::Between };
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let ctx = || format!("line {}: {raw:?}", i + 1);
            let p: Vec<&str> = line.split_whitespace().collect();
            match p[0] {
                "design" => design = Some(crate::cli::load_design(std::path::Path::new(p[1])).with_context(ctx)?),
                "phase" => {
                    phase = match p.get(1).copied() {
                        Some("tile") => PlanPhase::Tile,
                        Some("between") => PlanPhase::Between,
                        _ => bail!("expected 'phase tile' or 'phase between': {}", ctx()),
                    }
                }
                "let" => {
                    // let name = value (an integer, or a sum/difference of earlier variables)
                    let rest = line[3..].trim();
                    let (k, v) = rest.split_once('=').with_context(ctx)?;
                    if !overrides.contains_key(k.trim()) {
                        let v = eval(v.trim(), &vars).with_context(ctx)?;
                        vars.insert(k.trim().to_string(), v);
                    }
                }
                "open" | "close" => {
                    if p.len() < 6 {
                        bail!("expected '<open|close> <tick> <set|clear> x y z [state]': {}", ctx());
                    }
                    let int = |s: &str| s.parse::<i32>().with_context(ctx);
                    let pos = [int(p[3])?, int(p[4])?, int(p[5])?];
                    let state = match p[2] {
                        "set" => Some(p.get(6).with_context(ctx)?.to_string()),
                        "clear" => None,
                        // Right-click the block (a real lever or button in the design).
                        "use" => Some(USE.to_string()),
                        other => bail!("unknown action {other}: {}", ctx()),
                    };
                    events.push(Event { open: p[0] == "open", tick: p[1].to_string(), pos, state });
                }
                other => bail!("unknown schedule command {other}: {}", ctx()),
            }
        }
        if cli == 2 {
            phase = PlanPhase::Between;
        }
        Ok(Schedule { design: design.context("schedule has no 'design' line")?, vars, events, phase })
    }

    /// The design and both phases' events as they run under this schedule's phase rule (see the
    /// module doc), plus a note per input kept between ticks.
    pub fn realize(&self, vars: &BTreeMap<String, i64>) -> Result<(Design, Vec<Ev>, Vec<Ev>, Vec<String>)> {
        let open = self.phase(true, vars)?;
        let close = self.phase(false, vars)?;
        let mut design = self.design.clone();
        if self.phase == PlanPhase::Between {
            return Ok((design, open, close, Vec::new()));
        }
        let along_z = design.meta.get("door-axis").is_some_and(|v| v == "z");
        let mut notes = Vec::new();
        let mut cells: Vec<Pos> = open.iter().chain(close.iter()).filter(|e| e.2.as_deref() != Some(USE)).map(|e| e.1).collect();
        cells.sort();
        cells.dedup();
        let inputs: std::collections::HashSet<Pos> = cells.iter().copied().collect();
        let mut torch: BTreeMap<Pos, Pos> = BTreeMap::new();
        for &c in &cells {
            let evs: Vec<&Ev> = open.iter().chain(close.iter()).filter(|e| e.1 == c).collect();
            let rb = |s: &Option<String>| s.as_deref().is_none_or(|s| s.starts_with("minecraft:redstone_block"));
            let sets = |v: &[Ev]| v.iter().filter(|e| e.1 == c && e.2.is_some()).count();
            let s = [c[0], c[1], c[2] + c[2].signum()];
            let why = if along_z || c[2] == 0 {
                Some("in the door plane")
            } else if !evs.iter().all(|e| rb(&e.2)) {
                Some("not a redstone block")
            } else if evs.iter().any(|e| e.0 < 2) {
                Some("an event before tick 2 (lever-adjacent)")
            } else if sets(&open) > 3 || sets(&close) > 3 {
                Some("more than 3 pulses per edge (a torch would burn out)")
            } else if design.blocks.contains_key(&s) || inputs.contains(&s) || torch.values().any(|v| *v == s) {
                Some("its support cell is taken")
            } else if design.blocks.get(&c).is_some_and(|st| !st.starts_with("minecraft:redstone_block")) {
                Some("the cell holds another block at rest")
            } else {
                None
            };
            match why {
                Some(w) => notes.push(format!("input {c:?} acts between ticks: {w}")),
                None => {
                    torch.insert(c, s);
                }
            }
        }
        for (&c, &s) in &torch {
            let on = design.blocks.remove(&c).is_some();
            let facing = if c[2] < 0 { "south" } else { "north" };
            design.blocks.insert(c, format!("minecraft:redstone_wall_torch[facing={facing},lit={on}]"));
            design.blocks.insert(s, if on { "minecraft:stone" } else { "minecraft:redstone_block" }.to_string());
        }
        let rewrite = |evs: Vec<Ev>| -> Vec<Ev> {
            let mut out: Vec<Ev> = evs
                .into_iter()
                .map(|(t, p, st)| match torch.get(&p) {
                    Some(&s) => (t - 2, s, Some(if st.is_some() { "minecraft:stone" } else { "minecraft:redstone_block" }.to_string())),
                    None => (t, p, st),
                })
                .collect();
            out.sort_by_key(|e| e.0);
            out
        };
        Ok((design, rewrite(open), rewrite(close), notes))
    }

    /// Resolved events of one phase: (tick, pos, state) sorted by tick (stable).
    fn phase(&self, open: bool, vars: &BTreeMap<String, i64>) -> Result<Vec<(u32, Pos, Option<String>)>> {
        let mut out = Vec::new();
        for e in self.events.iter().filter(|e| e.open == open) {
            let t = eval(&e.tick, vars)?;
            if t < 0 {
                bail!("event at negative tick {t} ({})", e.tick);
            }
            out.push((t as u32, e.pos, e.state.clone()));
        }
        out.sort_by_key(|e| e.0);
        Ok(out)
    }

    /// Resolved events of the opening (`open`) or the closing: (tick, pos, state; `None` =
    /// clear, `Some("@use")` = right-click), sorted by tick. Used by `door cosearch search
    /// --reference-ideal` (the plan's piston events as a shaping target).
    pub fn resolved(&self, open: bool) -> Result<Vec<(u32, Pos, Option<String>)>> {
        self.phase(open, &self.vars)
    }
}

/// Knobs of one ideal run.
#[derive(Clone, Debug)]
pub struct IdealOptions {
    pub cycles: u32,
    pub offsets: Vec<Pos>,
    /// Print this plane (z, x0, y0, x1, y1) every tick of the first cycle at the first offset.
    pub trace: Option<(i32, i32, i32, i32, i32)>,
}

/// Run the schedule with these variables; Ok((open, close)) worst ticks if it behaves like a
/// door everywhere.
pub fn run(w: &mut Worker, s: &Schedule, vars: &BTreeMap<String, i64>, opts: &IdealOptions) -> Result<(u32, u32)> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<(u32, u32)> {
        let (design, open_ev, close_ev, _notes) = s.realize(vars)?;
        let mut worst = (0u32, 0u32);
        for (oi, off) in opts.offsets.iter().enumerate() {
            let d = design.translated(*off);
            let spec = DoorSpec { door: *d.ports.get("door").context("design has no door port")?, lever: [0, 0, 0], along_z: false };
            let (o, c) = run_at(w, sb, &d, &spec, *off, &open_ev, &close_ev, opts, oi == 0).with_context(|| format!("at offset {off:?}"))?;
            worst = (worst.0.max(o), worst.1.max(c));
        }
        Ok(worst)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

#[allow(clippy::too_many_arguments)]
fn run_at(
    w: &mut Worker,
    sb: u32,
    d: &Design,
    spec: &DoorSpec,
    off: Pos,
    open_ev: &[(u32, Pos, Option<String>)],
    close_ev: &[(u32, Pos, Option<String>)],
    opts: &IdealOptions,
    first: bool,
) -> Result<(u32, u32)> {
    w.clear(sb)?;
    crate::video::place(w, sb, d)?;
    let (wlo, whi) = crate::watch_box(d, spec);
    let rest = w.get_region(sb, wlo, whi)?;
    let problems = crate::closed_problems(&rest, spec);
    if !problems.is_empty() {
        bail!("not closed at rest: {}", problems.join("; "));
    }
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| rest.get(*p).unwrap_or("?").to_string()).collect();
    let mv = |p: Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
    // Worst open / close ticks over the cycles, measured like `door verify` does.
    let (mut worst_open, mut worst_close) = (0u32, 0u32);
    // Periodic rest, like the judge (vjeux 2026-10-19: internal state doesn't matter as long as the
    // door works repeatedly): every close must leave the door closed, and some closed state must
    // repeat an earlier one (the rest state included) within cycles + MAX_PERIOD_EXTRA.
    let mut seen_closed: Vec<mctas::sim::Region> = vec![rest.clone()];
    let mut periodic = false;
    for cycle in 0..opts.cycles + crate::MAX_PERIOD_EXTRA {
        if cycle >= opts.cycles && periodic {
            break;
        }
        for (phase, evs) in [("open", open_ev), ("close", close_ev)] {
            let trace = if first && cycle == 0 { opts.trace } else { None };
            let mut frames: Vec<mctas::sim::Region> = Vec::new();
            let mut t = 0u32;
            let mut i = 0;
            if let Some(_) = trace {
                println!("== {phase} ==");
            }
            while i < evs.len() {
                while i < evs.len() && evs[i].0 == t {
                    let (_, p, st) = &evs[i];
                    if st.as_deref() == Some(USE) {
                        w.use_block(sb, mv(*p))?;
                    } else {
                        let state = st.clone().unwrap_or_else(|| "minecraft:air".to_string());
                        w.set_blocks(sb, &[(mv(*p), state)], PlaceMode::Update)?;
                    }
                    i += 1;
                }
                if i >= evs.len() {
                    break;
                }
                w.step(sb, 1)?;
                t += 1;
                frames.push(w.get_region(sb, wlo, whi)?);
                if let Some((z, x0, y0, x1, y1)) = trace {
                    let r = w.get_region(sb, mv([x0, y0, z]), mv([x1, y1, z]))?;
                    println!("-- {phase} t={t}\n{}", crate::drive::render(&r, z + off[2], x0 + off[0], y0 + off[1], x1 + off[0], y1 + off[1]));
                }
            }
            if let Some((z, x0, y0, x1, y1)) = trace {
                for _ in 0..12 {
                    w.step(sb, 1)?;
                    t += 1;
                    frames.push(w.get_region(sb, wlo, whi)?);
                    let r = w.get_region(sb, mv([x0, y0, z]), mv([x1, y1, z]))?;
                    println!("-- {phase} t={t}\n{}", crate::drive::render(&r, z + off[2], x0 + off[0], y0 + off[1], x1 + off[0], y1 + off[1]));
                }
            }
            let settle = w.step_record(sb, 300, true, wlo, whi)?;
            if settle.idle != Some(true) {
                bail!("cycle {cycle}: still moving 300 ticks after the last {phase} event");
            }
            frames.extend(settle.frames);
            let ticks = if phase == "open" {
                crate::settled_from(&frames, |f| crate::open_problems(f, spec).is_empty())
            } else {
                crate::settled_from(&frames, |f| {
                    spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str()))
                })
            };
            let ticks = ticks.unwrap_or(u32::MAX);
            if phase == "open" {
                worst_open = worst_open.max(ticks);
            } else {
                worst_close = worst_close.max(ticks);
            }
            let now = w.get_region(sb, wlo, whi)?;
            if !w.entities(sb)?.is_empty() {
                bail!("cycle {cycle}: entities appeared while {phase}ing (something broke)");
            }
            if phase == "open" {
                let problems = crate::open_problems(&now, spec);
                if !problems.is_empty() {
                    bail!("cycle {cycle}: not open: {}", problems.join("; "));
                }
            } else {
                let problems = crate::closed_problems(&now, spec);
                if !problems.is_empty() {
                    bail!("cycle {cycle}: not closed after the close events: {}", problems.join("; "));
                }
                if seen_closed.iter().any(|s| crate::same_region(s, &now)) {
                    periodic = true;
                } else {
                    seen_closed.push(now.clone());
                }
            }
        }
    }
    if !periodic {
        let diffs: Vec<String> = rest
            .iter()
            .filter(|(p, s)| seen_closed.last().and_then(|n| n.get(*p)) != Some(*s))
            .take(6)
            .map(|(p, s)| format!("{:?} {} -> {}", [p[0] - off[0], p[1] - off[1], p[2] - off[2]], short(s), short(seen_closed.last().and_then(|n| n.get(p)).unwrap_or("?"))))
            .collect();
        bail!("never back at a closed state it had before within {} cycles (drifts: {})", opts.cycles + crate::MAX_PERIOD_EXTRA, diffs.join("; "));
    }
    Ok((worst_open, worst_close))
}

fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

/// The four default verifier positions plus `extra` pseudo-random ones (same generator as
/// `door verify --extra-offsets`).
pub fn offsets(extra: u32) -> Vec<Pos> {
    let mut out = crate::VerifyOptions::default().offsets;
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    for _ in 0..extra {
        let mut next = |m: i64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % (2 * m as u64 + 1)) as i32 - m as i32
        };
        let off = [next(13), next(40), next(13)];
        out.push(off);
    }
    out
}

/// CLI entry: run once, or sweep variables (`name=lo..hi`, several = grid).
pub fn cli(version: &str, path: &std::path::Path, set: &[String], sweep: &[String], independent: bool, extra_offsets: u32, cycles: u32, trace: Option<String>, workers: usize) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    let mut overrides = BTreeMap::new();
    for kv in set {
        let (k, v) = kv.split_once('=').context("--set name=value")?;
        overrides.insert(k.to_string(), v.parse::<i64>()?);
    }
    let s = Schedule::parse_with(&text, &overrides)?;
    let vars = s.vars.clone();
    let trace = match trace {
        Some(t) => {
            let v: Vec<i32> = t.split(',').map(|n| n.trim().parse()).collect::<Result<_, _>>()?;
            if v.len() != 5 {
                bail!("--trace z,x0,y0,x1,y1");
            }
            Some((v[0], v[1], v[2], v[3], v[4]))
        }
        None => None,
    };
    let opts = IdealOptions { cycles, offsets: offsets(extra_offsets), trace };
    let cfg = WorkerConfig { version: version.to_string(), heap_mb: 768, ..Default::default() };
    if sweep.is_empty() {
        if let Ok((_, _, _, notes)) = s.realize(&vars) {
            for n in notes {
                eprintln!("note: {n}");
            }
        }
        let mut w = Worker::spawn(&cfg)?;
        match run(&mut w, &s, &vars, &opts) {
            Ok((o, c)) => println!("WORKS ({} positions x {} cycles): open after {o} ticks / closed after {c} ticks", opts.offsets.len(), cycles),
            Err(e) => {
                println!("BROKEN: {e:#}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    // Grid of every combination of the swept ranges, or (independent) one axis at a time.
    let mut axes: Vec<(String, Vec<i64>)> = Vec::new();
    for sw in sweep {
        let (k, r) = sw.split_once('=').context("--sweep name=lo..hi")?;
        let (lo, hi) = r.split_once("..").context("--sweep name=lo..hi")?;
        let (lo, hi): (i64, i64) = (lo.parse()?, hi.parse()?);
        axes.push((k.to_string(), (lo..=hi).collect()));
    }
    let mut combos: Vec<(Vec<String>, BTreeMap<String, i64>)> = vec![(Vec::new(), vars.clone())];
    if independent {
        combos.clear();
        for (k, vals) in &axes {
            for v in vals {
                let mut c = vars.clone();
                c.insert(k.clone(), *v);
                combos.push((vec![format!("{k}={v}")], c));
            }
        }
    } else {
        for (k, vals) in &axes {
            let mut next = Vec::new();
            for (label, c) in &combos {
                for v in vals {
                    let mut c2 = c.clone();
                    c2.insert(k.clone(), *v);
                    let mut l2 = label.clone();
                    l2.push(format!("{k}={v}"));
                    next.push((l2, c2));
                }
            }
            combos = next;
        }
    }
    let pool = Pool::spawn(workers, &cfg)?;
    let results = pool.map(combos.clone(), |w, _, (_, c)| Ok(run(w, &s, &c, &IdealOptions { trace: None, ..opts.clone() })));
    for ((label, _), r) in combos.iter().zip(results) {
        match r {
            Ok(Ok((o, c))) => println!("{}: ok open {o} close {c}", label.join(" ")),
            Ok(Err(e)) => println!("{}: FAIL {}", label.join(" "), format!("{e:#}").chars().take(160).collect::<String>()),
            Err(e) => println!("{}: ERROR {e:#}", label.join(" ")),
        }
    }
    Ok(())
}
