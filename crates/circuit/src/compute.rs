//! Study 12, computational redstone (designs/circuits/compute/VARIATIONS.md): binary adders,
//! subtractors, incrementers, comparators, multiplexers, demultiplexers, decoders and encoders,
//! judged on their EXHAUSTIVE truth table, and binary counters with a parallel binary output.
//!
//! Ports: bit k of a value `a` is `input.a<k>` (k = 0: the lowest bit), outputs likewise
//! (`output.s<k>`); single ports by name (`cin`, `cout`, `eq`, `q`...). Unsigned arithmetic.
//!
//! The combinational stimulus visits every input state in Gray-code order, then every
//! single-input transition in both directions (an Euler tour of the n-cube, n <= 10 inputs), then
//! every state reached from its complement (every input changing at once: a parallel load),
//! each state held for the gap. Every output must settle to the ideal value and hold it to the
//! end of each hold. `latency`: the worst gt from an input change to the frame where every output
//! holds its final value, with the transition that takes it; `glitches`: holds where an output
//! changes more than once before it settles.

use crate::check::{Outcome, Sizing};
use crate::stimulus::{SETTLE, Schedule};
use crate::trace::Trace;
use crate::Circuit;
use anyhow::{Result, bail};
use std::collections::BTreeSet;

/// The function a compute circuit claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    Adder,
    Subtractor,
    Incrementer,
    Comparator,
    Mux,
    Demux,
    Decoder,
    Encoder,
    PriorityEncoder,
    BinaryCounter,
}

/// The most inputs judged exhaustively (2^16 states); bigger ripple designs are bit slices.
pub const MAX_INPUTS: usize = 16;
/// The most inputs for which every single-input transition is visited (n 2^n holds).
pub const MAX_WALK_INPUTS: usize = 10;
/// The comparator outputs.
pub const RELATIONS: [&str; 6] = ["eq", "ne", "lt", "gt", "le", "ge"];

impl Op {
    /// `adder 4`, `encoder 3 priority`, ...: the op and N, or None when the word is no compute kind.
    pub fn parse(words: &[&str]) -> Option<Result<(Op, u32)>> {
        let op = match *words.first()? {
            "adder" => Op::Adder,
            "subtractor" => Op::Subtractor,
            "incrementer" => Op::Incrementer,
            "comparator" => Op::Comparator,
            "mux" => Op::Mux,
            "demux" => Op::Demux,
            "decoder" => Op::Decoder,
            "encoder" if words.get(2) == Some(&"priority") => Op::PriorityEncoder,
            "encoder" => Op::Encoder,
            "binary-counter" => Op::BinaryCounter,
            _ => return None,
        };
        Some((|| {
            let n: u32 = words.get(1).ok_or_else(|| anyhow::anyhow!("circuit: {}: missing its N", words.join(" ")))?.parse().map_err(|_| anyhow::anyhow!("circuit: {}: bad N", words.join(" ")))?;
            if n == 0 || n > 16 {
                bail!("circuit: {}: N must be 1..16", words.join(" "));
            }
            // N = 1 where the kind is a study 4 gate or a wire
            if n == 1 && matches!(op, Op::Comparator | Op::Decoder | Op::Encoder | Op::PriorityEncoder) {
                bail!("circuit: {}: N = 1 is a study 4 gate (or a wire): use `gate <fn>`", words.join(" "));
            }
            Ok((op, n))
        })())
    }

    pub fn name(self) -> &'static str {
        match self {
            Op::Adder => "adder",
            Op::Subtractor => "subtractor",
            Op::Incrementer => "incrementer",
            Op::Comparator => "comparator",
            Op::Mux => "mux",
            Op::Demux => "demux",
            Op::Decoder => "decoder",
            Op::Encoder | Op::PriorityEncoder => "encoder",
            Op::BinaryCounter => "binary-counter",
        }
    }

    /// The kind as written on the `circuit:` line.
    pub fn kind_name(self, n: u32) -> String {
        match self {
            Op::PriorityEncoder => format!("encoder {n} priority"),
            _ => format!("{} {n}", self.name()),
        }
    }
}

fn bits(prefix: &str, n: u32) -> Vec<String> {
    (0..n).map(|k| format!("{prefix}{k}")).collect()
}

/// (required inputs, optional inputs, required outputs, optional outputs).
fn ports(op: Op, n: u32) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let w = 1u32 << n.min(16);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    match op {
        Op::Adder => ([bits("a", n), bits("b", n)].concat(), s(&["cin"]), bits("s", n), s(&["cout"])),
        Op::Subtractor => ([bits("a", n), bits("b", n)].concat(), s(&["bin"]), bits("d", n), s(&["bout"])),
        Op::Incrementer => (bits("a", n), s(&["cin"]), bits("s", n), s(&["cout"])),
        Op::Comparator => ([bits("a", n), bits("b", n)].concat(), Vec::new(), Vec::new(), s(&RELATIONS)),
        Op::Mux => ([bits("s", n), bits("d", w)].concat(), Vec::new(), s(&["q"]), Vec::new()),
        Op::Demux => ([bits("s", n), s(&["d"])].concat(), Vec::new(), bits("q", w), Vec::new()),
        Op::Decoder => (bits("a", n), s(&["e"]), bits("q", w), Vec::new()),
        Op::Encoder | Op::PriorityEncoder => (bits("d", w), Vec::new(), bits("q", n), s(&["valid"])),
        Op::BinaryCounter => (s(&["clk"]), s(&["r"]), bits("q", n), Vec::new()),
    }
}

/// The ports a compute kind needs (names: `input.a0`, `output.cout`, ...).
pub fn check_ports(c: &Circuit, op: Op, n: u32) -> Result<()> {
    let (ri, oi, ro, mut oo) = ports(op, n);
    if let Some(step) = c.slice {
        check_slice(c, op, n, step)?;
        if op == Op::BinaryCounter {
            oo.push("cout".into());
        }
    } else if op == Op::BinaryCounter && n == 1 {
        bail!("circuit: binary-counter 1: a 1-bit binary counter is a T flip-flop (study 2): circuit: tff (or a `slice:` design: one bit of a ripple counter)");
    }
    let ins: BTreeSet<&str> = c.inputs.iter().map(|i| i.name.as_str()).collect();
    let outs: BTreeSet<&str> = c.outputs.iter().map(|o| o.name.as_str()).collect();
    let kind = op.kind_name(n);
    for r in &ri {
        if !ins.contains(r.as_str()) {
            bail!("{kind}: needs inputs {} (port input.<name>), {r} is missing", ri.join(", "));
        }
    }
    for i in &ins {
        if !ri.iter().any(|r| r == i) && !oi.iter().any(|r| r == i) {
            bail!("{kind}: unknown input {i} (inputs {}{})", ri.join(", "), if oi.is_empty() { String::new() } else { format!(", optional {}", oi.join(", ")) });
        }
    }
    for r in &ro {
        if !outs.contains(r.as_str()) {
            bail!("{kind}: needs outputs {} (port output.<name>), {r} is missing", ro.join(", "));
        }
    }
    for o in &outs {
        if !ro.iter().any(|r| r == o) && !oo.iter().any(|r| r == o) {
            bail!("{kind}: unknown output {o} (outputs {}{})", ro.join(", "), if oo.is_empty() { String::new() } else { format!(", optional {}", oo.join(", ")) });
        }
    }
    if outs.is_empty() {
        bail!("{kind}: needs at least one output ({})", oo.join(", "));
    }
    if op != Op::BinaryCounter && ins.len() > MAX_INPUTS {
        bail!("{kind}: {} inputs is more than the {MAX_INPUTS} judged exhaustively: build it from bit slices", ins.len());
    }
    if !c.targets.is_empty() {
        bail!("{kind}: target ports are for the bud kind");
    }
    Ok(())
}

/// An input state: bit i = the circuit's i-th input (`Circuit::inputs` order).
type State = u32;

fn bit(c: &Circuit, x: State, name: &str) -> bool {
    c.inputs.iter().position(|i| i.name == name).is_some_and(|i| x >> i & 1 == 1)
}

fn value(c: &Circuit, x: State, prefix: &str, n: u32) -> u64 {
    (0..n).map(|k| (bit(c, x, &format!("{prefix}{k}")) as u64) << k).sum()
}

/// The ideal outputs (in `Circuit::outputs` order) for an input state.
pub fn ideal(c: &Circuit, op: Op, n: u32, x: State) -> Vec<bool> {
    let a = value(c, x, "a", n);
    let b = value(c, x, "b", n);
    let w = 1u64 << n;
    let out = |name: &str| -> bool {
        let k = |p: &str| name.strip_prefix(p).and_then(|r| r.parse::<u64>().ok());
        match op {
            Op::Adder | Op::Incrementer => {
                let cin = bit(c, x, "cin") as u64;
                let sum = if op == Op::Adder { a + b + cin } else if c.inputs.iter().any(|i| i.name == "cin") { a + cin } else { a + 1 };
                match (name, k("s")) {
                    ("cout", _) => sum >> n & 1 == 1,
                    (_, Some(j)) => sum >> j & 1 == 1,
                    _ => false,
                }
            }
            Op::Subtractor => {
                let diff = a as i64 - b as i64 - bit(c, x, "bin") as i64;
                match (name, k("d")) {
                    ("bout", _) => diff < 0,
                    (_, Some(j)) => (diff.rem_euclid(w as i64) as u64) >> j & 1 == 1,
                    _ => false,
                }
            }
            Op::Comparator => match name {
                "eq" => a == b,
                "ne" => a != b,
                "lt" => a < b,
                "gt" => a > b,
                "le" => a <= b,
                _ => a >= b,
            },
            Op::Mux => bit(c, x, &format!("d{}", value(c, x, "s", n))),
            Op::Demux => k("q") == Some(value(c, x, "s", n)) && bit(c, x, "d"),
            Op::Decoder => {
                let en = !c.inputs.iter().any(|i| i.name == "e") || bit(c, x, "e");
                k("q") == Some(a) && en
            }
            Op::Encoder | Op::PriorityEncoder => {
                let d = value(c, x, "d", 1 << n);
                let idx = if d == 0 { 0 } else { 63 - d.leading_zeros() as u64 };
                match (name, k("q")) {
                    ("valid", _) => d != 0,
                    (_, Some(j)) => idx >> j & 1 == 1,
                    _ => false,
                }
            }
            Op::BinaryCounter => false,
        }
    };
    c.outputs.iter().map(|o| out(&o.name)).collect()
}

/// Whether a state is part of the kind's domain (a plain encoder takes one-hot or zero inputs).
fn allowed(op: Op, x: State) -> bool {
    op != Op::Encoder || x.count_ones() <= 1
}

/// The visiting order: (section, state) after the rest state 0.
fn sequence(c: &Circuit, op: Op, short: bool) -> Vec<(&'static str, State)> {
    let m = c.inputs.len();
    let full: State = if m >= 32 { State::MAX } else { (1 << m) - 1 };
    let mut v: Vec<(&'static str, State)> = Vec::new();
    if op == Op::Encoder {
        // one-hot: each line on and off, then from line to line, then back to zero
        for i in 0..m {
            v.push(("gray", 1 << i));
            v.push(("gray", 0));
        }
        for i in 0..m {
            v.push(("jump", 1 << i));
        }
        v.push(("jump", 0));
        return v;
    }
    let gray: Vec<State> = (0..=full).map(|i| i ^ (i >> 1)).collect();
    let cap = if short { 64 } else { usize::MAX };
    v.extend(gray.iter().skip(1).take(cap).map(|x| ("gray", *x)));
    if short {
        v.extend(gray.iter().take(8).flat_map(|x| [("jump", !x & full), ("jump", *x)]));
        return v;
    }
    if m <= MAX_WALK_INPUTS {
        // Euler tour of the directed n-cube: every single-input change in both directions
        let mut next: Vec<usize> = vec![m; 1 << m];
        let mut stack: Vec<State> = vec![0];
        let mut path: Vec<State> = Vec::new();
        while let Some(&u) = stack.last() {
            let e = &mut next[u as usize];
            if *e == 0 {
                path.push(stack.pop().unwrap());
            } else {
                *e -= 1;
                stack.push(u ^ (1 << *e));
            }
        }
        path.reverse();
        let last = v.last().map(|p| p.1).unwrap_or(0);
        // the tour starts at 0: reach 0 from the end of the Gray code first
        if last != 0 {
            v.push(("walk", 0));
        }
        v.extend(path.into_iter().skip(1).map(|x| ("walk", x)));
    }
    // every state from its complement (every input at once)
    for x in &gray {
        v.push(("jump", !x & full));
        v.push(("jump", *x));
    }
    v.retain(|p| allowed(op, p.1));
    v.dedup_by_key(|p| p.1);
    v
}

/// The stimulus of a combinational kind (`gap` per state, at least 10 gt so that no torch toggles
/// 8 times in 60 gt from the walk alone). A sizing probe (gap >= 500) gets a short sequence.
pub fn stimulus(c: &Circuit, op: Op, n: u32, z: &Sizing) -> Schedule {
    if op == Op::BinaryCounter {
        return counter_stimulus(c, n, z);
    }
    let short = z.gap >= 500;
    let gap = z.gap.max(10);
    let mut s = Schedule::new(c.inputs.len());
    let mut t = c.clock.free_from() + SETTLE;
    let mut prev: State = 0;
    for (section, x) in sequence(c, op, short) {
        for i in 0..c.inputs.len() {
            if (x ^ prev) >> i & 1 == 1 {
                s.set(i, t, x >> i & 1 == 1);
            }
        }
        s.phase(t, t + gap, format!("{section} {x}"));
        t += gap;
        prev = x;
    }
    s.ticks = t;
    s
}

/// `a=0111 b=0000 cin=0`: a state by its input groups (MSB first).
fn describe(c: &Circuit, x: State) -> String {
    let mut groups: Vec<(String, Vec<(u32, bool)>)> = Vec::new();
    for (i, inp) in c.inputs.iter().enumerate() {
        let on = x >> i & 1 == 1;
        let split = inp.name.find(|ch: char| ch.is_ascii_digit());
        match split {
            Some(p) if p > 0 => {
                let (pre, k) = inp.name.split_at(p);
                let k: u32 = k.parse().unwrap_or(0);
                match groups.iter_mut().find(|g| g.0 == pre) {
                    Some(g) => g.1.push((k, on)),
                    None => groups.push((pre.to_string(), vec![(k, on)])),
                }
            }
            _ => groups.push((inp.name.clone(), vec![(u32::MAX, on)])),
        }
    }
    groups
        .into_iter()
        .map(|(name, mut v)| {
            v.sort_by(|a, b| b.0.cmp(&a.0));
            format!("{name}={}", v.iter().map(|(_, on)| if *on { '1' } else { '0' }).collect::<String>())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The transition from `p` to `x` in words.
fn transition(c: &Circuit, p: State, x: State) -> String {
    let changed: Vec<usize> = (0..c.inputs.len()).filter(|i| (p ^ x) >> i & 1 == 1).collect();
    match changed.as_slice() {
        [i] => format!("{} {} from {}", c.inputs[*i].name, if x >> i & 1 == 1 { "rises" } else { "falls" }, describe(c, p)),
        _ => format!("{} inputs change from {} to {}", changed.len(), describe(c, p), describe(c, x)),
    }
}

/// Judge a combinational run.
pub fn check(c: &Circuit, op: Op, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    if op == Op::BinaryCounter {
        return counter_check(c, n, s, t);
    }
    let mut table = Vec::new();
    let outs = c.outputs.len();
    let frames = t.frames();
    let vals = |f: usize| -> Vec<bool> { (0..outs).map(|k| t.on(k, f)).collect() };
    let names = |v: &[bool], e: &[bool]| -> String {
        (0..outs).filter(|k| v[*k] != e[*k]).map(|k| format!("{} is {}, want {}", c.outputs[k].name, v[k] as u8, e[k] as u8)).collect::<Vec<_>>().join("; ")
    };
    // at rest: the outputs of state 0
    let e0 = ideal(c, op, n, 0);
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(frames);
    for f in (c.clock.free_from() + SETTLE / 2) as usize..first.min(frames) {
        let v = vals(f);
        if v != e0 {
            return Outcome::fail(format!("wrong at rest ({}): {}", describe(c, 0), names(&v, &e0)), table);
        }
    }
    let mut prev: State = 0;
    let mut worst: (usize, String) = (0, String::new());
    let mut glitches = 0usize;
    let mut sections: Vec<(String, usize, usize)> = Vec::new();
    for (a, b, label) in &s.phases {
        let (a, b) = (*a as usize, (*b as usize).min(frames));
        let mut w = label.split_whitespace();
        let section = w.next().unwrap_or("").to_string();
        let x: State = w.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        let e = ideal(c, op, n, x);
        let ep = ideal(c, op, n, prev);
        if a >= b {
            break;
        }
        let end = vals(b - 1);
        if end != e {
            table.push(format!("  {section}: {} -> {}: {}", describe(c, prev), describe(c, x), names(&end, &e)));
            return Outcome::fail(format!("wrong output: {}: {}", transition(c, prev, x), names(&end, &e)), table);
        }
        // the first frame from which every output holds its final value
        let mut f0 = a;
        for f in (a..b).rev() {
            if vals(f) != e {
                f0 = f + 1;
                break;
            }
        }
        let lat = f0 - a;
        if lat > worst.0 || worst.1.is_empty() {
            worst = (lat, transition(c, prev, x));
        }
        // a glitch: an output changing more than its ideal once (or zero times) before it settles
        let mut glitch = false;
        for k in 0..outs {
            let mut last = if a > 0 { t.on(k, a - 1) } else { false };
            let mut changes = 0;
            for f in a..f0.max(a + 1).min(b) {
                let v = t.on(k, f);
                if v != last {
                    changes += 1;
                    last = v;
                }
            }
            if changes > usize::from(e[k] != ep[k]) {
                glitch = true;
            }
        }
        glitches += glitch as usize;
        match sections.iter_mut().find(|q| q.0 == section) {
            Some(q) => {
                q.1 += 1;
                q.2 = q.2.max(lat);
            }
            None => sections.push((section, 1, lat)),
        }
        prev = x;
    }
    for (name, count, lat) in &sections {
        table.push(format!("  {name}: {count} states, worst latency {lat} gt"));
    }
    let mut metrics = vec![("latency".to_string(), if worst.0 == 0 { "0 gt".to_string() } else { format!("{} gt ({})", worst.0, worst.1) })];
    if worst.0 == 0 {
        metrics.push(("class".to_string(), "instant".to_string()));
    }
    if glitches > 0 {
        metrics.push(("glitches".to_string(), glitches.to_string()));
    }
    let bits_per = match op {
        Op::Mux | Op::Demux | Op::Decoder | Op::Encoder | Op::PriorityEncoder => 0,
        _ => n,
    };
    if bits_per > 1 {
        metrics.push(("per bit".to_string(), format!("{:.1} blocks", c.blocks() as f64 / bits_per as f64)));
    }
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// Clock pulses of a binary counter (4 gt each, then the gap); with a reset input, resets at three
/// different counts. Phase labels: `clk <expected count>`, `reset 0`.
fn counter_stimulus(c: &Circuit, n: u32, z: &Sizing) -> Schedule {
    let gap = z.gap.max(10);
    let mut s = Schedule::new(c.inputs.len());
    let clk = c.inputs.iter().position(|i| i.name == "clk").unwrap_or(0);
    let rst = c.inputs.iter().position(|i| i.name == "r");
    let w = 1u32 << n;
    let mut t = c.clock.free_from() + SETTLE;
    let mut count = 0u32;
    let mut pulses = |s: &mut Schedule, t: &mut u32, count: &mut u32, k: u32| {
        for _ in 0..k {
            s.pulse(clk, *t, 4);
            *count = (*count + 1) % w;
            s.phase(*t, *t + 4 + gap, format!("clk {count}"));
            *t += 4 + gap;
        }
    };
    pulses(&mut s, &mut t, &mut count, w + 3);
    if let Some(r) = rst {
        for k in [3, w - 1] {
            s.pulse(r, t, 4);
            count = 0;
            s.phase(t, t + 4 + gap, "reset 0".to_string());
            t += 4 + gap;
            pulses(&mut s, &mut t, &mut count, k);
        }
        s.pulse(r, t, 4);
        s.phase(t, t + 4 + gap, "reset 0".to_string());
        t += 4 + gap;
    }
    s.ticks = t;
    s
}

/// Judge a binary counter: the outputs read the expected count at the end of every phase; the
/// latency is measured from the edge that moves them (rising: the pulse start, falling: its end).
fn counter_check(c: &Circuit, n: u32, s: &Schedule, t: &Trace) -> Outcome {
    let mut table = Vec::new();
    let frames = t.frames();
    // output index of bit k (outputs are sorted by name: q10 sorts before q2)
    let idx: Vec<usize> = (0..n).map(|k| c.outputs.iter().position(|o| o.name == format!("q{k}")).unwrap_or(0)).collect();
    let read = |f: usize| -> u32 { idx.iter().enumerate().map(|(k, i)| (t.on(*i, f) as u32) << k).sum() };
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(frames);
    for f in (c.clock.free_from() + SETTLE / 2) as usize..first.min(frames) {
        if read(f) != 0 {
            return Outcome::fail(format!("wrong at rest: the outputs read {}, want 0", read(f)), table);
        }
    }
    let mut worst = 0usize;
    let mut rising = 0usize;
    let mut falling = 0usize;
    for (a, b, label) in &s.phases {
        let (a, b) = (*a as usize, (*b as usize).min(frames));
        if a >= b {
            break;
        }
        let want: u32 = label.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        let got = read(b - 1);
        table.push(format!("  {label}: reads {got}"));
        if got != want {
            return Outcome::fail(format!("miscounts: after `{label}` the outputs read {got}, want {want}"), table);
        }
        let mut f0 = a;
        for f in (a..b).rev() {
            if read(f) != want {
                f0 = f + 1;
                break;
            }
        }
        // the first output change: before the pulse ends = the rising edge, else the falling one
        let changed = (a..b).find(|f| read(*f) != read(a.saturating_sub(1)));
        let edge = match changed {
            Some(f) if f < a + 4 => {
                rising += 1;
                a
            }
            Some(_) => {
                falling += 1;
                a + 4
            }
            None => a,
        };
        if label.starts_with("clk") {
            worst = worst.max(f0.saturating_sub(edge));
        }
    }
    let mut metrics = vec![("latency".to_string(), format!("{worst} gt"))];
    metrics.push(("edge".to_string(), if rising >= falling { "rising" } else { "falling" }.to_string()));
    if c.inputs.iter().any(|i| i.name == "r") {
        metrics.push(("reset".to_string(), "yes".to_string()));
    }
    metrics.push(("per bit".to_string(), format!("{:.1} blocks", c.blocks() as f64 / n as f64)));
    Outcome { pass: true, why: String::new(), metrics, table }
}

/// The copies a bit slice is stacked into, besides itself (N = 1).
pub const SLICE_COPIES: [u32; 3] = [2, 3, 4];

/// A bit slice's chain ports: (the chain input, the chain output) and the per-bit ports
/// (prefix of its inputs and outputs, renamed `<prefix><k>` in copy k).
fn chain(op: Op) -> Option<((&'static str, &'static str), &'static [&'static str], &'static [&'static str])> {
    match op {
        Op::Adder => Some((("cin", "cout"), &["a", "b"], &["s"])),
        Op::Subtractor => Some((("bin", "bout"), &["a", "b"], &["d"])),
        Op::Incrementer => Some((("cin", "cout"), &["a"], &["s"])),
        Op::BinaryCounter => Some((("clk", "cout"), &[], &["q"])),
        _ => None,
    }
}

/// A `slice:` design: one bit (N = 1) of a kind with a chain, its chain output a design block one
/// step from its chain input (an air cell the judge drives in copy 0, the previous copy's chain
/// output in the others).
fn check_slice(c: &Circuit, op: Op, n: u32, step: crate::Pos) -> Result<()> {
    let kind = op.kind_name(n);
    let Some(((cin, cout), _, _)) = chain(op) else {
        bail!("{kind}: slice: only adders, subtractors, incrementers and binary counters chain bit to bit");
    };
    if n != 1 {
        bail!("{kind}: slice: a slice is ONE bit (circuit: {} 1)", op.name());
    }
    let pi = c.inputs.iter().find(|i| i.name == cin).map(|i| i.pos);
    let po = c.outputs.iter().find(|o| o.name == cout).map(|o| o.pos);
    let (Some(pi), Some(po)) = (pi, po) else {
        bail!("{kind}: slice: needs the chain ports input.{cin} and output.{cout}");
    };
    if [pi[0] + step[0], pi[1] + step[1], pi[2] + step[2]] != po {
        bail!("{kind}: slice: output.{cout} at {po:?} must be input.{cin} + the step ({:?}): the next copy's {cin} is this copy's {cout}", [pi[0] + step[0], pi[1] + step[1], pi[2] + step[2]]);
    }
    if !c.clock.design.blocks.contains_key(&po) {
        bail!("{kind}: slice: output.{cout} at {po:?} must be a block of the slice (the next copy reads it as its {cin})");
    }
    Ok(())
}

/// The bit slice stacked into `copies` bits: copy k shifted by k steps, its per-bit ports renamed
/// `<prefix><k>`; copy 0's chain input stays the kind's input, the last copy's chain output its
/// output (a binary counter's is dropped).
pub fn sliced(c: &Circuit, op: Op, copies: u32) -> Result<Circuit> {
    let step = c.slice.ok_or_else(|| anyhow::anyhow!("{}: no `slice:` header", c.name()))?;
    let ((cin, cout), ins, outs) = chain(op).ok_or_else(|| anyhow::anyhow!("{}: not a chained kind", op.name()))?;
    let base = c.clock.design.clone();
    // the chain input cell holds the slice's idle state there (copy 0) or the previous copy's
    // chain output (the others)
    let cin_cell = base.ports.get(&format!("input.{cin}")).copied();
    let mut d = base.clone();
    d.meta.remove("slice");
    d.meta.retain(|k, _| !k.starts_with("port "));
    d.meta.insert("circuit".into(), op.kind_name(copies));
    d.ports.clear();
    for k in 0..copies as i32 {
        let off = [step[0] * k, step[1] * k, step[2] * k];
        let sh = |p: &crate::Pos| [p[0] + off[0], p[1] + off[1], p[2] + off[2]];
        if k > 0 {
            for (p, s) in &base.blocks {
                if Some(*p) == cin_cell {
                    continue;
                }
                let q = sh(p);
                if let Some(o) = d.blocks.get(&q) {
                    bail!("slice: copy {k} puts {} on {} at {q:?} (the copies overlap)", clock::short(s), clock::short(o));
                }
                d.blocks.insert(q, s.clone());
            }
            for (p, s) in &base.block_entities {
                d.block_entities.insert(sh(p), s.clone());
            }
            for e in &base.entities {
                let mut e2 = e.clone();
                for i in 0..3 {
                    e2.pos[i] += off[i] as f64;
                }
                d.entities.push(e2);
            }
        }
        for (name, p) in &base.ports {
            let (base_kind, sub) = name.split_once('.').unwrap_or((name.as_str(), ""));
            let renamed = match (base_kind, sub) {
                ("input", s) if s == cin => (k == 0).then(|| name.clone()),
                ("output", s) if s == cout => (k == copies as i32 - 1 && op != Op::BinaryCounter).then(|| name.clone()),
                ("input", s) if ins.iter().any(|pre| s == format!("{pre}0")) => Some(format!("input.{}{k}", &s[..s.len() - 1])),
                ("output", s) if outs.iter().any(|pre| s == format!("{pre}0")) => Some(format!("output.{}{k}", &s[..s.len() - 1])),
                ("start", _) => (k == 0).then(|| name.clone()),
                _ => bail!("slice: port {name}: not a per-bit or chain port of {}", op.name()),
            };
            if let Some(r) = renamed {
                if let Some(args) = base.meta.get(&format!("port {name}")) {
                    d.meta.insert(format!("port {r}"), args.clone());
                }
                d.ports.insert(r, sh(p));
            }
        }
    }
    // a port must not land on another copy's block (copy 0's chain input holds its own idle state)
    for (name, p) in &d.ports {
        if name.starts_with("input") && Some(*p) != cin_cell && d.blocks.contains_key(p) {
            bail!("slice: port {name} at {p:?} is covered by another copy");
        }
    }
    Circuit::from_design(d)
}

/// The leading number of an outcome's `latency` metric.
pub fn latency_of(o: &Outcome) -> u32 {
    o.metrics.iter().find(|(k, _)| k == "latency").and_then(|(_, v)| v.split_whitespace().next()?.parse().ok()).unwrap_or(0)
}

/// The frames each state is held in a compute schedule (its first phase).
pub fn gap_of(s: &Schedule) -> u32 {
    s.phases.first().map(|p| p.1 - p.0).unwrap_or(20)
}

/// A bit slice's stacks ([`sliced`], [`SLICE_COPIES`]) judged as their N-bit kind (`run` records a
/// stack's trace on redsim or in the game, `gap_for` sizes a stack's holds). Returns `base` (the
/// slice judged alone) with the `slice` metric (the latency law over N = 1..4), or why not.
pub fn slice_check(
    c: &Circuit,
    op: Op,
    base: &Outcome,
    gap_for: impl Fn(&Circuit) -> u32,
    mut run: impl FnMut(&Circuit, &Schedule) -> Result<std::result::Result<Trace, String>>,
) -> Result<Outcome> {
    let mut lats: Vec<(u32, u32)> = vec![(1, latency_of(base))];
    let mut table = base.table.clone();
    for n in SLICE_COPIES {
        let sc = sliced(c, op, n)?;
        let kind = crate::Kind::Compute(op, n);
        let s = crate::check::stimulus(&sc, &kind, &Sizing { gap: gap_for(&sc) });
        let t = match run(&sc, &s)? {
            Ok(t) => t,
            Err(why) => return Ok(Outcome::fail(format!("slice: {n} copies: {why}"), table)),
        };
        let o = crate::check::check(&sc, &kind, &s, &t);
        if !o.pass {
            return Ok(Outcome::fail(format!("slice: {n} copies ({}): {}", kind.name(), o.why), o.table));
        }
        table.push(format!("  slice x{n}: {}", o.metrics_text()));
        lats.push((n, latency_of(&o)));
    }
    let d: Vec<i64> = lats.windows(2).map(|w| w[1].1 as i64 - w[0].1 as i64).collect();
    let law = if d.iter().all(|x| *x == d[0]) {
        format!("latency {} + {} (N-1) gt", lats[0].1, d[0])
    } else {
        format!("latency {} gt", lats.iter().map(|(n, l)| format!("{l} at N={n}")).collect::<Vec<_>>().join(" / "))
    };
    let mut out = base.clone();
    out.table = table;
    out.metrics.push(("slice".to_string(), format!("{} blocks per bit, {law}", c.blocks())));
    Ok(out)
}
