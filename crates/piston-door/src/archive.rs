//! Keep every working design: `door verify --archive` copies a verified design into
//! `archive/<name>-<blocks>b.txt` next to it, with a header saying when and how it verified.

use std::path::{Path, PathBuf};

use anyhow::Result;
use mctas::design::Design;

/// Archive `d` (loaded from `source`) unless an identical design is already there. Returns
/// the new file, or None when it was already archived.
/// The archive folder for `source` (`into` when given; `designs/piston-door-3x3/archive` for a
/// built-in `@name`; else `archive/` next to the design file).
pub fn dir_for(source: &Path, into: Option<&Path>) -> PathBuf {
    let dir = match source.to_str() {
        _ if into.is_some() => into.unwrap().to_path_buf(),
        Some(s) if s.starts_with('@') => PathBuf::from("designs/piston-door-3x3/archive"),
        _ => source.parent().unwrap_or(Path::new(".")).join("archive"),
    };
    if dir.ends_with("archive/archive") { dir.parent().unwrap().to_path_buf() } else { dir }
}

pub fn save(source: &Path, d: &Design, blocks: usize, note: &str, into: Option<&Path>) -> Result<Option<PathBuf>> {
    // vjeux 2026-10-21: obsidian only where needed: an obsidian block the record judge's redsim
    // screen accepts as stone (same ticks) is refused here (`door material` writes the fix)
    if d.blocks.values().any(|s| s == "minecraft:obsidian") {
        let mut offsets = crate::VerifyOptions::default().offsets;
        offsets.extend(crate::extra_offsets(28));
        let m = crate::material::obsidian_pass(d, &offsets, 3, 400, true);
        if !m.replaced.is_empty() {
            anyhow::bail!(
                "not archived: obsidian at {} can be stone (the door passes with the same ticks on the redsim screen): `mctas door material {} --out DIR` writes it (vjeux 2026-10-21: obsidian only where needed)",
                m.replaced.iter().map(|p| format!("{p:?}")).collect::<Vec<_>>().join(", "),
                source.display()
            );
        }
    }
    let dir = dir_for(source, into);
    std::fs::create_dir_all(&dir)?;
    let body = d.to_text();
    for e in std::fs::read_dir(&dir)? {
        let p = e?.path();
        if p.canonicalize().ok() == source.canonicalize().ok() {
            continue;
        }
        if let Ok(old) = Design::load(&p) {
            if old.blocks == d.blocks && old.entities == d.entities {
                return Ok(None);
            }
        }
    }
    let base = d.name.replace(' ', "-");
    let mut path = dir.join(format!("{base}-{blocks}b.txt"));
    let mut i = 2;
    while path.exists() {
        path = dir.join(format!("{base}-{blocks}b-{i}.txt"));
        i += 1;
    }
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    let date = std::process::Command::new("date").arg("-u").arg("+%Y-%m-%d %H:%MZ").output().ok().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let text = format!("# {note}\n# archived {date} from {} (repo at {commit})\n{body}", source.display());
    std::fs::write(&path, text)?;
    Ok(Some(path))
}
