//! `mctas circuit tnt precision`: where the primed TNT go (the PRECISE and direction variations,
//! designs/circuits/tnt/VARIATIONS.md). The design runs in the real game; every primed TNT is
//! followed from its first sighting (one tick after it spawned, fuse 79) to fuse 1 (the tick
//! before it would explode) and killed there, so nothing explodes. Reported over the TNT that
//! reached fuse 1: the spread (max - min) of position and motion per axis at both moments, the
//! mean motion, and the direction class: stationary (at rest at fuse 1), dropped (falling), or
//! launched (moving sideways).

use anyhow::Result;
use clock::{Clock, Pos};
use mctas::sim::{SandboxOptions, Worker};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub first_tick: u32,
    pub first_pos: [f64; 3],
    pub first_motion: [f64; 3],
    pub end: Option<([f64; 3], [f64; 3])>,
}

fn fuse_of(state: &str) -> Option<i32> {
    let i = state.find("fuse: ")? + 6;
    let rest = &state[i..];
    let n: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
    n.parse().ok()
}

/// Follow up to `n` primed TNT (the run ends when they all reached fuse 1, or after `max_ticks`).
/// `extra`: input actions (a triggered duper's pulses) besides the design's own kick.
pub fn follow(w: &mut Worker, c: &Clock, extra: &[(u32, clock::Action)], n: usize, max_ticks: u32, margin: i32) -> Result<Vec<Track>> {
    let (lo, hi) = c.bounds();
    let m = margin.max(8);
    let from: Pos = [lo[0] - m, lo[1] - m, lo[2] - m];
    let to: Pos = [hi[0] + m, hi[1] + m, hi[2] + m];
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: 0 })?;
    let r = (|| -> Result<Vec<Track>> {
        clock::verify::place(w, sb, c)?;
        let mut actions = c.actions();
        actions.extend(extra.iter().cloned());
        actions.sort_by_key(|(t, _)| *t);
        let mut tracks: HashMap<String, Track> = HashMap::new();
        let mut order: Vec<String> = Vec::new();
        for t in 0..max_ticks {
            while actions.first().is_some_and(|(at, _)| *at == t) {
                let (_, a) = actions.remove(0);
                clock::verify::apply(w, sb, &a)?;
            }
            w.step(sb, 1)?;
            for e in w.entities(sb)? {
                if e.kind.trim_start_matches("minecraft:") != "tnt" {
                    continue;
                }
                let fuse = fuse_of(&e.state).unwrap_or(80);
                if !tracks.contains_key(&e.uuid) {
                    if order.len() >= n {
                        // enough followed: later ones are removed at once
                        w.exec(sb, &format!("kill {}", e.uuid))?;
                        continue;
                    }
                    order.push(e.uuid.clone());
                    tracks.insert(e.uuid.clone(), Track { first_tick: t + 1, first_pos: e.pos, first_motion: e.motion, end: None });
                }
                if fuse <= 1 {
                    if let Some(tr) = tracks.get_mut(&e.uuid) {
                        tr.end = Some((e.pos, e.motion));
                    }
                    w.exec(sb, &format!("kill {}", e.uuid))?;
                }
            }
            if order.len() >= n && order.iter().all(|u| tracks[u].end.is_some()) {
                break;
            }
        }
        Ok(order.iter().map(|u| tracks[u].clone()).collect())
    })();
    let _ = w.drop_sandbox(sb);
    r
}

fn stats(v: &[[f64; 3]]) -> ([f64; 3], [f64; 3]) {
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    let mut mean = [0.0; 3];
    for p in v {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
            mean[i] += p[i] / v.len() as f64;
        }
    }
    ([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]], mean)
}

/// The report lines for the followed TNT.
pub fn report(tracks: &[Track]) -> Vec<String> {
    let done: Vec<&Track> = tracks.iter().filter(|t| t.end.is_some()).collect();
    let mut out = vec![format!("{} primed TNT followed, {} reached fuse 1", tracks.len(), done.len())];
    if done.is_empty() {
        return out;
    }
    let f = |v: [f64; 3]| format!("({:.4}, {:.4}, {:.4})", v[0], v[1], v[2]);
    let (sp, mp) = stats(&done.iter().map(|t| t.first_pos).collect::<Vec<_>>());
    let (sm, mm) = stats(&done.iter().map(|t| t.first_motion).collect::<Vec<_>>());
    out.push(format!("at first sight: position spread {} (mean {}), motion spread {} (mean {})", f(sp), f(mp), f(sm), f(mm)));
    let (ep, epm) = stats(&done.iter().map(|t| t.end.unwrap().0).collect::<Vec<_>>());
    let (em, emm) = stats(&done.iter().map(|t| t.end.unwrap().1).collect::<Vec<_>>());
    out.push(format!("at fuse 1: position spread {} (mean {}), motion spread {} (mean {})", f(ep), f(epm), f(em), f(emm)));
    let fall = epm[1] - mp[1];
    let side = ((epm[0] - mp[0]).powi(2) + (epm[2] - mp[2]).powi(2)).sqrt();
    let class = if emm.iter().all(|v| v.abs() < 1e-3) && side < 0.5 && fall.abs() < 0.5 {
        "stationary"
    } else if side >= 1.0 {
        "launched"
    } else if fall <= -1.0 {
        "dropped"
    } else {
        "drifting"
    };
    let precise = ep.iter().all(|v| *v < 1e-9);
    out.push(format!("direction: {class} (moved {side:.2} sideways, {fall:.2} up by fuse 1); {}", if precise { "PRECISE: every TNT at the same point at fuse 1" } else { "not precise" }));
    out
}
