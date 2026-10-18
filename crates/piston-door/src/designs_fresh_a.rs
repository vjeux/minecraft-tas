//! Track `fresh-a`: doors designed from first principles (see
//! designs/piston-door-3x3/tracks/fresh-a/NOTES.md).
//! * `fresh-a-mech`: "C-up with capture": A lifts TM, the left slime group captures TM through
//!   the slime at (0,3) when its pusher PL fires, B rides A for C; Q (BM) is a pulse toggle fired
//!   by an observer watching BR; A's rest power goes through a conductor K glued to the left
//!   group (placeholders for the other steady inputs).
//! * `fresh-a-sf1-*`: slime-free: 6 side pullers (extended at rest), Q pulls BM, the A/B dance
//!   lifts C, P_T pulls TM west at y=3. `fresh-a-sf1-w2` is the fully wired, verified door.

use crate::designs::{Builder, FLOOR};
use mctas::design::Design;

pub fn build(name: &str) -> Option<Design> {
    match name {
        "fresh-a-mech" => Some(fresh_a_mech()),
        "fresh-a-sf1-mech" => Some(fresh_a_sf1_mech()),
        "fresh-a-sf1-w1" => Some(fresh_a_sf1_w1()),
        "fresh-a-sf1-w2" => Some(fresh_a_sf1_w2()),
        _ => None,
    }
}

/// fresh-a mechanism ("C-up with capture", see designs/piston-door-3x3/tracks/fresh-a/NOTES.md):
/// A lifts TM, the left group captures TM through the slime at (0,3) when PL pushes it west,
/// B rides A down so A pulls C up twice; Q pulls BM down. The B/A pulse cluster is wired:
/// lever -> O2 (both edges) -> R_B (stretch) -> dust on O -> dust on B; O watches that dust and
/// lights the X dust next to A (A directly when up, through X's floor when down).
pub fn fresh_a_mech() -> Design {
    let mut b = Builder::default();
    b.d.name = "fresh-a-mech".into();
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
    }
    // Left group (pushed west by PL through (0,3,-1)); (0,3) captures TM; K rides along.
    for p in [[-1, 0, 0], [-1, 1, 0], [-1, 2, 0], [-1, 3, 0], [0, 3, 0], [0, 3, -1]] {
        b.set(p, "slime_block");
    }
    b.set([0, 4, 0], stone); // K
    b.set([1, 3, -1], "sticky_piston[facing=west]"); // PL
    // Right group (pushed east by PR).
    for y in 0..4 {
        b.set([3, y, 0], "slime_block");
    }
    b.set([2, 3, 0], "sticky_piston[facing=east]"); // PR
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 5, 0], "sticky_piston[facing=down]"); // B
    b.set([1, -2, 0], "sticky_piston[facing=up]"); // Q (pulse toggle)
    b.set([2, -1, 0], "observer[facing=up]"); // O_Q watches BR
    b.set([2, -2, 0], stone); // C_Q: O_Q's output block, powers Q
    // B/A pulse cluster.
    b.set([1, 5, 4], FLOOR);
    b.set([1, 6, 4], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, 6, 4]);
    b.set([1, 6, 3], "observer[facing=south]"); // O2 watches the lever
    b.repeater([1, 6, 2], "north", 4); // R_B
    b.set([1, 5, 1], "observer[facing=up]"); // O watches the dust on it
    b.set([1, 6, 1], "redstone_wire");
    b.set([1, 6, 0], "redstone_wire"); // on B
    b.set([1, 3, 1], FLOOR);
    b.set([1, 4, 1], "redstone_wire[north=side,east=side,south=side,west=side]"); // X
    // A steady: R_K strongly powers K (only while the left group is home). Placeholder RB = NOT L.
    b.set([0, 4, 1], "repeater[facing=south,delay=1]");
    b.set([0, 3, 1], "minecraft:obsidian"); // floor next to the slime (0,3): must not be dragged
    b.set([0, 4, 2], "redstone_block");
    b.port("door", [0, 0, 0]);
    b.d
}

/// fresh-a SF1 mechanism, slime-free: 6 side pullers, Q (BM), A/B dance (C), P_T (TM west at
/// y=3), dance cluster wired (lever -> O2 -> R_B -> B's QC cell; O watches R_B -> X -> A).
/// Redstone blocks are placeholders for the not-yet-wired steady signals.
pub fn fresh_a_sf1_mech() -> Design {
    let mut b = Builder::default();
    b.d.name = "fresh-a-sf1-mech".into();
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
        b.set([-2, y, 0], "sticky_piston[facing=east,extended=true]");
        b.set([-1, y, 0], "piston_head[facing=east,type=sticky,short=false]");
        b.set([4, y, 0], "sticky_piston[facing=west,extended=true]");
        b.set([3, y, 0], "piston_head[facing=west,type=sticky,short=false]");
    }
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=true]"); // Q
    b.set([1, -1, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 5, 0], "sticky_piston[facing=down]"); // B
    b.set([-1, 3, 0], "sticky_piston[facing=east,extended=true]"); // P_T
    b.set([0, 3, 0], "piston_head[facing=east,type=sticky,short=false]");
    // Dance cluster.
    b.set([1, 5, 3], FLOOR);
    b.set([1, 6, 3], "lever[face=floor,facing=north,powered=false]");
    b.port("lever", [1, 6, 3]);
    b.set([1, 6, 2], "observer[facing=south]"); // O2 watches the lever
    b.set([1, 6, 1], "repeater[facing=south,delay=4]"); // R_B -> (1,6,0), B's QC cell
    b.set([1, 5, 1], "observer[facing=up]"); // O watches R_B
    b.set([1, 3, 1], FLOOR);
    b.set([1, 4, 1], "redstone_wire[north=side,east=side,south=side,west=side]"); // X
    // Placeholders.
    for p in [[-3, 1, 0], [-3, 2, 0], [5, 1, 0], [5, 2, 0], [1, -3, 0], [1, 4, -1], [-1, 3, -1]] {
        b.set(p, "redstone_block");
    }
    b.port("door", [0, 0, 0]);
    b.d
}

/// SF1 wiring stage 1: NOT-L network (torch T_N on the lever's floor) to the side pullers
/// (dust on K blocks: the dust points into the top puller, its K powers the middle puller
/// directly and the bottom one through quasi-connectivity) and to Q.
pub fn fresh_a_sf1_w1() -> Design {
    let mut d = fresh_a_sf1_mech();
    let mut b = Builder { d: std::mem::take(&mut d) };
    b.d.name = "fresh-a-sf1-w1".into();
    for p in [[-3, 1, 0], [-3, 2, 0], [5, 1, 0], [5, 2, 0], [1, -3, 0]] {
        b.d.blocks.remove(&p);
    }
    b.set([0, 5, 3], "redstone_wall_torch[facing=west]"); // T_N on the lever floor (1,5,3)
    b.dust([-1, 5, 3]);
    b.set([2, 5, 3], "redstone_wall_torch[facing=east]"); // T_N2: NOT L for the right side
    b.dust([3, 5, 3]);
    b.set([-3, 1, 0], "minecraft:stone"); // K_L
    b.set([-3, 2, 0], "redstone_wire"); // D_L
    b.set([5, 1, 0], "minecraft:stone"); // K_R
    b.set([5, 2, 0], "redstone_wire"); // D_R
    // Router: NOT-L network from T_N's dust (-1,5,3) to the left (D_L), right (D_R) and Q.
    for (p, s) in [
        ([-2, 3, 3], FLOOR), ([-2, 4, 3], "redstone_wire"), ([-3, 2, 3], FLOOR), ([-3, 3, 3], "redstone_wire"),
        ([-4, 1, 3], FLOOR), ([-4, 2, 3], "redstone_wire"), ([-4, 0, 2], FLOOR), ([-4, 1, 2], "redstone_wire"),
        ([-4, 0, 1], FLOOR), ([-4, 1, 1], "redstone_wire"), ([-4, 1, 0], FLOOR), ([-4, 2, 0], "redstone_wire"),
        ([-3, -1, 1], FLOOR), ([-3, 0, 1], "redstone_wire"), ([-3, -2, 0], FLOOR), ([-3, -1, 0], "redstone_wire"),
        ([-2, -3, 0], FLOOR), ([-2, -2, 0], "redstone_wire"), ([-1, -4, 0], FLOOR), ([-1, -3, 0], "redstone_wire"),
        ([0, -3, 0], FLOOR), ([0, -2, 0], "redstone_wire"),
        // T_N2's dust (3,5,3) to the right (D_R), arriving from the east only.
        ([3, 3, 2], FLOOR), ([3, 4, 2], "redstone_wire"), ([4, 2, 2], FLOOR), ([4, 3, 2], "redstone_wire"),
        ([5, 1, 2], FLOOR), ([5, 2, 2], "redstone_wire"), ([6, 1, 2], FLOOR), ([6, 2, 2], "redstone_wire"),
        ([6, 1, 1], FLOOR), ([6, 2, 1], "redstone_wire"), ([6, 1, 0], FLOOR), ([6, 2, 0], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    b.d
}

/// SF1 wiring stage 2: P_T and A torches on the north side. T_A (1,4,-1) on Y_A (1,4,-2),
/// T_P (-1,3,-1) on Y_P (-1,3,-2). Fast L arrives as dust D_a on Y_A; R6 makes L_6 (N6);
/// R_p1 feeds net M (P_T's NOR input, also fed by the late chain end), R_pin feeds Y_P;
/// the chain end also feeds Y_A late through R_al.
pub fn fresh_a_sf1_w2() -> Design {
    let mut b = Builder { d: fresh_a_sf1_w1() };
    b.d.name = "fresh-a-sf1-w2".into();
    b.d.blocks.remove(&[1, 4, -1]);
    b.d.blocks.remove(&[-1, 3, -1]);
    b.set([1, 4, -2], "minecraft:stone"); // Y_A
    b.set([1, 4, -1], "redstone_wall_torch[facing=south]"); // T_A -> A
    b.set([-1, 3, -2], "minecraft:stone"); // Y_P
    b.set([-1, 3, -1], "redstone_wall_torch[facing=south]"); // T_P -> P_T
    b.set([1, 5, -2], "redstone_wire"); // D_a: fast L on Y_A
    b.repeater([1, 5, -3], "north", 1); // R6
    // A late, WEAK into Y_A (a strong input would leak back into D_a's net and latch):
    // M extended west/south to R_al (on Y_P), which lights dust (0,4,-2) pointing into Y_A.
    b.dust([-2, 4, -4]);
    b.dust([-2, 4, -3]);
    b.dust([-2, 4, -2]);
    b.set([-1, 4, -2], "repeater[facing=west,delay=4]"); // R_al, flow east
    b.dust([0, 4, -2]);
    b.dust([1, 5, -4]); // N6 (floor (1,4,-4) feeds R_p1)
    b.repeater([0, 4, -4], "west", 1); // R_p1
    b.dust([-1, 4, -4]); // M (floor (-1,3,-4) feeds R_pin)
    b.repeater([-1, 3, -3], "south", 1); // R_pin -> Y_P
    // Late chain from N6: R_c1 north, R_c2 west, R_c3 south into M.
    b.repeater([1, 5, -5], "north", 4);
    b.dust([1, 5, -6]);
    b.repeater([0, 5, -6], "west", 4);
    b.dust([-1, 5, -6]); // its floor (-1,4,-6) feeds R_c3
    b.repeater([-1, 4, -5], "south", 4); // -> M (-1,4,-4)
    // Fast L: dust under the lever floor F_L, router path over the top to D_a.
    b.dust([1, 4, 3]);
    for (p, s) in [
        ([1, 3, 4], FLOOR), ([1, 4, 4], "redstone_wire"), ([1, 4, 5], FLOOR), ([1, 5, 5], "redstone_wire"),
        ([0, 5, 5], FLOOR), ([0, 6, 5], "redstone_wire"), ([0, 6, 4], FLOOR), ([0, 7, 4], "redstone_wire"),
        ([0, 7, 3], FLOOR), ([0, 8, 3], "redstone_wire"), ([0, 7, 2], FLOOR), ([0, 8, 2], "redstone_wire"),
        ([0, 7, 1], FLOOR), ([0, 8, 1], "redstone_wire"), ([0, 7, 0], FLOOR), ([0, 8, 0], "redstone_wire"),
        ([0, 6, -1], FLOOR), ([0, 7, -1], "redstone_wire"), ([0, 5, -2], FLOOR), ([0, 6, -2], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    b.d
}
