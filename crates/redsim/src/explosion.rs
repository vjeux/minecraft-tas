//! TNT explosions (clk-misc, 2026-10-20): `ServerExplosion` read in the 26.3 source, in its own
//! order. A primed TNT whose fuse runs out (`PrimedTnt.tick` -> `explode`) calls
//! `ServerLevel.explode(this, .., x, y + 0.98F * 0.0625, z, 4.0F, false, TNT)`: with the default
//! `tnt_explosion_drop_decay` off the block interaction is `DESTROY`. [`explode`]:
//!
//! 1. `gameEvent(EXPLODE, center)` (a vibration at the exact center: a model error near sculk
//!    sensors).
//! 2. `calculateExplodedPositions`: from the center, 1352 rays (the 16x16x16 grid's surface,
//!    normalized), each starting with `radius * (0.7F + nextFloat() * 0.6F)` (the level random,
//!    or [`Rolls`]), stepping 0.3 (as a float widened); every cell it passes loses `(resistance +
//!    0.3F) * 0.3F` when it is not air or holds a fluid (resistance = max(block, fluid): water
//!    100), joins the set while the power stays positive (air too), and the power loses
//!    `0.22500001F` per step. The set is a Java `HashSet<BlockPos>`: its iteration order (the
//!    hash table's buckets) is reproduced exactly ([`JavaHashSet`]).
//! 3. `hurtEntities`: every entity whose box meets the center's box +- (2 radius + 1), in entity
//!    section order: within 2 radius, `exposure` = the share of a grid of points on its box that
//!    see the center (`getSeenPercent`: block collision raycasts, `BlockGetter.clip`), the damage
//!    `((1 - d) e)^2 + (1 - d) e) / 2 * 7 * 2r + 1` (items lose health, vehicles gain damage, armor
//!    stands break, TNT and falling blocks take none) and a push of `(1 - d) * e` along the
//!    direction from the center to its eye (a TNT: its feet).
//! 4. `interactWithBlocks`: the list shuffled with the level random (`Util.shuffle`), then per
//!    cell that is not air: its drops collected (the loot table, merged into stacks of at most
//!    16 per item, `StackCollector`), `setBlockAndUpdate(air)` (its neighbors react at once), then
//!    `wasExploded` (a TNT block becomes a primed TNT with a 10..29 tick fuse). Then every
//!    collected stack pops (`Block.popResource`).

use crate::block::Block;
use crate::entity::{self, mth, Aabb, EntityKind, Vec3};
use crate::pos::BlockPos;
use crate::world::{flags, World};

/// `PrimedTnt.explosionPower`.
pub const TNT_POWER: f32 = 4.0;

/// The largest `nextFloat()` (`next(24) * 2^-24` with `next(24) = 2^24 - 1`).
pub const MAX_FLOAT: f32 = 1.0 - 1.0 / 16_777_216.0;

/// How each ray's factor `0.7F + nextFloat() * 0.6F` is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Rolls {
    /// The game's: one `nextFloat()` per ray from the mirrored level random
    /// (`World::mirror_level_random`, else a model error), then the block list shuffled with it.
    #[default]
    Level,
    /// Every ray at the largest `nextFloat()` ([`MAX_FLOAT`]): the strongest explosion the game
    /// can roll (damage and destruction grow with every ray's power, so surviving it is surviving
    /// every roll). The block list keeps the set's order and nothing is drawn.
    Max,
    /// Every ray at `nextFloat() = 0`: the weakest.
    Min,
    /// Every ray at this `nextFloat()` value (in [0, 1)).
    Fixed(f32),
}

/// `Block.getExplosionResistance()` (the `explosionResistance` of its properties in `Blocks`,
/// 26.3).
pub fn block_resistance(b: &Block) -> f32 {
    use crate::block::Simple as S;
    let simple = |s: &S| -> f32 {
        match s.name() {
            "obsidian" | "crying_obsidian" | "netherite_block" => 1200.0,
            "bedrock" => 3_600_000.0,
            "end_stone" => 9.0,
            "glass" | "tinted_glass" | "white_stained_glass" | "black_stained_glass" | "glowstone" | "sea_lantern" => 0.3,
            "packed_ice" | "soul_sand" | "soul_soil" | "dirt" | "mud" | "magma_block" => 0.5,
            "clay" | "grass_block" => 0.6,
            "calcite" => 0.75,
            "sandstone" | "quartz_block" => 0.8,
            n if n.ends_with("_wool") => 0.8,
            "pumpkin" | "shroomlight" | "nether_wart_block" | "warped_wart_block" => 1.0,
            "amethyst_block" | "bookshelf" => 1.5,
            "white_concrete" | "black_concrete" => 1.8,
            "crafting_table" => 2.5,
            "dried_kelp_block" => 2.5,
            "oak_planks" | "spruce_planks" | "lapis_block" | "mud_bricks" => 3.0,
            "terracotta" => 4.2,
            "slime_block" | "honey_block" => 0.0,
            // stone-likes, metal blocks, copper grates: 6
            _ => 6.0,
        }
    };
    match b {
        Block::Air => 0.0,
        Block::Simple(s) => simple(s),
        Block::Glazed { .. } => 1.4,
        Block::Wire { .. } | Block::Torch { .. } | Block::WallTorch { .. } | Block::Repeater { .. } | Block::Comparator { .. } => 0.0,
        Block::RedstoneBlock => 6.0,
        Block::Lever { .. } | Block::Button { .. } | Block::Plate { .. } | Block::Target { .. } | Block::Cake { .. } => 0.5,
        Block::Observer { .. } => 3.0,
        Block::Lamp { .. } => 0.3,
        Block::Bulb { .. } => 6.0,
        Block::Note { .. } => 0.8,
        Block::Trapdoor { iron, .. } | Block::Door { iron, .. } => {
            if *iron {
                5.0
            } else {
                3.0
            }
        }
        Block::FenceGate { .. } => 3.0,
        Block::Rail { .. } | Block::PlainRail { .. } | Block::DetectorRail { .. } => 0.7,
        Block::Dispenser { .. } | Block::Crafter { .. } | Block::Lantern { .. } => 3.5,
        Block::Hopper { .. } => 4.8,
        Block::Slab { .. } | Block::StoneSlab { .. } => 6.0,
        Block::Leaves { .. } => 0.2,
        Block::Scaffolding { .. } | Block::Tripwire { .. } | Block::TripwireHook { .. } | Block::Tnt { .. } | Block::CoralFan { .. } | Block::CoralWallFan { .. } | Block::BubbleColumn { .. } => 0.0,
        Block::Log { .. } => 2.0,
        Block::Skull { .. } | Block::WallSkull { .. } | Block::WallBanner { .. } => 1.0,
        Block::EndPortalFrame { .. } => 3_600_000.0,
        Block::Composter { .. } => 0.6,
        Block::RespawnAnchor { .. } => 1200.0,
        Block::Piston { .. } | Block::PistonHead { .. } | Block::Sculk { .. } | Block::ChiseledBookshelf { .. } => 1.5,
        // strength(-1.0F): explosionResistance is clamped at 0
        Block::MovingPiston { .. } => 0.0,
        Block::Falling { id } => {
            if crate::block::FALLING[*id as usize] == "gravel" {
                0.6
            } else {
                0.5
            }
        }
        Block::BigDripleaf { .. } => 0.1,
        Block::DaylightDetector { .. } => 0.2,
        Block::PowderSnow => 0.25,
        Block::Jukebox { .. } => 6.0,
        Block::Water { .. } => 100.0,
        Block::ShulkerBox { .. } => 2.0,
        Block::Chest { .. } => 2.5,
        Block::Fence { kind, .. } => {
            if crate::fence::FENCES[*kind as usize] == "nether_brick" {
                6.0
            } else {
                3.0
            }
        }
    }
}

/// `ExplosionDamageCalculator.getBlockExplosionResistance`: none for air without a fluid, else
/// max(the block's, the fluid's: water 100).
fn resistance(w: &World, p: BlockPos, b: &Block) -> Option<f32> {
    let fluid = crate::fluid::fluid_state(w, p);
    if b.is_air() && fluid.is_empty() {
        return None;
    }
    let f = if fluid.is_empty() { 0.0 } else { 100.0 };
    Some(block_resistance(b).max(f))
}

/// A Java `HashSet<BlockPos>` (`HashMap`'s table: 16 buckets, doubled past 3/4 full, a bucket of 9
/// first doubling a small table; insertion order within a bucket, kept by every split): only
/// what the explosion needs, its iteration order.
struct JavaHashSet {
    bins: Vec<Vec<(u32, BlockPos)>>,
    size: usize,
    threshold: usize,
    /// A bucket of 9 in a table of 64 or more becomes a tree (another iteration order).
    treeified: bool,
}

impl JavaHashSet {
    fn new() -> JavaHashSet {
        JavaHashSet { bins: Vec::new(), size: 0, threshold: 0, treeified: false }
    }

    /// `HashMap.hash(BlockPos)`: `Vec3i.hashCode` = `(y + z * 31) * 31 + x`, spread `h ^ (h >>> 16)`.
    fn hash(p: BlockPos) -> u32 {
        let h = p.y.wrapping_add(p.z.wrapping_mul(31)).wrapping_mul(31).wrapping_add(p.x) as u32;
        h ^ (h >> 16)
    }

    fn resize(&mut self) {
        let old = self.bins.len();
        let (cap, thr) = if old == 0 { (16, 12) } else { (old * 2, self.threshold * 2) };
        let mut bins: Vec<Vec<(u32, BlockPos)>> = vec![Vec::new(); cap];
        for bin in std::mem::take(&mut self.bins) {
            for (h, p) in bin {
                bins[h as usize & (cap - 1)].push((h, p));
            }
        }
        self.bins = bins;
        self.threshold = thr;
    }

    /// `HashMap.putVal` (an existing key changes nothing).
    fn insert(&mut self, p: BlockPos) {
        if self.bins.is_empty() {
            self.resize();
        }
        let h = Self::hash(p);
        let n = self.bins.len();
        let bin = &mut self.bins[h as usize & (n - 1)];
        if bin.iter().any(|(_, q)| *q == p) {
            return;
        }
        let had = bin.len();
        bin.push((h, p));
        // the new node appended after `had` nodes: binCount = had - 1 >= TREEIFY_THRESHOLD - 1
        if had >= 8 {
            if n < 64 {
                self.resize();
            } else {
                self.treeified = true;
            }
        }
        self.size += 1;
        if self.size > self.threshold {
            self.resize();
        }
    }

    /// `new ObjectArrayList<>(set)`: the iteration order.
    fn into_vec(self) -> Vec<BlockPos> {
        self.bins.into_iter().flatten().map(|(_, p)| p).collect()
    }
}

/// `ServerExplosion.calculateExplodedPositions`, in the set's iteration order.
fn exploded_positions(w: &mut World, center: Vec3, radius: f32, rolls: Rolls) -> Vec<BlockPos> {
    let mut set = JavaHashSet::new();
    let step = 0.3f32 as f64;
    for xx in 0..16 {
        for yy in 0..16 {
            for zz in 0..16 {
                if !(xx == 0 || xx == 15 || yy == 0 || yy == 15 || zz == 0 || zz == 15) {
                    continue;
                }
                // `xx / 15.0F * 2.0F - 1.0F`: float arithmetic, widened
                let mut xd = (xx as f32 / 15.0f32 * 2.0f32 - 1.0f32) as f64;
                let mut yd = (yy as f32 / 15.0f32 * 2.0f32 - 1.0f32) as f64;
                let mut zd = (zz as f32 / 15.0f32 * 2.0f32 - 1.0f32) as f64;
                let d = (xd * xd + yd * yd + zd * zd).sqrt();
                xd /= d;
                yd /= d;
                zd /= d;
                let f = match rolls {
                    Rolls::Level => w.level_random.as_mut().map(|r| r.next_float()).unwrap_or(0.0),
                    Rolls::Max => MAX_FLOAT,
                    Rolls::Min => 0.0,
                    Rolls::Fixed(v) => v,
                };
                let mut power = radius * (0.7f32 + f * 0.6f32);
                let (mut xp, mut yp, mut zp) = (center.x, center.y, center.z);
                while power > 0.0 {
                    let pos = BlockPos::new(mth::floor(xp), mth::floor(yp), mth::floor(zp));
                    // isInWorldBounds (the build height, -64..=319: a TNT that fell into the void
                    // below it reaches no cell, and its shuffle draws nothing)
                    if pos.y < crate::gravity::MIN_Y || pos.y > crate::gravity::MAX_Y {
                        break;
                    }
                    let b = w.get(pos);
                    if let Some(r) = resistance(w, pos, &b) {
                        power -= (r + 0.3f32) * 0.3f32;
                    }
                    // shouldBlockExplode: true (no nether portals)
                    if power > 0.0 {
                        set.insert(pos);
                    }
                    xp += xd * step;
                    yp += yd * step;
                    zp += zd * step;
                    power -= 0.225_000_01f32;
                }
            }
        }
    }
    if set.treeified {
        w.errors.push("explosion: a bucket of the exploded set's HashSet became a tree (its iteration order is not modeled)".into());
    }
    set.into_vec()
}

// ------------------------------------------------------------------ raycasts (getSeenPercent)

/// `VoxelShape.clip` hit test of a shape (`boxes`, world coordinates of the cell `p`): the point
/// 0.001 along the segment inside the shape, else the nearest face crossing (`AABB.clip`).
fn shape_hits(boxes: &[Aabb], from: Vec3, to: Vec3) -> bool {
    if boxes.is_empty() {
        return false;
    }
    let diff = to.subtract_v(from);
    if diff.length_sqr() < 1.0e-7 {
        return false;
    }
    let t = from.add_v(diff.scale(0.001));
    if boxes.iter().any(|b| b.min_x <= t.x && t.x < b.max_x && b.min_y <= t.y && t.y < b.max_y && b.min_z <= t.z && t.z < b.max_z) {
        return true;
    }
    // AABB.clip: the smallest 0 < s < 1 over every face the segment enters
    let (dx, dy, dz) = (diff.x, diff.y, diff.z);
    let mut scale = 1.0f64;
    let mut hit = false;
    let mut clip_point = |da: f64, db: f64, dc: f64, point: f64, min_b: f64, max_b: f64, min_c: f64, max_c: f64, fa: f64, fb: f64, fc: f64| {
        let s = (point - fa) / da;
        let pb = fb + s * db;
        let pc = fc + s * dc;
        if 0.0 < s && s < scale && min_b - 1.0e-7 < pb && pb < max_b + 1.0e-7 && min_c - 1.0e-7 < pc && pc < max_c + 1.0e-7 {
            scale = s;
            hit = true;
        }
    };
    for b in boxes {
        if dx > 1.0e-7 {
            clip_point(dx, dy, dz, b.min_x, b.min_y, b.max_y, b.min_z, b.max_z, from.x, from.y, from.z);
        } else if dx < -1.0e-7 {
            clip_point(dx, dy, dz, b.max_x, b.min_y, b.max_y, b.min_z, b.max_z, from.x, from.y, from.z);
        }
        if dy > 1.0e-7 {
            clip_point(dy, dz, dx, b.min_y, b.min_z, b.max_z, b.min_x, b.max_x, from.y, from.z, from.x);
        } else if dy < -1.0e-7 {
            clip_point(dy, dz, dx, b.max_y, b.min_z, b.max_z, b.min_x, b.max_x, from.y, from.z, from.x);
        }
        if dz > 1.0e-7 {
            clip_point(dz, dx, dy, b.min_z, b.min_x, b.max_x, b.min_y, b.max_y, from.z, from.x, from.y);
        } else if dz < -1.0e-7 {
            clip_point(dz, dx, dy, b.max_z, b.min_x, b.max_x, b.min_y, b.max_y, from.z, from.x, from.y);
        }
    }
    hit
}

/// `level.clip(new ClipContext(from, to, COLLIDER, NONE, entity)).getType() == MISS`:
/// `BlockGetter.traverseBlocks` over the cells the segment crosses, each block's collision shape
/// tested (fluids ignored).
pub fn clip_misses(w: &World, from: Vec3, to: Vec3) -> bool {
    if from == to {
        return true;
    }
    let to_x = mth::lerp(-1.0e-7, to.x, from.x);
    let to_y = mth::lerp(-1.0e-7, to.y, from.y);
    let to_z = mth::lerp(-1.0e-7, to.z, from.z);
    let from_x = mth::lerp(-1.0e-7, from.x, to.x);
    let from_y = mth::lerp(-1.0e-7, from.y, to.y);
    let from_z = mth::lerp(-1.0e-7, from.z, to.z);
    let (mut cx, mut cy, mut cz) = (mth::floor(from_x), mth::floor(from_y), mth::floor(from_z));
    let hits = |x: i32, y: i32, z: i32| -> bool {
        let p = BlockPos::new(x, y, z);
        let s = w.get(p);
        !s.is_air() && shape_hits(&entity::collide::boxes_at(w, &s, p), from, to)
    };
    if hits(cx, cy, cz) {
        return false;
    }
    let (dx, dy, dz) = (to_x - from_x, to_y - from_y, to_z - from_z);
    let (sx, sy, sz) = (mth::sign(dx), mth::sign(dy), mth::sign(dz));
    let t_dx = if sx == 0 { f64::MAX } else { sx as f64 / dx };
    let t_dy = if sy == 0 { f64::MAX } else { sy as f64 / dy };
    let t_dz = if sz == 0 { f64::MAX } else { sz as f64 / dz };
    let mut tx = t_dx * if sx > 0 { 1.0 - mth::frac(from_x) } else { mth::frac(from_x) };
    let mut ty = t_dy * if sy > 0 { 1.0 - mth::frac(from_y) } else { mth::frac(from_y) };
    let mut tz = t_dz * if sz > 0 { 1.0 - mth::frac(from_z) } else { mth::frac(from_z) };
    while tx <= 1.0 || ty <= 1.0 || tz <= 1.0 {
        if tx < ty {
            if tx < tz {
                cx += sx;
                tx += t_dx;
            } else {
                cz += sz;
                tz += t_dz;
            }
        } else if ty < tz {
            cy += sy;
            ty += t_dy;
        } else {
            cz += sz;
            tz += t_dz;
        }
        if hits(cx, cy, cz) {
            return false;
        }
    }
    true
}

/// `ServerExplosion.getSeenPercent(center, entity)` for an entity box `bb`.
pub fn seen_percent(w: &World, center: Vec3, bb: Aabb) -> f32 {
    let xs = 1.0 / ((bb.max_x - bb.min_x) * 2.0 + 1.0);
    let ys = 1.0 / ((bb.max_y - bb.min_y) * 2.0 + 1.0);
    let zs = 1.0 / ((bb.max_z - bb.min_z) * 2.0 + 1.0);
    let x_off = (1.0 - (1.0 / xs).floor() * xs) / 2.0;
    let z_off = (1.0 - (1.0 / zs).floor() * zs) / 2.0;
    if xs < 0.0 || ys < 0.0 || zs < 0.0 {
        return 0.0;
    }
    let (mut hits, mut count) = (0i32, 0i32);
    let mut xx = 0.0;
    while xx <= 1.0 {
        let mut yy = 0.0;
        while yy <= 1.0 {
            let mut zz = 0.0;
            while zz <= 1.0 {
                let x = mth::lerp(xx, bb.min_x, bb.max_x);
                let y = mth::lerp(yy, bb.min_y, bb.max_y);
                let z = mth::lerp(zz, bb.min_z, bb.max_z);
                if clip_misses(w, Vec3::new(x + x_off, y, z + z_off), center) {
                    hits += 1;
                }
                count += 1;
                zz += zs;
            }
            yy += ys;
        }
        xx += xs;
    }
    hits as f32 / count as f32
}

// ------------------------------------------------------------------ entities

/// An entity an explosion can reach: one of `entities.list` or a falling block.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Main(usize),
    Falling(usize),
}

/// `SectionPos.asLong` order within one x slab: z then y, as the masked (unsigned) fields.
fn section_key(p: Vec3) -> (i32, u64) {
    let (bx, by, bz) = (mth::floor(p.x), mth::floor(p.y), mth::floor(p.z));
    let (sx, sy, sz) = (bx >> 4, by >> 4, bz >> 4);
    (sx, ((sz as i64 & 0x3F_FFFF) as u64) << 20 | (sy as i64 & 0xF_FFFF) as u64)
}

/// `level.getEntities(source, box)`: the live entities (falling blocks included) whose box meets
/// `bb`, the source excluded, in entity-section order (x slab, then the section key), then spawn
/// order within a section.
fn entities_in(w: &World, source: Option<u32>, bb: Aabb) -> Vec<Target> {
    let mut v: Vec<((i32, u64), u32, Target)> = Vec::new();
    for (i, e) in w.entities.list.iter().enumerate() {
        if !e.removed && Some(e.id) != source && e.bb.intersects(bb) {
            v.push((section_key(e.pos), e.id, Target::Main(i)));
        }
    }
    for (i, f) in w.gravity.entities.iter().enumerate() {
        if !f.removed && Some(f.id) != source && f.bb().intersects(bb) {
            v.push((section_key(Vec3::new(f.x, f.y, f.z)), f.id, Target::Falling(i)));
        }
    }
    v.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    v.into_iter().map(|x| x.2).collect()
}

/// `ServerExplosion.hurtEntities`.
fn hurt_entities(w: &mut World, source: Option<u32>, center: Vec3, radius: f32) {
    if radius < 1.0e-5 {
        return;
    }
    let dr = radius * 2.0;
    let x0 = mth::floor(center.x - dr as f64 - 1.0) as f64;
    let x1 = mth::floor(center.x + dr as f64 + 1.0) as f64;
    let y0 = mth::floor(center.y - dr as f64 - 1.0) as f64;
    let y1 = mth::floor(center.y + dr as f64 + 1.0) as f64;
    let z0 = mth::floor(center.z - dr as f64 - 1.0) as f64;
    let z1 = mth::floor(center.z + dr as f64 + 1.0) as f64;
    for t in entities_in(w, source, Aabb::new(x0, y0, z0, x1, y1, z1)) {
        let (pos, bb, eye, tnt) = match t {
            Target::Main(i) => {
                let e = &w.entities.list[i];
                (e.pos, e.bb, e.kind.eye_height(), matches!(e.kind, EntityKind::PrimedTnt(_)))
            }
            Target::Falling(i) => {
                let f = &w.gravity.entities[i];
                (Vec3::new(f.x, f.y, f.z), f.bb(), 0.98f32 * 0.85f32, false)
            }
        };
        // ignoreExplosion: an invisible armor stand (redsim's are visible); the rest are hit
        let dist = pos.distance_to_sqr(center).sqrt() / dr as f64;
        if dist > 1.0 {
            continue;
        }
        let origin = if tnt { pos } else { Vec3::new(pos.x, pos.y + eye as f64, pos.z) };
        let direction = origin.subtract_v(center).normalize();
        let exposure = seen_percent(w, center, bb);
        // getEntityDamageAmount
        let pow = (1.0 - dist) * exposure as f64;
        let damage = ((pow * pow + pow) / 2.0 * 7.0 * dr as f64 + 1.0) as f32;
        hurt(w, t, damage);
        // knockback: (1 - dist) * exposure * multiplier (1) * (1 - knockback resistance (0))
        let power = (1.0 - dist) * exposure as f64 * 1.0f32 as f64 * (1.0 - 0.0);
        let k = direction.scale(power);
        if k.x.is_finite() && k.y.is_finite() && k.z.is_finite() {
            match t {
                Target::Main(i) => {
                    let e = &mut w.entities.list[i];
                    e.delta_movement = e.delta_movement.add(k.x, k.y, k.z);
                }
                Target::Falling(i) => {
                    let f = &mut w.gravity.entities[i];
                    f.vx += k.x;
                    f.vy += k.y;
                    f.vz += k.z;
                }
            }
        }
    }
}

/// `hurtServer(level, explosion damage source, damage)` of one target.
fn hurt(w: &mut World, t: Target, damage: f32) {
    let Target::Main(i) = t else { return };
    if !w.sculk.is_empty() {
        w.errors.push("explosion: an entity's damage vibration near sculk sensors is not modeled".into());
    }
    let e = &mut w.entities.list[i];
    match &mut e.kind {
        // PrimedTnt.hurtServer: false
        EntityKind::PrimedTnt(_) => {}
        // ItemEntity.hurtServer: health = (int)(health - damage); gone at 0
        EntityKind::Item(s) => {
            s.health = (s.health as f32 - damage) as i32;
            if s.health <= 0 {
                if s.payload != 0 {
                    w.errors.push("explosion: a destroyed shulker box item spills its contents (not modeled)".into());
                }
                w.entities.list[i].removed = true;
            }
        }
        // VehicleEntity.hurtServer: damage += 10 x, destroyed past 40 (its item drop is not
        // reproducible: the item's own random)
        EntityKind::Boat(_) | EntityKind::Minecart(_) => {
            e.vehicle_damage += damage * 10.0;
            if e.vehicle_damage > 40.0 {
                e.removed = true;
                w.errors.push("explosion: a vehicle it destroys drops its item (not modeled: the item's random)".into());
            }
        }
        // ArmorStand.hurtServer: an explosion breaks it (its drops are not reproducible)
        EntityKind::ArmorStand(_) => {
            e.removed = true;
            w.errors.push("explosion: an armor stand it breaks drops its items (not modeled)".into());
        }
    }
}

// ------------------------------------------------------------------ blocks

/// A `StackCollector`: one stack of the explosion's drops, popped at `pos`.
struct Collector {
    pos: BlockPos,
    item: String,
    count: u32,
    payload: u16,
}

/// `ServerExplosion.addOrAppendStack`: merged into the first collectors that take it (the same
/// item, the sum within a stack, at most 16 each), the rest in a new one.
fn add_or_append(stacks: &mut Vec<Collector>, pos: BlockPos, item: String, mut count: u32, payload: u16) {
    let max = crate::loot::max_stack(&item);
    for c in stacks.iter_mut() {
        if c.item == item && c.payload == payload && count + c.count <= max {
            let delta = (max.min(16) as i64 - c.count as i64).min(count as i64);
            c.count = (c.count as i64 + delta) as u32;
            count = (count as i64 - delta) as u32;
        }
        if count == 0 {
            return;
        }
    }
    stacks.push(Collector { pos, item, count, payload });
}

/// `ServerExplosion.interactWithBlocks`.
fn interact_with_blocks(w: &mut World, mut list: Vec<BlockPos>, rolls: Rolls) {
    if rolls == Rolls::Level {
        if let Some(r) = w.level_random.as_mut() {
            // Util.shuffle
            for i in (2..=list.len()).rev() {
                let j = r.next_int(i as i32) as usize;
                list.swap(i - 1, j);
            }
        }
    }
    let mut stacks: Vec<Collector> = Vec::new();
    for pos in list {
        let s = w.get(pos);
        if s.is_air() {
            continue;
        }
        // Block.onExplosionHit (DESTROY): the drops (dropFromExplosion: not a TNT block). A moving
        // block drops what it moves (`MovingPistonBlock.getDrops`: its block entity's
        // `movedState`, nothing without one); overwriting it runs `preRemoveSideEffects` ->
        // `finalTick` on a cell that is air by then (nothing placed), and `wasExploded` is the
        // default (a moving TNT is not primed)
        if !matches!(s, Block::Tnt { .. }) {
            let dropped = match s {
                Block::MovingPiston { .. } => w.piston.be(pos).map(|be| be.moved),
                _ => Some(s),
            };
            let payload = crate::redstone::shulker::box_payload(w, pos, &s);
            if payload.is_none() && w.containers.map.get(&pos).is_some_and(|c| c.slots.iter().any(|x| x.is_some())) {
                w.errors.push(format!("explosion: the container at {pos} spills its items (not modeled)"));
            }
            match dropped.map(|d| crate::loot::drops(&d)) {
                Some(Ok(v)) => {
                    for (item, n) in v {
                        add_or_append(&mut stacks, pos, item, n, payload.unwrap_or(0));
                    }
                }
                Some(Err(e)) => w.errors.push(format!("explosion: {e}")),
                None => {}
            }
        }
        // setBlockAndUpdate(air) (flags 3), then wasExploded
        if let Some(b) = w.explosions.last_mut() {
            b.destroyed.push((pos, s));
        }
        w.set_block(pos, Block::Air, flags::NEIGHBORS | flags::CLIENTS);
        if matches!(s, Block::Tnt { .. }) {
            crate::tnt::was_exploded(w, pos);
        }
    }
    for c in stacks {
        w.pop_resource(c.pos, &c.item, c.count, c.payload);
    }
}

/// One explosion's record (`World::explosions`).
#[derive(Clone, Debug, PartialEq)]
pub struct Blast {
    /// The game time it happened at.
    pub time: i64,
    pub center: Vec3,
    /// The blocks it destroyed, in the order it destroyed them (a block its neighbor updates
    /// removed in between is not listed: it was no longer there).
    pub destroyed: Vec<(BlockPos, Block)>,
    /// How many cells the rays reached (`toBlow`, air included: the shuffle's length).
    pub positions: usize,
}

/// A world where TNT lives and explodes like in a bridge sandbox seeded `seed` with the forced
/// chunk range `chunks`: TNT entities (`tnt_kick`, their kicks from the level random), the level
/// random mirrored, drops seeded like the bridge's (`drop_seed`), nothing removed before it
/// explodes, the game's rolls. Every entity ticks wherever it flies (`entity_chunks` cleared: in a
/// real world the player keeps those chunks loaded, so a TNT frozen at the sandbox's chunk border
/// would be an artifact). The viewer runs TNT designs this way (and `web check` its reference).
pub fn explode_on(w: &mut World, seed: i64, chunks: [i32; 4]) {
    w.tnt_kick = Some(0.0);
    w.mirror_level_random(seed, chunks);
    w.entity_chunks = None;
    w.drop_seed = Some(seed);
    w.remove_primed = false;
    w.explosion_rolls = Rolls::Level;
}

/// `ServerLevel.explode(source, .., center, radius, fire = false, TNT)` -> `ServerExplosion.explode`
/// (see the module doc). `source`: the exploding entity's id (excluded from the entities hit).
pub fn explode(w: &mut World, source: Option<u32>, center: Vec3, radius: f32) {
    let rolls = w.explosion_rolls;
    if rolls == Rolls::Level && w.level_random.is_none() {
        w.errors.push("explosion: Rolls::Level needs the mirrored level random (World::mirror_level_random), or choose Rolls::Max / Min / Fixed".into());
        return;
    }
    // gameEvent(EXPLODE, center)
    w.game_events.push(("explode", BlockPos::new(mth::floor(center.x), mth::floor(center.y), mth::floor(center.z))));
    if !w.sculk.is_empty() {
        w.errors.push("explosion: its vibration near sculk sensors is not modeled".into());
    }
    w.explosions.push(Blast { time: w.time, center, destroyed: Vec::new(), positions: 0 });
    let blow = exploded_positions(w, center, radius, rolls);
    if let Some(b) = w.explosions.last_mut() {
        b.positions = blow.len();
    }
    hurt_entities(w, source, center, radius);
    interact_with_blocks(w, blow, rolls);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Java's HashMap order for a handful of keys: buckets by the spread hash, a resize at 13.
    #[test]
    fn hashset_order_is_by_bucket() {
        let mut s = JavaHashSet::new();
        let ps: Vec<BlockPos> = (0..20).map(|i| BlockPos::new(i % 5, i / 5, 0)).collect();
        for p in &ps {
            s.insert(*p);
        }
        assert_eq!(s.bins.len(), 32);
        let v = s.into_vec();
        assert_eq!(v.len(), 20);
        let hs: Vec<usize> = v.iter().map(|p| JavaHashSet::hash(*p) as usize & 31).collect();
        assert!(hs.windows(2).all(|w| w[0] <= w[1]));
    }

    #[test]
    fn max_float_is_the_largest_next_float() {
        assert_eq!(MAX_FLOAT, ((1u32 << 24) - 1) as f32 * (1.0 / (1u32 << 24) as f32));
    }
}
