//! The 3x3 piston door.
//!
//! A design is a [`Design`] with two ports:
//! * `door`: the lowest corner (min x, min y) of the 3x3 opening. The door is the vertical
//!   plane `z = door.z` covering `x in door.x..door.x+3`, `y in door.y..door.y+3`.
//! * `lever`: a lever. On opens the door, off closes it.
//!
//! What "works" means (see docs/piston-door-3x3.md):
//! * at rest with the lever off, the 9 door cells hold full-cube blocks and the cells right in
//!   front of and behind the door (z-1 and z+1) are walkable (no collision);
//! * after the lever is switched on and everything settles, the 9 door cells and those 18
//!   corridor cells are all walkable;
//! * after the lever is switched off and everything settles, every block of the build is back
//!   exactly where it started (so the door is the same blocks, and it can cycle forever);
//! * no item entity appears (nothing was broken), repeated for several cycles, at several
//!   positions in the world (Minecraft has location-dependent update orders).

pub mod cli;
pub mod mechsynth;
pub mod archive;
pub mod designs;
pub mod designs_history;
pub mod designs_fresh_b;
pub mod designs_top;
pub mod designs_center_mech;
pub mod observer_seq;
pub mod leaderboard;
pub mod designs_gravity;
pub mod lm;
pub mod drive;
pub mod harvest;
pub mod ideal;
pub mod route;
pub mod minimize;
pub mod patch;
pub mod place;
pub mod search;
pub mod showcase;
pub mod video;
pub mod vanilla_check;

use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pos, Region, SandboxOptions, Worker};

/// Where the door and its lever are.
#[derive(Clone, Copy, Debug)]
pub struct DoorSpec {
    pub door: Pos,
    pub lever: Pos,
}

impl DoorSpec {
    pub fn from_design(d: &Design) -> Result<DoorSpec> {
        let door = *d.ports.get("door").context("design has no 'port door: x y z'")?;
        let lever = *d.ports.get("lever").context("design has no 'port lever: x y z'")?;
        Ok(DoorSpec { door, lever })
    }

    /// The 9 cells of the opening, row by row from the top (y desc), west to east.
    pub fn door_cells(&self) -> Vec<Pos> {
        let [x, y, z] = self.door;
        let mut out = Vec::new();
        for dy in (0..3).rev() {
            for dx in 0..3 {
                out.push([x + dx, y + dy, z]);
            }
        }
        out
    }

    /// The walkway: the 3x3 opening extended 3 blocks in front of and behind the door, so you
    /// can actually walk up to and through it.
    pub fn corridor_cells(&self) -> Vec<Pos> {
        let [x, y, z] = self.door;
        let mut out = Vec::new();
        for dz in [-3, -2, -1, 1, 2, 3] {
            for dy in 0..3 {
                for dx in 0..3 {
                    out.push([x + dx, y + dy, z + dz]);
                }
            }
        }
        out
    }
}

/// Knobs of [`verify`].
#[derive(Clone, Debug)]
pub struct VerifyOptions {
    /// Open/close cycles per position.
    pub cycles: u32,
    /// Cap on ticks to wait for the contraption to settle after each lever flip.
    pub max_ticks: u32,
    /// Positions (offsets added to the design) to test at.
    pub offsets: Vec<Pos>,
    /// Keep the per-tick frames of the first cycle (for timelines and videos).
    pub keep_frames: bool,
}

impl Default for VerifyOptions {
    fn default() -> Self {
        VerifyOptions {
            cycles: 3,
            max_ticks: 400,
            offsets: vec![[0, 0, 0], [-7, 3, 5], [5, -11, -6], [-3, 40, 2]],
            keep_frames: false,
        }
    }
}

/// Timing of one open/close cycle, in game ticks after the lever flip.
#[derive(Clone, Debug)]
pub struct CycleReport {
    /// First tick after which the opening stays walkable.
    pub open_ticks: u32,
    /// Ticks until everything was idle after opening.
    pub open_settle: u32,
    /// First tick after which the door is back (all 9 cells full and identical to the start).
    pub close_ticks: u32,
    pub close_settle: u32,
}

/// Result of [`verify`].
#[derive(Clone, Debug)]
pub struct Report {
    pub blocks: usize,
    pub size: [i32; 3],
    pub cycles: Vec<CycleReport>,
    /// Frames of the door area for the first open and first close (when kept).
    pub open_frames: Vec<Region>,
    pub close_frames: Vec<Region>,
    /// Warnings that do not fail the door (e.g. the file was not in its rest state).
    pub warnings: Vec<String>,
}

/// Verdict: Ok(report) when the door works everywhere, Err(reason) otherwise.
pub fn verify(w: &mut Worker, design: &Design, opts: &VerifyOptions) -> Result<Report> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = verify_in(w, sb, design, opts);
    let _ = w.drop_sandbox(sb);
    r
}

/// Count of placed blocks: every non-air block except piston heads (part of their piston).
pub fn block_count(d: &Design) -> usize {
    d.blocks.values().filter(|s| !s.starts_with("minecraft:piston_head")).count()
}

/// `n` deterministic pseudo-random extra test positions, on top of the default four.
pub fn extra_offsets(n: u32) -> Vec<Pos> {
    let mut seed: u64 = 0x9e3779b97f4a7c15;
    let mut next = |m: i64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % (2 * m as u64 + 1)) as i32 - m as i32
    };
    (0..n).map(|_| [next(13), next(40), next(13)]).collect()
}

/// Blocks a survival player cannot place (creative-only or technical); designs may not use them.
const FORBIDDEN: &[&str] = &[
    "command_block", "chain_command_block", "repeating_command_block", "structure_block", "structure_void",
    "jigsaw", "barrier", "light", "bedrock", "end_portal_frame", "end_portal", "end_gateway", "nether_portal",
    "spawner", "trial_spawner", "vault", "reinforced_deepslate", "budding_amethyst", "petrified_oak_slab",
    "moving_piston", "test_block", "test_instance_block", "frosted_ice", "bubble_column", "fire", "soul_fire",
];

/// Gravity-affected full blocks. A door made of these is a different category (it can fall).
pub fn is_gravity_block(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    let n = name.trim_start_matches("minecraft:");
    n.ends_with("concrete_powder") || matches!(n, "sand" | "red_sand" | "gravel" | "suspicious_sand" | "suspicious_gravel")
}

/// True when the design uses no slime and no honey blocks anywhere (its own leaderboard).
pub fn slime_free(d: &Design) -> bool {
    !d.blocks.values().any(|s| s.contains("slime_block") || s.contains("honey_block"))
}

/// Blocks standing in the walkway at rest. The rules allow non-colliding blocks there (dust,
/// torches, levers), but a clean doorway has none, so the leaderboards show the count.
pub fn walkway_clutter(d: &Design) -> usize {
    let Ok(spec) = DoorSpec::from_design(d) else { return 0 };
    spec.corridor_cells().iter().filter(|p| d.blocks.get(*p).is_some_and(|s| !mctas::blockstate::is_air(s))).count()
}

/// The door category of a design: "solid" (normal blocks), "gravity" (sand-like door blocks),
/// or "sculk" (uses sculk sensors: vibrations from players or mobs nearby can trigger them, so
/// those doors are ranked separately).
pub fn category(d: &Design) -> &'static str {
    let door = d.ports.get("door").and_then(|p| d.blocks.get(p));
    if door.is_some_and(|s| is_gravity_block(s)) {
        "gravity"
    } else if d.blocks.values().any(|s| s.contains("sculk_sensor")) {
        "sculk"
    } else {
        "solid"
    }
}

/// Static legality: no creative-only blocks, no infested blocks, and every piston head belongs to
/// an extended piston facing the same way (no headless/orphan heads).
pub fn legality_problems(d: &Design) -> Vec<String> {
    let mut out = Vec::new();
    for (p, s) in &d.blocks {
        let name = mctas::blockstate::block_name(s);
        let n = name.trim_start_matches("minecraft:");
        if FORBIDDEN.contains(&n) || n.starts_with("infested_") {
            out.push(format!("{p:?} is {s}: not placeable in survival"));
        }
        let facing = |st: &str| BlockStateLite::get(st, "facing");
        if n == "piston_head" {
            let f = facing(s).unwrap_or_default();
            let back = match f.as_str() {
                "up" => [p[0], p[1] - 1, p[2]],
                "down" => [p[0], p[1] + 1, p[2]],
                "north" => [p[0], p[1], p[2] + 1],
                "south" => [p[0], p[1], p[2] - 1],
                "east" => [p[0] - 1, p[1], p[2]],
                _ => [p[0] + 1, p[1], p[2]],
            };
            let ok = d.blocks.get(&back).is_some_and(|b| {
                let bn = mctas::blockstate::block_name(b);
                (bn == "minecraft:piston" || bn == "minecraft:sticky_piston")
                    && BlockStateLite::get(b, "extended").as_deref() == Some("true")
                    && facing(b).as_deref() == Some(f.as_str())
            });
            if !ok {
                out.push(format!("{p:?} is a piston head without its extended piston behind it"));
            }
        }
    }
    out
}

/// Minimal property reader for "minecraft:x[a=b,c=d]" strings.
struct BlockStateLite;
impl BlockStateLite {
    fn get(state: &str, key: &str) -> Option<String> {
        let props = state.split_once('[')?.1.trim_end_matches(']');
        props.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().to_string()))
    }
}

/// [`verify`] in an existing sandbox (it is cleared first; reuse saves creating levels).
pub fn verify_in(w: &mut Worker, sb: u32, design: &Design, opts: &VerifyOptions) -> Result<Report> {
    let illegal = legality_problems(design);
    if !illegal.is_empty() {
        bail!("illegal design: {}", illegal.join("; "));
    }
    let spec = DoorSpec::from_design(design)?;
    let (lo, hi) = design.bounds().context("empty design")?;
    let size = [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1];
    let mut report = Report {
        blocks: block_count(design),
        size,
        cycles: Vec::new(),
        open_frames: Vec::new(),
        close_frames: Vec::new(),
        warnings: Vec::new(),
    };
    let result = (|| -> Result<()> {
        for (i, off) in opts.offsets.iter().enumerate() {
            let d = design.translated(*off);
            let s = DoorSpec::from_design(&d)?;
            verify_at(w, sb, &d, &s, opts, &mut report, i == 0).with_context(|| format!("at offset {off:?}"))?;
        }
        Ok(())
    })();
    result?;
    let _ = spec;
    Ok(report)
}

/// The box the verifier watches: the design plus a margin (blocks can move out of it).
pub(crate) fn watch_box(d: &Design, spec: &DoorSpec) -> (Pos, Pos) {
    let (mut lo, mut hi) = d.bounds().unwrap();
    for p in spec.door_cells().iter().chain(spec.corridor_cells().iter()) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    ([lo[0] - 2, lo[1] - 2, lo[2] - 2], [hi[0] + 2, hi[1] + 2, hi[2] + 2])
}

fn is_moving(state: &str) -> bool {
    state.starts_with("minecraft:moving_piston")
}

/// Blocks that may not be door blocks: the door must be made of plain building blocks.
fn is_mechanism_block(state: &str) -> bool {
    let name = mctas::blockstate::block_name(state);
    [
        "slime_block", "honey_block", "piston", "sticky_piston", "piston_head", "moving_piston", "observer",
        "redstone_block", "redstone_lamp", "target", "repeater", "comparator", "redstone_wire", "redstone_torch",
        "redstone_wall_torch", "lever", "dispenser", "dropper", "hopper", "note_block", "tnt",
    ]
    .iter()
    .any(|n| name == format!("minecraft:{n}"))
}

/// Door cells hold 9 identical full plain blocks; corridor is walkable.
pub(crate) fn closed_problems(r: &Region, spec: &DoorSpec) -> Vec<String> {
    let mut out = Vec::new();
    let first = r.get(spec.door_cells()[0]).unwrap_or("?").to_string();
    for p in spec.door_cells() {
        let s = r.get(p).unwrap_or("?");
        if s != first {
            out.push(format!("door cell {p:?} is {s}, but the door must be 9 identical blocks ({first})"));
        }
        if is_mechanism_block(s) {
            out.push(format!("door cell {p:?} is {s}, not a plain door block"));
        }
    }
    for p in spec.door_cells() {
        let s = r.get(p).unwrap_or("?");
        let full = r.flags_at(p).is_some_and(|f| f.full);
        if !full || is_moving(s) {
            out.push(format!("door cell {p:?} is {s}"));
        }
    }
    for p in spec.corridor_cells() {
        if r.flags_at(p).is_some_and(|f| f.collides) {
            out.push(format!("corridor cell {p:?} is blocked by {}", r.get(p).unwrap_or("?")));
        }
    }
    out
}

/// Door and corridor cells are all walkable.
pub(crate) fn open_problems(r: &Region, spec: &DoorSpec) -> Vec<String> {
    let mut out = Vec::new();
    for p in spec.door_cells().into_iter().chain(spec.corridor_cells()) {
        let s = r.get(p).unwrap_or("?");
        if r.flags_at(p).is_some_and(|f| f.collides) || is_moving(s) {
            out.push(format!("{p:?} is {s}"));
        }
    }
    out
}

/// First index from which `ok(frame)` holds for every remaining frame (1-based tick count).
pub(crate) fn settled_from(frames: &[Region], ok: impl Fn(&Region) -> bool) -> Option<u32> {
    let mut first = None;
    for (i, f) in frames.iter().enumerate() {
        if ok(f) {
            if first.is_none() {
                first = Some(i as u32 + 1);
            }
        } else {
            first = None;
        }
    }
    first
}

fn verify_at(
    w: &mut Worker,
    sb: u32,
    d: &Design,
    spec: &DoorSpec,
    opts: &VerifyOptions,
    report: &mut Report,
    first_position: bool,
) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    // Compute redstone wire connections (raw placement skips shape updates), let every block
    // re-check its inputs once (no observer pulses), then settle.
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let settle = w.step_until_idle(sb, opts.max_ticks)?;
    if settle.idle != Some(true) {
        bail!("the build never settles after placement ({} ticks)", settle.ticked);
    }
    let (wlo, whi) = watch_box(d, spec);
    let rest = w.get_region(sb, wlo, whi)?;
    for (p, s) in &blocks {
        if rest.get(*p) != Some(s.as_str()) && first_position {
            report.warnings.push(format!("not at rest as written: {p:?} {s} became {}", rest.get(*p).unwrap_or("?")));
        }
    }
    let problems = closed_problems(&rest, spec);
    if !problems.is_empty() {
        bail!("not closed at rest: {}", problems.join("; "));
    }
    let lever = rest.get(spec.lever).unwrap_or("?");
    if !lever.starts_with("minecraft:lever") || !lever.contains("powered=false") {
        bail!("port lever {:?} is {lever}, expected an unpowered lever", spec.lever);
    }
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| rest.get(*p).unwrap().to_string()).collect();

    for cycle in 0..opts.cycles {
        // Open.
        w.use_block(sb, spec.lever)?;
        let open = w.step_record(sb, opts.max_ticks, true, wlo, whi)?;
        if open.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after opening", open.ticked);
        }
        let last = open.frames.last().context("no frames")?;
        let problems = open_problems(last, spec);
        if !problems.is_empty() {
            bail!("cycle {cycle}: not open after the lever was switched on: {}", problems.join("; "));
        }
        let open_ticks = settled_from(&open.frames, |f| open_problems(f, spec).is_empty()).unwrap();
        if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while opening (something broke): {:?}", w.entities(sb)?);
        }
        // Close.
        w.use_block(sb, spec.lever)?;
        let close = w.step_record(sb, opts.max_ticks, true, wlo, whi)?;
        if close.idle != Some(true) {
            bail!("cycle {cycle}: still moving {} ticks after closing", close.ticked);
        }
        let last = close.frames.last().context("no frames")?;
        let door_now: Vec<String> = spec.door_cells().iter().map(|p| last.get(*p).unwrap().to_string()).collect();
        if door_now != closed_door {
            bail!(
                "cycle {cycle}: door not restored: {}",
                spec.door_cells()
                    .iter()
                    .zip(door_now.iter().zip(closed_door.iter()))
                    .filter(|(_, (a, b))| a != b)
                    .map(|(p, (a, b))| format!("{p:?} is {a}, was {b}"))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
        }
        if last.data != rest.data || last.palette.len() != rest.palette.len() {
            let diffs: Vec<String> = rest
                .iter()
                .filter(|(p, s)| last.get(*p) != Some(*s))
                .take(8)
                .map(|(p, s)| format!("{p:?} {s} -> {}", last.get(p).unwrap_or("?")))
                .collect();
            if !diffs.is_empty() {
                bail!("cycle {cycle}: the build did not return to its rest state: {}", diffs.join("; "));
            }
        }
        if !w.entities(sb)?.is_empty() {
            bail!("cycle {cycle}: entities appeared while closing (something broke)");
        }
        let close_ticks = settled_from(&close.frames, |f| {
            spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str()))
        })
        .unwrap();
        if first_position && cycle == 0 {
            report.cycles.push(CycleReport { open_ticks, open_settle: open.ticked, close_ticks, close_settle: close.ticked });
            if opts.keep_frames {
                report.open_frames = open.frames;
                report.close_frames = close.frames;
            }
        } else if first_position {
            report.cycles.push(CycleReport { open_ticks, open_settle: open.ticked, close_ticks, close_settle: close.ticked });
        }
    }
    Ok(())
}

/// One character per door cell for timelines.
pub fn cell_char(state: &str, collides: bool) -> char {
    if is_moving(state) {
        '~'
    } else if state.starts_with("minecraft:piston_head") {
        'h'
    } else if mctas::blockstate::is_air(state) {
        '.'
    } else if collides {
        '#'
    } else {
        ','
    }
}

/// ASCII timeline of the 3x3 opening, one line per tick: three rows top to bottom.
pub fn timeline(frames: &[Region], spec: &DoorSpec) -> String {
    let mut out = String::new();
    for (t, f) in frames.iter().enumerate() {
        let cells = spec.door_cells();
        let mut line = format!("  t={:<3}", t + 1);
        for row in 0..3 {
            line.push(' ');
            for col in 0..3 {
                let p = cells[row * 3 + col];
                let s = f.get(p).unwrap_or("?");
                line.push(cell_char(s, f.flags_at(p).is_some_and(|fl| fl.collides)));
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// The design with every block replaced by the exact state it has once placed and settled
/// (wire connections, repeater flags...), so the file is the true rest state.
pub fn canonicalize(w: &mut Worker, d: &Design) -> Result<Design> {
    // A design written with stale redstone states (dust at power 0, a repeater off) can let a
    // piston fire before its signal settles, so the "rest" read back is a half-moved machine.
    // Settle, copy back only the redstone parts' states, and retry until the machine stays put.
    let is_redstone = |s: &str| {
        let n = mctas::blockstate::block_name(s);
        ["redstone_wire", "repeater", "comparator", "redstone_torch", "redstone_wall_torch", "observer", "redstone_lamp"]
            .iter()
            .any(|k| n == format!("minecraft:{k}"))
    };
    let mut cur = d.clone();
    for _ in 0..6 {
        let settled = canonicalize_once(w, &cur)?;
        let moved = cur.blocks.iter().any(|(p, s)| !is_redstone(s) && settled.blocks.get(p) != Some(s));
        if !moved {
            return Ok(settled);
        }
        let mut next = d.clone();
        for (p, s) in next.blocks.iter_mut() {
            if is_redstone(s) {
                *s = settled.blocks[p].clone();
            }
        }
        if next == cur {
            return Ok(settled);
        }
        cur = next;
    }
    canonicalize_once(w, &cur)
}

fn canonicalize_once(w: &mut Worker, d: &Design) -> Result<Design> {
    let sb = w.create(&SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    let settle = w.step_until_idle(sb, 400)?;
    if settle.idle != Some(true) {
        bail!("design never settles");
    }
    let (lo, hi) = d.bounds().context("empty design")?;
    let r = w.get_region(sb, lo, hi)?;
    let _ = w.drop_sandbox(sb);
    let mut out = d.clone();
    for (p, s) in out.blocks.iter_mut() {
        *s = r.get(*p).context("outside region")?.to_string();
    }
    Ok(out)
}
