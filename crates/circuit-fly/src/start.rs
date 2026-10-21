//! `circuit fly start-scan`: give a one-way flying machine a real START device (coordinator
//! 2026-10-21: "start: flip the lever"; the judge's [`circuit::check`] `flying_start`). The
//! machine's own kick (`port start`: a block the judge places for one tick in the cell its trigger
//! observer watches) is replaced by a lever (or a button) IN that cell, hung on a stone in one of its
//! free neighbours: the trigger observer then sees the device flip. The stone must stay put (no
//! slime or honey next to it) and neither block may stand in the machine's corridor (the cells its
//! blocks sweep along `travel`). Both are `port station` blocks, the launch station left behind.
//! Every candidate is judged on redsim with the judge's start stimulus (parked at rest, launched by
//! the first flip, flips again at other phases of the flight); the passing ones are printed fewest
//! blocks first and written to `--out`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mctas::design::Design;

type Pos = [i32; 3];

const DIRS: [(Pos, &str); 6] = [([0, 1, 0], "up"), ([0, -1, 0], "down"), ([0, 0, -1], "north"), ([0, 0, 1], "south"), ([1, 0, 0], "east"), ([-1, 0, 0], "west")];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn parse_pos(s: &str) -> Option<Pos> {
    let v: Vec<i32> = s.split_whitespace().take(3).filter_map(|t| t.parse().ok()).collect();
    (v.len() == 3).then(|| [v[0], v[1], v[2]])
}

/// The device hung on a support `back` of its cell (`back` = support cell - device cell): a lever
/// or a stone button facing away from its support.
fn device(back: Pos, button: bool) -> String {
    let (face, facing) = match back {
        [0, -1, 0] => ("floor", "north"),
        [0, 1, 0] => ("ceiling", "north"),
        [0, 0, -1] => ("wall", "south"),
        [0, 0, 1] => ("wall", "north"),
        [1, 0, 0] => ("wall", "west"),
        _ => ("wall", "east"),
    };
    if button { format!("minecraft:stone_button[face={face},facing={facing},powered=false]") } else { format!("minecraft:lever[face={face},facing={facing},powered=false]") }
}

/// Every candidate of one machine: (support cell, design text).
pub fn candidates(d: &Design, button: bool) -> Result<Vec<(Pos, Design)>> {
    let kick = *d.ports.get("start").context("no `port start` kick to replace")?;
    let travel = d.meta.get("travel").and_then(|t| parse_pos(t)).unwrap_or([1, 0, 0]);
    let dist: i32 = d.meta.get("distance").and_then(|t| t.trim().parse().ok()).unwrap_or(16);
    let mut corridor = std::collections::HashSet::new();
    for p in d.blocks.keys() {
        for j in 0..=dist {
            corridor.insert(add(*p, [travel[0] * j, travel[1] * j, travel[2] * j]));
        }
    }
    if corridor.contains(&kick) {
        anyhow::bail!("the kick cell {kick:?} is in the machine's corridor");
    }
    let sticky = |p: &Pos| d.blocks.get(p).is_some_and(|s| s.contains("slime_block") || s.contains("honey_block"));
    let mut base = d.clone();
    base.ports.remove("start");
    base.meta.remove("port start");
    let mut out = Vec::new();
    for (dv, _) in DIRS {
        let s = add(kick, dv);
        if d.blocks.contains_key(&s) || corridor.contains(&s) || DIRS.iter().any(|(e, _)| sticky(&add(s, *e))) {
            continue;
        }
        let mut c = base.clone();
        c.blocks.insert(s, "minecraft:stone".into());
        c.blocks.insert(kick, device(dv, button));
        c.ports.insert("input".into(), kick);
        c.meta.insert("port input".into(), if button { "button".into() } else { "lever".into() });
        c.ports.insert("station.0".into(), s);
        c.ports.insert("station.1".into(), kick);
        c.name = format!("{}-{}{}", d.name, if button { "button" } else { "lever" }, out.len());
        out.push((s, c));
    }
    Ok(out)
}

/// Judge one candidate on redsim: (pass, the summary line).
pub fn judge(d: &Design) -> Result<(bool, String)> {
    let c = circuit::Circuit::from_design(d.clone())?;
    let r = circuit::verify::sim(&c, &circuit::verify::Options::default())?;
    let pass = r.outcome.as_ref().is_some_and(|o| o.pass);
    Ok((pass, r.summary(&c)))
}

fn is_piston(s: &str) -> bool {
    s.starts_with("minecraft:piston") || s.starts_with("minecraft:sticky_piston")
}

fn facing_of(s: &str) -> Option<Pos> {
    let f = s.split("facing=").nth(1)?.split([',', ']']).next()?;
    DIRS.iter().find(|(_, n)| *n == f).map(|(d, _)| *d)
}

fn neg(a: Pos) -> Pos {
    [-a[0], -a[1], -a[2]]
}

/// Family B ([`watcher_candidates`]): the kick's trigger observer stays, and a launch watcher W
/// joins the machine: an observer on a piston's side (or back), its back on that piston so its
/// pulse fires it directly, stuck to a slime or honey block of the machine through up to
/// `max_glue` extra slime blocks. The lever (or button) stands in the cell W watches, hung on a
/// stone. When the piston W fires moves W's own part, W leaves the lever at the first push, so
/// later flips reach nothing (a watcher left on the parked part sees them all).
pub fn watcher_candidates(d: &Design, button: bool, max_glue: usize) -> Result<Vec<Design>> {
    d.ports.get("start").context("no `port start` kick to replace")?;
    let travel = d.meta.get("travel").and_then(|t| parse_pos(t)).unwrap_or([1, 0, 0]);
    let dist: i32 = d.meta.get("distance").and_then(|t| t.trim().parse().ok()).unwrap_or(16);
    let mut base = d.clone();
    base.ports.remove("start");
    base.meta.remove("port start");
    let glue = |s: &str| s.contains("slime_block") || s.contains("honey_block");
    let mut out: Vec<Design> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (p, ps) in d.blocks.iter().filter(|(_, s)| is_piston(s)) {
        let front = facing_of(ps).unwrap_or([0, 0, 0]);
        for (e, ename) in DIRS {
            let w = add(*p, e);
            if e == front || d.blocks.contains_key(&w) {
                continue;
            }
            let lever = add(w, e);
            if d.blocks.contains_key(&lever) {
                continue;
            }
            let wstate = format!("minecraft:observer[facing={ename},powered=false]");
            // glue sets: W already touches the machine's glue, or 1..max_glue extra slimes chain it
            let mut glue_sets: Vec<Vec<Pos>> = Vec::new();
            let touches = |q: Pos, extra: &[Pos]| DIRS.iter().any(|(f, _)| {
                let r = add(q, *f);
                extra.contains(&r) || d.blocks.get(&r).is_some_and(|s| glue(s))
            });
            if touches(w, &[]) {
                glue_sets.push(Vec::new());
            }
            if max_glue >= 1 {
                for (f1, _) in DIRS {
                    let g1 = add(w, f1);
                    if g1 == lever || d.blocks.contains_key(&g1) {
                        continue;
                    }
                    if touches(g1, &[]) {
                        glue_sets.push(vec![g1]);
                    }
                    if max_glue >= 2 {
                        for (f2, _) in DIRS {
                            let g2 = add(g1, f2);
                            if g2 == w || g2 == lever || d.blocks.contains_key(&g2) {
                                continue;
                            }
                            if touches(g2, &[]) && !touches(g1, &[]) {
                                glue_sets.push(vec![g1, g2]);
                            }
                        }
                    }
                }
            }
            for gs in glue_sets {
                let mut m = base.clone();
                m.blocks.insert(w, wstate.clone());
                for g in &gs {
                    m.blocks.insert(*g, "minecraft:slime_block".into());
                }
                let mut corridor = std::collections::HashSet::new();
                for q in m.blocks.keys() {
                    for j in 0..=dist {
                        corridor.insert(add(*q, [travel[0] * j, travel[1] * j, travel[2] * j]));
                    }
                }
                if corridor.contains(&lever) {
                    continue;
                }
                let sticky = |q: &Pos| m.blocks.get(q).is_some_and(|s| glue(s));
                for (sv, _) in DIRS {
                    let s = add(lever, sv);
                    if s == w || m.blocks.contains_key(&s) || corridor.contains(&s) || DIRS.iter().any(|(f, _)| sticky(&add(s, *f))) {
                        continue;
                    }
                    let mut c = m.clone();
                    c.blocks.insert(s, "minecraft:stone".into());
                    c.blocks.insert(lever, device(sv, button));
                    c.ports.insert("input".into(), lever);
                    c.meta.insert("port input".into(), if button { "button".into() } else { "lever".into() });
                    c.ports.insert("station.0".into(), s);
                    c.ports.insert("station.1".into(), lever);
                    if !seen.insert(c.content_key()) {
                        continue;
                    }
                    c.name = format!("{}-w{}", d.name, out.len());
                    out.push(c);
                }
            }
        }
    }
    let _ = neg;
    Ok(out)
}

/// The box volume and block count of a design (a flying machine's judged box is its own).
fn size(d: &Design) -> (i32, usize) {
    let v = d.bounds().map(|(lo, hi)| (hi[0] - lo[0] + 1) * (hi[1] - lo[1] + 1) * (hi[2] - lo[2] + 1)).unwrap_or(0);
    (v, d.blocks.values().filter(|s| !s.contains("lever") && !s.contains("_button")).count())
}

/// `circuit fly start-scan --watcher`: family B over every design, judged in parallel; passes
/// written to `out`, smallest box first.
pub fn scan_watchers(paths: &[PathBuf], out: Option<&Path>, button: bool, max_glue: usize, threads: usize, list: bool) -> Result<usize> {
    if let Some(o) = out {
        std::fs::create_dir_all(o)?;
    }
    let mut all: Vec<(i32, usize, String, Design)> = Vec::new();
    for path in paths {
        let d = Design::load(path)?;
        let cands = match watcher_candidates(&d, button, max_glue) {
            Ok(c) => c,
            Err(e) => {
                println!("{}: {e}", path.display());
                continue;
            }
        };
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|sc| {
            for _ in 0..threads.max(1) {
                sc.spawn(|| loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(c) = cands.get(i) else { break };
                    let v = judge(c).unwrap_or_else(|e| (false, format!("error: {e:#}")));
                    results.lock().unwrap().push((i, v));
                });
            }
        });
        let mut res = results.into_inner().unwrap();
        res.sort_by_key(|(i, _)| *i);
        let mut n = 0;
        for (i, (pass, line)) in res {
            if list {
                println!("  {line}");
            }
            if pass {
                n += 1;
                let (v, b) = size(&cands[i]);
                // the start device and its glue, as added to this design (to compare members)
                let sig: Vec<String> = cands[i].blocks.iter().filter(|(p, s)| d.blocks.get(*p) != Some(*s)).map(|(p, s)| format!("{},{},{}={}", p[0], p[1], p[2], s.trim_start_matches("minecraft:"))).collect();
                println!("  pass {}: sig {}", cands[i].name, sig.join(" "));
                all.push((v, b, line, cands[i].clone()));
            }
        }
        println!("{}: {n} of {} watcher candidates pass", path.display(), cands.len());
    }
    all.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    for (v, b, line, c) in &all {
        println!("  volume {v}, {b} blocks: {line}");
        if let Some(o) = out {
            c.save(&o.join(format!("{}.txt", c.name)))?;
        }
    }
    Ok(all.len())
}

/// `circuit fly start-scan`: every candidate of every design, judged; passes written to `out`.
pub fn scan(paths: &[PathBuf], out: Option<&Path>, button: bool, list: bool) -> Result<usize> {
    let mut passes = 0;
    if let Some(o) = out {
        std::fs::create_dir_all(o)?;
    }
    for path in paths {
        let d = Design::load(path)?;
        let cands = match candidates(&d, button) {
            Ok(c) => c,
            Err(e) => {
                println!("{}: {e}", path.display());
                continue;
            }
        };
        let mut ok = Vec::new();
        for (s, c) in &cands {
            let (pass, line) = judge(c)?;
            if list || pass {
                println!("  support {s:?}: {line}");
            }
            if pass {
                ok.push(c.clone());
            }
        }
        println!("{}: {} of {} candidates pass", path.display(), ok.len(), cands.len());
        passes += ok.len();
        if let Some(o) = out {
            for c in &ok {
                c.save(&o.join(format!("{}.txt", c.name)))?;
            }
        }
    }
    Ok(passes)
}
