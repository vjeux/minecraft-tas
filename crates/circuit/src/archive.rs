//! The archive and the catalog of the contraption studies. `mctas circuit archive` judges a design
//! in the real game and keeps it in `designs/circuits/<study>/archive/` (the design with its judge
//! line, a row of `verdicts.tsv`, a caption in `captions.tsv`); `mctas circuit catalog` writes, per
//! study, `CATALOG.md` (by the study's axis, smallest first; a design that only adds block kinds to
//! a smaller one with the same axis value is a variant, listed on `variants.md`), the index
//! `designs/circuits/CATALOG.md`, and the data the website renders: `designs/circuits/catalog.tsv`.

use crate::check::Outcome;
use crate::verify::{self, Options};
use crate::{Circuit, Kind};
use anyhow::{Context, Result, anyhow, bail};
use mctas::sim::Pool;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const VERDICTS: &str = "verdicts.tsv";
pub const CAPTIONS: &str = "captions.tsv";
const HEADER: &str = "archived\tfile\tkind\taxis\tmetrics\tblocks\tdims\tvolume\tfamily\trobustness\tkinds\ttype_key\thash\tsummary";

/// The studies, in order, with their titles.
pub const STUDIES: [(&str, &str); 8] = [
    ("pulse", "Pulse circuits"),
    ("tff", "T flip-flops"),
    ("delay", "Delay lines"),
    ("gates", "Logic gates"),
    ("memory", "Memory"),
    ("pext", "Piston extenders"),
    ("fly", "Flying machines"),
    ("shulker", "Shulker loaders and unloaders"),
];

/// One archived circuit (a row of `verdicts.tsv`).
#[derive(Clone, Debug)]
pub struct Row {
    pub archived: String,
    pub file: String,
    pub kind: String,
    /// The study's axis value (`L = 4 gt`, `xor`, `N = 3`...).
    pub axis: String,
    pub metrics: String,
    pub blocks: usize,
    pub dims: String,
    pub volume: i64,
    /// The kinds of the blocks whose state changes during the run.
    pub family: String,
    pub robustness: String,
    /// The block kinds it is built with (`+`-joined), for the dominance fold.
    pub kinds: String,
    pub type_key: String,
    pub hash: String,
    pub summary: String,
}

impl Row {
    fn to_tsv(&self) -> String {
        [
            self.archived.clone(),
            self.file.clone(),
            self.kind.clone(),
            self.axis.clone(),
            self.metrics.clone(),
            self.blocks.to_string(),
            self.dims.clone(),
            self.volume.to_string(),
            self.family.clone(),
            self.robustness.clone(),
            self.kinds.clone(),
            self.type_key.clone(),
            self.hash.clone(),
            self.summary.clone(),
        ]
        .map(|s| s.replace(['\t', '\n'], " "))
        .join("\t")
    }

    fn parse(line: &str) -> Option<Row> {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 14 {
            return None;
        }
        Some(Row {
            archived: f[0].into(),
            file: f[1].into(),
            kind: f[2].into(),
            axis: f[3].into(),
            metrics: f[4].into(),
            blocks: f[5].parse().ok()?,
            dims: f[6].into(),
            volume: f[7].parse().unwrap_or(0),
            family: f[8].into(),
            robustness: f[9].into(),
            kinds: f[10].into(),
            type_key: f[11].into(),
            hash: f[12].into(),
            summary: f[13].into(),
        })
    }

    fn kind_set(&self) -> BTreeSet<String> {
        self.kinds.split('+').filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    }
}

/// A study's archive directory.
pub fn archive_dir(root: &Path, study: &str) -> PathBuf {
    root.join(study).join("archive")
}

/// The rows of an archive, one per file (a repeated file keeps its newest row).
pub fn read_rows(dir: &Path) -> Result<Vec<Row>> {
    let p = dir.join(VERDICTS);
    if !p.exists() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    let mut rows: Vec<Row> = Vec::new();
    for r in text.lines().skip(1).filter_map(Row::parse) {
        rows.retain(|o| o.file != r.file);
        rows.push(r);
    }
    Ok(rows)
}

fn write_rows(dir: &Path, rows: &[Row]) -> Result<()> {
    let mut all = format!("{HEADER}\n");
    for r in rows {
        all.push_str(&r.to_tsv());
        all.push('\n');
    }
    std::fs::write(dir.join(VERDICTS), all)?;
    Ok(())
}

fn read_captions(dir: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(dir.join(CAPTIONS))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| l.split_once('\t'))
        .map(|(f, c)| (f.to_string(), c.to_string()))
        .collect()
}

/// The design the type key is computed on: every swap (sticky pistons -> pistons, inert solids ->
/// stone: `clock::ident`) whose run on redsim under the kind's stimulus is identical modulo the swap
/// (every cell's full state at every frame); the circuit itself when redsim does not model it.
pub fn normalized(c: &Circuit) -> Circuit {
    let Some(kind) = c.kind.clone() else {
        return c.clone();
    };
    let gap = verify::sizing(c, 3).unwrap_or(crate::check::Sizing::default().gap);
    let s = crate::check::stimulus(c, &kind, &crate::check::Sizing { gap });
    let Ok(mut base) = crate::run::sim_trace(c, &s, 3) else {
        return c.clone();
    };
    let mut cur = c.clone();
    let swaps: [&dyn Fn(&str) -> String; 2] = [&clock::ident::unstick, &clock::ident::unsolid];
    for f in swaps {
        let mut d = cur.clock.design.clone();
        let mut changed = false;
        for st in d.blocks.values_mut() {
            let n = f(st);
            if n != *st {
                *st = n;
                changed = true;
            }
        }
        let Some(c2) = changed.then(|| Circuit::from_design(d).ok()).flatten() else {
            continue;
        };
        let Ok(t2) = crate::run::sim_trace(&c2, &s, 3) else {
            continue;
        };
        let same = base.frames() == t2.frames()
            && base.cells.iter().zip(&t2.cells).all(|(a, b)| a.len() == b.len() && a.iter().zip(b).all(|((p, x), (q, y))| p == q && f(&base.names[*x as usize]) == f(&t2.names[*y as usize])));
        if same {
            cur = c2;
            base = t2;
        }
    }
    cur
}

/// The structure's TUNABLE key: the design with every repeater's delay and every comparator's mode
/// masked, with its input and output cells marked, up to translation, rotation and mirror image.
/// Designs that differ only by those settings are one tunable mechanism (vjeux 2026-10-19: "we
/// don't need 20 variations of the same design"): the catalog shows them as one group card.
pub fn tunable_key(c: &Circuit) -> String {
    let mut d = c.clock.design.clone();
    for st in d.blocks.values_mut() {
        if st.contains("repeater[") {
            *st = mask_prop(st, "delay", "1");
        }
        if st.contains("comparator[") {
            *st = mask_prop(st, "mode", "compare");
        }
    }
    for i in &c.inputs {
        d.blocks.insert(i.pos, "circuit:input".to_string());
        d.block_entities.remove(&i.pos);
    }
    for p in &c.outputs {
        d.entities.push(mctas::design::EntitySpec { kind: "circuit:output".into(), pos: [p.pos[0] as f64 + 0.5, p.pos[1] as f64, p.pos[2] as f64 + 0.5], nbt: None });
    }
    clock::canon::design_type_key(&d)
}

/// A block state with one property set to a value (`delay=3` -> `delay=1`).
fn mask_prop(state: &str, key: &str, value: &str) -> String {
    let Some(i) = state.find(&format!("{key}=")) else {
        return state.to_string();
    };
    let start = i + key.len() + 1;
    let end = state[start..].find([',', ']']).map(|j| start + j).unwrap_or(state.len());
    format!("{}{value}{}", &state[..start], &state[end..])
}

/// The structure's type key: the [`normalized`] design up to translation, the 4 rotations and the
/// mirror image (`clock::canon::design_type_key`), with its input cells marked (whatever drives
/// them: a lever and a redstone-block input are the same circuit) and its output and payload cells
/// marked.
pub fn type_key(c: &Circuit) -> String {
    let n = normalized(c);
    let c = &n;
    let mut d = c.clock.design.clone();
    for i in &c.inputs {
        // the input DEVICE is part of the type (a lever, a stone or a wooden button, a judge-placed
        // block: different timings, coordinator 2026-10-19); its facing is not
        let id = |s: &str| -> String {
            let s = s.split('[').next().unwrap_or(s);
            s.strip_prefix("minecraft:").unwrap_or(s).to_string()
        };
        let dev = match &i.how {
            crate::InputHow::Block { on, .. } => id(on),
            crate::InputHow::Lever => "lever".to_string(),
            crate::InputHow::Button => c.clock.design.blocks.get(&i.pos).map(|s| id(s)).unwrap_or_else(|| "button".into()),
        };
        d.blocks.insert(i.pos, format!("circuit:input_{dev}"));
        d.block_entities.remove(&i.pos);
    }
    for (tag, probes) in [("circuit:output", &c.outputs), ("circuit:payload", &c.payloads)] {
        for p in probes {
            d.entities.push(mctas::design::EntitySpec { kind: tag.into(), pos: [p.pos[0] as f64 + 0.5, p.pos[1] as f64, p.pos[2] as f64 + 0.5], nbt: None });
        }
    }
    clock::canon::design_type_key(&d)
}

/// A circuit's value on its study's axis.
pub fn axis(kind: &Kind, out: &Outcome, family: &str) -> String {
    let m = |k: &str| out.metrics.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()).unwrap_or_default();
    match kind {
        Kind::RisingEdge | Kind::FallingEdge | Kind::DualEdge | Kind::PulseLimiter | Kind::PulseExtender => format!("L = {}", m("L")),
        Kind::Delay => format!("N = {}", m("N")),
        Kind::Gate(_) => m("function"),
        Kind::Tff => family.to_string(),
        Kind::RsLatch | Kind::DLatch | Kind::TffReset | Kind::Dff | Kind::DffEnable => kind.name(),
        Kind::Counter(n) => format!("counter {n}"),
        Kind::Shift(n) => format!("shift {n}"),
        Kind::PistonExtender(n) => format!("N = {n}"),
        Kind::FlyingMachine => {
            let s = m("speed");
            if s.is_empty() { "two-way".into() } else { s }
        }
        // the item class: the gallery's rows; speed and size are the columns
        Kind::ShulkerLoader(s) => format!("load {s}-stack"),
        Kind::ShulkerUnloader(s) => format!("unload {s}-stack"),
    }
}

/// Result of [`archive`].
pub struct Archived {
    pub file: PathBuf,
    pub summary: String,
    pub new: bool,
}

/// `mctas circuit archive`: the judge, then the design kept in its study's archive. Refused: a
/// failing design, or one whose structure (type key) is already archived as the same kind.
pub fn archive(pool: &Pool, path: &Path, o: &Options, root: &Path, caption: Option<&str>) -> Result<Archived> {
    let c = Circuit::load(path)?;
    let kind = c.kind.clone().ok_or_else(|| anyhow!("{}: no `circuit: <kind>` line", path.display()))?;
    let dir = archive_dir(root, kind.study());
    let text = std::fs::read_to_string(path)?;
    // the same blocks and ports under another kind (a claim) are another entry
    let hash = format!("{}-{}", clock::catalog::content_hash(&c.clock), kind.name().replace(' ', "_"));
    let rows = read_rows(&dir)?;
    if let Some(r) = rows.iter().find(|r| r.hash == hash) {
        return Ok(Archived { file: dir.join(&r.file), summary: r.summary.clone(), new: false });
    }
    let tk = type_key(&c);
    if let Some(r) = rows.iter().find(|r| r.type_key == tk && r.kind == kind.name()).filter(|_| !c.clock.design.meta.contains_key("extend family")) {
        return Ok(Archived { file: dir.join(&r.file), summary: format!("same type as {} (a rotation, mirror image or move of it, same kind): not archived", r.file), new: false });
    }
    let rep = verify::verify_in_pool(pool, &c, o)?;
    if !rep.pass {
        bail!("not archived: {}", rep.summary(&c));
    }
    let base = rep.base().ok_or_else(|| anyhow!("no passing build"))?.clone();
    // an extendable family: n = low, low+1, low+2, the middle and the high end judged in the game
    // first (axis value and blocks must follow the formulas), recorded in extend.tsv
    let family = family_of(&c)?;
    let instances = match &family {
        Some(f) => check_family(pool, path, f, &kind, o).map_err(|e| anyhow!("not archived: {e:#}"))?,
        None => Vec::new(),
    };
    std::fs::create_dir_all(&dir)?;
    let stem: String = c.name().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '-' }).collect();
    let stem = if stem.is_empty() { "circuit".to_string() } else { stem };
    let mut file = format!("{stem}.txt");
    // a name taken by ANOTHER structure: name + '-' + a short hash of the type, then -2, -3...
    if dir.join(&file).exists() {
        let h = tk.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        file = format!("{stem}-{:06x}.txt", h & 0xffffff);
    }
    let base_file = file.trim_end_matches(".txt").to_string();
    let mut n = 2;
    while dir.join(&file).exists() {
        file = format!("{base_file}-{n}.txt");
        n += 1;
    }
    let summary = rep.summary(&c);
    let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
    let now = clock::catalog::now_utc();
    std::fs::write(dir.join(&file), format!("# archived {now} from {} by mctas circuit archive\n# {summary}\n{body}", path.display()))?;
    let [w, h, d] = base.dims;
    let row = Row {
        archived: now,
        file: file.clone(),
        kind: kind.name(),
        axis: axis(&kind, &base.outcome, &base.family),
        metrics: base.outcome.metrics_text(),
        blocks: c.blocks(),
        dims: format!("{w}x{h}x{d}"),
        volume: (w * h * d) as i64,
        family: base.family.clone(),
        robustness: if rep.note.is_empty() { rep.verdict.clone() } else { format!("{} ({})", rep.verdict, rep.note) },
        kinds: c.kinds().into_iter().collect::<Vec<_>>().join("+"),
        type_key: tk,
        hash,
        summary: summary.clone(),
    };
    let mut now_rows = read_rows(&dir)?;
    now_rows.retain(|r| r.file != row.file);
    now_rows.push(row);
    write_rows(&dir, &now_rows)?;
    let cap = caption.map(|s| s.to_string()).unwrap_or_else(|| clock::catalog::default_caption(&body));
    if !cap.is_empty() {
        let cp = dir.join(CAPTIONS);
        let old = if cp.exists() { std::fs::read_to_string(&cp)? } else { "file\tcaption\n".to_string() };
        let mut caps: String = old.lines().filter(|l| l.split('\t').next() != Some(file.as_str())).map(|l| format!("{l}\n")).collect();
        caps.push_str(&format!("{file}\t{}\n", cap.replace(['\t', '\n'], " ")));
        std::fs::write(&cp, caps)?;
    }
    if !instances.is_empty() {
        let ep = dir.join(EXTEND_TSV);
        let fam = &instances[0].family;
        let old = if ep.exists() { std::fs::read_to_string(&ep)? } else { "family\tn\taxis\tblocks\ttype_key\n".to_string() };
        let mut all: String = old.lines().filter(|l| l.split('\t').next() != Some(fam.as_str())).map(|l| format!("{l}\n")).collect();
        for i in &instances {
            all.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", i.family, i.n, i.axis, i.blocks, i.type_key));
        }
        std::fs::write(&ep, all)?;
    }
    let summary = match &family {
        Some(f) => format!("{summary}; extend family {}: {} = {}, n = {}..{} (judged: n = {})", f.id, axis_label(&axis(&kind, &base.outcome, &base.family)).0, f.period.text(), f.n_lo, f.n_hi, instances.iter().map(|i| i.n.to_string()).collect::<Vec<_>>().join(", ")),
        None => summary,
    };
    Ok(Archived { file: dir.join(file), summary, new: true })
}

/// The sort key of an axis value: its first number (if any), then the text.
fn axis_key(a: &str) -> (i64, String) {
    let digits: String = a.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    (digits.parse().unwrap_or(i64::MAX), a.to_string())
}

/// The first integer of an axis value (`L = 8 gt` -> 8).
fn axis_number(a: &str) -> Option<i64> {
    let digits: String = a.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// An axis value's label and unit (`L = 8 gt` -> ("L", "gt"); `N = 3` -> ("N", ""); a flying
/// machine's `10 gt/block` -> ("speed", "gt/block")).
fn axis_label(a: &str) -> (String, String) {
    match a.split_once(" = ") {
        Some((l, v)) => (l.to_string(), v.split_whitespace().nth(1).unwrap_or("").to_string()),
        None if a.ends_with(" gt/block") => ("speed".to_string(), "gt/block".to_string()),
        // a counter's axis is its kind, `counter N`
        None if a.starts_with("counter ") => ("N".to_string(), String::new()),
        None => (String::new(), String::new()),
    }
}

pub const EXTEND_TSV: &str = "extend.tsv";

/// A circuit's extendable family (the clock `extend` headers: docs/clocks.md#extendable-clocks):
/// `extend axis:` is the formula of the study's axis value in n (L, N, the reach...; `extend
/// period:` is read the same), `extend blocks:` the block count's.
pub fn family_of(c: &Circuit) -> Result<Option<clock::extend::Family>> {
    let mut d = c.clock.design.clone();
    if !d.meta.contains_key("extend family") {
        return Ok(None);
    }
    if let Some(a) = d.meta.get("extend axis").cloned() {
        d.meta.entry("extend period".to_string()).or_insert(a);
    }
    clock::extend::Family::of(&clock::Clock::from_design(d)?)
}

/// One judged member of a family (a row of `extend.tsv`).
#[derive(Clone, Debug)]
pub struct Instance {
    pub family: String,
    pub n: i64,
    pub axis: String,
    pub blocks: usize,
    pub type_key: String,
}

fn read_instances(dir: &Path) -> Vec<Instance> {
    std::fs::read_to_string(dir.join(EXTEND_TSV))
        .unwrap_or_default()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f.len() >= 5).then(|| Instance { family: f[0].into(), n: f[1].parse().unwrap_or(0), axis: f[2].into(), blocks: f[3].parse().unwrap_or(0), type_key: f[4].into() })
        })
        .collect()
}

/// Judge the members `ns` of a family, built with its generator, in the game: each must pass with
/// the formulas' axis value and block count.
fn check_family(pool: &Pool, path: &Path, f: &clock::extend::Family, kind: &Kind, o: &Options) -> Result<Vec<Instance>> {
    let dir = mctas::paths::run_dir().join(format!("circuit-extend-{}", std::process::id()));
    let mut out = Vec::new();
    for n in f.sample() {
        let p = f.build(path, n, &dir).with_context(|| format!("extend family {}: building n = {n}", f.id))?;
        let c = Circuit::load(&p)?;
        let rep = verify::verify_in_pool(pool, &c, o)?;
        let Some(b) = rep.base().filter(|_| rep.pass) else {
            bail!("extend family {}: n = {n}: {}", f.id, rep.summary(&c));
        };
        // the member's own kind: its argument is the axis of `piston-extender N` / `counter N`
        let ax = axis(c.kind.as_ref().unwrap_or(kind), &b.outcome, &b.family);
        let (want_a, want_b) = (f.period.eval(n), f.blocks.eval(n));
        if axis_number(&ax) != Some(want_a) || c.blocks() as i64 != want_b {
            bail!("extend family {}: n = {n}: {ax}, {} blocks; the formulas say {} = {want_a}, {want_b} blocks", f.id, c.blocks(), axis_label(&ax).0);
        }
        out.push(Instance { family: f.id.clone(), n, axis: ax, blocks: c.blocks(), type_key: type_key(&c) });
    }
    Ok(out)
}

pub const ALIASES: &str = "aliases.tsv";

/// `mctas circuit archive --rejudge`: judge every archived design again (the current judge, in
/// the game; family bases with their sampled members) in the given studies (all when empty). A
/// passing design keeps its row with the new metrics, box, family, robustness and summary (its
/// `# CIRCUIT:` line rewritten); a failing one leaves the archive (file, verdict row, caption,
/// its family's extend.tsv rows, aliases it hosts) and is copied back next to the track file it
/// was archived from, under `update-less/` when it fails for an update-less output (else
/// `rejudged/`), with a first line saying why.
pub fn rejudge(pool: &Pool, root: &Path, o: &Options, studies: &[String]) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for (study, _) in STUDIES.iter().filter(|(s, _)| studies.is_empty() || studies.iter().any(|w| w == s)) {
        let dir = archive_dir(root, study);
        let rows = read_rows(&dir)?;
        if rows.is_empty() {
            continue;
        }
        let mut keep: Vec<Row> = Vec::new();
        let mut removed: Vec<String> = Vec::new();
        let mut instances = read_instances(&dir);
        for row in rows {
            let path = dir.join(&row.file);
            if !path.exists() {
                out.push(format!("MISSING {}: no file, row dropped", path.display()));
                removed.push(row.file.clone());
                continue;
            }
            let c = Circuit::load(&path)?;
            let Some(kind) = c.kind.clone() else {
                keep.push(row);
                continue;
            };
            let rep = verify::verify_in_pool(pool, &c, o)?;
            let fam = family_of(&c)?;
            let fam_check = match (&fam, rep.pass) {
                (Some(f), true) => Some(check_family(pool, &path, f, &kind, o)),
                _ => None,
            };
            let failure = if !rep.pass {
                Some(rep.summary(&c))
            } else if let Some(Err(e)) = &fam_check {
                Some(format!("{e:#}"))
            } else {
                None
            };
            let text = std::fs::read_to_string(&path)?;
            if let Some(why) = failure {
                // back to the track it came from, with why
                let from = text.lines().find_map(|l| l.strip_prefix("# archived ").and_then(|r| r.split(" from ").nth(1)).map(|r| r.trim_end_matches(" by mctas circuit archive").to_string()));
                let sub = if why.contains("update-less output") { "update-less" } else { "rejudged" };
                let back = match from.as_deref().map(Path::new).and_then(|p| p.parent()) {
                    Some(d) if d.starts_with(root.join(study).join("tracks")) => d.join(sub),
                    _ => root.join(study).join("tracks").join(sub),
                };
                std::fs::create_dir_all(&back)?;
                let body: String = text.lines().filter(|l| !l.starts_with("# archived ") && !l.starts_with("# CIRCUIT: ")).map(|l| format!("{l}\n")).collect();
                let copy = back.join(&row.file);
                std::fs::write(&copy, format!("# {sub} ({}, mctas circuit archive --rejudge): {why}\n{body}", clock::catalog::now_utc()))?;
                std::fs::remove_file(&path)?;
                if let Some(f) = &fam {
                    instances.retain(|i| i.family != f.id);
                }
                out.push(format!("REMOVED {}: {why} (copy: {})", path.display(), copy.display()));
                removed.push(row.file.clone());
                continue;
            }
            let base = rep.base().ok_or_else(|| anyhow!("no passing build"))?.clone();
            let summary = rep.summary(&c);
            let [w, h, d] = base.dims;
            let new = Row {
                axis: axis(&kind, &base.outcome, &base.family),
                metrics: base.outcome.metrics_text(),
                blocks: c.blocks(),
                dims: format!("{w}x{h}x{d}"),
                volume: (w * h * d) as i64,
                family: base.family.clone(),
                robustness: if rep.note.is_empty() { rep.verdict.clone() } else { format!("{} ({})", rep.verdict, rep.note) },
                summary: summary.clone(),
                ..row.clone()
            };
            let changed = new.metrics != row.metrics || new.robustness != row.robustness || new.dims != row.dims;
            let header_lines: Vec<String> = text.lines().map(|l| if l.starts_with("# CIRCUIT: ") || l.starts_with("# FAIL: ") { format!("# {summary}") } else { l.to_string() }).collect();
            std::fs::write(&path, header_lines.join("\n") + "\n")?;
            if let (Some(f), Some(Ok(inst))) = (&fam, fam_check) {
                instances.retain(|i| i.family != f.id);
                instances.extend(inst);
            }
            out.push(if changed { format!("UPDATED {}: {} (was: {}, {})", path.display(), summary, row.metrics, row.robustness) } else { format!("KEPT {}: {summary}", path.display()) });
            keep.push(new);
        }
        write_rows(&dir, &keep)?;
        if !removed.is_empty() {
            let cp = dir.join(CAPTIONS);
            if cp.exists() {
                let caps: String = std::fs::read_to_string(&cp)?.lines().filter(|l| !removed.iter().any(|f| l.split('\t').next() == Some(f.as_str()))).map(|l| format!("{l}\n")).collect();
                std::fs::write(&cp, caps)?;
            }
            let ap = dir.join(ALIASES);
            if ap.exists() {
                let al: String = std::fs::read_to_string(&ap)?.lines().filter(|l| !removed.iter().any(|f| l.split('\t').next() == Some(f.as_str()))).map(|l| format!("{l}\n")).collect();
                std::fs::write(&ap, al)?;
            }
        }
        let ep = dir.join(EXTEND_TSV);
        if ep.exists() || !instances.is_empty() {
            let mut all = "family\tn\taxis\tblocks\ttype_key\n".to_string();
            for i in &instances {
                all.push_str(&format!("{}\t{}\t{}\t{}\t{}\n", i.family, i.n, i.axis, i.blocks, i.type_key));
            }
            std::fs::write(&ep, all)?;
        }
    }
    Ok(out)
}

/// The aliases of an archive: kept file -> the names of its removed twins (with what they were
/// built with).
fn read_aliases(dir: &Path) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for l in std::fs::read_to_string(dir.join(ALIASES)).unwrap_or_default().lines().skip(1) {
        let f: Vec<&str> = l.split('\t').collect();
        if f.len() >= 3 {
            m.entry(f[0].to_string()).or_default().push(f[2].to_string());
        }
    }
    m
}

/// `mctas circuit archive --dedupe`: in every study, keep ONE archived design per (kind, type) (the
/// earliest archived), remove the others (design, verdict row, caption) and record them in
/// `aliases.tsv` (kept file, removed file, its name and what it was built with), which the catalog
/// lists as "also found as". The type keys are recomputed from the files (the swaps included).
pub fn dedupe(root: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for (study, _) in STUDIES {
        let dir = archive_dir(root, study);
        let mut rows = read_rows(&dir)?;
        if rows.is_empty() {
            continue;
        }
        rows.sort_by(|a, b| (&a.archived, &a.file).cmp(&(&b.archived, &b.file)));
        let mut kept: BTreeMap<(String, String), String> = BTreeMap::new();
        let mut gone: Vec<(String, String, String)> = Vec::new();
        for r in &rows {
            let c = Circuit::load(&dir.join(&r.file));
            // an extendable family's base is its own entry: never removed, never removes another
            if c.as_ref().is_ok_and(|c| c.clock.design.meta.contains_key("extend family")) {
                continue;
            }
            let tk = c.as_ref().map(type_key).unwrap_or_else(|_| r.type_key.clone());
            match kept.get(&(r.kind.clone(), tk.clone())) {
                Some(k) if *k != r.file => {
                    let with = match (Circuit::load(&dir.join(k)), &c) {
                        (Ok(a), Ok(b)) => {
                            let (ka, kb) = (a.kinds(), b.kinds());
                            let extra: Vec<&String> = kb.difference(&ka).collect();
                            let missing: Vec<&String> = ka.difference(&kb).collect();
                            let mut parts = Vec::new();
                            if !extra.is_empty() {
                                parts.push(format!("with {}", extra.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                            }
                            if !missing.is_empty() {
                                parts.push(format!("without {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")));
                            }
                            if parts.is_empty() { String::new() } else { format!(" (built {})", parts.join(", ")) }
                        }
                        _ => String::new(),
                    };
                    gone.push((k.clone(), r.file.clone(), format!("{}{with}", r.file.trim_end_matches(".txt"))));
                }
                Some(_) => {}
                None => {
                    kept.insert((r.kind.clone(), tk), r.file.clone());
                }
            }
        }
        if gone.is_empty() {
            continue;
        }
        let removed: BTreeSet<String> = gone.iter().map(|g| g.1.clone()).collect();
        rows.retain(|r| !removed.contains(&r.file));
        write_rows(&dir, &rows)?;
        let caps: String = std::fs::read_to_string(dir.join(CAPTIONS)).unwrap_or_else(|_| "file\tcaption\n".into()).lines().filter(|l| !l.split('\t').next().is_some_and(|f| removed.contains(f))).map(|l| format!("{l}\n")).collect();
        std::fs::write(dir.join(CAPTIONS), caps)?;
        let ap = dir.join(ALIASES);
        let mut al = if ap.exists() { std::fs::read_to_string(&ap)? } else { "kept\tremoved\tname\n".to_string() };
        for (k, f, name) in &gone {
            al.push_str(&format!("{k}\t{f}\t{name}\n"));
            let _ = std::fs::remove_file(dir.join(f));
            lines.push(format!("REMOVED {study}/{f} ({name}): same type as {k} (kept, archived first)"));
        }
        std::fs::write(&ap, al)?;
    }
    if lines.is_empty() {
        lines.push("no duplicate types".into());
    }
    Ok(lines)
}

/// The "Extendable families" section of a study page (one card per family; families of one
/// `extend group`, or that differ by one block kind only, share a card, as for clocks) and the
/// families' rows of `families.tsv` (the website's data).
fn family_cards(fams: &[(Row, clock::extend::Family, BTreeSet<String>)], instances: &[Instance], ftsv: &mut String, study: &str) -> String {
    use clock::extend::Family;
    if fams.is_empty() {
        return String::new();
    }
    let items: Vec<clock::catalog::CardItem> = fams
        .iter()
        .enumerate()
        .map(|(i, (r, f, k))| clock::catalog::CardItem { formula: f.period.text(), kinds: k.clone(), size: (f.blocks.eval(f.n_lo), r.volume, i), group: f.group.clone() })
        .collect();
    let mut cards = clock::catalog::card_groups(&items);
    cards.sort_by_key(|c| {
        let f = &fams[c.members[0]].1;
        (f.period.eval(f.n_lo), f.id.clone())
    });
    let judged = |f: &Family| -> String { instances.iter().filter(|i| i.family == f.id).map(|i| i.n.to_string()).collect::<Vec<_>>().join(", ") };
    let mut md = String::from(
        "\n## Extendable families\n\nEach card: the value's formula in n, the size, how to extend it and the n range judged in the game (`mctas circuit archive` judges n = low, low+1, low+2, the middle and the high end); the members are not repeated below.\n",
    );
    for card in &cards {
        let (r, f, _) = &fams[card.members[0]];
        let (label, unit) = axis_label(&r.axis);
        let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        let val = |f: &Family, n: i64| format!("{label} = {}{u}", f.period.eval(n));
        if card.members.len() == 1 {
            md.push_str(&format!("\n### {label} = {}{u}\n\n**{}** ([base design](archive/{})) · {} blocks · {}\n\n", f.period.text(), f.id, r.file, f.blocks.text(), r.kind));
            let ns: Vec<i64> = [f.n_lo, f.n_lo + 1].into_iter().filter(|n| *n <= f.n_hi).collect();
            md.push_str(&ns.iter().map(|n| format!("n = {n}: {}, {} blocks", val(f, *n), f.blocks.eval(*n))).collect::<Vec<_>>().join(" · "));
            let j = judged(f);
            md.push_str(&format!(
                "\n\nExtend: {}. n = {}..{} ({} .. {}){}.\n",
                f.rule.trim_end_matches('.'),
                f.n_lo,
                f.n_hi,
                val(f, f.n_lo),
                val(f, f.n_hi),
                if j.is_empty() { String::new() } else { format!("; judged in the game: n = {j}") }
            ));
        } else {
            // a group: the general formula when given, else the formulas
            let fs: Vec<&Family> = card.members.iter().map(|&i| &fams[i].1).collect();
            let mut texts: Vec<String> = Vec::new();
            for g in &fs {
                if !texts.contains(&g.period.text()) {
                    texts.push(g.period.text());
                }
            }
            let heading = fs.iter().find_map(|g| g.group_period.clone()).unwrap_or_else(|| texts.join(" · "));
            let title = fs.iter().find_map(|g| g.group_title.clone()).or_else(|| card.group.clone()).unwrap_or_else(|| format!("{} (one block swapped: {} versions)", f.id, fs.len()));
            let note = fs.iter().find_map(|g| g.group_note.clone()).map(|n| format!(" · {n}")).unwrap_or_default();
            md.push_str(&format!("\n### {label} = {heading}{u}\n\n**{title}** · {} families{note}\n\n| family | {label} | blocks | n ({label}) | judged n |\n|---|---|---|---|---|\n", fs.len()));
            for &i in &card.members {
                let (mr, mf, _) = &fams[i];
                md.push_str(&format!("| [{}](archive/{}) | {} | {} | {}..{} ({} .. {}) | {} |\n", mf.id, mr.file, mf.period.text(), mf.blocks.text(), mf.n_lo, mf.n_hi, mf.period.eval(mf.n_lo), mf.period.eval(mf.n_hi), judged(mf)));
            }
        }
    }
    for (r, f, _) in fams {
        ftsv.push_str(
            &[study.to_string(), f.id.clone(), f.group.clone().unwrap_or_default(), r.kind.clone(), f.period.text(), f.blocks.text(), f.n_lo.to_string(), f.n_hi.to_string(), format!("{study}/archive/{}", r.file), f.rule.clone(), judged(f)]
                .map(|s| s.replace(['\t', '\n'], " "))
                .join("\t"),
        );
        ftsv.push('\n');
    }
    md
}

/// A tunable group's name from a member's file: the stem without its trailing value
/// (`pulse-extender-repeater-L8.txt` -> `pulse-extender-repeater`).
fn group_label(file: &str) -> String {
    let stem = file.trim_end_matches(".txt");
    if let Some((head, tail)) = stem.rsplit_once('-') {
        let t = tail.trim_start_matches(['L', 'N', 'P', 'n']);
        if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
            return head.to_string();
        }
    }
    stem.to_string()
}

/// The axis values of a group as a short range: (`4..38`, ` (every 2)`), (`4, 6, 8`, ``).
fn range_text(vals: &[i64]) -> (String, String) {
    let mut v: Vec<i64> = vals.to_vec();
    v.sort();
    v.dedup();
    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a.abs() } else { gcd(b, a % b) }
    }
    match v.len() {
        0 => (String::new(), String::new()),
        1 => (v[0].to_string(), String::new()),
        n if n <= 3 => (v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", "), String::new()),
        n => {
            let g = v.windows(2).fold(0, |g, w| gcd(g, w[1] - w[0]));
            let full = ((v[n - 1] - v[0]) / g.max(1) + 1) as usize;
            let note = if full == n { format!(" (every {g})") } else { format!(" (step {g}: {n} of {full} values)") };
            (format!("{}..{}", v[0], v[n - 1]), note)
        }
    }
}

/// The number of family cards on a study page (families of one group share a card).
fn clock_card_count(fams: &[(Row, clock::extend::Family, BTreeSet<String>)]) -> usize {
    if fams.is_empty() {
        return 0;
    }
    let items: Vec<clock::catalog::CardItem> = fams
        .iter()
        .enumerate()
        .map(|(i, (r, f, k))| clock::catalog::CardItem { formula: f.period.text(), kinds: k.clone(), size: (f.blocks.eval(f.n_lo), r.volume, i), group: f.group.clone() })
        .collect();
    clock::catalog::card_groups(&items).len()
}

/// The "Tunable groups" section of a study page: one card per group (the value range as the
/// heading, the two smallest members, a compact table value -> blocks -> design).
fn tunable_cards(cards: &[(String, String, Vec<&Row>)], circuits: &BTreeMap<String, Circuit>, fams: &[(Row, clock::extend::Family, BTreeSet<String>)]) -> String {
    if cards.is_empty() {
        return String::new();
    }
    let mut md = String::from(
        "\n## Tunable groups\n\nEach card: ONE mechanism whose value is set by its repeater delays, its comparator modes or the length of its repeater line (a `group:` header, or the same layout with only those settings changed); its members are not repeated below.\n",
    );
    for (name, kind, members) in cards {
        let (label, unit) = axis_label(&members[0].axis);
        let u = if unit.is_empty() { String::new() } else { format!(" {unit}") };
        let vals: Vec<i64> = members.iter().filter_map(|r| axis_number(&r.axis)).collect();
        let meta = |k: &str| members.iter().find_map(|r| circuits.get(&r.file).and_then(|c| c.clock.design.meta.get(k).cloned()));
        let title = meta("group title").unwrap_or_else(|| name.clone());
        let note = meta("group note").unwrap_or_else(|| {
            if members.iter().any(|r| circuits.get(&r.file).is_some_and(|c| c.clock.design.meta.contains_key("group"))) {
                "set by the repeater delays and the length of the repeater line".to_string()
            } else {
                "the same layout, set by its repeater delays / comparator modes".to_string()
            }
        });
        let mut small: Vec<&&Row> = members.iter().collect();
        small.sort_by(|a, b| (a.blocks, axis_number(&a.axis), a.volume, &a.file).cmp(&(b.blocks, axis_number(&b.axis), b.volume, &b.file)));
        let smallest: Vec<String> = small.iter().take(2).map(|r| format!("[{}](archive/{}) ({}, {} blocks)", r.file.trim_end_matches(".txt"), r.file, r.axis, r.blocks)).collect();
        let (core, rnote) = range_text(&vals);
        md.push_str(&format!("\n### {label} = {core}{u}{rnote}\n\n**{title}** · {kind} · {} designs · {note}. Smallest: {}.\n", members.len(), smallest.join(", ")));
        let fam: Vec<String> = fams
            .iter()
            .filter(|(fr, _, _)| circuits.get(&fr.file).and_then(|c| c.clock.design.meta.get("group")).is_some_and(|g| g == name))
            .map(|(fr, f, _)| format!("[{}](archive/{}): {label} = {}{u}, {} blocks", f.id, fr.file, f.period.text(), f.blocks.text()))
            .collect();
        if !fam.is_empty() {
            md.push_str(&format!("\nAlso as an extendable family: {}.\n", fam.join("; ")));
        }
        md.push_str(&format!("\n| {label} | blocks | design | metrics |\n|---|---|---|---|\n"));
        for r in members {
            md.push_str(&format!("| {} | {} | [{}](archive/{}) | {} |\n", axis_number(&r.axis).map(|v| format!("{v}{u}")).unwrap_or_else(|| r.axis.clone()), r.blocks, r.file.trim_end_matches(".txt"), r.file, r.metrics));
        }
    }
    md
}

/// `mctas circuit catalog`: every study's CATALOG.md and variants.md, the index, catalog.tsv and
/// families.tsv, from the archives' rows (no game). Returns one line per study.
pub fn catalog(root: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    let mut index = String::from(
        "# Contraption studies\n\nEvery design judged in the real game (`mctas circuit verify`: 4 positions x 4 rotations, [docs/circuits.md](../../docs/circuits.md)), by study; the data the website renders is [catalog.tsv](catalog.tsv). Generated by `mctas circuit catalog`.\n\n| study | designs | shown | variants |\n|---|---|---|---|\n",
    );
    let mut tsv = String::from("study\tkind\taxis\tname\tpath\tmetrics\tblocks\tdims\tfamily\trobustness\tvariant_of\tcaption\tgroup\n");
    let mut ftsv = String::from("study\tfamily\tgroup\tkind\taxis_formula\tblocks_formula\tn_lo\tn_hi\tbase\trule\tjudged_n\n");
    for (study, title) in STUDIES {
        let dir = archive_dir(root, study);
        let rows = read_rows(&dir)?;
        let caps = read_captions(&dir);
        let aliases = read_aliases(&dir);
        // extendable families (bases with `extend` headers) and their judged members: one card
        // each (cards of one group share one), the members leave the value tables
        let mut fams: Vec<(Row, clock::extend::Family, BTreeSet<String>)> = Vec::new();
        for r in &rows {
            if let Ok(c) = Circuit::load(&dir.join(&r.file))
                && let Ok(Some(f)) = family_of(&c)
            {
                fams.push((r.clone(), f, c.kinds()));
            }
        }
        let instances = read_instances(&dir);
        let member_keys: BTreeSet<&String> = instances.iter().filter(|i| fams.iter().any(|(_, f, _)| f.id == i.family)).map(|i| &i.type_key).collect();
        let in_family = |r: &Row| fams.iter().any(|(fr, _, _)| fr.file == r.file) || member_keys.contains(&r.type_key);
        // TUNABLE GROUPS (vjeux 2026-10-19): designs that are one mechanism with other repeater
        // delays, comparator modes or a longer U are ONE card: a `group: <name>` header (stamped by
        // the generators), else the same tunable key (delays and modes masked) and kind
        let circuits: BTreeMap<String, Circuit> = rows.iter().filter_map(|r| Circuit::load(&dir.join(&r.file)).ok().map(|c| (r.file.clone(), c))).collect();
        let mut tgroups: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| !in_family(r)) {
            let Some(c) = circuits.get(&r.file) else { continue };
            let (kind_key, id) = match c.clock.design.meta.get("group") {
                // an explicit group spans the kind's argument too (`counter N`: one card for
                // every N of one mechanism)
                Some(g) => (r.kind.split_whitespace().next().unwrap_or(&r.kind).to_string(), format!("h:{g}")),
                None => (r.kind.clone(), format!("t:{}", tunable_key(c))),
            };
            tgroups.entry((kind_key, id)).or_default().push(r);
        }
        tgroups.retain(|_, v| v.len() >= 2);
        let mut group_of: BTreeMap<String, String> = BTreeMap::new();
        let mut group_cards: Vec<(String, String, Vec<&Row>)> = Vec::new();
        for ((kind, id), members) in &mut tgroups {
            members.sort_by(|a, b| (axis_key(&a.axis), a.blocks, a.volume, &a.file).cmp(&(axis_key(&b.axis), b.blocks, b.volume, &b.file)));
            let name = match id.strip_prefix("h:") {
                Some(g) => g.to_string(),
                None => group_label(&members.iter().min_by_key(|r| (r.blocks, r.volume, r.file.clone())).map(|r| r.file.clone()).unwrap_or_default()),
            };
            for m in members.iter() {
                group_of.insert(m.file.clone(), name.clone());
            }
            group_cards.push((name, kind.clone(), members.clone()));
        }
        group_cards.sort_by_key(|(n, k, m)| (axis_key(&m[0].axis), k.clone(), n.clone()));
        let in_group = |r: &Row| group_of.contains_key(&r.file);
        // the dominance fold within one kind and axis value: a design with more blocks that only
        // adds block kinds to a smaller one is its variant
        let mut groups: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
        for r in rows.iter().filter(|r| !in_family(r) && !in_group(r)) {
            groups.entry((r.kind.clone(), r.axis.clone())).or_default().push(r);
        }
        let mut host: BTreeMap<String, String> = BTreeMap::new();
        for g in groups.values() {
            let items: Vec<(usize, i64, String, BTreeSet<String>)> = g.iter().map(|r| (r.blocks, r.volume, r.file.clone(), r.kind_set())).collect();
            for (i, h) in clock::catalog::fold_hosts(&items).into_iter().enumerate() {
                if let Some(h) = h {
                    host.insert(g[i].file.clone(), g[h].file.clone());
                }
            }
        }
        // EQUIVALENT LAYOUTS: the same kind, value, block count and block kinds: one shown, the
        // others listed as "also N other layouts" (catalog.tsv variant_of)
        let mut layout: BTreeMap<String, String> = BTreeMap::new();
        let mut layouts_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
        {
            let mut eq: BTreeMap<(String, String, usize, String), Vec<&Row>> = BTreeMap::new();
            for r in rows.iter().filter(|r| !in_family(r) && !in_group(r) && !host.contains_key(&r.file)) {
                eq.entry((r.kind.clone(), r.axis.clone(), r.blocks, r.kinds.clone())).or_default().push(r);
            }
            for v in eq.values_mut() {
                if v.len() < 2 {
                    continue;
                }
                v.sort_by(|a, b| (a.volume, &a.archived, &a.file).cmp(&(b.volume, &b.archived, &b.file)));
                for o in &v[1..] {
                    layout.insert(o.file.clone(), v[0].file.clone());
                    layouts_of.entry(v[0].file.clone()).or_default().push(o.file.clone());
                }
            }
        }
        let shown: Vec<&Row> = rows.iter().filter(|r| !host.contains_key(&r.file) && !in_family(r) && !in_group(r) && !layout.contains_key(&r.file)).collect();
        let cards_md = family_cards(&fams, &instances, &mut ftsv, study);
        let groups_md = tunable_cards(&group_cards, &circuits, &fams);
        let sdir = root.join(study);
        if rows.is_empty() {
            index.push_str(&format!("| {title} | 0 | 0 | 0 |\n"));
            continue;
        }
        std::fs::create_dir_all(&sdir)?;
        // by axis value (numbers ascending), then the smallest first
        let mut by_axis: BTreeMap<(i64, String), Vec<&Row>> = BTreeMap::new();
        for r in &shown {
            by_axis.entry(axis_key(&r.axis)).or_default().push(r);
        }
        let mut md = format!(
            "# {title}\n\n{} designs judged in the real game: {} shown, {} variants (more blocks that only add block kinds to a smaller design with the same value: [variants](variants.md)), {} other layouts (same kind, value, size and block kinds as a shown design){}{}. [All studies](../CATALOG.md) · [docs/circuits.md](../../../docs/circuits.md). Generated by `mctas circuit catalog`.\n",
            rows.len(),
            shown.len(),
            host.len(),
            layout.len(),
            if fams.is_empty() { String::new() } else { format!(", {} extendable famil{} (their members are on the cards)", fams.len(), if fams.len() == 1 { "y" } else { "ies" }) },
            if group_cards.is_empty() { String::new() } else { format!(", {} tunable group{} ({} designs on their cards)", group_cards.len(), if group_cards.len() == 1 { "" } else { "s" }, group_of.len()) }
        );
        md.push_str(&cards_md);
        md.push_str(&groups_md);
        for ((_, ax), rs) in &mut by_axis {
            rs.sort_by(|a, b| (a.blocks, a.volume, &a.file).cmp(&(b.blocks, b.volume, &b.file)));
            md.push_str(&format!("\n## {ax}\n\n| design | kind | metrics | blocks | box | robustness | how it works |\n|---|---|---|---|---|---|---|\n"));
            for r in rs.iter() {
                let also = aliases.get(&r.file).map(|v| format!(" (also found as {})", v.join(", "))).unwrap_or_default();
                let others = layouts_of
                    .get(&r.file)
                    .map(|v| format!(" (also {} other layout{}: {})", v.len(), if v.len() == 1 { "" } else { "s" }, v.iter().map(|f| format!("[{}](archive/{f})", f.trim_end_matches(".txt"))).collect::<Vec<_>>().join(", ")))
                    .unwrap_or_default();
                md.push_str(&format!(
                    "| [{}](archive/{}){also}{others} | {} | {} | {} | {} | {} | {} |\n",
                    r.file.trim_end_matches(".txt"),
                    r.file,
                    r.kind,
                    r.metrics,
                    r.blocks,
                    r.dims,
                    r.robustness,
                    caps.get(&r.file).map(|s| s.replace('|', "/")).unwrap_or_default()
                ));
            }
        }
        std::fs::write(sdir.join("CATALOG.md"), md)?;
        let mut vmd = format!("# {title}: variants\n\n[{title}](CATALOG.md)\n\nA variant has the same kind and value as a smaller design (its host) and uses every block kind the host does, plus others.\n");
        let mut hosts: BTreeMap<&String, Vec<&Row>> = BTreeMap::new();
        for r in &rows {
            if let Some(h) = host.get(&r.file) {
                hosts.entry(h).or_default().push(r);
            }
        }
        for (h, vs) in &hosts {
            let hr = rows.iter().find(|r| &&r.file == h).unwrap();
            vmd.push_str(&format!("\n**[{}](archive/{})** ({}, {}; {} blocks: {}):\n\n| variant | blocks | adds |\n|---|---|---|\n", hr.file.trim_end_matches(".txt"), hr.file, hr.kind, hr.axis, hr.blocks, hr.kinds.replace('+', ", ")));
            let hk = hr.kind_set();
            for v in vs {
                let adds: Vec<String> = v.kind_set().difference(&hk).cloned().collect();
                vmd.push_str(&format!("| [{}](archive/{}) | {} | {} |\n", v.file.trim_end_matches(".txt"), v.file, v.blocks, if adds.is_empty() { "-".to_string() } else { adds.join(", ") }));
            }
        }
        std::fs::write(sdir.join("variants.md"), vmd)?;
        for r in &rows {
            tsv.push_str(
                &[
                    study.to_string(),
                    r.kind.clone(),
                    r.axis.clone(),
                    r.file.trim_end_matches(".txt").to_string(),
                    format!("{study}/archive/{}", r.file),
                    r.metrics.clone(),
                    r.blocks.to_string(),
                    r.dims.clone(),
                    r.family.clone(),
                    r.robustness.clone(),
                    host.get(&r.file).or_else(|| layout.get(&r.file)).cloned().unwrap_or_default(),
                    caps.get(&r.file).cloned().unwrap_or_default(),
                    group_of.get(&r.file).cloned().unwrap_or_default(),
                ]
                .map(|s| s.replace(['\t', '\n'], " "))
                .join("\t"),
            );
            tsv.push('\n');
        }
        let cards = shown.len() + group_cards.len() + clock_card_count(&fams);
        // the card count without tunable groups and layouts (the fold over every non-family row)
        let before = {
            let mut g: BTreeMap<(String, String), Vec<&Row>> = BTreeMap::new();
            for r in rows.iter().filter(|r| !in_family(r)) {
                g.entry((r.kind.clone(), r.axis.clone())).or_default().push(r);
            }
            let mut folded = 0usize;
            for v in g.values() {
                let items: Vec<(usize, i64, String, BTreeSet<String>)> = v.iter().map(|r| (r.blocks, r.volume, r.file.clone(), r.kind_set())).collect();
                folded += clock::catalog::fold_hosts(&items).into_iter().filter(|h| h.is_some()).count();
            }
            rows.iter().filter(|r| !in_family(r)).count() - folded + clock_card_count(&fams)
        };
        index.push_str(&format!("| [{title}]({study}/CATALOG.md) | {} | {} | {} |\n", rows.len(), shown.len(), host.len()));
        lines.push(format!(
            "{study}: {} designs, {} shown, {} variants, {} other layouts, {} families, {} tunable groups ({} designs), {} values; cards {before} -> {cards}",
            rows.len(),
            shown.len(),
            host.len(),
            layout.len(),
            fams.len(),
            group_cards.len(),
            group_of.len(),
            by_axis.len()
        ));
    }
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("CATALOG.md"), index)?;
    std::fs::write(root.join("catalog.tsv"), tsv)?;
    std::fs::write(root.join("families.tsv"), ftsv)?;
    Ok(lines)
}
