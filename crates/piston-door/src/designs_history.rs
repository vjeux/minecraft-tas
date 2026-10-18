//! Reconstructed intermediate v2 designs (replayed from the session edit history) so every
//! verified step of the block-count work is archived.

use mctas::design::Design;

use crate::designs::{Builder, FLOOR};

pub fn v2_133() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-v2".into();
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    b.set([-3, 0, 0], "sticky_piston[facing=east,extended=true]");
    b.set([-2, 0, 0], "piston_head[facing=east,type=sticky,short=false]");
    b.set([5, 0, 0], "sticky_piston[facing=west,extended=true]");
    b.set([4, 0, 0], "piston_head[facing=west,type=sticky,short=false]");
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=true]");
    b.set([1, -1, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 5, 0], "sticky_piston[facing=down,extended=false]"); // B
    b.set([1, 3, 2], "sticky_piston[facing=north,extended=true]"); // P_T
    b.set([1, 3, 1], "piston_head[facing=north,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);

    // Lever and the B pulse chain: W -> O2 (both edges) -> R_ext (stretch to 8gt) -> R_B, which
    // QC-powers B through (1,6,0) and is watched by O, whose pulse lights the X dust that powers
    // A in both of its positions (directly at (1,4), through the (2,3,0) block at (1,3)).
    b.set([6, 5, 0], FLOOR);
    b.set([6, 6, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [6, 6, 0]);
    b.repeater([5, 6, 0], "west", 1); // W
    b.set([4, 6, 0], "observer[facing=east,powered=false]"); // O2
    b.repeater([3, 6, 0], "west", 4); // R_ext
    b.set([2, 5, 0], "observer[facing=up,powered=false]"); // O
    b.set([2, 6, 0], "repeater[facing=east,delay=1]"); // R_B (floor = O)
    b.set([2, 3, 0], stone);
    b.set([2, 4, 0], "redstone_wire[north=side,east=side,south=side,west=side]"); // X

    // Sides + bottom-middle: NOT lever, torches under/behind the three pistons.
    b.set([-3, -1, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.set([5, -1, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.set([1, -3, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.dust([6, 4, 0]);
    b.dust([7, 3, 0]);
    b.dust([8, 2, 0]);
    b.dust([8, 1, 1]);
    b.dust([7, 1, 1]);
    b.dust([6, 0, 1]);
    b.dust([5, 0, 1]); // floor (5,-1,1) = right torch attachment
    b.dust([4, -1, 1]);
    b.dust([3, -2, 1]);
    b.repeater([2, -2, 1], "west", 1);
    b.dust_run([1, -2, 1], [-1, -2, 1]); // floor (1,-3,1) = bottom torch attachment
    b.dust([-2, -1, 1]);
    b.dust([-3, 0, 1]); // floor (-3,-1,1) = left torch attachment

    // A steady power: torch (0,4,0) on (-1,4,0), lit = NOR(L, L_late).
    b.set([0, 4, 0], "redstone_wall_torch[facing=east,lit=true]");
    b.set([-1, 4, 0], FLOOR);
    // A's early input: a branch of the sides line (which carries L), up the left side.
    for (p, s) in [
        ([-4, -1, 1], FLOOR), ([-4, 0, 1], "redstone_wire"), ([-5, 0, 1], FLOOR), ([-5, 1, 1], "redstone_wire"),
        ([-5, 1, 0], FLOOR), ([-5, 2, 0], "redstone_wire"), ([-4, 2, 0], FLOOR), ([-4, 3, 0], "redstone_wire"),
        ([-3, 3, 0], FLOOR), ([-3, 4, 0], "redstone_wire"), ([-2, 4, 0], FLOOR), ([-2, 5, 0], "redstone_wire"),
        ([-1, 5, 0], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    // P_T power: torch (2,3,2) on (3,3,2), lit = NOR(L_6, L_28).
    b.set([2, 3, 2], "redstone_wall_torch[facing=west,lit=true]");
    b.dust([3, 4, 2]); // floor (3,3,2) = P_T torch attachment
    // L_6: R1 off the lever, then down to the repeater feeding the P_T net.
    b.repeater([7, 6, 0], "east", 3);
    b.dust([8, 6, 0]);
    b.dust([8, 6, 1]);
    b.dust([8, 5, 2]);
    b.dust([7, 4, 2]);
    b.dust([6, 4, 2]);
    b.dust([5, 4, 2]);
    b.repeater([4, 4, 2], "west", 1);
    // Late chain: L_6 + 22gt = L_28 into the P_T net, + 8gt more into the A net.
    b.repeater([6, 4, 3], "south", 4);
    b.repeater([6, 4, 4], "south", 4);
    b.repeater([6, 4, 5], "south", 2);
    b.dust_run([6, 4, 6], [3, 4, 6]);
    b.dust([3, 4, 5]);
    b.dust([3, 4, 4]);
    b.repeater([3, 4, 3], "north", 1);
    // A's late input: the P_T late net (L_28) through a 6gt repeater into the A net's end.
    for (p, s) in [
        ([2, 3, 4], FLOOR), ([2, 4, 4], "redstone_wire"), ([1, 3, 4], FLOOR), ([1, 4, 4], "redstone_wire"),
        ([0, 3, 4], FLOOR), ([0, 4, 4], "redstone_wire"), ([-1, 2, 4], FLOOR), ([-1, 3, 4], "redstone_wire"),
        ([-1, 3, 3], FLOOR), ([-1, 4, 3], "redstone_wire"), ([-1, 4, 2], FLOOR), ([-1, 5, 2], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    b.repeater([-1, 5, 1], "north", 3);
    b.d
}

/// 135: the 133 design before the stray dust next to the lever (and its floor) was removed.
pub fn v2_135() -> Design {
    let mut b = Builder { d: v2_133() };
    b.d.name = "piston-door-3x3-v2".into();
    b.dust([6, 6, 1]);
    b.d
}

/// 143: A's early input came from the lever through a 21-block route across the front.
pub fn v2_143() -> Design {
    let mut b = Builder { d: v2_135() };
    for p in [
        [-4, -1, 1], [-4, 0, 1], [-5, 0, 1], [-5, 1, 1], [-5, 1, 0], [-5, 2, 0], [-4, 2, 0], [-4, 3, 0],
        [-3, 3, 0], [-3, 4, 0], [-2, 4, 0], [-2, 5, 0],
    ] {
        b.d.blocks.remove(&p);
    }
    b.set([6, 5, -1], FLOOR);
    b.set([6, 6, -1], "redstone_wire"); // next to the lever (front)
    for (p, s) in [
        ([5, 4, -1], FLOOR), ([5, 5, -1], "redstone_wire"), ([4, 3, -1], FLOOR), ([4, 4, -1], "redstone_wire"),
        ([3, 3, -1], FLOOR), ([3, 4, -1], "redstone_wire"), ([3, 2, -2], FLOOR), ([3, 3, -2], "redstone_wire"),
        ([2, 3, -2], FLOOR), ([2, 4, -2], "redstone_wire"), ([1, 3, -2], FLOOR), ([1, 4, -2], "redstone_wire"),
        ([0, 3, -2], FLOOR), ([0, 4, -2], "redstone_wire"), ([-1, 2, -2], FLOOR), ([-1, 3, -2], "redstone_wire"),
        ([-1, 3, -1], FLOOR), ([-1, 4, -1], "redstone_wire"), ([-1, 5, 0], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    b.d
}
