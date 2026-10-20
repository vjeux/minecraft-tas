//! Census specs for the memory kinds, mirroring the judge's stimuli (crates/circuit/src/check.rs):
//! inputs in name order, steps spaced by a fixed gap.

use circuit_tff::census::Spec;
use circuit_tff::rs::Event;

/// The settle before the first step (the judge stands 40 gt at rest).
pub const SETTLE: u32 = 40;
/// Frames before SETTLE whose output must be steady.
pub const REST: u32 = 20;

/// The value of a cell at the end of each window `[starts[i], ends[i])`, from its rest value and
/// change frames; None when a window holds more than one change or a change comes before the
/// first window.
fn window_values(rest_on: bool, changes: &[u32], starts: &[u32], ends: &[u32]) -> Option<(Vec<bool>, u32)> {
    let mut v = rest_on;
    let mut out = Vec::new();
    let mut worst = 0;
    if changes.first().is_some_and(|&c| c < starts[0]) {
        return None;
    }
    for (i, (&a, &b)) in starts.iter().zip(ends).enumerate() {
        let n: Vec<u32> = changes.iter().copied().filter(|&c| c >= a && c < b).collect();
        if n.len() > 1 {
            return None;
        }
        if let Some(&c) = n.first() {
            v = !v;
            worst = worst.max(c - a);
        }
        let _ = i;
        out.push(v);
    }
    if changes.last().is_some_and(|&c| c >= *ends.last().unwrap()) {
        return None;
    }
    Some((out, worst))
}

/// rs-latch: inputs (r, s); pulses s, s, r, r, s, r of 4 gt and s, r of 20 gt.
pub struct RsSpec {
    pub gap: u32,
}

impl RsSpec {
    fn steps(&self) -> Vec<(usize, u32, u32)> {
        let mut t = SETTLE;
        let mut v = Vec::new();
        for (i, len) in [(1, 4), (1, 4), (0, 4), (0, 4), (1, 4), (0, 4), (1, 20), (0, 20)] {
            v.push((i, t, len));
            t += len + self.gap;
        }
        v
    }
}

impl Spec for RsSpec {
    fn events(&self) -> Vec<Event> {
        self.steps().iter().flat_map(|&(i, t, l)| [Event { t, input: i, on: true }, Event { t: t + l, input: i, on: false }]).collect()
    }
    fn frames(&self) -> u32 {
        let (_, t, l) = *self.steps().last().unwrap();
        t + l + self.gap
    }
    fn watch_from(&self) -> u32 {
        SETTLE - REST
    }
    fn react_by(&self) -> u32 {
        SETTLE + 16
    }
    fn judge(&self, rest_on: bool, changes: &[u32], _samples: &[u8]) -> Option<u32> {
        let st = self.steps();
        let starts: Vec<u32> = st.iter().map(|s| s.1).collect();
        let ends: Vec<u32> = st.iter().map(|s| s.1 + s.2 + self.gap).collect();
        let (vals, worst) = window_values(rest_on, changes, &starts, &ends)?;
        let want = [true, true, false, false, true, false, true, false];
        if vals.as_slice() == want { Some(worst) } else { None }
    }
}

/// d-latch: inputs (d, e); (d, e) held `gap` frames each, the judge's 12 steps; both enable
/// polarities pass (score + 1000 for enable low).
pub struct DSpec {
    pub gap: u32,
}

pub const D_STEPS: [(u8, u8); 12] = [(1, 0), (1, 1), (0, 1), (1, 1), (1, 0), (0, 0), (0, 1), (0, 0), (1, 0), (1, 1), (0, 1), (0, 0)];

impl Spec for DSpec {
    fn events(&self) -> Vec<Event> {
        let mut v = Vec::new();
        let (mut d0, mut e0) = (0u8, 0u8);
        for (k, &(d, e)) in D_STEPS.iter().enumerate() {
            let t = SETTLE + k as u32 * self.gap;
            if d != d0 {
                v.push(Event { t, input: 0, on: d == 1 });
            }
            if e != e0 {
                v.push(Event { t, input: 1, on: e == 1 });
            }
            (d0, e0) = (d, e);
        }
        v
    }
    fn frames(&self) -> u32 {
        SETTLE + D_STEPS.len() as u32 * self.gap
    }
    fn watch_from(&self) -> u32 {
        SETTLE - REST
    }
    fn react_by(&self) -> u32 {
        SETTLE + self.gap + 16
    }
    fn judge(&self, rest_on: bool, changes: &[u32], _samples: &[u8]) -> Option<u32> {
        let starts: Vec<u32> = (0..D_STEPS.len() as u32).map(|k| SETTLE + k * self.gap).collect();
        let ends: Vec<u32> = starts.iter().map(|s| s + self.gap).collect();
        let (vals, worst) = window_values(rest_on, changes, &starts, &ends)?;
        for (low, bonus) in [(false, 0), (true, 1000)] {
            let mut q = rest_on;
            let mut ok = true;
            for (k, &(d, e)) in D_STEPS.iter().enumerate() {
                let transparent = if low { e == 0 } else { e == 1 };
                if transparent {
                    q = d == 1;
                }
                if vals[k] != q {
                    ok = false;
                    break;
                }
            }
            if ok {
                return Some(worst + bonus);
            }
        }
        None
    }
}

/// counter N: 3N + 1 pulses of 4 gt spaced by `gap`; styles toggles (score), pulses (+1000),
/// level (+2000).
pub struct CounterSpec {
    pub n: u32,
    pub gap: u32,
}

impl CounterSpec {
    fn starts(&self) -> Vec<u32> {
        (0..3 * self.n + 1).map(|k| SETTLE + k * (4 + self.gap)).collect()
    }
}

impl Spec for CounterSpec {
    fn events(&self) -> Vec<Event> {
        circuit_tff::rs::pulses(0, SETTLE, 4, 4 + self.gap, 3 * self.n + 1)
    }
    fn frames(&self) -> u32 {
        SETTLE + (3 * self.n + 1) * (4 + self.gap)
    }
    fn watch_from(&self) -> u32 {
        SETTLE - REST
    }
    fn react_by(&self) -> u32 {
        // a level counter may never change on/off: no early drop
        self.frames()
    }
    fn samples(&self) -> Vec<u32> {
        self.starts().iter().map(|s| s + 4 + self.gap - 1).collect()
    }
    fn judge(&self, rest_on: bool, changes: &[u32], samples: &[u8]) -> Option<u32> {
        let starts = self.starts();
        let ends: Vec<u32> = starts.iter().map(|s| s + 4 + self.gap).collect();
        let n = self.n as usize;
        let per_window: Vec<Vec<u32>> = starts.iter().zip(&ends).map(|(&a, &b)| changes.iter().copied().filter(|&c| c >= a && c < b).collect()).collect();
        if changes.first().is_some_and(|&c| c < starts[0]) {
            return None;
        }
        // toggles: one change in the windows k = k0 (mod N), none elsewhere
        let busy: Vec<usize> = (0..starts.len()).filter(|&k| !per_window[k].is_empty()).collect();
        if n >= 2 && busy.len() >= 3 {
            let k0 = busy[0];
            let pattern = (0..starts.len()).all(|k| per_window[k].len() == if k >= k0 && (k - k0) % n == 0 { 1 } else { 0 });
            if pattern && k0 < n {
                let worst = busy.iter().map(|&k| per_window[k][0] - starts[k]).max().unwrap();
                return Some(worst);
            }
            // pulses: two changes (on, off) in those windows, off at rest
            let pattern2 = (0..starts.len()).all(|k| per_window[k].len() == if k >= k0 && (k - k0) % n == 0 { 2 } else { 0 });
            if pattern2 && k0 < n && !rest_on {
                let worst = busy.iter().map(|&k| per_window[k][0] - starts[k]).max().unwrap();
                return Some(worst + 1000);
            }
        }
        // level: the sampled levels repeat with smallest period exactly N
        if samples.len() == starts.len() && n >= 2 {
            let per = (1..=samples.len() / 2).find(|&p| (p..samples.len()).all(|i| samples[i] == samples[i - p]));
            if per == Some(n) {
                return Some(2000);
            }
        }
        None
    }
}
