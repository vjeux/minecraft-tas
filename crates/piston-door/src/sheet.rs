//! `mctas door sheet <file>...`: one markdown page that draws mechanisms by eye. Each file is an
//! ideal schedule (`door ideal` format) or a design with a lever; the page shows the plane of the
//! door at rest and once its opening has settled, side by side, and counts the FRAME at open:
//! the 12 cells around the opening (3 ceiling, 3 floor, 3 per wall) that the target requires to
//! hold full blocks when open (vjeux, 2026-10-19). Openings are replayed in redsim (the exact
//! model, milliseconds per mechanism), so a whole mechanism list draws in a second; no Minecraft.
//!
//! Glyphs: `·` air, `#` full block (door blocks, stone), `X` obsidian, piston bases by facing
//! (sticky upper case `U D E W`, normal lower case `u d e w`), heads `^ v > <`, `~` moving block,
//! `R` redstone block (an ideal input when the schedule sets it), `o` observer, `q` repeater,
//! `c` comparator, `t` torch, `+` dust, `l` lever, `%` anything else. `^^^` under a plane marks
//! the opening's columns; in the open plane an empty frame cell is drawn `!`.

use crate::DoorSpec;
use crate::cosearch::{RsDoor, Snap, bp, convert_state, full_cube, step_until_idle, watch_box};
use crate::ideal::Schedule;
use anyhow::{Context, Result};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::Pos;
use redsim::{Block, BlockPos, World};
use std::path::{Path, PathBuf};

fn glyph(state: &str) -> char {
    let Ok(b) = BlockState::parse(state) else { return '?' };
    let facing = |upper: bool| {
        let c = match b.get("facing") {
            Some("up") => 'u',
            Some("down") => 'd',
            Some("east") => 'e',
            Some("west") => 'w',
            Some("north") => 'n',
            Some("south") => 's',
            _ => '?',
        };
        if upper { c.to_ascii_uppercase() } else { c }
    };
    match b.short_name() {
        "air" | "cave_air" | "void_air" => '·',
        "obsidian" | "crying_obsidian" => 'X',
        "sticky_piston" => facing(true),
        "piston" => facing(false),
        "piston_head" => match b.get("facing") {
            Some("up") => '^',
            Some("down") => 'v',
            Some("east") => '>',
            Some("west") => '<',
            _ => '*',
        },
        "moving_piston" => '~',
        "redstone_block" => 'R',
        "observer" => 'o',
        "repeater" => 'q',
        "comparator" => 'c',
        "redstone_torch" | "redstone_wall_torch" => 't',
        "redstone_wire" => '+',
        "lever" => 'l',
        "slime_block" => 's',
        "honey_block" => 'y',
        n if n.contains("stone") || n.contains("concrete") || n.contains("planks") || n.contains("glass") || n == "dirt" => '#',
        _ => '%',
    }
}

/// The 12 frame cells around the opening (door plane): ceiling, floor, left wall, right wall.
pub fn frame_cells(door: Pos) -> Vec<Pos> {
    let [x, y, z] = door;
    let mut out = Vec::new();
    for dx in 0..3 {
        out.push([x + dx, y + 3, z]);
    }
    for dx in 0..3 {
        out.push([x + dx, y - 1, z]);
    }
    for dy in 0..3 {
        out.push([x - 1, y + dy, z]);
    }
    for dy in 0..3 {
        out.push([x + 3, y + dy, z]);
    }
    out
}

fn filled(b: &Block) -> bool {
    // A frame cell counts when it holds a plain full block (a door block or a quartz-like
    // building block); pistons, observers and other circuitry do not.
    !b.is_air() && full_cube(b) && !crate::cosearch::is_moving(b) && crate::cosearch::quartz_like(b)
}

/// `filled` for other modules (door cycle).
pub fn filled_frame(b: &Block) -> bool {
    filled(b)
}

/// The sheet glyph of a redsim block.
pub fn glyph_of(b: &Block) -> char {
    glyph(&b.to_string())
}

struct Drawn {
    title: String,
    pistons: usize,
    /// Pistons below the opening (y < door.y): (position, short state).
    below: Vec<(Pos, String)>,
    rest: Vec<String>,
    open: Option<std::result::Result<(Vec<String>, usize, Vec<Pos>), String>>,
    /// `--fill`: can static stone in the frame cells left empty when open make it 12/12?
    fill: Option<String>,
}

/// Place a design like the judge in a fresh redsim world and settle it.
fn build_world(d: &Design, spec: &DoorSpec) -> std::result::Result<(World, Pos, Pos), String> {
    let mut blocks: Vec<(Pos, Block)> = Vec::new();
    for (p, s) in &d.blocks {
        blocks.push((*p, convert_state(s)?));
    }
    let (wlo, whi) = watch_box(&blocks, spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 4, wlo[1] - 4, wlo[2] - 4), [whi[0] - wlo[0] + 9, whi[1] - wlo[1] + 9, whi[2] - wlo[2] + 9]);
    let list: Vec<(BlockPos, Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
    w.place_like_judge(&list);
    let (_, idle) = step_until_idle(&mut w, 300, |_| {});
    if !idle {
        return Err("never settles after placement".into());
    }
    Ok((w, wlo, whi))
}

/// Apply one phase of an ideal schedule (like `door ideal`), then settle.
fn replay(w: &mut World, s: &Schedule, open: bool) -> std::result::Result<(), String> {
    let evs = s.events_of(open, s.vars()).map_err(|e| format!("{e:#}"))?;
    let mut t = 0u32;
    let mut i = 0;
    while i < evs.len() {
        while i < evs.len() && evs[i].0 == t {
            let (_, p, st) = &evs[i];
            if st.as_deref() == Some("@use") {
                w.use_block(bp(*p));
            } else {
                let state = st.clone().unwrap_or_else(|| "minecraft:air".to_string());
                let b = convert_state(&state)?;
                w.set_block_and_update(bp(*p), b);
            }
            i += 1;
        }
        if i < evs.len() {
            w.tick();
            t += 1;
        }
    }
    let (_, idle) = step_until_idle(w, 300, |_| {});
    if !idle {
        return Err("still moving 300 ticks after the last event".into());
    }
    Ok(())
}

/// `fill_test` for a real door (lever): stone in the frame cells empty when open (air at rest),
/// then the exact redsim judge (4 positions x 3 cycles, `door verify`'s checks) and the frame
/// check on the recorded open state. Writes `<title>-ff.txt` when it works.
fn fill_design(design: &Design, rest_get: &dyn Fn(BlockPos) -> Block, empty: &[Pos], title: &str, out_dir: Option<&Path>) -> String {
    if empty.is_empty() {
        return "already 12/12".into();
    }
    let mut d2 = design.clone();
    for p in empty {
        if !rest_get(bp(*p)).is_air() {
            return format!("FAIL: frame cell {p:?} is not air at rest");
        }
        d2.blocks.insert(*p, "minecraft:stone".to_string());
    }
    let rs = match RsDoor::from_design(&d2) {
        Ok(r) => r,
        Err(e) => return format!("FAIL: {e}"),
    };
    if let Err(e) = crate::cosearch::judge(&rs, &crate::cosearch::JudgeOpts::with_extra(0)) {
        return format!("FAIL (redsim judge): {}", e.to_string().chars().take(160).collect::<String>());
    }
    let rec = match crate::cosearch::record(&rs) {
        Ok(r) => r,
        Err(e) => return format!("FAIL: {e}"),
    };
    let open = rec.open_state();
    let bad: Vec<Pos> = frame_cells(rs.spec.door).into_iter().filter(|p| !open.get(*p).is_some_and(|b| filled(&b))).collect();
    if !bad.is_empty() {
        return format!("FAIL: frame cells still empty when open {bad:?}");
    }
    if let Some(dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
        let _ = std::fs::write(dir.join(format!("{title}-ff.txt")), d2.to_text());
    }
    format!("OK (redsim judge, 4 positions x 3 cycles): +{} stone at {:?}", empty.len(), empty)
}

/// Put stone in the frame cells the open state leaves empty, replay two open/close cycles of
/// the schedule in redsim: opening clear, frame 12/12 when open, back at rest when closed.
/// Writes `<title>-ff.txt` + `<title>-ff.ideal` into `out_dir` when it works.
fn fill_test(design: &Design, s: &Schedule, text: &str, empty: &[Pos], title: &str, out_dir: Option<&Path>) -> String {
    if empty.is_empty() {
        return "already 12/12".into();
    }
    let mut d2 = design.clone();
    for p in empty {
        if let Some(b) = design.blocks.get(p) {
            return format!("FAIL: frame cell {p:?} holds {b} at rest");
        }
        d2.blocks.insert(*p, "minecraft:stone".to_string());
    }
    let Some(door) = design.ports.get("door").copied() else { return "FAIL: no door port".into() };
    let spec = DoorSpec { door, lever: [0, 0, 0], along_z: false };
    let (mut w, lo, hi) = match build_world(&d2, &spec) {
        Ok(x) => x,
        Err(e) => return format!("FAIL: {e}"),
    };
    let rest = Snap::take(&w, lo, hi);
    let frame = frame_cells(door);
    for cycle in 0..2 {
        if let Err(e) = replay(&mut w, s, true) {
            return format!("FAIL (cycle {cycle} open): {e}");
        }
        if !spec.door_cells().iter().all(|c| w.get(bp(*c)).is_air()) {
            return format!("FAIL (cycle {cycle}): the opening is not clear");
        }
        let bad: Vec<Pos> = frame.iter().copied().filter(|p| !filled(&w.get(bp(*p)))).collect();
        if !bad.is_empty() {
            return format!("FAIL (cycle {cycle}): frame cells empty when open {bad:?}");
        }
        if let Err(e) = replay(&mut w, s, false) {
            return format!("FAIL (cycle {cycle} close): {e}");
        }
        let now = Snap::take(&w, lo, hi);
        if now != rest {
            let diffs: Vec<String> = rest
                .iter()
                .filter(|(p, b)| now.get(*p) != Some(*b))
                .take(3)
                .map(|(p, b)| format!("{p:?} {b} -> {}", now.get(p).map(|x| x.to_string()).unwrap_or_default()))
                .collect();
            return format!("FAIL (cycle {cycle}): not back at rest: {}", diffs.join("; "));
        }
    }
    if let Some(dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
        let dpath = dir.join(format!("{title}-ff.txt"));
        let _ = std::fs::write(&dpath, d2.to_text());
        let sched: Vec<String> = text
            .lines()
            .map(|l| if l.trim_start().starts_with("design ") { format!("design {}", dpath.display()) } else { l.to_string() })
            .collect();
        let _ = std::fs::write(dir.join(format!("{title}-ff.ideal")), sched.join("\n") + "\n");
    }
    format!("OK: +{} stone at {:?}", empty.len(), empty)
}

#[allow(clippy::too_many_arguments)]
fn plane(get: &dyn Fn(BlockPos) -> Block, z: i32, x0: i32, y0: i32, x1: i32, y1: i32, door: Pos, mark_frame: bool) -> Vec<String> {
    let frame = frame_cells(door);
    let mut out = Vec::new();
    for y in (y0..=y1).rev() {
        let mut line = format!("{y:>3} ");
        for x in x0..=x1 {
            let b = get(BlockPos::new(x, y, z));
            let c = if mark_frame && frame.contains(&[x, y, z]) && !filled(&b) { '!' } else { glyph(&b.to_string()) };
            line.push(c);
        }
        out.push(line);
    }
    let mut marks = String::from("    ");
    for x in x0..=x1 {
        marks.push(if (door[0]..door[0] + 3).contains(&x) { '^' } else { ' ' });
    }
    out.push(marks);
    out
}

fn draw(path: &Path, fill: bool, fill_dir: Option<&Path>) -> Result<Drawn> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let is_schedule = text.lines().any(|l| l.trim_start().starts_with("design "));
    let (design, schedule): (Design, Option<Schedule>) = if is_schedule {
        let s = Schedule::parse(&text).with_context(|| path.display().to_string())?;
        (s.design().clone(), Some(s))
    } else {
        (crate::cli::load_design(path)?, None)
    };
    let door = *design.ports.get("door").context("design has no door port")?;
    let z = door[2];
    let (mut x0, mut y0, mut x1, mut y1) = (door[0], door[1], door[0] + 2, door[1] + 2);
    for p in design.blocks.keys().filter(|p| p[2] == z) {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let (x0, y0, x1, y1) = (x0 - 1, y0 - 1, x1 + 1, y1 + 1);
    let pistons = design.blocks.values().filter(|s| s.contains("piston")).count();
    let below: Vec<(Pos, String)> = design
        .blocks
        .iter()
        .filter(|(p, s)| p[1] < door[1] && s.contains("piston") && !s.contains("piston_head"))
        .map(|(p, s)| (*p, s.clone()))
        .collect();
    let title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let frame = frame_cells(door);
    let count = |get: &dyn Fn(BlockPos) -> Block| -> (usize, Vec<Pos>) {
        let empty: Vec<Pos> = frame.iter().copied().filter(|p| !filled(&get(bp(*p)))).collect();
        (12 - empty.len(), empty)
    };
    if schedule.is_none() && design.ports.contains_key("lever") {
        // A real door: flip its lever in redsim (cosearch's recorder).
        let rs = RsDoor::from_design(&design).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let rec = crate::cosearch::record(&rs).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let air = convert_state("minecraft:air").map_err(|e| anyhow::anyhow!(e))?;
        let rest_get = |p: BlockPos| rec.rest.get(p.arr()).unwrap_or(air);
        let open_snap = rec.open_state();
        let open_get = |p: BlockPos| open_snap.get(p.arr()).unwrap_or(air);
        let rest = plane(&rest_get, z, x0, y0, x1, y1, door, false);
        let open_lines = plane(&open_get, z, x0, y0, x1, y1, door, true);
        let (n, empty) = count(&open_get);
        let fill = if fill {
            Some(fill_design(&design, &rest_get, &empty, &title, fill_dir))
        } else {
            None
        };
        return Ok(Drawn { title, pistons, below, rest, open: Some(Ok((open_lines, n, empty))), fill });
    }
    let spec = DoorSpec { door, lever: [0, 0, 0], along_z: false };
    let mut blocks: Vec<(Pos, Block)> = Vec::new();
    for (p, s) in &design.blocks {
        blocks.push((*p, convert_state(s).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?));
    }
    let (wlo, whi) = watch_box(&blocks, &spec);
    let mut w = World::new(BlockPos::new(wlo[0] - 4, wlo[1] - 4, wlo[2] - 4), [whi[0] - wlo[0] + 9, whi[1] - wlo[1] + 9, whi[2] - wlo[2] + 9]);
    let list: Vec<(BlockPos, Block)> = blocks.iter().map(|(p, b)| (bp(*p), *b)).collect();
    w.place_like_judge(&list);
    step_until_idle(&mut w, 300, |_| {});
    let rest = plane(&|p| w.get(p), z, x0, y0, x1, y1, door, false);
    let open = schedule.as_ref().map(|s| -> std::result::Result<(Vec<String>, usize, Vec<Pos>), String> {
        replay(&mut w, s, true)?;
        let open_ok = spec.door_cells().iter().all(|c| w.get(bp(*c)).is_air());
        let mut lines = plane(&|p| w.get(p), z, x0, y0, x1, y1, door, true);
        if !open_ok {
            lines.push("    (opening NOT clear)".into());
        }
        let (n, empty) = count(&|p| w.get(p));
        Ok((lines, n, empty))
    });
    let fill = match (&schedule, &open, fill) {
        (Some(s), Some(Ok((_, _, empty))), true) => Some(fill_test(&design, s, &text, empty, &title, fill_dir)),
        _ => None,
    };
    Ok(Drawn { title, pistons, below, rest, open, fill })
}

/// CLI entry: draw every file, print (or write) one markdown page.
pub fn cli(files: &[PathBuf], out: Option<&Path>, heading: Option<&str>, fill: bool, fill_dir: Option<&Path>, single_up_below: bool) -> Result<()> {
    let mut drawn: Vec<(PathBuf, Result<Drawn>)> = crate::cosearch::par_map(files, 16, |p| (p.clone(), draw(p, fill, fill_dir)));
    if single_up_below {
        // Keep the mechanisms whose only piston below the opening is one sticky piston facing up.
        drawn.retain(|(_, d)| {
            d.as_ref().is_ok_and(|d| d.below.len() == 1 && d.below[0].1.contains("sticky_piston") && d.below[0].1.contains("facing=up"))
        });
    }
    let mut s = String::new();
    if let Some(h) = heading {
        s.push_str(&format!("# {h}\n\n"));
    }
    let full: Vec<&str> = drawn.iter().filter_map(|(_, d)| d.as_ref().ok()).filter(|d| matches!(&d.open, Some(Ok((_, 12, _))))).map(|d| d.title.as_str()).collect();
    let counted = drawn.iter().filter(|(_, d)| matches!(d, Ok(Drawn { open: Some(Ok(_)), .. }))).count();
    s.push_str(&format!(
        "FRAME FULL WHEN OPEN (all 12 cells around the opening hold full blocks): {} of {} drawn{}\n\n",
        full.len(),
        counted,
        if full.is_empty() { String::new() } else { format!(": {}", full.join(", ")) }
    ));
    if fill {
        let ok: Vec<&str> = drawn
            .iter()
            .filter_map(|(_, d)| d.as_ref().ok())
            .filter(|d| d.fill.as_deref().is_some_and(|f| f.starts_with("OK") || f.starts_with("already")))
            .map(|d| d.title.as_str())
            .collect();
        s.push_str(&format!(
            "STONE FILL (static stone in the frame cells left empty when open; 2 cycles replayed in redsim): {} work{}\n\n",
            ok.len(),
            if ok.is_empty() { String::new() } else { format!(": {}", ok.join(", ")) }
        ));
    }
    s.push_str(
        "Glyphs: `·` air, `#` door/stone block, `X` obsidian, piston bases by facing (sticky `U D E W`, normal `u d e w`), \
         heads `^ v > <`, `R` redstone block (ideal input), `o` observer, `q` repeater, `t` torch, `+` dust, `%` other. \
         `^^^` marks the opening's columns. Left: at rest. Right: open (replayed in redsim); `!` = an EMPTY frame cell when open.\n\n",
    );
    for (p, d) in &drawn {
        match d {
            Err(e) => s.push_str(&format!("### {}\n\nnot drawn: {e:#}\n\n", p.display())),
            Ok(d) => {
                let frame_note = match &d.open {
                    Some(Ok((_, n, _))) => format!(", frame {n}/12 when open"),
                    _ => String::new(),
                };
                let fill_note = d.fill.as_deref().map(|f| format!("\n\nstone fill: {f}")).unwrap_or_default();
                s.push_str(&format!("### {} ({} pistons{frame_note}){fill_note}\n\n```\n", d.title, d.pistons));
                let width = d.rest.iter().map(|l| l.chars().count()).max().unwrap_or(0);
                match &d.open {
                    None => {
                        for l in &d.rest {
                            s.push_str(&format!("{l}\n"));
                        }
                    }
                    Some(Err(e)) => {
                        for l in &d.rest {
                            s.push_str(&format!("{l}\n"));
                        }
                        s.push_str(&format!("open: not drawn ({e})\n"));
                    }
                    Some(Ok((open, _, _))) => {
                        s.push_str(&format!("{:<width$}    {}\n", "rest", "open"));
                        let n = d.rest.len().max(open.len());
                        for k in 0..n {
                            let a = d.rest.get(k).map(String::as_str).unwrap_or("");
                            let b = open.get(k).map(String::as_str).unwrap_or("");
                            let pad = width.saturating_sub(a.chars().count());
                            s.push_str(&format!("{a}{}    {b}\n", " ".repeat(pad)));
                        }
                    }
                }
                s.push_str("```\n\n");
            }
        }
    }
    match out {
        Some(o) => std::fs::write(o, &s)?,
        None => print!("{s}"),
    }
    Ok(())
}
