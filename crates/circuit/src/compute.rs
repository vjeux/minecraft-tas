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
    /// Study 15's STORAGE COUNTER (clk-obs, coordinator 2026-10-21): a chest's fill, as its
    /// comparator reads it (0..15), on two decimal digits ([`Table`]): the ones digit's segments
    /// a..g, the tens digit's ta..tg (blank below 10).
    StorageCounter(Table),
}

/// What a storage counter may show for a comparator level L (0..15). One comparator has 16
/// readings: L = 0 empty, L = 15 full, and L in 1..14 a fill f in [(L-1)/14, L/14)
/// (`getRedstoneSignalFromContainer`: floor(14 f) + 1). The display is correct when the number it
/// shows is one the true count rounds to for SOME fill with that reading ([`Table::allowed`]), and
/// the same number every time the level comes back: each design picks its own table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Table {
    /// Full stacks' worth of a 27-slot chest (27 f), 0..27.
    Stacks,
    /// Percent full (100 f), 0..99: full shows 99 (two digits).
    Percent,
}

impl Table {
    pub fn name(self) -> &'static str {
        match self {
            Table::Stacks => "stacks",
            Table::Percent => "percent",
        }
    }

    pub fn parse(w: &str) -> Option<Table> {
        [Table::Stacks, Table::Percent].into_iter().find(|t| t.name() == w)
    }

    /// The numbers allowed at comparator level `l`: p with p - 1/2 <= K f < p + 1/2 for some f
    /// with that reading (K = 27 stacks or 100 %), at least 1 for a non-empty chest, at most 99;
    /// full: K (27 stacks, 99 %).
    pub fn allowed(self, l: u64) -> std::ops::RangeInclusive<u64> {
        let k: i64 = match self {
            Table::Stacks => 27,
            Table::Percent => 100,
        };
        match l.min(15) as i64 {
            0 => 0..=0,
            15 => {
                let m = k.min(99) as u64;
                m..=m
            }
            l => {
                // 28 p > 2 k (l - 1) - 14 and 28 p < 2 k l + 14
                let lo = (2 * k * (l - 1) - 14).div_euclid(28) + 1;
                let hi = (2 * k * l + 14 - 1).div_euclid(28);
                (lo.max(1) as u64)..=(hi.min(99) as u64)
            }
        }
    }
}

/// The digit a lit set of segments shows (the decimal font of [`SEGMENTS`]), if any.
fn digit_of(lit: &str) -> Option<u8> {
    let mut l: Vec<char> = lit.chars().collect();
    l.sort();
    (0..10u8).find(|d| {
        let mut s: Vec<char> = SEGMENTS[*d as usize].chars().collect();
        s.sort();
        s == l
    })
}

/// `display counter-probe` (a storage counter's layout work, clk-obs): redsim holds the chest at
/// every level 0..15 in turn (`hold` gt each, up and then down) and reads the lamps at the end of
/// each hold. One line per level: the number shown (or the lit segments when they show none), and
/// against `want` (the intended table) the segments that differ.
pub fn counter_probe(c: &Circuit, want: Option<&[u64; 16]>, hold: u32, margin: i32) -> Result<String> {
    let chest = c
        .inputs
        .iter()
        .position(|i| i.name == "chest" || matches!(i.how, InputHow::Strength { .. }))
        .ok_or_else(|| anyhow::anyhow!("counter-probe: no strength input"))?;
    let mut s = Schedule::new(c.inputs.len());
    let mut t = c.clock.free_from() + SETTLE;
    let order: Vec<u8> = (1..16u8).chain((0..15u8).rev()).collect();
    for &l in &order {
        s.levels.push((t, chest, l));
        s.phase(t, t + hold, format!("level {l}"));
        t += hold;
    }
    s.ticks = t;
    let tr = crate::run::sim_trace(c, &s, margin).map_err(|e| anyhow::anyhow!("counter-probe: {e}"))?;
    let lit_at = |f: usize, pre: &str| -> String { "abcdefg".chars().filter(|ch| c.outputs.iter().position(|o| o.name == format!("{pre}{ch}")).is_some_and(|k| tr.on(k, f))).collect() };
    let mut out = String::new();
    let mut rest = vec![(0u8, (c.clock.free_from() + SETTLE - 1) as usize)];
    rest.extend(s.phases.iter().zip(&order).map(|((_, b, _), l)| (*l, (*b - 1) as usize)));
    for (l, f) in rest {
        let (tens, ones) = (lit_at(f, "t"), lit_at(f, ""));
        let shown = match (tens.is_empty(), digit_of(&tens), digit_of(&ones)) {
            (true, _, Some(o)) => format!("{o}"),
            (false, Some(t), Some(o)) => format!("{t}{o}"),
            _ => format!("? tens [{tens}] ones [{ones}]"),
        };
        let diff = match want {
            Some(w) => {
                let v = w[l as usize];
                let target = |pre: &str, d: Option<u64>| -> String { d.map(|d| SEGMENTS[d as usize].to_string()).unwrap_or_default().chars().map(|ch| format!("{pre}{ch}")).collect::<Vec<_>>().join(",") };
                let (wt, wo) = (if v >= 10 { Some(v / 10) } else { None }, Some(v % 10));
                let want_on: BTreeSet<String> = target("t", wt).split(',').chain(target("", wo).split(',')).filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
                let have_on: BTreeSet<String> = tens.chars().map(|ch| format!("t{ch}")).chain(ones.chars().map(|ch| ch.to_string())).collect();
                let extra: Vec<&String> = have_on.difference(&want_on).collect();
                let missing: Vec<&String> = want_on.difference(&have_on).collect();
                if extra.is_empty() && missing.is_empty() { format!("  ok ({v})") } else { format!("  want {v}: lit too {extra:?}, dark {missing:?}") }
            }
            None => String::new(),
        };
        // debugging probes: outputs named probe* print their level
        let probes: String = c
            .outputs
            .iter()
            .enumerate()
            .filter(|(_, o)| o.name.starts_with("probe"))
            .map(|(k, o)| format!(" {}={}", o.name, tr.levels.get(k).and_then(|v| v.get(f)).copied().unwrap_or(0)))
            .collect();
        out.push_str(&format!("L{l:<2} {shown}{diff}{probes}\n"));
    }
    Ok(out)
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
    /// A 7-segment display of the digits 0..9 only (`seven-segment 4 hex decimal`,
    /// designs/briefs/display-7seg.md): the input never goes above 9, and 10..15 are don't-cares.
    pub decimal: bool,
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
            Op::StorageCounter(_) => "storage-counter",
        }
    }
}

impl Spec {
    /// `adder 4`, `adder 4 hex`, `encoder 3 priority`, `storage-counter stacks`, ...: the spec, or
    /// None when the first word is no compute kind.
    pub fn parse(words: &[&str]) -> Option<Result<Spec>> {
        if *words.first()? == "storage-counter" {
            let line = words.join(" ");
            return Some((|| {
                let t = words.get(1).and_then(|w| Table::parse(w)).ok_or_else(|| anyhow::anyhow!("circuit: {line}: what it shows: storage-counter stacks | percent"))?;
                if words.len() > 2 {
                    bail!("circuit: {line}: storage-counter takes one word (stacks or percent)");
                }
                Ok(Spec { op: Op::StorageCounter(t), n: 4, hex: true, decimal: false })
            })());
        }
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
            let decimal = words.contains(&"decimal");
            for w in &words[2..] {
                if !matches!(*w, "hex" | "priority" | "decimal") || (*w == "priority" && op != Op::PriorityEncoder) {
                    bail!("circuit: {line}: unknown modifier {w:?}");
                }
            }
            if decimal && !(op == Op::SevenSegment && hex) {
                bail!("circuit: {line}: decimal is for a 7-segment display of one strength (seven-segment 4 hex decimal)");
            }
            if hex && !matches!(op, Op::Adder | Op::Subtractor | Op::Incrementer | Op::Comparator | Op::SevenSegment) {
                bail!("circuit: {line}: hex (values as signal strengths) is for adders, subtractors, incrementers, comparators and 7-segment displays");
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
            Ok(Spec { op, n, hex, decimal })
        })())
    }

    /// The kind as written on the `circuit:` line.
    pub fn kind_name(&self) -> String {
        let base = match self.op {
            Op::PriorityEncoder => format!("encoder {} priority", self.n),
            Op::StorageCounter(t) => return format!("storage-counter {}", t.name()),
            op => format!("{} {}", op.name(), self.n),
        };
        let base = if self.hex { format!("{base} hex") } else { base };
        if self.decimal { format!("{base} decimal") } else { base }
    }

    fn with_n(&self, n: u32) -> Spec {
        Spec { n, ..*self }
    }

    /// Whether an output carries a value as a level (a hex kind's `s` / `d`).
    fn level_output(&self, name: &str) -> bool {
        self.hex && !matches!(self.op, Op::StorageCounter(_) | Op::SevenSegment) && matches!(name, "s" | "d")
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
        Op::SevenSegment => (val("a"), Vec::new(), s(&["a", "b", "c", "d", "e", "f", "g", "af", "ab", "efg", "bcg", "de", "cd"]), Vec::new()),
        Op::StorageCounter(_) => (
            s(&["chest"]),
            Vec::new(),
            s(&["a", "b", "c", "d", "e", "f", "g", "af", "ab", "efg", "bcg", "de", "cd", "ta", "tb", "tc", "td", "te", "tf", "tg", "taf", "tab", "tefg", "tbcg", "tde", "tcd"]),
            Vec::new(),
        ),
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
        let counter = matches!(spec.op, Op::StorageCounter(_));
        let value = if counter { i.name == "chest" } else { spec.hex && matches!(i.name.as_str(), "a" | "b") };
        if strength != value {
            bail!("{kind}: input {}: {}", i.name, if counter { "the storage counter's chest is a strength input (port input.chest: x y z strength, a chest of the design)" } else if value { "a hex value is a strength input (port input.<name>: x y z strength, on a chest of the design)" } else { "strength inputs are a hex kind's values (a, b)" });
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
    if matches!(spec.op, Op::StorageCounter(_)) {
        counter_faces(c)?;
    }
    Ok(())
}

/// The standard 3-wide x 5-tall figure-8 of a 7-segment digit: each segment's (column, row from
/// the top) as seen from the front.
pub const FACE: [(&str, i32, i32); 7] = [("a", 1, 0), ("f", 0, 1), ("b", 2, 1), ("g", 1, 2), ("e", 0, 3), ("c", 2, 3), ("d", 1, 4)];

/// The face's corner lamps (vjeux 2026-10-21 21:36, "this doesn't look like a 7 display segment":
/// single-lamp segments with dark corners read as a dot grid): each corner belongs to the segments
/// it touches and is lit iff any of them is; named by those segments. The two interior cells
/// (1, 1) and (1, 3) stay dark.
pub const CORNERS: [(&str, i32, i32); 6] = [("af", 0, 0), ("ab", 2, 0), ("efg", 0, 2), ("bcg", 2, 2), ("de", 0, 4), ("cd", 2, 4)];

/// The two interior cells of the face, never lit.
pub const INTERIOR: [(i32, i32); 2] = [(1, 1), (1, 3)];

/// Study 15, a display a player can read (vjeux 2026-10-21: "the output is in the middle of the
/// redstone contraption, how do you feed it to the display?"): the seven outputs are LAMPS (an air
/// output cell takes the judge's lamp, or the design's own redstone lamp) forming [`FACE`] on ONE
/// vertical exterior face, read from outside (top row up, `f` on the viewer's left); no design
/// block and no judge input device in front of the face (the cells of the face's plane between the
/// lamps may hold design blocks). Returns the face's outward normal.
pub fn display_face(c: &Circuit) -> Result<Pos> {
    digit_face(c, "", "seven-segment").map(|f| f.0)
}

/// The storage counter's two digits ([`Op::StorageCounter`]): each a [`FACE`] like
/// [`display_face`]'s (the ones digit's outputs a..g, the tens digit's ta..tg), on one face, the
/// tens digit 4 columns to the viewer's left of the ones digit (one column between them), on the
/// same rows. Returns the face's outward normal.
pub fn counter_faces(c: &Circuit) -> Result<Pos> {
    let (n, right, o) = digit_face(c, "", "storage-counter (the ones digit, a..g)")?;
    let (n2, right2, o2) = digit_face(c, "t", "storage-counter (the tens digit, ta..tg)")?;
    let want = [o[0] - 4 * right[0], o[1], o[2] - 4 * right[2]];
    if n2 != n || right2 != right || o2 != want {
        bail!("storage-counter: the tens digit (ta..tg) must sit on the ones digit's face, its top-left cell at {want:?}: 4 columns to the viewer's left of the ones digit's, on the same rows (it is at {o2:?})");
    }
    Ok(n)
}

/// One digit's face: the outputs `<pre>a..<pre>g` as [`FACE`] on one vertical exterior face, read
/// from outside (see [`display_face`]). Returns the outward normal, the viewer's right and the
/// top-left cell.
fn digit_face(c: &Circuit, pre: &str, label: &str) -> Result<(Pos, Pos, Pos)> {
    let pos = |n: &str| c.outputs.iter().find(|o| o.name == format!("{pre}{n}")).map(|o| o.pos);
    for (n, _, _) in FACE.iter().chain(CORNERS.iter()) {
        let p = pos(n).ok_or_else(|| anyhow::anyhow!("{label}: output {pre}{n} is missing (the 3x5 face's 13 lamps: segments a..g and the corners af ab efg bcg de cd, each corner lit iff a segment it touches is)"))?;
        match c.clock.design.blocks.get(&p) {
            None => {}
            Some(s) if s.starts_with("minecraft:redstone_lamp") => {}
            Some(s) => bail!("{label}: output {pre}{n} at {p:?} holds {}: the face's cells are lamps (leave the cell air for the judge's lamp, or put a redstone_lamp there)", clock::short(s)),
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
        if FACE.iter().chain(CORNERS.iter()).any(|(s, col, row)| pos(s) != Some(at(*col, *row))) {
            continue;
        }
        // the interior cells stay dark: no lamp there
        if let Some((col, row)) = INTERIOR.iter().find(|(col, row)| c.clock.design.blocks.get(&at(*col, *row)).is_some_and(|s| s.starts_with("minecraft:redstone_lamp"))) {
            why = format!("the face's interior cell ({col},{row}) holds a lamp: it stays dark");
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
        return Ok((n, right, origin));
    }
    if why.is_empty() {
        why = format!("the outputs {pre}a..{pre}g and the corners do not form the 3x5 face on one vertical face, read from outside: (column, row from the top) a (1,0), f (0,1), b (2,1), g (1,2), e (0,3), c (2,3), d (1,4), corners af (0,0), ab (2,0), efg (0,2), bcg (2,2), de (0,4), cd (2,4), f on the viewer's left");
    }
    bail!("{label}: {why}")
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
            Op::SevenSegment => flag(name.chars().all(|ch| "abcdefg".contains(ch)) && name.chars().any(|ch| SEGMENTS[(a % 16) as usize].contains(ch))),
            Op::StorageCounter(_) => {
                // the reference: the empty chest's 0 (every other state is judged against the
                // design's own table, in [`check`])
                let _ = value(c, &lay, spec, x, "chest", n);
                flag(name.len() == 1 && SEGMENTS[0].contains(name))
            }
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
    if spec.op == Op::SevenSegment && spec.hex {
        // one signal strength (designs/briefs/display-7seg.md): a container can jump between any
        // two levels, so every ordered pair of values, as an Euler tour of the complete digraph on
        // the 16 values (Hierholzer: from each value, its unused successors in order)
        let n = if spec.decimal { 10 } else { (full + 1) as usize };
        let mut next = vec![0usize; n];
        let mut stack = vec![0usize];
        let mut tour: Vec<usize> = Vec::new();
        while let Some(&u) = stack.last() {
            // the next unused successor of u (skipping u itself)
            while next[u] < n && next[u] == u {
                next[u] += 1;
            }
            if next[u] < n {
                let w = next[u];
                next[u] += 1;
                stack.push(w);
            } else {
                tour.push(u);
                stack.pop();
            }
        }
        tour.reverse();
        v.extend(tour.into_iter().skip(1).map(|x| ("pairs", x as State)));
        return v;
    }
    if matches!(spec.op, Op::StorageCounter(_)) && !short {
        // a chest filled and emptied (its comparator level one step at a time, both ways), then
        // the walks below (every single-bit change of the level, every jump to the complement)
        v.extend((1..=full).map(|x| ("fill", x)));
        v.extend((0..full).rev().map(|x| ("empty", x)));
    }
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
    if matches!(spec.op, Op::StorageCounter(_)) && !short {
        // bursts of fast changes (a player shift-clicking stacks in and out, hoppers at full
        // rate): 4 levels 2..9 gt apart ("rush", not judged), then the level holds ("rushed",
        // judged like any state); steps of 1..3 levels either way
        let mut r: u32 = 0x2545_f491;
        let mut cur = v.last().map(|p| p.1 as i32).unwrap_or(0);
        for _ in 0..12 {
            for k in 0..5 {
                r = r.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let d = [-3, -2, -1, 1, 2, 3][((r >> 16) % 6) as usize];
                let mut nxt = (cur + d).clamp(0, full as i32);
                if nxt == cur {
                    nxt = (cur - d).clamp(0, full as i32);
                }
                cur = nxt;
                v.push((if k < 4 { "rush" } else { "rushed" }, cur as State));
            }
        }
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
    let mut k_phase: u32 = 0;
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
        // a storage counter's rush: 2..9 gt to the next change
        k_phase = k_phase.wrapping_mul(2_654_435_761).wrapping_add(97);
        let dur = if section == "rush" { 2 + (k_phase >> 16) % 8 } else { gap };
        s.phase(t, t + dur, format!("{section} {x}"));
        t += dur;
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
    let mut on_blips = 0usize;
    let mut sections: Vec<(String, usize, usize)> = Vec::new();
    // a storage counter shows its own table: the number on its lamps at each judged state must be
    // allowed at the chest's level ([`Table::allowed`]) and the same every time that level comes
    // back; `shown[L]` is the lamps' pattern there
    let counter = match spec.op {
        Op::StorageCounter(t) => Some(t),
        _ => None,
    };
    let mut shown: Vec<Option<Vec<u8>>> = vec![None; 16];
    shown[0] = Some(e0.clone());
    let lay = layout(c, &spec);
    let level_of = |x: State| value(c, &lay, &spec, x, "chest", spec.n) as usize;
    let mut prev_rushed = false;
    for (a, b, label) in &s.phases {
        let (a, b) = (*a as usize, (*b as usize).min(frames));
        let mut w = label.split_whitespace();
        let section = w.next().unwrap_or("").to_string();
        let x: State = w.next().and_then(|v| v.parse().ok()).unwrap_or(0);
        if a >= b {
            break;
        }
        if counter.is_some() && section == "rush" {
            prev = x;
            prev_rushed = true;
            continue;
        }
        let (e, ep) = match counter {
            None => (ideal(c, &spec, x), ideal(c, &spec, prev)),
            Some(tb) => {
                let end = vals(b - 1);
                let lit = |pre: &str| -> String { "abcdefg".chars().filter(|ch| c.outputs.iter().position(|o| o.name == format!("{pre}{ch}")).is_some_and(|k| end[k] != 0)).collect() };
                let (tens, ones) = (lit("t"), lit(""));
                let l = level_of(x);
                let number = match (tens.is_empty(), digit_of(&tens), digit_of(&ones)) {
                    (true, _, Some(o)) => Some(o as u64),
                    (false, Some(t), Some(o)) => Some(t as u64 * 10 + o as u64),
                    _ => None,
                };
                let Some(n) = number else {
                    table.push(format!("  {section}: chest level {l}: tens [{tens}] ones [{ones}]"));
                    return Outcome::fail(format!("shows no number at chest level {l} ({}): the tens digit's lit segments [{tens}], the ones digit's [{ones}]", transition(c, &spec, prev, x)), table);
                };
                let ok = tb.allowed(l as u64);
                if !ok.contains(&n) {
                    return Outcome::fail(format!("shows {n} at chest level {l} ({}): a fill read as level {l} shows {}..{} {}", transition(c, &spec, prev, x), ok.start(), ok.end(), tb.name()), table);
                }
                // the corners: each lit iff a segment it touches is (the 3x5 bitmap of the digits)
                let want: Vec<u8> = c
                    .outputs
                    .iter()
                    .map(|o| {
                        let (digit, name) = match o.name.strip_prefix('t') {
                            Some(rest) => ((n >= 10).then_some(n / 10), rest),
                            None => (Some(n % 10), o.name.as_str()),
                        };
                        u8::from(digit.is_some_and(|d| name.chars().any(|ch| SEGMENTS[d as usize].contains(ch))))
                    })
                    .collect();
                if want != end {
                    return Outcome::fail(format!("shows {n} at chest level {l} ({}), but its corners do not match the digits' segments: {}", transition(c, &spec, prev, x), names(&end, &want)), table);
                }
                if let Some(p) = &shown[l]
                    && *p != end
                {
                    return Outcome::fail(format!("shows {n} at chest level {l} ({}), another number earlier at that level", transition(c, &spec, prev, x)), table);
                }
                shown[l] = Some(end.clone());
                let ep = shown[level_of(prev)].clone().unwrap_or_else(|| end.clone());
                (end, ep)
            }
        };
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
        let mut blip = false;
        let mut blipped: Vec<&str> = Vec::new();
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
            // a visible on-blip (display-7seg.md): a lamp dark before and after that lights on the way
            if e[k] == 0 && ep[k] == 0 && (a..f0.min(b)).any(|f| at(f) != 0) {
                blip = true;
                blipped.push(c.outputs[k].name.as_str());
            }
        }
        if blip && !prev_rushed {
            table.push(format!("  on-blip: {}: {}", transition(c, &spec, prev, x), blipped.join(" ")));
        }
        // (a rush's end: its outputs ran through the burst, no glitch count)
        glitches += (glitch && !prev_rushed) as usize;
        on_blips += (blip && !prev_rushed) as usize;
        prev_rushed = false;
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
    // a 7-segment display shows its on-blips even at 0: a face flashing wrong lamps between values
    // reads as a broken counter (display-7seg.md, the number)
    if matches!(spec.op, Op::SevenSegment) {
        metrics.push(("on-blips".to_string(), on_blips.to_string()));
    }
    let arithmetic = matches!(spec.op, Op::Adder | Op::Subtractor | Op::Incrementer | Op::Comparator);
    if arithmetic && spec.n > 1 {
        metrics.push(("per bit".to_string(), format!("{:.1} blocks", c.blocks() as f64 / spec.n as f64)));
    }
    // an adder's carry ports (the adder study's axis: a half adder has a carry out only, a full
    // adder a carry in and out)
    if spec.op == Op::Adder {
        let cin = c.inputs.iter().any(|i| i.name == "cin");
        let cout = c.outputs.iter().any(|o| o.name == "cout");
        let carry = match (cin, cout) {
            (true, true) => Some("in and out"),
            (false, true) => Some("out"),
            (true, false) => Some("in"),
            (false, false) => None,
        };
        if let Some(v) = carry {
            metrics.push(("carry".to_string(), v.to_string()));
        }
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
