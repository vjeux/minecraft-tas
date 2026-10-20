//! Plain-language explanations for the circuit pages (vjeux, 01:05Z on a counter's page: "Very
//! unclear what that is and what to do"): what each kind of circuit does (with this design's
//! numbers), how to try it in the viewer, what every metric means, the studies' introductions,
//! gate truth tables, and the timing diagram of the judge's test.

use site_model::html::esc;
use site_model::{Build, Circuit, Wave, real_time};

/// `4 gt (0.2 s)`, from a metric value like `4 gt`.
fn with_seconds(v: &str) -> String {
    match v.trim().strip_suffix(" gt").and_then(|n| n.trim().parse::<u32>().ok()) {
        Some(n) => format!("{n} gt ({})", real_time(n)),
        None => v.to_string(),
    }
}

/// A metric of the circuit (`L`, `delay`, `latency`...), empty when absent.
fn metric<'a>(c: &'a Circuit, k: &str) -> &'a str {
    c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// The kind's family word: `counter 4` -> `counter`, `gate xor` -> `gate`.
fn base(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or(kind)
}

/// The number after the kind (`counter 4` -> 4).
fn kind_number(kind: &str) -> Option<u32> {
    kind.split_whitespace().nth(1).and_then(|n| n.parse().ok())
}

/// The gate function of a circuit: its `function` metric, else the kind's argument.
fn gate_function(c: &Circuit) -> String {
    let m = metric(c, "function");
    if !m.is_empty() {
        return m.to_string();
    }
    c.kind.split_whitespace().nth(1).unwrap_or("").to_string()
}

/// When a 2-input gate's output is on, in words (inputs A and B).
pub fn gate_rule(f: &str) -> &'static str {
    match f {
        "not" => "the opposite of the input: on while the input is off, off while it is on",
        "buffer" => "the same as the input",
        "and" => "on only while A and B are both on",
        "or" => "on while A or B (or both) is on",
        "xor" => "on while exactly one of A and B is on (not both)",
        "xnor" => "on while A and B are the same (both on or both off)",
        "nand" => "off only while A and B are both on (NOT AND)",
        "nor" => "on only while A and B are both off (NOT OR)",
        "imply" => "on unless A is on and B is off (A implies B)",
        "nimply" => "on only while A is on and B is off (A AND NOT B)",
        "imply-ba" => "on unless B is on and A is off (B implies A)",
        "nimply-ba" => "on only while B is on and A is off (B AND NOT A)",
        "not-a" => "the opposite of A (B is ignored)",
        "not-b" => "the opposite of B (A is ignored)",
        _ => "a fixed function of the inputs (see the truth table)",
    }
}

/// The truth table of a gate function as an HTML table (inputs in name order: A, B).
pub fn truth_table(f: &str, inputs: &[String]) -> String {
    let on = |v: bool| if v { "<td class=\"on\">on</td>" } else { "<td class=\"off\">off</td>" };
    if f == "not" || f == "buffer" || inputs.len() == 1 {
        let name = inputs.first().map(|s| s.to_uppercase()).unwrap_or_else(|| "IN".into());
        let rows: String = [false, true].iter().map(|&a| format!("<tr>{}{}</tr>", on(a), on(if f == "buffer" { a } else { !a }))).collect();
        return format!("<table class=\"truth\"><thead><tr><th>{}</th><th>OUT</th></tr></thead><tbody>{rows}</tbody></table>", esc(&name));
    }
    let Some((_, t)) = circuit::GATES2.iter().find(|(n, _)| *n == f) else { return String::new() };
    let (a, b) = (inputs.first().map(|s| s.to_uppercase()).unwrap_or_else(|| "A".into()), inputs.get(1).map(|s| s.to_uppercase()).unwrap_or_else(|| "B".into()));
    let rows: String = (0..4).map(|i| format!("<tr>{}{}{}</tr>", on(i >= 2), on(i % 2 == 1), on(t[i]))).collect();
    format!("<table class=\"truth\"><thead><tr><th>{}</th><th>{}</th><th>OUT</th></tr></thead><tbody>{rows}</tbody></table>", esc(&a), esc(&b))
}

/// The kind's name for people (`counter 4` -> `Counter (divide by 4)`).
pub fn kind_title(kind: &str) -> String {
    match base(kind) {
        "rising-edge" => "Rising-edge detector".into(),
        "falling-edge" => "Falling-edge detector".into(),
        "dual-edge" => "Dual-edge detector".into(),
        "pulse-limiter" => "Pulse limiter".into(),
        "pulse-extender" => "Pulse extender".into(),
        "tff" => "T flip-flop (toggle)".into(),
        "delay" => "Delay line".into(),
        "gate" => match kind.split_whitespace().nth(1) {
            Some(f) => format!("{} gate", f.to_uppercase()),
            None => "Logic gate".into(),
        },
        "rs-latch" => "RS latch (set / reset memory)".into(),
        "d-latch" => "D latch (data latch)".into(),
        "counter" => format!("Counter (counts to {})", kind_number(kind).unwrap_or(0)),
        "piston-extender" => format!("Piston extender (reach {})", kind_number(kind).unwrap_or(0)),
        "flying-machine" => "Flying machine".into(),
        _ => kind.to_string(),
    }
}

/// What a kind does in general (no numbers): the study pages' kind list.
pub fn kind_summary(kind: &str) -> String {
    match base(kind) {
        "rising-edge" => "when the input turns on, the output gives one short pulse; nothing when it turns off".into(),
        "falling-edge" => "when the input turns off, the output gives one short pulse; nothing when it turns on".into(),
        "dual-edge" => "a short output pulse every time the input changes, on or off".into(),
        "pulse-limiter" => "the output pulse lasts at most L, however long the input stays on".into(),
        "pulse-extender" => "any input pulse, even a 1-tick one, gives an output pulse of at least L".into(),
        "tff" => "each input pulse flips the output: on, off, on, off... (a push-button switch)".into(),
        "delay" => "the output is the input, N game ticks later (every pulse comes out the same length)".into(),
        "gate" => "the output is a fixed logic function of the inputs (NOT, AND, OR, XOR...)".into(),
        "rs-latch" => "a pulse on S turns the output on and it stays on; a pulse on R turns it off".into(),
        "d-latch" => "while the enable input E is active the output follows the data input D; otherwise it holds the last value".into(),
        "counter" => "counts input pulses: the output changes once every N pulses (a frequency divider)".into(),
        "piston-extender" => "one input pushes a block N cells out, the next brings it back exactly".into(),
        "flying-machine" => "moves itself through the world, one block at a time, once started".into(),
        _ => String::new(),
    }
}

/// The paragraph at the top of a circuit page: what this circuit does, with its numbers.
pub fn what_it_does(c: &Circuit) -> String {
    let l = with_seconds(metric(c, "L"));
    let delay = with_seconds(metric(c, "delay"));
    let min_pulse = with_seconds(metric(c, "min-pulse"));
    let one_input_pulse = if min_pulse.is_empty() { String::new() } else { format!(" It handles every input pulse of {min_pulse} or longer.") };
    match base(&c.kind) {
        "rising-edge" => format!(
            "<p>When the input turns <b>on</b>, the output gives <b>one short pulse</b> of L = {l}, starting {delay} after the input turned on. When the input turns off, nothing happens.{one_input_pulse}</p><p>Use it to turn a lever (which stays on) into a single short pulse, e.g. to fire a piston once.</p>"
        ),
        "falling-edge" => format!(
            "<p>When the input turns <b>off</b>, the output gives <b>one short pulse</b> of L = {l}, starting {delay} after the input turned off. When the input turns on, nothing happens.{one_input_pulse}</p><p>Use it to react to something stopping, e.g. a lever being switched back.</p>"
        ),
        "dual-edge" => format!(
            "<p>Every time the input <b>changes</b> (turns on or turns off), the output gives <b>one short pulse</b> of L = {l}, {delay} after the change.{one_input_pulse}</p><p>Use it to react to any change of a lever, in both directions.</p>"
        ),
        "pulse-limiter" => format!(
            "<p>However long the input stays on, the output pulse lasts <b>at most L = {l}</b>; it starts {delay} after the input turns on. (An input shorter than L may come out shorter.){one_input_pulse}</p><p>Use it to cut a long signal down to a short, fixed pulse.</p>"
        ),
        "pulse-extender" => format!(
            "<p>Any input pulse, even a 1-tick one, gives an output pulse of <b>at least L = {l}</b>, starting {delay} after the input turns on.{one_input_pulse}</p><p>Use it to make a brief signal (an observer's 2-tick pulse) last long enough for a slower part.</p>"
        ),
        "tff" => {
            let edge = metric(c, "edge");
            let when = match edge {
                "rising" => "when the input turns on",
                "falling" => "when the input turns off",
                _ => "on each input pulse",
            };
            let latency = with_seconds(metric(c, "latency"));
            let min_period = with_seconds(metric(c, "min-period"));
            format!(
                "<p>Each input pulse <b>flips the output</b>: the first pulse turns it on, the next turns it off, and so on; between pulses the output stays put. It flips {when}{}.</p><p>It works like a push-button lamp switch: press once for on, press again for off.{}{}</p>",
                if latency.is_empty() { String::new() } else { format!(", {latency} later") },
                if min_pulse.is_empty() { String::new() } else { format!(" It accepts input pulses of {min_pulse} or longer") },
                if min_period.is_empty() { ".".to_string() } else { format!(" and follows pulses as fast as one every {min_period}.") }
            )
        }
        "delay" => {
            let n = with_seconds(metric(c, "N"));
            let min_gap = with_seconds(metric(c, "min-gap"));
            let one_edge = c.verdict.contains("one-edge");
            format!(
                "<p>The output is <b>a copy of the input, N = {n} later</b>: a pulse goes in, the same pulse comes out N later{}.</p><p>{}{}</p>",
                if one_edge { " (this design delays the two edges by different amounts: a one-edge delay)" } else { ", same length" },
                if min_pulse.is_empty() { String::new() } else { format!("Pulses shorter than {min_pulse} get lost. ") },
                if min_gap.is_empty() { String::new() } else { format!("Two pulses closer than {min_gap} merge.") }
            )
        }
        "gate" => {
            let f = gate_function(c);
            let latency = metric(c, "latency");
            let instant = latency.trim() == "0 gt";
            let n = if c.inputs.len() == 1 { "input" } else { "inputs" };
            format!(
                "<p>A logic gate: the output depends only on the {n} right now. <b>{}</b>: the output is {}.</p>{}<p>{}</p>",
                esc(&f.to_uppercase()),
                gate_rule(&f),
                truth_table(&f, &c.inputs),
                if instant {
                    "It is <b>instant</b>: the output changes in the same game tick as the inputs (0 gt).".to_string()
                } else if latency.is_empty() {
                    String::new()
                } else {
                    format!("The output settles {} after an input changes.", with_seconds(latency))
                }
            )
        }
        "rs-latch" => {
            let qbar = c.outputs.iter().any(|o| o == "qbar");
            format!(
                "<p>A one-bit memory. A pulse on <b>S</b> (set) turns the output <b>on</b>, and it stays on; a pulse on <b>R</b> (reset) turns it <b>off</b>, and it stays off. It remembers which one came last.{}</p>",
                if qbar { " QBAR is always the opposite of Q." } else { "" }
            )
        }
        "d-latch" => {
            let high = metric(c, "enable") != "low";
            let (active, inactive) = if high { ("on", "off") } else { ("off", "on") };
            format!(
                "<p>A one-bit memory with a data input <b>D</b> and an enable input <b>E</b>. While E is <b>{active}</b>, the output Q copies D; when E turns {inactive}, Q keeps the value D had and ignores D until E is {active} again (enable {}).</p>",
                if high { "high" } else { "low" }
            )
        }
        "counter" => {
            let n = kind_number(&c.kind).unwrap_or(0);
            let style = metric(c, "output");
            let how = if style.starts_with("toggles") {
                format!("the output <b>flips once every {n} input pulses</b>: on for {n} pulses, then off for {n}")
            } else if style.starts_with("pulses") {
                format!("the output gives <b>one pulse for every {n} input pulses</b>")
            } else {
                format!("the output's state <b>repeats every {n} input pulses</b> (here it marks one pulse in every {n}: the output is on while that pulse is counted)")
            };
            format!("<p>It <b>counts input pulses</b>: {how}. In other words, it divides the number of pulses by {n}.</p><p>Use it to do something every {} time, or to slow a fast clock down {n} times.</p>", ordinal(n))
        }
        "piston-extender" => {
            let n = kind_number(&c.kind).unwrap_or(0);
            let ext = with_seconds(metric(c, "extend"));
            let ret = with_seconds(metric(c, "retract"));
            format!(
                "<p>Turning the input on pushes a block <b>{n} cell{} out</b> of the machine; turning it off brings the block <b>back exactly</b> where it was, with nothing left behind. A plain piston only reaches 1.</p><p>{}{}</p>",
                if n == 1 { "" } else { "s" },
                if ext.is_empty() { String::new() } else { format!("Extending takes {ext}") },
                if ret.is_empty() { String::new() } else { format!(", retracting {ret}.") }
            )
        }
        "flying-machine" => {
            let speed = metric(c, "speed");
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            if two_way {
                "<p>A <b>two-way flying machine</b>: pistons, slime or honey and observers that push each other along, then come back. Once started it shuttles back and forth by itself.</p>".to_string()
            } else {
                let per_s = speed.strip_suffix(" gt/block").and_then(|n| n.trim().parse::<f64>().ok()).map(|g| format!(" ({:.1} blocks per second)", 20.0 / g)).unwrap_or_default();
                format!(
                    "<p>A <b>flying machine</b>: pistons, slime or honey and observers that push each other along. Once started (the start block is placed), it moves itself one block at a time and keeps going until something stops it.</p><p>{}</p>",
                    if speed.is_empty() { String::new() } else { format!("It moves one block every {}{per_s}.", esc(speed).replace(" gt/block", " gt")) }
                )
            }
        }
        _ => format!("<p>{}</p>", esc(&kind_summary(&c.kind))),
    }
}

/// `2nd`, `3rd`, `11th`, `21st`.
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// The input's control in the viewer, as the page names it (the viewer labels its bar buttons
/// with the port name uppercased).
fn input_label(name: &str) -> String {
    name.to_uppercase()
}

/// The kinds whose inputs are steady (toggled) rather than pulsed (webrender's input mode).
pub fn input_mode(kind: &str) -> &'static str {
    match base(kind) {
        "gate" | "d-latch" | "piston-extender" => "toggle",
        _ => "pulse",
    }
}

/// How to try the circuit in the viewer: the demo, the input buttons, the markers.
pub fn how_to_try(c: &Circuit, has_viewer: bool) -> String {
    if !has_viewer {
        return "<p>Download the design (below) to build it in the game; the 3D view needs a browser with WebGL.</p>".into();
    }
    let mut s = String::new();
    if base(&c.kind) == "flying-machine" {
        s.push_str("<p>The machine starts by itself (its start block is placed when the view begins): press <b>play</b> and watch it fly along its corridor; <b>reset</b> to see it again. Drag to look around, pinch or scroll to zoom.</p>");
        return s;
    }
    if !c.stimulus.is_empty() {
        s.push_str("<p><b>The view first plays the judge's own test</b> on a loop (the badge reads <i>demo</i> with the step it is on; the same test is drawn as a timing diagram below). Touch any control to stop it and try the circuit yourself.</p>");
    }
    let mut controls: Vec<String> = Vec::new();
    for (i, name) in c.inputs.iter().enumerate() {
        let device = c.devices.get(i).map(String::as_str).unwrap_or("redstone block");
        let label = input_label(name);
        let what = match device {
            "lever" => format!("click the <b>lever</b> tagged <b>{label}</b> (or the {label} button of the bar) to switch it on or off"),
            "button" => format!("click the <b>button</b> tagged <b>{label}</b> (or the {label} button of the bar) to press it"),
            _ if input_mode(&c.kind) == "toggle" => {
                format!("tap the <b>{label}</b> button of the bar (or the cell tagged {label}) to turn that input on or off (it powers the cell with a redstone block)")
            }
            _ => format!("press and hold the <b>{label}</b> button of the bar (or the cell tagged {label}) to power that input; a quick tap sends a short 2 gt pulse"),
        };
        controls.push(what);
    }
    if !controls.is_empty() {
        s.push_str(&format!("<p>To try it: {}.</p>", controls.join("; ")));
    }
    let outs: Vec<String> = c.outputs.iter().map(|o| if o == "out" { "OUT".to_string() } else { o.to_uppercase() }).collect();
    if !outs.is_empty() {
        s.push_str(&format!(
            "<p>Inputs are tagged in <span class=\"tag-in\">lime</span>; the output is the <span class=\"tag-out\">orange</span> glass tagged <b>{}</b>, which <b>glows while the output is on</b> (the strip under the tick counter shows its last 40 gt). Drag to look around, pinch or scroll to zoom, step one game tick at a time with the step button.</p>",
            esc(&outs.join(", "))
        ));
    }
    s
}

/// What a metric means.
pub fn metric_definition(kind: &str, k: &str) -> &'static str {
    match (base(kind), k) {
        (_, "L") => "the length of the output pulse",
        ("piston-extender", "N") => "the reach: how many cells the block is pushed out",
        ("counter", "N") => "how many input pulses one output cycle takes",
        (_, "N") => "the delay: how much later the output copies the input",
        ("flying-machine", "latency") => "game ticks from the start until the machine first moves",
        (_, "latency") => "the worst time from an input change to the output's change",
        (_, "delay") => "the time from the input's edge to the start of the output pulse",
        (_, "min-pulse") => "the shortest input pulse handled correctly (every longer one works too)",
        (_, "min-gap") => "the shortest gap between two input pulses that still comes out as two pulses",
        (_, "min-period") => "the fastest train of pulses it follows: one pulse every this many game ticks",
        (_, "edge") => "which input edge flips the output: rising = the input turning on, falling = turning off",
        (_, "function") => "the logic function the truth table names",
        (_, "class") => "instant = the output changes in the same game tick as the input (0 gt latency)",
        (_, "glitches") => "extra output changes while the inputs change",
        (_, "enable") => "high: the latch follows D while E is on; low: while E is off",
        (_, "output") => "how the output shows the count: toggles (flips every N), pulses (one pulse every N) or level (its state repeats every N)",
        (_, "extend") => "game ticks from switching the input on until the block is fully out",
        (_, "retract") => "game ticks from switching the input off until the block is back",
        (_, "speed") => "game ticks per block travelled (20 gt = 1 second)",
        (_, "period") => "game ticks per cycle of the engine (one push of each piston)",
        (_, "two-way") => "goes and comes back by itself",
        _ => "",
    }
}

/// A metric's value with seconds (`4 gt (0.2 s)`).
pub fn metric_value(v: &str) -> String {
    let v = v.trim();
    if let Some(n) = v.strip_suffix(" gt").and_then(|n| n.trim().parse::<u32>().ok()) {
        return format!("{n} gt ({})", real_time(n));
    }
    v.to_string()
}

/// The studies, in order: (folder, title, what it collects and how designs are ranked).
pub const STUDIES: [(&str, &str, &str); 7] = [
    (
        "pulse",
        "Pulse circuits",
        "Circuits that turn one signal shape into another. Edge detectors make one short pulse when the input turns on (rising edge), off (falling edge) or both (dual edge); pulse limiters cut long pulses down to a fixed length L; pulse extenders stretch short pulses to at least L. The study's axis is L, the output pulse length: for each L, the designs are ranked by blocks placed, then by the size of their box.",
    ),
    (
        "tff",
        "T flip-flops",
        "Toggles: each input pulse flips the output (on, off, on...). The study collects one design per mechanism family (the block kinds that change state: copper bulbs, observers and pistons, droppers and hoppers, comparators...) and ranks them by blocks placed; it also measures how short a pulse and how fast a pulse train each one follows.",
    ),
    (
        "delay",
        "Delay lines",
        "The output is the input N game ticks later, both edges, pulse lengths kept. The axis is N; for each N the designs are ranked by blocks placed. Families that grow by a repeated step (one more repeater or comparator per 2 gt) cover every N.",
    ),
    (
        "gates",
        "Logic gates",
        "NOT and every function of two inputs (AND, OR, XOR, XNOR, NAND, NOR, IMPLY, NIMPLY), each checked in every input combination and every transition between them. The axis is the function; for each function the designs are ranked by blocks placed, then by latency. Instant gates (the output changes in the same game tick, 0 gt) are their own class.",
    ),
    (
        "memory",
        "Memory",
        "Circuits that remember: RS latches (set / reset), D latches (follow D while enabled, hold otherwise) and counters that divide the number of input pulses by N. The axis is the function, and N for counters; designs are ranked by blocks placed.",
    ),
    (
        "pext",
        "Piston extenders",
        "Machines that push a block N cells out with one input and bring it back exactly with the next, nothing left behind (a plain piston reaches 1). The axis is the reach N; designs are ranked by blocks placed, then by extend and retract times.",
    ),
    (
        "fly",
        "Flying machines",
        "Self-propelled structures of pistons, slime or honey and observers: once started they move one block at a time by themselves. Grouped by kind (one-way engines in each direction, two-way machines) and speed in game ticks per block; ranked by blocks placed.",
    ),
];

pub fn study(folder: &str) -> Option<(&'static str, &'static str)> {
    STUDIES.iter().find(|s| s.0 == folder).map(|s| (s.1, s.2))
}

/// The about page's glossary.
pub fn glossary() -> String {
    let items: [(&str, &str); 15] = [
        ("gt (game tick)", "the game's time step: 20 game ticks = 1 second, so 1 gt = 0.05 s. Every time on this site is in gt, with seconds next to it."),
        ("rt (redstone tick)", "2 game ticks, the unit repeater delays are counted in (a repeater set to 1 waits 1 rt = 2 gt)."),
        ("input / output", "the cells a circuit is driven from and read at. In the 3D view inputs are tagged in lime, the output is orange glass that glows while the output is on."),
        ("pulse", "the input (or output) on for a while, then off again; its length in gt."),
        ("edge", "the moment a signal changes: rising edge = it turns on, falling edge = it turns off."),
        ("L", "the length of an output pulse (pulse circuits)."),
        ("N", "the main number of a study: a delay line's delay in gt, a counter's count, a piston extender's reach in blocks."),
        ("delay", "time from the input's edge to the start of the output pulse."),
        ("latency", "the worst time from an input change to the output's change; 0 gt = instant."),
        ("min-pulse", "the shortest input pulse a circuit handles correctly (every longer one works too)."),
        ("min-gap", "the shortest gap between two pulses that a delay line still keeps apart."),
        ("min-period", "the fastest pulse train a T flip-flop follows: one pulse every this many gt."),
        ("period", "a clock's (or an engine's) cycle length in gt: its whole state repeats every period."),
        ("blocks", "blocks placed (a piston head is part of its piston; the input device is not counted); entities count one each."),
        ("robust", "it works the same at 4 positions and 4 rotations in the real game; otherwise position- or rotation-dependent."),
    ];
    let mut s = String::from("<dl class=\"glossary\">\n");
    for (k, v) in items {
        s.push_str(&format!("<dt id=\"g-{}\">{}</dt><dd>{}</dd>\n", k.split_whitespace().next().unwrap_or(k).to_lowercase(), esc(k), esc(v)));
    }
    s.push_str("</dl>\n");
    s
}

/// A caption without its simulator attribution (vjeux: "we don't need the attribution"): every
/// sentence naming redsim goes.
pub fn scrub_attribution(caption: &str) -> String {
    if !caption.to_lowercase().contains("redsim") {
        return caption.to_string();
    }
    let mut out: Vec<&str> = Vec::new();
    for s in caption.split_inclusive(". ") {
        if !s.to_lowercase().contains("redsim") {
            out.push(s);
        }
    }
    out.concat().trim().trim_end_matches(|c: char| c == ',' || c == ';').to_string()
}

/// The design's caption without its bookkeeping prefix (`Counter 2, dropper ring, 8 blocks
/// (track memory, ...): ` -> the explanation after it).
pub fn how_sentence(caption: &str) -> String {
    let c = caption.trim();
    if let Some(i) = c.find("): ") {
        let head = &c[..i];
        if i < 220 && head.contains('(') && (head.contains("track") || head.contains("census") || head.contains("found by") || head.contains("family")) {
            return site_model::html::esc(&first_cap(&c[i + 3..]));
        }
    }
    site_model::html::esc(c)
}

fn first_cap(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
        None => String::new(),
    }
}

/// The timing diagram of the judge's test (redsim): one row per input and output, lime for
/// inputs, orange for outputs (an output's height = its strength 0..15), the checker's phases
/// as separators with labels when they fit. Pixel sizes fixed (scrolls when long).
pub fn wave_svg(w: &Wave) -> String {
    let ticks = w.ticks.max(1) as f64;
    let px = (960.0 / ticks).clamp(0.6, 12.0);
    let label_w = 56.0;
    let row_h = 34.0;
    let top = 22.0;
    let width = label_w + ticks * px + 8.0;
    let height = top + row_h * w.rows.len() as f64 + 22.0;
    let x = |t: f64| label_w + t * px;
    let mut s = format!(
        "<svg class=\"wave\" xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\" aria-label=\"timing diagram of the judge's test\">"
    );
    // phases: a separator at each start, a label when it fits
    for (from, to, label) in &w.phases {
        let (a, b) = (x(*from as f64), x(*to as f64));
        s.push_str(&format!("<line class=\"ph\" x1=\"{a:.1}\" y1=\"{:.1}\" x2=\"{a:.1}\" y2=\"{:.1}\"/>", top - 4.0, height - 18.0));
        if (b - a) >= label.len() as f64 * 6.2 + 6.0 {
            s.push_str(&format!("<text class=\"phl\" x=\"{:.1}\" y=\"{:.1}\">{}</text>", a + 3.0, top - 8.0, esc(label)));
        }
    }
    for (i, r) in w.rows.iter().enumerate() {
        let y0 = top + row_h * i as f64 + row_h - 8.0; // baseline
        let hmax = row_h - 14.0;
        s.push_str(&format!("<text class=\"wl\" x=\"4\" y=\"{:.1}\">{}</text>", y0 - 2.0, esc(&if r.input { r.name.to_uppercase() } else if r.name == "out" { "OUT".into() } else { r.name.to_uppercase() })));
        s.push_str(&format!("<line class=\"base\" x1=\"{:.1}\" y1=\"{y0:.1}\" x2=\"{:.1}\" y2=\"{y0:.1}\"/>", x(0.0), x(ticks)));
        // the step outline as a filled polygon
        let mut pts: Vec<String> = vec![format!("{:.1},{y0:.1}", x(0.0))];
        for (k, (t, l)) in r.steps.iter().enumerate() {
            let end = r.steps.get(k + 1).map(|n| n.0 as f64).unwrap_or(ticks);
            // any signal shows: a weak comparator output (strength 1) still stands 40% tall
            let frac = if *l == 0 { 0.0 } else { 0.4 + 0.6 * (*l as f64 / 15.0) };
            let y = y0 - hmax * frac;
            pts.push(format!("{:.1},{y:.1}", x(*t as f64)));
            pts.push(format!("{:.1},{y:.1}", x(end)));
        }
        pts.push(format!("{:.1},{y0:.1}", x(ticks)));
        s.push_str(&format!("<polygon class=\"{}\" points=\"{}\"/>", if r.input { "win" } else { "wout" }, pts.join(" ")));
    }
    // a time axis: a tick every 20, 100 or 500 gt
    let step = [20u32, 50, 100, 200, 500, 1000].into_iter().find(|s| *s as f64 * px >= 60.0).unwrap_or(2000);
    let mut t = 0u32;
    let ya = height - 6.0;
    while (t as f64) <= ticks {
        s.push_str(&format!("<text class=\"ax\" x=\"{:.1}\" y=\"{ya:.1}\">{}</text>", x(t as f64), if t == 0 { "0".to_string() } else { format!("{t} gt") }));
        t += step;
    }
    s.push_str("</svg>");
    s
}

/// A short line under a circuit card: its kind for people.
pub fn card_kind(b: &Build) -> Option<String> {
    b.circuit.as_ref().map(|c| kind_title(&c.kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circuit(kind: &str, metrics: &[(&str, &str)], inputs: &[&str]) -> Circuit {
        Circuit {
            kind: kind.into(),
            metrics: metrics.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            inputs: inputs.iter().map(|s| s.to_string()).collect(),
            outputs: vec!["out".into()],
            ..Default::default()
        }
    }

    #[test]
    fn counters_and_gates_read_plainly() {
        let c = circuit("counter 2", &[("N", "2"), ("output", "level every 2 pulses")], &["in"]);
        let w = what_it_does(&c);
        assert!(w.contains("counts input pulses"), "{w}");
        assert!(w.contains("divides the number of pulses by 2"), "{w}");
        let g = circuit("gate xor", &[("function", "xor"), ("latency", "0 gt"), ("class", "instant")], &["a", "b"]);
        let w = what_it_does(&g);
        assert!(w.contains("exactly one of A and B"), "{w}");
        assert!(w.contains("instant"), "{w}");
        // XOR's truth table: off on on off
        let t = truth_table("xor", &["a".into(), "b".into()]);
        assert_eq!(t.matches("class=\"on\">on</td></tr>").count(), 2, "{t}");
        let n = truth_table("not", &["in".into()]);
        assert!(n.contains("<th>IN</th>"), "{n}");
    }

    #[test]
    fn edge_and_delay_numbers_have_seconds() {
        let c = circuit("rising-edge", &[("L", "2 gt"), ("delay", "5 gt"), ("min-pulse", "4 gt")], &["in"]);
        let w = what_it_does(&c);
        assert!(w.contains("L = 2 gt (0.1 s)") && w.contains("5 gt (0.25 s) after"), "{w}");
        assert_eq!(metric_value("4 gt"), "4 gt (0.2 s)");
        assert_eq!(metric_definition("tff", "min-period"), "the fastest train of pulses it follows: one pulse every this many game ticks");
        assert_eq!(metric_definition("counter 3", "N"), "how many input pulses one output cycle takes");
    }

    #[test]
    fn how_to_try_names_the_controls() {
        let mut c = circuit("counter 2", &[], &["in"]);
        c.devices = vec!["redstone block".into()];
        c.stimulus = "in 40+ 44-".into();
        let h = how_to_try(&c, true);
        assert!(h.contains("press and hold the <b>IN</b> button"), "{h}");
        assert!(h.contains("judge's own test"), "{h}");
        let mut g = circuit("gate and", &[], &["a", "b"]);
        g.devices = vec!["lever".into(), "redstone block".into()];
        let h = how_to_try(&g, true);
        assert!(h.contains("click the <b>lever</b> tagged <b>A</b>") && h.contains("tap the <b>B</b> button"), "{h}");
    }

    #[test]
    fn captions_lose_their_bookkeeping() {
        assert_eq!(
            how_sentence("Counter 2, dropper ring, 8 blocks (track memory, `mctas circuit memory ring --n 2`): the input cell fires 2 droppers."),
            "The input cell fires 2 droppers."
        );
        assert_eq!(how_sentence("An observer watching the input."), "An observer watching the input.");
        assert_eq!(scrub_attribution("Two hoppers trade a sword. redsim: P=8 gt, warmup 2"), "Two hoppers trade a sword.");
        assert_eq!(scrub_attribution("No mention."), "No mention.");
    }

    #[test]
    fn wave_draws_rows_and_phases() {
        let w = Wave {
            ticks: 100,
            rows: vec![site_model::WaveRow::from_levels("in", true, &[0, 0, 15, 15, 0]), site_model::WaveRow::from_levels("out", false, &[0, 0, 0, 7, 15])],
            phases: vec![(0, 50, "pulse 2".into()), (50, 100, "pulse 3".into())],
        };
        let s = wave_svg(&w);
        assert!(s.starts_with("<svg") && s.ends_with("</svg>"));
        assert_eq!(s.matches("<polygon").count(), 2);
        assert!(s.contains(">pulse 2</text>") && s.contains(">IN</text>") && s.contains(">OUT</text>"));
    }
}
