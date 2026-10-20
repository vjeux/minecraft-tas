//! `mctas web ...`: the in-browser viewer's commands (docs/webrender.md). Argument parsing and
//! dispatch only (compiled unoptimized); the work is in `crates/mcview`.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::{Path, PathBuf};

mod cdp;

#[derive(Subcommand)]
pub enum WebCmd {
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
        #[arg(long, default_value_t = 8)]
        wait: u64,
        #[arg(long, default_value = "/usr/lib64/chromium-browser/headless_shell")]
        browser: PathBuf,
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
    },
}

pub fn run(_version: &str, cmd: WebCmd) -> Result<()> {
    match cmd {
        WebCmd::Shot { designs, out, width, height, tick, flips, yaw, pitch } => cmd_shot(&designs, &out, mcview::tools::ShotOptions { width, height, tick, flips, yaw, pitch }),
        WebCmd::Check { designs, horizon, quiet, wasm } => cmd_check(&designs, horizon, quiet, wasm.as_deref()),
        WebCmd::Bundle { out } => cmd_bundle(&out),
        WebCmd::Demo { designs, out, viewer, live, autoplay } => cmd_demo(&designs, &out, &viewer, live, autoplay),
        WebCmd::Serve { dir, port } => cmd_serve(&dir, port),
        WebCmd::Compare { designs, img_dir, out } => cmd_compare(&designs, &img_dir, &out),
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
        WebCmd::Click { url, window, scale, touch, wait, browser } => crate::cdp::cmd_click(&url, &window, scale, touch, wait, &browser),
        WebCmd::Layout { url, seconds, window, scale, min_ticks, browser } => cmd_layout(&url, seconds, &window, scale, min_ticks, &browser),
        WebCmd::Reference { out, version, names } => cmd_reference(&out, &version, &names),
        WebCmd::Sheet { pngs, out, scale, cols } => {
            let (rgba, w, h) = mcview::tools::sheet(&pngs, scale, cols).map_err(anyhow::Error::msg)?;
            mcview::render::headless::write_png(&out, w, h, &rgba).map_err(anyhow::Error::msg)?;
            println!("{} ({} textures)", out.display(), pngs.len());
            Ok(())
        }
        WebCmd::Strip { design, out, cols, rows, step, width, height } => {
            let atlas = mcview::atlas::Atlas::build();
            let mut off = mcview::render::headless::Offscreen::new(&atlas).map_err(anyhow::Error::msg)?;
            let text = std::fs::read_to_string(&design)?;
            let (rgba, w, h) = mcview::tools::strip(&mut off, &atlas, &text, cols, rows, step, width, height).map_err(anyhow::Error::msg)?;
            mcview::render::headless::write_png(&out, w, h, &rgba).map_err(anyhow::Error::msg)?;
            println!("{} ({cols}x{rows} frames, {step} gt apart)", out.display());
            Ok(())
        }
    }
}

fn cmd_compare(designs: &[PathBuf], img_dir: &Path, out: &Path) -> Result<()> {
    let atlas = mcview::atlas::Atlas::build();
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
        match mcview::tools::compare(&mut off, &atlas, &text, &bytes) {
            Ok(c) => {
                let path = out.join(format!("{stem}.cmp.png"));
                mcview::render::headless::write_png(&path, c.width, c.height, &c.rgba).map_err(anyhow::Error::msg)?;
                println!("{stem}: pixel diff {:.1}, coarse diff {:.1} ({}) -> {}", c.pixel_diff, c.coarse_diff, c.note, path.display());
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

fn cmd_demo(designs: &[PathBuf], out: &Path, viewer: &Path, live: usize, autoplay: bool) -> Result<()> {
    std::fs::create_dir_all(out.join("designs"))?;
    std::fs::create_dir_all(out.join("viewer"))?;
    for f in ["mcview.js", "mcview_bg.wasm"] {
        std::fs::copy(viewer.join(f), out.join("viewer").join(f)).with_context(|| format!("copying {} (run `mctas web bundle` first)", viewer.join(f).display()))?;
    }
    let mut lives = String::new();
    let mut thumbs = String::new();
    for (i, d) in designs.iter().enumerate() {
        let name = d.file_name().unwrap_or_default().to_string_lossy().to_string();
        std::fs::copy(d, out.join("designs").join(&name)).with_context(|| format!("copying {}", d.display()))?;
        let src = html_escape(&format!("designs/{name}"));
        let label = html_escape(name.trim_end_matches(".txt"));
        if i < live {
            lives.push_str(&format!("<section><h2>{label}</h2><div class=\"live\" data-mcview=\"full\" data-src=\"{src}\"{}><p class=\"fallback\">loading {label}</p></div></section>\n", if autoplay { " data-autoplay" } else { "" }));
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
<h1>minecraft-tas: contraptions rendered and simulated in the browser (redsim + wgpu, Rust to WebAssembly)</h1>
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

fn cmd_shot(designs: &[PathBuf], out: &Path, o: mcview::tools::ShotOptions) -> Result<()> {
    if designs.is_empty() {
        bail!("no design given");
    }
    let atlas = mcview::atlas::Atlas::build();
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
    let (mut ok, mut flyers, mut bad, mut skipped) = (0, 0, 0, 0);
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        if let Err(e) = same_parse(&text) {
            bad += 1;
            println!("FAIL {} parse: {e}", d.display());
            continue;
        }
        let fly = is_flying_machine(&text);
        let line = match if fly { fly_pair(&text, probe.as_mut()) } else { clock_pair(&text, horizon, probe.as_mut()) } {
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
        if fly {
            flyers += 1;
        } else {
            ok += 1;
        }
        if !quiet {
            println!("ok   {} {line}", d.display());
        }
    }
    println!(
        "web check: {ok} clocks (same period as clock sim) and {flyers} flying machines (same frames and end as circuit fly sim) agree{}, {skipped} parse-only, {bad} failures",
        if probe.is_some() { " in the native and the wasm viewer" } else { "" }
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
        (v, Ok(p)) => Err(format!("clock sim {} but the viewer finds W{} P{}", v.word(), p.state_warmup, p.state_period)),
        (v, Err(e)) => Err(format!("clock sim {} (W{} P{}) but the viewer: {e}", v.word(), r.state_warmup, r.state_period)),
    }
}
