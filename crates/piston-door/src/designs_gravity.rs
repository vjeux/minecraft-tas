//! Gravity-category doors (all nine door blocks are sand), track `gravity`.
//!
//! g1 mechanism ("drop tower"):
//! * the side columns ride slime backbones pulled sideways by one sticky piston each (as v2);
//!   their sand lands on obsidian floors the slime cannot drag;
//! * the middle column is held up by a 3-piston tower in the shaft below the door. Opening
//!   collapses the tower top-down (R3 retracts, S2 pulls R3 down, S1 pulls the slime chain
//!   which drags S2 and R3 down) and the three sand blocks fall 3 cells into the shaft.
//!   Closing extends the tower bottom-up (S1 lifts chain + S2 + R3 + sand, S2 lifts R3 + sand,
//!   R3 lifts the sand into the door).
//! * control: A = NOT lever (torch T_A). R3 is powered by quasi-connectivity through T_A, which
//!   only reaches R3 at its top position (so R3 waits for S2 by itself); S2 = A + 3 ticks;
//!   S1 = A OR A + 6 ticks (first on, last off).

use crate::designs::{Builder, FLOOR};
use mctas::design::Design;

pub fn mechanism(b: &mut Builder) {
    let sand = "minecraft:sand";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], sand);
        }
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    b.set([-1, -1, 0], "obsidian");
    b.set([3, -1, 0], "obsidian");
    b.set([0, -1, 0], FLOOR);
    b.set([2, -1, 0], FLOOR);
    b.set([-3, 0, 0], "sticky_piston[facing=east,extended=true]");
    b.set([-2, 0, 0], "piston_head[facing=east,type=sticky,short=false]");
    b.set([5, 0, 0], "sticky_piston[facing=west,extended=true]");
    b.set([4, 0, 0], "piston_head[facing=west,type=sticky,short=false]");
    // Tower (closed state).
    b.set([1, -2, 0], "piston[facing=up,extended=true]"); // R3
    b.set([1, -1, 0], "piston_head[facing=up,type=normal,short=false]");
    b.set([1, -4, 0], "sticky_piston[facing=up,extended=true]"); // S2
    b.set([1, -3, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, -4, 1], "slime_block"); // chain
    b.set([1, -3, 1], "slime_block");
    b.set([1, -6, 1], "sticky_piston[facing=up,extended=true]"); // S1
    b.set([1, -5, 1], "piston_head[facing=up,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);
}

/// g1 base: mechanism, lever, T_A and the two side lines; S2/S1 drivers (repeaters) placed,
/// their feeds routed separately.
pub fn g1() -> Design {
    let mut b = Builder::default();
    b.d.name = "gravity-g1".into();
    mechanism(&mut b);
    // Lever on the doorway floor in front of the door, T_A under it.
    b.set([1, -1, -2], FLOOR);
    b.set([1, 0, -2], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, 0, -2]);
    b.set([1, -1, -1], "redstone_wall_torch[facing=south,lit=true]"); // T_A
    // Left side line: ends pointing into (-3,0,-1), next to the left piston.
    b.dust([0, -1, -1]);
    b.dust([-1, 0, -1]);
    b.dust([-2, 0, -1]);
    b.set([-3, 0, -1], FLOOR);
    // Right side line.
    b.dust([2, -1, -1]);
    b.dust([3, 0, -1]);
    b.dust([4, 0, -1]);
    b.set([5, 0, -1], FLOOR);
    // S2 driver: A + 3 into S2's top position.
    b.repeater([1, -4, -1], "south", 2);
    // S1 drivers: slow (A + 6) from the north.
    b.repeater([1, -6, 0], "south", 3);
    b.repeater([1, -6, -1], "south", 1);
    for (p, s) in g1_routes() {
        b.set(p, s);
    }
    b.d
}

fn g1_routes() -> Vec<([i32; 3], &'static str)> {
    let w = "minecraft:redstone_wire";
    let f = FLOOR;
    vec![
        // A down the north side: D1, D2 (D2's floor feeds RS2).
        ([2, -2, -2], w), ([2, -3, -2], f),
        ([1, -3, -2], w), ([1, -4, -2], f),
        // On to the floor behind RS1b.
        ([1, -4, -3], w), ([1, -5, -3], f),
        ([0, -5, -3], w), ([0, -6, -3], f),
        ([0, -5, -2], w), ([0, -6, -2], f),
        ([1, -5, -2], w), ([1, -6, -2], f),
        // S1 fast: a north-south line whose last floor touches S1.
        ([2, -5, -2], w), ([2, -6, -2], f),
        ([2, -5, -1], w), ([2, -6, -1], f),
        ([2, -5, 0], w), ([2, -6, 0], f),
        ([2, -5, 1], w), ([2, -6, 1], f),
    ]
}

/// g3: g1 with the sides PUSHED open from inside by sticky pistons that are the doorway floor
/// under the side columns (P_L at (0,-1,0) facing west, P_R at (2,-1,0) facing east): the
/// backbones reach down to y=-1, and once pushed the side sand rests on the piston heads.
/// The side pistons are powered by the lever itself (no torch, no side lines).
pub fn g3() -> Design {
    let mut b = Builder::default();
    b.d.name = "gravity-g3".into();
    let sand = "minecraft:sand";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], sand);
        }
    }
    for y in -1..3 {
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    b.set([0, -1, 0], "sticky_piston[facing=west,extended=false]"); // P_L
    b.set([2, -1, 0], "sticky_piston[facing=east,extended=false]"); // P_R
    b.set([1, -2, 0], "piston[facing=up,extended=true]"); // R3
    b.set([1, -1, 0], "piston_head[facing=up,type=normal,short=false]");
    b.set([1, -4, 0], "sticky_piston[facing=up,extended=true]"); // S2
    b.set([1, -3, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, -4, 1], "slime_block"); // chain
    b.set([1, -3, 1], "slime_block");
    b.set([1, -6, 1], "sticky_piston[facing=up,extended=true]"); // S1
    b.set([1, -5, 1], "piston_head[facing=up,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);
    // Lever block LB, T_A2 (R3 by QC) on its south face, T_A (A net) on its north face.
    b.set([1, -1, -2], FLOOR);
    b.set([1, 0, -2], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, 0, -2]);
    b.set([1, -1, -1], "redstone_wall_torch[facing=south,lit=true]"); // T_A2
    b.set([1, -1, -3], "redstone_wall_torch[facing=north,lit=true]"); // T_A
    // Side pistons: L through a dust on each side of LB into the block next to the piston.
    b.set([0, -1, -2], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.set([0, -2, -2], FLOOR);
    b.set([0, -1, -1], FLOOR);
    b.set([2, -1, -2], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.set([2, -2, -2], FLOOR);
    b.set([2, -1, -1], FLOOR);
    let w = "minecraft:redstone_wire";
    let f = FLOOR;
    for (p, s) in [
        // A: under T_A, down to the floor feeding RS2.
        ([1, -2, -3], w), ([1, -3, -3], f),
        ([1, -3, -2], w), ([1, -4, -2], f),
        // On to the floor behind RS1b.
        ([0, -4, -2], w), ([0, -5, -2], f),
        ([0, -5, -3], w), ([0, -6, -3], f),
        ([1, -5, -3], w), ([1, -6, -3], f),
        ([1, -5, -2], w), ([1, -6, -2], f),
        // S1 fast.
        ([2, -5, -2], w), ([2, -6, -2], f),
        ([2, -5, -1], w), ([2, -6, -1], f),
        ([2, -5, 0], w), ([2, -6, 0], f),
        ([2, -5, 1], w), ([2, -6, 1], f),
    ] {
        b.set(p, s);
    }
    b.repeater([1, -4, -1], "south", 2); // RS2
    b.repeater([1, -6, 0], "south", 3); // RS1
    b.repeater([1, -6, -1], "south", 1); // RS1b
    b.d
}

/// g4: g3 with the tower's slime chain and S1 moved to the north side (z=-1), next to the
/// control: S1's fast input is the floor of an A dust, S2 is fed through a block west of it.
pub fn g4() -> Design {
    let mut b = Builder::default();
    b.d.name = "gravity-g4".into();
    let sand = "minecraft:sand";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], sand);
        }
    }
    for y in -1..3 {
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    b.set([0, -1, 0], "sticky_piston[facing=west,extended=false]"); // P_L
    b.set([2, -1, 0], "sticky_piston[facing=east,extended=false]"); // P_R
    b.set([1, -2, 0], "piston[facing=up,extended=true]"); // R3
    b.set([1, -1, 0], "piston_head[facing=up,type=normal,short=false]");
    b.set([1, -4, 0], "sticky_piston[facing=up,extended=true]"); // S2
    b.set([1, -3, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, -4, -1], "slime_block"); // chain
    b.set([1, -3, -1], "slime_block");
    b.set([1, -6, -1], "sticky_piston[facing=up,extended=true]"); // S1
    b.set([1, -5, -1], "piston_head[facing=up,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);
    b.set([1, -1, -2], FLOOR); // LB
    b.set([1, 0, -2], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, 0, -2]);
    b.set([1, -1, -1], "redstone_wall_torch[facing=south,lit=true]"); // T_A2: R3 by QC
    b.set([1, -1, -3], "redstone_wall_torch[facing=north,lit=true]"); // T_A
    let cross = "redstone_wire[north=side,east=side,south=side,west=side]";
    b.set([0, -1, -2], cross);
    b.set([0, -2, -2], FLOOR);
    b.set([0, -1, -1], FLOOR);
    b.set([2, -1, -2], cross);
    b.set([2, -2, -2], FLOOR);
    b.set([2, -1, -1], FLOOR);
    let w = "minecraft:redstone_wire";
    let f = FLOOR;
    for (p, s) in [
        ([1, -2, -3], w), ([1, -3, -3], f),
        ([0, -3, -3], w), ([0, -4, -3], f),
        ([0, -4, -2], w), ([0, -5, -2], f),
        ([1, -5, -2], w), ([1, -6, -2], f), // floor = S1 fast
        ([-1, -5, -2], w), ([-1, -6, -2], f),
        ([-1, -6, -1], w), ([-1, -7, -1], f),
    ] {
        b.set(p, s);
    }
    // RS2 on an obsidian floor (the chain would drag a normal one), into the block west of S2.
    b.set([0, -4, -1], "repeater[facing=north,delay=2]");
    b.set([0, -5, -1], "obsidian");
    b.set([0, -4, 0], FLOOR);
    // RS1: A + 8 gt into S1 from the west.
    b.repeater([0, -6, -1], "east", 4);
    b.d
}

/// sf1: slime-free gravity door. Sides: 3 sticky pullers per side (one per row), powered by A
/// through one dust standing on a block next to the middle puller (the block powers the middle
/// puller, quasi-powers the bottom one, the dust points into the top one). Middle: the sand drop
/// tower without the slime chain: S1 pulls only S2, so S2 gets a second pulse (observer O
/// watching S1's slow repeater RS1) that pulls R3 down the last cell.
pub fn sf1() -> Design {
    let mut b = Builder::default();
    b.d.name = "gravity-sf1".into();
    let sand = "minecraft:sand";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], sand);
        }
        b.set([-2, y, 0], "sticky_piston[facing=east,extended=true]");
        b.set([-1, y, 0], "piston_head[facing=east,type=sticky,short=false]");
        b.set([4, y, 0], "sticky_piston[facing=west,extended=true]");
        b.set([3, y, 0], "piston_head[facing=west,type=sticky,short=false]");
    }
    for x in [-1, 0, 2, 3] {
        b.set([x, -1, 0], FLOOR);
    }
    b.set([1, -2, 0], "piston[facing=up,extended=true]"); // R3
    b.set([1, -1, 0], "piston_head[facing=up,type=normal,short=false]");
    b.set([1, -4, 0], "sticky_piston[facing=up,extended=true]"); // S2
    b.set([1, -3, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, -6, 0], "sticky_piston[facing=up,extended=true]"); // S1
    b.set([1, -5, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);
    b.set([1, -1, -2], FLOOR); // LB
    b.set([1, 0, -2], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, 0, -2]);
    b.set([1, -1, -1], "redstone_wall_torch[facing=south,lit=true]"); // T_A: R3 by QC + A net
    for p in [
        // left pullers
        [0, -1, -1], [-1, 0, -1], [-1, 1, -2], [-2, 2, -2], [-2, 2, -1],
        // right pullers
        [2, -1, -1], [3, 0, -1], [3, 1, -2], [4, 2, -2], [4, 2, -1],
        // down to RS2
        [0, -2, -2], [0, -3, -3], [1, -4, -3],
        // on to RS1 and S1 fast
        [2, -5, -3], [2, -6, -2], [1, -6, -2], [0, -6, -2], [0, -6, -1],
    ] {
        b.dust(p);
    }
    b.set([0, -6, 0], FLOOR); // S1 fast block
    b.set([1, -4, -2], "repeater[facing=north,delay=2]"); // RS2
    b.set([1, -5, -2], FLOOR);
    b.set([1, -4, -1], FLOOR); // B2: S2's source (RS2, and O's pulse)
    b.set([1, -5, -1], "observer[facing=down]"); // O watches RS1
    b.set([1, -6, -1], "repeater[facing=north,delay=4]"); // RS1: S1 slow
    b.set([1, -7, -1], FLOOR);
    b.d
}
