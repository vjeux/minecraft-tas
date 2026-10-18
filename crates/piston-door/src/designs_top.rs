//! Track `top-control`: every control near the top of the door.
//!
//! Mechanism (plane z=0 unless noted), same "UP dance" middle column as v2, but:
//! * the side pistons sit high: P_L (-3,4,0) pulls the left backbone by its top slime, P_R
//!   (5,2,0) pulls the right backbone;
//! * the left backbone runs y=-2..4 and carries two redstone blocks: (0,4,0) powers A while the
//!   left column is closed, (0,-2,0) powers the bottom-middle piston P_BM (1,-2,0);
//! * the right backbone has a slime arm (3,2,1),(3,2,2),(3,3,2) carrying a redstone block at
//!   (2,3,2) that powers P_T (1,3,2) while the right column is closed.
//!
//! So A, P_T and P_BM need no wiring of their own: the left column's window (opens first,
//! closes last) drives A and P_BM, the right column's window (opens after A lifted T, closes
//! after B's dance and before A pushes T back) drives P_T.

use crate::designs::{Builder, FLOOR};
use mctas::design::Design;

pub fn names() -> Vec<&'static str> {
    vec!["v3-mech", "v3a-skel"]
}

pub fn build(name: &str) -> Option<Design> {
    match name {
        "v3-mech" => Some(v3_mech()),
        "v3a-skel" => Some(v3a_skel()),
        "v3b-skel" => Some(v3b_skel()),
        "v3b" => Some(v3b()),
        "v3c" => Some(v3c()),
        _ => None,
    }
}

pub const PL: [i32; 3] = [-3, 4, 0];
pub const PR: [i32; 3] = [5, 2, 0];

/// The moving parts of v3 (no control): door, backbones, the six pistons, and the B/O/X core.
pub fn mech(b: &mut Builder) {
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
        b.set([3, y, 0], "slime_block");
    }
    // Left backbone y=-2..4 with its two redstone blocks.
    for y in -2..5 {
        b.set([-1, y, 0], "slime_block");
    }
    b.set([0, 4, 0], "redstone_block"); // powers A
    b.set([0, -2, 0], "redstone_block"); // powers P_BM
    // Right backbone arm carrying P_T's redstone block.
    b.set([3, 2, 1], "slime_block");
    b.set([3, 2, 2], "slime_block");
    b.set([3, 3, 2], "slime_block");
    b.set([2, 3, 2], "redstone_block"); // powers P_T
    // Pistons.
    b.set(PL, "sticky_piston[facing=east,extended=true]"); // P_L
    b.set([PL[0] + 1, PL[1], 0], "piston_head[facing=east,type=sticky,short=false]");
    b.set(PR, "sticky_piston[facing=west,extended=true]"); // P_R
    b.set([PR[0] - 1, PR[1], 0], "piston_head[facing=west,type=sticky,short=false]");
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=true]"); // P_BM
    b.set([1, -1, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 5, 0], "sticky_piston[facing=down,extended=false]"); // B
    b.set([1, 3, 2], "sticky_piston[facing=north,extended=true]"); // P_T
    b.set([1, 3, 1], "piston_head[facing=north,type=sticky,short=false]");
    // B/A pulse core (as v2): R_B QC-powers B through (1,6,0); O watches R_B and lights X.
    b.set([2, 5, 0], "observer[facing=up,powered=false]"); // O
    b.set([2, 6, 0], "repeater[facing=east,delay=4]"); // R_B (input at (3,6,0))
    b.set([2, 3, 0], stone);
    b.set([2, 4, 0], "redstone_wire[north=side,east=side,south=side,west=side]"); // X
    b.port("door", [0, 0, 0]);
}

/// Mechanism only, side pistons held by redstone blocks (drive it with `door drive`).
pub fn v3_mech() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-v3-mech".into();
    mech(&mut b);
    b.set([PL[0] - 1, PL[1], 0], "redstone_block"); // holds P_L
    b.set([PR[0] + 1, PR[1], 0], "redstone_block"); // holds P_R
    b.set([9, 9, 9], FLOOR);
    b.set([9, 10, 9], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [9, 10, 9]);
    b.d
}

/// v3a control block, all on the right (wired further with `door route`):
/// * lever (5,6,0) on LF (5,5,0); W (4,6,0) -> O2 (3,6,0) -> R_B: the v2 B chain;
/// * TL, P_L's NOT: a torch on LF's north face; LF also gets L24 through rLF, so TL is dark
///   from L0 to L24 (+2): the crossing carries TL's output to P_L;
/// * delay chain off the lever's side: r1 (L8), r2, r3 (L24);
/// * TR (5,3,0), P_R's NOT, on XR (6,3,0): XR gets L8 (dust on top) and L24 (rXR).
pub fn v3a_skel() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-v3a".into();
    mech(&mut b);
    b.set([5, 5, 0], FLOOR); // LF
    b.set([5, 6, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [5, 6, 0]);
    b.repeater([4, 6, 0], "west", 1); // W
    b.set([3, 6, 0], "observer[facing=east,powered=false]"); // O2
    b.set([5, 5, -1], "redstone_wall_torch[facing=north,lit=true]"); // TL on LF
    b.dust([4, 5, -1]); // crossing start, lit by TL
    b.repeater([6, 6, 0], "east", 4); // r1, input = the lever
    b.dust([7, 6, 0]); // N8
    b.repeater([8, 6, 0], "east", 4); // r2
    b.repeater([9, 6, 0], "east", 4); // r3
    b.dust([10, 6, 0]); // N24
    b.repeater([5, 5, 1], "north", 1); // rLF: L24 into LF
    // routed: rLF input (5,5,2) <- N24
    b.set([5, 3, 0], "redstone_wall_torch[facing=west,lit=true]"); // TR, powers P_R below
    b.set([6, 3, 0], FLOOR); // XR
    // routed: dust (6,4,0) on XR <- N8
    b.repeater([7, 3, 0], "west", 1); // rXR: L24 into XR
    // routed: rXR input (8,3,0) <- N24
    b.d
}

/// v3b: TL (P_L's NOT on the lever block LF) gives W_out = NOT(L0 or L24); a dust line at
/// y=6, z=-1 (one repeater over B's quasi-connected cell) carries it across the top to P_L.
/// P_R takes OR(W_out, W_out delayed 6gt) right at the piston (dust on one face, a repeater on
/// another), so it opens 6gt after P_L and closes with it (P_L's crossing is a bit slower).
/// L24 = the lever through three 4-tick repeaters (r1 south, r2 east, r3 north) into LF's east
/// face. B fires at 14 (W delay 2).
pub fn v3b_base(b: &mut Builder) {
    b.d.name = "piston-door-3x3-v3b".into();
    mech(b);
    b.set([5, 5, 0], FLOOR); // LF
    b.set([5, 6, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [5, 6, 0]);
    b.repeater([4, 6, 0], "west", 2); // W
    b.set([3, 6, 0], "observer[facing=east,powered=false]"); // O2
    b.set([5, 5, -1], "redstone_wall_torch[facing=north,lit=true]"); // TL on LF
    // L24 chain: r1 (5,6,1) south -> block (5,6,2) -> r2 (6,6,2) east -> block (7,6,2) ->
    // r3 (7,6,1) north -> dust (7,6,0) -> dust (6,5,0) pointing west into LF.
    b.repeater([5, 6, 1], "south", 4);
    b.set([5, 6, 2], FLOOR);
    b.repeater([6, 6, 2], "east", 4);
    b.set([7, 6, 2], FLOOR);
    b.repeater([7, 6, 1], "north", 4);
    b.dust([7, 6, 0]);
    b.dust([6, 5, 0]);
    // Crossing (W_out) to P_L.
    b.dust([4, 5, -1]);
    b.dust([3, 6, -1]);
    b.dust([2, 6, -1]);
    b.repeater([1, 6, -1], "west", 1);
    b.dust([0, 6, -1]);
    b.dust([-1, 6, -1]);
    b.dust([-2, 6, -1]);
    b.dust([-3, 6, -1]);
    b.dust([-3, 6, 0]); // floor (-3,5,0) powers P_L below
    // P_R: repeater (6,2,0) into P_R (W_out delayed); its input (7,2,0) and the dust (5,2,-1)
    // into P_R are routed from the W_out net.
    b.repeater([6, 2, 0], "west", 3);
}

pub fn v3b_skel() -> Design {
    let mut b = Builder::default();
    v3b_base(&mut b);
    b.d
}

pub fn v3b() -> Design {
    let mut b = Builder::default();
    v3b_base(&mut b);
    for (p, s) in [
        ([4, 3, -2], FLOOR), ([4, 4, -2], "redstone_wire"), ([5, 2, -2], FLOOR), ([5, 3, -2], "redstone_wire"),
        ([5, 1, -1], FLOOR), ([5, 2, -1], "redstone_wire"),
        ([6, 1, -2], FLOOR), ([6, 2, -2], "redstone_wire"), ([6, 0, -1], FLOOR), ([6, 1, -1], "redstone_wire"),
        ([7, 0, -1], FLOOR), ([7, 1, -1], "redstone_wire"), ([7, 1, 0], FLOOR), ([7, 2, 0], "redstone_wire"),
    ] {
        b.set(p, s);
    }
    b.d
}

/// v3c: v3b with the timings fixed (see NOTES.md): late chain of four 4-tick repeaters (L32),
/// crossing repeater 4gt (P_L = W_out + 4), P_R = OR(W_out, W_out + 8) right at the piston.
pub fn v3c() -> Design {
    let mut b = Builder::default();
    mech(&mut b);
    b.d.name = "piston-door-3x3-v3c".into();
    b.set([5, 5, 0], FLOOR); // LF
    b.set([5, 6, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [5, 6, 0]);
    b.repeater([4, 6, 0], "west", 2); // W
    b.set([3, 6, 0], "observer[facing=east,powered=false]"); // O2
    b.set([5, 5, -1], "redstone_wall_torch[facing=north,lit=true]"); // TL on LF
    // L32 chain into LF's east face.
    b.repeater([5, 6, 1], "south", 4);
    b.repeater([5, 6, 2], "south", 4);
    b.set([5, 6, 3], FLOOR);
    b.repeater([6, 6, 3], "east", 4);
    b.set([7, 6, 3], FLOOR);
    b.repeater([7, 6, 2], "north", 4);
    b.dust([7, 6, 1]);
    b.dust([7, 6, 0]);
    b.dust([6, 5, 0]);
    // Crossing (W_out) to P_L.
    b.dust([4, 5, -1]);
    b.dust([3, 6, -1]);
    b.dust([2, 6, -1]);
    b.repeater([1, 6, -1], "west", 2);
    b.dust([0, 6, -1]);
    b.dust([-1, 6, -1]);
    b.dust([-2, 6, -1]);
    b.dust([-3, 6, -1]);
    b.dust([-3, 6, 0]); // floor (-3,5,0) powers P_L below
    // P_R: W_out direct (dust (6,3,0) on (6,2,0)) and delayed 8gt (repeater (5,2,-1)).
    b.dust([4, 4, -2]);
    b.dust([5, 3, -2]);
    b.repeater([5, 2, -1], "south", 4);
    b.dust([6, 3, -2]);
    b.dust([6, 3, -1]);
    b.dust([6, 3, 0]);
    b.d
}
