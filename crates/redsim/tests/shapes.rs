//! `behavior::collision_boxes` against `behavior::sturdy_of`: for every block whose support shape
//! is its collision shape (the default `getBlockSupportShape`), the face tests on the boxes must
//! give the same sturdiness (FULL and RIGID on every face, CENTER on the top and bottom faces:
//! the only CENTER queries in the game).

use redsim::Block;
use redsim::behavior::{Support, collision_boxes, sturdy_of};
use redsim::piston::shape::{B16, face_supports};
use redsim::pos::Dir;

fn states() -> Vec<Block> {
    let mut v = Vec::new();
    let mut add = |s: String| v.push(Block::parse(&s).unwrap());
    for s in ["stone", "glass", "honey_block", "slime_block", "redstone_block", "observer[facing=up]", "redstone_lamp", "note_block", "target", "dropper", "oak_log"] {
        add(s.to_string());
    }
    for d in ["north", "east", "south", "west"] {
        add(format!("repeater[facing={d}]"));
        add(format!("comparator[facing={d}]"));
        add(format!("skeleton_wall_skull[facing={d}]"));
        for half in ["top", "bottom"] {
            for open in ["true", "false"] {
                add(format!("iron_trapdoor[facing={d},half={half},open={open}]"));
            }
        }
        for open in ["true", "false"] {
            for hinge in ["left", "right"] {
                add(format!("iron_door[facing={d},half=lower,hinge={hinge},open={open}]"));
            }
        }
    }
    for f in ["down", "north", "east", "south", "west"] {
        add(format!("hopper[facing={f}]"));
    }
    for t in ["bottom", "top", "double"] {
        add(format!("smooth_stone_slab[type={t}]"));
    }
    for b in 0..=6 {
        add(format!("cake[bites={b}]"));
    }
    for eye in ["true", "false"] {
        add(format!("end_portal_frame[eye={eye}]"));
    }
    for l in 0..=8 {
        add(format!("composter[level={l}]"));
    }
    add("skeleton_skull".into());
    for d in 0..=7 {
        add(format!("scaffolding[distance={d}]"));
    }
    v
}

#[test]
fn collision_boxes_agree_with_sturdiness() {
    let mut bad = Vec::new();
    for s in states() {
        let boxes: Vec<B16> = collision_boxes(&s).into_iter().map(B16).collect();
        for face in Dir::ALL {
            for sup in [Support::Full, Support::Center, Support::Rigid] {
                if sup == Support::Center && !matches!(face, Dir::Up | Dir::Down) {
                    continue;
                }
                let a = face_supports(&boxes, face, sup);
                let b = sturdy_of(&s, face, sup);
                if a != b {
                    bad.push(format!("{s} {face:?} {sup:?}: boxes {a}, sturdy_of {b}"));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
