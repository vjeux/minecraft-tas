//! Track clk-sculk's clock commands: `mctas clock sculk <args...>` calls [`run`] with every
//! argument after `sculk`. OWNED BY TRACK clk-sculk (sculk / vibration clocks): replace this stub
//! freely (add clap or anything to this crate's Cargo.toml); the clock judge's API is in
//! `crates/clock` (docs/clocks.md, "For explorer crates").

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas clock sculk: no commands yet (crates/clock-sculk is track clk-sculk's), got {args:?}")
}
