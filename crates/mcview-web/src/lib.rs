//! The in-browser viewer (docs/webrender.md). On start it renders every element with a
//! `data-mcview` attribute (or class `mcview`), and the ones added later (MutationObserver):
//!
//! * `data-mcview="thumb"` (default): a still picture, drawn lazily when the element becomes
//!   visible (IntersectionObserver) through ONE shared WebGL2 context and copied into the
//!   element's own canvas; cached by (source, size) so a re-added card paints at once.
//! * `data-mcview="full"` (or `live`): an interactive viewer with its own WebGL2 context: orbit /
//!   zoom / pan, play / pause / step the redsim simulation, click a lever to flip it.
//!
//! Attributes: `data-src` (URL of a design .txt, relative to the page) or `data-design` (the text),
//! optional `data-tick` (still-frame tick), `data-yaw` / `data-pitch` (degrees). The element gets
//! class `mcview-ready` once its canvas shows, `mcview-error` (and a `title`) if it cannot.

#![cfg(target_arch = "wasm32")]

mod gpu;
mod live;
mod overlay;

use mcview::atlas::Atlas;
use mcview::render::Frame;
use mcview::{Options, Session, scene};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, Element, HtmlCanvasElement, HtmlElement, ImageBitmap, IntersectionObserver, IntersectionObserverEntry, MutationObserver, MutationRecord, ResizeObserver, ResizeObserverEntry};

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Run `f` on the app (None when it is not up yet or already borrowed).
pub(crate) fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut g| g.as_mut().map(f)))
}

pub(crate) fn log(s: &str) {
    web_sys::console::log_1(&JsValue::from_str(&format!("mcview: {s}")));
}

/// `?mcview-debug` in the page URL: timings in the console.
pub(crate) fn debug() -> bool {
    thread_local! {
        static DEBUG: bool = web_sys::window().and_then(|w| w.location().search().ok()).is_some_and(|s| s.contains("mcview-debug"));
    }
    DEBUG.with(|d| *d)
}

pub(crate) fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

pub(crate) fn document() -> web_sys::Document {
    window().document().expect("no document")
}

enum ThumbGpu {
    Pending,
    Ready(gpu::Gpu),
    Failed(String),
}

pub(crate) struct View {
    pub el: HtmlElement,
    pub canvas: HtmlCanvasElement,
    pub full: bool,
    pub src: Option<String>,
    pub inline: Option<Rc<str>>,
    /// `data-trace`: a real-game recording a live viewer plays (designs redsim does not model).
    pub trace: Option<String>,
    pub tick: Option<u32>,
    /// `data-autoplay`: a live viewer starts playing once loaded.
    pub autoplay: bool,
    /// `data-stimulus`: a circuit's judge stimulus (the auto-demo with `data-autoplay`).
    pub stimulus: Option<String>,
    /// `data-events`: the judge's actions at cells (a BUD switch's target and footprint probes),
    /// replayed in the auto-demo.
    pub events: Option<String>,
    /// `data-phases`: the checker's phase labels.
    pub phases: Option<String>,
    /// `data-input-mode`: `pulse` | `toggle` (default from the circuit kind).
    pub input_mode: Option<String>,
    pub yaw: Option<f32>,
    pub pitch: Option<f32>,
    /// Canvas size in device pixels.
    pub w: u32,
    pub h: u32,
    pub visible: bool,
    pub done: bool,
    pub live: Option<live::Live>,
}

impl View {
    /// The cache key: source and size.
    fn key(&self) -> String {
        let src = match (&self.src, &self.inline) {
            (Some(s), _) => s.clone(),
            (None, Some(t)) => format!("inline:{:x}", fnv(t)),
            _ => String::new(),
        };
        format!("{src}|{}x{}|{:?}|{:?}|{:?}", self.w, self.h, self.tick, self.yaw, self.pitch)
    }
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    h
}

pub(crate) struct App {
    pub atlas: Rc<Atlas>,
    thumb: ThumbGpu,
    pub views: HashMap<u32, View>,
    next_id: u32,
    queue: VecDeque<u32>,
    texts: HashMap<String, Result<Rc<str>, String>>,
    fetching: HashSet<String>,
    cache: HashMap<String, ImageBitmap>,
    io: IntersectionObserver,
    ro: ResizeObserver,
    frame_scheduled: bool,
    work_scheduled: bool,
    /// The game's textures: still loading (thumbnails wait, up to a few seconds), or settled
    /// (applied, or unavailable: the hand-drawn tiles stay).
    textures_pending: bool,
    /// The game's entity textures were asked for (the first entity drawn without them).
    entities_requested: bool,
}

/// Where the game's assets come from: `data-mcview-textures` on the page's root element (`none` =
/// the hand-drawn tiles and the viewer's models only; else the directory of `atlas.png`,
/// `data.min.json`, `block_definition.min.json` and `model.min.json`), by default misode/mcmeta's
/// block atlas, block definitions and models of the judge's version.
fn texture_urls() -> Option<[String; 4]> {
    let root = document().document_element()?;
    match root.get_attribute("data-mcview-textures") {
        Some(v) if v.trim() == "none" => None,
        Some(v) if !v.trim().is_empty() => {
            let base = v.trim().trim_end_matches('/').to_string();
            Some([format!("{base}/atlas.png"), format!("{base}/data.min.json"), format!("{base}/block_definition.min.json"), format!("{base}/model.min.json")])
        }
        _ => Some([mcview::atlas::vanilla::ATLAS_URL.to_string(), mcview::atlas::vanilla::INDEX_URL.to_string(), mcview::gamemodel::DEFS_URL.to_string(), mcview::gamemodel::MODELS_URL.to_string()]),
    }
}

/// The game's entity and item atlases (`entities.png`, `entities.min.json`, `items.png`,
/// `items.min.json` in a `data-mcview-textures` directory; misode/mcmeta's by default), loaded
/// the first time a page draws an entity.
fn entity_urls() -> Option<[String; 4]> {
    use mcview::atlas::vanilla::*;
    let root = document().document_element()?;
    match root.get_attribute("data-mcview-textures") {
        Some(v) if v.trim() == "none" => None,
        Some(v) if !v.trim().is_empty() => {
            let base = v.trim().trim_end_matches('/').to_string();
            Some([format!("{base}/entities.png"), format!("{base}/entities.min.json"), format!("{base}/items.png"), format!("{base}/items.min.json")])
        }
        _ => Some([ENTITY_ATLAS_URL.to_string(), ENTITY_INDEX_URL.to_string(), ITEM_ATLAS_URL.to_string(), ITEM_INDEX_URL.to_string()]),
    }
}

/// Load the game's entity and item textures into the game texture (boats, minecarts, armor
/// stands and dropped items drawn with the viewer's stand-ins until then).
async fn load_entity_textures() {
    let Some([ep, ei, ip, ii]) = entity_urls() else { return };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    let (ep, ei, ip, ii) = futures_join4(fetch_cached(&ep), fetch_cached(&ei), fetch_cached(&ip), fetch_cached(&ii)).await;
    let got = (|| {
        let (er, ew, eh) = mcview::atlas::vanilla::decode_png(&ep?)?;
        let (ir, iw, ih) = mcview::atlas::vanilla::decode_png(&ip?)?;
        Ok::<_, String>((er, ew, eh, text(ei?)?, ir, iw, ih, text(ii?)?))
    })();
    with_app(|a| match got {
        Ok((er, ew, eh, ei, ir, iw, ih, ii)) => {
            let mut atlas = (*a.atlas).clone();
            match atlas.add_game_entities((&er, ew, eh, &ei), (&ir, iw, ih, &ii)) {
                Ok(n) => {
                    if debug() {
                        log(&format!("the game's entity textures ({n}) and items"));
                    }
                    a.set_atlas(atlas);
                }
                Err(e) => log(&format!("the game's entity textures: {e} (the viewer's stand-ins)")),
            }
        }
        Err(e) => log(&format!("the game's entity textures unavailable: {e} (the viewer's stand-ins)")),
    });
    ensure_frame();
}

/// A URL's bytes through the Cache API (kept across visits; the plain fetch when the Cache API
/// is unavailable, e.g. on http pages).
async fn fetch_cached(url: &str) -> Result<Vec<u8>, String> {
    use wasm_bindgen_futures::JsFuture;
    let w = window();
    let cache: Option<web_sys::Cache> = match w.caches() {
        Ok(cs) => JsFuture::from(cs.open("mcview-textures-1")).await.ok().map(|c| c.unchecked_into()),
        Err(_) => None,
    };
    let mut resp: Option<web_sys::Response> = None;
    if let Some(c) = &cache
        && let Ok(r) = JsFuture::from(c.match_with_str(url)).await
        && !r.is_undefined()
    {
        resp = Some(r.unchecked_into());
    }
    let resp = match resp {
        Some(r) => r,
        None => {
            let r: web_sys::Response = JsFuture::from(w.fetch_with_str(url)).await.map_err(|e| format!("fetch {url}: {e:?}"))?.unchecked_into();
            if !r.ok() {
                return Err(format!("fetch {url}: HTTP {}", r.status()));
            }
            if let Some(c) = &cache
                && let Ok(copy) = r.clone()
            {
                let _ = JsFuture::from(c.put_with_str(url, &copy)).await;
            }
            r
        }
    };
    let buf = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    Ok(js_sys::Uint8Array::new(&buf).to_vec())
}

/// Fetch the game's textures, apply them, re-upload every context's tiles and redraw.
async fn load_textures() {
    let Some([png_url, index_url, defs_url, models_url]) = texture_urls() else {
        with_app(|a| a.textures_pending = false);
        ensure_frame();
        return;
    };
    let text = |b: Vec<u8>| String::from_utf8(b).map_err(|e| e.to_string());
    // all four at once
    let (png, index, defs, models) = futures_join4(fetch_cached(&png_url), fetch_cached(&index_url), fetch_cached(&defs_url), fetch_cached(&models_url)).await;
    let got = (|| Ok::<_, String>((png?, text(index?)?)))();
    let game_text = (|| Ok::<_, String>((text(defs?)?, text(models?)?)))();
    with_app(|a| {
        let now = || window().performance().map(|p| p.now()).unwrap_or(0.0);
        let t0 = now();
        match got {
            Ok((png, index)) => {
                let mut atlas = (*a.atlas).clone();
                match mcview::atlas::vanilla::decode_png(&png) {
                    Ok((rgba, w, h)) => {
                        match atlas.apply_vanilla(&rgba, w, h, &index) {
                            Ok(n) => {
                                if debug() {
                                    log(&format!("the game's textures on {n} tiles ({:.0} ms)", now() - t0));
                                }
                            }
                            Err(e) => log(&format!("the game's textures: {e} (hand-drawn tiles)")),
                        }
                        // the game's own block models (the viewer's when they cannot be read)
                        let t1 = now();
                        match game_text.and_then(|(defs, models)| atlas.set_game(&rgba, w, h, &index, &defs, &models)) {
                            Ok(n) => {
                                if debug() {
                                    log(&format!("the game's block models ({n} textures, {:.0} ms)", now() - t1));
                                }
                            }
                            Err(e) => log(&format!("the game's block models unavailable: {e} (the viewer's models)")),
                        }
                        a.set_atlas(atlas);
                    }
                    Err(e) => log(&format!("the game's textures: {e} (hand-drawn tiles)")),
                }
            }
            Err(e) => log(&format!("the game's textures unavailable: {e} (hand-drawn tiles)")),
        }
        a.textures_pending = false;
    });
    ensure_frame();
}

/// Four futures driven together (no executor crate: a small poll loop over pinned futures).
async fn futures_join4<A: std::future::Future, B: std::future::Future, C: std::future::Future, D: std::future::Future>(a: A, b: B, c: C, d: D) -> (A::Output, B::Output, C::Output, D::Output) {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::Poll;
    let (mut a, mut b, mut c, mut d) = (Box::pin(a), Box::pin(b), Box::pin(c), Box::pin(d));
    let (mut ra, mut rb, mut rc, mut rd) = (None, None, None, None);
    std::future::poll_fn(move |cx| {
        if ra.is_none() && let Poll::Ready(v) = Pin::new(&mut a).poll(cx) {
            ra = Some(v);
        }
        if rb.is_none() && let Poll::Ready(v) = Pin::new(&mut b).poll(cx) {
            rb = Some(v);
        }
        if rc.is_none() && let Poll::Ready(v) = Pin::new(&mut c).poll(cx) {
            rc = Some(v);
        }
        if rd.is_none() && let Poll::Ready(v) = Pin::new(&mut d).poll(cx) {
            rd = Some(v);
        }
        if ra.is_some() && rb.is_some() && rc.is_some() && rd.is_some() {
            Poll::Ready((ra.take().unwrap(), rb.take().unwrap(), rc.take().unwrap(), rd.take().unwrap()))
        } else {
            Poll::Pending
        }
    })
    .await
}

#[wasm_bindgen(start)]
pub fn start() {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&JsValue::from_str(&format!("mcview panic: {info}")));
    }));
    let io_cb = Closure::<dyn FnMut(js_sys::Array, IntersectionObserver)>::new(|entries: js_sys::Array, _| {
        with_app(|a| a.on_intersect(entries));
        ensure_frame();
    });
    let io_init = web_sys::IntersectionObserverInit::new();
    io_init.set_root_margin("300px");
    let io = IntersectionObserver::new_with_options(io_cb.as_ref().unchecked_ref(), &io_init).expect("IntersectionObserver");
    io_cb.forget();
    let ro_cb = Closure::<dyn FnMut(js_sys::Array, ResizeObserver)>::new(|entries: js_sys::Array, _| {
        with_app(|a| a.on_resize(entries));
        ensure_frame();
    });
    let ro = ResizeObserver::new(ro_cb.as_ref().unchecked_ref()).expect("ResizeObserver");
    ro_cb.forget();
    let atlas = Rc::new(Atlas::build());
    let app = App {
        atlas: atlas.clone(),
        thumb: ThumbGpu::Pending,
        views: HashMap::new(),
        next_id: 1,
        queue: VecDeque::new(),
        texts: HashMap::new(),
        fetching: HashSet::new(),
        cache: HashMap::new(),
        io,
        ro,
        frame_scheduled: false,
        work_scheduled: false,
        textures_pending: true,
        entities_requested: false,
    };
    APP.with(|a| *a.borrow_mut() = Some(app));
    // the game's textures (hand-drawn tiles if they do not come within a few seconds)
    wasm_bindgen_futures::spawn_local(load_textures());
    {
        let cb = Closure::once_into_js(move || {
            let late = with_app(|a| std::mem::replace(&mut a.textures_pending, false)).unwrap_or(false);
            if late {
                log("the game's textures are slow: drawing with the hand-drawn tiles meanwhile");
                ensure_frame();
            }
        });
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 4000);
    }
    // the shared thumbnail context, on a canvas that is never shown
    let canvas: HtmlCanvasElement = document().create_element("canvas").unwrap().dyn_into().unwrap();
    canvas.set_width(240);
    canvas.set_height(180);
    // the shared thumbnail context lost (a GPU reset, too many contexts on iOS): thumbnails wait
    // and the context is made again once the canvas is restored (drawn pictures stay)
    {
        let lost = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
            e.prevent_default();
            with_app(|a| a.thumb = ThumbGpu::Pending);
            log("the thumbnails' WebGL context was lost; waiting for it to come back");
        });
        let _ = canvas.add_event_listener_with_callback("webglcontextlost", lost.as_ref().unchecked_ref());
        lost.forget();
        let c2 = canvas.clone();
        let restored = Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
            let c3 = c2.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let Some(atlas) = with_app(|a| a.atlas.clone()) else { return };
                if let Ok(g) = gpu::Gpu::new(c3, &atlas).await {
                    with_app(|a| a.thumb = ThumbGpu::Ready(g));
                    ensure_frame();
                }
            });
        });
        let _ = canvas.add_event_listener_with_callback("webglcontextrestored", restored.as_ref().unchecked_ref());
        restored.forget();
    }
    wasm_bindgen_futures::spawn_local(async move {
        let g = gpu::Gpu::new(canvas, &atlas).await;
        with_app(|a| {
            a.thumb = match g {
                Ok(g) => ThumbGpu::Ready(g),
                Err(e) => {
                    log(&format!("thumbnails disabled: {e}"));
                    ThumbGpu::Failed(e)
                }
            };
        });
        ensure_frame();
    });
    // the page as it is, then whatever is added later
    scan(&document().document_element().unwrap());
    let mo_cb = Closure::<dyn FnMut(js_sys::Array, MutationObserver)>::new(|records: js_sys::Array, _| {
        let mut removed = false;
        for r in records.iter() {
            let r: MutationRecord = r.unchecked_into();
            if r.type_() == "attributes" {
                if let Some(t) = r.target()
                    && let Ok(e) = t.dyn_into::<HtmlElement>()
                {
                    with_app(|a| a.src_changed(&e));
                }
                continue;
            }
            let added = r.added_nodes();
            for i in 0..added.length() {
                if let Some(n) = added.item(i)
                    && let Ok(e) = n.dyn_into::<Element>()
                {
                    scan(&e);
                }
            }
            removed |= r.removed_nodes().length() > 0;
        }
        if removed {
            with_app(|a| a.sweep());
        }
        ensure_frame();
    });
    let mo = MutationObserver::new(mo_cb.as_ref().unchecked_ref()).expect("MutationObserver");
    mo_cb.forget();
    let init = web_sys::MutationObserverInit::new();
    init.set_child_list(true);
    init.set_subtree(true);
    // a viewer's design swapped in place (the site's variant picker): `data-src` set last
    init.set_attributes(true);
    init.set_attribute_filter(&js_sys::Array::of1(&JsValue::from_str("data-src")));
    let _ = mo.observe_with_options(&document().document_element().unwrap(), &init);
    ensure_frame();
}

/// For `mctas web check --wasm`: memory the host fills with a design's text.
#[unsafe(no_mangle)]
pub extern "C" fn mcview_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// For `mctas web check --wasm`: the clock period detector of THIS wasm (the code the browser
/// runs) on a design text written at `ptr` (taken back and freed). Returns state warmup, state
/// period, visible warmup and visible period as four u16 (low to high), or u64::MAX.
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_clock_period(ptr: *mut u8, len: usize, horizon: u32) -> u64 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match Session::new(&text, &Options::default()).and_then(|s| s.period(horizon)) {
        Ok(p) if p.state_warmup < 65536 && p.state_period < 65536 => p.state_warmup as u64 | (p.state_period as u64) << 16 | (p.warmup as u64) << 32 | (p.period as u64) << 48,
        _ => u64::MAX,
    }
}

/// For `mctas web check --wasm`: a flying machine's end tick in THIS wasm's session (the judge's
/// corridor end), or u32::MAX. Takes back and frees the text like [`mcview_clock_period`].
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_fly_end(ptr: *mut u8, len: usize) -> u32 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match Session::new(&text, &Options::default()) {
        Ok(s) if s.simulated() => s.end_tick.unwrap_or(u32::MAX),
        _ => u32::MAX,
    }
}

/// For `mctas web check --wasm`: the digest of this wasm's frames 0..=`ticks`
/// ([`mcview::session::Session::frames_digest`]), or 0.
///
/// # Safety
/// `ptr` must come from `mcview_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn mcview_frames_digest(ptr: *mut u8, len: usize, ticks: u32) -> u64 {
    let bytes = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&bytes);
    match Session::new(&text, &Options::default()) {
        Ok(s) if s.simulated() => s.frames_digest(ticks),
        _ => 0,
    }
}

/// Register every viewer element in (and including) `root`.
fn scan(root: &Element) {
    let mut found: Vec<HtmlElement> = Vec::new();
    if root.matches("[data-mcview], .mcview").unwrap_or(false)
        && let Ok(h) = root.clone().dyn_into::<HtmlElement>()
    {
        found.push(h);
    }
    if let Ok(list) = root.query_selector_all("[data-mcview], .mcview") {
        for i in 0..list.length() {
            if let Some(n) = list.item(i)
                && let Ok(h) = n.dyn_into::<HtmlElement>()
            {
                found.push(h);
            }
        }
    }
    for el in found {
        if el.get_attribute("data-mcview-id").is_none() {
            with_app(|a| a.register(el));
        }
    }
}

/// An entity was drawn without the game's entity textures: load them (once).
fn maybe_load_entities() {
    let want = with_app(|a| {
        let want = !a.entities_requested && a.atlas.game.as_ref().is_some_and(|g| !g.entities && g.wants_entities.get());
        if want {
            a.entities_requested = true;
        }
        want
    })
    .unwrap_or(false);
    if want {
        wasm_bindgen_futures::spawn_local(load_entity_textures());
    }
}

/// Make sure the pending work runs: thumbnails in a task (no vsync needed, so they also draw in
/// background tabs and headless browsers), live viewers in animation frames.
pub(crate) fn ensure_frame() {
    maybe_load_entities();
    let work = with_app(|a| !std::mem::replace(&mut a.work_scheduled, true)).unwrap_or(false);
    if work {
        let cb = Closure::once_into_js(move || {
            let more = with_app(|a| {
                a.work_scheduled = false;
                a.thumbs()
            })
            .unwrap_or(false);
            maybe_load_entities();
            if more {
                ensure_frame();
            }
        });
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), 0);
    }
    let go = with_app(|a| a.views.values().any(|v| v.full) && !std::mem::replace(&mut a.frame_scheduled, true)).unwrap_or(false);
    if go {
        let cb = Closure::once_into_js(move |ts: f64| {
            let more = with_app(|a| {
                a.frame_scheduled = false;
                a.live_frames(ts)
            })
            .unwrap_or(false);
            maybe_load_entities();
            if more {
                ensure_frame();
            }
        });
        let _ = window().request_animation_frame(cb.unchecked_ref());
    }
}

/// Fetch a URL's text.
async fn fetch_text(url: &str) -> Result<String, String> {
    let p = window().fetch_with_str(url);
    let resp: web_sys::Response = wasm_bindgen_futures::JsFuture::from(p).await.map_err(|e| format!("fetch {url}: {e:?}"))?.unchecked_into();
    if !resp.ok() {
        return Err(format!("fetch {url}: HTTP {}", resp.status()));
    }
    let t = wasm_bindgen_futures::JsFuture::from(resp.text().map_err(|e| format!("{e:?}"))?).await.map_err(|e| format!("{e:?}"))?;
    t.as_string().ok_or_else(|| "not text".into())
}

fn attr_f32(el: &Element, k: &str) -> Option<f32> {
    el.get_attribute(k).and_then(|v| v.trim().parse().ok())
}

impl App {
    /// A viewer's `data-src` changed (a design swapped in place): every `data-*` attribute is read
    /// again; a live viewer loads the new design keeping its camera when `data-keep-camera` is
    /// set (and its play state and speed), its buttons, tags and overlays rebuilt; a thumbnail is
    /// drawn again.
    fn src_changed(&mut self, el: &HtmlElement) {
        let Some(id) = el.get_attribute("data-mcview-id").and_then(|s| s.parse::<u32>().ok()) else { return };
        let Some(v) = self.views.get_mut(&id) else { return };
        let src = el.get_attribute("data-src").filter(|s| !s.is_empty());
        if src.is_none() || (src == v.src && v.live.as_ref().is_some_and(|l| l.has_player())) {
            return;
        }
        v.src = src;
        v.inline = el.get_attribute("data-design").filter(|s| !s.is_empty()).map(Rc::from);
        v.trace = el.get_attribute("data-trace").filter(|s| !s.is_empty());
        v.tick = el.get_attribute("data-tick").and_then(|t| t.trim().parse().ok());
        v.autoplay = el.has_attribute("data-autoplay");
        v.stimulus = el.get_attribute("data-stimulus").filter(|s| !s.trim().is_empty());
        v.events = el.get_attribute("data-events").filter(|s| !s.trim().is_empty());
        v.phases = el.get_attribute("data-phases").filter(|s| !s.trim().is_empty());
        v.input_mode = el.get_attribute("data-input-mode").filter(|s| !s.trim().is_empty());
        v.yaw = attr_f32(el, "data-yaw");
        v.pitch = attr_f32(el, "data-pitch");
        let keep = el.has_attribute("data-keep-camera");
        if let Some(l) = v.live.as_mut() {
            l.begin_reload(keep);
        }
        let _ = el.class_list().remove_1("mcview-error");
        v.done = false;
        if v.full || v.visible {
            self.queue.push_back(id);
        }
    }

    /// New tiles (the game's textures arrived): every context uploads them, every picture is
    /// drawn again.
    fn set_atlas(&mut self, atlas: Atlas) {
        self.atlas = Rc::new(atlas);
        if let ThumbGpu::Ready(g) = &mut self.thumb {
            g.r.upload_atlas(&self.atlas);
        }
        self.cache.clear();
        let atlas = self.atlas.clone();
        let mut again = Vec::new();
        for (id, v) in self.views.iter_mut() {
            if let Some(l) = v.live.as_mut() {
                l.atlas_changed(&atlas);
            } else if v.done {
                v.done = false;
                if v.visible {
                    again.push(*id);
                }
            }
        }
        for id in again {
            if !self.queue.contains(&id) {
                self.queue.push_back(id);
            }
        }
    }

    fn register(&mut self, el: HtmlElement) {
        let id = self.next_id;
        self.next_id += 1;
        let _ = el.set_attribute("data-mcview-id", &id.to_string());
        let mode = el.get_attribute("data-mcview").filter(|m| !m.is_empty()).or_else(|| el.get_attribute("data-mode")).unwrap_or_else(|| "thumb".into());
        let full = matches!(mode.as_str(), "full" | "live");
        // keep the element's own children (a fallback card): the canvas covers them once ready
        if window().get_computed_style(&el).ok().flatten().and_then(|s| s.get_property_value("position").ok()).is_some_and(|p| p == "static" || p.is_empty()) {
            let _ = el.style().set_property("position", "relative");
        }
        let canvas: HtmlCanvasElement = document().create_element("canvas").unwrap().dyn_into().unwrap();
        let st = canvas.style();
        let _ = st.set_property("position", "absolute");
        let _ = st.set_property("left", "0");
        let _ = st.set_property("top", "0");
        let _ = st.set_property("width", "100%");
        let _ = st.set_property("height", "100%");
        let _ = st.set_property("display", "block");
        let _ = st.set_property("visibility", "hidden");
        let _ = el.append_child(&canvas);
        let (w, h) = pixel_size(&el);
        canvas.set_width(w.max(1));
        canvas.set_height(h.max(1));
        let view = View {
            src: el.get_attribute("data-src").filter(|s| !s.is_empty()),
            inline: el.get_attribute("data-design").filter(|s| !s.is_empty()).map(Rc::from),
            trace: el.get_attribute("data-trace").filter(|s| !s.is_empty()),
            tick: el.get_attribute("data-tick").and_then(|t| t.trim().parse().ok()),
            autoplay: el.has_attribute("data-autoplay"),
            stimulus: el.get_attribute("data-stimulus").filter(|s| !s.trim().is_empty()),
            events: el.get_attribute("data-events").filter(|s| !s.trim().is_empty()),
            phases: el.get_attribute("data-phases").filter(|s| !s.trim().is_empty()),
            input_mode: el.get_attribute("data-input-mode").filter(|s| !s.trim().is_empty()),
            yaw: attr_f32(&el, "data-yaw"),
            pitch: attr_f32(&el, "data-pitch"),
            el: el.clone(),
            canvas,
            full,
            w,
            h,
            visible: false,
            done: false,
            live: None,
        };
        if view.src.is_none() && view.inline.is_none() {
            self.fail(&el, "no data-src or data-design");
            return;
        }
        self.ro.observe(&el);
        if full {
            self.views.insert(id, view);
            live::start(self, id);
        } else {
            self.io.observe(&el);
            let click_src = view.src.clone().unwrap_or_default();
            let target = el.clone();
            let cb = Closure::<dyn FnMut(web_sys::Event)>::new(move |_e: web_sys::Event| {
                let init = web_sys::CustomEventInit::new();
                init.set_bubbles(true);
                init.set_detail(&JsValue::from_str(&click_src));
                if let Ok(ev) = web_sys::CustomEvent::new_with_event_init_dict("mcview-open", &init) {
                    let _ = target.dispatch_event(&ev);
                }
            });
            let _ = el.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref());
            cb.forget();
            self.views.insert(id, view);
        }
    }

    pub(crate) fn fail(&mut self, el: &HtmlElement, why: &str) {
        let _ = el.class_list().add_1("mcview-error");
        let _ = el.set_attribute("title", why);
        log(why);
    }

    fn id_of(el: &Element) -> Option<u32> {
        el.get_attribute("data-mcview-id").and_then(|s| s.parse().ok())
    }

    fn on_intersect(&mut self, entries: js_sys::Array) {
        for e in entries.iter() {
            let e: IntersectionObserverEntry = e.unchecked_into();
            let Some(id) = App::id_of(&e.target()) else { continue };
            let Some(v) = self.views.get_mut(&id) else { continue };
            v.visible = e.is_intersecting();
            if v.visible && !v.done && !self.queue.contains(&id) {
                self.queue.push_back(id);
            }
        }
    }

    fn on_resize(&mut self, entries: js_sys::Array) {
        for e in entries.iter() {
            let e: ResizeObserverEntry = e.unchecked_into();
            let Some(id) = App::id_of(&e.target()) else { continue };
            let Some(v) = self.views.get_mut(&id) else { continue };
            let (w, h) = pixel_size(&v.el);
            if (w, h) == (v.w, v.h) || w == 0 || h == 0 {
                continue;
            }
            v.w = w;
            v.h = h;
            if v.full {
                if let Some(l) = v.live.as_mut() {
                    l.resized();
                }
            } else {
                v.canvas.set_width(w);
                v.canvas.set_height(h);
                v.done = false;
                if v.visible && !self.queue.contains(&id) {
                    self.queue.push_back(id);
                }
            }
        }
    }

    /// Forget the views whose element left the page.
    fn sweep(&mut self) {
        let gone: Vec<u32> = self.views.iter().filter(|(_, v)| !v.el.is_connected()).map(|(id, _)| *id).collect();
        for id in gone {
            if let Some(v) = self.views.remove(&id) {
                self.io.unobserve(&v.el);
                self.ro.unobserve(&v.el);
                let _ = v.el.remove_attribute("data-mcview-id");
                v.canvas.remove();
            }
            self.queue.retain(|q| *q != id);
        }
    }

    /// The design text of a view, starting its fetch if needed (None while it loads).
    pub(crate) fn text_of(&mut self, id: u32) -> Option<Result<Rc<str>, String>> {
        let v = self.views.get(&id)?;
        if let Some(t) = &v.inline {
            return Some(Ok(t.clone()));
        }
        let src = v.src.clone()?;
        self.url_text(&src)
    }

    /// A view's recording (`data-trace`): None while it loads (or when it has none: check
    /// `View::trace` first).
    pub(crate) fn trace_of(&mut self, id: u32) -> Option<Result<Rc<str>, String>> {
        let url = self.views.get(&id)?.trace.clone()?;
        self.url_text(&url)
    }

    /// The text at a URL (cached), starting its fetch if needed: None while it loads. When it
    /// arrives, every view using it (as design or recording) is queued again.
    fn url_text(&mut self, url: &str) -> Option<Result<Rc<str>, String>> {
        if let Some(r) = self.texts.get(url) {
            return Some(r.clone());
        }
        let src = url.to_string();
        if self.fetching.insert(src.clone()) {
            wasm_bindgen_futures::spawn_local(async move {
                let r = fetch_text(&src).await.map(|t| Rc::<str>::from(t.as_str()));
                with_app(|a| {
                    a.fetching.remove(&src);
                    a.texts.insert(src.clone(), r);
                    let uses = |v: &View| v.src.as_deref() == Some(src.as_str()) || v.trace.as_deref() == Some(src.as_str());
                    let waiting: Vec<u32> = a.views.iter().filter(|(_, v)| uses(v) && (!v.done || v.full) && (v.visible || v.full)).map(|(id, _)| *id).collect();
                    for id in waiting {
                        if !a.queue.contains(&id) {
                            a.queue.push_back(id);
                        }
                    }
                });
                ensure_frame();
            });
        }
        None
    }

    /// Thumbnails (and live viewers waiting for their text) within a time budget. Returns whether
    /// work is left.
    fn thumbs(&mut self) -> bool {
        if self.textures_pending {
            // drawn once, with the game's textures (load_textures or its timeout wakes us)
            return false;
        }
        let perf = window().performance();
        let t0 = perf.as_ref().map(|p| p.now()).unwrap_or(0.0);
        if debug() && !self.queue.is_empty() {
            log(&format!("work at {t0:.0}: {} queued", self.queue.len()));
        }
        let mut waiting = VecDeque::new();
        while let Some(id) = self.queue.pop_front() {
            if perf.as_ref().is_some_and(|p| p.now() - t0 > 12.0) {
                self.queue.push_front(id);
                break;
            }
            let Some(v) = self.views.get(&id) else { continue };
            if v.done {
                continue;
            }
            if v.full {
                live::load(self, id);
                continue;
            }
            let el = v.el.clone();
            match self.thumb {
                ThumbGpu::Pending => {
                    waiting.push_back(id);
                    continue;
                }
                ThumbGpu::Failed(ref e) => {
                    let e = e.clone();
                    self.fail(&el, &format!("WebGL2 unavailable: {e}"));
                    if let Some(v) = self.views.get_mut(&id) {
                        v.done = true;
                    }
                    continue;
                }
                ThumbGpu::Ready(_) => {}
            }
            match self.text_of(id) {
                None => {}
                Some(Err(e)) => {
                    self.fail(&el, &e);
                    if let Some(v) = self.views.get_mut(&id) {
                        v.done = true;
                    }
                }
                Some(Ok(text)) => self.thumb(id, &text),
            }
        }
        self.queue.extend(waiting);
        !self.queue.is_empty() && !matches!(self.thumb, ThumbGpu::Pending)
    }

    /// One animation frame of every live viewer; true while one needs more (playing).
    fn live_frames(&mut self, ts: f64) -> bool {
        let mut more = false;
        let ids: Vec<u32> = self.views.iter().filter(|(_, v)| v.full).map(|(id, _)| *id).collect();
        for id in ids {
            more |= live::frame(self, id, ts);
        }
        more
    }

    /// Draw a thumbnail (from the cache when it has this source at this size).
    fn thumb(&mut self, id: u32, text: &str) {
        let t0 = window().performance().map(|p| p.now()).unwrap_or(0.0);
        let Some(v) = self.views.get(&id) else { return };
        let key = v.key();
        if debug() {
            log(&format!("thumb {key} start"));
        }
        let (w, h) = (v.w.max(1), v.h.max(1));
        let ctx: Option<CanvasRenderingContext2d> = v.canvas.get_context("2d").ok().flatten().and_then(|c| c.dyn_into().ok());
        let Some(ctx) = ctx else { return };
        if let Some(bmp) = self.cache.get(&key) {
            let _ = ctx.draw_image_with_image_bitmap(bmp, 0.0, 0.0);
            self.ready(id);
            return;
        }
        let picture = (|| -> Result<(), String> {
            let mut s = Session::new(text, &Options::default())?;
            match v.tick {
                Some(t) => {
                    s.find_auto_output(20_000);
                    s.run(t)
                }
                None => {
                    s.go_to_still(20_000);
                }
            }
            let look = scene::default_look(&s);
            let aspect = w as f32 / h as f32;
            let mut cam = scene::camera_showing_inputs(&self.atlas, &s, &look, aspect);
            if v.yaw.is_some() || v.pitch.is_some() {
                if let Some(y) = v.yaw {
                    cam.yaw = y.to_radians();
                }
                if let Some(p) = v.pitch {
                    cam.pitch = p.to_radians();
                }
                let (lo, hi) = scene::frame_bounds(&s, &look);
                cam.fit(lo, hi, aspect, 0.85);
            }
            let mesh = scene::build(&self.atlas, &s, 0.0, &look);
            let (solid, translucent) = scene::triangles(&mesh, &cam);
            let ThumbGpu::Ready(g) = &mut self.thumb else { return Err("no WebGL2".into()) };
            g.draw(&Frame { solid: &solid, translucent: &translucent, camera: cam, width: w, height: h })?;
            ctx.clear_rect(0.0, 0.0, w as f64, h as f64);
            ctx.draw_image_with_html_canvas_element(&g.canvas, 0.0, 0.0).map_err(|e| format!("{e:?}"))?;
            Ok(())
        })();
        match picture {
            Ok(()) => {
                if debug() {
                    log(&format!("thumb {key} done in {:.1} ms", window().performance().map(|p| p.now()).unwrap_or(0.0) - t0));
                }
                self.ready(id);
                let canvas = self.views[&id].canvas.clone();
                if let Ok(p) = window().create_image_bitmap_with_html_canvas_element(&canvas) {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(b) = wasm_bindgen_futures::JsFuture::from(p).await {
                            let b: ImageBitmap = b.unchecked_into();
                            with_app(|a| {
                                if a.cache.len() > 600 {
                                    a.cache.clear();
                                }
                                a.cache.insert(key, b);
                            });
                        }
                    });
                }
            }
            Err(e) => {
                let el = self.views[&id].el.clone();
                self.fail(&el, &e);
                if let Some(v) = self.views.get_mut(&id) {
                    v.done = true;
                }
            }
        }
    }

    pub(crate) fn ready(&mut self, id: u32) {
        if let Some(v) = self.views.get_mut(&id) {
            v.done = true;
            let _ = v.canvas.style().set_property("visibility", "visible");
            let _ = v.el.class_list().add_1("mcview-ready");
        }
    }
}

/// An element's size in device pixels (capped at 2x).
pub(crate) fn pixel_size(el: &HtmlElement) -> (u32, u32) {
    let r = el.get_bounding_client_rect();
    let dpr = window().device_pixel_ratio().clamp(1.0, 2.0);
    ((r.width() * dpr).round() as u32, (r.height() * dpr).round() as u32)
}
