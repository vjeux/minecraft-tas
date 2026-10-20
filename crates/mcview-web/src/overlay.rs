//! The live viewer's HTML overlays: a circuit's tags (its inputs IN, A, B... in lime, outputs in
//! orange, a shulker design's ITEMS / BOXES chests, a BUD switch's TARGET), ONE compact label per
//! container with what it holds (an icon and a count per kind: `[cobblestone] 1,344`, a shulker
//! box `12/27`, at most two kinds and `+n`), and the outputs' recent history. Placed every frame
//! by projecting their points with the player's camera, then laid out so that nothing overlaps:
//! a tag or label that would cover another goes higher (or aside) with a leader line back to its
//! cell; a label with no free spot is left out.

use crate::document;
use mcview::atlas::Atlas;
use mcview::labels::{self, Entry, Key, Rect, spot};
use mcview::player::{HISTORY, Player};
use std::collections::HashMap;
use wasm_bindgen::{Clamped, JsCast};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement};

/// One container's label: its element, what it shows (to rebuild only on change), its size.
struct Label {
    el: HtmlElement,
    content: String,
    size: (f64, f64),
}

/// A tag (a port's name over its cell); a container there (a judge's chest) shows its contents
/// inside the tag, after the name: one pill per chest.
struct Tag {
    el: HtmlElement,
    /// The world point it hangs at.
    point: [f32; 3],
    /// Its cell (the container whose contents it shows).
    cell: [i32; 3],
    contents: HtmlElement,
    content: String,
    /// Its size, measured again when its contents change.
    size: (f64, f64),
}

pub struct Overlays {
    layer: HtmlElement,
    /// The leader lines from moved tags and labels to their cells.
    leaders: Option<(HtmlCanvasElement, CanvasRenderingContext2d)>,
    tags: Vec<Tag>,
    /// The input tags (input index, tag): pressing one presses its input.
    pub input_tags: Vec<(usize, HtmlElement)>,
    labels: HashMap<Key, Label>,
    /// Item icons as data URLs (None: no texture: a coloured square), for the game assets
    /// `icons_of` (rebuilt when the item atlas arrives).
    icons: HashMap<String, Option<String>>,
    icons_of: (usize, bool),
    /// Container contents shown (the bar's toggle).
    pub containers_on: bool,
    history: Option<(HtmlCanvasElement, CanvasRenderingContext2d)>,
}

fn div(css: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element("div").unwrap().dyn_into().unwrap();
    let _ = e.set_attribute("style", css);
    e
}

fn span(css: &str, text: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element("span").unwrap().dyn_into().unwrap();
    let _ = e.set_attribute("style", css);
    e.set_text_content(Some(text));
    e
}

/// The colour of an item without a texture (the items redsim models).
fn item_color(item: &str) -> &'static str {
    match item {
        "stone" | "cobblestone" => "#8f8f8f",
        "redstone" => "#d01c10",
        "snowball" => "#f2f6fa",
        "ender_pearl" => "#1f8a7a",
        "wooden_shovel" => "#a07a45",
        "diamond_sword" => "#4ad8d0",
        "paper" => "#f4f1e4",
        id if id.ends_with("shulker_box") => "#9a6ea6",
        _ => "#c0c0c0",
    }
}

/// The game texture set an icon was made from (its address, and whether the item atlas is in).
fn game_of(atlas: &Atlas) -> (usize, bool) {
    atlas.game.as_ref().map(|g| (std::rc::Rc::as_ptr(g) as usize, g.entities)).unwrap_or((0, false))
}

fn show_at(e: &HtmlElement, r: &Rect) {
    let s = e.style();
    let _ = s.set_property("display", "flex");
    let _ = s.set_property("left", &format!("{:.1}px", r.x0));
    let _ = s.set_property("top", &format!("{:.1}px", r.y0));
}

fn hide(e: &HtmlElement) {
    let _ = e.style().set_property("display", "none");
}

/// An element's size (laid out now if needed).
fn measure(e: &HtmlElement) -> (f64, f64) {
    let _ = e.style().set_property("display", "flex");
    (e.offset_width() as f64, e.offset_height() as f64)
}

impl Overlays {
    pub fn new(parent: &HtmlElement) -> Overlays {
        let layer = div("position:absolute;left:0;top:0;right:0;bottom:0;pointer-events:none;overflow:hidden;z-index:1;font:600 11px system-ui,sans-serif");
        let _ = parent.append_child(&layer);
        let leaders = (|| {
            let c: HtmlCanvasElement = document().create_element("canvas").ok()?.dyn_into().ok()?;
            let _ = c.set_attribute("style", "position:absolute;left:0;top:0;width:100%;height:100%;pointer-events:none");
            let _ = layer.append_child(&c);
            let ctx = c.get_context("2d").ok()??.dyn_into::<CanvasRenderingContext2d>().ok()?;
            Some((c, ctx))
        })();
        Overlays { layer, leaders, tags: Vec::new(), input_tags: Vec::new(), labels: HashMap::new(), icons: HashMap::new(), icons_of: (0, false), containers_on: true, history: None }
    }

    /// The tags and the history strip of a (new) player.
    pub fn setup(&mut self, p: &Player) {
        for t in self.tags.drain(..) {
            t.el.remove();
        }
        for (_, l) in self.labels.drain() {
            l.el.remove();
        }
        self.input_tags.clear();
        let make = |layer: &HtmlElement, text: &str, color: &str, cell: [i32; 3]| -> Tag {
            let el = div(&format!("position:absolute;display:none;align-items:center;gap:4px;padding:1px 5px;border-radius:6px;background:rgba(0,0,0,0.62);color:{color};white-space:nowrap;width:max-content;letter-spacing:0.5px;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none"));
            let _ = el.append_child(&span("", text));
            let contents = span("display:none;align-items:center;gap:2px;color:#fff;font:600 10px/14px system-ui,sans-serif;letter-spacing:0", "");
            let _ = el.append_child(&contents);
            let _ = layer.append_child(&el);
            Tag { el, point: [cell[0] as f32 + 0.5, cell[1] as f32 + 1.05, cell[2] as f32 + 0.5], cell, contents, content: String::new(), size: (0.0, 0.0) }
        };
        for (i, ci) in p.session.circuit_inputs.iter().enumerate() {
            let name: String = ci.name.to_uppercase().chars().take(3).collect();
            let t = make(&self.layer, &name, "#a6f05a", ci.pos);
            // pressing the tag presses its input (a tap or a hold)
            let s = t.el.style();
            let _ = s.set_property("pointer-events", "auto");
            let _ = s.set_property("cursor", "pointer");
            let _ = s.set_property("touch-action", "none");
            self.input_tags.push((i, t.el.clone()));
            self.tags.push(t);
        }
        // a circuit's other ports: its outputs, a shulker design's chests, a BUD switch's target
        for (label, pos, colour) in mcview::circuit::port_tags(&p.session.design) {
            let t = make(&self.layer, &label, colour, pos);
            self.tags.push(t);
        }
        if let Some((c, _)) = self.history.take() {
            c.remove();
        }
        if !p.session.circuit_inputs.is_empty() && !p.history.is_empty() {
            let c: HtmlCanvasElement = document().create_element("canvas").unwrap().dyn_into().unwrap();
            let rows = p.history.len() as u32;
            c.set_width(HISTORY as u32 * 3);
            c.set_height(rows * 8 + (rows - 1) * 2);
            let _ = c.set_attribute("style", &format!("position:absolute;right:8px;top:34px;width:{}px;height:{}px;border-radius:3px;background:rgba(0,0,0,0.45);image-rendering:pixelated", HISTORY * 3, rows * 8 + (rows - 1) * 2));
            c.set_title("the output, on (orange) or off, over the last 40 game ticks");
            let _ = self.layer.append_child(&c);
            if let Some(ctx) = c.get_context("2d").ok().flatten().and_then(|x| x.dyn_into::<CanvasRenderingContext2d>().ok()) {
                self.history = Some((c, ctx));
            }
        }
    }

    /// An item's icon as a data URL (cached; None: no texture for it).
    fn icon(&mut self, atlas: &Atlas, id: &str) -> Option<String> {
        if let Some(u) = self.icons.get(id) {
            return u.clone();
        }
        let url = labels::item_icon(atlas, id).and_then(|rgba| {
            let c: HtmlCanvasElement = document().create_element("canvas").ok()?.dyn_into().ok()?;
            c.set_width(16);
            c.set_height(16);
            let ctx = c.get_context("2d").ok()??.dyn_into::<CanvasRenderingContext2d>().ok()?;
            let img = web_sys::ImageData::new_with_u8_clamped_array_and_sh(Clamped(&rgba), 16, 16).ok()?;
            ctx.put_image_data(&img, 0.0, 0.0).ok()?;
            c.to_data_url().ok()
        });
        if url.is_none()
            && labels::wants_item_atlas(atlas, id)
            && let Some(g) = &atlas.game
        {
            // the item atlas loads (the page's next frame), then the icons are made again
            g.wants_entities.set(true);
        }
        self.icons.insert(id.to_string(), url.clone());
        url
    }

    /// A label's contents: per kind (two at most) its icon and count, then `+n` more kinds.
    fn fill_label(&mut self, atlas: &Atlas, el: &HtmlElement, entries: &[Entry]) {
        el.set_inner_html("");
        for e in entries.iter().take(2) {
            let id = match e {
                Entry::Items { id, .. } | Entry::Boxes { id, .. } => *id,
            };
            let icon: HtmlElement = match self.icon(atlas, id) {
                Some(url) => {
                    let img: HtmlElement = document().create_element("img").unwrap().dyn_into().unwrap();
                    let _ = img.set_attribute("src", &url);
                    let _ = img.set_attribute("alt", "");
                    let _ = img.set_attribute("style", "width:12px;height:12px;image-rendering:pixelated;flex:none");
                    img
                }
                None => div(&format!("width:10px;height:10px;border-radius:2px;flex:none;background:{}", item_color(id))),
            };
            let _ = el.append_child(&icon);
            let _ = el.append_child(&span("margin-right:2px", &labels::entry_text(e)));
            // several boxes: how full they are, as a bar
            if let Entry::Boxes { n, used, slots, .. } = e
                && *n > 1
            {
                let bar = div("width:14px;height:4px;border-radius:2px;background:rgba(255,255,255,0.25);overflow:hidden;flex:none;margin-right:2px");
                let fill = div(&format!("height:100%;width:{:.0}%;background:#d9a8ff", *used as f64 * 100.0 / (*slots).max(1) as f64));
                let _ = bar.append_child(&fill);
                let _ = el.append_child(&bar);
            }
        }
        if entries.len() > 2 {
            let _ = el.append_child(&span("color:#ddd", &format!("+{}", entries.len() - 2)));
        }
        let title: Vec<String> = entries.iter().map(labels::entry_title).collect();
        let _ = el.set_attribute("aria-label", &title.join(", "));
    }

    /// Place everything for the player's camera (`w` x `h` CSS px view) and its current tick.
    pub fn update(&mut self, p: &Player, atlas: &Atlas, w: f64, h: f64) {
        let aspect = (w / h.max(1.0)) as f32;
        let (f, r, u) = p.camera.basis();
        let eye = p.camera.eye();
        let t = (p.camera.vfov.to_radians() / 2.0).tan();
        let project = |q: [f32; 3]| -> Option<(f64, f64)> {
            let v = [q[0] - eye[0], q[1] - eye[1], q[2] - eye[2]];
            let dot = |a: [f32; 3]| a[0] * v[0] + a[1] * v[1] + a[2] * v[2];
            let z = dot(f);
            if z <= 0.05 {
                return None;
            }
            let (x, y) = (dot(r) / (z * t * aspect), dot(u) / (z * t));
            if x.abs() > 1.1 || y.abs() > 1.1 {
                return None;
            }
            Some(((x as f64 + 1.0) / 2.0 * w, (1.0 - y as f64) / 2.0 * h))
        };
        let mut placed: Vec<Rect> = Vec::new();
        let mut leaders: Vec<((f64, f64), Rect)> = Vec::new();
        // the history strip stays clear
        if let Some((c, _)) = &self.history {
            let (cw, ch) = (c.offset_width() as f64, c.offset_height() as f64);
            placed.push(Rect { x0: w - 8.0 - cw, y0: 34.0, x1: w - 8.0, y1: 34.0 + ch });
        }
        // what the containers hold (nearest first)
        let game = game_of(atlas);
        if game != self.icons_of {
            self.icons.clear();
            self.icons_of = game;
        }
        let mut now = if self.containers_on { labels::summaries(&p.session, p.partial) } else { Vec::new() };
        let dist = |a: [f32; 3]| (a[0] - eye[0]).powi(2) + (a[1] - eye[1]).powi(2) + (a[2] - eye[2]).powi(2);
        now.sort_by(|a, b| dist(a.anchor).total_cmp(&dist(b.anchor)).then(a.key.cmp(&b.key)));
        // the tags first, always shown (at their cell when no free spot is left); a container
        // under a tag (a judge's chest) shows its contents in the tag
        let view = Rect { x0: 0.0, y0: 0.0, x1: w, y1: h };
        let mut in_tags: Vec<Key> = Vec::new();
        for i in 0..self.tags.len() {
            let cell = self.tags[i].cell;
            let held = now.iter().find(|s| s.key == Key::Block(cell));
            let content = match held {
                Some(s) => format!("{:?}{:?}", s.entries, self.icons_of),
                None => String::new(),
            };
            if let Some(s) = held {
                in_tags.push(s.key);
            }
            if self.tags[i].content != content || self.tags[i].size.0 <= 0.0 {
                let contents = self.tags[i].contents.clone();
                match held {
                    Some(s) => {
                        let entries = s.entries.clone();
                        self.fill_label(atlas, &contents, &entries);
                        let _ = contents.style().set_property("display", "inline-flex");
                    }
                    None => {
                        contents.set_inner_html("");
                        let _ = contents.style().set_property("display", "none");
                    }
                }
                let t = &mut self.tags[i];
                t.content = content;
                t.size = measure(&t.el);
            }
            let t = &self.tags[i];
            let Some((ax, ay)) = project(t.point) else {
                hide(&t.el);
                continue;
            };
            let (wt, ht) = t.size;
            let (rect, moved) = spot(&placed, &view, ax, ay, wt, ht, 5).unwrap_or((Rect { x0: ax - wt / 2.0, y0: ay - 2.0 - ht, x1: ax + wt / 2.0, y1: ay - 2.0 }, false));
            show_at(&t.el, &rect);
            placed.push(rect);
            if moved {
                leaders.push(((ax, ay), rect));
            }
        }
        // the other containers' labels; clear of the badges (top), the bar (bottom), and every
        // outlined cell (the ports and inputs: a label never covers one)
        now.retain(|s| !in_tags.contains(&s.key));
        for c in p.look.outlines.iter().copied().chain(p.look.port_outlines.iter().map(|(c, _)| *c)) {
            let corners: Vec<(f64, f64)> = (0..8)
                .filter_map(|k| project([c[0] as f32 + (k & 1) as f32, c[1] as f32 + ((k >> 1) & 1) as f32, c[2] as f32 + ((k >> 2) & 1) as f32]))
                .collect();
            if corners.len() == 8 {
                let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
                for (x, y) in corners {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
                placed.push(Rect { x0, y0, x1, y1 });
            }
        }
        let keep: Vec<Key> = now.iter().map(|s| s.key).collect();
        self.labels.retain(|k, l| {
            let k = keep.contains(k);
            if !k {
                l.el.remove();
            }
            k
        });
        let safe = Rect { x0: 2.0, y0: 30.0, x1: w - 2.0, y1: h - 62.0 };
        for s in &now {
            let content = format!("{:?}{:?}", s.entries, self.icons_of);
            if !self.labels.contains_key(&s.key) {
                let el = div("position:absolute;display:none;align-items:center;gap:2px;padding:1px 4px;border-radius:5px;background:rgba(0,0,0,0.68);color:#fff;font:600 10px/14px system-ui,sans-serif;white-space:nowrap;width:max-content;pointer-events:none");
                let _ = self.layer.append_child(&el);
                self.labels.insert(s.key, Label { el, content: String::new(), size: (0.0, 0.0) });
            }
            if self.labels[&s.key].content != content {
                let el = self.labels[&s.key].el.clone();
                self.fill_label(atlas, &el, &s.entries);
                let size = measure(&el);
                let l = self.labels.get_mut(&s.key).unwrap();
                l.content = content;
                l.size = size;
            }
            let l = &self.labels[&s.key];
            let at = project(s.anchor).and_then(|(ax, ay)| spot(&placed, &safe, ax, ay, l.size.0, l.size.1, 6).map(|(r, moved)| ((ax, ay), r, moved)));
            match at {
                Some((a, rect, moved)) => {
                    show_at(&l.el, &rect);
                    placed.push(rect);
                    if moved {
                        leaders.push((a, rect));
                    }
                }
                None => hide(&l.el),
            }
        }
        self.draw_leaders(&leaders, w, h);
        // the outputs' history
        if let Some((c, ctx)) = &self.history {
            let (cw, ch) = (c.width() as f64, c.height() as f64);
            ctx.clear_rect(0.0, 0.0, cw, ch);
            for (row, hist) in p.history.iter().enumerate() {
                let y = row as f64 * 10.0;
                let pad = HISTORY - hist.len().min(HISTORY);
                for (k, on) in hist.iter().enumerate() {
                    ctx.set_fill_style_str(if *on { "#ffab3d" } else { "#3a3a3a" });
                    ctx.fill_rect(((pad + k) * 3) as f64, y + if *on { 0.0 } else { 5.0 }, 2.0, if *on { 8.0 } else { 3.0 });
                }
            }
        }
    }

    /// A thin line from each moved tag or label down to its cell, with a dot there.
    fn draw_leaders(&self, leaders: &[((f64, f64), Rect)], w: f64, h: f64) {
        let Some((c, ctx)) = &self.leaders else { return };
        let dpr = crate::window().device_pixel_ratio().clamp(1.0, 3.0);
        let (pw, ph) = ((w * dpr).round().max(1.0) as u32, (h * dpr).round().max(1.0) as u32);
        if c.width() != pw || c.height() != ph {
            c.set_width(pw);
            c.set_height(ph);
        }
        ctx.clear_rect(0.0, 0.0, pw as f64, ph as f64);
        if leaders.is_empty() {
            return;
        }
        ctx.set_line_width(dpr);
        ctx.set_stroke_style_str("rgba(255,255,255,0.8)");
        ctx.set_fill_style_str("rgba(255,255,255,0.9)");
        for ((ax, ay), r) in leaders {
            // to the nearest point of the box's edge facing the cell
            let x = ax.clamp(r.x0 + 3.0, r.x1 - 3.0);
            let y = if *ay >= r.y1 { r.y1 } else if *ay <= r.y0 { r.y0 } else { ay.clamp(r.y0, r.y1) };
            ctx.begin_path();
            ctx.move_to(ax * dpr, ay * dpr);
            ctx.line_to(x * dpr, y * dpr);
            ctx.stroke();
            ctx.begin_path();
            let _ = ctx.arc(ax * dpr, ay * dpr, 1.6 * dpr, 0.0, std::f64::consts::TAU);
            ctx.fill();
        }
    }
}
