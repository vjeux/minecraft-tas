//! A scene's triangles: block models placed in their cells (moving blocks at their offsets),
//! faces against opaque neighbors culled, corners darkened like the game's smooth lighting
//! (ambient occlusion), faces shaded by direction, translucent faces kept apart for sorting.

use crate::atlas::Atlas;
use crate::design::Pos;
use crate::models::{self, Face, Layer, Model, Part};
use std::collections::{HashMap, HashSet};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    /// rgb = tint x shade x ambient occlusion, a = alpha (cutout faces are tested in the shader).
    pub color: [u8; 4],
}

/// One quad (two triangles), kept for back-to-front sorting.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub v: [Vertex; 4],
    pub center: [f32; 3],
}

/// Something to draw besides blocks: an entity (feet position in blocks, yaw in degrees).
#[derive(Clone, Debug)]
pub struct EntityDraw {
    pub kind: String,
    pub pos: [f32; 3],
    pub yaw: f32,
}

/// Everything a frame needs.
#[derive(Default)]
pub struct Mesh {
    /// Opaque and cutout triangles.
    pub solid: Vec<Vertex>,
    /// Translucent quads (sort them with [`Mesh::sorted_translucent`]).
    pub translucent: Vec<Quad>,
    /// Bounds of everything drawn (blocks), for framing.
    pub lo: [f32; 3],
    pub hi: [f32; 3],
}

/// A block to draw: cell, offset (blocks), state string.
pub struct Item<'a> {
    pub cell: Pos,
    pub offset: [f32; 3],
    pub state: &'a str,
}

fn rotate(p: [f32; 3], axis: u8, angle_deg: f32, o: [f32; 3]) -> [f32; 3] {
    let (s, c) = angle_deg.to_radians().sin_cos();
    let d = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
    let r = match axis {
        0 => [d[0], d[1] * c - d[2] * s, d[1] * s + d[2] * c],
        1 => [d[0] * c + d[2] * s, d[1], -d[0] * s + d[2] * c],
        _ => [d[0] * c - d[1] * s, d[0] * s + d[1] * c, d[2]],
    };
    [r[0] + o[0], r[1] + o[1], r[2] + o[2]]
}

/// Quarter turns about the block center: x maps up -> north -> down, y maps north -> east.
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

/// The 4 corners (top-left, bottom-left, bottom-right, top-right seen from outside) of face `i`
/// of a part, in pixels, and the game's automatic UV rect.
fn face_corners(from: [f32; 3], to: [f32; 3], i: usize) -> ([[f32; 3]; 4], [f32; 4]) {
    let [x0, y0, z0] = from;
    let [x1, y1, z1] = to;
    match i {
        models::DOWN => ([[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]], [x0, 16.0 - z1, x1, 16.0 - z0]),
        models::UP => ([[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]], [x0, z0, x1, z1]),
        models::NORTH => ([[x1, y1, z0], [x1, y0, z0], [x0, y0, z0], [x0, y1, z0]], [16.0 - x1, 16.0 - y1, 16.0 - x0, 16.0 - y0]),
        models::SOUTH => ([[x0, y1, z1], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1]], [x0, 16.0 - y1, x1, 16.0 - y0]),
        models::WEST => ([[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]], [z0, 16.0 - y1, z1, 16.0 - y0]),
        _ => ([[x1, y1, z1], [x1, y0, z1], [x1, y0, z0], [x1, y1, z0]], [16.0 - z1, 16.0 - y1, 16.0 - z0, 16.0 - y0]),
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// The game's directional shade for a face normal (up 1.0, down 0.5, z 0.8, x 0.6).
pub fn shade(n: [f32; 3]) -> f32 {
    let y = if n[1] > 0.0 { n[1] * n[1] } else { n[1] * n[1] * 0.5 };
    n[0] * n[0] * 0.6 + n[2] * n[2] * 0.8 + y
}

/// AO brightness by the number of opaque neighbors around a corner (side, side, corner).
const AO: [f32; 4] = [1.0, 0.8, 0.65, 0.5];

pub struct Builder<'a> {
    pub atlas: &'a Atlas,
    /// Cells holding an opaque full block (not moving).
    occ: HashSet<Pos>,
    cache: HashMap<String, Option<Model>>,
    pub mesh: Mesh,
}

impl<'a> Builder<'a> {
    pub fn new(atlas: &'a Atlas) -> Builder<'a> {
        Builder { atlas, occ: HashSet::new(), cache: HashMap::new(), mesh: Mesh { lo: [f32::MAX; 3], hi: [f32::MIN; 3], ..Default::default() } }
    }

    fn model(&mut self, state: &str) -> Option<Model> {
        if let Some(m) = self.cache.get(state) {
            return m.clone();
        }
        let m = models::model_for(state, self.atlas);
        self.cache.insert(state.to_string(), m.clone());
        m
    }

    /// Add blocks (call once per frame with every block).
    pub fn blocks(&mut self, items: &[Item]) {
        self.occ.clear();
        for it in items {
            if it.offset == [0.0; 3]
                && let Some(m) = self.model(it.state)
                && m.full
                && m.parts.iter().all(|p| p.layer == Layer::Opaque)
            {
                self.occ.insert(it.cell);
            }
        }
        for it in items {
            if let Some(m) = self.model(it.state) {
                let base = [it.cell[0] as f32 + it.offset[0], it.cell[1] as f32 + it.offset[1], it.cell[2] as f32 + it.offset[2]];
                let still = it.offset == [0.0; 3];
                self.add_model(&m, base, if still { Some(it.cell) } else { None }, None);
            }
        }
    }

    /// An entity model at a feet position, turned by yaw (degrees, the game's: 0 = south).
    pub fn entity(&mut self, e: &EntityDraw) {
        if let Some(m) = models::entity_model(&e.kind) {
            self.add_model(&m, [e.pos[0] - 0.5, e.pos[1], e.pos[2] - 0.5], None, Some(e.yaw));
        }
    }

    /// The orange marker (stained glass) or an outline box (input) at a cell.
    pub fn marker(&mut self, cell: Pos, glass: bool, color: [u8; 3]) {
        let base = [cell[0] as f32, cell[1] as f32, cell[2] as f32];
        if glass {
            let m = models::model_for("minecraft:orange_stained_glass", self.atlas).unwrap();
            self.add_model(&m, base, None, None);
            return;
        }
        let t = 0.6;
        let (a, b) = (-0.3, 16.3);
        let mut parts = Vec::new();
        for (from, to) in [
            ([a, a, a], [b, a + t, a + t]),
            ([a, b - t, a], [b, b, a + t]),
            ([a, a, b - t], [b, a + t, b]),
            ([a, b - t, b - t], [b, b, b]),
            ([a, a, a], [a + t, b, a + t]),
            ([b - t, a, a], [b, b, a + t]),
            ([a, a, b - t], [a + t, b, b]),
            ([b - t, a, b - t], [b, b, b]),
            ([a, a, a], [a + t, a + t, b]),
            ([b - t, a, a], [b, a + t, b]),
            ([a, b - t, a], [a + t, b, b]),
            ([b - t, b - t, a], [b, b, b]),
        ] {
            parts.push(Part::new(from, to).all("white_concrete").tint(color).emissive(true).nocull());
        }
        let m = Model { parts, ..Default::default() };
        self.add_model(&m, base, None, None);
    }

    fn add_model(&mut self, m: &Model, base: [f32; 3], cell: Option<Pos>, yaw: Option<f32>) {
        for p in &m.parts {
            for (i, f) in p.faces.iter().enumerate() {
                if let Some(f) = f {
                    self.add_face(m, p, i, f, base, cell, yaw);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn add_face(&mut self, m: &Model, p: &Part, i: usize, f: &Face, base: [f32; 3], cell: Option<Pos>, yaw: Option<f32>) {
        let (corners, auto) = face_corners(p.from, p.to, i);
        let mut pts = corners.map(|c| {
            let c = match p.rot {
                Some(r) => rotate(c, r.axis, r.angle, r.origin),
                None => c,
            };
            let c = [c[0] / 16.0, c[1] / 16.0, c[2] / 16.0];
            let c = quarter(c, m.xrot, m.yrot);
            match yaw {
                // entities: their model is built around x = z = 0.5; the game's yaw turns clockwise
                Some(y) => rotate(c, 1, -y, [0.5, 0.0, 0.5]),
                None => c,
            }
        });
        let n = norm(cross(sub(pts[1], pts[0]), sub(pts[2], pts[0])));
        // which block side the face lies on, if any (for culling and AO)
        let side = cell.and_then(|_| {
            let axis = (0..3).find(|&k| n[k].abs() > 0.999)?;
            let v = pts[0][axis];
            if !pts.iter().all(|q| (q[axis] - v).abs() < 1e-4) {
                return None;
            }
            let pos_side = n[axis] > 0.0;
            if (pos_side && (v - 1.0).abs() < 1e-4) || (!pos_side && v.abs() < 1e-4) {
                Some((axis, if pos_side { 1 } else { -1 }))
            } else {
                None
            }
        });
        if let (Some(c), Some((axis, s))) = (cell, side)
            && f.cull
        {
            let mut q = c;
            q[axis] += s;
            if self.occ.contains(&q) {
                return;
            }
        }
        let sh = if f.emissive || !p.shade { 1.0 } else { shade(n) };
        // ambient occlusion on block-side faces of still blocks
        let ao: [f32; 4] = match (cell, side) {
            (Some(c), Some((axis, s))) if !f.emissive && m.full => {
                let mut out = [1.0; 4];
                for (k, q) in pts.iter().enumerate() {
                    let mut front = c;
                    front[axis] += s;
                    let (a1, a2) = ((axis + 1) % 3, (axis + 2) % 3);
                    let d1 = if q[a1] > 0.5 { 1 } else { -1 };
                    let d2 = if q[a2] > 0.5 { 1 } else { -1 };
                    let mut s1 = front;
                    s1[a1] += d1;
                    let mut s2 = front;
                    s2[a2] += d2;
                    let mut cc = front;
                    cc[a1] += d1;
                    cc[a2] += d2;
                    let (o1, o2, o3) = (self.occ.contains(&s1), self.occ.contains(&s2), self.occ.contains(&cc));
                    let level = if o1 && o2 { 3 } else { o1 as usize + o2 as usize + o3 as usize };
                    out[k] = AO[level];
                }
                out
            }
            _ => [1.0; 4],
        };
        let r = f.uv.unwrap_or(auto);
        let tile = self.atlas.tile(f.tile);
        let [u0, v0, u1, v1] = self.atlas.uv(tile, r);
        // a hair inside the rect: nearest sampling never reads the neighbor tile
        let e = 0.01 / crate::atlas::SIZE as f32;
        let (u0, u1) = if u0 < u1 { (u0 + e, u1 - e) } else { (u0 - e, u1 + e) };
        let (v0, v1) = if v0 < v1 { (v0 + e, v1 - e) } else { (v0 - e, v1 + e) };
        let base_uv = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
        let uv: [[f32; 2]; 4] = std::array::from_fn(|k| base_uv[(k + f.rot as usize) % 4]);
        let alpha = 255u8;
        for q in pts.iter_mut() {
            *q = [q[0] + base[0], q[1] + base[1], q[2] + base[2]];
            for k in 0..3 {
                self.mesh.lo[k] = self.mesh.lo[k].min(q[k]);
                self.mesh.hi[k] = self.mesh.hi[k].max(q[k]);
            }
        }
        let v: [Vertex; 4] = std::array::from_fn(|k| {
            let l = sh * ao[k];
            let c = |t: u8| (t as f32 * l).round().clamp(0.0, 255.0) as u8;
            Vertex { pos: pts[k], uv: uv[k], color: [c(f.tint[0]), c(f.tint[1]), c(f.tint[2]), alpha] }
        });
        match p.layer {
            Layer::Translucent => {
                let center = [(pts[0][0] + pts[2][0]) / 2.0, (pts[0][1] + pts[2][1]) / 2.0, (pts[0][2] + pts[2][2]) / 2.0];
                self.mesh.translucent.push(Quad { v, center });
            }
            _ => {
                // flip the diagonal where AO would make an ugly seam (the game's rule)
                if ao[0] + ao[2] < ao[1] + ao[3] {
                    self.mesh.solid.extend_from_slice(&[v[1], v[2], v[3], v[1], v[3], v[0]]);
                } else {
                    self.mesh.solid.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
                }
            }
        }
    }
}

impl Mesh {
    /// The translucent quads as triangles, farthest from `eye` first.
    pub fn sorted_translucent(&self, eye: [f32; 3]) -> Vec<Vertex> {
        let mut order: Vec<(f32, usize)> = self
            .translucent
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let d = sub(q.center, eye);
                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2], i)
            })
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0));
        let mut out = Vec::with_capacity(order.len() * 6);
        for (_, i) in order {
            let v = self.translucent[i].v;
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
        out
    }
}

/// The ground: a grass plane under the build (top face at `y`), `r` blocks around its box.
pub fn ground(atlas: &Atlas, lo: [f32; 3], hi: [f32; 3], y: f32, r: f32) -> Vec<Vertex> {
    let tile = atlas.tile("grass_top");
    let [u0, v0, u1, v1] = atlas.uv(tile, [0.0, 0.0, 16.0, 16.0]);
    let e = 0.01 / crate::atlas::SIZE as f32;
    let (u0, v0, u1, v1) = (u0 + e, v0 + e, u1 - e, v1 - e);
    let x0 = (lo[0] - r).floor() as i32;
    let x1 = (hi[0] + r).ceil() as i32;
    let z0 = (lo[2] - r).floor() as i32;
    let z1 = (hi[2] + r).ceil() as i32;
    let mut out = Vec::new();
    let c = [255, 255, 255, 255];
    for x in x0..x1 {
        for z in z0..z1 {
            let (xf, zf) = (x as f32, z as f32);
            let v = [
                Vertex { pos: [xf, y, zf], uv: [u0, v0], color: c },
                Vertex { pos: [xf, y, zf + 1.0], uv: [u0, v1], color: c },
                Vertex { pos: [xf + 1.0, y, zf + 1.0], uv: [u1, v1], color: c },
                Vertex { pos: [xf + 1.0, y, zf], uv: [u1, v0], color: c },
            ];
            out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
        }
    }
    out
}
