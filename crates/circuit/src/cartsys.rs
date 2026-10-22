//! The cart-transport system run (designs/briefs/cart-transport.md; reviewer and coordinator
//! 2026-10-22: "claim from an end-to-end run"): a design holding a loader station, the track and an
//! unloader station. No circuit kind: a design aid of the rail study (`mctas circuit cart-system`).
//!
//! Ports: `port items:` (+ `port items.b:`): the supply chest (a double chest with both); `port
//! output:` (+ `port output.b:`): the storage chest; `port cart:` the cell where the judge summons
//! one empty hopper minecart, moving `cart.dir` (east by default) at 0.1 b/t. The judge fills the
//! supply with 3456 items (a double chest: 54 stacks of cobblestone, dirt and sand) and runs
//! until every item is in the storage chest, or the cap. It reports the minutes, items/s, the
//! trips, and fails on any item lost (on the floor, destroyed with a cart) or left behind.

use crate::rig::{GameRig, Rig, SimRig};
use crate::shulker::{self, CHEST, Stack};
use anyhow::{Result, anyhow};
use clock::Clock;
use mctas::design::EntitySpec;
use mctas::sim::{PlaceMode, Pool, SandboxOptions, Worker};
use std::collections::BTreeMap;
use std::path::Path;

type Pos = [i32; 3];

/// The supply: a double chest of items (54 stacks of 64: the number is minutes for 3456 items; the
/// station judges already cover 16-stackables and non-stackables).
pub fn supply() -> Vec<Stack> {
    let mut v = Vec::new();
    for k in 0..54 {
        let id = match k % 3 {
            0 => "minecraft:cobblestone",
            1 => "minecraft:dirt",
            _ => "minecraft:sand",
        };
        v.push(Stack::new(id, 64));
    }
    v
}

/// The cap: a double chest at 0.5 items/s.
pub const CAP: u32 = 3456 * 40;
/// The output is counted every SEG gt.
pub const SEG: u32 = 100;

#[derive(Debug, Default)]
pub struct Log {
    pub supplied: u64,
    pub delivered: BTreeMap<String, u64>,
    /// (gt, items in the storage chest) at every change.
    pub flow: Vec<(u32, u64)>,
    pub end: u32,
    pub done: bool,
    pub loose: Vec<String>,
    pub left: BTreeMap<String, u64>,
    pub carts: usize,
    /// The cart's x at every SEG (its trips: turns of direction).
    pub trips: u32,
}

struct Ports {
    items: Vec<Pos>,
    output: Vec<Pos>,
    cart: Pos,
    dir: [f64; 2],
}

fn ports(c: &Clock) -> Result<Ports> {
    let get = |n: &str| c.design.ports.get(n).copied();
    let items: Vec<Pos> = ["items", "items.b"].iter().filter_map(|n| get(n)).collect();
    let output: Vec<Pos> = ["output", "output.b"].iter().filter_map(|n| get(n)).collect();
    let cart = get("cart").ok_or_else(|| anyhow!("cart-system: no `port cart:` (where the judge puts the empty hopper minecart)"))?;
    if items.is_empty() || output.is_empty() {
        anyhow::bail!("cart-system: needs `port items:` and `port output:`");
    }
    let dir = match c.design.meta.get("cart.dir").map(|s| s.trim()) {
        Some("west") => [-1.0, 0.0],
        Some("north") => [0.0, -1.0],
        Some("south") => [0.0, 1.0],
        _ => [1.0, 0.0],
    };
    Ok(Ports { items, output, cart, dir })
}

/// The judge's chests (a double chest across two adjacent cells: left and right halves joined).
fn chests(cells: &[Pos]) -> Vec<(Pos, String)> {
    if cells.len() == 2 {
        let (a, b) = (cells[0], cells[1]);
        let d = [b[0] - a[0], b[2] - a[2]];
        let (facing, a_left) = match d {
            [1, 0] => ("north", true),
            [-1, 0] => ("north", false),
            [0, 1] => ("east", true),
            _ => ("east", false),
        };
        let st = |left: bool| format!("minecraft:chest[facing={facing},type={},waterlogged=false]", if left { "left" } else { "right" });
        return vec![(a, st(a_left)), (b, st(!a_left))];
    }
    cells.iter().map(|q| (*q, CHEST.to_string())).collect()
}

fn snbt(stacks: &[Stack]) -> String {
    let items: Vec<String> = stacks.iter().enumerate().map(|(k, s)| format!("{{Slot:{k}b,id:\"{}\",count:{}}}", s.id, s.count)).collect();
    format!("[{}]", items.join(","))
}

fn drive(rg: &mut dyn Rig, c: &Clock, p: &Ports, from: Pos, to: Pos) -> Result<Log> {
    rg.step(c.free_from() + crate::stimulus::SETTLE)?;
    let s = supply();
    let mut log = Log { supplied: s.iter().map(|x| x.count as u64).sum(), ..Default::default() };
    // the supply split over the chest's halves (27 slots each)
    for (k, q) in p.items.iter().enumerate() {
        let part: Vec<Stack> = s.iter().skip(27 * k).take(27).cloned().collect();
        rg.fill(*q, &snbt(&part))?;
    }
    let at = [p.cart[0] as f64 + 0.5, p.cart[1] as f64 + 0.0625, p.cart[2] as f64 + 0.5];
    let nbt = format!("{{Motion:[{:.3}d,0.0d,{:.3}d],Items:[]}}", 0.1 * p.dir[0], 0.1 * p.dir[1]);
    let cart = rg.summon(&[EntitySpec { kind: "minecraft:hopper_minecart".into(), pos: at, nbt: Some(nbt) }])?.into_iter().next().ok_or_else(|| anyhow!("summon: no handle"))?;
    let (mut t, mut last_x, mut last_dx) = (0u32, at[0], 0.0f64);
    while t < CAP {
        rg.step(SEG)?;
        t += SEG;
        let got: u64 = rg.containers(from, to)?.iter().filter(|(q, _)| p.output.contains(q)).flat_map(|(_, v)| v.iter().map(|x| x.count as u64)).sum();
        if log.flow.last().map(|x| x.1) != Some(got) {
            log.flow.push((t, got));
        }
        if let Some(e) = rg.entities()?.into_iter().find(|e| e.handle == cart) {
            let dx = e.pos[0] - last_x;
            if dx.abs() > 0.5 && last_dx.abs() > 0.5 && dx.signum() != last_dx.signum() {
                log.trips += 1;
            }
            if dx.abs() > 0.5 {
                last_dx = dx;
            }
            last_x = e.pos[0];
        }
        if got >= log.supplied {
            log.done = true;
            break;
        }
    }
    log.end = t;
    for (q, v) in rg.containers(from, to)? {
        for x in v {
            if p.output.contains(&q) {
                *log.delivered.entry(x.id.clone()).or_insert(0) += x.count as u64;
            } else {
                *log.left.entry(x.id.clone()).or_insert(0) += x.count as u64;
            }
        }
    }
    for e in rg.entities()? {
        if e.kind.ends_with("item") {
            log.loose.push(format!("{} at {:.1?}", e.stacks.first().map(|x| format!("{} x{}", x.id, x.count)).unwrap_or_default(), e.pos));
        } else if e.kind.contains("minecart") {
            log.carts += 1;
            for x in &e.stacks {
                *log.left.entry(x.id.clone()).or_insert(0) += x.count as u64;
            }
        }
    }
    Ok(log)
}

fn watch_box(c: &Clock) -> Result<(Pos, Pos)> {
    let (lo, hi) = c.design.bounds().ok_or_else(|| anyhow!("an empty design"))?;
    Ok(([lo[0] - 3, lo[1] - 3, lo[2] - 3], [hi[0] + 3, hi[1] + 3, hi[2] + 3]))
}

/// One run in the game.
pub fn game(w: &mut Worker, c: &Clock) -> Result<std::result::Result<Log, String>> {
    let p = ports(c)?;
    let (from, to) = watch_box(c)?;
    let chunks = [from[0].div_euclid(16), from[2].div_euclid(16), to[0].div_euclid(16), to[2].div_euclid(16)];
    let sb = w.create(&SandboxOptions { chunks, seed: shulker::build_seed(from, to, 0) })?;
    let r = (|| -> Result<std::result::Result<Log, String>> {
        clock::verify::place(w, sb, c)?;
        let floating = w.unsupported(sb, from, to)?;
        if let Some(q) = floating.first() {
            return Ok(Err(format!("{} at {q:?} has no support", clock::short(c.design.blocks.get(q).map(|s| s.as_str()).unwrap_or("?")))));
        }
        let mut cells = chests(&p.items);
        cells.extend(chests(&p.output));
        w.set_blocks(sb, &cells, PlaceMode::Raw)?;
        let mut rig = GameRig { w, sb, rec: None };
        Ok(Ok(drive(&mut rig, c, &p, from, to)?))
    })();
    let _ = w.drop_sandbox(sb);
    r
}

/// One run on redsim.
pub fn sim(c: &Clock) -> Result<std::result::Result<Log, String>> {
    let p = ports(c)?;
    let (from, to) = watch_box(c)?;
    let mut cells = chests(&p.items);
    cells.extend(chests(&p.output));
    let w = match crate::rig::sim_world_of(c, from, to, &cells) {
        Ok(Ok(w)) => w,
        Ok(Err(why)) | Err(why) => return Ok(Err(why)),
    };
    let mut rig = SimRig { w };
    Ok(Ok(drive(&mut rig, c, &p, from, to)?))
}

/// A line's materials, track and cart included (the judge's chests left out): iron (hoppers 5,
/// rails 6 per 16, detector rails 1, the hopper minecart 10), gold (a powered rail: 1, 6 per craft),
/// redstone (dust, torches, repeaters 3, comparators 3, a redstone block 9, a powered or detector
/// rail 1/6) and quartz (comparators).
pub fn bill(d: &mctas::design::Design) -> (f64, f64, f64, u32) {
    let (mut fe, mut au, mut rs, mut q) = (10.0f64, 0.0f64, 0.0f64, 0u32);
    for st in d.blocks.values() {
        match st.split('[').next().unwrap_or(st).trim_start_matches("minecraft:") {
            "hopper" => fe += 5.0,
            "rail" => fe += 6.0 / 16.0,
            "detector_rail" => {
                fe += 1.0;
                rs += 1.0 / 6.0;
            }
            "powered_rail" => {
                au += 1.0;
                rs += 1.0 / 6.0;
            }
            "redstone_wire" | "redstone_torch" | "redstone_wall_torch" => rs += 1.0,
            "repeater" => rs += 3.0,
            "comparator" => {
                rs += 3.0;
                q += 1;
            }
            "redstone_block" => rs += 9.0,
            _ => {}
        }
    }
    (fe, au, rs, q)
}

/// The run's verdict line and its table.
pub fn judge(c: &Clock, log: &Log) -> (bool, String, Vec<String>) {
    let l = |m: &BTreeMap<String, u64>| m.iter().map(|(k, n)| format!("{} x{n}", clock::short(k))).collect::<Vec<_>>().join(", ");
    let got: u64 = log.delivered.values().sum();
    let min = |gt: u32| gt as f64 / 1200.0;
    let mut table = vec![format!("delivered: {}", l(&log.delivered)), format!("left in the line: {}", if log.left.is_empty() { "nothing".into() } else { l(&log.left) })];
    let quarters: Vec<String> = [1u64, 2, 3, 4].iter().map(|k| log.flow.iter().find(|x| x.1 >= log.supplied * k / 4).map(|x| format!("{:.1} min", min(x.0))).unwrap_or_else(|| "-".into())).collect();
    table.push(format!("by the quarter: {}", quarters.join(", ")));
    if let Some(e) = log.loose.first() {
        return (false, format!("{} item(s) on the floor ({e}...)", log.loose.len()), table);
    }
    if !log.done {
        return (false, format!("{got} of {} delivered after {:.0} min (the cap)", log.supplied, min(log.end)), table);
    }
    if log.carts != 1 {
        return (false, format!("{} carts at the end (one was summoned: one destroyed?)", log.carts), table);
    }
    let blocks = c.design.blocks.len();
    let (fe, au, rs, q) = bill(&c.design);
    let line = format!("minutes for a double chest {:.1}, {:.2} items/s, {} trips, bill {fe:.0} iron, {au:.0} gold, {rs:.0} redstone, {q} quartz (track and cart included), {blocks} blocks", min(log.end), log.supplied as f64 * 20.0 / log.end.max(1) as f64, log.trips.div_ceil(2));
    (true, line, table)
}

/// `mctas circuit cart-line`: a whole line from two stations (designs/briefs/cart-transport.md):
/// the unloader turned half a turn at the west end, `n` blocks of track (rails on a stone floor, a
/// powered rail every `spacing` blocks from the third, each lit by a redstone torch on its south
/// side), the loader as built at the east end; both stations' `port line.in:` cells are the
/// track's two end cells. Ports: the loader's `items` with `items.b` beside it (north, else
/// south), the unloader's `output` with `output.b` beside it (south, else west), `cart` on the
/// track 10 blocks from the loader, heading east.
pub fn compose(name: &str, loader: &Clock, unloader: &Clock, n: i32, spacing: i32) -> Result<mctas::design::Design> {
    let port = |c: &Clock, p: &str| c.design.ports.get(p).copied().ok_or_else(|| anyhow!("{}: no `port {p}:`", c.design.name));
    let (ul, ll) = (port(unloader, "line.in")?, port(loader, "line.in")?);
    let mut d = mctas::design::Design { name: name.to_string(), ..Default::default() };
    let mut put = |d: &mut mctas::design::Design, q: Pos, s: String| -> Result<()> {
        if let Some(old) = d.blocks.get(&q)
            && *old != s
        {
            anyhow::bail!("cart-line: two blocks at {q:?} ({old} and {s})");
        }
        d.blocks.insert(q, s);
        Ok(())
    };
    // the unloader, half a turn: (x, z) -> (-x, -z), its line.in at (0, 0, 0)
    let um = |q: Pos| [-q[0] + ul[0], q[1] - ul[1], -q[2] + ul[2]];
    for (q, s) in &unloader.design.blocks {
        put(&mut d, um(*q), clock::canon::rotate_state(s, 2))?;
    }
    for (q, s) in &unloader.design.block_entities {
        d.block_entities.insert(um(*q), s.clone());
    }
    // the loader as built, its line.in at (n - 1, 0, 0)
    let lm = |q: Pos| [q[0] - ll[0] + n - 1, q[1] - ll[1], q[2] - ll[2]];
    for (q, s) in &loader.design.blocks {
        put(&mut d, lm(*q), s.clone())?;
    }
    for (q, s) in &loader.design.block_entities {
        d.block_entities.insert(lm(*q), s.clone());
    }
    // the track
    for x in 0..n {
        let powered = x >= 3 && (x - 3) % spacing == 0 && x <= n - 4;
        let rail = if powered { "minecraft:powered_rail[powered=true,shape=east_west,waterlogged=false]" } else { "minecraft:rail[shape=east_west,waterlogged=false]" };
        if !d.blocks.contains_key(&[x, 0, 0]) {
            put(&mut d, [x, 0, 0], rail.to_string())?;
        }
        if !d.blocks.contains_key(&[x, -1, 0]) {
            put(&mut d, [x, -1, 0], "minecraft:stone".to_string())?;
        }
        if powered {
            put(&mut d, [x, -1, 1], "minecraft:stone".to_string())?;
            put(&mut d, [x, 0, 1], "minecraft:redstone_torch[lit=true]".to_string())?;
        }
    }
    let beside = |d: &mctas::design::Design, q: Pos, sides: &[[i32; 3]]| -> Result<Pos> {
        sides.iter().map(|s| [q[0] + s[0], q[1] + s[1], q[2] + s[2]]).find(|c| !d.blocks.contains_key(c)).ok_or_else(|| anyhow!("cart-line: no free cell beside {q:?} for the double chest"))
    };
    let items = lm(port(loader, "items")?);
    let output = um(port(unloader, "output")?);
    // the second chest half off the rail row (z = 0): a chest over the track catches a climbing cart
    let items_b = beside(&d, items, &[[0, 0, -1], [0, 0, 1], [1, 0, 0], [-1, 0, 0]].into_iter().filter(|s| items[2] + s[2] != 0 || items[1] < -1).collect::<Vec<_>>())?;
    let output_b = beside(&d, output, &[[0, 0, 1], [-1, 0, 0], [0, 0, -1]])?;
    d.ports.insert("items".into(), items);
    d.ports.insert("items.b".into(), items_b);
    d.ports.insert("output".into(), output);
    d.ports.insert("output.b".into(), output_b);
    d.ports.insert("cart".into(), [n - 11, 0, 0]);
    d.meta.insert("cart.dir".into(), "east".into());
    Ok(d)
}

/// `mctas circuit cart-system`: one run (game or redsim).
pub fn run_one(pool: Option<&Pool>, path: &Path) -> Result<(bool, String, Vec<String>)> {
    let c = Clock::load(path)?;
    let r = match pool {
        None => sim(&c)?,
        Some(pool) => pool.map(vec![0u8], |w, _, _| game(w, &c)).into_iter().next().ok_or_else(|| anyhow!("no run"))??,
    };
    Ok(match r {
        Ok(log) => judge(&c, &log),
        Err(why) => (false, why, Vec::new()),
    })
}
