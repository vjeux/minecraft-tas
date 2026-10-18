//! `mctas door drive <script>`: drive a design by hand (place/remove blocks, use, step) and
//! print the z=0 plane as ASCII after every step. For debugging mechanisms before wiring them.
//!
//! Script lines: `design <path|@name>`, `plane <z> <x0> <y0> <x1> <y1>`, `set x y z <state>`
//! (update mode, like a player), `clear x y z`, `use x y z`, `step <n>` (prints each tick),
//! `quiet <n>` (steps without printing), `print`.

use anyhow::{Context, Result, bail};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{PlaceMode, Region, SandboxOptions, Worker};

fn cell(state: &str) -> char {
    let Ok(b) = BlockState::parse(state) else { return '?' };
    let arrow = |f: Option<&str>| match f {
        Some("up") => '^',
        Some("down") => 'v',
        Some("east") => '>',
        Some("west") => '<',
        Some("north") => 'n',
        Some("south") => 'u',
        _ => '*',
    };
    match b.short_name() {
        "air" => ' ',
        "stone" => '#',
        "slime_block" => 's',
        "honey_block" => 'y',
        "sticky_piston" | "piston" => {
            if b.get("extended") == Some("true") { 'P' } else { 'p' }
        }
        "piston_head" => arrow(b.get("facing")),
        "moving_piston" => '~',
        "observer" => 'O',
        "repeater" => {
            if b.get("powered") == Some("true") { 'Q' } else { 'q' }
        }
        "redstone_block" => 'R',
        "redstone_torch" | "redstone_wall_torch" => {
            if b.get("lit") == Some("true") { 'T' } else { 't' }
        }
        "redstone_wire" => {
            if b.get("power").is_some_and(|p| p != "0") { '+' } else { '.' }
        }
        "lever" => 'L',
        _ => '%',
    }
}

fn render(r: &Region, z: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
    let mut out = String::new();
    for y in (y0..=y1).rev() {
        out.push_str(&format!("{y:>4} "));
        for x in x0..=x1 {
            out.push(r.get([x, y, z]).map(cell).unwrap_or('?'));
        }
        out.push('\n');
    }
    out
}

pub fn run(w: &mut Worker, script: &str) -> Result<()> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut plane = (0, -6, -5, 8, 9);
    let mut tick = 0u32;
    let show = |w: &mut Worker, label: &str, plane: (i32, i32, i32, i32, i32)| -> Result<()> {
        let (z, x0, y0, x1, y1) = plane;
        let r = w.get_region(sb, [x0, y0, z], [x1, y1, z])?;
        println!("-- {label}\n{}", render(&r, z, x0, y0, x1, y1));
        Ok(())
    };
    for line in script.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> { parts.get(i).context("missing number")?.parse::<i32>().map_err(Into::into) };
        match parts[0] {
            "design" => {
                let d: Design = crate::cli::load_design(std::path::Path::new(parts[1]))?;
                crate::video::place(w, sb, &d)?;
            }
            "plane" => plane = (int(1)?, int(2)?, int(3)?, int(4)?, int(5)?),
            "set" => {
                w.set_blocks(sb, &[([int(1)?, int(2)?, int(3)?], parts[4].to_string())], PlaceMode::Update)?;
            }
            "clear" => {
                w.set_blocks(sb, &[([int(1)?, int(2)?, int(3)?], "minecraft:air".to_string())], PlaceMode::Update)?;
            }
            "use" => {
                w.use_block(sb, [int(1)?, int(2)?, int(3)?])?;
            }
            "step" => {
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    show(w, &format!("t={tick}"), plane)?;
                }
            }
            "quiet" => {
                let n = int(1)? as u32;
                w.step(sb, n)?;
                tick += n;
            }
            "print" => show(w, &format!("t={tick}"), plane)?,
            other => bail!("unknown script command {other}"),
        }
    }
    Ok(())
}
