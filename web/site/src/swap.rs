//! In-place swaps (vjeux: "Don't reload the page when clicking on variants, update inline and keep
//! the current camera for the viewer"):
//!
//! * a build page's tunable-group picker (`nav.picker a.pick`) fetches the member's page and swaps
//!   everything around the live 3D viewer (title, numbers, explanation, downloads, picker, related
//!   lists); the viewer element itself is never removed or moved: its `data-*` attributes are
//!   updated (`data-src` last, with `data-keep-camera`), and webrender's viewer reloads the design
//!   in place with the same camera; `history.pushState` keeps the member's URL, so the back
//!   button and deep links work;
//! * a family page's member rows (`a.view[data-src]`) swap the family viewer's design the same way.
//!
//! Only when the viewer says it can (class `mcview-live-src` or `data-mcview-live` on a ready
//! viewer); otherwise the links navigate as plain links.

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, Node};

/// A build page's live viewer.
const VIEWER: &str = "main > [data-mcview=\"full\"]";

/// The attributes of the viewer contract that follow the design (`data-src` is set last).
const VIEWER_ATTRS: [&str; 10] = ["data-stimulus", "data-phases", "data-input-mode", "data-circuit", "data-trace", "data-kind", "data-autoplay", "data-tick", "data-yaw", "data-pitch"];

fn doc() -> Document {
    web_sys::window().and_then(|w| w.document()).expect("a document")
}

/// Whether `v` is a ready viewer that reloads its design in place.
fn live(v: &Element) -> bool {
    let c = v.class_list();
    c.contains("mcview-ready") && (c.contains("mcview-live-src") || v.has_attribute("data-mcview-live"))
}

fn page_viewer() -> Option<Element> {
    doc().query_selector(VIEWER).ok().flatten()
}

fn current_url() -> String {
    web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default()
}

/// Install the handlers when the page has something to swap.
pub fn install() -> Result<(), JsValue> {
    let d = doc();
    let picker = d.query_selector("nav.picker").ok().flatten().is_some();
    let family = d.get_element_by_id("member-view").is_some();
    if !picker && !family {
        return Ok(());
    }
    // one delegated listener: the picker is replaced by every swap
    let click = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(|e: web_sys::MouseEvent| {
        if e.default_prevented() || e.button() != 0 || e.ctrl_key() || e.meta_key() || e.shift_key() || e.alt_key() {
            return;
        }
        let Some(t) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) else { return };
        if let Ok(Some(a)) = t.closest("nav.picker a.pick") {
            let Some(v) = page_viewer() else { return };
            if !live(&v) {
                return;
            }
            let Ok(a) = a.dyn_into::<web_sys::HtmlAnchorElement>() else { return };
            e.prevent_default();
            let href = a.href();
            if href != current_url() {
                go(href, true);
            }
        } else if let Ok(Some(a)) = t.closest("a.view[data-src]") {
            let Some(v) = doc().get_element_by_id("member-view") else { return };
            if !live(&v) {
                return;
            }
            e.prevent_default();
            show_member(&v, &a);
            if let (Some(w), Some(h)) = (web_sys::window(), a.get_attribute("href")) {
                if let Ok(hist) = w.history() {
                    let _ = hist.push_state_with_url(&JsValue::NULL, "", Some(&h));
                }
            }
        }
    });
    d.add_event_listener_with_callback("click", click.as_ref().unchecked_ref())?;
    click.forget();
    // back / forward: the page of that URL (a family page: the member of its fragment)
    let pop = Closure::<dyn FnMut(web_sys::Event)>::new(|_e: web_sys::Event| {
        if let Some(v) = doc().get_element_by_id("member-view") {
            let hash = web_sys::window().and_then(|w| w.location().hash().ok()).unwrap_or_default();
            let sel = if hash.len() > 1 { format!("a.view[href=\"{hash}\"]") } else { "a.view[data-src]".to_string() };
            if let Ok(Some(a)) = doc().query_selector(&sel) {
                if live(&v) {
                    show_member(&v, &a);
                }
            }
            return;
        }
        let Some(v) = page_viewer() else { return };
        if live(&v) {
            go(current_url(), false);
        } else if let Some(w) = web_sys::window() {
            let _ = w.location().reload();
        }
    });
    web_sys::window().ok_or("no window")?.add_event_listener_with_callback("popstate", pop.as_ref().unchecked_ref())?;
    pop.forget();
    Ok(())
}

/// A family page: show the member of row link `a` in the family viewer, same camera.
fn show_member(v: &Element, a: &Element) {
    let Some(src) = a.get_attribute("data-src") else { return };
    if v.get_attribute("data-src").as_deref() == Some(src.as_str()) {
        return;
    }
    let _ = v.set_attribute("data-keep-camera", "1");
    let _ = v.set_attribute("data-src", &src);
    if let (Some(cap), Some(text)) = (doc().get_element_by_id("member-caption"), a.get_attribute("data-caption")) {
        cap.set_text_content(Some(&text));
    }
    if let Ok(rows) = doc().query_selector_all("tr.showing") {
        for i in 0..rows.length() {
            if let Some(r) = rows.item(i).and_then(|n| n.dyn_into::<Element>().ok()) {
                let _ = r.class_list().remove_1("showing");
            }
        }
    }
    if let Ok(Some(tr)) = a.closest("tr") {
        let _ = tr.class_list().add_1("showing");
    }
}

/// Fetch `href` and swap its content in; push it to the history when asked. Any failure falls
/// back to a plain navigation.
fn go(href: String, push: bool) {
    wasm_bindgen_futures::spawn_local(async move {
        let ok = match fetch_doc(&href).await {
            Ok(nd) => swap(&nd).is_ok(),
            Err(_) => false,
        };
        let Some(w) = web_sys::window() else { return };
        if !ok {
            let _ = w.location().set_href(&href);
            return;
        }
        if push {
            if let Ok(h) = w.history() {
                let _ = h.push_state_with_url(&JsValue::NULL, "", Some(&href));
            }
        }
    });
}

async fn fetch_doc(href: &str) -> Result<Document, JsValue> {
    let w = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(w.fetch_with_str(href)).await?.dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!("{href}: HTTP {}", resp.status())));
    }
    let text = JsFuture::from(resp.text()?).await?.as_string().ok_or("not text")?;
    web_sys::DomParser::new()?.parse_from_string(&text, web_sys::SupportedType::TextHtml)
}

fn children(n: &Node) -> Vec<Node> {
    let list = n.child_nodes();
    (0..list.length()).filter_map(|i| list.item(i)).collect()
}

/// Swap the new page's content around the live viewer (which stays in place).
fn swap(nd: &Document) -> Result<(), JsValue> {
    let d = doc();
    let main = d.query_selector("main")?.ok_or("no main")?;
    let nmain = nd.query_selector("main")?.ok_or("no main in the new page")?;
    let cur = page_viewer().ok_or("no viewer")?;
    let new_v = nd.query_selector(VIEWER)?.ok_or("no viewer in the new page")?;
    // the viewer: its attributes, data-src last (the viewer reloads on it, keeping the camera)
    for a in VIEWER_ATTRS {
        match new_v.get_attribute(a) {
            Some(v) => cur.set_attribute(a, &v)?,
            None => cur.remove_attribute(a)?,
        }
    }
    if let (Some(f), Some(nf)) = (cur.query_selector(".fallback")?, new_v.query_selector(".fallback")?) {
        f.set_inner_html(&nf.inner_html());
    }
    cur.set_attribute("data-keep-camera", "1")?;
    if let Some(src) = new_v.get_attribute("data-src") {
        cur.set_attribute("data-src", &src)?;
    }
    // everything around it
    let cur_node: &Node = cur.as_ref();
    let new_node: &Node = new_v.as_ref();
    for n in children(main.as_ref()) {
        if !n.is_same_node(Some(cur_node)) {
            main.remove_child(&n)?;
        }
    }
    let mut before = true;
    for n in children(nmain.as_ref()) {
        if n.is_same_node(Some(new_node)) {
            before = false;
            continue;
        }
        let n = d.import_node_with_deep(&n, true)?;
        if before {
            main.insert_before(&n, Some(cur_node))?;
        } else {
            main.append_child(&n)?;
        }
    }
    d.set_title(&nd.title());
    Ok(())
}
