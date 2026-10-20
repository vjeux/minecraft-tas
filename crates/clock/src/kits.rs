//! `mctas clock kit`: structured censuses of ENTITY clocks on redsim. The generic `clock census`
//! fills a box with up to 4-5 blocks; the smallest entity clocks have 8 to 20 pieces (a dripleaf
//! with an armor stand, a minecart ping-pong, boat bouncers), so a kit enumerates ONE mechanism's
//! design space instead, in its natural coordinates, one candidate per mirror class:
//!
//! - `shuttle` (K1): a straight east-west track of 2..=`max_len` rails (plain, powered or
//!   detector), each over stone or a redstone block (which powers a powered rail above it), a
//!   stone bumper or nothing at each end, one empty minecart on a rail cell at rest or moving at
//!   0.4 b/t either way. Needs a detector rail (the only block that can change) and a powered rail
//!   over a redstone block (else the cart stops). The output port is the first detector rail.
//!
//! Every candidate is screened (`sim::entity_screen`: comes to rest, or no block change late in a
//! 1000 gt run, drops it), then judged by `sim::simulate` (for designs with entities, the
//! real-game judge's rule: block frames periodic for 3 periods within 4000 gt). A clock is
//! minimal when no single block can go with its period kept. Outputs: `clocks.tsv`, `summary.md`,
//! and `designs/` (every minimal clock, the smallest first per (period, family)).

use crate::sim::{SimOptions, Verdict, entity_screen, simulate};
use crate::{Clock, Design, Pos, canon, wave};
use anyhow::{Result, bail};
use mctas::design::EntitySpec;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

/// Options of [`run`].
#[derive(Clone, Debug)]
pub struct KitOptions {
    pub kit: String,
    /// The longest track (`shuttle`).
    pub max_len: usize,
    pub workers: usize,
    pub out: PathBuf,
    /// Ticks of the screen.
    pub screen: u32,
}

/// A clock a kit found.
#[derive(Clone, Debug)]
pub struct KitClock {
    pub period: u32,
    pub warmup: u32,
    pub family: String,
    pub blocks: usize,
    pub dims: [i32; 3],
    pub on: u32,
    pub edges: u32,
    /// Blocks' type key (`canon::type_key`) + the entities.
    pub type_key: String,
    pub minimal: bool,
    pub design: Design,
}

/// One kit's candidate space: `len` candidates, `design(i)` the i-th (None: not the canonical
/// member of its mirror class, or ruled out).
trait Space: Sync {
    fn len(&self) -> u64;
    fn design(&self, i: u64) -> Option<Design>;
}

const SHUTTLE_RAILS: [&str; 3] = [
    "minecraft:rail[shape=east_west]",
    "minecraft:powered_rail[shape=east_west]",
    "minecraft:detector_rail[shape=east_west]",
];
const SUPPORTS: [&str; 2] = ["minecraft:stone", "minecraft:redstone_block"];

/// K1: see the module doc. Index layout per track length L: motion (3), cart cell (L), bumpers
/// (4), then each cell's (rail, support) (6) from x = 0.
struct Shuttle {
    /// (L, first index, count).
    lens: Vec<(usize, u64, u64)>,
}

impl Shuttle {
    fn new(max_len: usize) -> Shuttle {
        let mut lens = Vec::new();
        let mut first = 0u64;
        for l in 2..=max_len {
            let n = 3 * l as u64 * 4 * 6u64.pow(l as u32);
            lens.push((l, first, n));
            first += n;
        }
        Shuttle { lens }
    }
}

/// A shuttle candidate in its own terms (mirror: x -> L - 1 - x, bumpers swapped, motion negated).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ShuttleKey {
    cells: Vec<(u8, u8)>,
    bumpers: (u8, u8),
    cart: usize,
    motion: u8,
}

impl ShuttleKey {
    fn mirror(&self) -> ShuttleKey {
        let l = self.cells.len();
        ShuttleKey {
            cells: self.cells.iter().rev().copied().collect(),
            bumpers: (self.bumpers.1, self.bumpers.0),
            cart: l - 1 - self.cart,
            motion: match self.motion {
                1 => 2,
                2 => 1,
                m => m,
            },
        }
    }
}

impl Space for Shuttle {
    fn len(&self) -> u64 {
        self.lens.iter().map(|x| x.2).sum()
    }

    fn design(&self, i: u64) -> Option<Design> {
        let &(l, first, _) = self.lens.iter().find(|(_, f, n)| i >= *f && i < f + n)?;
        let mut r = i - first;
        let motion = (r % 3) as u8;
        r /= 3;
        let cart = (r % l as u64) as usize;
        r /= l as u64;
        let b = (r % 4) as u8;
        r /= 4;
        let mut cells = Vec::with_capacity(l);
        for _ in 0..l {
            let o = (r % 6) as u8;
            r /= 6;
            cells.push((o % 3, o / 3));
        }
        let key = ShuttleKey { cells, bumpers: (b % 2, b / 2), cart, motion };
        // a detector rail (the only block that changes) and a powered rail over a redstone block;
        // a redstone block under a plain or detector rail powers nothing a stone would not (one
        // type: the stone one)
        if !key.cells.iter().any(|c| c.0 == 2) || !key.cells.iter().any(|c| *c == (1, 1)) || key.cells.iter().any(|c| c.1 == 1 && c.0 != 1) {
            return None;
        }
        if key.mirror() < key {
            return None;
        }
        let mut d = Design { name: format!("kit-shuttle-{l}-{i}"), ..Default::default() };
        for (x, (rail, sup)) in key.cells.iter().enumerate() {
            d.blocks.insert([x as i32, 1, 0], SHUTTLE_RAILS[*rail as usize].to_string());
            d.blocks.insert([x as i32, 0, 0], SUPPORTS[*sup as usize].to_string());
        }
        if key.bumpers.0 == 1 {
            d.blocks.insert([-1, 1, 0], "minecraft:stone".into());
        }
        if key.bumpers.1 == 1 {
            d.blocks.insert([l as i32, 1, 0], "minecraft:stone".into());
        }
        let nbt = match key.motion {
            1 => Some("{Motion:[0.4d,0.0d,0.0d]}".to_string()),
            2 => Some("{Motion:[-0.4d,0.0d,0.0d]}".to_string()),
            _ => None,
        };
        d.entities.push(EntitySpec { kind: "minecraft:minecart".into(), pos: [key.cart as f64 + 0.5, 1.0625, 0.5], nbt });
        let det = key.cells.iter().position(|c| c.0 == 2)?;
        d.ports.insert("output".into(), [det as i32, 1, 0]);
        Some(d)
    }
}

fn space(o: &KitOptions) -> Result<Box<dyn Space>> {
    Ok(match o.kit.as_str() {
        "shuttle" => Box::new(Shuttle::new(o.max_len)),
        "bouncer" => Box::new(Bouncer::new()),
        "dripleaf" => Box::new(Dripleaf::new()),
        other => bail!("unknown kit {other:?} (shuttle, bouncer, dripleaf)"),
    })
}

/// K2: an entity on a bubble column, a tripwire across its flight. Parameters: entity (oak boat
/// on a 2x2 pool, item, armor stand), pool (1x1, 2x2), base (soul sand: up, magma: down), the
/// other cells of a 2x2 pool (water sources or air), depth (1, 2 water blocks in the column),
/// tripwire height above the water (1..=4), its line (along x or z, through the first or second
/// row of a 2x2 pool; hooks just outside the pool on stone), a cap over the source column (none,
/// stone, top or bottom slab, top trapdoor, hanging lantern; 1 or 2 above the string). Every pool
/// cell is walled with stone. The output port is the first hook.
struct Bouncer {
    all: Vec<[u8; 9]>,
}

const BOUNCER_ENTITIES: [&str; 3] = ["minecraft:oak_boat", "minecraft:item", "minecraft:armor_stand"];
/// A kit's item: never picked up (PickupDelay 32767) and never despawning (Age -32768: an item
/// otherwise despawns after 6000 gt, which would stop the clock after 5 minutes).
const ITEM_NBT: &str = "{Item:{id:\"minecraft:stone\",count:1},PickupDelay:32767s,Age:-32768s}";
const BOUNCER_CAPS: [&str; 6] = [
    "",
    "minecraft:stone",
    "minecraft:stone_slab[type=top]",
    "minecraft:stone_slab[type=bottom]",
    "minecraft:oak_trapdoor[facing=north,half=top,open=false]",
    "minecraft:lantern[hanging=true]",
];

impl Bouncer {
    fn new() -> Bouncer {
        let mut all = Vec::new();
        for entity in 0..3u8 {
            for pool in 0..2u8 {
                for base in 0..2u8 {
                    for fill in 0..(if pool == 1 { 2u8 } else { 1 }) {
                        for depth in 1..=2u8 {
                            for h in 1..=4u8 {
                                for axis in 0..2u8 {
                                    for row in 0..(if pool == 1 { 2u8 } else { 1 }) {
                                        for cap in 0..6u8 {
                                            for c in 1..=(if cap == 0 { 1u8 } else { 2 }) {
                                                all.push([entity, pool, base, fill, depth, h, axis * 2 + row, cap, c]);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Bouncer { all }
    }
}

impl Space for Bouncer {
    fn len(&self) -> u64 {
        self.all.len() as u64
    }

    fn design(&self, i: u64) -> Option<Design> {
        let [entity, pool, base, fill, depth, h, line, cap, c] = self.all[i as usize];
        let n = if pool == 1 { 2 } else { 1 };
        let (axis, row) = (line / 2, (line % 2) as i32);
        let mut d = Design { name: format!("kit-bouncer-{i}"), ..Default::default() };
        let top = 0;
        let bottom = top - depth as i32 + 1;
        for x in 0..n {
            for z in 0..n {
                let source = x == 0 && z == 0;
                for y in bottom..=top {
                    let water = source || (fill == 1 && y == top);
                    if water {
                        d.blocks.insert([x, y, z], "minecraft:water[level=0]".into());
                    }
                }
                // the floor: the base under the source column, stone elsewhere (under the top
                // layer when the other cells are air or a single layer)
                let floor_y = if source || fill == 1 { bottom - 1 } else { top - 1 };
                d.blocks.insert([x, floor_y, z], if source { if base == 0 { "minecraft:soul_sand" } else { "minecraft:magma_block" } } else { "minecraft:stone" }.into());
                if !source && fill == 0 {
                    for y in bottom - 1..top - 1 {
                        d.blocks.insert([x, y, z], "minecraft:stone".into());
                    }
                }
            }
        }
        // walls around the pool, every water layer
        for y in bottom..=top {
            for k in 0..n {
                for p in [[-1, y, k], [n, y, k], [k, y, -1], [k, y, n]] {
                    d.blocks.insert(p, "minecraft:stone".into());
                }
            }
        }
        // the tripwire: along z (axis 0) at x = row, or along x (axis 1) at z = row
        let hy = top + h as i32;
        let (cells, hook_a, hook_b, sup_a, sup_b, fa, fb, conn): (Vec<Pos>, Pos, Pos, Pos, Pos, &str, &str, &str) = if axis == 0 {
            ((0..n).map(|z| [row, hy, z]).collect(), [row, hy, -1], [row, hy, n], [row, hy, -2], [row, hy, n + 1], "south", "north", "east=false,north=true,south=true,west=false")
        } else {
            ((0..n).map(|x| [x, hy, row]).collect(), [-1, hy, row], [n, hy, row], [-2, hy, row], [n + 1, hy, row], "east", "west", "east=true,north=false,south=false,west=true")
        };
        if row >= n {
            return None;
        }
        for p in &cells {
            d.blocks.insert(*p, format!("minecraft:tripwire[attached=true,disarmed=false,{conn},powered=false]"));
        }
        d.blocks.insert(hook_a, format!("minecraft:tripwire_hook[attached=true,facing={fa},powered=false]"));
        d.blocks.insert(hook_b, format!("minecraft:tripwire_hook[attached=true,facing={fb},powered=false]"));
        d.blocks.insert(sup_a, "minecraft:stone".into());
        d.blocks.insert(sup_b, "minecraft:stone".into());
        if cap > 0 {
            let p = [0, hy + c as i32, 0];
            if d.blocks.contains_key(&p) {
                return None;
            }
            d.blocks.insert(p, BOUNCER_CAPS[cap as usize].to_string());
        }
        let centre = n as f64 / 2.0;
        let (kind, nbt) = match entity {
            1 => (BOUNCER_ENTITIES[1], Some(ITEM_NBT.to_string())),
            e => (BOUNCER_ENTITIES[e as usize], None),
        };
        // the boat needs the 2x2 pool (it is 1.375 wide)
        if entity == 0 && pool == 0 {
            return None;
        }
        d.entities.push(EntitySpec { kind: kind.into(), pos: [centre, (top + 1) as f64, centre], nbt });
        d.ports.insert("output".into(), hook_a);
        Some(d)
    }
}

/// K3: a big dripleaf (facing north, on dirt) with an entity resting on its leaf (armor stand,
/// item, oak boat), an observer next to the leaf watching it (west, east or south), and more blocks
/// in the box x -2..=2, y -1..=2, z -1..=2 around the dirt, one of them on the observer's back
/// cell (taking its output): up to 3 of redstone dust, stone or a delay-1 repeater any way, or 4
/// of dust and stone (clk-misc-dripleaf-stand's own shape: two dusts stepping down to the dirt,
/// each on stone). One per mirror class (x -> -x). The output port is the observer.
struct Dripleaf {
    /// The free cells per observer side.
    free: Vec<Vec<Pos>>,
    /// (observer side, combinations of free-cell indices holding the back cell, states per cell,
    /// candidates = combinations x states^k x 3 entities), in index order.
    groups: Vec<(u8, Vec<Vec<usize>>, usize, u64)>,
}

const DRIP_FILL: [&str; 6] = [
    "minecraft:redstone_wire[east=side,north=side,south=side,west=side]",
    "minecraft:stone",
    "minecraft:repeater[delay=1,facing=north]",
    "minecraft:repeater[delay=1,facing=east]",
    "minecraft:repeater[delay=1,facing=south]",
    "minecraft:repeater[delay=1,facing=west]",
];

/// Observer next to the leaf at (0, 1, 0): (position, facing toward the leaf, back cell). Not above
/// it: the entity rests there.
const DRIP_OBS: [(Pos, &str, Pos); 3] = [([-1, 1, 0], "east", [-2, 1, 0]), ([1, 1, 0], "west", [2, 1, 0]), ([0, 1, 1], "north", [0, 1, 2])];

/// Every k-subset of 0..n, in lexicographic order.
fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    if k > n {
        return out;
    }
    let mut c: Vec<usize> = (0..k).collect();
    loop {
        out.push(c.clone());
        let mut i = k;
        loop {
            if i == 0 {
                return out;
            }
            i -= 1;
            if c[i] < n - k + i {
                c[i] += 1;
                for j in i + 1..k {
                    c[j] = c[j - 1] + 1;
                }
                break;
            }
        }
    }
}

impl Dripleaf {
    fn new() -> Dripleaf {
        let mut free_all = Vec::new();
        let mut groups = Vec::new();
        for (oi, (opos, _, back)) in DRIP_OBS.iter().enumerate() {
            let mut free: Vec<Pos> = Vec::new();
            for y in -1..=2 {
                for z in -1..=2 {
                    for x in -2..=2 {
                        let p = [x, y, z];
                        if p != [0, 1, 0] && p != [0, 0, 0] && p != *opos && p != [0, 2, 0] {
                            free.push(p);
                        }
                    }
                }
            }
            let bi = free.iter().position(|p| p == back).expect("the back cell is free");
            let others: Vec<usize> = (0..free.len()).filter(|&i| i != bi).collect();
            for (k, states) in [(1usize, 6usize), (2, 6), (3, 6), (4, 2)] {
                let combos: Vec<Vec<usize>> = subsets(others.len(), k - 1).into_iter().map(|s| std::iter::once(bi).chain(s.into_iter().map(|j| others[j])).collect()).collect();
                let n = combos.len() as u64 * (states as u64).pow(k as u32) * 3;
                groups.push((oi as u8, combos, states, n));
            }
            free_all.push(free);
        }
        Dripleaf { free: free_all, groups }
    }
}

impl Space for Dripleaf {
    fn len(&self) -> u64 {
        self.groups.iter().map(|g| g.3).sum()
    }

    fn design(&self, i: u64) -> Option<Design> {
        let mut r = i;
        let mut g = None;
        for grp in &self.groups {
            if r < grp.3 {
                g = Some(grp);
                break;
            }
            r -= grp.3;
        }
        let (oi, combos, states, _) = g?;
        let entity = (r % 3) as u8;
        r /= 3;
        let k = combos[0].len();
        let per = (*states as u64).pow(k as u32);
        let mut s = r % per;
        let combo = &combos[(r / per) as usize];
        let free = &self.free[*oi as usize];
        let mut cells: Vec<(Pos, u8)> = Vec::with_capacity(k);
        for &ci in combo {
            cells.push((free[ci], (s % *states as u64) as u8));
            s /= *states as u64;
        }
        let (opos, ofacing, _) = DRIP_OBS[*oi as usize];
        let mut blocks: BTreeMap<Pos, String> = BTreeMap::new();
        blocks.insert([0, 1, 0], "minecraft:big_dripleaf[facing=north,tilt=none]".into());
        blocks.insert([0, 0, 0], "minecraft:dirt".into());
        blocks.insert(opos, format!("minecraft:observer[facing={ofacing}]"));
        for (p, st) in &cells {
            blocks.insert(*p, DRIP_FILL[*st as usize].to_string());
        }
        // one per mirror class: x -> -x (east <-> west)
        let mirror: BTreeMap<Pos, String> = blocks
            .iter()
            .map(|(p, s)| ([-p[0], p[1], p[2]], s.replace("facing=east", "facing=TMP").replace("facing=west", "facing=east").replace("facing=TMP", "facing=west")))
            .collect();
        let enc = |m: &BTreeMap<Pos, String>| m.iter().map(|(p, s)| format!("{p:?}={s}")).collect::<Vec<_>>().join(";");
        if enc(&mirror) < enc(&blocks) {
            return None;
        }
        let mut d = Design { name: format!("kit-dripleaf-{i}"), blocks, ..Default::default() };
        let (kind, nbt) = match entity {
            0 => ("minecraft:armor_stand", None),
            1 => ("minecraft:item", Some(ITEM_NBT.to_string())),
            _ => ("minecraft:oak_boat", None),
        };
        d.entities.push(EntitySpec { kind: kind.into(), pos: [0.5, 1.9375, 0.5], nbt });
        d.ports.insert("output".into(), opos);
        Some(d)
    }
}

/// The entities part of a type key: their kinds, sorted (where an entity starts and how it moves
/// first is the start of a run, not the machine: the run's representative is the one with the
/// shortest warmup).
fn entity_key(d: &Design) -> String {
    let mut v: Vec<&str> = d.entities.iter().map(|e| e.kind.as_str()).collect();
    v.sort();
    v.join(";")
}

const SIM: SimOptions = SimOptions { horizon: 3000, margin: 2 };

fn judge(d: Design, screen: u32) -> Option<KitClock> {
    let c = Clock::from_design(d).ok()?;
    if !entity_screen(&c, SIM.margin, screen) {
        return None;
    }
    let r = simulate(&c, &SIM);
    if r.verdict != Verdict::Clock {
        return None;
    }
    let (on, edges) = r.output.as_ref().map(|w| (w.wave.on, w.wave.edges)).unwrap_or_default();
    Some(KitClock {
        period: r.period,
        warmup: r.warmup,
        family: wave::signature(&r.analysis.family),
        blocks: c.blocks(),
        dims: r.dims(),
        on,
        edges,
        type_key: format!("{}|{}", canon::type_key(c.design.blocks.iter()), entity_key(&c.design)),
        minimal: false,
        design: c.design.clone(),
    })
}

/// No single block can go with the period kept.
fn minimal(k: &KitClock) -> bool {
    k.design.blocks.keys().all(|p| {
        if k.design.ports.get("output") == Some(p) {
            return true;
        }
        let mut d2 = k.design.clone();
        d2.blocks.remove(p);
        match Clock::from_design(d2) {
            Ok(c2) => {
                let r2 = simulate(&c2, &SIM);
                !(r2.verdict == Verdict::Clock && r2.period == k.period)
            }
            Err(_) => true,
        }
    })
}

/// Run a kit (see the module doc): every clock's representative per type (blocks + entity kinds;
/// entities at rest first, then the shortest warmup), with its minimality; `progress(done, total,
/// clocks)` now and then.
pub fn run(o: &KitOptions, progress: &(dyn Fn(u64, u64, usize) + Sync)) -> Result<Vec<KitClock>> {
    let sp = space(o)?;
    let total = sp.len();
    let next = AtomicU64::new(0);
    let found: Mutex<Vec<KitClock>> = Mutex::new(Vec::new());
    const CHUNK: u64 = 2048;
    std::thread::scope(|s| {
        for _ in 0..o.workers.max(1) {
            s.spawn(|| {
                loop {
                    let start = next.fetch_add(CHUNK, Ordering::Relaxed);
                    if start >= total {
                        break;
                    }
                    let end = (start + CHUNK).min(total);
                    let mut local = Vec::new();
                    for i in start..end {
                        if let Some(d) = sp.design(i)
                            && let Some(k) = judge(d, o.screen)
                        {
                            local.push(k);
                        }
                    }
                    let n = {
                        let mut f = found.lock().unwrap();
                        f.extend(local);
                        f.len()
                    };
                    if (start / CHUNK) % 256 == 0 {
                        progress(end, total, n);
                    }
                }
            });
        }
    });
    // one representative per (type, period, family)
    let mut reps: BTreeMap<(String, u32, String), KitClock> = BTreeMap::new();
    for k in found.into_inner().unwrap() {
        let key = (k.type_key.clone(), k.period, k.family.clone());
        let data = |x: &KitClock| x.design.entities.iter().filter(|e| e.nbt.is_some()).count();
        // a start a player can build (entities at rest, no data) first, then the shortest warmup
        if reps.get(&key).is_none_or(|b| (data(&k), k.warmup, &k.design.name) < (data(b), b.warmup, &b.design.name)) {
            reps.insert(key, k);
        }
    }
    let mut all: Vec<KitClock> = reps.into_values().collect();
    // minimality, in parallel
    let next = AtomicU64::new(0);
    let flags: Vec<std::sync::atomic::AtomicBool> = (0..all.len()).map(|_| std::sync::atomic::AtomicBool::new(false)).collect();
    std::thread::scope(|s| {
        for _ in 0..o.workers.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed) as usize;
                    if i >= all.len() {
                        break;
                    }
                    flags[i].store(minimal(&all[i]), Ordering::Relaxed);
                }
            });
        }
    });
    for (k, f) in all.iter_mut().zip(&flags) {
        k.minimal = f.load(Ordering::Relaxed);
    }
    all.sort_by(|a, b| (a.period, &a.family, a.blocks, &a.type_key).cmp(&(b.period, &b.family, b.blocks, &b.type_key)));
    Ok(all)
}

/// Remove blocks one at a time (first to last) while the clock keeps its period and family, until
/// none can go: a minimal clock.
fn shrink(k: &KitClock) -> KitClock {
    let mut cur = k.clone();
    'outer: loop {
        let cells: Vec<Pos> = cur.design.blocks.keys().copied().collect();
        for p in cells {
            if cur.design.ports.get("output") == Some(&p) {
                continue;
            }
            let mut d2 = cur.design.clone();
            d2.blocks.remove(&p);
            let Ok(c2) = Clock::from_design(d2) else { continue };
            let r2 = simulate(&c2, &SIM);
            if r2.verdict == Verdict::Clock && r2.period == cur.period && wave::signature(&r2.analysis.family) == cur.family {
                let (on, edges) = r2.output.as_ref().map(|w| (w.wave.on, w.wave.edges)).unwrap_or_default();
                cur = KitClock {
                    period: r2.period,
                    warmup: r2.warmup,
                    family: cur.family.clone(),
                    blocks: c2.blocks(),
                    dims: r2.dims(),
                    on,
                    edges,
                    type_key: format!("{}|{}", canon::type_key(c2.design.blocks.iter()), entity_key(&c2.design)),
                    minimal: false,
                    design: c2.design.clone(),
                };
                continue 'outer;
            }
        }
        break;
    }
    cur.minimal = true;
    cur
}

/// Per period and entity kinds, the smallest clock (fewest blocks, then the smallest box, then the
/// type key): the
/// 3 smallest types are shrunk to minimal clocks ([`shrink`]) and the smallest result is kept.
pub fn best_per_period(o: &KitOptions, all: &[KitClock]) -> Vec<KitClock> {
    let vol = |k: &KitClock| k.dims[0] * k.dims[1] * k.dims[2];
    // per period and entity kinds (an armor stand, an item or a boat on the same leaf are three
    // clocks)
    let mut per: BTreeMap<(u32, String), Vec<&KitClock>> = BTreeMap::new();
    for k in all {
        per.entry((k.period, entity_key(&k.design))).or_default().push(k);
    }
    let jobs: Vec<Vec<&KitClock>> = per
        .into_values()
        .map(|mut v| {
            v.sort_by(|a, b| (a.blocks, vol(a), &a.type_key).cmp(&(b.blocks, vol(b), &b.type_key)));
            v.truncate(3);
            v
        })
        .collect();
    let next = AtomicU64::new(0);
    let out: Mutex<Vec<KitClock>> = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..o.workers.max(1) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed) as usize;
                    if i >= jobs.len() {
                        break;
                    }
                    let best = jobs[i].iter().map(|k| if k.minimal { (*k).clone() } else { shrink(k) }).min_by(|a, b| (a.blocks, vol(a), &a.type_key).cmp(&(b.blocks, vol(b), &b.type_key)));
                    if let Some(b) = best {
                        out.lock().unwrap().push(b);
                    }
                }
            });
        }
    });
    let mut v = out.into_inner().unwrap();
    v.sort_by_key(|k| k.period);
    v
}

/// `clocks.tsv` (every clock type), `summary.md` (per (period, family): types, minimal types, the
/// smallest) and `designs/`: the minimal clock of each period from [`best_per_period`]:
/// `kit-<kit>-p<P>-<n>b.txt`.
pub fn write_outputs(o: &KitOptions, all: &[KitClock], best: &[KitClock]) -> Result<(usize, usize)> {
    std::fs::create_dir_all(&o.out)?;
    let mut tsv = String::from("period\twarmup\tfamily\tblocks\tdims\ton\tedges\tminimal\ttype\tname\n");
    for k in all {
        tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}x{}x{}\t{}\t{}\t{}\t{}\t{}\n",
            k.period, k.warmup, k.family, k.blocks, k.dims[0], k.dims[1], k.dims[2], k.on, k.edges, k.minimal, k.type_key, k.design.name
        ));
    }
    std::fs::write(o.out.join("clocks.tsv"), tsv)?;
    let ddir = o.out.join("designs");
    let _ = std::fs::remove_dir_all(&ddir);
    std::fs::create_dir_all(&ddir)?;
    let mut per: BTreeMap<(u32, String), Vec<&KitClock>> = BTreeMap::new();
    for k in all {
        per.entry((k.period, k.family.clone())).or_default().push(k);
    }
    let vol = |k: &KitClock| k.dims[0] * k.dims[1] * k.dims[2];
    let mut md = format!(
        "# Entity clock kit `{}`\n\nGenerated by `mctas clock kit {}` (redsim; screened, then judged by the real-game judge's rule for designs with entities). {} clock types (blocks + entity kinds), {} periods. `designs/`: the smallest minimal clock of each period.\n\n| P | family | types | minimal types | smallest (blocks, box) |\n|---|---|---|---|---|\n",
        o.kit,
        o.kit,
        all.len(),
        all.iter().map(|k| k.period).collect::<BTreeSet<_>>().len()
    );
    for ((p, fam), ks) in &per {
        let types: BTreeSet<&str> = ks.iter().map(|k| k.type_key.as_str()).collect();
        let mins: BTreeSet<&str> = ks.iter().filter(|k| k.minimal).map(|k| k.type_key.as_str()).collect();
        let best = ks.iter().min_by_key(|k| (k.blocks, vol(k))).unwrap();
        md.push_str(&format!("| {} | {fam} | {} | {} | {}, {}x{}x{} |\n", crate::gt_time(*p), types.len(), mins.len(), best.blocks, best.dims[0], best.dims[1], best.dims[2]));
    }
    md.push_str("\n## The designs (one per period, shrunk to minimal)\n\n| P | design | blocks, box | family |\n|---|---|---|---|\n");
    for k in best {
        let p = k.period;
        let mut d = k.design.clone();
        let ent = d.entities.first().map(|e| crate::short(&e.kind).to_string()).unwrap_or_default();
        d.name = format!("kit-{}-{ent}-p{p}-{}b", o.kit, k.blocks);
        d.meta.insert("kit".into(), format!("{} ({}): period {p} gt, family {}, minimal; {} types with this period", o.kit, k.design.name, k.family, all.iter().filter(|x| x.period == p).count()));
        // the caption (`clock archive` takes the first comment block)
        let text = d.to_text();
        let (first, rest) = text.split_once('\n').unwrap_or((&text, ""));
        let comment: String = wrap(&caption(&o.kit, &d), 100).iter().map(|l| format!("# {l}\n")).collect();
        std::fs::write(ddir.join(format!("{}.txt", d.name)), format!("{first}\n{comment}{rest}"))?;
        md.push_str(&format!("| {} | {} | {}, {}x{}x{} | {} |\n", crate::gt_time(p), d.name, k.blocks, k.dims[0], k.dims[1], k.dims[2], k.family));
    }
    std::fs::write(o.out.join("summary.md"), md)?;
    Ok((all.len(), best.len()))
}

/// Words into lines of at most `width` characters.
fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in s.split_whitespace() {
        if !cur.is_empty() && cur.len() + 1 + w.len() > width {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(w);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// How a kit's clock works, from its blocks and entities.
fn caption(kit: &str, d: &Design) -> String {
    let entity = d.entities.first().map(|e| crate::short(&e.kind).replace('_', " ")).unwrap_or_default();
    let motion = d.entities.first().and_then(|e| e.nbt.as_deref()).map(|n| if n.contains("Motion:[-") { " moving west" } else if n.contains("Motion:[") { " moving east" } else { "" }).unwrap_or("");
    match kit {
        "shuttle" => {
            let xs: Vec<i32> = d.blocks.keys().filter(|p| p[1] == 1).map(|p| p[0]).collect();
            let (lo, hi) = (xs.iter().min().copied().unwrap_or(0), xs.iter().max().copied().unwrap_or(0));
            let track: Vec<String> = (lo..=hi)
                .map(|x| {
                    let top = d.blocks.get(&[x, 1, 0]).map(String::as_str).unwrap_or("air");
                    let under = d.blocks.get(&[x, 0, 0]).map(String::as_str).unwrap_or("air");
                    match crate::short(top).split('[').next().unwrap_or("") {
                        "powered_rail" if under.contains("redstone_block") => "P".into(),
                        "powered_rail" => "p".into(),
                        "detector_rail" => "D".into(),
                        "rail" => "R".into(),
                        "stone" => "S".into(),
                        other => other.to_string(),
                    }
                })
                .collect();
            let cart = d.entities.first().map(|e| e.pos[0].floor() as i32 - lo + 1).unwrap_or(0);
            format!(
                "Minecart shuttle (clock kit K1, `mctas clock kit shuttle`): a straight track {} from west to east (S stone bumper, P powered rail on a redstone block, p unpowered powered rail, R rail, D detector rail, every rail on its support); an empty minecart starts{motion} on piece {cart}. A powered rail launches the resting cart away from the bumper behind it and speeds it up, the cart coasts, stops against the far bumper or runs out of speed and comes back; the detector rail turns on as it passes and off at its 20 gt recheck.",
                track.join(" ")
            )
        }
        "bouncer" => {
            let base = if d.blocks.values().any(|s| s.contains("soul_sand")) { "soul sand (an upward bubble column)" } else { "magma (a downward bubble column)" };
            let cap = d.blocks.values().find(|s| s.contains("slab") || s.contains("trapdoor") || s.contains("lantern")).map(|s| format!(", under a {}", crate::short(s).split('[').next().unwrap_or("").replace('_', " "))).unwrap_or_default();
            format!(
                "Bubble-column bouncer (clock kit K2, `mctas clock kit bouncer`): a {entity} on a pool whose water source sits on {base}{cap}. The column launches it (or drags it down) and it falls back, again and again; on its way it trips a tripwire stretched across the pool, whose hooks are the output. No redstone."
            )
        }
        "dripleaf" => format!(
            "Big dripleaf clock (clock kit K3, `mctas clock kit dripleaf`): a {entity} rests on a big dripleaf; the leaf tilts under it, an observer watching the leaf pulses, and the pulse runs to the dirt under the leaf: power resets the tilt (and blocks re-tilting while it lasts), then the leaf tilts again."
        ),
        _ => format!("Entity clock (clock kit {kit})."),
    }
}

#[allow(dead_code)]
fn _pos(_: Pos) {}
