//! Odd periods by kicks (generic over loop families): every loading of every candidate loop with
//! an odd half loop delay is pre-simulated; the loadings running ONE pulse around the loop
//! (period = loop delay, one edge) are kicked at odd ticks (a temporary block starting a second
//! pulse in the other parity class); the smallest design per odd period wins.

use crate::kick;
use crate::sweep::{self, Found};
use std::collections::{BTreeMap, HashMap};

/// One loaded loop: `base` groups the loadings of one structure, `half` = the loop delay in
/// redstone ticks (the odd period sought, in gt).
pub struct Cand {
    pub name: String,
    pub base: String,
    pub text: String,
    pub half: u32,
    pub caption: String,
}

pub struct Hit {
    pub found: Found,
    pub how: String,
    pub caption: String,
}

pub fn search(cands: Vec<Cand>, max_len: u32, horizon: u32, threads: usize, per_base: usize) -> BTreeMap<u32, Hit> {
    let by_name: HashMap<String, &Cand> = cands.iter().map(|c| (c.name.clone(), c)).collect();
    let pre: Vec<(String, String)> = cands.iter().map(|c| (c.name.clone(), c.text.clone())).collect();
    eprintln!("{} loadings", pre.len());
    let found = sweep::run_all(pre, horizon, threads);
    let mut groups: BTreeMap<String, Vec<(u32, String)>> = BTreeMap::new();
    for f in &found {
        let Some(c) = by_name.get(&f.label) else { continue };
        if f.period != 2 * c.half || f.edges != 1 {
            continue;
        }
        let pulse = f.on.min(f.period - f.on);
        groups.entry(c.base.clone()).or_default().push((pulse, f.label.clone()));
    }
    let mut keep: Vec<&Cand> = Vec::new();
    for (_, mut v) in groups {
        v.sort();
        v.dedup_by(|a, b| a.0 == b.0);
        for (_, name) in v.into_iter().take(per_base) {
            keep.push(by_name[&name]);
        }
    }
    eprintln!("{} one-pulse loadings to kick", keep.len());
    let mut best: BTreeMap<u32, Hit> = BTreeMap::new();
    for c in keep {
        let Ok(d) = mctas::design::Design::parse(&c.text) else { continue };
        let cells = kick::kick_cells(&d);
        let lens: Vec<u32> = (1..=max_len).collect();
        let ats: Vec<u32> = (1..=(2 * c.half + 3)).filter(|a| a % 2 == 1).collect();
        for (cell, st, l, a, f) in kick::search(&d, &cells, &lens, &ats, horizon, threads) {
            if f.period % 2 == 0 {
                continue;
            }
            if best.get(&f.period).is_none_or(|o| (f.blocks, f.volume) < (o.found.blocks, o.found.volume)) {
                let how = format!("kick {} at {:?} for {l} gt at tick {a}", clock::short(&st), cell);
                best.insert(f.period, Hit { found: f, how, caption: c.caption.clone() });
            }
        }
    }
    best
}
