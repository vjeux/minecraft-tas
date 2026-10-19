//! The static pages: the index (the filtering UI's shell), one page per build, one per
//! extendable family card, the family index, a static list of everything (no script needed),
//! the about page and the 404 page. Every page is plain HTML5 with relative links (the site is a
//! GitHub project page under /minecraft-tas/); viewers are `data-mcview` elements that
//! webrender's bundle (viewer/mcview.js) renders when present.

use site_model::html::{build_card, esc, kind_title, member_card, member_href, size_line};
use site_model::{Build, Card, Member, Site, flag_label, gt_time, real_time, status_label};
use std::collections::BTreeMap;

pub const REPO_URL: &str = "https://github.com/vjeux/minecraft-tas";
pub const SITE_NAME: &str = "REDSTONEPEDIA";

/// What a page loads besides its HTML.
#[derive(Clone, Copy, Default)]
pub struct Scripts {
    /// webrender's viewer bundle (viewer/mcview.js): only when the site has one.
    pub viewer: bool,
    /// The filtering UI (pkg/redstonepedia.js).
    pub app: bool,
}

/// A whole page around `body`. `prefix` is the path from the page to the site root.
pub fn shell(title: &str, description: &str, prefix: &str, body: &str, scripts: Scripts, site: &Site) -> String {
    let mut s = String::with_capacity(body.len() + 4096);
    s.push_str("<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    s.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    s.push_str(&format!("<title>{}</title>\n", esc(&if title == SITE_NAME { title.to_string() } else { format!("{title} · {SITE_NAME}") })));
    s.push_str(&format!("<meta name=\"description\" content=\"{}\">\n", esc(description)));
    s.push_str(&format!("<link rel=\"stylesheet\" href=\"{prefix}style.css\">\n"));
    s.push_str(&format!("<link rel=\"icon\" href=\"{prefix}favicon.svg\" type=\"image/svg+xml\">\n"));
    s.push_str("</head>\n<body>\n");
    s.push_str(&format!(
        "<header class=\"top\"><a class=\"logo\" href=\"{prefix}index.html\"><span class=\"logo-dot\"></span>{SITE_NAME}</a><nav>\
         <a href=\"{prefix}index.html#kind=door\">Doors</a>\
         <a href=\"{prefix}index.html#kind=clock\">Clocks</a>\
         <a href=\"{prefix}index.html#kind=clock&amp;time=\">Clock by time</a>\
         <a href=\"{prefix}families/index.html\">Extendable families</a>\
         <a href=\"{prefix}all.html\">Everything</a>\
         <a href=\"{prefix}about.html\">About</a></nav></header>\n"
    ));
    s.push_str("<main>\n");
    s.push_str(body);
    s.push_str("\n</main>\n");
    s.push_str(&format!(
        "<footer>Built from <a href=\"{REPO_URL}/tree/{}\">vjeux/minecraft-tas@{}</a> on {} by <code>mctas site build</code>. Every build was verified in the real game (Minecraft 26.3, headless, tick by tick).</footer>\n",
        esc(&site.commit),
        esc(&site.commit.chars().take(9).collect::<String>()),
        esc(&site.generated)
    ));
    // module specifiers must start with ./ or ../ to be relative
    let module = if prefix.is_empty() { "./" } else { prefix };
    if scripts.app {
        s.push_str(&format!("<script type=\"module\">import init from \"{module}pkg/redstonepedia.js\"; init();</script>\n"));
    }
    if scripts.viewer {
        s.push_str(&format!("<script type=\"module\">import init from \"{module}viewer/mcview.js\"; init();</script>\n"));
    }
    s.push_str("</body>\n</html>\n");
    s
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

/// The viewer of a build page (a live view when the viewer bundle is there; the interim picture
/// or the fallback text until then).
fn viewer(prefix: &str, b: &Build) -> String {
    let trace = b.trace.as_ref().map(|t| format!(" data-trace=\"{}\"", esc(&format!("{prefix}{t}")))).unwrap_or_default();
    let (pic, cls) = match &b.picture {
        Some(p) => (format!("<img class=\"pic contain\" src=\"{}\" alt=\"{} in the game\" decoding=\"async\">", esc(p), esc(&b.name)), " has-pic"),
        None => (String::new(), ""),
    };
    format!(
        "<div class=\"viewer k-{}\" data-mcview=\"full\" data-src=\"{}\"{trace} data-kind=\"{}\"><div class=\"fallback{cls}\">{pic}<span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span><span class=\"fb-note\">{}</span></div></div>\n",
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
    body.push_str(&format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"{prefix}index.html#kind={}\">{}</a> › <a href=\"{prefix}index.html#kind={}&amp;group={}\">{}</a></p>\n",
        esc(&b.kind),
        kind_title(&b.kind),
        esc(&b.kind),
        site_model::encode(&b.group),
        esc(&b.group.replace('+', " + "))
    ));
    body.push_str(&format!("<h1>{}</h1>\n", esc(&b.name)));
    body.push_str(&format!("<p class=\"lede\">{}</p>\n", esc(&lede(b))));
    let badges = site_model::html::badges(b);
    if !badges.is_empty() {
        body.push_str(&format!("<p class=\"badges\">{}</p>\n", badges.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect::<String>()));
    }
    body.push_str(&viewer(prefix, b));
    if let Some(v) = b.door.as_ref().and_then(|d| d.video.as_ref()) {
        body.push_str(&format!(
            "<section><h2>Video (real game)</h2><video class=\"video\" controls preload=\"none\" src=\"{}\"></video><p><a href=\"{}\">download the video</a></p></section>\n",
            esc(v),
            esc(v)
        ));
    }
    if !b.caption.is_empty() {
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
        "<section><h2>Design</h2><p><a class=\"button\" href=\"{prefix}{}\" download>Download the design file</a> <a href=\"{REPO_URL}/blob/main/{}\">on GitHub</a></p>\n<p>Blocks used: {}</p>\n<details><summary>Design file ({} lines)</summary><pre class=\"design\">{}</pre></details></section>\n",
        esc(&b.design),
        esc(repo_path),
        chips(&b.kinds, prefix, "with"),
        design_text.lines().count(),
        esc(design_text)
    ));
    for (title, list) in similar(site, b) {
        if list.is_empty() {
            continue;
        }
        let cards: Vec<String> = list.iter().map(|o| build_card(o, prefix)).collect();
        body.push_str(&format!("<section><h2>{}</h2>\n{}</section>\n", esc(&title), grid(&cards)));
    }
    let desc = format!("{}: {}. {}", b.name, lede(b), b.caption.chars().take(160).collect::<String>());
    shell(&b.name, &desc, prefix, &body, scripts, site)
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
        _ => format!("{} {}, {} blocks", b.group, b.circuit.as_ref().map(|c| c.axis.clone()).unwrap_or_default(), b.blocks),
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
        s.push_str(&row("Study", &esc(&c.study)));
        if !c.axis.is_empty() {
            s.push_str(&row("Axis", &esc(&c.axis)));
        }
        s.push_str(&row("Blocks placed", &b.blocks.to_string()));
        for (k, v) in &c.metrics {
            s.push_str(&row(k, &esc(v)));
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
        let mut same: Vec<&Build> = others().filter(|o| o.clock.as_ref().is_some_and(|oc| oc.period == c.period)).collect();
        same.sort_by_key(|o| (o.blocks, o.volume, o.name.clone()));
        same.truncate(12);
        out.push((format!("Other clocks of {}", gt_time(c.period)), same));
        let mut fam: Vec<&Build> = others().filter(|o| o.group == b.group && o.clock.as_ref().is_some_and(|oc| oc.period != c.period)).collect();
        fam.sort_by_key(|o| (o.clock.as_ref().map(|oc| (oc.period as i64 - c.period as i64).abs()).unwrap_or(i64::MAX), o.blocks, o.name.clone()));
        fam.truncate(12);
        out.push((format!("Same family ({}), nearest periods", b.group.replace('+', " + ")), fam));
    } else if b.kind == "door" {
        let v = b.volume.unwrap_or(u32::MAX) as i64;
        let mut near: Vec<&Build> = others().filter(|o| o.group == b.group).collect();
        near.sort_by_key(|o| ((o.volume.unwrap_or(u32::MAX) as i64 - v).abs(), o.blocks, o.name.clone()));
        near.truncate(12);
        out.push((format!("Other {} doors of similar size", b.group), near));
    } else {
        let mut near: Vec<&Build> = others().filter(|o| o.group == b.group).collect();
        near.sort_by_key(|o| (o.blocks, o.name.clone()));
        near.truncate(12);
        out.push((format!("Other {} circuits", b.group), near));
    }
    out
}

/// The page of an extendable family card: its formula, its two smallest members, its families
/// and every member n with its period, blocks and status.
pub fn card_page(site: &Site, card: &Card, members: &[&Member], scripts: Scripts) -> String {
    let prefix = "../";
    let mut body = String::new();
    body.push_str(&format!("<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › <a href=\"index.html\">Extendable families</a></p>\n"));
    body.push_str(&format!("<h1 class=\"formula\">{}</h1>\n", esc(&card.heading)));
    let base = card.families.first().and_then(|f| site.builds.iter().find(|b| b.id == f.build));
    let mut lede = format!("<strong>{}</strong>", esc(&card.title));
    if !card.seconds.is_empty() {
        lede.push_str(&format!(" · P = {}", esc(&card.seconds)));
    }
    if let Some(b) = base {
        lede.push_str(&format!(" · {}", esc(&b.group.replace('+', " + "))));
    }
    if !card.note.is_empty() {
        lede.push_str(&format!(" · {}", esc(&card.note)));
    }
    body.push_str(&format!("<p class=\"lede\">{lede}</p>\n"));
    if let Some(f) = card.families.first() {
        body.push_str(&format!("<p>Extend: {}.</p>\n", esc(&f.rule)));
        let two: Vec<&&Member> = members.iter().filter(|m| m.family == f.id).take(2).collect();
        if !two.is_empty() {
            let cards: Vec<String> = two.iter().map(|m| member_card(m, site, prefix)).collect();
            body.push_str(&format!("<div class=\"grid two\">{}</div>\n", cards.concat()));
        }
    }
    if !card.works_with.is_empty() {
        body.push_str(&format!("<p>Works with any one of these {} blocks in place of the {}: {}.</p>\n", card.works_with.len(), esc(&card.works_with[0]), esc(&card.works_with.join(", "))));
    }
    if card.families.len() > 1 {
        body.push_str("<section><h2>Families of this card</h2><table class=\"list\"><thead><tr><th>family</th><th>P</th><th>blocks</th><th>n (verified range)</th><th>judged in the game: n</th></tr></thead><tbody>\n");
        for f in &card.families {
            let extra = if f.extra.is_empty() { String::new() } else { format!(" (+{})", esc(&f.extra.join(", +"))) };
            body.push_str(&format!(
                "<tr><td>{}{extra}</td><td>{} gt</td><td>{}</td><td>{}..{}</td><td>{}</td></tr>\n",
                link_build(site, &f.build, prefix),
                esc(&f.period_text),
                esc(&f.blocks_text),
                f.n_lo,
                f.n_hi,
                if f.judged.is_empty() { "-".to_string() } else { f.judged.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ") }
            ));
        }
        body.push_str("</tbody></table></section>\n");
    }
    body.push_str("<section><h2>Every member</h2><p>Each member's design was generated with the family's own generator when this site was built. <span class=\"badge st-judged\">judged in the game</span> = verified by the clock judge in the real game; <span class=\"badge st-redsim\">simulated</span> = its generated design runs at exactly the formula's period in redsim (the exact simulator), not judged in the game; <span class=\"badge st-formula\">formula</span> = not individually checked.</p>\n");
    body.push_str("<table class=\"list\"><thead><tr><th>family</th><th>n</th><th>period</th><th>blocks</th><th>box</th><th>status</th><th>design</th></tr></thead><tbody>\n");
    for m in members {
        let page = member_href(m, site);
        let name = if m.build.is_some() { format!("<a href=\"{prefix}{}\">{}</a>", esc(&page), esc(&m.family)) } else { esc(&m.family) };
        body.push_str(&format!(
            "<tr id=\"n{}{}\"><td>{name}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td><span class=\"badge st-{}\">{}</span></td><td><a href=\"{prefix}{}\" download>.txt</a></td></tr>\n",
            m.n,
            if card.families.len() > 1 && card.families.first().is_some_and(|f| f.id != m.family) { format!("-{}", esc(&m.family)) } else { String::new() },
            m.n,
            esc(&gt_time(m.period)),
            m.blocks,
            esc(m.dims.as_deref().unwrap_or("")),
            esc(&m.status),
            esc(status_label(&m.status)),
            esc(&m.design)
        ));
    }
    body.push_str("</tbody></table></section>\n");
    let desc = format!("Extendable redstone clock family {}: {}", card.title, card.heading);
    shell(&card.title, &desc, prefix, &body, scripts, site)
}

/// families/index.html: every card, by its smallest period.
pub fn families_index(site: &Site, members: &BTreeMap<String, Vec<&Member>>, scripts: Scripts) -> String {
    let prefix = "../";
    let mut body = format!(
        "<p class=\"crumbs\"><a href=\"{prefix}index.html\">{SITE_NAME}</a> › Extendable families</p>\n<h1>Extendable clock families</h1>\n<p class=\"lede\">{} families grow by a repeated step: one design for every n, its period given by a formula. Search a period in the <a href=\"{prefix}index.html#kind=clock&amp;time=\">clock time search</a> to see the exact member that hits it.</p>\n",
        site.cards.len()
    );
    body.push_str("<table class=\"list\"><thead><tr><th>card</th><th>period</th><th>families</th><th>members</th><th>periods reached</th></tr></thead><tbody>\n");
    for c in &site.cards {
        let ms = members.get(&c.id).map(|v| v.as_slice()).unwrap_or(&[]);
        let (lo, hi) = (ms.iter().map(|m| m.period).min(), ms.iter().map(|m| m.period).max());
        let reach = match (lo, hi) {
            (Some(l), Some(h)) => format!("{} .. {}", gt_time(l), gt_time(h)),
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
    body.push_str("</tbody></table>\n");
    shell("Extendable clock families", "Every extendable redstone clock family with its period formula", prefix, &body, scripts, site)
}

/// index.html: the filtering UI's shell (the UI itself is pkg/redstonepedia.js).
pub fn index_page(site: &Site, scripts: Scripts) -> String {
    let n = |k: &str| site.builds.iter().filter(|b| b.kind == k).count();
    let body = format!(
        "<section class=\"hero\"><h1>{SITE_NAME}</h1><p class=\"lede\">Every redstone build of <a href=\"{REPO_URL}\">minecraft-tas</a>, verified tick by tick in the real game: {} piston doors, {} clocks ({} extendable families, {} generated members){}. Filter by any property, or find a clock by its period.</p></section>\n\
         <div id=\"app\" class=\"app\"><aside id=\"filters\" class=\"filters\"><p class=\"loading\">Loading the filters…</p></aside>\
         <div class=\"results\"><div id=\"summary\" class=\"summary\"></div><div id=\"grid\" class=\"grid\"></div><p class=\"more-wrap\"><button id=\"more\" type=\"button\" hidden>Show more</button></p></div></div>\n\
         <noscript><p class=\"notice\">The filters need JavaScript and WebAssembly. Without them, browse <a href=\"all.html\">the list of everything</a>.</p></noscript>\n",
        n("door"),
        n("clock"),
        site.cards.len(),
        site.members.len(),
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

/// about.html.
pub fn about_page(site: &Site, scripts: Scripts) -> String {
    let body = format!(
        "<h1>About {SITE_NAME}</h1>\n\
         <p>{SITE_NAME} lists every build of <a href=\"{REPO_URL}\">vjeux/minecraft-tas</a>: a headless Minecraft 26.3 server that runs the game's own code one tick at a time, many worlds at once, and the exact Rust redstone simulator redsim built against it. Night after night, agents searched for the smallest 3x3 piston doors, then for every kind of redstone clock, then for the classic contraptions (pulse circuits, T flip-flops, delay lines, logic gates, memory, piston extenders, flying machines).</p>\n\
         <p>Nothing here is a screenshot of someone's world: each build is a design file (blocks, layer by layer) that the project's judge verified in the real game, at several positions and rotations. The 3D views are rendered live in your browser from those files, and simulated with redsim compiled to WebAssembly.</p>\n\
         <h2>Where the numbers come from</h2>\n<ul>\n\
         <li><strong>Doors</strong>: <code>mctas door verify --extra-offsets 28</code> (3 open/close cycles at 32 positions); size is the piston door community's volume (the bounding box of the build over a whole cycle, the lever excluded); seamless tiers, hallway and record eligibility follow the community regulation (<a href=\"{REPO_URL}/blob/main/docs/rules.md\">docs/rules.md</a>).</li>\n\
         <li><strong>Clocks</strong>: <code>mctas clock verify</code> (4 positions x 4 rotations in the real game); the period is the exact repeat of the whole state; the family is the set of block kinds that change state (<a href=\"{REPO_URL}/blob/main/docs/clocks.md\">docs/clocks.md</a>). A variant is a clock of a period that only adds block kinds to a smaller one. Extendable families give a formula for every n; their members are generated by the family's own generator, and each is marked as judged in the game, simulated in redsim, or given by the formula only.</li>\n\
         </ul>\n\
         <h2>Data</h2><p>Everything on this site is in <a href=\"data/site.json\">data/site.json</a> ({} builds, {} family members), regenerated by <code>mctas site build</code> from the repository.</p>\n",
        site.builds.len(),
        site.members.len()
    );
    shell("About", "What REDSTONEPEDIA is and where its numbers come from", "", &body, Scripts { viewer: false, ..scripts }, site)
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
        let p = build_page(&s, &s.builds[0], "name: obs2\n", "designs/clocks/archive/obs2.txt", Scripts { viewer: true, app: false });
        assert!(p.starts_with("<!DOCTYPE html>"));
        assert!(p.contains("<title>obs2 · REDSTONEPEDIA</title>"));
        assert!(p.contains("data-mcview=\"full\" data-src=\"../designs/clocks/obs2.txt\" data-kind=\"clock\""), "{p}");
        assert!(p.contains("Two observers face to face &lt;3"));
        assert!(p.contains("<a class=\"card\" href=\"../clocks/obs3.html\""), "similar builds link to the other clock of the period");
        assert!(p.contains("import init from \"../viewer/mcview.js\""));
        assert!(!p.contains("pkg/redstonepedia.js"));
        let d = build_page(&s, &s.builds[2], "name: fc2\n", "designs/piston-door-3x3/archive/fc2-40b.txt", Scripts::default());
        assert!(d.contains("144b (8x9x2)"));
        assert!(d.contains("14 / 16 gt"));
        assert!(!d.contains("mcview.js"), "no viewer script without the bundle");
    }

    #[test]
    fn index_loads_the_app() {
        let s = tiny_site();
        let p = index_page(&s, Scripts { viewer: true, app: false });
        assert!(p.contains("import init from \"./pkg/redstonepedia.js\""));
        assert!(p.contains("id=\"grid\""));
        assert!(p.contains("1 piston doors, 2 clocks"));
    }
}
