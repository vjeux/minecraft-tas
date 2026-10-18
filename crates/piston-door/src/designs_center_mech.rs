//! Track center-mech designs (see designs/piston-door-3x3/tracks/center-mech/NOTES.md).

use crate::designs::{Builder, FLOOR};
use mctas::design::Design;

/// v2u2: v2u with A's torch replaced by a redstone block R_A riding the left group (two more
/// backbone slime carry it). The side groups are fed L OR L_late, so they come home late, and
/// R_A with them: A's power is "NOR(L, L_late)" for free, with no A-early or A-late wiring.
/// L_24 joins the X line one-way: the late chain ends in a solid block (3,4,3) and a repeater
/// out of it starts the X line.
pub fn v2u2() -> Design {
    let mut d = v2u();
    d.name = "piston-door-3x3-v2u2".into();
    for p in [
        [0, 4, 0], [-1, 4, 0], [-1, 4, -1], [-1, 3, -1], [2, 4, 3], [2, 3, 3], [1, 4, 3], [1, 3, 3],
        [0, 3, 3], [0, 4, 3], [-1, 2, 3], [-1, 3, 3], [-1, 3, 2], [-1, 4, 2], [-1, 4, 1], [-1, 3, 1],
    ] {
        d.blocks.remove(&p);
    }
    let mut b = Builder { d };
    b.set([-1, 3, 0], "slime_block");
    b.set([-1, 4, 0], "slime_block");
    b.set([0, 4, 0], "redstone_block"); // R_A
    // Late chain ends in a solid block, strongly powered by the last repeater: it powers the P_T
    // net dust (3,4,2) and, one-way, the X line's stub (3,4,4).
    b.d.blocks.remove(&[3, 3, 3]);
    b.set([3, 4, 3], FLOOR);
    b.repeater([3, 4, 4], "south", 1);
    b.dust([3, 4, 5]);
    for (p, s) in [
        ([2, 2, 5], FLOOR), ([2, 3, 5], "redstone_wire"), ([2, 1, 6], FLOOR), ([2, 2, 6], "redstone_wire"),
        ([1, 0, 6], FLOOR), ([1, 1, 6], "redstone_wire"), ([1, -1, 5], FLOOR), ([1, 0, 5], "redstone_wire"),
        ([1, -2, 4], FLOOR), ([1, -1, 4], "redstone_wire"), ([1, -3, 3], FLOOR), ([1, -2, 3], "redstone_wire"),
        ([1, -2, 2], FLOOR), ([1, -1, 2], "redstone_wire"),
    ] {
        b.set(p, s); // router: X line from the late stub down to the dust pointing into X
    }
    for (p, s) in [
        ([7, 2, 4], FLOOR), ([7, 3, 4], "redstone_wire"), ([6, 1, 4], FLOOR), ([6, 2, 4], "redstone_wire"),
        ([5, 1, 4], FLOOR), ([5, 2, 4], "repeater[facing=east,delay=1]"), ([4, 2, 4], FLOOR), ([4, 3, 4], "redstone_wire"),
        ([4, 3, 5], FLOOR), ([4, 4, 5], "redstone_wire"),
    ] {
        b.set(p, s); // router: L net into the X line
    }
    b.d
}

/// v2nf (unfinished): slime-free attempt on top of v2u2: every side block gets its own sticky
/// puller (stacks of three at x=-2 / x=4, one torch each). Kept for reference; the feeds to the
/// side torches are not routed (see NOTES.md).
pub fn v2nf() -> Design {
    let mut d = v2u2();
    d.name = "piston-door-3x3-v2nf".into();
    d.blocks.retain(|_, s| !s.contains("slime_block") && !s.contains("honey_block"));
    for p in [[0, -1, 1], [2, -1, 1], [1, -1, 1], [2, -3, 0], [0, 4, 0], [1, -3, 0], [1, -2, 0]] {
        d.blocks.remove(&p);
    }
    let mut b = Builder { d };
    for y in 0..3 {
        b.set([-2, y, 0], "sticky_piston[facing=east,extended=true]");
        b.set([-1, y, 0], "piston_head[facing=east,type=sticky,short=false]");
        b.set([4, y, 0], "sticky_piston[facing=west,extended=true]");
        b.set([3, y, 0], "piston_head[facing=west,type=sticky,short=false]");
    }
    b.set([-3, 1, 0], "redstone_wall_torch[facing=east,lit=true]");
    b.set([-4, 1, 0], FLOOR);
    b.set([-3, 2, 0], FLOOR);
    b.set([5, 1, 0], "redstone_wall_torch[facing=west,lit=true]");
    b.set([6, 1, 0], FLOOR);
    b.set([5, 2, 0], FLOOR);
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=true]"); // P_M
    b.set([1, -1, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, -2, 1], "redstone_wall_torch[facing=north,lit=true]"); // on (1,-2,2), under the X line's end
    b.set([0, 4, 0], "redstone_wall_torch[facing=east,lit=true]"); // A torch on (-1,4,0)
    b.set([-1, 4, 0], FLOOR);
    b.d
}

/// v2u: v2's top (A, B, P_T and their control) over a new bottom with no torches and no
/// sides line. The side groups are PUSHED out from under the walkway by two sticky pistons that
/// share one obsidian block X (powered from the lever net); the bottom-middle rides a honey
/// carrier (honey never sticks to the side slime) pulled by P_M, which a redstone block riding
/// the right group keeps extended while the door is closed.
pub fn v2u() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-v2u".into();
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    // Bottom: pushers, X, honey M carrier, P_M, R_M.
    b.set([-1, -1, 0], "slime_block");
    b.set([-1, -1, 1], "slime_block");
    b.set([0, -1, 1], "sticky_piston[facing=west,extended=false]");
    b.set([1, -1, 1], "obsidian"); // X
    b.set([2, -1, 1], "sticky_piston[facing=east,extended=false]");
    for p in [[3, -1, 0], [3, -2, 0], [3, -3, 0], [3, -1, 1]] {
        b.set(p, "slime_block");
    }
    b.set([2, -3, 0], "redstone_block"); // R_M
    b.set([1, -1, 0], "honey_block");
    b.set([1, -3, 0], "sticky_piston[facing=up,extended=true]"); // P_M
    b.set([1, -2, 0], "piston_head[facing=up,type=sticky,short=false]");
    // Top (v2).
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 5, 0], "sticky_piston[facing=down,extended=false]"); // B
    b.set([1, 3, 2], "sticky_piston[facing=north,extended=true]"); // P_T
    b.set([1, 3, 1], "piston_head[facing=north,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);
    b.set([5, 5, 0], FLOOR);
    b.set([5, 6, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [5, 6, 0]);
    b.repeater([4, 6, 0], "west", 1); // W
    b.set([3, 6, 0], "observer[facing=east,powered=false]"); // O2
    b.set([2, 5, 0], "observer[facing=up,powered=false]"); // O
    b.set([2, 6, 0], "repeater[facing=east,delay=4]"); // R_B
    b.dust([5, 4, 0]); // L net
    b.set([2, 3, 0], stone);
    b.set([2, 4, 0], "redstone_wire[north=side,east=side,south=side,west=side]"); // X dust
    b.dust([6, 4, 0]);
    // A steady power: torch (0,4,0) on (-1,4,0), lit = NOR(L, L_late).
    b.set([0, 4, 0], "redstone_wall_torch[facing=east,lit=true]");
    b.set([-1, 4, 0], FLOOR);
    b.repeater([-1, 4, -1], "south", 1); // A early (input routed from the L net)
    // P_T power: torch (2,3,2) on (3,3,2), lit = NOR(L_6, L_28).
    b.set([2, 3, 2], "redstone_wall_torch[facing=west,lit=true]");
    b.dust([3, 4, 2]);
    b.dust([6, 4, 1]);
    b.dust([6, 4, 2]);
    b.dust([5, 4, 2]);
    b.repeater([4, 4, 2], "west", 4);
    b.dust([7, 4, 2]);
    b.dust([7, 4, 3]);
    b.repeater([6, 4, 3], "west", 4);
    b.repeater([5, 4, 3], "west", 4);
    b.repeater([4, 4, 3], "west", 4);
    b.dust([3, 4, 3]);
    b.repeater([2, 4, 3], "west", 1);
    b.repeater([1, 4, 3], "west", 1);
    for (p, s) in [
        ([0, 3, 3], FLOOR), ([0, 4, 3], "redstone_wire"), ([-1, 2, 3], FLOOR), ([-1, 3, 3], "redstone_wire"),
        ([-1, 3, 2], FLOOR), ([-1, 4, 2], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    b.repeater([-1, 4, 1], "north", 2);
    b.d
}
