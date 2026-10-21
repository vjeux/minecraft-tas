//! `circuit fly elevator`: the elevator generator (track fly, coordinator 2026-10-21; the judge is
//! circuit::elevator, rule B). The vertical two-way engine 012898 with honey for the top part's
//! two x = 1 blocks (honey is no conductor) and the stone platform P beside the upper honey, in a
//! column of its own (x = 2): the only docked-only conductor a press can reach without touching a
//! machine piston on its way. Obsidian stops `height` apart; a station flag at each floor (an
//! observer on a sticky piston: moved, it fires and sends the docked machine; in place, it sees
//! the arrival through a relay and bounces it).
//!
//! Call logic: a copper bulb TB at the bottom floor holds the target (lit = top). Each button
//! stands on a copper bulb Q (toggled once per press); a comparator reads Q and observers watch
//! the comparator, so each press gives one 2 gt edge however long it is held. Present: the edge,
//! through a repeater, strongly powers the docked platform and a second repeater reads it into
//! TB (top: into the event bulb E_t). Otherwise the edge goes through a repeater into the cell of a
//! stone that a piston moves next to TB while lit (top: that stands next to E_t while not lit);
//! E_t's lit state reaches TB through a comparator and an observer chain. TB's level reaches the
//! pistons and the top floor by a comparator, a block and a torch tower.

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
    /// A diode or dust and the stone under it.
    fn on(&mut self, p: Pos, s: &str, support: &str) {
        self.st(p, s);
        self.st([p[0], p[1] - 1, p[2]], support);
    }
    fn dust(&mut self, p: Pos) {
        self.on(p, DUST, STONE);
    }
}

/// The elevator of `h` blocks between floors (h = 5 for now: the torch tower's lit block must
/// stand at h + 2).
pub fn design(h: i32) -> Design {
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
    let t0 = h + 3; // the platform's level at the top floor
    // the stops
    b.st([0, -1, 0], OBS);
    b.st([0, h + 4, 0], OBS);
    // bottom relay: O's dock pulse through the bottom slime, 8 gt, then a repeater the flag watches
    b.on([0, 1, -1], &rep("south", 4), STONE);
    b.st([0, 1, -2], STONE);
    for p in [[1, 1, -2], [2, 1, -2], [3, 1, -2], [3, 1, -1]] {
        b.dust(p);
    }
    b.on([3, 1, 0], &rep("north", 1), STONE);
    // bottom flag: in at (2,1,0) (its back powers A) while FP_b is powered (target top)
    b.st([2, 1, 1], &obs("east"));
    b.st([2, 1, 2], &sticky("north"));
    b.stations.push([2, 1, 0]);
    // top relay: O's dock pulse through the bottom slime at the top floor, a dust the flag watches
    b.on([-1, h + 1, 0], &rep("east", 1), OBS);
    b.st([-2, h + 1, 0], STONE);
    b.st([-2, h + 2, 0], DUST);
    // top flag: in at (-1,h+2,0) (its back powers s) while FP_t is retracted (target bottom)
    b.st([-1, h + 2, 0], &obs("west"));
    b.st([-1, h + 2, -1], &sticky("south"));
    b.stations.push([-1, h + 2, 1]);

    // bottom gate (south side): button on Q, comparator, edge observer, repeater into P's cell
    b.st([2, 4, 4], button());
    b.st([2, 3, 4], BULB);
    b.on([2, 3, 3], &cmp("south"), STONE);
    b.st([2, 3, 2], &obs("south"));
    b.on([2, 3, 1], &rep("south", 1), STONE);
    // present: R_p_b reads the docked platform into TB
    b.on([3, 3, 0], &rep("west", 1), STONE);
    b.st([4, 3, 0], BULB);
    // lit: a second edge observer, a repeater into X's cell (4,3,1); X is pushed there from
    // (5,3,1) by a piston beside K1 (powered = lit)
    b.st([3, 3, 3], &obs("west"));
    b.st([4, 3, 3], STONE);
    b.on([4, 3, 2], &rep("south", 1), STONE);
    b.st([5, 3, 1], STONE);
    b.st([6, 3, 1], &sticky("west"));
    b.stations.push([4, 3, 1]);
    // TB's level: a comparator into K1; a dust under K1 runs round to (3,2,2), whose support
    // powers the bottom flag piston
    b.on([5, 3, 0], &cmp("west"), STONE);
    b.st([6, 3, 0], STONE);
    for p in [[6, 2, 0], [6, 2, 1], [6, 2, 2], [6, 2, 3], [6, 2, 4], [5, 2, 4], [4, 2, 4], [3, 2, 4], [3, 2, 3], [3, 2, 2]] {
        b.dust(p);
    }
    // the torch tower, on K2 (a second comparator reading TB from the north: a torch beside K1
    // would power FPx by quasi-connectivity): lit blocks at 3 + 4k, lit torches at 6 + 4k, up to
    // the lit block at h + 2 (h = 5: 7), which powers FPx_t beside it
    b.on([4, 3, -1], &cmp("south"), STONE);
    b.st([4, 3, -2], STONE);
    let mut y = 4;
    while y < h + 2 {
        let on_at_rest = (y - 4) % 4 == 0;
        b.st([4, y, -2], &format!("minecraft:redstone_torch[lit={on_at_rest}]"));
        b.st([4, y + 1, -2], STONE);
        y += 2;
    }
    assert_eq!(y, h + 3, "elevator: the tower's lit block must stand at h + 2");
    // the top flag piston's lit line: beside the lit torch at h + 1, west along z = -3, one step
    // up into FP_t's back
    let yl = h + 1;
    for x in (-1..=4).rev() {
        b.dust([x, yl, -3]);
    }
    b.dust([-1, yl + 1, -2]);

    // top gate (north side): button on Q_t, comparator, edge observer, repeater into P_t's cell
    b.st([2, t0 + 1, -4], button());
    b.st([2, t0, -4], BULB);
    b.on([2, t0, -3], &cmp("north"), STONE);
    b.st([2, t0, -2], &obs("north"));
    b.on([2, t0, -1], &rep("north", 1), STONE);
    // present: R_p_t reads the docked platform into the event bulb E_t
    b.on([3, t0, 0], &rep("west", 1), STONE);
    b.st([4, t0, 0], BULB);
    // not lit: a second edge observer, a repeater (4 gt) into X_t's cell (4,t0,-1), where X_t
    // stands while its piston below (powered = lit) is retracted
    b.st([3, t0, -3], &obs("west"));
    b.dust([4, t0, -3]);
    b.st([4, t0, -2], &rep("north", 2)); // on the tower's lit block
    assert_eq!(t0 - 1, h + 2);
    b.st([4, t0, -1], STONE);
    b.st([4, t0 - 1, -1], &sticky("up"));
    b.stations.push([4, t0 + 1, -1]);
    // E_t's lit state: a comparator, an observer chain down x = 4, z = 1, into TB from above
    b.st([4, t0, 1], &cmp("north"));
    for y in 5..t0 {
        b.st([4, y, 1], &obs("up"));
    }
    b.st([4, 5, 0], &obs("south"));
    b.st([4, 4, 0], &obs("up"));

    b.d.meta.insert("circuit".into(), format!("elevator {h}"));
    b.d.meta.insert("distance".into(), h.to_string());
    b.d.meta.insert("travel".into(), "0 1 0".into());
    b.d.ports.insert("rider".into(), [2, 4, 0]);
    b.d.ports.insert("input.bottom".into(), [2, 4, 4]);
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
