//! Redstone dust: `RedstoneWireBlock` + `DefaultRedstoneWireEvaluator` (redstone experiments
//! off, the default and the judge's world).

use crate::behavior::{self as bh, Support};
use crate::block::{Block, Kind, Side};
use crate::pos::{BlockPos, Dir, java_hashset_order};
use crate::world::World;

pub const CROSS: Block = Block::Wire { north: Side::Side, east: Side::Side, south: Side::Side, west: Side::Side, power: 0 };

fn side(s: &Block, d: Dir) -> Side {
    match *s {
        Block::Wire { north, east, south, west, .. } => match d {
            Dir::North => north,
            Dir::East => east,
            Dir::South => south,
            Dir::West => west,
            _ => Side::None,
        },
        _ => Side::None,
    }
}

fn with_side(s: Block, d: Dir, v: Side) -> Block {
    match s {
        Block::Wire { north, east, south, west, power } => match d {
            Dir::North => Block::Wire { north: v, east, south, west, power },
            Dir::East => Block::Wire { north, east: v, south, west, power },
            Dir::South => Block::Wire { north, east, south: v, west, power },
            Dir::West => Block::Wire { north, east, south, west: v, power },
            _ => s,
        },
        _ => s,
    }
}

pub fn power(s: &Block) -> u8 {
    match *s {
        Block::Wire { power, .. } => power,
        _ => 0,
    }
}

fn with_power(s: Block, p: u8) -> Block {
    match s {
        Block::Wire { north, east, south, west, .. } => Block::Wire { north, east, south, west, power: p },
        _ => s,
    }
}

fn is_cross(s: &Block) -> bool {
    Dir::HORIZONTAL.iter().all(|d| side(s, *d).connected())
}

fn is_dot(s: &Block) -> bool {
    Dir::HORIZONTAL.iter().all(|d| !side(s, *d).connected())
}

fn default_with_power(p: u8) -> Block {
    Block::Wire { north: Side::None, east: Side::None, south: Side::None, west: Side::None, power: p }
}

/// `shouldConnectTo(level, pos)` (direction null): only wires.
fn connects_to_null(w: &World, p: BlockPos) -> bool {
    bh::should_wire_connect(w, &w.get(p), p, None)
}

/// `canSurviveOn`: a sturdy top face, or a hopper.
pub fn can_survive_on(w: &World, p: BlockPos, s: &Block) -> bool {
    bh::sturdy(w, s, p, Dir::Up, Support::Full) || matches!(s, Block::Hopper { .. })
}

pub fn can_survive(w: &World, pos: BlockPos) -> bool {
    let b = pos.below();
    can_survive_on(w, b, &w.get(b))
}

fn connecting_side_up(w: &World, pos: BlockPos, d: Dir, can_connect_up: bool) -> Side {
    let rp = pos.rel(d);
    let rs = w.get(rp);
    if can_connect_up {
        let placeable_above = matches!(rs, Block::Trapdoor { .. }) || can_survive_on(w, rp, &rs);
        if placeable_above && connects_to_null(w, rp.above()) {
            if bh::sturdy(w, &rs, rp, d.opposite(), Support::Full) {
                return Side::Up;
            }
            return Side::Side;
        }
    }
    if !bh::should_wire_connect(w, &rs, rp, Some(d)) && (bh::is_conductor(w, &rs, rp) || !connects_to_null(w, rp.below())) {
        Side::None
    } else {
        Side::Side
    }
}

fn connecting_side(w: &World, pos: BlockPos, d: Dir) -> Side {
    let above = w.get(pos.above());
    connecting_side_up(w, pos, d, !bh::is_conductor(w, &above, pos))
}

fn missing_connections(w: &World, mut state: Block, pos: BlockPos) -> Block {
    let above = w.get(pos.above());
    let can_connect_up = !bh::is_conductor(w, &above, pos);
    for d in Dir::HORIZONTAL {
        if !side(&state, d).connected() {
            let sc = connecting_side_up(w, pos, d, can_connect_up);
            state = with_side(state, d, sc);
        }
    }
    state
}

/// `getConnectionState`.
pub fn connection_state(w: &World, state: Block, pos: BlockPos) -> Block {
    let was_dot = is_dot(&state);
    let mut state = missing_connections(w, default_with_power(power(&state)), pos);
    if was_dot && is_dot(&state) {
        return state;
    }
    let north = side(&state, Dir::North).connected();
    let south = side(&state, Dir::South).connected();
    let east = side(&state, Dir::East).connected();
    let west = side(&state, Dir::West).connected();
    let ns_empty = !north && !south;
    let ew_empty = !east && !west;
    if !west && ns_empty {
        state = with_side(state, Dir::West, Side::Side);
    }
    if !east && ns_empty {
        state = with_side(state, Dir::East, Side::Side);
    }
    if !north && ew_empty {
        state = with_side(state, Dir::North, Side::Side);
    }
    if !south && ew_empty {
        state = with_side(state, Dir::South, Side::Side);
    }
    state
}

pub fn update_shape(w: &mut World, state: Block, pos: BlockPos, d: Dir, np: BlockPos, ns: Block) -> Block {
    if d == Dir::Down {
        return if !can_survive_on(w, np, &ns) { Block::Air } else { state };
    }
    if d == Dir::Up {
        return connection_state(w, state, pos);
    }
    let sc = connecting_side(w, pos, d);
    if sc.connected() == side(&state, d).connected() && !is_cross(&state) {
        with_side(state, d, sc)
    } else {
        connection_state(w, with_side(with_power(CROSS, power(&state)), d, sc), pos)
    }
}

pub fn update_indirect_neighbour_shapes(w: &mut World, state: Block, pos: BlockPos, fl: u32, limit: i32) {
    for d in Dir::HORIZONTAL {
        let v = side(&state, d);
        if v != Side::None && w.get(pos.rel(d)).kind() != Kind::Wire {
            let down = pos.rel(d).below();
            if w.get(down).kind() == Kind::Wire {
                let np = down.rel(d.opposite());
                let ns = w.get(np);
                w.neighbor_shape_changed(d.opposite(), down, np, ns, fl, limit);
            }
            let up = pos.rel(d).above();
            if w.get(up).kind() == Kind::Wire {
                let np = up.rel(d.opposite());
                let ns = w.get(np);
                w.neighbor_shape_changed(d.opposite(), up, np, ns, fl, limit);
            }
        }
    }
}

/// `getBlockSignal`: the best neighbor signal with every wire's signal off.
fn block_signal(w: &World, pos: BlockPos) -> i32 {
    w.wire_signal.set(false);
    let s = w.best_neighbor_signal(pos);
    w.wire_signal.set(true);
    s
}

fn wire_signal_at(w: &World, p: BlockPos) -> i32 {
    power(&w.get(p)) as i32
}

/// `getIncomingWireSignal`.
fn incoming_wire_signal(w: &World, pos: BlockPos) -> i32 {
    let mut ws = 0;
    for d in Dir::HORIZONTAL {
        let np = pos.rel(d);
        let ns = w.get(np);
        ws = ws.max(wire_signal_at(w, np));
        let above = pos.above();
        let n_cond = bh::is_conductor(w, &ns, np);
        if n_cond && !bh::is_conductor(w, &w.get(above), above) {
            ws = ws.max(wire_signal_at(w, np.above()));
        } else if !n_cond {
            ws = ws.max(wire_signal_at(w, np.below()));
        }
    }
    (ws - 1).max(0)
}

fn target_strength(w: &World, pos: BlockPos) -> i32 {
    let bs = block_signal(w, pos);
    if bs == 15 { bs } else { bs.max(incoming_wire_signal(w, pos)) }
}

/// `DefaultRedstoneWireEvaluator.updatePowerStrength`.
fn update_power_strength(w: &mut World, pos: BlockPos, state: Block) {
    let target = target_strength(w, pos);
    if power(&state) as i32 != target {
        if w.get(pos) == state {
            w.set_block(pos, with_power(state, target as u8), crate::world::flags::CLIENTS);
        }
        let mut set = vec![pos];
        for d in Dir::ALL {
            set.push(pos.rel(d));
        }
        for p in java_hashset_order(&set) {
            w.update_neighbors_at(p, Kind::Wire);
        }
    }
}

fn check_corner_change_at(w: &mut World, pos: BlockPos) {
    if w.get(pos).kind() == Kind::Wire {
        w.update_neighbors_at(pos, Kind::Wire);
        for d in Dir::ALL {
            w.update_neighbors_at(pos.rel(d), Kind::Wire);
        }
    }
}

fn update_neighbors_of_neighboring_wires(w: &mut World, pos: BlockPos) {
    for d in Dir::HORIZONTAL {
        check_corner_change_at(w, pos.rel(d));
    }
    for d in Dir::HORIZONTAL {
        let t = pos.rel(d);
        if bh::is_conductor(w, &w.get(t), t) {
            check_corner_change_at(w, t.above());
        } else {
            check_corner_change_at(w, t.below());
        }
    }
}

pub fn on_place(w: &mut World, state: Block, pos: BlockPos, old: Block, _moved: bool) {
    if old.kind() != Kind::Wire {
        update_power_strength(w, pos, state);
        for d in Dir::VERTICAL {
            w.update_neighbors_at(pos.rel(d), Kind::Wire);
        }
        update_neighbors_of_neighboring_wires(w, pos);
    }
}

pub fn affect_neighbors_after_removal(w: &mut World, state: Block, pos: BlockPos, moved: bool) {
    if !moved {
        for d in Dir::ALL {
            w.update_neighbors_at(pos.rel(d), Kind::Wire);
        }
        update_power_strength(w, pos, state);
        update_neighbors_of_neighboring_wires(w, pos);
    }
}

pub fn neighbor_changed(w: &mut World, state: Block, pos: BlockPos, _src: Kind, _moved: bool) {
    if can_survive(w, pos) {
        update_power_strength(w, pos, state);
    } else {
        w.drops.push((pos, state));
        w.remove_block(pos, false);
    }
}

pub fn get_signal(w: &World, state: &Block, pos: BlockPos, d: Dir) -> i32 {
    if w.wire_signal.get() && d != Dir::Down {
        let p = power(state) as i32;
        if p == 0 {
            0
        } else if d != Dir::Up && !side(&connection_state(w, *state, pos), d.opposite()).connected() {
            0
        } else {
            p
        }
    } else {
        0
    }
}

pub fn get_direct_signal(w: &World, state: &Block, pos: BlockPos, d: Dir) -> i32 {
    if !w.wire_signal.get() { 0 } else { get_signal(w, state, pos, d) }
}
