//! `mctas door ...`

use crate::{DoorSpec, VerifyOptions, timeline, verify};
use anyhow::Result;
use clap::Subcommand;
use mctas::design::Design;
use mctas::sim::{Worker, WorkerConfig};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DoorCmd {
    /// Check that a design is a working 3x3 piston door; print timings and block count.
    Verify {
        design: PathBuf,
        /// Print the tick-by-tick opening/closing of the 3x3 opening.
        #[arg(long)]
        timeline: bool,
        /// Open/close cycles per position.
        #[arg(long, default_value_t = 3)]
        cycles: u32,
    },
}

pub fn run(version: &str, cmd: DoorCmd) -> Result<()> {
    let cfg = WorkerConfig { version: version.to_string(), ..Default::default() };
    match cmd {
        DoorCmd::Verify { design, timeline: show, cycles } => {
            let d = Design::load(&design)?;
            let spec = DoorSpec::from_design(&d)?;
            let mut w = Worker::spawn(&cfg)?;
            let opts = VerifyOptions { cycles, keep_frames: show, ..Default::default() };
            match verify(&mut w, &d, &opts) {
                Ok(r) => {
                    println!("WORKS: {} ({} blocks, {}x{}x{})", d.name, r.blocks, r.size[0], r.size[1], r.size[2]);
                    for (i, c) in r.cycles.iter().enumerate() {
                        println!(
                            "  cycle {i}: open after {} ticks (idle at {}), closed after {} ticks (idle at {})",
                            c.open_ticks, c.open_settle, c.close_ticks, c.close_settle
                        );
                    }
                    for warning in &r.warnings {
                        println!("  warning: {warning}");
                    }
                    if show {
                        println!("opening (top row, middle row, bottom row; # block, ~ moving, h piston head, . air):");
                        print!("{}", timeline(&r.open_frames, &spec));
                        println!("closing:");
                        print!("{}", timeline(&r.close_frames, &spec));
                    }
                }
                Err(e) => {
                    println!("BROKEN: {}: {e:#}", d.name);
                    std::process::exit(1);
                }
            }
        }
    }
    Ok(())
}
