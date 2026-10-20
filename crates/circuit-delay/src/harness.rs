//! The stimulus harness on redsim (a prototype of clockinfra's `mctas circuit` judge, following
//! its decided format): place a design like the clock judge (raw paste, wire shapes, self
//! updates), drive its INPUT cells between ticks, read its OUTPUT level every gt.
//!
//! Design lines (docs/circuits.md once it lands):
//! * `circuit: <kind> [args]` (delay, gate [function], ...).
//! * `port input: x y z [args]` / `port input.<name>: ...`: no args = an air cell where the judge
//!   writes a redstone block (on) and air (off); `<on-state> [<off-state>]` = custom states;
//!   `lever` / `button` = the design's lever / button there, right-clicked.
//! * `port output: x y z`: dust = its power; repeater, comparator (strength), torch, observer =
//!   its own output; any other cell = the best signal into the cell. On = level > 0.
//! * Block count: every block except those at input cells.

use anyhow::{Result, anyhow, bail};
use mctas::design::Design;
use redsim::world::flags;
use redsim::{Block, BlockPos, World};

pub type Pos = [i32; 3];

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

#[derive(Clone, Debug)]
pub enum InputKind {
    /// The judge writes `on` / `off` (with updates).
    Place { on: Block, off: Block },
    /// The design's lever or button at the cell, right-clicked.
    Use,
}

#[derive(Clone, Debug)]
pub struct Input {
    pub name: String,
    pub pos: Pos,
    pub kind: InputKind,
}

#[derive(Clone, Debug)]
pub struct Circuit {
    pub design: Design,
    pub inputs: Vec<Input>,
    pub output: Pos,
}

impl Circuit {
    pub fn load(path: &std::path::Path) -> Result<Circuit> {
        Circuit::from_design(Design::load(path)?)
    }

    pub fn from_design(design: Design) -> Result<Circuit> {
        let mut inputs = Vec::new();
        for (name, pos) in &design.ports {
            if name != "input" && !name.starts_with("input.") {
                continue;
            }
            let words: Vec<String> = design.meta.get(&format!("port {name}")).map(|s| s.split_whitespace().map(|w| w.to_string()).collect()).unwrap_or_default();
            let kind = match words.first().map(|s| s.as_str()) {
                Some("lever") | Some("button") => {
                    if !design.blocks.get(pos).is_some_and(|s| s.contains("lever") || s.contains("button")) {
                        bail!("input {name}: no lever/button at {pos:?}");
                    }
                    InputKind::Use
                }
                _ => {
                    if design.blocks.contains_key(pos) {
                        bail!("input cell {pos:?} holds {} (leave it air, or say `lever`/`button`)", design.blocks[pos]);
                    }
                    let parse = |s: &str| Block::parse(s).map_err(|e| anyhow!("input state {s}: {e}"));
                    let on = parse(words.first().map(|s| s.as_str()).unwrap_or("minecraft:redstone_block"))?;
                    let off = parse(words.get(1).map(|s| s.as_str()).unwrap_or("minecraft:air"))?;
                    InputKind::Place { on, off }
                }
            };
            inputs.push(Input { name: name.clone(), pos: *pos, kind });
        }
        if inputs.is_empty() {
            bail!("no `port input:` line");
        }
        let output = *design.ports.get("output").ok_or_else(|| anyhow!("no `port output:` line"))?;
        Ok(Circuit { design, inputs, output })
    }

    /// Blocks placed, without those at input cells.
    pub fn blocks(&self) -> usize {
        self.design.blocks.keys().filter(|p| !self.inputs.iter().any(|i| i.pos == **p)).count()
    }

    pub fn bounds(&self) -> (Pos, Pos) {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for p in self.design.blocks.keys().chain(self.inputs.iter().map(|i| &i.pos)).chain(std::iter::once(&self.output)) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        (lo, hi)
    }

    /// The bounding box of the counted blocks.
    pub fn dims(&self) -> [i32; 3] {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for p in self.design.blocks.keys().filter(|p| !self.inputs.iter().any(|i| i.pos == **p)) {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        if lo[0] > hi[0] {
            return [0, 0, 0];
        }
        [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1]
    }

    pub fn input_index(&self, name: &str) -> Option<usize> {
        let full = if name == "input" || name.starts_with("input.") { name.to_string() } else { format!("input.{name}") };
        self.inputs.iter().position(|i| i.name == full || (self.inputs.len() == 1 && name.is_empty()))
    }
}

/// One input change: between ticks after `at` ticks (it is visible in frame `at`).
#[derive(Clone, Copy, Debug)]
pub struct Change {
    pub at: u32,
    pub input: usize,
    pub on: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Trace {
    /// The output level per frame (frame t = after t ticks and the changes due at t).
    pub level: Vec<i32>,
    /// Each input's level per frame.
    pub inputs: Vec<Vec<bool>>,
}

impl Trace {
    pub fn out(&self) -> Vec<bool> {
        self.level.iter().map(|l| *l > 0).collect()
    }
}

/// The output level of cell `p` (the judge's rule).
pub fn output_level(w: &World, p: BlockPos) -> i32 {
    match w.get(p) {
        Block::Wire { power, .. } => power as i32,
        Block::Repeater { powered, .. } | Block::Observer { powered, .. } => {
            if powered { 15 } else { 0 }
        }
        Block::Torch { lit } | Block::WallTorch { lit, .. } => {
            if lit { 15 } else { 0 }
        }
        Block::Comparator { .. } => w.comparator_out.get(&p).copied().unwrap_or(0) as i32,
        _ => w.best_neighbor_signal(p),
    }
}

/// Place the circuit (inputs off) and run `frames` frames with the given input changes (sorted by
/// `at`). Errors: a block redsim does not model, a drop (an item), a write beyond the margin.
pub fn run(c: &Circuit, changes: &[Change], frames: u32, margin: i32) -> Result<Trace, String> {
    let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
    for (p, s) in &c.design.blocks {
        blocks.push((bp(*p), Block::parse(s).map_err(|e| format!("{s}: {e}"))?));
    }
    let inputs: Vec<(BlockPos, InputKind)> = c.inputs.iter().map(|i| (bp(i.pos), i.kind.clone())).collect();
    let (lo, hi) = c.bounds();
    run_raw(&blocks, &inputs, bp(c.output), (lo, hi), changes, frames, margin)
}

/// [`run`] on raw redsim blocks (the census's fast path): `bounds` = the box the design and its
/// ports fit in.
pub fn run_raw(blocks: &[(BlockPos, Block)], inputs: &[(BlockPos, InputKind)], op: BlockPos, bounds: (Pos, Pos), changes: &[Change], frames: u32, margin: i32) -> Result<Trace, String> {
    let (mut levels, ins) = run_raw_multi(blocks, inputs, &[op], bounds, changes, frames, margin)?;
    Ok(Trace { level: levels.remove(0), inputs: ins })
}

/// [`run_raw`] reading several output cells at once: (levels per output, input levels).
pub fn run_raw_multi(blocks: &[(BlockPos, Block)], inputs: &[(BlockPos, InputKind)], outs: &[BlockPos], bounds: (Pos, Pos), changes: &[Change], frames: u32, margin: i32) -> Result<(Vec<Vec<i32>>, Vec<Vec<bool>>), String> {
    run_raw_multi_early(blocks, inputs, outs, bounds, changes, frames, margin, None)
}

/// [`run_raw_multi`], giving up (`Err("static")`) at frame `early.1` when no output level moved
/// since frame `early.0`.
#[allow(clippy::too_many_arguments)]
pub fn run_raw_multi_early(blocks: &[(BlockPos, Block)], inputs: &[(BlockPos, InputKind)], outs: &[BlockPos], bounds: (Pos, Pos), changes: &[Change], frames: u32, margin: i32, early: Option<(u32, u32)>) -> Result<(Vec<Vec<i32>>, Vec<Vec<bool>>), String> {
    let mut all: Vec<(BlockPos, Block)> = blocks.to_vec();
    for (p, k) in inputs {
        if let InputKind::Place { off, .. } = k
            && *off != Block::Air
        {
            all.push((*p, *off));
        }
    }
    let (lo, hi) = bounds;
    let min = BlockPos::new(lo[0] - margin, lo[1] - margin, lo[2] - margin);
    let size = [hi[0] - lo[0] + 1 + 2 * margin, hi[1] - lo[1] + 1 + 2 * margin, hi[2] - lo[2] + 1 + 2 * margin];
    let mut w = World::new(min, size);
    w.place_raw(&all);
    w.place_updates(&all);
    let mut level_now = vec![false; inputs.len()];
    let mut ins: Vec<Vec<bool>> = vec![Vec::with_capacity(frames as usize); inputs.len()];
    let mut levels: Vec<Vec<i32>> = vec![Vec::with_capacity(frames as usize); outs.len()];
    let mut next = 0;
    for t in 0..frames {
        while next < changes.len() && changes[next].at == t {
            let ch = changes[next];
            next += 1;
            if level_now[ch.input] == ch.on {
                continue;
            }
            level_now[ch.input] = ch.on;
            let (ip, kind) = &inputs[ch.input];
            match kind {
                InputKind::Place { on, off } => {
                    w.set_block(*ip, if ch.on { *on } else { *off }, flags::ALL);
                }
                InputKind::Use => {
                    if !w.use_block(*ip) {
                        return Err(format!("frame {t}: the input at {ip} could not be used"));
                    }
                }
            }
        }
        if let Some(e) = w.errors.first() {
            return Err(format!("frame {t}: {e}"));
        }
        if let Some((p, b)) = w.drops.first() {
            return Err(format!("frame {t}: {b} at {p} was destroyed with a drop"));
        }
        for (k, op) in outs.iter().enumerate() {
            levels[k].push(output_level(&w, *op));
        }
        for (k, v) in level_now.iter().enumerate() {
            ins[k].push(*v);
        }
        if let Some((a, b)) = early
            && t == b
            && levels.iter().all(|lv| lv[a as usize..=b as usize].iter().all(|l| (*l > 0) == (lv[a as usize] > 0)))
        {
            return Err("static".into());
        }
        w.tick();
    }
    Ok((levels, ins))
}

/// Parse a `stimulus:` header: `a 0+ 4- 20+ 22-; b 10+ 30-` (the name may be left out with one
/// input) or `pulse 4 every 40 x 6 [from 20]`.
pub fn parse_stimulus(c: &Circuit, s: &str) -> Result<Vec<Change>> {
    let mut out = Vec::new();
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("pulse ") {
        let w: Vec<&str> = rest.split_whitespace().collect();
        let num = |i: usize| -> Result<u32> { w.get(i).ok_or_else(|| anyhow!("stimulus {s:?}"))?.parse::<u32>().map_err(|_| anyhow!("stimulus {s:?}")) };
        let (len, every, count) = (num(0)?, num(2)?, num(4)?);
        let from = if w.get(5) == Some(&"from") { num(6)? } else { 20 };
        for k in 0..count {
            out.push(Change { at: from + k * every, input: 0, on: true });
            out.push(Change { at: from + k * every + len, input: 0, on: false });
        }
    } else {
        for part in s.split(';') {
            let words: Vec<&str> = part.split_whitespace().collect();
            if words.is_empty() {
                continue;
            }
            let (name, evs) = if words[0].ends_with('+') || words[0].ends_with('-') { ("", &words[..]) } else { (words[0], &words[1..]) };
            let input = c.input_index(name).ok_or_else(|| anyhow!("stimulus: no input {name:?}"))?;
            for e in evs {
                let (num, on) = if let Some(n) = e.strip_suffix('+') { (n, true) } else if let Some(n) = e.strip_suffix('-') { (n, false) } else { bail!("stimulus event {e:?}: want <tick>+ or <tick>-") };
                out.push(Change { at: num.parse()?, input, on });
            }
        }
    }
    out.sort_by_key(|c| c.at);
    Ok(out)
}

/// A waveform as text: `_` off, `#` on (one char per frame), from frame `from`.
pub fn wave(v: &[bool], from: usize) -> String {
    v.iter().skip(from).map(|b| if *b { '#' } else { '_' }).collect()
}
