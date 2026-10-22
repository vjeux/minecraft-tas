//! Cactus (clk-misc, 2026-10-21: w86's cactus harvesters, study 18): `CactusBlock` and
//! `CactusFlowerBlock`, 26.3.
//!
//! A cactus stands on sand or a cactus, with no solid block (nor lava) beside it and no liquid
//! above; one that cannot breaks a tick after a shape update (`destroyBlock` with its drop). Its
//! random tick (with an empty cell above) climbs the column: a column of 3 at age 15 does nothing;
//! at age 8, when a cactus could stand above, a flower grows there on `nextDouble() <= 0.1` (0.25
//! on a column of 3); at age 15 under 3 tall the cactus grows a block (age 0) and restarts at 0;
//! under 15 the age climbs (flags 260: no neighbor updates). Its collision box is 14 px wide and
//! 15 tall; an entity whose box meets the cell takes 1 damage every tick (`entityInside`: an item,
//! 5 health, is gone on the fifth; each hit is an ENTITY_DAMAGE game event at the item).

use crate::block::{Block, Kind};
use crate::pos::{BlockPos, Dir};
use crate::ticks::Priority;
use crate::world::{World, flags};

/// `#supports_cactus` (= #sand) among redsim's blocks: sand, red sand.
fn supports(b: &Block) -> bool {
    matches!(b, Block::Falling { id } if matches!(crate::block::FALLING[*id as usize], "sand" | "red_sand"))
}

/// `CactusBlock.canSurvive`.
pub fn can_survive(w: &World, p: BlockPos) -> bool {
    for d in [Dir::North, Dir::East, Dir::South, Dir::West] {
        let q = p.rel(d);
        // (lava is not built by redsim)
        if crate::behavior::is_solid(&w.get(q)) {
            return false;
        }
    }
    let below = w.get(p.below());
    (matches!(below, Block::Cactus { .. }) || supports(&below)) && !liquid(&w.get(p.above()))
}

/// `BlockState.liquid()`: a fluid block (water, a bubble column).
fn liquid(b: &Block) -> bool {
    matches!(b, Block::Water { .. } | Block::BubbleColumn { .. })
}

/// `CactusBlock.updateShape`: a tick later it breaks when it cannot stand.
pub fn update_shape(w: &mut World, s: Block, p: BlockPos) -> Block {
    if !can_survive(w, p) {
        w.schedule_tick(p, Kind::Cactus, 1, Priority::Normal);
    }
    s
}

/// `CactusBlock.tick`.
pub fn tick(w: &mut World, p: BlockPos) {
    if !can_survive(w, p) {
        w.destroy_block(p, true, crate::world::UPDATE_LIMIT);
    }
}

/// `CactusFlowerBlock.mayPlaceOn` (`VegetationBlock.canSurvive`): on #support_override_cactus_flower
/// (cactus, farmland, ...) or a block whose top is sturdy at its center.
pub fn flower_can_survive(w: &World, p: BlockPos) -> bool {
    let below = w.get(p.below());
    matches!(below, Block::Cactus { .. } | Block::Farmland { .. })
        || crate::behavior::sturdy(w, &below, p.below(), Dir::Up, crate::behavior::Support::Center)
}

/// `VegetationBlock.updateShape` of the flower: air when it cannot stand.
pub fn flower_update_shape(w: &World, s: Block, p: BlockPos) -> Block {
    if !flower_can_survive(w, p) { Block::Air } else { s }
}

pub fn random_tick(w: &mut World, s: Block, p: BlockPos) {
    let Block::Cactus { age } = s else { return };
    let above = p.above();
    if !w.get(above).is_air() {
        return;
    }
    let mut height = 1;
    while matches!(w.get(p.offset(0, -height, 0)), Block::Cactus { .. }) {
        height += 1;
        if height == 3 && age == 15 {
            return;
        }
    }
    if age == 8 && can_survive(w, above) {
        let chance = if height >= 3 { 0.25 } else { 0.1 };
        let Some(r) = w.level_random.as_mut() else {
            let why = "a cactus's flower roll draws from the level random (World::mirror_level_random)".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
            return;
        };
        if r.next_double() <= chance {
            w.set_block_and_update(above, Block::CactusFlower);
        }
    } else if age == 15 && height < 3 {
        w.set_block_and_update(above, Block::Cactus { age: 0 });
        w.set_block(p, Block::Cactus { age: 0 }, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
        // level.neighborChanged(state age 0, above, cactus, null, false): a cactus has no
        // neighborChanged
    }
    if age < 15 {
        w.set_block(p, Block::Cactus { age: age + 1 }, flags::INVISIBLE | flags::SKIP_BLOCK_ENTITY_SIDEEFFECTS);
    }
}

/// `CactusBlock.entityInside`: `entity.hurt(cactus, 1.0F)`. An item loses 1 health (gone at 0;
/// `hurtServer` posts ENTITY_DAMAGE at the item, no source entity); other entities: a model error.
pub fn entity_inside(w: &mut World, i: usize) {
    let e = &mut w.entities.list[i];
    let at = [e.pos.x, e.pos.y, e.pos.z];
    match &mut e.kind {
        crate::entity::EntityKind::Item(s) => {
            s.health -= 1;
            let gone = s.health <= 0;
            let payload = s.payload;
            w.game_event_at("entity_damage", at, None, false);
            if gone {
                if payload != 0 {
                    w.errors.push("a cactus destroys a shulker box item, which spills its contents (not modeled)".into());
                }
                w.entities.list[i].removed = true;
            }
        }
        _ => {
            let why = "an entity other than an item inside a cactus (its damage) is not modeled".to_string();
            if !w.errors.contains(&why) {
                w.errors.push(why);
            }
        }
    }
}

/// `CactusBlock.SHAPE_COLLISION` = column(14, 0, 15), in px.
pub const COLLISION: [i32; 6] = [1, 15, 0, 15, 1, 15];

/// `ItemEntity.dampensVibrations`: an item of #dampens_vibrations (the wool items, wool carpets,
/// slabs and stairs).
pub fn item_dampens(item: &str) -> bool {
    let n = item.strip_prefix("minecraft:").unwrap_or(item);
    n.ends_with("_wool") || n.ends_with("_carpet") && !n.starts_with("moss") && !n.starts_with("pale_moss") || n.ends_with("_wool_slab") || n.ends_with("_wool_stairs")
}
