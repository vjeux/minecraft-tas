//! `mctas site dominated`: the circuits a builder has no reason to pick (vjeux 12:12Z on a
//! slime-free piston extender: "Why would anyone use this monstrosity? It's bigger and slower than
//! the other ones we have"). A design X is listed when another design Y of the same kind and main
//! value is smaller (blocks) and at least as fast on every speed metric of the study (strictly
//! faster on one), and X's only advantage over Y is a material restriction (it avoids a block kind
//! Y uses: slime, honey, torches, pistons...). A design with any other property Y lacks (one
//! button, flat, 1 wide, tileable, robust, a badge) is kept.

use site_model::{Build, Site};
use std::collections::BTreeSet;

/// The study's speed metrics: (metric, true when higher is better).
fn speed_metrics(kind: &str) -> &'static [(&'static str, bool)] {
    match kind.split_whitespace().next().unwrap_or("") {
        "piston-extender" => &[("extend", false), ("retract", false)],
        "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" => &[("delay", false), ("min-pulse", false)],
        "tff" => &[("latency", false), ("min-period", false), ("min-pulse", false)],
        "delay" => &[("min-pulse", false), ("min-gap", false)],
        "gate" | "rs-latch" | "d-latch" | "dff" | "dff-enable" | "tff-reset" | "shift" | "counter" => &[("latency", false)],
        "flying-machine" => &[("latency", false)],
        "shulker-loader" | "shulker-unloader" => &[("items/s", true), ("first box", false)],
        "bud" => &[("latency", false), ("re-arm", false)],
        "tnt-duper" => &[("rate", true), ("latency", false)],
        "tnt-duper-triggered" => &[("latency", false), ("gap", false)],
        "burst" => &[("latency", false), ("repeat", false)],
        "divider" => &[("latency", false), ("min-period", false)],
        "monostable" => &[("delay", false), ("re-arm", false)],
        "off-delay" => &[("latency", false), ("min-pulse", false)],
        "on-delay" => &[("latency", false)],
        "pulse-train" => &[("latency", false), ("stop", false)],
        _ => &[("latency", false)],
    }
}

/// A metric's leading number (`27 gt` -> 27, `2.50` -> 2.5, `4-6 gt` -> 6: the worst).
fn number(b: &Build, k: &str) -> Option<f64> {
    let v = b.circuit.as_ref()?.metrics.iter().find(|(n, _)| n == k)?.1.clone();
    let first = v.split_whitespace().next()?;
    let worst = first.rsplit('-').next().unwrap_or(first);
    worst.parse().ok()
}

/// Marks `faster` the shown designs that are bigger than another of the same study, kind and value
/// but strictly faster than every smaller one on the study's speed metrics (the coordinator: keep
/// pext-n3-flat-cascade19, 27/27 gt, next to the 18-block record's 29/35, and say it is faster).
pub fn mark_faster(builds: &mut [Build]) -> usize {
    let shown = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.host.is_none() && c.layout_of.is_none() && !c.card_member && (c.tunable.is_empty() || !c.span.is_empty()));
    let key = |b: &Build| b.circuit.as_ref().map(|c| (c.study.clone(), c.kind.clone(), c.axis.clone())).unwrap_or_default();
    let mut marked = Vec::new();
    for (i, x) in builds.iter().enumerate() {
        if !shown(x) {
            continue;
        }
        let Some(c) = &x.circuit else { continue };
        let metrics = speed_metrics(&c.kind);
        let smaller: Vec<&Build> = builds.iter().filter(|y| shown(y) && key(y) == key(x) && y.blocks < x.blocks).collect();
        if smaller.is_empty() {
            continue;
        }
        // strictly better than each smaller design on one speed metric, never worse on another
        let beats = |y: &Build| {
            let mut better = false;
            for (m, higher) in metrics {
                match (number(x, m), number(y, m)) {
                    (Some(a), Some(b)) => {
                        let (a, b) = if *higher { (-a, -b) } else { (a, b) };
                        if a > b {
                            return false;
                        }
                        if a < b {
                            better = true;
                        }
                    }
                    (None, Some(_)) => return false,
                    _ => {}
                }
            }
            better
        };
        if smaller.iter().all(|y| beats(y)) {
            marked.push(i);
        }
    }
    for &i in &marked {
        builds[i].flags.push("faster".into());
    }
    marked.len()
}

/// Flags that only restrict materials (avoiding a material is not a builder's property here).
const MATERIAL_FLAGS: [&str; 4] = ["slime-free", "honey-free", "torch-free", "piston-free"];

/// The material classes a design can avoid (vjeux's "slime-free, honey-free, torch-free,
/// piston-free..."): a design avoids a class when it uses none of its blocks.
const MATERIALS: [(&str, &[&str]); 5] = [
    ("slime", &["slime_block"]),
    ("honey", &["honey_block"]),
    ("torches", &["redstone_torch", "redstone_wall_torch"]),
    ("pistons", &["piston", "sticky_piston"]),
    ("copper bulbs", &["copper_bulb", "exposed_copper_bulb", "weathered_copper_bulb", "oxidized_copper_bulb", "waxed_copper_bulb", "waxed_exposed_copper_bulb", "waxed_weathered_copper_bulb", "waxed_oxidized_copper_bulb"]),
];

/// What a builder gets from X beyond its materials: its chips, every badge, its flags (an
/// extendable family, one button, 1 wide...) and its categorical metrics (a counter's output, a
/// flip-flop's edge, a burst's re-trigger class), the bookkeeping and the material ones aside.
fn properties(b: &Build) -> BTreeSet<String> {
    let mut s: BTreeSet<String> = site_model::html::props(b).chips.into_iter().filter(|c| !c.starts_with("tunable") && !c.starts_with('+') && !matches!(c.as_str(), "variant" | "layout")).collect();
    s.extend(b.flags.iter().filter(|f| !matches!(f.as_str(), "variant" | "layout" | "tunable")).cloned());
    if let Some(c) = &b.circuit {
        // the base design of an extendable family grows by one unit per n: a builder's property
        if c.extends {
            s.insert("extendable".into());
        }
        // what a machine carries or pulls (a flying machine's cargo) is its own property
        if !c.feature.is_empty() {
            s.insert(format!("feature={}", c.feature));
        }
        for (k, v) in &c.metrics {
            match k.as_str() {
                "badges" => s.extend(v.split('+').filter(|x| !x.is_empty()).map(String::from)),
                "output" | "edge" | "enable" | "class" | "retrigger" | "function" | "two-way" => {
                    s.insert(format!("{k}={}", v.split_whitespace().next().unwrap_or("")));
                }
                _ => {}
            }
        }
    }
    s.retain(|x| !MATERIAL_FLAGS.contains(&x.as_str()));
    s
}

/// The material classes Y uses and X avoids entirely.
fn avoided(x: &Build, y: &Build) -> Vec<String> {
    let uses = |b: &Build, blocks: &[&str]| b.kinds.iter().any(|k| blocks.contains(&k.as_str()));
    MATERIALS.iter().filter(|(_, blocks)| uses(y, blocks) && !uses(x, blocks)).map(|(name, _)| name.to_string()).collect()
}

/// One dominated design.
pub struct Dominated<'a> {
    pub x: &'a Build,
    pub y: &'a Build,
    /// (metric, X's value, Y's value) where Y is better.
    pub slower: Vec<(String, f64, f64)>,
    /// Block kinds Y uses that X avoids.
    pub avoids: Vec<String>,
}

/// What [`dominated`] looks for.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// Bigger and strictly slower, only a material restriction in its favour (the rule).
    SlowerMaterial,
    /// Bigger, the same speed, only a material restriction in its favour.
    SameSpeedMaterial,
    /// Bigger and no faster, with nothing in its favour, not even a material (vjeux's own words:
    /// "bigger and slower than the other ones we have"; the coordinator 19:05Z: a slime-free
    /// extender beaten by a smaller slime-free one at the same reach).
    Plain,
}

/// Every circuit X beaten by a smaller, at-least-as-fast Y of the same kind and main value, with
/// at most its materials in its favour, as `pattern` says.
pub fn dominated(site: &Site, pattern: Pattern) -> Vec<Dominated<'_>> {
    let mut out = Vec::new();
    let circuits: Vec<&Build> = site.builds.iter().filter(|b| b.circuit.as_ref().is_some_and(|c| c.layout_of.is_none())).collect();
    for x in &circuits {
        let cx = x.circuit.as_ref().unwrap();
        // (a design already folded under another is off the lists)
        if cx.host.is_some() {
            continue;
        }
        let mut best: Option<Dominated> = None;
        for y in &circuits {
            let cy = y.circuit.as_ref().unwrap();
            if y.id == x.id || cy.study != cx.study || cy.kind != cx.kind || cy.axis != cx.axis || cy.direction != cx.direction || y.blocks >= x.blocks {
                continue;
            }
            // only within its own group and with the same kind of input (a sand elevator never
            // beats a general extender; a lever basic never beats a gate taking a real signal: the
            // V18 rule)
            let device = |b: &Build| ["lever", "button"].into_iter().find(|f| b.flags.iter().any(|x| x == f));
            if y.group != x.group || device(x) != device(y) {
                continue;
            }
            // at least as fast everywhere
            let mut slower = Vec::new();
            let mut better_somewhere = false;
            for (k, higher) in speed_metrics(&cx.kind) {
                let (Some(a), Some(b)) = (number(x, k), number(y, k)) else { continue };
                if if *higher { b > a } else { b < a } {
                    slower.push((k.to_string(), a, b));
                }
                if if *higher { a > b } else { a < b } {
                    better_somewhere = true;
                }
            }
            if better_somewhere {
                continue;
            }
            // nothing but (maybe) its materials in X's favour
            if !properties(x).is_subset(&properties(y)) {
                continue;
            }
            let avoids = avoided(x, y);
            let fits = match pattern {
                Pattern::SlowerMaterial => !slower.is_empty() && !avoids.is_empty(),
                Pattern::SameSpeedMaterial => slower.is_empty() && !avoids.is_empty(),
                Pattern::Plain => avoids.is_empty(),
            };
            if !fits {
                continue;
            }
            let cand = Dominated { x, y, slower, avoids };
            if best.as_ref().is_none_or(|b| (cand.y.blocks, &cand.y.name) < (b.y.blocks, &b.y.name)) {
                best = Some(cand);
            }
        }
        if let Some(d) = best {
            out.push(d);
        }
    }
    out.sort_by(|a, b| (&a.x.circuit.as_ref().unwrap().study, &a.x.name).cmp(&(&b.x.circuit.as_ref().unwrap().study, &b.x.name)));
    out
}

/// The build's notes: every dominated design still published, for whoever builds the site to
/// check and add to crates/site/hidden.tsv (the coordinator: re-run the audit whenever a study
/// re-archives; the build does it every time).
pub fn notes(site: &Site) -> Vec<String> {
    let mut out = Vec::new();
    for (pattern, what) in [(Pattern::SlowerMaterial, "bigger AND slower"), (Pattern::SameSpeedMaterial, "bigger, the same speed"), (Pattern::Plain, "bigger and no faster")] {
        for d in dominated(site, pattern) {
            out.push(format!(
                "dominated ({what}, {}): {}/{} ({} blocks) is beaten by {} ({} blocks){}: check it, then hide it in crates/site/hidden.tsv (mctas site dominated)",
                if d.avoids.is_empty() { "nothing in its favour".to_string() } else { format!("only avoids {}", d.avoids.join(", ")) },
                d.x.circuit.as_ref().map(|c| c.study.as_str()).unwrap_or(""),
                d.x.name,
                d.x.blocks,
                d.y.name,
                d.y.blocks,
                if d.slower.is_empty() { String::new() } else { format!(", {}", d.slower.iter().map(|(k, a, b)| format!("{k} {a} vs {b}")).collect::<Vec<_>>().join(", ")) }
            ));
        }
    }
    out
}

/// `mctas site dominated`: the lists, one line each.
pub fn report(site: &Site) -> String {
    let mut s = String::new();
    let line = |d: &Dominated| -> String {
        let slower = if d.slower.is_empty() {
            "same speed".to_string()
        } else {
            d.slower.iter().map(|(k, a, b)| format!("{k} {a} vs {b}")).collect::<Vec<_>>().join(", ")
        };
        format!(
            "{}\t{} ({} blocks)\tbeaten by {} ({} blocks)\t+{} blocks, {}\t{}\n",
            d.x.circuit.as_ref().map(|c| c.study.as_str()).unwrap_or(""),
            d.x.name,
            d.x.blocks,
            d.y.name,
            d.y.blocks,
            d.x.blocks - d.y.blocks,
            slower,
            if d.avoids.is_empty() { "nothing in its favour".to_string() } else { format!("only avoids {}", d.avoids.join(", ")) }
        )
    };
    for (pattern, title) in [
        (Pattern::SlowerMaterial, "bigger AND slower, only a material restriction in their favour"),
        (Pattern::SameSpeedMaterial, "bigger, the same speed (or no speed metric), only a material restriction in their favour"),
        (Pattern::Plain, "bigger and no faster, nothing in their favour (not even a material)"),
    ] {
        let list = dominated(site, pattern);
        s.push_str(&format!("# {title}: {}\n", list.len()));
        for d in &list {
            s.push_str(&line(d));
        }
    }
    s
}
