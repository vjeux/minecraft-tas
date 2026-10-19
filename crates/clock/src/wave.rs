//! Period, waveform and family from a run's states: shared by the exact redsim detector
//! ([`crate::sim`]) and the real-game judge ([`crate::verify`]), so both measure the same way.

use crate::Pos;
use std::collections::HashMap;

/// Is a component "on" in this state? (the waveform's on-ticks and rising edges)
///
/// Pistons: extended; a piston head or a moving block: present; sculk sensors: ACTIVE; hoppers:
/// locked (`enabled=false`); doors, trapdoors, fence gates: open; dispensers, droppers, crafters:
/// triggered; leaves: distance < 7; scaffolding: distance > 0; else `lit`, then `power > 0`, then
/// `powered`; a block with none of these: present (not air).
pub fn active(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    if matches!(name, "air" | "cave_air" | "void_air") {
        return false;
    }
    let prop = |k: &str| -> Option<&str> {
        let inner = state.split_once('[')?.1.trim_end_matches(']');
        inner.split(',').find_map(|kv| kv.split_once('=').filter(|(kk, _)| kk.trim() == k).map(|(_, v)| v.trim()))
    };
    let is = |k: &str, v: &str| prop(k) == Some(v);
    match name {
        "piston" | "sticky_piston" => return is("extended", "true"),
        "piston_head" | "moving_piston" => return true,
        "sculk_sensor" | "calibrated_sculk_sensor" => return is("sculk_sensor_phase", "active"),
        "hopper" => return is("enabled", "false"),
        "dispenser" | "dropper" | "crafter" => return is("triggered", "true"),
        "oak_leaves" => return prop("distance").and_then(|d| d.parse::<u8>().ok()).is_some_and(|d| d < 7),
        "scaffolding" => return prop("distance").and_then(|d| d.parse::<u8>().ok()).is_some_and(|d| d > 0),
        _ => {}
    }
    if name.ends_with("_door") || name.ends_with("_trapdoor") || name.ends_with("_fence_gate") {
        return is("open", "true");
    }
    if let Some(v) = prop("lit") {
        return v == "true";
    }
    if let Some(v) = prop("power") {
        return v.parse::<u8>().is_ok_and(|p| p > 0);
    }
    if let Some(v) = prop("powered") {
        return v == "true";
    }
    true
}

/// A block state's name in a family signature, or `None` for blocks with no behavior of their
/// own (plain blocks, slabs, logs...: they only move or hold things up) and air. Piston heads and
/// moving blocks count as their piston (`type=sticky` -> `sticky_piston`), wall torches as
/// torches, every waxed copper bulb as `copper_bulb`, wall skulls as skulls.
pub fn family_name(state: &str) -> Option<String> {
    let name = mctas::blockstate::block_name(state);
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    match name {
        "air" | "cave_air" | "void_air" => return None,
        // a moving block says nothing about which piston moves it (`type=normal` is any moved
        // block): the piston's own base cell always changes too and names it
        "moving_piston" => return None,
        "piston_head" => {
            return Some(if state.contains("type=sticky") { "sticky_piston" } else { "piston" }.into());
        }
        "redstone_wall_torch" => return Some("redstone_torch".into()),
        "skeleton_wall_skull" => return Some("skeleton_skull".into()),
        _ => {}
    }
    if name.contains("copper_bulb") {
        return Some("copper_bulb".into());
    }
    match redsim::Block::parse(state) {
        Ok(redsim::Block::Simple(_) | redsim::Block::Glazed { .. } | redsim::Block::Slab { .. } | redsim::Block::Log { .. }) => None,
        // modeled behavior blocks and blocks redsim does not know: their own name
        _ => Some(name.to_string()),
    }
}

/// On-ticks, rising edges and pulse widths of a cyclic boolean sequence (one period). A
/// sequence that is always on has one pulse of the whole period and no rising edge.
pub fn bool_wave(seq: &[bool]) -> (u32, u32, Vec<u32>) {
    let n = seq.len();
    let on = seq.iter().filter(|b| **b).count() as u32;
    if n == 0 || on == 0 {
        return (0, 0, Vec::new());
    }
    if on as usize == n {
        return (on, 0, vec![on]);
    }
    let mut edges = 0;
    let mut widths = Vec::new();
    // start at a rising edge
    let start = (0..n).find(|&i| seq[i] && !seq[(i + n - 1) % n]).unwrap();
    let mut run = 0u32;
    for k in 0..n {
        let i = (start + k) % n;
        if seq[i] {
            if run == 0 {
                edges += 1;
            }
            run += 1;
        } else if run > 0 {
            widths.push(run);
            run = 0;
        }
    }
    if run > 0 {
        widths.push(run);
    }
    (on, edges, widths)
}

/// The waveform of one cell over one period.
#[derive(Clone, Debug)]
pub struct CellWave {
    pub pos: Pos,
    /// The states the cell takes in the period, in order of first appearance (shortened).
    pub states: Vec<String>,
    /// Family names of those states ([`family_name`]).
    pub kinds: Vec<String>,
    pub on: u32,
    pub edges: u32,
    pub widths: Vec<u32>,
}

impl CellWave {
    /// `on 2/6, 1 edge, pulses 2`.
    pub fn describe(&self, period: u32) -> String {
        format!("on {}/{period}, {} edge{}, pulses {:?}", self.on, self.edges, if self.edges == 1 { "" } else { "s" }, self.widths)
    }
}

/// The analysis of one period: every cell whose state changes, the family (sorted, deduplicated)
/// and the output's waveform.
#[derive(Clone, Debug, Default)]
pub struct PeriodAnalysis {
    pub cells: Vec<CellWave>,
    pub family: Vec<String>,
}

/// Analyze one period: `states[t][i]` = the state of `cells[i]` at tick `W + t` (t in 0..P).
pub fn analyze(cells: &[Pos], states: &[Vec<&str>]) -> PeriodAnalysis {
    let mut out = PeriodAnalysis::default();
    let p = states.len();
    if p == 0 {
        return out;
    }
    let mut fam: Vec<String> = Vec::new();
    for (i, pos) in cells.iter().enumerate() {
        let first = states[0][i];
        if states.iter().all(|s| s[i] == first) {
            continue;
        }
        let mut seen: Vec<String> = Vec::new();
        for s in states {
            let st = crate::short(s[i]).to_string();
            if !seen.contains(&st) {
                seen.push(st);
            }
        }
        let mut kinds: Vec<String> = seen.iter().filter_map(|s| family_name(s)).collect();
        kinds.sort();
        kinds.dedup();
        let seq: Vec<bool> = states.iter().map(|s| active(s[i])).collect();
        let (on, edges, widths) = bool_wave(&seq);
        fam.extend(kinds.iter().cloned());
        out.cells.push(CellWave { pos: *pos, states: seen, kinds, on, edges, widths });
    }
    fam.sort();
    fam.dedup();
    out.family = fam;
    out
}

/// The family signature: `observer+sticky_piston`, or `-` for none.
pub fn signature(family: &[String]) -> String {
    if family.is_empty() { "-".into() } else { family.join("+") }
}

/// The smallest period of a sequence of frame ids that is periodic from some warmup on, seen in
/// `frames[start..]`: for every candidate p, `W_p` = the earliest tick from which
/// `frames[t] == frames[t + p]` holds to the end; candidates need at least `min_periods` full
/// periods after `W_p`; the answer is the candidate with the smallest `W_p`, then the smallest p
/// (a multiple of the true period never starts earlier, and a false period that only holds on a
/// quiet tail starts later). Returns (warmup, period).
pub fn detect_period(frames: &[u64], start: usize, min_periods: usize) -> Option<(usize, usize)> {
    let n = frames.len();
    if n <= start + 1 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let maxp = (n - start) / min_periods.max(1);
    for p in 1..=maxp {
        // earliest W such that frames[t] == frames[t+p] for all t in W..n-p
        let mut w = n - p;
        while w > start && frames[w - 1] == frames[w - 1 + p] {
            w -= 1;
        }
        if n - w < min_periods * p {
            continue;
        }
        if best.is_none_or(|(bw, bp)| (w, p) < (bw, bp)) {
            best = Some((w, p));
        }
    }
    best
}

/// Interns frames (vectors of state ids) and states (strings) into dense ids.
#[derive(Default)]
pub struct Interner {
    pub states: Vec<String>,
    state_ids: HashMap<String, u32>,
    frame_ids: HashMap<Vec<u32>, u64>,
}

impl Interner {
    pub fn state(&mut self, s: &str) -> u32 {
        if let Some(i) = self.state_ids.get(s) {
            return *i;
        }
        let i = self.states.len() as u32;
        self.states.push(s.to_string());
        self.state_ids.insert(s.to_string(), i);
        i
    }
    pub fn frame(&mut self, f: &[u32]) -> u64 {
        let n = self.frame_ids.len() as u64;
        *self.frame_ids.entry(f.to_vec()).or_insert(n)
    }
    pub fn name(&self, id: u32) -> &str {
        &self.states[id as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves() {
        assert_eq!(bool_wave(&[true, true, false, false, false, false]), (2, 1, vec![2]));
        assert_eq!(bool_wave(&[false, true, false, true]), (2, 2, vec![1, 1]));
        assert_eq!(bool_wave(&[true, false, false, true]), (2, 1, vec![2]));
        assert_eq!(bool_wave(&[true, true]), (2, 0, vec![2]));
        assert_eq!(bool_wave(&[false, false]), (0, 0, vec![]));
    }

    #[test]
    fn periods() {
        // warmup 3, period 4
        let f: Vec<u64> = vec![9, 8, 7, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2, 3, 4, 1, 2];
        assert_eq!(detect_period(&f, 0, 3), Some((3, 4)));
        // a quiet tail does not fake period 1
        let mut g: Vec<u64> = Vec::new();
        for _ in 0..5 {
            g.extend([1, 2, 3]);
            g.extend([0; 20]);
        }
        assert_eq!(detect_period(&g, 0, 3), Some((0, 23)));
        assert_eq!(detect_period(&[5, 5, 5, 5], 0, 3), Some((0, 1)));
    }

    #[test]
    fn activity_and_family() {
        assert!(active("minecraft:observer[facing=up,powered=true]"));
        assert!(!active("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]"));
        assert!(active("minecraft:sticky_piston[extended=true,facing=up]"));
        assert!(!active("minecraft:air"));
        assert!(active("minecraft:stone"));
        assert_eq!(family_name("minecraft:moving_piston[facing=up,type=sticky]"), None);
        assert_eq!(family_name("minecraft:piston_head[facing=up,short=false,type=sticky]").as_deref(), Some("sticky_piston"));
        assert_eq!(family_name("minecraft:stone"), None);
        assert_eq!(family_name("minecraft:redstone_wall_torch[facing=north,lit=true]").as_deref(), Some("redstone_torch"));
        assert_eq!(family_name("minecraft:waxed_copper_bulb[lit=true,powered=false]").as_deref(), Some("copper_bulb"));
    }
}
