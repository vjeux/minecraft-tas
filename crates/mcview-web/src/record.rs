//! The viewer's video recorder (vjeux 2026-10-20: "add a record video button using mediabunny"):
//! the viewer's own rendering of redsim's run (the blocks and entities, the OUT and port boxes,
//! the tags and labels, the badges as shown), stepped and drawn frame by frame at a fixed rate
//! (exact partial ticks: never a screen capture of real time), encoded by the vendored mediabunny
//! (H.264 in MP4 when the browser can encode it, else VP9 in WebM) and offered as a download.
//! The frames are composed on a 2D canvas: the 3D view, then the HTML overlays painted from the
//! page's own layout ([`paint`]).

use crate::{document, window};
use js_sys::{Array, Function, Object, Promise, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{CanvasRenderingContext2d, CssStyleDeclaration, Element, HtmlCanvasElement, HtmlImageElement};

#[wasm_bindgen(inline_js = "export function mcview_mediabunny() { return import(new URL('./mediabunny.min.js', import.meta.url).href); }")]
extern "C" {
    /// The vendored mediabunny module, imported when a recording starts. `mctas web bundle`
    /// makes the bundle flat (this snippet sits next to mcview.js, and so does
    /// `mediabunny.min.js`). The one line of JavaScript binds the `import()` operator.
    fn mcview_mediabunny() -> Promise;
}

/// Frames per second of a recording.
pub const FPS: f64 = 30.0;
/// The longest recording, in seconds.
pub const MAX_SECONDS: f64 = 20.0;
/// The largest recording, in pixels.
pub const MAX_SIZE: (f64, f64) = (1920.0, 1080.0);

/// Whether this browser can encode video at all (WebCodecs' `VideoEncoder`).
pub fn supported() -> bool {
    Reflect::has(&window(), &JsValue::from_str("VideoEncoder")).unwrap_or(false)
}

fn err(e: JsValue) -> String {
    e.as_string().or_else(|| e.dyn_ref::<js_sys::Error>().map(|e| String::from(e.message()))).unwrap_or_else(|| format!("{e:?}"))
}

fn get(o: &JsValue, k: &str) -> Result<JsValue, String> {
    let v = Reflect::get(o, &JsValue::from_str(k)).map_err(err)?;
    if v.is_undefined() {
        return Err(format!("mediabunny has no {k}"));
    }
    Ok(v)
}

fn obj(pairs: &[(&str, JsValue)]) -> JsValue {
    let o = Object::new();
    for (k, v) in pairs {
        let _ = Reflect::set(&o, &JsValue::from_str(k), v);
    }
    o.into()
}

fn construct(ctor: &JsValue, args: &[JsValue]) -> Result<JsValue, String> {
    let f: &Function = ctor.dyn_ref().ok_or("not a constructor")?;
    Reflect::construct(f, &args.iter().collect::<Array>()).map_err(err)
}

/// Call method `m` of `o`; a promise it returns is awaited.
async fn call(o: &JsValue, m: &str, args: &[JsValue]) -> Result<JsValue, String> {
    let f: Function = get(o, m)?.dyn_into().map_err(|_| format!("{m} is not a function"))?;
    let r = f.apply(o, &args.iter().collect::<Array>()).map_err(err)?;
    match r.dyn_into::<Promise>() {
        Ok(p) => JsFuture::from(p).await.map_err(err),
        Err(v) => Ok(v),
    }
}

/// mediabunny's `Output` on a canvas: each [`Encoder::add`] encodes what the canvas shows.
pub struct Encoder {
    output: JsValue,
    source: JsValue,
    /// The file's extension and MIME type.
    pub ext: &'static str,
    pub mime: &'static str,
}

impl Encoder {
    /// An encoder of `canvas` (`w` x `h`, both even) at [`FPS`]: H.264 in MP4 when the browser
    /// can encode it, else VP9 in WebM (both 4:2:0, the encoder's default range).
    pub async fn new(canvas: &HtmlCanvasElement, w: u32, h: u32) -> Result<Encoder, String> {
        let mb = JsFuture::from(mcview_mediabunny()).await.map_err(|e| format!("loading mediabunny.min.js: {}", err(e)))?;
        let quality = get(&mb, "QUALITY_HIGH")?;
        let can: Function = get(&mb, "canEncodeVideo")?.dyn_into().map_err(|_| "canEncodeVideo")?;
        let probe = obj(&[("width", w.into()), ("height", h.into()), ("frameRate", FPS.into()), ("quality", quality.clone())]);
        let mut pick = None;
        for (codec, format, ext, mime) in [("avc", "Mp4OutputFormat", "mp4", "video/mp4"), ("vp9", "WebMOutputFormat", "webm", "video/webm")] {
            let p: Promise = can.call2(&JsValue::NULL, &JsValue::from_str(codec), &probe).map_err(err)?.dyn_into().map_err(|_| "canEncodeVideo: no promise")?;
            if JsFuture::from(p).await.map_err(err)?.as_bool() == Some(true) {
                pick = Some((codec, format, ext, mime));
                break;
            }
        }
        let (codec, format, ext, mime) = pick.ok_or("this browser cannot encode H.264 or VP9 video")?;
        // MP4: the index first (plays while it downloads, everywhere)
        let format_options = if ext == "mp4" { obj(&[("fastStart", "in-memory".into())]) } else { obj(&[]) };
        let fmt = construct(&get(&mb, format)?, &[format_options])?;
        let target = construct(&get(&mb, "BufferTarget")?, &[])?;
        let output = construct(&get(&mb, "Output")?, &[obj(&[("format", fmt), ("target", target)])])?;
        let source = construct(&get(&mb, "CanvasSource")?, &[canvas.into(), obj(&[("codec", codec.into()), ("quality", quality)])])?;
        call(&output, "addVideoTrack", &[source.clone(), obj(&[("frameRate", FPS.into())])]).await?;
        call(&output, "start", &[]).await?;
        Ok(Encoder { output, source, ext, mime })
    }

    /// What the canvas shows as frame `k` (at `k / FPS` s, `1 / FPS` s long); resolves when the
    /// encoder takes more.
    pub async fn add(&self, k: u32) -> Result<(), String> {
        call(&self.source, "add", &[(k as f64 / FPS).into(), (1.0 / FPS).into()]).await.map(|_| ())
    }

    /// The finished file.
    pub async fn finish(self) -> Result<js_sys::ArrayBuffer, String> {
        call(&self.output, "finalize", &[]).await?;
        get(&get(&self.output, "target")?, "buffer")?.dyn_into().map_err(|_| "no buffer".to_string())
    }

    pub async fn cancel(self) {
        let _ = call(&self.output, "cancel", &[]).await;
    }
}

/// Offer `bytes` as a download named `name`.
pub fn download(bytes: &js_sys::ArrayBuffer, mime: &str, name: &str) -> Result<(), String> {
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type(mime);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&Array::of1(bytes), &bag).map_err(err)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(err)?;
    let a: web_sys::HtmlAnchorElement = document().create_element("a").map_err(err)?.dyn_into().map_err(|_| "no anchor")?;
    a.set_href(&url);
    a.set_download(name);
    let _ = a.style().set_property("display", "none");
    if let Some(body) = document().body() {
        let _ = body.append_child(&a);
    }
    a.click();
    a.remove();
    // the URL lives a minute (the download has its bytes by then)
    let cb = Closure::once_into_js(move || {
        let _ = web_sys::Url::revoke_object_url(&url);
    });
    let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 60_000);
    Ok(())
}

/// A rounded rectangle's path.
fn rounded(ctx: &CanvasRenderingContext2d, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    ctx.begin_path();
    ctx.move_to(x + r, y);
    ctx.line_to(x + w - r, y);
    let _ = ctx.arc_to(x + w, y, x + w, y + r, r);
    ctx.line_to(x + w, y + h - r);
    let _ = ctx.arc_to(x + w, y + h, x + w - r, y + h, r);
    ctx.line_to(x + r, y + h);
    let _ = ctx.arc_to(x, y + h, x, y + h - r, r);
    ctx.line_to(x, y + r);
    let _ = ctx.arc_to(x, y, x + r, y, r);
    ctx.close_path();
}

fn px(s: &str) -> f64 {
    s.trim().trim_end_matches("px").parse().unwrap_or(0.0)
}

/// Paint an element and what it shows (its background pill, its text, its images and canvases)
/// onto `ctx`: page positions relative to `origin` (CSS px) times `scale`. The viewer's HTML
/// overlays (tags, labels, badges, the output strip) drawn into a recording's frames from the
/// page's own layout.
pub fn paint(ctx: &CanvasRenderingContext2d, el: &Element, origin: (f64, f64), scale: f64) {
    let Ok(Some(cs)) = window().get_computed_style(el) else { return };
    let prop = |k: &str| cs.get_property_value(k).unwrap_or_default();
    if prop("display") == "none" || prop("visibility") == "hidden" {
        return;
    }
    let r = el.get_bounding_client_rect();
    let (x, y, w, h) = ((r.left() - origin.0) * scale, (r.top() - origin.1) * scale, r.width() * scale, r.height() * scale);
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    ctx.save();
    let opacity: f64 = prop("opacity").trim().parse().unwrap_or(1.0);
    ctx.set_global_alpha(ctx.global_alpha() * opacity);
    if let Some(c) = el.dyn_ref::<HtmlCanvasElement>() {
        if c.width() > 0 && c.height() > 0 {
            ctx.set_image_smoothing_enabled(prop("image-rendering") != "pixelated");
            let _ = ctx.draw_image_with_html_canvas_element_and_dw_and_dh(c, x, y, w, h);
        }
    } else if let Some(img) = el.dyn_ref::<HtmlImageElement>() {
        if img.complete() && img.natural_width() > 0 {
            // item icons are pixel art
            ctx.set_image_smoothing_enabled(false);
            let _ = ctx.draw_image_with_html_image_element_and_dw_and_dh(img, x, y, w, h);
        }
    } else {
        let bg = prop("background-color");
        if !bg.is_empty() && bg != "transparent" && bg != "rgba(0, 0, 0, 0)" {
            rounded(ctx, x, y, w, h, px(&prop("border-top-left-radius")) * scale);
            ctx.set_fill_style_str(&bg);
            ctx.fill();
        }
        let kids = el.child_nodes();
        for i in 0..kids.length() {
            let Some(n) = kids.item(i) else { continue };
            if let Some(e) = n.dyn_ref::<Element>() {
                paint(ctx, e, origin, scale);
            } else if n.node_type() == web_sys::Node::TEXT_NODE {
                paint_text(ctx, &n, &cs, origin, scale);
            }
        }
    }
    ctx.restore();
}

/// A text node where the page laid it out: each line (its words grouped by the line they sit
/// on) at its own place, in the element's font and colour.
fn paint_text(ctx: &CanvasRenderingContext2d, node: &web_sys::Node, cs: &CssStyleDeclaration, origin: (f64, f64), scale: f64) {
    let text = node.text_content().unwrap_or_default();
    if text.trim().is_empty() {
        return;
    }
    let Ok(range) = document().create_range() else { return };
    let prop = |k: &str| cs.get_property_value(k).unwrap_or_default();
    ctx.set_font(&format!("{} {} {}px {}", prop("font-style"), prop("font-weight"), px(&prop("font-size")) * scale, prop("font-family")));
    ctx.set_fill_style_str(&prop("color"));
    ctx.set_text_baseline("middle");
    ctx.set_text_align("left");
    // the words (UTF-16 offsets, as ranges count them) and where each sits
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut words: Vec<(u32, u32)> = Vec::new();
    let mut start: Option<usize> = None;
    for (i, u) in units.iter().enumerate() {
        let space = *u == b' ' as u16 || *u == b'\n' as u16 || *u == b'\t' as u16;
        match (space, start) {
            (false, None) => start = Some(i),
            (true, Some(s)) => {
                words.push((s as u32, i as u32));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        words.push((s as u32, units.len() as u32));
    }
    // lines: (top, left, middle y, first word, last word)
    let mut lines: Vec<(f64, f64, f64, u32, u32)> = Vec::new();
    for (a, b) in words {
        if range.set_start(node, a).is_err() || range.set_end(node, b).is_err() {
            continue;
        }
        let r = range.get_bounding_client_rect();
        if r.width() <= 0.0 && r.height() <= 0.0 {
            continue;
        }
        match lines.last_mut() {
            Some(l) if (r.top() - l.0).abs() < r.height() / 2.0 => l.4 = b,
            _ => lines.push((r.top(), r.left(), r.top() + r.height() / 2.0, a, b)),
        }
    }
    for (_, left, mid, a, b) in lines {
        let s = String::from_utf16_lossy(&units[a as usize..b as usize]);
        let _ = ctx.fill_text(&s, (left - origin.0) * scale, (mid - origin.1) * scale);
    }
}
