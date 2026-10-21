//! One world-best design per kind (vjeux 21:47Z: "I want to have ONE world best implementation of
//! each of these instead of a bunch of working but mid implementations"). A kind is what a builder
//! asks for (a rising-edge detector, an AND gate, a shulker loader); a parametric kind (a piston
//! extender's reach, a counter's N, a delay's length, a flying machine's direction and load) is
//! one card whose page has a table of every size, each size's record keeping its page. Every other
//! design is not published (no card, no page, no search hit); its file stays in the repository.
//! `mctas site kinds` prints the plan for the coordinator.

use site_model::Build;
use std::collections::{BTreeMap, BTreeSet};

/// (Every kind's design is chosen by rule now, none by name: the coordinator 06:56Z, "pick the
/// smallest judged design of the kind automatically instead of a hard-coded id, so new records
/// don't need a site change". A size of a parametric kind still can be: SIZE_PICKS.)

/// What a kind's design must have where the ranking alone (robust, fewest blocks, fastest) would
/// pick against what builders ask for: (study/kind, what it must do, the test). The pick is the
/// best-ranked design that does it, so a new record needs no site change (the coordinator
/// 06:56Z: "pick the smallest judged design of the kind automatically instead of a hard-coded id").
type Rule = (&'static str, &'static str, fn(&Build) -> bool);
const RULES: [Rule; 7] = [
    // vjeux asked for 1-wide tileable shulker loaders (the unloaders that go with them likewise),
    // working with their chests kept full the way a player keeps them (the 3D view's endless run;
    // the judge gives a loader 3 boxes and a spare: webrender found the 28-block loader jams once
    // its box chest holds about 20)
    ("shulker/shulker-loader", "tiles side by side and keeps sending boxes out with its chests kept full", tiles_and_flows),
    ("shulker/shulker-unloader", "tiles side by side and keeps sending boxes out with its chests kept full", tiles_and_flows),
    // every TNT duper with an on/off switch (vjeux, via the coordinator 14:08Z: the lever
    // versions are the picks); the self-running one safe where it is meant to stand (vjeux 11:49Z:
    // its declared stand height, `stand: N`, the ground when it declares none); the designs without
    // a switch stay beside the picks ([`SWITCH_KINDS`]), the one that sits right on the ground too
    ("tnt/tnt-duper", "has an on/off lever and is safe at its own stand height", switch_and_safe),
    ("tnt/tnt-duper precise drop", "has an on/off lever", switchable),
    ("tnt/tnt-duper precise air burst", "has an on/off lever", switchable),
    // the flying machine with docking stations and an on/off lever (the coordinator 00:45Z)
    ("fly/flying-machine two-way", "has an on/off lever", has_lever),
    // a true BUD switch (vjeux 11:46Z: "a single observer is not a BUD switch", it misses pure
    // block updates, the defining case): it notices the pure update, and resets itself (a pulse is
    // what starts a farm or an alarm; the coordinator 11:47Z)
    ("bud/bud", "notices a pure block update and resets itself", true_bud),
];

/// One line a kind's card says beside its pick (the coordinator 11:47Z): (study/kind, the line).
const MENTIONS: [(&str, &str); 1] = [("bud/bud", "the simpler 2-block BUD switch (a sticky piston and a redstone block) latches until you reset it; for visible block changes a single observer is enough")];

/// A kind's mention ([`MENTIONS`]), for its page too.
pub fn mention(study: &str, kind: &str) -> Option<&'static str> {
    MENTIONS.iter().find(|(p, _)| *p == format!("{study}/{kind}")).map(|(_, m)| *m)
}

fn metric<'a>(b: &'a Build, k: &str) -> &'a str {
    b.circuit.as_ref().and_then(|c| c.metrics.iter().find(|(n, _)| n == k)).map(|(_, v)| v.as_str()).unwrap_or("")
}

fn tiles_side_by_side(b: &Build) -> bool {
    metric(b, "badges").split('+').any(|x| x == "tileable(1)")
}

/// Tiles side by side, and its 3D view's run (the judge's chests kept full without end, as a
/// player keeps them; run at build time) sends boxes out (no run: no evidence against it).
fn tiles_and_flows(b: &Build) -> bool {
    tiles_side_by_side(b) && b.circuit.as_ref().and_then(|c| c.view_run.as_ref()).is_none_or(|v| v.boxes > 0)
}

fn on_the_ground(b: &Build) -> bool {
    metric(b, "safe drop").split_whitespace().next() == Some("0")
}

/// The ground variant a kind keeps beside its pick when the pick stands higher (vjeux 11:49Z: the
/// 22-block clocked duper, "sits right on the ground"): the best design of the kind that works on
/// the ground.
const GROUND_VARIANTS: [&str; 1] = ["tnt/tnt-duper"];

/// The ground variants of the chosen picks ([`GROUND_VARIANTS`]): (variant, pick) pairs.
fn ground_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        let path = format!("{study}/{kind}");
        let Some(&pick) = keep.first() else { continue };
        if !GROUND_VARIANTS.contains(&path.as_str()) || stand_height(&builds[pick]) == 0 {
            continue;
        }
        let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| on_the_ground(b) && b.id != builds[pick].id).collect();
        if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
            out.push((v, pick));
        }
    }
    out
}

/// The height a design is meant to stand at (`stand: N`, the judge's `stands` metric), 0: on the
/// ground.
pub fn stand_height(b: &Build) -> u32 {
    metric(b, "stands").split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// Safe at its own stand height: the judge's safe drop no higher than where it is meant to stand.
fn safe_at_stand(b: &Build) -> bool {
    metric(b, "safe drop").split_whitespace().next().and_then(|n| n.parse::<u32>().ok()).is_some_and(|safe| safe <= stand_height(b))
}

/// Has an on/off switch: its axis says `switchable`, or the judge measured its switch (`switch
/// lever off: stops within 2 gt; ...`).
fn switchable(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.axis.contains("switchable")) || !metric(b, "switch").is_empty()
}

/// The self-running duper: an on/off switch (vjeux: on every TNT duper) and safe where it is
/// meant to stand.
fn switch_and_safe(b: &Build) -> bool {
    switchable(b) && safe_at_stand(b)
}

/// The kinds whose pick has an on/off switch, the best design without one kept beside it (the
/// coordinator 14:08Z: the no-lever dupers are variants, "no on/off switch, smaller").
const SWITCH_KINDS: [&str; 3] = ["tnt/tnt-duper", "tnt/tnt-duper precise drop", "tnt/tnt-duper precise air burst"];

/// The no-switch variants of the chosen picks ([`SWITCH_KINDS`]): (variant, pick) pairs.
fn switchless_variants(builds: &[Build], chosen: &BTreeMap<(String, String), Vec<usize>>) -> Vec<(usize, usize)> {
    let all = kinds_of(builds);
    let mut out = Vec::new();
    for ((study, kind), keep) in chosen {
        let Some(&pick) = keep.first() else { continue };
        if !SWITCH_KINDS.contains(&format!("{study}/{kind}").as_str()) || !switchable(&builds[pick]) {
            continue;
        }
        let list: Vec<&Build> = all.get(&(study.clone(), kind.clone())).into_iter().flatten().map(|&i| &builds[i]).filter(|b| !switchable(b)).collect();
        if let Some(v) = ranked(&list).first().and_then(|b| builds.iter().position(|o| o.id == b.id)) {
            out.push((v, pick));
        }
    }
    out
}

/// A true BUD switch: it notices the pure block update, and resets itself (not latched).
fn true_bud(b: &Build) -> bool {
    metric(b, "detects").split_whitespace().any(|w| w == "update") && !b.circuit.as_ref().is_some_and(|c| c.axis.contains("latched")) && !metric(b, "reset").contains("manual")
}

fn has_lever(b: &Build) -> bool {
    b.circuit.as_ref().is_some_and(|c| c.devices.iter().any(|d| d == "lever"))
}

/// The size a parametric kind's card pictures, where its smallest is not the one builders know:
/// (kind, row). The 3-dial combination lock (the coordinator 06:56Z).
const CARD_ROWS: [(&str, i64); 1] = [("lock", 3)];

/// Per-size picks: (study/kind, size, design). None now: reach 2 is the smallest box like every
/// reach (pext-n2-rider-flat, 6x2x1; vjeux via pext 11:55Z).
const SIZE_PICKS: [(&str, i64, &str); 0] = [];

/// The kinds whose designs differ by a number a builder picks (the reach, the count, the length):
/// one card per kind, a table of the archived sizes on its page. (Flying machines are not: one
/// design per engine shape, vjeux 00:33Z.)
pub fn parametric(kind: &str) -> bool {
    matches!(
        base(kind),
        "piston-extender" | "counter" | "divider" | "burst" | "delay" | "rising-edge" | "falling-edge" | "dual-edge" | "pulse-limiter" | "pulse-extender" | "monostable" | "off-delay" | "on-delay" | "pulse-train" | "shift" | "lock"
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
        // (an on/off switch is a property, not a kind: the coordinator 14:08Z, "precise drop,
        // switchable" is the precise drop with a lever)
        "tnt-duper" if c.axis.contains("precise") => {
            let v = c.axis.split_once("TNT/min").map(|(_, v)| v.trim()).unwrap_or("precise");
            format!("tnt-duper {}", v.replace("switchable", "").trim().trim_end_matches(',').trim())
        }
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

/// Why the one design of its kind (no other tested) is worth building, from its own facts (no
/// "the only one tested": critique, said of a duper picked over seven that had left the archive).
fn lone_reason(b: &Build) -> String {
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
    why.join(", ")
}

/// The candidates of one kind, best first: robust ones first, then the smallest ([`rank_size`]),
/// then the fastest, then the name.
pub fn ranked<'a>(list: &[&'a Build]) -> Vec<&'a Build> {
    let mut v: Vec<&Build> = list.to_vec();
    // (a mirror pair's tie goes to the A-first version: "A AND NOT B", not "B AND NOT A")
    let mirror = |b: &Build| b.circuit.as_ref().is_some_and(|c| c.kind.split_whitespace().nth(1).is_some_and(|f| f.ends_with("-ba")));
    v.sort_by(|a, b| (!robust(a), rank_size(a), speed(a), mirror(a)).partial_cmp(&(!robust(b), rank_size(b), speed(b), mirror(b))).unwrap_or(std::cmp::Ordering::Equal).then(a.name.cmp(&b.name)));
    v
}

/// What "smallest" means for a design's kind, as a sort key: fewest blocks, then the smallest box
/// (vjeux 13:11Z, "sort by blocks then volume"); the box first, then blocks, for the kinds vjeux
/// asked by volume ([`by_box`]), a TNT duper's box with the air it needs under it
/// (`html::effective_volume`: the coordinator 15:18Z).
fn rank_size(b: &Build) -> (u32, u32) {
    if by_box(b) {
        (site_model::html::effective_volume(b).unwrap_or(u32::MAX), b.blocks)
    } else {
        (b.blocks, b.volume.unwrap_or(u32::MAX))
    }
}

/// How a pick's claim says "smallest" ([`rank_size`]): the word, and what it counts.
fn small_words(b: &Build) -> (&'static str, &'static str) {
    let air = b.circuit.as_ref().map(site_model::html::tnt_air).unwrap_or(0);
    match b.circuit.as_ref().map(|c| c.kind.as_str()) {
        Some(k) if k.starts_with("tnt-duper") && air > 0 => ("smallest", " (counting the air it needs under it)"),
        _ if by_box(b) => ("smallest box", ""),
        _ => ("smallest", ""),
    }
}

/// A kind whose pick is its smallest box (vjeux's minimum-volume rule, for the kinds he asked it
/// for: piston extenders, via pext 11:55Z, "the smallest VOLUME per reach in any 3D shape", the
/// box being the game judge's, every cell over the whole run; TNT dupers and shulker loaders and
/// unloaders, via the coordinator 13:55Z); the sand elevator, whose sand falls through air it
/// does not own, and every other kind: blocks first.
fn by_box(b: &Build) -> bool {
    let Some(c) = &b.circuit else { return false };
    match base(&c.kind) {
        "piston-extender" => b.group != "Sand elevator",
        "tnt-duper" | "tnt-duper-triggered" | "shulker-loader" | "shulker-unloader" => true,
        _ => false,
    }
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
            // a rule narrows the candidates to what builders ask for (none qualifying: all of them)
            let pool: Vec<&Build> = match RULES.iter().find(|(p, ..)| *p == path) {
                Some((_, _, test)) if list.iter().any(|b| test(b)) => list.iter().copied().filter(|b| test(b)).collect(),
                _ => list.clone(),
            };
            ranked(&pool).first().and_then(|b| idx.iter().copied().find(|&i| builds[i].id == b.id)).into_iter().collect()
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
    for (path, name) in SIZE_PICKS.iter().map(|(p, _, n)| (*p, *n)) {
        let (study, kind) = path.split_once('/').unwrap_or((path, ""));
        let have = all.get(&(study.to_string(), kind.to_string())).is_some_and(|v| v.iter().any(|&i| builds[i].name == name));
        if !have {
            let used = chosen.get(&(study.to_string(), kind.to_string())).and_then(|v| v.first()).map(|&i| builds[i].name.clone()).unwrap_or_else(|| "nothing".into());
            missing.push(format!("pick {path}: {name} is not in the archive (yet); showing {used} by the rule"));
        }
    }
    // a rule nothing meets: the ranking's pick, loudly
    for (path, words, test) in RULES {
        let (study, kind) = path.split_once('/').unwrap_or((path, ""));
        if let Some(v) = all.get(&(study.to_string(), kind.to_string()))
            && !v.iter().any(|&i| test(&builds[i]))
        {
            let used = chosen.get(&(study.to_string(), kind.to_string())).and_then(|v| v.first()).map(|&i| builds[i].name.clone()).unwrap_or_else(|| "nothing".into());
            missing.push(format!("rule {path}: no design {words}; showing {used}, the best of the others"));
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
            // (the kinds ranked by their box first, [`by_box`]: "smallest" is their box's)
            // (what "smallest" counts for its kind: [`rank_size`], [`small_words`])
            let size = |j: usize| rank_size(&builds[j]);
            let smallest = pool.iter().all(|&j| size(j) >= size(i));
            let (small_word, counts) = small_words(&builds[i]);
            let path = format!("{study}/{kind}");
            let rule = RULES.iter().find(|(p, ..)| *p == path).filter(|(_, _, test)| test(&builds[i]));
            let reason = match (path.as_str(), builds[i].name.as_str()) {
                // (the best of those that do what builders ask, said so: "the smallest box of 13
                // tested that tiles side by side"; smallest of all, it is said plainly below)
                _ if rule.is_some() && !smallest => format!("the {small_word} {}{counts} that {}", tested(pool.len()), rule.map(|r| r.1).unwrap_or_default()),
                // (one design left in the archive says nothing of the others tested before it:
                // critique, "the only one tested" on a duper picked over seven; a pick says why)
                _ if pool.len() == 1 => lone_reason(&builds[i]),
                _ if parametric(kind) && smallest => format!("{small_word} for its size {}{counts}", tested(pool.len())),
                _ if parametric(kind) => format!("best for its size {}", tested(pool.len())),
                _ if smallest => format!("{small_word} {}{counts}", tested(pool.len())),
                _ => format!("best {}", tested(pool.len())),
            };
            // the kind's one-line mention beside its pick (the BUD: the latched classic, the
            // observer)
            let reason = match mention(study, kind) {
                Some(m) if !reason.is_empty() => format!("{reason}; {m}"),
                Some(m) => m.to_string(),
                None => reason,
            };
            claims.insert(i, reason);
        }
    }
    let mut kept: BTreeSet<usize> = chosen.values().flatten().copied().collect();
    // (no generators: the extendable families are gone, vjeux 00:33Z)
    let generators: BTreeSet<usize> = BTreeSet::new();
    kept.extend(generators.iter().copied());
    // a pick that stands higher keeps the kind's ground design beside it, a pick with an on/off
    // switch the kind's best design without one: a page, no card, linked from the pick's (vjeux
    // 11:49Z; the coordinator 14:08Z)
    let grounds = ground_variants(builds, &chosen);
    kept.extend(grounds.iter().map(|(v, _)| *v));
    let switchless = switchless_variants(builds, &chosen);
    kept.extend(switchless.iter().map(|(v, _)| *v));
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
            c.ground = None;
            c.no_switch = None;
        }
    }
    let ids: Vec<String> = builds.iter().map(|b| b.id.clone()).collect();
    for &(v, pick) in &grounds {
        let up = stand_height(&builds[pick]);
        builds[v].flags.push("size".into());
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = format!("sits right on the ground (the pick of its kind stands {up} blocks up)");
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.ground = Some(ids[v].clone());
        }
    }
    for &(v, pick) in &switchless {
        let fewer = builds[pick].blocks.saturating_sub(builds[v].blocks);
        let ground = grounds.iter().any(|&(g, _)| g == v);
        if !builds[v].flags.iter().any(|f| f == "size") {
            builds[v].flags.push("size".into());
        }
        if let Some(c) = builds[v].circuit.as_mut() {
            c.claim = if ground {
                // (one build both: the 22-block clocked duper beside the lever engine)
                c.claim.replacen("sits right on the ground", "sits right on the ground, with no on/off switch", 1)
            } else if fewer > 0 {
                format!("no on/off switch, {fewer} block{} smaller than the pick of its kind", if fewer == 1 { "" } else { "s" })
            } else {
                "no on/off switch".to_string()
            };
        }
        if let Some(c) = builds[pick].circuit.as_mut() {
            c.no_switch = Some(ids[v].clone());
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
        // (the size builders know first, where it is not the smallest: CARD_ROWS)
        let card_row = |j: usize| -> bool {
            let k = builds[j].circuit.as_ref().map(|c| base(&c.kind).to_string()).unwrap_or_default();
            CARD_ROWS.iter().any(|(kind, row)| *kind == k && row_of(&builds[j]).map(|r| r.0) == Some(*row))
        };
        order.sort_by_key(|&j| (!card_row(j), builds[j].blocks, row_of(&builds[j]).map(|r| r.0).unwrap_or(i64::MAX), builds[j].name.clone()));
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
            "lock" => "number of dials",
            _ => "length",
        };
        // (the smallest box for the kinds ranked by it: [`by_box`])
        let small = if by_box(&builds[rep]) { "the smallest box" } else { "the smallest" };
        if let Some(c) = builds[rep].circuit.as_mut() {
            c.claim = format!("{small} for each {size_word}, of {tested} designs tested");
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

/// Every published circuit design's file in the repository, one per line: each kind's pick, and
/// every row of a parametric kind's table (`site kinds --paths`: the list a sweep over the
/// published designs takes, e.g. the exact game traces of every pick).
pub fn paths(builds: &[Build], repo: &std::path::Path) -> String {
    let chosen = chosen(builds);
    let mut s = String::new();
    for ((study, kind), keep) in &chosen {
        let take: Vec<usize> = if parametric(kind) { keep.clone() } else { keep.first().copied().into_iter().collect() };
        for i in take {
            let b = &builds[i];
            let archived = repo.join(format!("designs/circuits/{study}/archive/{}.txt", b.name));
            if archived.exists() {
                s.push_str(&format!("{}\n", archived.strip_prefix(repo).unwrap_or(&archived).display()));
            } else {
                s.push_str(&format!("{}\n", b.design));
            }
        }
    }
    s
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
