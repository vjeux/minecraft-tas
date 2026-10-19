//! Track clk-rt's clock commands: `mctas clock rt <args...>` calls [`run`] with every argument
//! after `rt`. OWNED BY TRACK clk-rt (torch / repeater / comparator clocks): replace this stub
//! freely (add clap or anything to this crate's Cargo.toml); the clock judge's API is in
//! `crates/clock` (docs/clocks.md, "For explorer crates").

pub fn run(args: Vec<String>) -> anyhow::Result<()> {
    anyhow::bail!("mctas clock rt: no commands yet (crates/clock-rt is track clk-rt's), got {args:?}")
}
