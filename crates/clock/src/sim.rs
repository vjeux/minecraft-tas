//! `mctas clock sim`: the exact redsim period detector. Place the clock like the judge, kick it,
//! run, and find the first EXACT repeat of the whole state ([`redsim::World::state_key`]: every
//! cell, pending ticks, block events, moving blocks, block entities, torch memory): the warmup W is
//! the tick of the first occurrence and the period P the distance to the repeat. Then one period is
//! replayed to measure the waveform and the family ([`crate::wave`]).

use crate::wave::{self, CellWave, PeriodAnalysis};
use crate::{Action, Clock, Kick, Placement, Pos};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;
use std::hash::Hasher;

/// Knobs of [`simulate`].
#[derive(Clone, Debug)]
pub struct SimOptions {
    /// Ticks to run at most while looking for a repeat.
    pub horizon: u32,
    /// Free cells around the design's box (the simulated world; a write beyond it = escapes).
    pub margin: i32,
}

impl Default for SimOptions {
    fn default() -> Self {
        SimOptions { horizon: 50_000, margin: 4 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Periodic with at least one block changing in every period.
    Clock,
    /// The state stops changing (or only hidden state changes).
    Stops,
    /// No repeat within the horizon.
    NoRepeat,
    /// A block or a case redsim does not model exactly.
    Unmodeled,
    /// Not a clock by the rules: a drop (item entity), a write beyond the margin, a bad kick.
    Invalid,
    /// Independent sub-clocks side by side: the output's own period is shorter than the whole
    /// structure's (coordinator's ruling: rejected).
    Composite,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Clock => "CLOCK",
            Verdict::Stops => "STOPS",
            Verdict::NoRepeat => "NO-REPEAT",
            Verdict::Unmodeled => "UNMODELED",
            Verdict::Invalid => "INVALID",
            Verdict::Composite => "COMPOSITE",
        }
    }
}

/// The output cell's waveform.
#[derive(Clone, Debug)]
pub struct Output {
    pub pos: Pos,
    /// Not declared (`port output`): the changing cell with the most rising edges.
    pub auto: bool,
    pub wave: CellWave,
}

/// Result of [`simulate`].
#[derive(Clone, Debug)]
pub struct SimReport {
    pub name: String,
    pub verdict: Verdict,
    /// Why it is not a clock (empty for a clock).
    pub why: String,
    /// The first tick from which the VISIBLE state repeats (block states + comparator output
    /// strengths: what the real-game judge sees).
    pub warmup: u32,
    /// The first tick from which the WHOLE state repeats exactly (hidden state included: pending
    /// ticks, torch memory, container contents): >= `warmup`.
    pub state_warmup: u32,
    /// The clock's period: the visible state repeats every `period` gt (a divisor of
    /// `state_period`).
    pub period: u32,
    /// The exact period of the whole state (larger than `period` only when hidden state cycles
    /// slower than everything visible, e.g. items sloshing in hoppers no comparator reads).
    pub state_period: u32,
    pub analysis: PeriodAnalysis,
    pub output: Option<Output>,
    pub blocks: usize,
    /// Every cell non-air at any tick of the run (warmup and one period) and the design's box.
    pub swept: (Pos, Pos),
    pub start: String,
    /// A clock holding a sculk sensor: whether ambient game events leave its output alone
    /// ([`crate::vibration::sim_property`]; a property, never a failure).
    pub vibration: Option<crate::vibration::Property>,
}

impl SimReport {
    fn fail(c: &Clock, verdict: Verdict, why: String) -> SimReport {
        SimReport {
            name: c.name().to_string(),
            verdict,
            why,
            warmup: 0,
            state_warmup: 0,
            period: 0,
            state_period: 0,
            analysis: PeriodAnalysis::default(),
            output: None,
            blocks: c.blocks(),
            swept: c.bounds(),
            start: c.start_label(),
            vibration: None,
        }
    }

    /// Width x height x depth of the swept box.
    pub fn dims(&self) -> [i32; 3] {
        let (lo, hi) = self.swept;
        [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]
    }

    pub fn volume(&self) -> i32 {
        let d = self.dims();
        d[0] * d[1] * d[2]
    }

    /// The one-line summary (the judge's format, docs/clocks.md).
    pub fn summary(&self) -> String {
        let [w, h, d] = self.dims();
        match self.verdict {
            Verdict::Clock => {
                let (on, edges) = self.output.as_ref().map(|o| (o.wave.on, o.wave.edges)).unwrap_or((0, 0));
                format!(
                    "CLOCK: {} period {}, on {on}/{}, edges {edges}, warmup {}, family {}, {} blocks, {w}x{h}x{d}, {}",
                    self.name,
                    crate::gt_time(self.period),
                    self.period,
                    self.warmup,
                    wave::signature(&self.analysis.family),
                    self.blocks,
                    self.start
                )
            }
            v => format!("{}: {} {}", v.word(), self.name, self.why),
        }
    }
}

/// A fast 128-bit digest of a state key (two independent multiplicative lanes); every repeat it
/// finds is confirmed on the exact key.
struct H128 {
    a: u64,
    b: u64,
}

impl Hasher for H128 {
    fn finish(&self) -> u64 {
        self.a
    }
    fn write(&mut self, bytes: &[u8]) {
        for &x in bytes {
            self.a = (self.a ^ x as u64).wrapping_mul(0x100000001b3);
            self.b = (self.b.rotate_left(7) ^ x as u64).wrapping_mul(0x9e3779b97f4a7c15);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.a = (self.a ^ x).wrapping_mul(0x100000001b3).rotate_left(29);
        self.b = (self.b ^ x.rotate_left(17)).wrapping_mul(0x9e3779b97f4a7c15).rotate_left(31);
    }
    fn write_u8(&mut self, x: u8) {
        self.write_u64(x as u64 | 0x100);
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64 | 0x2_0000_0000);
    }
    fn write_i32(&mut self, x: i32) {
        self.write_u32(x as u32)
    }
    fn write_i64(&mut self, x: i64) {
        self.write_u64(x as u64 ^ 0x5555_0000_0000_0000)
    }
    fn write_isize(&mut self, x: isize) {
        self.write_u64(x as u64 ^ 0x3333_0000_0000_0000)
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64 ^ 0x7777_0000_0000_0000)
    }
}

fn digest(w: &World) -> u128 {
    let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
    w.state_key_into(&mut h);
    ((h.a as u128) << 64) | h.b as u128
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A placed world ready to run, and the kick's inputs still to apply (tick, action), in order.
struct Started {
    world: World,
    actions: Vec<(u32, Action)>,
}

/// Every block of the clock as a redsim state, or the names redsim does not model.
pub fn parse_blocks(c: &Clock) -> Result<Vec<(BlockPos, Block)>, String> {
    let mut out = Vec::new();
    let mut bad: Vec<String> = Vec::new();
    for (p, s) in &c.design.blocks {
        match Block::parse(s) {
            Ok(b) => out.push((bp(*p), b)),
            Err(e) => {
                if !bad.contains(&e) {
                    bad.push(e)
                }
            }
        }
    }
    if let Some(Kick::Place { state, .. }) = &c.kick
        && let Err(e) = Block::parse(state)
    {
        bad.push(e);
    }
    if bad.is_empty() { Ok(out) } else { Err(bad.join("; ")) }
}

fn start(c: &Clock, blocks: &[(BlockPos, Block)], margin: i32) -> Result<Started, (Verdict, String)> {
    let (lo, hi) = c.bounds();
    start_box(c, blocks, [lo[0] - margin, lo[1] - margin, lo[2] - margin], [hi[0] + margin, hi[1] + margin, hi[2] + margin])
}

/// [`start`] in a given world box (`from`..=`to`).
fn start_box(c: &Clock, blocks: &[(BlockPos, Block)], from: Pos, to: Pos) -> Result<Started, (Verdict, String)> {
    let min = BlockPos::new(from[0], from[1], from[2]);
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
    let mut w = World::new(min, size);
    let last = c.withheld().map(bp);
    let be_lines = |w: &mut World, only: Option<BlockPos>| -> Result<(), (Verdict, String)> {
        let lines: Vec<(BlockPos, &str)> = c.design.block_entities.iter().map(|(p, s)| (bp(*p), s.as_str())).filter(|(p, _)| if only.is_some() { Some(*p) == only } else { Some(*p) != last }).collect();
        redsim::redstone::container::apply_block_entities(w, lines).map_err(|e| (Verdict::Unmodeled, format!("nbt: {e}")))
    };
    match c.placement {
        Placement::Judge => {
            let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
            w.place_raw(&silent);
            be_lines(&mut w, None)?;
            w.place_updates(&silent);
        }
        Placement::Player => {
            for (p, b) in blocks.iter().filter(|(p, _)| Some(*p) != last) {
                w.set_block(*p, *b, flags::ALL);
            }
            be_lines(&mut w, None)?;
        }
    }
    // the design's entities, once the blocks stand (as the real-game judge summons them)
    for e in &c.design.entities {
        redsim::entity::spawn(&mut w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| (Verdict::Unmodeled, format!("not modeled by redsim: {why}")))?;
    }
    Ok(Started { world: w, actions: c.actions() })
}

/// Apply the kick's inputs due at tick `t` (between ticks); check the world is still trustworthy.
fn between_ticks(s: &mut Started, t: u32) -> Result<(), (Verdict, String)> {
    while s.actions.first().is_some_and(|(at, _)| *at == t) {
        let (_, a) = s.actions.remove(0);
        match a {
            Action::Place { pos, state, nbt } => {
                let b = Block::parse(&state).map_err(|e| (Verdict::Unmodeled, e))?;
                s.world.set_block(bp(pos), b, flags::ALL);
                if let Some(n) = nbt {
                    redsim::redstone::container::apply_block_entities(&mut s.world, [(bp(pos), n.as_str())]).map_err(|e| (Verdict::Unmodeled, format!("nbt: {e}")))?;
                }
            }
            Action::Use { pos } => {
                if !s.world.use_block(bp(pos)) {
                    return Err((Verdict::Unmodeled, format!("a right-click on {} is not modeled", s.world.get(bp(pos)))));
                }
            }
            Action::Clear { pos } => {
                s.world.set_block(bp(pos), Block::Air, flags::ALL);
            }
            Action::ShapeSet { pos, state } => {
                let b = Block::parse(&state).map_err(|e| (Verdict::Unmodeled, e))?;
                s.world.set_block(bp(pos), b, flags::CLIENTS);
            }
            Action::Update { pos } => {
                let src = s.world.get(bp(pos)).kind();
                s.world.update_neighbors_at(bp(pos), src);
            }
            Action::GameEvent { pos, event } => {
                s.world.game_event(event, bp(pos));
            }
        }
    }
    if let Some(e) = s.world.errors.first() {
        if e.contains("outside the box") {
            return Err((Verdict::Invalid, format!("escapes its box by more than the margin by tick {t}: {e}")));
        }
        return Err((Verdict::Unmodeled, format!("tick {t}: {e}")));
    }
    if let Some((p, b)) = s.world.drops.first() {
        return Err((Verdict::Invalid, format!("tick {t}: {b} at {p} was destroyed with a drop (an item entity)")));
    }
    Ok(())
}

/// Is this state a component whose own state says whether it is on (else the output reads the
/// power the cell receives)?
fn self_reporting(s: &str) -> bool {
    wave::family_name(s).is_some()
}

/// The first `ticks` ticks after placement, one line per tick listing every cell that changed
/// (`mctas clock sim --trace N`): how a clock actually runs.
pub fn trace(c: &Clock, o: &SimOptions, ticks: u32) -> Vec<String> {
    let blocks = match parse_blocks(c) {
        Ok(b) => b,
        Err(e) => return vec![format!("not modeled by redsim: {e}")],
    };
    let mut s = match start(c, &blocks, o.margin) {
        Ok(s) => s,
        Err((_, why)) => return vec![why],
    };
    let mut out = Vec::new();
    let mut prev: HashMap<BlockPos, Block> = HashMap::new();
    for t in 0..=ticks {
        if let Err((_, why)) = between_ticks(&mut s, t) {
            out.push(format!("t={t}: {why}"));
            break;
        }
        let now: HashMap<BlockPos, Block> = s.world.blocks().into_iter().collect();
        let mut changes: Vec<String> = Vec::new();
        let mut keys: Vec<BlockPos> = now.keys().chain(prev.keys()).copied().collect();
        keys.sort();
        keys.dedup();
        for p in keys {
            let a = prev.get(&p).copied().unwrap_or(Block::Air);
            let b = now.get(&p).copied().unwrap_or(Block::Air);
            if a != b {
                changes.push(format!("{},{},{} {}", p.x, p.y, p.z, crate::short(&b.to_string())));
            }
        }
        let pending = s.world.ticks.count();
        out.push(format!("t={t} ({} pending ticks, {} moving): {}", pending, s.world.piston.live_count(), if changes.is_empty() { "-".into() } else { changes.join("; ") }));
        prev = now;
        s.world.tick();
    }
    out
}

/// A redsim world over `from..=to` with the design placed as the real-game judge places it (its
/// block entities and entities too), before any tick: for callers that drive the run themselves
/// (the shulker judge's replay, `mctas circuit shulker compare`).
pub fn world_for(c: &Clock, from: Pos, to: Pos) -> Result<World, String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    start_box(c, &blocks, from, to).map(|s| s.world).map_err(|(_, why)| why)
}

/// The design's blocks that cannot stand where they are once placed like the judge places them:
/// the ones the placement removed (a floating dust loses its shape update) and the ones redsim's
/// `canSurvive` rejects (a floating lever, torch or repeater, which the raw placement keeps but a
/// player could not build), in the box `from`..=`to`. `Err` when redsim does not model the design.
pub fn unsupported(c: &Clock, from: Pos, to: Pos) -> Result<Vec<(Pos, String)>, String> {
    let mut blocks = parse_blocks(c).map_err(|e| format!("not modeled by redsim: {e}"))?;
    blocks.sort_by_key(|(p, _)| *p);
    let s = start_box(c, &blocks, from, to).map_err(|(_, why)| why)?;
    let withheld = c.withheld();
    let mut out = Vec::new();
    for (p, s0) in &c.design.blocks {
        if Some(*p) == withheld {
            continue;
        }
        let b = s.world.get(bp(*p));
        if matches!(b, Block::Air) {
            out.push((*p, s0.clone()));
        } else if !redsim::behavior::can_survive(&s.world, &b, bp(*p)) {
            out.push((*p, b.to_string()));
        }
    }
    Ok(out)
}

/// One driven run on redsim (the circuit judge's screen, `mctas circuit sim`): what [`drive`]
/// recorded after every tick 0..=ticks.
pub struct Driven {
    /// Every non-air cell's full state (block state + comparator strength + container contents),
    /// sorted by position.
    pub cells: Vec<Vec<(Pos, String)>>,
    /// The best signal the neighbors give into each probe cell (what a lamp there would see).
    pub signals: Vec<Vec<u8>>,
    /// The run stopped early: the tick and why (a drop, a write outside the box, a case redsim does
    /// not model); `cells` and `signals` hold the frames before it.
    pub stopped: Option<(u32, String)>,
    /// TNT primed (track tnt): (the frame it first shows in, its cell).
    pub primed: Vec<(u32, Pos)>,
}

/// Run a design on redsim in the world box `from`..=`to`: placed like the judge places it, its own
/// kick inputs and `extra` inputs applied between ticks (inputs due at tick t are applied after t
/// ticks, before frame t is recorded, like the real-game judge), recording [`Driven`] after every
/// tick 0..=ticks. Fails on a block redsim does not model; a problem during the run (a drop, a write
/// outside the box) stops it ([`Driven::stopped`]).
pub fn drive(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos) -> Result<Driven, (Verdict, String)> {
    drive_with(c, extra, ticks, probes, from, to, false)
}

/// [`drive`]; `remove_primed`: the caller removes every primed TNT before it explodes (the TNT
/// judge), so an explosion is no model note.
pub fn drive_with(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos, remove_primed: bool) -> Result<Driven, (Verdict, String)> {
    let mut blocks = parse_blocks(c).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
    for (_, a) in extra {
        if let Action::Place { state, .. } | Action::ShapeSet { state, .. } = a {
            Block::parse(state).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
        }
    }
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to)?;
    s.world.remove_primed = remove_primed;
    s.actions.extend(extra.iter().cloned());
    s.actions.sort_by_key(|(t, _)| *t);
    let mut out = Driven { cells: Vec::with_capacity(ticks as usize + 1), signals: Vec::with_capacity(ticks as usize + 1), stopped: None, primed: Vec::new() };
    let mut primed_seen = 0usize;
    for t in 0..=ticks {
        if let Err((_, why)) = between_ticks(&mut s, t) {
            out.stopped = Some((t, why));
            break;
        }
        for &(_, p) in &s.world.primed[primed_seen..] {
            out.primed.push((t, p.arr()));
        }
        primed_seen = s.world.primed.len();
        let mut v: Vec<(Pos, String)> = s.world.blocks().into_iter().map(|(p, b)| (p.arr(), wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, p, &b).as_deref()))).collect();
        v.sort_by_key(|(p, _)| *p);
        out.cells.push(v);
        out.signals.push(probes.iter().map(|q| s.world.best_neighbor_signal(bp(*q)).clamp(0, 15) as u8).collect());
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(out)
}

/// TNT dupers with explosions on (track tnt, all-randoms judge): how [`drive_tnt`] runs them.
#[derive(Clone, Debug)]
pub struct TntOptions {
    /// The kick of every primed TNT (`World::tnt_kick`: the angle's `nextDouble()`), unless
    /// `kicks` gives the k-th spawn's own.
    pub kick: f64,
    pub kicks: Vec<f64>,
    /// Every explosion ray at its largest roll (`explosion::Rolls::Max`): surviving it is surviving
    /// every roll.
    pub max_rolls: bool,
    /// A burning TNT this far from `design_box` is removed as out of range (the real-game judge's
    /// rule, `circuit::run::OUT_OF_RANGE`).
    pub out_of_range: f64,
    pub design_box: (Pos, Pos),
}

/// One primed TNT followed by [`drive_tnt`] (the fields the real-game runner records).
#[derive(Clone, Debug, Default)]
pub struct TntRecord {
    pub frame: u32,
    pub first_pos: [f64; 3],
    pub first_motion: [f64; 3],
    pub launch: Option<[f64; 3]>,
    pub end: Option<([f64; 3], [f64; 3])>,
    pub end_frame: u32,
    pub moved: u32,
    pub last_pos: [f64; 3],
    pub gone: Option<([f64; 3], [f64; 3])>,
}

/// The result of [`drive_tnt`].
pub struct TntDriven {
    pub driven: Driven,
    pub tnt: Vec<TntRecord>,
    /// The frame of every explosion.
    pub explosions: Vec<u32>,
    pub out_of_range: u32,
}

/// [`drive_with`] for TNT dupers with their explosions (redsim's exact port): every primed TNT is
/// an entity with the given kick, explodes at fuse 0 (rays at their largest roll with
/// `max_rolls`), and is removed out of range like the real-game judge does. A block of the design
/// destroyed by an explosion stops the run with `blown up: ...`.
pub fn drive_tnt(c: &Clock, extra: &[(u32, Action)], ticks: u32, probes: &[Pos], from: Pos, to: Pos, o: &TntOptions) -> Result<TntDriven, (Verdict, String)> {
    let mut blocks = parse_blocks(c).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
    for (_, a) in extra {
        if let Action::Place { state, .. } | Action::ShapeSet { state, .. } = a {
            Block::parse(state).map_err(|e| (Verdict::Unmodeled, format!("not modeled by redsim: {e}")))?;
        }
    }
    blocks.sort_by_key(|(p, _)| *p);
    let mut s = start_box(c, &blocks, from, to)?;
    s.world.remove_primed = false;
    s.world.tnt_kick = Some(o.kick);
    s.world.tnt_kicks = o.kicks.clone();
    if o.max_rolls {
        s.world.explosion_rolls = redsim::explosion::Rolls::Max;
    }
    s.actions.extend(extra.iter().cloned());
    s.actions.sort_by_key(|(t, _)| *t);
    let mut out = TntDriven { driven: Driven { cells: Vec::with_capacity(ticks as usize + 1), signals: Vec::with_capacity(ticks as usize + 1), stopped: None, primed: Vec::new() }, tnt: Vec::new(), explosions: Vec::new(), out_of_range: 0 };
    let mut index: std::collections::HashMap<u32, usize> = std::collections::HashMap::new();
    let mut blasts_seen = 0usize;
    let (mut primed_seen, mut chain_seen) = (0usize, 0usize);
    let (lo, hi) = o.design_box;
    for t in 0..=ticks {
        // explosions of the last tick: a design block they destroyed blows the machine up
        for b in &s.world.explosions[blasts_seen..] {
            out.explosions.push(t);
            if let Some((p, blk)) = b.destroyed.first() {
                out.driven.stopped = Some((t, format!("blown up: {blk} at {} destroyed by an explosion at frame {t}", p)));
            }
        }
        blasts_seen = s.world.explosions.len();
        if out.driven.stopped.is_some() {
            break;
        }
        if let Err((_, why)) = between_ticks(&mut s, t) {
            out.driven.stopped = Some((t, why));
            break;
        }
        for e in s.world.entities.list.iter_mut() {
            let redsim::entity::EntityKind::PrimedTnt(st) = e.kind else { continue };
            let pos = [e.pos.x, e.pos.y, e.pos.z];
            let mot = [e.delta_movement.x, e.delta_movement.y, e.delta_movement.z];
            let k = *index.entry(e.id).or_insert_with(|| {
                out.tnt.push(TntRecord { frame: t, first_pos: pos, first_motion: mot, moved: t, last_pos: pos, ..Default::default() });
                out.tnt.len() - 1
            });
            let r = &mut out.tnt[k];
            let h = |m: &[f64; 3]| (m[0] * m[0] + m[2] * m[2]).sqrt();
            if h(&mot) >= 0.4 && r.launch.is_none_or(|l| h(&mot) > h(&l)) {
                r.launch = Some(mot);
            }
            if pos.iter().zip(&r.last_pos).any(|(a, b)| a.to_bits() != b.to_bits()) {
                r.moved = t;
                r.last_pos = pos;
            }
            if st.fuse <= 1 && r.end.is_none() {
                r.end = Some((pos, mot));
                r.end_frame = t;
            }
            let dist = (0..3).map(|i| (lo[i] as f64 - pos[i]).max(pos[i] - (hi[i] as f64 + 1.0)).max(0.0)).fold(0.0f64, f64::max);
            if dist > o.out_of_range && !e.removed {
                e.removed = true;
                r.gone = Some((pos, mot));
                out.out_of_range += 1;
            }
        }
        s.world.entities.list.retain(|e| !e.removed);
        for &(_, p) in &s.world.primed[primed_seen..] {
            out.driven.primed.push((t, p.arr()));
        }
        primed_seen = s.world.primed.len();
        // an explosion-primed TNT is a new TNT too (the real-game runner sees every new entity)
        for &(_, p) in &s.world.chain_primed[chain_seen..] {
            out.driven.primed.push((t, p.arr()));
        }
        chain_seen = s.world.chain_primed.len();
        let mut v: Vec<(Pos, String)> = s.world.blocks().into_iter().map(|(p, b)| (p.arr(), wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, p, &b).as_deref()))).collect();
        v.sort_by_key(|(p, _)| *p);
        out.driven.cells.push(v);
        out.driven.signals.push(probes.iter().map(|q| s.world.best_neighbor_signal(bp(*q)).clamp(0, 15) as u8).collect());
        if t < ticks {
            s.world.tick();
        }
    }
    Ok(out)
}

/// Every block of the world (with comparator strengths and container contents) after each tick
/// 0..=ticks, sorted by position: for comparing two runs (`ident`). `None` when redsim does not
/// model the design or the run meets something it does not model.
pub fn run_states(c: &Clock, o: &SimOptions, ticks: u32) -> Option<Vec<Vec<(BlockPos, String)>>> {
    if !c.design.entities.is_empty() {
        return None;
    }
    let blocks = parse_blocks(c).ok()?;
    let mut s = start(c, &blocks, o.margin).ok()?;
    let mut out = Vec::with_capacity(ticks as usize + 1);
    for t in 0..=ticks {
        between_ticks(&mut s, t).ok()?;
        let mut v: Vec<(BlockPos, String)> = s.world.blocks().into_iter().map(|(p, b)| (p, wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, p, &b).as_deref()))).collect();
        v.sort_by_key(|(p, _)| *p);
        out.push(v);
        s.world.tick();
    }
    Some(out)
}

/// The census's fast screen (no waveform, no strings): place `blocks` like the judge (raw writes,
/// wire shapes, self updates), the block at index `last` withheld and placed last with updates
/// when given, then run until the whole state repeats. `Some((state_warmup, state_period))` for a
/// state that keeps changing; `None` when it goes still, never repeats within `horizon`, or the
/// model meets something it does not model exactly (an error, a drop, a write beyond `margin`).
pub fn quick_period(blocks: &[(BlockPos, Block)], last: Option<usize>, margin: i32, horizon: u32) -> Option<(u32, u32)> {
    let mut lo = blocks[0].0;
    let mut hi = blocks[0].0;
    for (p, _) in blocks {
        lo = BlockPos::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z));
        hi = BlockPos::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z));
    }
    let min = BlockPos::new(lo.x - margin, lo.y - margin, lo.z - margin);
    let size = [hi.x - lo.x + 1 + 2 * margin, hi.y - lo.y + 1 + 2 * margin, hi.z - lo.z + 1 + 2 * margin];
    let mut w = World::new(min, size);
    let silent: Vec<(BlockPos, Block)> = blocks.iter().enumerate().filter(|(i, _)| Some(*i) != last).map(|(_, b)| *b).collect();
    w.place_raw(&silent);
    w.place_updates(&silent);
    if let Some(i) = last {
        w.set_block(blocks[i].0, blocks[i].1, flags::ALL);
    }
    let hoppers = blocks.iter().any(|(_, b)| matches!(b, Block::Hopper { .. }));
    let mut seen: HashMap<u128, u32> = HashMap::new();
    for t in 0..=horizon {
        if !w.errors.is_empty() || !w.drops.is_empty() {
            return None;
        }
        if !hoppers && w.idle() && !w.sculk.busy() {
            return None;
        }
        let d = digest(&w);
        if let Some(&t0) = seen.get(&d) {
            return if t - t0 > 1 { Some((t0, t - t0)) } else { None };
        }
        seen.insert(d, t);
        w.tick();
    }
    None
}

/// One period replayed on redsim for a picture: (whole-state warmup W, the clock's period P, for
/// each tick W..W+P the number of changing cells that are ON and the number of moving blocks: a
/// moving block is invisible in a still picture). `None` unless redsim says CLOCK.
pub fn activity(c: &Clock, o: &SimOptions) -> Option<(u32, u32, Vec<(u32, u32)>)> {
    let r = simulate(c, o);
    if r.verdict != Verdict::Clock {
        return None;
    }
    let blocks = parse_blocks(c).ok()?;
    let mut s = start(c, &blocks, o.margin).ok()?;
    let w0 = r.state_warmup.max(r.warmup);
    let cells: Vec<BlockPos> = r.analysis.cells.iter().map(|w| bp(w.pos)).collect();
    let mut counts = Vec::new();
    for t in 0..(w0 + r.period) {
        between_ticks(&mut s, t).ok()?;
        if t >= w0 {
            let on = cells.iter().filter(|p| wave::active(&s.world.get(**p).to_string())).count() as u32;
            let moving = cells.iter().filter(|p| matches!(s.world.get(**p), Block::MovingPiston { .. })).count() as u32;
            counts.push((on, moving));
        }
        s.world.tick();
    }
    Some((w0, r.period, counts))
}

/// The real-game judge's horizon and minimum periods (`verify::VerifyOptions`), used for designs
/// with entities.
pub const ENTITY_HORIZON: u32 = 4000;
const ENTITY_MIN_PERIODS: usize = 3;

/// (warmup, period) of a design with entities by the real-game judge's rule: the block frames
/// (every cell of the world box: block state + block-entity extras; entities left out) over
/// 0..=[`ENTITY_HORIZON`], `wave::detect_period` from the first free tick with 3 periods, and the
/// period must hold over at least the second half of the horizon (warmup <= horizon / 2): an
/// entity that never repeats exactly (a drifting item) can show a short block pattern near the
/// end of any window (kit-bouncer-item-p12-20b: "period 12 from 3965" here, 13 from 4074 in the
/// game; clk-misc, 2026-10-20). `None`: not periodic.
fn entity_clock_period(c: &Clock, blocks: &[(BlockPos, Block)], o: &SimOptions, end: u32) -> Result<Option<(u32, u32)>, (Verdict, String)> {
    let mut s = start(c, blocks, o.margin)?;
    let wmin = s.world.min;
    let size = s.world.size;
    let cells: Vec<BlockPos> = (0..size[1])
        .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| BlockPos::new(wmin.x + x, wmin.y + y, wmin.z + z))))
        .collect();
    let mut ids: HashMap<u128, u64> = HashMap::new();
    let mut frames: Vec<u64> = Vec::with_capacity(ENTITY_HORIZON as usize + 1);
    for t in 0..=ENTITY_HORIZON {
        between_ticks(&mut s, t)?;
        use std::hash::Hash;
        let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
        for q in &cells {
            let b = s.world.get(*q);
            b.hash(&mut h);
            wave::be_extra_redsim(&s.world, *q, &b).hash(&mut h);
        }
        let d = ((h.a as u128) << 64) | h.b as u128;
        let n = ids.len() as u64;
        frames.push(*ids.entry(d).or_insert(n));
        if t < ENTITY_HORIZON {
            s.world.tick();
        }
    }
    Ok(wave::detect_period(&frames, end as usize, ENTITY_MIN_PERIODS).filter(|(w, _)| *w <= ENTITY_HORIZON as usize / 2).map(|(w, p)| (w as u32, p as u32)))
}

/// A quick screen for entity-kit candidates (`clock kit`): run `ticks` gt; false when the design
/// comes to rest (nothing pending and every entity still for 3 ticks in a row), when no cell of the
/// world box changes in the last two thirds of the run (no clock with a period that fits), or on an
/// invalid or unmodeled run.
pub fn entity_screen(c: &Clock, margin: i32, ticks: u32) -> bool {
    let Ok(blocks) = parse_blocks(c) else { return false };
    let Ok(mut s) = start(c, &blocks, margin) else { return false };
    let wmin = s.world.min;
    let size = s.world.size;
    let cells: Vec<BlockPos> = (0..size[1])
        .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| BlockPos::new(wmin.x + x, wmin.y + y, wmin.z + z))))
        .collect();
    let digest = |w: &World| -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for q in &cells {
            w.get(*q).hash(&mut h);
        }
        h.finish()
    };
    let mut prev = digest(&s.world);
    let mut changed_late = false;
    let mut still_run = 0;
    for t in 0..ticks {
        if between_ticks(&mut s, t).is_err() {
            return false;
        }
        s.world.tick();
        let d = digest(&s.world);
        if d != prev {
            if t >= ticks / 3 {
                changed_late = true;
            }
            prev = d;
        }
        let still = s.world.idle()
            && s.world.entities.list.iter().all(|e| {
                let v = e.delta_movement;
                v.x == 0.0 && v.y == 0.0 && v.z == 0.0 && e.pos.x == e.xo && e.pos.y == e.yo && e.pos.z == e.zo
            });
        still_run = if still { still_run + 1 } else { 0 };
        if still_run >= 3 {
            return false;
        }
    }
    changed_late && s.world.errors.is_empty() && s.world.drops.is_empty()
}

/// Simulate a clock on redsim (exact): see the module doc.
pub fn simulate(c: &Clock, o: &SimOptions) -> SimReport {
    let blocks = match parse_blocks(c) {
        Ok(b) => b,
        Err(e) => return SimReport::fail(c, Verdict::Unmodeled, format!("not modeled by redsim: {e}")),
    };
    let end = c.free_from();
    // Designs with entities: their exact state never repeats (an entity's doubles drift in the
    // low bits), so the period comes from the BLOCKS, by the real-game judge's own rule
    // (`verify::run_in`): the frames of every cell (block state + block-entity extras, entities
    // left out) must repeat with period p for 3 periods within its 4000 gt horizon; the smallest
    // warmup, then the smallest p (`wave::detect_period`).
    let entity_period = if c.design.entities.is_empty() {
        None
    } else {
        match entity_clock_period(c, &blocks, o, end) {
            Ok(Some(wp)) => Some(wp),
            Ok(None) => return SimReport::fail(c, Verdict::NoRepeat, format!("not periodic for 3 periods over the second half of {ENTITY_HORIZON} gt (the judge's rule for designs with entities)")),
            Err((v, why)) => return SimReport::fail(c, v, why),
        }
    };
    // Phase 1: the first repeat of the full state.
    let mut s = match start(c, &blocks, o.margin) {
        Ok(s) => s,
        Err((v, why)) => return SimReport::fail(c, v, why),
    };
    let mut seen: HashMap<u128, u32> = HashMap::new();
    let mut found = entity_period;
    for t in 0..=o.horizon {
        if found.is_some() {
            break;
        }
        if let Err((v, why)) = between_ticks(&mut s, t) {
            return SimReport::fail(c, v, why);
        }
        if t >= end {
            let d = digest(&s.world);
            if let Some(&t0) = seen.get(&d) {
                found = Some((t0, t - t0));
                break;
            }
            seen.insert(d, t);
        }
        s.world.tick();
    }
    let Some((w0, p)) = found else {
        return SimReport::fail(c, Verdict::NoRepeat, format!("no repeat of its state within {} gt", o.horizon));
    };
    // Phase 2: replay to W + P, confirm the repeat on the exact key, record one period.
    let mut s = start(c, &blocks, o.margin).ok().unwrap();
    let (lo, hi) = c.bounds();
    let (mut slo, mut shi) = (lo, hi);
    let wmin = s.world.min;
    let size = s.world.size;
    let cells: Vec<Pos> = (0..size[1])
        .flat_map(|y| (0..size[2]).flat_map(move |z| (0..size[0]).map(move |x| [wmin.x + x, wmin.y + y, wmin.z + z])))
        .collect();
    let mut period_states: Vec<Vec<String>> = Vec::with_capacity(p as usize);
    let mut out_seq: Vec<bool> = Vec::with_capacity(p as usize);
    let mut out_level: Vec<i32> = Vec::with_capacity(p as usize);
    let mut key_w = Vec::new();
    let out_pos = c.output;
    // a digest of the full cell states after every tick (block state + comparator strength +
    // container contents): the clock's warmup and period
    let mut frames: Vec<u128> = Vec::with_capacity((w0 + p + 1) as usize);
    for t in 0..=(w0 + p) {
        if let Err((v, why)) = between_ticks(&mut s, t) {
            return SimReport::fail(c, v, why);
        }
        for (q, _) in s.world.blocks() {
            crate::grow(&mut slo, &mut shi, q.arr());
        }
        {
            use std::hash::Hash;
            let mut h = H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 };
            for q in &cells {
                let b = s.world.get(bp(*q));
                b.hash(&mut h);
                wave::be_extra_redsim(&s.world, bp(*q), &b).hash(&mut h);
            }
            frames.push(((h.a as u128) << 64) | h.b as u128);
        }
        if t == w0 {
            key_w = s.world.state_key();
        }
        if t >= w0 && t < w0 + p {
            period_states.push(
                cells
                    .iter()
                    .map(|q| {
                        let b = s.world.get(bp(*q));
                        wave::with_extra(&b.to_string(), wave::be_extra_redsim(&s.world, bp(*q), &b).as_deref())
                    })
                    .collect(),
            );
            if let Some(op) = out_pos {
                let st = s.world.get(bp(op));
                let level = s.world.best_neighbor_signal(bp(op));
                let on = if self_reporting(&st.to_string()) { wave::active(&st.to_string()) } else { level > 0 };
                out_seq.push(on);
                out_level.push(level);
            }
        }
        if t == w0 + p {
            if entity_period.is_none() && s.world.state_key() != key_w {
                return SimReport::fail(c, Verdict::Unmodeled, format!("digest collision: states at {w0} and {} differ (report this)", w0 + p));
            }
            break;
        }
        s.world.tick();
    }
    // The clock's period F: the smallest divisor of the whole-state period over which every cell's
    // full state (block state + comparator strength + container contents) repeats; its warmup: the
    // first tick from which it does (what the real-game judge measures). Pending ticks and the
    // torch memory may cycle slower (`state_period`: their order among ticks due together is not
    // physics).
    let (w0u, pu) = (w0 as usize, p as usize);
    let full_p = (1..=pu).filter(|d| pu % d == 0).find(|&d| (w0u..w0u + pu).all(|t| frames[t] == frames[w0u + (t - w0u + d) % pu])).unwrap_or(pu);
    let mut vis_w = w0u;
    while vis_w > c.free_from() as usize && frames[vis_w - 1] == frames[vis_w - 1 + full_p] {
        vis_w -= 1;
    }
    let refs: Vec<Vec<&str>> = period_states[..full_p].iter().map(|v| v.iter().map(|s| s.as_str()).collect()).collect();
    out_seq.truncate(full_p);
    out_level.truncate(full_p);
    let mut analysis = wave::analyze(&cells, &refs);
    // the design's entities (a minecart riding its rails) are part of the mechanism: their kinds
    // join the family, as in the real-game judge (`detector_rail+minecart`)
    for e in &c.design.entities {
        let k = crate::short(&e.kind).to_string();
        if !analysis.family.contains(&k) {
            analysis.family.push(k);
        }
    }
    analysis.family.sort();
    let mut rep = SimReport {
        name: c.name().to_string(),
        verdict: Verdict::Clock,
        why: String::new(),
        warmup: vis_w as u32,
        state_warmup: w0,
        period: full_p as u32,
        state_period: p,
        analysis,
        output: None,
        blocks: c.blocks(),
        swept: (slo, shi),
        start: c.start_label(),
        vibration: None,
    };
    if rep.analysis.cells.is_empty() {
        rep.verdict = Verdict::Stops;
        rep.why = if p == 1 { format!("goes still at tick {w0}") } else { format!("no block changes in its period (the state repeats every {p} gt from tick {w0})") };
        return rep;
    }
    // The output: `port output`, else the changing cell with the longest own period (then the most
    // edges); its own period must be the clock's (coordinator's ruling 2026-10-19: else the design
    // is independent sub-clocks side by side: COMPOSITE).
    let cell_period = |pos: Pos| -> usize {
        match cells.iter().position(|q| *q == pos) {
            Some(i) => wave::cyclic_period(&refs.iter().map(|v| v[i]).collect::<Vec<_>>()),
            None => full_p,
        }
    };
    let (output, out_period) = match out_pos {
        Some(op) => {
            let (on, edges, widths) = wave::bool_wave(&out_seq);
            let states = rep.analysis.cells.iter().find(|w| w.pos == op).map(|w| w.states.clone()).unwrap_or_default();
            // a component reports its own state; a plain block or air: the power it receives
            let own = if states.is_empty() { wave::cyclic_period(&out_level) } else { cell_period(op) };
            (Some(Output { pos: op, auto: false, wave: CellWave { pos: op, states, kinds: Vec::new(), on, edges, widths } }), own)
        }
        None => {
            let best = rep.analysis.cells.iter().max_by(|a, b| (cell_period(a.pos), a.edges, a.on).cmp(&(cell_period(b.pos), b.edges, b.on)).then(b.pos.cmp(&a.pos)));
            match best {
                Some(w) => (Some(Output { pos: w.pos, auto: true, wave: w.clone() }), cell_period(w.pos)),
                None => (None, full_p),
            }
        }
    };
    rep.output = output;
    // Composite (coordinator's ruling 2026-10-19): the output repeats faster than the whole
    // structure AND the changing cells split into independent groups (sub-clocks side by side).
    // One mechanism whose output fires twice per cycle (an observer watching both strokes of a
    // piston) is a normal clock of the whole cycle. (A declared output that never changes says
    // nothing about the structure: not judged here.)
    let groups = wave::spatial_groups(&rep.analysis.cells.iter().map(|w| w.pos).collect::<Vec<_>>(), &wave::sensors_of(&c.design.blocks));
    if out_period < full_p && out_period > 1 && groups > 1 {
        rep.verdict = Verdict::Composite;
        let pos = rep.output.as_ref().map(|o| o.pos).unwrap_or([0, 0, 0]);
        rep.why = format!(
            "composite: the output {pos:?} repeats every {out_period} gt but the whole structure every {full_p} gt, and its changing cells form {groups} independent groups (sub-clocks side by side: the period would be their lcm); keep one sub-clock, or declare the combining cell as `port output` (a beat clock, marked `# composite`)"
        );
    }
    // a sculk clock: does it run the same with footsteps and blocks around it? (a property, never
    // a failure: crate::vibration)
    if rep.verdict == Verdict::Clock {
        rep.vibration = crate::vibration::sim_property(c, o.margin, rep.warmup, rep.period, rep.swept, rep.output.as_ref().map(|o| o.pos));
    }
    rep
}
