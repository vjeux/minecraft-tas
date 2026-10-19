//! `mctas door boxproof ...`: whole-box bounded model checking of flat (one z layer) 3x3 doors.
//! See designs/piston-door-3x3/tracks/boxproof/NOTES.md for the model, its coverage and results.

pub mod alg;
pub mod bmc;
pub mod diff;
pub mod round;
pub mod state;
pub mod tick;

use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub struct Args {
    pub pos: Vec<String>,
    pub kv: BTreeMap<String, String>,
}

impl Args {
    pub fn parse(v: &[String]) -> Args {
        let mut pos = Vec::new();
        let mut kv = BTreeMap::new();
        let mut i = 0;
        while i < v.len() {
            if let Some(k) = v[i].strip_prefix("--") {
                if let Some((a, b)) = k.split_once('=') {
                    kv.insert(a.to_string(), b.to_string());
                } else if i + 1 < v.len() && !v[i + 1].starts_with("--") {
                    kv.insert(k.to_string(), v[i + 1].clone());
                    i += 1;
                } else {
                    kv.insert(k.to_string(), "true".into());
                }
            } else {
                pos.push(v[i].clone());
            }
            i += 1;
        }
        Args { pos, kv }
    }
    pub fn get<T: std::str::FromStr>(&self, k: &str, d: T) -> T {
        self.kv.get(k).and_then(|v| v.parse().ok()).unwrap_or(d)
    }
    pub fn has(&self, k: &str) -> bool {
        self.kv.contains_key(k)
    }
    pub fn s(&self, k: &str) -> Option<&str> {
        self.kv.get(k).map(|s| s.as_str())
    }
}

pub fn cli(raw: &[String]) -> Result<()> {
    let a = Args::parse(raw);
    match a.pos.first().map(|s| s.as_str()) {
        Some("fuzz") => fuzz(&a),
        Some("case") => case(&a),
        Some("bmc") => bmc::cli(&a),
        Some("catalog") => catalog(&a),
        Some("batch") => batch(&a),
        Some("fill-column") => fill_column(&a),
        Some("watch") => watch(&a),
        Some("replay") => replay(&a),
        Some("run-list") => run_list(&a),
        _ => bail!("usage: door boxproof fuzz|case|bmc|catalog|batch [--flags]"),
    }
}

/// Families catalog rows -> mechanism designs (pistons + the 9 door blocks), one per row:
/// `catalog --tsv catalog.tsv --out DIR [--only 7x8]`. Prints `id w h dx dy path` per row.
fn catalog(a: &Args) -> Result<()> {
    let tsv = a.s("tsv").ok_or_else(|| anyhow::anyhow!("--tsv"))?;
    let out = a.s("out").unwrap_or("/tmp/bp/mech");
    let only = a.s("only");
    std::fs::create_dir_all(out)?;
    let text = std::fs::read_to_string(tsv)?;
    let mut lines = text.lines();
    let head: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
    let col = |n: &str| head.iter().position(|h| *h == n);
    let (ci, cb, co, cp) = (col("id").unwrap(), col("box").unwrap(), col("door_offset").unwrap(), col("pieces").unwrap());
    for l in lines {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() <= cp {
            continue;
        }
        if let Some(o) = only {
            if f[cb] != o {
                continue;
            }
        }
        let (w, h) = f[cb].split_once('x').map(|(a, b)| (a.parse::<i32>().unwrap_or(0), b.parse::<i32>().unwrap_or(0))).unwrap_or((0, 0));
        let (dx, dy) = f[co].split_once(',').map(|(a, b)| (a.parse::<i32>().unwrap_or(0), b.parse::<i32>().unwrap_or(0))).unwrap_or((0, 0));
        let mut blocks: Vec<((i32, i32), String)> = Vec::new();
        for y in 0..3 {
            for x in 0..3 {
                blocks.push(((x, y), "minecraft:stone".into()));
            }
        }
        for p in f[cp].split_whitespace() {
            let sticky = p.starts_with('S');
            let fc = match p.chars().nth(1) {
                Some('E') => "east",
                Some('W') => "west",
                Some('U') => "up",
                _ => "down",
            };
            let inner = p.split_once('(').map(|x| x.1.trim_end_matches(')')).unwrap_or("");
            let (x, y) = inner.split_once(',').map(|(a, b)| (a.trim().parse::<i32>().unwrap_or(0), b.trim().parse::<i32>().unwrap_or(0))).unwrap_or((0, 0));
            blocks.push(((x, y), format!("minecraft:{}piston[extended=false,facing={fc}]", if sticky { "sticky_" } else { "" })));
        }
        let mut s = format!("name: {}\nport door: 0 0 0\norigin: {} 0 0\npalette:\n", f[ci], -dx);
        let mut pal: Vec<String> = Vec::new();
        for (_, st) in &blocks {
            if !pal.contains(st) {
                pal.push(st.clone());
            }
        }
        let chars: Vec<char> = "ABCDEFGHIJ".chars().collect();
        for (i, p) in pal.iter().enumerate() {
            s.push_str(&format!("  {} {p}\n", chars[i]));
        }
        for y in -dy..h - dy {
            let row: String = (-dx..w - dx).map(|x| blocks.iter().find(|(q, _)| *q == (x, y)).map(|(_, st)| chars[pal.iter().position(|p| p == st).unwrap()]).unwrap_or('.')).collect();
            s.push_str(&format!("layer {y}:\n  {row}\n"));
        }
        let path = format!("{out}/{}.txt", f[ci]);
        std::fs::write(&path, s)?;
        println!("{} {w} {h} {dx} {dy} {path}", f[ci]);
    }
    Ok(())
}

/// Run many `bmc` instances as child processes: `batch --list FILE (lines "id w h dx dy path")
/// --jobs N --timeout S -- <bmc flags>`; one log per instance in --logs.
fn batch(a: &Args) -> Result<()> {
    let list = std::fs::read_to_string(a.s("list").ok_or_else(|| anyhow::anyhow!("--list"))?)?;
    let jobs: usize = a.get("jobs", 8);
    let timeout: u64 = a.get("timeout", 3600);
    let logs = a.s("logs").unwrap_or("/tmp/bp/logs").to_string();
    std::fs::create_dir_all(&logs)?;
    let extra: Vec<String> = a.s("bmc").unwrap_or("").split_whitespace().map(String::from).collect();
    let items: Vec<Vec<String>> = list.lines().filter(|l| !l.trim().is_empty()).map(|l| l.split_whitespace().map(String::from).collect()).collect();
    let next = Arc::new(AtomicUsize::new(0));
    let items = Arc::new(items);
    let exe = std::env::current_exe()?;
    let mut hs = Vec::new();
    for _ in 0..jobs {
        let (next, items, extra, logs, exe) = (next.clone(), items.clone(), extra.clone(), logs.clone(), exe.clone());
        hs.push(std::thread::spawn(move || {
            loop {
                let k = next.fetch_add(1, Ordering::SeqCst);
                if k >= items.len() {
                    break;
                }
                let it = &items[k];
                let log = format!("{logs}/{}.log", it[0]);
                let mut args: Vec<String> = vec!["door".into(), "boxproof".into(), "bmc".into(), "--w".into(), it[1].clone(), "--h".into(), it[2].clone(), "--dx".into(), it[3].clone(), "--dy".into(), it[4].clone(), "--out".into(), format!("{logs}/{}", it[0])];
                if it.len() > 5 && it[5] != "-" {
                    args.push("--fix".into());
                    args.push(it[5].clone());
                }
                // per-item extra flags after the path
                if it.len() > 6 {
                    args.extend(it[6..].iter().cloned());
                }
                args.extend(extra.iter().cloned());
                let t0 = std::time::Instant::now();
                let f = std::fs::File::create(&log).unwrap();
                let mut child = std::process::Command::new(&exe).args(&args).stdout(f.try_clone().unwrap()).stderr(f).spawn().unwrap();
                let status = loop {
                    if let Ok(Some(s)) = child.try_wait() {
                        break format!("exit {s}");
                    }
                    if t0.elapsed().as_secs() > timeout {
                        let _ = child.kill();
                        let _ = child.wait();
                        break "TIMEOUT".to_string();
                    }
                    std::thread::sleep(std::time::Duration::from_millis(500));
                };
                let res = std::fs::read_to_string(&log).unwrap_or_default();
                let last = res.lines().filter(|l| l.starts_with("SAT") || l.starts_with("UNSAT") || l.starts_with("built")).last().unwrap_or("").to_string();
                println!("{} {:.0}s {status}: {last}", it[0], t0.elapsed().as_secs_f64());
            }
        }));
    }
    for h in hs {
        let _ = h.join();
    }
    Ok(())
}

/// Fill the LAST cell of markdown table rows: `fill-column --file F --map M` where M has lines
/// `<row first-cell prefix>\t<new cell>`; a row whose last cell is `?` and whose first cell
/// starts with a prefix gets the new cell (a doc helper for docs/coverage.md's tool columns).
fn fill_column(a: &Args) -> Result<()> {
    let file = a.s("file").ok_or_else(|| anyhow::anyhow!("--file"))?;
    let map: Vec<(String, String)> = std::fs::read_to_string(a.s("map").ok_or_else(|| anyhow::anyhow!("--map"))?)?
        .lines()
        .filter_map(|l| l.split_once('\t').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    let text = std::fs::read_to_string(file)?;
    let mut out = String::new();
    let mut n = 0;
    for line in text.lines() {
        let mut l = line.to_string();
        if let Some(rest) = line.strip_prefix("| ") {
            if let Some(stem) = line.strip_suffix(" ? |") {
                if let Some((_, v)) = map.iter().find(|(k, _)| rest.starts_with(k.as_str())) {
                    l = format!("{stem} {v} |");
                    n += 1;
                }
            }
        }
        out.push_str(&l);
        out.push('\n');
    }
    std::fs::write(file, out)?;
    println!("{n} rows filled");
    Ok(())
}

/// Block until a line containing `--pattern` (default "screen Pass") appears in one of the
/// files given by `--files a,b,c` (or every `*.log` of `--dir D`, minus names containing any of
/// `--skip x,y`); print the matching lines and exit. For a background watcher on find runs.
fn watch(a: &Args) -> Result<()> {
    let pat = a.s("pattern").unwrap_or("screen Pass").to_string();
    let skip: Vec<String> = a.s("skip").map(|s| s.split(',').map(String::from).collect()).unwrap_or_default();
    let every: u64 = a.get("interval", 30);
    loop {
        let mut files: Vec<std::path::PathBuf> = a.s("files").map(|s| s.split(',').map(std::path::PathBuf::from).collect()).unwrap_or_default();
        if let Some(dirs) = a.s("dir") {
            for dir in dirs.split(',') {
                if let Ok(rd) = std::fs::read_dir(dir) {
                    for e in rd.flatten() {
                        let p = e.path();
                        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        if name.ends_with(".log") && !skip.iter().any(|s| name.contains(s.as_str())) {
                            files.push(p);
                        }
                    }
                }
            }
        }
        let mut hits = Vec::new();
        for f in &files {
            if let Ok(t) = std::fs::read_to_string(f) {
                for l in t.lines().filter(|l| l.contains(&pat)) {
                    hits.push(format!("{}: {l}", f.display()));
                }
            }
        }
        if !hits.is_empty() {
            for h in hits {
                println!("{h}");
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(every));
    }
}

/// Replay one design through the model and redsim tick by tick (`--design F --w --h --dx --dy
/// [--lever-on] [--ticks N]`): the first disagreement, the flags and the violations.
fn replay(a: &Args) -> Result<()> {
    let w: i32 = a.get("w", 7);
    let h: i32 = a.get("h", 8);
    let dx: i32 = a.get("dx", 2);
    let dy: i32 = a.get("dy", 2);
    let t: usize = a.get("ticks", 60);
    let lr = a.has("lever-on");
    let path = a.s("design").ok_or_else(|| anyhow::anyhow!("--design"))?;
    let d = mctas::design::Design::load(std::path::Path::new(path))?;
    let door = d.ports["door"];
    let lp = d.ports["lever"];
    let mut g = state::Geo { w, h, door: (dx, dy), levers: vec![], rounds: 2, split: false };
    let at = |x: i32, y: i32| -> [i32; 3] { [x - dx + door[0], y - dy + door[1], door[2]] };
    let mut cells = Vec::new();
    for i in 0..g.n() {
        let (x, y) = g.xy(i);
        cells.push(d.blocks.get(&at(x, y)).cloned().unwrap_or_else(|| "minecraft:air".into()));
    }
    let ls = d.blocks.get(&lp).cloned().unwrap_or_default();
    let pos = (lp[0] - door[0] + dx, lp[1] - door[1] + dy);
    let prop = |k: &str| ls.split(&format!("{k}=")).nth(1).map(|v| v.split([',', ']']).next().unwrap_or("").to_string()).unwrap_or_default();
    let sup = match (prop("face").as_str(), prop("facing").as_str()) {
        ("floor", _) => (pos.0, pos.1 - 1),
        ("ceiling", _) => (pos.0, pos.1 + 1),
        (_, "west") => (pos.0 + 1, pos.1),
        (_, "east") => (pos.0 - 1, pos.1),
        _ => pos,
    };
    let oop = lp[2] != door[2];
    let lev = if oop { state::LeverCand { pos: sup, oop: true, sup } } else { state::LeverCand { pos, oop: false, sup } };
    if let Some(li) = g.idx(pos.0, pos.1).filter(|_| !oop) {
        cells[li] = "LEVER".into();
    }
    g.levers = vec![lev.clone()];
    println!("lever {lev:?}");
    let o = diff::run_case_pol(&g, &cells, &lev, t, lr);
    println!("skipped {:?}\nflags {:?}\nviolations {:?}", o.skipped, o.flags, o.violations);
    match o.mismatch {
        Some((step, dd)) => {
            println!("mismatch at step {step}:");
            for l in dd {
                println!("  {l}");
            }
        }
        None => println!("match ({t} ticks per edge)"),
    }
    Ok(())
}

/// Run mctas commands from a list, `--jobs N` at a time (default 1, in order): `--list FILE`
/// with one `name arg arg ...` per line (whitespace-separated, no quoting: values carry no
/// spaces), each logged to `--logs DIR/name.log`; `#` lines are skipped.
fn run_list(a: &Args) -> Result<()> {
    let list = std::fs::read_to_string(a.s("list").ok_or_else(|| anyhow::anyhow!("--list"))?)?;
    let logs = a.s("logs").unwrap_or("/tmp/boxproof-runs").to_string();
    std::fs::create_dir_all(&logs)?;
    let jobs: usize = a.get("jobs", 1);
    let items: Vec<Vec<String>> = list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(|l| l.split_whitespace().map(String::from).collect()).collect();
    let exe = std::env::current_exe()?;
    let next = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..jobs.max(1) {
            s.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(it) = items.get(k) else { break };
                let log = format!("{logs}/{}.log", it[0]);
                let Ok(f) = std::fs::File::create(&log) else { continue };
                let Ok(f2) = f.try_clone() else { continue };
                let t0 = std::time::Instant::now();
                let st = std::process::Command::new(&exe).args(&it[1..]).stdout(f).stderr(f2).status();
                println!("{} {:.0}s {:?}", it[0], t0.elapsed().as_secs_f64(), st.map(|s| s.code()));
            });
        }
    });
    Ok(())
}

/// Random boxes vs redsim, in parallel.
fn fuzz(a: &Args) -> Result<()> {
    let count: usize = a.get("count", 1000);
    let threads: usize = a.get("threads", 16).min(48);
    let w: i32 = a.get("w", 5);
    let h: i32 = a.get("h", 5);
    let t: usize = a.get("ticks", 30);
    let seed: u64 = a.get("seed", 1);
    let show: usize = a.get("show", 5);
    let next = Arc::new(AtomicUsize::new(0));
    let stats = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
    let shown = Arc::new(Mutex::new(0usize));
    let mut hs = Vec::new();
    for _ in 0..threads {
        let next = next.clone();
        let stats = stats.clone();
        let shown = shown.clone();
        hs.push(std::thread::spawn(move || {
            loop {
                let k = next.fetch_add(1, Ordering::SeqCst);
                if k >= count {
                    break;
                }
                let mut r = diff::Rng(seed.wrapping_mul(1_000_003).wrapping_add(k as u64));
                let (g, cells, lev) = diff::random_box(&mut r, w, h);
                let o = diff::run_case(&g, &cells, &lev, t);
                let key = if let Some(s) = &o.skipped {
                    format!("skipped: {}", s.split(':').next().unwrap_or(s))
                } else if let Some((step, d)) = &o.mismatch {
                    let flagged = !o.flags.is_empty();
                    let mut sh = shown.lock().unwrap();
                    if !flagged && *sh < show {
                        *sh += 1;
                        println!("MISMATCH case {k} (seed {seed}) step {step}: {}", d.iter().take(4).cloned().collect::<Vec<_>>().join(" | "));
                    }
                    if flagged { format!("mismatch, flagged ({})", o.flags.join("; ")) } else { "MISMATCH (unflagged)".into() }
                } else if !o.violations.is_empty() {
                    format!("violation (not a door run): {}", o.violations.join("; "))
                } else if !o.flags.is_empty() {
                    format!("match, flagged ({})", o.flags.join("; "))
                } else if o.moved {
                    "match (pistons moved)".into()
                } else {
                    "match (static)".into()
                };
                *stats.lock().unwrap().entry(key).or_default() += 1;
            }
        }));
    }
    for h in hs {
        let _ = h.join();
    }
    let st = stats.lock().unwrap();
    for (k, v) in st.iter() {
        println!("{v:6}  {k}");
    }
    Ok(())
}

/// One fuzz case in detail.
fn case(a: &Args) -> Result<()> {
    let w: i32 = a.get("w", 5);
    let h: i32 = a.get("h", 5);
    let t: usize = a.get("ticks", 30);
    let seed: u64 = a.get("seed", 1);
    let k: u64 = a.get("k", 0);
    let mut r = diff::Rng(seed.wrapping_mul(1_000_003).wrapping_add(k));
    let (g, cells, lev) = diff::random_box(&mut r, w, h);
    for y in (0..h).rev() {
        let row: Vec<String> = (0..w).map(|x| cells[g.idx(x, y).unwrap()].trim_start_matches("minecraft:").to_string()).collect();
        println!("y={y}: {}", row.join(" | "));
    }
    println!("lever {lev:?}");
    let o = diff::run_case(&g, &cells, &lev, t);
    println!("skipped {:?}\nflags {:?}\nviolations {:?}", o.skipped, o.flags, o.violations);
    if let Some((step, d)) = o.mismatch {
        println!("mismatch at step {step}:");
        for l in d {
            println!("  {l}");
        }
    } else {
        println!("match");
    }
    Ok(())
}
