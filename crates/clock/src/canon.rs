//! Clock TYPES: a structure up to translation and the 4 horizontal rotations (docs/clocks.md).
//! Block states are turned and mirrored here in plain Rust on their text (facing, axis, the four
//! side properties, rotation, rail shapes, crafter orientation, hinge), so the census and the
//! catalog can canonicalize any block without a game. Dynamic properties (powered, lit, power...)
//! are dropped from the type key: they are the clock's running state, not its structure.

use crate::Pos;
use std::collections::BTreeMap;

const HORIZ: [&str; 4] = ["north", "east", "south", "west"];

fn turn_dir(d: &str, turns: u8) -> String {
    match HORIZ.iter().position(|h| *h == d) {
        Some(i) => HORIZ[(i + turns as usize) % 4].to_string(),
        None => d.to_string(),
    }
}

fn mirror_dir(d: &str) -> String {
    match d {
        "east" => "west".into(),
        "west" => "east".into(),
        _ => d.into(),
    }
}

/// Rail shapes turned clockwise once.
fn turn_shape(s: &str) -> Option<&'static str> {
    Some(match s {
        "north_south" => "east_west",
        "east_west" => "north_south",
        "ascending_east" => "ascending_south",
        "ascending_south" => "ascending_west",
        "ascending_west" => "ascending_north",
        "ascending_north" => "ascending_east",
        "south_east" => "south_west",
        "south_west" => "north_west",
        "north_west" => "north_east",
        "north_east" => "south_east",
        _ => return None,
    })
}

fn mirror_shape(s: &str) -> Option<&'static str> {
    Some(match s {
        "north_south" => "north_south",
        "east_west" => "east_west",
        "ascending_east" => "ascending_west",
        "ascending_west" => "ascending_east",
        "ascending_north" => "ascending_north",
        "ascending_south" => "ascending_south",
        "south_east" => "south_west",
        "south_west" => "south_east",
        "north_west" => "north_east",
        "north_east" => "north_west",
        // stairs
        "inner_left" => "inner_right",
        "inner_right" => "inner_left",
        "outer_left" => "outer_right",
        "outer_right" => "outer_left",
        _ => return None,
    })
}

fn split(state: &str) -> (&str, Vec<(String, String)>) {
    match state.find('[') {
        Some(i) => {
            let props = state[i + 1..].trim_end_matches(']');
            let kv = props.split(',').filter(|p| !p.is_empty()).filter_map(|p| p.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
            (&state[..i], kv)
        }
        None => (state, Vec::new()),
    }
}

fn join(name: &str, mut kv: Vec<(String, String)>) -> String {
    if kv.is_empty() {
        return name.to_string();
    }
    kv.sort();
    format!("{name}[{}]", kv.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(","))
}

/// A block state turned `turns` quarter turns clockwise seen from above (north -> east), like the
/// game's `BlockState.rotate(Rotation.CLOCKWISE_90)`.
pub fn rotate_state(state: &str, turns: u8) -> String {
    let turns = turns % 4;
    if turns == 0 {
        return state.to_string();
    }
    let (name, kv) = split(state);
    let sides: BTreeMap<String, String> = kv.iter().filter(|(k, _)| HORIZ.contains(&k.as_str())).cloned().collect();
    let mut out = Vec::new();
    for (k, v) in kv {
        let nv = match k.as_str() {
            "facing" => turn_dir(&v, turns),
            "axis" if turns % 2 == 1 => match v.as_str() {
                "x" => "z".into(),
                "z" => "x".into(),
                _ => v,
            },
            "rotation" => v.parse::<u8>().map(|r| ((r + 4 * turns) % 16).to_string()).unwrap_or(v),
            "shape" => {
                let mut s = v.clone();
                for _ in 0..turns {
                    match turn_shape(&s) {
                        Some(n) => s = n.to_string(),
                        None => break,
                    }
                }
                s
            }
            "orientation" => match v.split_once('_') {
                Some((a, b)) => format!("{}_{}", turn_dir(a, turns), turn_dir(b, turns)),
                None => v,
            },
            k if HORIZ.contains(&k) => continue,
            _ => v,
        };
        out.push((k, nv));
    }
    for (k, v) in &sides {
        out.push((turn_dir(k, turns), v.clone()));
    }
    join(name, out)
}

/// A block state mirrored front to back (x -> -x), like `BlockState.mirror(Mirror.FRONT_BACK)`.
pub fn mirror_state(state: &str) -> String {
    let (name, kv) = split(state);
    let mut out = Vec::new();
    for (k, v) in kv {
        let (nk, nv) = match k.as_str() {
            "facing" => (k.clone(), mirror_dir(&v)),
            "rotation" => (k.clone(), v.parse::<u8>().map(|r| ((16 - r) % 16).to_string()).unwrap_or(v)),
            "shape" => (k.clone(), mirror_shape(&v).map(|s| s.to_string()).unwrap_or(v)),
            "hinge" => (k.clone(), if v == "left" { "right".into() } else if v == "right" { "left".into() } else { v }),
            "orientation" => (
                k.clone(),
                match v.split_once('_') {
                    Some((a, b)) => format!("{}_{}", mirror_dir(a), mirror_dir(b)),
                    None => v,
                },
            ),
            "east" => ("west".into(), v),
            "west" => ("east".into(), v),
            _ => (k.clone(), v),
        };
        out.push((nk, nv));
    }
    join(name, out)
}

/// Properties that are a clock's running state, not its structure.
const DYNAMIC: [&str; 14] = ["powered", "lit", "power", "extended", "triggered", "enabled", "sculk_sensor_phase", "open", "locked", "short", "distance", "crafting", "bottom", "in_wall"];

/// A block state without its dynamic properties (and without `minecraft:`).
pub fn structural(state: &str) -> String {
    let (name, kv) = split(state);
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    join(name, kv.into_iter().filter(|(k, _)| !DYNAMIC.contains(&k.as_str())).collect())
}

fn encode(cells: &[(Pos, String)]) -> String {
    let mut v: Vec<(Pos, String)> = cells.to_vec();
    let lo = [0, 1, 2].map(|i| v.iter().map(|(p, _)| p[i]).min().unwrap_or(0));
    for (p, _) in v.iter_mut() {
        for i in 0..3 {
            p[i] -= lo[i];
        }
    }
    v.sort();
    v.iter().map(|(p, s)| format!("{},{},{}={}", p[0], p[1], p[2], s)).collect::<Vec<_>>().join(";")
}

/// The canonical type key of a set of blocks: the smallest encoding (cells moved to the origin,
/// sorted, `x,y,z=state;...`, structural states) over the 4 rotations AND their mirror images
/// (a mirror pair is one type; [`chiral`] tells them apart).
pub fn type_key<'a>(blocks: impl IntoIterator<Item = (&'a Pos, &'a String)> + Clone) -> String {
    let direct = rotation_key(blocks.clone());
    let m: Vec<(Pos, String)> = blocks.into_iter().map(|(p, s)| ([-p[0], p[1], p[2]], mirror_state(s))).collect();
    let mirrored = rotation_key(m.iter().map(|(p, s)| (p, s)));
    direct.min(mirrored)
}

/// The smallest encoding over the 4 rotations only (no mirror).
pub fn rotation_key<'a>(blocks: impl IntoIterator<Item = (&'a Pos, &'a String)> + Clone) -> String {
    (0..4u8)
        .map(|t| {
            let cells: Vec<(Pos, String)> = blocks.clone().into_iter().map(|(p, s)| (crate::rotate_pos(*p, t), structural(&rotate_state(s, t)))).collect();
            encode(&cells)
        })
        .min()
        .unwrap_or_default()
}

/// Is the structure different from its mirror image (up to rotation)?
pub fn chiral<'a>(blocks: impl IntoIterator<Item = (&'a Pos, &'a String)> + Clone) -> bool {
    let m: Vec<(Pos, String)> = blocks.clone().into_iter().map(|(p, s)| ([-p[0], p[1], p[2]], mirror_state(s))).collect();
    rotation_key(blocks) != rotation_key(m.iter().map(|(p, s)| (p, s)))
}

/// The type key of a whole design: its blocks with their block-entity data (`nbt:` lines, e.g.
/// container contents) attached to the cells, over rotations and mirror images.
pub fn design_type_key(d: &crate::Design) -> String {
    let mut best: Option<String> = None;
    for mirror in [false, true] {
        for t in 0..4u8 {
            let cells: Vec<(Pos, String)> = d
                .blocks
                .iter()
                .map(|(p, s)| {
                    let (p, s) = if mirror { ([-p[0], p[1], p[2]], mirror_state(s)) } else { (*p, s.clone()) };
                    let st = structural(&rotate_state(&s, t));
                    let nbt = d.block_entities.get(&(if mirror { [-p[0], p[1], p[2]] } else { p })).cloned().unwrap_or_default();
                    (crate::rotate_pos(p, t), format!("{st}{nbt}"))
                })
                // design entities (minecarts...) by their cell, turned like the blocks
                .chain(d.entities.iter().map(|e| {
                    let cell = [e.pos[0].floor() as i32, e.pos[1].floor() as i32, e.pos[2].floor() as i32];
                    let cell = if mirror { [-cell[0], cell[1], cell[2]] } else { cell };
                    (crate::rotate_pos(cell, t), format!("entity {}", e.kind))
                }))
                .collect();
            let e = encode(&cells);
            if best.as_ref().is_none_or(|b| e < *b) {
                best = Some(e);
            }
        }
    }
    best.unwrap_or_default()
}

/// The type key of the mirror image (to tell chiral types: key != mirror key).
pub fn mirror_type_key<'a>(blocks: impl IntoIterator<Item = (&'a Pos, &'a String)> + Clone) -> String {
    let m: Vec<(Pos, String)> = blocks.into_iter().map(|(p, s)| ([-p[0], p[1], p[2]], mirror_state(s))).collect();
    rotation_key(m.iter().map(|(p, s)| (p, s)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotations() {
        assert_eq!(rotate_state("minecraft:observer[facing=north,powered=false]", 1), "minecraft:observer[facing=east,powered=false]");
        assert_eq!(rotate_state("minecraft:observer[facing=up,powered=false]", 3), "minecraft:observer[facing=up,powered=false]");
        assert_eq!(
            rotate_state("minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=up]", 1),
            "minecraft:redstone_wire[east=side,north=up,power=0,south=none,west=none]"
        );
        assert_eq!(rotate_state("minecraft:oak_log[axis=x]", 1), "minecraft:oak_log[axis=z]");
        assert_eq!(rotate_state("minecraft:powered_rail[powered=false,shape=ascending_east,waterlogged=false]", 1), "minecraft:powered_rail[powered=false,shape=ascending_south,waterlogged=false]");
        assert_eq!(rotate_state("minecraft:crafter[crafting=false,orientation=north_up,triggered=false]", 1), "minecraft:crafter[crafting=false,orientation=east_up,triggered=false]");
        assert_eq!(rotate_state("minecraft:skeleton_skull[powered=false,rotation=14]", 1), "minecraft:skeleton_skull[powered=false,rotation=2]");
        for t in 0..4 {
            let s = "minecraft:repeater[delay=2,facing=west,locked=false,powered=false]";
            assert_eq!(rotate_state(&rotate_state(s, t), 4 - t), s);
        }
        assert_eq!(mirror_state("minecraft:repeater[delay=2,facing=west,locked=false,powered=false]"), "minecraft:repeater[delay=2,facing=east,locked=false,powered=false]");
        assert_eq!(structural("minecraft:repeater[delay=2,facing=west,locked=false,powered=true]"), "repeater[delay=2,facing=west]");
    }

    #[test]
    fn types() {
        // the observer pair in its 4 rotations is one type, and achiral
        let a: BTreeMap<Pos, String> = [([0, 0, 0], "minecraft:observer[facing=east]".to_string()), ([1, 0, 0], "minecraft:observer[facing=west]".to_string())].into();
        let b: BTreeMap<Pos, String> = [([5, 2, 3], "minecraft:observer[facing=south]".to_string()), ([5, 2, 4], "minecraft:observer[facing=north]".to_string())].into();
        assert_eq!(type_key(a.iter()), type_key(b.iter()));
        assert_eq!(type_key(a.iter()), mirror_type_key(a.iter()));
        // the two 4-observer rings (clockwise, counterclockwise) are mirror images: one type, chiral
        let cw: BTreeMap<Pos, String> = [
            ([0, 0, 0], "minecraft:observer[facing=east]".to_string()),
            ([0, 0, 1], "minecraft:observer[facing=north]".to_string()),
            ([1, 0, 0], "minecraft:observer[facing=south]".to_string()),
            ([1, 0, 1], "minecraft:observer[facing=west]".to_string()),
        ]
        .into();
        let ccw: BTreeMap<Pos, String> = [
            ([0, 0, 0], "minecraft:observer[facing=south]".to_string()),
            ([0, 0, 1], "minecraft:observer[facing=east]".to_string()),
            ([1, 0, 0], "minecraft:observer[facing=west]".to_string()),
            ([1, 0, 1], "minecraft:observer[facing=north]".to_string()),
        ]
        .into();
        assert_ne!(rotation_key(cw.iter()), rotation_key(ccw.iter()));
        assert_eq!(type_key(cw.iter()), type_key(ccw.iter()));
        assert!(chiral(cw.iter()));
        assert!(!chiral(a.iter()));
    }
}
