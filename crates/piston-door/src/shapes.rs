//! `mctas door shapes`: the shape of every working design relative to its door (layers in front
//! of and behind the door plane, rows below and above the opening, columns left and right of
//! it, bounding box; [`crate::Shape`]), then for each shape rule ([`ShapeLimits::PRESETS`] plus
//! any `--rule`) the best design today, per league, and the (volume, blocks) Pareto frontier.

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;
use mctas::design::Design;

use crate::leaderboard::Entry;
use crate::{Shape, ShapeLimits, block_count, shape};

/// A named shape rule.
pub struct Rule {
    pub name: String,
    pub terms: String,
    pub limits: ShapeLimits,
    pub what: String,
}

/// The presets, then each extra rule (a preset name or a `key<=n` list).
pub fn rules(extra: &[String]) -> Result<Vec<Rule>> {
    let mut out: Vec<Rule> = ShapeLimits::PRESETS
        .iter()
        .map(|(name, terms, what)| Ok(Rule { name: name.to_string(), terms: terms.to_string(), limits: ShapeLimits::parse(terms)?, what: what.to_string() }))
        .collect::<Result<_>>()?;
    for r in extra {
        out.push(Rule { name: r.clone(), terms: r.clone(), limits: ShapeLimits::parse(r)?, what: "extra rule".into() });
    }
    Ok(out)
}

/// Every design file in `dir` with door and lever ports, at most `max_blocks` blocks (files kept
/// for history as invalid are skipped), deduplicated by content.
pub fn collect_dir(dir: &Path, max_blocks: usize) -> Result<Vec<(String, Design)>> {
    let mut files: Vec<_> = std::fs::read_dir(dir)?.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    files.sort();
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for f in files {
        if f.file_name().is_some_and(|n| n.to_string_lossy().contains("invalid")) {
            continue;
        }
        let Ok(d) = Design::load(&f) else { continue };
        if !d.ports.contains_key("door") || !d.ports.contains_key("lever") || block_count(&d) > max_blocks {
            continue;
        }
        if seen.insert(format!("{:?}|{:?}", d.blocks, d.ports)) {
            out.push((f.display().to_string(), d));
        }
    }
    Ok(out)
}

/// The leagues a rule's best is reported in: (label, filter). Boards rank rotation-safe designs.
fn leagues() -> Vec<(&'static str, Box<dyn Fn(&Entry) -> bool>)> {
    vec![
        ("solid", Box::new(|e: &Entry| e.category == "solid")),
        ("slime-free", Box::new(|e: &Entry| e.category == "solid" && crate::slime_free(&e.design))),
        ("sculk", Box::new(|e: &Entry| e.category == "sculk")),
        ("gravity", Box::new(|e: &Entry| e.category == "gravity")),
    ]
}

fn stem(source: &str) -> String {
    let file = source.rsplit(['/', ':']).next().unwrap_or(source);
    file.trim_end_matches(".txt").to_string()
}

/// Short name of a design source for tables: the file stem, with the track for files of a
/// track folder or an explore branch (`origin/explore/x:.../tracks/x/BEST.txt` -> `x:BEST`).
pub fn short_source(source: &str) -> String {
    if let Some(rest) = source.split("/tracks/").nth(1) {
        if let Some((track, _)) = rest.split_once('/') {
            return format!("{track}:{}", stem(source));
        }
    }
    match source.split_once(':') {
        Some((r, _)) if r.starts_with("origin/explore/") => format!("{}:{}", r.trim_start_matches("origin/explore/"), stem(source)),
        _ => stem(source),
    }
}

/// The shape a board judges: the swept one (space needed at rest and while moving) when known.
fn board_shape(e: &Entry) -> Option<Shape> {
    e.swept.or_else(|| shape(&e.design))
}

/// Where the swept shape exceeds the rest shape ("L+1 B+2": the machine reaches one column
/// further left and two layers further behind while it moves).
fn moves_out(rest: &Shape, swept: &Shape) -> String {
    let parts = [
        ("F", swept.front - rest.front),
        ("B", swept.behind - rest.behind),
        ("D", swept.below - rest.below),
        ("U", swept.above - rest.above),
        ("L", swept.left - rest.left),
        ("R", swept.right - rest.right),
    ];
    parts.iter().filter(|(_, d)| *d > 0).map(|(k, d)| format!("{k}+{d}")).collect::<Vec<_>>().join(" ")
}

/// The table of every working design: blocks, category, slime-free, rotations, then the space it
/// needs (swept shape), what moves beyond its rest shape, and the preset rules it satisfies.
pub fn table(entries: &[Entry], rules: &[Rule], table_max: usize) -> String {
    let mut s = String::from(
        "Shapes are the space each door needs, at rest and while it moves (swept); 'moves out' = beyond the rest shape \
         (F front, B behind, D down, U up, L left, R right). below/above: rows (blocks at rest).\n\
         blocks cat     sf  rot  thick(F+B) below above left right  box WxHxT  volume  moves out  rules                                   design\n",
    );
    let mut rows: Vec<(&Entry, Shape)> = entries.iter().filter(|e| e.verdict.is_ok() && e.blocks <= table_max).filter_map(|e| Some((e, board_shape(e)?))).collect();
    rows.sort_by(|a, b| (a.0.category != "solid", a.0.blocks, a.1.volume(), &a.0.source).cmp(&(b.0.category != "solid", b.0.blocks, b.1.volume(), &b.0.source)));
    for (e, sh) in rows {
        // A rule asking for nothing below also needs the door to work on a stone floor: "!"
        // marks a design that fails (or has not passed) the ground check.
        let ok: Vec<String> = rules
            .iter()
            .filter(|r| r.limits.allows(&sh))
            .map(|r| if r.limits.below == Some(0) && !e.ground_safe() { format!("{}!", r.name) } else { r.name.clone() })
            .collect();
        let rot = match &e.rotation {
            Some(Ok(())) => "safe".to_string(),
            Some(Err((deg, _))) => format!("x{deg}"),
            None => "-".into(),
        };
        let out = shape(&e.design).map(|r| moves_out(&r, &sh)).unwrap_or_default();
        s.push_str(&format!(
            "{:>6} {:<7} {:<3} {:<4} {:>2} ({}+{})   {:>5} {:>5} {:>4} {:>5}  {:>3}x{:<2}x{:<2}  {:>6}  {:<9}  {:<39} {}\n",
            e.blocks,
            e.category,
            if crate::slime_free(&e.design) { "yes" } else { "" },
            rot,
            sh.thickness(),
            sh.front,
            sh.behind,
            format!("{}({})", sh.below, sh.blocks_below),
            format!("{}({})", sh.above, sh.blocks_above),
            sh.left,
            sh.right,
            sh.width(),
            sh.height(),
            sh.thickness(),
            sh.volume(),
            out,
            ok.join(" "),
            short_source(&e.source)
        ));
    }
    s
}

/// For each rule, the best rotation-safe design of each league (fewest blocks, ties by smaller
/// box), how many working rotation-safe solid designs satisfy it, and the best design that only
/// fails on rotations; then the (volume, blocks) Pareto frontier of the solid designs. Rules
/// apply to the swept shapes.
pub fn best_report(entries: &[Entry], rules: &[Rule]) -> String {
    let working: Vec<(&Entry, Shape)> = entries.iter().filter(|e| e.verdict.is_ok()).filter_map(|e| Some((e, board_shape(e)?))).collect();
    let mut s = String::from("\nBest design per shape rule (rotation-safe; fewest blocks, ties by smaller box):\n");
    let none = || "-".to_string();
    for r in rules {
        s.push_str(&format!("\n{} ({}): {}\n", r.name, r.terms, r.what));
        // Nothing below the door must also work standing on a stone floor (ground check).
        let fits_rule = |e: &Entry, sh: &Shape| r.limits.allows(sh) && (r.limits.below != Some(0) || e.ground_safe());
        for (label, keep) in leagues() {
            let fits: Vec<&(&Entry, Shape)> = working.iter().filter(|(e, sh)| e.rotation_safe() && keep(e) && fits_rule(e, sh)).collect();
            let best = fits.iter().min_by_key(|(e, sh)| (e.blocks, sh.volume(), e.source.clone()));
            let line = match best {
                Some((e, sh)) => format!(
                    "{:>3} blocks  {}x{}x{} box (thick {}, below {}, above {}, sides {}/{})  {}  [{} designs]",
                    e.blocks,
                    sh.width(),
                    sh.height(),
                    sh.thickness(),
                    sh.thickness(),
                    sh.below,
                    sh.above,
                    sh.left,
                    sh.right,
                    e.source,
                    fits.len()
                ),
                None => none(),
            };
            s.push_str(&format!("  {label:<10} {line}\n"));
        }
        let unrot = working.iter().filter(|(e, sh)| !e.rotation_safe() && fits_rule(e, sh)).min_by_key(|(e, _)| (e.blocks, e.source.clone()));
        if let Some((e, _)) = unrot {
            s.push_str(&format!("  (not rotation-safe: {} blocks {} {})\n", e.blocks, e.category, e.source));
        }
        if r.limits.below == Some(0) {
            for (e, _) in working.iter().filter(|(e, sh)| r.limits.allows(sh) && !e.ground_safe()) {
                let why = match &e.ground {
                    Some(Err(why)) => why.chars().take(160).collect::<String>(),
                    _ => "not checked".into(),
                };
                s.push_str(&format!("  (fails on a stone floor: {} blocks {}: {why})\n", e.blocks, e.source));
            }
        }
    }
    // Smallest box: the (volume, blocks) frontier, solid and rotation-safe.
    let solid: Vec<&(&Entry, Shape)> = working.iter().filter(|(e, _)| e.rotation_safe() && e.category == "solid").collect();
    let mut front: Vec<&&(&Entry, Shape)> = solid
        .iter()
        .filter(|(a, sa)| !solid.iter().any(|(b, sb)| sb.volume() <= sa.volume() && b.blocks <= a.blocks && (sb.volume() < sa.volume() || b.blocks < a.blocks)))
        .collect();
    front.sort_by_key(|(e, sh)| (sh.volume(), e.blocks, e.source.clone()));
    front.dedup_by_key(|(e, sh)| (sh.volume(), e.blocks));
    s.push_str("\nSmallest box (solid, rotation-safe): the (volume, blocks) Pareto frontier\n");
    for (e, sh) in front {
        s.push_str(&format!(
            "  volume {:>4} ({}x{}x{})  {:>3} blocks  {}{}\n",
            sh.volume(),
            sh.width(),
            sh.height(),
            sh.thickness(),
            e.blocks,
            e.source,
            if crate::slime_free(&e.design) { "  (slime-free)" } else { "" }
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use crate::{ShapeLimits, shape};
    use mctas::design::Design;

    fn archived(name: &str) -> Design {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../designs/piston-door-3x3/archive").join(name);
        Design::load(&path).unwrap()
    }

    #[test]
    fn fc2_shape() {
        // fc2: z 0..1, P_M two below the door, B's line up to y 6, P_L at x -3, the right
        // structure at x 3.
        let s = shape(&archived("fc2-40b.txt")).unwrap();
        assert_eq!((s.front, s.behind, s.below, s.above, s.left, s.right), (0, 1, 2, 4, 3, 1));
        assert_eq!((s.thickness(), s.width(), s.height(), s.volume()), (2, 7, 9, 126));
        assert_eq!(s.blocks_below, 3);
        assert!(ShapeLimits::parse("wall").unwrap().allows(&s));
        assert!(ShapeLimits::parse("wall2").unwrap().allows(&s));
        assert!(!ShapeLimits::parse("ground").unwrap().allows(&s));
        assert!(!ShapeLimits::parse("narrow2").unwrap().allows(&s));
        assert!(ShapeLimits::parse("side<=3,below<=2").unwrap().allows(&s));
        assert!(ShapeLimits::parse("volume=0").unwrap().volume == Some(0));
        assert!(ShapeLimits::parse("depth<=1").is_err());
    }
}
