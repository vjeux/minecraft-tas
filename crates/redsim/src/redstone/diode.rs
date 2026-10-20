//! Repeaters and comparators: `DiodeBlock`, `RepeaterBlock`, `ComparatorBlock` (+ its block
//! entity's output signal, kept in `World::comparator_out`).

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

fn facing(s: &Block) -> Dir {
    match *s {
        Block::Repeater { facing, .. } | Block::Comparator { facing, .. } => facing,
        _ => Dir::North,
    }
}

fn powered(s: &Block) -> bool {
    match *s {
        Block::Repeater { powered, .. } | Block::Comparator { powered, .. } => powered,
        _ => false,
    }
}

fn with_powered(s: Block, p: bool) -> Block {
    match s {
        Block::Repeater { facing, delay, locked, .. } => Block::Repeater { facing, delay, locked, powered: p },
        Block::Comparator { facing, subtract, .. } => Block::Comparator { facing, subtract, powered: p },
        _ => s,
    }
}

/// `getDelay` in game ticks.
fn delay(s: &Block) -> i32 {
    match *s {
        Block::Repeater { delay, .. } => delay as i32 * 2,
        _ => 2,
    }
}

pub fn can_survive_on(w: &World, np: BlockPos, ns: &Block) -> bool {
    bh::sturdy(w, ns, np, Dir::Up, Support::Rigid)
}

pub fn can_survive(w: &World, pos: BlockPos) -> bool {
    let b = pos.below();
    can_survive_on(w, b, &w.get(b))
}

/// `getInputSignal` (the comparator's adds container reading).
fn input_signal(w: &World, pos: BlockPos, s: &Block) -> i32 {
    let d = facing(s);
    let tp = pos.rel(d);
    let mut input = w.signal(tp, d);
    if input < 15 {
        if let Block::Wire { power, .. } = w.get(tp) {
            input = input.max(power as i32);
        }
    }
    if let Block::Comparator { .. } = s {
        let ts = w.get(tp);
        if bh::has_analog_output(&ts) {
            input = bh::analog_output(w, &ts, tp, d.opposite());
        } else if input < 15 && bh::is_conductor(w, &ts, tp) {
            let tp2 = tp.rel(d);
            let ts2 = w.get(tp2);
            // (item frames: entities, not modeled)
            if bh::has_analog_output(&ts2) {
                input = bh::analog_output(w, &ts2, tp2, d.opposite());
            }
        }
    }
    input
}

/// `getAlternateSignal`: the stronger side input (repeaters: diodes only).
fn alternate_signal(w: &World, pos: BlockPos, s: &Block) -> i32 {
    let d = facing(s);
    let cw = d.clockwise();
    let ccw = d.counter_clockwise();
    let only = matches!(s, Block::Repeater { .. });
    w.control_input_signal(pos.rel(cw), cw, only).max(w.control_input_signal(pos.rel(ccw), ccw, only))
}

pub fn is_locked(w: &World, pos: BlockPos, s: &Block) -> bool {
    match s {
        Block::Repeater { .. } => alternate_signal(w, pos, s) > 0,
        _ => false,
    }
}

fn should_turn_on(w: &World, pos: BlockPos, s: &Block) -> bool {
    match *s {
        Block::Comparator { subtract, .. } => {
            let input = input_signal(w, pos, s);
            if input == 0 {
                return false;
            }
            let side = alternate_signal(w, pos, s);
            input > side || (input == side && !subtract)
        }
        _ => input_signal(w, pos, s) > 0,
    }
}

fn calculate_output(w: &World, pos: BlockPos, s: &Block) -> i32 {
    let input = input_signal(w, pos, s);
    if input == 0 {
        return 0;
    }
    let alt = alternate_signal(w, pos, s);
    if alt > input {
        return 0;
    }
    match *s {
        Block::Comparator { subtract: true, .. } => input - alt,
        _ => input,
    }
}

/// `shouldPrioritize`: the block in front is a diode not facing back into this one.
fn should_prioritize(w: &World, pos: BlockPos, s: &Block) -> bool {
    let d = facing(s).opposite();
    let os = w.get(pos.rel(d));
    bh::is_diode(&os) && facing(&os) != d
}

/// `getOutputSignal` when powered.
fn output_signal(w: &World, pos: BlockPos, s: &Block) -> i32 {
    match s {
        Block::Comparator { .. } => w.comparator_out.get(&pos).copied().unwrap_or(0) as i32,
        _ => 15,
    }
}

pub fn get_signal(w: &World, s: &Block, pos: BlockPos, d: Dir) -> i32 {
    if facing(s) == d && powered(s) { output_signal(w, pos, s) } else { 0 }
}

fn update_neighbors_in_front(w: &mut World, pos: BlockPos, s: &Block) {
    let d = facing(s);
    let op = pos.rel(d.opposite());
    let k = s.kind();
    w.neighbor_changed(op, k);
    w.update_neighbors_except(op, k, d);
}

pub fn on_place(w: &mut World, s: Block, pos: BlockPos, _old: Block, _moved: bool) {
    update_neighbors_in_front(w, pos, &s);
}

pub fn affect_neighbors_after_removal(w: &mut World, s: Block, pos: BlockPos, moved: bool) {
    if !moved {
        update_neighbors_in_front(w, pos, &s);
    }
}

pub fn update_shape(w: &mut World, s: Block, pos: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    if d == Dir::Down && !can_survive_on(w, np, &ns) {
        return Block::Air;
    }
    if let Block::Repeater { facing, delay, powered, .. } = s
        && d.axis() != facing.axis()
    {
        return Block::Repeater { facing, delay, locked: is_locked(w, pos, &s), powered };
    }
    s
}

pub fn neighbor_changed(w: &mut World, s: Block, pos: BlockPos, _src: Kind, _moved: bool) {
    if w.get(pos).kind() != s.kind() {
        return;
    }
    if can_survive(w, pos) {
        check_tick_on_neighbor(w, pos, s);
    } else {
        // dropResources: its loot's stacks pop (level-random draws, seeded items)
        w.drop_resources(pos, s);
        w.drops.push((pos, s));
        w.remove_block(pos, false);
        for d in Dir::ALL {
            w.update_neighbors_at(pos.rel(d), s.kind());
        }
    }
}

fn check_tick_on_neighbor(w: &mut World, pos: BlockPos, s: Block) {
    match s {
        Block::Comparator { .. } => {
            if !w.will_tick_this_tick(pos, Kind::Comparator) {
                let out = calculate_output(w, pos, &s);
                let old = w.comparator_out.get(&pos).copied().unwrap_or(0) as i32;
                if out != old || powered(&s) != should_turn_on(w, pos, &s) {
                    let pr = if should_prioritize(w, pos, &s) { Priority::High } else { Priority::Normal };
                    w.schedule_tick(pos, Kind::Comparator, 2, pr);
                }
            }
        }
        _ => {
            if !is_locked(w, pos, &s) {
                let on = powered(&s);
                let should = should_turn_on(w, pos, &s);
                if on != should && !w.will_tick_this_tick(pos, s.kind()) {
                    let pr = if should_prioritize(w, pos, &s) {
                        Priority::ExtremelyHigh
                    } else if on {
                        Priority::VeryHigh
                    } else {
                        Priority::High
                    };
                    w.schedule_tick(pos, s.kind(), delay(&s), pr);
                }
            }
        }
    }
}

pub fn tick(w: &mut World, s: Block, pos: BlockPos) {
    match s {
        Block::Comparator { .. } => refresh_output_state(w, pos, s),
        _ => {
            if !is_locked(w, pos, &s) {
                let on = powered(&s);
                let should = should_turn_on(w, pos, &s);
                if on && !should {
                    w.set_block(pos, with_powered(s, false), flags::CLIENTS);
                } else if !on {
                    w.set_block(pos, with_powered(s, true), flags::CLIENTS);
                    if !should {
                        w.schedule_tick(pos, s.kind(), delay(&s), Priority::VeryHigh);
                    }
                }
            }
        }
    }
}

/// `ComparatorBlock.useWithoutItem`: cycle the mode, then refresh the output.
pub fn use_comparator(w: &mut World, s: Block, pos: BlockPos) {
    if let Block::Comparator { facing, subtract, powered } = s {
        let ns = Block::Comparator { facing, subtract: !subtract, powered };
        w.set_block(pos, ns, flags::CLIENTS);
        if w.get(pos).kind() == Kind::Comparator {
            refresh_output_state(w, pos, ns);
        }
    }
}

fn refresh_output_state(w: &mut World, pos: BlockPos, s: Block) {
    let out = calculate_output(w, pos, &s);
    let old = match w.comparator_out.get_mut(&pos) {
        Some(v) => {
            let o = *v as i32;
            *v = out as u8;
            o
        }
        None => 0,
    };
    let compare = matches!(s, Block::Comparator { subtract: false, .. });
    if old != out || compare {
        let source_on = should_turn_on(w, pos, &s);
        let is_on = powered(&s);
        if is_on && !source_on {
            w.set_block(pos, with_powered(s, false), flags::CLIENTS);
        } else if !is_on && source_on {
            w.set_block(pos, with_powered(s, true), flags::CLIENTS);
        }
        update_neighbors_in_front(w, pos, &s);
    }
}
