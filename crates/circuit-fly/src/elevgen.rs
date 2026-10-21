//! `circuit fly elevator`: the elevator generator (track fly, coordinator 2026-10-21; the judge is
//! circuit::elevator, rule B). The vertical two-way engine 012898 with honey for the top part's
//! two x = 1 blocks (honey is no conductor) and the stone platform P beside the upper honey, in a
//! column of its own (x = 2): the only docked-only conductor a press can reach without touching a
//! machine piston on its way. Obsidian stops `height` apart; a station flag at each floor (an
//! observer on a sticky piston: moved, it fires and sends the docked machine; in place, it sees
//! the arrival through a relay and bounces it).
//!
//! The target T (top or bottom) is a set/reset latch: a redstone block between two normal
//! pistons. Each press WRITES T from the platform's presence alone (rule B: a press at the floor
//! where it stands sends it, anywhere else calls it), so presses that agree never cancel (a toggle
//! that samples T does, when two presses race). Each button stands on a copper bulb toggled once
//! per press; a comparator reads the bulb and observers watch the comparator, so a press gives
//! 2 gt edges however long it is held. Bottom press: reset (target bottom), then set if the
//! platform is docked there (the edge, through a repeater, strongly powers the docked platform and
//! a second repeater reads it). Top press: set, then reset if the platform is docked at the top;
//! both go down by observer chains. T reaches the bottom flag piston directly and the top one by
//! a torch tower.

use mctas::design::Design;

type Pos = [i32; 3];

const OBS: &str = "minecraft:obsidian";
const STONE: &str = "minecraft:stone";
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";
const BULB: &str = "minecraft:waxed_copper_bulb[lit=false,powered=false]";

fn rep(facing: &str, delay: u8) -> String {
    format!("minecraft:repeater[delay={delay},facing={facing},locked=false,powered=false]")
}

fn cmp(facing: &str) -> String {
    format!("minecraft:comparator[facing={facing},mode=compare,powered=false]")
}

fn obs(facing: &str) -> String {
    format!("minecraft:observer[facing={facing},powered=false]")
}

fn sticky(facing: &str) -> String {
    format!("minecraft:sticky_piston[extended=false,facing={facing}]")
}

fn button() -> &'static str {
    "minecraft:stone_button[face=floor,facing=north,powered=false]"
}

struct B {
    d: Design,
    stations: Vec<Pos>,
}

impl B {
    fn put(&mut self, p: Pos, s: &str) {
        if let Some(old) = self.d.blocks.insert(p, s.to_string()) {
            panic!("elevator: cell {p:?} taken twice ({old} and {s})");
        }
    }
    /// A station block (counted, left out of the machine's shape).
    fn st(&mut self, p: Pos, s: &str) {
        self.put(p, s);
        self.stations.push(p);
    }
    /// A diode or dust and the block under it.
    fn on(&mut self, p: Pos, s: &str, support: &str) {
        self.st(p, s);
        self.st([p[0], p[1] - 1, p[2]], support);
    }
    fn dust(&mut self, p: Pos) {
        self.on(p, DUST, STONE);
    }
}

/// The elevator of `h` blocks between floors (only h = 5 is laid out so far).
pub fn design(h: i32) -> Design {
    assert!(h >= 3, "elevator: h >= 3");
    let mut b = B { d: Design::default(), stations: Vec::new() };
    b.d.name = format!("fly-elevator-{h}");
    // the machine (bottom floor as built) and its platform
    b.put([0, 0, 0], &obs("down"));
    b.put([0, 1, 0], "minecraft:slime_block");
    b.put([1, 1, 0], &sticky("up"));
    b.put([0, 2, 0], &sticky("down"));
    b.put([1, 2, 0], "minecraft:honey_block");
    b.put([0, 3, 0], &obs("up"));
    b.put([1, 3, 0], "minecraft:honey_block");
    b.put([2, 3, 0], STONE);
    // M_s: a stone the bottom slime drags along (its own column x = 0, z = 1): docked at (0,1,1)
    // only at the bottom floor, where an observer sees it come and go (presence at y = 1)
    b.put([0, 1, 1], STONE);
    let t0 = h + 3; // the platform's level at the top floor
    // the stops
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    // (no arrival relay at the bottom: the flag moves in only once the platform is docked, so its
    // moved pulse is the only kick, at a fixed time after the docking)
    // bottom flag: in at (2,1,0) (its back powers A) while FP_b is powered (target top AND docked)
    b.st([2, 1, 1], &obs("east"));
    b.st([2, 1, 2], &sticky("north"));
    b.stations.push([2, 1, 0]);
    // top relay: O's dock pulse through the bottom slime at the top floor, a dust the flag watches
    b.on([-1, h + 1, 0], &rep("east", 1), OBS);
    b.st([-2, h + 1, 0], STONE);
    b.st([-2, h + 2, 0], DUST);
    // top flag: in at (-1,h+3,0) while FP_t, south of it, is retracted (target bottom); extended
    // (target top) it pushes the flag out to (-1,h+3,-1). Its back powers o's dock cell, which
    // kicks s below it by quasi-connectivity: a flag beside s itself, moved in mid-trip, would
    // kick s one cell early (s's cell below that is on the path). It watches an observer on the
    // relay dust.
    b.st([-2, h + 3, 0], &obs("down"));
    b.st([-1, h + 3, 0], &obs("west"));
    b.st([-1, h + 3, 1], &sticky("north"));
    b.stations.push([-1, h + 3, -1]);

    // the target T: a 2-block slider between two normal pistons (y = 1, z = 3), a stone M2 west of
    // a redstone block M1. Target top: M2 (2,1,3), beside FP_b's back, M1 (3,1,3); target bottom
    // (as built): M2 (3,1,3), M1 (4,1,3). PS (5,1,3) pushes it west (set: target top), PR (1,1,3)
    // east (reset). R_pb powers M2's top cell while the platform is docked at the bottom, so FP_b
    // is powered iff target top AND docked; the tower reads (3,1,3): M1 there iff target top.
    b.st([1, 1, 3], "minecraft:piston[extended=false,facing=east]");
    b.stations.push([2, 1, 3]);
    b.st([3, 1, 3], STONE);
    b.st([4, 1, 3], "minecraft:redstone_block");
    b.st([5, 1, 3], "minecraft:piston[extended=false,facing=west]");
    b.on([2, 1, 4], &rep("south", 1), STONE);
    // presence at y = 1: an observer on M_s's dock cell toggles a bulb (lit as built), a comparator
    // reads it, a repeater and dust carry it round to R_pb
    b.st([0, 1, 2], &obs("north"));
    b.st([0, 1, 3], "minecraft:waxed_copper_bulb[lit=true,powered=false]");
    b.on([0, 1, 4], "minecraft:comparator[facing=north,mode=compare,powered=true]", STONE);
    b.on([0, 1, 5], &rep("north", 1), STONE);
    for p in [[0, 1, 6], [1, 1, 6], [2, 1, 6], [2, 1, 5]] {
        b.dust(p);
    }
    // the tower: RT0 reads (3,1,3), a second repeater, dust at z = 8 then x = -1 at y = 0 into K0
    // (-1,y0,2); torches and blocks up the column (-1,y,2) to the element at h + 3 beside FP_t,
    // one the target-top level powers (a block at y0 + 4k or a torch at y0 + 3 + 4k)
    let y0 = match h % 4 {
        0 => 0, // torch on top
        1 => 0, // block on top
        2 => 1, // block on top
        _ => panic!("elevator: h = 3 mod 4 is not laid out"),
    };
    b.on([3, 1, 4], &rep("north", 1), STONE);
    b.on([3, 1, 5], &rep("north", 1), STONE);
    b.on([3, 1, 6], &rep("north", 1), STONE); // (repeaters: no dust beside the presence line)
    for p in [[3, 1, 7], [3, 1, 8], [2, 0, 8], [1, 0, 8], [0, 0, 8], [-1, 0, 8], [-1, 0, 7], [-1, 0, 6], [-1, 0, 5], [-1, 0, 4]] {
        b.dust(p);
    }
    b.st([-1, 1, 6], STONE); // cuts the feed from the presence dust above it
    if y0 == 0 {
        b.dust([-1, 0, 3]);
    } else {
        b.dust([-1, 1, 3]);
    }
    b.st([-1, y0, 2], STONE);
    for y in y0 + 1..=h + 3 {
        let k = y - y0;
        let s = if k % 2 == 1 { format!("minecraft:redstone_torch[lit={}]", k % 4 == 1) } else { STONE.to_string() };
        b.st([-1, y, 2], &s);
    }

    // bottom gate (north side): button on Q, comparator, edge observer Wq, repeater R1 into P's
    // cell. Set if P is docked: R_p_b reads P, a dust down into PS (6 gt after the reset). Reset
    // always: a second edge observer W2, a dust round the west (by x = -3) and onto PR.
    b.st([2, 4, -4], button());
    b.st([2, 3, -4], BULB);
    b.on([2, 3, -3], &cmp("north"), STONE);
    b.st([2, 3, -2], &obs("north"));
    // (R1 reads the platform 6 gt after the press, after the reset path's lock was checked: a
    // platform leaving in between makes no write, which is right, it leaves for the top anyway)
    b.on([2, 3, -1], &rep("north", 3), STONE);
    b.on([3, 3, 0], &rep("west", 1), STONE);
    for p in [[4, 3, 0], [5, 3, 0], [5, 2, 1], [5, 1, 2]] {
        b.dust(p);
    }
    // presence at the bottom floor: an observer on P's dock cell toggles a bulb on each arrival
    // and departure (lit as built: docked); a comparator reading it locks the reset path's
    // repeater R_a while the platform is there, so a press writes exactly once
    b.st([2, 3, 1], &obs("north"));
    b.st([2, 3, 2], "minecraft:waxed_copper_bulb[lit=true,powered=false]");
    b.on([2, 3, 3], "minecraft:comparator[facing=north,mode=compare,powered=true]", STONE);
    // reset (unless docked): a second edge observer W2 east of Cq, a dust round the east (x = 6,
    // a repeater on the way) and back west along z = 5 into R_a (2,3,4), then onto PR's block
    b.st([3, 3, -3], &obs("west"));
    for p in [[4, 3, -3], [5, 3, -3], [6, 3, -3], [7, 3, -3], [7, 3, -2], [7, 3, -1]] {
        b.dust(p);
    }
    // (x = 7: one clear of the set line's descent at x = 5, or an up-step would join them)
    b.on([7, 3, 0], &rep("north", 2), STONE); // (4 gt: a comparator eats a 2 gt pulse)
    for p in [[7, 3, 1], [7, 3, 2], [7, 3, 3], [7, 3, 4], [7, 3, 5], [6, 3, 5], [5, 3, 5], [4, 3, 5], [3, 3, 5], [3, 3, 4]] {
        b.dust(p);
    }
    // (a comparator subtracting the presence level, not a repeater it locks: a lock that lands
    // during the pulse freezes the repeater on, and the reset piston with it)
    b.on([2, 3, 4], "minecraft:comparator[facing=east,mode=subtract,powered=false]", STONE);
    for p in [[1, 3, 4], [1, 3, 3]] {
        b.dust(p);
    }
    // (the last dust's support, above PR, powers it: dust on PR itself pops when it extends)

    // top gate (north side, the same parts at t0): reset if P_t is docked, by R_p_t (south of
    // P_t) and an observer chain down onto the bottom reset dust; set unless docked: W2_t, a dust
    // into R_a_t (6,t0,0), locked by the top presence comparator, an observer chain down onto the
    // bottom set dust
    b.st([2, t0 + 1, -4], button());
    b.st([2, t0, -4], BULB);
    b.on([2, t0, -3], &cmp("north"), STONE);
    b.st([2, t0, -2], &obs("north"));
    b.on([2, t0, -1], &rep("north", 3), STONE);
    b.on([2, t0, 1], &rep("north", 1), STONE);
    b.st([2, t0, 2], &obs("north"));
    for y in 5..t0 {
        b.st([2, y, 2], &obs("up"));
    }
    b.st([2, 5, 3], &obs("north"));
    b.st([1, 5, 3], &obs("east"));
    b.st([1, 4, 3], &obs("up"));
    // presence at the top floor
    b.st([3, t0, 0], &obs("west"));
    b.st([4, t0, 0], BULB);
    b.on([5, t0, 0], &cmp("west"), STONE);
    b.st([3, t0, -3], &obs("west"));
    b.on([4, t0, -3], &rep("west", 2), STONE); // (4 gt for the comparator)
    for p in [[5, t0, -3], [6, t0, -3], [6, t0, -2], [6, t0, -1]] {
        b.dust(p);
    }
    b.on([6, t0, 0], "minecraft:comparator[facing=north,mode=subtract,powered=false]", STONE);
    // (the chain watches a second repeater: an observer on R_a_t would fire on its lock changing)
    b.on([6, t0, 1], &rep("north", 1), STONE);
    b.st([6, t0, 2], &obs("north"));
    b.st([6, t0 - 1, 2], &obs("up"));
    b.st([5, t0 - 1, 2], &obs("east"));
    for y in 5..t0 - 1 {
        b.st([5, y, 2], &obs("up"));
    }
    b.st([4, 5, 2], &obs("east"));
    b.st([4, 5, 1], &obs("south"));
    b.st([4, 5, 0], &obs("south"));
    b.st([4, 4, 0], &obs("up"));

    b.d.meta.insert("circuit".into(), format!("elevator {h}"));
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.d.ports.insert("rider".into(), [2, 4, 0]);
    b.d.ports.insert("input.bottom".into(), [2, 4, -4]);
    b.d.meta.insert("port input.bottom".into(), "button".into());
    b.d.ports.insert("input.top".into(), [2, t0 + 1, -4]);
    b.d.meta.insert("port input.top".into(), "button".into());
    b.stations.sort();
    b.stations.dedup();
    for (k, p) in b.stations.iter().enumerate() {
        b.d.ports.insert(format!("station.{k}"), *p);
    }
    b.d
}
