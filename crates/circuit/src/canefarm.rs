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
