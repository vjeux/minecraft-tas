//! Pulse circuits (contraption study 1, track `pulse`): a prototype judge on redsim, until
//! clockinfra's `mctas circuit` lands. A design is a clock-format design plus `port input:
//! x y z`: an EMPTY cell where the stimulus places a redstone block (input on) and air again
//! (input off), with block updates, like a powered block the judge toggles. The output is the
//! power level a block at a cell would receive (`best_neighbor_signal`), recorded at the end of
//! every game tick: `port output: x y z` names the cell, else every cell around the design is a
//! candidate and each clean behavior found is reported.
//!
//! The stimulus: settle (the design must go idle), then input pulses of 1, 2, 3, 4, 5, 6, 8, 10,
//! 14, 20, 30 gt, each followed by a rest until everything is idle again (and the outputs back to
//! their rest level: a design that keeps state is not a pulse circuit). The input is switched
//! between ticks: on before tick 1, off before tick w + 1; output samples are taken after each
//! tick, so an output on from sample 1 has delay 0.
//!
//! Kinds (one output cell, every tested input length w):
//! * `rise L d`: one output pulse per input pulse, L gt long, starting d gt after the rising edge,
//!   whatever w (a rising-edge detector; a monostable / fixed-length pulse generator);
//! * `fall L d`: one pulse, L gt long, d gt after the falling edge;
//! * `dual L d/e`: two pulses of L gt, d after the rising and e after the falling edge (once the
//!   input is long enough to separate them);
//! * `limit L d`: one pulse of min(w, L) gt (long inputs cut to L, short ones passed);
//! * `extend L d`: one pulse of max(w, L) gt (short inputs stretched to L);
//! * `stretch +c d`: one pulse of w + c gt (c > 0);
//! * `delay d`: one pulse of w gt, d gt late (a delay line, study 3).

use clock_obs::sim::{self, Spec};
use anyhow::{Context, Result, bail};
use clap::Args as ClapArgs;
use redsim::world::flags;
use redsim::{Block, BlockPos, World};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const WIDTHS: [u32; 12] = [1, 2, 3, 4, 5, 6, 8, 10, 15, 20, 30, 40];

/// A pulse-circuit design: the clock-format blocks, the input cell, an optional output cell.
#[derive(Clone, Debug)]
pub struct Circuit {
    pub name: String,
    pub blocks: Vec<(BlockPos, Block)>,
    pub input: BlockPos,
    pub output: Option<BlockPos>,
}

impl Circuit {
    pub fn load(path: &std::path::Path) -> Result<Circuit> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let mut input = None;
        let mut rest = String::new();
        for line in text.lines() {
            if let Some(r) = line.trim().strip_prefix("port input:") {
                let v: Vec<i32> = r.split_whitespace().take(3).map(|t| t.parse()).collect::<Result<_, _>>()?;
                if v.len() == 3 {
                    input = Some(BlockPos::new(v[0], v[1], v[2]));
                }
                continue;
            }
            rest.push_str(line);
            rest.push('\n');
        }
        let s = Spec::parse(&rest)?;
        let Some(input) = input else { bail!("{}: no `port input: x y z`", path.display()) };
        Ok(Circuit { name: s.name, blocks: s.blocks, input, output: s.output })
    }

    pub fn to_text(&self, comment: &str) -> String {
        let spec = Spec { name: self.name.clone(), blocks: self.blocks.clone(), kick: sim::Kick::None, output: self.output };
        let mut t = spec.to_text(comment);
        let at = t.find("\npalette:").map(|i| i + 1).unwrap_or(t.len());
        t.insert_str(at, &format!("port input: {} {} {}\n", self.input.x, self.input.y, self.input.z));
        t
    }
}

/// One output pulse: start (samples after the rising edge, 0 = on at the first sample), length,
/// peak level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pulse {
    pub start: i32,
    pub len: u32,
    pub level: i32,
}

/// The recorded responses: for every candidate cell, the pulses after each input length.
pub struct Responses {
    pub cells: Vec<BlockPos>,
    /// [cell][width index] -> pulses
    pub pulses: Vec<Vec<Vec<Pulse>>>,
}

/// The judge's output level at a cell (docs/circuits.md, `circuit::trace::level`): dust its power;
/// a repeater, observer or torch its own output (15 when on); a comparator its strength; any
/// other cell the best signal into it.
pub fn level_at(w: &World, p: BlockPos) -> i32 {
    match w.get(p) {
        Block::Wire { power, .. } => power as i32,
        Block::Repeater { powered, .. } | Block::Observer { powered, .. } => {
            if powered {
                15
            } else {
                0
            }
        }
        Block::Torch { lit } | Block::WallTorch { lit, .. } => {
            if lit {
                15
            } else {
                0
            }
        }
        Block::Comparator { .. } => w.comparator_out.get(&p).copied().unwrap_or(0) as i32,
        _ => w.best_neighbor_signal(p),
    }
}

fn pulses_of(levels: &[i32]) -> Vec<Pulse> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < levels.len() {
        if levels[i] > 0 {
            let s = i;
            let mut peak = 0;
            while i < levels.len() && levels[i] > 0 {
                peak = peak.max(levels[i]);
                i += 1;
            }
            out.push(Pulse { start: s as i32 + 1, len: (i - s) as u32, level: peak });
        } else {
            i += 1;
        }
    }
    out
}

/// Run the stimulus. None when the design does not settle, keeps state, or touches the margin.
pub fn respond(c: &Circuit, widths: &[u32]) -> Option<Responses> {
    let margin = 3;
    let mut all: Vec<BlockPos> = c.blocks.iter().map(|(p, _)| *p).collect();
    all.push(c.input);
    let lo = BlockPos::new(all.iter().map(|p| p.x).min()?, all.iter().map(|p| p.y).min()?, all.iter().map(|p| p.z).min()?);
    let hi = BlockPos::new(all.iter().map(|p| p.x).max()?, all.iter().map(|p| p.y).max()?, all.iter().map(|p| p.z).max()?);
    let min = lo.offset(-margin, -margin, -margin);
    let size = [hi.x - lo.x + 1 + 2 * margin, hi.y - lo.y + 1 + 2 * margin, hi.z - lo.z + 1 + 2 * margin];
    let mut w = World::new(min, size);
    w.place_like_judge(&c.blocks);
    if w.run_until_idle(400) >= 400 && !w.idle() {
        return None;
    }
    // candidate cells: the given output, else every cell within 1 of the design (not the input)
    let cells: Vec<BlockPos> = match c.output {
        Some(o) => vec![o],
        None => {
            let mut v = Vec::new();
            for x in lo.x - 1..=hi.x + 1 {
                for y in lo.y - 1..=hi.y + 1 {
                    for z in lo.z - 1..=hi.z + 1 {
                        let p = BlockPos::new(x, y, z);
                        let near_input = (c.input.x - x).abs() + (c.input.y - y).abs() + (c.input.z - z).abs() <= 1;
                        if !near_input && all.iter().any(|q| (q.x - x).abs() + (q.y - y).abs() + (q.z - z).abs() <= 1) {
                            v.push(p);
                        }
                    }
                }
            }
            v
        }
    };
    let rest: Vec<i32> = cells.iter().map(|p| level_at(&w, *p)).collect();
    let snapshot = w.blocks();
    // one input pulse of `wd` gt: the recorded deviations from rest, None when the design does
    // not come back to rest (kept state, runaway)
    fn run_pulse(w: &mut World, input: BlockPos, cells: &[BlockPos], rest: &[i32], snapshot: &[(BlockPos, Block)], wd: u32) -> Option<Vec<Vec<i32>>> {
        let mut rec: Vec<Vec<i32>> = vec![Vec::new(); cells.len()];
        w.set_block(input, Block::RedstoneBlock, flags::ALL);
        let mut t = 0u32;
        let mut quiet = 0u32;
        loop {
            if t == wd {
                w.set_block(input, Block::Air, flags::ALL);
            }
            w.tick();
            t += 1;
            for (i, p) in cells.iter().enumerate() {
                rec[i].push(level_at(w, *p) - rest[i]);
            }
            if t > wd {
                let at_rest = cells.iter().enumerate().all(|(i, p)| level_at(w, *p) == rest[i]);
                quiet = if w.idle() && at_rest { quiet + 1 } else { 0 };
                if quiet >= 8 {
                    break;
                }
            }
            if t > wd + 300 {
                return None;
            }
        }
        (w.blocks() == snapshot).then_some(rec)
    }
    // a quick probe with the longest input: an inert design (no cell ever moves) is dropped here
    let probe = run_pulse(&mut w, c.input, &cells, &rest, &snapshot, widths.iter().copied().max().unwrap_or(40))?;
    if probe.iter().all(|r| r.iter().all(|l| *l == 0)) {
        return None;
    }
    let mut pulses: Vec<Vec<Vec<Pulse>>> = vec![Vec::new(); cells.len()];
    for &wd in widths {
        let rec = run_pulse(&mut w, c.input, &cells, &rest, &snapshot, wd)?;
        for i in 0..cells.len() {
            if rec[i].iter().any(|l| *l < 0) {
                pulses[i].push(vec![Pulse { start: -1, len: 0, level: -1 }]); // inverted: not handled
            } else {
                pulses[i].push(pulses_of(&rec[i]));
            }
        }
    }
    Some(Responses { cells, pulses })
}

/// The kind of one cell's responses (None: no clean pulse behavior), in the circuit judge's
/// vocabulary (clockinfra 2026-10-19): `<kind> L=<L> delay=<d> min-pulse=<w0>`, trying the
/// shortest min-pulse first (the behavior must hold for every tested input from w0 on).
pub fn classify(per_w: &[Vec<Pulse>], widths: &[u32]) -> Option<String> {
    classify_all(per_w, widths).into_iter().next()
}

/// Every kind these responses satisfy, each with its smallest min-pulse (the behavior holds for
/// every tested input from it on; the judge wants at least the 3 longest): a constant-length
/// monostable is a rising-edge detector, a limiter and an extender at once.
pub fn classify_all(per_w: &[Vec<Pulse>], widths: &[u32]) -> Vec<String> {
    let mut best: BTreeMap<String, String> = BTreeMap::new();
    for w0 in 0..widths.len().saturating_sub(2) {
        for k in kinds_from(&per_w[w0..], &widths[w0..]) {
            let name = k.split_whitespace().next().unwrap_or("").to_string();
            best.entry(name).or_insert_with(|| format!("{k} min-pulse={}", widths[w0]));
        }
    }
    best.into_values().collect()
}

fn kinds_from(per_w: &[Vec<Pulse>], widths: &[u32]) -> Vec<String> {
    let mut out = Vec::new();
    if per_w.iter().any(|p| p.iter().any(|q| q.level < 0)) || per_w.iter().all(|p| p.is_empty()) {
        return out;
    }
    if per_w.iter().all(|p| p.len() == 1) {
        let s: Vec<i32> = per_w.iter().map(|p| p[0].start).collect();
        let l: Vec<u32> = per_w.iter().map(|p| p[0].len).collect();
        let s_const = s.iter().all(|x| *x == s[0]);
        let l_const = l.iter().all(|x| *x == l[0]);
        if s_const && l_const {
            out.push(format!("rising-edge L={} delay={}", l[0], s[0]));
        }
        let e: Vec<i32> = s.iter().zip(widths).map(|(x, w)| x - *w as i32).collect();
        if e.iter().all(|x| *x == e[0]) && l_const {
            out.push(format!("falling-edge L={} delay={}", l[0], e[0]));
        }
        if s_const {
            let wmax = widths.iter().copied().max().unwrap_or(0);
            for big_l in 1..wmax {
                // limiter: L for every input >= L (shorter inputs pass shorter, up to L)
                if widths.iter().any(|w| *w > big_l) && l.iter().zip(widths).all(|(x, w)| if *w >= big_l { *x == big_l } else { *x <= big_l }) {
                    out.push(format!("pulse-limiter L={big_l} delay={}", s[0]));
                    break;
                }
            }
            for big_l in 2..=wmax + 20 {
                // extender: L for every input shorter than L, never shorter than L
                if widths.iter().any(|w| *w < big_l) && l.iter().zip(widths).all(|(x, w)| if *w < big_l { *x == big_l } else { *x >= big_l }) {
                    out.push(format!("pulse-extender L={big_l} delay={}", s[0]));
                    break;
                }
            }
            if l.iter().zip(widths).all(|(x, w)| x == w) {
                out.push(format!("delay N={}", s[0]));
            }
        }
        return out;
    }
    // dual: one pulse on each edge, for every input of the suffix (the judge's min-pulse counts
    // an input as handled only when both edges answer)
    let mut dual: Option<(u32, u32, i32, i32)> = None;
    for (p, w) in per_w.iter().zip(widths) {
        match p.len() {
            2 => {
                let key = (p[0].len, p[1].len, p[0].start, p[1].start - *w as i32);
                if dual.is_some_and(|d| d != key) {
                    return out;
                }
                dual = Some(key);
            }
            _ => return out,
        }
    }
    let Some((lr, lf, d, e)) = dual else { return out };
    if per_w.last().is_some_and(|p| p.len() == 2) {
        out.push(if lr == lf { format!("dual-edge L={lr} delay={d}/{e}") } else { format!("dual-edge L={lr}/{lf} delay={d}/{e}") });
    }
    out
}

#[allow(dead_code)]
fn classify_from(per_w: &[Vec<Pulse>], widths: &[u32]) -> Option<String> {
    if per_w.iter().any(|p| p.iter().any(|q| q.level < 0)) || per_w.iter().all(|p| p.is_empty()) {
        return None;
    }
    if per_w.iter().all(|p| p.len() == 1) {
        let s: Vec<i32> = per_w.iter().map(|p| p[0].start).collect();
        let l: Vec<u32> = per_w.iter().map(|p| p[0].len).collect();
        let s_const = s.iter().all(|x| *x == s[0]);
        let l_const = l.iter().all(|x| *x == l[0]);
        if s_const && l_const {
            return Some(format!("rising-edge L={} delay={}", l[0], s[0]));
        }
        let e: Vec<i32> = s.iter().zip(widths).map(|(x, w)| x - *w as i32).collect();
        if e.iter().all(|x| *x == e[0]) && l_const {
            return Some(format!("falling-edge L={} delay={}", l[0], e[0]));
        }
        if s_const {
            let wmax = *widths.iter().max()?;
            for big_l in 2..wmax {
                // limiter: L for every input >= L, shorter inputs pass (or stretch, up to L)
                if l.iter().zip(widths).all(|(x, w)| if *w >= big_l { *x == big_l } else { *x >= *w && *x <= big_l }) && widths.iter().any(|w| *w > big_l) {
                    return Some(format!("pulse-limiter L={big_l} delay={}", s[0]));
                }
                // extender: L for every input < L, never shorter than the input
                if l.iter().zip(widths).all(|(x, w)| if *w < big_l { *x == big_l } else { *x >= *w }) && widths.iter().any(|w| *w < big_l) {
                    let tail = l.iter().zip(widths).filter(|(_, w)| **w >= big_l).map(|(x, w)| *x as i64 - *w as i64).collect::<Vec<_>>();
                    let plus = if tail.iter().all(|c| *c == 0) { String::new() } else { " (longer inputs lengthened)".to_string() };
                    return Some(format!("pulse-extender L={big_l} delay={}{plus}", s[0]));
                }
            }
            let c0 = l[0] as i64 - widths[0] as i64;
            if l.iter().zip(widths).all(|(x, w)| *x as i64 - *w as i64 == c0) {
                return Some(if c0 == 0 { format!("delay N={}", s[0]) } else if c0 > 0 { format!("stretch +{c0} delay={}", s[0]) } else { format!("shrink {c0} delay={}", s[0]) });
            }
        }
        return None;
    }
    // dual: one pulse on each edge once separated (short inputs may merge into one)
    let mut dual: Option<(u32, u32, i32, i32)> = None;
    for (p, w) in per_w.iter().zip(widths) {
        match p.len() {
            2 => {
                let key = (p[0].len, p[1].len, p[0].start, p[1].start - *w as i32);
                if dual.is_some_and(|d| d != key) {
                    return None;
                }
                dual = Some(key);
            }
            1 => {}
            _ => return None,
        }
    }
    let (lr, lf, d, e) = dual?;
    if !per_w.last().is_some_and(|p| p.len() == 2) {
        return None;
    }
    Some(if lr == lf { format!("dual-edge L={lr} delay={d}/{e}") } else { format!("dual-edge L={lr}/{lf} delay={d}/{e}") })
}

/// The old prototype classification, kept for reference.
pub fn classify_v0(per_w: &[Vec<Pulse>], widths: &[u32]) -> Option<String> {
    if per_w.iter().any(|p| p.iter().any(|q| q.level < 0)) || per_w.iter().all(|p| p.is_empty()) {
        return None;
    }
    if per_w.iter().all(|p| p.len() == 1) {
        let s: Vec<i32> = per_w.iter().map(|p| p[0].start).collect();
        let l: Vec<u32> = per_w.iter().map(|p| p[0].len).collect();
        let s_const = s.iter().all(|x| *x == s[0]);
        let l_const = l.iter().all(|x| *x == l[0]);
        if s_const && l_const {
            return Some(format!("rise {} {}", l[0], s[0]));
        }
        let e: Vec<i32> = s.iter().zip(widths).map(|(x, w)| x - *w as i32).collect();
        if e.iter().all(|x| *x == e[0]) && l_const {
            return Some(format!("fall {} {}", l[0], e[0]));
        }
        if s_const {
            let wmax = *widths.iter().max()?;
            for big_l in 2..wmax {
                if l.iter().zip(widths).all(|(x, w)| *x == (*w).min(big_l)) {
                    return Some(format!("limit {big_l} {}", s[0]));
                }
                if l.iter().zip(widths).all(|(x, w)| *x == (*w).max(big_l)) {
                    return Some(format!("extend {big_l} {}", s[0]));
                }
            }
            let c0 = l[0] as i64 - widths[0] as i64;
            if l.iter().zip(widths).all(|(x, w)| *x as i64 - *w as i64 == c0) {
                return Some(if c0 == 0 { format!("delay {}", s[0]) } else if c0 > 0 { format!("stretch +{c0} {}", s[0]) } else { format!("shrink {c0} {}", s[0]) });
            }
        }
        return None;
    }
    // dual: two pulses once separated (short inputs may merge into one)
    let mut dual: Option<(u32, i32, i32)> = None;
    for (p, w) in per_w.iter().zip(widths) {
        match p.len() {
            2 => {
                let key = (p[0].len, p[0].start, p[1].start - *w as i32);
                if p[1].len != p[0].len || dual.is_some_and(|d| d != key) {
                    return None;
                }
                dual = Some(key);
            }
            1 => {}
            _ => return None,
        }
    }
    let (l, d, e) = dual?;
    // the longest inputs must separate
    if per_w.last().is_some_and(|p| p.len() == 2) {
        return Some(format!("dual {l} {d}/{e}"));
    }
    None
}

#[derive(ClapArgs, Debug, Clone)]
pub struct PulseArgs {
    /// Designs (files or folders)
    pub designs: Vec<PathBuf>,
    /// Print every response
    #[arg(long)]
    pub verbose: bool,
}

#[derive(ClapArgs, Debug, Clone)]
pub struct PulseCensusArgs {
    /// Blocks besides the input cell
    #[arg(long, default_value_t = 3)]
    pub k: usize,
    /// Palette (census shorthands: obs, piston, sticky, note, lamp, bulb, stone, slime, rblock,
    /// trap; plus dust, repeater, comparator, torch; or raw block states)
    #[arg(long, default_value = "obs,piston,sticky,note,lamp,bulb,stone,slime,rblock")]
    pub palette: String,
    /// Designs kept per kind
    #[arg(long, default_value_t = 3)]
    pub keep: usize,
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
    #[arg(long, default_value_t = 0)]
    pub threads: usize,
}

/// The pulse palette: the clock census shorthands plus redstone components.
pub fn pulse_palette(spec: &str) -> Result<Vec<Block>> {
    let mut v = Vec::new();
    for name in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let hd = ["north", "east", "south", "west"];
        match name {
            "dust" => v.push(Block::parse("minecraft:redstone_wire").map_err(|e| anyhow::anyhow!("{e}"))?),
            "repeater" => {
                for f in hd {
                    for d in 1..=4 {
                        v.push(Block::parse(&format!("minecraft:repeater[facing={f},delay={d}]")).map_err(|e| anyhow::anyhow!("{e}"))?);
                    }
                }
            }
            "repeater1" => {
                for f in hd {
                    v.push(Block::parse(&format!("minecraft:repeater[facing={f},delay=1]")).map_err(|e| anyhow::anyhow!("{e}"))?);
                }
            }
            "comparator" => {
                for f in hd {
                    for m in ["compare", "subtract"] {
                        v.push(Block::parse(&format!("minecraft:comparator[facing={f},mode={m}]")).map_err(|e| anyhow::anyhow!("{e}"))?);
                    }
                }
            }
            "torch" => {
                v.push(Block::parse("minecraft:redstone_torch").map_err(|e| anyhow::anyhow!("{e}"))?);
                for f in hd {
                    v.push(Block::parse(&format!("minecraft:redstone_wall_torch[facing={f}]")).map_err(|e| anyhow::anyhow!("{e}"))?);
                }
            }
            other => v.extend(clock_obs::census::palette(other)?),
        }
    }
    Ok(v)
}

/// The variant (kind of block, ignoring its state) of a block, for the drop check.
fn variant(b: &Block) -> String {
    let s = b.to_string();
    s.split('[').next().unwrap_or(&s).to_string()
}

/// After settling, does every design block still stand (no drop, no move)?
pub fn stands(c: &Circuit) -> bool {
    let mut all: Vec<BlockPos> = c.blocks.iter().map(|(p, _)| *p).collect();
    all.push(c.input);
    let lo = BlockPos::new(all.iter().map(|p| p.x).min().unwrap_or(0) - 3, all.iter().map(|p| p.y).min().unwrap_or(0) - 3, all.iter().map(|p| p.z).min().unwrap_or(0) - 3);
    let hi = BlockPos::new(all.iter().map(|p| p.x).max().unwrap_or(0) + 3, all.iter().map(|p| p.y).max().unwrap_or(0) + 3, all.iter().map(|p| p.z).max().unwrap_or(0) + 3);
    let mut w = World::new(lo, [hi.x - lo.x + 1, hi.y - lo.y + 1, hi.z - lo.z + 1]);
    w.place_like_judge(&c.blocks);
    w.run_until_idle(400);
    w.errors.is_empty() && c.blocks.iter().all(|(p, b)| variant(&w.get(*p)) == variant(b))
}

pub fn census(a: PulseCensusArgs) -> Result<()> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let pal = pulse_palette(&a.palette)?;
    let shapes = clock_obs::census::shape_classes(a.k + 1);
    // work units: (shape, input cell index, first block state)
    let mut units: Vec<(usize, usize, usize)> = Vec::new();
    for (si, s) in shapes.iter().enumerate() {
        for ii in 0..s.len() {
            for b0 in 0..pal.len() {
                units.push((si, ii, b0));
            }
        }
    }
    let threads = if a.threads > 0 { a.threads } else { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8).saturating_sub(6).max(1) };
    eprintln!("pulse census: k={} (+ input), {} shapes, palette {} states, {} units, {threads} threads", a.k, shapes.len(), pal.len(), units.len());
    let next = AtomicUsize::new(0);
    let sims = AtomicUsize::new(0);
    // kind -> [(canonical key, circuit)]
    let found: Mutex<BTreeMap<String, Vec<((u32, i32), u128, Circuit, String)>>> = Mutex::new(BTreeMap::new());
    let in_marker = Block::parse("minecraft:glass").unwrap();
    let out_marker = Block::parse("minecraft:honey_block").unwrap();
    let t0 = std::time::Instant::now();
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let u = next.fetch_add(1, Ordering::Relaxed);
                if u >= units.len() {
                    break;
                }
                if u % 2000 == 0 {
                    eprintln!("  unit {u}/{} ({:.0}s, {} sims)", units.len(), t0.elapsed().as_secs_f64(), sims.load(Ordering::Relaxed));
                }
                let (si, ii, b0) = units[u];
                let shape = &shapes[si];
                let input = BlockPos::new(shape[ii][0], shape[ii][1], shape[ii][2]);
                let cells: Vec<BlockPos> = shape.iter().enumerate().filter(|(i, _)| *i != ii).map(|(_, c)| BlockPos::new(c[0], c[1], c[2])).collect();
                let n = cells.len();
                let mut idx = vec![0usize; n];
                idx[0] = b0;
                loop {
                    let mut blocks: Vec<(BlockPos, Block)> = cells.iter().zip(&idx).map(|(p, i)| (*p, pal[*i])).collect();
                    clock_obs::census::fix_instruments(&mut blocks);
                    let c = Circuit { name: String::new(), blocks, input, output: None };
                    sims.fetch_add(1, Ordering::Relaxed);
                    if let Some(r) = respond(&c, &WIDTHS) {
                        let mut kinds: Vec<(String, BlockPos)> = Vec::new();
                        for (i, p) in r.cells.iter().enumerate() {
                            for k in classify_all(&r.pulses[i], &WIDTHS) {
                                if !kinds.iter().any(|(q, _)| *q == k) {
                                    kinds.push((k, *p));
                                }
                            }
                        }
                        if !kinds.is_empty() && stands(&c) {
                            let mut f = found.lock().unwrap();
                            for (k, out) in kinds {
                                let mut keyed = c.blocks.clone();
                                keyed.push((c.input, in_marker));
                                keyed.push((out, out_marker));
                                let key = clock_obs::census::canon_hash(&keyed);
                                let (group, rank) = group_rank(&k);
                                let e = f.entry(group).or_default();
                                if !e.iter().any(|(_, q, _, _)| *q == key) {
                                    e.push((rank, key, Circuit { output: Some(out), ..c.clone() }, k.clone()));
                                    e.sort_by_key(|x| (x.0, x.1));
                                    e.truncate(a.keep);
                                }
                            }
                        }
                    }
                    // next assignment of cells 1..n
                    let mut j = 1;
                    loop {
                        if j >= n {
                            break;
                        }
                        idx[j] += 1;
                        if idx[j] < pal.len() {
                            break;
                        }
                        idx[j] = 0;
                        j += 1;
                    }
                    if j >= n {
                        break;
                    }
                }
            });
        }
    });
    let found = found.into_inner().unwrap();
    eprintln!("pulse census done: {} sims in {:.0}s, {} kinds", sims.load(Ordering::Relaxed), t0.elapsed().as_secs_f64(), found.len());
    if let Some(d) = &a.out_dir {
        std::fs::create_dir_all(d)?;
    }
    println!("| kind, L | designs | best (min-pulse, delay) | example |\n|---|---|---|---|");
    for (g, v) in &found {
        let slug = g.replace(' ', "-").replace('/', "_").replace('+', "p").replace('=', "");
        let mut names = Vec::new();
        for (i, (_, _, c, k)) in v.iter().enumerate() {
            let name = format!("pulse-{slug}-{}b-{i:02}", a.k);
            if let Some(d) = &a.out_dir {
                let cc = Circuit { name: name.clone(), ..c.clone() };
                let kind = k.split_whitespace().next().unwrap_or("delay");
                let mut text = cc.to_text(&format!("Pulse census (clk-obs pulse prototype on redsim, k = {} + input): {k}.", a.k));
                text = text.replacen("\norigin:", &format!("\ncircuit: {kind}\norigin:"), 1);
                std::fs::write(d.join(format!("{name}.txt")), text)?;
            }
            names.push(name);
        }
        println!("| {g} | {} | {} | {} |", v.len(), v.first().map(|x| x.3.clone()).unwrap_or_default(), names.first().cloned().unwrap_or_default());
    }
    Ok(())
}

/// The gallery group of a classified kind (`<kind> L=<L>`) and its rank inside the group
/// (min-pulse, then delay: smaller first).
pub fn group_rank(k: &str) -> (String, (u32, i32)) {
    let toks: Vec<&str> = k.split_whitespace().collect();
    let group = toks.iter().take(2).cloned().collect::<Vec<_>>().join(" ");
    let get = |name: &str| toks.iter().find_map(|t| t.strip_prefix(name)).unwrap_or("0");
    let mp: u32 = get("min-pulse=").parse().unwrap_or(99);
    let delay: i32 = get("delay=").split('/').next().unwrap_or("0").parse().unwrap_or(99);
    (group, (mp, delay))
}

pub fn run(a: PulseArgs) -> Result<()> {
    for f in clock_obs::pick::files_of(&a.designs)? {
        let c = Circuit::load(&f)?;
        let Some(r) = respond(&c, &WIDTHS) else {
            println!("{}: NOT A PULSE CIRCUIT (does not settle, keeps state, or runs away)", f.display());
            continue;
        };
        let mut kinds: BTreeMap<String, Vec<BlockPos>> = BTreeMap::new();
        for (i, p) in r.cells.iter().enumerate() {
            for k in classify_all(&r.pulses[i], &WIDTHS) {
                kinds.entry(k).or_default().push(*p);
            }
            if a.verbose && r.pulses[i].iter().any(|v| !v.is_empty()) {
                let s: Vec<String> = r.pulses[i].iter().zip(WIDTHS).map(|(v, w)| format!("w{w}:{}", v.iter().map(|q| format!("{}+{}", q.start, q.len)).collect::<Vec<_>>().join(","))).collect();
                println!("  {p}: {}", s.join(" "));
            }
        }
        if kinds.is_empty() {
            println!("{}: no clean pulse behavior at any cell", f.display());
        }
        for (k, cells) in kinds {
            println!("{}: {k} at {}", f.display(), cells.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(" "));
        }
    }
    Ok(())
}
