//! The static pages: the index (the filtering UI's shell), one page per build, one per
//! extendable family card, the family index, a static list of everything (no script needed),
//! the about page and the 404 page. Every page is plain HTML5 with relative links (the site is a
//! GitHub project page under /minecraft-tas/); viewers are `data-mcview` elements that
//! webrender's bundle (viewer/mcview.js) renders when present.

use site_model::html::{build_card, esc, kind_title, member_href, size_line};
use site_model::{Build, Card, Member, Site, flag_label, gt_time, real_time, status_label};
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
    let tab = |k: &str, label: &str, frag: &str| -> String {
        format!("<a id=\"tab-{k}\" class=\"tab-top{}\" href=\"{prefix}index.html#{frag}\">{label}</a>", if kind == Some(k) { " on" } else { "" })
    };
    let hidden = match kind {
        Some(k) => format!("<input type=\"hidden\" name=\"kind\" value=\"{}\">", esc(k)),
        None => String::new(),
    };
    format!(
        "<header class=\"top\"><a class=\"logo\" href=\"{prefix}index.html\"><span class=\"logo-dot\"></span>{SITE_NAME}</a><nav class=\"tabs-top\" aria-label=\"kinds\">{}{}{}</nav>\
         <form id=\"hsearch\" class=\"hsearch\" action=\"{prefix}index.html\" method=\"get\" role=\"search\">{hidden}<input id=\"hq\" name=\"q\" type=\"search\" placeholder=\"Search\" aria-label=\"Search\" autocomplete=\"off\"></form></header>\n",
        tab("door", "Doors", "kind=door&amp;sort=volume"),
        tab("clock", "Clocks", "kind=clock"),
        tab("circuit", "Circuits", "kind=circuit")
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
    s.push_str(&header(prefix, kind));
    s.push_str("<main>\n");
    s.push_str(body);
    s.push_str("\n</main>\n");
    s.push_str(&format!(
        "<footer><nav class=\"foot\"><a href=\"{prefix}about.html\">About</a> · <a href=\"{prefix}boards.html\">Door boards</a> · <a href=\"{prefix}families/index.html\">Extendable families</a> · <a href=\"{prefix}circuits/index.html\">Circuit studies</a> · <a href=\"{prefix}all.html\">Everything</a></nav>Built from <a href=\"{REPO_URL}/tree/{}\">vjeux/minecraft-tas@{}</a> on {} by <code>mctas site build</code>. Every build was verified in the real game (Minecraft 26.3, headless, tick by tick).</footer>\n",
        esc(&site.commit),
        esc(&site.commit.chars().take(9).collect::<String>()),
        esc(&site.generated)
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

/// A circuit page's opening: what it does (with this design's numbers) and how this design does
/// it (its caption).
fn circuit_intro(b: &Build, c: &site_model::Circuit) -> String {
    let how = crate::explain::how_sentence(&b.caption);
    format!(
        "<section class=\"what\"><h2>What it does</h2>{}{}</section>\n",
        crate::explain::what_it_does(c),
        if how.is_empty() { String::new() } else { format!("<p class=\"howdesign\"><b>How this design does it:</b> {how}</p>") }
    )
}

/// A circuit page's sections after the viewer: how to try it, the judge's test as a timing
/// diagram, the measurements with their meanings.
fn circuit_sections(c: &site_model::Circuit, has_viewer: bool, prefix: &str) -> String {
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
            "<section><h2>The judge's test</h2><p>{intro} Time runs left to right in game ticks (20 gt = 1 s).</p><div class=\"wave-wrap\">{}</div>{notes}{}</section>\n",
            crate::explain::wave_svg(w),
            if c.table.is_empty() {
                String::new()
            } else {
                format!("<details><summary>The judge's report, step by step ({} lines)</summary><pre class=\"design\">{}</pre></details>", c.table.len(), esc(&c.table.join("\n")))
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
            "<section><h2>Measurements</h2><p class=\"hint\">Measured by the judge in the real game (4 positions x 4 rotations). gt = game tick = 1/20 s. Every term: <a href=\"{prefix}about.html#glossary\">glossary</a>.</p><table class=\"props metrics\"><thead><tr><th>metric</th><th>value</th><th>what it means</th></tr></thead><tbody>{rows}</tbody></table></section>\n"
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
    // what runs by itself plays at once: clocks, flying machines, self-running TNT dupers, the
    // judge's demo
    let autoplay = if b.kind == "clock" || b.circuit.as_ref().is_some_and(|c| !c.stimulus.is_empty() || !c.events.is_empty() || c.kind == "flying-machine" || c.kind == "tnt-duper") { " data-autoplay" } else { "" };
    let (pic, cls) = match &b.picture {
        Some(p) => (format!("<img class=\"pic contain\" src=\"{}\" alt=\"{} in the game\" decoding=\"async\">", esc(p), esc(&b.name)), " has-pic"),
        None => (String::new(), ""),
    };
    format!(
        "<div class=\"viewer k-{}\" data-mcview=\"full\" data-src=\"{}\"{trace}{circuit}{autoplay} data-kind=\"{}\"><div class=\"fallback{cls}\">{pic}<span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span><span class=\"fb-note\">{}</span></div></div>\n",
        esc(&b.kind),
        esc(&format!("{prefix}{}", b.design)),
        esc(&b.kind),
        esc(&site_model::html::headline(b)),
        esc(&b.name),
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
        // circuits: Circuits › study › kind
        let (study_title, _) = crate::explain::study(&c.study).unwrap_or(("Circuits", ""));
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › <a href=\"{prefix}circuits/{}/index.html\">{}</a> › <a href=\"{prefix}index.html#kind=circuit&amp;group={}\">{}</a></p>\n",
            esc(&c.study),
            esc(study_title),
            site_model::encode(&b.group),
            esc(&crate::explain::kind_title(&c.kind))
        ));
    } else {
        body.push_str(&format!(
            "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}index.html#kind={}\">{}</a> › <a href=\"{prefix}index.html#kind={}&amp;group={}\">{}</a></p>\n",
            esc(&b.kind),
            kind_title(&b.kind),
            esc(&b.kind),
            site_model::encode(&b.group),
            esc(&b.group.replace('+', " + "))
        ));
    }
    body.push_str(&format!("<h1>{}</h1>\n", esc(&b.name)));
    body.push_str(&format!("<p class=\"lede\">{}</p>\n", esc(&lede(b))));
    let badges = site_model::html::badges(b);
    if !badges.is_empty() {
        body.push_str(&format!("<p class=\"badges\">{}</p>\n", badges.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect::<String>()));
    }
    // the redstone first (vjeux: "Put the redstone before the explanation"): the live view and
    // its controls, a tunable group's picker under it, then what it does and the rest
    body.push_str(&viewer(prefix, b));
    let picker = group_picker(site, b, prefix);
    // the picker swaps versions in place (pkg/redstonepedia.js), keeping the viewer's camera
    let scripts = Scripts { app: scripts.ui && scripts.viewer && !picker.is_empty(), ..scripts };
    if let Some(c) = &b.circuit {
        body.push_str(&picker);
        body.push_str(&circuit_intro(b, c));
        body.push_str(&circuit_sections(c, scripts.viewer, prefix));
    }
    if let Some(v) = b.door.as_ref().and_then(|d| d.video.as_ref()) {
        body.push_str(&format!(
            "<section><h2>Video (real game)</h2><video class=\"video\" controls preload=\"none\" src=\"{}\"></video><p><a href=\"{}\">download the video</a></p></section>\n",
            esc(v),
            esc(v)
        ));
    }
    if !b.caption.is_empty() && b.circuit.is_none() {
        body.push_str(&format!("<section><h2>How it works</h2><p>{}</p></section>\n", esc(&b.caption)));
    }
    body.push_str("<section><h2>Properties</h2><table class=\"props\">\n");
    body.push_str(&properties(site, b, prefix));
    body.push_str("</table></section>\n");
    if let Some(d) = &b.door {
        if !d.props.is_empty() {
            body.push_str("<div class=\"allprops\"><details><summary>Every measured property (the community regulation's checks)</summary><table class=\"props small\">\n");
            for (k, v) in &d.props {
                body.push_str(&row(k, &esc(v)));
            }
            body.push_str("</table></details></div>\n");
        }
    }
    body.push_str(&format!(
        "<section><h2>Design</h2><p><a class=\"button\" href=\"{prefix}{}\" download>Download .litematic</a> <a class=\"button alt\" href=\"{prefix}{}\" download>Download the design file</a> <a href=\"{REPO_URL}/blob/main/{}\">on GitHub</a></p>\n<p class=\"hint\">The .litematic opens in <a href=\"https://github.com/maruohon/litematica\">Litematica</a> (Minecraft 26.3): every block, block entity and entity of the design in one region.</p>\n<p>Blocks used: {}</p>\n<details><summary>Design file ({} lines)</summary><pre class=\"design\">{}</pre></details></section>\n",
        esc(&crate::generate::litematic_path(&b.design)),
        esc(&b.design),
        esc(repo_path),
        chips(&b.kinds, prefix, "with"),
        design_text.lines().count(),
        esc(design_text)
    ));
    if b.circuit.is_some() {
        body.push_str(&layouts(site, b, prefix));
    }
    for (title, list) in similar(site, b) {
        if list.is_empty() {
            continue;
        }
        let cards: Vec<String> = list.iter().map(|o| build_card(o, prefix)).collect();
        body.push_str(&format!("<section><h2>{}</h2>\n{}</section>\n", esc(&title), grid(&cards)));
    }
    let desc = format!("{}: {}. {}", b.name, lede(b), b.caption.chars().take(160).collect::<String>());
    shell_full(&b.name, &desc, prefix, &body, scripts, site, b.picture.as_deref(), Some(&b.kind))
}

/// One line under a build's title.
fn lede(b: &Build) -> String {
    match b.kind.as_str() {
        "door" => {
            let mut s = format!("3x3 piston door, {}, {}", b.group, size_line(b));
            if let Some(d) = &b.door {
                if let (Some(o), Some(c)) = (d.open, d.close) {
                    s.push_str(&format!(", opens in {o} gt, closes in {c} gt"));
                }
            }
            s
        }
        "clock" => {
            let c = b.clock.as_ref();
            format!("Clock, period {}, {} blocks, family {}", c.map(|c| gt_time(c.period)).unwrap_or_default(), b.blocks, b.group.replace('+', " + "))
        }
        _ => match &b.circuit {
            Some(c) => {
                let mut parts = vec![crate::explain::kind_title(&c.kind)];
                // the study's axis when it says more than the kind (`L = 2 gt`, `N = 3 gt`)
                if !c.axis.is_empty() && c.axis != c.kind && !c.kind.ends_with(&c.axis) && c.axis.contains('=') {
                    parts.push(c.axis.clone());
                }
                parts.push(format!("{} blocks", b.blocks));
                if let Some(d) = &b.dims {
                    parts.push(d.clone());
                }
                parts.join(" · ")
            }
            None => format!("{} blocks", b.blocks),
        },
    }
}

fn link_build(site: &Site, id: &str, prefix: &str) -> String {
    match site.builds.iter().find(|b| b.id == id) {
        Some(b) => format!("<a href=\"{prefix}{}\">{}</a>", esc(&b.page), esc(&b.name)),
        None => esc(id.rsplit('/').next().unwrap_or(id)),
    }
}

/// The properties table rows of a build.
fn properties(site: &Site, b: &Build, prefix: &str) -> String {
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
        s.push_str(&row("Seamless tier", &esc(if d.seamless.is_empty() { "none" } else { &d.seamless })));
        if !d.frame.is_empty() {
            s.push_str(&row("Frame when open", &esc(&d.frame)));
        }
        s.push_str(&row("Hallway-compatible", yes_no(b.flags.iter().any(|f| f == "hallway"))));
        s.push_str(&row("Record-eligible (community regulation)", yes_no(b.flags.iter().any(|f| f == "eligible"))));
        if !d.boards.is_empty() {
            s.push_str(&row("Boards", &esc(&d.boards.join(", "))));
        }
        if !d.leads.is_empty() {
            s.push_str(&row("#1 on", &esc(&d.leads.join(", "))));
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
            s.push_str(&row("Variant of", &format!("{} (the best filling of the same mechanism)", link_build(site, h, prefix))));
        }
        if !d.variants.is_empty() {
            let shown: Vec<String> = d.variants.iter().take(40).map(|v| link_build(site, v, prefix)).collect();
            let more = if d.variants.len() > 40 { format!(" and {} more", d.variants.len() - 40) } else { String::new() };
            s.push_str(&row(&format!("Fill variants ({})", d.variants.len()), &format!("{}{more}", shown.join(", "))));
        }
    }
    if let Some(c) = &b.clock {
        s.push_str(&row("Period", &esc(&gt_time(c.period))));
        s.push_str(&row("Family (blocks that change state)", &esc(&b.group.replace('+', " + "))));
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        if let (Some(v), Some(dm)) = (b.volume, &b.dims) {
            s.push_str(&row("Box", &format!("{} (volume {v})", esc(dm))));
        }
        s.push_str(&row("Output", &format!("on {}/{} gt, {} rising edge{} per period{}", c.on, c.period, c.edges, if c.edges == 1 { "" } else { "s" }, if c.pulses.is_empty() { String::new() } else { format!(", pulses {}", esc(&c.pulses)) })));
        s.push_str(&row("Warmup", &format!("{} gt", c.warmup)));
        s.push_str(&row("Start", &esc(&c.start)));
        s.push_str(&row("Robustness", &esc(&c.robustness)));
        s.push_str(&row("Chiral (mirror image is another type)", yes_no(c.chiral)));
        if let Some(card) = &c.card {
            if let Some(cd) = site.cards.iter().find(|x| &x.id == card) {
                s.push_str(&row("Extendable family", &format!("<a href=\"{prefix}{}\">{}</a> ({}){}", esc(&cd.page), esc(&cd.title), esc(&cd.heading), c.n.map(|n| format!(", n = {n}")).unwrap_or_default())));
            }
        }
        if let Some(h) = &c.host {
            s.push_str(&row("Variant of", &format!("{} (fewer blocks, a subset of its block kinds, same period)", link_build(site, h, prefix))));
        }
        if !c.variants.is_empty() {
            s.push_str(&row("Variants", &c.variants.iter().map(|v| link_build(site, v, prefix)).collect::<Vec<_>>().join(", ")));
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
        s.push_str(&row("Kind", &format!("{} (<code>circuit: {}</code>)", esc(&crate::explain::kind_title(&c.kind)), esc(&c.kind))));
        if !c.axis.is_empty() {
            s.push_str(&row("The study's axis", &esc(&c.axis)));
        }
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        if let (Some(v), Some(dm)) = (b.volume, &b.dims) {
            s.push_str(&row("Box", &format!("{} (volume {v})", esc(dm))));
        }
        if !c.family.is_empty() {
            s.push_str(&row("Family (blocks that change state)", &esc(&c.family.replace('+', " + "))));
        }
        if !c.robustness.is_empty() {
            s.push_str(&row("Robustness", &format!("{} (same behavior at 4 positions x 4 rotations in the real game)", esc(&c.robustness)).replace("robust (same", "robust (the same")));
        }
        if !c.inputs.is_empty() || !c.outputs.is_empty() || c.kind == "tnt-duper" {
            let ins: Vec<String> = c.inputs.iter().enumerate().map(|(i, n)| format!("{} ({})", n.to_uppercase(), c.devices.get(i).map(String::as_str).unwrap_or("redstone block"))).collect();
            let outs: Vec<String> = c.outputs.iter().map(|o| if o == "out" { "OUT".to_string() } else { o.to_uppercase() }).collect();
            let text = match (ins.is_empty(), outs.is_empty()) {
                (false, false) => format!("{} / {}", ins.join(", "), outs.join(", ")),
                (false, true) => format!("{} (no output: it moves blocks)", ins.join(", ")),
                (true, false) if !c.events.is_empty() => format!("the judge's changes at its target / {}", outs.join(", ")),
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
        if !c.card.is_empty() {
            if let Some(cd) = site.cards.iter().find(|x| x.id == c.card) {
                s.push_str(&row("Extendable family", &format!("<a href=\"{prefix}{}\">{}</a> ({}: one design for every n, all generated)", esc(&cd.page), esc(&cd.title), esc(&cd.heading))));
            }
        }
        if !c.archived.is_empty() {
            s.push_str(&row("Archived", &esc(&c.archived)));
        }
        if !c.verdict.is_empty() {
            s.push_str(&row("Judge", &format!("<code>{}</code>", esc(&c.verdict))));
        }
    }
    let flags: Vec<String> = b.flags.iter().map(|f| flag_label(f).to_string()).collect();
    if !flags.is_empty() {
        s.push_str(&row("Flags", &esc(&flags.join(", "))));
    }
    s
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
        let smallest_per_period = |pool: Vec<&'a Build>| -> Vec<&'a Build> {
            let mut best: BTreeMap<u32, &Build> = BTreeMap::new();
            for o in pool {
                let p = o.clock.as_ref().map(|x| x.period).unwrap_or(0);
                if p == c.period {
                    continue;
                }
                let better = |cur: &&Build| (o.blocks, o.volume, &o.name) < (cur.blocks, cur.volume, &cur.name);
                if best.get(&p).is_none_or(better) {
                    best.insert(p, o);
                }
            }
            let mut v: Vec<&Build> = best.into_values().collect();
            v.sort_by_key(|o| {
                let p = o.clock.as_ref().map(|x| x.period).unwrap_or(0);
                ((p as i64 - c.period as i64).abs(), p)
            });
            v.truncate(8);
            v
        };
        out.push(("Nearby periods (the smallest clock of each)".to_string(), smallest_per_period(others().filter(shown).collect())));
        let mut same: Vec<&Build> = others().filter(shown).filter(|o| o.clock.as_ref().is_some_and(|oc| oc.period == c.period)).collect();
        same.sort_by_key(|o| (o.blocks, o.volume, o.name.clone()));
        same.truncate(8);
        out.push((format!("Other clocks of {}", gt_time(c.period)), same));
        out.push((format!("Same family ({}), other periods", b.group.replace('+', " + ")), smallest_per_period(others().filter(shown).filter(|o| o.group == b.group).collect())));
    } else if b.kind == "door" {
        let v = b.volume.unwrap_or(u32::MAX) as i64;
        let mut near: Vec<&Build> = others().filter(|o| o.group == b.group).collect();
        near.sort_by_key(|o| ((o.volume.unwrap_or(u32::MAX) as i64 - v).abs(), o.blocks, o.name.clone()));
        near.truncate(12);
        out.push((format!("Other {} doors of similar size", b.group), near));
    } else if let Some(c) = &b.circuit {
        out.push(circuit_axis_walk(site, b, c));
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
    let by_kind = matches!(base, "counter" | "piston-extender" | "delay" | "gate" | "flying-machine" | "tff" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender");
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
        "piston-extender" => "Piston extenders of other reaches (the smallest of each N)".to_string(),
        "counter" => "Counters of other N (the smallest of each)".to_string(),
        "delay" => "Delay lines of other delays (the smallest of each N)".to_string(),
        "gate" => "Other logic gates (the smallest of each function)".to_string(),
        "tff" => "Other T flip-flop mechanisms (the smallest of each)".to_string(),
        "flying-machine" => "Flying machines of other directions and speeds (the smallest of each)".to_string(),
        "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" => format!("{}s of other pulse lengths (the smallest of each L)", crate::explain::kind_title(base)),
        _ => format!("Other {} circuits (the smallest of each kind)", crate::explain::study(&c.study).map(|s| s.0.to_lowercase()).unwrap_or_else(|| c.study.clone())),
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
    // a chip: the member's value when the values differ (L 12), else what sets it apart (stone
    // button); its second line the other: what differs (delays 4+4), or the block count
    let chip_text = |o: &Build| -> (String, String) {
        let oc = o.circuit.as_ref();
        let axis = oc.map(|oc| oc.axis.replace(" = ", " ")).unwrap_or_default();
        let what = oc.map(|oc| oc.variant_label.clone()).unwrap_or_default();
        if same_axis {
            (if what.is_empty() { o.name.clone() } else { what }, site_model::html::blocks_word(o.blocks))
        } else {
            (axis, site_model::html::blocks_word(o.blocks))
        }
    };
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
                esc(&big),
                esc(&small)
            )
        })
        .collect();
    let differs = if c.group_differs.is_empty() { "their layout".to_string() } else { c.group_differs.clone() };
    let pick = if same_axis { "Pick one" } else { "Pick a value" };
    format!(
        "<section class=\"group\"><h2>{} <span class=\"count\">{}</span></h2><p class=\"hint\">One mechanism; its versions differ only in {}{}. {pick}: the 3D view (same camera), the numbers and the downloads follow.{}</p><nav class=\"picker\" aria-label=\"versions\">{chips}</nav></section>\n",
        esc(&title),
        esc(&if same_axis { format!("{} versions · {}", members.len(), c.axis) } else { span.clone() }),
        esc(&differs),
        if same_axis { String::new() } else { format!(", which sets {}", c.axis.split(" = ").next().filter(|l| l.len() <= 2).unwrap_or("its value")) },
        if c.group_note.is_empty() { String::new() } else { format!(" ({})", esc(&c.group_note)) }
    )
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
        body.push_str(&format!("<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"index.html\">Extendable families</a></p>\n"));
    }
    body.push_str(&format!("<h1 class=\"formula\">{}</h1>\n", esc(&card.heading)));
    let base = card.families.first().and_then(|f| site.builds.iter().find(|b| b.id == f.build));
    let mut lede = format!("<strong>{}</strong>", esc(&card.title));
    if !card.seconds.is_empty() {
        lede.push_str(&format!(" · P = {}", esc(&card.seconds)));
    }
    if let Some(b) = base {
        if circuit {
            lede.push_str(&format!(" · {}", esc(&crate::explain::kind_title(&b.group))));
        } else {
            lede.push_str(&format!(" · {}", esc(&b.group.replace('+', " + "))));
        }
    }
    if !card.note.is_empty() {
        lede.push_str(&format!(" · {}", esc(&card.note)));
    }
    body.push_str(&format!("<p class=\"lede\">{lede}</p>\n"));
    if circuit {
        if let Some(c) = base.and_then(|b| b.circuit.as_ref()) {
            body.push_str(&format!("<section class=\"what\"><h2>What it does</h2><p>{}.</p><p>This family grows by a repeated step: one design for every n, its value given by the formula above, every one built by the family's own generator.</p></section>\n", esc(&first_upper(&crate::explain::kind_summary(&c.kind)))));
        }
    }
    if let Some(f) = card.families.first() {
        // one live view of the members (a row below swaps its design in place, same camera)
        if let Some(m) = members.iter().find(|m| m.family == f.id && !m.design.is_empty()).or_else(|| members.iter().find(|m| !m.design.is_empty())) {
            let kind = if circuit { "circuit" } else { "clock" };
            body.push_str(&format!(
                "<div class=\"viewer k-{kind}\" id=\"member-view\" data-mcview=\"full\" data-src=\"{prefix}{}\" data-kind=\"{kind}\"{}><div class=\"fallback\"><span class=\"fb-big\">{}</span><span class=\"fb-small\">{} n = {}</span></div></div>\n<p id=\"member-caption\" class=\"hint\">{}</p>\n",
                esc(&m.design),
                if circuit { "" } else { " data-autoplay" },
                esc(&member_value(m)),
                esc(&m.family),
                m.n,
                esc(&member_caption(m))
            ));
        }
        body.push_str(&format!("<p>Extend: {}. Pick a member in the table to see it in the 3D view.</p>\n", esc(&f.rule)));
    }
    if !card.works_with.is_empty() {
        body.push_str(&format!("<p>Works with any one of these {} blocks in place of the {}: {}.</p>\n", card.works_with.len(), esc(&card.works_with[0]), esc(&card.works_with.join(", "))));
    }
    let value_head = if circuit { "value" } else { "period" };
    if card.families.len() > 1 {
        body.push_str(&format!("<section><h2>Families of this card</h2><table class=\"list\"><thead><tr><th>family</th><th>{}</th><th>blocks</th><th>n (verified range)</th><th>judged in the game: n</th></tr></thead><tbody>\n", if circuit { "value" } else { "P" }));
        for f in &card.families {
            let extra = if f.extra.is_empty() { String::new() } else { format!(" (+{})", esc(&f.extra.join(", +"))) };
            body.push_str(&format!(
                "<tr><td>{}{extra}</td><td>{}{}</td><td>{}</td><td>{}..{}</td><td>{}</td></tr>\n",
                link_build(site, &f.build, prefix),
                esc(&f.period_text),
                if circuit { "" } else { " gt" },
                esc(&f.blocks_text),
                f.n_lo,
                f.n_hi,
                if f.judged.is_empty() { "-".to_string() } else { f.judged.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ") }
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    let judge = if circuit { "the circuit judge" } else { "the clock judge" };
    let screen = if circuit { "gives exactly the formula's value in simulation" } else { "runs at exactly the formula's period in simulation" };
    body.push_str(&format!("<section><h2>Every member</h2><p>Each member's design was generated with the family's own generator when this site was built. <span class=\"badge st-judged\">judged in the game</span> = verified by {judge} in the real game; <span class=\"badge st-redsim\">simulated</span> = its generated design {screen}, not judged in the game; <span class=\"badge st-formula\">formula</span> = not individually checked.</p>\n"));
    body.push_str(&format!("<table class=\"list\"><thead><tr><th>family</th><th>n</th><th>{value_head}</th><th>blocks</th><th>box</th><th>status</th><th>download</th></tr></thead><tbody>\n"));
    for m in members {
        let page = member_href(m, site);
        let name = if m.build.is_some() { format!("<a href=\"{prefix}{}\">{}</a>", esc(&page), esc(&m.family)) } else { esc(&m.family) };
        let anchor = site_model::html::member_anchor(m, site);
        let n_cell = if m.design.is_empty() {
            m.n.to_string()
        } else {
            format!("<a class=\"view\" href=\"#{}\" data-src=\"{prefix}{}\" data-caption=\"{}\">{}</a>", esc(&anchor), esc(&m.design), esc(&member_caption(m)), m.n)
        };
        let download = if m.design.is_empty() {
            "-".to_string()
        } else {
            format!("<a href=\"{prefix}{}\" download>.litematic</a> · <a href=\"{prefix}{}\" download>.txt</a>", esc(&crate::generate::litematic_path(&m.design)), esc(&m.design))
        };
        body.push_str(&format!(
            "<tr id=\"{}\"><td>{name}</td><td>{n_cell}</td><td>{}</td><td>{}</td><td>{}</td><td><span class=\"badge st-{}\">{}</span></td><td>{download}</td></tr>\n",
            esc(&anchor),
            esc(&member_value(m)),
            m.blocks,
            esc(m.dims.as_deref().unwrap_or("")),
            esc(&m.status),
            esc(status_label(&m.status)),
        ));
    }
    body.push_str("</tbody></table></section>\n");
    let desc = if circuit { format!("Extendable redstone circuit family {}: {}", card.title, card.heading) } else { format!("Extendable redstone clock family {}: {}", card.title, card.heading) };
    // the member rows swap the view in place (pkg/redstonepedia.js)
    let scripts = Scripts { app: scripts.ui && scripts.viewer, ..scripts };
    shell_full(&card.title, &desc, prefix, &body, scripts, site, None, Some(if circuit { "circuit" } else { "clock" }))
}

/// A family member's value: a clock's period, a circuit's axis value.
fn member_value(m: &Member) -> String {
    if m.axis.is_empty() { gt_time(m.period) } else { m.axis.clone() }
}

/// The caption under a family page's view: which member it shows.
fn member_caption(m: &Member) -> String {
    format!("Showing {} n = {}: {} · {} · {}", m.family, m.n, member_value(m), site_model::html::blocks_word(m.blocks), status_label(&m.status))
}

/// families/index.html: every card, by its smallest period.
pub fn families_index(site: &Site, members: &BTreeMap<String, Vec<&Member>>, scripts: Scripts) -> String {
    let prefix = "../";
    let clocks: Vec<&Card> = site.cards.iter().filter(|c| c.kind.is_empty()).collect();
    let circuits: Vec<&Card> = site.cards.iter().filter(|c| c.kind == "circuit").collect();
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Extendable families</p>\n<h1>Extendable families</h1>\n<p class=\"lede\">Families grow by a repeated step: one design for every n, its number (a clock's period, a circuit's L or N) given by a formula. Every member was generated with the family's own generator when this site was built. Search a period in the <a href=\"{prefix}index.html#kind=clock\">Clocks</a> tab, or a value (L 50, N 26) in the <a href=\"{prefix}index.html#kind=circuit\">Circuits</a> tab, to see the exact member that hits it.</p>\n"
    );
    for (title, list, what) in [("Clocks", &clocks, "periods reached"), ("Circuits", &circuits, "values reached")] {
        if list.is_empty() {
            continue;
        }
        body.push_str(&format!("<section><h2>{title} <span class=\"count\">{} cards</span></h2><table class=\"list\"><thead><tr><th>card</th><th>formula</th><th>families</th><th>members</th><th>{what}</th></tr></thead><tbody>\n", list.len()));
        for c in list.iter() {
            let ms = members.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]);
            let (lo, hi) = (ms.iter().min_by_key(|m| m.period), ms.iter().max_by_key(|m| m.period));
            let show = |m: &Member| if m.axis.is_empty() { gt_time(m.period) } else { m.axis.clone() };
            let reach = match (lo, hi) {
                (Some(l), Some(h)) => format!("{} .. {}", show(l), show(h)),
                _ => "-".into(),
            };
            body.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(c.page.trim_start_matches("families/")),
                esc(&c.title),
                esc(&c.heading),
                c.families.len(),
                ms.len(),
                esc(&reach)
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    shell_full("Extendable families", "Every extendable redstone clock and circuit family with its formula", prefix, &body, scripts, site, None, None)
}

/// index.html: the filtering UI's shell (the UI itself is pkg/redstonepedia.js).
pub fn index_page(site: &Site, scripts: Scripts) -> String {
    let n = |k: &str| site.builds.iter().filter(|b| b.kind == k).count();
    let data_v = versions().data;
    let data_src = if data_v.is_empty() { "data/site.json".to_string() } else { format!("data/site.json?v={data_v}") };
    let body = format!(
        "<h1 class=\"sr-only\">{SITE_NAME}</h1><p class=\"lede intro\">Every redstone build of <a href=\"{REPO_URL}\">minecraft-tas</a>, verified tick by tick in the real game: {} piston doors, {} clocks{}.</p>\n\
         <div id=\"app\" class=\"app\"><aside id=\"filters\" class=\"filters\" data-src=\"{data_src}\"><p class=\"loading\">Loading the filters…</p></aside>\
         <div class=\"results\"><div id=\"summary\" class=\"summary\"></div><div id=\"grid\" class=\"grid\"></div><p class=\"more-wrap\"><button id=\"more\" type=\"button\" hidden>Show more</button></p></div></div>\n\
         <noscript><p class=\"notice\">The filters need JavaScript and WebAssembly. Without them, browse <a href=\"all.html\">the list of everything</a>.</p></noscript>\n",
        n("door"),
        n("clock"),
        if n("circuit") > 0 { format!(", {} circuits", n("circuit")) } else { String::new() }
    );
    shell(SITE_NAME, "Every redstone build of minecraft-tas: piston doors, clocks and circuits, verified in the real game, filterable by their properties", "", &body, Scripts { app: true, ..scripts }, site)
}

/// all.html: every build in plain tables (no script needed).
pub fn all_page(site: &Site, scripts: Scripts) -> String {
    let mut body = format!("<h1>Everything</h1>\n<p class=\"lede\">Every build in plain tables. The <a href=\"index.html\">index</a> filters them.</p>\n");
    for kind in ["door", "clock", "circuit"] {
        let list: Vec<&Build> = site.builds.iter().filter(|b| b.kind == kind).collect();
        if list.is_empty() {
            continue;
        }
        body.push_str(&format!("<section><h2>{} ({})</h2><table class=\"list\"><thead><tr><th>name</th><th>{}</th><th>blocks</th><th>{}</th><th>flags</th></tr></thead><tbody>\n", kind_title(kind), list.len(), if kind == "clock" { "period" } else { "size" }, if kind == "clock" { "family" } else { "group" }));
        for b in list {
            let main = match &b.clock {
                Some(c) => gt_time(c.period),
                None => site_model::html::headline(b),
            };
            body.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(&b.page),
                esc(&b.name),
                esc(&main),
                b.blocks,
                esc(&b.group.replace('+', " + ")),
                esc(&b.flags.join(", "))
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    if !site.cards.is_empty() {
        body.push_str(&format!("<section><h2>Extendable families ({})</h2><ul>\n", site.cards.len()));
        for c in &site.cards {
            body.push_str(&format!("<li><a href=\"{}\">{}</a>: {}</li>\n", esc(&c.page), esc(&c.title), esc(&c.heading)));
        }
        body.push_str("</ul></section>\n");
    }
    shell("Everything", "Every redstone build of minecraft-tas in plain tables", "", &body, Scripts { viewer: false, ..scripts }, site)
}

/// boards.html: the door boards as LEADERBOARD.md ranks them (the size boards by volume, then
/// blocks placed; the community regulation's seamless and record-eligible boards; the speed
/// boards), each with its leader's card and its top 10.
pub fn boards_page(site: &Site, scripts: Scripts) -> String {
    let doors: Vec<&Build> = site.builds.iter().filter(|b| b.kind == "door").collect();
    let has = |b: &Build, f: &str| b.flags.iter().any(|x| x == f);
    let vol = |b: &Build| b.volume.unwrap_or(u32::MAX);
    let ticks = |b: &Build| b.door.as_ref().map(|d| (d.open.unwrap_or(999), d.close.unwrap_or(999))).unwrap_or((999, 999));
    let tier = |b: &Build| site_tier(b.door.as_ref().map(|d| d.seamless.as_str()).unwrap_or(""));
    let mut boards: Vec<(String, String, Vec<&Build>)> = Vec::new();
    // the size boards of LEADERBOARD.md (leaderboard.rs `render`): rotation-safe, full frame
    // when open, ranked by volume, then blocks placed, then open + close
    let solid = |b: &Build| b.group == "solid";
    let size_boards: [(&str, &str, Box<dyn Fn(&Build) -> bool>); 8] = [
        ("Solid (main ranking)", "9 identical plain door blocks, no sculk sensors", Box::new(solid)),
        ("Solid, no slime or honey", "same, and not a single slime or honey block anywhere", Box::new(move |b: &Build| solid(b) && has(b, "slime-free"))),
        ("Solid, flat (one z layer), no slime or honey", "slime-free, and every block in the door's plane: the whole machine is one block thick", Box::new(move |b: &Build| solid(b) && has(b, "slime-free") && has(b, "flat"))),
        ("Solid, in a 3-thick wall", "every block within one layer of the door's plane, at rest and while it moves", Box::new(move |b: &Build| solid(b) && has(b, "wall"))),
        ("Solid, on the ground (nothing below the door)", "no block below the door's bottom row, at rest or while it moves; works standing on a stone floor", Box::new(move |b: &Build| solid(b) && has(b, "ground"))),
        ("Sculk", "uses sculk sensors (footsteps or mobs nearby can trigger them)", Box::new(|b: &Build| b.group == "sculk")),
        ("Gravity", "the 9 door blocks are sand, gravel or concrete powder", Box::new(|b: &Build| b.group == "gravity")),
        ("Entities allowed", "solid doors that may also use entities (counted apart: ranked by size, then entities, then blocks placed)", Box::new(|b: &Build| b.group == "solid" || b.group == "entity")),
    ];
    let entities = |b: &Build| b.door.as_ref().map(|d| d.entities).unwrap_or(0);
    for (title, rule, keep) in &size_boards {
        let mut list: Vec<&Build> = doors.iter().copied().filter(|b| has(b, "rotation-safe") && has(b, "frame-full") && keep(b)).collect();
        let ent = title.starts_with("Entities");
        list.sort_by_key(|b| (vol(b), if ent { entities(b) } else { 0 }, b.blocks, ticks(b).0 + ticks(b).1, b.name.clone()));
        boards.push((title.to_string(), format!("{rule}; rotation-safe; full frame when open; ranked by size (the volume), then blocks placed"), list));
    }
    let mut seam: Vec<&Build> = doors.iter().copied().filter(|b| b.group == "solid" && has(b, "rotation-safe") && tier(b) > 0).collect();
    seam.sort_by_key(|b| (std::cmp::Reverse(tier(b)), !has(b, "hallway"), vol(b), b.blocks, b.name.clone()));
    boards.push(("Seamless".into(), "solid doors with a seamless tier of the community regulation, ranked by tier, then hallway-compatible first, then size".into(), seam.clone()));
    let seam_flat: Vec<&Build> = seam.iter().copied().filter(|b| has(b, "slime-free") && has(b, "flat")).collect();
    boards.push(("Seamless, flat, no slime or honey".into(), "the same ranking for flat, slime-free doors".into(), seam_flat));
    let mut elig: Vec<&Build> = doors.iter().copied().filter(|b| b.group != "entity" && has(b, "rotation-safe") && has(b, "eligible")).collect();
    elig.sort_by_key(|b| (vol(b), b.blocks, b.name.clone()));
    boards.push(("Record-eligible (community regulation)".into(), "a seamless tier, hallway-compatible, lever on the surface with steady support, works with repeater input, unlimited uses, footsteps harmless; ranked by size".into(), elig));
    let mut unrot: Vec<&Build> = doors.iter().copied().filter(|b| !has(b, "rotation-safe")).collect();
    unrot.sort_by_key(|b| (vol(b), b.blocks, ticks(b).0 + ticks(b).1, b.name.clone()));
    boards.push(("Not rotation-safe".into(), "working designs (they pass the judge as built) that break when turned 90, 180 or 270 degrees, by size".into(), unrot));
    let mut frame: Vec<&Build> = doors.iter().copied().filter(|b| has(b, "rotation-safe") && !has(b, "frame-full")).collect();
    frame.sort_by_key(|b| (vol(b), b.blocks, ticks(b).0 + ticks(b).1, b.name.clone()));
    boards.push(("Frame not full when open".into(), "working, rotation-safe designs the size boards leave out: some of the 12 frame cells around the opening do not hold the door block when open".into(), frame));
    for (title, solid_only, key) in [("Fastest open", false, 0u8), ("Fastest open (solid only)", true, 0), ("Fastest close", false, 1), ("Fastest close (solid only)", true, 1), ("Fastest open + close", false, 2), ("Fastest open + close (solid only)", true, 2)] {
        let mut list: Vec<&Build> = doors
            .iter()
            .copied()
            .filter(|b| b.group != "entity" && has(b, "rotation-safe") && (!solid_only || b.group == "solid") && b.door.as_ref().is_some_and(|d| d.open.is_some() && d.close.is_some()))
            .collect();
        let t = |b: &Build| {
            let (o, c) = ticks(b);
            match key {
                0 => o,
                1 => c,
                _ => o + c,
            }
        };
        list.sort_by_key(|b| (t(b), vol(b), b.blocks, b.name.clone()));
        boards.push((title.to_string(), "fewest game ticks after the lever flip (worst of 3 cycles), ties by size, then blocks placed; rotation-safe".into(), list));
    }
    let mut body = format!(
        "<h1>Door boards</h1>\n<p class=\"lede\">The 3x3 piston door boards, ranked like <a href=\"{REPO_URL}/blob/main/designs/piston-door-3x3/LEADERBOARD.md\">LEADERBOARD.md</a>: size is the community's volume (the bounding box over a whole cycle, the lever excluded), then blocks placed. {} working doors.</p>\n<ul class=\"toc\">",
        doors.len()
    );
    for (i, (title, _, list)) in boards.iter().enumerate() {
        body.push_str(&format!("<li><a href=\"#b{i}\">{}</a> ({})</li>", esc(title), list.len()));
    }
    body.push_str("</ul>\n");
    for (i, (title, rule, list)) in boards.iter().enumerate() {
        body.push_str(&format!("<section id=\"b{i}\"><h2>{}</h2><p class=\"hint\">{}.</p>\n", esc(title), esc(rule)));
        let Some(first) = list.first() else {
            body.push_str("<p>No entry yet.</p></section>\n");
            continue;
        };
        body.push_str(&format!("<div class=\"board\"><div class=\"leader\">{}</div><table class=\"list\"><thead><tr><th>#</th><th>size</th><th>blocks</th><th>open / close</th><th>seamless</th><th>design</th></tr></thead><tbody>\n", build_card(first, "")));
        for (k, b) in list.iter().take(10).enumerate() {
            let (o, c) = ticks(b);
            body.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><a href=\"{}\">{}</a></td></tr>\n",
                k + 1,
                esc(&b.volume.map(|v| format!("{v}b ({})", b.dims.clone().unwrap_or_default())).unwrap_or_else(|| "-".into())),
                b.blocks,
                if o < 999 { format!("{o} / {c}") } else { "-".into() },
                esc(b.door.as_ref().map(|d| d.seamless.as_str()).filter(|s| !s.is_empty()).unwrap_or("-")),
                esc(&b.page),
                esc(&b.name)
            ));
        }
        body.push_str("</tbody></table></div></section>\n");
    }
    shell("Door boards", "Every 3x3 piston door board: smallest, slime-free, flat, seamless, record-eligible, fastest", "", &body, scripts, site)
}

/// Seamless tier rank: SUPER 4, FULL 3, SEMI 2, QUART 1, none 0 (leaderboard::tier_rank).
fn site_tier(t: &str) -> u8 {
    piston_door::leaderboard::tier_rank(t)
}

/// about.html.
pub fn about_page(site: &Site, scripts: Scripts) -> String {
    let body = format!(
        "<h1>About {SITE_NAME}</h1>\n\
         <p>{SITE_NAME} lists every build of <a href=\"{REPO_URL}\">vjeux/minecraft-tas</a>: a headless Minecraft 26.3 server that runs the game's own code one tick at a time, many worlds at once. Night after night, agents searched for the smallest 3x3 piston doors, then for every kind of redstone clock, then for the classic contraptions (pulse circuits, T flip-flops, delay lines, logic gates, memory, piston extenders, flying machines).</p>\n\
         <p>Nothing here is a screenshot of someone's world: each build is a design file (blocks, layer by layer) that the project's judge verified in the real game, at several positions and rotations. The 3D views are rendered and simulated live in your browser from those files.</p>\n\
         <p>Only the project's own designs are published here: doors derived from the piston door community's builds (which the project reproduced to calibrate its judge) are left out, to credit their authors rather than republish their work.</p>\n\
         <h2>Where the numbers come from</h2>\n<ul>\n\
         <li><strong>Doors</strong>: <code>mctas door verify --extra-offsets 28</code> (3 open/close cycles at 32 positions); size is the piston door community's volume (the bounding box of the build over a whole cycle, the lever excluded); seamless tiers, hallway and record eligibility follow the community regulation (<a href=\"{REPO_URL}/blob/main/docs/rules.md\">docs/rules.md</a>).</li>\n\
         <li><strong>Clocks</strong>: <code>mctas clock verify</code> (4 positions x 4 rotations in the real game); the period is the exact repeat of the whole state; the family is the set of block kinds that change state (<a href=\"{REPO_URL}/blob/main/docs/clocks.md\">docs/clocks.md</a>). A variant is a clock of a period that only adds block kinds to a smaller one. Extendable families give a formula for every n; their members are generated by the family's own generator, and each is marked as judged in the game, simulated, or given by the formula only.</li>\n\
         <li><strong>Circuits</strong>: <code>mctas circuit verify</code> (4 positions x 4 rotations in the real game) against each kind's spec: pulse circuits by their output pulse length, delay lines by their shift, gates by their truth table, T flip-flops, latches and counters, piston extenders by their reach, flying machines by their speed (<a href=\"{REPO_URL}/blob/main/docs/circuits.md\">docs/circuits.md</a>).</li>\n\
         </ul>\n\
         <h2>Data</h2><p>Everything on this site is in <a href=\"data/site.json\">data/site.json</a> ({} builds, {} family members), regenerated by <code>mctas site build</code> from the repository.</p>\n\
         <h2 id=\"glossary\">Glossary</h2>\n{}",
        site.builds.len(),
        site.members.len(),
        crate::explain::glossary()
    );
    shell("About", "What REDSTONEPEDIA is and where its numbers come from", "", &body, Scripts { viewer: false, ..scripts }, site)
}

/// The circuits of one study folder (`pulse`, `tff`, ...), representatives first.
fn study_builds<'a>(site: &'a Site, folder: &str) -> Vec<&'a Build> {
    let mut v: Vec<&Build> = site.builds.iter().filter(|b| b.page.starts_with(&format!("circuits/{folder}/"))).collect();
    v.sort_by(|a, b| {
        let key = |x: &Build| {
            let c = x.circuit.as_ref();
            (c.map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default(), c.and_then(|c| c.value).unwrap_or(i64::MAX), x.blocks, x.volume.unwrap_or(0), x.name.clone())
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
            let n = cmetric(b, key);
            format!(
                "<a href=\"{prefix}{}\">{}</a><br><span class=\"hint\">{}{}</span>",
                esc(&b.page),
                esc(&b.name),
                esc(&site_model::html::blocks_word(b.blocks)),
                if n.is_empty() { String::new() } else { format!(" · {}{unit}", esc(n)) }
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
    if all.is_empty() || !matches!(folder, "shulker" | "tnt" | "bud") {
        return String::new();
    }
    let has_badge = |b: &Build, badge: &str| cmetric(b, "badges").split('+').any(|x| x == badge);
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
        "<section><h2>Variations</h2><p>What builds need from these machines, each measured by the judge as its own scenario, and the best design for each (the smallest that has it). Every variation, how the judge measures it, and the ones still open: <a href=\"{}\">VARIATIONS.md</a>.</p><table class=\"list variations\"><thead>{head}</thead><tbody>\n{rows}</tbody></table></section>\n",
        esc(&link)
    )
}

/// circuits/<study>/index.html: what the study collects, how designs are ranked, then each kind
/// with its plain explanation and its designs (variants folded under a count).
pub fn study_page(site: &Site, folder: &str, scripts: Scripts) -> String {
    let prefix = "../../";
    let (title, intro) = match crate::explain::study(folder) {
        Some((t, i)) => (t.to_string(), i.to_string()),
        None => (format!("Circuits: {folder}"), String::new()),
    };
    let list = study_builds(site, folder);
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}circuits/index.html\">Circuits</a> › {}</p>\n<h1>{}</h1>\n<p class=\"lede\">{}</p>\n<p><a class=\"button alt\" href=\"{prefix}index.html#kind=circuit&amp;q={}\">Filter them in the index</a></p>\n",
        esc(&title),
        esc(&title),
        esc(&intro),
        site_model::encode(folder)
    );
    // the studies of machines: the best design for each variation (VARIATIONS.md)
    body.push_str(&variations_section(site, folder, prefix));
    // the study's extendable families: one line per card, its formula
    let cards: Vec<&Card> = site.cards.iter().filter(|c| c.kind == "circuit" && c.study == folder).collect();
    if !cards.is_empty() {
        body.push_str("<section><h2>Extendable families</h2><p>Each grows by a repeated step: one design for every n, its value given by the formula; every member is generated when the site is built (search a value, e.g. L 50 or N 26, in the Circuits tab to get the exact member).</p><table class=\"list\"><thead><tr><th>family</th><th>formula</th><th>blocks</th><th>n</th></tr></thead><tbody>\n");
        for c in cards {
            let f = c.families.first();
            body.push_str(&format!(
                "<tr><td><a href=\"{prefix}{}\">{}</a></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                esc(&c.page),
                esc(&c.title),
                esc(&c.heading),
                esc(&f.map(|f| f.blocks_text.clone()).unwrap_or_default()),
                esc(&f.map(|f| format!("{}..{}", f.n_lo, f.n_hi)).unwrap_or_default())
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    // one section per kind family (counter 2..24 together, every gate together)
    let mut kinds: Vec<String> = Vec::new();
    for b in &list {
        let k = b.circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default();
        if !kinds.contains(&k) {
            kinds.push(k);
        }
    }
    for k in &kinds {
        let of: Vec<&&Build> = list.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.kind.split_whitespace().next() == Some(k.as_str()))).collect();
        let shown = |c: &site_model::Circuit| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty());
        let reps: Vec<&&Build> = of.iter().copied().filter(|b| b.circuit.as_ref().is_none_or(shown)).collect();
        let n_var = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_some())).count();
        let n_lay = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.layout_of.is_some())).count();
        let n_mem = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.tunable.is_empty() && c.span.is_empty())).count();
        let n_fam = of.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.card_member && c.tunable.is_empty())).count();
        let mut hidden: Vec<String> = Vec::new();
        if n_mem > 0 {
            hidden.push(format!("{n_mem} more member{} of the tunable groups (on each group's page)", if n_mem == 1 { "" } else { "s" }));
        }
        if n_fam > 0 {
            hidden.push(format!("{n_fam} more base design{} of the extendable families (on each family's page)", if n_fam == 1 { "" } else { "s" }));
        }
        if n_lay > 0 {
            hidden.push(format!("{n_lay} other layout{}", if n_lay == 1 { "" } else { "s" }));
        }
        if n_var > 0 {
            hidden.push(format!("{n_var} variant{} (extra block kinds)", if n_var == 1 { "" } else { "s" }));
        }
        let heading = match k.as_str() {
            "counter" => "Counters".to_string(),
            "gate" => "Logic gates".to_string(),
            "piston-extender" => "Piston extenders".to_string(),
            "shulker-loader" => "Shulker box loaders".to_string(),
            "shulker-unloader" => "Shulker box unloaders".to_string(),
            "bud" => "BUD switches".to_string(),
            "tnt-duper" => "TNT dupers (self-running)".to_string(),
            "tnt-duper-triggered" => "TNT dupers (one shot per pulse)".to_string(),
            _ => crate::explain::kind_title(k),
        };
        let cards: Vec<String> = reps.iter().map(|b| build_card(b, prefix)).collect();
        body.push_str(&format!(
            "<section><h2>{}</h2><p>{}.</p>{}{}</section>\n",
            esc(&heading),
            esc(&first_upper(&crate::explain::kind_summary(k))),
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
    shell_full(&title, &intro, prefix, &body, scripts, site, None, Some("circuit"))
}

fn first_upper(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// circuits/index.html: every study, what it collects, and its smallest design of each kind.
pub fn circuits_index(site: &Site, scripts: Scripts) -> String {
    let prefix = "../";
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Circuits</p>\n<h1>Circuits</h1>\n<p class=\"lede\">The classic redstone contraptions, one study each. Every design was checked by the circuit judge in the real game against its kind's spec, at 4 positions and 4 rotations; each page explains what the circuit does, lets you try it in 3D, and shows the judge's test.</p>\n"
    );
    let folders: Vec<&str> = crate::explain::STUDIES.iter().map(|s| s.0).collect();
    for folder in folders {
        let list = study_builds(site, folder);
        if list.is_empty() {
            continue;
        }
        let Some((title, intro)) = crate::explain::study(folder) else { continue };
        // the smallest representative of each kind
        let mut seen: Vec<String> = Vec::new();
        let mut cards: Vec<String> = Vec::new();
        for b in &list {
            let Some(c) = &b.circuit else { continue };
            if c.host.is_some() || c.layout_of.is_some() || c.card_member || (!c.tunable.is_empty() && c.span.is_empty()) || seen.contains(&c.kind) {
                continue;
            }
            seen.push(c.kind.clone());
            if cards.len() < 8 {
                cards.push(build_card(b, prefix));
            }
        }
        body.push_str(&format!(
            "<section><h2><a href=\"{prefix}circuits/{folder}/index.html\">{}</a> <span class=\"count\">{} designs</span></h2><p>{}</p>{}<p><a href=\"{prefix}circuits/{folder}/index.html\">Every {} design →</a></p></section>\n",
            esc(title),
            list.len(),
            esc(intro),
            grid(&cards),
            esc(&title.to_lowercase())
        ));
    }
    shell_full("Circuits", "The classic redstone contraptions, judged in the real game: pulse circuits, T flip-flops, delay lines, logic gates, memory, piston extenders, flying machines", prefix, &body, scripts, site, None, Some("circuit"))
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
        assert!(p.contains("<title>obs2 · REDSTONEPEDIA</title>"));
        assert!(p.contains("data-mcview=\"full\" data-src=\"../designs/clocks/obs2.txt\" data-autoplay data-kind=\"clock\""), "{p}");
        assert!(p.contains("Two observers face to face &lt;3"));
        assert!(p.contains("<a class=\"card\" href=\"../clocks/obs3.html\""), "similar builds link to the other clock of the period");
        assert!(p.contains("import init from \"../viewer/mcview.js?v="), "{p}");
        assert!(p.contains("init({module_or_path: \"../viewer/mcview_bg.wasm?v="), "the wasm by its own versioned URL");
        assert!(!p.contains("pkg/redstonepedia.js"));
        let d = build_page(&s, &s.builds[2], "name: fc2\n", "designs/piston-door-3x3/archive/fc2-40b.txt", Scripts::default());
        assert!(d.contains("144b (8x9x2)"));
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
