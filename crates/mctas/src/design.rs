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

use crate::blockstate::BlockState;
use crate::sim::Pos;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Design {
    pub name: String,
    /// Other `key: value` header lines, in order of keys.
    pub meta: BTreeMap<String, String>,
    pub ports: BTreeMap<String, Pos>,
    /// Non-air blocks.
    pub blocks: BTreeMap<Pos, String>,
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
                    d.ports.insert(name.trim().to_string(), parse_pos(pos).with_context(err)?);
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

    /// The same design moved by `d` (ports included).
    pub fn translated(&self, d: Pos) -> Design {
        let mv = |p: &Pos| [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
        Design {
            name: self.name.clone(),
            meta: self.meta.clone(),
            ports: self.ports.iter().map(|(k, p)| (k.clone(), mv(p))).collect(),
            blocks: self.blocks.iter().map(|(p, s)| (mv(p), s.clone())).collect(),
        }
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
            out.push_str(&format!("{k}: {v}\n"));
        }
        for (k, p) in &self.ports {
            out.push_str(&format!("port {k}: {} {} {}\n", p[0], p[1], p[2]));
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
    }
}
