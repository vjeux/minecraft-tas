//! `mctas web qa`: the live viewer's visual QA at phone size in WebKit (WebKitGTK, the engine
//! family of iOS Safari, through the site's rig: `site::webkit::run_steps`). Each page's first
//! live viewer, scrolled on screen: screenshots after it is ready (cropped to the viewer and its
//! bar), its tags' boxes (they must not overlap each other, the tick badge or the status line,
//! nor hide under the bar or leave the view), its ticks over a while (it plays; a demo restarts),
//! the picture's brightness, every input control pressed once (the circuit or door reacts), the
//! page's console errors. Writes the crops, a contact sheet of every page and a report.

use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::path::Path;

/// Waits for the page's first live viewer, scrolled to the middle of the window, to be ready
/// (or failed); `none` when the page has none.
const READY: &str = r#"const v = document.querySelector('[data-mcview="full"]');
if (!v) return (document.readyState === 'complete' && performance.now() > 6000) ? 'none' : false;
v.scrollIntoView({block: 'center'});
return /mcview-(ready|error)/.test(v.className) ? v.className : false;"#;

/// Follows the viewer's tick every animation frame (its debug state): the first and last tick,
/// the highest, each restart (the tick going down: when, from, to), whether it plays.
const WATCH: &str = r#"if (window.__qaT) return true;
const v = document.querySelector('[data-mcview="full"]');
const T = window.__qaT = {first: null, last: null, max: 0, drops: [], playing: 0, frames: 0, t0: performance.now()};
(function f() {
  const m = /tick=(\d+) playing=(\d)/.exec(v.getAttribute('data-mcview-state') || '');
  if (m) {
    const t = +m[1];
    if (T.first === null) T.first = t;
    if (T.last !== null && t < T.last) T.drops.push([Math.round(performance.now() - T.t0), T.last, t]);
    T.last = t; T.max = Math.max(T.max, t); T.playing = +m[2]; T.frames++;
  }
  requestAnimationFrame(f);
})();
return true;"#;

/// The viewer now: its box (CSS px), the device scale, its state, its visible tags and fixed parts
/// (badge, status line, bar) as [text, x, y, w, h] relative to it, its input controls and the
/// page's console errors (not WebKitGTK's own media controls', which call an `Intl.DurationFormat`
/// its JavaScriptCore lacks on a page with a `<video controls>`, then fail on its formatter).
const PROBE: &str = r#"const v = document.querySelector('[data-mcview="full"]');
if (!v) return null;
const r = v.getBoundingClientRect();
const vis = e => !!e && getComputedStyle(e).display !== 'none' && getComputedStyle(e).visibility !== 'hidden' && e.getBoundingClientRect().width > 0;
const box = e => { const b = e.getBoundingClientRect(); return [b.x - r.x, b.y - r.y, b.width, b.height]; };
const text = e => e ? (e.innerText || e.textContent || '').replace(/\s+/g, ' ').trim() : '';
const layer = v.querySelector('.mcview-tags');
const tags = layer ? [...layer.children].filter(e => e.tagName !== 'CANVAS' && vis(e)).map(e => [text(e)].concat(box(e))) : [];
const fixed = ['mcview-badge', 'mcview-status', 'mcview-bar'].map(c => v.querySelector('.' + c)).filter(vis).map(e => [e.className].concat(box(e)));
const inputs = [...v.querySelectorAll('[data-mcview-input]')].filter(vis).map(e => [e.getAttribute('data-mcview-input'), text(e)]);
const log = (window.__mcLog || []).filter(l => /gpu error|panic|^uncaught|^unhandled|^error/i.test(l) && !/Intl\.DurationFormat|this\._formatter\.format/.test(l)).slice(0, 5);
return {rect: [r.x, r.y, r.width, r.height], dpr: devicePixelRatio, iw: innerWidth, cls: v.className, state: v.getAttribute('data-mcview-state') || '', title: v.getAttribute('title') || '', status: text(v.querySelector('.mcview-status')), badge: text(v.querySelector('.mcview-badge')), tags: tags, fixed: fixed, inputs: inputs, log: log};"#;

/// Presses every input control once (a bar button, else its tag; the door's lever), one after
/// the other: a pointer press and release then a click, and watches the state for 2.5 s: how
/// many different input levels, active cells and door states it went through.
const PRESS: &str = r#"const v = document.querySelector('[data-mcview="full"]');
const els = [...v.querySelectorAll('[data-mcview-input]')].filter(e => getComputedStyle(e).display !== 'none' && e.getBoundingClientRect().width > 0);
const seen = {}, list = [];
for (const e of els) { const k = e.getAttribute('data-mcview-input'); if (!(k in seen)) { seen[k] = 1; list.push(e); } }
const C = window.__qaC = {done: false, res: []};
const st = () => v.getAttribute('data-mcview-state') || '';
const field = (s, k) => { const m = new RegExp('(?:^| )' + k + '=(\\S*)').exec(s); return m ? m[1] : ''; };
let i = 0;
function next() {
  if (i >= list.length) { C.done = true; return; }
  const e = list[i++], k = e.getAttribute('data-mcview-input');
  const s0 = st(), a = new Set([field(s0, 'inputs')]), b = new Set([field(s0, 'active')]), d = new Set([field(s0, 'door')]), f = new Set([field(s0, 'frames')]);
  const t0 = performance.now(), o = {bubbles: true, cancelable: true, pointerId: 7, isPrimary: true, pointerType: 'touch', button: 0, buttons: 1};
  try { e.dispatchEvent(new PointerEvent('pointerdown', o)); } catch (x) {}
  setTimeout(() => { try { e.dispatchEvent(new PointerEvent('pointerup', Object.assign({}, o, {buttons: 0}))); } catch (x) {} e.click(); }, 120);
  (function w() {
    const s = st(); a.add(field(s, 'inputs')); b.add(field(s, 'active')); d.add(field(s, 'door')); f.add(field(s, 'frames'));
    if (performance.now() - t0 < 2500) requestAnimationFrame(w);
    else { C.res.push([k, (e.innerText || e.textContent || '').replace(/\s+/g, ' ').trim(), a.size, b.size, d.size, f.size]); setTimeout(next, 300); }
  })();
}
next();
return list.length + 1;"#;

/// One page's findings.
struct PageRun {
    page: String,
    ready: String,
    probes: Vec<Value>,
    shots: Vec<Option<(u32, u32, Vec<u8>)>>,
    ticks: Value,
    presses: Value,
}

/// The pages of every kind the site publishes (`--kinds`), from its `data/site.json`: each
/// circuit kind (its size number aside: counter N, burst N...) with up to four designs (all of
/// them, else its smallest, middle and largest by the kind's value), every door, and the first
/// clock of each of the `clocks` most common mechanisms (block sets).
pub fn kind_pages(dir: &Path, clocks: usize) -> Result<Vec<String>> {
    let site: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json")).context("reading data/site.json")?)?;
    let builds = site["builds"].as_array().context("no builds in site.json")?;
    let mut kinds: std::collections::BTreeMap<String, Vec<(f64, f64, String)>> = Default::default();
    let mut pages = Vec::new();
    for b in builds {
        let page = b["page"].as_str().unwrap_or("").to_string();
        match b["kind"].as_str() {
            Some("circuit") => {
                let c = &b["circuit"];
                let kind = c["kind"].as_str().unwrap_or("");
                let base = match kind.rsplit_once(' ') {
                    Some((head, n)) if n.chars().all(|ch| ch.is_ascii_digit()) => head,
                    _ => kind,
                };
                let key = format!("{}/{base}", c["study"].as_str().unwrap_or(""));
                kinds.entry(key).or_default().push((c["value"].as_f64().unwrap_or(0.0), c["rank"].as_f64().unwrap_or(0.0), page));
            }
            Some("door") => pages.push(page),
            _ => {}
        }
    }
    let mut circuits = Vec::new();
    for (_, mut v) in kinds {
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        if v.len() <= 4 {
            circuits.extend(v.into_iter().map(|x| x.2));
        } else {
            for i in [0, v.len() / 2, v.len() - 1] {
                circuits.push(v[i].2.clone());
            }
        }
    }
    let mut groups: std::collections::HashMap<String, Vec<(f64, String)>> = Default::default();
    for b in builds.iter().filter(|b| b["kind"].as_str() == Some("clock")) {
        groups.entry(b["group"].as_str().unwrap_or("").to_string()).or_default().push((b["blocks"].as_f64().unwrap_or(0.0), b["page"].as_str().unwrap_or("").to_string()));
    }
    let mut groups: Vec<(String, Vec<(f64, String)>)> = groups.into_iter().collect();
    groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
    let mut sample = Vec::new();
    for (_, mut v) in groups.into_iter().take(clocks) {
        v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        sample.push(v[0].1.clone());
    }
    Ok(circuits.into_iter().chain(pages).chain(sample).collect())
}

/// Each page's steps for `run_steps`: open it (debug state on), wait for its viewer, follow its
/// ticks, probe it, a screenshot and a probe at each of `at` (s after ready), its ticks after
/// `watch` s, then every control pressed.
fn steps_of(page: &str, at: &[f64], watch: f64) -> (Vec<(String, u64)>, usize) {
    let url = if page.contains('?') { format!("{page}&mcview-debug") } else { format!("{page}?mcview-debug") };
    let mut s = vec![(format!("@url {url}"), 0), (format!("@until {READY}"), 300), (WATCH.to_string(), 0), (PROBE.to_string(), 0)];
    let mut t = 0.0;
    for &a in at {
        s.push(("@shot".to_string(), ((a - t).max(0.0) * 1000.0) as u64));
        s.push((PROBE.to_string(), 0));
        t = a;
    }
    s.push(("return window.__qaT || null;".to_string(), ((watch - t).max(0.0) * 1000.0) as u64));
    s.push((PRESS.to_string(), 0));
    s.push(("@until return window.__qaC && window.__qaC.done ? window.__qaC : false;".to_string(), 500));
    let n = s.len();
    (s, n)
}

fn decode_shot(v: &Value) -> Option<(u32, u32, Vec<u8>)> {
    use base64::Engine as _;
    let png = base64::engine::general_purpose::STANDARD.decode(v.as_str()?).ok()?;
    let (w, h, px) = site::serve::decode_png(&png).ok()?;
    let mut rgba = Vec::with_capacity(px.len() * 4);
    for p in px {
        rgba.extend_from_slice(&[p[0], p[1], p[2], 255]);
    }
    Some((w, h, rgba))
}

fn crop(shot: &(u32, u32, Vec<u8>), x: f64, y: f64, w: f64, h: f64) -> (u32, u32, Vec<u8>) {
    let (sw, sh, px) = shot;
    let (x0, y0) = (x.max(0.0) as u32, y.max(0.0) as u32);
    let (x1, y1) = (((x + w) as u32).min(*sw), ((y + h) as u32).min(*sh));
    let (cw, ch) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
    let mut out = Vec::with_capacity((cw * ch * 4) as usize);
    for yy in y0..y1 {
        let a = ((yy * sw + x0) * 4) as usize;
        out.extend_from_slice(&px[a..a + (cw * 4) as usize]);
    }
    (cw, ch, out)
}

fn rect(v: &Value) -> Option<(String, [f64; 4])> {
    let a = v.as_array()?;
    Some((a.first()?.as_str().unwrap_or("").to_string(), [a.get(1)?.as_f64()?, a.get(2)?.as_f64()?, a.get(3)?.as_f64()?, a.get(4)?.as_f64()?]))
}

fn inter(a: &[f64; 4], b: &[f64; 4]) -> f64 {
    let w = (a[0] + a[2]).min(b[0] + b[2]) - a[0].max(b[0]);
    let h = (a[1] + a[3]).min(b[1] + b[3]) - a[1].max(b[1]);
    if w > 0.0 && h > 0.0 { w * h } else { 0.0 }
}

/// What is wrong with one probe's layout: tags over tags, over the badge or the status line,
/// under the bar, outside the view.
fn layout_problems(p: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let (vw, vh) = (p["rect"][2].as_f64().unwrap_or(0.0), p["rect"][3].as_f64().unwrap_or(0.0));
    let tags: Vec<(String, [f64; 4])> = p["tags"].as_array().into_iter().flatten().filter_map(rect).collect();
    let fixed: Vec<(String, [f64; 4])> = p["fixed"].as_array().into_iter().flatten().filter_map(rect).collect();
    let short = |s: &str| s.chars().take(18).collect::<String>();
    for i in 0..tags.len() {
        for j in i + 1..tags.len() {
            if inter(&tags[i].1, &tags[j].1) > 12.0 {
                out.push(format!("tag \"{}\" over \"{}\"", short(&tags[i].0), short(&tags[j].0)));
            }
        }
        for (name, r) in &fixed {
            let what = name.trim_start_matches("mcview-");
            if what == "bar" {
                if tags[i].1[1] + tags[i].1[3] > r[1] + 6.0 && inter(&tags[i].1, r) > 12.0 {
                    out.push(format!("tag \"{}\" under the bar", short(&tags[i].0)));
                }
            } else if inter(&tags[i].1, r) > 12.0 {
                out.push(format!("tag \"{}\" over the {what}", short(&tags[i].0)));
            }
        }
        let t = &tags[i].1;
        if t[0] < -1.0 || t[1] < -1.0 || t[0] + t[2] > vw + 1.0 || t[1] + t[3] > vh + 1.0 {
            out.push(format!("tag \"{}\" cut by the view's edge", short(&tags[i].0)));
        }
    }
    out
}

/// Mean luma and the black fraction of an RGBA picture.
fn brightness(img: &(u32, u32, Vec<u8>)) -> (f64, f64) {
    let px = &img.2;
    let n = (px.len() / 4).max(1);
    let (mut sum, mut black) = (0f64, 0usize);
    for p in px.chunks_exact(4).step_by(3) {
        let l = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        sum += l;
        black += (l < 16.0) as usize;
    }
    let m = n.div_ceil(3) as f64;
    (sum / m, black as f64 / m)
}

/// `mctas web qa`: see the module.
#[allow(clippy::too_many_arguments)]
pub fn cmd_qa(dir: &Path, pages: &[String], kinds: bool, clocks: usize, out: &Path, window: &str, at: &str, watch: f64, shards: usize) -> Result<()> {
    let (w, h) = window.split_once(',').and_then(|(a, b)| Some((a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?))).context("--window w,h")?;
    let at: Vec<f64> = at.split(',').filter_map(|x| x.trim().parse().ok()).collect();
    if at.is_empty() {
        bail!("--at: the seconds of the screenshots, e.g. 2,8");
    }
    let mut list: Vec<String> = pages.to_vec();
    if kinds {
        list.extend(kind_pages(dir, clocks)?);
    }
    if list.is_empty() {
        bail!("no pages: name them, or --kinds");
    }
    std::fs::create_dir_all(out)?;
    let shards = shards.clamp(1, 6).min(list.len());
    let chunk = list.len().div_ceil(shards);
    println!("web qa: {} pages in WebKit at {w}x{h}, {shards} at a time", list.len());
    let runs: Vec<Result<Vec<PageRun>>> = std::thread::scope(|sc| {
        let handles: Vec<_> = list
            .chunks(chunk)
            .enumerate()
            .map(|(k, part)| {
                let part = part.to_vec();
                let at = at.clone();
                sc.spawn(move || -> Result<Vec<PageRun>> {
                    // staggered: each rig picks a free X display
                    std::thread::sleep(std::time::Duration::from_millis(1500 * k as u64));
                    let mut steps = Vec::new();
                    let mut lens = Vec::new();
                    for p in &part {
                        let (s, n) = steps_of(p, &at, watch);
                        steps.extend(s);
                        lens.push(n);
                    }
                    let (vals, _) = site::webkit::run_steps(dir, None, "404.html", (w, h), &steps)?;
                    let mut runs = Vec::new();
                    let mut i = 0;
                    for (p, n) in part.iter().zip(lens) {
                        let v = &vals[i..(i + n).min(vals.len())];
                        i += n;
                        let get = |k: usize| v.get(k).cloned().unwrap_or(Value::Null);
                        let mut probes = vec![get(3)];
                        let mut shots = Vec::new();
                        for k in 0..at.len() {
                            shots.push(decode_shot(&get(4 + 2 * k)));
                            probes.push(get(5 + 2 * k));
                        }
                        let b = 4 + 2 * at.len();
                        runs.push(PageRun { page: p.clone(), ready: get(1).as_str().unwrap_or("timeout").to_string(), probes, shots, ticks: get(b), presses: get(b + 2) });
                    }
                    Ok(runs)
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap_or_else(|_| Err(anyhow::anyhow!("a shard panicked")))).collect()
    });
    let mut all = Vec::new();
    for r in runs {
        all.extend(r?);
    }
    report(&all, out, &at)
}

/// The report (printed and `report.txt`), each page's crops (`<page>-<k>.png`) and the contact
/// sheet (`sheet.png`: each page's viewer at the first screenshot, its name and its findings).
fn report(runs: &[PageRun], out: &Path, at: &[f64]) -> Result<()> {
    let font = match mctas::mcfont::Font::minecraft("26.3") {
        Ok(f) => Some(f),
        Err(e) => {
            println!("web qa: no font for the sheet's labels ({e:#}; https_proxy?)");
            None
        }
    };
    let (tw, th, lab) = (300u32, 400u32, 40u32);
    let cols = 8u32;
    let rows = (runs.len() as u32).div_ceil(cols);
    let mut sheet = mctas::mcfont::Image::new(cols * tw, rows * (th + lab), [24, 24, 28, 255]);
    let mut lines = Vec::new();
    let mut bad_pages = 0;
    for (n, r) in runs.iter().enumerate() {
        let stem = r.page.trim_end_matches(".html").replace(['/', '?', '#', '&', '='], "_");
        let mut problems: Vec<String> = Vec::new();
        let first = &r.probes[0];
        if r.ready == "none" {
            problems.push("no live viewer on the page".into());
        } else if r.ready.contains("mcview-error") || first["cls"].as_str().unwrap_or("").contains("mcview-error") {
            problems.push(format!("viewer error: {}", first["title"].as_str().unwrap_or("")));
        } else if r.ready == "timeout" {
            problems.push("viewer never ready".into());
        }
        let status = r.probes.iter().rev().find_map(|p| p["status"].as_str().filter(|s| !s.is_empty())).unwrap_or("").to_string();
        if status.contains("not simulated") || status.starts_with("stopped at") {
            problems.push(format!("status: {status}"));
        }
        // the layout at every probe, each problem once
        let mut lay: Vec<String> = Vec::new();
        for p in r.probes.iter().filter(|p| !p.is_null()) {
            for x in layout_problems(p) {
                if !lay.contains(&x) {
                    lay.push(x);
                }
            }
        }
        problems.extend(lay);
        // the ticks
        let t = &r.ticks;
        let anim = if t.is_null() {
            "no ticks".to_string()
        } else {
            let (f0, l, mx) = (t["first"].as_i64().unwrap_or(-1), t["last"].as_i64().unwrap_or(-1), t["max"].as_i64().unwrap_or(0));
            let drops: Vec<String> = t["drops"].as_array().into_iter().flatten().map(|d| format!("{:.1} s ({}->{})", d[0].as_f64().unwrap_or(0.0) / 1000.0, d[1], d[2])).collect();
            let playing = t["playing"].as_i64().unwrap_or(0) == 1;
            if f0 < 0 {
                problems.push("no state (debug)".into());
                "no state".to_string()
            } else if drops.is_empty() && l == f0 {
                if playing {
                    problems.push(format!("playing but stuck at tick {l}"));
                }
                format!("{} at tick {l}", if playing { "stuck" } else { "paused" })
            } else if drops.is_empty() {
                format!("plays, tick {f0}->{l}")
            } else {
                format!("plays to tick {mx}, restarts at {}", drops.join(", "))
            }
        };
        // the controls
        let mut ctl = Vec::new();
        for p in r.presses["res"].as_array().into_iter().flatten() {
            let (k, label) = (p[0].as_str().unwrap_or(""), p[1].as_str().unwrap_or(""));
            let reacted = (2..=5).any(|i| p[i].as_i64().unwrap_or(0) > 1);
            let name = if label.is_empty() { k.to_string() } else { label.chars().take(10).collect() };
            if !reacted {
                problems.push(format!("control {name} changed nothing"));
            }
            ctl.push(format!("{name}{}", if reacted { "" } else { "?" }));
        }
        let logs: Vec<String> = r.probes.iter().flat_map(|p| p["log"].as_array().cloned().unwrap_or_default()).filter_map(|l| l.as_str().map(|s| s.chars().take(120).collect())).collect();
        if !logs.is_empty() {
            problems.push(format!("console: {}", logs[0]));
        }
        // the crops
        let mut first_crop = None;
        for (k, shot) in r.shots.iter().enumerate() {
            let (Some(shot), p) = (shot, &r.probes[k + 1]) else { continue };
            let rv = |i: usize| p["rect"][i].as_f64().unwrap_or(0.0);
            let iw = p["iw"].as_f64().unwrap_or(390.0);
            let scale = if shot.0 as f64 > iw * 1.5 { p["dpr"].as_f64().unwrap_or(1.0).max(1.0) } else { 1.0 };
            let c = crop(shot, rv(0) * scale, rv(1) * scale, rv(2) * scale, rv(3) * scale);
            if c.0 == 0 || c.1 == 0 {
                continue;
            }
            if k == 0 {
                let (luma, black) = brightness(&c);
                if luma < 25.0 || black > 0.5 {
                    problems.push(format!("dark picture (mean luma {luma:.0}, {:.0}% black)", black * 100.0));
                }
            }
            let path = out.join(format!("{stem}-{}.png", k + 1));
            mcview::render::headless::write_png(&path, c.0, c.1, &c.2).map_err(anyhow::Error::msg)?;
            if first_crop.is_none() {
                first_crop = Some(c);
            }
        }
        bad_pages += (!problems.is_empty()) as usize;
        let line = format!(
            "{} {}: {}; inputs [{}]; tags {}; {}",
            if problems.is_empty() { "ok " } else { "BAD" },
            r.page,
            anim,
            ctl.join(" "),
            first["tags"].as_array().map(|a| a.len()).unwrap_or(0),
            if problems.is_empty() { "nothing wrong found".to_string() } else { problems.join("; ") }
        );
        println!("{line}");
        lines.push(line);
        // the sheet's tile
        let (ox, oy) = ((n as u32 % cols) * tw, (n as u32 / cols) * (th + lab));
        if let Some(c) = &first_crop {
            let f = (tw as f64 / c.0 as f64).min(th as f64 / c.1 as f64);
            let (dw, dh) = (((c.0 as f64 * f) as u32).max(1), ((c.1 as f64 * f) as u32).max(1));
            let small = mctas::mcfont::Image { w: dw, h: dh, px: mcview::tools::lanczos_resize(&c.2, c.0, c.1, dw, dh) };
            sheet.draw(&small, (ox + (tw - dw) / 2) as i64, (oy + (th - dh) / 2) as i64, 1);
        }
        if let Some(font) = &font {
            let name = r.page.trim_end_matches(".html").rsplit('/').next().unwrap_or("").to_string();
            let fit = |s: &str, max: u32| -> String {
                let mut s: String = s.to_string();
                while font.width(&s, 1) > max && s.len() > 3 {
                    s.pop();
                }
                s
            };
            font.draw(&mut sheet, (ox + 4) as i64, (oy + th + 4) as i64, &fit(&name, tw - 8), 1, [235, 235, 235], true);
            let (msg, col) = if problems.is_empty() { (format!("ok · {anim}"), [140, 220, 120]) } else { (problems.join("; "), [255, 120, 110]) };
            font.draw(&mut sheet, (ox + 4) as i64, (oy + th + 16) as i64, &fit(&msg, tw - 8), 1, col, true);
            let rest: String = msg.chars().skip(fit(&msg, tw - 8).chars().count()).collect();
            if !rest.is_empty() {
                font.draw(&mut sheet, (ox + 4) as i64, (oy + th + 27) as i64, &fit(&rest, tw - 8), 1, col, true);
            }
        }
    }
    let sheet_path = out.join("sheet.png");
    mcview::render::headless::write_png(&sheet_path, sheet.w, sheet.h, &sheet.px).map_err(anyhow::Error::msg)?;
    let summary = format!("web qa: {} pages, {bad_pages} with findings; crops at {:?} s after ready, contact sheet {}", runs.len(), at, sheet_path.display());
    println!("{summary}");
    lines.push(summary);
    std::fs::write(out.join("report.txt"), lines.join("\n") + "\n")?;
    Ok(())
}
