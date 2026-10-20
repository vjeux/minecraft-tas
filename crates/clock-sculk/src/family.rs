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
    /// replin | hairpin | hairpin-plain | racetrack | racetrack-plain
    name: String,
    /// Units (repeaters per line/arm, or sensors per side); `a..b` for a range.
    #[arg(long)]
    n: String,
    /// Write one design per n here.
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Write the one design (a single n) to this file (the `extend generator` form).
    #[arg(long)]
    out: Option<PathBuf>,
    /// Add the `extend` headers (family, period, blocks, rule, n range, base, generator): the
    /// family's catalog card.
    #[arg(long)]
    card: bool,
}

fn parse_range(s: &str) -> Result<(u32, u32)> {
    if let Some((a, b)) = s.split_once("..") {
        Ok((a.parse()?, b.parse()?))
    } else {
        let v: u32 = s.parse()?;
        Ok((v, v))
    }
}

/// The block the observer fires: a note block (note_block_play, 10), an oak trapdoor (block_open,
/// 10, on the pulse's rising edge) or an empty dispenser (its failed dispense 4 gt later, 10).
fn emitter_state(e: &str) -> &'static str {
    match e {
        "trapdoor" => "minecraft:oak_trapdoor[facing=north,half=bottom,open=false,powered=false,waterlogged=false]",
        "dispenser" => "minecraft:dispenser[facing=up,triggered=false]",
        _ => "minecraft:note_block[instrument=harp,note=0,powered=false]",
    }
}

/// `replin` with n repeaters.
pub fn replin(n: u32, emitter: &str) -> Build {
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
    b.put(e, emitter_state(emitter));
    b.keep_air(add(e, [0, 1, 0]));
    b
}

/// `hairpin` with n repeaters per arm: a line of n delay-4 repeaters away from the sensor, a dust
/// U-turn, n repeaters back, an observer beside the sensor watching the last one and a note block
/// diagonal to the sensor (travel 1). P = 16n + 3 (n >= 2), blocks = 4n + 7, unbounded n.
pub fn hairpin(n: u32, sensor: Sensor, emitter: &str) -> Build {
    let n = n as i32;
    let mut b = Build::default();
    let s: Pos = [0, 1, 0];
    b.put(s, sensor.state([-1, 0, 0]));
    b.output = Some(s);
    for i in 1..=n {
        // Out arm (z = 0), reading westward, and back arm (z = 1), reading eastward.
        b.put([i, 1, 0], "minecraft:repeater[delay=4,facing=west,locked=false,powered=true]");
        b.put([i, 0, 0], "minecraft:stone");
        b.put([i, 1, 1], "minecraft:repeater[delay=4,facing=east,locked=false,powered=true]");
        b.put([i, 0, 1], "minecraft:stone");
    }
    // The U-turn: two dust on stones east of the arms.
    b.put([n + 1, 1, 0], "minecraft:redstone_wire[east=none,north=none,power=0,south=side,west=side]");
    b.put([n + 1, 0, 0], "minecraft:stone");
    b.put([n + 1, 1, 1], "minecraft:redstone_wire[east=none,north=side,power=0,south=none,west=side]");
    b.put([n + 1, 0, 1], "minecraft:stone");
    // Observer south of the sensor watching the back arm's last repeater; note block west of it.
    b.put([0, 1, 1], "minecraft:observer[facing=east,powered=false]");
    let e = [-1, 1, 1];
    b.put(e, emitter_state(emitter));
    b.keep_air(add(e, [0, 1, 0]));
    b
}

/// `racetrack` with m sensors per line.
pub fn racetrack(m: u32, plain: bool) -> Build {
    let m = m as i32;
    // Calibrated: radius 16, line spacing 17 (hop 16), lines 18 apart, turn sensors 13 out (hops
    // 15). Plain: radius 8, spacing 9 (hop 8), lines 10 apart, turn sensors 6 out (hops 7).
    let (sp, w, tx) = if plain { (9, 10, 6) } else { (17, 18, 13) };
    let mut b = Build::default();
    let mut sensors: Vec<(Pos, Pos)> = Vec::new(); // (sensor, resonator) in ring order
    // Line 1 (z = 0) eastward; resonators on the east face.
    for i in 0..m {
        let p = [sp * i, 0, 0];
        sensors.push((p, add(p, [1, 0, 0])));
    }
    let xe = sp * (m - 1);
    // East turn sensor, halfway between the lines; its resonator on its south face.
    let t1 = [xe + tx, 0, w / 2];
    sensors.push((t1, add(t1, [0, 0, 1])));
    // Line 2 (z = w) westward; resonators on the west face.
    for i in (0..m).rev() {
        let p = [sp * i, 0, w];
        sensors.push((p, add(p, [-1, 0, 0])));
    }
    // West turn sensor, resonator on its north face (toward the first sensor).
    let t2 = [-tx, 0, w / 2];
    sensors.push((t2, add(t2, [0, 0, -1])));
    for (i, (p, r)) in sensors.iter().enumerate() {
        let sensor = if plain { Sensor::Plain } else { Sensor::Cal };
        if i == 0 {
            // The first sensor's back is west: its resonator (the note block, east) gets its power.
            b.put(*p, sensor.state([-1, 0, 0]));
            b.put(*r, "minecraft:note_block[instrument=harp,note=0,powered=false]");
            b.keep_air(add(*r, [0, 1, 0]));
        } else {
            b.put(*p, sensor.state([0, 0, 1]));
            b.put(*r, "minecraft:amethyst_block");
        }
    }
    b.output = Some(sensors[0].0);
    b.start = Some((sensors[0].1, "use".into()));
    b
}

/// One extendable family: builder, P = pa + pb n, blocks = ba + bb n, rule, judged n range.
struct Spec {
    pa: u32,
    pb: u32,
    ba: u32,
    bb: u32,
    lo: u32,
    hi: u32,
    rule: &'static str,
    caption: &'static str,
}

fn spec(name: &str) -> Result<Spec> {
    Ok(match name {
        "replin" => Spec { pa: 4, pb: 9, ba: 3, bb: 2, lo: 3, hi: 14, rule: "one more delay-4 repeater on a stone at the end of the line (the observer and note block move one block out): +8 gt of delay and +1 gt of travel", caption: "A calibrated sculk sensor, a straight line of n delay-4 repeaters read from it, an observer watching the last one and a note block behind it: the line's turn-on plays the note 8n+2 gt after the sensor fires and the vibration travels n+2 gt back (n <= 14 keeps the note within 16). Self-starting: the repeaters are placed powered." },
        "replin-trapdoor" => Spec { pa: 4, pb: 9, ba: 3, bb: 2, lo: 3, hi: 14, rule: "one more delay-4 repeater on a stone at the end of the line (the observer and trapdoor move one block out): +8 gt of delay and +1 gt of travel", caption: "The repeater line with an oak trapdoor as the emitter: a calibrated sculk sensor, n delay-4 repeaters, an observer watching the last one and a trapdoor behind it, opened on the observer's pulse 8n+2 gt after the sensor fires and heard n+2 gt later. Self-starting." },
        "replin-dispenser" => Spec { pa: 8, pb: 9, ba: 3, bb: 2, lo: 2, hi: 14, rule: "one more delay-4 repeater on a stone at the end of the line (the observer and dispenser move one block out): +8 gt of delay and +1 gt of travel", caption: "The repeater line with an empty dispenser as the emitter: a calibrated sculk sensor, n delay-4 repeaters, an observer watching the last one and a dispenser behind it, whose failed dispense comes 4 gt after the observer's pulse (8n+6 gt after the sensor fires) and is heard n+2 gt later. Self-starting." },
        "hairpin" => Spec { pa: 3, pb: 16, ba: 7, bb: 4, lo: 2, hi: 12, rule: "one more delay-4 repeater on a stone in each arm of the hairpin (the dust U-turn moves one block out)", caption: "A calibrated sculk sensor, n delay-4 repeaters out, a two-dust U-turn, n repeaters back, an observer beside the sensor watching the last one and a note block diagonal to the sensor: the note plays 16n+2 gt after the sensor fires and is heard 1 gt later, for any n. Self-starting." },
        "hairpin-trapdoor" => Spec { pa: 3, pb: 16, ba: 7, bb: 4, lo: 2, hi: 12, rule: "one more delay-4 repeater on a stone in each arm of the hairpin (the dust U-turn moves one block out)", caption: "The hairpin with an oak trapdoor as the emitter, diagonal to the calibrated sensor: n delay-4 repeaters out, a two-dust U-turn, n back, an observer opening the trapdoor 16n+2 gt after the sensor fires, heard 1 gt later. Self-starting." },
        "hairpin-dispenser" => Spec { pa: 7, pb: 16, ba: 7, bb: 4, lo: 1, hi: 12, rule: "one more delay-4 repeater on a stone in each arm of the hairpin (the dust U-turn moves one block out)", caption: "The hairpin with an empty dispenser as the emitter, diagonal to the calibrated sensor: n delay-4 repeaters out, a two-dust U-turn, n back, an observer firing the dispenser, whose failed dispense comes 16n+6 gt after the sensor fires, heard 1 gt later. Self-starting." },
        "hairpin-plain" => Spec { pa: 3, pb: 16, ba: 7, bb: 4, lo: 3, hi: 12, rule: "one more delay-4 repeater on a stone in each arm of the hairpin (the dust U-turn moves one block out)", caption: "The hairpin with a plain sculk sensor (re-armed 40 gt after firing, so n >= 3): n delay-4 repeaters out, a two-dust U-turn, n back, an observer and a note block next to the sensor. Self-starting." },
        "racetrack" => Spec { pa: 28, pb: 32, ba: 4, bb: 4, lo: 1, hi: 8, rule: "one more calibrated sensor + amethyst block on each straight side, 17 blocks apart (the far turn sensor moves 17 out)", caption: "A pure vibration racetrack: calibrated sculk sensors on two straight lines 18 apart (17 between sensors: hop 16) joined by a turn sensor at each end (hops 15); each sensor's resonator (amethyst; the first a note block, right-clicked as the kick) rings when it fires and only the next sensor is within 16." },
        "racetrack-plain" => Spec { pa: 12, pb: 16, ba: 4, bb: 4, lo: 3, hi: 10, rule: "one more plain sensor + amethyst block on each straight side, 9 blocks apart (the far turn sensor moves 9 out)", caption: "The racetrack with plain sculk sensors (radius 8, deaf 40 gt): lines 10 apart, 9 between sensors (hop 8), turn sensors 6 out (hops 7); m >= 3 so the next sensor has re-armed." },
        other => bail!("unknown family {other} (replin | hairpin | hairpin-plain | racetrack | racetrack-plain)"),
    })
}

fn build(name: &str, n: u32) -> Build {
    match name {
        "replin" => replin(n, "note"),
        "replin-trapdoor" => replin(n, "trapdoor"),
        "replin-dispenser" => replin(n, "dispenser"),
        "hairpin" => hairpin(n, Sensor::Cal, "note"),
        "hairpin-trapdoor" => hairpin(n, Sensor::Cal, "trapdoor"),
        "hairpin-dispenser" => hairpin(n, Sensor::Cal, "dispenser"),
        "hairpin-plain" => hairpin(n, Sensor::Plain, "note"),
        "racetrack" => racetrack(n, false),
        _ => racetrack(n, true),
    }
}

/// The archived designs that are instances of a family (same type key as instance n for some n in
/// the family's range), cards excluded; `--remove` deletes them (design, pictures, rows).
#[derive(clap::Args)]
pub struct DupesArgs {
    /// The archive directory.
    #[arg(long, default_value = "designs/clocks/archive")]
    dir: PathBuf,
    /// Delete them: the design, its pictures (designs/clocks/img/<stem>*.jpg) and its rows in
    /// verdicts-v1.tsv and captions.tsv.
    #[arg(long)]
    remove: bool,
}

pub fn dupes(a: &DupesArgs) -> Result<()> {
    let mut keys: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for name in ["replin", "replin-trapdoor", "replin-dispenser", "hairpin", "hairpin-trapdoor", "hairpin-dispenser", "hairpin-plain", "racetrack", "racetrack-plain"] {
        let sp = spec(name)?;
        for n in sp.lo..=sp.hi {
            if let Some(c) = build(name, n).clock("x") {
                keys.insert(clock::canon::design_type_key(&c.design), format!("{name} n={n}"));
            }
        }
    }
    let vp = a.dir.join("verdicts-v1.tsv");
    let text = std::fs::read_to_string(&vp)?;
    let mut header: Vec<&str> = Vec::new();
    let mut gone: Vec<String> = Vec::new();
    for l in text.lines() {
        let cols: Vec<&str> = l.split('\t').collect();
        if header.is_empty() {
            header = cols;
            continue;
        }
        let col = |k: &str| header.iter().position(|h| *h == k).and_then(|i| cols.get(i).copied()).unwrap_or("");
        let file = col("file");
        let Some(inst) = keys.get(col("type")) else { continue };
        let is_card = clock::Clock::load(&a.dir.join(file)).is_ok_and(|c| c.design.meta.contains_key("extend family"));
        if is_card {
            continue;
        }
        println!("{file}: instance of {inst}");
        gone.push(file.to_string());
    }
    if a.remove && !gone.is_empty() {
        let keep = |l: &&str| !gone.iter().any(|g| l.split('\t').nth(1) == Some(g.as_str()) || l.split('\t').next() == Some(g.as_str()));
        let v: Vec<&str> = text.lines().filter(keep).collect();
        std::fs::write(&vp, v.join("\n") + "\n")?;
        let cp = a.dir.join("captions.tsv");
        if let Ok(ct) = std::fs::read_to_string(&cp) {
            let c: Vec<&str> = ct.lines().filter(|l| !gone.iter().any(|g| l.split('\t').next() == Some(g.as_str()))).collect();
            std::fs::write(&cp, c.join("\n") + "\n")?;
        }
        let img = a.dir.parent().map(|p| p.join("img")).unwrap_or_default();
        for g in &gone {
            let _ = std::fs::remove_file(a.dir.join(g));
            let stem = g.trim_end_matches(".txt");
            for ext in [".jpg", ".strip.jpg"] {
                let _ = std::fs::remove_file(img.join(format!("{stem}{ext}")));
            }
        }
        println!("removed {} instance designs", gone.len());
    }
    Ok(())
}

pub fn family(a: &FamilyArgs) -> Result<()> {
    let (lo, hi) = parse_range(&a.n)?;
    let sp = spec(&a.name)?;
    if a.out.is_some() && lo != hi {
        bail!("--out takes one n");
    }
    for n in lo..=hi {
        let name = format!("{}-n{n}", a.name);
        let clock_name = if a.card { format!("clk-sculk-{}", a.name) } else { format!("clk-sculk-{name}") };
        let Some(mut c) = build(&a.name, n).clock(&clock_name) else { bail!("could not build {name}") };
        let gen_cmd = format!("mctas clock sculk family {} --n {{n}} --out {{out}}", a.name);
        if a.card {
            let m = &mut c.design.meta;
            m.insert("extend family".into(), format!("clk-sculk-{}", a.name));
            m.insert("extend period".into(), format!("{} + {}n", sp.pa, sp.pb));
            m.insert("extend blocks".into(), format!("{} + {}n", sp.ba, sp.bb));
            m.insert("extend rule".into(), sp.rule.into());
            m.insert("extend n".into(), format!("{}..{}", sp.lo, sp.hi));
            m.insert("extend base".into(), n.to_string());
            m.insert("extend generator".into(), gen_cmd.clone());
        }
        let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon: 20_000, margin: 4 });
        let text = format!(
            "# {} P = {} + {}n gt, {} + {}n blocks (n = {n}: P {}, {} blocks). Built by `mctas clock sculk family {} --n {n}`.\n{}",
            sp.caption,
            sp.pa,
            sp.pb,
            sp.ba,
            sp.bb,
            sp.pa + sp.pb * n,
            sp.ba + sp.bb * n,
            a.name,
            c.design.to_text()
        );
        let path = match (&a.out, &a.out_dir) {
            (Some(o), _) => o.clone(),
            (None, Some(d)) => {
                std::fs::create_dir_all(d)?;
                d.join(format!("{name}.txt"))
            }
            (None, None) => bail!("--out FILE or --out-dir DIR"),
        };
        std::fs::write(&path, text)?;
        println!("{}  [{}]", r.summary(), path.display());
    }
    let _ = dir_name;
    Ok(())
}
