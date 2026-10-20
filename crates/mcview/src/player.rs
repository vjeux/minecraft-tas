//! The live viewer's state, independent of the platform: a session played at a speed (ticks per
//! second) with the pointer's inputs (orbit, zoom, pan, click to flip a lever), the frame's
//! triangles rebuilt only when something moved.

use crate::atlas::Atlas;
use crate::camera::Camera;
use crate::design::Pos;
use crate::mesh::{Mesh, Vertex};
use crate::scene::{self, Look};
use crate::session::{Mode, Options, Session};

pub struct Player {
    pub session: Session,
    pub look: Look,
    pub camera: Camera,
    /// The camera as framed at load (double click / reset view).
    pub home: Camera,
    pub playing: bool,
    /// Game ticks per second (20 = real time).
    pub tps: f32,
    acc: f32,
    /// Fraction of the next tick shown (moving blocks interpolated; 0 when paused).
    pub partial: f32,
    mesh: Option<Mesh>,
    mesh_key: (u32, u32),
    /// Something changed since the last frame.
    pub dirty: bool,
}

impl Player {
    pub fn new(text: &str, aspect: f32) -> Result<Player, String> {
        let mut session = Session::new(text, &Options::default())?;
        session.find_auto_output(20_000);
        let look = scene::default_look(&session);
        let camera = scene::default_camera(&session, &look, aspect);
        Ok(Player { session, look, home: camera, camera, playing: false, tps: 20.0, acc: 0.0, partial: 0.0, mesh: None, mesh_key: (u32::MAX, 0), dirty: true })
    }

    /// Advance the clock by `dt` seconds; true when the picture changed.
    pub fn update(&mut self, dt: f32) -> bool {
        if !self.playing || !self.session.playable() {
            return std::mem::take(&mut self.dirty);
        }
        self.acc += dt.min(0.25) * self.tps;
        let mut n = 0;
        while self.acc >= 1.0 && n < 5000 {
            self.session.step();
            self.acc -= 1.0;
            n += 1;
        }
        if n >= 5000 {
            self.acc = 0.0;
        }
        // interpolate moving blocks only at watchable speeds
        self.partial = if self.tps <= 40.0 { self.acc.clamp(0.0, 0.999) } else { 0.0 };
        self.dirty = false;
        true
    }

    /// One tick (paused stepping).
    pub fn step(&mut self) {
        self.session.step();
        self.acc = 0.0;
        self.partial = 0.0;
        self.invalidate();
    }

    pub fn reset(&mut self) {
        self.session.reset();
        self.acc = 0.0;
        self.partial = 0.0;
        self.invalidate();
    }

    /// The block under a point of the view (x, y in -1..1, y up).
    pub fn pick(&self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        let (o, d) = self.camera.ray(x, y, aspect);
        let blocks = self.session.render_blocks(0.0);
        crate::pick::pick(atlas, &blocks, o, d, self.camera.dist * 4.0 + 50.0)
    }

    /// A click: flips the lever / presses the button under the pointer, paused or playing (the
    /// frame is rebuilt at once: the cached mesh is keyed by tick, which a click does not move).
    /// Returns the cell.
    pub fn click(&mut self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        let c = self.pick(atlas, x, y, aspect)?;
        if self.session.toggle(c) {
            self.invalidate();
            Some(c)
        } else {
            None
        }
    }

    /// Where on the view (x, y in -1..1, y up) a click picks `cell` (tests click there).
    pub fn screen_point(&self, atlas: &Atlas, cell: Pos, aspect: f32) -> Option<(f32, f32)> {
        scene::click_point(atlas, &self.session, &self.camera, cell, aspect)
    }

    /// Turn the camera (and its home) to a side every lever and button can be seen and clicked
    /// from, if the pictures' side hides one.
    pub fn show_inputs(&mut self, atlas: &Atlas, aspect: f32) {
        self.camera = scene::camera_showing_inputs(atlas, &self.session, &self.look, aspect);
        self.home = self.camera;
        self.dirty = true;
    }

    /// Flip a door's lever (or the first input).
    pub fn flip(&mut self) -> bool {
        let target = match self.session.mode {
            Mode::Door { lever, .. } => Some(lever),
            _ => self.session.inputs.first().copied(),
        };
        let ok = target.is_some_and(|p| self.session.toggle(p));
        if ok {
            self.invalidate();
        }
        ok
    }

    /// The frame's triangles (solid, translucent sorted for the camera).
    pub fn triangles(&mut self, atlas: &Atlas) -> (Vec<Vertex>, Vec<Vertex>) {
        let key = (self.session.tick, (self.partial * 64.0) as u32);
        if self.mesh.is_none() || key != self.mesh_key {
            self.mesh = Some(scene::build(atlas, &self.session, self.partial, &self.look));
            self.mesh_key = key;
        }
        scene::triangles(self.mesh.as_ref().unwrap(), &self.camera)
    }

    /// Forget the cached mesh (the look changed).
    pub fn invalidate(&mut self) {
        self.mesh = None;
        self.dirty = true;
    }

    /// After moving the camera: keep it above the ground (never under the floor).
    pub fn keep_camera_above_ground(&mut self) {
        if let Some(g) = self.look.ground {
            self.camera.keep_above(g);
        }
    }

    /// The tick counter: `123 gt · 6.15 s`, plus the ticks since the last flip for a door.
    pub fn label(&self) -> String {
        let s = &self.session;
        let t = s.tick.saturating_sub(s.rest_tick);
        let mut out = format!("{t} gt · {}", seconds(t));
        if s.last_input.is_some() {
            out.push_str(&format!(" · flip +{} gt", s.since_input()));
        }
        out
    }
}

/// `6.15 s`, `1 min 2.50 s` (always two decimals: the counter keeps its width).
pub fn seconds(gt: u32) -> String {
    let s = gt as f32 / 20.0;
    if s < 60.0 {
        format!("{s:.2} s")
    } else {
        let m = (s / 60.0).floor();
        format!("{m} min {:05.2} s", s - m * 60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plays_a_clock_and_flips_a_door() {
        let clock = "name: c\nport start: 0 0 0 last\npalette:\n  O minecraft:observer[facing=east,powered=false]\n  W minecraft:waxed_copper_bulb[lit=false,powered=false]\n  o minecraft:observer[facing=west,powered=false]\nlayer 0:\n  OoW\n";
        let mut p = Player::new(clock, 1.5).unwrap();
        p.playing = true;
        for _ in 0..4 {
            assert!(p.update(0.25));
        }
        assert_eq!(p.session.tick, 20);
        assert_eq!(seconds(123), "6.15 s");
        assert_eq!(seconds(20), "1.00 s");
        assert_eq!(seconds(1250), "1 min 02.50 s");
    }

    #[test]
    fn a_paused_click_shows_at_once() {
        // the lever flips while paused: the next frame shows it (no stale mesh of the same tick)
        let gadget = "name: g\norigin: 0 0 0\npalette:\n  S minecraft:stone\n  L minecraft:lever[face=floor,facing=east,powered=false]\n  B minecraft:waxed_copper_bulb[lit=false,powered=false]\nlayer 0:\n  SB\nlayer 1:\n  L\n";
        let atlas = Atlas::build();
        let mut p = Player::new(gadget, 1.5).unwrap();
        assert_eq!(p.session.inputs, vec![[0, 1, 0]]);
        let before = p.triangles(&atlas).0;
        let (x, y) = p.screen_point(&atlas, [0, 1, 0], 1.5).expect("the lever is visible");
        assert_eq!(p.click(&atlas, x, y, 1.5), Some([0, 1, 0]));
        assert!(p.update(0.0), "a paused click asks for a frame");
        let after = p.triangles(&atlas).0;
        assert!(before.len() != after.len() || before.iter().zip(&after).any(|(a, b)| a.pos != b.pos), "the frame changed");
        assert!(p.session.is_on([0, 1, 0]));
    }
}
