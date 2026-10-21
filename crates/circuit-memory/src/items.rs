//! `mctas circuit memory items --n N`: the item counter, the one counter whose size does not
//! depend on N. Two droppers facing each other hold N items between them; each input pulse fires
//! exactly one of them (the other is held: a dropper kept continuously powered sees no rising
//! edge), so the pile walks from one to the other and each dropper runs empty once every N
//! pulses. A comparator on each dropper is 0 only when it is empty, the torch on its output block
//! is therefore lit exactly then, and both torches power the copper bulb that is the phase and the
//! output: it toggles once every N pulses.
//!
//! The layout is fixed; only the preloaded count and the `circuit:` line change with N. A dropper
//! holds 9 slots of 64, so one pair counts to 576.
//!
//! Two rules the layout obeys (measured 2026-10-20): a dropper must never be STRONGLY powered (a
//! torch under it, a comparator pointing into it), or it powers the dropper beside it and that one
//! never sees a rising edge again; so both holds are weak (a stone powered by the phase comparator
//! under A, a torch beside B). And the hold torch is fed from that stone by a dust UNDER B, which
//! a dust cannot power upward.

use anyhow::{Result, bail};
use std::path::Path;

/// The design text for counter N (23 blocks, any 2 <= N <= 576).
pub fn design(n: usize) -> Result<String> {
    if !(2..=576).contains(&n) {
        bail!("counter {n}: the item counter holds 2..=576 items (9 slots x 64)");
    }
    let slots: Vec<String> = (0..).map(|s| (s, n.saturating_sub(64 * s).min(64))).take_while(|(_, c)| *c > 0).map(|(s, c)| format!("{{Slot:{s}b,id:\"minecraft:stone\",count:{c}}}")).collect();
    Ok(format!(
        "# Counter {n}, 23 blocks, with items (track memory, `mctas circuit memory items --n {n}`): \
the output toggles once every {n} pulses, and the same 23 blocks count any N up to 576; only the number \
of items loaded changes. Two droppers face each other and hold {n} items between them; each pulse fires \
only one of them, so the pile walks from one to the other and each runs empty once every {n} pulses. \
A comparator reads each dropper, and the torch on its output block is lit exactly while that dropper is \
empty; both torches touch the copper bulb, which therefore toggles each time a dropper runs empty. The \
bulb is the phase: a comparator reading it powers the stone under the left dropper, which holds it (a \
dropper kept powered sees no new pulse), and through a dust under the right dropper switches off the \
torch beside it, which held the right one. The pulse reaches the right dropper through the input block \
on its lid and the left one through a dust on its own lid.
name: counter{n}-items
circuit: counter {n}
group: item-pair
group title: Item counter (counter N, any N)
group note: two droppers facing each other hold N items; each pulse fires only the one the copper bulb phase does not hold, so each runs empty once every N pulses - 23 blocks for every N up to 576 (`mctas circuit memory items --n N`)
port input: 1 2 0
port output: 0 0 1
nbt: 0 1 0 {{Items:[{items}]}}
origin: -2 -1 -2
palette:
  A minecraft:dropper[facing=east,triggered=false]
  B minecraft:dropper[facing=west,triggered=true]
  a minecraft:comparator[facing=east,mode=compare,powered=true]
  b minecraft:comparator[facing=south,mode=compare,powered=false]
  C minecraft:comparator[facing=north,mode=compare,powered=false]
  W minecraft:waxed_copper_bulb[lit=false,powered=true]
  t minecraft:redstone_torch[lit=true]
  u minecraft:redstone_wall_torch[facing=west,lit=true]
  v minecraft:redstone_wall_torch[facing=north,lit=false]
  S minecraft:stone
  d minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]
layer 0:
  .....
  ..S..
  ...S.
layer 1:
  SSW..
  ..CS.
  .SSdS
layer 2:
  dduS.
  v..b.
  SaABt
layer 3:
  .....
  .....
  ..d..
",
        items = slots.join(",")
    ))
}

/// `items --n N[,N...] --out DIR`: write one design per N.
pub fn run(ns: &[usize], out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    for &n in ns {
        let path = out.join(format!("counter{n}-items.txt"));
        std::fs::write(&path, design(n)?)?;
        println!("counter {n}: 23 blocks, {}", path.display());
    }
    Ok(())
}
