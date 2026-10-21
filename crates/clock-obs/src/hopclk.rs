//! `mctas clock obs hopclk`: hopper clocks (clk-obs, 2026-10-21). Two hoppers pass N stones back
//! and forth; each hopper's comparator powers a sticky piston, and the two pistons move ONE
//! redstone block beside (sealed form) or over (open form) the hopper that receives, so the other
//! one sends, and pull it back when the sender is empty.
//!
//! * Open form (`--n N`): 13 blocks, period 16 N - 12 gt; the redstone block moves over the
//!   hoppers (their tops are open to stray items).
//! * Sealed form (`--sealed`): the redstone block moves BESIDE the hoppers (it locks them from the
//!   side) and a stone covers each hopper's top, so a dropped item can never reach them (the
//!   intrusion rule); 17 blocks, the same period. `--west K` / `--east K` (K = 1..4) put a
//!   repeater of delay K in that side's comparator-to-piston path: the piston lets go of the
//!   redstone block 2K gt later, so the period grows by 2K gt (+4 blocks each). `--bulbs B` adds B
//!   copper-bulb dividers (a bulb toggled by the clock, a comparator reading it): x 2 each, +3
//!   blocks each. Period = 2^B (16 N - 12 + 2 (west + east)).
//! * `--period P`: the smallest sealed design whose period is exactly P (N <= 320).

use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(ClapArgs, Debug)]
pub struct HopclkArgs {
    /// The stones (2 ..= 320).
    #[arg(long)]
    pub n: Option<u32>,
    #[arg(long)]
    pub out: PathBuf,
    /// Write the `extend` headers (the open family's base design).
    #[arg(long)]
    pub base: bool,
    /// The sealed form (the redstone block beside the hoppers, stones over them).
    #[arg(long)]
    pub sealed: bool,
    /// Sealed form: the west side's repeater delay (0: none).
    #[arg(long, default_value_t = 0)]
    pub west: u32,
    /// Sealed form: the east side's repeater delay (0: none).
    #[arg(long, default_value_t = 0)]
    pub east: u32,
    /// Sealed form: copper-bulb divider stages.
    #[arg(long, default_value_t = 0)]
    pub bulbs: u32,
    /// The smallest sealed design with exactly this period (gt).
    #[arg(long)]
    pub period: Option<u32>,
}

/// The design text for N stones (`base`: with the family's headers).
pub fn design(n: u32, base: bool) -> Result<String> {
    if !(2..=320).contains(&n) {
        bail!("hopclk: N = {n}: 2 ..= 320 stones (five stacks of 64 in the first hopper)");
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
    s.push_str(&format!("nbt: 2 0 0 {}\n", items(n)));
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

/// The first hopper's contents: N stones in stacks of 64.
fn items(n: u32) -> String {
    let mut v = Vec::new();
    let mut left = n;
    let mut slot = 0;
    while left > 0 {
        let c = left.min(64);
        v.push(format!("{{Slot:{slot}b,id:\"minecraft:stone\",count:{c}}}"));
        left -= c;
        slot += 1;
    }
    format!("{{Items:[{}]}}", v.join(","))
}

/// A sealed hopper clock: (period, blocks) for a configuration.
pub fn sealed_period(n: u32, west: u32, east: u32, bulbs: u32) -> u32 {
    (16 * n - 12 + 2 * (west + east)) << bulbs
}

pub fn sealed_blocks(west: u32, east: u32, bulbs: u32) -> u32 {
    17 + if west > 0 { 4 } else { 0 } + if east > 0 { 4 } else { 0 } + 3 * bulbs
}

/// The smallest sealed configuration (n, west, east, bulbs) with exactly this period: fewest
/// blocks, then fewest items.
pub fn sealed_for(period: u32) -> Option<(u32, u32, u32, u32)> {
    let mut best: Option<(u32, u32, (u32, u32, u32, u32))> = None;
    for bulbs in 0..=6 {
        for west in 0..=4 {
            for east in 0..=4 {
                if east > 0 && west == 0 {
                    continue;
                }
                let base = period >> bulbs;
                if base << bulbs != period {
                    continue;
                }
                let k = base + 12;
                let extra = 2 * (west + east);
                if k < extra || (k - extra) % 16 != 0 {
                    continue;
                }
                let n = (k - extra) / 16;
                if !(2..=320).contains(&n) {
                    continue;
                }
                let key = (sealed_blocks(west, east, bulbs), n, (n, west, east, bulbs));
                if best.is_none_or(|b| (key.0, key.1) < (b.0, b.1)) {
                    best = Some(key);
                }
            }
        }
    }
    best.map(|b| b.2)
}

/// The sealed design text.
pub fn sealed_design(n: u32, west: u32, east: u32, bulbs: u32) -> Result<String> {
    if !(2..=320).contains(&n) || west > 4 || east > 4 {
        bail!("hopclk --sealed: N 2..=320, repeater delays 0..=4");
    }
    let mut b: BTreeMap<[i32; 3], &str> = BTreeMap::new();
    let stone = "minecraft:stone";
    let dust = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
    b.insert([2, 0, 0], "minecraft:hopper[enabled=true,facing=east]");
    b.insert([3, 0, 0], "minecraft:hopper[enabled=true,facing=west]");
    b.insert([2, 1, 0], stone);
    b.insert([3, 1, 0], stone);
    b.insert([1, 0, 0], "minecraft:comparator[facing=east,mode=compare,powered=false]");
    b.insert([1, -1, 0], stone);
    b.insert([4, 0, 0], "minecraft:comparator[facing=west,mode=compare,powered=false]");
    b.insert([4, -1, 0], stone);
    b.insert([1, 0, 1], "minecraft:sticky_piston[extended=false,facing=east]");
    b.insert([4, 0, 1], "minecraft:sticky_piston[extended=false,facing=west]");
    b.insert([2, 0, 1], "minecraft:redstone_block");
    let rep = |d: u32, f: &str| -> String { format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]") };
    let wrep = rep(west.max(1), "east");
    let erep = rep(east.max(1), "west");
    // west side: the comparator's output block (or a delay repeater, its output block and a dust
    // line back to the piston's back)
    let tap = if west == 0 {
        b.insert([0, 0, 0], stone);
        b.insert([0, 0, 1], dust);
        b.insert([0, -1, 1], stone);
        0
    } else {
        b.insert([0, 0, 0], &wrep);
        b.insert([0, -1, 0], stone);
        b.insert([-1, 0, 0], stone);
        b.insert([-1, 0, 1], dust);
        b.insert([-1, -1, 1], stone);
        b.insert([0, 0, 1], dust);
        b.insert([0, -1, 1], stone);
        -1
    };
    if east == 0 {
        b.insert([5, 0, 0], stone);
        b.insert([5, 0, 1], dust);
        b.insert([5, -1, 1], stone);
    } else {
        b.insert([5, 0, 0], &erep);
        b.insert([5, -1, 0], stone);
        b.insert([6, 0, 0], stone);
        b.insert([6, 0, 1], dust);
        b.insert([6, -1, 1], stone);
        b.insert([5, 0, 1], dust);
        b.insert([5, -1, 1], stone);
    }
    // bulb dividers west of the tap: a bulb the tap block powers, a comparator reading it whose
    // output feeds the next bulb (or the output cell)
    let mut x = tap - 1;
    for _ in 0..bulbs {
        b.insert([x, 0, 0], "minecraft:waxed_copper_bulb[lit=false,powered=false]");
        b.insert([x - 1, 0, 0], "minecraft:comparator[facing=east,mode=compare,powered=false]");
        b.insert([x - 1, -1, 0], stone);
        x -= 2;
    }
    let out = [x, 0, 0];
    let p = sealed_period(n, west, east, bulbs);
    let mut s = String::new();
    s.push_str(&format!("# Sealed hopper clock (clk-obs), period {p} gt: two hoppers facing each other pass {n} stones, one per\n"));
    s.push_str("# 8 gt; each hopper's comparator powers a sticky piston through the dust behind it, and the two\n");
    s.push_str("# pistons hold one redstone block BESIDE the hopper that receives (a locked hopper still receives);\n");
    s.push_str("# a stone covers each hopper (no stray item can reach them).");
    if west + east > 0 {
        s.push_str(&format!(" A repeater in a piston's path delays its letting go by 2 x its delay (west {west}, east {east})."));
    }
    if bulbs > 0 {
        s.push_str(&format!(" {bulbs} copper bulb divider(s) (x 2 each)."));
    }
    s.push_str("\n");
    s.push_str(&format!("name: hopclk-sealed-p{p}\n"));
    s.push_str(&format!("port output: {} {} {}\n", out[0], out[1], out[2]));
    s.push_str(&format!("nbt: 2 0 0 {}\n", items(n)));
    // the palette and layers
    let mut keys: Vec<String> = Vec::new();
    for st in b.values() {
        if !keys.iter().any(|k| k == st) {
            keys.push(st.to_string());
        }
    }
    let sym = |i: usize| -> char { b"ABCDEFGHIJKLMNOPQRSTUVWXYZ"[i] as char };
    let (xlo, xhi) = (b.keys().map(|p| p[0]).min().unwrap(), b.keys().map(|p| p[0]).max().unwrap());
    s.push_str(&format!("origin: {xlo} 0 0\npalette:\n"));
    for (i, k) in keys.iter().enumerate() {
        s.push_str(&format!("  {} {k}\n", sym(i)));
    }
    for y in -1..=1 {
        s.push_str(&format!("layer {y}:\n"));
        for z in 0..=1 {
            let row: String = (xlo..=xhi).map(|x| b.get(&[x, y, z]).map(|st| sym(keys.iter().position(|k| k == st).unwrap())).unwrap_or('.')).collect();
            s.push_str(&format!("  {row}\n"));
        }
    }
    Ok(s)
}

pub fn run(a: HopclkArgs) -> Result<()> {
    if let Some(p) = a.period {
        let Some((n, w, e, bl)) = sealed_for(p) else { bail!("hopclk: no sealed hopper clock has period {p} gt (N <= 320, repeaters 0..4 per side, bulbs 0..6)") };
        eprintln!("period {p}: N = {n}, west {w}, east {e}, bulbs {bl}: {} blocks", sealed_blocks(w, e, bl));
        std::fs::write(&a.out, sealed_design(n, w, e, bl)?)?;
        return Ok(());
    }
    let n = a.n.ok_or_else(|| anyhow::anyhow!("hopclk: --n or --period"))?;
    let text = if a.sealed { sealed_design(n, a.west, a.east, a.bulbs)? } else { design(n, a.base)? };
    std::fs::write(&a.out, text)?;
    Ok(())
}
