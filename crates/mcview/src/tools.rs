//! Native tools behind `mctas web shot` / `mctas web check`: render a design to PNG offscreen
//! (the same meshes and shaders as the browser), and the clock-period comparison.

use crate::atlas::Atlas;
use crate::render::Frame;
use crate::render::headless::Offscreen;
use crate::scene;
use crate::session::{Options, Session};

/// Knobs of [`shot`].
#[derive(Clone, Debug)]
pub struct ShotOptions {
    pub width: u32,
    pub height: u32,
    /// Run this many ticks after placement (clocks) / after rest (doors).
    pub tick: Option<u32>,
    /// Lever flips before the picture (doors: 1 = open), each run until idle.
    pub flips: u32,
    /// Camera yaw / pitch in degrees (default: the real-game pictures' direction).
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
}

impl Default for ShotOptions {
    fn default() -> Self {
        ShotOptions { width: 800, height: 450, tick: None, flips: 0, yaw: None, pitch: None }
    }
}

/// Render a design: RGBA pixels and a one-line description of what was shown.
pub fn shot(off: &mut Offscreen, atlas: &Atlas, text: &str, o: &ShotOptions) -> Result<(Vec<u8>, String), String> {
    let mut s = Session::new(text, &Options::default())?;
    let look = scene::default_look(&s);
    let what = match o.tick {
        Some(t) => {
            s.run(t);
            format!("tick {}", s.tick)
        }
        None if s.mode.is_door() => "rest".to_string(),
        None => format!("still tick {}", s.go_to_still(20_000)),
    };
    for _ in 0..o.flips {
        if let crate::session::Mode::Door { lever, .. } = s.mode {
            s.toggle(lever);
            let mut n = 0;
            while !s.idle() && n < 400 {
                s.step();
                n += 1;
            }
        }
    }
    let aspect = o.width as f32 / o.height as f32;
    let mut cam = scene::default_camera(&s, &look, aspect);
    if o.yaw.is_some() || o.pitch.is_some() {
        if let Some(y) = o.yaw {
            cam.yaw = y.to_radians();
        }
        if let Some(p) = o.pitch {
            cam.pitch = p.to_radians();
        }
        let (lo, hi) = scene::frame_bounds(&s, &look);
        cam.fit(lo, hi, aspect, 0.85);
    }
    let mesh = scene::build(atlas, &s, 0.0, &look);
    let (solid, translucent) = scene::triangles(&mesh, &cam);
    let rgba = off.render(&Frame { solid: &solid, translucent: &translucent, camera: cam, width: o.width, height: o.height })?;
    let note = match &s.unsimulated {
        Some(why) => format!("{what} (not simulated: {why})"),
        None => what,
    };
    Ok((rgba, note))
}
