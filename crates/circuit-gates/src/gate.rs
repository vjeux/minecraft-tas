//! The gate checker (track gates): every ordered transition between input combinations (an
//! Eulerian walk: 2 for NOT, all 12 for two inputs, both inputs changing at once included), each
//! combination held `hold` gt; the output must settle to f(inputs) and stay. Latency = the worst gt
//! from a change to the output's final value; glitches = holds where the output moves more than
//! once (or moves although f did not change); latency 0 = the instant class.

use circuit_delay::harness::{Change, Circuit, run};

pub const WARM: u32 = 40;

/// The 12-edge Eulerian walk over the 4 combinations (bit 0 = input a, bit 1 = input b).
pub const WALK2: [u8; 13] = [0, 1, 0, 2, 0, 3, 1, 2, 1, 3, 2, 3, 0];
pub const WALK1: [u8; 5] = [0, 1, 0, 1, 0];

#[derive(Clone, Debug, Default)]
pub struct GateReport {
    /// GATE, NOT-A-GATE (an output that does not settle), ERROR.
    pub verdict: String,
    /// The truth table, index = a | b << 1.
    pub table: Vec<bool>,
    pub function: String,
    pub latency: u32,
    pub glitches: u32,
    pub blocks: usize,
    pub dims: [i32; 3],
    pub why: String,
}

impl GateReport {
    pub fn line(&self, name: &str) -> String {
        let d = self.dims;
        match self.verdict.as_str() {
            "GATE" => format!(
                "GATE: {name} {} latency {} gt{}, glitches {}, {} blocks, {}x{}x{}",
                self.function,
                self.latency,
                if self.latency == 0 { " (instant)" } else { "" },
                self.glitches,
                self.blocks,
                d[0],
                d[1],
                d[2]
            ),
            v => format!("{v}: {name} {}", self.why),
        }
    }
}

/// The function's name from its truth table (index = a | b << 1).
pub fn function_name(t: &[bool]) -> String {
    let bits: Vec<u8> = t.iter().map(|b| *b as u8).collect();
    match bits.as_slice() {
        [1, 0] => "NOT".into(),
        [0, 1] => "BUFFER".into(),
        [0, 1, 1, 1] => "OR".into(),
        [0, 0, 0, 1] => "AND".into(),
        [0, 1, 1, 0] => "XOR".into(),
        [1, 0, 0, 1] => "XNOR".into(),
        [1, 1, 1, 0] => "NAND".into(),
        [1, 0, 0, 0] => "NOR".into(),
        // a -> b = !a | b: f(a=1, b=0) = 0
        [1, 0, 1, 1] => "IMPLY (a -> b)".into(),
        [1, 1, 0, 1] => "IMPLY (b -> a)".into(),
        [0, 1, 0, 0] => "NIMPLY (a & !b)".into(),
        [0, 0, 1, 0] => "NIMPLY (b & !a)".into(),
        [0, 1, 0, 1] => "BUFFER a".into(),
        [0, 0, 1, 1] => "BUFFER b".into(),
        [1, 0, 1, 0] => "NOT a".into(),
        [1, 1, 0, 0] => "NOT b".into(),
        [0, 0, 0, 0] => "FALSE".into(),
        [1, 1, 1, 1] => "TRUE".into(),
        _ => format!("{bits:?}"),
    }
}

/// The stimulus: (changes, frames, walk).
pub fn stimulus(n_in: usize, hold: u32) -> Option<(Vec<Change>, u32, Vec<u8>)> {
    let walk: Vec<u8> = match n_in {
        1 => WALK1.to_vec(),
        2 => WALK2.to_vec(),
        _ => return None,
    };
    let mut changes = Vec::new();
    let mut prev = 0u8;
    for (k, &comb) in walk.iter().enumerate() {
        let at = WARM + k as u32 * hold;
        for i in 0..n_in {
            if k > 0 && (prev >> i & 1) != (comb >> i & 1) {
                changes.push(Change { at, input: i, on: comb >> i & 1 == 1 });
            }
        }
        prev = comb;
    }
    Some((changes, WARM + walk.len() as u32 * hold, walk))
}

pub fn analyze(c: &Circuit, hold: u32) -> GateReport {
    let n_in = c.inputs.len();
    let mut rep = match stimulus(n_in, hold) {
        None => GateReport { verdict: "ERROR".into(), why: format!("{n_in} inputs (1 or 2)"), ..Default::default() },
        Some((changes, frames, walk)) => match run(c, &changes, frames, 4) {
            Ok(tr) => analyze_levels(n_in, &walk, hold, &tr.out()),
            Err(e) => GateReport { verdict: "ERROR".into(), why: e, ..Default::default() },
        },
    };
    rep.blocks = c.blocks();
    rep.dims = c.dims();
    rep
}

/// The verdict for one output waveform `out` under [`stimulus`].
pub fn analyze_levels(n_in: usize, walk: &[u8], hold: u32, out: &[bool]) -> GateReport {
    let mut rep = GateReport::default();
    let mut table: Vec<Option<bool>> = vec![None; 1 << n_in];
    let mut latency = 0u32;
    let mut glitches = 0u32;
    let mut before = out[WARM as usize - 1];
    for (k, &comb) in walk.iter().enumerate() {
        let a = (WARM + k as u32 * hold) as usize;
        let b = a + hold as usize;
        let fin = out[b - 1];
        match table[comb as usize] {
            None => table[comb as usize] = Some(fin),
            Some(v) if v != fin => {
                rep.verdict = "NOT-A-GATE".into();
                rep.why = format!("combination {comb:02b} settles to {} once and {} later", v as u8, fin as u8);
                return rep;
            }
            _ => {}
        }
        // the hold must end settled: its last quarter constant
        if (b - (hold as usize / 4).max(1)..b).any(|t| out[t] != fin) {
            rep.verdict = "NOT-A-GATE".into();
            rep.why = format!("the output does not settle within {hold} gt after the change to {comb:02b}");
            return rep;
        }
        let lat = (a..b).rev().find(|&t| out[t] != fin).map(|t| t + 1 - a).unwrap_or(0) as u32;
        latency = latency.max(lat);
        let moves = (a..b).filter(|&t| out[t] != if t == a { before } else { out[t - 1] }).count();
        if moves > 1 || (moves == 1 && before == fin) {
            glitches += 1;
        }
        before = fin;
    }
    let t: Vec<bool> = table.iter().map(|v| v.unwrap_or(false)).collect();
    rep.verdict = "GATE".into();
    rep.function = function_name(&t);
    rep.table = t;
    rep.latency = latency;
    rep.glitches = glitches;
    rep
}
