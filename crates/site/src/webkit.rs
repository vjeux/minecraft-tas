//! The WebKit smoke (`mctas site smoke --engine webkit`): the pages that matter rendered by
//! WebKitGTK (the engine family of iOS Safari) through WebKitWebDriver (W3C WebDriver over HTTP)
//! in an Xvfb display, on a desktop window and on a phone-sized window (iPhone user agent, device
//! scale 2): every 3D view must become ready and draw a real picture (enough colors, not black).
//!
//! Needs WebKitGTK's WebKitWebDriver and MiniBrowser (CentOS: `dnf install webkit2gtk3
//! webkit2gtk3-devel`) and Xvfb (`xorg-x11-server-Xvfb`).

use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const MINIBROWSER: &str = "/usr/libexec/webkit2gtk-4.0/MiniBrowser";
const IPHONE_UA: &str = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1";

/// A child process killed when dropped.
struct Proc(Child);

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Xvfb, stopped with SIGTERM when dropped (so it removes its lock file and socket).
struct Xvfb(Child);

impl Drop for Xvfb {
    fn drop(&mut self) {
        let _ = Command::new("kill").args(["-TERM", &self.0.id().to_string()]).status();
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(5) {
            if let Ok(Some(_)) = self.0.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A display number in :90..:99 that no live X server holds (a stale lock of a dead server is
/// removed with its socket).
fn free_display() -> Option<u32> {
    (90..100).find(|n| {
        let lock = PathBuf::from(format!("/tmp/.X{n}-lock"));
        let sock = PathBuf::from(format!("/tmp/.X11-unix/X{n}"));
        match std::fs::read_to_string(&lock) {
            Err(_) => !sock.exists(),
            Ok(pid) => {
                let alive = Path::new(&format!("/proc/{}", pid.trim())).exists();
                if !alive {
                    let _ = std::fs::remove_file(&lock);
                    let _ = std::fs::remove_file(&sock);
                }
                !alive
            }
        }
    })
}

fn free_port() -> Result<u16> {
    Ok(TcpListener::bind(("127.0.0.1", 0))?.local_addr()?.port())
}

/// One HTTP request to the local WebDriver; the JSON body of the answer.
fn http(port: u16, method: &str, path: &str, body: Option<&serde_json::Value>) -> Result<serde_json::Value> {
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    s.set_read_timeout(Some(Duration::from_secs(120)))?;
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n\r\n{payload}",
        payload.len()
    );
    s.write_all(req.as_bytes())?;
    let mut all = Vec::new();
    s.read_to_end(&mut all)?;
    let head_end = all.windows(4).position(|w| w == b"\r\n\r\n").context("no HTTP header from WebKitWebDriver")?;
    let head = String::from_utf8_lossy(&all[..head_end]).to_lowercase();
    let mut body = all[head_end + 4..].to_vec();
    if head.contains("transfer-encoding: chunked") {
        let mut out = Vec::new();
        let mut rest = &body[..];
        while let Some(i) = rest.windows(2).position(|w| w == b"\r\n") {
            let n = usize::from_str_radix(String::from_utf8_lossy(&rest[..i]).trim(), 16).unwrap_or(0);
            if n == 0 {
                break;
            }
            let start = i + 2;
            out.extend_from_slice(&rest[start..(start + n).min(rest.len())]);
            rest = &rest[(start + n + 2).min(rest.len())..];
        }
        body = out;
    }
    let v: serde_json::Value = serde_json::from_slice(&body).with_context(|| format!("{method} {path}: {}", String::from_utf8_lossy(&body).chars().take(300).collect::<String>()))?;
    if let Some(e) = v["value"]["error"].as_str() {
        bail!("{method} {path}: {e}: {}", v["value"]["message"].as_str().unwrap_or(""));
    }
    Ok(v)
}

struct Session {
    port: u16,
    id: String,
    _driver: Proc,
}

impl Session {
    fn start(display: &str, phone: bool) -> Result<Session> {
        let port = free_port()?;
        let mut cmd = Command::new("WebKitWebDriver");
        cmd.arg(format!("--port={port}")).env("DISPLAY", display).stdout(Stdio::null()).stderr(Stdio::null());
        if phone {
            cmd.env("GDK_SCALE", "2");
        }
        let driver = Proc(cmd.spawn().context("running WebKitWebDriver (dnf install webkit2gtk3)")?);
        let t0 = Instant::now();
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if t0.elapsed() > Duration::from_secs(20) {
                bail!("WebKitWebDriver did not start");
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let mut args = vec!["--automation".to_string(), "--enable-webgl=true".to_string()];
        if phone {
            args.push(format!("--user-agent={IPHONE_UA}"));
        }
        let caps = serde_json::json!({"capabilities": {"alwaysMatch": {"webkitgtk:browserOptions": {"binary": MINIBROWSER, "args": args}}}});
        let v = http(port, "POST", "/session", Some(&caps))?;
        let id = v["value"]["sessionId"].as_str().context("no session id")?.to_string();
        let s = Session { port, id, _driver: driver };
        let (w, h) = if phone { (390, 844) } else { (1280, 900) };
        let _ = s.post("window/rect", serde_json::json!({"width": w, "height": h}));
        Ok(s)
    }

    fn post(&self, what: &str, body: serde_json::Value) -> Result<serde_json::Value> {
        http(self.port, "POST", &format!("/session/{}/{what}", self.id), Some(&body))
    }

    fn script(&self, js: &str) -> Result<serde_json::Value> {
        Ok(self.post("execute/sync", serde_json::json!({"script": js, "args": []}))?["value"].clone())
    }

    fn screenshot(&self) -> Result<(u32, u32, Vec<[u8; 3]>)> {
        let v = http(self.port, "GET", &format!("/session/{}/screenshot", self.id), None)?;
        self.decode_shot(&v)
    }

    fn decode_shot(&self, v: &serde_json::Value) -> Result<(u32, u32, Vec<[u8; 3]>)> {
        let b64 = v["value"].as_str().context("no screenshot")?;
        use base64::Engine as _;
        let png = base64::engine::general_purpose::STANDARD.decode(b64)?;
        // MCTAS_WEBKIT_SHOTS=<dir>: keep every screenshot (shot-1.png, shot-2.png...)
        if let Some(dir) = std::env::var_os("MCTAS_WEBKIT_SHOTS") {
            static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            let k = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(Path::new(&dir).join(format!("shot-{k}.png")), &png);
        }
        crate::serve::decode_png(&png)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = http(self.port, "DELETE", &format!("/session/{}", self.id), None);
    }
}

/// Colors, mean luma and the black fraction of a region of a screenshot.
fn picture_stats(shot: &(u32, u32, Vec<[u8; 3]>), x: f64, y: f64, w: f64, h: f64) -> (usize, f64, f64) {
    let (sw, sh, px) = shot;
    let (x0, y0) = (x.max(0.0) as u32, y.max(0.0) as u32);
    let (x1, y1) = (((x + w) as u32).min(*sw), ((y + h) as u32).min(*sh));
    let mut colors = std::collections::HashSet::new();
    let (mut sum, mut black, mut n) = (0f64, 0usize, 0usize);
    for yy in y0..y1 {
        for xx in x0..x1 {
            let p = px[(yy * sw + xx) as usize];
            let l = 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
            sum += l;
            if l < 16.0 {
                black += 1;
            }
            n += 1;
            colors.insert(p);
        }
    }
    if n == 0 {
        return (0, 0.0, 1.0);
    }
    (colors.len(), sum / n as f64, black as f64 / n as f64)
}

/// Check one page: its first viewer (`selector`) becomes ready and draws a picture.
fn check_page(s: &Session, base: &str, page: &str, selector: &str) -> Result<String> {
    s.post("url", serde_json::json!({"url": format!("{base}{page}")}))?;
    let t0 = Instant::now();
    let state = loop {
        let js = format!("const v = document.querySelector('{selector}'); if (!v) return 'none'; v.scrollIntoView({{block: 'center'}}); return v.className;");
        let c = s.script(&js)?.as_str().unwrap_or("").to_string();
        // the index draws its cards (and their views) only after loading the data: wait for them
        if c.contains("mcview-ready") || c.contains("mcview-error") {
            break c;
        }
        if t0.elapsed() > Duration::from_secs(90) {
            break format!("timeout ({c})");
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    if !state.contains("mcview-ready") {
        let title = s.script(&format!("const v = document.querySelector('{selector}'); return v ? (v.getAttribute('title') || '') : '';")).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        bail!("the view did not become ready: {state} {title}");
    }
    // let it draw a few frames (software GL is slow)
    std::thread::sleep(Duration::from_secs(4));
    picture(s, selector)
}

/// Poll a script until it returns true (or time out).
fn wait_true(s: &Session, js: &str, secs: u64) -> Result<bool> {
    let t0 = Instant::now();
    loop {
        if s.script(js)?.as_bool().unwrap_or(false) {
            return Ok(true);
        }
        if t0.elapsed() > Duration::from_secs(secs) {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// Click the element `css` through WebDriver (a real click: the page's own handlers run).
fn click(s: &Session, css: &str) -> Result<()> {
    let v = s.post("element", serde_json::json!({"using": "css selector", "value": css}))?;
    let id = v["value"].as_object().and_then(|o| o.values().next()).and_then(|x| x.as_str()).context("no element id")?.to_string();
    s.post(&format!("element/{id}/click"), serde_json::json!({}))?;
    Ok(())
}

/// The in-place swap of a tunable group's picker (vjeux: "Don't reload the page when clicking on
/// variants, update inline and keep the current camera"): after a click on another version the
/// viewer is the same element (no reload), its design is the version's, the URL and the title are
/// the version's, and it still draws a real picture.
fn check_picker_swap(s: &Session, base: &str, page: &str, other_page: &str, other_design: &str, other_name: &str) -> Result<String> {
    check_page(s, base, page, VIEW)?;
    let live = s.script(&format!("const v = document.querySelector('{VIEW}'); v.__probe = 'kept'; return v.classList.contains('mcview-live-src');"))?.as_bool().unwrap_or(false);
    if !live {
        bail!("the viewer does not mark itself mcview-live-src (no in-place swaps with this bundle)");
    }
    click(s, &format!("nav.picker a.pick[href$=\"{other_page}\"]"))?;
    let js = format!(
        "const v = document.querySelector('{VIEW}'); return !!v && location.pathname.endsWith('{other_page}') && (v.getAttribute('data-src') || '').endsWith('{other_design}') && document.title.startsWith('{other_name}');"
    );
    if !wait_true(s, &js, 20)? {
        let state = s.script(&format!("const v = document.querySelector('{VIEW}'); return [location.pathname, v ? v.getAttribute('data-src') : 'no viewer', document.title].join(' | ');"))?;
        bail!("no swap after the click: {state}");
    }
    let same = s.script(&format!("return document.querySelector('{VIEW}').__probe === 'kept';"))?.as_bool().unwrap_or(false);
    if !same {
        bail!("the page reloaded (the viewer element is a new one)");
    }
    std::thread::sleep(Duration::from_secs(4));
    let detail = picture(s, VIEW)?;
    Ok(format!("same viewer, URL + title + design swapped, {detail}"))
}

/// A family page's member row: the family view shows that member (same element, its design).
fn check_family_swap(s: &Session, base: &str, page: &str) -> Result<String> {
    check_page(s, base, page, "#member-view")?;
    s.script("document.querySelector('#member-view').__probe = 'kept'; return true;")?;
    let target = s.script("const a = document.querySelectorAll('a.view[data-src]'); return a.length > 1 ? [a[1].getAttribute('href'), a[1].getAttribute('data-src')] : null;")?;
    let (Some(href), Some(src)) = (target[0].as_str().map(String::from), target[1].as_str().map(String::from)) else { bail!("no second member row to click") };
    click(s, &format!("a.view[href=\"{href}\"]"))?;
    let js = format!("const v = document.querySelector('#member-view'); return v.getAttribute('data-src') === '{src}' && location.hash === '{href}' && v.__probe === 'kept';");
    let mut how = "click";
    if !wait_true(s, &js, 5)? {
        // the page must not hide the link (what is at its center?); WebKitWebDriver's synthesized
        // clicks can miss small targets at device scale 2: then a script click (the page's own
        // handler still runs)
        let hit = s.script(&format!("const a = document.querySelector('a.view[href=\"{href}\"]'); a.scrollIntoView({{block: 'center'}}); const r = a.getBoundingClientRect(); const e = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2); return !!e && (e === a || a.contains(e));"))?;
        if !hit.as_bool().unwrap_or(false) {
            bail!("the member link {href} is covered by another element");
        }
        s.script(&format!("document.querySelector('a.view[href=\"{href}\"]').click(); return true;"))?;
        how = "script click (the link is not covered)";
    }
    if !wait_true(s, &js, 10)? {
        let state = s.script("const v = document.querySelector('#member-view'); return [v ? v.getAttribute('data-src') : 'no view', v ? v.className : '', location.hash, v ? String(v.__probe) : ''].join(' | ');")?;
        bail!("the family view did not swap to {src}: {state}");
    }
    std::thread::sleep(Duration::from_secs(4));
    let detail = picture(s, "#member-view")?;
    Ok(format!("same view, member {href} shown ({how}), {detail}"))
}

/// The view `selector` must draw a real picture: enough colors, not dark, not blank (a cleared or
/// white canvas), measured on a screenshot of the window cropped to the view (what a user sees).
fn picture(s: &Session, selector: &str) -> Result<String> {
    s.script(&format!("document.querySelector('{selector}').scrollIntoView({{block: 'center', inline: 'nearest'}}); return true;"))?;
    std::thread::sleep(Duration::from_millis(800));
    let r = s.script(&format!("const v = document.querySelector('{selector}'); const r = v.getBoundingClientRect(); return [r.x, r.y, r.width, r.height, window.devicePixelRatio, window.innerWidth];"))?;
    let f = |i: usize| r[i].as_f64().unwrap_or(0.0);
    let dpr = f(4).max(1.0);
    let shot = s.screenshot()?;
    // the screenshot is in device pixels when it is wider than the CSS viewport
    let scale = if shot.0 as f64 > f(5) * 1.5 { dpr } else { 1.0 };
    let (colors, luma, black) = picture_stats(&shot, f(0) * scale, f(1) * scale, f(2) * scale, f(3) * scale);
    let detail = format!("{colors} colors, mean luma {luma:.0}, {:.0}% black ({}x{} css px, dpr {dpr})", black * 100.0, f(2) as u32, f(3) as u32);
    if colors < 40 || luma < 25.0 || luma > 225.0 || black > 0.1 {
        bail!("the view draws no real picture: {detail}");
    }
    Ok(detail)
}

const VIEW: &str = "main > [data-mcview=\"full\"]";

fn wait_for(path: &Path, secs: u64) -> bool {
    let t0 = Instant::now();
    while !path.exists() {
        if t0.elapsed() > Duration::from_secs(secs) {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}

/// Run the WebKit smoke on a generated site folder, or on the deployed site at `remote` (its
/// pages and bundles; the pages to check come from its own data/site.json). Returns the number of
/// failed checks.
pub fn smoke(dir: &Path, remote: Option<&str>) -> Result<usize> {
    if !Path::new(MINIBROWSER).is_file() {
        bail!("no {MINIBROWSER}: dnf install webkit2gtk3 webkit2gtk3-devel xorg-x11-server-Xvfb");
    }
    let (site, base): (site_model::Site, String) = match remote {
        Some(url) => {
            let url = if url.ends_with('/') { url.to_string() } else { format!("{url}/") };
            let o = Command::new("curl").args(["-fsSL", &format!("{url}data/site.json")]).output().context("running curl")?;
            if !o.status.success() {
                bail!("{url}data/site.json: {}", String::from_utf8_lossy(&o.stderr).trim());
            }
            (serde_json::from_slice(&o.stdout).context("the deployed data/site.json")?, url)
        }
        None => {
            let site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json"))?)?;
            // serve the folder
            let listener = TcpListener::bind(("127.0.0.1", 0))?;
            let port = listener.local_addr()?.port();
            let root: PathBuf = dir.to_path_buf();
            std::thread::spawn(move || crate::serve::serve_with(listener, root, Default::default()));
            (site, format!("http://127.0.0.1:{port}/"))
        }
    };
    // a display of our own
    let n = free_display().context("no free X display in :90..:99")?;
    let display = format!(":{n}");
    let _xvfb = Xvfb(Command::new("Xvfb").args([display.as_str(), "-screen", "0", "1400x1000x24", "-nolisten", "tcp"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().context("running Xvfb (dnf install xorg-x11-server-Xvfb)")?);
    if !wait_for(Path::new(&format!("/tmp/.X11-unix/X{n}")), 10) {
        bail!("Xvfb {display} did not start");
    }
    let pick = |kind: &str| site.builds.iter().find(|b| b.kind == kind && b.trace.is_none()).map(|b| b.page.clone());
    let mut pages: Vec<(String, String, &str)> = vec![("index".into(), "index.html#kind=clock".into(), ".thumb[data-mcview]")];
    for kind in ["door", "clock", "circuit"] {
        if let Some(p) = pick(kind) {
            pages.push((kind.to_string(), p, "[data-mcview=\"full\"]"));
        }
    }
    if let Some(c) = site.cards.first() {
        pages.push(("family".into(), c.page.clone(), "[data-mcview=\"full\"]"));
    }
    let mut fails = 0;
    // a tunable group with two versions (the in-place swap check)
    let group_pair = site.builds.iter().find_map(|b| {
        let c = b.circuit.as_ref()?;
        if c.tunable.is_empty() || c.host.is_some() || c.layout_of.is_some() {
            return None;
        }
        let other = site.builds.iter().find(|o| o.id != b.id && o.circuit.as_ref().is_some_and(|oc| oc.tunable == c.tunable && oc.host.is_none() && oc.layout_of.is_none()))?;
        let file = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        Some((b.page.clone(), file(&other.page), file(&other.design), other.name.clone()))
    });
    let family_page = site.cards.first().map(|c| c.page.clone());
    for phone in [false, true] {
        let label = if phone { "phone 390x844 @2x, iPhone UA" } else { "desktop 1280x900" };
        let s = match Session::start(&display, phone) {
            Ok(s) => s,
            Err(e) => {
                println!("FAIL webkit {label}: no browser session: {e:#}");
                fails += 1;
                continue;
            }
        };
        for (what, page, selector) in &pages {
            match check_page(&s, &base, page, selector) {
                Ok(d) => println!("PASS webkit {label}: {what} ({page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: {what} ({page}): {e:#}");
                    fails += 1;
                }
            }
        }
        if let Some((page, other_page, other_design, other_name)) = &group_pair {
            match check_picker_swap(&s, &base, page, other_page, other_design, other_name) {
                Ok(d) => println!("PASS webkit {label}: picker swap in place ({page} -> {other_page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: picker swap in place ({page} -> {other_page}): {e:#}");
                    fails += 1;
                }
            }
        }
        if let Some(page) = &family_page {
            match check_family_swap(&s, &base, page) {
                Ok(d) => println!("PASS webkit {label}: family member swap ({page}): {d}"),
                Err(e) => {
                    println!("FAIL webkit {label}: family member swap ({page}): {e:#}");
                    fails += 1;
                }
            }
        }
    }
    Ok(fails)
}
