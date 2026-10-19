//! Crafter item loops: a crafter crafts back and forth between 1 storage block and its 9 items
//! (slime block <-> 9 slime balls), a chain of hoppers carrying every result back into it
//! (extendable family `clk-misc-crafter-loop`).
//!
//! Layout (x east, z south, y = 0; the crafter C at the origin):
//! * k = 0: C faces north into the one hopper at (0, 0, -1), which faces south back into C.
//! * k >= 1: C faces east into a loop of 2k + 1 hoppers around a 2 x (k + 1) rectangle: (1..k, 0, 0)
//!   facing east (the last one north), then (k..1, 0, -1) facing west and (0, 0, -1) facing south into
//!   C.
//! * The hopper that feeds C, at (0, 0, -1), is read by a comparator at (-2, 0, -1) facing east
//!   THROUGH a stone at (-1, 0, -1) (the comparator stands on a stone); an observer at (-2, 0, 0)
//!   watches the comparator and an observer at (-1, 0, 0) watches that one, its back powering C.
//!
//! The comparator turns on when items reach the last hopper and off when it empties: then C holds
//! a whole recipe (the block, or the 9 balls one per slot: a hopper feeding a crafter fills every
//! enabled slot once before stacking), so the observer pulse 4 gt later crafts it and the result
//! goes back into the hoppers. The pulse on the rising edge finds C empty or partly filled: no
//! recipe (a grid of 1..8 slime balls matches nothing), nothing happens. Measured in the game (n = 0,
//! 1, 2, 3, 6, 10, 20): every 2 more hoppers add 28 gt (the block and the 9 balls each cross 2 more
//! hoppers; a hopper passes an item on 7 or 8 gt after receiving it).

use anyhow::Result;
use clap::Args as ClapArgs;
use clock::{Design, Pos};
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct LoopArgs {
    /// The family's n: 2n + 1 hoppers.
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// Write the design here (else print it).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

pub const PERIOD: (u32, u32) = (91, 28);

fn hopper(facing: &str) -> String {
    format!("minecraft:hopper[enabled=true,facing={facing}]")
}

pub fn build(n: u32) -> String {
    let k = n as i32;
    let mut d = Design { name: format!("clk-misc-crafter-loop-n{n}"), ..Default::default() };
    let mut put = |p: Pos, s: String| {
        d.blocks.insert(p, s);
    };
    let crafter_facing = if k == 0 { "north" } else { "east" };
    put([0, 0, 0], format!("minecraft:crafter[crafting=false,orientation={crafter_facing}_up,triggered=false]"));
    for x in 1..=k {
        put([x, 0, 0], hopper(if x == k { "north" } else { "east" }));
        put([x, 0, -1], hopper("west"));
    }
    put([0, 0, -1], hopper("south"));
    put([-1, 0, -1], "minecraft:stone".into());
    put([-2, 0, -1], "minecraft:comparator[facing=east,mode=compare,powered=false]".into());
    put([-2, -1, -1], "minecraft:stone".into());
    put([-2, 0, 0], "minecraft:observer[facing=north,powered=false]".into());
    put([-1, 0, 0], "minecraft:observer[facing=west,powered=false]".into());
    d.block_entities.insert([0, 0, 0], r#"{Items:[{Slot:0b,id:"minecraft:slime_block",count:1}]}"#.into());
    d.ports.insert("output".into(), [-2, 0, -1]);
    d.ports.insert("start".into(), [0, 1, 0]);
    d.meta.insert("port start".into(), "minecraft:redstone_block 2".into());
    let (p0, dp) = PERIOD;
    let blocks = 7 + 2 * n;
    d.meta.insert("extend family".into(), "clk-misc-crafter-loop".into());
    d.meta.insert("extend period".into(), format!("{p0} + {dp}n"));
    d.meta.insert("extend blocks".into(), "7 + 2n".into());
    d.meta.insert("extend rule".into(), "add 2 hoppers: the hopper loop one column longer".into());
    d.meta.insert("extend n".into(), "0..20".into());
    d.meta.insert("extend base".into(), n.to_string());
    d.meta.insert("extend generator".into(), "mctas clock misc crafter-loop --n {n} --out {out}".into());
    let caption = format!(
        "# Extendable family clk-misc-crafter-loop (n = {n}): P = {p0} + {dp}n = {} gt, 7 + 2n = {blocks} blocks. A crafter crafts back and forth\n\
         # between 1 slime block and 9 slime balls; {} hopper(s) carry each result back into it (one item per 8 gt). A comparator reads the hopper\n\
         # that feeds the crafter through a stone and two observers pulse the crafter on both of its edges: when that hopper empties the\n\
         # crafter holds a whole recipe and crafts 4 gt later; when it fills, the crafter is empty or partly filled and nothing matches.\n\
         # Extension: 2 more hoppers (+28 gt: the block and the 9 balls each cross 2 more hoppers).\n",
        p0 + dp * n,
        2 * n + 1
    );
    let text = d.to_text();
    match text.split_once('\n') {
        Some((first, rest)) => format!("{first}\n{caption}{rest}"),
        None => format!("{caption}{text}"),
    }
}

pub fn run(a: LoopArgs) -> Result<()> {
    let text = build(a.n);
    match &a.out {
        Some(p) => std::fs::write(p, &text)?,
        None => print!("{text}"),
    }
    Ok(())
}
