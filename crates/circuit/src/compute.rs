//! Study 12, computational redstone (designs/circuits/compute/VARIATIONS.md): binary adders,
//! subtractors, incrementers, comparators, multiplexers, demultiplexers, decoders and encoders,
//! judged on their EXHAUSTIVE truth table, and binary counters with a parallel binary output.
//!
//! Ports: bit k of a value `a` is `input.a<k>` (k = 0: the lowest bit), outputs likewise
//! (`output.s<k>`); single ports by name (`cin`, `cout`, `eq`, `q`...). Unsigned arithmetic. A
//! `hex` kind (`adder 4 hex`) carries each value on ONE line as a signal strength instead: inputs
//! `port input.a: x y z strength` (a chest the judge fills: [`strength_items`]), the value outputs
//! (`s`, `d`) read as levels that must equal the value.
//!
//! The combinational stimulus visits every input state in Gray-code order, then every
//! single-input transition in both directions (an Euler tour of the n-cube, n <= 10 input bits),
//! then every state reached from its complement (every input changing at once: a parallel load),
//! each state held for the gap. Every output must settle to the ideal value and hold it to the
//! end of each hold. `latency`: the worst gt from an input change to the frame where every output
//! holds its final value, with the transition that takes it; `glitches`: holds where an output
//! changes more than once before it settles.

use crate::check::{Outcome, Sizing};
use crate::stimulus::{SETTLE, Schedule};
use crate::trace::Trace;
use crate::{Circuit, InputHow, Pos};
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
    /// Study 15: a 7-segment display decoder: a 4-bit value to the segments a..g of its hex digit
    /// ([`SEGMENTS`]).
    SevenSegment,
}

/// The lit segments of each hex digit 0..F (a top, b upper right, c lower right, d bottom, e lower
/// left, f upper left, g middle): the usual font, 6 and 9 with their tails, 7 without f, b and d
/// in lower case.
pub const SEGMENTS: [&str; 16] = ["abcdef", "bc", "abdeg", "abcdg", "bcfg", "acdfg", "acdefg", "abc", "abcdefg", "abcdfg", "abcefg", "cdefg", "adef", "bcdeg", "adefg", "aefg"];

/// A compute kind: the function, N (bits), and the value encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spec {
    pub op: Op,
    pub n: u32,
    /// Values as signal strengths on one line each (`adder 4 hex`), not one line per bit.
    pub hex: bool,
}

/// The most input BITS judged exhaustively (2^16 states); bigger ripple designs are bit slices.
pub const MAX_INPUTS: usize = 16;
/// The most input bits for which every single-input transition is visited (n 2^n holds).
pub const MAX_WALK_INPUTS: usize = 10;
/// The comparator outputs.
pub const RELATIONS: [&str; 6] = ["eq", "ne", "lt", "gt", "le", "ge"];
/// The item a strength input's chest holds (a 64-stack).
pub const STRENGTH_ITEM: &str = "minecraft:cobblestone";

impl Op {
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
            Op::SevenSegment => "seven-segment",
        }
    }
}

impl Spec {
    /// `adder 4`, `adder 4 hex`, `encoder 3 priority`, ...: the spec, or None when the first word
    /// is no compute kind.
    pub fn parse(words: &[&str]) -> Option<Result<Spec>> {
        let op = match *words.first()? {
            "adder" => Op::Adder,
            "subtractor" => Op::Subtractor,
            "incrementer" => Op::Incrementer,
            "comparator" => Op::Comparator,
            "mux" => Op::Mux,
            "demux" => Op::Demux,
            "decoder" => Op::Decoder,
            "encoder" if words.contains(&"priority") => Op::PriorityEncoder,
            "encoder" => Op::Encoder,
            "binary-counter" => Op::BinaryCounter,
            "seven-segment" => Op::SevenSegment,
            _ => return None,
        };
        let line = words.join(" ");
        Some((|| {
            let n: u32 = words.get(1).ok_or_else(|| anyhow::anyhow!("circuit: {line}: missing its N"))?.parse().map_err(|_| anyhow::anyhow!("circuit: {line}: bad N"))?;
            if n == 0 || n > 16 {
                bail!("circuit: {line}: N must be 1..16");
            }
            let hex = words.contains(&"hex");
            for w in &words[2..] {
                if !matches!(*w, "hex" | "priority") || (*w == "priority" && op != Op::PriorityEncoder) {
                    bail!("circuit: {line}: unknown modifier {w:?}");
                }
            }
            if hex && !matches!(op, Op::Adder | Op::Subtractor | Op::Incrementer | Op::Comparator) {
                bail!("circuit: {line}: hex (values as signal strengths) is for adders, subtractors, incrementers and comparators");
            }
            if hex && n > 4 {
                bail!("circuit: {line}: a signal strength carries 4 bits at most (0..15)");
            }
            // N = 1 where the kind is a study 4 gate or a wire
            if n == 1 && !hex && matches!(op, Op::Comparator | Op::Decoder | Op::Encoder | Op::PriorityEncoder) {
                bail!("circuit: {line}: N = 1 is a study 4 gate (or a wire): use `gate <fn>`");
            }
            if op == Op::SevenSegment && n != 4 {
                bail!("circuit: {line}: a 7-segment decoder takes a 4-bit value (seven-segment 4)");
            }
            Ok(Spec { op, n, hex })
        })())
    }

    /// The kind as written on the `circuit:` line.
    pub fn kind_name(&self) -> String {
        let base = match self.op {
            Op::PriorityEncoder => format!("encoder {} priority", self.n),
            op => format!("{} {}", op.name(), self.n),
        };
        if self.hex { format!("{base} hex") } else { base }
    }

    fn with_n(&self, n: u32) -> Spec {
        Spec { n, ..*self }
    }

    /// Whether an output carries a value as a level (a hex kind's `s` / `d`).
    fn level_output(&self, name: &str) -> bool {
        self.hex && matches!(name, "s" | "d")
    }
}

fn bits(prefix: &str, n: u32) -> Vec<String> {
    (0..n).map(|k| format!("{prefix}{k}")).collect()
}

/// (required inputs, optional inputs, required outputs, optional outputs).
fn ports(spec: &Spec) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
    let n = spec.n;
    let w = 1u32 << n.min(16);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    // a value: n bit lines, or one strength line (hex)
    let val = |p: &str| if spec.hex { vec![p.to_string()] } else { bits(p, n) };
    match spec.op {
        Op::Adder => ([val("a"), val("b")].concat(), s(&["cin"]), val("s"), s(&["cout"])),
        Op::Subtractor => ([val("a"), val("b")].concat(), s(&["bin"]), val("d"), s(&["bout"])),
        Op::Incrementer => (val("a"), s(&["cin"]), val("s"), s(&["cout"])),
        Op::Comparator => ([val("a"), val("b")].concat(), Vec::new(), Vec::new(), s(&RELATIONS)),
        Op::Mux => ([bits("s", n), bits("d", w)].concat(), Vec::new(), s(&["q"]), Vec::new()),
        Op::Demux => ([bits("s", n), s(&["d"])].concat(), Vec::new(), bits("q", w), Vec::new()),
        Op::Decoder => (bits("a", n), s(&["e"]), bits("q", w), Vec::new()),
        Op::Encoder | Op::PriorityEncoder => (bits("d", w), Vec::new(), bits("q", n), s(&["valid"])),
        Op::BinaryCounter => (s(&["clk"]), s(&["r"]), bits("q", n), Vec::new()),
        Op::SevenSegment => (bits("a", n), Vec::new(), s(&["a", "b", "c", "d", "e", "f", "g"]), Vec::new()),
    }
}

/// The ports a compute kind needs (names: `input.a0`, `output.cout`, ...).
pub fn check_ports(c: &Circuit, spec: Spec) -> Result<()> {
    let (ri, oi, ro, mut oo) = ports(&spec);
    let kind = spec.kind_name();
    if let Some(step) = c.slice {
        check_slice(c, spec, step)?;
        if spec.op == Op::BinaryCounter {
            oo.push("cout".into());
        }
    } else if spec.op == Op::BinaryCounter && spec.n == 1 {
        bail!("circuit: binary-counter 1: a 1-bit binary counter is a T flip-flop (study 2): circuit: tff (or a `slice:` design: one bit of a ripple counter)");
    }
    let ins: BTreeSet<&str> = c.inputs.iter().map(|i| i.name.as_str()).collect();
    let outs: BTreeSet<&str> = c.outputs.iter().map(|o| o.name.as_str()).collect();
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
    // a hex kind's values are strength inputs, its carries plain lines
    for i in &c.inputs {
        let strength = matches!(i.how, InputHow::Strength { .. });
        let value = spec.hex && matches!(i.name.as_str(), "a" | "b");
        if strength != value {
            bail!("{kind}: input {}: {}", i.name, if value { "a hex value is a strength input (port input.<name>: x y z strength, on a chest of the design)" } else { "strength inputs are a hex kind's values (a, b)" });
        }
    }
    if spec.op != Op::BinaryCounter && layout(c, &spec).iter().map(|f| f.width as usize).sum::<usize>() > MAX_INPUTS {
        bail!("{kind}: more than {MAX_INPUTS} input bits is more than the judge walks exhaustively: build it from bit slices");
    }
    if !c.targets.is_empty() {
        bail!("{kind}: target ports are for the bud kind");
    }
    if spec.op == Op::SevenSegment {
        display_face(c)?;
    }
    Ok(())
}

/// The standard 3-wide x 5-tall figure-8 of a 7-segment digit: each segment's (column, row from
/// the top) as seen from the front.
pub const FACE: [(&str, i32, i32); 7] = [("a", 1, 0), ("f", 0, 1), ("b", 2, 1), ("g", 1, 2), ("e", 0, 3), ("c", 2, 3), ("d", 1, 4)];

/// Study 15, a display a player can read (vjeux 2026-10-21: "the output is in the middle of the
/// redstone contraption, how do you feed it to the display?"): the seven outputs are LAMPS (an air
/// output cell takes the judge's lamp, or the design's own redstone lamp) forming [`FACE`] on ONE
/// vertical exterior face, read from outside (top row up, `f` on the viewer's left); no design
/// block and no judge input device in front of the face (the cells of the face's plane between the
/// lamps may hold design blocks). Returns the face's outward normal.
pub fn display_face(c: &Circuit) -> Result<Pos> {
    let pos = |n: &str| c.outputs.iter().find(|o| o.name == n).map(|o| o.pos);
    for (n, _, _) in FACE {
        let p = pos(n).ok_or_else(|| anyhow::anyhow!("seven-segment: output {n} is missing"))?;
        match c.clock.design.blocks.get(&p) {
            None => {}
            Some(s) if s.starts_with("minecraft:redstone_lamp") => {}
            Some(s) => bail!("seven-segment: output {n} at {p:?} holds {}: the segments are lamps (leave the cell air for the judge's lamp, or put a redstone_lamp there)", clock::short(s)),
        }
    }
    // the four horizontal faces: outward normal and the viewer's right (looking at the face from
    // outside, along -normal)
    let faces: [(Pos, Pos); 4] = [([0, 0, -1], [-1, 0, 0]), ([0, 0, 1], [1, 0, 0]), ([-1, 0, 0], [0, 0, 1]), ([1, 0, 0], [0, 0, -1])];
    let a = pos("a").unwrap();
    let mut why = String::new();
    for (n, right) in faces {
        // a = origin + 1 * right + 0 rows down: the origin is the top-left cell
        let origin = [a[0] - right[0], a[1], a[2] - right[2]];
        let at = |col: i32, row: i32| [origin[0] + col * right[0], origin[1] - row, origin[2] + col * right[2]];
        if FACE.iter().any(|(s, col, row)| pos(s) != Some(at(*col, *row))) {
            continue;
        }
        // nothing in front: no design block (nor an input device) on the outer side of the plane
        let dot = |p: &Pos| (p[0] - origin[0]) * n[0] + (p[2] - origin[2]) * n[2];
        let mut devices: Vec<Pos> = Vec::new();
        for i in &c.inputs {
            if let InputHow::Driven { dir, .. } = i.how {
                let (r, sup, sw) = crate::driver_cells(i.pos, dir);
                devices.extend([r, sup, sw]);
            }
        }
        let face_cols = |p: &Pos| {
            let col = (p[0] - origin[0]) * right[0] + (p[2] - origin[2]) * right[2];
            (0..=2).contains(&col) && (origin[1] - 4..=origin[1]).contains(&p[1])
        };
        if let Some((p, s)) = c.clock.design.blocks.iter().find(|(p, _)| dot(p) > 0) {
            why = format!("{} at {p:?} stands in front of the display face", clock::short(s));
            continue;
        }
        if let Some(p) = devices.iter().find(|p| dot(p) > 0 && face_cols(p)) {
            why = format!("an input device at {p:?} stands in front of the display face");
            continue;
        }
        return Ok(n);
    }
    if why.is_empty() {
        why = "the outputs do not form the 3x5 figure-8 on one vertical face, read from outside: (column, row from the top) a (1,0), f (0,1), b (2,1), g (1,2), e (0,3), c (2,3), d (1,4), f on the viewer's left".into();
    }
    bail!("seven-segment: {why}")
}

/// An input state: the bits of every input, in `Circuit::inputs` order ([`layout`]).
type State = u32;

/// Where an input lives in a state: from bit `offset`, `width` bits (a strength input: n bits).
#[derive(Clone, Copy, Debug)]
struct Field {
    offset: u32,
    width: u32,
}

fn layout(c: &Circuit, spec: &Spec) -> Vec<Field> {
    let mut off = 0;
    c.inputs
        .iter()
        .map(|i| {
            let width = if matches!(i.how, InputHow::Strength { .. }) { spec.n } else { 1 };
            let f = Field { offset: off, width };
            off += width;
            f
        })
        .collect()
}

fn field(lay: &[Field], x: State, i: usize) -> u64 {
    let f = lay[i];
    ((x >> f.offset) & ((1 << f.width) - 1)) as u64
}

fn bit(c: &Circuit, lay: &[Field], x: State, name: &str) -> bool {
    c.inputs.iter().position(|i| i.name == name).is_some_and(|i| field(lay, x, i) != 0)
}

/// A value: the strength input `prefix` (hex) or the bit lines `<prefix>0..`.
fn value(c: &Circuit, lay: &[Field], spec: &Spec, x: State, prefix: &str, n: u32) -> u64 {
    if spec.hex && let Some(i) = c.inputs.iter().position(|i| i.name == prefix) {
        return field(lay, x, i);
    }
    (0..n).map(|k| (bit(c, lay, x, &format!("{prefix}{k}")) as u64) << k).sum()
}

/// The ideal outputs (in `Circuit::outputs` order) for an input state: 0/1, or a level for a hex
/// kind's value outputs.
pub fn ideal(c: &Circuit, spec: &Spec, x: State) -> Vec<u8> {
    let lay = layout(c, spec);
    let n = spec.n;
    let a = value(c, &lay, spec, x, "a", n);
    let b = value(c, &lay, spec, x, "b", n);
    let w = 1u64 << n;
    let has = |name: &str| c.inputs.iter().any(|i| i.name == name);
    let out = |name: &str| -> u64 {
        let k = |p: &str| name.strip_prefix(p).and_then(|r| r.parse::<u64>().ok());
        let flag = |v: bool| v as u64;
        match spec.op {
            Op::Adder | Op::Incrementer => {
                let cin = bit(c, &lay, x, "cin") as u64;
                let sum = if spec.op == Op::Adder { a + b + cin } else if has("cin") { a + cin } else { a + 1 };
                match (name, k("s")) {
                    ("cout", _) => sum >> n & 1,
                    ("s", _) => sum % w,
                    (_, Some(j)) => sum >> j & 1,
                    _ => 0,
                }
            }
            Op::Subtractor => {
                let diff = a as i64 - b as i64 - bit(c, &lay, x, "bin") as i64;
                let m = diff.rem_euclid(w as i64) as u64;
                match (name, k("d")) {
                    ("bout", _) => flag(diff < 0),
                    ("d", _) => m,
                    (_, Some(j)) => m >> j & 1,
                    _ => 0,
                }
            }
            Op::Comparator => flag(match name {
                "eq" => a == b,
                "ne" => a != b,
                "lt" => a < b,
                "gt" => a > b,
                "le" => a <= b,
                _ => a >= b,
            }),
            Op::Mux => flag(bit(c, &lay, x, &format!("d{}", value(c, &lay, spec, x, "s", n)))),
            Op::Demux => flag(k("q") == Some(value(c, &lay, spec, x, "s", n)) && bit(c, &lay, x, "d")),
            Op::Decoder => flag(k("q") == Some(a) && (!has("e") || bit(c, &lay, x, "e"))),
            Op::Encoder | Op::PriorityEncoder => {
                let d = value(c, &lay, spec, x, "d", 1 << n);
                let idx = if d == 0 { 0 } else { 63 - d.leading_zeros() as u64 };
                match (name, k("q")) {
                    ("valid", _) => flag(d != 0),
                    (_, Some(j)) => idx >> j & 1,
                    _ => 0,
                }
            }
            Op::BinaryCounter => 0,
            Op::SevenSegment => flag(name.len() == 1 && SEGMENTS[(a % 16) as usize].contains(name)),
        }
    };
    c.outputs.iter().map(|o| out(&o.name) as u8).collect()
}

/// Whether a state is part of the kind's domain (a plain encoder takes one-hot or zero inputs).
fn allowed(spec: &Spec, x: State) -> bool {
    spec.op != Op::Encoder || x.count_ones() <= 1
}

/// The visiting order: (section, state) after the rest state 0.
fn sequence(c: &Circuit, spec: &Spec, short: bool) -> Vec<(&'static str, State)> {
    let m: usize = layout(c, spec).iter().map(|f| f.width as usize).sum();
    let full: State = if m >= 32 { State::MAX } else { (1 << m) - 1 };
    let mut v: Vec<(&'static str, State)> = Vec::new();
    if spec.op == Op::Encoder {
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
        // Euler tour of the directed n-cube: every single-bit change in both directions
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
    v.retain(|p| allowed(spec, p.1));
    v.dedup_by_key(|p| p.1);
    v
}

/// The SNBT of a strength input's chest for a value: the fewest items of [`STRENGTH_ITEM`] whose
/// comparator strength is the value (`getRedstoneSignalFromContainer`: floor(14 f) + 1 for any
/// fill f > 0, f = the mean slot fill of 27 slots of 64).
pub fn strength_items(v: u8) -> String {
    let v = v.min(15) as u64;
    let n = if v == 0 { 0 } else { ((v - 1) * 1728).div_ceil(14).max(1) };
    let mut items = Vec::new();
    let mut left = n;
    let mut slot = 0;
    while left > 0 {
        let k = left.min(64);
        items.push(format!("{{Slot:{slot}b,id:\"{STRENGTH_ITEM}\",count:{k}}}"));
        left -= k;
        slot += 1;
    }
    format!("{{Items:[{}]}}", items.join(","))
}

/// The stimulus of a combinational kind (`gap` per state, at least 10 gt so that no torch toggles
/// 8 times in 60 gt from the walk alone). A sizing probe (gap >= 500) gets a short sequence.
pub fn stimulus(c: &Circuit, spec: Spec, z: &Sizing) -> Schedule {
    if spec.op == Op::BinaryCounter {
        return counter_stimulus(c, spec.n, z);
    }
    let short = z.gap >= 500;
    let gap = z.gap.max(10);
    let lay = layout(c, &spec);
    let mut s = Schedule::new(c.inputs.len());
    let mut t = c.clock.free_from() + SETTLE;
    let mut prev: State = 0;
    for (section, x) in sequence(c, &spec, short) {
        for i in 0..c.inputs.len() {
            let (v, pv) = (field(&lay, x, i), field(&lay, prev, i));
            if v == pv {
                continue;
            }
            if matches!(c.inputs[i].how, InputHow::Strength { .. }) {
                s.levels.push((t, i, v as u8));
            } else {
                s.set(i, t, v != 0);
            }
        }
        s.phase(t, t + gap, format!("{section} {x}"));
        t += gap;
        prev = x;
    }
    s.ticks = t;
    s
}

/// `a=0111 b=0000 cin=0` (a hex value in decimal: `a=7`): a state by its input groups.
fn describe(c: &Circuit, spec: &Spec, x: State) -> String {
    let lay = layout(c, spec);
    let mut groups: Vec<(String, Vec<(u32, bool)>)> = Vec::new();
    let mut words: Vec<(String, String)> = Vec::new();
    for (i, inp) in c.inputs.iter().enumerate() {
        if matches!(inp.how, InputHow::Strength { .. }) {
            words.push((inp.name.clone(), field(&lay, x, i).to_string()));
            continue;
        }
        let on = field(&lay, x, i) != 0;
        match inp.name.find(|ch: char| ch.is_ascii_digit()) {
            Some(p) if p > 0 => {
                let (pre, k) = inp.name.split_at(p);
                let k: u32 = k.parse().unwrap_or(0);
                match groups.iter_mut().find(|g| g.0 == pre) {
                    Some(g) => g.1.push((k, on)),
                    None => groups.push((pre.to_string(), vec![(k, on)])),
                }
            }
            _ => words.push((inp.name.clone(), if on { "1".into() } else { "0".into() })),
        }
    }
    for (name, mut v) in groups {
        v.sort_by(|a, b| b.0.cmp(&a.0));
        words.push((name, v.iter().map(|(_, on)| if *on { '1' } else { '0' }).collect()));
    }
    words.sort_by(|a, b| a.0.cmp(&b.0));
    words.into_iter().map(|(n, v)| format!("{n}={v}")).collect::<Vec<_>>().join(" ")
}

/// The transition from `p` to `x` in words.
fn transition(c: &Circuit, spec: &Spec, p: State, x: State) -> String {
    let lay = layout(c, spec);
    let changed: Vec<usize> = (0..c.inputs.len()).filter(|i| field(&lay, p, *i) != field(&lay, x, *i)).collect();
    match changed.as_slice() {
        [i] if lay[*i].width == 1 => format!("{} {} from {}", c.inputs[*i].name, if field(&lay, x, *i) != 0 { "rises" } else { "falls" }, describe(c, spec, p)),
        [i] => format!("{} goes {} -> {} from {}", c.inputs[*i].name, field(&lay, p, *i), field(&lay, x, *i), describe(c, spec, p)),
        _ => format!("{} inputs change from {} to {}", changed.len(), describe(c, spec, p), describe(c, spec, x)),
    }
}

/// Judge a combinational run.
pub fn check(c: &Circuit, spec: Spec, s: &Schedule, t: &Trace) -> Outcome {
    if spec.op == Op::BinaryCounter {
        return counter_check(c, spec.n, s, t);
    }
    let mut table = Vec::new();
    let outs = c.outputs.len();
    let frames = t.frames();
    let level_out: Vec<bool> = c.outputs.iter().map(|o| spec.level_output(&o.name)).collect();
    let vals = |f: usize| -> Vec<u8> { (0..outs).map(|k| if level_out[k] { t.levels[k].get(f).copied().unwrap_or(0) } else { t.on(k, f) as u8 }).collect() };
    let names = |v: &[u8], e: &[u8]| -> String {
        (0..outs).filter(|k| v[*k] != e[*k]).map(|k| format!("{} is {}, want {}", c.outputs[k].name, v[k], e[k])).collect::<Vec<_>>().join("; ")
    };
    // at rest: the outputs of state 0
    let e0 = ideal(c, &spec, 0);
    let first = s.phases.first().map(|p| p.0 as usize).unwrap_or(frames);
    for f in (c.clock.free_from() + SETTLE / 2) as usize..first.min(frames) {
        let v = vals(f);
        if v != e0 {
            return Outcome::fail(format!("wrong at rest ({}): {}", describe(c, &spec, 0), names(&v, &e0)), table);
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
        let e = ideal(c, &spec, x);
        let ep = ideal(c, &spec, prev);
        if a >= b {
            break;
        }
        let end = vals(b - 1);
        if end != e {
            table.push(format!("  {section}: {} -> {}: {}", describe(c, &spec, prev), describe(c, &spec, x), names(&end, &e)));
            return Outcome::fail(format!("wrong output: {}: {}", transition(c, &spec, prev, x), names(&end, &e)), table);
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
            worst = (lat, transition(c, &spec, prev, x));
        }
        // a glitch: an output changing more than its ideal once (or zero times) before it settles
        let mut glitch = false;
        for k in 0..outs {
            let at = |f: usize| if level_out[k] { t.levels[k].get(f).copied().unwrap_or(0) } else { t.on(k, f) as u8 };
            let mut last = if a > 0 { at(a - 1) } else { 0 };
            let mut changes = 0;
            for f in a..f0.max(a + 1).min(b) {
                let v = at(f);
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
    let arithmetic = matches!(spec.op, Op::Adder | Op::Subtractor | Op::Incrementer | Op::Comparator);
    if arithmetic && spec.n > 1 {
        metrics.push(("per bit".to_string(), format!("{:.1} blocks", c.blocks() as f64 / spec.n as f64)));
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
    let pulses = |s: &mut Schedule, t: &mut u32, count: &mut u32, k: u32| {
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
fn check_slice(c: &Circuit, spec: Spec, step: crate::Pos) -> Result<()> {
    let kind = spec.kind_name();
    let Some(((cin, cout), _, _)) = chain(spec.op) else {
        bail!("{kind}: slice: only adders, subtractors, incrementers and binary counters chain bit to bit");
    };
    if spec.n != 1 || spec.hex {
        bail!("{kind}: slice: a slice is ONE binary bit (circuit: {} 1)", spec.op.name());
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
pub fn sliced(c: &Circuit, spec: Spec, copies: u32) -> Result<Circuit> {
    let op = spec.op;
    let step = c.slice.ok_or_else(|| anyhow::anyhow!("{}: no `slice:` header", c.name()))?;
    let ((cin, cout), ins, outs) = chain(op).ok_or_else(|| anyhow::anyhow!("{}: not a chained kind", op.name()))?;
    let base = c.clock.design.clone();
    // the chain input cell holds the slice's idle state there (copy 0) or the previous copy's
    // chain output (the others)
    let cin_cell = base.ports.get(&format!("input.{cin}")).copied();
    let mut d = base.clone();
    d.meta.remove("slice");
    d.meta.retain(|k, _| !k.starts_with("port "));
    d.meta.insert("circuit".into(), spec.with_n(copies).kind_name());
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
    spec: Spec,
    base: &Outcome,
    gap_for: impl Fn(&Circuit) -> u32,
    mut run: impl FnMut(&Circuit, &Schedule) -> Result<std::result::Result<Trace, String>>,
) -> Result<Outcome> {
    let mut lats: Vec<(u32, u32)> = vec![(1, latency_of(base))];
    let mut table = base.table.clone();
    for n in SLICE_COPIES {
        let sc = sliced(c, spec, n)?;
        let kind = crate::Kind::Compute(spec.with_n(n));
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

#[cfg(test)]
mod face_tests {
    use crate::Circuit;

    /// A design whose outputs form the face (looking north at the z = -1 face: right = -x) with
    /// the inputs on the east side; `mirror` swaps the columns (f on the viewer's right).
    fn design(mirror: bool) -> String {
        let col = |c: i32| if mirror { c - 2 } else { -c };
        let face = [("a", 1, 0), ("f", 0, 1), ("b", 2, 1), ("g", 1, 2), ("e", 0, 3), ("c", 2, 3), ("d", 1, 4)];
        let mut s = String::from("name: face-test\ncircuit: seven-segment 4\n");
        for (k, z) in [(0, 0), (1, 1), (2, 2), (3, 3)] {
            s.push_str(&format!("port input.a{k}: 1 0 {z}\n"));
        }
        for (n, c, r) in face {
            s.push_str(&format!("port output.{n}: {} {} -1\n", col(c), 4 - r));
        }
        s.push_str("origin: -2 0 0\npalette:\n  S minecraft:stone\nlayer 0:\n  SSS\n");
        s
    }

    #[test]
    fn face_accepted_and_mirror_refused() {
        let ok = Circuit::from_design(mctas::design::Design::parse(&design(false)).unwrap());
        assert!(ok.is_ok(), "{:?}", ok.err());
        let bad = Circuit::from_design(mctas::design::Design::parse(&design(true)).unwrap());
        assert!(bad.is_err());
    }
}
