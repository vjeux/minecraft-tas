//! `mctas site read`: a page's text as a visitor reads it, in order (headings, paragraphs, list
//! items, table rows), for a player's-eye review of the kind pages (the coordinator 2026-10-21
//! 12:25Z: "does each card/page make sense for real use"). The 3D view, the charts and the design
//! text are left out; `--details` keeps the Details block (the judge's test, the measurements).

use anyhow::{Context, Result};
use std::path::Path;

/// Every `open ... close` span cut out of `s` (not nested).
fn cut_all(s: &str, open: &str, close: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find(open) {
        out.push_str(&rest[..i]);
        match rest[i..].find(close) {
            Some(j) => rest = &rest[i + j + close.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text of a generated page's `<main>`, one block per line (`#` headings, `-` list items, `|`
/// table rows).
pub fn page_text(html: &str, details: bool) -> String {
    let main = html.split("<main").nth(1).and_then(|m| m.split_once('>')).map(|(_, m)| m).and_then(|m| m.split("</main>").next()).unwrap_or(html);
    let mut s = main.to_string();
    if !details && let Some(i) = s.find("<details class=\"more\">") {
        s.truncate(i);
    }
    for (open, close) in [("<svg", "</svg>"), ("<script", "</script>"), ("<pre", "</pre>"), ("<style", "</style>"), ("<div class=\"fallback", "</div>")] {
        s = cut_all(&s, open, close);
    }
    for (tag, mark) in [("<h1", "\n# "), ("<h2", "\n## "), ("<h3", "\n### "), ("<li", "\n- "), ("<tr", "\n| "), ("<td", " · "), ("<th", " · "), ("<p", "\n"), ("<br", "\n"), ("<summary", "\n"), ("<section", "\n"), ("<div", "\n"), ("<dt", "\n"), ("<dd", " ")] {
        s = s.replace(tag, &format!("{mark}<x"));
    }
    // the tags themselves
    let mut text = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    let text = text.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&#39;", "'").replace("&quot;", "\"").replace("&nbsp;", " ").replace('\u{a0}', " ");
    let mut lines: Vec<String> = Vec::new();
    for l in text.lines() {
        let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
        let l = l.trim_start_matches("· ").to_string();
        if l.is_empty() || l == "#" || l == "##" || l == "-" || l == "|" || lines.last() == Some(&l) {
            continue;
        }
        lines.push(l);
    }
    lines.join("\n")
}

/// The pages of every card on the Doors and Circuits tabs (one per kind), in their order.
pub fn kind_pages(site: &site_model::Site) -> Vec<String> {
    let mut pages = Vec::new();
    for frag in ["kind=door", "kind=circuit"] {
        let q = site_model::Query::from_fragment(frag);
        if let Ok((hits, _)) = site.run(&q) {
            for h in hits {
                if let site_model::Hit::Build(i) = h
                    && let Some(b) = site.builds.get(i)
                {
                    pages.push(b.page.clone());
                }
            }
        }
    }
    pages
}

pub fn read(dir: &Path, kinds: bool, pages: &[String], details: bool) -> Result<()> {
    let mut list: Vec<String> = pages.to_vec();
    if kinds {
        let site: site_model::Site = serde_json::from_str(&std::fs::read_to_string(dir.join("data/site.json")).context("data/site.json")?)?;
        list.extend(kind_pages(&site));
    }
    for p in &list {
        let html = std::fs::read_to_string(dir.join(p)).with_context(|| format!("{}", dir.join(p).display()))?;
        println!("==== {p}\n{}\n", page_text(&html, details));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::page_text;

    #[test]
    fn a_page_reads_in_order_without_its_view() {
        let html = "<header>nav</header><main><h1>Lock · 64 blocks</h1><p class=\"sub\">made of <b>stone</b></p><div class=\"viewer\"><div class=\"fallback\"><span>3D view loading</span></div></div><section><h2>Try it</h2><p>Tap a dial&#39;s tag.</p><ul><li>one</li><li>two</li></ul></section><svg><text>D0</text></svg><details class=\"more\"><summary>Details</summary><p>deep</p></details></main><footer>f</footer>";
        assert_eq!(page_text(html, false), "# Lock · 64 blocks\nmade of stone\n## Try it\nTap a dial's tag.\n- one\n- two");
        assert!(page_text(html, true).ends_with("Details\ndeep"));
    }
}
