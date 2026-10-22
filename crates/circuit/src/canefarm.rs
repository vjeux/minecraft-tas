//! Full-size sugar cane farms (track clk-rt, designs/briefs/farm-sugar-cane.md): `circuit cane-gen`
//! writes THE OBVIOUS DESIGN, the bar of the brief: double rows of cane on mud, an observer and a
//! piston per cane, hoppers under every mud and water cell into one chest.
//!
//! Columns along x, repeated every 5 (`modules` double rows), canes along z (`length`):
//!
//! ```text
//!   x:  R  wB mB mA wA R  wB mB mA wA R ...
//! y=3:  d  o> .  .  <o d  ...          o = observer watching the cane's 3rd block
//! y=2:  s  p> c  c  <p s               p = piston facing the cane (breaks its 2nd block)
//! y=1:  .  .  c  c  .  .               c = sugar cane (planted by the judge)
//! y=0:  s  w  M  M  w  s               M = mud, w = water source, s = stone
//! y=-1: .  h  h  h  h  .               h = hopper (south), into a collector row at z = length
//! ```
//!
//! An observer's back powers the relay dust (R: dust on stone), the dust powers its stone, the stone
//! powers the pistons on both sides of it. A piston breaks the 2nd block (the 3rd pops a tick later)
//! and pushes the 2nd block's item into the neighbouring row; every item lands on mud (14/16 tall:
//! a hopper under it is not blocked, `HopperBlockEntity::suckInItems`) or in water (a hopper under
//! it too). One mud catcher (with its hopper) at each end of every cane row takes items that fly
//! off the row's ends.

use mctas::design::Design;
use std::collections::BTreeMap;

type Pos = [i32; 3];

/// LAYOUT 4 (designs/briefs/farm-sugar-cane.md): a `w` x `l` field, water on `x + 2z = 0 (mod 5)`
/// (every other cell is cane on mud: 4/5 of the floor), a hopper under every floor cell (rows east
/// into the collector), a ring of mud on hoppers around the field (items land up to ~1 block from
/// their cane), a piston facing down at each cane's 3rd-block height (the 3rd block never grows;
/// the head breaks the 2nd and pushes its item down), dust on every piston top (a carpet), fed by
/// repeaters on the west and east edges from distribution lines one cell further out, joined north
/// of the field and driven by `clock` (a design whose `port output` cell is the clock's on-mostly
/// output block: a wall torch on its north face makes the short pulse).
pub fn lattice(w: i32, l: i32, clock: &Design) -> Design {
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    let mut ports: BTreeMap<String, Pos> = BTreeMap::new();
    let water = |x: i32, z: i32| (x + 2 * z).rem_euclid(5) == 0;
    let mut n = 0;
    // floor, hoppers, ring
    for z in -1..=l {
        for x in -1..=w {
            let field = (0..w).contains(&x) && (0..l).contains(&z);
            // two collectors (the ring's west and east columns, each into its own chest): the
            // judge's speed-up (rts 30 for 200 canes: ~1.9 items/s) would load one hopper line
            // (2.5/s) to 75%, and its stray-item batches would back the rows up for minutes
            let facing = if x == -1 || x == w { "south" } else if x < w / 2 { "west" } else { "east" };
            b.insert([x, -1, z], hopper(facing));
            if water(x, z) {
                // the lattice continues into the ring: an edge cane's water may lie there; a stone
                // outside the ring holds that water in
                b.insert([x, 0, z], WATER.into());
                if !field {
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, nz) = (x + dx, z + dz);
                        if !(-1..=w).contains(&nx) || !(-1..=l).contains(&nz) {
                            b.insert([nx, 0, nz], STONE.into());
                        }
                    }
                }
            } else {
                b.insert([x, 0, z], MUD.into());
            }
            if field && !water(x, z) {
                ports.insert(format!("crop.{n:03}"), [x, 1, z]);
                n += 1;
                b.insert([x, 3, z], piston("down"));
                b.insert([x, 4, z], DUST.into());
            }
        }
    }
    ports.insert("output.a".into(), [-1, -1, l + 1]);
    ports.insert("output.b".into(), [w, -1, l + 1]);
    // a repeater's `facing` is its INPUT side: `rep(d)` outputs toward d
    let rep = |d: &str| {
        let f = match d {
            "east" => "west",
            "west" => "east",
            "north" => "south",
            _ => "north",
        };
        format!("minecraft:repeater[delay=1,facing={f},locked=false,powered=false]")
    };
    let mut wire = |b: &mut BTreeMap<Pos, String>, p: Pos, s: String| {
        b.insert([p[0], p[1] - 1, p[2]], STONE.into());
        b.insert(p, s);
    };
    // distribution lines (x = -2 and x = w + 1) with a repeater at their top and middle, entry
    // repeaters (x = -1 and x = w) into the carpet's edge every other cane-edged row
    let mid = l / 2 - 1;
    for (line, entry, into, edge) in [(-2, -1, "east", 0), (w + 1, w, "west", w - 1)] {
        for z in -1..l {
            let s = if z == -1 || z == mid { rep("south") } else { DUST.into() };
            wire(&mut b, [line, 4, z], s);
        }
        let mut last = -9;
        for z in 0..l {
            if !water(edge, z) && z != mid && z - last >= 2 {
                wire(&mut b, [entry, 4, z], rep(into));
                last = z;
            }
        }
    }
    // the north connector (z = -2) from x = -2 to w + 1, fed at x = jx from the clock's route
    let jx = w / 2;
    for x in -2..=w + 1 {
        let s = if x == jx - 1 { rep("west") } else if x == jx + 1 { rep("east") } else { DUST.into() };
        wire(&mut b, [x, 4, -2], s);
    }
    // the clock north of it: its output block at (jx + 2, 4, -6), the torch on that block's north
    // face, a repeater west of the torch, then dust south down x = jx to the connector
    let out = clock.ports.get("output").copied().unwrap_or([0, 0, 0]);
    let a = [out[0] + 1, out[1], out[2]];
    let off = [jx + 2 - a[0], 4 - a[1], -6 - a[2]];
    for (p, s) in &clock.blocks {
        b.insert([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s.clone());
    }
    let mut block_entities = BTreeMap::new();
    for (p, s) in &clock.block_entities {
        block_entities.insert([p[0] + off[0], p[1] + off[1], p[2] + off[2]], s.clone());
    }
    b.insert([jx + 2, 4, -7], "minecraft:redstone_wall_torch[facing=north,lit=true]".into());
    wire(&mut b, [jx + 1, 4, -7], rep("west"));
    for z in -7..=-3 {
        wire(&mut b, [jx, 4, z], DUST.into());
    }
    let mut meta = BTreeMap::new();
    meta.insert("circuit".to_string(), "harvester sugar_cane".to_string());
    Design { name: format!("cane-lattice-{w}x{l}"), meta, ports, blocks: b, entities: Vec::new(), block_entities }
}

const STONE: &str = "minecraft:stone";
const MUD: &str = "minecraft:mud";
const WATER: &str = "minecraft:water[level=0]";
const DUST: &str = "minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]";

fn hopper(facing: &str) -> String {
    format!("minecraft:hopper[enabled=true,facing={facing}]")
}

fn piston(facing: &str) -> String {
    format!("minecraft:piston[extended=false,facing={facing}]")
}

fn observer(facing: &str) -> String {
    format!("minecraft:observer[facing={facing},powered=false]")
}

/// The obvious design: `modules` double rows of `length` canes each.
pub fn obvious(modules: i32, length: i32) -> Design {
    let mut b: BTreeMap<Pos, String> = BTreeMap::new();
    let mut ports: BTreeMap<String, Pos> = BTreeMap::new();
    let mut put = |p: Pos, s: &str| {
        b.insert(p, s.to_string());
    };
    let relays: Vec<i32> = (0..=modules).map(|m| 5 * m).collect();
    for z in 0..length {
        for &r in &relays {
            put([r, 0, z], STONE);
            put([r, 2, z], STONE);
            put([r, 3, z], DUST);
        }
        for m in 0..modules {
            let base = 5 * m;
            // B side: piston and observer face east, A side: west
            for (w, cane, face) in [(base + 1, base + 2, "east"), (base + 4, base + 3, "west")] {
                put([w, 0, z], WATER);
                put([w, -1, z], &hopper("south"));
                put([w, 2, z], &piston(face));
                put([w, 3, z], &observer(face));
                put([cane, 0, z], MUD);
                put([cane, -1, z], &hopper("south"));
            }
        }
    }
    let mut n = 0;
    for m in 0..modules {
        let base = 5 * m;
        for cane in [base + 2, base + 3] {
            for z in 0..length {
                ports.insert(format!("crop.{n:03}"), [cane, 1, z]);
                n += 1;
            }
            // catchers at both ends of the row (no cane), the water channels walled
            put([cane, 0, -1], MUD);
            put([cane, -1, -1], &hopper("south"));
            put([cane, 0, length], MUD);
        }
        for w in [base + 1, base + 4] {
            // the water channels' ends: mud (holds the water) over a hopper, so an item that
            // lands there is taken too
            put([w, 0, -1], MUD);
            put([w, -1, -1], &hopper("south"));
            put([w, 0, length], MUD);
        }
        // a second catcher row at both ends (a 3rd block's item flies up to ~1.3 blocks: one
        // escaped past z = length + 0.75 in a 48000 gt run), into the first
        for x in base + 1..=base + 4 {
            put([x, 0, -2], MUD);
            put([x, -1, -2], &hopper("south"));
            put([x, 0, length + 1], MUD);
            put([x, -1, length + 1], &hopper("north"));
        }
    }
    // the collector row under z = length, west into the judge's chest
    let east = 5 * modules - 1;
    for x in 1..=east {
        put([x, -1, length], &hopper("west"));
    }
    ports.insert("output".into(), [0, -1, length]);
    let mut meta = BTreeMap::new();
    meta.insert("circuit".to_string(), "harvester sugar_cane".to_string());
    Design { name: format!("cane-obvious-{}x{}", 5 * modules + 1, length), meta, ports, blocks: b, entities: Vec::new(), block_entities: BTreeMap::new() }
}
