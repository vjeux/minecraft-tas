//! The functions of study 12's arithmetic kinds (designs/circuits/compute/VARIATIONS.md): port
//! names and the ideal truth table. An input is a bit line or (hex kinds) a value 0..2^N on one
//! line; a combination `c` packs them: input i occupies `widths[i]` bits from `offsets[i]` (in the
//! order of [`Func::inputs`]). An output is an on/off line or (hex `s` / `d`) an exact level.

use anyhow::{Result, bail};

/// A combinational function: named inputs and outputs, and the outputs of every combination.
#[derive(Clone, Debug)]
pub struct Func {
    /// The kind as written on the `circuit:` line (`adder 1`, `comparator 4`, `adder 4 hex`...).
    pub kind: String,
    pub inputs: Vec<String>,
    /// Bits per input: 1 for a line, N for a hex value (a strength input on a chest).
    pub widths: Vec<u32>,
    pub outputs: Vec<String>,
    /// Per output: an exact level (a hex value) rather than on/off.
    pub levels: Vec<bool>,
    /// [combination][output]: 0/1, or the level.
    pub table: Vec<Vec<u8>>,
}

fn bits(name: &str, n: u32) -> Vec<String> {
    (0..n).map(|k| format!("{name}{k}")).collect()
}

impl Func {
    /// Bits in a combination.
    pub fn n(&self) -> usize {
        self.widths.iter().sum::<u32>() as usize
    }

    pub fn offset(&self, i: usize) -> u32 {
        self.widths[..i].iter().sum()
    }

    /// Input i's value in combination c.
    pub fn value(&self, c: usize, i: usize) -> u32 {
        ((c >> self.offset(i)) as u32) & ((1u32 << self.widths[i]) - 1)
    }

    pub fn is_hex(&self, i: usize) -> bool {
        self.widths[i] > 1
    }

    /// Whether output o is on in combination c.
    pub fn on(&self, c: usize, o: usize) -> bool {
        self.table[c][o] > 0
    }

    /// The kind's function for a design with these port names (optional ports: cin / cout for
    /// adders and incrementers, bin / bout for subtractors, any of eq lt gt le ge ne for
    /// comparators). Also `xor`, `and`, `or`, `maj`, `parity` for tests (inputs a, b, c...).
    pub fn of(kind: &str, ins: &[String], outs: &[String]) -> Result<Func> {
        let w: Vec<&str> = kind.split_whitespace().collect();
        let n: u32 = match w.get(1) {
            Some(v) => v.parse()?,
            None => 0,
        };
        let hex = w.contains(&"hex");
        let has_in = |s: &str| ins.iter().any(|i| i == s);
        let has_out = |s: &str| outs.iter().any(|i| i == s);
        // (name, width) of the value inputs a / b
        let val = |name: &str| -> Vec<(String, u32)> { if hex { vec![(name.to_string(), n)] } else { bits(name, n).into_iter().map(|b| (b, 1)).collect() } };
        let vout = |name: &str| -> Vec<(String, bool)> { if hex { vec![(name.to_string(), true)] } else { bits(name, n).into_iter().map(|b| (b, false)).collect() } };
        let (inputs, outputs): (Vec<(String, u32)>, Vec<(String, bool)>) = match w.first().copied().unwrap_or("") {
            "adder" | "incrementer" => {
                let mut i = val("a");
                if w[0] == "adder" {
                    i.extend(val("b"));
                }
                if has_in("cin") {
                    i.push(("cin".into(), 1));
                }
                let mut o = vout("s");
                if has_out("cout") {
                    o.push(("cout".into(), false));
                }
                (i, o)
            }
            "subtractor" => {
                let mut i = val("a");
                i.extend(val("b"));
                if has_in("bin") {
                    i.push(("bin".into(), 1));
                }
                let mut o = vout("d");
                if has_out("bout") {
                    o.push(("bout".into(), false));
                }
                (i, o)
            }
            "comparator" => {
                let mut i = val("a");
                i.extend(val("b"));
                let o: Vec<(String, bool)> = ["eq", "lt", "gt", "le", "ge", "ne"].iter().filter(|s| has_out(s)).map(|s| (s.to_string(), false)).collect();
                if o.is_empty() {
                    bail!("comparator: needs at least one of the outputs eq lt gt le ge ne");
                }
                (i, o)
            }
            "xor" | "and" | "or" | "maj" | "parity" => (ins.iter().map(|s| (s.clone(), 1)).collect(), vec![(outs.first().cloned().unwrap_or_else(|| "out".into()), false)]),
            k => bail!("unknown kind {k:?} (adder N [hex], subtractor N [hex], incrementer N [hex], comparator N [hex]; tests: xor and or maj parity)"),
        };
        let names: Vec<String> = inputs.iter().map(|(s, _)| s.clone()).collect();
        let onames: Vec<String> = outputs.iter().map(|(s, _)| s.clone()).collect();
        for p in ins {
            if !names.contains(p) {
                bail!("{kind}: unexpected input port {p:?} (wants {})", names.join(" "));
            }
        }
        for p in &names {
            if !ins.contains(p) {
                bail!("{kind}: missing input port {p:?}");
            }
        }
        for p in outs {
            if !onames.contains(p) {
                bail!("{kind}: unexpected output port {p:?} (wants {})", onames.join(" "));
            }
        }
        let mut f = Func {
            kind: kind.to_string(),
            inputs: names,
            widths: inputs.iter().map(|(_, w)| *w).collect(),
            outputs: onames,
            levels: outputs.iter().map(|(_, l)| *l).collect(),
            table: Vec::new(),
        };
        let total = f.n();
        for c in 0..(1usize << total) {
            let row = f.row(&w, n, c);
            f.table.push(row);
        }
        Ok(f)
    }

    /// The ideal outputs of combination c.
    fn row(&self, w: &[&str], n: u32, c: usize) -> Vec<u8> {
        let idx = |name: &str| self.inputs.iter().position(|x| x == name);
        // a value: a hex input, or the bit lines name0..
        let value = |name: &str| -> i64 {
            if let Some(i) = idx(name) {
                return self.value(c, i) as i64;
            }
            (0..n).filter_map(|k| idx(&format!("{name}{k}"))).enumerate().map(|(k, i)| (self.value(c, i) as i64) << k).sum()
        };
        let bit = |name: &str| idx(name).map(|i| self.value(c, i) as i64);
        let (a, b) = (value("a"), value("b"));
        let m = 1i64 << n;
        let level_or_bit = |o: &str, v: i64, pre: &str| -> u8 {
            if o == pre {
                v.rem_euclid(m) as u8
            } else {
                (v.rem_euclid(m) >> o[pre.len()..].parse::<u32>().unwrap_or(0) & 1) as u8
            }
        };
        match w[0] {
            "adder" | "incrementer" => {
                let cin = bit("cin").unwrap_or(if w[0] == "incrementer" { 1 } else { 0 });
                let sum = a + if w[0] == "adder" { b } else { 0 } + cin;
                self.outputs.iter().map(|o| if o == "cout" { (sum >> n & 1) as u8 } else { level_or_bit(o, sum, "s") }).collect()
            }
            "subtractor" => {
                let diff = a - b - bit("bin").unwrap_or(0);
                self.outputs.iter().map(|o| if o == "bout" { (diff < 0) as u8 } else { level_or_bit(o, diff, "d") }).collect()
            }
            "comparator" => self
                .outputs
                .iter()
                .map(|o| {
                    (match o.as_str() {
                        "eq" => a == b,
                        "lt" => a < b,
                        "gt" => a > b,
                        "le" => a <= b,
                        "ge" => a >= b,
                        _ => a != b,
                    }) as u8
                })
                .collect(),
            f => {
                let ones = c.count_ones() as usize;
                let k = self.inputs.len();
                vec![(match f {
                    "xor" | "parity" => ones % 2 == 1,
                    "and" => ones == k,
                    "or" => ones > 0,
                    _ => 2 * ones > k,
                }) as u8]
            }
        }
    }

    /// A combination as `a=01 b=10 cin=1` (bits of each group from the highest; hex values in
    /// decimal).
    pub fn label(&self, c: usize) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut done = vec![false; self.inputs.len()];
        for (i, name) in self.inputs.iter().enumerate() {
            if done[i] {
                continue;
            }
            if self.is_hex(i) {
                done[i] = true;
                parts.push(format!("{name}={}", self.value(c, i)));
                continue;
            }
            let pre: String = name.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string();
            if pre.len() < name.len() {
                let mut members: Vec<(u32, usize)> =
                    self.inputs.iter().enumerate().filter(|(_, x)| x.trim_end_matches(|ch: char| ch.is_ascii_digit()) == pre && x.len() > pre.len()).map(|(j, x)| (x[pre.len()..].parse().unwrap_or(0), j)).collect();
                members.sort();
                let s: String = members.iter().rev().map(|(_, j)| if self.value(c, *j) == 1 { '1' } else { '0' }).collect();
                for (_, j) in &members {
                    done[*j] = true;
                }
                parts.push(format!("{pre}={s}"));
            } else {
                done[i] = true;
                parts.push(format!("{name}={}", self.value(c, i)));
            }
        }
        parts.join(" ")
    }
}

/// The judge's walk (VARIATIONS.md section 4): every state in Gray order from 0, an Euler tour of
/// the n-cube (every single-bit flip both ways, n <= 10), then every state reached from its
/// complement. Consecutive entries are the tested transitions.
pub fn walk(n: usize) -> Vec<usize> {
    let m = 1usize << n;
    let mut w: Vec<usize> = (0..m).map(|i| i ^ (i >> 1)).collect();
    if n <= 10 {
        // Hierholzer on the n-cube's directed edges, from the last Gray state
        let start = *w.last().unwrap_or(&0);
        let mut next: Vec<usize> = vec![0; m];
        let mut stack = vec![start];
        let mut tour = Vec::new();
        while let Some(&v) = stack.last() {
            if next[v] < n {
                let b = next[v];
                next[v] += 1;
                stack.push(v ^ (1 << b));
            } else {
                tour.push(v);
                stack.pop();
            }
        }
        tour.reverse();
        w.extend(tour.into_iter().skip(1));
    }
    for s in 0..m {
        let c = !s & (m - 1);
        if w.last() != Some(&c) {
            w.push(c);
        }
        w.push(s);
    }
    w
}
