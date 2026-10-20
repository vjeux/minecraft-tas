//! `mctas web click`: drives headless Chromium through the DevTools protocol over a pipe
//! (`--remote-debugging-pipe`: commands on fd 3, replies on fd 4, NUL-separated JSON) to click or
//! tap a page's live viewers like a person, and reads what the viewer reports in debug mode
//! (`?mcview-debug`: `data-mcview-state` on each viewer: tick, playing, a door's blocks, each
//! input's click point and state).

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

struct Browser {
    child: std::process::Child,
    to: std::fs::File,
    from: Receiver<Value>,
    next: u64,
    session: Option<String>,
    /// The page's console lines and exceptions seen so far.
    console: Vec<String>,
}

impl Browser {
    fn start(browser: &Path, window: &str, scale: f32, url: &str) -> Result<Browser> {
        let mut a = [0i32; 2];
        let mut b = [0i32; 2];
        // SAFETY: plain pipe(2) calls on local arrays.
        if unsafe { libc::pipe(a.as_mut_ptr()) } != 0 || unsafe { libc::pipe(b.as_mut_ptr()) } != 0 {
            bail!("pipe: {}", std::io::Error::last_os_error());
        }
        let (child_in, child_out) = (a[0], b[1]);
        let mut cmd = std::process::Command::new(browser);
        cmd.args(["--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--remote-debugging-pipe"])
            .arg(format!("--window-size={window}"))
            .arg(format!("--force-device-scale-factor={scale}"));
        // the page may fetch the game's textures from GitHub: through the box's proxy
        if let Some(p) = std::env::var_os("https_proxy").or_else(|| std::env::var_os("HTTPS_PROXY")) {
            cmd.arg(format!("--proxy-server={}", p.to_string_lossy())).arg("--proxy-bypass-list=127.0.0.1;localhost");
        }
        cmd.arg(url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        // SAFETY: only async-signal-safe dup2 calls between fork and exec.
        unsafe {
            cmd.pre_exec(move || {
                if libc::dup2(child_in, 3) < 0 || libc::dup2(child_out, 4) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = cmd.spawn().with_context(|| format!("starting {}", browser.display()))?;
        // SAFETY: the child has its copies; these fds are ours to close or own.
        unsafe {
            libc::close(child_in);
            libc::close(child_out);
        }
        let to = unsafe { std::fs::File::from_raw_fd(a[1]) };
        let mut from_pipe = unsafe { std::fs::File::from_raw_fd(b[0]) };
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let mut chunk = [0u8; 65536];
            while let Ok(n) = from_pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                while let Some(i) = buf.iter().position(|c| *c == 0) {
                    let msg: Vec<u8> = buf.drain(..=i).collect();
                    if let Ok(v) = serde_json::from_slice::<Value>(&msg[..msg.len() - 1])
                        && tx.send(v).is_err()
                    {
                        return;
                    }
                }
            }
        });
        Ok(Browser { child, to, from: rx, next: 1, session: None, console: Vec::new() })
    }

    /// Send a command (to the attached page when there is one) and wait for its reply.
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next;
        self.next += 1;
        let mut msg = json!({ "id": id, "method": method, "params": params });
        if let Some(s) = &self.session
            && !method.starts_with("Target.")
        {
            msg["sessionId"] = json!(s);
        }
        let mut bytes = serde_json::to_vec(&msg)?;
        bytes.push(0);
        self.to.write_all(&bytes)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let v = self.from.recv_timeout(left).with_context(|| format!("no reply to {method}"))?;
            if v["id"].as_u64() == Some(id) {
                if let Some(e) = v.get("error") {
                    bail!("{method}: {e}");
                }
                return Ok(v["result"].clone());
            }
            self.event(&v);
        }
    }

    /// Keep the page's console lines and exceptions (events met while waiting for replies).
    fn event(&mut self, v: &Value) {
        match v["method"].as_str() {
            Some("Runtime.consoleAPICalled") => {
                let args: Vec<String> = v["params"]["args"].as_array().map(|a| a.iter().map(|x| x["value"].as_str().map(str::to_string).unwrap_or_else(|| x["description"].as_str().unwrap_or("?").to_string())).collect()).unwrap_or_default();
                self.console.push(format!("{}: {}", v["params"]["type"].as_str().unwrap_or("log"), args.join(" ")));
            }
            Some("Runtime.exceptionThrown") => {
                self.console.push(format!("exception: {}", v["params"]["exceptionDetails"]["exception"]["description"].as_str().unwrap_or("?")));
            }
            _ => {}
        }
    }

    /// Take the events that arrived (console lines), waiting up to `wait`.
    fn drain(&mut self, wait: Duration) {
        let deadline = Instant::now() + wait;
        while let Ok(v) = self.from.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            self.event(&v);
        }
    }

    /// Attach to the page tab (the first `page` target).
    fn attach_page(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let t = self.call("Target.getTargets", json!({}))?;
            if let Some(page) = t["targetInfos"].as_array().and_then(|a| a.iter().find(|i| i["type"] == "page")) {
                let id = page["targetId"].as_str().unwrap_or_default().to_string();
                let r = self.call("Target.attachToTarget", json!({ "targetId": id, "flatten": true }))?;
                self.session = r["sessionId"].as_str().map(str::to_string);
                return Ok(());
            }
            if Instant::now() > deadline {
                bail!("no page target");
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn eval(&mut self, expr: &str) -> Result<Value> {
        let r = self.call("Runtime.evaluate", json!({ "expression": expr, "returnByValue": true }))?;
        Ok(r["result"]["value"].clone())
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A viewer's `data-mcview-state`: `tick=T playing=0|1 door=N|- inputs=x,y,on|...`.
#[derive(Debug, Clone, PartialEq)]
struct State {
    tick: u32,
    playing: bool,
    door: Option<u32>,
    inputs: Vec<(Option<(f64, f64)>, bool)>,
    active: Option<u32>,
    /// The camera: yaw, pitch, distance, target x, y, z.
    cam: Vec<f64>,
}

fn parse_state(s: &str) -> Option<State> {
    if !s.contains("tick=") {
        return None;
    }
    let mut st = State { tick: 0, playing: false, door: None, inputs: Vec::new(), active: None, cam: Vec::new() };
    for w in s.split_whitespace() {
        let (k, v) = w.split_once('=')?;
        match k {
            "tick" => st.tick = v.parse().ok()?,
            "playing" => st.playing = v == "1",
            "door" => st.door = v.parse().ok(),
            "active" => st.active = v.parse().ok(),
            "cam" => st.cam = v.split(',').filter_map(|x| x.parse().ok()).collect(),
            "inputs" => {
                for i in v.split('|').filter(|i| !i.is_empty()) {
                    let p: Vec<&str> = i.split(',').collect();
                    if p.len() != 3 {
                        continue;
                    }
                    let xy = match (p[0].parse::<f64>(), p[1].parse::<f64>()) {
                        (Ok(x), Ok(y)) => Some((x, y)),
                        _ => None,
                    };
                    st.inputs.push((xy, p[2] == "1"));
                }
            }
            _ => {}
        }
    }
    Some(st)
}

fn states(b: &mut Browser) -> Result<Vec<Option<State>>> {
    let v = b.eval("Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb').map(e => e.getAttribute('data-mcview-state') || '')")?;
    Ok(v.as_array().map(|a| a.iter().map(|s| s.as_str().and_then(parse_state)).collect()).unwrap_or_default())
}

/// A screenshot of a page after `seconds` of real time (animation frames run: playing viewers
/// move, demos advance), scrolled to `scroll_y` CSS px.
pub fn cmd_snap(url: &str, window: &str, scale: f32, seconds: f64, scroll_y: f64, eval: Option<&str>, out: &Path, browser: &Path) -> Result<()> {
    let mut b = Browser::start(browser, window, scale, url)?;
    b.attach_page()?;
    b.call("Page.enable", json!({}))?;
    b.call("Runtime.enable", json!({}))?;
    std::thread::sleep(Duration::from_millis(500));
    if scroll_y > 0.0 {
        b.eval(&format!("window.scrollTo(0, {scroll_y})"))?;
    }
    b.drain(Duration::from_secs_f64(seconds));
    let r = b.call("Page.captureScreenshot", json!({ "format": "png" }))?;
    let data = r["data"].as_str().context("no screenshot data")?;
    std::fs::write(out, base64_decode(data)?)?;
    for l in &b.console {
        println!("console {l}");
    }
    if let Some(js) = eval {
        println!("eval {js} = {}", b.eval(js)?);
    }
    println!("{} ({seconds} s of {url})", out.display());
    Ok(())
}

fn base64_decode(s: &str) -> Result<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => (c - b'A') as u32,
            b'a'..=b'z' => (c - b'a' + 26) as u32,
            b'0'..=b'9' => (c - b'0' + 52) as u32,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        })
    };
    let bytes: Vec<u8> = s.bytes().filter(|c| *c != b'=' && !c.is_ascii_whitespace()).collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n = 0u32;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c).context("bad base64")? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

/// The mean luma (0-255) of an RGBA image and the share of its pixels darker than 16.
pub fn luma_stats(rgba: &[u8]) -> (f64, f64) {
    let n = (rgba.len() / 4).max(1) as f64;
    let (mut sum, mut dark) = (0f64, 0f64);
    for p in rgba.chunks(4) {
        let l = 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64;
        sum += l;
        if l < 16.0 {
            dark += 1.0;
        }
    }
    (sum / n, dark / n)
}

/// Every viewer of a page (thumbnails and live viewers) in headless Chromium through the real
/// bundle: each scrolled into view once drawn, its on-screen pixels measured; fails when one is
/// mostly black (mean luma under `min` or over half its pixels near black): the WebGL failures
/// the native renderer cannot see.
pub fn cmd_luma(url: &str, window: &str, min: f64, browser: &Path) -> Result<()> {
    let mut b = Browser::start(browser, window, 1.0, url)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    b.call("Page.enable", json!({}))?;
    let count = |b: &mut Browser, sel: &str| -> Result<u64> { Ok(b.eval(&format!("document.querySelectorAll('{sel}').length"))?.as_u64().unwrap_or(0)) };
    // scroll through the page until every viewer has drawn (thumbnails draw near the viewport)
    let deadline = Instant::now() + Duration::from_secs(240);
    let total = loop {
        let total = count(&mut b, "[data-mcview]")?;
        if total > 0 {
            break total;
        }
        if Instant::now() > deadline {
            bail!("no viewer on the page");
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    loop {
        let height = b.eval("document.documentElement.scrollHeight")?.as_f64().unwrap_or(0.0);
        let view = b.eval("window.innerHeight")?.as_f64().unwrap_or(800.0).max(200.0);
        let mut y = 0.0;
        while y < height {
            b.eval(&format!("window.scrollTo(0, {y})"))?;
            b.drain(Duration::from_millis(120));
            y += view * 0.8;
        }
        let done = count(&mut b, "[data-mcview].mcview-ready, [data-mcview].mcview-error")?;
        if done >= total {
            break;
        }
        if Instant::now() > deadline {
            println!("web luma: only {done} of {total} viewers drew within 240 s");
            break;
        }
    }
    let (mut failed, mut worst) = (Vec::new(), (f64::MAX, String::new()));
    for i in 0..total {
        let r = b.eval(&format!(
            "(() => {{ const v = document.querySelectorAll('[data-mcview]')[{i}]; v.scrollIntoView({{block: 'center'}}); const r = v.getBoundingClientRect(); return [r.left + window.scrollX, r.top + window.scrollY, r.width, r.height, v.getAttribute('data-src') || '', v.className]; }})()"
        ))?;
        let a = r.as_array().cloned().unwrap_or_default();
        let num = |k: usize| a.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let (x, y, w, h) = (num(0), num(1), num(2), num(3));
        let src = a.get(4).and_then(Value::as_str).unwrap_or("").to_string();
        let class = a.get(5).and_then(Value::as_str).unwrap_or("").to_string();
        if w < 8.0 || h < 8.0 {
            continue;
        }
        b.drain(Duration::from_millis(80));
        let shot = b.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": x + 2.0, "y": y + 2.0, "width": w - 4.0, "height": h - 4.0, "scale": 0.5 } }))?;
        let png = base64_decode(shot["data"].as_str().unwrap_or(""))?;
        let (rgba, _, _) = mcview::tools::decode_png(&png).map_err(anyhow::Error::msg)?;
        let (mean, dark) = luma_stats(&rgba);
        if mean < worst.0 {
            worst = (mean, src.clone());
        }
        if mean < min || dark > 0.5 || class.contains("mcview-error") {
            if failed.len() < 3 {
                // the first dark ones, to look at
                let _ = std::fs::write(format!("/tmp/mctas-luma-dark-{}.png", failed.len()), &png);
            }
            failed.push(format!("{src}: mean luma {mean:.0}, {:.0}% near black{}", dark * 100.0, if class.contains("mcview-error") { ", mcview-error" } else { "" }));
        }
    }
    for f in &failed {
        println!("DARK {f}");
    }
    for l in b.console.iter().filter(|l| l.starts_with("error") || l.starts_with("exception") || l.contains("GPU error") || l.contains("panic")) {
        println!("console {l}");
    }
    println!("web luma: {total} viewers, {} dark (darkest {:.0}: {})", failed.len(), worst.0, worst.1);
    if !failed.is_empty() {
        bail!("{} viewers are mostly black", failed.len());
    }
    Ok(())
}

/// Press and hold each live viewer's first circuit input in the 3D view (mouse or touch): the
/// input must turn on while held (after the hold delay), stay on, turn off once released, and
/// the camera must not move at any point.
pub fn cmd_hold(url: &str, window: &str, scale: f32, touch: bool, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, scale, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    if touch {
        b.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": true, "maxTouchPoints": 5 }))?;
    }
    let deadline = Instant::now() + Duration::from_secs(60);
    let initial = loop {
        let s = states(&mut b)?;
        if !s.is_empty() && s.iter().all(|x| x.is_some()) {
            break s.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>();
        }
        if Instant::now() > deadline {
            bail!("the live viewers never reported (served? ?mcview-debug?)");
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let (mut tested, mut failed) = (0, 0);
    for (i, st) in initial.iter().enumerate() {
        // circuits only: a door's lever flips on a tap, it has no hold
        if st.door.is_some() {
            continue;
        }
        let Some(((fx, fy), _)) = st.inputs.iter().find_map(|(xy, on)| xy.map(|p| (p, *on))) else { continue };
        let rect = b.eval(&format!(
            "(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}]; v.scrollIntoView({{block: 'center'}}); const r = v.querySelector('canvas').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; }})()"
        ))?;
        let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if r.len() != 4 {
            continue;
        }
        std::thread::sleep(Duration::from_millis(300));
        let Some(Some(before)) = states(&mut b)?.get(i).cloned() else { continue };
        let Some(((fx, fy), on0)) = before.inputs.iter().find_map(|(xy, on)| xy.map(|p| (p, *on))).or(Some(((fx, fy), false))) else { continue };
        let (x, y) = (r[0] + fx * r[2], r[1] + fy * r[3]);
        tested += 1;
        // press, with a little jitter (a finger), and hold
        if touch {
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchStart", "touchPoints": [{ "x": x, "y": y }] }))?;
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchMove", "touchPoints": [{ "x": x + 2.0, "y": y + 1.0 }] }))?;
        } else {
            b.call("Input.dispatchMouseEvent", json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1 }))?;
            b.call("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x + 2.0, "y": y + 1.0, "button": "left" }))?;
        }
        std::thread::sleep(Duration::from_millis(700));
        let held = states(&mut b)?.get(i).cloned().flatten();
        std::thread::sleep(Duration::from_millis(500));
        let still = states(&mut b)?.get(i).cloned().flatten();
        if touch {
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchEnd", "touchPoints": [] }))?;
        } else {
            b.call("Input.dispatchMouseEvent", json!({ "type": "mouseReleased", "x": x + 2.0, "y": y + 1.0, "button": "left", "clickCount": 1 }))?;
        }
        std::thread::sleep(Duration::from_millis(600));
        let after = states(&mut b)?.get(i).cloned().flatten();
        let input_on = |s: &Option<State>| s.as_ref().and_then(|s| s.inputs.iter().find_map(|(xy, on)| xy.map(|_| *on)));
        let same_cam = |s: &Option<State>| s.as_ref().is_some_and(|s| s.cam.len() == 6 && s.cam.iter().zip(&before.cam).all(|(a, b)| (a - b).abs() < 1e-3));
        let ok = !on0 && input_on(&held) == Some(true) && input_on(&still) == Some(true) && input_on(&after) == Some(false) && same_cam(&held) && same_cam(&still) && same_cam(&after);
        let how = if touch { "touch" } else { "mouse" };
        if ok {
            println!("viewer {i}: {how} hold at ({x:.0}, {y:.0}): input on while held, off after release, camera unchanged: PASS");
        } else {
            failed += 1;
            println!(
                "viewer {i}: {how} hold at ({x:.0}, {y:.0}): FAIL (input before {on0}, held {:?}, still {:?}, after {:?}; camera before {:?}, after {:?})",
                input_on(&held),
                input_on(&still),
                input_on(&after),
                before.cam,
                after.as_ref().map(|s| s.cam.clone())
            );
        }
    }
    println!("web click --hold: {} of {tested} viewers hold their input while pressed ({window})", tested - failed);
    if tested == 0 {
        bail!("no live viewer with an input");
    }
    if failed > 0 {
        bail!("{failed} viewers failed the hold");
    }
    Ok(())
}

/// The site's in-place swap (`data-src` changed on a live viewer, `data-keep-camera` set): the
/// person orbits, the design is swapped, and the new design must show (not black, the viewer
/// marked `mcview-live-src`) under the very same camera.
pub fn cmd_swap(url: &str, to: &str, window: &str, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, 1.0, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    let st0 = loop {
        if let Some(Some(s)) = states(&mut b)?.first().cloned() {
            break s;
        }
        if Instant::now() > deadline {
            bail!("no live viewer reported");
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let rect = b.eval("(() => { const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0]; v.scrollIntoView({block: 'center'}); const r = v.querySelector('canvas').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; })()")?;
    let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if r.len() != 4 {
        bail!("no canvas");
    }
    // the person orbits away from the default camera (a drag on the sky corner, not an input)
    let (x, y) = (r[0] + r[2] * 0.1, r[1] + r[3] * 0.1);
    b.call("Input.dispatchMouseEvent", json!({ "type": "mousePressed", "x": x, "y": y, "button": "left", "clickCount": 1 }))?;
    for k in 1..=6 {
        b.call("Input.dispatchMouseEvent", json!({ "type": "mouseMoved", "x": x + 15.0 * k as f64, "y": y + 4.0 * k as f64, "button": "left" }))?;
    }
    b.call("Input.dispatchMouseEvent", json!({ "type": "mouseReleased", "x": x + 90.0, "y": y + 24.0, "button": "left", "clickCount": 1 }))?;
    std::thread::sleep(Duration::from_millis(400));
    let st1 = states(&mut b)?.first().cloned().flatten().context("state after the orbit")?;
    if st1.cam == st0.cam {
        bail!("the drag did not orbit the camera ({:?})", st1.cam);
    }
    let marked = b.eval("document.querySelectorAll('.mcview-live-src').length")?.as_u64().unwrap_or(0);
    b.eval(&format!(
        "(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0]; v.setAttribute('data-keep-camera', '1'); v.setAttribute('data-src', {}); return 1; }})()",
        serde_json::to_string(to)?
    ))?;
    std::thread::sleep(Duration::from_millis(2500));
    let st2 = states(&mut b)?.first().cloned().flatten().context("state after the swap")?;
    let same_cam = st2.cam.len() == 6 && st2.cam.iter().zip(&st1.cam).all(|(a, b)| (a - b).abs() < 1e-3);
    // the viewer's pixels: the new design drawn, not black
    let shot = b.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": r[0] + 2.0, "y": r[1] + 2.0, "width": r[2] - 4.0, "height": r[3] - 4.0, "scale": 0.5 } }))?;
    let (rgba, _, _) = mcview::tools::decode_png(&base64_decode(shot["data"].as_str().unwrap_or(""))?).map_err(anyhow::Error::msg)?;
    let (luma, dark) = luma_stats(&rgba);
    let changed = st2.inputs.len() != st1.inputs.len() || st2.tick < st1.tick.max(1) || st2.active != st1.active;
    println!(
        "swap to {to}: marked mcview-live-src {}, camera {} ({:?} -> {:?}), design changed {changed}, luma {luma:.0} ({:.0}% near black)",
        marked > 0,
        if same_cam { "kept" } else { "MOVED" },
        st1.cam,
        st2.cam,
        dark * 100.0
    );
    if marked == 0 || !same_cam || !changed || luma < 40.0 || dark > 0.5 {
        bail!("the in-place swap failed");
    }
    println!("web swap: PASS");
    Ok(())
}

/// A live viewer's WebGL context lost and restored (WEBGL_lose_context, what iOS does to the
/// oldest contexts): it must draw again (not black) once the canvas is restored.
pub fn cmd_lose(url: &str, window: &str, browser: &Path) -> Result<()> {
    let mut b = Browser::start(browser, window, 1.0, url)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if b.eval("document.querySelectorAll('[data-mcview].mcview-ready').length")?.as_u64().unwrap_or(0) > 0 {
            break;
        }
        if Instant::now() > deadline {
            bail!("no viewer drew");
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    std::thread::sleep(Duration::from_millis(800));
    let rect = b.eval("(() => { const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0]; v.scrollIntoView({block: 'center'}); const r = v.getBoundingClientRect(); return [r.left + window.scrollX, r.top + window.scrollY, r.width, r.height]; })()")?;
    let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if r.len() != 4 {
        bail!("no live viewer");
    }
    let mut measure = |b: &mut Browser| -> Result<f64> {
        b.drain(Duration::from_millis(300));
        let shot = b.call("Page.captureScreenshot", json!({ "format": "png", "clip": { "x": r[0] + 2.0, "y": r[1] + 2.0, "width": r[2] - 4.0, "height": r[3] - 4.0, "scale": 0.5 } }))?;
        let (rgba, _, _) = mcview::tools::decode_png(&base64_decode(shot["data"].as_str().unwrap_or(""))?).map_err(anyhow::Error::msg)?;
        Ok(luma_stats(&rgba).0)
    };
    let before = measure(&mut b)?;
    b.eval("(() => { const c = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[0].querySelector('canvas'); window.__lose = c.getContext('webgl2').getExtension('WEBGL_lose_context'); window.__lose.loseContext(); return 1; })()")?;
    std::thread::sleep(Duration::from_millis(500));
    b.eval("window.__lose.restoreContext(), 1")?;
    std::thread::sleep(Duration::from_millis(1500));
    // a nudge (a wheel) so a paused viewer draws a frame
    let sy = b.eval("window.scrollY")?.as_f64().unwrap_or(0.0);
    b.call("Input.dispatchMouseEvent", json!({ "type": "mouseWheel", "x": r[0] + r[2] / 2.0, "y": r[1] + r[3] / 2.0 - sy, "deltaX": 0, "deltaY": 1 }))?;
    let after = measure(&mut b)?;
    for l in b.console.iter().filter(|l| l.contains("lost") || l.contains("error") || l.contains("panic")) {
        println!("console {l}");
    }
    println!("web lose: luma {before:.0} before the context loss, {after:.0} after it came back");
    if before < 40.0 || after < 40.0 {
        bail!("the viewer did not draw again after its context came back");
    }
    println!("web lose: PASS");
    Ok(())
}

pub fn cmd_click(url: &str, window: &str, scale: f32, touch: bool, wait: u64, browser: &Path) -> Result<()> {
    let page = if url.contains('?') { format!("{url}&mcview-debug") } else { format!("{url}?mcview-debug") };
    let mut b = Browser::start(browser, window, scale, &page)?;
    b.attach_page()?;
    b.call("Runtime.enable", json!({}))?;
    if touch {
        b.call("Emulation.setTouchEmulationEnabled", json!({ "enabled": true, "maxTouchPoints": 5 }))?;
    }
    // wait for every live viewer to report its state
    let deadline = Instant::now() + Duration::from_secs(60);
    let initial = loop {
        let s = states(&mut b)?;
        if !s.is_empty() && s.iter().all(|x| x.is_some()) {
            break s.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>();
        }
        if Instant::now() > deadline {
            bail!("the live viewers never reported (served? ?mcview-debug? {} viewers)", s.len());
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    let (mut tested, mut failed) = (0, 0);
    for (i, st) in initial.iter().enumerate() {
        let Some(((fx, fy), on)) = st.inputs.iter().find_map(|(xy, on)| xy.map(|p| (p, *on))) else {
            println!("viewer {i}: no clickable input, skipped ({st:?})");
            continue;
        };
        tested += 1;
        if st.playing {
            println!("viewer {i}: already playing before the click (expected paused)");
        }
        // bring the viewer into view, then click where its input is on its canvas now
        let rect = b.eval(&format!(
            "(() => {{ const v = Array.from(document.querySelectorAll('[data-mcview]')).filter(e => (e.getAttribute('data-mcview') || '') !== 'thumb')[{i}]; v.scrollIntoView({{block: 'center'}}); const r = v.querySelector('canvas').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; }})()"
        ))?;
        let r: Vec<f64> = rect.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        if r.len() != 4 {
            bail!("viewer {i}: no canvas");
        }
        let (x, y) = (r[0] + fx * r[2], r[1] + fy * r[3]);
        let hit = b.eval(&format!("(() => {{ const e = document.elementFromPoint({x}, {y}); return e ? e.tagName + '.' + e.className : 'nothing'; }})()"))?;
        if hit.as_str() != Some("CANVAS.") {
            println!("viewer {i}: the point ({x:.0}, {y:.0}) is covered by {hit}");
        }
        if touch {
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchStart", "touchPoints": [{ "x": x, "y": y }] }))?;
            b.call("Input.dispatchTouchEvent", json!({ "type": "touchEnd", "touchPoints": [] }))?;
        } else {
            for kind in ["mousePressed", "mouseReleased"] {
                b.call("Input.dispatchMouseEvent", json!({ "type": kind, "x": x, "y": y, "button": "left", "clickCount": 1 }))?;
            }
        }
        // the input flips at once, the simulation starts, the build reacts
        let deadline = Instant::now() + Duration::from_secs(wait);
        let mut last = st.clone();
        let (mut saw_flip, mut saw_react) = (false, false);
        let verdict = loop {
            if let Some(Some(now)) = states(&mut b)?.get(i).cloned() {
                // a pulse input is on for 2 gt only, and a reaction can be brief: seen once is enough
                saw_flip |= now.inputs.iter().find_map(|(xy, on)| xy.map(|_| *on)) == Some(!on);
                saw_react |= match (st.door, now.door) {
                    (Some(a), Some(c)) => a != c,
                    _ => now.tick > st.tick + 1 && (now.active != st.active || now.active.is_none()),
                };
                last = now.clone();
                if saw_flip && now.playing && saw_react {
                    break Ok(now);
                }
            }
            if Instant::now() > deadline {
                break Err(last.clone());
            }
            std::thread::sleep(Duration::from_millis(40));
        };
        let how = if touch { "tap" } else { "click" };
        match verdict {
            Ok(now) => println!("viewer {i}: {how} at ({x:.0}, {y:.0}): input {} -> {}, playing, tick {} -> {}, door blocks {:?} -> {:?}: PASS", on as u8, !on as u8, st.tick, now.tick, st.door, now.door),
            Err(now) => {
                failed += 1;
                println!("viewer {i}: {how} at ({x:.0}, {y:.0}): FAIL (before {st:?}, after {now:?})");
            }
        }
    }
    if tested == 0 {
        bail!("no viewer had a clickable input");
    }
    if failed > 0 {
        bail!("{failed} of {tested} viewers did not react to a {}", if touch { "tap" } else { "click" });
    }
    println!("web click: {tested} viewers react to a {} on their input while paused ({window})", if touch { "tap" } else { "click" });
    Ok(())
}
