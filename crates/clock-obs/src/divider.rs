//! Long periods by division: a copper bulb toggles on every rising edge, so a bulb powered by a
//! clock's observer changes state every pulse (its lit state has twice the period), and a
//! comparator reading it powers the next bulb only on every other pulse. `divider` attaches such
//! a chain (bulb, then comparator + its floor + bulb per extra stage, in a straight line out of
//! an observer's back) to a base clock: P = P0 x 2^stages for a base whose observer fires once
//! per period.

use crate::sim::{self, Spec, Verdict};
use anyhow::{Result, bail};
use clap::Args as ClapArgs;
use redsim::{Block, BlockPos, Dir};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(ClapArgs, Debug, Clone)]
pub struct DividerArgs {
    /// Base clock design
    pub base: PathBuf,
    /// Bulbs in the chain (1 bulb = one doubling)
    #[arg(long, default_value_t = 3)]
    pub bulbs: usize,
    /// Write the design here
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Try every observer of the base (default: the first that works)
    #[arg(long)]
    pub all: bool,
}

pub fn build(base: &Spec, obs: BlockPos, back: Dir, bulbs: usize) -> Option<Spec> {
    let occupied: HashSet<BlockPos> = base.blocks.iter().map(|(p, _)| *p).collect();
    let bulb = Block::parse("minecraft:waxed_copper_bulb[lit=false,powered=false]").unwrap();
    let stone = Block::parse("minecraft:stone").unwrap();
    // comparator reading the bulb behind it (its input is at pos + facing): it faces the observer
    let comparator = Block::Comparator { facing: back.opposite(), subtract: false, powered: false };
    let mut blocks = base.blocks.clone();
    let mut p = obs;
    let mut last = obs;
    for i in 0..bulbs {
        if i > 0 {
            p = p.rel(back);
            let floor = p.below();
            if occupied.contains(&p) || occupied.contains(&floor) {
                return None;
            }
            blocks.push((p, comparator));
            blocks.push((floor, stone));
        }
        p = p.rel(back);
        if occupied.contains(&p) {
            return None;
        }
        blocks.push((p, bulb));
        last = p;
    }
    Some(Spec { name: format!("{}-div{bulbs}", base.name), blocks, kick: base.kick.clone(), output: Some(last) })
}

pub fn run(a: DividerArgs) -> Result<()> {
    let base = Spec::load(&a.base)?;
    let r0 = sim::simulate(&base, &sim::Opts::default());
    if r0.verdict != Verdict::Clock {
        bail!("the base is not a clock: {:?}", r0.verdict);
    }
    println!("base {}: P={} family {}", base.name, r0.period, r0.family_name());
    let mut done = false;
    for (pos, b) in &base.blocks {
        let Block::Observer { facing, .. } = b else { continue };
        let back = facing.opposite();
        let Some(spec) = build(&base, *pos, back, a.bulbs) else { continue };
        let r = sim::simulate(&spec, &sim::Opts { horizon: 200_000, ..Default::default() });
        println!("  observer {pos} (back {}): {:?} P={} ({} x {}) family {} blocks {}", back.name(), r.verdict, r.period, r0.period, r.period as f64 / r0.period as f64, r.family_name(), spec.blocks.len());
        if r.verdict == Verdict::Clock && !done {
            if let Some(out) = &a.out {
                let mut s = spec.clone();
                s.name = out.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or(s.name);
                std::fs::write(
                    out,
                    s.to_text(&format!(
                        "Divider chain on {}: {} copper bulb(s), each comparator reading a bulb powers the next one, so each bulb halves the frequency. P = {} = {} x {} (redsim), family {}.",
                        base.name,
                        a.bulbs,
                        r.period,
                        r0.period,
                        r.period / r0.period.max(1),
                        r.family_name()
                    )),
                )?;
            }
            done = true;
            if !a.all {
                break;
            }
        }
    }
    Ok(())
}
