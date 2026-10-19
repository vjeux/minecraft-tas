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
        if !self.playing || !self.session.simulated() {
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
        self.dirty = true;
    }

    pub fn reset(&mut self) {
        self.session.reset();
        self.acc = 0.0;
        self.partial = 0.0;
        self.dirty = true;
    }

    /// The block under a point of the view (x, y in -1..1, y up).
    pub fn pick(&self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        let (o, d) = self.camera.ray(x, y, aspect);
        let blocks = self.session.render_blocks(0.0);
        crate::pick::pick(atlas, &blocks, o, d, self.camera.dist * 4.0 + 50.0)
    }

    /// A click: flips the lever / presses the button under the pointer. Returns the cell.
    pub fn click(&mut self, atlas: &Atlas, x: f32, y: f32, aspect: f32) -> Option<Pos> {
        let c = self.pick(atlas, x, y, aspect)?;
        if self.session.toggle(c) {
            self.dirty = true;
            Some(c)
        } else {
            None
        }
    }

    /// Flip a door's lever (or the first input).
    pub fn flip(&mut self) -> bool {
        let target = match self.session.mode {
            Mode::Door { lever, .. } => Some(lever),
            _ => self.session.inputs.first().copied(),
        };
        let ok = target.is_some_and(|p| self.session.toggle(p));
        self.dirty |= ok;
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

    /// The tick counter: `123 gt (6.15 s)`, plus the ticks since the last flip for a door.
    pub fn label(&self) -> String {
        let s = &self.session;
        let t = s.tick.saturating_sub(s.rest_tick);
        let mut out = format!("{t} gt ({})", seconds(t));
        if s.last_input.is_some() {
            out.push_str(&format!(" · {} gt since the flip", s.since_input()));
        }
        out
    }
}

/// `6.15 s`, `1 min 2.5 s`.
pub fn seconds(gt: u32) -> String {
    let s = gt as f32 / 20.0;
    if s < 60.0 {
        let t = format!("{s:.2}");
        let t = t.trim_end_matches('0').trim_end_matches('.');
        format!("{t} s")
    } else {
        let m = (s / 60.0).floor();
        format!("{m} min {:.1} s", s - m * 60.0)
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
        assert_eq!(seconds(20), "1 s");
    }
}
