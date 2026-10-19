//! `mctas door satsweep <schedule.ideal>...`: a systematic `door satwire` sweep over many ideal
//! plans (track sweep56). For every plan, every placement of a W x H wiring box around its
//! mechanism (`--box-dims`), and every TIMING VARIANT of the plan, the variant is first replayed
//! with ideal inputs (`ideal::run`, 4 positions x 2 cycles: plans the mechanism cannot follow are
//! dropped), then handed to the SAT wiring synthesis (time warp, real-power recording, CEGAR with
//! the real judge). One TSV row per (plan, box, variant): SAT (wiring blocks, judged design) /
//! UNSAT (the pistons of the UNSAT core) / INVALID (the replay fails) / ERROR, with the time.
//!
//! Variants (`--variants`):
//! * `base`: the plan as written (the solver's time warp already stretches pauses and shortens
//!   static ticks);
//! * `gap`: for every pair of consecutive STEPS (inputs that start at the same tick) of a phase,
//!   the gap between them set to each value of `--gaps` (everything after the step shifts): the
//!   overlapping timings (gaps shorter than the motion) the warp cannot reach. Symmetric plans
//!   (the same train on both edges) are shifted on both edges alike;
//! * `triple`: every input pulsed once in a phase pulsed three times, `--triple-gaps` apart (a
//!   pull-type sticky piston pulled, pushed back and pulled again: net one move, but an odd number
//!   of changes for the observers that watch it), everything after it shifted.

use crate::ideal::{IdealOptions, Schedule};
use crate::satwire as sw;
use anyhow::{Context, Result};
use mctas::design::Design;
use mctas::sim::{Pos, Worker, WorkerConfig};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct SweepOpts {
    pub jobs: usize,
    pub palette: String,
    pub forbid_ring: bool,
    pub hall_ring: bool,
    pub lever_oop: bool,
    pub dmax: i32,
    pub skip: i32,
    pub settle: i32,
    pub core_tries: usize,
    pub box_dims: Option<(i32, i32)>,
    pub variants: Vec<String>,
    pub gaps: Vec<u32>,
    pub triple_gaps: Vec<u32>,
    /// `--menu-shifts`: per-toggle timing alternatives the solver picks from (empty: no menu).
    pub menu_shifts: Vec<i64>,
    /// `--menu-triples`: also offer each toggle tripled (`triple_gaps` spacings).
    pub menu_triples: bool,
    /// `--menu-widths`: also offer each toggle's pulse at these widths.
    pub menu_widths: Vec<u32>,
    /// `--menu-core`: one group for the core (inputs pulsed more than once and the single pulses
    /// between them): each alternative shifts one core pulse, changes its width, or holds two.
    pub menu_core: bool,
    /// Round every pulse width up to an even number of ticks.
    pub even: bool,
    /// Stretch every pulse shorter than this many ticks to it (0: keep the plan's widths).
    pub widen: u32,
    pub no_validate: bool,
    pub minimize: bool,
    pub tsv: PathBuf,
    pub out_dir: PathBuf,
    pub version: String,
    pub extra_offsets: u32,
}

/// One phase's events: (tick, pos, Some(state) = set / None = clear).
type Evs = Vec<(u32, Pos, Option<String>)>;

/// A pulse: an input set at `on` and cleared at `off` (or never: a level, `off` = None).
#[derive(Clone, Debug)]
struct Pulse {
    pos: Pos,
    on: u32,
    off: Option<u32>,
    state: String,
}

fn pulses(evs: &Evs) -> Vec<Pulse> {
    let mut out: Vec<Pulse> = Vec::new();
    let mut open: BTreeMap<Pos, usize> = BTreeMap::new();
    for (t, p, st) in evs {
        match st {
            Some(s) => {
                open.insert(*p, out.len());
                out.push(Pulse { pos: *p, on: *t, off: None, state: s.clone() });
            }
            None => {
                if let Some(i) = open.remove(p) {
                    out[i].off = Some(*t);
                } else {
                    // a clear of an input present at rest: a pulse that starts "before" the phase
                    out.push(Pulse { pos: *p, on: *t, off: Some(*t), state: String::new() });
                }
            }
        }
    }
    out
}

fn evs_of(ps: &[Pulse]) -> Evs {
    let mut v: Evs = Vec::new();
    for p in ps {
        if !p.state.is_empty() {
            v.push((p.on, p.pos, Some(p.state.clone())));
        }
        if let Some(off) = p.off {
            v.push((off, p.pos, None));
        }
    }
    v.sort_by_key(|e| e.0);
    v
}

/// The start ticks of the steps of a phase (sorted, distinct).
fn steps(ps: &[Pulse]) -> Vec<u32> {
    let mut v: Vec<u32> = ps.iter().filter(|p| !p.state.is_empty()).map(|p| p.on).collect();
    v.sort();
    v.dedup();
    v
}

/// Move every pulse that starts at or after `from` by `d` ticks (as a unit: widths kept).
fn shifted(ps: &[Pulse], from: u32, d: i64) -> Vec<Pulse> {
    let mv = |t: u32| -> u32 { (t as i64 + d).max(0) as u32 };
    ps.iter()
        .map(|p| if p.on >= from && !p.state.is_empty() { Pulse { pos: p.pos, on: mv(p.on), off: p.off.map(mv), state: p.state.clone() } } else { p.clone() })
        .collect()
}

/// A named variant: (name, open events, close events).
type Variant = (String, Evs, Evs);

fn variants(s: &Schedule, o: &SweepOpts) -> Result<Vec<Variant>> {
    let vars = s.vars().clone();
    let mut open = s.events_of(true, &vars)?;
    let mut close = s.events_of(false, &vars)?;
    let mut po = pulses(&open);
    let mut pc = pulses(&close);
    if o.widen > 0 || o.even {
        // pulses shorter than `widen` ticks (pistmodel's 1 gt pulses: no flat wiring makes them)
        // stretched to `widen` ticks, odd widths rounded up (`even`), starts kept
        for ps in [&mut po, &mut pc] {
            for p in ps.iter_mut() {
                if let Some(off) = p.off {
                    if !p.state.is_empty() {
                        let mut w = (off - p.on).max(o.widen);
                        if o.even && w % 2 == 1 {
                            w += 1;
                        }
                        p.off = Some(p.on + w);
                    }
                }
            }
        }
        open = evs_of(&po);
        close = evs_of(&pc);
    }
    let sym = evs_of(&po) == evs_of(&pc) && !po.is_empty();
    let mut out: Vec<Variant> = Vec::new();
    if o.variants.iter().any(|v| v == "base") {
        out.push(("base".into(), open.clone(), close.clone()));
    }
    // phase selectors: symmetric plans change both edges alike
    let phases: Vec<(&str, bool, bool)> = if sym { vec![("both", true, true)] } else { vec![("open", true, false), ("close", false, true)] };
    if o.variants.iter().any(|v| v == "gap") {
        for (pname, do_o, do_c) in &phases {
            let src = if *do_o { &po } else { &pc };
            let st = steps(src);
            for k in 0..st.len().saturating_sub(1) {
                let cur = st[k + 1] - st[k];
                for &g in &o.gaps {
                    if g == cur {
                        continue;
                    }
                    let d = g as i64 - cur as i64;
                    let no = if *do_o { evs_of(&shifted(&po, st[k + 1], d)) } else { open.clone() };
                    let nc = if *do_c { evs_of(&shifted(&pc, st[k + 1], d)) } else { close.clone() };
                    out.push((format!("gap-{pname}-s{}-{}g{g}", k + 1, k + 2), no, nc));
                }
            }
        }
    }
    if o.variants.iter().any(|v| v == "hold") {
        // two consecutive pulses of one input merged into ONE level (an elevator held extended
        // while the lifted piston works, retracting = the second pulse's pull), ending at the
        // second pulse's start or end
        for (pname, do_o, do_c) in &phases {
            let src = if *do_o { &po } else { &pc };
            let mut by: BTreeMap<Pos, Vec<usize>> = BTreeMap::new();
            for (i, p) in src.iter().enumerate().filter(|(_, p)| !p.state.is_empty() && p.off.is_some()) {
                by.entry(p.pos).or_default().push(i);
            }
            for (pos, idx) in &by {
                for w in idx.windows(2) {
                    let (a, b) = (w[0], w[1]);
                    for (end_name, end) in [("start", src[b].on), ("end", src[b].off.unwrap())] {
                        let make = |ps: &[Pulse]| -> Vec<Pulse> {
                            // the same pulses by (input, start) in the other phase (symmetric plans)
                            let (ia, ib) = (ps.iter().position(|q| q.pos == *pos && q.on == src[a].on), ps.iter().position(|q| q.pos == *pos && q.on == src[b].on));
                            let (Some(ia), Some(ib)) = (ia, ib) else { return ps.to_vec() };
                            let mut v = ps.to_vec();
                            v[ia].off = Some(end.max(v[ia].on + 1));
                            v.remove(ib);
                            v
                        };
                        let no = if *do_o { evs_of(&make(&po)) } else { open.clone() };
                        let nc = if *do_c { evs_of(&make(&pc)) } else { close.clone() };
                        out.push((format!("hold-{pname}-{},{},{}-{}{}-{end_name}", pos[0], pos[1], pos[2], src[a].on, src[b].on), no, nc));
                    }
                }
            }
        }
    }
    if o.variants.iter().any(|v| v == "triple") {
        for (pname, do_o, do_c) in &phases {
            let src = if *do_o { &po } else { &pc };
            // inputs pulsed exactly once in this phase
            let mut count: BTreeMap<Pos, usize> = BTreeMap::new();
            for p in src.iter().filter(|p| !p.state.is_empty() && p.off.is_some()) {
                *count.entry(p.pos).or_default() += 1;
            }
            for (pos, n) in &count {
                if *n != 1 {
                    continue;
                }
                for &g in &o.triple_gaps {
                    let make = |ps: &[Pulse]| -> Vec<Pulse> {
                        let Some(p) = ps.iter().find(|p| p.pos == *pos && !p.state.is_empty()) else { return ps.to_vec() };
                        let (on, off) = (p.on, p.off.unwrap());
                        let w = off - on;
                        // every pulse that starts after this one moves by 2g
                        let mut v: Vec<Pulse> = shifted(ps, on + 1, 2 * g as i64);
                        for j in 1..=2u32 {
                            v.push(Pulse { pos: *pos, on: on + j * g, off: Some(on + j * g + w), state: p.state.clone() });
                        }
                        v
                    };
                    let no = if *do_o { evs_of(&make(&po)) } else { open.clone() };
                    let nc = if *do_c { evs_of(&make(&pc)) } else { close.clone() };
                    out.push((format!("triple-{pname}-{},{},{}-g{g}", pos[0], pos[1], pos[2]), no, nc));
                }
            }
        }
    }
    Ok(out)
}

/// `--menu-shifts` / `--menu-triples`: for every input pulsed exactly once in a phase, the
/// alternative schedules the solver may pick from (that pulse alone shifted by each shift, or
/// tripled at each `--triple-gaps` spacing); grouped by input and phase.
fn menu_items(s: &Schedule, o: &SweepOpts) -> Result<Vec<(String, usize, String, Schedule)>> {
    let vars = s.vars().clone();
    let open = s.events_of(true, &vars)?;
    let close = s.events_of(false, &vars)?;
    let ph = [pulses(&open), pulses(&close)];
    let mut items = Vec::new();
    let mk_sched = |tr: usize, v: &[Pulse]| -> Schedule {
        let (oe, ce) = if tr == 0 { (evs_of(v), close.clone()) } else { (open.clone(), evs_of(v)) };
        let mut evs: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
        evs.extend(oe.iter().map(|(t, q, st)| (true, *t, *q, st.clone())));
        evs.extend(ce.iter().map(|(t, q, st)| (false, *t, *q, st.clone())));
        Schedule::from_events(s.design().clone(), &evs)
    };
    for tr in 0..2 {
        let ps = &ph[tr];
        let mut count: BTreeMap<Pos, usize> = BTreeMap::new();
        for p in ps.iter().filter(|p| !p.state.is_empty() && p.off.is_some()) {
            *count.entry(p.pos).or_default() += 1;
        }
        // the CORE: inputs pulsed more than once, and single pulses inside their span; one group
        // whose alternatives each change one core pulse (shift, width) or merge two (hold)
        let multi: Vec<&Pulse> = ps.iter().filter(|p| !p.state.is_empty() && count.get(&p.pos).copied().unwrap_or(0) > 1).collect();
        let span = (multi.iter().map(|p| p.on).min(), multi.iter().map(|p| p.on).max());
        let is_core = |p: &Pulse| -> bool {
            count.get(&p.pos).copied().unwrap_or(0) > 1 || matches!(span, (Some(a), Some(b)) if p.on > a && p.on < b)
        };
        if o.menu_core {
            for (i, p) in ps.iter().enumerate().filter(|(_, p)| !p.state.is_empty() && p.off.is_some() && is_core(p)) {
                let off = p.off.unwrap();
                for &d in &[-3i64, -2, -1, 1, 2, 3, 4, 6] {
                    if p.on as i64 + d < 0 {
                        continue;
                    }
                    let mut v = ps.clone();
                    v[i].on = (p.on as i64 + d) as u32;
                    v[i].off = Some((off as i64 + d) as u32);
                    items.push(("core".to_string(), tr, format!("{},{},{}@{}{d:+}", p.pos[0], p.pos[1], p.pos[2], p.on), mk_sched(tr, &v)));
                }
                for w in [1u32, 3, 4] {
                    if w == off - p.on {
                        continue;
                    }
                    let mut v = ps.clone();
                    v[i].off = Some(p.on + w);
                    items.push(("core".to_string(), tr, format!("{},{},{}@{}w{w}", p.pos[0], p.pos[1], p.pos[2], p.on), mk_sched(tr, &v)));
                }
            }
            // holds: two consecutive pulses of one core input merged into one level
            let mut by: BTreeMap<Pos, Vec<usize>> = BTreeMap::new();
            for (i, p) in ps.iter().enumerate().filter(|(_, p)| !p.state.is_empty() && p.off.is_some()) {
                by.entry(p.pos).or_default().push(i);
            }
            for (pos, idx) in &by {
                for w2 in idx.windows(2) {
                    let (a, b) = (w2[0], w2[1]);
                    for end in [ps[b].on, ps[b].off.unwrap()] {
                        let mut v = ps.clone();
                        v[a].off = Some(end.max(v[a].on + 1));
                        v.remove(b);
                        items.push(("core".to_string(), tr, format!("{},{},{}@{}hold{end}", pos[0], pos[1], pos[2], ps[a].on), mk_sched(tr, &v)));
                    }
                }
            }
        }
        for (pos, n) in &count {
            if *n != 1 {
                continue;
            }
            let Some(i) = ps.iter().position(|p| p.pos == *pos && !p.state.is_empty()) else { continue };
            let p = ps[i].clone();
            if o.menu_core && is_core(&p) {
                continue;
            }
            let off = p.off.unwrap();
            let mut alts: Vec<(String, Vec<Pulse>)> = Vec::new();
            for &d in &o.menu_shifts {
                if p.on as i64 + d < 0 || d == 0 {
                    continue;
                }
                let mut v = ps.clone();
                v[i].on = (p.on as i64 + d) as u32;
                v[i].off = Some((off as i64 + d) as u32);
                alts.push((format!("shift{d:+}"), v));
            }
            for &w in &o.menu_widths {
                if w == off - p.on || w == 0 {
                    continue;
                }
                let mut v = ps.clone();
                v[i].off = Some(p.on + w);
                alts.push((format!("w{w}"), v));
            }
            if o.menu_triples {
                for &g in &o.triple_gaps {
                    let mut v = ps.clone();
                    for j in 1..=2u32 {
                        v.push(Pulse { pos: p.pos, on: p.on + j * g, off: Some(off + j * g), state: p.state.clone() });
                    }
                    alts.push((format!("x3g{g}"), v));
                }
            }
            for (label, v) in alts {
                items.push((format!("{},{},{}", pos[0], pos[1], pos[2]), tr, label, mk_sched(tr, &v)));
            }
        }
    }
    Ok(items)
}

/// The mechanism's in-plane bounding box with the door's 3x3 (ideal inputs and levers excluded).
fn mech_bbox(d: &Design) -> Result<(i32, i32, i32, i32)> {
    let door = *d.ports.get("door").context("no door port")?;
    let (mut x0, mut y0, mut x1, mut y1) = (door[0], door[1], door[0] + 2, door[1] + 2);
    for (p, s) in &d.blocks {
        let n = s.strip_prefix("minecraft:").unwrap_or(s);
        if p[2] != door[2] || n.starts_with("redstone_block") || n.starts_with("lever") {
            continue;
        }
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    Ok((x0, y0, x1, y1))
}

/// Every W x H box (x0, y0, x1, y1) that holds the bounding box.
fn placements(bb: (i32, i32, i32, i32), dims: Option<(i32, i32)>) -> Vec<(i32, i32, i32, i32)> {
    let Some((w, h)) = dims else { return vec![bb] };
    let (bw, bh) = (bb.2 - bb.0 + 1, bb.3 - bb.1 + 1);
    if bw > w || bh > h {
        return vec![];
    }
    let mut v = Vec::new();
    for x0 in bb.2 - w + 1..=bb.0 {
        for y0 in bb.3 - h + 1..=bb.1 {
            v.push((x0, y0, x0 + w - 1, y0 + h - 1));
        }
    }
    v
}

struct Job {
    input: PathBuf,
    name: String,
    variant: String,
    sched: Schedule,
    bx: (i32, i32, i32, i32),
}

fn stem(p: &Path) -> String {
    p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "plan".into())
}

pub fn cli(inputs: &[PathBuf], o: &SweepOpts) -> Result<()> {
    std::fs::create_dir_all(&o.out_dir)?;
    let mut jobs: Vec<Job> = Vec::new();
    for inp in inputs {
        let text = std::fs::read_to_string(inp).with_context(|| format!("{}", inp.display()))?;
        let s = Schedule::parse(&text).with_context(|| format!("{}", inp.display()))?;
        let bb = mech_bbox(s.design())?;
        let boxes = placements(bb, o.box_dims);
        if boxes.is_empty() {
            println!("{}: mechanism {}x{} does not fit the box", inp.display(), bb.2 - bb.0 + 1, bb.3 - bb.1 + 1);
            continue;
        }
        for (vname, oe, ce) in variants(&s, o)? {
            let vname = if o.widen > 0 || o.even { format!("w{}{}-{vname}", o.widen, if o.even { "e" } else { "" }) } else { vname };
            let any_menu = !o.menu_shifts.is_empty() || o.menu_triples || !o.menu_widths.is_empty() || o.menu_core;
            let vname = if any_menu { format!("menu-{vname}") } else { vname };
            let mut evs: Vec<(bool, u32, Pos, Option<String>)> = Vec::new();
            evs.extend(oe.iter().map(|(t, p, st)| (true, *t, *p, st.clone())));
            evs.extend(ce.iter().map(|(t, p, st)| (false, *t, *p, st.clone())));
            let vs = Schedule::from_events(s.design().clone(), &evs);
            for bx in &boxes {
                let bname = if boxes.len() > 1 { format!("box{},{}", bx.0, bx.1) } else { "box".to_string() };
                jobs.push(Job { input: inp.clone(), name: format!("{}__{}__{}", stem(inp), vname, bname), variant: vname.clone(), sched: vs.clone(), bx: *bx });
            }
        }
    }
    println!("satsweep: {} jobs ({} plans), {} parallel", jobs.len(), inputs.len(), o.jobs);
    let new_file = !o.tsv.exists();
    let tsv = Mutex::new(std::fs::OpenOptions::new().create(true).append(true).open(&o.tsv)?);
    if new_file {
        writeln!(tsv.lock().unwrap(), "when\tplan\tvariant\tbox\tresult\twiring\tjudge\tcore\tsecs\tlog")?;
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let counts = Mutex::new(BTreeMap::<String, usize>::new());
    std::thread::scope(|sc| {
        for _ in 0..o.jobs.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(j) = jobs.get(i) else { break };
                let t0 = std::time::Instant::now();
                let log_path = o.out_dir.join(format!("{}.log", j.name));
                let (result, wiring, judge, core) = run_job(j, o, &log_path);
                let secs = t0.elapsed().as_secs_f64();
                let when = chrono_now();
                let line = format!(
                    "{when}\t{}\t{}\t{},{}:{},{}\t{result}\t{wiring}\t{judge}\t{core}\t{secs:.0}\t{}",
                    j.input.display(),
                    j.variant,
                    j.bx.0,
                    j.bx.1,
                    j.bx.2,
                    j.bx.3,
                    log_path.display()
                );
                println!("[{}/{}] {} {result} {wiring} {core} ({secs:.0}s)", i + 1, jobs.len(), j.name);
                if let Ok(mut f) = tsv.lock() {
                    let _ = writeln!(f, "{line}");
                    let _ = f.flush();
                }
                *counts.lock().unwrap().entry(result.split_whitespace().next().unwrap_or("?").to_string()).or_default() += 1;
            });
        }
    });
    println!("satsweep done: {:?}", counts.lock().unwrap());
    Ok(())
}

fn chrono_now() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    format!("{s}")
}

/// (result, wiring blocks, judge line, core) of one job.
fn run_job(j: &Job, o: &SweepOpts, log_path: &Path) -> (String, String, String, String) {
    let lines: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let log = |s: String| {
        lines.lock().unwrap().push(s);
    };
    let r = (|| -> Result<(String, String, String, String)> {
        let cfg = WorkerConfig { version: o.version.clone(), heap_mb: 1024, ..Default::default() };
        let rec = {
            let mut w = Worker::spawn(&cfg)?;
            if !o.no_validate {
                let opts = IdealOptions { cycles: 2, offsets: crate::search::judge_offsets(0), trace: None };
                if let Err(e) = crate::ideal::run(&mut w, &j.sched, j.sched.vars(), &opts) {
                    let msg = format!("{e:#}");
                    log(format!("replay with ideal inputs FAILS: {msg}"));
                    return Ok(("INVALID".into(), "-".into(), msg.chars().take(120).collect(), "-".into()));
                }
            }
            let mut rec = sw::record_ideal(&mut w, &j.sched, 80)?;
            if !o.menu_shifts.is_empty() || o.menu_triples || !o.menu_widths.is_empty() || o.menu_core {
                let items = menu_items(&j.sched, o)?;
                log(format!("menu: {} alternative recordings", items.len()));
                for l in sw::add_menu(&mut w, &mut rec, &items, 80)? {
                    log(l);
                }
            }
            rec
        };
        for (i, ph) in rec.phases.iter().enumerate() {
            let on: Vec<String> = (1..ph.power.len())
                .flat_map(|t| {
                    let mut v: Vec<String> = ph.power[t].iter().filter(|(_, b)| **b).map(|(c, _)| format!("t{t}:{},{}", c.0, c.1)).collect();
                    v.sort();
                    v
                })
                .collect();
            log(format!("{}: {} frames, powered piston-ticks {}", ["open", "close"][i], ph.frames.len(), on.join(" ")));
        }
        let po = sw::ProbOpts {
            bx: Some(j.bx),
            palette: sw::palette(&o.palette)?,
            forbid_ring: o.forbid_ring,
            hall_ring: o.hall_ring,
            levers: None,
            lever_oop: o.lever_oop,
            dmax: o.dmax,
            skip: o.skip,
            settle: o.settle,
            qc: true,
            levels: 15,
            blocked_dc: true,
            motion: true,
            clusters: true,
            full_frame: false,
        };
        let p = sw::build_prob(rec, &po)?;
        let out = o.out_dir.join(format!("{}.txt", j.name));
        let ro = sw::RunOpts { iters: 300, core_tries: o.core_tries, minimize: o.minimize, max_blocks: None, extra: o.extra_offsets, out: Some(out.clone()), name: j.name.clone(), version: o.version.clone() };
        let d = sw::run_log(&p, &ro, &log)?;
        let ls = lines.lock().unwrap().clone();
        let core = ls.iter().find_map(|l| l.split_once("core (pistons whose recorded power cannot be produced together): ").map(|x| x.1.to_string())).unwrap_or_else(|| "-".into());
        match d {
            Some(_) => {
                let judge = ls.iter().rev().find(|l| l.contains("JUDGE OK")).cloned().unwrap_or_default();
                let wiring = ls.iter().rev().find_map(|l| l.split_once("SAT (solve").and_then(|x| x.1.split_once("): ")).and_then(|x| x.1.split(' ').next().map(|s| s.to_string()))).unwrap_or_default();
                let extra_ok = ls.iter().any(|l| l.contains("JUDGE OK with --extra-offsets"));
                Ok((if extra_ok { "SAT".into() } else { "SAT-4pos".into() }, wiring, judge.trim().to_string(), out.display().to_string()))
            }
            None => {
                let unsat = ls.iter().any(|l| l.contains("UNSAT"));
                Ok((if unsat { "UNSAT-train".into() } else { "NOMODEL".into() }, "-".into(), "-".into(), core))
            }
        }
    })();
    let res = match r {
        Ok(x) => x,
        Err(e) => {
            let msg = format!("{e:#}");
            log(format!("ERROR: {msg}"));
            ("ERROR".into(), "-".into(), msg.chars().take(160).collect(), "-".into())
        }
    };
    let text = lines.lock().unwrap().join("\n");
    let _ = std::fs::write(log_path, text + "\n");
    res
}
