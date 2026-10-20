//! The collision (= support) shapes of the piston blocks, in 1/16 block units, and the face
//! tests of `isFaceSturdy` (`SupportType.FULL / CENTER / RIGID`) on them: `getFaceShape` takes
//! the voxel layer just inside the face (`findIndex(axis, 0.9999999 | 1e-7)`), extrudes it
//! through the cell, and compares it with the block (FULL: identical), the center column
//! (`Block.column(2, 0, 10)`) or the rigid ring (block minus `box(2, 0, 2, 14, 16, 14)`).
//! Moving pistons take their shape from the block entity (`PistonMovingBlockEntity
//! .getCollisionShape`): the moved block offset by the progress, a retracting base keeping its
//! extended base box.

use crate::behavior::Support;
use crate::block::Block;
use crate::pos::{BlockPos, Dir};
use crate::world::World;

/// An axis-aligned box in 1/16 units: [x0, x1) x [y0, y1) x [z0, z1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct B16(pub [i32; 6]);

impl B16 {
    pub fn full() -> B16 {
        B16([0, 16, 0, 16, 0, 16])
    }
    pub fn shifted(self, d: [i32; 3]) -> B16 {
        let b = self.0;
        B16([b[0] + d[0], b[1] + d[0], b[2] + d[1], b[3] + d[1], b[4] + d[2], b[5] + d[2]])
    }
}

fn axis_index(d: Dir) -> usize {
    match d {
        Dir::West | Dir::East => 0,
        Dir::Down | Dir::Up => 1,
        Dir::North | Dir::South => 2,
    }
}

fn positive(d: Dir) -> bool {
    matches!(d, Dir::East | Dir::Up | Dir::South)
}

/// A box spanning [lo, hi) along `facing`'s axis (in the frame where the face is at 0 for a
/// negative facing), full on the other two axes.
fn along(facing: Dir, lo: i32, hi: i32, others: (i32, i32)) -> B16 {
    let a = axis_index(facing);
    let mut b = [others.0, others.1, others.0, others.1, others.0, others.1];
    // positive facings mirror the range: [16 - hi, 16 - lo)
    let (l, h) = if positive(facing) { (16 - hi, 16 - lo) } else { (lo, hi) };
    b[2 * a] = l;
    b[2 * a + 1] = h;
    B16(b)
}

/// The extended base: the 12 px box behind the head.
pub fn base_extended(facing: Dir) -> Vec<B16> {
    vec![along(facing, 4, 16, (0, 16))]
}

/// `PistonHeadBlock` shapes: the 4 px plate at the front, the 4x4 arm behind it (20 px long, or
/// 16 px when short).
pub fn head(facing: Dir, short: bool) -> Vec<B16> {
    vec![along(facing, 0, 4, (0, 16)), along(facing, 4, if short { 16 } else { 20 }, (6, 10))]
}

/// A moved block's own collision shape (the empty collision context).
pub fn block_shape(b: Block) -> Vec<B16> {
    match b {
        Block::Piston { extended: false, .. } => vec![B16::full()],
        Block::Piston { facing, extended: true, .. } => base_extended(facing),
        Block::PistonHead { facing, short, .. } => head(facing, short),
        // every other block: its collision shape (redmodel's table: honey, soul sand, mud,
        // trapdoors, slabs, fence gates, scaffolding...; empty for air)
        _ => crate::behavior::collision_boxes(&b).into_iter().map(B16).collect(),
    }
}

/// `PistonMovingBlockEntity.getCollisionShape` (no entity being moved: no no-clip direction).
pub fn moving(w: &World, p: BlockPos) -> Vec<B16> {
    moving_noclip(w, p, None)
}

/// `PistonMovingBlockEntity.getCollisionShape` while an entity is moved by a piston in direction
/// `noclip` (`NOCLIP`, set by `moveEntityByPiston`): a block still moving that way keeps only
/// the extended base box of a retracting base (entity pushing, clk-misc).
pub fn moving_noclip(w: &World, p: BlockPos, noclip: Option<Dir>) -> Vec<B16> {
    let Some(be) = w.piston.be(p) else { return vec![] };
    let mut out = Vec::new();
    if !be.extending && be.source {
        if let Block::Piston { facing, .. } = be.moved {
            out.extend(base_extended(facing));
        }
    }
    if be.progress < 2 && noclip == Some(be.push_direction()) {
        return out;
    }
    let prog16 = be.progress as i32 * 8;
    let moved = if be.source {
        // PISTON_HEAD facing `direction`, short = extending != (1 - progress < 0.25)
        let short = be.extending != (16 - prog16 < 4);
        head(be.direction, short)
    } else {
        block_shape(be.moved)
    };
    let ext = if be.extending { prog16 - 16 } else { 16 - prog16 };
    let st = be.direction.step();
    out.extend(moved.into_iter().map(|b| b.shifted([st[0] * ext, st[1] * ext, st[2] * ext])));
    out
}

/// The collision shape of a piston block.
pub fn of(w: &World, s: Block, p: BlockPos) -> Vec<B16> {
    match s {
        Block::Piston { extended: false, .. } => vec![B16::full()],
        Block::Piston { facing, extended: true, .. } => base_extended(facing),
        Block::PistonHead { facing, short, .. } => head(facing, short),
        Block::MovingPiston { .. } => moving(w, p),
        _ => vec![],
    }
}

/// `isFaceSturdy(face, support)` of a shape.
pub fn face_supports(boxes: &[B16], face: Dir, support: Support) -> bool {
    let a = axis_index(face);
    // the layer just inside the face
    let covers = |b: &B16| {
        let (lo, hi) = (b.0[2 * a], b.0[2 * a + 1]);
        if positive(face) { lo < 16 && hi >= 16 } else { lo <= 0 && hi >= 1 }
    };
    let (u, v) = match a {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };
    // the extruded slice, rasterized on [-24, 40)^2
    const LO: i32 = -24;
    const N: usize = 64;
    let mut grid = [[false; N]; N];
    let mut any = false;
    for b in boxes.iter().filter(|b| covers(b)) {
        any = true;
        for i in b.0[2 * u].max(LO)..b.0[2 * u + 1].min(LO + N as i32) {
            for j in b.0[2 * v].max(LO)..b.0[2 * v + 1].min(LO + N as i32) {
                grid[(i - LO) as usize][(j - LO) as usize] = true;
            }
        }
    }
    if !any {
        return false;
    }
    let at = |i: i32, j: i32| grid[(i - LO) as usize][(j - LO) as usize];
    let need = |i: i32, j: i32| -> bool {
        match support {
            Support::Full => (0..16).contains(&i) && (0..16).contains(&j),
            Support::Center => {
                // Block.column(2, 0, 10): x, z in [7, 9), y in [0, 10)
                let (x, y, z) = match a {
                    0 => (None, Some(i), Some(j)),
                    1 => (Some(i), None, Some(j)),
                    _ => (Some(i), Some(j), None),
                };
                x.is_none_or(|x| (7..9).contains(&x)) && y.is_none_or(|y| (0..10).contains(&y)) && z.is_none_or(|z| (7..9).contains(&z))
            }
            Support::Rigid => {
                if a == 1 {
                    (0..16).contains(&i) && (0..16).contains(&j) && !((2..14).contains(&i) && (2..14).contains(&j))
                } else {
                    (0..16).contains(&i) && (0..16).contains(&j)
                }
            }
        }
    };
    for i in LO..LO + N as i32 {
        for j in LO..LO + N as i32 {
            let n = need(i, j);
            let c = at(i, j);
            if n && !c {
                return false;
            }
            // FULL: the slice must also not stick out of the block
            if support == Support::Full && c && !n {
                return false;
            }
        }
    }
    true
}
