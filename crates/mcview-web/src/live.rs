//! The live viewer: its own WebGL2 context on the element's canvas, a control bar (reset, play /
//! pause, step one game tick, speed, flip the lever, the tick counter in gt and seconds), and the
//! pointer: drag to orbit, right-drag / shift-drag to pan, wheel to zoom, click a lever or button
//! to use it, double-click to reset the view. Keys: space play/pause, right arrow or `.` step,
//! `r` reset, `f` flip.

use crate::gpu::Gpu;
use crate::{App, document, ensure_frame, log, with_app};
use mcview::player::Player;
use mcview::render::Frame;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlButtonElement, HtmlElement, HtmlSelectElement, KeyboardEvent, PointerEvent, WheelEvent};

struct Drag {
    x: f64,
    y: f64,
    moved: bool,
    pan: bool,
}

pub struct Live {
    gpu: Option<Gpu>,
    player: Option<Player>,
    label: HtmlElement,
    status: HtmlElement,
    play: HtmlButtonElement,
    flip: HtmlButtonElement,
    drag: Option<Drag>,
    last_ts: Option<f64>,
    redraw: bool,
}

const SPEEDS: [(&str, f32); 8] = [("1/4x", 5.0), ("1/2x", 10.0), ("1x (20 gt/s)", 20.0), ("2x", 40.0), ("5x", 100.0), ("20x", 400.0), ("100x", 2000.0), ("1 gt/s", 1.0)];

fn el(tag: &str) -> HtmlElement {
    document().create_element(tag).unwrap().dyn_into().unwrap()
}

fn button(text: &str, title: &str) -> HtmlButtonElement {
    let b: HtmlButtonElement = document().create_element("button").unwrap().dyn_into().unwrap();
    b.set_text_content(Some(text));
    b.set_title(title);
    let s = b.style();
    for (k, v) in [("background", "rgba(255,255,255,0.12)"), ("color", "#fff"), ("border", "1px solid rgba(255,255,255,0.25)"), ("border-radius", "4px"), ("padding", "2px 8px"), ("font", "inherit"), ("cursor", "pointer")] {
        let _ = s.set_property(k, v);
    }
    b
}

/// Run `f` on a live view and the atlas.
fn with_live<R>(id: u32, f: impl FnOnce(&mut Live, &mcview::atlas::Atlas, &mut crate::View) -> R) -> Option<R> {
    with_app(|a| {
        let atlas = a.atlas.clone();
        let v = a.views.get_mut(&id)?;
        let mut l = v.live.take()?;
        let r = f(&mut l, &atlas, v);
        v.live = Some(l);
        Some(r)
    })
    .flatten()
}

fn on<E: JsCast + wasm_bindgen::convert::FromWasmAbi + 'static>(target: &web_sys::EventTarget, kind: &str, f: impl FnMut(E) + 'static) {
    let cb = Closure::<dyn FnMut(E)>::new(f);
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(false);
    let _ = target.add_event_listener_with_callback_and_add_event_listener_options(kind, cb.as_ref().unchecked_ref(), &opts);
    cb.forget();
}

/// Build the controls, start the WebGL2 context and the design's fetch.
pub fn start(app: &mut App, id: u32) {
    let v = app.views.get_mut(&id).unwrap();
    let root = v.el.clone();
    let _ = root.set_attribute("tabindex", "0");
    let _ = root.style().set_property("outline", "none");
    let bar = el("div");
    let bs = bar.style();
    for (k, val) in [
        ("position", "absolute"),
        ("left", "0"),
        ("right", "0"),
        ("bottom", "0"),
        ("display", "flex"),
        ("gap", "6px"),
        ("align-items", "center"),
        ("flex-wrap", "wrap"),
        ("padding", "6px 8px"),
        ("background", "linear-gradient(transparent, rgba(0,0,0,0.65))"),
        ("color", "#fff"),
        ("font", "12px system-ui, sans-serif"),
        ("z-index", "2"),
    ] {
        let _ = bs.set_property(k, val);
    }
    let reset = button("⟲", "back to the placed state (r)");
    let play = button("▶", "play / pause (space)");
    let step = button("⏭ 1 gt", "one game tick (right arrow)");
    let flip = button("flip lever", "flip the lever (f), or click a lever");
    let speed: HtmlSelectElement = document().create_element("select").unwrap().dyn_into().unwrap();
    for (i, (name, _)) in SPEEDS.iter().enumerate() {
        let o: web_sys::HtmlOptionElement = document().create_element("option").unwrap().dyn_into().unwrap();
        o.set_text(name);
        o.set_value(&i.to_string());
        let _ = speed.append_child(&o);
    }
    speed.set_value("2");
    let ss = speed.style();
    for (k, val) in [("background", "rgba(0,0,0,0.4)"), ("color", "#fff"), ("border", "1px solid rgba(255,255,255,0.25)"), ("border-radius", "4px"), ("font", "inherit")] {
        let _ = ss.set_property(k, val);
    }
    let label = el("span");
    let _ = label.style().set_property("font-variant-numeric", "tabular-nums");
    let status = el("span");
    let _ = status.style().set_property("opacity", "0.8");
    for c in [reset.unchecked_ref::<HtmlElement>(), play.unchecked_ref(), step.unchecked_ref(), speed.unchecked_ref(), flip.unchecked_ref(), &label, &status] {
        let _ = bar.append_child(c);
    }
    let _ = root.append_child(&bar);
    let _ = flip.style().set_property("display", "none");
    v.live = Some(Live { gpu: None, player: None, label, status, play: play.clone(), flip: flip.clone(), drag: None, last_ts: None, redraw: true });
    let canvas = v.canvas.clone();
    let _ = canvas.style().set_property("touch-action", "none");
    // controls
    on(&reset, "click", move |_: web_sys::Event| {
        with_live(id, |l, _, _| {
            if let Some(p) = l.player.as_mut() {
                p.reset();
                l.redraw = true;
            }
        });
        ensure_frame();
    });
    on(&play, "click", move |_: web_sys::Event| {
        with_live(id, |l, _, _| l.toggle_play());
        ensure_frame();
    });
    on(&step, "click", move |_: web_sys::Event| {
        with_live(id, |l, _, _| l.step());
        ensure_frame();
    });
    on(&flip, "click", move |_: web_sys::Event| {
        with_live(id, |l, _, _| {
            if let Some(p) = l.player.as_mut() {
                p.flip();
                l.redraw = true;
            }
        });
        ensure_frame();
    });
    let sp = speed.clone();
    on(&speed, "change", move |_: web_sys::Event| {
        let i: usize = sp.value().parse().unwrap_or(2);
        with_live(id, |l, _, _| {
            if let Some(p) = l.player.as_mut() {
                p.tps = SPEEDS[i.min(SPEEDS.len() - 1)].1;
            }
        });
    });
    // pointer
    on(&canvas, "pointerdown", move |e: PointerEvent| {
        let pan = e.button() != 0 || e.shift_key();
        with_live(id, |l, _, v| {
            l.drag = Some(Drag { x: e.client_x() as f64, y: e.client_y() as f64, moved: false, pan });
            let _ = v.canvas.set_pointer_capture(e.pointer_id());
            let _ = v.el.focus();
        });
        e.prevent_default();
    });
    on(&canvas, "pointermove", move |e: PointerEvent| {
        with_live(id, |l, _, v| l.pointer_move(&e, v));
        ensure_frame();
    });
    on(&canvas, "pointerup", move |e: PointerEvent| {
        with_live(id, |l, atlas, v| l.pointer_up(&e, atlas, v));
        ensure_frame();
    });
    on(&canvas, "wheel", move |e: WheelEvent| {
        e.prevent_default();
        with_live(id, |l, _, _| {
            if let Some(p) = l.player.as_mut() {
                p.camera.dist = (p.camera.dist * (e.delta_y() as f32 * 0.0015).exp()).clamp(1.0, 2000.0);
                l.redraw = true;
            }
        });
        ensure_frame();
    });
    on(&canvas, "dblclick", move |_: web_sys::Event| {
        with_live(id, |l, _, _| {
            if let Some(p) = l.player.as_mut() {
                p.camera = p.home;
                l.redraw = true;
            }
        });
        ensure_frame();
    });
    on(&canvas, "contextmenu", move |e: web_sys::Event| e.prevent_default());
    on(&root, "keydown", move |e: KeyboardEvent| {
        let handled = with_live(id, |l, _, _| {
            match e.key().as_str() {
                " " => l.toggle_play(),
                "ArrowRight" | "." => l.step(),
                "r" => {
                    if let Some(p) = l.player.as_mut() {
                        p.reset();
                        l.redraw = true;
                    }
                }
                "f" => {
                    if let Some(p) = l.player.as_mut() {
                        p.flip();
                        l.redraw = true;
                    }
                }
                _ => return false,
            }
            true
        })
        .unwrap_or(false);
        if handled {
            e.prevent_default();
            ensure_frame();
        }
    });
    // the context, then the design
    let atlas = app.atlas.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match Gpu::new(canvas, &atlas).await {
            Ok(g) => {
                with_live(id, |l, _, _| {
                    l.gpu = Some(g);
                    l.redraw = true;
                });
            }
            Err(e) => {
                with_app(|a| {
                    if let Some(v) = a.views.get(&id) {
                        let el = v.el.clone();
                        a.fail(&el, &format!("WebGL2 unavailable: {e}"));
                    }
                });
            }
        }
        with_app(|a| load(a, id));
        ensure_frame();
    });
}

/// Create the player once the design text is here.
pub fn load(app: &mut App, id: u32) {
    let Some(v) = app.views.get(&id) else { return };
    if v.live.as_ref().is_none_or(|l| l.player.is_some()) {
        return;
    }
    let el = v.el.clone();
    let text = match app.text_of(id) {
        None => return,
        Some(Err(e)) => {
            app.fail(&el, &e);
            if let Some(v) = app.views.get_mut(&id) {
                v.done = true;
            }
            return;
        }
        Some(Ok(t)) => t,
    };
    let v = app.views.get_mut(&id).unwrap();
    let aspect = v.w.max(1) as f32 / v.h.max(1) as f32;
    match Player::new(&text, aspect) {
        Ok(mut p) => {
            if v.yaw.is_some() || v.pitch.is_some() {
                if let Some(y) = v.yaw {
                    p.camera.yaw = y.to_radians();
                }
                if let Some(pt) = v.pitch {
                    p.camera.pitch = pt.to_radians();
                }
                let (lo, hi) = mcview::scene::frame_bounds(&p.session, &p.look);
                p.camera.fit(lo, hi, aspect, 0.85);
                p.home = p.camera;
            }
            if let Some(t) = v.tick {
                p.session.run(t);
            }
            let l = v.live.as_mut().unwrap();
            if p.session.mode.is_door() || !p.session.inputs.is_empty() {
                let _ = l.flip.style().set_property("display", "inline-block");
            }
            match &p.session.unsimulated {
                Some(why) => l.status.set_text_content(Some(&format!("shown as built, not simulated: {why}"))),
                None => l.status.set_text_content(Some(match p.session.mode {
                    mcview::Mode::Door { .. } => "redsim · click the lever",
                    _ => "redsim",
                })),
            }
            l.player = Some(p);
            l.redraw = true;
        }
        Err(e) => {
            let el = v.el.clone();
            app.fail(&el, &e);
        }
    }
    if let Some(v) = app.views.get_mut(&id) {
        v.done = true;
    }
}

/// Advance and draw one frame; true while it needs more frames (playing, dragging).
pub fn frame(app: &mut App, id: u32, ts: f64) -> bool {
    let atlas = app.atlas.clone();
    let mut need_ready = false;
    let more = {
        let Some(v) = app.views.get_mut(&id) else { return false };
        let (w, h) = (v.w.max(1), v.h.max(1));
        let Some(l) = v.live.as_mut() else { return false };
        let dt = l.last_ts.map(|t| ((ts - t) / 1000.0) as f32).unwrap_or(0.0);
        l.last_ts = Some(ts);
        let (Some(p), Some(g)) = (l.player.as_mut(), l.gpu.as_mut()) else { return false };
        let changed = p.update(dt) | std::mem::take(&mut l.redraw);
        if changed {
            let (solid, translucent) = p.triangles(&atlas);
            match g.draw(&Frame { solid: &solid, translucent: &translucent, camera: p.camera, width: w, height: h }) {
                Ok(()) => need_ready = true,
                Err(e) => log(&e),
            }
            l.label.set_text_content(Some(&p.label()));
        }
        p.playing
    };
    if need_ready {
        app.ready(id);
    }
    more
}

impl Live {
    fn toggle_play(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.playing = !p.playing;
            self.play.set_text_content(Some(if p.playing { "⏸" } else { "▶" }));
            self.last_ts = None;
            self.redraw = true;
        }
    }

    fn step(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.playing = false;
            self.play.set_text_content(Some("▶"));
            p.step();
            self.redraw = true;
        }
    }

    pub fn resized(&mut self) {
        self.redraw = true;
    }

    fn pointer_move(&mut self, e: &PointerEvent, v: &crate::View) {
        let Some(d) = self.drag.as_mut() else { return };
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        let (dx, dy) = (x - d.x, y - d.y);
        if dx.abs() + dy.abs() > 3.0 {
            d.moved = true;
        }
        if !d.moved {
            return;
        }
        d.x = x;
        d.y = y;
        let h = v.el.get_bounding_client_rect().height().max(1.0);
        if let Some(p) = self.player.as_mut() {
            if d.pan {
                p.camera.pan((dx / h) as f32, (dy / h) as f32);
            } else {
                p.camera.orbit(-(dx as f32) * 0.008, (dy as f32) * 0.008);
            }
            self.redraw = true;
        }
    }

    fn pointer_up(&mut self, e: &PointerEvent, atlas: &mcview::atlas::Atlas, v: &crate::View) {
        let Some(d) = self.drag.take() else { return };
        if d.moved {
            return;
        }
        let r = v.canvas.get_bounding_client_rect();
        let x = ((e.client_x() as f64 - r.left()) / r.width().max(1.0) * 2.0 - 1.0) as f32;
        let y = (1.0 - (e.client_y() as f64 - r.top()) / r.height().max(1.0) * 2.0) as f32;
        let aspect = (r.width() / r.height().max(1.0)) as f32;
        if let Some(p) = self.player.as_mut()
            && p.click(atlas, x, y, aspect).is_some()
        {
            self.redraw = true;
        }
    }
}
