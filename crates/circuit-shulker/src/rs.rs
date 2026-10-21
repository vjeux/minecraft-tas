//! The shulker judge's run in redsim: crates/circuit/src/shulker.rs `run_in` stepped the way the
//! judge steps it in the game (the design placed as the judge places it, the judge's chests raw,
//! the settle, the box supply and the source, the start levers, then segments of [`SEG`] gt with
//! every output chest read each gt and the judge's visit after each segment, then the rest and the
//! inventory), producing the judge's own [`RunLog`] so that [`shulker::judge`] decides. redsim
//! replays these designs tick for tick (`mctas circuit shulker compare`) and draws the drops from
//! the build's seed as the bridge does, so a run here is the game's run of the same build. The
//! search screen of `mctas circuit shulker lns`; the game (`mctas circuit verify`) keeps the last
//! word (colours: redsim has no coloured boxes; rotations; the drop stress).

use circuit::shulker::{self, DROP_LIFE, Delivery, REST, RunLog, SEG, STARVE_WAIT, Scenario, Stack as JStack};
use circuit::{Circuit, InputHow, Pos};
use redsim::block::Block;
use redsim::entity::EntityKind;
use redsim::pos::BlockPos;
use redsim::redstone::container::{self, Stack};
use redsim::redstone::shulker::{chest_stacks, set_chest};
use redsim::world::World;

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A redsim stack as the judge's (a box's contents and name from its payload).
pub fn jstack(w: &World, s: &Stack) -> JStack {
    let mut j = JStack::new(&format!("minecraft:{}", container::item_name(s.item)), s.count as u32);
    if s.payload > 0 {
        let p = &w.payloads[s.payload as usize - 1];
        j.contents = p.slots.iter().flatten().map(|x| JStack::new(&format!("minecraft:{}", container::item_name(x.item)), x.count as u32)).collect();
        j.name = p.name.clone();
    }
    j
}

/// A judge stack as a redsim one (`None`: an item redsim does not know, e.g. a coloured box).
fn rstack(w: &mut World, s: &JStack) -> Option<Stack> {
    let item = container::item_id(&s.id)?;
    let mut contents = Vec::new();
    for (i, x) in s.contents.iter().enumerate() {
        contents.push((i, Stack::new(container::item_id(&x.id)?, x.count as u8)));
    }
    Some(Stack { item, count: s.count as u8, payload: container::new_payload_named(w, &contents, s.name.clone()) })
}

fn in_box(p: &BlockPos, from: Pos, to: Pos) -> bool {
    p.x >= from[0] && p.x <= to[0] && p.y >= from[1] && p.y <= to[1] && p.z >= from[2] && p.z <= to[2]
}

/// The run's settings beyond the scenario.
#[derive(Clone, Copy, Debug)]
pub struct RunOpts {
    /// The watched box's margin around the circuit (the judge's `--margin`, 3).
    pub margin: i32,
    /// Stop at the cap of this many gt instead of the scenario's (a screen: a candidate that has
    /// not delivered its boxes by then is out).
    pub cap: Option<u32>,
    /// The site viewer's endless supply (mcview, `Chests::endless`): every box chest set back to
    /// its starting boxes before every gt, so a box leaving never shows on a comparator.
    pub endless_boxes: bool,
    /// `box-starved` with this many boxes at the start instead of 2 (a design aid: a loader whose
    /// box-arrival watcher rides a shuttle may miss the late boxes after an odd number).
    pub starve_first: Option<usize>,
}

impl Default for RunOpts {
    fn default() -> RunOpts {
        RunOpts { margin: 3, cap: None, endless_boxes: false, starve_first: None }
    }
}

/// One scenario on a circuit (as built: rotation 0; a scenario with `groups > 1` tiles it):
/// `Err(why)` when redsim does not model it or a rule of the run breaks (a block without support,
/// something reaching the edge of the watched box), else the judge's log.
pub fn run(c: &Circuit, sc: &Scenario, o: RunOpts) -> Result<RunLog, String> {
    run_with(c, sc, o, None)
}

/// [`run`] with a hook called after every gt of the run (t, the world).
pub fn run_with(c: &Circuit, sc: &Scenario, o: RunOpts, mut hook: Option<&mut dyn FnMut(u32, &World)>) -> Result<RunLog, String> {
    let tiled;
    let c = if sc.groups > 1 {
        tiled = circuit::tiled(c).map_err(|e| format!("{e:#}"))?;
        &tiled
    } else {
        c
    };
    let (from, to) = circuit::run::watch_box(c, o.margin);
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    let seed = shulker::build_seed(from, to, sc.seed_salt);
    w.drop_seed = Some(seed);
    w.mirror_level_random(seed, redsim::redstone::shulker::chunk_range(from, to));
    if let Some((p, s)) = clock::sim::unsupported(&c.clock, from, to)?.into_iter().next() {
        return Err(format!("{} at {p:?} has no support", clock::short(&s)));
    }
    let groups = sc.groups.max(1) as usize;
    let sources: Vec<(BlockPos, usize)> = c.stores.iter().filter(|s| s.role == "items").map(|s| (bp(s.pos), shulker::group_of(&s.name) as usize)).collect();
    let supplies: Vec<(BlockPos, u32)> = c.stores.iter().filter(|s| s.role == "boxes").map(|s| (bp(s.pos), shulker::group_of(&s.name))).collect();
    let rejects: Vec<BlockPos> = c.stores.iter().filter(|s| s.role == "reject").map(|s| bp(s.pos)).collect();
    let outs: Vec<(String, BlockPos)> = c.outputs.iter().map(|p| (p.name.clone(), bp(p.pos))).collect();
    let chest = Block::Chest { facing: redsim::pos::Dir::North, waterlogged: false };
    let cells: Vec<(BlockPos, Block)> = c.stores.iter().map(|s| bp(s.pos)).chain(outs.iter().map(|x| x.1)).map(|p| (p, chest)).collect();
    w.place_raw(&cells);
    for _ in 0..c.clock.free_from() + circuit::stimulus::SETTLE {
        w.tick();
    }
    let mut log = RunLog { dims: dims(c), supplied_g: vec![0; groups], drained_g: vec![0; groups], ..Default::default() };
    // what the design's own containers hold before the judge fills anything
    let judge_cells: Vec<BlockPos> = cells.iter().map(|x| x.0).collect();
    for (p, be) in &w.containers.map {
        if !in_box(p, from, to) || judge_cells.contains(p) {
            continue;
        }
        for s in be.slots.iter().flatten() {
            let j = jstack(&w, s);
            log.initial_items += sc.counts(&j);
            if j.is_box() {
                log.initial_boxes += j.count as u64;
            }
        }
    }
    // the start: the box supply (badge box-starved: 2 now, the rest later), the items, the levers
    let mut supply: Vec<JStack> = sc.supply();
    let first = o.starve_first.unwrap_or(2);
    let held: Vec<JStack> = if sc.starve {
        supply.split_off(first.min(supply.len()))
    } else if sc.one_box {
        supply.split_off(1.min(supply.len()))
    } else {
        Vec::new()
    };
    let mut trickled = 0usize;
    let mut start_boxes: Vec<(BlockPos, Vec<Stack>)> = Vec::new();
    for g in 0..groups as u32 {
        let mine: Vec<BlockPos> = supplies.iter().filter(|(_, sg)| *sg == g).map(|(p, _)| *p).collect();
        for (k, chunk) in supply.chunks(27).enumerate() {
            let p = *mine.get(k).ok_or_else(|| format!("{} boxes need {} `boxes` chests (copy {g})", supply.len(), supply.len().div_ceil(27)))?;
            let st: Vec<Stack> = chunk.iter().map(|s| rstack(&mut w, s).ok_or_else(|| format!("{} is not a redsim item", s.id))).collect::<Result<_, _>>()?;
            set_chest(&mut w, p, &st);
            start_boxes.push((p, st));
        }
        log.supply.extend(supply.iter().cloned());
    }
    log.supply.extend(held.iter().cloned());
    let pause_at = sc.total() / 2;
    let mut paused_until: Option<u32> = None;
    let stacks: Vec<Stack> = sc.source_stacks().iter().map(|s| rstack(&mut w, s).ok_or_else(|| format!("{} is not a redsim item", s.id))).collect::<Result<_, _>>()?;
    let mut cursor: Vec<usize> = vec![0; groups];
    let topup = |w: &mut World, log: &mut RunLog, t: u32, paused_until: &mut Option<u32>, cursor: &mut Vec<usize>| {
        if !sc.loader {
            return;
        }
        if let Some(len) = sc.pause {
            if paused_until.is_none() && log.supplied >= pause_at {
                *paused_until = Some(t + len);
            }
            if paused_until.is_some_and(|u| t < u) {
                return;
            }
        }
        for (p, g) in &sources {
            if cursor[*g] >= stacks.len() {
                continue;
            }
            let mut have = chest_stacks(w, *p);
            let before = have.len();
            while have.len() < 27 && cursor[*g] < stacks.len() {
                if sc.pause.is_some() && paused_until.is_none() && log.supplied_g[*g] >= pause_at {
                    break;
                }
                let s = stacks[cursor[*g]];
                log.supplied_g[*g] += s.count as u64;
                log.supplied += s.count as u64;
                have.push(s);
                cursor[*g] += 1;
            }
            if have.len() != before {
                set_chest(w, *p, &have);
            }
        }
    };
    topup(&mut w, &mut log, 0, &mut paused_until, &mut cursor);
    for i in &c.inputs {
        let mode = i.name == "mode" || i.name.starts_with("mode.");
        if i.how == InputHow::Lever && (!mode || sc.mode_on) && !w.use_block(bp(i.pos)) {
            return Err(format!("the lever at {:?} is not modeled", i.pos));
        }
    }
    let seg = if sc.poll_entities { 1 } else { SEG };
    let cap = o.cap.unwrap_or(sc.cap).min(sc.cap);
    let sinks: Vec<(BlockPos, usize)> = if sc.loader { Vec::new() } else { sources.clone() };
    let mut drained = 0u64;
    let mut t = 0u32;
    let junk_total: u64 = if sc.filter { sc.source_stacks().iter().filter(|s| s.id != sc.item).map(|s| s.count as u64).sum() } else { 0 };
    loop {
        // a segment: every output chest read each gt (the judge's per-gt frames), the flow
        let mut seen: Vec<usize> = vec![0; outs.len()];
        for k in 0..seg {
            if o.endless_boxes || sc.endless {
                for (p, st) in &start_boxes {
                    set_chest(&mut w, *p, st);
                }
            }
            w.tick();
            if let Some(e) = w.errors.first() {
                return Err(format!("t={}: redsim: {e}", t + k + 1));
            }
            if let Some(h) = hook.as_mut() {
                h(t + k + 1, &w);
            }
            for (oi, (name, p)) in outs.iter().enumerate() {
                let have = chest_stacks(&w, *p);
                if have.len() > seen[oi] {
                    for s in &have[seen[oi]..] {
                        log.deliveries.push(Delivery { t: t + k + 1, port: name.clone(), stack: jstack(&w, s) });
                    }
                    seen[oi] = have.len();
                }
            }
            let held_now: u64 = sources.iter().map(|(p, _)| chest_stacks(&w, *p).iter().map(|s| s.count as u64).sum::<u64>()).sum();
            let moved = if sc.loader { log.supplied.saturating_sub(held_now) } else { drained + held_now };
            if log.flow.last().is_none_or(|x| x.1 != moved) {
                log.flow.push((t + k + 1, moved));
            }
            // badge drop-free: the item entities alive now (seg = 1: every gt)
            if sc.poll_entities {
                let tt = t + k + 1;
                for e in w.entities.list.iter().filter(|e| !e.removed && matches!(e.kind, EntityKind::Item(_))) {
                    let r = log.item_lives.entry(e.id.to_string()).or_insert((tt, tt, false));
                    r.1 = tt;
                    r.2 |= e.on_ground;
                }
            }
        }
        t += seg;
        // the judge's visit: the outputs, the sinks and the rejects emptied, the sources topped up
        for (_, p) in &outs {
            if !chest_stacks(&w, *p).is_empty() {
                set_chest(&mut w, *p, &[]);
            }
        }
        for (p, g) in &sinks {
            let have = chest_stacks(&w, *p);
            if !have.is_empty() {
                for s in &have {
                    let j = jstack(&w, s);
                    if sc.is_item(&j.id) && j.contents.is_empty() {
                        drained += j.count as u64;
                        log.drained_g[*g] += j.count as u64;
                    } else {
                        log.foreign.push(j.id.clone());
                    }
                }
                log.drained.push((t, drained));
                set_chest(&mut w, *p, &[]);
            }
        }
        for p in &rejects {
            let have = chest_stacks(&w, *p);
            if !have.is_empty() {
                for s in &have {
                    let j = jstack(&w, s);
                    if j.id != sc.item && sc.is_item(&j.id) && j.contents.is_empty() {
                        log.rejected += j.count as u64;
                    } else {
                        log.misrejected.push(shulker::describe(&j, sc));
                    }
                }
                set_chest(&mut w, *p, &[]);
            }
        }
        topup(&mut w, &mut log, t, &mut paused_until, &mut cursor);
        let boxes_out = log.deliveries.iter().filter(|d| d.stack.is_box()).count() as u32;
        // the judge's one-box check: one more box at every visit (a hopper feeding the chest)
        if sc.one_box && trickled < held.len() {
            let p = supplies[0].0;
            let mut have = chest_stacks(&w, p);
            have.push(rstack(&mut w, &held[trickled]).ok_or_else(|| format!("{} is not a redsim item", held[trickled].id))?);
            set_chest(&mut w, p, &have);
            trickled += 1;
        }
        if sc.starve && !held.is_empty() && boxes_out >= first.min(sc.boxes as usize) as u32 && !log.refilled {
            let second = log.deliveries.iter().filter(|d| d.stack.is_box()).nth(first.saturating_sub(1)).map(|d| d.t).unwrap_or(t);
            if t >= second + STARVE_WAIT {
                let p = supplies[0].0;
                let mut have = chest_stacks(&w, p);
                for s in &held {
                    have.push(rstack(&mut w, s).ok_or_else(|| format!("{} is not a redsim item", s.id))?);
                }
                set_chest(&mut w, p, &have);
                log.refilled = true;
            }
        }
        let delivered_items: u64 = log.deliveries.iter().filter(|d| d.stack.is_box()).map(|d| sc.counts(&d.stack)).sum();
        let done = if sc.filter {
            log.supplied >= sc.source_total() && boxes_out >= sc.boxes && log.rejected >= junk_total
        } else if sc.mixed {
            log.supplied >= sc.source_total() && delivered_items >= sc.source_total()
        } else if sc.loader {
            boxes_out >= sc.boxes_all() && log.supplied >= sc.source_total() * sc.groups.max(1) as u64
        } else {
            boxes_out >= sc.boxes_all() && drained >= sc.total_all()
        };
        if done || t >= cap {
            log.timed_out = !done;
            break;
        }
    }
    log.end = t;
    if log.timed_out {
        return Ok(log);
    }
    // rest: up to 5 windows of REST gt, the design must reach one in which nothing changes
    let snap = |w: &World| -> (Vec<Block>, Vec<(BlockPos, Vec<Option<Stack>>)>) {
        let mut cells = Vec::new();
        for y in from[1]..=to[1] {
            for z in from[2]..=to[2] {
                for x in from[0]..=to[0] {
                    cells.push(w.get(BlockPos::new(x, y, z)));
                }
            }
        }
        let mut cs: Vec<(BlockPos, Vec<Option<Stack>>)> = w.containers.map.iter().filter(|(p, _)| in_box(p, from, to)).map(|(p, c)| (*p, c.slots.clone())).collect();
        cs.sort_by_key(|(p, _)| (p.x, p.y, p.z));
        (cells, cs)
    };
    let mut a = snap(&w);
    let mut b = a.clone();
    for _ in 0..5 {
        for _ in 0..REST {
            w.tick();
        }
        b = snap(&w);
        if a == b && w.idle() {
            break;
        }
        a = b.clone();
    }
    if a.0 != b.0 {
        log.restless = format!("blocks still change {} gt after the last box", 5 * REST);
    } else if a.1 != b.1 {
        log.restless = format!("container contents still change {} gt after the last box", 5 * REST);
    } else if !w.idle() {
        log.restless = format!("ticks, block events or moving pistons pending {} gt after the last box", 5 * REST);
    }
    // lag: the design's hoppers still unlocked at rest
    log.awake = c.clock.design.blocks.keys().filter(|p| matches!(w.get(bp(**p)), Block::Hopper { enabled: true, .. })).count() as u32;
    // nothing may reach the outer layer of the watched box
    for y in from[1]..=to[1] {
        for z in from[2]..=to[2] {
            for x in from[0]..=to[0] {
                let p = [x, y, z];
                if (0..3).any(|i| p[i] == from[i] || p[i] == to[i]) && w.get(bp(p)) != Block::Air {
                    return Err(format!("{} at {p:?} reached the edge of the watched box", w.get(bp(p))));
                }
            }
        }
    }
    // the inventory: every container and placed box in the box, every entity
    let list: Vec<(BlockPos, Vec<Option<Stack>>)> = w.containers.map.iter().filter(|(p, _)| in_box(p, from, to)).map(|(p, c)| (*p, c.slots.clone())).collect();
    for (p, slots) in &list {
        for s in slots.iter().flatten() {
            let j = jstack(&w, s);
            log.left_items += sc.counts(&j);
            if j.is_box() {
                log.left_boxes += j.count as u64;
            }
        }
        if redsim::redstone::shulker::box_item_of(&w.get(*p)).is_some() {
            log.left_boxes += 1;
        }
    }
    for e in w.entities.list.iter().filter(|e| !e.removed) {
        match &e.kind {
            EntityKind::Item(it) => {
                if let Some(id) = container::item_id(&it.item) {
                    let j = jstack(&w, &Stack { item: id, count: it.count, payload: it.payload });
                    log.left_items += sc.counts(&j);
                    if j.is_box() {
                        log.left_boxes += j.count as u64;
                    }
                }
                log.loose.push(format!("{} x{} at {:.1},{:.1},{:.1}", it.item, it.count, e.pos.x, e.pos.y, e.pos.z));
            }
            _ => log.loose.push(format!("an entity at {:.1},{:.1},{:.1}", e.pos.x, e.pos.y, e.pos.z)),
        }
    }
    log.left_items = log.left_items.saturating_sub(log.initial_items);
    log.left_boxes = log.left_boxes.saturating_sub(log.initial_boxes);
    Ok(log)
}

/// The design's block box (judge chests and input cells left out), as the judge measures it.
pub fn dims(c: &Circuit) -> [i32; 3] {
    let ins: Vec<Pos> = c.inputs.iter().map(|i| i.pos).collect();
    let mut lo = [i32::MAX; 3];
    let mut hi = [i32::MIN; 3];
    for p in c.clock.design.blocks.keys().filter(|p| !ins.contains(p)) {
        for i in 0..3 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    if lo[0] == i32::MAX { [0, 0, 0] } else { [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1] }
}

/// The badge part of a verdict that a redsim run can decide (`colours` cannot: no coloured boxes).
pub fn badge_ok(name: &str, sc: &Scenario, log: &RunLog) -> Result<(), String> {
    match name {
        "box-starved" if !log.refilled => Err("the run ended before the held boxes were added".into()),
        "drop-free" => match log.item_lives.values().find(|(a, b, g)| b - a > DROP_LIFE || *g) {
            Some((a, b, g)) => Err(format!("an item entity lived {} gt{}", b - a + 1, if *g { " and touched the ground" } else { "" })),
            None => Ok(()),
        },
        "flush" | "mixed" => {
            let dry = log.flow.iter().find(|(_, m)| *m >= sc.source_total()).map(|x| x.0).ok_or("the source never ran dry")?;
            let last = log.deliveries.iter().filter(|d| d.stack.is_box()).map(|d| d.t).max().unwrap_or(0);
            if last > dry + shulker::FLUSH_WAIT {
                return Err(format!("the last box arrived {} gt after the source ran dry", last - dry));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// One named scenario judged on redsim: `Ok(metrics)` or why not.
pub fn check(c: &Circuit, name: &str, sc: &Scenario, o: RunOpts) -> Result<Vec<(String, String)>, String> {
    let log = run(c, sc, o)?;
    let out = shulker::judge(sc, &log);
    if !out.pass {
        return Err(out.why);
    }
    badge_ok(name, sc, &log)?;
    Ok(out.metrics)
}

/// The scenarios of a design's badges that redsim decides (every badge scenario of the judge
/// but `colours`), after the plain one (`base`).
pub fn scenarios(c: &Circuit) -> Vec<(String, Scenario)> {
    let Some(kind) = c.kind.as_ref() else { return Vec::new() };
    let Some(base) = Scenario::base(kind) else { return Vec::new() };
    let mut v = vec![("base".to_string(), base.clone())];
    v.extend(shulker::badge_scenarios(c, &base).into_iter().filter(|(n, _)| n != "colours"));
    // the judge's full-chest check (required): a chest full of boxes; and the same chest kept
    // full (the endless supply, required too)
    v.push(("full-chest".to_string(), shulker::full_chest_scenario(c, &base)));
    v.push(("endless".to_string(), shulker::endless_scenario(c, &base)));
    v.push(("one-box".to_string(), shulker::one_box_scenario(c, &base)));
    // the drop stress's burst run (loaders): boxes that already hold all but one stack, so each
    // breaks right after it is placed
    if base.loader {
        let mut s = Scenario::with_stack(true, 1);
        s.burst = true;
        s.spare = 0;
        v.push(("burst".to_string(), s));
    }
    v
}
