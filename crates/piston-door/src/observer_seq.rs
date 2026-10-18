//! observer-seq track: event-driven 3x3 doors (every piston retracted at rest, one observer
//! pulse per action; see designs/piston-door-3x3/tracks/observer-seq/NOTES.md).

use crate::designs::{Builder, FLOOR};
use mctas::design::Design;
use mctas::sim::Pos;

pub fn names() -> Vec<&'static str> {
    vec!["obs-d2", "obs-d3", "obs-e1m"]
}

pub fn build(name: &str) -> Option<Design> {
    match name {
        "obs-d2" => Some(d2()),
        "obs-d3" => Some(d3()),
        "obs-e1m" => Some(e1_mech()),
        _ => None,
    }
}

/// e1 mechanism (toggles + redstone-block tricks): P_L (0,3,0) faces west into the left
/// structure's top slime (-1,3,0) and shares (0,4,0) with A; the left structure carries RB_BM
/// (0,-2,0) that holds P_BM extended; P_R (2,4,0) faces east into (3,4,0) and the right
/// structure's slime (2,3,0) drags T out of (1,3,0) (no P_T). A and B retracted at rest.
pub fn e1_mech() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-obs-e1m".into();
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], STONE);
        }
    }
    for y in 0..4 {
        b.set([-1, y, 0], SLIME);
    }
    for y in 0..5 {
        b.set([3, y, 0], SLIME);
    }
    b.set([2, 3, 0], SLIME);
    // P_BM toggle, pulsed by the left column's bottom observer through (0,-2,0).
    b.set([1, -2, 0], &sticky("up"));
    b.set([0, -1, 0], &observer("up"));
    b.set([0, -2, 0], STONE);
    b.set([0, 3, 0], &sticky("west")); // P_L
    b.set([2, 4, 0], &sticky("east")); // P_R
    b.set([1, 4, 0], &sticky("down")); // A
    b.set([1, 5, 0], &sticky("down")); // B
    // rb2's window: R_B (1,6,1) -> B's QC cell; O (1,5,1) watches R_B -> X (1,4,1) on (1,3,1).
    b.set([1, 6, 1], &repeater("north", 4));
    b.set([1, 5, 1], &observer("up"));
    b.set([1, 4, 1], "minecraft:redstone_wire[north=side,east=side,south=side,west=side]");
    b.set([1, 3, 1], STONE);
    b.port("door", [0, 0, 0]);
    b.port("lever", [9, 9, 9]);
    b.d
}

const WIRE: &str = "minecraft:redstone_wire";

/// d3: like d2, but O watches B itself (2 pulses per B window, unaffected by R_B's lock), R_B
/// sits north of B, the park observer (1,3,-2) feeds R_B (locked while the lever is off) and,
/// through a 2gt repeater, the X net (late A: pushes T+C home in the closing, harmless in the
/// opening). Lever-edge pulses for the sides/M and P_T are not wired yet (driven by script).
pub fn d3() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-obs-d3".into();
    mech_bottom_inner(&mut b);
    // A on both lever edges: left column top observer.
    put(&mut b, &[([0, 3, 0], &observer("down")), ([0, 4, 0], STONE)]);
    // Window: R_B (1,5,-1) flows south into B; O (2,5,0) watches B -> D1 (3,5,0) -> X (2,4,0).
    put(
        &mut b,
        &[
            ([1, 5, -1], &repeater("south", 4)),
            ([1, 4, -1], FLOOR),
            ([2, 5, 0], &observer("west")),
            ([3, 5, 0], WIRE),
            ([3, 4, 0], FLOOR),
            ([2, 4, 0], WIRE),
            ([2, 3, 0], STONE),
        ],
    );
    // Lock: lever on (-2,5,-1), torch (-1,5,-1), RL (0,5,-1) into R_B's west side.
    put(
        &mut b,
        &[
            ([-2, 5, -1], FLOOR),
            ([-2, 6, -1], "minecraft:lever[face=floor,facing=west,powered=false]"),
            ([-1, 5, -1], "minecraft:redstone_wall_torch[facing=east,lit=true]"),
            ([0, 5, -1], &repeater("east", 1)),
            ([0, 4, -1], FLOOR),
        ],
    );
    b.port("lever", [-2, 6, -1]);
    // Park observer (1,3,-2) -> (1,3,-3) -> dust up to R_B's input (1,5,-2).
    put(
        &mut b,
        &[
            ([1, 3, -2], &observer("south")),
            ([1, 3, -3], FLOOR),
            ([1, 4, -3], WIRE),
            ([1, 4, -2], FLOOR),
            ([1, 5, -2], WIRE),
        ],
    );
    // Late A: (1,5,-2) -> (2,5,-2) -> (3,5,-2) -> repeater (3,5,-1) south into D1.
    put(
        &mut b,
        &[
            ([2, 5, -2], WIRE),
            ([2, 4, -2], FLOOR),
            ([3, 5, -2], WIRE),
            ([3, 4, -2], FLOOR),
            ([3, 5, -1], &repeater("south", 1)),
            ([3, 4, -1], FLOOR),
        ],
    );
    b.d
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
