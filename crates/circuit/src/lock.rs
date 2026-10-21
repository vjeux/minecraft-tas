//! Study 14: combination locks (`circuit: lock N`).
//!
//! The player's dials are N item frames holding an item (`port input.d0: x y z frame`, d0..d<N-1>):
//! a right-click turns a frame's item one step of 8, and a comparator reading the frame through
//! the block it hangs on gives rotation + 1 (`ItemFrame.getAnalogOutput`). The code is set the
//! same way, by N hidden code frames (`port input.k0: x y z frame`, one per dial): the lock opens
//! when every dial shows its code frame's rotation.
//!
//! The judge turns the frames like a player: one right-click with an empty hand per step (the
//! bridge's `use_entity`, `ItemFrame.interact` -> `setRotation(r + 1)`, which updates the
//! comparators reading it; rewriting the frame's data would not). It tries EVERY combination of
//! the dials (an odometer, [`STEP`] ticks per combination), then sets a second code by turning
//! every code frame 3 steps and tries every combination again. The output must be on exactly at
//! the code and off everywhere else: in each combination it may change once, from what the
//! previous combination showed to what this one must show, and must have settled before the next
//! click (no flicker while turning, no false open on the way through).

use crate::check::{Outcome, Sizing};
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, InputHow};
use anyhow::{bail, Result};

/// Ticks each combination is held: the lock must settle within it.
pub const STEP: u32 = 16;
/// Steps each code frame turns to set the second code.
const RECODE: u8 = 3;

/// The dials' and the code frames' input indices, in digit order.
fn frames(c: &Circuit, n: u32) -> (Vec<usize>, Vec<usize>) {
    let find = |p: &str, i: u32| c.inputs.iter().position(|x| x.name == format!("{p}{i}"));
    let dials = (0..n).filter_map(|i| find("d", i)).collect();
    let keys = (0..n).filter_map(|i| find("k", i)).collect();
    (dials, keys)
}

/// Ports of a `lock N`: dials d0..d<N-1> and code frames k0..k<N-1>, every one a `frame` input,
/// and one output.
pub fn check_ports(c: &Circuit, n: u32) -> Result<()> {
    if n == 0 || n > 6 {
        bail!("lock {n}: 1 to 6 dials (8^N combinations are tried)");
    }
    let (dials, keys) = frames(c, n);
    if dials.len() != n as usize || keys.len() != n as usize || c.inputs.len() != 2 * n as usize {
        let names: Vec<_> = c.inputs.iter().map(|i| i.name.as_str()).collect();
        bail!("lock {n}: needs inputs d0..d{} (the dials) and k0..k{} (their code frames), nothing else; has {names:?}", n - 1, n - 1);
    }
    for i in &c.inputs {
        if !matches!(i.how, InputHow::Frame { .. }) {
            bail!("lock {n}: input {} must be an item frame (`port input.{}: x y z frame`)", i.name, i.name);
        }
    }
    if c.outputs.len() != 1 {
        bail!("lock {n}: needs exactly one output (on while the lock is open)");
    }
    Ok(())
}

fn rotation(c: &Circuit, i: usize) -> u8 {
    match c.inputs[i].how {
        InputHow::Frame { rotation } => rotation,
        _ => 0,
    }
}

fn digits(v: &[u8]) -> String {
    v.iter().map(|r| r.to_string()).collect::<Vec<_>>().join("")
}

/// Every combination of the dials (twice: the built code, then a code reset by the code frames).
/// Each phase is one combination: `dials 3517 code 3517 open` / `... shut`.
pub fn stimulus(c: &Circuit, n: u32, _z: &Sizing) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let (dials, keys) = frames(c, n);
    let mut rot: Vec<u8> = (0..c.inputs.len()).map(|i| rotation(c, i)).collect();
    let mut t = c.clock.free_from() + 40;
    for round in 0..2 {
        if round == 1 {
            // the player sets a new code: every code frame turned RECODE steps, one click each
            for &k in &keys {
                for _ in 0..RECODE {
                    rot[k] = (rot[k] + 1) % 8;
                    s.levels.push((t, k, rot[k]));
                    t += 4;
                }
            }
            t += 40;
        }
        let code: Vec<u8> = keys.iter().map(|&k| rot[k]).collect();
        let start: Vec<u8> = dials.iter().map(|&d| rot[d]).collect();
        let total = 8u32.pow(n);
        for step in 0..total {
            if step > 0 {
                // odometer: dial 0 one step; a dial back at its start carries into the next one
                for (j, &d) in dials.iter().enumerate() {
                    rot[d] = (rot[d] + 1) % 8;
                    s.levels.push((t, d, rot[d]));
                    if rot[d] != start[j] {
                        break;
                    }
                }
            }
            let now: Vec<u8> = dials.iter().map(|&d| rot[d]).collect();
            let open = now == code;
            s.phase(t, t + STEP, format!("dials {} code {} {}", digits(&now), digits(&code), if open { "open" } else { "shut" }));
            t += STEP;
        }
        t += 20;
    }
    s.ticks = t;
    s
}

/// The output per combination: settled to its value before the next click, at most one change
/// in each combination (from the previous combination's value to this one's).
pub fn check(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let frames = t.frames();
    let on = |f: usize| f < frames && t.on(0, f);
    let mut table = Vec::new();
    let (mut open_lat, mut close_lat, mut tried, mut opened) = (0usize, 0usize, 0usize, 0usize);
    let mut prev: Option<bool> = None;
    let mut prev_end = 0u32;
    for (a, b, label) in &s.phases {
        let want = label.ends_with(" open");
        let (a, b) = (*a as usize, (*b as usize).min(frames));
        if a >= b {
            return Outcome::fail(format!("the run ended before {label}"), table);
        }
        // a phase right after a gap (the code being set) starts from whatever the lock shows
        let from = if a as u32 == prev_end { prev } else { None };
        let seq: Vec<bool> = (a..b).map(on).collect();
        let changes = seq.windows(2).filter(|w| w[0] != w[1]).count();
        if seq.last() != Some(&want) {
            table.extend(states(c, t, b - 1));
            return Outcome::fail(
                format!("{label}: the output is {} at the end of the combination ({} gt after the click), want {}", onoff(*seq.last().unwrap_or(&false)), b - a, onoff(want)),
                table,
            );
        }
        let first = seq.first().copied().unwrap_or(false);
        let flicker = changes > 1 || (changes == 1 && from == Some(want)) || (from.is_some_and(|p| p != first) && first != want);
        if flicker {
            let at: Vec<String> = seq.iter().enumerate().filter(|(i, v)| *i == 0 || seq[i - 1] != **v).map(|(i, v)| format!("{} at +{i}", onoff(*v))).collect();
            return Outcome::fail(format!("{label}: the output flickers while turning ({})", at.join(", ")), table);
        }
        // latency: the first frame from which the output stays at its value
        let lat = seq.iter().rposition(|v| *v != want).map(|i| i + 1).unwrap_or(0);
        if want && from == Some(false) {
            open_lat = open_lat.max(lat);
            opened += 1;
            table.push(format!("{label} after {lat} gt"));
        }
        if !want && from == Some(true) {
            close_lat = close_lat.max(lat);
            table.push(format!("{label} after {lat} gt"));
        }
        tried += 1;
        prev = Some(want);
        prev_end = b as u32;
    }
    let codes = s.phases.iter().filter(|p| p.2.ends_with(" open")).count();
    if opened != codes || codes != 2 {
        return Outcome::fail(format!("lock {n}: opened at {opened} of {codes} codes (2 codes tried)"), table);
    }
    let metrics = vec![
        ("dials".to_string(), format!("{n}")),
        ("combinations tried".to_string(), format!("{tried} ({} per code, 2 codes)", 8usize.pow(n))),
        ("opens within".to_string(), format!("{open_lat} gt")),
        ("closes within".to_string(), format!("{close_lat} gt")),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}

fn onoff(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

/// The design's redstone components at frame `f` (a failure's evidence: what each comparator
/// read and gave).
fn states(c: &Circuit, t: &Trace, f: usize) -> Vec<String> {
    let mut v = vec![format!("components at frame {f}:")];
    for p in c.clock.design.blocks.keys() {
        if let Some(s) = t.state(f, *p).filter(|s| ["comparator", "repeater", "wire", "torch", "lamp"].iter().any(|k| s.contains(k))) {
            v.push(format!("  {p:?} {}", s.trim_start_matches("minecraft:")));
        }
    }
    v
}
