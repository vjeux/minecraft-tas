//! `circuit tnt lever-scan`: give a TNT duper an on/off LEVER (vjeux 2026-10-21: "Can we have an on
//! off switch for all the tnt"). The judge wants the lever off as built, the machine idle and
//! primeless then, so a lever must stop it by holding something powered while it is OFF: an
//! inverter. This tries every inverter around the design: a redstone torch in an air cell (standing,
//! or on a wall in each direction), a stone under or behind it (the cell must be air), a lever on
//! another face of the stone (a free cell), and judges each machine on redsim with the lever
//! stimulus (`circuit::tnt::check_lever`: off at rest and idle; on/off cycles at other moments of
//! the period, each back to the rest state), at the design's declared stand, a few kicks on each
//! ground; the passing ones are printed fewest blocks first and written to `--out`.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use mctas::design::Design;

type Pos = [i32; 3];

/// What to scan.
pub struct LeverOpts {
    /// Cells around the design's box a torch may stand in.
    pub margin: i32,
    /// Remove the design's `port start` (a clock the lever starts by itself).
    pub no_start: bool,
    /// Kicks per ground for a candidate that passes the first one.
    pub kicks: usize,
    pub threads: usize,
    pub out: Option<PathBuf>,
    /// Only torches next to one of these cells (empty: anywhere).
    pub near: Vec<Pos>,
    /// Print every candidate's verdict.
    pub list: bool,
    /// Gates instead of inverters ([`gate_candidates`]).
    pub gate: bool,
    /// A pass must carry this in its line (e.g. `precise:`: the lever keeps the design's flag).
    pub want: String,
    /// With `out`: also write the candidates whose failure contains this (FAIL-<i>.txt).
    pub keep_fail: String,
}

const DIRS: [(Pos, &str); 6] = [([0, 1, 0], "up"), ([0, -1, 0], "down"), ([0, 0, -1], "north"), ([0, 0, 1], "south"), ([1, 0, 0], "east"), ([-1, 0, 0], "west")];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

/// One candidate: (torch cell, torch state, stone cell, lever cell, lever state).
type Cand = (Pos, String, Pos, Pos, String);

fn candidates(d: &Design, margin: i32) -> Vec<Cand> {
    let Some((lo, hi)) = d.bounds() else { return Vec::new() };
    let free = |p: Pos| !d.blocks.contains_key(&p);
    let mut v = Vec::new();
    for x in lo[0] - margin..=hi[0] + margin {
        for y in lo[1] - margin..=hi[1] + margin {
            for z in lo[2] - margin..=hi[2] + margin {
                let t = [x, y, z];
                if !free(t) {
                    continue;
                }
                // a torch near nothing holds nothing: one of its neighbours must be a design block
                if !DIRS.iter().any(|(dv, _)| !free(add(t, *dv))) {
                    continue;
                }
                // standing (stone below) or on a wall (stone behind: a wall torch facing east hangs
                // on the block west of it)
                let mut torches: Vec<(Pos, String)> = vec![([0, -1, 0], "minecraft:redstone_torch[lit=true]".to_string())];
                for (dv, name) in &DIRS[2..] {
                    torches.push(([-dv[0], -dv[1], -dv[2]], format!("minecraft:redstone_wall_torch[facing={name},lit=true]")));
                }
                for (back, ts) in torches {
                    let x = add(t, back);
                    if !free(x) {
                        continue;
                    }
                    for (dv, name) in &DIRS {
                        let l = add(x, *dv);
                        if l == t || !free(l) {
                            continue;
                        }
                        // a wall lever facing east hangs on the block west of it
                        let ls = match *name {
                            "up" => "minecraft:lever[face=floor,facing=north,powered=false]".to_string(),
                            "down" => "minecraft:lever[face=ceiling,facing=north,powered=false]".to_string(),
                            f => format!("minecraft:lever[face=wall,facing={f},powered=false]"),
                        };
                        v.push((t, ts.clone(), x, l, ls));
                    }
                }
            }
        }
    }
    v
}

fn build(d: &Design, (t, ts, x, l, ls): &Cand) -> Design {
    let mut e = d.clone();
    e.blocks.insert(*t, ts.clone());
    e.blocks.insert(*x, "minecraft:stone".into());
    e.blocks.insert(*l, ls.clone());
    e.ports.insert("input".into(), *l);
    e.meta.insert("port input".into(), "lever".into());
    e
}

/// `--gate`: a lever that runs the machine by PUSHING one of its own blocks into place (a sticky
/// piston the lever powers directly: lever off, the block stands one cell back, the machine's loop
/// broken; on, the piston pushes it home and its arrival restarts the loop at the loop's own rest
/// point). Every block of the design, every direction, every lever cell next to the piston on a
/// support (a new stone or a stone or obsidian block of the design). Each candidate as a full
/// design.
fn gate_candidates(d: &Design) -> Vec<(String, Design)> {
    let free = |e: &Design, p: Pos| !e.blocks.contains_key(&p);
    let mut v = Vec::new();
    for (&w, st) in &d.blocks {
        if st.contains("piston") || st.starts_with("minecraft:water") || st.starts_with("minecraft:tnt") || st.contains("lever") {
            continue;
        }
        for (dv, name) in &DIRS {
            let b = [w[0] - dv[0], w[1] - dv[1], w[2] - dv[2]];
            let q = [b[0] - dv[0], b[1] - dv[1], b[2] - dv[2]];
            if !free(d, b) || !free(d, q) {
                continue;
            }
            let mut base = d.clone();
            base.blocks.remove(&w);
            base.blocks.insert(b, st.clone());
            base.blocks.insert(q, format!("minecraft:sticky_piston[extended=false,facing={name}]"));
            for (lv, _) in &DIRS {
                let l = add(q, *lv);
                if l == b || !free(&base, l) {
                    continue;
                }
                for (sv, sname) in &DIRS {
                    let s = add(l, *sv);
                    if s == q {
                        continue;
                    }
                    let new_stone = free(&base, s);
                    if !new_stone && !base.blocks.get(&s).is_some_and(|x| x == "minecraft:stone" || x == "minecraft:obsidian") {
                        continue;
                    }
                    // the lever hangs on the support: on its top (floor), under it (ceiling), or on
                    // a wall facing away from it
                    let ls = match *sname {
                        "down" => "minecraft:lever[face=floor,facing=north,powered=false]".to_string(),
                        "up" => "minecraft:lever[face=ceiling,facing=north,powered=false]".to_string(),
                        "north" => "minecraft:lever[face=wall,facing=south,powered=false]".to_string(),
                        "south" => "minecraft:lever[face=wall,facing=north,powered=false]".to_string(),
                        "east" => "minecraft:lever[face=wall,facing=west,powered=false]".to_string(),
                        _ => "minecraft:lever[face=wall,facing=east,powered=false]".to_string(),
                    };
                    let mut e = base.clone();
                    if new_stone {
                        e.blocks.insert(s, "minecraft:stone".into());
                    }
                    e.blocks.insert(l, ls);
                    e.ports.insert("input".into(), l);
                    e.meta.insert("port input".into(), "lever".into());
                    v.push((format!("gate: {} {w:?} pushed home {name} by a piston at {q:?}, lever {l:?} on {}{s:?}", clock::short(st), if new_stone { "new stone " } else { "" }), e));
                }
            }
        }
    }
    v
}

/// The lever judge on redsim at the declared stand: `kicks` kicks on each ground (the first kick
/// on grass alone when `kicks` is 0). Ok(the outcome line) or Err(why it fails, and the machine as
/// it stands still at the end of its first off window when it does: the design to try built in
/// that state, a clock stopped elsewhere than where the file draws it).
fn judge(d: &Design, kicks: usize) -> Result<String, (String, Option<Design>)> {
    let fail = |w: String| (w, None);
    let mut c = circuit::Circuit::from_design(d.clone()).map_err(|e| fail(format!("{e:#}")))?;
    c.drop = circuit::robust::declared_stand(&c);
    let kind = c.kind.clone().ok_or(fail("no kind".into()))?;
    let o = circuit::verify::Options { margin: 3, ..Default::default() };
    let (s, _) = circuit::verify::schedule(&c, &o).map_err(|e| fail(format!("{e:#}")))?;
    let jobs: Vec<(circuit::Ground, f64)> = if kicks == 0 {
        vec![(circuit::Ground::Grass, 0.0)]
    } else {
        circuit::Ground::ALL.iter().flat_map(|&g| (0..kicks).map(move |k| (g, k as f64 / kicks as f64))).collect()
    };
    let mut line = String::new();
    for (g, kick) in jobs {
        let t = circuit::run::tnt_kick_trace(&c, &s, 3, kick, g, false).map_err(fail)?;
        let out = circuit::check::check(&c, &kind, &s, &t);
        if !out.pass {
            return Err((out.why, settled(d, &s, &t)));
        }
        let m: Vec<String> = out.metrics.iter().map(|(a, b)| format!("{a} {b}")).collect();
        line = format!("{}; {}", m.join(", "), out.why);
    }
    Ok(line)
}

/// The design rebuilt from the run's frame at the end of its first off window, when the machine
/// stands still there (no moving block over the window's idle part).
fn settled(d: &Design, s: &circuit::stimulus::Schedule, t: &circuit::trace::Trace) -> Option<Design> {
    let (_, b, _) = s.phases.iter().find(|(_, _, l)| l == "off 1")?;
    let b = (*b as usize).min(t.cells.len());
    let idle = circuit::tnt::LEVER_IDLE as usize;
    if b < idle || !(b - idle..b).all(|f| t.cells[f] == t.cells[b - 1]) {
        return None;
    }
    let mut e = d.clone();
    e.blocks = t.cells[b - 1].iter().map(|(p, id)| (*p, t.names[*id as usize].split([' ', '{']).next().unwrap_or_default().to_string())).collect();
    if e.blocks.values().any(|st| st.starts_with("minecraft:moving_piston")) || e.blocks == d.blocks {
        return None;
    }
    Some(e)
}

pub fn scan(path: &Path, o: &LeverOpts) -> anyhow::Result<()> {
    let mut d = Design::load(path)?;
    if o.no_start {
        d.ports.remove("start");
        d.meta.remove("port start");
    }
    let cands: Vec<(String, Design)> = if o.gate {
        gate_candidates(&d)
    } else {
        candidates(&d, o.margin)
            .into_iter()
            .filter(|c| o.near.is_empty() || o.near.iter().any(|n| DIRS.iter().any(|(dv, _)| add(*n, *dv) == c.0)))
            .map(|c| (format!("torch {:?} {} on stone {:?}, lever {:?} {}", c.0, clock::short(&c.1), c.2, c.3, clock::short(&c.4)), build(&d, &c)))
            .collect()
    };
    eprintln!("[lever-scan] {}: {} {} to try", d.name, cands.len(), if o.gate { "gates" } else { "inverters" });
    let next = AtomicUsize::new(0);
    let found: std::sync::Mutex<Vec<(usize, String, String, Design)>> = std::sync::Mutex::new(Vec::new());
    let why: std::sync::Mutex<std::collections::BTreeMap<String, usize>> = std::sync::Mutex::new(Default::default());
    std::thread::scope(|sc| {
        for _ in 0..o.threads.max(1) {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= cands.len() {
                    break;
                }
                let (label, e) = &cands[i];
                // as drawn, then (a clock the lever stopped elsewhere) built as it stops
                let r = match judge(e, 0) {
                    Ok(_) => judge(e, o.kicks).map(|l| (l, e.clone())),
                    Err((w, Some(st))) => match judge(&st, 0).and_then(|_| judge(&st, o.kicks)) {
                        Ok(l) => Ok((format!("{l} (built as it stops)"), st)),
                        Err((w2, _)) => Err((format!("{w} / built as it stops: {w2}"), None)),
                    },
                    Err(x) => Err(x),
                };
                let r = match r {
                    Ok((line, _)) if !o.want.is_empty() && !line.contains(&o.want) => Err((format!("passes without `{}`: {}", o.want, line.chars().take(120).collect::<String>()), None)),
                    r => r,
                };
                match r {
                    Ok((line, design)) => {
                        let vol = design.bounds().map(|(lo, hi)| (0..3).map(|i| (hi[i] - lo[i] + 1) as usize).product::<usize>()).unwrap_or(0);
                        found.lock().unwrap().push((vol * 1000 + design.blocks.len(), label.clone(), line, design))
                    }
                    Err((w, _)) => {
                        if o.list {
                            eprintln!("[lever-scan] {label}: {}", w.chars().take(260).collect::<String>());
                        }
                        // --keep-fail: the candidates failing this way written too (FAIL-<i>.txt), to study
                        if let (Some(dir), false) = (&o.out, o.keep_fail.is_empty())
                            && w.contains(&o.keep_fail)
                        {
                            let _ = std::fs::create_dir_all(dir);
                            let _ = std::fs::write(dir.join(format!("FAIL-{i}.txt")), format!("# circuit tnt lever-scan: {label}: {w}\n{}", cands[i].1.to_text()));
                        }
                        let key: String = w.chars().take(90).collect();
                        *why.lock().unwrap().entry(key).or_default() += 1;
                    }
                }
            });
        }
    });
    let mut found = found.into_inner().unwrap();
    found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    let mut whys: Vec<(String, usize)> = why.into_inner().unwrap().into_iter().collect();
    whys.sort_by(|a, b| b.1.cmp(&a.1));
    for (w, n) in whys.iter().take(10) {
        eprintln!("[lever-scan]   {n} fail: {w}");
    }
    for (k, (n, label, line, design)) in found.iter().enumerate() {
        println!("PASS box {} ({} blocks): {label}: {line}", n / 1000, n % 1000);
        if let Some(dir) = &o.out {
            std::fs::create_dir_all(dir)?;
            let mut e = design.clone();
            e.name = format!("{}-lever-{k}", d.name);
            std::fs::write(dir.join(format!("{}.txt", e.name)), format!("# circuit tnt lever-scan of {}: {label}: {line}\n{}", path.display(), e.to_text()))?;
        }
    }
    eprintln!("[lever-scan] {} of {} pass", found.len(), cands.len());
    Ok(())
}
