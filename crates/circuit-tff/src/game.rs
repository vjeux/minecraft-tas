//! The real-game runner (a stopgap until clockinfra's `mctas circuit verify` lands): the circuit
//! built like the clock judge builds a clock (raw paste, `nbt:` merge, wire shapes, self updates)
//! at the judge's positions x 4 rotations, inputs driven between ticks, the best signal into each
//! output cell recorded every gt (output cells must be air or plain blocks here).

use crate::model::{Circuit, Drive, Pos};
use crate::rs::Event;
use anyhow::{Context, Result, bail};
use mctas::sim::{Pool, SandboxOptions, Worker, WorkerConfig};

pub fn spawn_pool(workers: usize, version: &str) -> Result<Pool> {
    Pool::spawn(workers.max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })
}

/// One build (turned `turns` quarter turns, moved by `off`): the output levels `[output][frame]`.
pub fn run_build(w: &mut Worker, c: &Circuit, events: &[Event], frames: u32, turns: u8, off: Pos, margin: i32) -> Result<Vec<Vec<u8>>> {
    let clock = clock::Clock::from_design(c.design.clone())?;
    let rc = clock::verify::rotated(w, &clock, turns)?.translated(off);
    let cc = Circuit::from_design(rc.design.clone())?;
    for (n, p) in &cc.outputs {
        if let Some(s) = cc.design.blocks.get(p) {
            let name = mctas::blockstate::block_name(s);
            if ["redstone_wire", "repeater", "comparator", "redstone_torch", "redstone_wall_torch", "observer"].iter().any(|k| name.ends_with(k)) {
                bail!("output{}: a {} there reads its own output (not supported by this stopgap runner)", if n.is_empty() { String::new() } else { format!(".{n}") }, name);
            }
        }
    }
    let (lo, hi) = cc.bounds();
    let from = [lo[0] - margin, lo[1] - margin, lo[2] - margin];
    let to = [hi[0] + margin, hi[1] + margin, hi[2] + margin];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = (|| -> Result<Vec<Vec<u8>>> {
        clock::verify::place(w, sb, &rc)?;
        let probes: Vec<Pos> = cc.outputs.iter().map(|(_, p)| *p).collect();
        let mut ev: Vec<Event> = events.to_vec();
        ev.sort_by_key(|e| e.t);
        let mut levels: Vec<Vec<u8>> = vec![Vec::new(); probes.len()];
        let mut k = 0;
        let mut t = 0;
        while t < frames {
            while k < ev.len() && ev[k].t == t {
                let e = ev[k];
                let i = &cc.inputs[e.input];
                match &i.drive {
                    Drive::Block => {
                        let a = if e.on {
                            clock::Action::Place { pos: i.pos, state: "minecraft:redstone_block".into(), nbt: None }
                        } else {
                            clock::Action::Clear { pos: i.pos }
                        };
                        clock::verify::apply(w, sb, &a)?;
                    }
                    Drive::Lever => {
                        w.use_block(sb, i.pos)?;
                    }
                    Drive::Button => {
                        if e.on {
                            w.use_block(sb, i.pos)?;
                        }
                    }
                    Drive::States { .. } => bail!("custom input states are not supported by this stopgap runner"),
                }
                k += 1;
            }
            // frame t, then step to the next event (or the end), probing every tick
            let now = w.signals(sb, &probes)?;
            for (o, (best, _)) in now.iter().enumerate() {
                levels[o].push(*best);
            }
            let next = ev.get(k).map(|e| e.t).unwrap_or(frames).min(frames);
            let n = next.saturating_sub(t);
            if n == 0 {
                break;
            }
            // `step_probe` returns the probes after each of the n ticks: frames t+1..=t+n
            let st = w.step_probe(sb, n, false, &probes, None)?;
            let got = st.signals.len() as u32;
            for (j, sg) in st.signals.iter().enumerate() {
                let f = t + 1 + j as u32;
                if f >= next {
                    // frame `next` is re-read after its inputs are applied
                    break;
                }
                for (o, lvl) in sg.iter().step_by(2).enumerate() {
                    levels[o].push(*lvl);
                }
            }
            if got < n {
                bail!("the game stepped {got} of {n} ticks");
            }
            t = next;
        }
        let ents = w.entities(sb)?;
        if !ents.is_empty() {
            bail!("entities at the end: {} (a drop or a popped block)", ents.iter().map(|e| e.kind.clone()).collect::<Vec<_>>().join(", "));
        }
        Ok(levels)
    })();
    let _ = w.drop_sandbox(sb);
    r.with_context(|| format!("rotation {turns} at {off:?}"))
}

/// Every build of the judge's grid: (turns, offset, levels or error).
pub fn run_all(pool: &Pool, c: &Circuit, events: &[Event], frames: u32, positions: usize, rotations: bool) -> Vec<(u8, Pos, Result<Vec<Vec<u8>>>)> {
    let offsets = clock::offsets(positions);
    let turns: Vec<u8> = if rotations { vec![0, 1, 2, 3] } else { vec![0] };
    let jobs: Vec<(u8, Pos)> = turns.iter().flat_map(|t| offsets.iter().map(move |o| (*t, *o))).collect();
    let res = pool.map(jobs.clone(), |w, _, (t, off)| run_build(w, c, events, frames, t, off, 3));
    jobs.into_iter().zip(res).map(|((t, o), r)| (t, o, r)).collect()
}
