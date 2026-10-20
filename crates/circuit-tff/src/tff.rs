//! T flip-flop metrics on redsim (the judge's names): latency (input edge -> output change),
//! min-pulse (the shortest input pulse it follows), min-period (the shortest spacing between pulse
//! starts it follows), edge (rising | falling).
//!
//! "Follows a pulse train": the output is steady (on/off = level > 0) for the frames before the
//! first pulse, and then changes exactly once per pulse: the i-th change comes at or after the i-th
//! pulse's start and at most `MAX_LATENCY` gt after it, and there are no other changes up to the
//! end of the run.

use crate::rs::{Event, Prepared, pulses};

/// Frames before the first pulse (the placement settles); the output must be steady over the last
/// `REST` of them.
pub const SETTLE: u32 = 40;
pub const REST: u32 = 20;
/// Frames after the last pulse.
pub const TAIL: u32 = 80;
pub const MAX_LATENCY: u32 = 40;

#[derive(Clone, Debug)]
pub struct TrainResult {
    pub ok: bool,
    pub why: String,
    /// Output change - pulse start, per pulse (when ok).
    pub latencies: Vec<u32>,
    /// The output level at rest before the first pulse (0 = off).
    pub rest: u8,
}

/// Run `count` pulses of `len` gt every `period` gt on `input`; judge output `out`.
pub fn train(p: &Prepared, input: usize, out: usize, len: u32, period: u32, count: u32) -> Result<TrainResult, String> {
    let events: Vec<Event> = pulses(input, SETTLE, len, period, count);
    let frames = SETTLE + (count - 1) * period + len.max(period) + TAIL;
    let lv = p.run(&events, frames)?;
    Ok(judge(&lv[out], len, period, count))
}

pub fn judge(levels: &[u8], len: u32, period: u32, count: u32) -> TrainResult {
    let _ = len;
    let on: Vec<bool> = levels.iter().map(|&l| l > 0).collect();
    let rest = levels[(SETTLE - 1) as usize];
    let fail = |why: String| TrainResult { ok: false, why, latencies: Vec::new(), rest };
    let r0 = SETTLE as usize;
    if (r0 - REST as usize..r0).any(|t| on[t] != on[r0 - 1]) {
        return fail("the output is not steady before the first pulse".into());
    }
    let changes: Vec<usize> = (r0..on.len()).filter(|&t| on[t] != on[t - 1]).collect();
    if changes.len() != count as usize {
        return fail(format!("{} output changes for {count} pulses", changes.len()));
    }
    let mut lat = Vec::new();
    for (i, &t) in changes.iter().enumerate() {
        let r = r0 + i * period as usize;
        if t < r {
            return fail(format!("change {} at {} before its pulse at {r}", i + 1, t));
        }
        if t - r > MAX_LATENCY as usize {
            return fail(format!("change {} comes {} gt after its pulse", i + 1, t - r));
        }
        lat.push((t - r) as u32);
    }
    TrainResult { ok: true, why: String::new(), latencies: lat, rest }
}

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    /// Accepted pulse lengths among 1..=MAX_PULSE (6 pulses, 40 gt apart).
    pub accepted: Vec<u32>,
    pub min_pulse: Option<u32>,
    /// Every length from min_pulse to MAX_PULSE accepted.
    pub long_ok: bool,
    /// (latency on, latency off) at min_pulse (from the edge below).
    pub latency: Option<(u32, u32)>,
    pub edge: &'static str,
    /// (period, pulse length) the fastest train followed.
    pub min_period: Option<(u32, u32)>,
    pub rest: u8,
    pub why: String,
}

pub const MAX_PULSE: u32 = 30;

pub fn metrics(p: &Prepared, input: usize, out: usize) -> Result<Metrics, String> {
    let mut m = Metrics { edge: "rising", ..Default::default() };
    let mut lat_of: Vec<(u32, Vec<u32>)> = Vec::new();
    for len in 1..=MAX_PULSE {
        let r = train(p, input, out, len, len + 40, 6)?;
        m.rest = r.rest;
        if r.ok {
            m.accepted.push(len);
            lat_of.push((len, r.latencies));
        } else if m.why.is_empty() {
            m.why = format!("pulse {len}: {}", r.why);
        }
    }
    m.min_pulse = m.accepted.first().copied();
    let Some(lmin) = m.min_pulse else { return Ok(m) };
    m.long_ok = (lmin..=MAX_PULSE).all(|l| m.accepted.contains(&l));
    // edge: a falling-edge TFF's latency from the rising edge grows with the pulse length
    let lat_at = |l: u32| lat_of.iter().find(|(x, _)| *x == l).map(|(_, v)| v.clone());
    let l2 = m.accepted.iter().copied().find(|&l| l >= lmin + 4);
    let mut shift = 0;
    if let (Some(a), Some(l2)) = (lat_at(lmin), l2)
        && let Some(b) = lat_at(l2)
        && a.iter().zip(&b).all(|(x, y)| *y == *x + (l2 - lmin))
    {
        m.edge = "falling";
        shift = lmin;
    }
    if let Some(v) = lat_at(lmin) {
        // pulses 1, 3, 5 turn the output on (from rest off) or off (from rest on)
        let first: u32 = v.iter().step_by(2).copied().max().unwrap_or(0) - shift;
        let second: u32 = v.iter().skip(1).step_by(2).copied().max().unwrap_or(0) - shift;
        m.latency = Some(if m.rest == 0 { (first, second) } else { (second, first) });
    }
    // the fastest train: per accepted length (up to 16), the smallest period that it and the next
    // 8 periods follow
    let mut best: Option<(u32, u32)> = None;
    for &len in m.accepted.iter().filter(|&&l| l <= 16) {
        let mut found = None;
        let mut t = len + 1;
        while t <= len + 60 {
            let ok_at = |t: u32| train(p, input, out, len, t, 12).map(|r| r.ok);
            if ok_at(t)? && (t + 1..=t + 8).all(|u| ok_at(u).unwrap_or(false)) {
                found = Some(t);
                break;
            }
            t += 1;
        }
        if let Some(t) = found
            && best.is_none_or(|(bt, _)| t < bt)
        {
            best = Some((t, len));
        }
    }
    m.min_period = best;
    Ok(m)
}

impl Metrics {
    pub fn summary(&self) -> String {
        let Some(lmin) = self.min_pulse else {
            return format!("NOT A TFF ({})", self.why);
        };
        let lat = match self.latency {
            Some((a, b)) if a == b => format!("latency {a} gt"),
            Some((a, b)) => format!("latency {a}/{b} gt (on/off)"),
            None => "latency ?".into(),
        };
        let pulses = if self.long_ok {
            format!("min-pulse {lmin}")
        } else {
            let acc: Vec<String> = self.accepted.iter().map(|l| l.to_string()).collect();
            format!("pulses {{{}}} only", acc.join(","))
        };
        let per = match self.min_period {
            Some((t, l)) => format!("min-period {t} (pulse {l})"),
            None => "min-period ?".into(),
        };
        format!("TFF {lat}, {pulses}, {per}, edge {}{}", self.edge, if self.rest > 0 { ", rests ON" } else { "" })
    }
}
