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
        "delay" => {
            let n = if m(c, "N").is_empty() { c.axis.trim_start_matches("N = ").trim_end_matches(" gt").to_string() } else { m(c, "N").trim_end_matches(" gt").to_string() };
            // a 0-tick delay passes the signal on in the same tick: a signal carrier, not a delay
            // (critique: "0-tick delay" on a page promising "a moment later")
            if n == "0" {
                "Carries a signal to its output in the same game tick, with no delay at all: a wire that does not slow the signal down.".to_string()
            } else {
                format!("Whatever goes into the input comes out {} later, every pulse the same length.", ticks(&format!("{n} gt")))
            }
        }
        "gate" => {
            let f = if m(c, "function").is_empty() { c.kind.split_whitespace().nth(1).unwrap_or("") } else { m(c, "function") };
            let instant = m(c, "class") == "instant" || m(c, "latency") == "0 gt";
            // two-input gates introduce their inputs (critique: "A and B are never introduced")
            let two = !matches!(f, "not" | "buffer");
            format!("{}{}{}.", if two { "It has two inputs, A and B. " } else { "" }, gate_words(f), if instant { ", in the very same tick (an instant gate)" } else { "" })
        }
        "rs-latch" => "Press the set button (S) and the output turns on and stays on; press the reset button (R) and it turns off and stays off.".to_string(),
        "d-latch" => "Two inputs: while the enable input (E) is on, the output copies the data input (D); when E turns off, the output keeps the last value.".to_string(),
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
                // (direction words: "flies sideways", not "flies horizontal": critique)
                let dir = match c.direction.as_str() {
                    "" => String::new(),
                    "horizontal" => " sideways".to_string(),
                    d => format!(" {d}"),
                };
                let carries = if c.feature.is_empty() { String::new() } else { format!(" It {}.", c.feature) };
                format!("Once started it flies{dir} by itself{pace}, until something stops it.{carries}")
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
                "shape" => "grows (like a crop)",
                "update" => "updated with nothing changing",
                other => other,
            }).collect();
            format!(
                "Fires its output when the block next to it (the cell marked TARGET in the 3D view) is {}, without any redstone signal reaching it: a block update detector (BUD).",
                if words.is_empty() { "changed".to_string() } else { list_words(&words, "or") }
            )
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
        "delay" if c.axis.trim_start_matches("N = ").starts_with("0 ") || m(c, "N").starts_with("0 ") => "pass a signal on without slowing it, where every other component adds a tick: keep one part of a machine in step with another".into(),
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
        "monostable" => "a light that stays on a few seconds after each press, restarting with every press: a hallway light on a pressure plate".into(),
        "off-delay" => "a light that stays on 10 seconds after you step off a pressure plate".into(),
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
        "flying-machine" => return "Press play under the view: it puts the start block in place and the machine flies off; reset to see it again.".to_string(),
        "tnt-duper" => {
            // (the try-it matches the card: a launcher throws its TNT, critique)
            return if c.axis.contains("launched") {
                "It runs by itself: watch it throw a lit TNT away each cycle while its TNT block stays in place.".to_string()
            } else {
                "It runs by itself: watch a lit TNT come out of the machine each cycle while its TNT block stays in place.".to_string()
            };
        }
        "shulker-loader" | "shulker-unloader" if !c.unmodeled.is_empty() => return "The view shows it as built, with our test's chests; download it to try it in the game.".to_string(),
        "shulker-loader" | "shulker-unloader" => {
            let lever = c.inputs.iter().zip(c.devices.iter()).any(|(n, d)| d == "lever" && !(n == "mode" || n.starts_with("mode.")));
            return format!(
                "{}Watch the items and boxes move between the chests (the numbers over each container show what it holds); the speed button goes up to 100×.",
                if lever { "Flip its start lever (the lime IN button under the view). " } else { "It starts by itself. " }
            );
        }
        "bud" => return "The view replays our test: a block is placed, removed and changed at the cell marked TARGET; the box marked OUT turns red when it fires.".to_string(),
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
    // the viewer draws the output as a box tagged OUT that turns red while powered (webrender
    // 1742c0308)
    let watch = if c.outputs.is_empty() { "watch the machine move" } else { "the box marked OUT turns red while the output is on" };
    let hint = match b {
        "piston-extender" => format!("turn it on and watch the block go {} blocks out; turn it off and it comes back", number(&c.kind)),
        "burst" => format!("each tap turns OUT red exactly {} times", number(&c.kind)),
        "divider" | "counter" if number(&c.kind) > 16 => format!("the demo counts the {} pulses for you: press the speed button until it shows 100× and watch OUT change", number(&c.kind)),
        "divider" | "counter" => format!("tap it {} times and OUT goes through one on-off cycle", number(&c.kind)),
        "monostable" if c.kind.contains("non-retriggerable") => "a tap keeps OUT red for a while; tapping again while it is red changes nothing".to_string(),
        "monostable" => "a tap keeps OUT red for a while; tap again while it is red and it stays red longer".to_string(),
        "off-delay" => "turn it on, then off: OUT stays red a while after".to_string(),
        "on-delay" => "a quick tap does nothing; hold it and OUT turns red".to_string(),
        "pulse-train" => "turn it on and OUT keeps flashing red until you turn it off".to_string(),
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
    let mut out: Vec<String> = Vec::new();
    if b.kind == "door" && b.group == "sculk" {
        out.push("Uses a sculk sensor: footsteps or other noises nearby can open or close it. Keep walkways away from it, or shield it with wool.".to_string());
    }
    // a sculk clock: sounds nearby can add pulses (critique fd51ca0d2: sculk builds "keep noise
    // away"; the clocks' test runs in silence)
    if b.clock.is_some() && b.kinds.iter().any(|k| k.contains("sculk_sensor")) {
        out.push("Uses a sculk sensor: other sounds nearby (footsteps, another machine) can add pulses, so keep noise away from it.".to_string());
    }
    // a vibration-proof sculk circuit (clk-obs's vibration test; the sensitive ones are hidden)
    if let Some(c) = &b.circuit {
        let proof = c.metrics.iter().any(|(k, v)| k == "vibration-proof" && v.starts_with("yes"));
        if proof && b.kinds.iter().any(|k| k.contains("sculk_sensor")) {
            out.push(format!(
                "Uses a sculk sensor tuned so footsteps and other noises don't trigger it{}.",
                if b.kinds.iter().any(|k| k == "chest") { ": the items in its chest set the one sound it listens to" } else { "" }
            ));
        }
    }
    // what the world around it does to it (vjeux 19:02Z: these builds are published, with their
    // limitation said plainly instead of hidden)
    if b.kinds.iter().any(|k| k.starts_with("tripwire")) {
        out.push("Uses a tripwire: anything that crosses the string (a mob, a player, a dropped item) adds a pulse, so enclose it.".to_string());
    }
    if b.kinds.iter().any(|k| k.starts_with("daylight_detector")) {
        out.push("Uses a daylight detector: it needs open sky and runs only in daytime; it stops at night and in rain.".to_string());
    }
    // a block change detector that misses pure block updates says so (the coordinator 21:55Z)
    if let Some(c) = b.circuit.as_ref().filter(|c| c.kind.starts_with("bud")) {
        let detects = c.metrics.iter().find(|(k, _)| k == "detects").map(|(_, v)| v.clone()).unwrap_or_default();
        if !detects.split_whitespace().any(|w| w == "update") {
            out.push("It does not notice pure block updates: a neighbour changing while the watched block itself stays the same gives no pulse.".to_string());
        }
    }
    let robustness = match (&b.circuit, &b.clock) {
        (Some(c), _) => c.robustness.as_str(),
        (None, Some(c)) => c.robustness.as_str(),
        _ => "",
    };
    let directional = robustness.starts_with("rotation") || (b.kind == "door" && b.door.is_some() && !b.flags.iter().any(|f| f == "rotation-safe"));
    if directional {
        out.push("Works only facing the way it is shown here: build it from the download without turning it; turned another way it stops or changes its timing (our test tried all 4 directions).".to_string());
    } else if robustness.contains("position") || robustness.contains("offset") {
        out.push("Depends on where it is built: at another spot in the world its timing changed in our test, so try it where you build it.".to_string());
    }
    // what a chunk reload does to a clock (clockinfra's `reload`: never a reason to hide)
    if let Some(w) = b.clock.as_ref().and_then(|c| reload_words(&c.reload)) {
        out.push(w);
    }
    if out.is_empty() { None } else { Some(out.join(" ")) }
}

/// What a build needs to work where you build it, for its opening lines (critique, real-world
/// lens: a TNT duper that must stand 8 blocks up said so only deep in its notes): a TNT duper's
/// height above the ground (`safe drop`), the items its containers start with (the design's
/// `nbt:` lines), water, open sky.
pub fn needs(b: &Build, design: &str) -> Vec<String> {
    let mut v = Vec::new();
    if let Some(c) = &b.circuit {
        if let Some((_, drop)) = c.metrics.iter().find(|(k, _)| k == "safe drop") {
            match drop.split_whitespace().next().and_then(|n| n.parse::<u32>().ok()) {
                Some(0) => v.push("Works standing on the ground.".to_string()),
                // (true for launchers and droppers alike: critique, "lands under it" contradicted a
                // launcher's caption, its TNT pile up 10 to 20 blocks away and get thrown back)
                Some(n) => v.push(format!("Must stand at least {n} blocks above the ground: lower, its own TNT explodes close enough to destroy it.")),
                None => {}
            }
        }
    }
    // the items its containers start with, summed per item
    let mut items: Vec<(String, u64)> = Vec::new();
    for line in design.lines().filter_map(|l| l.trim_start().strip_prefix("nbt:")) {
        for part in line.split("id:\"").skip(1) {
            let id = part.split('"').next().unwrap_or("").trim_start_matches("minecraft:").replace('_', " ");
            let count = part.split("count:").nth(1).and_then(|c| c.split(|ch: char| !ch.is_ascii_digit()).next()).and_then(|c| c.parse::<u64>().ok()).unwrap_or(1);
            match items.iter_mut().find(|(i, _)| *i == id) {
                Some(e) => e.1 += count,
                None => items.push((id, count)),
            }
        }
    }
    if !items.is_empty() {
        let list: Vec<String> = items.iter().map(|(i, n)| format!("{} {i}", thousands(*n))).collect();
        let one = items.len() == 1 && items[0].1 == 1;
        v.push(format!("Comes loaded with {} in its containers: the download includes {}; building it by hand, put {} in first.", list.join(" and "), if one { "it" } else { "them" }, if one { "it" } else { "them" }));
    }
    if b.kinds.iter().any(|k| k == "water") {
        v.push("Needs water where the view shows it.".to_string());
    }
    // (open sky for a daylight detector: said in its warning line)
    v
}

/// `1111` -> `1,111`.
fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// What a chunk reload does to a clock, in plain words (clockinfra's `reload`; the coordinator:
/// "stops for good if its chunk unloads mid-cycle (you walk away and come back)"); None when it
/// keeps going or was not checked.
pub fn reload_words(reload: &str) -> Option<String> {
    let r = reload.trim();
    let (what, some) = match r.split_once(" in ") {
        Some((w, k)) if k.ends_with(" builds") => (w.trim(), Some(k.trim_end_matches(" builds").to_string())),
        _ => (r, None),
    };
    let some = some.map(|k| format!(" (in {k} of our test builds)")).unwrap_or_default();
    let walk = "its chunk unloads mid-cycle (you walk away and come back)";
    Some(if what == "stops" {
        format!("Stops for good if {walk}{some}: start it again the way you started it after building it.")
    } else if what == "glitch then resumes" {
        format!("If {walk}, it skips a beat, then keeps going{some}.")
    } else if let Some(p) = what.strip_prefix("changes period to ") {
        let gt = p.trim_end_matches(" gt").trim();
        format!("If {walk}, its period changes to {}{some}.", ticks(&format!("{gt} gt")))
    } else if let Some(why) = what.strip_prefix("fails: ") {
        format!("A chunk reload breaks it{some}: {why}.")
    } else {
        return None;
    })
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
        // (honest about the sensor: critique, "a hidden entrance for a base" right above "footsteps
        // nearby can open it")
        // (the sensor's warning is the line right under it: said once, critique)
        "sculk" => "among the smallest 3×3 doors, for a vault or a quiet corner.".to_string(),
        "gravity" => "a 3×3 entrance whose door is made of sand or gravel that falls back into place.".to_string(),
        "entity" => "a 3×3 entrance that uses an entity (a minecart or a falling block) to work.".to_string(),
        _ => format!("a 3×3 entrance for a base, opened by one lever{}.", if seamless { ", nothing of the machine showing when closed" } else { "" }),
    };
    (what, first_cap(&why), "Flip the lever (tap it in the 3D view) and watch the door open; flip it again to close it.".to_string())
}

/// A clock's plain lines: (what it does, why you would build it, how to try it).
pub fn clock(b: &Build) -> (String, String, String) {
    let c = b.clock.as_ref();
    let period = c.map(|c| ticks(&format!("{} gt", c.every()))).unwrap_or_default();
    // an output that pulses several times in one cycle of its parts says both
    let cycle = c.filter(|c| c.pulse_every.is_some()).map(|c| format!(" (its parts repeat every {})", ticks(&format!("{} gt", c.period)))).unwrap_or_default();
    let kicked = c.is_some_and(|c| !(c.start.is_empty() || c.start.starts_with("self")));
    let start = if kicked { " Built in the game it needs one kick to start (the last block placed); then it keeps going." } else { " It starts on its own once built." };
    (
        // (not "forever" when something stops it: critique, the daylight clock stops at night)
        format!("Its output pulses by itself every {period}{cycle}{}.{start}", if b.kinds.iter().any(|k| k.starts_with("daylight_detector")) { " while the sun is up" } else { ", forever" }),
        if b.kinds.iter().any(|k| k.starts_with("daylight_detector")) { "A beat that runs only while the sun is up: lamps or machines that should work by day and rest at night." } else { "A steady beat for farms, item sorters, flashing lamps and anything that should run by itself." }.to_string(),
        if kicked { "The view starts it for you, then it runs by itself: pause and step one tick at a time with the buttons under it." } else { "It runs by itself: the view plays it; pause and step one tick at a time with the buttons under it." }.to_string(),
    )
}

/// The "Try it" box under the 3D view: how to try it, and how this design does it (its plain
/// mechanism); what it does and why build it are in the heading, on the first screen.
pub fn intro_box(b: &Build, has_viewer: bool) -> String {
    let try_line = match (&b.circuit, b.kind.as_str()) {
        (Some(c), _) if b.group == "Sand elevator" && has_viewer => format!(
            "Flip the lever marked IN (or tap IN under the view): the sand rises {} blocks; flip it back and it falls into place.",
            number(&c.kind)
        ),
        // the item counters: what sets the count (critique: "nothing says HOW to load the items")
        (Some(c), _) if !c.layout_key.is_empty() && base(&c.kind) == "counter" => format!(
            "{} Its count is the number of items in its droppers: they pass the items one way per pulse, and the numbers over them show how many each holds. Load another number of items (stone here) to make it react on that count.",
            try_it(c, has_viewer)
        ),
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
        (Some(_), _) if b.group == "Sand elevator" => "A sand elevator or a hidden floor that rises and falls back with one lever.".to_string(),
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
    fn warnings_say_every_limitation() {
        // a tripwire clock that only works facing one way says both, in plain words
        let b = Build {
            kind: "clock".into(),
            kinds: vec!["tripwire".into(), "tripwire_hook".into(), "oak_boat".into()],
            clock: Some(site_model::ClockInfo { period: 40, robustness: "rotation-dependent".into(), ..Default::default() }),
            ..Default::default()
        };
        let w = warning(&b).unwrap_or_default();
        assert!(w.contains("Uses a tripwire") && w.contains("Works only facing the way it is shown here"), "{w}");
        let day = Build { kind: "clock".into(), kinds: vec!["daylight_detector".into()], clock: Some(site_model::ClockInfo { period: 20, robustness: "robust".into(), ..Default::default() }), ..Default::default() };
        assert!(warning(&day).unwrap_or_default().contains("stops at night and in rain"));
        assert!(warning(&Build { kind: "clock".into(), clock: Some(site_model::ClockInfo { period: 4, robustness: "robust".into(), ..Default::default() }), ..Default::default() }).is_none());
        // a chunk reload, in plain words (clockinfra's reload column)
        assert!(reload_words("stops").unwrap().starts_with("Stops for good if its chunk unloads mid-cycle"));
        assert_eq!(reload_words("glitch then resumes in 2 of 4 builds").unwrap(), "If its chunk unloads mid-cycle (you walk away and come back), it skips a beat, then keeps going (in 2 of 4 of our test builds).");
        assert!(reload_words("changes period to 12 gt").unwrap().contains("its period changes to 12 ticks (0.6 s)"));
        assert!(reload_words("ok").is_none() && reload_words("").is_none());
    }

    #[test]
    fn needs_say_what_the_build_needs() {
        // a TNT duper's safe height, and the items its containers start with
        let b = Build { circuit: Some(circuit("tnt-duper", &[("safe drop", "8 blocks")])), ..Default::default() };
        let design = "nbt: 2 0 3 {Items:[{Slot:0b,id:\"minecraft:cobblestone\",count:64},{Slot:1b,id:\"minecraft:cobblestone\",count:1047}]}";
        let v = needs(&b, design);
        assert_eq!(v[0], "Must stand at least 8 blocks above the ground: lower, its own TNT explodes close enough to destroy it.");
        assert!(v[1].starts_with("Comes loaded with 1,111 cobblestone in its containers"), "{}", v[1]);
        let ground = Build { circuit: Some(circuit("tnt-duper", &[("safe drop", "0 blocks (on the ground)")])), ..Default::default() };
        assert_eq!(needs(&ground, ""), vec!["Works standing on the ground.".to_string()]);
    }

    #[test]
    fn plain_sentences_carry_the_numbers() {
        let mono = circuit("monostable retriggerable", &[("L", "32 gt")]);
        assert_eq!(one_liner(&mono), "One press keeps the output on for 32 ticks (1.6 s); pressing again while it is on restarts the countdown.");
        assert!(why_build(&mono).starts_with("A light that stays on a few seconds after each press"), "{}", why_build(&mono));
        assert_eq!(ticks("1 gt"), "1 tick (0.05 s)");
        assert_eq!(ticks("4-8 gt"), "4 to 8 ticks");
        assert_eq!(ticks_short("28 gt (from the press)"), "28 ticks");
        let ext = circuit("piston-extender 3", &[("extend", "27 gt"), ("retract", "56 gt (from the press)")]);
        assert!(one_liner(&ext).contains("pushed 3 blocks out") && one_liner(&ext).contains("comes back in 56 ticks"), "{}", one_liner(&ext));
        let gate = circuit("gate and", &[("function", "and"), ("latency", "0 gt")]);
        assert!(one_liner(&gate).contains("both inputs") && one_liner(&gate).contains("instant"));
    }
}
