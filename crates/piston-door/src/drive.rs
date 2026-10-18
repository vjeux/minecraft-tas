//! `mctas door drive <script>...`: drive a design by hand (place/remove blocks, use, step) and
//! print the z=0 plane as ASCII after every step. For debugging mechanisms before wiring them,
//! and for proving gadgets (`probe` + `trace` + `expect`, see designs/gadgets/).
//!
//! Script lines: `design <path|@name>`, `plane <z> <x0> <y0> <x1> <y1>`, `planex <x> <z0> <y0> <z1> <y1>`
//! (side view), `set x y z <state>`
//! (update mode, like a player), `clear x y z`, `use x y z`, `step <n>` (prints each tick),
//! `quiet <n>` (steps without printing), `print`.
//!
//! Probes: `probe <name> x y z` watches a block (`probe_signal <name> x y z` watches the best
//! signal the cell's neighbors give into it: what a piston or lamp there would see, shown as its
//! level, 0/+ in `expect_trace`); `trace <n>` steps n ticks printing one line per
//! tick with every probe's compact value (see [`probe_value`]: dust power, 1/0 for powered or lit
//! components, E/r for extended/retracted pistons, ~ moving, . air, # other solid);
//! `expect <name> <value>` checks a probe now; `expect_state x y z <text>` checks that the block
//! state contains `<text>`; `expect_trace <name> <values>` steps one tick per character of
//! `<values>` and checks the probe after each (one character per tick: dust 0 or +, others their
//! probe value, `?` = any).
//! A failed expectation makes the command exit non-zero.
//!
//! Change log: `box x0 y0 z0 x1 y1 z1` sets a 3D watch box, `changes <n>` steps n ticks printing
//! every block change inside it per tick, `mark <label>` prints a label and re-baselines the log.

use anyhow::{Context, Result, bail};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, Worker};

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
        "observer" => {
            if b.get("powered") == Some("true") { 'O' } else { 'o' }
        }
        "repeater" => {
            if b.get("powered") == Some("true") { 'Q' } else { 'q' }
        }
        "comparator" => {
            if b.get("powered") == Some("true") { 'C' } else { 'c' }
        }
        "redstone_block" => 'R',
        "redstone_torch" | "redstone_wall_torch" => {
            if b.get("lit") == Some("true") { 'T' } else { 't' }
        }
        "redstone_lamp" | "copper_bulb" | "waxed_copper_bulb" => {
            if b.get("lit") == Some("true") { 'B' } else { 'b' }
        }
        "redstone_wire" => {
            if b.get("power").is_some_and(|p| p != "0") { '+' } else { '.' }
        }
        "lever" => {
            if b.get("powered") == Some("true") { 'L' } else { 'l' }
        }
        "target" => 'x',
        _ => '%',
    }
}

/// Compact value of a block for probes: dust power (0-15), 1/0 for powered/lit components,
/// E/r for extended/retracted pistons, h piston head, ~ moving block, . air, # anything else.
pub fn probe_value(state: &str) -> String {
    let Ok(b) = BlockState::parse(state) else { return "?".into() };
    let flag = |k: &str| if b.get(k) == Some("true") { "1" } else { "0" };
    match b.short_name() {
        "air" => ".".into(),
        "redstone_wire" => b.get("power").unwrap_or("0").to_string(),
        "sticky_piston" | "piston" => (if b.get("extended") == Some("true") { "E" } else { "r" }).into(),
        "piston_head" => "h".into(),
        "moving_piston" => "~".into(),
        "redstone_torch" | "redstone_wall_torch" | "redstone_lamp" => flag("lit").into(),
        n if n.ends_with("copper_bulb") => flag("lit").into(),
        "repeater" | "comparator" | "observer" | "lever" | "stone_button" | "oak_button" => flag("powered").into(),
        n if n.ends_with("_rail") || n.ends_with("trapdoor") || n.ends_with("door") || n == "note_block" => flag("powered").into(),
        n if n.ends_with("fence_gate") || n.ends_with("_skull") || n.ends_with("_head") || n.ends_with("_shelf") => flag("powered").into(),
        "dispenser" | "dropper" | "crafter" => flag("triggered").into(),
        "scaffolding" => b.get("distance").unwrap_or("?").to_string(),
        "target" | "sculk_sensor" | "calibrated_sculk_sensor" | "daylight_detector" => b.get("power").unwrap_or("0").to_string(),
        "slime_block" => "s".into(),
        "honey_block" => "y".into(),
        "redstone_block" => "R".into(),
        _ => "#".into(),
    }
}

pub(crate) fn render(r: &Region, z: i32, x0: i32, y0: i32, x1: i32, y1: i32) -> String {
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

/// Side view of the plane `x`, z from left to right (same cell characters as [`render`]).
pub(crate) fn render_x(r: &Region, x: i32, z0: i32, y0: i32, z1: i32, y1: i32) -> String {
    let mut out = String::new();
    for y in (y0..=y1).rev() {
        out.push_str(&format!("{y:>4} "));
        for z in z0..=z1 {
            out.push(r.get([x, y, z]).map(cell).unwrap_or('?'));
        }
        out.push('\n');
    }
    out
}

/// Outcome of one script: how many expectations ran and which failed.
#[derive(Default, Debug)]
pub struct DriveOutcome {
    pub checks: u32,
    pub failures: Vec<String>,
}

/// Run a script; `verbose` prints planes and traces (otherwise only failures are reported).
pub fn run(w: &mut Worker, script: &str) -> Result<DriveOutcome> {
    run_opts(w, script, true)
}

pub fn run_opts(w: &mut Worker, script: &str, verbose: bool) -> Result<DriveOutcome> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = run_in(w, sb, script, verbose, None);
    let _ = w.drop_sandbox(sb);
    r
}

/// Run a script silently and return every cell it sweeps: each cell that is non-air at any tick
/// (within the design's box + 3), plus every neighbor of a slime/honey block or a moving piston
/// at any tick. `door route --swept-script` uses it to wire a mechanism that so far only works
/// with ideal (scripted) inputs.
pub fn swept(w: &mut Worker, script: &str) -> Result<std::collections::HashSet<Pos>> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let mut cells = std::collections::HashSet::new();
    let r = run_in(w, sb, script, false, Some(&mut cells));
    let _ = w.drop_sandbox(sb);
    r?;
    Ok(cells)
}

fn collect_cells(w: &mut Worker, sb: u32, bx: Option<(Pos, Pos)>, out: &mut std::collections::HashSet<Pos>) -> Result<()> {
    let Some((lo, hi)) = bx else { return Ok(()) };
    let r = w.get_region(sb, lo, hi)?;
    for (p, s) in r.iter() {
        if mctas::blockstate::is_air(s) {
            continue;
        }
        out.insert(p);
        if s.starts_with("minecraft:slime_block") || s.starts_with("minecraft:honey_block") || s.contains("moving_piston") {
            for f in [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]] {
                out.insert([p[0] + f[0], p[1] + f[1], p[2] + f[2]]);
            }
        }
    }
    Ok(())
}

fn run_in(w: &mut Worker, sb: u32, script: &str, verbose: bool, mut collect: Option<&mut std::collections::HashSet<Pos>>) -> Result<DriveOutcome> {
    let mut bx: Option<(Pos, Pos)> = None;
    let mut plane = (0, -6, -5, 8, 9);
    // `planex <x> <z0> <y0> <z1> <y1>`: show a side view (an x plane, z left to right) instead.
    let mut plane_x: Option<(i32, i32, i32, i32, i32)> = None;
    let mut tick = 0u32;
    let mut watch: Option<(Pos, Pos)> = None;
    let mut prev: Option<Region> = None;
    let mut probes: Vec<(String, Pos, bool)> = Vec::new();
    let mut out = DriveOutcome::default();
    let show = |w: &mut Worker, label: &str, plane: (i32, i32, i32, i32, i32), plane_x: Option<(i32, i32, i32, i32, i32)>| -> Result<()> {
        if let Some((x, z0, y0, z1, y1)) = plane_x {
            let r = w.get_region(sb, [x, y0, z0], [x, y1, z1])?;
            println!("-- {label} (x={x}, z {z0}..{z1})\n{}", render_x(&r, x, z0, y0, z1, y1));
            return Ok(());
        }
        let (z, x0, y0, x1, y1) = plane;
        let r = w.get_region(sb, [x0, y0, z], [x1, y1, z])?;
        println!("-- {label}\n{}", render(&r, z, x0, y0, x1, y1));
        Ok(())
    };
    let read = |w: &mut Worker, p: Pos| -> Result<String> {
        let r = w.get_region(sb, p, p)?;
        Ok(r.get(p).unwrap_or("?").to_string())
    };
    // A probe's value: the block's probe value, or for a `probe_signal` the best signal the
    // cell's neighbors give into it (0-15: what a piston or lamp there would see).
    let value = |w: &mut Worker, p: Pos, signal: bool| -> Result<String> {
        if signal {
            Ok(w.signals(sb, &[p])?.first().map(|s| s.0).unwrap_or(0).to_string())
        } else {
            Ok(probe_value(&read(w, p)?))
        }
    };
    let trace_line = |w: &mut Worker, probes: &[(String, Pos, bool)], tick: u32| -> Result<String> {
        let mut line = format!("t={tick:<4}");
        for (name, p, signal) in probes {
            line.push_str(&format!(" {name}={}", value(w, *p, *signal)?));
        }
        Ok(line)
    };
    for (lineno, raw) in script.lines().enumerate() {
        // Comments: a line starting with '#', or " # " followed by text ('#' alone is a value).
        let line = raw.trim();
        let line = if line.starts_with('#') { "" } else { line.split(" # ").next().unwrap().trim() };
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> {
            parts.get(i).with_context(|| format!("line {}: missing number", lineno + 1))?.parse::<i32>().map_err(Into::into)
        };
        let pos = |i: usize| -> Result<Pos> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
        match parts[0] {
            "design" => {
                let d: Design = crate::cli::load_design(std::path::Path::new(parts[1]))?;
                // Optional world offset (`design <path> dx dy dz`): reproduce position-dependent
                // behavior seen by the verifier; script coordinates stay design-relative only if
                // you add the offset yourself.
                let d = if parts.len() >= 5 { d.translated([int(2)?, int(3)?, int(4)?]) } else { d };
                crate::video::place(w, sb, &d)?;
                if let Some((lo, hi)) = d.bounds() {
                    bx = Some(([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]));
                }
                if let Some(c) = collect.as_deref_mut() {
                    collect_cells(w, sb, bx, c)?;
                }
            }
            "plane" => {
                plane = (int(1)?, int(2)?, int(3)?, int(4)?, int(5)?);
                plane_x = None;
            }
            "planex" => plane_x = Some((int(1)?, int(2)?, int(3)?, int(4)?, int(5)?)),
            "set" => {
                let state = parts.get(4).context("set needs a state")?;
                w.set_blocks(sb, &[(pos(1)?, state.to_string())], PlaceMode::Update)?;
            }
            "clear" => {
                w.set_blocks(sb, &[(pos(1)?, "minecraft:air".to_string())], PlaceMode::Update)?;
            }
            // `setblock x y z <state>`: the game's own `/setblock` command (shape updates, then
            // neighbor updates: `SetBlockCommand.setBlock`), so `door vanilla-gadget` can replay the
            // very same call on the official server. Use it for ideal inputs in gadget scripts.
            "setblock" => {
                let state = parts.get(4).context("setblock needs a state")?;
                let p = pos(1)?;
                w.exec(sb, &format!("setblock {} {} {} {state}", p[0], p[1], p[2]))?;
            }
            "use" => {
                w.use_block(sb, pos(1)?)?;
            }
            "step" => {
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    if let Some(c) = collect.as_deref_mut() {
                        collect_cells(w, sb, bx, c)?;
                    }
                    if verbose {
                        show(w, &format!("t={tick}"), plane, plane_x)?;
                    }
                }
            }
            "quiet" => {
                let n = int(1)? as u32;
                if let Some(c) = collect.as_deref_mut() {
                    for _ in 0..n {
                        w.step(sb, 1)?;
                        collect_cells(w, sb, bx, c)?;
                    }
                } else {
                    w.step(sb, n)?;
                }
                tick += n;
            }
            // `settle [max]`: step until nothing is pending (like the verifier between flips).
            "settle" => {
                let max = if parts.len() > 1 { int(1)? as u32 } else { 400 };
                let r = w.step_until_idle(sb, max)?;
                tick += r.ticked;
                if verbose {
                    println!("-- settled after {} ticks (t={tick})", r.ticked);
                }
            }
            "print" => {
                if verbose {
                    show(w, &format!("t={tick}"), plane, plane_x)?
                }
            }
            "probe" | "probe_signal" => {
                let name = parts.get(1).context("probe needs a name")?.to_string();
                probes.push((name, pos(2)?, parts[0] == "probe_signal"));
            }
            "trace" => {
                if verbose {
                    println!("{}", trace_line(w, &probes, tick)?);
                }
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    if verbose {
                        println!("{}", trace_line(w, &probes, tick)?);
                    }
                }
            }
            "expect" => {
                let name = parts.get(1).context("expect needs a probe name")?;
                let want = parts.get(2).context("expect needs a value")?;
                let (_, p, signal) = probes.iter().find(|(n, _, _)| n == name).with_context(|| format!("no probe {name}"))?.clone();
                let got = value(w, p, signal)?;
                out.checks += 1;
                if got != *want {
                    out.failures.push(format!("line {}: t={tick} expect {name}={want}, got {got}", lineno + 1));
                }
            }
            "expect_state" => {
                let want = parts.get(4).context("expect_state needs text")?;
                let got = read(w, pos(1)?)?;
                out.checks += 1;
                if !got.contains(want) {
                    out.failures.push(format!("line {}: t={tick} expect state containing {want}, got {got}", lineno + 1));
                }
            }
            "expect_trace" => {
                let name = parts.get(1).context("expect_trace needs a probe name")?;
                let want = parts.get(2).context("expect_trace needs values")?;
                let (_, p, signal) = probes.iter().find(|(n, _, _)| n == name).with_context(|| format!("no probe {name}"))?.clone();
                let mut got = String::new();
                for _ in want.chars() {
                    w.step(sb, 1)?;
                    tick += 1;
                    let v = value(w, p, signal)?;
                    // One character per tick: dust and signals are 0 or + (powered), others
                    // their probe value.
                    let c = match v.parse::<u32>() {
                        Ok(n) if signal => if n > 0 { '+' } else { '0' },
                        Ok(n) if v.len() > 1 || n > 1 => '+',
                        _ => v.chars().next().unwrap_or('?'),
                    };
                    got.push(c);
                    if verbose {
                        println!("{}", trace_line(w, &probes, tick)?);
                    }
                }
                out.checks += 1;
                let ok = want.chars().zip(got.chars()).all(|(a, b)| a == '?' || a == b);
                if !ok {
                    out.failures.push(format!("line {}: t={tick} expect_trace {name} {want}, got {got}", lineno + 1));
                }
            }
            "box" => {
                watch = Some((pos(1)?, pos(4)?));
                prev = None;
            }
            "changes" => {
                // Steps n ticks, printing every block change inside the `box` per tick (3D).
                let (lo, hi) = watch.context("`changes` needs a `box x0 y0 z0 x1 y1 z1` first")?;
                if prev.is_none() {
                    prev = Some(w.get_region(sb, lo, hi)?);
                }
                for _ in 0..int(1)? {
                    w.step(sb, 1)?;
                    tick += 1;
                    let now = w.get_region(sb, lo, hi)?;
                    let before = prev.as_ref().unwrap();
                    let short = |s: &str| s.strip_prefix("minecraft:").unwrap_or(s).to_string();
                    let diffs: Vec<String> = now
                        .iter()
                        .filter(|(p, s)| before.get(*p) != Some(*s))
                        .map(|(p, s)| format!("{p:?} {} -> {}", short(before.get(p).unwrap_or("?")), short(s)))
                        .collect();
                    if verbose && !diffs.is_empty() {
                        println!("t={tick:<3} {}", diffs.join("\n      "));
                    }
                    prev = Some(now);
                }
            }
            "mark" => {
                // Re-baseline the change log (hides scripted edits) and print a label.
                if let Some((lo, hi)) = watch {
                    prev = Some(w.get_region(sb, lo, hi)?);
                }
                if verbose {
                    println!("== {} (t={tick})", parts[1..].join(" "));
                }
            }
            other => bail!("line {}: unknown script command {other}", lineno + 1),
        }
    }
    Ok(out)
}
