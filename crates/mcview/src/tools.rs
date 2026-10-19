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
    let what = match o.tick {
        Some(t) => {
            s.find_auto_output(20_000);
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
    let look = scene::default_look(&s);
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
        None => format!("{what}, output {:?}, marker {:?}", s.outputs.first(), look.marker),
    };
    Ok((rgba, note))
}

/// A real-game picture next to ours: (side-by-side RGBA, its width, height, scores). Scores: the
/// mean absolute RGB difference per pixel (0-255) and the same on a 32x18 grid of block means
/// (the composition: framing, ground, sky, where the build is).
pub struct Comparison {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub pixel_diff: f64,
    pub coarse_diff: f64,
    pub note: String,
}

pub fn compare(off: &mut Offscreen, atlas: &Atlas, text: &str, jpg: &[u8]) -> Result<Comparison, String> {
    let mut dec = jpeg_decoder::Decoder::new(jpg);
    let px = dec.decode().map_err(|e| format!("jpeg: {e}"))?;
    let info = dec.info().ok_or("jpeg: no info")?;
    let (w, h) = (info.width as u32, info.height as u32);
    let real: Vec<[u8; 3]> = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => px.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        jpeg_decoder::PixelFormat::L8 => px.iter().map(|v| [*v, *v, *v]).collect(),
        f => return Err(format!("jpeg: unsupported pixel format {f:?}")),
    };
    let (mine, note) = shot(off, atlas, text, &ShotOptions { width: w, height: h, ..Default::default() })?;
    let mut out = vec![0u8; (2 * w * h * 4) as usize];
    let mut diff = 0f64;
    for y in 0..h as usize {
        for x in 0..w as usize {
            let r = real[y * w as usize + x];
            let m = &mine[(y * w as usize + x) * 4..(y * w as usize + x) * 4 + 3];
            let o = (y * 2 * w as usize + x) * 4;
            out[o..o + 4].copy_from_slice(&[r[0], r[1], r[2], 255]);
            let o = (y * 2 * w as usize + w as usize + x) * 4;
            out[o..o + 4].copy_from_slice(&[m[0], m[1], m[2], 255]);
            diff += (0..3).map(|k| (r[k] as f64 - m[k] as f64).abs()).sum::<f64>() / 3.0;
        }
    }
    let (gw, gh) = (32usize, 18usize);
    let mut coarse = 0f64;
    for gy in 0..gh {
        for gx in 0..gw {
            let (x0, x1) = (gx * w as usize / gw, (gx + 1) * w as usize / gw);
            let (y0, y1) = (gy * h as usize / gh, (gy + 1) * h as usize / gh);
            let mut a = [0f64; 3];
            let mut b = [0f64; 3];
            let n = ((x1 - x0) * (y1 - y0)).max(1) as f64;
            for y in y0..y1 {
                for x in x0..x1 {
                    let r = real[y * w as usize + x];
                    for k in 0..3 {
                        a[k] += r[k] as f64 / n;
                        b[k] += mine[(y * w as usize + x) * 4 + k] as f64 / n;
                    }
                }
            }
            coarse += (0..3).map(|k| (a[k] - b[k]).abs()).sum::<f64>() / 3.0;
        }
    }
    Ok(Comparison { rgba: out, width: 2 * w, height: h, pixel_diff: diff / (w * h) as f64, coarse_diff: coarse / (gw * gh) as f64, note })
}
