//! REDSTONEPEDIA's data model and its query engine.
//!
//! `mctas site build` (crates/site) exports every build of the repository (the archived 3x3
//! piston doors, the clocks, and the circuit studies' designs) as one [`Site`] in
//! `site/data/site.json`; the browser UI (web/site: Rust compiled to WebAssembly) loads it and
//! filters it with [`Query`]. The query engine lives here, in plain Rust, so it is unit-tested
//! natively (`cargo test -p site-model`) and the browser only draws what it returns.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub mod html;

/// Bumped when a field changes meaning (the UI refuses a newer major).
pub const DATA_VERSION: u32 = 1;

/// Everything the site shows.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Site {
    pub version: u32,
    /// `YYYY-MM-DDTHH:MMZ`.
    pub generated: String,
    /// The repository commit the site was built from.
    pub commit: String,
    pub builds: Vec<Build>,
    /// Extendable clock families, one card each (families differing by one swapped block share
    /// a card, as in designs/clocks/CATALOG.md).
    #[serde(default)]
    pub cards: Vec<Card>,
    /// Every member n of every extendable family, generated at build time with the family's own
    /// `extend generator` (the clock time search expands families to these).
    #[serde(default)]
    pub members: Vec<Member>,
}

/// One build: an archived design with its properties.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Build {
    /// `doors/<stem>`, `clocks/<stem>`, `circuits/<kind>/<stem>`: unique, also the page path
    /// without `.html`.
    pub id: String,
    /// `door`, `clock` or `circuit`.
    pub kind: String,
    /// The main facet: a door's category (solid, gravity, sculk, entity), a clock's family (the
    /// block kinds that change state in a period: `observer+redstone_lamp`), a circuit's kind.
    pub group: String,
    pub name: String,
    /// Path of its page from the site root (`doors/fc2-40b.html`).
    pub page: String,
    /// Path of its design file from the site root (`designs/doors/fc2-40b.txt`).
    pub design: String,
    /// Blocks placed (a piston head is part of its piston; entities count one each).
    pub blocks: u32,
    /// Bounding-box volume (doors: the community's volume over a whole cycle; clocks: the box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    /// Width x height x depth (`7x9x1`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dims: Option<String>,
    /// Block kinds used (ids without `minecraft:`), sorted.
    #[serde(default)]
    pub kinds: Vec<String>,
    /// How it works (one paragraph).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub caption: String,
    /// Filterable yes/no properties ([`FLAGS`]).
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<Door>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<ClockInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub circuit: Option<Circuit>,
    /// A real-game recording for the viewer's replay (`designs/clocks/traces/<stem>.trace`:
    /// entities and fluids redsim does not model), path from the site root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<String>,
    /// An interim picture (absolute URL: a clock's real-game picture, a door's board GIF) shown
    /// until the 3D viewer renders the build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
}

/// A 3x3 piston door's facts (designs/piston-door-3x3/archive and its caches).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Door {
    /// Worst-cycle ticks after the lever flip until open / closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub open: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub close: Option<u32>,
    /// `safe`, `breaks at 90`, ... (`""`: unknown).
    #[serde(default)]
    pub rotation: String,
    /// The community regulation's seamless tier (`SUPER`, `FULL`, `SEMI`, `QUART`; `""` none).
    #[serde(default)]
    pub seamless: String,
    /// Frame cells holding the door block when open (`12/12`).
    #[serde(default)]
    pub frame: String,
    /// The boards it is on, and the ones it leads.
    #[serde(default)]
    pub boards: Vec<String>,
    #[serde(default)]
    pub leads: Vec<String>,
    /// Blocks of its row in the solid record progression (records.tsv), if it took a record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
    /// `door buildable`'s verdict (a survival build order), if a script exists.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buildable: Option<String>,
    /// The agent track it came from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    /// `YYYY-MM-DD HH:MMZ` it was archived.
    #[serde(default)]
    pub archived: String,
    #[serde(default)]
    pub entities: u32,
    /// Every cached property (static `door properties` + dynamic checks), for the properties
    /// table: (key, value).
    #[serde(default)]
    pub props: Vec<(String, String)>,
}

/// A clock's facts (designs/clocks/archive: verdicts-v1.tsv, extend.tsv, aliases.tsv).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct ClockInfo {
    /// Period in game ticks (20 gt = 1 s).
    pub period: u32,
    pub warmup: u32,
    /// Output on-ticks per period, rising edges per period, pulse widths.
    pub on: u32,
    pub edges: u32,
    #[serde(default)]
    pub pulses: String,
    /// `robust`, or how it depends on position / rotation.
    #[serde(default)]
    pub robustness: String,
    /// How it starts (`self-starting`, `kick: last 1,0,0`...).
    #[serde(default)]
    pub start: String,
    /// Its mirror image is a different type.
    #[serde(default)]
    pub chiral: bool,
    /// Other archived names of the same type.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Build id of the smaller clock of its period whose block kinds it only adds to (a variant).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Build ids of its variants.
    #[serde(default)]
    pub variants: Vec<String>,
    /// The extendable family it is the base design or a member of: card id, family id, n.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub n: Option<i64>,
    /// A beat clock (sub-clocks side by side, marked `# composite`).
    #[serde(default)]
    pub composite: bool,
    /// An archived design that is a member of an extendable family other than its base design
    /// (hidden with the variants by default: the family's card stands for it, as in the catalog).
    #[serde(default, skip_serializing_if = "is_false")]
    pub instance: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// A circuit's facts (designs/circuits: the contraption studies, docs/circuits.md): its kind, the
/// value of its study's axis (pulse length, delay, reach...) and the other metrics, as the
/// judge's checker reports them.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Circuit {
    /// The study (`pulse`, `tff`, `delay`, `gates`, `memory`, `pext`, `fly`).
    pub study: String,
    /// The kind as its `circuit:` line writes it (`rising-edge`, `gate xor`, `counter 4`).
    #[serde(default)]
    pub kind: String,
    /// The value of the study's axis (`L 4 gt`, `xor`, `N 3`), and its number when numeric.
    #[serde(default)]
    pub axis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<i64>,
    /// Every metric in report order: (name, value).
    #[serde(default)]
    pub metrics: Vec<(String, String)>,
    /// Where the metrics come from: the judge's verdict line, else the redsim screen's.
    #[serde(default)]
    pub verdict: String,
    /// Inputs and outputs (port names), the markers of the 3D view.
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
}

/// A period or block-count formula in n (the clock study's `extend period:`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "f")]
pub enum Formula {
    /// a + b·n
    #[serde(rename = "lin")]
    Linear { a: i64, b: i64 },
    /// a·k^n + c
    #[serde(rename = "geo")]
    Geometric { a: i64, k: i64, c: i64 },
}

impl Formula {
    pub fn eval(&self, n: i64) -> i64 {
        match *self {
            Formula::Linear { a, b } => a + b * n,
            Formula::Geometric { a, k, c } => a.saturating_mul(k.saturating_pow(n.max(0) as u32)).saturating_add(c),
        }
    }

    /// The n in `lo..=hi` whose value lies in `[min, max]` (the time search's "which member hits
    /// it"), in increasing n.
    pub fn solve(&self, lo: i64, hi: i64, min: i64, max: i64) -> Vec<i64> {
        (lo..=hi).filter(|&n| (min..=max).contains(&self.eval(n))).collect()
    }
}

/// One extendable card.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Card {
    /// Slug, also its page `families/<id>.html`.
    pub id: String,
    pub title: String,
    /// `P = 10 + 4n gt` (several families: the general formula).
    pub heading: String,
    /// The same in seconds (`0.5 s + 0.2 s·n`), when the card's families share one formula.
    #[serde(default)]
    pub seconds: String,
    pub page: String,
    /// The block kinds every family of the card uses, and the blocks that can be swapped in
    /// ("works with any one of these").
    #[serde(default)]
    pub core: Vec<String>,
    #[serde(default)]
    pub works_with: Vec<String>,
    #[serde(default)]
    pub note: String,
    /// The card's families, the pictured (simplest) one first.
    pub families: Vec<Family>,
}

/// One extendable family (a base design with `extend` headers).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Family {
    pub id: String,
    /// Build id of its base design.
    pub build: String,
    pub period: Formula,
    pub period_text: String,
    pub blocks: Formula,
    pub blocks_text: String,
    /// How to extend it.
    pub rule: String,
    /// The n range verified in the game, and the n of the base design.
    pub n_lo: i64,
    pub n_hi: i64,
    pub base: i64,
    /// The n judged individually in the game (archive/extend.tsv).
    #[serde(default)]
    pub judged: Vec<i64>,
    /// Block kinds beyond the card's core (the swapped relay).
    #[serde(default)]
    pub extra: Vec<String>,
}

/// One member of an extendable family, generated at build time.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Member {
    pub card: String,
    pub family: String,
    pub n: i64,
    /// The formula's period and blocks for this n.
    pub period: u32,
    pub blocks: u32,
    /// Path of its generated design from the site root.
    pub design: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dims: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<u32>,
    /// [`STATUS_JUDGED`] (judged in the real game: archive/extend.tsv), [`STATUS_REDSIM`] (its
    /// generated design simulated in redsim at build time: the formula's period, exactly) or
    /// [`STATUS_FORMULA`] (formula, not individually judged).
    pub status: String,
    /// The archived build of this exact member, if any (its base design, or an archived instance).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<String>,
    /// An interim picture (absolute URL), for the members the catalog pictured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
}

pub const STATUS_JUDGED: &str = "judged";
pub const STATUS_REDSIM: &str = "redsim";
pub const STATUS_FORMULA: &str = "formula";

/// A member's status in words.
pub fn status_label(s: &str) -> &'static str {
    match s {
        STATUS_JUDGED => "judged in the game",
        STATUS_REDSIM => "simulated (redsim), not judged in the game",
        _ => "formula, not individually judged",
    }
}

/// The flags, (key, label, kind it applies to), in display order.
pub const FLAGS: &[(&str, &str, &str)] = &[
    ("rotation-safe", "rotation-safe", "door"),
    ("slime-free", "no slime or honey", "door"),
    ("flat", "flat (one block thick)", "door"),
    ("wall", "fits a 3-thick wall", "door"),
    ("ground", "on the ground", "door"),
    ("seamless", "seamless (any tier)", "door"),
    ("seamless-full", "seamless FULL or better", "door"),
    ("hallway", "hallway-compatible", "door"),
    ("frame-full", "full frame when open", "door"),
    ("eligible", "record-eligible", "door"),
    ("record", "took a record", "door"),
    ("leader", "#1 on a board", "door"),
    ("video", "has a video", "door"),
    ("buildable", "buildable in survival", "door"),
    ("entities", "uses entities", "door"),
    ("robust", "robust (any position and rotation)", "clock"),
    ("self-starting", "self-starting", "clock"),
    ("kicked", "needs a start kick", "clock"),
    ("chiral", "chiral", "clock"),
    ("extendable", "extendable family", "clock"),
    ("variant", "variant of a smaller clock", "clock"),
    ("composite", "composite (beat)", "clock"),
    ("calibration", "judge calibration design", "circuit"),
    ("instant", "instant (0 gt latency)", "circuit"),
    ("traced", "real-game replay", "clock"),
];

pub fn flag_label(key: &str) -> &str {
    FLAGS.iter().find(|f| f.0 == key).map(|f| f.1).unwrap_or(key)
}

// ---- time ------------------------------------------------------------------------------------

/// Just the real time of `gt` game ticks: `1.5 s`, `1 min 55.2 s`, `2 h 5 min` (as the clock
/// catalog writes it).
pub fn real_time(gt: u32) -> String {
    let num = |x: f64| -> String {
        let s = format!("{x:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let secs = gt as f64 / 20.0;
    if secs < 60.0 {
        return format!("{} s", num(secs));
    }
    let mins = (secs / 60.0).floor();
    let rest = secs - mins * 60.0;
    if mins < 60.0 {
        return if rest > 0.0 { format!("{} min {} s", mins, num(rest)) } else { format!("{} min", mins) };
    }
    let hours = (mins / 60.0).floor();
    let m = mins - hours * 60.0;
    if m > 0.0 { format!("{} h {} min", hours, m) } else { format!("{} h", hours) }
}

/// `20 gt (1 s)`.
pub fn gt_time(gt: u32) -> String {
    format!("{gt} gt ({})", real_time(gt))
}

/// Parse a clock time search: a period in game ticks (`48`, `48 gt`, `48 ticks`), a time
/// (`2.5 s`, `1 min`, `1 min 30 s`, `1:30`, `2 h`), or a range of either (`40-60 gt`,
/// `40..60`, `2-3 s`, `2 s to 3 s`, `< 60 gt`, `>= 1 min`). Returns the inclusive range in gt
/// (an exact time is the one period it names; a time between two ticks matches nothing).
pub fn parse_time_query(q: &str) -> Result<(u32, u32), String> {
    let s = q.trim().to_lowercase().replace('–', "-").replace('—', "-").replace('≤', "<=").replace('≥', ">=");
    if s.is_empty() {
        return Err("empty".into());
    }
    for (op, f) in [("<=", 0), (">=", 1), ("<", 2), (">", 3)] {
        if let Some(rest) = s.strip_prefix(op) {
            let v = parse_amount(rest.trim(), None)?;
            return Ok(match f {
                0 => (0, floor_gt(v)),
                1 => (ceil_gt(v), u32::MAX),
                2 => (0, ceil_gt(v).saturating_sub(1)),
                _ => (floor_gt(v).saturating_add(1), u32::MAX),
            });
        }
    }
    // a range: "a-b", "a..b", "a to b" (a unit on the right end only applies to both)
    let split = ["..", " to ", "-"].iter().find_map(|sep| s.find(sep).filter(|&i| i > 0).map(|i| (i, sep.len())));
    if let Some((i, len)) = split {
        let (a, b) = (s[..i].trim(), s[i + len..].trim());
        let unit_b = unit_of(b);
        let hi = parse_amount(b, None)?;
        let lo = parse_amount(a, unit_b)?;
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let (l, h) = (ceil_gt(lo), floor_gt(hi));
        if l > h {
            return Err(format!("no whole game tick between {} and {}", a, b));
        }
        return Ok((l, h));
    }
    let v = parse_amount(&s, None)?;
    let (l, h) = (ceil_gt(v), floor_gt(v));
    if l > h {
        return Err(format!("{} is not a whole number of game ticks (1 gt = 0.05 s)", q.trim()));
    }
    Ok((l, h))
}

const EPS: f64 = 1e-6;

fn ceil_gt(v: f64) -> u32 {
    (v - EPS).ceil().max(0.0).min(u32::MAX as f64) as u32
}

fn floor_gt(v: f64) -> u32 {
    (v + EPS).floor().max(0.0).min(u32::MAX as f64) as u32
}

/// Game ticks per unit word.
fn unit_scale(u: &str) -> Option<f64> {
    Some(match u {
        "" | "gt" | "t" | "tick" | "ticks" | "game tick" | "game ticks" => 1.0,
        "rt" | "redstone tick" | "redstone ticks" => 2.0,
        "s" | "sec" | "secs" | "second" | "seconds" => 20.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 1200.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 72000.0,
        _ => return None,
    })
}

/// The unit word at the end of an amount (`"3 s"` -> `s`), if it names one.
fn unit_of(s: &str) -> Option<f64> {
    let t = s.trim();
    let i = t.rfind(|c: char| c.is_ascii_digit() || c == '.')?;
    let u = t[i + 1..].trim();
    if u.is_empty() { None } else { unit_scale(u) }
}

/// An amount in game ticks: `48`, `48gt`, `2.5 s`, `1 min 30 s`, `1:30` (m:ss); a bare number
/// takes `default_unit` (gt when none).
fn parse_amount(s: &str, default_unit: Option<f64>) -> Result<f64, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("missing number".into());
    }
    if let Some((m, sec)) = s.split_once(':') {
        let m: f64 = m.trim().parse().map_err(|_| format!("bad time {s:?}"))?;
        let sec: f64 = sec.trim().trim_end_matches('s').trim().parse().map_err(|_| format!("bad time {s:?}"))?;
        return Ok(m * 1200.0 + sec * 20.0);
    }
    // a sequence of "<number> <unit>" parts: "1 min 30 s"
    let mut total = 0.0;
    let mut rest = s;
    let mut parts = 0;
    while !rest.is_empty() {
        let num_end = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        if num_end == 0 {
            return Err(format!("bad time {s:?}"));
        }
        let n: f64 = rest[..num_end].parse().map_err(|_| format!("bad number in {s:?}"))?;
        rest = rest[num_end..].trim_start();
        let unit_end = rest.find(|c: char| c.is_ascii_digit()).unwrap_or(rest.len());
        let unit = rest[..unit_end].trim();
        let scale = if unit.is_empty() { default_unit.unwrap_or(1.0) } else { unit_scale(unit).ok_or_else(|| format!("unknown unit {unit:?}"))? };
        total += n * scale;
        rest = rest[unit_end..].trim_start();
        parts += 1;
    }
    if parts == 0 {
        return Err(format!("bad time {s:?}"));
    }
    Ok(total)
}

// ---- query -----------------------------------------------------------------------------------

/// An inclusive numeric range; `None` ends are open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Range {
    pub min: Option<i64>,
    pub max: Option<i64>,
}

impl Range {
    pub fn is_open(&self) -> bool {
        self.min.is_none() && self.max.is_none()
    }

    /// A value passes when it lies in the range; a missing value passes only an open range.
    pub fn admits(&self, v: Option<i64>) -> bool {
        match v {
            None => self.is_open(),
            Some(v) => self.min.is_none_or(|m| v >= m) && self.max.is_none_or(|m| v <= m),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// Fewest blocks, then volume, then name.
    #[default]
    Blocks,
    Volume,
    Period,
    Open,
    Close,
    Name,
}

impl Sort {
    pub const ALL: [(Sort, &'static str, &'static str); 6] = [
        (Sort::Blocks, "blocks", "fewest blocks"),
        (Sort::Volume, "volume", "smallest volume"),
        (Sort::Period, "period", "shortest period"),
        (Sort::Open, "open", "fastest open"),
        (Sort::Close, "close", "fastest close"),
        (Sort::Name, "name", "name"),
    ];

    pub fn key(self) -> &'static str {
        Sort::ALL.iter().find(|s| s.0 == self).map(|s| s.1).unwrap_or("blocks")
    }

    pub fn from_key(k: &str) -> Sort {
        Sort::ALL.iter().find(|s| s.1 == k).map(|s| s.0).unwrap_or_default()
    }
}

/// What the person asked for. Facets: `groups` (OR), block kinds and flags as include (all
/// required) / exclude (none allowed) sets, numeric ranges, a free-text search, and the clock
/// time search, which also expands extendable families to their members.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// `door`, `clock`, `circuit`; empty: every kind.
    pub kind: String,
    pub text: String,
    /// The clock time search, as typed ([`parse_time_query`]).
    pub time: String,
    pub groups: BTreeSet<String>,
    pub kinds_in: BTreeSet<String>,
    pub kinds_out: BTreeSet<String>,
    pub flags_in: BTreeSet<String>,
    pub flags_out: BTreeSet<String>,
    pub blocks: Range,
    pub volume: Range,
    pub period: Range,
    pub open: Range,
    pub close: Range,
    /// Show the variants (clocks folded under a smaller clock of their period).
    pub variants: bool,
    pub sort: Sort,
}

/// One result: a build, or a generated member of an extendable family (time search).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Hit {
    Build(usize),
    Member(usize),
}

/// Which facet to leave out when counting (the counts of a facet's values are computed with
/// every other filter applied, so picking one value never zeroes its siblings).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Skip {
    None,
    Kind,
    Groups,
    Kinds,
    Flags,
}

/// Facet counts for the current query.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facets {
    pub kinds: BTreeMap<String, usize>,
    pub groups: BTreeMap<String, usize>,
    pub block_kinds: BTreeMap<String, usize>,
    pub flags: BTreeMap<String, usize>,
}

/// The fields a hit is filtered on (a member borrows its family's base build for the rest).
struct View<'a> {
    kind: &'a str,
    group: &'a str,
    name: &'a str,
    blocks: i64,
    volume: Option<i64>,
    period: Option<i64>,
    open: Option<i64>,
    close: Option<i64>,
    kinds: &'a [String],
    flags: &'a [String],
    text: [&'a str; 3],
    variant: bool,
}

impl Site {
    /// Build index by id.
    pub fn index(&self) -> BTreeMap<&str, usize> {
        self.builds.iter().enumerate().map(|(i, b)| (b.id.as_str(), i)).collect()
    }

    fn base_of(&self, m: &Member, index: &BTreeMap<&str, usize>) -> Option<&Build> {
        let card = self.cards.iter().find(|c| c.id == m.card)?;
        let fam = card.families.iter().find(|f| f.id == m.family)?;
        index.get(fam.build.as_str()).map(|&i| &self.builds[i])
    }

    fn view<'a>(&'a self, h: Hit, index: &BTreeMap<&str, usize>) -> Option<View<'a>> {
        match h {
            Hit::Build(i) => {
                let b = self.builds.get(i)?;
                Some(View {
                    kind: &b.kind,
                    group: &b.group,
                    name: &b.name,
                    blocks: b.blocks as i64,
                    volume: b.volume.map(|v| v as i64),
                    period: b.clock.as_ref().map(|c| c.period as i64),
                    open: b.door.as_ref().and_then(|d| d.open).map(|v| v as i64),
                    close: b.door.as_ref().and_then(|d| d.close).map(|v| v as i64),
                    kinds: &b.kinds,
                    flags: &b.flags,
                    text: [&b.name, &b.caption, &b.group],
                    variant: b.clock.as_ref().is_some_and(|c| c.host.is_some() || c.instance),
                })
            }
            Hit::Member(i) => {
                let m = self.members.get(i)?;
                let b = self.base_of(m, index)?;
                Some(View {
                    kind: "clock",
                    group: &b.group,
                    name: &m.family,
                    blocks: m.blocks as i64,
                    volume: m.volume.map(|v| v as i64),
                    period: Some(m.period as i64),
                    open: None,
                    close: None,
                    kinds: &b.kinds,
                    flags: &b.flags,
                    text: [&m.family, &b.caption, &b.name],
                    variant: false,
                })
            }
        }
    }

    /// Run the query: the hits in display order and the facet counts. With a time search, the
    /// clocks of those periods plus every extendable family's members that hit them (an archived
    /// design that is exactly a member shows once, as the member); without one, the builds.
    pub fn run(&self, q: &Query) -> Result<(Vec<Hit>, Facets), String> {
        let index = self.index();
        let time = if q.time.trim().is_empty() { None } else { Some(parse_time_query(&q.time)?) };
        let words: Vec<String> = q.text.to_lowercase().split_whitespace().map(String::from).collect();
        let mut cands: Vec<Hit> = (0..self.builds.len()).map(Hit::Build).collect();
        if time.is_some() {
            // members replace the archived designs they are (same family and n)
            let covered: BTreeSet<&str> = self.members.iter().filter_map(|m| m.build.as_deref()).collect();
            cands.retain(|h| match h {
                Hit::Build(i) => !covered.contains(self.builds[*i].id.as_str()),
                Hit::Member(_) => true,
            });
            cands.extend((0..self.members.len()).map(Hit::Member));
        }
        let pass = |v: &View, skip: Skip| -> bool {
            if skip != Skip::Kind && !q.kind.is_empty() && v.kind != q.kind {
                return false;
            }
            if let Some((lo, hi)) = time {
                match v.period {
                    Some(p) if p >= lo as i64 && p <= hi as i64 => {}
                    _ => return false,
                }
            }
            if !q.variants && v.variant {
                return false;
            }
            if skip != Skip::Groups && !q.groups.is_empty() && !q.groups.contains(v.group) {
                return false;
            }
            if skip != Skip::Kinds {
                if !q.kinds_in.iter().all(|k| v.kinds.iter().any(|x| x == k)) {
                    return false;
                }
                if v.kinds.iter().any(|x| q.kinds_out.contains(x)) {
                    return false;
                }
            }
            if skip != Skip::Flags {
                if !q.flags_in.iter().all(|f| v.flags.iter().any(|x| x == f)) {
                    return false;
                }
                if v.flags.iter().any(|x| q.flags_out.contains(x)) {
                    return false;
                }
            }
            if !(q.blocks.admits(Some(v.blocks)) && q.volume.admits(v.volume) && q.period.admits(v.period) && q.open.admits(v.open) && q.close.admits(v.close)) {
                return false;
            }
            if !words.is_empty() {
                let hay = v.text.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>().join(" ");
                if !words.iter().all(|w| hay.contains(w.as_str()) || v.kinds.iter().any(|k| k.contains(w.as_str()))) {
                    return false;
                }
            }
            true
        };
        let views: Vec<(Hit, View)> = cands.iter().filter_map(|h| self.view(*h, &index).map(|v| (*h, v))).collect();
        let mut facets = Facets::default();
        for (_, v) in &views {
            if pass(v, Skip::Kind) {
                *facets.kinds.entry(v.kind.to_string()).or_default() += 1;
            }
            if pass(v, Skip::Groups) {
                *facets.groups.entry(v.group.to_string()).or_default() += 1;
            }
            if pass(v, Skip::Kinds) {
                for k in v.kinds {
                    *facets.block_kinds.entry(k.clone()).or_default() += 1;
                }
            }
            if pass(v, Skip::Flags) {
                for f in v.flags {
                    *facets.flags.entry(f.clone()).or_default() += 1;
                }
            }
        }
        let mut hits: Vec<(Hit, &View)> = views.iter().filter(|(_, v)| pass(v, Skip::None)).map(|(h, v)| (*h, v)).collect();
        let big = i64::MAX;
        hits.sort_by(|(ha, a), (hb, b)| {
            let primary = match q.sort {
                Sort::Blocks => a.blocks.cmp(&b.blocks).then(a.volume.unwrap_or(big).cmp(&b.volume.unwrap_or(big))),
                Sort::Volume => a.volume.unwrap_or(big).cmp(&b.volume.unwrap_or(big)).then(a.blocks.cmp(&b.blocks)),
                Sort::Period => a.period.unwrap_or(big).cmp(&b.period.unwrap_or(big)).then(a.blocks.cmp(&b.blocks)),
                Sort::Open => a.open.unwrap_or(big).cmp(&b.open.unwrap_or(big)).then(a.close.unwrap_or(big).cmp(&b.close.unwrap_or(big))),
                Sort::Close => a.close.unwrap_or(big).cmp(&b.close.unwrap_or(big)).then(a.open.unwrap_or(big).cmp(&b.open.unwrap_or(big))),
                Sort::Name => std::cmp::Ordering::Equal,
            };
            primary.then(a.group.cmp(b.group)).then(a.name.cmp(b.name)).then(ha.cmp(hb))
        });
        Ok((hits.into_iter().map(|(h, _)| h).collect(), facets))
    }
}

/// The query as a URL fragment (`kind=clock&time=2-3%20s&sort=blocks`), and back: shareable
/// links to a search.
impl Query {
    pub fn to_fragment(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let mut push = |k: &str, v: &str| {
            if !v.is_empty() {
                parts.push(format!("{k}={}", encode(v)));
            }
        };
        push("kind", &self.kind);
        push("q", &self.text);
        push("time", &self.time);
        let join = |s: &BTreeSet<String>| s.iter().cloned().collect::<Vec<_>>().join(",");
        push("group", &join(&self.groups));
        push("with", &join(&self.kinds_in));
        push("without", &join(&self.kinds_out));
        push("is", &join(&self.flags_in));
        push("not", &join(&self.flags_out));
        for (k, r) in [("blocks", self.blocks), ("volume", self.volume), ("period", self.period), ("open", self.open), ("close", self.close)] {
            if !r.is_open() {
                push(k, &format!("{}-{}", r.min.map(|v| v.to_string()).unwrap_or_default(), r.max.map(|v| v.to_string()).unwrap_or_default()));
            }
        }
        if self.variants {
            push("variants", "1");
        }
        if self.sort != Sort::Blocks {
            push("sort", self.sort.key());
        }
        parts.join("&")
    }

    pub fn from_fragment(s: &str) -> Query {
        let mut q = Query::default();
        let set = |v: &str| -> BTreeSet<String> { v.split(',').filter(|x| !x.is_empty()).map(String::from).collect() };
        let range = |v: &str| -> Range {
            let (a, b) = v.split_once('-').unwrap_or((v, v));
            Range { min: a.trim().parse().ok(), max: b.trim().parse().ok() }
        };
        for part in s.trim_start_matches('#').split('&') {
            let Some((k, v)) = part.split_once('=') else { continue };
            let v = decode(v);
            match k {
                "kind" => q.kind = v,
                "q" => q.text = v,
                "time" => q.time = v,
                "group" => q.groups = set(&v),
                "with" => q.kinds_in = set(&v),
                "without" => q.kinds_out = set(&v),
                "is" => q.flags_in = set(&v),
                "not" => q.flags_out = set(&v),
                "blocks" => q.blocks = range(&v),
                "volume" => q.volume = range(&v),
                "period" => q.period = range(&v),
                "open" => q.open = range(&v),
                "close" => q.close = range(&v),
                "variants" => q.variants = v == "1",
                "sort" => q.sort = Sort::from_key(&v),
                _ => {}
            }
        }
        q
    }
}

/// Percent-encoding for a URL fragment value (unreserved characters and `,` kept).
pub fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b',' | b'+' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

pub fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let hex = |b: u8| -> Option<u8> { (b as char).to_digit(16).map(|d| d as u8) };
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_queries() {
        assert_eq!(parse_time_query("48"), Ok((48, 48)));
        assert_eq!(parse_time_query("48 gt"), Ok((48, 48)));
        assert_eq!(parse_time_query("48gt"), Ok((48, 48)));
        assert_eq!(parse_time_query("2.5 s"), Ok((50, 50)));
        assert_eq!(parse_time_query("2.5s"), Ok((50, 50)));
        assert_eq!(parse_time_query("1 min"), Ok((1200, 1200)));
        assert_eq!(parse_time_query("1 min 30 s"), Ok((1800, 1800)));
        assert_eq!(parse_time_query("1:30"), Ok((1800, 1800)));
        assert_eq!(parse_time_query("2 h"), Ok((144000, 144000)));
        assert_eq!(parse_time_query("40-60 gt"), Ok((40, 60)));
        assert_eq!(parse_time_query("40..60"), Ok((40, 60)));
        assert_eq!(parse_time_query("2-3 s"), Ok((40, 60)));
        assert_eq!(parse_time_query("2 s to 3 s"), Ok((40, 60)));
        assert_eq!(parse_time_query("1-2 min"), Ok((1200, 2400)));
        assert_eq!(parse_time_query("60 - 40"), Ok((40, 60)));
        assert_eq!(parse_time_query("< 60 gt"), Ok((0, 59)));
        assert_eq!(parse_time_query("<= 3 s"), Ok((0, 60)));
        assert_eq!(parse_time_query(">= 1 min"), Ok((1200, u32::MAX)));
        assert_eq!(parse_time_query("> 10"), Ok((11, u32::MAX)));
        assert_eq!(parse_time_query("2 rt"), Ok((4, 4)));
        assert_eq!(parse_time_query("0.4 S"), Ok((8, 8)));
        // between two ticks: no period matches, said plainly
        assert!(parse_time_query("0.33 s").is_err());
        assert!(parse_time_query("abc").is_err());
        assert!(parse_time_query("5 parsecs").is_err());
        assert_eq!(parse_time_query("0.3-0.33 s"), Ok((6, 6)));
    }

    #[test]
    fn real_times() {
        assert_eq!(real_time(20), "1 s");
        assert_eq!(real_time(30), "1.5 s");
        assert_eq!(real_time(2304), "1 min 55.2 s");
        assert_eq!(real_time(1200), "1 min");
        assert_eq!(real_time(72000 + 6000), "1 h 5 min");
        assert_eq!(gt_time(8), "8 gt (0.4 s)");
    }

    #[test]
    fn formulas_solve() {
        let f = Formula::Linear { a: 10, b: 4 };
        assert_eq!(f.solve(0, 47, 48, 48), Vec::<i64>::new());
        assert_eq!(f.solve(0, 47, 50, 50), vec![10]);
        assert_eq!(f.solve(0, 47, 40, 60), vec![8, 9, 10, 11, 12]);
        let g = Formula::Geometric { a: 12, k: 2, c: 0 };
        assert_eq!(g.solve(0, 8, 40, 100), vec![2, 3]);
        assert_eq!(g.eval(8), 3072);
    }

    fn clock(id: &str, period: u32, blocks: u32, group: &str, kinds: &[&str], flags: &[&str]) -> Build {
        Build {
            id: format!("clocks/{id}"),
            kind: "clock".into(),
            group: group.into(),
            name: id.into(),
            page: format!("clocks/{id}.html"),
            design: format!("designs/clocks/{id}.txt"),
            blocks,
            volume: Some(blocks),
            kinds: kinds.iter().map(|s| s.to_string()).collect(),
            flags: flags.iter().map(|s| s.to_string()).collect(),
            clock: Some(ClockInfo { period, ..Default::default() }),
            ..Default::default()
        }
    }

    fn site() -> Site {
        let mut ring = clock("obs-ring", 8, 4, "observer", &["observer"], &["robust", "extendable"]);
        ring.clock.as_mut().unwrap().card = Some("obs-ring".into());
        let mut variant = clock("obs-note", 6, 3, "observer+note_block", &["note_block", "observer"], &["variant"]);
        variant.clock.as_mut().unwrap().host = Some("clocks/obs2".into());
        let door = Build {
            id: "doors/fc2-40b".into(),
            kind: "door".into(),
            group: "solid".into(),
            name: "fc2-40b".into(),
            blocks: 40,
            volume: Some(144),
            kinds: vec!["observer".into(), "slime_block".into(), "sticky_piston".into()],
            flags: vec!["rotation-safe".into(), "wall".into()],
            door: Some(Door { open: Some(14), close: Some(16), ..Default::default() }),
            ..Default::default()
        };
        let members = (0..=10)
            .map(|n| Member {
                card: "obs-ring".into(),
                family: "obs-ring".into(),
                n,
                period: 8 + 4 * n as u32,
                blocks: 4 + 2 * n as u32,
                design: format!("designs/clocks/members/obs-ring-n{n}.txt"),
                dims: None,
                volume: Some(4 + 2 * n as u32),
                status: if n < 3 { STATUS_JUDGED.into() } else { STATUS_FORMULA.into() },
                build: (n == 0).then(|| "clocks/obs-ring".to_string()),
                picture: None,
            })
            .collect();
        Site {
            version: DATA_VERSION,
            builds: vec![
                clock("obs2", 6, 2, "observer", &["observer"], &["robust"]),
                ring,
                clock("lamp48", 48, 9, "observer+redstone_lamp", &["observer", "redstone_lamp"], &[]),
                variant,
                door,
            ],
            cards: vec![Card {
                id: "obs-ring".into(),
                families: vec![Family {
                    id: "obs-ring".into(),
                    build: "clocks/obs-ring".into(),
                    period: Formula::Linear { a: 8, b: 4 },
                    period_text: "8 + 4n".into(),
                    blocks: Formula::Linear { a: 4, b: 2 },
                    blocks_text: "4 + 2n".into(),
                    rule: "insert 2 observers".into(),
                    n_lo: 0,
                    n_hi: 10,
                    base: 0,
                    judged: vec![0, 1, 2],
                    extra: vec![],
                }],
                ..Default::default()
            }],
            members,
            ..Default::default()
        }
    }

    #[test]
    fn query_kinds_flags_ranges() {
        let s = site();
        let ids = |q: &Query| -> Vec<String> {
            s.run(q).unwrap().0.iter().map(|h| match h {
                Hit::Build(i) => s.builds[*i].id.clone(),
                Hit::Member(i) => format!("{}#n{}", s.members[*i].family, s.members[*i].n),
            }).collect()
        };
        // default: builds, variants hidden, fewest blocks first
        assert_eq!(ids(&Query::default()), vec!["clocks/obs2", "clocks/obs-ring", "clocks/lamp48", "doors/fc2-40b"]);
        let q = Query { kind: "door".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["doors/fc2-40b"]);
        let q = Query { kinds_out: ["redstone_lamp".to_string()].into(), kind: "clock".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2", "clocks/obs-ring"]);
        let q = Query { kinds_in: ["redstone_lamp".to_string()].into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48"]);
        let q = Query { flags_in: ["robust".to_string()].into(), flags_out: ["extendable".to_string()].into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2"]);
        let q = Query { variants: true, kind: "clock".into(), period: Range { min: Some(6), max: Some(6) }, ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2", "clocks/obs-note"]);
        let q = Query { open: Range { min: None, max: Some(14) }, ..Default::default() };
        assert_eq!(ids(&q), vec!["doors/fc2-40b"]);
        let q = Query { text: "LAMP".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48"]);
        // facet counts ignore their own facet
        let q = Query { kind: "door".into(), ..Default::default() };
        let (_, f) = s.run(&q).unwrap();
        assert_eq!(f.kinds.get("clock"), Some(&3));
        assert_eq!(f.kinds.get("door"), Some(&1));
        assert_eq!(f.block_kinds.get("slime_block"), Some(&1));
    }

    #[test]
    fn time_search_expands_families() {
        let s = site();
        let ids = |q: &Query| -> Vec<String> {
            s.run(q).unwrap().0.iter().map(|h| match h {
                Hit::Build(i) => s.builds[*i].id.clone(),
                Hit::Member(i) => format!("{}#n{}", s.members[*i].family, s.members[*i].n),
            }).collect()
        };
        // 48 gt: the archived lamp clock (9 blocks) and the ring's n = 10 member (24 blocks)
        let q = Query { time: "48 gt".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48", "obs-ring#n10"]);
        let q = Query { time: "2.4 s".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/lamp48", "obs-ring#n10"]);
        // the base design (n = 0) shows once, as the member
        let q = Query { time: "8".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["obs-ring#n0"]);
        // 6..12 gt: the variant (6 gt) stays hidden
        let q = Query { time: "0.3-0.6 s".into(), ..Default::default() };
        assert_eq!(ids(&q), vec!["clocks/obs2", "obs-ring#n0", "obs-ring#n1"]);
        assert!(s.run(&Query { time: "0.33 s".into(), ..Default::default() }).is_err());
    }

    #[test]
    fn fragments_round_trip() {
        let q = Query {
            kind: "clock".into(),
            text: "lamp ring".into(),
            time: "2-3 s".into(),
            kinds_in: ["observer".to_string()].into(),
            flags_out: ["variant".to_string(), "chiral".to_string()].into(),
            blocks: Range { min: Some(2), max: None },
            sort: Sort::Period,
            variants: true,
            ..Default::default()
        };
        let f = q.to_fragment();
        assert_eq!(Query::from_fragment(&f), q, "{f}");
        assert_eq!(Query::from_fragment(""), Query::default());
        assert_eq!(decode(&encode("a b/ç&=")), "a b/ç&=");
    }
}
