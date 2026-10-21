//! `mctas clock obs hopclk --n N --out F`: the compact hopper clock (clk-obs, 2026-10-21): two
//! hoppers pass N stones back and forth; each hopper's comparator powers, through the block in
//! front of it and a dust on that block, a sticky piston; the two pistons move ONE redstone block
//! over the hopper that receives, so the other one sends, and pull it back when the sender is
//! empty. 13 blocks for every N, period 16 N - 12 gt (N = 2: 20 gt ... N = 320, five full
//! stacks: 5108 gt). The output is an exterior air cell where the judge's lamp lights evenly.

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct HopclkArgs {
    /// The stones (2 ..= 320).
    #[arg(long)]
    pub n: u32,
    #[arg(long)]
    pub out: PathBuf,
    /// Write the `extend` headers (the family's base design).
    #[arg(long)]
    pub base: bool,
}

/// The design text for N stones (`base`: with the family's headers).
pub fn design(n: u32, base: bool) -> Result<String> {
    if !(2..=320).contains(&n) {
        bail!("hopclk: N = {n}: 2 ..= 320 stones (five stacks of 64 in the first hopper)");
    }
    let mut items = Vec::new();
    let mut left = n;
    let mut slot = 0;
    while left > 0 {
        let c = left.min(64);
        items.push(format!("{{Slot:{slot}b,id:\"minecraft:stone\",count:{c}}}"));
        left -= c;
        slot += 1;
    }
    let mut s = String::new();
    s.push_str("# Compact hopper clock (clk-obs): two hoppers facing each other pass N stones, one per 8 gt; each\n");
    s.push_str("# hopper's comparator lights the dust on the block in front of it, which powers a sticky piston;\n");
    s.push_str("# the two pistons hold one redstone block over the hopper that receives (a locked hopper still\n");
    s.push_str("# receives) and pull it over the other one when the sender runs empty. Period 16 N - 12 gt.\n");
    s.push_str(&format!("name: hopclk-n{n}\n"));
    if base {
        s.push_str("extend family: clk-obs-hopclk\n");
        s.push_str("extend period: 16n - 12\n");
        s.push_str("extend blocks: 13\n");
        s.push_str("extend rule: one more stone in the first hopper (same blocks; up to 5 stacks of 64)\n");
        s.push_str("extend n: 2..320\n");
        s.push_str(&format!("extend base: {n}\n"));
        s.push_str("extend generator: mctas clock obs hopclk --n {n} --out {out}\n");
    }
    s.push_str("port output: -1 0 0\n");
    s.push_str(&format!("nbt: 2 0 0 {{Items:[{}]}}\n", items.join(",")));
    s.push_str("origin: 0 0 0\n");
    s.push_str("palette:\n");
    s.push_str("  S minecraft:stone\n");
    s.push_str("  C minecraft:comparator[facing=east,mode=compare,powered=false]\n");
    s.push_str("  D minecraft:comparator[facing=west,mode=compare,powered=false]\n");
    s.push_str("  H minecraft:hopper[enabled=true,facing=east]\n");
    s.push_str("  I minecraft:hopper[enabled=true,facing=west]\n");
    s.push_str("  P minecraft:sticky_piston[extended=false,facing=east]\n");
    s.push_str("  Q minecraft:sticky_piston[extended=false,facing=west]\n");
    s.push_str("  R minecraft:redstone_block\n");
    s.push_str("  r minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]\n");
    s.push_str("layer -1:\n  .S..S.\n");
    s.push_str("layer 0:\n  SCHIDS\n");
    s.push_str("layer 1:\n  rPR.Qr\n");
    Ok(s)
}

pub fn run(a: HopclkArgs) -> Result<()> {
    std::fs::write(&a.out, design(a.n, a.base)?)?;
    Ok(())
}
