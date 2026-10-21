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
        ("door", Some(v), Some(d)) => format!("{v}b ({d}) · {}", blocks_word(b.blocks)),
        ("door", Some(v), None) => format!("{v}b · {}", blocks_word(b.blocks)),
        _ => blocks_word(b.blocks),
    }
}

/// The headline number of a build (the fallback card's big text): a door's volume, a clock's
/// period, a circuit's axis value.
pub fn headline(b: &Build) -> String {
    // the big text over a picture or a loading view: in words, like the card's title (critique:
    // "N = 2 gt" badges over thumbnails whose card says "2-tick delay")
    if let Some(c) = &b.clock {
        return format!("every {} ticks", c.every());
    }
    if let Some(c) = &b.circuit {
        let (main, _) = circuit_props(c);
        return if main.is_empty() { kind_words(&c.kind) } else { main.split(" · ").next().unwrap_or(&main).to_string() };
    }
    match &b.dims {
        Some(d) => d.replace('x', "×"),
        None => blocks_word(b.blocks),
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
            let p = b.clock.as_ref().map(|c| gt_time(c.every())).unwrap_or_default();
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
            v.push(door_lead_words(&d.leads[0], &d.seamless));
        }
        if !d.seamless.is_empty() {
            v.push(seamless_words(&d.seamless));
        }
    }
    for (f, words) in [("slime-free", "no slime"), ("flat", "1 block thick"), ("eligible", "record-eligible"), ("extendable", "comes in every size"), ("self-starting", "starts by itself"), ("variant", "a bigger version of another design"), ("composite", "several clocks in one"), ("layout", "another layout")] {
        if b.flags.iter().any(|x| x == f) {
            v.push(words.to_string());
        }
    }
    // a tunable group's shown design: the group's span (`13 versions, 12 to 36 ticks`)
    if let Some(c) = &b.circuit {
        if !c.span.is_empty() {
            v.push(span_words(&c.span));
        }
        // the machines' variations: a shulker loader's badges, what a BUD switch detects
        let m = |k: &str| c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("");
        let badges: Vec<&str> = m("badges").split('+').filter(|s| !s.is_empty()).collect();
        for x in badges.iter().take(4) {
            v.push(badge_words(x));
        }
        if badges.len() > 4 {
            v.push(format!("+{} more", badges.len() - 4));
        }
        // (what a BUD switch notices is its headline)
    }
    v
}

pub fn thumb(prefix: &str, design: &str, trace: Option<&str>, picture: Option<&str>, kind: &str, big: &str, small: &str) -> String {
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
        ("door", Some(d)) => format!("{} · {}", d.replace('x', "×"), door_category(&b.group)),
        ("clock", _) => format!("made of {}", human_blocks(&b.group)),
        _ => b.group.clone(),
    }
}

/// A circuit kind's base in words, singular (vjeux 12:15Z: "Circuit kind should be piston
/// extender, not one per N"), in everyday words (13:06Z: "I have no clue what a monostable
/// retriggerable system means"): `piston-extender 5` -> `Piston extender`, `monostable
/// retriggerable` -> `One-shot timer`, `bud` -> `Block change detector`. The number is a
/// property, not a kind; the jargon name is in the kind's title (crates/site explain::kind_title).
pub fn kind_words(kind: &str) -> String {
    let base = kind.split_whitespace().next().unwrap_or("");
    let words = match base {
        "rising-edge" => "One pulse when switched on",
        "falling-edge" => "One pulse when switched off",
        "dual-edge" => "One pulse on every switch",
        "pulse-limiter" => "Signal shortener",
        "pulse-extender" => "Signal stretcher",
        "tff" => "Toggle switch",
        "tff-reset" => "Toggle switch with reset",
        "delay" => "Delay",
        "gate" => "Logic gate",
        "rs-latch" => "On/off memory",
        "d-latch" => "Copy-and-hold memory",
        "dff" => "Clocked memory",
        "dff-enable" => "Clocked memory with enable",
        "shift" => "Shift register",
        "counter" => "Counter",
        "adder" => "Adder",
        "piston-extender" => "Piston extender",
        "flying-machine" => "Flying machine",
        "shulker-loader" => "Shulker box loader",
        "shulker-unloader" => "Shulker box unloader",
        "bud" => "Block change detector",
        "tnt-duper" => "TNT duper",
        "tnt-duper-triggered" => "TNT duper, one per press",
        "burst" => "Pulses per press",
        "divider" => "Clock slower",
        "monostable" => "One-shot timer",
        "off-delay" => "Stay-on timer",
        "on-delay" => "Hold-to-activate",
        "pulse-train" => "Rapid fire while on",
        "sorter" => "Item sorter",
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

/// A circuit's value in words, from its axis (`L = 12 gt` -> `12 ticks`; a piston extender's
/// `N = 3` -> `reach 3`; a counter's -> `every 3rd`): a picker chip, a member's value.
pub fn value_words(kind: &str, axis: &str) -> String {
    let v = axis.split_once(" = ").map(|(_, v)| v.trim()).unwrap_or(axis.trim());
    // `12 gt` or `12 gt retriggerable` (the kind's qualifier is the same for every version)
    let w: Vec<&str> = v.split_whitespace().collect();
    if w.len() >= 2 && w[1] == "gt" {
        return tk(&format!("{} gt", w[0]));
    }
    // the value's number: the first numeric word (`counter 2` -> `2`)
    let n = v.split_whitespace().find(|t| t.chars().next().is_some_and(|c| c.is_ascii_digit())).unwrap_or(v);
    match kind.split_whitespace().next().unwrap_or("") {
        "piston-extender" => format!("reach {n}"),
        "counter" | "divider" => format!("every {}", nth(n)),
        "burst" => format!("{n} pulses"),
        "shift" | "adder" => format!("{n} bits"),
        "delay" => tk(&format!("{n} gt")),
        _ => v.to_string(),
    }
}

/// A machine's badge in words (`tileable(9)` -> `tiles every 9 blocks`, `tileable(1)` -> `tiles
/// side by side`, `top-up` -> `tops up part-full boxes`): short, for chips and card headlines
/// (critique: "restart", "top-up", "drop-free" need the variations table's words).
pub fn badge_words(badge: &str) -> String {
    match badge.split_once('(') {
        Some(("tileable", n)) => match n.trim_end_matches(')') {
            "1" => "tiles side by side".to_string(),
            n => format!("tiles every {n} blocks"),
        },
        _ => match badge {
            "16-stack" => "16-stack items too",
            "unstackable" => "unstackable items too",
            "mixed" => "packs mixed items",
            "filter" => "packs one kind, passes the rest",
            "tileable" => "tiles side by side",
            "always-on" => "always on, no lever",
            "restart" => "restarts by itself",
            "box-starved" => "waits out a box shortage",
            "colours" => "keeps box colours and names",
            "flush" => "sends out a part-full box",
            "top-up" => "tops up part-full boxes",
            "drop-free" => "drops nothing on the ground",
            "combo" => "loads and unloads",
            "any-box" => "empties any box",
            // (an item sorter's: a full chest blocks nothing)
            "overflow" => "a full chest never jams the line",
            other => other,
        }
        .to_string(),
    }
}

/// A tunable group's range as a card's headline (`every 17th to 576th pulse`, `on for 12 to 36
/// ticks · restarts on every press`, `reach 2 to 4`), from its span (`L 12-36 gt, 5 values`);
/// None for a design that is no group's card or whose span is no range.
fn group_range(c: &crate::Circuit) -> Option<String> {
    // the flying machine card (the coordinator 21:55Z: one card, a table of direction and load)
    if c.kind.starts_with("flying-machine") && c.tunable.starts_with("kind:") && !c.span.is_empty() {
        return Some("flies sideways, up or down, with cargo or trailers".into());
    }
    // one machine timed with several kinds of items (critique: three cards for 1-, 16- and
    // 64-stack items): one card that works with any of them
    if c.kind.starts_with("shulker-") && !c.span.is_empty() {
        // a card that leads with what it adds keeps its own title (critique: "works with any
        // stack size" stopped saying anything once a smaller loader did it too)
        if !c.lead.is_empty() {
            return None;
        }
        // (with this card's own rate: critique, "card 1 has no items/s figure")
        let rate = c.metrics.iter().find(|(k, _)| k == "items/s").map(|(_, v)| format!(" · {v} items a second")).unwrap_or_default();
        return Some(format!("works with any stack size{rate}"));
    }
    let (range, _) = c.span.split_once(", ")?;
    let (lo, hi) = range.split_whitespace().find_map(|w| {
        let (a, b) = w.split_once('-')?;
        Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
    })?;
    let mut w = c.kind.split_whitespace();
    let (k0, k1) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
    // (with seconds: critique, "on for 12 to 36 ticks" had none)
    let ticks = format!("{lo} to {hi} ticks ({} to {})", crate::real_time(lo), crate::real_time(hi));
    Some(match k0 {
        "counter" | "divider" => format!("every {} to {} pulse", nth(&lo.to_string()), nth(&hi.to_string())),
        "piston-extender" => format!("reach {lo} to {hi}"),
        "burst" => format!("{lo} to {hi} pulses per press"),
        "shift" | "adder" => format!("{lo} to {hi} bits"),
        "rising-edge" | "falling-edge" | "dual-edge" => format!("{lo}- to {hi}-tick pulse"),
        "pulse-limiter" => format!("cuts pulses to {lo}–{hi} ticks, by version"),
        "pulse-extender" => format!("stretches pulses to {lo}–{hi} ticks, by version"),
        "delay" => format!("{lo}- to {hi}-tick delay"),
        "monostable" => format!("on for {ticks}{}", if k1 == "retriggerable" { " · restarts on every press" } else if k1 == "non-retriggerable" { " · ignores presses while on" } else { "" }),
        "off-delay" => format!("stays on {ticks} after"),
        "on-delay" => format!("must be held {ticks}"),
        "pulse-train" => format!("a pulse every {ticks}"),
        _ => return None,
    })
}

/// A tunable group's span in words: `L 12-36 gt, 13 values` -> `13 versions, 12 to 36 ticks`.
pub fn span_words(span: &str) -> String {
    let Some((range, count)) = span.split_once(", ") else {
        // not a range of values (`2 versions`): as it is
        return span.to_string();
    };
    let n = count.split_whitespace().next().unwrap_or("");
    // `L 12-36 gt` (a letter first) or `17-576` (no letter: a counter's count)
    let words: Vec<&str> = range.split_whitespace().collect();
    let start = if words.first().is_some_and(|w| w.starts_with(|c: char| c.is_ascii_digit())) { 0 } else { 1 };
    let r = words.get(start).copied().unwrap_or("");
    let unit = words.get(start + 1).copied().unwrap_or("");
    let r = match r.split_once('-') {
        Some((a, b)) => format!("{a} to {b}"),
        None => r.to_string(),
    };
    let unit = match unit {
        "gt" => " ticks".to_string(),
        "" => String::new(),
        other => format!(" {other}"),
    };
    if n.is_empty() { format!("versions {r}{unit}") } else { format!("{n} versions, {r}{unit}") }
}

/// The searches to offer when a text finds nothing (critique C12: "an empty search suggests the
/// nearest kinds instead of 0 results"): the kinds whose words share a word (or its first 4
/// letters) with the text, else the most asked ones; links into the index.
pub fn nearest_searches(text: &str) -> String {
    const SEARCHES: [(&str, &str); 14] = [
        ("toggle switch", "kind=circuit&q=toggle"),
        ("one-shot timer", "kind=circuit&q=one+shot"),
        ("stay-on timer", "kind=circuit&q=stay+on"),
        ("piston extender", "kind=circuit&q=piston+extender"),
        ("logic gate", "kind=circuit&q=logic+gate"),
        ("delay", "kind=circuit&q=delay"),
        ("counter", "kind=circuit&q=counter"),
        ("block change detector", "kind=circuit&q=bud"),
        ("flying machine", "kind=circuit&q=flying+machine"),
        ("TNT duper", "kind=circuit&q=tnt"),
        ("shulker box loader", "kind=circuit&q=shulker"),
        ("pulse shortener or stretcher", "kind=circuit&q=pulse"),
        ("3×3 piston door", "kind=door"),
        ("clock", "kind=clock"),
    ];
    let words: Vec<String> = text.to_lowercase().split_whitespace().map(String::from).collect();
    let close = |label: &str| {
        let l = label.to_lowercase();
        words.iter().any(|w| l.split_whitespace().any(|x| x == w || (w.len() >= 4 && x.len() >= 4 && x[..4] == w[..4])))
    };
    let mut picked: Vec<&(&str, &str)> = SEARCHES.iter().filter(|(l, _)| close(l)).collect();
    if picked.is_empty() {
        picked = SEARCHES.iter().filter(|(l, _)| ["toggle switch", "one-shot timer", "piston extender", "3×3 piston door", "clock"].contains(l)).collect();
    }
    picked.iter().map(|(l, f)| format!("<a href=\"#{}\">{}</a>", esc(f), esc(l))).collect::<Vec<_>>().join(" · ")
}

/// The words a player may search a build by, beyond its name and caption: the community's names
/// for its kind (critique C12: "toggle", "one shot", "hidden door" found nothing useful).
pub fn search_aliases(b: &Build) -> &'static str {
    match b.kind.as_str() {
        "door" => "3x3 piston door hidden secret entrance",
        "clock" => "clock pulser oscillator repeating",
        _ => match b.circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("")).unwrap_or("") {
            "rising-edge" => "rising edge detector pulse on press",
            "falling-edge" => "falling edge detector pulse on release",
            "dual-edge" => "dual edge detector both edges",
            "pulse-limiter" => "pulse limiter shortener",
            "pulse-extender" => "pulse extender stretcher",
            "tff" => "toggle t flip-flop tff on off switch",
            "tff-reset" => "toggle t flip-flop tff reset",
            "delay" => "delay line repeater",
            "gate" => "logic gate and or not xor nand nor xnor imply nimply",
            "rs-latch" => "rs latch sr latch set reset memory",
            "d-latch" => "d latch data memory",
            "dff" => "d flip-flop dff clocked memory",
            "dff-enable" => "d flip-flop dff enable",
            "shift" => "shift register",
            "counter" => "counter divider frequency divider",
            "piston-extender" => "piston extender double triple quadruple dpe tpe qpe",
            // (the coordinator 00:31Z: "payload" and "two-way" stay search words)
            "flying-machine" => "flying machine flier engine payload cargo two-way back and forth docking station return flat tall",
            "shulker-loader" => "shulker box loader filler packer",
            "shulker-unloader" => "shulker box unloader emptier",
            "bud" => "bud switch block update detector",
            "tnt-duper" => "tnt duper duplicator world eater precise air burst",
            "sorter" => "item sorter filter hopper sorting system storage",
            "tnt-duper-triggered" => "tnt duper duplicator cannon",
            "burst" => "burst pulse multiplier",
            "divider" => "clock divider frequency divider",
            "monostable" => "monostable one shot one-shot timer",
            "off-delay" => "off delay off-delay timer stay on",
            "on-delay" => "on delay on-delay hold to activate pulse filter",
            "pulse-train" => "pulse train rapid fire gated clock",
            _ => "",
        },
    }
}

/// The community's seamless tiers in words (properties.rs `seamless`: QUART = closed, nothing of
/// the machine shows on one side; SEMI = that and open; FULL = both sides, closed and open; SUPER
/// = FULL and every tick of the motion), short enough for a chip.
pub fn seamless_words(tier: &str) -> String {
    // the community regulation's tiers (piston-door properties.rs): QUART = closed, nothing of
    // the machine on one side; SEMI = QUART + the open doorway clean; FULL = QUART on both sides
    // + open clean; SUPER = FULL + clean while moving
    match tier {
        "SUPER" => "seamless: machine hidden from both sides, closed, open and while moving".to_string(),
        "FULL" => "seamless: machine hidden from both sides, closed and open".to_string(),
        "SEMI" => "machine hidden when open, and from one side when closed".to_string(),
        "QUART" => "machine hidden from one side when closed".to_string(),
        "" => String::new(),
        other => format!("{} seamless", other.to_lowercase()),
    }
}

/// A board a door leads, in words, with the door's own seamless tier for the seamless boards
/// (they rank the tier first, then the size: critique, "most compact hidden door" on a 41-block
/// row of a page about a 38-block door hidden when closed): `seamless` + `FULL` -> `most compact
/// fully seamless door`.
pub fn door_lead_words(board: &str, tier: &str) -> String {
    if let Some(rest) = board.strip_prefix("seamless") {
        let t = match tier {
            "SUPER" => "super-seamless",
            "FULL" => "fully seamless",
            "SEMI" => "half-seamless",
            "QUART" => "partly seamless",
            _ => "seamless",
        };
        let flat = if rest.contains("flat") { "1-block-thick " } else { "" };
        let slime = if rest.contains("no slime") { " with no slime" } else { "" };
        return format!("most compact {t} {flat}door{slime}");
    }
    lead_words(board)
}

/// A board a door leads, in words (`solid` -> `most compact stone door`, `sculk, not
/// rotation-safe` -> `most compact sculk door that only works facing one way`,
/// `fastest open (solid only)` -> `fastest stone door to open`).
pub fn lead_words(board: &str) -> String {
    let (base, one_way) = match board.strip_suffix(", not rotation-safe") {
        Some(b) => (b, true),
        None => (board, false),
    };
    let tail = if one_way { " that only works facing one way" } else { "" };
    if let Some(rest) = base.strip_prefix("fastest ") {
        let (what, scope) = rest.split_once(" (").map(|(w, s)| (w, s.trim_end_matches(')'))).unwrap_or((rest, ""));
        let what = match what {
            "open" => "to open",
            "close" => "to close",
            "open + close" => "to open and close",
            other => other,
        };
        let kind = if scope == "solid only" { "stone door" } else { "door" };
        return format!("fastest {kind} {what}{tail}");
    }
    // the size boards rank by the space a door takes, not its blocks (critique: "smallest hidden
    // door" on a 41-block card next to a 40-block one): "most compact"
    let what = match base {
        "solid" => "stone door",
        "sculk" => "sculk door",
        "gravity" => "gravity door (its doorway blocks are sand or gravel)",
        "entity" => "door that uses entities (a minecart or the like) as moving parts",
        "slime-free" => "door with no slime",
        "flat" => "1-block-thick door",
        "flat, no slime or honey" => "1-block-thick door with no slime",
        "seamless" => "hidden door",
        "seamless, flat, no slime or honey" => "hidden 1-block-thick door",
        "wall" => "door in a 3-thick wall",
        "ground" => "door that stands on flat ground (no digging below it)",
        "entities" => "door that uses entities (a minecart or the like) as moving parts",
        other => return format!("best: {other}{tail}"),
    };
    format!("most compact {what}{tail}")
}

/// What a chunk reload does to a clock, in a few words, where it matters (clockinfra's `reload`
/// column; the coordinator 19:20Z: a property, a chip where it matters): `stops`, `changes
/// period to 4 gt`, `fails: ...`; None for `ok`, `glitch then resumes` and unchecked.
pub fn reload_chip(reload: &str) -> Option<&'static str> {
    let what = reload.split(" in ").next().unwrap_or("").trim();
    if what == "stops" {
        Some("stops if its chunk unloads")
    } else if what.starts_with("changes period") {
        Some("period changes on chunk reload")
    } else if what.starts_with("fails") {
        Some("breaks on chunk reload")
    } else {
        None
    }
}

/// A TNT duper's rate in words, one number a builder would say (critique: "Lights 85.7 TNT/min"
/// next to the caption's "86 lit TNT a minute"): `85.7 TNT/min` -> `about 86 TNT a minute`,
/// `200 TNT/min` -> `200 TNT a minute`.
pub fn tnt_rate_words(rate: &str) -> String {
    let Some(n) = rate.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()) else { return rate.to_string() };
    let r = n.round();
    if (r - n).abs() < 1e-9 { format!("{r} TNT a minute") } else { format!("about {r} TNT a minute") }
}

/// Block ids for people: `observer+redstone_lamp` -> `observer, redstone lamp`.
pub fn human_blocks(ids: &str) -> String {
    ids.split('+').filter(|s| !s.is_empty()).map(|s| s.trim_start_matches("minecraft:").replace('_', " ")).collect::<Vec<_>>().join(", ")
}

/// A door's category in words (the kind line).
pub fn door_category(group: &str) -> &'static str {
    match group {
        "solid" => "stone door",
        "sculk" => "triggered by a sculk sensor",
        "gravity" => "sand or gravel door blocks",
        "entity" => "uses moving entities",
        _ => "",
    }
}

/// `32 gt` -> `32 ticks (1.6 s)` (critique C50: every tick count with its seconds); a range ->
/// `4 to 8 ticks`.
fn tk(v: &str) -> String {
    let v = v.trim().trim_end_matches(" (from the press)");
    // `over 30 gt` -> `over 30 ticks (1.5 s)` (critique: "handles presses over 30 gt apart")
    if let Some(rest) = v.strip_prefix("over ") {
        return format!("over {}", tk(rest));
    }
    let mut w = v.split_whitespace();
    let n = w.next().unwrap_or("");
    if w.next() != Some("gt") {
        return v.to_string();
    }
    match n.parse::<u32>() {
        Ok(1) => format!("1 tick ({})", crate::real_time(1)),
        Ok(x) => format!("{x} ticks ({})", crate::real_time(x)),
        // a range with its seconds too (critique: "4 to 6 ticks apart" had none)
        Err(_) => match n.split_once('-').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))) {
            Some((a, b)) => format!("{a} to {b} ticks ({} to {})", crate::real_time(a), crate::real_time(b)),
            None => format!("{} ticks", n.replace('-', " to ")),
        },
    }
}

/// `3` -> `3rd`.
fn nth(n: &str) -> String {
    let v: u32 = n.parse().unwrap_or(0);
    let suffix = match (v % 10, v % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{v}{suffix}")
}

/// A logic function by its rule (`nimply` -> `A AND NOT B`).
/// A circuit's sort value within its kind: a gate's function by how common it is (critique: "A AND
/// B" was 9th of 9), none for a shulker machine (its value is the stack size of its test items, so
/// they sort by blocks: critique, the smallest loader was card 2), else its main value.
pub fn sort_value(c: &crate::Circuit) -> Option<i64> {
    if c.kind.starts_with("gate ") {
        Some(gate_rank(c.kind.split_whitespace().nth(1).unwrap_or("")))
    } else if c.kind.starts_with("shulker-") {
        None
    } else {
        c.value
    }
}

/// A gate function's place in lists, the most common first (critique: "A AND B" was 9th of 9):
/// OR, AND, NOT, NOR, NAND, XOR, XNOR, then the one-sided ones.
pub fn gate_rank(f: &str) -> i64 {
    match f {
        "or" => 0,
        "and" => 1,
        "not" => 2,
        "nor" => 3,
        "nand" => 4,
        "xor" => 5,
        "xnor" => 6,
        "nimply" | "nimply-ba" => 7,
        "imply" | "imply-ba" => 8,
        "buffer" => 9,
        _ => 10,
    }
}

pub fn gate_label(f: &str) -> String {
    match f {
        "not" => "NOT".into(),
        "buffer" => "copy".into(),
        "and" => "A AND B".into(),
        "or" => "A OR B".into(),
        "xor" => "A XOR B (one, not both)".into(),
        "xnor" => "A same as B".into(),
        "nand" => "NOT (A AND B)".into(),
        "nor" => "NOT (A OR B)".into(),
        "imply" => "(NOT A) OR B".into(),
        "nimply" => "A AND (NOT B)".into(),
        "imply-ba" => "A OR (NOT B)".into(),
        "nimply-ba" => "B AND (NOT A)".into(),
        "not-a" => "NOT A".into(),
        "not-b" => "NOT B".into(),
        other => other.to_uppercase(),
    }
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

/// A circuit's main property in words, without its kind (`reach 3`, `slows by 3 · even on/off`,
/// `A AND B · instant`; empty when the kind has no number: a latch), and its secondary ones, from
/// the judge's metrics (vjeux 13:06Z: plain player language, no letters before meaning).
fn circuit_props(c: &crate::Circuit) -> (String, Vec<String>) {
    let mut w = c.kind.split_whitespace();
    let (k0, k1) = (w.next().unwrap_or(""), w.next().unwrap_or(""));
    let l = tk(cm(c, "L"));
    let num = |s: &str| s.split_whitespace().next().unwrap_or("").to_string();
    let said = |label: &str, k: &str| -> Option<String> {
        let v = cm(c, k);
        if v.is_empty() { None } else { Some(label.replace("{}", &tk(v))) }
    };
    let sub = |keys: &[(&str, &str)]| -> Vec<String> { keys.iter().filter_map(|(label, k)| said(label, k)).collect() };
    match k0 {
        // (an extendable family's base shows the family's range: critique, "30-tick pulse" badged
        // "comes in every size"; a card's lead, when it has one, follows)
        "rising-edge" | "falling-edge" | "dual-edge" => {
            let main = format!("{}-tick pulse", num(cm(c, "L")));
            let lead = if c.lead.is_empty() { String::new() } else { format!(" · {}", c.lead) };
            (format!("{main}{lead}"), sub(&[("starts after {}", "delay"), ("needs an input of at least {}", "min-pulse")]))
        }
        "pulse-limiter" => (format!("at most {l}"), sub(&[("starts after {}", "delay"), ("needs an input of at least {}", "min-pulse")])),
        "pulse-extender" => (format!("at least {l}"), sub(&[("starts after {}", "delay"), ("needs an input of at least {}", "min-pulse")])),
        "tff" => {
            // what sets it apart from the other toggles leads (critique: a 7-block card lost every
            // number to the 3-block one with nothing saying what it adds)
            let lead = if c.lead.is_empty() { String::new() } else { format!("{} · ", c.lead) };
            (format!("{lead}{}", said("reacts in {}", "latency").unwrap_or_default()), sub(&[("handles presses {} apart", "min-period")]))
        }
        "delay" => {
            let n = if cm(c, "N").is_empty() { num(c.axis.trim_start_matches("N = ")) } else { num(cm(c, "N")) };
            // (a 0-tick delay is a signal carrier: critique, "0-tick delay" on a page of delays)
            (if n == "0" { "no delay: same-tick signal carrier".to_string() } else { format!("{n}-tick delay") }, sub(&[("needs an input of at least {}", "min-pulse"), ("two pulses {} apart stay separate", "min-gap")]))
        }
        "gate" => {
            let f = if cm(c, "function").is_empty() { k1 } else { cm(c, "function") };
            let instant = cm(c, "class") == "instant" || cm(c, "latency") == "0 gt";
            (format!("{}{}", gate_label(f), if instant { " · instant" } else { "" }), if instant { Vec::new() } else { sub(&[("reacts in {}", "latency")]) })
        }
        "rs-latch" | "d-latch" | "dff" | "dff-enable" | "tff-reset" => {
            let mut v = sub(&[("reacts in {}", "latency"), ("resets in {}", "reset")]);
            match cm(c, "enable") {
                // (the input's polarity in plain words: critique, "while the enable input is off"
                // next to another card's "while E is on")
                "low" => v.push("copies its input while its lock input is unpowered, holds while powered".into()),
                "high" => v.push("copies its input while its enable input is powered, holds while unpowered".into()),
                _ => {}
            }
            match cm(c, "edge") {
                "rising" => v.push("copies when the clock turns on".into()),
                "falling" => v.push("copies when the clock turns off".into()),
                _ => {}
            }
            (String::new(), v)
        }
        "shift" => (format!("{k1} bits"), sub(&[("reacts in {}", "latency")])),
        "adder" => (format!("{k1} bits"), sub(&[("reacts in {}", "latency")])),
        // (a counter whose output toggles says so: it reads differently from a level counter of
        // twice its count, critique)
        "counter" => (if cm(c, "output").starts_with("toggles") { format!("toggles every {} pulse", nth(k1)) } else { format!("every {} pulse", nth(k1)) }, sub(&[("reacts in {}", "latency")])),
        "piston-extender" => {
            let (e, r) = (tk(cm(c, "extend")), tk(cm(c, "retract")));
            let timing = if e.is_empty() { None } else { Some(format!("out in {e}, back in {r}")) };
            // what sets it apart from the others of its reach leads (critique: three reach-2
            // cards read the same)
            let lead = if c.lead.is_empty() { String::new() } else { format!(" · {}", c.lead) };
            (format!("reach {k1}{lead}"), timing.into_iter().collect())
        }
        "flying-machine" => {
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            // (direction words: "flies sideways", not "flies horizontal": critique)
            let flies = match c.direction.as_str() {
                "horizontal" => "flies sideways".to_string(),
                "" => "flies".to_string(),
                d => format!("flies {d}"),
            };
            let speed = cm(c, "speed").split_whitespace().next().and_then(|g| g.parse::<f64>().ok()).filter(|g| *g > 0.0).map(|g| {
                let s = format!("{:.2}", 20.0 / g);
                format!("{} blocks a second", s.trim_end_matches('0').trim_end_matches('.'))
            });
            // the title says what sets it apart, the rest goes under it (critique: five
            // "horizontal · 2 blocks a second" cards differed only by block count; a payload
            // title ran 4 lines)
            let (lead, mut rest) = if two_way {
                // (vjeux 00:31Z: "For back and forth, can you call this docking station")
                ("flies to its docking station and back".to_string(), vec![])
            } else if c.feature.starts_with("carries") && c.extend_n.is_some() {
                // a tall machine's cargo capacity is its headline (vjeux 00:31Z: a property)
                (format!("carries up to {} extra blocks", c.extend_n.map(|n| n.1).unwrap_or(0)), vec![flies])
            } else if !c.feature.is_empty() {
                (c.feature.clone(), vec![flies])
            } else {
                (flies, vec![])
            };
            rest.extend(speed);
            rest.extend(sub(&[("starts in {}", "latency")]));
            (lead, rest)
        }
        "shulker-loader" | "shulker-unloader" => {
            // what the card is best at leads (critique: cards with the same numbers differed only
            // by their badges); the items it is timed with go in the second line (critique: "first
            // box after 11 min" with nothing saying 64-stack items take 1728 to fill a box)
            let rate = cm(c, "items/s");
            // (the time to fill one box, to the half minute: critique, "first box after 11 min
            // 33.75 s" said nothing of what it measured)
            let unloader = c.kind.starts_with("shulker-unloader");
            let first = cm(c, "first box").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| {
                // (an unloader empties its box: critique, "fills a box" under "empties full boxes")
                let verb = if unloader { "empties" } else { "fills" };
                let secs = g as f64 / 20.0;
                if secs < 90.0 {
                    format!("{verb} a box in about {} s", secs.round())
                } else {
                    let halves = (secs / 30.0).round() as u64;
                    format!("{verb} a box in about {}{} min", halves / 2, if halves % 2 == 1 { "½" } else { "" })
                }
            });
            let lead = if c.lead.is_empty() { String::new() } else { format!("{} · ", badge_words(&c.lead)) };
            let stack = c.axis.split_whitespace().find_map(|w| w.strip_suffix("-stack")).unwrap_or(k1);
            let items = match stack {
                "" => None,
                "1" => Some("timed with unstackable items".to_string()),
                n => Some(format!("timed with {n}-stack items")),
            };
            (if rate.is_empty() { lead.trim_end_matches(" · ").to_string() } else { format!("{lead}{rate} items a second") }, items.into_iter().chain(first).collect())
        }
        "bud" => {
            let d: Vec<&str> = cm(c, "detects").split_whitespace().map(|k| match k {
                "place" => "placed",
                "remove" => "removed",
                "state" => "changed",
                "shape" => "grown",
                "update" => "updated",
                other => other,
            }).collect();
            (if d.is_empty() { String::new() } else { format!("notices blocks {}", d.join(", ")) }, sub(&[("reacts in {}", "latency"), ("ready again after {}", "re-arm")]))
        }
        "tnt-duper" => {
            // the variation first, the rate second (critique: three cards of 30, 43 and 75 blocks
            // read the same): the axis names it after the rate (`13.2 TNT/min precise air burst`)
            let variation = c.axis.split_once("TNT/min").map(|(_, v)| v.trim()).unwrap_or("");
            let safe = cm(c, "safe drop").starts_with("0 ");
            let lead = match variation {
                "precise air burst" => "explodes each TNT at one exact point in the air",
                "precise" => "lands each TNT on the same spot",
                "switchable" => "has an off switch",
                "launched" if safe => "safe standing on the ground",
                "launched" => "throws its TNT far away",
                _ => "",
            };
            let rate = tnt_rate_words(cm(c, "rate"));
            (if lead.is_empty() { rate } else { format!("{lead} · {rate}") }, sub(&[("a push every {}", "period"), ("first TNT after {}", "latency")]))
        }
        "tnt-duper-triggered" => (format!("{} TNT per press", if cm(c, "per pulse").is_empty() { "1" } else { cm(c, "per pulse") }), sub(&[("lights after {}", "latency"), ("presses {} apart", "gap")])),
        // an item sorter (study 13): what it sorts and how fast, the first item's time second
        "sorter" => {
            let rate = cm(c, "items/s");
            let first = cm(c, "first item").trim_start_matches("by ").split_whitespace().next().and_then(|g| g.parse::<u32>().ok()).map(|g| format!("its first item sorted within {}", crate::real_time(g)));
            (
                if rate.is_empty() { "takes one item kind out of a mixed line".to_string() } else { format!("takes one item kind out of a mixed line · {rate} items a second") },
                first.into_iter().collect(),
            )
        }
        "burst" => (format!("{k1} pulses per press"), sub(&[("{} apart", "period"), ("starts after {}", "latency")])),
        // (what sets a divider apart from a counter leads: how fast a clock it follows; critique:
        // "slows by 3 · 55 blocks" next to a 17-block counter needed its reason)
        "divider" => {
            let follows = cm(c, "min-period");
            let even = if cm(c, "output") == "square" { " · even on/off" } else { "" };
            (if follows.is_empty() { format!("slows by {k1}{even}") } else { format!("slows by {k1} · follows clocks down to {}{even}", tk(follows)) }, Vec::new())
        }
        "monostable" => (
            format!("on for {l}{}", if k1 == "retriggerable" { " · restarts on every press" } else if k1 == "non-retriggerable" { " · ignores presses while on" } else { "" }),
            // (a retriggerable timer's re-arm reads like a contradiction next to "restarts on
            // every press": critique C49; it stays in the measurements)
            if k1 == "retriggerable" { sub(&[("starts after {}", "delay")]) } else { sub(&[("starts after {}", "delay"), ("ready again after {}", "re-arm")]) },
        ),
        "off-delay" => (format!("stays on {l} after"), sub(&[("needs an input of at least {}", "min-pulse")])),
        "on-delay" => (format!("must be held {}", tk(cm(c, "T"))), sub(&[("comes out {} shorter", "shortening")])),
        "pulse-train" => (format!("a pulse every {}", tk(cm(c, "P"))), sub(&[("{} pulses", "pulse"), ("first after {}", "latency")])),
        _ => {
            let axis = if c.axis == c.kind { String::new() } else { axis_shown(&c.kind, &c.axis).unwrap_or_default() };
            let n = if k1.is_empty() || axis.contains(k1) { String::new() } else { k1.to_string() };
            ([n, axis].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · "), sub(&[("reacts in {}", "latency")]))
        }
    }
}

/// The card of a build, by its properties ([`Props`]).
pub fn props(b: &Build) -> Props {
    let flags = |list: &[&str]| -> Vec<String> { list.iter().filter(|f| b.flags.iter().any(|x| x == *f)).map(|f| f.to_string()).collect() };
    let dims = b.dims.as_deref().map(|d| d.replace('x', "×")).unwrap_or_default();
    match b.kind.as_str() {
        "door" => {
            let mut chips: Vec<String> = Vec::new();
            if let Some(d) = &b.door {
                if let Some(l) = d.leads.first() {
                    chips.push(door_lead_words(l, &d.seamless));
                }
                if !d.seamless.is_empty() {
                    chips.push(seamless_words(&d.seamless));
                }
            }
            for (f, words) in [("slime-free", "no slime"), ("flat", "1 block thick"), ("rotation-safe", "works facing any way"), ("variant", "a bigger version")] {
                if b.flags.iter().any(|x| x == f) {
                    chips.push(words.into());
                }
            }
            let timing = b.door.as_ref().and_then(|d| Some(format!("opens in {} · closes in {}", crate::real_time(d.open?), crate::real_time(d.close?))));
            let mut sub = timing.unwrap_or_default();
            // the one door card says what sits behind it (critique: the lone card never said 11
            // records are on its page)
            if let Some(n) = b.door.as_ref().map(|d| d.records).filter(|n| *n > 1) {
                sub = format!("the best of {n} records (smallest, fastest, hidden, sculk…): pick one on its page · {sub}");
            }
            let cat = door_category(&b.group);
            // one block count (critique: "7×8×1 · 38 blocks" next to "56 blocks of space" read as
            // two counts): the blocks, in their space
            let title = if dims.is_empty() { blocks_word(b.blocks) } else { format!("{} in a {dims} space", blocks_word(b.blocks)) };
            // one short kind line (critique: "3×3 PISTON DOOR · STONE DOOR" on every card); a
            // fully seamless stone door says so (vjeux 23:44Z: what people call a piston door)
            let full = b.door.as_ref().is_some_and(|d| d.seamless == "FULL" || d.seamless == "SUPER");
            let kind = match b.group.as_str() {
                "solid" if full => "3×3 seamless stone door",
                "solid" => "3×3 stone door",
                "sculk" => "3×3 sculk door",
                "gravity" => "3×3 sand or gravel door",
                "entity" => "3×3 entity door",
                _ => if cat.is_empty() { "3×3 piston door" } else { cat },
            };
            Props { kind: kind.into(), title, sub, chips }
        }
        "clock" => {
            let c = b.clock.as_ref();
            let period = c.map(|c| format!("every {} ({})", if c.every() == 1 { "tick".to_string() } else { format!("{} ticks", c.every()) }, crate::real_time(c.every()))).unwrap_or_default();
            // an output that pulses several times evenly in a cycle: its page says the cycle; the
            // card's "2 pulses in each 8-tick cycle" under "every 4 ticks" added nothing (critique)
            let pulse = c.and_then(|c| match (c.pulse_every, c.edges) {
                (Some(_), _) => None,
                (None, 0) => None,
                (None, 1) if c.on > 0 => Some(format!("on for {} of {} ticks", c.on, c.period)),
                (None, 1) => None,
                (None, e) => Some(format!("{e} pulses a cycle")),
            });
            let made = format!("made of {}", human_blocks(&b.group));
            let sub = [Some(made), pulse].into_iter().flatten().collect::<Vec<_>>().join(" · ");
            let mut chips: Vec<String> = Vec::new();
            for (f, words) in [("flat", "1 block thick"), ("slime-free", "no slime"), ("self-starting", "starts by itself"), ("extendable", "comes in every size"), ("composite", "several clocks in one"), ("variant", "a bigger version")] {
                if b.flags.iter().any(|x| x == f) {
                    chips.push(words.into());
                }
            }
            // what a chunk reload does, where it matters (clockinfra's reload; the coordinator)
            if let Some(chip) = c.and_then(|c| reload_chip(&c.reload)) {
                chips.push(chip.into());
            }
            // the one clock card: the periods its page's table covers (vjeux 21:47Z)
            if let Some(range) = c.map(|c| c.kind_range.clone()).filter(|r| !r.is_empty()) {
                let (span, tested) = range.split_once(':').unwrap_or((range.as_str(), ""));
                let (lo, hi) = span.split_once('-').unwrap_or((span, span));
                let hi_words = hi.parse::<u32>().map(crate::real_time).unwrap_or_else(|_| format!("{hi} ticks"));
                // (the periods tested, no more: the generated ones are gone, vjeux 00:33Z)
                return Props {
                    kind: "Clock".into(),
                    title: format!("the best clock for each of {tested} periods, from {lo} ticks to {hi_words}"),
                    sub: format!("each the smallest that passed our real-game test · pictured: {period}, {}", blocks_word(b.blocks)),
                    chips: Vec::new(),
                };
            }
            Props { kind: "Clock".into(), title: format!("{period} · {}", blocks_word(b.blocks)), sub, chips }
        }
        _ => match &b.circuit {
            Some(c) => {
                let (main, sub) = circuit_props(c);
                let k0 = c.kind.split_whitespace().next().unwrap_or("");
                // a sand elevator lifts its block up (its own section: critique)
                let main = if b.group == "Sand elevator" { format!("{} up", main.replace("reach ", "lifts ")) } else { main };
                let mut chips: Vec<String> = Vec::new();
                if !c.span.is_empty() {
                    chips.push(format!("{} versions", c.span.split(", ").last().unwrap_or("").trim_end_matches(" values").trim_end_matches(" versions")));
                }
                // what "faster" means for its kind (critique: "faster than smaller versions" on a
                // delay, whose speed metrics are the shortest pulse and gap it handles)
                let faster = match k0 {
                    "delay" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "off-delay" => "handles shorter pulses than smaller versions",
                    "tff" => "handles faster presses than smaller versions",
                    "shulker-loader" | "shulker-unloader" => "moves more items a second than smaller versions",
                    _ => "faster than smaller versions",
                };
                for (f, words) in [("faster", faster), ("button", "has its own button"), ("lever", "has its own lever"), ("mirrored", "mirror version too"), ("slime-free", "no slime"), ("flat", "1 block thick"), ("extendable", "comes in every size"), ("variant", "a bigger version"), ("layout", "another layout")] {
                    if b.flags.iter().any(|x| x == f) {
                        chips.push(words.into());
                    }
                }
                // the same build archived as another kind too
                for a in &c.also {
                    // (the same build is also another kind's best: critique, "also a one pulse
                    // when switched on" was garbled)
                    chips.push(format!("also the best “{}”", a.to_lowercase()));
                }
                // the box's shape: 1 high (a flat 2-D design) or 1 wide (tiles side by side); a
                // BUD's box leaves out the block it watches, which its picture shows (critique: a
                // "lies flat" card with a fence standing on it)
                let watches = c.kind.starts_with("bud");
                if let Some(d) = &b.dims {
                    let n: Vec<u32> = d.split('x').filter_map(|v| v.trim().parse().ok()).collect();
                    if n.len() == 3 && !chips.iter().any(|c| c == "1 block thick") {
                        if n[1] == 1 && !c.ports_off_layers {
                            chips.push(if watches { "1 block high, plus the block it watches".into() } else { "lies flat (1 block high)".into() });
                        } else if n[0] == 1 || n[2] == 1 {
                            chips.push("1 block wide".into());
                        }
                    }
                }
                let badges: Vec<&str> = cm(c, "badges").split('+').filter(|s| !s.is_empty() && !c.lead.split(" + ").any(|l| l == badge_words(s))).collect();
                // (every badge: a card caps its chips after leaving out the shared ones, so it
                // never shows only "+4 more": critique)
                chips.extend(badges.iter().map(|s| badge_words(s)));
                // a chip the card's lead already says is not repeated ("no slime" in "reach 2 ·
                // no slime, 1 block wide"; "faster than smaller versions" under "fastest")
                if !c.lead.is_empty() {
                    chips.retain(|ch| !c.lead.contains(ch.as_str()) && !(ch.ends_with("than smaller versions") && c.lead.contains("fast")));
                }
                let mut sub = sub;
                if !c.direction.is_empty() && c.kind != "flying-machine" {
                    sub.insert(0, c.direction.clone());
                }
                // what it is made of (the blocks that change state) and its box tell siblings apart
                // (no design names in lists: vjeux 13:11Z; critique: two delay cards read the
                // same, "2 comparators" vs "a repeater"; fly cards: slime or honey); only the
                // design's own blocks, not the test's (a BUD's target block: critique, "made of oak
                // log, observer")
                let own: Vec<&str> = c.family.split('+').filter(|k| !k.is_empty() && (b.kinds.is_empty() || b.kinds.iter().any(|x| x == k))).collect();
                if !own.is_empty() && own.len() <= 3 {
                    sub.push(format!("made of {}", human_blocks(&own.join("+"))));
                }
                if !dims.is_empty() {
                    sub.push(format!("fits in {dims}"));
                }
                let title = if main.is_empty() { blocks_word(b.blocks) } else { format!("{main} · {}", blocks_word(b.blocks)) };
                // a design with no number (a latch): its first property leads (critique: a card read
                // just "4 blocks")
                let (title, sub) = if main.is_empty() && !sub.is_empty() {
                    let mut sub = sub;
                    let first = sub.remove(0);
                    (format!("{first} · {}", blocks_word(b.blocks)), sub)
                } else {
                    (title, sub)
                };
                // why it is the one on the site leads its second line (critique: "cards don't say
                // why they're the best"; not a chip: the shared chips fold into a section's line)
                let sub = if c.claim.is_empty() { sub } else { std::iter::once(c.claim.clone()).chain(sub).collect() };
                // (the kind line: the catalog's group; a two-way flying machine is one "with docking
                // stations": vjeux 00:31Z)
                let two_way = c.kind.starts_with("flying-machine") && (c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way"));
                let kind = if two_way {
                    "Flying machine with docking stations".to_string()
                } else if c.kind.starts_with("flying-machine") {
                    // by engine shape (vjeux 00:31Z): stacked = tall, side by side = flat
                    if c.feature.starts_with("carries") { "Tall flying machine".to_string() } else { "Flat flying machine".to_string() }
                } else if b.group.is_empty() {
                    kind_words(&c.kind)
                } else {
                    b.group.clone()
                };
                Props { kind, title, sub: sub.join(" · "), chips }
            }
            None => Props { kind: b.group.clone(), title: blocks_word(b.blocks), sub: String::new(), chips: flags(&["slime-free", "flat"]) },
        },
    }
}

/// A result card for a build: its kind (small, above), its properties as the headline, its name
/// small at the bottom (and in the tooltip). `prefix` is the path from the page to the site root
/// (`""`, `"../"`).
pub fn build_card(b: &Build, prefix: &str) -> String {
    build_card_opts(b, prefix, true, &[])
}

/// A card without what every card around it says (critique C43): no kind line under a heading
/// that names the kind, and none of the chips shared by every card in view.
pub fn build_card_opts(b: &Build, prefix: &str, show_kind: bool, shared: &[String]) -> String {
    let kind = if show_kind { Some(props(b).kind) } else { None };
    card_inner(b, prefix, kind, shared)
}

/// A card on the index's tab of its own kind: the kind line without the kind's name every card
/// there carries (a door keeps its category, a clock drops it; critique: six cards in a row read
/// "3×3 PISTON DOOR · STONE DOOR"), and without the chips most cards share (said once above).
pub fn build_card_tab(b: &Build, prefix: &str, tab: &str, shared: &[String]) -> String {
    card_inner(b, prefix, tab_kind(b, tab), shared)
}

/// The kind line of a card on the index's tab `tab` ([`build_card_tab`]).
pub fn tab_kind(b: &Build, tab: &str) -> Option<String> {
    let full = props(b).kind;
    if tab != b.kind {
        Some(full)
    } else {
        match b.kind.as_str() {
            "door" => Some(full.trim_start_matches("3×3 ").to_string()),
            "clock" => None,
            _ => Some(full),
        }
    }
}

fn card_inner(b: &Build, prefix: &str, kind: Option<String>, shared: &[String]) -> String {
    let mut p = props(b);
    // a group's card says the whole range, not its smallest member's value (critique: "counter
    // 17 · 9 versions" does 17 to 576); the design's own page keeps its own value
    if let Some(r) = b.circuit.as_ref().and_then(group_range) {
        let size = b.circuit.as_ref().map(|c| c.span_blocks.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| blocks_word(b.blocks));
        // (what the group adds over the smaller ones of its kind, when it says: "adds 18, 34 ticks")
        let lead = b.circuit.as_ref().filter(|c| !c.lead.is_empty() && !c.kind.starts_with("shulker-") && !c.kind.starts_with("piston-extender")).map(|c| format!(" · {}", c.lead)).unwrap_or_default();
        p.title = format!("{r}{lead} · {size}");
        // a kind's card: its facts are the pictured size's, so they say which (critique: "reach 2
        // to 12" beside the reach-2 design's "out in 9 ticks, fits in 5×3×1")
        if let Some(c) = b.circuit.as_ref().filter(|c| c.tunable.starts_with("kind:") && !p.sub.is_empty()) {
            let which = if c.kind.starts_with("flying-machine") { c.variant_label.clone() } else { value_words(&c.kind, &c.axis) };
            if !which.is_empty() {
                // (the kind's claim first, then the pictured size's facts)
                let rest = if c.claim.is_empty() { p.sub.clone() } else { p.sub.strip_prefix(&format!("{} · ", c.claim)).map(String::from).unwrap_or_else(|| p.sub.clone()) };
                p.sub = if c.claim.is_empty() || rest == p.sub { format!("pictured, {which}: {rest}") } else { format!("{} · pictured, {which}: {rest}", c.claim) };
            }
        }
    }
    let mut chips: Vec<String> = p
        .chips
        .iter()
        .filter(|x| !shared.contains(x))
        .cloned()
        // a chip most cards around share, missing here, is said (critique: two cards read the same),
        // unless the card's lead says it
        .chain(shared.iter().filter(|s| !p.chips.contains(s) && !b.circuit.as_ref().is_some_and(|c| c.lead.contains(s.as_str()))).filter_map(|s| unlike_most(s)))
        .collect();
    // at most 5 chips on a card, then how many more its page lists
    if chips.len() > 6 {
        let more = chips.len() - 5;
        chips.truncate(5);
        chips.push(format!("+{more} more"));
    }
    let chips: String = chips.iter().map(|x| format!("<span class=\"badge\">{}</span>", esc(x))).collect();
    format!(
        "<a class=\"card\" href=\"{}\" aria-label=\"{}\">{}<div class=\"meta\">{}<div class=\"title\">{}</div>{}<div class=\"badges\">{chips}</div></div></a>",
        esc(&format!("{prefix}{}", b.page)),
        esc(&format!("{}: {}", p.kind, p.title)),
        thumb(prefix, &b.design, b.trace.as_deref(), b.picture.as_deref(), &b.kind, &headline(b), &fallback_line(b)),
        if let Some(k) = kind.filter(|k| !k.is_empty()) { format!("<div class=\"kind\">{}</div>", esc(&k)) } else { String::new() },
        esc(&keep_units(&p.title)),
        if p.sub.is_empty() { String::new() } else { format!("<div class=\"sub\">{}</div>", esc(&keep_units(&p.sub))) }
    )
}

/// The chips every card of a list shares (to leave out: they tell nothing there).
pub fn shared_chips(list: &[&Build]) -> Vec<String> {
    if list.len() < 2 {
        return Vec::new();
    }
    let first = props(list[0]).chips;
    first.into_iter().filter(|c| list.iter().skip(1).all(|b| props(b).chips.contains(c))).collect()
}

/// The chips most cards of a list share (two thirds or more, at least 3 cards): said once above
/// the list instead of on every card (critique C43: "has its own lever" on 7 of 8 cards); the
/// cards without one say so ([`unlike_most`]), so two cards never read the same because one chip
/// moved up (critique: rider7-flat vs the slime-free sf7-1).
pub fn common_chips(list: &[&Build]) -> Vec<String> {
    if list.len() < 3 {
        return shared_chips(list);
    }
    let all: Vec<Vec<String>> = list.iter().map(|b| props(b).chips).collect();
    let mut seen: Vec<String> = Vec::new();
    for chips in &all {
        for c in chips {
            if !seen.contains(c) && all.iter().filter(|x| x.contains(c)).count() * 3 >= list.len() * 2 {
                seen.push(c.clone());
            }
        }
    }
    seen
}

/// A common chip a card lacks, in words (`no slime` -> `uses slime`); a chip without its own
/// negation says nothing (critique: "not works facing any way").
pub fn unlike_most(chip: &str) -> Option<String> {
    Some(
        match chip {
            "no slime" => "uses slime",
            "has its own lever" => "no lever of its own",
            "has its own button" => "no button of its own",
            "1 block wide" => "wider than 1 block",
            "1 block thick" => "thicker than 1 block",
            "lies flat (1 block high)" => "taller than 1 block",
            "comes in every size" => "one size only",
            "works facing any way" => "works only facing one way",
            "starts by itself" => "needs a kick to start",
            _ => return None,
        }
        .to_string(),
    )
}

/// A chip as what most of a list do (`has its own lever` -> `have their own lever`), for the line
/// above the list.
pub fn chip_plural(chip: &str) -> String {
    if let Some(rest) = chip.strip_prefix("has its own ") {
        return format!("have their own {rest}");
    }
    if chip.starts_with("no ") {
        return format!("use {chip}");
    }
    // (critique: "Most are machine hidden from both sides")
    if let Some(rest) = chip.strip_prefix("machine hidden ") {
        return format!("hide their machine {rest}");
    }
    if let Some(rest) = chip.strip_prefix("also a ") {
        return format!("are also {rest}s");
    }
    match chip {
        "comes in every size" => "come in every size".to_string(),
        "lies flat (1 block high)" => "lie flat (1 block high)".to_string(),
        "works facing any way" => "work facing any way".to_string(),
        "starts by itself" => "start by themselves".to_string(),
        "carries extra blocks" => "carry extra blocks".to_string(),
        "mirror version too" => "come in a mirror version too".to_string(),
        "1 block high, plus the block it watches" => "are 1 block high, plus the block they watch".to_string(),
        "16-stack items too" => "handle 16-stack items too".to_string(),
        "always on, no lever" => "run on their own (no lever)".to_string(),
        "unstackable items too" => "handle unstackable items too".to_string(),
        "packs mixed items" => "pack mixed items".to_string(),
        "packs one kind, passes the rest" => "pack one kind and pass the rest".to_string(),
        "tiles side by side" => "tile side by side".to_string(),
        "restarts by itself" => "restart by themselves".to_string(),
        "waits out a box shortage" => "wait out a box shortage".to_string(),
        "keeps box colours and names" => "keep box colours and names".to_string(),
        "sends out a part-full box" => "send out a part-full box".to_string(),
        "tops up part-full boxes" => "top up part-full boxes".to_string(),
        "drops nothing on the ground" => "drop nothing on the ground".to_string(),
        "loads and unloads" => "load and unload".to_string(),
        "empties any box" => "empty any box".to_string(),
        other => format!("are {other}"),
    }
}

/// A number keeps its unit on the same line (critique C42: "2 / blocks" wrapped): the space after
/// a digit becomes a no-break space.
pub fn keep_units(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_digit = false;
    for ch in s.chars() {
        if ch == ' ' && prev_digit {
            out.push('\u{a0}');
        } else if ch == '–' && prev_digit {
            // a range stays on one line: "2–511 ticks" (critique: "cuts pulses to 2– / 511")
            out.push_str("\u{2060}–\u{2060}");
        } else {
            out.push(ch);
        }
        prev_digit = ch.is_ascii_digit();
    }
    out
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

/// A result card for an extendable family's member: its value and size lead (`every 48 ticks
/// (2.4 s) · 24 blocks`; a circuit family's member: its axis value); no family id or n (vjeux:
/// no ids in lists).
pub fn member_card(m: &Member, site: &Site, prefix: &str) -> String {
    let base = site
        .cards
        .iter()
        .find(|c| c.id == m.card)
        .and_then(|c| c.families.iter().find(|f| f.id == m.family))
        .and_then(|f| site.builds.iter().find(|b| b.id == f.build));
    let group = base.map(|b| if b.kind == "clock" { format!("made of {}", human_blocks(&b.group)) } else { b.group.clone() }).unwrap_or_default();
    let status = format!("<span class=\"badge st-{}\">{}</span>", esc(&m.status), esc(status_label(&m.status)));
    let circuit = !m.axis.is_empty();
    let (kind, big, value) = if circuit {
        ("circuit", m.axis.clone(), axis_shown("", &m.axis).unwrap_or_else(|| m.axis.clone()))
    } else {
        ("clock", format!("every {} ticks", m.period), format!("every {} ticks ({})", m.period, crate::real_time(m.period)))
    };
    let kind_line = base.and_then(|b| b.circuit.as_ref().map(|c| kind_words(&c.kind))).unwrap_or_else(|| "Clock".into());
    // (each number kept with its unit: critique, "2 / blocks" wrapped)
    format!(
        "<a class=\"card member\" href=\"{}\" aria-label=\"{}: {} · {}\">{}<div class=\"meta\"><div class=\"kind\">{}</div><div class=\"title\">{} · {}</div><div class=\"sub\">{}</div><div class=\"badges\"><span class=\"badge\">comes in every size</span>{status}</div></div></a>",
        esc(&format!("{prefix}{}", member_href(m, site))),
        esc(&kind_line),
        esc(&value),
        esc(&blocks_word(m.blocks)),
        thumb(prefix, &m.design, None, m.picture.as_deref(), kind, &big, ""),
        esc(&kind_line),
        esc(&keep_units(&value)),
        esc(&keep_units(&blocks_word(m.blocks))),
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

/// [`hit_card`] on the index's tab `tab` (`door`, `clock`, `circuit`, or empty for every kind),
/// without the chips in `shared` ([`build_card_tab`]).
pub fn hit_card_tab(site: &Site, h: Hit, prefix: &str, tab: &str, shared: &[String]) -> String {
    let mut prev = None;
    hit_card_run(site, h, prefix, tab, shared, &mut prev)
}

/// [`hit_card_tab`] in a run of cards: on the every-kind list (the home page) a kind line the
/// previous card already showed is left out, so it reads like a heading over its run (critique:
/// the home repeated "3×3 STONE DOOR" on every card).
pub fn hit_card_run(site: &Site, h: Hit, prefix: &str, tab: &str, shared: &[String], prev: &mut Option<String>) -> String {
    match h {
        Hit::Build(i) => site
            .builds
            .get(i)
            .map(|b| {
                let kind = tab_kind(b, tab);
                let shown = if tab.is_empty() && kind.is_some() && kind == *prev { None } else { kind.clone() };
                *prev = kind;
                card_inner(b, prefix, shown, shared)
            })
            .unwrap_or_default(),
        Hit::Member(i) => {
            *prev = None;
            site.members.get(i).map(|m| member_card(m, site, prefix)).unwrap_or_default()
        }
    }
}

/// The chips most of the first hits share (said once above the index's cards), from the first
/// 48 build hits (at least 3).
pub fn hits_common_chips(site: &Site, hits: &[Hit]) -> Vec<String> {
    let builds: Vec<&Build> = hits.iter().filter_map(|h| match h {
        Hit::Build(i) => site.builds.get(*i),
        Hit::Member(_) => None,
    }).take(48).collect();
    if builds.len() < 3 {
        return Vec::new();
    }
    common_chips(&builds)
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
        assert!(h.starts_with("<a class=\"card\" href=\"../clocks/x.html\" aria-label=\"Clock: every 26 ticks (1.3 s) · 6 blocks\">"), "{h}");
        assert!(h.contains("data-mcview=\"thumb\" data-src=\"../designs/clocks/x.txt\" data-kind=\"clock\""), "{h}");
        // the properties lead; no design name anywhere on the card (vjeux 13:11Z); numbers keep
        // their units (no-break spaces)
        assert!(h.contains("<div class=\"title\">every 26\u{a0}ticks (1.3\u{a0}s) · 6\u{a0}blocks</div>"), "{h}");
        assert!(h.contains("<div class=\"sub\">made of observer, redstone lamp</div>"), "{h}");
        assert!(!h.contains("x&lt;1&gt;") && !h.contains("class=\"name\""), "{h}");
        assert!(h.contains("<span class=\"badge\">starts by itself</span>"), "{h}");
    }

    #[test]
    fn circuit_cards_lead_with_properties() {
        let ext = Build {
            kind: "circuit".into(),
            name: "pext-n3-flat-cascade19".into(),
            blocks: 19,
            dims: Some("9x6x1".into()),
            kinds: vec!["sticky_piston".into()],
            flags: vec!["button".into(), "flat".into()],
            circuit: Some(crate::Circuit { kind: "piston-extender 3".into(), axis: "N = 3".into(), metrics: vec![("N".into(), "3".into()), ("extend".into(), "27 gt".into()), ("retract".into(), "56 gt (from the press)".into())], ..Default::default() }),
            ..Default::default()
        };
        let p = props(&ext);
        assert_eq!(p.kind, "Piston extender");
        assert_eq!(p.title, "reach 3 · 19 blocks");
        assert_eq!(p.sub, "out in 27 ticks (1.35 s), back in 56 ticks (2.8 s) · fits in 9×6×1");
        assert_eq!(p.chips, vec!["has its own button".to_string(), "1 block thick".to_string()]);
        let div = Build { kind: "circuit".into(), blocks: 13, circuit: Some(crate::Circuit { kind: "divider 3".into(), metrics: vec![("output".into(), "square".into()), ("latency".into(), "37 gt".into())], ..Default::default() }), ..Default::default() };
        assert_eq!((props(&div).kind.as_str(), props(&div).title.as_str()), ("Clock slower", "slows by 3 · even on/off · 13 blocks"));
        let mono = Build { kind: "circuit".into(), blocks: 21, circuit: Some(crate::Circuit { kind: "monostable retriggerable".into(), metrics: vec![("L".into(), "32 gt".into())], ..Default::default() }), ..Default::default() };
        assert_eq!((props(&mono).kind.as_str(), props(&mono).title.as_str()), ("One-shot timer", "on for 32 ticks (1.6 s) · restarts on every press · 21 blocks"));
        assert_eq!(kind_words("piston-extender 5"), "Piston extender");
        assert_eq!(kind_words("gate and"), "Logic gate");
        assert_eq!(kind_words("full-adder 4"), "Full adder");
        assert_eq!(gate_label("nimply"), "A AND (NOT B)");
        assert_eq!(human_blocks("observer+redstone_lamp"), "observer, redstone lamp");
        // an empty search offers the nearest kinds
        assert!(nearest_searches("timers").contains("one-shot timer") && nearest_searches("timers").contains("stay-on timer"));
        assert!(nearest_searches("xyzzy").contains("toggle switch"));
    }
}
