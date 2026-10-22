//! The storage counter's table search (`mctas circuit display counter-tables`, clk-obs
//! 2026-10-21): a storage counter (`circuit: storage-counter percent|stacks`) shows, at each
//! comparator level L of its chest, any number `compute::Table::allowed` allows (one the true count
//! rounds to for some fill with that reading). Its lamps are analog windows over L, so the table
//! decides the hardware: this search picks, per level, the number that needs the fewest window
//! units over all 14 segments (an inner window [lo, hi] costs 2, one touching L = 0 or L = 15 costs
//! 1, a window used by several lamps counts once; a segment lit at every level or never costs
//! nothing; a segment is lit by its ON runs, or by a torch off its OFF runs (1 more), or is the
//! complement of another segment (a torch, 1)).

use crate::analog::{Run, runs};
use circuit::compute::{SEGMENTS, Table};
use std::collections::BTreeSet;

/// The 14 segments' ON masks over L (ones a..g, then tens ta..tg; the tens digit blank below 10).
pub fn masks(v: &[u64; 16]) -> [u16; 14] {
    let mut m = [0u16; 14];
    for (l, &n) in v.iter().enumerate() {
        for (k, ch) in "abcdefg".chars().enumerate() {
            if SEGMENTS[(n % 10) as usize].contains(ch) {
                m[k] |= 1 << l;
            }
            if n >= 10 && SEGMENTS[(n / 10 % 10) as usize].contains(ch) {
                m[7 + k] |= 1 << l;
            }
        }
    }
    m
}

fn unit(r: &Run) -> u32 {
    if r.lo == 0 || r.hi == 15 { 1 } else { 2 }
}

/// How each segment is made, and the table's cost.
#[derive(Clone, Debug)]
pub struct Plan {
    pub cost: u32,
    pub windows: Vec<Run>,
    /// per segment: "const", "never", "on", "off", or "not <seg>"
    pub forms: Vec<String>,
}

thread_local! {
    /// `--per-lamp`: score each lamp on its own ([`plan_lamps`]).
    pub static PER_LAMP: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// `--inner`: an inner window's per-lamp cost.
    pub static INNER: std::cell::Cell<u32> = const { std::cell::Cell::new(3) };
}

/// The per-lamp cost (no sharing between lamps, the way a layout with one tap tree builds it): a
/// run touching L = 15 (a threshold: a dust node at that depth) 1, one touching L = 0 (a torch off a
/// node) 1, an inner run 3 (a subtract comparator, its side repeater, a second tap); the OFF form
/// one torch more; a constant lamp 1 (a redstone block), a dark one 0.
fn plan_lamps(v: &[u64; 16]) -> Plan {
    let m = masks(v);
    let inner = INNER.with(|w| w.get());
    // --inner 0: every run costs 1 (terms per lamp, inner or edge alike)
    let price = |rs: &[Run]| -> u32 { rs.iter().map(|r| if inner == 0 { 1 } else if r.lo == 0 || r.hi == 15 { 1 } else { inner }).sum() };
    let mut forms = Vec::new();
    let mut have: BTreeSet<(u8, u8)> = BTreeSet::new();
    let mut cost = 0;
    for &mk in &m {
        if mk == 0 {
            forms.push("never".to_string());
            continue;
        }
        if mk == u16::MAX {
            forms.push("const".to_string());
            cost += 1;
            continue;
        }
        let (on, off) = (runs(mk), runs(!mk));
        // (terms model: the lamp's driver is a torch or a repeater either way, no extra cost)
        let (pon, poff) = (price(&on), price(&off) + u32::from(inner != 0));
        let (rs, f) = if pon <= poff { (on, "on") } else { (off, "off") };
        cost += pon.min(poff);
        for r in rs {
            have.insert((r.lo, r.hi));
        }
        forms.push(f.to_string());
    }
    Plan { cost, windows: have.into_iter().map(|(lo, hi)| Run { lo, hi }).collect(), forms }
}

/// The plan for one table: segments in order of fewest runs first, each taking the cheaper of its
/// forms given the windows already bought.
pub fn plan(v: &[u64; 16]) -> Plan {
    if PER_LAMP.with(|p| p.get()) {
        return plan_lamps(v);
    }
    let m = masks(v);
    let mut order: Vec<usize> = (0..14).collect();
    order.sort_by_key(|&k| runs(m[k]).len().min(runs(!m[k]).len()));
    let mut have: BTreeSet<(u8, u8)> = BTreeSet::new();
    let mut forms = vec![String::new(); 14];
    let mut done: Vec<usize> = Vec::new();
    let mut cost = 0;
    for k in order {
        if m[k] == 0 {
            forms[k] = "never".into();
            continue;
        }
        if m[k] == u16::MAX {
            forms[k] = "const".into();
            continue;
        }
        if let Some(&j) = done.iter().find(|&&j| m[j] == m[k]) {
            forms[k] = format!("= {}", NAMES[j]);
            cost += 1;
            done.push(k);
            continue;
        }
        if let Some(&j) = done.iter().find(|&&j| m[j] == !m[k]) {
            forms[k] = format!("not {}", NAMES[j]);
            cost += 1;
            done.push(k);
            continue;
        }
        let price = |rs: &[Run]| -> u32 { rs.iter().filter(|r| !have.contains(&(r.lo, r.hi))).map(unit).sum() };
        let (on, off) = (runs(m[k]), runs(!m[k]));
        let (pon, poff) = (price(&on), price(&off) + 1);
        let (rs, form) = if pon <= poff { (on, "on") } else { (off, "off") };
        cost += pon.min(poff) + 1;
        for r in rs {
            have.insert((r.lo, r.hi));
        }
        forms[k] = form.into();
        done.push(k);
    }
    Plan { cost, windows: have.into_iter().map(|(lo, hi)| Run { lo, hi }).collect(), forms }
}

/// The search: local descent from random tables (a level's number changed one at a time while
/// the cost drops), `restarts` times; the `top` cheapest distinct tables.
pub fn search(t: Table, restarts: u32, top: usize) -> Vec<([u64; 16], Plan)> {
    let choices: Vec<Vec<u64>> = (0..16).map(|l| t.allowed(l).collect()).collect();
    let mut rng: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = |n: usize| -> usize {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n as u64) as usize
    };
    let mut best: Vec<([u64; 16], Plan)> = Vec::new();
    for _ in 0..restarts {
        let mut v = [0u64; 16];
        for l in 0..16 {
            v[l] = choices[l][next(choices[l].len())];
        }
        let mut cur = plan(&v).cost;
        loop {
            let mut improved = false;
            for l in 1..15 {
                for &c in &choices[l] {
                    if c == v[l] {
                        continue;
                    }
                    let old = v[l];
                    v[l] = c;
                    let p = plan(&v).cost;
                    if p < cur {
                        cur = p;
                        improved = true;
                    } else {
                        v[l] = old;
                    }
                }
            }
            if !improved {
                break;
            }
        }
        if best.iter().any(|(b, _)| *b == v) {
            continue;
        }
        best.push((v, plan(&v)));
        best.sort_by_key(|(_, p)| p.cost);
        best.truncate(top);
    }
    best
}

/// `counter-tables`: the cheapest tables, their windows and each segment's form.
pub fn print(t: Table, restarts: u32, top: usize) {
    for (v, p) in search(t, restarts, top) {
        let nums: Vec<String> = v.iter().map(|n| n.to_string()).collect();
        let ws: Vec<String> = p.windows.iter().map(|r| if r.lo == r.hi { format!("[{}]", r.lo) } else { format!("[{},{}]", r.lo, r.hi) }).collect();
        let forms: Vec<String> = NAMES.iter().zip(&p.forms).map(|(n, f)| format!("{n}:{f}")).collect();
        println!("cost {} | {} | windows {} {} | {}", p.cost, nums.join(" "), ws.len(), ws.join(" "), forms.join(" "));
    }
}

const NAMES: [&str; 14] = ["a", "b", "c", "d", "e", "f", "g", "ta", "tb", "tc", "td", "te", "tf", "tg"];

/// The 14 segments' names, in [`masks`] order.
pub fn names() -> [&'static str; 14] {
    NAMES
}
