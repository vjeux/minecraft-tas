//! The 3x3 piston doors: every working design of designs/piston-door-3x3/archive with the facts
//! the boards and the catalog use, read from the caches next to the archive (verdicts, shapes,
//! properties, dynamic checks; no JVM), as [`Build`]s.

use anyhow::Result;
use mctas::design::Design;
use piston_door::leaderboard::{self, Entry, tier_rank};
use piston_door::showcase::{self, BLOCK_BOARDS};
use site_model::{Build, Door};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

pub const ARCHIVE: &str = "designs/piston-door-3x3/archive";

/// The site's knobs for the doors.
pub struct DoorOptions {
    /// Asset names of the GitHub release with the videos (`<stem>.mp4`); empty: no video links.
    pub release_assets: HashSet<String>,
    pub release_tag: String,
}

/// The archive's candidates as `door catalog` takes them: every `.txt` with door and lever
/// ports, files marked invalid left out.
fn candidates(dir: &Path) -> Result<Vec<(String, Design)>> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        if stem.contains("invalid") {
            continue;
        }
        let Ok(d) = Design::load(&p) else { continue };
        if d.ports.contains_key("door") && d.ports.contains_key("lever") {
            out.push((p.display().to_string(), d));
        }
    }
    Ok(out)
}

/// Block kinds of a design (ids without `minecraft:`; a piston head is part of its piston, a wall
/// torch is a torch, every copper bulb a copper bulb), plus its entities' kinds: the clock
/// study's normalization, so one block-kind filter works across doors and clocks.
pub fn kinds_of(d: &Design) -> Vec<String> {
    let norm = |s: &str| -> Option<String> {
        let id = s.split('[').next().unwrap_or(s);
        let id = id.strip_prefix("minecraft:").unwrap_or(id);
        if id == "piston_head" || id == "moving_piston" || id == "air" {
            return None;
        }
        Some(if id == "redstone_wall_torch" {
            "redstone_torch".to_string()
        } else if id.contains("copper_bulb") {
            "copper_bulb".to_string()
        } else {
            id.to_string()
        })
    };
    let set: BTreeSet<String> = d.blocks.values().filter_map(|s| norm(s)).chain(d.entities.iter().filter_map(|e| norm(&e.kind))).collect();
    set.into_iter().collect()
}

/// `YYYY-MM-DD HH:MMZ` from a design's `# archived ...` header line.
pub fn archived_at(text: &str) -> String {
    text.lines()
        .take(4)
        .find_map(|l| l.strip_prefix("# archived ").map(|r| r.split(" from ").next().unwrap_or("").trim().to_string()))
        .unwrap_or_default()
}

/// Every working archived door, sorted by size (volume, blocks). Returns the builds and the
/// stems left out because main's judge has no cached verdict for them yet.
pub fn export(repo: &Path, o: &DoorOptions) -> Result<(Vec<Build>, Vec<String>)> {
    let dir = repo.join(ARCHIVE);
    let (entries, uncached) = leaderboard::cached_entries(candidates(&dir)?, &dir);
    let working: Vec<&Entry> = entries.iter().filter(|e| e.verdict.is_ok()).collect();
    let all: Vec<showcase::Archived> = working.iter().map(|e| showcase::archived_of(e)).collect();
    let records = showcase::progression(&dir);
    let record_how: HashMap<String, String> = records.iter().map(|(_, s, h)| (s.clone(), h.clone())).collect();
    let record_blocks: HashMap<String, usize> = records.iter().map(|(b, s, _)| (s.clone(), *b)).collect();
    let captions = showcase::captions(&dir);
    let builds_v = piston_door::catalog::buildability(&dir);
    let props = piston_door::properties::load_cache(&dir);
    let dynp = piston_door::properties::load_dyn(&dir);
    // board leaders, labeled as the catalog labels them
    let mut leads: HashMap<String, Vec<String>> = HashMap::new();
    for b in showcase::boards(&all, &record_how) {
        leads.entry(b.best.stem.clone()).or_default().push(match b.key {
            Some("sculk-unrotated") => "sculk, not rotation-safe".to_string(),
            Some(k) => k.to_string(),
            None => b.title.to_lowercase(),
        });
    }
    let mut out = Vec::new();
    for (e, a) in working.iter().zip(&all) {
        let h = leaderboard::design_hash(&e.design);
        let get = |k: &str| -> String { dynp.get(&h).and_then(|p| p.get(k)).or_else(|| props.get(&h).and_then(|p| p.get(k))).cloned().unwrap_or_default() };
        let yes = |k: &str| get(k).starts_with("yes");
        let f = &a.facts;
        let tier = get("seamless").split_whitespace().next().unwrap_or("").to_string();
        let rank = tier_rank(&tier);
        let hallway = yes("hallway");
        let eligible = rank > 0 && yes("lever_surface") && yes("lever_support") && yes("repeater_input") && yes("uses") && yes("steps") && hallway;
        let frame = get("frame");
        let mut boards: Vec<String> = BLOCK_BOARDS.iter().filter(|b| (b.3)(f)).map(|b| b.1.to_string()).collect();
        if !f.rotation_safe {
            boards.push("Not rotation-safe".into());
        }
        let my_leads = leads.get(&a.stem).cloned().unwrap_or_default();
        let video = format!("{}.mp4", a.stem);
        let video = o.release_assets.contains(&video).then(|| showcase::asset_url(&o.release_tag, &video));
        // the board leaders' GIFs (door gallery) as interim pictures until the 3D viewer
        let gif = format!("{}.gif", a.stem);
        let picture = o.release_assets.contains(&gif).then(|| showcase::asset_url(&o.release_tag, &gif));
        let buildable = builds_v.get(&a.stem).map(|(v, _)| v.clone());
        let text = std::fs::read_to_string(&a.path).unwrap_or_default();
        let shape = f.shape;
        let mut flags: Vec<&str> = Vec::new();
        let mut flag = |on: bool, k: &'static str| {
            if on {
                flags.push(k);
            }
        };
        flag(f.rotation_safe, "rotation-safe");
        flag(f.slime_free, "slime-free");
        flag(f.flat, "flat");
        flag(shape.is_some_and(|s| s.front <= 1 && s.behind <= 1), "wall");
        flag(f.ground_safe && shape.is_some_and(|s| s.below == 0), "ground");
        flag(rank > 0, "seamless");
        flag(rank >= 3, "seamless-full");
        flag(hallway, "hallway");
        flag(frame == "12/12", "frame-full");
        flag(eligible, "eligible");
        flag(record_blocks.contains_key(&a.stem), "record");
        flag(!my_leads.is_empty(), "leader");
        flag(video.is_some(), "video");
        flag(buildable.as_deref().is_some_and(|b| b.starts_with("yes")), "buildable");
        flag(!e.design.entities.is_empty(), "entities");
        let caption = captions
            .get(&a.stem)
            .map(|c| showcase::sentence(c))
            .or_else(|| showcase::track_of(&a.path).map(|t| format!("From track {t}.")))
            .unwrap_or_default();
        let mut all_props: Vec<(String, String)> = props.get(&h).map(|p| p.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
        if let Some(dp) = dynp.get(&h) {
            for (k, v) in dp {
                if !all_props.iter().any(|(kk, _)| kk == k) {
                    all_props.push((k.clone(), v.clone()));
                }
            }
        }
        all_props.sort();
        let rotation = match &e.rotation {
            Some(Ok(())) => "safe".to_string(),
            Some(Err((deg, _))) => format!("breaks at {deg}°"),
            None => String::new(),
        };
        out.push(Build {
            id: format!("doors/{}", a.stem),
            kind: "door".into(),
            group: e.category.to_string(),
            name: a.stem.clone(),
            page: format!("doors/{}.html", a.stem),
            design: format!("designs/doors/{}.txt", a.stem),
            blocks: e.blocks as u32,
            volume: e.volume.map(|v| v as u32),
            dims: e.volume_dims.clone().filter(|d| !d.is_empty()),
            kinds: kinds_of(&e.design),
            caption,
            flags: flags.into_iter().map(String::from).collect(),
            door: Some(Door {
                open: e.open_ticks,
                close: e.close_ticks,
                rotation,
                seamless: tier,
                frame,
                boards,
                leads: my_leads,
                record: record_blocks.get(&a.stem).map(|b| *b as u32),
                video,
                buildable,
                track: showcase::track_of(&a.path),
                archived: archived_at(&text),
                entities: e.design.entities.len() as u32,
                props: all_props,
                ..Default::default()
            }),
            clock: None,
            circuit: None,
            trace: None,
            picture,
            archived: site_model::normalize_time(&archived_at(&text)),
        });
    }
    out.sort_by(|a, b| (a.volume.unwrap_or(u32::MAX), a.blocks, &a.name).cmp(&(b.volume.unwrap_or(u32::MAX), b.blocks, &b.name)));
    fold_fills(&mut out, &dir);
    let uncached = uncached.iter().map(|s| Path::new(s).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default()).collect();
    Ok((out, uncached))
}

/// The design a fill variant was made from: `note: door fill of <path>: <changes>`.
pub fn fill_parent(text: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix("note: door fill of ").map(|r| r.split(": ").next().unwrap_or(r).trim().to_string()))
}

/// Where an archived design came from: the path in its `# archived ... from <ref>:<path> (repo
/// at ...)` header line, the ref dropped.
pub fn archived_from(text: &str) -> Option<String> {
    let l = text.lines().take(4).find(|l| l.starts_with("# archived "))?;
    let from = l.split(" from ").nth(1)?.split(" (repo at").next()?.trim();
    Some(from.rsplit_once(':').map(|(_, p)| p).unwrap_or(from).to_string())
}

/// Fold the fill variants of each mechanism (vjeux: "record all variants"; the archive holds
/// hundreds): the doors filled from one design, with that design itself when it is archived,
/// form a group; the best of the group (the list is sorted by size, then blocks) stands for it
/// and the others become its variants (hidden by default, like the clock variants).
fn fold_fills(out: &mut [Build], dir: &Path) {
    let text = |b: &Build| std::fs::read_to_string(dir.join(format!("{}.txt", b.name))).unwrap_or_default();
    let mut parent_of: Vec<Option<String>> = Vec::with_capacity(out.len());
    let mut source_of: Vec<Option<String>> = Vec::with_capacity(out.len());
    for b in out.iter() {
        let t = text(b);
        parent_of.push(fill_parent(&t));
        source_of.push(archived_from(&t));
    }
    let parents: BTreeSet<String> = parent_of.iter().flatten().cloned().collect();
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for i in 0..out.len() {
        let key = parent_of[i].clone().or_else(|| source_of[i].clone().filter(|s| parents.contains(s)));
        if let Some(k) = key {
            groups.entry(k).or_default().push(i);
        }
    }
    for (parent, members) in groups {
        for &i in &members {
            if let Some(d) = out[i].door.as_mut() {
                d.fill_of = parent_of[i].clone().unwrap_or_default();
            }
        }
        if members.len() < 2 {
            continue;
        }
        // the list is sorted by (volume, blocks, name): the first member is the best
        let host = members[0];
        let host_id = out[host].id.clone();
        let ids: Vec<String> = members[1..].iter().map(|&i| out[i].id.clone()).collect();
        for &i in &members[1..] {
            if let Some(d) = out[i].door.as_mut() {
                d.host = Some(host_id.clone());
            }
            out[i].flags.push("variant".into());
        }
        if let Some(d) = out[host].door.as_mut() {
            d.variants = ids;
        }
        let _ = parent;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_normalized() {
        let d = Design::parse("name: t\npalette:\n  P minecraft:sticky_piston[facing=up,extended=true]\n  H minecraft:piston_head[facing=up,type=sticky]\n  T minecraft:redstone_wall_torch[facing=east]\n  C minecraft:waxed_copper_bulb[lit=false]\nlayer 0:\n  PTC\nlayer 1:\n  H\n").unwrap();
        assert_eq!(kinds_of(&d), vec!["copper_bulb", "redstone_torch", "sticky_piston"]);
    }

    #[test]
    fn archived_header() {
        assert_eq!(archived_at("# verified WORKS\n# archived 2026-10-19 10:35Z from /tmp/x.txt (repo at ccbb856)\nname: x\n"), "2026-10-19 10:35Z");
        assert_eq!(archived_at("name: x\n"), "");
    }

    #[test]
    fn fill_headers() {
        let t = "# verified WORKS by door leaderboard: 37 blocks, category sculk\n# archived 2026-10-19 20:23Z from origin/explore/c77:designs/piston-door-3x3/tracks/c77/runs/x/e78st-fill-37b-1.txt (repo at 7c5e37c16)\nname: e78st-fill\nnote: door fill of designs/piston-door-3x3/tracks/c77/e78st-56b-38-1s.txt: (0,-3) calibrated_sculk_sensor[facing=west] (-2,-1) air\n";
        assert_eq!(fill_parent(t).as_deref(), Some("designs/piston-door-3x3/tracks/c77/e78st-56b-38-1s.txt"));
        assert_eq!(archived_from(t).as_deref(), Some("designs/piston-door-3x3/tracks/c77/runs/x/e78st-fill-37b-1.txt"));
        assert_eq!(archived_from("# archived 2026-10-19 10:35Z from /tmp/x.txt (repo at ccbb856)\n").as_deref(), Some("/tmp/x.txt"));
        assert_eq!(fill_parent("name: x\n"), None);
    }
}
