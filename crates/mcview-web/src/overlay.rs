//! What a live viewer draws over its canvas as page elements (crisp text, no 3D text needed): a
//! circuit's input and output tags (IN, A, B... in lime, OUT in orange), the contents of the
//! containers (droppers, dispensers, hoppers) above each one, and the outputs' recent history.
//! Placed every frame by projecting their cells with the player's camera.

use crate::document;
use mcview::design::Pos;
use mcview::player::{HISTORY, Player};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlElement};

pub struct Overlays {
    layer: HtmlElement,
    /// (tag, the world point it hangs at).
    tags: Vec<(HtmlElement, [f32; 3])>,
    boxes: HashMap<Pos, (HtmlElement, String)>,
    /// Container contents shown (the bar's toggle).
    pub containers_on: bool,
    history: Option<(HtmlCanvasElement, CanvasRenderingContext2d)>,
}

fn div(css: &str) -> HtmlElement {
    let e: HtmlElement = document().create_element("div").unwrap().dyn_into().unwrap();
    let _ = e.set_attribute("style", css);
    e
}

/// The color of an item's icon (the items redsim models).
fn item_color(item: &str) -> &'static str {
    match item {
        "stone" => "#8f8f8f",
        "redstone" => "#d01c10",
        "snowball" => "#f2f6fa",
        "ender_pearl" => "#1f8a7a",
        "wooden_shovel" => "#a07a45",
        "diamond_sword" => "#4ad8d0",
        _ => "#c0c0c0",
    }
}

impl Overlays {
    pub fn new(parent: &HtmlElement) -> Overlays {
        let layer = div("position:absolute;left:0;top:0;right:0;bottom:0;pointer-events:none;overflow:hidden;z-index:1;font:600 11px system-ui,sans-serif");
        let _ = parent.append_child(&layer);
        Overlays { layer, tags: Vec::new(), boxes: HashMap::new(), containers_on: true, history: None }
    }

    /// The tags and the history strip of a (new) player.
    pub fn setup(&mut self, p: &Player) {
        for (t, _) in self.tags.drain(..) {
            t.remove();
        }
        let tag = |text: &str, color: &str| {
            let t = div(&format!("position:absolute;transform:translate(-50%,-100%);padding:1px 5px;border-radius:6px;background:rgba(0,0,0,0.62);color:{color};white-space:nowrap;letter-spacing:0.5px"));
            t.set_text_content(Some(text));
            t
        };
        for ci in &p.session.circuit_inputs {
            let name: String = ci.name.to_uppercase().chars().take(3).collect();
            let t = tag(&name, "#a6f05a");
            let _ = self.layer.append_child(&t);
            self.tags.push((t, [ci.pos[0] as f32 + 0.5, ci.pos[1] as f32 + 1.05, ci.pos[2] as f32 + 0.5]));
        }
        if !p.session.circuit_inputs.is_empty() {
            for (label, pos) in mcview::circuit::outputs(&p.session.design) {
                let t = tag(&label, "#ffb04a");
                let _ = self.layer.append_child(&t);
                self.tags.push((t, [pos[0] as f32 + 0.5, pos[1] as f32 + 1.05, pos[2] as f32 + 0.5]));
            }
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

    /// Place everything for the player's camera (`w` x `h` CSS px view) and its current tick.
    pub fn update(&mut self, p: &Player, w: f64, h: f64) {
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
        let place = |e: &HtmlElement, at: Option<(f64, f64)>| {
            let s = e.style();
            match at {
                Some((x, y)) => {
                    let _ = s.set_property("display", "block");
                    let _ = s.set_property("left", &format!("{x:.1}px"));
                    let _ = s.set_property("top", &format!("{y:.1}px"));
                }
                None => {
                    let _ = s.set_property("display", "none");
                }
            }
        };
        for (e, q) in &self.tags {
            place(e, project(*q));
        }
        // container contents
        let now = if self.containers_on { p.session.containers() } else { Vec::new() };
        let keep: Vec<Pos> = now.iter().map(|(c, _, _)| *c).collect();
        self.boxes.retain(|c, (e, _)| {
            let k = keep.contains(c);
            if !k {
                e.remove();
            }
            k
        });
        for (cell, _slots, stacks) in now {
            let key = format!("{stacks:?}");
            let entry = self.boxes.entry(cell).or_insert_with(|| {
                let e = div("position:absolute;transform:translate(-50%,-100%);display:flex;gap:2px;padding:2px;border-radius:5px;background:rgba(0,0,0,0.58)");
                let _ = self.layer.append_child(&e);
                (e, String::new())
            });
            if entry.1 != key {
                entry.1 = key;
                let e = &entry.0;
                e.set_inner_html("");
                if stacks.is_empty() {
                    let s = div("width:14px;height:14px;border-radius:3px;border:1px dashed rgba(255,255,255,0.35);box-sizing:border-box");
                    let _ = e.append_child(&s);
                }
                let shown = stacks.len().min(5);
                for (item, count) in stacks.iter().take(shown) {
                    let s = div(&format!("position:relative;width:16px;height:16px;border-radius:3px;background:{};box-shadow:inset 0 -3px rgba(0,0,0,0.25)", item_color(item)));
                    s.set_title(&format!("{count} {}", item.replace('_', " ")));
                    if *count > 1 {
                        let n = div("position:absolute;right:-2px;bottom:-3px;color:#fff;font:700 10px system-ui,sans-serif;text-shadow:0 0 2px #000,0 0 2px #000");
                        n.set_text_content(Some(&count.to_string()));
                        let _ = s.append_child(&n);
                    }
                    let _ = e.append_child(&s);
                }
                if stacks.len() > shown {
                    let more = div("color:#fff;padding:0 2px;line-height:16px");
                    more.set_text_content(Some(&format!("+{}", stacks.len() - shown)));
                    let _ = e.append_child(&more);
                }
            }
            place(&entry.0, project([cell[0] as f32 + 0.5, cell[1] as f32 + 1.0, cell[2] as f32 + 0.5]));
        }
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
}
