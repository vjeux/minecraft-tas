//! `mctas circuit display chests <design> --out F` (clk-obs, 2026-10-21): every constant hopper (a
//! hopper with no container above or below it, so its items never move) becomes a single chest
//! giving the same comparator signal. Only hoppers pick up item entities, so a chest has no
//! intake for the judge's item intrusion check (clock::intrusion::intakes: an open hopper top a
//! dropped item reaches), at no block cost: the analog decoders' window constants were hoppers
//! (rings7 fails the intrusion check on 40 open tops; sealing them costs 40 glass).
//!
//! A container's signal: floor(14 f) + 1 for f > 0, f = the slots' fill (count / max stack) over
//! the slot count (`AbstractContainerMenu.getRedstoneSignalFromContainer`); a hopper has 5 slots,
//! a single chest 27. The chest gets cobblestone one item past its signal's lower bound (or 27
//! full stacks for 15), clear of float rounding at the bound.

use anyhow::{Context, Result, bail};
use mctas::design::Design;
use std::path::Path;

fn id(state: &str) -> &str {
    state.split('[').next().unwrap_or(state).trim_start_matches("minecraft:")
}

fn is_container(state: &str) -> bool {
    let i = id(state);
    ["hopper", "chest", "trapped_chest", "barrel", "dropper", "dispenser", "crafter", "composter"].contains(&i) || i.ends_with("shulker_box")
}

/// (item id, count) of every `Items` entry of a block entity's SNBT.
fn items(snbt: &str) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    for part in snbt.split('{').skip(1) {
        let mut name = None;
        let mut count = None;
        for kv in part.split([',', '}']) {
            if let Some(v) = kv.trim().strip_prefix("id:") {
                name = Some(v.trim_matches('"').trim_start_matches("minecraft:").to_string());
            }
            if let Some(v) = kv.trim().strip_prefix("count:") {
                count = v.trim_end_matches('b').parse::<u32>().ok();
            }
        }
        if let (Some(n), Some(c)) = (name, count) {
            out.push((n, c));
        }
    }
    out
}

/// The comparator signal of `slots` slots holding these stacks (every item here stacks to 64).
fn signal(items: &[(String, u32)], slots: u32) -> Result<u32> {
    for (n, _) in items {
        if !["cobblestone", "stone", "redstone", "glass", "sand"].contains(&n.as_str()) {
            bail!("item {n}: only 64-stacking items are converted");
        }
    }
    let total: u32 = items.iter().map(|(_, c)| *c).sum();
    Ok(if total == 0 { 0 } else { (14 * total) / (64 * slots) + 1 })
}

/// The chest contents for a signal (27 slots of 64).
fn chest_items(level: u32) -> String {
    let cap = 27 * 64;
    let n = if level >= 15 { cap } else if level == 0 { 0 } else { ((cap * (level - 1)).div_ceil(14) + 1).min(cap) };
    let mut parts = Vec::new();
    let mut left = n;
    let mut slot = 0;
    while left > 0 {
        let c = left.min(64);
        parts.push(format!("{{Slot:{slot}b,id:\"minecraft:cobblestone\",count:{c}}}"));
        left -= c;
        slot += 1;
    }
    format!("{{Items:[{}]}}", parts.join(","))
}

pub fn run(design: &Path, out: &Path) -> Result<()> {
    let text = std::fs::read_to_string(design).with_context(|| format!("reading {}", design.display()))?;
    let mut d = Design::parse(&text)?;
    // only the hoppers the judge's item intrusion check reaches (the cell above them an intake):
    // the others stay (a hopper is no redstone conductor and has a sturdy top)
    let intakes = circuit::intrusion::intakes(&circuit::Circuit::load(design)?);
    let hoppers: Vec<[i32; 3]> = d.blocks.iter().filter(|(p, s)| id(s) == "hopper" && intakes.contains(&[p[0], p[1] + 1, p[2]])).map(|(p, _)| *p).collect();
    let mut n = 0;
    let mut shulkers = 0;
    let mut kept: Vec<[i32; 3]> = Vec::new();
    for p in hoppers {
        for dy in [1, -1] {
            if let Some(s) = d.blocks.get(&[p[0], p[1] + dy, p[2]])
                && is_container(s)
            {
                bail!("hopper at {p:?}: a container ({s}) {} it: its items move, not a constant", if dy > 0 { "above" } else { "below" });
            }
        }
        let st = &d.blocks[&p];
        if !st.contains("facing=down") {
            bail!("hopper at {p:?} ({st}) does not face down: it may push into a container beside it");
        }
        let nbt = d.block_entities.get(&p).cloned().unwrap_or_default();
        let level = signal(&items(&nbt), 5)?;
        let check = signal(&items(&chest_items(level)), 27)?;
        if check != level {
            bail!("hopper at {p:?}: signal {level}, its chest would give {check}");
        }
        // a block standing on the hopper (the ring above's bridge or copy comparator, a lane
        // repeater) needs a sturdy top: a shulker box (27 slots, a full block); else a chest (no
        // redstone conductor). A shulker box is a conductor: one a torch below or a diode facing
        // in would power strongly (it would then power the dust beside it) stays a hopper
        let above = d.blocks.get(&[p[0], p[1] + 1, p[2]]).cloned();
        let strong = d.blocks.get(&[p[0], p[1] - 1, p[2]]).is_some_and(|s| id(s).contains("redstone_torch") || id(s).contains("redstone_wall_torch"))
            // a diode's blockstate `facing` points at its input: one at p + dd outputs into p when
            // it faces along dd
            || [([1, 0, 0], "east"), ([-1, 0, 0], "west"), ([0, 0, 1], "south"), ([0, 0, -1], "north")].iter().any(|(dd, facing)| {
                d.blocks.get(&[p[0] + dd[0], p[1], p[2] + dd[2]]).is_some_and(|s| (id(s) == "repeater" || id(s) == "comparator") && s.contains(&format!("facing={facing}")))
            });
        if above.is_some() && strong {
            kept.push(p);
            continue;
        }
        let state = if above.is_some() { "minecraft:shulker_box[facing=up]" } else { "minecraft:chest[facing=north,type=single,waterlogged=false]" };
        if above.is_some() {
            shulkers += 1;
        }
        d.blocks.insert(p, state.into());
        d.block_entities.insert(p, chest_items(level));
        n += 1;
    }
    let head: String = text.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    std::fs::write(out, format!("{head}# constants in chests and shulker boxes (clk-obs `display chests`: {n} hoppers, same signals; neither takes a dropped item)\n{}", d.to_text()))?;
    println!("{}: {n} hoppers -> {} chests + {shulkers} shulker boxes (a block on top), {} blocks; kept as hoppers (a shulker box there would be powered): {kept:?}", out.display(), n - shulkers, d.blocks.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels() {
        for level in 0..=15 {
            assert_eq!(signal(&items(&chest_items(level)), 27).unwrap(), level, "level {level}");
        }
        // the builder's hopper for 10: 206 cobblestone in 5 slots
        let h = "{Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:64},{Slot:1b,id:\"minecraft:cobblestone\",count:64},{Slot:2b,id:\"minecraft:cobblestone\",count:64},{Slot:3b,id:\"minecraft:cobblestone\",count:14}]}";
        assert_eq!(signal(&items(h), 5).unwrap(), 10);
    }
}
