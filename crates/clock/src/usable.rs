//! The usable-output rule (vjeux 2026-10-21 on clk-rt-ring3x3-p10-r-12b: "Can we have some better
//! heuristics for out placement. This one is unusable for a redstone circuit"; the coordinator's
//! rule, reading D, widened to the standard taps). A clock's output is where a builder takes its
//! signal into a circuit, with one of the standard TAPS ([`TapKind`]) in the tap cell:
//!
//! * a REPEATER reading the signal in the tap cell (the tap cell is the `port output`);
//! * a COMPARATOR reading a container or block of the build behind it, or an OBSERVER watching a
//!   block of the build that changes every cycle: such a tap joins the design (with a support
//!   under a comparator), and its output, the next cell outward, is the `port output`.
//!
//! The tap cell: an AIR cell free all cycle that the tap stands on (the build's ground level, the
//! lowest layer of its swept box; above it, a sturdy block of the build or a free cell a builder
//! puts a block in: the coordinator, "what a builder does"); on the exterior (its input side toward
//! the build inside the swept box, the way out horizontal and leaving it); the line outward free
//! all cycle with nothing of the build next to it (a line leads away); not next to a moving part (a
//! cell whose block comes and goes, a piston head or moving block, an entity's cell).
//!
//! The TAP RUN: the tap in its cell (delay-1 repeater, comparator in compare mode, observer; its
//! input toward the build, its output outward), stone under it and under the line where the build
//! has no block, the judge's lamp in the next cell: the clock keeps its period and the lamp rises
//! at evenly spaced ticks, every g ticks, g the clock's own pulse spacing (the gaps of its archived
//! output: a move never changes a clock's time). A repeater tap also passes the judge's lamp in the
//! tap cell itself (the port's own test). A comparator or observer tap's design (the tap joined)
//! must also pass the repeater test at its port: a fragile build can run another way with a
//! repeater there than with the lamp.
//!
//! The judge records it as a property of the declared output (`usable`: `yes: <tap>`, or `no:
//! <why>`, a plain warning on the site; never a reason to hide); `clock retap --usable` moves a
//! shown clock's unusable output to the best usable tap: the lowest level (the ground first), then
//! the nearest the build's middle, then the evenest pulse (equal widths, then a duty nearest half),
//! then repeater over comparator over observer, then the position. A usable output stays where it
//! is; a hidden clock is judged, never moved.

use crate::sim::{SimOptions, SimReport, Verdict, drive, simulate};
use crate::wave::CellWave;
use crate::{Clock, Pos};
use std::collections::{HashMap, HashSet};

const H4: [[i32; 3]; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const N6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn add(a: Pos, b: [i32; 3]) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn below(p: Pos) -> Pos {
    [p[0], p[1] - 1, p[2]]
}

/// A horizontal direction's name (`east`).
fn facing(v: [i32; 3]) -> &'static str {
    match (v[0], v[2]) {
        (1, _) => "east",
        (-1, _) => "west",
        (_, 1) => "south",
        _ => "north",
    }
}

/// What a builder puts in the tap cell to take the signal (the coordinator, 2026-10-21: the
/// standard taps).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TapKind {
    /// A repeater reading the signal in the tap cell (the tap cell is the port).
    Repeater,
    /// A comparator reading a container or block of the build behind it (it joins the design;
    /// the port is in front of it).
    Comparator,
    /// An observer watching a block of the build that changes every cycle (it joins the design;
    /// the port is behind it).
    Observer,
}

impl TapKind {
    pub const ALL: [TapKind; 3] = [TapKind::Repeater, TapKind::Comparator, TapKind::Observer];

    pub fn word(self) -> &'static str {
        match self {
            TapKind::Repeater => "repeater",
            TapKind::Comparator => "comparator",
            TapKind::Observer => "observer",
        }
    }

    /// The tap's block for the way out `dir`: its input (a diode's `facing`, an observer's face)
    /// toward the build.
    pub fn block(self, dir: [i32; 3]) -> String {
        let toward = facing([-dir[0], 0, -dir[2]]);
        match self {
            TapKind::Repeater => format!("minecraft:repeater[delay=1,facing={toward},locked=false,powered=false]"),
            TapKind::Comparator => format!("minecraft:comparator[facing={toward},mode=compare,powered=false]"),
            TapKind::Observer => format!("minecraft:observer[facing={toward},powered=false]"),
        }
    }

    /// Whether the tap joins the design (its output is the port): a comparator or an observer.
    pub fn joins(self) -> bool {
        self != TapKind::Repeater
    }

    /// The tap in words, for the reasons.
    fn phrase(self) -> &'static str {
        match self {
            TapKind::Repeater => "with a repeater in the output cell",
            TapKind::Comparator => "with a comparator reading the build",
            TapKind::Observer => "with an observer watching the build",
        }
    }
}

/// A tap: its cell, the way out of the build (a horizontal unit step) and its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Tap {
    pub out: Pos,
    pub dir: [i32; 3],
    pub kind: TapKind,
}

impl Tap {
    /// The next cell outward (the judge's lamp in the tap run).
    pub fn next(&self) -> Pos {
        add(self.out, self.dir)
    }
    /// The cell the tap reads (the build side).
    pub fn input(&self) -> Pos {
        [self.out[0] - self.dir[0], self.out[1] - self.dir[1], self.out[2] - self.dir[2]]
    }
    /// The way out as a word (`west`).
    pub fn way(&self) -> &'static str {
        facing(self.dir)
    }
    /// The line a builder lays outward, after the tap cell (a joined tap's first cell is its port).
    pub fn line(&self) -> Vec<Pos> {
        let n = self.next();
        if self.kind.joins() { vec![n, add(n, self.dir)] } else { vec![n] }
    }
    /// Where the signal is read (the `port output`): the tap cell of a repeater, the cell after a
    /// joined tap.
    pub fn port(&self) -> Pos {
        if self.kind.joins() { self.next() } else { self.out }
    }
}

/// The repeater a builder puts in the output cell (delay 1, its input on the build side).
pub fn tap_repeater(dir: [i32; 3]) -> String {
    TapKind::Repeater.block(dir)
}

/// The clock with its output at `out` (the judge's lamp goes there).
pub fn with_output(c: &Clock, out: Pos) -> anyhow::Result<Clock> {
    let mut d = c.design.clone();
    d.ports.insert("output".into(), out);
    Clock::from_design(d)
}

/// Where a tap may stand. The default is the coordinator's ruling (reading D, 2026-10-21): the
/// ground level, or above it on a sturdy block of the build or over a free cell (a block put under
/// it). [`Rule::strict`]: the ground level only (reading A, for comparison).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Also a cell above the ground standing on a block of the build (its top face sturdy, a block
    /// that stays all cycle).
    pub lift: bool,
    /// Also a cell above the ground over a free cell (a builder puts a block there: the tap run
    /// places stone).
    pub prop: bool,
}

impl Default for Rule {
    fn default() -> Self {
        Rule { lift: true, prop: true }
    }
}

impl Rule {
    /// The coordinator's words before the ruling (reading A): the ground level only.
    pub fn strict() -> Rule {
        Rule { lift: false, prop: false }
    }
}

/// What one run of the free clock (no output port) shows around it.
pub struct Scene {
    pub swept: (Pos, Pos),
    /// The ground level: the swept box's lowest layer (the ground is under it).
    pub ground: i32,
    /// Every cell a block stands in at some tick.
    pub filled: HashSet<Pos>,
    /// Cells whose block comes and goes (or changes kind, or is a piston head or a moving block at
    /// some tick) and the cells an entity occupies.
    pub moving: HashSet<Pos>,
    /// Cells holding the same block every tick (a comparator can read them).
    pub steady: HashSet<Pos>,
    /// Steady cells whose state changes during the cycle (an observer can watch them).
    pub changing: HashSet<Pos>,
    /// Steady blocks a tap can stand on (a sturdy top face): for [`Rule::lift`].
    pub supports: HashSet<Pos>,
    pub rule: Rule,
}

impl Scene {
    fn inside(&self, q: Pos) -> bool {
        let (lo, hi) = self.swept;
        (0..3).all(|k| q[k] >= lo[k] && q[k] <= hi[k])
    }

    /// Whether a cell above the ground can stand on `under` by the rule: a sturdy block of the
    /// build ([`Rule::lift`]), or a free cell a builder puts a block in ([`Rule::prop`]).
    fn supported(&self, under: &Pos) -> bool {
        (self.rule.lift && self.supports.contains(under)) || (self.rule.prop && !self.filled.contains(under) && !self.moving.contains(under))
    }

    /// What the tap at `p` stands on, in words.
    pub fn stand(&self, p: Pos) -> &'static str {
        if p[1] == self.ground {
            "on the ground"
        } else if self.supports.contains(&below(p)) {
            "on a block of the build"
        } else {
            "on a block put under it"
        }
    }
}

/// The free clock's [`Scene`] over its warmup and one period (`base`: its redsim report).
pub fn scene(free: &Clock, base: &SimReport, rule: Rule) -> Result<Scene, String> {
    let (slo, shi) = base.swept;
    let (lo, hi) = free.bounds();
    let from = [lo[0].min(slo[0]) - 4, lo[1].min(slo[1]) - 4, lo[2].min(slo[2]) - 4];
    let to = [hi[0].max(shi[0]) + 4, hi[1].max(shi[1]) + 4, hi[2].max(shi[2]) + 4];
    let ticks = base.state_warmup + base.period;
    let d = drive(free, &[], ticks, &[], from, to).map_err(|(_, why)| why)?;
    if let Some((t, why)) = &d.stopped {
        return Err(format!("the run stops at tick {t}: {why}"));
    }
    let name = |s: &str| mctas::blockstate::block_name(crate::wave::block_part(s)).to_string();
    // per cell: ticks present, its first block name and state, whether the name or state changed
    struct Seen {
        ticks: usize,
        name: String,
        state: String,
        renamed: bool,
        restated: bool,
    }
    let mut seen: HashMap<Pos, Seen> = HashMap::new();
    let mut moving: HashSet<Pos> = base.occupied.iter().copied().collect();
    for frame in &d.cells {
        for (p, s) in frame {
            let n = name(s);
            if n.contains("moving_piston") || n.contains("piston_head") {
                moving.insert(*p);
            }
            match seen.get_mut(p) {
                None => {
                    seen.insert(*p, Seen { ticks: 1, name: n, state: s.clone(), renamed: false, restated: false });
                }
                Some(e) => {
                    e.ticks += 1;
                    e.renamed |= e.name != n;
                    e.restated |= e.state != *s;
                }
            }
        }
    }
    let frames = d.cells.len();
    let filled: HashSet<Pos> = seen.keys().copied().collect();
    for (p, e) in &seen {
        if e.ticks < frames || e.renamed {
            moving.insert(*p);
        }
    }
    let steady: HashSet<Pos> = seen.iter().filter(|(p, e)| e.ticks == frames && !e.renamed && !moving.contains(*p)).map(|(p, _)| *p).collect();
    let changing: HashSet<Pos> = steady.iter().filter(|p| seen[*p].restated).copied().collect();
    let supports = free
        .design
        .blocks
        .iter()
        .filter(|(p, _)| steady.contains(*p))
        .filter(|(_, st)| redsim::Block::parse(st).is_ok_and(|b| redsim::behavior::sturdy_of(&b, redsim::Dir::Up, redsim::behavior::Support::Rigid)))
        .map(|(p, _)| *p)
        .collect();
    Ok(Scene { swept: base.swept, ground: slo[1], filled, moving, steady, changing, supports, rule })
}

/// The taps the rule's geometry allows (before any run): see the module doc. A comparator needs a
/// steady block to read, an observer a changing one.
pub fn candidates(s: &Scene) -> Vec<Tap> {
    let (slo, shi) = s.swept;
    let top = if s.rule.lift || s.rule.prop { shi[1] } else { s.ground };
    let mut out = Vec::new();
    for y in s.ground..=top {
        for x in slo[0] - 1..=shi[0] + 1 {
            for z in slo[2] - 1..=shi[2] + 1 {
                let op = [x, y, z];
                if y > s.ground && !s.supported(&below(op)) {
                    continue;
                }
                if s.filled.contains(&op) || N6.iter().any(|e| s.moving.contains(&add(op, *e))) {
                    continue;
                }
                for d in H4 {
                    let probe = Tap { out: op, dir: d, kind: TapKind::Repeater };
                    if !s.inside(probe.input()) || s.inside(probe.next()) {
                        continue;
                    }
                    'kinds: for kind in TapKind::ALL {
                        match kind {
                            TapKind::Comparator if !s.steady.contains(&probe.input()) => continue,
                            TapKind::Observer if !s.changing.contains(&probe.input()) => continue,
                            _ => {}
                        }
                        let tap = Tap { kind, ..probe };
                        let line = tap.line();
                        for (k, &c) in line.iter().enumerate() {
                            let prev = if k == 0 { op } else { line[k - 1] };
                            if s.filled.contains(&c) || s.moving.contains(&c) {
                                continue 'kinds;
                            }
                            // nothing of the build next to the line (its previous cell and the
                            // cell under it aside)
                            if N6.iter().map(|e| add(c, *e)).any(|q| q != prev && q != below(c) && s.filled.contains(&q)) {
                                continue 'kinds;
                            }
                        }
                        out.push(tap);
                    }
                }
            }
        }
    }
    out
}

/// How near a cell is to the build's middle (twice the horizontal distance, squared).
pub fn middle_distance(s: &Scene, q: Pos) -> i64 {
    let (lo, hi) = s.swept;
    let dx = (2 * q[0] + 1 - (lo[0] + hi[0] + 1)) as i64;
    let dz = (2 * q[2] + 1 - (lo[2] + hi[2] + 1)) as i64;
    dx * dx + dz * dz
}

/// How even a pulse is (smaller is evener): the spread of its widths, then how far its duty is
/// from half the period.
pub fn evenness(w: &CellWave, period: u32) -> (u32, u32) {
    let spread = w.widths.iter().max().copied().unwrap_or(0) - w.widths.iter().min().copied().unwrap_or(0);
    (spread, (2 * w.on as i64 - period as i64).unsigned_abs() as u32)
}

/// The clock with the tap in place (the tap run): stone under the tap cell and the line where the
/// build has no block (and always at the ground), the tap in its cell, the judge's lamp in the
/// next cell (its `port output`).
pub fn with_tap(c: &Clock, tap: Tap, s: &Scene) -> anyhow::Result<Clock> {
    let mut d = c.design.clone();
    for p in std::iter::once(tap.out).chain(tap.line()) {
        if p[1] == s.ground || !d.blocks.contains_key(&below(p)) {
            d.blocks.insert(below(p), "minecraft:stone".into());
        }
    }
    d.blocks.insert(tap.out, tap.kind.block(tap.dir));
    d.ports.insert("output".into(), tap.next());
    Clock::from_design(d)
}

/// The design a joined tap makes (`clock retap --usable --write`): the comparator or observer in
/// its cell (a comparator with stone under it where the build has none), the port at its output.
pub fn joined(c: &Clock, tap: Tap) -> anyhow::Result<Clock> {
    let mut d = c.design.clone();
    if tap.kind == TapKind::Comparator && !d.blocks.contains_key(&below(tap.out)) {
        d.blocks.insert(below(tap.out), "minecraft:stone".into());
    }
    d.blocks.insert(tap.out, tap.kind.block(tap.dir));
    d.ports.insert("output".into(), tap.next());
    Clock::from_design(d)
}

/// A tap that passes: the tap run's report, and a repeater tap's lamp run in its cell.
#[derive(Clone, Debug)]
pub struct Usable {
    pub tap: Tap,
    pub lamp: Option<SimReport>,
    pub tapped: SimReport,
}

impl Usable {
    /// The wave the ranking weighs: the lamp in a repeater's cell, else the lamp after the tap.
    fn wave(&self) -> &CellWave {
        let r = self.lamp.as_ref().unwrap_or(&self.tapped);
        &r.output.as_ref().expect("checked").wave
    }
}

/// Why a wave is not the clock's pulse train: a clock of `period` whose output rises at evenly
/// spaced ticks, every `gap` ticks when given (the clock's own spacing).
fn pulse_problem(what: &str, verdict: Verdict, why: &str, got_period: u32, w: Option<&CellWave>, period: u32, gap: Option<u32>) -> Result<(), String> {
    if verdict != Verdict::Clock {
        return Err(format!("{what}: {} {why}", verdict.word()));
    }
    if got_period != period {
        return Err(format!("{what} the clock runs at period {got_period} gt, not {period}"));
    }
    let w = w.ok_or_else(|| format!("{what}: no output"))?;
    if w.gaps.is_empty() {
        return Err(format!("{what}, no signal (it never turns on)"));
    }
    if let Some((lo, hi)) = w.irregular() {
        return Err(format!("{what}, it rises unevenly (gaps {lo} to {hi} gt)"));
    }
    if let Some(g) = gap
        && w.gaps[0] != g
    {
        return Err(format!("{what}, it pulses every {} gt, the clock every {g} gt", w.gaps[0]));
    }
    Ok(())
}

/// Why a tap run (redsim's or the game's: its verdict, period and the lamp after the tap) does not
/// carry the clock's pulses (every `gap` ticks), or Ok.
#[allow(clippy::too_many_arguments)]
pub fn tap_problem_parts(kind: TapKind, verdict: Verdict, why: &str, got_period: u32, w: Option<&CellWave>, period: u32, gap: Option<u32>) -> Result<(), String> {
    pulse_problem(kind.phrase(), verdict, why, got_period, w, period, gap)
}

/// The lamp run in a repeater's cell (the port's own test) and the tap run on redsim, for one tap
/// of the free clock (`period`: its own; `gap`: its pulse spacing). Err: why it fails.
pub fn try_tap(free: &Clock, tap: Tap, s: &Scene, period: u32, gap: Option<u32>) -> Result<Usable, String> {
    let lamp = if tap.kind == TapKind::Repeater {
        let lc = with_output(free, tap.out).map_err(|e| format!("{e:#}"))?;
        let r = simulate(&lc, &SimOptions::default());
        pulse_problem("with the lamp there", r.verdict, &r.why, r.period, r.output.as_ref().map(|o| &o.wave), period, gap)?;
        Some(r)
    } else {
        None
    };
    let tc = with_tap(free, tap, s).map_err(|e| format!("{e:#}"))?;
    let tapped = simulate(&tc, &SimOptions::default());
    tap_problem_parts(tap.kind, tapped.verdict, &tapped.why, tapped.period, tapped.output.as_ref().map(|o| &o.wave), period, gap)?;
    // every pulse: the tap's own signal into the port (what the lamp reads, before the lamp's
    // 4-tick fall merges close pulses: an observer firing on both edges of a short pulse) rises as
    // often, at the same spacing
    let lamp_gap = tapped.output.as_ref().map(|o| o.wave.gaps[0]).unwrap_or(0);
    let raw = raw_gaps(&tc, tap.next(), tapped.state_warmup.max(tapped.warmup), tapped.period)?;
    if raw.is_empty() || raw.iter().any(|g| *g != lamp_gap) {
        return Err(format!("{}, its own signal pulses at gaps {raw:?} gt (a lamp after it reads every {lamp_gap} gt): a circuit after it sees other pulses", tap.kind.phrase()));
    }
    // a joined tap's design is a clock of its own whose port takes a repeater like any other (a
    // fragile build can run another way with a repeater there than with the lamp: a crafter loop
    // whose comparator a repeater read at gaps of 10 and 81 gt)
    if tap.kind.joins() {
        let j = joined(free, tap).map_err(|e| format!("{e:#}"))?;
        judge(&j, s.rule, gap).map_err(|e| format!("{}, then a repeater after it: {e}", tap.kind.phrase()))?;
    }
    Ok(Usable { tap, lamp, tapped })
}

/// The gaps between the rises of the signal into `port` (the best signal its neighbours give: what
/// a lamp there sees before its own delay) over one period after `warmup`, cyclic.
fn raw_gaps(c: &Clock, port: Pos, warmup: u32, period: u32) -> Result<Vec<u32>, String> {
    let (lo, hi) = c.bounds();
    let from = [lo[0].min(port[0]) - 6, lo[1].min(port[1]) - 6, lo[2].min(port[2]) - 6];
    let to = [hi[0].max(port[0]) + 6, hi[1].max(port[1]) + 6, hi[2].max(port[2]) + 6];
    let d = drive(c, &[], warmup + period, &[port], from, to).map_err(|(_, why)| why)?;
    if let Some((t, why)) = &d.stopped {
        return Err(format!("the tap run stops at tick {t}: {why}"));
    }
    let on: Vec<bool> = (warmup..warmup + period).map(|t| d.signals.get(t as usize).and_then(|s| s.first()).is_some_and(|v| *v > 0)).collect();
    let n = on.len();
    let rises: Vec<usize> = (0..n).filter(|&i| on[i] && !on[(i + n - 1) % n]).collect();
    Ok((0..rises.len()).map(|k| ((rises[(k + 1) % rises.len()] + n - rises[k] - 1) % n + 1) as u32).collect())
}

/// The free clock (no output port) and its redsim report, or why it is not a clock.
fn free_of(c: &Clock) -> Result<(Clock, SimReport), String> {
    let mut d = c.design.clone();
    d.ports.remove("output");
    let free = Clock::from_design(d).map_err(|e| format!("{e:#}"))?;
    let base = simulate(&free, &SimOptions::default());
    if base.verdict != Verdict::Clock {
        return Err(format!("not a clock on redsim without an output: {} {}", base.verdict.word(), base.why));
    }
    Ok((free, base))
}

/// The best usable tap of a clock (its declared output left out), by the rule's ranking. Err: why
/// there is none (the reasons of the first candidates).
pub fn pick(c: &Clock, rule: Rule, gap: Option<u32>) -> Result<(Usable, Scene), String> {
    let (free, base) = free_of(c)?;
    let s = scene(&free, &base, rule)?;
    let mut cands = candidates(&s);
    if cands.is_empty() {
        return Err(format!("no air cell a tap can stand on (the ground at y = {}, a sturdy block or a free cell under it) on the exterior with a free line outward, away from moving parts", s.ground));
    }
    // the lowest level first (the ground), then the nearest the build's middle
    let tier_of = |t: &Tap| (t.out[1], middle_distance(&s, t.out));
    cands.sort_by_key(|t| (tier_of(t), t.kind, t.out, t.dir));
    let mut why: Vec<String> = Vec::new();
    let mut i = 0;
    while i < cands.len() {
        let tier = tier_of(&cands[i]);
        let mut pass: Vec<Usable> = Vec::new();
        while i < cands.len() && tier_of(&cands[i]) == tier {
            match try_tap(&free, cands[i], &s, base.period, gap) {
                Ok(u) => pass.push(u),
                Err(e) => {
                    if why.len() < 3 {
                        why.push(format!("{:?} {} {}: {e}", cands[i].out, cands[i].kind.word(), cands[i].way()));
                    }
                }
            }
            i += 1;
        }
        if !pass.is_empty() {
            pass.sort_by_key(|u| (evenness(u.wave(), base.period), u.tap.kind, u.tap.out, u.tap.dir));
            return Ok((pass.swap_remove(0), s));
        }
    }
    Err(format!("no usable tap among {} candidates: {}", cands.len(), why.join("; ")))
}

/// Whether a clock's declared output is usable (redsim): a repeater there carries the clock's
/// pulses (every `gap` ticks). Ok(the tap, the scene) or why not (the first reason).
pub fn judge(c: &Clock, rule: Rule, gap: Option<u32>) -> Result<(Tap, Scene), String> {
    let op = c.output.ok_or("no output port")?;
    let (free, base) = free_of(c)?;
    let s = scene(&free, &base, rule)?;
    let supported = op[1] == s.ground || (op[1] > s.ground && s.supported(&below(op)));
    if !supported {
        let why = if op[1] < s.ground { format!("is under the build (the build's ground level is y = {})", s.ground) } else { "has a moving or non-sturdy block under it".to_string() };
        return Err(format!("the output {op:?} {why}: a repeater there has nothing to stand on"));
    }
    let taps: Vec<Tap> = candidates(&s).into_iter().filter(|t| t.out == op && t.kind == TapKind::Repeater).collect();
    if taps.is_empty() {
        let why = if s.filled.contains(&op) {
            "is not free all cycle".to_string()
        } else if N6.iter().any(|e| s.moving.contains(&add(op, *e))) {
            "is next to a moving part".to_string()
        } else {
            "has no free line outward (the next cell taken, or next to the build)".to_string()
        };
        return Err(format!("the output {op:?} {why}"));
    }
    let mut first = None;
    for t in taps {
        match try_tap(&free, t, &s, base.period, gap) {
            Ok(_) => return Ok((t, s)),
            Err(e) => {
                first.get_or_insert(e);
            }
        }
    }
    Err(first.unwrap_or_else(|| "no tap".into()))
}

/// The `usable` column's word for a judgement of the declared output: `yes: repeater`, or `no:
/// <why>`.
pub fn word<T>(r: &Result<T, String>) -> String {
    match r {
        Ok(_) => "yes: repeater".into(),
        Err(e) => format!("no: {}", e.replace(['\t', '\n'], " ")),
    }
}

/// What `clock retap --usable` does to one archived clock (redsim).
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Its output is usable: it stays.
    Unchanged(Tap),
    /// Moved to the best usable tap: the old output and why it was not usable, the tap, what it
    /// stands on.
    Moved { from: Option<Pos>, was: String, tap: Tap, stand: &'static str },
    /// No usable tap: the output stays, with the warning.
    NoUsable(String),
    /// Not judged: why (redsim does not model it, not a clock without its output...).
    Skip(String),
}

/// Judge one clock's output by the rule and, when it is not usable and `movable`, pick the best
/// usable tap (`gap`: the clock's pulse spacing).
pub fn retap(c: &Clock, rule: Rule, gap: Option<u32>, movable: bool) -> Outcome {
    let was = match judge(c, rule, gap) {
        Ok((t, _)) => return Outcome::Unchanged(t),
        Err(e) => e,
    };
    if was.starts_with("not a clock on redsim") || was.starts_with("the run stops") {
        return Outcome::Skip(was);
    }
    if !movable {
        return Outcome::NoUsable(was);
    }
    match pick(c, rule, gap) {
        Ok((u, s)) => Outcome::Moved { from: c.output, was, tap: u.tap, stand: s.stand(u.tap.out) },
        Err(e) => Outcome::NoUsable(format!("{was}; {e}")),
    }
}

/// A judge line (`# CLOCK: ... on 4/10, edges 1, warmup 4, ..., 12 blocks, 3x2x3, robust`) with
/// the output's numbers, the block count and the box replaced.
pub fn patch_line(line: &str, on: u32, edges: u32, warmup: u32, blocks: usize, dims: &str) -> String {
    let mut out = String::new();
    for (k, part) in line.split(", ").enumerate() {
        if k > 0 {
            out.push_str(", ");
        }
        let is_dims = part.split('x').count() == 3 && part.split('x').all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()));
        if let Some(rest) = part.strip_prefix("on ")
            && let Some((_, p)) = rest.split_once('/')
        {
            out.push_str(&format!("on {on}/{p}"));
        } else if part.starts_with("edges ") {
            out.push_str(&format!("edges {edges}"));
        } else if part.starts_with("warmup ") {
            out.push_str(&format!("warmup {warmup}"));
        } else if part.ends_with(" blocks") && part.trim_end_matches(" blocks").chars().all(|c| c.is_ascii_digit()) {
            out.push_str(&format!("{blocks} blocks"));
        } else if is_dims {
            out.push_str(dims);
        } else {
            out.push_str(part);
        }
    }
    out
}

/// The comment a moved design carries.
fn moved_comment(tap: Tap, stand: &str, from: Option<Pos>, was: &str) -> String {
    let from_s = from.map(|f| format!("was {} {} {}: {}", f[0], f[1], f[2], was.replace('\n', " "))).unwrap_or_else(|| "no output before".into());
    let what = match tap.kind {
        TapKind::Repeater => format!("an air cell where a repeater facing {} ({stand}) takes every pulse", tap.way()),
        TapKind::Comparator => format!("the output of a comparator facing {} ({stand}) that reads the build, added at {} {} {}", tap.way(), tap.out[0], tap.out[1], tap.out[2]),
        TapKind::Observer => format!("the output of an observer watching the build to its {}, added at {} {} {}", facing([-tap.dir[0], 0, -tap.dir[2]]), tap.out[0], tap.out[1], tap.out[2]),
    };
    format!("# port output moved 2026-10-21 (usable-output rule): {what} ({from_s})")
}

/// A repeater tap's design text: its `port output` moved to the tap cell, with a comment.
fn moved_text(text: &str, tap: Tap, comment: &str) -> anyhow::Result<String> {
    let p = tap.port();
    let port = format!("port output: {} {} {}", p[0], p[1], p[2]);
    let lines: Vec<&str> = text.lines().collect();
    // no output yet: after the last `port` line, else after `name:`
    let insert_at = if lines.iter().any(|l| l.starts_with("port output:")) { None } else { lines.iter().rposition(|l| l.starts_with("port ")).or_else(|| lines.iter().position(|l| l.starts_with("name:"))) };
    let mut out = String::with_capacity(text.len() + 300);
    let mut done = false;
    for (i, l) in lines.iter().enumerate() {
        if l.starts_with("port output:") {
            out.push_str(&format!("{comment}\n{port}\n"));
            done = true;
            continue;
        }
        out.push_str(l);
        out.push('\n');
        if Some(i) == insert_at {
            out.push_str(&format!("{comment}\n{port}\n"));
            done = true;
        }
    }
    if !done {
        anyhow::bail!("no `name:` line to put `port output:` after");
    }
    Ok(out)
}

/// A joined tap's design text: the old text's comments (in order), the comment, then the design
/// with the tap ([`joined`]).
fn joined_text(text: &str, c: &Clock, tap: Tap, comment: &str) -> anyhow::Result<String> {
    let j = joined(c, tap)?;
    let mut out = String::new();
    for l in text.lines().filter(|l| l.starts_with('#')) {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(comment);
    out.push('\n');
    out.push_str(&j.design.to_text());
    Ok(out)
}

/// The `# CLOCK:` line of a design text replaced.
fn with_judge_line(text: &str, f: impl Fn(&str) -> String) -> String {
    let mut out = String::with_capacity(text.len());
    for l in text.lines() {
        match l.strip_prefix("# CLOCK: ") {
            Some(rest) => out.push_str(&format!("# CLOCK: {}\n", f(rest))),
            None => {
                out.push_str(l);
                out.push('\n');
            }
        }
    }
    out
}

/// A row's pulse spacing (its archived output's even gaps), None when it has none.
pub fn row_gap(r: &crate::catalog::Row) -> Option<u32> {
    crate::catalog::parse_gaps(&r.gaps).filter(|g| !g.is_empty() && !crate::catalog::uneven(g)).map(|g| g[0])
}

/// [`retap`] over archived clocks (`dir`: the archive, `files` in it), `threads` at a time; only
/// the shown ones move. A clock not buildable in survival (`crate::gaps::design_hide`) or that does
/// not load is skipped.
pub fn retap_all(dir: &std::path::Path, files: &[String], threads: usize, rule: Rule) -> anyhow::Result<Vec<(String, Outcome)>> {
    let rows = crate::catalog::read_rows(dir)?;
    let info: HashMap<&str, (Option<u32>, bool)> = rows.iter().map(|r| (r.file.as_str(), (row_gap(r), r.shown()))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::with_capacity(files.len()));
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(f) = files.get(i) else { break };
                    let (gap, shown) = info.get(f.as_str()).copied().unwrap_or((None, false));
                    let o = match Clock::load(&dir.join(f)) {
                        Ok(c) => match crate::gaps::design_hide(&c) {
                            Some(why) => Outcome::Skip(why),
                            None => retap(&c, rule, gap, shown),
                        },
                        Err(e) => Outcome::Skip(format!("does not load: {e:#}")),
                    };
                    out.lock().unwrap().push((i, o));
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|(i, _)| *i);
    Ok(v.into_iter().map(|(i, o)| (files[i].clone(), o)).collect())
}

/// [`retap`] over hidden `no output` clocks (clockinfra, the wireable-clocks job: "re-tap existing
/// archived clocks first"): each one's best usable tap, pulsing every `gaps[period]` ticks (the
/// period's record's spacing: a hidden clock has no published time of its own; a period missing
/// from `gaps`: once per period). Other rows given are skipped.
pub fn retap_hidden(dir: &std::path::Path, files: &[String], threads: usize, rule: Rule, gaps: &HashMap<u32, u32>) -> anyhow::Result<Vec<(String, Outcome)>> {
    let rows = crate::catalog::read_rows(dir)?;
    let movable: HashMap<&str, u32> = rows.iter().filter(|r| r.hidden.starts_with("no output")).map(|r| (r.file.as_str(), gaps.get(&r.period).copied().unwrap_or(r.period))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(Vec::with_capacity(files.len()));
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(f) = files.get(i) else { break };
                    let o = match movable.get(f.as_str()) {
                        None => Outcome::Skip("not hidden for `no output`".into()),
                        Some(&gap) => match Clock::load(&dir.join(f)) {
                            Ok(c) => match crate::gaps::design_hide(&c) {
                                Some(why) => Outcome::Skip(why),
                                None => retap(&c, rule, Some(gap), true),
                            },
                            Err(e) => Outcome::Skip(format!("does not load: {e:#}")),
                        },
                    };
                    out.lock().unwrap().push((i, o));
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|(i, _)| *i);
    Ok(v.into_iter().map(|(i, o)| (files[i].clone(), o)).collect())
}

/// The design a tap gives a clock (what the archive keeps): a joined tap's block in it, else the
/// port moved to the tap cell.
pub fn tapped_design(c: &Clock, tap: Tap) -> anyhow::Result<Clock> {
    if tap.kind.joins() { joined(c, tap) } else { with_output(c, tap.out) }
}

/// Counts of an [`apply`], and the moves it could not write (why).
#[derive(Default, Debug)]
pub struct Applied {
    pub unchanged: usize,
    pub moved: usize,
    pub no_usable: usize,
    pub skipped: usize,
    pub failed: Vec<(String, String)>,
}

/// Write [`retap_all`]'s outcomes to the archive: a moved clock's design (its port, a joined tap's
/// block, a comment, its `# CLOCK:` line) and row (hash, output numbers, blocks, box, type,
/// summary) from a redsim run of the new design; every judged row's `usable` column (`yes: <tap>`,
/// or `no: <why>`; a row already `yes: <tap>` keeps its tap).
pub fn apply(dir: &std::path::Path, results: &[(String, Outcome)]) -> anyhow::Result<Applied> {
    let mut rows = crate::catalog::read_rows(dir)?;
    let by: HashMap<&str, &Outcome> = results.iter().map(|(f, o)| (f.as_str(), o)).collect();
    let mut a = Applied::default();
    for r in rows.iter_mut() {
        let Some(o) = by.get(r.file.as_str()) else { continue };
        match o {
            Outcome::Unchanged(_) => {
                if !r.usable.starts_with("yes: ") {
                    r.usable = "yes: repeater".into();
                }
                a.unchanged += 1;
            }
            Outcome::Moved { from, was, tap, stand } => {
                let path = dir.join(&r.file);
                let text = std::fs::read_to_string(&path)?;
                let comment = moved_comment(*tap, stand, *from, was);
                let new = if tap.kind.joins() { joined_text(&text, &Clock::from_design(mctas::design::Design::parse(&text)?)?, *tap, &comment)? } else { moved_text(&text, *tap, &comment)? };
                let c = Clock::from_design(mctas::design::Design::parse(&new)?)?;
                let rep = simulate(&c, &SimOptions::default());
                let Some(w) = rep.output.as_ref().map(|o| o.wave.clone()).filter(|_| rep.verdict == Verdict::Clock && rep.period == r.period) else {
                    a.failed.push((r.file.clone(), format!("the new design on redsim: {}", rep.summary())));
                    continue;
                };
                let [dx, dy, dz] = rep.dims();
                let dims = format!("{dx}x{dy}x{dz}");
                let blocks = c.blocks();
                let new = with_judge_line(&new, |l| patch_line(l, w.on, w.edges, rep.warmup, blocks, &dims));
                std::fs::write(&path, &new)?;
                let c = Clock::from_design(mctas::design::Design::parse(&new)?)?;
                r.hash = crate::catalog::content_hash(&c);
                r.warmup = rep.warmup;
                r.on = w.on;
                r.edges = w.edges;
                r.pulses = format!("{:?}", w.widths);
                r.gaps = format!("{:?}", w.gaps);
                r.blocks = blocks;
                r.volume = (dx * dy * dz) as i64;
                r.summary = patch_line(&r.summary, w.on, w.edges, rep.warmup, blocks, &dims);
                r.dims = dims;
                if tap.kind.joins() {
                    r.type_key = crate::ident::type_key(&c);
                    r.chiral = crate::canon::chiral(c.design.blocks.iter());
                }
                r.usable = format!("yes: {}", tap.kind.word());
                a.moved += 1;
            }
            Outcome::NoUsable(why) => {
                r.usable = format!("no: {}", why.replace(['\t', '\n'], " "));
                a.no_usable += 1;
            }
            Outcome::Skip(_) => a.skipped += 1,
        }
    }
    crate::gaps::write_rows(dir, &rows)?;
    Ok(a)
}

/// The game's word on one archived clock's output (`clock retap --usable --game`): the clock as
/// archived (the judge's lamp at its port) and the tap run (a repeater at the port), both judged in
/// the game.
pub struct GameWord {
    pub lamp: crate::verify::VerifyReport,
    /// Ok(the tap run's report) when the repeater carries the clock's pulses, else why not (the
    /// geometry or the run).
    pub tap: Result<crate::verify::VerifyReport, String>,
}

impl GameWord {
    /// The `usable` column's word (`kind`: the tap the row names).
    pub fn usable(&self, kind: &str) -> String {
        match &self.tap {
            Ok(_) => format!("yes: {kind}"),
            Err(e) => format!("no: {}", e.replace(['\t', '\n'], " ")),
        }
    }
}

/// [`GameWord`] for one clock (`pool`, `o`: the game judge; `gap`: its pulse spacing).
pub fn game_word(pool: &mctas::sim::Pool, o: &crate::verify::VerifyOptions, c: &Clock, gap: Option<u32>) -> anyhow::Result<GameWord> {
    let lamp = crate::verify::verify_in_pool(pool, c, o)?;
    let want = lamp.base().filter(|_| lamp.verdict == Verdict::Clock).map(|b| b.period);
    let tap = match (judge(c, Rule::default(), gap), want) {
        (Err(e), _) => Err(format!("{e} (redsim)")),
        (_, None) => Err(format!("the clock as archived: {}", lamp.summary())),
        (Ok((t, s)), Some(period)) => {
            let tc = with_tap(c, t, &s)?;
            let rep = crate::verify::verify_in_pool(pool, &tc, o)?;
            let b = rep.base();
            match tap_problem_parts(t.kind, rep.verdict, &rep.robustness, b.map(|b| b.period).unwrap_or(0), b.and_then(|b| b.output.as_ref().map(|o| &o.wave)), period, gap) {
                Ok(()) => Ok(rep),
                Err(e) => Err(format!("{e} (the game)")),
            }
        }
    };
    Ok(GameWord { lamp, tap })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock(text: &str) -> Clock {
        Clock::from_design(mctas::design::Design::parse(text).unwrap()).unwrap()
    }

    /// The observer pair: its output in front of an observer's back at the ground level, a
    /// repeater there carries every pulse.
    #[test]
    fn an_observer_pairs_output_is_usable() {
        let c = clock("name: t\norigin: 0 0 0\nport output: -1 0 0\nport start: 1 0 0 last\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  B minecraft:observer[facing=west,powered=false]\nlayer 0:\n  AB\n");
        let (t, s) = judge(&c, Rule::default(), Some(6)).unwrap();
        assert_eq!(s.ground, 0);
        assert_eq!((t.out, t.dir, t.kind), ([-1, 0, 0], [-1, 0, 0], TapKind::Repeater));
        assert_eq!(tap_repeater([-1, 0, 0]), "minecraft:repeater[delay=1,facing=east,locked=false,powered=false]");
        // a tap that would change the clock's time is no tap
        assert!(judge(&c, Rule::default(), Some(3)).err().unwrap().contains("the clock every 3 gt"));
    }

    /// clk-rt-ring3x3-p10-r-12b (vjeux: "unusable for a redstone circuit"): its corner cell is lit
    /// only by the stone above it; a repeater there reads nothing; no ground-level cell feeds one.
    /// The rule moves it to a repeater beside a powered corner stone, a block under it.
    #[test]
    fn the_ring3x3s_corner_is_not_usable() {
        let c = clock("name: t\nport output: 0 0 0\norigin: 0 0 0\npalette:\n  A minecraft:repeater[delay=1,facing=west,locked=false,powered=true]\n  B minecraft:repeater[delay=2,facing=south,locked=false,powered=false]\n  R minecraft:repeater[delay=1,facing=east,locked=false,powered=false]\n  S minecraft:stone\n  r minecraft:repeater[delay=1,facing=north,locked=false,powered=true]\nlayer 0:\n  .S\n  S.S\n  .S\nlayer 1:\n  SAS\n  B.r\n  SRS\n");
        let e = judge(&c, Rule::strict(), Some(10)).err().unwrap();
        assert!(e.contains("repeater"), "{e}");
        assert!(pick(&c, Rule::strict(), Some(10)).is_err());
        // the rule: the middle of the west face, an observer watching the delay-2 repeater (its
        // pulse is one 4-tick pulse a cycle at the port, the clock's spacing)
        let (u, _) = pick(&c, Rule::default(), Some(10)).unwrap();
        assert_eq!((u.tap.out, u.tap.kind, u.tap.port()), ([-1, 1, 1], TapKind::Observer, [-2, 1, 1]), "{:?}", u.tap);
        assert_eq!(u.tapped.output.as_ref().unwrap().wave.gaps, vec![10]);
    }

    /// A comparator reads a hopper clock's hopper: the tap joins the design with a stone under it,
    /// the port in front of it.
    #[test]
    fn a_joined_comparator_tap_is_part_of_the_design() {
        let c = clock("name: t\nport output: 5 0 0\norigin: 0 0 0\npalette:\n  S minecraft:stone\nlayer 0:\n  S\n");
        let tap = Tap { out: [1, 0, 0], dir: [1, 0, 0], kind: TapKind::Comparator };
        let j = joined(&c, tap).unwrap();
        assert_eq!(j.design.blocks.get(&[1, 0, 0]).map(String::as_str), Some("minecraft:comparator[facing=west,mode=compare,powered=false]"));
        assert_eq!(j.design.blocks.get(&[1, -1, 0]).map(String::as_str), Some("minecraft:stone"));
        assert_eq!(j.output, Some([2, 0, 0]));
        assert_eq!(TapKind::Observer.block([0, 0, 1]), "minecraft:observer[facing=north,powered=false]");
        assert_eq!(patch_line("CLOCK: t period 10 gt (0.5 s), on 4/10, edges 1, warmup 4, family repeater, 12 blocks, 3x2x3, robust", 5, 1, 3, 14, "4x3x3"), "CLOCK: t period 10 gt (0.5 s), on 5/10, edges 1, warmup 3, family repeater, 14 blocks, 4x3x3, robust");
    }
}
