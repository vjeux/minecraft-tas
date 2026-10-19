//! `mctas clock sculk family`: the EXTENDABLE sculk clocks (a repeated unit; P = base + n * inc,
//! blocks = b0 + n * db), one design per n, for the catalog's extendable cards.
//!
//! * `replin`: a calibrated sensor, a straight line of n delay-4 repeaters (each on a stone) read
//!   from it, an observer watching the last repeater and a note block behind the observer. The
//!   line's turn-on reaches the note block 8n + 2 gt after the sensor fires and the note is
//!   n + 2 blocks away: P = 9n + 4, blocks = 2n + 3, for 3 <= n <= 14 (8n + 2 >= 20: the sensor
//!   has re-armed; n + 2 <= 16: in range). Unit: one repeater + its stone (+9 gt, +2 blocks).
//!   Self-starting: the repeaters are placed powered with no input; their turn-off is the kick.
//! * `racetrack`: a ring of calibrated sensors on two straight lines 18 apart, joined by one turn
//!   sensor at each end; every sensor has a resonator (amethyst; the first one a note block,
//!   right-clicked as the kick). Line sensors are 17 apart (hop 16: the resonator on the face
//!   toward the next sensor), the turns are hops of 15, so a sensor two hops ahead is always out
//!   of range. m sensors per line: P = 32m + 28, blocks = 4m + 4. Unit: one sensor + resonator on
//!   each line (+32 gt, +4 blocks).

use crate::build::{add, dir_name, Build, Sensor};
use anyhow::{bail, Result};
use clock::Pos;
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct FamilyArgs {
    /// replin | racetrack
    name: String,
    /// Units: repeaters (replin) or sensors per line (racetrack); `a..b` for a range.
    #[arg(long)]
    n: String,
    /// Write one design per n here.
    #[arg(long)]
    out_dir: PathBuf,
}

fn parse_range(s: &str) -> Result<(u32, u32)> {
    if let Some((a, b)) = s.split_once("..") {
        Ok((a.parse()?, b.parse()?))
    } else {
        let v: u32 = s.parse()?;
        Ok((v, v))
    }
}

/// `replin` with n repeaters.
pub fn replin(n: u32) -> Build {
    let mut b = Build::default();
    let s: Pos = [0, 1, 0];
    // The sensor faces east... its back (west) is empty; it powers the first repeater east of it.
    b.put(s, Sensor::Cal.state([-1, 0, 0]));
    b.output = Some(s);
    for i in 1..=n as i32 {
        b.put([i, 1, 0], "minecraft:repeater[delay=4,facing=west,locked=false,powered=true]");
        b.put([i, 0, 0], "minecraft:stone");
    }
    let o = [n as i32 + 1, 1, 0];
    b.put(o, "minecraft:observer[facing=west,powered=false]");
    let e = [n as i32 + 2, 1, 0];
    b.put(e, "minecraft:note_block[instrument=harp,note=0,powered=false]");
    b.keep_air(add(e, [0, 1, 0]));
    b
}

/// `racetrack` with m sensors per line.
pub fn racetrack(m: u32) -> Build {
    let m = m as i32;
    let mut b = Build::default();
    let mut sensors: Vec<(Pos, Pos)> = Vec::new(); // (sensor, resonator) in ring order
    // Line 1 (z = 0) eastward: x = 0, 17, ..; resonators on the east face.
    for i in 0..m {
        let p = [17 * i, 0, 0];
        sensors.push((p, add(p, [1, 0, 0])));
    }
    let xe = 17 * (m - 1);
    // East turn sensor: 13 east of the line ends, halfway; its resonator on its south face.
    let t1 = [xe + 13, 0, 9];
    // The last line-1 sensor's resonator: its east face (toward t1).
    sensors.push((t1, add(t1, [0, 0, 1])));
    // Line 2 (z = 18) westward; resonators on the west face.
    for i in (0..m).rev() {
        let p = [17 * i, 0, 18];
        sensors.push((p, add(p, [-1, 0, 0])));
    }
    // West turn sensor, resonator on its north face (toward the first sensor).
    let t2 = [-13, 0, 9];
    sensors.push((t2, add(t2, [0, 0, -1])));
    for (i, (p, r)) in sensors.iter().enumerate() {
        if i == 0 {
            // The first sensor faces away from... its resonator (the note block) gets its power.
            b.put(*p, Sensor::Cal.state([-1, 0, 0]));
            b.put(*r, "minecraft:note_block[instrument=harp,note=0,powered=false]");
            b.keep_air(add(*r, [0, 1, 0]));
        } else {
            b.put(*p, "minecraft:calibrated_sculk_sensor[facing=north,power=0,sculk_sensor_phase=inactive,waterlogged=false]");
            b.put(*r, "minecraft:amethyst_block");
        }
    }
    b.output = Some(sensors[0].0);
    b.start = Some((sensors[0].1, "use".into()));
    b
}

pub fn family(a: &FamilyArgs) -> Result<()> {
    let (lo, hi) = parse_range(&a.n)?;
    std::fs::create_dir_all(&a.out_dir)?;
    for n in lo..=hi {
        let (b, formula, rule) = match a.name.as_str() {
            "replin" => (replin(n), format!("P = 9n + 4 = {}, blocks = 2n + 3 = {}", 9 * n + 4, 2 * n + 3), "one more delay-4 repeater (on a stone) in the line: +9 gt, +2 blocks; 3 <= n <= 14"),
            "racetrack" => (racetrack(n), format!("P = 32m + 28 = {}, blocks = 4m + 4 = {}", 32 * n + 28, 4 * n + 4), "one more calibrated sensor + amethyst on each straight side (17 apart): +32 gt, +4 blocks; m >= 1"),
            other => bail!("unknown family {other} (replin | racetrack)"),
        };
        let name = format!("{}-n{n}", a.name);
        let Some(mut c) = b.clock(&format!("clk-sculk-{name}")) else { bail!("could not build {name}") };
        c.design.meta.insert("family".into(), format!("{} n={n}: {formula}; unit: {rule}", a.name));
        let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 20_000, margin: 4 });
        let text = format!(
            "# Extendable sculk clock `{}` with n = {n} ({formula}); unit: {rule}. Built by `mctas clock sculk family {} --n {n}`.\n{}",
            a.name,
            a.name,
            c.design.to_text()
        );
        let path = a.out_dir.join(format!("{name}.txt"));
        std::fs::write(&path, text)?;
        println!("{}  [{}]", r.summary(), path.display());
    }
    let _ = dir_name;
    Ok(())
}
