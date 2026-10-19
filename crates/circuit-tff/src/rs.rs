//! The redsim circuit runner: the judge's placement (raw paste, `nbt:` lines, wire shapes, self
//! updates), inputs driven between ticks, every output's level recorded each gt (frame t = the
//! state after t ticks, with the inputs due at t applied).

use crate::model::{Circuit, Drive, Pos};
use redsim::world::flags;
use redsim::{Block, BlockPos, World};

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// One input change, applied between ticks before frame `t` is recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub t: u32,
    pub input: usize,
    pub on: bool,
}

/// An input with its parsed states.
#[derive(Clone, Debug)]
struct In {
    pos: BlockPos,
    kind: InKind,
}

#[derive(Clone, Debug)]
enum InKind {
    States { on: Block, off: Block },
    Use,
    Button,
}

/// A circuit ready to run many times (blocks parsed once).
#[derive(Clone, Debug)]
pub struct Prepared {
    blocks: Vec<(BlockPos, Block)>,
    be: Vec<(BlockPos, String)>,
    inputs: Vec<In>,
    pub outputs: Vec<BlockPos>,
    min: BlockPos,
    size: [i32; 3],
}

impl Prepared {
    pub fn new(c: &Circuit, margin: i32) -> Result<Prepared, String> {
        let mut blocks = Vec::new();
        for (p, s) in &c.design.blocks {
            blocks.push((bp(*p), Block::parse(s)?));
        }
        let mut inputs = Vec::new();
        for i in &c.inputs {
            let kind = match &i.drive {
                Drive::Block => InKind::States { on: Block::RedstoneBlock, off: Block::Air },
                Drive::States { on, off } => InKind::States { on: Block::parse(on)?, off: Block::parse(off)? },
                Drive::Lever => InKind::Use,
                Drive::Button => InKind::Button,
            };
            inputs.push(In { pos: bp(i.pos), kind });
        }
        let (lo, hi) = c.bounds();
        Ok(Prepared {
            blocks,
            be: c.design.block_entities.iter().map(|(p, s)| (bp(*p), s.clone())).collect(),
            inputs,
            outputs: c.outputs.iter().map(|(_, p)| bp(*p)).collect(),
            min: BlockPos::new(lo[0] - margin, lo[1] - margin, lo[2] - margin),
            size: [hi[0] - lo[0] + 1 + 2 * margin, hi[1] - lo[1] + 1 + 2 * margin, hi[2] - lo[2] + 1 + 2 * margin],
        })
    }

    /// Build the world (the judge's placement).
    pub fn start(&self) -> Result<World, String> {
        let mut w = World::new(self.min, self.size);
        w.place_raw(&self.blocks);
        redsim::redstone::container::apply_block_entities(&mut w, self.be.iter().map(|(p, s)| (*p, s.as_str()))).map_err(|e| format!("nbt: {e}"))?;
        w.place_updates(&self.blocks);
        Ok(w)
    }

    fn apply(&self, w: &mut World, e: &Event) -> Result<(), String> {
        let i = &self.inputs[e.input];
        match &i.kind {
            InKind::States { on, off } => {
                // the judge's input device: the cell holds the off state whenever the input is off
                // and the on state stays put while on
                let want = if e.on { *off } else { *on };
                if w.get(i.pos) != want {
                    return Err(format!("tick {}: the input cell holds {} (expected {want}): something moved into it or pushed the input block", e.t, w.get(i.pos)));
                }
                w.set_block(i.pos, if e.on { *on } else { *off }, flags::ALL);
            }
            InKind::Use => {
                if !w.use_block(i.pos) {
                    return Err(format!("a right-click on {} is not modeled", w.get(i.pos)));
                }
            }
            InKind::Button => {
                if e.on && !w.use_block(i.pos) {
                    return Err(format!("a right-click on {} is not modeled", w.get(i.pos)));
                }
            }
        }
        Ok(())
    }

    /// [`Prepared::run`] plus, per frame, the cells that changed since the previous frame (state
    /// and comparator strength / container contents), as text.
    pub fn run_traced(&self, events: &[Event], frames: u32) -> Result<(Vec<Vec<u8>>, Vec<String>), String> {
        let mut w = self.start()?;
        let mut ev: Vec<Event> = events.to_vec();
        ev.sort_by_key(|e| e.t);
        let mut k = 0;
        let mut levels = vec![Vec::with_capacity(frames as usize); self.outputs.len()];
        let mut texts = Vec::new();
        let snap = |w: &World| -> std::collections::BTreeMap<BlockPos, String> {
            w.blocks()
                .into_iter()
                .map(|(p, b)| {
                    let mut s = b.to_string().replace("minecraft:", "");
                    if let Some(o) = w.comparator_out.get(&p) {
                        s.push_str(&format!("{{out={o}}}"));
                    }
                    if redsim::redstone::container::is_container_block(&b) {
                        s.push_str(&format!("{{a={}}}", redsim::redstone::container::analog_output(w, p)));
                    }
                    (p, s)
                })
                .collect()
        };
        let mut prev = snap(&w);
        for t in 0..frames {
            while k < ev.len() && ev[k].t == t {
                self.apply(&mut w, &ev[k])?;
                k += 1;
            }
            if let Some(e) = w.errors.first() {
                return Err(format!("tick {t}: {e}"));
            }
            for (o, p) in self.outputs.iter().enumerate() {
                levels[o].push(level(&w, *p));
            }
            let now = snap(&w);
            let mut ch = Vec::new();
            for (p, s) in &now {
                if prev.get(p) != Some(s) {
                    ch.push(format!("{},{},{} {s}", p.x, p.y, p.z));
                }
            }
            for p in prev.keys() {
                if !now.contains_key(p) {
                    ch.push(format!("{},{},{} air", p.x, p.y, p.z));
                }
            }
            texts.push(ch.join("; "));
            prev = now;
            w.tick();
        }
        Ok((levels, texts))
    }

    /// Frames 0..frames: the inputs due at t applied, every output's level recorded, one tick.
    /// `levels[o][t]`.
    pub fn run(&self, events: &[Event], frames: u32) -> Result<Vec<Vec<u8>>, String> {
        let mut w = self.start()?;
        let mut ev: Vec<Event> = events.to_vec();
        ev.sort_by_key(|e| e.t);
        let mut k = 0;
        let mut levels = vec![Vec::with_capacity(frames as usize); self.outputs.len()];
        for t in 0..frames {
            while k < ev.len() && ev[k].t == t {
                self.apply(&mut w, &ev[k])?;
                k += 1;
            }
            if let Some(e) = w.errors.first() {
                return Err(format!("tick {t}: {e}"));
            }
            if let Some((p, b)) = w.drops.first() {
                return Err(format!("tick {t}: {b} at {p} dropped as an item"));
            }
            for (o, p) in self.outputs.iter().enumerate() {
                levels[o].push(level(&w, *p));
            }
            w.tick();
        }
        Ok(levels)
    }
}

/// The judge's output level of a cell: dust its power; repeater, comparator, torch, observer
/// their own output (comparator: its strength); any other cell the best signal into it.
pub fn level(w: &World, p: BlockPos) -> u8 {
    match w.get(p) {
        Block::Wire { power, .. } => power,
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
        Block::Comparator { .. } => w.comparator_out.get(&p).copied().unwrap_or(0),
        _ => w.best_neighbor_signal(p).clamp(0, 15) as u8,
    }
}

/// A pulse train on input `input`: pulses of `len` gt starting at `start + i * period`.
pub fn pulses(input: usize, start: u32, len: u32, period: u32, count: u32) -> Vec<Event> {
    let mut v = Vec::new();
    for i in 0..count {
        let t = start + i * period;
        v.push(Event { t, input, on: true });
        v.push(Event { t: t + len, input, on: false });
    }
    v
}
