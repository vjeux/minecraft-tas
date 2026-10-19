//! The design text format: a contraption as human-readable layers.
//!
//! ```text
//! # comment
//! name: tiny-demo
//! origin: 0 0 0                  # world position of row 0, column 0 (y adds to the layer y)
//! port lever: 2 1 0              # named positions (inputs, outputs, ...)
//! palette:
//!   S minecraft:stone
//!   P minecraft:sticky_piston[facing=up]
//!   L minecraft:lever[face=floor,facing=east]
//! layer 0:
//!   S.
//! layer 1:
//!   PL
//! ```
//!
//! Rows run north to south (z grows downward), columns west to east (x grows rightward).
//! `.` and spaces are air. Every other character must be in the palette.
//!
//! Entities (minecarts, armor stands, boats...) are header lines, world coordinates as decimals
//! (not shifted by `origin`), with optional SNBT for the entity's own data (rotation, contents):
//!
//! ```text
//! entity: minecraft:minecart 0.5 1.0625 3.5
//! entity: minecraft:armor_stand 2.5 1 0.5 {Rotation:[90f,0f]}
//! ```

use crate::blockstate::BlockState;
use crate::sim::Pos;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::path::Path;

/// An entity of a design: its type (`minecraft:minecart`), its position (the entity's feet, world
/// coordinates as decimals) and optional SNBT data (`{Rotation:[90f,0f]}`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EntitySpec {
    pub kind: String,
    pub pos: [f64; 3],
    pub nbt: Option<String>,
}

// Positions are finite decimals read from text (never NaN), so equality is total.
impl Eq for EntitySpec {}

impl EntitySpec {
    /// `minecraft:minecart 0.5 1.0625 3.5 [{snbt}]` (the text after `entity:`).
    pub fn parse(s: &str) -> Result<EntitySpec> {
        fn next_word(s: &str) -> (&str, &str) {
            let r = s.trim_start();
            let end = r.find(char::is_whitespace).unwrap_or(r.len());
            r.split_at(end)
        }
        let s = s.trim();
        let usage = || anyhow!("expected 'entity: <type> <x> <y> <z> [snbt]', got {s:?}");
        let (kind, mut rest) = next_word(s);
        if kind.is_empty() {
            return Err(usage());
        }
        let mut pos = [0.0f64; 3];
        for p in pos.iter_mut() {
            let (w, r) = next_word(rest);
            rest = r;
            if w.is_empty() {
                return Err(usage());
            }
            *p = w.parse::<f64>().with_context(|| format!("bad entity coordinate {w:?} in {s:?}"))?;
            if !p.is_finite() {
                bail!("bad entity coordinate {w:?} in {s:?}");
            }
        }
        let nbt = rest.trim();
        if !nbt.is_empty() && !nbt.starts_with('{') {
            bail!("entity data must be SNBT in braces, got {nbt:?}");
        }
        let kind = if kind.contains(':') { kind.to_string() } else { format!("minecraft:{kind}") };
        Ok(EntitySpec { kind, pos, nbt: (!nbt.is_empty()).then(|| nbt.to_string()) })
    }

    /// The text after `entity: ` ([`EntitySpec::parse`] reads it back exactly).
    pub fn to_text(&self) -> String {
        let [x, y, z] = self.pos;
        match &self.nbt {
            Some(n) => format!("{} {x} {y} {z} {n}", self.kind),
            None => format!("{} {x} {y} {z}", self.kind),
        }
    }

    /// The block cell the entity's position is in.
    pub fn cell(&self) -> Pos {
        [self.pos[0].floor() as i32, self.pos[1].floor() as i32, self.pos[2].floor() as i32]
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Design {
    pub name: String,
    /// Other `key: value` header lines, in order of keys.
    pub meta: BTreeMap<String, String>,
    pub ports: BTreeMap<String, Pos>,
    /// Non-air blocks.
    pub blocks: BTreeMap<Pos, String>,
    /// Entities (`entity:` lines), in file order. Most designs have none.
    pub entities: Vec<EntitySpec>,
}

fn parse_pos(s: &str) -> Result<Pos> {
    let v: Vec<i32> = s.split_whitespace().map(|t| t.parse::<i32>()).collect::<Result<_, _>>()?;
    if v.len() != 3 {
        bail!("expected 'x y z', got {s:?}");
    }
    Ok([v[0], v[1], v[2]])
}

impl Design {
    pub fn load(path: &Path) -> Result<Design> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Design::parse(&text).with_context(|| format!("parsing {}", path.display()))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.to_text())?;
        Ok(())
    }

    pub fn parse(text: &str) -> Result<Design> {
        enum Section {
            Header,
            Palette,
            Layer(i32),
        }
        let mut d = Design::default();
        let mut origin: Pos = [0, 0, 0];
        let mut palette: BTreeMap<char, String> = BTreeMap::new();
        let mut section = Section::Header;
        let mut row = 0i32;
        for (lineno, raw) in text.lines().enumerate() {
            let err = || format!("line {}: {raw:?}", lineno + 1);
            let line = match raw.find('#') {
                // '#' starts a comment except inside a layer row, where it can be a palette char.
                Some(i) if !matches!(section, Section::Layer(_)) || !raw.starts_with("  ") => &raw[..i],
                _ => raw,
            };
            if line.trim().is_empty() {
                continue;
            }
            let indented = line.starts_with("  ");
            if !indented {
                let l = line.trim();
                if l == "palette:" {
                    section = Section::Palette;
                } else if let Some(y) = l.strip_prefix("layer ").and_then(|r| r.strip_suffix(':')) {
                    section = Section::Layer(y.trim().parse().with_context(err)?);
                    row = 0;
                } else if let Some(rest) = l.strip_prefix("port ") {
                    let (name, pos) = rest.split_once(':').ok_or_else(|| anyhow!("{}", err()))?;
                    // `port <name>: x y z [args...]`: words after the position (a clock's start
                    // kick: `port start: 1 0 0 minecraft:redstone_block 1`) are kept in `meta`
                    // under `port <name>` and written back on the port line.
                    let words: Vec<&str> = pos.split_whitespace().collect();
                    let (xyz, args) = if words.len() > 3 { (words[..3].join(" "), words[3..].join(" ")) } else { (pos.to_string(), String::new()) };
                    d.ports.insert(name.trim().to_string(), parse_pos(&xyz).with_context(err)?);
                    if !args.is_empty() {
                        d.meta.insert(format!("port {}", name.trim()), args);
                    }
                    section = Section::Header;
                } else if let Some(rest) = l.strip_prefix("entity:") {
                    d.entities.push(EntitySpec::parse(rest).with_context(err)?);
                    section = Section::Header;
                } else if let Some((k, v)) = l.split_once(':') {
                    let (k, v) = (k.trim(), v.trim());
                    match k {
                        "name" => d.name = v.to_string(),
                        "origin" => origin = parse_pos(v).with_context(err)?,
                        _ => {
                            d.meta.insert(k.to_string(), v.to_string());
                        }
                    }
                    section = Section::Header;
                } else {
                    bail!("unexpected {}", err());
                }
                continue;
            }
            match section {
                Section::Header => bail!("indented line outside palette/layer: {}", err()),
                Section::Palette => {
                    let body = line.trim();
                    let mut chars = body.chars();
                    let c = chars.next().unwrap();
                    let state = chars.as_str().trim();
                    if c == '.' || state.is_empty() {
                        bail!("bad palette entry {}", err());
                    }
                    let parsed = BlockState::parse(state).with_context(err)?;
                    palette.insert(c, parsed.to_string());
                }
                Section::Layer(y) => {
                    let body = &line[2..];
                    for (col, c) in body.chars().enumerate() {
                        if c == '.' || c == ' ' {
                            continue;
                        }
                        let state = palette.get(&c).ok_or_else(|| anyhow!("unknown palette char {c:?} at {}", err()))?;
                        let pos = [origin[0] + col as i32, origin[1] + y, origin[2] + row];
                        d.blocks.insert(pos, state.clone());
                    }
                    row += 1;
                }
            }
        }
        Ok(d)
    }

    /// Inclusive bounding box of the blocks.
    pub fn bounds(&self) -> Option<(Pos, Pos)> {
        let mut it = self.blocks.keys();
        let first = *it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        Some((lo, hi))
    }

    /// Number of non-air blocks.
    pub fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// The same design moved by `d` (ports and entities included).
    pub fn translated(&self, d: Pos) -> Design {
        let mv = |p: &Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        Design {
            name: self.name.clone(),
            meta: self.meta.clone(),
            ports: self.ports.iter().map(|(k, p)| (k.clone(), mv(p))).collect(),
            blocks: self.blocks.iter().map(|(p, s)| (mv(p), s.clone())).collect(),
            entities: self
                .entities
                .iter()
                .map(|e| EntitySpec {
                    pos: [e.pos[0] + d[0] as f64, e.pos[1] + d[1] as f64, e.pos[2] + d[2] as f64],
                    ..e.clone()
                })
                .collect(),
        }
    }

    /// A key identifying the design's content (blocks, ports, and entities when it has any), for
    /// deduplication and verdict caches: a design without entities keeps its old key.
    pub fn content_key(&self) -> String {
        let mut key = format!("{:?}|{:?}", self.blocks, self.ports);
        if !self.entities.is_empty() {
            let ents: Vec<String> = self.entities.iter().map(|e| e.to_text()).collect();
            key.push_str(&format!("|{ents:?}"));
        }
        key
    }

    /// Blocks as a list for [`crate::sim::Worker::set_blocks`].
    pub fn block_list(&self) -> Vec<(Pos, String)> {
        self.blocks.iter().map(|(p, s)| (*p, s.clone())).collect()
    }

    /// Serialize; palette characters are picked from the block names.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        if !self.name.is_empty() {
            out.push_str(&format!("name: {}\n", self.name));
        }
        for (k, v) in &self.meta {
            if k.starts_with("port ") {
                continue;
            }
            out.push_str(&format!("{k}: {v}\n"));
        }
        for (k, p) in &self.ports {
            match self.meta.get(&format!("port {k}")) {
                Some(args) => out.push_str(&format!("port {k}: {} {} {} {args}\n", p[0], p[1], p[2])),
                None => out.push_str(&format!("port {k}: {} {} {}\n", p[0], p[1], p[2])),
            }
        }
        for e in &self.entities {
            out.push_str(&format!("entity: {}\n", e.to_text()));
        }
        let Some((lo, hi)) = self.bounds() else { return out };
        out.push_str(&format!("origin: {} 0 {}\n", lo[0], lo[2]));
        // Assign palette characters: uppercase initial, then lowercase, then a fallback pool.
        let mut states: Vec<&String> = self.blocks.values().collect();
        states.sort();
        states.dedup();
        let mut used: BTreeMap<char, ()> = BTreeMap::new();
        let mut chars: BTreeMap<&String, char> = BTreeMap::new();
        let pool: Vec<char> =
            "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789@$%&*+=?!<>^~/\\|;,'`\"(){}"
                .chars()
                .collect();
        for s in &states {
            let short = BlockState::parse(s).map(|b| b.short_name().to_string()).unwrap_or_default();
            let initial = short.chars().next().unwrap_or('X');
            let candidates = [initial.to_ascii_uppercase(), initial.to_ascii_lowercase()];
            let c = candidates
                .into_iter()
                .chain(pool.iter().copied())
                .find(|c| !used.contains_key(c))
                .expect("palette exhausted");
            used.insert(c, ());
            chars.insert(s, c);
        }
        out.push_str("palette:\n");
        let mut by_char: Vec<(char, &String)> = chars.iter().map(|(s, c)| (*c, *s)).collect();
        by_char.sort();
        for (c, s) in by_char {
            out.push_str(&format!("  {c} {s}\n"));
        }
        for y in lo[1]..=hi[1] {
            out.push_str(&format!("layer {y}:\n"));
            for z in lo[2]..=hi[2] {
                let mut row = String::from("  ");
                for x in lo[0]..=hi[0] {
                    row.push(self.blocks.get(&[x, y, z]).map(|s| chars[s]).unwrap_or('.'));
                }
                let trimmed = row.trim_end_matches('.');
                // An all-air row must stay visible or the parser would skip it and shift the rows.
                out.push_str(if trimmed.len() <= 2 { "  ." } else { trimmed });
                out.push('\n');
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let text = "name: demo\nport lever: 1 1 0\npalette:\n  S stone\n  P sticky_piston[facing=up]\nlayer 0:\n  S.\n  .S\nlayer 1:\n  P\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.block_count(), 3);
        assert_eq!(d.blocks[&[0, 1, 0]], "minecraft:sticky_piston[facing=up]");
        assert_eq!(d.blocks[&[1, 0, 1]], "minecraft:stone");
        assert_eq!(d.ports["lever"], [1, 1, 0]);
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again.blocks, d.blocks);
        assert_eq!(again.ports, d.ports);
        assert!(again.entities.is_empty());
        assert_eq!(again.content_key(), format!("{:?}|{:?}", d.blocks, d.ports));
    }

    #[test]
    fn port_args_round_trip() {
        let text = "name: c\nport output: 1 0 0\nport start: 0 0 1 minecraft:redstone_block 1\npalette:\n  O observer[facing=east]\nlayer 0:\n  OO\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.ports["start"], [0, 0, 1]);
        assert_eq!(d.meta["port start"], "minecraft:redstone_block 1");
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again, d);
        assert!(d.to_text().contains("port start: 0 0 1 minecraft:redstone_block 1\n"));
    }

    #[test]
    fn entities_round_trip() {
        let text = "name: e\nport door: 0 0 0\nentity: minecraft:minecart 0.5 1.0625 -3.25\nentity: armor_stand 2 1 0.5 {Rotation:[90f,0f]}\npalette:\n  S stone\nlayer 0:\n  S\n";
        let d = Design::parse(text).unwrap();
        assert_eq!(d.entities.len(), 2);
        assert_eq!(d.entities[0], EntitySpec { kind: "minecraft:minecart".into(), pos: [0.5, 1.0625, -3.25], nbt: None });
        assert_eq!(d.entities[1].kind, "minecraft:armor_stand");
        assert_eq!(d.entities[1].nbt.as_deref(), Some("{Rotation:[90f,0f]}"));
        assert_eq!(d.entities[0].cell(), [0, 1, -4]);
        let again = Design::parse(&d.to_text()).unwrap();
        assert_eq!(again, d);
        let moved = d.translated([1, -2, 3]);
        assert_eq!(moved.entities[0].pos, [1.5, -0.9375, -0.25]);
        assert!(Design::parse("entity: minecraft:minecart 1 2\n").is_err());
        assert!(Design::parse("entity: minecraft:minecart 1 2 3 Rotation\n").is_err());
    }
}
