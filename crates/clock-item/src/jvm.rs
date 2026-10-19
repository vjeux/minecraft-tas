//! The real game (the bridge): place an item-clock design like the clock judge, record every
//! block state and every block entity's data after each tick, measure the period there, and
//! compare the trace with redsim tick for tick.

use crate::sim::{self, Spec};
use anyhow::{Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, SandboxOptions, Worker};
use redsim::BlockPos;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

/// One tick of the recorded box: block states (x fastest, then z, then y) and the block-entity
/// dump (`x y z type SNBT` lines).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Frame {
    pub blocks: Vec<String>,
    pub be: String,
}

/// The recorded box of a design: its blocks' bounding box plus [`sim::MARGIN`].
pub fn record_box(d: &Design) -> Result<(Pos, Pos)> {
    let Some((lo, hi)) = d.bounds() else { bail!("empty design") };
    let m = sim::MARGIN;
    Ok(([lo[0] - m, lo[1] - m, lo[2] - m], [hi[0] + m, hi[1] + m, hi[2] + m]))
}

/// The clock judge's placement in the game: raw paste, `nbt:` lines, wire shapes, self updates
/// (no settling).
pub fn place(w: &mut Worker, sb: u32, d: &Design) -> Result<()> {
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    mctas::design::apply_block_entities(w, sb, d)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    if !wires.is_empty() {
        w.update(sb, &wires, "shape")?;
    }
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    Ok(())
}

/// Place `d` in a fresh sandbox and record `ticks` ticks: frame 0 is right after placement.
pub fn record(w: &mut Worker, d: &Design, ticks: u32) -> Result<Vec<Frame>> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Vec<Frame>> {
        place(w, sb, d)?;
        let (lo, hi) = record_box(d)?;
        let first = w.get_region(sb, lo, hi)?;
        let be0 = w.block_entities(sb, lo, hi)?;
        let be0: Vec<String> = be0.iter().map(|b| format!("{} {} {} {} {}", b.pos[0], b.pos[1], b.pos[2], b.kind, b.nbt)).collect();
        let mut frames = vec![Frame { blocks: first.iter().map(|(_, s)| s.to_string()).collect(), be: be0.join("\n") }];
        let st = w.step_record_be(sb, ticks, false, lo, hi)?;
        for (f, be) in st.frames.iter().zip(st.block_entities.iter()) {
            frames.push(Frame { blocks: f.iter().map(|(_, s)| s.to_string()).collect(), be: be.clone() });
        }
        Ok(frames)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

fn key(f: &Frame) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    f.hash(&mut h);
    h.finish()
}

/// The period seen in a recording: the first repeat of (blocks, block entities) that keeps
/// repeating to the end of the recording (the game's pending ticks are not in a frame, so a
/// repeat is only trusted when the rest of the recording agrees). (period, transient).
pub fn period(frames: &[Frame]) -> Option<(u32, u32)> {
    let keys: Vec<u64> = frames.iter().map(key).collect();
    let mut seen: HashMap<u64, usize> = HashMap::new();
    for (t, k) in keys.iter().enumerate() {
        if let Some(&t0) = seen.get(k) {
            let p = t - t0;
            if (t..keys.len()).all(|i| keys[i] == keys[i - p]) && keys.len() >= t0 + 3 * p {
                return Some((p as u32, t0 as u32));
            }
        }
        seen.entry(*k).or_insert(t);
    }
    None
}

/// The same design in redsim: block states of the recorded box after each tick (frame 0 = after
/// placement). Err when redsim does not model it.
pub fn redsim_frames(d: &Design, ticks: u32) -> Result<Vec<Vec<String>>, String> {
    let spec = Spec::from_design(d)?;
    let mut w = sim::new_world(&spec);
    sim::place(&mut w, &spec)?;
    let (lo, hi) = record_box(d).map_err(|e| e.to_string())?;
    let snap = |w: &redsim::World| -> Vec<String> {
        let mut v = Vec::new();
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                for x in lo[0]..=hi[0] {
                    v.push(w.get(BlockPos::new(x, y, z)).to_string());
                }
            }
        }
        v
    };
    let mut out = vec![snap(&w)];
    for _ in 0..ticks {
        w.tick();
        if let Some(e) = sim::problem(&w) {
            return Err(e);
        }
        out.push(snap(&w));
    }
    Ok(out)
}

/// The first tick where the game and redsim disagree (a cell and both states), if any.
pub fn first_mismatch(d: &Design, game: &[Frame], model: &[Vec<String>]) -> Option<String> {
    let (lo, hi) = record_box(d).ok()?;
    let dx = (hi[0] - lo[0] + 1) as usize;
    let dz = (hi[2] - lo[2] + 1) as usize;
    for (t, (g, m)) in game.iter().zip(model.iter()).enumerate() {
        for (i, (a, b)) in g.blocks.iter().zip(m.iter()).enumerate() {
            if a != b {
                let x = lo[0] + (i % dx) as i32;
                let z = lo[2] + ((i / dx) % dz) as i32;
                let y = lo[1] + (i / (dx * dz)) as i32;
                return Some(format!("tick {t} at [{x}, {y}, {z}]: game {a}, redsim {b}"));
            }
        }
    }
    None
}

/// `d` turned `turns` quarter turns clockwise around the origin by the game's own rotation of
/// block states (positions (x, z) -> (-z, x)); contents stay as they are.
pub fn turned(w: &mut Worker, d: &Design, turns: u8) -> Result<Design> {
    if turns % 4 == 0 {
        return Ok(d.clone());
    }
    let rot = |p: Pos| {
        let mut q = p;
        for _ in 0..turns % 4 {
            q = [-q[2], q[1], q[0]];
        }
        q
    };
    let states: Vec<String> = d.blocks.values().cloned().collect();
    let t = w.transform_states(&states, turns % 4, false)?;
    let mut out = Design { name: d.name.clone(), meta: d.meta.clone(), ..Default::default() };
    for ((p, _), s) in d.blocks.iter().zip(t) {
        out.blocks.insert(rot(*p), s);
    }
    for (k, p) in &d.ports {
        out.ports.insert(k.clone(), rot(*p));
    }
    for (p, s) in &d.block_entities {
        out.block_entities.insert(rot(*p), s.clone());
    }
    Ok(out)
}
