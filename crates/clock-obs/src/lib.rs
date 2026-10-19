//! Track clk-obs's clock commands: `mctas clock obs <args...>` calls [`run`] with every argument
//! after `obs`. OWNED BY TRACK clk-obs (observer / piston / bulb / note block clocks): replace this
//! stub freely (add clap or anything to this crate's Cargo.toml); the clock judge's API is in
//! `crates/clock` (docs/clocks.md, "For explorer crates").

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas clock obs: no commands yet (crates/clock-obs is track clk-obs's), got {args:?}")
}
