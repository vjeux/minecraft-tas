//! A contraption running on redsim: placed exactly like its judge (the door judge's placement
//! for doors, the clock judge's placement and start kick for clocks), stepped one game tick at a
//! time, with the player's inputs (lever flips, button presses) applied between ticks.
//!
//! The clock logic mirrors `crates/clock/src/sim.rs` (`start`, `between_ticks`, the period
//! detector) and the door logic `crates/piston-door/src/cosearch.rs` (`place_and_settle`), so the
//! browser shows what the judges measured; `mctas web check` compares the two natively and on the
//! shipped wasm.

use crate::design::{Design, Pos};
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
#[derive(Clone, Debug)]
pub struct Options {
    /// Free cells around the design in the simulated world (the clock judge uses 4).
    pub margin: i32,
    /// Ticks a door may take to settle after placement.
    pub settle: u32,
}

impl Default for Options {
    fn default() -> Self {
        Options { margin: 4, settle: 400 }
    }
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
}

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
        let design = Design::parse(text)?;
        if design.blocks.is_empty() && design.entities.is_empty() {
            return Err("empty design".into());
        }
        let mode = Mode::of(&design)?;
        let (mut lo, mut hi) = design.bounds().ok_or("empty design")?;
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
        let min = BlockPos::new(lo[0] - m, lo[1] - m, lo[2] - m);
        let size = [hi[0] - lo[0] + 1 + 2 * m, hi[1] - lo[1] + 1 + 2 * m, hi[2] - lo[2] + 1 + 2 * m];
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
        if !design.entities.is_empty() {
            unsimulated.push("entities (minecarts...) are not simulated by redsim".into());
        }
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
        };
        if !unsimulated.is_empty() {
            // Shown as built: every block the model knows placed raw, nothing ticks.
            world.place_raw(&blocks);
            s.world = world;
            s.unsimulated = Some(unsimulated.join("; "));
            return Ok(s);
        }
        if let Err(e) = s.place(&mut world, &blocks, o) {
            // e.g. container contents redsim does not model: shown as built, not simulated
            let mut w = World::new(min, size);
            w.place_raw(&blocks);
            s.world = w;
            s.unsimulated = Some(e);
            return Ok(s);
        }
        s.world = world;
        s.initial = Some((s.world.clone(), s.actions.clone(), s.tick));
        // a session at tick t has the inputs due at t applied (the judge's between-ticks order)
        s.apply_due();
        Ok(s)
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
            self.apply_due();
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
        if !self.simulated() {
            return;
        }
        self.apply_due();
        self.world.tick();
        self.tick += 1;
        self.apply_due();
    }

    /// Ticks run since the last input (or since rest / placement).
    pub fn since_input(&self) -> u32 {
        self.tick - self.last_input.unwrap_or(self.rest_tick)
    }

    /// Is this cell clickable (a lever, a button, an input port)?
    pub fn is_input(&self, p: Pos) -> bool {
        self.inputs.contains(&p)
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
        let min = w.min;
        for y in 0..w.size[1] {
            for z in 0..w.size[2] {
                for x in 0..w.size[0] {
                    let q = min.offset(x, y, z);
                    let b = w.get(q);
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
        let (mut lo, mut hi) = self.design.bounds().unwrap_or(([0, 0, 0], [0, 0, 0]));
        if let Mode::Door { door, along_z, .. } = self.mode {
            for c in door_cells(door, along_z) {
                grow(&mut lo, &mut hi, c);
            }
        }
        (lo, hi)
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

fn frame_digest(w: &World) -> u128 {
    use std::hash::Hash;
    let mut h = H128::default();
    let min = w.min;
    for y in 0..w.size[1] {
        for z in 0..w.size[2] {
            for x in 0..w.size[0] {
                let q = min.offset(x, y, z);
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
        let mut s = self.fresh();
        let mut seen: HashMap<u128, u32> = HashMap::new();
        let mut found = None;
        for t in 0..=horizon {
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
            frames.push(frame_digest(&s.world));
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
        };
        s.apply_due();
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
        let Some((w0, p)) = self.picture_period(horizon) else { return self.tick };
        let cells = self.period_cells(w0, p);
        if self.outputs.is_empty()
            && let Some(o) = Session::auto_output(&cells)
        {
            self.outputs.push(o);
        }
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

    /// Find the clock judge's output (no `port output`) without moving the session.
    pub fn find_auto_output(&mut self, horizon: u32) {
        if !self.outputs.is_empty() {
            return;
        }
        if let Some((w0, p)) = self.picture_period(horizon)
            && let Some(o) = Session::auto_output(&self.period_cells(w0, p))
        {
            self.outputs.push(o);
        }
    }
}

/// [`active`] for a block state string (a recording's cells).
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
fn is_port_named(name: &str, prefix: &str) -> bool {
    let long = if prefix == "in" { "input" } else { "output" };
    let rest = name.strip_prefix(long).or_else(|| name.strip_prefix(prefix));
    rest.is_some_and(|r| r.is_empty() || r.starts_with('_') || r.starts_with('-') || r.chars().all(|c| c.is_ascii_alphanumeric()) && r.chars().next().is_some_and(|c| c.is_ascii_digit() || c.is_ascii_uppercase()))
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
