//! Re-orienting machines: the 24 rotations of the cube applied to a design (positions, the facing
//! of observers and pistons, the kick cell, the travel vector). `mctas circuit fly orient` runs
//! every orientation of each design on redsim: which ones fly, which way, how fast (a horizontal
//! machine tilted to fly up or down, a machine rolled about its travel axis).

use crate::machine::Machine;
use crate::sim::{self, SimOptions};
use clock::Pos;

/// A rotation of the cube: a signed permutation matrix with determinant 1 (rows: new x, y, z).
pub type Rot = [[i32; 3]; 3];

pub fn apply(r: &Rot, p: Pos) -> Pos {
    [0, 1, 2].map(|i| r[i][0] * p[0] + r[i][1] * p[1] + r[i][2] * p[2])
}

fn det(r: &Rot) -> i32 {
    r[0][0] * (r[1][1] * r[2][2] - r[1][2] * r[2][1]) - r[0][1] * (r[1][0] * r[2][2] - r[1][2] * r[2][0]) + r[0][2] * (r[1][0] * r[2][1] - r[1][1] * r[2][0])
}

/// The 24 proper rotations of the cube (identity first).
pub fn rotations() -> Vec<Rot> {
    let mut out = Vec::new();
    let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    for p in perms {
        for signs in 0..8 {
            let mut r = [[0; 3]; 3];
            for i in 0..3 {
                r[i][p[i]] = if signs >> i & 1 == 1 { -1 } else { 1 };
            }
            if det(&r) == 1 {
                out.push(r);
            }
        }
    }
    out.sort_by_key(|r| *r != [[1, 0, 0], [0, 1, 0], [0, 0, 1]]);
    out
}

const FACINGS: [(&str, Pos); 6] = [("east", [1, 0, 0]), ("west", [-1, 0, 0]), ("up", [0, 1, 0]), ("down", [0, -1, 0]), ("south", [0, 0, 1]), ("north", [0, 0, -1])];

fn turn_state(r: &Rot, s: &str) -> Option<String> {
    let id = s.split('[').next().unwrap_or(s);
    let short = id.trim_start_matches("minecraft:");
    match short {
        "observer" | "piston" | "sticky_piston" | "piston_head" => {
            let inner = s.split_once('[').map(|x| x.1.trim_end_matches(']')).unwrap_or("");
            let mut props: Vec<String> = Vec::new();
            for kv in inner.split(',').filter(|k| !k.is_empty()) {
                let (k, v) = kv.split_once('=')?;
                if k == "facing" {
                    let f = FACINGS.iter().find(|(n, _)| *n == v)?.1;
                    let g = apply(r, f);
                    let name = FACINGS.iter().find(|(_, d)| *d == g)?.0;
                    props.push(format!("facing={name}"));
                } else {
                    props.push(kv.to_string());
                }
            }
            Some(format!("{id}[{}]", props.join(",")))
        }
        _ if s.contains("facing=") || s.contains("axis=") => None,
        _ => Some(s.to_string()),
    }
}

/// The machine under rotation `r` (around the origin), or None when it holds a block this cannot
/// turn (other directional blocks).
pub fn rotated(m: &Machine, r: &Rot) -> Option<Machine> {
    let mut d = m.design().clone();
    let mut blocks = std::collections::BTreeMap::new();
    for (p, s) in &m.design().blocks {
        blocks.insert(apply(r, *p), turn_state(r, s)?);
    }
    d.blocks = blocks;
    d.ports = d.ports.iter().map(|(k, p)| (k.clone(), apply(r, *p))).collect();
    if let Some(t) = &m.travel {
        let nt = apply(r, *t);
        d.meta.insert("travel".into(), format!("{} {} {}", nt[0], nt[1], nt[2]));
    }
    if let Some(args) = d.meta.get("port start").cloned() {
        // a temporary kick block's state turns too
        let mut w: Vec<String> = args.split_whitespace().map(String::from).collect();
        if w.len() >= 2 && w[0].contains(':') {
            w[0] = turn_state(r, &w[0])?;
        }
        d.meta.insert("port start".into(), w.join(" "));
    }
    let mut mm = Machine::parse(&d.to_text()).ok()?;
    mm.stops = m.stops.iter().map(|(p, s)| (apply(r, *p), turn_state(r, s).unwrap_or(s.clone()))).collect();
    Some(mm)
}

/// One orientation's outcome on redsim.
pub struct Oriented {
    pub index: usize,
    pub machine: Machine,
    pub report: crate::report::FlyReport,
}

/// Every orientation of `m` on redsim.
pub fn orient(m: &Machine) -> Vec<Oriented> {
    let o = SimOptions { distance: 16, ..Default::default() };
    rotations()
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let mm = rotated(m, r)?;
            let report = sim::simulate(&mm, &o);
            Some(Oriented { index: i, machine: mm, report })
        })
        .collect()
}
