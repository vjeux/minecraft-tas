//! Block state strings like `minecraft:sticky_piston[extended=false,facing=up]`.

use anyhow::{Result, bail};
use std::collections::BTreeMap;
use std::fmt;

/// A parsed block state: namespaced block id plus properties (sorted, like Minecraft prints).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockState {
    pub name: String,
    pub props: BTreeMap<String, String>,
}

impl BlockState {
    /// Parse `name` or `name[k=v,...]`; a missing namespace means `minecraft:`.
    pub fn parse(s: &str) -> Result<BlockState> {
        let s = s.trim();
        let (name, rest) = match s.find('[') {
            Some(i) => (&s[..i], Some(&s[i + 1..])),
            None => (s, None),
        };
        if name.is_empty() {
            bail!("empty block name in {s:?}");
        }
        let name = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
        let mut props = BTreeMap::new();
        if let Some(rest) = rest {
            let Some(body) = rest.strip_suffix(']') else { bail!("missing ']' in {s:?}") };
            for kv in body.split(',').filter(|kv| !kv.trim().is_empty()) {
                let Some((k, v)) = kv.split_once('=') else { bail!("bad property {kv:?} in {s:?}") };
                props.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
        Ok(BlockState { name, props })
    }

    /// Block id without the `minecraft:` namespace.
    pub fn short_name(&self) -> &str {
        self.name.strip_prefix("minecraft:").unwrap_or(&self.name)
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.props.get(key).map(String::as_str)
    }

    pub fn with(&self, key: &str, value: &str) -> BlockState {
        let mut s = self.clone();
        s.props.insert(key.to_string(), value.to_string());
        s
    }

    /// Rotate clockwise (seen from above) by `quarter_turns` * 90 degrees: `facing`,
    /// `axis` and per-side properties (`north`, `east`, ... as on redstone wire).
    pub fn rotate_y(&self, quarter_turns: i32) -> BlockState {
        let turns = quarter_turns.rem_euclid(4);
        if turns == 0 {
            return self.clone();
        }
        let rot = |d: &str| -> String {
            let order = ["north", "east", "south", "west"];
            match order.iter().position(|o| *o == d) {
                Some(i) => order[(i + turns as usize) % 4].to_string(),
                None => d.to_string(),
            }
        };
        let mut out = BlockState { name: self.name.clone(), props: BTreeMap::new() };
        for (k, v) in &self.props {
            match k.as_str() {
                "facing" | "horizontal_facing" => {
                    out.props.insert(k.clone(), rot(v));
                }
                "axis" if turns % 2 == 1 => {
                    let v = match v.as_str() {
                        "x" => "z",
                        "z" => "x",
                        other => other,
                    };
                    out.props.insert(k.clone(), v.to_string());
                }
                "north" | "east" | "south" | "west" => {
                    out.props.insert(rot(k), v.clone());
                }
                _ => {
                    out.props.insert(k.clone(), v.clone());
                }
            }
        }
        out
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)?;
        if !self.props.is_empty() {
            f.write_str("[")?;
            for (i, (k, v)) in self.props.iter().enumerate() {
                if i > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{k}={v}")?;
            }
            f.write_str("]")?;
        }
        Ok(())
    }
}

/// Air of any kind.
pub fn is_air(state: &str) -> bool {
    let name = state.split('[').next().unwrap_or(state);
    matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air" | "air")
}

/// The block id part of a state string (`minecraft:lever[...]` -> `minecraft:lever`).
pub fn block_name(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_print() {
        let s = BlockState::parse("sticky_piston[facing=up,extended=false]").unwrap();
        assert_eq!(s.to_string(), "minecraft:sticky_piston[extended=false,facing=up]");
        assert_eq!(s.rotate_y(1).get("facing"), Some("up"));
        let w = BlockState::parse("minecraft:redstone_wire[east=side,north=none,power=0,south=up,west=none]").unwrap();
        let r = w.rotate_y(1);
        assert_eq!(r.get("south"), Some("side"));
        assert_eq!(r.get("west"), Some("up"));
        assert_eq!(BlockState::parse("minecraft:lever[facing=north]").unwrap().rotate_y(3).get("facing"), Some("west"));
    }
}
