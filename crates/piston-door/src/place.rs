//! `mctas door place <base> --groups <file>`: try every combination of alternative patches.
//!
//! When a mechanism is known to work with ideal inputs but its wiring is a 3D puzzle (which
//! cell holds the lever, which way each observer faces, where the delay repeater sits), list
//! the candidate placements of each part as a *group* of alternatives and let the simulator
//! judge every combination: each combination is applied to the base design (patch lines, see
//! `door patch`), screened at one position for one cycle, and the screen survivors are judged
//! at the default positions (fewest blocks first). Working designs are written next to `--out`.
//!
//! Groups file:
//! ```text
//! group lever            # a part with several placements
//! alt a                  # one placement: patch lines until the next alt/group
//! set -1 4 1 minecraft:lever[face=wall,facing=north,powered=false]
//! port lever -1 4 1
//! alt b
//! ...
//! alt none               # an empty alternative (the part is optional)
//! ```

use crate::{VerifyOptions, block_count, verify_in};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::Pool;
use std::path::Path;
use std::sync::Mutex;

/// One group: a name and its alternatives (label, patch script).
pub struct Group {
    pub name: String,
    pub alts: Vec<(String, String)>,
}

pub fn parse_groups(text: &str) -> Result<Vec<Group>> {
    let mut groups: Vec<Group> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix("group ") {
            groups.push(Group { name: name.trim().to_string(), alts: Vec::new() });
        } else if let Some(label) = line.strip_prefix("alt") {
            let g = groups.last_mut().with_context(|| format!("line {}: alt before any group", i + 1))?;
            g.alts.push((label.trim().to_string(), String::new()));
        } else {
            let g = groups.last_mut().with_context(|| format!("line {}: patch line before any group", i + 1))?;
            let alt = g.alts.last_mut().with_context(|| format!("line {}: patch line before any alt", i + 1))?;
            alt.1.push_str(line);
            alt.1.push('\n');
        }
    }
    for g in &groups {
        if g.alts.is_empty() {
            bail!("group {} has no alternatives", g.name);
        }
    }
    Ok(groups)
}

/// Every combination (one alternative index per group), in lexicographic order.
fn combinations(groups: &[Group]) -> Vec<Vec<usize>> {
    let mut out = vec![Vec::new()];
    for g in groups {
        let mut next = Vec::new();
        for c in &out {
            for a in 0..g.alts.len() {
                let mut c2 = c.clone();
                c2.push(a);
                next.push(c2);
            }
        }
        out = next;
    }
    out
}

pub struct PlaceOptions {
    pub screen: VerifyOptions,
    pub judge: VerifyOptions,
    /// Stop after this many working designs (0 = judge every screen survivor).
    pub want: usize,
}

/// Run the search; returns (blocks, labels, design) of every judged-working combination.
pub fn place(
    pool: &Pool,
    base: &Design,
    groups: &[Group],
    opts: &PlaceOptions,
    out: &Path,
    log: &dyn Fn(&str),
) -> Result<Vec<(usize, String, Design)>> {
    let combos = combinations(groups);
    log(&format!("{} combinations of {} groups", combos.len(), groups.len()));
    let build = |c: &[usize]| -> Result<(String, Design)> {
        let mut d = base.clone();
        let mut labels = Vec::new();
        for (g, &a) in groups.iter().zip(c) {
            let (label, script) = &g.alts[a];
            d = crate::patch::apply(&d, script).with_context(|| format!("group {} alt {label}", g.name))?;
            labels.push(format!("{}={label}", g.name));
        }
        Ok((labels.join(" "), d))
    };
    let sandboxes: Vec<Mutex<Option<u32>>> = (0..pool.len()).map(|_| Mutex::new(None)).collect();
    let sb_for = |w: &mut mctas::sim::Worker, wi: usize| -> Result<u32> {
        let mut s = sandboxes[wi].lock().unwrap();
        if let Some(sb) = *s {
            return Ok(sb);
        }
        let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        *s = Some(sb);
        Ok(sb)
    };
    let screened = pool.map(combos.clone(), |w, wi, c| {
        let (labels, d) = build(&c)?;
        let sb = sb_for(w, wi)?;
        Ok(verify_in(w, sb, &d, &opts.screen).is_ok().then_some((block_count(&d), labels, d)))
    });
    let mut pass: Vec<(usize, String, Design)> = Vec::new();
    let mut errors = 0;
    for r in screened {
        match r {
            Ok(Some(x)) => pass.push(x),
            Ok(None) => {}
            Err(e) => {
                if errors == 0 {
                    log(&format!("  error: {e:#}"));
                }
                errors += 1;
            }
        }
    }
    pass.sort_by_key(|x| x.0);
    log(&format!("{} passed the screen ({errors} errors)", pass.len()));
    let mut found = Vec::new();
    for part in pass.chunks(pool.len().max(1)) {
        let res = pool.map(part.to_vec(), |w, wi, (n, labels, d)| {
            let sb = sb_for(w, wi)?;
            Ok(verify_in(w, sb, &d, &opts.judge).map(|r| (n, labels, d, r)).map_err(|e| format!("{e:#}")))
        });
        for r in res {
            match r {
                Ok(Ok((n, labels, d, rep))) => {
                    let open = rep.cycles.iter().map(|c| c.open_ticks).max().unwrap_or(0);
                    let close = rep.cycles.iter().map(|c| c.close_ticks).max().unwrap_or(0);
                    let path = out.with_file_name(format!(
                        "{}-{}-{n}b.txt",
                        out.file_stem().and_then(|s| s.to_str()).unwrap_or("place"),
                        found.len()
                    ));
                    d.save(&path)?;
                    log(&format!("WORKS {n} blocks (open {open}, close {close}): {labels} -> {}", path.display()));
                    found.push((n, labels, d));
                }
                Ok(Err(e)) => log(&format!("  judge failed: {}: {e}", part.iter().find(|_| true).map(|x| x.1.as_str()).unwrap_or(""))),
                Err(e) => log(&format!("  judge error: {e:#}")),
            }
        }
        if opts.want > 0 && found.len() >= opts.want {
            break;
        }
    }
    found.sort_by_key(|x| x.0);
    Ok(found)
}
