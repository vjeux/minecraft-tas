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
            .arg(format!("--force-device-scale-factor={scale}"))
            .arg(url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
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
        Ok(Browser { child, to, from: rx, next: 1, session: None })
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
}

fn parse_state(s: &str) -> Option<State> {
    if !s.contains("tick=") {
        return None;
    }
    let mut st = State { tick: 0, playing: false, door: None, inputs: Vec::new(), active: None };
    for w in s.split_whitespace() {
        let (k, v) = w.split_once('=')?;
        match k {
            "tick" => st.tick = v.parse().ok()?,
            "playing" => st.playing = v == "1",
            "door" => st.door = v.parse().ok(),
            "active" => st.active = v.parse().ok(),
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
        let verdict = loop {
            if let Some(Some(now)) = states(&mut b)?.get(i).cloned() {
                let flipped = now.inputs.iter().find_map(|(xy, on)| xy.map(|_| *on)) == Some(!on);
                let reacted = match (st.door, now.door) {
                    (Some(a), Some(c)) => a != c,
                    _ => now.tick > st.tick + 2 && (now.active != st.active || now.active.is_none()),
                };
                last = now.clone();
                if flipped && now.playing && reacted {
                    break Ok(now);
                }
            }
            if Instant::now() > deadline {
                break Err(last.clone());
            }
            std::thread::sleep(Duration::from_millis(200));
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
