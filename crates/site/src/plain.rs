//! Plain player language for every build page (vjeux 13:06Z: "I have no clue what a monostable
//! retriggerable system means. Can you make sure you explain all the builds so someone can
//! understand them"): what the build does in one sentence with its own numbers, why you would
//! build it (a concrete use), and how to try it in the 3D view. No jargon, no judge metrics, no
//! raw ids: those live in the page's collapsed details.

use site_model::html::esc;
use site_model::{Build, Circuit, real_time};

fn m<'a>(c: &'a Circuit, k: &str) -> &'a str {
    c.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str()).unwrap_or("")
}

/// `32 gt` -> `32 ticks (1.6 s)`, `1 gt` -> `1 tick (0.05 s)`, `4-8 gt` -> `4 to 8 ticks`; a value
/// with words after the unit keeps them (`28 gt (from the press)` -> `28 ticks (1.4 s)`).
pub fn ticks(v: &str) -> String {
    let v = v.trim();
    let mut w = v.split_whitespace();
    let n = w.next().unwrap_or("");
    if w.next() != Some("gt") {
        return v.to_string();
    }
    if let Ok(x) = n.parse::<u32>() {
        return format!("{x} tick{} ({})", if x == 1 { "" } else { "s" }, real_time(x));
    }
    match n.split_once('-') {
        Some((a, b)) => format!("{a} to {b} ticks"),
        None => v.to_string(),
    }
}

/// `32 gt` -> `32 ticks` (no seconds: a card's headline).
pub fn ticks_short(v: &str) -> String {
    let v = v.trim();
    let mut w = v.split_whitespace();
    let n = w.next().unwrap_or("");
    if w.next() != Some("gt") {
        return v.to_string();
    }
    match n.parse::<u32>() {
        Ok(1) => "1 tick".to_string(),
        Ok(x) => format!("{x} ticks"),
        Err(_) => format!("{} ticks", n.replace('-', " to ")),
    }
}

fn number(kind: &str) -> u32 {
    kind.split_whitespace().nth(1).and_then(|n| n.parse().ok()).unwrap_or(0)
}

fn base(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or("")
}

pub fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A logic function in plain words: when the output is on.
fn gate_words(f: &str) -> &'static str {
    match f {
        "not" => "The output is on when the input is off, and off when the input is on",
        "buffer" => "The output copies the input",
        "and" => "The output is on only when both inputs, A and B, are on",
        "or" => "The output is on when A or B (or both) is on",
        "xor" => "The output is on when exactly one of A and B is on, not both",
        "xnor" => "The output is on when A and B are the same: both on or both off",
        "nand" => "The output is on unless both A and B are on",
        "nor" => "The output is on only when both A and B are off",
        "imply" => "The output is off only when A is on and B is off",
        "nimply" => "The output is on only when A is on and B is off",
        "imply-ba" => "The output is off only when B is on and A is off",
        "nimply-ba" => "The output is on only when B is on and A is off",
        "not-a" => "The output is on when A is off (B does not matter)",
        "not-b" => "The output is on when B is off (A does not matter)",
        _ => "The output is a fixed combination of the inputs (see the truth table in the details)",
    }
}

fn gate_use(f: &str) -> &'static str {
    match f {
        "not" => "turn a signal around: a lamp that lights while a lever is off, a door that opens when a pressure plate is released",
        "and" => "a door that only opens when two levers are both on, a safety switch",
        "or" => "two buttons (inside and outside) that open the same door",
        "xor" => "two light switches for one light, like at the top and bottom of stairs",
        "xnor" => "check that two signals agree: a combination lock",
        "nand" | "nor" => "the building block of every other gate and of memory cells",
        "imply" | "nimply" | "imply-ba" | "nimply-ba" => "let one signal block another: a lever that disables a button",
        _ => "combine signals in a contraption",
    }
}

/// What it does, in one sentence with its own numbers.
pub fn one_liner(c: &Circuit) -> String {
    let n = number(&c.kind);
    let l = ticks(m(c, "L"));
    match base(&c.kind) {
        "rising-edge" => format!("When the input turns on, the output gives one short pulse of {l}; turning the input off does nothing."),
        "falling-edge" => format!("When the input turns off, the output gives one short pulse of {l}; turning the input on does nothing."),
        "dual-edge" => format!("Every time the input changes, on or off, the output gives one short pulse of {l}."),
        "pulse-limiter" => format!("However long the input stays on, the output pulse lasts at most {l}."),
        "pulse-extender" => format!("Even the shortest input pulse comes out at least {l} long."),
        "tff" => "Press once and the output turns on; press again and it turns off.".to_string(),
        "tff-reset" => "Each press flips the output on or off; a press on R turns it off.".to_string(),
        "delay" => format!("Whatever goes into the input comes out {} later, every pulse the same length.", ticks(&format!("{} gt", if m(c, "N").is_empty() { c.axis.trim_start_matches("N = ").trim_end_matches(" gt").to_string() } else { m(c, "N").trim_end_matches(" gt").to_string() }))),
        "gate" => {
            let f = if m(c, "function").is_empty() { c.kind.split_whitespace().nth(1).unwrap_or("") } else { m(c, "function") };
            let instant = m(c, "class") == "instant" || m(c, "latency") == "0 gt";
            format!("{}{}.", gate_words(f), if instant { ", in the very same tick (an instant gate)" } else { "" })
        }
        "rs-latch" => "Press S and the output turns on and stays on; press R and it turns off and stays off.".to_string(),
        "d-latch" => "While E is on, the output copies D; when E turns off, the output keeps the last value.".to_string(),
        "dff" => "At each clock pulse the output takes the value of D and keeps it until the next clock pulse.".to_string(),
        "dff-enable" => "At each clock pulse, if E is on, the output takes the value of D; otherwise it keeps its value.".to_string(),
        "shift" => format!("Each clock pulse moves every stored bit one step along: what goes in comes out {n} clock pulses later."),
        "counter" => {
            let style = m(c, "output");
            if style.starts_with("toggles") {
                format!("Every {} input pulse the output flips: on for {n} pulses, off for the next {n}.", ordinal(n))
            } else if style.starts_with("pulses") {
                format!("The output gives one pulse for every {n} input pulses.")
            } else {
                match c.counter_on {
                    Some((k, mm)) if mm == n && k > 0 && k < n => format!("The output's on/off pattern repeats every {n} input pulses: on for {k} of them, off for the other {}.", n - k),
                    _ => format!("The output's on/off pattern repeats every {n} input pulses."),
                }
            }
        }
        "piston-extender" => {
            let (e, r) = (ticks(m(c, "extend")), ticks(m(c, "retract")));
            format!(
                "Turn the input on and a block is pushed {n} blocks out; turn it off and the block comes back exactly where it was.{}",
                if e.is_empty() { String::new() } else { format!(" It goes out in {e} and comes back in {r}.") }
            )
        }
        "flying-machine" => {
            let speed = m(c, "speed");
            let two_way = c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way");
            let per = speed.split_whitespace().next().and_then(|g| g.parse::<f64>().ok());
            let pace = match per {
                Some(g) if g > 0.0 => format!(", one block every {} ({} blocks a second)", ticks_short(&format!("{} gt", g as u32)), trim(20.0 / g)),
                _ => String::new(),
            };
            if two_way {
                format!("Once started it flies one way, then comes back by itself{pace}.")
            } else {
                let dir = if c.direction.is_empty() { String::new() } else { format!(" {}", c.direction) };
                format!("Once started it flies{dir} by itself{pace}, until something stops it.")
            }
        }
        "shulker-loader" => format!("Takes items from a chest, puts them into empty shulker boxes and sends each full box out{}.", rate(m(c, "items/s"))),
        "shulker-unloader" => format!("Empties full shulker boxes into a chest and sends the empty boxes out{}.", rate(m(c, "items/s"))),
        "bud" => {
            let d: Vec<&str> = m(c, "detects").split_whitespace().collect();
            let words: Vec<&str> = d.iter().map(|k| match *k {
                "place" => "placed",
                "remove" => "removed",
                "state" => "changed",
                "shape" => "grows (a shape-only change)",
                "update" => "updated with nothing changing",
                other => other,
            }).collect();
            format!("Fires its output when a block next to it (the TARGET cell) is {}, even though nothing powers it.", if words.is_empty() { "changed".to_string() } else { list_words(&words, "or") })
        }
        "tnt-duper" => format!("Lights {} without ever using up its own TNT block.", if m(c, "rate").is_empty() { "TNT over and over".to_string() } else { m(c, "rate").to_string() }),
        "tnt-duper-triggered" => format!("Each press lights {} TNT without using up the TNT block.", if m(c, "per pulse").is_empty() { "one" } else { m(c, "per pulse") }),
        "burst" => format!("One press gives exactly {n} pulses{}.", if m(c, "period").is_empty() { String::new() } else { format!(", {} apart", ticks_short(m(c, "period"))) }),
        "divider" => format!(
            "Feed it a clock and the output goes on and off once for every {n} input pulses{}.",
            if m(c, "output") == "square" { ", on for exactly half of the time" } else { "" }
        ),
        "monostable" => {
            if c.kind.contains("non-retriggerable") {
                format!("One press keeps the output on for {l}; presses while it is on are ignored.")
            } else if c.kind.contains("retriggerable") {
                format!("One press keeps the output on for {l}; pressing again while it is on restarts the countdown.")
            } else {
                format!("One press, however short, keeps the output on for {l}.")
            }
        }
        "off-delay" => format!("The output turns on with the input and stays on {l} after the input turns off."),
        "on-delay" => format!("The output only turns on once the input has been on for {}; shorter presses do nothing.", ticks(m(c, "T"))),
        "pulse-train" => format!("While the input is on, the output pulses every {}.", ticks(m(c, "P"))),
        _ => {
            let s = crate::explain::kind_summary(&c.kind);
            if s.is_empty() { String::new() } else { format!("{}.", first_cap(&s)) }
        }
    }
}

/// Why you would build it: a concrete use.
pub fn why_build(c: &Circuit) -> String {
    let n = number(&c.kind);
    let s: String = match base(&c.kind) {
        "rising-edge" => "turn a lever into a single pulse: fire a dispenser or a dropper once per flip, or start a machine without holding it on".into(),
        "falling-edge" => "do something when a lever is switched off or a player steps off a pressure plate: close a door behind someone".into(),
        "dual-edge" => "react to both flips of one lever: one lever that toggles something on every flip, or a clock running twice as fast".into(),
        "pulse-limiter" => "stop a long press from keeping a piston or a dispenser busy: one short pulse however long a lever stays on".into(),
        "pulse-extender" => "make a 1-tick observer pulse long enough for a piston, a door or a lamp to notice it".into(),
        "tff" | "tff-reset" => "turn a button, which only gives a short pulse, into an on/off switch: one press opens a door, the next press closes it".into(),
        "delay" => "time things: fire pistons one after another, or make a door wait before it closes".into(),
        "gate" => {
            let f = if m(c, "function").is_empty() { c.kind.split_whitespace().nth(1).unwrap_or("") } else { m(c, "function") };
            gate_use(f).into()
        }
        "rs-latch" => "remember that something happened: an alarm that stays on after someone crosses a tripwire, until you reset it".into(),
        "d-latch" | "dff" | "dff-enable" => "store one bit: a digit of a combination lock, the floor an elevator is on".into(),
        "shift" => "move a pattern along a row: running lights, a sequence of pistons, a queue of items".into(),
        "counter" => format!("do something every {} time: every {} item, every {} player through a door, or slow a fast clock down", ordinal(n), ordinal(n), ordinal(n)),
        "piston-extender" => "push a block several blocks away and get it back: hidden doors, bridges that extend, secret staircases".into(),
        "flying-machine" => "move things across the world with no power line: quarries, tree farms, slime-block trains, travel".into(),
        "shulker-loader" => "pack the output of a farm into shulker boxes by itself, ready to carry away".into(),
        "shulker-unloader" => "unpack shulker boxes into a storage system without opening them by hand".into(),
        "bud" => "notice a change you cannot otherwise detect: a crop or a tree growing, someone placing or breaking a block".into(),
        "tnt-duper" => "endless TNT for world eaters, tunnel bores and cannons".into(),
        "tnt-duper-triggered" => "cannons and single shots: one TNT per press, without crafting any".into(),
        "burst" => format!("fire a dispenser or a dropper exactly {n} times, or push a piston {n} times, from one button press"),
        "divider" => "slow a fast clock down: run a farm, a sorter or a light show at a slower beat".into(),
        "monostable" if c.kind.contains("non-retriggerable") => "one shot per press however much the button is mashed: a door that opens for a fixed time".into(),
        "monostable" => "keep a door open while people keep stepping on a pressure plate, or a light that stays on while there is activity".into(),
        "off-delay" => "a light or a door that stays on a while after the last press of a pressure plate".into(),
        "on-delay" => "ignore accidental taps and observer flicker: a door that only opens if you hold the button".into(),
        "pulse-train" => "rapid fire while a lever is on: dispensers, droppers, item spammers".into(),
        _ => String::new(),
    };
    if s.is_empty() { String::new() } else { format!("{}.", first_cap(&s)) }
}

/// How to try it in the 3D view, in one or two short sentences.
pub fn try_it(c: &Circuit, has_viewer: bool) -> String {
    if !has_viewer {
        return "Download the design below to build it in the game (the 3D view needs a browser with WebGL).".to_string();
    }
    let b = base(&c.kind);
    match b {
        "flying-machine" => return "It starts by itself: press play under the view and watch it fly; reset to see it again.".to_string(),
        "tnt-duper" => return "It runs by itself: watch the lit TNT drop out of the machine each cycle while the TNT block lands back in place.".to_string(),
        "shulker-loader" | "shulker-unloader" if !c.unmodeled.is_empty() => return "The view shows it as built, with our test's chests; download it to try it in the game.".to_string(),
        "shulker-loader" | "shulker-unloader" => {
            let lever = c.inputs.iter().zip(c.devices.iter()).any(|(n, d)| d == "lever" && !(n == "mode" || n.starts_with("mode.")));
            return format!(
                "{}Watch the items and boxes move between the chests (the numbers over each container show what it holds); the speed button goes up to 100×.",
                if lever { "Flip its start lever (the lime IN button under the view). " } else { "It starts by itself. " }
            );
        }
        "bud" => return "The view replays our test: a block is placed, removed and changed at the cell marked TARGET; watch the lamp marked OUT.".to_string(),
        _ => {}
    }
    if !c.unmodeled.is_empty() {
        return "The view shows it as built (the browser's simulator cannot run it yet); download it to try it in the game.".to_string();
    }
    let toggle = crate::explain::input_mode(&c.kind) == "toggle";
    let device = c.devices.first().map(String::as_str).unwrap_or("");
    let names: Vec<String> = c.inputs.iter().map(|n| n.to_uppercase()).collect();
    let press = match (device, names.len()) {
        (_, 0) => String::new(),
        ("lever", 1) => format!("Flip the lever marked {} (or tap {} under the view)", names[0], names[0]),
        ("button", 1) => format!("Press the button marked {} (or tap {} under the view)", names[0], names[0]),
        (_, 1) if toggle => format!("Tap {} under the view to turn the input on, again to turn it off", names[0]),
        (_, 1) => format!("Hold {} under the view for a long press, tap it for a short one", names[0]),
        _ => format!("Use the {} buttons under the view to drive the inputs", list_words(&names.iter().map(String::as_str).collect::<Vec<_>>(), "and")),
    };
    let watch = if c.outputs.is_empty() { "watch the machine move" } else { "the lamp marked OUT lights while the output is on" };
    let hint = match b {
        "piston-extender" => format!("turn it on and watch the block go {} blocks out; turn it off and it comes back", number(&c.kind)),
        "burst" => format!("each tap gives exactly {} flashes of OUT", number(&c.kind)),
        "divider" | "counter" if number(&c.kind) > 16 => format!("the demo counts the {} pulses for you: set the speed to 100× and watch OUT change", number(&c.kind)),
        "divider" | "counter" => format!("tap it {} times and OUT goes through one on-off cycle", number(&c.kind)),
        "monostable" if c.kind.contains("non-retriggerable") => "a tap keeps OUT lit for a while; tapping again while it is lit changes nothing".to_string(),
        "monostable" => "a tap keeps OUT lit for a while; tap again while it is lit and it stays lit longer".to_string(),
        "off-delay" => "turn it on, then off: OUT stays lit a while after".to_string(),
        "on-delay" => "a quick tap does nothing; hold it and OUT lights".to_string(),
        "pulse-train" => "turn it on and OUT keeps flashing until you turn it off".to_string(),
        "tff" | "tff-reset" => "each tap flips OUT on or off".to_string(),
        _ => String::new(),
    };
    let demo = if c.stimulus.is_empty() { String::new() } else { " The view first plays our test on a loop; touch any control to take over.".to_string() };
    if press.is_empty() {
        return format!("Watch the view: {watch}.{demo}");
    }
    format!(
        "{press}: {}.{demo}",
        match (hint.is_empty(), c.outputs.is_empty()) {
            (true, _) => watch.to_string(),
            (false, true) => hint,
            (false, false) => format!("{hint} ({watch})"),
        }
    )
}

fn rate(items_s: &str) -> String {
    if items_s.is_empty() { String::new() } else { format!(" ({items_s} items a second)") }
}

fn trim(x: f64) -> String {
    let s = format!("{x:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn list_words(v: &[&str], and: &str) -> String {
    match v.len() {
        0 => String::new(),
        1 => v[0].to_string(),
        _ => format!("{} {and} {}", v[..v.len() - 1].join(", "), v[v.len() - 1]),
    }
}

fn first_cap(s: &str) -> String {
    let mut ch = s.chars();
    match ch.next() {
        Some(f) => f.to_uppercase().collect::<String>() + ch.as_str(),
        None => String::new(),
    }
}

/// What a builder must know before building it, near the top (critique, coordinator 13:54Z):
/// a sculk door can be set off by footsteps; a directional build works only facing one way.
pub fn warning(b: &Build) -> Option<String> {
    if b.kind == "door" && b.group == "sculk" {
        return Some("Uses a sculk sensor: footsteps or other noises nearby can open or close it. Keep walkways away from it, or shield it with wool.".to_string());
    }
    // a vibration-proof sculk circuit (clk-obs's vibration test; the sensitive ones are hidden)
    if let Some(c) = &b.circuit {
        let proof = c.metrics.iter().any(|(k, v)| k == "vibration-proof" && v.starts_with("yes"));
        if proof && b.kinds.iter().any(|k| k.contains("sculk_sensor")) {
            return Some("Uses a sculk sensor tuned so footsteps and other noises don't trigger it.".to_string());
        }
    }
    let robustness = match (&b.circuit, &b.clock) {
        (Some(c), _) => c.robustness.as_str(),
        (None, Some(c)) => c.robustness.as_str(),
        _ => "",
    };
    let directional = robustness.starts_with("rotation") || (b.kind == "door" && b.door.is_some() && !b.flags.iter().any(|f| f == "rotation-safe"));
    if directional {
        return Some("Works only when built facing the way it is shown here: turned another way, it breaks.".to_string());
    }
    if robustness.contains("position") || robustness.contains("offset") {
        return Some("Depends on where it is built: it does not work at every spot in the world, so test it where you build it.".to_string());
    }
    None
}

/// A door's plain lines: (what it does, why you would build it, how to try it).
pub fn door(b: &Build) -> (String, String, String) {
    let d = b.door.as_ref();
    let timing = d.and_then(|d| Some(format!(" It opens in {} and closes in {}.", ticks(&format!("{} gt", d.open?)), ticks(&format!("{} gt", d.close?))))).unwrap_or_default();
    // a sculk door: the lever's click is what its sensor hears (critique: "say the sensor hears
    // the lever's click")
    let what = if b.group == "sculk" {
        format!("Flip the lever: the sculk sensor hears its click and the 3×3 doorway opens; flip it back and it closes.{timing}")
    } else {
        format!("Flip the lever and the 3×3 doorway opens; flip it back and it closes.{timing}")
    };
    let seamless = d.is_some_and(|d| !d.seamless.is_empty());
    let why = match b.group.as_str() {
        "sculk" => "a hidden 3×3 entrance for a base.".to_string(),
        "gravity" => "a 3×3 entrance whose door is made of sand or gravel that falls back into place.".to_string(),
        "entity" => "a 3×3 entrance that uses an entity (a minecart or a falling block) to work.".to_string(),
        _ => format!("a 3×3 entrance for a base, opened by one lever{}.", if seamless { ", nothing of the machine showing when closed" } else { "" }),
    };
    (what, first_cap(&why), "Flip the lever (tap it in the view, or the flip button under it) and watch the door open; flip it again to close it.".to_string())
}

/// A clock's plain lines: (what it does, why you would build it, how to try it).
pub fn clock(b: &Build) -> (String, String, String) {
    let c = b.clock.as_ref();
    let period = c.map(|c| ticks(&format!("{} gt", c.period))).unwrap_or_default();
    let kicked = c.is_some_and(|c| !(c.start.is_empty() || c.start.starts_with("self")));
    let start = if kicked { " Built in the game it needs one kick to start (the last block placed); then it keeps going." } else { " It starts on its own once built." };
    (
        format!("Its output pulses by itself every {period}, forever.{start}"),
        "A steady beat for farms, item sorters, flashing lamps and anything that should run by itself.".to_string(),
        if kicked { "The view starts it for you, then it runs by itself: pause and step one tick at a time with the buttons under it." } else { "It runs by itself: the view plays it; pause and step one tick at a time with the buttons under it." }.to_string(),
    )
}

/// The "Try it" box under the 3D view: how to try it, and how this design does it (its plain
/// mechanism); what it does and why build it are in the heading, on the first screen.
pub fn intro_box(b: &Build, has_viewer: bool) -> String {
    let try_line = match (&b.circuit, b.kind.as_str()) {
        (Some(c), _) => try_it(c, has_viewer),
        (None, "door") => door(b).2,
        (None, "clock") => clock(b).2,
        _ => String::new(),
    };
    let how = crate::explain::caption_plain(&b.caption);
    let how = first_sentence(&how);
    if try_line.is_empty() && how.is_empty() {
        return String::new();
    }
    let mut s = String::from("<section class=\"what plain\"><h2>Try it</h2>");
    if !try_line.is_empty() {
        s.push_str(&format!("<p>{}</p>", esc(&try_line)));
    }
    if !how.is_empty() {
        s.push_str(&format!("<p class=\"howdesign\"><b>How this design does it:</b> {how}</p>"));
    }
    s.push_str("</section>\n");
    s
}

/// Why you would build it, for any build (the heading's second line).
pub fn why_any(b: &Build) -> String {
    match (&b.circuit, b.kind.as_str()) {
        (Some(_), _) if b.group == "Sand elevator" => "A sand elevator or a hidden floor: a block of sand that rises and falls back with one lever.".to_string(),
        (Some(c), _) => why_build(c),
        (None, "door") => door(b).1,
        (None, "clock") => clock(b).1,
        _ => String::new(),
    }
}

/// The first sentence of an (escaped) text: up to the first ". " that is not an abbreviation.
pub fn first_sentence(s: &str) -> String {
    let mut from = 0;
    while let Some(i) = s[from..].find(". ") {
        let end = from + i;
        let head = &s[..end];
        if head.len() >= 24 && !head.ends_with("e.g") && !head.ends_with("i.e") && !head.ends_with("vs") {
            return s[..=end].to_string();
        }
        from = end + 2;
    }
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circuit(kind: &str, metrics: &[(&str, &str)]) -> Circuit {
        Circuit { kind: kind.into(), metrics: metrics.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Default::default() }
    }

    #[test]
    fn plain_sentences_carry_the_numbers() {
        let mono = circuit("monostable retriggerable", &[("L", "32 gt")]);
        assert_eq!(one_liner(&mono), "One press keeps the output on for 32 ticks (1.6 s); pressing again while it is on restarts the countdown.");
        assert!(why_build(&mono).starts_with("Keep a door open while people keep stepping on a pressure plate"));
        assert_eq!(ticks("1 gt"), "1 tick (0.05 s)");
        assert_eq!(ticks("4-8 gt"), "4 to 8 ticks");
        assert_eq!(ticks_short("28 gt (from the press)"), "28 ticks");
        let ext = circuit("piston-extender 3", &[("extend", "27 gt"), ("retract", "56 gt (from the press)")]);
        assert!(one_liner(&ext).contains("pushed 3 blocks out") && one_liner(&ext).contains("comes back in 56 ticks"), "{}", one_liner(&ext));
        let gate = circuit("gate and", &[("function", "and"), ("latency", "0 gt")]);
        assert!(one_liner(&gate).contains("both inputs") && one_liner(&gate).contains("instant"));
    }
}
