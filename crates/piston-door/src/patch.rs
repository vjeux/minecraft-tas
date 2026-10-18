//! `mctas door patch`: small edit scripts over a design (set/clear blocks, move ports), so a
//! variant of a design is a few readable lines instead of a hand-edited layer file.

use anyhow::{Context, Result, bail};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::Pos;

fn head_offset(state: &str) -> Option<(Pos, String)> {
    let b = BlockState::parse(state).ok()?;
    let name = b.short_name();
    if (name != "piston" && name != "sticky_piston") || b.get("extended") != Some("true") {
        return None;
    }
    let facing = b.get("facing").unwrap_or("north").to_string();
    let off = match facing.as_str() {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        _ => [0, -1, 0],
    };
    let ty = if name == "sticky_piston" { "sticky" } else { "normal" };
    Some((off, format!("minecraft:piston_head[facing={facing},short=false,type={ty}]")))
}

/// Apply an edit script (see the `door patch` help) to a design.
pub fn apply(d: &Design, script: &str) -> Result<Design> {
    let mut out = d.clone();
    for (lineno, raw) in script.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        let int = |i: usize| -> Result<i32> {
            parts.get(i).with_context(|| format!("line {}: missing number", lineno + 1))?.parse::<i32>().map_err(Into::into)
        };
        let pos = |i: usize| -> Result<Pos> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
        match parts[0] {
            "set" => {
                let p = pos(1)?;
                let state = BlockState::parse(parts.get(4).context("set needs a state")?)?.to_string();
                if let Some(old) = out.blocks.remove(&p) {
                    if let Some((off, _)) = head_offset(&old) {
                        out.blocks.remove(&[p[0] + off[0], p[1] + off[1], p[2] + off[2]]);
                    }
                }
                if let Some((off, head)) = head_offset(&state) {
                    out.blocks.insert([p[0] + off[0], p[1] + off[1], p[2] + off[2]], head);
                }
                out.blocks.insert(p, state);
            }
            "clear" => {
                let p = pos(1)?;
                if let Some(old) = out.blocks.remove(&p) {
                    if let Some((off, _)) = head_offset(&old) {
                        out.blocks.remove(&[p[0] + off[0], p[1] + off[1], p[2] + off[2]]);
                    }
                }
            }
            "port" => {
                let name = parts.get(1).context("port needs a name")?.to_string();
                out.ports.insert(name, [int(2)?, int(3)?, int(4)?]);
            }
            other => bail!("line {}: unknown patch command {other}", lineno + 1),
        }
    }
    Ok(out)
}
