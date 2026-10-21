//! The simulations a page needs, as pure functions of a design (what src/cache.rs stores): a
//! clock member generated and simulated, the circuit judge's run of a design, the 3D view's own
//! session (whether it runs a design, a shulker machine's run, a clock's chart and lamp check).
//!
//! Every cache domain hashes this file (build.rs): a change here recomputes the cached results.
//! Text a visitor reads stays out of what is computed here (phase labels, sentences: the pages
//! write them from these results), so wording edits elsewhere keep the cache.

use circuit::Circuit as Design;
use circuit::verify::{Options, sim};
use clock::Clock;
use clock::extend::Family as ExtFamily;
use clock::sim::{SimOptions, Verdict, simulate};
use serde::{Deserialize, Serialize};
use std::path::Path;

// ----- clock members -----

/// A generated clock member: its design text (its output retapped when the generator's broke the
/// output rule) and what the judge's simulation says.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemberGen {
    pub text: String,
    pub dims: String,
    pub volume: u32,
    /// The period redsim measured, when it found the design to be a clock within the horizon.
    pub sim: Option<u32>,
    pub blocks: usize,
    /// Its own numbers from that simulation (None: redsim found no clock).
    pub run: Option<site_model::MemberRun>,
}

/// Width x height x depth and volume of a design's box.
pub fn dims_of(c: &Clock) -> (String, u32) {
    let (lo, hi) = c.bounds();
    let d = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    (format!("{}x{}x{}", d[0], d[1], d[2]), (d[0] * d[1] * d[2]).max(0) as u32)
}

/// Generate member `n` of family `f` (its archived base design at `base_path`) into `tmp` and
/// simulate it, once (at the default horizon): the output check is the period's run, and a
/// retapped output comes with its own run (the retap search simulates its pick).
pub fn generate_member(f: &ExtFamily, base_path: &Path, n: i64, horizon: u32, tmp: &Path) -> Result<MemberGen, String> {
    let path = f.build(base_path, n, tmp).map_err(|e| format!("{e:#}"))?;
    let result = (|| {
        let c = Clock::load(&path).map_err(|e| format!("{e:#}"))?;
        // a generator older than the output rule (its output a cell a block enters during the
        // cycle, or not an exterior air cell): the member's output moves to the exterior air cell
        // where the judge's lamp lights evenly, redsim's pick (as `clock retap` moved the archived
        // clocks'), with the run of the design with that output
        let retap = |c: &Clock| -> Result<Option<(Clock, clock::sim::SimReport)>, String> {
            let mut d = c.design.clone();
            d.ports.remove("output");
            let free = Clock::from_design(d).map_err(|e| format!("{e:#}"))?;
            match clock::sim::lamp_outputs(&free, 1).into_iter().next() {
                Some((to, rep)) => {
                    let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
                    std::fs::write(&path, clock::gaps::with_output(&text, to, c.output, None).map_err(|e| format!("{e:#}"))?).map_err(|e| e.to_string())?;
                    Ok(Some((Clock::load(&path).map_err(|e| format!("{e:#}"))?, rep)))
                }
                None => Ok(None),
            }
        };
        // an output cell holding a block of the design breaks the rule whatever the run: straight
        // to the retap (its search runs the design itself; when it finds no cell, the design is no
        // clock without its output either, and the run below says what it is), one long run saved
        // (dropring n = 426: 50 s)
        let held = c.output.is_some_and(|o| c.design.blocks.contains_key(&o));
        let early = if held { retap(&c)? } else { None };
        let (c, report) = match early {
            Some((moved, rep)) => (moved, rep),
            None => {
                let first = simulate(&c, &SimOptions::default());
                // (a held cell's retap already found nothing: not again)
                if !held && first.verdict == Verdict::Invalid && first.why.contains("output cell") {
                    match retap(&c)? {
                        Some((moved, rep)) => (moved, rep),
                        None => (c, first),
                    }
                } else {
                    (c, first)
                }
            }
        };
        let text = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        // the period at another horizon than the default one: its own run
        let default = SimOptions::default();
        let report = if horizon > 0 && horizon != default.horizon { simulate(&c, &SimOptions { horizon, margin: default.margin }) } else { report };
        let clock = (report.verdict == Verdict::Clock).then_some(&report);
        let sim = clock.filter(|_| horizon > 0 && f.period.eval(n) <= horizon as i64).map(|r| r.period);
        // its own numbers (the base design's are not its: critique, an extended hopper clock of
        // 148 ticks said "on for 18 ticks", its base's 18 of 20)
        let run = clock.and_then(|r| r.output.as_ref()).map(|o| site_model::MemberRun {
            warmup: report.warmup,
            on: o.wave.on,
            edges: o.wave.edges,
            pulses: format!("{:?}", o.wave.widths),
            pulse_every: (!o.wave.gaps.is_empty() && !clock::catalog::uneven(&o.wave.gaps)).then(|| o.wave.gaps[0]).filter(|g| *g > 0 && *g < report.period),
        });
        let (dims, volume) = dims_of(&c);
        Ok(MemberGen { text, dims, volume, sim, blocks: c.blocks(), run })
    })();
    let _ = std::fs::remove_file(&path);
    result
}

// ----- circuits: the judge's run -----

/// What a circuit page shows of the judge's run: the inputs' devices, the stimulus (`stimulus:`
/// text) and its phases, the redsim run as a timing diagram, the checker's report.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Details {
    pub devices: Vec<String>,
    pub stimulus: String,
    /// `data-levels`: a strength input's values (the hex kinds).
    pub levels: String,
    /// `data-phases` (written by the page from `raw_phases`, in words).
    pub phases: String,
    /// The checker's labelled spans as it names them (`pulse 3`, `clock 40`): the page puts them in
    /// words, here and in `wave`.
    pub raw_phases: Vec<(u32, u32, String)>,
    /// The circuit kind's name, for those words.
    pub kind_name: String,
    pub wave: Option<site_model::Wave>,
    pub table: Vec<String>,
    /// The judge's metrics when its run on the redsim screen passes.
    pub metrics: Option<Vec<(String, String)>>,
    /// The redsim screen's one-line verdict.
    pub summary: String,
    /// Why the screen fails, when it does.
    pub fails: Option<String>,
    /// Every output cell gets the judge's lamp in the 3D view.
    pub output_lamp: bool,
    /// The judge's actions at the target / probe cells (`data-events`).
    pub events: String,
    /// Why redsim cannot run the design (empty: it can), in words (set by the page).
    pub unmodeled: String,
    /// A shulker machine run like its 3D view runs it.
    pub view_run: Option<site_model::ViewRun>,
    /// A counter's measured duty: (pulse windows the output is on, N).
    pub counter_on: Option<(u32, u32)>,
}

/// The circuit judge's run of design `c`, for its page.
pub fn circuit_run(c: &Design) -> Details {
    let devices: Vec<String> = c
        .inputs
        .iter()
        .map(|i| match &i.how {
            circuit::InputHow::Lever => "lever".to_string(),
            circuit::InputHow::Button => "button".to_string(),
            circuit::InputHow::Strength { .. } => "strength (a chest)".to_string(),
            circuit::InputHow::Frame { .. } => "item frame".to_string(),
            circuit::InputHow::Block { on, .. } if on == circuit::INPUT_ON => "redstone block".to_string(),
            circuit::InputHow::Block { on, .. } => on.trim_start_matches("minecraft:").to_string(),
            // R7: a plain input is driven by the judge's repeater outside the build
            circuit::InputHow::Driven { .. } => "repeater".to_string(),
        })
        .collect();
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    // the viewer puts the judge's lamp in every air output cell (webrender bbe3bf7ba): then the
    // lamp shows the output, else the orange marker does
    let consumers = c.consumer_cells();
    let output_lamp = !c.outputs.is_empty() && c.outputs.iter().all(|o| consumers.contains(&o.pos));
    let mut d = Details { devices, output_lamp, ..Default::default() };
    let r = match sim(c, &Options::default()) {
        Ok(r) => r,
        Err(e) => {
            d.summary = format!("simulation failed: {e:#}");
            return d;
        }
    };
    d.summary = r.summary(c);
    d.kind_name = c.kind.as_ref().map(|k| k.name()).unwrap_or_default();
    d.raw_phases = r.schedule.phases.clone();
    if !names.is_empty() && r.schedule.edges.iter().any(|e| !e.is_empty()) {
        d.stimulus = r.schedule.text(&names);
    }
    // a strength input's values (the hex kinds: the judge's chest fills), webrender's data-levels
    if !r.schedule.levels.is_empty() {
        d.levels = r.schedule.levels.iter().filter_map(|(t, i, v)| names.get(*i).map(|n| format!("{t} {n} {v}"))).collect::<Vec<_>>().join("; ");
    }
    // the judge's actions at the BUD's target and probe cells, in the design's coordinates
    let probes = c.probe_cells();
    d.events = r
        .schedule
        .events
        .iter()
        .filter_map(|(frame, at, op)| {
            let p = match at {
                circuit::stimulus::EventAt::Target(i) => c.targets.get(*i)?.pos,
                circuit::stimulus::EventAt::Probe(k) => *probes.get(*k)?,
                // the vibration test's game events change no block: nothing to show
                circuit::stimulus::EventAt::Input(_) | circuit::stimulus::EventAt::Cell(_) => return None,
            };
            let what = match op {
                circuit::stimulus::TargetOp::Place(s) => format!("place {} {} {} {s}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Clear => format!("clear {} {} {}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Shape(s) => format!("shape {} {} {} {s}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::Update => format!("update {} {} {}", p[0], p[1], p[2]),
                circuit::stimulus::TargetOp::GameEvent(_) => return None,
            };
            Some(format!("{frame} {what}"))
        })
        .collect::<Vec<_>>()
        .join("; ");
    if let Some(o) = &r.outcome {
        d.table = o.table.clone();
        if o.pass {
            d.metrics = Some(o.metrics.clone());
        } else {
            d.fails = Some(o.why.clone());
        }
    }
    if let Ok(t) = &r.trace {
        let mut rows: Vec<site_model::WaveRow> = Vec::new();
        for (k, i) in c.inputs.iter().enumerate() {
            let lv: Vec<u8> = t.inputs[k].iter().map(|on| if *on { 15 } else { 0 }).collect();
            rows.push(site_model::WaveRow::from_levels(&i.name, true, &lv));
        }
        // a BUD's judge acts at its target cells: one row of marks (the footprint probes left out)
        let marks: Vec<(u32, String)> = r
            .schedule
            .events
            .iter()
            .filter(|(_, at, _)| matches!(at, circuit::stimulus::EventAt::Target(_)))
            .map(|(f, _, op)| {
                let what = match op {
                    circuit::stimulus::TargetOp::Place(_) => "place",
                    circuit::stimulus::TargetOp::Clear => "clear",
                    circuit::stimulus::TargetOp::Shape(_) => "shape",
                    circuit::stimulus::TargetOp::Update => "update",
                    circuit::stimulus::TargetOp::GameEvent(e) => e,
                };
                (*f, what.to_string())
            })
            .collect();
        if !marks.is_empty() {
            rows.push(site_model::WaveRow { name: "target".into(), input: true, marks, ..Default::default() });
        }
        for (k, o) in c.outputs.iter().enumerate() {
            rows.push(site_model::WaveRow::from_levels(&o.name, false, &t.levels[k]));
        }
        // the machines' position tracks (vjeux: "Can you put the position of the block?")
        let metric = |k: &str| r.outcome.as_ref().and_then(|o| o.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())).unwrap_or_default();
        match &c.kind {
            Some(circuit::Kind::PistonExtender(n)) => {
                if let Some(row) = payload_track(c, t, *n) {
                    let mut row = row;
                    let (ext, ret) = (metric("extend"), metric("retract"));
                    if !ext.is_empty() {
                        row.note = format!("out to N = {n} in {ext}, back in {ret}");
                    }
                    rows.push(row);
                }
            }
            Some(circuit::Kind::FlyingMachine) => {
                if let Some(mut row) = flight_track(c, t) {
                    let speed = metric("speed");
                    row.note = if speed.is_empty() { metric("two-way") } else { format!("1 block every {}", speed.trim_end_matches("/block")) };
                    rows.push(row);
                }
            }
            Some(circuit::Kind::Counter(n)) if *n > 0 && !t.inputs.is_empty() && !t.levels.is_empty() => {
                // a counter's duty, measured: in how many of every N pulse windows its output is
                // on (sampled half-way between two input pulses, after the first N windows)
                let input = &t.inputs[0];
                let edges: Vec<usize> = (1..input.len()).filter(|&f| input[f] && !input[f - 1]).collect();
                let windows: Vec<bool> = edges.windows(2).skip(*n as usize).map(|w| t.levels[0].get((w[0] + w[1]) / 2).is_some_and(|&l| l > 0)).collect();
                if windows.len() >= *n as usize {
                    let on = windows.iter().filter(|x| **x).count() as f64 / windows.len() as f64;
                    d.counter_on = Some(((on * *n as f64).round() as u32, *n));
                }
            }
            _ => {}
        }
        if !rows.is_empty() && t.frames() > 1 {
            d.wave = Some(site_model::Wave { ticks: t.frames() as u32, rows, phases: d.raw_phases.clone() });
        }
    }
    d
}

/// A piston extender's payload position per frame along its push direction (0 = home .. N =
/// extended); while it moves (carried inside a moving piston) the last position holds.
fn payload_track(c: &Design, t: &circuit::trace::Trace, n: u32) -> Option<site_model::WaveRow> {
    let d = c.travel?;
    let p0 = c.payloads.first()?.pos;
    let rest_at = (c.clock.free_from() + circuit::stimulus::SETTLE).saturating_sub(1) as usize;
    let payload = t.state(rest_at.min(t.frames().saturating_sub(1)), p0)?.to_string();
    let id = t.names.iter().position(|s| *s == payload)? as u32;
    let mut last = 0i32;
    let mut levels: Vec<u8> = Vec::with_capacity(t.frames());
    let mut top = n as i32;
    for f in 0..t.frames() {
        let here: Vec<&circuit::Pos> = t.cells[f].iter().filter(|(_, x)| *x == id).map(|(p, _)| p).collect();
        if here.len() == 1 {
            let p = here[0];
            last = (p[0] - p0[0]) * d[0] + (p[1] - p0[1]) * d[1] + (p[2] - p0[2]) * d[2];
        }
        top = top.max(last);
        levels.push(last.clamp(0, 255) as u8);
    }
    let mut row = site_model::WaveRow::from_levels("payload", false, &levels);
    row.scale = top.max(1) as u32;
    row.target = Some(n);
    Some(row)
}

/// A flying machine's displacement along its travel per frame (cells from where it was built);
/// while it moves between intact positions the last one holds.
fn flight_track(c: &Design, t: &circuit::trace::Trace) -> Option<site_model::WaveRow> {
    let travel = c.travel.unwrap_or([1, 0, 0]);
    let kick = match &c.clock.kick {
        Some(clock::Kick::Place { pos, ticks, .. }) => Some((*pos, c.clock.kick_at, c.clock.kick_at + ticks)),
        _ => None,
    };
    let mut rec = circuit::fly::Recorder::new(&c.clock.design.blocks, kick, &c.stations);
    for f in 0..t.frames() {
        rec.ingest(f as u32, t.cells[f].iter().map(|(p, id)| (*p, t.names[*id as usize].as_str())));
    }
    let mut last = 0i32;
    let mut top = 1i32;
    let mut levels: Vec<u8> = Vec::with_capacity(t.frames());
    for s in &rec.shifts {
        if let Some(s) = s {
            last = (s[0] * travel[0] + s[1] * travel[1] + s[2] * travel[2]).abs();
        }
        top = top.max(last);
        levels.push(last.clamp(0, 255) as u8);
    }
    if top <= 1 && levels.iter().all(|l| *l == 0) {
        return None;
    }
    let mut row = site_model::WaveRow::from_levels("moved", false, &levels);
    row.scale = top as u32;
    Some(row)
}

// ----- the 3D view's session -----

/// Game ticks a shulker machine is run at most the way its 3D view runs it, until it has sent out
/// four boxes (a 64-stack filter loader takes ~13800 gt a box).
pub const VIEW_RUN: u32 = 60_000;
/// Boxes the view's run waits for (per OUT chest).
const VIEW_BOXES: usize = 4;

/// What the 3D view does with a circuit design: why it cannot run it (its own words, None: it
/// runs it), and a shulker machine's run.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ViewCheck {
    pub unsimulated: Option<String>,
    pub view_run: Option<site_model::ViewRun>,
}

/// The 3D view's own session on a circuit design.
pub fn view_check(text: &str) -> ViewCheck {
    match mcview::session::Session::new(text, &Default::default()) {
        Ok(mut s) => {
            let view_run = (s.unsimulated.is_none() && s.shulker.is_some()).then(|| view_run(&mut s, VIEW_RUN));
            ViewCheck { unsimulated: s.unsimulated.clone(), view_run }
        }
        Err(e) => ViewCheck { unsimulated: Some(format!("the viewer cannot load it: {e}")), view_run: None },
    }
}

/// Run a shulker machine the way the 3D view does (the viewer's own session: the judge's chests
/// kept full / emptied, its start lever turned on like the visitor and the judge do) and count
/// the boxes that reach the OUT chests: the page's numbers, and proof the view's machine runs.
fn view_run(s: &mut mcview::session::Session, ticks: u32) -> site_model::ViewRun {
    use redsim::redstone::{container, shulker};
    let levers: Vec<usize> = s
        .circuit_inputs
        .iter()
        .enumerate()
        .filter(|(_, ci)| ci.how == mcview::circuit::InputHow::Lever && !(ci.name == "mode" || ci.name.starts_with("mode.")))
        .map(|(i, _)| i)
        .collect();
    for i in levers {
        s.set_input(i, true);
    }
    let outs = s.shulker.as_ref().map(|c| c.outputs.clone()).unwrap_or_default();
    let rejects = s.shulker.as_ref().map(|c| c.rejects.clone()).unwrap_or_default();
    let item = s.shulker.as_ref().map(|c| container::item_name(c.item).to_string()).unwrap_or_default();
    let mut arrivals: Vec<u32> = Vec::new();
    let mut rejected = 0u32;
    let mut run = ticks;
    // a machine with several OUT chests (two lines) delivers in bursts: wait for 4 boxes a chest
    let want = VIEW_BOXES * outs.len().max(1);
    for k in 0..ticks {
        // an OUT chest holds what the last tick delivered until the next visit empties it
        let n: u32 = outs
            .iter()
            .map(|p| shulker::chest_stacks(&s.world, *p).iter().filter(|st| container::item_name(st.item).ends_with("shulker_box")).map(|st| st.count as u32).sum::<u32>())
            .sum();
        arrivals.extend(std::iter::repeat_n(k, n as usize));
        rejected += rejects.iter().map(|p| shulker::chest_stacks(&s.world, *p).iter().map(|st| st.count as u32).sum::<u32>()).sum::<u32>();
        if arrivals.len() >= want {
            run = k;
            break;
        }
        s.step();
    }
    let cycle = match (arrivals.first(), arrivals.last()) {
        (Some(a), Some(b)) if arrivals.len() >= 2 => Some((b - a) as f64 / (arrivals.len() - 1) as f64),
        _ => None,
    };
    site_model::ViewRun { ticks: run, boxes: arrivals.len() as u32, first: arrivals.first().copied(), cycle, item, rejected, lines: outs.len() as u32 }
}

/// What a redstone lamp on a clock's output does (critique a417977c3: a lamp goes dark only when
/// its power has been off for more than 4 gt, so a clock whose output is never off longer than
/// that keeps it lit), simulated with the judge's lamp in the output cell, the way the 3D view
/// runs it. `Fast(verdict, gap)`: `stays lit` (never dark after the output's first pulse) or
/// `misses pulses` (it lights fewer times than the output rises), the output's shortest off-gap 4
/// gt or less (the reason), or `relit` (it stays lit though the output is off longer: something
/// relights it within the tick its power goes off); `Follows(gap)`: it blinks with every pulse;
/// `Unexplained`: it misses pulses for no reason we can name (no claim); `Entered`: blocks move
/// through the output cell (its lamp is pushed away or replaced).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Lamp {
    Fast(String, u32),
    Follows(u32),
    Unexplained(u32),
    Entered,
}

/// Why a clock's chart is not drawn.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum ChartErr {
    /// The viewer cannot run it (its own words, raw).
    NotRun(String),
    NoPeriod,
    /// Blocks move through its output cell.
    Entered,
    /// The design names no output and no block of it turns on and off.
    NoBlock,
    /// The output stayed off over the ticks run.
    StayedOff(u32),
}

/// A clock's output over its start-up and a few periods, as the 3D view plays it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Chart {
    /// The output's runs: (from tick, 0 or 15).
    pub runs: Vec<(u32, u8)>,
    /// Ticks drawn.
    pub ticks: u32,
    /// Its start-up, shaded.
    pub warm: u32,
    /// The block drawn when the design names no output (its busiest block), else empty.
    pub picked: String,
}

/// A clock's chart and lamp check, from one run of the 3D view's session.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClockView {
    pub chart: Result<Chart, ChartErr>,
    pub lamp: Option<Lamp>,
}

/// The 3D view's session on a clock design of `period` and `warmup` (the judge's), run once for
/// its chart (its start-up and 3 periods, 2 when long) and its lamp check (4 periods).
pub fn clock_view(text: &str, period: u32, warmup: u32) -> ClockView {
    let mut s = match mcview::session::Session::new(text, &mcview::session::Options::default()) {
        Ok(s) => s,
        Err(e) => return ClockView { chart: Err(ChartErr::NotRun(e)), lamp: None },
    };
    if let Some(why) = &s.unsimulated {
        return ClockView { chart: Err(ChartErr::NotRun(why.clone())), lamp: None };
    }
    if !s.simulated() {
        return ClockView { chart: Err(ChartErr::NotRun("the browser's simulator does not run this design".into())), lamp: None };
    }
    if period == 0 {
        return ClockView { chart: Err(ChartErr::NoPeriod), lamp: None };
    }
    let periods = if period > 200 { 2 } else { 3 };
    let warm = warmup.min(2000);
    let chart_total = (warm + periods * period).min(4000);
    let lamp_total = (warm + 4 * period + 8).min(4000);
    // the declared output cell, else (older clocks) the block that turns on most often after the
    // start-up: the clock judge's own rule for a census clock's output
    let declared = s.output_on().is_some();
    let out = s.outputs.first().copied();
    let lamp_cell = out.filter(|o| declared && s.judge_lamp == Some(*o));
    let cells: Vec<mcview::Pos> = if declared { Vec::new() } else { s.design.blocks.keys().copied().collect() };
    let total = if lamp_cell.is_some() { chart_total.max(lamp_total) } else { chart_total };
    let mut power: Vec<bool> = Vec::with_capacity(total as usize);
    let mut lit: Vec<bool> = Vec::with_capacity(total as usize);
    let mut per_cell: Vec<Vec<bool>> = vec![Vec::with_capacity(chart_total as usize); cells.len()];
    let mut entered: Option<u32> = None;
    for t in 0..total {
        if let Some(o) = lamp_cell {
            // (a block moving through the output cell: no steady output, no lamp to judge)
            if !s.world.get(mcview::session::bp(o)).to_string().starts_with("minecraft:redstone_lamp") {
                entered = Some(t);
                break;
            }
        }
        if declared {
            // the power delivered into the output's cell, as the clock judge measures it
            // (critique: a 4-tick clock drew one solid bar, the judge's lamp never going dark
            // between its 2-tick pulses because of the lamp's own off-delay)
            power.push(match out {
                Some(o) => s.output_powered(o),
                None => s.output_on() == Some(true),
            });
            lit.push(s.output_on() == Some(true));
        } else if t < chart_total {
            for (k, p) in cells.iter().enumerate() {
                per_cell[k].push(mcview::session::active(&s.world.get(mcview::session::bp(*p))));
            }
        }
        s.step();
    }
    let chart = chart_of(&s, declared, &power, &per_cell, &cells, entered, chart_total, warm, period, periods);
    let lamp = lamp_cell.and_then(|_| match entered {
        Some(t) if t < lamp_total => Some(Lamp::Entered),
        _ => lamp_of(&power[..power.len().min(lamp_total as usize)], &lit[..lit.len().min(lamp_total as usize)], warm),
    });
    ClockView { chart, lamp }
}

#[allow(clippy::too_many_arguments)]
fn chart_of(s: &mcview::session::Session, declared: bool, power: &[bool], per_cell: &[Vec<bool>], cells: &[mcview::Pos], entered: Option<u32>, total: u32, warm: u32, period: u32, periods: u32) -> Result<Chart, ChartErr> {
    let mut levels: Vec<u8>;
    let mut picked = String::new();
    if declared {
        if entered.is_some_and(|t| t < total) {
            return Err(ChartErr::Entered);
        }
        levels = power.iter().take(total as usize).map(|on| if *on { 15 } else { 0 }).collect();
    } else {
        let rises = |v: &Vec<bool>| (warm as usize + 1..v.len()).filter(|&t| v[t] && !v[t - 1]).count();
        let best = (0..cells.len()).max_by_key(|&k| (rises(&per_cell[k]), std::cmp::Reverse(k)));
        match best {
            Some(k) if rises(&per_cell[k]) > 0 => {
                levels = per_cell[k].iter().map(|on| if *on { 15 } else { 0 }).collect();
                picked = s.design.blocks.get(&cells[k]).map(|st| st.trim_start_matches("minecraft:").split('[').next().unwrap_or("").replace('_', " ")).unwrap_or_default();
            }
            _ => return Err(ChartErr::NoBlock),
        }
    }
    if levels.iter().all(|l| *l == 0) {
        return Err(ChartErr::StayedOff(total));
    }
    // the start-up the view shows: when the judge's warm-up (the game's, e.g. waiting for the
    // daylight sensor's update) is much longer than the simulation's first pulse, the chart starts
    // where the view does (critique: "starting up (2000 ticks)" while the view pulses after 5 s)
    let mut warm = warm;
    let mut total = total.min(levels.len() as u32);
    if let Some(first) = (0..levels.len()).find(|&t| levels[t] > 0 && (t == 0 || levels[t - 1] == 0)) {
        let first = first as u32;
        if warm > first + 2 * period {
            warm = first;
            total = (warm + periods * period).min(levels.len() as u32);
            levels.truncate(total as usize);
        }
    }
    let runs = site_model::WaveRow::from_levels("out", false, &levels).steps;
    Ok(Chart { runs, ticks: total, warm, picked })
}

fn lamp_of(power: &[bool], lit: &[bool], warm: u32) -> Option<Lamp> {
    let rises = |v: &[bool], from: usize| (from.max(1)..v.len()).filter(|&t| v[t] && !v[t - 1]).count();
    // from the output's first pulse after the start-up (the simulation's start-up can be shorter
    // than the game's: then its first pulse)
    let first = |from: usize| (from.max(1)..power.len()).find(|&t| power[t] && !power[t - 1]);
    let start = first(warm as usize).or_else(|| first(1))?;
    let pr = rises(power, start + 1);
    if pr < 2 {
        return None;
    }
    let mut gap = u32::MAX;
    let mut run = 0;
    for &p in &power[start..] {
        if p {
            if run > 0 {
                gap = gap.min(run);
            }
            run = 0;
        } else {
            run += 1;
        }
    }
    let dark = lit[start..].iter().any(|l| !l);
    let verdict = if !dark { Some("stays lit") } else if rises(lit, start + 1) < pr { Some("misses pulses") } else { None };
    Some(match verdict {
        Some(v) if gap <= 4 => Lamp::Fast(v.to_string(), gap),
        // its power is off longer, yet the lamp stays lit: something relights it within the tick
        // its power goes off (clk-misc saw it in the game on clk-obs-p12-bulb-obs-rb-stk: the
        // moving redstone block, the coordinator 03:20Z)
        Some("stays lit") => Lamp::Fast("relit".to_string(), gap),
        Some(_) => Lamp::Unexplained(gap),
        None => Lamp::Follows(gap),
    })
}
