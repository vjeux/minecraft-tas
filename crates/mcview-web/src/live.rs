//! The live viewer: its own WebGL2 context on the element's canvas, a touch-sized control bar
//! (reset, play / pause, one game tick, speed, flip the lever, the tick counter in gt and seconds)
//! and gestures: one finger or the mouse drags to orbit, two fingers pinch to zoom and drag to pan
//! (right-drag / shift-drag with a mouse), the wheel zooms, a tap or click on a lever or button
//! uses it, a double tap / double click resets the view. Keys: space play/pause, right arrow or
//! `.` step, `r` reset, `f` flip.

use crate::gpu::Gpu;
use crate::{App, document, ensure_frame, log, with_app};
use mcview::player::Player;
use mcview::render::Frame;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlButtonElement, HtmlElement, KeyboardEvent, PointerEvent, WheelEvent};

/// A pointer down on the canvas (finger, pen or mouse).
struct Touch {
    id: i32,
    x: f64,
    y: f64,
    /// Where it went down (a tap moves less than a few pixels).
    sx: f64,
    sy: f64,
}

pub struct Live {
    gpu: Option<Gpu>,
    player: Option<Player>,
    label: HtmlElement,
    bar: HtmlElement,
    status: HtmlElement,
    play: HtmlButtonElement,
    flip: HtmlButtonElement,
    speed: HtmlButtonElement,
    speed_i: usize,
    touches: Vec<Touch>,
    /// The current gesture moved (orbit, pan, pinch): not a tap.
    moved: bool,
    /// Mouse right-drag or shift-drag: pan instead of orbit.
    pan_button: bool,
    /// Two-finger gesture: last finger distance and midpoint.
    pinch: Option<(f64, f64, f64)>,
    /// When the last tap ended (ms), for double taps.
    last_tap: f64,
    last_ts: Option<f64>,
    redraw: bool,
}

/// Speeds the speed button cycles through: label, game ticks per second.
const SPEEDS: [(&str, f32); 7] = [("1×", 20.0), ("2×", 40.0), ("5×", 100.0), ("20×", 400.0), ("100×", 2000.0), ("¼×", 5.0), ("½×", 10.0)];

fn el(tag: &str) -> HtmlElement {
    document().create_element(tag).unwrap().dyn_into().unwrap()
}

fn style(e: &HtmlElement, props: &[(&str, &str)]) {
    let s = e.style();
    for (k, v) in props {
        let _ = s.set_property(k, v);
    }
}

/// Inline SVG icons (glyphs like ⏭ are missing from many fonts).
const ICON_PLAY: &str = "M8 5v14l11-7z";
const ICON_PAUSE: &str = "M6 5h4v14H6zM14 5h4v14h-4z";
const ICON_STEP: &str = "M5 5v14l9-7zM15 5h3v14h-3z";
const ICON_RESET: &str = "M12 5V1.5L7 6l5 4.5V7a5 5 0 1 1-5 5H5a7 7 0 1 0 7-7z";
const ICON_LEVER: &str = "M17 7H7a5 5 0 0 0 0 10h10a5 5 0 0 0 0-10zm0 8a3 3 0 1 1 0-6 3 3 0 0 1 0 6z";

fn icon(path: &str) -> String {
    format!("<svg width=\"24\" height=\"24\" viewBox=\"0 0 24 24\" fill=\"currentColor\" aria-hidden=\"true\" style=\"display:block\"><path d=\"{path}\"/></svg>")
}

/// A touch-sized (44 px) button showing an icon (an SVG path) or a short text.
fn button(content: &str, title: &str) -> HtmlButtonElement {
    let b: HtmlButtonElement = document().create_element("button").unwrap().dyn_into().unwrap();
    if content.starts_with('M') {
        b.set_inner_html(&icon(content));
    } else {
        b.set_text_content(Some(content));
    }
    b.set_title(title);
    let _ = b.set_attribute("aria-label", title);
    style(
        b.unchecked_ref(),
        &[
            ("min-width", "44px"),
            ("height", "44px"),
            ("padding", "0 10px"),
            ("flex", "0 0 auto"),
            ("display", "inline-flex"),
            ("align-items", "center"),
            ("justify-content", "center"),
            ("background", "rgba(20,22,28,0.72)"),
            ("color", "#fff"),
            ("border", "1px solid rgba(255,255,255,0.22)"),
            ("border-radius", "12px"),
            ("font", "600 18px/1 system-ui, sans-serif"),
            ("cursor", "pointer"),
            ("pointer-events", "auto"),
            ("touch-action", "manipulation"),
            ("-webkit-tap-highlight-color", "transparent"),
            ("user-select", "none"),
        ],
    );
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

/// A button that runs `f` on the live view, then draws.
fn on_click(b: &HtmlButtonElement, id: u32, f: fn(&mut Live)) {
    on(b, "click", move |e: web_sys::Event| {
        e.stop_propagation();
        with_live(id, |l, _, _| f(l));
        ensure_frame();
    });
}

/// Build the controls, start the WebGL2 context and the design's fetch.
pub fn start(app: &mut App, id: u32) {
    let v = app.views.get_mut(&id).unwrap();
    let root = v.el.clone();
    let _ = root.set_attribute("tabindex", "0");
    let _ = root.style().set_property("outline", "none");
    let bar = el("div");
    style(
        &bar,
        &[
            ("position", "absolute"),
            ("left", "0"),
            ("right", "0"),
            ("bottom", "0"),
            ("display", "flex"),
            ("gap", "6px"),
            ("align-items", "center"),
            ("flex-wrap", "nowrap"),
            ("padding", "8px"),
            ("height", "60px"),
            ("overflow", "hidden"),
            // only the buttons take the pointer: a tap on the scene behind the bar (a lever near
            // the bottom of the view) reaches the canvas
            ("pointer-events", "none"),
            ("background", "linear-gradient(transparent, rgba(0,0,0,0.55))"),
            ("color", "#fff"),
            ("font", "13px system-ui, sans-serif"),
            ("z-index", "2"),
            ("box-sizing", "border-box"),
        ],
    );
    let reset = button(ICON_RESET, "Back to the start (r)");
    let play = button(ICON_PLAY, "Play / pause (space)");
    let _ = play.style().set_property("min-width", "52px");
    let step = button(ICON_STEP, "One game tick (→)");
    let speed = button(SPEEDS[0].0, "Speed (1× = 20 game ticks per second, real time)");
    let _ = speed.style().set_property("font-size", "15px");
    let flip = button(ICON_LEVER, "Flip the lever (f), or tap the lever");
    // the tick counter: a badge at the top right, fixed width (tabular digits), so nothing moves
    // while it runs and the bar's buttons fit a phone
    let label = el("div");
    style(
        &label,
        &[
            ("position", "absolute"),
            ("right", "8px"),
            ("top", "8px"),
            ("min-width", "17ch"),
            ("height", "20px"),
            ("line-height", "20px"),
            ("padding", "0 8px"),
            ("box-sizing", "content-box"),
            ("border-radius", "8px"),
            ("background", "rgba(0,0,0,0.45)"),
            ("color", "#fff"),
            ("font", "13px system-ui, sans-serif"),
            ("font-variant-numeric", "tabular-nums"),
            ("text-align", "right"),
            ("white-space", "nowrap"),
            ("pointer-events", "none"),
            ("z-index", "2"),
        ],
    );
    label.set_text_content(Some("0 gt · 0.00 s"));
    for c in [reset.unchecked_ref::<HtmlElement>(), play.unchecked_ref(), step.unchecked_ref(), speed.unchecked_ref(), flip.unchecked_ref()] {
        let _ = bar.append_child(c);
    }
    let _ = root.append_child(&bar);
    let _ = root.append_child(&label);
    let _ = flip.style().set_property("display", "none");
    // what is shown: redsim, a recording, or the design as built
    let status = el("div");
    style(
        &status,
        &[
            ("position", "absolute"),
            ("left", "8px"),
            ("top", "8px"),
            ("max-width", "calc(100% - 200px)"),
            ("height", "20px"),
            ("line-height", "20px"),
            ("padding", "0 8px"),
            ("border-radius", "8px"),
            ("background", "rgba(0,0,0,0.45)"),
            ("color", "#fff"),
            ("font", "12px system-ui, sans-serif"),
            ("white-space", "nowrap"),
            ("overflow", "hidden"),
            ("text-overflow", "ellipsis"),
            ("pointer-events", "none"),
            ("z-index", "2"),
        ],
    );
    let _ = root.append_child(&status);
    v.live = Some(Live {
        gpu: None,
        player: None,
        label,
        bar: bar.clone(),
        status,
        play: play.clone(),
        flip: flip.clone(),
        speed: speed.clone(),
        speed_i: 0,
        touches: Vec::new(),
        moved: false,
        pan_button: false,
        pinch: None,
        last_tap: -1e9,
        last_ts: None,
        redraw: true,
    });
    let canvas = v.canvas.clone();
    let _ = canvas.style().set_property("touch-action", "none");
    on_click(&reset, id, |l| l.reset());
    on_click(&play, id, |l| l.toggle_play());
    on_click(&step, id, |l| l.step());
    on_click(&flip, id, |l| l.flip());
    on_click(&speed, id, |l| l.next_speed());
    // gestures
    on(&canvas, "pointerdown", move |e: PointerEvent| {
        with_live(id, |l, _, v| {
            l.pointer_down(&e);
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
        with_live(id, |l, atlas, v| l.pointer_up(&e, atlas, v, true));
        ensure_frame();
    });
    on(&canvas, "pointercancel", move |e: PointerEvent| {
        with_live(id, |l, atlas, v| l.pointer_up(&e, atlas, v, false));
        ensure_frame();
    });
    on(&canvas, "wheel", move |e: WheelEvent| {
        e.prevent_default();
        with_live(id, |l, _, _| l.zoom((e.delta_y() as f32 * 0.0015).exp()));
        ensure_frame();
    });
    on(&canvas, "dblclick", move |e: web_sys::MouseEvent| {
        e.prevent_default();
        with_live(id, |l, _, _| l.home());
        ensure_frame();
    });
    on(&canvas, "contextmenu", move |e: web_sys::Event| e.prevent_default());
    on(&root, "keydown", move |e: KeyboardEvent| {
        let handled = with_live(id, |l, _, _| {
            match e.key().as_str() {
                " " => l.toggle_play(),
                "ArrowRight" | "." => l.step(),
                "r" => l.reset(),
                "f" => l.flip(),
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
    let has_trace = v.trace.is_some();
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
    // a recording to play (data-trace): wait for it too; without it the design is shown as built
    let trace = if has_trace {
        match app.trace_of(id) {
            None => return,
            Some(r) => Some(r),
        }
    } else {
        None
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
            } else {
                p.show_inputs(&app.atlas, aspect);
            }
            if let Some(t) = v.tick {
                p.session.run(t);
            }
            let mut trace_note = None;
            match trace {
                Some(Ok(tt)) => match p.session.attach_trace(&tt) {
                    Ok(()) => {
                        p.look = mcview::scene::default_look(&p.session);
                        p.invalidate();
                        let r = p.session.replay.as_ref().unwrap();
                        trace_note = Some(format!("recorded in the game · {} gt loop", r.period));
                    }
                    Err(e) => trace_note = Some(format!("recording unreadable: {e}")),
                },
                Some(Err(e)) => trace_note = Some(format!("recording unavailable: {e}")),
                None => {}
            }
            let l = v.live.as_mut().unwrap();
            if p.session.simulated() && (p.session.mode.is_door() || !p.session.inputs.is_empty()) {
                let _ = l.flip.style().set_property("display", "inline-flex");
            }
            if !p.session.playable() {
                l.play.set_disabled(true);
                let _ = l.play.style().set_property("opacity", "0.4");
            }
            match (&trace_note, &p.session.unsimulated) {
                (Some(n), _) => l.status.set_text_content(Some(n)),
                (None, Some(why)) => l.status.set_text_content(Some(&format!("shown as built, not simulated: {why}"))),
                (None, None) => l.status.set_text_content(Some(match p.session.mode {
                    mcview::Mode::Door { .. } => "simulated by redsim · tap the lever",
                    _ => "simulated by redsim",
                })),
            }
            l.player = Some(p);
            l.redraw = true;
            if v.autoplay {
                l.toggle_play();
            }
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
            if crate::debug() {
                // for `mctas web layout`: nothing may move while the counter runs
                let r = |e: &HtmlElement| {
                    let b = e.get_bounding_client_rect();
                    format!("{:.1},{:.1},{:.1},{:.1}", b.x(), b.y(), b.width(), b.height())
                };
                log(&format!("layout view {id} tick {} el {} bar {} label {} status {}", p.session.tick, r(&v.el), r(&l.bar), r(&l.label), r(&l.status)));
                // for `mctas web click`: where a click uses each input (fractions of the canvas),
                // and what the sim shows
                let cr = v.canvas.get_bounding_client_rect();
                let aspect = (cr.width() / cr.height().max(1.0)) as f32;
                let inputs: Vec<String> = p
                    .session
                    .inputs
                    .clone()
                    .iter()
                    .map(|c| {
                        let on = p.session.is_on(*c) as u8;
                        match p.screen_point(&atlas, *c, aspect) {
                            Some((x, y)) => format!("{:.4},{:.4},{on}", (x + 1.0) / 2.0, (1.0 - y) / 2.0),
                            None => format!("-,-,{on}"),
                        }
                    })
                    .collect();
                let door = p.session.door_blocks().map(|n| n.to_string()).unwrap_or_else(|| "-".into());
                let active = p.session.design.blocks.keys().filter(|c| !p.session.inputs.contains(c) && p.session.is_on(**c)).count();
                let _ = v.el.set_attribute("data-mcview-state", &format!("tick={} playing={} door={door} active={active} inputs={}", p.session.tick, p.playing as u8, inputs.join("|")));
            }
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
            if !p.session.playable() {
                return;
            }
            p.playing = !p.playing;
            self.play.set_inner_html(&icon(if p.playing { ICON_PAUSE } else { ICON_PLAY }));
            self.last_ts = None;
            self.redraw = true;
        }
    }

    fn step(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.playing = false;
            self.play.set_inner_html(&icon(ICON_PLAY));
            p.step();
            self.redraw = true;
        }
    }

    fn reset(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.reset();
            self.redraw = true;
        }
    }

    fn flip(&mut self) {
        if let Some(p) = self.player.as_mut()
            && p.flip()
        {
            self.redraw = true;
            self.play_after_input();
        }
    }

    /// After using a lever or button: run the simulation so it reacts (a door opens).
    fn play_after_input(&mut self) {
        if self.player.as_ref().is_some_and(|p| !p.playing) {
            self.toggle_play();
        }
    }

    fn next_speed(&mut self) {
        self.speed_i = (self.speed_i + 1) % SPEEDS.len();
        let (name, tps) = SPEEDS[self.speed_i];
        self.speed.set_text_content(Some(name));
        self.speed.set_title(&format!("Speed: {name} = {tps} game ticks per second"));
        if let Some(p) = self.player.as_mut() {
            p.tps = tps;
        }
    }

    fn home(&mut self) {
        if let Some(p) = self.player.as_mut() {
            p.camera = p.home;
            self.redraw = true;
        }
    }

    /// Zoom by a distance factor (> 1: farther).
    fn zoom(&mut self, f: f32) {
        if let Some(p) = self.player.as_mut() {
            p.camera.dist = (p.camera.dist * f).clamp(1.0, 2000.0);
            p.keep_camera_above_ground();
            self.redraw = true;
        }
    }

    pub fn resized(&mut self) {
        self.redraw = true;
    }

    fn pointer_down(&mut self, e: &PointerEvent) {
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        self.touches.retain(|t| t.id != e.pointer_id());
        self.touches.push(Touch { id: e.pointer_id(), x, y, sx: x, sy: y });
        match self.touches.len() {
            1 => {
                self.moved = false;
                self.pan_button = e.button() != 0 || e.shift_key();
                self.pinch = None;
            }
            _ => {
                // two fingers: pinch and pan, never a tap
                self.moved = true;
                self.pinch = Some(self.two_finger());
            }
        }
    }

    /// The first two pointers' distance and midpoint.
    fn two_finger(&self) -> (f64, f64, f64) {
        let (a, b) = (&self.touches[0], &self.touches[1]);
        (((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt().max(1.0), (a.x + b.x) / 2.0, (a.y + b.y) / 2.0)
    }

    fn pointer_move(&mut self, e: &PointerEvent, v: &crate::View) {
        let Some(i) = self.touches.iter().position(|t| t.id == e.pointer_id()) else { return };
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        let (dx, dy) = (x - self.touches[i].x, y - self.touches[i].y);
        self.touches[i].x = x;
        self.touches[i].y = y;
        let h = v.el.get_bounding_client_rect().height().max(1.0);
        let Some(p) = self.player.as_mut() else { return };
        if self.touches.len() >= 2 {
            let (d0, mx0, my0) = self.pinch.unwrap_or((1.0, x, y));
            let (a, b) = (&self.touches[0], &self.touches[1]);
            let (d1, mx1, my1) = (((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt().max(1.0), (a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
            p.camera.dist = (p.camera.dist * (d0 / d1) as f32).clamp(1.0, 2000.0);
            p.camera.pan(((mx1 - mx0) / h) as f32, ((my1 - my0) / h) as f32);
            self.pinch = Some((d1, mx1, my1));
        } else {
            let t = &self.touches[i];
            if (t.x - t.sx).abs() + (t.y - t.sy).abs() > 6.0 {
                self.moved = true;
            }
            if !self.moved {
                return;
            }
            if self.pan_button {
                p.camera.pan((dx / h) as f32, (dy / h) as f32);
            } else {
                p.camera.orbit(-(dx as f32) * 0.008, (dy as f32) * 0.008);
            }
        }
        p.keep_camera_above_ground();
        self.redraw = true;
    }

    /// A pointer lifted (`up`) or cancelled: a tap uses the lever or button under it; a double
    /// tap resets the view.
    fn pointer_up(&mut self, e: &PointerEvent, atlas: &mcview::atlas::Atlas, v: &crate::View, up: bool) {
        let was_single = self.touches.len() == 1;
        self.touches.retain(|t| t.id != e.pointer_id());
        if self.touches.len() == 1 {
            self.pinch = None;
        }
        if !up || !was_single || self.moved {
            return;
        }
        let now = crate::window().performance().map(|p| p.now()).unwrap_or(0.0);
        if e.pointer_type() == "touch" && now - self.last_tap < 320.0 {
            self.last_tap = -1e9;
            self.home();
            return;
        }
        self.last_tap = now;
        let r = v.canvas.get_bounding_client_rect();
        let x = ((e.client_x() as f64 - r.left()) / r.width().max(1.0) * 2.0 - 1.0) as f32;
        let y = (1.0 - (e.client_y() as f64 - r.top()) / r.height().max(1.0) * 2.0) as f32;
        let aspect = (r.width() / r.height().max(1.0)) as f32;
        if let Some(p) = self.player.as_mut()
            && p.click(atlas, x, y, aspect).is_some()
        {
            self.redraw = true;
            self.play_after_input();
        }
    }
}
