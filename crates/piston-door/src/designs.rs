//! Hand-built door designs, written as code so wiring is easy to iterate on, then exported to
//! the text format with `mctas door gen <name>`.

use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::Pos;

/// Small builder: blocks by absolute position, later writes win.
#[derive(Default)]
pub struct Builder {
    pub d: Design,
}

pub const FLOOR: &str = "minecraft:smooth_stone";

impl Builder {
    pub fn set(&mut self, p: Pos, state: &str) -> &mut Self {
        let s = BlockState::parse(state).expect("bad state").to_string();
        self.d.blocks.insert(p, s);
        self
    }
    /// Dust at `p` with a floor block under it.
    pub fn dust(&mut self, p: Pos) -> &mut Self {
        self.set(p, "redstone_wire");
        self.set([p[0], p[1] - 1, p[2]], FLOOR)
    }
    /// A repeater at `p` (with floor) whose signal flows toward `out` (an axis direction).
    pub fn repeater(&mut self, p: Pos, flow: &str, delay: u8) -> &mut Self {
        // A repeater's `facing` points at its input, i.e. against the flow.
        let facing = match flow {
            "east" => "west",
            "west" => "east",
            "north" => "south",
            "south" => "north",
            _ => panic!("bad flow"),
        };
        self.set(p, &format!("repeater[facing={facing},delay={delay}]"));
        self.set([p[0], p[1] - 1, p[2]], FLOOR)
    }
    /// Straight dust run from `a` to `b` (inclusive) along one axis, floors included.
    pub fn dust_run(&mut self, a: Pos, b: Pos) -> &mut Self {
        let step = [(b[0] - a[0]).signum(), (b[1] - a[1]).signum(), (b[2] - a[2]).signum()];
        let mut p = a;
        loop {
            self.dust(p);
            if p == b {
                break;
            }
            p = [p[0] + step[0], p[1] + step[1], p[2] + step[2]];
        }
        self
    }
    pub fn port(&mut self, name: &str, p: Pos) -> &mut Self {
        self.d.ports.insert(name.to_string(), p);
        self
    }
}

pub fn names() -> Vec<&'static str> {
    vec!["v1", "v2"]
}

pub fn build(name: &str) -> Option<Design> {
    match name {
        "v1" => Some(v1()),
        "v2" => Some(v2()),
        "rb1" => Some(rb1()),
        "rb2" => Some(rb2()),
        "rb3" => Some(rb3()),
        "rb5" => Some(rb5()),
        "rb6" => Some(rb6()),
        "rb4-bench" => Some(rb4_bench()),
        "rb1-base" => Some(rb1_route_base()),
        "v2-133" => Some(crate::designs_history::v2_133()),
        "v2-135" => Some(crate::designs_history::v2_135()),
        "v2-143" => Some(crate::designs_history::v2_143()),
        other => crate::lm::build(other),
    }
}

/// v1: every door block moves one cell except the center, which rides up two cells with the
/// top-middle slime block.
///
/// Mechanics (plane z=0):
/// * left/right columns: one sticky piston each pulls a slime "backbone" column sideways,
///   dragging the three door blocks;
/// * bottom-middle: pulled down by a sticky piston;
/// * center C (stone) + top-middle T (slime) go up two: A retracts (pair up 1), B retracts
///   (lifts the retracted A), an observer pulse makes A extend and retract (pair up 1 more).
///   Closing: B extends (pushes A, T, C down 1) and A, landing on its powered spot, extends
///   (pushes T, C down 1 more). The sides close last so T never drags the corners.
///
/// Control (L = lever, Ln = L delayed ~n game ticks; torches invert):
/// * sides + bottom (high when closed) = NOT(L OR L_late) -> torches under the three pistons
/// * A steady power = NOT(L_a) -> torch next to A's closed spot
/// * B = NOT(L_b) -> torch next to B
/// * A's second pull: observer watching a repeater on the L_c line, pulsing into A's raised spot
pub fn v1() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-v1".into();
    let stone = "minecraft:stone";
    // ---------------- mechanics (plane z=0) ----------------
    for y in 0..3 {
        b.set([0, y, 0], stone);
        b.set([2, y, 0], stone);
        b.set([-1, y, 0], "slime_block");
        b.set([3, y, 0], "slime_block");
    }
    b.set([1, 0, 0], stone); // bottom-middle
    b.set([1, 1, 0], stone); // center
    b.set([1, 2, 0], "slime_block"); // top-middle
    b.set([-3, 0, 0], "sticky_piston[facing=east,extended=true]");
    b.set([-2, 0, 0], "piston_head[facing=east,type=sticky,short=false]");
    b.set([5, 0, 0], "sticky_piston[facing=west,extended=true]");
    b.set([4, 0, 0], "piston_head[facing=west,type=sticky,short=false]");
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=true]");
    b.set([1, -1, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 6, 0], "sticky_piston[facing=down,extended=true]"); // B
    b.set([1, 5, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.port("door", [0, 0, 0]);

    // ---------------- control ----------------
    // Lever on top of (8,7,5); trunk dust (7..1, 8, 5) carries L (undelayed).
    b.set([8, 7, 5], FLOOR);
    b.set([8, 8, 5], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [8, 8, 5]);
    b.dust_run([7, 8, 5], [1, 8, 5]);

    // B is powered ONLY through quasi-connectivity (torch next to the air above it), so A,
    // parked right under B, never sees B's power. B torch at (1,7,1), attachment (1,7,2) <-
    // repeaters (1,7,3),(1,7,4) <- trunk floor (1,7,5).
    b.set([1, 7, 1], "redstone_wall_torch[facing=north,lit=true]");
    b.set([1, 7, 2], FLOOR);
    b.repeater([1, 7, 3], "north", 2);
    b.repeater([1, 7, 4], "north", 2);

    // A: torch (1,4,1) powers A's closed spot; attachment (1,4,2) <- repeaters <- (1,4,5),
    // fed by a staircase from the trunk: (5,8,6) (4,7,6) (3,6,6) (2,5,6) (1,4,6).
    b.set([1, 4, 1], "redstone_wall_torch[facing=north,lit=true]");
    b.set([1, 4, 2], FLOOR);
    b.repeater([1, 4, 3], "north", 1);
    b.repeater([1, 4, 4], "north", 1);
    b.dust([1, 4, 5]);
    for i in 0..5 {
        b.dust([5 - i, 8 - i, 6]);
    }

    // A's second pull: observer (1,5,1) watches repeater (1,5,2), pulses into (1,5,0), where A
    // sits once B has lifted it. L_c: (2,5,6) -> (2,5,5) -> repeaters -> (2,5,2).
    b.set([1, 5, 1], "observer[facing=south,powered=false]");
    b.set([1, 5, 2], "repeater[facing=east,delay=1]");
    b.dust([2, 5, 2]);
    b.repeater([2, 5, 3], "north", 1);
    b.repeater([2, 5, 4], "north", 4);
    b.dust([2, 5, 5]);

    // Sides + bottom: torches under the three pistons, attachments on one dust line.
    b.set([-3, -1, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.set([5, -1, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.set([1, -3, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.dust([-3, 0, 1]); // floor = left torch attachment
    b.dust([-2, -1, 1]);
    b.dust_run([-1, -2, 1], [1, -2, 1]); // (1,-3,1) = bottom torch attachment
    b.repeater([2, -2, 1], "west", 1);
    b.dust([3, -2, 1]);
    b.dust([4, -1, 1]);
    b.dust_run([5, 0, 1], [6, 0, 1]); // (5,-1,1) = right torch attachment
    // L from the lever block down a staircase: (8,6,5) (8,5,4) (8,4,3) (8,3,2) (8,2,1) (7,1,1).
    for i in 0..5 {
        b.dust([8, 6 - i, 5 - i]);
    }
    b.dust([7, 1, 1]);
    // L_late: lever block -> (9,7,5) -> 3 x 4-tick repeaters -> back into (9,5,4) next to the
    // staircase dust (8,5,4). Keeps the sides open until the middle column is back.
    b.dust([9, 7, 5]);
    b.repeater([10, 7, 5], "east", 4);
    b.repeater([11, 7, 5], "east", 4);
    b.repeater([12, 7, 5], "east", 4);
    b.dust([13, 7, 5]);
    b.dust([13, 6, 4]);
    b.dust_run([12, 5, 4], [9, 5, 4]);
    b.d
}

/// v2: all nine door blocks are plain stone ("UP dance", see docs/piston-door-3x3.md).
/// Mechanism: side columns ride slime backbones (P_L, P_R), bottom-middle drops (P_BM), the
/// top-middle T is lifted by A then pulled behind the wall by P_T, and the center C climbs two
/// cells by A riding B: B pushes A down, an observer pulse makes A pull C up, B lifts A, a
/// second pulse makes A pull C again. Closing runs the dance backwards (B pushes A and C down
/// together, A spits C), P_T pushes T back and A pushes it into the door.
pub fn v2() -> Design {
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
    b.set([5, 5, 0], FLOOR);
    b.set([5, 6, 0], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [5, 6, 0]);
    b.repeater([4, 6, 0], "west", 1); // W
    b.set([3, 6, 0], "observer[facing=east,powered=false]"); // O2
    b.set([2, 5, 0], "observer[facing=up,powered=false]"); // O
    b.set([2, 6, 0], "repeater[facing=east,delay=4]"); // R_B: stretches the O2 pulse to 8gt (floor = O)
    b.dust([5, 4, 0]); // under the lever block: feeds the sides line
    b.set([2, 3, 0], stone);
    b.set([2, 4, 0], "redstone_wire[north=side,east=side,south=side,west=side]"); // X

    // Sides + bottom-middle: NOT lever, torches under/behind the three pistons.
    b.set([-3, -1, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.set([5, -1, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.set([1, -3, 0], "redstone_wall_torch[facing=north,lit=true]");
    b.dust([6, 4, 0]);
    for (p, s) in [
        ([8, 2, 2], FLOOR), ([8, 3, 2], "redstone_wire"), ([8, 1, 1], FLOOR), ([8, 2, 1], "redstone_wire"),
        ([7, 0, 1], FLOOR), ([7, 1, 1], "redstone_wire"), ([6, -1, 1], FLOOR), ([6, 0, 1], "redstone_wire"),
    ] {
        b.set(p, s); // router: down the back-right corner to the sides line
    }
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
    // A early: a branch of the sides line ends in a repeater into the A torch attachment.
    b.repeater([-1, 4, -1], "south", 1);
    for (p, s) in [
        ([-1, -3, 0], "minecraft:smooth_stone"),
        ([-1, -2, 0], "minecraft:redstone_wire"),
        ([-1, -2, -1], "minecraft:smooth_stone"),
        ([-1, -1, -1], "minecraft:redstone_wire"),
        ([-1, -1, -2], "minecraft:smooth_stone"),
        ([-1, 0, -2], "minecraft:redstone_wire"),
        ([-2, 0, -2], "minecraft:smooth_stone"),
        ([-2, 1, -2], "minecraft:redstone_wire"),
        ([-2, 1, -3], "minecraft:smooth_stone"),
        ([-2, 2, -3], "minecraft:redstone_wire"),
        ([-1, 2, -3], "minecraft:smooth_stone"),
        ([-1, 3, -3], "minecraft:redstone_wire"),
        ([-1, 3, -2], "minecraft:smooth_stone"),
        ([-1, 4, -2], "minecraft:redstone_wire"),
    ] {
        b.set(p, s);
    }
    // P_T power: torch (2,3,2) on (3,3,2), lit = NOR(L_6, L_28).
    b.set([2, 3, 2], "redstone_wall_torch[facing=west,lit=true]");
    b.dust([3, 4, 2]); // floor (3,3,2) = P_T torch attachment
    // L_6: R1 off the lever, then down to the repeater feeding the P_T net.
    b.dust([6, 4, 1]); // joins the sides line: the P_T nets hang off L directly
    b.dust([6, 4, 2]);
    b.dust([5, 4, 2]);
    b.repeater([4, 4, 2], "west", 4);
    // Late chain: L_6 + 22gt = L_28 into the P_T net, + 8gt more into the A net.
    b.dust([7, 4, 2]);
    b.dust([7, 4, 3]);
    b.repeater([6, 4, 3], "west", 4);
    b.repeater([5, 4, 3], "west", 4);
    b.repeater([4, 4, 3], "west", 4);
    b.dust([3, 4, 3]); // joins the P_T net
    // A late: the P_T net through a repeater, routed to the repeater into the A torch attachment.
    // Two repeaters: a powered block or a dust floor at (1,4,3)/(1,3,3) would power P_T.
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

/// rb1 ("movable power", track redstone-block): v2's UP dance with power carried by moving
/// blocks. Both side structures are PUSHED open (pistons off at rest, no torches). One redstone
/// block RB_A, shuttled by the lever-driven sticky piston Q, powers A at rest and, moved, powers
/// P_R: A lifts T first, then P_R's structure drags T out of the column (its top slime touches T),
/// so P_T is gone. On close Q pulls RB_A back: P_R retracts (pushes T back) and A's extension
/// waits for T to land. BM is powered by a redstone block riding the left structure.
pub fn rb1_mech(b: &mut Builder) {
    let stone = "minecraft:stone";
    for y in 0..3 {
        for x in 0..3 {
            b.set([x, y, 0], stone);
        }
    }
    // Left structure S1, pushed west by P_L (top), carries RB_BM.
    for y in -2..=4 {
        b.set([-1, y, 0], "slime_block");
    }
    b.set([0, -2, 0], "redstone_block"); // RB_BM: powers BM at rest
    b.set([0, 4, 0], "sticky_piston[facing=west,extended=false]"); // P_L
    // Right structure S2, pushed east by P_R; (2,3,0) drags T.
    for y in 0..=4 {
        b.set([3, y, 0], "slime_block");
    }
    b.set([2, 3, 0], "slime_block");
    b.set([2, 4, 0], "sticky_piston[facing=east,extended=false]"); // P_R
    b.set([1, -2, 0], "sticky_piston[facing=up,extended=true]"); // BM
    b.set([1, -1, 0], "piston_head[facing=up,type=sticky,short=false]");
    b.set([1, 4, 0], "sticky_piston[facing=down,extended=true]"); // A
    b.set([1, 3, 0], "piston_head[facing=down,type=sticky,short=false]");
    b.set([1, 5, 0], "sticky_piston[facing=down,extended=false]"); // B
    b.set([0, 4, -1], "sticky_piston[facing=east,extended=false]"); // Q
    b.set([1, 4, -1], "redstone_block"); // RB_A
    b.port("door", [0, 0, 0]);
    // A pulses: R_B QC-powers B through (1,6,0); O watches R_B and lights X (both A heights).
    b.set([1, 3, 1], FLOOR);
    b.set([1, 4, 1], "redstone_wire[north=side,east=side,south=side,west=side]"); // X
    b.set([1, 5, 1], "observer[facing=up]"); // O
    b.set([1, 6, 1], "repeater[facing=south,delay=4]"); // R_B (input from (1,6,2))
    b.set([1, 6, 2], "observer[facing=south]"); // O2 watches (1,6,3)
}


/// rb1 skeleton plus the first lever dust, for routing.
pub fn rb1_route_base() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-rb1".into();
    rb1_mech(&mut b);
    b.set([0, 5, -1], FLOOR);
    b.set([0, 6, -1], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [0, 6, -1]);
    for z in [-2, -3, -4] {
        b.set([0, 4, z], FLOOR);
        b.set([0, 5, z], "repeater[facing=north,delay=4]");
    }
    b.dust([-1, 6, -1]);
    b.d
}
pub fn rb1() -> Design {
    let mut d = rb1_route_base();
    let mut b = Builder { d: std::mem::take(&mut d) };
    // router: lever net -> late chain input
    b.set([-2, 5, -1], "minecraft:smooth_stone");
    b.set([-2, 6, -1], "minecraft:redstone_wire");
    b.set([-2, 6, 0], "minecraft:smooth_stone");
    b.set([-2, 7, 0], "minecraft:redstone_wire");
    b.set([-2, 5, 1], "minecraft:smooth_stone");
    b.set([-2, 6, 1], "minecraft:redstone_wire");
    b.set([-2, 4, 2], "minecraft:smooth_stone");
    b.set([-2, 5, 2], "minecraft:redstone_wire");
    b.set([-2, 3, 3], "minecraft:smooth_stone");
    b.set([-2, 4, 3], "minecraft:redstone_wire");
    b.set([-1, 3, 3], "minecraft:smooth_stone");
    b.set([-1, 4, 3], "minecraft:redstone_wire");
    b.set([0, 4, 3], "minecraft:smooth_stone");
    b.set([0, 5, 3], "minecraft:redstone_wire");
    b.set([1, 5, 3], "minecraft:smooth_stone");
    b.set([1, 6, 3], "minecraft:redstone_wire");
    b.set([-2, 5, -1], "minecraft:smooth_stone");
    b.set([-2, 6, -1], "minecraft:redstone_wire");
    b.set([-2, 4, -2], "minecraft:smooth_stone");
    b.set([-2, 5, -2], "minecraft:redstone_wire");
    b.set([-2, 3, -3], "minecraft:smooth_stone");
    b.set([-2, 4, -3], "minecraft:redstone_wire");
    b.set([-2, 2, -4], "minecraft:smooth_stone");
    b.set([-2, 3, -4], "minecraft:redstone_wire");
    b.set([-2, 2, -5], "minecraft:smooth_stone");
    b.set([-2, 3, -5], "minecraft:redstone_wire");
    b.set([-1, 3, -5], "minecraft:smooth_stone");
    b.set([-1, 4, -5], "minecraft:redstone_wire");
    b.set([0, 4, -5], "minecraft:smooth_stone");
    b.set([0, 5, -5], "minecraft:redstone_wire");
    b.d
}

/// rb2: rb1 with short nets. O2 sits next to the lever (watching it) and feeds R_B through three
/// dust; Q's late hold is a 3-repeater chain into Q from the north, fed by dust from the lever.
pub fn rb2() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-rb2".into();
    rb1_mech(&mut b);
    b.d.blocks.remove(&[1, 6, 2]); // O2 moves next to the lever
    b.set([0, 5, -1], FLOOR); // X_L
    b.set([0, 6, -1], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [0, 6, -1]);
    // B trigger: O2 watches the lever, its back lights (0,6,1) -> (0,6,2) -> (1,6,2) -> R_B.
    b.set([0, 6, 0], "observer[facing=north]");
    b.dust([0, 6, 1]);
    b.dust([0, 6, 2]);
    b.dust([1, 6, 2]);
    // Q late hold: 3 x 4-tick repeaters into Q from the north, fed from the lever by dust.
    for z in [-2, -3, -4] {
        b.set([0, 3, z], FLOOR);
        b.set([0, 4, z], "repeater[facing=north,delay=4]");
    }
    b.dust([0, 6, -2]);
    b.dust([0, 6, -3]);
    b.dust([0, 6, -4]);
    b.dust([0, 5, -5]);
    b.d
}

/// rb3: rb2 with BM quasi-powered by RB_BM at (0,-1,0) (next to BM's head; the head forwards
/// neighbor updates to BM), so the left backbone stops at y=-1.
pub fn rb3() -> Design {
    let mut d = rb2();
    d.name = "piston-door-3x3-rb3".into();
    d.blocks.remove(&[-1, -2, 0]);
    d.blocks.remove(&[0, -2, 0]);
    d.blocks.insert([0, -1, 0], "minecraft:redstone_block".into());
    d
}

/// rb4 test bench: rb2 without the late chain; Q's hold is a silent quasi source H (a repeater
/// into (-1,5,-1), next to Q's quasi cell (0,5,-1)). Driven by `rb4-drive.txt`.
pub fn rb4_bench() -> Design {
    let mut d = rb2();
    d.name = "piston-door-3x3-rb4-bench".into();
    for z in [-2, -3, -4] {
        d.blocks.remove(&[0, 3, z]);
        d.blocks.remove(&[0, 4, z]);
        d.blocks.remove(&[0, 6, z]);
        d.blocks.remove(&[0, 5, z]);
    }
    d.blocks.remove(&[0, 5, -5]);
    d.blocks.remove(&[0, 4, -5]);
    let mut b = Builder { d };
    b.set([-1, 5, -1], FLOOR); // H block
    b.repeater([-1, 5, -2], "south", 1); // H repeater: input from (-1,5,-3)
    b.d
}

/// rb5: rb2 with a silent hold instead of the 24gt late chain. Q keeps extended after the lever
/// goes off because H (-1,5,-1), next to Q's quasi cell, is powered by L delayed 14gt; H turns off
/// without updating Q, so Q stays extended (BUD) until A, lifted back by B at the end of the
/// closing dance, lands next to Q's head (the head forwards the update to Q).
pub fn rb5() -> Design {
    let mut d = rb4_bench();
    d.name = "piston-door-3x3-rb5".into();
    d.blocks.remove(&[-1, 4, -2]);
    d.blocks.remove(&[-1, 5, -2]);
    let mut b = Builder { d };
    b.repeater([0, 6, -2], "north", 4); // rep1: input from the lever
    b.dust([0, 6, -3]);
    b.dust([-1, 5, -3]);
    b.repeater([-1, 5, -2], "south", 4); // rep2 into H
    b.d
}

/// rb6: rb5 with a 7-block silent hold and mechanics' 5-block O2 -> R_B link.
/// Hold: lever -> repeater (-1,6,-1) (its floor is H) -> block (-2,6,-1) -> dust (-3,6,-1) ->
/// its floor powers repeater (-2,5,-1) -> H (-1,5,-1), next to Q's quasi cell. The repeater's
/// floor (-2,4,-1) touches the open left backbone: glazed terracotta, which slime never drags.
/// O2 link (from mechanics' 51): O2 strongly powers (0,6,1), which lights the cross dust (0,6,2),
/// which strongly powers (1,6,2) behind R_B.
pub fn rb6() -> Design {
    let mut b = Builder::default();
    b.d.name = "piston-door-3x3-rb6".into();
    rb1_mech(&mut b);
    b.d.blocks.remove(&[1, 6, 2]);
    b.set([0, 5, -1], FLOOR); // X_L
    b.set([0, 6, -1], "lever[face=floor,facing=west,powered=false]");
    b.port("lever", [0, 6, -1]);
    b.set([0, 6, 0], "observer[facing=north]"); // O2 watches the lever
    b.set([0, 6, 1], FLOOR);
    b.set([0, 5, 2], FLOOR);
    b.set([0, 6, 2], "redstone_wire[north=side,east=side,south=side,west=side]");
    b.set([1, 6, 2], FLOOR);
    // silent hold
    b.repeater([-1, 6, -1], "west", 4); // floor (-1,5,-1) = H
    b.set([-2, 6, -1], FLOOR);
    b.dust([-3, 6, -1]);
    b.set([-2, 5, -1], "repeater[facing=west,delay=4]");
    b.set([-2, 4, -1], "white_glazed_terracotta[facing=north]");
    b.d
}
