//! The judge's door checks run on redsim (crates/redsim) instead of the game: an exact, fast
//! pre-screen for the search tools (a redsim run of a whole door costs microseconds per tick).
//!
//! [`screen`] mirrors `verify_at` (lib.rs) at each offset: the judge's placement (raw, wire
//! shapes, self updates), settle (at least one tick, then until idle), nothing floating, closed
//! at rest, nothing in front of or behind the door, an unpowered lever, then `cycles` lever
//! cycles: open (until idle, every door and walkway cell walkable at the end) and close (the
//! door blocks and the whole watch box back to rest). Designs with entities, blocks redsim does
//! not model, or a run that leaves redsim's box are [`Screen::Unsupported`]: judge them in the
//! game. redsim matches the game on 100k random redstone circuits and every archived door, so a
//! [`Screen::Fail`] is a game failure; a [`Screen::Pass`] still goes to the real judge (rotations,
//! entities, position-dependent edge cases outside the offsets screened).

use crate::{DoorSpec, closed_problems, open_problems, settled_from, watch_box};
use mctas::design::Design;
use mctas::sim::{Pos, Region, StateFlags};
use redsim::behavior as bh;
use redsim::{Block, BlockPos, World};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Screen {
    /// Works at every screened offset: worst open and close ticks.
    Pass { open: u32, close: u32 },
    /// Fails like the judge would, with the judge's kind of message.
    Fail(String),
    /// redsim cannot judge it (entities, unmodeled blocks, a run leaving its box).
    Unsupported(String),
}

impl Screen {
    pub fn passed(&self) -> bool {
        matches!(self, Screen::Pass { .. })
    }
}

fn bp(p: Pos) -> BlockPos {
    BlockPos::new(p[0], p[1], p[2])
}

/// A `Region` of the world (states as the game prints them, judge flags from redsim).
pub fn region(w: &World, lo: Pos, hi: Pos) -> Region {
    let mut palette: Vec<String> = Vec::new();
    let mut flags: Vec<StateFlags> = Vec::new();
    let mut index: HashMap<Block, u16> = HashMap::new();
    let mut data = Vec::new();
    for y in lo[1]..=hi[1] {
        for z in lo[2]..=hi[2] {
            for x in lo[0]..=hi[0] {
                let s = w.get(BlockPos::new(x, y, z));
                let i = *index.entry(s).or_insert_with(|| {
                    palette.push(s.to_string());
                    flags.push(StateFlags { collides: bh::collides(&s), full: bh::full_cube(&s) });
                    (palette.len() - 1) as u16
                });
                data.push(i);
            }
        }
    }
    Region { from: lo, size: [hi[0] - lo[0] + 1, hi[1] - lo[1] + 1, hi[2] - lo[2] + 1], palette, flags, data }
}

/// Tick at least once, then until idle (the bridge's `step` with `until_idle`), recording the
/// watch box after every tick. None: still busy after `max` ticks.
fn run_recorded(w: &mut World, max: u32, lo: Pos, hi: Pos) -> (Vec<Region>, bool) {
    let mut frames = Vec::new();
    for _ in 0..max.max(1) {
        w.tick();
        frames.push(region(w, lo, hi));
        if w.idle() {
            return (frames, true);
        }
    }
    (frames, false)
}

/// Screen one design at these offsets (`cycles` open/close cycles each, at most `max_ticks`
/// ticks per settle).
pub fn screen(design: &Design, offsets: &[Pos], cycles: usize, max_ticks: u32, empty_walkway: bool) -> Screen {
    screen_ext(design, offsets, cycles, max_ticks, empty_walkway, false)
}

/// `screen` with the lever polarity as an option: `lever_on_rest` = the door rests closed with
/// the lever ON (the community 7x9's polarity; the judge itself wants the lever off at rest).
pub fn screen_ext(design: &Design, offsets: &[Pos], cycles: usize, max_ticks: u32, empty_walkway: bool, lever_on_rest: bool) -> Screen {
    if !design.entities.is_empty() {
        return Screen::Unsupported("the design has entities".into());
    }
    let illegal = crate::legality_problems(design);
    if !illegal.is_empty() {
        return Screen::Fail(format!("illegal design: {}", illegal.join("; ")));
    }
    let spec = match DoorSpec::from_design(design) {
        Ok(s) => s,
        Err(e) => return Screen::Fail(format!("{e:#}")),
    };
    let mut worst = (0, 0);
    for off in offsets {
        let d = design.translated(*off);
        let s = spec.translated(*off);
        match screen_at(&d, &s, cycles, max_ticks, empty_walkway, lever_on_rest) {
            Screen::Pass { open, close } => worst = (worst.0.max(open), worst.1.max(close)),
            Screen::Fail(m) => return Screen::Fail(format!("at offset {off:?}: {m}")),
            u => return u,
        }
    }
    Screen::Pass { open: worst.0, close: worst.1 }
}

fn screen_at(d: &Design, spec: &DoorSpec, cycles: usize, max_ticks: u32, empty_walkway: bool, lever_on_rest: bool) -> Screen {
    let (wlo, whi) = watch_box(d, spec);
    let m = 14;
    let min = BlockPos::new(wlo[0] - m, wlo[1] - m, wlo[2] - m);
    let size = [whi[0] - wlo[0] + 1 + 2 * m, whi[1] - wlo[1] + 1 + 2 * m, whi[2] - wlo[2] + 1 + 2 * m];
    let mut w = World::new(min, size);
    let mut blocks = Vec::new();
    for (p, s) in d.block_list() {
        match Block::parse(&s) {
            Ok(b) => blocks.push((bp(p), b)),
            Err(e) => return Screen::Unsupported(e),
        }
    }
    w.place_like_judge(&blocks);
    let (_, idle) = run_recorded(&mut w, max_ticks, wlo, whi);
    if let Some(e) = w.errors.first() {
        return Screen::Unsupported(e.clone());
    }
    if !idle {
        return Screen::Fail(format!("the build never settles after placement ({max_ticks} ticks)"));
    }
    let rest = region(&w, wlo, whi);
    // nothing floating (the bridge's static_problems: canSurvive of every block in the box)
    let mut floating = Vec::new();
    for (p, s) in rest.iter() {
        let b = w.get(bp(p));
        if !b.is_air() && !bh::can_survive(&w, &b, bp(p)) {
            floating.push(format!("{p:?} {s}"));
        }
    }
    if !floating.is_empty() {
        return Screen::Fail(format!("unsupported blocks (they could not stand there in survival): {}", floating.join("; ")));
    }
    let problems = closed_problems(&rest, spec);
    if !problems.is_empty() {
        return Screen::Fail(format!("not closed at rest: {}", problems.join("; ")));
    }
    if empty_walkway {
        let reach = (whi[0] - wlo[0]).max(whi[2] - wlo[2]) + 1;
        let clutter: Vec<String> = spec
            .in_front_and_behind(reach)
            .iter()
            .filter_map(|p| rest.get(*p).filter(|s| !mctas::blockstate::is_air(s)).map(|s| format!("{p:?} {s}")))
            .collect();
        if !clutter.is_empty() {
            return Screen::Fail(format!("blocks in front of or behind the door: {}", clutter.join("; ")));
        }
    }
    let lever = rest.get(spec.lever).unwrap_or("?").to_string();
    let want = if lever_on_rest { "powered=true" } else { "powered=false" };
    if !lever.starts_with("minecraft:lever") || !lever.contains(want) {
        return Screen::Fail(format!("port lever {:?} is {lever}, expected a lever with {want}", spec.lever));
    }
    let closed_door: Vec<String> = spec.door_cells().iter().map(|p| rest.get(*p).unwrap_or("?").to_string()).collect();
    let mut worst = (0, 0);
    for cycle in 0..cycles {
        w.use_block(bp(spec.lever));
        let (open, idle) = run_recorded(&mut w, max_ticks, wlo, whi);
        if !w.errors.is_empty() {
            return Screen::Unsupported(w.errors[0].clone());
        }
        if !idle {
            return Screen::Fail(format!("cycle {cycle}: still moving {} ticks after opening", open.len()));
        }
        let last = open.last().unwrap();
        let problems = open_problems(last, spec);
        if !problems.is_empty() {
            return Screen::Fail(format!("cycle {cycle}: not open after the lever was switched on: {}", problems.join("; ")));
        }
        if !w.drops.is_empty() {
            return Screen::Fail(format!("cycle {cycle}: entities appeared while opening (something broke): {:?}", w.drops));
        }
        let open_ticks = settled_from(&open, |f| open_problems(f, spec).is_empty()).unwrap_or(0);
        w.use_block(bp(spec.lever));
        let (close, idle) = run_recorded(&mut w, max_ticks, wlo, whi);
        if !w.errors.is_empty() {
            return Screen::Unsupported(w.errors[0].clone());
        }
        if !idle {
            return Screen::Fail(format!("cycle {cycle}: still moving {} ticks after closing", close.len()));
        }
        let last = close.last().unwrap();
        let door_now: Vec<String> = spec.door_cells().iter().map(|p| last.get(*p).unwrap_or("?").to_string()).collect();
        if door_now != closed_door {
            return Screen::Fail(format!("cycle {cycle}: door not restored"));
        }
        let diffs: Vec<String> = rest.iter().filter(|(p, s)| last.get(*p) != Some(*s)).take(8).map(|(p, s)| format!("{p:?} {s} -> {}", last.get(p).unwrap_or("?"))).collect();
        if !diffs.is_empty() {
            return Screen::Fail(format!("cycle {cycle}: the build did not return to its rest state: {}", diffs.join("; ")));
        }
        if !w.drops.is_empty() {
            return Screen::Fail(format!("cycle {cycle}: entities appeared while closing (something broke): {:?}", w.drops));
        }
        let close_ticks = settled_from(&close, |f| spec.door_cells().iter().zip(closed_door.iter()).all(|(p, s)| f.get(*p) == Some(s.as_str()))).unwrap_or(0);
        worst = (worst.0.max(open_ticks), worst.1.max(close_ticks));
    }
    Screen::Pass { open: worst.0, close: worst.1 }
}
