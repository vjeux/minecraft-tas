//! Greedy block-count minimization: remove any block whose removal keeps the door working.

use crate::{VerifyOptions, block_count, verify_in};
use anyhow::Result;
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{Pool, Pos, SandboxOptions, Worker};
use std::sync::Mutex;

/// `d` without the block at `p` (an extended piston loses its head too).
pub fn without(d: &Design, p: Pos) -> Design {
    let mut out = d.clone();
    if let Some(s) = out.blocks.remove(&p) {
        if let Ok(b) = BlockState::parse(&s) {
            if b.get("extended") == Some("true") {
                let off = match b.get("facing") {
                    Some("north") => [0, 0, -1],
                    Some("south") => [0, 0, 1],
                    Some("east") => [1, 0, 0],
                    Some("west") => [-1, 0, 0],
                    Some("up") => [0, 1, 0],
                    _ => [0, -1, 0],
                };
                out.blocks.remove(&[p[0] + off[0], p[1] + off[1], p[2] + off[2]]);
            }
        }
    }
    out
}

/// Per-worker reusable sandboxes.
pub struct Sandboxes(Vec<Mutex<Option<u32>>>);

impl Sandboxes {
    pub fn new(pool: &Pool) -> Sandboxes {
        Sandboxes((0..pool.len()).map(|_| Mutex::new(None)).collect())
    }
    pub fn get(&self, w: &mut Worker, wi: usize) -> Result<u32> {
        let mut slot = self.0[wi].lock().unwrap();
        if let Some(sb) = *slot {
            return Ok(sb);
        }
        let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        *slot = Some(sb);
        Ok(sb)
    }
}

/// Repeatedly try every single-block removal in parallel and keep those that still verify.
pub fn minimize(pool: &Pool, d: &Design, opts: &VerifyOptions, log: &dyn Fn(&str)) -> Result<Design> {
    let sandboxes = Sandboxes::new(pool);
    let lever = d.ports.get("lever").copied();
    let mut cur = d.clone();
    loop {
        let cands: Vec<Pos> = cur
            .blocks
            .iter()
            .filter(|(p, s)| !s.starts_with("minecraft:piston_head") && Some(**p) != lever)
            .map(|(p, _)| *p)
            .collect();
        let snapshot = cur.clone();
        let results = pool.map(cands.clone(), |w, wi, p| {
            let sb = sandboxes.get(w, wi)?;
            Ok(verify_in(w, sb, &without(&snapshot, p), opts).is_ok())
        });
        let ok: Vec<Pos> = cands.into_iter().zip(results).filter(|(_, r)| matches!(r, Ok(true))).map(|(p, _)| p).collect();
        log(&format!("{} blocks: {} single removals keep it working", block_count(&cur), ok.len()));
        if ok.is_empty() {
            break;
        }
        let mut progressed = false;
        let mut w = pool.worker(0);
        let sb = sandboxes.get(&mut w, 0)?;
        for p in ok {
            if !cur.blocks.contains_key(&p) {
                continue;
            }
            let cand = without(&cur, p);
            if verify_in(&mut w, sb, &cand, opts).is_ok() {
                log(&format!("  removed {p:?} {}", snapshot.blocks[&p]));
                cur = cand;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    Ok(cur)
}
