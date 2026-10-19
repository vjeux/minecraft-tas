//! Minecart clocks (design entities: `clock verify` summons the design's `entity:` lines).
//!
//! `cart-pingpong` (extendable family `clk-misc-cart-pingpong`): an empty minecart bounces between
//! two stone bumpers on a straight east-west track: a powered rail at each end (on a redstone block,
//! always powered) launches the resting cart away from the bumper behind it (0.02 b/t, then
//! +0.06 b/t per tick on powered rails, at most 0.4); a detector rail next to the west end turns on
//! when the cart passes it and off at its next 20 gt check once the cart has left. Between the
//! detector rail and the east end: a plain rail, n + 1 powered rails (on redstone blocks), a plain
//! rail; the empty cart coasts over plain rails (x0.96 per tick). Every bounce clamps the cart
//! against a bumper, so from the second lap its motion repeats exactly. Measured in the game
//! (n + 1 = 1, 2, 3, 4, 5, 8, 11, 21): P = 38 + 5n gt (each powered rail adds a cell crossed twice
//! at the 0.4 b/t cap: 5 gt). Without the middle powered rail (`--bare`) the period is 40.

use anyhow::Result;
use clap::Args as ClapArgs;
use clock::Design;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct PingpongArgs {
    /// The family's n: n + 1 powered rails in the middle of the track.
    #[arg(long, default_value_t = 0)]
    pub n: u32,
    /// No powered rail in the middle (P = 40; not a family member).
    #[arg(long)]
    pub bare: bool,
    /// Write the design here (else print it).
    #[arg(long)]
    pub out: Option<PathBuf>,
}

const POWERED: &str = "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]";
const PLAIN: &str = "minecraft:rail[shape=east_west,waterlogged=false]";

pub fn build(n: u32, bare: bool) -> String {
    let middle = if bare { 0 } else { n + 1 };
    let name = if bare { "clk-misc-cart-pingpong-bare".to_string() } else { format!("clk-misc-cart-pingpong-n{n}") };
    let mut d = Design { name, ..Default::default() };
    let mut x = 0;
    let mut cell = |rail: &str, below: &str| {
        d.blocks.insert([x, 0, 0], rail.to_string());
        if !below.is_empty() {
            d.blocks.insert([x, -1, 0], below.to_string());
        }
        x += 1;
    };
    cell("minecraft:stone", "");
    cell(POWERED, "minecraft:redstone_block");
    cell("minecraft:detector_rail[powered=false,shape=east_west,waterlogged=false]", "minecraft:stone");
    cell(PLAIN, "minecraft:stone");
    for _ in 0..middle {
        cell(POWERED, "minecraft:redstone_block");
    }
    cell(PLAIN, "minecraft:stone");
    cell(POWERED, "minecraft:redstone_block");
    cell("minecraft:stone", "");
    d.ports.insert("output".into(), [2, 0, 0]);
    d.entities.push(mctas::design::EntitySpec { kind: "minecraft:minecart".into(), pos: [1.5, 0.0625, 0.5], nbt: None });
    if !bare {
        d.meta.insert("extend family".into(), "clk-misc-cart-pingpong".into());
        d.meta.insert("extend period".into(), "38 + 5n".into());
        d.meta.insert("extend blocks".into(), "15 + 2n".into());
        d.meta.insert("extend rule".into(), "one more powered rail (on a redstone block) in the middle of the track".into());
        d.meta.insert("extend n".into(), "0..20".into());
        d.meta.insert("extend base".into(), n.to_string());
        d.meta.insert("extend generator".into(), "mctas clock misc cart-pingpong --n {n} --out {out}".into());
    }
    let head = if bare {
        "Minecart ping-pong, bare (P = 40 gt, 13 pieces: 12 blocks + the cart)".to_string()
    } else {
        format!("Extendable family clk-misc-cart-pingpong (n = {n}): P = 38 + 5n = {} gt, 15 + 2n = {} pieces (blocks + the cart)", 38 + 5 * n, 15 + 2 * n)
    };
    let caption = format!(
        "# {head}. An empty minecart bounces between two stone bumpers\n\
         # on a straight track: a powered rail at each end (on a redstone block) launches the resting cart away from the bumper behind it\n\
         # and speeds it up; it coasts over plain rails, is sped up by the {middle} powered rail(s) in the middle, stops against the far\n\
         # bumper and is launched back. A detector rail next to the west end turns on as the cart passes and off at its next 20 gt check\n\
         # once the cart has left. Every bounce clamps the cart against a bumper, so from the second lap its motion repeats exactly.\n"
    );
    let text = d.to_text();
    match text.split_once('\n') {
        Some((first, rest)) => format!("{first}\n{caption}{rest}"),
        None => format!("{caption}{text}"),
    }
}

pub fn run(a: PingpongArgs) -> Result<()> {
    let text = build(a.n, a.bare);
    match &a.out {
        Some(p) => std::fs::write(p, &text)?,
        None => print!("{text}"),
    }
    Ok(())
}
