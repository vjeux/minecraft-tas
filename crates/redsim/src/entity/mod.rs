//! Entities (redsim M2; docs: designs/clocks/tracks/clockinfra/REDSIM-FLUIDS-ENTITIES.md):
//! boats, minecarts and item entities in ONE list, in insertion order (`entityTickList`), ticked
//! in the entity phase of [`World::tick`] (after block events, before block entities).
//!
//! Ownership: clockinfra owns this module's core (`mod.rs`, `vec3.rs`, `mth.rs`, `collide.rs`,
//! `fluid_push.rs`: `Entity.move`, collisions, `checkInsideBlocks`, fluid pushing); clk-misc owns
//! the kinds (`boat.rs`, `minecart.rs`, `item.rs`: `AbstractBoat`, `AbstractMinecart`, ...) and
//! the blocks that react to entities. The engine calls keep the game's names and order.
//!
//! STATUS: API skeleton. The list, its tick hook and the queries are live; `move_`,
//! `check_inside_blocks` and `update_fluid_height_and_do_fluid_pushing` are stubs that record a
//! model error (a run using them is not trustworthy) until M2 fills them. Falling blocks still
//! live in `gravity.rs` (they move into this list as a kind in M2, bit-identical).

pub mod armor_stand;
pub mod boat;
pub mod fluid_push;
pub mod item;
pub mod minecart;
pub mod mth;
pub mod vec3;

use crate::block::Block;
use crate::pos::BlockPos;
use crate::world::World;
pub use vec3::{Aabb, Vec3};

/// `MoverType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoverType {
    Self_,
    Player,
    Piston,
    ShulkerBox,
    Shulker,
}

/// What an entity is; the kind's own state is its payload (owned by the kind's module).
#[derive(Clone, PartialEq, Debug)]
pub enum EntityKind {
    Boat(boat::BoatState),
    Minecart(minecart::MinecartState),
    Item(item::ItemState),
    ArmorStand(armor_stand::ArmorStandState),
}

impl EntityKind {
    /// `EntityType.getDimensions`: (width, height).
    pub fn dims(&self) -> (f32, f32) {
        match self {
            EntityKind::Boat(_) => (boat::WIDTH, boat::HEIGHT),
            EntityKind::Minecart(_) => (minecart::WIDTH, minecart::HEIGHT),
            EntityKind::Item(_) => (item::WIDTH, item::HEIGHT),
            EntityKind::ArmorStand(_) => (armor_stand::WIDTH, armor_stand::HEIGHT),
        }
    }

    /// `EntityDimensions.eyeHeight` (`eyeHeight(..)` in `EntityTypes`, else height * 0.85F).
    pub fn eye_height(&self) -> f32 {
        match self {
            EntityKind::Boat(_) => 0.5625,
            EntityKind::Minecart(_) => minecart::HEIGHT * 0.85,
            EntityKind::Item(_) => 0.2125,
            EntityKind::ArmorStand(_) => 1.7775,
        }
    }
}

/// The `Entity` fields the engine reads and writes (the game's names).
#[derive(Clone, PartialEq, Debug)]
pub struct Entity {
    /// Insertion order (the tick order); never reused.
    pub id: u32,
    pub kind: EntityKind,
    /// `position()`.
    pub pos: Vec3,
    /// `getDeltaMovement()`.
    pub delta_movement: Vec3,
    pub y_rot: f32,
    pub x_rot: f32,
    pub y_rot_o: f32,
    pub x_rot_o: f32,
    /// `xo, yo, zo` (the position at the start of the tick).
    pub xo: f64,
    pub yo: f64,
    pub zo: f64,
    /// `xOld, yOld, zOld`.
    pub x_old: f64,
    pub y_old: f64,
    pub z_old: f64,
    /// `getBoundingBox()`.
    pub bb: Aabb,
    pub on_ground: bool,
    pub horizontal_collision: bool,
    pub vertical_collision: bool,
    pub vertical_collision_below: bool,
    pub minor_horizontal_collision: bool,
    pub removed: bool,
    pub was_touching_water: bool,
    pub was_eye_in_water: bool,
    /// `fluidHeight` per fluid tag: [water, lava].
    pub fluid_height: [f64; 2],
    pub first_tick: bool,
    pub fall_distance: f64,
    pub tick_count: i32,
    /// `EntityFluidInteraction` (trackers, current): `fluid_push.rs`.
    pub fluid: fluid_push::FluidInteraction,
}

/// `EntityDimensions.makeBoundingBox(x, y, z)`: float half width and height, widened.
pub fn make_bounding_box(pos: Vec3, width: f32, height: f32) -> Aabb {
    let w = (width / 2.0) as f64;
    let h = height as f64;
    Aabb::new(pos.x - w, pos.y, pos.z - w, pos.x + w, pos.y + h, pos.z + w)
}

impl Entity {
    /// A new entity at `pos` (as `EntityType.create` + `moveTo` would leave it).
    pub fn new(id: u32, kind: EntityKind, pos: Vec3, y_rot: f32) -> Entity {
        let (w, h) = kind.dims();
        Entity {
            id,
            kind,
            pos,
            delta_movement: Vec3::ZERO,
            y_rot,
            x_rot: 0.0,
            y_rot_o: y_rot,
            x_rot_o: 0.0,
            xo: pos.x,
            yo: pos.y,
            zo: pos.z,
            x_old: pos.x,
            y_old: pos.y,
            z_old: pos.z,
            bb: make_bounding_box(pos, w, h),
            on_ground: false,
            horizontal_collision: false,
            vertical_collision: false,
            vertical_collision_below: false,
            minor_horizontal_collision: false,
            removed: false,
            was_touching_water: false,
            was_eye_in_water: false,
            fluid_height: [0.0; 2],
            first_tick: true,
            fall_distance: 0.0,
            tick_count: 0,
            fluid: Default::default(),
        }
    }

    /// `setPos`: the position and the bounding box made from it.
    pub fn set_pos(&mut self, pos: Vec3) {
        self.pos = pos;
        let (w, h) = self.kind.dims();
        self.bb = make_bounding_box(pos, w, h);
    }

    /// `blockPosition()`.
    pub fn block_position(&self) -> BlockPos {
        BlockPos::new(mth::floor(self.pos.x), mth::floor(self.pos.y), mth::floor(self.pos.z))
    }
}

/// The level's entities, in insertion order.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Entities {
    pub list: Vec<Entity>,
    pub next_id: u32,
}

impl Entities {
    /// `addFreshEntity`: appended (ticked after every older entity). Returns its id.
    pub fn add(&mut self, kind: EntityKind, pos: Vec3, y_rot: f32) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.list.push(Entity::new(id, kind, pos, y_rot));
        id
    }

    pub fn is_empty(&self) -> bool {
        self.list.iter().all(|e| e.removed)
    }
}

/// The entity phase: every live entity's `tick`, in insertion order; removed ones leave the list.
pub fn tick_all(w: &mut World) {
    if w.entities.list.is_empty() {
        return;
    }
    let n = w.entities.list.len();
    for i in 0..n {
        if w.entities.list[i].removed {
            continue;
        }
        match w.entities.list[i].kind {
            EntityKind::Boat(_) => boat::tick(w, i),
            EntityKind::Minecart(_) => minecart::tick(w, i),
            EntityKind::Item(_) => item::tick(w, i),
            EntityKind::ArmorStand(_) => armor_stand::tick(w, i),
        }
    }
    w.entities.list.retain(|e| !e.removed);
}

/// `Entity.move(type, delta)` of entity `i` (index in the list). STUB until M2.
pub fn move_(w: &mut World, i: usize, _mover: MoverType, delta: Vec3) {
    w.errors.push("entity move is not modeled yet (redsim M2)".into());
    let p = w.entities.list[i].pos.add_v(delta);
    w.entities.list[i].set_pos(p);
}

/// `Entity.checkInsideBlocks` + `applyEffectsFromBlocks` of entity `i`: every block whose shape
/// the entity's box touches gets its `entityInside` (clk-misc's plates, tripwire, detector rails,
/// dripleaf, bubble columns). STUB until M2.
pub fn check_inside_blocks(w: &mut World, _i: usize) {
    w.errors.push("entity checkInsideBlocks is not modeled yet (redsim M2)".into());
}

/// `Entity.updateFluidInteraction` (the old name `updateFluidHeightAndDoFluidPushing`): see
/// [`fluid_push::update_fluid_interaction`]; the fluid and scale are the game's (water, 0.014).
pub fn update_fluid_height_and_do_fluid_pushing(w: &mut World, i: usize, _fluid: crate::fluid::Fluid, _flow_scale: f64) -> bool {
    fluid_push::update_fluid_interaction(w, i)
}

impl World {
    /// The block collision boxes that intersect `bb` (`BlockCollisions` with the empty collision
    /// context: every cell from floor(min - 1e-7) - 1 to floor(max + 1e-7) + 1, each block's
    /// `behavior::collision_boxes` moved to its cell). Moving pistons and context-dependent shapes
    /// (scaffolding) come in M2.
    pub fn collision_shapes(&self, bb: Aabb) -> Vec<Aabb> {
        let lo = |v: f64| mth::floor(v - 1.0e-7) - 1;
        let hi = |v: f64| mth::floor(v + 1.0e-7) + 1;
        let mut out = Vec::new();
        for y in lo(bb.min_y)..=hi(bb.max_y) {
            for z in lo(bb.min_z)..=hi(bb.max_z) {
                for x in lo(bb.min_x)..=hi(bb.max_x) {
                    let p = BlockPos::new(x, y, z);
                    let b = self.get(p);
                    if matches!(b, Block::Air) {
                        continue;
                    }
                    for c in crate::behavior::collision_boxes(&b) {
                        let f = |v: i32| v as f64 / 16.0;
                        let a = Aabb::new(x as f64 + f(c[0]), y as f64 + f(c[2]), z as f64 + f(c[4]), x as f64 + f(c[1]), y as f64 + f(c[3]), z as f64 + f(c[5]));
                        if a.intersects(bb) {
                            out.push(a);
                        }
                    }
                }
            }
        }
        out
    }

    /// `getEntities(except none, bb, filter)`: the live entities whose box intersects `bb`, in
    /// list order (indices into `entities.list`).
    pub fn get_entities(&self, bb: Aabb, filter: impl Fn(&Entity) -> bool) -> Vec<usize> {
        self.entities.list.iter().enumerate().filter(|(_, e)| !e.removed && e.bb.intersects(bb) && filter(e)).map(|(i, _)| i).collect()
    }
}
