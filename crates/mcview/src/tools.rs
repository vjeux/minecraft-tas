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
    /// As the game pictures it: without the viewer's own marks (the lime outlines on the inputs).
    pub picture: bool,
    /// The ground's reach around the build (default: the viewer's).
    pub floor_margin: Option<f32>,
}

impl Default for ShotOptions {
    fn default() -> Self {
        ShotOptions { width: 800, height: 450, tick: None, flips: 0, yaw: None, pitch: None, picture: false, floor_margin: None }
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
    let mut look = if o.picture { scene::default_look(&s) } else { scene::viewer_look(&s) };
    if o.picture {
        look.outlines.clear();
        look.port_outlines.clear();
    }
    // as the real-game pictures show it (`web compare`): their camera and their grass reach;
    // else the viewer's (a door: the side its lever is seen from)
    look.floor_margin = o.floor_margin.or(if o.picture { Some(scene::PICTURE_GROUND_REACH) } else { None });
    let mut cam = if o.picture {
        scene::picture_camera(&s, &look, aspect)
    } else {
        // the viewer's thumbnail camera (a door: the side its lever is seen from)
        scene::thumb_camera(atlas, &s, &look, aspect)
    };
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
    /// The pixel diff of the bottom eighth (usually grass only: the textures' noise floor) and of
    /// the top eighth (the horizon, the sky band).
    pub bottom_diff: f64,
    pub top_diff: f64,
    pub note: String,
}

pub fn compare(off: &mut Offscreen, atlas: &Atlas, text: &str, jpg: &[u8], floor_margin: Option<f32>) -> Result<Comparison, String> {
    let mut dec = jpeg_decoder::Decoder::new(jpg);
    let px = dec.decode().map_err(|e| format!("jpeg: {e}"))?;
    let info = dec.info().ok_or("jpeg: no info")?;
    let (w, h) = (info.width as u32, info.height as u32);
    let real: Vec<[u8; 3]> = match info.pixel_format {
        jpeg_decoder::PixelFormat::RGB24 => px.chunks(3).map(|c| [c[0], c[1], c[2]]).collect(),
        jpeg_decoder::PixelFormat::L8 => px.iter().map(|v| [*v, *v, *v]).collect(),
        f => return Err(format!("jpeg: unsupported pixel format {f:?}")),
    };
    // like the pictures: the game's frame (1920 x 1080) cropped to its centre 64% and scaled down
    // (lanczos) to the picture's width: rendered at the crop's size, then resampled the same way
    let (bw, bh) = ((w as f32 * 1228.8 / 800.0).round() as u32, (h as f32 * 1228.8 / 800.0).round() as u32);
    let (big, note) = shot(off, atlas, text, &ShotOptions { width: bw, height: bh, picture: true, floor_margin, ..Default::default() })?;
    let mine = lanczos_resize(&big, bw, bh, w, h);
    let mut out = vec![0u8; (2 * w * h * 4) as usize];
    let mut diff = 0f64;
    let (mut bottom, mut top) = (0f64, 0f64);
    let band = (h / 8).max(1) as usize;
    for y in 0..h as usize {
        for x in 0..w as usize {
            let r = real[y * w as usize + x];
            let m = &mine[(y * w as usize + x) * 4..(y * w as usize + x) * 4 + 3];
            let o = (y * 2 * w as usize + x) * 4;
            out[o..o + 4].copy_from_slice(&[r[0], r[1], r[2], 255]);
            let o = (y * 2 * w as usize + w as usize + x) * 4;
            out[o..o + 4].copy_from_slice(&[m[0], m[1], m[2], 255]);
            let d = (0..3).map(|k| (r[k] as f64 - m[k] as f64).abs()).sum::<f64>() / 3.0;
            diff += d;
            if y >= h as usize - band {
                bottom += d;
            }
            if y < band {
                top += d;
            }
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
    Ok(Comparison { rgba: out, width: 2 * w, height: h, pixel_diff: diff / (w * h) as f64, coarse_diff: coarse / (gw * gh) as f64, bottom_diff: bottom / (band * w as usize) as f64, top_diff: top / (band * w as usize) as f64, note })
}

/// A camera variant tried by [`fit`]: zoom (tan of the half field of view divided by it), pitch
/// and yaw offsets (degrees), the target moved up (blocks), the distance scaled.
#[derive(Clone, Copy, Debug)]
pub struct CamTweak {
    pub zoom: f32,
    pub dpitch: f32,
    pub dyaw: f32,
    pub dy: f32,
    pub dist: f32,
}

impl CamTweak {
    pub fn apply(&self, mut c: crate::camera::Camera) -> crate::camera::Camera {
        let t = (c.vfov.to_radians() / 2.0).tan() / self.zoom;
        c.vfov = (2.0 * t.atan()).to_degrees();
        c.pitch += self.dpitch.to_radians();
        c.yaw += self.dyaw.to_radians();
        c.target[1] += self.dy;
        c.dist *= self.dist;
        c
    }
}

/// Search the camera tweak that best overlays the viewer's frames on the real-game pictures
/// (mean RGB difference at `w` x `h`): the designs' texts and their pictures (JPEG). Returns the
/// tweaks tried, best first, with their mean difference.
pub fn fit(off: &mut Offscreen, atlas: &Atlas, cases: &[(String, Vec<u8>)], w: u32, h: u32, tweaks: &[CamTweak]) -> Result<Vec<(f64, CamTweak)>, String> {
    struct Case {
        mesh: crate::mesh::Mesh,
        cam: crate::camera::Camera,
        real: Vec<u8>,
    }
    let aspect = w as f32 / h as f32;
    let mut prepared = Vec::new();
    for (text, jpg) in cases {
        let (px, iw, ih) = decode_image(jpg)?;
        // box-downscale the picture to w x h
        let mut real = vec![0u8; (w * h * 3) as usize];
        for y in 0..h {
            for x in 0..w {
                let (x0, x1) = (x * iw / w, ((x + 1) * iw / w).max(x * iw / w + 1));
                let (y0, y1) = (y * ih / h, ((y + 1) * ih / h).max(y * ih / h + 1));
                let mut s = [0u32; 3];
                for yy in y0..y1 {
                    for xx in x0..x1 {
                        for k in 0..3 {
                            s[k] += px[((yy * iw + xx) * 4) as usize + k] as u32;
                        }
                    }
                }
                let n = (x1 - x0) * (y1 - y0);
                for k in 0..3 {
                    real[((y * w + x) * 3) as usize + k] = (s[k] / n) as u8;
                }
            }
        }
        let mut s = Session::new(text, &Options::default())?;
        if !s.mode.is_door() {
            s.go_to_still(20_000);
        }
        let mut look = scene::default_look(&s);
        look.floor_margin = Some(scene::PICTURE_GROUND_REACH);
        let cam = scene::picture_camera(&s, &look, aspect);
        prepared.push(Case { mesh: scene::build(atlas, &s, 0.0, &look), cam, real });
    }
    let mut out = Vec::new();
    for t in tweaks {
        let mut total = 0f64;
        for c in &prepared {
            let cam = t.apply(c.cam);
            let (solid, translucent) = scene::triangles(&c.mesh, &cam);
            let mine = off.render(&Frame { solid: &solid, translucent: &translucent, camera: cam, width: w, height: h })?;
            let mut d = 0f64;
            for i in 0..(w * h) as usize {
                for k in 0..3 {
                    d += (mine[i * 4 + k] as f64 - c.real[i * 3 + k] as f64).abs();
                }
            }
            total += d / (w * h * 3) as f64;
        }
        out.push((total / prepared.len().max(1) as f64, *t));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

/// Resize RGBA with a separable Lanczos-3 filter (ffmpeg's `flags=lanczos` downscale).
pub fn lanczos_resize(src: &[u8], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<u8> {
    fn weights(src_n: u32, dst_n: u32) -> Vec<(usize, Vec<f32>)> {
        let scale = src_n as f32 / dst_n as f32;
        let support = 3.0 * scale.max(1.0);
        let lanczos = |x: f32| -> f32 {
            if x.abs() < 1e-6 {
                1.0
            } else if x.abs() >= 3.0 {
                0.0
            } else {
                let px = std::f32::consts::PI * x;
                3.0 * px.sin() * (px / 3.0).sin() / (px * px)
            }
        };
        (0..dst_n)
            .map(|i| {
                let center = (i as f32 + 0.5) * scale;
                let a = ((center - support).floor() as i64).max(0) as usize;
                let b = ((center + support).ceil() as i64).min(src_n as i64) as usize;
                let mut ws: Vec<f32> = (a..b).map(|j| lanczos((j as f32 + 0.5 - center) / scale.max(1.0))).collect();
                let s: f32 = ws.iter().sum();
                if s.abs() > 1e-6 {
                    for v in &mut ws {
                        *v /= s;
                    }
                }
                (a, ws)
            })
            .collect()
    }
    let wx = weights(sw, dw);
    let wy = weights(sh, dh);
    // horizontal pass
    let mut tmp = vec![0f32; (dw * sh * 4) as usize];
    for y in 0..sh as usize {
        for (x, (a, ws)) in wx.iter().enumerate() {
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let s = (y * sw as usize + a + k) * 4;
                for c in 0..4 {
                    acc[c] += src[s + c] as f32 * w;
                }
            }
            let d = (y * dw as usize + x) * 4;
            tmp[d..d + 4].copy_from_slice(&acc);
        }
    }
    // vertical pass
    let mut out = vec![0u8; (dw * dh * 4) as usize];
    for (y, (a, ws)) in wy.iter().enumerate() {
        for x in 0..dw as usize {
            let mut acc = [0f32; 4];
            for (k, w) in ws.iter().enumerate() {
                let s = ((a + k) * dw as usize + x) * 4;
                for c in 0..4 {
                    acc[c] += tmp[s + c] * w;
                }
            }
            let d = (y * dw as usize + x) * 4;
            for c in 0..4 {
                out[d + c] = acc[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    out
}

/// A grid of frames over time (the animation the browser plays): a door from its lever flip, a
/// clock from its still moment; `step` game ticks between frames (fractions show moving blocks
/// mid-way). Returns the grid's RGBA and its size.
pub fn strip(off: &mut Offscreen, atlas: &Atlas, text: &str, cols: u32, rows: u32, step: f32, w: u32, h: u32) -> Result<(Vec<u8>, u32, u32), String> {
    // the browser's player, played at 20 gt/s: the same camera (inputs in view, a flying
    // machine followed), the same interpolation
    let aspect = w as f32 / h as f32;
    let mut p = crate::player::Player::new(text, aspect)?;
    p.show_inputs(atlas, aspect, scene::live_window(h as f32));
    if p.session.mode.is_door() {
        p.flip();
    } else {
        p.session.go_to_still(20_000);
        p.invalidate();
    }
    p.playing = true;
    let (gw, gh) = (cols * w, rows * h);
    let mut grid = vec![0u8; (gw * gh * 4) as usize];
    for i in 0..cols * rows {
        let (solid, translucent) = p.triangles(atlas);
        let rgba = off.render(&Frame { solid: &solid, translucent: &translucent, camera: p.camera, width: w, height: h })?;
        let (ox, oy) = ((i % cols) * w, (i / cols) * h);
        for y in 0..h {
            let src = &rgba[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
            let o = (((oy + y) * gw + ox) * 4) as usize;
            grid[o..o + (w * 4) as usize].copy_from_slice(src);
        }
        let mut left = step / p.tps;
        while left > 1e-6 {
            let d = left.min(0.25);
            p.update(d);
            left -= d;
        }
    }
    Ok((grid, gw, gh))
}

/// Decode a PNG or a JPEG to RGBA8.
pub fn decode_image(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), String> {
    if bytes.starts_with(&[0xff, 0xd8]) {
        let mut dec = jpeg_decoder::Decoder::new(bytes);
        let px = dec.decode().map_err(|e| format!("jpeg: {e}"))?;
        let info = dec.info().ok_or("jpeg: no info")?;
        let rgba: Vec<u8> = match info.pixel_format {
            jpeg_decoder::PixelFormat::RGB24 => px.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
            jpeg_decoder::PixelFormat::L8 => px.iter().flat_map(|v| [*v, *v, *v, 255]).collect(),
            f => return Err(format!("jpeg: unsupported pixel format {f:?}")),
        };
        return Ok((rgba, info.width as u32, info.height as u32));
    }
    decode_png(bytes)
}

pub use crate::atlas::vanilla::decode_png;

/// The game's block atlas of misode/mcmeta (`atlas::vanilla`) and its block definitions and
/// models (`gamemodel`), downloaded once into mctas's cache (LOCAL use: comparisons with the
/// real-game pictures; the browser fetches them itself): the PNG bytes, the atlas index, the
/// definitions and the models.
pub fn vanilla_atlas() -> Result<(Vec<u8>, String), String> {
    let (png, index, _, _) = vanilla_assets()?;
    Ok((png, index))
}

/// [`vanilla_atlas`] with the block definitions and models.
pub fn vanilla_assets() -> Result<(Vec<u8>, String, String, String), String> {
    let dir = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join(".cache/mctas/mcmeta/26.3-atlas");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let get = |url: &str, name: &str| -> Result<Vec<u8>, String> {
        let p = dir.join(name);
        if !p.exists() {
            let out = std::process::Command::new("curl").args(["-sSfL", "-o"]).arg(&p).arg(url).output().map_err(|e| format!("curl: {e}"))?;
            if !out.status.success() {
                let _ = std::fs::remove_file(&p);
                return Err(format!("downloading {url}: {}", String::from_utf8_lossy(&out.stderr)));
            }
        }
        std::fs::read(&p).map_err(|e| e.to_string())
    };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    let png = get(crate::atlas::vanilla::ATLAS_URL, "atlas.png")?;
    let index = text(get(crate::atlas::vanilla::INDEX_URL, "data.min.json")?)?;
    let defs = text(get(crate::gamemodel::DEFS_URL, "block_definition.min.json")?)?;
    let models = text(get(crate::gamemodel::MODELS_URL, "model.min.json")?)?;
    Ok((png, index, defs, models))
}

/// The game's entity and item atlases (PNG bytes, index), cached like [`vanilla_assets`].
pub fn vanilla_entity_assets() -> Result<((Vec<u8>, String), (Vec<u8>, String)), String> {
    let dir = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join(".cache/mctas/mcmeta/26.3-atlas");
    let get = |url: &str, name: &str| -> Result<Vec<u8>, String> {
        let p = dir.join(name);
        if !p.exists() {
            let out = std::process::Command::new("curl").args(["-sSfL", "-o"]).arg(&p).arg(url).output().map_err(|e| format!("curl: {e}"))?;
            if !out.status.success() {
                let _ = std::fs::remove_file(&p);
                return Err(format!("downloading {url}: {}", String::from_utf8_lossy(&out.stderr)));
            }
        }
        std::fs::read(&p).map_err(|e| e.to_string())
    };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    use crate::atlas::vanilla::*;
    Ok((
        (get(ENTITY_ATLAS_URL, "entities.png")?, text(get(ENTITY_INDEX_URL, "entities.min.json")?)?),
        (get(ITEM_ATLAS_URL, "items.png")?, text(get(ITEM_INDEX_URL, "items.min.json")?)?),
    ))
}

/// The colors of a texture: its mean and a k-means palette of `k` colors with their share of the
/// opaque pixels (most common first), each color optionally multiplied by `tint` (grass, leaves).
pub fn palette(rgba: &[u8], k: usize, tint: Option<[u8; 3]>) -> ([u8; 3], Vec<([u8; 3], f32)>) {
    let t = tint.map(|c| [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0]).unwrap_or([1.0; 3]);
    let px: Vec<[f32; 3]> = rgba.chunks(4).filter(|c| c[3] >= 128).map(|c| [c[0] as f32 * t[0], c[1] as f32 * t[1], c[2] as f32 * t[2]]).collect();
    if px.is_empty() {
        return ([0; 3], Vec::new());
    }
    let n = px.len() as f32;
    let mean = (0..3).map(|i| px.iter().map(|p| p[i]).sum::<f32>() / n).collect::<Vec<_>>();
    let lum = |p: &[f32; 3]| p[0] * 0.3 + p[1] * 0.59 + p[2] * 0.11;
    let mut sorted = px.clone();
    sorted.sort_by(|a, b| lum(a).total_cmp(&lum(b)));
    let k = k.max(1).min(px.len());
    // seeds at luminance quantiles, then Lloyd iterations
    let mut centers: Vec<[f32; 3]> = (0..k).map(|i| sorted[(i * 2 + 1) * sorted.len() / (2 * k)]).collect();
    let mut assign = vec![0usize; px.len()];
    for _ in 0..20 {
        for (j, p) in px.iter().enumerate() {
            assign[j] = (0..k).min_by(|a, b| {
                let d = |c: &[f32; 3]| (0..3).map(|i| (c[i] - p[i]).powi(2)).sum::<f32>();
                d(&centers[*a]).total_cmp(&d(&centers[*b]))
            }).unwrap();
        }
        for (c, center) in centers.iter_mut().enumerate() {
            let members: Vec<&[f32; 3]> = px.iter().zip(&assign).filter(|(_, a)| **a == c).map(|(p, _)| p).collect();
            if !members.is_empty() {
                for i in 0..3 {
                    center[i] = members.iter().map(|p| p[i]).sum::<f32>() / members.len() as f32;
                }
            }
        }
    }
    let mut out: Vec<([u8; 3], f32)> = (0..k).map(|c| (centers[c].map(|v| v.round().clamp(0.0, 255.0) as u8), assign.iter().filter(|a| **a == c).count() as f32 / n)).filter(|(_, f)| *f > 0.0).collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1));
    ([mean[0].round() as u8, mean[1].round() as u8, mean[2].round() as u8], out)
}

/// Textures in a grid, each scaled up `scale` times (nearest), on a checkerboard (transparency).
pub fn sheet(paths: &[std::path::PathBuf], scale: u32, cols: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let cell = 16 * scale + 4;
    let rows = (paths.len() as u32).div_ceil(cols.max(1));
    let (w, h) = (cols * cell, rows.max(1) * cell);
    let mut out = vec![40u8; (w * h * 4) as usize];
    for (i, p) in paths.iter().enumerate() {
        let bytes = std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let (px, tw, th) = decode_png(&bytes).map_err(|e| format!("{}: {e}", p.display()))?;
        let (ox, oy) = ((i as u32 % cols) * cell + 2, (i as u32 / cols) * cell + 2);
        for y in 0..16 * scale {
            for x in 0..16 * scale {
                let (sx, sy) = (x * tw.min(16) / (16 * scale), y * th.min(16).max(1) / (16 * scale));
                let s = ((sy * tw + sx) * 4) as usize;
                let a = px[s + 3] as u32;
                let bg = if ((x / 8) + (y / 8)) % 2 == 0 { 90 } else { 120 };
                let o = (((oy + y) * w + ox + x) * 4) as usize;
                for k in 0..3 {
                    out[o + k] = ((px[s + k] as u32 * a + bg * (255 - a)) / 255) as u8;
                }
                out[o + 3] = 255;
            }
        }
    }
    Ok((out, w, h))
}

/// The shipped wasm (`mcview_bg.wasm`) run natively by an interpreter, to check that the browser's
/// simulation is the native one: its `mcview_clock_period` export on design texts. Every import
/// (the browser's API) traps if called; the period detector needs none.
pub struct WasmProbe {
    store: wasmi::Store<()>,
    memory: wasmi::Memory,
    alloc: wasmi::TypedFunc<u32, u32>,
    period: wasmi::TypedFunc<(u32, u32, u32), u64>,
    fly_end: wasmi::TypedFunc<(u32, u32), u32>,
    digest: wasmi::TypedFunc<(u32, u32, u32), u64>,
}

impl WasmProbe {
    pub fn load(bytes: &[u8]) -> Result<WasmProbe, String> {
        let engine = wasmi::Engine::default();
        let module = wasmi::Module::new(&engine, bytes).map_err(|e| format!("wasm: {e}"))?;
        let mut store = wasmi::Store::new(&engine, ());
        let mut linker = wasmi::Linker::<()>::new(&engine);
        for imp in module.imports() {
            if let wasmi::ExternType::Func(ft) = imp.ty() {
                let label = format!("{}.{}", imp.module(), imp.name());
                linker.func_new(imp.module(), imp.name(), ft.clone(), move |_, _, _| Err(wasmi::Error::new(format!("the check called the browser import {label}")))).map_err(|e| format!("{e}"))?;
            }
        }
        let instance = linker.instantiate_and_start(&mut store, &module).map_err(|e| format!("instantiate: {e}"))?;
        let memory = instance.get_memory(&store, "memory").ok_or("no exported memory")?;
        let alloc = instance.get_typed_func::<u32, u32>(&store, "mcview_alloc").map_err(|e| format!("mcview_alloc: {e}"))?;
        let period = instance.get_typed_func::<(u32, u32, u32), u64>(&store, "mcview_clock_period").map_err(|e| format!("mcview_clock_period: {e}"))?;
        let fly_end = instance.get_typed_func::<(u32, u32), u32>(&store, "mcview_fly_end").map_err(|e| format!("mcview_fly_end: {e}"))?;
        let digest = instance.get_typed_func::<(u32, u32, u32), u64>(&store, "mcview_frames_digest").map_err(|e| format!("mcview_frames_digest: {e}"))?;
        Ok(WasmProbe { store, memory, alloc, period, fly_end, digest })
    }

    /// Copy a text into the wasm's memory: (pointer, length).
    fn put(&mut self, text: &str) -> Result<(u32, u32), String> {
        let len = text.len() as u32;
        let ptr = self.alloc.call(&mut self.store, len).map_err(|e| format!("alloc: {e}"))?;
        self.memory.write(&mut self.store, ptr as usize, text.as_bytes()).map_err(|e| format!("write: {e}"))?;
        Ok((ptr, len))
    }

    /// A flying machine's end tick in the wasm (None: not simulated).
    pub fn fly_end(&mut self, text: &str) -> Result<Option<u32>, String> {
        let (ptr, len) = self.put(text)?;
        let t = self.fly_end.call(&mut self.store, (ptr, len)).map_err(|e| format!("fly_end: {e}"))?;
        Ok((t != u32::MAX).then_some(t))
    }

    /// The wasm's frames digest over ticks 0..=`ticks`.
    pub fn frames_digest(&mut self, text: &str, ticks: u32) -> Result<u64, String> {
        let (ptr, len) = self.put(text)?;
        self.digest.call(&mut self.store, (ptr, len, ticks)).map_err(|e| format!("digest: {e}"))
    }

    /// (state warmup, state period, warmup, period), or None when the wasm finds no clock.
    pub fn period(&mut self, text: &str, horizon: u32) -> Result<Option<(u32, u32, u32, u32)>, String> {
        let len = text.len() as u32;
        let ptr = self.alloc.call(&mut self.store, len).map_err(|e| format!("alloc: {e}"))?;
        self.memory.write(&mut self.store, ptr as usize, text.as_bytes()).map_err(|e| format!("write: {e}"))?;
        let r = self.period.call(&mut self.store, (ptr, len, horizon)).map_err(|e| format!("period: {e}"))?;
        if r == u64::MAX {
            return Ok(None);
        }
        let f = |k: u32| ((r >> (16 * k)) & 0xffff) as u32;
        Ok(Some((f(0), f(1), f(2), f(3))))
    }
}
