//! A scene's triangles: block models placed in their cells (moving blocks at their offsets),
//! faces against opaque neighbors culled, corners darkened like the game's smooth lighting
//! (ambient occlusion), faces shaded by direction, translucent faces kept apart for sorting.

use crate::atlas::{Atlas, GAME_LAYER};
use crate::design::Pos;
use crate::models::{self, Face, Layer, Model, Part};
use std::collections::{HashMap, HashSet};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    /// Texture coordinates within the tile (0..1; beyond 1 repeats).
    pub uv: [f32; 2],
    /// The tile: a layer of the texture array.
    pub layer: u32,
    /// rgb = tint x shade x ambient occlusion, a = alpha (cutout faces are tested in the shader).
    pub color: [u8; 4],
}

/// One quad (two triangles), kept for back-to-front sorting.
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub v: [Vertex; 4],
    pub center: [f32; 3],
}

/// Something to draw besides blocks: an entity (feet position in blocks, yaw in degrees), with
/// its pitch (a minecart on a slope), age in ticks (a dropped item's bob and spin) and fuse left
/// (a primed TNT's flash and swell).
#[derive(Clone, Debug, Default)]
pub struct EntityDraw {
    pub kind: String,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub age: f32,
    pub fuse: f32,
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

/// `minecraft:water[level=1]` -> `minecraft:water`.
fn base_name(state: &str) -> &str {
    state.split('[').next().unwrap_or(state)
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
    /// Block names of the still cells (translucent faces between the same blocks are culled).
    names: HashMap<Pos, String>,
    /// The ground: every cell below this y counts as an opaque block (culling, AO).
    pub ground: Option<i32>,
    cache: HashMap<String, Option<Model>>,
    pub mesh: Mesh,
    /// A transform for the block model being drawn as part of an entity (a minecart's hopper, a
    /// dropped block item): its unit cube goes through it.
    xf: Option<crate::entitymodel::M>,
    /// The block model being drawn flashes white (a primed TNT: `OverlayTexture`'s white at
    /// u = 15, 75% white): its solid faces carry the mix in their vertex alpha.
    white: bool,
}

impl<'a> Builder<'a> {
    pub fn new(atlas: &'a Atlas) -> Builder<'a> {
        Builder { atlas, occ: HashSet::new(), names: HashMap::new(), ground: None, cache: HashMap::new(), mesh: Mesh { lo: [f32::MAX; 3], hi: [f32::MIN; 3], ..Default::default() }, xf: None, white: false }
    }

    fn occupied(&self, c: Pos) -> bool {
        self.occ.contains(&c) || self.ground.is_some_and(|g| c[1] < g)
    }

    fn model(&mut self, state: &str) -> Option<Model> {
        self.model_at(state, None)
    }

    /// A block's model; at `cell` for a block the game draws as one of several variants picked
    /// from its position (cached per state, and per cell for those).
    fn model_at(&mut self, state: &str, cell: Option<Pos>) -> Option<Model> {
        let positional = cell.is_some() && self.atlas.game.as_ref().is_some_and(|g| g.models.positional(state));
        let key = if positional { format!("{state}@{:?}", cell) } else { state.to_string() };
        if let Some(m) = self.cache.get(&key) {
            return m.clone();
        }
        let m = models::model_at(state, self.atlas, if positional { cell } else { None });
        self.cache.insert(key, m.clone());
        m
    }

    /// Add blocks (call once per frame with every block).
    pub fn blocks(&mut self, items: &[Item]) {
        self.occ.clear();
        self.names.clear();
        let states: HashMap<Pos, &str> = items.iter().filter(|it| it.offset == [0.0; 3]).map(|it| (it.cell, it.state)).collect();
        for it in items {
            if it.offset == [0.0; 3] {
                // water and bubble columns are one fluid: no faces between them
                let n = match base_name(it.state) {
                    "minecraft:bubble_column" => "minecraft:water",
                    n => n,
                };
                self.names.insert(it.cell, n.to_string());
                if let Some(m) = self.model_at(it.state, Some(it.cell))
                    && m.full
                    && m.parts.iter().all(|p| p.layer == Layer::Opaque)
                {
                    self.occ.insert(it.cell);
                }
            }
        }
        for it in items {
            // water: the game's sloped surface (corner heights from the neighbouring cells), not
            // cuboids (their steps showed as lines across a pool)
            if it.offset == [0.0; 3] && matches!(base_name(it.state), "minecraft:water" | "minecraft:bubble_column") {
                self.water(it.cell, &states);
                continue;
            }
            // water under water fills its cell (the game's surface is only at the top)
            let state = if it.offset == [0.0; 3] && base_name(it.state) == "minecraft:water" && self.names.get(&[it.cell[0], it.cell[1] + 1, it.cell[2]]).is_some_and(|n| n == "minecraft:water") {
                "minecraft:water[level=8]"
            } else {
                it.state
            };
            // chests and shulker boxes: the game's block-entity models (ChestModel, ShulkerModel)
            // with its entity textures, once loaded
            if it.offset == [0.0; 3] && self.block_entity(state, it.cell) {
                continue;
            }
            if let Some(m) = self.model_at(state, Some(it.cell)) {
                let base = [it.cell[0] as f32 + it.offset[0], it.cell[1] as f32 + it.offset[1], it.cell[2] as f32 + it.offset[2]];
                let still = it.offset == [0.0; 3];
                self.add_model(&m, base, if still { Some(it.cell) } else { None }, None);
            }
        }
    }

    /// A chest or shulker box drawn like the game's block-entity renderers (false: not one, or
    /// the entity textures are not loaded yet: the viewer's model is drawn).
    fn block_entity(&mut self, state: &str, cell: Pos) -> bool {
        let id = base_name(state);
        if id != "minecraft:chest" && !id.ends_with("shulker_box") {
            return false;
        }
        let Some(g) = self.atlas.game.clone() else { return false };
        if !g.entities {
            g.wants_entities.set(true);
            return false;
        }
        match crate::entitymodel::block_entity_triangles(&g, state, cell) {
            Some(tris) => {
                for v in &tris {
                    for k in 0..3 {
                        self.mesh.lo[k] = self.mesh.lo[k].min(v.pos[k]);
                        self.mesh.hi[k] = self.mesh.hi[k].max(v.pos[k]);
                    }
                }
                self.mesh.solid.extend_from_slice(&tris);
                true
            }
            None => false,
        }
    }

    /// An entity model at a feet position, turned by yaw (degrees, the game's: 0 = south).
    pub fn entity(&mut self, e: &EntityDraw) {
        if let Some(state) = e.kind.strip_prefix("falling:") {
            // FallingBlockRenderer: the block model, half a block back (the game's or the viewer's)
            if let Some(m) = self.model(state) {
                self.add_model(&m, [e.pos[0] - 0.5, e.pos[1], e.pos[2] - 0.5], None, None);
            }
            return;
        }
        if let Some(state) = e.kind.strip_prefix("primed:") {
            // TntRenderer: the block model centred on the entity, swelling over the last 10
            // ticks, flashing white every 5 (the white overlay: 75% white)
            if let Some(m) = self.model(state) {
                let (scale, white) = crate::tnt::look(e.fuse);
                self.xf = Some(crate::entitymodel::M::id().translate([e.pos[0], e.pos[1] + 0.5, e.pos[2]]).scale([scale, scale, scale]).translate([-0.5, -0.5, -0.5]));
                self.white = white;
                self.add_model(&m, [0.0; 3], None, None);
                self.xf = None;
                self.white = false;
            }
            return;
        }
        let game = self.atlas.game.clone();
        if let Some(g) = &game {
            if !g.entities {
                // drawn with the viewer's stand-in until the page loads the game's entity textures
                g.wants_entities.set(true);
            } else if self.game_entity(g, e) {
                return;
            }
        }
        // an item entity: a small cube of its block's texture, else of its color
        if let Some(id) = e.kind.strip_prefix("item:") {
            let n = id.strip_prefix("minecraft:").unwrap_or(id);
            let part = match self.atlas.names.get_key_value(n) {
                Some((tile, _)) => Part::new([-2.0, 0.0, -2.0], [2.0, 4.0, 2.0]).all(tile),
                None => Part::new([-2.0, 0.0, -2.0], [2.0, 4.0, 2.0]).all("generic").tint(models::name_color(n)),
            };
            let m = Model::one(part.nocull());
            self.add_model(&m, [e.pos[0], e.pos[1] + 0.1, e.pos[2]], None, Some(e.yaw));
            return;
        }
        if let Some(m) = models::entity_model(&e.kind) {
            self.add_model(&m, e.pos, None, Some(e.yaw));
        }
    }

    /// An entity drawn like the game: its own model and texture (boats, minecarts with their
    /// display block, armor stands), a falling block's block model, a dropped item (a block
    /// item's block model, else its sprite). False when the viewer has nothing for it.
    fn game_entity(&mut self, g: &crate::atlas::GameAssets, e: &EntityDraw) -> bool {
        use crate::entitymodel as em;
        let push = |b: &mut Self, tris: Vec<Vertex>| {
            for v in &tris {
                for k in 0..3 {
                    b.mesh.lo[k] = b.mesh.lo[k].min(v.pos[k]);
                    b.mesh.hi[k] = b.mesh.hi[k].max(v.pos[k]);
                }
            }
            b.mesh.solid.extend_from_slice(&tris);
        };
        if let Some(id) = e.kind.strip_prefix("item:") {
            let name = id.strip_prefix("minecraft:").unwrap_or(id);
            if g.tex.contains_key(&format!("item/{name}")) {
                if let Some(t) = em::item_sprite_triangles(g, id, e.pos, e.age) {
                    push(self, t);
                    return true;
                }
            } else if let Some(m) = self.model(&format!("minecraft:{name}")) {
                self.xf = Some(em::item_block_transform(e.pos, e.age));
                self.add_model(&m, [0.0; 3], None, None);
                self.xf = None;
                return true;
            }
            return false;
        }
        let Some(tris) = em::entity_triangles(g, &e.kind, e.pos, e.yaw, e.pitch) else { return false };
        push(self, tris);
        // a hopper minecart's hopper (display offset 1), a chest minecart's chest...
        let k = e.kind.strip_prefix("minecraft:").unwrap_or(&e.kind);
        let display = match k {
            "hopper_minecart" => Some(("minecraft:hopper[enabled=true,facing=down]", 1)),
            "chest_minecart" => Some(("minecraft:chest[facing=north,type=single,waterlogged=false]", 8)),
            "tnt_minecart" => Some(("minecraft:tnt[unstable=false]", 6)),
            "furnace_minecart" => Some(("minecraft:furnace[facing=north,lit=false]", 6)),
            _ => None,
        };
        if let Some((state, offset)) = display
            && let Some(m) = self.model(state)
        {
            self.xf = Some(em::minecart_block_transform(e.pos, e.yaw, e.pitch, offset));
            self.add_model(&m, [0.0; 3], None, None);
            self.xf = None;
        }
        true
    }

    /// A water (or bubble column) cell like the game draws fluids: its surface at each corner the
    /// average height of the water cells sharing that corner (full when water stands above any
    /// of them), faces only toward cells that are neither water nor opaque cubes.
    fn water(&mut self, cell: Pos, states: &HashMap<Pos, &str>) {
        let is_water = |names: &HashMap<Pos, String>, p: Pos| names.get(&p).is_some_and(|n| n == "minecraft:water");
        // the own height of a water cell (in blocks): full under water, else the game's amount / 9
        let level_of = |p: Pos| -> Option<u32> {
            states.get(&p).map(|s| {
                if base_name(s) == "minecraft:bubble_column" {
                    0
                } else {
                    s.split("level=").nth(1).and_then(|r| r.split([',', ']']).next()).and_then(|v| v.parse().ok()).unwrap_or(0)
                }
            })
        };
        let names = &self.names;
        let height = |p: Pos| -> Option<f32> {
            if !is_water(names, p) {
                return None;
            }
            if is_water(names, [p[0], p[1] + 1, p[2]]) {
                return Some(1.0);
            }
            let l = level_of(p).unwrap_or(0);
            Some(if l == 0 || l >= 8 { 8.0 / 9.0 } else { (8 - l) as f32 / 9.0 })
        };
        let above = is_water(names, [cell[0], cell[1] + 1, cell[2]]);
        // corner (dx, dz) in {0,1}: the cells sharing it
        let corner = |dx: i32, dz: i32| -> f32 {
            if above {
                return 1.0;
            }
            let (mut sum, mut n) = (0.0f32, 0.0f32);
            for (ox, oz) in [(dx - 1, dz - 1), (dx, dz - 1), (dx - 1, dz), (dx, dz)] {
                let q = [cell[0] + ox, cell[1], cell[2] + oz];
                if let Some(h) = height(q) {
                    if h >= 1.0 {
                        return 1.0;
                    }
                    sum += h;
                    n += 1.0;
                }
            }
            if n > 0.0 { sum / n } else { 8.0 / 9.0 }
        };
        let h = [corner(0, 0), corner(0, 1), corner(1, 1), corner(1, 0)];
        let tile = self.atlas.tile("water") as u32;
        let (x, y, z) = (cell[0] as f32, cell[1] as f32, cell[2] as f32);
        let quad = |b: &mut Self, pts: [[f32; 3]; 4], uv: [[f32; 2]; 4], n: [f32; 3]| {
            let l = shade(n);
            let c = (255.0 * l).round() as u8;
            let v: [Vertex; 4] = std::array::from_fn(|k| Vertex { pos: pts[k], uv: uv[k], layer: tile, color: [c, c, c, 255] });
            let center = [(pts[0][0] + pts[2][0]) / 2.0, (pts[0][1] + pts[2][1]) / 2.0, (pts[0][2] + pts[2][2]) / 2.0];
            b.mesh.translucent.push(Quad { v, center });
            for q in pts {
                for k in 0..3 {
                    b.mesh.lo[k] = b.mesh.lo[k].min(q[k]);
                    b.mesh.hi[k] = b.mesh.hi[k].max(q[k]);
                }
            }
        };
        let open = |b: &Self, q: Pos| !is_water(&b.names, q) && !b.occupied(q);
        // the surface (seen from above and below)
        if !above {
            quad(self, [[x, y + h[0], z], [x, y + h[1], z + 1.0], [x + 1.0, y + h[2], z + 1.0], [x + 1.0, y + h[3], z]], [[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]], [0.0, 1.0, 0.0]);
            quad(self, [[x, y + h[0], z], [x + 1.0, y + h[3], z], [x + 1.0, y + h[2], z + 1.0], [x, y + h[1], z + 1.0]], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], [0.0, 1.0, 0.0]);
        }
        if open(self, [cell[0], cell[1] - 1, cell[2]]) {
            quad(self, [[x, y, z], [x + 1.0, y, z], [x + 1.0, y, z + 1.0], [x, y, z + 1.0]], [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]], [0.0, -1.0, 0.0]);
        }
        // sides: north (-z) uses corners 0 (x0,z0) and 3 (x1,z0); south (+z) 1 and 2; west (-x)
        // 0 and 1; east (+x) 3 and 2
        let sides: [(Pos, [[f32; 3]; 4], [f32; 2], [f32; 3]); 4] = [
            ([0, 0, -1], [[x + 1.0, y, z], [x, y, z], [x, y + h[0], z], [x + 1.0, y + h[3], z]], [h[3], h[0]], [0.0, 0.0, -1.0]),
            ([0, 0, 1], [[x, y, z + 1.0], [x + 1.0, y, z + 1.0], [x + 1.0, y + h[2], z + 1.0], [x, y + h[1], z + 1.0]], [h[1], h[2]], [0.0, 0.0, 1.0]),
            ([-1, 0, 0], [[x, y, z], [x, y, z + 1.0], [x, y + h[1], z + 1.0], [x, y + h[0], z]], [h[0], h[1]], [-1.0, 0.0, 0.0]),
            ([1, 0, 0], [[x + 1.0, y, z + 1.0], [x + 1.0, y, z], [x + 1.0, y + h[3], z], [x + 1.0, y + h[2], z + 1.0]], [h[2], h[3]], [1.0, 0.0, 0.0]),
        ];
        for (d, pts, top, n) in sides {
            if open(self, [cell[0] + d[0], cell[1] + d[1], cell[2] + d[2]]) {
                quad(self, pts, [[0.0, 1.0], [1.0, 1.0], [1.0, 1.0 - top[1]], [0.0, 1.0 - top[0]]], n);
            }
        }
    }

    /// The orange marker (stained glass) or an outline box (input) at a cell.
    pub fn marker(&mut self, cell: Pos, glass: bool, color: [u8; 3], lit: bool) {
        let base = [cell[0] as f32, cell[1] as f32, cell[2] as f32];
        if glass {
            // culled against the solid blocks it touches, like the game draws glass: never a
            // face coplanar with a block's (no z-fighting)
            let m = models::marker_model(lit);
            self.add_model(&m, base, Some(cell), None);
            return;
        }
        let t = 0.4;
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
        let (qx, qy) = p.qrot.unwrap_or((m.xrot, m.yrot));
        let mut pts = corners.map(|c| {
            let c = match p.rot {
                Some(r) => {
                    let mut q = rotate(c, r.axis, r.angle, r.origin);
                    if r.rescale {
                        // stretched back across the other two axes (the game's rescale)
                        let k = 1.0 / r.angle.to_radians().cos().abs().max(1e-3);
                        for a in 0..3 {
                            if a != r.axis as usize {
                                q[a] = r.origin[a] + (q[a] - r.origin[a]) * k;
                            }
                        }
                    }
                    q
                }
                None => c,
            };
            let c = [c[0] / 16.0, c[1] / 16.0, c[2] / 16.0];
            let c = quarter(c, qx, qy);
            let c = match yaw {
                // entities: their model is built around x = z = 0 (the entity's position); the
                // game's yaw turns clockwise
                Some(y) => rotate(c, 1, -y, [0.0, 0.0, 0.0]),
                None => c,
            };
            // a block model drawn as part of an entity (its unit cube through the transform)
            match &self.xf {
                Some(m) => m.point(c),
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
            // against an opaque cube; translucent blocks also against their own kind (water, glass)
            if self.occupied(q) || (p.layer == Layer::Translucent && self.names.get(&q).is_some_and(|n| self.names.get(&c) == Some(n))) {
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
                    let (o1, o2, o3) = (self.occupied(s1), self.occupied(s2), self.occupied(cc));
                    let level = if o1 && o2 { 3 } else { o1 as usize + o2 as usize + o3 as usize };
                    out[k] = AO[level];
                }
                out
            }
            _ => [1.0; 4],
        };
        let r = f.uv.unwrap_or(auto);
        // uvlock: the texture keeps the world's orientation: each corner's pixel from its final
        // place on the block, like the automatic UVs of an unturned face
        let locked: Option<[[f32; 2]; 4]> = if p.uvlock && p.rot.is_none() && (qx, qy) != (0, 0) {
            let axis = (0..3).find(|&k| n[k].abs() > 0.999);
            axis.map(|a| {
                pts.map(|q| {
                    let (x, y, z) = (q[0] * 16.0, q[1] * 16.0, q[2] * 16.0);
                    match (a, n[a] > 0.0) {
                        (1, true) => [x, z],
                        (1, false) => [x, 16.0 - z],
                        (2, false) => [16.0 - x, 16.0 - y],
                        (2, true) => [x, 16.0 - y],
                        (0, false) => [z, 16.0 - y],
                        _ => [16.0 - z, 16.0 - y],
                    }
                })
            })
        } else {
            None
        };
        // the game's own texture (a game model's face): its place in the game texture
        let game = self.atlas.game.as_ref().and_then(|g| g.tex.get(f.tile).map(|t| (t.rect, g.width as f32, g.height as f32)));
        let (layer, uv): (u32, [[f32; 2]; 4]) = match game {
            Some((rect, w, h)) => {
                let s = rect[2] as f32 / 16.0;
                let (x0, y0) = (rect[0] as f32, rect[1] as f32);
                let (x1, y1) = (x0 + rect[2] as f32, y0 + rect[3] as f32);
                // inside the sprite by a sliver: nearest sampling never reads the neighbour
                let map = |px: [f32; 2]| -> [f32; 2] { [((x0 + px[0] * s).clamp(x0 + 0.02, x1 - 0.02)) / w, ((y0 + px[1] * s).clamp(y0 + 0.02, y1 - 0.02)) / h] };
                let uv = match locked {
                    Some(l) => l.map(map),
                    None => {
                        let base_px = [[r[0], r[1]], [r[0], r[3]], [r[2], r[3]], [r[2], r[1]]];
                        std::array::from_fn(|k| map(base_px[(k + f.rot as usize) % 4]))
                    }
                };
                (GAME_LAYER, uv)
            }
            None => {
                let tile = self.atlas.tile(f.tile);
                let uv = match locked {
                    Some(l) => l.map(|px| [px[0] / 16.0, px[1] / 16.0]),
                    None => {
                        let [u0, v0, u1, v1] = self.atlas.uv(tile, r);
                        let base_uv = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
                        std::array::from_fn(|k| base_uv[(k + f.rot as usize) % 4])
                    }
                };
                (tile as u32, uv)
            }
        };
        // solid faces: 255, or the white flash's 25% (fs_solid mixes toward white by 1 - alpha)
        let alpha = if self.white && p.layer != Layer::Translucent { 64u8 } else { 255u8 };
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
            Vertex { pos: pts[k], uv: uv[k], layer, color: [c(f.tint[0]), c(f.tint[1]), c(f.tint[2]), alpha] }
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
/// Which of 4 equally weighted blockstate variants the game draws at a block (grass blocks' 4
/// turns): `random.setSeed(Mth.getSeed(pos))` then `random.nextInt(4)` with the game's
/// `LegacyRandomSource` (java.util.Random's generator).
pub fn variant_of_4(x: i32, y: i32, z: i32) -> u32 {
    // Mth.getSeed: `x * 3129871` is an int product, widened
    let mut seed: i64 = (x.wrapping_mul(3129871) as i64) ^ (z as i64).wrapping_mul(116129781) ^ (y as i64);
    seed = seed.wrapping_mul(seed).wrapping_mul(42317861).wrapping_add(seed.wrapping_mul(11));
    let seed = seed >> 16;
    const MASK: i64 = (1 << 48) - 1;
    let mut s = (seed ^ 0x5DEECE66D) & MASK;
    s = (s.wrapping_mul(0x5DEECE66D).wrapping_add(0xB)) & MASK;
    let next31 = (s >> (48 - 31)) as i32 as i64;
    // nextInt(bound) for a power of two: (bound * next(31)) >> 31
    ((4 * next31) >> 31) as u32
}

pub fn ground(atlas: &Atlas, lo: [f32; 3], hi: [f32; 3], y: f32, r: f32) -> Vec<Vertex> {
    // a one-block-thick layer of grass blocks: the grass tile repeated once per block on top,
    // dirt under it and grass-block sides around it (seen from below or from the edge too)
    let top = atlas.tile("grass_top") as u32;
    let dirt = atlas.tile("dirt") as u32;
    let side = atlas.tile("grass_side") as u32;
    let x0 = (lo[0] - r).floor();
    let x1 = (hi[0] + r).ceil();
    let z0 = (lo[2] - r).floor();
    let z1 = (hi[2] + r).ceil();
    let yb = y - 1.0;
    let lum = |s: f32| {
        let c = (255.0 * s).round() as u8;
        [c, c, c, 255]
    };
    let mut out = Vec::new();
    let mut quad = |v: [Vertex; 4]| out.extend_from_slice(&[v[0], v[1], v[2], v[0], v[2], v[3]]);
    let c = lum(1.0);
    // one quad per grass block near the build: the game turns each block's model by a random
    // quarter (its blockstate's 4 variants, picked from the block's position), so the top texture
    // lines up with the real game's pixel for pixel; farther out (small on screen) a few large
    // quads with the texture repeated
    let gy = (y - 1.0).floor() as i32;
    let (ix0, ix1, iz0, iz1) = (x0 as i32, x1 as i32, z0 as i32, z1 as i32);
    let near = 24;
    let (nx0, nx1, nz0, nz1) = ((lo[0].floor() as i32 - near).max(ix0), (hi[0].ceil() as i32 + near).min(ix1), (lo[2].floor() as i32 - near).max(iz0), (hi[2].ceil() as i32 + near).min(iz1));
    for bz in nz0..nz1 {
        for bx in nx0..nx1 {
            let k = variant_of_4(bx, gy, bz);
            // the texture coordinate of a corner (local dx, dz): the corner turned back by k
            // quarters (the game's y rotation turns clockwise seen from above)
            let uv = |dx: f32, dz: f32| -> [f32; 2] {
                let (mut px, mut pz) = (dx - 0.5, dz - 0.5);
                for _ in 0..k {
                    (px, pz) = (pz, -px);
                }
                [px + 0.5, pz + 0.5]
            };
            let (fx, fz) = (bx as f32, bz as f32);
            quad([
                Vertex { pos: [fx, y, fz], uv: uv(0.0, 0.0), layer: top, color: c },
                Vertex { pos: [fx, y, fz + 1.0], uv: uv(0.0, 1.0), layer: top, color: c },
                Vertex { pos: [fx + 1.0, y, fz + 1.0], uv: uv(1.0, 1.0), layer: top, color: c },
                Vertex { pos: [fx + 1.0, y, fz], uv: uv(1.0, 0.0), layer: top, color: c },
            ]);
        }
    }
    // the far grass around the near square: four strips, the texture repeated (world coordinates)
    let (nx0, nx1, nz0, nz1) = (nx0 as f32, nx1 as f32, nz0 as f32, nz1 as f32);
    for (a, b) in [([x0, z0], [x1, nz0]), ([x0, nz1], [x1, z1]), ([x0, nz0], [nx0, nz1]), ([nx1, nz0], [x1, nz1])] {
        if b[0] <= a[0] || b[1] <= a[1] {
            continue;
        }
        quad([
            Vertex { pos: [a[0], y, a[1]], uv: [a[0], a[1]], layer: top, color: c },
            Vertex { pos: [a[0], y, b[1]], uv: [a[0], b[1]], layer: top, color: c },
            Vertex { pos: [b[0], y, b[1]], uv: [b[0], b[1]], layer: top, color: c },
            Vertex { pos: [b[0], y, a[1]], uv: [b[0], a[1]], layer: top, color: c },
        ]);
    }
    let c = lum(0.5);
    quad([
        Vertex { pos: [x0, yb, z0], uv: [x0, z0], layer: dirt, color: c },
        Vertex { pos: [x1, yb, z0], uv: [x1, z0], layer: dirt, color: c },
        Vertex { pos: [x1, yb, z1], uv: [x1, z1], layer: dirt, color: c },
        Vertex { pos: [x0, yb, z1], uv: [x0, z1], layer: dirt, color: c },
    ]);
    // sides, wound to face outward (north -z, south +z, west -x, east +x)
    let (cz, cx) = (lum(0.8), lum(0.6));
    quad([
        Vertex { pos: [x0, y, z0], uv: [x0, 0.0], layer: side, color: cz },
        Vertex { pos: [x1, y, z0], uv: [x1, 0.0], layer: side, color: cz },
        Vertex { pos: [x1, yb, z0], uv: [x1, 1.0], layer: side, color: cz },
        Vertex { pos: [x0, yb, z0], uv: [x0, 1.0], layer: side, color: cz },
    ]);
    quad([
        Vertex { pos: [x1, y, z1], uv: [x1, 0.0], layer: side, color: cz },
        Vertex { pos: [x0, y, z1], uv: [x0, 0.0], layer: side, color: cz },
        Vertex { pos: [x0, yb, z1], uv: [x0, 1.0], layer: side, color: cz },
        Vertex { pos: [x1, yb, z1], uv: [x1, 1.0], layer: side, color: cz },
    ]);
    quad([
        Vertex { pos: [x0, y, z1], uv: [z1, 0.0], layer: side, color: cx },
        Vertex { pos: [x0, y, z0], uv: [z0, 0.0], layer: side, color: cx },
        Vertex { pos: [x0, yb, z0], uv: [z0, 1.0], layer: side, color: cx },
        Vertex { pos: [x0, yb, z1], uv: [z1, 1.0], layer: side, color: cx },
    ]);
    quad([
        Vertex { pos: [x1, y, z0], uv: [z0, 0.0], layer: side, color: cx },
        Vertex { pos: [x1, y, z1], uv: [z1, 0.0], layer: side, color: cx },
        Vertex { pos: [x1, yb, z1], uv: [z1, 1.0], layer: side, color: cx },
        Vertex { pos: [x1, yb, z0], uv: [z0, 1.0], layer: side, color: cx },
    ]);
    out
}
