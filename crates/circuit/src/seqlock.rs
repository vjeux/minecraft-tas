//! Button sequence locks (`circuit: sequence-lock N L`, the lock study; coordinator 2026-10-21):
//! press N buttons in the right order of L presses to open (the classic "password door").
//!
//! * Ports: N buttons `port input.a..: x y z button` (a, b, c, d), pressed for real by the judge
//!   (each holds its own time: stone 20 gt, wood 30 gt); L code frames `port input.k0..: x y z
//!   frame`, whose rotation is the button of that step (0 = a, 1 = b, ...), set inside by the owner
//!   (hidden-code rule, [`hidden_code`]); one output, a tap a builder can take (the design's
//!   repeater or comparator with a free line out of its box, [`crate::random::check_ports`]'s rule).
//! * The lock (strict reset): state s = correct presses so far. A press of the button of step s
//!   moves to s + 1; any other press resets to 0 (it does not count as a first press). At s = L the
//!   output is on; the next press, whatever it is, closes it (s = 0).
//! * Stimulus: for the code as built and two codes the judge sets by turning the frames (every
//!   step the last button: repeats; then a mixed code), every sequence of L presses from the reset
//!   state (N^L of them, each preceded by a wrong press when the lock is not at 0). The presses come
//!   as fast as the buttons allow (the next one 2 gt after the previous button popped out) for the
//!   first and third codes, and at mixed paces (fast, 40, 120 gt, now and then 600 gt) for the
//!   second.
//! * Check: before the first press the output is off; after every press it changes at most once,
//!   from the previous state's value to the new one, and has settled before the next press (no
//!   flicker: a wrong sequence never shows the output on, even for a tick).

use crate::check::Outcome;
use crate::stimulus::Schedule;
use crate::trace::Trace;
use crate::{Circuit, InputHow};
use anyhow::{Result, bail};

/// Button names in index order.
pub const BUTTONS: [&str; 4] = ["a", "b", "c", "d"];

/// The extra paces of the mixed run (gt between two presses, beyond the fastest).
const MIXED: [u32; 7] = [0, 18, 0, 98, 0, 0, 38];

fn index(c: &Circuit, name: &str) -> Option<usize> {
    c.inputs.iter().position(|i| i.name == name)
}

/// The buttons' and the code frames' input indices.
fn ports(c: &Circuit, n: u32, l: u32) -> (Vec<usize>, Vec<usize>) {
    let b = BUTTONS[..n as usize].iter().filter_map(|x| index(c, x)).collect();
    let k = (0..l).filter_map(|i| index(c, &format!("k{i}"))).collect();
    (b, k)
}

/// The ports a `sequence-lock N L` needs.
pub fn check_ports(c: &Circuit, n: u32, l: u32) -> Result<()> {
    if !(2..=4).contains(&n) || !(2..=6).contains(&l) {
        bail!("sequence-lock {n} {l}: 2 to 4 buttons, a code of 2 to 6 presses");
    }
    let (b, k) = ports(c, n, l);
    if b.len() != n as usize || k.len() != l as usize || c.inputs.len() != (n + l) as usize {
        let names: Vec<_> = c.inputs.iter().map(|i| i.name.as_str()).collect();
        bail!("sequence-lock {n} {l}: needs the buttons {} and the code frames k0..k{}, nothing else; has {names:?}", BUTTONS[..n as usize].join(", "), l - 1);
    }
    for &i in &b {
        if c.inputs[i].how != InputHow::Button {
            bail!("sequence-lock {n} {l}: input {} must be the design's button (`port input.{}: x y z button`)", c.inputs[i].name, c.inputs[i].name);
        }
    }
    for &i in &k {
        match c.inputs[i].how {
            InputHow::Frame { rotation } if (rotation as u32) < n => {}
            InputHow::Frame { rotation } => bail!("sequence-lock {n} {l}: code frame {} is at rotation {rotation}, not a button (0..{})", c.inputs[i].name, n - 1),
            _ => bail!("sequence-lock {n} {l}: input {} must be an item frame (`port input.{}: x y z frame`)", c.inputs[i].name, c.inputs[i].name),
        }
    }
    if c.outputs.len() != 1 {
        bail!("sequence-lock {n} {l}: needs exactly one output (on while the lock is open)");
    }
    crate::random::tap_rule(c, &format!("sequence-lock {n} {l}"))?;
    hidden_code(c, n, l)
}

/// A button's outward direction (the side its player stands on).
fn button_facing(state: &str) -> Option<[i32; 3]> {
    let prop = |k: &str| state.split(&format!("{k}=")).nth(1).and_then(|r| r.split([',', ']']).next());
    match prop("face")? {
        "floor" => Some([0, 1, 0]),
        "ceiling" => Some([0, -1, 0]),
        _ => Some(match prop("facing")? {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "west" => [-1, 0, 0],
            _ => [1, 0, 0],
        }),
    }
}

fn dot(a: [i32; 3], b: [i32; 3]) -> i32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The hidden-code rule (the combination lock's, [`crate::lock::hidden_code`], with buttons): every
/// button faces one side F, the user side is at or beyond the buttons' plane, and every code frame
/// hangs behind that plane facing away from the user, or is enclosed by opaque blocks.
pub fn hidden_code(c: &Circuit, n: u32, l: u32) -> Result<()> {
    let (b, k) = ports(c, n, l);
    let face = |i: usize| c.clock.design.blocks.get(&c.inputs[i].pos).and_then(|s| button_facing(s));
    let f = face(b[0]).ok_or_else(|| anyhow::anyhow!("sequence-lock {n} {l}: button a has no face"))?;
    for &i in &b {
        if face(i) != Some(f) {
            bail!("sequence-lock {n} {l}: every button must face the same side (the user side): {} does not", c.inputs[i].name);
        }
    }
    let plane = b.iter().map(|&i| dot(c.inputs[i].pos, f)).min().unwrap_or(0);
    for &i in &k {
        crate::lock::frame_hidden(c, i, f, plane).map_err(|why| anyhow::anyhow!("sequence-lock {n} {l}: {why}"))?;
    }
    Ok(())
}

/// The reference lock: the state after pressing `b` in state `s`.
pub fn step(code: &[u32], s: usize, b: u32) -> usize {
    if s < code.len() && code[s] == b { s + 1 } else { 0 }
}

/// The codes the judge tries: as built, every step the last button, then a mixed one.
pub fn codes(built: &[u32], n: u32) -> Vec<Vec<u32>> {
    let l = built.len();
    let rep = vec![n - 1; l];
    let mut mixed: Vec<u32> = (0..l as u32).map(|k| (k * 3 + 1) % n).collect();
    if mixed == built || mixed == rep {
        mixed = (0..l as u32).map(|k| (k + 2) % n).collect();
    }
    vec![built.to_vec(), rep, mixed]
}

fn word(code: &[u32]) -> String {
    code.iter().map(|b| BUTTONS[*b as usize]).collect()
}

/// The stimulus (module doc). Phases: `rest shut`, `recode shut`, and one per press:
/// `code abc: press b (1 -> 2) shut`.
pub fn stimulus(c: &Circuit, n: u32, l: u32, start: u32) -> Schedule {
    let mut s = Schedule::new(c.inputs.len());
    let (b, k) = ports(c, n, l);
    let own = b.iter().map(|&i| crate::tnt::press_ticks(c.clock.design.blocks.get(&c.inputs[i].pos).map(|s| s.as_str()).unwrap_or(""))).max().unwrap_or(30);
    let fast = own + 2;
    let mut rot: Vec<u8> = k.iter().map(|&i| match c.inputs[i].how { InputHow::Frame { rotation } => rotation, _ => 0 }).collect();
    let built: Vec<u32> = rot.iter().map(|r| *r as u32).collect();
    let mut t = start;
    s.phase(t.saturating_sub(30), t, "rest shut".to_string());
    let mut presses = 0usize;
    for (round, code) in codes(&built, n).into_iter().enumerate() {
        if round > 0 {
            // the owner turns every code frame to its new button, one click per step
            let from = t;
            for (j, &i) in k.iter().enumerate() {
                while rot[j] as u32 != code[j] {
                    rot[j] = (rot[j] + 1) % 8;
                    s.levels.push((t, i, rot[j]));
                    t += 4;
                }
            }
            t += 40;
            s.phase(from, t, "recode shut".to_string());
        }
        let mut st = 0usize;
        let total = n.pow(l);
        let mut press = |s: &mut Schedule, t: &mut u32, st: &mut usize, btn: u32, presses: &mut usize| {
            let next = step(&code, *st, btn);
            let pace = if round == 1 { fast + MIXED[*presses % MIXED.len()] + if *presses % 97 == 96 { 600 } else { 0 } } else { fast };
            s.pulse(b[btn as usize], *t, 1);
            s.phase(*t, *t + pace, format!("code {}: press {} ({} -> {}) {}", word(&code), BUTTONS[btn as usize], st, next, if next == l as usize { "open" } else { "shut" }));
            *t += pace;
            *st = next;
            *presses += 1;
        };
        for w in 0..total {
            if st != 0 {
                // a wrong press first (any press closes an open lock)
                let wrong = if st < l as usize { (0..n).find(|x| *x != code[st]).unwrap_or(0) } else { 0 };
                press(&mut s, &mut t, &mut st, wrong, &mut presses);
            }
            let mut v = w;
            let digits: Vec<u32> = (0..l).map(|_| { let d = v % n; v /= n; d }).collect();
            for d in digits.into_iter().rev() {
                press(&mut s, &mut t, &mut st, d, &mut presses);
            }
        }
        if st != 0 {
            let wrong = if st < l as usize { (0..n).find(|x| *x != code[st]).unwrap_or(0) } else { 0 };
            press(&mut s, &mut t, &mut st, wrong, &mut presses);
        }
    }
    s.ticks = t + 20;
    s
}

fn onoff(v: bool) -> &'static str {
    if v { "on" } else { "off" }
}

/// Judge one run (module doc).
pub fn check(_c: &Circuit, n: u32, l: u32, s: &Schedule, t: &Trace) -> Outcome {
    let frames = t.frames();
    let on = |f: usize| f < frames && t.on(0, f);
    let mut table = Vec::new();
    let (mut open_lat, mut close_lat, mut presses, mut opened) = (0usize, 0usize, 0usize, 0usize);
    let mut prev: Option<bool> = None;
    let mut prev_end = 0u32;
    for (a, b, label) in &s.phases {
        let want = label.ends_with(" open");
        let (a, b) = (*a as usize, (*b as usize).min(frames));
        if a >= b {
            return Outcome::fail(format!("the run ended before {label}"), table);
        }
        let from = if a as u32 == prev_end { prev } else { None };
        let seq: Vec<bool> = (a..b).map(on).collect();
        let changes = seq.windows(2).filter(|w| w[0] != w[1]).count();
        let last = *seq.last().unwrap_or(&false);
        if last != want {
            return Outcome::fail(format!("{label}: the output is {} {} gt after the press, want {}", onoff(last), b - a, onoff(want)), table);
        }
        let first = seq.first().copied().unwrap_or(false);
        let flicker = changes > 1 || (changes == 1 && from == Some(want)) || (from.is_some_and(|p| p != first) && first != want) || (!label.contains("press") && changes > 0);
        if flicker {
            let at: Vec<String> = seq.iter().enumerate().filter(|(i, v)| *i == 0 || seq[i - 1] != **v).map(|(i, v)| format!("{} at +{i}", onoff(*v))).collect();
            return Outcome::fail(format!("{label}: the output flickers ({})", at.join(", ")), table);
        }
        let lat = seq.iter().rposition(|v| *v != want).map(|i| i + 1).unwrap_or(0);
        if want && from == Some(false) {
            open_lat = open_lat.max(lat);
            opened += 1;
        }
        if !want && from == Some(true) {
            close_lat = close_lat.max(lat);
        }
        if label.contains("press") {
            presses += 1;
        }
        prev = Some(want);
        prev_end = b as u32;
    }
    let want_opens = s.phases.iter().filter(|p| p.2.ends_with(" open")).count();
    if opened != want_opens || want_opens < 3 {
        return Outcome::fail(format!("opened {opened} times of {want_opens}"), table);
    }
    table.push(format!("{presses} presses, opened {opened} times (3 codes, every sequence of {l} presses each)"));
    let metrics = vec![
        ("buttons".to_string(), n.to_string()),
        ("code".to_string(), format!("{l} presses")),
        ("presses tried".to_string(), presses.to_string()),
        ("opens within".to_string(), format!("{open_lat} gt")),
        ("closes within".to_string(), format!("{close_lat} gt")),
    ];
    Outcome { pass: true, why: String::new(), metrics, table }
}
