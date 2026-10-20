//! Primed TNT, which the viewer draws itself. redsim records when and where TNT primes
//! (`World::primed`) but does not simulate the entity: its motion draws from `level.random`, and
//! the judge removes primed TNT before it explodes. Here it moves like the game's `PrimedTnt.tick`
//! (gravity, the game's collision, air drag, ground friction, slime bounce) and is pushed by moving
//! blocks like `PistonMovingBlockEntity.moveCollidedEntities` (a moving slime block launches it at
//! one block per tick: how a TNT duper throws its TNT out). It vanishes at fuse 0 without
//! exploding.

use redsim::block::Simple;
use redsim::entity::collide::{block_collisions, collide_with_shapes};
use redsim::entity::{Aabb, Vec3};
use redsim::pos::{BlockPos, Dir};
use redsim::{Block, World};

/// `PrimedTnt`'s fuse when primed by a block (`TntBlock.prime`).
pub const FUSE: i32 = 80;
/// `EntityTypes.TNT`: 0.98 x 0.98.
const HALF: f64 = 0.49;
const HEIGHT: f64 = 0.98;

#[derive(Clone, Debug, PartialEq)]
pub struct PrimedTnt {
    pub pos: [f64; 3],
    /// Where it was at the start of the last tick (drawn between the two).
    pub prev: [f64; 3],
    pub vel: [f64; 3],
    pub fuse: i32,
    /// `Entity.pistonDeltas` of the current tick (`limitPistonMovement`).
    piston: [f64; 3],
}

impl PrimedTnt {
    /// `TntBlock.prime` at `p`: the entity at the cell's bottom centre. Its sideways direction is
    /// random in the game (`level.random`); a hash of the cell and the time stands in.
    pub fn new(p: BlockPos, time: i64) -> PrimedTnt {
        let h = (p.x as i64).wrapping_mul(73_856_093) ^ (p.y as i64).wrapping_mul(19_349_663) ^ (p.z as i64).wrapping_mul(83_492_791) ^ time.wrapping_mul(2_654_435_761);
        let rot = h.rem_euclid(3600) as f64 / 3600.0 * std::f64::consts::TAU;
        let pos = [p.x as f64 + 0.5, p.y as f64, p.z as f64 + 0.5];
        PrimedTnt { pos, prev: pos, vel: [-rot.sin() * 0.02, 0.2, -rot.cos() * 0.02], fuse: FUSE, piston: [0.0; 3] }
    }

    fn bb(&self) -> Aabb {
        Aabb::new(self.pos[0] - HALF, self.pos[1], self.pos[2] - HALF, self.pos[0] + HALF, self.pos[1] + HEIGHT, self.pos[2] + HALF)
    }
}

/// The block shapes around `bb`, and the ground the viewer draws (its top at `floor`).
fn shapes(w: &World, bb: Aabb, floor: f64) -> Vec<Vec<Aabb>> {
    let mut v: Vec<Vec<Aabb>> = block_collisions(w, bb).into_iter().map(|(_, s)| s).collect();
    v.push(vec![Aabb::new(bb.min_x - 1.0, floor - 1.0, bb.min_z - 1.0, bb.max_x + 1.0, floor, bb.max_z + 1.0)]);
    v
}

/// `Entity.move` for a primed TNT: the game's collision, a horizontal collision stops that axis,
/// a vertical one calls `updateEntityMovementAfterFallOn` (a slime block bounces a non-living
/// entity back at 0.8 of its speed, any other block stops it). Returns whether it landed.
fn move_by(w: &World, t: &mut PrimedTnt, m: Vec3, floor: f64) -> bool {
    let bb = t.bb();
    let d = collide_with_shapes(m, bb, &shapes(w, bb.expand_towards(m.x, m.y, m.z), floor));
    t.pos = [t.pos[0] + d.x, t.pos[1] + d.y, t.pos[2] + d.z];
    if m.x != d.x {
        t.vel[0] = 0.0;
    }
    if m.z != d.z {
        t.vel[2] = 0.0;
    }
    if m.y != d.y {
        // getOnPosLegacy: the block 0.2 under its feet
        let below = BlockPos::new(t.pos[0].floor() as i32, (t.pos[1] - 0.2).floor() as i32, t.pos[2].floor() as i32);
        if matches!(w.get(below), Block::Simple(Simple::SlimeBlock)) && t.vel[1] < 0.0 {
            t.vel[1] = -t.vel[1] * 0.8;
        } else {
            t.vel[1] = 0.0;
        }
    }
    m.y != d.y && m.y < 0.0
}

/// `PrimedTnt.tick` (the entity phase): gravity, move, air drag, ground friction, fuse.
pub fn tick(w: &World, t: &mut PrimedTnt, floor: f64) {
    t.prev = t.pos;
    t.piston = [0.0; 3];
    t.vel[1] -= 0.04;
    let landed = move_by(w, t, Vec3::new(t.vel[0], t.vel[1], t.vel[2]), floor);
    for v in t.vel.iter_mut() {
        *v *= 0.98;
    }
    if landed {
        t.vel[0] *= 0.7;
        t.vel[1] *= -0.5;
        t.vel[2] *= 0.7;
    }
    t.fuse -= 1;
}

/// `PistonMath.getMovementArea`: the slab `delta` thick in front of `b`'s face toward `dir`.
fn movement_area(b: Aabb, dir: Dir, delta: f64) -> Aabb {
    match dir {
        Dir::West => Aabb::new(b.min_x - delta, b.min_y, b.min_z, b.min_x, b.max_y, b.max_z),
        Dir::East => Aabb::new(b.max_x, b.min_y, b.min_z, b.max_x + delta, b.max_y, b.max_z),
        Dir::Down => Aabb::new(b.min_x, b.min_y - delta, b.min_z, b.max_x, b.min_y, b.max_z),
        Dir::Up => Aabb::new(b.min_x, b.max_y, b.min_z, b.max_x, b.max_y + delta, b.max_z),
        Dir::North => Aabb::new(b.min_x, b.min_y, b.min_z - delta, b.max_x, b.max_y, b.min_z),
        Dir::South => Aabb::new(b.min_x, b.min_y, b.max_z, b.max_x, b.max_y, b.max_z + delta),
    }
}

/// `PistonMovingBlockEntity.getMovement`: how far `e` must move along `dir` to leave `area`.
fn movement_out(area: Aabb, dir: Dir, e: Aabb) -> f64 {
    match dir {
        Dir::East => area.max_x - e.min_x,
        Dir::West => e.max_x - area.min_x,
        Dir::Up => area.max_y - e.min_y,
        Dir::Down => e.max_y - area.min_y,
        Dir::South => area.max_z - e.min_z,
        Dir::North => e.max_z - area.min_z,
    }
}

/// `PistonMovingBlockEntity.moveCollidedEntities` for every block that moved in the tick just run
/// (the block-entity phase, after the entities): the moving block's collision boxes at its old
/// progress push each TNT in front of them out of the way (`Entity.move(PISTON)`, at most 0.51
/// per axis and tick), and a moving slime block sets its speed along the move to one block per
/// tick.
pub fn push(w: &World, tnts: &mut [PrimedTnt], floor: f64) {
    if tnts.is_empty() {
        return;
    }
    for (p, be) in w.piston.all() {
        if be.progress == be.progress_o || (!be.extending && be.source) {
            continue;
        }
        let delta = (be.progress as f64 - be.progress_o as f64) / 2.0;
        let dir = if be.extending { be.direction } else { be.direction.opposite() };
        let off = if be.extending { be.progress_o as f64 / 2.0 - 1.0 } else { 1.0 - be.progress_o as f64 / 2.0 };
        let st = be.direction.step();
        let (ox, oy, oz) = (p.x as f64 + off * st[0] as f64, p.y as f64 + off * st[1] as f64, p.z as f64 + off * st[2] as f64);
        let boxes: Vec<Aabb> = redsim::behavior::collision_boxes(&be.moved)
            .iter()
            .map(|c| Aabb::new(ox + c[0] as f64 / 16.0, oy + c[2] as f64 / 16.0, oz + c[4] as f64 / 16.0, ox + c[1] as f64 / 16.0, oy + c[3] as f64 / 16.0, oz + c[5] as f64 / 16.0))
            .collect();
        let Some(first) = boxes.first() else { continue };
        let all = boxes.iter().fold(*first, |a, b| Aabb::new(a.min_x.min(b.min_x), a.min_y.min(b.min_y), a.min_z.min(b.min_z), a.max_x.max(b.max_x), a.max_y.max(b.max_y), a.max_z.max(b.max_z)));
        let sweep = movement_area(all, dir, delta);
        let query = Aabb::new(all.min_x.min(sweep.min_x), all.min_y.min(sweep.min_y), all.min_z.min(sweep.min_z), all.max_x.max(sweep.max_x), all.max_y.max(sweep.max_y), all.max_z.max(sweep.max_z));
        let slime = matches!(be.moved, Block::Simple(Simple::SlimeBlock));
        let ds = dir.step();
        for t in tnts.iter_mut() {
            if !query.intersects(t.bb()) {
                continue;
            }
            if slime {
                for k in 0..3 {
                    if ds[k] != 0 {
                        t.vel[k] = ds[k] as f64;
                    }
                }
            }
            let mut most = 0.0f64;
            for b in &boxes {
                let area = movement_area(*b, dir, delta);
                if area.intersects(t.bb()) {
                    most = most.max(movement_out(area, dir, t.bb()));
                    if most >= delta {
                        break;
                    }
                }
            }
            if most > 0.0 {
                let amount = most.min(delta) + 0.01;
                // limitPistonMovement: at most 0.51 per axis per tick in total
                let k = (0..3).find(|&k| ds[k] != 0).unwrap_or(1);
                let total = (t.piston[k] + amount * ds[k] as f64).clamp(-0.51, 0.51);
                let a = total - t.piston[k];
                t.piston[k] = total;
                if a.abs() > 1.0e-5 {
                    let mut m = [0.0; 3];
                    m[k] = a;
                    move_by(w, t, Vec3::new(m[0], m[1], m[2]), floor);
                }
            }
        }
    }
}

/// `TntRenderer`: the swell over the last 10 ticks of the fuse (`fuse` = the fuse left as drawn:
/// `getFuse() - partialTicks + 1`), and whether it flashes white (every 5 ticks).
pub fn look(fuse: f32) -> (f32, bool) {
    let swell = if fuse < 10.0 {
        let g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
        let g = g * g;
        g * g * 0.3
    } else {
        0.0
    };
    (1.0 + swell, fuse >= 0.0 && (fuse / 5.0) as i32 % 2 == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primed_tnt_pops_up_and_lands() {
        let w = World::new(BlockPos::new(-4, -4, -4), [9, 9, 9]);
        let mut t = PrimedTnt::new(BlockPos::new(0, 0, 0), 0);
        let mut top = 0.0f64;
        for _ in 0..40 {
            tick(&w, &mut t, 0.0);
            top = top.max(t.pos[1]);
        }
        assert!(top > 0.3 && top < 0.7, "pops up about half a block: {top}");
        assert!(t.pos[1].abs() < 1.0e-6, "back on the ground: {:?}", t.pos);
        assert_eq!(t.fuse, FUSE - 40);
        assert_eq!(look(79.0), (1.0, false));
        assert!(look(4.0).1 && look(2.0).0 > 1.1);
    }
}
