//! `mctas door route2`: a port-aware, timing-aware net router, and the rip-up-and-re-route of a
//! design's nets with it.
//!
//! The plain router (`door route`, route.rs) lays dust between two cells. This one routes a
//! whole net. It may tap any powered block of the design (the lever, the block it hangs on, an
//! observer, a repeater, a powered floor, dust...) or watch any cell the machine changes; it
//! enters each receiver any way the game allows (a powered side, the quasi-connectivity cell
//! above a piston, a powered block next to it, dust on it or pointing into it); and it places
//! dust, repeaters (any delay), observers (a 2 gt pulse on every change of what they watch) and
//! plain blocks as conductors (glazed terracotta next to moving slime), choosing delays so that
//! every receiver sees the signal it needs, tick for tick, while nothing else gets powered.
//!
//! **Static model** ([`Model`]). Every block is a node (synth.rs's [`Node`]); a node feeds
//! another as in the game: dust powers its floor and what it points into (its shape is computed
//! like `RedstoneWireBlock.getConnectionState`), a powered block powers the components next to it
//! but dust only when strongly powered, a repeater, comparator or observer strongly powers the
//! block in front, a torch the block above it. Every node carries a [`Wave`]: its on/off state at
//! each tick of one open phase and one close phase (lever on at tick 0 of the open phase, off at
//! tick 0 of the close phase), computed by running each element's state machine (repeater delay
//! and pulse stretching, observer pulses, torch inversion, as in the game's `tick` methods) on
//! the OR of its inputs. Sources the machine itself drives (observers watching moving blocks,
//! sculk sensors, shuttled redstone blocks) get the waves measured in the simulator. A piston
//! cell's wave is the OR of what its neighbors (not its face) and the neighbors of its
//! quasi-connectivity cell emit into those cells; every cell that holds a piston at some tick of
//! the cycle is a receiver cell.
//!
//! **Equivalence.** A re-wired design is accepted by the model when every receiver cell gets the
//! same wave as in the original (up to a shift of `tolerance` ticks per cell). That one check
//! covers the required receivers, their timing windows, and the cells that must stay unpowered.
//!
//! **Search** ([`route_net`]). The receiver cells whose wave is wrong once a net is ripped are
//! connected one at a time (a beam over receiver orders and alternative hits) by a Dijkstra over
//! placements (cost: new blocks, floors included) from every tap at once; each placement's wave
//! comes from all its inputs (the parent and any powered context block next to it). New blocks
//! stay out of the cells the machine occupies at any tick, the door and the walkway; only
//! non-sticky floors go next to moving slime. Candidates the model accepts are screened and
//! judged in the simulator (search.rs `try_edits`).
//!
//! **Rip-up** ([`rip_and_reroute`]): every connected group of 2..=`max_rip` control blocks (not
//! pistons, slime, door blocks, the lever, or blocks that move) is ripped and re-routed with
//! fewer blocks.

use crate::search::{Edit, SearchOptions};
use crate::synth::{Kind, Node, drives};
use crate::{DoorSpec, block_count};
use anyhow::{Context, Result, bail};
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pool, Pos, Worker};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};
use std::sync::Mutex;

/// Ticks recorded per phase.
pub const T: usize = 64;

const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const HORIZ: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
pub const CROSS: &str = "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]";
const FLOOR: &str = "minecraft:smooth_stone";
const NONSTICK: &str = "minecraft:white_glazed_terracotta[facing=north]";

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn sub(a: Pos, b: Pos) -> Pos {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn manhattan(a: Pos, b: Pos) -> i32 {
    (0..3).map(|i| (a[i] - b[i]).abs()).sum()
}
pub fn fmt_pos(p: Pos) -> String {
    format!("({},{},{})", p[0], p[1], p[2])
}
fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}
fn bname(s: &str) -> &str {
    short(mctas::blockstate::block_name(s))
}
fn is_piston(s: &str) -> bool {
    matches!(bname(s), "piston" | "sticky_piston")
}
fn facing_of(s: &str) -> Option<Pos> {
    let b = mctas::blockstate::BlockState::parse(s).ok()?;
    Some(match b.get("facing")? {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}
fn repeater_delay(s: &str) -> usize {
    mctas::blockstate::BlockState::parse(s).ok().and_then(|b| b.get("delay").and_then(|d| d.parse().ok())).unwrap_or(1)
}

// ---- waves --------------------------------------------------------------------------------

/// A signal over one open phase and one close phase: bit t of `.0[0]` (`.0[1]`) is the state at
/// the end of tick t after the lever turns on (off). Each phase starts from the other's end.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)]
pub struct Wave(pub [u64; 2]);

impl Wave {
    pub const OFF: Wave = Wave([0, 0]);
    pub const ON: Wave = Wave([u64::MAX, u64::MAX]);
    /// The lever: on from tick 0 of the open phase, off from tick 0 of the close phase.
    pub const LEVER: Wave = Wave([u64::MAX, 0]);

    pub fn get(&self, ph: usize, t: usize) -> bool {
        (self.0[ph] >> t) & 1 == 1
    }
    pub fn or(self, o: Wave) -> Wave {
        Wave([self.0[0] | o.0[0], self.0[1] | o.0[1]])
    }
    pub fn is_off(&self) -> bool {
        self.0 == [0, 0]
    }
    /// Bits set here and not in `o`.
    pub fn minus(self, o: Wave) -> Wave {
        Wave([self.0[0] & !o.0[0], self.0[1] & !o.0[1]])
    }
    /// A warm-up of T ticks at the close phase's last value, the open phase, the close phase.
    fn seq(&self) -> Vec<bool> {
        let mut v = vec![self.get(1, T - 1); T];
        for ph in 0..2 {
            for t in 0..T {
                v.push(self.get(ph, t));
            }
        }
        v
    }
    fn from_seq(s: &[bool]) -> Wave {
        let mut w = [0u64; 2];
        for (ph, x) in w.iter_mut().enumerate() {
            for t in 0..T {
                if s[T * (ph + 1) + t] {
                    *x |= 1 << t;
                }
            }
        }
        Wave(w)
    }
    /// Later by `k` ticks (earlier if negative).
    pub fn shift(&self, k: i32) -> Wave {
        let s = self.seq();
        let n = s.len() as i32;
        let out: Vec<bool> = (0..n).map(|i| s[(i - k).clamp(0, n - 1) as usize]).collect();
        Wave::from_seq(&out)
    }
    /// "open 4-7, close 4-7": the on ticks of each phase.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        for (ph, name) in [(0, "open"), (1, "close")] {
            let mut runs = Vec::new();
            let mut t = 0;
            while t < T {
                if self.get(ph, t) {
                    let s = t;
                    while t < T && self.get(ph, t) {
                        t += 1;
                    }
                    runs.push(if t == T {
                        format!("{s}-")
                    } else if t - 1 == s {
                        format!("{s}")
                    } else {
                        format!("{s}-{}", t - 1)
                    });
                } else {
                    t += 1;
                }
            }
            parts.push(format!("{name} {}", if runs.is_empty() { "off".to_string() } else { runs.join(",") }));
        }
        parts.join(", ")
    }
    /// `on`, `off`, `level+D` (the lever's level D ticks later), `pulse+D/L` (an L-tick pulse D
    /// ticks after each flip), or `open:A-B,C;close:D-` (explicit on ticks, `A-` = to the end).
    pub fn parse(s: &str) -> Result<Wave> {
        let s = s.trim();
        if s == "on" {
            return Ok(Wave::ON);
        }
        if s == "off" {
            return Ok(Wave::OFF);
        }
        if let Some(d) = s.strip_prefix("level+") {
            return Ok(Wave::LEVER.shift(d.parse()?));
        }
        if let Some(r) = s.strip_prefix("pulse+") {
            let (d, l) = r.split_once('/').context("pulse+D/L")?;
            let (d, l): (usize, usize) = (d.parse()?, l.parse()?);
            let mut w = [0u64; 2];
            for x in w.iter_mut() {
                for t in d..(d + l).min(T) {
                    *x |= 1 << t;
                }
            }
            return Ok(Wave(w));
        }
        let mut w = [0u64; 2];
        for part in s.split(';') {
            let (ph, runs) = part.split_once(':').context("expected open:... or close:...")?;
            let i = match ph.trim() {
                "open" => 0,
                "close" => 1,
                x => bail!("unknown phase {x:?}"),
            };
            for r in runs.split(',').map(str::trim).filter(|r| !r.is_empty() && *r != "off") {
                let (a, b) = match r.split_once('-') {
                    Some((a, "")) => (a.parse::<usize>()?, T - 1),
                    Some((a, b)) => (a.parse::<usize>()?, b.parse::<usize>()?),
                    None => (r.parse::<usize>()?, r.parse::<usize>()?),
                };
                for t in a..=b.min(T - 1) {
                    w[i] |= 1 << t;
                }
            }
        }
        Ok(Wave(w))
    }
}

/// The same wave up to a shift of at most `tol` ticks.
pub fn same_wave(a: Wave, b: Wave, tol: i32) -> bool {
    a == b || (tol > 0 && (-tol..=tol).any(|k| a.shift(k) == b))
}

fn changes(s: &[bool]) -> Vec<bool> {
    (0..s.len()).map(|t| t > 0 && s[t] != s[t - 1]).collect()
}

/// `ObserverBlock`: a change schedules a tick 2 gt later (unless powered or already scheduled);
/// the tick turns it on for 2 gt.
fn observer_seq(ch: &[bool]) -> Vec<bool> {
    let mut out = vec![false; ch.len()];
    let (mut on, mut pending) = (false, None::<usize>);
    for t in 0..ch.len() {
        if pending == Some(t) {
            pending = None;
            if on {
                on = false;
            } else {
                on = true;
                pending = Some(t + 2);
            }
        }
        if ch[t] && !on && pending.is_none() {
            pending = Some(t + 2);
        }
        out[t] = on;
    }
    out
}

/// `DiodeBlock` (repeater, delay `k` gt): input changes schedule a tick k later; a pulse shorter
/// than k is stretched to k.
fn repeater_seq(s: &[bool], k: usize) -> Vec<bool> {
    let mut out = vec![false; s.len()];
    let (mut on, mut pending) = (s[0], None::<usize>);
    for t in 0..s.len() {
        let before = if t > 0 { s[t - 1] } else { s[0] };
        if pending == Some(t) {
            pending = None;
            if on && !before {
                on = false;
            } else if !on {
                on = true;
                if !before {
                    pending = Some(t + k);
                }
            }
        }
        if t > 0 && s[t] != s[t - 1] && on != s[t] && pending.is_none() {
            pending = Some(t + k);
        }
        out[t] = on;
    }
    out
}

/// `RedstoneTorchBlock`: inverts its block's power 2 gt later.
fn torch_seq(s: &[bool]) -> Vec<bool> {
    let mut out = vec![false; s.len()];
    let (mut lit, mut pending) = (!s[0], None::<usize>);
    for t in 0..s.len() {
        let before = if t > 0 { s[t - 1] } else { s[0] };
        if pending == Some(t) {
            pending = None;
            if lit && before {
                lit = false;
            } else if !lit && !before {
                lit = true;
            }
        }
        if t > 0 && s[t] != s[t - 1] && lit == s[t] && pending.is_none() {
            pending = Some(t + 2);
        }
        out[t] = lit;
    }
    out
}

/// A node's output wave from the OR of its inputs.
fn transfer(n: &Node, input: Wave) -> Wave {
    match n.kind {
        Kind::Repeater => Wave::from_seq(&repeater_seq(&input.seq(), 2 * repeater_delay(&n.state))),
        Kind::Observer => Wave::from_seq(&observer_seq(&changes(&input.seq()))),
        Kind::Torch => Wave::from_seq(&torch_seq(&input.seq())),
        Kind::Comparator => input.shift(2),
        Kind::Dust | Kind::Lamp | Kind::Block => input,
        _ => Wave::OFF,
    }
}

// ---- static model -------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Feed {
    /// The reader takes the feeder's output.
    Any,
    /// Dust next to (or on) a block: only the block's strong power.
    StrongOf,
    /// Dust into a block: weak power.
    Weak,
    /// A diode, observer, torch or lever into a block: strong power.
    Strong,
}

/// Is the block at `p` a redstone conductor (full opaque block)?
fn conductor(blocks: &BTreeMap<Pos, String>, p: Pos) -> bool {
    blocks.get(&p).is_some_and(|s| Node::new(p, s).kind == Kind::Block)
}

/// Does dust at `n` see something to connect to (`shouldRedstoneWireConnectTo`) coming from
/// direction `dir` (None: from above/below)?
fn wire_connects(blocks: &BTreeMap<Pos, String>, n: Pos, dir: Option<Pos>) -> bool {
    let Some(s) = blocks.get(&n) else { return false };
    match bname(s) {
        "redstone_wire" => true,
        "repeater" => dir.is_some_and(|d| facing_of(s).is_some_and(|f| f == d || f == [-d[0], -d[1], -d[2]])),
        "observer" => dir.is_some_and(|d| facing_of(s) == Some(d)),
        "lever" | "redstone_torch" | "redstone_wall_torch" | "redstone_block" | "comparator" | "target" | "daylight_detector"
        | "sculk_sensor" | "calibrated_sculk_sensor" => dir.is_some(),
        x if x.ends_with("_button") || x.ends_with("_pressure_plate") => dir.is_some(),
        _ => false,
    }
}

/// Which horizontal sides (in HORIZ order) dust at `p` points into, as the game shapes it.
fn dust_points(blocks: &BTreeMap<Pos, String>, p: Pos) -> [bool; 4] {
    let can_up = !conductor(blocks, add(p, [0, 1, 0]));
    let mut conn = [false; 4];
    for (i, h) in HORIZ.iter().enumerate() {
        let n = add(p, *h);
        let sturdy_n = blocks.get(&n).is_some_and(|s| Node::new(n, s).sturdy);
        if can_up && sturdy_n && wire_connects(blocks, add(n, [0, 1, 0]), None) {
            conn[i] = true;
            continue;
        }
        conn[i] = wire_connects(blocks, n, Some(*h)) || (!conductor(blocks, n) && wire_connects(blocks, add(n, [0, -1, 0]), None));
    }
    let was_dot = blocks.get(&p).is_some_and(|s| !s.contains("=side") && !s.contains("=up"));
    if was_dot && !conn.iter().any(|c| *c) {
        return [false; 4];
    }
    // HORIZ = E, W, S, N.
    let ns_empty = !conn[2] && !conn[3];
    let ew_empty = !conn[0] && !conn[1];
    [conn[0] || ns_empty, conn[1] || ns_empty, conn[2] || ew_empty, conn[3] || ew_empty]
}

/// Does node `u` (with dust shape `pts`) emit power into the cell `c` (for a piston there)?
fn emits_into(u: &Node, pts: Option<[bool; 4]>, c: Pos) -> bool {
    let d = sub(c, u.pos);
    if manhattan(c, u.pos) != 1 {
        return false;
    }
    match u.kind {
        Kind::Dust => {
            d[1] == -1 || (d[1] == 0 && HORIZ.iter().position(|h| *h == d).is_some_and(|i| pts.is_none_or(|p| p[i])))
        }
        Kind::Block | Kind::RBlock | Kind::Lever => true,
        Kind::Repeater | Kind::Comparator | Kind::Observer => c == sub(u.pos, u.facing),
        Kind::Torch => Some(c) != u.support,
        _ => false,
    }
}

/// How `u` feeds `v` (None: it does not).
fn feed(blocks: &BTreeMap<Pos, String>, u: &Node, upts: Option<[bool; 4]>, v: &Node) -> Option<Feed> {
    if !drives(u, v) {
        return None;
    }
    let dy = v.pos[1] - u.pos[1];
    if u.kind == Kind::Dust && v.kind == Kind::Dust {
        if dy == 1 && conductor(blocks, add(u.pos, [0, 1, 0])) {
            return None;
        }
        if dy == -1 && conductor(blocks, add(v.pos, [0, 1, 0])) {
            return None;
        }
    }
    // Dust powers sideways only where it points.
    if u.kind == Kind::Dust && v.kind != Kind::Dust && v.kind != Kind::Observer && dy == 0 {
        let d = sub(v.pos, u.pos);
        if let (Some(i), Some(p)) = (HORIZ.iter().position(|h| *h == d), upts) {
            if !p[i] {
                return None;
            }
        }
    }
    Some(match (u.kind, v.kind) {
        (Kind::Block, Kind::Dust) => Feed::StrongOf,
        (Kind::Dust, Kind::Block) => Feed::Weak,
        (_, Kind::Block) => Feed::Strong,
        _ => Feed::Any,
    })
}

/// The design's control logic as a graph with a wave per node.
pub struct Model {
    pub blocks: BTreeMap<Pos, String>,
    pub nodes: Vec<Node>,
    pub at: HashMap<Pos, usize>,
    pts: HashMap<Pos, [bool; 4]>,
    inputs: Vec<Vec<(usize, Feed)>>,
    fixed: HashMap<usize, Wave>,
    pub any: Vec<Wave>,
    pub strong: Vec<Wave>,
}

impl Model {
    /// `fixed`: measured waves of sources the machine drives (by cell); `extra`: source blocks
    /// that are not there at rest (shuttled redstone blocks: their occupancy waves).
    pub fn new(blocks: &BTreeMap<Pos, String>, lever: Option<Pos>, fixed: &HashMap<Pos, Wave>, extra: &[(Pos, String, Wave)]) -> Model {
        let mut nodes = Vec::new();
        let mut at = HashMap::new();
        let mut fx = HashMap::new();
        for (p, s) in blocks {
            at.insert(*p, nodes.len());
            nodes.push(Node::new(*p, s));
        }
        let mut all = blocks.clone();
        for (p, s, w) in extra {
            if let Some(&i) = at.get(p) {
                fx.insert(i, *w);
                continue;
            }
            at.insert(*p, nodes.len());
            fx.insert(nodes.len(), *w);
            nodes.push(Node::new(*p, s));
            all.insert(*p, s.clone());
        }
        for (i, n) in nodes.iter().enumerate() {
            if n.kind == Kind::RBlock && !fx.contains_key(&i) {
                fx.insert(i, Wave::ON);
            }
        }
        if let Some(l) = lever {
            if let Some(&i) = at.get(&l) {
                fx.insert(i, Wave::LEVER);
            }
        }
        for (p, w) in fixed {
            if let Some(&i) = at.get(p) {
                fx.insert(i, *w);
            }
        }
        let pts: HashMap<Pos, [bool; 4]> = nodes.iter().filter(|n| n.kind == Kind::Dust).map(|n| (n.pos, dust_points(&all, n.pos))).collect();
        let mut inputs = vec![Vec::new(); nodes.len()];
        for (vi, v) in nodes.iter().enumerate() {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        if dx == 0 && dy == 0 && dz == 0 {
                            continue;
                        }
                        let q = add(v.pos, [dx, dy, dz]);
                        let Some(&ui) = at.get(&q) else { continue };
                        let u = &nodes[ui];
                        if let Some(f) = feed(&all, u, pts.get(&q).copied(), v) {
                            inputs[vi].push((ui, f));
                        }
                    }
                }
            }
        }
        let n = nodes.len();
        let mut m = Model { blocks: all, nodes, at, pts, inputs, fixed: fx, any: vec![Wave::OFF; n], strong: vec![Wave::OFF; n] };
        m.evaluate();
        m
    }

    fn evaluate(&mut self) {
        let n = self.nodes.len();
        for (&i, w) in &self.fixed {
            self.any[i] = *w;
            self.strong[i] = *w;
        }
        for _ in 0..60 {
            let mut changed = false;
            for v in 0..n {
                if self.fixed.contains_key(&v) {
                    continue;
                }
                let (mut a, mut s) = (Wave::OFF, Wave::OFF);
                for &(u, f) in &self.inputs[v] {
                    match f {
                        Feed::Any | Feed::Weak => a = a.or(self.any[u]),
                        Feed::StrongOf => a = a.or(self.strong[u]),
                        Feed::Strong => {
                            a = a.or(self.any[u]);
                            s = s.or(self.any[u]);
                        }
                    }
                }
                let node = &self.nodes[v];
                let (na, ns) = match node.kind {
                    Kind::Block => (a, s),
                    _ => {
                        let w = transfer(node, a);
                        (w, w)
                    }
                };
                if na != self.any[v] || ns != self.strong[v] {
                    self.any[v] = na;
                    self.strong[v] = ns;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    /// The signal a piston facing `facing` gets at cell `p`.
    pub fn piston_wave(&self, p: Pos, facing: Pos) -> Wave {
        let mut w = Wave::OFF;
        let up = add(p, [0, 1, 0]);
        for d in FACES {
            let q = add(p, d);
            if d != facing {
                if let Some(&i) = self.at.get(&q) {
                    if emits_into(&self.nodes[i], self.pts.get(&q).copied(), p) {
                        w = w.or(self.any[i]);
                    }
                }
            }
            let r = add(up, d);
            if r != p {
                if let Some(&i) = self.at.get(&r) {
                    if emits_into(&self.nodes[i], self.pts.get(&r).copied(), up) {
                        w = w.or(self.any[i]);
                    }
                }
            }
        }
        w
    }

    pub fn wave_at(&self, p: Pos) -> Option<Wave> {
        self.at.get(&p).map(|&i| self.any[i])
    }
}

// ---- measurement --------------------------------------------------------------------------

/// What one simulated cycle of the original design tells the router.
#[derive(Clone, Default)]
pub struct Measured {
    /// Cells holding a piston at some tick, with its facing.
    pub pistons: Vec<(Pos, Pos)>,
    /// Waves of sources the machine drives (observers watching non-control cells, sculk
    /// sensors), by cell.
    pub fixed: HashMap<Pos, Wave>,
    /// Shuttled redstone blocks: (cell, state, occupancy).
    pub extra: Vec<(Pos, String, Wave)>,
    /// Cells non-air at some tick but not at rest... and at rest (all cells the machine uses).
    pub occupied: HashSet<Pos>,
    /// Cells whose block changes during the cycle.
    pub moving: HashSet<Pos>,
    /// Free cells next to slime/honey/moving blocks at some tick (only non-sticky floors there).
    pub drag: HashSet<Pos>,
    /// For each cell the machine changes: the wave an observer watching it would emit.
    pub watch: HashMap<Pos, Wave>,
    /// Measured on/off waves of every control component (for calibration).
    pub measured: HashMap<Pos, Wave>,
    /// The phases went idle within T ticks.
    pub idle: bool,
}

fn place_settled(w: &mut Worker, sb: u32, d: &Design) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    if w.step_until_idle(sb, 400)?.idle != Some(true) {
        bail!("never settles");
    }
    Ok(())
}

/// On/off of a component state (None: not a component we track).
fn on_value(s: &str) -> Option<bool> {
    let b = mctas::blockstate::BlockState::parse(s).ok()?;
    let flag = |k: &str| b.get(k) == Some("true");
    Some(match b.short_name() {
        "redstone_wire" => b.get("power").is_some_and(|p| p != "0"),
        "repeater" | "comparator" | "observer" | "lever" => flag("powered"),
        "redstone_torch" | "redstone_wall_torch" | "redstone_lamp" => flag("lit"),
        "sculk_sensor" | "calibrated_sculk_sensor" | "target" => b.get("power").is_some_and(|p| p != "0"),
        _ => return None,
    })
}

pub fn measure(w: &mut Worker, d: &Design) -> Result<Measured> {
    let spec = DoorSpec::from_design(d)?;
    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Measured> {
        place_settled(w, sb, d)?;
        let (lo, hi) = d.bounds().context("empty design")?;
        let (lo, hi) = ([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]);
        let rest = w.get_region(sb, lo, hi)?;
        // frames[ph][t]: the region at the end of tick t of phase ph (t = 0: right after the flip).
        let mut frames: Vec<Vec<mctas::sim::Region>> = Vec::new();
        let mut idle = true;
        for _ in 0..2 {
            w.use_block(sb, spec.lever)?;
            let mut f = vec![w.get_region(sb, lo, hi)?];
            let r = w.step_probe(sb, (T - 1) as u32, false, &[], Some((lo, hi)))?;
            f.extend(r.frames);
            idle &= w.status(sb)?.idle;
            frames.push(f);
        }
        let mut m = Measured { idle, ..Default::default() };
        let cells: Vec<Pos> = rest.iter().map(|(p, _)| p).collect();
        let state = |ph: usize, t: usize, p: Pos| -> &str { frames[ph][t.min(frames[ph].len() - 1)].get(p).unwrap_or("minecraft:air") };
        let mut pistons: BTreeMap<Pos, Pos> = BTreeMap::new();
        let mut near: HashSet<Pos> = HashSet::new();
        for &p in &cells {
            let r0 = rest.get(p).unwrap_or("minecraft:air");
            let mut moved = false;
            let mut rb = Wave::OFF;
            let mut chg = [0u64; 2];
            let mut prev = frames[1][T - 1].get(p).unwrap_or("minecraft:air").to_string();
            for ph in 0..2 {
                for t in 0..T {
                    let s = state(ph, t, p);
                    if !mctas::blockstate::is_air(s) {
                        m.occupied.insert(p);
                    }
                    if bname(s) != bname(r0) {
                        moved = true;
                    }
                    if s != prev {
                        chg[ph] |= 1 << t;
                    }
                    prev = s.to_string();
                    if is_piston(s) {
                        if let Some(f) = facing_of(s) {
                            pistons.insert(p, f);
                        }
                    }
                    if bname(s) == "redstone_block" {
                        rb.0[ph] |= 1 << t;
                    }
                    if matches!(bname(s), "slime_block" | "honey_block") {
                        for f in FACES {
                            near.insert(add(p, f));
                        }
                    }
                }
            }
            if !mctas::blockstate::is_air(r0) {
                m.occupied.insert(p);
            }
            if moved {
                m.moving.insert(p);
            }
            if !rb.is_off() && (moved || bname(r0) != "redstone_block") {
                m.extra.push((p, "minecraft:redstone_block".to_string(), rb));
            }
            // Observers may watch what the machine changes (moving blocks, pistons), never a
            // control component (it may be the one being ripped).
            if (moved || on_value(r0).is_none()) && chg != [0, 0] {
                let mut ch = vec![false; T];
                for ph in 0..2 {
                    for t in 0..T {
                        ch.push(chg[ph] >> t & 1 == 1);
                    }
                }
                m.watch.insert(p, Wave::from_seq(&observer_seq(&ch)));
            }
            if on_value(r0).is_some() {
                let mut wv = Wave::OFF;
                for ph in 0..2 {
                    for t in 0..T {
                        if on_value(state(ph, t, p)) == Some(true) {
                            wv.0[ph] |= 1 << t;
                        }
                    }
                }
                m.measured.insert(p, wv);
            }
        }
        m.drag = near.difference(&m.moving).copied().collect();
        m.pistons = pistons.into_iter().collect();
        // Sources the machine drives: observers watching a non-control cell, sculk sensors.
        for (p, s) in &d.blocks {
            let watched_control = |q: Pos| {
                d.blocks.get(&q).is_some_and(|x| matches!(Node::new(q, x).kind, Kind::Dust | Kind::Repeater | Kind::Comparator | Kind::Observer | Kind::Torch | Kind::Lamp | Kind::Lever))
            };
            let src = match bname(s) {
                "observer" => facing_of(s).is_some_and(|f| !watched_control(add(*p, f))),
                "sculk_sensor" | "calibrated_sculk_sensor" | "daylight_detector" | "target" => true,
                _ => false,
            };
            if src {
                if let Some(wv) = m.measured.get(p) {
                    m.fixed.insert(*p, *wv);
                }
            }
        }
        Ok(m)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

// ---- the router ---------------------------------------------------------------------------

/// A receiver cell: a piston (facing `facing`) at `pos` at some tick must get `want`; what the
/// rest of the design already gives it is `rest`.
#[derive(Clone, Debug)]
pub struct Receiver {
    pub pos: Pos,
    pub facing: Pos,
    pub want: Wave,
    pub rest: Wave,
}

/// Router knobs.
#[derive(Clone)]
pub struct RouteOptions {
    /// Block states the router may place (expanded palette).
    pub palette: Vec<String>,
    pub tolerance: i32,
    /// Partial solutions kept per receiver step, and hits taken per receiver per partial solution.
    pub beam: usize,
    pub hits: usize,
    /// Dijkstra states expanded per search.
    pub max_states: usize,
    /// Routing box margin around the design.
    pub margin: i32,
    /// Only tap these cells (empty: any powered node or watched cell).
    pub taps: Vec<Pos>,
    /// Allow watching cells the machine changes (observers on moving blocks).
    pub watch: bool,
}

pub fn palette(spec: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match tok {
            "dust" => out.push(CROSS.to_string()),
            "block" => out.push(FLOOR.to_string()),
            "gt" => out.push(NONSTICK.to_string()),
            "obs" => out.extend(["north", "east", "south", "west", "up", "down"].iter().map(|f| format!("minecraft:observer[facing={f},powered=false]"))),
            "rep" => {
                for d in 1..=4 {
                    out.extend(["north", "east", "south", "west"].iter().map(|f| format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]")));
                }
            }
            "torch" => {
                out.push("minecraft:redstone_torch[lit=true]".into());
                out.extend(["north", "east", "south", "west"].iter().map(|f| format!("minecraft:redstone_wall_torch[facing={f},lit=true]")));
            }
            s if s.starts_with("minecraft:") => out.push(s.to_string()),
            s => bail!("unknown palette entry {s:?} (dust, block, gt, obs, rep, torch or a full state)"),
        }
    }
    Ok(out)
}

/// The fixed context of one routing problem.
pub struct Problem<'a> {
    pub base: &'a Design,
    pub meas: &'a Measured,
    pub lever: Option<Pos>,
    /// Where nothing new may go (door, walkway).
    pub forbidden: HashSet<Pos>,
    pub lo: Pos,
    pub hi: Pos,
}

#[derive(Clone)]
struct SNode {
    node: Node,
    any: Wave,
    strong: Wave,
    strength: u8,
    cost: u32,
    parent: usize,
    /// Blocks this step placed (the element, its floor).
    placed: Vec<(Pos, String)>,
}

const ROOT: usize = usize::MAX;

impl Problem<'_> {
    fn model(&self, blocks: &BTreeMap<Pos, String>) -> Model {
        Model::new(blocks, self.lever, &self.meas.fixed, &self.meas.extra)
    }

    /// Wave per receiver cell (every cell a piston occupies at some tick).
    pub fn receiver_waves(&self, m: &Model) -> Vec<(Pos, Pos, Wave)> {
        self.meas.pistons.iter().map(|&(p, f)| (p, f, m.piston_wave(p, f))).collect()
    }

    fn free(&self, blocks: &BTreeMap<Pos, String>, path: &HashMap<Pos, String>, c: Pos) -> bool {
        (0..3).all(|i| c[i] >= self.lo[i] && c[i] <= self.hi[i])
            && !blocks.contains_key(&c)
            && !path.contains_key(&c)
            && !self.forbidden.contains(&c)
            && !self.meas.moving.contains(&c)
    }

    /// Cheapest placements (up to `opts.hits` alternatives) after which `r` gets its wave, on top
    /// of `blocks`; each alternative is the list of new blocks.
    fn search(&self, blocks: &BTreeMap<Pos, String>, r: &Receiver, budget: u32, opts: &RouteOptions) -> Vec<Vec<(Pos, String)>> {
        let m = self.model(blocks);
        let mut arena: Vec<SNode> = Vec::new();
        let mut heap: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();
        let mut best: HashMap<(Pos, String, Wave, Wave), u32> = HashMap::new();
        // Taps: every powered node, and observers watching any cell the machine changes.
        for (i, n) in m.nodes.iter().enumerate() {
            if m.any[i].is_off() || matches!(n.kind, Kind::Inert | Kind::Other) {
                continue;
            }
            if !opts.taps.is_empty() && !opts.taps.contains(&n.pos) {
                continue;
            }
            let strength = if n.kind == Kind::Dust { 15 } else { 15 };
            arena.push(SNode { node: n.clone(), any: m.any[i], strong: m.strong[i], strength, cost: 0, parent: ROOT, placed: Vec::new() });
            heap.push(Reverse((0, arena.len() - 1)));
        }
        let mut found: Vec<Vec<(Pos, String)>> = Vec::new();
        let mut seen_sets: HashSet<Vec<(Pos, String)>> = HashSet::new();
        let path_of = |arena: &Vec<SNode>, mut i: usize| -> Vec<(Pos, String)> {
            let mut out = Vec::new();
            while i != ROOT {
                out.extend(arena[i].placed.iter().cloned());
                i = arena[i].parent;
            }
            out
        };
        // Watch taps: an observer watching a cell the machine changes.
        if opts.watch {
            for (&c, &wv) in &self.meas.watch {
                if wv.is_off() || (!opts.taps.is_empty() && !opts.taps.contains(&c)) {
                    continue;
                }
                for f in FACES {
                    let o = sub(c, f);
                    // observer at o facing toward c (facing = f).
                    if !self.free(blocks, &HashMap::new(), o) || self.meas.drag.contains(&o) {
                        continue;
                    }
                    let fname = match f {
                        [1, 0, 0] => "east",
                        [-1, 0, 0] => "west",
                        [0, 1, 0] => "up",
                        [0, -1, 0] => "down",
                        [0, 0, 1] => "south",
                        _ => "north",
                    };
                    let s = format!("minecraft:observer[facing={fname},powered=false]");
                    if !opts.palette.contains(&s) {
                        continue;
                    }
                    let node = Node::new(o, &s);
                    arena.push(SNode { node, any: wv, strong: wv, strength: 15, cost: 1, parent: ROOT, placed: vec![(o, s)] });
                    heap.push(Reverse((1, arena.len() - 1)));
                }
            }
        }
        let mut expanded = 0usize;
        while let Some(Reverse((cost, ui))) = heap.pop() {
            if cost > budget || found.len() >= opts.hits || expanded >= opts.max_states {
                break;
            }
            expanded += 1;
            let u = arena[ui].clone();
            // Path blocks so far.
            let placed = path_of(&arena, ui);
            let path: HashMap<Pos, String> = placed.iter().cloned().collect();
            // Hit?
            let mut all = blocks.clone();
            for (p, s) in &placed {
                all.insert(*p, s.clone());
            }
            let pts = (u.node.kind == Kind::Dust).then(|| dust_points(&all, u.node.pos));
            let up = add(r.pos, [0, 1, 0]);
            let hits_r = (manhattan(u.node.pos, r.pos) == 1 && sub(u.node.pos, r.pos) != r.facing && emits_into(&u.node, pts, r.pos))
                || (u.node.pos != r.pos && manhattan(u.node.pos, up) == 1 && emits_into(&u.node, pts, up));
            if std::env::var("MCTAS_ROUTE2_DEBUG").is_ok() && cost <= 2 {
                eprintln!("pop cost {cost} {} {} any [{}] hits_r {hits_r} rest [{}] want [{}]", fmt_pos(u.node.pos), short(&u.node.state), u.any.describe(), r.rest.describe(), r.want.describe());
            }
            if hits_r && same_wave(r.rest.or(u.any), r.want, opts.tolerance) && !placed.is_empty() {
                let mut key = placed.clone();
                key.sort();
                if seen_sets.insert(key) {
                    found.push(placed.clone());
                }
                continue;
            }
            // Expansions.
            let mut cells: Vec<Pos> = u.node.em.clone();
            if u.node.changes {
                cells.extend(FACES.iter().map(|f| add(u.node.pos, *f)));
            }
            cells.sort();
            cells.dedup();
            for c in cells {
                if !self.free(blocks, &path, c) {
                    // A context or path block already there: a conductor the tap powers is a relay.
                    if let Some(s) = all.get(&c) {
                        let v = Node::new(c, s);
                        if v.kind == Kind::Block && !path.contains_key(&c) {
                            if let Some(f) = feed(&all, &u.node, pts, &v) {
                                let (a, st) = match f {
                                    Feed::Strong => (u.any, u.any),
                                    _ => (u.any, Wave::OFF),
                                };
                                let i = m.at.get(&c).copied();
                                let a = i.map_or(a, |i| a.or(m.any[i]));
                                let st = i.map_or(st, |i| st.or(m.strong[i]));
                                let key = (c, s.clone(), a, st);
                                if best.get(&key).is_none_or(|&b| cost < b) {
                                    best.insert(key, cost);
                                    arena.push(SNode { node: v, any: a, strong: st, strength: 15, cost, parent: ui, placed: Vec::new() });
                                    heap.push(Reverse((cost, arena.len() - 1)));
                                }
                            }
                        }
                    }
                    continue;
                }
                let in_drag = self.meas.drag.contains(&c);
                if std::env::var("MCTAS_ROUTE2_DEBUG").is_ok() && cost == 0 {
                    eprintln!("  expand {} -> cell {} drag {in_drag}", fmt_pos(u.node.pos), fmt_pos(c));
                }
                for s in &opts.palette {
                    let v = Node::new(c, s);
                    if in_drag && !(v.kind == Kind::Block && s.contains("glazed")) {
                        continue;
                    }
                    let Some(f) = feed(&all, &u.node, pts, &v) else { continue };
                    if u.node.kind == Kind::Block && v.kind == Kind::Dust && u.strong.is_off() {
                        continue;
                    }
                    let strength = match (u.node.kind, v.kind) {
                        (Kind::Dust, Kind::Dust) => u.strength.saturating_sub(1),
                        _ => 15,
                    };
                    if v.kind == Kind::Dust && strength == 0 {
                        continue;
                    }
                    // Support.
                    let mut add_cost = 1;
                    let mut step: Vec<(Pos, String)> = vec![(c, s.clone())];
                    if let Some(sc) = v.support {
                        let ok_ctx = all.get(&sc).is_some_and(|x| Node::new(sc, x).sturdy) && !self.meas.moving.contains(&sc);
                        if !ok_ctx {
                            if !self.free(blocks, &path, sc) {
                                continue;
                            }
                            let fl = if self.meas.drag.contains(&sc) { NONSTICK } else { FLOOR };
                            step.push((sc, fl.to_string()));
                            add_cost += 1;
                        }
                    }
                    if cost + add_cost > budget {
                        continue;
                    }
                    // Its input: the parent plus anything powered next to it.
                    let mut with = all.clone();
                    for (p, x) in &step {
                        with.insert(*p, x.clone());
                    }
                    let contrib = |w: Wave, st: Wave, f: Feed| -> (Wave, Wave) {
                        match f {
                            Feed::Any | Feed::Weak => (w, Wave::OFF),
                            Feed::StrongOf => (st, Wave::OFF),
                            Feed::Strong => (w, w),
                        }
                    };
                    let (mut a, mut st) = contrib(u.any, u.strong, f);
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            for dz in -1..=1 {
                                let q = add(c, [dx, dy, dz]);
                                if q == c || q == u.node.pos {
                                    continue;
                                }
                                if let Some(&i) = m.at.get(&q) {
                                    if m.any[i].is_off() {
                                        continue;
                                    }
                                    let qp = (m.nodes[i].kind == Kind::Dust).then(|| dust_points(&with, q));
                                    if let Some(g) = feed(&with, &m.nodes[i], qp, &v) {
                                        let (x, y) = contrib(m.any[i], m.strong[i], g);
                                        a = a.or(x);
                                        st = st.or(y);
                                    }
                                }
                            }
                        }
                    }
                    let (na, ns) = if v.kind == Kind::Block { (a, st) } else { (transfer(&v, a), transfer(&v, a)) };
                    if na.is_off() {
                        continue;
                    }
                    let key = (c, s.clone(), na, ns);
                    let nc = cost + add_cost;
                    if best.get(&key).is_none_or(|&b| nc < b) {
                        best.insert(key, nc);
                        // A new floor under dust is a powered block too (a relay).
                        let floor = step.get(1).cloned();
                        arena.push(SNode { node: v.clone(), any: na, strong: ns, strength, cost: nc, parent: ui, placed: step });
                        let vi = arena.len() - 1;
                        heap.push(Reverse((nc, vi)));
                        if let (Some((fp, fs)), Kind::Dust) = (floor, v.kind) {
                            let fnode = Node::new(fp, &fs);
                            arena.push(SNode { node: fnode, any: na, strong: Wave::OFF, strength: 15, cost: nc, parent: vi, placed: Vec::new() });
                            heap.push(Reverse((nc, arena.len() - 1)));
                        }
                    }
                }
            }
        }
        found
    }
}

/// Route the receivers on top of `base` (beam over orders and alternatives); returns candidate
/// block lists with fewer than `budget + 1` blocks, cheapest first.
pub fn route_net(pb: &Problem, receivers: &[Receiver], budget: u32, opts: &RouteOptions) -> Vec<Vec<(Pos, String)>> {
    // Partial solutions: (placed blocks, receivers left).
    let mut beam: Vec<(Vec<(Pos, String)>, Vec<usize>)> = vec![(Vec::new(), (0..receivers.len()).collect())];
    let mut done: Vec<Vec<(Pos, String)>> = Vec::new();
    let mut seen: HashSet<Vec<(Pos, String)>> = HashSet::new();
    for _ in 0..receivers.len() {
        let mut next: Vec<(Vec<(Pos, String)>, Vec<usize>)> = Vec::new();
        for (placed, left) in &beam {
            let mut blocks = pb.base.blocks.clone();
            for (p, s) in placed {
                blocks.insert(*p, s.clone());
            }
            let m = pb.model(&blocks);
            for &ri in left {
                let r = &receivers[ri];
                let now = m.piston_wave(r.pos, r.facing);
                let rest: Vec<usize> = left.iter().copied().filter(|x| *x != ri).collect();
                if same_wave(now, r.want, opts.tolerance) {
                    next.push((placed.clone(), rest));
                    continue;
                }
                let r2 = Receiver { rest: now, ..r.clone() };
                let left_budget = budget.saturating_sub(placed.len() as u32);
                for alt in pb.search(&blocks, &r2, left_budget, opts) {
                    let mut p2 = placed.clone();
                    p2.extend(alt);
                    if p2.len() as u32 <= budget {
                        next.push((p2, rest.clone()));
                    }
                }
            }
        }
        next.sort_by_key(|(p, l)| (l.len(), p.len()));
        let mut kept = Vec::new();
        let mut keys: HashSet<(Vec<(Pos, String)>, Vec<usize>)> = HashSet::new();
        for (p, l) in next {
            let mut k = p.clone();
            k.sort();
            let mut lk = l.clone();
            lk.sort();
            if !keys.insert((k.clone(), lk)) {
                continue;
            }
            if l.is_empty() {
                if seen.insert(k) {
                    done.push(p);
                }
                continue;
            }
            if kept.len() < opts.beam {
                kept.push((p, l));
            }
        }
        beam = kept;
        if beam.is_empty() {
            break;
        }
    }
    done.sort_by_key(|p| p.len());
    done
}

// ---- rip-up and re-route ------------------------------------------------------------------

/// The judge's door cells and walkway, where nothing new goes.
fn forbidden_cells(d: &Design) -> Result<HashSet<Pos>> {
    let spec = DoorSpec::from_design(d)?;
    Ok(spec.door_cells().into_iter().chain(spec.corridor_cells()).collect())
}

/// Blocks a rip may take: wiring and control parts, not the machine.
fn control_blocks(d: &Design, meas: &Measured) -> Result<Vec<Pos>> {
    let spec = DoorSpec::from_design(d)?;
    let door: HashSet<Pos> = spec.door_cells().into_iter().collect();
    Ok(d.blocks
        .iter()
        .filter(|(p, s)| {
            !door.contains(*p)
                && **p != spec.lever
                && !meas.moving.contains(*p)
                && !matches!(bname(s), "piston" | "sticky_piston" | "piston_head" | "moving_piston" | "slime_block" | "honey_block")
        })
        .map(|(p, _)| *p)
        .collect())
}

/// Connected groups (face neighbors or a feed between them) of 2..=max control blocks.
fn rip_sets(d: &Design, control: &[Pos], max: usize) -> Vec<Vec<Pos>> {
    let set: HashSet<Pos> = control.iter().copied().collect();
    let near = |a: Pos, b: Pos| -> bool {
        if manhattan(a, b) == 1 {
            return true;
        }
        let (Some(sa), Some(sb)) = (d.blocks.get(&a), d.blocks.get(&b)) else { return false };
        let (na, nb) = (Node::new(a, sa), Node::new(b, sb));
        drives(&na, &nb) || drives(&nb, &na)
    };
    let mut adj: HashMap<Pos, Vec<Pos>> = HashMap::new();
    for &a in control {
        for &b in control {
            if a != b && (0..3).all(|i| (a[i] - b[i]).abs() <= 1) && near(a, b) {
                adj.entry(a).or_default().push(b);
            }
        }
    }
    let mut all: HashSet<Vec<Pos>> = HashSet::new();
    let mut frontier: Vec<Vec<Pos>> = control.iter().map(|p| vec![*p]).collect();
    for _ in 1..max {
        let mut next = Vec::new();
        for g in &frontier {
            for p in g {
                for n in adj.get(p).map(|v| v.as_slice()).unwrap_or(&[]) {
                    if set.contains(n) && !g.contains(n) {
                        let mut h = g.clone();
                        h.push(*n);
                        h.sort();
                        if all.insert(h.clone()) {
                            next.push(h);
                        }
                    }
                }
            }
        }
        frontier = next;
    }
    let mut out: Vec<Vec<Pos>> = all.into_iter().collect();
    out.sort();
    out
}

/// What a rip of `cells` needs: the receivers whose wave changes (None if the rip leaves a cell
/// with power it must not have, which re-routing cannot fix).
fn rip_receivers(pb: &Problem, orig: &[(Pos, Pos, Wave)], ripped: &Model, tol: i32) -> Option<Vec<Receiver>> {
    let mut out = Vec::new();
    for &(p, f, want) in orig {
        let now = ripped.piston_wave(p, f);
        if same_wave(now, want, tol) {
            continue;
        }
        if !now.minus(want).is_off() && tol == 0 {
            return None;
        }
        out.push(Receiver { pos: p, facing: f, want, rest: now });
    }
    let _ = pb;
    Some(out)
}

/// Print every piston cell's signal in the model and compare the model with the simulator on
/// every control component of `d`.
pub fn model_report(d: &Design, meas: &Measured, log: &(dyn Fn(&str) + Sync)) {
    let lever = DoorSpec::from_design(d).ok().map(|s| s.lever);
    let m = Model::new(&d.blocks, lever, &meas.fixed, &meas.extra);
    for &(p, f) in &meas.pistons {
        log(&format!("  piston cell {} facing {}: {}", fmt_pos(p), fmt_pos(f), m.piston_wave(p, f).describe()));
    }
    for (p, w) in &meas.fixed {
        log(&format!("  measured source {} {}: {}", fmt_pos(*p), short(d.blocks.get(p).map(|s| s.as_str()).unwrap_or("?")), w.describe()));
    }
    for (p, _, w) in &meas.extra {
        log(&format!("  shuttled redstone block at {}: {}", fmt_pos(*p), w.describe()));
    }
    let mut bad = 0;
    let mut cells: Vec<&Pos> = meas.measured.keys().collect();
    cells.sort();
    for p in cells {
        let wm = meas.measured[p];
        if let Some(ws) = m.wave_at(*p) {
            if ws != wm && !meas.fixed.contains_key(p) {
                bad += 1;
                log(&format!(
                    "  model differs at {} {}: model [{}] sim [{}]",
                    fmt_pos(*p),
                    short(d.blocks.get(p).map(|s| s.as_str()).unwrap_or("?")),
                    ws.describe(),
                    wm.describe()
                ));
            }
        }
    }
    log(&format!("  calibration: {} components, {bad} differ from the simulator", meas.measured.len()));
}

/// Screen and judge whole designs (any block count); the passing ones, fewest blocks first.
pub fn validate(pool: &Pool, cands: Vec<(Design, String)>, sopts: &SearchOptions, log: &(dyn Fn(&str) + Sync)) -> Vec<(Design, String)> {
    let sandboxes = crate::minimize::Sandboxes::new(pool);
    let n = cands.len();
    let res = pool.map(cands, |w, wi, (d, desc)| {
        let sb = sandboxes.get(w, wi)?;
        crate::search::screen_in(w, sb, &d, &sopts.screen)?;
        crate::verify_in(w, sb, &d, &sopts.judge)?;
        Ok((d, desc))
    });
    let mut ok: Vec<(Design, String)> = res.into_iter().filter_map(|r| r.ok()).collect();
    ok.sort_by_key(|(d, _)| block_count(d));
    log(&format!("{n} candidates simulated, {} pass the judge", ok.len()));
    ok
}

/// Route a new net on `d`: each `needs` piston cell must get its wave; every other piston cell
/// keeps the signal it has. Returns the verified designs, fewest blocks first.
pub fn route_new(
    pool: &Pool,
    d: &Design,
    meas: &Measured,
    needs: &[(Pos, Wave)],
    o: &RipOptions,
    sopts: &SearchOptions,
    log: &(dyn Fn(&str) + Sync),
) -> Result<Vec<(Design, String)>> {
    let spec = DoorSpec::from_design(d)?;
    let (lo, hi) = d.bounds().context("empty design")?;
    let mg = o.route.margin;
    let pb = Problem {
        base: d,
        meas,
        lever: Some(spec.lever),
        forbidden: forbidden_cells(d)?,
        lo: [lo[0] - mg, lo[1] - mg, lo[2] - mg],
        hi: [hi[0] + mg, hi[1] + mg, hi[2] + mg],
    };
    let m = pb.model(&d.blocks);
    let mut want: Vec<(Pos, Pos, Wave)> = pb.receiver_waves(&m);
    for (p, w) in needs {
        match want.iter_mut().find(|x| x.0 == *p) {
            Some(x) => x.2 = *w,
            None => bail!("{} holds no piston at any tick", fmt_pos(*p)),
        }
    }
    let recv: Vec<Receiver> = want
        .iter()
        .filter(|(p, f, w)| !same_wave(m.piston_wave(*p, *f), *w, o.route.tolerance))
        .map(|&(p, f, w)| Receiver { pos: p, facing: f, want: w, rest: m.piston_wave(p, f) })
        .collect();
    for r in &recv {
        log(&format!("  need {} [{}], has [{}]", fmt_pos(r.pos), r.want.describe(), r.rest.describe()));
    }
    let cands = route_net(&pb, &recv, 40, &o.route);
    log(&format!("{} routings", cands.len()));
    let mut designs = Vec::new();
    for c in cands {
        let mut new = d.clone();
        for (p, x) in &c {
            new.blocks.insert(*p, x.clone());
        }
        let nm = pb.model(&new.blocks);
        let ok = o.no_check || want.iter().all(|&(p, f, w)| same_wave(nm.piston_wave(p, f), w, o.route.tolerance));
        if ok {
            let desc = c.iter().map(|(p, x)| format!("{} {}", fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" ");
            designs.push((new, format!("+{} blocks: {desc}", c.len())));
        }
    }
    log(&format!("{} pass the static model", designs.len()));
    Ok(validate(pool, designs, sopts, log))
}

pub struct RipOptions {
    pub max_rip: usize,
    pub route: RouteOptions,
    /// Simulate candidates the static model rejects too.
    pub no_check: bool,
    /// Only these rip sets (empty: every connected group).
    pub rip: Vec<Vec<Pos>>,
    pub verbose: bool,
    /// Allow this many more blocks than the rip removes (1: equal-size re-routes, a sanity check).
    pub extra_blocks: u32,
}

/// One rip-up-and-re-route pass over `d`: every candidate (fewer blocks) that the static model
/// accepts, as edits of `d`.
pub fn rip_candidates(d: &Design, meas: &Measured, o: &RipOptions, log: &(dyn Fn(&str) + Sync)) -> Result<Vec<Edit>> {
    let spec = DoorSpec::from_design(d)?;
    let (lo, hi) = d.bounds().context("empty design")?;
    let mg = o.route.margin;
    let pb0 = Problem {
        base: d,
        meas,
        lever: Some(spec.lever),
        forbidden: forbidden_cells(d)?,
        lo: [lo[0] - mg, lo[1] - mg, lo[2] - mg],
        hi: [hi[0] + mg, hi[1] + mg, hi[2] + mg],
    };
    let orig_model = pb0.model(&d.blocks);
    let orig = pb0.receiver_waves(&orig_model);
    if o.verbose {
        model_report(d, meas, log);
    }
    let control = control_blocks(d, meas)?;
    let sets = if o.rip.is_empty() { rip_sets(d, &control, o.max_rip) } else { o.rip.clone() };
    log(&format!("{} control blocks, {} rip sets (up to {})", control.len(), sets.len(), o.max_rip));
    let out: Mutex<Vec<Edit>> = Mutex::new(Vec::new());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let stats = Mutex::new((0usize, 0usize, 0usize));
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(24);
    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if i >= sets.len() {
                        break;
                    }
                    let set = &sets[i];
                    let mut base = d.clone();
                    for p in set {
                        base.blocks.remove(p);
                    }
                    let pb = Problem { base: &base, meas, lever: pb0.lever, forbidden: pb0.forbidden.clone(), lo: pb0.lo, hi: pb0.hi };
                    let ripped = pb.model(&base.blocks);
                    let Some(recv) = rip_receivers(&pb, &orig, &ripped, o.route.tolerance) else { continue };
                    let budget = (set.len() as i64 - 1 + o.extra_blocks as i64).max(0) as u32;
                    let cands = if recv.is_empty() { vec![Vec::new()] } else { route_net(&pb, &recv, budget, &o.route) };
                    if o.verbose {
                        log(&format!(
                            "  rip {} ({} receivers: {}): {} routings, best {}",
                            set.iter().map(|p| format!("{} {}", fmt_pos(*p), bname(&d.blocks[p]))).collect::<Vec<_>>().join(" "),
                            recv.len(),
                            recv.iter().map(|r| format!("{} [{}]", fmt_pos(r.pos), r.want.describe())).collect::<Vec<_>>().join(" "),
                            cands.len(),
                            cands.first().map(|c| c.len().to_string()).unwrap_or("-".into())
                        ));
                    }
                    let mut st = stats.lock().unwrap();
                    st.0 += 1;
                    st.1 += cands.len();
                    drop(st);
                    for c in cands {
                        let mut new = base.clone();
                        for (p, x) in &c {
                            new.blocks.insert(*p, x.clone());
                        }
                        // The static model must find every receiver cell unchanged.
                        let ok = o.no_check || {
                            let m = pb.model(&new.blocks);
                            orig.iter().all(|&(p, f, w)| same_wave(m.piston_wave(p, f), w, o.route.tolerance))
                        };
                        if !ok {
                            if o.verbose {
                                let m = pb.model(&new.blocks);
                                let bad: Vec<String> = orig
                                    .iter()
                                    .filter(|&&(p, f, w)| !same_wave(m.piston_wave(p, f), w, o.route.tolerance))
                                    .map(|&(p, f, w)| format!("{} gets [{}] wants [{}]", fmt_pos(p), m.piston_wave(p, f).describe(), w.describe()))
                                    .collect();
                                log(&format!("    rejected by the model: {} ({})", c.iter().map(|(p, x)| format!("{} {}", fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" "), bad.join("; ")));
                            }
                            continue;
                        }
                        if o.verbose {
                            log(&format!("    candidate: {}", c.iter().map(|(p, x)| format!("{} {}", fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" ")));
                        }
                        stats.lock().unwrap().2 += 1;
                        let desc = format!(
                            "rip {} -> {}",
                            set.iter().map(|p| format!("{} {}", fmt_pos(*p), short(&d.blocks[p]))).collect::<Vec<_>>().join(" "),
                            if c.is_empty() { "nothing".to_string() } else { c.iter().map(|(p, x)| format!("{} {}", fmt_pos(*p), short(x))).collect::<Vec<_>>().join(" ") }
                        );
                        out.lock().unwrap().push(Edit { remove: set.clone(), set: c, ports: Vec::new(), desc });
                    }
                }
            });
        }
    });
    let st = stats.into_inner().unwrap();
    log(&format!("{} rip sets routed, {} candidates, {} pass the static model", st.0, st.1, st.2));
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|e| (e.set.len() as i64 - e.remove.len() as i64, e.desc.clone()));
    Ok(v)
}

/// Rip and re-route until nothing improves; returns the best verified design.
pub fn rip_and_reroute(
    pool: &Pool,
    d: &Design,
    o: &RipOptions,
    sopts: &SearchOptions,
    log: &(dyn Fn(&str) + Sync),
    on_improve: &mut dyn FnMut(&Design, &str) -> Result<()>,
) -> Result<Design> {
    let mut cur = d.clone();
    loop {
        let meas = {
            let mut w = pool.worker(0);
            measure(&mut w, &cur)?
        };
        if !meas.idle {
            log("warning: the design is not idle within T ticks of a flip; waves are cut");
        }
        let edits = rip_candidates(&cur, &meas, o, log)?;
        if edits.is_empty() {
            log("no candidate");
            return Ok(cur);
        }
        let found = crate::search::try_edits(pool, &cur, edits, sopts, &|m: &str| log(m));
        let Some((best, desc)) = found.into_iter().next() else {
            log("no candidate passes the judge");
            return Ok(cur);
        };
        let canon = {
            let mut w = pool.worker(0);
            let c = crate::canonicalize(&mut w, &best)?;
            if c.blocks.values().any(|s| mctas::blockstate::is_air(s)) { best.clone() } else { c }
        };
        {
            let mut w = pool.worker(0);
            crate::verify(&mut w, &canon, &sopts.judge).context("the accepted design fails the judge in a fresh sandbox")?;
        }
        log(&format!("improved: {} -> {} blocks: {desc}", block_count(&cur), block_count(&canon)));
        on_improve(&canon, &desc)?;
        cur = canon;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waves() {
        let lever = Wave::LEVER;
        let r = Wave::from_seq(&repeater_seq(&lever.seq(), 4));
        assert_eq!(r, lever.shift(4));
        let o = transfer(&Node::new([0, 0, 0], "minecraft:observer[facing=north,powered=false]"), lever);
        assert_eq!(o.describe(), "open 2-3, close 2-3");
        // A 2 gt pulse into a delay-4 repeater (8 gt) comes out 8 gt long.
        let p = Wave::from_seq(&repeater_seq(&o.seq(), 8));
        assert_eq!(p.describe(), "open 10-17, close 10-17");
        let t = Wave::from_seq(&torch_seq(&lever.seq()));
        assert_eq!(t.describe(), "open 0-1, close 2-");
        assert_eq!(Wave::parse("level+4").unwrap(), lever.shift(4));
        assert_eq!(Wave::parse("pulse+2/2").unwrap(), o);
        assert_eq!(Wave::parse("open:2-3;close:2-3").unwrap(), o);
    }
}
