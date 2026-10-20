//! A session as a picture: its blocks (moving ones mid-way), the output marker (orange stained
//! glass, the real-game pictures' marker), outlines on the inputs, the design's entities, the
//! grass ground; and the default camera framing it like `mctas clock shot`.

use crate::atlas::Atlas;
use crate::camera::{CAM_DIR, Camera, VFOV};
use crate::design::Pos;
use crate::mesh::{self, Builder, EntityDraw, Item, Mesh, Vertex};
use crate::session::{Session, bp};
use redsim::Block;

/// What to draw besides the blocks.
#[derive(Clone, Debug, Default)]
pub struct Look {
    /// The orange glass marker's cell (next to the output: [`marker_cell`]).
    pub marker: Option<Pos>,
    /// The marker glows while the output is on (the live viewer; still pictures keep the
    /// real-game pictures' plain orange glass).
    pub lit_marker: bool,
    /// Lime outlines (clickable inputs).
    pub outlines: Vec<Pos>,
    /// The ground's top (None: no ground).
    pub ground: Option<f32>,
}

/// Where the real-game pictures put the orange marker (`clock/src/shot.rs::marker_cell`): the
/// output cell when it is air; else the air cell the output component powers (an observer's back,
/// a repeater's or comparator's front, above a standing torch); else its air neighbor most visible
/// from the camera.
pub fn marker_cell(s: &Session, out: Pos) -> Option<Pos> {
    let air = |p: Pos| -> bool {
        if s.simulated() {
            matches!(s.world.get(bp(p)), Block::Air)
        } else {
            !s.design.blocks.contains_key(&p)
        }
    };
    if air(out) {
        return Some(out);
    }
    let state = if s.simulated() { s.world.get(bp(out)).to_string() } else { s.design.blocks.get(&out).cloned().unwrap_or_default() };
    let st = crate::models::St::parse(&state);
    let step = |f: &str| -> Option<Pos> {
        Some(match f {
            "north" => [0, 0, -1],
            "south" => [0, 0, 1],
            "east" => [1, 0, 0],
            "west" => [-1, 0, 0],
            "up" => [0, 1, 0],
            "down" => [0, -1, 0],
            _ => return None,
        })
    };
    let preferred: Option<Pos> = match st.name {
        "observer" | "repeater" | "comparator" => st.get("facing").and_then(step).map(|d| [-d[0], -d[1], -d[2]]),
        "redstone_torch" => Some([0, 1, 0]),
        _ => None,
    };
    let mut cands: Vec<Pos> = vec![[0, 0, 1], [0, 1, 0], [-1, 0, 0], [1, 0, 0], [0, 0, -1], [0, -1, 0]];
    let dot = |d: &Pos| d[0] as f32 * CAM_DIR[0] + d[1] as f32 * CAM_DIR[1] + d[2] as f32 * CAM_DIR[2];
    cands.sort_by(|a, b| dot(b).total_cmp(&dot(a)));
    if let Some(d) = preferred {
        cands.retain(|c| *c != d);
        cands.insert(0, d);
    }
    cands.into_iter().map(|d| [out[0] + d[0], out[1] + d[1], out[2] + d[2]]).find(|p| air(*p))
}

/// The default look of a session: the marker next to its first output, outlines on its inputs
/// (a door's lever), grass under its lowest block.
pub fn default_look(s: &Session) -> Look {
    let marker = s.outputs.first().and_then(|o| marker_cell(s, *o));
    let mut outlines: Vec<Pos> = Vec::new();
    for p in &s.inputs {
        if !outlines.contains(p) {
            outlines.push(*p);
        }
    }
    let (lo, _) = s.path_box();
    Look { marker, lit_marker: false, outlines, ground: Some(lo[1] as f32) }
}

/// The frame of a session at `partial` (0..1) after its current tick.
pub fn build(atlas: &Atlas, s: &Session, partial: f32, look: &Look) -> Mesh {
    let blocks = s.render_blocks(partial);
    let items: Vec<Item> = blocks.iter().map(|b| Item { cell: b.cell, offset: b.offset, state: &b.state }).collect();
    let mut b = Builder::new(atlas);
    b.ground = look.ground.map(|y| y.floor() as i32);
    b.blocks(&items);
    if let Some(r) = &s.replay {
        for e in r.entities(partial) {
            b.entity(&e);
        }
    } else if !s.simulated() {
        for e in &s.design.entities {
            b.entity(&EntityDraw { kind: e.kind.clone(), pos: [e.pos[0] as f32, e.pos[1] as f32, e.pos[2] as f32], yaw: entity_yaw(e.nbt.as_deref()) });
        }
    } else {
        for e in s.entities(partial) {
            b.entity(&e);
        }
    }
    if let Some(m) = look.marker {
        b.marker(m, true, [255, 255, 255], look.lit_marker && s.output_on().unwrap_or(false));
    }
    for o in &look.outlines {
        b.marker(*o, false, [110, 230, 40], false);
    }
    let mut m = b.mesh;
    if let Some(y) = look.ground {
        // the pictures' sandbox floor: grass around the batch of clocks (`clock shot`: 24 blocks
        // around the batch, whose other clocks push it further), so the sky shows beyond its edge
        // like in the real-game pictures (48: the best mean over `web compare`'s pictures; a
        // travelling design's whole path)
        let (lo, hi) = if s.corridor.is_some() { s.path_box() } else { s.frame_box() };
        let g = mesh::ground(atlas, [lo[0] as f32, 0.0, lo[2] as f32], [hi[0] as f32 + 1.0, 0.0, hi[2] as f32 + 1.0], y, 48.0);
        m.solid.extend_from_slice(&g);
    }
    m
}

/// `Rotation:[yaw f, pitch f]` in an entity's SNBT.
pub fn entity_yaw(nbt: Option<&str>) -> f32 {
    let Some(s) = nbt else { return 0.0 };
    let Some(i) = s.find("Rotation:[") else { return 0.0 };
    let rest = &s[i + "Rotation:[".len()..];
    let num: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-').collect();
    num.parse().unwrap_or(0.0)
}

/// The box the default camera frames: the design (and marker), 0.25 margin like the pictures.
pub fn frame_bounds(s: &Session, look: &Look) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = s.frame_box();
    if let Some(m) = look.marker {
        for k in 0..3 {
            lo[k] = lo[k].min(m[k]);
            hi[k] = hi[k].max(m[k]);
        }
    }
    ([lo[0] as f32 - 0.25, lo[1] as f32 - 0.25, lo[2] as f32 - 0.25], [hi[0] as f32 + 1.25, hi[1] as f32 + 1.25, hi[2] as f32 + 1.25])
}

/// The real-game pictures' camera for a session at an aspect ratio: `mctas clock shot` frames the
/// box at a vertical FOV of 70 degrees, then keeps the centered 64% of the frame (its ffmpeg
/// `crop=iw*0.64:ih*0.64`): the same eye with a narrower view. Zoomed out when a block would leave
/// the view.
pub fn default_camera(s: &Session, look: &Look, aspect: f32) -> Camera {
    camera_from(s, look, aspect, CAM_DIR)
}

/// The pictures' framing from direction `dir` (eye side, from the target): `clock shot`'s camera
/// exactly (`video::frame_box`: the box at 85% of the game's full 70-degree frame), then its
/// centre crop (64%): a build may run off the picture's edges like in the real-game pictures.
pub fn camera_from(s: &Session, look: &Look, aspect: f32, dir: [f32; 3]) -> Camera {
    let (lo, hi) = frame_bounds(s, look);
    let mut cam = Camera::frame(lo, hi, dir, VFOV, aspect, 0.85);
    cam.vfov = 2.0 * (0.64 * (VFOV.to_radians() / 2.0).tan()).atan().to_degrees();
    cam
}

/// Solid and translucent triangles of a mesh for a camera.
pub fn triangles(m: &Mesh, cam: &Camera) -> (Vec<Vertex>, Vec<Vertex>) {
    (m.solid.clone(), m.sorted_translucent(cam.eye()))
}

/// Where on the view (x, y in -1..1, y up) a click picks `cell`: the first of a few points of the
/// cell that the pick ray reaches (None: hidden or out of view).
pub fn click_point(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    let blocks = s.render_blocks(0.0);
    let steps = [0.5f32, 0.3, 0.7, 0.15, 0.85];
    for &dy in &steps {
        for &dx in &steps {
            for &dz in &steps {
                let p = [cell[0] as f32 + dx, cell[1] as f32 + dy, cell[2] as f32 + dz];
                let v = [p[0] - eye[0], p[1] - eye[1], p[2] - eye[2]];
                let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
                let z = dot(f);
                if z <= 0.01 {
                    continue;
                }
                let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
                if x.abs() >= 0.98 || y.abs() >= 0.98 {
                    continue;
                }
                let (o, d) = cam.ray(x, y, aspect);
                if crate::pick::pick(atlas, &blocks, o, d, cam.dist * 4.0 + 50.0) == Some(cell) {
                    return Some((x, y));
                }
            }
        }
    }
    None
}

/// The pictures' camera, turned to another diagonal side when that is the one a person can see
/// (and click) every lever and button from: a door's lever is often behind the build from the
/// pictures' side.
pub fn camera_showing_inputs(atlas: &Atlas, s: &Session, look: &Look, aspect: f32) -> Camera {
    let base = default_camera(s, look, aspect);
    if s.inputs.is_empty() || !s.simulated() {
        return base;
    }
    for (sx, sz) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
        let cam = if (sx, sz) == (1.0, 1.0) { base } else { camera_from(s, look, aspect, [CAM_DIR[0] * sx, CAM_DIR[1], CAM_DIR[2] * sz]) };
        if s.inputs.iter().all(|c| click_point(atlas, s, &cam, *c, aspect).is_some()) {
            return cam;
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Options;

    const OBS2: &str = "name: c\norigin: 0 0 0\nport output: 0 0 0\nport start: 1 0 0 last\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  B minecraft:observer[facing=west,powered=false]\nlayer 0:\n  AB\n";

    #[test]
    fn the_marker_never_shares_a_face_with_a_block() {
        let atlas = Atlas::build();
        let s = Session::new(OBS2, &Options::default()).unwrap();
        let look = default_look(&s);
        assert_eq!(look.marker, Some([-1, 0, 0]));
        let m = build(&atlas, &s, 0.0, &look);
        // the marker is the only translucent block: its faces against the observer and the
        // ground are culled (they would z-fight), the other 4 remain
        assert_eq!(m.translucent.len(), 4);
        assert!(m.translucent.iter().all(|q| q.center[0] < -0.01 && q.center[1] > 0.01));
    }
}
