//! The local census (`mctas circuit local-census`, `mctas circuit sorter-census`): every design
//! with up to R of the base design's blocks removed and fewer palette blocks added in the air cells
//! of a box, so every candidate is smaller than the base, each screened on redsim by a judge the
//! caller gives (a kind's trace checker with the base's main value, the sorter runs). A shrink in
//! one step across a neighbourhood the greedy game shrink (`mctas circuit shrink`) cannot cross:
//! it removes blocks only, one at a time.

use crate::verify::Options;
use crate::{Circuit, Pos};
use anyhow::{Result, anyhow};
use mctas::design::Design;

/// The census: `keep` (substrings of block states: `hopper`, `comparator`) never removed; ports
/// stay air. Returns the passing designs (block count, the design), smallest first.
pub fn census(base: &Circuit, remove: usize, cells: (Pos, Pos), palette: &[String], keep: &[String], threads: usize, pass: &(dyn Fn(&Circuit) -> bool + Sync)) -> Vec<(usize, Design)> {
    let d0 = base.clock.design.clone();
    let core = |s: &str| keep.iter().any(|k| s.contains(k.as_str()));
    let removable: Vec<Pos> = d0.blocks.iter().filter(|(_, s)| !core(s)).map(|(p, _)| *p).collect();
    let ports: Vec<Pos> = d0.ports.values().copied().collect();
    let mut free: Vec<Pos> = Vec::new();
    for x in cells.0[0]..=cells.1[0] {
        for y in cells.0[1]..=cells.1[1] {
            for z in cells.0[2]..=cells.1[2] {
                let p = [x, y, z];
                if !ports.contains(&p) && !d0.blocks.get(&p).is_some_and(|s| core(s)) {
                    free.push(p);
                }
            }
        }
    }
    // every (removed set, added cells) with |added| < |removed|; the palette choices per job
    let mut jobs: Vec<(Vec<Pos>, Vec<Pos>)> = Vec::new();
    for r in 1..=remove.min(removable.len()) {
        for rm in combos(&removable, r) {
            let open: Vec<Pos> = free.iter().copied().filter(|p| rm.contains(p) || !d0.blocks.contains_key(p)).collect();
            for n in 0..r {
                for add in combos(&open, n) {
                    jobs.push((rm.clone(), add));
                }
            }
        }
    }
    let total: usize = jobs.iter().map(|(_, add)| palette.len().pow(add.len() as u32)).sum();
    eprintln!("local census: {} removable, {} free cells, {} palette states: {} placements, {total} candidates", removable.len(), free.len(), palette.len(), jobs.len());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let found = std::sync::Mutex::new(Vec::new());
    let screened = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some((rm, add)) = jobs.get(i) else { break };
                    for code in 0..palette.len().pow(add.len() as u32) {
                        let mut d = d0.clone();
                        for p in rm {
                            d.blocks.remove(p);
                            d.block_entities.remove(p);
                        }
                        let mut c = code;
                        for p in add {
                            d.blocks.insert(*p, palette[c % palette.len()].clone());
                            c /= palette.len();
                        }
                        let Ok(cand) = Circuit::from_design(d.clone()) else { continue };
                        screened.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if pass(&cand) {
                            let b = cand.blocks();
                            let added: Vec<String> = add.iter().map(|p| format!("{p:?} {}", clock::short(&d.blocks[p]))).collect();
                            eprintln!("  found: {b} blocks (removed {rm:?}, added {added:?})");
                            found.lock().unwrap().push((b, d));
                        }
                    }
                }
            });
        }
    });
    eprintln!("local census: {} candidates screened", screened.into_inner());
    let mut v = found.into_inner().unwrap();
    v.sort_by_key(|(b, _)| *b);
    v
}

/// The judge for a kind the trace checker judges: a candidate passes the redsim screen with the
/// base's main value (its first metric: `L 16 gt`, `N = 4 gt`, `function xor`...) and the base's
/// values of the metrics named in `same` (`delay` keeps an instant design instant). Err: the base
/// itself does not pass.
pub fn trace_judge(base: &Circuit, same: &[String]) -> Result<impl Fn(&Circuit) -> bool + Sync> {
    trace_judge_value(base, same, None)
}

/// [`trace_judge`] with the main value replaced (`value`: the first metric's value as the checker
/// prints it, e.g. `3 gt`): a census from a base of one value that looks for another (a 5 gt delay
/// seeding a search for a 3 gt one).
pub fn trace_judge_value(base: &Circuit, same: &[String], value: Option<&str>) -> Result<impl Fn(&Circuit) -> bool + Sync> {
    let o = Options::default();
    let r = crate::verify::sim(base, &o)?;
    let out = r.outcome.ok_or_else(|| anyhow!("{}: no checker verdict on redsim", base.name()))?;
    if !out.pass {
        return Err(anyhow!("{}: fails the redsim screen ({}), nothing to shrink", base.name(), out.why));
    }
    let pick = |m: &[(String, String)]| -> Vec<Option<(String, String)>> {
        let mut v = vec![m.first().cloned()];
        v.extend(same.iter().map(|k| m.iter().find(|(n, _)| n == k).cloned()));
        v
    };
    let mut want = pick(&out.metrics);
    if let (Some(v), Some(Some((_, val)))) = (value, want.first_mut()) {
        *val = v.to_string();
    }
    // the candidates' test spacing: twice the base's (sizing every candidate costs more than its
    // run; one that needs more is judged with its own spacing by the game judge later)
    let gap = crate::verify::sizing(base, o.margin).map(|g| 2 * g);
    Ok(move |c: &Circuit| crate::verify::sim(c, &Options { gap, ..Options::default() }).ok().and_then(|r| r.outcome).is_some_and(|o| o.pass && pick(&o.metrics) == want))
}

/// The k-subsets of `v`, in order.
pub fn combos(v: &[Pos], k: usize) -> Vec<Vec<Pos>> {
    let mut out = Vec::new();
    let mut idx: Vec<usize> = (0..k).collect();
    if k > v.len() {
        return out;
    }
    loop {
        out.push(idx.iter().map(|i| v[*i]).collect());
        let mut i = k;
        while i > 0 && idx[i - 1] == v.len() - k + i - 1 {
            i -= 1;
        }
        if i == 0 {
            return out;
        }
        idx[i - 1] += 1;
        for j in i..k {
            idx[j] = idx[j - 1] + 1;
        }
    }
}
