//! Pure observer rings: a closed self-avoiding loop of n cells on the lattice, an observer in each
//! cell facing (watching) the previous cell of the loop. One kick (an observer placed last) starts
//! one 2 gt pulse that travels 2 gt per observer, so P = 2n; the loop of 2 (two observers face to
//! face) is the exception, P = 6 (the pulse comes back while the first observer is still on, and
//! the falling edge restarts it). Lattice loops have even length, so single-kick pure observer
//! clocks reach exactly P = 6 and P = 8, 12, 16, ... (multiples of 4).

use crate::sim::{self, Kick, Spec};
use anyhow::Result;
use redsim::{Block, BlockPos, Dir};
use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

type P3 = [i32; 3];

const STEPS: [P3; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn add(a: P3, b: P3) -> P3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: P3, b: P3) -> P3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
/// A quarter turn about the vertical axis.
pub fn rot(p: P3) -> P3 {
    [-p[2], p[1], p[0]]
}
pub fn mirror(p: P3) -> P3 {
    [-p[0], p[1], p[2]]
}

pub fn dir_of(v: P3) -> Dir {
    match v {
        [1, 0, 0] => Dir::East,
        [-1, 0, 0] => Dir::West,
        [0, 1, 0] => Dir::Up,
        [0, -1, 0] => Dir::Down,
        [0, 0, 1] => Dir::South,
        _ => Dir::North,
    }
}

/// A design as (cell, facing vector) pairs, normalized by translation and sorted.
type Form = Vec<(P3, P3)>;

fn normalize(mut f: Form) -> Form {
    let mut lo = [i32::MAX; 3];
    for (c, _) in &f {
        for i in 0..3 {
            lo[i] = lo[i].min(c[i]);
        }
    }
    for (c, _) in f.iter_mut() {
        *c = sub(*c, lo);
    }
    f.sort();
    f
}

/// The canonical form under translation and the 4 horizontal rotations.
pub fn canonical(f: &Form) -> Form {
    let mut best: Option<Form> = None;
    let mut cur = f.clone();
    for _ in 0..4 {
        cur = cur.iter().map(|(c, d)| (rot(*c), rot(*d))).collect();
        let n = normalize(cur.clone());
        if best.as_ref().is_none_or(|b| n < *b) {
            best = Some(n);
        }
    }
    best.unwrap()
}

/// Every oriented loop of length n through the origin as the observer form (cell, facing the
/// previous cell), canonical up to translation and rotation.
fn ring_forms(n: usize) -> HashSet<Form> {
    let mut out = HashSet::new();
    if n == 2 {
        for s in STEPS {
            let f: Form = vec![([0, 0, 0], s), (s, sub([0, 0, 0], s))];
            out.insert(canonical(&f));
        }
        return out;
    }
    let mut path: Vec<P3> = vec![[0, 0, 0]];
    let mut on: HashSet<P3> = HashSet::from([[0, 0, 0]]);
    fn dfs(n: usize, path: &mut Vec<P3>, on: &mut HashSet<P3>, out: &mut HashSet<Form>) {
        let last = *path.last().unwrap();
        let left = n - path.len();
        for s in STEPS {
            let c = add(last, s);
            let dist = c[0].abs() + c[1].abs() + c[2].abs();
            if left == 0 {
                if c == [0, 0, 0] {
                    let f: Form = (0..n).map(|i| (path[i], sub(path[(i + n - 1) % n], path[i]))).collect();
                    out.insert(canonical(&f));
                }
                continue;
            }
            if on.contains(&c) || dist as usize > left {
                continue;
            }
            path.push(c);
            on.insert(c);
            dfs(n, path, on, out);
            on.remove(&c);
            path.pop();
        }
    }
    dfs(n, &mut path, &mut on, &mut out);
    out
}

pub fn spec_of(name: &str, f: &Form) -> Spec {
    let blocks: Vec<(BlockPos, Block)> =
        f.iter().map(|(c, d)| (BlockPos::new(c[0], c[1], c[2]), Block::Observer { facing: dir_of(*d), powered: false })).collect();
    let first = blocks[0].0;
    Spec { name: name.to_string(), blocks, kick: Kick::Last(first), output: Some(first) }
}

pub fn run(max_len: usize, out_dir: Option<PathBuf>, write_max_period: u32) -> Result<()> {
    if let Some(d) = &out_dir {
        std::fs::create_dir_all(d)?;
    }
    println!("| observers | types (rotation classes) | chiral pairs | periods (count) | smallest volume |");
    println!("|---|---|---|---|---|");
    for n in (2..=max_len).step_by(2) {
        let forms = ring_forms(n);
        let mut forms: Vec<Form> = forms.into_iter().collect();
        forms.sort();
        let set: HashSet<Form> = forms.iter().cloned().collect();
        let mut chiral = 0;
        let mut periods: BTreeMap<String, usize> = BTreeMap::new();
        let mut min_vol = i64::MAX;
        for (i, f) in forms.iter().enumerate() {
            let m = canonical(&f.iter().map(|(c, d)| (mirror(*c), mirror(*d))).collect());
            if m != *f && set.contains(&m) {
                chiral += 1;
            }
            let name = format!("clk-obs-ring{n}-{i:05}");
            let spec = spec_of(&name, f);
            let r = sim::simulate(&spec, &sim::Opts { horizon: 400, ..Default::default() });
            let key = match &r.verdict {
                sim::Verdict::Clock => format!("P={}", r.period),
                v => format!("{v:?}"),
            };
            *periods.entry(key).or_default() += 1;
            min_vol = min_vol.min(r.volume());
            if let Some(d) = &out_dir {
                if r.verdict == sim::Verdict::Clock && r.period <= write_max_period {
                    let text = spec.to_text(&format!(
                        "Pure observer ring of {n}: each observer watches the previous one; one kick (the first observer placed last) starts one 2 gt pulse. P={} (redsim), family {}.",
                        r.period,
                        r.family_name()
                    ));
                    std::fs::write(d.join(format!("{name}.txt")), text)?;
                }
            }
        }
        let p: Vec<String> = periods.iter().map(|(k, v)| format!("{k} ({v})")).collect();
        println!("| {n} | {} | {} | {} | {} |", forms.len(), chiral / 2, p.join(", "), min_vol);
    }
    Ok(())
}

/// Experiment (not a clock design): a 2 x n/2 rectangle ring of observers, with pulses injected
/// at observer 0 at the given ticks (its tick scheduled directly, as if it saw a change), then
/// run: the steady-state period of a ring carrying several pulses. Shows which pulse trains a
/// pure observer ring can carry (spacing >= 4 or 5, both parities), i.e. what a starter would
/// have to inject for an odd-period pure observer clock.
pub fn run_pulsetrain(n: usize, times: &[u32]) -> Result<()> {
    anyhow::ensure!(n >= 4 && n % 2 == 0, "ring length must be even >= 4");
    let half = n / 2;
    // cells in loop order: top row west->east, bottom row east->west
    let mut cells: Vec<P3> = (0..half as i32).map(|x| [x, 0, 0]).collect();
    cells.extend((0..half as i32).rev().map(|x| [x, 0, 1]));
    let f: Form = (0..n).map(|i| (cells[i], sub(cells[(i + n - 1) % n], cells[i]))).collect();
    let spec = Spec { name: "pulsetrain".into(), blocks: f.iter().map(|(c, d)| (BlockPos::new(c[0], c[1], c[2]), Block::Observer { facing: dir_of(*d), powered: false })).collect(), kick: Kick::None, output: None };
    let mut w = sim::build(&spec, 4);
    let o0 = BlockPos::new(cells[0][0], cells[0][1], cells[0][2]);
    let mut keys: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
    let last = times.iter().copied().max().unwrap_or(0);
    for t in 0..20000u32 {
        if times.contains(&t) {
            w.schedule_tick(o0, redsim::Kind::Observer, 2, redsim::ticks::Priority::Normal);
        }
        w.tick();
        if t > last {
            let k = w.state_key();
            if let Some(t0) = keys.get(&k) {
                let p = t - t0;
                // the observer 0's on ticks in one period
                let mut ons = Vec::new();
                for i in 0..p {
                    w.tick();
                    if matches!(w.get(o0), Block::Observer { powered: true, .. }) {
                        ons.push(i);
                    }
                }
                println!("ring {n}, pulses injected at {times:?}: P = {p} (warmup {t0}), observer 0 on at {ons:?} of each period");
                return Ok(());
            }
            keys.insert(k, t);
        }
        if w.idle() {
            println!("ring {n}, pulses injected at {times:?}: stops at tick {t}");
            return Ok(());
        }
    }
    println!("ring {n}: no repeat");
    Ok(())
}
/// of an observer: the observer before it (straight through: its back faces the relay) powers
/// it, the observer after it watches it. A note block or an iron trapdoor follows its power at
/// once (P = 2 x observers, so an odd number of observers gives P = 2 mod 4); a copper bulb also
/// toggles (P doubles); a lamp goes dark 4 gt late (its watcher sees two edges per pass).
pub fn run_relays(max_len: usize, kinds: &str, out_dir: Option<PathBuf>, write_max_period: u32) -> Result<()> {
    if let Some(d) = &out_dir {
        std::fs::create_dir_all(d)?;
    }
    let kinds: Vec<(&str, Block)> = kinds
        .split(',')
        .filter_map(|k| {
            let s = match k {
                "note" => "minecraft:note_block",
                "lamp" => "minecraft:redstone_lamp[lit=false]",
                "bulb" => "minecraft:waxed_copper_bulb[lit=false,powered=false]",
                "trap" => "minecraft:iron_trapdoor[facing=north,half=bottom,open=false,powered=false]",
                _ => return None,
            };
            Some((k, Block::parse(s).unwrap()))
        })
        .collect();
    println!("| ring cells | observers | relay | types | periods (count) |");
    println!("|---|---|---|---|---|");
    for n in (4..=max_len).step_by(2) {
        let mut forms: Vec<Form> = ring_forms(n).into_iter().collect();
        forms.sort();
        for (kname, kb) in &kinds {
            let mut seen: HashSet<u128> = HashSet::new();
            let mut periods: BTreeMap<String, usize> = BTreeMap::new();
            let mut written_periods: HashSet<u32> = HashSet::new();
            for f in &forms {
                let at: std::collections::HashMap<P3, P3> = f.iter().copied().collect();
                for (s, _) in f {
                    // the predecessor p of s: the observer whose back faces s, straight through
                    let Some((p, dp)) = f.iter().find(|(c, d)| add(*c, *d) != *s && sub(*c, *d) == *s && at.contains_key(&add(*c, *d))) else { continue };
                    let _ = (p, dp);
                    let mut spec = spec_of("", f);
                    let sp = BlockPos::new(s[0], s[1], s[2]);
                    for (q, b) in spec.blocks.iter_mut() {
                        if *q == sp {
                            *b = *kb;
                        }
                    }
                    if !seen.insert(crate::census::canon_hash(&spec.blocks)) {
                        continue;
                    }
                    let first_obs = spec.blocks.iter().find(|(q, _)| *q != sp).map(|(q, _)| *q).unwrap();
                    spec.kick = Kick::Last(first_obs);
                    spec.output = Some(first_obs);
                    let r = sim::simulate(&spec, &sim::Opts { horizon: 2000, ..Default::default() });
                    let key = match &r.verdict {
                        sim::Verdict::Clock => format!("P={}", r.period),
                        v => format!("{v:?}"),
                    };
                    *periods.entry(key).or_default() += 1;
                    if let Some(d) = &out_dir {
                        if r.verdict == sim::Verdict::Clock && r.period <= write_max_period && written_periods.insert(r.period) {
                            let name = format!("clk-obs-{kname}ring{n}-p{}", r.period);
                            spec.name = name.clone();
                            let text = spec.to_text(&format!(
                                "A ring of {} observers and one {kname} relay ({n} cells): the observer before the relay powers it, the next one watches it. P={} (redsim), family {}.",
                                n - 1,
                                r.period,
                                r.family_name()
                            ));
                            std::fs::write(d.join(format!("{name}.txt")), text)?;
                        }
                    }
                }
            }
            let p: Vec<String> = periods.iter().map(|(k, v)| format!("{k} ({v})")).collect();
            println!("| {n} | {} | {kname} | {} | {} |", n - 1, seen.len(), p.join(", "));
        }
    }
    Ok(())
}
