//! The game's own entity models (its client code: `BoatModel`, `MinecartModel`,
//! `ArmorStandModel`, read in the decompiled 26.3 client) with their renderers' transforms
//! (`AbstractBoatRenderer`, `AbstractMinecartRenderer`, `ArmorStandRenderer`,
//! `ItemEntityRenderer`, `FallingBlockRenderer`), drawn with the game's entity and item textures
//! (misode/mcmeta's entity and item atlases, packed below the block atlas in the game texture) and
//! the game's entity lighting (two directional lights, `light.glsl`).

use crate::atlas::GameAssets;
use crate::mesh::Vertex;

/// A box of a model part (`CubeListBuilder.texOffs(u, v).addBox(x, y, z, w, h, d)`).
struct Cube {
    tex: [f32; 2],
    from: [f32; 3],
    size: [f32; 3],
    mirror: bool,
}

/// A model part: its boxes, its pose (offset in pixels, rotations in radians, `PartPose`).
struct EPart {
    cubes: Vec<Cube>,
    offset: [f32; 3],
    rot: [f32; 3],
}

fn cube(u: f32, v: f32, from: [f32; 3], size: [f32; 3]) -> Cube {
    Cube { tex: [u, v], from, size, mirror: false }
}

fn part(cubes: Vec<Cube>, offset: [f32; 3], rot: [f32; 3]) -> EPart {
    EPart { cubes, offset, rot }
}

const PI: f32 = std::f32::consts::PI;

/// `BoatModel.addCommonParts` (+ the chest boat's chest), textures 128 x 64 (128 x 128 chest),
/// the paddles where `AbstractBoatModel.animatePaddle` holds them at rowing time 0 (an idle boat).
fn boat(chest: bool) -> Vec<EPart> {
    let lerp = |f: f32, a: f32, b: f32| a + f.clamp(0.0, 1.0) * (b - a);
    let paddle_x = lerp(((-0.0f32).sin() + 1.0) / 2.0, -PI / 3.0, -PI / 12.0);
    let paddle_y = lerp(((1.0f32).sin() + 1.0) / 2.0, -PI / 4.0, PI / 4.0);
    let mut v = vec![
        part(vec![cube(0.0, 0.0, [-14.0, -9.0, -3.0], [28.0, 16.0, 3.0])], [0.0, 3.0, 1.0], [PI / 2.0, 0.0, 0.0]),
        part(vec![cube(0.0, 19.0, [-13.0, -7.0, -1.0], [18.0, 6.0, 2.0])], [-15.0, 4.0, 4.0], [0.0, PI * 3.0 / 2.0, 0.0]),
        part(vec![cube(0.0, 27.0, [-8.0, -7.0, -1.0], [16.0, 6.0, 2.0])], [15.0, 4.0, 0.0], [0.0, PI / 2.0, 0.0]),
        part(vec![cube(0.0, 35.0, [-14.0, -7.0, -1.0], [28.0, 6.0, 2.0])], [0.0, 4.0, -9.0], [0.0, PI, 0.0]),
        part(vec![cube(0.0, 43.0, [-14.0, -7.0, -1.0], [28.0, 6.0, 2.0])], [0.0, 4.0, 9.0], [0.0, 0.0, 0.0]),
        part(vec![cube(62.0, 0.0, [-1.0, 0.0, -5.0], [2.0, 2.0, 18.0]), cube(62.0, 0.0, [-1.001, -3.0, 8.0], [1.0, 6.0, 7.0])], [3.0, -5.0, 9.0], [paddle_x, paddle_y, PI / 16.0]),
        part(vec![cube(62.0, 20.0, [-1.0, 0.0, -5.0], [2.0, 2.0, 18.0]), cube(62.0, 20.0, [0.001, -3.0, 8.0], [1.0, 6.0, 7.0])], [3.0, -5.0, -9.0], [paddle_x, PI - paddle_y, PI / 16.0]),
    ];
    if chest {
        v.push(part(vec![cube(0.0, 76.0, [0.0, 0.0, 0.0], [12.0, 8.0, 12.0])], [-2.0, -5.0, -6.0], [0.0, -PI / 2.0, 0.0]));
        v.push(part(vec![cube(0.0, 59.0, [0.0, 0.0, 0.0], [12.0, 4.0, 12.0])], [-2.0, -9.0, -6.0], [0.0, -PI / 2.0, 0.0]));
        v.push(part(vec![cube(0.0, 59.0, [0.0, 0.0, 0.0], [2.0, 4.0, 1.0])], [-1.0, -6.0, -1.0], [0.0, -PI / 2.0, 0.0]));
    }
    v
}

/// `MinecartModel.createBodyLayer`, texture 64 x 32.
fn minecart() -> Vec<EPart> {
    let side = |offset: [f32; 3], yrot: f32| part(vec![cube(0.0, 0.0, [-8.0, -9.0, -1.0], [16.0, 8.0, 2.0])], offset, [0.0, yrot, 0.0]);
    vec![
        part(vec![cube(0.0, 10.0, [-10.0, -8.0, -1.0], [20.0, 16.0, 2.0])], [0.0, 4.0, 0.0], [PI / 2.0, 0.0, 0.0]),
        side([-9.0, 4.0, 0.0], PI * 3.0 / 2.0),
        side([9.0, 4.0, 0.0], PI / 2.0),
        side([0.0, 4.0, -7.0], PI),
        side([0.0, 4.0, 7.0], 0.0),
    ]
}

/// `ArmorStandModel.createBodyLayer` in its default pose (`ArmorStand.DEFAULT_*_POSE`; arms hidden,
/// base plate shown and kept square to the world), texture 64 x 64.
fn armor_stand(yaw_deg: f32) -> Vec<EPart> {
    let d = PI / 180.0;
    let legs = |u: f32, v: f32, x: f32, pose: [f32; 3], mirror: bool| {
        let mut c = cube(u, v, [-1.0, 0.0, -1.0], [2.0, 11.0, 2.0]);
        c.mirror = mirror;
        part(vec![c], [x, 12.0, 0.0], [pose[0] * d, pose[1] * d, pose[2] * d])
    };
    vec![
        part(vec![cube(0.0, 0.0, [-1.0, -7.0, -1.0], [2.0, 7.0, 2.0])], [0.0, 1.0, 0.0], [0.0; 3]),
        part(vec![cube(0.0, 26.0, [-6.0, 0.0, -1.5], [12.0, 3.0, 3.0])], [0.0; 3], [0.0; 3]),
        legs(8.0, 0.0, -1.9, [1.0, 0.0, 1.0], false),
        legs(40.0, 16.0, 1.9, [-1.0, 0.0, -1.0], true),
        part(vec![cube(16.0, 0.0, [-3.0, 3.0, -1.0], [2.0, 7.0, 2.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(48.0, 16.0, [1.0, 3.0, -1.0], [2.0, 7.0, 2.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(0.0, 48.0, [-4.0, 10.0, -1.0], [8.0, 2.0, 2.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(0.0, 32.0, [-6.0, 11.0, -6.0], [12.0, 1.0, 12.0])], [0.0, 12.0, 0.0], [0.0, -yaw_deg * d, 0.0]),
    ]
}

/// A 4x4 affine transform, column vectors (`m * [x, y, z, 1]`).
#[derive(Clone, Copy)]
pub struct M([[f32; 4]; 4]);

impl M {
    pub fn id() -> M {
        M([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }
    pub fn mul(&self, o: &M) -> M {
        let mut r = [[0.0f32; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, x) in row.iter_mut().enumerate() {
                *x = (0..4).map(|k| self.0[i][k] * o.0[k][j]).sum();
            }
        }
        M(r)
    }
    pub fn translate(&self, t: [f32; 3]) -> M {
        let mut m = M::id();
        m.0[0][3] = t[0];
        m.0[1][3] = t[1];
        m.0[2][3] = t[2];
        self.mul(&m)
    }
    pub fn scale(&self, s: [f32; 3]) -> M {
        let mut m = M::id();
        m.0[0][0] = s[0];
        m.0[1][1] = s[1];
        m.0[2][2] = s[2];
        self.mul(&m)
    }
    /// A right-handed rotation about axis 0 / 1 / 2 by `a` radians.
    pub fn rot(&self, axis: usize, a: f32) -> M {
        let (s, c) = a.sin_cos();
        let mut m = M::id();
        let (i, j) = match axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        m.0[i][i] = c;
        m.0[i][j] = -s;
        m.0[j][i] = s;
        m.0[j][j] = c;
        self.mul(&m)
    }
    pub fn point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3])
    }
    pub fn dir(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [0, 1, 2].map(|i| m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2])
    }
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-9);
    [v[0] / l, v[1] / l, v[2] / l]
}

/// The game's entity light for a face normal (`light.glsl`: two diffuse lights, 0.6 power, 0.4
/// ambient).
pub fn entity_light(n: [f32; 3]) -> f32 {
    let l0 = normalize([0.2, 1.0, -0.7]);
    let l1 = normalize([-0.2, 1.0, 0.7]);
    let d = |l: [f32; 3]| (l[0] * n[0] + l[1] * n[1] + l[2] * n[2]).max(0.0);
    ((d(l0) + d(l1)) * 0.6 + 0.4).min(1.0)
}

/// The triangles of a model's parts (`ModelPart.Cube`'s polygons and UVs, mirror included) under
/// `m`, textured from `rect` (x, y, w, h in the game texture) of a `tex_size` texture.
fn emit(out: &mut Vec<Vertex>, parts: &[EPart], m: &M, tex_size: [f32; 2], rect: [u32; 4], tw: f32, th: f32) {
    for p in parts {
        let pm = m.translate([p.offset[0] / 16.0, p.offset[1] / 16.0, p.offset[2] / 16.0]).rot(2, p.rot[2]).rot(1, p.rot[1]).rot(0, p.rot[0]);
        for c in &p.cubes {
            let (mut x0, y0, z0) = (c.from[0], c.from[1], c.from[2]);
            let (mut x1, y1, z1) = (x0 + c.size[0], y0 + c.size[1], z0 + c.size[2]);
            if c.mirror {
                std::mem::swap(&mut x0, &mut x1);
            }
            let (w, h, d) = (c.size[0], c.size[1], c.size[2]);
            let t0 = [x0, y0, z0];
            let t1 = [x1, y0, z0];
            let t2 = [x1, y1, z0];
            let t3 = [x0, y1, z0];
            let l0 = [x0, y0, z1];
            let l1 = [x1, y0, z1];
            let l2 = [x1, y1, z1];
            let l3 = [x0, y1, z1];
            let (u, v) = (c.tex[0], c.tex[1]);
            let (u0, u1, u2, u22, u3, u4) = (u, u + d, u + d + w, u + d + w + w, u + d + w + d, u + d + w + d + w);
            let (v0, v1, v2) = (v, v + d, v + d + h);
            let polys: [([[f32; 3]; 4], [f32; 4]); 6] = [
                ([l1, l0, t0, t1], [u1, v0, u2, v1]),
                ([t2, t3, l3, l2], [u2, v1, u22, v0]),
                ([t0, l0, l3, t3], [u0, v1, u1, v2]),
                ([t1, t0, t3, t2], [u1, v1, u2, v2]),
                ([l1, t1, t2, l2], [u2, v1, u3, v2]),
                ([l0, l1, l2, l3], [u3, v1, u4, v2]),
            ];
            for (verts, [pu0, pv0, pu1, pv1]) in polys {
                let mut uv = [[pu1, pv0], [pu0, pv0], [pu0, pv1], [pu1, pv1]];
                let mut vs = verts;
                if c.mirror {
                    vs.reverse();
                    uv.reverse();
                }
                let world: [[f32; 3]; 4] = vs.map(|q| pm.point([q[0] / 16.0, q[1] / 16.0, q[2] / 16.0]));
                let e1 = [world[1][0] - world[0][0], world[1][1] - world[0][1], world[1][2] - world[0][2]];
                let e2 = [world[2][0] - world[0][0], world[2][1] - world[0][1], world[2][2] - world[0][2]];
                let n = normalize([e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]);
                if n.iter().all(|x| x.is_nan()) {
                    continue;
                }
                let l = (entity_light(n) * 255.0).round() as u8;
                let tex = |t: [f32; 2]| -> [f32; 2] {
                    let x = rect[0] as f32 + (t[0] / tex_size[0]).clamp(0.0, 1.0) * rect[2] as f32;
                    let y = rect[1] as f32 + (t[1] / tex_size[1]).clamp(0.0, 1.0) * rect[3] as f32;
                    [(x.clamp(rect[0] as f32 + 0.02, (rect[0] + rect[2]) as f32 - 0.02)) / tw, (y.clamp(rect[1] as f32 + 0.02, (rect[1] + rect[3]) as f32 - 0.02)) / th]
                };
                let vx: [Vertex; 4] = std::array::from_fn(|k| Vertex { pos: world[k], uv: tex(uv[k]), layer: crate::atlas::GAME_LAYER, color: [l, l, l, 255] });
                out.extend_from_slice(&[vx[0], vx[1], vx[2], vx[0], vx[2], vx[3]]);
            }
        }
    }
}

/// A minecart's place and rotation like `AbstractMinecartRenderer.oldRender`: on a rail, the
/// rail's line under it and the direction between the points 0.3 ahead and behind (pitch
/// `atan(dy) * 73`); off rails, its own yaw and pitch.
pub fn minecart_pose(w: &redsim::World, pos: [f64; 3], yaw: f32, pitch: f32) -> ([f64; 3], f32, f32) {
    use redsim::BlockPos;
    use redsim::rails;
    let floor = |v: f64| v.floor() as i32;
    let rail_at = |x: f64, y: f64, z: f64| -> Option<(i32, i32, i32, ([i32; 3], [i32; 3]))> {
        let (xt, mut yt, zt) = (floor(x), floor(y), floor(z));
        if rails::is_rail(&w.get(BlockPos::new(xt, yt - 1, zt))) {
            yt -= 1;
        }
        let s = w.get(BlockPos::new(xt, yt, zt));
        rails::is_rail(&s).then(|| (xt, yt, zt, rails::exits(rails::shape_of(&s))))
    };
    // OldMinecartBehavior.getPos
    let get_pos = |x: f64, y: f64, z: f64| -> Option<[f64; 3]> {
        let (xt, yt, zt, (e0, e1)) = rail_at(x, y, z)?;
        let p0 = [xt as f64 + 0.5 + e0[0] as f64 * 0.5, yt as f64 + 0.0625 + e0[1] as f64 * 0.5, zt as f64 + 0.5 + e0[2] as f64 * 0.5];
        let p1 = [xt as f64 + 0.5 + e1[0] as f64 * 0.5, yt as f64 + 0.0625 + e1[1] as f64 * 0.5, zt as f64 + 0.5 + e1[2] as f64 * 0.5];
        let (xd, yd, zd) = (p1[0] - p0[0], (p1[1] - p0[1]) * 2.0, p1[2] - p0[2]);
        let progress = if xd == 0.0 {
            z - zt as f64
        } else if zd == 0.0 {
            x - xt as f64
        } else {
            ((x - p0[0]) * xd + (z - p0[2]) * zd) * 2.0
        };
        let mut y = p0[1] + yd * progress;
        if yd < 0.0 {
            y += 1.0;
        } else if yd > 0.0 {
            y += 0.5;
        }
        Some([p0[0] + xd * progress, y, p0[2] + zd * progress])
    };
    // OldMinecartBehavior.getPosOffs
    let get_pos_offs = |x: f64, y: f64, z: f64, offs: f64| -> Option<[f64; 3]> {
        let (xt, yt, zt, (e0, e1)) = rail_at(x, y, z)?;
        let slope = e0[1] != 0 || e1[1] != 0;
        let mut yy = if slope { yt as f64 + 1.0 } else { yt as f64 };
        let (mut xd, mut zd) = ((e1[0] - e0[0]) as f64, (e1[2] - e0[2]) as f64);
        let dd = (xd * xd + zd * zd).sqrt();
        xd /= dd;
        zd /= dd;
        let (x, z) = (x + xd * offs, z + zd * offs);
        if e0[1] != 0 && floor(x) - xt == e0[0] && floor(z) - zt == e0[2] {
            yy += e0[1] as f64;
        } else if e1[1] != 0 && floor(x) - xt == e1[0] && floor(z) - zt == e1[2] {
            yy += e1[1] as f64;
        }
        get_pos(x, yy, z)
    };
    let (x, y, z) = (pos[0], pos[1], pos[2]);
    if let (Some(on), Some(front), Some(back)) = (get_pos(x, y, z), get_pos_offs(x, y, z, 0.3), get_pos_offs(x, y, z, -0.3)) {
        let d = [back[0] - front[0], back[1] - front[1], back[2] - front[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let p = [on[0], (front[1] + back[1]) / 2.0, on[2]];
        if l != 0.0 {
            let d = [d[0] / l, d[1] / l, d[2] / l];
            return (p, (d[2].atan2(d[0]) * 180.0 / std::f64::consts::PI) as f32, (d[1].atan() * 73.0) as f32);
        }
        return (p, yaw, pitch);
    }
    (pos, yaw, pitch)
}

/// The game texture rect of a texture, if packed.
fn rect(g: &GameAssets, name: &str) -> Option<[u32; 4]> {
    g.tex.get(name).map(|t| t.rect)
}

/// `ChestModel.createSingleBodyLayer` (closed), texture 64 x 64, in block pixels (y up).
fn chest() -> Vec<EPart> {
    vec![
        part(vec![cube(0.0, 19.0, [1.0, 0.0, 1.0], [14.0, 10.0, 14.0])], [0.0; 3], [0.0; 3]),
        part(vec![cube(0.0, 0.0, [1.0, 0.0, 0.0], [14.0, 5.0, 14.0])], [0.0, 9.0, 1.0], [0.0; 3]),
        part(vec![cube(0.0, 0.0, [7.0, -2.0, 14.0], [2.0, 4.0, 1.0])], [0.0, 9.0, 1.0], [0.0; 3]),
    ]
}

/// `ShulkerModel.createBoxLayer` (closed: the lid at 24, not turned), texture 64 x 64.
fn shulker() -> Vec<EPart> {
    vec![
        part(vec![cube(0.0, 0.0, [-8.0, -16.0, -8.0], [16.0, 12.0, 16.0])], [0.0, 24.0, 0.0], [0.0; 3]),
        part(vec![cube(0.0, 28.0, [-8.0, -8.0, -8.0], [16.0, 8.0, 16.0])], [0.0, 24.0, 0.0], [0.0; 3]),
    ]
}

/// A chest or shulker box at `cell` drawn like the game's block-entity renderers
/// (`ChestRenderer`: turned about the cell's centre by its facing; `ShulkerBoxRenderer`: turned
/// by `Direction.getRotation`, flipped, 0.9995 scale), with the game's entity textures. None:
/// not one of them, or its texture is not packed.
pub fn block_entity_triangles(g: &GameAssets, state: &str, cell: [i32; 3]) -> Option<Vec<Vertex>> {
    let name = state.split('[').next()?.trim().trim_start_matches("minecraft:");
    let facing = state.split("facing=").nth(1).and_then(|r| r.split([',', ']']).next()).unwrap_or("");
    let (tw, th) = (g.width as f32, g.height as f32);
    let base = M::id().translate([cell[0] as f32, cell[1] as f32, cell[2] as f32]);
    let mut out = Vec::new();
    if name == "chest" {
        let r = rect(g, "entity/chest/normal")?;
        // Direction.toYRot
        let yrot: f32 = match facing {
            "south" => 0.0,
            "west" => 90.0,
            "east" => 270.0,
            _ => 180.0,
        };
        let m = base.translate([0.5, 0.0, 0.5]).rot(1, (-yrot).to_radians()).translate([-0.5, 0.0, -0.5]);
        emit(&mut out, &chest(), &m, [64.0, 64.0], r, tw, th);
        return Some(out);
    }
    if name.ends_with("shulker_box") {
        let colour = name.trim_end_matches("shulker_box").trim_end_matches('_');
        let r = if colour.is_empty() { rect(g, "entity/shulker/shulker") } else { rect(g, &format!("entity/shulker/shulker_{colour}")).or_else(|| rect(g, "entity/shulker/shulker")) }?;
        // Direction.getRotation
        let turn = match facing {
            "down" => M::id().rot(0, PI),
            "north" => M::id().rot(0, PI / 2.0).rot(2, PI),
            "south" => M::id().rot(0, PI / 2.0),
            "west" => M::id().rot(0, PI / 2.0).rot(2, PI / 2.0),
            "east" => M::id().rot(0, PI / 2.0).rot(2, -PI / 2.0),
            _ => M::id(),
        };
        let m = base.translate([0.5, 0.5, 0.5]).scale([0.9995, 0.9995, 0.9995]).mul(&turn).scale([1.0, -1.0, -1.0]).translate([0.0, -1.0, 0.0]);
        emit(&mut out, &shulker(), &m, [64.0, 64.0], r, tw, th);
        return Some(out);
    }
    None
}

/// The triangles of an entity drawn like the game (None: not one of the game's models the viewer
/// has, or its texture is not loaded: the viewer's own stand-in is drawn).
pub fn entity_triangles(g: &GameAssets, kind: &str, pos: [f32; 3], yaw: f32, pitch: f32) -> Option<Vec<Vertex>> {
    let k = kind.strip_prefix("minecraft:").unwrap_or(kind);
    let (tw, th) = (g.width as f32, g.height as f32);
    let mut out = Vec::new();
    let at = M::id().translate(pos);
    if k.ends_with("_boat") || k.ends_with("_raft") {
        let chest = k.contains("chest");
        let wood = k.trim_end_matches("_chest_boat").trim_end_matches("_boat").trim_end_matches("_chest_raft").trim_end_matches("_raft");
        let name = if chest { format!("entity/chest_boat/{wood}") } else { format!("entity/boat/{wood}") };
        let r = rect(g, &name).or_else(|| rect(g, if chest { "entity/chest_boat/oak" } else { "entity/boat/oak" }))?;
        // AbstractBoatRenderer.submit
        let m = at.translate([0.0, 0.375, 0.0]).rot(1, (180.0 - yaw).to_radians()).scale([-1.0, -1.0, 1.0]).rot(1, PI / 2.0);
        emit(&mut out, &boat(chest), &m, if chest { [128.0, 128.0] } else { [128.0, 64.0] }, r, tw, th);
        return Some(out);
    }
    if k.contains("minecart") {
        let r = rect(g, "entity/minecart/minecart")?;
        // AbstractMinecartRenderer.oldRender (the caller passes the rail-aligned pose)
        let m = at.translate([0.0, 0.375, 0.0]).rot(1, (180.0 - yaw).to_radians()).rot(2, (-pitch).to_radians());
        emit(&mut out, &minecart(), &m.scale([-1.0, -1.0, 1.0]), [64.0, 32.0], r, tw, th);
        return Some(out);
    }
    if k == "armor_stand" {
        let r = rect(g, "entity/armorstand/armorstand")?;
        // LivingEntityRenderer.submit + ArmorStandRenderer.setupRotations
        let m = at.rot(1, (180.0 - yaw).to_radians()).scale([-1.0, -1.0, 1.0]).translate([0.0, -1.501, 0.0]);
        emit(&mut out, &armor_stand(yaw), &m, [64.0, 64.0], r, tw, th);
        return Some(out);
    }
    None
}

/// `TntRenderer`: a primed TNT's swell over the last 10 ticks of its fuse (`fuse` = the fuse left
/// as drawn: `getFuse() - partialTicks + 1`), and whether it flashes white (every 5 ticks).
pub fn tnt_look(fuse: f32) -> (f32, bool) {
    let swell = if fuse < 10.0 {
        let g = (1.0 - fuse / 10.0).clamp(0.0, 1.0);
        let g = g * g;
        g * g * 0.3
    } else {
        0.0
    };
    (1.0 + swell, fuse >= 0.0 && (fuse / 5.0) as i32 % 2 == 0)
}

/// A minecart's display block transform (`AbstractMinecartRenderer`: 0.75 scale, offset by the
/// display offset, a quarter turn), before the model's flip: the block model's unit cube goes
/// through it.
pub fn minecart_block_transform(pos: [f32; 3], yaw: f32, pitch: f32, display_offset: i32) -> M {
    M::id().translate(pos).translate([0.0, 0.375, 0.0]).rot(1, (180.0 - yaw).to_radians()).rot(2, (-pitch).to_radians()).scale([0.75, 0.75, 0.75]).translate([-0.5, (display_offset - 8) as f32 / 16.0, 0.5]).rot(1, PI / 2.0)
}

/// A dropped item's transform (`ItemEntityRenderer`: bob and spin with its age; `bobOffs` 0)
/// for a block item: the block model scaled by the item model's `ground` display (0.25, 3 px up).
pub fn item_block_transform(pos: [f32; 3], age: f32) -> M {
    let bob = (age / 10.0).sin() * 0.1 + 0.1;
    let spin = age / 20.0;
    // ItemRenderer: display transform (translation [0, 3, 0] px, scale 0.25), then the model's
    // unit cube centred
    M::id().translate(pos).translate([0.0, bob + 0.0625 + 0.125 - 0.1875, 0.0]).rot(1, spin).translate([0.0, 0.1875, 0.0]).scale([0.25, 0.25, 0.25]).translate([-0.5, -0.5, -0.5])
}

/// A dropped item that is not a block: its item sprite as a flat double-sided square (the
/// `ground` display: 0.5 scale, 2 px up), bobbing and spinning.
pub fn item_sprite_triangles(g: &GameAssets, item: &str, pos: [f32; 3], age: f32) -> Option<Vec<Vertex>> {
    let name = format!("item/{}", item.strip_prefix("minecraft:").unwrap_or(item));
    let r = rect(g, &name)?;
    let (tw, th) = (g.width as f32, g.height as f32);
    let bob = (age / 10.0).sin() * 0.1 + 0.1;
    let m = M::id().translate(pos).translate([0.0, bob + 0.0625, 0.0]).rot(1, age / 20.0).translate([0.0, 0.125, 0.0]).scale([0.5, 0.5, 0.5]).translate([-0.5, -0.5, 0.0]);
    let q = [[0.0, 1.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]].map(|p| m.point(p));
    let uv = |u: f32, v: f32| [(r[0] as f32 + 0.02 + u * (r[2] as f32 - 0.04)) / tw, (r[1] as f32 + 0.02 + v * (r[3] as f32 - 0.04)) / th];
    let n = normalize(m.dir([0.0, 0.0, 1.0]));
    let mut out = Vec::new();
    for (side, nn) in [(false, n), (true, [-n[0], -n[1], -n[2]])] {
        let l = (entity_light(nn) * 255.0).round() as u8;
        let mk = |k: usize, u: f32, v: f32| Vertex { pos: q[k], uv: uv(u, v), layer: crate::atlas::GAME_LAYER, color: [l, l, l, 255] };
        let vs = [mk(0, 0.0, 0.0), mk(1, 0.0, 1.0), mk(2, 1.0, 1.0), mk(3, 1.0, 0.0)];
        if side {
            out.extend_from_slice(&[vs[0], vs[2], vs[1], vs[0], vs[3], vs[2]]);
        } else {
            out.extend_from_slice(&[vs[0], vs[1], vs[2], vs[0], vs[2], vs[3]]);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_like_joml() {
        // rotationZYX order: x first, then y, then z
        let m = M::id().rot(2, PI / 2.0).rot(1, 0.0).rot(0, PI / 2.0);
        let p = m.point([0.0, 1.0, 0.0]);
        // x turns +y to +z, then z leaves +z
        assert!((p[2] - 1.0).abs() < 1e-5, "{p:?}");
        assert!((entity_light([0.0, 1.0, 0.0]) - 1.0).abs() < 1e-6);
        assert!(entity_light([0.0, -1.0, 0.0]) < 0.41);
    }

    /// The boat's hull spans 1.75 blocks along its length, sits on y 0.375 - 3/16 .. 0.375 +
    /// 6/16 + ..., and its faces point outward (counter-clockwise from outside).
    #[test]
    fn boat_geometry() {
        let mut out = Vec::new();
        let m = M::id().translate([0.0, 0.375, 0.0]).rot(1, PI).scale([-1.0, -1.0, 1.0]).rot(1, PI / 2.0);
        emit(&mut out, &boat(false), &m, [128.0, 64.0], [0, 0, 128, 64], 1024.0, 1024.0);
        let lo = (0..3).map(|k| out.iter().map(|v| v.pos[k]).fold(f32::MAX, f32::min)).collect::<Vec<_>>();
        let hi = (0..3).map(|k| out.iter().map(|v| v.pos[k]).fold(f32::MIN, f32::max)).collect::<Vec<_>>();
        eprintln!("boat bounds {lo:?} .. {hi:?}, {} triangles", out.len() / 3);
        assert!(hi[1] > 0.5 && lo[1] > -0.2 && lo[1] < 0.3, "{lo:?} {hi:?}");
        // the hull's bottom plank alone: its lower face (y lowest) faces down
        let mut hull = Vec::new();
        emit(&mut hull, &boat(false)[..1], &m, [128.0, 64.0], [0, 0, 128, 64], 1024.0, 1024.0);
        let low = hull.iter().map(|v| v.pos[1]).fold(f32::MAX, f32::min);
        let bottom = hull.chunks(3).find(|t| t.iter().all(|v| (v.pos[1] - low).abs() < 1e-4)).expect("a bottom triangle");
        let (a, b, c) = (bottom[0].pos, bottom[1].pos, bottom[2].pos);
        let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
        let ny = e1[2] * e2[0] - e1[0] * e2[2];
        assert!(ny < 0.0, "the hull's underside faces down ({ny})");
        // the hull is 28 px long (along z at yaw 0: the boat faces south) and 16 px wide
        let span = |k: usize| hull.iter().map(|v| v.pos[k]).fold(f32::MIN, f32::max) - hull.iter().map(|v| v.pos[k]).fold(f32::MAX, f32::min);
        assert!((span(2) - 1.75).abs() < 1e-3 && (span(0) - 1.0).abs() < 1e-3, "{} x {}", span(0), span(2));
    }
}
