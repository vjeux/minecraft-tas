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
use crate::video::design_shot;
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
        vec![
            ("Solid, with slime or honey", pick(&|a| a.facts.category == "solid" && !a.facts.slime_free)),
            ("Solid, no slime or honey", pick(&|a| a.facts.category == "solid" && a.facts.slime_free && !a.facts.flat)),
            ("Solid, flat (one block thick), no slime or honey", pick(&|a| a.facts.category == "solid" && a.facts.slime_free && a.facts.flat)),
            ("Gravity (sand / gravel / concrete powder door)", pick(&|a| a.facts.category == "gravity")),
            ("Sculk (sensors hear you: walk softly)", pick(&|a| a.facts.category == "sculk")),
        ]
    };
    let count: usize = rows.iter().map(|(_, v)| v.len()).sum();
    eprintln!("{count} working designs in {} rows", rows.len());

    // The settled rest state of every design (the judge's placement sequence), in parallel.
    let designs: Vec<Design> = rows.iter().flat_map(|(_, v)| v.iter().map(|a| a.design.clone())).collect();
    let pool = Pool::spawn(o.workers.max(1), &WorkerConfig { version: version.to_string(), ..Default::default() })?;
    let rest: Vec<Result<Option<Design>>> = pool.map(designs.clone(), |w, _, d| crate::vanilla_check::rest_state(w, &d));
    drop(pool);

    // Layout.
    let mut placed: Vec<Placed> = Vec::new();
    // Camera poses (row, eye, yaw, pitch) in front of each design, for `--shots`.
    let mut shot_poses: Vec<(usize, [f64; 3], f32, f32)> = Vec::new();
    let mut row_titles: Vec<([f64; 3], String)> = Vec::new();
    let mut k = 0;
    let mut row_z = 0;
    for (ri, (title, members)) in rows.iter().enumerate() {
        if ri > 0 {
            // The sculk row keeps an extra distance from the pistons of the other rows.
            row_z += if title.starts_with("Sculk") { ROW_STEP + 20 } else { ROW_STEP };
        }
        let gap = if title.starts_with("Sculk") { SCULK_GAP } else { GAP };
        row_titles.push(([-14.0, DOOR_Y as f64 + 8.0, row_z as f64 + 0.5], format!("{title}: {} designs, smallest first", members.len())));
        let mut cursor = 0;
        for a in members {
            let d0 = &designs[k];
            let settled = match &rest[k] {
                Ok(Some(r)) => r.clone(),
                Ok(None) => {
                    eprintln!("warning: {}: no stable rest state in the simulator; pasting the file as written", a.stem);
                    d0.clone()
                }
                Err(e) => bail!("{}: {e:#}", a.stem),
            };
            k += 1;
            let spec = DoorSpec::from_design(d0)?;
            let (lo, hi) = d0.bounds().context("empty design")?;
            let t = [cursor - lo[0], DOOR_Y - spec.door[1], row_z - spec.door[2]];
            let mv = |p: Pos| [p[0] + t[0], p[1] + t[1], p[2] + t[2]];
            let blocks: Vec<(Pos, String)> = settled.blocks.iter().map(|(p, s)| (mv(*p), s.clone())).collect();
            // The number, readable from the side the video camera films (the door's front),
            // in the door's plane, 5 blocks above the machine.
            let shot = design_shot(d0, 0.0, 16.0 / 9.0)?;
            let front_south = shot.dir[2] >= 0.0;
            {
                // Frame the machine and its number above it.
                let mut s = shot.translated(t);
                let top = (hi[1] + t[1] + 10) as f64;
                s.target[1] = (s.target[1] + top) / 2.0;
                s.dist *= 2.0;
                let (eye, yaw, pitch) = s.pose(0.0);
                shot_poses.push((ri, eye, yaw, pitch));
            }
            let (cells, width) = number_cells(a.facts.blocks);
            let center_x = spec.door[0] + 1 + t[0];
            let base_y = hi[1] + t[1] + 5;
            let z = spec.door[2] + t[2];
            let label: Vec<Pos> = cells
                .iter()
                .map(|(dx, dy)| {
                    let x = if front_south { center_x - width / 2 + dx } else { center_x + width / 2 - dx };
                    [x, base_y + dy, z]
                })
                .collect();
            let mut text = format!("{}: {} blocks, {}", a.stem, a.facts.blocks, a.facts.kind());
            if rotation_known && !a.facts.rotation_safe {
                text.push_str(", NOT rotation-safe");
            }
            if let (Some(op), Some(cl)) = (a.facts.open_ticks, a.facts.close_ticks) {
                text.push_str(&format!(" (opens in {op} ticks, closes in {cl})"));
            }
            placed.push(Placed { blocks, label, text_at: [center_x as f64 + 0.5, base_y as f64 + 7.0, z as f64 + 0.5], text });
            cursor += hi[0] - lo[0] + 1 + gap;
        }
    }

    // Build it in the official server.
    let mut v = VanillaServer::start(&VanillaOptions { version: version.to_string(), keep_dir: true, ..Default::default() })?;
    let world_dir = v.dir.join("world");
    eprintln!("vanilla server up ({})", v.dir.display());
    for (i, p) in placed.iter().enumerate() {
        let mut lo = [i32::MAX; 3];
        let mut hi = [i32::MIN; 3];
        for q in p.blocks.iter().map(|(q, _)| *q).chain(p.label.iter().copied()) {
            for j in 0..3 {
                lo[j] = lo[j].min(q[j]);
                hi[j] = hi[j].max(q[j]);
            }
        }
        crate::vanilla_check::load_box(&mut v, lo, hi)?;
        for (q, s) in &p.blocks {
            let out = v.cmd(&format!("setblock {} {} {} {s} strict", q[0], q[1], q[2]))?;
            if !out.contains("Changed the block") {
                bail!("setblock {q:?} {s}: {out}");
            }
        }
        for q in &p.label {
            v.cmd_ok(&format!("setblock {} {} {} minecraft:red_concrete strict", q[0], q[1], q[2]))?;
        }
        let [x, y, z] = p.text_at;
        let out = v.cmd_ok(&format!(
            "summon minecraft:text_display {x} {y} {z} {{billboard:\"center\",text:{},background:1073741824}}",
            snbt_str(&p.text)
        ))?;
        if i == 0 {
            eprintln!("  first label: {out}");
        }
        if (i + 1) % 20 == 0 {
            eprintln!("  {} / {} designs built", i + 1, placed.len());
        }
    }
    for ([x, y, z], text) in &row_titles {
        crate::vanilla_check::load_box(&mut v, [*x as i32, *y as i32, *z as i32], [*x as i32, *y as i32, *z as i32])?;
        v.cmd_ok(&format!(
            "summon minecraft:text_display {x} {y} {z} {{billboard:\"center\",text:{},transformation:{{left_rotation:[0f,0f,0f,1f],right_rotation:[0f,0f,0f,1f],translation:[0f,0f,0f],scale:[3f,3f,3f]}}}}",
            snbt_str(text)
        ))?;
    }
    // Spawn: a glass platform west of the first row, wide enough for any spawn radius.
    let (sx, sz) = (-40, 0);
    crate::vanilla_check::load_box(&mut v, [sx - 12, DOOR_Y - 1, sz - 12], [sx + 12, DOOR_Y - 1, sz + 12])?;
    v.cmd_ok(&format!("fill {} {} {} {} {} {} minecraft:glass strict", sx - 12, DOOR_Y - 1, sz - 12, sx + 12, DOOR_Y - 1, sz + 12))?;
    v.cmd_ok(&format!("setworldspawn {sx} {DOOR_Y} {sz}"))?;
    v.cmd_ok(&format!(
        "summon minecraft:text_display {sx}.5 {} {sz}.5 {{billboard:\"center\",text:{},transformation:{{left_rotation:[0f,0f,0f,1f],right_rotation:[0f,0f,0f,1f],translation:[0f,0f,0f],scale:[2f,2f,2f]}}}}",
        DOOR_Y + 4,
        snbt_str(&format!("minecraft-tas: {count} working 3x3 piston doors. Fly east (creative). Red number = blocks."))
    ))?;
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
    if let Some(server_dir) = world_dir.parent() {
        let _ = std::fs::remove_dir_all(server_dir);
    }
    // Read every design and number back from the saved region files: the save must hold exactly
    // what was pasted (after 5 s of running, so a design that is not at rest shows up here too).
    let dim = {
        let modern = dest.join("dimensions").join("minecraft").join("overworld");
        if modern.exists() { modern } else { dest.clone() }
    };
    let mut bad = 0;
    // 26.x saves a block's default state as a bare name: expand those with the simulator.
    let mut oracle = crate::vanilla_check::Oracle::default();
    let mut w = mctas::sim::Worker::spawn(&WorkerConfig { version: version.to_string(), ..Default::default() })?;
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
    let tar = o.out.join(format!("{}.tar.gz", o.name));
    let status = std::process::Command::new("tar").arg("-C").arg(&o.out).arg("-czf").arg(&tar).arg(&o.name).status()?;
    if !status.success() {
        bail!("tar failed");
    }
    eprintln!("world: {} ({count} designs), packed as {}", dest.display(), tar.display());
    if o.shots > 0 {
        screenshots(version, &dest, &placed, &shot_poses, o.shots, &o.out.join(format!("{}-shots", o.name)))?;
    }
    Ok(tar)
}

/// Open the saved world in the official server, join it with the real client, and take a
/// screenshot of the spawn and of the first `n` designs of each row from their front.
fn screenshots(version: &str, world: &Path, placed: &[Placed], poses: &[(usize, [f64; 3], f32, f32)], n: usize, out: &Path) -> Result<()> {
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
    let mut shots: Vec<(String, [f64; 3], f32, f32)> = vec![("spawn".into(), [-52.0, DOOR_Y as f64 + 6.0, 10.0], -120.0, 15.0)];
    let mut per_row: std::collections::HashMap<usize, usize> = Default::default();
    for (row, eye, yaw, pitch) in poses {
        let k = per_row.entry(*row).or_default();
        if *k < n {
            *k += 1;
            shots.push((format!("row{row}-{k}"), *eye, *yaw, *pitch));
        }
    }
    let _ = placed;
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
