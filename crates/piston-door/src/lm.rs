//! logic-min track: v2's mechanism with a minimized control (see
//! designs/piston-door-3x3/tracks/logic-min/NOTES.md).
//!
//! Every piston except P_T is retracted at rest and driven by 2gt observer pulses: an open
//! pulse pulls its block, a close pulse pushes it back and spits it. P_T is the only steady
//! signal: torch = NOT(R1 OR R_P), R1 = lever + 8gt (P_T lets go of T on open), R_P = an 8gt
//! window 2gt ahead of B's (P_T pushes T back in the tick B lifts A on close).

use crate::designs::{Builder, FLOOR};

pub fn names() -> Vec<&'static str> {
    vec!["lm1"]
}

pub fn build(name: &str) -> Option<mctas::design::Design> {
    match name {
        "lm1" => Some(lm1()),
        _ => None,
    }
}

/// The pulse-driven v2 mechanism: door, backbones, all pistons (only P_T extended).
fn mechanism(b: &mut Builder) {
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    b.set([-3, 0, 0], "sticky_piston[facing=east,extended=false]"); // P_L
    b.set([5, 1, 0], "sticky_piston[facing=west,extended=false]"); // P_R
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=false]"); // P_BM
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=false]"); // A
    b.set([1, 5, 0], "sticky_piston[facing=down,extended=false]"); // B
    b.set([1, 3, 2], "sticky_piston[facing=north,extended=true]"); // P_T
    b.set([1, 3, 1], "piston_head[facing=north,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);
}

pub fn lm1() -> mctas::design::Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-lm1".into();
    mechanism(&mut b);
    // Lever on top of O_R, which watches it: pulse at t=2 into K_R (obsidian: next to the right
    // backbone's open cell, so it must be immovable) -> P_R below it, and R_P behind it.
    b.set([5, 3, 0], "observer[facing=up]"); // O_R
    b.set([5, 4, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [5, 4, 0]);
    b.set([5, 2, 0], "obsidian"); // K_R
    // A's first pulse: O_a (below) lights X through the stone (3,4,0).
    b.set([2, 3, 0], "minecraft:stone"); // floor of X (powers A in its low cell)
    b.set([3, 4, 0], "minecraft:stone");
    b.set([2, 4, 0], "redstone_wire[north=side,east=side,south=side,west=side]"); // X
    // B chain: O_a (t=2) -> O_b (t=4) -> block -> dust -> R_B (d4: B window 12..20) -> QC B.
    b.set([4, 4, 0], "observer[facing=east]"); // O_a watches the lever
    b.set([4, 5, 0], "observer[facing=down]"); // O_b watches O_a, fires up into (4,6,0)
    b.set([4, 6, 0], FLOOR);
    b.dust([3, 6, 0]);
    b.set([2, 6, 0], "repeater[facing=east,delay=4]"); // R_B (floor = O)
    b.set([2, 5, 0], "observer[facing=up]"); // O: B edges -> X
    // P_T: torch (2,3,2) on K_P (3,3,2), dust (3,4,2) on K_P is the P_T net.
    b.set([2, 3, 2], "redstone_wall_torch[facing=west,lit=true]");
    b.dust([3, 4, 2]);
    // R1: lever + 8gt.
    b.repeater([5, 4, 1], "south", 4);
    // R_P: K_R pulse (t=2) + 8gt window.
    b.repeater([5, 2, 1], "south", 4);
    // Nets to the P_T dust (laid by hand, to be re-routed): R1 -> (5,4,2) (4,4,2) -> (3,4,2).
    b.dust([5, 4, 2]);
    b.dust([4, 4, 2]);
    // R_P -> the P_T net: laid by `door route`.
    b.dust([4, 3, 3]);
    b.dust([5, 2, 3]);
    b.dust([5, 2, 2]);
    // S: P_BM fires when the right column moves (O watching (2,0,0), output into (2,-2,0)).
    b.set([2, -1, 0], "observer[facing=up]");
    b.set([2, -2, 0], FLOOR);
    // P_L: observer chain watching O_m, ending in a stone that powers a lone dust next to P_L.
    b.set([2, -1, 1], "observer[facing=north]");
    for x in -2..=1 {
        b.set([x, -1, 1], "observer[facing=east]");
    }
    b.set([-3, -1, 1], "minecraft:stone");
    b.set([-3, 0, 1], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.d
}
