//! Track clk-item's clock commands: `mctas clock item <args...>` calls [`run`] with every argument
//! after `item`. OWNED BY TRACK clk-item (hopper / dropper / item clocks): replace this stub freely
//! (add clap or anything to this crate's Cargo.toml); the clock judge's API is in `crates/clock`
//! (docs/clocks.md, "For explorer crates").

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas clock item: no commands yet (crates/clock-item is track clk-item's), got {args:?}")
}
