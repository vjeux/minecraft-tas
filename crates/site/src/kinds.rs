//! One world-best design per kind (vjeux 21:47Z: "I want to have ONE world best implementation of
//! each of these instead of a bunch of working but mid implementations"). A kind is what a builder
//! asks for (a rising-edge detector, an AND gate, a shulker loader); a parametric kind (a piston
//! extender's reach, a counter's N, a delay's length, a flying machine's direction and load) is
//! one card whose page has a table of every size, each size's record keeping its page. Every other
//! design is not published (no card, no page, no search hit); its file stays in the repository.
//! `mctas site kinds` prints the plan for the coordinator.

use site_model::Build;
use std::collections::{BTreeMap, BTreeSet};

/// The coordinator's picks where the rule is ambiguous (2026-10-20 21:55Z, 23:49Z, 00:01Z):
/// (study/kind, design).
const PICKS: [(&str, &str); 6] = [
    // vjeux asked for 1-wide tileable loaders; it keeps being shrunk
    ("shulker/shulker-loader", "shulker-loader-1wide"),
    // safe on the ground, built from catalog parts, 85.7 TNT a minute (clk-misc 62c785ba5; the
    // dupers that had to stand in the air left the archive)
    ("tnt/tnt-duper", "tnt-clocked-22b"),
    // the precise duper (the coordinator 00:14Z): every TNT explodes at one point in mid-air
    ("tnt/tnt-duper precise air burst", "tnt-airburst"),
    // the flying machine with docking stations and an on/off lever (clk-item, explore/tff
    // 68717ef8c9; the coordinator 00:45Z)
    ("fly/flying-machine two-way", "fly-docking-15b"),
    // a block change detector; its page says it does not see pure block updates
    ("bud/bud", "bud-observer"),
    // study 13 (clk-obs ff0e2f6208): the classic 1-wide tileable, overflow-safe hopper filter
    ("sorter/sorter", "sorter-classic"),
];

/// Per-size picks: (study/kind, size, design). Reach 2: the 1-wide flat one, so the whole reach
/// table is the 1-wide flat line.
const SIZE_PICKS: [(&str, i64, &str); 1] = [("pext/piston-extender", 2, "pext-n2-rider7-flat")];

/// The kinds whose designs differ by a number a builder picks (the reach, the count, the length):
/// one card per kind, a table of the archived sizes on its page. (Flying machines are not: one
/// design per engine shape, vjeux 00:33Z.)
pub fn parametric(kind: &str) -> bool {
    matches!(
        base(kind),
        "piston-extender" | "counter" | "divider" | "burst" | "delay" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "monostable" | "off-delay" | "on-delay" | "pulse-train" | "shift"
    )
}

fn base(kind: &str) -> &str {
    kind.split_whitespace().next().unwrap_or(kind)
}

/// A circuit's kind as a builder asks for it: a gate's function (a mirror pair is one kind), a
/// timer's restart rule, a two-way flying machine, a sand elevator; else the kind's first word.
pub fn kind_key(b: &Build) -> String {
    let Some(c) = &b.circuit else { return b.kind.clone() };
    let mut w = c.kind.split_whitespace();
    let k0 = w.next().unwrap_or("");
    match k0 {
        "gate" => format!("gate {}", w.next().unwrap_or("").trim_end_matches("-ba")),
        "monostable" => format!("monostable {}", w.next().unwrap_or("")),
        "flying-machine" if c.axis.contains("two-way") || c.metrics.iter().any(|(k, _)| k == "two-way") => "flying-machine two-way".into(),
        // flying machines by engine shape (vjeux 00:31Z, the coordinator): a tall engine (stacked:
        // the one that carries extra blocks) and a flat one (side by side, 1 block tall); the
        // other one-way machines (up, down, trains) are not published
        "flying-machine" if c.feature.starts_with("carries") => "flying-machine tall".into(),
        "flying-machine" if c.feature.is_empty() && (c.direction.is_empty() || c.direction == "horizontal") => "flying-machine flat".into(),
        "flying-machine" => format!("-flying-machine {}", if c.direction.is_empty() { "other" } else { c.direction.as_str() }),
        // the precise dupers are kinds of their own (vjeux 00:14Z: "We still need another
        // variation for precise drop location"): the air burst (every TNT explodes at one point in
        // mid-air) and the precise drop (every TNT falls straight down to one point)
        "tnt-duper" if c.axis.contains("precise") => format!("tnt-duper {}", c.axis.split_once("TNT/min").map(|(_, v)| v.trim()).unwrap_or("precise")),
        "piston-extender" if b.group == "Sand elevator" => "sand-elevator".into(),
        _ => k0.to_string(),
    }
}

/// The row a design takes in its kind's table: its size, or a flying machine's direction or load.
/// (sort key, words).
pub fn row_of(b: &Build) -> Option<(i64, String)> {
    let c = b.circuit.as_ref()?;
    if c.kind.starts_with("flying-machine") {
        let (k, w) = if c.feature.starts_with("carries") {
            (3, "carries extra blocks")
        } else if c.feature.starts_with("pulls") {
            (4, "pulls trailers")
        } else {
            match c.direction.as_str() {
                "up" => (1, "flies up"),
                "down" => (2, "flies down"),
                _ => (0, "flies sideways"),
            }
        };
        return Some((k, w.to_string()));
    }
    c.value.map(|v| (v, v.to_string()))
}

/// Robust: the same behaviour at 4 positions and 4 rotations in the game.
fn robust(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.robustness == "robust" || c.robustness.is_empty())
}

/// The study's first speed metric (lower is better), for ties.
fn speed(b: &Build) -> f64 {
    let Some(c) = &b.circuit else { return f64::MAX };
    let num = |k: &str| c.metrics.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.split_whitespace().next()?.rsplit('-').next()?.parse::<f64>().ok());
    match base(&c.kind) {
        "shulker-loader" | "shulker-unloader" | "sorter" => -num("items/s").unwrap_or(0.0),
        "tnt-duper" => -num("rate").unwrap_or(0.0),
        "piston-extender" => num("extend").unwrap_or(f64::MAX),
        "flying-machine" => num("speed").unwrap_or(f64::MAX),
        _ => num("latency").or_else(|| num("delay")).unwrap_or(f64::MAX),
    }
}

/// Why the coordinator's pick is the one, from its own facts (a design the coordinator did not
/// pick, alone in its kind: no claim).
fn pick_reason(path: &str, b: &Build) -> String {
    if !PICKS.iter().any(|(p, n)| *p == path && *n == b.name) {
        return String::new();
    }
    let Some(c) = &b.circuit else { return String::new() };
    let mut why: Vec<&str> = Vec::new();
    // (standing on the ground is in a TNT duper's title already: not repeated)
    // (a sorter: what builders tile it for; "tiles side by side" is its chip)
    if c.kind.starts_with("sorter") {
        why.push("the classic one-block-wide slice");
    }
    if robust(b) {
        why.push("passed at every position and facing we tested");
    }
    if why.is_empty() { "the pick".to_string() } else { format!("the pick: {}", why.join(", ")) }
}

/// The candidates of one kind, best first: robust ones first, then fewest blocks, then the
/// fastest, then the name.
pub fn ranked<'a>(list: &[&'a Build]) -> Vec<&'a Build> {
    let mut v: Vec<&Build> = list.to_vec();
    // (a mirror pair's tie goes to the A-first version: "A AND NOT B", not "B AND NOT A")
    let mirror = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.kind.split_whitespace().nth(1).is_some_and(|f| f.ends_with("-ba")));
    v.sort_by(|a, b| (!robust(a), a.blocks, speed(a), mirror(a)).partial_cmp(&(!robust(b), b.blocks, speed(b), mirror(b))).unwrap_or(std::cmp::Ordering::Equal).then(a.name.cmp(&b.name)));
    v
}

/// A gate with its own levers (the basics): not a gate a builder wires in (the coordinator 21:55Z:
/// the lever basics are dropped).
fn basic(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.kind.starts_with("gate ")) && b.flags.iter().any(|f| f == "lever")
}

/// The kinds of a set of circuit builds: (study, kind) -> the candidates (no basics, no other
/// layouts).
fn kinds_of(builds: &[Build]) -> BTreeMap<(String, String), Vec<usize>> {
    let mut by: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
    for (i, b) in builds.iter().enumerate() {
        let Some(c) = &b.circuit else { continue };
        if c.layout_of.is_some() || basic(b) {
            continue;
        }
        // (a kind key starting with "-" is no published kind: its designs stay unpublished)
        let key = kind_key(b);
        if key.starts_with('-') {
            continue;
        }
        by.entry((c.study.clone(), key)).or_default().push(i);
    }
    by
}

/// What each kind keeps: a non-parametric kind its one design, a parametric kind the record of
/// each row of its table. (study, kind) -> the kept builds' indices, the table's order.
fn chosen(builds: &[Build]) -> BTreeMap<(String, String), Vec<usize>> {
    let mut out = BTreeMap::new();
    for ((study, kind), idx) in kinds_of(builds) {
        let list: Vec<&Build> = idx.iter().map(|&i| &builds[i]).collect();
        let at = |name: &str| idx.iter().copied().find(|&i| builds[i].name == name);
        let path = format!("{study}/{kind}");
        let keep: Vec<usize> = if parametric(&kind) {
            let mut rows: BTreeMap<i64, usize> = BTreeMap::new();
            for b in ranked(&list) {
                let i = idx.iter().copied().find(|&i| builds[i].id == b.id).unwrap();
                if let Some((k, _)) = row_of(b) {
                    rows.entry(k).or_insert(i);
                }
            }
            for (p, size, name) in SIZE_PICKS {
                if p == path
                    && let Some(i) = at(name)
                {
                    rows.insert(size, i);
                }
            }
            rows.into_values().collect()
        } else {
            let pick = PICKS.iter().find(|(p, _)| *p == path).and_then(|(_, name)| at(name));
            pick.or_else(|| ranked(&list).first().and_then(|b| idx.iter().copied().find(|&i| builds[i].id == b.id))).into_iter().collect()
        };
        out.insert((study, kind), keep);
    }
    out
}

/// Keeps one design per kind (a parametric kind: the record of each row, one card for them all,
/// a table on its pages; its extendable families' base designs too, as generators with a page and
/// no card) and drops every other circuit build; returns the notes.
pub fn select_circuits(builds: &mut Vec<Build>) -> Vec<String> {
    let chosen = chosen(builds);
    let all = kinds_of(builds);
    // a pick the archive does not hold falls back to the rule, loudly (nothing silently dropped)
    let mut missing: Vec<String> = Vec::new();
    for (path, name) in PICKS.iter().map(|(p, n)| (*p, *n)).chain(SIZE_PICKS.iter().map(|(p, _, n)| (*p, *n))) {
        let (study, kind) = path.split_once('/').unwrap_or((path, ""));
        let have = all.get(&(study.to_string(), kind.to_string())).is_some_and(|v| v.iter().any(|&i| builds[i].name == name));
        if !have {
            let used = chosen.get(&(study.to_string(), kind.to_string())).and_then(|v| v.first()).map(|&i| builds[i].name.clone()).unwrap_or_else(|| "nothing".into());
            missing.push(format!("pick {path}: {name} is not in the archive (yet); showing {used} by the rule"));
        }
    }
    // why each kept design is the one (critique: "cards don't say why they're the best"): the
    // coordinator's reason for a pick, else smallest (or best) of the designs tested for its kind,
    // or for its size
    let mut claims: BTreeMap<usize, String> = BTreeMap::new();
    for ((study, kind), keep) in &chosen {
        let cands = all.get(&(study.clone(), kind.clone())).cloned().unwrap_or_default();
        let tested = |n: usize| format!("of {n} tested");
        for &i in keep {
            let row = row_of(&builds[i]).map(|r| r.0);
            let pool: Vec<usize> = if parametric(kind) { cands.iter().copied().filter(|&j| row_of(&builds[j]).map(|r| r.0) == row).collect() } else { cands.clone() };
            let smallest = pool.iter().all(|&j| builds[j].blocks >= builds[i].blocks);
            let path = format!("{study}/{kind}");
            let reason = match (path.as_str(), builds[i].name.as_str()) {
                ("shulker/shulker-loader", "shulker-loader-1wide") => "the loader that tiles side by side".to_string(),
                // (one design left in the archive says nothing of the others tested before it:
                // critique, "the only one tested" on a duper picked over seven; a pick says why)
                _ if pool.len() == 1 => pick_reason(&path, &builds[i]),
                _ if parametric(kind) && smallest => format!("smallest for its size {}", tested(pool.len())),
                _ if parametric(kind) => format!("best for its size {}", tested(pool.len())),
                _ if smallest => format!("smallest {}", tested(pool.len())),
                _ => format!("best {}", tested(pool.len())),
            };
            claims.insert(i, reason);
        }
    }
    let mut kept: BTreeSet<usize> = chosen.values().flatten().copied().collect();
    // (no generators: the extendable families are gone, vjeux 00:33Z)
    let generators: BTreeSet<usize> = BTreeSet::new();
    kept.extend(generators.iter().copied());
    let before = builds.iter().filter(|b| b.circuit.is_some()).count();
    // the kinds' groups: the records of a parametric kind form one group, its smallest design the
    // card (flying machines: the sideways one first), one entry per group
    let mut group_of: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for ((study, kind), keep) in &chosen {
        if parametric(kind) && keep.len() > 1 {
            group_of.insert(format!("kind:{study}:{kind}"), keep.clone());
        }
    }
    let mut notes = vec![format!("one design per kind: {} kinds, {} designs kept of {before} circuit designs (the rest stay in the repository, unpublished)", chosen.len(), kept.len())];
    notes.extend(missing);
    for ((study, kind), keep) in &chosen {
        notes.push(format!("kind {study}/{kind}: {}", keep.iter().map(|&i| builds[i].name.clone()).collect::<Vec<_>>().join(", ")));
    }
    // the folds of the old lists no longer apply: every kept design is its kind's
    let groups: Vec<(String, Vec<usize>)> = group_of.into_iter().collect();
    for i in kept.iter().copied() {
        let b = &mut builds[i];
        b.flags.retain(|f| !matches!(f.as_str(), "variant" | "layout" | "faster" | "mirrored" | "tunable" | "kind-twin" | "census" | "twin" | "size" | "generator"));
        if generators.contains(&i) {
            b.flags.push("size".into());
            b.flags.push("generator".into());
        }
        if let Some(c) = b.circuit.as_mut() {
            c.host = None;
            c.layout_of = None;
            c.lead.clear();
            c.claim = claims.get(&i).cloned().unwrap_or_default();
            c.tunable.clear();
            c.span.clear();
            c.span_blocks.clear();
            c.card_member = false;
        }
    }
    // a parametric kind's card: its smallest design, unless that build already pictures another
    // kind's card (critique: the toggle switch and the counter showed one build, so did the
    // rising edge and the signal shortener): then its next smallest
    let id_of = |b: &Build| b.circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, b.name)).unwrap_or_default();
    let mut pictured: BTreeSet<String> = BTreeSet::new();
    for ((_, kind), keep) in &chosen {
        if !(parametric(kind) && keep.len() > 1) {
            for &i in keep {
                pictured.insert(id_of(&builds[i]));
                pictured.extend(builds[i].circuit.as_ref().map(|c| c.same_build.clone()).unwrap_or_default());
            }
        }
    }
    for (g, members) in &groups {
        let mut order: Vec<usize> = members.clone();
        order.sort_by_key(|&j| (builds[j].blocks, row_of(&builds[j]).map(|r| r.0).unwrap_or(i64::MAX), builds[j].name.clone()));
        let twin = |j: usize| pictured.contains(&id_of(&builds[j])) || builds[j].circuit.as_ref().is_some_and(|c| c.same_build.iter().any(|s| pictured.contains(s)));
        let rep = order.iter().copied().find(|&j| !twin(j)).unwrap_or(order[0]);
        pictured.insert(id_of(&builds[rep]));
        pictured.extend(builds[rep].circuit.as_ref().map(|c| c.same_build.clone()).unwrap_or_default());
        // the card speaks for the kind: the smallest design for each size (critique: "the best
        // of 42 designs tested" did not say best at what)
        let tested = g.strip_prefix("kind:").and_then(|k| k.split_once(':')).and_then(|(s, k)| all.get(&(s.to_string(), k.to_string()))).map(|v| v.len()).unwrap_or(0);
        let size_word = match builds[rep].circuit.as_ref().map(|c| c.kind.split_whitespace().next().unwrap_or("").to_string()).unwrap_or_default().as_str() {
            "piston-extender" => "reach",
            "counter" => "count",
            "divider" => "factor",
            "burst" => "number of pulses",
            "delay" => "delay",
            "on-delay" => "hold time",
            "pulse-train" => "beat",
            "flying-machine" => "direction and load",
            "shift" => "length",
            _ => "length",
        };
        if let Some(c) = builds[rep].circuit.as_mut() {
            c.claim = format!("the smallest for each {size_word}, of {tested} designs tested");
        }
        let values: BTreeSet<i64> = members.iter().filter_map(|&j| builds[j].circuit.as_ref().and_then(|c| c.value)).collect();
        let fly = builds[rep].circuit.as_ref().is_some_and(|c| c.kind.starts_with("flying-machine"));
        let axis = builds[rep].circuit.as_ref().map(|c| c.axis.clone()).unwrap_or_default();
        let (letter, unit) = match axis.split_once(" = ") {
            Some((p, v)) => (p.split_whitespace().last().unwrap_or("").to_string(), if v.split_whitespace().nth(1) == Some("gt") { " gt" } else { "" }),
            None => (String::new(), ""),
        };
        let span = match (values.first(), values.last()) {
            (Some(lo), Some(hi)) if values.len() > 1 && !fly => format!("{letter} {lo}-{hi}{unit}, {} values", values.len()).trim().to_string(),
            _ => format!("{} versions", members.len()),
        };
        let lo_b = members.iter().map(|&j| builds[j].blocks).min().unwrap_or(0);
        let hi_b = members.iter().map(|&j| builds[j].blocks).max().unwrap_or(0);
        for &j in members {
            let label = row_of(&builds[j]).map(|r| r.1).unwrap_or_default();
            let b = &mut builds[j];
            if let Some(c) = b.circuit.as_mut() {
                c.tunable = g.clone();
                c.group_title = crate::explain::kind_title(&c.kind);
                c.group_note.clear();
                c.group_differs = if fly { "the direction and the load".into() } else { "the size".into() };
                c.variant_label = label;
                if j == rep {
                    c.span = span.clone();
                    c.span_blocks = if hi_b > lo_b { format!("{lo_b} to {hi_b} blocks") } else { String::new() };
                }
            }
            if j == rep {
                b.flags.push("tunable".into());
            }
        }
    }
    // what a kept design says about the same build archived as another kind: only kinds kept
    let ids: BTreeMap<String, String> = kept.iter().map(|&i| (builds[i].circuit.as_ref().map(|c| format!("circuits/{}/{}", c.study, builds[i].name)).unwrap_or_default(), builds[i].group.clone())).collect();
    for i in kept.iter().copied() {
        let also: Vec<String> = builds[i].circuit.as_ref().map(|c| c.same_build.iter().filter_map(|s| ids.get(s).cloned()).filter(|g| *g != builds[i].group).collect()).unwrap_or_default();
        if let Some(c) = builds[i].circuit.as_mut() {
            c.also = also;
        }
    }
    let mut k = 0;
    builds.retain(|b| {
        let keep = b.circuit.is_none() || kept.contains(&k);
        k += 1;
        keep
    });
    notes
}

/// Doors (the coordinator 21:55Z): one card, the overall smallest solid door (the "solid" board's
/// leader); every board's record keeps its page (no card); every other door is not published.
pub fn select_doors(builds: &mut Vec<Build>) -> Vec<String> {
    let leads = |b: &Build| b.door.as_ref().map(|d| d.leads.clone()).unwrap_or_default();
    let before = builds.iter().filter(|b| b.door.is_some()).count();
    builds.retain(|b| b.door.is_none() || !leads(b).is_empty());
    // a record of "X, not rotation-safe" that the record of X (which works facing any way) beats
    // or ties on size is the best at nothing (critique: the one-way sculk door, 40 blocks, under
    // the any-way one, 37)
    let size = |b: &Build| (b.volume.unwrap_or(u32::MAX), b.blocks);
    let dominated: BTreeSet<String> = builds
        .iter()
        .filter(|b| {
            let l = leads(b);
            !l.is_empty()
                && l.iter().all(|x| {
                    x.strip_suffix(", not rotation-safe").is_some_and(|base| builds.iter().any(|o| o.id != b.id && leads(o).iter().any(|y| y == base) && size(o) <= size(b)))
                })
        })
        .map(|b| b.id.clone())
        .collect();
    builds.retain(|b| !dominated.contains(&b.id));
    // the one card: the best fully seamless door, the seamless board's record (vjeux 23:44Z: "Can
    // you show a seamless piston door. This is what people usually consider to be piston
    // doors"); without one, the smallest stone door
    let main = builds.iter().position(|b| leads(b).iter().any(|l| l == "seamless")).or_else(|| builds.iter().position(|b| leads(b).iter().any(|l| l == "solid")));
    // what the one card holds: how many records sit behind it
    let records = builds.iter().filter(|b| b.door.is_some()).count();
    let mut kept = 0;
    for (i, b) in builds.iter_mut().enumerate() {
        let Some(d) = b.door.as_mut() else { continue };
        d.host = None;
        d.variants.clear();
        d.records = if Some(i) == main { records } else { 0 };
        kept += 1;
        b.flags.retain(|f| !matches!(f.as_str(), "variant" | "twin" | "size"));
        if Some(i) != main {
            b.flags.push("size".into());
        }
    }
    vec![format!("one door card: {kept} board records kept of {before} doors ({} best at nothing dropped; the rest stay in the repository, unpublished)", dominated.len())]
}

/// Clocks (vjeux 21:47Z: "one card per period is the per-size table of a single clock kind
/// page"): per period the best clock (robust first, then fewest blocks) keeps its page, in the
/// table of the one clock card; the extendable families of the kept clocks stay; every other
/// clock is not published. The card: the smallest of them.
pub fn select_clocks(builds: &mut Vec<Build>, cards: &mut Vec<site_model::Card>, members: &mut Vec<site_model::Member>) -> Vec<String> {
    // every clock is a card under its period (vjeux 01:24Z: "show all of them with a card grouped
    // by timing"); a folded twin (the same mechanism with the same numbers: generate's
    // fold_twins) is not
    let before = builds.iter().filter(|b| b.clock.is_some()).count();
    builds.retain(|b| b.clock.is_none() || !b.flags.iter().any(|f| f == "twin"));
    let _ = cards;
    let mut periods: BTreeSet<u32> = builds.iter().filter_map(|b| b.clock.as_ref().map(|c| c.every())).collect();
    periods.extend(members.iter().filter(|m| m.axis.is_empty()).map(|m| m.period));
    let mut kept = 0;
    for b in builds.iter_mut() {
        let Some(c) = b.clock.as_mut() else { continue };
        kept += 1;
        c.host = None;
        // (an archived family member is an extended clock too: its card says so)
        let extended = c.instance && c.n.is_some();
        c.instance = false;
        // every clock is a card of its own (vjeux 01:24Z: "show all of them with a card grouped
        // by timing"): no clock stands for the others
        c.kind_range.clear();
        b.flags.retain(|f| !matches!(f.as_str(), "variant" | "twin" | "census" | "size" | "generator"));
        if extended && !b.flags.iter().any(|f| f == "extended") {
            b.flags.push("extended".into());
        }
    }
    vec![format!("clocks: {kept} of {before} (folded twins aside) and {} verified family members, every one a card under its period ({} periods)", members.iter().filter(|m| m.axis.is_empty()).count(), periods.len())]
}

/// A verified clock family member as a clock of its own (vjeux 01:24Z: "members get real pages
/// again, just not family pages"): its base's facts, its own period, blocks, box and design.
pub fn member_build(m: &site_model::Member, base: &Build) -> Build {
    let name = m.design.rsplit('/').next().unwrap_or("").trim_end_matches(".txt").to_string();
    let mut b = base.clone();
    b.id = format!("clocks/{name}");
    b.name = name.clone();
    b.page = format!("clocks/{name}.html");
    b.design = m.design.clone();
    b.blocks = m.blocks;
    b.dims = m.dims.clone();
    b.volume = m.volume;
    b.picture = m.picture.clone();
    b.trace = None;
    b.flags.retain(|f| !matches!(f.as_str(), "extendable" | "census" | "size" | "variant" | "twin"));
    b.flags.push("extended".into());
    // (the base's own words say how it works; the page says what the extension adds, from the
    // family's rule: critique, "A longer version of clk-item-dropring-1d (n = 15)" was all it said)
    b.caption = base.caption.clone();
    if m.status != site_model::STATUS_JUDGED {
        b.flags.push("simulated".into());
    }
    if let Some(c) = b.clock.as_mut() {
        c.period = m.period;
        // its own numbers, from its own simulation; none when it was not simulated (no claim:
        // never its base's, critique: an extended hopper clock of 148 ticks said "on for 18
        // ticks", its base's 18 of 20)
        match &m.run {
            Some(r) => {
                c.warmup = r.warmup;
                c.on = r.on;
                c.edges = r.edges;
                c.pulses = r.pulses.clone();
                c.pulse_every = r.pulse_every;
            }
            None => {
                c.on = 0;
                c.edges = 0;
                c.pulses.clear();
                c.pulse_every = None;
            }
        }
        // (what the base's chunk reload did is not this clock's: not checked)
        c.reload.clear();
        c.kind_range.clear();
        c.host = None;
        c.variants.clear();
        c.instance = false;
        c.family = Some(m.family.clone());
        c.n = Some(m.n);
        c.aliases.clear();
    }
    b
}

/// The plan, one line per kind: the chosen design (a parametric kind: its rows) and the other
/// candidates.
pub fn report(builds: &[Build]) -> String {
    let chosen = chosen(builds);
    let all = kinds_of(builds);
    let mut s = String::new();
    for ((study, kind), keep) in &chosen {
        let fact = |b: &Build| -> String { format!("{} {}b{}", b.name, b.blocks, if robust(b) { "" } else { " NOT-ROBUST" }) };
        let rows: Vec<String> = keep.iter().map(|&i| format!("{}={}", row_of(&builds[i]).map(|r| r.1).unwrap_or_default(), fact(&builds[i]))).collect();
        let dropped = all.get(&(study.clone(), kind.clone())).map(|v| v.len()).unwrap_or(0) - keep.len();
        if parametric(kind) {
            s.push_str(&format!("{study}\t{kind}\tTABLE {}\t({dropped} other designs unpublished)\n", rows.join(", ")));
        } else {
            s.push_str(&format!("{study}\t{kind}\tPICK {}\t({dropped} other designs unpublished)\n", keep.first().map(|&i| fact(&builds[i])).unwrap_or_default()));
        }
    }
    s
}
