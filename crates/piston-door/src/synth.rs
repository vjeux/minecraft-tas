//! `mctas door synth`: circuit synthesis by exhaustive simulation.
//!
//! Given a context (a door design, or a gadget harness with a lever and probe cells), a box of
//! free cells, a palette of block states and a target behavior, enumerate every circuit of 1..k
//! blocks that could carry a signal from the source cells to the sink cells, simulate each one in
//! reusable sandboxes on many workers, and report the smallest that meet the target.
//!
//! **Enumeration** (a static signal model, see [`Node`] / [`drives`]): a circuit grows from the
//! sources. Every new block must be driven by a source, by a block already in the circuit, or by a
//! context block the circuit powers (a repeater reading it, dust next to it, a torch on it, an
//! observer watching it...); a block that needs support gets one (a context block, a circuit
//! block, or a floor from `--floors`, which counts). A circuit is kept when some block of it
//! drives a sink cell and (unless `--loose`) every block lies on a source-to-sink path or holds
//! one up. Sets are deduplicated; a distance bound prunes circuits that cannot reach a sink in
//! the remaining budget; `--require kind:n` / `--limit kind:n` constrain the parts list.
//!
//! **Targets**: by default the door judge (the frame-less screen at `--screen-offsets` positions,
//! then the full judge at 32 positions); with `--spec FILE`, a behavior script run on the context
//! (see [`Spec`]): `probe name x y z` (the best signal the cell's neighbors give into it: what a
//! piston or lamp there would see), `probe_state name x y z` (block-state probe value as in
//! `door drive`), `use x y z`, `run n` (step n ticks recording every probe), `settle [max]`,
//! `expect name lo..hi on|off|<n>|<state>` (ticks of the last `run`, 1-based), `idle`.

use crate::search::screen_in;
use crate::{DoorSpec, VerifyOptions, verify_in};
use anyhow::{Context, Result, anyhow, bail};
use mctas::blockstate::BlockState;
use mctas::design::Design;
use mctas::sim::{PlaceMode, Pool, Pos, Worker};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const FACES: [Pos; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const HORIZ: [Pos; 4] = [[1, 0, 0], [-1, 0, 0], [0, 0, 1], [0, 0, -1]];
const HORIZONTAL: [&str; 4] = ["north", "east", "south", "west"];
const ALL_DIRS: [&str; 6] = ["north", "east", "south", "west", "up", "down"];

fn add(a: Pos, b: Pos) -> Pos {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn neg(a: Pos) -> Pos {
    [-a[0], -a[1], -a[2]]
}
fn dir(f: &str) -> Option<Pos> {
    Some(match f {
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "east" => [1, 0, 0],
        "west" => [-1, 0, 0],
        "up" => [0, 1, 0],
        "down" => [0, -1, 0],
        _ => return None,
    })
}
pub fn fmt_pos(p: Pos) -> String {
    format!("({},{},{})", p[0], p[1], p[2])
}
fn short(s: &str) -> &str {
    s.strip_prefix("minecraft:").unwrap_or(s)
}

/// Parse "x,y,z;x,y,z" (empty = none).
pub fn parse_cells(s: &str) -> Result<Vec<Pos>> {
    s.split(';')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            let v: Vec<i32> = t.split(',').map(|x| x.trim().parse::<i32>()).collect::<Result<_, _>>()?;
            anyhow::ensure!(v.len() == 3, "expected x,y,z, got {t:?}");
            Ok([v[0], v[1], v[2]])
        })
        .collect()
}

/// Expand palette shorthands: `block` (smooth_stone), `gt` (glazed terracotta: a floor slime
/// never drags), `glass`, `obsidian`, `dust` (cross wire), `dot` (dot wire), `rep` (every delay)
/// or `rep1`..`rep4` (4 facings each), `cmp` (comparator, both modes, 4 facings), `obs`
/// (observer, 6 facings), `torch` (standing + 4 wall torches), `rblock`, `lamp`, `bulb` (waxed
/// copper bulb, a T flip-flop), `target`, `note` (note block: a conductor whose `powered` state
/// follows its power at once, so an observer watching it pulses on both edges, and which updates
/// its neighbors when it changes), or a full `minecraft:...` state.
pub fn palette_states(spec: &str) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for tok in spec.split(',').map(str::trim).filter(|t| !t.is_empty()) {
        match tok {
            "block" => out.push("minecraft:smooth_stone".into()),
            "gt" => out.push("minecraft:white_glazed_terracotta[facing=north]".into()),
            "glass" => out.push("minecraft:glass".into()),
            "obsidian" => out.push("minecraft:obsidian".into()),
            "dust" => out.push("minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".into()),
            "dot" => out.push("minecraft:redstone_wire[east=none,north=none,power=0,south=none,west=none]".into()),
            "rep" | "rep1" | "rep2" | "rep3" | "rep4" => {
                let delays: Vec<u32> = match tok.strip_prefix("rep").and_then(|d| d.parse::<u32>().ok()) {
                    Some(d) => vec![d],
                    None => vec![1, 2, 3, 4],
                };
                for d in delays {
                    out.extend(HORIZONTAL.iter().map(|f| format!("minecraft:repeater[delay={d},facing={f},locked=false,powered=false]")));
                }
            }
            "cmp" => {
                for m in ["compare", "subtract"] {
                    out.extend(HORIZONTAL.iter().map(|f| format!("minecraft:comparator[facing={f},mode={m},powered=false]")));
                }
            }
            "obs" => out.extend(ALL_DIRS.iter().map(|f| format!("minecraft:observer[facing={f},powered=false]"))),
            "torch" => {
                out.push("minecraft:redstone_torch[lit=true]".into());
                out.extend(HORIZONTAL.iter().map(|f| format!("minecraft:redstone_wall_torch[facing={f},lit=true]")));
            }
            "rblock" => out.push("minecraft:redstone_block".into()),
            "lamp" => out.push("minecraft:redstone_lamp[lit=false]".into()),
            "bulb" => out.push("minecraft:waxed_copper_bulb[lit=false,powered=false]".into()),
            "target" => out.push("minecraft:target[power=0]".into()),
            // A conductor whose own state flips (powered) the tick its power changes: an
            // observer can watch any powered spot through it, on both edges, with no delay.
            "note" => out.push("minecraft:note_block[instrument=harp,note=0,powered=false]".into()),
            s if s.starts_with("minecraft:") => out.push(s.to_string()),
            s => bail!("unknown palette entry {s:?}"),
        }
    }
    Ok(out)
}

/// What a block does in the static signal model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Full conductor: relays the strong power it gets to dust and components next to it.
    Block,
    /// Full block that conducts nothing (glass, slime, pistons...).
    Inert,
    Dust,
    Repeater,
    Comparator,
    Observer,
    Torch,
    Lamp,
    RBlock,
    Lever,
    /// Sculk sensor (category sculk): driven wirelessly by the lever's vibration (always fed),
    /// weak power on every side (a calibrated one: all but its back), strong into the block below.
    Sensor,
    /// Not sturdy, not a conductor (piston heads, moving blocks...).
    Other,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub pos: Pos,
    pub state: String,
    pub kind: Kind,
    /// Repeater/comparator: toward the input (back) cell; observer: toward the watched cell.
    pub facing: Pos,
    /// Cell that must hold a sturdy block (floor or wall).
    pub support: Option<Pos>,
    /// Can hold dust, repeaters and torches.
    pub sturdy: bool,
    /// Its block state changes with its input (observers can watch it).
    pub changes: bool,
    /// Cached [`Node::emits`] and [`Node::reads`].
    pub em: Vec<Pos>,
    pub rd: Vec<Pos>,
}

impl Node {
    pub fn new(pos: Pos, state: &str) -> Node {
        let b = BlockState::parse(state).ok();
        let get = |k: &str| b.as_ref().and_then(|b| b.get(k)).map(str::to_string);
        let name = short(mctas::blockstate::block_name(state)).to_string();
        let below = Some(add(pos, [0, -1, 0]));
        let facing = get("facing").and_then(|f| dir(&f)).unwrap_or([0, 0, 0]);
        let (kind, support, sturdy, changes) = match name.as_str() {
            "redstone_wire" => (Kind::Dust, below, false, true),
            "repeater" => (Kind::Repeater, below, false, true),
            "comparator" => (Kind::Comparator, below, false, true),
            "observer" => (Kind::Observer, None, true, true),
            "redstone_torch" => (Kind::Torch, below, false, true),
            "redstone_wall_torch" => (Kind::Torch, Some(add(pos, neg(facing))), false, true),
            "redstone_lamp" => (Kind::Lamp, None, true, true),
            // A copper bulb takes power like a lamp and shows it only to observers (and
            // comparators): it toggles `lit` on each rising edge.
            n if n.ends_with("copper_bulb") => (Kind::Lamp, None, true, true),
            "redstone_block" => (Kind::RBlock, None, true, false),
<<<<<<< HEAD
            // A note block conducts like any full block, and its `powered` state follows its
            // power in the same tick (observers can watch it).
            "note_block" => (Kind::Block, None, true, true),
=======
            "sculk_sensor" | "calibrated_sculk_sensor" => (Kind::Sensor, None, false, true),
>>>>>>> origin/explore/sculk-2
            "lever" => {
                let s = match get("face").as_deref() {
                    Some("floor") => below,
                    Some("ceiling") => Some(add(pos, [0, 1, 0])),
                    _ => Some(add(pos, neg(facing))),
                };
                (Kind::Lever, s, false, true)
            }
            "piston" | "sticky_piston" => (Kind::Inert, None, get("extended").as_deref() != Some("true"), true),
            "piston_head" | "moving_piston" | "air" => (Kind::Other, None, false, false),
            n if n.contains("glass") || n == "slime_block" || n == "honey_block" => (Kind::Inert, None, true, false),
            n if n.contains("slab") || n.contains("stairs") || n.contains("torch") || n.contains("button") || n.contains("rail") => {
                (Kind::Other, None, false, false)
            }
            _ => (Kind::Block, None, true, false),
        };
        Node { pos, state: state.to_string(), kind, facing, support, sturdy, changes, em: Vec::new(), rd: Vec::new() }.cached()
    }

    /// Fill the emits/reads caches (call again after changing `kind`).
    pub fn cached(mut self) -> Node {
        self.em = self.emits();
        self.rd = self.reads();
        self
    }

    /// Cells whose emitters can drive this block.
    pub fn reads(&self) -> Vec<Pos> {
        let p = self.pos;
        match self.kind {
            Kind::Block | Kind::Lamp => FACES.iter().map(|f| add(p, *f)).collect(),
            Kind::Dust => {
                let mut v: Vec<Pos> = FACES.iter().map(|f| add(p, *f)).collect();
                for h in HORIZ {
                    v.push(add(add(p, h), [0, 1, 0]));
                    v.push(add(add(p, h), [0, -1, 0]));
                }
                v
            }
            Kind::Repeater | Kind::Observer => vec![add(p, self.facing)],
            Kind::Comparator => {
                let side = [self.facing[2], 0, self.facing[0]];
                vec![add(p, self.facing), add(p, side), add(p, neg(side))]
            }
            Kind::Torch => self.support.into_iter().collect(),
            _ => Vec::new(),
        }
    }

    /// Cells this block drives when it is on.
    pub fn emits(&self) -> Vec<Pos> {
        let p = self.pos;
        match self.kind {
            Kind::Block | Kind::RBlock | Kind::Lever => FACES.iter().map(|f| add(p, *f)).collect(),
            // A calibrated sensor's `facing` points away from its back (its frequency input).
            Kind::Sensor => FACES.iter().map(|f| add(p, *f)).filter(|c| self.facing == [0, 0, 0] || *c != add(p, neg(self.facing))).collect(),
            Kind::Dust => {
                let mut v: Vec<Pos> = vec![add(p, [0, -1, 0])];
                for h in HORIZ {
                    v.push(add(p, h));
                    v.push(add(add(p, h), [0, 1, 0]));
                    v.push(add(add(p, h), [0, -1, 0]));
                }
                v
            }
            Kind::Repeater | Kind::Comparator | Kind::Observer => vec![add(p, neg(self.facing))],
            Kind::Torch => FACES.iter().map(|f| add(p, *f)).filter(|c| Some(*c) != self.support).collect(),
            _ => Vec::new(),
        }
    }
}

/// `u` (when on) drives `v` in the static model.
pub fn drives(u: &Node, v: &Node) -> bool {
    if v.kind == Kind::Observer {
        return u.changes && add(v.pos, v.facing) == u.pos;
    }
    // A comparator reads a copper bulb behind it (analog output 15 while lit): the bulb's T
    // flip-flop state as a level.
    if v.kind == Kind::Comparator && u.kind == Kind::Lamp && u.state.contains("copper_bulb") {
        return add(v.pos, v.facing) == u.pos;
    }
    if !u.em.contains(&v.pos) || !v.rd.contains(&u.pos) {
        return false;
    }
    let dy = v.pos[1] - u.pos[1];
    let diag = (v.pos[0] - u.pos[0]).abs() + (v.pos[2] - u.pos[2]).abs() + dy.abs() > 1;
    match (u.kind, v.kind) {
        (_, Kind::Inert | Kind::Other | Kind::RBlock | Kind::Lever | Kind::Sensor) => false,
        (Kind::Lamp | Kind::Inert | Kind::Other, _) => false,
        // A sensor strongly powers only the block right below it.
        (Kind::Sensor, Kind::Block) => dy == -1 && !diag,
        // A torch reads the strong power of the block it hangs on.
        (Kind::Block, Kind::Torch) => v.support == Some(u.pos),
        (_, Kind::Torch) => false,
        // Blocks do not power blocks; a torch strongly powers only the block above it; a lever
        // only its attachment; a redstone block none.
        (Kind::Block, Kind::Block) => false,
        (Kind::Torch, Kind::Block) => dy == 1,
        (Kind::Lever, Kind::Block) => u.support == Some(v.pos),
        (Kind::RBlock, Kind::Block) => false,
        // Dust powers the block below it and the blocks it points into (never diagonally).
        (Kind::Dust, Kind::Block | Kind::Lamp) => !diag && dy <= 0,
        (Kind::Dust, Kind::Dust) => true,
        (_, Kind::Dust) => !diag,
        _ => !diag,
    }
}

/// Synthesis knobs (see the module docs).
pub struct SynthOptions {
    pub lo: Pos,
    pub hi: Pos,
    pub palette: Vec<String>,
    pub floors: Vec<String>,
    pub k: usize,
    pub min_k: usize,
    pub sources: Vec<Pos>,
    /// Cells that change while the machine runs (observers may watch them).
    pub watch: Vec<Pos>,
    pub sinks: Vec<Pos>,
    pub forbid: HashSet<Pos>,
    pub require: Vec<(String, usize)>,
    pub limit: Vec<(String, usize)>,
    /// Keep only circuits with at least one block whose state contains one of these
    /// (see [`require_any_names`]): sweep new palette entries on designs the old palette
    /// already exhausted.
    pub require_any: Vec<String>,
    pub loose: bool,
    pub max_cands: usize,
}

/// Expand `--require-any` shorthands (lamp, bulb, note, target, or any state substring).
pub fn require_any_names(spec: &str) -> Vec<String> {
    spec.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| match t {
            "lamp" => "redstone_lamp".to_string(),
            "bulb" => "copper_bulb".to_string(),
            "note" => "note_block".to_string(),
            "target" => "minecraft:target".to_string(),
            s => s.to_string(),
        })
        .collect()
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Block => "block",
        Kind::Inert => "inert",
        Kind::Dust => "dust",
        Kind::Repeater => "repeater",
        Kind::Comparator => "comparator",
        Kind::Observer => "observer",
        Kind::Torch => "torch",
        Kind::Lamp => "lamp",
        Kind::RBlock => "rblock",
        Kind::Lever => "lever",
        Kind::Sensor => "sensor",
        Kind::Other => "other",
    }
}

/// The enumerator: every candidate atom (state at a free cell), the context's sources and relays.
pub struct Gen {
    pub atoms: Vec<Node>,
    in_palette: Vec<bool>,
    by_cell: HashMap<Pos, Vec<u32>>,
    floors_at: HashMap<Pos, Vec<u32>>,
    sources: Vec<Node>,
    relays: Vec<Node>,
    context: BTreeMap<Pos, String>,
    sinks: Vec<Pos>,
    /// Free cells a context observer watches: a block there that changes drives that observer.
    watched: Vec<Pos>,
    free: HashSet<Pos>,
    k: usize,
    require: Vec<(Kind, usize)>,
    limit: Vec<(Kind, usize)>,
    require_any: Vec<String>,
    loose: bool,
    max_cands: usize,
}

fn parse_kind(s: &str) -> Result<Kind> {
    Ok(match s {
        "block" => Kind::Block,
        "inert" | "glass" => Kind::Inert,
        "dust" => Kind::Dust,
        "repeater" | "rep" => Kind::Repeater,
        "comparator" | "cmp" => Kind::Comparator,
        "observer" | "obs" => Kind::Observer,
        "torch" => Kind::Torch,
        "lamp" => Kind::Lamp,
        "rblock" => Kind::RBlock,
        "sensor" => Kind::Sensor,
        _ => bail!("unknown kind {s:?}"),
    })
}

/// "kind:n,kind:n".
pub fn parse_counts(s: &str) -> Result<Vec<(String, usize)>> {
    s.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| {
            let (k, n) = t.split_once(':').ok_or_else(|| anyhow!("expected kind:n, got {t:?}"))?;
            Ok((k.trim().to_string(), n.trim().parse::<usize>()?))
        })
        .collect()
}

impl Gen {
    pub fn new(context: &Design, o: &SynthOptions) -> Result<Gen> {
        let mut forbid = o.forbid.clone();
        if let Ok(spec) = DoorSpec::from_design(context) {
            forbid.extend(spec.door_cells());
            forbid.extend(spec.corridor_cells());
            forbid.insert(spec.lever);
        }
        let mut free = HashSet::new();
        for x in o.lo[0]..=o.hi[0] {
            for y in o.lo[1]..=o.hi[1] {
                for z in o.lo[2]..=o.hi[2] {
                    let p = [x, y, z];
                    if !context.blocks.contains_key(&p) && !forbid.contains(&p) {
                        free.insert(p);
                    }
                }
            }
        }
        let mut atoms = Vec::new();
        let mut in_palette = Vec::new();
        let mut by_cell: HashMap<Pos, Vec<u32>> = HashMap::new();
        let mut floors_at: HashMap<Pos, Vec<u32>> = HashMap::new();
        let mut cells: Vec<Pos> = free.iter().copied().collect();
        cells.sort();
        for p in &cells {
            for s in &o.palette {
                let id = atoms.len() as u32;
                atoms.push(Node::new(*p, s));
                in_palette.push(true);
                by_cell.entry(*p).or_default().push(id);
                if o.floors.contains(s) {
                    floors_at.entry(*p).or_default().push(id);
                }
            }
            for s in &o.floors {
                if o.palette.contains(s) {
                    continue;
                }
                let id = atoms.len() as u32;
                atoms.push(Node::new(*p, s));
                in_palette.push(false);
                floors_at.entry(*p).or_default().push(id);
            }
        }
        let mut sources = Vec::new();
        for p in &o.sources {
            let s = context.blocks.get(p).with_context(|| format!("source {} is air in the context", fmt_pos(*p)))?;
            sources.push(Node::new(*p, s));
        }
        // Watched cells: they change while the machine runs (a moving block, a piston), so an
        // observer may watch them; they drive nothing else.
        for p in &o.watch {
            let s = context.blocks.get(p).cloned().unwrap_or_else(|| "minecraft:air".into());
            let mut n = Node::new(*p, &s);
            n.kind = Kind::Other;
            n.changes = true;
            sources.push(n.cached());
        }
        // Context conductors next to the box can relay power the circuit gives them.
        let mut relays = Vec::new();
        for (p, s) in &context.blocks {
            let near = (0..3).all(|i| p[i] >= o.lo[i] - 1 && p[i] <= o.hi[i] + 1);
            if near && !o.sources.contains(p) {
                let n = Node::new(*p, s);
                if n.kind == Kind::Block {
                    relays.push(n);
                }
            }
        }
        let require = o.require.iter().map(|(k, n)| Ok((parse_kind(k)?, *n))).collect::<Result<Vec<_>>>()?;
        let limit = o.limit.iter().map(|(k, n)| Ok((parse_kind(k)?, *n))).collect::<Result<Vec<_>>>()?;
        Ok(Gen {
            atoms,
            in_palette,
            by_cell,
            floors_at,
            sources,
            relays,
            context: context.blocks.clone(),
            sinks: o.sinks.clone(),
            watched: context
                .blocks
                .iter()
                .filter(|(_, s)| s.starts_with("minecraft:observer"))
                .map(|(p, s)| add(*p, Node::new(*p, s).facing))
                .filter(|w| free.contains(w))
                .collect(),
            free,
            k: o.k,
            require,
            limit,
            require_any: o.require_any.clone(),
            loose: o.loose,
            max_cands: o.max_cands,
        })
    }

    fn sturdy_at(&self, set: &[u32], p: Pos) -> bool {
        if let Some(s) = self.context.get(&p) {
            return Node::new(p, s).sturdy;
        }
        set.iter().any(|&i| self.atoms[i as usize].pos == p && self.atoms[i as usize].sturdy)
    }

    fn occupied(&self, set: &[u32], p: Pos) -> bool {
        self.context.contains_key(&p) || set.iter().any(|&i| self.atoms[i as usize].pos == p)
    }

    /// Which atoms of `set` and which relays are driven (transitively) from the sources.
    fn fed(&self, set: &[u32]) -> (Vec<bool>, Vec<bool>) {
        let mut fa = vec![false; set.len()];
        let mut fr = vec![false; self.relays.len()];
        loop {
            let mut changed = false;
            for (i, &a) in set.iter().enumerate() {
                if fa[i] {
                    continue;
                }
                let v = &self.atoms[a as usize];
                let by = v.kind == Kind::Sensor
                    || self.sources.iter().any(|s| drives(s, v))
                    || set.iter().enumerate().any(|(j, &b)| fa[j] && drives(&self.atoms[b as usize], v))
                    || self.relays.iter().enumerate().any(|(j, r)| fr[j] && drives(r, v));
                if by {
                    fa[i] = true;
                    changed = true;
                }
            }
            for (j, r) in self.relays.iter().enumerate() {
                if fr[j] {
                    continue;
                }
                if set.iter().enumerate().any(|(i, &a)| fa[i] && drives(&self.atoms[a as usize], r)) {
                    fr[j] = true;
                    changed = true;
                }
            }
            if !changed {
                return (fa, fr);
            }
        }
    }

    fn reaches_sink(&self, n: &Node) -> bool {
        self.sinks.iter().any(|s| n.em.contains(s)) || (n.changes && self.watched.contains(&n.pos))
    }

    /// A finished circuit: some driven block drives a sink, every block is on a source-to-sink
    /// path or holds one up (unless loose), and the parts constraints hold.
    fn complete(&self, set: &[u32], fa: &[bool], fr: &[bool]) -> bool {
        let reached = set.iter().enumerate().any(|(i, &a)| fa[i] && self.reaches_sink(&self.atoms[a as usize]))
            || self.relays.iter().enumerate().any(|(j, r)| fr[j] && self.reaches_sink(r));
        if !reached {
            return false;
        }
        if !self.require_any.is_empty() && !set.iter().any(|&a| self.require_any.iter().any(|s| self.atoms[a as usize].state.contains(s.as_str()))) {
            return false;
        }
        for (k, n) in &self.require {
            if set.iter().filter(|&&a| self.atoms[a as usize].kind == *k).count() < *n {
                return false;
            }
        }
        if self.loose {
            return true;
        }
        // Backward: useful = driven and (drives a sink or drives a useful block).
        let n = set.len();
        let mut useful = vec![false; n];
        let mut useful_relay = vec![false; self.relays.len()];
        loop {
            let mut changed = false;
            for i in 0..n {
                if useful[i] || !fa[i] {
                    continue;
                }
                let u = &self.atoms[set[i] as usize];
                let ok = self.reaches_sink(u)
                    || (0..n).any(|j| useful[j] && drives(u, &self.atoms[set[j] as usize]))
                    || self.relays.iter().enumerate().any(|(j, r)| useful_relay[j] && drives(u, r));
                if ok {
                    useful[i] = true;
                    changed = true;
                }
            }
            for (j, r) in self.relays.iter().enumerate() {
                if useful_relay[j] || !fr[j] {
                    continue;
                }
                if self.reaches_sink(r) || (0..n).any(|i| useful[i] && drives(r, &self.atoms[set[i] as usize])) {
                    useful_relay[j] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        (0..n).all(|i| {
            useful[i] || {
                let p = self.atoms[set[i] as usize].pos;
                (0..n).any(|j| useful[j] && self.atoms[set[j] as usize].support == Some(p))
            }
        })
    }

    /// Lower bound on blocks still needed to drive a sink from the current drivers.
    fn need(&self, set: &[u32], fa: &[bool], fr: &[bool]) -> usize {
        let dist = |p: Pos| -> i32 {
            self.sinks
                .iter()
                .map(|s| {
                    let h = (p[0] - s[0]).abs() + (p[2] - s[2]).abs();
                    let v = (p[1] - s[1]).abs();
                    h.max(v)
                })
                .min()
                .unwrap_or(0)
        };
        let mut best = i32::MAX;
        for s in &self.sources {
            best = best.min(dist(s.pos));
        }
        for (i, &a) in set.iter().enumerate() {
            if fa[i] {
                if self.reaches_sink(&self.atoms[a as usize]) {
                    return 0;
                }
                best = best.min(dist(self.atoms[a as usize].pos));
            }
        }
        for (j, r) in self.relays.iter().enumerate() {
            if fr[j] {
                if self.reaches_sink(r) {
                    return 0;
                }
                best = best.min(dist(r.pos));
            }
        }
        (best - 1).max(0) as usize
    }

    /// Every candidate circuit, bucketed by size (index = number of blocks). The search tree is
    /// split over `threads` threads sharing one (sharded) table of visited sets.
    pub fn enumerate(&self, log: &dyn Fn(&str)) -> Result<Vec<Vec<Vec<u32>>>> {
        let t = Instant::now();
        let shards: Vec<Mutex<HashSet<u64>>> = (0..256).map(|_| Mutex::new(HashSet::new())).collect();
        let total = AtomicUsize::new(0);
        // Sets visited by all threads: capped at 200 x max_cands, so a box/palette/k whose
        // search tree explodes (while few of its sets are finished circuits, e.g. with
        // --require-any) is skipped instead of enumerating for hours.
        let visits = AtomicUsize::new(0);
        let (fa, fr) = self.fed(&[]);
        let roots = self.expansions(&[], &fa, &fr);
        let next = AtomicUsize::new(0);
        let merged: Mutex<Vec<Vec<Vec<u32>>>> = Mutex::new(vec![Vec::new(); self.k + 1]);
        let failure: Mutex<Option<anyhow::Error>> = Mutex::new(None);
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(32);
        std::thread::scope(|s| {
            for _ in 0..threads {
                s.spawn(|| {
                    let mut out: Vec<Vec<Vec<u32>>> = vec![Vec::new(); self.k + 1];
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        if i >= roots.len() || failure.lock().unwrap().is_some() {
                            break;
                        }
                        let mut set = roots[i].clone();
                        if set.len() > self.k {
                            continue;
                        }
                        if let Err(e) = self.visit(&mut set, &shards, &mut out, &total, &visits) {
                            *failure.lock().unwrap() = Some(e);
                            break;
                        }
                    }
                    let mut m = merged.lock().unwrap();
                    for (k, v) in out.into_iter().enumerate() {
                        m[k].extend(v);
                    }
                });
            }
        });
        if let Some(e) = failure.into_inner().unwrap() {
            return Err(e);
        }
        let mut out = merged.into_inner().unwrap();
        for v in out.iter_mut() {
            v.sort();
        }
        let visited: usize = shards.iter().map(|s| s.lock().unwrap().len()).sum();
        log(&format!(
            "enumerated {} sets ({} visited) in {:.1}s on {threads} threads: {}",
            total.load(Ordering::Relaxed),
            visited,
            t.elapsed().as_secs_f64(),
            out.iter().enumerate().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{k}:{}", v.len())).collect::<Vec<_>>().join(" ")
        ));
        Ok(out)
    }

    fn key(set: &[u32]) -> u64 {
        let mut s = set.to_vec();
        s.sort_unstable();
        let mut h = DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    }

    /// The ways to grow `set` by one driven block (plus its floor when it needs one).
    fn expansions(&self, set: &[u32], fa: &[bool], fr: &[bool]) -> Vec<Vec<u32>> {
        let mut drivers: Vec<&Node> = self.sources.iter().collect();
        drivers.extend(set.iter().enumerate().filter(|(i, _)| fa[*i]).map(|(_, &a)| &self.atoms[a as usize]));
        drivers.extend(self.relays.iter().enumerate().filter(|(j, _)| fr[*j]).map(|(_, r)| r));
        let mut cands: Vec<u32> = Vec::new();
        for d in &drivers {
            let mut cells = d.em.clone();
            if d.changes {
                cells.extend(FACES.iter().map(|f| add(d.pos, *f)));
            }
            for c in cells {
                let Some(ids) = self.by_cell.get(&c) else { continue };
                if self.occupied(set, c) {
                    continue;
                }
                for &id in ids {
                    if self.in_palette[id as usize] && drives(d, &self.atoms[id as usize]) {
                        cands.push(id);
                    }
                }
            }
        }
        cands.sort_unstable();
        // Sensors need no wire: any free cell of the box may hold one (once per set).
        if !set.iter().any(|&i| self.atoms[i as usize].kind == Kind::Sensor) {
            for (id, a) in self.atoms.iter().enumerate() {
                if a.kind == Kind::Sensor && self.in_palette[id] && !self.occupied(set, a.pos) {
                    cands.push(id as u32);
                }
            }
            cands.sort_unstable();
        }
        cands.dedup();
        let mut out = Vec::new();
        for id in cands {
            let a = &self.atoms[id as usize];
            match a.support {
                Some(sc) if !self.sturdy_at(set, sc) => {
                    if self.occupied(set, sc) || !self.free.contains(&sc) {
                        continue;
                    }
                    for &f in self.floors_at.get(&sc).map(|v| v.as_slice()).unwrap_or(&[]) {
                        out.push(vec![id, f]);
                    }
                }
                _ => out.push(vec![id]),
            }
        }
        out
    }

    /// `set` was just grown: record it if it is a finished circuit, then grow it further.
    fn visit(&self, set: &mut Vec<u32>, shards: &[Mutex<HashSet<u64>>], out: &mut Vec<Vec<Vec<u32>>>, total: &AtomicUsize, visits: &AtomicUsize) -> Result<()> {
        let key = Self::key(set);
        if !shards[(key % shards.len() as u64) as usize].lock().unwrap().insert(key) {
            return Ok(());
        }
        let cap = self.max_cands.saturating_mul(200);
        if visits.fetch_add(1, Ordering::Relaxed) >= cap {
            bail!("more than {cap} sets visited (200 x --max-cands): shrink the box, palette or k");
        }
        let count = |k: Kind| set.iter().filter(|&&x| self.atoms[x as usize].kind == k).count();
        if !self.limit.iter().all(|(k, n)| count(*k) <= *n) {
            return Ok(());
        }
        let need_req: usize = self.require.iter().map(|(k, n)| n.saturating_sub(count(*k))).sum();
        if need_req > self.k - set.len() {
            return Ok(());
        }
        let (fa, fr) = self.fed(set);
        if self.complete(set, &fa, &fr) {
            let mut s = set.clone();
            s.sort_unstable();
            out[s.len()].push(s);
            if total.fetch_add(1, Ordering::Relaxed) + 1 > self.max_cands {
                bail!("more than {} candidates: shrink the box, palette or k", self.max_cands);
            }
        }
        if set.len() >= self.k || self.need(set, &fa, &fr) > self.k - set.len() {
            return Ok(());
        }
        for add_ids in self.expansions(set, &fa, &fr) {
            if set.len() + add_ids.len() > self.k {
                continue;
            }
            let before = set.len();
            set.extend(&add_ids);
            self.visit(set, shards, out, total, visits)?;
            set.truncate(before);
        }
        Ok(())
    }

    pub fn describe(&self, set: &[u32]) -> String {
        set.iter()
            .map(|&i| {
                let a = &self.atoms[i as usize];
                format!("+{} {}", fmt_pos(a.pos), short(&a.state))
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn design(&self, base: &Design, set: &[u32]) -> Design {
        let mut d = base.clone();
        for &i in set {
            let a = &self.atoms[i as usize];
            d.blocks.insert(a.pos, a.state.clone());
        }
        d
    }

    pub fn kinds(&self, set: &[u32]) -> String {
        let mut m: BTreeMap<&str, usize> = BTreeMap::new();
        for &i in set {
            *m.entry(kind_name(self.atoms[i as usize].kind)).or_default() += 1;
        }
        m.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")
    }
}

// ---- behavior specs -----------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Want {
    On,
    Off,
    Level(u8),
    State(String),
}

#[derive(Clone, Debug)]
pub enum SpecOp {
    Use(Pos),
    Run(u32),
    Settle(u32),
    Expect { probe: usize, lo: u32, hi: u32, want: Want },
    Idle,
}

/// A behavior script (see the module docs).
#[derive(Clone, Debug, Default)]
pub struct Spec {
    /// (name, cell, state probe?)
    pub probes: Vec<(String, Pos, bool)>,
    pub ops: Vec<SpecOp>,
}

impl Spec {
    pub fn parse(text: &str) -> Result<Spec> {
        let mut s = Spec::default();
        for (lineno, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap().trim();
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            let err = || anyhow!("spec line {}: {raw:?}", lineno + 1);
            let int = |i: usize| -> Result<i32> { parts.get(i).ok_or_else(err)?.parse::<i32>().map_err(|_| err()) };
            let pos = |i: usize| -> Result<Pos> { Ok([int(i)?, int(i + 1)?, int(i + 2)?]) };
            match parts[0] {
                "probe" | "probe_state" => s.probes.push((parts.get(1).ok_or_else(err)?.to_string(), pos(2)?, parts[0] == "probe_state")),
                "use" => s.ops.push(SpecOp::Use(pos(1)?)),
                "run" => s.ops.push(SpecOp::Run(int(1)? as u32)),
                "settle" => s.ops.push(SpecOp::Settle(if parts.len() > 1 { int(1)? as u32 } else { 400 })),
                "idle" => s.ops.push(SpecOp::Idle),
                "expect" => {
                    let name = parts.get(1).ok_or_else(err)?;
                    let probe = s.probes.iter().position(|(n, _, _)| n == name).ok_or_else(|| anyhow!("no probe {name}"))?;
                    let range = parts.get(2).ok_or_else(err)?;
                    let (lo, hi) = match range.split_once("..") {
                        Some((a, b)) => (a.parse::<u32>()?, b.parse::<u32>()?),
                        None => (range.parse::<u32>()?, range.parse::<u32>()?),
                    };
                    let w = parts.get(3).ok_or_else(err)?;
                    let want = match *w {
                        "on" => Want::On,
                        "off" => Want::Off,
                        _ if s.probes[probe].2 => Want::State(w.to_string()),
                        _ => Want::Level(w.parse::<u8>()?),
                    };
                    s.ops.push(SpecOp::Expect { probe, lo, hi, want });
                }
                other => bail!("spec line {}: unknown command {other}", lineno + 1),
            }
        }
        Ok(s)
    }

    fn translated(&self, off: Pos) -> Spec {
        let mut s = self.clone();
        for p in s.probes.iter_mut() {
            p.1 = add(p.1, off);
        }
        for op in s.ops.iter_mut() {
            if let SpecOp::Use(p) = op {
                *p = add(*p, off);
            }
        }
        s
    }
}

/// Place a design like the judge does (raw, wire shapes, every block re-checks, settle).
fn place_settled(w: &mut Worker, sb: u32, d: &Design, max_ticks: u32) -> Result<()> {
    w.clear(sb)?;
    let blocks = d.block_list();
    w.set_blocks(sb, &blocks, PlaceMode::Raw)?;
    let wires: Vec<Pos> = blocks.iter().filter(|(_, s)| s.starts_with("minecraft:redstone_wire")).map(|(p, _)| *p).collect();
    w.update(sb, &wires, "shape")?;
    let positions: Vec<Pos> = blocks.iter().map(|(p, _)| *p).collect();
    w.update(sb, &positions, "self")?;
    if w.step_until_idle(sb, max_ticks)?.idle != Some(true) {
        bail!("never settles");
    }
    Ok(())
}

/// Run a spec on a placed design; Err(reason) on the first failed expectation.
pub fn run_spec(w: &mut Worker, sb: u32, d: &Design, spec: &Spec, verbose: bool) -> Result<()> {
    place_settled(w, sb, d, 400)?;
    let sig: Vec<(usize, Pos)> = spec.probes.iter().enumerate().filter(|(_, p)| !p.2).map(|(i, p)| (i, p.1)).collect();
    let st: Vec<(usize, Pos)> = spec.probes.iter().enumerate().filter(|(_, p)| p.2).map(|(i, p)| (i, p.1)).collect();
    let sig_pos: Vec<Pos> = sig.iter().map(|(_, p)| *p).collect();
    let rec = if st.is_empty() {
        None
    } else {
        let mut lo = st[0].1;
        let mut hi = st[0].1;
        for (_, p) in &st {
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        Some((lo, hi))
    };
    // values[t][probe] for the last run.
    let mut values: Vec<Vec<String>> = Vec::new();
    for op in &spec.ops {
        match op {
            SpecOp::Use(p) => {
                w.use_block(sb, *p)?;
            }
            SpecOp::Settle(max) => {
                if w.step_until_idle(sb, *max)?.idle != Some(true) {
                    bail!("never settles");
                }
            }
            SpecOp::Idle => {
                if !w.status(sb)?.idle {
                    bail!("not idle");
                }
            }
            SpecOp::Run(n) => {
                let r = w.step_probe(sb, *n, false, &sig_pos, rec)?;
                values = (0..*n as usize)
                    .map(|t| {
                        let mut row = vec![String::new(); spec.probes.len()];
                        for (k, (i, _)) in sig.iter().enumerate() {
                            row[*i] = r.signals.get(t).and_then(|v| v.get(2 * k)).copied().unwrap_or(0).to_string();
                        }
                        for (i, p) in &st {
                            row[*i] = r.frames.get(t).and_then(|f| f.get(*p)).map(crate::drive::probe_value).unwrap_or_else(|| "?".into());
                        }
                        row
                    })
                    .collect();
                if verbose {
                    for (t, row) in values.iter().enumerate() {
                        let cols: Vec<String> = spec.probes.iter().zip(row).map(|(p, v)| format!("{}={v}", p.0)).collect();
                        println!("  t={:<3} {}", t + 1, cols.join(" "));
                    }
                }
            }
            SpecOp::Expect { probe, lo, hi, want } => {
                for t in *lo..=*hi {
                    let v = values.get(t as usize - 1).map(|r| r[*probe].as_str()).unwrap_or("?");
                    let ok = match want {
                        Want::On => v.parse::<u8>().is_ok_and(|x| x > 0),
                        Want::Off => v.parse::<u8>().is_ok_and(|x| x == 0),
                        Want::Level(l) => v.parse::<u8>().is_ok_and(|x| x == *l),
                        Want::State(s) => v == s,
                    };
                    if !ok {
                        bail!("{} at t={t} is {v}, want {want:?}", spec.probes[*probe].0);
                    }
                }
            }
        }
    }
    Ok(())
}

// ---- evaluation ---------------------------------------------------------------------------

/// How candidates are judged.
pub enum Target {
    /// The door: screen at a few positions, then the full judge. `max_open` / `max_close`
    /// (speed boards) reject a door that opens (closes) later than that many ticks in any
    /// cycle at the first position, as the leaderboard measures it.
    Door { screen: VerifyOptions, judge: VerifyOptions, max_open: Option<u32>, max_close: Option<u32> },
    /// A behavior spec at a few world offsets.
    Spec { spec: Spec, offsets: Vec<Pos> },
}

/// An error that means the worker itself is gone (not a verdict on the candidate).
fn infra(e: &anyhow::Error) -> bool {
    let m = format!("{e:#}");
    ["bridge exited", "Broken pipe", "OutOfMemoryError", "failed to fill whole buffer", "Connection reset"].iter().any(|k| m.contains(k))
}

/// Ok(verdict), or Err when the worker died.
fn screen(w: &mut Worker, sb: u32, d: &Design, target: &Target) -> Result<bool> {
    let r = match target {
        Target::Door { screen, .. } => screen_in(w, sb, d, screen),
        Target::Spec { spec, offsets } => offsets.iter().try_for_each(|off| run_spec(w, sb, &d.translated(*off), &spec.translated(*off), false)),
    };
    match r {
        Ok(()) => Ok(true),
        Err(e) if infra(&e) => Err(e),
        Err(_) => Ok(false),
    }
}

fn judge(w: &mut Worker, sb: u32, d: &Design, target: &Target) -> Result<String> {
    match target {
        Target::Door { judge, max_open, max_close, .. } => {
            // Fail fast: one cycle at every judge position (frame-less) before the full judge.
            let quick = VerifyOptions { cycles: 1, max_ticks: 200, offsets: judge.offsets.clone(), keep_frames: false, empty_walkway: judge.empty_walkway };
            screen_in(w, sb, d, &quick)?;
            let r = verify_in(w, sb, d, judge)?;
            let (o, c) = crate::search::worst_ticks(&r);
            if let Some(m) = max_open.filter(|m| o > *m) {
                bail!("too slow: opens after {o} ticks (max {m})");
            }
            if let Some(m) = max_close.filter(|m| c > *m) {
                bail!("too slow: closes after {c} ticks (max {m})");
            }
            Ok(format!("door works at {} positions, open {o} / close {c}", judge.offsets.len()))
        }
        Target::Spec { spec, offsets } => {
            for off in offsets {
                run_spec(w, sb, &d.translated(*off), &spec.translated(*off), false)?;
            }
            Ok(format!("spec holds at {} positions", offsets.len()))
        }
    }
}

/// Run the whole synthesis on enumerated `buckets` (see [`Gen::enumerate`]): screen size by
/// size, judge the survivors. Returns the verified circuits of the smallest size that has any (or
/// of every size with `all_sizes`).
#[allow(clippy::too_many_arguments)]
pub fn synth(
    pool: &Pool,
    base: &Design,
    en: &Gen,
    buckets: &[Vec<Vec<u32>>],
    target: &Target,
    min_k: usize,
    all_sizes: bool,
    want: usize,
    log: &(dyn Fn(&str) + Sync),
) -> Result<Vec<(Vec<u32>, Design, String)>> {
    let sandboxes = Sandboxes::new(pool);
    synth_in(pool, &sandboxes, base, en, buckets, target, min_k, all_sizes, want, log)
}

/// Per-worker sandboxes, created once and reused (resynth calls synth hundreds of times: one
/// set for all of them; dropping a used level can stall the bridge).
pub struct Sandboxes(Vec<Mutex<Option<u32>>>);

impl Sandboxes {
    pub fn new(pool: &Pool) -> Sandboxes {
        Sandboxes((0..pool.len()).map(|_| Mutex::new(None)).collect())
    }
    fn get(&self, w: &mut Worker, wi: usize) -> Result<u32> {
        let mut slot = self.0[wi].lock().unwrap();
        if let Some(sb) = *slot {
            return Ok(sb);
        }
        let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
        *slot = Some(sb);
        Ok(sb)
    }
}

/// [`synth`] with caller-owned sandboxes.
#[allow(clippy::too_many_arguments)]
pub fn synth_in(
    pool: &Pool,
    sandboxes: &Sandboxes,
    base: &Design,
    en: &Gen,
    buckets: &[Vec<Vec<u32>>],
    target: &Target,
    min_k: usize,
    all_sizes: bool,
    want: usize,
    log: &(dyn Fn(&str) + Sync),
) -> Result<Vec<(Vec<u32>, Design, String)>> {
    let mut found = Vec::new();
    for (size, cands) in buckets.iter().enumerate() {
        if cands.is_empty() || size < min_k {
            continue;
        }
        let t = Instant::now();
        let done = AtomicUsize::new(0);
        let passed: Mutex<Vec<usize>> = Mutex::new(Vec::new());
        let last = Mutex::new(Instant::now());
        // Small chunks when there are few candidates, so every worker gets some (a chunk of slow
        // candidates, e.g. clocks that run to the settle cap, must not hold up the batch). Each
        // worker pulls ranges from a shared queue; a worker whose JVM dies (out of memory: a
        // candidate that flies off) gives the rest of its range back and stops, so no candidate
        // is silently dropped.
        let chunk = (cands.len() / (pool.len().max(1) * 16)).clamp(1, 256);
        let queue: Mutex<std::collections::VecDeque<(usize, usize)>> =
            Mutex::new((0..cands.len()).step_by(chunk).map(|s| (s, (s + chunk).min(cands.len()))).collect());
        let crashed: Mutex<Vec<usize>> = Mutex::new(Vec::new());
        let errors: Vec<String> = pool
            .map((0..pool.len()).collect(), |w, wi, _| {
                loop {
                    let next = queue.lock().unwrap().pop_front();
                    let Some((s, e)) = next else { break };
                    for i in s..e {
                        let d = en.design(base, &cands[i]);
                        let verdict = sandboxes.get(w, wi).and_then(|sb| screen(w, sb, &d, target));
                        match verdict {
                            Ok(true) => passed.lock().unwrap().push(i),
                            Ok(false) => {}
                            Err(err) => {
                                crashed.lock().unwrap().push(i);
                                if i + 1 < e {
                                    queue.lock().unwrap().push_back((i + 1, e));
                                }
                                return Err(err.context(format!("worker {wi} died on {}", en.describe(&cands[i]))));
                            }
                        }
                    }
                    let n = done.fetch_add(e - s, Ordering::Relaxed) + (e - s);
                    let mut l = last.lock().unwrap();
                    if l.elapsed().as_secs_f64() > 20.0 {
                        *l = Instant::now();
                        let rate = n as f64 / t.elapsed().as_secs_f64();
                        log(&format!(
                            "  size {size}: {n}/{} screened ({rate:.0}/s, eta {:.0}s), {} passed",
                            cands.len(),
                            (cands.len().saturating_sub(n)) as f64 / rate.max(1.0),
                            passed.lock().unwrap().len()
                        ));
                    }
                }
                Ok(())
            })
            .into_iter()
            .filter_map(|r| r.err().map(|e| format!("{e:#}")))
            .collect();
        for e in &errors {
            log(&format!("  WORKER LOST: {e}"));
        }
        let left: usize = queue.lock().unwrap().iter().map(|(s, e)| e - s).sum();
        if left > 0 {
            bail!("every worker died with {left} candidates of size {size} left");
        }
        let mut pass = passed.into_inner().unwrap();
        pass.sort_unstable();
        log(&format!("size {size}: {} candidates screened in {:.1}s, {} passed the screen", cands.len(), t.elapsed().as_secs_f64(), pass.len()));
        // Judge survivors (a pool-sized chunk at a time).
        let mut verified = Vec::new();
        for part in pass.chunks(pool.len().max(1)) {
            let res = pool.map(part.to_vec(), |w, wi, i| {
                let d = en.design(base, &cands[i]);
                let sb = sandboxes.get(w, wi)?;
                Ok(judge(w, sb, &d, target).map(|m| (i, d, m)).map_err(|e| format!("{e:#}")))
            });
            for r in res {
                match r {
                    Ok(Ok((i, d, m))) => {
                        log(&format!("  VERIFIED size {size}: {} [{}] -- {m}", en.describe(&cands[i]), en.kinds(&cands[i])));
                        verified.push((cands[i].clone(), d, m));
                    }
                    Ok(Err(e)) => log(&format!("  judge rejected: {e}")),
                    Err(e) => log(&format!("  judge worker error: {e:#}")),
                }
            }
            if want > 0 && verified.len() + found.len() >= want {
                break;
            }
        }
        let any = !verified.is_empty();
        found.extend(verified);
        if (any && !all_sizes) || (want > 0 && found.len() >= want) {
            break;
        }
    }
    Ok(found)
}

// ---- resynthesis: replace any small sub-circuit of a door with a smaller one ---------------

/// The design as it really rests: placed like the judge does and settled, then every non-air
/// block of the area read back (unlike `canonicalize`, blocks that moved while settling are
/// kept at their new place).
pub fn settled_design(w: &mut Worker, d: &Design) -> Result<Design> {
    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<Design> {
        place_settled(w, sb, d, 400)?;
        let (lo, hi) = d.bounds().context("empty design")?;
        let r = w.get_region(sb, [lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3])?;
        let mut out = d.clone();
        out.blocks = r.blocks().into_iter().collect();
        Ok(out)
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// Cells of a door that change while it cycles (watch candidates for observers) and cells that
/// are air at rest but filled while it moves (no new block may go there).
pub fn door_motion(w: &mut Worker, d: &Design) -> Result<(HashSet<Pos>, HashSet<Pos>)> {
    let spec = DoorSpec::from_design(d)?;
    let sb = w.create(&mctas::sim::SandboxOptions { chunks: [-2, -2, 1, 1], seed: 0 })?;
    let r = (|| -> Result<(HashSet<Pos>, HashSet<Pos>)> {
        place_settled(w, sb, d, 400)?;
        let (wlo, whi) = crate::watch_box(d, &spec);
        let rest = w.get_region(sb, wlo, whi)?;
        let (mut changing, mut swept) = (HashSet::new(), HashSet::new());
        for _ in 0..2 {
            w.use_block(sb, spec.lever)?;
            let r = w.step_record(sb, 400, true, wlo, whi)?;
            for f in &r.frames {
                for (p, s) in f.iter() {
                    let before = rest.get(p).unwrap_or("?");
                    if before != s {
                        changing.insert(p);
                        if mctas::blockstate::is_air(before) {
                            swept.insert(p);
                        }
                    }
                }
            }
        }
        Ok((changing, swept))
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// Knobs of [`resynth`].
pub struct ResynthOptions {
    pub min_size: usize,
    pub max_size: usize,
    pub palette: Vec<String>,
    pub floors: Vec<String>,
    /// Box = the group's bounding box grown by this.
    pub margin: i32,
    /// Skip a group whose enumeration exceeds this many circuits.
    pub max_cands: usize,
    pub loose: bool,
    /// Speed bounds for the judge (see [`Target::Door`]).
    pub max_open: Option<u32>,
    pub max_close: Option<u32>,
    /// Write every improvement here as soon as it is found (the design as judged).
    pub save_to: Option<std::path::PathBuf>,
    /// After an improvement, search every group again (default: only groups whose
    /// neighborhood changed).
    pub global_retry: bool,
    /// See [`SynthOptions::require_any`].
    pub require_any: Vec<String>,
    /// Only groups whose blocks all lie in this box (inclusive corners): focus a large-group
    /// run on one region.
    pub only: Option<(Pos, Pos)>,
}

/// Blocks resynthesis may take out: wiring (dust, repeaters, comparators, observers, torches,
/// lamps, copper bulbs, note blocks, targets) and plain blocks that do not move; never the door,
/// the lever or what holds it up, pistons, slime/honey or redstone blocks.
fn removable(d: &Design, changing: &HashSet<Pos>) -> Vec<Pos> {
    let spec = DoorSpec::from_design(d).ok();
    let door: HashSet<Pos> = spec.map(|s| s.door_cells().into_iter().collect()).unwrap_or_default();
    let lever = spec.map(|s| s.lever);
    let lever_support = lever.and_then(|l| d.blocks.get(&l).and_then(|s| Node::new(l, s).support));
    d.blocks
        .iter()
        .filter(|(p, s)| {
            let n = Node::new(**p, s);
<<<<<<< HEAD
            let wiring = matches!(n.kind, Kind::Dust | Kind::Repeater | Kind::Comparator | Kind::Observer | Kind::Torch | Kind::Lamp)
                || s.contains("note_block")
                || s.contains("minecraft:target");
=======
            let wiring = matches!(n.kind, Kind::Dust | Kind::Repeater | Kind::Comparator | Kind::Observer | Kind::Torch | Kind::Lamp | Kind::Sensor);
>>>>>>> origin/explore/sculk-2
            let still_block = matches!(n.kind, Kind::Block) && !changing.contains(*p);
            let glass = n.kind == Kind::Inert && s.contains("glass");
            (wiring || still_block || glass) && !door.contains(*p) && Some(**p) != lever && Some(**p) != lever_support
        })
        .map(|(p, _)| *p)
        .collect()
}

/// Connected groups (faces or a dust-style diagonal step) of `cells` with sizes in lo..=hi.
fn groups(cells: &[Pos], lo: usize, hi: usize) -> Vec<Vec<Pos>> {
    let set: HashSet<Pos> = cells.iter().copied().collect();
    let linked = |a: Pos, b: Pos| {
        let d = [(a[0] - b[0]).abs(), (a[1] - b[1]).abs(), (a[2] - b[2]).abs()];
        d[0] + d[1] + d[2] == 1 || (d[1] == 1 && d[0] + d[2] == 1)
    };
    let mut seen: HashSet<Vec<Pos>> = HashSet::new();
    let mut frontier: Vec<Vec<Pos>> = cells.iter().map(|c| vec![*c]).collect();
    let mut out = Vec::new();
    for size in 1..=hi {
        let mut next = Vec::new();
        for g in &frontier {
            if size >= lo {
                out.push(g.clone());
            }
            if size == hi {
                continue;
            }
            for c in &set {
                if g.contains(c) || !g.iter().any(|x| linked(*x, *c)) {
                    continue;
                }
                let mut h = g.clone();
                h.push(*c);
                h.sort();
                if seen.insert(h.clone()) {
                    next.push(h);
                }
            }
        }
        frontier = next;
    }
    out
}

/// Large-neighborhood search by exact synthesis: for every connected group of removable blocks
/// (sizes `min_size..=max_size`), clear it and synthesize every circuit of fewer blocks that
/// could reconnect the cells around it; a replacement the judge accepts is applied and the search
/// restarts. Returns the best design found (the input if nothing improves).
pub fn resynth(pool: &Pool, design: &Design, o: &ResynthOptions, judge_opts: &VerifyOptions, screen_opts: &VerifyOptions, log: &(dyn Fn(&str) + Sync)) -> Result<Design> {
    let mut cur = design.clone();
    let mut tried: HashSet<(u64, Vec<Pos>)> = HashSet::new();
    let sandboxes = Sandboxes::new(pool);
    // A group's candidate circuits depend only on the blocks around it (box = bounding box +
    // margin, sinks = pistons within 3): after an improvement elsewhere, a group whose
    // neighborhood is unchanged is not searched again (unless --global-retry).
    let local_key = |d: &Design, g: &[Pos]| -> u64 {
        let r = o.margin + 3;
        let mut near: Vec<(&Pos, &String)> = d.blocks.iter().filter(|(p, _)| g.iter().any(|c| (0..3).all(|i| (p[i] - c[i]).abs() <= r))).collect();
        near.sort();
        let mut hs = DefaultHasher::new();
        near.hash(&mut hs);
        hs.finish()
    };
    'outer: loop {
        let (changing, swept) = {
            let mut w = pool.worker(0);
            door_motion(&mut w, &cur)?
        };
        let rem = removable(&cur, &changing);
        let mut gs = groups(&rem, o.min_size.max(1), o.max_size);
        if let Some((lo, hi)) = o.only {
            gs.retain(|g| g.iter().all(|p| (0..3).all(|i| p[i] >= lo[i] && p[i] <= hi[i])));
        }
        let h = crate::search::design_hash(&cur);
        log(&format!("{} blocks: {} removable, {} groups of {}..{}", crate::block_count(&cur), rem.len(), gs.len(), o.min_size, o.max_size));
        for g in gs {
            let key = if o.global_retry { h } else { local_key(&cur, &g) };
            if !tried.insert((key, g.clone())) {
                continue;
            }
            let mut base = cur.clone();
            for p in &g {
                base.blocks.remove(p);
            }
            let near = |p: &Pos, r: i32| g.iter().any(|c| (0..3).all(|i| (p[i] - c[i]).abs() <= r));
            let mut ctx_cells: Vec<Pos> = base.blocks.keys().filter(|p| near(p, 1)).copied().collect();
            ctx_cells.sort();
            // Sinks also: pistons near the group and the cells above them (quasi-connectivity
            // through an air cell, e.g. a repeater pointing into the cell above B).
            let mut sinks = ctx_cells.clone();
            for (p, s) in &base.blocks {
                let n = short(mctas::blockstate::block_name(s));
                if (n == "piston" || n == "sticky_piston") && near(p, 3) {
                    for c in [*p, add(*p, [0, 1, 0])] {
                        if !sinks.contains(&c) {
                            sinks.push(c);
                        }
                    }
                }
            }
            let watch: Vec<Pos> = changing.iter().filter(|p| near(p, 2) && !base.blocks.contains_key(*p)).copied().collect();
            let mut lo = g[0];
            let mut hi = g[0];
            for p in &g {
                for i in 0..3 {
                    lo[i] = lo[i].min(p[i] - o.margin);
                    hi[i] = hi[i].max(p[i] + o.margin);
                }
            }
            // A flat design (every block in the door plane) stays flat: no circuit block off it.
            if crate::single_layer(&base) {
                if let Some(door) = base.ports.get("door") {
                    lo[2] = door[2];
                    hi[2] = door[2];
                }
            }
            let so = SynthOptions {
                lo,
                hi,
                palette: o.palette.clone(),
                floors: o.floors.clone(),
                k: g.len() - 1,
                min_k: 1,
                sources: ctx_cells.clone(),
                watch,
                sinks,
                forbid: swept.clone(),
                require: Vec::new(),
                limit: Vec::new(),
                require_any: o.require_any.clone(),
                loose: o.loose,
                max_cands: o.max_cands,
            };
            // 4-letter tags; "redstone_" stripped so wire / lamp / torch / block stay distinct.
            let tag = |s: &str| short(mctas::blockstate::block_name(s)).trim_start_matches("redstone_").chars().take(4).collect::<String>();
            let desc = g.iter().map(|p| format!("{}{}", fmt_pos(*p), tag(&cur.blocks[p]))).collect::<Vec<_>>().join(" ");
            let en = match Gen::new(&base, &so) {
                Ok(e) => e,
                Err(e) => {
                    log(&format!("  group {desc}: {e:#}"));
                    continue;
                }
            };
            let quiet = |_: &str| {};
            let buckets = match en.enumerate(&quiet) {
                Ok(b) => b,
                Err(e) => {
                    log(&format!("  group {desc}: skipped ({e:#})"));
                    continue;
                }
            };
            let n: usize = buckets.iter().map(|b| b.len()).sum();
            let target = Target::Door { screen: screen_opts.clone(), judge: judge_opts.clone(), max_open: o.max_open, max_close: o.max_close };
            let t = Instant::now();
            let inner = |m: &str| {
                if m.contains("VERIFIED") || (m.contains("passed the screen") && !m.ends_with(" 0 passed the screen")) {
                    log(&format!("    {}", m.trim()));
                }
            };
            let found = synth_in(pool, &sandboxes, &base, &en, &buckets, &target, 1, false, 1, &inner)?;
            log(&format!("  group [{desc}] -> <= {} blocks: {n} circuits in {:.1}s{}", g.len() - 1, t.elapsed().as_secs_f64(), if found.is_empty() { String::new() } else { " FOUND".into() }));
            if let Some((set, d, msg)) = found.into_iter().next() {
                log(&format!("IMPROVED {} -> {} blocks: [{desc}] replaced by {} -- {msg}", crate::block_count(&cur), crate::block_count(&d), en.describe(&set)));
                if let Some(path) = &o.save_to {
                    match d.save(path) {
                        Ok(()) => log(&format!("  saved to {}", path.display())),
                        Err(e) => log(&format!("  could not save to {}: {e:#}", path.display())),
                    }
                }
                cur = d;
                continue 'outer;
            }
        }
        return Ok(cur);
    }
}
