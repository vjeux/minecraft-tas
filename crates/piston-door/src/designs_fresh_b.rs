//! Track fresh-b designs (see designs/piston-door-3x3/tracks/fresh-b/NOTES.md).
//!
//! `sf1`: a slime-free 3x3 door (no slime or honey anywhere).
//! * Side columns: 3 sticky pistons per side pull TL/ML/BL west and TR/MR/BR east. Each side is
//!   powered at rest by one wall torch at y=1 (direct to the middle piston, quasi-connectivity
//!   to the bottom one, and the block above the torch to the top one).
//! * Middle column, no slime: T (sticky, facing down, retracted at (1,3,0)) "pumps": a long
//!   pulse pushes TM, C (and BM if still there) down one cell and the sticky retract pulls TM
//!   back. A (sticky, facing up, retracted at (1,-2,0)) only ever gets 2 gt pulses: the first
//!   pulls BM down to (1,-1,0) (open) or spits C up (close); P_B pulls BM aside to (2,-1,0)
//!   (open) or pushes it back (close); the second A pulse grabs C down to (1,-1,0) (open) or
//!   spits BM + C up into place (close). B (sticky, facing down, at (1,5,0), head on T) lifts T
//!   one cell; a pulse that only reaches T's upper spot makes T grab TM up to (1,3,0). On close
//!   B pushes T and TM back down together.
//! * Every signal is a delayed copy of the lever, the same order on open and close.
use crate::designs::Builder;
use mctas::design::Design;

pub fn sf1() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-fresh-b-sf1".into();
    let stone = "minecraft:stone";
    let f = crate::designs::FLOOR;
    // ---------------- mechanism ----------------
    for x in 0..3 {
        for y in 0..3 {
            b.set([x, y, 0], stone);
        }
    }
    for y in 0..3 {
        b.set([-2, y, 0], "sticky_piston[facing=east,extended=true]");
        b.set([-1, y, 0], "piston_head[facing=east,type=sticky,short=false]");
        b.set([4, y, 0], "sticky_piston[facing=west,extended=true]");
        b.set([3, y, 0], "piston_head[facing=west,type=sticky,short=false]");
    }
    b.set([1, 3, 0], "sticky_piston[facing=down,extended=false]"); // T
    b.set([1, 5, 0], "sticky_piston[facing=down,extended=true]"); // B
    b.set([1, 4, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=false]"); // A
    b.set([3, -1, 0], "sticky_piston[facing=west,extended=true]"); // P_B
    b.set([2, -1, 0], "piston_head[facing=west,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);

    // ---------------- sides ----------------
    // Left: torch (-3,1,0) on SL (-4,1,0); block (-3,2,0) above it powers LP2.
    b.set([-3, 1, 0], "redstone_wall_torch[facing=east,lit=true]");
    b.set([-4, 1, 0], f);
    b.set([-3, 2, 0], f);
    // Right: torch (5,1,0) on SR (6,1,0); block (5,2,0).
    b.set([5, 1, 0], "redstone_wall_torch[facing=west,lit=true]");
    b.set([6, 1, 0], f);
    b.set([5, 2, 0], f);

    // ---------------- left tower: delays for B and T's grab ----------------
    b.set([-3, 3, 0], "redstone_torch[lit=false]"); // L @4
    b.set([-3, 4, 0], f);
    b.set([-3, 5, 0], "redstone_torch[lit=true]"); // NOT L @6
    b.set([-3, 6, 0], f);
    // T's grab (upper spot): repeaters (-2,4,0),(-1,4,0) -> observer OP2 (0,4,0) -> T at (1,4,0).
    b.repeater([-2, 4, 0], "east", 4);
    b.repeater([-1, 4, 0], "east", 4);
    b.set([0, 4, 0], "observer[facing=west]");
    // B: (-3,6,0) -> repeaters -> into the air (1,6,0) above B (quasi-connectivity only, so T in
    // its upper spot is never powered through B's cell).
    b.repeater([-2, 6, 0], "east", 4);
    b.repeater([-1, 6, 0], "east", 1);
    b.repeater([0, 6, 0], "east", 1);

    // ---------------- T's pump (lower spot), watching RP2's head cell ----------------
    // OP1 (3,3,0) watches (3,2,0) (RP2's head, then TR); its output block B1 (3,4,0) lights dust
    // (3,4,-1) -> stretch repeater (2,4,-1) -> dust (1,4,-1) on (1,3,-1), which powers T only in
    // its lower spot (1,3,0). (An observer in RP2's quasi cell (4,3,0) made RP2 oscillate.)
    b.set([3, 3, 0], "observer[facing=down]");
    b.set([3, 4, 0], f);
    b.set([3, 4, -1], "redstone_wire");
    b.set([3, 3, -1], f);
    b.repeater([2, 4, -1], "west", 2);
    b.set([1, 4, -1], "redstone_wire");
    b.set([1, 3, -1], f);

    // ---------------- bottom ----------------
    b.set([1, -2, -2], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, -2, -2]);
    b.set([1, -3, -2], f);
    b.set([1, -2, -1], "observer[facing=north]"); // Q1 watches the lever -> A
    b.set([3, -2, 0], "redstone_torch[lit=true]"); // P_B torch
    b.set([3, -3, 0], f); // SP
    b.set([1, -3, 0], "observer[facing=down]"); // Q2 watches (1,-4,0) -> A
    b.d
}

/// `pd3`: the TOP PUMP door, every piston pulse-driven (retracted at rest), all signals in the
/// same order on open and close (see tracks/fresh-b/NOTES.md, mech3p.sched for the ideal timing).
/// * Top row glued to a slime row (y=3); T (1,4,0) pumps it down one cell and back, which pushes
///   C down into BM's cell; U (0,5,0, gap at (0,4,0)) lifts the slime row + top row + T (open)
///   and pushes them back down with a spit (close).
/// * Sides pushed open by LP (1,-1,-1) and RP (2,-1,0), both pulsed through one block P.
///   The left structure's carrier slime (0,-1,0) drags BM aside at y=-1 (and back on close).
/// * A (1,-2,0) gets two pulses: pull BM down / spit C up, then grab C down / spit BM + C up.
/// Pulses (game ticks after the flip): A1 2, sides 8, T 14..18, A2 16, U 20.
pub fn pd3() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-fresh-b-pd3".into();
    let stone = "minecraft:stone";
    let f = crate::designs::FLOOR;
    // ---------------- mechanism ----------------
    for x in 0..3 {
        for y in 0..3 {
            b.set([x, y, 0], stone);
        }
        b.set([x, 3, 0], "slime_block");
    }
    for p in [[-1, -1, 0], [-1, 0, 0], [-1, 1, 0], [0, -1, 0], [0, -1, -1], [3, -1, 0], [3, 0, 0], [3, 1, 0]] {
        b.set(p, "slime_block");
    }
    b.set([1, -1, -1], "sticky_piston[facing=west,extended=false]"); // LP
    b.set([2, -1, 0], "sticky_piston[facing=east,extended=false]"); // RP
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=false]"); // A
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=false]"); // T
    b.set([0, 5, 0], "sticky_piston[facing=down,extended=false]"); // U
    b.port("door", [0, 0, 0]);

    // ---------------- bottom: lever, A pulses, sides ----------------
    b.set([1, -2, -2], "lever[face=wall,facing=south,powered=false]");
    b.port("lever", [1, -2, -2]);
    b.set([1, -2, -3], stone); // X
    b.set([1, -2, -1], "observer[facing=north]"); // O_A1 watches the lever -> A
    b.dust([1, -3, -2]);
    b.dust([1, -3, -1]);
    b.repeater([2, -3, -1], "east", 3); // R6: L6
    b.set([2, -2, -1], "observer[facing=down]"); // O_S watches R6 -> P
    b.set([2, -1, -1], stone); // P powers LP and RP
    b.set([3, -3, -1], stone); // K: R6 output block
    b.set([3, -2, -1], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.repeater([3, -2, 0], "south", 4); // L14
    b.set([2, -2, 0], "observer[facing=east]"); // O_A2 -> A

    // ---------------- top: tower from X, T pump, U pulse ----------------
    b.set([1, -2, -4], "redstone_wall_torch[facing=north,lit=true]"); // NOT L @2
    b.set([1, -1, -4], stone);
    b.set([1, 0, -4], "redstone_torch[lit=false]"); // L @4
    b.set([1, 1, -4], stone);
    b.set([1, 2, -4], "redstone_torch[lit=true]"); // NOT L @6
    b.set([1, 3, -4], stone);
    b.set([1, 4, -4], "redstone_torch[lit=false]"); // L @8
    b.set([1, 5, -4], stone);
    b.set([1, 6, -4], "redstone_torch[lit=true]"); // NOT L @10
    b.set([1, 4, -3], "observer[facing=north]"); // O_T
    b.repeater([1, 4, -2], "south", 2); // stretch to 4gt
    b.set([1, 4, -1], "minecraft:obsidian"); // powers T in its lower spot only
    b.set([0, 6, -4], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.set([0, 5, -4], f);
    b.set([0, 5, -3], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.set([0, 4, -3], f);
    b.repeater([0, 5, -2], "south", 4); // L18
    b.set([0, 5, -1], "observer[facing=north]"); // O_U -> U
    b.d
}
