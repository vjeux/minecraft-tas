//! `mctas door world`: one Minecraft world (a save folder for the real game) holding every
//! working design of the archive, in rows by board, smallest first, each with its block count
//! written above it in red concrete and its name on a floating label, so they can be tried by
//! hand in the real client.
//!
//! The world is built by the official server jar (as `door vanilla-check` runs it): an empty
//! void world (nothing under or around the machines, like the judge's world), every design
//! pasted with `/setblock ... strict` in its settled rest state (the simulator's judge sequence:
//! raw placement, wire shapes, a self update of every block, settle), so nothing fires while it
//! is built. Creative mode; a glass platform at the world spawn, away from the machines.

use crate::showcase::Archived;
use crate::DoorSpec;
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{Pool, Pos, WorkerConfig};
use mctas::vanilla::{VanillaOptions, VanillaServer};
use std::path::{Path, PathBuf};

pub struct WorldOptions {
    pub archive: PathBuf,
    /// Where the world folder is written (`<out>/<name>`), plus `<out>/<name>.tar.gz`.
    pub out: PathBuf,
    /// The save's folder name (what the game lists).
    pub name: String,
    pub workers: usize,
    /// After building, screenshot the spawn and the first `shots` designs of each row with the
    /// real client joined to the saved world (0 = none).
    pub shots: usize,
    /// Read every design back from the saved region files and fail on any difference.
    pub check: bool,
    /// Take the verdicts from this verdict-cache file (a `verdicts-v*.tsv` of the archive)
    /// instead of judging: only designs marked `ok` there are built (no rotation info).
    pub verdicts: Option<PathBuf>,
}

/// The working designs of `archive` according to a verdict-cache file (hash, ok|broken, open,
/// close, ...), without simulating anything.
fn archived_from_cache(archive: &Path, verdicts: &Path) -> Result<Vec<Archived>> {
    let text = std::fs::read_to_string(verdicts).with_context(|| format!("reading {}", verdicts.display()))?;
    let mut ok: std::collections::HashMap<u64, (Option<u32>, Option<u32>)> = Default::default();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() >= 4 && f[1] == "ok" {
            if let Ok(h) = u64::from_str_radix(f[0], 16) {
                ok.insert(h, (f[2].parse().ok(), f[3].parse().ok()));
            }
        }
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(archive)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        if stem.contains("invalid") {
            continue;
        }
        let Ok(d) = Design::load(&p) else { continue };
        let Some((open, close)) = ok.get(&crate::leaderboard::design_hash(&d)).copied() else { continue };
        let mut facts = crate::video::Facts::of(&d);
        facts.open_ticks = open;
        facts.close_ticks = close;
        out.push(Archived { stem, path: p, design: d, facts });
    }
    Ok(out)
}

/// Height of every door's bottom row.
const DOOR_Y: i32 = 100;
/// Blocks between two designs of a row (moving parts stay well apart; sculk sensors hear 8, a
/// calibrated one 16 blocks: the sculk row uses [`SCULK_GAP`]).
const GAP: i32 = 10;
const SCULK_GAP: i32 = 24;
/// Distance between rows (door planes).
const ROW_STEP: i32 = 40;

/// 3x5 digits, rows top to bottom, `#` = a block.
const DIGITS: [[&str; 5]; 10] = [
    ["###", "#.#", "#.#", "#.#", "###"],
    [".#.", "##.", ".#.", ".#.", "###"],
    ["###", "..#", "###", "#..", "###"],
    ["###", "..#", "###", "..#", "###"],
    ["#.#", "#.#", "###", "..#", "..#"],
    ["###", "#..", "###", "..#", "###"],
    ["###", "#..", "###", "#.#", "###"],
    ["###", "..#", "..#", "..#", "..#"],
    ["###", "#.#", "###", "#.#", "###"],
    ["###", "#.#", "###", "..#", "###"],
];

/// The cells (dx, dy) of `n` written in [`DIGITS`], left to right, dy = 0 at the bottom.
fn number_cells(n: usize) -> (Vec<(i32, i32)>, i32) {
    let text = n.to_string();
    let mut cells = Vec::new();
    for (i, ch) in text.chars().enumerate() {
        let g = DIGITS[ch.to_digit(10).unwrap_or(0) as usize];
        for (row, line) in g.iter().enumerate() {
            for (col, c) in line.chars().enumerate() {
                if c == '#' {
                    cells.push((i as i32 * 4 + col as i32, 4 - row as i32));
                }
            }
        }
    }
    (cells, text.len() as i32 * 4 - 1)
}

/// A string as an SNBT string literal.
fn snbt_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

struct Placed {
    /// Blocks to paste (absolute coordinates, settled rest states).
    blocks: Vec<(Pos, String)>,
    /// Entities to summon (absolute coordinates): type, position, SNBT data.
    entities: Vec<(String, [f64; 3], Option<String>)>,
    /// Red concrete cells of the block count.
    label: Vec<Pos>,
    /// Where the name label floats, and its text.
    text_at: [f64; 3],
    text: String,
}

pub fn build(version: &str, o: &WorldOptions) -> Result<PathBuf> {
    let all = match &o.verdicts {
        Some(v) => archived_from_cache(&o.archive, v)?,
        None => crate::showcase::archived(version, &o.archive, o.workers)?,
    };
    let rotation_known = o.verdicts.is_none();
    let rows: Vec<(&str, Vec<&Archived>)> = {
        let pick = |keep: &dyn Fn(&Archived) -> bool| -> Vec<&Archived> {
            let mut v: Vec<&Archived> = all.iter().filter(|a| keep(a)).collect();
            v.sort_by(|a, b| (a.facts.blocks, &a.stem).cmp(&(b.facts.blocks, &b.stem)));
            v
        };
        // Front (next to the spawn) to back (vjeux's order). The fastest solid doors (open + close
        // within 20 ticks, the speed tracks' 100+ block family) get their own row, fastest first.
        let fast = |a: &Archived| a.facts.category == "solid" && matches!((a.facts.open_ticks, a.facts.close_ticks), (Some(o), Some(c)) if o + c <= 20);
        let mut fastest = pick(&|a| fast(a));
        fastest.sort_by(|a, b| {
            let t = |x: &Archived| x.facts.open_ticks.unwrap_or(99) + x.facts.close_ticks.unwrap_or(99);
            (t(a), a.facts.blocks, &a.stem).cmp(&(t(b), b.facts.blocks, &b.stem))
        });
        vec![
            ("Solid, flat (one block thick), no slime or honey", pick(&|a| a.facts.category == "solid" && a.facts.slime_free && a.facts.flat && !fast(a))),
            ("Solid, no slime or honey", pick(&|a| a.facts.category == "solid" && a.facts.slime_free && !a.facts.flat && !fast(a))),
            ("Solid, with slime or honey", pick(&|a| a.facts.category == "solid" && !a.facts.slime_free && !fast(a))),
            ("Gravity (sand / gravel / concrete powder door)", pick(&|a| a.facts.category == "gravity")),
            ("Fastest solid doors (open + close within 20 ticks), fastest first", fastest),
            ("Sculk (sensors hear you: walk softly)", pick(&|a| a.facts.category == "sculk")),
            ("Entities allowed (minecarts, armor stands, boats... count as blocks)", pick(&|a| a.facts.category == "entity")),
        ]
    };
    let count: usize = rows.iter().map(|(_, v)| v.len()).sum();
    eprintln!("{count} working designs in {} rows", rows.len());

    // Every door faces the viewer, who stands south of each row looking north: a design whose
    // lever faces north is turned 180 degrees when it is rotation-safe (the judge's rotation check
    // passed for it). Then the settled rest state of every design (the judge's placement
    // sequence), in parallel.
    let jobs: Vec<(Design, bool)> = rows
        .iter()
        .flat_map(|(_, v)| v.iter().map(|a| (a.design.clone(), rotation_known && a.facts.rotation_safe && lever_faces_north(&a.design))))
        .collect();
    let pool = Pool::spawn(o.workers.max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let prepared: Vec<Result<(Design, bool, Option<Design>, i32)>> = pool.map(jobs, |w, _, (d, turn)| {
        let d = if turn { crate::rotate_design(w, &d, 2)?.0 } else { d };
        let rest = crate::vanilla_check::rest_state(w, &d)?;
        let top = highest_cell(w, &d)?;
        Ok((d, turn, rest, top))
    });
    drop(pool);
    // Every number at one height: 3 blocks above the highest cell any machine reaches during a
    // cycle (relative to its door), so they line up when you fly over (vjeux).
    let number_y = DOOR_Y + 3 + prepared.iter().filter_map(|r| r.as_ref().ok().map(|x| x.3)).max().unwrap_or(10);
    let mut w = mctas::sim::Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;

    // Layout: rows along x (smallest first, west to east), one behind the other to the north.
    let mut placed: Vec<Placed> = Vec::new();
    // Camera poses (row, eye, yaw, pitch) south of each design looking north, for `--shots`.
    let mut shot_poses: Vec<(usize, [f64; 3], f32, f32)> = Vec::new();
    let mut row_titles: Vec<([f64; 3], String)> = Vec::new();
    // The spawn: (platform lo, platform hi, player position) in front of the first door.
    let mut spawn: Option<(Pos, Pos, [f64; 3])> = None;
    let mut k = 0;
    let mut row_z = 0;
    for (ri, (title, members)) in rows.iter().enumerate() {
        if ri > 0 {
            row_z -= ROW_STEP;
        }
        let gap = if title.starts_with("Sculk") { SCULK_GAP } else { GAP };
        let order = if title.contains("first") { "" } else { ", smallest first" };
        row_titles.push(([-14.0, DOOR_Y as f64 + 8.0, row_z as f64 + 0.5], format!("{title}: {} designs{order}", members.len())));
        let mut cursor = 0;
        for a in members {
            let (d0, turned, rest, _) = match &prepared[k] {
                Ok(x) => x.clone(),
                Err(e) => bail!("{}: {e:#}", a.stem),
            };
            let settled = match rest {
                Some(r) => r,
                None => {
                    eprintln!("warning: {}: no stable rest state in the simulator; pasting the file as written", a.stem);
                    d0.clone()
                }
            };
            k += 1;
            let spec = DoorSpec::from_design(&d0)?;
            let (lo, hi) = d0.bounds().context("empty design")?;
            let t = [cursor - lo[0], DOOR_Y - spec.door[1], row_z - spec.door[2]];
            let mv = |p: Pos| [p[0] + t[0], p[1] + t[1], p[2] + t[2]];
            let blocks: Vec<(Pos, String)> = settled.blocks.iter().map(|(p, s)| (mv(*p), s.clone())).collect();
            // The design's entities (category entity), moved like its blocks.
            let entities: Vec<(String, [f64; 3], Option<String>)> = d0
                .entities
                .iter()
                .map(|e| (e.kind.clone(), [e.pos[0] + t[0] as f64, e.pos[1] + t[1] as f64, e.pos[2] + t[2] as f64], e.nbt.clone()))
                .collect();
            // The number, in the door's plane 5 blocks above the machine, reads left to right
            // from the south (the viewer's side), like every other number (vjeux).
            let (cells, width) = number_cells(a.facts.blocks);
            let center_x = spec.door[0] + 1 + t[0];
            let base_y = number_y;
            let z = spec.door[2] + t[2];
            let label: Vec<Pos> = cells.iter().map(|(dx, dy)| [center_x - width / 2 + dx, base_y + dy, z]).collect();
            {
                let target = [center_x as f64 + 0.5, (DOOR_Y + base_y + 5) as f64 / 2.0, z as f64 + 0.5];
                let eye = [target[0], target[1] + 3.0, target[2] + 26.0];
                let (yaw, pitch) = mctas::video::look_at(eye, target);
                shot_poses.push((ri, eye, yaw, pitch));
            }
            if spawn.is_none() {
                spawn = Some(spawn_platform(&mut w, &d0, t)?);
            }
            let mut text = format!("{}: {} blocks, {}", a.stem, a.facts.blocks, a.facts.kind());
            if rotation_known && !a.facts.rotation_safe {
                text.push_str(", NOT rotation-safe");
                if lever_faces_north(&d0) {
                    text.push_str(" (its lever is on the far side: it only works facing this way)");
                }
            }
            if let (Some(op), Some(cl)) = (a.facts.open_ticks, a.facts.close_ticks) {
                text.push_str(&format!(" (opens in {op} ticks, closes in {cl})"));
            }
            if turned {
                text.push_str(", turned 180 degrees so its lever faces you");
            }
            placed.push(Placed { blocks, entities, label, text_at: [center_x as f64 + 0.5, base_y as f64 + 7.0, z as f64 + 0.5], text });
            cursor += hi[0] - lo[0] + 1 + gap;
        }
    }
    let (plo, phi, spawn_at) = spawn.context("no design")?;

    // Build it in the official server.
    let mut v = VanillaServer::start(&VanillaOptions { version: version.to_string(), keep_dir: true, level_name: o.name.clone(), ..Default::default() })?;
    let world_dir = v.dir.join(&v.level_name);
    eprintln!("vanilla server up ({})", v.dir.display());
    // Every block, number and label goes into one datapack function, run as one command (a command
    // per block over RCON took minutes): load the chunks, write the function, run it.
    let mut boxes: Vec<(Pos, Pos)> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for p in &placed {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for q in p.blocks.iter().map(|(q, _)| *q).chain(p.label.iter().copied()) {
            for j in 0..3 {
                lo[j] = lo[j].min(q[j]);
                hi[j] = hi[j].max(q[j]);
            }
        }
        boxes.push((lo, hi));
        for (q, s) in &p.blocks {
            lines.push(format!("setblock {} {} {} {s} strict", q[0], q[1], q[2]));
        }
        // Entities after the blocks they stand on.
        for (kind, [x, y, z], nbt) in &p.entities {
            lines.push(format!("summon {kind} {x} {y} {z}{}", nbt.as_ref().map(|n| format!(" {n}")).unwrap_or_default()));
        }
        for q in &p.label {
            lines.push(format!("setblock {} {} {} minecraft:red_concrete strict", q[0], q[1], q[2]));
        }
        let [x, y, z] = p.text_at;
        lines.push(format!("summon minecraft:text_display {x} {y} {z} {{billboard:\"center\",text:{},background:1073741824}}", snbt_str(&p.text)));
    }
    for ([x, y, z], text) in &row_titles {
        let c = [*x as i32, *y as i32, *z as i32];
        boxes.push((c, c));
        lines.push(format!(
            "summon minecraft:text_display {x} {y} {z} {{billboard:\"center\",text:{},transformation:{{left_rotation:[0f,0f,0f,1f],right_rotation:[0f,0f,0f,1f],translation:[0f,0f,0f],scale:[3f,3f,3f]}}}}",
            snbt_str(text)
        ));
    }
    // Spawn: a glass platform in front of the first door (far enough from every cell the machine
    // ever occupies that it cannot touch it, close enough to reach its lever), facing north.
    boxes.push((plo, phi));
    lines.push(format!("fill {} {} {} {} {} {} minecraft:glass strict", plo[0], plo[1], plo[2], phi[0], phi[1], phi[2]));
    let [sx, sy, sz] = spawn_at;
    lines.push(format!("setworldspawn {} {} {} 180 15", sx.floor() as i32, sy.floor() as i32, sz.floor() as i32));
    lines.push(format!(
        "summon minecraft:text_display {} {} {} {{billboard:\"center\",text:{},transformation:{{left_rotation:[0f,0f,0f,1f],right_rotation:[0f,0f,0f,1f],translation:[0f,0f,0f],scale:[1.5f,1.5f,1.5f]}}}}",
        sx - 9.0,
        sy + 3.0,
        sz - 4.0,
        snbt_str(&format!(
            "minecraft-tas: {count} working 3x3 piston doors. Rows front to back: flat, no slime, slime or honey, sand, fastest, sculk, entities. Red number = blocks. Every lever faces you; fly east along a row, north to the next row."
        ))
    ));
    let mut chunks: std::collections::BTreeSet<(i32, i32)> = Default::default();
    for (lo, hi) in &boxes {
        v.cmd(&format!("forceload add {} {} {} {}", lo[0], lo[2], hi[0], hi[2]))?;
        for cx in lo[0].div_euclid(16)..=hi[0].div_euclid(16) {
            for cz in lo[2].div_euclid(16)..=hi[2].div_euclid(16) {
                chunks.insert((cx, cz));
            }
        }
    }
    let t0 = std::time::Instant::now();
    for (cx, cz) in &chunks {
        while !v.cmd(&format!("execute if loaded {} 0 {}", cx * 16, cz * 16))?.contains("Test passed") {
            if t0.elapsed() > std::time::Duration::from_secs(180) {
                bail!("chunk {cx},{cz} never finished loading");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    let pack = world_dir.join("datapacks").join("mctas_world");
    std::fs::create_dir_all(pack.join("data").join("mctas").join("function"))?;
    std::fs::write(pack.join("pack.mcmeta"), r#"{"pack":{"description":"mctas world build","min_format":121,"max_format":121}}"#)?;
    std::fs::write(pack.join("data").join("mctas").join("function").join("build.mcfunction"), lines.join("\n") + "\n")?;
    // /reload finishes asynchronously and enables the newly found pack: retry while the function
    // is unknown; it must run exactly once (it summons the labels).
    v.cmd("reload")?;
    let out = loop {
        let out = v.cmd("function mctas:build")?;
        if out.contains("Executed") || out.contains("Running function") {
            break out;
        }
        if !out.contains("Unknown function") || t0.elapsed() > std::time::Duration::from_secs(240) {
            bail!("function mctas:build: {out}");
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    };
    eprintln!("built {} designs ({} commands in one function): {}", placed.len(), lines.len(), out.trim());
    let _ = v.cmd("datapack disable \"file/mctas_world\"");
    let _ = v.cmd("gamerule respawn_radius 0");
    for c in ["gamerule advance_time false", "gamerule advance_weather false", "gamerule spawn_mobs false", "time set noon", "weather clear"] {
        let _ = v.cmd(c);
    }
    // Let the world run a little (every design is at rest: nothing should move), then save.
    v.cmd_ok("forceload remove all")?;
    v.cmd_ok("tick unfreeze")?;
    std::thread::sleep(std::time::Duration::from_secs(5));
    v.cmd_ok("save-on")?;
    let out = v.cmd("save-all flush")?;
    if !out.to_lowercase().contains("saved") {
        bail!("save-all flush: {out}");
    }
    drop(v);

    // Copy the save out (without the bot's player data) and pack it.
    let dest = o.out.join(&o.name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest)?;
    }
    std::fs::create_dir_all(&o.out)?;
    copy_dir(&world_dir, &dest)?;
    let _ = std::fs::remove_dir_all(dest.join("playerdata"));
    let _ = std::fs::remove_dir_all(dest.join("players"));
    let _ = std::fs::remove_dir_all(dest.join("datapacks").join("mctas_world"));
    if let Some(server_dir) = world_dir.parent() {
        let _ = std::fs::remove_dir_all(server_dir);
    }
    // With --check: read every design and number back from the saved region files (the save must
    // hold exactly what was pasted, after 5 s of running, so a design not at rest shows up too).
    if o.check {
        let dim = {
            let modern = dest.join("dimensions").join("minecraft").join("overworld");
            if modern.exists() { modern } else { dest.clone() }
        };
        let mut bad = 0;
        // 26.x saves a block's default state as a bare name: expand those with the simulator.
        let mut oracle = crate::vanilla_check::Oracle::default();
        for p in &placed {
            let mut lo = [i32::MAX; 3];
            let mut hi = [i32::MIN; 3];
            for q in p.blocks.iter().map(|(q, _)| *q).chain(p.label.iter().copied()) {
                for j in 0..3 {
                    lo[j] = lo[j].min(q[j]);
                    hi[j] = hi[j].max(q[j]);
                }
            }
            let mut snap = mctas::vanilla::read_box(&dim, lo, hi)?;
            oracle.apply(&mut w, &mut [&mut snap.region])?;
            let mut diffs: Vec<String> = p.blocks.iter().filter(|(q, s)| snap.region.get(*q) != Some(s.as_str())).map(|(q, s)| format!("{q:?} {s} -> {}", snap.region.get(*q).unwrap_or("?"))).collect();
            diffs.extend(p.label.iter().filter(|q| snap.region.get(**q) != Some("minecraft:red_concrete")).map(|q| format!("{q:?} label missing")));
            if !diffs.is_empty() {
                bad += 1;
                eprintln!("MISMATCH in the save: {}: {}", p.text, diffs.iter().take(4).cloned().collect::<Vec<_>>().join("; "));
            }
        }
        if bad > 0 {
            bail!("{bad} designs are not in the save exactly as pasted");
        }
        eprintln!("read back: all {} designs and their numbers are in the save exactly", placed.len());
    }
    let tar = o.out.join(format!("{}.tar.gz", o.name));
    let status = std::process::Command::new("tar").arg("-C").arg(&o.out).arg("-czf").arg(&tar).arg(&o.name).status()?;
    if !status.success() {
        bail!("tar failed");
    }
    eprintln!("world: {} ({count} designs), packed as {}", dest.display(), tar.display());
    if o.shots > 0 {
        screenshots(version, &dest, spawn_at, &shot_poses, o.shots, &o.out.join(format!("{}-shots", o.name)))?;
    }
    Ok(tar)
}

/// The highest cell (relative to the door's bottom row) the machine occupies at rest or at any
/// tick of one open/close cycle (the judge's first position).
fn highest_cell(w: &mut mctas::sim::Worker, d: &Design) -> Result<i32> {
    let spec = DoorSpec::from_design(d)?;
    let mut top = d.bounds().context("empty design")?.1[1];
    let opts = crate::VerifyOptions { cycles: 1, offsets: vec![[0, 0, 0]], keep_frames: true, empty_walkway: false, ..Default::default() };
    if let Ok(r) = crate::verify(w, d, &opts) {
        for f in r.open_frames.iter().chain(r.close_frames.iter()) {
            for (p, s) in f.iter() {
                if !mctas::blockstate::is_air(s) {
                    top = top.max(p[1]);
                }
            }
        }
    }
    Ok(top - spec.door[1])
}

/// Does the design's lever face north (away from a viewer south of the door)? A wall lever
/// pointing north; otherwise (floor, ceiling, pointing sideways) a lever north of the door plane.
fn lever_faces_north(d: &Design) -> bool {
    let (Some(lever), Some(door)) = (d.ports.get("lever"), d.ports.get("door")) else { return false };
    let state = d.blocks.get(lever).map(String::as_str).unwrap_or("");
    let prop = |k: &str| state.split(['[', ',', ']']).find_map(|kv| kv.strip_prefix(k).and_then(|r| r.strip_prefix('=')).map(String::from));
    match (prop("face").as_deref(), prop("facing").as_deref()) {
        (Some("wall"), Some("north")) => true,
        (Some("wall"), Some("south")) => false,
        _ => lever[2] < door[2],
    }
}

/// The spawn platform for a design placed with offset `t`: 5x3 glass at the door's floor level
/// south of the door, at Chebyshev distance >= 2 from every cell the machine occupies during a
/// cycle (so it touches nothing: no observer sees it, no slime drags it, no push reaches it),
/// as close as that allows (to reach the lever). Returns (lo, hi, player position).
fn spawn_platform(w: &mut mctas::sim::Worker, d: &Design, t: Pos) -> Result<(Pos, Pos, [f64; 3])> {
    let spec = DoorSpec::from_design(d)?;
    let opts = crate::VerifyOptions { cycles: 1, offsets: vec![[0, 0, 0]], keep_frames: true, empty_walkway: false, ..Default::default() };
    let r = crate::verify(w, d, &opts)?;
    let mut used: std::collections::HashSet<Pos> = d.blocks.keys().copied().collect();
    for f in r.open_frames.iter().chain(r.close_frames.iter()) {
        for (p, s) in f.iter() {
            if !mctas::blockstate::is_air(s) {
                used.insert(p);
            }
        }
    }
    let y = spec.door[1] - 1;
    let cx = spec.door[0] + 1;
    let clear = |z0: i32| {
        (cx - 2..=cx + 2).all(|x| (z0..=z0 + 2).all(|z| !used.iter().any(|u| (u[0] - x).abs() < 2 && (u[1] - y).abs() < 2 && (u[2] - z).abs() < 2)))
    };
    let z0 = (spec.door[2] + 1..spec.door[2] + 30).find(|z| clear(*z)).context("no room for the spawn platform")?;
    // Back off from the closest safe spot and rise a few blocks: an overview of the rows on
    // joining rather than standing in the first door (vjeux); fly forward to reach a lever.
    let (z0, y) = (z0 + 12, y + 4);
    let mv = |p: Pos| [p[0] + t[0], p[1] + t[1], p[2] + t[2]];
    let lo = mv([cx - 3, y, z0]);
    let hi = mv([cx + 3, y, z0 + 4]);
    let at = mv([cx, y + 1, z0 + 2]);
    Ok((lo, hi, [at[0] as f64 + 0.5, at[1] as f64, at[2] as f64 + 0.5]))
}

/// Open the saved world in the official server, join it with the real client, and take a
/// screenshot from the spawn and of the first `n` designs of each row from the viewer's side.
fn screenshots(version: &str, world: &Path, spawn_at: [f64; 3], poses: &[(usize, [f64; 3], f32, f32)], n: usize, out: &Path) -> Result<()> {
    use mctas::video::{Client, Display};
    std::fs::create_dir_all(out)?;
    let mut v = VanillaServer::start(&VanillaOptions { version: version.to_string(), world: Some(world.to_path_buf()), view_distance: 6, ..Default::default() })?;
    let (w, h) = (1280, 720);
    let display = Display::start(w, h)?;
    let build = mctas::video::build_client(version)?;
    let game_dir = v.dir.join("client");
    let mut client = Client::launch(&build, &display, v.game_port(), w, h, &game_dir)?;
    let t0 = std::time::Instant::now();
    while !v.cmd("list")?.contains("mctas,") && !v.cmd("list")?.trim_end().ends_with("mctas") {
        if !client.alive() {
            bail!("the client exited; see {}", client.log.display());
        }
        if t0.elapsed() > std::time::Duration::from_secs(240) {
            bail!("the client never joined; see {}", client.log.display());
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    v.cmd_ok("gamemode spectator mctas")?;
    // Entities (the name labels) reach a client through the entity tracker, which only runs in
    // ticks: run the world (every design is at rest).
    v.cmd_ok("tick unfreeze")?;
    eprintln!("text labels in the world: {}", v.count_entities("@e[type=minecraft:text_display]")?);
    let mut shots: Vec<(String, [f64; 3], f32, f32)> = vec![("spawn".into(), [spawn_at[0], spawn_at[1] + 1.62, spawn_at[2]], 180.0, 10.0)];
    let mut per_row: std::collections::HashMap<usize, usize> = Default::default();
    for (row, eye, yaw, pitch) in poses {
        let k = per_row.entry(*row).or_default();
        if *k < n {
            *k += 1;
            shots.push((format!("row{row}-{k}"), *eye, *yaw, *pitch));
        }
    }
    for (name, eye, yaw, pitch) in shots {
        v.cmd_ok(&format!("tp mctas {} {} {} {yaw} {pitch}", eye[0], eye[1] - 1.62, eye[2]))?;
        std::thread::sleep(std::time::Duration::from_secs(8));
        let path = out.join(format!("{name}.png"));
        mctas::video::screenshot(&display, w, h, &path)?;
        eprintln!("shot {}", path.display());
    }
    drop(client);
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let p = e.path();
        let q = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&p, &q)?;
        } else if e.file_name() != "session.lock" {
            std::fs::copy(&p, &q)?;
        }
    }
    Ok(())
}
