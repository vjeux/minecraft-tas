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

/// `mp-mech`: the MIRRORED top pump (mechanism only, ideal inputs: mp.sched). The bottom row is
/// glued to a slime row (y=-1) that the pump P (1,-2,0) pushes up one cell and pulls back (C goes
/// up into TM's cell) and U (0,-3,0) lowers at the end (P rides along). A (1,4,0) pulls TM up, the
/// right structure's carrier (2,3,0) drags it east, A pulls C up. Sides pushed from the top.
pub fn mp_mech() -> Design {
    let mut b = Builder::default();
    b.d.name = "fresh-b-mp-mech".into();
    for x in 0..3 {
        for y in 0..3 {
            b.set([x, y, 0], "minecraft:stone");
        }
        b.set([x, -1, 0], "slime_block");
    }
    for p in [[-1, 1, 0], [-1, 2, 0], [-1, 3, 0], [3, 1, 0], [3, 2, 0], [3, 3, 0], [3, 4, 0], [2, 3, 0]] {
        b.set(p, "slime_block");
    }
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=false]"); // P (pump)
    b.set([0, -3, 0], "sticky_piston[facing=up,extended=false]"); // U (lower)
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=false]"); // A
    b.set([0, 3, 0], "sticky_piston[facing=west,extended=false]"); // H
    b.set([2, 4, 0], "sticky_piston[facing=east,extended=false]"); // F
    b.set([4, -4, 0], "minecraft:stone");
    b.set([4, -3, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("door", [0, 0, 0]);
    b.port("lever", [4, -3, 0]);
    b.d
}

/// `mp1-top`: mp-mech with the top wired (bottom P and U still ideal, see mp1-top.sched).
/// Lever (0,6,0) on (0,6,-1). O_H (0,5,0) watches the lever -> H (QC) at 2. O_X (1,6,0) watches the
/// lever, O_A (1,5,0) watches O_X -> A at 4. F chain O_b..O_f -> stone (2,4,-1) -> F at 12.
/// A2: O_h (1,6,-1) watches O_e, O_i.. -> O_g (1,5,-1) -> stone (1,4,-1) -> A.
pub fn mp1_top() -> Design {
    let mut b = Builder::default();
    let m = mp_mech();
    for (p, s) in m.blocks.iter() {
        if s.contains("lever") || *p == [4, -4, 0] {
            continue;
        }
        b.set(*p, s);
    }
    b.d.name = "fresh-b-mp1-top".into();
    let st = "minecraft:stone";
    b.set([0, 6, -1], st);
    b.set([0, 6, 0], "lever[face=wall,facing=south,powered=false]");
    b.port("lever", [0, 6, 0]);
    b.port("door", [0, 0, 0]);
    b.set([0, 5, 0], "observer[facing=up]"); // O_H
    b.set([1, 6, 0], "observer[facing=west]"); // O_X
    b.set([1, 5, 0], "observer[facing=up]"); // O_A
    b.set([2, 6, 0], "observer[facing=west]"); // O_b 4
    b.set([2, 7, 0], "observer[facing=down]"); // O_c 6
    b.set([2, 7, -1], "observer[facing=south]"); // O_d 8
    b.set([2, 6, -1], "observer[facing=up]"); // O_e 10
    b.set([2, 5, -1], "observer[facing=up]"); // O_f 12
    b.set([2, 4, -1], st); // F
    b.set([2, 6, -2], "observer[facing=south]"); // O_p 12 (watches O_e)
    b.set([1, 6, -2], "observer[facing=east]"); // O_q 14
    b.set([1, 6, -1], "observer[facing=north]"); // O_h 16
    b.set([1, 5, -1], "observer[facing=up]"); // O_g 18
    b.set([1, 4, -1], st); // A2
    b.d
}

/// `mp2`: mirrored pump, top wired rb6-style. Lever (1,5,-2) on S (2,5,-2): O_L (1,5,-1) -> A QC;
/// R_F (2,5,-1, d4) -> F QC; dust (0,5,-2) -> dust (0,4,-1) -> block (0,3,-1) -> H steady.
/// A2: O_A2 (2,5,0) watches a flag (3,5,0) riding the right structure -> A QC.
pub fn mp2() -> Design {
    let mut b = Builder::default();
    let m = mp_mech();
    for (p, s) in m.blocks.iter() {
        if s.contains("lever") || *p == [4, -4, 0] {
            continue;
        }
        b.set(*p, s);
    }
    b.d.name = "fresh-b-mp2".into();
    let st = "minecraft:stone";
    let cross = "redstone_wire[north=side,east=side,south=side,west=side]";
    b.set([2, 5, -2], st); // S
    b.set([1, 5, -2], "lever[face=wall,facing=west,powered=false]");
    b.port("lever", [1, 5, -2]);
    b.port("door", [0, 0, 0]);
    b.set([1, 5, -1], "observer[facing=north]"); // O_L -> A
    b.set([2, 5, -1], "repeater[facing=north,delay=4]"); // R_F -> F (QC)
    b.set([2, 4, -1], crate::designs::FLOOR);
    b.set([0, 4, -2], st);
    b.set([0, 5, -2], cross);
    b.set([0, 3, -1], st);
    b.set([0, 4, -1], cross);
    b.set([3, 5, 0], st); // flag
    b.set([2, 5, 0], "observer[facing=east]"); // O_A2
    b.d
}

/// `mp5-mech`: mirrored pump where only BM and the bottom-LEFT corner ride the slime row
/// ((0,-1,0),(1,-1,0)); the right column is 3 high. The pump can then run BEFORE the right side
/// opens (it only needs TM up and the left side open): A1, H, pump, F, A2 = F+2, U.
pub fn mp5_mech() -> Design {
    let mut b = Builder::default();
    b.d.name = "fresh-b-mp5-mech".into();
    for x in 0..3 {
        for y in 0..3 {
            b.set([x, y, 0], "minecraft:stone");
        }
    }
    for p in [[0, -1, 0], [1, -1, 0], [-1, 1, 0], [-1, 2, 0], [-1, 3, 0], [3, 0, 0], [3, 1, 0], [3, 2, 0], [3, 3, 0], [3, 4, 0], [2, 3, 0]] {
        b.set(p, "slime_block");
    }
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=false]"); // P (pump)
    b.set([0, -3, 0], "sticky_piston[facing=up,extended=false]"); // U (lower)
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=false]"); // A
    b.set([0, 3, 0], "sticky_piston[facing=west,extended=false]"); // H
    b.set([2, 4, 0], "sticky_piston[facing=east,extended=false]"); // F
    b.set([5, -4, 0], "minecraft:stone");
    b.set([5, -3, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("door", [0, 0, 0]);
    b.port("lever", [5, -3, 0]);
    b.d
}

/// `mp6`: the MIRRORED PUMP door, wired with motion triggers ("flags": plain blocks glued to a
/// moving slime structure, watched by an observer that is not itself next to any slime).
/// Order (same on open and close): A1, H | pump | F | A2 = F+2 | U.
/// * Lever (0,3,-1) on the ceiling S' (0,4,-1): powers H directly; S' feeds R_F (d4) -> F at 8.
/// * Left structure moves at 1: top-left flag (-1,4,0) -> O (0,4,0) -> A (A1, TM up);
///   bottom-left flag (-1,1,-1) -> O_lf -> Y -> dust -> X -> R_P (d2, 4gt) -> slime cell (0,-1,0)
///   -> quasi-powers the pump P (0,-2,0): bottom row (BM + left corner) up, C into TM's cell.
/// * Right structure moves at 8 (drags TM east): top-right flag (3,5,0) -> O_a (2,5,0) -> A QC
///   (A2, C up); bottom-right flag (3,-1,0) -> O_rf -> O2 -> O3 -> U (1,-3,0) lowers the row.
pub fn mp6() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-fresh-b-mp6".into();
    let st = "minecraft:stone";
    let cross = "redstone_wire[north=side,east=side,south=side,west=side]";
    for x in 0..3 {
        for y in 0..3 {
            b.set([x, y, 0], st);
        }
    }
    for p in [[0, -1, 0], [1, -1, 0], [-1, 1, 0], [-1, 2, 0], [-1, 3, 0], [3, 0, 0], [3, 1, 0], [3, 2, 0], [3, 3, 0], [3, 4, 0], [2, 3, 0]] {
        b.set(p, "slime_block");
    }
    b.set([0, -2, 0], "sticky_piston[facing=up,extended=false]"); // P (pump)
    b.set([1, -3, 0], "sticky_piston[facing=up,extended=false]"); // U (lower)
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=false]"); // A
    b.set([0, 3, 0], "sticky_piston[facing=west,extended=false]"); // H
    b.set([2, 4, 0], "sticky_piston[facing=east,extended=false]"); // F
    b.port("door", [0, 0, 0]);
    // top
    b.set([0, 4, -1], st); // S'
    b.set([0, 3, -1], "lever[face=ceiling,facing=north,powered=false]");
    b.port("lever", [0, 3, -1]);
    b.set([-1, 4, 0], st); // top-left flag
    b.set([0, 4, 0], "observer[facing=west]"); // O -> A1
    b.set([1, 4, -1], "repeater[facing=west,delay=4]"); // R_F
    b.set([1, 3, -1], crate::designs::FLOOR);
    b.set([2, 4, -1], st); // F-block
    b.set([3, 5, 0], st); // top-right flag
    b.set([2, 5, 0], "observer[facing=east]"); // O_a -> A2
    // bottom: pump
    b.set([-1, 1, -1], st); // bottom-left flag
    b.set([-1, 0, -1], "observer[facing=up]"); // O_lf
    b.set([-1, -1, -1], st); // Y
    b.set([-2, -1, -1], cross);
    b.set([-2, -2, -1], st);
    b.set([-2, -1, 0], st); // X
    b.set([-1, -1, 0], "repeater[facing=west,delay=2]"); // R_P -> slime cell (0,-1,0)
    b.set([-1, -2, 0], "minecraft:obsidian");
    // bottom: lower
    b.set([3, -1, 0], st); // bottom-right flag
    b.set([3, -2, 0], "observer[facing=up]"); // O_rf
    b.set([3, -3, 0], "observer[facing=up]"); // O2
    b.set([2, -3, 0], "observer[facing=east]"); // O3 -> U
    b.d
}
