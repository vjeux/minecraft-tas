//! `mctas door provenance`: which archived doors are OURS and which derive from community designs
//! (vjeux, 2026-10-20: "remove the community door ... only things we generated on the website").
//! Writes `designs/piston-door-3x3/archive/provenance.tsv` (name, origin = ours |
//! community-derived, reason).
//!
//! Community roots: the community 7x8 (SacredRedstone's, reproduced in tracks/c70b/community-7x8,
//! c77's fix and copies) and the community 7x9 (c79: tracks/c79/c79-closed). A design is
//! community-derived when
//! (a) its name or its archive provenance line (`# archived ... from <path>`) names a root or a
//!     community-derived track / run: the token `c79`, `community`, `comm-` / `cst-` / `c47-`
//!     (c77's community runs), `closed-b` (fills of c77's community closed-b), the tracks c88 /
//!     s79 / m79s / m10top (c79's mechanism re-laid / mutated), `s26` / `s27` (c88's doors); or
//! (b) its MECHANISM (the pistons that move door blocks or carry such pistons, from the redsim
//!     cycle of `door audit`; wiring ignored; left-right and front-back mirror images merged) is
//!     within `--near` pieces (symmetric difference) of a root's, or its door blocks take exactly a
//!     root's opening paths (the community 7x9's moves re-laid in an 8x8 box); or
//! (c) (lineage; `--no-propagate` turns it off) its mechanism is within `--near` pieces of, or takes the paths of,
//!     a design already found community-derived (repeated to a fixpoint).
//! Designs whose mechanism cannot be extracted are judged by (a) alone (listed in the summary).

use crate::audit;
use crate::cosearch as cs;
use anyhow::{Context, Result};
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub type Sig = Vec<[i32; 8]>;

/// The four mirror images (x -> 2 - x, z -> -z) of a signature, each sorted.
fn images(s: &Sig) -> Vec<Sig> {
    let mut out = Vec::new();
    for mx in [false, true] {
        for mz in [false, true] {
            let mut v: Sig = s
                .iter()
                .map(|e| {
                    let mut e = *e;
                    if mx {
                        e[0] = 2 - e[0];
                        e[3] = -e[3];
                    }
                    if mz {
                        e[2] = -e[2];
                        e[5] = -e[5];
                    }
                    e
                })
                .collect();
            v.sort();
            out.push(v);
        }
    }
    out
}

/// Pieces in one mechanism and not the other (the best mirror image).
pub fn distance(a: &Sig, b: &Sig) -> usize {
    let sb: HashSet<&[i32; 8]> = b.iter().collect();
    images(a)
        .iter()
        .map(|va| {
            let sa: HashSet<&[i32; 8]> = va.iter().collect();
            sa.symmetric_difference(&sb).count()
        })
        .min()
        .unwrap_or(usize::MAX)
}

/// Rule (a): the root / community lineage a name or path names, if any.
pub fn text_root(s: &str) -> Option<&'static str> {
    let l = s.to_lowercase();
    if l.contains("community") {
        return Some("community");
    }
    // a token: preceded by start or a non-alphanumeric, followed by a non-digit (`word`: by a
    // non-alphanumeric: `comm-k2` but not `commit`)
    let token = |t: &str, word: bool| -> bool {
        let b = l.as_bytes();
        let tb = t.as_bytes();
        (0..b.len().saturating_sub(tb.len() - 1)).any(|i| {
            let end = i + tb.len();
            &b[i..end] == tb
                && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
                && (end >= b.len() || if word { !b[end].is_ascii_alphanumeric() } else { !b[end].is_ascii_digit() })
        })
    };
    for (t, word, why) in [
        ("c79", false, "c79"),
        ("c88", false, "c88 (c79's mechanism in 8x8)"),
        ("m79s", false, "m79s (c79 variants)"),
        ("m10top", false, "m10top (c79 with BR right, TR up)"),
        ("s79", false, "s79 (7x8 from c79's mechanism)"),
        ("c78z", false, "c78z (s79's c79 variant)"),
        ("s26", false, "s26 (c88)"),
        ("s27", false, "s27 (c88)"),
        ("closed-b", false, "closed-b (fills of c77's community closed-b)"),
        ("comm", true, "comm (c77's community runs)"),
        ("cst", true, "cst (c77's community stone copy)"),
        ("c47", true, "c47 (c77's community 47b copy)"),
    ] {
        if token(t, word) {
            return Some(why);
        }
    }
    None
}

/// The `# archived ... from <path>` line's path.
fn archived_from(text: &str) -> String {
    text.lines()
        .filter(|l| l.starts_with('#') && l.contains("archived"))
        .find_map(|l| l.split(" from ").nth(1).map(|r| r.split_whitespace().next().unwrap_or("").to_string()))
        .unwrap_or_default()
}

/// A design's mechanism: (piston signature, door-block path signature), or why there is none.
fn signature_of(path: &Path) -> std::result::Result<(Sig, PSig), String> {
    let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let (_, door) = cs::load_door(path).map_err(|e| format!("does not load in redsim: {e:#}"))?;
    let m = audit::mechanism(&name, &door)?;
    let s = m.signature();
    if s.is_empty() {
        return Err("no mechanism piston found".into());
    }
    Ok((s, m.path_signature()))
}

/// Door-block opening paths (see `Mechanism::path_signature`).
pub type PSig = Vec<Vec<[i32; 3]>>;

/// Same choreography (mirror images merged: x -> 2 - x, z -> -z).
pub fn same_paths(a: &PSig, b: &PSig) -> bool {
    if a.is_empty() || a.iter().all(|p| p.len() <= 1) {
        return false;
    }
    for mx in [false, true] {
        for mz in [false, true] {
            let mut v: PSig = a.iter().map(|p| p.iter().map(|c| [if mx { 2 - c[0] } else { c[0] }, c[1], if mz { -c[2] } else { c[2] }]).collect()).collect();
            v.sort();
            if v == *b {
                return true;
            }
        }
    }
    false
}

/// Family prefix of an archive name (for the summary): up to the first '-' (letters and digits).
fn family(name: &str) -> String {
    name.split('-').next().unwrap_or(name).to_string()
}

pub fn run(archive: &Path, roots: &[PathBuf], out: &Path, threads: usize, near: usize, propagate: bool) -> Result<()> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(archive)
        .with_context(|| format!("{}", archive.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "txt"))
        .collect();
    files.sort();
    let stem = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    // the roots' mechanisms: the given root files + every archived copy whose name is a root
    let mut root_sigs: Vec<(String, Sig, PSig)> = Vec::new();
    for r in roots {
        match signature_of(r) {
            Ok((s, p)) => root_sigs.push((stem(r), s, p)),
            Err(e) => eprintln!("[provenance] root {}: {e}", r.display()),
        }
    }
    eprintln!("[provenance] {} archived designs, {} root mechanisms; extracting mechanisms...", files.len(), root_sigs.len());
    let sigs = cs::par_map(&files, threads, |f| signature_of(f));
    for (f, s) in files.iter().zip(&sigs) {
        let name = stem(f);
        if let Ok((s, p)) = s
            && (name.starts_with("community") || name.starts_with("c79-community"))
            && !root_sigs.iter().any(|(_, r, _)| distance(s, r) == 0)
        {
            root_sigs.push((name, s.clone(), p.clone()));
        }
    }
    let names: Vec<String> = files.iter().map(|f| stem(f)).collect();
    // (a) names / provenance, (b) roots' mechanisms and paths
    let mut verdict: Vec<Option<String>> = vec![None; files.len()];
    let mut nearest: Vec<Option<(usize, String)>> = vec![None; files.len()];
    for (i, f) in files.iter().enumerate() {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        let from = archived_from(&text);
        if let Some(t) = text_root(&names[i]).or_else(|| text_root(&from)) {
            verdict[i] = Some(format!("provenance names {t}: {} (archived from {})", names[i], if from.is_empty() { "-" } else { &from }));
            continue;
        }
        let Ok((s, p)) = &sigs[i] else { continue };
        nearest[i] = root_sigs.iter().map(|(rn, r, _)| (distance(s, r), rn.clone())).min();
        if let Some((d, rn)) = nearest[i].as_ref().filter(|(d, _)| *d <= near) {
            verdict[i] = Some(if *d == 0 { format!("mechanism = {rn}'s") } else { format!("mechanism within {d} pieces of {rn}'s") });
        } else if let Some((rn, _, _)) = root_sigs.iter().find(|(_, _, rp)| same_paths(p, rp)) {
            verdict[i] = Some(format!("door-block paths = {rn}'s (its moves, pistons re-laid)"));
        }
    }
    // (c) lineage: close to a design already community-derived, to a fixpoint
    let mut propagated = 0;
    if propagate {
        loop {
            let comm: Vec<usize> = (0..files.len()).filter(|&i| verdict[i].is_some() && sigs[i].is_ok()).collect();
            let mut added = Vec::new();
            for i in 0..files.len() {
                if verdict[i].is_some() {
                    continue;
                }
                let Ok((s, p)) = &sigs[i] else { continue };
                if let Some(&j) = comm.iter().find(|&&j| {
                    let (sj, pj) = sigs[j].as_ref().unwrap();
                    distance(s, sj) <= near || same_paths(p, pj)
                }) {
                    added.push((i, format!("lineage: mechanism within {near} pieces (or the paths) of {} (community-derived)", names[j])));
                }
            }
            if added.is_empty() {
                break;
            }
            propagated += added.len();
            for (i, why) in added {
                verdict[i] = Some(why);
            }
        }
    }
    let mut tsv = String::from("name\torigin\treason\n");
    let mut fam: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut unchecked = Vec::new();
    let (mut n_ours, mut n_comm) = (0, 0);
    for i in 0..files.len() {
        let name = &names[i];
        let (origin, reason) = match &verdict[i] {
            Some(r) => ("community-derived", r.clone()),
            None => {
                let why = match (&nearest[i], &sigs[i]) {
                    (Some((d, rn)), Ok((s, _))) => format!("own mechanism ({} pieces; nearest community mechanism {rn} at distance {d})", s.len()),
                    (_, Err(e)) => {
                        unchecked.push(name.clone());
                        format!("no mechanism extracted ({}); provenance names no root", e.lines().next().unwrap_or(""))
                    }
                    _ => "own mechanism".into(),
                };
                ("ours", why)
            }
        };
        let e = fam.entry(family(name)).or_insert((0, 0));
        if origin == "ours" {
            n_ours += 1;
            e.0 += 1;
        } else {
            n_comm += 1;
            e.1 += 1;
        }
        let _ = writeln!(tsv, "{name}\t{origin}\t{}", reason.replace('\t', " "));
    }
    std::fs::write(out, &tsv).with_context(|| format!("{}", out.display()))?;
    println!(
        "door provenance: {} designs: {n_ours} ours, {n_comm} community-derived (near = {near}{}); wrote {}",
        files.len(),
        if propagate { format!(", {propagated} by lineage") } else { String::new() },
        out.display()
    );
    println!("roots: {}", root_sigs.iter().map(|(n, s, _)| format!("{n} ({} pieces)", s.len())).collect::<Vec<_>>().join(", "));
    println!("| family | ours | community-derived |\n|---|---|---|");
    for (k, (o, c)) in &fam {
        println!("| {k} | {o} | {c} |");
    }
    if !unchecked.is_empty() {
        println!("no mechanism extracted (judged by name / provenance only): {}", unchecked.join(" "));
    }
    Ok(())
}
