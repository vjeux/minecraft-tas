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
            // `rising-edge · L = 2 gt · 3 blocks`, `flying-machine · up · 10 gt/block · 6 blocks`:
            // the axis value whenever the kind does not already say it
            Some(c) => {
                let mut parts = vec![b.group.clone()];
                if !c.direction.is_empty() {
                    parts.push(c.direction.clone());
                }
                if !c.axis.is_empty() && c.axis != c.kind && !c.kind.ends_with(c.axis.as_str()) && !c.axis.starts_with("detects") {
                    if let Some(a) = axis_shown(&c.kind, &c.axis) {
                        parts.push(a);
                    }
                }
                // a machine's speed: a shulker loader's items per second
                if let Some((_, v)) = c.metrics.iter().find(|(n, _)| n == "items/s") {
                    parts.push(format!("{v} items/s"));
                }
                parts.push(blocks_word(b.blocks));
                parts.join(" · ")
            }
            None => format!("{} · {}", b.group, blocks_word(b.blocks)),
        },
    }
}

/// `1 block`, `3 blocks`.
/// The axis as a card shows it: `L = 12 gt` of `off L = 12 gt` or `L = 12 gt retriggerable` (the
/// kind already says on / off / retriggerable), nothing when the kind already carries the number
/// (`divider 3` with `N = 3`).
pub fn axis_shown(kind: &str, axis: &str) -> Option<String> {
    let Some((head, value)) = axis.split_once(" = ") else {
        return Some(axis.to_string());
    };
    let letter = head.split_whitespace().last().unwrap_or(head);
    let mut v = value.split_whitespace();
    let n = v.next().unwrap_or("");
    let unit = match v.next() {
        Some("gt") => " gt",
        _ => "",
    };
    if kind.split_whitespace().nth(1) == Some(n) {
        return None;
    }
    Some(format!("{letter} = {n}{unit}"))
}

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
        // the machines' variations: a shulker loader's badges, what a BUD switch detects
        let m = |k: &str| c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let badges: Vec<&str> = m("badges").split('+').filter(|s| !s.is_empty()).collect();
        for x in badges.iter().take(4) {
            v.push(x.to_string());
        }
        if badges.len() > 4 {
            v.push(format!("+{} more", badges.len() - 4));
        }
        let detects = m("detects");
        if !detects.is_empty() {
            v.push(format!("detects {}", detects.split_whitespace().collect::<Vec<_>>().join(", ")));
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

/// A circuit kind's base in words, singular (vjeux 12:15Z: "Circuit kind should be piston
/// extender, not one per N"): `piston-extender 5` -> `Piston extender`, `divider 3` ->
/// `Divider`, `gate and` -> `Logic gate`. The number is a property, not a kind.
pub fn kind_words(kind: &str) -> String {
    let base = kind.split_whitespace().next().unwrap_or("");
    let words = match base {
        "rising-edge" => "Rising-edge detector",
        "falling-edge" => "Falling-edge detector",
        "dual-edge" => "Dual-edge detector",
        "pulse-limiter" => "Pulse limiter",
        "pulse-extender" => "Pulse extender",
        "tff" => "T flip-flop",
        "tff-reset" => "T flip-flop with reset",
        "delay" => "Delay line",
        "gate" => "Logic gate",
        "rs-latch" => "RS latch",
        "d-latch" => "D latch",
        "dff" => "D flip-flop",
        "dff-enable" => "D flip-flop with enable",
        "shift" => "Shift register",
        "counter" => "Counter",
        "adder" => "Adder",
        "piston-extender" => "Piston extender",
        "flying-machine" => "Flying machine",
        "shulker-loader" => "Shulker loader",
        "shulker-unloader" => "Shulker unloader",
        "bud" => "BUD switch",
        "tnt-duper" => "TNT duper",
        "tnt-duper-triggered" => "TNT duper (per pulse)",
        "burst" => "Burst",
        "divider" => "Divider",
        "monostable" => "Monostable",
        "off-delay" => "Off-delay timer",
        "on-delay" => "On-delay timer",
        "pulse-train" => "Pulse train",
        _ => "",
    };
    if !words.is_empty() {
        return words.to_string();
    }
    // a kind this list does not know yet: its base, dashes as spaces, capitalized
    let s = base.replace('-', " ");
    let mut ch = s.chars();
    ch.next().map(|f| f.to_uppercase().collect::<String>() + ch.as_str()).unwrap_or_default()
}

/// What a card leads with (vjeux 12:09Z: "put the focus on what are the properties of the build"):
/// the kind in words (small, above), the headline (the study's main property and the block
/// count), a line of secondary properties and chips; the design's name is only small secondary
/// text.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Props {
    pub kind: String,
    pub title: String,
    pub sub: String,
    pub chips: Vec<String>,
}

/// A circuit metric's value (`""` when absent).
fn cm<'a>(c: &'a crate::Circuit, k: &str) -> &'a str {
    c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// `label value` when the value exists.
fn labeled(label: &str, v: &str) -> Option<String> {
    let v = v.trim();
    if v.is_empty() { None } else if label.is_empty() { Some(v.to_string()) } else { Some(format!("{label} {v}")) }
}

/// A circuit's main property, without its kind (`N = 3`, `÷3 · square`, `AND · instant`;
/// empty when the kind has no number: a latch), and its secondary ones, from the judge's metrics.
fn circuit_props(c: &crate::Circuit) -> (String, Vec<String>) {
    let mut w = c.kind.split_whitespace();
    let (k0, k1) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
    let l = cm(c, "L");
    let short = |s: &str| s.trim_end_matches(" (from the press)").to_string();
    let sub = |keys: &[(&str, &str)]| -> Vec<String> { keys.iter().filter_map(|(label, k)| labeled(label, cm(c, k))).collect() };
    match k0 {
        "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" => (format!("L = {l}"), sub(&[("delay", "delay"), ("min-pulse", "min-pulse")])),
        "tff" => (c.axis.replace('+', " + ").replace('_', " "), sub(&[("latency", "latency"), ("fastest", "min-period")])),
        "delay" => (format!("N = {}", if cm(c, "N").is_empty() { c.axis.trim_start_matches("N = ").to_string() } else { cm(c, "N").to_string() }), sub(&[("min-pulse", "min-pulse"), ("min-gap", "min-gap")])),
        "gate" => {
            let f = if cm(c, "function").is_empty() { k1 } else { cm(c, "function") };
            let instant = cm(c, "class") == "instant" || cm(c, "latency") == "0 gt";
            (format!("{}{}", f.to_uppercase(), if instant { " · instant" } else { "" }), sub(&[("latency", "latency")]))
        }
        "rs-latch" | "d-latch" | "dff" | "dff-enable" | "tff-reset" => (String::new(), sub(&[("latency", "latency"), ("enable", "enable"), ("edge", "edge"), ("reset", "reset")])),
        "shift" => (format!("{k1}-bit"), sub(&[("latency", "latency")])),
        "adder" => (format!("{k1}-bit"), sub(&[("latency", "latency")])),
        "counter" => (format!("N = {k1}"), sub(&[("", "output"), ("latency", "latency")])),
        "piston-extender" => {
            let (e, r) = (short(cm(c, "extend")), short(cm(c, "retract")));
            let timing = if e.is_empty() { None } else { Some(format!("extend {e} / retract {r}")) };
            (format!("N = {k1}"), timing.into_iter().collect())
        }
        "flying-machine" => {
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            let dir = if two_way { "two-way".to_string() } else { c.direction.clone() };
            let speed = cm(c, "speed");
            ([dir, speed.to_string()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), sub(&[("latency", "latency")]))
        }
        "shulker-loader" | "shulker-unloader" => {
            let stack = match k1 {
                "16" => "16-stack · ",
                "1" => "unstackable · ",
                _ => "",
            };
            let rate = cm(c, "items/s");
            (if rate.is_empty() { stack.trim_end_matches(" · ").to_string() } else { format!("{stack}{rate} items/s") }, sub(&[("first box", "first box")]))
        }
        "bud" => {
            let d: Vec<&str> = cm(c, "detects").split_whitespace().collect();
            (if d.is_empty() { String::new() } else { format!("detects {}", d.join(", ")) }, sub(&[("latency", "latency"), ("re-arm", "re-arm"), ("footprint", "footprint")]))
        }
        "tnt-duper" => (cm(c, "rate").to_string(), sub(&[("period", "period"), ("latency", "latency")])),
        "tnt-duper-triggered" => (format!("{} TNT per pulse", if cm(c, "per pulse").is_empty() { "1" } else { cm(c, "per pulse") }), sub(&[("latency", "latency"), ("gap", "gap")])),
        "burst" => (format!("×{k1}"), sub(&[("every", "period"), ("latency", "latency")])),
        "divider" => (format!("÷{k1}{}", if cm(c, "output") == "square" { " · square" } else { "" }), sub(&[("latency", "latency"), ("fastest clock", "min-period")])),
        "monostable" => (format!("L = {l}{}", if k1 == "retriggerable" { " · retriggerable" } else if k1 == "non-retriggerable" { " · non-retriggerable" } else { "" }), sub(&[("delay", "delay"), ("re-arm", "re-arm")])),
        "off-delay" => (format!("L = {l}"), sub(&[("latency", "latency"), ("min-pulse", "min-pulse")])),
        "on-delay" => (format!("T = {}", cm(c, "T")), sub(&[("shortening", "shortening"), ("latency", "latency")])),
        "pulse-train" => (format!("P = {}", cm(c, "P")), sub(&[("pulse", "pulse"), ("latency", "latency")])),
        _ => {
            let axis = if c.axis == c.kind { String::new() } else { axis_shown(&c.kind, &c.axis).unwrap_or_default() };
            let n = if k1.is_empty() || axis.contains(k1) { String::new() } else { k1.to_string() };
            ([n, axis].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), sub(&[("latency", "latency")]))
        }
    }
}

/// The card of a build, by its properties ([`Props`]).
pub fn props(b: &Build) -> Props {
    let flags = |list: &[&str]| -> Vec<String> { list.iter().filter(|f| b.flags.iter().any(|x| x == *f)).map(|f| f.to_string()).collect() };
    match b.kind.as_str() {
        "door" => {
            let mut chips: Vec<String> = Vec::new();
            if let Some(d) = &b.door {
                if let Some(l) = d.leads.first() {
                    chips.push(format!("#1 {l}"));
                }
            }
            // (the category is in the kind line)
            if let Some(d) = &b.door {
                if !d.seamless.is_empty() {
                    chips.push(format!("{} seamless", d.seamless));
                }
            }
            chips.extend(flags(&["slime-free", "flat", "rotation-safe", "eligible", "variant"]));
            let sub = b.door.as_ref().and_then(|d| Some(format!("open {} gt · close {} gt", d.open?, d.close?))).unwrap_or_default();
            Props { kind: format!("Piston door · {}", b.group), title: size_line(b).replace('x', "×"), sub, chips }
        }
        "clock" => {
            let c = b.clock.as_ref();
            let period = c.map(|c| gt_time(c.period)).unwrap_or_default();
            let pulse = c.and_then(|c| match c.edges {
                0 => None,
                1 if c.on > 0 => Some(format!("on {} gt", c.on)),
                1 => None,
                e => Some(format!("{e} pulses a cycle")),
            });
            let sub = [pulse, Some(b.group.replace('+', " + "))].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            Props { kind: "Clock".into(), title: format!("{period} · {}", blocks_word(b.blocks)), sub, chips: flags(&["flat", "slime-free", "self-starting", "extendable", "composite", "variant"]) }
        }
        _ => match &b.circuit {
            Some(c) => {
                let (main, sub) = circuit_props(c);
                let mut chips: Vec<String> = Vec::new();
                if !c.span.is_empty() {
                    chips.push(format!("tunable: {}", c.span));
                }
                chips.extend(flags(&["button", "lever", "slime-free", "flat", "extendable", "variant", "layout"]));
                // the box's shape: 1 high (a flat 2-D design) or 1 wide (tiles side by side)
                if let Some(d) = &b.dims {
                    let n: Vec<u32> = d.split('x').filter_map(|v| v.trim().parse().ok()).collect();
                    if n.len() == 3 && !chips.iter().any(|c| c == "flat") {
                        if n[1] == 1 {
                            chips.push("1 high".into());
                        } else if n[0] == 1 || n[2] == 1 {
                            chips.push("1 wide".into());
                        }
                    }
                }
                let badges: Vec<&str> = cm(c, "badges").split('+').filter(|s| !s.is_empty()).collect();
                chips.extend(badges.iter().take(4).map(|s| s.to_string()));
                if badges.len() > 4 {
                    chips.push(format!("+{} more", badges.len() - 4));
                }
                let mut sub = sub;
                if !c.direction.is_empty() && c.kind != "flying-machine" {
                    sub.insert(0, c.direction.clone());
                }
                let title = if main.is_empty() { blocks_word(b.blocks) } else { format!("{main} · {}", blocks_word(b.blocks)) };
                Props { kind: kind_words(&c.kind), title, sub: sub.join(" · "), chips }
            }
            None => Props { kind: b.group.clone(), title: blocks_word(b.blocks), sub: String::new(), chips: flags(&["slime-free", "flat"]) },
        },
    }
}

/// A result card for a build: its kind (small, above), its properties as the headline, its name
/// small at the bottom (and in the tooltip). `prefix` is the path from the page to the site root
/// (`""`, `"../"`).
pub fn build_card(b: &Build, prefix: &str) -> String {
    let p = props(b);
    let chips: String = p.chips.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect();
    format!(
        "<a class=\"card\" href=\"{}\" title=\"{}\">{}<div class=\"meta\"><div class=\"kind\">{}</div><div class=\"title\">{}</div>{}<div class=\"badges\">{chips}</div><div class=\"name\">{}</div></div></a>",
        esc(&format!("{prefix}{}", b.page)),
        esc(&b.name),
        thumb(prefix, &b.design, b.trace.as_deref(), b.picture.as_deref(), &b.kind, &headline(b), &fallback_line(b)),
        esc(&p.kind),
        esc(&p.title),
        if p.sub.is_empty() { String::new() } else { format!("<div class=\"sub\">{}</div>", esc(&p.sub)) },
        esc(&b.name)
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

/// A result card for an extendable family's member: `family, n = 10 → 48 gt, 24 blocks` (a
/// circuit family's member: its axis value, `N = 26 gt`).
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
    let circuit = !m.axis.is_empty();
    let (kind, big, value) = if circuit { ("circuit", m.axis.clone(), m.axis.clone()) } else { ("clock", format!("{} gt", m.period), gt_time(m.period)) };
    // the member's value and size lead; its family and n are the secondary text
    format!(
        "<a class=\"card member\" href=\"{}\" title=\"{} n = {}\">{}<div class=\"meta\"><div class=\"title\">{} · {}</div><div class=\"sub\">{}</div><div class=\"badges\"><span class=\"badge\">extendable</span>{status}</div><div class=\"name\">{} <span class=\"n\">n = {}</span></div></div></a>",
        esc(&format!("{prefix}{}", member_href(m, site))),
        esc(&m.family),
        m.n,
        thumb(prefix, &m.design, None, m.picture.as_deref(), kind, &big, &format!("{} n={}", m.family, m.n)),
        esc(&value),
        esc(&blocks_word(m.blocks)),
        esc(&group),
        esc(&m.family),
        m.n
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
        assert!(h.starts_with("<a class=\"card\" href=\"../clocks/x.html\" title=\"x&lt;1&gt;\">"), "{h}");
        assert!(h.contains("data-mcview=\"thumb\" data-src=\"../designs/clocks/x.txt\" data-kind=\"clock\""), "{h}");
        // the properties lead, the name is small text
        assert!(h.contains("<div class=\"title\">26 gt (1.3 s) · 6 blocks</div>"), "{h}");
        assert!(h.contains("<div class=\"sub\">observer + redstone_lamp</div>"), "{h}");
        assert!(h.contains("<div class=\"name\">x&lt;1&gt;</div>"), "{h}");
        assert!(h.contains("<span class=\"badge\">self-starting</span>"), "{h}");
    }

    #[test]
    fn circuit_cards_lead_with_properties() {
        let ext = Build {
            kind: "circuit".into(),
            name: "pext-n3-flat-cascade19".into(),
            blocks: 19,
            kinds: vec!["sticky_piston".into()],
            flags: vec!["button".into(), "flat".into()],
            circuit: Some(crate::Circuit { kind: "piston-extender 3".into(), axis: "N = 3".into(), metrics: vec![("N".into(), "3".into()), ("extend".into(), "27 gt".into()), ("retract".into(), "56 gt (from the press)".into())], ..Default::default() }),
            ..Default::default()
        };
        let p = props(&ext);
        assert_eq!(p.kind, "Piston extender");
        assert_eq!(p.title, "N = 3 · 19 blocks");
        assert_eq!(p.sub, "extend 27 gt / retract 56 gt");
        assert_eq!(p.chips, vec!["button".to_string(), "flat".to_string()]);
        let div = Build { kind: "circuit".into(), blocks: 13, circuit: Some(crate::Circuit { kind: "divider 3".into(), metrics: vec![("output".into(), "square".into()), ("latency".into(), "37 gt".into())], ..Default::default() }), ..Default::default() };
        assert_eq!((props(&div).kind.as_str(), props(&div).title.as_str()), ("Divider", "÷3 · square · 13 blocks"));
        assert_eq!(kind_words("piston-extender 5"), "Piston extender");
        assert_eq!(kind_words("gate and"), "Logic gate");
        assert_eq!(kind_words("full-adder 4"), "Full adder");
    }
}
