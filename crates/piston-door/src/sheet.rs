//! `mctas door sheet <file>...`: one markdown page that draws mechanisms by eye. Each file is an
//! ideal schedule (`door ideal` format) or a design; the page shows the plane of the door at rest
//! and, for a schedule, once its opening has settled, side by side. The opening is replayed in
//! redsim (the exact model, milliseconds per mechanism), so a whole mechanism list draws in a
//! second; no Minecraft is started.
//!
//! Glyphs: `·` air, `#` full block (door blocks, stone), `X` obsidian, piston bases by facing
//! (sticky upper case `U D E W`, normal lower case `u d e w`), heads `^ v > <`, `~` moving block,
//! `R` redstone block (an ideal input when the schedule sets it), `o` observer, `q` repeater,
//! `c` comparator, `t` torch, `+` dust, `l` lever, `%` anything else. `^^^` under a plane marks
//! the opening's columns.

use crate::DoorSpec;
use crate::cosearch::{bp, convert_state, step_until_idle, watch_box};
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

struct Drawn {
    title: String,
    pistons: usize,
    rest: Vec<String>,
    open: Option<std::result::Result<Vec<String>, String>>,
}

fn plane(w: &World, z: i32, x0: i32, y0: i32, x1: i32, y1: i32, door_x: i32) -> Vec<String> {
    let mut out = Vec::new();
    for y in (y0..=y1).rev() {
        let mut line = format!("{y:>3} ");
        for x in x0..=x1 {
            line.push(glyph(&w.get(BlockPos::new(x, y, z)).to_string()));
        }
        out.push(line);
    }
    let mut marks = String::from("    ");
    for x in x0..=x1 {
        marks.push(if (door_x..door_x + 3).contains(&x) { '^' } else { ' ' });
    }
    out.push(marks);
    out
}

fn draw(path: &Path) -> Result<Drawn> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let is_schedule = text.lines().any(|l| l.trim_start().starts_with("design "));
    let (design, schedule): (Design, Option<Schedule>) = if is_schedule {
        let s = Schedule::parse(&text).with_context(|| path.display().to_string())?;
        (s.design().clone(), Some(s))
    } else {
        (crate::cli::load_design(path)?, None)
    };
    let door = *design.ports.get("door").context("design has no door port")?;
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
    // The plane: the design's extent at the door's z, one cell of margin.
    let z = door[2];
    let (mut x0, mut y0, mut x1, mut y1) = (door[0], door[1], door[0] + 2, door[1] + 2);
    for p in design.blocks.keys().filter(|p| p[2] == z) {
        x0 = x0.min(p[0]);
        y0 = y0.min(p[1]);
        x1 = x1.max(p[0]);
        y1 = y1.max(p[1]);
    }
    let (x0, y0, x1, y1) = (x0 - 1, y0 - 1, x1 + 1, y1 + 1);
    let rest = plane(&w, z, x0, y0, x1, y1, door[0]);
    let pistons = design.blocks.values().filter(|s| s.contains("piston")).count();
    let open = schedule.map(|s| -> std::result::Result<Vec<String>, String> {
        let evs = s.events_of(true, s.vars()).map_err(|e| format!("{e:#}"))?;
        let mut t = 0u32;
        let mut i = 0;
        while i < evs.len() {
            while i < evs.len() && evs[i].0 == t {
                let (_, p, st) = &evs[i];
                if st.as_deref() != Some("@use") {
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
        let (_, idle) = step_until_idle(&mut w, 300, |_| {});
        if !idle {
            return Err("still moving 300 ticks after the last event".into());
        }
        let open_ok = spec.door_cells().iter().all(|c| w.get(bp(*c)).to_string().ends_with("air"));
        let mut lines = plane(&w, z, x0, y0, x1, y1, door[0]);
        if !open_ok {
            lines.push("    (opening NOT clear)".into());
        }
        Ok(lines)
    });
    let title = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    Ok(Drawn { title, pistons, rest, open })
}

/// CLI entry: draw every file, print (or write) one markdown page.
pub fn cli(files: &[PathBuf], out: Option<&Path>, heading: Option<&str>) -> Result<()> {
    let drawn: Vec<(PathBuf, Result<Drawn>)> = crate::cosearch::par_map(files, 16, |p| (p.clone(), draw(p)));
    let mut s = String::new();
    if let Some(h) = heading {
        s.push_str(&format!("# {h}\n\n"));
    }
    s.push_str(
        "Glyphs: `·` air, `#` door/stone block, `X` obsidian, piston bases by facing (sticky `U D E W`, normal `u d e w`), \
         heads `^ v > <`, `R` redstone block (ideal input), `o` observer, `q` repeater, `t` torch, `+` dust, `%` other. \
         `^^^` marks the opening's columns. Left: at rest. Right: open (the schedule replayed in redsim).\n\n",
    );
    for (p, d) in &drawn {
        match d {
            Err(e) => s.push_str(&format!("### {}\n\nnot drawn: {e:#}\n\n", p.display())),
            Ok(d) => {
                s.push_str(&format!("### {} ({} pistons)\n\n```\n", d.title, d.pistons));
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
                    Some(Ok(open)) => {
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
