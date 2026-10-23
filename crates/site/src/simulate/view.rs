//! The 3D view's own session on a design: the view's cache domain (build.rs hashes this file with
//! redsim and the viewer): whether it runs a design, a shulker machine's run, where a circuit's
//! demo stops, a clock's chart and lamp check, a thumbnail's still frame.

use serde::{Deserialize, Serialize};

/// Game ticks a shulker machine is run at most the way its 3D view runs it, until it has sent out
/// four boxes (a 64-stack filter loader takes ~13800 gt a box).
pub const VIEW_RUN: u32 = 60_000;
/// Boxes the view's run waits for (per OUT chest).
const VIEW_BOXES: usize = 4;
/// Game ticks of a circuit's demo run at most to see whether the view plays it to its end.
const DEMO_MAX: u32 = 200_000;

/// What the 3D view does with a circuit design: why it cannot run it (its own words, None: it
/// runs it), and a shulker machine's run.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ViewCheck {
    pub unsimulated: Option<String>,
    pub view_run: Option<site_model::ViewRun>,
}

/// The level a display's lit still settles at: 5 (eleven of its thirteen lamps, a digit that
/// reads one way only: seen from behind it would read mirrored).
pub const STILL_LEVEL: u8 = 5;

/// Game ticks a display's lit still runs after its input is set (its slowest change is 32 gt).
const STILL_SETTLE: u32 = 200;

/// A display's lit still (the coordinator 2026-10-22: the 0-9 display's card and page showed its
/// 13 lamps dark at tick 0; "ship the settled frame's blocks as the still; no simulation on
/// load"): the design run in the view's own session with its strength input at `level`, settled,
/// written back as a design, every block as it stands then (the face's lamps lit, the judge's
/// input chest filled to the level), the design's header and ports kept. None when the view does
/// not run it or it has no strength input.
pub fn lit_snapshot(text: &str, level: u8) -> Option<String> {
    let mut s = mcview::session::Session::new(text, &mcview::session::Options::default()).ok()?;
    if !s.simulated() {
        return None;
    }
    let i = s.circuit_inputs.iter().position(|c| matches!(c.how, mcview::circuit::InputHow::Strength { .. }))?;
    let input = s.circuit_inputs[i].pos;
    s.set_level(i, level);
    for _ in 0..STILL_SETTLE {
        s.step();
    }
    if s.halted.is_some() {
        return None;
    }
    let mut d = mctas::design::Design::parse(text).ok()?;
    d.blocks = s.render_blocks(0.0).into_iter().filter(|b| b.offset == [0.0; 3] && b.state != "minecraft:air").map(|b| (b.cell, b.state)).collect();
    d.block_entities.insert(input, mcview::circuit::strength_items(level));
    Some(d.to_text())
}

/// The 3D view's own session on a circuit design.
pub fn view_check(text: &str) -> ViewCheck {
    view_check_with(text, None)
}

/// [`view_check`] with a big crop farm's judge's speed (the page's `data-farm-rts`,
/// `Options::farm_rts`: without it the view shows a big farm as built).
pub fn view_check_with(text: &str, farm_rts: Option<u32>) -> ViewCheck {
    match mcview::session::Session::new(text, &mcview::session::Options { farm_rts, ..Default::default() }) {
        Ok(mut s) => {
            let view_run = (s.unsimulated.is_none() && s.shulker.is_some()).then(|| view_run(&mut s, VIEW_RUN));
            ViewCheck { unsimulated: s.unsimulated.clone(), view_run }
        }
        Err(e) => ViewCheck { unsimulated: Some(format!("the viewer cannot load it: {e}")), view_run: None },
    }
}

/// Where the 3D view's demo of a circuit stops, raw (`tick 44: dropper at (1,0,2) picks a random
/// slot among 9 stacks (not modeled)`), None when it plays to its end: the judge's stimulus on its
/// inputs, the way the page's autoplay runs it (the viewer's live.rs: `parse_stimulus(.., 40)`).
/// Where redsim meets something it does not model, the view stops at the last exact state and
/// says so (`Session::halted`): a page must not promise a demo that stops there (webrender's QA:
/// the 100- and 576-item counters stop at tick 44, their droppers holding several stacks).
pub fn demo_halt(text: &str, stimulus: &str) -> Option<String> {
    if stimulus.trim().is_empty() {
        return None;
    }
    let mut s = mcview::session::Session::new(text, &Default::default()).ok()?;
    // (a design the view does not run at all is shown as built: `ViewCheck::unsimulated`)
    if !s.simulated() {
        return None;
    }
    let names: Vec<String> = s.circuit_inputs.iter().map(|c| c.name.clone()).collect();
    if names.is_empty() {
        return None;
    }
    let d = mcview::circuit::parse_stimulus(stimulus, &names, 40).ok()?;
    let ticks = d.ticks.min(DEMO_MAX);
    s.attach_demo(d);
    for _ in 0..ticks {
        if s.halted.is_some() {
            break;
        }
        s.step();
    }
    s.halted.clone()
}

/// A thumbnail's still frame (`data-still`, webrender's contract): the viewer's own still-frame
/// search, run at build time so the browser plays straight to it; and the blocks at that frame,
/// the picture the card shows (crates/site/src/mechanism.rs folds the clocks of a period whose
/// pictures differ only by swappable blocks).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Still {
    pub attr: Option<String>,
    /// Every block at the still frame in the design's box and one cell around it (what moved out
    /// of it), the view's floor aside: (cell, state).
    pub cells: Vec<([i32; 3], String)>,
}

pub fn still(text: &str) -> Still {
    let Ok(mut s) = mcview::session::Session::new(text, &mcview::session::Options::default()) else { return Still::default() };
    let attr = s.still_attr(mcview::session::STILL_HORIZON);
    let mut cells = Vec::new();
    if !s.design.blocks.is_empty() {
        let lo = [0, 1, 2].map(|k| s.design.blocks.keys().map(|p| p[k]).min().unwrap_or(0) - 1);
        let hi = [0, 1, 2].map(|k| s.design.blocks.keys().map(|p| p[k]).max().unwrap_or(0) + 1);
        let floor = s.floor.as_ref().map(|f| (f.lo, f.hi));
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    let p = [x, y, z];
                    if floor.is_some_and(|(a, b)| (0..3).all(|k| p[k] >= a[k] && p[k] <= b[k])) {
                        continue;
                    }
                    let b = s.world.get(mcview::session::bp(p));
                    if !b.is_air() {
                        cells.push((p, b.to_string()));
                    }
                }
            }
        }
    }
    Still { attr: Some(attr), cells }
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

#[cfg(test)]
mod tests {
    /// The 0-9 display's lit still (the coordinator 2026-10-22): its face settled at 5, read from
    /// the front (x runs right to left there): the top, middle and bottom rows lit, the upper left
    /// lamp and the lower right one lit, their mirror twins dark; its header and ports kept, its
    /// input chest filled to the level; the still loads as a design the view shows without a tick.
    #[test]
    fn the_display_still_shows_a_lit_five() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/display/archive/seven-segment-decimal.txt")).unwrap();
        let still = super::lit_snapshot(&text, super::STILL_LEVEL).expect("a lit still");
        let d = mctas::design::Design::parse(&still).unwrap();
        let lit = |x: i32, y: i32| d.blocks.get(&[x, y, -21]).map(|s| s.as_str());
        for y in [4, 6, 8] {
            for x in 0..3 {
                assert_eq!(lit(x, y), Some("minecraft:redstone_lamp[lit=true]"), "row {y}, x {x}");
            }
        }
        // (from the front, +x is on the left: x = 2 is the face's left column)
        assert_eq!(lit(2, 7), Some("minecraft:redstone_lamp[lit=true]"), "upper left");
        assert_eq!(lit(0, 7), Some("minecraft:redstone_lamp[lit=false]"), "upper right");
        assert_eq!(lit(0, 5), Some("minecraft:redstone_lamp[lit=true]"), "lower right");
        assert_eq!(lit(2, 5), Some("minecraft:redstone_lamp[lit=false]"), "lower left");
        assert_eq!(d.meta.get("circuit").map(|s| s.as_str()), Some("seven-segment 4 hex decimal"));
        assert_eq!(d.ports.len(), 14, "its input and its 13 outputs");
        assert_eq!(d.block_entities.get(&[0, 2, 2]), Some(&mcview::circuit::strength_items(super::STILL_LEVEL)));
        let s = mcview::session::Session::new(&still, &mcview::session::Options::still()).unwrap();
        assert!(s.render_blocks(0.0).iter().filter(|b| b.state == "minecraft:redstone_lamp[lit=true]").count() >= 11);
        // played from the still (the page's view simulates it on play): its chest emptied, the
        // face shows 0 (the middle lamp dark, both upper and lower sides lit); filled to 8, all 13
        let mut p = mcview::session::Session::new(&still, &mcview::session::Options::live()).unwrap();
        let i = p.circuit_inputs.iter().position(|c| matches!(c.how, mcview::circuit::InputHow::Strength { .. })).unwrap();
        let face = |p: &mcview::session::Session, x: i32, y: i32| p.render_blocks(0.0).into_iter().find(|b| b.cell == [x, y, -21]).map(|b| b.state).unwrap_or_default();
        p.set_level(i, 0);
        for _ in 0..200 {
            p.step();
        }
        assert_eq!(face(&p, 1, 6), "minecraft:redstone_lamp[lit=false]", "0: the middle lamp dark");
        assert_eq!(face(&p, 0, 7), "minecraft:redstone_lamp[lit=true]", "0: upper right lit");
        assert_eq!(face(&p, 2, 5), "minecraft:redstone_lamp[lit=true]", "0: lower left lit");
        p.set_level(i, 8);
        for _ in 0..200 {
            p.step();
        }
        assert_eq!(p.render_blocks(0.0).iter().filter(|b| b.cell[2] == -21 && b.state == "minecraft:redstone_lamp[lit=true]").count(), 13, "8: every lamp");
    }
}
