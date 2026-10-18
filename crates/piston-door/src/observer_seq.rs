//! observer-seq track: event-driven 3x3 doors (every piston retracted at rest, one observer
//! pulse per action; see designs/piston-door-3x3/tracks/observer-seq/NOTES.md).

use crate::designs::{Builder, FLOOR};
use mctas::design::Design;
use mctas::sim::Pos;

pub fn names() -> Vec<&'static str> {
    vec!["obs-d2"]
}

pub fn build(name: &str) -> Option<Design> {
    match name {
        "obs-d2" => Some(d2()),
        _ => None,
    }
}

const STONE: &str = "minecraft:stone";
const SLIME: &str = "minecraft:slime_block";

fn sticky(f: &str) -> String {
    format!("minecraft:sticky_piston[facing={f},extended=false]")
}
fn observer(f: &str) -> String {
    format!("minecraft:observer[facing={f},powered=false]")
}
/// Repeater whose signal flows toward `flow` (facing points at the input).
fn repeater(flow: &str, delay: u8) -> String {
    let facing = match flow {
        "east" => "west",
        "west" => "east",
        "north" => "south",
        _ => "north",
    };
    format!("minecraft:repeater[facing={facing},delay={delay}]")
}

fn put(b: &mut Builder, blocks: &[(Pos, &str)]) {
    for (p, s) in blocks {
        b.set(*p, s);
    }
}

/// The toggle mechanism with inner side pistons under the door (bottom row y=-1).
/// Sides: P_L (0,-1,0) faces west into the left backbone's bottom slime (-1,-1,0), P_R (2,-1,0)
/// faces east into (3,-1,0). P_BM (1,-2,0) pulls M down between them. A (1,4,0) / B (1,5,0)
/// do the UP dance, P_T (1,3,1) spits T forward to the park (1,3,-1).
fn mech_bottom_inner(b: &mut Builder) {
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], STONE);
        }
    }
    for y in -1..3 {
        b.set([-1, y, 0], SLIME);
        b.set([3, y, 0], SLIME);
    }
    b.set([0, -1, 0], &sticky("west"));
    b.set([2, -1, 0], &sticky("east"));
    b.set([1, -2, 0], &sticky("up"));
    b.set([1, 4, 0], &sticky("down")); // A
    b.set([1, 5, 0], &sticky("down")); // B
    b.set([1, 3, 1], &sticky("north")); // P_T
    b.port("door", [0, 0, 0]);
}

/// d2: bottom inner side pistons; the left column's top observer pulses A on both lever
/// edges; the park observer pulses A (late, through a repeater) and opens the B window
/// (R_B, locked while the lever is off); O on R_B pulses A in both dance positions (v2's X).
pub fn d2() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-obs-d2".into();
    mech_bottom_inner(&mut b);
    // Left column top observer: (0,3,0) watches (0,2,0), pulses (0,4,0) -> A.
    put(&mut b, &[([0, 3, 0], &observer("down")), ([0, 4, 0], STONE)]);
    // Window: R_B (2,6,0) -> B QC cell (1,6,0); O (2,5,0) watches R_B -> X dust (2,4,0) on (2,3,0).
    put(
        &mut b,
        &[
            ([2, 6, 0], &repeater("west", 4)),
            ([2, 5, 0], &observer("up")),
            ([2, 4, 0], "minecraft:redstone_wire[north=side,east=side,south=side,west=side]"),
            ([2, 3, 0], STONE),
        ],
    );
    // Lock: torch (2,6,-2) on the lever block (2,6,-3) -> R_lock (2,6,-1) into R_B's north side.
    put(
        &mut b,
        &[
            ([2, 6, -3], FLOOR),
            ([2, 7, -3], "minecraft:lever[face=floor,facing=west,powered=false]"),
            ([2, 6, -2], "minecraft:redstone_wall_torch[facing=south,lit=true]"),
            ([2, 6, -1], &repeater("south", 1)),
            ([2, 5, -1], FLOOR),
        ],
    );
    b.port("lever", [2, 7, -3]);
    // Park observer (2,3,-1) watches the park (1,3,-1), powers (3,3,-1).
    put(&mut b, &[([2, 3, -1], &observer("west")), ([3, 3, -1], FLOOR)]);
    // Late A: repeater (2,4,-1) on the park observer, flowing south into X.
    b.set([2, 4, -1], &repeater("south", 1));
    b.d
}
