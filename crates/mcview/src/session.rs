//! A contraption running on redsim: placed exactly like its judge (the door judge's placement
//! for doors, the clock judge's placement and start kick for clocks), stepped one game tick at a
//! time, with the player's inputs (lever flips, button presses) applied between ticks.
//!
//! The clock logic mirrors `crates/clock/src/sim.rs` (`start`, `between_ticks`, the period
//! detector) and the door logic `crates/piston-door/src/cosearch.rs` (`place_and_settle`), so the
//! browser shows what the judges measured; `mctas web check` compares the two natively and on the
//! shipped wasm.

use crate::design::{Design, Pos, parse_travel};
use redsim::piston::MovingBe;
use redsim::world::flags;
use redsim::{Block, BlockPos, Dir, World};
use std::collections::HashMap;
use std::hash::Hasher;

/// The clock judge's one-time start kick (`port start: x y z <kick> [at N]`).
#[derive(Clone, Debug, PartialEq)]
pub enum Kick {
    Last { pos: Pos },
    Use { pos: Pos, again: Option<u32> },
    Place { pos: Pos, state: String, ticks: u32 },
}

/// One input applied between ticks.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Place { pos: Pos, block: Block, nbt: Option<String> },
    Use { pos: Pos },
    Clear { pos: Pos },
    /// Change the block with shape updates only (flags 2: the BUD judge).
    ShapeSet { pos: Pos, block: Block },
    /// A pure block update around the cell (`Level.updateNeighborsAt`: the BUD judge).
    Update { pos: Pos },
}

/// What kind of contraption a design is (from its ports).
#[derive(Clone, Debug, PartialEq)]
pub enum Mode {
    /// A 3x3 piston door: `port door` (bottom corner of the opening) and `port lever`.
    Door { door: Pos, lever: Pos, along_z: bool },
    /// Anything that runs by itself (clocks; plain designs): optional output and start kick.
    Clock { output: Option<Pos>, kick: Option<(Kick, u32)>, player_placement: bool },
}

/// Knobs of [`Session::new`].
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Free cells around the design in the simulated world (the clock judge uses 4).
    pub margin: i32,
    /// Ticks a door may take to settle after placement.
    pub settle: u32,
    /// A TNT duper's ground reach (None: the reach its judge would grow the ground to,
    /// [`Session::with_ground`]).
    pub ground_reach: Option<i32>,
}

impl Default for Options {
    fn default() -> Self {
        Options { margin: 4, settle: 400, ground_reach: None }
    }
}

/// A TNT duper's ground (`Circuit::ground_blocks`, the judge's: grass over two dirt over
/// bedrock), real blocks of the simulated world: `lo..=hi` (x, y, z), its top layer at `hi[1]`.
#[derive(Clone, Debug)]
pub struct Floor {
    pub lo: Pos,
    pub hi: Pos,
    /// Its layers, top first: the block of each and its name.
    pub layers: Vec<(Block, String)>,
    /// How far it reaches around the design's box, and that box (the design as written).
    pub reach: i32,
    pub design_box: (Pos, Pos),
    /// Its cells that no longer hold their layer's block (a crater, something fallen in) and what
    /// they hold now, sorted; rescanned after every tick ([`Session::step`]).
    pub changed: Vec<(Pos, Block)>,
}

impl Floor {
    /// The judge's ground layers (`Ground::Grass`), top first.
    pub const LAYERS: [&'static str; 4] = ["minecraft:grass_block", "minecraft:dirt", "minecraft:dirt", "minecraft:bedrock"];

    /// The ground `reach` blocks around the design box `lo..=hi`, its top `drop` air blocks under
    /// the design's lowest layer (`Circuit::ground_blocks`).
    pub fn grass(lo: Pos, hi: Pos, reach: i32, drop: i32) -> Result<Floor, String> {
        let layers = Floor::LAYERS.iter().map(|s| Block::parse(s).map(|b| (b, b.to_string())).map_err(|e| format!("ground {s}: {e}"))).collect::<Result<Vec<_>, _>>()?;
        let top = lo[1] - 1 - drop;
        let n = layers.len() as i32;
        Ok(Floor { lo: [lo[0] - reach, top - n + 1, lo[2] - reach], hi: [hi[0] + reach, top, hi[2] + reach], layers, reach, design_box: (lo, hi), changed: Vec::new() })
    }

    pub fn contains(&self, p: Pos) -> bool {
        (0..3).all(|i| p[i] >= self.lo[i] && p[i] <= self.hi[i])
    }

    /// The layer's block of a cell at height `y` (in the ground).
    pub fn layer(&self, y: i32) -> &(Block, String) {
        &self.layers[((self.hi[1] - y) as usize).min(self.layers.len() - 1)]
    }

    /// Every cell of the ground as laid (before anything runs).
    pub fn blocks(&self) -> Vec<(BlockPos, Block)> {
        let mut v = Vec::new();
        for y in self.lo[1]..=self.hi[1] {
            let b = self.layer(y).0;
            for x in self.lo[0]..=self.hi[0] {
                for z in self.lo[2]..=self.hi[2] {
                    v.push((BlockPos::new(x, y, z), b));
                }
            }
        }
        v
    }

    /// What a cell of the ground holds now (its layer's block unless it changed).
    pub fn now(&self, p: Pos) -> Block {
        match self.changed.binary_search_by_key(&p, |(q, _)| *q) {
            Ok(i) => self.changed[i].1,
            Err(_) => self.layer(p[1]).0,
        }
    }

    /// Whether a cell of the ground changed (a crater, something fallen in).
    pub fn is_changed(&self, p: Pos) -> bool {
        self.changed.binary_search_by_key(&p, |(q, _)| *q).is_ok()
    }

    /// The state drawn at a cell of the ground (None: air).
    pub fn shown(&self, p: Pos) -> Option<String> {
        let b = self.now(p);
        if b == Block::Air {
            None
        } else if b == self.layer(p[1]).0 {
            Some(self.layer(p[1]).1.clone())
        } else {
            Some(b.to_string())
        }
    }
}

/// The reach a TNT duper's judge lays its ground at first, the widest it grows it to (it doubles
/// the reach while a burning TNT leaves the ground: `circuit::run::{GROUND_REACH,
/// MAX_GROUND_REACH}`), and the ticks the viewer runs to choose it.
pub const GROUND_REACH: i32 = 24;
pub const MAX_GROUND_REACH: i32 = 96;
pub const GROUND_PROBE: u32 = 600;

/// A TNT duper with its explosions (`Circuit::explodes`): it stands on its judge's ground.
pub fn ground_duper(d: &Design) -> bool {
    let kind = d.meta.get("circuit").and_then(|c| c.split_whitespace().next().map(str::to_string)).unwrap_or_default();
    (kind == "tnt-duper" || kind == "tnt-duper-triggered") && d.meta.get("explode").is_none_or(|v| v.trim() != "off")
}

/// The safe drop an archived TNT duper's judge found (`# CIRCUIT: ... safe drop N blocks`: the
/// air blocks between its lowest layer and the ground); 0 (on the ground) when not written.
pub fn safe_drop(text: &str) -> i32 {
    text.lines()
        .filter(|l| l.starts_with("# CIRCUIT:"))
        .find_map(|l| l.split("safe drop ").nth(1).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse::<i32>().ok()))
        .unwrap_or(0)
        .max(0)
}

pub struct Session {
    pub design: Design,
    pub mode: Mode,
    pub world: World,
    /// Ticks run since placement (a door's settling ticks included).
    pub tick: u32,
    /// The tick the door came to rest after placement (0 for clocks).
    pub rest_tick: u32,
    /// The last input's tick (a lever flip), if any.
    pub last_input: Option<u32>,
    actions: Vec<(u32, Action)>,
    /// Why the design is shown but not simulated (a block or entity redsim does not model).
    pub unsimulated: Option<String>,
    /// Plain blocks simulated as stone (redsim models one stone for every quartz-like block):
    /// their own names, for the renderer, while the cell still holds that stone.
    pub overlay: HashMap<Pos, String>,
    /// Cells a click toggles (levers, buttons, the input ports).
    pub inputs: Vec<Pos>,
    /// Output ports (the orange marker).
    pub outputs: Vec<Pos>,
    /// Everything as placed, for [`Session::reset`].
    initial: Option<(World, Vec<(u32, Action)>, u32)>,
    /// A real-game recording played instead of simulating ([`Session::attach_trace`]).
    pub replay: Option<crate::replay::Replay>,
    /// The box a travelling design runs in (a flying machine: `circuit fly sim`'s corridor).
    pub corridor: Option<(Pos, Pos)>,
    /// A flying machine: the tick its judge stops at (it reaches the corridor's end, comes to
    /// rest or breaks); the viewer loops there.
    pub end_tick: Option<u32>,
    /// A circuit's inputs (`port input[.<name>]`), driven like its judge drives them.
    pub circuit_inputs: Vec<crate::circuit::CircuitInput>,
    /// Each circuit input's level now.
    pub input_on: Vec<bool>,
    /// The judge's stimulus being replayed (the auto-demo), from placement.
    pub demo: Option<crate::circuit::Schedule>,
    /// Pending input releases (tick, input): a tapped pulse input goes off there.
    releases: Vec<(u32, usize)>,
    /// A clock's picture box (`clock shot`'s frame: the judge's swept box), once known.
    pub picture: Option<(Pos, Pos)>,
    /// The judge's consumer lamps placed in a circuit's air output cells (they show the output).
    pub consumers: Vec<Pos>,
    /// The clock judge's lamp in a clock's empty output cell (`clock::sim::judge_lamp`): the OUT
    /// signal there is the power delivered into the cell, not the lamp's own lit state.
    pub judge_lamp: Option<Pos>,
    /// Where each falling block (by entity id) was before the tick just run: drawn between there
    /// and where it is (redsim keeps no previous position for them).
    prev_falling: Vec<(u32, [f64; 3])>,
    /// redsim met something it does not model during the run (`World::errors`): the view stops
    /// at the last exact state and says so (never animates past it). Cleared by a reset.
    pub halted: Option<String>,
    /// A shulker loader's or unloader's judge chests (redsim's driver: kept full, emptied), and
    /// their state as placed (for [`Session::reset`]).
    pub shulker: Option<redsim::redstone::shulker::Chests>,
    shulker_initial: Option<redsim::redstone::shulker::Chests>,
    /// The judge's visit is due before the next tick (it follows each tick's frame).
    visit_due: bool,
    /// A TNT duper's ground (its judge's: real blocks of the world, drawn from the world); None:
    /// no ground (nothing solid is drawn where redsim has void).
    pub floor: Option<Floor>,
    /// With a ground: the part of the world the machine lives in (the design's box and margin),
    /// where the blocks are drawn, hashed and framed from (the ground is drawn on its own); None:
    /// the whole world box.
    pub machine_box: Option<(BlockPos, [i32; 3])>,
}

/// The corridor of a design with `travel:` (`circuit_fly::machine::Machine::corridor`): its
/// blocks, kick cell and `stop:` blocks swept `distance:` cells (32 by default) along the travel,
/// plus 3 free cells; a piston extender sweeps its N (`circuit: piston-extender N`) + 1.
pub fn corridor_of(d: &Design, mode: &Mode) -> Option<(Pos, Pos)> {
    let ((lo, hi), margin) = travel_box(d, mode)?;
    Some(([lo[0] - margin, lo[1] - margin, lo[2] - margin], [hi[0] + margin, hi[1] + margin, hi[2] + margin]))
}

/// A travelling design's swept box (where it can be, without the free cells) and the corridor's
/// free margin.
pub fn travel_box(d: &Design, mode: &Mode) -> Option<((Pos, Pos), i32)> {
    let v = parse_travel(d.meta.get("travel")?)?;
    let kind = d.meta.get("circuit").map(|s| s.as_str()).unwrap_or("flying-machine");
    let (mut lo, mut hi) = d.bounds()?;
    if let Mode::Clock { kick: Some((k, _)), .. } = mode {
        let p = match k {
            Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
        };
        grow(&mut lo, &mut hi, p);
    }
    for (p, _) in &d.stops {
        grow(&mut lo, &mut hi, *p);
    }
    let (k, margin) = if kind.starts_with("flying-machine") {
        (d.meta.get("distance").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(32), 3)
    } else {
        let n = kind.split_whitespace().nth(1).and_then(|w| w.parse::<i32>().ok()).unwrap_or(1);
        (d.meta.get("distance").and_then(|s| s.trim().parse::<i32>().ok()).unwrap_or(n + 1), 4)
    };
    for i in 0..3 {
        if v[i] > 0 {
            hi[i] += k;
        } else if v[i] < 0 {
            lo[i] -= k;
        }
    }
    Some(((lo, hi), margin))
}

/// Ticks a shulker design is run ahead on load: a run that meets something redsim does not
/// model by then is shown as built.
pub const SHULKER_PROBE: u32 = 1000;

pub fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A design block as redsim state; quartz-like plain blocks (`crates/piston-door/src/properties.rs`)
/// fall back to stone like the door tools do.
fn convert(state: &str) -> Result<(Block, bool), String> {
    match Block::parse(state) {
        Ok(b) => Ok((b, false)),
        Err(e) => {
            if quartz_like(state) {
                Ok((Block::Simple(redsim::block::Simple::Stone), true))
            } else {
                Err(e)
            }
        }
    }
}

/// `piston-door/src/properties.rs::quartz_like`: plain full blocks the door tools treat as stone.
pub fn quartz_like(state: &str) -> bool {
    let n = state.split('[').next().unwrap_or(state);
    let n = n.strip_prefix("minecraft:").unwrap_or(n);
    if n.contains("glazed") || n.ends_with("concrete_powder") || n.contains("slab") || n.contains("stairs") || n.contains("wall") {
        return false;
    }
    matches!(
        n,
        "stone" | "smooth_stone" | "cobblestone" | "mossy_cobblestone" | "stone_bricks" | "mossy_stone_bricks" | "cracked_stone_bricks"
            | "chiseled_stone_bricks" | "granite" | "polished_granite" | "diorite" | "polished_diorite" | "andesite" | "polished_andesite"
            | "deepslate" | "cobbled_deepslate" | "polished_deepslate" | "deepslate_bricks" | "deepslate_tiles" | "tuff" | "calcite"
            | "dirt" | "coarse_dirt" | "quartz_block" | "smooth_quartz" | "quartz_bricks" | "chiseled_quartz_block" | "quartz_pillar"
            | "sandstone" | "smooth_sandstone" | "cut_sandstone" | "red_sandstone" | "bricks" | "terracotta" | "netherrack" | "end_stone"
            | "end_stone_bricks" | "blackstone" | "polished_blackstone" | "polished_blackstone_bricks" | "prismarine" | "dark_prismarine"
            | "purpur_block" | "mud_bricks" | "packed_mud" | "bone_block" | "nether_bricks" | "red_nether_bricks" | "basalt" | "smooth_basalt"
            | "polished_basalt" | "amethyst_block" | "lapis_block" | "coal_block" | "iron_block" | "gold_block" | "diamond_block" | "emerald_block"
    ) || n.ends_with("_planks")
        || n.ends_with("_wool")
        || n.ends_with("_concrete")
        || n.ends_with("_terracotta")
        || n.ends_with("_log")
        || n.ends_with("_wood")
        || (n.starts_with("waxed_") && (n.ends_with("copper") || n.ends_with("cut_copper") || n.ends_with("copper_block")))
}

fn parse_kick(pos: Pos, args: &str) -> Result<(Kick, u32), String> {
    let mut words: Vec<&str> = args.split_whitespace().collect();
    let mut at = 0;
    if words.len() >= 2 && words[words.len() - 2] == "at" {
        let n = words[words.len() - 1];
        at = n.parse().map_err(|_| format!("bad kick tick {n:?}"))?;
        words.truncate(words.len() - 2);
    }
    let k = match words.as_slice() {
        ["last"] => Kick::Last { pos },
        ["use"] => Kick::Use { pos, again: None },
        ["use", t] => Kick::Use { pos, again: Some(t.parse().map_err(|_| format!("bad tick count {t:?}"))?) },
        [state, t] => {
            let ticks: u32 = t.parse().map_err(|_| format!("bad tick count {t:?}"))?;
            if ticks == 0 {
                return Err("a temporary start block stays at least 1 gt".into());
            }
            Kick::Place { pos, state: crate::design::normalize_state(state)?, ticks }
        }
        _ => return Err(format!("port start: x y z (last | use [T] | <block state> <T>) [at N], got {args:?}")),
    };
    Ok((k, at))
}

impl Mode {
    pub fn of(d: &Design) -> Result<Mode, String> {
        if let (Some(door), Some(lever)) = (d.port("door"), d.port("lever")) {
            let along_z = d.meta.get("door-axis").is_some_and(|v| v.trim() == "z");
            return Ok(Mode::Door { door, lever, along_z });
        }
        let kick = match d.ports.get("start") {
            Some((p, args)) if !args.is_empty() => Some(parse_kick(*p, args)?),
            Some(_) => return Err("port start needs a kick after its position: last | use [T] | <block state> <T>".into()),
            None => None,
        };
        let player_placement = match d.meta.get("placement").map(|s| s.as_str()) {
            None | Some("judge") => false,
            Some("player") => true,
            Some(v) => return Err(format!("placement: judge | player, got {v:?}")),
        };
        Ok(Mode::Clock { output: d.port("output"), kick, player_placement })
    }

    pub fn is_door(&self) -> bool {
        matches!(self, Mode::Door { .. })
    }
}

/// The 9 cells of a door's opening.
pub fn door_cells(door: Pos, along_z: bool) -> Vec<Pos> {
    let mut out = Vec::new();
    for dy in (0..3).rev() {
        for dw in 0..3 {
            out.push(if along_z { [door[0], door[1] + dy, door[2] + dw] } else { [door[0] + dw, door[1] + dy, door[2]] });
        }
    }
    out
}

fn grow(lo: &mut Pos, hi: &mut Pos, p: Pos) {
    for i in 0..3 {
        lo[i] = lo[i].min(p[i]);
        hi[i] = hi[i].max(p[i]);
    }
}

impl Session {
    /// Parse, place like the judge, and (doors) settle. A design redsim cannot simulate still
    /// loads: [`Session::unsimulated`] says why and the world holds what it could place.
    pub fn new(text: &str, o: &Options) -> Result<Session, String> {
        let mut design = Design::parse(text)?;
        if design.blocks.is_empty() && design.entities.is_empty() {
            return Err("empty design".into());
        }
        if o.ground_reach.is_none() && ground_duper(&design) {
            return Session::with_ground(text, o);
        }
        // the design's box as written (a TNT duper's ground is laid around it)
        let written = design.blocks.keys().fold(None, |acc: Option<(Pos, Pos)>, p| match acc {
            None => Some((*p, *p)),
            Some((mut lo, mut hi)) => {
                grow(&mut lo, &mut hi, *p);
                Some((lo, hi))
            }
        });
        // a circuit's inputs as its judge reads them, on the design as written (before its
        // consumers: the drivers' directions depend on the build's own box)
        let circuit_inputs = crate::circuit::inputs(&design);
        // a circuit runs as its judge builds it: a lamp (the consumer) in every air output cell
        let consumers = crate::circuit::consumer_cells(&design);
        for p in &consumers {
            design.blocks.insert(*p, crate::circuit::CONSUMER.to_string());
        }
        // and every driven input's stone, repeater and support (R7, `Circuit::with_drivers`; the
        // switch comes and goes with the input edges)
        for ci in &circuit_inputs {
            if let crate::circuit::InputHow::Driven { dir, .. } = ci.how {
                let (r, sup, _) = crate::circuit::driver_cells(ci.pos, dir);
                design.blocks.entry(ci.pos).or_insert_with(|| crate::circuit::DRIVEN_STONE.to_string());
                design.blocks.insert(r, crate::circuit::driver_repeater(dir));
                design.blocks.insert(sup, crate::circuit::DRIVEN_STONE.to_string());
            }
        }
        // a shulker loader or unloader: the judge's chests on its ports, run by redsim's driver
        // once the design stands (`redstone::shulker::Chests`, below); their cells are in the box
        let shulker = crate::circuit::is_shulker(&design);
        let chest_cells = crate::circuit::chest_cells(&design);
        let mode = Mode::of(&design)?;
        let (mut lo, mut hi) = design.bounds().ok_or("empty design")?;
        for c in &chest_cells {
            grow(&mut lo, &mut hi, *c);
        }
        let mut outputs = Vec::new();
        let mut inputs = Vec::new();
        for (name, (p, _)) in &design.ports {
            if is_port_named(name, "out") {
                outputs.push(*p);
            }
            if name == "lever" || is_port_named(name, "in") {
                inputs.push(*p);
            }
        }
        for ci in &circuit_inputs {
            if !inputs.contains(&ci.pos) {
                inputs.push(ci.pos);
            }
        }
        match &mode {
            Mode::Door { door, along_z, .. } => {
                for c in door_cells(*door, *along_z) {
                    grow(&mut lo, &mut hi, c);
                }
                // the walkway in front of and behind the opening (the judge's watch box)
                let (a, b) = (door_cells(*door, *along_z)[0], door_cells(*door, *along_z)[8]);
                let depth = if *along_z { [3, 0, 0] } else { [0, 0, 3] };
                grow(&mut lo, &mut hi, [a[0] - depth[0], a[1], a[2] - depth[2]]);
                grow(&mut lo, &mut hi, [b[0] + depth[0], b[1], b[2] + depth[2]]);
                lo = [lo[0] - 2, lo[1] - 2, lo[2] - 2];
                hi = [hi[0] + 2, hi[1] + 2, hi[2] + 2];
            }
            Mode::Clock { kick: Some((k, _)), .. } => {
                let p = match k {
                    Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
                };
                grow(&mut lo, &mut hi, p);
            }
            Mode::Clock { .. } => {}
        }
        for (p, s) in &design.blocks {
            let id = s.split('[').next().unwrap_or("");
            if matches!(id, "minecraft:lever" | "minecraft:stone_button" | "minecraft:oak_button") && !inputs.contains(p) {
                inputs.push(*p);
            }
        }
        let m = o.margin;
        let mut min = BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m);
        let mut size = [hi[0] - lo[0] + 1 + 2 * m, hi[1] - lo[1] + 1 + 2 * m, hi[2] - lo[2] + 1 + 2 * m];
        // a design that travels (a flying machine, a piston extender's payload): the world is the
        // judge's corridor, never the design's box (blocks leaving it would be cut off)
        let corridor = corridor_of(&design, &mode);
        if let Some((clo, chi)) = corridor {
            min = BlockPos::new(clo[0], clo[1], clo[2]);
            size = [chi[0] - clo[0] + 1, chi[1] - clo[1] + 1, chi[2] - clo[2] + 1];
        }
        // a TNT duper with its explosions stands on its judge's ground (coordinator 2026-10-20,
        // after vjeux 17:53Z "Please don't put a fake ground if the tnt goes through it"): real
        // blocks of the simulated world laid like the judge's strict run (`circuit::run::sim_tnt`:
        // `Circuit::ground_blocks`, `reach` around the design as written, its top the archived
        // safe drop under it) in the judge's world box around it; the machine keeps its own box
        let mut floor = None;
        let mut machine_box = None;
        if let (Some(r), true, Some((wlo, whi))) = (o.ground_reach, ground_duper(&design), written) {
            let drop = safe_drop(text);
            floor = Some(Floor::grass(wlo, whi, r, drop)?);
            let from = [wlo[0] - r - 2, wlo[1] - 6 - drop, wlo[2] - r - 2];
            let to = [whi[0] + r + 2, whi[1] + 16, whi[2] + r + 2];
            // the machine's box, within the world's
            let mlo = [min.x.max(from[0]), min.y.max(from[1]), min.z.max(from[2])];
            let mhi = [(min.x + size[0] - 1).min(to[0]), (min.y + size[1] - 1).min(to[1]), (min.z + size[2] - 1).min(to[2])];
            machine_box = Some((BlockPos::new(mlo[0], mlo[1], mlo[2]), [mhi[0] - mlo[0] + 1, mhi[1] - mlo[1] + 1, mhi[2] - mlo[2] + 1]));
            min = BlockPos::new(from[0], from[1], from[2]);
            size = [to[0] - from[0] + 1, to[1] - from[1] + 1, to[2] - from[2] + 1];
        }
        let mut world = World::new(min, size);
        let mut overlay = HashMap::new();
        let mut unsimulated: Vec<String> = Vec::new();
        let mut blocks: Vec<(BlockPos, Block)> = Vec::new();
        for (p, s) in &design.blocks {
            match convert(s) {
                Ok((b, stone)) => {
                    if stone {
                        overlay.insert(*p, s.clone());
                    }
                    blocks.push((bp(*p), b));
                }
                Err(e) => {
                    if !unsimulated.contains(&e) {
                        unsimulated.push(e);
                    }
                }
            }
        }
        // the clock judge's lamp (`clock::sim::judge_lamp`, vjeux 2026-10-20): in a clock's
        // declared output cell when the design leaves it empty, placed after the design's blocks
        // (what `clock::sim::parse_blocks` does); not a block of the design. Clocks only: a
        // design with a `circuit:` kind or an input port keeps its judge's consumers.
        let judge_lamp = design.ports.get("output").map(|(p, _)| *p).filter(|p| !design.blocks.contains_key(p) && !design.meta.contains_key("circuit") && !design.ports.keys().any(|k| k.starts_with("input")));
        if let Some(op) = judge_lamp
            && let Ok(b) = Block::parse(JUDGE_LAMP)
        {
            blocks.push((bp(op), b));
        }
        // shown as built (below): a shulker design's chests (the judge places them) keep it
        // readable; nothing the judge does not place is added
        let as_built = |blocks: &[(BlockPos, Block)]| -> Vec<(BlockPos, Block)> {
            let mut v = blocks.to_vec();
            if shulker {
                let chest = Block::Chest { facing: Dir::North, waterlogged: false };
                v.extend(chest_cells.iter().map(|p| (bp(*p), chest)));
            }
            v
        };
        let mut s = Session {
            design,
            mode,
            world: World::new(min, [1, 1, 1]),
            tick: 0,
            rest_tick: 0,
            last_input: None,
            actions: Vec::new(),
            unsimulated: None,
            overlay,
            inputs,
            outputs,
            initial: None,
            replay: None,
            corridor,
            end_tick: None,
            input_on: vec![false; circuit_inputs.len()],
            circuit_inputs,
            demo: None,
            releases: Vec::new(),
            picture: None,
            consumers,
            judge_lamp,
            prev_falling: Vec::new(),
            halted: None,
            shulker: None,
            shulker_initial: None,
            visit_due: false,
            floor,
            machine_box,
        };
        if !unsimulated.is_empty() {
            // Shown as built: every block the model knows placed raw, nothing ticks (and nothing
            // to drive: no inputs). No ground: nothing falls on it.
            world.place_raw(&as_built(&blocks));
            s.world = world;
            s.unsimulated = Some(unsimulated.join("; "));
            s.circuit_inputs.clear();
            s.input_on.clear();
            s.floor = None;
            return Ok(s);
        }
        if let Err(e) = s.place(&mut world, &blocks, o) {
            // e.g. container contents redsim does not model: shown as built, not simulated
            let mut w = World::new(min, size);
            w.place_raw(&as_built(&blocks));
            s.world = w;
            s.unsimulated = Some(e);
            s.circuit_inputs.clear();
            s.input_on.clear();
            s.floor = None;
            return Ok(s);
        }
        // a shulker loader or unloader runs like its judge: the chests placed raw after the
        // design, the settle, the boxes and the items (the start lever left to the visitor; the
        // probe below runs with it on); then the judge's visit before every tick (`step`)
        if let Some(mut ch) = crate::circuit::shulker_chests(&s.design) {
            // the judge's sandbox (clk-misc): the level random mirrored and the drops seeded from
            // the seed the judge builds this design with (its watch box), so a dropper choosing
            // among stacks and a hopper choosing among minecarts draw what the game draws
            if let Some((from, to)) = crate::circuit::judge_box(&s.design) {
                redsim::redstone::shulker::judge_sandbox(&mut world, from, to);
            }
            ch.place(&mut world);
            for _ in 0..redsim::redstone::shulker::SETTLE {
                world.tick();
            }
            ch.start(&mut world);
            // (the start lever stays off: the visitor turns it on, as the judge does at its start)
            // a run redsim cannot follow (something it does not model; the random choices are
            // drawn from the mirrored level random above): shown as built, chests and all
            let (mut pw, mut pc) = (world.clone(), ch.clone());
            for ci in &s.circuit_inputs {
                if ci.how == crate::circuit::InputHow::Lever && !(ci.name == "mode" || ci.name.starts_with("mode.")) {
                    pw.use_block(bp(ci.pos));
                }
            }
            for t in 0..SHULKER_PROBE {
                if t > 0 {
                    pc.visit(&mut pw);
                }
                pw.tick();
                if let Some(e) = pw.errors.first() {
                    s.world = world;
                    s.unsimulated = Some(format!("not modeled by redsim (tick {t} of its run): {e}"));
                    s.circuit_inputs.clear();
                    s.input_on.clear();
                    return Ok(s);
                }
            }
            s.tick += redsim::redstone::shulker::SETTLE;
            s.rest_tick = s.tick;
            s.shulker_initial = Some(ch.clone());
            s.shulker = Some(ch);
        }
        // a design with TNT (clk-misc): its TNT lives and explodes like in a bridge sandbox
        // (redsim's TNT entities, the level random mirrored and the drops seeded from seed 0 with
        // the world box's chunks, like the clock traces): `explosion::explode_on`
        // a TNT duper's ground, raw, once the design stands (`clock::sim::drive_tnt`'s order)
        if let Some(f) = &s.floor {
            world.place_raw(&f.blocks());
        }
        if is_tnt_design(&s.design) {
            let to = [min.x + size[0] - 1, min.y + size[1] - 1, min.z + size[2] - 1];
            redsim::explosion::explode_on(&mut world, 0, redsim::redstone::shulker::chunk_range([min.x, min.y, min.z], to));
        }
        s.world = world;
        s.initial = Some((s.world.clone(), s.actions.clone(), s.tick));
        // a session at tick t has the inputs due at t applied (the judge's between-ticks order)
        s.apply_due();
        s.rescan_floor();
        if s.corridor.is_some() && s.design.meta.get("circuit").is_some_and(|k| k.starts_with("flying-machine")) {
            s.end_tick = Some(s.fly_end());
        }
        Ok(s)
    }

    /// A TNT duper on its judge's ground at the reach its judge grows the ground to: 24, doubled
    /// while a burning TNT leaves it within the first [`GROUND_PROBE`] ticks (the judge's stimulus
    /// replayed for a triggered or switchable duper), at most 96; cached per design text.
    fn with_ground(text: &str, o: &Options) -> Result<Session, String> {
        thread_local! {
            static REACH: std::cell::RefCell<HashMap<u64, i32>> = std::cell::RefCell::new(HashMap::new());
        }
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut h);
            h.finish()
        };
        if let Some(r) = REACH.with(|m| m.borrow().get(&key).copied()) {
            return Session::new(text, &Options { ground_reach: Some(r), ..*o });
        }
        let mut r = GROUND_REACH;
        loop {
            let mut s = Session::new(text, &Options { ground_reach: Some(r), ..*o })?;
            if r >= MAX_GROUND_REACH || s.floor.is_none() || !s.tnt_leaves_ground(GROUND_PROBE) {
                s.reset();
                REACH.with(|m| m.borrow_mut().insert(key, r));
                return Ok(s);
            }
            r *= 2;
        }
    }

    /// Whether a burning TNT goes past the ground's reach within `ticks` (`TntOptions::escape`:
    /// more than `reach - 1` blocks sideways from the design's box), the judge's stimulus replayed
    /// (`circuit::tnt::stimulus`: a triggered duper's 4 gt pulses 100 gt apart, a switchable one
    /// off then on 400 gt each). The session is left where the probe stopped.
    fn tnt_leaves_ground(&mut self, ticks: u32) -> bool {
        let Some(((lo, hi), reach)) = self.floor.as_ref().map(|f| (f.design_box, f.reach as f64 - 1.0)) else { return false };
        let triggered = self.design.meta.get("circuit").is_some_and(|c| c.split_whitespace().next() == Some("tnt-duper-triggered"));
        let t0 = self.free_from() + 20;
        let end = self.tick + ticks;
        while self.tick < end && self.halted.is_none() {
            if !self.circuit_inputs.is_empty() && self.tick >= t0 {
                let k = self.tick - t0;
                let on = if triggered { k % 104 < 4 } else { (k / 400) % 2 == 1 };
                if self.input_on.first() != Some(&on) {
                    self.set_input(0, on);
                }
            }
            self.step();
            for e in self.world.entities.list.iter().filter(|e| !e.removed) {
                if matches!(e.kind, redsim::entity::EntityKind::PrimedTnt(_)) {
                    let p = [e.pos.x, e.pos.y, e.pos.z];
                    let side = |i: usize| (lo[i] as f64 - p[i]).max(p[i] - (hi[i] as f64 + 1.0)).max(0.0);
                    if side(0).max(side(2)) > reach {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Look over the ground for the cells that no longer hold their layer's block.
    fn rescan_floor(&mut self) {
        let w = &self.world;
        let Some(f) = self.floor.as_mut() else { return };
        f.changed.clear();
        for x in f.lo[0]..=f.hi[0] {
            for y in f.lo[1]..=f.hi[1] {
                let layer = f.layer(y).0;
                for z in f.lo[2]..=f.hi[2] {
                    let b = w.get(BlockPos::new(x, y, z));
                    if b != layer {
                        f.changed.push(([x, y, z], b));
                    }
                }
            }
        }
    }

    /// The part of the world the machine's blocks are drawn and hashed from: its box, the ground
    /// left out.
    pub fn machine(&self) -> (BlockPos, [i32; 3]) {
        self.machine_box.unwrap_or((self.world.min, self.world.size))
    }

    fn in_floor(&self, p: Pos) -> bool {
        self.floor.as_ref().is_some_and(|f| f.contains(p))
    }

    /// The machine's non-air cells (the ground left out).
    pub fn machine_blocks(&self) -> Vec<(BlockPos, Block)> {
        let (min, size) = self.machine();
        let mut v = Vec::new();
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let q = min.offset(x, y, z);
                    let b = self.world.get(q);
                    if b != Block::Air && !self.in_floor(q.arr()) {
                        v.push((q, b));
                    }
                }
            }
        }
        v
    }

    /// The ground as drawn: each of its non-air cells and the state shown there (its layer's
    /// block, or what a changed cell holds now).
    pub fn floor_drawn(&self) -> Vec<(Pos, String)> {
        let Some(f) = &self.floor else { return Vec::new() };
        let mut v = Vec::new();
        for x in f.lo[0]..=f.hi[0] {
            for y in f.lo[1]..=f.hi[1] {
                for z in f.lo[2]..=f.hi[2] {
                    if let Some(s) = f.shown([x, y, z]) {
                        v.push(([x, y, z], s));
                    }
                }
            }
        }
        v
    }

    fn place(&mut self, w: &mut World, blocks: &[(BlockPos, Block)], o: &Options) -> Result<(), String> {
        let bes = self.design.block_entities.clone();
        let nbt = |w: &mut World, only: Option<Pos>, skip: Option<Pos>| -> Result<(), String> {
            let lines: Vec<(BlockPos, &str)> = bes.iter().filter(|(p, _)| only.is_none_or(|q| q == **p) && skip != Some(**p)).map(|(p, s)| (bp(*p), s.as_str())).collect();
            redsim::redstone::container::apply_block_entities(w, lines).map_err(|e| format!("nbt: {e}"))
        };
        match self.mode.clone() {
            Mode::Door { .. } => {
                w.place_raw(blocks);
                nbt(w, None, None)?;
                w.place_updates(blocks);
                // `cosearch::place_and_settle`: tick until idle (at least once)
                let mut n = 0;
                while n < o.settle {
                    w.tick();
                    n += 1;
                    if w.idle() {
                        break;
                    }
                }
                self.tick = n;
                self.rest_tick = n;
            }
            Mode::Clock { kick, player_placement, .. } => {
                let last = match &kick {
                    Some((Kick::Last { pos }, _)) => Some(*pos),
                    _ => None,
                };
                // `stop:` blocks first, placed and updated on their own (the environment stands
                // before the build: `circuit fly sim`'s order)
                let mut stops: Vec<(BlockPos, Block)> = Vec::new();
                for (p, s) in &self.design.stops {
                    stops.push((bp(*p), convert(s)?.0));
                }
                if !stops.is_empty() {
                    w.place_raw(&stops);
                    w.place_updates(&stops);
                }
                let silent: Vec<(BlockPos, Block)> = blocks.iter().filter(|(p, _)| Some(p.arr()) != last).copied().collect();
                if player_placement {
                    for (p, b) in &silent {
                        w.set_block(*p, *b, flags::ALL);
                    }
                    nbt(w, None, last)?;
                } else {
                    w.place_raw(&silent);
                    nbt(w, None, last)?;
                    w.place_updates(&silent);
                }
                // the design's entities, once the blocks stand (as the judge summons them)
                for e in self.design.entities.clone() {
                    redsim::entity::spawn(w, &e.kind, e.pos, e.nbt.as_deref()).map_err(|why| format!("not modeled by redsim: {why}"))?;
                }
                if let Some((k, at)) = kick {
                    self.actions = match k {
                        Kick::Last { pos } => {
                            let b = blocks.iter().find(|(p, _)| p.arr() == pos).map(|(_, b)| *b).ok_or("port start: no design block there to place last")?;
                            vec![(at, Action::Place { pos, block: b, nbt: self.design.block_entities.get(&pos).cloned() })]
                        }
                        Kick::Use { pos, again } => {
                            let mut v = vec![(at, Action::Use { pos })];
                            if let Some(t) = again {
                                v.push((at + t, Action::Use { pos }));
                            }
                            v
                        }
                        Kick::Place { pos, state, ticks } => {
                            let b = convert(&state)?.0;
                            vec![(at, Action::Place { pos, block: b, nbt: None }), (at + ticks, Action::Clear { pos })]
                        }
                    };
                }
            }
        }
        Ok(())
    }

    /// Back to the placed state.
    pub fn reset(&mut self) {
        if let Some(r) = self.replay.as_mut() {
            r.reset();
            self.tick = r.tick;
            return;
        }
        if let Some((w, a, t)) = &self.initial {
            self.world = w.clone();
            self.actions = a.clone();
            self.tick = *t;
            self.last_input = None;
            self.input_on = vec![false; self.circuit_inputs.len()];
            self.releases.clear();
            self.prev_falling.clear();
            self.halted = None;
            self.shulker = self.shulker_initial.clone();
            self.visit_due = false;
            self.apply_due();
            self.inputs_due();
            self.rescan_floor();
        }
    }

    pub fn simulated(&self) -> bool {
        self.unsimulated.is_none()
    }

    /// Something moves when played: a redsim simulation or a recording.
    pub fn playable(&self) -> bool {
        self.simulated() || self.replay.is_some()
    }

    /// Is the (first) output on now? (None: no output, or nothing simulated or recorded.)
    pub fn output_on(&self) -> Option<bool> {
        let o = *self.outputs.first()?;
        if let Some(r) = &self.replay {
            return Some(r.state_at(o).is_some_and(active_state));
        }
        if !self.simulated() {
            return None;
        }
        Some(active(&self.world.get(bp(o))))
    }

    /// Play a real-game recording (`designs/clocks/traces/<stem>.trace`) instead of simulating:
    /// for the designs redsim does not model (boats, minecarts, fluids).
    pub fn attach_trace(&mut self, text: &str) -> Result<(), String> {
        let r = crate::replay::Replay::parse(text)?;
        if self.outputs.is_empty()
            && let Some(o) = r.output
        {
            self.outputs.push(o);
        }
        self.tick = r.tick;
        self.rest_tick = 0;
        self.replay = Some(r);
        Ok(())
    }

    /// Apply the scheduled inputs due now (between ticks).
    fn apply_due(&mut self) {
        while self.actions.first().is_some_and(|(at, _)| *at == self.tick) {
            let (_, a) = self.actions.remove(0);
            match a {
                Action::Place { pos, block, nbt } => {
                    self.world.set_block(bp(pos), block, flags::ALL);
                    if let Some(n) = nbt {
                        let _ = redsim::redstone::container::apply_block_entities(&mut self.world, [(bp(pos), n.as_str())]);
                    }
                }
                Action::Use { pos } => {
                    self.world.use_block(bp(pos));
                }
                Action::Clear { pos } => {
                    self.world.set_block(bp(pos), Block::Air, flags::ALL);
                }
                Action::ShapeSet { pos, block } => {
                    self.world.set_block(bp(pos), block, flags::CLIENTS);
                }
                Action::Update { pos } => {
                    let src = self.world.get(bp(pos)).kind();
                    self.world.update_neighbors_at(bp(pos), src);
                }
            }
        }
    }

    /// One game tick, then the scheduled inputs due at the new tick (so the state shown at tick t
    /// is what the judge sees between ticks t and t+1).
    pub fn step(&mut self) {
        if let Some(r) = self.replay.as_mut() {
            r.step();
            self.tick = r.tick;
            return;
        }
        if !self.simulated() || self.halted.is_some() {
            return;
        }
        self.apply_due();
        // a shulker design: the judge's visit after the previous tick's frame
        if self.visit_due
            && let Some(ch) = self.shulker.as_mut()
        {
            ch.visit(&mut self.world);
        }
        self.prev_falling = self.world.gravity.entities.iter().filter(|f| !f.removed).map(|f| (f.id, [f.x, f.y, f.z])).collect();
        self.world.tick();
        self.visit_due = self.shulker.is_some();
        self.tick += 1;
        self.apply_due();
        self.inputs_due();
        self.rescan_floor();
        // redsim met something it does not model: the view stops at the last exact state
        if let Some(e) = self.world.errors.first() {
            self.halted = Some(format!("tick {}: {e}", self.tick));
        }
    }

    /// Ticks run since the last input (or since rest / placement).
    pub fn since_input(&self) -> u32 {
        self.tick - self.last_input.unwrap_or(self.rest_tick)
    }

    /// Is this cell clickable (a lever, a button, an input port)?
    pub fn is_input(&self, p: Pos) -> bool {
        self.inputs.contains(&p)
    }

    /// Drive circuit input `i` like the judge (between ticks): its cell written with updates, or its
    /// lever used at every edge, or its button at a rising edge. False when nothing changed.
    pub fn set_input(&mut self, i: usize, on: bool) -> bool {
        if !self.simulated() || i >= self.circuit_inputs.len() || self.input_on[i] == on {
            return false;
        }
        let ci = self.circuit_inputs[i].clone();
        match &ci.how {
            crate::circuit::InputHow::Block { on: a, off: b } => {
                self.world.set_block(bp(ci.pos), if on { *a } else { *b }, flags::ALL);
            }
            crate::circuit::InputHow::Lever => {
                self.world.use_block(bp(ci.pos));
            }
            crate::circuit::InputHow::Button => {
                if on {
                    self.world.use_block(bp(ci.pos));
                }
            }
            crate::circuit::InputHow::Strength { .. } => {
                self.set_strength(i, if on { 15 } else { 0 });
            }
            // the judge's switch behind its repeater: placed or removed (the input cell's power
            // follows 2 gt later, through the repeater)
            crate::circuit::InputHow::Driven { dir, .. } => {
                let (_, _, sw) = crate::circuit::driver_cells(ci.pos, *dir);
                let b = if on { Block::parse(crate::circuit::INPUT_ON).unwrap_or(Block::Air) } else { Block::Air };
                self.world.set_block(bp(sw), b, flags::ALL);
            }
        }
        self.input_on[i] = on;
        self.last_input = Some(self.tick);
        true
    }

    /// A strength input's value, like the judge (`circuit::Circuit::actions`): its chest placed
    /// again (its state unchanged), then its contents set to the value's fill
    /// ([`crate::circuit::strength_items`]).
    fn set_strength(&mut self, i: usize, value: u8) {
        let Some(crate::circuit::InputHow::Strength { chest }) = self.circuit_inputs.get(i).map(|c| c.how.clone()) else { return };
        let pos = bp(self.circuit_inputs[i].pos);
        self.world.set_block(pos, chest, flags::ALL);
        if let Err(e) = redsim::redstone::container::apply_block_entities(&mut self.world, [(pos, crate::circuit::strength_items(value).as_str())]) {
            self.world.errors.push(format!("strength input: {e}"));
        }
        self.input_on[i] = value > 0;
        self.last_input = Some(self.tick);
    }

    /// A short pulse on input `i`: on now, off `len` ticks later (a tap on a pulse input).
    pub fn pulse_input(&mut self, i: usize, len: u32) -> bool {
        if !self.set_input(i, true) {
            return false;
        }
        self.releases.push((self.tick + len.max(1), i));
        true
    }

    /// Input `i` goes off at `tick` (a short press released early).
    pub fn release_input_at(&mut self, i: usize, tick: u32) {
        self.releases.push((tick, i));
    }

    /// Replay the judge's stimulus from placement (the auto-demo); its edges due now are applied.
    pub fn attach_demo(&mut self, s: crate::circuit::Schedule) {
        self.demo = Some(s);
        self.inputs_due();
    }

    /// The demo's edges and events, and the pending releases due at this tick.
    fn inputs_due(&mut self) {
        let t = self.tick;
        if let Some(d) = self.demo.clone() {
            for (i, e) in d.edges.iter().enumerate() {
                // a driven input's switch goes 2 gt (its repeater's delay) before the edge, so the
                // input cell's power changes at the stimulus tick (`Circuit::actions`)
                let early = if matches!(self.circuit_inputs.get(i).map(|c| &c.how), Some(crate::circuit::InputHow::Driven { .. })) { 2 } else { 0 };
                for &(at, on) in e {
                    if at.saturating_sub(early) == t {
                        self.set_input(i, on);
                    }
                }
            }
            // the judge's strength values (after the edges, like `Circuit::actions`)
            if self.simulated() {
                for &(at, i, value) in &d.levels {
                    if at == t {
                        self.set_strength(i, value);
                    }
                }
            }
            if self.simulated() {
                for (at, p, op) in &d.events {
                    if *at != t {
                        continue;
                    }
                    let q = bp(*p);
                    match op {
                        crate::circuit::EventOp::Place(b) => {
                            self.world.set_block(q, *b, flags::ALL);
                        }
                        crate::circuit::EventOp::Clear => {
                            self.world.set_block(q, Block::Air, flags::ALL);
                        }
                        crate::circuit::EventOp::Shape(b) => {
                            self.world.set_block(q, *b, flags::CLIENTS);
                        }
                        crate::circuit::EventOp::Update => {
                            let src = self.world.get(q).kind();
                            self.world.update_neighbors_at(q, src);
                        }
                    }
                }
            }
        }
        let due: Vec<usize> = self.releases.iter().filter(|(at, _)| *at <= t).map(|(_, i)| *i).collect();
        self.releases.retain(|(at, _)| *at > t);
        for i in due {
            self.set_input(i, false);
        }
    }

    /// The player's right click on a cell (between ticks): levers flip, buttons press. Returns
    /// false when nothing happened.
    pub fn toggle(&mut self, p: Pos) -> bool {
        if !self.simulated() {
            return false;
        }
        let b = self.world.get(bp(p));
        if !matches!(b, Block::Lever { .. } | Block::Button { .. } | Block::Note { .. } | Block::Trapdoor { iron: false, .. } | Block::Door { iron: false, .. }) {
            return false;
        }
        let ok = self.world.use_block(bp(p));
        if ok {
            self.last_input = Some(self.tick);
        }
        ok
    }

    /// Is a component "on" now (a lever's or button's `powered`, a lamp's `lit`...)?
    pub fn is_on(&self, p: Pos) -> bool {
        if let Some(r) = &self.replay {
            return r.state_at(p).is_some_and(active_state);
        }
        self.simulated() && active(&self.world.get(bp(p)))
    }

    /// Is the output at `o` powered this tick (redsim's state)? Its own power state when it has
    /// one (a lamp lit, dust with power, a powered repeater or observer, an extended piston), else
    /// a redstone signal into its cell (a big dripleaf, a solid block); air never is. Its OUT box
    /// is red while it is.
    pub fn output_powered(&self, o: Pos) -> bool {
        if !self.simulated() || self.replay.is_some() {
            return self.is_on(o);
        }
        let b = self.world.get(bp(o));
        if matches!(b, Block::Air) {
            return false;
        }
        // the clock judge's lamp: the power delivered into its cell (what dust or a repeater
        // there would get; the lamp's own 4 gt off-delay is the lamp's, not the clock's)
        if self.judge_lamp == Some(o) {
            return self.world.best_neighbor_signal(bp(o)) > 0;
        }
        powered_by_state(&b.to_string()).unwrap_or_else(|| self.world.has_neighbor_signal(bp(o)))
    }

    /// The shortest cycle, 2 to `max_p` ticks, the blocks repeat in over the second half of the
    /// first `ticks` ticks of the run as placed (redsim's own run, from a copy). None: they stand
    /// still, cycle slower, or redsim stops. A fast machine starts in slow motion.
    pub fn short_cycle(&self, max_p: u32, ticks: u32) -> Option<u32> {
        if !self.simulated() || self.replay.is_some() || self.initial.is_none() {
            return None;
        }
        let mut f = self.fresh();
        let mut h = Vec::new();
        for _ in 0..ticks {
            h.push(frame_hash(0xcbf29ce484222325, &f.machine_blocks()));
            f.step();
            if f.halted.is_some() {
                return None;
            }
        }
        let n = h.len();
        let from = n - n / 2;
        if h[from..].iter().all(|x| *x == h[n - 1]) {
            return None;
        }
        (2..=max_p).find(|&p| (from..n).all(|t| t >= p as usize && h[t] == h[t - p as usize]))
    }

    /// Is everything this session draws redsim's own state for this tick? Every still block drawn
    /// is the world's block in that cell (a quartz-like block simulated as stone drawn under its
    /// own name), nothing is drawn in an air cell, and the drawn entities are exactly redsim's
    /// live entities and falling blocks, each at redsim's position (a minecart on a rail where the
    /// game's renderer draws it, `AbstractMinecartRenderer.oldRender`: on the rail's line, from
    /// redsim's position). Moving blocks are drawn from redsim's block entities (not compared
    /// cell by cell); markers and outlines are annotations, not blocks. Err: the first difference.
    pub fn drawn_matches_simulation(&self) -> Result<(), String> {
        use redsim::entity::EntityKind;
        if !self.simulated() || self.replay.is_some() {
            return Ok(());
        }
        let w = &self.world;
        let mut still: HashMap<Pos, String> = HashMap::new();
        for b in self.render_blocks(0.0) {
            if b.offset != [0.0; 3] || matches!(w.get(bp(b.cell)), Block::MovingPiston { .. }) {
                continue;
            }
            if let Some(prev) = still.insert(b.cell, b.state.clone()) {
                return Err(format!("{:?} drawn twice ({prev}, {})", b.cell, b.state));
            }
        }
        // the ground (a TNT duper's: drawn on its own) as drawn, cell by cell
        for (c, st) in self.floor_drawn() {
            if let Some(prev) = still.insert(c, st.clone()) {
                return Err(format!("{c:?} drawn twice ({prev}, ground {st})"));
            }
        }
        for y in 0..w.size[1] {
            for z in 0..w.size[2] {
                for x in 0..w.size[0] {
                    let q = w.min.offset(x, y, z);
                    let c = q.arr();
                    let wb = w.get(q);
                    match wb {
                        Block::Air => {
                            if let Some(s) = still.get(&c) {
                                return Err(format!("{c:?}: {s} drawn where redsim has air"));
                            }
                        }
                        Block::MovingPiston { .. } => {}
                        _ => {
                            let want = match (&wb, self.overlay.get(&c)) {
                                (Block::Simple(redsim::block::Simple::Stone), Some(name)) => name.clone(),
                                _ => wb.to_string(),
                            };
                            match still.get(&c) {
                                Some(s) if *s == want => {}
                                Some(s) => return Err(format!("{c:?}: drawn {s}, redsim has {want}")),
                                None => return Err(format!("{c:?}: redsim has {want}, nothing drawn")),
                            }
                        }
                    }
                }
            }
        }
        let drawn = self.entities(1.0);
        let live: Vec<_> = w.entities.list.iter().filter(|e| !e.removed).collect();
        let falling: Vec<_> = w.gravity.entities.iter().filter(|f| !f.removed).collect();
        if drawn.len() != live.len() + falling.len() {
            return Err(format!("{} entities drawn, redsim has {} entities and {} falling blocks", drawn.len(), live.len(), falling.len()));
        }
        let near = |a: [f32; 3], b: [f64; 3]| (0..3).all(|k| (a[k] as f64 - b[k]).abs() < 1e-3);
        for (d, e) in drawn.iter().zip(&live) {
            let p = [e.pos.x, e.pos.y, e.pos.z];
            let want = match &e.kind {
                EntityKind::Minecart(_) => crate::entitymodel::minecart_pose(w, p, e.y_rot, e.x_rot).0,
                _ => p,
            };
            if !near(d.pos, want) {
                return Err(format!("entity {} ({}) drawn at {:?}, redsim has it at {want:?}", e.id, d.kind, d.pos));
            }
            let kind_ok = match &e.kind {
                EntityKind::Item(_) => d.kind.starts_with("item:"),
                EntityKind::PrimedTnt(_) => d.kind.starts_with("primed:"),
                _ => !d.kind.starts_with("item:") && !d.kind.starts_with("primed:") && !d.kind.starts_with("falling:"),
            };
            if !kind_ok {
                return Err(format!("entity {} drawn as {}", e.id, d.kind));
            }
        }
        for (d, f) in drawn[live.len()..].iter().zip(&falling) {
            if !d.kind.starts_with("falling:") || !near(d.pos, [f.x, f.y, f.z]) {
                return Err(format!("falling block {} drawn as {} at {:?}, redsim has it at {:?}", f.id, d.kind, d.pos, [f.x, f.y, f.z]));
            }
        }
        Ok(())
    }

    /// The simulated entities to draw, each at its position `partial` of the way through the
    /// current tick (from where it started the tick), like the game interpolates them.
    pub fn entities(&self, partial: f32) -> Vec<crate::mesh::EntityDraw> {
        use redsim::entity::EntityKind;
        let lerp = |a: f64, b: f64| (a + (b - a) * partial as f64) as f32;
        self.world
            .entities
            .list
            .iter()
            .filter(|e| !e.removed)
            .map(|e| {
                // the design's own type id (its wood, ...) for the entities it summons (spawned
                // first, in file order), else the kind's
                let kind = match self.design.entities.get(e.id as usize) {
                    Some(d) => d.kind.clone(),
                    None => match &e.kind {
                        EntityKind::Boat(b) if b.chest => "minecraft:oak_chest_boat".to_string(),
                        EntityKind::Boat(_) => "minecraft:oak_boat".to_string(),
                        EntityKind::Minecart(m) if m.hopper => "minecraft:hopper_minecart".to_string(),
                        EntityKind::Minecart(_) => "minecraft:minecart".to_string(),
                        EntityKind::Item(_) => String::new(),
                        EntityKind::ArmorStand(_) => "minecraft:armor_stand".to_string(),
                        EntityKind::PrimedTnt(_) => "minecraft:tnt".to_string(),
                    },
                };
                let kind = match &e.kind {
                    EntityKind::Item(it) => format!("item:{}", it.item),
                    _ => kind,
                };
                let yaw = e.y_rot_o + (e.y_rot - e.y_rot_o) * partial;
                let pitch = e.x_rot_o + (e.x_rot - e.x_rot_o) * partial;
                let pos = [lerp(e.xo, e.pos.x), lerp(e.yo, e.pos.y), lerp(e.zo, e.pos.z)];
                let age = e.tick_count as f32 + partial;
                if let EntityKind::PrimedTnt(st) = &e.kind {
                    // TntRenderer: the fuse as drawn (getFuse() - partialTicks + 1) flashes and swells it
                    return crate::mesh::EntityDraw { kind: "primed:minecraft:tnt[unstable=false]".to_string(), pos, fuse: st.fuse as f32 - partial + 1.0, ..Default::default() };
                }
                if let EntityKind::Minecart(_) = &e.kind {
                    // on a rail: the rail's line and slope (AbstractMinecartRenderer.oldRender)
                    let p64 = [e.xo + (e.pos.x - e.xo) * partial as f64, e.yo + (e.pos.y - e.yo) * partial as f64, e.zo + (e.pos.z - e.zo) * partial as f64];
                    let (p, rot, xrot) = crate::entitymodel::minecart_pose(&self.world, p64, yaw, pitch);
                    return crate::mesh::EntityDraw { kind, pos: [p[0] as f32, p[1] as f32, p[2] as f32], yaw: rot, pitch: xrot, age, ..Default::default() };
                }
                crate::mesh::EntityDraw { kind, pos, yaw, pitch, age, ..Default::default() }
            })
            .chain(self.world.gravity.entities.iter().filter(|f| !f.removed).map(|f| {
                // a falling block between where it was before the tick and where it is (one that
                // appeared in the tick: where it is)
                let was = self.prev_falling.iter().find(|(id, _)| *id == f.id).map(|(_, p)| *p).unwrap_or([f.x, f.y, f.z]);
                crate::mesh::EntityDraw { kind: format!("falling:{}", f.block), pos: [lerp(was[0], f.x), lerp(was[1], f.y), lerp(was[2], f.z)], ..Default::default() }
            }))
            .collect()
    }

    /// Every non-air cell of the world box as the judges trace it: (cell, full state = block state
    /// with the comparator strength / container contents the clock judge adds, `wave::with_extra`).
    pub fn cell_states(&self) -> Vec<(Pos, String)> {
        let mut v: Vec<(Pos, String)> = self
            .world
            .blocks()
            .into_iter()
            .map(|(p, b)| {
                let s = b.to_string();
                let full = match be_extra(&self.world, p, &b) {
                    Some(e) => format!("{s}{e}"),
                    None => s,
                };
                (p.arr(), full)
            })
            .collect();
        v.sort();
        v
    }

    /// The containers redsim models (droppers, dispensers, hoppers) with their stacks: (cell,
    /// slot count, (item, count) per non-empty slot).
    pub fn containers(&self) -> Vec<(Pos, usize, Vec<(&'static str, u8)>)> {
        if !self.simulated() || self.replay.is_some() {
            return Vec::new();
        }
        let mut v: Vec<(Pos, usize, Vec<(&'static str, u8)>)> = self
            .world
            .containers
            .map
            .iter()
            .map(|(p, c)| (p.arr(), c.slots.len(), c.slots.iter().flatten().map(|s| (redsim::redstone::container::item_name(s.item), s.count)).collect()))
            .collect();
        v.sort_by_key(|(p, _, _)| *p);
        v
    }

    /// A door's opening: how many of its 9 cells hold a block (0 = open, 9 = closed); None for
    /// other designs.
    pub fn door_blocks(&self) -> Option<usize> {
        let Mode::Door { door, along_z, .. } = self.mode else { return None };
        Some(door_cells(door, along_z).iter().filter(|c| !matches!(self.world.get(bp(**c)), Block::Air)).count())
    }

    /// Nothing pending (no scheduled ticks, events, moving blocks, pending inputs).
    pub fn idle(&self) -> bool {
        self.actions.is_empty() && self.world.idle()
    }

    /// Every visible block at a moment between this tick and the next (`partial` in 0..1):
    /// moving blocks at their interpolated offset, like the client draws them.
    pub fn render_blocks(&self, partial: f32) -> Vec<RBlock> {
        let mut out = Vec::new();
        if let Some(r) = &self.replay {
            return r.render_blocks();
        }
        if !self.simulated() {
            // what redsim could not place is drawn from the design text
            for (p, s) in &self.design.blocks {
                out.push(RBlock { cell: *p, offset: [0.0; 3], state: s.clone() });
            }
            return out;
        }
        let w = &self.world;
        // the machine's part of the world (a TNT duper's ground is drawn on its own: `floor_drawn`)
        let (min, size) = self.machine();
        for y in 0..size[1] {
            for z in 0..size[2] {
                for x in 0..size[0] {
                    let q = min.offset(x, y, z);
                    let b = w.get(q);
                    if b != Block::Air && self.in_floor(q.arr()) {
                        continue;
                    }
                    match b {
                        Block::Air => {}
                        Block::MovingPiston { .. } => {
                            if let Some(be) = w.piston.be(q) {
                                moving_parts(q.arr(), be, partial, &mut out);
                            }
                        }
                        Block::Simple(redsim::block::Simple::Stone) if self.overlay.contains_key(&q.arr()) => {
                            out.push(RBlock { cell: q.arr(), offset: [0.0; 3], state: self.overlay[&q.arr()].clone() });
                        }
                        _ => out.push(RBlock { cell: q.arr(), offset: [0.0; 3], state: b.to_string() }),
                    }
                }
            }
        }
        out
    }

    /// The box the camera frames: the design and its ports (a door's opening included).
    pub fn frame_box(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = match self.picture {
            Some(b) => b,
            None => self.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0])),
        };
        if let Mode::Door { door, along_z, .. } = self.mode {
            for c in door_cells(door, along_z) {
                grow(&mut lo, &mut hi, c);
            }
        }
        // a circuit's input cells (often empty: the judge writes a block there) are in the picture
        for ci in &self.circuit_inputs {
            grow(&mut lo, &mut hi, ci.pos);
        }
        (lo, hi)
    }

    /// [`Session::frame_box`] plus a travelling design's whole path, up to where its judge stops
    /// it (the corridor's edge): the floor goes under it.
    pub fn path_box(&self) -> (Pos, Pos) {
        let (mut lo, mut hi) = self.frame_box();
        if self.corridor.is_some()
            && let Some(((a, b), margin)) = travel_box(&self.design, &self.mode)
        {
            grow(&mut lo, &mut hi, a);
            grow(&mut lo, &mut hi, b);
            if let Some(v) = self.design.meta.get("travel").and_then(|t| parse_travel(t)) {
                for i in 0..3 {
                    if v[i] > 0 {
                        hi[i] += margin;
                    } else if v[i] < 0 {
                        lo[i] -= margin;
                    }
                }
            }
        }
        (lo, hi)
    }

    /// A flying machine's end, the way `circuit fly sim` ends a run: the first tick a block stands
    /// on the corridor's edge, or something is destroyed, or nothing is pending (at rest), or the
    /// horizon (200 + 60 gt per corridor cell).
    pub fn fly_end(&self) -> u32 {
        let Some((lo, hi)) = self.corridor else { return 0 };
        let k = self.design.meta.get("distance").and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(32).max(1);
        let horizon = 200 + 60 * k;
        let mut s = self.fresh();
        let edge = |p: BlockPos| p.x <= lo[0] || p.y <= lo[1] || p.z <= lo[2] || p.x >= hi[0] || p.y >= hi[1] || p.z >= hi[2];
        loop {
            let t = s.tick;
            if !s.world.drops.is_empty() || !s.world.errors.is_empty() || s.world.blocks().iter().any(|(p, _)| edge(*p)) || (s.actions.is_empty() && s.world.idle()) || t >= horizon {
                return t;
            }
            s.step();
        }
    }

    /// A digest of every frame (the non-air cells, sorted) of ticks 0..=`ticks` from placement:
    /// `mctas web check` compares it with `circuit fly sim`'s frames and with the shipped wasm's.
    pub fn frames_digest(&self, ticks: u32) -> u64 {
        let mut s = self.fresh();
        let mut h: u64 = 0xcbf29ce484222325;
        for _ in 0..=ticks {
            // a run redsim stops following (an error: something it does not model) is shown up to
            // its last exact frame
            if s.halted.is_some() {
                break;
            }
            h = frame_hash(h, &s.world.blocks());
            s.step();
        }
        h
    }

    /// [`Session::frames_digest`] with every entity too ([`entity_hash`]: its kind and the bits of
    /// its position and motion): `web check`'s TNT designs.
    pub fn frames_entities_digest(&self, ticks: u32) -> u64 {
        let mut s = self.fresh();
        let mut h: u64 = 0xcbf29ce484222325;
        for _ in 0..=ticks {
            if s.halted.is_some() {
                break;
            }
            h = frame_hash(h, &s.world.blocks());
            h = entity_hash(h, &s.world);
            s.step();
        }
        h
    }

    /// Run `n` ticks (stops early at an error).
    pub fn run(&mut self, n: u32) {
        for _ in 0..n {
            self.step();
        }
    }
}

/// One block to draw: its cell, an offset in blocks (a moving block), its state string.
#[derive(Clone, Debug, PartialEq)]
pub struct RBlock {
    pub cell: Pos,
    pub offset: [f32; 3],
    pub state: String,
}

/// A moving block entity as the client's `PistonHeadRenderer` draws it.
fn moving_parts(cell: Pos, be: &MovingBe, partial: f32, out: &mut Vec<RBlock>) {
    let p = (be.progress_o as f32 + (be.progress as f32 - be.progress_o as f32) * partial.clamp(0.0, 1.0)) / 2.0;
    let p = p.min(1.0);
    let ext = if be.extending { p - 1.0 } else { 1.0 - p };
    let st = be.direction.step();
    let off = [st[0] as f32 * ext, st[1] as f32 * ext, st[2] as f32 * ext];
    match be.moved {
        Block::PistonHead { sticky, facing, .. } => {
            out.push(RBlock { cell, offset: off, state: Block::PistonHead { sticky, facing, short: p <= 0.5 }.to_string() });
        }
        Block::Piston { sticky, facing, .. } if be.source && !be.extending => {
            out.push(RBlock { cell, offset: off, state: Block::PistonHead { sticky, facing, short: p >= 0.5 }.to_string() });
            out.push(RBlock { cell, offset: [0.0; 3], state: Block::Piston { sticky, facing, extended: true }.to_string() });
        }
        m => out.push(RBlock { cell, offset: off, state: m.to_string() }),
    }
}

/// The clock judge's fast 128-bit digest (`crates/clock/src/sim.rs::H128`, copied so the browser
/// finds the same repeats).
pub struct H128 {
    pub a: u64,
    pub b: u64,
}

impl Default for H128 {
    fn default() -> Self {
        H128 { a: 0xcbf29ce484222325, b: 0x84222325cbf29ce4 }
    }
}

impl Hasher for H128 {
    fn finish(&self) -> u64 {
        self.a
    }
    fn write(&mut self, bytes: &[u8]) {
        for &x in bytes {
            self.a = (self.a ^ x as u64).wrapping_mul(0x100000001b3);
            self.b = (self.b.rotate_left(7) ^ x as u64).wrapping_mul(0x9e3779b97f4a7c15);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.a = (self.a ^ x).wrapping_mul(0x100000001b3).rotate_left(29);
        self.b = (self.b ^ x.rotate_left(17)).wrapping_mul(0x9e3779b97f4a7c15).rotate_left(31);
    }
    fn write_u8(&mut self, x: u8) {
        self.write_u64(x as u64 | 0x100);
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64 | 0x2_0000_0000);
    }
    fn write_i32(&mut self, x: i32) {
        self.write_u32(x as u32)
    }
    fn write_i64(&mut self, x: i64) {
        self.write_u64(x as u64 ^ 0x5555_0000_0000_0000)
    }
    fn write_isize(&mut self, x: isize) {
        self.write_u64(x as u64 ^ 0x3333_0000_0000_0000)
    }
    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64 ^ 0x7777_0000_0000_0000)
    }
}

/// A world's whole-state digest (`clock::sim::digest`).
pub fn digest(w: &World) -> u128 {
    let mut h = H128::default();
    w.state_key_into(&mut h);
    ((h.a as u128) << 64) | h.b as u128
}

/// A clock's exact numbers on redsim, as `mctas clock sim` finds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Period {
    /// First tick of the first exact repeat of the whole state.
    pub state_warmup: u32,
    /// Distance to the repeat.
    pub state_period: u32,
    /// The visible period: the smallest divisor of the state period over which every cell (block
    /// state + comparator strength + container contents) repeats.
    pub period: u32,
    /// The first tick from which the visible state repeats.
    pub warmup: u32,
}

/// Comparator strength and container contents: the clock judge's `wave::be_extra_redsim`.
fn be_extra(w: &World, p: BlockPos, b: &Block) -> Option<String> {
    match b {
        Block::Comparator { .. } => Some(format!("{{out={}}}", w.comparator_out.get(&p).copied().unwrap_or(0))),
        Block::Hopper { .. } | Block::Dispenser { .. } | Block::Crafter { .. } => {
            let c = w.containers.map.get(&p);
            let mut list: Vec<String> = Vec::new();
            if let Some(c) = c {
                for (slot, st) in c.slots.iter().enumerate() {
                    if let Some(st) = st {
                        list.push(format!("{slot}:{}:{}", st.count, redsim::redstone::container::item_name(st.item).trim_start_matches("minecraft:")));
                    }
                }
            }
            let cd = if matches!(b, Block::Hopper { .. }) { format!(",cd={}", c.map(|c| c.cooldown).unwrap_or(-1)) } else { String::new() };
            Some(format!("{{items={}{cd}}}", list.join(",")))
        }
        _ => None,
    }
}

/// The machine's visible state this tick (every cell of its box with its comparator strength or
/// container contents; a TNT duper's ground left out, like its judge's frames).
fn frame_digest(s: &Session) -> u128 {
    use std::hash::Hash;
    let mut h = H128::default();
    let w = &s.world;
    let (min, size) = s.machine();
    for y in 0..size[1] {
        for z in 0..size[2] {
            for x in 0..size[0] {
                let q = min.offset(x, y, z);
                if s.in_floor(q.arr()) {
                    continue;
                }
                let b = w.get(q);
                b.hash(&mut h);
                be_extra(w, q, &b).hash(&mut h);
            }
        }
    }
    ((h.a as u128) << 64) | h.b as u128
}

impl Session {
    /// The tick from which the clock runs with no input (the end of its kick).
    pub fn free_from(&self) -> u32 {
        self.initial.as_ref().map(|(_, a, _)| a.iter().map(|(t, _)| *t).max().unwrap_or(0)).unwrap_or(0)
    }

    /// The clock judge's exact period detector on a fresh copy of the placed state (the session
    /// itself is not moved): first exact repeat of the whole state, then the visible period.
    pub fn period(&self, horizon: u32) -> Result<Period, String> {
        if let Some(why) = &self.unsimulated {
            return Err(format!("not modeled by redsim: {why}"));
        }
        let end = self.free_from();
        let found = if !self.design.entities.is_empty() || self.world.tnt_kick.is_some() {
            // designs with entities never repeat their exact state (an entity's doubles drift;
            // a TNT design's TNT keep coming and exploding): the real-game judge's rule
            // (`clock::sim::entity_clock_period`): the block frames over 4000 gt, periodic for 3
            // periods from the first free tick
            let mut s = self.fresh();
            let mut ids: HashMap<u128, u64> = HashMap::new();
            let mut frames: Vec<u64> = Vec::with_capacity(ENTITY_HORIZON as usize + 1);
            for t in 0..=ENTITY_HORIZON {
                s.apply_due();
                s.check(t)?;
                let d = frame_digest(&s);
                let n = ids.len() as u64;
                frames.push(*ids.entry(d).or_insert(n));
                if t < ENTITY_HORIZON {
                    s.world.tick();
                    s.tick += 1;
                }
            }
            Some(detect_period(&frames, end as usize, 3).map(|(w, p)| (w as u32, p as u32)).ok_or_else(|| format!("not periodic for 3 periods within {ENTITY_HORIZON} gt (the judge's rule for designs with entities)"))?)
        } else {
            None
        };
        let mut s = self.fresh();
        let mut seen: HashMap<u128, u32> = HashMap::new();
        let mut found = found;
        for t in 0..=horizon {
            if found.is_some() {
                break;
            }
            s.apply_due();
            s.check(t)?;
            if t >= end {
                let d = digest(&s.world);
                if let Some(&t0) = seen.get(&d) {
                    found = Some((t0, t - t0));
                    break;
                }
                seen.insert(d, t);
            }
            s.world.tick();
            s.tick += 1;
        }
        let (w0, p) = found.ok_or_else(|| format!("no repeat of its state within {horizon} gt"))?;
        let mut s = self.fresh();
        let mut frames: Vec<u128> = Vec::with_capacity((w0 + p + 1) as usize);
        for t in 0..=(w0 + p) {
            s.apply_due();
            s.check(t)?;
            frames.push(frame_digest(&s));
            if t == w0 + p {
                break;
            }
            s.world.tick();
            s.tick += 1;
        }
        let (w0u, pu) = (w0 as usize, p as usize);
        let full_p = (1..=pu).filter(|d| pu % d == 0).find(|&d| (w0u..w0u + pu).all(|t| frames[t] == frames[w0u + (t - w0u + d) % pu])).unwrap_or(pu);
        let mut vis_w = w0u;
        while vis_w > end as usize && frames[vis_w - 1] == frames[vis_w - 1 + full_p] {
            vis_w -= 1;
        }
        Ok(Period { state_warmup: w0, state_period: p, period: full_p as u32, warmup: vis_w as u32 })
    }

    /// Where the entities and falling blocks go over the first `ticks` ticks of the run as placed
    /// (redsim's own run, from a copy): for framing the build's whole moving extent.
    pub fn run_extent(&self, ticks: u32) -> Vec<[f32; 3]> {
        if !self.simulated() || self.replay.is_some() || self.initial.is_none() {
            return Vec::new();
        }
        let mut f = self.fresh();
        let mut pts = Vec::new();
        for _ in 0..ticks {
            f.step();
            if f.halted.is_some() {
                break;
            }
            pts.extend(f.entities(1.0).iter().map(|e| e.pos));
        }
        pts
    }

    /// A copy back at placement (the inputs due at its tick applied).
    fn fresh(&self) -> Session {
        let (w, a, t) = self.initial.clone().expect("simulated session");
        let mut s = Session {
            design: Design::default(),
            mode: self.mode.clone(),
            world: w,
            tick: t,
            rest_tick: self.rest_tick,
            last_input: None,
            actions: a,
            unsimulated: None,
            overlay: HashMap::new(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            initial: None,
            replay: None,
            corridor: self.corridor,
            end_tick: None,
            circuit_inputs: Vec::new(),
            input_on: Vec::new(),
            demo: None,
            releases: Vec::new(),
            picture: None,
            consumers: Vec::new(),
            judge_lamp: self.judge_lamp,
            prev_falling: Vec::new(),
            halted: None,
            shulker: self.shulker_initial.clone(),
            shulker_initial: None,
            visit_due: false,
            floor: self.floor.clone(),
            machine_box: self.machine_box,
        };
        s.apply_due();
        s.rescan_floor();
        s
    }

    /// The clock judge's validity checks between ticks.
    fn check(&self, t: u32) -> Result<(), String> {
        if let Some(e) = self.world.errors.first() {
            if e.contains("outside the box") {
                return Err(format!("INVALID: escapes its box by more than the margin by tick {t}: {e}"));
            }
            return Err(format!("UNMODELED: tick {t}: {e}"));
        }
        if let Some((p, b)) = self.world.drops.first() {
            return Err(format!("INVALID: tick {t}: {b} at {p} was destroyed with a drop (an item entity)"));
        }
        Ok(())
    }

    /// The clock's (warmup, period) for pictures: the exact detector like `clock::sim::activity`
    /// (whole-state warmup, visible period), else the archive's judge line.
    pub fn picture_period(&self, horizon: u32) -> Option<(u32, u32)> {
        if !self.simulated() || self.mode.is_door() {
            return None;
        }
        match self.period(horizon) {
            Ok(r) if r.period > 0 => Some((r.state_warmup.max(r.warmup), r.period)),
            _ => self.design.clock_hint().filter(|(w, p)| w + p <= horizon && *p > 0),
        }
    }

    /// One period from `w0`: every changing cell and its state at each tick (two passes on fresh
    /// copies; the session is not moved).
    pub fn period_cells(&self, w0: u32, p: u32) -> Vec<(Pos, Vec<Block>)> {
        let mut s = self.fresh();
        while s.tick < w0 {
            s.step();
        }
        let first = cells_of(&s.world);
        let mut changing = vec![false; first.len()];
        for _ in 0..p {
            for (i, b) in cells_of(&s.world).iter().enumerate() {
                if *b != first[i] {
                    changing[i] = true;
                }
            }
            s.step();
        }
        let idx: Vec<usize> = (0..first.len()).filter(|&i| changing[i]).collect();
        let w = &s.world;
        let pos_of = |i: usize| -> Pos {
            let (sx, sz) = (w.size[0] as usize, w.size[2] as usize);
            let (x, z, y) = (i % sx, (i / sx) % sz, i / (sx * sz));
            [w.min.x + x as i32, w.min.y + y as i32, w.min.z + z as i32]
        };
        let mut out: Vec<(Pos, Vec<Block>)> = idx.iter().map(|&i| (pos_of(i), Vec::with_capacity(p as usize))).collect();
        let mut s = self.fresh();
        while s.tick < w0 {
            s.step();
        }
        for _ in 0..p {
            let cells = cells_of(&s.world);
            for (k, &i) in idx.iter().enumerate() {
                out[k].1.push(cells[i]);
            }
            s.step();
        }
        out
    }

    /// The clock judge's output when none is declared: the changing cell with the longest own
    /// period, then the most rising edges, then the most ticks on (`clock::sim::simulate`).
    pub fn auto_output(cells: &[(Pos, Vec<Block>)]) -> Option<Pos> {
        let stat = |seq: &[Block]| {
            let n = seq.len();
            let period = (1..=n.max(1)).filter(|d| n % d == 0).find(|&d| (0..n).all(|t| seq[t] == seq[(t + d) % n])).unwrap_or(n.max(1));
            let on: Vec<bool> = seq.iter().map(active).collect();
            let edges = (0..n).filter(|&t| on[t] && !on[(t + n - 1) % n]).count();
            let ons = on.iter().filter(|b| **b).count();
            (period, edges, ons)
        };
        cells.iter().max_by(|a, b| stat(&a.1).cmp(&stat(&b.1)).then(b.0.cmp(&a.0))).map(|(p, _)| *p)
    }

    /// The still frame a thumbnail shows: a door at rest (the settled closed state); a clock at
    /// the most active moment of one period (most changing cells on, nothing moving if
    /// possible), like `mctas clock shot` picks it. Leaves the session at that tick and records the
    /// clock judge's output when the design declares none.
    pub fn go_to_still(&mut self, horizon: u32) -> u32 {
        // a travelling design (flying machine, extender) is pictured as built: no period to find
        if self.corridor.is_some() {
            return self.tick;
        }
        let Some((w0, p)) = self.picture_period(horizon) else { return self.tick };
        let cells = self.period_cells(w0, p);
        if self.outputs.is_empty()
            && let Some(o) = Session::auto_output(&cells)
        {
            self.outputs.push(o);
        }
        self.picture = self.picture_box(horizon);
        let mut best = (0u32, (std::cmp::Reverse(usize::MAX), 0usize));
        for t in 0..p as usize {
            let on = cells.iter().filter(|(_, seq)| active(&seq[t])).count();
            let moving = cells.iter().filter(|(_, seq)| matches!(seq[t], Block::MovingPiston { .. })).count();
            let score = (std::cmp::Reverse(moving), on);
            if t == 0 || score > best.1 {
                best = (t as u32, score);
            }
        }
        let target = w0 + best.0;
        self.reset();
        while self.tick < target {
            self.step();
        }
        target
    }

    /// A clock's picture box, like `clock shot`: the judge's swept box (`clock::sim`: the design's
    /// box with its kick cell, grown by every cell occupied over the state warmup and one state
    /// period), and for an elongated clock (its longest side over 16 blocks and over 5x the next) a
    /// window of it along that side around the output, 3x the next side long (8 to 16 blocks).
    pub fn picture_box(&self, horizon: u32) -> Option<(Pos, Pos)> {
        if !self.simulated() || self.mode.is_door() || self.corridor.is_some() || self.replay.is_some() {
            return None;
        }
        let r = self.period(horizon).ok()?;
        let (mut lo, mut hi) = self.design.bounds()?;
        if let Mode::Clock { kick: Some((k, _)), .. } = &self.mode {
            let p = match k {
                Kick::Last { pos } | Kick::Use { pos, .. } | Kick::Place { pos, .. } => *pos,
            };
            grow(&mut lo, &mut hi, p);
        }
        let mut s = self.fresh();
        let end = r.state_warmup + r.state_period;
        loop {
            for (q, _) in s.machine_blocks() {
                grow(&mut lo, &mut hi, q.arr());
            }
            if s.tick >= end {
                break;
            }
            s.step();
        }
        let ext = |lo: &Pos, hi: &Pos, k: usize| hi[k] - lo[k] + 1;
        let mut axes = [0usize, 1, 2];
        axes.sort_by_key(|&k| std::cmp::Reverse(ext(&lo, &hi, k)));
        let (long, next) = (axes[0], axes[1]);
        if ext(&lo, &hi, long) > 16 && ext(&lo, &hi, long) > 5 * ext(&lo, &hi, next) {
            let win = (3 * ext(&lo, &hi, next)).clamp(8, 16);
            let c = self.outputs.first().map(|o| o[long]).unwrap_or((lo[long] + hi[long]) / 2);
            let a = (c - win / 2).clamp(lo[long], hi[long] - win + 1);
            lo[long] = a;
            hi[long] = a + win - 1;
        }
        Some((lo, hi))
    }

    /// Find the clock judge's output (no `port output`) without moving the session.
    pub fn find_auto_output(&mut self, horizon: u32) {
        // a circuit has its outputs as ports (a TNT duper has none: no marker)
        if !self.outputs.is_empty() || self.corridor.is_some() || self.design.meta.contains_key("circuit") {
            return;
        }
        if let Some((w0, p)) = self.picture_period(horizon)
            && let Some(o) = Session::auto_output(&self.period_cells(w0, p))
        {
            self.outputs.push(o);
        }
    }
}

/// Fold one frame (non-air cells, any order) into an FNV-1a digest: the cells sorted by position,
/// each as `x,y,z=state;`, then `|`.
pub fn frame_hash(mut h: u64, cells: &[(BlockPos, Block)]) -> u64 {
    let mut v: Vec<(BlockPos, String)> = cells.iter().map(|(p, b)| (*p, b.to_string())).collect();
    v.sort_by_key(|(p, _)| (p.x, p.y, p.z));
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for (p, s) in &v {
        eat(format!("{},{},{}={s};", p.x, p.y, p.z).as_bytes());
    }
    eat(b"|");
    h
}

/// Every live entity (falling blocks included) folded into `h` in spawn order: its kind and the
/// bits of its position and motion (FNV-1a, like [`frame_hash`]).
pub fn entity_hash(mut h: u64, w: &World) -> u64 {
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
    };
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        eat(format!("{:?}", std::mem::discriminant(&e.kind)).as_bytes());
        for v in [e.pos.x, e.pos.y, e.pos.z, e.delta_movement.x, e.delta_movement.y, e.delta_movement.z] {
            eat(&v.to_bits().to_le_bytes());
        }
    }
    for f in w.gravity.entities.iter().filter(|f| !f.removed) {
        for v in [f.x, f.y, f.z, f.vy] {
            eat(&v.to_bits().to_le_bytes());
        }
    }
    eat(b"#");
    h
}

/// The block the clock judge puts in a clock's empty output cell (`clock::sim::JUDGE_LAMP`).
pub const JUDGE_LAMP: &str = "minecraft:redstone_lamp[lit=false]";

/// A design with a TNT block: it runs with redsim's TNT entities and explosions
/// (`explosion::explode_on`).
pub fn is_tnt_design(d: &Design) -> bool {
    d.blocks.values().any(|s| s.trim_start_matches("minecraft:").split('[').next() == Some("tnt"))
}
/// Is a block powered by its own state: a lamp lit, dust with power, a powered repeater or
/// observer, an extended piston, a hopper locked, a sculk sensor active? None: it has no such
/// state (a big dripleaf, a solid block: see [`Session::output_powered`]).
fn powered_by_state(state: &str) -> Option<bool> {
    let st = crate::models::St::parse(state);
    match st.name {
        "hopper" => return Some(st.get("enabled") == Some("false")),
        "sculk_sensor" | "calibrated_sculk_sensor" => return Some(st.get("sculk_sensor_phase") == Some("active")),
        _ => {}
    }
    for k in ["powered", "lit", "extended", "triggered", "open", "attached"] {
        if let Some(v) = st.get(k) {
            return Some(v == "true");
        }
    }
    st.get("power").map(|p| p.parse::<u32>().is_ok_and(|p| p > 0))
}

pub fn active_state(state: &str) -> bool {
    let st = crate::models::St::parse(state);
    match st.name {
        "air" | "cave_air" | "void_air" => return false,
        "piston_head" | "moving_piston" => return true,
        "hopper" => return st.get("enabled") == Some("false"),
        "sculk_sensor" | "calibrated_sculk_sensor" => return st.get("sculk_sensor_phase") == Some("active"),
        "leaves" => return st.get("distance").and_then(|d| d.parse::<u32>().ok()).is_some_and(|d| d < 7),
        _ => {}
    }
    for k in ["powered", "lit", "extended", "open", "triggered", "attached"] {
        if let Some(v) = st.get(k) {
            return v == "true";
        }
    }
    if let Some(p) = st.get("power") {
        return p.parse::<u32>().is_ok_and(|p| p > 0);
    }
    true
}

/// Is a component "on" (`crates/clock/src/wave.rs::active`)?
pub fn active(b: &Block) -> bool {
    match *b {
        Block::Air => false,
        Block::Piston { extended, .. } => extended,
        Block::PistonHead { .. } | Block::MovingPiston { .. } => true,
        Block::Sculk { phase, .. } => phase == 1,
        Block::Hopper { enabled, .. } => !enabled,
        Block::Dispenser { triggered, .. } => triggered,
        Block::Crafter { triggered, .. } => triggered,
        Block::Leaves { distance, .. } => distance < 7,
        Block::Scaffolding { distance, .. } => distance > 0,
        Block::Door { open, .. } | Block::Trapdoor { open, .. } | Block::FenceGate { open, .. } => open,
        Block::Torch { lit } | Block::WallTorch { lit, .. } | Block::Lamp { lit } | Block::Bulb { lit, .. } => lit,
        Block::Wire { power, .. } | Block::Target { power } => power > 0,
        Block::Lever { powered, .. }
        | Block::Button { powered, .. }
        | Block::Repeater { powered, .. }
        | Block::Comparator { powered, .. }
        | Block::Observer { powered, .. }
        | Block::Note { powered, .. }
        | Block::Rail { powered, .. }
        | Block::Skull { powered, .. }
        | Block::WallSkull { powered, .. } => powered,
        _ => true,
    }
}

/// Every cell of the world's box, in storage order.
pub fn cells_of(w: &World) -> Vec<Block> {
    let mut v = Vec::with_capacity((w.size[0] * w.size[1] * w.size[2]) as usize);
    let min = w.min;
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                v.push(w.get(min.offset(x, y, z)));
            }
        }
    }
    v
}

/// `out`, `out_a`, `out2`, `output`, `output_b`...: a port named after `prefix`.
/// The real-game judge's horizon for designs with entities (`clock::sim::ENTITY_HORIZON`).
pub const ENTITY_HORIZON: u32 = 4000;

/// `clock::wave::detect_period`: the smallest period of a sequence of frame ids periodic from some
/// warmup on, seen in `frames[start..]`: per candidate p the earliest W from which
/// `frames[t] == frames[t + p]` holds to the end, with at least `min_periods` periods after it;
/// the smallest W, then the smallest p.
pub fn detect_period(frames: &[u64], start: usize, min_periods: usize) -> Option<(usize, usize)> {
    let n = frames.len();
    if n <= start + 1 {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let maxp = (n - start) / min_periods.max(1);
    for p in 1..=maxp {
        let mut w = n - p;
        while w > start && frames[w - 1] == frames[w - 1 + p] {
            w -= 1;
        }
        if n - w < min_periods * p {
            continue;
        }
        if best.is_none_or(|(bw, bp)| (w, p) < (bw, bp)) {
            best = Some((w, p));
        }
    }
    best
}

fn is_port_named(name: &str, prefix: &str) -> bool {
    let long = if prefix == "in" { "input" } else { "output" };
    let rest = name.strip_prefix(long).or_else(|| name.strip_prefix(prefix));
    rest.is_some_and(|r| r.is_empty() || r.starts_with('_') || r.starts_with('-') || r.starts_with('.') || r.chars().all(|c| c.is_ascii_alphanumeric()) && r.chars().next().is_some_and(|c| c.is_ascii_digit() || c.is_ascii_uppercase()))
}

/// Unit step of a direction as f32.
pub fn dir_step(d: Dir) -> [f32; 3] {
    let s = d.step();
    [s[0] as f32, s[1] as f32, s[2] as f32]
}

#[cfg(test)]
mod tests {
    use super::*;

    const BULB_CLOCK: &str = "name: c\nport start: 0 0 0 last\norigin: 0 0 0\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";

    /// A shulker loader and an unloader run live (redsim's driver of the judge's chests): their
    /// dispensers place box after box, the pistons break them, the drops are picked up, and a
    /// reset replays the same run.
    #[test]
    fn shulker_designs_run_live() {
        for (file, boxes) in [("shulker-ref-loader-1", 5), ("shulker-ref-unloader-1", 5), ("shulker-filter-loader", 1)] {
            let text = std::fs::read_to_string(format!("{}/../../designs/circuits/shulker/archive/{file}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut s = Session::new(&text, &Options::default()).unwrap();
            assert!(s.simulated(), "{file}: {:?}", s.unsimulated);
            // the visitor turns the start lever on (the judge does at its start)
            for i in 0..s.circuit_inputs.len() {
                s.set_input(i, true);
            }
            let cell = crate::circuit::box_cells(&s.design)[0];
            let ticks = if file.contains("filter") { 16000 } else { 1300 };
            let (mut placed, mut was) = (0, false);
            for _ in 0..ticks {
                s.step();
                let now = matches!(s.world.get(bp(cell)), Block::ShulkerBox { .. });
                placed += (now && !was) as u32;
                was = now;
            }
            assert!(s.world.errors.is_empty(), "{file}: {:?}", s.world.errors);
            assert!(placed >= boxes, "{file}: {placed} boxes in {ticks} gt");
            let d = s.frames_digest(400);
            s.reset();
            assert_eq!(d, s.frames_digest(400), "{file}: a reset replays the run");
        }
    }

    #[test]
    fn entity_clocks_run_their_entities() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-misc-boat-ring-3x3.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let start = s.entities(0.0);
        assert_eq!(start.len(), 1, "the boat");
        assert_eq!(start[0].kind, "minecraft:oak_boat");
        for _ in 0..60 {
            s.step();
        }
        let later = s.entities(0.0);
        let moved = (0..3).map(|k| (later[0].pos[k] - start[0].pos[k]).abs()).sum::<f32>();
        assert!(moved > 0.5, "the boat moves with the current: {:?} -> {:?}", start[0].pos, later[0].pos);
    }

    /// A falling block (redsim's gravity entities) is drawn: its block model where it is, from
    /// where it was before the tick (redsim's own position then, never a guess).
    #[test]
    fn falling_blocks_are_drawn() {
        let mut s = Session::new(BULB_CLOCK, &Options::default()).unwrap();
        let sand = redsim::Block::parse("minecraft:sand").unwrap();
        let top = (s.world.min.y + s.world.size[1] - 1) as f64;
        s.world.gravity.entities.push(redsim::gravity::FallingBlock::new(sand, 4.5, top, 0.5, 0.0, 3, 0));
        // just appeared (no tick run since): where it is
        let e = s.entities(0.5);
        let f = e.iter().find(|e| e.kind.starts_with("falling:")).expect("the falling block");
        assert!((f.pos[1] as f64 - top).abs() < 1e-4, "{:?}", f.pos);
        s.step();
        let now = s.world.gravity.entities.iter().find(|f| !f.removed).expect("still falling").y;
        assert!(now < top, "it fell: {now}");
        let e = s.entities(0.5);
        let f = e.iter().find(|e| e.kind.starts_with("falling:")).expect("the falling block");
        assert!(f.kind.contains("sand"), "{}", f.kind);
        assert!((f.pos[1] as f64 - (top + now) / 2.0).abs() < 1e-4, "half way from {top} to {now}: {:?}", f.pos);
        s.drawn_matches_simulation().unwrap();
        let atlas = crate::atlas::Atlas::build();
        let mut b = crate::mesh::Builder::new(&atlas);
        b.entity(f);
        assert!(!b.mesh.solid.is_empty(), "a falling block draws its block model");
        // a full block, half a block back from its position (FallingBlockRenderer)
        let lo = (0..3).map(|k| b.mesh.solid.iter().map(|v| v.pos[k]).fold(f32::MAX, f32::min)).collect::<Vec<_>>();
        assert!((lo[0] - 4.0).abs() < 1e-4 && (lo[1] - f.pos[1]).abs() < 1e-4 && lo[2].abs() < 1e-4, "{lo:?}");
    }

    /// vjeux: the view shows only what redsim simulates. Everything drawn is redsim's state on
    /// every tick: a TNT design's blasts (TNT entities, drops), an entity clock's boats, a
    /// shulker unloader's carts and boxes.
    #[test]
    fn drawn_is_redsims_state() {
        for (file, ticks) in [("circuits/tnt/archive/tnt-clocked-22b", 300), ("clocks/archive/clk-misc-boat-ring-3x3", 200), ("circuits/shulker/archive/shulker-ref-unloader-1", 300)] {
            let text = std::fs::read_to_string(format!("{}/../../designs/{file}.txt", env!("CARGO_MANIFEST_DIR"))).unwrap();
            let mut s = Session::new(&text, &Options::default()).unwrap();
            assert!(s.simulated(), "{file}: {:?}", s.unsimulated);
            for i in 0..s.circuit_inputs.len() {
                s.set_input(i, true);
            }
            let mut entities = 0;
            for _ in 0..ticks {
                s.drawn_matches_simulation().unwrap_or_else(|e| panic!("{file} tick {}: {e}", s.tick));
                entities = entities.max(s.entities(1.0).len());
                s.step();
            }
            assert!(s.halted.is_none(), "{file}: {:?}", s.halted);
            assert!(entities > 0 || file.contains("shulker"), "{file}: no entity drawn");
        }
    }

    /// vjeux 17:53Z: "Please don't put a fake ground if the tnt goes through it". A TNT duper
    /// stands on its judge's ground, real blocks of its world: grass over two dirt over bedrock,
    /// its top the archived safe drop under the design (tnt-clocked-22b, the TNT study's one
    /// duper since 2026-10-20: 0 blocks, right under it), the judge's reach around it; its TNT
    /// land and explode on it (craters: ground cells that changed), and what is drawn of it is the
    /// world's every tick. A clock has no ground at all.
    #[test]
    fn tnt_dupers_stand_on_their_judges_ground() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits/tnt/archive/tnt-clocked-22b.txt")).unwrap();
        assert_eq!(safe_drop(&text), 0);
        let mut s = Session::new(&text, &Options::default()).unwrap();
        assert!(s.simulated(), "{:?}", s.unsimulated);
        let f = s.floor.clone().expect("a TNT duper's ground");
        let lowest = s.design.blocks.keys().map(|p| p[1]).min().unwrap();
        assert_eq!(f.hi[1], lowest - 1, "the ground right under the design");
        assert_eq!(f.hi[1] - f.lo[1], 3, "4 layers");
        assert!([GROUND_REACH, 2 * GROUND_REACH, MAX_GROUND_REACH].contains(&f.reach), "reach {}", f.reach);
        assert_eq!(s.world.get(BlockPos::new(f.lo[0], f.hi[1], f.lo[2])).to_string(), Block::parse("minecraft:grass_block").unwrap().to_string());
        assert_eq!(s.world.get(BlockPos::new(f.hi[0], f.lo[1], f.hi[2])), Block::parse("minecraft:bedrock").unwrap());
        let mut craters = 0;
        for _ in 0..300 {
            s.drawn_matches_simulation().unwrap_or_else(|e| panic!("tick {}: {e}", s.tick));
            craters = craters.max(s.floor.as_ref().unwrap().changed.len());
            s.step();
        }
        assert!(s.halted.is_none(), "{:?}", s.halted);
        assert!(craters > 0, "no TNT went off on the ground");
        // the reset lays it back as it was
        s.reset();
        assert!(s.floor.as_ref().unwrap().changed.is_empty());
        let clock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-misc-dripleaf-stand.txt")).unwrap();
        let c = Session::new(&clock, &Options::default()).unwrap();
        assert!(c.floor.is_none() && crate::scene::viewer_look(&c).ground.is_none());
    }

    /// vjeux: a clock's OUT box is red while its output is powered. The dripleaf stand's big
    /// dripleaf (its tilt looks the same either way) is powered from below 2 ticks of each 4 gt
    /// cycle; its blocks repeat in 4 (it starts in slow motion). (Its `port output: 0 0 0` names
    /// the air two blocks above the leaf: ports are world coordinates, not shifted by `origin`.)
    #[test]
    fn dripleaf_output_pulses() {
        let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/clocks/archive/clk-misc-dripleaf-stand.txt")).unwrap();
        let mut s = Session::new(&text, &Options::default()).unwrap();
        s.find_auto_output(20_000);
        assert_eq!(s.short_cycle(9, 120), Some(4));
        let leaf = [0, -2, 0];
        assert!(s.design.blocks[&leaf].contains("big_dripleaf"));
        let mut seq = Vec::new();
        for _ in 0..40 {
            s.step();
            seq.push(s.output_powered(leaf));
        }
        assert_eq!(seq[20..40].iter().filter(|x| **x).count(), 10, "{seq:?}");
        assert_eq!(seq[20..24], seq[24..28], "{seq:?}");
        let o = s.outputs[0];
        let look = crate::scene::viewer_look(&s);
        assert!(look.marker.is_none() && look.port_outlines.iter().any(|(p, c)| *p == o && *c == crate::scene::OUT_COLOUR), "the OUT box, no glass");
    }

    #[test]
    fn auto_output_is_the_clock_judges() {
        let s = Session::new(BULB_CLOCK, &Options::default()).unwrap();
        let (w0, p) = s.picture_period(20_000).unwrap();
        assert_eq!((w0, p), (0, 12));
        let cells = s.period_cells(w0, p);
        assert_eq!(Session::auto_output(&cells), Some([2, 0, 0]));
    }

    #[test]
    fn the_kick_block_shows_at_tick_0() {
        // `port start: 0 0 0 last`: the observer placed last is there from the first frame
        let s = Session::new(BULB_CLOCK, &Options::default()).unwrap();
        assert_eq!(s.tick, 0);
        assert!(s.render_blocks(0.0).iter().any(|b| b.cell == [0, 0, 0] && b.state.contains("observer")));
    }

    #[test]
    fn recorded_output_states_light_the_marker() {
        assert!(active_state("minecraft:redstone_lamp[lit=true]"));
        assert!(!active_state("minecraft:redstone_wire[power=0,east=side]"));
        assert!(active_state("minecraft:redstone_wire[power=3]"));
        assert!(!active_state("minecraft:air"));
        assert!(active_state("minecraft:hopper[enabled=false,facing=down]"));
    }
}
