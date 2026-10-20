//! The static pages: the index (the filtering UI's shell), one page per build, one per
//! extendable family card, the family index, a static list of everything (no script needed),
//! the about page and the 404 page. Every page is plain HTML5 with relative links (the site is a
//! GitHub project page under /minecraft-tas/); viewers are `data-mcview` elements that
//! webrender's bundle (viewer/mcview.js) renders when present.

use site_model::html::{build_card, esc, kind_title, member_href};
use site_model::{Build, Card, Member, Site, flag_label, real_time, status_label};
use std::collections::BTreeMap;

pub const REPO_URL: &str = "https://github.com/vjeux/minecraft-tas";
pub const SITE_NAME: &str = "REDSTONEPEDIA";

/// What a page loads besides its HTML.
#[derive(Clone, Copy, Default)]
pub struct Scripts {
    /// webrender's viewer bundle (viewer/mcview.js): only when the site has one.
    pub viewer: bool,
    /// The filtering UI (pkg/redstonepedia.js) on this page.
    pub app: bool,
    /// Whether the site has the UI at all (pages with in-place swaps load it too).
    pub ui: bool,
}

/// A whole page around `body`. `prefix` is the path from the page to the site root.
pub fn shell(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site) -> String {
    shell_full(title, description, prefix, body, scripts, site, None, None)
}

/// [`shell`] with a preview image for link unfurls (Open Graph).
pub fn shell_with(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site, image: Option<&str>) -> String {
    shell_full(title, description, prefix, body, scripts, site, image, None)
}

/// The header (vjeux, 01:13Z: "clean up the top"): the title, the Doors / Clocks / Circuits tabs
/// (`kind`: the page's own, shown active) and the search box (a plain form to the index, which
/// the index's UI turns into its live search; on the Clocks tab it also takes a period).
pub fn header(prefix: &str, kind: Option<&str>) -> String {
    header_with(prefix, kind, None)
}

/// The shown designs per tab (the index's own counts with no search), computed once per build.
fn tab_counts(site: &Site) -> [usize; 3] {
    static COUNTS: std::sync::OnceLock<[usize; 3]> = std::sync::OnceLock::new();
    *COUNTS.get_or_init(|| {
        let n = |k: &str| site.run(&site_model::Query::from_fragment(&format!("kind={k}"))).map(|(h, _)| h.len()).unwrap_or(0);
        [n("door"), n("clock"), n("circuit")]
    })
}

/// [`header`] with the tabs' counts: every page's header reads like the index's (critique C44:
/// the study pages' tabs lost their counts and the search said just "Search").
pub fn header_with(prefix: &str, kind: Option<&str>, counts: Option<[usize; 3]>) -> String {
    let tab = |k: &str, label: &str, frag: &str, n: Option<usize>| -> String {
        let count = n.map(|n| format!(" <span class=\"count\">{n}</span>")).unwrap_or_default();
        format!("<a id=\"tab-{k}\" class=\"tab-top{}\" href=\"{prefix}index.html#{frag}\">{label}{count}</a>", if kind == Some(k) { " on" } else { "" })
    };
    let hidden = match kind {
        Some(k) => format!("<input type=\"hidden\" name=\"kind\" value=\"{}\">", esc(k)),
        None => String::new(),
    };
    // the index UI's own placeholders (web/site's render)
    let placeholder = match kind {
        Some("clock") => "Search: 48 ticks, 2.5 s…",
        Some("door") => "Search doors",
        Some("circuit") => "Search: toggle, timer, 12…",
        _ => "Search every build",
    };
    format!(
        "<header class=\"top\"><a class=\"logo\" href=\"{prefix}index.html\"><span class=\"logo-dot\"></span>{SITE_NAME}</a><nav class=\"tabs-top\" aria-label=\"kinds\">{}{}{}</nav>\
         <form id=\"hsearch\" class=\"hsearch\" action=\"{prefix}index.html\" method=\"get\" role=\"search\">{hidden}<input id=\"hq\" name=\"q\" type=\"search\" placeholder=\"{placeholder}\" aria-label=\"Search\" autocomplete=\"off\"></form></header>\n",
        tab("door", "Doors", "kind=door", counts.map(|c| c[0])),
        tab("clock", "Clocks", "kind=clock", counts.map(|c| c[1])),
        tab("circuit", "Circuits", "kind=circuit", counts.map(|c| c[2]))
    )
}

/// [`shell`] with everything: the preview image and the page's kind (its header tab).
#[allow(clippy::too_many_arguments)]
pub fn shell_full(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site, image: Option<&str>, kind: Option<&str>) -> String {
    let mut s = String::with_capacity(body.len() + 4096);
    s.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    s.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    let full_title = if title == SITE_NAME { title.to_string() } else { format!("{title} · {SITE_NAME}") };
    s.push_str(&format!("<title>{}</title>\n", esc(&full_title)));
    s.push_str(&format!("<meta name=\"description\" content=\"{}\">\n", esc(description)));
    s.push_str(&format!("<meta property=\"og:title\" content=\"{}\">\n<meta property=\"og:description\" content=\"{}\">\n<meta property=\"og:site_name\" content=\"{SITE_NAME}\">\n", esc(&full_title), esc(description)));
    if let Some(img) = image {
        s.push_str(&format!("<meta property=\"og:image\" content=\"{}\">\n<meta name=\"twitter:card\" content=\"summary_large_image\">\n", esc(img)));
    }
    s.push_str(&format!("<link rel=\"stylesheet\" href=\"{prefix}style.css{}\">\n", { let c = versions().css; if c.is_empty() { String::new() } else { format!("?v={c}") } }));
    s.push_str(&format!("<link rel=\"icon\" href=\"{prefix}favicon.svg\" type=\"image/svg+xml\">\n"));
    s.push_str("</head>\n<body>\n");
    s.push_str(&header_with(prefix, kind, Some(tab_counts(site))));
    s.push_str("<main>\n");
    s.push_str(body);
    s.push_str("\n</main>\n");
    // no page links in the footer (vjeux 13:35Z: "it's not to put them back in the footer"); the
    // date without the time, so it does not wrap on a phone (critique C52)
    s.push_str(&format!(
        "<footer>Built from <a href=\"{REPO_URL}/tree/{}\">vjeux/minecraft-tas@{}</a> on <span class=\"nowrap\">{}</span>. Every archived design was tested in the real game (Minecraft 26.3, tick by tick); the members of extendable families are generated from them and marked tested in the game, simulated or formula.</footer>\n",
        esc(&site.commit),
        esc(&site.commit.chars().take(9).collect::<String>()),
        esc(site.generated.split('T').next().unwrap_or(&site.generated))
    ));
    // module specifiers must start with ./ or ../ to be relative
    let module = if prefix.is_empty() { "./" } else { prefix };
    // cache-busting (GitHub Pages serves max-age=600): every bundle by a content hash, the wasm
    // passed to init explicitly (its default URL would drop the query)
    let v = versions();
    if scripts.app {
        s.push_str(&format!(
            "<script type=\"module\">import init from \"{module}pkg/redstonepedia.js?v={}\"; init({{module_or_path: \"{module}pkg/redstonepedia_bg.wasm?v={}\"}});</script>\n",
            v.ui_js, v.ui_wasm
        ));
    }
    if scripts.viewer {
        s.push_str(&format!(
            "<script type=\"module\">import init from \"{module}viewer/mcview.js?v={}\"; init({{module_or_path: \"{module}viewer/mcview_bg.wasm?v={}\"}});</script>\n",
            v.viewer_js, v.viewer_wasm
        ));
    }
    s.push_str("</body>\n</html>\n");
    s
}

/// Content hashes of the files pages load by a fixed URL (`?v=<hash>`), set by `site build`
/// once the files are written (empty: no query).
#[derive(Clone, Debug, Default)]
pub struct Versions {
    pub css: String,
    pub viewer_js: String,
    pub viewer_wasm: String,
    pub ui_js: String,
    pub ui_wasm: String,
    pub data: String,
}

static VERSIONS: std::sync::RwLock<Option<Versions>> = std::sync::RwLock::new(None);

pub fn set_versions(v: Versions) {
    if let Ok(mut g) = VERSIONS.write() {
        *g = Some(v);
    }
}

fn versions() -> Versions {
    VERSIONS.read().ok().and_then(|g| g.clone()).unwrap_or_default()
}

/// The first 8 hex digits of a file's FNV-1a hash ("" when the file is missing).
pub fn file_hash(path: &std::path::Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => bytes_hash(&bytes),
        Err(_) => String::new(),
    }
}

pub fn bytes_hash(bytes: &[u8]) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", h >> 32)
}

fn row(k: &str, v: &str) -> String {
    format!("<tr><th scope=\"row\">{}</th><td>{v}</td></tr>\n", esc(k))
}

fn yes_no(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

fn chips(items: &[String], prefix: &str, param: &str) -> String {
    items
        .iter()
        .map(|k| format!("<a class=\"chip\" href=\"{prefix}index.html#{param}={}\">{}</a>", site_model::encode(k), esc(&k.replace('_', " "))))
        .collect::<Vec<_>>()
        .join(" ")
}

fn grid(cards: &[String]) -> String {
    format!("<div class=\"grid\">{}</div>\n", cards.concat())
}

/// A circuit page's sections after the viewer: how to try it, the judge's test as a timing
/// diagram, the measurements with their meanings.
fn circuit_sections(c: &site_model::Circuit, has_viewer: bool) -> String {
    let mut s = format!("<section><h2>How to try it</h2>{}</section>\n", crate::explain::how_to_try(c, has_viewer));
    if let Some(w) = &c.wave {
        let has_pos = w.rows.iter().any(|r| r.scale > 0);
        let pos_text = if !has_pos {
            String::new()
        } else if c.kind.starts_with("piston-extender") {
            " <span class=\"tag-pos\">Blue</span>: where the pushed block is, 0 = home, up to N = fully out (the dashed line); it must reach N while the input is on and come all the way back.".to_string()
        } else {
            " <span class=\"tag-pos\">Blue</span>: how far the machine has moved along its path, in blocks.".to_string()
        };
        let notes: String = w.rows.iter().filter(|r| !r.note.is_empty()).map(|r| format!("<p class=\"wave-note\"><span class=\"tag-pos\">{}</span>: {}.</p>", esc(&r.name.to_uppercase()), esc(&r.note))).collect();
        let has_marks = w.rows.iter().any(|r| !r.marks.is_empty());
        let intro = if has_marks {
            // a BUD switch: the judge changes its target cell, it has no inputs
            "The judge changes the target cell with this schedule (the TARGET row: <span class=\"mk-place-t\">green</span> = a block placed or changed, <span class=\"mk-clear-t\">red</span> = removed, <span class=\"mk-shape-t\">purple</span> = a shape-only change, <span class=\"mk-update-t\">blue</span> = a pure update with nothing changing; each phase's name says which test it is) and checks the output after every change; here it is simulated. <span class=\"tag-out\">Orange</span>: the output is on.".to_string()
        } else {
            format!(
                "The judge drives the input{} with this schedule and checks the output after every change; here it is simulated (the 3D view's demo plays the same). <span class=\"tag-in\">Lime</span>: an input is on; <span class=\"tag-out\">orange</span>: the output is on (taller = a stronger signal, 1 to 15).{pos_text}",
                if c.inputs.len() == 1 { "" } else { "s" }
            )
        };
        s.push_str(&format!(
            "<section><h2>Our test in the real game</h2><p>{intro} Time runs left to right in game ticks (20 gt = 1 s).</p><div class=\"wave-wrap\">{}</div><p class=\"hint phone-only\">Swipe the diagram sideways: the whole test lasts {} ticks ({}).</p>{notes}{}</section>\n",
            crate::explain::wave_svg(w),
            w.ticks,
            site_model::real_time(w.ticks),
            if c.table.is_empty() {
                String::new()
            } else {
                format!("<details><summary>Our test's report, step by step ({} lines)</summary><pre class=\"design\">{}</pre></details>", c.table.len(), esc(&c.table.join("\n")))
            }
        ));
    }
    if !c.metrics.is_empty() {
        let rows: String = c
            .metrics
            .iter()
            .map(|(k, v)| {
                let def = crate::explain::metric_definition(&c.kind, k);
                format!("<tr><th scope=\"row\">{}</th><td>{}</td><td class=\"def\">{}</td></tr>\n", esc(k), esc(&crate::explain::metric_value(&if k == "badges" { v.replace('+', ", ") } else { v.clone() })), esc(def))
            })
            .collect();
        s.push_str(&format!(
            "<section><h2>Measurements</h2><p class=\"hint\">Measured in our test in the real game, facing all 4 ways at 4 spots in the world. gt = game tick = 1/20 s (20 gt = 1 second).</p><table class=\"props metrics\"><thead><tr><th>metric</th><th>value</th><th>what it means</th></tr></thead><tbody>{rows}</tbody></table></section>\n"
        ));
    }
    s
}

/// The viewer of a build page (a live view when the viewer bundle is there; the interim picture
/// or the fallback text until then). Clocks run by themselves: their viewer starts playing.
fn viewer(prefix: &str, b: &Build) -> String {
    let trace = b.trace.as_ref().map(|t| format!(" data-trace=\"{}\"", esc(&format!("{prefix}{t}")))).unwrap_or_default();
    // circuits: the judge's test as the viewer's demo (webrender's contract: data-stimulus,
    // data-phases, data-input-mode), playing from the start
    let circuit = match &b.circuit {
        Some(c) => {
            let mut a = String::new();
            if !c.stimulus.is_empty() {
                a.push_str(&format!(" data-stimulus=\"{}\"", esc(&c.stimulus)));
            }
            // a strength input's values (the hex kinds: webrender 2e541dc02)
            if !c.levels.is_empty() {
                a.push_str(&format!(" data-levels=\"{}\"", esc(&c.levels)));
            }
            // the BUD's judge acts at its target cells, not at inputs: `data-events`
            if !c.events.is_empty() {
                a.push_str(&format!(" data-events=\"{}\"", esc(&c.events)));
            }
            if !c.stimulus.is_empty() || !c.events.is_empty() {
                a.push_str(&format!(" data-phases=\"{}\"", esc(&c.phases)));
            }
            a.push_str(&format!(" data-input-mode=\"{}\" data-circuit=\"{}\"", crate::explain::input_mode(&c.kind), esc(&c.kind)));
            a
        }
        None => String::new(),
    };
    // what runs by itself plays at once: clocks, flying machines, self-running TNT dupers, shulker
    // machines (always-on ones start by themselves; a start lever is the visitor's), the judge's
    // demo
    let autoplay = if b.kind == "clock" || b.circuit.as_ref().is_some_and(|c| !c.stimulus.is_empty() || !c.events.is_empty() || c.kind == "flying-machine" || c.kind == "tnt-duper" || c.kind.starts_with("shulker-")) { " data-autoplay" } else { "" };
    let (pic, cls) = match &b.picture {
        Some(p) => (format!("<img class=\"pic contain\" src=\"{}\" alt=\"{} in the game\" decoding=\"async\">", esc(p), esc(&page_title(b))), " has-pic"),
        None => (String::new(), ""),
    };
    format!(
        "<div class=\"viewer k-{}\" data-mcview=\"full\" data-src=\"{}\"{trace}{circuit}{autoplay} data-kind=\"{}\"><div class=\"fallback{cls}\">{pic}<span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span><span class=\"fb-note\">{}</span></div></div>\n",
        esc(&b.kind),
        esc(&format!("{prefix}{}", b.design)),
        esc(&b.kind),
        esc(&site_model::html::headline(b)),
        esc(&site_model::html::props(b).kind),
        if b.picture.is_some() { "picture from the real game; the live 3D view replaces it when available" } else { "3D view loading (or not available in this browser)" }
    )
}

/// The page of one build.
/// The path from a page (`doors/x.html`, `circuits/gates/y.html`) back to the site root.
pub fn prefix_of(page: &str) -> String {
    "../".repeat(page.matches('/').count())
}

pub fn build_page(site: &Site, b: &Build, design_text: &str, repo_path: &str, scripts: Scripts) -> String {
    let prefix_s = prefix_of(&b.page);
    let prefix = prefix_s.as_str();
    let mut body = String::new();
    if let Some(c) = &b.circuit {
        // circuits: Circuits › study (the kind is the line right under: critique, "Piston
        // extender" three times at the top)
        let (study_title, _) = crate::explain::study(&c.study).unwrap_or(("Circuits", ""));
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › <a href=\"{prefix}circuits/{}/index.html\">{}</a></p>\n",
            esc(&c.study),
            esc(study_title)
        ));
    } else {
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}index.html#kind={}\">{}</a> › <a href=\"{prefix}index.html#kind={}&amp;group={}\">{}</a></p>\n",
            esc(&b.kind),
            kind_title(&b.kind),
            esc(&b.kind),
            site_model::encode(&b.group),
            esc(&match b.kind.as_str() {
                "clock" => format!("made of {}", site_model::html::human_blocks(&b.group)),
                "door" => {
                    let w = site_model::html::door_category(&b.group);
                    if w.is_empty() { b.group.clone() } else { w.to_string() }
                }
                _ => b.group.clone(),
            })
        ));
    }
    body.push_str(&page_heading(b, design_text));
    let badges = site_model::html::badges(b);
    if !badges.is_empty() {
        body.push_str(&format!("<p class=\"badges\">{}</p>\n", badges.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect::<String>()));
    }
    // the redstone first (vjeux: "Put the redstone before the explanation"): the live view and
    // its controls, a tunable group's picker under it, then why build it and how to try it; the
    // jargon, our test and the measurements fold into one "Details" (vjeux 13:06Z)
    body.push_str(&viewer(prefix, b));
    let picker = group_picker(site, b, prefix);
    // the picker swaps versions in place (pkg/redstonepedia.js), keeping the viewer's camera
    let scripts = Scripts { app: scripts.ui && scripts.viewer && !picker.is_empty(), ..scripts };
    if b.circuit.is_some() {
        body.push_str(&picker);
    }
    body.push_str(&crate::plain::intro_box(b, scripts.viewer));
    if let Some(c) = &b.clock {
        body.push_str(&clock_timeline(design_text, c));
    }
    if let Some(v) = b.door.as_ref().and_then(|d| d.video.as_ref()) {
        body.push_str(&format!(
            "<section><h2>Video (real game)</h2><video class=\"video\" controls preload=\"none\" src=\"{}\"></video><p><a href=\"{}\">download the video</a></p></section>\n",
            esc(v),
            esc(v)
        ));
    }
    body.push_str(&format!(
        "<section><h2>Build it</h2><p><a class=\"button\" href=\"{prefix}{}\" download>Download .litematic</a> <a class=\"button alt\" href=\"{prefix}{}\" download>Download the design as text</a></p>\n<p class=\"hint\">The .litematic opens in <a href=\"https://github.com/maruohon/litematica\">Litematica</a> (Minecraft 26.3): every block of the design in one region.{} The text file is the same build as a plain-text design, for this project's own tools.</p>\n<p>Blocks used: {}</p></section>\n",
        esc(&crate::generate::litematic_path(&b.design)),
        esc(&b.design),
        if b.kind == "door" || b.circuit.is_some() || b.clock.is_some() { " Verified in the real game." } else { "" },
        chips(&b.kinds, prefix, "with")
    ));
    body.push_str("<details class=\"more\"><summary>Details: how it works in depth, our test in the real game, measurements and properties</summary>\n");
    if let Some(c) = &b.circuit {
        body.push_str(&format!("<section class=\"what\"><h2>In depth</h2>{}</section>\n", crate::explain::what_it_does(c)));
        body.push_str(&circuit_sections(c, scripts.viewer));
    }
    let notes = crate::explain::caption_plain(&b.caption);
    if !notes.is_empty() {
        body.push_str(&format!("<section><h2>Notes from its designer</h2><p>{notes}</p></section>\n"));
    }
    body.push_str("<section><h2>Properties</h2><table class=\"props\">\n");
    body.push_str(&properties(site, b, prefix, design_text));
    body.push_str("</table></section>\n");
    if let Some(d) = &b.door {
        if !d.props.is_empty() {
            body.push_str("<div class=\"allprops\"><details><summary>Every check of the community regulation</summary><table class=\"props small\">\n");
            for (k, v) in &d.props {
                if let Some((label, value)) = crate::explain::door_prop(k, v) {
                    body.push_str(&row(&label, &esc(&value)));
                }
            }
            body.push_str("</table></details></div>\n");
        }
    }
    body.push_str(&format!(
        "<section><h2>The design file</h2><p><a href=\"{REPO_URL}/blob/main/{}\">on GitHub</a></p><details><summary>Design file ({} lines)</summary><pre class=\"design\">{}</pre></details></section>\n",
        esc(repo_path),
        design_text.lines().count(),
        esc(design_text)
    ));
    body.push_str("</details>\n");
    if b.circuit.is_some() {
        body.push_str(&layouts(site, b, prefix));
    }
    for (title, list) in similar(site, b) {
        if list.is_empty() {
            continue;
        }
        // like the index's tabs: no kind line every card repeats, the chips most share said once
        // (critique: "Other sculk doors" cards each repeated the kicker and four badges)
        let shared = site_model::html::common_chips(&list);
        let all_same = list.iter().all(|o| site_model::html::props(o).kind == site_model::html::props(list[0]).kind);
        let cards: Vec<String> = list.iter().map(|o| site_model::html::build_card_opts(o, prefix, !all_same, &shared)).collect();
        let most = if shared.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = shared.iter().map(|c| site_model::html::chip_plural(c)).collect();
            let words = match parts.len() {
                1 => parts[0].clone(),
                n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
            };
            let all = site_model::html::shared_chips(&list);
            format!("<p class=\"hint\">{} {}.</p>", if shared.iter().all(|c| all.contains(c)) { "All" } else { "Most" }, esc(&words))
        };
        body.push_str(&format!("<section><h2>{}</h2>\n{most}{}</section>\n", esc(&title), grid(&cards)));
    }
    let p = site_model::html::props(b);
    let desc = format!("{}: {}. {}", p.kind, p.title, b.caption.chars().take(160).collect::<String>());
    shell_full(&page_title(b), &desc, prefix, &body, scripts, site, b.picture.as_deref(), Some(&b.kind))
}

/// A build page's `<title>`: its kind and properties first, the design's name after
/// (`Piston extender: N = 3 · 19 blocks (pext-n3-flat-cascade19)`).
pub fn page_title(b: &Build) -> String {
    let p = site_model::html::props(b);
    format!("{}: {}", p.kind, p.title)
}

/// A build page's heading (vjeux 12:09Z: lead with the properties; 13:06Z: plain words; 13:11Z:
/// no design id): its kind in words with the community's name (small, above), the property line
/// as the title, what it does in one plain sentence, then the secondary properties. The design's
/// name is only in the collapsed details, the URL and the downloads.
fn page_heading(b: &Build, design_text: &str) -> String {
    let p = site_model::html::props(b);
    let kind = match &b.circuit {
        Some(_) if b.group == "Sand elevator" => "Sand elevator: lifts a sand block and lets it fall back (gravity piston extender)".to_string(),
        Some(c) => crate::explain::kind_title(&c.kind),
        None => p.kind.clone(),
    };
    let what = match (&b.circuit, b.kind.as_str()) {
        (Some(c), _) if b.group == "Sand elevator" => format!("Turn the input on and a block of sand is lifted {} blocks up; turn it off and it falls back into place.", c.kind.split_whitespace().nth(1).unwrap_or("2")),
        (Some(c), _) => crate::plain::one_liner(c),
        (None, "door") => crate::plain::door(b).0,
        (None, "clock") => crate::plain::clock(b).0,
        _ => String::new(),
    };
    let mut warn = crate::plain::warning(b).map(|w| format!("<p class=\"warn\">{}</p>\n", esc(&w))).unwrap_or_default();
    // what it needs where you build it, in the opening lines too (critique, real-world lens)
    let needs = crate::plain::needs(b, design_text);
    if !needs.is_empty() {
        warn.push_str(&format!("<p class=\"needs\">{}</p>\n", esc(&needs.join(" "))));
    }
    // why build it, on the first screen (critique: a phone's first screen said what but not why)
    let why = crate::plain::why_any(b);
    // the secondary line without what the opening sentence already says (critique: "subtitles
    // restate the opening sentence")
    let numbers = |s: &str| -> Vec<String> { s.split(|c: char| !(c.is_ascii_digit() || c == '.')).map(|t| t.trim_matches('.').to_string()).filter(|t| !t.is_empty()).collect() };
    let said = numbers(&what);
    let lede: Vec<&str> = p.sub.split(" · ").filter(|part| {
        let n = numbers(part);
        n.is_empty() || !n.iter().all(|x| said.contains(x))
    }).collect();
    let lede = lede.join(" · ");
    format!(
        "<p class=\"kindline\">{}</p>\n<h1>{}</h1>\n{}{}{warn}{}",
        esc(&kind),
        // (each number kept with its unit: critique, "2 / blocks" wrapped in the title)
        esc(&site_model::html::keep_units(&p.title)),
        if what.is_empty() { String::new() } else { format!("<p class=\"oneliner\">{}</p>\n", esc(&what)) },
        if why.is_empty() { String::new() } else { format!("<p class=\"why\"><b>Why build it:</b> {}</p>\n", esc(&why)) },
        if lede.is_empty() { String::new() } else { format!("<p class=\"lede\">{}</p>\n", esc(&site_model::html::keep_units(&lede))) }
    )
}

fn link_build(site: &Site, id: &str, prefix: &str) -> String {
    match site.builds.iter().find(|b| b.id == id) {
        Some(b) => format!("<a href=\"{prefix}{}\">{}</a>", esc(&b.page), esc(&site_model::html::props(b).title)),
        None => esc(id.rsplit('/').next().unwrap_or(id)),
    }
}

/// The properties table rows of a build.
fn properties(site: &Site, b: &Build, prefix: &str, design_text: &str) -> String {
    let mut s = String::new();
    if let Some(d) = &b.door {
        s.push_str(&row("Size (community volume)", &esc(&match (b.volume, &b.dims) {
            (Some(v), Some(dm)) => format!("{v}b ({dm})"),
            (Some(v), None) => format!("{v}b"),
            _ => "not measured".into(),
        })));
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        s.push_str(&row("Category", &esc(&b.group)));
        if let (Some(o), Some(c)) = (d.open, d.close) {
            s.push_str(&row("Open / close", &format!("{o} / {c} gt ({} / {}), worst of 3 cycles after the lever flip", real_time(o), real_time(c))));
        }
        s.push_str(&row("Rotations", &esc(if d.rotation.is_empty() { "unknown" } else { &d.rotation })));
        s.push_str(&row("No slime or honey", yes_no(b.flags.iter().any(|f| f == "slime-free"))));
        s.push_str(&row("Flat (one block thick)", yes_no(b.flags.iter().any(|f| f == "flat"))));
        s.push_str(&row("Seamless", &esc(&if d.seamless.is_empty() { "no".to_string() } else { site_model::html::seamless_words(&d.seamless) })));
        if !d.frame.is_empty() {
            s.push_str(&row("Frame when open", &esc(&d.frame)));
        }
        s.push_str(&row("Hallway-compatible", yes_no(b.flags.iter().any(|f| f == "hallway"))));
        s.push_str(&row("Record-eligible (community regulation)", yes_no(b.flags.iter().any(|f| f == "eligible"))));
        if !d.boards.is_empty() {
            s.push_str(&row("Boards", &esc(&d.boards.join(", "))));
        }
        if !d.leads.is_empty() {
            s.push_str(&row("Best door for", &esc(&d.leads.iter().map(|l| site_model::html::lead_words(l)).collect::<Vec<_>>().join("; "))));
        }
        if let Some(r) = d.record {
            s.push_str(&row("Record progression", &format!("took the solid record at {r} blocks placed")));
        }
        if let Some(v) = &d.buildable {
            s.push_str(&row("Buildable in survival", &esc(v)));
        }
        if d.entities > 0 {
            s.push_str(&row("Entities", &d.entities.to_string()));
        }
        if let Some(t) = &d.track {
            s.push_str(&row("Found by", &format!("agent track <a href=\"{REPO_URL}/blob/main/designs/piston-door-3x3/tracks/{}/NOTES.md\">{}</a>", esc(t), esc(t))));
        }
        if !d.archived.is_empty() {
            s.push_str(&row("Archived", &esc(&d.archived)));
        }
        if let Some(v) = &d.video {
            s.push_str(&row("Video", &format!("<a href=\"{}\">watch (real game)</a>", esc(v))));
        }
        if !d.fill_of.is_empty() {
            s.push_str(&row("Filled from", &format!("<a href=\"{REPO_URL}/blob/main/{}\">{}</a> (<code>door fill</code>: the same mechanism, other blocks in its free cells)", esc(&d.fill_of), esc(d.fill_of.rsplit('/').next().unwrap_or(&d.fill_of)))));
        }
        if let Some(h) = &d.host {
            s.push_str(&row("Another version of", &format!("{} (the same size and speed, shown as one card)", link_build(site, h, prefix))));
        }
        if !d.variants.is_empty() {
            // the versions read alike (the same numbers): numbered, with their block count
            let shown: Vec<String> = d
                .variants
                .iter()
                .take(40)
                .enumerate()
                .map(|(k, v)| match site.builds.iter().find(|o| o.id == *v) {
                    Some(o) => format!("<a href=\"{prefix}{}\">version {} ({})</a>", esc(&o.page), k + 2, site_model::html::blocks_word(o.blocks)),
                    None => link_build(site, v, prefix),
                })
                .collect();
            let more = if d.variants.len() > 40 { format!(" and {} more", d.variants.len() - 40) } else { String::new() };
            s.push_str(&row(&format!("Other versions of this door ({})", d.variants.len()), &format!("{}{more}", shown.join(", "))));
        }
    }
    if let Some(c) = &b.clock {
        s.push_str(&row("Pulses every", &esc(&crate::plain::ticks(&format!("{} gt", c.every())))));
        if c.pulse_every.is_some() {
            s.push_str(&row("Full cycle (its parts repeat)", &esc(&crate::plain::ticks(&format!("{} gt", c.period)))));
        }
        s.push_str(&row("Made of (the blocks that change state)", &esc(&site_model::html::human_blocks(&b.group))));
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        if let (Some(v), Some(dm)) = (b.volume, &b.dims) {
            s.push_str(&row("Box", &format!("{} ({v} blocks of space)", esc(&dm.replace('x', "×")))));
        }
        s.push_str(&row(
            "Output",
            &format!(
                "on for {} of every {} ticks{}",
                c.on,
                c.period,
                if c.edges > 1 { format!(", {} pulses per cycle", c.edges) } else { String::new() }
            ),
        ));
        s.push_str(&row("Settles after", &esc(&crate::plain::ticks(&format!("{} gt", c.warmup)))));
        s.push_str(&row("To start it", &esc(&start_words(&c.start, design_text))));
        s.push_str(&row("Works in any direction, anywhere", &esc(&crate::explain::direction_words(&c.robustness))));
        s.push_str(&row("Mirror image", if c.chiral { "a different design (listed separately)" } else { "the same design" }));
        if let Some(card) = &c.card {
            if let Some(cd) = site.cards.iter().find(|x| &x.id == card) {
                s.push_str(&row("Extendable family", &format!("<a href=\"{prefix}{}\">{}</a> ({}){}", esc(&cd.page), esc(&cd.title), esc(&cd.heading), c.n.map(|n| format!(", n = {n}")).unwrap_or_default())));
            }
        }
        if let Some(h) = &c.host {
            s.push_str(&row("Variant of", &format!("{} (fewer blocks, a subset of its block kinds, same period)", link_build(site, h, prefix))));
        }
        if !c.variants.is_empty() {
            // a wall of variants folds to a count (critique C10: 31 links in a row)
            let links: Vec<String> = c.variants.iter().map(|v| link_build(site, v, prefix)).collect();
            let cell = if links.len() <= 6 { links.join(", ") } else { format!("<details><summary>{} variants</summary>{}</details>", links.len(), links.join(", ")) };
            s.push_str(&row("Variants (bigger, the same period)", &cell));
        }
        if !c.aliases.is_empty() {
            s.push_str(&row("Also found as", &esc(&c.aliases.join(", "))));
        }
        if c.composite {
            s.push_str(&row("Composite", "yes: sub-clocks side by side (a beat clock)"));
        }
    }
    if let Some(c) = &b.circuit {
        let study = crate::explain::study(&c.study).map(|s| s.0).unwrap_or(c.study.as_str());
        s.push_str(&row("Study", &format!("<a href=\"{prefix}circuits/{}/index.html\">{}</a>", esc(&c.study), esc(study))));
        s.push_str(&row("Kind", &esc(&crate::explain::kind_title(&c.kind))));
        // the pictures and the 3D view add the test parts (critique C41: "3 blocks" next to a
        // picture of five), said only where they do (not on flying machines or extenders)
        let own_device = b.flags.iter().any(|f| f == "lever" || f == "button");
        let mut extra: Vec<&str> = Vec::new();
        if !c.inputs.is_empty() && !own_device {
            extra.push("the input levers or buttons");
        }
        if c.output_lamp {
            extra.push("the test lamp");
        }
        let count = if extra.is_empty() { b.blocks.to_string() } else { format!("{} (not counting {} the view adds)", b.blocks, extra.join(" and ")) };
        s.push_str(&row("Blocks placed", &count));
        if let (Some(v), Some(dm)) = (b.volume, &b.dims) {
            s.push_str(&row("Box", &format!("{} ({v} blocks of space)", esc(&dm.replace('x', "×")))));
        }
        if !c.family.is_empty() {
            s.push_str(&row("Made of (the blocks that change state)", &esc(&site_model::html::human_blocks(&c.family))));
        }
        if !c.robustness.is_empty() {
            s.push_str(&row("Works in any direction, anywhere", &esc(&crate::explain::direction_words(&c.robustness))));
        }
        if !c.inputs.is_empty() || !c.outputs.is_empty() || c.kind == "tnt-duper" {
            let ins: Vec<String> = c.inputs.iter().enumerate().map(|(i, n)| format!("{} ({})", n.to_uppercase(), c.devices.get(i).map(String::as_str).unwrap_or("redstone block"))).collect();
            let outs: Vec<String> = c.outputs.iter().map(|o| if o == "out" { "OUT".to_string() } else { o.to_uppercase() }).collect();
            let text = match (ins.is_empty(), outs.is_empty()) {
                (false, false) => format!("{} / {}", ins.join(", "), outs.join(", ")),
                (false, true) => format!("{} (no output: it moves blocks)", ins.join(", ")),
                (true, false) if !c.events.is_empty() => format!("changes at its target cell / {}", outs.join(", ")),
                (true, false) => format!("no input / {}", outs.join(", ")),
                (true, true) => "none: it runs by itself".to_string(),
            };
            s.push_str(&row("Inputs / outputs", &esc(&text)));
        }
        if let Some(h) = &c.host {
            s.push_str(&row("Variant of", &format!("{} (fewer blocks, a subset of its block kinds, same kind and value)", link_build(site, h, prefix))));
        }
        if let Some(h) = &c.layout_of {
            s.push_str(&row("Another layout of", &format!("{} (the same kind, value, block count and block kinds)", link_build(site, h, prefix))));
        }
        if !c.same_build.is_empty() {
            let links: Vec<String> = c.same_build.iter().map(|o| link_build(site, o, prefix)).collect();
            s.push_str(&row("The same build is also", &format!("{} (the same blocks and ports, judged under another kind)", links.join(", "))));
        }
        if !c.card.is_empty() {
            if let Some(cd) = site.cards.iter().find(|x| x.id == c.card) {
                s.push_str(&row("Extendable family", &format!("<a href=\"{prefix}{}\">{}</a> ({}: one design for every n, all generated)", esc(&cd.page), esc(&cd.title), esc(&cd.heading))));
            }
        }
        if !c.archived.is_empty() {
            s.push_str(&row("Archived", &esc(&c.archived)));
        }
    }
    let flags: Vec<String> = b.flags.iter().map(|f| flag_label(f).to_string()).collect();
    if !flags.is_empty() {
        s.push_str(&row("Flags", &esc(&flags.join(", "))));
    }
    // the design's file name: only here, in the collapsed details (vjeux 13:11Z: no ids in lists)
    s.push_str(&row("Design file", &format!("<code>{}</code>", esc(&b.name))));
    s
}

/// A clock's start in words: `kick: last 1,0,0` -> `place the observer at (1, 0, 0) last` (the
/// block read from the design).
fn start_words(start: &str, design_text: &str) -> String {
    let s = start.trim();
    if s.is_empty() || s == "self-starting" || s == "self-start" || s == "self" {
        return "nothing: it starts by itself once built".to_string();
    }
    if let Some(cell) = s.strip_prefix("kick: last ") {
        let p: Vec<i32> = cell.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let block = (p.len() == 3)
            .then(|| mcview::Design::parse(design_text).ok())
            .flatten()
            .and_then(|d| d.blocks.get(&[p[0], p[1], p[2]]).map(|st| st.trim_start_matches("minecraft:").split('[').next().unwrap_or("").replace('_', " ")));
        return match block {
            Some(b) if !b.is_empty() => format!("place the {b} at ({}) last: it starts ticking when that block goes down", cell.replace(',', ", ")),
            _ => format!("place the block at ({}) last", cell.replace(',', ", ")),
        };
    }
    s.replace("kick", "a kick")
}

/// Related builds: same period and same family (clocks), same category and the nearest sizes
/// (doors), at most 12 each.
fn similar<'a>(site: &'a Site, b: &Build) -> Vec<(String, Vec<&'a Build>)> {
    let mut out = Vec::new();
    let others = || site.builds.iter().filter(move |o| o.id != b.id && o.kind == b.kind);
    if let Some(c) = &b.clock {
        // walk the axis first (vjeux on circuits: "show [them] of different sizes, not others of
        // size 1"): the smallest clock of each nearby period, then the same period's other
        // clocks, then the family's own other periods
        let shown = |o: &&Build| o.clock.as_ref().is_some_and(|oc| oc.host.is_none() && !oc.instance);
        let every = c.every();
        let smallest_per_period = |pool: Vec<&'a Build>| -> Vec<&'a Build> {
            let mut best: BTreeMap<u32, &Build> = BTreeMap::new();
            for o in pool {
                let p = o.clock.as_ref().map(|x| x.every()).unwrap_or(0);
                if p == every {
                    continue;
                }
                let better = |cur: &&Build| (o.blocks, o.volume, &o.name) < (cur.blocks, cur.volume, &cur.name);
                if best.get(&p).is_none_or(better) {
                    best.insert(p, o);
                }
            }
            let mut v: Vec<&Build> = best.into_values().collect();
            // the 8 nearest periods, shown in order of period (critique: "5, 7, 4, 8 ticks")
            v.sort_by_key(|o| {
                let p = o.clock.as_ref().map(|x| x.every()).unwrap_or(0);
                ((p as i64 - every as i64).abs(), p)
            });
            v.truncate(8);
            v.sort_by_key(|o| o.clock.as_ref().map(|x| x.every()).unwrap_or(0));
            v
        };
        out.push(("Nearby periods (the smallest clock of each)".to_string(), smallest_per_period(others().filter(shown).collect())));
        let mut same: Vec<&Build> = others().filter(shown).filter(|o| o.clock.as_ref().is_some_and(|oc| oc.every() == every)).collect();
        same.sort_by_key(|o| (o.blocks, o.volume, o.name.clone()));
        same.truncate(8);
        out.push((format!("Other clocks pulsing every {}", crate::plain::ticks(&format!("{every} gt"))), same));
        out.push((format!("Also made of {}, other periods", site_model::html::human_blocks(&b.group)), smallest_per_period(others().filter(shown).filter(|o| o.group == b.group).collect())));
    } else if b.kind == "door" {
        let v = b.volume.unwrap_or(u32::MAX) as i64;
        // not its own fill variants (the same stats again: critique) nor any folded variant
        let mut near: Vec<&Build> = others().filter(|o| o.group == b.group && !o.flags.iter().any(|f| f == "variant") && o.door.as_ref().is_none_or(|d| d.host.as_deref() != Some(b.id.as_str()))).collect();
        near.sort_by_key(|o| ((o.volume.unwrap_or(u32::MAX) as i64 - v).abs(), o.blocks, o.name.clone()));
        // one card per box and block count (critique: four "7×8×1 · 40 blocks" in a row), none
        // reading like this page's own door
        let mut seen: Vec<(Option<String>, u32)> = vec![(b.dims.clone(), b.blocks)];
        near.retain(|o| {
            let k = (o.dims.clone(), o.blocks);
            if seen.contains(&k) {
                false
            } else {
                seen.push(k);
                true
            }
        });
        near.truncate(12);
        out.push((format!("Other {} doors of a similar size", b.group), near));
    } else if let Some(c) = &b.circuit {
        out.push(circuit_axis_walk(site, b, c));
        // counters and clock slowers do the same job (critique C18): each links the other's
        // nearest counts
        let other = match base_kind(&c.kind) {
            "counter" => Some(("divider", "Clock slowers that do the same on a clock (the smallest of each)")),
            "divider" => Some(("counter", "Counters that do the same with any input (the smallest of each)")),
            _ => None,
        };
        if let Some((kind, title)) = other {
            let mut best: BTreeMap<i64, &Build> = BTreeMap::new();
            for o in &site.builds {
                let Some(oc) = &o.circuit else { continue };
                if base_kind(&oc.kind) != kind || oc.host.is_some() || oc.layout_of.is_some() {
                    continue;
                }
                let Some(v) = oc.value else { continue };
                if best.get(&v).is_none_or(|cur| (o.blocks, &o.name) < (cur.blocks, &cur.name)) {
                    best.insert(v, o);
                }
            }
            let mut list: Vec<&Build> = best.into_values().collect();
            let mine = c.value.unwrap_or(0);
            list.sort_by_key(|o| {
                let v = o.circuit.as_ref().and_then(|x| x.value).unwrap_or(0);
                ((v - mine).abs(), v)
            });
            list.truncate(6);
            list.sort_by_key(|o| o.circuit.as_ref().and_then(|x| x.value).unwrap_or(0));
            if !list.is_empty() {
                out.push((title.to_string(), list));
            }
        }
    }
    out
}

/// A circuit kind's family word: `counter 4` -> `counter`, `gate xor` -> `gate`.
fn base_kind(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or(kind)
}

/// The circuits along this one's study axis (vjeux on pext-n1-sticky: "show extenders of
/// different sizes, not others of size 1"): the smallest design of each OTHER value of the axis
/// (an extender's reach, a counter's or a delay line's N, a pulse circuit's L, a gate's function, a
/// T flip-flop's mechanism, a flying machine's direction and speed; latches: the other kinds of
/// their study), nearest value first. Same-value designs are on the page already (other layouts,
/// variants), and so are this design's own tunable group (the picker).
fn circuit_axis_walk<'a>(site: &'a Site, b: &Build, c: &site_model::Circuit) -> (String, Vec<&'a Build>) {
    let base = base_kind(&c.kind);
    let by_kind = matches!(
        base,
        "counter" | "piston-extender" | "delay" | "gate" | "flying-machine" | "tff" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "burst" | "divider" | "monostable" | "off-delay" | "on-delay" | "pulse-train"
    );
    let key = |xc: &site_model::Circuit| -> String {
        if base_kind(&xc.kind) == "flying-machine" {
            format!("{} {}", xc.direction, xc.axis)
        } else if by_kind {
            xc.axis.clone()
        } else {
            format!("{} {}", xc.kind, xc.axis)
        }
    };
    let mine = key(c);
    let mut best: BTreeMap<String, &Build> = BTreeMap::new();
    for o in &site.builds {
        let Some(oc) = &o.circuit else { continue };
        if o.id == b.id || oc.host.is_some() || oc.layout_of.is_some() {
            continue;
        }
        if !c.tunable.is_empty() && oc.tunable == c.tunable {
            continue;
        }
        let in_scope = if by_kind { base_kind(&oc.kind) == base } else { oc.study == c.study };
        if !in_scope {
            continue;
        }
        let k = key(oc);
        if k == mine {
            continue;
        }
        let better = |cur: &&Build| (o.blocks, o.volume.unwrap_or(u32::MAX), &o.name) < (cur.blocks, cur.volume.unwrap_or(u32::MAX), &cur.name);
        if best.get(&k).is_none_or(better) {
            best.insert(k, o);
        }
    }
    let mut list: Vec<&Build> = best.into_values().collect();
    list.sort_by_key(|o| {
        let oc = o.circuit.as_ref();
        let ov = oc.and_then(|x| x.value);
        let dist = match (c.value, ov) {
            (Some(a), Some(v)) => (a - v).abs(),
            (None, None) => 0,
            _ => i64::MAX,
        };
        let other_direction = oc.is_some_and(|x| x.direction != c.direction);
        (dist, other_direction, ov.unwrap_or(i64::MAX), o.blocks, o.name.clone())
    });
    list.truncate(12);
    let title = match base {
        "piston-extender" => "Piston extenders of other reaches (the smallest of each)".to_string(),
        "counter" => "Counters of other counts (the smallest of each)".to_string(),
        "delay" => "Delays of other lengths (the smallest of each)".to_string(),
        "gate" => "Other logic gates (the smallest of each function)".to_string(),
        "tff" => "Other toggle switch mechanisms (the smallest of each)".to_string(),
        "flying-machine" => "Flying machines of other directions and speeds (the smallest of each)".to_string(),
        "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" => "The same with other pulse lengths (the smallest of each)".to_string(),
        "burst" => "Other numbers of pulses per press (the smallest of each)".to_string(),
        "divider" => "Clock slowers by other factors (the smallest of each)".to_string(),
        "monostable" => "One-shot timers of other lengths (the smallest of each)".to_string(),
        "off-delay" => "Stay-on timers of other lengths (the smallest of each)".to_string(),
        "on-delay" => "Hold-to-activate circuits with other hold times (the smallest of each)".to_string(),
        "pulse-train" => "Rapid fire at other beats (the smallest of each)".to_string(),
        // (the study's title without its parenthesized name: critique, "Other block change
        // detectors (bud switches) (the smallest of each kind)")
        _ => format!("Other {} (the smallest of each kind)", crate::explain::study(&c.study).map(|s| s.0.split(" (").next().unwrap_or(s.0).to_lowercase()).unwrap_or_else(|| c.study.clone())),
    };
    (title, list)
}

/// A tunable group's picker (every member page and the shown design's): one link per member,
/// its value and block count, the current one marked; following one swaps the 3D view, the
/// numbers and the downloads to that member.
fn group_picker(site: &Site, b: &Build, prefix: &str) -> String {
    let Some(c) = &b.circuit else { return String::new() };
    if c.tunable.is_empty() {
        return String::new();
    }
    let mut members: Vec<&Build> = site
        .builds
        .iter()
        .filter(|o| o.circuit.as_ref().is_some_and(|oc| oc.tunable == c.tunable && oc.host.is_none() && oc.layout_of.is_none()))
        .collect();
    members.sort_by_key(|o| (o.circuit.as_ref().and_then(|oc| oc.value).unwrap_or(i64::MAX), o.blocks, o.name.clone()));
    if members.len() < 2 {
        return String::new();
    }
    let span = members.iter().find_map(|o| o.circuit.as_ref().map(|oc| oc.span.clone()).filter(|s| !s.is_empty())).unwrap_or_default();
    let title = {
        let t = c.group_title.trim();
        let mut ch = t.chars();
        match ch.next() {
            Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
            None => String::new(),
        }
    };
    let same_axis = members.iter().all(|m| m.circuit.as_ref().map(|x| &x.axis) == Some(&c.axis));
    // a chip: the member's value in words when the values differ (12 ticks, reach 3), else what
    // sets it apart (stone button, else its box); its second line the block count. No design
    // names (vjeux 13:11Z), no letters (critique P6).
    let chip_text = |o: &Build| -> (String, String) {
        let oc = o.circuit.as_ref();
        let value = oc.map(|oc| site_model::html::value_words(&oc.kind, &oc.axis)).unwrap_or_default();
        let what = oc.map(|oc| oc.variant_label.clone()).unwrap_or_default();
        if same_axis {
            // what sets it apart: its label, else its property line, else its box
            let line = site_model::html::props(o).title;
            let line = line.rsplit_once(" · ").map(|(a, _)| a.to_string()).unwrap_or_default();
            let big = if !what.is_empty() { what } else if !line.is_empty() { line } else { o.dims.as_deref().map(|d| format!("box {}", d.replace('x', "×"))).unwrap_or_default() };
            (big, site_model::html::blocks_word(o.blocks))
        } else {
            (value, site_model::html::blocks_word(o.blocks))
        }
    };
    // two chips that would read the same are numbered (critique: "3 blocks / 3 blocks")
    let mut texts: Vec<(String, String)> = members.iter().map(|o| chip_text(o)).collect();
    let all = texts.clone();
    for (k, t) in texts.iter_mut().enumerate() {
        if t.0.is_empty() || all.iter().filter(|x| **x == *t).count() > 1 {
            t.0 = if t.0.is_empty() { format!("version {}", k + 1) } else { format!("{} · version {}", t.0, k + 1) };
        }
    }
    let chip_text = |o: &Build| -> (String, String) { members.iter().position(|m| m.id == o.id).map(|i| texts[i].clone()).unwrap_or_default() };
    let chips: String = members
        .iter()
        .map(|o| {
            let cur = o.id == b.id;
            let (big, small) = chip_text(o);
            format!(
                "<a class=\"pick{}\" href=\"{prefix}{}\"{}><b>{}</b><span>{}</span></a>",
                if cur { " on" } else { "" },
                esc(&o.page),
                if cur { " aria-current=\"page\"" } else { "" },
                esc(&site_model::html::keep_units(&big)),
                esc(&site_model::html::keep_units(&small))
            )
        })
        .collect();
    let differs = if c.group_differs.is_empty() { "their layout".to_string() } else { c.group_differs.clone() };
    let _ = (&title, &span);
    format!(
        "<section class=\"group\"><h2>Other versions <span class=\"count\">{}</span></h2><p class=\"hint\">The same mechanism; the versions differ only in {}. Pick one: the 3D view (same camera), the numbers and the downloads follow.</p><nav class=\"picker\" aria-label=\"versions\">{chips}</nav></section>\n",
        esc(&if same_axis || span.is_empty() { format!("{} versions", members.len()) } else { site_model::html::span_words(&span) }),
        esc(&differs)
    )
}

/// A clock's timeline (vjeux 13:46Z: "Why is there no judge timeline for clocks?"): its output
/// over its start-up and three periods (two when long), simulated from the design at build time
/// the way the 3D view runs it; the start-up shaded, each period marked. Empty when the browser's
/// simulator cannot run the design (a boat, a minecart).
fn clock_timeline(design_text: &str, c: &site_model::ClockInfo) -> String {
    match clock_wave(design_text, c) {
        Ok(html) => html,
        // never silently missing (the coordinator): say why
        Err(why) => format!("<section><h2>Its output over time</h2><p class=\"hint\">Not simulated: {}.</p></section>\n", esc(&why)),
    }
}

fn clock_wave(design_text: &str, c: &site_model::ClockInfo) -> Result<String, String> {
    let mut s = mcview::session::Session::new(design_text, &mcview::session::Options::default())?;
    if let Some(why) = &s.unsimulated {
        return Err(crate::explain::plain_reason(why));
    }
    if !s.simulated() {
        return Err("the browser's simulator does not run this design".into());
    }
    if c.period == 0 {
        return Err("no period measured".into());
    }
    let periods = if c.period > 200 { 2 } else { 3 };
    let warm = c.warmup.min(2000);
    let total = (warm + periods * c.period).min(4000);
    // the declared output cell, else (older clocks) the block that turns on most often after the
    // start-up: the clock judge's own rule for a census clock's output
    let declared = s.output_on().is_some();
    let cells: Vec<mcview::Pos> = if declared { Vec::new() } else { s.design.blocks.keys().copied().collect() };
    let mut levels: Vec<u8> = Vec::with_capacity(total as usize);
    let mut per_cell: Vec<Vec<bool>> = vec![Vec::with_capacity(total as usize); cells.len()];
    for _ in 0..total {
        if declared {
            levels.push(if s.output_on() == Some(true) { 15 } else { 0 });
        } else {
            for (k, p) in cells.iter().enumerate() {
                per_cell[k].push(mcview::session::active(&s.world.get(mcview::session::bp(*p))));
            }
        }
        s.step();
    }
    let mut picked = String::new();
    if !declared {
        let rises = |v: &Vec<bool>| (warm as usize + 1..v.len()).filter(|&t| v[t] && !v[t - 1]).count();
        let best = (0..cells.len()).max_by_key(|&k| (rises(&per_cell[k]), std::cmp::Reverse(k)));
        match best {
            Some(k) if rises(&per_cell[k]) > 0 => {
                levels = per_cell[k].iter().map(|on| if *on { 15 } else { 0 }).collect();
                let block = s.design.blocks.get(&cells[k]).map(|st| st.trim_start_matches("minecraft:").split('[').next().unwrap_or("").replace('_', " ")).unwrap_or_default();
                picked = format!(" (the design names no output, so this is its {block} that turns on most often)");
            }
            _ => return Err("no block of it turns on and off in the browser's simulation".into()),
        }
    }
    if levels.iter().all(|l| *l == 0) {
        return Err(format!("its output stayed off for the first {total} ticks in the browser's simulation"));
    }
    let mut phases: Vec<(u32, u32, String)> = Vec::new();
    if warm > 0 {
        phases.push((0, warm, "starting up".to_string()));
    }
    // every cycle labelled the same way (critique: "period 2", "period 3" but no "period 1")
    let mut t = warm;
    let mut k = 1;
    while t + c.period <= total {
        phases.push((t, t + c.period, format!("cycle {k}")));
        t += c.period;
        k += 1;
    }
    let wave = site_model::Wave { ticks: total, rows: vec![site_model::WaveRow::from_levels("out", false, &levels)], phases };
    let on = if c.pulse_every.is_some() {
        format!(": its output pulses every {}, {} times in each cycle", crate::plain::ticks(&format!("{} gt", c.every())), c.edges)
    } else if c.on > 0 && c.edges <= 1 {
        format!(", on for {} of them", crate::plain::ticks(&format!("{} gt", c.on)))
    } else if c.edges > 1 {
        format!(", {} pulses in each", c.edges)
    } else {
        String::new()
    };
    // the grey start-up is said only when there is one (critique: "no grey is drawn")
    let start = if warm > 0 { format!("the grey part is the clock starting up ({}), then ", crate::plain::ticks(&format!("{warm} gt"))) } else { String::new() };
    Ok(format!(
        "<section><h2>Its output over time</h2><p class=\"hint\">The output (orange) over the first {total} ticks, simulated like the 3D view runs it{picked}: {start}it repeats every {}{on}. 20 ticks = 1 second.</p><div class=\"wave-wrap fit\">{}</div></section>\n",
        crate::plain::ticks(&format!("{} gt", c.period)),
        crate::explain::wave_svg_fit(&wave, 340.0)
    ))
}

/// The equivalent layouts of a shown design (same kind, value, blocks and block kinds),
/// collapsed.
fn layouts(site: &Site, b: &Build, prefix: &str) -> String {
    let list: Vec<&Build> = site.builds.iter().filter(|o| o.circuit.as_ref().is_some_and(|c| c.layout_of.as_deref() == Some(b.id.as_str()))).collect();
    if list.is_empty() {
        return String::new();
    }
    let cards: Vec<String> = list.iter().map(|o| build_card(o, prefix)).collect();
    format!(
        "<section><details class=\"layouts\"><summary>Also {} other layout{} (the same kind, value, block count and block kinds)</summary>{}</details></section>\n",
        list.len(),
        if list.len() == 1 { "" } else { "s" },
        grid(&cards)
    )
}

/// The page of an extendable family card: its formula, its two smallest members, its families
/// and every member n with its period, blocks and status.
pub fn card_page(site: &Site, card: &Card, members: &[&Member], scripts: Scripts) -> String {
    let prefix = "../";
    let circuit = card.kind == "circuit";
    let mut body = String::new();
    if circuit {
        let (study_title, _) = crate::explain::study(&card.study).unwrap_or(("Circuits", ""));
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › <a href=\"{prefix}circuits/{}/index.html\">{}</a> › Extendable family</p>\n",
            esc(&card.study),
            esc(study_title)
        ));
    } else {
        // (the families index is gone: vjeux 13:35Z) the Clocks tab
        body.push_str(&format!("<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}index.html#kind=clock\">Clocks</a></p>\n"));
    }
    let base = card.families.first().and_then(|f| site.builds.iter().find(|b| b.id == f.build));
    // a title in words, the range it covers and its step (critique C14 / P10 / C51: the H1 was a
    // formula, "n" and "N" mixed); the formula goes to a details line
    let mut sorted: Vec<&&Member> = members.iter().filter(|m| card.families.first().is_some_and(|f| m.family == f.id)).collect();
    sorted.sort_by_key(|m| m.n);
    let value_of = |m: &Member| -> i64 { if m.axis.is_empty() { m.period as i64 } else { m.axis.split_whitespace().find_map(|w| w.parse::<i64>().ok()).unwrap_or(0) } };
    let steps: Vec<i64> = sorted.windows(2).map(|w| value_of(w[1]) - value_of(w[0])).collect();
    let step = steps.first().copied().filter(|s| *s != 0 && steps.iter().all(|x| x == s));
    let (lo_m, hi_m) = (sorted.first().copied(), sorted.last().copied());
    let kind_of_base = base.and_then(|b| b.circuit.as_ref()).map(|c| c.kind.clone()).unwrap_or_default();
    let what = match base.and_then(|b| b.circuit.as_ref()) {
        Some(c) => site_model::html::kind_words(&c.kind),
        None => format!("Clock made of {}", base.map(|b| site_model::html::human_blocks(&b.group)).unwrap_or_default()),
    };
    let title = match (lo_m, hi_m) {
        (Some(a), Some(b)) if !circuit => format!("A pulse every {} to {} ticks ({} to {})", a.period, b.period, site_model::real_time(a.period), site_model::real_time(b.period)),
        (Some(a), Some(b)) => format!("Every size, from {} to {}", member_value(a, &kind_of_base), member_value(b, &kind_of_base)),
        _ => card.title.clone(),
    };
    body.push_str(&format!("<p class=\"kindline\">{}</p>\n<h1>{}</h1>\n", esc(&what), esc(&site_model::html::keep_units(&title))));
    let mut lede = format!("{} sizes, all ready to download", sorted.len());
    if let Some(s) = step {
        let more = match kind_of_base.split_whitespace().next().unwrap_or("") {
            "" => format!("each size pulses {s} ticks slower"),
            "piston-extender" => format!("each size reaches {s} block{} further", if s == 1 { "" } else { "s" }),
            "counter" | "divider" => format!("each size counts {s} more"),
            _ if lo_m.is_some_and(|m| m.axis.contains(" gt")) => format!("each size is {s} ticks longer"),
            _ => format!("each size adds {s}"),
        };
        lede.push_str(&format!("; {more}"));
    }
    body.push_str(&format!("<p class=\"oneliner\">{}.</p>\n", esc(&lede)));
    if circuit {
        if let Some(c) = base.and_then(|b| b.circuit.as_ref()) {
            body.push_str(&format!("<p class=\"why\"><b>What it does:</b> {}. Pick the size you need below.</p>\n", esc(&first_upper(&crate::explain::kind_summary(&c.kind)))));
        }
    } else {
        body.push_str("<p class=\"why\"><b>What it does:</b> a clock you can make slower one step at a time: pick the period you need below.</p>\n");
    }
    if let Some(f) = card.families.first() {
        // one live view of the members (a row below swaps its design in place, same camera)
        if let Some(m) = members.iter().find(|m| m.family == f.id && !m.design.is_empty()).or_else(|| members.iter().find(|m| !m.design.is_empty())) {
            let kind = if circuit { "circuit" } else { "clock" };
            body.push_str(&format!(
                "<div class=\"viewer k-{kind}\" id=\"member-view\" data-mcview=\"full\" data-src=\"{prefix}{}\" data-kind=\"{kind}\"{}><div class=\"fallback\"><span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span></div></div>\n<p id=\"member-caption\" class=\"hint\">{}</p>\n",
                esc(&m.design),
                if circuit { "" } else { " data-autoplay" },
                esc(&member_value(m, &kind_of_base)),
                esc(&site_model::html::blocks_word(m.blocks)),
                esc(&member_caption(m, &kind_of_base))
            ));
        }
        body.push_str(&format!("<p><b>Each step adds:</b> {}. Pick a size in the table to see it in the 3D view.</p>\n", esc(&f.rule)));
    }
    if !card.works_with.is_empty() {
        body.push_str(&format!("<p>Works with any one of these {} blocks in place of the {}: {}.</p>\n", card.works_with.len(), esc(&site_model::html::human_blocks(&card.works_with[0])), esc(&card.works_with.iter().map(|w| site_model::html::human_blocks(w)).collect::<Vec<_>>().join(", "))));
    }
    let value_head = if circuit { "size" } else { "pulses every" };
    if card.families.len() > 1 {
        body.push_str(&format!("<section><h2>The designs in this family</h2><table class=\"list\"><thead><tr><th>design</th><th>{}</th><th>blocks</th><th>sizes</th><th>sizes tested in the game</th></tr></thead><tbody>\n", if circuit { "sizes from" } else { "periods from" }));
        for f in &card.families {
            let extra = if f.extra.is_empty() { String::new() } else { format!(" (+{})", esc(&f.extra.join(", +"))) };
            let fm: Vec<&&Member> = members.iter().filter(|m| m.family == f.id).collect();
            let range = match (fm.iter().min_by_key(|m| m.n), fm.iter().max_by_key(|m| m.n)) {
                (Some(a), Some(b)) => format!("{} to {}", member_value(a, &kind_of_base), member_value(b, &kind_of_base)),
                _ => "-".into(),
            };
            let tested = fm.iter().filter(|m| m.status == site_model::STATUS_JUDGED).count();
            body.push_str(&format!(
                "<tr><td>{}{extra}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                link_build(site, &f.build, prefix),
                esc(&range),
                esc(&f.blocks_text.replace('n', " per step")),
                fm.len(),
                if tested == 0 { "-".to_string() } else { tested.to_string() }
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    let screen = if circuit { "gives exactly its value in simulation" } else { "runs at exactly its period in simulation" };
    body.push_str(&format!("<section><h2>Every size</h2><p>Each size is generated from the smallest design, one step at a time. <span class=\"badge st-judged\">tested in the game</span> = tested in the real game; <span class=\"badge st-redsim\">simulated</span> = it {screen}, not tested in the game; <span class=\"badge st-formula\">formula only</span> = not checked one by one.</p>\n"));
    body.push_str(&format!("<table class=\"list\"><thead><tr><th>{value_head}</th><th>blocks</th><th>box</th><th>status</th><th>download</th></tr></thead><tbody>\n"));
    for m in members {
        let anchor = site_model::html::member_anchor(m, site);
        // the value is the row's name (no family id or n: vjeux 13:11Z); it swaps the view in place
        let value_cell = if m.design.is_empty() {
            esc(&member_value(m, &kind_of_base))
        } else {
            // (a clock's row: the period alone, its column says "pulses every")
            let shown = if m.axis.is_empty() { format!("{} ticks ({})", m.period, site_model::real_time(m.period)) } else { member_value(m, &kind_of_base) };
            format!("<a class=\"view\" href=\"#{}\" data-src=\"{prefix}{}\" data-caption=\"{}\">{}</a>", esc(&anchor), esc(&m.design), esc(&member_caption(m, &kind_of_base)), esc(&site_model::html::keep_units(&shown)))
        };
        let download = if m.design.is_empty() {
            "-".to_string()
        } else {
            format!("<a href=\"{prefix}{}\" download>.litematic</a> · <a href=\"{prefix}{}\" download>.txt</a>", esc(&crate::generate::litematic_path(&m.design)), esc(&m.design))
        };
        let page_link = if m.build.is_some() { format!(" · <a href=\"{prefix}{}\">page</a>", esc(&member_href(m, site))) } else { String::new() };
        body.push_str(&format!(
            "<tr id=\"{}\"><td>{value_cell}</td><td>{}</td><td>{}</td><td><span class=\"badge st-{}\">{}</span></td><td>{download}{page_link}</td></tr>\n",
            esc(&anchor),
            m.blocks,
            esc(&m.dims.as_deref().unwrap_or("").replace('x', "×")),
            esc(&m.status),
            esc(status_label(&m.status)),
        ));
    }
    body.push_str("</tbody></table></section>\n");
    // the formula, for the curious (critique P10: "keep them in details")
    let formula = if card.note.is_empty() { card.heading.clone() } else { format!("{}; {}", card.heading, card.note) };
    body.push_str(&format!("<details class=\"more\"><summary>The formula behind the sizes</summary><p><code>{}</code></p><p class=\"hint\">n counts the steps from the smallest size.</p></details>\n", esc(&formula)));
    let desc = format!("{what}: {title}, every size ready to download");
    // the member rows swap the view in place (pkg/redstonepedia.js)
    let scripts = Scripts { app: scripts.ui && scripts.viewer, ..scripts };
    shell_full(&format!("{what}: {title}"), &desc, prefix, &body, scripts, site, None, Some(if circuit { "circuit" } else { "clock" }))
}

/// A family member's value: a clock's period, a circuit's axis value, in words.
fn member_value(m: &Member, kind: &str) -> String {
    if m.axis.is_empty() { format!("every {} ticks ({})", m.period, site_model::real_time(m.period)) } else { site_model::html::value_words(kind, &m.axis) }
}

/// The caption under a family page's view: which member it shows.
fn member_caption(m: &Member, kind: &str) -> String {
    format!("Showing: {} · {} · {}", member_value(m, kind), site_model::html::blocks_word(m.blocks), status_label(&m.status))
}

/// index.html: the filtering UI's shell (the UI itself is pkg/redstonepedia.js).
pub fn index_page(site: &Site, scripts: Scripts) -> String {
    // what each tab shows (critique C1: one number per thing, the one a visitor can browse)
    let n = |k: &str| site.run(&site_model::Query::from_fragment(&format!("kind={k}"))).map(|(h, _)| h.len()).unwrap_or(0);
    let data_v = versions().data;
    let data_src = if data_v.is_empty() { "data/site.json".to_string() } else { format!("data/site.json?v={data_v}") };
    let body = format!(
        "<h1 class=\"sr-only\">{SITE_NAME}</h1><p class=\"lede intro\">Redstone builds you can download, each design tested tick by tick in the real game: {} piston doors, {} clocks{}.</p>\n\
         <div id=\"app\" class=\"app\"><aside id=\"filters\" class=\"filters\" data-src=\"{data_src}\"><p class=\"loading\">Loading the filters…</p></aside>\
         <div class=\"results\"><div id=\"summary\" class=\"summary\"></div><div id=\"grid\" class=\"grid\"></div><p class=\"more-wrap\"><button id=\"more\" type=\"button\" hidden>Show more</button></p></div></div>\n\
         <noscript><p class=\"notice\">The filters need JavaScript and WebAssembly. Without them, browse <a href=\"circuits/index.html\">the circuits</a>: every design page works without them.</p></noscript>\n",
        n("door"),
        n("clock"),
        if n("circuit") > 0 { format!(", {} circuits", n("circuit")) } else { String::new() }
    );
    shell(SITE_NAME, "Redstone builds you can download: piston doors, clocks and circuits, each tested in the real game, filterable by their properties", "", &body, Scripts { app: true, ..scripts }, site)
}

/// The circuits of one study folder (`pulse`, `tff`, ...), representatives first.
fn study_builds<'a>(site: &'a Site, folder: &str) -> Vec<&'a Build> {
    let mut v: Vec<&Build> = site.builds.iter().filter(|b| b.page.starts_with(&format!("circuits/{folder}/"))).collect();
    v.sort_by(|a, b| {
        let key = |x: &Build| {
            let c = x.circuit.as_ref();
            // (gates: the most common function first, critique)
            let value = c.and_then(|c| if c.kind.starts_with("gate ") { Some(site_model::html::gate_rank(c.kind.split_whitespace().nth(1).unwrap_or(""))) } else { c.value }).unwrap_or(i64::MAX);
            (c.map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default(), value, x.blocks, x.volume.unwrap_or(0), x.name.clone())
        };
        key(a).cmp(&key(b))
    });
    v
}

/// A circuit's metric (`items/s`, `badges`...), empty when absent.
fn cmetric<'a>(b: &'a Build, k: &str) -> &'a str {
    b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(n, _)| n == k)).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// The leading number of a metric (`2.50` of `2.50`, `200` of `200 TNT/min`).
fn cnum(b: &Build, k: &str) -> Option<f64> {
    cmetric(b, k).split_whitespace().next().and_then(|v| v.parse().ok())
}

/// The design of `v` with the smallest key (ties: by name).
fn pick_min<'a>(v: Vec<&'a Build>, key: &dyn Fn(&Build) -> (i64, i64)) -> Option<&'a Build> {
    v.into_iter().min_by(|a, b| (key(a), &a.name).cmp(&(key(b), &b.name)))
}

/// A variations table's cell: the best design (link, blocks, its key number) or `none yet`.
fn best_cell(b: Option<&Build>, prefix: &str, key: &str, unit: &str) -> String {
    match b {
        Some(b) => {
            // the design's properties, no name (vjeux 13:11Z: no ids in lists)
            let n = cmetric(b, key);
            let p = site_model::html::props(b);
            let extra = if n.is_empty() || p.title.contains(n) { String::new() } else { format!("{}{unit}", esc(n)) };
            format!(
                "<a href=\"{prefix}{}\">{}</a>{}",
                esc(&b.page),
                esc(&p.title),
                if extra.is_empty() { String::new() } else { format!("<br><span class=\"hint\">{extra}</span>") }
            )
        }
        None => "<span class=\"hint\">none yet</span>".to_string(),
    }
}

/// The studies of machines (shulker loaders, TNT dupers, BUD switches): for each variation of
/// the study's VARIATIONS.md that the judge measures, what it means and the best design that has
/// it, computed from the designs' metrics.
fn variations_section<'a>(site: &'a Site, folder: &str, prefix: &str) -> String {
    let all: Vec<&'a Build> = site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.study == folder && c.layout_of.is_none())).collect();
    if all.is_empty() || !matches!(folder, "shulker" | "tnt" | "bud" | "pulsemult") {
        return String::new();
    }
    // a badge with a number (`tileable(9)`: tiles every 9 blocks) is that badge (critique: the
    // tileable row said "none yet" next to three tileable designs)
    let has_badge = |b: &Build, badge: &str| cmetric(b, "badges").split('+').any(|x| x == badge || x.split('(').next() == Some(badge));
    let mut rows = String::new();
    let link = format!("{REPO_URL}/blob/main/designs/circuits/{folder}/VARIATIONS.md");
    let head;
    match folder {
        "shulker" => {
            head = "<tr><th>variation</th><th>what it means</th><th>best loader</th><th>best unloader</th></tr>";
            // the base scenario is 64-stack items: the designs judged on 16-stack or unstackable
            // items as their base (`shulker-loader 16`) have their own value of the study's axis
            let loaders: Vec<&Build> = all.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind == "shulker-loader")).collect();
            let unloaders: Vec<&Build> = all.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind == "shulker-unloader")).collect();
            let speed = |b: &Build| (cnum(b, "items/s").unwrap_or(0.0) * 1000.0) as i64;
            let mut row = |label: &str, meaning: &str, l: Option<&Build>, u: Option<&Build>, l_na: bool, u_na: bool| {
                let na = "<span class=\"hint\">(not for this kind)</span>".to_string();
                rows.push_str(&format!(
                    "<tr><th scope=\"row\">{}</th><td class=\"def\">{}</td><td>{}</td><td>{}</td></tr>\n",
                    esc(label),
                    esc(meaning),
                    if l_na { na.clone() } else { best_cell(l, prefix, "items/s", " items/s") },
                    if u_na { na } else { best_cell(u, prefix, "items/s", " items/s") }
                ));
            };
            row("smallest", "fewest blocks placed (the judge's chests not counted)", pick_min(loaders.clone(), &|b| (b.blocks as i64, -speed(b))), pick_min(unloaders.clone(), &|b| (b.blocks as i64, -speed(b))), false, false);
            row("fastest", "most items per second at the steady rate", pick_min(loaders.clone(), &|b| (-speed(b), b.blocks as i64)), pick_min(unloaders.clone(), &|b| (-speed(b), b.blocks as i64)), false, false);
            for badge in ["16-stack", "unstackable", "mixed", "filter", "tileable", "always-on", "restart", "box-starved", "colours", "flush", "top-up", "drop-free", "combo", "any-box"] {
                let with = |v: &[&'a Build]| -> Option<&'a Build> { pick_min(v.iter().copied().filter(|b| has_badge(b, badge)).collect(), &|b| (b.blocks as i64, -speed(b))) };
                let loader_only = matches!(badge, "mixed" | "filter" | "flush" | "top-up");
                let unloader_only = badge == "any-box";
                row(badge, crate::explain::badge_meaning(badge), with(&loaders), with(&unloaders), unloader_only, loader_only);
            }
        }
        "tnt" => {
            head = "<tr><th>variation</th><th>what it means</th><th>best design</th></tr>";
            let running: Vec<&Build> = all.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind == "tnt-duper")).collect();
            let pulsed: Vec<&Build> = all.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind == "tnt-duper-triggered")).collect();
            let rate = |b: &Build| cnum(b, "rate").unwrap_or(0.0) as i64;
            let mut row = |label: &str, meaning: &str, b: Option<&Build>, key: &str, unit: &str| {
                rows.push_str(&format!("<tr><th scope=\"row\">{}</th><td class=\"def\">{}</td><td>{}</td></tr>\n", esc(label), esc(meaning), best_cell(b, prefix, key, unit)));
            };
            row("smallest, self-running", "fewest blocks of the dupers that run by themselves", pick_min(running.clone(), &|b| (b.blocks as i64, -rate(b))), "rate", "");
            row("fastest", "most lit TNT per minute", pick_min(running.clone(), &|b| (-rate(b), b.blocks as i64)), "rate", "");
            row("smallest, one shot per pulse", "fewest blocks of the dupers that light TNT on an input pulse", pick_min(pulsed.clone(), &|b| (b.blocks as i64, 0)), "per pulse", " TNT per pulse");
        }
        "pulsemult" => {
            head = "<tr><th>variation</th><th>what it means</th><th>best design</th></tr>";
            rows.push_str(&pulsemult_rows(site, &all, prefix));
        }
        _ => {
            head = "<tr><th>variation</th><th>what it means</th><th>best design</th></tr>";
            let detects = |b: &Build, k: &str| cmetric(b, "detects").split_whitespace().any(|x| x == k);
            let latency = |b: &Build| cnum(b, "latency").unwrap_or(999.0) as i64;
            let footprint = |b: &Build| cnum(b, "footprint").unwrap_or(999.0) as i64;
            let mut row = |label: &str, meaning: &str, b: Option<&Build>, key: &str, unit: &str| {
                rows.push_str(&format!("<tr><th scope=\"row\">{}</th><td class=\"def\">{}</td><td>{}</td></tr>\n", esc(label), esc(meaning), best_cell(b, prefix, key, unit)));
            };
            for class in ["place", "remove", "state", "shape", "update"] {
                row(&format!("detects {class}"), crate::explain::badge_meaning(class), pick_min(all.iter().copied().filter(|b| detects(b, class)).collect(), &|b| (b.blocks as i64, latency(b))), "latency", " latency");
            }
            row("detects all five", "every kind of change at the target", pick_min(all.iter().copied().filter(|b| ["place", "remove", "state", "shape", "update"].iter().all(|k| detects(b, k))).collect(), &|b| (b.blocks as i64, latency(b))), "latency", " latency");
            row("fastest", "the shortest latency", pick_min(all.clone(), &|b| (latency(b), b.blocks as i64)), "latency", " latency");
            row("smallest footprint", "the fewest cells around it that must be left alone", pick_min(all.clone(), &|b| (footprint(b), b.blocks as i64)), "footprint", " footprint");
        }
    }
    format!(
        "<section><h2>Variations</h2><p>What builds need from these machines, each tested as its own scenario in the real game, and the best design for each (the smallest that has it). <a href=\"{}\">The full list of variations</a>, with the ones nobody has built yet.</p><div class=\"scroll-x\"><table class=\"list variations\"><thead>{head}</thead><tbody>\n{rows}</tbody></table></div><p class=\"hint phone-only\">Swipe the table sideways to see every column.</p></section>\n",
        esc(&link)
    )
}

/// The kind's first word (`burst` of `burst 2`, `monostable` of `monostable retriggerable`).
fn kind_base(b: &Build) -> &str {
    b.circuit.as_ref().and_then(|c| c.kind.split_whitespace().next()).unwrap_or("")
}

/// The number in the kind (`2` of `burst 2`).
fn kind_n(b: &Build) -> Option<u32> {
    b.circuit.as_ref().and_then(|c| c.kind.split_whitespace().nth(1)).and_then(|n| n.parse().ok())
}

/// A variations row that another study answers: a link to that study's page and the smallest
/// design of the kind there.
fn linked_cell(site: &Site, prefix: &str, study: &str, kind: &str, key: &str, unit: &str) -> String {
    let title = crate::explain::study(study).map(|s| s.0).unwrap_or(study);
    let number = circuit::archive::STUDIES.iter().position(|s| s.0 == study).map(|i| format!(" (study {})", i + 1)).unwrap_or_default();
    let of: Vec<&Build> = site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.study == study && c.layout_of.is_none()) && kind_base(b) == kind).collect();
    let best = pick_min(of, &|b| (b.blocks as i64, 0));
    format!(
        "<span class=\"hint\">linked:</span> <a href=\"{prefix}circuits/{study}/index.html\">{}</a>{}<br>{}",
        esc(title),
        esc(&number),
        if best.is_some() { format!("smallest there: {}", best_cell(best, prefix, key, unit)) } else { String::new() }
    )
}

/// Study 11's variations (designs/circuits/pulsemult/VARIATIONS.md): the best design of each,
/// and rows that point at the earlier studies that answer them.
fn pulsemult_rows<'a>(site: &'a Site, all: &[&'a Build], prefix: &str) -> String {
    let mut rows = String::new();
    let mut row = |label: &str, meaning: &str, cell: String| {
        rows.push_str(&format!("<tr><th scope=\"row\">{}</th><td class=\"def\">{}</td><td>{cell}</td></tr>\n", esc(label), esc(meaning)));
    };
    let of = |k: &str| -> Vec<&'a Build> { all.iter().copied().filter(|b| kind_base(b) == k).collect() };
    let num = |b: &Build, k: &str| cnum(b, k).map(|v| v as i64);
    let lat = |b: &Build| num(b, "latency").or_else(|| num(b, "delay")).unwrap_or(999);
    let values = |v: &[&Build]| -> Vec<u32> {
        let mut n: Vec<u32> = v.iter().filter_map(|b| kind_n(b)).collect();
        n.sort_unstable();
        n.dedup();
        n
    };
    // bursts: one row per N, the re-trigger classes, the fastest repeat
    let bursts = of("burst");
    for n in values(&bursts) {
        let with: Vec<&Build> = bursts.iter().copied().filter(|b| kind_n(b) == Some(n)).collect();
        row(&format!("burst {n}"), &format!("exactly {n} output pulses per input pulse"), best_cell(pick_min(with, &|b| (b.blocks as i64, lat(b))), prefix, "latency", " latency"));
    }
    let consistent: Vec<&Build> = bursts.iter().copied().filter(|b| matches!(cmetric(b, "retrigger"), "ignores" | "adds")).collect();
    row("burst, a second press ignored (or adding a burst)", "the same answer to a second press whenever it comes during the burst", best_cell(pick_min(consistent, &|b| (b.blocks as i64, lat(b))), prefix, "retrigger", ""));
    row("burst, fastest repeat", "keeps up with the fastest presses, each still giving exactly N pulses", best_cell(pick_min(bursts.clone(), &|b| (num(b, "repeat").unwrap_or(999), b.blocks as i64)), prefix, "repeat", " repeat"));
    // dividers
    let divs = of("divider");
    for n in values(&divs) {
        let square: Vec<&Build> = divs.iter().copied().filter(|b| kind_n(b) == Some(n) && cmetric(b, "output") == "square").collect();
        row(&format!("divider {n}, square"), &format!("one output cycle per {n} input pulses, on for exactly half of it"), best_cell(pick_min(square, &|b| (b.blocks as i64, num(b, "min-period").unwrap_or(999))), prefix, "min-period", " fastest clock"));
    }
    row("divider, fastest input clock", "divides the fastest clock (any N)", best_cell(pick_min(divs.clone(), &|b| (num(b, "min-period").unwrap_or(999), b.blocks as i64)), prefix, "min-period", " fastest clock"));
    row("divider N, pulse output", "a short output pulse per N input pulses: every counter of study 5 passes the divider test on running clocks", linked_cell(site, prefix, "memory", "counter", "N", " pulses per cycle"));
    row("divide by 2", "a T flip-flop divides a clock by 2", linked_cell(site, prefix, "tff", "tff", "min-period", " fastest clock"));
    row("×2 frequency multiplier", "one pulse per input edge: on a square clock, two evenly spaced pulses per period", linked_cell(site, prefix, "pulse", "dual-edge", "L", ""));
    // monostables
    let monos = of("monostable");
    let retrig: Vec<&Build> = monos.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind.ends_with(" retriggerable"))).collect();
    row("monostable, retriggerable", "one output pulse of L per press, stretched to L after the last press while presses keep coming", best_cell(pick_min(retrig, &|b| (b.blocks as i64, lat(b))), prefix, "L", ""));
    let non: Vec<&Build> = monos.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind.ends_with("non-retriggerable"))).collect();
    if non.is_empty() {
        row("monostable, non-retriggerable", "one output pulse of L per press, presses during it ignored: fed single pulses it is study 1's rising-edge detector", linked_cell(site, prefix, "pulse", "rising-edge", "L", ""));
    } else {
        row("monostable, non-retriggerable", "one output pulse of L per press, presses during it ignored", best_cell(pick_min(non, &|b| (b.blocks as i64, lat(b))), prefix, "L", ""));
    }
    row("longest output pulse (pulse extender)", "an output at least L long for any input: study 1", linked_cell(site, prefix, "pulse", "pulse-extender", "L", ""));
    row("shortest output pulse (pulse limiter)", "an output at most L long for any input: study 1", linked_cell(site, prefix, "pulse", "pulse-limiter", "L", ""));
    // timers
    let offs = of("off-delay");
    row("off-delay", "the output stays on L after the input turns off", best_cell(pick_min(offs.clone(), &|b| (b.blocks as i64, lat(b))), prefix, "L", ""));
    row("off-delay, longest L", "the longest hold", best_cell(pick_min(offs, &|b| (-num(b, "L").unwrap_or(0), b.blocks as i64)), prefix, "L", ""));
    let ons = of("on-delay");
    row("on-delay (pulse filter)", "the output only once the input has been on for T; shorter inputs give nothing", best_cell(pick_min(ons.clone(), &|b| (b.blocks as i64, lat(b))), prefix, "T", ""));
    row("on-delay, longest T", "the longest threshold", best_cell(pick_min(ons, &|b| (-num(b, "T").unwrap_or(0), b.blocks as i64)), prefix, "T", ""));
    // gated pulse trains
    let trains = of("pulse-train");
    row("pulse train, smallest", "fewest blocks", best_cell(pick_min(trains.clone(), &|b| (b.blocks as i64, num(b, "P").unwrap_or(999))), prefix, "P", " period"));
    row("pulse train, fastest", "the shortest period P", best_cell(pick_min(trains.clone(), &|b| (num(b, "P").unwrap_or(999), b.blocks as i64)), prefix, "P", " period"));
    let tunable = pick_min(trains.iter().copied().filter(|b| b.circuit.as_ref().is_some_and(|c| !c.span.is_empty())).collect(), &|b| (b.blocks as i64, 0));
    row(
        "pulse train, tunable P",
        "one design whose period a repeater sets",
        match tunable {
            Some(b) => format!("{}<br><span class=\"hint\">{}</span>", best_cell(Some(b), prefix, "", ""), esc(&b.circuit.as_ref().map(|c| c.span.clone()).unwrap_or_default())),
            None => best_cell(None, prefix, "", ""),
        },
    );
    row("self-running clocks", "no on switch", format!("<span class=\"hint\">linked:</span> <a href=\"{prefix}index.html#kind=clock\">the Clocks tab</a>"));
    rows
}

/// circuits/<study>/index.html: what the study collects, how designs are ranked, then each kind
/// with its plain explanation and its designs (variants folded under a count).
pub fn study_page(site: &Site, folder: &str, scripts: Scripts) -> String {
    let prefix = "../../";
    let (title, intro) = match crate::explain::study(folder) {
        Some((t, i)) => (t.to_string(), i.to_string()),
        None => (format!("Circuits: {folder}"), String::new()),
    };
    let uses = crate::explain::study_uses(folder);
    let list = study_builds(site, folder);
    // the gates whose designs are being re-tested are named instead of promised (critique: the
    // intro promised AND, XOR... while R7's re-judge held every one of them back)
    let missing = if folder == "gates" {
        let have: Vec<String> = list.iter().filter_map(|b| b.circuit.as_ref()).map(|c| c.kind.split_whitespace().nth(1).unwrap_or("").to_string()).collect();
        let gone: Vec<String> = ["not", "and", "or", "xor", "xnor", "nand", "nor", "imply", "nimply"].iter().filter(|f| !have.iter().any(|h| h == *f)).map(|f| site_model::html::gate_label(f)).collect();
        if gone.is_empty() {
            String::new()
        } else {
            format!(" Not on the site right now: {}. Their designs are being re-tested with real levers and buttons in the game, and come back once they pass.", gone.join(", "))
        }
    } else {
        String::new()
    };
    // what you would use them for first; the designs right after (critique C6 / P5); how we test
    // and rank them folds at the end
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › {}</p>\n<h1>{}</h1>\n<p class=\"lede\">{}{}</p>\n<p><a class=\"button alt\" href=\"{prefix}index.html#kind=circuit&amp;q={}\">Filter them in the index</a></p>\n{}",
        esc(&title),
        esc(&title),
        esc(if uses.is_empty() { &intro } else { uses }),
        esc(&missing),
        site_model::encode(folder),
        // (only where the view adds a test lamp: critique, not on flying machines or extenders)
        if list.iter().any(|b| b.circuit.as_ref().is_some_and(|c| c.output_lamp)) { "<p class=\"hint\">The pictures show each design with its inputs and a test lamp; the block counts leave those out.</p>\n" } else { "" }
    );
    let mut after = String::new();
    // the studies of machines: the best design for each variation (VARIATIONS.md)
    after.push_str(&variations_section(site, folder, prefix));
    // the study's extendable families: one line per card, its range in words (critique P10)
    let cards: Vec<&Card> = site.cards.iter().filter(|c| c.kind == "circuit" && c.study == folder).collect();
    if !cards.is_empty() {
        after.push_str("<section><h2>Designs you can make longer</h2><p>Add one more step to get a longer delay, a longer pulse or a further reach: every size is ready to download from the design's page.</p><table class=\"list\"><thead><tr><th>design</th><th>sizes</th><th>blocks</th></tr></thead><tbody>\n");
        for c in cards {
            let f = c.families.first();
            let base = f.and_then(|f| site.builds.iter().find(|b| b.id == f.build));
            let name = base.map(|b| site_model::html::props(b).title).unwrap_or_else(|| c.title.clone());
            let sizes = f.map(|f| format!("{} sizes", f.n_hi.saturating_sub(f.n_lo) + 1)).unwrap_or_default();
            after.push_str(&format!(
                "<tr><td><a href=\"{prefix}{}\">{}</a></td><td>{}</td><td>{}</td></tr>\n",
                esc(&c.page),
                esc(&name),
                esc(&sizes),
                esc(&f.map(|f| f.blocks_text.clone()).unwrap_or_default())
            ));
        }
        after.push_str("</tbody></table></section>\n");
    }
    if !intro.is_empty() {
        after.push_str(&format!("<details class=\"more\"><summary>How we test and rank them</summary><p>{}</p></details>\n", esc(&intro)));
    }
    // one section per kind family (counter 2..24 together, every gate together)
    let mut kinds: Vec<String> = Vec::new();
    // one section per kind; the basics of a study get their own section at the end (the gates
    // with their own levers: the coordinator's decision (c)) and so do the sand elevators
    let key_of = |b: &Build| -> String {
        let base = b.circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
        match b.circuit.as_ref() {
            Some(c) if c.order % 100 >= 50 => format!("{base}:basics"),
            _ => base,
        }
    };
    let mut ordered: Vec<&&Build> = list.iter().collect();
    ordered.sort_by_key(|b| b.circuit.as_ref().map(|c| c.order % 100).unwrap_or(0));
    for b in ordered {
        let k = key_of(b);
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    for k in &kinds {
        let of: Vec<&&Build> = list.iter().filter(|b| key_of(b) == *k).collect();
        let k = &k.trim_end_matches(":basics").to_string();
        let basics = of.first().and_then(|b| b.circuit.as_ref()).is_some_and(|c| c.order % 100 >= 50);
        let shown = |c: &site_model::Circuit| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty());
        let reps: Vec<&&Build> = of.iter().copied().filter(|b| b.circuit.as_ref().is_none_or(shown)).collect();
        let n_var = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_some())).count();
        let n_lay = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.layout_of.is_some())).count();
        let n_mem = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.tunable.is_empty() && c.span.is_empty())).count();
        let n_fam = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.card_member && c.tunable.is_empty())).count();
        let mut hidden: Vec<String> = Vec::new();
        if n_mem > 0 {
            hidden.push(format!("{n_mem} other version{} of the designs above (each design's page lists them)", if n_mem == 1 { "" } else { "s" }));
        }
        if n_fam > 0 {
            hidden.push(format!("{n_fam} more size{} of the designs you can make longer (on their pages)", if n_fam == 1 { "" } else { "s" }));
        }
        if n_lay > 0 {
            hidden.push(format!("{n_lay} other layout{}", if n_lay == 1 { "" } else { "s" }));
        }
        if n_var > 0 {
            hidden.push(format!("{n_var} bigger variant{} using extra kinds of blocks", if n_var == 1 { "" } else { "s" }));
        }
        // the kind's title: what it does first, the community's name in parentheses (critique P1);
        // a study's basics: their own heading
        let (heading, summary) = match (k.as_str(), basics) {
            ("gate", true) => ("Basics: the simplest gates, with their own levers".to_string(), "the classic first circuits: a stone block, a torch, a couple of levers; handy to learn how the logic works".to_string()),
            ("piston-extender", true) => ("Sand elevator: lifts a sand block and lets it fall back (gravity piston extender)".to_string(), "only for sand or gravel, only upward: a sand elevator, a hidden floor".to_string()),
            _ => (crate::explain::kind_title(k), crate::explain::kind_summary(k)),
        };
        // under the kind's heading the cards drop the kind line, and the chips most of them share
        // are said once above them (critique C43)
        let shown: Vec<&Build> = reps.iter().map(|b| **b).collect();
        let shared = site_model::html::common_chips(&shown);
        let all_share = site_model::html::shared_chips(&shown);
        let cards: Vec<String> = reps.iter().map(|b| site_model::html::build_card_opts(b, prefix, false, &shared)).collect();
        let most = if shared.is_empty() {
            String::new()
        } else {
            // in words: "Most have their own lever and are 1 block wide." (critique)
            let parts: Vec<String> = shared.iter().map(|c| site_model::html::chip_plural(c)).collect();
            let list = match parts.len() {
                1 => parts[0].clone(),
                n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
            };
            format!(" <span class=\"hint\">{} {}.</span>", if shared.iter().all(|c| all_share.contains(c)) { "All" } else { "Most" }, esc(&list))
        };
        // a section whose kind is the study itself (the piston extenders): no second heading and
        // intro under the page's own (critique: the heading and intro repeated the page's)
        let study_title = crate::explain::study(folder).map(|s| s.0.to_lowercase()).unwrap_or_default();
        let same_as_study = !basics && study_title.starts_with(&heading.to_lowercase());
        let head = if same_as_study { if most.is_empty() { String::new() } else { format!("<p>{}</p>", most.trim()) } } else { format!("<h2>{}</h2><p>{}.{most}</p>", esc(&heading), esc(&first_upper(&summary))) };
        body.push_str(&format!(
            "<section>{head}{}{}</section>\n",
            grid(&cards),
            if hidden.is_empty() {
                String::new()
            } else {
                format!("<p class=\"hint\">Also {}: <a href=\"{prefix}index.html#kind=circuit&amp;q={}&amp;variants=1\">show them in the index</a>.</p>", hidden.join(", "), site_model::encode(folder))
            }
        ));
    }
    if list.is_empty() {
        body.push_str("<p>No design archived yet.</p>\n");
    }
    body.push_str(&after);
    shell_full(&title, if uses.is_empty() { &intro } else { uses }, prefix, &body, scripts, site, None, Some("circuit"))
}

fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// circuits/index.html: a compact list of the studies (critique C7: it was 27 phone screens): one
/// card each, its best design's picture, what you would use it for, its count.
pub fn circuits_index(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Circuits</p>\n<h1>Circuits</h1>\n<p class=\"lede\">The classic redstone contraptions, one collection each. Every design was tested in the real game; each page says what it does, why you would build it, and lets you try it in 3D.</p>\n"
    );
    let mut cards: Vec<String> = Vec::new();
    let mut pictured: Vec<String> = Vec::new();
    for (folder, title, _) in crate::explain::STUDIES.iter() {
        let list = study_builds(site, folder);
        // the designs the study page shows (critique C1: tiles added up to more than the tab)
        let shown: Vec<&&Build> = list.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty()))).collect();
        // the picture: a design of 3 blocks or more (a 1-block one is mostly the test lamp's
        // outline), not one another tile already shows, by name or as the same build (critique:
        // the Toggle and Memory tiles showed one picture, counter2-bulb is tff-bulb); memory
        // shows a set/reset memory first
        let key = |b: &Build| b.circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, b.name)).unwrap_or_default();
        let used = |b: &Build| pictured.contains(&key(b)) || b.circuit.as_ref().is_some_and(|c| c.same_build.iter().any(|s| pictured.contains(s)));
        let preferred = if *folder == "memory" { shown.iter().find(|b| !used(b) && b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("rs-latch"))) } else { None };
        // (a study's basics, the lever gates and the sand elevator, never picture it)
        let basic = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.order % 100 == 50);
        let Some(first) = preferred
            .or_else(|| shown.iter().find(|b| b.blocks >= 3 && !used(b) && !basic(b)))
            .or_else(|| shown.iter().find(|b| b.blocks >= 3 && !used(b)))
            .or_else(|| shown.iter().find(|b| !used(b)))
            .or_else(|| shown.first())
        else {
            continue;
        };
        pictured.push(key(first));
        cards.push(format!(
            "<a class=\"card study\" href=\"{prefix}circuits/{folder}/index.html\" aria-label=\"{}\">{}<div class=\"meta\"><div class=\"kind\">{} designs</div><div class=\"title\">{}</div><div class=\"sub\">{}</div></div></a>",
            esc(title),
            site_model::html::thumb(prefix, &first.design, first.trace.as_deref(), first.picture.as_deref(), &first.kind, title, ""),
            shown.len(),
            esc(title),
            esc(crate::explain::study_blurb(folder))
        ));
    }
    body.push_str(&grid(&cards));
    shell_full("Circuits", "The classic redstone contraptions, tested in the real game: pulse shapers, toggle switches, delays, logic gates, memory, piston extenders, flying machines, shulker box loaders, TNT dupers, block change detectors, timers", prefix, &body, scripts, site, None, Some("circuit"))
}

pub fn not_found_page(site: &Site) -> String {
    // GitHub Pages serves this file at any depth: absolute links to the project's root
    let body = "<h1>Not found</h1><p>This page does not exist (any more). Start from <a href=\"/minecraft-tas/index.html\">the index</a>.</p>";
    shell("Not found", "Page not found", "/minecraft-tas/", body, Scripts::default(), site)
}

/// `<folder>/index.html` for the obvious per-kind URLs (`/clocks/`, `/doors/`, `/circuits/`): a
/// redirect to the index filtered on that kind, with a plain link for clients without refresh.
pub fn redirect_page(site: &Site, prefix: &str, title: &str, fragment: &str) -> String {
    let target = format!("{prefix}index.html#{fragment}");
    let body = format!("<h1>{}</h1><p>Opening <a href=\"{}\">{} in the index</a>…</p>", esc(title), esc(&target), esc(title));
    let page = shell(title, &format!("Every {} of REDSTONEPEDIA", title.to_lowercase()), prefix, &body, Scripts::default(), site);
    page.replacen("</head>", &format!("<meta http-equiv=\"refresh\" content=\"0; url={}\">\n</head>", esc(&target)), 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use site_model::{ClockInfo, Door};

    pub fn tiny_site() -> Site {
        let clock = |id: &str, p: u32, blocks: u32| Build {
            id: format!("clocks/{id}"),
            kind: "clock".into(),
            group: "observer".into(),
            name: id.into(),
            page: format!("clocks/{id}.html"),
            design: format!("designs/clocks/{id}.txt"),
            blocks,
            volume: Some(blocks),
            dims: Some("2x1x1".into()),
            kinds: vec!["observer".into()],
            caption: "Two observers face to face <3".into(),
            flags: vec!["robust".into()],
            clock: Some(ClockInfo { period: p, on: 2, edges: 1, ..Default::default() }),
            ..Default::default()
        };
        let door = Build {
            id: "doors/fc2-40b".into(),
            kind: "door".into(),
            group: "solid".into(),
            name: "fc2-40b".into(),
            page: "doors/fc2-40b.html".into(),
            design: "designs/doors/fc2-40b.txt".into(),
            blocks: 40,
            volume: Some(144),
            dims: Some("8x9x2".into()),
            kinds: vec!["observer".into(), "sticky_piston".into()],
            flags: vec!["rotation-safe".into()],
            door: Some(Door { open: Some(14), close: Some(16), rotation: "safe".into(), seamless: "FULL".into(), frame: "12/12".into(), props: vec![("volume".into(), "144".into())], ..Default::default() }),
            ..Default::default()
        };
        Site { version: 1, generated: "2026-10-19T23:00Z".into(), commit: "0123456789abcdef".into(), builds: vec![clock("obs2", 6, 2), clock("obs3", 6, 3), door], ..Default::default() }
    }

    #[test]
    fn build_page_has_the_parts() {
        let s = tiny_site();
        let p = build_page(&s, &s.builds[0], "name: obs2\n", "designs/clocks/archive/obs2.txt", Scripts { viewer: true, app: false, ui: false });
        assert!(p.starts_with("<!DOCTYPE html>"));
        // the kind and the properties lead; the design's name only in the collapsed details
        assert!(p.contains("<title>Clock: every 6 ticks (0.3 s) · 2 blocks · REDSTONEPEDIA</title>"), "{p}");
        let details = p.find("<details class=\"more\">").expect("the details");
        assert!(!p[..details].contains("obs2\n") && !p[..details].contains(">obs2<"), "no design name before the details");
        assert!(p[details..].contains("<code>obs2</code>"), "{p}");
        assert!(p.contains("<p class=\"oneliner\">Its output pulses by itself every 6 ticks (0.3 s), forever."), "{p}");
        assert!(p.contains("data-mcview=\"full\" data-src=\"../designs/clocks/obs2.txt\" data-autoplay data-kind=\"clock\""), "{p}");
        assert!(p.contains("Two observers face to face &lt;3"));
        assert!(p.contains("<a class=\"card\" href=\"../clocks/obs3.html\""), "similar builds link to the other clock of the period");
        assert!(p.contains("import init from \"../viewer/mcview.js?v="), "{p}");
        assert!(p.contains("init({module_or_path: \"../viewer/mcview_bg.wasm?v="), "the wasm by its own versioned URL");
        assert!(!p.contains("pkg/redstonepedia.js"));
        let d = build_page(&s, &s.builds[2], "name: fc2\n", "designs/piston-door-3x3/archive/fc2-40b.txt", Scripts::default());
        assert!(d.contains("<h1>40\u{a0}blocks in a 8×9×2\u{a0}space</h1>"), "{d}");
        assert!(d.contains("14 / 16 gt"));
        assert!(!d.contains("mcview.js"), "no viewer script without the bundle");
    }

    #[test]
    fn index_loads_the_app() {
        let s = tiny_site();
        let p = index_page(&s, Scripts { viewer: true, app: false, ui: false });
        assert!(p.contains("import init from \"./pkg/redstonepedia.js?v="), "{p}");
        assert!(p.contains("id=\"grid\""));
        assert!(p.contains("1 piston doors, 2 clocks"));
    }
}
