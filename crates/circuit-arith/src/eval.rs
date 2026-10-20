//! A design run on redsim the way the circuit judge runs it (placed raw, wire shapes, self updates;
//! the judge's lamp in every air output cell; inputs written with updates between ticks: a
//! redstone block for 1, air for 0), through the judge's walk: per transition the outputs' final
//! values, the latency and the glitches.

use crate::func::Func;
use anyhow::{Context, Result, anyhow, bail};
use mctas::design::Design;
use redsim::world::{World, flags};
use redsim::{Block, BlockPos};

pub type Pos = [i32; 3];

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A design ready to run: its blocks, and its input and output cells in the function's order.
#[derive(Clone, Debug)]
pub struct Rig {
    pub blocks: Vec<(BlockPos, Block)>,
    pub inputs: Vec<Pos>,
    pub outputs: Vec<Pos>,
    /// The judge puts a lamp in an output cell the design leaves air.
    pub lamps: Vec<bool>,
}

impl Rig {
    pub fn from_design(d: &Design, f: &Func) -> Result<Rig> {
        let port = |pre: &str, name: &str| -> Result<Pos> { d.ports.get(&format!("{pre}.{name}")).copied().ok_or_else(|| anyhow!("missing port {pre}.{name}")) };
        let inputs: Vec<Pos> = f.inputs.iter().map(|n| port("input", n)).collect::<Result<_>>()?;
        let outputs: Vec<Pos> = f.outputs.iter().map(|n| port("output", n)).collect::<Result<_>>()?;
        let mut blocks = Vec::new();
        for (p, s) in &d.blocks {
            blocks.push((bp(*p), Block::parse(s).map_err(|e| anyhow!("{s}: {e}"))?));
        }
        for p in &inputs {
            if d.blocks.contains_key(p) {
                bail!("input cell {p:?} holds a block: an input cell is air (the judge writes a redstone block there)");
            }
        }
        let lamps = outputs.iter().map(|p| !d.blocks.contains_key(p)).collect();
        Ok(Rig { blocks, inputs, outputs, lamps })
    }

    /// The box of the blocks and ports.
    pub fn bounds(&self) -> (Pos, Pos) {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        let mut add = |p: Pos| {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        };
        for (p, _) in &self.blocks {
            add([p.x, p.y, p.z]);
        }
        for p in self.inputs.iter().chain(&self.outputs) {
            add(*p);
        }
        (lo, hi)
    }

    /// A placed world at rest (inputs off), `None` when it never goes still or breaks.
    pub fn world(&self) -> std::result::Result<World, String> {
        let (lo, hi) = self.bounds();
        let m = 4;
        let min = BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m);
        let size = [hi[0] - lo[0] + 1 + 2 * m, hi[1] - lo[1] + 1 + 2 * m, hi[2] - lo[2] + 1 + 2 * m];
        let mut w = World::new(min, size);
        let mut all = self.blocks.clone();
        for (k, p) in self.outputs.iter().enumerate() {
            if self.lamps[k] {
                all.push((bp(*p), Block::Lamp { lit: false }));
            }
        }
        w.place_raw(&all);
        w.place_updates(&all);
        for (p, b) in &self.blocks {
            if !redsim::behavior::can_survive(&w, b, *p) {
                return Err(format!("{b} at {},{},{} cannot stand there", p.x, p.y, p.z));
            }
        }
        for _ in 0..200 {
            if w.idle() {
                break;
            }
            w.tick();
        }
        if !w.idle() {
            return Err("never goes still after placement".into());
        }
        if !w.errors.is_empty() || !w.drops.is_empty() {
            return Err(format!("broke at placement: {:?} {:?}", w.errors.first(), w.drops.first()));
        }
        Ok(w)
    }

    fn set_inputs(&self, w: &mut World, prev: Option<usize>, c: usize) {
        for (i, p) in self.inputs.iter().enumerate() {
            let on = (c >> i) & 1 == 1;
            if prev.is_some_and(|q| (q >> i) & 1 == 1) == on && prev.is_some() {
                continue;
            }
            w.set_block(bp(*p), if on { Block::RedstoneBlock } else { Block::Air }, flags::ALL);
        }
    }

    fn levels(&self, w: &World) -> Vec<u8> {
        self.outputs.iter().map(|p| w.best_neighbor_signal(bp(*p)).clamp(0, 15) as u8).collect()
    }

    fn lamps_lit(&self, w: &World) -> Vec<Option<bool>> {
        self.outputs
            .iter()
            .enumerate()
            .map(|(k, p)| {
                if !self.lamps[k] {
                    return None;
                }
                Some(matches!(w.get(bp(*p)), Block::Lamp { lit: true }))
            })
            .collect()
    }
}

/// One transition of the walk.
#[derive(Clone, Debug)]
pub struct Step {
    pub from: usize,
    pub to: usize,
    /// Frames from the input change to the last output change (0: none or instant).
    pub latency: u32,
    /// Output changes beyond the needed ones.
    pub glitches: usize,
}

/// The walk's result.
#[derive(Clone, Debug, Default)]
pub struct Run {
    pub pass: bool,
    pub why: String,
    pub steps: Vec<Step>,
    /// The worst latency, and the index of its step.
    pub latency: u32,
    pub worst: usize,
    pub glitches: usize,
    /// Per output, the worst latency of a transition where it changes.
    pub per_output: Vec<u32>,
}

/// Run the walk `seq` (states, from rest = state 0 placed), each state held `gap` frames.
pub fn run(rig: &Rig, f: &Func, seq: &[usize], gap: u32) -> Run {
    let mut w = match rig.world() {
        Ok(w) => w,
        Err(why) => return Run { why, ..Default::default() },
    };
    let mut out = Run { per_output: vec![0; f.outputs.len()], ..Default::default() };
    let mut prev_state: Option<usize> = None;
    let mut cur: Vec<bool> = rig.levels(&w).iter().map(|l| *l > 0).collect();
    // at rest (state 0, inputs off) the outputs must already read f(0)
    if seq.first() == Some(&0) && cur != f.table[0] {
        out.why = format!("at rest ({}) the outputs read {} instead of {}", f.label(0), bits(&cur), bits(&f.table[0]));
        return out;
    }
    for (k, &s) in seq.iter().enumerate() {
        if k == 0 && s == 0 {
            prev_state = Some(0);
            continue;
        }
        rig.set_inputs(&mut w, prev_state, s);
        let start: Vec<bool> = cur.clone();
        let mut last_change = vec![None::<u32>; f.outputs.len()];
        let mut changes = vec![0usize; f.outputs.len()];
        for t in 0..gap {
            if t > 0 {
                w.tick();
            }
            if !w.errors.is_empty() || !w.drops.is_empty() {
                out.why = format!("{} -> {}: broke ({:?} {:?})", f.label(prev_state.unwrap_or(0)), f.label(s), w.errors.first(), w.drops.first());
                return out;
            }
            let now: Vec<bool> = rig.levels(&w).iter().map(|l| *l > 0).collect();
            for o in 0..now.len() {
                if now[o] != cur[o] {
                    last_change[o] = Some(t);
                    changes[o] += 1;
                }
            }
            cur = now;
        }
        let from = prev_state.unwrap_or(0);
        let label = || format!("{} -> {}", f.label(from), f.label(s));
        if cur != f.table[s] {
            out.why = format!("{}: outputs {} instead of {}", label(), bits(&cur), bits(&f.table[s]));
            return out;
        }
        if let Some(t) = last_change.iter().flatten().max()
            && *t + (gap / 3).max(3) > gap
        {
            out.why = format!("{}: still changing at frame {t} of a {gap} gt hold", label());
            return out;
        }
        let lit = rig.lamps_lit(&w);
        for (o, l) in lit.iter().enumerate() {
            if let Some(l) = l
                && *l != f.table[s][o]
            {
                out.why = format!("{}: the lamp at output {} is {} (the output reads {}): it drives nothing", label(), f.outputs[o], if *l { "lit" } else { "off" }, f.table[s][o] as u8);
                return out;
            }
        }
        let lat = last_change.iter().flatten().max().copied().unwrap_or(0);
        let mut g = 0;
        for o in 0..cur.len() {
            let needed = (start[o] != cur[o]) as usize;
            g += changes[o].saturating_sub(needed);
            if needed == 1 {
                out.per_output[o] = out.per_output[o].max(last_change[o].unwrap_or(0));
            }
        }
        if lat > out.latency || out.steps.is_empty() {
            if lat > out.latency {
                out.worst = out.steps.len();
            }
            out.latency = out.latency.max(lat);
        }
        out.glitches += g;
        out.steps.push(Step { from, to: s, latency: lat, glitches: g });
        prev_state = Some(s);
    }
    out.pass = true;
    out
}

pub fn bits(v: &[bool]) -> String {
    v.iter().map(|b| if *b { '1' } else { '0' }).collect()
}

/// Load a design and its function (the `circuit:` line, or `kind` when given).
pub fn load(path: &std::path::Path, kind: Option<&str>) -> Result<(Design, Func, Rig)> {
    let d = Design::load(path)?;
    let k = match kind {
        Some(k) => k.to_string(),
        None => d.meta.get("circuit").cloned().with_context(|| format!("{}: no `circuit:` line (or pass --kind)", path.display()))?,
    };
    let ins: Vec<String> = d.ports.keys().filter_map(|k| k.strip_prefix("input.").map(|s| s.to_string())).collect();
    let outs: Vec<String> = d.ports.keys().filter_map(|k| k.strip_prefix("output.").map(|s| s.to_string())).collect();
    let f = Func::of(&k, &ins, &outs).with_context(|| path.display().to_string())?;
    let rig = Rig::from_design(&d, &f)?;
    Ok((d, f, rig))
}

/// The block count the judge reports: every design block except piston heads.
pub fn blocks(d: &Design) -> usize {
    d.blocks.values().filter(|s| !s.contains("piston_head")).count()
}

/// The design's box (blocks and ports) as `WxHxD`, and its height in blocks.
pub fn dims(rig: &Rig) -> (String, i32) {
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for (p, _) in &rig.blocks {
        let q = [p.x, p.y, p.z];
        for i in 0..3 {
            lo[i] = lo[i].min(q[i]);
            hi[i] = hi[i].max(q[i]);
        }
    }
    (format!("{}x{}x{}", hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1), hi[1] - lo[1] + 1)
}
