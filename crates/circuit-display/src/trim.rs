//! `mctas circuit display trim <design> --out F` (clk-obs, 2026-10-21): the stones a design does not
//! need. Every stone is tried alone (in parallel): the design without it must pass the redsim judge
//! (`circuit::verify::sim`: the kind's stimulus, the supports, the item intrusion run) with no
//! glitches and a box no larger; then the removable ones are taken out one at a time, each
//! re-judged with the earlier ones gone (two stones can each be spare only while the other stays).
//! The game judge (`circuit verify`) has the last word.

use anyhow::{Context, Result, anyhow};
use mctas::design::Design;
use std::path::Path;

/// Whether a design passes the redsim judge with no glitches, and its box volume.
fn judge(d: &Design, tmp: &Path) -> Option<i64> {
    std::fs::write(tmp, d.to_text()).ok()?;
    let c = circuit::Circuit::load(tmp).ok()?;
    let r = circuit::verify::sim(&c, &circuit::verify::Options::default()).ok()?;
    let out = r.outcome?;
    if !out.pass || out.metrics.iter().any(|(k, _)| k == "glitches") {
        return None;
    }
    let (lo, hi) = d.bounds()?;
    Some((0..3).map(|i| (hi[i] - lo[i] + 1) as i64).product())
}

pub fn run(design: &Path, out: &Path, threads: usize) -> Result<()> {
    let text = std::fs::read_to_string(design).with_context(|| format!("reading {}", design.display()))?;
    let base = Design::parse(&text)?;
    let dir = std::env::temp_dir().join(format!("display-trim-{}", std::process::id()));
    std::fs::create_dir_all(&dir)?;
    let vol = judge(&base, &dir.join("base.txt")).ok_or_else(|| anyhow!("{} does not pass the redsim judge glitch-free", design.display()))?;
    let stones: Vec<[i32; 3]> = base.blocks.iter().filter(|(_, s)| s.as_str() == "minecraft:stone").map(|(p, _)| *p).collect();
    println!("{} stones; base volume {vol}", stones.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let spare = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for t in 0..threads.max(1) {
            let (next, spare, base, dir, stones) = (&next, &spare, &base, &dir, &stones);
            s.spawn(move || {
                let tmp = dir.join(format!("t{t}.txt"));
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(p) = stones.get(i) else { break };
                    let mut d = base.clone();
                    d.blocks.remove(p);
                    if judge(&d, &tmp).is_some_and(|v| v <= vol) {
                        spare.lock().unwrap().push(*p);
                    }
                }
            });
        }
    });
    let mut spare = spare.into_inner().unwrap();
    spare.sort();
    println!("{} spare alone: {spare:?}", spare.len());
    // one at a time, each re-judged with the earlier ones gone
    let mut d = base.clone();
    let mut gone = Vec::new();
    for p in spare {
        let mut e = d.clone();
        e.blocks.remove(&p);
        if judge(&e, &dir.join("seq.txt")).is_some_and(|v| v <= vol) {
            d = e;
            gone.push(p);
        }
    }
    let head: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    std::fs::write(out, format!("{head}# trimmed (clk-obs `display trim`): {} stones the redsim judge does not need: {gone:?}\n{}", gone.len(), d.to_text()))?;
    let _ = std::fs::remove_dir_all(&dir);
    println!("{}: {} stones removed, {} blocks", out.display(), gone.len(), d.blocks.len());
    Ok(())
}
