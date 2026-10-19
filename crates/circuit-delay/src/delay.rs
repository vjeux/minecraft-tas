//! The delay-line checker (track delay): output = input shifted by N gt, both edges, pulses and
//! gaps preserved down to a minimum length (min-pulse). Mirrors the spec clockinfra's judge
//! checks: N from long pulses, then the shortest pulse/gap length m from which every pulse and gap
//! comes out exactly (stimulus S(m): every pair of a pulse and a gap with lengths in
//! {m, m+1, ..., m+4, 2m+6}), one-edge delays flagged with their rising/falling shifts.

use crate::harness::{Change, Circuit, run};

pub const WARM: u32 = 60;

#[derive(Clone, Debug, Default)]
pub struct DelayReport {
    /// DELAY, ONE-EDGE, NOT, ERROR.
    pub verdict: String,
    pub n: Option<u32>,
    pub rise: Option<u32>,
    pub fall: Option<u32>,
    pub min_pulse: Option<u32>,
    pub blocks: usize,
    pub dims: [i32; 3],
    pub why: String,
}

impl DelayReport {
    pub fn line(&self, name: &str) -> String {
        let d = self.dims;
        match self.verdict.as_str() {
            "DELAY" => format!(
                "DELAY: {name} N {} gt, min-pulse {}, {} blocks, {}x{}x{}",
                self.n.unwrap_or(0),
                self.min_pulse.map(|m| format!("{m} gt")).unwrap_or_else(|| "> 20 gt".into()),
                self.blocks,
                d[0],
                d[1],
                d[2]
            ),
            "ONE-EDGE" => format!("ONE-EDGE: {name} rising {} gt, falling {} gt, {} blocks, {}x{}x{}", self.rise.unwrap_or(0), self.fall.unwrap_or(0), self.blocks, d[0], d[1], d[2]),
            v => format!("{v}: {name} {}", self.why),
        }
    }
}

/// Input edges and output edges after `from`: (frame, rising).
fn edges(v: &[bool], from: usize) -> Vec<(usize, bool)> {
    (from.max(1)..v.len()).filter(|&t| v[t] != v[t - 1]).map(|t| (t, v[t])).collect()
}

/// The stimulus of resolution m from `start`: every (pulse L, gap G) with L, G in
/// {m..=m+4, 2m+6}; returns the changes and the frame after the last gap.
pub fn stimulus(m: u32, start: u32) -> (Vec<Change>, u32) {
    let mut lens: Vec<u32> = (m..=m + 4).collect();
    lens.push(2 * m + 6);
    let mut t = start;
    let mut out = Vec::new();
    for &l in &lens {
        for &g in &lens {
            out.push(Change { at: t, input: 0, on: true });
            out.push(Change { at: t + l, input: 0, on: false });
            t += l + g;
        }
    }
    (out, t)
}

/// Does the output equal the input shifted by n from frame `from` on (input off before)?
fn shifted(inp: &[bool], out: &[bool], n: usize, from: usize) -> bool {
    (from..out.len()).all(|t| out[t] == (t >= n && inp[t - n]))
}

/// The edge shifts of one output waveform under the long-pulse stimulus [`long_stimulus`]:
/// `Some((rise, fall))` when every input edge comes out once, in order, with constant shifts.
pub fn edge_shifts(inp: &[bool], out: &[bool]) -> Option<(u32, u32)> {
    if out[WARM as usize - 1] {
        return None;
    }
    let ie = edges(inp, WARM as usize - 1);
    let oe = edges(out, WARM as usize - 1);
    if oe.len() != ie.len() || oe.iter().zip(&ie).any(|(o, i)| o.1 != i.1 || o.0 < i.0) {
        return None;
    }
    let d: Vec<usize> = oe.iter().zip(&ie).map(|(o, i)| o.0 - i.0).collect();
    if d.len() != 4 || d[0] != d[2] || d[1] != d[3] {
        return None;
    }
    Some((d[0] as u32, d[1] as u32))
}

/// Two pulses of `p` gt with `p` gt gaps from [`WARM`]: (changes, frames up to the last edge).
pub fn long_stimulus(p: u32) -> (Vec<Change>, u32) {
    (
        vec![
            Change { at: WARM, input: 0, on: true },
            Change { at: WARM + p, input: 0, on: false },
            Change { at: WARM + 2 * p, input: 0, on: true },
            Change { at: WARM + 3 * p, input: 0, on: false },
        ],
        WARM + 3 * p,
    )
}

/// The shortest pulse/gap length (1..=max) a runner's output preserves with shift n.
pub fn min_pulse(n: u32, max: u32, runner: &dyn Fn(&[Change], u32) -> Option<(Vec<bool>, Vec<bool>)>) -> Option<u32> {
    for m in 1..=max {
        let (ch, end) = stimulus(m, WARM);
        let Some((inp, out)) = runner(&ch, end + n + 20) else { continue };
        if shifted(&inp, &out, n as usize, WARM as usize) {
            return Some(m);
        }
    }
    None
}

pub fn analyze(c: &Circuit, max_n: u32) -> DelayReport {
    let mut rep = DelayReport { blocks: c.blocks(), dims: c.dims(), ..Default::default() };
    if c.inputs.len() != 1 {
        rep.verdict = "ERROR".into();
        rep.why = "a delay line has one input".into();
        return rep;
    }
    // 1. long pulses: N (or the two edges' shifts)
    let p = 60u32;
    let changes = vec![
        Change { at: WARM, input: 0, on: true },
        Change { at: WARM + p, input: 0, on: false },
        Change { at: WARM + 2 * p, input: 0, on: true },
        Change { at: WARM + 3 * p, input: 0, on: false },
    ];
    let frames = WARM + 4 * p + max_n + 40;
    let tr = match run(c, &changes, frames, 4) {
        Ok(t) => t,
        Err(e) => {
            rep.verdict = "ERROR".into();
            rep.why = e;
            return rep;
        }
    };
    let out = tr.out();
    if out[WARM as usize - 1] {
        rep.verdict = "NOT".into();
        rep.why = "the output is on with the input off".into();
        return rep;
    }
    let ie = edges(&tr.inputs[0], WARM as usize - 1);
    let oe = edges(&out, WARM as usize - 1);
    if oe.len() != ie.len() || oe.iter().zip(&ie).any(|(o, i)| o.1 != i.1 || o.0 < i.0) {
        rep.verdict = "NOT".into();
        rep.why = format!("output edges {:?} for input edges {:?}", oe.iter().map(|e| e.0).collect::<Vec<_>>(), ie.iter().map(|e| e.0).collect::<Vec<_>>());
        return rep;
    }
    let d: Vec<usize> = oe.iter().zip(&ie).map(|(o, i)| o.0 - i.0).collect();
    if d[0] != d[2] || d[1] != d[3] {
        rep.verdict = "NOT".into();
        rep.why = format!("edge shifts {d:?} vary");
        return rep;
    }
    if d[0] != d[1] {
        rep.verdict = "ONE-EDGE".into();
        rep.rise = Some(d[0] as u32);
        rep.fall = Some(d[1] as u32);
        return rep;
    }
    let n = d[0] as u32;
    rep.verdict = "DELAY".into();
    rep.n = Some(n);
    // 2. the shortest pulse/gap length it preserves
    for m in 1..=20u32 {
        let (ch, end) = stimulus(m, WARM);
        let Ok(t) = run(c, &ch, end + n + 40, 4) else { continue };
        if shifted(&t.inputs[0], &t.out(), n as usize, WARM as usize) {
            rep.min_pulse = Some(m);
            break;
        }
    }
    rep
}
