//! `mctas web ...`: the in-browser viewer's commands (docs/webrender.md). Argument parsing and
//! dispatch only (compiled unoptimized); the work is in `crates/mcview`.

use anyhow::{Context, Result, bail};
use clap::Subcommand;
use std::path::{Path, PathBuf};

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
}

pub fn run(_version: &str, cmd: WebCmd) -> Result<()> {
    match cmd {
        WebCmd::Shot { designs, out, width, height, tick, flips, yaw, pitch } => cmd_shot(&designs, &out, mcview::tools::ShotOptions { width, height, tick, flips, yaw, pitch }),
        WebCmd::Check { designs, horizon, quiet } => cmd_check(&designs, horizon, quiet),
        WebCmd::Bundle { out } => cmd_bundle(&out),
        WebCmd::Demo { designs, out, viewer, live } => cmd_demo(&designs, &out, &viewer, live),
        WebCmd::Serve { dir, port } => cmd_serve(&dir, port),
        WebCmd::Compare { designs, img_dir, out } => cmd_compare(&designs, &img_dir, &out),
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

fn cmd_demo(designs: &[PathBuf], out: &Path, viewer: &Path, live: usize) -> Result<()> {
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
            lives.push_str(&format!("<section><h2>{label}</h2><div class=\"live\" data-mcview=\"full\" data-src=\"{src}\"><p class=\"fallback\">loading {label}</p></div></section>\n"));
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

fn cmd_check(designs: &[PathBuf], horizon: u32, quiet: bool) -> Result<()> {
    let (mut ok, mut bad, mut skipped) = (0, 0, 0);
    for d in designs {
        let text = std::fs::read_to_string(d).with_context(|| format!("reading {}", d.display()))?;
        if let Err(e) = same_parse(&text) {
            bad += 1;
            println!("FAIL {} parse: {e}", d.display());
            continue;
        }
        let line = match clock_pair(&text, horizon) {
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
        ok += 1;
        if !quiet {
            println!("ok   {} {line}", d.display());
        }
    }
    println!("web check: {ok} clocks agree, {skipped} parse-only, {bad} failures");
    if bad > 0 {
        bail!("{bad} failures");
    }
    Ok(())
}

/// A clock's numbers from both detectors: Some(summary) when they agree, None when the design is
/// not a redsim clock, Err when they differ.
fn clock_pair(text: &str, horizon: u32) -> Result<Option<String>, String> {
    let design = mctas::design::Design::parse(text).map_err(|e| format!("{e:#}"))?;
    if design.ports.contains_key("door") {
        return Ok(None);
    }
    let Ok(c) = clock::Clock::from_design(design) else { return Ok(None) };
    let r = clock::sim::simulate(&c, &clock::sim::SimOptions { horizon, margin: 4 });
    let s = mcview::Session::new(text, &mcview::Options::default())?;
    let mine = s.period(horizon);
    match (r.verdict, mine) {
        (clock::sim::Verdict::Clock | clock::sim::Verdict::Stops | clock::sim::Verdict::Composite, Ok(p)) => {
            let theirs = (r.state_warmup, r.state_period, r.warmup, r.period);
            let ours = (p.state_warmup, p.state_period, p.warmup, p.period);
            if theirs == ours { Ok(Some(format!("W{} P{} (state W{} P{})", p.warmup, p.period, p.state_warmup, p.state_period))) } else { Err(format!("clock sim {theirs:?} vs viewer {ours:?}")) }
        }
        (clock::sim::Verdict::Unmodeled | clock::sim::Verdict::Invalid | clock::sim::Verdict::NoRepeat, Err(_)) => Ok(None),
        (v, Ok(p)) => Err(format!("clock sim {} but the viewer finds W{} P{}", v.word(), p.state_warmup, p.state_period)),
        (v, Err(e)) => Err(format!("clock sim {} (W{} P{}) but the viewer: {e}", v.word(), r.state_warmup, r.state_period)),
    }
}
