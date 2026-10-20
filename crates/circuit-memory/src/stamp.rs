//! Tunable groups for the counters (coordinator 2026-10-20: "the catalog shows counters as one
//! tunable card per mechanism"): the `group:` / `group title:` / `group note:` headers the ring
//! generator stamps, and `stamp-groups` to stamp designs already written (by their name).

use anyhow::Result;
use std::path::PathBuf;

/// One counter mechanism: its group name, card title and note.
pub struct Group {
    pub name: &'static str,
    pub title: &'static str,
    pub note: &'static str,
}

pub const BULB_CHAIN: Group = Group {
    name: "bulb-chain",
    title: "Copper bulb chain (counter 2^K)",
    note: "K copper bulb T flip-flops in a line, each read by a comparator that toggles the next bulb: 3K blocks",
};

pub const DROPPER_RING: Group = Group {
    name: "dropper-ring",
    title: "Dropper ring (counter N)",
    note: "N droppers the input fires (directly, through dust, dust relays or repeater relays) with N hoppers between them; one stone advances one dropper per pulse and a comparator reads one dropper (`mctas circuit memory ring --n N`)",
};

pub const RING_BULBS: Group = Group {
    name: "ring-bulbs",
    title: "Dropper ring + copper bulbs (counter N x 2^K)",
    note: "a dropper ring whose comparator toggles K copper bulb T flip-flops in a line (`mctas circuit memory ring --n N --bulbs K`)",
};

pub const RING_CHAIN: Group = Group {
    name: "ring-chain",
    title: "Chained dropper rings (counter N1 x N2 x ...)",
    note: "each ring's comparator strongly powers a stone whose neighbours are the next ring's droppers, so each ring advances once per lap of the previous one; copper bulbs may follow (`mctas circuit memory ring --n N1 --then N2,N3 [--bulbs K]`)",
};

pub const RING_DIVIDER: Group = Group {
    name: "ring-divider",
    title: "Dropper ring + one bulb as a divider (output toggles every N)",
    note: "the ring's comparator lights once per lap and toggles a copper bulb T flip-flop; the bulb's comparator changes state once every N pulses (`mctas circuit memory ring --n N --bulbs 1 --divider`)",
};

/// The mechanism of a counter design from its name (`counter<N>-...`), if it is one of ours.
pub fn classify(name: &str) -> Option<&'static Group> {
    let rest = name.strip_prefix("counter")?;
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_digit());
    let parts: Vec<&str> = rest.split('-').filter(|s| !s.is_empty()).collect();
    let rings = parts.iter().filter(|p| p.starts_with("ring") || **p == "corner" || **p == "star").count();
    let bulbs = parts.iter().any(|p| p.starts_with("bulb"));
    if parts.last() == Some(&"div") {
        return Some(&RING_DIVIDER);
    }
    match (rings, bulbs) {
        (0, true) => Some(&BULB_CHAIN),
        (0, false) => None,
        (1, false) => Some(&DROPPER_RING),
        (1, true) => Some(&RING_BULBS),
        _ => Some(&RING_CHAIN),
    }
}

/// The design text with the group headers set right after its `circuit:` line (older group
/// headers dropped).
pub fn stamp(text: &str, g: &Group) -> String {
    let headers = [format!("group: {}", g.name), format!("group title: {}", g.title), format!("group note: {}", g.note)];
    let mut out: Vec<String> = Vec::new();
    let mut done = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("group:") || t.starts_with("group title:") || t.starts_with("group note:") {
            continue;
        }
        out.push(line.to_string());
        if !done && t.starts_with("circuit:") {
            out.extend(headers.iter().cloned());
            done = true;
        }
    }
    if !done {
        let at = out.iter().position(|l| l.starts_with("name:")).map(|i| i + 1).unwrap_or(0);
        for (j, h) in headers.iter().enumerate() {
            out.insert(at + j, h.clone());
        }
    }
    out.join("\n") + "\n"
}

/// `stamp-groups <files>`: stamp every counter design whose name names a known mechanism.
pub fn run(files: &[PathBuf], dry_run: bool) -> Result<()> {
    for f in files {
        let text = std::fs::read_to_string(f)?;
        let name = text
            .lines()
            .find_map(|l| l.strip_prefix("name:").map(|s| s.trim().to_string()))
            .unwrap_or_else(|| f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
        match classify(&name) {
            Some(g) => {
                if !dry_run {
                    std::fs::write(f, stamp(&text, g))?;
                }
                println!("{}: {}", f.display(), g.name);
            }
            None => println!("{}: no group", f.display()),
        }
    }
    Ok(())
}
