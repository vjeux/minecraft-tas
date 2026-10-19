//! `mctas door boxproof ...`: whole-box bounded model checking of flat (one z layer) 3x3 doors.
//! See designs/piston-door-3x3/tracks/boxproof/NOTES.md for the model, its coverage and results.

pub mod alg;
pub mod bmc;
pub mod diff;
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
        _ => bail!("usage: door boxproof fuzz|case|bmc [--flags]"),
    }
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
