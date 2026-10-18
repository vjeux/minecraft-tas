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
//! the next tick runs, like a player acting between ticks.

/// Marker state of a `use` event.
const USE: &str = "@use";

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
        Schedule { design, vars: BTreeMap::new(), events }
    }

    pub fn parse(text: &str) -> Result<Schedule> {
        let mut design = None;
        let mut vars = BTreeMap::new();
        let mut events = Vec::new();
        for (i, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let ctx = || format!("line {}: {raw:?}", i + 1);
            let p: Vec<&str> = line.split_whitespace().collect();
            match p[0] {
                "design" => design = Some(crate::cli::load_design(std::path::Path::new(p[1])).with_context(ctx)?),
                "let" => {
                    // let name = value
                    let rest = line[3..].trim();
                    let (k, v) = rest.split_once('=').with_context(ctx)?;
                    vars.insert(k.trim().to_string(), v.trim().parse::<i64>().with_context(ctx)?);
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
        Ok(Schedule { design: design.context("schedule has no 'design' line")?, vars, events })
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
        let open_ev = s.phase(true, vars)?;
        let close_ev = s.phase(false, vars)?;
        let mut worst = (0u32, 0u32);
        for (oi, off) in opts.offsets.iter().enumerate() {
            let d = s.design.translated(*off);
            let spec = DoorSpec { door: *d.ports.get("door").context("design has no door port")?, lever: [0, 0, 0] };
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
    for cycle in 0..opts.cycles {
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
                let diffs: Vec<String> = rest
                    .iter()
                    .filter(|(p, s)| now.get(*p) != Some(*s))
                    .take(6)
                    .map(|(p, s)| format!("{:?} {} -> {}", [p[0] - off[0], p[1] - off[1], p[2] - off[2]], short(s), short(now.get(p).unwrap_or("?"))))
                    .collect();
                if !diffs.is_empty() {
                    bail!("cycle {cycle}: not back at rest after closing: {}", diffs.join("; "));
                }
            }
        }
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
    let s = Schedule::parse(&text)?;
    let mut vars = s.vars.clone();
    for kv in set {
        let (k, v) = kv.split_once('=').context("--set name=value")?;
        vars.insert(k.to_string(), v.parse()?);
    }
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
