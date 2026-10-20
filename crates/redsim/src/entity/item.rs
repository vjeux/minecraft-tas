//! Item entities (redsim M5, clockinfra): `ItemEntity` (26.3), for items a design summons
//! (`entity: minecraft:item x y z {Item:{id:"minecraft:stone",count:1},Motion:[..],PickupDelay:10s}`).
//!
//! * `tick`: `Entity.tick` (= `baseTick`: fluid interaction), pickup delay, water movement (x0.99,
//!   a 5e-4 lift under 0.06) or gravity 0.04, `move` (skipped 3 ticks in 4 when resting on the
//!   ground: `(tickCount + id) % 4`, the game's network id, here the insertion id), the drag 0.98
//!   (times the block friction on the ground), the bounce (y x -0.5 on the ground), merging with
//!   neighbors every 2 ticks while moving between cells (40 at rest), age, a second
//!   `updateFluidInteraction`, despawn at 6000.
//! * Not reproducible in the game itself, so not modeled (a model note): an item stuck inside a
//!   block (`moveTowardsClosestSpace` draws from the entity's own unseeded random) and drops (the
//!   level random places them). Hoppers (blocks and minecarts) pull item entities
//!   (`redstone::container::item_entities_above`); stack sizes come from the container item list.

use crate::entity::{EntityKind, MoverType, Vec3, collide, mth};
use crate::world::World;

/// `EntityTypes.ITEM.sized(0.25F, 0.25F)`.
pub const WIDTH: f32 = 0.25;
pub const HEIGHT: f32 = 0.25;

/// The item entity's own state.
#[derive(Clone, PartialEq, Default)]
pub struct ItemState {
    /// The stack's item id (`minecraft:...`; empty = no item: discarded at its next tick).
    pub item: String,
    pub count: u8,
    /// `pickupDelay` (32767 = never).
    pub pickup_delay: i32,
    /// `age` (-32768 = never ages).
    pub age: i32,
    /// A shulker box item's contents (`redstone::container::Stack::payload`; clk-rt): 0 = none.
    pub payload: u16,
}

impl std::fmt::Debug for ItemState {
    /// As derived, without a zero payload (so states without one print as before it existed).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("ItemState");
        d.field("item", &self.item).field("count", &self.count).field("pickup_delay", &self.pickup_delay).field("age", &self.age);
        if self.payload != 0 {
            d.field("payload", &self.payload);
        }
        d.finish()
    }
}

impl ItemState {
    /// From the summon NBT: `Item:{id:"..",count:N}`, `PickupDelay:Ns`, `Age:Ns`.
    pub fn from_nbt(nbt: Option<&str>) -> ItemState {
        let mut s = ItemState { item: String::new(), count: 1, pickup_delay: 0, age: 0, payload: 0 };
        let Some(n) = nbt else { return s };
        if let Some(i) = n.find("id:\"") {
            s.item = n[i + 4..].split('"').next().unwrap_or("").to_string();
        }
        let num = |key: &str| -> Option<i32> {
            let i = n.find(key)?;
            let v: String = n[i + key.len()..].chars().take_while(|c| c.is_ascii_digit() || *c == '-').collect();
            v.parse().ok()
        };
        if let Some(c) = num("count:") {
            s.count = c.clamp(0, 255) as u8;
        }
        if let Some(d) = num("PickupDelay:") {
            s.pickup_delay = d;
        }
        if let Some(a) = num("Age:") {
            s.age = a;
        }
        s
    }
}

fn state(w: &World, i: usize) -> ItemState {
    match &w.entities.list[i].kind {
        EntityKind::Item(s) => s.clone(),
        _ => ItemState::default(),
    }
}

fn set_state(w: &mut World, i: usize, s: ItemState) {
    w.entities.list[i].kind = EntityKind::Item(s);
}

/// `ItemEntity.isMergable`.
fn mergable(e: &crate::entity::Entity, s: &ItemState) -> bool {
    !e.removed && s.pickup_delay != 32767 && s.age != -32768 && s.age < 6000 && s.count < crate::redstone::container::max_stack_of(&s.item)
}

/// `ItemEntity.mergeWithNeighbours` / `tryToMerge` / `merge`: the smaller stack moves into the
/// larger one (candidates in list order).
fn merge_with_neighbours(w: &mut World, i: usize) {
    if !mergable(&w.entities.list[i], &state(w, i)) {
        return;
    }
    let bb = w.entities.list[i].bb.inflate(0.5, 0.0, 0.5);
    let others = w.get_entities(bb, |o| matches!(o.kind, EntityKind::Item(_)));
    for j in others {
        if j == i {
            continue;
        }
        let (a, b) = (state(w, i), state(w, j));
        if !mergable(&w.entities.list[j], &b) {
            continue;
        }
        let max = crate::redstone::container::max_stack_of(&a.item) as u32;
        if a.item != b.item || a.count as u32 + b.count as u32 > max {
            continue;
        }
        // tryToMerge: into this one unless the other is at least as large
        let (to, from) = if b.count < a.count { (i, j) } else { (j, i) };
        let (mut ts, mut fs) = (state(w, to), state(w, from));
        // merge(toStack, fromStack, 64): min(min(maxStack, 64) - count, from count)
        let delta = ((max.min(64)) as i32 - ts.count as i32).min(fs.count as i32).max(0);
        ts.count = (ts.count as i32 + delta) as u8;
        fs.count = (fs.count as i32 - delta) as u8;
        ts.pickup_delay = ts.pickup_delay.max(fs.pickup_delay);
        ts.age = ts.age.min(fs.age);
        let empty = fs.count == 0;
        set_state(w, to, ts);
        set_state(w, from, fs);
        if empty {
            w.entities.list[from].removed = true;
        }
        if w.entities.list[i].removed {
            break;
        }
    }
}

/// `ItemEntity.tick` of entity `i`.
pub fn tick(w: &mut World, i: usize) {
    let mut s = state(w, i);
    if s.item.is_empty() || s.count == 0 {
        w.entities.list[i].removed = true;
        return;
    }
    // Entity.tick = baseTick
    crate::entity::base_tick(w, i);
    if w.entities.list[i].removed {
        return;
    }
    if s.pickup_delay > 0 && s.pickup_delay != 32767 {
        s.pickup_delay -= 1;
    }
    set_state(w, i, s);
    let gravity = crate::entity::kinds::gravity(w, i);
    let old_movement;
    {
        let e = &mut w.entities.list[i];
        e.xo = e.pos.x;
        e.yo = e.pos.y;
        e.zo = e.pos.z;
        old_movement = e.delta_movement;
        if e.was_touching_water && e.fluid.water_height() > 0.1f32 as f64 {
            // setUnderwaterMovement: setFluidMovement(0.99F)
            let m = e.delta_movement;
            let k = 0.99f32 as f64;
            e.delta_movement = Vec3::new(m.x * k, m.y + if m.y < 0.06f32 as f64 { 5.0e-4f32 as f64 } else { 0.0 }, m.z * k);
        } else if gravity != 0.0 {
            e.delta_movement = e.delta_movement.add(0.0, -gravity, 0.0);
        }
    }
    let bb = w.entities.list[i].bb.deflate(1.0e-7, 1.0e-7, 1.0e-7);
    let stuck = !crate::entity::no_collision(w, i, bb);
    w.entities.list[i].no_physics = stuck;
    if stuck {
        w.errors.push("an item entity inside a block (moveTowardsClosestSpace: the entity's own random) is not modeled".into());
        return;
    }
    let (on_ground, hd, phase) = {
        let e = &w.entities.list[i];
        (e.on_ground, e.delta_movement.horizontal_distance_sqr(), (e.tick_count + e.network_id).rem_euclid(4))
    };
    if on_ground && !(hd > 1.0e-5f32 as f64) && phase != 0 {
        // applyEffectsFromBlocksForLastMovements: last tick's moves replayed
        collide::apply_effects_for_last_movements(w, i);
    } else {
        let d = w.entities.list[i].delta_movement;
        crate::entity::move_(w, i, MoverType::Self_, d);
        crate::entity::apply_effects_from_blocks(w, i);
        let air = 0.98f32;
        let mut ground = air;
        if w.entities.list[i].on_ground {
            let below = collide::get_on_pos(w, &w.entities.list[i], 0.999999);
            ground *= crate::entity::boat::block_friction(&w.get(below));
        }
        let e = &mut w.entities.list[i];
        e.delta_movement = e.delta_movement.multiply(ground as f64, air as f64, ground as f64);
        if e.on_ground && e.delta_movement.y < 0.0 {
            e.delta_movement = e.delta_movement.multiply(1.0, -0.5, 1.0);
        }
    }
    let moved = {
        let e = &w.entities.list[i];
        mth::floor(e.xo) != mth::floor(e.pos.x) || mth::floor(e.yo) != mth::floor(e.pos.y) || mth::floor(e.zo) != mth::floor(e.pos.z)
    };
    let rate = if moved { 2 } else { 40 };
    if w.entities.list[i].tick_count % rate == 0 {
        merge_with_neighbours(w, i);
    }
    if w.entities.list[i].removed {
        return;
    }
    let mut s = state(w, i);
    if s.age != -32768 {
        s.age += 1;
    }
    let age = s.age;
    set_state(w, i, s);
    crate::entity::fluid_push::update_fluid_interaction(w, i);
    let _ = old_movement;
    if age >= 6000 {
        w.entities.list[i].removed = true;
    }
}
