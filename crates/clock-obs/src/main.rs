//! Development entry point of track clk-obs's generators (`mctas clock obs` once registered).
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "clk-obs", about = "Clock study, track clk-obs: block-update clock generators and searches")]
struct Cli {
    #[command(flatten)]
    args: clock_obs::Args,
}

fn main() -> anyhow::Result<()> {
    clock_obs::run(Cli::parse().args)
}
