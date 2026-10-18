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
    vec!["v1"]
}

pub fn build(name: &str) -> Option<Design> {
    match name {
        "v1" => Some(v1()),
        _ => None,
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
