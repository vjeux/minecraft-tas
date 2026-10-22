//! `circuit fly elevator --floors 3`: the multi-floor elevator generator (track fly, coordinator
//! 2026-10-21; judged by circuit::elevator as `elevator-floors N S`). Destination buttons: every
//! floor has a button per floor; the car rests at the floor of the last press.
//!
//! Layout (north-centric; the machine column at x = 0..2, z = 0): the v8 engine with M_s (a stone
//! the slime drags) at (0,1,-1) and X (a stone the upper honey drags) at (1,3,-1). Push steps: up =
//! A pushes the top part, down = s pushes the bottom part; a stop must block a push. Middle floor
//! (offset m): up-stop = a piston head above X ((1,m+4,-2) facing south), guarded by obsidian at
//! (1,m+4,1) (a head extended while X passes shoves X into H2, and H2 into the obsidian: the push
//! fails); down-stop = a head below M_s ((0,m,-2)), guarded by obsidian at (0,m,1); up-flag in at
//! (2,m+1,0) (its back into A's dock cell), down-flag in at (-1,m+2,0) (into s's dock cell).

use mctas::design::Design;

type Pos = [i32; 3];

const OBS: &str = "minecraft:obsidian";
const STONE: &str = "minecraft:stone";
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const BULB: &str = "minecraft:waxed_copper_bulb[lit=false,powered=false]";

pub fn rep(facing: &str, delay: u8) -> String {
    format!("minecraft:repeater[delay={delay},facing={facing},locked=false,powered=false]")
}
pub fn cmp(facing: &str, mode: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode={mode},powered=false]")
}
pub fn obs(facing: &str) -> String {
    format!("minecraft:observer[facing={facing},powered=false]")
}
pub fn sticky(facing: &str) -> String {
    format!("minecraft:sticky_piston[extended=false,facing={facing}]")
}
pub fn piston(facing: &str) -> String {
    format!("minecraft:piston[extended=false,facing={facing}]")
}

pub struct B {
    pub d: Design,
    pub stations: Vec<Pos>,
}

impl B {
    pub fn new(name: String) -> B {
        let mut b = B { d: Design::default(), stations: Vec::new() };
        b.d.name = name;
        b
    }
    pub fn put(&mut self, p: Pos, s: &str) {
        if let Some(old) = self.d.blocks.insert(p, s.to_string()) {
            panic!("elevator: cell {p:?} taken twice ({old} and {s})");
        }
    }
    pub fn st(&mut self, p: Pos, s: &str) {
        self.put(p, s);
        self.stations.push(p);
    }
    /// A station cell that is empty at rest (a part moves there).
    pub fn free(&mut self, p: Pos) {
        self.stations.push(p);
    }
    pub fn on(&mut self, p: Pos, s: &str) {
        self.st(p, s);
        self.st([p[0], p[1] - 1, p[2]], STONE);
    }
    pub fn dust(&mut self, p: Pos) {
        self.on(p, DUST);
    }
    pub fn lever(&mut self, p: Pos, name: &str) {
        self.on(p, "minecraft:lever[face=floor,facing=north,powered=false]");
        self.d.ports.insert(format!("input.{name}"), p);
        self.d.meta.insert(format!("port input.{name}"), "lever".into());
    }
    pub fn finish(mut self) -> Design {
        self.stations.sort();
        self.stations.dedup();
        for (k, p) in self.stations.iter().enumerate() {
            self.d.ports.insert(format!("station.{k}"), *p);
        }
        self.d
    }
}

/// The machine at the bottom floor (north-centric: M_s and X on the north side).
pub fn machine(b: &mut B) {
    for (p, st) in [
        ([0, 0, 0], obs("down")),
        ([0, 1, 0], "minecraft:slime_block".into()),
        ([1, 1, 0], sticky("up")),
        ([0, 2, 0], sticky("down")),
        ([1, 2, 0], "minecraft:honey_block".into()),
        ([0, 3, 0], obs("up")),
        ([1, 3, 0], "minecraft:honey_block".into()),
        ([2, 3, 0], STONE.into()),
        ([0, 1, -1], STONE.into()),
        ([1, 3, -1], STONE.into()),
    ] {
        b.put(p, &st);
    }
}

/// A middle floor at offset m: the guarded stops and the two flags (their pistons' cells
/// returned: up-stop, down-stop, up-flag, down-flag).
pub fn middle(b: &mut B, m: i32) -> [Pos; 4] {
    b.st([1, m + 4, -2], &piston("south"));
    b.st([1, m + 4, 1], OBS);
    b.free([1, m + 4, -1]);
    b.st([0, m, -2], &piston("south"));
    b.st([0, m, 1], OBS);
    b.free([0, m, -1]);
    b.st([2, m + 1, -1], &obs("east"));
    b.st([2, m + 1, -2], &sticky("south"));
    b.free([2, m + 1, 0]);
    b.st([-1, m + 2, -1], &obs("west"));
    b.st([-1, m + 2, -2], &sticky("south"));
    b.free([-1, m + 2, 0]);
    [[1, m + 4, -2], [0, m, -2], [2, m + 1, -2], [-1, m + 2, -2]]
}

/// The bottom flag (in at (2,1,0), its back into A) and its piston at (2,1,-2) (returned).
pub fn bottom_flag(b: &mut B) -> Pos {
    b.st([2, 1, -1], &obs("east"));
    b.st([2, 1, -2], &sticky("south"));
    b.free([2, 1, 0]);
    [2, 1, -2]
}

/// The top flag at (0,h+3,-1) facing north (its back into o's dock cell: s below it fires by
/// quasi-connectivity), pushed out to (0,h+4,-1) by its piston at (0,h+2,-1) (returned) while
/// the target is the top; its arrival relay: an observer chain from M_s's top dock cell.
pub fn top_flag(b: &mut B, h: i32) -> Pos {
    for (p, st) in [
        ([-1, h + 1, -1], obs("east")),
        ([-1, h + 2, -1], obs("down")),
        ([-1, h + 3, -1], obs("down")),
        ([-1, h + 3, -2], obs("south")),
        ([0, h + 3, -2], obs("west")),
        ([0, h + 3, -1], obs("north")),
        ([0, h + 2, -1], sticky("up")),
    ] {
        b.st(p, &st);
    }
    b.free([0, h + 4, -1]);
    [0, h + 2, -1]
}

/// The 3-floor mechanism with levers (ideal inputs): `b` bottom flag, `t` holds the top flag
/// out, `us`/`ds` the middle stops, `uf`/`df` the middle flags.
pub fn levers3(s: i32) -> Design {
    let (m, h) = (s, 2 * s);
    let mut b = B::new(format!("fly-elevator3-levers-{s}"));
    machine(&mut b);
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    let fb = bottom_flag(&mut b);
    b.lever([fb[0] + 1, fb[1], fb[2]], "b");
    let ft = top_flag(&mut b, h);
    b.lever([ft[0], ft[1], ft[2] - 1], "t");
    let [us, ds, uf, df] = middle(&mut b, m);
    b.lever([us[0], us[1], us[2] - 1], "us");
    b.lever([ds[0], ds[1], ds[2] - 1], "ds");
    b.lever([uf[0], uf[1], uf[2] - 1], "uf");
    b.lever([df[0], df[1], df[2] - 1], "df");
    b.d.meta.insert("circuit".into(), "flying-machine".into());
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.finish()
}

/// Floor f's destination buttons (x = 1, 2, 3 for floors 0, 1, 2) at (x, f+4, -4), each on a
/// copper bulb with a comparator and an observer: one pulse per press however long it is held.
/// The bottom floor's pulses climb observer chains (x, y, -7), the top floor's come down them; all
/// three floors meet in the merge block M_x at (x, m+3, -7).
pub fn buttons(b: &mut B, s: i32) {
    let (m, h) = (s, 2 * s);
    for (k, f) in [0, m, h].into_iter().enumerate() {
        for x in 1..=3 {
            let d = x - 1;
            b.st([x, f + 4, -4], "minecraft:stone_button[face=floor,facing=north,powered=false]");
            b.d.ports.insert(format!("input.{k}-{d}"), [x, f + 4, -4]);
            b.d.meta.insert(format!("port input.{k}-{d}"), "button".into());
            b.st([x, f + 3, -4], BULB);
            b.on([x, f + 3, -5], &cmp("south", "compare"));
            b.st([x, f + 3, -6], &obs("south"));
        }
    }
    for x in 1..=3 {
        // bottom: a side observer, then the chain up to the merge block
        b.st([x, 3, -7], &obs("south"));
        for y in 4..=m + 2 {
            b.st([x, y, -7], &obs("down"));
        }
        b.st([x, m + 3, -7], STONE);
        // top: a side observer, then the chain down to the merge block
        b.st([x, h + 3, -7], &obs("south"));
        for y in m + 4..=h + 2 {
            b.st([x, y, -7], &obs("up"));
        }
    }
}

/// The target latches: G1 = (target >= 1) at y = m+2, G2 = (target >= 2) at y = m+4, z = -10,
/// each a stone and a redstone block between a reset piston (x = 1) and a set piston (x = 5):
/// reset = free (2), stone (3), block (4); set = stone (2), block (3), free (4). Writes: floor 0
/// resets both, floor 1 sets G1 and resets G2, floor 2 sets both. K0 (1,m+3,-10) between the
/// reset pistons takes floor 0's pulses, K2 (5,m+3,-10) between the set pistons floor 2's; floor
/// 1's hops up an observer chain, crosses over at m+7 and comes down to PS1's and PR2's north
/// faces.
pub fn latches(b: &mut B, s: i32) {
    let m = s;
    for (y, _) in [(m + 2, "G1"), (m + 4, "G2")] {
        b.st([1, y, -10], &piston("east"));
        b.free([2, y, -10]);
        b.st([3, y, -10], STONE);
        b.st([4, y, -10], "minecraft:redstone_block");
        b.st([5, y, -10], &piston("west"));
    }
    // floor 0 -> K0
    b.on([1, m + 3, -8], &rep("south", 1));
    b.on([1, m + 3, -9], &rep("south", 1));
    b.st([1, m + 3, -10], STONE);
    // floor 2 -> K2
    b.on([4, m + 3, -7], &rep("west", 1));
    b.dust([5, m + 3, -7]);
    b.dust([5, m + 3, -8]);
    b.on([5, m + 3, -9], &rep("south", 1));
    b.st([5, m + 3, -10], STONE);
    // floor 1: up, over, down
    b.on([2, m + 3, -8], &rep("south", 1));
    b.st([2, m + 3, -9], &obs("south"));
    for y in m + 4..=m + 6 {
        b.st([2, y, -9], &obs("down"));
    }
    b.st([2, m + 7, -9], STONE);
    b.on([2, m + 7, -10], &rep("south", 1));
    for p in [[2, m + 7, -11], [2, m + 7, -12], [3, m + 7, -12], [4, m + 7, -12]] {
        b.dust(p);
    }
    // (the crossing's end repeaters stand on the observers that watch them)
    b.st([1, m + 7, -12], &rep("east", 1));
    b.st([5, m + 7, -12], &rep("west", 1));
    for y in m + 4..=m + 6 {
        b.st([1, y, -12], &obs("up"));
    }
    b.st([1, m + 4, -11], &obs("north"));
    for y in m + 2..=m + 6 {
        b.st([5, y, -12], &obs("up"));
    }
    b.st([5, m + 2, -11], &obs("north"));
}

/// Step B: the mechanism with the buttons and the latches (the stops and flags still levers).
pub fn step_b(s: i32) -> Design {
    let (m, h) = (s, 2 * s);
    let mut b = B::new(format!("fly-elevator3-b-{s}"));
    machine(&mut b);
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    bottom_flag(&mut b);
    top_flag(&mut b, h);
    middle(&mut b, m);
    buttons(&mut b, s);
    latches(&mut b, s);
    b.d.meta.insert("circuit".into(), format!("elevator-floors 3 {s}"));
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.d.ports.insert("rider".into(), [2, 4, 0]);
    b.finish()
}
