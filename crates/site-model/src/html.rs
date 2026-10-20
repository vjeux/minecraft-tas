//! HTML fragments shared by the static pages (crates/site) and the browser UI (web/site): the
//! result cards are built by the same code on both sides, so a card looks the same in a build
//! page's "similar builds" and in the filter grid.

use crate::{Build, Hit, Member, Site, gt_time, status_label};

/// Text escaped for HTML content and double-quoted attribute values.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// The kind's display name, plural.
pub fn kind_title(kind: &str) -> &'static str {
    match kind {
        "door" => "Doors",
        "clock" => "Clocks",
        "circuit" => "Circuits",
        _ => "Builds",
    }
}

/// The size line of a build: `63b (7x9x1) · 43 blocks` (doors: the community's volume first).
pub fn size_line(b: &Build) -> String {
    match (b.kind.as_str(), b.volume, b.dims.as_deref()) {
        ("door", Some(v), Some(d)) => format!("{v}b ({d}) · {} blocks", b.blocks),
        ("door", Some(v), None) => format!("{v}b · {} blocks", b.blocks),
        _ => format!("{} blocks", b.blocks),
    }
}

/// The headline number of a build (the fallback card's big text): a door's volume, a clock's
/// period, a circuit's axis value.
pub fn headline(b: &Build) -> String {
    if let Some(c) = &b.clock {
        return format!("{} gt", c.period);
    }
    if let Some(c) = &b.circuit {
        return c.axis.clone();
    }
    match b.volume {
        Some(v) => format!("{v}b"),
        None => format!("{} blocks", b.blocks),
    }
}

/// The card's second line.
pub fn subline(b: &Build) -> String {
    match b.kind.as_str() {
        "door" => {
            let mut s = format!("{} · {}", size_line(b), b.group);
            if let Some(d) = &b.door {
                if let (Some(o), Some(c)) = (d.open, d.close) {
                    s.push_str(&format!(" · {o}/{c} gt"));
                }
            }
            s
        }
        "clock" => {
            let p = b.clock.as_ref().map(|c| gt_time(c.period)).unwrap_or_default();
            format!("{p} · {} · {}", blocks_word(b.blocks), b.group.replace('+', " + "))
        }
        _ => match &b.circuit {
            // `rising-edge · L = 2 gt · 3 blocks`
            Some(c) if c.axis.contains('=') => format!("{} · {} · {}", b.group, c.axis, blocks_word(b.blocks)),
            _ => format!("{} · {}", b.group, blocks_word(b.blocks)),
        },
    }
}

/// `1 block`, `3 blocks`.
pub fn blocks_word(n: u32) -> String {
    if n == 1 { "1 block".to_string() } else { format!("{n} blocks") }
}

/// Badges shown on a card (short).
pub fn badges(b: &Build) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(d) = &b.door {
        if !d.leads.is_empty() {
            v.push(format!("#1 {}", d.leads[0]));
        }
        if !d.seamless.is_empty() {
            v.push(d.seamless.clone());
        }
    }
    for f in ["slime-free", "flat", "eligible", "extendable", "self-starting", "variant", "composite", "layout"] {
        if b.flags.iter().any(|x| x == f) {
            v.push(f.to_string());
        }
    }
    // a tunable group's shown design: the group's span (`tunable: L 6-40 gt, 16 values`)
    if let Some(c) = &b.circuit {
        if !c.span.is_empty() {
            v.push(format!("tunable: {}", c.span));
        }
    }
    v
}

fn thumb(prefix: &str, design: &str, trace: Option<&str>, picture: Option<&str>, kind: &str, big: &str, small: &str) -> String {
    let trace = trace.map(|t| format!(" data-trace=\"{}\"", esc(&format!("{prefix}{t}")))).unwrap_or_default();
    // the interim picture (until the 3D viewer renders): lazy, under the headline label
    let (pic, cls) = match picture {
        Some(p) => (format!("<img class=\"pic\" src=\"{}\" alt=\"\" loading=\"lazy\" decoding=\"async\">", esc(p)), " has-pic"),
        None => (String::new(), ""),
    };
    // no design file (a member listed from its formula only): no 3D view
    let view = if design.is_empty() { String::new() } else { format!(" data-mcview=\"thumb\" data-src=\"{}\"{trace} data-kind=\"{kind}\"", esc(&format!("{prefix}{design}"))) };
    format!(
        "<div class=\"thumb k-{kind}\"{view}><div class=\"fallback{cls}\">{pic}<span class=\"fb-big\">{}</span><span class=\"fb-small\">{}</span></div></div>",
        esc(big),
        esc(small)
    )
}

/// The fallback card's small line (under the headline): a door's box and category, a clock's
/// family.
pub fn fallback_line(b: &Build) -> String {
    match (b.kind.as_str(), &b.dims) {
        ("door", Some(d)) => format!("{d} · {}", b.group),
        _ => b.group.replace('+', " + "),
    }
}

/// A result card for a build. `prefix` is the path from the page to the site root (`""`,
/// `"../"`).
pub fn build_card(b: &Build, prefix: &str) -> String {
    let badges: String = badges(b).iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect();
    format!(
        "<a class=\"card\" href=\"{}\">{}<div class=\"meta\"><div class=\"title\">{}</div><div class=\"sub\">{}</div><div class=\"badges\">{badges}</div></div></a>",
        esc(&format!("{prefix}{}", b.page)),
        thumb(prefix, &b.design, b.trace.as_deref(), b.picture.as_deref(), &b.kind, &headline(b), &fallback_line(b)),
        esc(&b.name),
        esc(&subline(b))
    )
}

/// The anchor of a member's row on its family card's page: `n<k>` for the card's first family,
/// `n<k>-<family>` for the others.
pub fn member_anchor(m: &Member, site: &Site) -> String {
    let first = site.cards.iter().find(|c| c.id == m.card).and_then(|c| c.families.first()).map(|f| f.id.as_str());
    if first.is_none_or(|f| f == m.family) { format!("n{}", m.n) } else { format!("n{}-{}", m.n, m.family) }
}

/// The page a member links to: its archived build, else its family's card at its row.
pub fn member_href(m: &Member, site: &Site) -> String {
    let card = || format!("families/{}.html#{}", m.card, member_anchor(m, site));
    match &m.build {
        Some(id) => site.builds.iter().find(|b| &b.id == id).map(|b| b.page.clone()).unwrap_or_else(card),
        None => card(),
    }
}

/// A result card for an extendable family's member: `family, n = 10 → 48 gt, 24 blocks`.
pub fn member_card(m: &Member, site: &Site, prefix: &str) -> String {
    let group = site
        .cards
        .iter()
        .find(|c| c.id == m.card)
        .and_then(|c| c.families.iter().find(|f| f.id == m.family))
        .and_then(|f| site.builds.iter().find(|b| b.id == f.build))
        .map(|b| b.group.replace('+', " + "))
        .unwrap_or_default();
    let status = format!("<span class=\"badge st-{}\">{}</span>", esc(&m.status), esc(status_label(&m.status)));
    format!(
        "<a class=\"card member\" href=\"{}\">{}<div class=\"meta\"><div class=\"title\">{} <span class=\"n\">n = {}</span></div><div class=\"sub\">{} · {} blocks · {}</div><div class=\"badges\"><span class=\"badge\">extendable</span>{status}</div></div></a>",
        esc(&format!("{prefix}{}", member_href(m, site))),
        thumb(prefix, &m.design, None, m.picture.as_deref(), "clock", &format!("{} gt", m.period), &format!("{} n={}", m.family, m.n)),
        esc(&m.family),
        m.n,
        esc(&gt_time(m.period)),
        m.blocks,
        esc(&group)
    )
}

/// The card of a query hit.
pub fn hit_card(site: &Site, h: Hit, prefix: &str) -> String {
    match h {
        Hit::Build(i) => site.builds.get(i).map(|b| build_card(b, prefix)).unwrap_or_default(),
        Hit::Member(i) => site.members.get(i).map(|m| member_card(m, site, prefix)).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping() {
        assert_eq!(esc("a<b>&\"c'"), "a&lt;b&gt;&amp;&quot;c&#39;");
    }

    #[test]
    fn card_markup() {
        let b = Build {
            id: "clocks/x".into(),
            kind: "clock".into(),
            group: "observer+redstone_lamp".into(),
            name: "x<1>".into(),
            page: "clocks/x.html".into(),
            design: "designs/clocks/x.txt".into(),
            blocks: 6,
            clock: Some(crate::ClockInfo { period: 26, ..Default::default() }),
            flags: vec!["self-starting".into()],
            ..Default::default()
        };
        let h = build_card(&b, "../");
        assert!(h.starts_with("<a class=\"card\" href=\"../clocks/x.html\">"), "{h}");
        assert!(h.contains("data-mcview=\"thumb\" data-src=\"../designs/clocks/x.txt\" data-kind=\"clock\""), "{h}");
        assert!(h.contains("x&lt;1&gt;"), "{h}");
        assert!(h.contains("26 gt (1.3 s) · 6 blocks · observer + redstone_lamp"), "{h}");
        assert!(h.contains("<span class=\"badge\">self-starting</span>"), "{h}");
    }
}
