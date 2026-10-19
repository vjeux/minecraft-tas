//! Differential testing of the whole-box model against redsim: random flat boxes are settled
//! in redsim, then the lever is flipped on and off; after every tick redsim's world (blocks,
//! moving-block entities, pending ticks, pending block events) is compared with the model's
//! state evaluated on bools (`Conc`). The same code builds the SAT circuit, so a model that
//! matches here is the circuit the solver searches.

use super::alg::{Alg, Conc};
use super::state::*;
use redsim::{Block, BlockPos, World};

pub fn dname(d: usize) -> &'static str {
    ["east", "west", "up", "down"][d]
}
fn dir_of(s: &str) -> Option<usize> {
    match s {
        "east" => Some(E),
        "west" => Some(W),
        "up" => Some(U),
        "down" => Some(D),
        _ => None,
    }
}
fn prop<'a>(s: &'a str, key: &str) -> Option<&'a str> {
    let inner = s.split_once('[')?.1.strip_suffix(']')?;
    inner.split(',').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v)
    })
}
fn short(s: &str) -> &str {
    let s = s.strip_prefix("minecraft:").unwrap_or(s);
    s.split('[').next().unwrap_or(s)
}

pub fn fidx(f: &[bool; 4]) -> usize {
    f.iter().position(|&b| b).unwrap_or(0)
}

/// Block-state string of a (concrete) model cell; `lever` gives the lever state for LEVER cells.
pub fn cell_state(c: &Cell<bool>, lever: &str) -> String {
    let k = c.k.iter().position(|&b| b).unwrap_or(AIR);
    let f = dname(fidx(&c.f));
    let lvl = c.lvl.iter().filter(|&&b| b).count();
    match k {
        AIR => "minecraft:air".into(),
        STONE => "minecraft:stone".into(),
        OBSI => "minecraft:obsidian".into(),
        GLASS => "minecraft:glass".into(),
        NOTE => format!("minecraft:note_block[instrument=harp,note=0,powered={}]", c.on),
        LAMP => format!("minecraft:redstone_lamp[lit={}]", c.on),
        BULB => format!("minecraft:waxed_copper_bulb[lit={},powered={}]", c.on, c.ext),
        COMP => format!("minecraft:comparator[facing={f},mode=compare,powered={}]", c.on),
        RB => "minecraft:redstone_block".into(),
        DUST => format!("minecraft:redstone_wire[east=side,north=side,power={lvl},south=side,west=side]"),
        REP => format!("minecraft:repeater[delay={},facing={f},locked=false,powered={}]", c.dl.iter().position(|&b| b).unwrap_or(0) + 1, c.on),
        OBSV => format!("minecraft:observer[facing={f},powered={}]", c.on),
        TORCH => format!("minecraft:redstone_torch[lit={}]", c.on),
        WTORCH => format!("minecraft:redstone_wall_torch[facing={f},lit={}]", c.on),
        PISTON => format!("minecraft:{}piston[extended={},facing={f}]", if c.st { "sticky_" } else { "" }, c.ext),
        HEAD => format!("minecraft:piston_head[facing={f},short=false,type={}]", if c.st { "sticky" } else { "normal" }),
        MOVING => format!("minecraft:moving_piston[facing={f},type={}]", if c.st { "sticky" } else { "normal" }),
        _ => lever.to_string(),
    }
}

/// A model cell from a block-state string (static part only).
pub fn cell_of_state(s: &str) -> Option<Cell<bool>> {
    let a = Conc::default();
    let f = prop(s, "facing").and_then(dir_of);
    let on = prop(s, "powered").or(prop(s, "lit")) == Some("true");
    let mut c = match short(s) {
        "air" | "cave_air" => konst_cell(&a, AIR),
        "stone" | "smooth_stone" | "cobblestone" => konst_cell(&a, STONE),
        "obsidian" => konst_cell(&a, OBSI),
        "glass" => konst_cell(&a, GLASS),
        "note_block" => konst_cell(&a, NOTE),
        "redstone_lamp" => konst_cell(&a, LAMP),
        "waxed_copper_bulb" => {
            let mut c = konst_cell(&a, BULB);
            c.on = prop(s, "lit") == Some("true");
            c.ext = prop(s, "powered") == Some("true");
            return Some(c);
        }
        "comparator" => {
            if prop(s, "mode") == Some("subtract") {
                return None;
            }
            konst_cell(&a, COMP)
        }
        "redstone_block" => konst_cell(&a, RB),
        "redstone_wire" => {
            let mut c = konst_cell(&a, DUST);
            let p: usize = prop(s, "power").and_then(|v| v.parse().ok()).unwrap_or(0);
            for j in 0..p {
                c.lvl[j] = true;
            }
            c
        }
        "repeater" => {
            let mut c = konst_cell(&a, REP);
            let d: usize = prop(s, "delay").and_then(|v| v.parse().ok()).unwrap_or(1);
            c.dl[d - 1] = true;
            c
        }
        "observer" => konst_cell(&a, OBSV),
        "redstone_torch" => konst_cell(&a, TORCH),
        "redstone_wall_torch" => konst_cell(&a, WTORCH),
        "piston" | "sticky_piston" => {
            let mut c = konst_cell(&a, PISTON);
            c.st = short(s) == "sticky_piston";
            c.ext = prop(s, "extended") == Some("true");
            c
        }
        "piston_head" => {
            let mut c = konst_cell(&a, HEAD);
            c.st = prop(s, "type") == Some("sticky");
            c
        }
        "lever" => konst_cell(&a, LEVER),
        _ => return None,
    };
    if let Some(f) = f {
        if !c.k[LEVER] {
            c.f[f] = true;
        }
    }
    if c.k[REP] || c.k[OBSV] || c.k[TORCH] || c.k[WTORCH] || c.k[NOTE] || c.k[LAMP] || c.k[COMP] {
        c.on = on;
    }
    Some(c)
}

/// Note-block instrument a block state gives the note block above it (`harp` for others).
pub fn instrument_below(s: &str) -> &'static str {
    match short(s) {
        "stone" | "smooth_stone" | "cobblestone" | "obsidian" | "observer" => "basedrum",
        "glass" => "hat",
        "note_block" => "bass",
        _ => "harp",
    }
}

/// Give every note block of a W x H grid of states the instrument of the block below it.
pub fn fix_instruments(w: i32, h: i32, cells: &mut [String]) {
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if short(&cells[i]) != "note_block" {
                continue;
            }
            let below = if y > 0 { cells[i - w as usize].clone() } else { "minecraft:air".into() };
            let ins = instrument_below(&below);
            let powered = prop(&cells[i], "powered").unwrap_or("false").to_string();
            cells[i] = format!("minecraft:note_block[instrument={ins},note=0,powered={powered}]");
        }
    }
}

/// Read redsim's world into model cells (z = 0 plane at a tick boundary). None: the world
/// holds something the model does not represent.
pub fn read_world(w: &World, g: &Geo) -> Result<Vec<Cell<bool>>, String> {
    let n = g.n();
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let (x, y) = g.xy(i);
        let p = BlockPos::new(x, y, 0);
        let b = w.get(p);
        let st = b.to_string();
        let mut c = if short(&st) == "moving_piston" {
            let a = Conc::default();
            let mut c = konst_cell(&a, MOVING);
            c.st = prop(&st, "type") == Some("sticky");
            if let Some(f) = prop(&st, "facing").and_then(dir_of) {
                c.f[f] = true;
            }
            let be = w.piston.be(p).ok_or_else(|| format!("moving piston without entity at {x},{y}"))?;
            let ms = be.moved.to_string();
            let mc = cell_of_state(&ms).ok_or_else(|| format!("unmodeled moving content {ms}"))?;
            if mc.k[STONE] {
                c.mk[MC_STONE] = true;
            } else if mc.k[GLASS] {
                c.mk[MC_GLASS] = true;
            } else if mc.k[NOTE] {
                c.mk[MC_NOTE] = true;
                c.mon = mc.on;
            } else if mc.k[LAMP] {
                c.mk[MC_LAMP] = true;
                c.mon = mc.on;
            } else if mc.k[BULB] {
                c.mk[MC_BULB] = true;
                c.mon = mc.on;
                c.mst = mc.ext;
            } else if mc.k[RB] {
                c.mk[MC_RB] = true;
            } else if mc.k[OBSV] {
                c.mk[MC_OBSV] = true;
                c.mon = mc.on;
            } else if mc.k[PISTON] {
                c.mk[MC_PISTON] = true;
                c.mst = mc.st;
            } else if mc.k[HEAD] {
                c.mk[MC_HEAD] = true;
                c.mst = mc.st;
            } else {
                return Err(format!("unmodeled moving content {ms}"));
            }
            c.mf = mc.f;
            c.mext = be.extending;
            c.msrc = be.source;
            c.mold = be.progress >= 2;
            c.mnew = false;
            c
        } else {
            cell_of_state(&st).ok_or_else(|| format!("unmodeled block {st} at {x},{y}"))?
        };
        if c.k[LEVER] {
            c.on = false;
        }
        if c.k[COMP] {
            let o = w.comparator_out.get(&p).copied().unwrap_or(0) as usize;
            for j in 0..o.min(15) {
                c.lvl[j] = true;
            }
        }
        v.push(std::mem::replace(&mut c, konst_cell(&Conc::default(), AIR)));
    }
    for t in w.ticks.all() {
        if t.pos.z != 0 {
            continue;
        }
        if let Some(i) = g.idx(t.pos.x, t.pos.y) {
            let j = t.trigger - (w.time + 1);
            if (0..8).contains(&j) {
                v[i].sch[j as usize] = true;
            } else {
                return Err(format!("tick at {:?} due in {j}", t.pos));
            }
        }
    }
    for e in w.events.iter() {
        if !matches!(e.kind, redsim::Kind::Piston(_)) {
            continue;
        }
        if let Some(i) = g.idx(e.pos.x, e.pos.y) {
            if e.pos.z == 0 && (0..3).contains(&e.a) {
                v[i].ev[e.a as usize] = true;
            }
        }
    }
    Ok(v)
}

/// Lever block-state string for a candidate.
pub fn lever_state(l: &LeverCand, on: bool) -> String {
    let (face, facing) = if l.oop {
        ("wall", "south")
    } else {
        let d = (l.sup.0 - l.pos.0, l.sup.1 - l.pos.1);
        match d {
            (0, -1) => ("floor", "north"),
            (0, 1) => ("ceiling", "north"),
            (1, 0) => ("wall", "west"),
            _ => ("wall", "east"),
        }
    };
    format!("minecraft:lever[face={face},facing={facing},powered={on}]")
}

pub fn lever_pos3(l: &LeverCand) -> BlockPos {
    if l.oop { BlockPos::new(l.sup.0, l.sup.1, 1) } else { BlockPos::new(l.pos.0, l.pos.1, 0) }
}

/// Compare two concrete states field by field; returns the first differences.
pub fn diff_cells(g: &Geo, a: &[Cell<bool>], b: &[Cell<bool>]) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..g.n() {
        let (x, y) = g.xy(i);
        let (p, q) = (&a[i], &b[i]);
        let ka = p.k.iter().position(|&v| v).unwrap_or(99);
        let kb = q.k.iter().position(|&v| v).unwrap_or(99);
        let name = |k: usize| KNAMES.get(k).copied().unwrap_or("?");
        if ka != kb {
            out.push(format!("({x},{y}) kind {} vs {}", name(ka), name(kb)));
            continue;
        }
        let k = ka;
        let mut fields = Vec::new();
        if matches!(k, REP | OBSV | WTORCH | PISTON | HEAD | MOVING | COMP) && p.f != q.f {
            fields.push("facing");
        }
        if matches!(k, PISTON | HEAD | MOVING) && p.st != q.st {
            fields.push("sticky");
        }
        if k == PISTON && p.ext != q.ext {
            fields.push("extended");
        }
        if matches!(k, REP | OBSV | TORCH | WTORCH | NOTE | LAMP | COMP) && p.on != q.on {
            fields.push("powered");
        }
        if k == BULB && (p.on != q.on || p.ext != q.ext) {
            fields.push("bulb lit/powered");
        }
        if k == COMP && p.lvl != q.lvl {
            fields.push("comparator output");
        }
        if k == MOVING && p.mk[MC_BULB] && (p.mon != q.mon || p.mst != q.mst) {
            fields.push("moving bulb state");
        }
        if k == MOVING && (p.mk[MC_OBSV] || p.mk[MC_NOTE] || p.mk[MC_LAMP]) && p.mon != q.mon {
            fields.push("moving content power");
        }
        if k == DUST && p.lvl != q.lvl {
            fields.push("power");
        }
        if k == MOVING && (p.mk != q.mk || p.mf != q.mf || p.mext != q.mext || p.msrc != q.msrc || p.mold != q.mold) {
            fields.push("moving entity");
        }
        if k == MOVING && p.mk[MC_OBSV] && p.mon != q.mon {
            fields.push("moving observer power");
        }
        if p.sch != q.sch {
            fields.push("scheduled tick");
        }
        if p.ev != q.ev {
            fields.push("block event");
        }
        if !fields.is_empty() {
            let bits = |v: &[bool]| v.iter().map(|&b| if b { '1' } else { '0' }).collect::<String>();
            out.push(format!("({x},{y}) {} {}: {} [real sch {} ev {} | model {} sch {} ev {}]", name(k), cell_state(p, "lever"), fields.join(","), bits(&p.sch), bits(&p.ev), cell_state(q, "lever"), bits(&q.sch), bits(&q.ev)));
        }
    }
    out
}

pub struct Rng(pub u64);
impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// A random flat box and lever (cells as block states, lever candidate).
pub fn random_box(r: &mut Rng, w: i32, h: i32) -> (Geo, Vec<String>, LeverCand) {
    let mut cells: Vec<String> = Vec::new();
    let full = |s: &str| matches!(short(s), "stone" | "obsidian" | "redstone_block" | "observer" | "glass" | "note_block" | "redstone_lamp" | "waxed_copper_bulb") || s.contains("extended=false");
    for i in 0..(w * h) as usize {
        let f = dname(r.below(4) as usize);
        let fh = dname(r.below(2) as usize);
        let below_full = i >= w as usize && full(&cells[i - w as usize]);
        let left_full = i % (w as usize) > 0 && full(&cells[i - 1]);
        let mut s = match r.below(25) {
            0..=4 => "minecraft:air".to_string(),
            5..=7 => "minecraft:stone".to_string(),
            8 => if r.below(2) == 0 { "minecraft:obsidian".to_string() } else { "minecraft:glass".to_string() },
            9 => "minecraft:redstone_block".to_string(),
            10..=11 => "minecraft:redstone_wire[east=side,north=side,power=0,south=side,west=side]".to_string(),
            12 => format!("minecraft:repeater[delay={},facing={fh},locked=false,powered=false]", r.below(4) + 1),
            13..=14 => format!("minecraft:observer[facing={f},powered=false]"),
            15 => "minecraft:redstone_torch[lit=true]".to_string(),
            16 => "minecraft:redstone_wall_torch[facing=east,lit=true]".to_string(),
            17 => format!("minecraft:piston[extended=false,facing={f}]"),
            19 => "minecraft:note_block[instrument=harp,note=0,powered=false]".to_string(),
            20 => "minecraft:redstone_lamp[lit=false]".to_string(),
            21 => format!("minecraft:waxed_copper_bulb[lit={},powered=false]", r.below(2) == 0),
            22..=23 => format!("minecraft:comparator[facing={fh},mode=compare,powered=false]"),
            _ => format!("minecraft:sticky_piston[extended=false,facing={f}]"),
        };
        let needs_below = matches!(short(&s), "redstone_wire" | "repeater" | "redstone_torch" | "comparator");
        if (needs_below && !below_full) || (short(&s) == "redstone_wall_torch" && !left_full) {
            s = if r.below(2) == 0 { "minecraft:air".into() } else { "minecraft:stone".into() };
        }
        cells.push(s);
    }
    // lever: in-box air cell with a sturdy neighbor, or outside the box, or out of the plane
    let geo0 = Geo { w, h, door: (0, 0), levers: vec![], rounds: 3 };
    let sturdy = |s: &str| matches!(short(s), "stone" | "obsidian" | "redstone_block" | "observer" | "glass" | "note_block" | "redstone_lamp" | "waxed_copper_bulb");
    let mut lev = None;
    for _ in 0..200 {
        let mode = r.below(3);
        let i = r.below((w * h) as u64) as usize;
        let (x, y) = geo0.xy(i);
        if mode == 2 {
            if sturdy(&cells[i]) {
                lev = Some(LeverCand { pos: (x, y), oop: true, sup: (x, y) });
                break;
            }
            continue;
        }
        let d = r.below(4) as usize;
        let (lx, ly) = (x - DIRS[d].0, y - DIRS[d].1);
        if !sturdy(&cells[i]) {
            continue;
        }
        let inside = geo0.idx(lx, ly);
        if mode == 0 {
            if let Some(li) = inside {
                if li != i {
                    cells[li] = "LEVER".into();
                    lev = Some(LeverCand { pos: (lx, ly), oop: false, sup: (x, y) });
                    break;
                }
            }
        } else if inside.is_none() {
            lev = Some(LeverCand { pos: (lx, ly), oop: false, sup: (x, y) });
            break;
        }
    }
    let lev = lev.unwrap_or_else(|| {
        cells[0] = "minecraft:stone".into();
        LeverCand { pos: (0, 0), oop: true, sup: (0, 0) }
    });
    let g = Geo { w, h, door: (0, 0), levers: vec![lev.clone()], rounds: 3 };
    fix_instruments(w, h, &mut cells);
    (g, cells, lev)
}

pub struct Outcome {
    /// ticks compared before the first mismatch (None: never mismatched)
    pub mismatch: Option<(usize, Vec<String>)>,
    pub flags: Vec<&'static str>,
    pub violations: Vec<&'static str>,
    pub skipped: Option<String>,
    pub moved: bool,
}

/// Settle the box in redsim, flip the lever on for `t` ticks and off for `t` ticks, comparing
/// every tick with the model.
pub fn run_case(g: &Geo, cells: &[String], lev: &LeverCand, t: usize) -> Outcome {
    let mut out = Outcome { mismatch: None, flags: vec![], violations: vec![], skipped: None, moved: false };
    let mut w = World::new(BlockPos::new(-6, -6, -3), [g.w + 12, g.h + 12, 7]);
    let mut blocks = Vec::new();
    for (i, s) in cells.iter().enumerate() {
        let (x, y) = g.xy(i);
        let st = if s == "LEVER" { lever_state(lev, false) } else { s.clone() };
        match Block::parse(&st) {
            Ok(b) => blocks.push((BlockPos::new(x, y, 0), b)),
            Err(e) => {
                out.skipped = Some(e);
                return out;
            }
        }
    }
    if !cells.iter().any(|s| s == "LEVER") {
        blocks.push((lever_pos3(lev), Block::parse(&lever_state(lev, false)).unwrap()));
    }
    w.place_like_judge(&blocks);
    w.run_until_idle(100);
    if !w.idle() || !w.drops.is_empty() || !w.errors.is_empty() {
        out.skipped = Some("does not settle cleanly".into());
        return out;
    }
    let lp = lever_pos3(lev);
    if !w.get(lp).to_string().contains("lever") {
        out.skipped = Some("lever popped".into());
        return out;
    }
    // the settled build must stay inside the box (the lever excepted)
    for (p, b) in w.blocks() {
        if p != lp && (p.z != 0 || g.idx(p.x, p.y).is_none()) && !b.is_air() {
            out.skipped = Some("settles outside the box".into());
            return out;
        }
    }
    let mut s = match read_world(&w, g) {
        Ok(s) => s,
        Err(e) => {
            out.skipped = Some(e);
            return out;
        }
    };
    let mut cx = Ctx { a: Conc::default(), g, lsel: vec![true], flags_on: true };
    // the model's own dust levels must agree at rest
    let der = cx.derive(&s, false);
    for i in 0..g.n() {
        if !s[i].k[COMP] && der.lvl[i] != s[i].lvl {
            out.mismatch = Some((0, vec![format!("rest dust level at {:?}", g.xy(i))]));
            return out;
        }
    }
    let mut step = 0;
    for lon in [true, false] {
        w.use_block(lp);
        s = cx.flip(&s, lon);
        step += 1;
        for k in 0..=t {
            if k > 0 {
                w.tick();
                s = cx.tick(&s, lon);
                step += 1;
            }
            if !w.drops.is_empty() || !w.errors.is_empty() {
                out.skipped = Some(format!("redsim drop/error at step {step}"));
                out.flags = cx.a.flags.clone();
                out.violations = cx.a.violations.clone();
                return out;
            }
            let real = match read_world(&w, g) {
                Ok(r) => r,
                Err(e) => {
                    out.skipped = Some(e);
                    return out;
                }
            };
            if real.iter().any(|c| c.k[MOVING]) {
                out.moved = true;
            }
            if !cx.a.violations.is_empty() {
                out.flags = cx.a.flags.clone();
                out.violations = cx.a.violations.clone();
                return out;
            }
            let d = diff_cells(g, &real, &s);
            if !d.is_empty() {
                out.mismatch = Some((step, d));
                out.flags = cx.a.flags.clone();
                out.violations = cx.a.violations.clone();
                return out;
            }
        }
    }
    out.flags = cx.a.flags.clone();
    out.violations = cx.a.violations.clone();
    out
}
