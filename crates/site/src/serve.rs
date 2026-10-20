//! `mctas site serve` (a static file server for previewing a generated site) and `mctas site
//! smoke` (the browser check without a person: serve the site on a free local port, load pages
//! in a headless Chrome with `--dump-dom`, and check the DOM the WebAssembly UI built: result
//! cards, the clock time search expanding families, facet filters, build pages).

use anyhow::{Context, Result, bail};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;

fn content_type(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "txt" => "text/plain; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => "application/octet-stream",
    }
}

fn respond(mut s: TcpStream, root: &Path, extra: &std::collections::HashMap<String, String>) -> Result<()> {
    let mut reader = BufReader::new(s.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    // drain the headers
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h)? == 0 || h == "\r\n" || h == "\n" {
            break;
        }
    }
    let raw = line.split_whitespace().nth(1).unwrap_or("/");
    let path = raw.split(['?', '#']).next().unwrap_or("/");
    let path = site_model::decode(path);
    // `/__slow?ms=N`: answers after N real milliseconds (a pending request keeps a headless
    // Chrome's virtual clock from running out while a WebAssembly bundle compiles)
    if path == "/__slow" {
        let ms: u64 = raw.split("ms=").nth(1).and_then(|v| v.split('&').next()).and_then(|v| v.parse().ok()).unwrap_or(1000);
        std::thread::sleep(std::time::Duration::from_millis(ms.min(120_000)));
        write!(s, "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")?;
        return Ok(());
    }
    if let Some(page) = extra.get(&path) {
        write!(s, "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", page.len())?;
        s.write_all(page.as_bytes())?;
        return Ok(());
    }
    let rel = path.trim_start_matches('/');
    let mut file = root.join(rel);
    if rel.split('/').any(|c| c == "..") {
        file = root.join("__forbidden__");
    }
    if file.is_dir() {
        file = file.join("index.html");
    }
    match std::fs::read(&file) {
        Ok(body) => {
            write!(s, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", content_type(&file), body.len())?;
            s.write_all(&body)?;
        }
        Err(_) => {
            let body = std::fs::read(root.join("404.html")).unwrap_or_else(|_| b"not found".to_vec());
            write!(s, "HTTP/1.1 404 Not Found\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len())?;
            s.write_all(&body)?;
        }
    }
    Ok(())
}

/// Serve `root` on `listener` forever (one thread per connection); `extra` pages are served at
/// their paths too (the smoke test's probe page).
pub fn serve_on(listener: TcpListener, root: PathBuf) {
    serve_with(listener, root, Default::default())
}

pub fn serve_with(listener: TcpListener, root: PathBuf, extra: std::collections::HashMap<String, String>) {
    let extra = std::sync::Arc::new(extra);
    for s in listener.incoming().flatten() {
        let root = root.clone();
        let extra = extra.clone();
        std::thread::spawn(move || {
            let _ = respond(s, &root, &extra);
        });
    }
}

pub fn serve(dir: &Path, port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port)).with_context(|| format!("binding 127.0.0.1:{port}"))?;
    println!("serving {} at http://127.0.0.1:{}/", dir.display(), listener.local_addr()?.port());
    serve_on(listener, dir.to_path_buf());
    Ok(())
}

/// The headless Chrome to use: the given path, $CHROME, else Chrome for Testing's headless shell
/// unpacked in ~/tools.
fn chrome(given: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = given {
        return Ok(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("CHROME") {
        return Ok(PathBuf::from(p));
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let p = PathBuf::from(home).join("tools/chrome-headless-shell-linux64/chrome-headless-shell");
    if p.is_file() {
        return Ok(p);
    }
    bail!("no headless Chrome: pass --chrome, set CHROME, or unpack Chrome for Testing's chrome-headless-shell-linux64.zip into ~/tools")
}

/// The DOM of `url` after its scripts ran (`--dump-dom` with a virtual time budget).
pub fn dump_dom(chrome: &Path, url: &str, budget_ms: u32) -> Result<String> {
    let o = Command::new(chrome)
        .args(["--headless", "--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--hide-scrollbars", "--window-size=1400,1000"])
        .arg(format!("--virtual-time-budget={budget_ms}"))
        .arg("--dump-dom")
        .arg(url)
        .output()
        .with_context(|| format!("running {}", chrome.display()))?;
    if !o.status.success() {
        bail!("chrome failed on {url}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A screenshot of `url` (PNG bytes): unlike `--dump-dom`, it produces frames, so WebGL and
/// requestAnimationFrame run (WebGL2 in software: SwiftShader).
pub fn screenshot(chrome: &Path, url: &str, budget_ms: u32, w: u32, h: u32) -> Result<Vec<u8>> {
    let file = std::env::temp_dir().join(format!("mctas-site-shot-{}.png", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let o = Command::new(chrome)
        .args(["--headless", "--no-sandbox", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--hide-scrollbars"])
        .arg(format!("--window-size={w},{h}"))
        .arg(format!("--virtual-time-budget={budget_ms}"))
        .arg(format!("--screenshot={}", file.display()))
        .arg(url)
        .output()
        .with_context(|| format!("running {}", chrome.display()))?;
    if !o.status.success() {
        bail!("chrome failed on {url}: {}", String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or(""));
    }
    let png = std::fs::read(&file).with_context(|| format!("no screenshot of {url}"))?;
    let _ = std::fs::remove_file(&file);
    Ok(png)
}

/// Decode an 8-bit RGB or RGBA, non-interlaced PNG (what Chrome writes) into RGB pixels.
pub fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<[u8; 3]>)> {
    use std::io::Read;
    if bytes.len() < 8 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" {
        bail!("not a PNG");
    }
    let (mut w, mut h, mut channels) = (0u32, 0u32, 0usize);
    let mut idat = Vec::new();
    let mut i = 8;
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes(bytes[i..i + 4].try_into()?) as usize;
        let kind = &bytes[i + 4..i + 8];
        let data = bytes.get(i + 8..i + 8 + len).context("truncated PNG chunk")?;
        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes(data[0..4].try_into()?);
                h = u32::from_be_bytes(data[4..8].try_into()?);
                let (depth, color, interlace) = (data[8], data[9], data[12]);
                if depth != 8 || interlace != 0 {
                    bail!("PNG bit depth {depth} / interlace {interlace} not supported");
                }
                channels = match color {
                    2 => 3,
                    6 => 4,
                    c => bail!("PNG color type {c} not supported"),
                };
            }
            b"IDAT" => idat.extend_from_slice(data),
            b"IEND" => break,
            _ => {}
        }
        i += 12 + len;
    }
    let mut raw = Vec::new();
    flate2::read::ZlibDecoder::new(&idat[..]).read_to_end(&mut raw)?;
    let stride = w as usize * channels;
    if raw.len() < (stride + 1) * h as usize {
        bail!("PNG data too short");
    }
    let mut prev = vec![0u8; stride];
    let mut cur = vec![0u8; stride];
    let mut px = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        let row = &raw[y * (stride + 1)..(y + 1) * (stride + 1)];
        let filter = row[0];
        for x in 0..stride {
            let a = if x >= channels { cur[x - channels] as i32 } else { 0 };
            let b = prev[x] as i32;
            let c = if x >= channels { prev[x - channels] as i32 } else { 0 };
            let pred = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                4 => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());
                    if pa <= pb && pa <= pc { a } else if pb <= pc { b } else { c }
                }
                f => bail!("PNG filter {f}"),
            };
            cur[x] = (row[1 + x] as i32 + pred) as u8;
        }
        for x in 0..w as usize {
            px.push([cur[x * channels], cur[x * channels + 1], cur[x * channels + 2]]);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    Ok((w, h, px))
}

/// The probe page of the viewer check: one design filling the window as a thumbnail, over a
/// flat fallback with no text (one color), so any rendering shows as many colors; a slow request
/// holds the page open while the viewer compiles and draws.
fn probe_page(root: &str, design: &str, kind: &str) -> String {
    format!(
        "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>probe</title><style>html,body{{margin:0;background:#808080}}#v{{position:fixed;inset:0}}.fallback{{position:absolute;inset:0;background:#808080}}.mcview-ready>.fallback{{display:none}}img{{position:fixed;width:1px;height:1px}}</style></head><body><div id=\"v\" data-mcview=\"thumb\" data-src=\"{root}{}\" data-kind=\"{kind}\"><div class=\"fallback\"></div></div><img src=\"__slow?ms=15000\" alt=\"\"><script type=\"module\">import init from \"{}viewer/mcview.js\"; init();</script></body></html>",
        site_model::html::esc(design),
        if root.is_empty() { "./" } else { root }
    )
}

/// The number in `<span id="f-count"><strong>N</strong>`.
/// GET a path from the local server: the body (status 200 only).
fn http_get_local(port: u16, path: &str) -> Result<Vec<u8>> {
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    write!(s, "GET /{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path.trim_start_matches('/'))?;
    let mut all = Vec::new();
    std::io::Read::read_to_end(&mut s, &mut all)?;
    let head_end = all.windows(4).position(|w| w == b"\r\n\r\n").context("no HTTP header")?;
    let head = String::from_utf8_lossy(&all[..head_end]).to_string();
    if !head.starts_with("HTTP/1.1 200") {
        bail!("GET /{path}: {}", head.lines().next().unwrap_or(""));
    }
    Ok(all[head_end + 4..].to_vec())
}

fn result_count(dom: &str) -> Option<usize> {
    let i = dom.find("id=\"f-count\"")?;
    let rest = &dom[i..];
    let s = rest.find("<strong>")? + 8;
    let e = rest[s..].find("</strong>")? + s;
    rest[s..e].trim().parse().ok()
}

/// Smoke-test a generated site in a headless browser. Returns the number of failed checks.
/// `remote`: check the deployed site at that URL instead (the pages and the bundles there; the
/// expected answers still come from `dir`'s data, the build that was deployed).
pub fn smoke(dir: &Path, chrome_path: Option<&Path>, budget_ms: u32, remote: Option<&str>) -> Result<usize> {
    let chrome = chrome(chrome_path)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let root = dir.to_path_buf();
    let site: site_model::Site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json"))?)?;
    let remote = remote.map(|r| if r.ends_with('/') { r.to_string() } else { format!("{r}/") });
    // the viewer probes (served locally, loading the site's bundle and designs): one door, one
    // clock, each filling the window
    let mut extra = std::collections::HashMap::new();
    let probe_root = remote.clone().unwrap_or_default();
    let probes: Vec<(String, String)> = ["door", "clock"]
        .iter()
        .filter_map(|k| site.builds.iter().find(|b| b.kind == *k).map(|b| (format!("/__probe-{k}.html"), probe_page(&probe_root, &b.design, k))))
        .collect();
    for (p, html) in &probes {
        extra.insert(p.clone(), html.clone());
    }
    std::thread::spawn(move || serve_with(listener, root, extra));
    let local = format!("http://127.0.0.1:{port}/");
    let base = remote.clone().unwrap_or_else(|| local.clone());
    let mut fails = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{} {name}: {detail}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            fails += 1;
        }
    };
    // static checks (the files of the build): nothing community-derived is published (vjeux
    // 01:15Z), and the circuit pages explain themselves (vjeux 01:05Z)
    let community: Vec<&str> = site
        .builds
        .iter()
        .filter(|b| b.kind == "door")
        .filter(|b| crate::doors::community_seed(&b.name, &std::fs::read_to_string(dir.join(&b.design)).unwrap_or_default()).is_some())
        .map(|b| b.name.as_str())
        .collect();
    let stray = std::fs::read_dir(dir.join("designs/doors")).map(|rd| rd.flatten().filter(|e| crate::doors::community_seed(&e.file_name().to_string_lossy(), "").is_some()).count()).unwrap_or(0);
    check("no community door published (pages, data, design files)", community.is_empty() && stray == 0, format!("{} builds, {stray} files{}", community.len(), community.first().map(|n| format!(", e.g. {n}")).unwrap_or_default()));
    if let Some(b) = site.builds.iter().find(|b| b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("counter "))) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let ok = ["<h2>What it does</h2>", "<h2>How to try it</h2>", "data-stimulus=\"", "data-input-mode=\"pulse\"", "<svg class=\"wave\"", "<h2>Measurements</h2>", "about.html#glossary"].iter().all(|s| page.contains(s));
        check("circuit page explains itself: what it does, how to try it, the judge's test, metric meanings", ok, b.page.clone());
        // the redstone first (vjeux): the live view comes before the explanation
        let first = page.find("data-mcview=\"full\"").zip(page.find("<h2>What it does</h2>")).is_some_and(|(v, w)| v < w);
        check("circuit page: the live 3D view before the explanation", first, b.page.clone());
    }
    let study = std::fs::read_to_string(dir.join("circuits/memory/index.html")).unwrap_or_default();
    check("study page: intro + designs", study.contains("<h1>Memory</h1>") && study.contains("class=\"card"), "circuits/memory/index.html".into());
    // no simulator attribution on the pages (vjeux 02:01Z): design files (<pre>) and class names aside
    let mut attributed: Vec<String> = Vec::new();
    let sample: Vec<String> = ["index.html", "about.html", "families/index.html"].iter().map(|s| s.to_string()).chain(site.builds.iter().filter(|b| b.kind == "circuit").take(40).map(|b| b.page.clone())).chain(site.cards.iter().map(|c| c.page.clone())).collect();
    for p in &sample {
        let html = std::fs::read_to_string(dir.join(p)).unwrap_or_default();
        let mut visible = String::new();
        let mut rest = html.as_str();
        while let Some(i) = rest.find("<pre") {
            visible.push_str(&rest[..i]);
            rest = rest[i..].find("</pre>").map(|j| &rest[i + j..]).unwrap_or("");
        }
        visible.push_str(rest);
        if visible.replace("st-redsim", "").to_lowercase().contains("redsim") {
            attributed.push(p.clone());
        }
    }
    check("no simulator attribution on the pages", attributed.is_empty(), format!("{} of {} pages{}", attributed.len(), sample.len(), attributed.first().map(|p| format!(", e.g. {p}")).unwrap_or_default()));
    if let Some(b) = site.builds.iter().find(|b| b.circuit.as_ref().is_some_and(|c| !c.span.is_empty())) {
        let page = std::fs::read_to_string(dir.join(&b.page)).unwrap_or_default();
        let picks = page.matches("class=\"pick").count();
        check("tunable group: the member picker, the current member marked", picks >= 2 && page.contains("aria-current=\"page\""), format!("{}: {picks} members", b.page));
    }
    // the index: the UI ran, counted and drew the first page of cards
    let dom = dump_dom(&chrome, &format!("{base}index.html"), budget_ms)?;
    let want = site.run(&site_model::Query::default()).map(|(h, _)| h.len()).unwrap_or(0);
    let got = result_count(&dom);
    let cards = dom.matches("class=\"card").count();
    check("index: the UI loaded data/site.json and counted the default results", got == Some(want), format!("{got:?} results, engine says {want}"));
    check("index: first page of cards drawn", cards == want.min(60), format!("{cards} cards"));
    check("index: every card has a 3D thumbnail slot", dom.matches("data-mcview=\"thumb\"").count() == cards, format!("{} thumbnails", dom.matches("data-mcview=\"thumb\"").count()));
    // the header: the title, the three kind tabs and the search box, nothing else (vjeux 01:13Z)
    let header = dom.split("<header class=\"top\">").nth(1).and_then(|h| h.split("</header>").next()).unwrap_or("");
    check(
        "index: the header is the title, Doors / Clocks / Circuits and the search box",
        header.matches("<a ").count() == 4 && header.contains("id=\"tab-door\"") && header.contains("id=\"tab-clock\"") && header.contains("id=\"tab-circuit\"") && header.contains("id=\"hq\"") && header.contains("class=\"count\""),
        format!("{} links, search box: {}", header.matches("<a ").count(), header.contains("id=\"hq\"")),
    );
    check("index: the filter panel was drawn", dom.contains("data-act=\"block\"") && !dom.contains("id=\"f-time\""), "block-kind chips, no second search box".into());
    // the clock time search expands extendable families to their members
    let frag = "kind=clock&time=48%20gt";
    let q = site_model::Query::from_fragment(frag);
    let (hits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let members = hits.iter().filter(|h| matches!(h, site_model::Hit::Member(_))).count();
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("time search 48 gt: count", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {} ({members} members)", result_count(&dom), hits.len()));
    check("time search 48 gt: family members shown with their n", members == 0 || dom.contains("<span class=\"n\">n = "), format!("{} member cards", dom.matches("class=\"card member\"").count()));
    check("time search 48 gt: the count names the period", dom.contains("with a period of 48 gt (2.4 s)"), "summary".into());
    // the header's search form from another page (index.html?kind=clock&q=48): the Clocks tab's
    // search box takes a period
    let dom = dump_dom(&chrome, &format!("{base}index.html?kind=clock&q=48"), budget_ms)?;
    check("header search from another page (?kind=clock&q=48): the time search", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {}", result_count(&dom), hits.len()));
    // the circuit value search expands the tunable groups to the member that hits it
    let frag = "kind=circuit&value=L%2012";
    let q = site_model::Query::from_fragment(frag);
    let (vhits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("circuit value search L 12: count", result_count(&dom) == Some(vhits.len()) && !vhits.is_empty(), format!("{:?} results, engine says {}", result_count(&dom), vhits.len()));
    // ... and the extendable circuit families to the generated member (the first card's n_lo + 1)
    let probe = site.cards.iter().filter(|c| c.kind == "circuit").find_map(|c| {
        let f = c.families.first()?;
        let m = site.members.iter().find(|m| m.card == c.id && m.family == f.id && m.n == f.n_lo + 1 && m.build.is_none())?;
        let letter = m.axis.split_once(" = ").map(|(l, _)| l.to_string())?;
        Some((format!("{letter} {}", m.period), m.family.clone(), m.n))
    });
    if let Some((value, family, n)) = probe {
        let frag = format!("kind=circuit&value={}", site_model::encode(&value));
        let q = site_model::Query::from_fragment(&frag);
        let (fhits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
        let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
        let shown = dom.contains(&format!("{family} <span class=\"n\">n = {n}</span>"));
        check(&format!("circuit value search {value}: the generated family member"), result_count(&dom) == Some(fhits.len()) && shown, format!("{family} n = {n}: {:?} results, engine says {}", result_count(&dom), fhits.len()));
    }
    // a facet filter from the URL
    let frag = "kind=door&is=flat,slime-free&sort=volume";
    let q = site_model::Query::from_fragment(frag);
    let (hits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("doors, flat + slime-free, by volume: count", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {}", result_count(&dom), hits.len()));
    if let Some(site_model::Hit::Build(i)) = hits.first() {
        let first = &site.builds[*i];
        check("doors, flat + slime-free, by volume: the smallest first", dom.find(&format!("href=\"{}\"", first.page)).is_some_and(|p| p < dom.find("class=\"card").map(|c| c + 400).unwrap_or(0)), first.name.clone());
    }
    // a build page and a family page
    if let Some(b) = site.builds.iter().find(|b| b.kind == "door") {
        let dom = dump_dom(&chrome, &format!("{base}{}", b.page), budget_ms)?;
        check("door page: live viewer slot + properties", dom.contains("data-mcview=\"full\"") && dom.contains("Open / close"), b.page.clone());
        // its .litematic: linked, served, a schematic of exactly the design file next to it
        let lpath = crate::generate::litematic_path(&b.design);
        let linked = dom.contains(&format!("href=\"../{lpath}\""));
        let verdict = http_get_local(port, &lpath).and_then(|bytes| {
            let s = litematic::read(&bytes)?;
            let d = mctas::design::Design::parse(&std::fs::read_to_string(dir.join(&b.design))?)?;
            litematic::compare(&d, &s)?;
            Ok(format!("{} bytes, {} blocks, Version {} for data version {}", bytes.len(), s.total_blocks.unwrap_or(0), s.version, s.data_version))
        });
        check("door page: .litematic linked, served and equal to the design", linked && verdict.is_ok(), match verdict {
            Ok(v) => format!("{lpath}: {v}"),
            Err(e) => format!("{lpath}: linked {linked}, {e:#}"),
        });
    }
    if let Some(c) = site.cards.first() {
        let dom = dump_dom(&chrome, &format!("{base}{}", c.page), budget_ms)?;
        check("family page: every member listed", dom.matches("<tr id=\"n").count() == site.members.iter().filter(|m| m.card == c.id).count(), c.page.clone());
    }
    // the 3D viewer (webrender's bundle), when the site has one: a probe page renders one
    // design over a one-color fallback; a render has thousands of colors
    if dir.join("viewer/mcview.js").exists() {
        for (path, _) in &probes {
            let png = screenshot(&chrome, &format!("{local}{}", path.trim_start_matches('/')), budget_ms.max(60_000), 480, 360)?;
            let (w, h, px) = decode_png(&png)?;
            let distinct: std::collections::HashSet<[u8; 3]> = px.iter().copied().collect();
            let gray = px.iter().filter(|p| **p == [128, 128, 128]).count();
            check(
                &format!("viewer renders {}", path.trim_start_matches("/__probe-").trim_end_matches(".html")),
                distinct.len() > 500 && gray * 2 < px.len(),
                format!("{w}x{h}: {} colors, {:.0}% fallback gray", distinct.len(), 100.0 * gray as f64 / px.len().max(1) as f64),
            );
        }
    } else {
        println!("SKIP viewer: no viewer/mcview.js in the site (build with --viewer)");
    }
    Ok(fails)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_parse() {
        assert_eq!(result_count("<span id=\"f-count\"><strong>42</strong> results</span>"), Some(42));
        assert_eq!(result_count("nothing"), None);
    }

    /// A 2x2 RGBA PNG with a Sub-filtered and an Up-filtered row decodes to its pixels.
    #[test]
    fn png_decodes() {
        use std::io::Write as _;
        let raw: Vec<u8> = vec![
            1, 10, 20, 30, 255, 5, 5, 5, 0, // Sub: (10,20,30) then (15,25,35)
            2, 1, 1, 1, 0, 2, 2, 2, 0, // Up: (11,21,31) then (17,27,37)
        ];
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&raw).unwrap();
        let idat = z.finish().unwrap();
        let chunk = |kind: &[u8], data: &[u8]| -> Vec<u8> {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(kind);
            c.extend_from_slice(data);
            c.extend_from_slice(&[0, 0, 0, 0]);
            c
        };
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(chunk(b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 2, 8, 6, 0, 0, 0]));
        png.extend(chunk(b"IDAT", &idat));
        png.extend(chunk(b"IEND", &[]));
        let (w, h, px) = decode_png(&png).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(px, vec![[10, 20, 30], [15, 25, 35], [11, 21, 31], [17, 27, 37]]);
    }

    #[test]
    fn serves_files() {
        let dir = std::env::temp_dir().join(format!("site-serve-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("pkg")).unwrap();
        std::fs::write(dir.join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(dir.join("pkg/a.wasm"), [0u8, 97, 115, 109]).unwrap();
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let root = dir.clone();
        std::thread::spawn(move || serve_on(l, root));
        let get = |path: &str| -> String {
            let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(s, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
            let mut out = Vec::new();
            std::io::Read::read_to_end(&mut s, &mut out).unwrap();
            String::from_utf8_lossy(&out).into_owned()
        };
        let r = get("/");
        assert!(r.starts_with("HTTP/1.1 200") && r.ends_with("<p>hi</p>"), "{r}");
        assert!(get("/pkg/a.wasm").contains("Content-Type: application/wasm"));
        assert!(get("/../etc/passwd").starts_with("HTTP/1.1 404"));
        assert!(get("/index.html?x=1#y").starts_with("HTTP/1.1 200"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
