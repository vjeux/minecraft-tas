//! `mctas circuit tnt graft` (track clk-rt, 2026-10-21): re-place one MODULE of a design (a clock,
//! a converter, a route: a set of its cells) at every target and in every orientation, the other
//! blocks unchanged, and judge each placement like `circuit tnt compact` (screen tiers, then the
//! redsim judge). The module's `--anchor` (e.g. the cell its clock pulses into, or one of its own
//! blocks) goes to each `--target` cell; the 24 rotations of the cube are tried (`--rotations y`:
//! the 4 turns about y only). A block whose orientation cannot follow a rotation (a wall torch,
//! repeater, trapdoor, fence, dust... turned off the horizontal) makes that rotation invalid.
//! Quasi-connectivity and update order are not rotation-invariant, so every placement is judged.
//! Writes the passing placements, smallest volume first, to DIR/ and DIR/graft.tsv.

use crate::compact::{self, CompactOptions, St, Target};
use anyhow::Result;
use clock::{Clock, Pos};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct GraftOptions {
    pub design: PathBuf,
    pub module: Vec<Pos>,
    pub anchor: Pos,
    pub targets: Vec<Pos>,
    pub all_rotations: bool,
    pub region: Option<(Pos, Pos)>,
    pub target: Target,
    pub ticks: u32,
    pub nth: usize,
    pub kicks: usize,
    pub workers: usize,
    pub out: PathBuf,
}

type M = [[i32; 3]; 3];

fn mul(m: &M, v: Pos) -> Pos {
    [m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2], m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2], m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2]]
}

/// The 24 proper rotations (signed permutation matrices with determinant 1).
fn rotations() -> Vec<M> {
    let perms = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let mut out = Vec::new();
    for p in perms {
        for s in 0..8 {
            let sign = |i: usize| if s >> i & 1 == 1 { -1 } else { 1 };
            let mut m = [[0; 3]; 3];
            for (r, &c) in p.iter().enumerate() {
                m[r][c] = sign(r);
            }
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            if det == 1 {
                out.push(m);
            }
        }
    }
    out
}

fn dir_vec(d: &str) -> Option<Pos> {
    Some(match d {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}

fn vec_dir(v: Pos) -> &'static str {
    match v {
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [1, 0, 0] => "east",
        [-1, 0, 0] => "west",
        [0, 1, 0] => "up",
        _ => "down",
    }
}

/// The state turned by `m` (None when the block cannot take that orientation).
fn rotate_state(s: &str, m: &M) -> Option<String> {
    let id = s.split('[').next().unwrap_or(s);
    // a rotation about y: the game's own rule
    if m[1] == [0, 1, 0] {
        let turns = (0..4u8).find(|t| clock::rotate_pos([1, 0, 0], *t) == mul(m, [1, 0, 0]))?;
        return Some(clock::canon::rotate_state(s, turns));
    }
    let Some(open) = s.find('[') else { return Some(s.to_string()) };
    let props = &s[open + 1..s.len() - 1];
    let six = id.ends_with("observer") || id.ends_with("piston") || id.ends_with("dispenser") || id.ends_with("dropper");
    let mut out = Vec::new();
    for kv in props.split(',') {
        let (k, v) = kv.split_once('=')?;
        match k {
            "facing" if six => out.push(format!("facing={}", vec_dir(mul(m, dir_vec(v)?)))),
            // orientation-free properties
            "powered" | "extended" | "lit" | "unstable" | "waterlogged" | "level" | "power" | "triggered" => out.push(kv.to_string()),
            _ => return None,
        }
    }
    Some(format!("{id}[{}]", out.join(",")))
}

/// `f` over `items` on `workers` threads, results in order.
fn par_map<T: Sync, R: Send>(items: &[T], workers: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = std::sync::atomic::AtomicUsize::new(0);
    let out: std::sync::Mutex<Vec<Option<R>>> = std::sync::Mutex::new((0..items.len()).map(|_| None).collect());
    std::thread::scope(|sc| {
        for _ in 0..workers.max(1) {
            sc.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    let r = f(&items[i]);
                    out.lock().unwrap()[i] = Some(r);
                }
            });
        }
    });
    out.into_inner().unwrap().into_iter().map(|r| r.unwrap()).collect()
}

pub fn graft(o: &GraftOptions) -> Result<usize> {
    let seed = Clock::load(&o.design)?;
    let co = CompactOptions {
        seed: o.design.clone(),
        region: o.region,
        frozen: Default::default(),
        palette: Vec::new(),
        iterations: 0,
        workers: 1,
        rng: 1,
        ticks: o.ticks,
        nth: o.nth,
        kicks: o.kicks,
        target: o.target,
        out: o.out.clone(),
        stale: 0,
        repair: 0,
    };
    let all = crate::precise::default_kicks();
    let k = o.kicks.clamp(1, all.len());
    let kicks: Vec<f64> = (0..k).map(|i| all[i * all.len() / k]).collect();
    let module: BTreeMap<Pos, String> = o.module.iter().filter_map(|p| seed.design.blocks.get(p).map(|s| (*p, s.clone()))).collect();
    anyhow::ensure!(!module.is_empty(), "no design block in --module");
    let mut rest = seed.design.blocks.clone();
    for p in module.keys() {
        rest.remove(p);
    }
    let rots: Vec<M> = rotations().into_iter().filter(|m| o.all_rotations || m[1] == [0, 1, 0]).collect();
    let mut jobs: Vec<St> = Vec::new();
    for m in &rots {
        let states: Option<Vec<(Pos, String)>> = module.iter().map(|(p, s)| rotate_state(s, m).map(|t| (mul(m, [p[0] - o.anchor[0], p[1] - o.anchor[1], p[2] - o.anchor[2]]), t))).collect();
        let Some(states) = states else { continue };
        for t in &o.targets {
            let mut b = rest.clone();
            let mut ok = true;
            for (rel, s) in &states {
                let q = [t[0] + rel[0], t[1] + rel[1], t[2] + rel[2]];
                let inside = o.region.is_none_or(|(lo, hi)| (0..3).all(|i| q[i] >= lo[i] && q[i] <= hi[i]));
                if !inside || b.insert(q, s.clone()).is_some() {
                    ok = false;
                    break;
                }
            }
            if !ok {
                continue;
            }
            // ports on module cells follow the module
            let mut ports = seed.design.ports.clone();
            for p in ports.values_mut() {
                if module.contains_key(p) {
                    let rel = mul(m, [p[0] - o.anchor[0], p[1] - o.anchor[1], p[2] - o.anchor[2]]);
                    *p = [t[0] + rel[0], t[1] + rel[1], t[2] + rel[2]];
                }
            }
            jobs.push(St { b, ports });
        }
    }
    eprintln!("{} placements ({} rotations x {} targets, overlaps dropped)", jobs.len(), rots.len(), o.targets.len());
    let res = par_map(&jobs, o.workers, |st| compact::screen(&seed, st, &co, &kicks));
    let mut passing: Vec<(compact::Score, String, &St)> = Vec::new();
    let mut tiers = [0usize; 5];
    for (st, (s, v)) in jobs.iter().zip(res) {
        tiers[s.tier as usize] += 1;
        if s.tier >= 3 {
            let (s2, v2) = compact::judge(&seed, st, &co);
            if s2.tier == 4 {
                passing.push((s2, v2, st));
                continue;
            }
            let _ = v;
        }
    }
    eprintln!("screen tiers 0-3: {:?}; {} pass the judge", &tiers[..4], passing.len());
    passing.sort_by(|a, b| b.0.cmp(&a.0));
    std::fs::create_dir_all(&o.out)?;
    let mut tsv = String::from("name\tvolume\tsize\tblocks\tverdict\n");
    for (i, (sc, v, st)) in passing.iter().enumerate() {
        let (_, s) = compact::volume(&st.b);
        let vol = -sc.neg_volume;
        let mut d = seed.design.clone();
        d.blocks = st.b.clone();
        d.ports = st.ports.clone();
        d.name = format!("{}-g{i}-v{vol}", seed.design.name);
        std::fs::write(o.out.join(format!("{}.txt", d.name)), format!("# {v}\n{}", d.to_text()))?;
        tsv += &format!("{}\t{vol}\t{}x{}x{} (static)\t{}\t{v}\n", d.name, s[0], s[1], s[2], st.b.len());
        println!("{}: judged volume {vol} (static {}x{}x{}), {} blocks", d.name, s[0], s[1], s[2], st.b.len());
    }
    std::fs::write(o.out.join("graft.tsv"), tsv)?;
    Ok(passing.len())
}
