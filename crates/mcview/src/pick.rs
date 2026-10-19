//! Clicking a block: the ray from the camera through the pointer walks the cells it crosses
//! (Amanatides-Woo) and stops at the first block whose model boxes it hits.

use crate::atlas::Atlas;
use crate::design::Pos;
use crate::models;
use crate::session::RBlock;
use std::collections::HashMap;

fn quarter(p: [f32; 3], xrot: u8, yrot: u8) -> [f32; 3] {
    let mut q = [p[0] - 0.5, p[1] - 0.5, p[2] - 0.5];
    for _ in 0..xrot % 4 {
        q = [q[0], q[2], -q[1]];
    }
    for _ in 0..yrot % 4 {
        q = [-q[2], q[1], q[0]];
    }
    [q[0] + 0.5, q[1] + 0.5, q[2] + 0.5]
}

/// Distance along the ray to a box, if it hits.
fn hit_box(o: [f32; 3], d: [f32; 3], lo: [f32; 3], hi: [f32; 3]) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for k in 0..3 {
        if d[k].abs() < 1e-9 {
            if o[k] < lo[k] || o[k] > hi[k] {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo[k] - o[k]) / d[k], (hi[k] - o[k]) / d[k]);
        let (a, b) = if a < b { (a, b) } else { (b, a) };
        t0 = t0.max(a);
        t1 = t1.min(b);
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

/// The boxes (world coordinates) of a block's model at a cell, a little padded so thin blocks
/// (dust, levers) stay easy to click.
pub fn boxes(atlas: &Atlas, state: &str, cell: Pos) -> Vec<([f32; 3], [f32; 3])> {
    let Some(m) = models::model_for(state, atlas) else { return Vec::new() };
    let mut out = Vec::new();
    for p in &m.parts {
        let a = quarter([p.from[0] / 16.0, p.from[1] / 16.0, p.from[2] / 16.0], m.xrot, m.yrot);
        let b = quarter([p.to[0] / 16.0, p.to[1] / 16.0, p.to[2] / 16.0], m.xrot, m.yrot);
        let mut lo = [0.0; 3];
        let mut hi = [0.0; 3];
        for k in 0..3 {
            lo[k] = a[k].min(b[k]).max(0.0) - 0.06 + cell[k] as f32;
            hi[k] = a[k].max(b[k]).min(1.0) + 0.06 + cell[k] as f32;
        }
        out.push((lo, hi));
    }
    out
}

/// The first still block the ray hits within `max` blocks.
pub fn pick(atlas: &Atlas, blocks: &[RBlock], origin: [f32; 3], dir: [f32; 3], max: f32) -> Option<Pos> {
    let map: HashMap<Pos, &str> = blocks.iter().filter(|b| b.offset == [0.0; 3]).map(|b| (b.cell, b.state.as_str())).collect();
    let mut cell = [origin[0].floor() as i32, origin[1].floor() as i32, origin[2].floor() as i32];
    let step: [i32; 3] = std::array::from_fn(|k| if dir[k] > 0.0 { 1 } else { -1 });
    let mut tmax = [0f32; 3];
    let mut tdelta = [0f32; 3];
    for k in 0..3 {
        if dir[k].abs() < 1e-9 {
            tmax[k] = f32::MAX;
            tdelta[k] = f32::MAX;
        } else {
            let next = if dir[k] > 0.0 { cell[k] as f32 + 1.0 } else { cell[k] as f32 };
            tmax[k] = (next - origin[k]) / dir[k];
            tdelta[k] = 1.0 / dir[k].abs();
        }
    }
    let mut t = 0.0;
    while t <= max {
        // the cell and its neighbors: models reach a little beyond their cell (wall torches,
        // piston arms), so test the 3x3x3 around the walked cell, nearest hit first
        let mut best: Option<(f32, Pos)> = None;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let c = [cell[0] + dx, cell[1] + dy, cell[2] + dz];
                    if let Some(st) = map.get(&c) {
                        for (lo, hi) in boxes(atlas, st, c) {
                            if let Some(h) = hit_box(origin, dir, lo, hi)
                                && best.is_none_or(|(b, _)| h < b)
                            {
                                best = Some((h, c));
                            }
                        }
                    }
                }
            }
        }
        if let Some((h, c)) = best
            && h <= t + 1.8
        {
            return Some(c);
        }
        let k = if tmax[0] < tmax[1] && tmax[0] < tmax[2] {
            0
        } else if tmax[1] < tmax[2] {
            1
        } else {
            2
        };
        cell[k] += step[k];
        t = tmax[k];
        tmax[k] += tdelta[k];
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_nearest_block() {
        let atlas = Atlas::build();
        let blocks = vec![
            RBlock { cell: [0, 0, 0], offset: [0.0; 3], state: "minecraft:stone".into() },
            RBlock { cell: [0, 0, 3], offset: [0.0; 3], state: "minecraft:lever[face=floor,facing=north,powered=false]".into() },
        ];
        let o = [0.5, 0.5, 10.0];
        let d = [0.0, 0.0, -1.0];
        assert_eq!(pick(&atlas, &blocks, o, d, 50.0), Some([0, 0, 3]));
        assert_eq!(pick(&atlas, &blocks, [0.5, 0.9, 10.0], d, 50.0), Some([0, 0, 0]));
        assert_eq!(pick(&atlas, &blocks, [5.5, 0.5, 10.0], d, 50.0), None);
    }
}
