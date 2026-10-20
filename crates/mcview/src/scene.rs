//! A session as a picture: its blocks (moving ones mid-way), the output marker (orange stained
//! glass, the real-game pictures' marker), outlines on the inputs, the design's entities, the
//! grass ground; and the default camera framing it like `mctas clock shot`.

use crate::atlas::Atlas;
use crate::camera::{CAM_DIR, Camera, VFOV};
use crate::design::Pos;
use crate::mesh::{self, Builder, EntityDraw, Item, Mesh, Vertex};
use crate::session::{Session, bp};
use redsim::Block;

/// How far the viewer's grass reaches around the build: past the browser viewer's render
/// distance (24 chunks, 384 blocks), so its fogged end sits at the horizon and no edge shows.
pub const GROUND_REACH: f32 = 420.0;
/// The real-game pictures' grass reach around a clock (`web compare`).
pub const PICTURE_GROUND_REACH: f32 = 56.0;

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
    /// A driven input's lime box (R7): its cell and the judge's devices that drive it (the
    /// repeater, its support and the switch), one box (low, high corner cells): the judge's
    /// equipment reads as part of the input it drives.
    pub input_spans: Vec<(Pos, Pos)>,
    /// A circuit's other ports, outlined in their tag's colour: OUT orange, a shulker design's
    /// ITEMS gold and BOXES lilac chests, a BUD switch's TARGET cyan.
    pub port_outlines: Vec<(Pos, [u8; 3])>,
    /// The ground's top (None: no ground).
    pub ground: Option<f32>,
    /// How far the ground reaches around the build (None: 110 around a door, 56 otherwise).
    pub floor_margin: Option<f32>,
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
    // a circuit output holding the judge's lamp shows itself (lit when on), and a shulker
    // design's output holds the judge's chest: no marker there
    let shulker = crate::circuit::is_shulker(&s.design);
    let marker = s.outputs.first().filter(|o| !s.consumers.contains(o) && !shulker).and_then(|o| marker_cell(s, *o));
    let mut outlines: Vec<Pos> = Vec::new();
    // a design shown as built has nothing to press: no outlines (a recording plays by itself)
    if s.simulated() {
        for p in &s.inputs {
            if !outlines.contains(p) {
                outlines.push(*p);
            }
        }
    }
    // a circuit's other ports (shown as built too: nothing to press, but where they are)
    let hex = |c: &str| -> [u8; 3] {
        let v = u32::from_str_radix(c.trim_start_matches('#'), 16).unwrap_or(0xffffff);
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    let port_outlines: Vec<(Pos, [u8; 3])> = crate::circuit::port_tags(&s.design).into_iter().filter(|(_, p, _)| !outlines.contains(p)).map(|(_, p, c)| (p, hex(c))).collect();
    // a driven input (R7): one lime box around its cell and the judge's devices driving it
    let mut input_spans: Vec<(Pos, Pos)> = Vec::new();
    if s.simulated() {
        for ci in &s.circuit_inputs {
            if let crate::circuit::InputHow::Driven { dir, .. } = ci.how {
                // the input, its repeater and the switch in a row (the repeater's stone support
                // under it stays plain)
                let (r, _, sw) = crate::circuit::driver_cells(ci.pos, dir);
                let cells = [ci.pos, r, sw];
                let lo = std::array::from_fn(|k| cells.iter().map(|c| c[k]).min().unwrap_or(0));
                let hi = std::array::from_fn(|k| cells.iter().map(|c| c[k]).max().unwrap_or(0));
                input_spans.push((lo, hi));
                outlines.retain(|p| *p != ci.pos);
            }
        }
    }
    let (lo, _) = s.path_box();
    // the pictures' floor: `clock shot` fills grass at the box's lowest y - 2, its top one block
    // under the lowest cell
    Look { marker, lit_marker: false, outlines, input_spans, port_outlines, ground: Some(lo[1] as f32 - 1.0), floor_margin: None }
}

/// An OUT box's outline colour (the OUT tag's), and while its output is powered.
pub const OUT_COLOUR: [u8; 3] = [255, 176, 74];
pub const OUT_POWERED: [u8; 3] = [255, 59, 48];

/// The viewer's look (live viewers, thumbnails): [`default_look`], with a clock's output in the
/// circuits' OUT box (vjeux: red while powered) instead of the real-game pictures' orange glass
/// marker (kept for `web compare`).
pub fn viewer_look(s: &Session) -> Look {
    let mut look = default_look(s);
    // no orange glass anywhere in the viewer: an output has its OUT box (a circuit's from its
    // port tags, a clock's added here)
    look.marker = None;
    if !s.design.meta.contains_key("circuit")
        && !s.mode.is_door()
        && let Some(o) = s.outputs.first()
    {
        if !look.port_outlines.iter().any(|(p, _)| p == o) {
            look.port_outlines.push((*o, OUT_COLOUR));
        }
    }
    look
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
            b.entity(&EntityDraw { kind: e.kind.clone(), pos: [e.pos[0] as f32, e.pos[1] as f32, e.pos[2] as f32], yaw: entity_yaw(e.nbt.as_deref()), ..Default::default() });
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
    for (lo, hi) in &look.input_spans {
        b.marker_span(*lo, *hi, [110, 230, 40]);
    }
    for (o, c) in &look.port_outlines {
        // an output's box: red while it is powered (redsim's state this tick)
        if s.outputs.contains(o) && s.output_powered(*o) {
            b.marker(*o, false, OUT_POWERED, false);
            b.out_fill(*o);
        } else {
            b.marker(*o, false, *c, false);
        }
    }
    let occ = b.opaque_cells().clone();
    let mut m = b.mesh;
    if let Some(y) = look.ground {
        // the floor: past the fog's end in the viewer (no edge ever shows: the far grass fades into
        // the sky like the game's render distance); `web compare` passes the real-game pictures'
        // own reach (`clock shot` fills grass around a batch of clocks: 56 is the best mean over
        // its pictures, their sky shows beyond the edge); a travelling design's whole path
        let (lo, hi) = if s.corridor.is_some() { s.path_box() } else { s.frame_box() };
        let margin = look.floor_margin.unwrap_or(GROUND_REACH);
        let g = mesh::ground(atlas, [lo[0] as f32, 0.0, lo[2] as f32], [hi[0] as f32 + 1.0, 0.0, hi[2] as f32 + 1.0], y, margin, &occ);
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
/// `crop=iw*0.64:ih*0.64`): the same eye with a narrower view; a build may run off its edges.
/// Only `web compare` (and `web fit`) use it, to compare with those pictures; the viewer frames
/// with [`default_camera`].
pub fn picture_camera(s: &Session, look: &Look, aspect: f32) -> Camera {
    camera_from(s, look, aspect, CAM_DIR)
}

/// Where a live viewer's build goes: x within +-0.8 of the view (80% of its width), y between
/// the bar (bottom, 60 of 480 px) and the badges (top), with room above for the tags. A viewer
/// of known height uses [`live_window`].
pub const LIVE_WINDOW: (f32, f32, f32) = (0.8, -0.7, 0.62);
/// A thumbnail's: 80% of each side, centred.
pub const THUMB_WINDOW: (f32, f32, f32) = (0.8, -0.8, 0.8);

/// [`LIVE_WINDOW`] for a viewer `h` CSS px tall: clear of its bar (60 px and a margin) and its
/// badges and tags (44 px) whatever its height (a short phone viewer: the bar takes more of it).
pub fn live_window(h: f32) -> (f32, f32, f32) {
    let h = h.max(120.0);
    let y0 = -1.0 + 2.0 * 74.0 / h;
    let y1 = (1.0 - 2.0 * 46.0 / h).max(y0 + 0.4);
    (0.8, y0, y1)
}

/// The smallest build a view frames, in blocks across (x, z) and high: a tiny one (a 2-block
/// clock) stands in a little grass instead of filling the view edge to edge.
const MIN_VIEW_W: f32 = 3.0;
const MIN_VIEW_H: f32 = 2.0;

/// A long thin build (its plan this many times longer than wide) is seen at 45 degrees along its
/// length (a 3/4 view) instead of side-on: its length foreshortened, the camera closer.
const LONG_BUILD: f32 = 3.0;

/// The direction a view looks from: the pictures' ([`CAM_DIR`]), or for a long thin build the
/// same elevation at 45 degrees in plan (from the same quarter).
pub fn view_dir(pts: &[[f32; 3]]) -> [f32; 3] {
    if pts.is_empty() {
        return CAM_DIR;
    }
    let ext = |k: usize| pts.iter().map(|p| p[k]).fold(f32::MIN, f32::max) - pts.iter().map(|p| p[k]).fold(f32::MAX, f32::min);
    let (wx, wz) = (ext(0), ext(2));
    if wx.max(wz) < LONG_BUILD * wx.min(wz).max(1.0) {
        return CAM_DIR;
    }
    let h = (CAM_DIR[0] * CAM_DIR[0] + CAM_DIR[2] * CAM_DIR[2]).sqrt() / std::f32::consts::SQRT_2;
    [-h, CAM_DIR[1], h]
}

/// The viewer's camera (live viewers and thumbnails alike): from the pictures' direction, the
/// build's whole moving extent ([`view_points`]) filling `window`, never cut.
pub fn default_camera(s: &Session, look: &Look, aspect: f32) -> Camera {
    let pts = view_points(s, look);
    view_camera_at(s, look, &pts, aspect, view_dir(&pts), LIVE_WINDOW)
}

/// The points a view keeps in frame: the corners of every cell the build occupies or sweeps (its
/// blocks now and as placed, a door's opening, a clock's swept box over a period, a piston
/// extender's travel, its ports, inputs and marker, its entities); not its bounding box (a
/// sparse build would come out small).
pub fn view_points(s: &Session, look: &Look) -> Vec<[f32; 3]> {
    points(s, look, true)
}

/// [`view_points`] of a still (a thumbnail): what the build occupies now and as placed, without
/// the travel ahead (a long extender's path, where redsim's run takes its entities).
pub fn still_points(s: &Session, look: &Look) -> Vec<[f32; 3]> {
    points(s, look, false)
}

fn points(s: &Session, look: &Look, moving: bool) -> Vec<[f32; 3]> {
    let mut cells: std::collections::BTreeSet<Pos> = std::collections::BTreeSet::new();
    for b in s.render_blocks(0.0) {
        cells.insert(b.cell);
    }
    cells.extend(s.design.blocks.keys().copied());
    cells.extend(s.inputs.iter().copied());
    cells.extend(look.marker);
    cells.extend(crate::circuit::port_tags(&s.design).into_iter().map(|(_, p, _)| p));
    if let crate::session::Mode::Door { door, along_z, .. } = s.mode {
        cells.extend(crate::session::door_cells(door, along_z));
    }
    let mut boxes: Vec<(Pos, Pos)> = Vec::new();
    if let Some(b) = s.picture {
        boxes.push(b);
    }
    // a design that travels (a piston extender's payload): its whole swept path (without the
    // corridor's free cells; a still: not); a flying machine is followed instead
    let flies = s.design.meta.get("circuit").is_some_and(|k| k.starts_with("flying-machine"));
    if moving
        && s.corridor.is_some()
        && !flies
        && let Some((b, _)) = crate::session::travel_box(&s.design, &s.mode)
    {
        boxes.push(b);
    }
    let mut pts: Vec<[f32; 3]> = Vec::new();
    for c in &cells {
        for k in 0..8 {
            pts.push([c[0] as f32 + (k & 1) as f32, c[1] as f32 + ((k >> 1) & 1) as f32, c[2] as f32 + ((k >> 2) & 1) as f32]);
        }
    }
    for (lo, hi) in boxes {
        for k in 0..8 {
            pts.push([if k & 1 == 0 { lo[0] } else { hi[0] + 1 } as f32, if k & 2 == 0 { lo[1] } else { hi[1] + 1 } as f32, if k & 4 == 0 { lo[2] } else { hi[2] + 1 } as f32]);
        }
    }
    let ents = if s.simulated() { s.entities(0.0) } else { Vec::new() };
    let mut ent_pos: Vec<[f32; 3]> = ents.iter().map(|e| e.pos).chain(s.design.entities.iter().map(|e| [e.pos[0] as f32, e.pos[1] as f32, e.pos[2] as f32])).collect();
    // a design with entities or TNT: where they go in redsim's run (a launched TNT's flight, a
    // cart's loop), within VIEW_REACH blocks of the build (one flung far away leaves the view)
    if moving && (!s.design.entities.is_empty() || crate::session::is_tnt_design(&s.design)) {
        let lo = pts.iter().fold([f32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
        let hi = pts.iter().fold([f32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
        let near = |p: &[f32; 3]| (0..3).all(|k| p[k] >= lo[k] - VIEW_REACH && p[k] <= hi[k] + VIEW_REACH);
        ent_pos.extend(s.run_extent(VIEW_RUN).into_iter().filter(near));
    }
    for e in ent_pos {
        for k in 0..8 {
            pts.push([e[0] + if k & 1 == 0 { -0.7 } else { 0.7 }, e[1] + if k & 2 == 0 { 0.0 } else { 1.0 }, e[2] + if k & 4 == 0 { -0.7 } else { 0.7 }]);
        }
    }
    // a tiny build (smaller than MIN_VIEW_W in every direction): framed as if MIN_VIEW_W blocks
    // across and MIN_VIEW_H high, around its middle (a thin tower is not tiny: no padding)
    if !pts.is_empty() {
        let lo = pts.iter().fold([f32::MAX; 3], |a, p| [a[0].min(p[0]), a[1].min(p[1]), a[2].min(p[2])]);
        let hi = pts.iter().fold([f32::MIN; 3], |a, p| [a[0].max(p[0]), a[1].max(p[1]), a[2].max(p[2])]);
        if (0..3).all(|k| hi[k] - lo[k] < MIN_VIEW_W) {
            let half = |k: usize, min: f32| ((hi[k] - lo[k]).max(min)) / 2.0;
            let (hx, hy, hz) = (half(0, MIN_VIEW_W), half(1, MIN_VIEW_H), half(2, MIN_VIEW_W));
            // around the build's middle, up and down alike (it sits mid-frame, not low)
            let c = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
            for k in 0..8 {
                pts.push([c[0] + if k & 1 == 0 { -hx } else { hx }, c[1] + if k & 2 == 0 { -hy } else { hy }, c[2] + if k & 4 == 0 { -hz } else { hz }]);
            }
        }
    }
    pts
}

/// How many ticks of redsim's run frame a design's moving entities ([`view_points`]).
const VIEW_RUN: u32 = 200;
/// How far beyond the build those entities stay framed.
const VIEW_REACH: f32 = 8.0;

/// The viewer's framing from direction `dir`: [`view_points`] filling `window` (see
/// [`LIVE_WINDOW`]) at the pictures' cropped field of view.
pub fn view_camera(s: &Session, look: &Look, aspect: f32, dir: [f32; 3], window: (f32, f32, f32)) -> Camera {
    view_camera_at(s, look, &view_points(s, look), aspect, dir, window)
}

/// [`view_camera`] with the points already known.
fn view_camera_at(s: &Session, look: &Look, pts: &[[f32; 3]], aspect: f32, dir: [f32; 3], window: (f32, f32, f32)) -> Camera {
    let (lo, hi) = frame_bounds(s, look);
    let crop = 2.0 * (0.64 * (VFOV.to_radians() / 2.0).tan()).atan().to_degrees();
    let mut cam = Camera::frame(lo, hi, dir, crop, aspect, 0.85);
    cam.fit_points(pts, aspect, window.0, window.1, window.2);
    cam
}

/// The pictures' framing from direction `dir` (eye side, from the target): `clock shot`'s camera
/// exactly (`video::frame_box`: the box at 85% of the game's full 70-degree frame), then its
/// centre crop (64%): a build may run off the picture's edges like in the real-game pictures.
pub fn camera_from(s: &Session, look: &Look, aspect: f32, dir: [f32; 3]) -> Camera {
    let (lo, hi) = frame_bounds(s, look);
    let crop = 2.0 * (0.64 * (VFOV.to_radians() / 2.0).tan()).atan().to_degrees();
    if s.design.meta.contains_key("circuit") {
        // a circuit: the whole build in view (no real-game picture to match; a tall BUD switch
        // would run off the cropped frame)
        let mut cam = Camera::frame(lo, hi, dir, crop, aspect, 0.85);
        cam.vfov = crop;
        return cam;
    }
    let mut cam = Camera::frame(lo, hi, dir, VFOV, aspect, 0.85);
    cam.vfov = crop;
    cam
}

/// Solid and translucent triangles of a mesh for a camera.
pub fn triangles(m: &Mesh, cam: &Camera) -> (Vec<Vertex>, Vec<Vertex>) {
    (m.solid.clone(), m.sorted_translucent(cam.eye()))
}

/// Where on the view (x, y in -1..1, y up) a click picks `cell`: the first of a few points of the
/// cell that the pick ray reaches (None: hidden or out of view).
pub fn click_point(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
    click_point_at(atlas, s, cam, cell, aspect, &[0.5, 0.3, 0.7, 0.15, 0.85])
}

/// [`click_point`] at the cell's centre only: where a tap on it (and a test's click) lands.
pub fn centre_point(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
    click_point_at(atlas, s, cam, cell, aspect, &[0.5])
}

fn click_point_at(atlas: &Atlas, s: &Session, cam: &Camera, cell: Pos, aspect: f32, steps: &[f32]) -> Option<(f32, f32)> {
    let (f, r, u) = cam.basis();
    let eye = cam.eye();
    let t = (cam.vfov.to_radians() / 2.0).tan();
    let blocks = s.render_blocks(0.0);
    let empty = !blocks.iter().any(|b| b.cell == cell);
    for &dy in steps {
        for &dx in steps {
            for &dz in steps {
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
                let picked = crate::pick::pick(atlas, &blocks, o, d, cam.dist * 4.0 + 50.0);
                if picked == Some(cell) {
                    return Some((x, y));
                }
                // an empty cell (an input's cell with nothing in it): reached before any block,
                // the rule of `Player::input_at`, a few pixels around too (not a sliver)
                if empty {
                    let reached = |x: f32, y: f32| -> bool {
                        let (o, d) = cam.ray(x, y, aspect);
                        let picked = crate::pick::pick(atlas, &blocks, o, d, cam.dist * 4.0 + 50.0);
                        cell_entry(o, d, cell).is_some_and(|t| t <= picked.and_then(|p| cell_entry(o, d, p)).unwrap_or(f32::MAX) + 1e-3)
                    };
                    let e = 0.008;
                    if [(0.0, 0.0), (e, 0.0), (-e, 0.0), (0.0, e), (0.0, -e)].iter().all(|(a, b)| reached(x + a, y + b)) {
                        return Some((x, y));
                    }
                }
            }
        }
    }
    None
}

/// The pictures' camera, turned to another diagonal side when that is the one a person can see
/// (and click) every lever and button from: a door's lever is often behind the build from the
/// pictures' side.
pub fn camera_showing_inputs(atlas: &Atlas, s: &Session, look: &Look, aspect: f32, window: (f32, f32, f32)) -> Camera {
    camera_showing_inputs_of(atlas, s, look, aspect, window, &view_points(s, look))
}

/// A thumbnail's camera: [`camera_showing_inputs`] of the still ([`still_points`]) in
/// [`THUMB_WINDOW`].
pub fn thumb_camera(atlas: &Atlas, s: &Session, look: &Look, aspect: f32) -> Camera {
    camera_showing_inputs_of(atlas, s, look, aspect, THUMB_WINDOW, &still_points(s, look))
}

fn camera_showing_inputs_of(atlas: &Atlas, s: &Session, look: &Look, aspect: f32, window: (f32, f32, f32), pts: &[[f32; 3]]) -> Camera {
    let pts = pts.to_vec();
    // the pictures' direction, or a 3/4 view along a long thin build
    let dir = view_dir(&pts);
    let base = view_camera_at(s, look, &pts, aspect, dir, window);
    if s.inputs.is_empty() || !s.simulated() {
        return base;
    }
    // a side every input's centre can be tapped from, else one every input can be tapped from
    for centre in [true, false] {
        for (sx, sz) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            let cam = if (sx, sz) == (1.0, 1.0) { base } else { view_camera_at(s, look, &pts, aspect, [dir[0] * sx, dir[1], dir[2] * sz], window) };
            if s.inputs.iter().all(|c| if centre { centre_point(atlas, s, &cam, *c, aspect).is_some() } else { click_point(atlas, s, &cam, *c, aspect).is_some() }) {
                return cam;
            }
        }
    }
    base
}

/// Where a ray (origin, direction) enters a cell's box, if it does (the slab test).
fn cell_entry(o: [f32; 3], d: [f32; 3], c: Pos) -> Option<f32> {
    let (mut t0, mut t1) = (0.0f32, f32::MAX);
    for i in 0..3 {
        let (lo, hi) = (c[i] as f32, c[i] as f32 + 1.0);
        if d[i].abs() < 1e-9 {
            if o[i] < lo || o[i] > hi {
                return None;
            }
            continue;
        }
        let (a, b) = ((lo - o[i]) / d[i], (hi - o[i]) / d[i]);
        t0 = t0.max(a.min(b));
        t1 = t1.min(a.max(b));
        if t0 > t1 {
            return None;
        }
    }
    Some(t0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Options;

    /// An input whose centre hides behind another block from the pictures' side (here A behind
    /// the judge's lamp in the output cell in front of it) makes the camera start on a side every
    /// input's centre can be tapped from.
    #[test]
    fn empty_input_cells_are_tappable() {
        let text = "name: t\ncircuit: gate and\nport input.a: 0 1 0\nport input.b: 1 1 0\nport output: 0 1 1\npalette:\n  S minecraft:stone\nlayer 0:\n  SS\n  S.\n";
        let s = Session::new(text, &Options::default()).unwrap();
        let atlas = crate::atlas::Atlas::build();
        let look = viewer_look(&s);
        let aspect = 1217.0 / 480.0;
        let cam = camera_showing_inputs(&atlas, &s, &look, aspect, LIVE_WINDOW);
        assert!(s.inputs.iter().all(|c| centre_point(&atlas, &s, &cam, *c, aspect).is_some()), "every input's centre tappable from {cam:?}");
    }

    /// Every circuit port is outlined like its tag: the inputs lime (when the design runs), the
    /// outputs orange, a shulker design's ITEMS gold and BOXES lilac chests, a BUD switch's TARGET
    /// cyan; for every archived shulker, TNT and BUD design.
    #[test]
    fn every_port_is_outlined_like_its_tag() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../designs/circuits");
        let mut n = 0;
        for kind in ["shulker", "tnt", "bud"] {
            let mut files: Vec<_> = std::fs::read_dir(format!("{root}/{kind}/archive")).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
            files.sort();
            for f in files {
                let s = Session::new(&std::fs::read_to_string(&f).unwrap(), &Options::default()).unwrap();
                let look = default_look(&s);
                for (label, p, colour) in crate::circuit::port_tags(&s.design) {
                    let found = look.port_outlines.iter().find(|(q, _)| *q == p).map(|(_, c)| *c);
                    let v = u32::from_str_radix(colour.trim_start_matches('#'), 16).unwrap();
                    assert_eq!(found, Some([(v >> 16) as u8, (v >> 8) as u8, v as u8]), "{}: {label} at {p:?}", f.display());
                }
                for ci in &s.circuit_inputs {
                    // an input outlined (a driven one: in its span with the judge's devices)
                    let spanned = look.input_spans.iter().any(|(lo, hi)| (0..3).all(|k| ci.pos[k] >= lo[k] && ci.pos[k] <= hi[k]));
                    assert_eq!(look.outlines.contains(&ci.pos) || spanned, s.simulated(), "{}: input {} at {:?}", f.display(), ci.name, ci.pos);
                }
                n += 1;
            }
        }
        assert!(n >= 20, "{n} designs");
    }

    const OBS2: &str = "name: c\norigin: 0 0 0\nport output: 0 0 0\nport start: 1 0 0 last\npalette:\n  A minecraft:observer[facing=east,powered=false]\n  B minecraft:observer[facing=west,powered=false]\nlayer 0:\n  AB\n";

    #[test]
    fn the_marker_never_shares_a_face_with_a_block() {
        let atlas = Atlas::build();
        let s = Session::new(OBS2, &Options::default()).unwrap();
        let look = default_look(&s);
        assert_eq!(look.marker, Some([-1, 0, 0]));
        let m = build(&atlas, &s, 0.0, &look);
        // the marker is the only translucent block: its face against the observer is culled (it
        // would z-fight); the floor is one block lower (the pictures' floor), so 5 faces remain
        assert_eq!(m.translucent.len(), 5);
        assert!(m.translucent.iter().all(|q| q.center[0] < -0.01));
    }
}
