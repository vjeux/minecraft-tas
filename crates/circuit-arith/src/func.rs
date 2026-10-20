//! The functions of study 12's arithmetic kinds (designs/circuits/compute/VARIATIONS.md): port
//! names and the ideal truth table. Input i of a combination `c` is bit i of `c` (inputs in the
//! order of [`Func::inputs`]).

use anyhow::{Result, bail};

/// A combinational function: named inputs and outputs, and the outputs of every combination.
#[derive(Clone, Debug)]
pub struct Func {
    /// The kind as written on the `circuit:` line (`adder 1`, `comparator 4`, ...).
    pub kind: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    /// [combination][output].
    pub table: Vec<Vec<bool>>,
}

fn bits(name: &str, n: u32) -> Vec<String> {
    (0..n).map(|k| format!("{name}{k}")).collect()
}

fn value(c: usize, idx: &[usize]) -> i64 {
    idx.iter().enumerate().map(|(k, &i)| (((c >> i) & 1) as i64) << k).sum()
}

impl Func {
    pub fn n(&self) -> usize {
        self.inputs.len()
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
        let has_in = |s: &str| ins.iter().any(|i| i == s);
        let has_out = |s: &str| outs.iter().any(|i| i == s);
        let (inputs, outputs): (Vec<String>, Vec<String>) = match w.first().copied().unwrap_or("") {
            "adder" | "incrementer" => {
                let mut i = bits("a", n);
                if w[0] == "adder" {
                    i.extend(bits("b", n));
                }
                if has_in("cin") {
                    i.push("cin".into());
                }
                let mut o = bits("s", n);
                if has_out("cout") {
                    o.push("cout".into());
                }
                (i, o)
            }
            "subtractor" => {
                let mut i = bits("a", n);
                i.extend(bits("b", n));
                if has_in("bin") {
                    i.push("bin".into());
                }
                let mut o = bits("d", n);
                if has_out("bout") {
                    o.push("bout".into());
                }
                (i, o)
            }
            "comparator" => {
                let mut i = bits("a", n);
                i.extend(bits("b", n));
                let o: Vec<String> = ["eq", "lt", "gt", "le", "ge", "ne"].iter().filter(|s| has_out(s)).map(|s| s.to_string()).collect();
                if o.is_empty() {
                    bail!("comparator: needs at least one of the outputs eq lt gt le ge ne");
                }
                (i, o)
            }
            "xor" | "and" | "or" | "maj" | "parity" => (ins.to_vec(), vec![outs.first().cloned().unwrap_or_else(|| "out".into())]),
            k => bail!("unknown kind {k:?} (adder N, subtractor N, incrementer N, comparator N; tests: xor and or maj parity)"),
        };
        for p in ins {
            if !inputs.contains(p) {
                bail!("{kind}: unexpected input port {p:?} (wants {})", inputs.join(" "));
            }
        }
        for p in &inputs {
            if !ins.contains(p) {
                bail!("{kind}: missing input port {p:?}");
            }
        }
        for p in outs {
            if !outputs.contains(p) {
                bail!("{kind}: unexpected output port {p:?} (wants {})", outputs.join(" "));
            }
        }
        let idx = |name: &str| inputs.iter().position(|x| x == name);
        let group = |pre: &str| -> Vec<usize> { (0..n).filter_map(|k| idx(&format!("{pre}{k}"))).collect() };
        let (ai, bi) = (group("a"), group("b"));
        let m = inputs.len();
        let mut table = Vec::with_capacity(1 << m);
        for c in 0..(1usize << m) {
            let bit = |name: &str| idx(name).map(|i| ((c >> i) & 1) as i64);
            let (a, b) = (value(c, &ai), value(c, &bi));
            let row: Vec<bool> = match w[0] {
                "adder" | "incrementer" => {
                    let cin = bit("cin").unwrap_or(if w[0] == "incrementer" { 1 } else { 0 });
                    let sum = a + if w[0] == "adder" { b } else { 0 } + cin;
                    outputs.iter().map(|o| if o == "cout" { sum >> n & 1 == 1 } else { sum >> o[1..].parse::<u32>().unwrap_or(0) & 1 == 1 }).collect()
                }
                "subtractor" => {
                    let diff = a - b - bit("bin").unwrap_or(0);
                    let d = diff.rem_euclid(1 << n);
                    outputs.iter().map(|o| if o == "bout" { diff < 0 } else { d >> o[1..].parse::<u32>().unwrap_or(0) & 1 == 1 }).collect()
                }
                "comparator" => outputs
                    .iter()
                    .map(|o| match o.as_str() {
                        "eq" => a == b,
                        "lt" => a < b,
                        "gt" => a > b,
                        "le" => a <= b,
                        "ge" => a >= b,
                        _ => a != b,
                    })
                    .collect(),
                f => {
                    let ones = c.count_ones();
                    vec![match f {
                        "xor" | "parity" => ones % 2 == 1,
                        "and" => ones as usize == m,
                        "or" => ones > 0,
                        _ => 2 * ones as usize > m,
                    }]
                }
            };
            table.push(row);
        }
        Ok(Func { kind: kind.to_string(), inputs, outputs, table })
    }

    /// A combination as `a=01 b=10 cin=1` (bits of each group from the highest).
    pub fn label(&self, c: usize) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut done = vec![false; self.inputs.len()];
        for (i, name) in self.inputs.iter().enumerate() {
            if done[i] {
                continue;
            }
            let pre: String = name.trim_end_matches(|ch: char| ch.is_ascii_digit()).to_string();
            if pre.len() < name.len() {
                let mut members: Vec<(u32, usize)> =
                    self.inputs.iter().enumerate().filter(|(_, x)| x.trim_end_matches(|ch: char| ch.is_ascii_digit()) == pre && x.len() > pre.len()).map(|(j, x)| (x[pre.len()..].parse().unwrap_or(0), j)).collect();
                members.sort();
                let s: String = members.iter().rev().map(|(_, j)| if (c >> j) & 1 == 1 { '1' } else { '0' }).collect();
                for (_, j) in &members {
                    done[*j] = true;
                }
                parts.push(format!("{pre}={s}"));
            } else {
                done[i] = true;
                parts.push(format!("{name}={}", (c >> i) & 1));
            }
        }
        parts.join(" ")
    }
}

/// The judge's walk (VARIATIONS.md section 4): every state in Gray order from 0, an Euler tour of
/// the n-cube (every single-input flip both ways, n <= 10), then every state reached from its
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
