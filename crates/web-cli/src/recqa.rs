//! `mctas web recqa`: the Record button's files at phone size, in Chromium and in WebKitGTK (the
//! engine family of iOS Safari). Each page's live viewer, on screen, gets its Record button
//! pressed; the video it offers is caught (`URL.createObjectURL` of a `video/*` blob, the download
//! itself suppressed) and saved; then every file is played in both browsers (a `<video>` that must
//! load, play on and draw a picture) and checked natively: the codec and size the viewer reported,
//! the duration (its frames at 30 per second), every frame decoded and none black, and its colours:
//! the last frame decoded under each range (tv / pc) and matrix (BT.709 / BT.601) next to the
//! colours the encoder read from the viewer's canvas (`?mcview-debug`: `grid=` in
//! `data-mcview-recorded`): the file's own colour tags must name the reading that matches.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::io::Read;
use std::path::{Path, PathBuf};

/// The page's first live viewer, on screen, once it is ready and has its Record button.
pub(crate) const READY: &str = r#"const v = document.querySelector('[data-mcview="full"]');
if (!v) return false;
v.scrollIntoView({block: 'center'});
return /mcview-(ready|error)/.test(v.className) && v.querySelector('[data-mcview-record]') ? v.className : false;"#;

/// Catches the video the Record button offers (the blob it makes a URL of) and stops the download
/// itself (a headless browser has nowhere to put it; WebKit would leave the page).
pub(crate) const HOOK: &str = r#"if (!window.__qaHook) {
  window.__qaHook = 1; window.__qaRec = null; window.__qaRecName = '';
  const make = URL.createObjectURL.bind(URL);
  URL.createObjectURL = function (b) { if (b && b.type && /^video\//.test(b.type)) window.__qaRec = b; return make(b); };
  const click = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () { if (this.download && /^blob:/.test(this.href)) { window.__qaRecName = this.download; return; } return click.call(this); };
}
return typeof VideoEncoder;"#;

pub(crate) const PRESS: &str = r#"const v = document.querySelector('[data-mcview="full"]');
const r = v && v.querySelector('[data-mcview-record]');
if (!r) return 'none';
if (getComputedStyle(r).display === 'none') return 'hidden (VideoEncoder: ' + (typeof VideoEncoder) + ')';
window.__qaT0 = performance.now();
r.click();
return 'pressed';"#;

/// Done (the file and the viewer's note), failed (the button's title), or not yet (false).
pub(crate) const POLL: &str = r#"const v = document.querySelector('[data-mcview="full"]');
const r = v && v.querySelector('[data-mcview-record]');
const note = v && v.getAttribute('data-mcview-recorded');
if (window.__qaRec && note) return {done: 1, size: window.__qaRec.size, type: window.__qaRec.type, name: window.__qaRecName, secs: (performance.now() - window.__qaT0) / 1000, note: note};
if (r && /failed/i.test(r.title || '')) return {done: 0, error: r.title};
return false;"#;

pub(crate) const READ: &str = r#"window.__qaB64 = null;
const fr = new FileReader();
fr.onload = () => { const s = fr.result; window.__qaB64 = s.slice(s.indexOf(',') + 1); };
fr.onerror = () => { window.__qaB64 = 'ERROR'; };
fr.readAsDataURL(window.__qaRec);
return true;"#;

pub(crate) const READ_POLL: &str = "return window.__qaB64 || false;";

/// Play the file at `__SRC__` (`__TYPE__` its MIME type with its codec) in a `<video>`, from a blob
/// as the viewer's own download is (a plain HTTP server without byte ranges cannot be seeked by
/// Chromium): canPlayType, its size and duration once loaded, whether it plays on, a frame drawn
/// 2.5 s in (its mean luma), then a seek to 3/4 of it (the file's index: MP4's moov, WebM's cues)
/// and the frame there. Gives up after 20 s (`timeout`, with the element's states).
pub(crate) const PLAY: &str = r#"const P = window.__qaPlay = {state: 'loading'};
const v = document.createElement('video');
v.muted = true; v.playsInline = true; v.setAttribute('playsinline', ''); v.preload = 'auto';
v.style.cssText = 'position:fixed;left:0;top:0;width:160px;height:160px;z-index:99';
P.canPlay = v.canPlayType('__TYPE__');
const luma = () => {
  try {
    const c = document.createElement('canvas'); c.width = 64; c.height = 64;
    const g = c.getContext('2d'); g.drawImage(v, 0, 0, 64, 64);
    const d = g.getImageData(0, 0, 64, 64).data; let s = 0, k = 0;
    for (let i = 0; i < d.length; i += 4) { s += 0.299 * d[i] + 0.587 * d[i + 1] + 0.114 * d[i + 2]; k++; }
    return s / k;
  } catch (e) { P.drawErr = String(e); return -1; }
};
const t0 = performance.now();
fetch('__SRC__').then(r => r.blob()).then(b => {
  P.fetchMs = Math.round(performance.now() - t0); P.bytes = b.size;
  v.onerror = () => { P.state = 'error'; P.error = v.error ? v.error.code + ' ' + (v.error.message || '') : 'unknown'; };
  v.onloadedmetadata = () => { P.w = v.videoWidth; P.h = v.videoHeight; P.duration = v.duration; };
  document.body.appendChild(v);
  v.src = URL.createObjectURL(new Blob([b], {type: '__TYPE__'.split(';')[0]}));
  v.play().then(() => { P.played = 1; }).catch(e => { P.playErr = String(e); });
  setTimeout(() => {
    P.t = v.currentTime; P.ready = v.readyState; P.luma = luma();
    if (P.state !== 'loading') return;
    v.pause();
    P.seekTo = (v.duration || 0) * 0.75;
    v.onseeked = () => { if (P.state !== 'loading') return; P.seeked = v.currentTime; P.luma2 = luma(); P.state = 'done'; v.remove(); };
    v.currentTime = P.seekTo;
    setTimeout(() => { if (P.state === 'loading') { P.state = 'done'; P.seekTimeout = 1; v.remove(); } }, 5000);
  }, 2500);
}).catch(e => { P.state = 'error'; P.error = 'fetch: ' + e; });
setTimeout(() => { if (P.state === 'loading') { P.state = 'timeout'; P.ready = v.readyState; P.network = v.networkState; P.t = v.currentTime; } }, 20000);
return true;"#;

pub(crate) const PLAY_POLL: &str = "return window.__qaPlay && window.__qaPlay.state !== 'loading' ? window.__qaPlay : false;";

/// What one browser's Record button gave for a page.
#[derive(Clone, Debug)]
pub(crate) struct Capture {
    pub engine: String,
    pub page: String,
    /// The VideoEncoder check, the press, then the file or why not.
    pub encoder: String,
    pub file: Option<PathBuf>,
    pub note: String,
    pub secs: f64,
    pub error: Option<String>,
}

fn stem_of(page: &str) -> String {
    page.trim_end_matches(".html").rsplit('/').next().unwrap_or("page").to_string()
}

/// A finished capture's file, written to `<out>/<engine>/` and copied to `<dir>/qa-rec/` (served,
/// for the playback checks): from the base64 the page read.
pub(crate) fn save(out: &Path, dir: &Path, engine: &str, page: &str, b64: &str, mime: &str) -> Result<PathBuf> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).context("the page's base64")?;
    let ext = if mime.contains("mp4") { "mp4" } else { "webm" };
    let d = out.join(engine);
    std::fs::create_dir_all(&d)?;
    let path = d.join(format!("{}.{ext}", stem_of(page)));
    std::fs::write(&path, &bytes)?;
    let served = dir.join("qa-rec");
    std::fs::create_dir_all(&served)?;
    std::fs::write(served.join(format!("{engine}-{}.{ext}", stem_of(page))), &bytes)?;
    Ok(path)
}

/// WebKit: every page recorded in one browser (each opened in turn), then every file in `files`
/// played (`qa-rec/<name>`, MIME type).
fn webkit_record(dir: &Path, out: &Path, pages: &[String], size: (u32, u32), timeout: u64) -> Result<Vec<Capture>> {
    let mut steps: Vec<(String, u64)> = Vec::new();
    for p in pages {
        let url = if p.contains('?') { format!("{p}&mcview-debug") } else { format!("{p}?mcview-debug") };
        steps.push((format!("@url {url}"), 0));
        steps.push((format!("@until90 {READY}"), 500));
        steps.push((HOOK.to_string(), 500));
        steps.push((PRESS.to_string(), 0));
        steps.push((format!("@until{timeout} {POLL}"), 500));
        steps.push((READ.to_string(), 0));
        steps.push((format!("@until60 {READ_POLL}"), 0));
    }
    let (vals, console) = site::webkit::run_steps(dir, None, "404.html", size, &steps)?;
    let mut caps = Vec::new();
    for (i, p) in pages.iter().enumerate() {
        let v = |k: usize| vals.get(i * 7 + k).cloned().unwrap_or(Value::Null);
        let (ready, enc, press, poll, b64) = (v(1), v(2), v(3), v(4), v(6));
        let mut c = Capture { engine: "webkit".into(), page: p.clone(), encoder: format!("VideoEncoder {}; {}", enc.as_str().unwrap_or("?"), press.as_str().unwrap_or("?")), file: None, note: String::new(), secs: 0.0, error: None };
        if ready.is_null() {
            c.error = Some("the viewer was never ready".into());
        } else if press.as_str() != Some("pressed") {
            c.error = Some(format!("Record: {}", press.as_str().unwrap_or("?")));
        } else if poll.is_null() {
            c.error = Some(format!("no file within {timeout} s"));
        } else if poll["done"].as_i64() != Some(1) {
            c.error = Some(format!("the recording failed: {}", poll["error"].as_str().unwrap_or("?")));
        } else {
            c.note = poll["note"].as_str().unwrap_or("").to_string();
            c.secs = poll["secs"].as_f64().unwrap_or(0.0);
            match b64.as_str() {
                Some(s) if s != "ERROR" => c.file = Some(save(out, dir, "webkit", p, s, poll["type"].as_str().unwrap_or(""))?),
                _ => c.error = Some("could not read the file back".into()),
            }
        }
        caps.push(c);
    }
    let errs: Vec<&String> = console.iter().filter(|l| l.contains("record:") && !l.contains("record: saved")).collect();
    if !errs.is_empty() {
        println!("webkit console: {}", errs.iter().take(4).map(|s| s.chars().take(200).collect::<String>()).collect::<Vec<_>>().join(" || "));
    }
    Ok(caps)
}

/// The MIME type (with its codec) a `<video>` is asked about for a recorded file: the codecs the
/// viewer encodes (`record::Encoder`: H.264 in MP4, else VP9 in WebM).
pub(crate) fn mime_of(ext: &str) -> &'static str {
    if ext == "mp4" { "video/mp4; codecs=\"avc1.42E01E\"" } else { "video/webm; codecs=\"vp09.00.10.08\"" }
}

fn webkit_play(dir: &Path, files: &[(String, String)], size: (u32, u32)) -> Result<Vec<Value>> {
    // each on a fresh page (a page that played several videos once gave no answer for the next)
    let mut steps: Vec<(String, u64)> = Vec::new();
    for (name, mime) in files {
        steps.push(("@url 404.html".to_string(), 0));
        steps.push((PLAY.replace("__SRC__", &format!("qa-rec/{name}")).replace("__TYPE__", &mime.replace('"', "\\\"")), 800));
        steps.push((format!("@until40 {PLAY_POLL}"), 500));
    }
    let (vals, _) = site::webkit::run_steps(dir, None, "404.html", size, &steps)?;
    Ok((0..files.len()).map(|i| vals.get(i * 3 + 2).cloned().unwrap_or(Value::Null)).collect())
}

/// ffprobe's view of a file's video stream and container.
fn probe(path: &Path) -> Result<Value> {
    let o = std::process::Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=codec_name,profile,pix_fmt,width,height,r_frame_rate,avg_frame_rate,color_range,color_space,color_transfer,color_primaries:format=duration,format_name", "-of", "json"])
        .arg(path)
        .output()
        .context("running ffprobe")?;
    if !o.status.success() {
        bail!("ffprobe: {}", String::from_utf8_lossy(&o.stderr).trim());
    }
    Ok(serde_json::from_slice(&o.stdout)?)
}

/// One interpretation of Y'CbCr: (full range, BT.709 matrix).
fn to_rgb(y: f64, u: f64, v: f64, full: bool, bt709: bool) -> [f64; 3] {
    let (yy, cb, cr) = if full { (y / 255.0, (u - 128.0) / 255.0, (v - 128.0) / 255.0) } else { ((y - 16.0) / 219.0, (u - 128.0) / 224.0, (v - 128.0) / 224.0) };
    let (r, g, b) = if bt709 { (yy + 1.5748 * cr, yy - 0.1873 * cb - 0.4681 * cr, yy + 1.8556 * cb) } else { (yy + 1.402 * cr, yy - 0.344136 * cb - 0.714136 * cr, yy + 1.772 * cb) };
    [r * 255.0, g * 255.0, b * 255.0].map(|x| x.clamp(0.0, 255.0))
}

/// An MP4's top-level boxes in order (its index, `moov`, must come before the media, `mdat`, for a
/// phone to play it while it loads: `fastStart`); None for another container.
fn mp4_boxes(path: &Path) -> Option<Vec<String>> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < 8 || &bytes[4..8] != b"ftyp" {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= bytes.len() {
        let size = u32::from_be_bytes(bytes[i..i + 4].try_into().ok()?) as u64;
        let kind = String::from_utf8_lossy(&bytes[i + 4..i + 8]).to_string();
        let size = match size {
            0 => (bytes.len() - i) as u64,
            1 => u64::from_be_bytes(bytes.get(i + 8..i + 16)?.try_into().ok()?),
            s => s,
        };
        out.push(kind);
        if size < 8 {
            break;
        }
        i += size as usize;
    }
    Some(out)
}

/// The native checks of one file.
struct Native {
    codec: String,
    pix_fmt: String,
    size: (u32, u32),
    duration: f64,
    frames: usize,
    tags: String,
    /// Frames darker than the black test: (index, mean luma).
    black: Vec<(usize, f64)>,
    /// The last frame's 8 x 8 grid of mean Y, U, V.
    last_grid: Vec<[f64; 3]>,
    /// Sample frames (RGB, BT.709 limited or the file's own range) for the sheet: (index, w, h, rgba).
    samples: Vec<(usize, u32, u32, Vec<u8>)>,
    full_tag: Option<bool>,
    matrix_tag: Option<bool>,
}

fn native(path: &Path, expect_frames: usize) -> Result<Native> {
    let p = probe(path)?;
    let s = &p["streams"][0];
    let st = |k: &str| s[k].as_str().unwrap_or("").to_string();
    let (w, h) = (s["width"].as_u64().unwrap_or(0) as u32, s["height"].as_u64().unwrap_or(0) as u32);
    let pix_fmt = st("pix_fmt");
    let duration: f64 = p["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0);
    let range = st("color_range");
    let full_tag = match range.as_str() {
        "pc" => Some(true),
        "tv" => Some(false),
        _ => (pix_fmt == "yuvj420p").then_some(true),
    };
    let space = st("color_space");
    let matrix_tag = match space.as_str() {
        "bt709" => Some(true),
        "smpte170m" | "bt470bg" => Some(false),
        _ => None,
    };
    let tags = format!("range {}, matrix {}, transfer {}, primaries {}", if range.is_empty() { "?" } else { &range }, if space.is_empty() { "?" } else { &space }, st("color_transfer").replace("", "").chars().take(20).collect::<String>(), st("color_primaries"));
    if w == 0 || h == 0 || !(pix_fmt == "yuv420p" || pix_fmt == "yuvj420p") {
        bail!("{} {w}x{h} {pix_fmt}: not 4:2:0 video", st("codec_name"));
    }
    // every frame, as decoded (no conversion: the file's own planes)
    let mut child = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "rawvideo", "-pix_fmt", &pix_fmt, "-"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("running ffmpeg")?;
    let mut so = child.stdout.take().context("ffmpeg stdout")?;
    let (cw, ch) = (w.div_ceil(2) as usize, h.div_ceil(2) as usize);
    let fsize = (w * h) as usize + 2 * cw * ch;
    let mut buf = vec![0u8; fsize];
    let full = full_tag.unwrap_or(false);
    let want: Vec<usize> = if expect_frames > 0 { vec![0, expect_frames / 4, expect_frames / 2, 3 * expect_frames / 4, expect_frames - 1] } else { vec![0] };
    let (mut n, mut black, mut samples, mut last_grid) = (0usize, Vec::new(), Vec::new(), Vec::new());
    while so.read_exact(&mut buf).is_ok() {
        let (yp, up, vp) = (&buf[..(w * h) as usize], &buf[(w * h) as usize..(w * h) as usize + cw * ch], &buf[(w * h) as usize + cw * ch..]);
        // the black test (as `web luma`): mean luma under 25 or over half the pixels near black
        let (mut sum, mut dark, mut k) = (0f64, 0usize, 0usize);
        for y in (0..h as usize).step_by(4) {
            for x in (0..w as usize).step_by(4) {
                let yy = yp[y * w as usize + x] as f64;
                let l = if full { yy } else { (yy - 16.0) * 255.0 / 219.0 };
                sum += l;
                dark += (l < 16.0) as usize;
                k += 1;
            }
        }
        let mean = sum / k.max(1) as f64;
        if mean < 25.0 || dark * 2 > k {
            black.push((n, mean));
        }
        // the last frame's grid (kept for every frame: the last one stays)
        let g = 8usize;
        last_grid.clear();
        for gy in 0..g {
            for gx in 0..g {
                let (x0, x1, y0, y1) = (gx * w as usize / g, (gx + 1) * w as usize / g, gy * h as usize / g, (gy + 1) * h as usize / g);
                let (mut s, mut c) = ([0f64; 3], 0f64);
                for y in (y0..y1).step_by(2) {
                    for x in (x0..x1).step_by(2) {
                        s[0] += yp[y * w as usize + x] as f64;
                        s[1] += up[(y / 2) * cw + x / 2] as f64;
                        s[2] += vp[(y / 2) * cw + x / 2] as f64;
                        c += 1.0;
                    }
                }
                last_grid.push(s.map(|v| v / c.max(1.0)));
            }
        }
        if want.contains(&n) || (expect_frames == 0 && n == 0) {
            let mut rgba = Vec::with_capacity((w * h * 4) as usize);
            let bt709 = matrix_tag.unwrap_or(true);
            for y in 0..h as usize {
                for x in 0..w as usize {
                    let c = to_rgb(yp[y * w as usize + x] as f64, up[(y / 2) * cw + x / 2] as f64, vp[(y / 2) * cw + x / 2] as f64, full, bt709);
                    rgba.extend_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
                }
            }
            samples.push((n, w, h, rgba));
        }
        n += 1;
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let status = child.wait()?;
    if !status.success() || !err.trim().is_empty() {
        bail!("ffmpeg could not decode it: {}", err.trim().lines().take(3).collect::<Vec<_>>().join(" | "));
    }
    // the last frame for the sheet when the count was off
    Ok(Native { codec: format!("{} {}", st("codec_name"), st("profile")), pix_fmt, size: (w, h), duration, frames: n, tags, black, last_grid, samples, full_tag, matrix_tag })
}

/// The colours the encoder read (the viewer's `grid=`), next to the last frame decoded under each
/// reading: (full range, BT.709, mean |difference|, max) sorted best first.
fn colour_fit(truth: &[[f64; 3]], grid: &[[f64; 3]]) -> Vec<(bool, bool, f64, f64)> {
    let mut v = Vec::new();
    for full in [false, true] {
        for bt709 in [true, false] {
            let (mut sum, mut max, mut k) = (0f64, 0f64, 0f64);
            for (t, g) in truth.iter().zip(grid) {
                let c = to_rgb(g[0], g[1], g[2], full, bt709);
                for i in 0..3 {
                    let d = (c[i] - t[i]).abs();
                    sum += d;
                    max = max.max(d);
                    k += 1.0;
                }
            }
            v.push((full, bt709, sum / k.max(1.0), max));
        }
    }
    v.sort_by(|a, b| a.2.total_cmp(&b.2));
    v
}

fn parse_grid(note: &str) -> Vec<[f64; 3]> {
    note.split("grid=").nth(1).unwrap_or("").split(';').filter_map(|c| {
        let v: Vec<f64> = c.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        (v.len() == 3).then(|| [v[0], v[1], v[2]])
    }).collect()
}

/// `mctas web recqa`: see the module. `engines`: `chromium` (its headless shell: `browser`),
/// `chrome` (Google's Chrome for Testing headless shell: `chrome`, H.264 in MP4 where Chromium has
/// no H.264), `webkit` (WebKitGTK through the site's rig).
#[allow(clippy::too_many_arguments)]
pub fn cmd_recqa(dir: &Path, pages: &[String], out: &Path, engines: &str, window: &str, scale: f32, timeout: u64, browser: &Path, chrome: Option<&Path>) -> Result<()> {
    let (w, h) = window.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))).context("--window w,h")?;
    if pages.is_empty() {
        bail!("name the pages to record (paths in --dir)");
    }
    std::fs::create_dir_all(out)?;
    let _ = std::fs::remove_dir_all(dir.join("qa-rec"));
    // the site served for Chromium (WebKit's rig serves it itself)
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let base = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());
    let root = dir.to_path_buf();
    std::thread::spawn(move || site::serve::serve_injecting(listener, root, Default::default(), String::new()));
    let mut caps: Vec<Capture> = Vec::new();
    let want = |e: &str| engines.split(',').any(|x| x.trim() == e);
    // the Chromium family: (label, binary)
    let mut blink: Vec<(&str, PathBuf)> = Vec::new();
    if want("chromium") {
        blink.push(("chromium", browser.to_path_buf()));
    }
    if want("chrome") {
        blink.push(("chrome", chrome.context("--engines chrome needs --chrome <chrome-headless-shell>")?.to_path_buf()));
    }
    let use_webkit = want("webkit");
    for (engine, bin) in &blink {
        for p in pages {
            let c = crate::cdp::chromium_record(engine, &base, dir, out, p, window, scale, timeout, bin).unwrap_or_else(|e| Capture { engine: engine.to_string(), page: p.clone(), encoder: String::new(), file: None, note: String::new(), secs: 0.0, error: Some(format!("{e:#}")) });
            println!("{engine} {p}: {}", c.error.clone().unwrap_or_else(|| format!("{} in {:.1} s", c.note.split(" grid=").next().unwrap_or(""), c.secs)));
            caps.push(c);
        }
    }
    if use_webkit {
        for c in webkit_record(dir, out, pages, (w, h), timeout)? {
            println!("webkit {}: {}", c.page, c.error.clone().unwrap_or_else(|| format!("{} in {:.1} s", c.note.split(" grid=").next().unwrap_or(""), c.secs)));
            caps.push(c);
        }
    }
    // every file played in every browser
    let files: Vec<(String, String)> = caps
        .iter()
        .filter_map(|c| {
            let f = c.file.as_ref()?;
            let ext = f.extension()?.to_string_lossy().to_string();
            Some((format!("{}-{}.{ext}", c.engine, stem_of(&c.page)), mime_of(&ext).to_string()))
        })
        .collect();
    let mut plays: Vec<(String, Vec<Value>)> = Vec::new();
    if !files.is_empty() {
        for (engine, bin) in &blink {
            plays.push((engine.to_string(), crate::cdp::chromium_play(&base, &files, window, scale, bin)?));
        }
        if use_webkit {
            plays.push(("webkit".into(), webkit_play(dir, &files, (w, h))?));
        }
    }
    // WebKitGTK plays H.264 through a GStreamer decoder (none on a bare CentOS: then an MP4 is
    // not checked there; Safari has its own)
    let webkit_h264 = ["avdec_h264", "openh264dec", "vah264dec", "nvh264dec"].iter().any(|e| std::process::Command::new("gst-inspect-1.0").arg(e).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success()));
    // the native checks, the report, the sheet
    let font = mctas::mcfont::Font::minecraft("26.3").ok();
    let (tw, th) = (150u32, 190u32);
    let rows = caps.iter().filter(|c| c.file.is_some()).count().max(1) as u32;
    let mut sheet = mctas::mcfont::Image::new(5 * tw + 340, rows * (th + 8), [24, 24, 28, 255]);
    let mut row = 0u32;
    let mut bad = 0usize;
    let mut lines = Vec::new();
    for c in &caps {
        let Some(f) = &c.file else {
            bad += 1;
            let l = format!("BAD {} {}: {} ({})", c.engine, c.page, c.error.clone().unwrap_or_default(), c.encoder);
            println!("{l}");
            lines.push(l);
            continue;
        };
        let name = format!("{}-{}.{}", c.engine, stem_of(&c.page), f.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default());
        let idx = files.iter().position(|x| x.0 == name);
        let words: Vec<&str> = c.note.split_whitespace().collect();
        let frames: usize = words.iter().position(|w| *w == "frames").and_then(|i| words.get(i.wrapping_sub(1))).and_then(|n| n.parse().ok()).unwrap_or(0);
        let size: Option<(u32, u32)> = words.iter().find_map(|w| {
            let (a, b) = w.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        });
        let mut problems: Vec<String> = Vec::new();
        // the sheet row's place and words (drawn once everything is checked)
        let mut label: Option<(u32, String, String)> = None;
        let mut facts: Vec<String> = Vec::new();
        match native(f, frames) {
            Ok(nv) => {
                facts.push(format!("{} {} {}x{}, {} frames, {:.2} s, {}", nv.codec.trim(), nv.pix_fmt, nv.size.0, nv.size.1, nv.frames, nv.duration, nv.tags));
                if nv.frames != frames {
                    problems.push(format!("{} frames decoded, the viewer encoded {frames}", nv.frames));
                }
                let want = frames as f64 / 30.0;
                if (nv.duration - want).abs() > 1.5 / 30.0 {
                    problems.push(format!("lasts {:.3} s, {frames} frames at 30/s make {want:.3} s", nv.duration));
                }
                if size.is_some_and(|s| s != nv.size) {
                    problems.push(format!("{}x{} but the viewer made {:?}", nv.size.0, nv.size.1, size));
                }
                if !nv.black.is_empty() {
                    problems.push(format!("{} black frames (first {} at luma {:.0})", nv.black.len(), nv.black[0].0, nv.black[0].1));
                }
                if let Some(b) = mp4_boxes(f) {
                    let (moov, mdat) = (b.iter().position(|x| x == "moov"), b.iter().position(|x| x == "mdat"));
                    facts.push(format!("boxes {}", b.join(" ")));
                    if !matches!((moov, mdat), (Some(a), Some(z)) if a < z) {
                        problems.push(format!("the MP4's index (moov) is not before its media (mdat): {}", b.join(" ")));
                    }
                }
                // the colours: the reading that matches the encoder's input, and the file's tags
                let truth = parse_grid(&c.note);
                if truth.len() == nv.last_grid.len() && !truth.is_empty() {
                    let fit = colour_fit(&truth, &nv.last_grid);
                    let (bf, b709, bmean, bmax) = fit[0];
                    let name = |full: bool, bt709: bool| format!("{} range {}", if full { "full" } else { "tv" }, if bt709 { "BT.709" } else { "BT.601" });
                    facts.push(format!("colours match {} (mean {:.1}, max {:.0} levels; next {} at {:.1})", name(bf, b709), bmean, bmax, name(fit[1].0, fit[1].1), fit[1].2));
                    if bmean > 4.0 || bmax > 24.0 {
                        problems.push(format!("no reading matches the canvas: best {} at mean {bmean:.1}, max {bmax:.0}", name(bf, b709)));
                    }
                    // the tags must name that reading (unknown: players assume tv range; BT.709 for HD)
                    let tag_full = nv.full_tag.unwrap_or(false);
                    let tag_709 = nv.matrix_tag.unwrap_or(true);
                    let tagged = fit.iter().find(|x| x.0 == tag_full && x.1 == tag_709).copied().unwrap_or(fit[0]);
                    if tagged.2 > bmean + 2.0 {
                        problems.push(format!("the file reads as {} ({}), off by mean {:.1} levels (max {:.0}); its pixels are {}", name(tag_full, tag_709), if nv.full_tag.is_none() || nv.matrix_tag.is_none() { "untagged: a player's assumption" } else { "its tags" }, tagged.2, tagged.3, name(bf, b709)));
                    }
                    if nv.full_tag.is_none() {
                        problems.push("no colour range in the file (ffprobe: unknown; players assume tv)".into());
                    }
                } else {
                    problems.push("no colours from the viewer to compare (grid=)".into());
                }
                // the sheet's row (its words once the playback is checked too)
                let oy = row * (th + 8);
                for (k, (_, sw, sh, rgba)) in nv.samples.iter().enumerate().take(5) {
                    let s = (tw as f64 / *sw as f64).min(th as f64 / *sh as f64);
                    let (dw, dh) = (((*sw as f64 * s) as u32).max(1), ((*sh as f64 * s) as u32).max(1));
                    let small = mctas::mcfont::Image { w: dw, h: dh, px: mcview::tools::lanczos_resize(rgba, *sw, *sh, dw, dh) };
                    sheet.draw(&small, (k as u32 * tw + (tw - dw) / 2) as i64, (oy + (th - dh) / 2) as i64, 1);
                }
                row += 1;
                label = Some((oy, format!("{} {}x{} {} fr {:.2} s", nv.codec.split_whitespace().next().unwrap_or(""), nv.size.0, nv.size.1, nv.frames, nv.duration), nv.tags.chars().take(52).collect::<String>()));
            }
            Err(e) => problems.push(format!("{e:#}")),
        }
        // the playback in every browser (one with no decoder for the codec: not checked there; at
        // least one must play it)
        let mut cannot: Vec<String> = Vec::new();
        let mut played = 0usize;
        let mp4 = name.ends_with(".mp4");
        for (who, list) in &plays {
            let Some(p) = idx.and_then(|i| list.get(i)) else { continue };
            // this WebKitGTK has no GStreamer H.264 decoder: an MP4 stalls its page (no answer)
            if p.is_null() && who == "webkit" && mp4 && !webkit_h264 {
                cannot.push("webkit (its page stalled: no GStreamer H.264 decoder here)".into());
                continue;
            }
            if p.is_null() {
                problems.push(format!("{who}: no playback answer"));
                continue;
            }
            let err = p["error"].as_str().unwrap_or("");
            let no_decoder = p["canPlay"].as_str() == Some("")
                || err.starts_with("4 ")
                || err.contains("DECODER_ERROR_NOT_SUPPORTED")
                || (who == "webkit" && mp4 && !webkit_h264 && p["state"].as_str() != Some("done"));
            if no_decoder {
                cannot.push(format!("{who}{}", if err.is_empty() { String::new() } else { format!(" ({})", err.chars().take(60).collect::<String>()) }));
                continue;
            }
            let t = p["t"].as_f64().unwrap_or(0.0);
            let seek_to = p["seekTo"].as_f64().unwrap_or(0.0);
            let seeked = p["seeked"].as_f64();
            let seek_ok = seeked.is_some_and(|s| (s - seek_to).abs() < 0.5) && p["luma2"].as_f64().unwrap_or(0.0) > 20.0;
            let ok = p["state"].as_str() == Some("done") && p["w"].as_u64().unwrap_or(0) > 0 && t > 0.5 && p["luma"].as_f64().unwrap_or(0.0) > 20.0 && seek_ok;
            let what = format!(
                "{who} plays it: {}x{}, {:.2} s long, at {t:.2} s after 2.5 s, luma {:.0}; seek to {seek_to:.2} s: {}{}{}",
                p["w"],
                p["h"],
                p["duration"].as_f64().unwrap_or(0.0),
                p["luma"].as_f64().unwrap_or(0.0),
                match seeked {
                    Some(s) => format!("at {s:.2} s, luma {:.0}", p["luma2"].as_f64().unwrap_or(0.0)),
                    None => "never got there".to_string(),
                },
                p["error"].as_str().map(|e| format!(", error {e}")).unwrap_or_default(),
                p["playErr"].as_str().map(|e| format!(", play(): {e}")).unwrap_or_default()
            );
            if ok {
                facts.push(what);
                played += 1;
            } else {
                problems.push(format!("{what} (canPlayType: {})", p["canPlay"].as_str().unwrap_or("")));
            }
        }
        if !cannot.is_empty() {
            facts.push(format!("not played in {} (no decoder for it in that browser here)", cannot.join(", ")));
        }
        if !plays.is_empty() && played == 0 {
            problems.push("no browser played it".into());
        }
        if let (Some(font), Some((oy, what, tags))) = (&font, &label) {
            let x = (5 * tw + 6) as i64;
            font.draw(&mut sheet, x, *oy as i64 + 4, &format!("{} {}", c.engine, stem_of(&c.page)), 1, [235, 235, 235], true);
            font.draw(&mut sheet, x, *oy as i64 + 16, what, 1, [200, 200, 210], true);
            font.draw(&mut sheet, x, *oy as i64 + 28, tags, 1, [200, 200, 210], true);
            let col = if problems.is_empty() { [140, 220, 120] } else { [255, 120, 110] };
            let msg = if problems.is_empty() { format!("ok; plays in {played}{}", if cannot.is_empty() { String::new() } else { format!(", no decoder in {}", cannot.len()) }) } else { problems.join("; ") };
            for (i, chunk) in msg.chars().collect::<Vec<_>>().chunks(52).take(8).enumerate() {
                font.draw(&mut sheet, x, *oy as i64 + 42 + 11 * i as i64, &chunk.iter().collect::<String>(), 1, col, true);
            }
        }
        bad += (!problems.is_empty()) as usize;
        let l = format!("{} {} {}: {} in {:.1} s; {}{}", if problems.is_empty() { "ok " } else { "BAD" }, c.engine, c.page, f.display(), c.secs, facts.join("; "), if problems.is_empty() { String::new() } else { format!(" | PROBLEMS: {}", problems.join("; ")) });
        println!("{l}");
        lines.push(l);
    }
    let sheet_path = out.join("sheet.png");
    mcview::render::headless::write_png(&sheet_path, sheet.w, sheet.h, &sheet.px).map_err(anyhow::Error::msg)?;
    let summary = format!("web recqa: {} recordings, {bad} with problems; frames at 0, 1/4, 1/2, 3/4 and the end in {}", caps.len(), sheet_path.display());
    println!("{summary}");
    lines.push(summary);
    std::fs::write(out.join("report.txt"), lines.join("\n") + "\n")?;
    if bad > 0 {
        bail!("{bad} recordings with problems");
    }
    Ok(())
}
