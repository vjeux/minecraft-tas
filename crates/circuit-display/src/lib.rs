//! Contraption study 15: 7-segment display decoders (`circuit: seven-segment 4`, the judge in
//! crates/circuit/src/compute.rs, the board in designs/circuits/display/WR.md).
//! `mctas circuit display <command> ...` dispatches here; designs live in
//! designs/circuits/tracks/display/.

pub mod abuild;
pub mod agen;
pub mod analog;
pub mod astack;
pub mod ladder;
pub mod check;
pub mod colstack;
pub mod equalize;
pub mod glitch;
pub mod settle;
pub mod stack;
pub mod stack7;
pub mod therm;
pub mod rings;
pub mod route;
pub mod towers;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "mctas circuit display", about = "7-segment decoder generators (contraption study 15)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// The ring stack (crates/circuit-display/src/towers.rs): one ring of 4 subtract comparators
    /// per segment, stacked every `--pitch` levels; the ring corners and the comparators' rear
    /// functions come from torch towers (both literal polarities, the row functions of a1 a0),
    /// every tower and feed line routed by a small search. Writes the design to `--out`.
    Towers(towers::Opts),
    /// The compact stack (crates/circuit-display/src/towers/stack_build.rs) of a stack-search
    /// solution: rings every 2 levels in a 9x9 column, rear columns as torch chains plus injections.
    Stack(towers::stack_build::StackOpts),
    /// The compact stack's feasibility search (crates/circuit-display/src/stack.rs): segment
    /// orders x corner assignments whose 4 rear columns are torch chains plus 2 literal injections.
    StackSearch {
        /// Bit 0: no outer injection (7x7); bit 1: no Y1 side dust.
        #[arg(long, default_value_t = 0)]
        model: u8,
        /// Compact sides (letters of n, e, s, w): nothing at distance 4 there.
        #[arg(long, default_value = "")]
        compact: String,
    },
    /// The analog decoder's codes (crates/circuit-display/src/analog.rs): the 384 weight orders
    /// and complements of s = 15 - sum(w_i b_i), ranked by inner windows, then runs.
    AnalogOrders {
        #[arg(long, default_value_t = 20)]
        top: usize,
    },
    /// One analog cross ring after the chain (crates/circuit-display/src/astack.rs), for one segment.
    AnalogTest {
        #[arg(long)]
        seg: char,
        #[arg(long, default_value = "4,1,8,2")]
        w: String,
        #[arg(long, default_value_t = 15)]
        neg: u8,
        #[arg(long)]
        out: PathBuf,
    },
    /// The analog stack (crates/circuit-display/src/agen.rs): chain, riser spiral, one cross ring
    /// per segment; searches the chain placement and riser direction, writes the first clean build.
    AnalogStack {
        #[arg(long)]
        out: PathBuf,
        /// Lowest ring level.
        #[arg(long, default_value_t = 3)]
        y0: i32,
        /// Only this chain placement: `mirror,rot,tx,tz,dir` (e.g. `0,0,-1,0,1`).
        #[arg(long)]
        place: Option<String>,
        /// No riser: the chain on top, s flowing down through the rings' bridges (7x7).
        #[arg(long)]
        down: bool,
        /// The code's weights of a0..a3 (a0 complemented, the others plain).
        #[arg(long, default_value = "8,2,4,1")]
        w: String,
        /// Rows the chain's descent may use outside the 7x7 on the north side (--down).
        #[arg(long, default_value_t = 0)]
        slack: i32,
        /// The chain one level lower, standing on the top ring (--down).
        #[arg(long)]
        flat: bool,
        /// Pick each ring's layout by a timing model of its glitches (--down).
        #[arg(long)]
        score: bool,
    },
    /// Every single-bit and complement transition of a seven-segment decoder on redsim: which
    /// outputs glitch, with their waveforms (crates/circuit-display/src/glitch.rs).
    Glitches {
        design: PathBuf,
        #[arg(long)]
        verbose: bool,
    },
    /// The column stack (crates/circuit-display/src/colstack.rs): mirrored rings, 4 lossless
    /// columns, every window's two sides switching in the same tick.
    ColumnStack {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 2)]
        y0: i32,
        #[arg(long, default_value = "8,2,4,1")]
        w: String,
        /// The chain's level above the top ring: 2 (its supports on the network level) or 3.
        #[arg(long, default_value_t = 2)]
        lift: i32,
    },
    /// Slow the faster inputs' repeaters so every input reaches the blocks below y = `--below` in
    /// the same tick (crates/circuit-display/src/equalize.rs).
    Equalize {
        design: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        /// Watch every block below this y (the stack under the chain).
        #[arg(long, default_value_t = i32::MIN)]
        below: i32,
        /// Watch only this cell instead (`x,y,z`: e.g. the chain's output).
        #[arg(long)]
        at: Option<String>,
    },
    /// Hold each value on redsim and compare every tower torch (the design's `# tower` lines)
    /// with the value it should carry; prints the outputs and the lowest wrong torch per tower.
    Check {
        design: PathBuf,
        /// The frame read (the value is set at 10).
        #[arg(long, default_value_t = 200)]
        hold: u32,
        /// Cells to print per value (`x,y,z;x,y,z`).
        #[arg(long, default_value = "")]
        probe: String,
    },
    /// The thermometer decoder's codes (crates/circuit-display/src/therm.rs): codes ranked by runs,
    /// then runs touching s = 0.
    ThermOrders {
        #[arg(long)]
        neg: Option<u8>,
        #[arg(long, default_value_t = 20)]
        top: usize,
    },
    /// One thermometer strip after the chain, for one segment (crates/circuit-display/src/therm.rs).
    ThermTest {
        #[arg(long)]
        seg: char,
        #[arg(long, default_value = "8,1,2,4")]
        w: String,
        #[arg(long, default_value_t = 15)]
        neg: u8,
        #[arg(long)]
        out: PathBuf,
    },
    /// The whole thermometer decoder (crates/circuit-display/src/therm.rs): chain, spines, seven
    /// strips beside a lattice of lanes behind the display face.
    Therm {
        #[arg(long, default_value = "4,1,8,2")]
        w: String,
        #[arg(long, default_value_t = 15)]
        neg: u8,
        #[arg(long)]
        out: PathBuf,
        /// therm-v3's packing (one height per side, bands 12 apart, stairs at the entries).
        #[arg(long)]
        flat: bool,
        /// therm-v4: narrow strips (no return rows), bands 8 apart.
        #[arg(long)]
        narrow: bool,
        /// therm-v5: the tower (strips along z in two columns, stacked every 3 levels, lanes routed
        /// by crates/circuit-display/src/route.rs).
        #[arg(long)]
        tower: bool,
        /// tower: z of the strips' ruler start (lanes get z 1 ..= oz - 5).
        #[arg(long, default_value_t = 14)]
        oz: i32,
        /// tower: slot assignments x routing orders to try.
        #[arg(long, default_value_t = 200)]
        tries: usize,
        /// tower: A* expansions per lane.
        #[arg(long, default_value_t = 200000)]
        expand: usize,
        #[arg(long, default_value_t = 0)]
        seed: u64,
        /// tower: fixed slots (7 comma-separated slot indices, a f b g e c d).
        #[arg(long)]
        slots: Option<String>,
        /// tower: fixed routing order (7 comma-separated lane indices, a f b g e c d).
        #[arg(long)]
        order: Option<String>,
        /// tower: shared rulers (a pair of segments per strip where their runs allow it).
        #[arg(long)]
        shared: bool,
        /// tower: routing orders tried per promising base.
        #[arg(long, default_value_t = 8)]
        orders: usize,
        /// tower: fixed strips (`"ab dg c e f"`: a pair shares a ruler, +v side first).
        #[arg(long)]
        groups: Option<String>,
        /// tower: routing columns beyond the base's x extent.
        #[arg(long, default_value_t = 0)]
        margin: i32,
        /// tower: the s spiral's footprint depth beyond the hub row.
        #[arg(long, default_value_t = 2)]
        spiral_depth: i32,
        /// tower: column B's strip x (column A at 3).
        #[arg(long, default_value_t = 15)]
        cxb: i32,
        /// tower: the design's name.
        #[arg(long, default_value = "seven-segment-therm-tower")]
        name: String,
    },
    /// Three analog ring stacks one level above the lamps, outputs under the front axis
    /// (crates/circuit-display/src/rings.rs).
    Rings {
        #[arg(long, default_value = "4,1,8,2")]
        w: String,
        #[arg(long)]
        out: PathBuf,
        /// The copy comparators a ring may use (mask of 4).
        #[arg(long, default_value_t = 0)]
        copies: u8,
        /// Bridges every ring must leave out (bit 3: the north one, under the lane).
        #[arg(long, default_value_t = 8)]
        bridges_off: u8,
        /// The lane's x offset from the ring's axis.
        #[arg(long, default_value_t = 0)]
        lane: i32,
    },
    /// Rewrite a design's block states with its state at rest on redsim (inputs off, `--frames`
    /// after placement), so that it is settled the moment it is placed.
    Settle {
        design: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 300)]
        frames: u32,
    },
}

pub fn run(args: Vec<String>) -> Result<()> {
    let cli = Cli::try_parse_from(std::iter::once("mctas circuit display".to_string()).chain(args)).map_err(|e| anyhow::anyhow!("{e}"))?;
    match cli.cmd {
        Cmd::Towers(o) => towers::run(&o),
        Cmd::Stack(o) => towers::stack_build::run(&o),
        Cmd::StackSearch { model, compact } => {
            stack::COMPACT.store(stack::compact_mask(&compact), std::sync::atomic::Ordering::Relaxed);
            if model == 7 {
                stack7::search7()
            } else if model == 12 {
                match stack7::search_best_d() {
                    Some(s) => print!("{}", stack7::describe_c(&s)),
                    None => println!("no solution"),
                }
                Ok(())
            } else if model == 11 {
                match stack7::search_best_c() {
                    Some(s) => print!("{}", stack7::describe_c(&s)),
                    None => println!("no solution"),
                }
                Ok(())
            } else if model == 10 {
                match stack7::search_best_f() {
                    Some(s) => print!("{}", stack7::describe_f(&s)),
                    None => println!("no solution"),
                }
                Ok(())
            } else if model == 9 {
                stack7::diagnose_b();
                Ok(())
            } else if model == 8 {
                match stack7::search_best_b() {
                    Some(s) => {
                        print!("{}", stack7::describe_b(&s));
                        Ok(())
                    }
                    None => {
                        println!("no solution");
                        Ok(())
                    }
                }
            } else {
                stack::search(model)
            }
        }
        Cmd::AnalogOrders { top } => {
            analog::orders(top);
            Ok(())
        }
        Cmd::AnalogTest { seg, w, neg, out } => {
            let w: Vec<u8> = w.split(',').map(|x| x.parse().unwrap()).collect();
            let code = analog::Code { w: [w[0], w[1], w[2], w[3]], neg };
            let si = "abcdefg".find(seg).ok_or_else(|| anyhow::anyhow!("segment"))?;
            let rs = astack::seg_runs(&code, si);
            let feed = [((-3, 0, -3), 0u8)];
            let slots = astack::solve_ring(&rs, &feed).ok_or_else(|| anyhow::anyhow!("no ring for {rs:?}"))?;
            println!("runs {rs:?} slots {slots:?}");
            let mut b = abuild::B::default();
            astack::build_chain(&mut b, [0, 0, 0], &code);
            b.comp([6, 2, 0], abuild::D::E, false);
            astack::build_ring(&mut b, [10, 2, 3], &slots, &feed, 0);
            b.port(&format!("output.{seg}"), [10, 2, 3]);
            let d = b.design(&format!("analog-test-{seg}"), None);
            std::fs::write(&out, d.to_text())?;
            Ok(())
        }
        Cmd::AnalogStack { out, y0, place, down, w, slack, flat, score } => {
            if down {
                let w: Vec<u8> = w.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                anyhow::ensure!(w.len() == 4, "--w takes 4 weights");
                agen::search_down(&out, y0, &w, slack, flat, score)
            } else {
                agen::search_and_write(&out, y0, place.as_deref())
            }
        }
        Cmd::ColumnStack { out, y0, w, lift } => {
            let w: Vec<u8> = w.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            anyhow::ensure!(w.len() == 4, "--w takes 4 weights");
            colstack::search(&out, &analog::Code { w: [w[0], w[1], w[2], w[3]], neg: 1 }, y0, lift)
        }
        Cmd::Equalize { design, out, below, at } => {
            let at = match at {
                Some(s) => {
                    let v: Vec<i32> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                    anyhow::ensure!(v.len() == 3, "--at takes x,y,z");
                    Some([v[0], v[1], v[2]])
                }
                None => None,
            };
            anyhow::ensure!(at.is_some() || below != i32::MIN, "give --below or --at");
            equalize::run(&design, out.as_deref().unwrap_or(&design), below, at)
        }
        Cmd::Glitches { design, verbose } => glitch::run(&design, verbose),
        Cmd::Check { design, hold, probe } => check::run(&design, hold, &probe),
        Cmd::ThermOrders { neg, top } => {
            therm::orders(neg, top);
            Ok(())
        }
        Cmd::ThermTest { seg, w, neg, out } => therm::test(seg, &therm::parse_code(&w, neg)?, &out),
        Cmd::Therm { w, neg, out, flat, narrow, tower, oz, tries, expand, seed, slots, order, shared, orders, groups, margin, spiral_depth, cxb, name } => {
            let c = therm::parse_code(&w, neg)?;
            if tower {
                let slots: Option<Vec<usize>> = slots.map(|s| s.split(',').filter_map(|x| x.trim().parse().ok()).collect());
                let groups: Option<Vec<Vec<char>>> = groups.map(|g| g.split_whitespace().map(|x| x.chars().collect()).collect());
                let order = match order {
                    Some(s) => {
                        let v: Vec<usize> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        anyhow::ensure!(v.len() == 7, "--order takes 7 indices");
                        Some([v[0], v[1], v[2], v[3], v[4], v[5], v[6]])
                    }
                    None => None,
                };
                therm::full_tower(&c, &therm::TowerOpts { oz, tries, expand, seed, slots, shared, orders, groups, order, margin, spiral_depth, cxb, name }, &out)
            } else if flat || narrow {
                therm::full_flat(&c, narrow, &out)
            } else {
                therm::full(&c, &out)
            }
        }
        Cmd::Rings { w, out, copies, bridges_off, lane } => {
            if w == "all" {
                for c in analog::all_codes().into_iter().filter(|c| c.neg == 15) {
                    match rings::build(&c, copies, bridges_off, lane, &out) {
                        Ok(()) => {
                            println!("code w {:?} fits", c.w);
                            return Ok(());
                        }
                        Err(e) => println!("code w {:?}: {e}", c.w),
                    }
                }
                anyhow::bail!("no code fits")
            }
            rings::build(&therm::parse_code(&w, 15)?, copies, bridges_off, lane, &out)
        }
        Cmd::Settle { design, out, frames } => settle::run(&design, out.as_deref().unwrap_or(&design), frames),
    }
}
