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
pub mod chests;
pub mod colstack;
pub mod obvious;
pub mod counter;
pub mod equalize;
pub mod glitch;
pub mod settle;
pub mod stack;
pub mod stack7;
pub mod therm;
pub mod rand4;
pub mod rand8;
pub mod seqgen;
pub mod seqgen2;
pub mod randsearch;
pub mod rings;
pub mod rings2;
pub mod route;
pub mod towers;
pub mod trim;

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
    /// The obvious 7-segment display for one signal strength (designs/briefs/display-7seg.md
    /// layout 1; crates/circuit-display/src/obvious.rs).
    Obvious {
        #[arg(long)]
        out: std::path::PathBuf,
        #[arg(long, default_value = "seven-segment-obvious")]
        name: String,
        #[arg(long, default_value_t = 300000)]
        expand: usize,
    },
    /// A storage counter's layout probe (circuit::compute::counter_probe): redsim holds the chest at
    /// each level and prints what the display shows, against --table when given.
    CounterProbe {
        design: std::path::PathBuf,
        /// The intended table, 16 numbers (levels 0..15), comma- or space-separated.
        #[arg(long)]
        table: Option<String>,
        #[arg(long, default_value_t = 60)]
        hold: u32,
        #[arg(long, default_value_t = 3)]
        margin: i32,
    },
    /// A storage counter's cheapest tables (crates/circuit-display/src/counter.rs): per chest
    /// level, the allowed number that needs the fewest analog windows over all 14 segments.
    CounterTables {
        /// percent or stacks
        #[arg(long, default_value = "percent")]
        table: String,
        #[arg(long, default_value_t = 2000)]
        restarts: u32,
        #[arg(long, default_value_t = 10)]
        top: usize,
        /// Score each lamp on its own (a tap-tree layout: thresholds 1, inner windows 3).
        #[arg(long)]
        per_lamp: bool,
        /// With --per-lamp: an inner window's cost.
        #[arg(long, default_value_t = 3)]
        inner: u32,
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
        /// The judge's own stimulus instead (its Gray-code walk and Euler tour, its gap): each
        /// phase the judge counts as a glitch.
        #[arg(long)]
        judge: bool,
    },
    /// Every constant hopper (no container above or below) into a single chest with the same
    /// comparator signal: no open hopper top for the item intrusion check, no block added
    /// (crates/circuit-display/src/chests.rs).
    Chests {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// A rings2 design finished: `equalize --at` (the chain's output), `settle`, `chests`, the name.
    Finish {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        name: String,
        /// The cell the inputs' arrivals are equalised at (`x,y,z`: the chain's output).
        #[arg(long, default_value = "2,4,13")]
        at: String,
    },
    /// The stones a design does not need: each tried alone on the redsim judge (glitch-free, box
    /// no larger), then the spare ones removed one at a time (crates/circuit-display/src/trim.rs).
    Trim {
        design: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 16)]
        threads: usize,
        /// Try every block, not only stones.
        #[arg(long)]
        all: bool,
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
    /// rings, solved first and wired after (crates/circuit-display/src/rings2.rs, clk-item): with
    /// `--lean` a lower ring is fed by the ring above's coupling alone when that fits.
    Rings2 {
        #[arg(long, default_value = "4,1,8,2")]
        w: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 15)]
        copies: u8,
        #[arg(long, default_value_t = 8)]
        bridges_off: u8,
        #[arg(long)]
        lean: bool,
        /// Leave out arc corners and cells a ring does not need (crates/circuit-display/src/rings2.rs prune).
        #[arg(long)]
        prune: bool,
        /// Prune orders tried per ring (the cheapest whose coupling still feeds the ring below).
        #[arg(long, default_value_t = 1)]
        tries: u64,
        /// The side stacks one cell closer, with their line feeds nearest the middle.
        #[arg(long)]
        tight: bool,
        /// Each lane's first repeater at this delay (ticks): dips shorter than it never reach the lamp.
        #[arg(long, default_value_t = 0)]
        deglitch: u8,
        /// Segments whose rings compute the complement, inverted back by a torch between two
        /// filtering repeaters in the lane (the middle stack's a/g/d: their short pulses go too).
        #[arg(long, default_value = "")]
        off: String,
        /// Segments whose rings must be race-free (clk-obs): every inner window a transition crosses
        /// with the segment off at both ends gets its O contact's s no later than its W contact's
        /// and at most 2 gt earlier (astack::PULSE_RUNS; line and coupling times modelled).
        #[arg(long, default_value = "")]
        timing: String,
        /// Rings that must have their own line feed (`g`), whatever the coupling gives.
        #[arg(long, default_value = "")]
        direct: String,
        /// Extra bridges left out per ring: `a=4,g=1` (bit i: astack::BRIDGES[i]).
        #[arg(long, default_value = "")]
        ring_bridges_off: String,
        /// Ring a's line straight up from the hub onto the arc cell east of its rear hopper (its
        /// back bridge then points west: the ring below is coupled west of its hopper first).
        #[arg(long)]
        middle_east: bool,
        /// Print every ring's race report (its inner windows' contact times, the watched runs).
        #[arg(long)]
        report: bool,
        /// The b/c stack's line onto the arc cell east of b's rear hopper (as --middle-east).
        #[arg(long)]
        side_east: bool,
        /// The chain one level down and four cells forward, under the lines (inputs from z = 14):
        /// the box four cells shallower.
        #[arg(long)]
        chain_front: bool,
        /// With --chain-front --side-east: b's line straight from the chain's first s block, the hub
        /// climbed from the west (b's line no longer runs from the hub).
        #[arg(long)]
        hub_west: bool,
    },
    /// Every subset of `--candidates` as the rings2 `--off` segments, built in parallel with the
    /// other options; one line per subset, the cheapest first (crates/circuit-display/src/rings2.rs
    /// sweep, clk-obs).
    Rings2Sweep {
        #[arg(long, default_value = "4,1,8,2")]
        w: String,
        #[arg(long, default_value = "/tmp/rings2-sweep")]
        out_dir: PathBuf,
        #[arg(long, default_value_t = 15)]
        copies: u8,
        #[arg(long, default_value_t = 8)]
        bridges_off: u8,
        #[arg(long)]
        lean: bool,
        #[arg(long)]
        prune: bool,
        #[arg(long, default_value_t = 1)]
        tries: u64,
        #[arg(long)]
        tight: bool,
        #[arg(long, default_value_t = 0)]
        deglitch: u8,
        /// The segments that may compute their complement.
        #[arg(long, default_value = "abcdefg")]
        candidates: String,
        #[arg(long, default_value = "")]
        timing: String,
        #[arg(long, default_value = "")]
        direct: String,
        #[arg(long, default_value = "")]
        ring_bridges_off: String,
        #[arg(long)]
        middle_east: bool,
        #[arg(long)]
        side_east: bool,
        #[arg(long)]
        chain_front: bool,
        #[arg(long)]
        hub_west: bool,
        #[arg(long, default_value_t = 8)]
        threads: usize,
        /// Finish the K cheapest and run them through the judge's stimulus on redsim (their
        /// glitching phases).
        #[arg(long, default_value_t = 0)]
        judge: usize,
        /// The codes swept: `all` (every order of the weights 1, 2, 4, 8) or `4,1,8,2;1,2,4,8`
        /// (default: --w alone).
        #[arg(long, default_value = "")]
        codes: String,
    },
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
    /// The layout search for randomizers with `--k` token pairs (crates/circuit-display/src/randsearch.rs):
    /// writes its layout lines to `--out` (build them with `rand4 --pic <file>`).
    RandSearch {
        #[arg(long, default_value_t = 2)]
        k: usize,
        /// Only layouts whose box is at most this volume.
        #[arg(long, default_value_t = 210)]
        bound: i32,
        /// Every complete layout within the bound, not only each new best.
        #[arg(long)]
        all: bool,
        /// Dropper arrangements over a whole window around pair 0, not only the near ones.
        #[arg(long)]
        wide: bool,
        /// Refuse layouts with a quasi-connectivity hazard (a dust's support next to a dropper's
        /// top cell): fewer layouts, none that leaves a dropper stuck triggered.
        #[arg(long)]
        qc_safe: bool,
        /// Leave room in front of every output for randomizer 8's second stage (`rand8`).
        #[arg(long)]
        stage2: bool,
        /// Randomizer 8 from k = 2: every output twice, each with a sink dust at its other side
        /// for a third coin's bit (`rand8` routes it).
        #[arg(long)]
        dup: bool,
        #[arg(long)]
        out: PathBuf,
    },
    /// Randomizer 8 in two stages from randomizer-4 layout lines (crates/circuit-display/src/rand8.rs):
    /// every line of `--pic` that builds is written as <out>/<name>-<i>.txt.
    Rand8 {
        #[arg(long)]
        pic: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "rand8")]
        name: String,
    },
    /// A button sequence lock of 2 buttons and an L-press code (`circuit: sequence-lock 2 L`):
    /// fixed tiles joined by routed dust (crates/circuit-display/src/seqgen.rs).
    Seqlock {
        #[arg(long, default_value_t = 3)]
        l: u32,
        /// The code as built (letters a, b), one per step.
        #[arg(long, default_value = "aba")]
        code: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "seqlock")]
        name: String,
        /// Generator v2: lanes and observer taps, no bus crossings (seqgen2.rs).
        #[arg(long)]
        v2: bool,
    },
    /// The randomizer-4 decoder found by the 3D layout search (crates/circuit-display/src/rand4.rs);
    /// `--pic` builds any layout line the search prints instead.
    Rand4 {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        pic: Option<String>,
        #[arg(long, default_value = "rand4")]
        name: String,
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
        Cmd::Obvious { out, name, expand } => obvious::build(&name, &out, expand),
        Cmd::CounterProbe { design, table, hold, margin } => {
            // debugging probes: `port output.probe<name>: x y z` lines are taken out before the
            // kind's port check and watched as extra outputs (their levels print per line)
            let text = std::fs::read_to_string(&design)?;
            let mut extra: Vec<circuit::Probe> = Vec::new();
            let mut kept = String::new();
            for l in text.lines() {
                if let Some(rest) = l.strip_prefix("port output.probe") {
                    let (n, p) = rest.split_once(':').ok_or_else(|| anyhow::anyhow!("bad probe line {l:?}"))?;
                    let v: Vec<i32> = p.split_whitespace().map(|w| w.parse()).collect::<std::result::Result<_, _>>()?;
                    extra.push(circuit::Probe { name: format!("probe{n}"), pos: [v[0], v[1], v[2]] });
                } else {
                    kept.push_str(l);
                    kept.push('\n');
                }
            }
            let tmp = std::env::temp_dir().join(format!("counter-probe-{}.txt", std::process::id()));
            std::fs::write(&tmp, kept)?;
            let mut c = circuit::Circuit::load(&tmp)?;
            c.outputs.extend(extra);
            let want = match table {
                Some(t) => {
                    let v: Vec<u64> = t.split([',', ' ']).filter(|w| !w.is_empty()).map(|w| w.parse()).collect::<std::result::Result<_, _>>()?;
                    anyhow::ensure!(v.len() == 16, "--table: 16 numbers (levels 0..15), got {}", v.len());
                    let mut a = [0u64; 16];
                    a.copy_from_slice(&v);
                    Some(a)
                }
                None => None,
            };
            print!("{}", circuit::compute::counter_probe(&c, want.as_ref(), hold, margin)?);
            Ok(())
        }
        Cmd::CounterTables { table, restarts, top, per_lamp, inner } => {
            let t = circuit::compute::Table::parse(&table).ok_or_else(|| anyhow::anyhow!("--table percent | stacks"))?;
            counter::PER_LAMP.with(|p| p.set(per_lamp));
            counter::INNER.with(|p| p.set(inner));
            counter::print(t, restarts, top);
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
        Cmd::Glitches { design, verbose, judge } => {
            if judge {
                for l in glitch::run_judge(&design)? {
                    println!("{l}");
                }
                Ok(())
            } else {
                glitch::run(&design, verbose)
            }
        }
        Cmd::Chests { design, out } => chests::run(&design, &out),
        Cmd::Trim { design, out, threads, all } => trim::run(&design, &out, threads, all),
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
        Cmd::Rings2 { w, out, copies, bridges_off, lean, prune, tries, tight, deglitch, off, timing, direct, ring_bridges_off, middle_east, report, side_east, chain_front, hub_west } => {
            let opts = rings2::Extra { timing, direct, ring_bridges_off, middle_east, report, side_east, quiet: false, chain_front, hub_west };
            rings2::build(&therm::parse_code(&w, 15)?, copies, bridges_off, lean, prune, tries, tight, deglitch, &off, &opts, &out).map(|_| ())
        }
        Cmd::Rings2Sweep { w, out_dir, copies, bridges_off, lean, prune, tries, tight, deglitch, candidates, timing, direct, ring_bridges_off, middle_east, side_east, chain_front, hub_west, threads, judge, codes } => {
            let opts = rings2::Extra { timing, direct, ring_bridges_off, middle_east, report: false, side_east, quiet: true, chain_front, hub_west };
            // the codes: --w alone, a `;` list, or `all` (every order of the weights 1, 2, 4, 8)
            let list: Vec<analog::Code> = if codes == "all" {
                let mut v = Vec::new();
                for a in [1u8, 2, 4, 8] {
                    for b in [1u8, 2, 4, 8] {
                        for c in [1u8, 2, 4, 8] {
                            for d in [1u8, 2, 4, 8] {
                                let mut s = [a, b, c, d];
                                s.sort();
                                if s == [1, 2, 4, 8] {
                                    v.push(analog::Code { w: [a, b, c, d], neg: 15 });
                                }
                            }
                        }
                    }
                }
                v
            } else if codes.is_empty() {
                vec![therm::parse_code(&w, 15)?]
            } else {
                codes.split(';').map(|c| therm::parse_code(c.trim(), 15)).collect::<Result<Vec<_>>>()?
            };
            rings2::sweep(&list, copies, bridges_off, lean, prune, tries, tight, deglitch, &candidates, &opts, &out_dir, threads, judge)
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
        Cmd::Seqlock { out, name, .. } if name == "prim4" => seqgen::prim4(&out),
        Cmd::Seqlock { l, code, out, name, v2 } if v2 => seqgen2::build(l, &code, &name, &out),
        Cmd::Seqlock { l, code, out, name, .. } => seqgen::build(l, &code, &name, &out),
        Cmd::Rand8 { pic, out, name } => {
            std::fs::create_dir_all(&out)?;
            for (i, line) in std::fs::read_to_string(&pic)?.lines().enumerate().filter(|(_, l)| l.contains("lo (")) {
                let n = format!("{name}-{i}");
                if let Err(e) = rand8::build(line, &n, &out.join(format!("{n}.txt"))) {
                    println!("{n}: {e}");
                }
            }
            Ok(())
        }
        Cmd::RandSearch { k, bound, all, wide, qc_safe, stage2, dup, out } => {
            randsearch::QC_SAFE.store(qc_safe, std::sync::atomic::Ordering::Relaxed);
            randsearch::STAGE2.store(stage2, std::sync::atomic::Ordering::Relaxed);
            randsearch::DUP.store(dup, std::sync::atomic::Ordering::Relaxed);
            let lines = randsearch::run(k, bound, all, wide);
            std::fs::write(&out, lines.join("\n") + "\n")?;
            println!("{} layouts -> {}", lines.len(), out.display());
            Ok(())
        }
        Cmd::Rand4 { out, pic, name } => match pic {
            // a file of layout lines: every line that routes is written as <out>/<name>-<i>.txt
            Some(file) if std::path::Path::new(&file).is_file() => {
                std::fs::create_dir_all(&out)?;
                for (i, line) in std::fs::read_to_string(&file)?.lines().enumerate().filter(|(_, l)| l.contains("lo (")) {
                    let n = format!("{name}-{i}");
                    if let Err(e) = rand4::from_pic(line, &n, &out.join(format!("{n}.txt"))) {
                        println!("{n}: {e}");
                    }
                }
                Ok(())
            }
            Some(line) => rand4::from_pic(&line, &name, &out),
            None => rand4::build(&out),
        },
        Cmd::Settle { design, out, frames } => settle::run(&design, out.as_deref().unwrap_or(&design), frames),
        Cmd::Finish { design, out, name, at } => {
            let v: Vec<i32> = at.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            anyhow::ensure!(v.len() == 3, "--at takes x,y,z");
            finish(&design, &out, &name, [v[0], v[1], v[2]])?;
            println!("{}: {name}", out.display());
            Ok(())
        }
    }
}

/// The rings2 finish (clk-obs): the inputs equalised at `at` (the chain's output), the states at
/// rest, the intake hoppers into chests, the name.
pub fn finish(design: &std::path::Path, out: &std::path::Path, name: &str, at: [i32; 3]) -> Result<()> {
    equalize::run(design, out, i32::MIN, Some(at))?;
    settle::run(out, out, 300)?;
    chests::run(out, out)?;
    let text = std::fs::read_to_string(out)?;
    let renamed: String = text.lines().map(|l| if l.starts_with("name: ") { format!("name: {name}\n") } else { format!("{l}\n") }).collect();
    std::fs::write(out, renamed)?;
    Ok(())
}
