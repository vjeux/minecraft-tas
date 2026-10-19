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
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Clock => "CLOCK",
            Verdict::Stops => "STOPS",
            Verdict::NoRepeat => "NO-REPEAT",
            Verdict::Unmodeled => "UNMODELED",
            Verdict::Invalid => "INVALID",
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
    /// The first tick from which the BLOCK STATES repeat (what the real-game judge sees).
    pub warmup: u32,
    /// The first tick from which the WHOLE state repeats exactly (hidden state included: pending
    /// ticks, torch memory, block entities): >= `warmup`.
    pub state_warmup: u32,
    /// The exact period: the whole state repeats every `period` gt.
    pub period: u32,
    /// The smallest period of the block states alone (a divisor of `period`; smaller only when
    /// hidden state cycles slower than the blocks).
    pub block_period: u32,
    pub analysis: PeriodAnalysis,
    pub output: Option<Output>,
    pub blocks: usize,
    /// Every cell non-air at any tick of the run (warmup and one period) and the design's box.
    pub swept: (Pos, Pos),
    pub start: String,
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
            block_period: 0,
            analysis: PeriodAnalysis::default(),
            output: None,
            blocks: c.blocks(),
            swept: c.bounds(),
            start: c.start_label(),
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
                    "CLOCK: {} period {} gt, on {on}/{}, edges {edges}, warmup {}, family {}, {} blocks, {w}x{h}x{d}, {}",
                    self.name,
                    self.period,
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
    let min = BlockPos::new(lo[0] - margin, lo[1] - margin, lo[2] - margin);
    let size = [hi[0] - lo[0] + 1 + 2 * margin, hi[1] - lo[1] + 1 + 2 * margin, hi[2] - lo[2] + 1 + 2 * margin];
    let mut w = World::new(min, size);
    let last = c.withheld().map(bp);
    match c.placement {
        Placement::Judge => {
            let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(*p) != last).copied().collect();
            w.place_raw(&silent);
            w.place_updates(&silent);
        }
        Placement::Player => {
            for (p, b) in blocks.iter().filter(|(p, _)| Some(*p) != last) {
                w.set_block(*p, *b, flags::ALL);
            }
        }
    }
    Ok(Started { world: w, actions: c.actions() })
}

/// Apply the kick's inputs due at tick `t` (between ticks); check the world is still trustworthy.
fn between_ticks(s: &mut Started, t: u32) -> Result<(), (Verdict, String)> {
    while s.actions.first().is_some_and(|(at, _)| *at == t) {
        let (_, a) = s.actions.remove(0);
        match a {
            Action::Place { pos, state } => {
                let b = Block::parse(&state).map_err(|e| (Verdict::Unmodeled, e))?;
                s.world.set_block(bp(pos), b, flags::ALL);
            }
            Action::Use { pos } => {
                if !s.world.use_block(bp(pos)) {
                    return Err((Verdict::Unmodeled, format!("a right-click on {} is not modeled", s.world.get(bp(pos)))));
                }
            }
            Action::Clear { pos } => {
                s.world.set_block(bp(pos), Block::Air, flags::ALL);
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

/// Simulate a clock on redsim (exact): see the module doc.
pub fn simulate(c: &Clock, o: &SimOptions) -> SimReport {
    let blocks = match parse_blocks(c) {
        Ok(b) => b,
        Err(e) => return SimReport::fail(c, Verdict::Unmodeled, format!("not modeled by redsim: {e}")),
    };
    let end = c.free_from();
    // Phase 1: the first repeat of the full state.
    let mut s = match start(c, &blocks, o.margin) {
        Ok(s) => s,
        Err((v, why)) => return SimReport::fail(c, v, why),
    };
    let mut seen: HashMap<u128, u32> = HashMap::new();
    let mut found = None;
    for t in 0..=o.horizon {
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
    let mut key_w = Vec::new();
    let out_pos = c.output;
    // a digest of the block states after every tick (the visible warmup and period)
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
                s.world.get(bp(*q)).hash(&mut h);
            }
            frames.push(((h.a as u128) << 64) | h.b as u128);
        }
        if t == w0 {
            key_w = s.world.state_key();
        }
        if t >= w0 && t < w0 + p {
            period_states.push(cells.iter().map(|q| s.world.get(bp(*q)).to_string()).collect());
            if let Some(op) = out_pos {
                let st = s.world.get(bp(op));
                let on = if self_reporting(&st.to_string()) { wave::active(&st.to_string()) } else { s.world.best_neighbor_signal(bp(op)) > 0 };
                out_seq.push(on);
            }
        }
        if t == w0 + p {
            if s.world.state_key() != key_w {
                return SimReport::fail(c, Verdict::Unmodeled, format!("digest collision: states at {w0} and {} differ (report this)", w0 + p));
            }
            break;
        }
        s.world.tick();
    }
    // Visible period: the smallest divisor of P the block states repeat with; visible warmup: the
    // first tick from which they do (what the real-game judge measures).
    let (w0u, pu) = (w0 as usize, p as usize);
    let block_period = (1..=pu).filter(|d| pu % d == 0).find(|&d| (w0u..w0u + pu).all(|t| frames[t] == frames[w0u + (t - w0u + d) % pu])).unwrap_or(pu);
    let mut vis_w = w0u;
    while vis_w > c.free_from() as usize && frames[vis_w - 1] == frames[vis_w - 1 + block_period] {
        vis_w -= 1;
    }
    let refs: Vec<Vec<&str>> = period_states.iter().map(|v| v.iter().map(|s| s.as_str()).collect()).collect();
    let analysis = wave::analyze(&cells, &refs);
    let mut rep = SimReport {
        name: c.name().to_string(),
        verdict: Verdict::Clock,
        why: String::new(),
        warmup: vis_w as u32,
        state_warmup: w0,
        period: p,
        block_period: block_period as u32,
        analysis,
        output: None,
        blocks: c.blocks(),
        swept: (slo, shi),
        start: c.start_label(),
    };
    if rep.analysis.cells.is_empty() {
        rep.verdict = Verdict::Stops;
        rep.why = if p == 1 { format!("goes still at tick {w0}") } else { format!("no block changes in its period (the state repeats every {p} gt from tick {w0})") };
        return rep;
    }
    rep.output = match out_pos {
        Some(op) => {
            let (on, edges, widths) = wave::bool_wave(&out_seq);
            let states = rep.analysis.cells.iter().find(|w| w.pos == op).map(|w| w.states.clone()).unwrap_or_default();
            Some(Output { pos: op, auto: false, wave: CellWave { pos: op, states, kinds: Vec::new(), on, edges, widths } })
        }
        None => rep
            .analysis
            .cells
            .iter()
            .max_by(|a, b| (a.edges, a.on).cmp(&(b.edges, b.on)).then(b.pos.cmp(&a.pos)))
            .map(|w| Output { pos: w.pos, auto: true, wave: w.clone() }),
    };
    rep
}
