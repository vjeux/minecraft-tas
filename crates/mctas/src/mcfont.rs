//! Text in the Minecraft font, for title cards and labels on videos: the bitmap glyph sheets
//! of the client jar (`assets/minecraft/font/include/default.json`), drawn into RGBA images
//! the way the game draws them (1 px advance after each glyph, shadow at +1,+1 at 1/4
//! brightness). PNG files are read and written through the static ffmpeg build (no image crate).

use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// An RGBA image (8 bits per channel, row-major).
#[derive(Clone, Debug)]
pub struct Image {
    pub w: u32,
    pub h: u32,
    pub px: Vec<u8>,
}

impl Image {
    pub fn new(w: u32, h: u32, rgba: [u8; 4]) -> Image {
        let mut px = Vec::with_capacity((w * h * 4) as usize);
        for _ in 0..w * h {
            px.extend_from_slice(&rgba);
        }
        Image { w, h, px }
    }

    /// Decode a PNG (size from its header, pixels through ffmpeg).
    pub fn load_png(path: &Path) -> Result<Image> {
        let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
            bail!("{} is not a PNG", path.display());
        }
        let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
        let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
        let out = Command::new(crate::video::ffmpeg()?)
            .args(["-loglevel", "error", "-i"])
            .arg(path)
            .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
            .output()?;
        if !out.status.success() || out.stdout.len() != (w * h * 4) as usize {
            bail!("decoding {} failed: {}", path.display(), String::from_utf8_lossy(&out.stderr));
        }
        Ok(Image { w, h, px: out.stdout })
    }

    /// Encode as PNG (keeps the alpha channel).
    pub fn save_png(&self, path: &Path) -> Result<()> {
        self.save(path)
    }

    /// Encode in the format of the file extension (PNG keeps the alpha channel; JPEG at good
    /// quality).
    pub fn save(&self, path: &Path) -> Result<()> {
        let png = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png"));
        let mut cmd = Command::new(crate::video::ffmpeg()?);
        cmd.args(["-loglevel", "error", "-y", "-f", "rawvideo", "-pix_fmt", "rgba", "-s", &format!("{}x{}", self.w, self.h)])
            .args(["-i", "-", "-frames:v", "1"]);
        if png {
            cmd.args(["-pix_fmt", "rgba"]);
        } else {
            cmd.args(["-q:v", "3"]);
        }
        let mut child = cmd.arg(path).stdin(Stdio::piped()).spawn().context("starting ffmpeg")?;
        child.stdin.take().unwrap().write_all(&self.px)?;
        if !child.wait()?.success() {
            bail!("writing {} failed", path.display());
        }
        Ok(())
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.w + x) * 4) as usize;
        [self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]]
    }

    /// Alpha-blend `c` over the pixel at (x, y); outside the image is ignored.
    pub fn blend(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w as i64 || y >= self.h as i64 || c[3] == 0 {
            return;
        }
        let i = ((y as u32 * self.w + x as u32) * 4) as usize;
        let a = c[3] as u32;
        let da = self.px[i + 3] as u32;
        // "over" with straight alpha.
        let out_a = a + da * (255 - a) / 255;
        for k in 0..3 {
            let s = c[k] as u32 * a;
            let d = self.px[i + k] as u32 * da * (255 - a) / 255;
            self.px[i + k] = if out_a == 0 { 0 } else { ((s + d) / out_a).min(255) as u8 };
        }
        self.px[i + 3] = out_a.min(255) as u8;
    }

    pub fn fill_rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 4]) {
        for yy in y..y + h {
            for xx in x..x + w {
                self.blend(xx, yy, c);
            }
        }
    }

    /// Tile `tex` (scaled `scale` times, nearest neighbor) over the whole image, each channel
    /// multiplied by `shade` (the game's darkened dirt backgrounds use 0.25).
    pub fn tile(&mut self, tex: &Image, scale: u32, shade: f32) {
        let s = scale.max(1);
        for y in 0..self.h {
            for x in 0..self.w {
                let c = tex.get((x / s) % tex.w, (y / s) % tex.h);
                let i = ((y * self.w + x) * 4) as usize;
                for k in 0..3 {
                    self.px[i + k] = (c[k] as f32 * shade).round().clamp(0.0, 255.0) as u8;
                }
                self.px[i + 3] = 255;
            }
        }
    }

    /// Draw `src` scaled `scale` times (nearest neighbor) with its top-left corner at (x, y).
    pub fn draw(&mut self, src: &Image, x: i64, y: i64, scale: u32) {
        let s = scale.max(1) as i64;
        for sy in 0..src.h as i64 {
            for sx in 0..src.w as i64 {
                let c = src.get(sx as u32, sy as u32);
                if c[3] == 0 {
                    continue;
                }
                for dy in 0..s {
                    for dx in 0..s {
                        self.blend(x + sx * s + dx, y + sy * s + dy, c);
                    }
                }
            }
        }
    }

    /// Draw `src` resized to `w` x `h` (nearest neighbor) at (x, y).
    pub fn draw_resized(&mut self, src: &Image, x: i64, y: i64, w: u32, h: u32) {
        for dy in 0..h {
            for dx in 0..w {
                let c = src.get(dx * src.w / w.max(1), dy * src.h / h.max(1));
                self.blend(x + dx as i64, y + dy as i64, c);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Glyph {
    sheet: usize,
    x: u32,
    y: u32,
    cell_w: u32,
    cell_h: u32,
    /// Ink width in sheet pixels (rightmost opaque column + 1).
    ink: u32,
    /// Font pixels per sheet pixel (provider height / cell height).
    scale: f32,
    ascent: i32,
}

/// The game's default bitmap font.
pub struct Font {
    sheets: Vec<Image>,
    glyphs: HashMap<char, Glyph>,
}

/// Line height in font pixels (as the game's `Font.lineHeight`).
pub const LINE_HEIGHT: u32 = 9;

impl Font {
    /// The default font of the client jar of `version`, extracted once into the version's
    /// cache folder.
    pub fn minecraft(version: &str) -> Result<Font> {
        let dir = crate::paths::version_dir(version).join("font");
        std::fs::create_dir_all(&dir)?;
        let mut zip = zip::ZipArchive::new(std::fs::File::open(client_jar(version)?)?).context("opening the client jar")?;
        let mut read = |name: &str| -> Result<Vec<u8>> {
            let mut f = zip.by_name(name).with_context(|| format!("{name} not in the client jar"))?;
            let mut v = Vec::new();
            f.read_to_end(&mut v)?;
            Ok(v)
        };
        let json: serde_json::Value = serde_json::from_slice(&read("assets/minecraft/font/include/default.json")?)?;
        let mut font = Font { sheets: Vec::new(), glyphs: HashMap::new() };
        for p in json["providers"].as_array().context("font json has no providers")? {
            if p["type"] != "bitmap" {
                continue;
            }
            let Some(file) = p["file"].as_str() else { continue };
            let rel = file.trim_start_matches("minecraft:");
            let png: PathBuf = dir.join(rel.replace('/', "_"));
            if !png.exists() {
                std::fs::write(&png, read(&format!("assets/minecraft/textures/{rel}"))?)?;
            }
            let sheet = Image::load_png(&png)?;
            let rows: Vec<Vec<char>> = p["chars"].as_array().into_iter().flatten().filter_map(|r| r.as_str()).map(|r| r.chars().collect()).collect();
            let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32;
            if rows.is_empty() || cols == 0 {
                continue;
            }
            let (cell_w, cell_h) = (sheet.w / cols, sheet.h / rows.len() as u32);
            let height = p["height"].as_f64().unwrap_or(8.0) as f32;
            let ascent = p["ascent"].as_i64().unwrap_or(7) as i32;
            let idx = font.sheets.len();
            for (r, row) in rows.iter().enumerate() {
                for (c, &ch) in row.iter().enumerate() {
                    if ch == '\0' || font.glyphs.contains_key(&ch) {
                        continue;
                    }
                    let (x, y) = (c as u32 * cell_w, r as u32 * cell_h);
                    let mut ink = 0;
                    for gx in 0..cell_w {
                        if (0..cell_h).any(|gy| sheet.get(x + gx, y + gy)[3] > 0) {
                            ink = gx + 1;
                        }
                    }
                    font.glyphs.insert(ch, Glyph { sheet: idx, x, y, cell_w, cell_h, ink, scale: height / cell_h as f32, ascent });
                }
            }
            font.sheets.push(sheet);
        }
        if !font.glyphs.contains_key(&'A') {
            bail!("the font has no glyph for 'A'");
        }
        Ok(font)
    }

    fn glyph(&self, ch: char) -> Option<&Glyph> {
        self.glyphs.get(&ch).or_else(|| self.glyphs.get(&'?'))
    }

    /// Advance of one character in font pixels.
    fn advance(&self, ch: char) -> f32 {
        if ch == ' ' {
            return 4.0;
        }
        match self.glyph(ch) {
            Some(g) => (0.5 + g.ink as f32 * g.scale).floor() + 1.0,
            None => 0.0,
        }
    }

    /// Width of `text` in screen pixels at `scale` screen pixels per font pixel.
    pub fn width(&self, text: &str, scale: u32) -> u32 {
        let w: f32 = text.chars().map(|c| self.advance(c)).sum();
        // The last glyph's 1 px spacing is not ink.
        ((w - 1.0).max(0.0) * scale as f32).round() as u32
    }

    /// Draw `text` with its line box's top-left corner at (x, y) (the line box is
    /// [`LINE_HEIGHT`] font pixels tall), `scale` screen pixels per font pixel, optionally with
    /// the game's drop shadow.
    pub fn draw(&self, img: &mut Image, x: i64, y: i64, text: &str, scale: u32, color: [u8; 3], shadow: bool) {
        if shadow {
            let sc = [color[0] / 4, color[1] / 4, color[2] / 4];
            self.draw_plain(img, x + scale as i64, y + scale as i64, text, scale, sc);
        }
        self.draw_plain(img, x, y, text, scale, color);
    }

    fn draw_plain(&self, img: &mut Image, x: i64, y: i64, text: &str, scale: u32, color: [u8; 3]) {
        let s = scale as f32;
        let baseline = y as f32 + 7.0 * s;
        let mut pen = x as f32;
        for ch in text.chars() {
            if ch != ' ' {
                if let Some(g) = self.glyph(ch) {
                    let sheet = &self.sheets[g.sheet];
                    let k = g.scale * s; // screen pixels per sheet pixel
                    let top = baseline - g.ascent as f32 * s;
                    for gy in 0..g.cell_h {
                        for gx in 0..g.ink.min(g.cell_w) {
                            let c = sheet.get(g.x + gx, g.y + gy);
                            if c[3] == 0 {
                                continue;
                            }
                            let (x0, y0) = ((pen + gx as f32 * k).round() as i64, (top + gy as f32 * k).round() as i64);
                            let (x1, y1) = ((pen + (gx + 1) as f32 * k).round() as i64, (top + (gy + 1) as f32 * k).round() as i64);
                            let rgba = [
                                (c[0] as u32 * color[0] as u32 / 255) as u8,
                                (c[1] as u32 * color[1] as u32 / 255) as u8,
                                (c[2] as u32 * color[2] as u32 / 255) as u8,
                                c[3],
                            ];
                            img.fill_rect(x0, y0, x1 - x0, y1 - y0, rgba);
                        }
                    }
                }
            }
            pen += self.advance(ch) * s;
        }
    }

    /// Draw `text` horizontally centered on `cx`.
    pub fn draw_centered(&self, img: &mut Image, cx: i64, y: i64, text: &str, scale: u32, color: [u8; 3], shadow: bool) {
        let w = self.width(text, scale) as i64;
        self.draw(img, cx - w / 2, y, text, scale, color, shadow);
    }

    /// Greedy word wrap of `text` into lines at most `max_w` screen pixels wide at `scale`.
    pub fn wrap(&self, text: &str, scale: u32, max_w: u32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if self.width(&cand, scale) <= max_w || cur.is_empty() {
                cur = cand;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        lines
    }
}

/// The client jar of `version` (downloaded when missing).
fn client_jar(version: &str) -> Result<PathBuf> {
    let jar = crate::paths::version_dir(version).join("client.jar");
    if jar.exists() {
        return Ok(jar);
    }
    Ok(crate::setup::install_client(version)?.client_jar)
}

/// A texture of the client jar (e.g. `block/dirt`), extracted once.
pub fn texture(version: &str, name: &str) -> Result<Image> {
    let dir = crate::paths::version_dir(version).join("font");
    std::fs::create_dir_all(&dir)?;
    let png = dir.join(format!("tex_{}.png", name.replace('/', "_")));
    if !png.exists() {
        let mut zip = zip::ZipArchive::new(std::fs::File::open(client_jar(version)?)?)?;
        let mut f = zip.by_name(&format!("assets/minecraft/textures/{name}.png")).with_context(|| format!("texture {name} not in the client jar"))?;
        let mut v = Vec::new();
        f.read_to_end(&mut v)?;
        std::fs::write(&png, v)?;
    }
    Image::load_png(&png)
}
