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

fn respond(mut s: TcpStream, root: &Path) -> Result<()> {
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

/// Serve `root` on `listener` forever (one thread per connection).
pub fn serve_on(listener: TcpListener, root: PathBuf) {
    for s in listener.incoming().flatten() {
        let root = root.clone();
        std::thread::spawn(move || {
            let _ = respond(s, &root);
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
        .args(["--headless", "--no-sandbox", "--disable-gpu", "--enable-unsafe-swiftshader", "--hide-scrollbars", "--window-size=1400,1000"])
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

/// The number in `<span id="f-count"><strong>N</strong>`.
fn result_count(dom: &str) -> Option<usize> {
    let i = dom.find("id=\"f-count\"")?;
    let rest = &dom[i..];
    let s = rest.find("<strong>")? + 8;
    let e = rest[s..].find("</strong>")? + s;
    rest[s..e].trim().parse().ok()
}

/// Smoke-test a generated site in a headless browser. Returns the number of failed checks.
pub fn smoke(dir: &Path, chrome_path: Option<&Path>, budget_ms: u32) -> Result<usize> {
    let chrome = chrome(chrome_path)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let port = listener.local_addr()?.port();
    let root = dir.to_path_buf();
    std::thread::spawn(move || serve_on(listener, root));
    let base = format!("http://127.0.0.1:{port}/");
    let site: site_model::Site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json"))?)?;
    let mut fails = 0;
    let mut check = |name: &str, ok: bool, detail: String| {
        println!("{} {name}: {detail}", if ok { "PASS" } else { "FAIL" });
        if !ok {
            fails += 1;
        }
    };
    // the index: the UI ran, counted and drew the first page of cards
    let dom = dump_dom(&chrome, &format!("{base}index.html"), budget_ms)?;
    let want = site.run(&site_model::Query::default()).map(|(h, _)| h.len()).unwrap_or(0);
    let got = result_count(&dom);
    let cards = dom.matches("class=\"card").count();
    check("index: the UI loaded data/site.json and counted the default results", got == Some(want), format!("{got:?} results, engine says {want}"));
    check("index: first page of cards drawn", cards == want.min(60), format!("{cards} cards"));
    check("index: every card has a 3D thumbnail slot", dom.matches("data-mcview=\"thumb\"").count() == cards, format!("{} thumbnails", dom.matches("data-mcview=\"thumb\"").count()));
    check("index: the filter panel was drawn", dom.contains("id=\"f-time\"") && dom.contains("data-act=\"block\""), "time search + block-kind chips".into());
    // the clock time search expands extendable families to their members
    let frag = "kind=clock&time=48%20gt";
    let q = site_model::Query::from_fragment(frag);
    let (hits, _) = site.run(&q).map_err(anyhow::Error::msg)?;
    let members = hits.iter().filter(|h| matches!(h, site_model::Hit::Member(_))).count();
    let dom = dump_dom(&chrome, &format!("{base}index.html#{frag}"), budget_ms)?;
    check("time search 48 gt: count", result_count(&dom) == Some(hits.len()), format!("{:?} results, engine says {} ({members} members)", result_count(&dom), hits.len()));
    check("time search 48 gt: family members shown with their n", members == 0 || dom.contains("<span class=\"n\">n = "), format!("{} member cards", dom.matches("class=\"card member\"").count()));
    check("time search 48 gt: the hint names the period", dom.contains("= 48 gt (2.4 s)"), "hint".into());
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
    }
    if let Some(c) = site.cards.first() {
        let dom = dump_dom(&chrome, &format!("{base}{}", c.page), budget_ms)?;
        check("family page: every member listed", dom.matches("<tr id=\"n").count() == site.members.iter().filter(|m| m.card == c.id).count(), c.page.clone());
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
