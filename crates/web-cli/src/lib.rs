//! `mctas web ...`: the in-browser viewer's commands (docs/webrender.md). Argument parsing and
//! dispatch only (compiled unoptimized); the work is in `crates/mcview`.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::{Path, PathBuf};

mod cdp;

#[derive(Subcommand)]
pub enum WebCmd {
    /// The viewer's entities (redsim, what the browser draws) tick by tick, next to a real-game
    /// trace's (`designs/clocks/traces/<stem>.trace`, `mctas clock misc trace-export`) with the
    /// distance between them; ends with the largest distance over the ticks shown.
    Entities {
        design: PathBuf,
        #[arg(long)]
        trace: Option<PathBuf>,
        #[arg(long, default_value_t = 0)]
        from: u32,
        #[arg(long, default_value_t = 40)]
        to: u32,
    },
    /// Write the game texture the viewer samples (the game's block atlas, with the entity and
    /// item textures packed below it) to a PNG, and list where the entity textures went.
    GameTexture {
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Render designs to PNG offscreen with the browser viewer's renderer (wgpu, Vulkan headless:
    /// lavapipe on a GPU-less box). Clocks at their most active moment (like `clock shot`), doors
    /// at rest; `--tick N` runs N ticks first, `--flips N` flips a door's lever N times.
    Shot {
        designs: Vec<PathBuf>,
        /// Output PNG (one design) or directory (`<stem>.png` each).
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 800)]
        width: u32,
        #[arg(long, default_value_t = 450)]
        height: u32,
        #[arg(long)]
        tick: Option<u32>,
        #[arg(long, default_value_t = 0)]
        flips: u32,
        /// Camera yaw / pitch in degrees (default: the real-game pictures' camera).
        #[arg(long)]
        yaw: Option<f32>,
        #[arg(long)]
        pitch: Option<f32>,
        /// The hand-drawn tiles only (default: the game's textures, misode/mcmeta's atlas,
        /// downloaded once into ~/.cache/mctas, local only).
        #[arg(long)]
        procedural: bool,
        /// As a live viewer of this size frames it (its default camera and outlines), not a
        /// thumbnail.
        #[arg(long)]
        live: bool,
        /// With --live: the device pixel ratio (the viewer's CSS size is the picture's / this;
        /// a phone: --width 654 --height 960 --dpr 2).
        #[arg(long, default_value_t = 1.0)]
        dpr: f32,
    },
    /// Check the viewer against the repo's tools: its design parser against `mctas::design` on
    /// every block and port, and its clock period detector (the code the browser runs) against
    /// `mctas clock sim` (state warmup + period, visible warmup + period).
    Check {
        designs: Vec<PathBuf>,
        /// Ticks to look for a repeat.
        #[arg(long, default_value_t = 50_000)]
        horizon: u32,
        /// Only failures and the summary.
        #[arg(long, short)]
        quiet: bool,
        /// Also run the period detector of this built wasm (web/viewer/mcview_bg.wasm, run by an
        /// interpreter) and require the same numbers: the browser simulates exactly like redsim.
        #[arg(long)]
        wasm: Option<PathBuf>,
    },
    /// Build the browser bundle: `crates/mcview-web` for wasm32 (profile `web`), then wasm-bindgen
    /// (`--target web --out-name mcview`): `<out>/mcview.js` + `<out>/mcview_bg.wasm`. Needs the
    /// wasm32 target (`rustup target add wasm32-unknown-unknown`) and `wasm-bindgen` 0.2.129
    /// (`cargo install wasm-bindgen-cli --version =0.2.129`). Run from the repository root.
    Bundle {
        #[arg(long, short, default_value = "web/viewer")]
        out: PathBuf,
    },
    /// Write a standalone demo page: `<out>/index.html` (a grid of thumbnails and live viewers),
    /// the designs under `<out>/designs/`, the bundle copied from `--viewer` into `<out>/viewer/`.
    Demo {
        designs: Vec<PathBuf>,
        #[arg(long, short)]
        out: PathBuf,
        /// The built bundle (mcview.js + mcview_bg.wasm).
        #[arg(long, default_value = "web/viewer")]
        viewer: PathBuf,
        /// How many of the designs get a live viewer (the first ones).
        #[arg(long, default_value_t = 2)]
        live: usize,
        /// The live viewers start playing (`data-autoplay`).
        #[arg(long)]
        autoplay: bool,
    },
    /// Serve a directory over HTTP on localhost (to open a demo page in a browser).
    Serve {
        dir: PathBuf,
        #[arg(long, default_value_t = 8000)]
        port: u16,
    },
    /// Sanity check against the real game: each design's real-game picture
    /// (`<img-dir>/<stem>.jpg`, from `mctas clock shot`) next to the viewer's frame of the same
    /// moment and camera, as `<out>/<stem>.cmp.png`, with the mean RGB difference per pixel and on
    /// a coarse 32x18 grid.
    Compare {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "designs/clocks/img")]
        img_dir: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
        /// The hand-drawn tiles only (default: the game's textures).
        #[arg(long)]
        procedural: bool,
        /// Also write `<stem>.zoom.png`: this region (x,y,w,h of the picture) of the real picture
        /// and of the viewer's, side by side, scaled up 4x (nearest), to compare pixels.
        #[arg(long)]
        zoom: Option<String>,
        /// The ground's reach around the build (default: the viewer's, 56 around a clock), to
        /// measure the pictures' floor edges.
        #[arg(long)]
        floor_margin: Option<f32>,
    },
    /// Layout-shift check of a page's live viewers in headless Chromium (real time, WebGL2 in
    /// software): opens `<url>?mcview-debug` for `--seconds`, reads every frame's bounding boxes of
    /// each viewer, its bar, counter and badge, and fails unless they never moved over at least
    /// `--min-ticks` ticks (give the viewers `data-autoplay`).
    Layout {
        url: String,
        #[arg(long, default_value_t = 8)]
        seconds: u64,
        /// Window size in CSS px, `W,H` (390,844 for a phone).
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        #[arg(long, default_value_t = 100)]
        min_ticks: u32,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Click test of a page's live viewers in headless Chromium (real input events through the
    /// DevTools protocol): for every viewer with a lever or button, clicks it (or taps it with
    /// `--touch`) while paused and requires the input to flip, the simulation to start and the
    /// build to react (a door's opening changes, or another block turns on) within `--wait`
    /// seconds.
    Click {
        url: String,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Tap (touch events, a phone) instead of a mouse click.
        #[arg(long)]
        touch: bool,
        /// Press and HOLD each circuit input in the view instead: it must be on while held, off
        /// once released, and the camera must not move.
        #[arg(long)]
        hold: bool,
        #[arg(long, default_value_t = 8)]
        wait: u64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Press a live viewer's Record button in headless Chromium and keep the video it downloads
    /// (`<design>.mp4` or `.webm`) in `--out`.
    Record {
        url: String,
        #[arg(long, short)]
        out: PathBuf,
        /// Which live viewer of the page (0: the first).
        #[arg(long, default_value_t = 0)]
        index: usize,
        #[arg(long, default_value = "1280,720")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Seconds to wait for the file.
        #[arg(long, default_value_t = 300)]
        timeout: u64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// A screenshot of a page in headless Chromium after some seconds of real time (playing
    /// viewers and demos advance), e.g. to check a live viewer's overlays.
    Snap {
        url: String,
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 4.0)]
        seconds: f64,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value_t = 1.0)]
        scale: f32,
        /// Scroll the page down this many CSS px first.
        #[arg(long, default_value_t = 0.0)]
        scroll: f64,
        /// Also evaluate this JavaScript expression in the page at the end and print its value
        /// (diagnostics: a WebGL limit, an element's state).
        #[arg(long)]
        eval: Option<String>,
        /// Evaluate this JavaScript after `--before-at` seconds, then wait `--seconds` more
        /// before the screenshot (an in-place swap: set `data-keep-camera` and `data-src`).
        #[arg(long)]
        before: Option<String>,
        #[arg(long, default_value_t = 4.0)]
        before_at: f64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// A page's first live viewer loses its WebGL context and gets it back
    /// (WEBGL_lose_context): it must draw again.
    Lose {
        url: String,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// The site's in-place design swap on a page's first live viewer: orbit, set
    /// `data-keep-camera` then `data-src` to `--to`: the new design must show under the same
    /// camera, the viewer marked `mcview-live-src`.
    Swap {
        url: String,
        #[arg(long)]
        to: String,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Mostly-black check: natively, every design given rendered offscreen (its still, small);
    /// with `--url`, every viewer of a page in headless Chromium through the real bundle (the
    /// WebGL path). Fails on any frame with a mean luma under `--min` or mostly near-black.
    Luma {
        designs: Vec<PathBuf>,
        #[arg(long)]
        url: Option<String>,
        #[arg(long, default_value_t = 40.0)]
        min: f64,
        #[arg(long, default_value = "1280,900")]
        window: String,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
    },
    /// Fit the default camera to the real-game pictures: renders each design under camera tweaks
    /// (zoom, pitch, yaw, height, distance) and prints the best by mean pixel difference.
    Fit {
        designs: Vec<PathBuf>,
        #[arg(long, default_value = "designs/clocks/img")]
        img_dir: PathBuf,
        /// Zooms to try (comma-separated).
        #[arg(long, default_value = "1.0,1.1,1.2,1.3")]
        zoom: String,
        #[arg(long, default_value = "-2,0,2")]
        pitch: String,
        #[arg(long, default_value = "-2,0,2")]
        yaw: String,
        #[arg(long, default_value = "0")]
        dy: String,
        #[arg(long, default_value = "1.0")]
        dist: String,
    },
    /// Texture contact sheet: PNGs scaled up (nearest) in a grid, in the given order (row by row),
    /// to pick textures by eye.
    Sheet {
        pngs: Vec<PathBuf>,
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 8)]
        scale: u32,
        #[arg(long, default_value_t = 8)]
        cols: u32,
    },
    /// The viewer's own textures as PNGs (`<out>/<tile>.png`), to compare with the game's.
    Tiles {
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Colors of textures: mean and a k-means palette with shares (`--tint rrggbb` multiplies,
    /// for the game's grayscale grass and leaves).
    Palette {
        pngs: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        k: usize,
        #[arg(long)]
        tint: Option<String>,
        /// Only this rectangle `x,y,w,h` of each image (PNG or JPEG: a region of a real-game
        /// picture); default: the first 16x16 (a texture's first frame).
        #[arg(long)]
        crop: Option<String>,
    },
    /// LOCAL REFERENCE ONLY (never commit or ship the output: Mojang's assets): the game's block
    /// textures from the client jar of `--version` (downloaded once into the version's cache
    /// directory) into `<out>/<name>.png`; every block texture, or only the given names.
    Reference {
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value = "26.3")]
        version: String,
        names: Vec<String>,
    },
    /// A grid of frames over time, as the browser animates them: a door from its lever flip, a
    /// clock from its still moment, `--step` game ticks apart (0.5 shows moving blocks mid-way).
    Strip {
        design: PathBuf,
        #[arg(long, short)]
        out: PathBuf,
        #[arg(long, default_value_t = 6)]
        cols: u32,
        #[arg(long, default_value_t = 4)]
        rows: u32,
        #[arg(long, default_value_t = 0.5)]
        step: f32,
        #[arg(long, default_value_t = 320)]
        width: u32,
        #[arg(long, default_value_t = 240)]
        height: u32,
        /// The hand-drawn tiles only (default: the game's textures).
        #[arg(long)]
        procedural: bool,
    },
}

/// The tiles: the game's textures over the hand-drawn ones (misode/mcmeta's atlas, cached
/// locally) and the game's own block models (`--procedural`: the hand-drawn tiles and the
/// viewer's own models only; `MCVIEW_NO_GAME_MODELS=1`: the game's textures on the viewer's
/// models).
fn atlas(procedural: bool) -> mcview::atlas::Atlas {
    let mut a = mcview::atlas::Atlas::build();
    if !procedural {
        match mcview::tools::vanilla_assets() {
            Ok((png, idx, defs, models)) => {
                match a.apply_vanilla_png(&png, &idx) {
                    Ok(n) => eprintln!("the game's textures on {n} tiles"),
                    Err(e) => eprintln!("the game's textures unavailable ({e}): hand-drawn tiles"),
                }
                if std::env::var_os("MCVIEW_NO_GAME_MODELS").is_none() {
                    let r = mcview::tools::decode_png(&png).and_then(|(rgba, w, h)| a.set_game(&rgba, w, h, &idx, &defs, &models));
                    match r {
                        Ok(n) => eprintln!("the game's block models ({n} textures)"),
                        Err(e) => eprintln!("the game's block models unavailable ({e}): the viewer's models"),
                    }
                    // the entity and item textures (boats, minecarts, armor stands, dropped items)
                    let r = mcview::tools::vanilla_entity_assets().and_then(|((ep, ei), (ip, ii))| {
                        let (er, ew, eh) = mcview::tools::decode_png(&ep)?;
                        let (ir, iw, ih) = mcview::tools::decode_png(&ip)?;
                        a.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii))
                    });
                    if let Err(e) = r {
                        eprintln!("the game's entity textures unavailable ({e}): the viewer's stand-ins");
                    }
                }
            }
            Err(e) => eprintln!("the game's textures unavailable ({e}): hand-drawn tiles"),
        }
    }
    a
}

pub fn run(_version: &str, cmd: WebCmd) -> Result<()> {
    match cmd {
        WebCmd::Shot { designs, out, width, height, tick, flips, yaw, pitch, procedural, live, dpr } => cmd_shot(&designs, &out, mcview::tools::ShotOptions { width, height, tick, flips, yaw, pitch, picture: false, floor_margin: None, live, dpr }, procedural),
        WebCmd::Check { designs, horizon, quiet, wasm } => cmd_check(&designs, horizon, quiet, wasm.as_deref()),
        WebCmd::Bundle { out } => cmd_bundle(&out),
        WebCmd::Demo { designs, out, viewer, live, autoplay } => cmd_demo(&designs, &out, &viewer, live, autoplay),
        WebCmd::Serve { dir, port } => cmd_serve(&dir, port),
        WebCmd::Compare { designs, img_dir, out, procedural, zoom, floor_margin } => cmd_compare(&designs, &img_dir, &out, procedural, zoom.as_deref(), floor_margin),
        WebCmd::Tiles { out } => {
            let atlas = mcview::atlas::Atlas::build();
            std::fs::create_dir_all(&out)?;
            let mut names: Vec<(&str, u16)> = atlas.names.iter().map(|(k, v)| (*k, *v)).collect();
            names.sort();
            for (name, layer) in &names {
                let px = &atlas.mips[0][*layer as usize * 16 * 16 * 4..(*layer as usize + 1) * 16 * 16 * 4];
                mcview::render::headless::write_png(&out.join(format!("{name}.png")), 16, 16, px).map_err(anyhow::Error::msg)?;
            }
            println!("{} tiles in {}", names.len(), out.display());
            Ok(())
        }
        WebCmd::Entities { design, trace, from, to } => {
            let text = std::fs::read_to_string(&design)?;
            let mut s = mcview::Session::new(&text, &mcview::Options::default()).map_err(anyhow::Error::msg)?;
            let replay = match &trace {
                Some(p) => Some(mcview::replay::Replay::parse(&std::fs::read_to_string(p)?).map_err(anyhow::Error::msg)?),
                None => None,
            };
            while s.tick < from {
                s.step();
            }
            let mut worst = 0f32;
            while s.tick <= to {
                // the simulated positions at the end of the tick (the trace's convention; the
                // renderer draws a minecart on a rail at the rail's line instead, like
                // AbstractMinecartRenderer.oldRender, so the drawn pose is not compared)
                let mine: Vec<(String, [f32; 3], f32)> = s
                    .world
                    .entities
                    .list
                    .iter()
                    .filter(|e| !e.removed)
                    .zip(s.entities(1.0))
                    .map(|(e, d)| (d.kind, [e.pos.x as f32, e.pos.y as f32, e.pos.z as f32], e.y_rot))
                    .collect();
                let line: Vec<String> = mine.iter().map(|(k, p, yaw)| format!("{k} ({:.4}, {:.4}, {:.4}) yaw {yaw:.1}", p[0], p[1], p[2])).collect();
                println!("t {}: {}", s.tick, line.join("; "));
                // what the viewer draws besides redsim's entities: falling blocks, primed TNT
                let extra: Vec<String> = s.entities(1.0).iter().filter(|e| e.kind.starts_with("primed:") || e.kind.starts_with("falling:")).map(|e| format!("{} ({:.3}, {:.3}, {:.3}){}", e.kind.split('[').next().unwrap_or(""), e.pos[0], e.pos[1], e.pos[2], if e.fuse > 0.0 { format!(" fuse {:.0}", e.fuse) } else { String::new() })).collect();
                if !extra.is_empty() {
                    println!("  drawn: {}", extra.join("; "));
                }
                if let Some(r) = &replay {
                    let theirs = r.entities_at(s.tick);
                    let mut parts = Vec::new();
                    for (k, e) in theirs.iter().enumerate() {
                        let d = mine.get(k).map(|(_, m, _)| ((m[0] - e.pos[0]).powi(2) + (m[1] - e.pos[1]).powi(2) + (m[2] - e.pos[2]).powi(2)).sqrt());
                        if let Some(d) = d {
                            worst = worst.max(d);
                        }
                        parts.push(format!("{} ({:.4}, {:.4}, {:.4}) yaw {:.1}{}", e.kind, e.pos[0], e.pos[1], e.pos[2], e.yaw, d.map(|d| format!(" [{d:.4}]")).unwrap_or_default()));
                    }
                    println!("  game: {}", parts.join("; "));
                }
                s.step();
            }
            if replay.is_some() {
                println!("largest distance to the game: {worst:.4} blocks over ticks {from}..={to}");
            }
            Ok(())
        }
        WebCmd::GameTexture { out } => {
            let a = atlas(false);
            let g = a.game.as_ref().ok_or_else(|| anyhow::anyhow!("no game assets"))?;
            mcview::render::headless::write_png(&out, g.width, g.height, &g.mips[0]).map_err(anyhow::Error::msg)?;
            let mut ent: Vec<_> = g.tex.iter().filter(|(k, _)| k.starts_with("entity/")).map(|(k, t)| format!("{k} {:?}", t.rect)).collect();
            ent.sort();
            for l in &ent {
                println!("{l}");
            }
            println!("{} ({}x{}, entities packed: {})", out.display(), g.width, g.height, g.entities);
            Ok(())
        }
        WebCmd::Palette { pngs, k, tint, crop } => {
            let tint = match tint {
                Some(h) => {
                    let v = u32::from_str_radix(h.trim_start_matches('#'), 16)?;
                    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
                }
                None => None,
            };
            let crop: Option<Vec<u32>> = crop.map(|c| c.split(',').filter_map(|w| w.trim().parse().ok()).collect());
            for p in &pngs {
                let (px, w, h) = mcview::tools::decode_image(&std::fs::read(p)?).map_err(anyhow::Error::msg)?;
                let (x0, y0, cw, ch) = match crop.as_deref() {
                    Some([x, y, cw, ch]) => (*x, *y, *cw, *ch),
                    _ => (0, 0, w.min(16), h.min(16)),
                };
                let mut sel = Vec::new();
                for y in y0..(y0 + ch).min(h) {
                    for x in x0..(x0 + cw).min(w) {
                        let o = ((y * w + x) * 4) as usize;
                        sel.extend_from_slice(&px[o..o + 4]);
                    }
                }
                let (mean, pal) = mcview::tools::palette(&sel, k, tint);
                let hex = |c: [u8; 3]| format!("{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                let parts: Vec<String> = pal.iter().map(|(c, f)| format!("{} {:.0}%", hex(*c), f * 100.0)).collect();
                println!("{:<32} mean {}  {}", p.file_stem().unwrap_or_default().to_string_lossy(), hex(mean), parts.join("  "));
            }
            Ok(())
        }
        WebCmd::Fit { designs, img_dir, zoom, pitch, yaw, dy, dist } => {
            let nums = |s: &str| -> Vec<f32> { s.split(',').filter_map(|w| w.trim().parse().ok()).collect() };
            let mut tweaks = Vec::new();
            for &z in &nums(&zoom) {
                for &p in &nums(&pitch) {
                    for &y in &nums(&yaw) {
                        for &d in &nums(&dy) {
                            for &ds in &nums(&dist) {
                                tweaks.push(mcview::tools::CamTweak { zoom: z, dpitch: p, dyaw: y, dy: d, dist: ds });
                            }
                        }
                    }
                }
            }
            let mut cases = Vec::new();
            for d in &designs {
                let stem = d.file_stem().unwrap_or_default().to_string_lossy().to_string();
                let jpg = img_dir.join(format!("{stem}.jpg"));
                match std::fs::read(&jpg) {
                    Ok(b) => cases.push((std::fs::read_to_string(d)?, b)),
                    Err(_) => eprintln!("{stem}: no picture {}", jpg.display()),
                }
            }
            let atlas = atlas(false);
            let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
            let r = mcview::tools::fit(&mut off, &atlas, &cases, 400, 225, &tweaks).map_err(anyhow::Error::msg)?;
            for (d, t) in r.iter().take(12) {
                println!("{d:6.2}  zoom {:.3} pitch {:+.1} yaw {:+.1} dy {:+.2} dist {:.3}", t.zoom, t.dpitch, t.dyaw, t.dy, t.dist);
            }
            Ok(())
        }
        WebCmd::Lose { url, window, browser } => crate::cdp::cmd_lose(&url, &window, &browser),
        WebCmd::Swap { url, to, window, browser } => crate::cdp::cmd_swap(&url, &to, &window, &browser),
        WebCmd::Luma { designs, url, min, window, browser } => match url {
            Some(u) => crate::cdp::cmd_luma(&u, &window, min, &browser),
            None => cmd_luma_native(&designs, min),
        },
        WebCmd::Snap { url, out, seconds, window, scale, scroll, eval, before, before_at, browser } => crate::cdp::cmd_snap(&url, &window, scale, seconds, scroll, eval.as_deref(), before.as_deref().map(|b| (b, before_at)), &out, &browser),
        WebCmd::Click { url, window, scale, touch, hold, wait, browser } => {
            if hold {
                crate::cdp::cmd_hold(&url, &window, scale, touch, &browser)
            } else {
                crate::cdp::cmd_click(&url, &window, scale, touch, wait, &browser)
            }
        }
        WebCmd::Record { url, out, index, window, scale, timeout, browser } => crate::cdp::cmd_record(&url, &out, index, &window, scale, timeout, &browser),
        WebCmd::Layout { url, seconds, window, scale, min_ticks, browser } => cmd_layout(&url, seconds, &window, scale, min_ticks, &browser),
        WebCmd::Reference { out, version, names } => cmd_reference(&out, &version, &names),
        WebCmd::Sheet { pngs, out, scale, cols } => {
            let (rgba, w, h) = mcview::tools::sheet(&pngs, scale, cols).map_err(anyhow::Error::msg)?;
            mcview::render::headless::write_png(&out, w, h, &rgba).map_err(anyhow::Error::msg)?;
            println!("{} ({} textures)", out.display(), pngs.len());
            Ok(())
        }
        WebCmd::Strip { design, out, cols, rows, step, width, height, procedural } => {
            let atlas = atlas(procedural);
            let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
            let text = std::fs::read_to_string(&design)?;
            let (rgba, w, h) = mcview::tools::strip(&mut off, &atlas, &text, cols, rows, step, width, height).map_err(anyhow::Error::msg)?;
            mcview::render::headless::write_png(&out, w, h, &rgba).map_err(anyhow::Error::msg)?;
            println!("{} ({cols}x{rows} frames, {step} gt apart)", out.display());
            Ok(())
        }
    }
}

/// Every design's still rendered offscreen (small): fails on a mostly black frame.
fn cmd_luma_native(designs: &[PathBuf], min: f64) -> Result<()> {
    let atlas = atlas(false);
    let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
    let (mut n, mut dark, mut errors) = (0usize, Vec::new(), 0usize);
    let mut worst = (f64::MAX, String::new());
    for d in designs {
        let Ok(text) = std::fs::read_to_string(d) else { continue };
        match mcview::tools::shot(&mut off, &atlas, &text, &mcview::tools::ShotOptions { width: 160, height: 90, ..Default::default() }) {
            Ok((rgba, _)) => {
                n += 1;
                let (mean, black) = crate::cdp::luma_stats(&rgba);
                if mean < worst.0 {
                    worst = (mean, d.display().to_string());
                }
                if mean < min || black > 0.5 {
                    dark.push(format!("{}: mean luma {mean:.0}, {:.0}% near black", d.display(), black * 100.0));
                }
            }
            Err(e) => {
                // a file the viewer cannot read at all (not a design): reported, not a frame
                errors += 1;
                if errors <= 5 {
                    println!("{}: {e}", d.display());
                }
            }
        }
    }
    for l in &dark {
        println!("DARK {l}");
    }
    println!("web luma: {n} designs rendered natively, {} dark, {errors} unreadable (darkest {:.0}: {})", dark.len(), worst.0, worst.1);
    if !dark.is_empty() {
        bail!("{} designs render mostly black", dark.len());
    }
    Ok(())
}

fn cmd_compare(designs: &[PathBuf], img_dir: &Path, out: &Path, procedural: bool, zoom: Option<&str>, floor_margin: Option<f32>) -> Result<()> {
    let atlas = atlas(procedural);
    let zoom: Option<Vec<u32>> = zoom.map(|z| z.split(',').filter_map(|v| v.trim().parse().ok()).collect());
    if zoom.as_ref().is_some_and(|z| z.len() != 4) {
        bail!("--zoom wants x,y,w,h");
    }
    let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
    std::fs::create_dir_all(out)?;
    let (mut n, mut sum_px, mut sum_coarse) = (0, 0.0, 0.0);
    for d in designs {
        let stem = d.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let jpg = img_dir.join(format!("{stem}.jpg"));
        let Ok(bytes) = std::fs::read(&jpg) else {
            println!("{stem}: no picture at {}", jpg.display());
            continue;
        };
        let text = std::fs::read_to_string(d)?;
        match mcview::tools::compare(&mut off, &atlas, &text, &bytes, floor_margin) {
            Ok(c) => {
                let path = out.join(format!("{stem}.cmp.png"));
                mcview::render::headless::write_png(&path, c.width, c.height, &c.rgba).map_err(anyhow::Error::msg)?;
                if let Some(z) = &zoom {
                    // the region of both halves (real left, viewer right), 4x nearest
                    let (zx, zy, zw, zh, s) = (z[0], z[1], z[2], z[3], 4u32);
                    let half = c.width / 2;
                    let (ow, oh) = (2 * zw * s + 8, zh * s);
                    let mut img = vec![30u8; (ow * oh * 4) as usize];
                    for (k, x_off) in [(0u32, 0u32), (1, zw * s + 8)] {
                        for y in 0..zh * s {
                            for x in 0..zw * s {
                                let (sx, sy) = ((zx + x / s).min(half - 1) + k * half, (zy + y / s).min(c.height - 1));
                                let si = ((sy * c.width + sx) * 4) as usize;
                                let di = ((y * ow + x_off + x) * 4) as usize;
                                img[di..di + 4].copy_from_slice(&c.rgba[si..si + 4]);
                            }
                        }
                    }
                    mcview::render::headless::write_png(&out.join(format!("{stem}.zoom.png")), ow, oh, &img).map_err(anyhow::Error::msg)?;
                }
                println!("{stem}: pixel diff {:.1}, coarse diff {:.1}, bottom {:.1}, top {:.1} ({}) -> {}", c.pixel_diff, c.coarse_diff, c.bottom_diff, c.top_diff, c.note, path.display());
                n += 1;
                sum_px += c.pixel_diff;
                sum_coarse += c.coarse_diff;
            }
            Err(e) => println!("{stem}: ERROR {e}"),
        }
    }
    if n > 0 {
        println!("web compare: {n} pictures, mean pixel diff {:.1}, mean coarse diff {:.1} (0 = identical, 255 = opposite)", sum_px / n as f64, sum_coarse / n as f64);
    }
    Ok(())
}

/// A tool from PATH, else from ~/.cargo/bin.
fn tool(name: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let cargo_bin = home.join(".cargo/bin").join(name);
    if cargo_bin.exists() {
        return cargo_bin;
    }
    PathBuf::from(name)
}

fn cmd_bundle(out: &Path) -> Result<()> {
    let status = std::process::Command::new(tool("cargo"))
        .args(["build", "-p", "mcview-web", "--profile", "web", "--target", "wasm32-unknown-unknown"])
        .status()
        .context("running cargo (is the wasm32 target installed?)")?;
    if !status.success() {
        bail!("cargo build of mcview-web for wasm32 failed");
    }
    let wasm = Path::new("target/wasm32-unknown-unknown/web/mcview_web.wasm");
    if !wasm.exists() {
        bail!("{} missing: run from the repository root", wasm.display());
    }
    std::fs::create_dir_all(out)?;
    let status = std::process::Command::new(tool("wasm-bindgen"))
        .args(["--target", "web", "--out-name", "mcview", "--no-typescript", "--out-dir"])
        .arg(out)
        .arg(wasm)
        .status()
        .context("running wasm-bindgen (cargo install wasm-bindgen-cli --version =0.2.129)")?;
    if !status.success() {
        bail!("wasm-bindgen failed");
    }
    for f in ["mcview.js", "mcview_bg.wasm"] {
        let p = out.join(f);
        println!("{} ({} KB)", p.display(), std::fs::metadata(&p)?.len() / 1024);
    }
    // the bundle is flat (the site copies the files of web/viewer/, not its folders): wasm-bindgen's
    // snippets move next to mcview.js and its imports follow
    let snippets = out.join("snippets");
    if snippets.is_dir() {
        let mut js = std::fs::read_to_string(out.join("mcview.js"))?;
        let mut stack = vec![snippets.clone()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir)?.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                let rel = p.strip_prefix(out).unwrap_or(&p).to_string_lossy().replace('\\', "/");
                let flat = format!("mcview-{}", p.file_name().unwrap_or_default().to_string_lossy());
                std::fs::rename(&p, out.join(&flat))?;
                // the module's import statement follows the file; the wasm's import object keeps
                // the snippet's own name as its key (the name the wasm imports it by)
                for q in ['"', '\''] {
                    js = js.replace(&format!("from {q}./{rel}{q}"), &format!("from {q}./{flat}{q}"));
                }
                println!("{} (from {rel})", out.join(&flat).display());
            }
        }
        std::fs::write(out.join("mcview.js"), js)?;
        std::fs::remove_dir_all(&snippets)?;
    }
    // the video recorder's encoder (vendored mediabunny, MPL-2.0: its license beside it), imported
    // by the viewer only when a recording starts; `.js` (every static server sends it as
    // JavaScript, which a module import requires; some send `.mjs` as bytes)
    let vendor = Path::new("crates/mcview-web/vendor/mediabunny");
    std::fs::copy(vendor.join("mediabunny.min.mjs"), out.join("mediabunny.min.js")).context("copying the vendored mediabunny")?;
    std::fs::copy(vendor.join("LICENSE"), out.join("mediabunny.LICENSE.txt"))?;
    println!("{} ({} KB, MPL-2.0: mediabunny.LICENSE.txt)", out.join("mediabunny.min.js").display(), std::fs::metadata(out.join("mediabunny.min.js"))?.len() / 1024);
    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The client jar of `version`, downloaded (alone, no libraries or assets) when missing.
fn client_jar(version: &str) -> Result<PathBuf> {
    let jar = mctas::paths::version_dir(version).join("client.jar");
    if jar.exists() {
        return Ok(jar);
    }
    let v = mctas::setup::version_json(version)?;
    let c = &v["downloads"]["client"];
    let url = c["url"].as_str().context("no client download in the version json")?;
    mctas::net::download(url, &jar, mctas::net::Checksum::Sha1(c["sha1"].as_str().unwrap_or_default()))?;
    Ok(jar)
}

fn cmd_reference(out: &Path, version: &str, names: &[String]) -> Result<()> {
    let jar = client_jar(version)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&jar)?).context("opening the client jar")?;
    std::fs::create_dir_all(out)?;
    let prefix = "assets/minecraft/textures/block/";
    let mut n = 0;
    for i in 0..zip.len() {
        let mut f = zip.by_index(i)?;
        let Some(rest) = f.name().strip_prefix(prefix).map(str::to_string) else { continue };
        let Some(stem) = rest.strip_suffix(".png") else { continue };
        if stem.contains('/') || (!names.is_empty() && !names.iter().any(|w| w == stem)) {
            continue;
        }
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut bytes)?;
        std::fs::write(out.join(&rest), bytes)?;
        n += 1;
    }
    println!("{n} block textures of {version} in {} (local reference only: never commit them)", out.display());
    Ok(())
}

fn cmd_layout(url: &str, seconds: u64, window: &str, scale: f32, min_ticks: u32, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut child = std::process::Command::new(browser)
        .args(["--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--enable-logging=stderr", "--v=0", "--remote-debugging-port=0"])
        .arg(format!("--window-size={window}"))
        .arg(format!("--force-device-scale-factor={scale}"))
        .arg(&page)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("starting {}", browser.display()))?;
    let stderr = child.stderr.take().context("no stderr")?;
    let reader = std::thread::spawn(move || {
        let mut lines = Vec::new();
        for l in std::io::BufRead::lines(std::io::BufReader::new(stderr)).map_while(Result::ok) {
            if let Some(i) = l.find("mcview: layout ") {
                let rest = &l[i + "mcview: layout ".len()..];
                lines.push(rest.split('"').next().unwrap_or(rest).to_string());
            }
        }
        lines
    });
    std::thread::sleep(std::time::Duration::from_secs(seconds));
    let _ = child.kill();
    let _ = child.wait();
    let lines = reader.join().map_err(|_| anyhow::anyhow!("log reader panicked"))?;
    // "view 1 tick 12 el x,y,w,h bar ... label ... status ..."
    let mut views: std::collections::BTreeMap<String, (String, u32, u32, usize, usize)> = Default::default();
    for l in &lines {
        let w: Vec<&str> = l.split_whitespace().collect();
        if w.len() < 4 || w[0] != "view" || w[2] != "tick" {
            continue;
        }
        let tick: u32 = w[3].parse().unwrap_or(0);
        let rects = w[4..].join(" ");
        let e = views.entry(w[1].to_string()).or_insert((rects.clone(), tick, tick, 0, 0));
        e.1 = e.1.min(tick);
        e.2 = e.2.max(tick);
        e.3 += 1;
        if rects != e.0 {
            e.4 += 1;
            println!("view {} moved at tick {tick}: {rects} (first: {})", w[1], e.0);
        }
    }
    if views.is_empty() {
        bail!("no layout lines: no live viewer drew (is the page right, and served?)");
    }
    let mut bad = 0;
    for (id, (rects, t0, t1, n, moved)) in &views {
        println!("view {id}: {n} frames, ticks {t0}..{t1}, {moved} moved; {rects}");
        if *moved > 0 || t1 - t0 < min_ticks {
            bad += 1;
        }
    }
    if bad > 0 {
        bail!("{bad} viewers moved or ran fewer than {min_ticks} ticks");
    }
    println!("web layout: {} viewers, nothing moved over {min_ticks}+ ticks at {window}", views.len());
    Ok(())
}

/// A circuit's judge stimulus and phases as the site emits them (`data-stimulus`, `data-phases`),
/// from the circuit crate; empty for other designs.
fn circuit_attrs(d: &Path) -> String {
    let Ok(c) = circuit::Circuit::load(d) else { return String::new() };
    let Some(kind) = c.kind.clone() else { return String::new() };
    if matches!(kind, circuit::Kind::FlyingMachine) || kind.is_shulker() {
        return String::new();
    }
    let gap = circuit::verify::sizing(&c, 3).unwrap_or(circuit::check::Sizing::default().gap);
    let s = circuit::check::stimulus(&c, &kind, &circuit::check::Sizing { gap });
    if c.inputs.is_empty() && s.events.is_empty() {
        return String::new();
    }
    let names: Vec<String> = c.inputs.iter().map(|i| i.name.clone()).collect();
    let phases: Vec<String> = s.phases.iter().map(|(a, b, l)| format!("{a}-{b} {l}")).collect();
    let mut out = String::new();
    if !c.inputs.is_empty() {
        out.push_str(&format!(" data-stimulus=\"{}\"", html_escape(&s.text(&names))));
    }
    let events = events_text(&c, &s);
    if !events.is_empty() {
        out.push_str(&format!(" data-events=\"{}\"", html_escape(&events)));
    }
    // a strength input's values (the hex kinds: the judge's chest fills)
    if !s.levels.is_empty() {
        let levels: Vec<String> = s.levels.iter().filter_map(|(t, i, v)| names.get(*i).map(|n| format!("{t} {n} {v}"))).collect();
        out.push_str(&format!(" data-levels=\"{}\"", html_escape(&levels.join("; "))));
    }
    out.push_str(&format!(" data-phases=\"{}\"", html_escape(&phases.join("; "))));
    out
}

/// A schedule's judge actions at cells (`Schedule::events`: a BUD switch's target and footprint
/// probes) in design coordinates: (frame, cell, op, state).
fn cell_events(c: &circuit::Circuit, s: &circuit::stimulus::Schedule) -> Vec<(u32, [i32; 3], &'static str, Option<String>)> {
    use circuit::stimulus::{EventAt, TargetOp};
    let probes = if s.events.iter().any(|e| matches!(e.1, EventAt::Probe(_))) { c.probe_cells() } else { Vec::new() };
    let mut v = Vec::new();
    for (t, at, op) in &s.events {
        let pos = match at {
            EventAt::Target(i) => match c.targets.get(*i) {
                Some(tg) => tg.pos,
                None => continue,
            },
            EventAt::Probe(k) => match probes.get(*k) {
                Some(p) => *p,
                None => continue,
            },
            // the vibration test's game events change no block: nothing to show
            EventAt::Input(_) | EventAt::Cell(_) => continue,
        };
        let (name, state) = match op {
            TargetOp::Place(st) => ("place", Some(st.clone())),
            TargetOp::Clear => ("clear", None),
            TargetOp::Shape(st) => ("shape", Some(st.clone())),
            TargetOp::Update => ("update", None),
            TargetOp::GameEvent(_) => continue,
        };
        v.push((*t, pos, name, state));
    }
    v
}

/// `data-events`: `<frame> <op> <x> <y> <z> [<state>]; ...` (the site's format).
fn events_text(c: &circuit::Circuit, s: &circuit::stimulus::Schedule) -> String {
    cell_events(c, s)
        .iter()
        .map(|(t, p, op, st)| match st {
            Some(st) => format!("{t} {op} {} {} {} {st}", p[0], p[1], p[2]),
            None => format!("{t} {op} {} {} {}", p[0], p[1], p[2]),
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// The same events as the viewer replays them (through its `data-events` parser).
fn viewer_events(c: &circuit::Circuit, s: &circuit::stimulus::Schedule) -> Vec<(u32, [i32; 3], mcview::circuit::EventOp)> {
    mcview::circuit::parse_events(&events_text(c, s)).unwrap_or_default()
}

fn cmd_demo(designs: &[PathBuf], out: &Path, viewer: &Path, live: usize, autoplay: bool) -> Result<()> {
    std::fs::create_dir_all(out.join("designs"))?;
    std::fs::create_dir_all(out.join("viewer"))?;
    // the whole (flat) bundle: mcview.js, its wasm, its snippets, the recorder's encoder
    if !viewer.join("mcview.js").is_file() {
        bail!("no {} (run `mctas web bundle` first)", viewer.join("mcview.js").display());
    }
    for e in std::fs::read_dir(viewer).with_context(|| format!("reading {}", viewer.display()))?.flatten() {
        if e.path().is_file() {
            std::fs::copy(e.path(), out.join("viewer").join(e.file_name()))?;
        }
    }
    let mut lives = String::new();
    let mut thumbs = String::new();
    for (i, d) in designs.iter().enumerate() {
        let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
        std::fs::copy(d, out.join("designs").join(&name)).with_context(|| format!("copying {}", d.display()))?;
        let src = html_escape(&format!("designs/{name}"));
        let label = html_escape(name.trim_end_matches(".txt"));
        if i < live {
            lives.push_str(&format!("<section><h2>{label}</h2><div class=\"live\" data-mcview=\"full\" data-src=\"{src}\"{}{}><p class=\"fallback\">loading {label}</p></div></section>\n", if autoplay { " data-autoplay" } else { "" }, circuit_attrs(d)));
        }
        thumbs.push_str(&format!("<a class=\"card\" href=\"#\" title=\"{label}\"><div class=\"thumb\" data-mcview=\"thumb\" data-src=\"{src}\"><p class=\"fallback\">{label}</p></div><span>{label}</span></a>\n"));
    }
    let page = format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>minecraft-tas viewer demo</title>
<style>
body {{ font: 14px system-ui, sans-serif; margin: 24px; background: #1d1f24; color: #e8e8e8; }}
h1 {{ font-size: 20px; }} h2 {{ font-size: 15px; margin: 18px 0 6px; }}
.live {{ width: 100%; height: 480px; background: #2a2d33; border-radius: 6px; overflow: hidden; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, 240px); gap: 12px; }}
.card {{ color: inherit; text-decoration: none; font-size: 12px; }}
.thumb {{ width: 240px; height: 180px; background: #2a2d33; border-radius: 6px; overflow: hidden; }}
.card span {{ display: block; margin-top: 4px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
.mcview-ready .fallback {{ display: none; }}
.mcview-error {{ outline: 2px solid #c33; }}
.fallback {{ margin: 0; padding: 8px; opacity: 0.6; }}
</style></head><body>
<h1>minecraft-tas: live contraptions</h1>
{lives}<h2>Thumbnails</h2>
<div class="grid">
{thumbs}</div>
<script type="module">import init from "./viewer/mcview.js"; init();</script>
</body></html>
"#
    );
    std::fs::write(out.join("index.html"), page)?;
    println!("{} ({} designs, {} live)", out.join("index.html").display(), designs.len(), live.min(designs.len()));
    Ok(())
}

/// A minimal static file server (GET only), one connection at a time.
fn cmd_serve(dir: &Path, port: u16) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("binding port {port}"))?;
    println!("serving {} at http://127.0.0.1:{port}/", dir.display());
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else { continue };
        let mut reader = BufReader::new(stream.try_clone()?);
        let mut first = String::new();
        if reader.read_line(&mut first).is_err() {
            continue;
        }
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
                break;
            }
        }
        let path = first.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
        let rel = path.trim_start_matches('/');
        let rel = if rel.is_empty() || rel.ends_with('/') { format!("{rel}index.html") } else { rel.to_string() };
        let file = dir.join(&rel);
        let ok = !rel.contains("..") && file.is_file();
        let (status, body) = if ok { ("200 OK", std::fs::read(&file).unwrap_or_default()) } else { ("404 Not Found", b"not found".to_vec()) };
        let ctype = match file.extension().and_then(|e| e.to_str()) {
            Some("html") => "text/html; charset=utf-8",
            Some("js") => "text/javascript",
            Some("wasm") => "application/wasm",
            Some("png") => "image/png",
            Some("json") => "application/json",
            _ => "text/plain; charset=utf-8",
        };
        let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-cache\r\n\r\n", body.len());
        let _ = stream.write_all(&body);
    }
    Ok(())
}

fn cmd_shot(designs: &[PathBuf], out: &Path, o: mcview::tools::ShotOptions, procedural: bool) -> Result<()> {
    if designs.is_empty() {
        bail!("no design given");
    }
    let atlas = atlas(procedural);
    let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
    let to_dir = designs.len() > 1 || out.is_dir();
    if to_dir {
        std::fs::create_dir_all(out)?;
    }
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        let path = if to_dir { out.join(format!("{}.png", d.file_stem().unwrap_or_default().to_string_lossy())) } else { out.to_path_buf() };
        let t = std::time::Instant::now();
        match mcview::tools::shot(&mut off, &atlas, &text, &o) {
            Ok((rgba, what)) => {
                mcview::render::headless::write_png(&path, o.width, o.height, &rgba).map_err(anyhow::Error::msg)?;
                println!("{} -> {} ({what}, {:.0} ms)", d.display(), path.display(), t.elapsed().as_secs_f64() * 1000.0);
            }
            Err(e) => println!("{}: ERROR {e}", d.display()),
        }
    }
    Ok(())
}

/// Blocks and ports of both parsers, normalized through redsim's block parser when it knows the
/// state (property order and defaults do not matter), else the raw strings.
fn same_parse(text: &str) -> Result<(), String> {
    let a = mctas::design::Design::parse(text).map_err(|e| format!("mctas: {e:#}"))?;
    let b = mcview::Design::parse(text).map_err(|e| format!("mcview: {e}"))?;
    let norm = |s: &str| redsim::Block::parse(s).map(|x| x.to_string()).unwrap_or_else(|_| s.to_string());
    if a.blocks.len() != b.blocks.len() {
        return Err(format!("{} blocks vs {}", a.blocks.len(), b.blocks.len()));
    }
    for ((pa, sa), (pb, sb)) in a.blocks.iter().zip(b.blocks.iter()) {
        if pa != pb || norm(sa) != norm(sb) {
            return Err(format!("block {pa:?} {sa} vs {pb:?} {sb}"));
        }
    }
    for (k, p) in &a.ports {
        if b.port(k) != Some(*p) {
            return Err(format!("port {k}: {p:?} vs {:?}", b.port(k)));
        }
    }
    if a.entities.len() != b.entities.len() || a.block_entities.len() != b.block_entities.len() {
        return Err("entities / nbt lines differ".into());
    }
    Ok(())
}

fn cmd_check(designs: &[PathBuf], horizon: u32, quiet: bool, wasm: Option<&Path>) -> Result<()> {
    let mut probe = match wasm {
        Some(p) => Some(mcview::tools::WasmProbe::load(&std::fs::read(p).with_context(|| format!("reading {}", p.display()))?).map_err(anyhow::Error::msg)?),
        None => None,
    };
    let (mut ok, mut flyers, mut circuits, mut bad, mut skipped, mut tnts) = (0, 0, 0, 0, 0, 0);
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        if let Err(e) = same_parse(&text) {
            bad += 1;
            println!("FAIL {} parse: {e}", d.display());
            continue;
        }
        let fly = is_flying_machine(&text);
        // circuits judged under a stimulus: inputs, BUD targets (data-events); shulker designs run
        // with the judge's chests (`shulker_pair`); designs with TNT run with exploding TNT
        // (`tnt_pair`)
        let shulker = !fly && text.lines().any(|l| l.starts_with("circuit: shulker-"));
        // item sorters run with the judge's chests and stream (`sorter_pair`)
        let sorter = !fly && !shulker && text.lines().any(|l| l.starts_with("circuit: sorter"));
        let tnt = !fly && !shulker && !sorter && mcview::design::Design::parse(&text).is_ok_and(|d| mcview::session::is_tnt_design(&d));
        let circ = !fly && !shulker && !sorter && !tnt && text.lines().any(|l| l.starts_with("circuit:")) && text.lines().any(|l| l.starts_with("port input") || l.starts_with("port target"));
        let run = if fly {
            fly_pair(&text, probe.as_mut())
        } else if shulker {
            shulker_pair(&text, probe.as_mut())
        } else if sorter {
            sorter_pair(&text, probe.as_mut())
        } else if tnt {
            tnt_pair(&text, probe.as_mut())
        } else if circ {
            circuit_pair(&text)
        } else {
            clock_pair(&text, horizon, probe.as_mut())
        };
        let line = match run {
            Ok(Some(l)) => l,
            Ok(None) => {
                skipped += 1;
                if !quiet {
                    println!("ok   {} (parse only)", d.display());
                }
                continue;
            }
            Err(e) => {
                bad += 1;
                println!("FAIL {} {e}", d.display());
                continue;
            }
        };
        // what the viewer draws is redsim's state, every tick (circuits: in `circuit_pair`)
        if !circ {
            let busy = tnt || shulker || sorter || mcview::design::Design::parse(&text).is_ok_and(|d| !d.entities.is_empty());
            if let Err(e) = drawn_pass(&text, if busy { TNT_TICKS } else { DRAWN_TICKS }) {
                bad += 1;
                println!("FAIL {} {e}", d.display());
                continue;
            }
        }
        if fly {
            flyers += 1;
        } else if tnt {
            tnts += 1;
        } else if circ || shulker || sorter {
            circuits += 1;
        } else {
            ok += 1;
        }
        if !quiet {
            println!("ok   {} {line}", d.display());
        }
    }
    println!(
        "web check: {ok} clocks (same period as clock sim), {flyers} flying machines (same frames and end as circuit fly sim), {circuits} circuits (same frames as circuit sim under the judge's stimulus) and {tnts} TNT designs (same blocks and entities as redsim with exploding TNT) agree{}, and draw exactly redsim's blocks and entities every tick, {skipped} parse-only, {bad} failures",
        if probe.is_some() { " (clocks, flying machines, shulker, sorter and TNT designs in the native and the wasm viewer)" } else { "" }
    );
    if bad > 0 {
        bail!("{bad} failures");
    }
    Ok(())
}

/// A design whose `circuit:` header says flying machine.
fn is_flying_machine(text: &str) -> bool {
    mcview::Design::parse(text).is_ok_and(|d| d.meta.get("circuit").is_some_and(|k| k.starts_with("flying-machine")))
}

/// A flying machine in `circuit fly sim` and in the viewer (and the wasm when given): the same
/// end tick (the corridor's end, rest, or a break) and the same frames up to it.
fn fly_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use circuit_fly::report::{End, Verdict};
    let m = circuit_fly::machine::Machine::parse(text).map_err(|e| format!("{e:#}"))?;
    let o = circuit_fly::sim::SimOptions::default();
    let r = circuit_fly::sim::simulate(&m, &o);
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    if r.verdict == Verdict::Unmodeled {
        return if s.simulated() { Err(format!("circuit fly sim: unmodeled ({}) but the viewer simulates it", r.why)) } else { Ok(None) };
    }
    let end = match r.end {
        End::Idle(t) | End::Edge(t) | End::Horizon(t) | End::Drop(t, _) | End::Error(t, _) => t,
    };
    let frames = circuit_fly::sim::frames(&m, &o, end)?;
    let ticks = frames.len().saturating_sub(1) as u32;
    let theirs = frames.iter().fold(0xcbf29ce484222325u64, |h, f| mcview::session::frame_hash(h, f));
    let mine_end = s.end_tick.ok_or("the viewer found no corridor end")?;
    if mine_end != end {
        return Err(format!("circuit fly sim ends at tick {end} ({:?}) but the viewer at {mine_end}", r.end));
    }
    let ours = s.frames_digest(ticks);
    if ours != theirs {
        return Err(format!("frames 0..={ticks} differ from circuit fly sim's"));
    }
    if let Some(p) = probe {
        let wend = p.fly_end(text)?;
        if wend != Some(end) {
            return Err(format!("circuit fly sim ends at tick {end} but the wasm viewer at {wend:?}"));
        }
        if p.frames_digest(text, ticks)? != theirs {
            return Err(format!("the wasm viewer's frames 0..={ticks} differ from circuit fly sim's"));
        }
    }
    Ok(Some(format!("{} {}, ends at tick {end}, {} frames agree", r.verdict.word(), r.speed_text(), ticks + 1)))
}

/// A circuit's run under its judge's stimulus in the viewer and in `circuit sim`
/// (`circuit::run::sim_trace`, the judge's consumers included): every cell's full state at every
/// frame must agree. None when the design is no judged circuit.
fn circuit_pair(text: &str) -> Result<Option<String>, String> {
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    if !d.meta.contains_key("circuit") {
        return Ok(None);
    }
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(kind) = c.kind.clone() else { return Ok(None) };
    if matches!(kind, circuit::Kind::FlyingMachine) || c.kind.as_ref().is_some_and(|k| k.is_shulker()) {
        return Ok(None);
    }
    let gap = circuit::verify::sizing(&c, 3).unwrap_or(circuit::check::Sizing::default().gap);
    let s = circuit::check::stimulus(&c, &kind, &circuit::check::Sizing { gap });
    if c.inputs.is_empty() && s.events.is_empty() {
        return Ok(None);
    }
    let trace = match circuit::run::sim_trace(&c, &s, 3) {
        Ok(t) => t,
        Err(why) => {
            // redsim cannot run it (a block or a random draw it does not model): the viewer must
            // say so too: shown as built, or stopping where redsim stops under the judge's
            // stimulus (vjeux: only what redsim simulates)
            let mut v = mcview::Session::new(text, &mcview::Options::default())?;
            if !v.simulated() {
                return Ok(None);
            }
            v.attach_demo(mcview::circuit::Schedule { edges: s.edges.clone(), ticks: s.ticks, events: viewer_events(&c, &s), levels: s.levels.clone() });
            for _ in 0..=s.ticks {
                if let Some(h) = &v.halted {
                    return Ok(Some(format!("{} not simulated after {h} (circuit sim: {why})", kind.name())));
                }
                v.step();
            }
            return Err(format!("circuit sim: {why}; the viewer runs on"));
        }
    };
    let (from, to) = circuit::run::watch_box(&c, 3);
    let inside = |p: &[i32; 3]| (0..3).all(|k| p[k] >= from[k] && p[k] <= to[k]);
    let mut session = mcview::Session::new(text, &mcview::Options::default())?;
    if !session.simulated() {
        return Err(format!("circuit sim runs it but the viewer does not: {}", session.unsimulated.clone().unwrap_or_default()));
    }
    session.attach_demo(mcview::circuit::Schedule { edges: s.edges.clone(), ticks: s.ticks, events: viewer_events(&c, &s), levels: s.levels.clone() });
    // the judge's input devices (R7: a driven input's repeater, support and switch) are not the
    // circuit's: circuit sim leaves them out of its frames
    let judge: Vec<mcview::design::Pos> = session
        .circuit_inputs
        .iter()
        .filter_map(|i| match i.how {
            mcview::circuit::InputHow::Driven { dir, .. } => {
                let (r, sup, sw) = mcview::circuit::driver_cells(i.pos, dir);
                Some([r, sup, sw])
            }
            _ => None,
        })
        .flatten()
        .collect();
    let frames = trace.cells.len();
    for (f, cells) in trace.cells.iter().enumerate() {
        let theirs: Vec<(mcview::design::Pos, &str)> = cells.iter().filter(|(p, _)| inside(p)).map(|(p, i)| (*p, trace.names[*i as usize].as_str())).collect();
        let mine_all = session.cell_states();
        let mine: Vec<(mcview::design::Pos, &str)> = mine_all.iter().filter(|(p, _)| inside(p) && !judge.contains(p)).map(|(p, s)| (*p, s.as_str())).collect();
        if theirs != mine {
            let only = |a: &Vec<(mcview::design::Pos, &str)>, b: &Vec<(mcview::design::Pos, &str)>| a.iter().filter(|x| !b.contains(x)).take(6).map(|(p, s)| format!("{p:?} {s}")).collect::<Vec<_>>().join(", ");
            let diff = format!("circuit sim only: {}; the viewer only: {}", only(&theirs, &mine), only(&mine, &theirs));
            let hows = |a: String, b: String| if a == b { String::new() } else { format!("; inputs: circuit sim {a}, the viewer {b}") };
            return Err(format!("frame {f} differs from circuit sim's ({diff}){}", hows(format!("{:?}", c.inputs.iter().map(|i| format!("{} {:?}", i.name, i.how)).collect::<Vec<_>>()), format!("{:?}", session.circuit_inputs.iter().map(|i| format!("{} {:?}", i.name, i.how)).collect::<Vec<_>>()))));
        }
        // and what the viewer draws is that state (vjeux: nothing animated by the viewer itself)
        session.drawn_matches_simulation().map_err(|e| format!("frame {f}: drawn differs from redsim: {e}"))?;
        session.step();
    }
    Ok(Some(format!("{} {frames} frames agree", kind.name())))
}

/// Ticks of the drawn-state pass (`drawn_pass`) for a design without entities, TNT or shulker
/// boxes (their runs: TNT_TICKS).
const DRAWN_TICKS: u32 = 120;

/// Everything the viewer draws is redsim's state (`Session::drawn_matches_simulation`: the
/// blocks, and the entities and falling blocks at redsim's positions) on every tick of a run of
/// `ticks` as placed (a circuit's run under the judge's stimulus: in `circuit_pair`).
fn drawn_pass(text: &str, ticks: u32) -> Result<(), String> {
    let mut s = mcview::Session::new(text, &mcview::Options::default())?;
    // no ground but real blocks (vjeux 17:53Z): the viewer's look draws no decorative ground, and
    // a TNT duper's ground is checked cell by cell below (`Floor::now` against the world)
    if mcview::scene::viewer_look(&s).ground.is_some() {
        return Err("the viewer draws a ground that is not redsim blocks".into());
    }
    for _ in 0..=ticks {
        s.drawn_matches_simulation().map_err(|e| format!("tick {}: drawn differs from redsim: {e}", s.tick))?;
        if s.halted.is_some() {
            break;
        }
        s.step();
    }
    Ok(())
}

/// Ticks a TNT design runs in `web check` (TNT primed early explode by ~85; several fuses).
const TNT_TICKS: u32 = 400;

/// A design with TNT (`mcview::session::is_tnt_design`) in the viewer (redsim's TNT entities and
/// explosions: `explosion::explode_on`, seed 0, the viewer box's chunks) and in a reference run
/// built from the repo's tools (`clock::sim::drive_custom`: the clock judge's placement and
/// inputs in the viewer's box, the same configuration): the same blocks and entities (positions
/// and motions, bit for bit) every tick for TNT_TICKS ticks as placed, and the same blocks in the
/// wasm viewer when given. None when redsim does not run it (the viewer shows it as built).
fn tnt_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    if !s.simulated() {
        return Ok(None);
    }
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    // a TNT circuit as its judge builds it: its consumers and its driven inputs' devices (R7)
    let c = match circuit::Circuit::from_design(d.clone()) {
        Ok(cc) => match cc.with_consumers() {
            Ok(cc) => cc.with_drivers().clock,
            Err(_) => cc.with_drivers().clock,
        },
        Err(_) => match clock::Clock::from_design(d) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        },
    };
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let chunks = redsim::redstone::shulker::chunk_range(from, to);
    // a TNT duper stands on its judge's ground (`Circuit::ground_blocks`, laid raw once the design
    // stands, like `clock::sim::drive_tnt`), in the judge's box around it
    let ground: Vec<(redsim::BlockPos, redsim::Block)> = s.floor.as_ref().map(|f| f.blocks()).unwrap_or_default();
    let (mut h, mut hb): (u64, u64) = (0xcbf29ce484222325, 0xcbf29ce484222325);
    let mut blasts = 0usize;
    let mut destroyed = 0usize;
    // the viewer stops at redsim's first error (something it does not model: vjeux, only what
    // redsim simulates exactly is shown): the frames before it are compared
    let mut stop: Option<(u32, String)> = None;
    clock::sim::drive_custom(&c, from, to, TNT_TICKS, |w| {
        if !ground.is_empty() {
            w.place_raw(&ground);
        }
        redsim::explosion::explode_on(w, 0, chunks)
    }, |t, w| {
        if stop.is_none()
            && let Some(e) = w.errors.first()
        {
            stop = Some((t, e.clone()));
        }
        if stop.is_some() {
            return;
        }
        let b = w.blocks();
        h = mcview::session::frame_hash(h, &b);
        h = mcview::session::entity_hash(h, w);
        hb = mcview::session::frame_hash(hb, &b);
        blasts = w.explosions.len();
        destroyed = w.explosions.iter().map(|x| x.destroyed.len()).sum();
    })?;
    if s.frames_entities_digest(TNT_TICKS) != h {
        return Err(format!("frames 0..={TNT_TICKS} (blocks and entities) differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, TNT_TICKS)? != hb
    {
        return Err(format!("the wasm viewer's frames 0..={TNT_TICKS} differ from the reference run's"));
    }
    let shown = match &stop {
        Some((t, e)) => format!("shown up to tick {t}, where redsim stops: {e}"),
        None => format!("{TNT_TICKS} ticks"),
    };
    Ok(Some(format!("TNT design: {shown} agree (blocks and entities; {blasts} explosions, {destroyed} blocks destroyed)")))
}

/// Ticks a shulker design runs in `web check` (3 boxes and more of unstackables).
const SHULKER_TICKS: u32 = 1000;

/// A shulker loader or unloader in the viewer (redsim's driver of the judge's chests: endless,
/// unstackables unless it sorts) and in a reference run built from the repo's tools: the design
/// placed by the clock judge's placement (`clock::sim::world_for`) in the viewer's box, the
/// chests on `circuit::Circuit`'s ports, the settle, the supply, then the judge's visit before
/// every tick: the same frames for SHULKER_TICKS ticks as placed (the start lever off, as the
/// viewer starts; in the wasm viewer too when given) and, with a start lever, after the visitor
/// turns it on at once (the judge's run). None when the design is no shulker design, or when
/// redsim cannot follow its run (the viewer must then show it as built).
fn shulker_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use redsim::redstone::shulker::{Chests, SETTLE};
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(kind) = c.kind.clone() else { return Ok(None) };
    let (loader, stack) = match kind {
        circuit::Kind::ShulkerLoader(s) => (true, s),
        circuit::Kind::ShulkerUnloader(s) => (false, s),
        _ => return Ok(None),
    };
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let bp = |p: [i32; 3]| redsim::BlockPos::new(p[0], p[1], p[2]);
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let role = |r: &str| -> Vec<redsim::BlockPos> {
        let mut v: Vec<[i32; 3]> = c.stores.iter().filter(|s| s.role == r).map(|s| s.pos).collect();
        v.sort();
        v.into_iter().map(bp).collect()
    };
    let mut outs: Vec<[i32; 3]> = c.outputs.iter().map(|o| o.pos).collect();
    outs.sort();
    let rejects = role("reject");
    let item = match (rejects.is_empty(), stack) {
        (true, _) | (false, 1) => "iron_shovel",
        (false, 16) => "snowball",
        _ => "cobblestone",
    };
    let item = redsim::redstone::container::item_id(item).ok_or("no such item")?;
    let levers: Vec<[i32; 3]> = c.inputs.iter().filter(|i| i.how == circuit::InputHow::Lever && !(i.name == "mode" || i.name.starts_with("mode."))).map(|i| i.pos).collect();
    // the judge's sandbox (its watch box: seed and forced chunks), from the circuit crate; the
    // viewer computes it from its own design (`mcview::circuit::judge_box`): the same box, or the
    // viewer would draw another level random than the judge's
    let judge = circuit::run::watch_box(&c, redsim::redstone::shulker::JUDGE_MARGIN);
    let viewer_box = mcview::design::Design::parse(text).ok().and_then(|d| mcview::circuit::judge_box(&d));
    if viewer_box != Some(judge) {
        return Err(format!("the viewer's judge box {viewer_box:?} is not the judge's watch box {judge:?}"));
    }
    // the reference run, its start lever on (`lever`) or off: its frames' hash, None when redsim
    // meets something it does not model
    let reference = |lever: bool| -> Result<Option<u64>, String> {
        let mut w = clock::sim::world_for(&c.clock, from, to)?;
        redsim::redstone::shulker::judge_sandbox(&mut w, judge.0, judge.1);
        let mut ch = Chests::endless(loader, item, role("items"), role("boxes"), outs.iter().map(|p| bp(*p)).collect(), rejects.clone());
        ch.place(&mut w);
        for _ in 0..SETTLE {
            w.tick();
        }
        ch.start(&mut w);
        if lever {
            for p in &levers {
                w.use_block(bp(*p));
            }
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for t in 0..=SHULKER_TICKS {
            if t > 0 {
                ch.visit(&mut w);
                w.tick();
            }
            if !w.errors.is_empty() {
                return Ok(None);
            }
            h = mcview::session::frame_hash(h, &w.blocks());
        }
        Ok(Some(h))
    };
    let (placed, started) = (reference(false)?, if levers.is_empty() { None } else { reference(true)? });
    if placed.is_none() || (!levers.is_empty() && started.is_none()) {
        return if s.simulated() { Err("the reference run meets something redsim does not model but the viewer simulates it".into()) } else { Ok(None) };
    }
    if !s.simulated() {
        return Err(format!("the viewer does not simulate it ({}) but the reference run does", s.unsimulated.clone().unwrap_or_default()));
    }
    let placed = placed.unwrap();
    if s.frames_digest(SHULKER_TICKS) != placed {
        return Err(format!("frames 0..={SHULKER_TICKS} as placed differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, SHULKER_TICKS)? != placed
    {
        return Err(format!("the wasm viewer's frames 0..={SHULKER_TICKS} differ from the reference run's"));
    }
    let mut lever_note = String::new();
    if let Some(theirs) = started {
        // the visitor turns the start lever on before the first tick
        let mut v = mcview::Session::new(text, &mcview::Options::default())?;
        for i in 0..v.circuit_inputs.len() {
            if levers.contains(&v.circuit_inputs[i].pos) {
                v.set_input(i, true);
            }
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for t in 0..=SHULKER_TICKS {
            if t > 0 {
                v.step();
            }
            h = mcview::session::frame_hash(h, &v.world.blocks());
        }
        if h != theirs {
            return Err(format!("frames 0..={SHULKER_TICKS} after its start lever differ from the reference run's"));
        }
        lever_note = ", and after its start lever".to_string();
    }
    Ok(Some(format!("{} {SHULKER_TICKS} ticks agree (the judge's chests, endless){lever_note}", kind.name())))
}

/// Ticks an item sorter runs in `web check` (its judge visits every 20).
const SORTER_TICKS: u32 = 1200;

/// An item sorter in the viewer (redsim's chest driver: the judge's stream fed, the sinks emptied
/// every `circuit::shulker::SEG` gt) against a reference run built from the repo's tools: the
/// design placed by the clock judge's placement (`clock::sim::world_for`) in the viewer's box, a
/// chest on each of `circuit::Circuit`'s `items`, `pass` and output ports, the settle, the
/// judge's own base stream (`circuit::sorter::Scenario::base`) and visits: the same frames for
/// SORTER_TICKS ticks (in the wasm viewer too when given); and the items the viewer's sinks
/// receive visit by visit are those of the judge's redsim screen (`circuit::sorter::sim_run`'s
/// flow). None when the design is no sorter, or when redsim cannot follow its run (the viewer
/// must then show it as built).
fn sorter_pair(text: &str, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    use redsim::redstone::container as rc;
    let d = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    let c = circuit::Circuit::from_design(d).map_err(|e| format!("{e:#}"))?;
    let Some(circuit::Kind::Sorter(item)) = c.kind.clone() else { return Ok(None) };
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let bp = |p: [i32; 3]| redsim::BlockPos::new(p[0], p[1], p[2]);
    let (lo, size) = (s.world.min, s.world.size);
    let (from, to) = ([lo.x, lo.y, lo.z], [lo.x + size[0] - 1, lo.y + size[1] - 1, lo.z + size[2] - 1]);
    let role = |r: &str| -> Vec<[i32; 3]> {
        let mut v: Vec<[i32; 3]> = c.stores.iter().filter(|s| s.role == r).map(|s| s.pos).collect();
        v.sort();
        v
    };
    let mut outs: Vec<[i32; 3]> = c.outputs.iter().map(|o| o.pos).collect();
    outs.sort();
    let sources: Vec<redsim::BlockPos> = role("items").into_iter().map(bp).collect();
    let sinks: Vec<redsim::BlockPos> = role("pass").into_iter().chain(outs).map(bp).collect();
    let sc = circuit::sorter::Scenario::base(&item, &circuit::sorter::design_kinds(&c)).map_err(|e| format!("{e:#}"))?;
    let mut stream = Vec::new();
    for st in &sc.stream {
        stream.push(rc::Stack::new(rc::item_id(&st.id).ok_or_else(|| format!("redsim has no item {}", st.id))?, st.count as u8));
    }
    let queue: Vec<rc::Stack> = (0..mcview::circuit::SORTER_PASSES).flat_map(|_| stream.iter().copied()).collect();
    let seg = circuit::shulker::SEG;
    if seg != mcview::circuit::SORTER_SEG {
        return Err(format!("the viewer's sorter visits every {} gt, the judge's every {seg}", mcview::circuit::SORTER_SEG));
    }
    // the reference run: its frames' hash and the items its sinks got at each visit
    let mut w = clock::sim::world_for(&c.clock, from, to)?;
    let chest = redsim::Block::parse(circuit::shulker::CHEST)?;
    for p in sources.iter().chain(&sinks) {
        w.set_block(*p, chest, redsim::world::flags::ALL);
    }
    for _ in 0..c.clock.free_from() + circuit::stimulus::SETTLE {
        w.tick();
    }
    let mut ch = redsim::redstone::shulker::Chests::judged(true, sources.clone(), Vec::new(), sinks.clone(), Vec::new(), Vec::new(), queue);
    ch.start(&mut w);
    let count = |w: &redsim::World| -> u64 { sinks.iter().map(|p| redsim::redstone::shulker::chest_stacks(w, *p).iter().map(|s| s.count as u64).sum::<u64>()).sum() };
    let mut h: u64 = 0xcbf29ce484222325;
    let mut flow: Vec<(u32, u64)> = Vec::new();
    let mut total = 0u64;
    for t in 0..=SORTER_TICKS {
        if t > 0 {
            if t > 1 && (t - 1) % seg == 0 {
                let got = count(&w);
                if got > 0 {
                    total += got;
                    flow.push((t - 1, total));
                }
                ch.visit(&mut w);
            }
            w.tick();
        }
        if !w.errors.is_empty() {
            return if s.simulated() { Err("the reference run meets something redsim does not model but the viewer simulates it".into()) } else { Ok(None) };
        }
        h = mcview::session::frame_hash(h, &w.blocks());
    }
    if !s.simulated() {
        return Err(format!("the viewer does not simulate it ({}) but the reference run does", s.unsimulated.clone().unwrap_or_default()));
    }
    if s.frames_digest(SORTER_TICKS) != h {
        return Err(format!("frames 0..={SORTER_TICKS} differ from the reference run's"));
    }
    if let Some(p) = probe
        && p.frames_digest(text, SORTER_TICKS)? != h
    {
        return Err(format!("the wasm viewer's frames 0..={SORTER_TICKS} differ from the reference run's"));
    }
    // the judge's own redsim screen: the same deliveries at the same visits
    let log = circuit::sorter::sim_run(&c, &sc, 4)?;
    let theirs: Vec<(u32, u64)> = log.flow.iter().copied().filter(|(t, _)| *t < SORTER_TICKS).collect();
    if theirs != flow {
        return Err(format!("the sinks' deliveries {:?} differ from the judge's redsim screen's {:?}", &flow[..flow.len().min(6)], &theirs[..theirs.len().min(6)]));
    }
    if total == 0 {
        return Err(format!("nothing reached its sinks in {SORTER_TICKS} ticks"));
    }
    Ok(Some(format!("sorter {SORTER_TICKS} ticks agree (the judge's chests and stream; {total} items delivered, as the judge's redsim screen)")))
}

/// A clock's numbers from both detectors (and the wasm's when given): Some(summary) when they
/// agree, None when the design is not a redsim clock, Err when they differ.
fn clock_pair(text: &str, horizon: u32, probe: Option<&mut mcview::tools::WasmProbe>) -> Result<Option<String>, String> {
    let design = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    if design.ports.contains_key("door") {
        return Ok(None);
    }
    let Ok(c) = clock::Clock::from_design(design) else { return Ok(None) };
    let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon, margin: 4 });
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let mine = s.period(horizon);
    let wasm = match probe {
        Some(p) => Some(p.period(text, horizon)?),
        None => None,
    };
    match (r.verdict, mine) {
        (clock::sim::Verdict::Clock | clock::sim::Verdict::Stops | clock::sim::Verdict::Composite, Ok(p)) => {
            let theirs = (r.state_warmup, r.state_period, r.warmup, r.period);
            let ours = (p.state_warmup, p.state_period, p.warmup, p.period);
            if theirs != ours {
                return Err(format!("clock sim {theirs:?} vs viewer {ours:?}"));
            }
            if let Some(w) = wasm
                && w != Some(ours)
            {
                return Err(format!("clock sim and the native viewer {ours:?} vs the wasm viewer {w:?}"));
            }
            Ok(Some(format!("W{} P{} (state W{} P{})", p.warmup, p.period, p.state_warmup, p.state_period)))
        }
        (clock::sim::Verdict::Unmodeled | clock::sim::Verdict::Invalid | clock::sim::Verdict::NoRepeat, Err(_)) => match wasm {
            Some(Some(w)) => Err(format!("clock sim {} but the wasm viewer finds {w:?}", r.verdict.word())),
            _ => Ok(None),
        },
        // the clock judge's output rule (vjeux 2026-10-20: the output must be an empty cell where
        // the signal arrives) is a verdict on where it is read, not on the simulation: the
        // viewer runs the build as redsim does
        (clock::sim::Verdict::Invalid, Ok(p)) if r.why.contains("the output cell") => Ok(Some(format!("W{} P{} (state W{} P{}); clock sim INVALID by the output rule: {}", p.warmup, p.period, p.state_warmup, p.state_period, r.why))),
        (v, Ok(p)) => Err(format!("clock sim {} but the viewer finds W{} P{}", v.word(), p.state_warmup, p.state_period)),
        (v, Err(e)) => Err(format!("clock sim {} (W{} P{}) but the viewer: {e}", v.word(), r.state_warmup, r.state_period)),
    }
}
