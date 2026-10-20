//! `mctas circuit memory items --n N`: the item counter, the one counter whose size does not
//! depend on N. Two droppers facing each other hold N items between them; each input pulse fires
//! exactly one of them (the other is held: a dropper kept continuously powered sees no rising
//! edge), so the pile walks from one to the other and each dropper runs empty once every N
//! pulses. A comparator on each dropper is 0 only when it is empty, the wall torch on its output
//! block is therefore lit exactly then, and both torches power the copper bulb that is the phase
//! and the output: it toggles once every N pulses.
//!
//! The layout is fixed; only the preloaded count and the `circuit:` line change with N. A dropper
//! holds 9 slots of 64, so one pair counts to 576.

use anyhow::{Result, bail};
use std::path::Path;

/// The design text for counter N (39 blocks, any 2 <= N <= 576).
pub fn design(n: usize) -> Result<String> {
    if !(2..=576).contains(&n) {
        bail!("counter {n}: the item counter holds 2..=576 items (9 slots x 64)");
    }
    let slots: Vec<String> = (0..).map(|s| (s, n.saturating_sub(64 * s).min(64))).take_while(|(_, c)| *c > 0).map(|(s, c)| format!("{{Slot:{s}b,id:\"minecraft:stone\",count:{c}}}")).collect();
    Ok(format!(
        "# Item counter, {n} items, 39 blocks (track memory, `mctas circuit memory items --n {n}`): \
the one counter whose size does not depend on N. Droppers A and B face each other and hold {n} items \
between them; each pulse fires exactly one of them, so the pile walks from A to B and back and each \
dropper runs empty once every {n} pulses. Comparators a and b read them (level 0 only when empty), so \
the wall torches on their output blocks are lit exactly when their own dropper is empty, and both \
torches power the copper bulb W above them: W toggles once every {n} pulses and is the output. W is \
also the phase. Comparator C reads it and its dust line powers the stone south of A, which holds A; \
the wall torch on that stone is lit only when the line is off, and its line holds B through the stone \
east of it. A held dropper is one kept continuously powered: it sees no rising edge, so a pulse fires \
only the other one. The pulse is the dust on the two lids, since a dropper fires on a signal at its \
own cell or at the cell above it.
name: counter{n}-items
circuit: counter {n}
group: item-pair
group title: Item counter (counter N, any N)
group note: two droppers facing each other hold N items; each pulse fires only the one the copper bulb phase does not hold, so each runs empty once every N pulses - 39 blocks for every N up to 576 (`mctas circuit memory items --n N`)
port input: 0 3 0
port output: -2 1 1
nbt: 1 1 0 {{Items:[{items}]}}
origin: -2 0 -3
palette:
  A minecraft:dropper[facing=east,triggered=false]
  B minecraft:dropper[facing=west,triggered=false]
  a minecraft:comparator[facing=south,mode=compare,powered=false]
  b minecraft:comparator[facing=south,mode=compare,powered=false]
  C minecraft:comparator[facing=east,mode=compare,powered=false]
  W minecraft:waxed_copper_bulb[lit=false,powered=false]
  t minecraft:redstone_wall_torch[facing=north,lit=true]
  w minecraft:redstone_wall_torch[facing=south,lit=true]
  S minecraft:stone
  r minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]
layer 0:
  SS...
  S....
  S.SS.
  S....
  SS..S
  ...SS
layer 1:
  SStt.
  r.SS.
  r.ab.
  r.ABS
  rrS.r
  ..wrr
layer 2:
  rCWS.
  .....
  .....
  ..rr.
  .....
  .....
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
        println!("counter {n}: 39 blocks, {}", path.display());
    }
    Ok(())
}
